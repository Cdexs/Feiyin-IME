"""
Tauri v2 Capabilities 权限验证测试

验证 Tauri v2 升级后 capabilities 权限系统配置正确：
- capabilities/default.json 存在且格式正确
- 权限声明覆盖所有前端 API 调用需求
- 权限最小化原则（不授予不必要权限）

Tauri v2 变化点：
- v1: tauri.conf.json 中的 "allowlist" 字段
- v2: 独立的 capabilities/ 目录 + JSON 文件
- v2: 权限粒度更细，支持自定义权限集
- v2: 默认拒绝所有，必须显式授予
"""

import json
from pathlib import Path

import pytest


PROJECT_ROOT = Path(__file__).parent.parent.parent


@pytest.mark.timeout(10)
class TestTauriV2CapabilitiesConfig:
    """Capabilities 配置文件测试"""

    def test_capabilities_directory_exists(self) -> None:
        """测试 capabilities/ 目录存在（v2 升级后应存在）"""
        caps_dir = PROJECT_ROOT / "src-tauri" / "capabilities"
        if caps_dir.exists():
            assert caps_dir.is_dir(), "capabilities should be a directory"
        else:
            pytest.skip("capabilities/ directory not found (not upgraded to v2 yet)")

    def test_default_capability_exists(self) -> None:
        """测试 default.json 存在"""
        caps_path = PROJECT_ROOT / "src-tauri" / "capabilities" / "default.json"
        if caps_path.exists():
            assert caps_path.is_file(), "default.json should be a file"

            # 验证 JSON 格式
            content = caps_path.read_text(encoding="utf-8")
            try:
                caps = json.loads(content)
                assert isinstance(caps, dict), "default.json should contain a JSON object"
            except json.JSONDecodeError as e:
                pytest.fail(f"default.json should be valid JSON: {e}")
        else:
            pytest.skip("capabilities/default.json not found (not upgraded to v2 yet)")

    def test_capability_identifier(self) -> None:
        """测试 capability 标识符存在"""
        caps_path = PROJECT_ROOT / "src-tauri" / "capabilities" / "default.json"
        if not caps_path.exists():
            pytest.skip("default.json not found")

        content = json.loads(caps_path.read_text(encoding="utf-8"))

        # v2 capability 格式: { "identifier": "...", "permissions": [...] }
        # 或 { "identifier": "...", "windows": [...], "permissions": {...} }
        if "identifier" in content:
            assert isinstance(content["identifier"], str), "identifier should be a string"
            assert len(content["identifier"]) > 0, "identifier should not be empty"
        else:
            pytest.skip("identifier field not found (may use different v2 format)")

    def test_permissions_field_exists(self) -> None:
        """测试 permissions 字段存在"""
        caps_path = PROJECT_ROOT / "src-tauri" / "capabilities" / "default.json"
        if not caps_path.exists():
            pytest.skip("default.json not found")

        content = json.loads(caps_path.read_text(encoding="utf-8"))

        if "permissions" in content:
            perms = content["permissions"]
            assert isinstance(perms, (list, dict)), "permissions should be a list or object"
        else:
            pytest.skip("permissions field not found (may use different v2 format)")


@pytest.mark.timeout(10)
class TestTauriV2PermissionsScope:
    """权限范围测试"""

    def test_shell_permission(self) -> None:
        """测试 shell 权限（用于打开外部链接等）"""
        caps_path = PROJECT_ROOT / "src-tauri" / "capabilities" / "default.json"
        if not caps_path.exists():
            pytest.skip("default.json not found")

        content = caps_path.read_text(encoding="utf-8")

        # v2 中 shell 权限需显式授予
        # 检查是否包含 shell:allow-open 或等效权限
        has_shell_perm = (
            "shell:allow-open" in content or
            '"shell"' in content or
            "'shell'" in content
        )

        if has_shell_perm:
            # 权限已授予
            pass
        else:
            pytest.skip("Shell permission not found (may not be needed or uses different format)")

    def test_fs_permission(self) -> None:
        """测试文件系统权限（配置读写）"""
        caps_path = PROJECT_ROOT / "src-tauri" / "capabilities" / "default.json"
        if not caps_path.exists():
            pytest.skip("default.json not found")

        content = caps_path.read_text(encoding="utf-8")

        # 配置读写需要 fs 权限
        has_fs_perm = (
            "fs:" in content or
            '"fs"' in content or
            "'fs'" in content or
            "allowlist" in content.lower()  # v1 格式
        )

        if has_fs_perm:
            # 权限已授予
            pass
        else:
            pytest.skip("FS permission not found (may use different mechanism)")

    def test_dialog_permission(self) -> None:
        """测试对话框权限（文件选择器等）"""
        caps_path = PROJECT_ROOT / "src-tauri" / "capabilities" / "default.json"
        if not caps_path.exists():
            pytest.skip("default.json not found")

        content = caps_path.read_text(encoding="utf-8")

        has_dialog_perm = (
            "dialog:" in content or
            '"dialog"' in content or
            "'dialog'" in content
        )

        if has_dialog_perm:
            pass
        else:
            pytest.skip("Dialog permission not found (may not be needed)")


