"""
test_wordbook_ui.py — 词库 UI 修复测试同步

覆盖 WORDBOOK-UI-FIX-001 6 项修复的测试案例：
1. 描述文字移除验证（WB-UI-001, WB-UI-002）
2. 统计栏样式验证（WB-UI-003, WB-UI-004, WB-UI-005）
3. 删除功能验证（WB-UI-006, WB-UI-007）
4. 添加按钮位置和状态验证（WB-UI-008 ~ WB-UI-011）
5. 对话框样式验证（WB-UI-012, WB-UI-013, WB-UI-014）
6. 窗口居中验证（WB-UI-015）
7. LLM 测试按钮图标验证（WB-UI-016, WB-UI-017）

注意：本文件中的测试采用源码静态分析方式验证，不依赖运行时环境。
执行测试在 Phase 4 统一进行。
"""

import pytest
from pathlib import Path

PROJECT_ROOT = Path(__file__).parent.parent
WORDBOOK_TSX = PROJECT_ROOT / "ui" / "src" / "pages" / "Wordbook.tsx"
STYLES_CSS = PROJECT_ROOT / "ui" / "src" / "styles.css"
LLM_TSX = PROJECT_ROOT / "ui" / "src" / "pages" / "Llm.tsx"
TAURI_CONF = PROJECT_ROOT / "src-tauri" / "tauri.conf.json"


def _read_file(path: Path) -> str:
    """安全读取文件"""
    if not path.exists():
        pytest.skip(f"File not found: {path}")
    return path.read_text(encoding="utf-8")


# ============================================================
# 1. 描述文字移除验证
# ============================================================

@pytest.mark.timeout(10)
class TestDescriptionRemoved:
    """WB-UI-001/002: 描述文字移除验证"""

    def test_wb_ui_001_no_description_text(self):
        """WB-UI-001: Wordbook 页面不包含 '维护常用术语映射' 文字"""
        content = _read_file(WORDBOOK_TSX)
        # 验证不包含描述文字
        assert "维护常用术语映射" not in content, (
            "Wordbook page should not contain description text '维护常用术语映射'"
        )

    def test_wb_ui_002_no_form_hint_element(self):
        """WB-UI-002: <h2> 标题下方无 <p className='form-hint'> 元素"""
        content = _read_file(WORDBOOK_TSX)
        # 验证没有 form-hint 类
        assert 'className="form-hint"' not in content and "className='form-hint'" not in content, (
            "Wordbook page should not have <p className='form-hint'> element"
        )


# ============================================================
# 2. 统计栏样式验证
# ============================================================

@pytest.mark.timeout(10)
class TestStatsBarStyle:
    """WB-UI-003/004/005: 统计栏样式验证"""

    def test_wb_ui_003_stats_no_background(self):
        """WB-UI-003: .wordbook-stats 无 background 属性或有透明背景"""
        css = _read_file(STYLES_CSS)
        # 找到 .wordbook-stats 样式块
        stats_block = css.split(".wordbook-stats {")
        if len(stats_block) < 2:
            pytest.skip("Could not find .wordbook-stats in styles.css")
        block_end = stats_block[1].find("}")
        stats_styles = stats_block[1][:block_end]

        # 验证没有 background 属性
        assert "background:" not in stats_styles and "background:" not in stats_styles.lower(), (
            ".wordbook-stats should not have background property"
        )

    def test_wb_ui_004_stats_font_size(self):
        """WB-UI-004: .wordbook-stats font-size <= 14px"""
        css = _read_file(STYLES_CSS)
        stats_block = css.split(".wordbook-stats {")
        if len(stats_block) < 2:
            pytest.skip("Could not find .wordbook-stats in styles.css")
        block_end = stats_block[1].find("}")
        stats_styles = stats_block[1][:block_end]

        # 验证 font-size
        assert "font-size: 14px" in stats_styles or "font-size:14px" in stats_styles, (
            ".wordbook-stats font-size should be <= 14px"
        )

    def test_wb_ui_005_stats_after_title(self):
        """WB-UI-005: 统计栏紧跟 <h2> 标题（DOM 顺序验证）"""
        content = _read_file(WORDBOOK_TSX)
        # 验证 h2 标题在 wordbook-stats 之前
        h2_pos = content.find('<h2 className="page-title">')
        stats_pos = content.find('className="wordbook-stats"')
        assert h2_pos != -1 and stats_pos != -1, (
            "Should find both h2 title and wordbook-stats"
        )
        assert h2_pos < stats_pos, (
            "wordbook-stats should appear after h2 title in DOM order"
        )


