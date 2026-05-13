use crate::config::ChineseScript;
use zhconv::{zhconv, Variant};

pub fn normalize_text_for_language(text: &str, language: &str, script: ChineseScript) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();

    // 中文简繁转换
    if is_chinese_language(language) {
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

pub fn script_instruction(language: &str, script: ChineseScript) -> Option<&'static str> {
    if !is_chinese_language(language) {
        return None;
    }

    Some(match script {
        ChineseScript::Simplified => "请将最终输出转换为简体中文（中国大陆简体字）。",
        ChineseScript::Traditional => "请将最终输出转换为繁体中文（台湾正体字）。",
    })
}

fn is_chinese_language(language: &str) -> bool {
    language.trim().eq_ignore_ascii_case("zh") || language.trim().starts_with("zh-")
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

    #[test]
    fn normalizes_to_simplified_chinese() {
        let text = normalize_text_for_language("阿拉伯聯合酋長國", "zh", ChineseScript::Simplified);
        assert_eq!(text, "阿拉伯联合酋长国");
    }

    #[test]
    fn normalizes_to_traditional_chinese() {
        let text =
            normalize_text_for_language("阿拉伯联合酋长国", "zh", ChineseScript::Traditional);
        assert_eq!(text, "阿拉伯聯合酋長國");
    }

    #[test]
    fn leaves_non_chinese_language_unchanged() {
        let text = normalize_text_for_language("阿拉伯聯合酋長國", "en", ChineseScript::Simplified);
        assert_eq!(text, "阿拉伯聯合酋長國");
    }

    // ASR 英文大小写测试
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
