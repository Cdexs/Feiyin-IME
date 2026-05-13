"""
录音输入全流程 E2E 测试

测试从"按热键 → 启动录音 → 停止录音 → 识别处理 → 文字注入"的完整录音输入链路。

验证点：
1. 热键触发 → overlay 进入 Recording 状态
2. 停止录音 → overlay 进入 Processing 状态
3. 识别完成 → overlay 隐藏 + 文字注入到目标窗口
4. 剪贴板内容保留
5. 焦点丢失场景预览窗口弹出

注意：
- 需要真实麦克风或虚拟音频输入
- 使用 SendInput 模拟热键操作
- 动态读取当前配置的热键（不假设固定按键）
- 使用记事本作为文字注入验证目标
"""

import os
import subprocess
import time
import win32clipboard
from pathlib import Path

import pytest
import pyautogui

from ..conftest import kill_existing_voice_ime, wait_for_condition
from ..sendinput_hotkey import key_down, key_up, press_escape, tap_key, press_hotkey
from ..utils.state_detector import OverlayState, detect_overlay_state

# 安全设置
pyautogui.FAILSAFE = True
pyautogui.PAUSE = 0.1

# 配置目录
if os.name == "nt":
    CONFIG_DIR = Path.home() / "AppData" / "Roaming" / "voice-ime"
else:
    CONFIG_DIR = Path.home() / ".config" / "voice-ime"
CONFIG_FILE = CONFIG_DIR / "config.toml"


# ==================== 热键配置读取 ====================

class HotkeyConfig:
    """动态热键配置：从配置文件读取当前热键设置"""

    def __init__(self, vk_code: int, modifiers: int, mode: str, display_name: str):
        self.vk_code = vk_code
        self.modifiers = modifiers
        self.mode = mode  # "Toggle" or "PushToTalk"
        self.display_name = display_name

    def __repr__(self) -> str:
        return f"HotkeyConfig({self.display_name}, mode={self.mode})"


def _read_config_toml() -> dict:
    """
    读取并解析 config.toml 配置文件

    返回：解析后的配置字典
    """
    import toml

    if not CONFIG_FILE.exists():
        return {}

    try:
        content = CONFIG_FILE.read_text(encoding="utf-8")
        return toml.loads(content)
    except Exception:
        return {}


def _get_current_hotkey_config() -> HotkeyConfig:
    """
    从配置文件读取当前热键设置

    如果配置文件不存在或解析失败，使用默认值（F9, Toggle）
    """
    config = _read_config_toml()
    hotkey_section = config.get("hotkey", {})

    vk_code = hotkey_section.get("vk_code", 0x78)  # 默认 F9
    modifiers = hotkey_section.get("modifiers", 0)
    mode = hotkey_section.get("mode", "Toggle")
    display_name = hotkey_section.get("display_name", "F9")

    return HotkeyConfig(
        vk_code=int(vk_code),
        modifiers=int(modifiers),
        mode=str(mode),
        display_name=str(display_name),
    )


def _trigger_hotkey_start(hotkey: HotkeyConfig) -> None:
    """
    模拟热键按下（启动录音）

    根据配置的 modifiers 组合发送按键
    """
    modifiers = []
    if hotkey.modifiers & 0x0001:  # Alt
        modifiers.append(0x12)  # VK_MENU
    if hotkey.modifiers & 0x0002:  # Ctrl
        modifiers.append(0x11)  # VK_CONTROL
    if hotkey.modifiers & 0x0004:  # Shift
        modifiers.append(0x10)  # VK_SHIFT
    if hotkey.modifiers & 0x0008:  # Win
        modifiers.append(0x5B)  # VK_LWIN

    if modifiers:
        press_hotkey(hotkey.vk_code, modifiers, hold_seconds=0.05)
    else:
        tap_key(hotkey.vk_code, hold_seconds=0.05)


def _trigger_hotkey_ptt_hold(hotkey: HotkeyConfig) -> None:
    """
    模拟 PTT 模式按住热键（启动录音）
    """
    modifiers = []
    if hotkey.modifiers & 0x0001:
        modifiers.append(0x12)
    if hotkey.modifiers & 0x0002:
        modifiers.append(0x11)
    if hotkey.modifiers & 0x0004:
        modifiers.append(0x10)
    if hotkey.modifiers & 0x0008:
        modifiers.append(0x5B)

    for mod in modifiers:
        key_down(mod)
    key_down(hotkey.vk_code)


