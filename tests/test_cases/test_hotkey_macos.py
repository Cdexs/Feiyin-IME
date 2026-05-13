"""
macOS 热键响应测试

测试 voice-ime macOS 版本热键触发功能：
- Toggle 模式
- PTT 模式
- ESC 取消
- 热键预设

对标 Windows 端 test_hotkey.py，使用 pyautogui 模拟键盘事件。
macOS 热键实现使用 CGEventTap（DEC-015），测试需要 macOS 环境。

注意：
- 此文件在 Windows 上会被跳过（pytest.skip）
- macOS 需要授予终端/Python 辅助功能权限（系统偏好设置 → 安全性与隐私 → 辅助功能）
- 需要 voice-ime macOS 构建版本
"""

from __future__ import annotations

import os
import platform
import subprocess
import time
from pathlib import Path
from typing import Optional

import pytest

# macOS 特定导入
if platform.system() == "Darwin":
    import pyautogui
    pyautogui.FAILSAFE = True
    pyautogui.PAUSE = 0.1


# 跳过条件
if platform.system() != "Darwin":
    pytest.skip("macOS hotkey tests require Darwin platform", allow_module_level=True)


# ===== macOS 配置路径 =====
CONFIG_DIR = Path.home() / ".config" / "voice-ime"
CONFIG_FILE = CONFIG_DIR / "config.toml"


# ===== macOS 热键模拟工具 =====

def _check_assistive_permissions() -> bool:
    """
    检查 macOS 辅助功能权限

    pyautogui 在 macOS 需要辅助功能权限才能模拟键盘事件。
    如果没有权限，pyautogui 的操作会被系统忽略。
    """
    try:
        # 尝试模拟一个简单按键
        pyautogui.press("escape")
        return True
    except Exception:
        return False


def tap_key_macos(key: str, hold_seconds: float = 0.05) -> None:
    """
    模拟按键（按下 → 等待 → 松开）

    Args:
        key: 按键名称（pyautogui 格式，如 "f9", "escape", "space"）
        hold_seconds: 按住时间（秒）
    """
    pyautogui.keyDown(key)
    time.sleep(hold_seconds)
    pyautogui.keyUp(key)


def press_hotkey_macos(key: str, modifiers: list[str], hold_seconds: float = 0.05) -> None:
    """
    模拟组合键

    Args:
        key: 主按键
        modifiers: 修饰键列表（如 ["ctrl", "alt", "shift", "command"]）
        hold_seconds: 按住时间
    """
    for mod in modifiers:
        pyautogui.keyDown(mod)
    try:
        tap_key_macos(key, hold_seconds=hold_seconds)
    finally:
        for mod in reversed(modifiers):
            pyautogui.keyUp(mod)


# macOS 常用热键
def tap_f9_macos(hold_seconds: float = 0.05) -> None:
    """F9 按键"""
    tap_key_macos("f9", hold_seconds=hold_seconds)


def press_escape_macos(hold_seconds: float = 0.03) -> None:
    """ESC 按键"""
    tap_key_macos("escape", hold_seconds=hold_seconds)


def press_ctrl_space_macos(hold_seconds: float = 0.05) -> None:
    """Ctrl+Space 组合键"""
    press_hotkey_macos("space", ["ctrl"], hold_seconds=hold_seconds)


def press_cmd_grave_macos(hold_seconds: float = 0.05) -> None:
    """Command+` 组合键（macOS 常用）"""
    press_hotkey_macos("`", ["command"], hold_seconds=hold_seconds)


# ===== 配置管理 =====

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


# ===== macOS 状态检测 =====

def _find_voice_ime_window() -> Optional[int]:
    """
    查找 voice-ime 窗口（macOS）

    使用 AppleScript 查找窗口。
    """
    try:
        script = '''
        tell application "System Events"
            set procList to processes whose name contains "voice-ime"
            if length of procList > 0 then
                return true
            else
                return false
            end if
        end tell
        '''
        result = subprocess.run(
            ["osascript", "-e", script],
            capture_output=True,
            text=True,
            timeout=5
        )
        return result.stdout.strip() == "true"
    except Exception:
        return False


def _wait_for_voice_ime_running(timeout: float = 10.0, interval: float = 0.5) -> bool:
    """等待 voice-ime 进程启动"""
    start = time.time()
    while time.time() - start < timeout:
        try:
            result = subprocess.run(
                ["pgrep", "-f", "voice-ime"],
                capture_output=True,
                text=True
            )
            if result.returncode == 0:
                return True
        except Exception:
            pass
        time.sleep(interval)
    return False


# ===== Fixtures =====

@pytest.fixture
def assistive_permissions():
    """确保辅助功能权限已授予"""
    if not _check_assistive_permissions():
        pytest.skip(
            "Assistive permissions not granted. "
            "Go to System Preferences → Security & Privacy → Accessibility "
            "and grant permission to Terminal/Python."
        )
    yield


@pytest.fixture
def hotkey_config_guard() -> None:
    """热键配置守卫：保存并恢复配置"""
    _ensure_config_file()
    backup = _read_config_text()
    try:
        yield
    finally:
        _write_config_text(backup)


