"""
test_wordbook_frontend.py — 前端调用验证

覆盖 API-005 ~ API-008: 验证 Wordbook.tsx 中对 Tauri API 的调用。
"""

import pytest
from pathlib import Path

PROJECT_ROOT = Path(__file__).parent.parent
WORDBOOK_TSX = PROJECT_ROOT / "ui" / "src" / "pages" / "Wordbook.tsx"


def _read_wordbook_tsx() -> str:
    if not WORDBOOK_TSX.exists():
        pytest.skip("Wordbook.tsx not found")
    return WORDBOOK_TSX.read_text(encoding="utf-8")


@pytest.mark.timeout(10)
class TestWordbookFrontendCalls:
    """API-005 ~ API-008: 前端调用验证"""

    def test_api_005_invoke_import(self):
        """API-005: Wordbook.tsx 导入 @tauri-apps/api invoke"""
        content = _read_wordbook_tsx()
        assert "import" in content and "invoke" in content and "@tauri-apps/api/core" in content, (
            "Wordbook.tsx should import invoke from @tauri-apps/api/core"
        )

    def test_api_006_delete_invoke_call(self):
        """API-006: handleDelete 调用 invoke('delete_wordbook_entry_by_id')"""
        content = _read_wordbook_tsx()
        assert "invoke('delete_wordbook_entry_by_id'" in content or 'invoke("delete_wordbook_entry_by_id"' in content, (
            "handleDelete should call invoke('delete_wordbook_entry_by_id')"
        )

    def test_api_007_add_invoke_call(self):
        """API-007: handleAdd 调用 invoke('add_wordbook_entry')"""
        content = _read_wordbook_tsx()
        assert "invoke('add_wordbook_entry'" in content or 'invoke("add_wordbook_entry"' in content, (
            "handleAdd should call invoke('add_wordbook_entry')"
        )

    def test_api_008_load_entries_on_mount(self):
        """API-008: useEffect/组件挂载调用 invoke('get_wordbook_entries')"""
        content = _read_wordbook_tsx()
        assert "invoke('get_wordbook_entries'" in content or 'invoke("get_wordbook_entries"' in content, (
            "Component should call invoke('get_wordbook_entries') on mount"
        )
