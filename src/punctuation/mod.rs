use std::path::{Path, PathBuf};

use crate::transcription;

const PUNCT_MODEL_SUBDIR: &str = "punct-ct-transformer-zh";

/// 短句阈值：字/词数 <= 5 时不加末尾标点（Gavin 2026-08-08 拍板）
///
/// PUNCT-GOVERNANCE-030-A ①：字/词数小于等于此值时剥末尾标点。
/// 仅开关开启时生效（开关关闭时全文已剥，自动覆盖，见 ⑥）。
pub const SHORT_TEXT_UNIT_THRESHOLD: usize = 5;

/// 全量标点字符集（PUNCT-GOVERNANCE-030-A A1 扩充版）
///
/// 覆盖 Gavin 要求「任何标点都剥」的集合：
/// - 中文：，。！？；：、""''…——（含 ——）（）【】《》「」『』间隔号 ·
/// - 英文：, . ! ? ; : " ' ( ) [ ]（半角句点 . 由调用方按小数点/URL 保护逻辑豁免）
///
/// 注：半角句点 `.` 在集合内，但 strip_punctuation 对数字间小数点与
/// 字母数字夹持的域名点（example.com）做保护（实测结论，见 result.md）。
pub const PUNCT_CHARS: &[char] = &[
    '，', '。', '！', '？', '；', '：', '、', '\u{201C}', '\u{201D}', '\u{2018}',
    '\u{2019}', // “ ” ‘ ’
    '…', '—', '（', '）', '【', '】', '《', '》', '「', '」', '『', '』', '·', ',', '!', '?', ';',
    ':', '"', '\'', '(', ')', '[', ']', '.',
];

/// 句末终结标点字符集（PUNCT-GOVERNANCE-030-A-2 新增）
///
/// `strip_trailing_punctuation` 专用：只含「句末终结类」标点。末尾剥离的意图是
/// 去掉句末终结标点，不是所有标点都剥。
///
/// - 中文：。！？；，、…— **：**（全角冒号，030-A-2 验收补：与半角 `:` 同族，不能漏）
/// - 半角：.!?;:,
///
/// 🔴 与 `PUNCT_CHARS` 逐字符对照，**刻意不进本集合**的字符及理由（验收返工显式化，
/// 防以后再漏）：
/// - 成对符号右半：`）】》」』""''()[]` —— 不是句末终结标点，剥掉会让左半成孤儿
///   （`他说"好"` → `他说"好`）
/// - 成对符号左半：`（【《「『""` —— 剥掉同样破坏成对性（末尾罕见，但不剥）
/// - `·` 间隔号：非句末终结符（`3·14` 是词内间隔）
/// - 无其他：全角 `：`/半角 `:` 都在，全角 `，`/`；`/`、`/`…`/`—` 与半角
///   `,`/`;` 都在 —— 每个终结类字符全角半角同进同出，无同族挑掉
pub const TRAILING_PUNCT_CHARS: &[char] = &[
    '。', '！', '？', '；', '，', '、', '…', '—', '：', '.', '!', '?', ';', ',', ':',
];

/// 判断字符是否属于标点字符集
pub fn is_punctuation(c: char) -> bool {
    PUNCT_CHARS.contains(&c)
}

/// 判定文本是否含「有效标点」（PUNCT-GOVERNANCE-030-A-2，主控 c 方案）
///
/// 对 `PUNCT_CHARS` 全集合统一做「词内嵌豁免」：字符虽属标点集合，但若同时被
/// 左右两个 ASCII 字母/数字夹住（`3.14` 小数点、`don't` 缩写撇号、`3:30` 时间
/// 冒号、`example.com` 域名点），视为词内嵌入字符而非标点语义，不计为有效标点。
/// 任何其余（未被夹持）的标点字符出现即返回 `true`。
///
/// 判据与 `strip_trailing_punctuation` 共用「什么算标点」的边界口径（同源、不分裂），
/// 是一条覆盖全部同族的机制性规则，而非逐个字符打补丁。
///
/// 已知口径（主控 2026-08-08 裁定，勿当 bug 重修）：
/// - 无句号仅引号（`他说“好”`）→ 有效标点 `true`（引号虽非句子级标点，但属标点，
///   主控裁定接受，跳过标点引擎；与句子级方案的差异已钉测试）
/// - 全角数字（`３.１４`）两侧非 ASCII 字母数字 → 不豁免 → `true`，与半角行为
///   不一致（已实测钉测试，只报不改）
pub fn has_effective_punctuation(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if !is_punctuation(c) {
            continue;
        }
        let prev = chars.get(i.wrapping_sub(1)).copied().unwrap_or(' ');
        let next = chars.get(i + 1).copied().unwrap_or(' ');
        if prev.is_ascii_alphanumeric() && next.is_ascii_alphanumeric() {
            continue;
        }
        return true;
    }
    false
}

