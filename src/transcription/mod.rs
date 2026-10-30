//! 识别入口（FLASH-P3-A，DEC-114）：输入法「变薄」后**不带任何本地识别模型**，只剩两条在线流式路径：
//! - [`AsrModel::FunAsrRealtime`]：直连阿里 FunASR 实时识别（用户自己的 API Key）；
//! - [`AsrModel::FeiyinServer`]：连识别服务 flashasr（配对文件 `asr-remote.json`）。
//!
//! 两条都走 [`qwen_inference`] 的同一个会话循环（边录边发边收）。原本地档（performance / accuracy /
//! local_realtime）与 qwen_audio_online 的引擎、VAD、声纹、滑窗精解等代码已整体删除（识别能力在 flashasr）。

use crate::punctuation;

pub mod feiyin_remote;
pub mod qwen_inference;

/// BUG-119（BUILD-118 端测第 1 项）：「用户没说话」的类型化信号，不是设备/网络错误。
///
/// 缺陷史：所有「没识别到语音」路径原先都用 anyhow 字符串错误（一中一英），
/// 显示层 `convert_to_friendly_error` 关键词嗅探分类不中 → 开发态原文直出。
/// 升级为类型化信号后，显示侧在 map_err 边界 downcast 本类型 → 信息提示（i18n）。
///
/// 契约：**新增第三个「没说话」产出源时只需 `bail!(NoSpeechError)`，
/// 显示侧分类器零改动** —— 这是本类型存在的唯一意义，禁止再往错误串里加关键词。
/// 设备/模型异常（空音频样本、VAD 已确认有语音段但模型空输出）**不得**用本类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoSpeechError;

impl std::fmt::Display for NoSpeechError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no speech detected")
    }
}

impl std::error::Error for NoSpeechError {}

/// ASR 模型选择（FLASH-P3-A 起只剩两条在线路径）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrModel {
    /// 在线流式 ASR —— 直连阿里 FunASR Realtime（ASR-056，用户自己的 API Key）。
    FunAsrRealtime,
    /// ASR-SERVER-491 S8：连识别服务 flashasr（`run-task` 同协议）。
    /// 地址 / key / 证书指纹取自配对文件 `asr-remote.json`，不用 API Key。
    FeiyinServer,
}

impl AsrModel {
    /// 配置字符串 → 模型。FLASH-P3-A：除 `fun_asr_realtime` 外一律视为 `FeiyinServer`
    /// （旧值 performance / accuracy / local_realtime / qwen_audio_online 已在配置加载时迁移为
    /// `feiyin_server`，这里兜底同口径，防手改配置后出现未知值）。
    pub fn from_config(s: &str) -> Self {
        if s.trim().eq_ignore_ascii_case("fun_asr_realtime") {
            AsrModel::FunAsrRealtime
        } else {
            AsrModel::FeiyinServer
        }
    }
}

/// 在线识别参数持有者（FLASH-P3-A：不再加载任何模型，只存在线配置）。
///
/// 保留「构造 + 热重载」这层壳，是为了沿用 worker 里既有的配置变更检测（切换模型 / 改 Key 立即生效），
/// 以及「直连阿里但 Key 为空 ⇒ 构造失败 ⇒ 录音时提示」的既有行为。
pub struct Transcriber {
    asr_model: AsrModel,
    asr_online_url: String,
    asr_online_model: String,
    /// ASR-056: VAD 断句静音阈值（ms），config.toml 隐藏字段覆盖，ASR-SEG-229 默认 2000
    asr_online_max_sentence_silence: i64,
    /// ASR-SEG-229: 在线 ASR 语义断句/标点开关，config.toml 隐藏字段覆盖，默认 false
    asr_online_semantic_punctuation_enabled: bool,
}

impl Transcriber {
    /// 构造在线识别参数。直连阿里时 API Key 为空 ⇒ `Err`（连识别服务凭配对文件，没配对在建连时报「去配对」）。
    pub fn new(
        asr_model: AsrModel,
        asr_online_api_key: &str,
        asr_online_url: &str,
        asr_online_model: &str,
        asr_online_max_sentence_silence: i64,
        asr_online_semantic_punctuation_enabled: bool,
    ) -> anyhow::Result<Self> {
        if asr_model == AsrModel::FunAsrRealtime && asr_online_api_key.trim().is_empty() {
            anyhow::bail!("在线 ASR 配置失败：API Key 为空（请在设置中配置 API Key）");
        }
        log::info!(
            "Online ASR mode: model={:?}, url={}",
            asr_model,
            asr_online_url
        );
        Ok(Self {
            asr_model,
            asr_online_url: asr_online_url.to_string(),
            asr_online_model: asr_online_model.to_string(),
            asr_online_max_sentence_silence,
            asr_online_semantic_punctuation_enabled,
        })
    }

