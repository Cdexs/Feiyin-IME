"""
OVERLAY-054-B / 051-G E2E 位置断言

背景：Gavin 两次报告"录音窗口闪屏幕左上角"（OVERLAY-054-B）。这是纯单测永远抓不到的类——
`overlay_geometry`（src/main.rs:3114-3134）与 `monitor_work_rect`（:3093）依赖真实 HWND 与
显示器工作区，E2E 是唯一能兜住它的层。

本文件新增面向"窗口位置"的断言（此前 harness 只判尺寸/状态，从不看 (left, top)）：

  1. 左上角排除：不出现 (0,0)/(小坐标) 闪角 —— 直击 [0,0] 硬编码回归
  2. 底部区域：overlay 垂直中心落在屏幕工作区下半部（生产契约 y = work.top + (work_h - h - 64)）
  3. 水平居中：overlay 水平中心与工作区水平中心之差 ≤ 容差（默认 20px，与尺寸容差带同级）

判据全部与生产同源（non 硬编码，跳过 E2E-OVERLAY-SIZE-STALE-001 类错位）：
  - 尺寸：STATE_SIZES 由 src/main.rs 常量实时解析（E2E-HARNESS-050 既有机制）
  - 底部上移量：OVERLAY_BOTTOM_OFFSET_PX 由 src/main.rs overlay_geometry max(0) 公式实时解析
  - 工作区：get_overlay_work_area() 用与生产 monitor_work_rect 相同的 Win32 调用

覆盖两条路径：
  - 本地模型（performance）→ 热键 Start 首次 Show 走 overlay_geometry 显式传 pos（:3420）
  - 在线流式（qwen_audio_online）→ 首次 Show 走 show_overlay_streaming_idle 传 pos=None，
    overlay 线程内 resolve 到 overlay_geometry（:1044）——Gavin 撞上的就是这条。
    在线路径需要真实 API Key + 网络；无 Key/离线时如实 SKIP（不造假绿）。
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

import pytest

if sys.platform != "win32":
    pytest.skip("Win32-only tests", allow_module_level=True)

from ..conftest import (
    kill_existing_voice_ime,
    voice_ime_config_file,
    wait_for_condition,
)
from ..sendinput_hotkey import tap_f9
from ..utils.state_detector import (
    OVERLAY_BOTTOM_OFFSET_PX,
    STATE_SIZES,
    OverlayState,
    detect_overlay_state,
    find_overlay_window,
    get_overlay_work_area,
    get_window_rect,
)


# 与尺寸容差带（TOLERANCE_PX=15）同级的位置容差：吸 1px 边框与多显示器 DPI 抖动。
# 任务书建议 ≤ 20px。生产 x 用整除 (work_w - w) / 2，本机整数坐标差异通常 ≤ 2px。
POSITION_TOLERANCE_PX = 20

# 左上角排除阈值：窗口出现在屏幕最左上 ~100x100 内视为 [0,0] 闪角回归
TOP_LEFT_EXCLUDE = 100

CONFIG_FILE = voice_ime_config_file()
CONFIG_DIR = CONFIG_FILE.parent


def _ensure_config_file() -> None:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    if not CONFIG_FILE.exists():
        CONFIG_FILE.write_text("", encoding="utf-8")


def _read_config_text() -> str:
    _ensure_config_file()
    return CONFIG_FILE.read_text(encoding="utf-8")


def _write_config_text(content: str) -> None:
    CONFIG_FILE.write_text(content, encoding="utf-8")


def _write_config_section(section: str, key: str, value: object) -> None:
    content = _read_config_text()
    lines = content.splitlines()
    section_header = f"[{section}]"
    section_start = None
    section_end = len(lines)

    for index, line in enumerate(lines):
        if line.strip() == section_header:
            section_start = index
            continue
        if section_start is not None and line.strip().startswith("["):
            section_end = index
            break

    if section_start is None:
        if lines and lines[-1] != "":
            lines.append("")
        lines.extend([section_header, f"{key} = {_format_value(value)}"])
        _write_config_text("\n".join(lines) + "\n")
        return

    for index in range(section_start + 1, section_end):
        if lines[index].strip().startswith(f"{key} =") or lines[index].strip().startswith(f"{key}="):
            lines[index] = f"{key} = {_format_value(value)}"
            _write_config_text("\n".join(lines) + "\n")
            return

    lines.insert(section_end, f"{key} = {_format_value(value)}")
    _write_config_text("\n".join(lines) + "\n")


def _format_value(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, str):
        return f'"{value}"'
    raise TypeError(f"Unsupported config value: {value!r}")


def _wait_for_overlay_state(expected: OverlayState, timeout: float = 8.0, interval: float = 0.1) -> bool:
    return wait_for_condition(
        lambda: detect_overlay_state() == expected,
        timeout=timeout,
        interval=interval,
        description=f"overlay state {expected.value}",
    )


def _assert_overlay_position() -> dict:
    """断言 overlay 当前出现位置满足 OVERLAY-051-G/054-B 几何契约，返回窗口信息。"""
    info = None
    hwnd = find_overlay_window()
    assert hwnd, "overlay window must exist for position assertion"

    def _collect():
        nonlocal info
        info = (
            get_window_rect(hwnd),
            get_overlay_work_area(hwnd),
            detect_overlay_state(),
        )
        return info[0] is not None and info[1] is not None

    assert wait_for_condition(
        _collect, timeout=5.0, interval=0.1, description="overlay rect + work area"
    ), "overlay rect / work area unavailable"

    (left, top, right, bottom), (wl, wt, wr, wb), state = info
    width = right - left
    height = bottom - top
    exp_w, exp_h = STATE_SIZES["recording"]

    # 1) 左上角排除：直击 [0,0] 硬编码回归（OVERLAY-054-B）
    assert not (left < TOP_LEFT_EXCLUDE and top < TOP_LEFT_EXCLUDE), (
        f"overlay flashed at screen top-left ({left},{top}) — OVERLAY-054-B regression, "
        f"state={state.value}"
    )

    # 允许 processing(同 240x36) / streaming 变体，位置契约均来自 overlay_geometry(fallback)
    assert (width, height) == (exp_w, exp_h), (
        f"unexpected overlay size {width}x{height}, expected {exp_w}x{exp_h}, state={state.value}"
    )

    # 2) 水平居中（生产：x = work.left + (work_w - w) / 2）
    work_w = wr - wl
    work_center_x = wl + work_w / 2
    overlay_center_x = left + width / 2
    assert abs(overlay_center_x - work_center_x) <= POSITION_TOLERANCE_PX, (
        f"overlay not horizontally centered: center_x={overlay_center_x:.1f} "
        f"vs work_center_x={work_center_x:.1f}, state={state.value}"
    )

    # 3) 底部区域（生产：y = work.top + (work_h - h - OFFSET).max(0)）
    work_h = wb - wt
    overlay_center_y = top + height / 2
    work_center_y = wt + work_h / 2
    assert overlay_center_y > work_center_y, (
        f"overlay vertical center above work-area half: center_y={overlay_center_y:.1f} "
        f"vs work_center_y={work_center_y:.1f}, state={state.value}"
    )
    expected_y = wt + max(work_h - height - OVERLAY_BOTTOM_OFFSET_PX, 0)
    assert abs(top - expected_y) <= POSITION_TOLERANCE_PX, (
        f"overlay bottom-anchor y off contract: top={top} expected={expected_y} "
        f"(offset={OVERLAY_BOTTOM_OFFSET_PX}), state={state.value}"
    )

    return {"left": left, "top": top, "width": width, "height": height}


@pytest.fixture
def position_config_guard() -> None:
    """字节级备份/还原 config.toml（避免 LF→CRLF 副作用）。"""
    _ensure_config_file()
    backup = CONFIG_FILE.read_bytes()
    try:
        yield
    finally:
        CONFIG_FILE.write_bytes(backup)


@pytest.fixture
def toggle_f9_position_config(position_config_guard) -> None:
    _write_config_section("hotkey", "vk_code", 0x78)
    _write_config_section("hotkey", "modifiers", 0)
    _write_config_section("hotkey", "display_name", "F9")
    _write_config_section("hotkey", "mode", "Toggle")
    yield


def _prewarm_recording(timeout: float = 2.5) -> None:
    """[E2E-COLD-START-RACE-001] 预热：断言序列前先跑一轮 Start→Stop 往返。"""
    for attempt in range(1, 4):
        tap_f9()
        if not _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0):
            raise AssertionError(f"prewarm {attempt}: not recording after F9")
        time.sleep(timeout)
        tap_f9()
        if _wait_for_overlay_state(OverlayState.HIDDEN, timeout=6.0):
            return
        time.sleep(1.0)
    raise AssertionError("prewarm failed: worker could not stop recording")


@pytest.fixture
def local_position_process(exe_path: Path, toggle_f9_position_config) -> subprocess.Popen:
    """本地模型（performance）：启动进程并预热。"""
    kill_existing_voice_ime()
    process = subprocess.Popen(
        [str(exe_path)],
        cwd=str(exe_path.parent),
        creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
    )
    try:
        assert wait_for_condition(
            lambda: process.poll() is None,
            timeout=10.0,
            interval=0.2,
            description="local voice-ime init",
        ), f"voice-ime exited early, code={process.poll()}"
        time.sleep(1.0)
        _prewarm_recording()
        yield process
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        kill_existing_voice_ime()


@pytest.fixture
def online_position_process(exe_path: Path, position_config_guard) -> subprocess.Popen:
    """在线流式（qwen_audio_online）：需 API Key + 网络，缺则 SKIP（如实标注）。"""
    api_key = os.getenv("VOICE_IME_ONLINE_ASR_KEY", "").strip()
    if not api_key:
        pytest.skip(
            "在线流式路径需要真实 ASR API Key 与网络（触发热键 Start 后走 "
            "show_overlay_streaming_idle -> overlay 线程 resolve overlay_geometry，"
            "即 Gavin 撞上的 OVERLAY-054-B 路径）。未设置环境变量 "
            "VOICE_IME_ONLINE_ASR_KEY，SKIP。"
        )
    _write_config_section("audio", "asr_model", "qwen_audio_online")
    _write_config_section("audio", "asr_online_api_key", api_key)
    _write_config_section("hotkey", "vk_code", 0x78)
    _write_config_section("hotkey", "modifiers", 0)
    _write_config_section("hotkey", "display_name", "F9")
    _write_config_section("hotkey", "mode", "Toggle")

    kill_existing_voice_ime()
    process = subprocess.Popen(
        [str(exe_path)],
        cwd=str(exe_path.parent),
        creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
    )
    try:
        assert wait_for_condition(
            lambda: process.poll() is None,
            timeout=10.0,
            interval=0.2,
            description="online voice-ime init",
        ), f"voice-ime exited early, code={process.poll()}"
        time.sleep(1.0)
        _prewarm_recording()
        yield process
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        kill_existing_voice_ime()


@pytest.mark.timeout(60)
@pytest.mark.gui
class TestOverlayPosition:
    """录音 overlay 位置断言（OVERLAY-051-G/054-B 的 E2E 唯一真护栏）。"""

    def test_recording_overlay_not_top_left_and_centered(
        self,
        local_position_process: subprocess.Popen,
    ) -> None:
        """本地模型：F9 录制，overlay 不得闪左上角，且水平居中、底部贴边上移 64px。"""
        tap_f9()
        assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0), (
            f"expected recording overlay, got {detect_overlay_state().value}"
        )
        _assert_overlay_position()

    def test_stop_recording_overlay_still_valid_position(
        self,
        local_position_process: subprocess.Popen,
    ) -> None:
        """本地模型：停止录音后的过渡 overlay（Recording→Processing，仍 240x36）位置契约保持。"""
        tap_f9()
        assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0)
        _assert_overlay_position()

        tap_f9()
        # 停止过渡（FallingToProcessing / Processing）仍是 240x36，尺寸无法区分 recording；
        # 只要窗口仍可见，就继续用同一契约断言位置（不能闪回左上角）。
        hwnd = find_overlay_window()
        if hwnd:
            _assert_overlay_position()

    def test_online_streaming_overlay_not_top_left(
        self,
        online_position_process: subprocess.Popen,
    ) -> None:
        """在线流式：首次 Show 走 show_overlay_streaming_idle（pos=None），resolve 后必须正确。"""
        tap_f9()
        assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0), (
            f"expected recording (streaming-idle) overlay, got {detect_overlay_state().value}"
        )
        _assert_overlay_position()