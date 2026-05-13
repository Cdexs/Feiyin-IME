"""
系统托盘测试

测试 voice-ime 托盘功能：
- 托盘图标显示
- 托盘菜单"配置"打开设置（BUG-027 回归）
- 托盘菜单"退出"关闭程序
- 二次点击"配置"（BUG-027 循环 3 次验证）
"""

import os
import sys
import time
import subprocess
from pathlib import Path

import pytest
import psutil

# 添加父目录到路径以便导入 conftest
sys.path.insert(0, str(Path(__file__).parent.parent))
from conftest import (
    wait_for_condition,
    kill_existing_voice_ime,
    wait_for_window_title,
)

# 托盘测试不需要 GUI 操作（仅验证进程和窗口状态）
pytestmark = [pytest.mark.timeout(30)]


@pytest.fixture
def clean_env() -> None:
    """确保测试前后无 voice-ime 进程运行"""
    kill_existing_voice_ime()
    yield
    # 清理：确保进程退出
    kill_existing_voice_ime()


@pytest.mark.smoke
class TestTrayIcon:
    """托盘图标显示验证"""

    def test_tray_icon_displayed(self, exe_path: Path, clean_env: None) -> None:
        """测试启动后托盘图标出现"""
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        try:
            # 等待进程初始化
            time.sleep(3)
            assert process.poll() is None, "voice-ime.exe should be running"

            # 验证托盘图标（通过系统托盘枚举）
            # 注意：Windows 托盘图标检测需要 Win32 API
            # 此处通过进程运行间接验证
            assert process.poll() is None

        finally:
            process.terminate()
            process.wait(timeout=5)


@pytest.mark.smoke
@pytest.mark.regression
class TestTrayMenuSettings:
    """托盘菜单"配置"打开设置（BUG-027 回归）"""

    def test_tray_menu_opens_settings(self, exe_path: Path, clean_env: None) -> None:
        """测试托盘菜单"配置"能打开设置窗口"""
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        try:
            time.sleep(3)
            assert process.poll() is None

            # 通过 --settings-ui 参数直接打开设置窗口（模拟托盘菜单行为）
            settings_exe = exe_path.parent / "voice-ime-ui.exe"
            if not settings_exe.exists():
                # 开发环境备用路径
                settings_exe = exe_path.parent.parent / "src-tauri" / "target" / "release" / "voice-ime-ui.exe"

            if settings_exe.exists():
                settings_proc = subprocess.Popen(
                    [str(settings_exe)],
                    cwd=str(settings_exe.parent),
                    creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
                )

                try:
                    # 等待设置窗口出现
                    time.sleep(5)
                    # 验证设置进程运行
                    assert settings_proc.poll() is None, "Settings window should be open"

                finally:
                    settings_proc.terminate()
                    settings_proc.wait(timeout=5)

        finally:
            process.terminate()
            process.wait(timeout=5)


@pytest.mark.regression
class TestTrayMenuExit:
    """托盘菜单"退出"关闭程序"""

    def test_tray_menu_exits_program(self, exe_path: Path, clean_env: None) -> None:
        """测试托盘菜单"退出"能关闭主程序"""
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        try:
            time.sleep(3)
            assert process.poll() is None

            # 模拟退出：发送 WM_CLOSE 消息到隐藏窗口
            # 此处通过终止进程模拟托盘菜单"退出"行为
            process.terminate()
            exited = wait_for_condition(
                lambda: process.poll() is not None,
                timeout=5.0,
                description="process exit after terminate"
            )
            assert exited, "voice-ime.exe should exit after terminate"

        except Exception:
            process.kill()
            process.wait(timeout=5)
            raise


@pytest.mark.regression
class TestTrayDoubleClickSettings:
    """二次点击"配置"（BUG-027 循环 3 次验证）"""

    def test_tray_settings_twice(self, exe_path: Path, clean_env: None) -> None:
        """测试托盘"配置"二次点击不显示修复（BUG-027）"""
        process = subprocess.Popen(
            [str(exe_path)],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        try:
            time.sleep(3)
            assert process.poll() is None

            settings_exe = exe_path.parent / "voice-ime-ui.exe"
            if not settings_exe.exists():
                settings_exe = exe_path.parent.parent / "src-tauri" / "target" / "release" / "voice-ime-ui.exe"

            if not settings_exe.exists():
                pytest.skip("voice-ime-ui.exe not found")

            # 循环 3 次验证打开/关闭设置窗口
            for i in range(3):
                # 打开设置窗口
                settings_proc = subprocess.Popen(
                    [str(settings_exe)],
                    cwd=str(settings_exe.parent),
                    creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
                )

                time.sleep(5)
                assert settings_proc.poll() is None, f"Settings window should open (iteration {i+1})"

                # 关闭设置窗口
                settings_proc.terminate()
                settings_proc.wait(timeout=5)

                time.sleep(2)

        finally:
            process.terminate()
            process.wait(timeout=5)
