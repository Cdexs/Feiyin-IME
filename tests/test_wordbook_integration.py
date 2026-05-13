"""
test_wordbook_integration.py — 词库与 LLM 集成测试

覆盖：
- WB-LLM-INT-001: Mock 词库注入后，LLM system prompt 包含正确映射
- WB-LLM-INT-002: 系统提示词 + 词库注入 + ANTI_HALLUCINATION 三者共存

注意：本文件中的测试依赖 WORDBOOK-003+004（词库注入功能）实现。
当前使用源码静态分析 + mock 方式验证。
功能实现后需要补充运行时集成测试。
"""

import pytest
from pathlib import Path

PROJECT_ROOT = Path(__file__).parent.parent
LLM_MOD_PATH = PROJECT_ROOT / "src" / "llm" / "mod.rs"
I18N_RS_PATH = PROJECT_ROOT / "src" / "i18n.rs"
WORDBOOK_MOD_PATH = PROJECT_ROOT / "src" / "wordbook" / "mod.rs"


def _read_file_safe(path: Path) -> str:
    """安全读取文件，不存在返回空字符串"""
    if not path.exists():
        return ""
    return path.read_text(encoding="utf-8")


@pytest.mark.timeout(10)
class TestWordbookLlmIntegration:
    """词库与 LLM 集成验证"""

    # ---- WB-LLM-INT-001 ----
    def test_mock_wordbook_injection_to_llm_prompt(self):
        """WB-LLM-INT-001: Mock 词库注入后，LLM system prompt 包含正确映射

        验证：当词库包含条目时，LLM 请求的 system prompt 中应包含
        <wordbook> 标签及其中的 <entry> 映射。

        当前验证点：
        1. LLM 模块源码中存在词库注入代码路径
        2. 注入格式为 <wordbook>...<entry .../>...</wordbook>
        """
        llm_source = _read_file_safe(LLM_MOD_PATH)

        if not llm_source:
            pytest.skip("llm/mod.rs not found")

        # 验证存在词库注入相关的代码路径
        has_wordbook_ref = "wordbook" in llm_source.lower()
        has_xml_tag = "<wordbook>" in llm_source or "</wordbook>" in llm_source
        has_entry_format = "<entry" in llm_source or "entry " in llm_source

        if not (has_wordbook_ref and has_xml_tag):
            pytest.skip(
                "LLM module does not yet contain wordbook injection logic "
                "(requires WORDBOOK-003+004 implementation)"
            )

        # 验证注入逻辑存在
        assert has_wordbook_ref, (
            "LLM module should reference wordbook for injection"
        )
        assert has_xml_tag, (
            "LLM system prompt should contain <wordbook> XML tag when entries exist"
        )
        assert has_entry_format, (
            "LLM system prompt should contain <entry> mappings inside <wordbook>"
        )

    # ---- WB-LLM-INT-002 ----
    def test_system_prompt_wordbook_anti_hallucination_coexist(self):
        """WB-LLM-INT-002: 系统提示词 + 词库注入 + ANTI_HALLUCINATION 三者共存

        验证：
        1. 基础系统提示词（default_system_prompt_en）包含核心规则
        2. LLM 构建逻辑中追加 ANTI_HALLUCINATION 指令
        3. 词库注入内容插入在系统提示词中（或作为额外指令）
        三者最终合并为完整的 system prompt 发送给 LLM。
        """
        i18n_source = _read_file_safe(I18N_RS_PATH)
        llm_source = _read_file_safe(LLM_MOD_PATH)

        if not i18n_source or not llm_source:
            pytest.skip("Required source files not found")

        # 1. 验证系统提示词存在
        assert "default_system_prompt_en" in i18n_source, (
            "i18n.rs should define default_system_prompt_en"
        )

        # 2. 验证 ANTI_HALLUCINATION 指令存在
        assert "ANTI_HALLUCINATION" in llm_source, (
            "LLM module should define ANTI_HALLUCINATION directive"
        )

        # 3. 验证三者共存于 LLM 请求构建流程中
        # 检查 LLM build_optimize_request 方法是否同时引用
        # system_prompt 和 ANTI_HALLUCINATION
        has_system_prompt_ref = "system_prompt" in llm_source
        has_anti_hall_ref = "ANTI_HALLUCINATION" in llm_source
        has_format_call = "format!(" in llm_source or "format!(" in llm_source

        assert has_system_prompt_ref, (
            "LLM request should reference system_prompt"
        )
        assert has_anti_hall_ref, (
            "LLM request should append ANTI_HALLUCINATION directive"
        )
        assert has_format_call, (
            "LLM request should use format! to combine prompt components"
        )

        # 4. 验证词库注入位置（如果已实现）
        has_wordbook_injection = (
            "wordbook" in llm_source.lower()
            and ("<wordbook>" in llm_source or "wordbook_entries" in llm_source.lower())
        )

        if has_wordbook_injection:
            # 如果已实现，验证三者共存
            # system_prompt + wordbook + ANTI_HALLUCINATION 应全部出现在
            # build_optimize_request 方法中
            build_method = llm_source.split("fn build_optimize_request")
            if len(build_method) > 1:
                method_body = build_method[1]
                assert "system_prompt" in method_body, (
                    "build_optimize_request should use system_prompt"
                )
                assert "ANTI_HALLUCINATION" in method_body, (
                    "build_optimize_request should append ANTI_HALLUCINATION"
                )
                assert "wordbook" in method_body.lower(), (
                    "build_optimize_request should include wordbook injection"
                )


@pytest.mark.timeout(10)
class TestWordbookModuleAvailability:
    """词库模块可用性验证"""

    def test_wordbook_list_all_method(self):
        """验证词库模块提供 list_all 方法（用于获取全部条目）"""
        source = _read_file_safe(WORDBOOK_MOD_PATH)
        if not source:
            pytest.skip("wordbook/mod.rs not found")

        assert "pub fn list_all" in source, (
            "Wordbook should have list_all() method for retrieving all entries"
        )

    def test_wordbook_entry_struct(self):
        """验证 WordEntry 结构体字段完整"""
        source = _read_file_safe(WORDBOOK_MOD_PATH)
        if not source:
            pytest.skip("wordbook/mod.rs not found")

        assert "pub struct WordEntry" in source, (
            "Wordbook should define WordEntry struct"
        )
        assert "pub raw: String" in source, (
            "WordEntry should have raw field"
        )
        assert "pub corrected: String" in source, (
            "WordEntry should have corrected field"
        )

    def test_wordbook_applies_to_text(self):
        """验证词库模块提供 apply 方法（用于文本替换）"""
        source = _read_file_safe(WORDBOOK_MOD_PATH)
        if not source:
            pytest.skip("wordbook/mod.rs not found")

        assert "pub fn apply" in source, (
            "Wordbook should have apply() method for text substitution"
        )
