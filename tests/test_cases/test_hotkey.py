"""
热键响应测试

测试 voice-ime 热键触发功能：
- Toggle 模式
- PTT 模式
- ESC 取消
"""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

import pytest

if sys.platform != "win32":
    pytest.skip("Win32-only tests", allow_module_level=True)

from ..conftest import kill_existing_voice_ime, voice_ime_config_file, wait_for_condition
from ..sendinput_hotkey import (
    press_alt_grave,
    press_ctrl_space,
    press_escape,
    tap_f9,
)
from ..utils.state_detector import OverlayState, detect_overlay_state


# 配置文件路径：主程序读取 <exe_dir>/config.toml（同源 src/config/mod.rs:340-346），
# harness 必须与之一致（[E2E-CONFIG-PATH-STALE-001] 错位一修复）。
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


def _format_value(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, str):
        return f'"{value}"'
    raise TypeError(f"Unsupported config value: {value!r}")


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


def _wait_for_overlay_state(expected_state: OverlayState, timeout: float = 5.0, interval: float = 0.1) -> bool:
    return wait_for_condition(
        lambda: detect_overlay_state() == expected_state,
        timeout=timeout,
        interval=interval,
        description=f"overlay state {expected_state.value}",
    )


def _wait_for_overlay_not_state(unexpected_state: OverlayState, timeout: float = 5.0, interval: float = 0.1) -> bool:
    return wait_for_condition(
        lambda: detect_overlay_state() != unexpected_state,
        timeout=timeout,
        interval=interval,
        description=f"overlay state not {unexpected_state.value}",
    )


@pytest.fixture
def hotkey_config_guard() -> None:
    # 字节级备份/还原：避免文本模式读写下把 LF 改写为 CRLF（E2E-CONFIG-PATH 副效应）
    _ensure_config_file()
    backup = CONFIG_FILE.read_bytes()
    try:
        yield
    finally:
        CONFIG_FILE.write_bytes(backup)


@pytest.fixture
def toggle_f9_config(hotkey_config_guard) -> None:
    _write_config_section("hotkey", "vk_code", 0x78)
    _write_config_section("hotkey", "modifiers", 0)
    _write_config_section("hotkey", "display_name", "F9")
    _write_config_section("hotkey", "mode", "Toggle")
    yield


@pytest.fixture
def ptt_f9_config(hotkey_config_guard) -> None:
    _write_config_section("hotkey", "vk_code", 0x78)
    _write_config_section("hotkey", "modifiers", 0)
    _write_config_section("hotkey", "display_name", "F9")
    _write_config_section("hotkey", "mode", "PushToTalk")
    yield


@pytest.fixture
def ctrl_space_config(hotkey_config_guard) -> None:
    _write_config_section("hotkey", "vk_code", 0x20)
    _write_config_section("hotkey", "modifiers", 0x0002)
    _write_config_section("hotkey", "display_name", "Ctrl+Space")
    _write_config_section("hotkey", "mode", "Toggle")
    yield


@pytest.fixture
def alt_grave_config(hotkey_config_guard) -> None:
    _write_config_section("hotkey", "vk_code", 0xC0)
    _write_config_section("hotkey", "modifiers", 0x0001)
    _write_config_section("hotkey", "display_name", "Alt+`")
    _write_config_section("hotkey", "mode", "Toggle")
    yield


def _prewarm_recording(start_gesture, stop_gesture, settle: float = 2.5, attempts: int = 3) -> None:
    """冷启动预热（[E2E-COLD-START-RACE-001] harness 修复）。

    cold 启动后 worker 处理首个 Start 会触发 transcriber 热重载/重建
    （src/main.rs:3971-3973 处处理 Start 时无条件清空 cancel/stop 信号），
    若 Stop/CancelStop 在该窗口内到达会被清掉 → 录音无法停止。
    warm 进程快速手势 4/4 全部成功（探针证据），故在断言序列前先完成一轮
    Start→settle→Stop 往返验证 worker 已就绪。不改产品码、不弱化断言语义。
    """
    for attempt in range(1, attempts + 1):
        start_gesture()
        if not _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0):
            raise AssertionError(
                f"prewarm attempt {attempt}: overlay not recording after start gesture"
            )
        time.sleep(settle)
        stop_gesture()
        if _wait_for_overlay_not_state(OverlayState.RECORDING, timeout=6.0):
            return
        time.sleep(1.0)
    raise AssertionError(
        f"prewarm failed after {attempts} attempts: worker could not stop recording "
        "(cold-start transcriber reload window persisted?)"
    )


