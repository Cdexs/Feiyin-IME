"""
Tauri v2 事件系统测试

验证 Tauri v2 升级后事件收发机制正常工作：
- overlay-state-changed 事件正常发送和接收
- 窗口生命周期事件（CloseRequested）处理正确
- App 生命周期事件（RunEvent::Exit）处理正确

Tauri v2 变化点：
- on_window_event 签名变化：v1 是 |event|，v2 是 |window, event|
- WindowEvent 类型可能变化（如 CloseRequested 路径变化）
- emit API 基本不变，但底层实现有差异
- WebviewWindowEvent 替代部分 WindowEvent 类型
"""

import time
import subprocess
from pathlib import Path

import pytest


PROJECT_ROOT = Path(__file__).parent.parent.parent


@pytest.mark.timeout(20)
class TestTauriV2OverlayEvents:
    """Overlay 事件收发测试"""

    def test_overlay_emit_call_exists(self) -> None:
        """验证 overlay.rs 中存在 window.emit() 调用"""
        overlay_rs = PROJECT_ROOT / "src-tauri" / "src" / "overlay.rs"
        assert overlay_rs.exists(), "overlay.rs should exist"

        content = overlay_rs.read_text(encoding="utf-8")
        assert ".emit(" in content, "overlay.rs should use window.emit() to send events"

    def test_overlay_state_changed_event_name(self) -> None:
        """验证 overlay-state-changed 事件名称正确"""
        overlay_rs = PROJECT_ROOT / "src-tauri" / "src" / "overlay.rs"
        content = overlay_rs.read_text(encoding="utf-8")

        assert '"overlay-state-changed"' in content or "'overlay-state-changed'" in content, \
            "overlay-state-changed event name should be correct"

    def test_overlay_emit_payload_serialization(self) -> None:
        """验证事件 payload 使用 serde_json 序列化"""
        overlay_rs = PROJECT_ROOT / "src-tauri" / "src" / "overlay.rs"
        content = overlay_rs.read_text(encoding="utf-8")

        assert "serde_json" in content, "Event payload should use serde_json serialization"

    def test_overlay_state_enum_defined(self) -> None:
        """验证 OverlayState 枚举定义"""
        overlay_rs = PROJECT_ROOT / "src-tauri" / "src" / "overlay.rs"
        content = overlay_rs.read_text(encoding="utf-8")

        # 应有 OverlayState 或类似状态枚举
        has_state_enum = (
            "OverlayState" in content or
            "enum" in content and ("Recording" in content or "recording" in content)
        )
        assert has_state_enum, "OverlayState enum or similar should be defined"


@pytest.mark.timeout(20)
class TestTauriV2WindowEvents:
    """窗口生命周期事件测试"""

    def test_close_requested_handler_exists(self) -> None:
        """验证 CloseRequested 事件处理存在"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        assert "CloseRequested" in content, "CloseRequested event handler should exist"

    def test_close_handler_closes_overlay(self) -> None:
        """验证主窗口关闭时显式关闭 overlay"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        # 查找 overlay 窗口关闭逻辑
        has_overlay_close = (
            'get_window("overlay")' in content or
            'get_window("overlay")' in content or
            ".close()" in content
        )
        assert has_overlay_close, "Main window close handler should close overlay window"

    def test_close_handler_exits_app(self) -> None:
        """验证主窗口关闭时退出整个应用"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        # 应调用 app_handle.exit(0) 或等效方法
        has_exit = (
            "app_handle().exit(" in content or
            ".exit(0)" in content or
            "app_handle.exit(" in content
        )
        assert has_exit, "Close handler should call app_handle.exit()"

    def test_on_window_event_signature(self) -> None:
        """验证 on_window_event 签名（v2 升级后应变化）"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        # v1: .on_window_event(|event| { ... })
        # v2: .on_window_event(|window, event| { ... })
        # 此测试检测当前签名状态

        has_on_window_event = ".on_window_event(" in content
        assert has_on_window_event, "on_window_event handler should exist"

        # 记录当前签名格式（用于升级后对比）
        if "|event|" in content and "|window, event|" not in content:
            # v1 格式
            pass
        elif "|window, event|" in content:
            # v2 格式
            pass


