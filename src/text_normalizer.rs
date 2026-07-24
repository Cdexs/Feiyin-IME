use crate::config::ChineseScript;
use zhconv::{zhconv, Variant};

/// LANG-AUTO-001: normalize_text_for_language 现按内容（contains_han）门控简繁转换，
/// 不再依赖 language 配置（语言配置恒为 "auto"）。language 参数已删除。
pub fn normalize_text_for_language(text: &str, script: ChineseScript) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();

    // 中文简繁转换（按内容含汉字判定，不依赖 language 配置）
    if contains_han(&result) {
        let variant = match script {
            ChineseScript::Simplified => Variant::ZhCN,
            ChineseScript::Traditional => Variant::ZhTW,
        };
        result = zhconv(&result, variant);
    }

    // ASR 英文大小写后处理（SenseVoice 输出全大写）
    result = fix_asr_english_case(&result);

    result
}

/// FMT-LLM-005: 仅做中文简繁转换，不做 fix_asr_english_case 大小写后处理。
/// 用于 LLM optimize 成功路径——LLM 输出的大小写是正确意图（如 "Dear Mr. Wang,"），
/// 二次 normalize 会用 fix_asr_english_case 打回小写（"Dear mr. wang,"），破坏 LLM 成果。
/// 与 ASR 原文/LLM 失败兜底路径（仍用 normalize_text_for_language）区分开。
pub fn normalize_script_only(text: &str, script: ChineseScript) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }
    // 仅当内容含汉字时才做简繁转换（contains_han 判定，避免对纯英文文本误转）
    if contains_han(text) {
        let variant = match script {
            ChineseScript::Simplified => Variant::ZhCN,
            ChineseScript::Traditional => Variant::ZhTW,
        };
        return zhconv(text, variant);
    }
    text.to_string()
}

/// LANG-AUTO-001: 内容检测——文本是否含 CJK 汉字。
/// 覆盖：CJK 统一表意文字 (U+4E00-9FFF) + 扩展 A 区 (U+3400-4DBF) + 兼容区 (U+F900-FAFF)。
/// 替代旧的 is_chinese_language(language) 配置门控——语言配置恒为 "auto"，判定靠内容。
pub fn contains_han(text: &str) -> bool {
    text.chars().any(|c| {
        ('\u{4E00}'..='\u{9FFF}').contains(&c)
            || ('\u{3400}'..='\u{4DBF}').contains(&c)
            || ('\u{F900}'..='\u{FAFF}').contains(&c)
    })
}

/// 修复 ASR 输出的英文大小写问题
///
/// SenseVoice 模型输出英文全大写（如 "你好 WORLD"、"HELLO WORLD"），需要规则处理。
///
/// 规则：
/// - 混合模式（含非 ASCII 字符）：英文词全部 lowercase
/// - 纯英文模式：首字母大写，其余 lowercase；独立的 "I" 保持大写
pub fn fix_asr_english_case(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }

    // 判断是否含非 ASCII 字符（中/日/韩/假名/谚文等）
    let has_non_ascii = text.chars().any(|c| !c.is_ascii());

    if has_non_ascii {
        // 混合模式：英文词全部 lowercase，非 ASCII 保持不变
        fix_mixed_text_case(text)
    } else {
        // 纯英文模式：首字母大写 + "I" 保持大写
        fix_pure_english_case(text)
    }
}

/// 混合模式：将英文词（纯 ASCII token）转为 lowercase
fn fix_mixed_text_case(text: &str) -> String {
    let mut result = String::new();
    let mut current_ascii_word = String::new();

    for c in text.chars() {
        if c.is_ascii() {
            current_ascii_word.push(c);
        } else {
            // 遇到非 ASCII 字符，先输出累积的英文词（lowercase）
            if !current_ascii_word.is_empty() {
                result.push_str(&current_ascii_word.to_lowercase());
                current_ascii_word.clear();
            }
            result.push(c);
        }
    }

    // 处理末尾剩余的英文词
    if !current_ascii_word.is_empty() {
        result.push_str(&current_ascii_word.to_lowercase());
    }

    result
}

