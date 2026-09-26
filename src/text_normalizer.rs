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

/// FORMAT-FALLBACK-303: 保守的本地语气词去除（中/英/日/韩）。**纯函数、无 IO、无配置**。
///
/// 只做两件事，看不懂上下文就不动：
/// - **规则 A**：摘除**句首**（文本开头或句末标点之后）的纯犹豫词；句中一律不动。
/// - **规则 B**：折叠**紧邻的字面重复**（≥2 次，中间可选逗号/空白）。
///
/// 🔴 保守口径（宁可漏摘，不可误摘）：
/// - 规则 A 只收几乎无实义的犹豫词；有实义用法的（`那个`/`就是`/`然后`/`あの`/`그`…）**一概不收**
///   —— 词表刻意比 `is_effective_text` 的检测表窄，两者不共用。
/// - 摘除后要求其后紧跟 空白/逗号/顿号/句末标点/文本末尾，否则视为「可能是实词」而不摘
///   —— **英/日/韩全部 + 中文 T2 字**（防 `Uhura`/`어디`/`まあまあ`/`唉声叹气`/`呐喊`）；
///   中文 **T1 字 `呃`/`嗯`** 几乎不作词首，不受此限（`呃我觉得` 也摘，第 17 条）。
/// - 规则 B 只折叠**完全字面相同**的相邻重复，**且重复单元须在「话语标记白名单」内**：
///   中文限 2–3 字单元（`看看`/`哈哈` 单字叠词与 `研究研究` ABAB 重叠是正常语法，绝不折叠）；
///   英/韩按空白分词、查各自白名单（`I I` → `I`、`그 그` → `그`，但 `had had`/`that that` 原样）。
/// - 全程不做「看起来像」的猜测；只在两条规则明确命中时改动。
pub fn strip_fillers_conservative(text: &str) -> String {
    let a = strip_leading_fillers(text);
    // 规则 B 可能折叠出新的可折叠串（4 连叠 → 2 连叠）；规则 C 折掉一份后可能又暴露新的相邻重复；
    // 规则 D（FILLER-RESTART-440）折掉一截重说后同理 ⇒ 三条规则一起迭代到不动点，保证幂等。
    let pass = |s: &str| collapse_restarts(&collapse_long_repeats(&collapse_adjacent_repeats(s)));
    let mut cur = pass(&a);
    loop {
        let next = pass(&cur);
        if next == cur {
            break;
        }
        cur = next;
    }
    cur
}

/// 规则 A 的句首犹豫词表。**刻意只收几乎无实义者**，中文按「能否起头组成实义词」分两档。
///
/// T1（无边界即可摘）：`呃`/`嗯` 几乎不作常用词词首（`呃逆` 是唯一生僻医学词），可无边界摘。
/// T2（须边界才摘）：`啊哦噢唉诶欸呐` 能起头组词（`唉声叹气`/`呐喊`/`哦豁`…），直接接汉字则不摘。
/// 🔴 拿不准的字一律放 T2 —— 漏摘只是没优化到，误摘是吞字。
const LEADING_FILLERS_ZH_T1: &[&str] = &["呃", "嗯"];
const LEADING_FILLERS_ZH_T2: &[&str] = &["啊", "哦", "噢", "唉", "诶", "欸", "呐"];
const LEADING_FILLERS_JA: &[&str] = &[
    "えーと",
    "えっと",
    "ええと",
    "うーん",
    "あのー",
    "あのう",
    "まあ",
];
const LEADING_FILLERS_KO: &[&str] = &["어", "음", "에", "아"];
const LEADING_FILLERS_EN: &[&str] = &["hmm", "erm", "um", "uh", "er", "mm", "ah", "eh"];

/// 🔴 FORMAT-FALLBACK-303 / FIX-FF303-B：规则 B 的**可折叠单元白名单**（只有这些词的紧邻重复才判为口吃）。
///
/// 为什么必须白名单：`然后然后`（口吃）与 `研究研究`（ABAB 动词重叠＝稍微研究一下）在字面上
/// 完全无法区分，按长度放宽会两头误伤（英文 `had had` / `that that` 同理）。故只折叠
/// **本身即已知话语标记**的重复；任何能独立承担实义的词都不许进表。
///
/// ⚠️ 本表**与规则 A 的句首表用途不同，不要合并**：`然后/就是/那个/这个` 有实义用法，
/// 单独出现时不能摘（故不在 A 表）；但它们的**紧邻重复**是口吃的可靠信号（故在 B 表）。
const REPEAT_COLLAPSIBLE_ZH: &[&str] = &["然后", "就是", "那个", "这个", "所以", "反正", "其实"];
const REPEAT_COLLAPSIBLE_EN: &[&str] = &[
    "i", "the", "a", "and", "so", "but", "like", "you", "we", "it",
];
const REPEAT_COLLAPSIBLE_JA: &[&str] = &["その", "あの", "えー", "まあ"];
const REPEAT_COLLAPSIBLE_KO: &[&str] = &["그", "저", "음"];