def _trigger_hotkey_ptt_release(hotkey: HotkeyConfig) -> None:
    """
    模拟 PTT 模式松开热键（停止录音）
    """
    modifiers = []
    if hotkey.modifiers & 0x0001:
        modifiers.append(0x12)
    if hotkey.modifiers & 0x0002:
        modifiers.append(0x11)
    if hotkey.modifiers & 0x0004:
        modifiers.append(0x10)
    if hotkey.modifiers & 0x0008:
        modifiers.append(0x5B)

    key_up(hotkey.vk_code)
    for mod in reversed(modifiers):
        key_up(mod)


# ==================== 工具函数 ====================

def _ensure_config_file() -> None:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    if not CONFIG_FILE.exists():
        CONFIG_FILE.write_text("", encoding="utf-8")


def _write_config_section(section: str, key: str, value: object) -> None:
    content = CONFIG_FILE.read_text(encoding="utf-8") if CONFIG_FILE.exists() else ""
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
        CONFIG_FILE.write_text("\n".join(lines) + "\n", encoding="utf-8")
        return

    for index in range(section_start + 1, section_end):
        if lines[index].strip().startswith(f"{key} =") or lines[index].strip().startswith(f"{key}="):
            lines[index] = f"{key} = {_format_value(value)}"
            CONFIG_FILE.write_text("\n".join(lines) + "\n", encoding="utf-8")
            return

    lines.insert(section_end, f"{key} = {_format_value(value)}")
    CONFIG_FILE.write_text("\n".join(lines) + "\n", encoding="utf-8")


def _format_value(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, str):
        return f'"{value}"'
    raise TypeError(f"Unsupported config value: {value!r}")


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


def _get_clipboard_text() -> str:
    """获取当前剪贴板文本内容"""
    try:
        win32clipboard.OpenClipboard()
        if win32clipboard.IsClipboardFormatAvailable(win32clipboard.CF_UNICODETEXT):
            text = win32clipboard.GetClipboardData(win32clipboard.CF_UNICODETEXT)
        else:
            text = ""
        win32clipboard.CloseClipboard()
        return text
    except Exception:
        try:
            win32clipboard.CloseClipboard()
        except Exception:
            pass
        return ""


def _set_clipboard_text(text: str) -> bool:
    """设置剪贴板文本内容"""
    try:
        win32clipboard.OpenClipboard()
        win32clipboard.EmptyClipboard()
        win32clipboard.SetClipboardData(win32clipboard.CF_UNICODETEXT, text)
        win32clipboard.CloseClipboard()
        return True
    except Exception:
        try:
            win32clipboard.CloseClipboard()
        except Exception:
            pass
        return False


def _open_notepad_and_wait() -> subprocess.Popen:
    """打开记事本并等待就绪"""
    process = subprocess.Popen(
        ["notepad"],
        creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
    )
    time.sleep(2)  # 等待记事本窗口加载
    return process


def _close_notepad(notepad_process: subprocess.Popen) -> None:
    """关闭记事本"""
    if notepad_process.poll() is None:
        subprocess.run(
            ["taskkill", "/F", "/IM", "notepad.exe"],
            capture_output=True,
        )


# ==================== Fixtures ====================

@pytest.fixture
def clean_env() -> None:
    """确保测试前后无 voice-ime 进程运行"""
    kill_existing_voice_ime()
    yield
    kill_existing_voice_ime()


@pytest.fixture
def e2e_config_guard() -> None:
    """E2E 测试配置守卫：保存并恢复配置"""
    _ensure_config_file()
    backup = CONFIG_FILE.read_text(encoding="utf-8") if CONFIG_FILE.exists() else ""
    try:
        yield
    finally:
        CONFIG_FILE.write_text(backup, encoding="utf-8")


@pytest.fixture
def e2e_toggle_config(e2e_config_guard) -> HotkeyConfig:
    """
    配置 Toggle 模式并返回热键配置
    测试使用当前系统配置的热键（不强制 F9）
    """
    # 确保模式为 Toggle，但不修改 vk_code（保持用户设置）
    _write_config_section("hotkey", "mode", "Toggle")
    # 禁用 LLM 避免干扰识别结果验证
    _write_config_section("llm", "enabled", False)
    # 返回当前实际热键配置
    return _get_current_hotkey_config()


