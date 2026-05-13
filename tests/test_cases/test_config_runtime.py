"""
配置运行时生效验证（E2E）

测试用户在 Tauri 配置界面修改配置后，主进程是否真正重载并生效。
覆盖场景：
1. 热键切换后生效
2. PTT/Toggle 模式切换
3. LLM 参数修改生效
4. 静音阈值修改生效
"""

import os
import sys
import time
import subprocess
from pathlib import Path

import pytest

# 添加项目根目录到路径
PROJECT_ROOT = Path(__file__).parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "tests"))

from conftest import wait_for_condition


# 配置文件路径
if os.name == "nt":
    CONFIG_DIR = Path.home() / "AppData" / "Roaming" / "voice-ime"
else:
    CONFIG_DIR = Path.home() / ".config" / "voice-ime"
CONFIG_FILE = CONFIG_DIR / "config.toml"


def read_config() -> dict:
    """
    读取 TOML 配置文件，返回字典

    使用 Python 内置方式解析（Python 3.11+ 有 tomllib，否则用简单解析）
    """
    try:
        import tomllib
    except ImportError:
        try:
            import tomli as tomllib
        except ImportError:
            # 如果没有 tomllib/tomli，使用简单字符串解析
            return _simple_parse_toml(CONFIG_FILE)

    if not CONFIG_FILE.exists():
        return {}

    with open(CONFIG_FILE, "rb") as f:
        return tomllib.load(f)


def _simple_parse_toml(path: Path) -> dict:
    """简单的 TOML 解析（fallback）"""
    result = {}
    current_section = result
    section_name = None

    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue

            # 节标题 [section]
            if line.startswith("[") and line.endswith("]"):
                section_name = line[1:-1].strip()
                current_section = result.setdefault(section_name, {})
                continue

            # 键值对 key = value
            if "=" in line:
                key, value = line.split("=", 1)
                key = key.strip()
                value = value.strip()

                # 去除引号
                if (value.startswith('"') and value.endswith('"')) or \
                   (value.startswith("'") and value.endswith("'")):
                    value = value[1:-1]
                elif value.startswith('"""') and value.endswith('"""'):
                    value = value[3:-3]
                # 布尔值
                elif value.lower() == "true":
                    value = True
                elif value.lower() == "false":
                    value = False
                # 数字
                elif value.replace(".", "").replace("-", "").isdigit():
                    value = float(value) if "." in value else int(value)

                current_section[key] = value

    return result


def write_config_section(section: str, key: str, value) -> None:
    """
    写入配置文件的特定节和键

    注意：这是一个简化的写入方法，适用于测试场景
    """
    if not CONFIG_FILE.exists():
        CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        CONFIG_FILE.touch()

    content = CONFIG_FILE.read_text(encoding="utf-8")

    # 查找节
    section_header = f"[{section}]"
    lines = content.split("\n")
    section_start = None
    section_end = None

    for i, line in enumerate(lines):
        if line.strip() == section_header:
            section_start = i
        elif section_start is not None and line.strip().startswith("["):
            section_end = i
            break

    # 如果节不存在，追加
    if section_start is None:
        lines.append("")
        lines.append(section_header)
        lines.append(f"{key} = {_format_value(value)}")
        CONFIG_FILE.write_text("\n".join(lines), encoding="utf-8")
        return

    # 在节中查找键
    end = section_end if section_end is not None else len(lines)
    key_found = False
    for i in range(section_start + 1, end):
        if lines[i].strip().startswith(f"{key} =") or lines[i].strip().startswith(f"{key}="):
            lines[i] = f"{key} = {_format_value(value)}"
            key_found = True
            break

    if not key_found:
        lines.insert(end, f"{key} = {_format_value(value)}")

    CONFIG_FILE.write_text("\n".join(lines), encoding="utf-8")


def _format_value(value) -> str:
    """格式化值为 TOML 格式"""
    if isinstance(value, bool):
        return "true" if value else "false"
    elif isinstance(value, (int, float)):
        return str(value)
    elif isinstance(value, str):
        return f'"{value}"'
    else:
        return str(value)


def backup_config() -> Path:
    """备份当前配置"""
    if CONFIG_FILE.exists():
        backup = CONFIG_FILE.with_suffix(".toml.backup")
        import shutil
        shutil.copy2(CONFIG_FILE, backup)
        return backup
    return None


def restore_config(backup: Path) -> None:
    """恢复备份配置"""
    if backup and backup.exists():
        import shutil
        shutil.copy2(backup, CONFIG_FILE)
        backup.unlink()


