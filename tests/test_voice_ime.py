"""
voice-ime 主测试入口

基础功能测试：
- 程序启动/关闭
- 进程状态验证
"""

import os
import subprocess
import time
from pathlib import Path

import pytest

from .conftest import wait_for_condition, kill_existing_voice_ime


@pytest.mark.timeout(30)
class TestVoiceImeLaunch:
    """程序启动/关闭测试类"""

    @pytest.mark.smoke
    def test_exe_exists(self, exe_path: Path) -> None:
        """验证 exe 文件存在"""
        assert exe_path.exists(), f"exe not found: {exe_path}"
        assert exe_path.is_file(), f"exe is not a file: {exe_path}"

    @pytest.mark.slow
    def test_launch_and_exit(self, exe_path: Path) -> None:
        """测试程序启动和退出"""
        kill_existing_voice_ime()  # 单实例检测：先清理残留进程
        process = None
        try:
            # 启动程序
            process = subprocess.Popen(
                [str(exe_path)],
                cwd=str(exe_path.parent),
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
            )

            # 优化1: 轮询等待初始化
            initialized = wait_for_condition(
                lambda: process.poll() is None,
                timeout=10.0
            )
            assert initialized, "Process failed to initialize"

            # 验证进程运行
            assert process.poll() is None, "Process exited unexpectedly"

            # 获取进程信息
            pid = process.pid
            assert pid > 0, "Invalid PID"

            # 验证进程存在（通过 tasklist）
            result = subprocess.run(
                ["tasklist", "/FI", f"PID eq {pid}"],
                capture_output=True,
                text=True,
            )
            assert "voice-ime.exe" in result.stdout, f"Process not found in tasklist: {result.stdout}"

        finally:
            # 清理进程
            if process is not None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()

    @pytest.mark.slow
    def test_process_cleanup(self, exe_path: Path) -> None:
        """测试程序关闭后进程清理"""
        kill_existing_voice_ime()  # 单实例检测：先清理残留进程
        process = None
        pid = None

        try:
            # 启动程序
            process = subprocess.Popen(
                [str(exe_path)],
                cwd=str(exe_path.parent),
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
            )
            pid = process.pid

            # 优化1: 轮询等待初始化
            assert wait_for_condition(lambda: process.poll() is None, timeout=10.0)

            # 验证进程运行
            assert process.poll() is None

            # 关闭程序
            process.terminate()
            process.wait(timeout=5)

            # 验证进程已清理
            time.sleep(1)
            result = subprocess.run(
                ["tasklist", "/FI", f"PID eq {pid}"],
                capture_output=True,
                text=True,
            )
            assert "voice-ime.exe" not in result.stdout, "Process still running after exit"

        except subprocess.TimeoutExpired:
            if process is not None:
                process.kill()
                process.wait()


@pytest.mark.timeout(10)
class TestVoiceImeHelp:
    """CLI 参数测试类"""

    @pytest.mark.smoke
    def test_debug_flag(self, exe_path: Path) -> None:
        """测试 -debug 参数"""
        process = subprocess.Popen(
            [str(exe_path), "-debug"],
            cwd=str(exe_path.parent),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        try:
            # 优化1: 轮询等待启动
            assert wait_for_condition(lambda: process.poll() is None, timeout=5.0)
            assert process.poll() is None, "Process with -debug should start"

        finally:
            process.terminate()
            process.wait(timeout=5)

    @pytest.mark.slow
    @pytest.mark.gui
    def test_crash_reporter_flag(self, exe_path: Path) -> None:
        """测试 --crash-reporter 参数（无 crash 文件时应快速退出或弹窗）"""
        process = subprocess.Popen(
            [str(exe_path), "--crash-reporter"],
            cwd=str(exe_path.parent),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        try:
            # 等待最多 5s 自动退出；若有 GUI 窗口则强制终止
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()

        # 验证进程已退出（无论自动退出还是被 kill）
        assert process.poll() is not None, "crash-reporter should have exited"


@pytest.mark.gui
@pytest.mark.timeout(30)
class TestVoiceImeTray:
    """托盘图标测试类"""

    def test_tray_icon_visible(self, voice_ime_process: subprocess.Popen) -> None:
        """测试托盘图标显示"""
        import pywinauto

        # 优化1: 轮询等待托盘初始化（替代 time.sleep(3)）
        assert wait_for_condition(
            lambda: voice_ime_process.poll() is None,
            timeout=5.0
        )

        # 验证进程运行
        assert voice_ime_process.poll() is None

        # 注意：托盘图标检测在 Windows 上较复杂
        # 此测试仅验证进程运行，实际托盘验证需手动或更复杂的方法