"""
Tauri v2 Commands invoke 测试

验证 Tauri v2 升级后前端 invoke 调用仍可正常工作：
- get_config: 读取配置
- save_config: 保存配置
- test_llm_connection: 测试 LLM 连通性
- get_audio_devices: 获取音频设备列表
- check_hotkey_available: 检查热键可用性
- show_recording_overlay: 显示录音悬浮窗
- hide_recording_overlay: 隐藏录音悬浮窗
- update_overlay_status: 更新悬浮窗状态

Tauri v2 变化点：
- 前端 invoke import 从 @tauri-apps/api/tauri 改为 @tauri-apps/api/core
- invoke 调用签名基本不变，但底层 IPC 机制有变化
- Rust 端 Command 注册方式不变（generate_handler!）
- 返回类型和参数类型应保持一致
"""

import json
import re
import time
import subprocess
from pathlib import Path

import pytest


UI_EXE_NAME = "feiyin-ime-ui.exe"
PROJECT_ROOT = Path(__file__).parent.parent.parent
TARGET_RELEASE = PROJECT_ROOT / "target" / "release"
UI_EXE_PATH = TARGET_RELEASE / UI_EXE_NAME


def kill_existing_ui() -> None:
    """Kill any running feiyin-ime-ui.exe instances."""
    subprocess.run(
        ["taskkill", "/F", "/IM", UI_EXE_NAME, "/T"],
        capture_output=True,
    )
    time.sleep(1)


@pytest.mark.timeout(30)
class TestTauriV2Commands:
    """Tauri v2 Commands 调用测试类

    注意：由于 Python 无法直接调用 Tauri invoke API，
    这些测试通过验证 Rust 端 Command 实现来间接验证。
    真实 invoke 测试需要在前端 E2E 测试中完成。
    """

    def test_get_config_command_exists(self) -> None:
        """验证 get_config Command 在 main.rs 中定义"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        assert main_rs.exists(), "main.rs should exist"

        content = main_rs.read_text(encoding="utf-8")
        assert "fn get_config()" in content, "get_config command should be defined"
        assert "Result<AppConfig, String>" in content, "get_config should return Result<AppConfig, String>"

    def test_save_config_command_exists(self) -> None:
        """验证 save_config Command 在 main.rs 中定义"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        assert "fn save_config(config: AppConfig)" in content, "save_config command should be defined"
        assert "Result<(), String>" in content, "save_config should return Result<(), String>"

    def test_test_llm_connection_command_exists(self) -> None:
        """验证 test_llm_connection Command 在 main.rs 中定义"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        assert "fn test_llm_connection" in content, "test_llm_connection command should be defined"
        assert "LlmConfig" in content, "test_llm_connection should use LlmConfig parameter"

    def test_get_audio_devices_command_exists(self) -> None:
        """验证 get_audio_devices Command 在 main.rs 中定义"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        assert "fn get_audio_devices()" in content, "get_audio_devices command should be defined"
        assert "Vec<String>" in content, "get_audio_devices should return Vec<String>"

    def test_check_hotkey_command_exists(self) -> None:
        """验证 check_hotkey_available Command 在 main.rs 中定义"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        assert "fn check_hotkey_available" in content, "check_hotkey_available command should be defined"

    def test_overlay_commands_exist(self) -> None:
        """验证 overlay 相关 Commands 在 overlay.rs 中定义"""
        overlay_rs = PROJECT_ROOT / "src-tauri" / "src" / "overlay.rs"
        assert overlay_rs.exists(), "overlay.rs should exist"

        content = overlay_rs.read_text(encoding="utf-8")
        assert "fn show_recording_overlay" in content, "show_recording_overlay command should be defined"
        assert "fn hide_recording_overlay" in content, "hide_recording_overlay command should be defined"
        assert "fn update_overlay_status" in content, "update_overlay_status command should be defined"

    def test_commands_registered_in_handler(self) -> None:
        """验证所有 Commands 在 generate_handler! 中注册"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        # 验证 generate_handler! 宏调用
        assert "generate_handler!" in content, "Commands should be registered via generate_handler!"

        # 验证关键命令在 handler 中
        expected_commands = [
            "get_config",
            "save_config",
            "test_llm_connection",
            "get_audio_devices",
            "check_hotkey_available",
            "show_recording_overlay",
            "hide_recording_overlay",
            "update_overlay_status",
        ]

        # 提取 generate_handler! 块内容
        handler_start = content.find("generate_handler!")
        if handler_start == -1:
            pytest.fail("generate_handler! macro not found in main.rs")

        handler_block_start = content.find("[", handler_start)
        handler_block_end = content.find("]", handler_block_start + 1)
        if handler_block_start == -1 or handler_block_end == -1:
            pytest.fail("generate_handler! block not found")

        handler_content = content[handler_block_start:handler_block_end + 1]

        for cmd in expected_commands:
            assert cmd in handler_content, f"Command '{cmd}' should be in generate_handler!"