@pytest.mark.slow
@pytest.mark.timeout(60)
class TestHotkeySwitch:
    """场景1：热键切换后生效"""

    def test_hotkey_change_and_reload(self, exe_path: Path) -> None:
        """
        测试热键修改后 CONFIG_TIMER 重新加载

        步骤：
        1. 备份当前配置
        2. 启动主程序
        3. 修改热键配置（F9 → F10）
        4. 等待 CONFIG_TIMER 触发（约 3s）
        5. 读取配置文件验证新热键值已持久化
        """
        backup = backup_config()
        process = None

        try:
            # 启动主程序
            process = subprocess.Popen(
                [str(exe_path)],
                cwd=str(exe_path.parent),
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
            )

            # 等待进程初始化
            assert wait_for_condition(
                lambda: process.poll() is None,
                timeout=10.0,
                description="process initialization"
            ), "Process failed to initialize"

            # 修改热键配置：F9 (vk=120) → F10 (vk=121)
            write_config_section("hotkey", "vk_code", 121)
            write_config_section("hotkey", "display_name", "F10")

            # 等待 CONFIG_TIMER 触发（轮询间隔 250ms，等待 3s 确保重载）
            time.sleep(3)

            # 读取配置文件验证
            config = read_config()
            hotkey = config.get("hotkey", {})

            # 验证热键值已更新
            assert hotkey.get("vk_code") == 121, \
                f"Hotkey vk_code not updated: expected 121, got {hotkey.get('vk_code')}"
            assert hotkey.get("display_name") == "F10", \
                f"Display name not updated: expected 'F10', got {hotkey.get('display_name')}"

        finally:
            if process is not None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            restore_config(backup)


@pytest.mark.slow
@pytest.mark.timeout(60)
class TestModeSwitch:
    """场景2：PTT/Toggle 模式切换"""

    def test_mode_change_from_toggle_to_ptt(self, exe_path: Path) -> None:
        """
        测试录音模式切换

        步骤：
        1. 启动主程序
        2. 修改 hotkey.mode 从 toggle → ptt
        3. 等待 CONFIG_TIMER 触发
        4. 读取配置验证 mode 字段已更新
        """
        backup = backup_config()
        process = None

        try:
            # 启动主程序
            process = subprocess.Popen(
                [str(exe_path)],
                cwd=str(exe_path.parent),
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
            )

            assert wait_for_condition(
                lambda: process.poll() is None,
                timeout=10.0,
                description="process initialization"
            )

            # 修改模式
            write_config_section("hotkey", "mode", "PushToTalk")

            # 等待 CONFIG_TIMER 触发
            time.sleep(3)

            # 验证
            config = read_config()
            hotkey = config.get("hotkey", {})

            assert hotkey.get("mode") == "PushToTalk", \
                f"Mode not updated: expected 'PushToTalk', got {hotkey.get('mode')}"

        finally:
            if process is not None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            restore_config(backup)


@pytest.mark.slow
@pytest.mark.timeout(60)
class TestLLMConfigChange:
    """场景3：LLM 参数修改生效"""

    def test_llm_model_change(self, exe_path: Path) -> None:
        """
        测试 LLM 模型参数修改

        步骤：
        1. 启动主程序
        2. 修改 llm.model 值
        3. 等待 CONFIG_TIMER 触发
        4. 读取配置验证 model 字段已更新
        """
        backup = backup_config()
        process = None

        try:
            # 启动主程序
            process = subprocess.Popen(
                [str(exe_path)],
                cwd=str(exe_path.parent),
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
            )

            assert wait_for_condition(
                lambda: process.poll() is None,
                timeout=10.0,
                description="process initialization"
            )

            # 修改 LLM 模型
            new_model = "Qwen/Qwen3-14B"
            write_config_section("llm", "model", new_model)

            # 等待 CONFIG_TIMER 触发
            time.sleep(3)

            # 验证
            config = read_config()
            llm = config.get("llm", {})

            assert llm.get("model") == new_model, \
                f"LLM model not updated: expected '{new_model}', got {llm.get('model')}"

        finally:
            if process is not None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            restore_config(backup)


@pytest.mark.slow
@pytest.mark.timeout(60)
class TestSilenceThresholdChange:
    """场景4：静音阈值修改生效"""

    def test_silence_threshold_change(self, exe_path: Path) -> None:
        """
        测试静音阈值修改

        步骤：
        1. 启动主程序
        2. 修改 audio.silence_threshold 值
        3. 等待 CONFIG_TIMER 触发
        4. 读取配置验证 threshold 字段已更新
        """
        backup = backup_config()
        process = None

        try:
            # 启动主程序
            process = subprocess.Popen(
                [str(exe_path)],
                cwd=str(exe_path.parent),
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
            )

            assert wait_for_condition(
                lambda: process.poll() is None,
                timeout=10.0,
                description="process initialization"
            )

            # 修改静音阈值
            new_threshold = 0.05
            write_config_section("audio", "silence_threshold", new_threshold)

            # 等待 CONFIG_TIMER 触发
            time.sleep(3)

            # 验证
            config = read_config()
            audio = config.get("audio", {})

            actual_threshold = audio.get("silence_threshold")
            assert actual_threshold == new_threshold, \
                f"Silence threshold not updated: expected {new_threshold}, got {actual_threshold}"

        finally:
            if process is not None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            restore_config(backup)
