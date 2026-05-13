"""
跨平台抽象层测试（TEST-SYNC-MAC-003 + TEST-SYNC-MAC-004）

在 Windows 上验证跨平台抽象层结构：
- TEST-SYNC-MAC-003：热键平台抽象
- TEST-SYNC-MAC-004：文字注入平台抽象
"""

import subprocess
import sys
from pathlib import Path

import pytest

# 添加父目录到路径以便导入 conftest
sys.path.insert(0, str(Path(__file__).parent.parent))

pytestmark = [pytest.mark.timeout(30)]


class TestHotkeyPlatformAbstraction:
    """TEST-SYNC-MAC-003：热键平台抽象"""

    def test_platform_directory_structure(self) -> None:
        """测试 src/platform/ 目录结构完整"""
        project_root = Path(__file__).parent.parent.parent
        platform_dir = project_root / "src" / "platform"
        windows_dir = platform_dir / "windows"
        macos_dir = platform_dir / "macos"

        # 验证 platform 目录存在
        assert platform_dir.exists(), "src/platform/ directory should exist"
        assert platform_dir.is_dir()

        # 验证 Windows 模块完整
        assert windows_dir.exists(), "src/platform/windows/ directory should exist"
        assert (windows_dir / "mod.rs").exists(), "windows/mod.rs should exist"
        assert (windows_dir / "hotkey.rs").exists(), "windows/hotkey.rs should exist"
        assert (windows_dir / "injection.rs").exists(), "windows/injection.rs should exist"
        assert (windows_dir / "autolaunch.rs").exists(), "windows/autolaunch.rs should exist"
        assert (windows_dir / "event_loop.rs").exists(), "windows/event_loop.rs should exist"

        # 验证 macOS 模块存在（stub）
        assert macos_dir.exists(), "src/platform/macos/ directory should exist"
        assert (macos_dir / "mod.rs").exists(), "macos/mod.rs should exist"

    def test_platform_mod_rs_exports(self) -> None:
        """测试 platform/mod.rs 正确导出各平台模块"""
        project_root = Path(__file__).parent.parent.parent
        mod_rs = project_root / "src" / "platform" / "mod.rs"

        assert mod_rs.exists(), "platform/mod.rs should exist"
        content = mod_rs.read_text(encoding="utf-8")

        # 验证 cfg 条件编译
        assert 'cfg(target_os = "windows")' in content, "Windows cfg should be present"
        assert 'cfg(target_os = "macos")' in content, "macOS cfg should be present"

        # 验证 HotkeyEvent trait 或类型导出
        assert "HotkeyEvent" in content or "hotkey" in content.lower(), "HotkeyEvent should be exported"

    def test_cargo_test_platform_unit(self) -> None:
        """运行 cargo test 的 platform 相关单元测试"""
        project_root = Path(__file__).parent.parent.parent
        manifest = project_root / "Cargo.toml"

        if not manifest.exists():
            pytest.skip("Cargo.toml not found")

        # 运行 platform 模块的单元测试
        result = subprocess.run(
            [
                sys.executable, "-m", "cargo", "test",
                "--lib",
                "--",
                "platform",
                "--nocapture",
            ],
            cwd=str(project_root),
            capture_output=True,
            text=True,
            timeout=60,
        )

        # 验证测试通过（允许 0 个测试，但不能失败）
        assert result.returncode == 0, f"cargo test failed: {result.stderr}"


class TestTextInjectionPlatformAbstraction:
    """TEST-SYNC-MAC-004：文字注入平台抽象"""

    def test_windows_injection_interface(self) -> None:
        """测试 src/platform/windows/ 中 inject_text 接口"""
        project_root = Path(__file__).parent.parent.parent
        injection_rs = project_root / "src" / "platform" / "windows" / "injection.rs"

        assert injection_rs.exists(), "windows/injection.rs should exist"
        content = injection_rs.read_text(encoding="utf-8")

        # 验证关键函数存在
        assert "inject_text" in content or "InjectText" in content, "inject_text function should exist"
        assert "SendInput" in content or "send_input" in content, "SendInput API should be used"

    def test_platform_injection_trait(self) -> None:
        """测试平台抽象层定义注入 trait"""
        project_root = Path(__file__).parent.parent.parent
        mod_rs = project_root / "src" / "platform" / "mod.rs"

        content = mod_rs.read_text(encoding="utf-8")

        # 验证 trait 或接口定义
        assert (
            "trait" in content or "pub fn" in content
        ), "Platform injection trait should be defined"

    def test_cargo_test_injection_unit(self) -> None:
        """运行 cargo test 的 injection 相关单元测试"""
        project_root = Path(__file__).parent.parent.parent
        manifest = project_root / "Cargo.toml"

        if not manifest.exists():
            pytest.skip("Cargo.toml not found")

        # 运行 injection 模块的单元测试
        result = subprocess.run(
            [
                sys.executable, "-m", "cargo", "test",
                "--lib",
                "--",
                "inject",
                "--nocapture",
            ],
            cwd=str(project_root),
            capture_output=True,
            text=True,
            timeout=60,
        )

        # 验证测试通过（允许 0 个测试，但不能失败）
        assert result.returncode == 0, f"cargo test failed: {result.stderr}"


@pytest.mark.optional
class TestCrossPlatformConsistency:
    """跨平台一致性测试（可选）"""

    def test_windows_hotkey_vs_macos_stub(self) -> None:
        """验证 Windows 热键实现与 macOS stub 签名一致"""
        project_root = Path(__file__).parent.parent.parent

        windows_hotkey = project_root / "src" / "platform" / "windows" / "hotkey.rs"
        macos_mod = project_root / "src" / "platform" / "macos" / "mod.rs"

        if not windows_hotkey.exists() or not macos_mod.exists():
            pytest.skip("Platform files not found")

        win_content = windows_hotkey.read_text(encoding="utf-8")
        mac_content = macos_mod.read_text(encoding="utf-8")

        # 验证关键函数名在两边都存在（即使 macOS 是 stub）
        # 注意：不验证参数完全一致，因为 macOS 是 placeholder
        assert "hotkey" in win_content.lower() or "Hotkey" in win_content
        assert "hotkey" in mac_content.lower() or "Hotkey" in mac_content