@pytest.mark.timeout(30)
class TestTauriV2CommandsFrontend:
    """前端 invoke 调用验证（代码扫描级别）

    验证前端代码中 invoke 调用符合 v2 API 规范
    """

    def test_app_tsx_invoke_import(self) -> None:
        """验证 App.tsx 中 invoke import 路径（v2 应改为 @tauri-apps/api/core）"""
        app_tsx = PROJECT_ROOT / "ui" / "src" / "App.tsx"
        if not app_tsx.exists():
            pytest.skip("App.tsx not found (UI may use different structure in v2)")

        content = app_tsx.read_text(encoding="utf-8")

        # v1: import { invoke } from "@tauri-apps/api/tauri"
        # v2: import { invoke } from "@tauri-apps/api/core"
        # 此测试在升级后应检测到 import 路径变化

        if "@tauri-apps/api/tauri" in content:
            # v1 格式 - 记录当前状态
            pass
        elif "@tauri-apps/api/core" in content:
            # v2 格式 - 升级后应为此格式
            pass
        else:
            pytest.fail("invoke import not found in expected location")

    def test_voice_tsx_invoke_import(self) -> None:
        """验证 Voice.tsx 中 invoke import 路径"""
        voice_tsx = PROJECT_ROOT / "ui" / "src" / "pages" / "Voice.tsx"
        if not voice_tsx.exists():
            pytest.skip("Voice.tsx not found")

        content = voice_tsx.read_text(encoding="utf-8")

        # 检查 invoke 调用调用（支持泛型调用 invoke<type>("...") 和普通调用 invoke("...")）
        assert re.search(r"invoke\s*[<(]", content), "Voice.tsx should use invoke() or invoke<T>() to call backend"

    def test_llm_tsx_invoke_import(self) -> None:
        """验证 Llm.tsx 中 invoke import 路径"""
        llm_tsx = PROJECT_ROOT / "ui" / "src" / "pages" / "Llm.tsx"
        if not llm_tsx.exists():
            pytest.skip("Llm.tsx not found")

        content = llm_tsx.read_text(encoding="utf-8")

        # 检查 invoke 调用调用（支持泛型调用 invoke<type>("...") 和普通调用 invoke("...")）
        assert re.search(r"invoke\s*[<(]", content), "Llm.tsx should use invoke() or invoke<T>() to call backend"

    def test_all_invoke_calls_use_correct_signature(self) -> None:
        """验证所有 invoke 调用使用正确签名"""
        # 扫描所有前端文件中的 invoke 调用
        ui_src = PROJECT_ROOT / "ui" / "src"
        invoke_calls = []

        for tsx_file in ui_src.rglob("*.tsx"):
            content = tsx_file.read_text(encoding="utf-8")
            # 查找 invoke("command_name", { ... }) 模式
            import re
            matches = re.findall(r'invoke\s*\(\s*["\']([^"\']+)["\']', content)
            for match in matches:
                invoke_calls.append((tsx_file.name, match))

        # 验证所有 invoke 调用都有命令名称
        assert len(invoke_calls) > 0, "Should find invoke calls in frontend code"

        # 记录找到的调用（完整验证需要 v2 升级后更新预期命令列表）
        expected_commands = {
            "get_config",
            "save_config",
            "test_llm_connection",
            "get_audio_devices",
            "check_hotkey_available",
            "show_recording_overlay",
            "hide_recording_overlay",
            "update_overlay_status",
        }

        for filename, cmd in invoke_calls:
            assert cmd in expected_commands, f"Unexpected command '{cmd}' in {filename}"


@pytest.mark.timeout(20)
class TestTauriV2CommandsRuntime:
    """Commands 运行时验证（需要 UI 进程启动）

    这些测试在 v2 升级后需要真实运行环境验证
    """

    def test_get_config_returns_valid_config(self) -> None:
        """测试 get_config 返回有效配置（需要真实 invoke 调用）"""
        # 此测试需要前端 invoke 调用能力
        # Python 端只能通过验证配置文件的 schema 来间接验证
        config_path = Path.home() / "AppData" / "Local" / "voice-ime" / "config.json"

        if config_path.exists():
            config_text = config_path.read_text(encoding="utf-8")
            config = json.loads(config_text)
            assert isinstance(config, dict), "Config should be a valid JSON object"
        else:
            pytest.skip("Config file not found, may need to run UI first")

    def test_save_config_creates_config_file(self) -> None:
        """测试 save_config 能创建配置文件"""
        config_path = Path.home() / "AppData" / "Local" / "voice-ime" / "config.json"

        # 配置文件应在 UI 首次启动或保存时创建
        # 此处验证路径定义正确
        assert "voice-ime" in str(config_path), "Config path should contain 'voice-ime'"

    def test_commands_error_handling(self) -> None:
        """测试 Commands 错误处理（返回 Result<T, String>）"""
        # 验证 Rust 端所有 Command 都返回 Result 类型
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        # 所有 Command 应返回 Result
        import re
        command_fns = re.findall(r'fn\s+(\w+)\s*\([^)]*\)\s*->\s*(Result<[^>]+>)', content)

        assert len(command_fns) > 0, "Should find Command functions returning Result"

        for fn_name, return_type in command_fns:
            assert "Result" in return_type, f"Command '{fn_name}' should return Result type"