/// 纯英文模式：首字母大写 + 独立的 "I" 保持大写
fn fix_pure_english_case(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut result = String::new();
    let mut chars = lower.chars().peekable();
    let mut is_word_start = true;
    let mut prev_char = ' ';

    while let Some(c) = chars.next() {
        // 判断是否为独立单词的开头
        let is_boundary = prev_char == ' '
            || prev_char == '.'
            || prev_char == ','
            || prev_char == '?'
            || prev_char == '!'
            || prev_char == '\n';

        if c.is_alphabetic() && is_boundary {
            // 检查是否为独立的 "I"
            let next_char = chars.peek().copied();
            let is_standalone_i = c == 'i'
                && (next_char.is_none() || next_char.map_or(false, |n| !n.is_alphabetic()));

            if is_standalone_i {
                result.push('I');
            } else if is_word_start {
                // 首字母或句首大写
                result.push(c.to_ascii_uppercase());
                is_word_start = false;
            } else {
                result.push(c);
            }
        } else {
            result.push(c);
            if c.is_alphabetic() {
                is_word_start = false;
            }
        }

        prev_char = c;
    }

    result
}

/// LANG-AUTO-001: script_instruction 改按内容（contains_han）门控。
/// 旧签名 (language, script) 改为 (text, script)，language 参数被 text 替代。
pub fn script_instruction(text: &str, script: ChineseScript) -> Option<&'static str> {
    if !contains_han(text) {
        return None;
    }

    Some(match script {
        ChineseScript::Simplified => "请将最终输出转换为简体中文（中国大陆简体字）。",
        ChineseScript::Traditional => "请将最终输出转换为繁体中文（台湾正体字）。",
    })
}