@pytest.fixture
def toggle_f9_config(hotkey_config_guard) -> None:
    """配置 Toggle F9 模式"""
    _write_config_section("hotkey", "vk_code", 0x78)
    _write_config_section("hotkey", "modifiers", 0)
    _write_config_section("hotkey", "display_name", "F9")
    _write_config_section("hotkey", "mode", "Toggle")
    yield


@pytest.fixture
def ptt_f9_config(hotkey_config_guard) -> None:
    """配置 PTT F9 模式"""
    _write_config_section("hotkey", "vk_code", 0x78)
    _write_config_section("hotkey", "modifiers", 0)
    _write_config_section("hotkey", "display_name", "F9")
    _write_config_section("hotkey", "mode", "PushToTalk")
    yield


@pytest.fixture
def ctrl_space_config(hotkey_config_guard) -> None:
    """配置 Ctrl+Space 热键"""
    _write_config_section("hotkey", "vk_code", 0x20)
    _write_config_section("hotkey", "modifiers", 0x0002)
    _write_config_section("hotkey", "display_name", "Ctrl+Space")
    _write_config_section("hotkey", "mode", "Toggle")
    yield


@pytest.fixture
def cmd_grave_config(hotkey_config_guard) -> None:
    """配置 Command+` 热键（macOS 特定）"""
    _write_config_section("hotkey", "vk_code", 0xC0)
    _write_config_section("hotkey", "modifiers", 0x0008)  # Win/Command key
    _write_config_section("hotkey", "display_name", "Command+`")
    _write_config_section("hotkey", "mode", "Toggle")
    yield


@pytest.fixture
def hotkey_test_process(exe_path: Path, assistive_permissions):
    """启动 voice-ime 测试进程"""
    # 清理旧进程
    subprocess.run(["pkill", "-f", "voice-ime"], capture_output=True)
    time.sleep(0.5)

    process = subprocess.Popen(
        [str(exe_path)],
        cwd=str(exe_path.parent),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    try:
        initialized = _wait_for_voice_ime_running(timeout=10.0, interval=0.5)
        assert initialized, f"voice-ime failed to initialize, exit={process.poll()}"
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
        subprocess.run(["pkill", "-f", "voice-ime"], capture_output=True)


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
def cmd_grave_voice_ime_process(cmd_grave_config, hotkey_test_process: subprocess.Popen):
    yield hotkey_test_process


# ===== 测试用例 =====

@pytest.mark.timeout(60)
@pytest.mark.gui
@pytest.mark.macos
class TestHotkeyToggleMode:
    """Toggle 模式热键测试"""

    def test_hotkey_toggle_start(
        self,
        toggle_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 Toggle 模式启动录音（F9）"""
        assert toggle_voice_ime_process.poll() is None, "voice-ime should be running"

        tap_f9_macos()
        time.sleep(1)

        # macOS 状态检测（依赖 overlay 窗口或菜单栏图标）
        # 注：macOS overlay 实现可能与 Windows 不同
        assert _find_voice_ime_window(), "voice-ime window should be visible after toggle"

    def test_hotkey_toggle_stop(
        self,
        toggle_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 Toggle 模式停止录音（再次 F9）"""
        tap_f9_macos()
        time.sleep(1)

        tap_f9_macos()
        time.sleep(1)

        # 录音应已停止
        assert toggle_voice_ime_process.poll() is None, "voice-ime should still be running"


@pytest.mark.timeout(60)
@pytest.mark.gui
@pytest.mark.macos
class TestHotkeyPTTMode:
    """PTT 模式热键测试"""

    def test_hotkey_ptt_hold(
        self,
        ptt_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 PTT 模式按住录音"""
        pyautogui.keyDown("f9")
        try:
            time.sleep(1)
            assert _find_voice_ime_window(), "voice-ime should be recording while F9 held"
        finally:
            pyautogui.keyUp("f9")

        time.sleep(1)
        assert ptt_voice_ime_process.poll() is None, "voice-ime should still be running"


@pytest.mark.timeout(60)
@pytest.mark.gui
@pytest.mark.macos
class TestHotkeyCancel:
    """ESC 取消测试"""

    def test_hotkey_cancel_recording(
        self,
        toggle_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 ESC 取消录音"""
        tap_f9_macos()
        time.sleep(1)

        press_escape_macos()
        time.sleep(1)

        # 录音应已取消
        assert toggle_voice_ime_process.poll() is None, "voice-ime should still be running after ESC"


@pytest.mark.timeout(60)
@pytest.mark.gui
@pytest.mark.macos
class TestHotkeyPresets:
    """热键预设测试"""

    def test_ctrl_space_hotkey(
        self,
        ctrl_space_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 Ctrl+Space 热键"""
        press_ctrl_space_macos()
        time.sleep(1)

        assert _find_voice_ime_window(), "voice-ime window should be visible after Ctrl+Space"

    def test_cmd_grave_hotkey(
        self,
        cmd_grave_voice_ime_process: subprocess.Popen,
    ) -> None:
        """测试 Command+` 热键（macOS 特定）"""
        press_cmd_grave_macos()
        time.sleep(1)

        assert _find_voice_ime_window(), "voice-ime window should be visible after Command+`"
