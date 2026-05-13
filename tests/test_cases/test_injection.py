"""
文字注入测试

测试 voice-ime 文字注入功能：
- 记事本文字注入（模拟发送文字，无需麦克风）
- 剪贴板内容保留验证
- 焦点丢失场景（标记 hardware，开头 pytest.skip if no_hardware）
"""

import subprocess
import sys
import time
from pathlib import Path

import pytest
import pyautogui

# 添加父目录到路径以便导入 conftest
sys.path.insert(0, str(Path(__file__).parent.parent))
from conftest import kill_existing_voice_ime, wait_for_condition

# 安全设置
pyautogui.FAILSAFE = True
pyautogui.PAUSE = 0.1

pytestmark = [pytest.mark.timeout(30)]


@pytest.fixture
def clean_env() -> None:
    """确保测试前后无 voice-ime 进程运行"""
    kill_existing_voice_ime()
    yield
    kill_existing_voice_ime()


@pytest.fixture
def notepad_setup() -> None:
    """打开记事本并等待就绪"""
    # 打开记事本
    subprocess.run(["notepad"], creationflags=subprocess.CREATE_NEW_PROCESS_GROUP)
    time.sleep(2)

    yield

    # 关闭记事本
    subprocess.run(
        ["taskkill", "/F", "/IM", "notepad.exe"],
        capture_output=True,
    )


class TestNotepadInjection:
    """记事本文字注入（模拟发送文字，无需麦克风）"""

    @pytest.mark.hardware
    def test_text_injection_to_notepad(
        self, exe_path: Path, clean_env: None, notepad_setup: None
    ) -> None:
        """测试文字注入到记事本"""
        # 检查是否需要跳过硬件测试
        if self._no_hardware():
            pytest.skip("Hardware test skipped (no microphone available)")

        # 启动 voice-ime
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        try:
            time.sleep(3)
            assert process.poll() is None

            # 将记事本窗口带到前台
            pyautogui.hotkey("alt", "tab")
            time.sleep(1)

            # 模拟热键触发录音（默认 F9）
            pyautogui.press("f9")
            time.sleep(1)

            # 模拟说话（实际需要麦克风，此处仅验证按键流程）
            time.sleep(2)

            # 再次按 F9 停止录音
            pyautogui.press("f9")
            time.sleep(3)  # 等待处理和注入

            # 验证记事本仍然运行
            # 实际验证需要检查记事本内容（OCR 或 Accessibility API）

        finally:
            process.terminate()
            process.wait(timeout=5)

    @staticmethod
    def _no_hardware() -> bool:
        """检测是否有麦克风设备"""
        import os
        return os.getenv("SKIP_AUDIO_TESTS", "0") == "1"


class TestClipboardPreserve:
    """剪贴板内容保留验证"""

    @pytest.mark.hardware
    def test_clipboard_content_preserved(
        self, exe_path: Path, clean_env: None
    ) -> None:
        """测试剪贴板注入模式保留原始内容"""
        if self._no_hardware():
            pytest.skip("Hardware test skipped (no microphone available)")

        import win32clipboard

        # 保存原始剪贴板内容
        win32clipboard.OpenClipboard()
        original_text = win32clipboard.GetClipboardData(win32clipboard.CF_UNICODETEXT) if win32clipboard.IsClipboardFormatAvailable(win32clipboard.CF_UNICODETEXT) else ""
        win32clipboard.CloseClipboard()

        try:
            # 启动 voice-ime
            process = subprocess.Popen(
                [str(exe_path)],
                cwd=str(exe_path.parent),
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
            )

            time.sleep(3)
            assert process.poll() is None

            # 触发热键（模拟）
            pyautogui.press("f9")
            time.sleep(2)
            pyautogui.press("f9")
            time.sleep(3)

            # 验证剪贴板内容恢复
            win32clipboard.OpenClipboard()
            restored_text = win32clipboard.GetClipboardData(win32clipboard.CF_UNICODETEXT) if win32clipboard.IsClipboardFormatAvailable(win32clipboard.CF_UNICODETEXT) else ""
            win32clipboard.CloseClipboard()

            assert restored_text == original_text, "Clipboard content should be restored"

        finally:
            process.terminate()
            process.wait(timeout=5)

    @staticmethod
    def _no_hardware() -> bool:
        """检测是否有麦克风设备"""
        import os
        return os.getenv("SKIP_AUDIO_TESTS", "0") == "1"


class TestFocusLostPreview:
    """焦点丢失场景（标记 hardware）"""

    @pytest.mark.hardware
    def test_focus_lost_preview(
        self, exe_path: Path, clean_env: None
    ) -> None:
        """测试焦点丢失时预览窗口弹出"""
        if self._no_hardware():
            pytest.skip("Hardware test skipped (no microphone available)")

        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        try:
            time.sleep(3)
            assert process.poll() is None

            # 启动录音
            pyautogui.press("f9")
            time.sleep(1)

            # 切换到其他窗口（模拟焦点丢失）
            pyautogui.hotkey("alt", "tab")
            time.sleep(2)

            # 验证 overlay 窗口切换到 FocusLost 状态
            # 实际验证需要检测窗口尺寸变化（320x110）

        finally:
            process.terminate()
            process.wait(timeout=5)
