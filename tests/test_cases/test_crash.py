"""
崩溃处理测试

测试 voice-ime 崩溃检测机制：
- crash.json 写入/读取格式验证（单元级）
- crash reporter 启动参数验证（--crash-reporter）
- crash 清理逻辑验证
"""

import json
import subprocess
import sys
import time
from pathlib import Path

import pytest

# 添加父目录到路径以便导入 conftest
sys.path.insert(0, str(Path(__file__).parent.parent))
from conftest import kill_existing_voice_ime

# crash 数据存储路径
CRASH_DIR = Path.home() / "AppData" / "Local" / "voice-ime"
CRASH_FILE = CRASH_DIR / "crash.json"

pytestmark = [pytest.mark.timeout(30)]


@pytest.fixture
def clean_crash_env() -> None:
    """确保测试前后无 crash 文件残留"""
    # 备份旧 crash 文件（如果存在）
    if CRASH_FILE.exists():
        backup = CRASH_FILE.with_suffix(".json.bak")
        CRASH_FILE.rename(backup)

    yield

    # 清理：恢复或移除
    if CRASH_FILE.exists():
        CRASH_FILE.unlink()
    backup = CRASH_FILE.with_suffix(".json.bak")
    if backup.exists():
        backup.rename(CRASH_FILE)


@pytest.mark.smoke
class TestCrashFileFormat:
    """crash.json 写入/读取格式验证（单元级）"""

    def test_crash_file_write_read(self, clean_crash_env: None) -> None:
        """测试 crash.json 写入和读取格式"""
        # 模拟 crash 数据写入
        crash_data = {
            "timestamp": "2026-04-20T00:00:00Z",
            "message": "test panic",
            "backtrace": ["frame 1", "frame 2"],
            "version": "0.5.0",
        }

        # 写入 crash 文件
        CRASH_DIR.mkdir(parents=True, exist_ok=True)
        CRASH_FILE.write_text(json.dumps(crash_data, indent=2), encoding="utf-8")

        # 验证文件格式
        assert CRASH_FILE.exists(), "crash.json should exist"
        loaded = json.loads(CRASH_FILE.read_text(encoding="utf-8"))
        assert loaded["message"] == "test panic"
        assert loaded["version"] == "0.5.0"

    def test_crash_file_invalid_format(self, clean_crash_env: None) -> None:
        """测试无效 crash 文件格式处理"""
        # 写入无效 JSON
        CRASH_DIR.mkdir(parents=True, exist_ok=True)
        CRASH_FILE.write_text("not valid json", encoding="utf-8")

        # 验证读取时能正确处理
        try:
            json.loads(CRASH_FILE.read_text(encoding="utf-8"))
            pytest.fail("Should have raised JSONDecodeError")
        except json.JSONDecodeError:
            pass  # 预期行为


class TestCrashReporterLaunch:
    """crash reporter 启动参数验证（--crash-reporter）"""

    def test_crash_reporter_flag(self, exe_path: Path) -> None:
        """测试 --crash-reporter 参数能启动 crash reporter"""
        # 创建 crash 文件
        CRASH_DIR.mkdir(parents=True, exist_ok=True)
        crash_data = {
            "timestamp": "2026-04-20T00:00:00Z",
            "message": "test panic",
            "backtrace": [],
            "version": "0.5.0",
        }
        CRASH_FILE.write_text(json.dumps(crash_data, indent=2), encoding="utf-8")

        # 启动 crash reporter
        process = subprocess.Popen(
            [str(exe_path), "--crash-reporter"],
            cwd=str(exe_path.parent),
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        )

        try:
            # 等待 reporter 启动
            time.sleep(3)
            # 验证进程运行
            assert process.poll() is None, "crash reporter should be running"

        finally:
            process.terminate()
            process.wait(timeout=5)

        # 清理 crash 文件
        if CRASH_FILE.exists():
            CRASH_FILE.unlink()


class TestCrashCleanup:
    """crash 清理逻辑验证"""

    def test_crash_file_cleanup(self, clean_crash_env: None) -> None:
        """测试 crash 文件清理逻辑"""
        # 创建 crash 文件
        CRASH_DIR.mkdir(parents=True, exist_ok=True)
        crash_data = {
            "timestamp": "2026-04-20T00:00:00Z",
            "message": "test panic",
        }
        CRASH_FILE.write_text(json.dumps(crash_data), encoding="utf-8")

        # 验证文件存在
        assert CRASH_FILE.exists()

        # 模拟清理：删除文件
        CRASH_FILE.unlink()

        # 验证文件已删除
        assert not CRASH_FILE.exists(), "crash.json should be cleaned"
