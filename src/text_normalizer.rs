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
        return Some("保留输入中各语种的原文形态，不要将任何内容翻译成其他语言。");
    }

    Some(match script {
        ChineseScript::Simplified => "请将输出中的中文部分统一为简体中文（中国大陆简体字）字形；不要翻译任何非中文内容，英文、日文、韩文一律保留原文原样。",
        ChineseScript::Traditional => "请将输出中的中文部分统一为繁体中文（台湾正体字）字形；不要翻译任何非中文内容，英文、日文、韩文一律保留原文原样。",
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
        ChineseScript::Simplified => "请将输出中的中文部分统一为简体中文（中国大陆简体字）字形。",
        ChineseScript::Traditional => "请将输出中的中文部分统一为繁体中文（台湾正体字）字形。",
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
            !s.contains("简体中文"),
            "protection must not mention 简体中文"
        );
        assert!(
            !s.contains("繁体中文"),
            "protection must not mention 繁体中文"
        );
        assert!(
            s.contains("不要"),
            "protection must retain do-not-translate guard"
        );
        assert!(s.contains("翻译"));
    }

    #[test]
    fn lang_mixed_sino_korean_returns_protection_only() {
        // 中韩混合：含谚文 → 纯保护措辞
        let instr = script_instruction("你好 안녕하세요", ChineseScript::Simplified);
        assert!(instr.is_some());
        let s = instr.unwrap();
        assert!(!s.contains("简体中文"));
        assert!(!s.contains("繁体中文"));
        assert!(s.contains("不要"));
        assert!(s.contains("翻译"));
    }

    #[test]
    fn lang_mixed_sino_english_keeps_script_instruction() {
        // 中英混合：无假名无谚文 → 字形统一 + 不要翻译
        let instr = script_instruction("你好 world", ChineseScript::Simplified);
        assert!(instr.is_some());
        let s = instr.unwrap();
        assert!(s.contains("简体中文"));
        assert!(s.contains("不要翻译"));
    }

    #[test]
    fn lang_mixed_pure_japanese_returns_protection_only() {
        // 纯日语：含假名也含汉字（漢字在 CJK 区）→ 纯保护措辞
        let instr = script_instruction("日本語テスト", ChineseScript::Simplified);
        assert!(instr.is_some(), "pure JP with kanji should get protection");
        let s = instr.unwrap();
        assert!(!s.contains("简体中文"));
        assert!(s.contains("不要"));
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
        assert!(instr.unwrap().contains("简体中文"));
    }

    #[test]
    fn lang_mixed_pure_chinese_traditional_keeps_instruction() {
        // 纯中文繁体：繁体分支对称
        let instr = script_instruction("你好世界", ChineseScript::Traditional);
        assert!(instr.is_some());
        let s = instr.unwrap();
        assert!(s.contains("繁体中文"));
        assert!(s.contains("不要翻译"));
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
        assert!(s.contains("简体中文"));
        assert!(
            !s.contains("不要翻译"),
            "translate path must NOT contain do-not-translate"
        );
        assert!(
            !s.contains("保留原文"),
            "translate path must NOT contain protection-only wording"
        );
    }

    #[test]
    fn translate_path_instruction_no_kana_no_hangul_traditional() {
        let instr = script_instruction_for_translate("你好世界", ChineseScript::Traditional);
        assert!(instr.is_some());
        let s = instr.unwrap();
        assert!(s.contains("繁体中文"));
        assert!(!s.contains("不要翻译"));
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

    #[test]
    fn translate_path_instruction_no_kana_no_hangul_mixed_cn_en() {
        // 中英混合：翻译路径只字形约束
        let instr = script_instruction_for_translate("你好 world", ChineseScript::Simplified);
        assert!(instr.is_some());
        assert!(!instr.unwrap().contains("不要翻译"));
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