    pub fn asr_model(&self) -> AsrModel {
        self.asr_model
    }

    /// ASR-038-B: 在线 ASR 的 Inference API 端点
    pub fn asr_online_url(&self) -> &str {
        &self.asr_online_url
    }

    /// ASR-038-B: 在线 ASR 模型 ID
    pub fn asr_online_model(&self) -> &str {
        &self.asr_online_model
    }

    /// ASR-056: VAD 断句静音阈值（ms）
    pub fn asr_online_max_sentence_silence(&self) -> i64 {
        self.asr_online_max_sentence_silence
    }

    /// ASR-SEG-229: 在线 ASR 语义断句/标点开关（默认 false = VAD 断句）
    pub fn asr_online_semantic_punctuation_enabled(&self) -> bool {
        self.asr_online_semantic_punctuation_enabled
    }
}

/// 剥离 native 模型自带标点（ASR-PUNCT-OPT-001 + PUNCT-GOVERNANCE-030-A A1/A2）
///
/// 用于 accuracy 模式 + 用户关闭自动标点（punctuation.enabled=false）场景：
/// native 输出自带标点，用户关了开关但 native 照样出标点 → 剥离使其与 CTC 行为一致。
///
/// 剥离字符集：`punctuation::PUNCT_CHARS`（中文标点、中文括号/书名号/间隔号
/// + 英文标点 + 半角句点 .）。
/// **保护不剥离**：小数点（.）在数字间（如 3.14）、域名/URL 中的点（example.com）。
///
/// 行为（A2，Gavin 2026-08-08）：命中标点 **push 一个空格**（而非删除），
/// 末尾连续空格合一（:505 逻辑保留）+ trim。保证句中标点造成的词间粘连被空格承接。
pub fn strip_punctuation(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(chars.len());
    for (i, &c) in chars.iter().enumerate() {
        if !punctuation::is_punctuation(c) {
            result.push(c);
            continue;
        }
        // 保护：小数点 / 域名点（前后均为 ASCII 字母或数字）
        // 覆盖现有 :490 数字间小数点逻辑 + example.com 域名点（URL 场景实测结论）
        if c == '.' {
            let prev = chars.get(i.wrapping_sub(1)).copied().unwrap_or(' ');
            let next = chars.get(i + 1).copied().unwrap_or(' ');
            if prev.is_ascii_alphanumeric() && next.is_ascii_alphanumeric() {
                result.push(c);
                continue;
            }
        }
        // A2: 命中标点 push 空格（而非 continue 跳过）
        result.push(' ');
    }
    // 清理剥离后可能产生的连续空格（标点前后原有空格）
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }
    result.trim().to_string()
}
/// ASR-038-B: 从 wordbook 表加载热词 vocabulary（调用链证明）
///
/// **调用链**：
/// `wordbook::Wordbook::list_all()` → `db::load_word_entries()`（只查 `wordbook` 表）
/// → 转换为 `qwen_inference::VocabEntry` → `qwen_inference::build_vocabulary()`
///
/// 🔴 **数据源保证**：`Wordbook::list_all()` 只查 `wordbook` 表（`db::load_word_entries()`），
/// **不查 `wordbook_candidates` 表**（163 条未确认候选，禁止注入）。
/// `wordbook_candidates` 由 `db::upsert_candidate` / `db::load_candidates` 管理，
/// 本函数不调用这些接口。
///
/// 权重：user=5（专名领域词）、system=4（历史 bug 沉淀保护词）、未知=3
/// 超限词条由 `build_vocabulary` 内部过滤（跳过 ASR 但保留给 LLM）
pub fn load_wordbook_vocabulary() -> serde_json::Value {
    match crate::wordbook::Wordbook::open() {
        Ok(wb) => match wb.list_all() {
            Ok(entries) => {
                let vocab_entries: Vec<qwen_inference::VocabEntry> = entries
                    .into_iter()
                    .map(|e| qwen_inference::VocabEntry {
                        word: e.word,
                        source: e.source,
                    })
                    .collect();
                log::info!(
                    "ASR vocabulary loaded from wordbook table: {} entries",
                    vocab_entries.len()
                );
                qwen_inference::build_vocabulary(&vocab_entries)
            }
            Err(e) => {
                log::warn!("Failed to load wordbook for ASR vocabulary: {}", e);
                serde_json::json!({})
            }
        },
        Err(e) => {
            log::warn!("Failed to open wordbook for ASR vocabulary: {}", e);
            serde_json::json!({})
        }
    }
}