/// 统计文本的"字/词"单位数（PUNCT-GOVERNANCE-030-A B1，口径 Gavin 2026-08-08）
///
/// - CJK 表意文字 + 日文假名（Hiragana/Katakana）→ 逐字符计 1
/// - 其余连续非空白片段（拉丁字母、韩文 Hangul、数字）→ 每个空格分隔片段计 1
/// - 标点、空白不计数；韩文按词计不按音节计（韩文用空格分词）
///
/// 例：GPT很好用 = 1词 + 3字 = 4
pub fn count_units(text: &str) -> usize {
    let mut units = 0usize;
    let mut in_word = false;
    for c in text.chars() {
        if c.is_whitespace() || is_punctuation(c) {
            in_word = false;
            continue;
        }
        if is_cjk_ideograph_or_kana(c) {
            units += 1;
            in_word = false;
        } else if !in_word {
            units += 1;
            in_word = true;
        }
    }
    units
}

/// 是否 CJK 表意文字或日文假名（逐字符计数的字符）
fn is_cjk_ideograph_or_kana(c: char) -> bool {
    (c >= '\u{3040}' && c <= '\u{30FF}') // Hiragana + Katakana
        || (c >= '\u{4E00}' && c <= '\u{9FFF}') // CJK Unified Ideographs
        || (c >= '\u{3400}' && c <= '\u{4DBF}') // CJK Extension A
        || (c >= '\u{F900}' && c <= '\u{FAFF}') // CJK Compatibility Ideographs
}

/// 从末尾循环剥离标点直到末字符非标点（PUNCT-GOVERNANCE-030-A B2）
///
/// 处理连续标点：好？！ → 好，话…… → 话
///
/// 🔴 与 strip_punctuation（A，标点换空格）不同：末尾标点**直接删除、不留空格**，
/// 删完 trim_end。理由：末尾留空格是垃圾字符，注入后光标前多一空格，体验更差。
/// Gavin 说的「留空格」针对句中标点造成的词间粘连，不是句末。
///
/// 🔴 PUNCT-GOVERNANCE-030-A-2：只剥 `TRAILING_PUNCT_CHARS`（句末终结标点），
/// **不剥成对符号右半**（`他说"好"` 的右引号 / `（笑）` 的右括号）——那些不是
/// 句末终结标点，剥掉会让左半成孤儿。
pub fn strip_trailing_punctuation(text: &str) -> String {
    let mut s = text.trim_end().to_string();
    while let Some(c) = s.chars().next_back() {
        if TRAILING_PUNCT_CHARS.contains(&c) {
            s.pop();
        } else {
            break;
        }
    }
    s.trim_end().to_string()
}

pub struct PunctuationEngine {
    punct: sherpa_onnx::OfflinePunctuation,
}

impl PunctuationEngine {
    pub fn new(model_dir: &Path) -> Option<Self> {
        let punct_dir = model_dir.join(PUNCT_MODEL_SUBDIR);
        let model_path = punct_dir.join("model.onnx");

        if !model_path.exists() {
            log::warn!(
                "Punctuation model not found at {:?}, disabling punctuation",
                model_path
            );
            return None;
        }

        let model_path_str = model_path.to_str()?.to_string();

        let mut config = sherpa_onnx::OfflinePunctuationConfig::default();
        config.model.ct_transformer = Some(model_path_str);
        config.model.num_threads = 1;
        config.model.debug = false;
        config.model.provider = Some("cpu".to_string());

        match sherpa_onnx::OfflinePunctuation::create(&config) {
            Some(punct) => {
                log::info!("Punctuation model loaded from {:?}", punct_dir);
                Some(Self { punct })
            }
            None => {
                log::error!("Failed to create OfflinePunctuation from {:?}", punct_dir);
                None
            }
        }
    }