/// OPT-002: Check if text contains effective content (not empty or filler-only).
///
/// Returns false for:
/// - Empty/whitespace-only text
/// - Text containing only filler words (啊呃嗯哦噢那个就是)
///
/// Returns true for text with >= 2 meaningful characters after filler removal.
pub fn is_effective_text(text: &str) -> bool {
    let stripped = text.trim();
    if stripped.is_empty() {
        return false;
    }

    // Remove common Chinese filler words
    const FILLERS: &[&str] = &[
        "啊", "呃", "嗯", "哦", "噢", "那个", "就是", "然后", "所以", "但是",
    ];
    let mut cleaned = stripped.to_string();
    for filler in FILLERS {
        cleaned = cleaned.replace(filler, "");
    }

    // Require at least 2 meaningful characters
    cleaned.trim().chars().count() >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // normalize_text_for_language（LANG-AUTO-001：按内容 contains_han 门控）
    // ============================================================

    #[test]
    fn normalizes_to_simplified_chinese() {
        // 含汉字 → 简繁转换生效（不依赖 language 参数）
        let text = normalize_text_for_language("阿拉伯聯合酋長國", ChineseScript::Simplified);
        assert_eq!(text, "阿拉伯联合酋长国");
    }

    #[test]
    fn normalizes_to_traditional_chinese() {
        let text = normalize_text_for_language("阿拉伯联合酋长国", ChineseScript::Traditional);
        assert_eq!(text, "阿拉伯聯合酋長國");
    }

    #[test]
    fn leaves_non_chinese_content_unchanged() {
        // LANG-AUTO-001: 纯英文文本不含汉字 → 不做简繁转换（但仍做 fix_asr_english_case 大小写后处理）
        let text = normalize_text_for_language("HELLO WORLD", ChineseScript::Simplified);
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn mixed_chinese_english_normalizes_script_only() {
        // 含汉字 → 简繁转换；英文混合词经 fix_asr_english_case → lowercase
        let text = normalize_text_for_language("你好 WORLD", ChineseScript::Simplified);
        assert_eq!(text, "你好 world");
    }

    // ============================================================
    // normalize_script_only（FMT-LLM-005：仅简繁，不动大小写）
    // ============================================================

    #[test]
    fn normalize_script_only_preserves_english_case() {
        // 含汉字 → 简繁转换；英文大小写保持不变（关键：LLM 成功路径保护）
        let text = normalize_script_only("Dear Mr. Wang, 你好", ChineseScript::Simplified);
        assert_eq!(text, "Dear Mr. Wang, 你好");
    }

    #[test]
    fn normalize_script_only_traditional() {
        // zhconv ZhTW: "台湾" → "臺灣"；英文大小写保持不变
        let text = normalize_script_only("台湾 World", ChineseScript::Traditional);
        assert_eq!(text, "臺灣 World");
    }

    #[test]
    fn normalize_script_only_pure_english_unchanged() {
        // 纯英文不含汉字 → 原样返回
        let text = normalize_script_only("Dear Mr. Wang,", ChineseScript::Simplified);
        assert_eq!(text, "Dear Mr. Wang,");
    }

    // ============================================================
    // contains_han（LANG-AUTO-001：内容检测）
    // ============================================================

    #[test]
    fn contains_han_basic_chinese() {
        assert!(contains_han("你好"));
        assert!(contains_han("hello 你好"));
        assert!(contains_han("日本語"));
    }

    #[test]
    fn contains_han_pure_english_false() {
        assert!(!contains_han("hello world"));
        assert!(!contains_han("HELLO 123"));
    }

    #[test]
    fn contains_han_empty() {
        assert!(!contains_han(""));
        assert!(!contains_han("   "));
    }

    #[test]
    fn contains_han_extension_a() {
        // CJK 扩展 A 区字符（U+3400-U+4DBF）也应命中
        assert!(contains_han("㐀㐁"));
    }

    // ============================================================
    // script_instruction（LANG-AUTO-001：按内容门控）
    // ============================================================

    #[test]
    fn script_instruction_chinese_content_returns_instruction() {
        let instr = script_instruction("你好", ChineseScript::Simplified);
        assert!(instr.is_some());
        assert!(instr.unwrap().contains("简体中文"));
    }

    #[test]
    fn script_instruction_pure_english_returns_none() {
        let instr = script_instruction("hello world", ChineseScript::Simplified);
        assert!(
            instr.is_none(),
            "pure English should not get script instruction"
        );
    }

    #[test]
    fn script_instruction_traditional() {
        let instr = script_instruction("你好", ChineseScript::Traditional);
        assert!(instr.unwrap().contains("繁体中文"));
    }

    // ============================================================
    // ASR 英文大小写测试（fix_asr_english_case，回归）
    // ============================================================
    #[test]
    fn fix_mixed_chinese_english() {
        assert_eq!(fix_asr_english_case("你好 WORLD"), "你好 world");
    }

    #[test]
    fn fix_pure_english_with_i() {
        assert_eq!(fix_asr_english_case("HELLO I AM HERE"), "Hello I am here");
    }

    #[test]
    fn fix_pure_english_simple() {
        assert_eq!(fix_asr_english_case("HELLO WORLD"), "Hello world");
    }

    #[test]
    fn fix_mixed_korean_english() {
        assert_eq!(fix_asr_english_case("안녕 HELLO"), "안녕 hello");
    }

    #[test]
    fn fix_mixed_japanese_english() {
        assert_eq!(fix_asr_english_case("こんにちは HELLO"), "こんにちは hello");
    }

    #[test]
    fn fix_pure_english_sentence_end() {
        assert_eq!(
            fix_asr_english_case("HELLO WORLD I AM HERE."),
            "Hello world I am here."
        );
    }

    #[test]
    fn fix_empty_string() {
        assert_eq!(fix_asr_english_case(""), "");
    }

    #[test]
    fn fix_only_chinese() {
        assert_eq!(fix_asr_english_case("你好世界"), "你好世界");
    }

    #[test]
    fn fix_mixed_with_punctuation() {
        assert_eq!(
            fix_asr_english_case("你好 WORLD，HELLO"),
            "你好 world，hello"
        );
    }

    // OPT-002: is_effective_text tests
    #[test]
    fn effective_text_normal() {
        assert!(is_effective_text("你好世界"));
        assert!(is_effective_text("今天天气很好"));
    }

    #[test]
    fn effective_text_empty() {
        assert!(!is_effective_text(""));
        assert!(!is_effective_text("   "));
    }

    #[test]
    fn effective_text_filler_only() {
        assert!(!is_effective_text("啊"));
        assert!(!is_effective_text("呃嗯"));
        assert!(!is_effective_text("那个就是"));
    }

    #[test]
    fn effective_text_with_filler() {
        // Contains filler but also meaningful content
        assert!(is_effective_text("你好啊"));
        assert!(is_effective_text("那个嗯今天天气很好"));
    }

    #[test]
    fn effective_text_single_char() {
        // Single meaningful char is not enough (need >= 2)
        assert!(!is_effective_text("好"));
    }

    #[test]
    fn effective_text_two_chars() {
        assert!(is_effective_text("你好"));
    }
}