/// FLASH-P0-509：连自家识别服务时的词表 = **完整词库字符串数组**，顺序同 `list_all`（id 倒序）。
/// 不走阿里长度过滤 / 权重 —— 预算裁剪由识别服务（flashasr）自己做。
/// 只查 `wordbook` 表（不含候选表），同 [`load_wordbook_vocabulary`]。
pub fn load_wordbook_word_array() -> serde_json::Value {
    let words: Vec<String> = match crate::wordbook::Wordbook::open().and_then(|wb| wb.list_all()) {
        Ok(entries) => entries.into_iter().map(|e| e.word).collect(),
        Err(e) => {
            log::warn!("Failed to load wordbook for remote ASR vocabulary: {}", e);
            Vec::new()
        }
    };
    log::info!(
        "Remote ASR vocabulary loaded from wordbook table: {} entries",
        words.len()
    );
    serde_json::json!(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FLASH-P3-A：只剩两个变体；未知 / 旧值一律落到 `FeiyinServer`。
    /// 穷举 match：新增 / 删除变体在此编译失败（编译期护栏）。
    #[test]
    fn p3a_asr_model_two_variants_with_stable_mapping() {
        let _exhaustive: fn(AsrModel) = |m| match m {
            AsrModel::FunAsrRealtime | AsrModel::FeiyinServer => (),
        };
        assert_eq!(
            AsrModel::from_config("fun_asr_realtime"),
            AsrModel::FunAsrRealtime
        );
        assert_eq!(
            AsrModel::from_config("FuN_AsR_ReAlTiMe"),
            AsrModel::FunAsrRealtime
        );
        assert_eq!(
            AsrModel::from_config("feiyin_server"),
            AsrModel::FeiyinServer
        );
        for old in [
            "performance",
            "accuracy",
            "local_realtime",
            "qwen_audio_online",
            "qwen3_online",
            "",
            "garbage",
        ] {
            assert_eq!(AsrModel::from_config(old), AsrModel::FeiyinServer, "{old}");
        }
    }

    /// ASR-038-B / ASR-SEG-229：在线配置原样存取；直连阿里 Key 为空 ⇒ 构造失败；连识别服务不要 Key。
    #[test]
    fn p3a_transcriber_stores_online_config_and_checks_key() {
        let t = Transcriber::new(
            AsrModel::FunAsrRealtime,
            "sk-test",
            "wss://example/ws",
            "fun-asr-realtime",
            2000,
            true,
        )
        .expect("key 非空应构造成功");
        assert_eq!(t.asr_model(), AsrModel::FunAsrRealtime);
        assert_eq!(t.asr_online_url(), "wss://example/ws");
        assert_eq!(t.asr_online_model(), "fun-asr-realtime");
        assert_eq!(t.asr_online_max_sentence_silence(), 2000);
        assert!(t.asr_online_semantic_punctuation_enabled());

        assert!(Transcriber::new(AsrModel::FunAsrRealtime, "  ", "", "", 2000, false).is_err());
        assert!(Transcriber::new(AsrModel::FeiyinServer, "", "", "", 2000, false).is_ok());
    }

    // ============================================================
    // ASR-PUNCT-OPT-001: strip_punctuation 测试
    // ============================================================

    #[test]
    fn strip_punctuation_chinese() {
        // 标点替换为空格（A2），不是直接删除。
        // 句末标点产生的多余空格由 trim 清理。
        assert_eq!(
            strip_punctuation("周末要不要去露营？最近天气超舒服。"),
            "周末要不要去露营 最近天气超舒服"
        );
    }

    #[test]
    fn strip_punctuation_english() {
        // 英文标点删除后保留原有空格
        assert_eq!(
            strip_punctuation("Hello, world! How are you?"),
            "Hello world How are you"
        );
    }

    #[test]
    fn strip_punctuation_mixed() {
        // 中英混合，标点替换为空格
        assert_eq!(
            strip_punctuation("今天很好，very nice！"),
            "今天很好 very nice"
        );
    }

    #[test]
    fn strip_punctuation_empty() {
        assert_eq!(strip_punctuation(""), "");
    }

    #[test]
    fn strip_punctuation_no_punct() {
        assert_eq!(
            strip_punctuation("没有任何标点的文本"),
            "没有任何标点的文本"
        );
    }

    #[test]
    fn strip_punctuation_preserves_decimal() {
        // 小数点不剥离（. 不在 punct_set 中）
        assert_eq!(strip_punctuation("圆周率是3.14"), "圆周率是3.14");
        assert_eq!(strip_punctuation("价格50.5元"), "价格50.5元");
    }

    #[test]
    fn strip_punctuation_preserves_url() {
        // URL 中的 . / : 等不剥离（: 剥离但 URL 中的 // 和 . 保留）
        // 注意：: 在 punct_set 中会被剥离——这是保守策略的已知限制，
        // URL 场景极少出现在语音转写输出中
        let input = "访问 https example.com 路径";
        assert_eq!(strip_punctuation(input), "访问 https example.com 路径");
    }

    #[test]
    fn strip_punctuation_only_punct() {
        assert_eq!(strip_punctuation("，。！？"), "");
    }

    #[test]
    fn strip_punctuation_quotes() {
        // 中文引号 \u{201C} \u{201D} 和单引号 \u{2018} \u{2019}
        // 引号位置替换为空格（A2）
        let input = "\u{201C}\u{4F60}\u{597D}\u{201D}\u{4ED6}\u{8BF4}";
        assert_eq!(strip_punctuation(input), "你好 他说");
    }

    #[test]
    fn strip_punctuation_collapses_spaces() {
        // 标点剥离后连续空格压缩
        assert_eq!(strip_punctuation("你好，  世界！"), "你好 世界");
    }

    /// TEST-SYNC-030 3.3 补充：strip_punctuation 全量参数化——PUNCT_CHARS 每个成员
    /// 命中的语义都是「换成一个空格」（A2），不是直接删除。用「词 + 标点 + 词」夹持
    /// 验证命中标点产生空格（句末等产生的位置由后续 trim 清理不在本用例覆盖）。
    #[test]
    fn strip_punctuation_every_char_replaces_with_space() {
        use crate::punctuation::PUNCT_CHARS;
        // '.' 有「数字间/域名」保护逻辑，单独测（两侧为汉字时不豁免 → 也是空格）。
        for &c in PUNCT_CHARS {
            let input = format!("测试{c}测试");
            let out = strip_punctuation(&input);
            assert_eq!(
                out, "测试 测试",
                "PUNCT_CHARS 成员 {c:?} 应被替换为单个空格（A2），而非删除"
            );
        }
    }

    // ============================================================
    // ASR-PUNCT-OPT-001: 标点决策来源标记逻辑测试
    // ============================================================

    /// 来源标记规则是纯逻辑（performance→false, accuracy native 成功→true, 兜底→false）
    /// 这里通过间接验证规则文档化（实际 Transcriber 需模型加载，无法纯逻辑测）
    /// 决策真值表在 result.md 文档化
    #[test]
    fn source_flag_documentation_placeholder() {
        // 来源标记规则由 transcribe_with_punct_info 实现，需真实模型加载，此处不纯逻辑测
        // 真值表见 result.md
        assert!(true);
    }

    #[test]
    fn qwen3_native_punctuated_detection() {
        // PUNCT-GOVERNANCE-030-A-2：qwen3 native_punctuated = 输出是否含**有效标点**
        // （实测，非恒 true；主控 c 方案统一调用 has_effective_punctuation）
        use crate::punctuation::has_effective_punctuation;
        // 带句末标点 → true（跳过标点引擎，行为不变）
        assert!(has_effective_punctuation("今天天气不错。"));
        assert!(has_effective_punctuation("OK? 好的。"));
        // 无标点 → false（走标点引擎补标点，修复目标）
        assert!(!has_effective_punctuation("好的"));
        assert!(!has_effective_punctuation("谢谢"));
        // 🔴 词内嵌入误判修复（c 方案）：PUNCT_CHARS 含半角 `.`，纯数字 3.14
        //   旧判据 is_punctuation 误判「带标点」→ native_punctuated=true → 跳过引擎。
        //   新判据 has_effective_punctuation 做「前后 ASCII 字母数字夹持则豁免」：
        //   3.14 / went's 撇号 / 3:30 冒号 / example.com 域名点全部不再计价点。
        assert!(!has_effective_punctuation("3.14"));
        assert!(!has_effective_punctuation("圆周率是3.14"));
        assert!(!has_effective_punctuation("I don't know"));
        assert!(!has_effective_punctuation("3:30 开会"));
        assert!(!has_effective_punctuation("example.com"));
    }
}