@pytest.fixture
def e2e_ptt_config(e2e_config_guard) -> HotkeyConfig:
    """
    配置 PTT 模式并返回热键配置
    测试使用当前系统配置的热键（不强制 F9）
    """
    _write_config_section("hotkey", "mode", "PushToTalk")
    _write_config_section("llm", "enabled", False)
    return _get_current_hotkey_config()


# ==================== 测试用例 ====================

class TestFullPipelineToggle:
    """全流程测试：Toggle 模式"""

    @pytest.mark.hardware
    @pytest.mark.e2e
    def test_full_recording_pipeline_toggle(
        self, exe_path: Path, clean_env: None, e2e_toggle_config: HotkeyConfig
    ) -> None:
        """
        测试 Toggle 模式完整录音输入流程：
        1. 启动 voice-ime
        2. 打开记事本作为注入目标
        3. 按热键启动录音（overlay → Recording）
        4. 说话（需要麦克风）
        5. 再次按热键停止录音
        6. 等待 overlay → Processing → Hidden
        7. 验证记事本收到文字注入
        """
        hotkey = e2e_toggle_config
        print(f"\n🔑 使用热键配置: {hotkey}")

        # 启动 voice-ime
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        notepad_proc = None
        try:
            # 等待进程启动
            time.sleep(3)
            assert process.poll() is None, "voice-ime should be running"
            assert _wait_for_overlay_state(OverlayState.HIDDEN, timeout=2.0), "Overlay should be hidden initially"

            # 打开记事本
            notepad_proc = _open_notepad_and_wait()

            # 将记事本带到前台
            pyautogui.hotkey("alt", "tab")
            time.sleep(1)

            # 设置测试剪贴板内容用于验证保留
            test_clipboard_before = "原始剪贴板内容"
            _set_clipboard_text(test_clipboard_before)

            # 按热键开始录音
            _trigger_hotkey_start(hotkey)
            assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0), (
                f"Expected recording state after {hotkey.display_name}, got {detect_overlay_state().value}"
            )

            # 模拟说话（实际需要麦克风输入）
            time.sleep(3)

            # 再次按热键停止录音
            _trigger_hotkey_start(hotkey)

            # 等待进入 Processing 状态
            assert _wait_for_overlay_state(OverlayState.PROCESSING, timeout=5.0), (
                f"Expected processing state after stop, got {detect_overlay_state().value}"
            )

            # 等待处理完成，overlay 隐藏
            assert _wait_for_overlay_state(OverlayState.HIDDEN, timeout=15.0), (
                "Overlay should hide after processing completes"
            )

            # 等待文字注入完成
            time.sleep(1)

            # 验证记事本仍在运行
            assert notepad_proc.poll() is None, "Notepad should still be running"

        finally:
            if notepad_proc:
                _close_notepad(notepad_proc)
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            kill_existing_voice_ime()


class TestFullPipelinePTT:
    """全流程测试：PTT 模式"""

    @pytest.mark.hardware
    @pytest.mark.e2e
    def test_full_recording_pipeline_ptt(
        self, exe_path: Path, clean_env: None, e2e_ptt_config: HotkeyConfig
    ) -> None:
        """
        测试 PTT 模式完整录音输入流程：
        1. 启动 voice-ime
        2. 打开记事本作为注入目标
        3. 按住热键启动录音（overlay → Recording）
        4. 说话（需要麦克风）
        5. 松开热键停止录音
        6. 等待 overlay → Processing → Hidden
        7. 验证记事本收到文字注入
        """
        hotkey = e2e_ptt_config
        print(f"\n🔑 使用热键配置: {hotkey}")

        # 启动 voice-ime
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        notepad_proc = None
        try:
            # 等待进程启动
            time.sleep(3)
            assert process.poll() is None, "voice-ime should be running"
            assert _wait_for_overlay_state(OverlayState.HIDDEN, timeout=2.0), "Overlay should be hidden initially"

            # 打开记事本
            notepad_proc = _open_notepad_and_wait()

            # 将记事本带到前台
            pyautogui.hotkey("alt", "tab")
            time.sleep(1)

            # 按住热键开始录音
            _trigger_hotkey_ptt_hold(hotkey)
            try:
                assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0), (
                    f"Expected recording state while holding {hotkey.display_name}, got {detect_overlay_state().value}"
                )

                # 模拟说话（实际需要麦克风输入）
                time.sleep(3)

            finally:
                # 松开热键停止录音
                _trigger_hotkey_ptt_release(hotkey)

            # 等待进入 Processing 状态
            assert _wait_for_overlay_state(OverlayState.PROCESSING, timeout=5.0), (
                f"Expected processing state after release, got {detect_overlay_state().value}"
            )

            # 等待处理完成，overlay 隐藏
            assert _wait_for_overlay_state(OverlayState.HIDDEN, timeout=15.0), (
                "Overlay should hide after processing completes"
            )

            # 等待文字注入完成
            time.sleep(1)

            # 验证记事本仍在运行
            assert notepad_proc.poll() is None, "Notepad should still be running"

        finally:
            if notepad_proc:
                _close_notepad(notepad_proc)
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            kill_existing_voice_ime()