@pytest.fixture
def hotkey_test_process(exe_path: Path):
    kill_existing_voice_ime()
    process = subprocess.Popen(
        [str(exe_path)],
        cwd=str(exe_path.parent),
        creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
    )
    try:
        initialized = wait_for_condition(
            lambda: process.poll() is None,
            timeout=10.0,
            interval=0.2,
            description="hotkey test process initialization",
        )
        assert initialized and process.poll() is None, (
            f"voice-ime failed to initialize for hotkey test, exit={process.poll()}"
        )
        time.sleep(1.0)
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
def toggle_voice_ime_process(toggle_f9_config, hotkey_test_process: subprocess.Popen):
    yield hotkey_test_process


@pytest.fixture
def ptt_voice_ime_process(ptt_f9_config, hotkey_test_process: subprocess.Popen):
    yield hotkey_test_process


@pytest.fixture
def ctrl_space_voice_ime_process(ctrl_space_config, hotkey_test_process: subprocess.Popen):
    yield hotkey_test_process


@pytest.fixture
def alt_grave_voice_ime_process(alt_grave_config, hotkey_test_process: subprocess.Popen):
    yield hotkey_test_process


@pytest.mark.timeout(60)
@pytest.mark.gui
class TestHotkeyToggleMode:
    """Toggle 模式热键测试"""

    def test_hotkey_toggle_start(
        self,
        toggle_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 Toggle 模式启动录音（F9）"""
        wait_for_condition(lambda: toggle_voice_ime_process.poll() is None, timeout=5.0)
        assert toggle_voice_ime_process.poll() is None, "voice-ime should be running"
        assert _wait_for_overlay_state(OverlayState.HIDDEN, timeout=2.0)

        tap_f9()

        assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0), (
            f"Expected recording overlay after F9, got {detect_overlay_state().value}"
        )

    def test_hotkey_toggle_stop(
        self,
        toggle_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 Toggle 模式停止录音（再次 F9）"""
        wait_for_condition(lambda: toggle_voice_ime_process.poll() is None, timeout=5.0)
        _prewarm_recording(tap_f9, tap_f9)
        tap_f9()
        assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0)

        tap_f9()

        assert _wait_for_overlay_not_state(OverlayState.RECORDING, timeout=5.0), (
            "Overlay stayed in recording state after second F9"
        )


@pytest.mark.timeout(60)
@pytest.mark.gui
class TestHotkeyPTTMode:
    """PTT 模式热键测试"""

    def test_hotkey_ptt_hold(
        self,
        ptt_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 PTT 模式按住录音"""
        wait_for_condition(lambda: ptt_voice_ime_process.poll() is None, timeout=5.0)
        assert _wait_for_overlay_state(OverlayState.HIDDEN, timeout=2.0)
        _prewarm_recording(tap_f9, tap_f9)

        from ..sendinput_hotkey import key_down, key_up, VK_F9

        _prewarm_recording(
            lambda: key_down(VK_F9),
            lambda: key_up(VK_F9),
            settle=0.5,
        )

        key_down(VK_F9)
        try:
            assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0), (
                f"Expected recording overlay while holding F9, got {detect_overlay_state().value}"
            )
        finally:
            key_up(VK_F9)

        assert _wait_for_overlay_not_state(OverlayState.RECORDING, timeout=5.0), (
            "Overlay stayed in recording state after releasing F9 in PTT mode"
        )


@pytest.mark.timeout(60)
@pytest.mark.gui
class TestHotkeyCancel:
    """ESC 取消测试"""

    def test_hotkey_cancel_recording(
        self,
        toggle_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 ESC 取消录音"""
        wait_for_condition(lambda: toggle_voice_ime_process.poll() is None, timeout=5.0)
        _prewarm_recording(tap_f9, tap_f9)
        tap_f9()
        assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0)

        press_escape()

        assert _wait_for_overlay_not_state(OverlayState.RECORDING, timeout=5.0), (
            "Overlay stayed in recording state after ESC"
        )


@pytest.mark.timeout(60)
@pytest.mark.gui
class TestHotkeyPresets:
    """热键预设测试"""

    def test_ctrl_space_hotkey(
        self,
        ctrl_space_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 Ctrl+Space 热键"""
        wait_for_condition(lambda: ctrl_space_voice_ime_process.poll() is None, timeout=5.0)
        assert _wait_for_overlay_state(OverlayState.HIDDEN, timeout=2.0)

        press_ctrl_space()

        assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0), (
            f"Expected recording overlay after Ctrl+Space, got {detect_overlay_state().value}"
        )

    def test_alt_grave_hotkey(
        self,
        alt_grave_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 Alt+` 热键"""
        wait_for_condition(lambda: alt_grave_voice_ime_process.poll() is None, timeout=5.0)
        assert _wait_for_overlay_state(OverlayState.HIDDEN, timeout=2.0)

        press_alt_grave()

        assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0), (
            f"Expected recording overlay after Alt+`, got {detect_overlay_state().value}"
        )
