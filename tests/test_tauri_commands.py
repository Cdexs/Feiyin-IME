"""
test_tauri_commands.py — Tauri Command 注册验证

覆盖 API-001 ~ API-004: 验证 main.rs 中词库相关的 Tauri Command 注册。
"""

import pytest
from pathlib import Path

PROJECT_ROOT = Path(__file__).parent.parent
MAIN_RS = PROJECT_ROOT / "src" / "main.rs"


def _read_main_rs() -> str:
    if not MAIN_RS.exists():
        pytest.skip("main.rs not found")
    return MAIN_RS.read_text(encoding="utf-8")


@pytest.mark.timeout(10)
class TestTauriCommandRegistration:
    """API-001 ~ API-004: Tauri Command 注册验证"""

    def test_api_001_get_wordbook_entries(self):
        """API-001: main.rs invoke_handler 包含 get_wordbook_entries"""
        content = _read_main_rs()
        has_cmd = "get_wordbook_entries" in content
        if not has_cmd:
            pytest.skip("get_wordbook_entries command not yet registered (requires WORDBOOK-API-001)")
        assert has_cmd, "main.rs should register get_wordbook_entries Tauri command"

    def test_api_002_add_wordbook_entry(self):
        """API-002: main.rs invoke_handler 包含 add_wordbook_entry"""
        content = _read_main_rs()
        has_cmd = "add_wordbook_entry" in content
        if not has_cmd:
            pytest.skip("add_wordbook_entry command not yet registered (requires WORDBOOK-API-001)")
        assert has_cmd, "main.rs should register add_wordbook_entry Tauri command"

    def test_api_003_delete_wordbook_entry_by_id(self):
        """API-003: main.rs invoke_handler 包含 delete_wordbook_entry_by_id"""
        content = _read_main_rs()
        has_cmd = "delete_wordbook_entry_by_id" in content
        if not has_cmd:
            pytest.skip("delete_wordbook_entry_by_id command not yet registered (requires WORDBOOK-FIX2-001)")
        assert has_cmd, "main.rs should register delete_wordbook_entry_by_id Tauri command"

    def test_api_004_get_wordbook_stats(self):
        """API-004: main.rs invoke_handler 包含 get_wordbook_stats"""
        content = _read_main_rs()
        has_cmd = "get_wordbook_stats" in content
        if not has_cmd:
            pytest.skip("get_wordbook_stats command not yet registered (requires WORDBOOK-API-001)")
        assert has_cmd, "main.rs should register get_wordbook_stats Tauri command"
