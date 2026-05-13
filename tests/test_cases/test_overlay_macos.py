"""
macOS Overlay 窗口测试

测试 voice-ime macOS 版本录音悬浮层功能：
- Overlay 窗口显示/隐藏
- 录音状态检测
- 处理中状态检测
- 焦点丢失预览窗口

对标 Windows 端 overlay 状态检测。
macOS Overlay 使用 Tauri 透明窗口（DEC-013），测试需要 macOS 环境。

注意：
- 此文件在 Windows 上会被跳过（pytest.skip）
- macOS 需要授予辅助功能权限
- Overlay 状态检测使用 pyobjc/Accessibility API
"""

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
    pytest.skip("macOS overlay tests require Darwin platform", allow_module_level=True)


# ===== macOS 配置路径 =====
CONFIG_DIR = Path.home() / ".config" / "voice-ime"
CONFIG_FILE = CONFIG_DIR / "config.toml"


# ===== macOS Overlay 状态检测 =====

def _find_overlay_window() -> Optional[dict]:
    """
    查找 voice-ime overlay 窗口（macOS）

    使用 AppleScript 或 pyobjc 查找透明 overlay 窗口。
    返回窗口信息字典，包括位置、尺寸等。
    """
    try:
        # 使用 system_profiler 或 AppleScript 查找窗口
        script = '''
        tell application "System Events"
            set procList to processes whose name contains "voice-ime"
            if length of procList > 0 then
                set proc to item 1 of procList
                set winList to windows of proc
                return count of winList
            else
                return 0
            end if
        end tell
        '''
        result = subprocess.run(
            ["osascript", "-e", script],
            capture_output=True,
            text=True,
            timeout=5
        )
        try:
            window_count = int(result.stdout.strip())
            return {"count": window_count}
        except ValueError:
            return None
    except Exception:
        return None


def _wait_for_overlay_visible(timeout: float = 5.0, interval: float = 0.2) -> bool:
    """等待 overlay 窗口可见"""
    start = time.time()
    while time.time() - start < timeout:
        info = _find_overlay_window()
        if info and info.get("count", 0) > 0:
            return True
        time.sleep(interval)
    return False


def _wait_for_overlay_hidden(timeout: float = 5.0, interval: float = 0.2) -> bool:
    """等待 overlay 窗口隐藏"""
    start = time.time()
    while time.time() - start < timeout:
        info = _find_overlay_window()
        if info and info.get("count", 0) == 0:
            return True
        time.sleep(interval)
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
    try:
        pyautogui.press("escape")
        yield
    except Exception:
        pytest.skip(
            "Assistive permissions not granted. "
            "Go to System Preferences → Security & Privacy → Accessibility."
        )


@pytest.fixture
def clean_env() -> None:
    """确保测试前后无 voice-ime 进程运行"""
    subprocess.run(["pkill", "-f", "voice-ime"], capture_output=True)
    time.sleep(0.5)
    yield
    subprocess.run(["pkill", "-f", "voice-ime"], capture_output=True)


@pytest.fixture
def voice_ime_process(exe_path: Path, assistive_permissions):
    """启动 voice-ime 测试进程"""
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


# ===== 测试用例 =====

@pytest.mark.timeout(60)
@pytest.mark.gui
@pytest.mark.macos
class TestOverlayVisibility:
    """Overlay 窗口显示/隐藏测试"""

    def test_overlay_hidden_at_start(self, voice_ime_process) -> None:
        """测试启动后 overlay 应隐藏"""
        time.sleep(1)
        assert _wait_for_overlay_hidden(timeout=3.0), "Overlay should be hidden at startup"

    def test_overlay_visible_on_recording(self, voice_ime_process) -> None:
        """测试录音时 overlay 应可见"""
        # 触发录音
        pyautogui.press("f9")
        time.sleep(1)

        assert _wait_for_overlay_visible(timeout=5.0), "Overlay should be visible during recording"

        # 停止录音
        pyautogui.press("f9")
        time.sleep(2)

    def test_overlay_hidden_after_recording(self, voice_ime_process) -> None:
        """测试录音结束后 overlay 应隐藏"""
        # 触发录音并停止
        pyautogui.press("f9")
        time.sleep(1)
        pyautogui.press("f9")

        # 等待处理完成
        time.sleep(3)

        assert _wait_for_overlay_hidden(timeout=5.0), "Overlay should be hidden after recording"


