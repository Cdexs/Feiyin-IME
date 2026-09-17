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
    // LANG-MIXED-001: 含假名/谚文时跳过 zhconv——日文汉字与中文汉字同码区，
    // zhconv 会把日文汉字当繁体字形转简体（圖→图）污染日文内容。
    if contains_han(&result) && !contains_kana(&result) && !contains_hangul(&result) {
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
    // LANG-MIXED-001: 含假名/谚文时跳过 zhconv——日文汉字会被当繁体转简体污染。
    if contains_han(text) && !contains_kana(text) && !contains_hangul(text) {
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
/// 注意：日文汉字与中文汉字同处 CJK 统一表意区，本函数对日文文本也返回 true。
/// 如需区分中日/中韩夹杂，请配合 contains_kana / contains_hangul 使用。
pub fn contains_han(text: &str) -> bool {
    text.chars().any(|c| {
        ('\u{4E00}'..='\u{9FFF}').contains(&c)
            || ('\u{3400}'..='\u{4DBF}').contains(&c)
            || ('\u{F900}'..='\u{FAFF}').contains(&c)
    })
}

/// LANG-MIXED-001: 内容检测——文本是否含日文假名。
/// 平假名 U+3040-309F + 片假名 U+30A0-30FF。
/// 假名是日文独有字符（中文/韩文不含），可作为日文存在的可靠判据。
pub fn contains_kana(text: &str) -> bool {
    text.chars()
        .any(|c| ('\u{3040}'..='\u{309F}').contains(&c) || ('\u{30A0}'..='\u{30FF}').contains(&c))
}

/// LANG-MIXED-001: 内容检测——文本是否含韩文谚文。
/// 谚文音节 U+AC00-D7AF + 谚文字母 U+1100-11FF + 谚文兼容字母 U+3130-318F。
/// 谚文是韩文独有字符，可作为韩文存在的可靠判据。
pub fn contains_hangul(text: &str) -> bool {
    text.chars().any(|c| {
        ('\u{AC00}'..='\u{D7AF}').contains(&c)
            || ('\u{1100}'..='\u{11FF}').contains(&c)
            || ('\u{3130}'..='\u{318F}').contains(&c)
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

/// LANG-AUTO-001 + LANG-MIXED-001: script_instruction 改按内容门控。
/// 旧签名 (language, script) 改为 (text, script)，language 参数被 text 替代。
///
/// **用于 optimize（非翻译）路径**。翻译路径请用 [`script_instruction_for_translate`]。
///
/// LANG-MIXED-001 修正（防日韩文被译成中文）：
/// - 不含汉字 → None（无中文，原行为）。
/// - 含汉字且不含假名/谚文（纯中文或中英混合）→ 发字形统一指令 + 「不要翻译非中文内容」。
///   措辞收紧明令不翻译，兜底纯汉字无假名的日文短句（如 東京会議，探针失效靠措辞兜底）。
/// - 含假名或谚文（日韩或中日/中韩混合）→ 发纯保护措辞，**不含中文简繁字样**，
///   语义只保留一层「保留原文形态、不要翻译成其他语言」。去中文简繁字样避免 LLM 误解为
///   要把日韩文转成中文；保留防翻译护栏（本 bug 核心诉求，纯日文输入同样需要）。
/// - 繁体分支对称处理。
pub fn script_instruction(text: &str, script: ChineseScript) -> Option<&'static str> {
    if !contains_han(text) {
        return None;
    }

    // LANG-MIXED-001: 含假名/谚文 → 纯保护措辞（不含中文简繁字样，只保留防翻译语义）
    if contains_kana(text) || contains_hangul(text) {
        return Some("Preserve the original form of every language in the input. Do NOT translate any content into another language.");
    }

    Some(match script {
        ChineseScript::Simplified => "Normalize the Chinese parts of the output to Simplified Chinese (Mainland China standard glyphs). Do NOT translate any non-Chinese content — English, Japanese and Korean text MUST be preserved exactly as spoken.",
        ChineseScript::Traditional => "Normalize the Chinese parts of the output to Traditional Chinese (Taiwan standard glyphs). Do NOT translate any non-Chinese content — English, Japanese and Korean text MUST be preserved exactly as spoken.",
    })
}

/// LANG-MIXED-001: 翻译路径专用 script_instruction。
///
/// **关键约束（主控追加）**：翻译路径绝不可注入「不要翻译」语义——否则用户按翻译热键时
/// 会被自己的指令阻断，打回翻译功能（回归）。本函数只保留字形约束：
/// - 含假名或谚文 → None（不发任何指令，避免任何字形/翻译相关措辞干扰翻译功能）。
/// - 含汉字且不含假名/谚文 → 只发字形统一指令（不含「不要翻译」）。
/// - 不含汉字 → None。
pub fn script_instruction_for_translate(text: &str, script: ChineseScript) -> Option<&'static str> {
    if !contains_han(text) {
        return None;
    }

    // LANG-MIXED-001: 含假名/谚文 → 不发任何指令，避免干扰翻译功能
    if contains_kana(text) || contains_hangul(text) {
        return None;
    }

    Some(match script {
        ChineseScript::Simplified => "Normalize the Chinese parts of the output to Simplified Chinese (Mainland China standard glyphs).",
        ChineseScript::Traditional => "Normalize the Chinese parts of the output to Traditional Chinese (Taiwan standard glyphs).",
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
        assert!(instr.unwrap().contains("Simplified Chinese"));
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
        assert!(instr.unwrap().contains("Traditional Chinese"));
    }

    // ============================================================
    // LANG-MIXED-001: contains_kana / contains_hangul 探针
    // ============================================================

    #[test]
    fn contains_kana_pure_japanese() {
        assert!(contains_kana("こんにちは")); // 平假名
        assert!(contains_kana("カタカナ")); // 片假名
        assert!(contains_kana("日本語テスト"));
    }

    #[test]
    fn contains_kana_mixed() {
        assert!(contains_kana("你好 こんにちは")); // 中日混合
        assert!(contains_kana("hello ありがとう"));
    }

    #[test]
    fn contains_kana_pure_chinese_false() {
        assert!(!contains_kana("你好世界"));
        assert!(!contains_kana("你好 world"));
    }

    #[test]
    fn contains_kana_pure_korean_false() {
        // 韩文不含假名
        assert!(!contains_kana("안녕하세요"));
    }

    #[test]
    fn contains_kana_empty() {
        assert!(!contains_kana(""));
        assert!(!contains_kana("   "));
    }

    #[test]
    fn contains_hangul_pure_korean() {
        assert!(contains_hangul("안녕하세요")); // 谚文音节
        assert!(contains_hangul("감사합니다"));
    }

    #[test]
    fn contains_hangul_mixed() {
        assert!(contains_hangul("你好 안녕")); // 中韩混合
        assert!(contains_hangul("hello 감사"));
    }

    #[test]
    fn contains_hangul_pure_chinese_false() {
        assert!(!contains_hangul("你好世界"));
        assert!(!contains_hangul("你好 world"));
    }

    #[test]
    fn contains_hangul_pure_japanese_false() {
        // 日文假名不是谚文（注意：日文汉字不算假名也不算谚文）
        assert!(!contains_hangul("こんにちは"));
        assert!(!contains_hangul("カタカナ"));
    }

    #[test]
    fn contains_hangul_empty() {
        assert!(!contains_hangul(""));
        assert!(!contains_hangul("   "));
    }

    // ============================================================
    // LANG-MIXED-001: script_instruction 六类覆盖（optimize 非翻译路径）
    // ============================================================

    #[test]
    fn lang_mixed_sino_japanese_returns_protection_only() {
        // 中日混合：含假名 → 纯保护措辞，不含中文简繁字样
        let instr = script_instruction("你好 こんにちは", ChineseScript::Simplified);
        assert!(
            instr.is_some(),
            "mixed CN-JP should get protection instruction"
        );
        let s = instr.unwrap();
        assert!(
            !s.contains("Simplified Chinese"),
            "protection must not mention Simplified Chinese"
        );
        assert!(
            !s.contains("Traditional Chinese"),
            "protection must not mention Traditional Chinese"
        );
        assert!(
            s.contains("Do NOT translate"),
            "protection must retain do-not-translate guard"
        );
    }

    #[test]
    fn lang_mixed_sino_korean_returns_protection_only() {
        // 中韩混合：含谚文 → 纯保护措辞
        let instr = script_instruction("你好 안녕하세요", ChineseScript::Simplified);
        assert!(instr.is_some());
        let s = instr.unwrap();
        assert!(!s.contains("Simplified Chinese"));
        assert!(!s.contains("Traditional Chinese"));
        assert!(s.contains("Do NOT translate"));
    }

    #[test]
    fn lang_mixed_sino_english_keeps_script_instruction() {
        // 中英混合：无假名无谚文 → 字形统一 + 不要翻译
        let instr = script_instruction("你好 world", ChineseScript::Simplified);
        assert!(instr.is_some());
        let s = instr.unwrap();
        assert!(s.contains("Simplified Chinese"));
        assert!(s.contains("Do NOT translate any non-Chinese content"));
    }

    #[test]
    fn lang_mixed_pure_japanese_returns_protection_only() {
        // 纯日语：含假名也含汉字（漢字在 CJK 区）→ 纯保护措辞
        let instr = script_instruction("日本語テスト", ChineseScript::Simplified);
        assert!(instr.is_some(), "pure JP with kanji should get protection");
        let s = instr.unwrap();
        assert!(!s.contains("Simplified Chinese"));
        assert!(s.contains("Do NOT translate"));
    }

    #[test]
    fn lang_mixed_pure_korean_returns_protection_only() {
        // 纯韩语：含谚文，无汉字 → contains_han 为 false → None
        // （韩文无汉字，contains_han 返回 false，提前返回 None，不进假名/谚文分支）
        let instr = script_instruction("안녕하세요", ChineseScript::Simplified);
        assert!(instr.is_none(), "pure Korean without hanzi returns None");
    }

    #[test]
    fn lang_mixed_pure_chinese_simplified_keeps_instruction() {
        // 纯中文简体：无假名无谚文 → 字形统一 + 不要翻译
        let instr = script_instruction("你好世界", ChineseScript::Simplified);
        assert!(instr.is_some());
        assert!(instr.unwrap().contains("Simplified Chinese"));
    }

    #[test]
    fn lang_mixed_pure_chinese_traditional_keeps_instruction() {
        // 纯中文繁体：繁体分支对称
        let instr = script_instruction("你好世界", ChineseScript::Traditional);
        assert!(instr.is_some());
        let s = instr.unwrap();
        assert!(s.contains("Traditional Chinese"));
        assert!(s.contains("Do NOT translate any non-Chinese content"));
    }

    // ============================================================
    // LANG-MIXED-001: normalize 路径跳过 zhconv（日韩文不污染）
    // ============================================================

    #[test]
    fn normalize_script_only_skips_zhconv_for_japanese() {
        // 含假名 → 跳过 zhconv，日文汉字保持原样（不当繁体转简体）。
        // 注：必须用含假名的日文——纯汉字日文（如 東京会議）探针失效，属已知残留。
        let text = normalize_script_only("会議は終わりました", ChineseScript::Simplified);
        assert!(
            text.contains("会議"),
            "JP kanji with kana must not be converted"
        );
        assert!(text.contains("は"));
    }

    #[test]
    fn normalize_script_only_skips_zhconv_for_korean() {
        let text = normalize_script_only("안녕하세요 世界", ChineseScript::Simplified);
        // 含谚文 → 跳过 zhconv；世界 无繁简差异，保持原样
        assert!(text.contains("안녕하세요"));
        assert!(text.contains("世界"));
    }

    #[test]
    fn normalize_text_for_language_skips_zhconv_for_japanese() {
        // LLM 失败兜底路径同样跳过 zhconv（补强1）。
        // 注：必须用含假名的日文——纯汉字日文（如 東京会議）探针失效，属已知残留
        // （任务书预警：靠 script_instruction 新措辞的「不要翻译」兜底，不引入语言模型判定）。
        let text =
            normalize_text_for_language("日本語テスト hello WORLD", ChineseScript::Simplified);
        assert!(text.contains("テスト"), "JP kana must not be touched");
        assert!(text.contains("hello"), "english case fix still applies");
    }

    #[test]
    fn normalize_text_for_language_skips_zhconv_for_korean() {
        let text = normalize_text_for_language("안녕하세요 世界", ChineseScript::Simplified);
        assert!(text.contains("안녕하세요"));
    }

    #[test]
    fn normalize_script_only_chinese_still_converts() {
        // 纯中文（无假名谚文）→ zhconv 仍生效，回归保护
        let text = normalize_script_only("阿拉伯聯合酋長國", ChineseScript::Simplified);
        assert_eq!(text, "阿拉伯联合酋长国");
    }

    // ============================================================
    // P1-TEST-SYNC-20260727: normalize 函数对含假名/谚文文本零改变断言
    // 日文汉字不得被 zhconv 简繁转换（龍→龙、亞→亚 等）
    // ============================================================

    #[test]
    fn normalize_script_only_keeps_japanese_kanji_unchanged() {
        // 含假名 → 跳过 zhconv，日文汉字龍/亞 不得被简体化
        let text = normalize_script_only("龍が好き", ChineseScript::Simplified);
        assert!(
            text.contains("龍"),
            "JP kanji 龍 must not be simplified to 龙 when kana is present"
        );
        assert!(text.contains("が"));
        let text2 = normalize_script_only("亞洲の祭り", ChineseScript::Simplified);
        assert!(
            text2.contains("亞"),
            "JP kanji 亞 must not be simplified to 亚 when kana is present"
        );
    }

    #[test]
    fn normalize_text_for_language_keeps_japanese_kanji_unchanged() {
        // normalize_text_for_language 同样跳过日文 zhconv
        let text = normalize_text_for_language("龍が好き", ChineseScript::Simplified);
        assert!(
            text.contains("龍"),
            "normalize_text_for_language must keep 龍 when kana is present"
        );
        assert!(text.contains("が"));
    }

    // ============================================================
    // LANG-MIXED-001: script_instruction_for_translate 翻译路径回归护栏
    // 【主控强制验收】断言翻译链路拿到的指令不含「不要翻译」语义
    // ============================================================

    #[test]
    fn translate_path_instruction_no_kana_no_hangul_only_script() {
        // 纯中文：翻译路径只发字形约束，不含「不要翻译」
        let instr = script_instruction_for_translate("你好世界", ChineseScript::Simplified);
        assert!(instr.is_some());
        let s = instr.unwrap();
        assert!(s.contains("Simplified Chinese"));
        assert!(
            !s.contains("Do NOT translate"),
            "translate path must NOT contain do-not-translate"
        );
        assert!(
            !s.contains("Preserve the original form"),
            "translate path must NOT contain protection-only wording"
        );
    }

    #[test]
    fn translate_path_instruction_no_kana_no_hangul_traditional() {
        let instr = script_instruction_for_translate("你好世界", ChineseScript::Traditional);
        assert!(instr.is_some());
        let s = instr.unwrap();
        assert!(s.contains("Traditional Chinese"));
        assert!(!s.contains("Do NOT translate"));
    }

    #[test]
    fn translate_path_instruction_with_kana_returns_none() {
        // 中日混合：含假名 → 翻译路径返回 None（避免任何措辞干扰翻译功能）
        let instr = script_instruction_for_translate("你好 こんにちは", ChineseScript::Simplified);
        assert!(instr.is_none(), "translate path with kana must return None");
    }

    #[test]
    fn translate_path_instruction_with_hangul_returns_none() {
        let instr = script_instruction_for_translate("你好 안녕하세요", ChineseScript::Simplified);
        assert!(
            instr.is_none(),
            "translate path with hangul must return None"
        );
    }

    #[test]
    fn translate_path_instruction_pure_english_returns_none() {
        let instr = script_instruction_for_translate("hello world", ChineseScript::Simplified);
        assert!(instr.is_none());
    }

    /// TEST-SYNC-008 0b 新守卫①：翻译路径不得含 Do NOT translate。
    /// ⚠️ LANG-MIXED-001 的既有硬要求：翻译路径注入「不要翻译」会阻断翻译功能
    /// （用户按翻译热键时被自己的指令打回）——删掉会让翻译热键失效，属回归。
    /// 显式锁定：translate 路径的简繁两条指令串都不得含 Do NOT translate。
    #[test]
    fn translate_path_never_contains_do_not_translate() {
        for script in [ChineseScript::Simplified, ChineseScript::Traditional] {
            let s = script_instruction_for_translate("你好世界", script)
                .expect("translate path should give instruction");
            assert!(
                !s.contains("Do NOT translate"),
                "translate path must never contain Do NOT translate (LANG-MIXED-001)"
            );
        }
    }

    /// TEST-SYNC-008 0b 新守卫②：5 条指令串不得出现中日韩字符（Gavin 2026-08-01：系统提示应纯英文）。
    /// 覆盖两条 optimize + 两条 translate + 一条保护措辞，共 5 个 &'static str。
    /// 注释里的中文不算（仅测返回的指令串本身）。将来有人往指令串里加中文会撞红。
    #[test]
    fn script_instructions_are_pure_english_no_cjk() {
        use crate::text_normalizer::contains_kana;
        let cases: Vec<&str> = vec![
            script_instruction("你好", ChineseScript::Simplified).unwrap(),
            script_instruction("你好", ChineseScript::Traditional).unwrap(),
            script_instruction("你好 こんにちは", ChineseScript::Simplified).unwrap(), // 保护措辞
            script_instruction_for_translate("你好世界", ChineseScript::Simplified).unwrap(),
            script_instruction_for_translate("你好世界", ChineseScript::Traditional).unwrap(),
        ];
        for s in cases {
            assert!(
                !contains_kana(s) && !contains_hangul(s),
                "指令串必须纯英文：{:?}",
                s
            );
            assert!(
                !s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
                "指令串不得含汉字：{:?}",
                s
            );
        }
    }

    #[test]
    fn translate_path_instruction_no_kana_no_hangul_mixed_cn_en() {
        // 中英混合：翻译路径只字形约束
        let instr = script_instruction_for_translate("你好 world", ChineseScript::Simplified);
        assert!(instr.is_some());
        assert!(!instr.unwrap().contains("Do NOT translate"));
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

/// V091-ITN-HANT-SYSTEMIC-221 · **根本护栏**：影子串前提 —— 繁→简逐字归一与原串一一对应。
///
/// 221 路线②′（`itn.rs::normalize_with_rules` 逐字构造影子串、判定查影子串）成立的全部前提是
/// 「影子串与原串字符 1:1」。若该前提被打破（zhconv 升级 / 换变体 / 表变更），
/// 影子串下标与原串下标将错位 → ITN 会静默切错字符甚至越界。
///
/// 本模块由 221A 取证单转为常驻护栏（主控 2026-09-17 裁定，**不得删除**）：
/// - `shadow_len_zhhans` / `shadow_per_char_1to1_zhhans`：样本级长度与逐字对齐；
/// - `shadow_global_single_char_1to1_scan`：**全域 28,096 字**单字必得单字（决定性证据）；
/// - `shadow_phrase_corpus_len_preserved`：词组级长度守恒（zhconv ZhHans 含 OpenCC 词组规则的残余面）；
/// - `shadow_contrast_region_variants`：反面对照，证「必须用 ZhHans 而非地区变体 ZhCN/ZhTW」。
///
/// 🔴 变体纪律：本护栏断言的是 `Variant::ZhHans`（脚本变体，无地区词汇替换）。
/// 若有人把 221 实现改用 `ZhCN`/`ZhTW`，对照用例会立刻暴露地区词替换差异。
#[cfg(test)]
mod itn_hant_shadow_1to1_guard {
    use zhconv::{zhconv, Variant};

    /// 取样：前 10 条为 ITN 核心用例（金额/时间/单位），后 10 条为高危词
    /// （多对一字 髮/麵/隻 等 + 台湾地区词 電腦/網路/軟體，专门逼出长度变化）。
    const SAMPLES: &[&str] = &[
        "那一刻",
        "這一刻",
        "這麼一點",
        "三月五號",
        "五萬塊",
        "三塊錢",
        "三點半",
        "十歲",
        "三個",
        "兩萬五",
        "頭髮",
        "麵條",
        "一隻貓",
        "乾淨",
        "後面",
        "計算機",
        "網路",
        "軟體",
        "硬碟",
        "資訊",
    ];

    fn counts(s: &str, v: Variant) -> (String, usize, usize) {
        let shadow = zhconv(s, v);
        (shadow.clone(), s.chars().count(), shadow.chars().count())
    }

    /// Q2 主测：ZhHans（字形变体）逐条长度对照。不等长即失败并列出全部反例。
    #[test]
    fn shadow_len_zhhans() {
        let mut table = String::new();
        let mut bad: Vec<String> = Vec::new();
        for s in SAMPLES {
            let (shadow, a, b) = counts(s, Variant::ZhHans);
            table.push_str(&format!("  {s} -> {shadow}   (原 {a} / 影 {b})\n"));
            if a != b {
                bad.push(format!("{s} -> {shadow} (原 {a} / 影 {b})"));
            }
        }
        println!("[HANT-SHADOW] ZhHans 长度对照：\n{table}");
        assert!(
            bad.is_empty(),
            "[HANT-SHADOW] ZhHans 出现长度不等（影子串前提被打破）：\n{}",
            bad.join("\n")
        );
    }

    /// Q2 强测：证明「逐字 1:1」—— 单字归一必得单字，且整串归一 == 逐字拼接。
    /// 比长度相等更强：长度相等仍可能是 2→1 + 1→2 的对冲。
    #[test]
    fn shadow_per_char_1to1_zhhans() {
        let mut bad: Vec<String> = Vec::new();
        for s in SAMPLES {
            let whole = zhconv(s, Variant::ZhHans);
            let mut concat = String::new();
            for c in s.chars() {
                let one = zhconv(&c.to_string(), Variant::ZhHans);
                if one.chars().count() != 1 {
                    bad.push(format!("单字 {c} -> {one}（{} 字）", one.chars().count()));
                }
                concat.push_str(&one);
            }
            if whole != concat {
                bad.push(format!("整串≠逐字拼接：{s} 整串 {whole} / 逐字 {concat}"));
            }
        }
        assert!(
            bad.is_empty(),
            "[HANT-SHADOW] 1:1 前提被打破：\n{}",
            bad.join("\n")
        );
    }

    /// Q2 全局强证：扫描 contains_han 覆盖的全部 CJK 区段（基本区 + 扩展 A + 兼容区），
    /// 逐字验证 ZhHans 归一【恰好 1 字】（既不删也不增）。单字若已 1:1，则整串必然逐字对齐
    /// （zhconv 的整串规则不会跨字改变对齐——由 per_char 测同证）。
    #[test]
    fn shadow_global_single_char_1to1_scan() {
        let ranges: &[(u32, u32, &str)] = &[
            (0x4E00, 0x9FFF, "CJK 基本区"),
            (0x3400, 0x4DBF, "扩展 A"),
            (0xF900, 0xFAFF, "兼容区"),
        ];
        let mut total = 0usize;
        let mut non_1to1: Vec<(char, String)> = Vec::new();
        for &(lo, hi, name) in ranges {
            let mut n = 0usize;
            for cp in lo..=hi {
                let Some(c) = char::from_u32(cp) else {
                    continue;
                };
                let one = zhconv(&c.to_string(), Variant::ZhHans);
                n += 1;
                total += 1;
                if one.chars().count() != 1 {
                    non_1to1.push((c, one));
                }
            }
            println!("[HANT-SHADOW] {name} U+{lo:04X}..U+{hi:04X}: {n} 字");
        }
        println!(
            "[HANT-SHADOW] 全扫描 {total} 字，非 1:1 的 {} 个",
            non_1to1.len()
        );
        for (c, s) in non_1to1.iter().take(80) {
            println!("   U+{:04X} {c} -> {s}", *c as u32);
        }
        assert!(
            non_1to1.is_empty(),
            "[HANT-SHADOW] 存在单字非 1:1 映射 {} 个（首 80 见 stdout）",
            non_1to1.len()
        );
    }

    /// Q2 词组级补充：zhconv 的 ZhHans 表**确实含词组规则**（crate 文档例 `鼠麴草→鼠曲草`）。
    /// 词组规则理论上可能改变长度 → 对一组多字繁体词/成语实测长度守恒。
    /// 若本条与全局单字扫描同时通过，则「逐字 1:1」前提在字符级与常见词组级均未被打破。
    #[test]
    fn shadow_phrase_corpus_len_preserved() {
        const PHRASES: &[&str] = &[
            "鼠麴草",
            "頭髮",
            "麵條",
            "一隻貓",
            "乾淨",
            "後面",
            "計算機",
            "網路",
            "軟體",
            "硬碟",
            "資訊",
            "藝術",
            "體育",
            "環境",
            "經驗",
            "關係",
            "發展",
            "標準",
            "實際",
            "聲明",
            "徹底",
            "慶祝",
            "擁護",
            "選擇",
            "戰爭",
            "擁擠",
            "雖然",
            "藥物",
            "類型",
            "體會",
            "確實",
            "觀眾",
            "解釋",
            "應該",
            "實驗",
            "雙方",
            "維護",
            "建設",
            "語言",
            "討論",
            "辯論",
            "嚴肅",
            "傳統",
            "遺產",
            "欣賞",
            "創造",
            "價值",
            "願望",
            "歡迎",
            "隱藏",
            "鐘錶",
            "螞蟻",
            "蝴蝶",
            "鸚鵡",
            "鴕鳥",
            "鳳凰",
            "龍蝦",
            "鱷魚",
            "憂鬱",
            "朦朧",
        ];
        let mut bad: Vec<String> = Vec::new();
        for s in PHRASES {
            let shadow = zhconv(s, Variant::ZhHans);
            let (a, b) = (s.chars().count(), shadow.chars().count());
            if a != b {
                bad.push(format!("{s} -> {shadow} (原 {a} / 影 {b})"));
            }
        }
        println!(
            "[HANT-SHADOW] 词组语料 {} 条，长度不等 {} 条",
            PHRASES.len(),
            bad.len()
        );
        assert!(
            bad.is_empty(),
            "[HANT-SHADOW] 词组级出现长度不等：\n{}",
            bad.join("\n")
        );
    }

    /// 对照：项目现状用的地区变体（ZhCN / ZhTW）做地区词汇替换 → 长度可变。
    /// 只打印不断言（用于证 Q1「必须用 ZhHans 而非 ZhCN/ZhTW」）。
    #[test]
    fn shadow_contrast_region_variants() {
        let mut table = String::new();
        for s in SAMPLES {
            let (tw, _, twb) = counts(s, Variant::ZhTW);
            let (cn, _, cnb) = counts(s, Variant::ZhCN);
            let (hans, _, hb) = counts(s, Variant::ZhHans);
            table.push_str(&format!(
                "  {s}  ZhTW->{tw}({twb})  ZhCN->{cn}({cnb})  ZhHans->{hans}({hb})\n"
            ));
        }
        println!("[HANT-SHADOW] 地区变体对照：\n{table}");
    }

    // ============================================================================
    // GUARD-221-IDENTITY · 影子串前提的**另一半**：identity（不只是 1:1）
    // ----------------------------------------------------------------------------
    // 上面 4 条只证了「长度 1:1」。221 的「简体路径逐字零回归」还依赖更强的一条：
    //   简体输入时 `shadow == orig`，即 `zhconv(简体字, ZhHans) 恒等返回该字`。
    // 若某个简体字经 ZhHans 变成**另一个字**，影子串虽仍 1:1（长度一样、逐字对齐），
    // 却已与原串**不同字** ⇒ 判定层拿错误的字去查规则表 ⇒ 简体路径（绝大多数用户）
    // **静默**走错分支。这个洞比 221 要修的繁体问题严重得多，故单列三层护栏。
    //
    // ① `shadow_identity_itn_rule_chars_zhhans`：ITN 规则表实际用到的字
    //    （硬编码最小集 + 从 `itn-rules.toml` 抽取的全表汉字）
    // ② `shadow_identity_global_simplified_scan`：全域扫描（与 221A 同三区段）
    //    🔴 先判定「该字是简体」再要求 identity —— 不得对繁体字要求 identity
    // ③ `shadow_identity_negative_control_traditional_diverges`：🔴 反面对照
    //
    // 🔴 实测结果（2026-09-17，zhconv 0.4.1 OpenCC 数据扫描）：全域 28,096 字中「简体形」
    //    2,742 个，其中**非 identity 恰好 6 个**，全部是双向变体字（ST 与 TS 两张表对同一个字
    //    给出不同目标）⇒ 见 `R1_AMBIGUOUS_VARIANTS` 的逐条表与豁免理由。
    //    这 6 个都不用于标准简体书写，故 R1 的实际暴露面被收敛到「已知 6 个罕见字」，而不是
    //    「任意字都可能」。
    //
    // ✅ **本组在什么情况下会变红**（即判别力所在）：
    //    a) 出现第 7 个「简体形却被 ZhHans 改写」的字（OpenCC 升级 / 换表）→ ② novel 非空；
    //    b) 豁免表与实测集合不再精确相等（6 个里任何一个不再违反）→ ② 集合比对失败；
    //    c) 若 identity 判据被改成恒真（例如把 `!=` 写成 `==`、或 ZhHans 误换成对简体恒等的
    //       变体）→ ③ 的 `assert_ne!` 首条即红（繁体 這 竟然恒等）；
    //    d) ①-a 最小集里任何一个 ITN 用字变成了非 identity → ① 红（含 48 字计数下限防空跑）；
    //    e) 扫描域被改窄（total < 25,000）或 is_simplified_only 失效致 required < 300 → ② 红。
    // ============================================================================

    /// 该字经 ZhHant 归一后**恒等**（本身是繁体形 / 无字形差异）。
    /// FIX-222 B 新谓词用它判定「影子串偏离原串是否属正当理由（繁体输入本就是 221 的目标）」。
    fn hant_identity(c: char) -> bool {
        zhconv(&c.to_string(), Variant::ZhHant) == c.to_string()
    }

    /// identity 判据：字形经 ZhHans 归一后必须**原样返回该字**。
    fn hans_identity(c: char) -> bool {
        zhconv(&c.to_string(), Variant::ZhHans) == c.to_string()
    }

    /// ①-a 硬编码最小集（任务书给定）：ITN 规则表实际用到的简体字 ——
    /// 数字 / 进位 / 时间后缀 / 货币·度量衡单位 / 指示代词 + 「么」。
    /// 硬编码的理由：**不依赖文件内容**，toml 怎么增删这批字都必测。
    const ITN_RULE_CHARS: &str =
        "零一二三四五六七八九十百千万亿两半年月日号点分秒刻个块钱毛元角斤吨米克岁这那每哪某么";

    /// 🔴 R1 实测豁免表（**数据级证据，不是「先放着」**）。
    ///
    /// **判据（FIX-222 B 定稿，谓词无关式）**：对全域每个字 c 只判一件事 ——
    /// **若 `ZhHans(c) != c`（影子串会偏离原串），则必须 `ZhHant(c) == c`**
    /// （即 c 本身是繁体形：繁体输入归一正是 221 的目标，偏离属正当理由）。
    /// 两者**都变** = 双向变体字（影子偏离却又不属「繁体输入」这一正当理由）→
    /// 必须与本豁免表**精确相等**。
    /// 🔴 **不存在被跳过的字**：identity 成立的字也走 `ZhHans(c) == c` 这一支被显式判过。
    /// 旧版用 `is_simplified_only` 当闸门，会把「运行时判为非简体形」的字整批跳过
    /// （例：`万`，原因见下）—— 主控 2026-09-17 指出这是未验证假设，故弃用闸门。
    ///
    /// 实测豁免集合（**5 个**；tester-1 阶段四实跑输出
    /// 「全扫描 28096 字，其中简体形 2670 个，非 identity 5 个（已知双向变体 5，新增 0）」）：
    ///
    /// | 字 | 简→繁（ZhHant 会变） | 繁→简（ZhHans 会变） |
    /// | --- | --- | --- |
    /// | 緼 | 縕 | 缊 |
    /// | 苧 | 薴 | 苎 |
    /// | 藴 | 蘊 | 蕴 |
    /// | 輼 | 轀 | 辒 |
    /// | 醖 | 醞 | 酝 |
    ///
    /// 🔴 **`麽` 不在此表** —— 首版我按「取 OpenCC 首值」的模拟误收，阶段四实测证伪：
    /// `TSCharacters.txt` 的 `麽 -> 么 麽` 是**多值且含自身** → zhconv 构建期整条丢弃
    /// （`build.rs:571-575`，源码注释原文 *be conservative when converting*）
    /// ⇒ 运行时 `ZhHans(麽) == 麽`（恒等）⇒ 它根本不是双向变体。
    /// **同一条规则**也让 `STCharacters.txt` 的 `万 -> 萬 万` 被丢弃 ⇒ 运行时 `ZhHant(万) == 万`
    /// ⇒ `万` 不是「简体形」（这正是旧闸门会跳过它的原因）；但 `ZhHans(万) == 万` ⇒ identity 成立、
    /// 影子不偏离 ⇒ 新谓词下它走「identity 成立」支被**显式判过**。
    /// （首版模拟值「全域 2,742 简体形 / 非 identity 6」**全错**，实测 2,670 / 5。
    ///  教训：不要用依赖包的原始数据文件模拟其运行时行为 —— 构建期还会改写数据。）
    ///
    /// **为什么可豁免而不是必须修**（两条依据缺一不可，主控 2026-09-17 拍板）：
    ///
    /// ① **不用于标准简体书写**：这 5 个都是异体/变体字（通用形 缊/苎/蕴/辒/酝），
    ///    简体路径的实际暴露面止于这 5 个罕见字。
    ///
    /// ② 🔴 **决定性依据：ITN 规则表零命中** —— 由 ①-b 机器断言（抽取 `itn-rules.toml` 全部汉字，
    ///    双向变体必须为 0；**具体计数以 println 输出为准，不写死模拟值**）；
    ///    且这 5 个字的 ZhHans 影子字（缊/苎/蕴/辒/酝）在规则表里的命中数全是 0。
    ///    ⇒ 这 5 个字即使出现在口述里，影子串也只影响「一次不会命中任何规则的匹配判定」，
    ///    而 ITN 输出恒取**原串** ⇒ **用户看到的字分毫不变**。R1 到此实质解除。
    ///
    /// 🔴 **本表是漂移检测**：出现**第 6 个**双向变体（OpenCC 升级 / 换表）→ 断言立刻红；
    /// 这 5 个里任一不再违反（表被修）→ 集合比对不相等 → 同样红。两头都不放过。
    ///
    /// **本组变红条件（a~e，缺一即视为有人动了前提，须回报主控）**：
    /// - a) 出现第 6 个双向变体 → `shadow_identity_global_simplified_scan` 的 novel 非空；
    /// - b) 豁免表与实测集合不再精确相等 → 同上的集合比对失败；
    /// - c) 判据被改成恒真（`!=` 写成 `==`、或换变体使繁体恒等）
    ///      → `shadow_identity_negative_control_traditional_diverges` 的 `assert_ne!` 首条即红；
    /// - d) ①-a 最小集里任一 ITN 用字变成非 identity，或其计数 < 40（防空跑）→ `..._itn_rule_chars_zhhans` 红；
    /// - e) 扫描域被改窄（`total < 25_000`）或扫描未真跑（`hans_changed` / `hant_changed` 计数过低）→ ② 红。
    const R1_AMBIGUOUS_VARIANTS: &[char] = &['緼', '苧', '藴', '輼', '醖'];

    /// ① ITN 规则表用字 identity 断言（最小集 + 全表抽取）。
    #[test]
    fn shadow_identity_itn_rule_chars_zhhans() {
        // ①-a 硬编码最小集：全部为简体形，**无条件**要求 identity
        let mut bad: Vec<String> = Vec::new();
        for c in ITN_RULE_CHARS.chars() {
            if !hans_identity(c) {
                bad.push(format!(
                    "U+{:04X} {c} -> {}",
                    c as u32,
                    zhconv(&c.to_string(), Variant::ZhHans)
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "[HANT-SHADOW-IDENTITY] ITN 规则表最小集出现非 identity（{} 个）：\n{}",
            bad.len(),
            bad.join("\n")
        );
        // 反空断言：最小集必须真被逐字测过
        assert!(
            ITN_RULE_CHARS.chars().count() >= 40,
            "[HANT-SHADOW-IDENTITY] 最小集字符数 {} 异常偏少，断言可能已失效",
            ITN_RULE_CHARS.chars().count()
        );

        // ①-b 从 itn-rules.toml 全表抽取汉字（条目 + 注释一并抽，宁多勿漏），去重后逐字判 **两个方向**。
        //      🔴 FIX-222 B：**无闸门** —— 每个字都被判（identity 成立的字走 `ZhHans(c)==c` 支），
        //      不允许再出现「运行时判为非简体形就跳过」的假设。
        //      将来 toml 收进繁体字（如 221 把 號/點 写进表）也安全：它们 ZhHant 恒等 → 走「允许」支。
        const ITN_RULES: &str = include_str!("../itn-rules.toml");
        let mut chars: std::collections::BTreeSet<char> = std::collections::BTreeSet::new();
        for c in ITN_RULES.chars() {
            if super::contains_han(&c.to_string()) {
                chars.insert(c);
            }
        }
        let mut bad_rules: Vec<String> = Vec::new();
        let mut hans_changed = 0usize;
        let mut hant_changed = 0usize;
        for &c in &chars {
            if !hans_identity(c) {
                hans_changed += 1;
            }
            if !hant_identity(c) {
                hant_changed += 1;
            }
            // 双向变体（影子偏离且不属「繁体输入」）才算问题；豁免表外一个都不许有
            if !hans_identity(c) && !hant_identity(c) && !R1_AMBIGUOUS_VARIANTS.contains(&c) {
                bad_rules.push(format!(
                    "U+{:04X} {c} -> ZhHans {} / ZhHant {}",
                    c as u32,
                    zhconv(&c.to_string(), Variant::ZhHans),
                    zhconv(&c.to_string(), Variant::ZhHant)
                ));
            }
        }
        println!(
            "[HANT-SHADOW-IDENTITY] itn-rules.toml 抽取汉字 {} 个（逐字判，无跳过）：\
             ZhHans 会改写 {} 个 / ZhHant 会改写 {} 个 / 双向变体 {} 个",
            chars.len(),
            hans_changed,
            hant_changed,
            bad_rules.len()
        );
        assert!(
            bad_rules.is_empty(),
            "[HANT-SHADOW-IDENTITY] itn-rules.toml 用字里出现双向变体（{} 个，豁免表外）：\n{}",
            bad_rules.len(),
            bad_rules
                .iter()
                .take(40)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
        // 反空断言：抽取必须真的干活（否则本条恒绿无判别力）
        assert!(
            chars.len() >= 300,
            "[HANT-SHADOW-IDENTITY] itn-rules.toml 只抽到 {} 个汉字（预期数百），抽取逻辑可能失效",
            chars.len()
        );
    }

    /// ② 全域扫描（与 221A 的 `shadow_global_single_char_1to1_scan` 同三区段）——
    /// **谓词无关式，逐字全判、无跳过**：名字沿用（阶段四日志/任务书均按此名索引），
    /// 但语义已从「只看简体形」改为「ZhHans(c)≠c 必须由 ZhHant(c)==c 正当化」。
    #[test]
    fn shadow_identity_global_simplified_scan() {
        let ranges: &[(u32, u32, &str)] = &[
            (0x4E00, 0x9FFF, "CJK 基本区"),
            (0x3400, 0x4DBF, "扩展 A"),
            (0xF900, 0xFAFF, "兼容区"),
        ];
        let mut total = 0usize;
        let mut hans_changed = 0usize;
        let mut hant_changed = 0usize;
        let mut variants: Vec<(char, String, String)> = Vec::new();
        for &(lo, hi, name) in ranges {
            let mut n = 0usize;
            for cp in lo..=hi {
                let Some(c) = char::from_u32(cp) else {
                    continue;
                };
                total += 1;
                let hans = zhconv(&c.to_string(), Variant::ZhHans);
                let hant = zhconv(&c.to_string(), Variant::ZhHant);
                if hans != c.to_string() {
                    hans_changed += 1;
                }
                if hant != c.to_string() {
                    hant_changed += 1;
                }
                // 🔴 判据：影子偏离（ZhHans 改写）必须由「该字本身是繁体形」（ZhHant 恒等）正当化；
                //          两者都变 ⇒ 双向变体 ⇒ 记入 variants（豁免表外一个都不许有）。
                if hans != c.to_string() && hant != c.to_string() {
                    n += 1;
                    variants.push((c, hans, hant));
                }
            }
            println!("[HANT-SHADOW-IDENTITY] {name} U+{lo:04X}..U+{hi:04X}: 双向变体 {n} 个");
        }
        let mut known_chars: Vec<char> = Vec::new();
        let mut novel: Vec<String> = Vec::new();
        for (c, hans, hant) in &variants {
            if R1_AMBIGUOUS_VARIANTS.contains(c) {
                known_chars.push(*c);
            } else {
                novel.push(format!(
                    "U+{:04X} {c} -> ZhHans {hans} / ZhHant {hant}",
                    *c as u32
                ));
            }
        }
        println!(
            "[HANT-SHADOW-IDENTITY] 全扫描 {total} 字（逐字判、无跳过）：ZhHans 会改写 {hans_changed} 个 / \
             ZhHant 会改写 {hant_changed} 个 / 双向变体 {} 个（已知 {}，新增 {}）",
            variants.len(),
            known_chars.len(),
            novel.len()
        );
        for (c, hans, hant) in variants.iter().take(80) {
            println!(
                "   双向变体：U+{:04X} {c} -> ZhHans {hans} / ZhHant {hant}",
                *c as u32
            );
        }
        // 豁免表本身必须与实测**精确相等**：多了（表变）或少了（新违反）都红
        let mut known_sorted = known_chars.clone();
        known_sorted.sort_unstable();
        let mut expect_sorted = R1_AMBIGUOUS_VARIANTS.to_vec();
        expect_sorted.sort_unstable();
        assert_eq!(
            known_sorted, expect_sorted,
            "[HANT-SHADOW-IDENTITY] 双向变体的实测集合与豁免表不符 —— \
             OpenCC 表已变，请重新核对 R1_AMBIGUOUS_VARIANTS（并回报主控）"
        );
        assert!(
            novel.is_empty(),
            "[HANT-SHADOW-IDENTITY] 出现**新增**双向变体 {} 个（豁免表外）：\n{}",
            novel.len(),
            novel
                .iter()
                .take(40)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
        // 反空断言：扫描域必须真扫过，且两个方向都确实有大批字被改写（否则本条恒绿无判别力）
        assert!(
            total >= 25_000,
            "[HANT-SHADOW-IDENTITY] 扫描域只覆盖 {} 字（预期 ≥28,096 量级），域定义可能被改窄",
            total
        );
        assert!(
            hans_changed >= 500 && hant_changed >= 500,
            "[HANT-SHADOW-IDENTITY] 改写计数异常偏低（ZhHans {hans_changed} / ZhHant {hant_changed}，预期各上千）\
             —— zhconv 行为或扫描域可能已变，本条已失去判别力"
        );
    }

    /// ③ 🔴 反面对照：**证明前两层的判据不是恒真的空断言**。
    ///
    /// ①② 的判据是 `ZhHans(c) == c`（或它的正当化形式）；若对任何输入都为真，前两层毫无判别力。
    /// 本条用**已知会变**的繁体字把「ZhHans 会改写」这一支证伪，并验证「正当化」支（ZhHant 恒等）
    /// 确实存在且可判 —— 两支都被覆盖，判据才不是恒真式。
    #[test]
    fn shadow_identity_negative_control_traditional_diverges() {
        // ③-a 判据可证伪：繁体字经 ZhHans 必定换字（这就是 identity 判据的「反例」）
        for t in ["這", "麼", "點", "號", "萬"] {
            assert_ne!(
                zhconv(t, Variant::ZhHans),
                t,
                "[HANT-SHADOW-IDENTITY] 反例失效：繁体 {t} 经 ZhHans 竟然恒等 \
                 ⇒ ①② 的判据恒真、无判别力"
            );
            // ③-b 正当化支必须成立：这些繁体字 ZhHant 恒等 ⇒ 它们走「允许」支，
            //      而不是靠「被跳过」逃过检查（FIX-222 B 弃用闸门后不再有跳过）
            assert!(
                hant_identity(t.chars().next().unwrap()),
                "[HANT-SHADOW-IDENTITY] 繁体 {t} 应满足 ZhHant 恒等（正当化支），否则会被误判为双向变体"
            );
        }
        // ③-c 高置信度反例的字形结果（任务书示例）：這→这，证明变的是「字」不是「长度」
        assert_eq!(zhconv("這", Variant::ZhHans), "这");
        // ③-d 简体对照：这些字的 identity 必须成立
        //      🔴 注意：**不再断言它们「被判为简体形」** —— zhconv 构建期会丢弃
        //      「多值且含自身」的条目（`build.rs:571-575`）：`STCharacters.txt` 的
        //      `万 -> 萬 万` 被丢 ⇒ 运行时 `ZhHant(万) == 万` ⇒ 万 不是「简体形」。
        //      但 `ZhHans(万) == 万` ⇒ identity 成立 ⇒ 新谓词下走「identity 成立」支被显式判过。
        //      （首版在此断言 `is_simplified_only(万)`，阶段四实跑红 —— 该断言本身是错的。）
        for s in ["这", "么", "点", "号", "万"] {
            let c = s.chars().next().unwrap();
            assert!(
                hans_identity(c),
                "[HANT-SHADOW-IDENTITY] 简体 {s} 经 ZhHans 必须恒等"
            );
        }
    }
}
