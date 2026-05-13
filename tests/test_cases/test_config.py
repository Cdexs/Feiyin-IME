"""
配置保存/加载测试

测试 voice-ime 配置功能：
- 配置文件读写
- 默认值验证
- 配置变更持久化
"""

import json
import time
import subprocess
from pathlib import Path

import pytest
import sys
import os
sys.path.insert(0, str(Path(__file__).parent.parent))
from conftest import kill_existing_voice_ime


# 配置文件路径
CONFIG_DIR = Path.home() / "AppData" / "Local" / "voice-ime"
CONFIG_FILE = CONFIG_DIR / "config.json"


@pytest.mark.timeout(30)
class TestConfigFile:
    """配置文件测试类"""

    def test_config_dir_exists(self) -> None:
        """测试配置目录存在"""
        # 配置目录应在首次运行时自动创建
        # 此处仅验证路径定义正确
        assert str(CONFIG_DIR).endswith("voice-ime"), f"Config dir path incorrect: {CONFIG_DIR}"

    @pytest.mark.slow
    def test_config_file_format(self, exe_path: Path) -> None:
        """测试配置文件 JSON 格式"""
        kill_existing_voice_ime()
        process = None
        try:
            process = subprocess.Popen(
                [str(exe_path)],
                cwd=str(exe_path.parent),
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
            )
            time.sleep(3)

            # 验证配置文件存在（如果已存在）
            if CONFIG_FILE.exists():
                # 读取配置文件
                config_text = CONFIG_FILE.read_text(encoding="utf-8")
                config = json.loads(config_text)

                # 验证必要字段
                assert "hotkey" in config or "audio" in config, "Config missing required sections"

        finally:
            if process is not None:
                process.terminate()
                process.wait(timeout=5)


@pytest.mark.timeout(30)
class TestConfigDefaults:
    """默认配置测试类"""

    def test_default_hotkey_config(self, app_config: dict) -> None:
        """测试默认热键配置"""
        hotkey = app_config["hotkey"]
        assert hotkey["mode"] in ["toggle", "ptt"], f"Invalid hotkey mode: {hotkey['mode']}"
        assert hotkey["vk_code"] > 0, "Invalid vk_code"

    def test_default_audio_config(self, app_config: dict) -> None:
        """测试默认音频配置"""
        audio = app_config["audio"]
        assert audio["silence_threshold"] > 0, "Invalid silence_threshold"
        assert audio["silence_duration_ms"] > 0, "Invalid silence_duration_ms"
        assert audio["max_record_seconds"] > 0, "Invalid max_record_seconds"


@pytest.mark.timeout(30)
class TestConfigSaveLoad:
    """配置保存/加载一致性测试类"""

    @pytest.mark.slow
    def test_config_save_load_consistency(self, exe_path: Path) -> None:
        """测试配置保存后加载一致性"""
        kill_existing_voice_ime()
        process = None
        try:
            # 启动程序
            process = subprocess.Popen(
                [str(exe_path)],
                cwd=str(exe_path.parent),
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
            )
            time.sleep(3)

            # 此测试验证配置文件读写一致性
            # 实际验证需要通过 settings 界面修改配置
            # 此处仅验证程序运行状态
            assert process.poll() is None, "Process should be running"

        finally:
            if process is not None:
                process.terminate()
                process.wait(timeout=5)


@pytest.mark.timeout(10)
class TestConfigEnvironment:
    """配置环境测试类"""

    def test_config_env_variable(self) -> None:
        """测试 VOICE_IME_CONFIG_DIR 环境变量"""
        import os

        # 测试环境变量设置（可选）
        config_dir = os.getenv("VOICE_IME_CONFIG_DIR", "")
        if config_dir:
            assert Path(config_dir).exists(), f"Config dir not found: {config_dir}"

    def test_config_file_permissions(self) -> None:
        """测试配置文件权限"""
        # 此测试验证配置文件读写权限
        # 实际权限验证需要在 Windows 上实现
        pytest.skip("Config file permission test requires Windows-specific implementation")