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

/// 「某字符是否为**有效标点**」的唯一判据（词内嵌豁免，PUNCT-GOVERNANCE-030-A-2）。
///
/// 字符虽属 `PUNCT_CHARS`，但若同时被左右两个 ASCII 字母/数字夹住
/// （`3.14` 小数点、`don't` 缩写撇号、`3:30` 时间冒号、`example.com` 域名点），
/// 视为词内嵌入字符而非标点语义；其余（未被夹持）的标点字符即「有效标点」。
///
/// 🔴 `has_effective_punctuation` 与 `strip_effective_punctuation`
/// （PUNCT-FINAL-REDO-350）**必须共用本谓词，不得各写一份循环** —— 两份实现早晚分家
/// （`[FIRST-MARKER-BOUNDARY-001]` / `[CONFIG-MIRROR-DRIFT-001]` 教训）。
/// 位置 0 的前驱取 `' '` 哨兵（`usize::wrapping_sub(1)` ⇒ `get` 取不到），末尾后继同理。
fn is_effective_punctuation(c: char, prev: char, next: char) -> bool {
    is_punctuation(c) && !(prev.is_ascii_alphanumeric() && next.is_ascii_alphanumeric())
}

/// 判定文本是否含「有效标点」（PUNCT-GOVERNANCE-030-A-2，主控 c 方案）
///
/// 对 `PUNCT_CHARS` 全集合统一做「词内嵌豁免」（谓词见 `is_effective_punctuation`）。
/// 任何其余（未被夹持）的标点字符出现即返回 `true`。
///
/// 判据与 `strip_trailing_punctuation` 共用「什么算标点」的边界口径（同源、不分裂），
/// 是一条覆盖全部同族的机制性规则，而非逐个字符打补丁。
///
/// 已知口径（主控 2026-08-08 裁定，勿当 bug 重修）：
/// - 无句号仅引号（`他说“好”`）→ 有效标点 `true`（引号虽非句子级标点，但属标点，
///   主控裁定接受；与句子级方案的差异已钉测试）
/// - 全角数字（`３.１４`）两侧非 ASCII 字母数字 → 不豁免 → `true`，与半角行为
///   不一致（已实测钉测试，只报不改）
pub fn has_effective_punctuation(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let prev = chars.get(i.wrapping_sub(1)).copied().unwrap_or(' ');
        let next = chars.get(i + 1).copied().unwrap_or(' ');
        if is_effective_punctuation(c, prev, next) {
            return true;
        }
    }
    false
}

/// 剥离文本中的**有效标点**，返回剥离后的文本（PUNCT-FINAL-REDO-350）。
///
/// 是 `has_effective_punctuation` 的**严格对偶**：两者共用 `is_effective_punctuation`，
/// 因此恒有 `has_effective_punctuation(strip_effective_punctuation(t)) == false`
/// （已由穷举性质测试覆盖，见本文件 `test_350_*`）。
///
/// 🔴 **词内嵌字符一字不剥**：`3.14` / `3:30` / `example.com` / `don't` / `3.5亿`
/// 剥完**逐字不变** —— 剥错会把 `3.14` 变 `314`、`3:30` 变 `330`，毁掉 ITN
/// （DEC-030/036）规整出来的结果。
///
/// 用途：CT-Transformer 重打标点前先清空已有标点，从结构上根除
/// 「对已带标点文本二次打点 ⇒ `。。` 叠加」（PUNCT-DOUBLE-334 的老 bug）——
/// 输入无标点 ⇒ 不可能叠加。
pub fn strip_effective_punctuation(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let prev = chars.get(i.wrapping_sub(1)).copied().unwrap_or(' ');
        let next = chars.get(i + 1).copied().unwrap_or(' ');
        if !is_effective_punctuation(c, prev, next) {
            out.push(c);
        }
    }
    out
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

/// KOJA-PUNCT-464：文本是否含标点模型（CT-Transformer，中英词表）处理不了的文字 —— 日文假名 / 韩文。
///
/// 实测（`poc464_preview_punct`，证据 `collab/evidence/464/preview-punct.md`）：韩文送进去**空格全被吃掉**
/// （「조금만 생각을」→「조금만생각을」），日文只在句末补「。」、无句中标点。含这两种文字 ⇒ 不送标点模型。
/// 只看假名 / 谚文：日文汉字与中文同区，单凭汉字判不出日文，有假名才算日文。
pub fn ct_unsupported_script(text: &str) -> bool {
    text.chars().any(|c| {
        ('\u{3040}'..='\u{30FF}').contains(&c) // 平假名 + 片假名
            || ('\u{31F0}'..='\u{31FF}').contains(&c) // 片假名音标扩展
            || ('\u{FF66}'..='\u{FF9D}').contains(&c) // 半角片假名
            || ('\u{AC00}'..='\u{D7A3}').contains(&c) // 谚文音节
            || ('\u{1100}'..='\u{11FF}').contains(&c) // 谚文字母
            || ('\u{3130}'..='\u{318F}').contains(&c) // 谚文兼容字母
    })
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

/// L2 后处理走过的分支（PUNCT-GOVERNANCE-030-E，日志方案 C）
///
/// 供调用方区分「剥了哪条分支」打对应日志；也与 `apply_l2_postprocess` 返回值
/// 一起供单测断言「判定走了哪条」，避免只用字符串相等（文本本就无末尾标点时会
/// 与 no-op 结果相同，字符串断言失去判别力）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L2Action {
    /// (a) 开关关闭 → 全文剥标点
    StripAll,
    /// (b) 开关开启且（恒剥尾开关开启 或 字/词数 <= 5）→ 剥末尾标点
    StripTrailing,
    /// (c) 其余 → 原样返回
    NoOp,
}