/// 规则 B：**连写语言**（中/日）的 CJK 单元是否在可折叠白名单里（ZH ∪ JA）。
/// 韩语走空白分词分支，不经过本函数。
fn is_collapsible_cjk_unit(unit: &str) -> bool {
    REPEAT_COLLAPSIBLE_ZH.contains(&unit) || REPEAT_COLLAPSIBLE_JA.contains(&unit)
}

/// 连写语言（中/日）字符：Han / 假名（含长音符 ー）。**不含韩文** —— 韩语是空白分词语言，
/// 规则 B 按 token 处理（口径同英文）。
fn is_cjk_char(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF        // 平假名 / 片假名（含长音符 ー）
        | 0x3400..=0x4DBF      // CJK 扩展 A
        | 0x4E00..=0x9FFF      // CJK 统一表意
        | 0xF900..=0xFAFF      // CJK 兼容表意
    )
}

/// 韩文字符（音节 / 字母）。韩语按空白分词 ⇒ 规则 B 用 token 比对（同英文口径）。
fn is_hangul_char(c: char) -> bool {
    matches!(c as u32, 0xAC00..=0xD7AF | 0x1100..=0x11FF)
}

/// 规则 A：句末标点（可含后续空白）之后视为新的「句首」。
fn is_sentence_end(c: char) -> bool {
    matches!(c, '。' | '！' | '？' | '.' | '!' | '?' | '；' | ';')
}

/// 规则 A：链路后允许出现的边界（否则判为可能是实词，不摘）。
fn is_filler_boundary(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '，' | ',' | '、' | '。' | '！' | '？' | '.' | '!' | '?' | '；' | ';'
        )
}

/// 规则 A：摘掉后若紧跟的标点/空白也一并吃掉。
fn is_filler_trailing_sep(c: char) -> bool {
    c.is_whitespace() || matches!(c, '，' | ',' | '、')
}

/// 规则 B：相邻重复之间允许的间隔（仅逗号/顿号/空白，不含句末标点）。
fn is_repeat_sep(c: char) -> bool {
    c.is_whitespace() || matches!(c, '，' | ',' | '、')
}

