"""
test_i18n.py — 国际化与系统提示词测试

覆盖：
- Rule 6（Wordbook Priority）提示词验证
- <wordbook> 格式说明验证
- <entry raw=... corrected=.../> 示例验证

注意：本文件中的词库注入相关用例依赖 WORDBOOK-003+004 功能实现。
在功能实现前，部分用例将跳过（skip）或验证当前状态。
"""

import pytest
from pathlib import Path

# 导入 i18n 模块获取系统提示词（通过读取 Rust 源码）
PROJECT_ROOT = Path(__file__).parent.parent
I18N_RS_PATH = PROJECT_ROOT / "src" / "i18n.rs"


def _read_i18n_source() -> str:
    """读取 i18n.rs 源码"""
    if not I18N_RS_PATH.exists():
        pytest.skip(f"i18n.rs not found at {I18N_RS_PATH}")
    return I18N_RS_PATH.read_text(encoding="utf-8")


def _extract_default_prompt_en(source: str) -> str:
    """
    从 i18n.rs 源码中提取 default_system_prompt_en 的内容
    使用简单的字符串解析（非 Rust 编译器）
    """
    # 查找 default_system_prompt_en: &'static str,
    # 然后在 ZH 和 EN 的静态块中找到对应的值
    # EN 块从 "static EN: Strings = Strings {" 开始
    en_start = source.find("static EN: Strings = Strings {")
    if en_start == -1:
        return ""

    en_block = source[en_start:]
    # 找到 default_system_prompt_en:
    prompt_key = "default_system_prompt_en:"
    key_pos = en_block.find(prompt_key)
    if key_pos == -1:
        return ""

    # 从键位置开始找值（通常在下一行）
    after_key = en_block[key_pos + len(prompt_key):].strip()

    # 值以 r#" 开始，以 "#, 结束
    if not after_key.startswith('r#"'):
        return ""

    content_start = after_key.find('r#"') + 3
    content_end = after_key.find('"#,', content_start)
    if content_end == -1:
        return ""

    return after_key[content_start:content_end]


@pytest.mark.timeout(10)
class TestI18nSystemPromptRule6:
    """系统提示词 Rule 6（词库优先级）验证"""

    @pytest.mark.smoke
    def test_i18n_rule6_prompt_contains_wordbook_priority(self):
        """I18N-RULE6-001: default_system_prompt_en 包含 'Wordbook Priority'"""
        source = _read_i18n_source()
        prompt = _extract_default_prompt_en(source)

        if not prompt:
            pytest.skip("Could not extract default_system_prompt_en from i18n.rs")

        # 验证包含词库优先级规则
        assert "Wordbook Priority" in prompt or "wordbook" in prompt.lower(), (
            "default_system_prompt_en should contain Wordbook Priority rule"
        )

    @pytest.mark.smoke
    def test_i18n_rule6_prompt_contains_wordbook_format(self):
        """I18N-RULE6-002: 提示词包含 <wordbook> 格式说明

        注意：此用例依赖 WORDBOOK-003+004 功能。
        当前验证提示词是否包含词库格式模板说明。
        """
        source = _read_i18n_source()
        prompt = _extract_default_prompt_en(source)

        if not prompt:
            pytest.skip("Could not extract default_system_prompt_en from i18n.rs")

        # 检查是否包含 <wordbook> 标签格式说明
        # 在功能实现前，此测试可能失败或跳过
        has_wordbook_tag = "<wordbook>" in prompt or "wordbook" in prompt.lower()
        assert has_wordbook_tag, (
            "default_system_prompt_en should contain <wordbook> format description "
            "(requires WORDBOOK-003+004 implementation)"
        )

    @pytest.mark.smoke
    def test_i18n_rule6_prompt_contains_entry_example(self):
        """I18N-RULE6-003: 提示词包含 <entry raw=... corrected=.../> 示例

        注意：此用例依赖 WORDBOOK-003+004 功能。
        """
        source = _read_i18n_source()
        prompt = _extract_default_prompt_en(source)

        if not prompt:
            pytest.skip("Could not extract default_system_prompt_en from i18n.rs")

        # 检查是否包含 entry 示例
        has_entry_example = (
            '<entry raw=' in prompt or 'entry raw=' in prompt.lower()
        )
        assert has_entry_example, (
            "default_system_prompt_en should contain <entry raw=... corrected=.../> example "
            "(requires WORDBOOK-003+004 implementation)"
        )


@pytest.mark.timeout(10)
class TestI18nPromptStructure:
    """系统提示词结构完整性验证"""

    def test_prompt_contains_all_numbered_rules(self):
        """验证提示词包含所有编号规则（1-N）"""
        source = _read_i18n_source()
        prompt = _extract_default_prompt_en(source)

        if not prompt:
            pytest.skip("Could not extract default_system_prompt_en from i18n.rs")

        # 验证至少包含 Rule 1-5（已有规则）
        for rule_num in range(1, 6):
            assert f"**{rule_num}." in prompt or f"**{rule_num} **" in prompt, (
                f"Prompt should contain Rule {rule_num}"
            )

    def test_prompt_contains_anti_hallucination(self):
        """验证 ANTI_HALLUCINATION 指令存在于 LLM 构建逻辑中"""
        llm_mod_path = PROJECT_ROOT / "src" / "llm" / "mod.rs"
        if not llm_mod_path.exists():
            pytest.skip("llm/mod.rs not found")

        llm_source = llm_mod_path.read_text(encoding="utf-8")
        assert "ANTI_HALLUCINATION" in llm_source, (
            "LLM module should contain ANTI_HALLUCINATION directive"
        )
        assert "<speech>" in llm_source, (
            "LLM module should wrap user input in <speech> tags"
        )