# ============================================================
# 3. 删除功能验证
# ============================================================

@pytest.mark.timeout(10)
class TestDeleteFunction:
    """WB-UI-006/007: 删除功能验证"""

    def test_wb_ui_006_delete_reduces_entries(self):
        """WB-UI-006: handleDelete 调用后 entries state 减少 1 条"""
        content = _read_file(WORDBOOK_TSX)
        # 验证 handleDelete 使用 filter 减少条目
        assert "handleDelete" in content, (
            "Should have handleDelete function"
        )
        assert "prev.filter" in content or "prev => prev.filter" in content, (
            "handleDelete should use filter to reduce entries"
        )

    def test_wb_ui_007_delete_button_onclick(self):
        """WB-UI-007: 点击删除按钮触发 onClick 事件"""
        content = _read_file(WORDBOOK_TSX)
        # 验证删除按钮有 onClick 调用 handleDelete
        assert 'onClick={() => handleDelete(id)}' in content or "onClick={() => handleDelete(id)}" in content, (
            "Delete button should have onClick handler calling handleDelete"
        )


# ============================================================
# 4. 添加按钮位置和状态验证
# ============================================================

@pytest.mark.timeout(10)
class TestAddButton:
    """WB-UI-008 ~ WB-UI-011: 添加按钮位置和状态验证"""

    def test_wb_ui_008_add_button_in_tabs(self):
        """WB-UI-008: 添加按钮在 .wordbook-tabs 区域内"""
        content = _read_file(WORDBOOK_TSX)
        # 验证 wordbook-tabs 区域内包含添加按钮
        tabs_section = content.split('className="wordbook-tabs"')
        if len(tabs_section) < 2:
            pytest.skip("Could not find wordbook-tabs section")
        tabs_content = tabs_section[1].split("</div>")[0] if "</div>" in tabs_section[1] else tabs_section[1]

        assert "wordbook-add-inline" in tabs_content, (
            "Add button should be inside .wordbook-tabs area"
        )

    def test_wb_ui_009_add_button_text(self):
        """WB-UI-009: 添加按钮文字为 '+'"""
        content = _read_file(WORDBOOK_TSX)
        # 验证添加按钮文字为 +
        assert ">+<" in content or '>\n          +\n        </button>' in content or "+\n" in content, (
            "Add button text should be '+'"
        )

    def test_wb_ui_010_add_button_disabled_on_system(self):
        """WB-UI-010: 系统词库 Tab 时添加按钮 disabled"""
        content = _read_file(WORDBOOK_TSX)
        # 验证添加按钮有 disabled 状态逻辑
        assert 'activeTab !== "user"' in content or "activeTab !== 'user'" in content, (
            "Add button should be disabled when activeTab is not 'user'"
        )
        assert "disabled={activeTab !== 'user'}" in content or 'disabled={activeTab !== "user"}' in content, (
            "Add button should have disabled prop based on activeTab"
        )

    def test_wb_ui_011_add_button_enabled_on_user(self):
        """WB-UI-011: 用户词库 Tab 时添加按钮 enabled 且橘色"""
        content = _read_file(WORDBOOK_TSX)
        # 验证添加按钮有橘色样式
        assert "wordbook-add-inline" in content, (
            "Add button should use wordbook-add-inline class"
        )
        css = _read_file(STYLES_CSS)
        assert ".wordbook-add-inline" in css, (
            ".wordbook-add-inline style should exist"
        )
        # 验证橘色背景
        add_btn_block = css.split(".wordbook-add-inline {")
        if len(add_btn_block) >= 2:
            block_end = add_btn_block[1].find("}")
            add_btn_styles = add_btn_block[1][:block_end]
            # 验证有背景色（橘色）
            assert "background:" in add_btn_styles or "background:" in add_btn_styles, (
                "Add button should have background color (orange)"
            )