/// L2 后处理决策（PUNCT-GOVERNANCE-030-E：纯函数，从 run_pipeline_core 抽出以便单测）
///
/// 判定矩阵（tester-1 规格，行为较 main.rs 内联版零变更）：
/// (a) `punctuation_enabled=false` → `transcription::strip_punctuation`（标点换空格、
///     连续空格合一、trim），action = `StripAll`
/// (b) `true` 且（`strip_trailing_always` 或 `count_units <= SHORT_TEXT_UNIT_THRESHOLD`）
///     → `strip_trailing_punctuation`（直接删除不留空格），action = `StripTrailing`
/// (c) 其余 → 原样返回，action = `NoOp`（**最大回归风险点**：>5 单位且未开恒剥尾的长句
///     必须逐字符不动）
///
/// note：返回值与 action 是否「实际改动」无关——`StripAll`/`StripTrailing` 分支若文本
/// 本就无标点会返回原串（日志侧按 `!=` 判定是否打，沿用现码行为）。
///
/// V091-PUNCT-TAIL-214：`strip_trailing_always` 由配置项 `punctuation.strip_trailing` 驱动。
/// 🔴 全剥（a）优先于恒剥尾（b）：关标点时本就不该留任何标点，新开关不应把它降级成只剥尾。
pub fn apply_l2_postprocess(
    text: &str,
    punctuation_enabled: bool,
    strip_trailing_always: bool,
) -> (String, L2Action) {
    if !punctuation_enabled {
        (transcription::strip_punctuation(text), L2Action::StripAll)
    } else if strip_trailing_always || count_units(text) <= SHORT_TEXT_UNIT_THRESHOLD {
        (strip_trailing_punctuation(text), L2Action::StripTrailing)
    } else {
        (text.to_string(), L2Action::NoOp)
    }
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

    /// KOJA-PUNCT-464：含假名 / 谚文 ⇒ 标点模型处理不了；纯中英（含日文汉字单独出现）照常送。
    #[test]
    fn koja464_ct_unsupported_script() {
        assert!(ct_unsupported_script("조금만 생각을 하면서"));
        assert!(ct_unsupported_script("うちの中学は弁当制"));
        assert!(ct_unsupported_script("カタカナ"));
        assert!(ct_unsupported_script("ｶﾀｶﾅ"));
        assert!(ct_unsupported_script("今天说了一句 안녕하세요"));
        assert!(!ct_unsupported_script("开放时间早上九点至下午五点"));
        assert!(!ct_unsupported_script(
            "the tribal chieftain called for the boy"
        ));
        assert!(
            !ct_unsupported_script("中学"),
            "只有汉字判不出日文 ⇒ 照常送"
        );
        assert!(!ct_unsupported_script(""));
    }

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
        for c in [
            '）', '】', '》', '」', '』', '\u{201D}', '\'', ')', ']', '·',
        ] {
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

    // ============================================================
    // PUNCT-FINAL-REDO-350: strip_effective_punctuation（has_ 的严格对偶）
    // ============================================================

    /// 🔴 性质测试：对任意 t，`has_effective_punctuation(strip_effective_punctuation(t)) == false`。
    /// 用小字母表做**穷举**（长度 ≤ 4，覆盖 CJK / ASCII 字母数字 / 全半角标点 / 空白 / 撇号），
    /// 这是「严格对偶」这一不变量最直接的机器证明。
    #[test]
    fn test_350_strip_is_strict_dual_of_has_exhaustive() {
        const ALPHABET: &[char] = &['3', '.', '1', 'a', '，', '。', ':', '中', ' ', '\'', '５'];
        let mut corpus: Vec<String> = vec![String::new()];
        let mut frontier: Vec<String> = vec![String::new()];
        for _ in 0..4 {
            let mut next = Vec::with_capacity(frontier.len() * ALPHABET.len());
            for base in &frontier {
                for &c in ALPHABET {
                    let mut s = base.clone();
                    s.push(c);
                    next.push(s.clone());
                    corpus.push(s);
                }
            }
            frontier = next;
        }
        for t in &corpus {
            let stripped = strip_effective_punctuation(t);
            assert!(
                !has_effective_punctuation(&stripped),
                "严格对偶被破坏：strip({t:?}) = {stripped:?} 仍含有效标点"
            );
            // 幂等：再剥一次不改变结果
            assert_eq!(strip_effective_punctuation(&stripped), stripped);
        }
    }

    /// 词内嵌边界外用例：剥完**逐字不变**（剥错会毁 ITN 的 `3.14` / `3:30` / `3.5亿`）。
    #[test]
    fn test_350_inline_punctuation_survives_strip() {
        for t in [
            "3.14",
            "3:30",
            "3：30 开会",
            "example.com",
            "don't",
            "3.5亿",
            "圆周率是3.14",
            "I don't know",
        ] {
            assert_eq!(strip_effective_punctuation(t), t, "词内嵌标点被误剥：{t}");
        }
    }

    /// 🔴 回归：钉死 PUNCT-DOUBLE-334 的老 bug（`。。` / `，。` 叠加）不复发。
    ///
    /// 334 的修法是「已带标点就跳过引擎」；350 改为「先剥光再整段重打」——
    /// 机制保证进引擎的文本**已无有效标点**，故结构上不可能叠加。
    /// （引擎级最终效果需端测；本测试钉住可单测的那一半不变量。）
    #[test]
    fn test_350_no_double_punctuation_fingerprint() {
        assert_eq!(strip_effective_punctuation("你好。。"), "你好");
        assert_eq!(
            strip_effective_punctuation("今天天气不错，。"),
            "今天天气不错"
        );
        assert_eq!(strip_effective_punctuation("Hello, world.."), "Hello world");
        assert_eq!(strip_effective_punctuation("好！！"), "好");
        assert_eq!(strip_effective_punctuation("，。、"), "");
        // 指纹输入剥完必须「无有效标点」⇒ 重打不会叠加
        for t in [
            "你好。。",
            "今天天气不错，。",
            "Hello, world..",
            "好！！",
            "，。、",
        ] {
            assert!(
                !has_effective_punctuation(&strip_effective_punctuation(t)),
                "重复标点指纹未剥净：{t}"
            );
        }
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

    // ============================================================
    // TEST-SYNC-030-B · 函数 1: apply_l2_postprocess 判定矩阵（030-E 纯函数）
    // 判定矩阵（tester-1 规格）：每条同时断言 String 与 L2Action 两个维度。
    // 语意红线（主控裁定，已写进 doc）：L2Action = 走了哪条分支，不是「是否变化」。
    // ============================================================
    #[test]
    fn test_apply_l2_postprocess_disabled_strips_all() {
        // 行 1：开关关闭 → 全文剥标点（换空格、连续空格合一、trim），action=StripAll
        let (out, action) = apply_l2_postprocess("周末能去爬山", false, false);
        assert_eq!(out, "周末能去爬山");
        assert_eq!(action, L2Action::StripAll);
        // 行 1 附加：标点换空格 + 连续空格合一 + trim
        let (out, action) = apply_l2_postprocess("周末 ，能去爬山", false, false);
        assert_eq!(out, "周末 能去爬山");
        assert_eq!(action, L2Action::StripAll);
        // 行 2：开关关闭、句末标点 → 剥成「再见」
        let (out, action) = apply_l2_postprocess("再见。", false, false);
        assert_eq!(out, "再见");
        assert_eq!(action, L2Action::StripAll);
    }

    #[test]
    fn test_apply_l2_postprocess_short_strip_trailing() {
        // 行 3：开关开启、≤5 单位 + 句末标点 → 直接删末尾，不留空格
        let (out, action) = apply_l2_postprocess("再见。", true, false);
        assert_eq!(out, "再见");
        assert_eq!(action, L2Action::StripTrailing);
        // 行 4：开关开启、≤5 单位 + 感叹号
        let (out, action) = apply_l2_postprocess("好的！", true, false);
        assert_eq!(out, "好的");
        assert_eq!(action, L2Action::StripTrailing);
    }

    #[test]
    fn test_apply_l2_postprocess_long_noop() {
        // 行 5：开关开启、>5 单位 → 完全 no-op（最大回归风险点：长句逐字符不动）
        let input = "这个方案不错，我们下周再评审。";
        let (out, action) = apply_l2_postprocess(input, true, false);
        assert_eq!(out, input);
        assert_eq!(action, L2Action::NoOp);
        // 行 6：开关开启、>5 单位，即使带末尾标点与内部数字也不动
        let input = "圆周率是3.14";
        let (out, action) = apply_l2_postprocess(input, true, false);
        assert_eq!(out, input);
        assert_eq!(action, L2Action::NoOp);
    }

    #[test]
    fn test_apply_l2_postprocess_threshold_boundary() {
        // 行 7：恰 5 单位 + 句末标点 → 走 ≤5 分支剥末尾
        let (out, action) = apply_l2_postprocess("一二三四五。", true, false);
        assert_eq!(out, "一二三四五");
        assert_eq!(action, L2Action::StripTrailing);
        // 行 8：恰 6 单位 + 句末标点 → 走 >5 no-op
        let input = "一二三四五六。";
        let (out, action) = apply_l2_postprocess(input, true, false);
        assert_eq!(out, input);
        assert_eq!(action, L2Action::NoOp);
    }

    #[test]
    fn test_apply_l2_postprocess_action_is_branch_not_change() {
        // 行 9：开关关闭、文本本无标点 → 输出=入参但 action 是 StripAll（不是 NoOp）
        let (out, action) = apply_l2_postprocess("好的", false, false);
        assert_eq!(out, "好的");
        assert_eq!(action, L2Action::StripAll);
        // 行 10：开关开启、≤5 且本无末尾标点 → 输出=入参但 action 是 StripTrailing（不是 NoOp）
        let (out, action) = apply_l2_postprocess("好的", true, false);
        assert_eq!(out, "好的");
        assert_eq!(action, L2Action::StripTrailing);
    }

    // ============================================================================
    // V091-PUNCT-TAIL-214 / V091-ITN-SKIP-ONLINE-215 · 阶段三交叉护栏
    // （TEST-SYNC-214/215，coder-2 独立推导；作者 coder-1 阶段一只做了既有调用点的
    //   三参迁移，一条新护栏都没有 —— 本组全部为新增断言）
    //
    // 分工说明（为什么不重复既有面）：`count_units` 口径、`strip_trailing_punctuation`
    // 字符集、`has_effective_punctuation`、`apply_l2_postprocess` 旧行为（第三参恒 false）
    // 已被本文件既有 40+ 条用例覆盖；本组只钉**新开关引入的新边界** + **验收标准 a
    // （开关关闭时 5 条产出源逐字一致）的回归面**。
    //
    // 结构护栏写法沿用项目既有 idiom（`main.rs` 的 `nospeech_122_guard_tests` /
    // `overlay_121_guard_tests`）：`include_str!` + 截断到首个 `#[cfg(test)]` 只扫生产区；
    // 行首 `startswith` 定位；花括号定界取块。
    // ============================================================================
    mod guard_214_215 {
        use super::*;

        /// 生产区逐行 trim：**剔除所有 test-gated 项**（共享 helper，FIX-GUARD-301-B）。
        ///
        /// 🔴 本组护栏扫的是 **`main.rs`**（不是本文件）。原实现「截断到首个 `#[cfg(test)]`」
        /// 在 298 于 `main.rs:9165` 插入 `mod parallel_acc_298_tests` 后塌缩，
        /// G7/G9/G10/G11 四条锚点（L9265/9391/9478/9503）全落扫描区外 ⇒ 假红
        /// （`TEST-EXEC-302` 实测）。301 当时只改了 `main.rs` 内部的 4 处，
        /// **漏了本处跨文件扫描的第 5 处**（主控 grep 只扫了 main.rs，范围定窄）。
        fn prod_lines(src: &'static str) -> Vec<String> {
            crate::guard_prod_lines::prod_lines_excluding_cfg_test(src)
        }

        fn main_prod_lines() -> Vec<String> {
            prod_lines(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/main.rs"
            )))
        }

        fn find_line(lines: &[String], needle: &str) -> Option<usize> {
            lines.iter().position(|l| l.starts_with(needle))
        }

        fn brace_delta(line: &str) -> i32 {
            line.matches('{').count() as i32 - line.matches('}').count() as i32
        }

        /// 花括号定界：以 anchor 行为起点返回 (open_idx, close_idx)。
        fn block_bounds(lines: &[String], anchor: usize) -> Option<(usize, usize)> {
            let mut depth = 0i32;
            let mut opened = false;
            let mut open_idx = usize::MAX;
            for (i, line) in lines.iter().enumerate().skip(anchor) {
                depth += brace_delta(line);
                if depth > 0 && !opened {
                    opened = true;
                    open_idx = i;
                }
                if opened && depth == 0 {
                    return Some((open_idx, i));
                }
            }
            None
        }

        /// `if` **真分支体**：anchor 行之后、到第一个以 `}` 开头的行之前（不含两端）。
        ///
        /// 🔴 FIX-222 A：**不能用花括号定界取真分支** —— `} else {` 一行 `{`/`}` 净差为 0，
        /// 深度不归零，`block_bounds` 的窗口会一直延伸到整个 `if/else` 的 `};`，
        /// 于是 else 分支体也被算进「真分支内」→ 反向断言必然误报（首跑 guard_215_g10 即此）。
        /// 改为按「首个以 `}` 开头的行」截断，天然的 else / 嵌套 if 都能正确断开。
        fn if_true_body<'a>(lines: &'a [String], anchor: usize) -> &'a [String] {
            let tail = &lines[anchor + 1..];
            let n = tail
                .iter()
                .position(|l| l.starts_with('}'))
                .unwrap_or(tail.len());
            &tail[..n]
        }

        /// 语句窗口：anchor 行到首个恰好等于 `end` 的行（含首尾）。
        ///
        /// 用途：`let x = if <多行条件> { … } else { … };` 形态的判定点 ——
        /// 条件表达式写在 `{` **之前**，花括号定界拿不到条件行，故改用语句终止符定界。
        fn stmt_window<'a>(lines: &'a [String], anchor: usize, end: &str) -> &'a [String] {
            let tail = &lines[anchor..];
            let n = tail
                .iter()
                .position(|l| l == end)
                .map(|i| i + 1)
                .unwrap_or(tail.len());
            &tail[..n]
        }

        fn window_has(window: &[String], needle: &str) -> bool {
            window.iter().any(|l| l.contains(needle))
        }

        // ---------------------------------------------------------------------
        // 214 · 真值表与恒剥尾新行为（纯函数边界）
        // ---------------------------------------------------------------------

        /// G1 · 四格真值表 + 🔴 全剥优先（最易写错的一格）。
        /// 同一长文本四格对照：任一格语义漂移必红。
        #[test]
        fn guard_214_g1_strip_all_wins_over_strip_trailing() {
            // 13 单位 > 5；含内部逗号 + 尾句号，可区分 StripAll 与 StripTrailing
            let long = "这个方案不错，我们下周再评审。";

            // 格 1：关闭 + 关尾 → StripAll（内部逗号也换空格 = 全剥特征）
            let (out, action) = apply_l2_postprocess(long, false, false);
            assert_eq!(action, L2Action::StripAll);
            assert_eq!(out, "这个方案不错 我们下周再评审");

            // 格 2：🔴 关闭 + 开尾 → 仍是 StripAll（全剥优先，不得降级成只剥尾）
            let (out2, action2) = apply_l2_postprocess(long, false, true);
            assert_eq!(
                action2,
                L2Action::StripAll,
                "关标点时恒剥尾开关不得把全剥降级"
            );
            assert_eq!(out2, out, "关标点时第三参不改变输出");

            // 格 3：开启 + 关尾 + 长文本 → NoOp（最大回归风险点，逐字符不动）
            let (out3, action3) = apply_l2_postprocess(long, true, false);
            assert_eq!(action3, L2Action::NoOp);
            assert_eq!(out3, long);

            // 格 4：开启 + 开尾 + 长文本 → StripTrailing（只剥尾，内部逗号保留）
            let (out4, action4) = apply_l2_postprocess(long, true, true);
            assert_eq!(action4, L2Action::StripTrailing);
            assert_eq!(out4, "这个方案不错，我们下周再评审");
            assert_ne!(out4, out3, "恒剥尾开启必须与关闭在长文本上可区分");
        }

        /// G2 · 恒剥尾对英文（翻译路径）同样生效，且只剥「句末终结类」标点。
        #[test]
        fn guard_214_g2_strip_trailing_always_english() {
            // 8 单位 > 5；内部半角逗号/句点保留，仅尾 `?` 被剥
            let en = "Hello, this is a long sentence. now what?";
            let (out, action) = apply_l2_postprocess(en, true, true);
            assert_eq!(action, L2Action::StripTrailing);
            assert_eq!(out, "Hello, this is a long sentence. now what");
            // 对照：第三参 false 时英文长句是 NoOp（防「英文走另一条路」的假设）
            let (out2, action2) = apply_l2_postprocess(en, true, false);
            assert_eq!(action2, L2Action::NoOp);
            assert_eq!(out2, en);
            // 关闭开关 → 半角标点换空格（StripAll 的语言无关性）
            let (out3, action3) = apply_l2_postprocess(en, false, false);
            assert_eq!(action3, L2Action::StripAll);
            assert_eq!(out3, "Hello this is a long sentence now what");
        }

        /// G3 · action 是「走了哪条分支」不是「有没有改动」：恒剥尾开启时，
        /// 无标点长文本的输出与 NoOp 相同，但 action 必须是 StripTrailing ——
        /// 字符串相等时唯一判别力来自 action（L2Action 存在的理由）。
        #[test]
        fn guard_214_g3_action_is_branch_with_always_flag() {
            let plain = "这个方案不错我们下周再评审"; // 13 单位，无任何标点
            let (out, action) = apply_l2_postprocess(plain, true, true);
            assert_eq!(out, plain);
            assert_eq!(
                action,
                L2Action::StripTrailing,
                "判据是「开关/长度」，不是「有无标点」"
            );
            let (out2, action2) = apply_l2_postprocess(plain, true, false);
            assert_eq!(out2, out, "两格输出相同");
            assert_eq!(action2, L2Action::NoOp);
            assert_ne!(action, action2, "字符串相等时只能靠 action 判别");
        }

        /// G4 · 退化输入的边界：纯标点 / 空串 / 仅尾标点 / 尾随空白。
        #[test]
        fn guard_214_g4_degenerate_inputs() {
            // 纯尾标点：恒剥尾 → 空串；action 仍是 StripTrailing（不是 NoOp）
            assert_eq!(
                apply_l2_postprocess("。。。", true, true),
                (String::new(), L2Action::StripTrailing)
            );
            // 同输入、关标点 → StripAll（全剥优先，见 G1）
            assert_eq!(
                apply_l2_postprocess("。。。", false, true),
                (String::new(), L2Action::StripAll)
            );
            // 空串：count_units=0 ≤ 5 → 短文本分支
            assert_eq!(
                apply_l2_postprocess("", true, false),
                (String::new(), L2Action::StripTrailing)
            );
            // 仅一个尾标点 + 尾随空白（尾空白不得残留）
            assert_eq!(
                apply_l2_postprocess("好。", true, true),
                ("好".to_string(), L2Action::StripTrailing)
            );
            assert_eq!(
                apply_l2_postprocess("好。  ", true, true),
                ("好".to_string(), L2Action::StripTrailing)
            );
        }

        // ---------------------------------------------------------------------
        // 214 · 验收标准 a 的回归面（配置 / 结构护栏）
        // ---------------------------------------------------------------------

        /// G5 · 🔴 存量用户零回归的根：配置缺字段必须回落 false。
        /// 旧 `config.toml` 没有 `strip_trailing`，serde 少了 default 会让
        /// 「关标点 → 恒剥尾」等行为对存量用户静默改变。
        #[test]
        fn guard_214_g5_config_field_defaults_false_when_absent() {
            use crate::config::PunctuationConfig;
            // 结构体默认值
            let d = PunctuationConfig::default();
            assert!(d.enabled, "enabled 默认必须 true（存量行为）");
            assert!(
                !d.strip_trailing,
                "strip_trailing 默认必须 false（存量行为）"
            );
            // 旧配置（无该字段）→ false
            let old: PunctuationConfig =
                toml::from_str("enabled = true\n").expect("旧配置必须能解析");
            assert!(
                !old.strip_trailing,
                "缺字段必须回落 false，否则存量用户行为改变"
            );
            // 关闭标点的旧配置同样回落 false
            let off: PunctuationConfig =
                toml::from_str("enabled = false\n").expect("旧配置必须能解析");
            assert!(!off.strip_trailing);
            // 显式开启 → true（开关可达，不是死字段）
            let on: PunctuationConfig =
                toml::from_str("enabled = true\nstrip_trailing = true\n").expect("新配置可解析");
            assert!(on.strip_trailing);
        }

        /// G6 · 🔴 产出源 #4/#5「刻意不覆盖」：overlay 编辑态提交只读 `strip_trailing`，
        /// **不得**读 `enabled`，也不得改用全剥函数。
        /// 这是验收标准 a 的直接护栏：若有人把它「补全」成
        /// `enabled || strip_trailing`（或换成 strip_punctuation），本组必红。
        #[test]
        fn guard_214_g6_overlay_submit_reads_only_strip_trailing() {
            let lines = main_prod_lines();
            let anchor = find_line(&lines, "let text_to_inject = if clone_runtime_config")
                .expect("main.rs 必须保留编辑态提交的 text_to_inject 判定点");
            let w = stmt_window(&lines, anchor, "};");
            assert!(
                window_has(&w, ".strip_trailing"),
                "判定必须读 punctuation.strip_trailing"
            );
            assert!(
                window_has(&w, "punctuation::strip_trailing_punctuation(&text)"),
                "打开时必须调用 strip_trailing_punctuation（只剥尾）"
            );
            assert!(
                window_has(&w, "text.clone()"),
                "关闭时必须原样使用 text（逐字一致）"
            );
            // 反向：不得读 enabled（关标点的全剥现状本就**不**覆盖 #4/#5，刻意维持）
            assert!(
                !window_has(&w, "enabled"),
                "🔴 #4/#5 刻意不覆盖 enabled 全剥 —— 补上去会破坏验收标准 a"
            );
            // 反向：不得改用全剥函数（注意 strip_trailing_punctuation 不含子串 strip_punctuation）
            assert!(
                !window_has(&w, "strip_punctuation"),
                "🔴 #4/#5 不得使用全剥（strip_punctuation）"
            );
        }

        /// G7 · 产出源 #1 主 pipeline 调用点必须把开关传进去（防漏传/写死 false）。
        #[test]
        fn guard_214_g7_main_pipeline_passes_config_flag() {
            let lines = main_prod_lines();
            let anchor = find_line(
                &lines,
                "let (l2_text, l2_action) = punctuation::apply_l2_postprocess(",
            )
            .expect("主 pipeline 的 L2 调用点必须存在");
            let w = stmt_window(&lines, anchor, ");");
            assert!(
                window_has(&w, "config.punctuation.enabled"),
                "第 2 实参必须是 config.punctuation.enabled"
            );
            assert!(
                window_has(&w, "config.punctuation.strip_trailing"),
                "第 3 实参必须是 config.punctuation.strip_trailing（写死 false = 新开关对主 pipeline 失效）"
            );
        }

        /// G8 · src-tauri 侧镜像字段必须存在且带 `serde(default)`。
        /// 缺失会让 UI 存档路径静默丢字段（Tauri 反序列化该结构再序列化写回）；
        /// 不带 default 则旧存档（无该字段）反序列化失败。
        /// 跨 crate 无法编译期引用，只能对源文件做结构断言。
        #[test]
        fn guard_214_g8_tauri_mirror_field_kept() {
            let src = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src-tauri/src/config.rs"
            ));
            let needle = "pub strip_trailing: bool";
            let idx = src
                .find(needle)
                .expect("src-tauri 侧必须有 strip_trailing 镜像字段（否则 UI 存 config 会丢字段）");
            let before = &src[..idx];
            let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let ctx_begin = line_start.saturating_sub(200);
            assert!(
                src[ctx_begin..line_start].contains("serde(default)"),
                "strip_trailing 必须带 #[serde(default)]（旧存档缺字段不得反序列化失败）"
            );
        }

        // ---------------------------------------------------------------------
        // 215 · 在线 ASR 跳 ITN 的判据与通道边界
        // ---------------------------------------------------------------------

        /// G9 · 🔴 判据必须是数据路径事实（`initial_text`），不得回退成 config。
        /// `config.audio.asr_model` 与 transcriber 热重载存在瞬态不同步，
        /// 按 config 判会把**本地** ASR 输出也误跳 ITN（main.rs 注释已声明该理由）。
        /// 时序（必须在 `initial_text` 被 move 之前取值）由借用检查器保证，无需额外断言。
        #[test]
        fn guard_215_g9_judge_is_data_path_not_config() {
            let lines = main_prod_lines();
            let anchor = find_line(
                &lines,
                "let from_online_streaming = initial_text.is_some();",
            )
            .expect("215 判据行必须存在（数据路径事实 initial_text）");
            assert!(
                !lines[anchor].contains("config"),
                "判据不得来自 config（与引擎热重载有瞬态不同步）"
            );
            assert!(
                !lines[anchor].contains("asr_model"),
                "判据不得来自 asr_model"
            );
            // 全生产区非注释出现次数必须恰为 2：1 次定义 + 1 次使用（只跳主通道）。
            // 出现第 3 处 = 有人给它加了第二个门控（例如把补丁通道也跳掉）= 回归。
            let uses = lines
                .iter()
                .filter(|l| !l.starts_with("//") && l.contains("from_online_streaming"))
                .count();
            assert_eq!(
                uses, 2,
                "from_online_streaming 只允许「定义 1 处 + 主通道使用 1 处」；多出即新增门控"
            );
        }

        /// G10 · 主通道两条分支都必须存在：online → 逐字 `raw_text`；offline → 主通道 ITN。
        #[test]
        fn guard_215_g10_main_channel_both_branches() {
            let lines = main_prod_lines();
            let anchor = find_line(&lines, "let pre_llm_text = if from_online_streaming {")
                .expect("主通道 skip 判定必须存在");
            // 🔴 FIX-222 A：只扫**真分支体**（`} else {` 之前），不把 else 分支算进来。
            let online = if_true_body(&lines, anchor);
            // 非恒真自证：若有人把 `itn::normalize_numbers(&raw_text)` **挪进 online 分支**
            // （即挪到这行 `}` 之前），`online` 就会包含它 → 下面第二条断言立刻变红。
            // 反之，原实现把 else 分支也纳入窗口，所以它对**正确的**代码也报红（首跑假阳性根因）。
            assert!(
                online.iter().any(|l| l.starts_with("raw_text.clone()")),
                "online 分支必须原样使用 raw_text（真分支体为空或分支被删都会红）"
            );
            assert!(
                !online
                    .iter()
                    .any(|l| l.starts_with("itn::normalize_numbers")),
                "online 分支内不得再跑主通道 ITN（否则 gate 形同虚设）"
            );
            assert!(
                find_line(&lines, "itn::normalize_numbers(&raw_text)").is_some(),
                "offline 分支必须保留主通道 normalize_numbers"
            );
        }

        /// G11 · 🔴 补丁通道刻意保留（DEC-036 双通道的另一半），且不被 skip 门控 ——
        /// 「只跳主通道」是决定不是漏。若有人把补丁通道也塞进门控块，本组必红。
        #[test]
        fn guard_215_g11_patch_channel_survives_and_not_gated() {
            let lines = main_prod_lines();
            let patch = find_line(
                &lines,
                "let final_text = itn::normalize_unit_symbols_only(&final_text);",
            )
            .expect("补丁通道调用必须保留（只跳主通道，不是两条都跳）");
            let gate = find_line(&lines, "let pre_llm_text = if from_online_streaming {")
                .expect("主通道门控必须存在");
            let (_, gate_end) = block_bounds(&lines, gate).expect("门控块必须闭合");
            assert!(
                patch > gate_end,
                "补丁通道必须在门控块之外（块内 = 被 online 门控吞掉）"
            );
            assert!(
                !lines[patch].contains("from_online_streaming"),
                "补丁通道调用不得挂在 online 门控上"
            );
        }

        /// G12 · 补丁通道的「价值」与「边界」（语义旁证，证明只跳主通道不是漏跳）：
        /// 它能做主通道不做的事（单位符号），也不做越权的事（不转中文数字）。
        #[test]
        fn guard_215_g12_patch_channel_value_and_limits() {
            // 主通道：中文数字 → 阿拉伯数字（跳过它确有实质效果，不是恒等操作）
            assert_eq!(crate::itn::normalize_numbers("三百二十五"), "325");
            // 补丁通道：阿拉伯数字 + 中文单位 → 单位符号（LLM 纠正 ASR 同音错字后仍能定型）
            assert_eq!(crate::itn::normalize_unit_symbols_only("40摄氏度"), "40℃");
            // 补丁通道不越权：中文数字原样（它不是主通道的复制品）
            assert_eq!(
                crate::itn::normalize_unit_symbols_only("三百二十五"),
                "三百二十五"
            );
            // 幂等：已定型输出不二次改动
            assert_eq!(crate::itn::normalize_unit_symbols_only("40℃"), "40℃");
        }
    }
}