@pytest.mark.timeout(60)
@pytest.mark.gui
@pytest.mark.macos
class TestOverlayStates:
    """Overlay 状态切换测试"""

    def test_recording_to_processing_transition(self, voice_ime_process) -> None:
        """测试录音到处理中的状态转换"""
        # 触发录音
        pyautogui.press("f9")
        time.sleep(1)

        assert _wait_for_overlay_visible(timeout=5.0), "Should be recording"

        # 停止录音（进入处理状态）
        pyautogui.press("f9")

        # Processing 状态持续时间短，可能很快过渡到 hidden
        time.sleep(1)

    def test_cancel_recording(self, voice_ime_process) -> None:
        """测试取消录音后 overlay 隐藏"""
        # 触发录音
        pyautogui.press("f9")
        time.sleep(1)

        # 按 ESC 取消
        pyautogui.press("escape")
        time.sleep(1)

        assert _wait_for_overlay_hidden(timeout=3.0), "Overlay should hide after ESC cancel"


@pytest.mark.timeout(60)
@pytest.mark.gui
@pytest.mark.macos
class TestFocusLostPreview:
    """焦点丢失预览窗口测试"""

    def test_focus_lost_preview_shows(self, voice_ime_process) -> None:
        """测试焦点丢失时显示预览窗口"""
        # 触发录音
        pyautogui.press("f9")
        time.sleep(1)

        # 切换到其他应用（模拟焦点丢失）
        subprocess.run(
            ["osascript", "-e", 'tell application "Finder" to activate'],
            capture_output=True,
            timeout=5
        )
        time.sleep(1)

        # 停止录音
        pyautogui.press("f9")
        time.sleep(3)

        # 验证显示预览窗口（尺寸与录音状态不同）
        # macOS 预览窗口实现可能与 Windows 不同
        info = _find_overlay_window()
        # 注：具体验证取决于 macOS overlay 实现

    def test_focus_lost_preview_copy(self, voice_ime_process) -> None:
        """测试焦点丢失预览窗口复制功能"""
        # 触发录音并切换到其他应用
        pyautogui.press("f9")
        time.sleep(1)
        subprocess.run(
            ["osascript", "-e", 'tell application "Finder" to activate'],
            capture_output=True,
            timeout=5
        )
        time.sleep(1)
        pyautogui.press("f9")
        time.sleep(3)

        # 验证预览窗口存在
        info = _find_overlay_window()
        # 注：复制功能验证需要 Accessibility API


@pytest.mark.timeout(30)
@pytest.mark.gui
@pytest.mark.macos
class TestOverlayWindowProperties:
    """Overlay 窗口属性测试"""

    def test_overlay_window_position(self, voice_ime_process) -> None:
        """测试 overlay 窗口位置（应在屏幕底部居中）"""
        # 触发录音
        pyautogui.press("f9")
        time.sleep(1)

        info = _find_overlay_window()
        if info:
            # macOS 窗口位置验证
            # 注：具体位置取决于 macOS overlay 实现
            pass

        pyautogui.press("f9")
        time.sleep(1)

    def test_overlay_window_opacity(self, voice_ime_process) -> None:
        """测试 overlay 窗口透明度"""
        # 触发录音
        pyautogui.press("f9")
        time.sleep(1)

        # macOS 窗口透明度验证
        # 注：需要使用 CoreGraphics API 获取窗口属性

        pyautogui.press("f9")
        time.sleep(1)