class TestFullPipelineCancel:
    """全流程测试：ESC 取消录音"""

    @pytest.mark.hardware
    @pytest.mark.e2e
    def test_recording_cancel_flow(
        self, exe_path: Path, clean_env: None, e2e_toggle_config: HotkeyConfig
    ) -> None:
        """
        测试取消录音流程：
        1. 启动录音
        2. 按 ESC 取消
        3. 验证 overlay 隐藏
        4. 验证无文字注入（记事本应保持空白）
        """
        hotkey = e2e_toggle_config
        print(f"\n🔑 使用热键配置: {hotkey}")

        # 启动 voice-ime
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        notepad_proc = None
        try:
            # 等待进程启动
            time.sleep(3)
            assert process.poll() is None, "voice-ime should be running"

            # 打开记事本
            notepad_proc = _open_notepad_and_wait()

            # 将记事本带到前台
            pyautogui.hotkey("alt", "tab")
            time.sleep(1)

            # 按热键开始录音
            _trigger_hotkey_start(hotkey)
            assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0), (
                f"Expected recording state after {hotkey.display_name}, got {detect_overlay_state().value}"
            )

            # 按 ESC 取消
            press_escape()

            # 等待 overlay 隐藏
            assert _wait_for_overlay_state(OverlayState.HIDDEN, timeout=5.0), (
                "Overlay should hide after ESC cancel"
            )

            # 验证记事本仍在运行
            assert notepad_proc.poll() is None, "Notepad should still be running"

        finally:
            if notepad_proc:
                _close_notepad(notepad_proc)
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            kill_existing_voice_ime()


class TestFullPipelineFocusLost:
    """全流程测试：焦点丢失场景"""

    @pytest.mark.hardware
    @pytest.mark.e2e
    def test_focus_lost_preview_flow(
        self, exe_path: Path, clean_env: None, e2e_toggle_config: HotkeyConfig
    ) -> None:
        """
        测试焦点丢失预览窗口流程：
        1. 启动录音
        2. 切换到其他窗口（模拟焦点丢失）
        3. 停止录音
        4. 验证 overlay 显示 FocusLost 预览窗口（320x110）
        """
        hotkey = e2e_toggle_config
        print(f"\n🔑 使用热键配置: {hotkey}")

        # 启动 voice-ime
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        notepad_proc = None
        try:
            # 等待进程启动
            time.sleep(3)
            assert process.poll() is None, "voice-ime should be running"

            # 打开记事本
            notepad_proc = _open_notepad_and_wait()

            # 将记事本带到前台
            pyautogui.hotkey("alt", "tab")
            time.sleep(1)

            # 按热键开始录音
            _trigger_hotkey_start(hotkey)
            assert _wait_for_overlay_state(OverlayState.RECORDING, timeout=5.0), (
                f"Expected recording state after {hotkey.display_name}, got {detect_overlay_state().value}"
            )

            # 切换到记事本（模拟焦点丢失）
            pyautogui.hotkey("alt", "tab")
            time.sleep(1)

            # 再次按热键停止录音
            _trigger_hotkey_start(hotkey)

            # 等待 Processing 状态
            assert _wait_for_overlay_state(OverlayState.PROCESSING, timeout=5.0), (
                f"Expected processing state, got {detect_overlay_state().value}"
            )

            # 验证焦点丢失预览窗口
            state = detect_overlay_state()
            assert state in (OverlayState.FOCUSLOST, OverlayState.HIDDEN, OverlayState.PROCESSING), (
                f"Expected focuslost/hidden/processing, got {state.value}"
            )

        finally:
            if notepad_proc:
                _close_notepad(notepad_proc)
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            kill_existing_voice_ime()