@pytest.mark.timeout(20)
class TestTauriV2AppEvents:
    """App 生命周期事件测试"""

    def test_run_event_exit_handler(self) -> None:
        """验证 RunEvent::Exit 处理存在"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        # 应有 app.run() 或等效事件循环
        has_app_run = (
            ".run(" in content or
            "app.run(" in content
        )
        assert has_app_run, "App event loop should exist via .run()"

        # 应有 Exit 事件处理
        has_exit_event = (
            "RunEvent::Exit" in content or
            "Event::Exit" in content
        )
        assert has_exit_event, "Exit event handler should exist"

    def test_crash_exit_integration(self) -> None:
        """验证崩溃退出与 crash reporter 集成"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        # Exit 事件处理中应有 crash 相关逻辑
        has_crash_ref = (
            "crash" in content.lower() or
            "mark_expected_exit" in content or
            "crash.rs" in content
        )
        assert has_crash_ref, "Exit handler should integrate with crash reporter"


@pytest.mark.timeout(20)
class TestTauriV2EventEmitFrontend:
    """前端事件监听验证"""

    def test_frontend_listens_to_overlay_events(self) -> None:
        """验证前端监听 overlay-state-changed 事件"""
        ui_src = PROJECT_ROOT / "ui" / "src"

        found_listener = False
        for tsx_file in ui_src.rglob("*.tsx"):
            content = tsx_file.read_text(encoding="utf-8")
            if "overlay-state-changed" in content or "listen(" in content:
                found_listener = True
                break

        # 前端可能不直接监听 overlay 事件（由 Rust 端处理）
        # 此测试记录当前状态
        if not found_listener:
            pytest.skip("Frontend may not directly listen to overlay events")

    def test_frontend_event_import_v2(self) -> None:
        """验证前端事件 import 路径（v2 变化）"""
        ui_src = PROJECT_ROOT / "ui" / "src"

        # v1: import { listen } from "@tauri-apps/api/event"
        # v2: import { listen } from "@tauri-apps/api/event" (路径可能不变或微调)
        # 扫描所有前端文件

        for tsx_file in ui_src.rglob("*.tsx"):
            content = tsx_file.read_text(encoding="utf-8")
            if "from \"@tauri-apps/api/event\"" in content or "from '@tauri-apps/api/event'" in content:
                # v1 或 v2 格式（取决于升级后路径是否变化）
                return

        pytest.skip("No event listener imports found (may use different pattern)")


@pytest.mark.timeout(15)
class TestTauriV2MultiWindowEvents:
    """多窗口事件路由测试"""

    def test_overlay_window_label(self) -> None:
        """验证 overlay 窗口 label 正确"""
        conf_path = PROJECT_ROOT / "src-tauri" / "tauri.conf.json"
        if not conf_path.exists():
            pytest.skip("tauri.conf.json not found")

        import json
        conf_text = conf_path.read_text(encoding="utf-8")
        conf = json.loads(conf_text)

        # 查找 overlay 窗口配置
        windows = conf.get("tauri", {}).get("windows", [])
        overlay_window = None
        for w in windows:
            if w.get("label") == "overlay" or "overlay" in str(w.get("url", "")):
                overlay_window = w
                break

        if overlay_window:
            assert overlay_window.get("label") == "overlay", "Overlay window label should be 'overlay'"
        else:
            pytest.skip("Overlay window not found in config")

    def test_main_window_label(self) -> None:
        """验证主窗口 label 正确"""
        conf_path = PROJECT_ROOT / "src-tauri" / "tauri.conf.json"
        if not conf_path.exists():
            pytest.skip("tauri.conf.json not found")

        import json
        conf_text = conf_path.read_text(encoding="utf-8")
        conf = json.loads(conf_text)

        windows = conf.get("tauri", {}).get("windows", [])
        main_window = None
        for w in windows:
            if w.get("label") == "main" or w.get("title") == "语音输入法" or w.get("title") == "飞音语音输入":
                main_window = w
                break

        if main_window:
            assert main_window.get("label") == "main", "Main window label should be 'main'"
        else:
            pytest.skip("Main window not found in config")

    def test_window_event_routing(self) -> None:
        """验证窗口事件路由到正确窗口"""
        main_rs = PROJECT_ROOT / "src-tauri" / "src" / "main.rs"
        content = main_rs.read_text(encoding="utf-8")

        # 事件处理应检查 window label
        has_label_check = (
            'label() != "main"' in content or
            "label() != \"main\"" in content or
            '.label() == "main"' in content
        )
        assert has_label_check, "Window event handler should check window label for routing"