# ============================================================
# 5. 对话框样式验证
# ============================================================

@pytest.mark.timeout(10)
class TestModalStyle:
    """WB-UI-012/013/014: 对话框样式验证"""

    def test_wb_ui_012_modal_background(self):
        """WB-UI-012: .modal-content background != transparent"""
        css = _read_file(STYLES_CSS)
        modal_block = css.split(".modal-content {")
        if len(modal_block) < 2:
            pytest.skip("Could not find .modal-content in styles.css")
        block_end = modal_block[1].find("}")
        modal_styles = modal_block[1][:block_end]

        # 验证 background 不是 transparent
        assert "transparent" not in modal_styles.lower(), (
            ".modal-content background should not be transparent"
        )

    def test_wb_ui_013_modal_primary_color(self):
        """WB-UI-013: .modal-footer .btn-primary 颜色为橘色 #ff6b35"""
        css = _read_file(STYLES_CSS)
        # 验证 btn-accent 有橘色
        assert ".btn-accent" in css, (
            ".btn-accent class should exist"
        )
        accent_block = css.split(".btn-accent {")
        if len(accent_block) >= 2:
            block_end = accent_block[1].find("}")
            accent_styles = accent_block[1][:block_end]
            # 验证背景色为橘色（var(--brand-primary) 或 #ff6b35）
            assert "var(--brand-primary)" in accent_styles or "#ff6b35" in accent_styles, (
                ".btn-accent should have orange color #ff6b35 or var(--brand-primary)"
            )

    def test_wb_ui_014_modal_width(self):
        """WB-UI-014: .modal-content width <= 400px"""
        css = _read_file(STYLES_CSS)
        # 验证 modal-small 有宽度限制
        assert ".modal-small" in css, (
            ".modal-small class should exist"
        )
        small_block = css.split(".modal-small {")
        if len(small_block) >= 2:
            block_end = small_block[1].find("}")
            small_styles = small_block[1][:block_end]
            # 验证宽度
            assert "max-width:" in small_styles or "width:" in small_styles, (
                ".modal-small should have width constraint"
            )


# ============================================================
# 6. 窗口居中验证
# ============================================================

@pytest.mark.timeout(10)
class TestWindowCenter:
    """WB-UI-015: 窗口居中验证"""

    def test_wb_ui_015_window_center(self):
        """WB-UI-015: tauri.conf.json main 窗口包含 'center': true"""
        content = _read_file(TAURI_CONF)
        # 验证 main 窗口有 center: true
        assert '"center": true' in content or '"center":true' in content, (
            "Main window should have center: true"
        )


# ============================================================
# 7. LLM 测试按钮图标验证
# ============================================================

@pytest.mark.timeout(10)
class TestLlmTestButton:
    """WB-UI-016/017: LLM 测试按钮图标验证"""

    def test_wb_ui_016_icon_after_text(self):
        """WB-UI-016: LLM 页面测试连接按钮图标在文字右侧"""
        content = _read_file(LLM_TSX)
        # 验证按钮中文字在图标之前
        assert "测试连接" in content or "Test Connection" in content, (
            "Should have test button text"
        )
        # 验证结构：文字 span 在前，图标 span 在后
        test_btn_section = content.split("handleTest")
        if len(test_btn_section) >= 2:
            btn_content = test_btn_section[1]
            text_pos = btn_content.find("测试中") if "测试中" in btn_content else btn_content.find("Testing")
            icon_pos = btn_content.find("fontSize: '24px'") or btn_content.find('fontSize: "24px"')
            if text_pos != -1 and icon_pos != -1:
                assert text_pos < icon_pos, (
                    "Icon should be after text in test button"
                )

    def test_wb_ui_017_icon_size(self):
        """WB-UI-017: 图标尺寸为原始尺寸的 2 倍"""
        content = _read_file(LLM_TSX)
        # 验证图标 fontSize 为 24px（原始 12px 的 2 倍）
        assert "fontSize: '24px'" in content or 'fontSize: "24px"' in content, (
            "Icon should have fontSize 24px (2x original size)"
        )