    pub fn add_punctuation(&mut self, text: &str) -> Option<String> {
        let result = self.punct.add_punctuation(text);
        result.map(|s| convert_punctuation_for_english(&s))
    }

    pub fn model_dir() -> PathBuf {
        transcription::model_dir()
    }
}

fn convert_punctuation_for_english(text: &str) -> String {
    let ascii_letter_count = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
    let total_chars = text
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .count();
    if total_chars == 0 {
        return text.to_string();
    }
    let ratio = ascii_letter_count as f64 / total_chars as f64;
    if ratio > 0.5 {
        text.replace('，', ",")
            .replace('。', ".")
            .replace('？', "?")
            .replace('、', ",")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_punctuation_model_subdir_constant() {
        assert_eq!(PUNCT_MODEL_SUBDIR, "punct-ct-transformer-zh");
    }

    #[test]
    fn test_model_dir_delegates_to_transcription() {
        let dir = PunctuationEngine::model_dir();
        assert!(dir.ends_with("models") || dir.to_string_lossy().contains("models"));
    }

    #[test]
    fn test_convert_punctuation_pure_english() {
        let input = "Hello world，this is a test。How are you？";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, "Hello world,this is a test.How are you?");
    }

    #[test]
    fn test_convert_punctuation_mixed_chinese_no_convert() {
        let input = "今天天气真好，我们去公园玩吧。";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_convert_punctuation_mixed_english_heavy() {
        let input = "We used Python，built an API，and deployed it。OK？";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, "We used Python,built an API,and deployed it.OK?");
    }

    #[test]
    fn test_convert_punctuation_boundary_ratio_exactly_50() {
        // 2 ASCII letters + 2 Chinese chars = 4 alphanumeric total, ratio = 2/4 = 0.5
        // Threshold is > 0.5, so exactly 0.5 should NOT convert
        let input = "ab中文";
        // First add some punctuation marks to verify they stay
        let input_punct = "ab，中文。";
        let output = convert_punctuation_for_english(input_punct);
        assert_eq!(
            output, input_punct,
            "Exactly 50% ratio should NOT convert (threshold is >0.5)"
        );
    }

    #[test]
    fn test_convert_punctuation_empty_text() {
        let input = "";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, "", "Empty text should return empty");
    }

    #[test]
    fn test_convert_punctuation_only_punctuation() {
        // Only punctuation marks: total_chars == 0, ratio would divide by zero
        // Function should return text as-is
        let input = "，。？！";
        let output = convert_punctuation_for_english(input);
        assert_eq!(
            output, input,
            "Punctuation-only text should be returned unchanged"
        );
    }

    #[test]
    fn test_convert_punctuation_just_above_threshold() {
        // 7 letters + 1 number + 1 space + 2 punct = 11 chars, 7 letters = 63.6%
        let input = "abcdefg，h1。";
        let output = convert_punctuation_for_english(input);
        assert_eq!(
            output, "abcdefg,h1.",
            "Ratio > 0.5 should convert to half-width"
        );
    }

    #[test]
    fn test_convert_punctuation_numbers_not_counted_as_letters() {
        // Numbers don't count as ASCII letters in the ratio
        let input = "123，456。789？";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, input, "Numbers-only should not trigger conversion");
    }

    // ============================================================
    // PUNCT-GOVERNANCE-030-A B1/B2: count_units / strip_trailing_punctuation
    // ============================================================
    #[test]
    fn test_count_units_chinese() {
        assert_eq!(count_units("好的"), 2);
        assert_eq!(count_units("我知道了"), 4);
        assert_eq!(count_units("这个方案可以"), 6);
    }

    #[test]
    fn test_count_units_english() {
        assert_eq!(count_units("OK"), 1);
        assert_eq!(count_units("thank you"), 2);
        assert_eq!(count_units("hello world"), 2);
    }

    #[test]
    fn test_count_units_mixed() {
        assert_eq!(count_units("GPT很好用"), 4);
    }

    #[test]
    fn test_count_units_punct_and_space_not_counted() {
        assert_eq!(count_units("你好，世界"), 4);
        assert_eq!(count_units("好 好"), 2);
    }

    #[test]
    fn test_count_units_empty() {
        assert_eq!(count_units(""), 0);
        assert_eq!(count_units("，。！？"), 0);
    }

    /// TEST-SYNC-030 3.3：口语口补齐（Gavin 2026-08-08 拍板口径）。
    /// - 韩文按空格分词：Hangul 音节不在 CJK/假名范围 → 走「空格分段计 1」分支
    /// - 日文假名逐字符计
    /// - 中英混合边界 + 内部数字
    /// - 纯数字（小数点是标点，重置词边界）
    /// - 阈值边界恰好 5 与 6：分别为 apply_l2_postprocess 的 ≤5 判据上下沿
    #[test]
    fn test_count_units_korean_space_segmented() {
        assert_eq!(count_units("안녕 세계"), 2);
        assert_eq!(count_units("안녕하세요"), 1);
        assert_eq!(count_units("처음 뵙겠습니다"), 2);
    }

    #[test]
    fn test_count_units_japanese_kana_per_char() {
        assert_eq!(count_units("ありがとう"), 5);
        assert_eq!(count_units("こんにちは"), 5);
        assert_eq!(count_units("アリガトウ"), 5);
    }

    #[test]
    fn test_count_units_zh_en_digit_mixed() {
        // 我用(2) + GPT(1词) + 写了(2) + 3(1词) + 个方案(3) = 9
        assert_eq!(count_units("我用GPT写了3个方案"), 9);
    }

    #[test]
    fn test_count_units_pure_digits_decimal() {
        // 3(1) .(标点重置) 14(1) = 2
        assert_eq!(count_units("3.14"), 2);
        assert_eq!(count_units("3.14.15"), 3);
    }

    #[test]
    fn test_count_units_threshold_boundary_5_and_6() {
        // 阈值边界：恰 5 与 6 各一条，锁死 L2 用 ≤5 而非 <5（配合缺口 1 规格表）。
        assert_eq!(count_units("一二三四五"), 5);
        assert_eq!(count_units("一二三四五六"), 6);
        assert_eq!(count_units("hello"), 1);
        assert_eq!(count_units("hello world foo bar baz"), 5);
        assert_eq!(count_units("hello world foo bar baz qux"), 6);
    }

    #[test]
    fn test_strip_trailing_punctuation_single() {
        assert_eq!(strip_trailing_punctuation("好吗？"), "好吗");
        assert_eq!(strip_trailing_punctuation("OK!"), "OK");
        assert_eq!(strip_trailing_punctuation("这个方案可以。"), "这个方案可以");
    }

    #[test]
    fn test_strip_trailing_punctuation_consecutive() {
        assert_eq!(strip_trailing_punctuation("好？！"), "好");
        assert_eq!(strip_trailing_punctuation("话……"), "话");
        assert_eq!(strip_trailing_punctuation("我们走。！？"), "我们走");
    }

    #[test]
    fn test_strip_trailing_punctuation_no_trailing_noop() {
        assert_eq!(strip_trailing_punctuation("好的"), "好的");
        assert_eq!(strip_trailing_punctuation("这个方案可以"), "这个方案可以");
        assert_eq!(strip_trailing_punctuation(""), "");
    }

    #[test]
    fn test_strip_trailing_punctuation_trim_end() {
        // 末尾直接删除、不留空格
        assert_eq!(strip_trailing_punctuation("太好了！"), "太好了");
    }

    #[test]
    fn test_strip_trailing_punctuation_keeps_pair_symbols_right_half() {
        // PUNCT-GOVERNANCE-030-A-2：成对符号右半不是句末终结标点，不剥
        assert_eq!(
            strip_trailing_punctuation("他说\u{201C}好\u{201D}"),
            "他说\u{201C}好\u{201D}"
        );
        assert_eq!(strip_trailing_punctuation("（笑）"), "（笑）");
        assert_eq!(strip_trailing_punctuation("重点【三】"), "重点【三】");
    }

    #[test]
    fn test_strip_trailing_punctuation_terminal_after_closer() {
        // 结束标点在成对符号外侧时仍剥（天然满足：碰到右引号不在集合即停）
        assert_eq!(
            strip_trailing_punctuation("\u{201C}好\u{201D}。"),
            "\u{201C}好\u{201D}"
        );
        assert_eq!(strip_trailing_punctuation("（好的）。"), "（好的）");
    }