// ============================================================================
// TEST-SYNC-352（阶段三，非作者 coder-2）：给 PUNCT-FINAL-REDO-350 补独立护栏
//
// 350 把 `has_effective_punctuation` 由内联循环改为调共享谓词 `is_effective_punctuation`，
// 等价性当时**只有人工论证**。本模块用「旧语义显式重写版」做机器对照，并补退化 / UTF-8 边界。
// 🔴 只读生产符号，不改任何生产代码。
// ============================================================================
#[cfg(test)]
mod sync352_punct_final_350_tests {
    use super::*;

    /// 旧语义显式重写（350 重构前 `has_effective_punctuation` 的逐字逻辑，取自 `c9b59b3^`）。
    /// **刻意不复用生产共享谓词**：一旦生产谓词被改坏，这份独立重写真值表才是判别基准。
    fn has_effective_punctuation_legacy(text: &str) -> bool {
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

    /// 🔴 sync352-1：`has_effective_punctuation` 重构等价性（新 vs 旧逐字逻辑逐串相同）。
    ///
    /// **防的退化**：内联循环改调共享谓词时，若哨兵（位置 0/末尾）、索引、夹持方向被改错，
    /// 行为会**静默漂移**（`[ENUM-EQ-CHECK-MISSES-NEW-VARIANT-001]` 同族：编译器不报错）。
    /// 用小字母表穷举长度 ≤4，覆盖 CJK / ASCII 字母数字 / 全半角标点 / 空白 / 撇号 / 多字节。
    #[test]
    fn sync352_has_refactor_equals_legacy_semantics() {
        const ALPHABET: &[char] = &[
            '.', ':', '！', 'a', '1', '中', '，', '。', '５', '．', '👍', 'é', ' ', '\'',
        ];
        let mut corpus: Vec<String> = vec![String::new()];
        let mut frontier: Vec<String> = vec![String::new()];
        for _ in 0..4 {
            let mut next = Vec::with_capacity(frontier.len() * ALPHABET.len());
            for base in &frontier {
                for &c in ALPHABET {
                    let mut s = base.clone();
                    s.push(c);
                    next.push(s.clone());
                    corpus.push(s);
                }
            }
            frontier = next;
        }
        for t in &corpus {
            let new = has_effective_punctuation(t);
            assert_eq!(
                new,
                has_effective_punctuation_legacy(t),
                "重构等价性被破坏（新旧语义不一致）：{t:?}"
            );
            // 补充不变量：`strip` 恒等 ⟺ `has` 为 false（严格对偶的可判形式）。
            assert_eq!(
                strip_effective_punctuation(t) == *t,
                !new,
                "strip 恒等性与 has 不一致：{t:?}"
            );
        }
    }

    /// 🔴 sync352-2：退化输入下「严格对偶 + 幂等 + 无有效标点则恒等」三条性质仍成立。
    ///
    /// **防的退化**：空串 / 纯标点串 / 首尾即标点 / 单字符 / 超长串等边界上 strip 与 has 走岔
    /// （典型：空串哨兵退化、纯标点末字符被漏剥）。
    #[test]
    fn sync352_degenerate_inputs_dual_identity_idempotent() {
        let long: String = "中".repeat(300) + "。" + &"a".repeat(300);
        let cases: Vec<String> = vec![
            String::new(),
            "，。、".to_string(),
            "。".to_string(),
            "a".to_string(),
            "中".to_string(),
            "。开头".to_string(),
            "结尾。".to_string(),
            "。。".to_string(),
            "，，，".to_string(),
            long.clone(),
        ];
        for t in &cases {
            let s = strip_effective_punctuation(t);
            assert!(
                !has_effective_punctuation(&s),
                "严格对偶被破坏：strip({t:?}) = {s:?} 仍含有效标点"
            );
            assert_eq!(strip_effective_punctuation(&s), s, "非幂等：{t:?}");
            if !has_effective_punctuation(t) {
                assert_eq!(&s, t, "无有效标点时 strip 必须恒等：{t:?}");
            }
        }
    }

    /// 🔴 sync352-3：UTF-8 安全 —— 多字节字符紧邻标点 / emoji 混排（344 P0 崩溃同族防线）。
    ///
    /// **防的退化**：`strip_` 若哪天被改成按**字节**切片，会在多字节字符中间 panic 或产出坏串；
    /// 也要钉住「多字节字符本身绝不被当标点剥掉」。
    #[test]
    fn sync352_utf8_multibyte_adjacent_to_punctuation() {
        assert_eq!(strip_effective_punctuation("中。"), "中");
        assert_eq!(strip_effective_punctuation("。中"), "中");
        assert_eq!(strip_effective_punctuation("中，文"), "中文");
        assert_eq!(strip_effective_punctuation("👍。"), "👍");
        assert_eq!(strip_effective_punctuation("。👍。"), "👍");
        // 无有效标点 ⇒ 多字节原样保留（emoji / 重音字母必须完整）
        assert_eq!(strip_effective_punctuation("a👍b"), "a👍b");
        assert_eq!(strip_effective_punctuation("é中é"), "é中é");
        // 🔴 钉住既有口径（半/全角不一致，非本单引入，只报不改）：全角数字两侧**非** ASCII
        //    ⇒ 不豁免 ⇒ 判为有效标点，与半角 `3.14` 行为不同。
        assert!(has_effective_punctuation("５.５"));
        assert!(!has_effective_punctuation("3.14"));
        // 多字节紧邻不 panic，且不产生替换字符（编码损坏的指纹 U+FFFD）
        for t in ["。é。", "👍👍。", "中。é。", "。👍中。"] {
            let s = strip_effective_punctuation(t);
            assert!(!has_effective_punctuation(&s), "对偶破坏：{t:?} -> {s:?}");
            assert!(
                !s.contains('\u{FFFD}'),
                "输出出现替换字符（编码损坏）：{t:?} -> {s:?}"
            );
        }
    }
}