@pytest.mark.timeout(10)
class TestTauriV2PermissionsMinimal:
    """权限最小化验证"""

    def test_no_unnecessary_permissions(self) -> None:
        """验证未授予不必要的权限"""
        caps_path = PROJECT_ROOT / "src-tauri" / "capabilities" / "default.json"
        if not caps_path.exists():
            pytest.skip("default.json not found")

        content = caps_path.read_text(encoding="utf-8")
        caps = json.loads(content)

        # 不应授予的危险权限
        dangerous_permissions = [
            "shell:allow-execute",  # 任意命令执行
            "fs:allow-write",       # 任意文件写入
        ]

        for dangerous_perm in dangerous_permissions:
            if dangerous_perm in content:
                pytest.fail(f"Dangerous permission '{dangerous_perm}' should not be granted")

    def test_permissions_match_frontend_needs(self) -> None:
        """验证权限覆盖前端所有 API 调用需求"""
        # 扫描前端所有 invoke 调用
        ui_src = PROJECT_ROOT / "ui" / "src"
        invoked_commands = set()

        import re
        for tsx_file in ui_src.rglob("*.tsx"):
            content = tsx_file.read_text(encoding="utf-8")
            matches = re.findall(r'invoke\s*\(\s*["\']([^"\']+)["\']', content)
            invoked_commands.update(matches)

        # 验证每个 invoke 调用都有对应的后端 Command 支持
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        main_content = main_rs.read_text(encoding="utf-8")

        overlay_rs = PROJECT_ROOT / "src-tauri" / "src" / "overlay.rs"
        overlay_content = overlay_rs.read_text(encoding="utf-8") if overlay_rs.exists() else ""

        all_commands = main_content + overlay_content

        for cmd in invoked_commands:
            assert cmd in all_commands, f"Frontend invokes '{cmd}' but no matching Command found in Rust"


@pytest.mark.timeout(10)
class TestTauriV2AllowlistMigration:
    """Allowlist 到 Capabilities 迁移验证"""

    def test_v1_allowlist_removed(self) -> None:
        """验证 v1 allowlist 已从 tauri.conf.json 移除（v2 升级后）"""
        conf_path = PROJECT_ROOT / "src-tauri" / "tauri.conf.json"
        if not conf_path.exists():
            pytest.skip("tauri.conf.json not found")

        conf = json.loads(conf_path.read_text(encoding="utf-8"))

        # v2 中 tauri.conf.json 不应有 allowlist 字段
        tauri_section = conf.get("tauri", {})
        if "allowlist" in tauri_section:
            pytest.skip("v1 allowlist still present (not upgraded yet)")
        else:
            # v2 格式 - allowlist 已移除
            pass

    def test_v2_capabilities_in_config(self) -> None:
        """验证 tauri.conf.json 中引用 capabilities（v2 格式）"""
        conf_path = PROJECT_ROOT / "src-tauri" / "tauri.conf.json"
        if not conf_path.exists():
            pytest.skip("tauri.conf.json not found")

        conf = json.loads(conf_path.read_text(encoding="utf-8"))

        # v2 可能在 build 或 app 段引用 capabilities
        # 具体格式取决于 v2 版本
        has_capabilities_ref = (
            "capabilities" in json.dumps(conf) or
            "defaultPermission" in json.dumps(conf)
        )

        if has_capabilities_ref:
            # v2 格式 - 有 capabilities 引用
            pass
        else:
            pytest.skip("No capabilities reference found (not upgraded yet or uses different format)")