#[test]
    fn test_strip_trailing_punctuation_comma_dash() {
        // 逗号/分号/半角冒号等句末出现的标点也剥（TRAILING 集合含之）
        assert_eq!(strip_trailing_punctuation("好的，"), "好的");
        assert_eq!(strip_trailing_punctuation("好的；"), "好的");
        assert_eq!(strip_trailing_punctuation("好——"), "好");
        assert_eq!(strip_trailing_punctuation("OK,"), "OK");
        assert_eq!(strip_trailing_punctuation("OK;"), "OK");
        assert_eq!(strip_trailing_punctuation("OK:"), "OK");
        // 全角冒号也在 TRAILING 集合（030-A-2 验收补：与半角 : 同族同进同出）
        // 短句「重点：」也剥 → 「重点」，与「重点，」行为一致
        assert_eq!(strip_trailing_punctuation("重点："), "重点");
        assert_eq!(strip_trailing_punctuation("好的："), "好的");
    }

    /// TEST-SYNC-030 3.4 补充：TRAILING_PUNCT_CHARS 全量参数化——每个成员都必须能剥。
    /// 用「文本 + 目标字符」包裹，锁定集合内每个终结标点单独在句末时都剥掉（A2 验收谱系）。
    #[test]
    fn test_strip_trailing_punctuation_every_member_strips() {
        for &c in TRAILING_PUNCT_CHARS {
            let input = format!("测试文本{c}");
            let out = strip_trailing_punctuation(&input);
            assert_eq!(
                out, "测试文本",
                "TRAILING_PUNCT_CHARS 成员 {c:?} 句末应被剥掉"
            );
        }
    }

    /// 030-A-2 反向参数化：成对符号右半 + `·` 逐字符确认不被剥（与 partition 护栏对照）。
    #[test]
    fn test_strip_trailing_punctuation_non_members_kept() {
        for c in ['）', '】', '》', '」', '』', '\u{201D}', '\'', ')', ']', '·'] {
            let input = format!("测试{c}");
            assert_eq!(
                strip_trailing_punctuation(&input),
                input.as_str(),
                "成对右半/间隔号 {c:?} 不剥"
            );
        }
    }

    #[test]
    fn test_short_text_unit_threshold_constant() {
        assert_eq!(SHORT_TEXT_UNIT_THRESHOLD, 5);
    }

    // ============================================================
    // PUNCT-GOVERNANCE-030-A-2: has_effective_punctuation（主控 c 方案）
    // ============================================================

    /// 主控 2026-08-08 验收清单：词内嵌字符（小数点/缩写撇号/时间冒号/域名点）
    /// 全部豁免 → false；真标点（句末）→ true。
    #[test]
    fn test_has_effective_punctuation_acceptance_list() {
        // 修复目标：长句带小数 → false → 走引擎补句号
        assert!(!has_effective_punctuation("圆周率是3.14"));
        assert!(!has_effective_punctuation("3.14"));
        assert!(!has_effective_punctuation("example.com"));
        assert!(!has_effective_punctuation("I don't know"));
        assert!(!has_effective_punctuation("3:30 开会"));
        // 真句末标点 → true
        assert!(has_effective_punctuation("今天天气不错。"));
        assert!(has_effective_punctuation("Hello, world."));
        assert!(has_effective_punctuation("안녕하세요."));
        // 无标点 → false
        assert!(!has_effective_punctuation("好的"));
        assert!(!has_effective_punctuation(""));
    }

    /// 已知接受边界（PUNCT-GOVERNANCE-030-A-2 附加要求 ①）
    ///
    /// 两方案差异点：`他说"好"` 仅引号无句号 —— 本方案判 true（跳过标点引擎），
    /// 句子级标点方案判 false（走引擎补标点）。主控裁定接受本方案口径
    /// （引号无句号不是用户会报的缺陷），此测试钉住不该改动，将来勿当 bug 重修。
    #[test]
    fn test_effective_punctuation_quotes_alone_boundary() {
        assert!(has_effective_punctuation("他说\u{201C}好\u{201D}"));
        assert!(has_effective_punctuation("I said \u{201C}hi\u{201D}"));
    }

    /// 附加要求 ②（只报不改，已实测现状并单列至 result.md）：
    /// 夹持判据用 is_ascii_alphanumeric，全角数字（３.１４）两侧非 ASCII →
    /// 不豁免 → 判 true，与半角行为不一致。本测试只为记录，不改判据。
    #[test]
    fn test_effective_punctuation_fullwidth_digits_divergence() {
        assert!(has_effective_punctuation("３.１４"));
    }

    /// 边界确认项（主控 2026-08-08，只查不改）：中文时间「3：30」用全角冒号。
    /// 全角冒号位于 PUNCT_CHARS，但两侧是半角数字 → 夹持豁免 → 判 false，与
    /// 半角「3:30」行为一致（非缺陷）。本测试钉住实测现状。
    #[test]
    fn test_effective_punctuation_fullwidth_colon_time() {
        assert!(!has_effective_punctuation("3：30 开会"));
        assert!(has_effective_punctuation("3：30 开会，请准时。"));
    }

    /// TEST-SYNC-030 3.4：DEC-047 已知边界钉现状（主控 2026-08-08 确认口径）。
    /// 补 coder-2 已钉的两条之外的边界：
    /// - `https://a.com` 的 `:` → 前是 `s`（ASCII 字母）、后是 `/`（非字母数字）→
    ///   不是两侧 ASCII 夹持 → 不计词内豁免 → 判 true。
    /// - 位置 0 的标点（`。你好`）→ 前驱取 `' '` 哨兵、后继非 ASCII → 不豁免 → true。
    /// - 空串 → false。
    #[test]
    fn test_effective_punctuation_url_colon_single_sided() {
        assert!(has_effective_punctuation("https://a.com"));
        assert!(has_effective_punctuation("https://example.com/path"));
    }

    #[test]
    fn test_effective_punctuation_punct_at_position_zero() {
        assert!(has_effective_punctuation("。你好"));
        assert!(has_effective_punctuation("！提醒"));
        assert!(has_effective_punctuation("，等等"));
    }

    #[test]
    fn test_effective_punctuation_empty_string() {
        assert!(!has_effective_punctuation(""));
    }

    /// PUNCT_CHARS ↔ TRAILING_PUNCT_CHARS 逐字符对照护栏（030-A-2 验收返工补）。
    ///
    /// 防「同族挑掉一个」反复复发：PUNCT_CHARS 中每个**终结类**字符（非成对符号、
    /// 非间隔号）必须 ∈ TRAILING；反之 TRAILING ⊆ PUNCT 由构造保证。刻意不在
    /// TRAILING 的只有两类，显式枚举断言：
    /// - 成对符号（开/闭引号、括号）：剥掉会让左半成孤儿
    /// - `·` 间隔号：非句末终结符
    #[test]
    fn test_trailing_chars_partition_pair_symbols_and_separator() {
        let paired: &[char] = &[
            '（', '）', '【', '】', '《', '》', '「', '」', '『', '』', '\u{201C}', '\u{201D}',
            '\u{2018}', '\u{2019}', '(', ')', '[', ']', '"', '\'',
        ];
        let non_trailing: Vec<char> = PUNCT_CHARS
            .iter()
            .copied()
            .filter(|c| !TRAILING_PUNCT_CHARS.contains(c))
            .collect();
        for c in &non_trailing {
            assert!(
                paired.contains(c) || *c == '·',
                "non-trailing char {c:?} must be a paired symbol or the interpunct ·; add it to TRAILING if it is a terminal point"
            );
        }
        // TRAILING 每个成员必须是终结类且在 PUNCT 里
        for c in TRAILING_PUNCT_CHARS {
            assert!(
                PUNCT_CHARS.contains(c),
                "trailing char {c:?} must be in PUNCT_CHARS"
            );
            assert!(
                !paired.contains(c),
                "trailing must not contain paired symbol {c:?}"
            );
        }
    }
}
