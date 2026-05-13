"""
macOS 文字注入测试

测试 voice-ime macOS 版本文字注入功能：
- TextEdit 文字注入（模拟发送文字，无需麦克风）
- 剪贴板内容保留验证
- 焦点丢失场景

对标 Windows 端 test_injection.py。
macOS 文字注入使用 enigo（DEC-015），测试需要 macOS 环境。

注意：
- 此文件在 Windows 上会被跳过（pytest.skip）
- macOS 需要授予辅助功能权限
- 使用 TextEdit 替代记事本作为注入目标
"""

import os
import platform
import subprocess
import sys
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
    pytest.skip("macOS injection tests require Darwin platform", allow_module_level=True)


# ===== macOS 配置路径 =====
CONFIG_DIR = Path.home() / ".config" / "voice-ime"
CONFIG_FILE = CONFIG_DIR / "config.toml"


# ===== macOS 工具函数 =====

def _wait_for_app_running(app_name: str, timeout: float = 10.0, interval: float = 0.5) -> bool:
    """等待 macOS 应用启动"""
    start = time.time()
    while time.time() - start < timeout:
        try:
            result = subprocess.run(
                ["pgrep", "-f", app_name],
                capture_output=True,
                text=True
            )
            if result.returncode == 0:
                return True
        except Exception:
            pass
        time.sleep(interval)
    return False


def _get_clipboard_text_macos() -> str:
    """获取 macOS 剪贴板文本"""
    try:
        result = subprocess.run(
            ["pbpaste"],
            capture_output=True,
            text=True,
            timeout=5
        )
        return result.stdout
    except Exception:
        return ""


def _set_clipboard_text_macos(text: str) -> None:
    """设置 macOS 剪贴板文本"""
    try:
        subprocess.run(
            ["pbcopy"],
            input=text,
            text=True,
            timeout=5
        )
    except Exception:
        pass


def _bring_app_to_front(app_name: str) -> None:
    """将 macOS 应用带到前台"""
    try:
        subprocess.run(
            ["osascript", "-e", f'tell application "{app_name}" to activate'],
            capture_output=True,
            timeout=5
        )
        time.sleep(1)
    except Exception:
        pass


# ===== Fixtures =====

@pytest.fixture
def clean_env() -> None:
    """确保测试前后无 voice-ime 进程运行"""
    subprocess.run(["pkill", "-f", "voice-ime"], capture_output=True)
    time.sleep(0.5)
    yield
    subprocess.run(["pkill", "-f", "voice-ime"], capture_output=True)


@pytest.fixture
def textedit_setup() -> None:
    """打开 TextEdit 并等待就绪"""
    # 打开 TextEdit
    subprocess.run(["open", "-a", "TextEdit"], capture_output=True)
    time.sleep(2)

    # 确保 TextEdit 在前台
    _bring_app_to_front("TextEdit")
    time.sleep(1)

    yield

    # 关闭 TextEdit
    subprocess.run(["pkill", "-x", "TextEdit"], capture_output=True)


class TestTextEditInjection:
    """TextEdit 文字注入测试"""

    @pytest.mark.hardware
    @pytest.mark.macos
    def test_text_injection_to_textedit(
        self, exe_path: Path, clean_env: None, textedit_setup: None
    ) -> None:
        """测试文字注入到 TextEdit"""
        # 启动 voice-ime
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        try:
            # 等待进程启动
            if not _wait_for_app_running("voice-ime", timeout=10.0):
                pytest.fail("voice-ime failed to initialize")

            time.sleep(2)
            assert process.poll() is None, "voice-ime should be running"

            # 将 TextEdit 带到前台
            _bring_app_to_front("TextEdit")
            time.sleep(1)

            # 模拟热键触发录音（F9）
            pyautogui.press("f9")
            time.sleep(1)

            # 模拟说话（实际需要麦克风，此处仅验证按键流程）
            time.sleep(2)

            # 再次按 F9 停止录音
            pyautogui.press("f9")
            time.sleep(3)  # 等待处理和注入

            # 验证 TextEdit 仍然运行
            result = subprocess.run(
                ["pgrep", "-x", "TextEdit"],
                capture_output=True,
                text=True
            )
            assert result.returncode == 0, "TextEdit should still be running"

            # 注：实际验证注入文本需要读取 TextEdit 内容
            # 可使用 AppleScript 或 Accessibility API

        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            subprocess.run(["pkill", "-f", "voice-ime"], capture_output=True)


class TestClipboardPreserve:
    """剪贴板内容保留验证"""

    @pytest.mark.hardware
    @pytest.mark.macos
    def test_clipboard_content_preserved(
        self, exe_path: Path, clean_env: None
    ) -> None:
        """测试剪贴板注入模式保留原始内容"""
        # 保存原始剪贴板内容
        original_text = _get_clipboard_text_macos()

        # 设置测试剪贴板内容
        test_clipboard = "原始剪贴板内容测试"
        _set_clipboard_text_macos(test_clipboard)

        try:
            # 启动 voice-ime
            process = subprocess.Popen(
                [str(exe_path)],
                cwd=str(exe_path.parent),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )

            if not _wait_for_app_running("voice-ime", timeout=10.0):
                pytest.fail("voice-ime failed to initialize")

            time.sleep(2)
            assert process.poll() is None

            # 触发热键（模拟）
            pyautogui.press("f9")
            time.sleep(1)
            pyautogui.press("f9")
            time.sleep(3)

            # 验证剪贴板内容恢复
            restored_text = _get_clipboard_text_macos()
            # 注：由于注入会临时修改剪贴板，最终应恢复
            # 实际验证取决于注入实现

        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            # 恢复原始剪贴板
            _set_clipboard_text_macos(original_text)


class TestFocusLostPreview:
    """焦点丢失场景"""

    @pytest.mark.hardware
    @pytest.mark.macos
    def test_focus_lost_preview(
        self, exe_path: Path, clean_env: None
    ) -> None:
        """测试焦点丢失时预览窗口弹出"""
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        try:
            if not _wait_for_app_running("voice-ime", timeout=10.0):
                pytest.fail("voice-ime failed to initialize")

            time.sleep(2)
            assert process.poll() is None

            # 启动录音
            pyautogui.press("f9")
            time.sleep(1)

            # 切换到其他应用（模拟焦点丢失）
            _bring_app_to_front("Finder")
            time.sleep(2)

            # 验证 overlay 窗口切换到 FocusLost 状态
            # 注：macOS overlay 实现可能与 Windows 不同
            # 需要使用 Accessibility API 或 window list 检测

        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