/// 在 `k` 处匹配一个犹豫词，取**最长**命中（防 `erm` 被 `er` 截断）。
/// 返回 `(字符长度, 是否要求后续边界)`：
/// - 中文单字犹豫词几乎无实义 ⇒ **不要求**边界（`呃我觉得` 也摘，见 FF303 第 17 条用例）。
/// - 英/日/韩 ⇒ **要求**边界（防 `Uhura` 抠 `uh`、`어디` 抠 `어`）。
fn match_filler_token(chars: &[char], k: usize) -> Option<(usize, bool)> {
    let n = chars.len();
    let starts_with = |tok: &str| -> bool {
        let t: Vec<char> = tok.chars().collect();
        k + t.len() <= n && chars[k..k + t.len()] == t[..]
    };
    let starts_with_ci = |tok: &str| -> bool {
        let t: Vec<char> = tok.chars().collect();
        k + t.len() <= n
            && chars[k..k + t.len()]
                .iter()
                .zip(t.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
    };
    let mut best_len = 0usize;
    let mut best_strict = false;
    // 中文 T1（呃/嗯）：几乎无实义 ⇒ 无边界要求。
    for tok in LEADING_FILLERS_ZH_T1 {
        if starts_with(tok) {
            let l = tok.chars().count();
            if l > best_len {
                best_len = l;
                best_strict = false;
            }
        }
    }
    // 中文 T2（啊哦噢唉诶欸呐）：能起头组词 ⇒ 要求边界。
    for tok in LEADING_FILLERS_ZH_T2 {
        if starts_with(tok) {
            let l = tok.chars().count();
            if l > best_len {
                best_len = l;
                best_strict = true;
            }
        }
    }
    for tok in LEADING_FILLERS_JA.iter().chain(LEADING_FILLERS_KO) {
        if starts_with(tok) {
            let l = tok.chars().count();
            if l > best_len {
                best_len = l;
                best_strict = true;
            }
        }
    }
    for tok in LEADING_FILLERS_EN {
        // 英文必须词边界：前一个字符不能是字母数字（防从 "Uhura" 里抠 "uh"）。
        if starts_with_ci(tok) && (k == 0 || !chars[k - 1].is_ascii_alphanumeric()) {
            let l = tok.chars().count();
            if l > best_len {
                best_len = l;
                best_strict = true;
            }
        }
    }
    if best_len > 0 {
        Some((best_len, best_strict))
    } else {
        None
    }
}

/// 从 `start` 起（允许前导空白）贪婪吃掉一串犹豫词。
/// 只有链中含「要求边界」的词时，才要求链路后是边界；纯中文链不受此限。
fn match_leading_filler_chain(chars: &[char], start: usize) -> Option<usize> {
    let n = chars.len();
    let mut k = start;
    while k < n && chars[k].is_whitespace() {
        k += 1;
    }
    let mut cursor = k;
    let mut matched = false;
    let mut needs_boundary = false;
    while let Some((len, strict)) = match_filler_token(chars, cursor) {
        cursor += len;
        matched = true;
        needs_boundary |= strict;
    }
    if !matched {
        return None;
    }
    if needs_boundary && cursor < n && !is_filler_boundary(chars[cursor]) {
        return None;
    }
    Some(cursor)
}

/// 规则 A 主体：只改「文本开头 / 句末标点后」的句首位置。
fn strip_leading_fillers(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    let mut at_slot_start = true;
    while i < n {
        if at_slot_start {
            if let Some(end) = match_leading_filler_chain(&chars, i) {
                i = end;
                while i < n && is_filler_trailing_sep(chars[i]) {
                    i += 1;
                }
                continue; // 内容尚未输出，仍处于句首
            }
        }
        let c = chars[i];
        at_slot_start = is_sentence_end(c);
        out.push(c);
        i += 1;
    }
    out
}

/// 规则 B：折叠紧邻的字面重复（≥2 次，中间可选逗号/空白）。
///
/// 🔴 **白名单门控**（FIX-FF303-B）：只有重复单元本身是**已知话语标记**（`REPEAT_COLLAPSIBLE_*`）
/// 才判为口吃并折叠；否则一律原样 —— 因为 `然后然后`（口吃）与 `研究研究`（ABAB 动词重叠）
/// 字面上无法区分，英文 `had had`/`that that` 同理。
/// 英文按空白分词（小写归一后查表，字面完全相同）；中日韩按 2–3 字 CJK 单元查表。
fn collapse_adjacent_repeats(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < n {
        let c = chars[i];
        if c.is_ascii_alphanumeric() {
            let mut e = i;
            while e < n && chars[e].is_ascii_alphanumeric() {
                e += 1;
            }
            let word: String = chars[i..e].iter().collect();
            // FIX-FF303-B：只有白名单词（小写归一）的紧邻重复才折叠（`I I`→`I`；
            // `had had`/`that that` 不在表内 ⇒ 原样）。
            let foldable = REPEAT_COLLAPSIBLE_EN.contains(&word.to_ascii_lowercase().as_str());
            let mut k = e;
            let mut cnt = 1usize;
            if foldable {
                loop {
                    let mut j = k;
                    while j < n && is_repeat_sep(chars[j]) {
                        j += 1;
                    }
                    let mut e2 = j;
                    while e2 < n && chars[e2].is_ascii_alphanumeric() {
                        e2 += 1;
                    }
                    if e2 > j
                        && chars[j..e2]
                            .iter()
                            .collect::<String>()
                            .eq_ignore_ascii_case(&word)
                    {
                        cnt += 1;
                        k = e2;
                    } else {
                        break;
                    }
                }
            }
            out.push_str(&word);
            i = if cnt >= 2 { k } else { e };
        } else if is_hangul_char(c) {
            // 韩语：**按空白分词**（同英文口径，不管 CJK 单元）。这样 KO 白名单的单字
            // 才能命中：`그 그 사람` → `그 사람`；`그 그림` 不是同 token 重复 ⇒ 不动。
            let mut e = i;
            while e < n && is_hangul_char(chars[e]) {
                e += 1;
            }
            let word: String = chars[i..e].iter().collect();
            let foldable = REPEAT_COLLAPSIBLE_KO.contains(&word.as_str());
            let mut k = e;
            let mut cnt = 1usize;
            if foldable {
                loop {
                    let mut j = k;
                    while j < n && is_repeat_sep(chars[j]) {
                        j += 1;
                    }
                    let mut e2 = j;
                    while e2 < n && is_hangul_char(chars[e2]) {
                        e2 += 1;
                    }
                    if e2 > j && chars[j..e2] == chars[i..e] {
                        cnt += 1;
                        k = e2;
                    } else {
                        break;
                    }
                }
            }
            out.push_str(&word);
            i = if cnt >= 2 { k } else { e };
        } else if is_cjk_char(c) {
            let mut folded = false;
            // FIX-FF303-A：单字单元一律不折叠（p 从 2 起）。
            // FIX-FF303-B：**且单元必须在白名单内**（`然后然后`→折叠；`研究研究`→原样）。
            for p in 2..=3usize {
                if i + p > n || chars[i..i + p].iter().any(|&x| !is_cjk_char(x)) {
                    continue;
                }
                let unit: String = chars[i..i + p].iter().collect();
                if !is_collapsible_cjk_unit(&unit) {
                    continue;
                }
                let mut k = i + p;
                let mut cnt = 1usize;
                loop {
                    let mut j = k;
                    while j < n && is_repeat_sep(chars[j]) {
                        j += 1;
                    }
                    if j + p <= n && chars[j..j + p] == chars[i..i + p] {
                        cnt += 1;
                        k = j + p;
                    } else {
                        break;
                    }
                }
                if cnt >= 2 {
                    out.push_str(&unit);
                    i = k;
                    folded = true;
                    break;
                }
            }
            if !folded {
                out.push(c);
                i += 1;
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// 规则 C：整段重复折叠时，两份之间允许的分隔（全部标点 + 空白）。
fn is_long_repeat_sep(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '，' | '。' | '、' | '！' | '？' | '；' | '：' | ',' | '.' | '!' | '?' | ';' | ':'
        )
}

/// 规则 C：把文本切成「内容单元」——ASCII 字母数字连续段算 1 个词；CJK / 韩文每字算 1 个单元；
/// 规则 C 分隔丢弃；其余字符（引号 / 括号 / 其他符号）各自算 1 个内容单元（保证「括号内重复」
/// 不因括号被当分隔而误折）。返回 `(unit 文本, 结束字符下标[开])`。
fn rule_c_units(chars: &[char]) -> Vec<(String, usize)> {
    let n = chars.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let c = chars[i];
        if c.is_ascii_alphanumeric() {
            let mut e = i;
            while e < n && chars[e].is_ascii_alphanumeric() {
                e += 1;
            }
            out.push((chars[i..e].iter().collect(), e));
            i = e;
        } else if is_cjk_char(c) || is_hangul_char(c) {
            out.push((c.to_string(), i + 1));
            i += 1;
        } else if is_long_repeat_sep(c) {
            i += 1;
        } else {
            out.push((c.to_string(), i + 1));
            i += 1;
        }
    }
    out
}

/// 规则 C：单元序列是否为**更短单元的重复**（「哈哈哈哈…」=「哈」×n）⇒ 是则不折。
fn rule_c_units_periodic(units: &[String]) -> bool {
    let l = units.len();
    l > 0 && (1..l).any(|p| l % p == 0 && (0..l).all(|i| units[i] == units[i % p]))
}

/// 规则 C：单元序列是否含**字母 / CJK / 韩文**（纯数字或数字+符号 ⇒ 否 ⇒ 不折：电话、编号）。
fn rule_c_has_letters(units: &[String]) -> bool {
    units.iter().any(|t| {
        t.chars()
            .any(|c| c.is_alphabetic() || is_cjk_char(c) || is_hangul_char(c))
    })
}

/// 规则 C：整段是否可折（长度门 + 排除项）。
///
/// - 中 / 日 / 韩内容 ≥6 个字；英文等空白分词 ≥3 个词；
/// - 排除：纯数字 / 数字+符号；单元本身是更短单元的重复。
fn rule_c_segment_foldable(units: &[String]) -> bool {
    if units.is_empty() || !rule_c_has_letters(units) || rule_c_units_periodic(units) {
        return false;
    }
    let cjk_units = units
        .iter()
        .filter(|t| t.chars().any(|c| is_cjk_char(c) || is_hangul_char(c)))
        .count();
    if cjk_units > 0 {
        units.iter().map(|t| t.chars().count()).sum::<usize>() >= 6
    } else {
        units.len() >= 3
    }
}

/// 规则 C：折叠**紧挨着、一字不差重复的整段**，只保留第一份。
///
/// 比较忽略规则 C 分隔（标点 / 空白），两份之间只允许这些分隔；只折**完全相同**（差一字不动）。
/// 例：`我们明天去公园，我们明天去公园。` → `我们明天去公园。`；
/// `I think we should I think we should go` → `I think we should go`。
/// 单次调用也会把三连及以上一次折到位；迭代到不动点由 [`strip_fillers_conservative`] 负责。
fn collapse_long_repeats(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let units = rule_c_units(&chars);
    let m = units.len();
    if m < 2 {
        return text.to_string();
    }
    let texts: Vec<String> = units.iter().map(|(t, _)| t.clone()).collect();
    let ends: Vec<usize> = units.iter().map(|(_, e)| *e).collect();
    let mut deleted = vec![false; n];
    let mut t = 0usize;
    while t < m {
        let max_l = (m - t) / 2;
        // 取**最长**的相邻相同单元段（「整段」）；不可折（长度门/排除）则本位置跳过。
        let mut chosen = 0usize;
        for l in (1..=max_l).rev() {
            if texts[t..t + l] == texts[t + l..t + 2 * l] {
                chosen = l;
                break;
            }
        }
        if chosen == 0 || !rule_c_segment_foldable(&texts[t..t + chosen]) {
            t += 1;
            continue;
        }
        // 合并连续多份（三连重复一次折到位）。
        let mut cnt = 1usize;
        while t + (cnt + 1) * chosen <= m
            && texts[t + cnt * chosen..t + (cnt + 1) * chosen] == texts[t..t + chosen]
        {
            cnt += 1;
        }
        // 删掉第 2..cnt 份 + 第 2 份前的分隔（[第一份末单元结束, 末份末单元结束)）。
        for d in ends[t + chosen - 1]..ends[t + cnt * chosen - 1] {
            deleted[d] = true;
        }
        t += cnt * chosen;
    }
    let mut out = String::with_capacity(text.len());
    for (idx, &c) in chars.iter().enumerate() {
        if !deleted[idx] {
            out.push(c);
        }
    }
    out
}

/// 规则 D：两段之间允许的**唯一**分隔标点（逗号 / 顿号）。句末标点（。？！等）隔开的不算重说。
fn is_restart_sep(c: char) -> bool {
    matches!(c, '，' | '、' | ',')
}

/// 规则 D：汉字数字字符（前段全由它们组成 ⇒ 不折，防「三百，三百五十」这类列举被误删；
/// REFLOW-FILLER-ONCE-441 起本地实时去重在 ITN 之前执行，看到的是汉字数字）。
fn is_cn_numeral_char(c: char) -> bool {
    matches!(
        c,
        '零' | '〇'
            | '一'
            | '二'
            | '两'
            | '三'
            | '四'
            | '五'
            | '六'
            | '七'
            | '八'
            | '九'
            | '十'
            | '百'
            | '千'
            | '万'
            | '亿'
            | '点'
            | '半'
    )
}

/// 规则 D：前段是否可作为「说一半重说」被删（长度门 + 排除项）。
///
/// - 含中 / 日 / 韩字 ⇒ ≥3 个字；否则（英文等）⇒ ≥2 个词；
/// - 排除：无字母（纯数字 / 符号）；周期串（「哈哈哈」）；全为汉字数字。
fn rule_d_prefix_foldable(units: &[String]) -> bool {
    if units.is_empty() || !rule_c_has_letters(units) || rule_c_units_periodic(units) {
        return false;
    }
    if units.iter().all(|t| t.chars().all(is_cn_numeral_char)) {
        return false;
    }
    let cjk_units = units
        .iter()
        .filter(|t| t.chars().any(|c| is_cjk_char(c) || is_hangul_char(c)))
        .count();
    if cjk_units > 0 {
        units.iter().map(|t| t.chars().count()).sum::<usize>() >= 3
    } else {
        units.len() >= 2
    }
}

/// 规则 D（FILLER-RESTART-440，Gavin 2026-09-26）：折叠「说一半重说」。
///
/// 把文本按标点（规则 C 分隔中除空白外的字符）切段；相邻两段之间**只隔一个逗号 / 顿号**（其后可带空白），
/// 且前段内容单元是后段的**严格前缀**（后段更长），前段过 [`rule_d_prefix_foldable`] ⇒ 删掉前段及其后分隔。
/// 例：`你们的知，你们的知识不可能…` → `你们的知识不可能…`；`I think, I think we should go` →
/// `I think we should go`；`春天，春天来了`（前段 2 字）/ `你们的知。你们的知识`（句号隔开）不动。
/// 前后两段完全相同的不归本规则（那是规则 C 的事，且有 ≥6 字门）。
fn collapse_restarts(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let is_punct_sep = |c: char| is_long_repeat_sep(c) && !c.is_whitespace();
    // 段 = 两个标点分隔之间的内容，去掉首尾空白：(内容起, 内容止[开], 其后第一个分隔的下标)
    let mut segs: Vec<(usize, usize, usize)> = Vec::new();
    let mut i = 0usize;
    while i < n {
        let start = i;
        while i < n && !is_punct_sep(chars[i]) {
            i += 1;
        }
        let (mut a, mut b) = (start, i);
        while a < b && chars[a].is_whitespace() {
            a += 1;
        }
        while b > a && chars[b - 1].is_whitespace() {
            b -= 1;
        }
        segs.push((a, b, i));
        i += 1; // 跳过分隔
    }
    let units_of = |a: usize, b: usize| -> Vec<String> {
        rule_c_units(&chars[a..b])
            .into_iter()
            .map(|(t, _)| t)
            .collect()
    };
    let mut deleted = vec![false; n];
    for k in 0..segs.len().saturating_sub(1) {
        let (a0, b0, sep_at) = segs[k];
        let (a1, b1, _) = segs[k + 1];
        if a0 >= b0 || a1 >= b1 || sep_at >= n || !is_restart_sep(chars[sep_at]) {
            continue;
        }
        // 两段之间只许「一个逗号/顿号 + 空白」：前段内容止 → 分隔，全为空白；分隔 → 后段起，全为空白。
        if !chars[b0..sep_at].iter().all(|c| c.is_whitespace())
            || !chars[sep_at + 1..a1].iter().all(|c| c.is_whitespace())
        {
            continue;
        }
        let prev = units_of(a0, b0);
        let next = units_of(a1, b1);
        if next.len() > prev.len()
            && next[..prev.len()] == prev[..]
            && rule_d_prefix_foldable(&prev)
        {
            for d in &mut deleted[a0..a1] {
                *d = true;
            }
        }
    }
    let mut out = String::with_capacity(text.len());
    for (idx, &c) in chars.iter().enumerate() {
        if !deleted[idx] {
            out.push(c);
        }
    }
    out
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

    // ============================================================
    // FORMAT-FALLBACK-303: strip_fillers_conservative（保守语气词去除，中/英/日/韩）
    // ============================================================

    // ---- 必须摘（7 条）----

    #[test]
    fn ff303_strip_chinese_leading_filler() {
        assert_eq!(
            strip_fillers_conservative("呃，我觉得这个方案可以"),
            "我觉得这个方案可以"
        );
    }

    #[test]
    fn ff303_strip_english_leading_filler() {
        assert_eq!(strip_fillers_conservative("Um, I think so."), "I think so.");
    }

    #[test]
    fn ff303_strip_japanese_leading_filler() {
        assert_eq!(
            strip_fillers_conservative("えーと、ちょっと待って"),
            "ちょっと待って"
        );
    }

    #[test]
    fn ff303_strip_korean_leading_filler() {
        assert_eq!(strip_fillers_conservative("음, 좋아요"), "좋아요");
    }

    #[test]
    fn ff303_strip_stacked_filler() {
        assert_eq!(strip_fillers_conservative("嗯嗯，可以"), "可以");
    }

    #[test]
    fn ff303_rule_b_chinese_repeat() {
        assert_eq!(
            strip_fillers_conservative("然后然后我们开始"),
            "然后我们开始"
        );
    }

    #[test]
    fn ff303_rule_b_english_repeat() {
        assert_eq!(strip_fillers_conservative("I I think"), "I think");
    }

    // ---- 绝不许摘（边界外，8 条）----

    #[test]
    fn ff303_keep_demonstrative_nage() {
        assert_eq!(
            strip_fillers_conservative("那个文件删了吗"),
            "那个文件删了吗"
        );
    }

    #[test]
    fn ff303_keep_copula_jiushi() {
        assert_eq!(strip_fillers_conservative("问题就是这里"), "问题就是这里");
    }

    #[test]
    fn ff303_keep_midsentence_ranhou() {
        assert_eq!(
            strip_fillers_conservative("先存盘然后重启"),
            "先存盘然后重启"
        );
    }

    #[test]
    fn ff303_keep_japanese_ano() {
        assert_eq!(
            strip_fillers_conservative("あの人は誰ですか"),
            "あの人は誰ですか"
        );
    }

    #[test]
    fn ff303_keep_korean_geu() {
        assert_eq!(
            strip_fillers_conservative("그 사람이 왔어요"),
            "그 사람이 왔어요"
        );
    }

    #[test]
    fn ff303_keep_word_internal_uhura() {
        assert_eq!(
            strip_fillers_conservative("Uhura is a character."),
            "Uhura is a character."
        );
    }

    #[test]
    fn ff303_keep_midsentence_filler() {
        assert_eq!(
            strip_fillers_conservative("我觉得呃这个不行"),
            "我觉得呃这个不行"
        );
    }

    // ---- 两种文本形态（第 16/17 条，供下一单节点包装）----

    #[test]
    fn ff303_multisentence_punctuated_strips_each_sentence_head() {
        // 已带标点多句：每个句首都能摘。
        assert_eq!(
            strip_fillers_conservative("呃，我觉得可以。嗯，那就这样。"),
            "我觉得可以。那就这样。"
        );
    }

    #[test]
    fn ff303_unpunctuated_single_run_strips_only_leading() {
        // 无标点单段：只有开头那一个能摘；句中「嗯」分不出句子，保守不动。
        assert_eq!(
            strip_fillers_conservative("呃我觉得可以嗯那就这样"),
            "我觉得可以嗯那就这样"
        );
    }

    // ---- FIX-FF303-A 追加（18~27）：单字叠词不折叠 + 中文 T2 分档 ----

    #[test]
    fn ff303a_18_keep_verb_reduplication_kanakan() {
        // Gavin 真实语料：动词重叠表短时/尝试，折叠即改变语义。
        assert_eq!(
            strip_fillers_conservative("看看有什么好看的电影"),
            "看看有什么好看的电影"
        );
    }

    #[test]
    fn ff303a_19_keep_emotional_reduplication_hahaha() {
        assert_eq!(
            strip_fillers_conservative("哈哈哈太好笑了"),
            "哈哈哈太好笑了"
        );
    }

    #[test]
    fn ff303a_20_keep_other_verb_reduplication() {
        assert_eq!(strip_fillers_conservative("想想再说"), "想想再说");
        assert_eq!(strip_fillers_conservative("试试看"), "试试看");
    }

    #[test]
    fn ff303a_21_keep_aoshengtanqi() {
        assert_eq!(
            strip_fillers_conservative("唉声叹气了一整天"),
            "唉声叹气了一整天"
        );
    }

    #[test]
    fn ff303a_22_keep_nahan() {
        assert_eq!(strip_fillers_conservative("呐喊了一声"), "呐喊了一声");
    }

    #[test]
    fn ff303a_23_keep_ohuo() {
        assert_eq!(strip_fillers_conservative("哦豁完蛋了"), "哦豁完蛋了");
    }

    #[test]
    fn ff303a_24_rule_a_covers_stacked_filler() {
        // 句首叠写由规则 A 独立摘除，不依赖规则 B 的单字折叠。
        assert_eq!(strip_fillers_conservative("嗯嗯，可以"), "可以");
    }

    #[test]
    fn ff303a_25_t1_still_strips_without_boundary() {
        assert_eq!(strip_fillers_conservative("呃我觉得可以"), "我觉得可以");
    }

    #[test]
    fn ff303a_26_t2_strips_when_followed_by_punct() {
        assert_eq!(strip_fillers_conservative("唉，今天真累"), "今天真累");
    }

    #[test]
    fn ff303a_27_rule_b_multichar_still_folds() {
        assert_eq!(
            strip_fillers_conservative("然后然后我们开始"),
            "然后我们开始"
        );
    }

    // ---- FIX-FF303-B 追加（28~34 + 韩语分词）：规则 B 白名单门控 ----

    #[test]
    fn ff303b_28_keep_abab_verb_reduplication() {
        assert_eq!(
            strip_fillers_conservative("研究研究这个方案"),
            "研究研究这个方案"
        );
    }

    #[test]
    fn ff303b_29_keep_other_abab_reduplication() {
        assert_eq!(strip_fillers_conservative("讨论讨论"), "讨论讨论");
        assert_eq!(strip_fillers_conservative("商量商量"), "商量商量");
        assert_eq!(strip_fillers_conservative("休息休息"), "休息休息");
    }

    #[test]
    fn ff303b_30_keep_english_past_perfect() {
        assert_eq!(
            strip_fillers_conservative("I had had enough"),
            "I had had enough"
        );
    }

    #[test]
    fn ff303b_31_keep_english_that_that() {
        assert_eq!(
            strip_fillers_conservative("the thing that that man said"),
            "the thing that that man said"
        );
    }

    #[test]
    fn ff303b_32_fold_whitelisted_ranhou() {
        assert_eq!(
            strip_fillers_conservative("然后然后我们开始"),
            "然后我们开始"
        );
    }

    #[test]
    fn ff303b_33_fold_whitelisted_jiushi() {
        assert_eq!(
            strip_fillers_conservative("就是就是这个意思"),
            "就是这个意思"
        );
    }

    #[test]
    fn ff303b_34_fold_whitelisted_english_i() {
        assert_eq!(strip_fillers_conservative("I I think so"), "I think so");
    }

    #[test]
    fn ff303b_korean_token_fold_and_nonrepeat_safe() {
        // 韩语按空白分词：白名单词的同 token 重复折叠；不同 token 不动。
        assert_eq!(strip_fillers_conservative("그 그 사람"), "그 사람");
        assert_eq!(strip_fillers_conservative("그 그림"), "그 그림");
    }

    // ---- 幂等（第 15 条）----
    #[test]
    fn ff303_idempotent() {
        let cases = [
            "呃，我觉得这个方案可以",
            "Um, I think so.",
            "えーと、ちょっと待って",
            "음, 좋아요",
            "嗯嗯，可以",
            "然后然后我们开始",
            "I I think",
            "那个文件删了吗",
            "问题就是这里",
            "先存盘然后重启",
            "あの人は誰ですか",
            "그 사람이 왔어요",
            "Uhura is a character.",
            "我觉得呃这个不行",
            "哈哈哈",
            "看看有什么好看的电影",
            "哈哈哈太好笑了",
            "想想再说",
            "试试看",
            "唉声叹气了一整天",
            "呐喊了一声",
            "哦豁完蛋了",
            "呃我觉得可以",
            "唉，今天真累",
            "研究研究这个方案",
            "讨论讨论",
            "商量商量",
            "休息休息",
            "I had had enough",
            "the thing that that man said",
            "然后然后我们开始",
            "就是就是这个意思",
            "I I think so",
            "그 그 사람",
            "그 그림",
        ];
        for c in cases {
            let once = strip_fillers_conservative(c);
            let twice = strip_fillers_conservative(&once);
            assert_eq!(
                twice, once,
                "幂等失败（输入 {c:?}）：once={once:?} twice={twice:?}"
            );
        }
    }

    // ============================================================
    // FILLER-LONG-REPEAT-419：规则 C —— 紧挨着一字不差重复的整段只留第一份
    // ============================================================

    #[test]
    fn rule_c_chinese_exact_whole_segment_folds() {
        assert_eq!(
            strip_fillers_conservative("我们明天去公园，我们明天去公园。"),
            "我们明天去公园。"
        );
        assert_eq!(
            strip_fillers_conservative("我们明天去公园我们明天去公园。"),
            "我们明天去公园。"
        );
    }

    #[test]
    fn rule_c_triple_repeat_keeps_one() {
        assert_eq!(
            strip_fillers_conservative("我们明天去公园，我们明天去公园，我们明天去公园。"),
            "我们明天去公园。"
        );
    }

    #[test]
    fn rule_c_english_exact_folds() {
        assert_eq!(
            strip_fillers_conservative("I think we should I think we should go"),
            "I think we should go"
        );
        assert_eq!(
            strip_fillers_conservative("we should go, we should go!"),
            "we should go!"
        );
    }

    #[test]
    fn rule_c_japanese_exact_folds() {
        assert_eq!(
            strip_fillers_conservative("あしたはあめです。あしたはあめです。"),
            "あしたはあめです。"
        );
    }

    #[test]
    fn rule_c_korean_exact_folds() {
        assert_eq!(
            strip_fillers_conservative("우리 내일 공원에 가요 우리 내일 공원에 가요"),
            "우리 내일 공원에 가요"
        );
    }

    #[test]
    fn rule_c_keeps_shorter_unit_repetition() {
        // 单元本身是更短单元的重复 ⇒ 不折（归 B 或本就正常表达）。
        for s in ["哈哈哈哈哈哈", "对对对对对对", "666666", "哈哈哈哈哈"] {
            assert_eq!(strip_fillers_conservative(s), s, "不应折：{s}");
        }
    }

    #[test]
    fn rule_c_keeps_numbers() {
        for s in ["138138", "138 138 138 138 138 138", "编号 1234 1234 1234"] {
            assert_eq!(strip_fillers_conservative(s), s, "数字/编号不应折：{s}");
        }
    }

    #[test]
    fn rule_c_keeps_near_miss_one_char_diff() {
        // 差一个字就不动（近似重复交给接缝层，不在这里猜）。
        assert_eq!(
            strip_fillers_conservative("我们明天去公园，我们明天去别的。"),
            "我们明天去公园，我们明天去别的。"
        );
    }

    #[test]
    fn rule_c_keeps_abab_reduplication() {
        assert_eq!(strip_fillers_conservative("研究研究"), "研究研究");
        assert_eq!(strip_fillers_conservative("看看"), "看看");
        assert_eq!(strip_fillers_conservative("讨论讨论这个"), "讨论讨论这个");
    }

    #[test]
    fn rule_c_idempotent() {
        let t = "我们明天去公园，我们明天去公园，我们明天去公园。";
        let once = strip_fillers_conservative(t);
        assert_eq!(strip_fillers_conservative(&once), once);
    }

    // ============================================================
    // 规则 D（FILLER-RESTART-440）：折叠「说一半重说」
    // ============================================================

    #[test]
    fn rule_d_folds_half_word_restart() {
        assert_eq!(
            strip_fillers_conservative("你们的知，你们的知识不可能只在简单的仪式就变得完整"),
            "你们的知识不可能只在简单的仪式就变得完整"
        );
    }

    #[test]
    fn rule_d_folds_phrase_restart() {
        assert_eq!(
            strip_fillers_conservative("我觉得，我觉得这个方案可以"),
            "我觉得这个方案可以"
        );
        assert_eq!(
            strip_fillers_conservative("I think, I think we should go"),
            "I think we should go"
        );
    }

    #[test]
    fn rule_d_keeps_below_length_gate() {
        // 注：`I, I think` 由规则 B（英文白名单叠词，可隔逗号）折成 `I think`，属既有行为，不在本条验证。
        for t in ["春天，春天来了", "看看，看看这个", "cat, cat food"] {
            assert_eq!(strip_fillers_conservative(t), t, "{t}");
        }
    }

    #[test]
    fn rule_d_keeps_sentence_final_separator() {
        let t = "你们的知。你们的知识";
        assert_eq!(strip_fillers_conservative(t), t);
    }

    #[test]
    fn rule_d_keeps_numbers() {
        for t in ["123，1234", "三百，三百五十", "一二三，一二三四"] {
            assert_eq!(strip_fillers_conservative(t), t, "{t}");
        }
    }

    #[test]
    fn rule_d_keeps_non_prefix_and_equal() {
        // 后段不以前段开头 / 前后完全相同（<6 字，不归规则 C）⇒ 不动。
        for t in ["你们的爱，他们的爱人", "你们的知，你们的知"] {
            assert_eq!(strip_fillers_conservative(t), t, "{t}");
        }
    }

    #[test]
    fn rule_d_multi_restart_reaches_fixpoint() {
        assert_eq!(
            strip_fillers_conservative("你们的知，你们的知，你们的知识"),
            "你们的知识"
        );
    }

    #[test]
    fn rule_d_gavin_full_text_only_removes_restart() {
        let before = "轮回的目的是要学习更多，不断学更多东西，因为你不可能再一次简单的人事，就把一切通通学会。重生的主要目的，并不是为了改正，而是去增加学习和体验，你们的知，你们的知识不可能只在简单的仪式就变得完整。你必须活过许多人事，才能完全明了你给自己指定的课题";
        let after = before.replacen("你们的知，", "", 1);
        assert_eq!(strip_fillers_conservative(before), after);
    }

    #[test]
    fn rule_d_idempotent() {
        let t = "你们的知，你们的知，你们的知识。我觉得，我觉得可以";
        let once = strip_fillers_conservative(t);
        assert_eq!(strip_fillers_conservative(&once), once);
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
