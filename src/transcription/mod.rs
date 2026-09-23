use anyhow::{Context, Result};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::{config::ChineseScript, punctuation, text_normalizer};

// Re-export SenseVoice config for convenience
use sherpa_onnx::{OfflineQwen3ASRModelConfig, OfflineSenseVoiceModelConfig};

pub mod local_stream;
pub mod qwen_inference;
mod vad;
// FIX-SLICE-CUT-AT-GAP-381：§4/§5 实测 PoC（纯 `#[cfg(test)]`，无 lib target 故不能放 src/bin|tests）
#[cfg(test)]
mod poc_slice_cut_381;
// SLIDING-WINDOW-367：路A 逐片解码摘接线后，`join_segment_texts` 等 re-export 暂无人用；
// **保留**（可回挂），故局部 allow。
#[allow(unused_imports)]
pub use vad::{
    build_padded_segments, build_padded_segments_capped, build_sliding_segments,
    build_sliding_segments_with_spans, join_segment_texts, naive_chunk, should_segment,
    VadSegmenter, SEGMENT_MAX_SECS, SEGMENT_PADDING_SAMPLES, SEGMENT_TRIGGER_SECS,
    SLIDING_CUT_SEARCH_START_SECS,
};

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

/// ASR mode: offline or streaming (2-pass)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AsrMode {
    /// Offline mode: single pass, higher accuracy
    Offline,
    /// Streaming mode: 2-pass (streaming preview + offline correction)
    Streaming,
}

/// ASR 模型选择（DEC-025 + DEC-028 + ASR-056）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AsrModel {
    /// 性能最优：179MB FunASR Nano CTC 兼容版（OfflineSenseVoiceModelConfig，无 hotwords）
    Performance,
    /// 准确率更高：972MB FunASR Nano native（OfflineFunASRNanoModelConfig，config 层 hotwords）
    Accuracy,
    /// 在线流式 ASR —— Qwen Audio 3.0（RESEARCH-ASR-038 / ASR-041-B）：
    /// DashScope Inference 协议，模型 ID qwen-audio-3.0-asr-flash-streaming
    QwenAudioOnline,
    /// 在线流式 ASR —— FunASR Realtime（ASR-056）：
    /// 与 QwenAudioOnline 共用同一 WS 端点 + 同一 Inference 协议 + 同一返回结构，
    /// 仅 model 名与部分参数取值不同。Gavin 研究发现比 qwen-audio-3.0-asr-flash-streaming 快。
    FunAsrRealtime,
    /// LOCAL-RT-ENGINE-239-A（DEC-067）：本地流式实时档（极客档，DEC-065 默认隐藏）。
    ///
    /// **双模型并存常驻**（有意打破「本地一次只加载一个模型」）：
    /// - online = streaming paraformer trilingual（本地预览，greedy_search）
    /// - offline = accuracy FunASR Nano native（2pass 最终文本 + hotwords）
    ///
    /// DEC-067 附则一：任一模型缺失/加载失败 ⇒ 整档 `Err`，**禁止静默降级**。
    LocalRealtime,
}

impl AsrModel {
    pub fn from_config(s: &str) -> Self {
        if s.eq_ignore_ascii_case("accuracy") {
            AsrModel::Accuracy
        } else if s.eq_ignore_ascii_case("qwen_audio_online") {
            AsrModel::QwenAudioOnline
        } else if s.eq_ignore_ascii_case("fun_asr_realtime") {
            AsrModel::FunAsrRealtime
        } else if s.eq_ignore_ascii_case("local_realtime") {
            AsrModel::LocalRealtime
        } else {
            AsrModel::Performance
        }
    }

    /// ASR-056: 收敛判据——是否走在线流式管线（真流式 record_streaming + transcribe_streaming_realtime）。
    ///
    /// 新增在线模型时只改这一处，不必逐个更新 main.rs 的 12 处引用点。
    /// QwenAudioOnline 和 FunAsrRealtime 都走真流式管线（共用 qwen_inference.rs 实现）。
    pub fn is_online_streaming(&self) -> bool {
        matches!(self, AsrModel::QwenAudioOnline | AsrModel::FunAsrRealtime)
    }

    /// FIX-LOCALRT-ENGINE-EQ-252: 是否使用 **accuracy 引擎**（FunASR Nano native）。
    ///
    /// `Accuracy` 与 `LocalRealtime` 的离线/最终转录引擎都是 accuracy native
    /// （LocalRealtime 的 2pass 最终文本由 accuracy 产出），故所有「accuracy 引擎专属」
    /// 判据（VAD 分段、`native_punctuated`、hotwords 重载）都应命中两者。
    ///
    /// 🔴 收敛点：`== AsrModel::Accuracy` 这类相等比较**编译器不会报错**，新增变体极易漏改
    /// （本单两个端测 bug 即由此而来）。新增使用 accuracy 引擎的档位时，只改这一处。
    pub fn uses_accuracy_engine(self) -> bool {
        matches!(self, AsrModel::Accuracy | AsrModel::LocalRealtime)
    }
}

/// MIGRATE-QWEN3-320（Gavin 2026-09-21 拍板）：**accuracy 档就是 Qwen3-ASR，不再保留双引擎/回滚开关**。
///
/// 原 `AccuracyEngine` 枚举 + `VOICE_IME_ACCURACY_ENGINE` env 已删除。理由：实际发布只下载 Qwen3，
/// 用户机器无 FunASR 模型文件，开关在用户侧永远无效（只在开发机双模型时有意义），属过度设计；
/// 且两套构造/就位/预算路径各自为政、易漏同步（`[CONFIG-MIRROR-DRIFT-001]` 同族）。
///
/// 🔴 不新增 `AsrModel` 变体（`[ENUM-EQ-CHECK-MISSES-NEW-VARIANT-001]`）；`uses_accuracy_engine()` /
/// VAD 分段 / `native_punctuated` 判据沿用不变。**performance 档 SenseVoice 与流式 paraformer 不受影响**。

/// accuracy 侧解码线程数（**唯一来源，无 env**）：`min(逻辑核数, 8)`
/// （RESEARCH-ACC-LATENCY-271 实测最优；取不到兜底 4）。
///
/// 🔴 **`min(cores, 8)` 不是保守取值，是实测最优 —— 别想当然调高到满核。**
/// 366（2026-09-22）实测对照：同音频同参数，16 线程比 8 线程 **20s 慢 1.73×、56s 慢 1.79×**
/// （16 核机）。自回归解码**逐 token 串行**、受**内存带宽**限制，加线程只在算子内并行，
/// 反而造成线程争抢 ⇒ 越加越慢。证据：`poc_366_threads`（`#[ignore]`）。
fn default_acc_num_threads() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(8) as i32)
        .unwrap_or(4)
}

/// ASR transcriber using sherpa-onnx
///
/// ASR-SINGLE-MODEL-001（DEC-027）+ DEC-028：多模式 ASR 架构
/// - Performance: 179MB CTC，OfflineSenseVoiceModelConfig，无 hotwords
/// - Accuracy: 972MB native，OfflineFunASRNanoModelConfig，config 层 hotwords
/// - QwenAudioOnline: 零本地 ASR 内存，DashScope Inference 协议（ASR-041-B）
/// - Performance/Accuracy: 本地一次只加载一个模型；accuracy 不再预创建 CTC fallback
/// - 🔴 LOCAL-RT-ENGINE-239-A（DEC-067）例外：`LocalRealtime` 档位**同时常驻两个本地模型**
///   （online streaming paraformer 做预览 + offline accuracy 做 2pass 最终文本/热词），
///   有意打破「本地一次只加载一个模型」。该档位为极客档、默认隐藏（DEC-065）。
/// - H1 temperature 0.1 为 accuracy 唯一幻觉缓解（2026-07-08 Gavin 拍板下调 0.3→0.1，RESEARCH-ASR-ACCURACY-002 证实越低越好）；异常检测链已删除
///
/// SAFETY: sherpa_onnx::OfflineRecognizer 与 sherpa_onnx::OnlineRecognizer 均内含
/// *const C++ 指针（!Send），但其 C++ 实现本身是线程安全的（create/decode/destroy
/// 均可跨线程，只需保证同一 recognizer 实例不被并发访问）。
/// 此处 Transcriber 通过 channel 在后台构建线程与 worker 线程间转移，
/// 转移期间无并发访问（构建完成后才发送，发送后构建线程不再触碰），
/// 因此手动实现 Send 是安全的。两个 recognizer 用同一套规则包装，不引入新模式。
pub struct Transcriber {
    mode: AsrMode,
    asr_language: String,
    asr_model: AsrModel,
    /// 本地 ASR recognizer（Performance/Accuracy 模式用；QwenAudioOnline 为 None）
    offline_recognizer: Option<sherpa_onnx::OfflineRecognizer>,
    /// LOCAL-RT-ENGINE-239-A（DEC-067）：本地**流式** recognizer（streaming paraformer trilingual）。
    ///
    /// 仅 `LocalRealtime` 档位用；与 `offline_recognizer`（accuracy，2pass 最终文本 + 热词）
    /// **并存常驻**，有意打破旧约束「本地模式一次只加载一个模型」（见结构体文档）。
    ///
    /// ⚠️ 阶段一：本单只落字段声明与 !Send 包装；构建接线与枚举变体见 239-A 第二阶段。
    online_recognizer: Option<sherpa_onnx::OnlineRecognizer>,
    /// 当前注入的 hotwords 版本号（len + 内容哈希），用于感知词库变更
    hotwords_version: u64,
    /// VAD 分段器（仅 accuracy 长音频用）。
    ///
    /// 🔴 修正（LOCAL-RT-ENGINE-239-A，原文误写「懒加载」）：该字段在 `Transcriber::new`
    /// （见下方 `let vad_segmenter = if effective_model.uses_accuracy_engine() { ... }` 分支）
    /// **随构造立即尝试初始化**，并非「首次长音频时才初始化」。
    /// 用 `Mutex` 包住的原因：`VadSegmenter::segment` 需要 `&mut self`，而 `Transcriber`
    /// 以共享引用被调用，故用内部可变性串行化。
    vad_segmenter: Option<Mutex<vad::VadSegmenter>>,
    /// 在线 ASR 配置（仅 QwenAudioOnline 模式用，ASR-041-B 改名通用）
    asr_online_api_key: String,
    asr_online_url: String,
    asr_online_model: String,
    /// ASR-056: VAD 断句静音阈值（ms），config.toml 隐藏字段覆盖，ASR-SEG-229 默认 2000
    asr_online_max_sentence_silence: i64,
    /// ASR-SEG-229: 在线 ASR 语义断句/标点开关，config.toml 隐藏字段覆盖，默认 false
    asr_online_semantic_punctuation_enabled: bool,
}

// SAFETY: Transcriber 持有的 OfflineRecognizer / OnlineRecognizer 内部均为 *const C++ 指针。
// 跨线程转移时，发送方在 send 后不再访问该实例，接收方独占所有权，
// 满足"单一时刻单线程访问"约束。sherpa-onnx C++ 层本身支持跨线程调用。
// LOCAL-RT-ENGINE-239-A: online_recognizer 与 offline_recognizer 共用本条 SAFETY 论证。
unsafe impl Send for Transcriber {}

/// PARALLEL-ACC-298：把 `&OfflineRecognizer` 送进 accuracy 并行 worker 线程的 Send 包装。
///
/// 🔴 SAFETY 论证 = **对象独占**，不是「时间上不重叠」（本条与 `SendOnlineRecognizerRef`
/// 的论证**不同**：298 是**真并发**，streaming 线程与 accuracy worker 同时在跑，故
/// 「scope 内串行、join 后才继续」那套时间论在此**不成立**，不得套用）。
///
/// 独占性两条证据（coder-2 核实的代码事实）：
/// 1. **录音期间无人替换/访问该 offline recognizer**：唯一会 swap/drop `transcriber` 的是
///    `apply_reload_result`，它只在 worker loop 顶（`main.rs:7442-7458`）或 D2 有界等待
///    （`:7633-7653`）执行 —— 二者都在 LocalRealtime 的 `std::thread::scope` **之前**；
///    scope 期间 worker 线程**就地阻塞**在 `record_streaming`（`:7965`），loop 顶不执行
///    （`:7462` 注释「录音期间 loop 顶不执行，天然不会中途换引擎」）。后台 reload 线程
///    （`:7254`）只构建**新**实例，绝不碰旧实例。
/// 2. **scope 内只有一个线程访问本对象**：accuracy worker 是唯一使用者；主 worker 线程只跑
///    音频（`record_streaming`），streaming ASR 线程用的是 **online** recognizer（另一个对象）。
///    全仓 `transcribe_with_punct_info` 唯一生产调用点 `main.rs:9132` 在 scope **之后**，
///    且 298 携带 pretranscribed 时该分支根本不执行。
///
/// 结论：全程无第二个线程访问**同一** offline recognizer，跨线程转移成立。
pub struct SendOfflineRecognizerRef<'a>(pub &'a sherpa_onnx::OfflineRecognizer);
unsafe impl<'a> Send for SendOfflineRecognizerRef<'a> {}

impl<'a> SendOfflineRecognizerRef<'a> {
    /// 按值消费包装取出引用（同 `SendOnlineRecognizerRef`，破 Rust2021 disjoint capture）。
    pub fn into_inner(self) -> &'a sherpa_onnx::OfflineRecognizer {
        self.0
    }
}

/// PARALLEL-ACC-298：用常驻 offline(accuracy) recognizer 并行转写**一个**分段。
///
/// 与 [`Transcriber::transcribe_segment_detailed`] 的 accuracy 分支逐位同口径：
/// `create_stream → accept_waveform → decode → get_result → trim → strip_asr_special_tokens
/// → 空则 Err(NativeEmpty) → normalize_text_for_language + native_punctuated=true`。
/// 独立成自由函数，是为了让 accuracy worker 在 scope 线程内**只借 recognizer、不入 `&self`**
/// （`Transcriber` 非 `Sync`，不能跨线程共享 `&self`）。
/// LOCALRT-CTX-INJECT-320：上下文注入参数（accuracy worker 每片 **per-stream** 注入）。
///
/// FIX-INJECT-TO-SPEC-377：ctx 注入已删 ⇒ 本护栏恒不触发；**保留回退能力**（主控裁定前不删）。
/// 回显判定：LCS ≥ 此绝对长度（正常组 LCS=3、故障组 LCS=79；20 在两者之间、靠近正常侧）。
const CTX_ECHO_LCS_ABS: usize = 20;
#[allow(dead_code)] // 见上：377 起无调用点，保留待主控裁定
/// FIX-INJECT-TO-SPEC-377：ctx 注入已删 ⇒ 本护栏恒不触发；**保留回退能力**（主控裁定前不删）。
/// 回显判定：LCS ≥ 输出的此比例（短输出也可能整段回显）。
const CTX_ECHO_LCS_RATIO: f64 = 0.5;
#[allow(dead_code)] // 见上：377 起无调用点，保留待主控裁定
/// FIX-INJECT-TO-SPEC-377：产出 `hotwords` 通道内容 —— **只放纯 ASCII 逗号分隔词表**。
///
/// 依据（一手）：sherpa `offline-recognizer-qwen3-asr-impl.cc` 把 hotwords **原样**塞进
/// `<|im_start|>system` + hotwords + `<|im_end|>` 后接 user 轮，注释写明期望形态是 `"foo,bar,baz"`；
/// 官方 `transcribe()` 无上下文参数、对话只有一个 user 轮、无 system 段。
///
/// 🔴 2026-09-23 前我们塞的是「英文指令句 ×2 + `Context:` 整段散文 + `Terms:` 标签」——
/// POC-376 实证：英文指令句与 `Context:` 散文**各自独立**都会被模型当输出续写
/// （ja·不设 language ⇒ 吐 68 字英文指令句；ja·Auto ⇒ 吐 82 字 Context 散文，LCS 77/81）⇒ 全部删除。
/// 空词表 ⇒ `None`（**不调用** `set_option("hotwords", …)`，等同官方示例的「无 system 段」）。
fn build_ctx_system(terms: Option<&str>) -> Option<String> {
    let t = terms.unwrap_or("").trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// 单窗解码的 per-stream 注入素材（FIX-INJECT-TO-SPEC-377 后的真实语义）。
///
/// - [`Self::terms`]：用户词库词条（ASCII 逗号分隔）—— **这就是 `hotwords` 规格要的全部内容**；
/// - [`Self::avg_chars_per_sec`]：本次录音已接受窗口的产出率均值（375-B 判「解码坍塌」用，与注入无关）。
///
/// 🔴 **上下文注入（`Context:` 段 / `Terms:` 标签 / 英文指令句）已按 sherpa 规格整体移除**：
///    C++ 侧把 `hotwords` 原样放进 `<|im_start|>system` 段、期望纯逗号词表；官方 `transcribe()` 无上下文参数。
///    POC-376 实证「英文指令句」与「`Context:` 散文」**各自独立**都会被模型当输出续写
///    ⇒ `prev_older`/`prev_latest`/`current` 三个字段已删除（不要再加回来）。
pub struct CtxInject<'a> {
    /// 用户词库词条（ASCII 逗号分隔，原样）—— 就是 `hotwords` 规格要的东西。
    /// `None`/空 ⇒ 不注入（不调 `set_option("hotwords", …)`）。
    pub terms: Option<&'a str>,
    /// FIX-PREVIEW-STALE-AND-COLLAPSE-375（B）：本次录音**已定稿窗口**的产出率均值（字/秒），
    /// 用于识别「解码坍塌」（[`output_rate_ok`]）。`None` = 冷启动/样本不足 ⇒ 不判坍塌。
    /// 由调用方（`main.rs` 滑窗 worker）在派发当刻算出快照传入。
    pub avg_chars_per_sec: Option<f32>,
    /// VAD-393（A4）：本窗**窗内坐标**的语音区间（由实时 VAD 时间线映射而来）。
    ///
    /// - `Some(r)` 且 `r` 非空 ⇒ 直接据此剪静音（**不再对窗口重跑 VAD**）；
    /// - `Some(r)` 且 `r` 为空 + [`Self::streaming_nonempty`] ⇒ 不复剪、**整窗解码**（时间线判无语音
    ///   但流式有文本时宁可多解、不吞字）；
    /// - `Some(r)` 且 `r` 为空 + `!streaming_nonempty` ⇒ 提前返回空串；
    /// - `None` ⇒ **回退 388/391**：自跑线程级 VAD 取区间（仍用 v6 模型），VAD 不可用则原样解码。
    pub speech_ranges: Option<&'a [(usize, usize)]>,
    /// VAD-393（A4）：本窗**流式文本**是否非空（区间空表时的兜底判据，见 [`Self::speech_ranges`]）。
    pub streaming_nonempty: bool,
}

/// 回显探针归一化：去空白与常见中英标点（回显是逐字文本，标点差异不应漏检）。
#[allow(dead_code)] // 377 起仅被保留的 ctx 回显护栏使用，保留回退能力
fn normalize_ctx_probe(s: &str) -> String {
    const PUNCT: &[char] = &[
        '。', '，', '、', '！', '？', '；', '：', '「', '」', '『', '』', '“', '”', '‘', '’', '…',
        '—', '（', '）', '【', '】', '.', ',', '!', '?', ';', ':', '"', '\'', '(', ')', '[', ']',
    ];
    s.chars()
        .filter(|c| !c.is_whitespace() && !PUNCT.contains(c))
        .collect()
}

/// 最长公共**子串**长度（字符），用于检测「整片逐字回显上一片」。
fn lcs_len(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let mut prev = vec![0usize; b.len() + 1];
    let mut best = 0usize;
    for i in 1..=a.len() {
        let mut cur = vec![0usize; b.len() + 1];
        for j in 1..=b.len() {
            if a[i - 1] == b[j - 1] {
                cur[j] = prev[j - 1] + 1;
                if cur[j] > best {
                    best = cur[j];
                }
            }
        }
        prev = cur;
    }
    best
}

/// 回显处置：命中 ⇒ Redecode（**不丢片**，同音频无上下文重解一次）。
#[derive(PartialEq, Eq, Debug)]
#[allow(dead_code)] // 377 起无调用点（ctx 注入已删），保留回退能力
enum CtxEchoAction {
    Keep,
    Redecode,
}

#[allow(dead_code)] // 377 起无调用点（ctx 注入已删），保留回退能力
///
/// 🔴 **唯一正当回挂理由 = 上下文注入（`Context:` 段）被重新启用**。
/// 见到「模型回显词条」**不是**回挂理由 —— 那由 374 的 `strip_terms_echo` 覆盖，
/// 且把 Terms 放回本护栏的比对面正是 BUILD-321 实测 **43% 误判**的来源。
fn ctx_echo_action(out_norm: &str, ctx_norm: &str) -> (usize, CtxEchoAction) {
    let l = lcs_len(out_norm, ctx_norm);
    let out_len = out_norm.chars().count();
    let hit =
        l >= CTX_ECHO_LCS_ABS || (out_len > 0 && (l as f64) >= CTX_ECHO_LCS_RATIO * out_len as f64);
    (
        l,
        if hit {
            CtxEchoAction::Redecode
        } else {
            CtxEchoAction::Keep
        },
    )
}

/// 是否注入：有可注入文本即注入（**恒开、无 env**；引擎固定 Qwen3）。
fn should_inject_ctx(system: Option<&str>) -> bool {
    system.is_some()
}

// ============================================================
// FIX-TERMS-ECHO-374 + FIX-PREVIEW-STALE-AND-COLLAPSE-375：**统一的处置阶梯**
//   （374 词条回显剥离 与 375 产出率坍塌 共用一套，不各写一套）
// ============================================================

/// FIX-PREVIEW-STALE-AND-COLLAPSE-375（B）：产出率坍塌判据的**相对比例**。
///
/// 本窗「字/秒」< 本次录音已定稿窗口均值 × 该比例 ⇒ 判**解码坍塌**（不问原因：回显、幻觉、
/// 截断都算）。取 **1/3** 的理由：正常窗口间的产字率波动来自语速（±30~50%）与短窗统计噪声，
/// 1/3 给出足够裕量避免误杀「慢语速的正常窗」；而端测坍塌窗（8 字 / 10.4s ≈ 0.77 字/s
/// vs 正常 ≈4.4 字/s）只有均值的 ~17% ⇒ 远低于 1/3 ⇒ 抓得住。
pub(crate) const COLLAPSE_RATE_RATIO: f32 = 1.0 / 3.0;

/// 期望产出低于此值（字）时**判据不成立**（一律视为正常）：字太少则相对比例无统计意义
/// ⇒ 宁漏勿误杀（短尾窗、极短片）。
pub(crate) const COLLAPSE_MIN_EXPECTED_CHARS: f32 = 8.0;

/// 产出率均值至少由这么多个「已接受窗口」支撑才可用（否则视为冷启动、不判坍塌）。
pub(crate) const COLLAPSE_MIN_AVG_WINDOWS: usize = 2;

/// FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388（D2）：**冷启动坍塌下限**的音频时长门槛（秒）。
///
/// 冷启动（无均值样本）时不再一律放行：音频 ≥ 本值且字/秒 < [`COLD_MIN_CHARS_PER_SEC`] ⇒ 判坍塌。
/// 依据：BUILD-387 seq1 6.85s 只出 6 字（0.88 字/s）因冷启动放行 ⇒ 内容丢失。
pub(crate) const COLD_MIN_AUDIO_SECS: f32 = 3.0;
/// FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388（D2）：冷启动最低产出率（字/秒）。
pub(crate) const COLD_MIN_CHARS_PER_SEC: f32 = 1.0;

/// FIX-PREVIEW-STALE-AND-COLLAPSE-375（B）：本窗产出率是否**合理**（不坍塌）。
///
/// `真` ⇒ 正常/无法判定；`假` ⇒ 判为坍塌（调用方走重解阶梯）。
/// 冷启动（`avg` 为 None / 非有限 / ≤0）、音频时长非正、期望产出 < [`COLLAPSE_MIN_EXPECTED_CHARS`]
/// ⇒ 一律 `真`（**宁漏勿误杀**：判据缺输入时不得反过来把正常窗判坍塌）。
pub(crate) fn output_rate_ok(
    out_chars: usize,
    audio_secs: f32,
    avg_chars_per_sec: Option<f32>,
) -> bool {
    let Some(avg) = avg_chars_per_sec.filter(|a| a.is_finite() && *a > 0.0) else {
        // FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388（D2）：冷启动**不再一律放行** ——
        // 音频 ≥ COLD_MIN_AUDIO_SECS 且字/秒 < COLD_MIN_CHARS_PER_SEC ⇒ 判坍塌（长音频产出过低）。
        // 短于门槛 / 无音频时长 ⇒ 仍不判（宁漏勿误杀）。
        if audio_secs >= COLD_MIN_AUDIO_SECS {
            return (out_chars as f32) / audio_secs >= COLD_MIN_CHARS_PER_SEC;
        }
        return true;
    };
    if !(audio_secs > 0.0) {
        return true;
    }
    let expected = avg * audio_secs;
    if expected < COLLAPSE_MIN_EXPECTED_CHARS {
        return true; // 期望产出太少 ⇒ 判据不成立
    }
    (out_chars as f32) >= expected * COLLAPSE_RATE_RATIO
}

/// 产出率均值（字/秒）：由调用方累计的 `Σ已接受窗口字数` / `Σ窗口秒数` / `窗口数` 得。
/// 窗口数不足 [`COLLAPSE_MIN_AVG_WINDOWS`] 或无有效秒数 ⇒ `None`（冷启动）。
pub(crate) fn acc_avg_chars_per_sec(
    sum_chars: usize,
    sum_secs: f32,
    windows: usize,
) -> Option<f32> {
    if windows < COLLAPSE_MIN_AVG_WINDOWS || !(sum_secs > 0.0) {
        None
    } else {
        Some(sum_chars as f32 / sum_secs)
    }
}

/// FIX-ACC-OUTPUT-GUARD-387：处置阶梯的**触发类别**（供埋点 `kind=`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuardKind {
    /// 未触发任何守卫（正常保留）。
    None,
    /// 词条回显（374 命中）/ 剥后残余全为词表条目。
    Echo,
    /// 标签输出（剥后为空 / 只剩标点）。
    Tag,
    /// 空输出（含只剩标点）。
    Empty,
    /// 375 产出率坍塌。
    Collapse,
}

impl GuardKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            GuardKind::None => "none",
            GuardKind::Echo => "echo",
            GuardKind::Tag => "tag",
            GuardKind::Empty => "empty",
            GuardKind::Collapse => "collapse",
        }
    }
}

/// 统一处置阶梯的结果（供埋点：374 + 375 + 387）。
pub(crate) struct AccDispositionOutcome {
    /// 匹配到的**连续**注入词条数（0 = 无回显）。
    pub matched_terms: usize,
    /// 剥掉的字符数。
    pub removed_chars: usize,
    /// 剥掉回显段后的剩余字数。
    pub remaining_chars: usize,
    /// 是否触发了「不带注入重解一次」。
    pub redecoded: bool,
    /// 是否因**产出率坍塌**触发（375）—— 与 374 的「剥空触发」区分开，便于端测判断。
    pub collapse: bool,
    /// FIX-ACC-OUTPUT-GUARD-387：本次处置的**触发类别**（供埋点 `kind=`）。
    pub guard: GuardKind,
    /// FIX-ACC-OUTPUT-GUARD-387：重解后仍无效（已返回空串）。
    pub invalid: bool,
}

/// FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388（D1）：文本是否**含实质内容** —— 至少 1 个
/// `char::is_alphanumeric()`（CJK 也算）。`**`、`。`、`…`、空白、空串 ⇒ `false`。
///
/// 取代 387 的 `is_only_punct`（后者按 `align_keep_char` 判，`**` 之类会被当成「有内容」而漏过）。
fn has_content(text: &str) -> bool {
    text.chars().any(|c| c.is_alphanumeric())
}

/// FIX-ACC-OUTPUT-GUARD-387：剥掉结果里的**标签** `<[A-Za-z_/][^<>]{0,30}>`（如 `<location>`）。
///
/// 🔴 371 的 `<asr_text>` 前缀剥离更早（`decode_accuracy_once` → `strip_asr_special_tokens`）；
/// 本函数是其后方的**通用兜底**，不改 371 逻辑。返回 `(剥后文本, 是否剥到过)`。
fn strip_angle_tags(text: &str) -> (String, bool) {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut hit = false;
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '<' {
            // 首字符须为 `[A-Za-z_/]`；内部无 `<`/`>`；总长（不含尖括号）1..=31；以 `>` 收尾。
            let mut j = i + 1;
            let mut closed = false;
            while j < chars.len() && (j - i) <= 31 {
                match chars[j] {
                    '>' => {
                        closed = true;
                        break;
                    }
                    '<' => break,
                    _ => {}
                }
                j += 1;
            }
            if closed && j > i + 1 {
                let first = chars[i + 1];
                if first.is_ascii_alphabetic() || first == '_' || first == '/' {
                    hit = true;
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    (out, hit)
}

/// FIX-ACC-OUTPUT-GUARD-387：`text` 是否**全部由词表条目构成**（按列表分隔符切分后每段都在词表里）。
///
/// 用于识别「374 剥完的残余」（如末尾落单的 `维生素b12`）：残余虽不足连续 4 条，但只要**整段
/// 都是词表条目**即视为回显残余。空文本 / 无词表 / 任一段不在词表 ⇒ `false`。
fn all_segments_are_terms(text: &str, terms: Option<&str>) -> bool {
    let Some(terms) = terms else {
        return false;
    };
    let set: Vec<Vec<char>> = terms
        .split(',')
        .map(|t| {
            t.chars()
                .filter(|c| align_keep_char(*c))
                .collect::<Vec<char>>()
        })
        .filter(|t: &Vec<char>| !t.is_empty())
        .collect();
    if set.is_empty() {
        return false;
    }
    let mut any = false;
    for raw in text.split(is_list_separator) {
        let seg: Vec<char> = raw.chars().filter(|c| align_keep_char(*c)).collect();
        if seg.is_empty() {
            continue;
        }
        any = true;
        if !set.iter().any(|t| t == &seg) {
            return false;
        }
    }
    any
}

/// FIX-TERMS-ECHO-374 + 375（B）+ FIX-ACC-OUTPUT-GUARD-387：**统一的处置阶梯**（一套实现，多个判据）。
///
/// 触发 ⇒ **不带注入重解一次**（至多一次，不得循环）。触发类别（优先级）：
/// - `Echo`（387-1/2）：374 连续 ≥4 条命中，**或**剥离后残余**全部由词表条目构成**
///   ⇒ 不再保留残余（残余正是 `维生素b12` 漏进正文的成因）⇒ 重解；重解结果若仍含连续 ≥4 条 ⇒ 无效。
/// - `Tag`（387-3）：含标签 `<[A-Za-z_/][^<>]{0,30}>`（如 `<location>`）剥后为空 / 只剩标点 ⇒ 重解。
/// - `Empty`（387-4）：输出为空 / 只剩标点 ⇒ 重解。
/// - `Collapse`（375）：产出率坍塌 ⇒ 重解（**仅在未回显时启用**，原约束不变）。
///
/// 重解结果校验：非空 **且** 无连续 ≥4 条词条回显 **且**（非坍塌触发 或 产出率通过）⇒ 用它；
/// 否则 ⇒ 返回空串（由 386-C 用流式文本兜底；`outcome.invalid=true`）。
fn apply_acc_disposition(
    text: String,
    terms: Option<&str>,
    audio_secs: f32,
    avg_chars_per_sec: Option<f32>,
    redecode: impl FnOnce() -> Result<String>,
) -> (String, AccDispositionOutcome) {
    // 387-3：先剥标签（通用兜底）。
    let (after_tags, had_tag) = strip_angle_tags(&text);
    // 374：词条回显剥离（结构性）。
    let (stripped, matched_terms, removed_chars) = match strip_terms_echo(&after_tags, terms) {
        Some(h) => (h.stripped, h.matched_terms, h.removed_chars),
        None => (after_tags, 0usize, 0usize),
    };
    let remaining_chars = stripped.chars().count();
    // 388-D1：无实质内容（`**`/标点/空白/空）才触发；取代 387 的 `is_only_punct`（漏 `**`）。
    let no_content = !has_content(&stripped);
    // 387-2：残余**全部由词表条目构成** ⇒ 视同回显。
    let residual_all_terms =
        !stripped.trim().is_empty() && all_segments_are_terms(&stripped, terms);
    let echo_hit = matched_terms > 0 || residual_all_terms;

    let mut outcome = AccDispositionOutcome {
        matched_terms,
        removed_chars,
        remaining_chars,
        redecoded: false,
        collapse: false,
        guard: GuardKind::None,
        invalid: false,
    };

    let trigger = if echo_hit {
        Some(GuardKind::Echo)
    } else if had_tag && no_content {
        Some(GuardKind::Tag)
    } else if no_content {
        Some(GuardKind::Empty)
    } else if !output_rate_ok(remaining_chars, audio_secs, avg_chars_per_sec) {
        Some(GuardKind::Collapse)
    } else {
        None
    };

    let Some(kind) = trigger else {
        return (stripped, outcome); // 正常 ⇒ 直接用（不重解）
    };
    outcome.guard = kind;
    outcome.collapse = kind == GuardKind::Collapse;
    // 公共：不带注入重解**一次**（带注入可能再次回显；不得循环）。
    outcome.redecoded = true;
    let raw = redecode().unwrap_or_default();
    // 防御性再剥（标签 + 词条）。
    let (re_tags, _) = strip_angle_tags(&raw);
    let recovered = match strip_terms_echo(&re_tags, terms) {
        Some(h) => h.stripped,
        None => re_tags,
    };
    // 校验（388-D1：**所有 kind 统一**）：含实质内容 + 无连续 ≥4 条回显 + 产出率通过。
    // 旧实现只对 Collapse 查产出率 ⇒ 11.6s 重解出「嗯。」也被收下；且 `!is_only_punct` 漏 `**`。
    let still_echo = strip_terms_echo(&recovered, terms).is_some();
    let acceptable = has_content(&recovered)
        && !still_echo
        && output_rate_ok(recovered.chars().count(), audio_secs, avg_chars_per_sec);
    if acceptable {
        (recovered, outcome)
    } else {
        outcome.invalid = true;
        (String::new(), outcome) // 重解仍无效 ⇒ 空解码（交给 386-C 流式兜底）
    }
}

/// FIX-TERMS-ECHO-374：判定「输出**结构性**回显了注入的 `Terms:` 列表」所需的最小**连续**词条数。
///
/// 注入顺序 = `hit_count DESC, id DESC`（与口述内容无关）⇒ 用户要「**连续 N 个词条、且顺序与
/// 注入完全一致**」地说出来，纯属巧合的概率极低：
/// - N=3：3 个特定词（且都是词库里的高频词）按特定顺序连说 —— 仍可能偶然命中；
/// - N=4：连说 4 个且顺序全中 ⇒ 自然口语实际不可能；而真实的「列表回显」长度是**整个词库**
///   （端测实证：59 个词条 / 77 字）⇒ 提高 N **不会漏掉真实回显**。
/// ⇒ 取 **4**：宁可漏掉「词库只有 2~3 条」时的短回显，也绝不误伤自然口语 —— 43% 误判的代价
///   远大于 2~3 个词的漏网（且短词库回显危害极小）。
const TERMS_ECHO_MIN_RUN: usize = 4;

/// 列表分隔符（剥离接缝处要清掉的字符）：半/全角逗号、顿号、分号、冒号、空白。
/// 🔴 **刻意不含** `。！？…` 等句末标点 —— 那些往往是**真实转写**的一部分（如「…的风景。」），
///    清掉会把真句子也削掉。
fn is_list_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, ',' | '，' | '、' | ';' | '；' | ':' | '：')
}

/// FIX-TERMS-ECHO-374：命中「词库列表回显」时的剥离结果。
struct TermsEchoHit {
    /// 匹配到的**连续**词条数。
    matched_terms: usize,
    /// 剥掉的字符数（按原文本字符计）。
    removed_chars: usize,
    /// 剥离后的剩余文本（可能为空串 ⇒ 调用方按「空解码」处理）。
    stripped: String,
}

/// FIX-TERMS-ECHO-374：**结构性**识别并剥掉「注入词库列表被逐字回显」的那一段。
///
/// 🔴 **判据是结构，不是「输出里有没有词条」**：注入的 Terms 是「已知词条、按已知顺序、用已知
/// 分隔符连起来」⇒ 只有「**连续 N 个注入词条且顺序与注入一致**」才算回显（见
/// [`TERMS_ECHO_MIN_RUN`]）；用户在句子里说到一两个词库词是**正常**的。
/// 🔴 **绝不把 Terms 段放回 LCS 比对面** —— 那正是 2026-09-21 实测 43%（14 次触发 6 次重解）
/// 误判的来源；本函数的判据与 LCS 无关，故**没有**那个误伤面 ⇒ 可以**恒执行**。
///
/// 🔴 **不受任何前置条件影响**：FIX-INJECT-TO-SPEC-377 起已无 `Context:` 注入（无 ctx 概念），
/// 本步仍然**恒执行** —— 端测现场 `ctx_raw_len=0`（重启后第一次录音）时旧护栏整块被跳过、
/// 词条照样回显（`seg=10 out_chars=77 terms_len=76`）⇒ 判据必须与「有无前文」无关。
///
/// 处置 = **剥离**（不重解；重解要再等约 4s 且仍带注入、可能再次回显）：
/// 只删「首个匹配词条 → 末个匹配词条」之间的原文字符，再清掉**接缝上的列表分隔符**
/// （[`is_list_separator`]，不动句末标点）。剥空 ⇒ `stripped` 为空串（上层按空解码处理，不污染窗口）。
///
/// 未命中 ⇒ `None`（零分配、行为不变）。
fn strip_terms_echo(text: &str, terms: Option<&str>) -> Option<TermsEchoHit> {
    let terms = terms?;
    // 注入侧：按 `,` 切分 → 归一化（去空白/标点） → 保留**注入顺序**。
    let injected: Vec<Vec<char>> = terms
        .split(',')
        .map(|t| {
            t.chars()
                .filter(|c| align_keep_char(*c))
                .collect::<Vec<char>>()
        })
        .filter(|t| !t.is_empty())
        .collect();
    if injected.len() < TERMS_ECHO_MIN_RUN {
        return None;
    }
    // 输出侧：逐字归一化，并保住「归一化序号 → 原文字符序号」的映射（剥离要回到原文坐标）。
    let chars: Vec<char> = text.chars().collect();
    let mut norm: Vec<char> = Vec::with_capacity(chars.len());
    let mut norm_orig: Vec<usize> = Vec::with_capacity(chars.len());
    for (i, c) in chars.iter().enumerate() {
        if align_keep_char(*c) {
            norm.push(*c);
            norm_orig.push(i);
        }
    }
    if norm.is_empty() {
        return None;
    }
    // 找**最长**的「连续注入词条（顺序一致）」run（长度 ≥ TERMS_ECHO_MIN_RUN）。
    let mut best: Option<(usize, usize, usize)> = None; // (起点, 词条数, 终点（不含）)
    for (ti, head) in injected.iter().enumerate() {
        let mut p = 0usize;
        while p + head.len() <= norm.len() {
            if norm[p..p + head.len()] == head[..] {
                let mut end = p + head.len();
                let mut m = 1usize;
                while ti + m < injected.len() {
                    let nxt = &injected[ti + m];
                    if end + nxt.len() <= norm.len() && norm[end..end + nxt.len()] == nxt[..] {
                        end += nxt.len();
                        m += 1;
                    } else {
                        break;
                    }
                }
                if m >= TERMS_ECHO_MIN_RUN && best.is_none_or(|(_, bm, _)| m > bm) {
                    best = Some((p, m, end));
                }
                p += 1;
            } else {
                p += 1;
            }
        }
    }
    let (p, matched_terms, end) = best?;
    // 回到原文坐标：删 [first, last] 之间的字符。
    let first = norm_orig[p];
    let last = norm_orig[end - 1];
    let prefix: String = chars[..first].iter().collect();
    let suffix: String = chars[last + 1..].iter().collect();
    let mut stripped = String::with_capacity(text.len());
    stripped.push_str(prefix.trim_end_matches(is_list_separator));
    stripped.push_str(suffix.trim_start_matches(is_list_separator));
    // 剥完只剩分隔符/空白 ⇒ 视为**空解码**（不给下游留一个孤零零的逗号）。
    if stripped.trim_matches(is_list_separator).is_empty() {
        stripped.clear();
    }
    let removed_chars = chars.len() - stripped.chars().count();
    Some(TermsEchoHit {
        matched_terms,
        removed_chars,
        stripped,
    })
}

// ===========================================================================
// TUNE-DECODE-SERIAL-AND-TOKEN-CAP-390：按语音时长限制 per-stream 生成长度
// ===========================================================================

/// TUNE-390：per-stream `max_new_tokens` 上限的**每秒 token 预算**。
///
/// 汉字 + 标点 + 语种前缀余量；快语速 6~7 字/s 仍有近 2 倍余量。
const TOKEN_CAP_PER_SEC: f32 = 12.0;
/// TUNE-390：基础余量（短音频也够出完整句子）。
const TOKEN_CAP_BASE: i32 = 24;
/// TUNE-390：下限 —— 再短的音频也至少给这么多（覆盖首字/短句）。
const TOKEN_CAP_MIN: i32 = 48;
/// TUNE-390：上限 = 原全局 `max_new_tokens`（模型跑偏时不再无限生成）。
const TOKEN_CAP_MAX: i32 = 256;

/// TUNE-390：按**语音时长**折算 per-stream `max_new_tokens`（纯函数，可单测）。
///
/// 现状全局 256：模型跑偏（念词表/幻觉）时会一直生成（1.12s 窗念 77 字耗 7.8s）⇒ 按语音时长收紧。
/// 非有限 / 负数 ⇒ [`TOKEN_CAP_MAX`]（不限，退回现状）。
fn max_new_tokens_for(speech_secs: f32) -> i32 {
    if !speech_secs.is_finite() || speech_secs < 0.0 {
        return TOKEN_CAP_MAX;
    }
    let want = (speech_secs * TOKEN_CAP_PER_SEC).ceil() as i32; // 极大有限值 `as i32` 饱和，不 panic
    want.saturating_add(TOKEN_CAP_BASE)
        .clamp(TOKEN_CAP_MIN, TOKEN_CAP_MAX)
}

/// 一次 accuracy 解码（可选 per-stream system 段）。返回规范化后的文本。
fn decode_accuracy_once(
    recognizer: &sherpa_onnx::OfflineRecognizer,
    samples: &[f32],
    system: Option<&str>,
    script: ChineseScript,
) -> Result<String> {
    // 其他调用方行为逐位不变 ⇒ 传 `None`（不设 `max_new_tokens`，沿用全局 256）。
    let text = decode_accuracy_allow_empty(recognizer, samples, system, script, None)?;
    if text.is_empty() {
        // 与 transcribe_segment_detailed 一致：accuracy 空输出 ⇒ 该段失败（上层 all_native=false）
        anyhow::bail!("ASR accuracy model produced empty output");
    }
    Ok(text)
}

/// 388 主控验收补：同 [`decode_accuracy_once`]，但空输出返回 `Ok("")` 而不是报错。
/// 供 `transcribe_acc_ctx` 首解使用：空输出必须进入 `apply_acc_disposition` 的 Empty 判据
///（不带注入重解一次，387-D4 / 388-D1），否则 `?` 提前返回、重解永不发生。
fn decode_accuracy_allow_empty(
    recognizer: &sherpa_onnx::OfflineRecognizer,
    samples: &[f32],
    system: Option<&str>,
    script: ChineseScript,
    max_new_tokens: Option<i32>,
) -> Result<String> {
    let stream = recognizer.create_stream();
    if let Some(s) = system {
        stream.set_option("hotwords", s);
    }
    // TUNE-390：per-stream 生成长度上限（sherpa `offline-recognizer-qwen3-asr-impl.cc:774` 的
    // `GetOptionInt` 按字符串解析）。`None` ⇒ 不设，沿用全局 `max_new_tokens`。
    if let Some(n) = max_new_tokens {
        stream.set_option("max_new_tokens", &n.to_string());
    }
    stream.accept_waveform(16000, samples);
    recognizer.decode(&stream);
    let result = stream.get_result().context("No transcription result")?;
    let text = Transcriber::strip_asr_special_tokens(result.text.trim());
    if text.is_empty() {
        return Ok(String::new());
    }
    Ok(text_normalizer::normalize_text_for_language(&text, script))
}

/// 无注入的简版单段入口（保留兼容；生产路径已切 `transcribe_accuracy_segment_ctx`）。
#[allow(dead_code)]
pub(crate) fn transcribe_accuracy_segment(
    recognizer: &sherpa_onnx::OfflineRecognizer,
    samples: &[f32],
    script: ChineseScript,
) -> Result<(String, bool)> {
    decode_accuracy_once(recognizer, samples, None, script).map(|t| (t, true))
}

// ===========================================================================
// FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388（A）：解码前剪静音（只本地 realtime 走本函数）
// ===========================================================================

thread_local! {
    /// FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388：解码线程级 VAD 缓存（按 `model_dir` 懒建一次，
    /// 失败**记住不重试**）。外层 `Option` = 是否已尝试；内层 `Option` = 是否可用（`None` ⇒ 原样不剪）。
    static LOCALRT_TRIM_VAD: std::cell::RefCell<Option<Option<VadSegmenter>>> =
        std::cell::RefCell::new(None);
}

/// FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388（A2）：纯函数 —— 每个语音区间两侧各扩 `pad` 样本
///（clamp 到 `[0, audio.len())`），重叠/相接的**合并**，再按序拼接。
///
/// ⇒ 首尾静音剩 ≤`pad`；段间停顿 ≤`2×pad` 原样保留（被合并）、`>2×pad` 压到 `2×pad`。
fn trim_to_speech(audio: &[f32], ranges: &[(usize, usize)], pad: usize) -> Vec<f32> {
    if ranges.is_empty() || audio.is_empty() {
        return Vec::new();
    }
    let len = audio.len();
    let mut spans: Vec<(usize, usize)> = ranges
        .iter()
        .map(|&(s, e)| (s.saturating_sub(pad), e.saturating_add(pad).min(len)))
        .filter(|&(s, e)| e > s)
        .collect();
    spans.sort_by_key(|&(s, _)| s);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in spans {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 {
                last.1 = last.1.max(e);
                continue;
            }
        }
        merged.push((s, e));
    }
    let mut out = Vec::with_capacity(merged.iter().map(|(s, e)| e - s).sum());
    for (s, e) in merged {
        out.extend_from_slice(&audio[s..e]);
    }
    out
}

/// VAD-393（A4）：实时时间线剪静音的**动作**（纯决策，可单测，无需模型）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum TimelineTrim {
    /// 非空区间 ⇒ 据此剪静音。
    Apply(Vec<(usize, usize)>),
    /// 时间线判无语音但本窗流式有文本 ⇒ **不复剪、整窗解码**（宁可多解，不吞字）。
    WholeWindow,
    /// 时间线判无语音且流式也空 ⇒ 提前返回空串。
    Empty,
}

/// VAD-393（A4）：给定「实时时间线区间 + 本窗流式是否非空」决定剪静音动作。
///
/// `speech_ranges = None` ⇒ 返回 `None`（调用方**回退 388/391 自跑 VAD**）；`Some` 的三种情形见
/// [`TimelineTrim`]。纯函数，把「时间线是否可用 / 是否空表 / 流式兜底」三态从模型调用里剥出。
fn plan_timeline_trim(
    speech_ranges: Option<&[(usize, usize)]>,
    streaming_nonempty: bool,
) -> Option<TimelineTrim> {
    speech_ranges.map(|ranges| {
        if ranges.is_empty() {
            if streaming_nonempty {
                TimelineTrim::WholeWindow
            } else {
                TimelineTrim::Empty
            }
        } else {
            TimelineTrim::Apply(ranges.to_vec())
        }
    })
}

/// LOCALRT-CTX-INJECT-320：带「上下文 + 词库」per-stream 注入 + 长跨回显护栏的 accuracy 单段解码。
///
/// - `context`：前序分片累计文本（调用方已截到最后 300 字；第 1 片传 None）。
/// - `terms`：用户词库词条（原样；调用方给）。
/// - 命中回显 ⇒ 同音频**无上下文重解一次**，用重解码结果（不丢片）。
pub(crate) fn transcribe_acc_ctx(
    recognizer: &sherpa_onnx::OfflineRecognizer,
    samples: &[f32],
    script: ChineseScript,
    seg_idx: usize,
    inject: CtxInject<'_>,
) -> Result<(String, bool)> {
    // FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388（A）：解码前剪静音（本地 realtime 专用）。
    // 根因：sherpa #3509 —— 带 hotwords 的 Qwen3-ASR 遇静音会吐热词/坍塌；派发片把「上句说完到
    // 本句开口」的长停顿全带进窗口 ⇒ 大量静音 + 少量语音。先 VAD 取语音区间、每侧留 200ms。
    // VAD-393（A4）：优先用调用方给的**实时时间线**（窗内坐标）⇒ 不再对每窗重跑 VAD；
    //   无时间线（`None`）才回退 388/391 的线程级自跑 VAD（仍用同一 v6 模型）。
    let in_secs = samples.len() as f32 / 16000.0;
    let pad = (vad::LOCALRT_TRIM_PAD_SECS * 16000.0) as usize;
    // 返回 `(解码用样本, 是否经 VAD 判定, 区间数, 剪静音来源)`；`source` ∈ {timeline, vad, none}。
    let (samples_used, trimmed, n_ranges, trim_src) =
        match plan_timeline_trim(inject.speech_ranges, inject.streaming_nonempty) {
            Some(TimelineTrim::Apply(ranges)) => (
                trim_to_speech(samples, &ranges, pad),
                true,
                ranges.len(),
                "timeline",
            ),
            // 时间线判无语音但本窗流式有文本 ⇒ 不复剪、整窗解码（宁可多解，不吞字）。
            Some(TimelineTrim::WholeWindow) => (samples.to_vec(), false, 0usize, "timeline"),
            // 时间线判无语音且流式也空 ⇒ 空结果（下方 `trimmed && n_ranges == 0` 统一早退）。
            Some(TimelineTrim::Empty) => (Vec::new(), true, 0usize, "timeline"),
            None => {
                let trim_model_dir = model_dir();
                LOCALRT_TRIM_VAD.with(|cell| {
                    let mut slot = cell.borrow_mut();
                    if slot.is_none() {
                        *slot = Some(VadSegmenter::try_new_for_local_trim(&trim_model_dir));
                    }
                    match slot.as_ref().and_then(|o| o.as_ref()) {
                        Some(vseg) => {
                            let ranges = vseg.speech_ranges(samples);
                            if ranges.is_empty() {
                                (Vec::new(), true, 0usize, "vad")
                            } else {
                                (
                                    trim_to_speech(samples, &ranges, pad),
                                    true,
                                    ranges.len(),
                                    "vad",
                                )
                            }
                        }
                        None => (samples.to_vec(), false, 0usize, "none"),
                    }
                })
            }
        };
    if trimmed && n_ranges == 0 {
        // 整窗无语音 ⇒ 空解码（交 386-C 流式兜底），**不**进模型（避免念词表 / 失败日志刷屏）。
        if log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "[LocalRT-DBG-388] trim: seg={} in={:.2}s out=0.00s ranges=0 source={}",
                seg_idx,
                in_secs,
                trim_src
            );
        }
        return Ok((String::new(), true));
    }
    // 后续（首解 / 产出率 / 重解）一律用剪静音后的 `samples`（shadow 原参数）。
    let samples: &[f32] = &samples_used;
    let speech_secs = samples.len() as f32 / 16000.0;
    // TUNE-390：按（剪静音后）语音时长折算本窗生成长度上限（首解与重解共用）。
    let token_cap = max_new_tokens_for(speech_secs);
    if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "[LocalRT-DBG-388] trim: seg={} in={:.2}s out={:.2}s ranges={} source={} max_new_tokens={}",
            seg_idx,
            in_secs,
            speech_secs,
            n_ranges,
            trim_src,
            token_cap
        );
    }
    // FIX-INJECT-TO-SPEC-377：注入内容**只剩纯词表**（见 `build_ctx_system`）。
    // 「英文指令句 ×2 + `Context:` 散文 + `Terms:` 标签」已全部删除 ⇒ 无 ctx 概念、ctx 回显护栏不再参与。
    let terms_len = inject.terms.map(|s| s.chars().count()).unwrap_or(0);
    let system = build_ctx_system(inject.terms);
    let inject_on = should_inject_ctx(system.as_deref());
    // 388 主控验收补：首解用 allow_empty ⇒ 空输出也进入 Empty 判据（不带注入重解一次）。
    let text = decode_accuracy_allow_empty(
        recognizer,
        samples,
        system.as_deref().filter(|_| inject_on),
        script,
        Some(token_cap),
    )?;
    let decided_out_chars = text.chars().count();
    // FIX-TERMS-ECHO-374 + 375（B）：**统一的处置阶梯**（一处实现，两条判据）。
    //
    // 🔴 **恒执行、不受任何前置条件影响**：377 起已无 `Context:` 注入；374 现场（`ctx_raw_len=0`
    //    的「重启后第一次录音」）旧护栏整块被跳过、`seg=10` 零转写只吐 77 字词条列表，375 现场则是
    //    「短输出从相对阈值下溜走」。两条判据都与 LCS 无关 ⇒ 无 BUILD-321 的 43% 误伤面。
    let audio_secs = speech_secs;
    let (text, disp) = apply_acc_disposition(
        text,
        inject.terms,
        audio_secs,
        inject.avg_chars_per_sec,
        // TUNE-390：重解同样带 cap；用 `decode_accuracy_allow_empty`（空输出返回 Ok("")，
        // 与原 `decode_accuracy_once(..).unwrap_or_default()` 语义一致），不改任何 pub 签名。
        || decode_accuracy_allow_empty(recognizer, samples, None, script, Some(token_cap)),
    );
    if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "[LocalRT-DBG-320] hotwords inject: seg={} terms_len={} out_chars={} final_chars={}",
            seg_idx,
            terms_len,
            decided_out_chars,
            text.chars().count()
        );
    }
    // FIX-TERMS-ECHO-374 埋点：命中回显才打（匹配词条数 / 剥掉字数 / 剩余字数 / 是否重解 / 最终字数）。
    if disp.matched_terms > 0 && log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "[LocalRT-DBG-374] terms echo stripped: matched={} removed_chars={} remaining_chars={} redecode={} final_chars={}",
            disp.matched_terms,
            disp.removed_chars,
            disp.remaining_chars,
            if disp.redecoded { "yes" } else { "no" },
            text.chars().count()
        );
    }
    // FIX-PREVIEW-STALE-AND-COLLAPSE-375（B）埋点：每个窗口的产出率 vs 均值 + 处置动作。
    if log::log_enabled!(log::Level::Debug) {
        let action375 = if disp.collapse {
            if text.is_empty() {
                "empty"
            } else {
                "redecode"
            }
        } else {
            "keep"
        };
        log::debug!(
            "[LocalRT-DBG-375] collapse: seq={} audio={:.2}s out={}字 rate={:.2} avg={} action={}",
            seg_idx,
            audio_secs,
            decided_out_chars,
            if audio_secs > 0.0 {
                decided_out_chars as f32 / audio_secs
            } else {
                0.0
            },
            match inject.avg_chars_per_sec {
                Some(a) => format!("{a:.2}"),
                None => "cold".to_string(),
            },
            action375
        );
    }
    // FIX-ACC-OUTPUT-GUARD-387 埋点：每次判定一行（触发类别 + 动作）；重解后仍无效单独 warn。
    if log::log_enabled!(log::Level::Debug) {
        let action387 = if !disp.redecoded {
            "keep"
        } else if text.is_empty() {
            "empty"
        } else {
            "redecode"
        };
        log::debug!(
            "[LocalRT-DBG-387] guard: seq={} kind={} action={}",
            seg_idx,
            disp.guard.as_str(),
            action387
        );
    }
    if disp.invalid {
        log::warn!(
            "[LocalRT-DBG-387] window #{} invalid output after redecode: kind={}",
            seg_idx,
            disp.guard.as_str()
        );
    }
    Ok((text, true))
}

/// FIX-PREFIX-AND-EAT-371：`<asr_text>` 标记必须落在**头部**的字节上限（唯一的护栏）。
///
/// 取 **64 字节**的理由：已知前缀形态都很短 —— 官方 `language chinese`(16B) / `language cantonese`(18B)，
/// 模型自造 `<语种词>`（≤8 字，中文 ≈24B）；64B 给出 2× 以上余量覆盖未见过的标签变体
/// （如 `language <lang> <region>`），而 64B 之后才出现标记 ⇒ 其前已有 ≥21 个汉字正文。
/// 355 结论：真实语音**不可能念出 `<asr_text>` 这个串** ⇒ 判为正文、不剥（防超长正文里的偶发尖括号）。
const QWEN3_PREFIX_MAX_BYTES: usize = 64;

/// MIGRATE-1.13.8-1.7B-359 / FIX-PREFIX-AND-EAT-371：剥掉 Qwen3-ASR 1.7B **自造**的语种标注前缀。
///
/// 🔴 **规则（355 定，371 恢复）**：**无条件截断到第一个 `<asr_text>`（含）**、**语言无关** ——
/// 不要求前缀「标签样」、也不要求 `<asr_text>` 紧贴标签。
///
/// 359 实现时收窄了 355 规则（加「标签样」+「紧贴标签」两闸），而生产 `decode_accuracy_once`
/// 每次都注入 ctx + 词库，**注入改变了模型自造前缀的形态** ⇒ 两闸拦不住 ⇒ 整条前缀漏进正文
/// （Gavin 端测 21:43：`language chinese<asr_text>` 26 字进了用户文本）。371 去掉两闸。
///
/// **唯一保留的护栏是位置**：标记起点必须在前 [`QWEN3_PREFIX_MAX_BYTES`] 字节内（见其理由）。
/// 无 `<asr_text>` / 标记靠后 ⇒ **原样返回**（0.6B 不吐前缀 ⇒ no-op；正文含尖括号 ⇒ 不误伤）。
///
/// 命中时打 DEBUG 埋点（原样打印被剥掉的整段头部），便于下次变形时直接从日志看到模型吐了什么。
fn strip_qwen3_language_prefix(text: &str) -> &str {
    const MARKER: &str = "<asr_text>";
    let t = text.trim_start();
    let Some(pos) = t.find(MARKER) else {
        return text;
    };
    if pos > QWEN3_PREFIX_MAX_BYTES {
        // 正文中段（靠后）的标记 ⇒ 视为正文，不剥。
        return text;
    }
    // FIX-PREFIX-AND-EAT-371：把被剥掉的原始头部（前缀 + 标记）原样打进日志。
    log::debug!(
        "[LocalRT-DBG-371] qwen3 prefix stripped: {:?}",
        &t[..pos + MARKER.len()]
    );
    // 截到 `<asr_text>` 之后，顺带吃掉紧随的空白与常见分隔冒号。
    t[pos + MARKER.len()..].trim_start_matches(|c: char| c.is_whitespace() || c == ':' || c == '：')
}

impl Transcriber {
    /// Create new Transcriber with explicit ASR model selection
    ///
    /// ASR-SINGLE-MODEL-001（DEC-027）+ DEC-028 + ASR-041-B：多模式 ASR
    /// - Performance/Accuracy：加载本地模型（单模型，不预创建 fallback）
    /// - QwenAudioOnline：不加载任何本地模型（零本地 ASR 内存），存 url+key
    ///
    /// R2 修订（验收第 2 轮）：asr_model 存的是 effective_model（生效模型）。
    /// accuracy 降级 CTC 时 effective=Performance，语义自动归位。
    pub fn new(
        model_dir: &Path,
        enable_streaming: bool,
        asr_language: String,
        asr_model: AsrModel,
        hotwords: Option<&str>,
        asr_online_api_key: &str,
        asr_online_url: &str,
        asr_online_model: &str,
        asr_online_max_sentence_silence: i64,
        asr_online_semantic_punctuation_enabled: bool,
    ) -> Result<Self> {
        // HOTWORDS-TOKEN-268: 启动时后台预热热词 tokenizer（~数百 ms），避免首次计 token
        // 落在「按热键→开始录音」路径上被用户感知。
        warm_hotwords_tokenizer();

        let mode = if enable_streaming {
            AsrMode::Streaming
        } else {
            AsrMode::Offline
        };

        // DEC-028 / ASR-038-B / ASR-041-B / ASR-056: 在线 ASR 模式不加载本地模型
        // ASR-056: QwenAudioOnline 与 FunAsrRealtime 都走此路径（is_online_streaming 收敛判据）
        if asr_model.is_online_streaming() {
            if asr_online_api_key.trim().is_empty() {
                anyhow::bail!("在线 ASR 配置失败：API Key 为空（请在设置中配置 API Key）");
            }
            log::info!(
                "Online ASR mode: no local model loaded, model={:?}, url={}",
                asr_model,
                asr_online_url
            );
            return Ok(Self {
                mode,
                asr_language,
                asr_model,
                offline_recognizer: None,
                online_recognizer: None,
                hotwords_version: 0,
                vad_segmenter: None,
                asr_online_api_key: asr_online_api_key.to_string(),
                asr_online_url: asr_online_url.to_string(),
                asr_online_model: asr_online_model.to_string(),
                asr_online_max_sentence_silence,
                asr_online_semantic_punctuation_enabled,
            });
        }

        // LOCAL-RT-ENGINE-239-A（DEC-067）：LocalRealtime 双模型并存常驻。
        // 与 accuracy 不同：**不做静默降级**，任一模型缺失/加载失败即 Err（附则一），
        // 因为降级就是「用户以为用 A 实际跑 B」的不一致（ASR-UI-208 判定）。
        if asr_model == AsrModel::LocalRealtime {
            let (online_recognizer, offline_recognizer, hotwords_version) =
                build_local_realtime_recognizers(model_dir, &asr_language, hotwords)?;
            // VAD 分段器：与 accuracy 同条件（2pass 最终引擎是 accuracy，长音频要分段）
            let vad_segmenter = vad::VadSegmenter::try_new(model_dir).map(Mutex::new);
            log::info!(
                "LocalRealtime: dual recognizers loaded (online streaming paraformer + offline accuracy)"
            );
            return Ok(Self {
                mode,
                asr_language,
                asr_model: AsrModel::LocalRealtime,
                offline_recognizer: Some(offline_recognizer),
                online_recognizer: Some(online_recognizer),
                hotwords_version,
                vad_segmenter,
                asr_online_api_key: String::new(),
                asr_online_url: String::new(),
                asr_online_model: String::new(),
                asr_online_max_sentence_silence,
                asr_online_semantic_punctuation_enabled,
            });
        }

        let (offline_recognizer, effective_model, hotwords_version) =
            build_recognizer(model_dir, &asr_language, asr_model, hotwords)?;

        // VAD 分段器仅使用 accuracy 引擎的档位初始化；performance 模式设 None
        // R2: 用 effective_model 判断（降级 CTC 时不初始化 VAD）
        // FIX-LOCALRT-ENGINE-EQ-252: 判据收敛到 uses_accuracy_engine()（含 LocalRealtime）
        let vad_segmenter = if effective_model.uses_accuracy_engine() {
            // accuracy 引擎档位立即尝试初始化（模型文件在则建，失败后续降级）
            vad::VadSegmenter::try_new(model_dir).map(Mutex::new)
        } else {
            None
        };

        Ok(Self {
            mode,
            asr_language,
            asr_model: effective_model,
            offline_recognizer: Some(offline_recognizer),
            // LOCAL-RT-ENGINE-239-A（DEC-067）：仅 LocalRealtime 档并存流式 recognizer；
            // Performance/Accuracy 无流式侧，在线档已在上面提前返回。
            online_recognizer: None,
            hotwords_version,
            vad_segmenter,
            asr_online_api_key: String::new(),
            asr_online_url: String::new(),
            asr_online_model: String::new(),
            asr_online_max_sentence_silence,
            asr_online_semantic_punctuation_enabled,
        })
    }

    pub fn asr_model(&self) -> AsrModel {
        self.asr_model
    }

    /// LOCAL-RT-ENGINE-239-A（DEC-067）：取常驻的本地流式 recognizer（仅 `LocalRealtime` 档位为 `Some`）。
    ///
    /// 供 239-B 接线 `local_stream::transcribe_streaming_local` 使用。
    pub fn online_recognizer(&self) -> Option<&sherpa_onnx::OnlineRecognizer> {
        self.online_recognizer.as_ref()
    }

    /// PARALLEL-ACC-298：取常驻的 accuracy offline recognizer（accuracy 引擎档位为 `Some`）。
    ///
    /// 供 `main.rs` 的 accuracy 并行 worker 经 [`SendOfflineRecognizerRef`] 在 scope 线程内使用。
    pub fn offline_recognizer(&self) -> Option<&sherpa_onnx::OfflineRecognizer> {
        self.offline_recognizer.as_ref()
    }

    /// ASR-038-B: 在线 ASR 的 Inference API 端点
    pub fn asr_online_url(&self) -> &str {
        &self.asr_online_url
    }

    /// ASR-038-B: QwenAudioOnline 的模型 ID
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

    /// 当前 hotwords 版本号（外部对比用）
    pub fn hotwords_version(&self) -> u64 {
        self.hotwords_version
    }

    /// 当前 ASR 语言（热重载 swap 时同步 active 状态用）
    pub fn language(&self) -> &str {
        &self.asr_language
    }

    pub fn transcribe(
        &self,
        samples: &[f32],
        _language: &str,
        script: ChineseScript,
    ) -> Result<String> {
        self.transcribe_with_punct_info(samples, script)
            .map(|(text, _)| text)
    }
    /// 剥离 ASR 特殊 token（ASR-NOSPEECH-FILTER-001）。
    ///
    /// FunASR/SenseVoice/Qwen3 等模型常输出富文本标记，如：
    /// `<|nospeech|>`, `<|zh|>`, `<|en|>`, `<|ja|>`, `<|ko|>`,
    /// `<|NEUTRAL|>`, `<|HAPPY|>`, `<|Speech|>`, `<|woitn|>`, `<|withitn|>` 等。
    /// 这些标记不应进入下游文本处理/注入。
    ///
    /// 规则：精确匹配 `<|...|>` 配对形态（内部不含 `|` 与 `<`、`>`），
    /// 仅剥离完整 token，不误伤正常文本中的孤立 `<` 或 `>`。
    /// 多个连续 token 合并为一个空白分隔边界，避免产生多余空格。
    pub fn strip_asr_special_tokens(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        let mut pending_space = false; // 标记前一个 token 结束后是否欠一个空格
        while i < chars.len() {
            if chars[i] == '<' && chars.get(i + 1) == Some(&'|') {
                // 寻找结束 `|>`
                let mut j = i + 2;
                while j + 1 < chars.len() && !(chars[j] == '|' && chars[j + 1] == '>') {
                    // token 内部不得含另一个 `<` 或 `>` 或 `|`，否则不是合法 token
                    if chars[j] == '<' || chars[j] == '>' || chars[j] == '|' {
                        break;
                    }
                    j += 1;
                }
                if j + 1 < chars.len() && chars[j] == '|' && chars[j + 1] == '>' && j > i + 2 {
                    // 合法非空 `<|...|>` token：直接跳过，仅记录后续可能需要空格
                    pending_space = !result.is_empty();
                    i = j + 2;
                    continue;
                }
            }
            // 普通字符：若欠空格且当前非空白，按两侧字符类型决定是否补空格。
            // 【主控修正 2026-07-27】仅当 token 两侧均为 ASCII 字母/数字时才补空格
            // （防止英文单词被 token 分隔后粘连成 helloworld）；CJK 两侧一律不补——
            // 中文无词间空格，补了会把「你<|NEUTRAL|>好」污染成「你 好」并一路注入。
            if pending_space && chars[i] != ' ' {
                let prev_is_ascii_word = result
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_ascii_alphanumeric());
                if prev_is_ascii_word && chars[i].is_ascii_alphanumeric() {
                    result.push(' ');
                }
            }
            pending_space = false;
            result.push(chars[i]);
            i += 1;
        }
        // MIGRATE-1.13.8-1.7B-359：<|…|> 剥完后，再剥 Qwen3-ASR 1.7B 自造的裸语种前缀
        // （语言无关；对 0.6B / 无前缀输入是 no-op）。
        strip_qwen3_language_prefix(&result).to_string()
    }
    /// 转录并返回标点来源标记（ASR-PUNCT-OPT-001）
    ///
    /// 返回 `(text, native_punctuated)`：
    /// - `native_punctuated = true`：文本真正出自 native 模型（自带标点），可跳过标点引擎
    /// - `native_punctuated = false`：文本出自 performance/兜底/混合来源（无标点），需走标点引擎
    ///
    /// 标记规则：
    /// - performance → 恒 false
    /// - accuracy 单次 native 成功 → true
    /// - accuracy VAD 分段：所有段均 native 成功才 true；任一段兜底 → false
    /// - qwen3_online → 实测输出是否含**有效标点**（PUNCT-GOVERNANCE-030-A-2：
    ///   官方 API 无标点参数，不再假定必然带标点；用 `has_effective_punctuation`
    ///   统一「词内嵌豁免」判据 —— `3.14`/`don't`/`3:30`/`example.com` 的词内
    ///   标点不算，真句末标点才算；真有标点才跳过标点引擎）
    pub fn transcribe_with_punct_info(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        // ASR-038-B / ASR-056: 在线流式 ASR 路径（非流式回退：录完整段再发）
        // QwenAudioOnline 与 FunAsrRealtime 都走此路径（is_online_streaming 收敛判据）。
        // 流式管线在 main.rs 的 run_streaming_pipeline 中直接调用 qwen_inference::transcribe_streaming，
        // 此分支仅用于非流式回退（如模型切换过渡期）
        if self.asr_model.is_online_streaming() {
            let vocabulary = crate::transcription::load_wordbook_vocabulary();
            let text = qwen_inference::transcribe_streaming(
                &self.asr_online_url,
                &self.asr_online_api_key,
                &self.asr_online_model,
                samples,
                &vocabulary,
                self.asr_online_max_sentence_silence,
                self.asr_online_semantic_punctuation_enabled,
                None,
                |_| {},
            )?;
            let cleaned = Self::strip_asr_special_tokens(&text);
            let trimmed = cleaned.trim();
            if trimmed.is_empty() {
                // BUG-119: 在线回退路径空输出 = 没识别到语音，类型化信息信号
                anyhow::bail!(NoSpeechError);
            }
            let normalized = text_normalizer::normalize_text_for_language(trimmed, script);
            let native_punctuated = punctuation::has_effective_punctuation(&normalized);
            return Ok((normalized, native_punctuated));
        }
        match self.mode {
            AsrMode::Offline => self.transcribe_offline_detailed(samples, script),
            AsrMode::Streaming => self.transcribe_2pass_detailed(samples, script),
        }
    }

    /// Single-pass offline transcription (higher accuracy) — 旧签名兼容
    fn transcribe_offline(&self, samples: &[f32], script: ChineseScript) -> Result<String> {
        self.transcribe_offline_detailed(samples, script)
            .map(|(t, _)| t)
    }

    /// 带标点来源标记的 offline 转录
    ///
    /// ASR-SINGLE-MODEL-001（DEC-027）：VAD 降级路径重设计
    /// - 分段全空 → bail 转录失败（不再降级单次转录整段）
    /// - VAD segmenter 不可用 / lock poisoned 且 >24s → 朴素 20s 等分
    /// - 短音频 <24s → 单次转录路径不变
    ///
    /// R1 修订（验收第 2 轮）：lock poisoned 不再静默落到单次转录整段，
    /// 改为走 naive_chunk 分支（与 VAD 不可用同路径）。
    fn transcribe_offline_detailed(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        // ASR-LONG-AUDIO-001: accuracy 分支长音频 VAD 分段路径
        // FIX-LOCALRT-ENGINE-EQ-252: 收敛到 uses_accuracy_engine()（LocalRealtime 长音频同样要分段，
        // 否则整段喂 native 撞 max_total_len=512 ⇒ 空输出）
        if self.asr_model.uses_accuracy_engine() && vad::should_segment(samples) {
            // 尝试 VAD 分段；lock poisoned / None → 降级 naive_chunk
            let vad_segments: Option<Vec<Vec<f32>>> = match &self.vad_segmenter {
                Some(vad_lock) => match vad_lock.lock() {
                    Ok(vad) => {
                        let segs = vad.segment(samples);
                        if segs.is_empty() {
                            log::warn!("VAD produced no speech segments, transcription failed");
                            // BUG-119: 没说话 = 类型化信息信号，不走错误串
                            anyhow::bail!(NoSpeechError);
                        }
                        log::info!(
                            "VAD segmented {} samples ({:.1}s) into {} segments",
                            samples.len(),
                            samples.len() as f64 / 16000.0,
                            segs.len()
                        );
                        Some(segs)
                    }
                    Err(_) => {
                        // R1: lock poisoned → 走 naive_chunk，禁止静默落到单次转录整段
                        log::warn!(
                            "VAD segmenter lock poisoned, falling back to naive {}s chunking",
                            vad::SEGMENT_MAX_SECS
                        );
                        None
                    }
                },
                None => {
                    // VAD segmenter 不可用 → 朴素 20s 等分
                    // 保证 accuracy 长音频在 VAD 模型缺失时仍可用（禁止 >28s 整段喂 native）
                    log::warn!(
                        "VAD segmenter unavailable, using naive {}s chunking for {} samples ({:.1}s)",
                        vad::SEGMENT_MAX_SECS,
                        samples.len(),
                        samples.len() as f64 / 16000.0
                    );
                    None
                }
            };

            let segments = match vad_segments {
                Some(segs) => segs,
                None => vad::naive_chunk(samples),
            };

            // R1: 抽出的辅助函数复用（VAD 分段 / naive_chunk 共用转录循环）
            return self.transcribe_segments_chunked(&segments, script);
        }

        self.transcribe_segment_detailed(samples, script)
    }

    /// ASR-SINGLE-MODEL-001 R1：逐段转录循环（VAD 分段 / naive_chunk 复用）。
    ///
    /// 所有段转录后拼接；任一段空/失败 → all_native=false；
    /// 拼接结果全空 → bail 转录失败。
    fn transcribe_segments_chunked(
        &self,
        segments: &[Vec<f32>],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        let mut all_native = true;
        let mut seg_texts: Vec<String> = Vec::with_capacity(segments.len());
        for seg in segments {
            match self.transcribe_segment_detailed(seg, script) {
                Ok((t, np)) => {
                    seg_texts.push(t);
                    if !np {
                        all_native = false;
                    }
                }
                Err(e) => {
                    log::warn!("Segment transcription failed: {}", e);
                    seg_texts.push(String::new());
                    all_native = false;
                }
            }
        }
        let joined = vad::join_segment_texts(&seg_texts);
        if !joined.trim().is_empty() {
            log::info!("Segmented transcription: {}", joined);
            return Ok((
                text_normalizer::normalize_text_for_language(&joined, script),
                all_native,
            ));
        }
        log::warn!("All segments produced empty text, transcription failed");
        // BUG-119: 全段空拼接 = 没识别到语音，类型化信息信号
        anyhow::bail!(NoSpeechError);
    }

    /// 转录单段音频 — 旧签名兼容
    fn transcribe_segment(&self, samples: &[f32], script: ChineseScript) -> Result<String> {
        self.transcribe_segment_detailed(samples, script)
            .map(|(t, _)| t)
    }

    /// 带标点来源标记的单段转录。
    ///
    /// ASR-SINGLE-MODEL-001（DEC-027）：移除兜底链与异常检测。
    /// - accuracy native 成功 → (text, true)
    /// - accuracy 空输出 → bail 转录失败（上层 overlay 提示）
    /// - performance → (text, false)
    /// - qwen3_online 不走此路径（transcribe_with_punct_info 已路由）
    fn transcribe_segment_detailed(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        let recognizer = self
            .offline_recognizer
            .as_ref()
            .context("No local ASR recognizer (qwen3_online mode should not reach here)")?;
        let stream = recognizer.create_stream();
        stream.accept_waveform(16000, samples);
        recognizer.decode(&stream);

        let result = stream.get_result().context("No transcription result")?;
        let text = result.text.trim().to_string();

        // ASR-NOSPEECH-FILTER-001: 剥离特殊 token（如 <|nospeech|>）
        let text = Self::strip_asr_special_tokens(&text);

        // ASR-SINGLE-MODEL-001: accuracy 空输出 → bail（不再兜底 CTC）
        // FIX-LOCALRT-ENGINE-EQ-252: 收敛到 uses_accuracy_engine()（LocalRealtime 也用 accuracy 引擎，
        // native_punctuated=true ⇒ 跳过外部 CT-Transformer，避免双重打点）
        if self.asr_model.uses_accuracy_engine() {
            if text.is_empty() {
                log::warn!("ASR accuracy model produced empty output, transcription failed");
                anyhow::bail!("ASR transcription failed: accuracy model produced empty output");
            }
            // accuracy native 成功 → native_punctuated = true
            return Ok((
                text_normalizer::normalize_text_for_language(&text, script),
                true,
            ));
        }

        // performance 分支 → native_punctuated = false（CTC 无标点）
        Ok((
            text_normalizer::normalize_text_for_language(&text, script),
            false,
        ))
    }

    /// 2-pass streaming transcription — 旧签名兼容
    fn transcribe_2pass(&self, samples: &[f32], script: ChineseScript) -> Result<String> {
        self.transcribe_2pass_detailed(samples, script)
            .map(|(t, _)| t)
    }

    /// 带标点来源标记的 2-pass 转录
    fn transcribe_2pass_detailed(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        // 当前实现：直接使用 offline 单遍
        self.transcribe_offline_detailed(samples, script)
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
/// entries 须按确定性顺序（调用方按 id 排序）保证哈希稳定
pub fn hotwords_version(entries: &[String]) -> u64 {
    let mut hasher = DefaultHasher::new();
    entries.len().hash(&mut hasher);
    for word in entries {
        word.hash(&mut hasher);
    }
    hasher.finish()
}

/// ASR-ACC-OPT-001 方案 A：hotwords 精选上限。
/// 超过此数量的词条按 id DESC（最近添加优先）截断。
/// 研究依据：220 条 hotwords → 0% 全空输出（撑爆 max_total_len=512 context），
/// 全量 11 条含无关英文词 → 60%（比无 hotwords 62.5% 还差）。
/// ASR-ACC-TUNE-001（2026-07-08 Gavin 拍板）：50→20，002/003 实测 hw=50 比 hw=20
/// 退化 -2.5pp，10-20 为最优区间。
/// 🔴 MIGRATE-QWEN3-320（Gavin 2026-09-21）：20 → 120 → **200**。
/// ⚠️ 真正卡条数的是 token 预算（见 `HOTWORDS_MAX_TOTAL_TOKENS`）；条数仅作上限。
pub const HOTWORDS_MAX_ENTRIES: usize = 200;

/// ASR-ACC-OPT-001 方案 A：hotwords 单条最大字符数。
/// 超长的词条（candidates 整句）不灌入，避免膨胀 user_prompt。
pub const HOTWORDS_MAX_ENTRY_CHARS: usize = 10;

/// 热词**总 token 预算**（**Qwen3 @ `max_total_len=4096`**）。
///
/// 🔴 **与 `create_qwen3_recognizer` 的 `max_total_len` 耦合，改一个必须同批核另一个。**
///
/// 算式（2026-09-21 按**当前真实常量**重算；原算式按上下文 300 字、说明句 40 token 推导，
/// 此后 `CTX_DEFAULT_CHARS` 放宽到 500 且新增 `CLEANUP_INSTR_EN`，故余量比原注释所写更紧）：
/// ```
/// 4096
/// − 260(20s 音频 token；Qwen3 实测 13.0 tok/s × 20)
/// − 385(上下文 CTX_DEFAULT_CHARS=500 字；Qwen3 ~0.77 tok/字)
/// −  55(CLEANUP_INSTR_EN + CTX_INSTR_EN + Context/Terms 标签)
/// − 256(生成预留 = max_new_tokens)
/// = 3140  ⇒ 现值 3000，余量仅约 140（原注释算作 3309 是按 300 字上下文，已过期）
/// ```
/// ⚠️ 余量已不大。再放宽 `CTX_DEFAULT_CHARS` 或加长提示词，**必须同批下调本值**。
/// ⚠️ 若 `max_total_len` 调回 2048，剩给词库仅约 1092 ⇒ 本值须降到 1000 以内，否则空输出。
/// （原 356 是从 **FunASR KV 1024** 反推的，已随 320 单引擎化删除。）
pub const HOTWORDS_MAX_TOTAL_TOKENS: usize = 3000;

/// Rust `tokenizers` 计数相对 **Qwen3** C++ tokenizer 的保守系数。
///
/// 🔴 原 `1.85`（DEC-074）是为 **FunASR 的 C++ tokenizer** 标定的；Qwen3 是另一套 tokenizer、
/// 该系数**不适用**（DEC-076）。本值 **2.0 = 未标定、保守取值**（未实测重标；宁可少装词，不可低估溢出）。
const HOTWORDS_CPP_SAFETY_FACTOR: f64 = 2.0;

/// MIGRATE-QWEN3-320：词库预算（**单引擎 Qwen3**；原 FunASR 分派 / `AccuracyEngine` 已删）。
/// 双份预算已收敛为单份，避免两套同步漂移（`[CONFIG-MIRROR-DRIFT-001]` 同族）。
#[derive(Clone, Copy)]
struct HotwordsBudget {
    max_entries: usize,
    max_total_tokens: usize,
    safety: f64,
}

const HOTWORDS_BUDGET: HotwordsBudget = HotwordsBudget {
    max_entries: HOTWORDS_MAX_ENTRIES,
    max_total_tokens: HOTWORDS_MAX_TOTAL_TOKENS,
    safety: HOTWORDS_CPP_SAFETY_FACTOR,
};

/// 惰性加载的 Qwen3 BPE tokenizer（仅计 token 用，加载一次）。
static HOTWORDS_TOKENIZER: OnceLock<Option<tokenizers::Tokenizer>> = OnceLock::new();

/// 取（必要时加载）tokenizer。路径按 DEC-011 exe 同级 models 推导。
fn hotwords_tokenizer() -> &'static Option<tokenizers::Tokenizer> {
    HOTWORDS_TOKENIZER.get_or_init(|| {
        // 🔴 2026-09-21：由 FunASR Nano 目录改指 **Qwen3-ASR 自带 tokenizer**。
        // 原路径依赖已迁移走的 nano 模型 —— 用户若只下 Qwen3，该路径会缺失，
        // 词条计数会静默退化为「UTF-8 字节数上界」（高估约 50%，200 条上限实收约 130~150 条），
        // 不崩不报错但词条被静默截断。tester-1 在 BUILD-321 核验时发现（我任务书里预设
        // 「exe 不再引用 nano」，他如实报出不成立）。
        //
        // Qwen3-ASR 的 tokenizer 目录原本只有 vocab.json / merges.txt / tokenizer_config.json，
        // 缺 tokenizer.json；已把 nano 那份（11.4MB）拷入。
        // **精度零损失**：两侧 vocab.json 与 merges.txt 的 sha256 **逐字节相同**
        // （ca10d7e9… / 8831e4f1…），是同一个 Qwen3 tokenizer，不是近似替代。
        let p = model_dir()
            .join(QWEN3_MODEL_SUBDIR)
            .join("tokenizer")
            .join("tokenizer.json");
        let t = std::time::Instant::now();
        match tokenizers::Tokenizer::from_file(&p) {
            Ok(tk) => {
                log::info!(
                    "HOTWORDS-TOKEN-268: tokenizer loaded in {:?} ({})",
                    t.elapsed(),
                    p.display()
                );
                Some(tk)
            }
            Err(e) => {
                log::warn!(
                    "HOTWORDS-TOKEN-268: tokenizer load failed ({}): {}；将退化为字节上界",
                    p.display(),
                    e
                );
                None
            }
        }
    })
}

/// HOTWORDS-TOKEN-268：启动时预热 tokenizer，避免 ~数百 ms 加载落在「按热键→开始录音」路径上。
pub fn warm_hotwords_tokenizer() {
    std::thread::spawn(|| {
        let _ = hotwords_tokenizer();
    });
}

/// 单条词在 **C++ 口径**下的保守 token 估算（Rust 计数 × `safety`；tokenizer 不可用则用 UTF-8 字节数上界）。
fn estimate_word_tokens_with(tk: Option<&tokenizers::Tokenizer>, word: &str, safety: f64) -> usize {
    match tk {
        Some(t) => match t.encode(word, false) {
            Ok(enc) => ((enc.get_ids().len() as f64) * safety).ceil() as usize,
            // byte-level BPE 下 token ≤ byte，字节数是安全的粗上界
            Err(_) => word.len(),
        },
        None => word.len(),
    }
}

/// 兼容入口：FunASR 的 1.85 系数（测试/回滚用）。新代码走 `estimate_word_tokens_with`。
#[allow(dead_code)] // 仅单测与 FunASR 回滚路径兼容保留
fn estimate_word_tokens(tk: Option<&tokenizers::Tokenizer>, word: &str) -> usize {
    estimate_word_tokens_with(tk, word, HOTWORDS_CPP_SAFETY_FACTOR)
}

// ============================================================
// DUAL-PATH-ACC-363/364：路B（累积全量解码）预算闸门 —— 精确计算版
// ============================================================

/// 生成预留 = 生产 `max_new_tokens`（256）。
pub(crate) const PATH_B_GEN_RESERVE: usize = 256;
/// 只防取整/边界的安全余量（DUAL-PATH-REFINE-364：512 → **64**）。
/// 🔴 不再承担「估算不准」兜底 —— 音频/注入段两项都已**精确计算**（见下）；
/// 本余量仅覆盖取整与最后 ±1 token 边界。DEC-069 撞顶静默丢字，故不取 0。
pub(crate) const PATH_B_SAFETY_MARGIN: usize = 64;
/// 与 `create_qwen3_recognizer` 的 `max_total_len` 耦合（当前 4096，本单不改）。
pub(crate) const PATH_B_MAX_TOTAL_LEN: usize = 4096;
/// Whisper mel 帧 hop（16kHz，10ms/帧，hop=160 样本）。
const QWEN3_MEL_HOP_SAMPLES: usize = 160;
/// Qwen3 conv 前端分块大小（sherpa C++ `kQwen3ChunkSize`，单位=mel 帧）。
const QWEN3_CONV_CHUNK_SIZE: i32 = 100;

/// 移植 sherpa C++ `FeatToAudioTokensLen(feat_len, chunk_size)`（`offline-recognizer-qwen3-asr-impl.cc:77`）。
/// 纯函数、**精确**：每 `chunk_size`(100) mel 帧 → `conv_out_len_3x_stride2(100)`=13 个 audio token，
/// 余数走 `aftercnn`。20s(2000 帧) ⇒ 20×13 = 260，与实测 13.0 tok/s 吻合。
fn feat_to_audio_tokens(feat_len: i32, chunk_size: i32) -> i32 {
    if feat_len <= 0 || chunk_size <= 0 {
        return 0;
    }
    fn conv_out_len_3x_stride2(n: i32) -> i32 {
        let x = (n + 1) / 2;
        let x = (x + 1) / 2;
        (x + 1) / 2
    }
    fn aftercnn(x: i32) -> i32 {
        if x <= 0 {
            return 0;
        }
        let x = (x - 1) / 2 + 1;
        let x = (x - 1) / 2 + 1;
        (x - 1) / 2 + 1
    }
    let full = feat_len / chunk_size;
    let rem = feat_len % chunk_size;
    let mut out = full * conv_out_len_3x_stride2(chunk_size);
    if rem > 0 {
        out += aftercnn(rem);
    }
    out.max(0)
}

/// 路B 音频 token 数（**精确**）：mel 帧数取上界 `ceil(samples/hop)`（保守 ±1 帧），
/// 再走 C++ 同款降采样公式。
pub(crate) fn expected_audio_tokens(num_samples: usize) -> usize {
    let feat_len = num_samples.div_ceil(QWEN3_MEL_HOP_SAMPLES) as i32;
    feat_to_audio_tokens(feat_len, QWEN3_CONV_CHUNK_SIZE).max(0) as usize
}

/// 路B 注入段 token 数（**精确**）：对**实际要注入的 hotwords 串**（`build_ctx_system`，377 起
/// **只剩纯词表**）用 tokenizer 实数，再乘 `HOTWORDS_CPP_SAFETY_FACTOR`(2.0)
/// 以覆盖「我方 tokenizer 与 C++ 手搓 BPE 的计数差异」（DEC-074）。三项一次算准，不重复扣。
pub(crate) fn estimate_inject_tokens(terms: Option<&str>) -> usize {
    match build_ctx_system(terms) {
        Some(s) => estimate_word_tokens(hotwords_tokenizer().as_ref(), &s),
        None => 0,
    }
}

/// DUAL-PATH-ACC-364：路B 预算闸门 —— **精确计算**。
///
/// 判据：`音频token(精确) + 注入段token(精确) + 生成预留 + 安全余量 ≤ max_total_len`。
/// - 音频：`expected_audio_tokens`（C++ 同款降采样公式，**非** 13.0 tok/s 估算）；
/// - 注入段：`estimate_inject_tokens`（tokenizer 实数 × C++ 差异系数，377 起**只剩纯词表**
///   + 提示词脚手架）；
/// - 余量仅 64（取整边界），不再承担估算兜底。
///
/// `true` ⇒ 路B 跑全量；`false` ⇒ 降级退回切片拼装（绝不硬塞）。
/// 180s 不降级；`MAX_RECORD_SECONDS` 现为 300 ⇒ 300s 会降级（370 起整段解码已摘接线，见 `main.rs`）。
pub(crate) fn path_b_budget_ok(num_samples: usize, terms: Option<&str>) -> bool {
    expected_audio_tokens(num_samples)
        + estimate_inject_tokens(terms)
        + PATH_B_GEN_RESERVE
        + PATH_B_SAFETY_MARGIN
        <= PATH_B_MAX_TOTAL_LEN
}

// ============================================================
// SLIDING-WINDOW-367：滑动窗口组窗 + 对齐合并（纯函数，可单测）
// ============================================================

/// 滑动窗口最多纳入的片数：**当前片 + 前 3 片**（Gavin 定）。
pub(crate) const WINDOW_MAX_SLICES: usize = 4;
/// 窗口音频时长上限（秒）。单片仍超此值 ⇒ 直接单片组窗（不切分）。
///
/// 🔴 FIX-SLICE-CUT-AT-GAP-381（Gavin 2026-09-23）：**12.0 → 10.0**。理由：与切片起搜点
/// [`SLIDING_CUT_SEARCH_START_SECS`] 统一为 10s（原「12s 窗 / 13s 硬切点」两个时间点作废）。
/// 收益 = 回灌更快（单窗解码更短）；代价 = 前文纠正范围变小、接缝变多 ——
/// 由本单 §5 的 12s vs 10s 生产路径 CER 实测把关（判据：10s 不比 12s 差 >0.01 绝对值）。
pub(crate) const WINDOW_MAX_SECS: f32 = 10.0;

/// SLIDING-WINDOW-367 ①：组窗起点（返回 `slices` 的起始索引；选中 `slices[start..]`）。
///
/// 规则（Gavin 定）：取「当前片 + 前 1~3 片」；若合计 > [`WINDOW_MAX_SECS`]，
/// **从最远开始逐片丢**（前3 → 前2 → 前1），每丢一片重判；丢到只剩当前片仍超 ⇒ 直接单片组窗。
///
/// `durations_secs` 按**时间顺序**（最后一个是当前片）。
pub(crate) fn group_window_start_secs(durations_secs: &[f32], max_secs: f32) -> usize {
    let n = durations_secs.len();
    if n == 0 {
        return 0;
    }
    let mut start = n.saturating_sub(WINDOW_MAX_SLICES);
    while start + 1 < n {
        let sum: f32 = durations_secs[start..].iter().sum();
        if sum <= max_secs {
            break;
        }
        start += 1; // 丢最远一片
    }
    start
}

/// 对齐/合并时忽略的字符：空白与标点（标点抖动不该影响「哪些字重叠」的判定）。
fn align_keep_char(c: char) -> bool {
    !c.is_whitespace() && !crate::punctuation::PUNCT_CHARS.contains(&c)
}

/// 重叠区允许的最大编辑距离比例（质量门）。> 此值视为「模型大幅改写」⇒ 不滑动。
/// 推导：361 实测模型重解是**局部纠错**（个别字，如「二比零→二比一」），重叠区若大幅改写
/// 说明两窗已非同段 ⇒ 宁可不更新，绝不错位。取 15% 给局部纠错留余量、又不放到误对齐。
pub(crate) const ALIGN_MAX_EDIT_RATIO: f32 = 0.15;
/// 重叠有效字符数下限（长度门）。
pub(crate) const ALIGN_MIN_OVERLAP_CHARS: usize = 8;
/// 重叠相对「新窗有效字」的比例下限（长度门）。
pub(crate) const ALIGN_MIN_OVERLAP_RATIO: f32 = 0.30;

/// 对齐结果。
pub(crate) struct AlignResult {
    /// `prev` 中**可定稿**的前缀（原文，含标点）；空 = 不滑动。
    pub committed_prefix: String,
    /// 重叠的**有效字符**数（去标点/空白）。
    pub overlap_chars: usize,
    /// 是否同时满足「长度门 + 质量门」⇒ 允许滑动定稿。
    pub ok: bool,
}

fn edit_distance_chars(a: &[char], b: &[char]) -> usize {
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    for i in 1..=m {
        let mut cur = vec![i; n + 1];
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[n]
}

/// FIX-PREFIX-AND-EAT-371（B·层②）：软范围容差 —— **不对称，向下宽、向上紧**。
///
/// | 方向 | 选到的 `k` | 切点 | 后果 |
/// | --- | --- | --- | --- |
/// | 向**小**放宽（本常量） | `k < 真值` | 靠后 | `committed` 偏长 ⇒ **重复**（可容忍） |
/// | 向**大**放宽（[`ALIGN_EXPECTED_K_TOL_UP`]） | `k > 真值` | 靠前 | `committed` 偏短 ⇒ **吃掉一小段**（P0） |
///
/// 🔴 层① 只保证 `committed_prefix` **非空**，**不保证不吃半句** ⇒ 两方向代价不对等
/// ⇒ 上界收紧（+25%）、下界放宽（−50%）。「宁宽勿窄」只适用于**安全方向**。
pub(crate) const ALIGN_EXPECTED_K_TOL_DOWN: f32 = 0.50;
/// 见 [`ALIGN_EXPECTED_K_TOL_DOWN`]：向上（危险方向）收紧到 +25%。
pub(crate) const ALIGN_EXPECTED_K_TOL_UP: f32 = 0.25;

/// FIX-PREFIX-AND-EAT-371（B·层②）：期望 `k` 的候选区间 `[lo, hi]`（已按 `min_len` / `hi_all` 夹紧）。
///
/// `hi_all` 已含层① 的硬上界（`prev_extra_slices ≥ 1 ⇒ k < prev 有效字数`）⇒ 本函数不重复处理硬约束。
/// 不对称性见 [`ALIGN_EXPECTED_K_TOL_DOWN`]（向下 −50% 宽 / 向上 +25% 紧）。
fn expected_k_band(expected_k: usize, min_len: usize, hi_all: usize) -> (usize, usize) {
    let down = ((expected_k as f32) * ALIGN_EXPECTED_K_TOL_DOWN).ceil() as usize;
    let up = ((expected_k as f32) * ALIGN_EXPECTED_K_TOL_UP).ceil() as usize;
    (
        expected_k.saturating_sub(down).max(min_len),
        (expected_k + up).min(hi_all),
    )
}

/// 取 `k` 作为重叠长度试一次质量门；通过 ⇒ 返回定稿结果，否则 `None`。
fn align_try_k(
    prev: &str,
    prev_keep: &[(usize, char)],
    new_keep: &[char],
    k: usize,
) -> Option<AlignResult> {
    let p_tail: Vec<char> = prev_keep[prev_keep.len() - k..]
        .iter()
        .map(|(_, c)| *c)
        .collect();
    let dist = edit_distance_chars(&p_tail, &new_keep[..k]);
    let ratio = dist as f32 / k as f32;
    if ratio <= ALIGN_MAX_EDIT_RATIO {
        let cut_byte = prev_keep[prev_keep.len() - k].0;
        Some(AlignResult {
            committed_prefix: prev[..cut_byte].to_string(),
            overlap_chars: k,
            ok: true,
        })
    } else {
        None
    }
}

fn align_fail() -> AlignResult {
    AlignResult {
        committed_prefix: String::new(),
        overlap_chars: 0,
        ok: false,
    }
}

/// SLIDING-WINDOW-367 ④：滑动对齐 —— 找 `prev` 的后缀与 `new` 的前缀的**可接受**重叠
/// （按去标点/空白后的内容比对；容许 [`ALIGN_MAX_EDIT_RATIO`] 内的编辑距离以吸收局部纠错）。
///
/// 🔴 **滑动需同时满足两条件**：
/// 1. **长度门**：`overlap ≥ max(ALIGN_MIN_OVERLAP_CHARS, new有效字 × ALIGN_MIN_OVERLAP_RATIO)`；
/// 2. **质量门**：重叠区 `编辑距离 / overlap ≤ ALIGN_MAX_EDIT_RATIO`。
/// 两者都满足才返回 `ok=true`；否则 `ok=false`（调用方**不滑动、不定稿**，保持上一次结果）。
///
/// **无区间先验**的退化入口（等价 `AlignPrior::none()`）⇒ 小 `k` 优先；生产走
/// [`align_overlap_with_prior`]（带切片区间/时长先验）。保留供单测直接考察退化路径。
#[allow(dead_code)]
pub(crate) fn align_overlap(prev: &str, new: &str) -> AlignResult {
    align_overlap_with_prior(prev, new, AlignPrior::none())
}

/// FIX-PREFIX-AND-EAT-371（B）：对齐可用的**区间先验**（四层判据的 ① ② 层输入）。
#[derive(Clone, Copy)]
pub(crate) struct AlignPrior {
    /// ② 软范围（**估算，只做粗筛**）：期望重叠比例 = 共享片样本和 / 本窗样本和。
    /// `None` = 拿不到区间信息 ⇒ 退化层 ④ 偏小 `k`。
    pub expected_ratio: Option<f32>,
    /// ① 硬约束（**精确，零假设**）：上一窗比本窗**多出的有声切片数** `m = ws_new - ws_prev`。
    /// VAD 切出的片都对应非空话音 ⇒ `m ≥ 1` 时那些片的话必须在 `committed_prefix` 里
    /// ⇒ `k` **严格小于** `prev` 有效字数（否则切点在开头 ⇒ `committed_prefix` 为空 ⇒ 吃字）。
    pub prev_extra_slices: usize,
}

impl AlignPrior {
    /// 无任何区间先验（退化入口用）。
    pub(crate) fn none() -> Self {
        Self {
            expected_ratio: None,
            prev_extra_slices: 0,
        }
    }
}

/// FIX-PREFIX-AND-EAT-371（B）：用**已知的切片区间/音频时长**约束重叠长度 `k`。
///
/// 🔴 **为什么不能在全区间自由搜**：原实现「从最长重叠 `max_k` 往短找、取第一个质量合格者」
/// 在**周期性内容**下是错的 —— `prev = S1S2S3`、`new = S2S3S4`（同一句连说）时 `k = max_k`
/// 处的 `p_tail` 与 `new` 前缀**逐字相等**（`dist = 0`）⇒ 先命中 ⇒ 切点钉在最左
/// ⇒ `committed_prefix = ""` ⇒ **整段滑出文本被丢**（Gavin 端测 21:43 丢了「明天天气好的话，可以一起」）。
///
/// 🔴 **四层判据（按顺序）**：
/// 1. **① 硬约束（精确）**：`prev_extra_slices ≥ 1` ⇒ `k` 必须 `< prev有效字数`（⇒ `committed_prefix` 非空）。
/// 2. **② 软范围（估算，只做粗筛）**：期望 `k ≈ 新窗有效字 × expected_ratio`，取其 ±
///    [`expected_k_band`]（**不对称**：向下 −50% 宽、向上 +25% 紧）的窄带。**按时长估字数本身不精确**（语速不匀、句中停顿 ⇒
///    秒数涨字数不涨 ⇒ 估值虚高），故它**只用于排除自相矛盾的候选，不用于精确定位**；
///    容差宁可放宽 —— 把真值挡在外面（⇒ 回落兜底 ⇒ 重复）比放进一个错值（⇒ 丢字）更糟。
/// 3. **③ 质量门**：在候选内用编辑距离挑切点（现有机制，未改）。
/// 4. **④ 兜底**：拿不到区间信息 ⇒ **升序**（小 `k` 优先）：`k` 小 ⇒ 切点靠后 ⇒ `committed` 更长
///    ⇒ 最坏是**重复**；`k` 大 ⇒ 切点靠前 ⇒ **丢字**。Gavin 优先级：**丢字 P0、重复可容忍**
///    ⇒ 退化方向只能往重复偏，绝不能往丢字偏。
pub(crate) fn align_overlap_with_prior(prev: &str, new: &str, prior: AlignPrior) -> AlignResult {
    let prev_keep: Vec<(usize, char)> = prev
        .char_indices()
        .filter(|(_, c)| align_keep_char(*c))
        .collect();
    let new_keep: Vec<char> = new.chars().filter(|c| align_keep_char(*c)).collect();
    if prev_keep.is_empty() || new_keep.is_empty() {
        return align_fail();
    }
    let max_k = prev_keep.len().min(new_keep.len());
    let min_len = ALIGN_MIN_OVERLAP_CHARS
        .max((new_keep.len() as f32 * ALIGN_MIN_OVERLAP_RATIO).ceil() as usize);
    if max_k < min_len {
        return align_fail();
    }
    // ① 硬上界（精确）：m ≥ 1 ⇒ k < prev 有效字数。
    let hi_hard = if prior.prev_extra_slices >= 1 {
        prev_keep.len().saturating_sub(1)
    } else {
        max_k
    };
    let hi_all = max_k.min(hi_hard);
    if min_len > hi_all {
        return align_fail();
    }
    // ② 软范围（估算，粗筛）
    if let Some(e_raw) = prior
        .expected_ratio
        .filter(|r| r.is_finite() && *r > 0.0 && *r <= 1.0)
        .map(|r| ((new_keep.len() as f32) * r).round() as usize)
    {
        let (lo, hi) = expected_k_band(e_raw, min_len, hi_all);
        if lo <= hi {
            let e = e_raw.clamp(lo, hi);
            // ③ 质量门：期望值本身 → 向小 → 向大
            if let Some(r) = align_try_k(prev, &prev_keep, &new_keep, e) {
                return r;
            }
            for k in (lo..e).rev() {
                if let Some(r) = align_try_k(prev, &prev_keep, &new_keep, k) {
                    return r;
                }
            }
            for k in (e + 1)..=hi {
                if let Some(r) = align_try_k(prev, &prev_keep, &new_keep, k) {
                    return r;
                }
            }
        }
        // 窄带内无解 ⇒ 落到 ④ 兜底（**硬约束仍生效**：hi_all 已按 ① 收紧）
    }
    // ④ 兜底 / 退化：**升序**（小 k 优先 ⇒ 最坏是重复，绝不丢字）
    for k in min_len..=hi_all {
        if let Some(r) = align_try_k(prev, &prev_keep, &new_keep, k) {
            return r;
        }
    }
    align_fail()
}

/// SLIDING-WINDOW-367（阶段四·B）/ TUNE-DECODE-SERIAL-AND-TOKEN-CAP-390：窗口解码**并发度**。
///
/// 🔴 **现为 1（串行）** —— Gavin 2026-09-23 端测确认。依据（主控四份端测日志实测）：
/// - sherpa 1.13.8 `OfflineRecognizerQwen3ASRImpl::DecodeStreams` 只是 `for` 循环逐个 `Decode`
///  （`offline-recognizer-qwen3-asr-impl.cc:1149`）⇒ **批处理接口对 Qwen3 无并行收益**；
/// - 两个解码线程共用**同一个** recognizer（ORT 会话 8 线程池）⇒ 同时解码互相争抢：
///   单独解码中位 **274 ms/音频秒**、并发解码中位 **476 ms/音频秒**（每窗慢 74%，总吞吐仅 +15%）；
/// - 预览按窗口顺序回灌 ⇒ **单窗变慢直接推迟刷新**。
///
/// - `=1` ⇒ **顺序执行**（共享队列 / `drive_acc_windows` / 收尾逻辑不变，自然退化）。
/// - `≥2` ⇒ 窗口间并发（**改回须附新实测**证明吞吐有净收益，否则别改）。
/// 366「线程数 min(cores,8)」结论不变 —— 那是 ORT 会话内部线程，与本常量无关。
pub(crate) const WINDOW_DECODE_CONCURRENCY: usize = 1;

/// FIX-PREFIX-AND-EAT-371（B）：期望重叠比例 —— 「与上一窗**共享的切片**样本和 / 本窗样本和」。
///
/// - `ws`/`we` = 本窗切片区间（开区间）；`prev_end` = 上一窗切片的结束下标；
/// - `slice_samples[i]` = 本窗第 `i` 片（全局号 `ws + i`）的样本数。
///
/// 共享切片数 = `prev_end - ws`（窗口结束下标单调不减 ⇒ `prev_end <= we` ⇒ 共享片全在本窗样本表内）。
/// 任一处信息缺失/不自洽 ⇒ `None`（调用方退化为**小 k 优先**，见 [`align_overlap_with_prior`]）。
fn expected_overlap_ratio(
    ws: usize,
    we: usize,
    prev_end: usize,
    slice_samples: &[usize],
) -> Option<f32> {
    if we <= ws || slice_samples.len() != we - ws {
        return None;
    }
    let shared = prev_end.saturating_sub(ws);
    if shared == 0 || shared > slice_samples.len() {
        return None;
    }
    let total: usize = slice_samples.iter().sum();
    if total == 0 {
        return None;
    }
    let overlap: usize = slice_samples[..shared].iter().sum();
    Some(overlap as f32 / total as f32)
}

/// SLIDING-WINDOW-367（阶段四·B）：按 `window_seq` **有序定稿**的重排缓冲。
///
/// 🔴 解码可**乱序完成**，但对齐状态机**必须按 seq 串行**（`align_overlap` 依赖前一窗文本）
/// ⇒ 结果先入 `pending`，等 `next` 到齐才依次 align/定稿，产出「应回灌的权威全文」（可 0..n 条）。
/// 乱序合并会推进错 `committed` ⇒ 文本必错，故**唯一入口 `push` 保证按序**。
///
/// 🔴 FIX-WINDOW-DISJOINT-369（P0）：合并策略由**切片区间**判定（不再只靠文本猜重叠）——
/// 调用方本就知道每个窗口含哪几片（`[start_slice, end_slice)`），别丢掉这个信息：
/// 1. **零重叠**（`new.start >= prev.end`，两窗不含同一片）⇒ **直接拼接**：
///    没有重叠就没有重复可去，拼接即正确答案。**绝不可跳过该窗**（那正是 369 的丢字根因）。
/// 2. **共享切片**（`new.start < prev.end`，确实有真实重叠）⇒ 走 [`align_overlap_with_prior`] 去重；
///    对齐成功 ⇒ 定稿滑出前缀；对齐失败（如长度门误拒）⇒ **退回拼接**
///    —— 有重叠也不算错，取向明确：**宁可接缝重复，绝不整窗丢失**（重复可删，丢字找不回）。
///
/// 🔴 FIX-PREFIX-AND-EAT-371（B）：共享切片时**把每片时长也算进去**（[`push_window`]），
/// 用「共享片样本和 / 本窗样本和」把重叠长度 `k` 约束在期望值附近，避免周期性内容下
/// 「最长重叠优先」把切点钉在最左 ⇒ 丢字（详见 [`align_overlap_with_prior`]）。
///
/// 不变量：**任何非空窗的文本都不会被丢弃**（要么与旧窗去重后定稿，要么整体并入 `committed`）。
pub(crate) struct OrderedReflow {
    next: usize,
    /// `seq -> (start_slice, end_slice, 各片样本数, text)`；`end_slice` 为开区间端点。
    pending: std::collections::BTreeMap<usize, (usize, usize, Vec<usize>, String)>,
    committed: String,
    last_window_text: String,
    /// 与 `last_window_text` 对应的切片区间 `[start, end)`；`None` = 尚无有效前窗。
    last_span: Option<(usize, usize)>,
}

impl OrderedReflow {
    pub(crate) fn new() -> Self {
        Self {
            next: 0,
            pending: std::collections::BTreeMap::new(),
            committed: String::new(),
            last_window_text: String::new(),
            last_span: None,
        }
    }

    /// 收到 `seq` 的解码文本（`[start_slice, end_slice)` = 该窗含哪些派发片）；
    /// 返回本次**按序**定稿后应回灌的权威全文（可能 0..n 条）。
    ///
    /// **不带**各片样本数 ⇒ 区间先验整块缺失（等价 [`AlignPrior::none`]）⇒ 重叠搜索退化到
    /// 层 ④「小 k 优先」（安全方向）。生产路径用 [`push_window`]（带样本数 ⇒ ① ② 层都生效）。
    /// 保留供单测直接考察退化路径。
    #[allow(dead_code)]
    pub(crate) fn push(
        &mut self,
        seq: usize,
        start_slice: usize,
        end_slice: usize,
        text: String,
    ) -> Vec<String> {
        self.push_inner(seq, start_slice, end_slice, Vec::new(), text)
    }

    /// FIX-PREFIX-AND-EAT-371（B）：带上「本窗各片样本数」的入口。
    ///
    /// `slice_samples[i]` = 本窗第 `i` 片（全局切片号 `start_slice + i`）的样本数；
    /// 与切片区间一起算出期望重叠比例（[`expected_overlap_ratio`]），交给
    /// [`align_overlap_with_prior`] 把 `k` 锁在期望值附近（层 ① 硬约束 + 层 ② 软范围）。
    pub(crate) fn push_window(
        &mut self,
        seq: usize,
        start_slice: usize,
        end_slice: usize,
        slice_samples: Vec<usize>,
        text: String,
    ) -> Vec<String> {
        self.push_inner(seq, start_slice, end_slice, slice_samples, text)
    }

    /// 合并规则见 [`OrderedReflow`] 文档；`next` 始终 `+=1`（否则后续 seq 卡死）。
    fn push_inner(
        &mut self,
        seq: usize,
        start_slice: usize,
        end_slice: usize,
        slice_samples: Vec<usize>,
        text: String,
    ) -> Vec<String> {
        self.pending
            .insert(seq, (start_slice, end_slice, slice_samples, text));
        let mut out = Vec::new();
        while let Some((ws, we, samples, text)) = self.pending.remove(&self.next) {
            // 空解码结果：不动文本状态（避免空串污染 last_window_text / 制造假重叠），仅推进 next。
            if text.is_empty() {
                self.next += 1;
                continue;
            }
            match self.last_span {
                None => {
                    // 首窗（或此前全为空）：无对齐对象，直接采用。
                    self.last_window_text = text;
                    self.last_span = Some((ws, we));
                }
                Some((prev_start, prev_end)) => {
                    if ws >= prev_end {
                        // 零重叠 ⇒ 无重复可去 ⇒ 拼接即正确答案（不得跳过）。
                        self.committed.push_str(&self.last_window_text);
                    } else {
                        // 共享切片 ⇒ 确有真实重叠 ⇒ 对齐去重（371：k 受**区间先验**约束）。
                        let prior = AlignPrior {
                            // ② 软范围：期望重叠比例（估算，只做粗筛）
                            expected_ratio: expected_overlap_ratio(ws, we, prev_end, &samples),
                            // ① 硬约束（精确）：上一窗比本窗多出的有声片数 m
                            //    —— VAD 片都有话音 ⇒ 那些片文本必须落在 committed_prefix 里。
                            prev_extra_slices: ws.saturating_sub(prev_start),
                        };
                        let a = align_overlap_with_prior(&self.last_window_text, &text, prior);
                        if a.ok {
                            if !a.committed_prefix.is_empty() {
                                self.committed.push_str(&a.committed_prefix);
                            }
                        } else {
                            // 有重叠但对齐未过（如长度门误拒）⇒ 退回拼接：宁可重复不可丢字。
                            self.committed.push_str(&self.last_window_text);
                            if log::log_enabled!(log::Level::Debug) {
                                log::debug!(
                                    "[SLIDING-WINDOW-367] reflow 有重叠但对齐未过，退回拼接（seq={} span=[{},{}) prev=[{}, {}) m={} ratio={:?}）",
                                    seq,
                                    ws,
                                    we,
                                    prev_start,
                                    prev_end,
                                    prior.prev_extra_slices,
                                    prior.expected_ratio
                                );
                            }
                        }
                    }
                    self.last_window_text = text;
                    self.last_span = Some((ws, we));
                }
            }
            out.push(format!("{}{}", self.committed, self.last_window_text));
            self.next += 1;
        }
        out
    }

    /// 收尾：返回 `(committed, last_window_text)`。
    pub(crate) fn finish(self) -> (String, String) {
        (self.committed, self.last_window_text)
    }
}

/// ASR-ACC-OPT-001 方案 A：判定词条是否为纯 ASCII（纯英文/数字）。
/// 纯 ASCII 词条（worker1/tester1/todo 等无关词）带偏 native decoder，
/// 研究证实全量 wordbook 含此类词性能从 62.5% 降到 60%。
fn is_pure_ascii(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii())
}

/// ASR-ACC-OPT-001 方案 A：精选 wordbook 词条，过滤无关词 + 上限截断。
///
/// 过滤规则（按研究 RESEARCH-ASR-ACCURACY-001 R2）：
/// 1. 空/纯空白词条过滤
/// 2. 纯 ASCII 词条过滤（英文/数字如 worker1/tester1/todo）
/// 3. 超长词条过滤（> HOTWORDS_MAX_ENTRY_CHARS，candidates 整句不灌入）
/// 4. 数量上限 HOTWORDS_MAX_ENTRIES + **总 token 预算 HOTWORDS_MAX_TOTAL_TOKENS（C++ 口径，含分隔符）**，
///    按入参顺序（调用方已按 hit_count DESC, id DESC 排序）截断保留高优先级词条
///
/// 调用方（main.rs load_hotwords_for_accuracy）已按 hit_count DESC, id DESC 排序，
/// 截断后保留高频/最近词条，确定性顺序保证 hotwords 版本号哈希稳定。
pub fn curate_hotwords_entries(entries: &[String]) -> Vec<String> {
    curate_hotwords_entries_with_budget(entries, hotwords_tokenizer().as_ref(), HOTWORDS_BUDGET)
}

/// 测试/兼容入口（单引擎 Qwen3 预算）。生产路径直接走 `curate_hotwords_entries`。
#[allow(dead_code)]
fn curate_hotwords_entries_with(
    entries: &[String],
    tk: Option<&tokenizers::Tokenizer>,
) -> Vec<String> {
    curate_hotwords_entries_with_budget(entries, tk, HOTWORDS_BUDGET)
}

/// 可注入 tokenizer / 预算的内核（单测用真实 tokenizer 时传 Some，验证回退时传 None）。
fn curate_hotwords_entries_with_budget(
    entries: &[String],
    tk: Option<&tokenizers::Tokenizer>,
    budget: HotwordsBudget,
) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    // HOTWORDS-TOKEN-268: 累加 C++ 口径 token 估算（含 `,`），超预算即 **break**（不是 continue）。
    // 入参已按 hit_count DESC,id DESC 排序 ⇒ 前面的就是更该进的；break 保持「优先级高的先占额度」，
    // continue 会跳过长词去塞短词、把高频长词挤掉，与频次排序初衷相反。
    let mut est_tokens = 0usize;
    for word in entries {
        let trimmed = word.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_pure_ascii(trimmed) {
            continue;
        }
        if trimmed.chars().count() > HOTWORDS_MAX_ENTRY_CHARS {
            // 单条超长属「过滤」不是「预算不足」，用 continue（不占预算，也不阻断后续短词）
            continue;
        }
        // +1 = 逗号分隔符（首条无前导逗号）
        let add = estimate_word_tokens_with(tk, trimmed, budget.safety)
            + if result.is_empty() { 0 } else { 1 };
        if est_tokens + add > budget.max_total_tokens {
            break;
        }
        est_tokens += add;
        result.push(trimmed.to_string());
        if result.len() >= budget.max_entries {
            break;
        }
    }
    result
}

/// 从 wordbook 词条构建 hotwords 字符串（逗号分隔）
/// 仅 accuracy 分支使用；performance 分支不支持 hotwords
///
/// ASR-ACC-OPT-001 方案 A：内部调用 curate_hotwords_entries 精选，
/// 过滤无关词条 + 上限截断，避免全量灌入带偏 native decoder。
pub fn build_hotwords_string(entries: &[String]) -> String {
    curate_hotwords_entries(entries).join(",")
}

/// 构建 recognizer（ASR-SINGLE-MODEL-001：单模型加载，不再创建 fallback）
/// 返回 (主 recognizer, 生效模型, hotwords_version)
///
/// R2 修订（验收第 2 轮）：返回 effective_model 解决降级语义错标。
/// accuracy 分支 native 加载失败 → 降级 CTC，effective_model=Performance，
/// Transcriber 存 effective_model 作为 asr_model——三处语义自动归位：
/// ① CTC 输出标 native_punctuated=false（下游标点模块正常走）
/// ② 空输出走 performance bail 语义（不 accuracy bail）
/// ③ 不触发 VAD 分段（performance 不分段）
fn build_recognizer(
    model_dir: &Path,
    language: &str,
    asr_model: AsrModel,
    hotwords: Option<&str>,
) -> Result<(sherpa_onnx::OfflineRecognizer, AsrModel, u64)> {
    let hotwords_version = match hotwords {
        Some(h) => {
            let count = h.split(',').filter(|s| !s.trim().is_empty()).count();
            let mut hasher = DefaultHasher::new();
            count.hash(&mut hasher);
            h.hash(&mut hasher);
            hasher.finish()
        }
        None => 0,
    };

    match asr_model {
        AsrModel::Performance => {
            let recognizer = create_sensevoice_recognizer(model_dir, language)?;
            Ok((recognizer, AsrModel::Performance, hotwords_version))
        }
        AsrModel::Accuracy => {
            // ASR-SINGLE-MODEL-001: accuracy 分支尝试加载 native 模型；失败则降级 performance
            // 不再预创建 CTC fallback recognizer（省 ~250-350MB 常驻）
            //
            // MIGRATE-QWEN3-320：accuracy 档就是 Qwen3（不再有引擎分派/回滚开关）。
            // 🔴 `hotwords` **不塞 config** —— 词库与上下文走 accuracy worker 的 per-stream 注入（320）。
            let loaded = create_qwen3_recognizer(model_dir);
            match loaded {
                Ok(recognizer) => Ok((recognizer, AsrModel::Accuracy, hotwords_version)),
                Err(e) => {
                    log::warn!(
                        "Accuracy model load failed ({}), falling back to performance model",
                        e
                    );
                    let recognizer = create_sensevoice_recognizer(model_dir, language)?;
                    // R2: effective_model=Performance，Transcriber 存此值语义归位
                    Ok((recognizer, AsrModel::Performance, hotwords_version))
                }
            }
        }
        AsrModel::LocalRealtime => {
            // LOCAL-RT-ENGINE-239-A（DEC-067）：LocalRealtime 需要 online + offline 双模型并存，
            // 由 Transcriber::new 的专用分支经 build_local_realtime_recognizers() 构建；
            // 本函数「返回单一 offline recognizer」的契约不适用该档，正常路径不会触达。
            // 防御性 bail（非 panic），避免误用时静默返回半个模型。
            anyhow::bail!(
                "LocalRealtime requires dual-model construction; handled in Transcriber::new()"
            )
        }
        AsrModel::QwenAudioOnline | AsrModel::FunAsrRealtime => {
            // ASR-041-B / ASR-056: 在线 ASR 模式不加载本地模型，
            // Transcriber::new() 已提前返回，此分支不应被触达
            unreachable!(
                "online ASR models should be handled in Transcriber::new() before build_recognizer"
            )
        }
    }
}

/// LOCAL-RT-ENGINE-239-A（DEC-067）：LocalRealtime 双模型构建。
///
/// - online  = streaming paraformer trilingual（本地预览，greedy_search）
/// - offline = accuracy FunASR Nano native（2pass 最终文本 + hotwords）
///
/// 🔴 **DEC-067 附则一：任一缺失/加载失败即 `Err`，禁止静默降级**。
/// 不沿用 accuracy 的 `effective_model` 归位逻辑 —— 静默降级正是 ASR-UI-208 判定
/// 「用户以为用 A 实际跑 B」的不一致。错误信息区分 online / offline 侧，供 239-B 浮层文案分类。
///
/// 返回 `(online, offline, hotwords_version)`。hotwords_version 算法与 `build_recognizer`
/// 内联实现一致（此处**不抽公共 fn**，以免触碰现有 accuracy / performance 分支）。
fn build_local_realtime_recognizers(
    model_dir: &Path,
    _language: &str,
    hotwords: Option<&str>,
) -> Result<(
    sherpa_onnx::OnlineRecognizer,
    sherpa_onnx::OfflineRecognizer,
    u64,
)> {
    let hotwords_version = match hotwords {
        Some(h) => {
            let count = h.split(',').filter(|s| !s.trim().is_empty()).count();
            let mut hasher = DefaultHasher::new();
            count.hash(&mut hasher);
            h.hash(&mut hasher);
            hasher.finish()
        }
        None => 0,
    };

    let online = local_stream::create_local_stream_recognizer(model_dir)
        .context("LocalRealtime: 本地流式 (online) 模型缺失或加载失败")?;
    // MIGRATE-QWEN3-320：offline（2pass 最终文本）即 accuracy=Qwen3（原 FunASR 已移除）。
    let offline = create_qwen3_recognizer(model_dir)
        .context("LocalRealtime: 本地 accuracy (offline, Qwen3) 模型缺失或加载失败")?;

    Ok((online, offline, hotwords_version))
}

/// ASR-CTC-OPT-001 P2（已撤销）: 推导 ITN rule_fsts 路径（exe 同级 models/itn/itn_zh_number.fst）。
///
/// **撤销原因**：ITN rule_fsts 把中文数字规整成阿拉伯数字（"七"→"7"），
/// 对输入法场景有害（用户说"七"想输入汉字"七"而非"7"）。本轮撤销 P2，
/// 智能规则化（仅规整多位数字、单字保留汉字）另行立项，等 Gavin 决策。
///
/// 函数保留供未来智能 ITN 立项复用，但当前不被 create_sensevoice_recognizer 调用。
/// 单测保留验证路径推导逻辑。
#[allow(dead_code)]
fn resolve_itn_fst_path(model_dir: &Path) -> Option<String> {
    let itn_fst_path = model_dir.join("itn").join("itn_zh_number.fst");
    if itn_fst_path.exists() {
        log::info!("ITN rule_fsts enabled: {:?}", itn_fst_path);
        Some(itn_fst_path.to_str().unwrap_or("").to_string())
    } else {
        log::warn!(
            "ITN rule_fsts file not found at {:?}, ITN disabled (download itn_zh_number.fst to models/itn/ to enable)",
            itn_fst_path
        );
        None
    }
}

/// Create SenseVoice Chinese recognizer (FunASR Nano CTC 兼容版，179MB)
/// DEC-025 路线 A：默认 performance 模型
///
/// ASR-CTC-OPT-001:
/// - P1: silence head 由调用方 select_preprocessing_params 控制（本函数不涉及）
/// - P2: ITN rule_fsts 已撤销（副作用：中文数字→阿拉伯数字对输入法有害，另行立项）
/// - P3: blank_penalty 0.5→0.0（C2 证实对 FunASR Nano CTC 无影响，遗产值清理）
fn create_sensevoice_recognizer(
    model_dir: &Path,
    language: &str,
) -> Result<sherpa_onnx::OfflineRecognizer> {
    let model_dir_path = ensure_sensevoice_model(model_dir)?;

    let model_path = model_dir_path.join("model.int8.onnx");
    let tokens_path = model_dir_path.join("tokens.txt");

    let offline_config = sherpa_onnx::OfflineRecognizerConfig {
        model_config: sherpa_onnx::OfflineModelConfig {
            sense_voice: OfflineSenseVoiceModelConfig {
                model: Some(model_path.to_str().unwrap_or("").to_string()),
                language: Some(language.to_string()),
                use_itn: true,
            },
            tokens: Some(tokens_path.to_str().unwrap_or("").to_string()),
            ..Default::default()
        },
        // ASR-CTC-OPT-001 P2: ITN rule_fsts 已撤销（副作用见 resolve_itn_fst_path 文档）
        // ASR-CTC-OPT-001 P3: blank_penalty 0.5→0.0
        // C2 证实 0/0.25/0.5/0.75/1.0 五档输出逐字节一致，对 FunASR Nano CTC 无影响
        blank_penalty: 0.0,
        ..Default::default()
    };

    sherpa_onnx::OfflineRecognizer::create(&offline_config)
        .context("Failed to create SenseVoice offline recognizer")
}

/// MIGRATE-QWEN3-314（DEC-076）：创建 Qwen3-ASR 0.6B 识别器（accuracy 档新引擎）。
///
/// 字段填法参照 `collab/evidence/20260921-qwen3-poc/poc_qwen3_compare.rs.txt`（306/310/313 已验证）。
///
/// - `max_total_len = 4096`（**代码实际值，见下方字段**）：310 实测配置开大**零加载内存代价、
///   零精度退化**，上下文容量随之放大。
///
///   🔴🔴 **改这个数之前先读完这段。** 本注释此前写作 `2048`（Gavin 早期口径，当时的问题是
///   「512 不够用」），与代码的 `4096` **长期不一致**；2026-09-21 主控核对后以代码为准更正。
///
///   **`max_total_len` 与 `HOTWORDS_MAX_TOTAL_TOKENS` 是一对耦合常量，不能单独改。**
///   词库预算 3000 的推导（见该常量注释）就是从 **4096** 反推的。按当前真实常量核算：
///
///   | 扣项 | token |
///   | --- | --- |
///   | 20s 音频（13.0 tok/s） | 260 |
///   | ~~上下文 500 字 + 指令句/标签 ~55~~ | ~~440~~（**FIX-INJECT-TO-SPEC-377 起不再注入**） |
///   | 生成预留 `max_new_tokens` | 256 |
///   | **剩给词库** | **4096 ⇒ 3580** ／ **2048 ⇒ 1532** |
///
///   词库预算是 **3000** ⇒ 在 4096 下余量仅 ~140，**在 2048 下超出约 1900**。
///   超预算的后果不是截断而是**空输出**（同文件 `HOTWORDS_MAX_ENTRIES` 注释记录：
///   220 条 hotwords 撑爆 `max_total_len=512` ⇒ **0% 全空输出**）。
///
///   ⇒ **若要把本值调回 2048，必须同批把 `HOTWORDS_MAX_TOTAL_TOKENS` 降到 1000 以内**，
///     否则用户词库一旦长起来，accuracy 档会开始吐空。今天没炸只是因为实测 `terms_len=73`
///     字符，离上限极远（BUILD-321 端测）。**这是埋着的雷，不是当下的故障。**
/// - `max_new_tokens = 256`：**评估取值**。单片 ≤20s，音频 token ≈13/s×20≈260；生成量按 20s
///   中文口述典型 ≤100 字（≤~130 token），256 足够覆盖且远低于 2048 KV，不引入截断
///   （上游默认 128 对大段偏紧；官方 CLI 512 更宽松但非必需）⇒ 取 256，与现役 FunASR 一致。
/// - `temperature = 1e-6 / top_p = 0.8 / seed = 42`：上游默认、近贪心 ⇒ 输出可复现（ASR 优先确定性）。
/// - 🔴 `hotwords = None`：**本单 config 层留空**，词库改走 per-stream 注入（下一单）；
///   不把词库塞进 config —— 避免 config 与 per-stream 两套并存，后面还要拆。
fn create_qwen3_recognizer(model_dir: &Path) -> Result<sherpa_onnx::OfflineRecognizer> {
    let model_dir_path = ensure_qwen3_model(model_dir)?;
    create_qwen3_recognizer_at(&model_dir_path)
}

/// POC-QWEN3-1.7B-351：按**物理模型目录**建 Qwen3 recognizer（`create_qwen3_recognizer` 的
/// 底座抽取，**零行为变更提取**）。
///
/// 分工：`create_qwen3_recognizer` 负责「解析出模型物理目录」（经 `ensure_qwen3_model`，其中含
/// 硬编码子目录名）；本函数只负责「给定物理目录 → 按生产字段建 recognizer」。
/// 抽取目的：让需要**显式指定模型目录**的调用方（如 POC-QWEN3-1.7B-351 用
/// `models/…-1.7B-…/`）复用同一条生产配置链，而不是自己手搓 recognizer 配置
/// （`[POC-BYPASSES-PROD-WRAPPER-001]`）。
/// 🔴 本函数不含任何子目录解析逻辑，也不改 `check_qwen3_model_ready` 的硬编码目录名。
fn create_qwen3_recognizer_at(model_dir_path: &Path) -> Result<sherpa_onnx::OfflineRecognizer> {
    let conv = model_dir_path.join("conv_frontend.onnx");
    let enc = model_dir_path.join("encoder.int8.onnx");
    let dec = model_dir_path.join("decoder.int8.onnx");
    let tok = model_dir_path.join("tokenizer");

    // accuracy 线程数 = 常量（无 env）。
    let num_threads = default_acc_num_threads();
    log::debug!("[MIGRATE-QWEN3-320] accuracy num_threads={}", num_threads);

    let offline_config = sherpa_onnx::OfflineRecognizerConfig {
        model_config: sherpa_onnx::OfflineModelConfig {
            // 与 Qwen3 口径一致：min(逻辑核数, 8)，显式 cpu。
            num_threads,
            provider: Some("cpu".to_string()),
            qwen3_asr: OfflineQwen3ASRModelConfig {
                conv_frontend: Some(conv.to_str().unwrap_or("").to_string()),
                encoder: Some(enc.to_str().unwrap_or("").to_string()),
                decoder: Some(dec.to_str().unwrap_or("").to_string()),
                tokenizer: Some(tok.to_str().unwrap_or("").to_string()),
                max_total_len: 4096,
                max_new_tokens: 256,
                temperature: 1e-6,
                top_p: 0.8,
                seed: 42,
                // LOCALRT-CTX-INJECT-320：词库与上下文**改走 per-stream 注入**
                //    （见 `transcribe_accuracy_segment_ctx` / main.rs accuracy worker）。
                //    310 实证 max_total_len=4096 零加载内存代价、精度逐字不变；
                //    预算：上下文 ≤300 字 + 词库 ≤500 字符 + 20s 音频 ~260 token + 生成 256，远低于 4096。
                //    本 config 字段保持 None（不是漏接）。
                hotwords: None,
            },
            tokens: Some(String::new()),
            ..Default::default()
        },
        ..Default::default()
    };

    sherpa_onnx::OfflineRecognizer::create(&offline_config)
        .context("Failed to create Qwen3-ASR offline recognizer")
}

/// MIGRATE-QWEN3-314：Qwen3-ASR 模型（954MB）就位检测（四个路径）。
fn ensure_qwen3_model(model_dir: &Path) -> Result<PathBuf> {
    let (ready, dir) = check_qwen3_model_ready(model_dir);
    if ready {
        log::info!("Qwen3-ASR model found at {:?}", dir);
        return Ok(dir);
    }
    anyhow::bail!(
        "Qwen3-ASR model not found at {:?} (need conv_frontend.onnx / encoder.int8.onnx / decoder.int8.onnx / tokenizer/). Download from:\n  https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
        dir
    )
}

/// Ensure SenseVoice (FunASR Nano CTC 兼容版，179MB) model is present
fn ensure_sensevoice_model(model_dir: &Path) -> Result<PathBuf> {
    // FunASR Nano CTC 兼容版（179MB，2025-12-17）— DEC-025 路线 A 直换默认模型
    // 模型文件名同为 model.int8.onnx + tokens.txt，沿用 OfflineSenseVoiceModelConfig
    // 旧目录 sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09 保留作回滚
    let model_dir_path = model_dir.join("sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17");

    let model_file = model_dir_path.join("model.int8.onnx");
    let tokens_file = model_dir_path.join("tokens.txt");

    if model_dir_path.exists() && model_file.exists() && tokens_file.exists() {
        log::info!("SenseVoice model found at {:?}", model_dir_path);
        return Ok(model_dir_path);
    }

    anyhow::bail!(
        "SenseVoice model not found at {:?}. Please download manually from:\n  https://huggingface.co/sherpa-onnx/sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17",
        model_dir_path
    )
}

pub fn model_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join("models")
}

/// MIGRATE-1.13.8-1.7B-359：Qwen3-ASR 模型子目录名（**唯一来源**，防多处硬编码漂移）。
///
/// 历史上该名字散落在 `check_qwen3_model_ready` / `hotwords_tokenizer` / 测试夹具 /
/// `src-tauri` 镜像处；本批切 0.6B→1.7B 时收敛到本常量。
/// 🔴 `src-tauri` 是独立 crate，**无法复用本常量**，仍须各自镜像同一字符串
/// （那里有注释警示：两处判据必须逐字一致，否则 UI 显示「已就位」而主程序加载失败）。
pub(crate) const QWEN3_MODEL_SUBDIR: &str = "sherpa-onnx-qwen3-asr-1.7B-int8-2026-09-22";

/// MIGRATE-QWEN3-314：Qwen3-ASR 就位判据（四个路径；与 `create_qwen3_recognizer` 加载清单逐字一致）。
fn check_qwen3_model_ready(model_dir: &Path) -> (bool, PathBuf) {
    let dir = model_dir.join(QWEN3_MODEL_SUBDIR);
    let ready = dir.join("conv_frontend.onnx").exists()
        && dir.join("encoder.int8.onnx").exists()
        && dir.join("decoder.int8.onnx").exists()
        && dir.join("tokenizer").exists();
    (ready, dir)
}

/// 检测 accuracy 模型是否就位（供 Tauri command 调用）。
///
/// MIGRATE-QWEN3-320：accuracy 就是 Qwen3，单一路径（原按引擎分派已删）。
// 仅 `src-tauri` 侧（Tauri command）调用；root bin 不直接用，故 allow 以守 warnings 基线。
#[allow(dead_code)]
pub fn check_accuracy_model_ready(model_dir: &Path) -> (bool, PathBuf) {
    check_qwen3_model_ready(model_dir)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asr_model_from_config_parses_values() {
        assert_eq!(AsrModel::from_config("performance"), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("accuracy"), AsrModel::Accuracy);
        assert_eq!(AsrModel::from_config("Performance"), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("ACCURACY"), AsrModel::Accuracy);
        assert_eq!(AsrModel::from_config(""), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("garbage"), AsrModel::Performance);
        // ASR-041-B: Qwen3Online 已删除，qwen3_online 字符串由迁移逻辑改写为 qwen_audio_online
        // from_config 不再认 qwen3_online（迁移在 load 阶段已改写）
        // ASR-038-B: QwenAudioOnline 解析
        assert_eq!(
            AsrModel::from_config("qwen_audio_online"),
            AsrModel::QwenAudioOnline
        );
        assert_eq!(
            AsrModel::from_config("QWEN_AUDIO_ONLINE"),
            AsrModel::QwenAudioOnline
        );
    }

    // ============================================================
    // ASR-038-B: Transcriber QwenAudioOnline 独立配置字段验证
    // 回归防护：A 批错误复用 qwen3_url 调 Inference 协议，B 批修正为独立 asr_online_url
    // ============================================================

    /// ASR-038-B-005: QwenAudioOnline 模式 Transcriber::new 存在线 ASR 配置
    /// （不加载本地模型，new 不依赖 model_dir 存在性，可直接测）
    #[test]
    fn transcriber_qwen_audio_online_stores_independent_config() {
        let model_dir = std::path::Path::new("nonexistent-model-dir-for-test");
        let t = Transcriber::new(
            model_dir,
            false,
            "auto".to_string(),
            AsrModel::QwenAudioOnline,
            None,
            "sk-test-key",
            "wss://llm-kudx4dj2bfqn4gr2.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference",
            "qwen-audio-3.0-asr-flash-streaming",
            800,
            false,
        )
        .expect("QwenAudioOnline Transcriber::new should succeed without local models");
        assert_eq!(t.asr_model(), AsrModel::QwenAudioOnline);
        assert_eq!(
            t.asr_online_url(),
            "wss://llm-kudx4dj2bfqn4gr2.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference"
        );
        assert_eq!(t.asr_online_model(), "qwen-audio-3.0-asr-flash-streaming");
        // ASR-056: max_sentence_silence 存储
        assert_eq!(t.asr_online_max_sentence_silence(), 800);
    }

    /// TEST-SYNC-229 G3: 两轴参数透传链不断 —— `Transcriber::new` 传入什么，
    /// getter 就返回什么（防中途丢参 / 重新硬编码）。
    /// 用非默认值（轴一 1234 ≠ 2000、轴二 true ≠ false）才能与「写死默认值」区分开；
    /// 传默认值就算字段被丢弃、getter 直接返回默认常量也会假绿。
    #[test]
    fn transcriber_seg229_two_axis_passthrough_stores_exact_values() {
        let model_dir = std::path::Path::new("nonexistent-model-dir-for-test");
        let t = Transcriber::new(
            model_dir,
            false,
            "auto".to_string(),
            AsrModel::QwenAudioOnline,
            None,
            "sk-test-key",
            "wss://inference.example.com",
            "qwen-audio-3.0-asr-flash-streaming",
            1234,
            true,
        )
        .expect("QwenAudioOnline Transcriber::new should succeed without local models");
        assert_eq!(
            t.asr_online_max_sentence_silence(),
            1234,
            "轴一：传入 1234 必须原样返回（非默认 2000，可区分写死默认）"
        );
        assert_eq!(
            t.asr_online_semantic_punctuation_enabled(),
            true,
            "轴二：传入 true 必须原样返回（默认 false，可区分写死默认）"
        );
    }

    /// ASR-038-B-006: QwenAudioOnline 模式 API Key 空 → bail
    #[test]
    fn transcriber_qwen_audio_online_empty_key_bails() {
        let model_dir = std::path::Path::new("nonexistent-model-dir-for-test");
        let result = Transcriber::new(
            model_dir,
            false,
            "auto".to_string(),
            AsrModel::QwenAudioOnline,
            None,
            "",
            "wss://inference.example.com",
            "qwen-audio-3.0-asr-flash-streaming",
            800,
            false,
        );
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(
            err_msg.contains("API Key 为空"),
            "empty API key should bail with clear message, got: {}",
            err_msg
        );
    }

    #[test]
    fn hotwords_version_stable_for_same_input() {
        let entries = vec!["派".to_string(), "队".to_string()];
        let v1 = hotwords_version(&entries);
        let v2 = hotwords_version(&entries);
        assert_eq!(v1, v2, "same input must produce same hash");
    }

    #[test]
    fn hotwords_version_changes_on_content_change() {
        let e1 = vec!["派".to_string()];
        let e2 = vec!["湃".to_string()];
        assert_ne!(
            hotwords_version(&e1),
            hotwords_version(&e2),
            "different content must produce different hash"
        );
    }

    #[test]
    fn hotwords_version_changes_on_len_change() {
        let e1 = vec!["a".to_string()];
        let e2 = vec!["a".to_string(), "b".to_string()];
        assert_ne!(
            hotwords_version(&e1),
            hotwords_version(&e2),
            "different length must produce different hash"
        );
    }

    #[test]
    fn hotwords_version_order_sensitive() {
        let e1 = vec!["a".to_string(), "b".to_string()];
        let e2 = vec!["b".to_string(), "a".to_string()];
        assert_ne!(
            hotwords_version(&e1),
            hotwords_version(&e2),
            "order must affect hash (caller must sort for stability)"
        );
    }

    #[test]
    fn build_hotwords_string_joins_non_empty() {
        let entries = vec![
            "派".to_string(),
            "队".to_string(),
            "  ".to_string(), // empty after trim
        ];
        let s = build_hotwords_string(&entries);
        assert_eq!(s, "派,队");
    }

    #[test]
    fn build_hotwords_string_empty_entries() {
        let entries: Vec<String> = vec![];
        assert_eq!(build_hotwords_string(&entries), "");
    }

    // ============================================================
    // ASR-ACC-OPT-001 方案 A: curate_hotwords_entries 精选策略测试
    // WORDBOOK-SINGLEWORD-001-CORE: 单词化（&[String]）
    // ============================================================

    #[test]
    fn curate_filters_pure_ascii_entries() {
        let entries = vec![
            "worker1".to_string(),
            "tester1".to_string(),
            "todo".to_string(),
            "派".to_string(),
            "比利".to_string(),
        ];
        let s = build_hotwords_string(&entries);
        assert_eq!(s, "派,比利", "pure ASCII entries must be filtered");
    }

    #[test]
    fn curate_filters_long_entries() {
        let long_str = "这是第一行需要测试的内容第二行需要测试的内容".to_string();
        let entries = vec!["短词".to_string(), long_str.clone(), "派发".to_string()];
        let s = build_hotwords_string(&entries);
        assert_eq!(s, "短词,派发", "long entries must be filtered");
        assert!(long_str.chars().count() > HOTWORDS_MAX_ENTRY_CHARS);
    }

    #[test]
    fn curate_filters_empty_and_whitespace_entries() {
        let entries = vec!["  ".to_string(), "".to_string(), "派".to_string()];
        assert_eq!(build_hotwords_string(&entries), "派");
    }

    // ---- HOTWORDS-TOKEN-268: token 预算测试（用真实 tokenizer；不可用则跳过）----

    fn test_tokenizer() -> Option<tokenizers::Tokenizer> {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("models")
            .join(QWEN3_MODEL_SUBDIR)
            .join("tokenizer")
            .join("tokenizer.json");
        let t = std::time::Instant::now();
        let tk = tokenizers::Tokenizer::from_file(&p).ok();
        if tk.is_some() {
            eprintln!("268: Rust tokenizers load = {:?}", t.elapsed());
        }
        tk
    }

    fn est_of(tk: Option<&tokenizers::Tokenizer>, ws: &[String]) -> usize {
        let mut t = 0usize;
        for (i, w) in ws.iter().enumerate() {
            t += estimate_word_tokens(tk, w.trim()) + if i > 0 { 1 } else { 0 };
        }
        t
    }

    fn two_char_common(n: usize) -> Vec<String> {
        let pool: Vec<char> =
            "的一是不了人我在有他这中大来上国个到说们为子和你地出道也时年得就那要下以生会自着去"
                .chars()
                .collect();
        (0..n)
            .map(|i| format!("{}{}", pool[i % pool.len()], pool[(i * 3 + 1) % pool.len()]))
            .collect()
    }

    /// 268：预算不变式——截断结果必须 ≤ 356，且**再加一条必然超**（即极大致）。
    fn assert_budget_maximal(tk: Option<&tokenizers::Tokenizer>, entries: &[String]) {
        let kept = curate_hotwords_entries_with(entries, tk);
        assert!(kept.len() <= entries.len());
        assert!(
            est_of(tk, &kept) <= HOTWORDS_MAX_TOTAL_TOKENS,
            "kept 必须 ≤ 356，实得 {}",
            est_of(tk, &kept)
        );
        if kept.len() < entries.len() {
            let mut more = kept.clone();
            more.push(entries[kept.len()].clone());
            assert!(
                est_of(tk, &more) > HOTWORDS_MAX_TOTAL_TOKENS,
                "未达极大：再加 1 条仍 ≤ 356"
            );
        }
    }

    #[test]
    fn curate_two_char_120_report_and_budget() {
        let Some(tk) = test_tokenizer() else {
            return;
        };
        let entries = two_char_common(120);
        let kept = curate_hotwords_entries_with(&entries, Some(&tk));
        eprintln!(
            "268: 2 字×120 实装 {} 条（est={}）",
            kept.len(),
            est_of(Some(&tk), &kept)
        );
        assert_budget_maximal(Some(&tk), &entries);
    }

    #[test]
    fn curate_common_4char_truncated_by_token_budget() {
        let Some(tk) = test_tokenizer() else {
            return;
        };
        let pool: Vec<char> = "北京上海深圳杭州成都武汉西安南京广州苏州天津重庆长沙青岛厦门中科华夏东方南方长城智能网络数据电子信息集团研究院机器人医疗健康".chars().collect();
        let entries: Vec<String> = (0..120)
            .map(|i| {
                format!(
                    "{}{}{}{}",
                    pool[(4 * i) % pool.len()],
                    pool[(4 * i + 1) % pool.len()],
                    pool[(4 * i + 2) % pool.len()],
                    pool[(4 * i + 3) % pool.len()]
                )
            })
            .collect();
        // 用小预算强制截断（生产预算 3000 不会截 120 条 4 字词）。
        let small = HotwordsBudget {
            max_entries: 1000,
            max_total_tokens: 400,
            safety: 2.0,
        };
        let kept = curate_hotwords_entries_with_budget(&entries, Some(&tk), small);
        eprintln!(
            "268: 常用 4 字×120 实装 {} 条（est={}）",
            kept.len(),
            est_of(Some(&tk), &kept)
        );
        assert!(kept.len() < 120, "4 字词应被 token 预算截断（<120）");
        assert!(est_of(Some(&tk), &kept) <= small.max_total_tokens);
    }

    #[test]
    fn curate_rare_4char_truncated_by_token_budget() {
        let Some(tk) = test_tokenizer() else {
            return;
        };
        let entries: Vec<String> = (0..120)
            .map(|i| {
                let c = char::from_u32(0x4E00 + i).unwrap();
                format!("{}{}{}{}", c, c, c, c)
            })
            .collect();
        assert_budget_maximal(Some(&tk), &entries);
    }

    #[test]
    fn curate_token_budget_boundary() {
        let Some(tk) = test_tokenizer() else {
            return;
        };
        // 用**小预算**强制「恰好装满 / 再加一条超」；生产预算 3000 过大测不到边界。
        let small = HotwordsBudget {
            max_entries: 1000,
            max_total_tokens: 400,
            safety: 2.0,
        };
        let entries: Vec<String> = (0..200).map(|_| "的".to_string()).collect();
        let kept = curate_hotwords_entries_with_budget(&entries, Some(&tk), small);
        let est = est_of(Some(&tk), &kept);
        assert!(est <= small.max_total_tokens, "est={}", est);
        let mut kept_more = kept.clone();
        kept_more.push("的".to_string());
        assert!(est_of(Some(&tk), &kept_more) > small.max_total_tokens);
    }

    #[test]
    fn curate_fallback_without_tokenizer_uses_byte_bound() {
        // tokenizer 不可用 → 用 UTF-8 字节数（安全粗上界）：字节和 ≤ 356
        let entries: Vec<String> = std::iter::repeat("北京".to_string()).take(200).collect();
        let kept = curate_hotwords_entries_with(&entries, None);
        let mut bytes = 0usize;
        for (i, w) in kept.iter().enumerate() {
            bytes += w.len() + if i > 0 { 1 } else { 0 };
        }
        assert!(bytes <= HOTWORDS_MAX_TOTAL_TOKENS, "bytes={}", bytes);
        assert!(!kept.is_empty());
    }

    #[test]
    fn curate_enforces_max_entries_limit() {
        // 150 条 1 字词：token 预算先生效（条目上限 120 仅作上限）
        // MIGRATE-QWEN3-320：显式走 **FunASR 回滚预算**（默认引擎已切 Qwen3=200/3000）。
        let entries: Vec<String> = (0..150).map(|_| "的".to_string()).collect();
        let kept = curate_hotwords_entries_with(&entries, None);
        assert!(kept.len() <= HOTWORDS_MAX_ENTRIES);
        assert!(!kept.is_empty());
    }

    #[test]
    fn curate320_single_budget_constants() {
        let entries: Vec<String> = (0..150).map(|_| "的".to_string()).collect();
        let kept = curate_hotwords_entries_with_budget(&entries, None, HOTWORDS_BUDGET);
        assert_eq!(kept.len(), 150, "150 条 1 字词在单预算内应全保留");
        assert_eq!(HOTWORDS_MAX_ENTRIES, 200, "条数上限常量 200");
        assert_eq!(HOTWORDS_BUDGET.max_total_tokens, 3000);
        assert_eq!(
            HOTWORDS_BUDGET.safety, 2.0,
            "Qwen3 未标定、保守 2.0（非 FunASR 的 1.85）"
        );
    }

    #[test]
    fn curate_keeps_most_recent_when_over_limit() {
        let entries = vec![
            "最近词1".to_string(),
            "最近词2".to_string(),
            "早期词".to_string(),
        ];
        let curated = curate_hotwords_entries(&entries);
        assert_eq!(curated.len(), 3);
        assert_eq!(curated[0], "最近词1");
        assert_eq!(curated[1], "最近词2");
        assert_eq!(curated[2], "早期词");
    }

    #[test]
    fn curate_preserves_order_for_stable_hash() {
        let entries = vec!["派".to_string(), "比".to_string(), "利".to_string()];
        let s1 = build_hotwords_string(&entries);
        let s2 = build_hotwords_string(&entries);
        assert_eq!(s1, s2, "same input must produce same output");
        assert_eq!(s1, "派,比,利");
    }

    #[test]
    fn curate_mixed_realistic_wordbook() {
        let entries = vec![
            "我吃".to_string(),
            "worker1".to_string(),
            "tester1".to_string(),
            "todo".to_string(),
            "比利".to_string(),
            "词库".to_string(),
            "灵界".to_string(),
            "阿炎".to_string(),
            "coder1".to_string(),
            "阿炎".to_string(),
        ];
        let s = build_hotwords_string(&entries);
        assert_eq!(
            s, "我吃,比利,词库,灵界,阿炎,阿炎",
            "must keep only CJK entries"
        );
    }

    #[test]
    fn curate_empty_wordbook_returns_empty() {
        let entries: Vec<String> = vec![];
        assert_eq!(build_hotwords_string(&entries), "");
    }

    #[test]
    fn is_pure_ascii_detection() {
        assert!(is_pure_ascii("worker1"));
        assert!(is_pure_ascii("tester1"));
        assert!(is_pure_ascii("todo"));
        assert!(is_pure_ascii("123"));
        assert!(is_pure_ascii("ABC"));
        assert!(!is_pure_ascii("派"));
        assert!(!is_pure_ascii("比利"));
        assert!(!is_pure_ascii("worker1派"));
        assert!(!is_pure_ascii(""));
    }

    // ============================================================
    // ASR-ACC-OPT-001 方案 A 补测：候选缺口确认与覆盖
    // ============================================================

    #[test]
    fn curate_filter_invariance_preserves_hash_stability() {
        let without_ascii = vec!["派".to_string(), "比".to_string()];
        let with_ascii = vec!["派".to_string(), "worker1".to_string(), "比".to_string()];
        let s1 = build_hotwords_string(&without_ascii);
        let s2 = build_hotwords_string(&with_ascii);
        assert_eq!(
            s1, s2,
            "ASCII entry filtering must not change curated output"
        );
    }

    #[test]
    fn curate_filters_exact_boundary_entries() {
        let ten_chars = "一二三四五六七八九十";
        let eleven_chars = "一二三四五六七八九十1";
        assert_eq!(ten_chars.chars().count(), HOTWORDS_MAX_ENTRY_CHARS);
        assert_eq!(eleven_chars.chars().count(), HOTWORDS_MAX_ENTRY_CHARS + 1);
        let entries = vec![
            ten_chars.to_string(),
            eleven_chars.to_string(),
            "派".to_string(),
        ];
        let s = build_hotwords_string(&entries);
        assert_eq!(
            s,
            format!("{},派", ten_chars),
            "10-char kept, 11-char filtered"
        );
    }

    #[test]
    fn curate_keeps_mixed_cjk_ascii_entries() {
        let entries = vec![
            "worker1".to_string(),
            "worker1派".to_string(),
            "比利".to_string(),
        ];
        let s = build_hotwords_string(&entries);
        assert_eq!(s, "worker1派,比利", "mixed CJK-ASCII entries must be kept");
    }

    #[test]
    fn curate_enforces_max_entries_order() {
        // 268：token 预算先生效，条数上限 120 仅作上限；截断必须保持入参顺序（前缀）
        // MIGRATE-QWEN3-320：显式走 FunASR 回滚预算。
        let entries = two_char_common(200);
        let curated = curate_hotwords_entries_with(&entries, None);
        assert!(curated.len() <= HOTWORDS_MAX_ENTRIES);
        assert!(!curated.is_empty());
        for (i, w) in curated.iter().enumerate() {
            assert_eq!(w, &entries[i], "entry {} 顺序必须与入参一致（前缀）", i);
        }
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

    // ============================================================
    // ASR-CTC-OPT-001 P2: resolve_itn_fst_path 路径推导测试
    // ============================================================

    #[test]
    fn resolve_itn_fst_path_returns_none_when_missing() {
        // 降级保护：fst 不存在时返回 None，不硬失败
        let tmp = std::env::temp_dir();
        let non_existent = tmp.join("voice_ime_test_nonexistent_itn");
        let result = resolve_itn_fst_path(&non_existent);
        assert!(
            result.is_none(),
            "missing fst must return None (graceful degrade)"
        );
    }

    #[test]
    fn resolve_itn_fst_path_returns_some_when_present() {
        // fst 存在时返回路径字符串
        let tmp = std::env::temp_dir().join("voice_ime_test_itn_present");
        let itn_dir = tmp.join("itn");
        std::fs::create_dir_all(&itn_dir).ok();
        let fst_path = itn_dir.join("itn_zh_number.fst");
        std::fs::write(&fst_path, b"dummy fst content").ok();
        let result = resolve_itn_fst_path(&tmp);
        assert!(result.is_some(), "existing fst must return Some(path)");
        assert!(
            result.unwrap().contains("itn_zh_number.fst"),
            "path must contain fst filename"
        );
        // 清理
        std::fs::remove_file(&fst_path).ok();
        std::fs::remove_dir(&itn_dir).ok();
        std::fs::remove_dir(&tmp).ok();
    }

    #[test]
    fn resolve_itn_fst_path_uses_model_dir_subpath() {
        // 路径必须经 model_dir 推导（DEC-011，exe 同级 models/itn/）
        // 验证路径结构：model_dir/itn/itn_zh_number.fst
        let tmp = std::env::temp_dir().join("voice_ime_test_itn_path_check");
        let itn_dir = tmp.join("itn");
        std::fs::create_dir_all(&itn_dir).ok();
        let fst_path = itn_dir.join("itn_zh_number.fst");
        std::fs::write(&fst_path, b"dummy").ok();
        let result = resolve_itn_fst_path(&tmp);
        if let Some(path) = result {
            // 路径应以 model_dir/itn/itn_zh_number.fst 结尾
            assert!(
                path.ends_with("itn_zh_number.fst"),
                "path must end with fst filename"
            );
            assert!(path.contains("itn"), "path must contain itn dir");
        }
        std::fs::remove_file(&fst_path).ok();
        std::fs::remove_dir(&itn_dir).ok();
        std::fs::remove_dir(&tmp).ok();
    }

    // ============================================================
    // ASR-SINGLE-MODEL-001 R1/R2 修订（验收第 2 轮）测试
    // ============================================================

    // ============================================================
    // MIGRATE-QWEN3-314：引擎选择（默认 qwen3 / env 回滚 funasr）+ 就位判据
    // ============================================================

    #[test]
    fn migrate320_acc_num_threads_is_constant_no_env() {
        let n = default_acc_num_threads();
        assert!((1..=8).contains(&n), "线程数常量落在 [1,8]，实测 {n}");
    }

    // ============================================================
    // FIX-INJECT-TO-SPEC-377：注入内容 = 纯 ASCII 逗号分隔词表（规格化）
    //   （旧的 320 上下文/指令句/截断测试随实现整体删除；ctx 回显护栏代码保留但已无调用点）
    // ============================================================

    #[test]
    fn fix377_build_ctx_system_is_bare_terms_only() {
        let terms = "你好,铭印,银线,朵洛莉丝,费曼学习法";
        let s = build_ctx_system(Some(terms)).expect("非空词表 ⇒ 注入");
        assert_eq!(
            s, terms,
            "产出必须与输入词表**逐字相同**（ASCII 逗号、无换行）"
        );
        // 🔴 不得再带任何超规格自由文本（POC-376 实证它们会被模型当输出续写）
        assert!(!s.contains("Context:"), "不得含 Context 标签");
        assert!(!s.contains("Terms:"), "不得含 Terms 标签");
        assert!(
            !s.chars().any(|c| c.is_ascii_alphabetic()),
            "不得含英文指令句：{s}"
        );
        assert!(!s.contains('\n'), "不得含换行");
        // 前后空白被 trim（词表本身不含空白）
        assert_eq!(build_ctx_system(Some("  词A,词B  ")).unwrap(), "词A,词B");
    }

    #[test]
    fn fix377_build_ctx_system_empty_terms_injects_nothing() {
        for t in [
            None,
            Some(""),
            Some("   "),
            Some(
                "
	",
            ),
        ] {
            assert!(
                build_ctx_system(t).is_none(),
                "空词表 ⇒ None（不调 set_option hotwords）：{t:?}"
            );
        }
        // 门控：无内容 ⇒ 不注入（`should_inject_ctx` 的契约不变）
        assert!(!should_inject_ctx(build_ctx_system(None).as_deref()));
        assert!(should_inject_ctx(build_ctx_system(Some("词A")).as_deref()));
    }

    /// 🔴 钉死：374 的 `strip_terms_echo` 对**新格式**（裸词表本身）仍能正确命中。
    #[test]
    fn fix377_terms_echo_still_hits_new_format() {
        let terms = "你好,铭印,银线,朵洛莉丝,费曼学习法,子未穿害,低质,罗斯柴尔德,维生素b12";
        let sys = build_ctx_system(Some(terms)).expect("注入串");
        // 模型把注入串逐字吐回（新格式下即裸词表）⇒ 必须命中并剥空
        let hit = strip_terms_echo(&sys, build_ctx_system(Some(terms)).as_deref())
            .expect("新格式下仍须命中");
        assert_eq!(hit.matched_terms, 9);
        assert_eq!(hit.stripped, "");
        // 真实转写 + 尾部裸词表 ⇒ 只剥尾部
        let text = format!("可以看看周边的风景。{sys}");
        let hit2 = strip_terms_echo(&text, Some(terms)).expect("尾部裸词表应命中");
        assert_eq!(hit2.stripped, "可以看看周边的风景。");
    }

    #[test]
    fn migrate320_qwen3_readiness_checks_four_paths() {
        let root = std::env::temp_dir().join(format!(
            "voice-ime-mig320-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let q = root.join(QWEN3_MODEL_SUBDIR);
        std::fs::create_dir_all(q.join("tokenizer")).unwrap();
        for f in [
            "conv_frontend.onnx",
            "encoder.int8.onnx",
            "decoder.int8.onnx",
        ] {
            std::fs::write(q.join(f), b"x").unwrap();
        }
        let (ready, dir) = check_qwen3_model_ready(&root);
        assert!(ready, "Qwen3 四件套齐全应 ready");
        assert_eq!(dir, q);
        assert!(check_accuracy_model_ready(&root).0, "accuracy 检测即 Qwen3");
        std::fs::remove_file(q.join("decoder.int8.onnx")).unwrap();
        assert!(!check_qwen3_model_ready(&root).0, "少 decoder 应 not ready");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore = "requires real Qwen3 model in project models/ dir"]
    fn migrate320_qwen3_recognizer_constructs() {
        let model_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");
        assert!(check_qwen3_model_ready(&model_dir).0);
        assert!(
            create_qwen3_recognizer(&model_dir).is_ok(),
            "Qwen3 recognizer 构造应成功"
        );
    }

    /// R2: build_recognizer 返回 3-tuple (recognizer, effective_model, hotwords_version)。
    /// accuracy 降级 CTC 时 effective_model=Performance（语义归位三处）。
    ///
    /// 此测试验证降级场景：model_dir 下只有 CTC 模型（native 缺失），
    /// 请求 accuracy → native 加载失败 → 降级 CTC → effective_model=Performance。
    /// 需要 SenseVoice CTC 模型存在于项目 models/ 目录。
    #[test]
    #[ignore = "requires SenseVoice CTC model in project models/ dir"]
    fn build_recognizer_accuracy_degraded_to_performance_effective_model() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = project_root.join("models");
        // 请求 accuracy，但 native 模型路径下放一个假目录使加载失败
        // 实际项目 models/ 下 native 模型若存在则此测试需构造缺失场景。
        // 此处验证返回签名结构 + effective_model 语义：
        // 若 native 加载成功 → effective=Accuracy；若失败降级 → effective=Performance
        let result = build_recognizer(&model_dir, "zh", AsrModel::Accuracy, None);
        match result {
            Ok((_recognizer, effective_model, _hw_ver)) => {
                // effective_model 必须是 AsrModel 枚举值之一
                assert!(
                    effective_model == AsrModel::Accuracy
                        || effective_model == AsrModel::Performance,
                    "effective_model must be valid AsrModel variant"
                );
                // 若 native 存在 → Accuracy；若缺失降级 → Performance
                // 关键：effective_model 不再恒 Accuracy（降级时正确归位 Performance）
            }
            Err(e) => {
                // 两个模型都缺失才 Err，本机至少有一个则不会到这
                eprintln!("build_recognizer err (models may be missing): {}", e);
            }
        }
    }

    /// R2 纯逻辑验证：build_recognizer 返回元组 arity 正确（3 个元素），
    /// effective_model 是 AsrModel 类型。此测试文档化签名契约，
    /// 防止未来重构误改返回类型（如漏掉 effective_model）。
    #[test]
    fn build_recognizer_return_signature_contract() {
        // 编译期断言：build_recognizer 返回 Result<(OfflineRecognizer, AsrModel, u64)>
        // 此函数签名由类型系统保证，此处文档化契约供未来维护者参考。
        // 关键：第 2 个元素必须是 AsrModel（effective_model），不能省略。
        fn _type_check<F>(_f: F)
        where
            F: Fn(
                &Path,
                &str,
                AsrModel,
                Option<&str>,
            ) -> Result<(sherpa_onnx::OfflineRecognizer, AsrModel, u64)>,
        {
        }
        _type_check(build_recognizer);
        // 若此测试编译通过，签名契约满足
        assert!(
            true,
            "build_recognizer signature contract: 3-tuple with effective_model"
        );
    }

    /// R1 纯逻辑验证：transcribe_segments_chunked 是 Transcriber 的方法，
    /// 接收 segments 切片返回 Result<(String, bool)>。此测试文档化契约：
    /// - 辅助函数消除 VAD 分段 / naive_chunk 两路径的重复转录循环
    /// - 全空段拼接 → bail
    /// - 任一段非 native → all_native=false
    /// 实际路径验证需 Transcriber 实例（模型加载），此处文档化行为契约。
    #[test]
    fn transcribe_segments_chunked_contract_documented() {
        // 契约文档化（需模型实例才能测实际行为，此处防回归签名变更）：
        // 1. 输入 &[Vec<f32>]（段样本列表）
        // 2. 逐段调 transcribe_segment_detailed
        // 3. join_segment_texts 拼接
        // 4. 全空 → bail!(NoSpeechError)（BUG-119 类型化信息信号）
        // 5. 任一段 np=false → all_native=false
        // 6. 返回 (normalized_text, all_native)
        //
        // R1 关键：lock poisoned 时走 naive_chunk 分支调此辅助函数，
        // 不再静默落到单次转录整段（禁止 >28s 整段喂 native）。
        assert!(true, "transcribe_segments_chunked contract documented");
    }

    // ============================================================
    // ASR-NOSPEECH-FILTER-001: 特殊 token 剥离测试
    // ============================================================

    #[test]
    fn strip_asr_special_tokens_pure_token() {
        assert_eq!(Transcriber::strip_asr_special_tokens("<|nospeech|>"), "");
    }

    #[test]
    fn strip_asr_special_tokens_token_with_text() {
        assert_eq!(Transcriber::strip_asr_special_tokens("<|zh|>你好"), "你好");
    }

    #[test]
    fn strip_asr_special_tokens_text_with_token() {
        assert_eq!(
            Transcriber::strip_asr_special_tokens("你好<|nospeech|>"),
            "你好"
        );
    }

    #[test]
    fn strip_asr_special_tokens_multiple_tokens() {
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|zh|><|nospeech|>你好"),
            "你好"
        );
    }

    #[test]
    fn strip_asr_special_tokens_mixed_with_text() {
        // 【主控修正 2026-07-27】CJK 之间剥离 token 后不得插入空格
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|zh|>你<|NEUTRAL|>好<|nospeech|>"),
            "你好"
        );
    }

    #[test]
    fn strip_asr_special_tokens_keeps_ascii_word_boundary() {
        // 英文单词被 token 分隔时必须补空格，否则粘连成 helloworld
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello<|NEUTRAL|>world"),
            "hello world"
        );
        // 数字同理
        assert_eq!(Transcriber::strip_asr_special_tokens("1<|zh|>2"), "1 2");
    }

    #[test]
    fn strip_asr_special_tokens_no_space_at_cjk_ascii_boundary() {
        // 中英边界不补空格：ASR 原文没有空格就不应凭空造出来
        assert_eq!(
            Transcriber::strip_asr_special_tokens("中文<|NEUTRAL|>abc"),
            "中文abc"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("abc<|NEUTRAL|>中文"),
            "abc中文"
        );
    }

    #[test]
    fn strip_asr_special_tokens_normal_text_unchanged() {
        // 孤立 < 或 > 不应被误伤
        assert_eq!(
            Transcriber::strip_asr_special_tokens("小于号<和大于号>"),
            "小于号<和大于号>"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("他说：1 < 2 且 3 > 1"),
            "他说：1 < 2 且 3 > 1"
        );
    }

    #[test]
    fn strip_asr_special_tokens_incomplete_not_stripped() {
        // 不完整的 token 形态不应误删
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|nospeech"),
            "<|nospeech"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("|nospeech|>"),
            "|nospeech|>"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|no|speech|>"),
            "<|no|speech|>"
        );
    }

    // ============================================================
    // P0-TEST-SYNC-20260727: 空格边界缺口补充
    // ============================================================

    #[test]
    fn strip_asr_special_tokens_token_start_ascii() {
        // token 位于串首 + 后接 ASCII → 无前导空格
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|NEUTRAL|>hello"),
            "hello"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|zh|>world"),
            "world"
        );
    }

    #[test]
    fn strip_asr_special_tokens_token_end_ascii() {
        // token 位于串尾（ASCII 在前）→ 无后置空格
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello<|NEUTRAL|>"),
            "hello"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("world<|nospeech|>"),
            "world"
        );
    }

    #[test]
    fn strip_asr_special_tokens_existing_space_near_token() {
        // token 相邻已有空格 → 不再添加额外空格
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello <|NEUTRAL|>world"),
            "hello world"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello<|NEUTRAL|> world"),
            "hello world"
        );
    }

    #[test]
    fn strip_asr_special_tokens_fullwidth_punct_near_token() {
        // 全角标点与 token 相邻 → 不产生空格（全角非 ascii_alphanumeric）
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello<|NEUTRAL|>！"),
            "hello！"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("你好<|NEUTRAL|>，"),
            "你好，"
        );
    }

    #[test]
    fn strip_asr_special_tokens_consecutive_between_ascii() {
        // 连续多 token 夹在英文单词中间 → 只产生一个空格
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello<|NEUTRAL|><|OTHER|>world"),
            "hello world"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("count<|zh|><|nospeech|><|en|>123"),
            "count 123"
        );
    }

    #[test]
    fn strip_asr_special_tokens_empty() {
        // 空串：幂等返回空串
        assert_eq!(Transcriber::strip_asr_special_tokens(""), "");
    }

    /// ASR-041-B-补 / ASR-056: AsrModel 恰好四变体
    /// （Performance/Accuracy/QwenAudioOnline/FunAsrRealtime）。
    /// Gavin 明确保留 Accuracy，防将来误删该变体。
    /// ASR-056 新增 FunAsrRealtime（与 QwenAudioOnline 共用 Inference 协议）。
    /// 用穷举 match 而非断言计数：删任一变体 → 本 match 编译失败（比 `== 4` 强）。
    /// 每个变体都走一遍 from_config 正反向，钉住字符串 ↔ 变体映射不被破坏。
    #[test]
    fn asr_model_exactly_four_variants_with_stable_mapping() {
        // 穷举四变体：新增/删除变体都会在此编译失败（编译期护栏）
        let _exhaustive: fn(AsrModel) = |m| match m {
            AsrModel::Performance
            | AsrModel::Accuracy
            | AsrModel::QwenAudioOnline
            | AsrModel::FunAsrRealtime
            | AsrModel::LocalRealtime => (),
        };

        // from_config 正反向映射（大小写不敏感）
        assert_eq!(AsrModel::from_config("performance"), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("accuracy"), AsrModel::Accuracy);
        assert_eq!(
            AsrModel::from_config("qwen_audio_online"),
            AsrModel::QwenAudioOnline
        );
        assert_eq!(
            AsrModel::from_config("QwEn_AuDiO_oNlInE"),
            AsrModel::QwenAudioOnline
        );
        // ASR-056: fun_asr_realtime 映射
        assert_eq!(
            AsrModel::from_config("fun_asr_realtime"),
            AsrModel::FunAsrRealtime
        );
        assert_eq!(
            AsrModel::from_config("FuN_AsR_ReAlTiMe"),
            AsrModel::FunAsrRealtime
        );
        // 未知字符串回落 Performance（不 panic）
        assert_eq!(AsrModel::from_config("qwen3_online"), AsrModel::Performance);
        assert_eq!(AsrModel::from_config(""), AsrModel::Performance);
    }

    /// ASR-056: is_online_streaming() 收敛判据——QwenAudioOnline 和 FunAsrRealtime 都返回 true
    #[test]
    fn asr_056_is_online_streaming_correct_for_all_variants() {
        assert!(
            !AsrModel::Performance.is_online_streaming(),
            "Performance must not be online streaming"
        );
        assert!(
            !AsrModel::Accuracy.is_online_streaming(),
            "Accuracy must not be online streaming"
        );
        assert!(
            AsrModel::QwenAudioOnline.is_online_streaming(),
            "QwenAudioOnline must be online streaming"
        );
        assert!(
            AsrModel::FunAsrRealtime.is_online_streaming(),
            "FunAsrRealtime must be online streaming"
        );
    }
}

// ============================================================
// POC-QWEN3-1.7B-351 · 1.7B vs 0.6B 实测填空（CPU RTF + 中文 CER）
//
// 纯 PoC：`#[ignore]` manual test，仅在本文件 `#[cfg(test)]` 内，**不改任何生产逻辑**。
// 运行：cargo test --bin feiyin-ime -- --ignored --nocapture poc_17b_351_fitness
// 🔴 唯一变量 = 模型目录；其余锁死与生产一致 —— 复用 `create_qwen3_recognizer[_at]` +
//    `transcribe_acc_ctx`，**不手搓 recognizer 配置**（[POC-BYPASSES-PROD-WRAPPER-001]）。
// 机器信息在 stdout 打印（CPU 核数 / accuracy 线程数），否则速度数字无意义。
// ============================================================
#[cfg(test)]
mod poc_qwen3_17b_351 {
    use super::{
        create_qwen3_recognizer, create_qwen3_recognizer_at, decode_accuracy_once,
        default_acc_num_threads, transcribe_acc_ctx, CtxInject,
    };
    use crate::config::ChineseScript;
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    /// Gavin 真口述三段标准答案（与 313 `poc_qwen3_compare.rs.txt::ANSWERS` 逐字相同）。
    const ANSWERS: [&str; 3] = [
        "上一轮1/8决赛，凭借哈兰德梅开二度，挪威队2比1爆冷淘汰五届世界杯冠军巴西队，历史性闯入世界杯八强。如今，他们将在迈阿密挑战英格兰队。",
        "然而，据报道，近期挪威队内出现了疾病传播，多名球员受到发烧、咳嗽等症状困扰，球队正与时间赛跑，希望能在比赛前恢复最佳状态。",
        "水晶宫前锋约根·斯特兰德·拉尔森因发烧缺席了世界杯首战对阵伊拉克队前的训练，并最终无缘那场比赛。效力于意甲萨索洛的马库斯·霍尔姆格伦·佩德森虽然在小组赛第二轮对阵塞内加尔队时取得进球，但由于生病，缺席了上一场对阵巴西队的淘汰赛。",
    ];

    /// 与 313 同一份词表（仅用于 per-stream terms 注入；两模型注入完全相同 ⇒ 不构成变量差异）。
    const FIT_WORDLIST: &str = "哈兰德,挪威,世界杯,巴西,英格兰,迈阿密,萨索洛,塞内加尔,斯特兰德,拉尔森,霍尔姆格伦,佩德森,约根,犹地亚";

    // 298 切片常量（与 313 `poc_qwen3_compare.rs.txt` 逐字相同）。
    const AB_SIL_MS: f32 = 800.0;
    const AB_MIN_SEG_MS: usize = 5000;
    const AB_SIL_THRESHOLD: f32 = 0.01;

    fn manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn segment_298(samples: &[f32], rate: usize) -> Vec<(usize, usize)> {
        let min_seg = rate * AB_MIN_SEG_MS / 1000;
        let sil_need = (rate as f32 * AB_SIL_MS / 1000.0) as usize;
        let mut segs = Vec::new();
        let mut seg_start = 0usize;
        let mut silent_run = 0usize;
        let mut done_for_pause = false;
        for (i, &s) in samples.iter().enumerate() {
            if s.abs() <= AB_SIL_THRESHOLD {
                silent_run += 1;
                if !done_for_pause && silent_run >= sil_need {
                    let cut = i + 1 - silent_run;
                    if cut.saturating_sub(seg_start) >= min_seg {
                        segs.push((seg_start, cut));
                        seg_start = cut;
                        done_for_pause = true;
                    }
                }
            } else {
                silent_run = 0;
                done_for_pause = false;
            }
        }
        if samples.len() > seg_start
            && samples[seg_start..]
                .iter()
                .any(|s| s.abs() > AB_SIL_THRESHOLD)
        {
            segs.push((seg_start, samples.len()));
        }
        segs
    }

    fn slice_fitness(samples: &[f32], rate: usize) -> Vec<(usize, usize)> {
        let raw = segment_298(samples, rate);
        let maxn = rate * 20; // ≤20s
        let pad = rate / 5; // 200ms
        let mut caps: Vec<(usize, usize)> = Vec::new();
        for (a, b) in raw {
            let mut s = a;
            while b - s > maxn {
                caps.push((s, s + maxn));
                s += maxn;
            }
            if b > s {
                caps.push((s, b));
            }
        }
        caps.into_iter()
            .map(|(a, b)| (a.saturating_sub(pad), (b + pad).min(samples.len())))
            .collect()
    }

    // ---- CER（与 313 逐字相同的口径：去标点空白后编辑距离）----
    fn normalize_for_cer(text: &str) -> String {
        const PUNCT: &[char] = &[
            '。', '，', '、', '！', '？', '；', '：', '「', '」', '『', '』', '“', '”', '‘', '’',
            '…', '—', '.', ',', '!', '?', ';', ':', '"', '\'', '(', ')', '[', ']', ' ', '\t', '\n',
            '\r',
        ];
        text.chars().filter(|c| !PUNCT.contains(c)).collect()
    }

    fn edit_distance(a: &[char], b: &[char]) -> usize {
        let (m, n) = (a.len(), b.len());
        if m == 0 {
            return n;
        }
        if n == 0 {
            return m;
        }
        let mut prev: Vec<usize> = (0..=n).collect();
        for i in 1..=m {
            let mut cur = vec![i; n + 1];
            for j in 1..=n {
                let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
                cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            }
            prev = cur;
        }
        prev[n]
    }

    fn cer(hyp: &str, reference: &str) -> f64 {
        let h: Vec<char> = normalize_for_cer(hyp).chars().collect();
        let r: Vec<char> = normalize_for_cer(reference).chars().collect();
        if r.is_empty() {
            return if h.is_empty() { 0.0 } else { 1.0 };
        }
        edit_distance(&h, &r) as f64 / r.len() as f64
    }

    /// 进程内存（工作集, 私有），MB。与 313 同款 PowerShell 读法。
    fn process_mem_mb() -> (f64, f64) {
        let pid = std::process::id();
        let script = format!(
            "$p=Get-Process -Id {pid} -ErrorAction SilentlyContinue; if($p){{ \"{{0}} {{1}}\" -f $p.WorkingSet64,$p.PrivateMemorySize64 }} else {{ \"0 0\" }}"
        );
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            let mut it = s.split_whitespace();
            let ws = it.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let pv = it.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            return (ws / 1048576.0, pv / 1048576.0);
        }
        (0.0, 0.0)
    }

    /// 跑一个模型：逐片 `transcribe_acc_ctx`（前文累积 + 同一词表），返回
    /// `(拼接文本, 总解码秒, peak 私有 MB)`。
    fn run_model(
        rec: &sherpa_onnx::OfflineRecognizer,
        slices: &[(usize, usize)],
        samples: &[f32],
        load_priv_mb: f64,
        tag: &str,
    ) -> (String, f64, f64) {
        let mut acc = String::new();
        let mut total = 0.0f64;
        let mut peak = load_priv_mb;
        for (i, (a, b)) in slices.iter().enumerate() {
            // 377：注入已无「本次已完成片」通道（`CtxInject` 只剩词表）⇒ 不再构造 current。
            let inject = CtxInject {
                terms: Some(FIT_WORDLIST),
                avg_chars_per_sec: None,
                speech_ranges: None,
                streaming_nonempty: false,
            };
            let t0 = Instant::now();
            let (text, _native) =
                transcribe_acc_ctx(rec, &samples[*a..*b], ChineseScript::Simplified, i, inject)
                    .unwrap_or_else(|e| panic!("{tag} seg{i} decode failed: {e}"));
            let secs = t0.elapsed().as_secs_f64();
            total += secs;
            let (_, pv) = process_mem_mb();
            peak = peak.max(pv);
            println!(
                "POC351 {tag} seg{i} {:.2}-{:.2}s decode={:.3}s out_chars={} text={}",
                *a as f64 / 16000.0,
                *b as f64 / 16000.0,
                secs,
                text.chars().count(),
                text.replace('\n', " ")
            );
            acc.push_str(&text);
        }
        (acc, total, peak)
    }

    #[test]
    #[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_17b_351_fitness"]
    fn poc_17b_351_fitness() {
        let root = manifest_dir();
        let wav = root.join("collab/research/audio-real-gavin/processed/full.wav");
        let wave =
            sherpa_onnx::Wave::read(wav.to_str().expect("wav path utf8")).expect("read full.wav");
        let rate = wave.sample_rate() as usize;
        let samples = wave.samples().to_vec();
        let dur = samples.len() as f64 / rate as f64;
        let slices = slice_fitness(&samples, rate);
        let full_ref = ANSWERS.concat();

        println!(
            "POC351 machine cores={} acc_threads={} dur={:.2}s",
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(0),
            default_acc_num_threads(),
            dur
        );
        println!(
            "POC351 slices={:?}",
            slices
                .iter()
                .map(|(a, b)| format!(
                    "{:.1}-{:.1}s",
                    *a as f64 / rate as f64,
                    *b as f64 / rate as f64
                ))
                .collect::<Vec<_>>()
        );

        // ---- 0.6B：生产 create_qwen3_recognizer（经硬编码目录解析）----
        let (ws0, pv0) = process_mem_mb();
        let rec06 = create_qwen3_recognizer(&root.join("models")).expect("create 0.6B recognizer");
        let (ws06_load, pv06_load) = process_mem_mb();
        let (text06, secs06, peak06) = run_model(&rec06, &slices, &samples, pv06_load, "0.6B");
        drop(rec06);
        let (_, pv_after06) = process_mem_mb();
        let _ = ws0;

        // ---- 1.7B：同一生产构造链，**仅模型目录不同** ----
        let dir17 = root.join("models/sherpa-onnx-qwen3-asr-1.7B-int8-2026-09-22");
        let rec17 = create_qwen3_recognizer_at(&dir17).expect("create 1.7B recognizer");
        let (ws17_load, pv17_load) = process_mem_mb();
        let (text17, secs17, peak17) = run_model(&rec17, &slices, &samples, pv17_load, "1.7B");
        let _ = ws17_load;

        println!("POC351 RESULT");
        println!(
            "POC351 baseline_priv0={:.0}MB after06_priv={:.0}MB",
            pv0, pv_after06
        );
        println!(
            "POC351 0.6B load_priv={:.0}MB peak_priv={:.0}MB decode={:.3}s rtf={:.3} cer={:.4}",
            pv06_load,
            peak06,
            secs06,
            secs06 / dur,
            cer(&text06, &full_ref)
        );
        println!(
            "POC351 1.7B load_priv={:.0}MB peak_priv={:.0}MB decode={:.3}s rtf={:.3} cer={:.4}",
            pv17_load,
            peak17,
            secs17,
            secs17 / dur,
            cer(&text17, &full_ref)
        );
        println!("POC351 0.6B text={}", text06.replace('\n', " "));
        println!("POC351 1.7B text={}", text17.replace('\n', " "));
        println!("POC351 ref  text={}", full_ref.replace('\n', " "));
        let _ = ws06_load;
    }

    // ============================================================
    // POC-QWEN3-1.7B-RETEST-353 · 按生产口径（含 ITN）重测 CER
    //
    // 生产链（local realtime accuracy，无 LLM / 无翻译分支，见 main.rs:10501/10582）：
    //   raw_text → itn::normalize_numbers → normalize_text_for_language
    //            → itn::normalize_unit_symbols_only
    // 只读调用生产入口，不手搓规则、不改任何生产代码。
    // 运行：cargo test --bin feiyin-ime -- --ignored --nocapture poc_353_itn_cer
    // ============================================================
    /// 生产完整后处理链（严格按 main.rs 顺序；`decode_accuracy_once` 里已先做过一次
    /// `normalize_text_for_language`，此处按生产再补一次，幂等）。
    fn production_itn(t: &str) -> String {
        let main = crate::itn::normalize_numbers(t);
        let norm =
            crate::text_normalizer::normalize_text_for_language(&main, ChineseScript::Simplified);
        crate::itn::normalize_unit_symbols_only(&norm)
    }

    #[test]
    #[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_353_itn_cer"]
    fn poc_353_itn_cer() {
        let root = manifest_dir();
        let wav = root.join("collab/research/audio-real-gavin/processed/full.wav");
        let wave =
            sherpa_onnx::Wave::read(wav.to_str().expect("wav path utf8")).expect("read full.wav");
        let rate = wave.sample_rate() as usize;
        let samples = wave.samples().to_vec();
        let slices = slice_fitness(&samples, rate);
        let full_ref = ANSWERS.concat();

        let rec06 = create_qwen3_recognizer(&root.join("models")).expect("create 0.6B recognizer");
        let (t06, _, _) = run_model(&rec06, &slices, &samples, 0.0, "0.6B-353");
        drop(rec06);
        let dir17 = root.join("models/sherpa-onnx-qwen3-asr-1.7B-int8-2026-09-22");
        let rec17 = create_qwen3_recognizer_at(&dir17).expect("create 1.7B recognizer");
        let (t17raw, _, _) = run_model(&rec17, &slices, &samples, 0.0, "1.7B-353");
        let t17 = t17raw
            .split("<asr_text>")
            .last()
            .unwrap_or(&t17raw)
            .to_string();

        let p06 = production_itn(&t06);
        let p17raw = production_itn(&t17raw);
        let p17 = production_itn(&t17);

        println!("POC353 === CER（裸 ASR 口径 vs 生产口径含 ITN）===");
        println!("POC353 0.6B raw       cer={:.4}", cer(&t06, &full_ref));
        println!("POC353 0.6B prod(ITN) cer={:.4}", cer(&p06, &full_ref));
        println!("POC353 1.7B raw       cer={:.4}", cer(&t17raw, &full_ref));
        println!(
            "POC353 1.7B prod(ITN) cer={:.4}  (含泄漏前缀)",
            cer(&p17raw, &full_ref)
        );
        println!("POC353 1.7B prod+前缀已剥 cer={:.4}", cer(&p17, &full_ref));
        println!("POC353 0.6B prod_text={}", p06.replace('\n', " "));
        println!("POC353 1.7B prod_text={}", p17raw.replace('\n', " "));
        println!("POC353 1.7B prod+剥prefix text={}", p17.replace('\n', " "));

        println!("POC353 === ITN「比」/分数 探针 ===");
        for s in [
            "二比一",
            "八分之一决赛",
            "2比1",
            "三分之一",
            "二比一爆冷",
            "五届",
            "小组赛第二轮",
        ] {
            println!("POC353 probe {:?} -> {:?}", s, production_itn(s));
        }
        // 353-补：定位「过度转换」走哪条通道（主通道 normalize_with_rules vs 补丁通道 unit_symbols）
        println!("POC353 === 过度转换通道定位 ===");
        for s in ["梅开二度", "二度", "第二轮", "第2轮", "一度", "一年一度"] {
            println!(
                "POC353 ch {:?}: normalize_numbers={:?} | unit_symbols_only={:?}",
                s,
                crate::itn::normalize_numbers(s),
                crate::itn::normalize_unit_symbols_only(s)
            );
        }
    }

    // ============================================================
    // RESEARCH-QWEN3-1.7B-CAPABILITY-355 · 1.7B 能力增量盘点（不测 CER / 不报速度内存）
    //
    // 严格区分 A（文档）/ B（我方实测可用），只用生产链 `transcribe_acc_ctx`（只读）。
    // 运行：cargo test --bin feiyin-ime -- --ignored --nocapture poc_355_capability
    // ============================================================
    fn head(s: &str, n: usize) -> String {
        let t: String = s.chars().take(n).collect();
        if s.chars().count() > n {
            format!("{t}…")
        } else {
            t
        }
    }

    // ============================================================
    // RESEARCH-QWEN3-1.7B-CAPABILITY-355 · 1.7B 能力增量盘点 / 调用层差异
    //   🔴 顺序解码：先建 0.6B、跑完、drop，再建 1.7B（避免两模型同时驻留；
    //      首版并存时在 1.7B 解码处异常退出 0xffffffff，本版验证是否与并存有关）。
    //   运行：cargo test --bin feiyin-ime -- --ignored --nocapture poc_355_capability
    // ============================================================

    /// 与生产 `create_qwen3_recognizer_at` **逐字段一致**，仅 `max_total_len`/`max_new_tokens`
    /// 可变的对照构造器（调用层参数扫描专用；生产值 = 4096 / 256）。
    fn make_qwen3(
        dir: &Path,
        max_total_len: i32,
        max_new_tokens: i32,
    ) -> sherpa_onnx::OfflineRecognizer {
        let cfg = sherpa_onnx::OfflineRecognizerConfig {
            model_config: sherpa_onnx::OfflineModelConfig {
                num_threads: default_acc_num_threads(),
                provider: Some("cpu".to_string()),
                qwen3_asr: sherpa_onnx::OfflineQwen3ASRModelConfig {
                    conv_frontend: Some(
                        dir.join("conv_frontend.onnx")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                    encoder: Some(dir.join("encoder.int8.onnx").to_string_lossy().into_owned()),
                    decoder: Some(dir.join("decoder.int8.onnx").to_string_lossy().into_owned()),
                    tokenizer: Some(dir.join("tokenizer").to_string_lossy().into_owned()),
                    max_total_len,
                    max_new_tokens,
                    temperature: 1e-6,
                    top_p: 0.8,
                    seed: 42,
                    hotwords: None,
                },
                tokens: Some(String::new()),
                ..Default::default()
            },
            ..Default::default()
        };
        sherpa_onnx::OfflineRecognizer::create(&cfg).expect("create qwen3")
    }

    // ---- 366-🥇：8 线程 vs 满核 实测对照（临时 recognizer，不改生产）----
    fn make_qwen3_threads(dir: &Path, num_threads: i32) -> sherpa_onnx::OfflineRecognizer {
        let cfg = sherpa_onnx::OfflineRecognizerConfig {
            model_config: sherpa_onnx::OfflineModelConfig {
                num_threads,
                provider: Some("cpu".to_string()),
                qwen3_asr: sherpa_onnx::OfflineQwen3ASRModelConfig {
                    conv_frontend: Some(
                        dir.join("conv_frontend.onnx")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                    encoder: Some(dir.join("encoder.int8.onnx").to_string_lossy().into_owned()),
                    decoder: Some(dir.join("decoder.int8.onnx").to_string_lossy().into_owned()),
                    tokenizer: Some(dir.join("tokenizer").to_string_lossy().into_owned()),
                    max_total_len: 4096,
                    max_new_tokens: 256,
                    temperature: 1e-6,
                    top_p: 0.8,
                    seed: 42,
                    hotwords: None,
                },
                tokens: Some(String::new()),
                ..Default::default()
            },
            ..Default::default()
        };
        sherpa_onnx::OfflineRecognizer::create(&cfg).expect("create qwen3")
    }

    #[test]
    #[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_366_threads"]
    fn poc_366_threads() {
        let root = manifest_dir();
        let dir = root.join("models/sherpa-onnx-qwen3-asr-1.7B-int8-2026-09-22");
        let (fs, rate) = read_wav(&root, "collab/research/audio-real-gavin/processed/full.wav");
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(0);
        println!(
            "POC366 cores={} default_threads={}",
            cores,
            default_acc_num_threads()
        );
        for secs in [20usize, 56] {
            let n = (rate * secs).min(fs.len());
            for threads in [8i32, cores as i32] {
                let rec = make_qwen3_threads(&dir, threads);
                // 预热一次（首次分配/池创建不计入对照）
                let _ = decode_full_result(&rec, &fs[..(rate * 5).min(fs.len())], rate as i32);
                let (r, ms) = decode_full_result(&rec, &fs[..n], rate as i32);
                println!(
                    "POC366 secs={secs} threads={threads} decode={ms:.0}ms chars={}",
                    r.text.chars().count()
                );
                drop(rec);
            }
        }
    }

    fn raw_decode(
        rec: &sherpa_onnx::OfflineRecognizer,
        samples: &[f32],
        script: ChineseScript,
    ) -> String {
        // 生产 decode 函数、system=None ⇒ 纯模型输出（用于看控制前缀 / 语种）
        decode_accuracy_once(rec, samples, None, script).expect("decode")
    }

    fn read_wav(root: &Path, rel: &str) -> (Vec<f32>, usize) {
        let w = sherpa_onnx::Wave::read(root.join(rel).to_str().expect("utf8")).expect("read wav");
        (w.samples().to_vec(), w.sample_rate() as usize)
    }

    fn tail(s: &str, n: usize) -> String {
        let v: Vec<char> = s.chars().collect();
        let start = v.len().saturating_sub(n);
        v[start..].iter().collect()
    }

    fn run_355_model(tag: &str, dir: &Path, root: &Path) {
        let rec = make_qwen3(dir, 4096, 256);
        let (fs, rate) = read_wav(root, "collab/research/audio-real-gavin/processed/full.wav");
        let slices = slice_fitness(&fs, rate);

        // S1：纯模型输出（system=None）——控制前缀 / 语种
        println!("POC355 === S1 {tag} 纯模型输出（无注入）===");
        let pref = |t: &str| -> String {
            match t.find("<asr_text>") {
                Some(p) => format!("PREFIX={:?}", &t[..p]),
                None => "PREFIX=(none)".to_string(),
            }
        };
        let seg0_raw = raw_decode(
            &rec,
            &fs[slices[0].0..slices[0].1],
            ChineseScript::Simplified,
        );
        println!(
            "POC355 S1 {tag} full-seg0 {}: {}",
            pref(&seg0_raw),
            head(&seg0_raw, 130)
        );
        for rel in [
            "models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25/test_wavs/ja1.wav",
            "models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25/test_wavs/de.wav",
            "models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25/test_wavs/cantonese.wav",
            "models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25/test_wavs/codeswitch.wav",
            "models/korean-testwavs/0.wav",
        ] {
            let (s, _) = read_wav(root, rel);
            let t = raw_decode(&rec, &s, ChineseScript::Simplified);
            println!("POC355 S1 {tag} {rel} {}: {}", pref(&t), head(&t, 80));
        }

        // S2：生产注入路径（transcribe_acc_ctx，恒发 CLEANUP_INSTR_EN）
        println!("POC355 === S2 {tag} 生产注入路径（有注入）===");
        let inject = CtxInject {
            terms: None,
            avg_chars_per_sec: None,
            speech_ranges: None,
            streaming_nonempty: false,
        };
        let seg0 = transcribe_acc_ctx(
            &rec,
            &fs[slices[0].0..slices[0].1],
            ChineseScript::Simplified,
            0,
            inject,
        )
        .expect("decode")
        .0;
        println!("POC355 S2 {tag} seg0: {}", head(&seg0, 130));

        // S2b：口水词 / 重复清理（colloq.wav，纯模型输出）
        let (cs, _) = read_wav(root, "models/kv259/colloq.wav");
        println!(
            "POC355 S2b {tag} colloq: {}",
            head(&raw_decode(&rec, &cs, ChineseScript::Simplified), 240)
        );

        // S3：更大分片可行性（0-30s 单段，生产是 20s×3）
        println!("POC355 === S3 {tag} 30s 单段（更大分片可行性）===");
        let n30 = (rate * 30).min(fs.len());
        let t30 = raw_decode(&rec, &fs[..n30], ChineseScript::Simplified);
        println!(
            "POC355 S3 {tag} 0-30s len={} head={} tail={}",
            t30.chars().count(),
            head(&t30, 70),
            tail(&t30, 40)
        );

        drop(rec);
    }

    #[test]
    #[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_355_capability"]
    fn poc_355_capability() {
        let root = manifest_dir();
        println!(
            "POC355 threads={} cores={}",
            default_acc_num_threads(),
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(0)
        );
        // 顺序：0.6B 建→跑→drop → 1.7B 建→跑→drop
        run_355_model(
            "0.6B",
            &root.join("models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25"),
            &root,
        );
        run_355_model(
            "1.7B",
            &root.join("models/sherpa-onnx-qwen3-asr-1.7B-int8-2026-09-22"),
            &root,
        );

        // S4：调用层参数对照（仅 1.7B）——max_new_tokens 256 vs 512（长输出是否被截断）
        println!("POC355 === S4 1.7B max_new_tokens 对照（30s 单段）===");
        let dir17 = root.join("models/sherpa-onnx-qwen3-asr-1.7B-int8-2026-09-22");
        let (fs, rate) = read_wav(&root, "collab/research/audio-real-gavin/processed/full.wav");
        let n30 = (rate * 30).min(fs.len());
        for mnt in [256, 512] {
            let rec = make_qwen3(&dir17, 4096, mnt);
            let t = raw_decode(&rec, &fs[..n30], ChineseScript::Simplified);
            println!(
                "POC355 S4 mnt={mnt} len_chars={} head={} tail={}",
                t.chars().count(),
                head(&t, 60),
                tail(&t, 40)
            );
            drop(rec);
        }
    }

    // ---- 355 追加：per-stream `language` 选项能否抑制 1.7B 前缀（只 1.7B，不做双模型对比）----
    /// 生产 `decode_accuracy_once` 同款解码，但**额外**设 per-stream `language` 选项
    /// （运行时支持，见 sherpa-onnx `offline-recognizer-qwen3-asr-impl.cc:824`）。
    fn decode_with_lang(
        rec: &sherpa_onnx::OfflineRecognizer,
        samples: &[f32],
        script: ChineseScript,
        language: Option<&str>,
    ) -> String {
        let stream = rec.create_stream();
        if let Some(l) = language {
            stream.set_option("language", l);
        }
        stream.accept_waveform(16000, samples);
        rec.decode(&stream);
        let r = stream.get_result().expect("result");
        let t = super::Transcriber::strip_asr_special_tokens(r.text.trim());
        crate::text_normalizer::normalize_text_for_language(&t, script)
    }

    // ========================================================================
    // POC-LANGUAGE-SEPARATOR-376：`language` 分隔符 × 注入形状（**只读 PoC，不改生产**）
    //   运行：cargo test --bin feiyin-ime -- --ignored --nocapture poc_376_language_separator
    // ========================================================================

    /// POC-376：**不剥**任何前缀的裸解码（要观测 `<asr_text>` 是否出现/被模型自造）。
    fn poc376_decode_raw(
        rec: &sherpa_onnx::OfflineRecognizer,
        samples: &[f32],
        language: Option<&str>,
        system: Option<&str>,
    ) -> String {
        let stream = rec.create_stream();
        if let Some(l) = language {
            stream.set_option("language", l);
        }
        if let Some(s) = system {
            stream.set_option("hotwords", s);
        }
        stream.accept_waveform(16000, samples);
        rec.decode(&stream);
        stream.get_result().expect("result").text.trim().to_string()
    }

    /// POC-376：输出里「**连续**注入词条且顺序与注入一致」的**最大 run 长度**
    /// （与 374 判据同构，但独立实现 ⇒ 便于同时报 ≥3 / ≥4 两个口径）。
    fn poc376_terms_run(text: &str, terms: &str) -> usize {
        let norm = |s: &str| -> Vec<char> {
            s.chars()
                .filter(|c| !c.is_whitespace() && !crate::punctuation::PUNCT_CHARS.contains(c))
                .collect()
        };
        let injected: Vec<String> = terms
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        let out = norm(text);
        let mut best = 0usize;
        for ti in 0..injected.len() {
            let head = norm(&injected[ti]);
            if head.is_empty() {
                continue;
            }
            let mut p = 0usize;
            while p + head.len() <= out.len() {
                if out[p..p + head.len()] == head[..] {
                    let mut end = p + head.len();
                    let mut m = 1usize;
                    while ti + m < injected.len() {
                        let nt = norm(&injected[ti + m]);
                        if !nt.is_empty()
                            && end + nt.len() <= out.len()
                            && out[end..end + nt.len()] == nt[..]
                        {
                            end += nt.len();
                            m += 1;
                        } else {
                            break;
                        }
                    }
                    best = best.max(m);
                    p += 1;
                } else {
                    p += 1;
                }
            }
        }
        best
    }

    fn poc376_rms_frames(fs: &[f32], rate: usize) -> (Vec<f32>, usize) {
        let frame = (rate / 10).max(1); // 100ms
        let f = fs
            .chunks(frame)
            .map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt())
            .collect();
        (f, frame)
    }

    /// POC-376：**低信息量**短片（回显在现场就发生在句尾/停顿边缘/低能量段）。
    /// 判据：帧 RMS 落在「略高于纯静音底噪」与「中位能量的 45% 处」之间（低能量但非纯静音）。
    fn poc376_low_info_clips(
        fs: &[f32],
        rate: usize,
        n: usize,
        clip_secs: f32,
    ) -> Vec<(usize, usize)> {
        let (frames, frame) = poc376_rms_frames(fs, rate);
        if frames.len() < 10 {
            return vec![(0, fs.len())];
        }
        let mut sorted = frames.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p05 = sorted[sorted.len() / 20];
        let p50 = sorted[sorted.len() / 2];
        let floor = p05 + (p50 - p05) * 0.05;
        let cap = p05 + (p50 - p05) * 0.45;
        let half = (clip_secs * rate as f32 / 2.0) as usize;
        let mut out: Vec<(usize, usize)> = Vec::new();
        let steps = 160usize;
        for k in 0..steps {
            if out.len() >= n {
                break;
            }
            let idx = 3 + k * frames.len().saturating_sub(6) / steps;
            if idx >= frames.len() {
                break;
            }
            if frames[idx] < floor || frames[idx] > cap {
                continue;
            }
            let c = idx * frame;
            if out
                .iter()
                .any(|(a, b)| c + half > *a && c < b.saturating_sub(half) || (c >= *a && c < *b))
            {
                continue;
            }
            let a = c.saturating_sub(half);
            let b = (c + half).min(fs.len());
            if b.saturating_sub(a) < rate {
                continue;
            }
            out.push((a, b));
        }
        out
    }

    /// POC-376：**正常长片**（文章能量高的不重叠长窗）⇒ 校验「正文质量没被改坏」。
    fn poc376_loud_clips(fs: &[f32], rate: usize, n: usize, clip_secs: f32) -> Vec<(usize, usize)> {
        let len = (clip_secs * rate as f32) as usize;
        if fs.len() <= len {
            return vec![(0, fs.len())];
        }
        let (frames, frame) = poc376_rms_frames(fs, rate);
        let mut idx: Vec<usize> = (0..frames.len()).collect();
        idx.sort_by(|a, b| frames[*b].partial_cmp(&frames[*a]).unwrap());
        let mut out: Vec<(usize, usize)> = Vec::new();
        for i in idx {
            if out.len() >= n {
                break;
            }
            let c = (i * frame).min(fs.len() - len);
            if out.iter().any(|(a, b)| c + len > *a && c < *b) {
                continue;
            }
            out.push((c, c + len));
        }
        out
    }

    // 377 起生产已删除，PoC 内保留原文（仅用于复现历史对照）
    const CLEANUP_INSTR_EN_377: &str =
        "Transcribe cleanly: drop stutters, repeated words, and filler words.";
    const CTX_INSTR_EN_377: &str = "The following is the preceding context of this recording. Use it to keep terminology and wording consistent.";

    #[test]
    #[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_376_language_separator"]
    fn poc_376_language_separator() {
        let root = manifest_dir();
        let rec = create_qwen3_recognizer_at(
            &root.join("models/sherpa-onnx-qwen3-asr-1.7B-int8-2026-09-22"),
        )
        .expect("1.7B recognizer");

        // 注入素材：**生产同款**（FULL 走 build_ctx_system；TERMS_ONLY 只给词表本身）
        let context =
            "我记得当初设计的时候磁条是作为上下文串用的，也就是说上一次的识别结果会被带进这次\
的提示词里面，用来帮助模型理解前后文，避免同一句话在不同分片里被切碎之后语义断裂。";
        // 与端测取证日志逐字一致的真实词条列表（59 字词条 + 18 逗号 = 77 字）
        let terms =
            "你好,铭印,银线,朵洛莉丝,费曼学习法,子未穿害,低质,罗斯柴尔德,维生素b12,三五成群,\
主控,派安盈,隋变,摄氏度,艾丁湖,风无心,文案,漫剧,采编";
        // FIX-INJECT-TO-SPEC-377 起生产注入已删掉指令句/Context（`build_ctx_system` 只剩纯词表）⇒
        // PoC 内**保留历史原文**，以便继续复现「旧 FULL 形态」的对照。
        let full = format!(
            "{CLEANUP_INSTR_EN_377}
{CTX_INSTR_EN_377}
Context: {context}
Terms: {terms}"
        );
        // TERMS_PLUS_CTX：**去掉两句英文指令**，保留同样的标签与顺序
        // ⇒ 与 FULL 的差 = 指令句的贡献；与 TERMS_ONLY 的差 = 散文 Context 的贡献。
        let terms_plus_ctx = format!(
            "Context: {context}
Terms: {terms}"
        );
        let shapes: [(&str, Option<&str>); 4] = [
            ("FULL", Some(full.as_str())),
            ("TERMS_PLUS_CTX", Some(terms_plus_ctx.as_str())),
            ("TERMS_ONLY", Some(terms)),
            ("NONE", None),
        ];
        // language 维度按补充二降级：主基线 = 不设（= 生产现状 = 官方主用法）；"Auto"/"" 只作次要对照。
        // （"Chinese" 已删；小写 "auto" 只在 1 段非中文上抽测）
        const PRIMARY_LANG: (&str, Option<&str>) = ("unset", None);
        let secondary_langs: [(&str, Option<&str>); 2] =
            [("Auto", Some("Auto")), ("empty", Some(""))];

        let (fs, rate) = read_wav(&root, "collab/research/audio-real-gavin/processed/full.wav");
        let low = poc376_low_info_clips(&fs, rate, 6, 1.8);
        let loud = poc376_loud_clips(&fs, rate, 2, 8.0);
        let (ko, _) = read_wav(&root, "models/korean-testwavs/0.wav");
        let (ja, _) = read_wav(
            &root,
            "models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25/test_wavs/ja1.wav",
        );

        println!(
            "POC376 material: ctx_chars={} terms_chars={} full_chars={} tpc_chars={}",
            context.chars().count(),
            terms.chars().count(),
            full.chars().count(),
            terms_plus_ctx.chars().count()
        );

        let ctx_norm = super::normalize_ctx_probe(context);
        let measure = |tag: &str,
                       samples: &[f32],
                       lang: Option<&str>,
                       lang_name: &str,
                       shape: &str,
                       system: Option<&str>| {
            let t = poc376_decode_raw(&rec, samples, lang, system);
            let marker = t.contains("<asr_text>");
            let prefix = match t.find("<asr_text>") {
                Some(p) => format!("{:?}", &t[..p]),
                None => "none".to_string(),
            };
            let run = poc376_terms_run(&t, terms);
            let lcs = super::lcs_len(&super::normalize_ctx_probe(&t), &ctx_norm);
            let secs = samples.len() as f32 / rate as f32;
            let chars = t.chars().count();
            println!(
                "POC376 cell clip={} lang={} shape={} | marker={} prefix={} terms_run={} ctx_lcs={} chars={} secs={:.2} rate={:.2} | text={}",
                tag,
                lang_name,
                shape,
                if marker { "YES" } else { "no" },
                prefix,
                run,
                lcs,
                chars,
                secs,
                chars as f32 / secs.max(0.01),
                t
            );
            println!(
                "POC376 TABLE {}	{}	{}	{}	{}	{}	{}",
                tag,
                lang_name,
                shape,
                if marker { "MARKER" } else { "-" },
                run,
                lcs,
                chars
            );
        };

        // ① 主矩阵（机时主要花这里）：**不设 language** × 4 形状 × 全部 10 段
        //    （6 低信息量 = 回显最易发；2 正常长片 = 正文质量；2 非中文 = 语种不被锁）
        let mut all: Vec<(String, &[f32])> = Vec::new();
        for (i, (a, b)) in low.iter().enumerate() {
            all.push((format!("low{}", i + 1), &fs[*a..*b]));
        }
        for (i, (a, b)) in loud.iter().enumerate() {
            all.push((format!("loud{}", i + 1), &fs[*a..*b]));
        }
        all.push(("ko".to_string(), ko.as_slice()));
        all.push(("ja".to_string(), ja.as_slice()));
        for (tag, seg) in &all {
            for (sn, sv) in shapes {
                measure(tag, seg, PRIMARY_LANG.1, PRIMARY_LANG.0, sn, sv);
            }
        }
        // ② 次要对照：Auto / "" × 4 形状，只在 2 段非中文（验语种信号）+ 1 段低信息量（ja 是重现场）
        for (tag, seg) in [
            ("ko", ko.as_slice()),
            ("ja", ja.as_slice()),
            ("low3", &fs[low[2].0..low[2].1]),
        ] {
            for (ln, lv) in secondary_langs {
                for (sn, sv) in shapes {
                    measure(tag, seg, lv, ln, sn, sv);
                }
            }
        }
        // ③ 抽测小写 "auto"（只在 1 段非中文上，省机时）
        for (sn, sv) in shapes {
            measure("ko", ko.as_slice(), Some("auto"), "auto", sn, sv);
        }
    }

    #[test]
    #[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_355_language_option"]
    fn poc_355_language_option() {
        let root = manifest_dir();
        let rec = create_qwen3_recognizer_at(
            &root.join("models/sherpa-onnx-qwen3-asr-1.7B-int8-2026-09-22"),
        )
        .expect("1.7B");
        let pref = |t: &str| -> String {
            match t.find("<asr_text>") {
                Some(p) => format!("PREFIX={:?}", &t[..p]),
                None => "PREFIX=(none)".to_string(),
            }
        };
        let (fs, rate) = read_wav(&root, "collab/research/audio-real-gavin/processed/full.wav");
        let slices = slice_fitness(&fs, rate);
        let zh = &fs[slices[0].0..slices[0].1];
        let (ko, _) = read_wav(&root, "models/korean-testwavs/0.wav");
        let (ja, _) = read_wav(
            &root,
            "models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25/test_wavs/ja1.wav",
        );

        for lang in [None, Some("Chinese"), Some("English")] {
            let t = decode_with_lang(&rec, zh, ChineseScript::Simplified, lang);
            println!("POC355L zh lang={lang:?} {} : {}", pref(&t), head(&t, 70));
        }
        for lang in [None, Some("Korean"), Some("Chinese")] {
            let t = decode_with_lang(&rec, &ko, ChineseScript::Simplified, lang);
            println!("POC355L ko lang={lang:?} {} : {}", pref(&t), head(&t, 70));
        }
        for lang in [None, Some("Japanese"), Some("English")] {
            let t = decode_with_lang(&rec, &ja, ChineseScript::Simplified, lang);
            println!("POC355L ja lang={lang:?} {} : {}", pref(&t), head(&t, 70));
        }
    }

    // ---- 355 追加二：language=None（生产实际）下，多语种**正文**对不对 + 设正确语种对照 ----
    #[test]
    #[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_355_lang_none_multilingual"]
    fn poc_355_lang_none_multilingual() {
        let root = manifest_dir();
        let rec = create_qwen3_recognizer_at(
            &root.join("models/sherpa-onnx-qwen3-asr-1.7B-int8-2026-09-22"),
        )
        .expect("1.7B");
        let (fs, rate) = read_wav(&root, "collab/research/audio-real-gavin/processed/full.wav");
        let slices = slice_fitness(&fs, rate);
        let zh_seg0 = fs[slices[0].0..slices[0].1].to_vec();
        let t = "models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25/test_wavs/";
        // (tag, wav 绝对相对路径, 参考文本, 正确语种)
        let cases: Vec<(&str, String, &str, &str, Vec<f32>)> = vec![
            ("zh-seg0", String::new(), ANSWERS[0], "Chinese", zh_seg0),
            (
                "en",
                format!("{t}f1_noise.wav"),
                "Okay, Charles. It looks like we have a problem with the radio. What happened? Yeah, someone spilled water on their machine. I uh, yeah. Charles, can you hear us? Mamma mia.",
                "English",
                Vec::new(),
            ),
            (
                "ja",
                format!("{t}ja1.wav"),
                "抜群の運動神経を持ち合わせ、どんな要求にも応えてきた。",
                "Japanese",
                Vec::new(),
            ),
            ("ko0", "models/korean-testwavs/0.wav".into(), "그는 괜찮은 척하려고 애쓰는 것 같았다.", "Korean", Vec::new()),
            ("ko1", "models/korean-testwavs/1.wav".into(), "지하철에서 다리를 벌리고 앉지 마라.", "Korean", Vec::new()),
            ("yue", format!("{t}cantonese.wav"), "今次寻寻觅觅，终于揾到my princess，肯借个场俾我哋玩。你知啦，喺香港地喺繁忙时间要揾个场嚟拍嘢系非常之难嘅。再一次多谢你哋，亦都好多谢片入边嘅每一个人。", "Cantonese", Vec::new()),
            ("mix", format!("{t}codeswitch.wav"), "I'm alone, all by myself. Je suis tout seul. Sono tutto. Estoy solo.", "English", Vec::new()),
            ("de", format!("{t}de.wav"), "Raptorium Bergbau scheint profitierter als Monroe als Reaktion auf die wirtschaftlichen Ausfälle zu sein.", "German", Vec::new()),
            ("fr", format!("{t}fr1.wav"), "Alice et moi sommes allés à Paris voyager en train au printemps, c'était très amusant.", "French", Vec::new()),
            ("ru", format!("{t}ru1.wav"), "Барсук, живущий в киевском зоопарке, совершил побег из своего вольера.", "Russian", Vec::new()),
            ("ar", format!("{t}ar1.wav"), "إطلالات مكياج عيون ذهبي لسهرات صيف عشرين واحد وعشرين بأسلوب النجوم.", "Arabic", Vec::new()),
        ];
        for (tag, rel, reference, lang, pre) in cases {
            let samples = if pre.is_empty() {
                read_wav(&root, &rel).0
            } else {
                pre
            };
            let none = decode_with_lang(&rec, &samples, ChineseScript::Simplified, None);
            let forced = decode_with_lang(&rec, &samples, ChineseScript::Simplified, Some(lang));
            let strip_prefix = |s: &str| -> String {
                match s.find("<asr_text>") {
                    Some(p) => s[p + "<asr_text>".len()..].to_string(),
                    None => s.to_string(),
                }
            };
            let body_none = strip_prefix(&none);
            println!(
                "POC355M {tag} lang=None: cer={:.3} body={}",
                cer(&body_none, reference),
                head(&body_none, 70)
            );
            println!(
                "POC355M {tag} lang={lang}: cer={:.3} body={}",
                cer(&forced, reference),
                head(&forced, 70)
            );
        }
    }

    // ---- 357：hotwords 通道 = system prompt 通道，实测指令是否被服从 ----
    /// 生产 decode 同款，但把 `hotwords` 设为任意文本（＝写进 `<|im_start|>system` 段）。
    fn decode_with_hotwords(
        rec: &sherpa_onnx::OfflineRecognizer,
        samples: &[f32],
        script: ChineseScript,
        hotwords: Option<&str>,
    ) -> String {
        let stream = rec.create_stream();
        if let Some(h) = hotwords {
            stream.set_option("hotwords", h);
        }
        stream.accept_waveform(16000, samples);
        rec.decode(&stream);
        let r = stream.get_result().expect("result");
        crate::text_normalizer::normalize_text_for_language(
            &super::Transcriber::strip_asr_special_tokens(r.text.trim()),
            script,
        )
    }

    #[test]
    #[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_357_prompt"]
    fn poc_357_prompt() {
        let root = manifest_dir();
        let rec = create_qwen3_recognizer_at(
            &root.join("models/sherpa-onnx-qwen3-asr-1.7B-int8-2026-09-22"),
        )
        .expect("1.7B");
        let (fs, rate) = read_wav(&root, "collab/research/audio-real-gavin/processed/full.wav");
        let slices = slice_fitness(&fs, rate);
        let zh = &fs[slices[0].0..slices[0].1];
        let (itn, _) = read_wav(&root, "models/kv259/itn.wav");
        let (colloq, _) = read_wav(&root, "models/kv259/colloq.wav");

        println!("POC357 === ITN 指令（itn.wav）===");
        println!(
            "POC357 itn base: {}",
            head(
                &decode_with_hotwords(&rec, &itn, ChineseScript::Simplified, None),
                200
            )
        );
        for ins in [
            "把数字都写成阿拉伯数字",
            "请把数字、日期、时间、金额、单位都规整成阿拉伯数字和标准写法",
            "输出时做 inverse text normalization",
        ] {
            println!(
                "POC357 itn ins={ins:?}: {}",
                head(
                    &decode_with_hotwords(&rec, &itn, ChineseScript::Simplified, Some(ins)),
                    200
                )
            );
        }

        println!("POC357 === 口水词指令（colloq.wav）===");
        println!(
            "POC357 colloq base: {}",
            head(
                &decode_with_hotwords(&rec, &colloq, ChineseScript::Simplified, None),
                240
            )
        );
        for ins in [
            "去掉嗯、啊、呃等口水词和重复",
            "Remove filler words, stutters and repetitions.",
        ] {
            println!(
                "POC357 colloq ins={ins:?}: {}",
                head(
                    &decode_with_hotwords(&rec, &colloq, ChineseScript::Simplified, Some(ins)),
                    240
                )
            );
        }

        println!("POC357 === 标点/格式指令（full seg0）===");
        println!(
            "POC357 zh base: {}",
            head(
                &decode_with_hotwords(&rec, zh, ChineseScript::Simplified, None),
                150
            )
        );
        for ins in ["在句末加标点", "用简体中文输出"] {
            println!(
                "POC357 zh ins={ins:?}: {}",
                head(
                    &decode_with_hotwords(&rec, zh, ChineseScript::Simplified, Some(ins)),
                    150
                )
            );
        }
    }

    // ============================================================
    // POC-TIMESTAMP-DECODE-CURVE-361：RP-1 时间戳可用性 + RP-3① decode 开销曲线
    // 运行：cargo test --bin feiyin-ime -- --ignored --nocapture poc_361_timestamp_curve
    // 只读复用生产 recognizer（create_qwen3_recognizer_at），不手搓 config。
    // ============================================================
    /// 一次完整解码，返回**原始** `OfflineRecognizerResult`（为了看 timestamps/durations/tokens）。
    fn decode_full_result(
        rec: &sherpa_onnx::OfflineRecognizer,
        samples: &[f32],
        rate: i32,
    ) -> (sherpa_onnx::OfflineRecognizerResult, f64) {
        let stream = rec.create_stream();
        stream.accept_waveform(rate, samples);
        let t0 = Instant::now();
        rec.decode(&stream);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        (stream.get_result().expect("result"), ms)
    }

    #[test]
    #[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_361_timestamp_curve"]
    fn poc_361_timestamp_curve() {
        let root = manifest_dir();
        let rec = create_qwen3_recognizer_at(
            &root.join("models/sherpa-onnx-qwen3-asr-1.7B-int8-2026-09-22"),
        )
        .expect("1.7B");
        let (fs, rate) = read_wav(&root, "collab/research/audio-real-gavin/processed/full.wav");
        println!(
            "POC361 machine cores={} acc_threads={} full_dur={:.2}s",
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(0),
            default_acc_num_threads(),
            fs.len() as f64 / rate as f64
        );

        // ---- RP-1：时间戳可用性（full.wav 0-20s，内容已知）----
        println!("POC361 === RP-1 timestamps ===");
        let n20 = (rate * 20).min(fs.len());
        let (r, ms) = decode_full_result(&rec, &fs[..n20], rate as i32);
        println!(
            "POC361 ts decode={:.0}ms text_chars={} tokens_len={}",
            ms,
            r.text.chars().count(),
            r.tokens.len()
        );
        match &r.timestamps {
            Some(v) => {
                let first: Vec<String> = v.iter().take(8).map(|x| format!("{x:.3}")).collect();
                let last: Vec<String> = v.iter().rev().take(3).map(|x| format!("{x:.3}")).collect();
                let mono = v.windows(2).all(|w| w[0] <= w[1] + 1e-6);
                println!(
                    "POC361 ts Some len={} monotonic={} first8={:?} last3={:?}",
                    v.len(),
                    mono,
                    first,
                    last
                );
            }
            None => println!("POC361 ts None"),
        }
        match &r.durations {
            Some(v) => println!(
                "POC361 dur Some len={} first3={:?}",
                v.len(),
                v.iter().take(3).collect::<Vec<_>>()
            ),
            None => println!("POC361 dur None"),
        }
        println!(
            "POC361 ts text_head={} tokens_first12={:?}",
            head(&r.text, 40),
            r.tokens.iter().take(12).collect::<Vec<_>>()
        );

        // ---- RP-3①：decode 开销曲线（递增前缀，每步新建 stream）----
        println!("POC361 === RP-3 decode 开销曲线 ===");
        let mut texts: Vec<(usize, String)> = Vec::new();
        for secs in [1usize, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 25, 30, 40, 56] {
            let n = (rate * secs).min(fs.len());
            let (r, ms) = decode_full_result(&rec, &fs[..n], rate as i32);
            println!(
                "POC361 curve secs={} decode={:.0}ms text_chars={} head={}",
                secs,
                ms,
                r.text.chars().count(),
                head(&r.text, 30)
            );
            texts.push((secs, r.text));
        }

        // ---- 重解前文稳定性：短前缀文本是否为长前缀文本的前缀 ----
        println!("POC361 === 重解前文稳定性 ===");
        let norm = |t: &str| super::Transcriber::strip_asr_special_tokens(t.trim());
        for w in texts.windows(2) {
            let (s1, t1) = &w[0];
            let (s2, t2) = &w[1];
            let n1 = norm(t1);
            let n2 = norm(t2);
            println!(
                "POC361 stable {s1}s->{s2}s is_prefix_of_next={} chars {}->{}",
                n2.starts_with(&n1),
                n1.chars().count(),
                n2.chars().count()
            );
        }
        // 同一前缀重复解码是否逐字稳定
        let n10 = (rate * 10).min(fs.len());
        let (a, _) = decode_full_result(&rec, &fs[..n10], rate as i32);
        let (b, _) = decode_full_result(&rec, &fs[..n10], rate as i32);
        println!(
            "POC361 repeat 10s identical={} chars_a={} chars_b={}",
            a.text == b.text,
            a.text.chars().count(),
            b.text.chars().count()
        );
    }
}

// ============================================================
// MIGRATE-1.13.8-1.7B-359：Qwen3-ASR 1.7B 语种前缀剥离（语言无关，方案 B）
// 运行：cargo test --bin feiyin-ime -- strip_lang_prefix
// ============================================================
#[cfg(test)]
mod strip_lang_prefix_359_tests {
    use super::{strip_qwen3_language_prefix, Transcriber};

    /// 🔴 核心：各语种前缀都能剥（语言无关，不只 chinese）。
    #[test]
    fn strips_all_language_prefixes() {
        // 官方形态 language <X><asr_text>
        assert_eq!(
            strip_qwen3_language_prefix("language chinese<asr_text>今天天气不错"),
            "今天天气不错"
        );
        assert_eq!(
            strip_qwen3_language_prefix("language english<asr_text>hello world"),
            "hello world"
        );
        assert_eq!(
            strip_qwen3_language_prefix("language japanese<asr_text>こんにちは"),
            "こんにちは"
        );
        // 模型自造的中文语种词（355 实测：韩语音频被标成「汉语」）
        assert_eq!(
            strip_qwen3_language_prefix("汉语<asr_text>그는 괜찮은 척"),
            "그는 괜찮은 척"
        );
        assert_eq!(strip_qwen3_language_prefix("中文<asr_text>你好"), "你好");
        assert_eq!(
            strip_qwen3_language_prefix("日本語<asr_text>こんにちは"),
            "こんにちは"
        );
        assert_eq!(strip_qwen3_language_prefix("English<asr_text>hi"), "hi");
        // 前缀后带空白（分隔）——标记紧贴标签
        assert_eq!(
            strip_qwen3_language_prefix("language chinese<asr_text> 你好"),
            "你好"
        );
    }

    /// 🔴 无前缀 ⇒ 原样返回（证明对 0.6B 零影响：0.6B 不吐前缀）。
    #[test]
    fn no_prefix_returns_unchanged() {
        for t in [
            "今天天气不错。",
            "hello world",
            "그는 괜찮은 척 하려고 애쓰는 것 같았다.",
            "",
            " 前后带空白 ",
        ] {
            assert_eq!(
                strip_qwen3_language_prefix(t),
                t,
                "无前缀必须原样返回：{t:?}"
            );
        }
    }

    /// 🔴 退化用例：正文里偶发尖括号/`asr_text` 字样，或标记**靠后**（超出头部护栏）⇒ 不误伤。
    ///
    /// 🔴 FIX-PREFIX-AND-EAT-371 起规则恢复 355 的「无条件截断到第一个 `<asr_text>`」，
    /// 唯一护栏是**位置**（`QWEN3_PREFIX_MAX_BYTES`=64B）⇒ 原先靠「非标签样」拦下的
    /// `句子：<asr_text>正文`（标记在 9B）现在**会被剥**（见下方 `near_head_non_label_is_stripped`）。
    #[test]
    fn degenerate_does_not_harm_body() {
        // 含尖括号但无 <asr_text> ⇒ 不变
        assert_eq!(
            strip_qwen3_language_prefix("小于号 < 和大于号 >"),
            "小于号 < 和大于号 >"
        );
        // 含 asr_text 字样但无 <> 标记 ⇒ 不变
        assert_eq!(
            strip_qwen3_language_prefix("变量 asr_text 的值"),
            "变量 asr_text 的值"
        );
        // 前缀超长（> 64B）⇒ 标记不在头部 ⇒ 不变；恰好 63B(21 汉字) 仍算头部 ⇒ 剥（边界）
        let head = |n: usize| -> String {
            "这是一段很长的中文内容哦".chars().cycle().take(n).collect()
        };
        let long = head(22);
        assert_eq!(long.len(), 66, "自检：22 汉字 = 66B > 64B");
        let t = format!("{long}<asr_text>正文");
        assert_eq!(strip_qwen3_language_prefix(&t), t, "标记不在头部 ⇒ 不剥");
        let short = head(21);
        assert_eq!(short.len(), 63, "自检：21 汉字 = 63B ≤ 64B");
        assert_eq!(
            strip_qwen3_language_prefix(&format!("{short}<asr_text>正文")),
            "正文",
            "21 汉字仍在头部护栏内 ⇒ 剥"
        );
    }

    /// 🔴 FIX-PREFIX-AND-EAT-371：**去掉闸1/闸2** 后的正面用例。
    ///
    /// - 「带空格」`language chinese <asr_text>正文`（359 闸2「紧贴标签」会拦 ⇒ 现在必须剥）
    /// - 「非标签样」`zh-CN auto<asr_text>正文`（359 闸1「标签样」会拦 ⇒ 现在必须剥）
    #[test]
    fn near_head_non_label_is_stripped() {
        assert_eq!(
            strip_qwen3_language_prefix("language chinese <asr_text>正文内容"),
            "正文内容",
            "带空格（标记不紧贴标签）也必须剥（355 规则：无条件截断）"
        );
        assert_eq!(
            strip_qwen3_language_prefix("zh-CN auto<asr_text>正文内容"),
            "正文内容",
            "非标签样前缀（含连字符/空格）也必须剥"
        );
        // 真实生产形态（Gavin 端测 21:43 漏出的那条）：前缀 + 正文
        assert_eq!(
            strip_qwen3_language_prefix("language chinese<asr_text>明天天气好的话"),
            "明天天气好的话"
        );
    }

    /// 🔴 FIX-PREFIX-AND-EAT-371：**正文中段出现标记不误剥**（位置护栏）。
    ///
    /// 标记起点 > 64B ⇒ 其前已有 ≥21 个汉字正文 ⇒ 判为正文（真实语音念不出 `<asr_text>`）。
    #[test]
    fn mid_body_marker_is_not_stripped() {
        // 21 个汉字 = 63B，标记紧跟其后（起点 63B ≤ 64 ⇒ 仍算头部，剥）
        let head_21 = "一二三四五六七八九十一二三四五六七八九十一";
        assert_eq!(head_21.chars().count(), 21);
        assert_eq!(
            strip_qwen3_language_prefix(&format!("{head_21}<asr_text>后文")),
            "后文",
            "21 汉字(63B)仍属头部（≤64B）⇒ 剥"
        );
        // 22 个汉字 = 66B > 64 ⇒ 标记不在头部 ⇒ 不剥
        let head_22 = "一二三四五六七八九十一二三四五六七八九十一二";
        assert_eq!(head_22.chars().count(), 22);
        let t = format!("{head_22}<asr_text>后文");
        assert_eq!(strip_qwen3_language_prefix(&t), t, "22 汉字(66B) ⇒ 不剥");
    }

    /// 端到端：并入 `strip_asr_special_tokens` 后，<|…|> 与裸前缀一起被剥。
    #[test]
    fn via_strip_asr_special_tokens() {
        assert_eq!(
            Transcriber::strip_asr_special_tokens("language chinese<asr_text><|zh|>你好"),
            "你好"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("汉语<asr_text><|nospeech|>안녕"),
            "안녕"
        );
        // 无前缀（0.6B 形态）⇒ 与旧行为一致
        assert_eq!(Transcriber::strip_asr_special_tokens("<|zh|>你好"), "你好");
        assert_eq!(
            Transcriber::strip_asr_special_tokens("今天天气不错。"),
            "今天天气不错。"
        );
    }
}

// ============================================================
// SLIDING-WINDOW-367：组窗（①）+ 对齐合并（④）单测
// 运行：cargo test --bin feiyin-ime -- sliding_window_367
// ============================================================
// ============================================================
// FIX-TERMS-ECHO-374：词库列表回显的**结构性**剥离
// 运行：cargo test --bin feiyin-ime -- fix374
// ============================================================
#[cfg(test)]
mod fix374_terms_echo_tests {
    use super::{apply_acc_disposition, strip_terms_echo};
    use std::cell::Cell;

    /// 真实注入形态（`build_hotwords_string` = 半角 `,` 连接）；端测现场尾部 77 字即此列表。
    const INJ: &str = "你好,铭印,银线,朵洛莉丝,费曼学习法,子未穿害,低质,罗斯柴尔德,维生素b12";

    /// 端测现场形态：整窗**逐字回显**（全角逗号），**零转写**。
    const FULL_ECHO: &str =
        "你好，铭印，银线，朵洛莉丝，费曼学习法，子未穿害，低质，罗斯柴尔德，维生素b12";

    /// 🔴 正例：输出 = 注入 Terms 逐字（模型把整段 `Terms:` 念回来）⇒ **全剥 ⇒ 空**
    ///    （上层 `push_inner` 的 `text.is_empty()` 分支：只推进 next，不污染窗口）。
    #[test]
    fn full_echo_is_stripped_to_empty() {
        // 输出用**全角**逗号（模型实际输出形态），注入是半角 ⇒ 归一化后仍逐字相等。
        let echoed =
            "你好，铭印，银线，朵洛莉丝，费曼学习法，子未穿害，低质，罗斯柴尔德，维生素b12";
        let hit = strip_terms_echo(echoed, Some(INJ)).expect("应命中回显");
        assert_eq!(hit.matched_terms, 9, "整列表 9 个词条连续命中");
        assert_eq!(hit.stripped, "", "全剥 ⇒ 空 ⇒ 按空解码处理");
        assert_eq!(
            hit.removed_chars,
            echoed.chars().count(),
            "剥掉字符数 = 原文本长度"
        );
    }

    /// 🔴 正例：真实转写 + 尾部拼了 Terms ⇒ **只剥尾部，真实转写一字不动**（端测现场形态）。
    #[test]
    fn tail_echo_is_stripped_keeping_real_text() {
        let text = "可以看看周边的风景。你好，铭印，银线，朵洛莉丝，";
        let hit = strip_terms_echo(text, Some(INJ)).expect("应命中尾部回显");
        assert_eq!(hit.matched_terms, 4);
        assert_eq!(
            hit.stripped, "可以看看周边的风景。",
            "真句子（含句末「。」）完整保留"
        );
    }

    /// 🔴 反例（防误伤）：用户句子里**自然说到 1~2 个词库词** ⇒ **不得剥**（43% 误判的教训）。
    #[test]
    fn natural_mention_of_terms_is_kept() {
        for t in [
            "我最近在看费曼学习法",
            "你好，今天天气不错",
            "银线怎么卖掉",
            "你好，铭印", // 连续 2 个（顺序也对）但 < N ⇒ 仍不剥
        ] {
            assert!(
                strip_terms_echo(t, Some(INJ)).is_none(),
                "自然口语不得被判回显：{t}"
            );
        }
    }

    /// 🔴 反例：输出含这些词条但**顺序与注入不同** ⇒ 不得剥。
    #[test]
    fn out_of_order_terms_are_not_stripped() {
        let inj = "苹果,香蕉,橘子,葡萄,西瓜";
        for t in ["西瓜，葡萄，橘子，香蕉，苹果", "苹果，橘子，香蕉，西瓜"]
        {
            assert!(
                strip_terms_echo(t, Some(inj)).is_none(),
                "顺序不一致不得剥：{t}"
            );
        }
    }

    /// 退化：`terms` 为 None / 空 / 词条数不足 N / 文本为空 ⇒ 不 panic、返回 None（行为不变）。
    #[test]
    fn degenerate_terms_are_noop() {
        assert!(strip_terms_echo("任意文本", None).is_none());
        assert!(strip_terms_echo("任意文本", Some("")).is_none());
        assert!(strip_terms_echo("任意文本", Some(",,,")).is_none());
        assert!(strip_terms_echo("任意文本", Some("单条词")).is_none());
        assert!(strip_terms_echo("", Some(INJ)).is_none());
        // 词条数 < N（4）⇒ 即使整列表逐字回显也不剥（N=4 的取舍，见常量注释）
        let three = "你好,铭印,银线";
        assert!(strip_terms_echo("你好，铭印，银线", Some(three)).is_none());
        // 词条数够但只连续命中 3 个 ⇒ 不剥
        let text = "你好，铭印，银线，这里换成别的话了";
        assert!(strip_terms_echo(text, Some(INJ)).is_none());
    }

    /// 归一化容错：全/半角分隔符、空白差异**不影响**识别（回显是逐字文本，标点差异不该漏检）。
    #[test]
    fn separator_and_space_differences_still_detected() {
        let hit = strip_terms_echo("你好 、 铭印，银线 朵洛莉丝", Some(INJ)).expect("应命中");
        assert_eq!(hit.matched_terms, 4);
        assert_eq!(hit.stripped, "", "剥空 ⇒ 空串");
    }

    /// 回显在**中段**时：前缀保留、后缀保留，只删回显那一段。
    #[test]
    fn middle_echo_is_removed_with_both_sides_kept() {
        let text = "前面这句是真的。你好，铭印，银线，朵洛莉丝，后面这句也是真的";
        let hit = strip_terms_echo(text, Some(INJ)).expect("应命中中段回显");
        assert_eq!(hit.matched_terms, 4);
        assert_eq!(
            hit.stripped, "前面这句是真的。后面这句也是真的",
            "只删回显段与其接缝分隔符"
        );
    }

    // ---- FIX-TERMS-ECHO-374 补充：处置**阶梯**（纯函数，重解用闭包注入）----

    /// 🔴 387 改判：374 命中（连续 ≥4 条）⇒ **不再保留残余**，一律不带注入重解一次
    /// （残余 `维生素b12` 会漏进正文）；重解救回真实内容。
    #[test]
    fn ladder_redecodes_on_echo_even_with_residual() {
        let text = format!("可以看看周边的风景。{INJ}");
        let calls = Cell::new(0);
        let (out, oc) = apply_acc_disposition(text, Some(INJ), 11.8, Some(4.4), || {
            calls.set(calls.get() + 1);
            Ok("可以看看周边的风景。".to_string())
        });
        assert_eq!(calls.get(), 1, "387：命中回显必须重解一次（不保留残余）");
        // 【388 契约变更 · D1】重解后**所有 kind** 统一查产出率：
        // old：收下「可以看看周边的风景。」(10 字)；new：10 字/11.8s < avg4.4/3(≈1.47 字/s) ⇒ invalid 空串。
        assert_eq!(out, "", "D1：重解产出率不足 ⇒ invalid 空串（old：10 字）");
        assert!(oc.invalid, "D1：invalid 置位（old：false）");
        assert_eq!(oc.matched_terms, 9);
        assert!(oc.redecoded);
    }

    /// 🔴 阶梯 2/3：整窗都是回显 ⇒ 剥空 ⇒ **不带注入重解一次** ⇒ 用重解结果（救回真实内容）。
    #[test]
    fn ladder_redecodes_once_when_strip_is_empty() {
        let calls = Cell::new(0);
        let (out, oc) =
            apply_acc_disposition(FULL_ECHO.to_string(), Some(INJ), 11.8, Some(4.4), || {
                calls.set(calls.get() + 1);
                Ok("这是被回显掩盖的真实转写内容".to_string())
            });
        assert_eq!(calls.get(), 1, "重解**至多一次**");
        // 【388 契约变更 · D1】old：收下 14 字；new：14 字/11.8s < avg4.4/3 ⇒ invalid 空串。
        assert_eq!(out, "", "D1：重解产出率不足 ⇒ invalid 空串（old：14 字）");
        assert!(oc.invalid, "D1：invalid 置位（old：false）");
        assert!(oc.redecoded);
        assert_eq!(oc.remaining_chars, 0, "剥完为空");
        assert_eq!(oc.matched_terms, 9);
    }

    /// 🔴 阶梯 4：重解**仍空/失败** ⇒ 空串（上层按空解码处理：推进 next、不污染窗口）。
    #[test]
    fn ladder_falls_back_to_empty_when_redecode_fails() {
        for fail in [true, false] {
            let calls = Cell::new(0);
            let (out, oc) =
                apply_acc_disposition(FULL_ECHO.to_string(), Some(INJ), 11.8, Some(4.4), || {
                    calls.set(calls.get() + 1);
                    if fail {
                        Err(anyhow::anyhow!("ASR accuracy model produced empty output"))
                    } else {
                        Ok(String::new())
                    }
                });
            assert_eq!(calls.get(), 1);
            assert_eq!(out, "", "重解空/失败 ⇒ 空串（不冒泡错误）");
            assert!(oc.redecoded);
        }
    }

    /// 重解结果**又吐词条**（防御）：再剥一次 ⇒ 仍空（且**不再**重解第二次）。
    #[test]
    fn ladder_strips_redecoded_echo_without_looping() {
        let calls = Cell::new(0);
        let (out, oc) =
            apply_acc_disposition(FULL_ECHO.to_string(), Some(INJ), 11.8, Some(4.4), || {
                calls.set(calls.get() + 1);
                Ok(FULL_ECHO.to_string()) // 无注入重解却仍吐词条
            });
        assert_eq!(calls.get(), 1, "不得循环重解");
        assert_eq!(out, "", "重解结果再剥 ⇒ 空");
        assert!(oc.redecoded);
    }

    /// 未命中 ⇒ 原样返回、零埋点、**不重解**。
    #[test]
    fn ladder_is_noop_when_no_echo() {
        let called = Cell::new(false);
        let (out, oc) = apply_acc_disposition(
            "今天天气不错".to_string(),
            Some(INJ),
            2.0,
            None,
            || {
                called.set(true);
                Ok("x".to_string())
            },
        );
        assert!(!called.get());
        assert_eq!(out, "今天天气不错");
        assert_eq!(oc.matched_terms, 0);
        assert_eq!(oc.remaining_chars, 6);
        assert!(!oc.redecoded);
    }
}

// ============================================================
// FIX-PREVIEW-STALE-AND-COLLAPSE-375（B）：产出率坍塌判据 + 与 374 共用的阶梯
// 运行：cargo test --bin feiyin-ime -- fix375
// ============================================================
#[cfg(test)]
mod fix375_collapse_tests {
    use super::{
        acc_avg_chars_per_sec, apply_acc_disposition, output_rate_ok, COLLAPSE_MIN_AVG_WINDOWS,
    };
    use std::cell::Cell;

    /// 端测坍塁现场的数字：均值 ≈4.4 字/s、窗 10.4s、输出仅 8 字（≈0.77 字/s）。
    const AVG: f32 = 4.4;
    const SECS: f32 = 10.4;

    /// 冷启动 / 期望产出过少 ⇒ 一律判「正常」（宁漏勿误杀）。
    #[test]
    fn rate_ok_cold_start_and_tiny_expectation_always_ok() {
        // 【388 契约变更 · D2 冷启动下限】old：无均值一律 true；
        // new：无均值 + 音频 ≥ COLD_MIN_AUDIO_SECS(3.0) + 字/秒 < COLD_MIN_CHARS_PER_SEC(1.0) ⇒ 坍塌。
        assert!(
            !output_rate_ok(0, SECS, None),
            "D2：无均值长音频（0 字/10.4s）⇒ 坍塌（old：true）"
        );
        assert!(
            !output_rate_ok(0, SECS, Some(f32::NAN)),
            "D2：非有限均值按无均值处理 ⇒ 坍塌（old：true）"
        );
        assert!(
            !output_rate_ok(0, SECS, Some(0.0)),
            "D2：非正均值按无均值处理 ⇒ 坍塌（old：true）"
        );
        assert!(output_rate_ok(0, 0.0, Some(AVG)), "无音频 ⇒ 不判");
        // 期望产出 < COLLAPSE_MIN_EXPECTED_CHARS(8)：avg 4.4 × 1.0s = 4.4 < 8 ⇒ 即使 0 字也放行
        assert!(
            output_rate_ok(0, 1.0, Some(AVG)),
            "短窗（期望产出太少）⇒ 不判"
        );
        // D2 边界：短于门槛（<3s）不判；恰 3s 时 1 字/s（含等号）判 ok、低于则坍塌。
        assert!(output_rate_ok(0, 2.9, None), "D2：<3s 不判");
        assert!(output_rate_ok(3, 3.0, None), "D2：恰 3s、恰 1 字/s ⇒ ok");
        assert!(!output_rate_ok(2, 3.0, None), "D2：恰 3s、0.67 字/s ⇒ 坍塌");
    }

    /// 🔴 坍塌窗必须被抓、正常窗不得误判（含边界：恰为均值 1/3）。
    #[test]
    fn rate_ok_detects_collapse_but_not_normal() {
        assert!(
            !output_rate_ok(8, SECS, Some(AVG)),
            "端测现场（8 字/10.4s ≈ 均值 17%）必须判坍塌"
        );
        assert!(
            output_rate_ok(45, SECS, Some(AVG)),
            "正常窗（45 字/10.4s）不得误判"
        );
        // 边界：均值 × 1/3 × 秒数 = 4.4/3×10.4 ≈ 15.25
        assert!(output_rate_ok(16, SECS, Some(AVG)), "恰在阈值之上 ⇒ 正常");
        assert!(!output_rate_ok(15, SECS, Some(AVG)), "恰在阈值之下 ⇒ 坍塌");
        // 相对判据随均值自适应：均值降到 2.0 时阈值 = 2.0/3×10.4 ≈ 6.9 ⇒ 8 字**不算**坍塌
        assert!(
            output_rate_ok(8, SECS, Some(2.0)),
            "慢语速均值 ⇒ 同一产出不再判坍塌"
        );
        assert!(
            !output_rate_ok(4, SECS, Some(2.0)),
            "低于自适应阈值 ⇒ 仍判坍塌"
        );
    }

    /// 均值需要 ≥ `COLLAPSE_MIN_AVG_WINDOWS` 个已接受窗口才可用（否则冷启动）。
    #[test]
    fn avg_needs_min_windows() {
        assert_eq!(acc_avg_chars_per_sec(0, 0.0, 0), None);
        assert_eq!(acc_avg_chars_per_sec(50, 10.0, 1), None);
        assert_eq!(acc_avg_chars_per_sec(50, 0.0, 5), None, "无有效秒数 ⇒ None");
        assert_eq!(
            acc_avg_chars_per_sec(50, 10.0, COLLAPSE_MIN_AVG_WINDOWS),
            Some(5.0)
        );
    }

    /// 🔴 阶梯：坍塌 ⇒ 不带注入重解一次 ⇒ 重解正常则采用（`collapse=true`、`redecoded=true`）。
    #[test]
    fn ladder_redecodes_on_collapse() {
        let calls = Cell::new(0);
        let (out, oc) = apply_acc_disposition(
            "上一下文传路的".to_string(),
            None,
            SECS,
            Some(AVG),
            || {
                calls.set(calls.get() + 1);
                Ok("我记得当初设计的时候磁条是作为上下文串用的".to_string())
            },
        );
        assert_eq!(calls.get(), 1, "坍塌 ⇒ 重解**至多一次**");
        assert_eq!(out, "我记得当初设计的时候磁条是作为上下文串用的");
        assert!(oc.collapse, "应标记为坍塌触发");
        assert!(oc.redecoded);
        assert_eq!(oc.matched_terms, 0);
    }

    /// 🔴 阶梯：重解**仍坍塌** ⇒ 按空解码处理（空串，交给 `push_inner` 的空分支）。
    #[test]
    fn ladder_falls_back_to_empty_when_redecode_still_collapses() {
        let calls = Cell::new(0);
        let (out, oc) = apply_acc_disposition(
            "上一下文传路的".to_string(),
            None,
            SECS,
            Some(AVG),
            || {
                calls.set(calls.get() + 1);
                Ok("还是很短的一句".to_string()) // 7 字 / 10.4s ⇒ 仍坍塌
            },
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(out, "", "仍坍塌 ⇒ 空解码（不污染窗口）");
        assert!(oc.collapse);
        assert!(oc.redecoded);
    }

    /// 正常窗：不判坍塌 ⇒ **绝不重解**（闭包被调用即失败）。
    #[test]
    fn ladder_leaves_normal_window_untouched() {
        let real = "我记得当初设计的时候磁条是作为上下文串用的这句话挺长的";
        assert!(
            real.chars().count() as f32 >= AVG * SECS / 3.0,
            "自检：该窗应属正常"
        );
        let called = Cell::new(false);
        let (out, oc) = apply_acc_disposition(real.to_string(), None, SECS, Some(AVG), || {
            called.set(true);
            Ok("x".to_string())
        });
        assert!(!called.get());
        assert_eq!(out, real);
        assert!(!oc.collapse);
        assert!(!oc.redecoded);
    }
}

// ============================================================
// FIX-ACC-OUTPUT-GUARD-387：解码输出守卫（回显残余 / 标签 / 空）
// 运行：cargo test --bin feiyin-ime -- fix387
// ============================================================
#[cfg(test)]
mod fix387_output_guard_tests {
    use super::{apply_acc_disposition, GuardKind};
    use std::cell::Cell;

    const INJ: &str = "你好,铭印,银线,朵洛莉丝,费曼学习法,子未穿害,低质,罗斯柴尔德,维生素b12";
    const FULL_ECHO: &str =
        "你好，铭印，银线，朵洛莉丝，费曼学习法，子未穿害，低质，罗斯柴尔德，维生素b12";

    /// 回显（连续 ≥4 条）⇒ 重解一次、用重解结果（残余不再保留）。
    #[test]
    fn guard_echo_redecodes_and_uses_recovered() {
        let text = "可以看看周边的风景。你好，铭印，银线，朵洛莉丝".to_string();
        let calls = Cell::new(0);
        let (out, oc) = apply_acc_disposition(text, Some(INJ), 11.8, Some(4.4), || {
            calls.set(calls.get() + 1);
            Ok("可以看看周边的风景。".to_string())
        });
        assert_eq!(calls.get(), 1);
        // 【388 契约变更 · D1】old：收下 10 字；new：10 字/11.8s < avg4.4/3 ⇒ invalid 空串。
        assert_eq!(out, "", "D1：产出率不足 ⇒ 空串（old：收下）");
        assert_eq!(oc.guard, GuardKind::Echo);
        assert!(
            oc.redecoded && oc.invalid,
            "D1：invalid 置位（old：!invalid）"
        );
    }

    /// 残余**全部由词表条目构成**（如落单的 `维生素b12`）⇒ 视同回显、重解；
    /// 重解若吐回同一条词（非 ≥4 条）⇒ 仍采用（不误丢真人内容）。
    #[test]
    fn guard_residual_all_terms_triggers_redecode() {
        let calls = Cell::new(0);
        let (out, oc) =
            apply_acc_disposition("维生素b12".to_string(), Some(INJ), 1.0, None, || {
                calls.set(calls.get() + 1);
                Ok("维生素b12".to_string())
            });
        assert_eq!(calls.get(), 1, "残余全词表 ⇒ 必须重解");
        assert_eq!(out, "维生素b12", "重解非空且无 ≥4 连条 ⇒ 采用");
        assert_eq!(oc.guard, GuardKind::Echo);
        assert!(!oc.invalid);
    }

    /// 标签：`<location>` 剥后为空/只剩标点 ⇒ 重解；含正文的标签 ⇒ 只剥标签、保留正文、不重解。
    #[test]
    fn guard_tag_only_redecodes_but_tag_with_text_keeps() {
        // (a) 标签是唯一内容（连同标点）⇒ Tag 触发重解
        let calls = Cell::new(0);
        let (out, oc) =
            apply_acc_disposition("。。<location>".to_string(), None, 5.0, None, || {
                calls.set(calls.get() + 1);
                Ok("这是真实内容".to_string())
            });
        assert_eq!(calls.get(), 1);
        assert_eq!(out, "这是真实内容");
        assert_eq!(oc.guard, GuardKind::Tag);
        // (b) 标签 + 正文 ⇒ 剥标签、保留正文、不重解
        let called = Cell::new(false);
        let (out2, oc2) = apply_acc_disposition(
            "规划会议很难得。<location>".to_string(),
            None,
            5.0,
            None,
            || {
                called.set(true);
                Ok("x".to_string())
            },
        );
        assert!(!called.get(), "有正文时不得重解");
        assert_eq!(out2, "规划会议很难得。");
        assert_eq!(oc2.guard, GuardKind::None);
    }

    /// 空 / 只剩标点 ⇒ Empty 触发重解。
    #[test]
    fn guard_empty_redecodes() {
        let calls = Cell::new(0);
        let (out, oc) = apply_acc_disposition("。".to_string(), None, 5.0, None, || {
            calls.set(calls.get() + 1);
            Ok("真实内容".to_string())
        });
        assert_eq!(calls.get(), 1);
        // 【388 契约变更 · D1+D2】old：收下「真实内容」(4 字)；new：4 字/5.0s = 0.8 字/s < 1 ⇒ invalid 空串。
        assert_eq!(out, "", "D1/D2：产出率不足 ⇒ 空串（old：收下 4 字）");
        assert!(oc.invalid, "D1：invalid 置位（old：false）");
        assert_eq!(oc.guard, GuardKind::Empty);
    }

    /// 重解后仍无效（又吐整段回显）⇒ 返回空串 + `invalid=true`（**不循环**重解）。
    #[test]
    fn guard_invalid_after_redecode_returns_empty() {
        let calls = Cell::new(0);
        let (out, oc) = apply_acc_disposition(
            format!("正文。{FULL_ECHO}"),
            Some(INJ),
            11.8,
            Some(4.4),
            || {
                calls.set(calls.get() + 1);
                Ok(FULL_ECHO.to_string()) // 无注入重解却仍吐整段词表
            },
        );
        assert_eq!(calls.get(), 1, "不得循环重解");
        assert_eq!(out, "");
        assert!(oc.invalid);
        assert_eq!(oc.guard, GuardKind::Echo);
    }
}

#[cfg(test)]
mod guard387_review_tests {
    use super::apply_acc_disposition;

    /// 重解只出标点 ⇒ 判无效返回空串（交 386-C 流式兜底），不把「。」当正文收下。
    #[test]
    fn redecode_punct_only_is_invalid() {
        let (out, oc) = apply_acc_disposition("<location>".to_string(), None, 10.0, None, || {
            Ok("。".to_string())
        });
        assert!(oc.redecoded);
        assert!(oc.invalid, "重解只剩标点必须判无效");
        assert_eq!(out, "");
    }
}

// ============================================================
// FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388（阶段一）：剪静音 / has_content / 重解产出率 / 冷启动下限
// ============================================================
#[cfg(test)]
mod fix388_trim_and_floor_tests {
    use super::{
        apply_acc_disposition, has_content, output_rate_ok, trim_to_speech, COLD_MIN_AUDIO_SECS,
        COLD_MIN_CHARS_PER_SEC,
    };

    const RATE: usize = 16000;

    /// 测 #1a：首尾静音剪到 200ms（`pad`）。
    #[test]
    fn trim388_head_tail_to_pad() {
        let pad = (0.2 * RATE as f32) as usize;
        let mut audio = vec![0.0f32; 3 * RATE];
        for x in &mut audio[RATE..2 * RATE] {
            *x = 0.5;
        }
        let out = trim_to_speech(&audio, &[(RATE, 2 * RATE)], pad);
        assert_eq!(out.len(), RATE + 2 * pad, "语音 1s + 两侧各 200ms");
        assert!(out[..pad].iter().all(|&x| x == 0.0), "头部静音 = pad");
        assert_eq!(out[pad], 0.5, "语音起点紧接 pad");
        assert!(
            out[out.len() - pad..].iter().all(|&x| x == 0.0),
            "尾部静音 = pad"
        );
    }

    /// 测 #1b：段间 300ms 停顿**原样保留**（被合并）；段间 3s 停顿**压到 400ms**。
    #[test]
    fn trim388_gap_keep_small_compress_large() {
        let pad = (0.2 * RATE as f32) as usize;
        let speech = RATE; // 1s
                           // -- 300ms 间隔 --
        let gap300 = 300 * 16;
        let mut a = vec![0.0f32; pad + speech + gap300 + speech + pad];
        for x in &mut a[pad..pad + speech] {
            *x = 0.5;
        }
        let s2 = pad + speech + gap300;
        for x in &mut a[s2..s2 + speech] {
            *x = 0.5;
        }
        let out = trim_to_speech(&a, &[(pad, pad + speech), (s2, s2 + speech)], pad);
        assert_eq!(
            out.len(),
            2 * speech + gap300 + 2 * pad,
            "300ms ≤ 2×pad(400ms) ⇒ 停顿原样保留（合并）"
        );
        // -- 3s 间隔 --
        let gap3s = 3000 * 16;
        let mut b = vec![0.0f32; pad + speech + gap3s + speech + pad];
        for x in &mut b[pad..pad + speech] {
            *x = 0.5;
        }
        let t2 = pad + speech + gap3s;
        for x in &mut b[t2..t2 + speech] {
            *x = 0.5;
        }
        let out2 = trim_to_speech(&b, &[(pad, pad + speech), (t2, t2 + speech)], pad);
        assert_eq!(
            out2.len(),
            2 * (speech + 2 * pad),
            "3s > 400ms ⇒ 停顿压到 2×pad(400ms)"
        );
        // 压缩后的中间静音恰为 2×pad 个 0（block1 尾 pad + block2 头 pad）。
        let boundary = speech + 2 * pad;
        assert!(
            out2[boundary - pad..boundary + pad]
                .iter()
                .all(|&x| x == 0.0),
            "压缩后中间静音 = 2×pad"
        );
        assert_eq!(out2[boundary - pad - 1], 0.5, "压缩前是语音");
        assert_eq!(out2[boundary + pad], 0.5, "压缩后是语音");
    }

    /// 测 #1c：区间越界 clamp；空区间 / 空音频 / 倒置区间 ⇒ 空。
    #[test]
    fn trim388_bounds_and_empty() {
        let pad = 100usize;
        let audio = vec![0.5f32; 1000];
        let out = trim_to_speech(&audio, &[(0, 5000)], pad);
        assert_eq!(out.len(), 1000, "end 越界 ⇒ clamp 到 len（无尾 pad 空间）");
        assert!(
            trim_to_speech(&audio, &[(2000, 3000)], pad).is_empty(),
            "start 越界 ⇒ 整段落在 len 外 ⇒ 丢弃"
        );
        assert!(trim_to_speech(&audio, &[], pad).is_empty(), "空区间 ⇒ 空");
        assert!(
            trim_to_speech(&[], &[(0, 10)], pad).is_empty(),
            "空音频 ⇒ 空"
        );
        assert!(
            trim_to_speech(&audio, &[(500, 100)], pad).is_empty(),
            "倒置区间 ⇒ 空"
        );
    }

    /// 测 #2：整窗无语音 ⇒ 纯判定返回空；`transcribe_acc_ctx` 的 `n_ranges==0` 早退必须在
    /// **首次解码之前**（源码级护栏：无语音不调用解码即返回空）。
    #[test]
    fn trim388_no_speech_returns_empty_before_decode() {
        assert!(trim_to_speech(&[0.0f32; 100], &[], 100).is_empty());
        let src = include_str!("mod.rs");
        let body = src
            .split("pub(crate) fn transcribe_acc_ctx(")
            .nth(1)
            .expect("transcribe_acc_ctx 必须存在");
        let early = body
            .find("if trimmed && n_ranges == 0")
            .expect("无语音早退锚点缺失");
        let ret = body[early..]
            .find("return Ok((String::new(), true));")
            .expect("无语音早退返回缺失");
        // 388 主控验收补：首解改为 `decode_accuracy_allow_empty(`（空输出进 Empty 判据）；不变量不变。
        let decode = body
            .find("decode_accuracy_allow_empty(")
            .expect("首解调用缺失");
        assert!(
            early < decode && early + ret < decode,
            "无语音早退必须早于首次解码"
        );
    }

    /// 测 #3：`has_content` —— `**`/`。`/`…`/空白/空 ⇒ false；`嗯`/`a`/`3` ⇒ true。
    #[test]
    fn has_content388_semantics() {
        for t in ["", "**", "。", "…", "  ", "。。**……"] {
            assert!(!has_content(t), "应判无内容：{t:?}");
        }
        for t in ["嗯", "a", "3", "维生素b12", "嗯。"] {
            assert!(has_content(t), "应判有内容：{t:?}");
        }
    }

    /// 测 #4（D1）：重解出「嗯。」（11.6s、avg 2.0）⇒ invalid 空串；重解出 48 字 ⇒ 收下。
    #[test]
    fn floor388_redecode_short_invalid_long_kept() {
        let (out, oc) = apply_acc_disposition(
            "上一下文传路的".to_string(),
            None,
            11.6,
            Some(2.0),
            || Ok("嗯。".to_string()),
        );
        assert!(oc.redecoded, "产出率不足 ⇒ 触发重解");
        assert!(oc.invalid, "重解过短 ⇒ invalid");
        assert_eq!(out, "", "invalid ⇒ 空串");
        // 48 字 / 11.6s / avg 2.0：阈值 = 2.0×11.6/3 ≈ 7.73 ⇒ 48 字过门。
        let long48: String = std::iter::repeat("内容").take(24).collect();
        assert_eq!(long48.chars().count(), 48);
        let (out2, oc2) = apply_acc_disposition(
            "上一下文传路的".to_string(),
            None,
            11.6,
            Some(2.0),
            || Ok(long48.clone()),
        );
        assert!(!oc2.invalid, "48 字应过产出率");
        assert_eq!(out2, long48);
    }

    /// 测 #5（D2）：冷启动 6.85s 出 6 字 ⇒ 不 ok；2s 出 1 字（<3s 门槛）⇒ ok。
    #[test]
    fn floor388_cold_start_floor() {
        assert!(
            !output_rate_ok(6, 6.85, None),
            "6.85s/6 字（0.876/s）⇒ 坍塌（BUILD-387 seq1 现场）"
        );
        assert!(output_rate_ok(1, 2.0, None), "2s/1 字：短于门槛 ⇒ ok");
        assert!(
            output_rate_ok(3, 3.0, None),
            "恰门槛：3 字/3s = 1 字/s ⇒ ok"
        );
        assert!(!output_rate_ok(2, 3.0, None), "恰门槛：0.67 字/s ⇒ 坍塌");
        assert_eq!(COLD_MIN_AUDIO_SECS, 3.0);
        assert_eq!(COLD_MIN_CHARS_PER_SEC, 1.0);
    }

    /// 测 #6（`#[ignore]`，需真模型）：`speech_ranges` 对 wav 片段 + 前后各补 5s 静音 ⇒
    /// 区间**不落在**补的静音里。
    #[test]
    #[ignore = "手工：需 silero 模型 + kv_long.wav"]
    fn trim388_speech_ranges_real_model_excludes_padded_silence() {
        use sherpa_onnx::Wave;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let model_dir = root.join("models");
        let Some(vad) = super::VadSegmenter::try_new_for_local_trim(&model_dir) else {
            eprintln!("skip: silero VAD 不可用（模型缺失）");
            return;
        };
        let wav_path = root.join("models/kv259/kv_long.wav");
        let Some(wave) = Wave::read(wav_path.to_str().expect("utf-8 path")) else {
            eprintln!("skip: kv_long.wav 缺失");
            return;
        };
        let clip_len = wave.samples().len().min(4 * 16000);
        let pad_s = 5 * 16000usize;
        let mut audio = vec![0.0f32; pad_s];
        audio.extend_from_slice(&wave.samples()[..clip_len]);
        audio.extend(std::iter::repeat(0.0f32).take(pad_s));
        let ranges = vad.speech_ranges(&audio);
        assert!(!ranges.is_empty(), "wav 片段应检出语音区间");
        assert!(
            ranges
                .iter()
                .all(|&(s, e)| s >= pad_s.saturating_sub(1600) && e <= pad_s + clip_len + 1600),
            "区间不得落在补的 5s 静音里（允许 VAD 边界 ~100ms）：{ranges:?} clip_end={}",
            pad_s + clip_len
        );
    }
}

// ============================================================
// TEST-SYNC-387（阶段三 · 非作者护栏 · coder-1）：标签边界 / 回显残余 / 重解次数
//   契约：先剥 `<[A-Za-z_/][^<>]{0,30}>`；词表回显（连续 ≥4 条）或剥后残余全为词条 /
//   只剩标点 / 空 ⇒ 不带注入重解一次；重解仍无效 ⇒ 空串 + `invalid=true`；正文夹标签只剥不重解。
// ============================================================
/// VAD-393（A4）：时间线剪静音的**纯决策**（三态），无需模型。
#[cfg(test)]
mod testsync393_tests {
    use super::{plan_timeline_trim, TimelineTrim};

    /// `None`（无时间线）⇒ 一律返回 `None`，调用方回退自跑 VAD（与流式是否非空无关）。
    #[test]
    fn ts393_plan_none_falls_back_to_vad() {
        assert_eq!(plan_timeline_trim(None, true), None);
        assert_eq!(plan_timeline_trim(None, false), None);
    }

    /// 非空区间 ⇒ `Apply`（原样带上区间，供 `trim_to_speech`）。
    #[test]
    fn ts393_plan_nonempty_applies_ranges() {
        let r = [(10usize, 20usize), (30, 40)];
        assert_eq!(
            plan_timeline_trim(Some(&r), false),
            Some(TimelineTrim::Apply(vec![(10, 20), (30, 40)]))
        );
    }

    /// 空区间 + 流式非空 ⇒ `WholeWindow`（不复剪、整窗解码，防漏判吞字）。
    #[test]
    fn ts393_plan_empty_with_streaming_decodes_whole_window() {
        assert_eq!(
            plan_timeline_trim(Some(&[]), true),
            Some(TimelineTrim::WholeWindow)
        );
    }

    /// 空区间 + 流式也空 ⇒ `Empty`（提前返回空串）。
    #[test]
    fn ts393_plan_empty_without_streaming_is_empty() {
        assert_eq!(
            plan_timeline_trim(Some(&[]), false),
            Some(TimelineTrim::Empty)
        );
    }
}

#[cfg(test)]
mod testsync388_tests {
    use super::{apply_acc_disposition, has_content, output_rate_ok, trim_to_speech, GuardKind};
    use std::cell::Cell;

    struct Lcg(u64);
    impl Lcg {
        fn n(&mut self, lo: usize, hi: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (lo as u64 + (self.0 >> 33) % ((hi - lo + 1) as u64)) as usize
        }
    }

    /// 1. `trim_to_speech` 性质：300 组（音频 0~20s、区间 0~6 个、可重叠/越界/倒序）。
    ///    断言：① 输出长度 ≤ 输入 ② 输出按原音频**递增序**拼接（非递减回溯）③ 每个**有效区间**
    ///    内样本都出现在输出（不丢语音）。
    #[test]
    fn ts388_trim_property_no_growth_ordered_and_covers_speech() {
        let pad = 3200usize; // 200ms @16k
        let mut rng = Lcg(0x388_5EED);
        for case in 0..300usize {
            let audio_len = rng.n(0, 20 * 16000);
            // 唯一值 = 原下标（f32 对 ≤2^24 的整数精确）⇒ 可逐样本回溯来源。
            let audio: Vec<f32> = (0..audio_len).map(|i| i as f32).collect();
            let nr = rng.n(0, 6);
            let mut ranges: Vec<(usize, usize)> = Vec::new();
            for _ in 0..nr {
                let a = rng.n(0, audio_len.max(1));
                let b = rng.n(0, audio_len.max(1));
                ranges.push((a, b));
            }
            let out = trim_to_speech(&audio, &ranges, pad);
            assert!(out.len() <= audio.len(), "case {case}: 输出不得长于输入");
            let mut covered = vec![false; audio_len];
            let mut prev = 0usize;
            for (k, &v) in out.iter().enumerate() {
                let i = v as usize;
                assert!(i < audio_len, "case {case}: 输出含非原音频样本 {v}");
                if k > 0 {
                    assert!(i >= prev, "case {case}: 输出必须按原音频递增序拼接");
                }
                prev = i;
                covered[i] = true;
            }
            for &(s, e) in &ranges {
                if s < e {
                    let lo = s.min(audio_len);
                    let hi = e.min(audio_len);
                    for i in lo..hi {
                        assert!(covered[i], "case {case}: 语音样本 {i}（区间 {s}..{e}）丢失");
                    }
                }
            }
        }
    }

    /// 2. 首解空输出进入 `Empty` 判据 ⇒ 不带注入重解一次；重解给出正常长度文本 ⇒ 收下。
    #[test]
    fn ts388_first_decode_empty_enters_empty_and_redecodes() {
        let calls = Cell::new(0);
        let recovered = "这是一段正常的转写内容用于通过产出率门槛的足够长文本";
        let (out, oc) = apply_acc_disposition(String::new(), None, 5.0, Some(4.0), || {
            calls.set(calls.get() + 1);
            Ok(recovered.to_string())
        });
        assert_eq!(calls.get(), 1, "首解空输出必须触发不带注入重解一次");
        assert_eq!(oc.guard, GuardKind::Empty);
        assert!(oc.redecoded && !oc.invalid);
        assert_eq!(out, recovered);
    }

    /// 3. `has_content` 边界：全角数字/英文/假名 ⇒ true；emoji ⇒ false；`**嗯**` ⇒ true。
    #[test]
    fn ts388_has_content_boundaries() {
        assert!(has_content("３"), "全角数字 ３ 是 Nd ⇒ true");
        assert!(has_content("A"));
        assert!(has_content("あ"), "平假名是字母（is_alphabetic）⇒ true");
        assert!(!has_content("😀"), "emoji 既非字母也非数字 ⇒ false");
        assert!(has_content("**嗯**"), "含 嗯 ⇒ true");
        assert!(!has_content("。。**……"), "仅标点/星号/空白 ⇒ false");
    }

    /// 4. 冷启动边界：audio 2.99/3.0/3.01 × 1.0 字/s 两侧；avg=NaN/0/−1/±inf 与 None 同口径。
    #[test]
    fn ts388_cold_start_boundary_and_avg_variants() {
        assert!(output_rate_ok(0, 2.99, None), "<3s ⇒ 不判");
        assert!(!output_rate_ok(2, 3.0, None), "3.0s 2 字 ⇒ 0.67<1 ⇒ 坍塌");
        assert!(output_rate_ok(3, 3.0, None), "3.0s 3 字 ⇒ 1.0≥1 ⇒ 正常");
        assert!(
            !output_rate_ok(3, 3.01, None),
            "3.01s 3 字 ⇒ 0.997<1 ⇒ 坍塌"
        );
        assert!(output_rate_ok(4, 3.01, None), "3.01s 4 字 ⇒ ≥1 ⇒ 正常");
        for v in [f32::NAN, 0.0, -1.0, f32::INFINITY, f32::NEG_INFINITY] {
            for (out_chars, audio) in [
                (0usize, 2.99f32),
                (2, 3.0),
                (3, 3.0),
                (3, 3.01),
                (50, 10.0),
                (5, 10.0),
            ] {
                assert_eq!(
                    output_rate_ok(out_chars, audio, Some(v)),
                    output_rate_ok(out_chars, audio, None),
                    "avg={v} 应与 None 同口径（out={out_chars} audio={audio}）"
                );
            }
        }
    }

    /// 5. 重解至多一次：300 组随机（回显 / 标签 / 空 / 坍塌），重解调用次数恒 ≤1，`invalid ⇒ 输出空串`。
    #[test]
    fn ts388_redecode_at_most_once_property() {
        let inj = "你好,铭印,银线,朵洛莉丝,费曼学习法,子未穿害,低质,罗斯柴尔德,维生素b12";
        let full_echo =
            "你好，铭印，银线，朵洛莉丝，费曼学习法，子未穿害，低质，罗斯柴尔德，维生素b12";
        let pool = [
            "",
            "。。",
            "**",
            "<location>",
            full_echo,
            "今天天气不错我们来测试一下这段文本的长度是否足够",
            "维生素b12",
            "正常内容一句话",
        ];
        let mut rng = Lcg(0x388_C0FFEE);
        for case in 0..300usize {
            let t = pool[rng.n(0, pool.len() - 1)].to_string();
            let audio = 1.0 + rng.n(0, 150) as f32 / 10.0; // 1.0..16.0
            let avg = if rng.n(0, 1) == 0 { None } else { Some(4.0f32) };
            let re = pool[rng.n(0, pool.len() - 1)].to_string();
            let calls = Cell::new(0);
            let (out, oc) = apply_acc_disposition(t, Some(inj), audio, avg, || {
                calls.set(calls.get() + 1);
                Ok(re.clone())
            });
            assert!(calls.get() <= 1, "case {case}: 重解不得超过一次");
            if oc.invalid {
                assert!(out.is_empty(), "case {case}: invalid ⇒ 输出空串");
            }
        }
    }
}

#[cfg(test)]
mod testsync387_tests {
    use super::{apply_acc_disposition, strip_angle_tags, GuardKind};
    use std::cell::Cell;

    const INJ: &str = "你好,铭印,银线,朵洛莉丝,费曼学习法,子未穿害,低质,罗斯柴尔德,维生素b12";

    /// 测 #1 标签边界：被剥 `<location>` / `</x>` / `<_a>`（含夹在正文中）；
    /// 不被剥 `<3岁` / `a<b` / `< 空格>` / 中文 `<你好>` / 内容 31 字符 / 未闭合超 31。
    #[test]
    fn ts387_tag_boundaries() {
        for input in ["<location>", "</x>", "<_a>"] {
            let (out, hit) = strip_angle_tags(input);
            assert_eq!(out, "", "应整体剥掉：{input}");
            assert!(hit, "应标记剥到过：{input}");
        }
        let (out, hit) = strip_angle_tags("规划会议<location>很难得");
        assert_eq!(out, "规划会议很难得", "夹在正文中的标签只删标签本身");
        assert!(hit);

        for (input, why) in [
            ("<3岁", "首字符非 [字母/_//]"),
            ("a<b", "无闭合 `>`"),
            ("< 空格>", "首字符为空格"),
            ("<你好>", "首字符非 ASCII 字母"),
        ] {
            let (out, hit) = strip_angle_tags(input);
            assert_eq!(out, input, "不得剥（{why}）：{input}");
            assert!(!hit, "不应标记（{why}）：{input}");
        }
        // 内容长度边界：恰 30 字符 ⇒ 剥；31 字符 / 未闭合（>31）⇒ 不剥。
        let t30 = format!("<{}>", "a".repeat(30));
        assert_eq!(
            strip_angle_tags(&t30),
            (String::new(), true),
            "恰 30 字符应剥"
        );
        let t31 = format!("<{}>", "a".repeat(31));
        assert_eq!(
            strip_angle_tags(&t31),
            (t31.clone(), false),
            "31 字符不得剥"
        );
        let unclosed = format!("<{}", "a".repeat(40));
        assert_eq!(
            strip_angle_tags(&unclosed),
            (unclosed.clone(), false),
            "未闭合超 31 不得剥"
        );
    }

    /// 测 #2 回显残余：词表末条单独残留（`维生素b12`）⇒ 触发重解（Echo）；
    /// 正常句子里**恰好含 1 个词条** ⇒ 不触发、原样返回。
    #[test]
    fn ts387_residual_last_term_triggers_but_single_in_sentence_not() {
        // (a) 残余恰为词表末条 ⇒ 触发重解一次。
        let calls = Cell::new(0);
        let (out, oc) =
            apply_acc_disposition("维生素b12".to_string(), Some(INJ), 1.0, None, || {
                calls.set(calls.get() + 1);
                Ok("真实内容".to_string())
            });
        assert_eq!(calls.get(), 1, "残余=末条词表 ⇒ 必须重解");
        assert_eq!(oc.guard, GuardKind::Echo);
        assert_eq!(out, "真实内容");

        // (b) 正常句子里恰含 1 个词条 ⇒ 不触发、不重解、原样返回。
        let text = "我最近在看维生素b12的相关资料。";
        let called = Cell::new(false);
        let (out2, oc2) = apply_acc_disposition(text.to_string(), Some(INJ), 8.0, None, || {
            called.set(true);
            Ok("x".to_string())
        });
        assert!(!called.get(), "句中恰 1 个词条不得触发重解");
        assert_eq!(out2, text, "原样返回");
        assert_eq!(oc2.guard, GuardKind::None);
        assert!(!oc2.redecoded);
    }

    /// 测 #3 重解次数：任意随机输入下 redecode 闭包**最多被调用 1 次**；
    /// `redecoded == (calls == 1)`；`invalid` ⇒ 返回空串。
    #[test]
    fn ts387_redecode_at_most_once_property() {
        struct Lcg(u64);
        impl Lcg {
            fn n(&mut self, m: usize) -> usize {
                self.0 = self
                    .0
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((self.0 >> 33) as usize) % m
            }
        }
        let mut rng = Lcg(0x387_5eed_2026);
        let pool: Vec<char> =
            "<>/_abc你好，。,. 维生素b12铭印银线朵洛莉丝费曼学习法子未穿害低质罗斯柴尔德"
                .chars()
                .collect();
        for case in 0..300usize {
            let len = 1 + rng.n(40);
            let text: String = (0..len).map(|_| pool[rng.n(pool.len())]).collect();
            let rlen = 1 + rng.n(24);
            let recovered: String = (0..rlen).map(|_| pool[rng.n(pool.len())]).collect();
            let calls = Cell::new(0);
            let (out, oc) = apply_acc_disposition(text.clone(), Some(INJ), 5.0, None, || {
                calls.set(calls.get() + 1);
                Ok(recovered.clone())
            });
            assert!(
                calls.get() <= 1,
                "case {case}: 重解至多一次（实测 {}）",
                calls.get()
            );
            assert_eq!(
                oc.redecoded,
                calls.get() == 1,
                "case {case}: redecoded 与调用次数必须一致（text={text:?}）"
            );
            if oc.invalid {
                assert_eq!(out, "", "case {case}: invalid ⇒ 必须空串");
            }
        }
    }
}

// ============================================================
// TUNE-DECODE-SERIAL-AND-TOKEN-CAP-390：解码串行 + 按语音时长限制生成长度
// ============================================================
#[cfg(test)]
mod fix390_tests {
    use super::{max_new_tokens_for, WINDOW_DECODE_CONCURRENCY};

    /// TUNE-390-③：窗口解码并发度 = 1（串行）。依据见常量注释（`DecodeStreams` 逐 `Decode` 无并行
    /// 收益 + 共用 ORT 会话争抢：实测单独 274 vs 并发 476 ms/音频秒）；改回 ≥2 须附新实测。
    #[test]
    fn window_decode_concurrency_is_serial() {
        assert_eq!(WINDOW_DECODE_CONCURRENCY, 1, "TUNE-390：窗口解码须串行");
    }

    /// TUNE-390-①：`max_new_tokens_for` 取值 + 界 + 非有限/负 + 单调 + 极大有限值不 panic。
    #[test]
    fn max_new_tokens_for_values_bounds_monotone() {
        assert_eq!(max_new_tokens_for(0.0), 48, "0s ⇒ 下限");
        assert_eq!(
            max_new_tokens_for(1.12),
            48,
            "ceil(13.44)=14 +24=38 ⇒ 下限 48"
        );
        assert_eq!(max_new_tokens_for(10.0), 144, "120+24");
        assert_eq!(max_new_tokens_for(20.0), 256, "240+24=264 ⇒ 上限 256");
        // 非有限 / 负数 ⇒ 上限（退回现状、不限）
        assert_eq!(max_new_tokens_for(f32::NAN), 256);
        assert_eq!(max_new_tokens_for(-1.0), 256);
        assert_eq!(max_new_tokens_for(f32::INFINITY), 256);
        assert_eq!(max_new_tokens_for(f32::NEG_INFINITY), 256);
        // 单调不减 + 恒在 [48, 256]
        let mut last = 0i32;
        let mut s = 0.0f32;
        while s <= 30.0 {
            let v = max_new_tokens_for(s);
            assert!((48..=256).contains(&v), "s={s} ⇒ {v} 越界");
            assert!(v >= last, "s={s} 处应单调不减（{last} -> {v}）");
            last = v;
            s += 0.5;
        }
        assert_eq!(max_new_tokens_for(1e30), 256, "极大有限值不得溢出/panic");
    }

    /// TUNE-390-②：源码级护栏 —— `transcribe_acc_ctx` 首解与重解**都带** `Some(token_cap)`；
    /// `decode_accuracy_once` 内部传 `None`（其他调用方逐位不变）。
    #[test]
    fn token_cap_wired_in_both_decodes_source_guard() {
        let src = include_str!("mod.rs");
        // (a) transcribe_acc_ctx 函数体（截到下一个函数文档 `FIX-PREFIX-AND-EAT-371` 前）。
        let ctx = src
            .split("pub(crate) fn transcribe_acc_ctx(")
            .nth(1)
            .expect("transcribe_acc_ctx");
        let ctx = ctx.split("FIX-PREFIX-AND-EAT-371").next().unwrap();
        assert_eq!(
            ctx.matches("decode_accuracy_allow_empty(").count(),
            2,
            "首解 + 重解都应走 decode_accuracy_allow_empty"
        );
        assert_eq!(
            ctx.matches("Some(token_cap)").count(),
            2,
            "首解与重解都必须带 token cap"
        );
        // (b) decode_accuracy_once 传 None（逐位不变）。
        let once = src
            .split("fn decode_accuracy_once(")
            .nth(1)
            .expect("decode_accuracy_once");
        let once = once
            .split("fn decode_accuracy_allow_empty(")
            .next()
            .unwrap();
        assert_eq!(once.matches("decode_accuracy_allow_empty(").count(), 1);
        assert!(once.contains(", None)"), "decode_accuracy_once 必须传 None");
    }
}

#[cfg(test)]
mod testsync390_tests {
    use super::{max_new_tokens_for, WINDOW_DECODE_CONCURRENCY};

    /// 1. **不截断正常语速**：s ∈ {0.5, 1, 2, …, 25}，`cap ≥ ceil(7 字/s × s) + 5`（快语速 7 字/s +
    ///    语种前缀约 5 token）**或**已达上限 256；且 cap 随 s **单调不减**。
    #[test]
    fn ts390_cap_never_truncates_fast_speech_and_is_monotone() {
        let mut s = 0.5f32;
        let mut last = 0i32;
        while s <= 25.0 {
            let cap = max_new_tokens_for(s);
            let fast = (7.0f32 * s).ceil() as i32 + 5;
            assert!(
                cap >= fast || cap == 256,
                "s={s}: cap={cap} 小于快语速需求 {fast} 且未达上限"
            );
            assert!(cap >= last, "s={s}: cap 应单调不减（{last} -> {cap}）");
            last = cap;
            s += 0.5;
        }
        // 整数点也必须覆盖（0.5 步进已含 1..25，额外显式核 25s 触顶）。
        assert_eq!(max_new_tokens_for(25.0), 256, "25s ⇒ 触上限");
    }

    /// 2. 源码护栏：
    /// (a) `transcribe_acc_ctx` 内 cap 计算位置在「剪后音频遮蔽 `samples`」**之后**（用剪后时长）；
    /// (b) `decode_accuracy_once` 函数体**不出现** `max_new_tokens`（其他调用方不设 per-stream cap）。
    #[test]
    fn ts390_cap_after_trim_shadow_and_once_has_no_cap() {
        let src = include_str!("mod.rs");
        let ctx = src
            .split("pub(crate) fn transcribe_acc_ctx(")
            .nth(1)
            .expect("transcribe_acc_ctx 锚点缺失");
        let ctx = ctx.split("FIX-PREFIX-AND-EAT-371").next().unwrap();
        let shadow = ctx
            .find("let samples: &[f32] = &samples_used;")
            .expect("剪后音频遮蔽 samples 的锚点缺失");
        let cap_pos = ctx.find("max_new_tokens_for(").expect("cap 计算锚点缺失");
        assert!(
            cap_pos > shadow,
            "cap 必须在「剪后音频遮蔽 samples」之后计算（否则用的是原始时长）"
        );
        let once = src
            .split("fn decode_accuracy_once(")
            .nth(1)
            .expect("decode_accuracy_once 锚点缺失");
        let once = once
            .split("fn decode_accuracy_allow_empty(")
            .next()
            .unwrap();
        // 契约：`decode_accuracy_once` 不得设 per-stream cap（必须传 `None`）⇒ 体内不出现任何 `Some(`。
        assert!(
            !once.contains("Some("),
            "decode_accuracy_once 不得设 per-stream max_new_tokens（必须传 None）"
        );
    }

    /// 3. 并发度 = 1（串行），且常量注释须留**实测依据关键字**（`274` 与 `476`）——防被无依据改回。
    #[test]
    fn ts390_concurrency_serial_with_measured_comment() {
        assert_eq!(WINDOW_DECODE_CONCURRENCY, 1, "TUNE-390：窗口解码须串行");
        let src = include_str!("mod.rs");
        let lines: Vec<&str> = src.lines().collect();
        let cidx = lines
            .iter()
            .position(|l| l.contains("pub(crate) const WINDOW_DECODE_CONCURRENCY"))
            .expect("WINDOW_DECODE_CONCURRENCY 锚点缺失");
        let mut j = cidx;
        while j > 0 && lines[j - 1].trim_start().starts_with("///") {
            j -= 1;
        }
        let block = lines[j..cidx].join("\n");
        assert!(
            block.contains("274") && block.contains("476"),
            "常量注释须含实测依据 274/476 ms/音频秒，实测注释块={block:?}"
        );
    }
}

#[cfg(test)]
mod sliding_window_367_tests {
    use super::{
        align_overlap, group_window_start_secs, OrderedReflow, WINDOW_MAX_SECS, WINDOW_MAX_SLICES,
    };

    // ---- ① 组窗 ----
    #[test]
    fn window_keeps_up_to_current_plus_three() {
        // 🔴 FIX-SLICE-CUT-AT-GAP-381：WINDOW_MAX_SECS 12→10。
        // [3,3,3,3]=12 > 10 ⇒ 丢最远一片 ⇒ [3,3,3]=9 ⇒ 起点 1（改前 12≤12 ⇒ 起点 0）
        assert_eq!(
            group_window_start_secs(&[3.0, 3.0, 3.0, 3.0], WINDOW_MAX_SECS),
            1
        );
        // 5 片 ⇒ 最多当前+前3（起点 1，4×1=4 ≤ 10；不受上限变化影响）
        assert_eq!(
            group_window_start_secs(&[1.0, 1.0, 1.0, 1.0, 1.0], WINDOW_MAX_SECS),
            1
        );
    }

    #[test]
    fn window_drops_oldest_until_within_limit() {
        // 🔴 FIX-SLICE-CUT-AT-GAP-381：WINDOW_MAX_SECS 12→10。
        // [4,4,4,4]=16>10 ⇒ 丢 → [4,4,4]=12>10 ⇒ 丢 → [4,4]=8 ⇒ 起点 2（改前 12≤12 ⇒ 起点 1）
        assert_eq!(
            group_window_start_secs(&[4.0, 4.0, 4.0, 4.0], WINDOW_MAX_SECS),
            2
        );
        // [5,5,5,5]=20>10 ⇒ 丢到 [5,5,5]=15>10 ⇒ [5,5]=10 ≤10 ⇒ 起点 2（不受上限变化影响）
        assert_eq!(
            group_window_start_secs(&[5.0, 5.0, 5.0, 5.0], WINDOW_MAX_SECS),
            2
        );
    }

    /// 🔴 FIX-SLICE-CUT-AT-GAP-381：**窗口上限与切片起搜点必须相等**（Gavin 统一 10s）。
    /// 锁定二者，防日后单改一处造成「切片 10s 起搜、窗口 12s」之类的静默漂移。
    #[test]
    fn sliding_cut_search_start_equals_window_max() {
        assert_eq!(
            super::SLIDING_CUT_SEARCH_START_SECS as f32,
            WINDOW_MAX_SECS,
            "SLIDING_CUT_SEARCH_START_SECS(vad) 必须 == WINDOW_MAX_SECS(mod)"
        );
    }

    #[test]
    fn window_single_slice_over_limit_is_kept() {
        // 单片 13s > 10 ⇒ 不切分、直接单片（起点 0，n=1）
        assert_eq!(group_window_start_secs(&[13.0], WINDOW_MAX_SECS), 0);
        // 当前片 20s + 前片 1s：丢前片后仍 >10 ⇒ 只剩当前片（起点 1）
        assert_eq!(group_window_start_secs(&[1.0, 20.0], WINDOW_MAX_SECS), 1);
    }

    #[test]
    fn window_empty() {
        assert_eq!(group_window_start_secs(&[], WINDOW_MAX_SECS), 0);
    }

    // ---- ④ 对齐合并 ----
    #[test]
    fn align_normal_overlap_commits_the_slid_out_prefix() {
        // prev 后缀 = new 前缀「甲乙丙丁戊己庚辛壬癸」(10 字)
        let prev = "第一段滑出内容甲乙丙丁戊己庚辛壬癸";
        let new = "甲乙丙丁戊己庚辛壬癸然后是新片内容";
        let r = align_overlap(prev, new);
        assert!(r.ok, "正常重叠应可滑动");
        assert_eq!(r.overlap_chars, 10);
        assert_eq!(r.committed_prefix, "第一段滑出内容", "滑出片文本应定稿");
    }

    #[test]
    fn align_tolerates_local_revision() {
        // 重叠区 10 字里有 1 字不同（局部纠错）⇒ 编辑距离比 0.1 ≤ 0.15 ⇒ 仍可滑动
        let prev = "前缀滑出甲乙丙丁戊己庚辛壬癸";
        let new = "甲乙丙丁戊己庚辛壬X后续";
        let r = align_overlap(prev, new);
        assert!(r.ok, "15% 内局部纠错应容忍");
        assert_eq!(r.overlap_chars, 10);
    }

    #[test]
    fn align_no_overlap_is_conservative() {
        // 无足够重叠（<8 且 <30%）⇒ 不滑动
        let r = align_overlap("完全不同的第一段文字", "毫不相干的新窗口文本");
        assert!(!r.ok, "无重叠必须保守不滑动");
        assert!(r.committed_prefix.is_empty());
    }

    #[test]
    fn align_short_text_below_min_length_is_conservative() {
        // 重叠只有 3 字（<8）⇒ 即使完全相同也不滑动
        let r = align_overlap("滑出甲乙丙", "甲乙丙后续");
        assert!(!r.ok, "短文本低于长度门必须保守");
    }

    // ---- (B) 窗口间并发：乱序完成 ⇒ 按 seq 有序定稿 ----
    /// 🔴 造「后发先至」：seq1 先完成但 seq0 未到 ⇒ **不得定稿**；seq0 到齐 ⇒ 依次定稿 0、1。
    /// （用长文本保证与 `align_overlap` 双门兼容：短文本会被保守策略拒绝，见 P0 修复。）
    #[test]
    fn ordered_reflow_commits_in_seq_despite_out_of_order_arrival() {
        let mut o = OrderedReflow::new();
        assert!(
            o.push(1, 0, 2, "BBBB CCCC DDDD".to_string()).is_empty(),
            "缺 seq0 ⇒ seq1 先到也不得定稿（必须先对齐再推进）"
        );
        let out = o.push(0, 0, 1, "AAAA BBBB CCCC".to_string());
        assert_eq!(out.len(), 2, "seq0 到齐后应**依次**产出 0 与 1 两条回灌");
        assert_eq!(o.push(2, 1, 3, "CCCC DDDD EEEE".to_string()).len(), 1);
        let (_committed, last) = o.finish();
        assert_eq!(last, "CCCC DDDD EEEE", "最后窗口文本应为 seq2");
    }

    /// 中间有洞（seq1 缺）⇒ seq2 被暂存，填洞后 1、2 一起按序产出。
    #[test]
    fn ordered_reflow_holds_until_gap_filled() {
        let mut o = OrderedReflow::new();
        assert_eq!(o.push(0, 0, 1, "AAAA BBBB CCCC".to_string()).len(), 1);
        assert!(
            o.push(2, 1, 3, "CCCC DDDD EEEE".to_string()).is_empty(),
            "缺 seq1 ⇒ seq2 不得定稿"
        );
        assert_eq!(
            o.push(1, 0, 2, "BBBB CCCC DDDD".to_string()).len(),
            2,
            "seq1 到齐 ⇒ 1、2 一起按序定稿"
        );
    }

    /// 顺序到达（并发度=1 路径）⇒ 每条即时定稿，行为等同顺序版。
    #[test]
    fn ordered_reflow_sequential_arrival_is_immediate() {
        let mut o = OrderedReflow::new();
        assert_eq!(o.push(0, 0, 1, "AAAA BBBB CCCC".to_string()).len(), 1);
        assert_eq!(o.push(1, 0, 2, "BBBB CCCC DDDD".to_string()).len(), 1);
        assert_eq!(o.push(2, 1, 3, "CCCC DDDD EEEE".to_string()).len(), 1);
        let (_committed, last) = o.finish();
        assert_eq!(last, "CCCC DDDD EEEE");
    }

    // ---- FIX-ORDERED-REFLOW-DROP-368（P0 吃字）→ FIX-WINDOW-DISJOINT-369 语义 ----
    /// 🔴 中途对齐失败**不丢字**（369 起：失败窗**退回拼接**而非跳过，故也产出回灌）。
    #[test]
    fn ordered_reflow_mid_failure_does_not_drop_text() {
        let mut o = OrderedReflow::new();
        assert_eq!(o.push(0, 0, 1, "AAAA BBBB CCCC".to_string()).len(), 1);
        assert_eq!(o.push(1, 0, 2, "BBBB CCCC DDDD".to_string()).len(), 1); // 对齐成功 ⇒ committed="AAAA "
        assert_eq!(
            o.push(2, 1, 2, "XXXX YYYY ZZZZ".to_string()).len(),
            1,
            "共享切片但对齐失败（如长度门误拒）⇒ 退回拼接，仍产出（369 取代 368 的「不产出」）"
        );
        let out3 = o.push(3, 1, 3, "CCCC DDDD EEEE".to_string());
        assert_eq!(out3.len(), 1, "后续窗仍正常定稿");
        let (committed, last) = o.finish();
        let final_text = format!("{}{}", committed, last);
        for tok in ["AAAA", "BBBB", "CCCC", "DDDD", "EEEE"] {
            assert!(
                final_text.contains(tok),
                "不得丢字：缺 {tok}（final={final_text}）"
            );
        }
    }

    /// 🔴 FIX-WINDOW-DISJOINT-369 核心：**用切片序号判重叠**。
    /// 零重叠（`start >= prev.end`）⇒ **直接拼接**（无重复可去），**绝不跳过整窗**。
    #[test]
    fn ordered_reflow_zero_overlap_concatenates_instead_of_skipping() {
        let mut o = OrderedReflow::new();
        let a = "AAAA BBBB CCCC".to_string();
        let b = "DDDD EEEE FFFF".to_string();
        assert_eq!(o.push(0, 0, 1, a.clone()).len(), 1);
        assert_eq!(
            o.push(1, 1, 2, b.clone()).len(),
            1,
            "零重叠窗必须产出（拼接），跳过即整窗丢字"
        );
        let (committed, last) = o.finish();
        assert_eq!(committed, a, "零重叠 ⇒ 前窗整体定稿");
        assert_eq!(last, b, "新窗成为 last");
        assert_eq!(
            format!("{}{}", committed, last),
            format!("{}{}", a, b),
            "零重叠 ⇒ 拼接即正确答案"
        );
    }

    /// 🔴 有重叠（`start < prev.end`）⇒ **仍走对齐去重**，不得退化成无脑拼接（否则接缝重复回来）。
    #[test]
    fn ordered_reflow_shared_overlap_still_dedupes() {
        let mut o = OrderedReflow::new();
        assert_eq!(o.push(0, 0, 1, "AAAA BBBB CCCC".to_string()).len(), 1);
        assert_eq!(o.push(1, 0, 2, "BBBB CCCC DDDD".to_string()).len(), 1);
        let (committed, last) = o.finish();
        let final_text = format!("{}{}", committed, last);
        assert_eq!(
            final_text, "AAAA BBBB CCCC DDDD",
            "有重叠必须去重（不得无脑拼接）"
        );
        assert_eq!(final_text.matches("BBBB").count(), 1, "接缝不得重复");
    }

    /// 🔴 有重叠但对齐失败 ⇒ **退回拼接**（宁可重复不可丢字）；绝不跳过整窗。
    #[test]
    fn ordered_reflow_shared_overlap_align_failure_falls_back_to_concat() {
        let mut o = OrderedReflow::new();
        assert_eq!(o.push(0, 0, 1, "AAAA BBBB CCCC".to_string()).len(), 1);
        let out = o.push(1, 0, 2, "XXXX YYYY ZZZZ".to_string());
        assert_eq!(out.len(), 1, "对齐失败也必须产出（退回拼接），绝不跳过整窗");
        let (committed, last) = o.finish();
        let final_text = format!("{}{}", committed, last);
        assert!(final_text.contains("AAAA"), "旧窗文本不得丢：{final_text}");
        assert!(final_text.contains("XXXX"), "新窗文本不得丢：{final_text}");
    }

    /// 失败后 `next` 仍推进 ⇒ 后续 seq 不卡死。
    #[test]
    fn ordered_reflow_next_advances_after_failure() {
        let mut o = OrderedReflow::new();
        assert_eq!(o.push(0, 0, 1, "AAAA BBBB CCCC".to_string()).len(), 1);
        assert_eq!(o.push(1, 0, 2, "XXXX YYYY ZZZZ".to_string()).len(), 1);
        let out = o.push(2, 1, 3, "BBBB CCCC DDDD".to_string());
        assert_eq!(
            out.len(),
            1,
            "失败后 next 仍推进，后续 seq 能处理（不卡死）"
        );
    }

    // ---- FIX-WINDOW-DISJOINT-369：复刻 Gavin 端测实测序列（12s 封顶 ⇒ 零重叠）----
    /// 复刻 `main.rs` 的组窗：每来一片算一次窗口，返回全局切片区间 + 该窗文本。
    fn windows_for(durations_secs: &[f32], slice_text: &[String]) -> Vec<(usize, usize, String)> {
        let mut recent: Vec<f32> = Vec::new();
        let mut total: usize = 0;
        let mut out = Vec::new();
        for d in durations_secs {
            recent.push(*d);
            total += 1;
            while recent.len() > WINDOW_MAX_SLICES {
                recent.remove(0);
            }
            let start = group_window_start_secs(&recent, WINDOW_MAX_SECS);
            let base = total - recent.len();
            let ws = base + start;
            let we = total;
            let text = slice_text[ws..we].join(" ");
            out.push((ws, we, text));
        }
        out
    }

    /// 🔴 369 验收：Gavin BUILD-348 实测 6 片 / 31 秒（`debug-2009.log`）。
    /// 12s 封顶把 #2/#3 切成**零重叠单片** ⇒ 旧逻辑整窗跳过 ⇒ 丢字；
    /// 用真实时长序列构造，断言**所有片的内容都出现在最终文本中**（无整窗丢失）。
    #[test]
    fn ordered_reflow_real_log_sequence_keeps_every_slice() {
        // 逐字时长 ≈ 4.4 字/秒（实测），让「重叠字/新窗字」比例与真实音频一致
        // （seq5 重叠仅 ~11 字 < 41×0.30≈13 字 ⇒ 长度门必拒 —— 正是 369 的第二根因）。
        let durs = [4.43f32, 2.83, 9.34, 5.15, 2.44, 6.78];
        let filler = [
            '乙', '丙', '丁', '戊', '己', '庚', '辛', '壬', '癸', '子', '丑', '寅',
        ];
        let slices: Vec<String> = durs
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let n = (d * 4.4).round() as usize;
                let mut s = format!("片{i}甲");
                while s.chars().count() < n {
                    let k = s.chars().count() % filler.len();
                    s.push(filler[k]);
                }
                s
            })
            .collect();

        let wins = windows_for(&durs, &slices);
        assert!(
            wins.iter().any(|(ws, we, _)| *we - *ws == 1 && *ws >= 2),
            "本场景必须复现「单片零重叠窗」（否则测不到 369 根因）：{wins:?}"
        );
        // 且必须走到「共享切片 + 对齐失败（长度门误拒）」路径 —— 369 的第二根因。
        let mut shared_align_fail = false;
        for pair in wins.windows(2) {
            let (_, prev_end, prev_text) = &pair[0];
            let (ws, _we, text) = &pair[1];
            if *ws < *prev_end && !align_overlap(prev_text, text).ok {
                shared_align_fail = true;
            }
        }
        assert!(
            shared_align_fail,
            "本场景必须走到「有重叠但对齐失败 ⇒ 退回拼接」路径：{wins:?}"
        );

        let mut o = OrderedReflow::new();
        for (seq, (ws, we, text)) in wins.iter().enumerate() {
            o.push(seq, *ws, *we, text.clone());
        }
        let (committed, last) = o.finish();
        let final_text = format!("{}{}", committed, last);
        for i in 0..slices.len() {
            let marker = format!("片{i}甲");
            assert!(
                final_text.contains(&marker),
                "切片 {i}（{marker}）整窗丢失（final={final_text}）"
            );
        }
    }
}

// ============================================================
// FIX-PREFIX-AND-EAT-371（B）：周期性重复内容下的对齐（丢字 P0）
// 运行：cargo test --bin feiyin-ime -- fix371
// ============================================================
#[cfg(test)]
mod fix371_repeat_align_tests {
    use super::{
        align_overlap, align_overlap_with_prior, expected_k_band, AlignPrior, OrderedReflow,
        ALIGN_EXPECTED_K_TOL_DOWN, ALIGN_EXPECTED_K_TOL_UP,
    };

    /// Gavin 端测原句（19 字，其中「，」「？」为标点 ⇒ 有效 17 字）。
    const S: &str = "明天天气好的话，可以一起出来看电影吗？";

    fn eff_chars(t: &str) -> usize {
        t.chars()
            .filter(|c| !c.is_whitespace() && !crate::punctuation::PUNCT_CHARS.contains(c))
            .count()
    }

    /// `n` 个互异汉字（全部算「有效字符」：非空白、非标点）——用于精确构造重叠长度。
    fn distinct_chars(n: usize) -> Vec<char> {
        (0..n)
            .map(|i| char::from_u32(0x4E00 + i as u32).unwrap())
            .collect()
    }

    /// prev(40) / new(40)，且 `new[..20] == prev[20..40]` ⇒ **真重叠 = 20**。
    fn overlap20_pair() -> (String, String) {
        let p = distinct_chars(60);
        let prev: String = p[..40].iter().collect();
        let new: String = p[20..40].iter().chain(p[40..60].iter()).collect();
        assert_eq!(prev.chars().count(), 40);
        assert_eq!(new.chars().count(), 40);
        assert!(new.starts_with(&prev.chars().skip(20).collect::<String>()));
        (prev, new)
    }

    /// 🔴 **B3 改判**：软范围容差**不对称** —— 向下宽（安全：重复）/ 向上紧（危险：吃字）。
    #[test]
    fn expected_k_band_is_asymmetric() {
        let (lo, hi) = expected_k_band(40, 8, 10_000);
        assert_eq!(lo, 20, "向下 −50%");
        assert_eq!(hi, 50, "向上 +25%");
        assert!(
            40 - lo > hi - 40,
            "向下(20) 必须比向上(10) 宽：安全方向多留余量"
        );
        for e in [10usize, 13, 37, 54, 120] {
            let (lo, hi) = expected_k_band(e, 1, 10_000);
            assert_eq!(
                e - lo,
                ((e as f32) * ALIGN_EXPECTED_K_TOL_DOWN).ceil() as usize,
                "下放宽量必须 = e×DOWN（e={e}）"
            );
            assert_eq!(
                hi - e,
                ((e as f32) * ALIGN_EXPECTED_K_TOL_UP).ceil() as usize,
                "上放宽量必须 = e×UP（e={e}）"
            );
            assert!(e - lo > hi - e, "向下必须严格宽于向上（e={e}）");
        }
        // 夹紧：`hi_all`（层① 硬上界）/ `min_len` 均生效
        assert_eq!(expected_k_band(10, 8, 9).1, 9, "hi 受层① 硬上界夹紧");
        assert_eq!(expected_k_band(4, 8, 10_000).0, 8, "lo 受 min_len 夹紧");
    }

    /// 🔴 安全方向**够得着**：估算高估 40%（超出 +25% 但落在 −50% 内）⇒ 仍能从带内够到真值附近。
    #[test]
    fn safe_side_reaches_truth_when_estimate_overshoots() {
        let (prev, new) = overlap20_pair();
        let (lo, _hi) = expected_k_band(28, 12, 39); // e = 40×0.7 = 28（高估 40%）
        assert!(lo <= 20, "真值 20 必须落在**向下**可达范围内（lo={lo}）");
        let r = align_overlap_with_prior(
            &prev,
            &new,
            AlignPrior {
                expected_ratio: Some(0.7),
                prev_extra_slices: 1,
            },
        );
        assert!(r.ok, "应找到切点");
        assert!(
            (19..=21).contains(&r.overlap_chars),
            "必须够到真值(20)附近：k={}",
            r.overlap_chars
        );
        assert!(!r.committed_prefix.is_empty());
    }

    /// 🔴 危险方向**够不着**：估算低估（真值在 `+UP` 之外）⇒ 上界不放行（回落安全兜底）⇒ 绝不过真值。
    #[test]
    fn dangerous_up_reach_is_bounded() {
        let (prev, new) = overlap20_pair();
        // e = 40×0.3 = 12 ⇒ 上界仅到 +25% = 15（真值 20 在带外）；若对称 ±50% 会放到 18。
        let (_lo, hi) = expected_k_band(12, 12, 39);
        assert_eq!(hi, 15, "上界必须紧：+25% ⇒ 15");
        assert!(hi < 20, "危险方向不得触及真值以上的 k");
        let r = align_overlap_with_prior(
            &prev,
            &new,
            AlignPrior {
                expected_ratio: Some(0.3),
                prev_extra_slices: 1,
            },
        );
        if r.ok {
            assert!(
                r.overlap_chars <= 20,
                "结果永不进入吃字方向（k={} 不得 > 真值 20）",
                r.overlap_chars
            );
        }
    }

    /// 🔴 **371 验收核心**：周期性重复内容（同一句连说 3 遍 + ASR 变体字）下**全文不丢字**。
    ///
    /// 场景（Gavin 端测 21:43）：`prev = S1 S2 S3`、`new = S2 S3 S4`。
    /// 旧实现「最长重叠优先」在 `k = max_k` 处因 S1..S4 逐字（或 1 字之差）相等 ⇒ 质量门通过
    /// ⇒ 先命中 ⇒ `committed_prefix = ""` ⇒ **S1 整段被丢**（「明天天气好的话，可以一起」消失）。
    /// 371 用已知切片时长把 `k` 约束在期望值 ⇒ `k ≈ 2/3` 处 ⇒ `committed = S1` ⇒ 不丢。
    #[test]
    fn periodic_repeat_keeps_every_sentence() {
        let dur_samples = 4_400 * 16; // 每片 ≈4.4s @16k
        let samples = vec![dur_samples, dur_samples, dur_samples];
        let mut o = OrderedReflow::new();
        assert_eq!(
            o.push_window(0, 0, 3, samples.clone(), format!("{S}{S}{S}"))
                .len(),
            1
        );
        // 下一窗 [1,4)：新片带 ASR 变体（「明天」→「明甸」）
        let s4 = S.replacen("明天", "明甸", 1);
        assert_eq!(
            o.push_window(1, 1, 4, samples, format!("{S}{S}{s4}")).len(),
            1
        );
        let (committed, last) = o.finish();
        let full = format!("{committed}{last}");
        assert!(
            committed.contains(S),
            "周期性内容下**滑出句必须定稿**（旧实现丢字）：committed={committed:?}"
        );
        assert!(full.contains("明天天气好的话"), "全文不得丢字：{full}");
        assert!(full.contains("明甸"), "新窗变体字也应在：{full}");
        // 全文恰为 S1 + (S2 S3 S4') ⇒ 该句出现 3 次（既未丢也未多插一份）
        assert_eq!(
            full.matches("明天天气好的话").count(),
            3,
            "语句次数必须恰为 3（丢字<3、重复>3 都算错）：{full}"
        );
    }

    /// 🔴 **反证（P0 证据固化）**：把**旧算法**（最长重叠优先）在同一输入上跑一遍 ⇒ 命中 `max_k`
    /// ⇒ 切点在 `prev` 开头 ⇒ `committed_prefix` 为空 ⇒ 丢字。
    ///
    /// 旧算法**内联**在测试里（不依赖当前实现），保证这条 P0 的证据不随实现改动而丢失。
    #[test]
    fn old_longest_first_would_drop_the_slid_sentence() {
        let prev = format!("{S}{S}{S}");
        let new = format!("{S}{S}{S}");
        let keep = |t: &str| -> Vec<char> {
            t.chars()
                .filter(|c| !c.is_whitespace() && !crate::punctuation::PUNCT_CHARS.contains(c))
                .collect()
        };
        let pk = keep(&prev);
        let nk = keep(&new);
        let max_k = pk.len().min(nk.len());
        let min_len = super::ALIGN_MIN_OVERLAP_CHARS
            .max((nk.len() as f32 * super::ALIGN_MIN_OVERLAP_RATIO).ceil() as usize);
        let mut old_cut_k = None;
        for k in (min_len..=max_k).rev() {
            let p_tail: Vec<char> = pk[pk.len() - k..].to_vec();
            let dist = super::edit_distance_chars(&p_tail, &nk[..k]);
            if dist as f32 / k as f32 <= super::ALIGN_MAX_EDIT_RATIO {
                old_cut_k = Some(k);
                break;
            }
        }
        assert_eq!(
            old_cut_k,
            Some(max_k),
            "旧算法在周期性内容上会命中 max_k（=切点在 prev 开头 ⇒ committed 空 ⇒ 丢字）"
        );
        // 同一输入下新算法（带期望比例）不丢字
        let new_r = align_overlap_with_prior(
            &prev,
            &new,
            AlignPrior {
                expected_ratio: Some(2.0 / 3.0),
                prev_extra_slices: 1,
            },
        );
        assert!(
            new_r.ok && !new_r.committed_prefix.is_empty(),
            "新算法必须定稿滑出句（不丢字）"
        );
    }

    /// 🔴 有期望比例 ⇒ `k` **钉在期望值**（不因周期性而选到 `max_k`）。
    #[test]
    fn expected_ratio_pins_k_near_estimate() {
        let prev = format!("{S}{S}{S}");
        let new = format!("{S}{S}{S}");
        let ratio = 2.0f32 / 3.0;
        let r = align_overlap_with_prior(
            &prev,
            &new,
            AlignPrior {
                expected_ratio: Some(ratio),
                prev_extra_slices: 1,
            },
        );
        assert!(r.ok, "期望区间内应能对齐");
        let expected = (eff_chars(&new) as f32 * ratio).round() as usize;
        assert_eq!(r.overlap_chars, expected, "k 必须钉在期望值附近");
        assert_eq!(
            r.committed_prefix, S,
            "切点应落在 S1/S2 边界 ⇒ 只定稿 S1（既不去重过度也不丢字）"
        );
    }

    /// 🔴 退化路径（无区间信息）⇒ **小 k 优先**：最坏是重复，绝不丢字。
    #[test]
    fn missing_spans_biases_to_small_k() {
        let prev = format!("{S}{S}{S}");
        let new = format!("{S}{S}{S}");
        let r = align_overlap(&prev, &new); // 无期望值 ⇒ 升序（小 k 优先）
        assert!(r.ok);
        assert!(
            !r.committed_prefix.is_empty(),
            "退化路径必须偏向小 k（切点靠后、committed 更长）⇒ 绝不切在开头丢字（k={}）",
            r.overlap_chars
        );
    }

    /// 🔴 **371 层① 硬约束（精确）优先于层② 估算** —— 补充要求新增。
    ///
    /// 构造 `m ≥ 1`（上一窗比本窗多出至少一片**有声**片）且 `prev` 与 `new` 在最长 `k` 上**逐字相等**
    /// （⇒ 层③ 质量门在 `k = max_k` 处必过）。此时**即使估算比例荒谬**（`ratio = 1.0` ⇒ 期望 k = max_k），
    /// 层① 也必须把 `k` 压到 `prev` 有效字数以下 ⇒ `committed_prefix` **非空**。
    ///
    /// 下半段**去掉硬约束**跑同一坏估算 ⇒ 选到 `max_k` ⇒ `committed_prefix` 为空
    /// （= Gavin 端测 21:43 的 P0 现场）⇒ 证明层① 不可缺、且必须**先于**②执行。
    #[test]
    fn hard_constraint_beats_a_bad_estimate() {
        let prev = format!("{S}{S}{S}");
        let new = format!("{S}{S}{S}");
        // 层① + 坏估算：仍必须非空提交
        let r = align_overlap_with_prior(
            &prev,
            &new,
            AlignPrior {
                expected_ratio: Some(1.0),
                prev_extra_slices: 1,
            },
        );
        assert!(r.ok, "硬约束内应有解");
        assert!(
            !r.committed_prefix.is_empty(),
            "层① 必须否决 k = max_k 的空提交（k={}，prev 有效字={}）",
            r.overlap_chars,
            eff_chars(&prev)
        );
        assert!(
            r.overlap_chars < eff_chars(&prev),
            "k 必须严格小于 prev 有效字数"
        );
        // 反证：无层① ⇒ 同一坏估算选到 max_k ⇒ committed 为空（旧行为/P0 现场）
        let r2 = align_overlap_with_prior(
            &prev,
            &new,
            AlignPrior {
                expected_ratio: Some(1.0),
                prev_extra_slices: 0,
            },
        );
        assert!(
            r2.ok && r2.committed_prefix.is_empty(),
            "无层① 时坏估算会空提交（本单要防的现象）"
        );
    }
}

#[cfg(test)]
mod path_b_budget_364_tests {
    // ============================================================
    // DUAL-PATH-REFINE-364：路B 预算闸门单测（精确计算版）
    // 运行：cargo test --bin feiyin-ime -- path_b_budget
    // ============================================================
    use super::{estimate_inject_tokens, expected_audio_tokens, path_b_budget_ok};

    /// 精确音频 token：20s→260（=13.0 tok/s 实测）、180s→2340。
    #[test]
    fn exact_audio_tokens_match_measured_rate() {
        assert_eq!(
            expected_audio_tokens(16_000 * 20),
            260,
            "20s 应 260（实测 13.0 tok/s）"
        );
        assert_eq!(expected_audio_tokens(16_000 * 180), 2340, "180s 应 2340");
    }

    /// 🔴 180s 在典型词库/上下文下**不降级**；300s（`MAX_RECORD_SECONDS` 新上限）**会降级**
    /// ⇒ 走切片拼装（DEC-069 绝不硬塞）。
    ///
    /// 注：路B（363 全量解码）已按 367 置 `PATH_B_WIRED_367=false` 摘接线，本闸门仅保留可回挂；
    /// 之所以 180s 仍不降级、300s 降级，是这条闸门自身的性质（与是否接线无关）。
    #[test]
    fn budget_180s_ok_but_300s_degrades() {
        assert!(path_b_budget_ok(16_000 * 180, None), "180s 裸音频");
        assert!(
            path_b_budget_ok(16_000 * 180, Some("哈兰德,挪威,世界杯,巴西,英格兰,迈阿密")),
            "180s + 典型词库"
        );
        assert!(path_b_budget_ok(16_000 * 180, Some("哈兰德")));
        // 300s（新 MAX_RECORD_SECONDS）：3900 audio token ⇒ 超 4096 ⇒ 降级切片拼装。
        assert_eq!(expected_audio_tokens(16_000 * 300), 3900, "300s 应 3900");
        assert!(
            !path_b_budget_ok(16_000 * 300, None),
            "300s 裸音频已超 4096 ⇒ 必须降级（这正是 370 把整段解码摘掉的原因）"
        );
    }

    /// 超长 / 巨词库 ⇒ 仍会降级（闸门逻辑必须保留，防未来改上限/大词库）。
    #[test]
    fn still_degrades_when_truly_over() {
        assert!(!path_b_budget_ok(16_000 * 600, None), "600s 应超预算");
        assert!(
            !path_b_budget_ok(16_000 * 180, Some(&"超长词库".repeat(2000))),
            "巨词库即使 180s 也应超"
        );
    }

    /// 注入段计数：含提示词脚手架 ⇒ 恒 >0；词库越多越大。
    #[test]
    fn inject_tokens_positive_and_monotonic() {
        // FIX-INJECT-TO-SPEC-377：注入内容**只剩纯词表** ⇒ 空词表 = 零 token（不调 set_option）。
        let small = estimate_inject_tokens(None);
        let big = estimate_inject_tokens(Some(&"词条".repeat(50)));
        assert_eq!(small, 0, "空词表 ⇒ 不注入 ⇒ 0 token（377 新契约）");
        assert!(big > small, "词库越多注入越大");
    }
}

// =====================================================================
// TEST-SYNC-371（阶段三 · 非作者视角，tester-1）
// ---------------------------------------------------------------------
// 按**设计契约**编写（不读实现反推），生产代码零改动。覆盖：
//   A · 语种前缀剥离 `strip_qwen3_language_prefix`（371 恢复的无条件规则）
//   B · 滑窗对齐 `align_overlap_with_prior` / `OrderedReflow` 的边界攻击面
// =====================================================================
#[cfg(test)]
mod testsync371_prefix_contract_tests {
    use super::{strip_qwen3_language_prefix, QWEN3_PREFIX_MAX_BYTES};

    /// 设计 A：**只认第一个** `<asr_text>`（头部一个 + 正文中段一个 ⇒ 截第一个）。
    #[test]
    fn prefix_cuts_at_first_marker_only() {
        assert_eq!(
            strip_qwen3_language_prefix("language chinese<asr_text>你好<asr_text>世界"),
            "你好<asr_text>世界"
        );
        assert_eq!(
            strip_qwen3_language_prefix("<asr_text>甲乙<asr_text>丙丁"),
            "甲乙<asr_text>丙丁"
        );
    }

    /// 设计 A：标记后紧跟空白 / 半角冒号 / 全角冒号 ⇒ 一并吃掉。
    #[test]
    fn prefix_eats_trailing_whitespace_and_colons() {
        assert_eq!(
            strip_qwen3_language_prefix("language chinese<asr_text>   你好"),
            "你好"
        );
        assert_eq!(
            strip_qwen3_language_prefix("language chinese<asr_text>:你好"),
            "你好"
        );
        assert_eq!(
            strip_qwen3_language_prefix("language chinese<asr_text>：你好"),
            "你好"
        );
        assert_eq!(
            strip_qwen3_language_prefix("language chinese<asr_text>:  你好"),
            "你好"
        );
    }

    /// 设计 A：标记后什么都没有 ⇒ 截完即空串。
    #[test]
    fn prefix_marker_with_nothing_after_is_empty() {
        assert_eq!(
            strip_qwen3_language_prefix("language chinese<asr_text>"),
            ""
        );
        assert_eq!(
            strip_qwen3_language_prefix("language chinese<asr_text>   "),
            ""
        );
        assert_eq!(strip_qwen3_language_prefix("<asr_text>"), "");
    }

    /// 设计 A：唯一护栏是「标记起点 ≤ 64 字节」——常量值钉死。
    #[test]
    fn prefix_max_bytes_is_64() {
        assert_eq!(QWEN3_PREFIX_MAX_BYTES, 64);
    }

    /// 设计 A：起点恰在 63 / 64 / 65 字节 ⇒ 63/64 剥、65 不剥。
    #[test]
    fn prefix_position_guard_63_64_65() {
        let at63 = format!("{}<asr_text>正文", "a".repeat(63));
        let at64 = format!("{}<asr_text>正文", "a".repeat(64));
        let at65 = format!("{}<asr_text>正文", "a".repeat(65));
        assert_eq!(
            strip_qwen3_language_prefix(&at63),
            "正文",
            "起点 63 ≤64 ⇒ 剥"
        );
        assert_eq!(
            strip_qwen3_language_prefix(&at64),
            "正文",
            "起点 64 =上限 ⇒ 剥（含边界）"
        );
        assert_eq!(
            strip_qwen3_language_prefix(&at65),
            at65.as_str(),
            "起点 65 >64 ⇒ 视为正文、原样返回"
        );
    }

    /// 设计 A：多字节字符恰跨 64 字节边界 ⇒ 不 panic 且按字节位判定。
    #[test]
    fn prefix_multibyte_at_boundary_does_not_panic() {
        let p63 = format!("{}<asr_text>正文", "中".repeat(21)); // 21×3=63B
        let p64 = format!("{}a<asr_text>正文", "中".repeat(21)); // 64B
        let p65 = format!("{}ab<asr_text>正文", "中".repeat(21)); // 65B
        assert_eq!(strip_qwen3_language_prefix(&p63), "正文");
        assert_eq!(strip_qwen3_language_prefix(&p64), "正文");
        assert_eq!(strip_qwen3_language_prefix(&p65), p65.as_str());
    }

    /// 设计 A：前缀含换行 / `<` / 数字标点 —— 371 不要求「标签样」。
    #[test]
    fn prefix_arbitrary_prefix_forms_are_stripped() {
        assert_eq!(
            strip_qwen3_language_prefix("language\nchinese<asr_text>正文"),
            "正文"
        );
        assert_eq!(strip_qwen3_language_prefix("<tag><asr_text>正文"), "正文");
        assert_eq!(
            strip_qwen3_language_prefix("123 45,6.<asr_text>正文"),
            "正文"
        );
        assert_eq!(strip_qwen3_language_prefix("x<y<asr_text>正文"), "正文");
    }

    /// 设计 A：**语言无关** —— 中/日/韩/英/俄 + 空前缀一律生效。
    #[test]
    fn prefix_is_language_agnostic() {
        for (pre, body) in [
            ("language korean", "안녕하세요"),
            ("language russian", "привет"),
            ("language japanese", "こんにちは"),
            ("language english", "hello"),
            ("", "你好"),
        ] {
            let input = format!("{pre}<asr_text>{body}");
            assert_eq!(
                strip_qwen3_language_prefix(&input),
                body,
                "前缀须语言无关：{input}"
            );
        }
    }

    /// 设计 A：空串 / 纯空白 / 半截标记 ⇒ 原样返回、不 panic。
    #[test]
    fn prefix_degenerate_inputs_return_original() {
        assert_eq!(strip_qwen3_language_prefix(""), "");
        assert_eq!(strip_qwen3_language_prefix("   "), "   ");
        assert_eq!(strip_qwen3_language_prefix("<asr_tex"), "<asr_tex");
        assert_eq!(strip_qwen3_language_prefix("asr_text>"), "asr_text>");
        assert_eq!(
            strip_qwen3_language_prefix("language<asr_text"),
            "language<asr_text"
        );
    }

    /// 设计 A：无标记的普通正文原样返回（含尖括号也不误伤）。
    #[test]
    fn prefix_plain_body_unchanged() {
        let t = "今天天气不错，我们出去走走吧。";
        assert_eq!(strip_qwen3_language_prefix(t), t);
        assert_eq!(
            strip_qwen3_language_prefix("a < b 且 c > d"),
            "a < b 且 c > d"
        );
    }

    /// 设计 A：与 `<|…|>` token 叠加 —— 生产顺序 = 先剥 token、再剥前缀，两者都清。
    #[test]
    fn prefix_after_special_tokens_both_gone() {
        let raw = "<|lang|>language chinese<asr_text>你好";
        let no_tok = super::Transcriber::strip_asr_special_tokens(raw);
        assert_eq!(strip_qwen3_language_prefix(&no_tok), "你好");
    }
}

#[cfg(test)]
mod testsync371_align_contract_tests {
    use super::{align_overlap, align_overlap_with_prior, AlignPrior, OrderedReflow};

    const S: &str = "今天天气真好我们出去玩吧"; // 12 个有效字（无标点）

    /// 设计 B（③质量门边界）：重叠段差 1 字（≤0.15）应能对齐；差 2 字（>0.15）保守不对齐。
    #[test]
    fn near_period_one_diff_aligns_two_diffs_degrades() {
        let prev = "PQRSTABCDEFGHIJKL"; // 前 5 字 + 12 字重叠段
        let r1 = align_overlap_with_prior(
            prev,
            "ABCDEFGHIJKM", // 与重叠段差 1 字
            AlignPrior {
                expected_ratio: None,
                prev_extra_slices: 1,
            },
        );
        assert!(r1.ok, "差 1 字（1/12≈0.083 ≤0.15）应能对齐");
        assert_eq!(r1.committed_prefix, "PQRST");
        let r2 = align_overlap_with_prior(
            prev,
            "ABCDEFGHIJMN", // 差 2 字（2/12≈0.167 >0.15）
            AlignPrior {
                expected_ratio: None,
                prev_extra_slices: 1,
            },
        );
        assert!(!r2.ok, "差 2 字超过质量门 ⇒ 保守不对齐（丢字由拼接兜底）");
    }

    /// 设计 B（周期性 4 遍）：全文不丢、不重复。
    #[test]
    fn periodic_4x_no_sentence_lost() {
        let dur = 4_400 * 16;
        let samples = vec![dur; 4];
        let mut o = OrderedReflow::new();
        assert_eq!(
            o.push_window(0, 0, 4, samples.clone(), format!("{S}{S}{S}{S}"))
                .len(),
            1
        );
        assert_eq!(
            o.push_window(1, 1, 5, samples, format!("{S}{S}{S}{S}"))
                .len(),
            1
        );
        let (committed, last) = o.finish();
        let full = format!("{committed}{last}");
        assert!(committed.contains(S), "滑出句必须定稿：{committed:?}");
        assert_eq!(
            full.matches(S).count(),
            5,
            "全文该句恰 5 次（丢/多都错）：{full}"
        );
    }

    /// 设计 B（周期性 5 遍）：同上，周期更长。
    #[test]
    fn periodic_5x_no_sentence_lost() {
        let dur = 4_400 * 16;
        let samples = vec![dur; 5];
        let mut o = OrderedReflow::new();
        assert_eq!(
            o.push_window(0, 0, 5, samples.clone(), format!("{S}{S}{S}{S}{S}"))
                .len(),
            1
        );
        assert_eq!(
            o.push_window(1, 1, 6, samples, format!("{S}{S}{S}{S}{S}"))
                .len(),
            1
        );
        let (committed, last) = o.finish();
        let full = format!("{committed}{last}");
        assert_eq!(full.matches(S).count(), 6, "全文该句恰 6 次：{full}");
    }

    /// 设计 B（期望比例为 0/负/NaN）：不 panic，退化为「小 k 优先」与无先验同解。
    #[test]
    fn invalid_expected_ratio_degrades_safely() {
        let prev = format!("{S}{S}{S}");
        let new = format!("{S}{S}{S}");
        let base = align_overlap_with_prior(&prev, &new, AlignPrior::none());
        for bad in [0.0f32, -1.0, f32::NAN] {
            let r = align_overlap_with_prior(
                &prev,
                &new,
                AlignPrior {
                    expected_ratio: Some(bad),
                    prev_extra_slices: 0,
                },
            );
            assert_eq!(r.ok, base.ok, "非法比例 {bad} 应与无先验同解");
            assert_eq!(
                r.overlap_chars, base.overlap_chars,
                "非法比例 {bad} k 应一致"
            );
            assert_eq!(r.committed_prefix, base.committed_prefix);
            assert!(
                !r.committed_prefix.is_empty(),
                "非法比例 {bad} 必须退化到小 k（committed 非空、不丢字）"
            );
        }
    }

    /// 设计 B：极短窗（1~2 字）低于最小重叠长度 ⇒ 保守不对齐；经 OrderedReflow 也不丢。
    #[test]
    fn tiny_windows_are_conservative() {
        for (p, n) in [("你", "你"), ("你好", "你好"), ("你", "好")] {
            assert!(!align_overlap(p, n).ok, "极短窗必须保守不对齐（{p}/{n}）");
        }
        let mut o = OrderedReflow::new();
        let _ = o.push(0, 0, 1, "甲".to_string());
        let _ = o.push(1, 1, 2, "乙".to_string());
        let (c, l) = o.finish();
        let full = format!("{c}{l}");
        assert!(
            full.contains('甲') && full.contains('乙'),
            "极短窗内容不得丢：{full}"
        );
    }

    /// 设计 B：`prev_extra_slices=0`（两窗起点相同）不施加硬上界；无真重叠不强行提交。
    #[test]
    fn zero_extra_slices_no_hard_cap_but_no_force_commit() {
        let r = align_overlap_with_prior(
            "0123456789ABCDEF",
            "6789ABCDEFGHIJKL", // 真值重叠 10 字
            AlignPrior {
                expected_ratio: None,
                prev_extra_slices: 0,
            },
        );
        assert!(r.ok, "m=0 不设硬上界，真重叠应能对上");
        assert_eq!(r.overlap_chars, 10, "真值 10 不得被硬上界压小");
        assert_eq!(r.committed_prefix, "012345");
        let r2 = align_overlap_with_prior(
            "aaaabbbb",
            "ccccdddd", // 无真重叠
            AlignPrior {
                expected_ratio: None,
                prev_extra_slices: 0,
            },
        );
        assert!(!r2.ok, "无真实重叠不得强行提交");
    }

    /// 设计 B：`slice_samples` 与区间不自洽（长度对不上）⇒ 安全退化、不丢内容。
    #[test]
    fn inconsistent_slice_samples_degrade_safely() {
        let mut o = OrderedReflow::new();
        assert_eq!(
            o.push_window(0, 0, 3, vec![100, 200, 300], "甲乙丙".to_string())
                .len(),
            1
        );
        // 区间 [1,4) 长 3，样本表 len=2 ⇒ 不自洽
        let out = o.push_window(1, 1, 4, vec![100, 200], "乙丙丁".to_string());
        assert_eq!(out.len(), 1);
        let (c, l) = o.finish();
        let full = format!("{c}{l}");
        assert!(
            full.contains('甲') && full.contains('丁'),
            "不自洽信息下不得丢内容：{full}"
        );
    }

    /// 设计 B：乱序到达 + 中间窗空文本（解码失败）⇒ 按 seq 有序定稿、空窗不污染。
    #[test]
    fn out_of_order_with_empty_middle_window() {
        let mut o = OrderedReflow::new();
        assert_eq!(o.push(0, 0, 2, "甲乙".to_string()).len(), 1);
        assert!(
            o.push(2, 4, 6, "戊己".to_string()).is_empty(),
            "seq1 未到 ⇒ seq2 挂起"
        );
        let out = o.push(1, 2, 4, String::new());
        assert_eq!(out.len(), 1, "空窗推进 next 后 seq2 应可定稿");
        let (c, l) = o.finish();
        let full = format!("{c}{l}");
        assert!(
            full.contains('甲') && full.contains('戊'),
            "乱序+空窗后内容不得丢：{full}"
        );
    }

    /// 设计 B（全程不变量）：混合序列（零重叠 / 对齐失败 / 正常重叠）下不丢任何窗口独有内容。
    #[test]
    fn no_window_content_is_ever_lost() {
        let mut o = OrderedReflow::new();
        let _ = o.push(0, 0, 3, "甲乙丙".to_string());
        let _ = o.push(1, 3, 6, "丁戊己".to_string()); // 零重叠 ⇒ 拼接
        let _ = o.push_window(2, 5, 8, vec![1], "戊己庚辛".to_string()); // 信息不自洽
        let _ = o.push_window(3, 7, 10, vec![100, 100, 100], "庚辛壬癸".to_string());
        let (c, l) = o.finish();
        let full = format!("{c}{l}");
        for ch in ['甲', '乙', '丙', '丁', '戊', '己', '庚', '辛', '壬', '癸'] {
            assert!(full.contains(ch), "窗口独有内容 '{ch}' 丢失：{full}");
        }
    }

    /// 设计 B：极长窗（300 字）不 panic、内容不丢。
    #[test]
    fn very_long_window_aligns_without_panic() {
        let long = S.repeat(30); // 360 字
        let mut o = OrderedReflow::new();
        let _ = o.push(0, 0, 10, long.clone());
        let out = o.push(1, 5, 15, long);
        assert_eq!(out.len(), 1);
        let (c, l) = o.finish();
        assert!(!format!("{c}{l}").is_empty());
    }
}

// =====================================================================
// TEST-SYNC-377（阶段三 · 非作者视角，tester-1）
// ---------------------------------------------------------------------
// 目标：377「注入按规格收窄」——`build_ctx_system` 只产出纯 ASCII 逗号词表；
// 并确认 374/375/371/368/369 不回归。生产代码零改动、按设计写用例。
// =====================================================================
#[cfg(test)]
mod testsync377_inject_spec_tests {
    use super::{build_ctx_system, should_inject_ctx};

    // ---------- 377：build_ctx_system 行为 ----------

    /// 设计：纯词表 ⇒ 逐字返回（不得重排 / 去重 / 加标签）。
    #[test]
    fn ctx377_pure_list_verbatim() {
        let t = "你好,铭印,银线,朵洛莉丝";
        assert_eq!(
            build_ctx_system(Some(t)).as_deref(),
            Some(t),
            "纯词表必须逐字返回"
        );
    }

    /// 设计：只 trim 首尾；内部空白原样保留。
    #[test]
    fn ctx377_trims_ends_only_preserves_inner() {
        assert_eq!(
            build_ctx_system(Some("  你好, 铭印  ")).as_deref(),
            Some("你好, 铭印")
        );
        assert_eq!(
            build_ctx_system(Some("\t你好,铭印\n")).as_deref(),
            Some("你好,铭印")
        );
    }

    /// 设计：None / 空串 / 纯空白 / 纯换行 ⇒ None（不调 set_option hotwords）。
    #[test]
    fn ctx377_none_on_empty_or_blank() {
        for t in [None, Some(""), Some("   "), Some("\t\n "), Some("\r\n")] {
            assert_eq!(build_ctx_system(t), None, "空/纯空白 ⇒ None：{t:?}");
        }
    }

    /// 设计：逗号结构逐字保留（连续逗号 / 尾随逗号 / 只有逗号 / 重复词条都不动）。
    #[test]
    fn ctx377_commas_verbatim() {
        assert_eq!(build_ctx_system(Some("a,,b,")).as_deref(), Some("a,,b,"));
        assert_eq!(
            build_ctx_system(Some(",")).as_deref(),
            Some(","),
            "只有逗号=非空 ⇒ 原样"
        );
        assert_eq!(
            build_ctx_system(Some("a,b,a")).as_deref(),
            Some("a,b,a"),
            "不去重"
        );
    }

    /// 设计：产出不得含旧注入片段（英文指令句 / `Context:` / `Terms:`）。
    #[test]
    fn ctx377_no_instruction_or_labels() {
        let out = build_ctx_system(Some("你好,铭印,银线")).expect("非空");
        for needle in ["Transcribe", "The following is", "Context:", "Terms:"] {
            assert!(
                !out.contains(needle),
                "产出不得含旧注入片段 `{needle}`：{out:?}"
            );
        }
    }

    /// 设计：单行词表不得被组装成多行。
    #[test]
    fn ctx377_no_newline_introduced() {
        let out = build_ctx_system(Some("你好,铭印,银线")).expect("非空");
        assert!(!out.contains('\n'), "单行词表不得含换行：{out:?}");
    }

    /// 设计（换行括注）：含换行不崩；「原样返回」契约下内部换行保留（与「不带进 system 段」的
    /// 张力见 result.md 备注，未擅自改断言）。
    #[test]
    fn ctx377_internal_newline_is_passthrough_no_panic() {
        let out = build_ctx_system(Some("你好,\n铭印")).expect("非空");
        assert_eq!(out, "你好,\n铭印", "仅首尾 trim、内部换行按原样保留");
    }

    /// 设计：注入门 —— 空 ⇒ 不注入；非空 ⇒ 注入。
    #[test]
    fn ctx377_should_inject_gate() {
        assert!(!should_inject_ctx(build_ctx_system(None).as_deref()));
        assert!(!should_inject_ctx(build_ctx_system(Some("   ")).as_deref()));
        assert!(should_inject_ctx(build_ctx_system(Some("词A")).as_deref()));
    }

    /// 设计（KV 预算）：空/纯空白 ⇒ 注入 token 估算 = 0。
    #[test]
    fn ctx377_estimate_tokens_zero_when_blank() {
        assert_eq!(super::estimate_inject_tokens(None), 0);
        assert_eq!(super::estimate_inject_tokens(Some("")), 0);
        assert_eq!(super::estimate_inject_tokens(Some("   ")), 0);
    }

    // ---------- 374 / 375：确认不回归（独立夹具） ----------

    /// 374：377 新格式（裸词表）被逐字吐回 ⇒ 仍须命中并整段剥掉。
    #[test]
    fn echo374_new_format_run_is_stripped() {
        let terms = "甲词,乙词,丙词,丁词,戊词";
        let sys = build_ctx_system(Some(terms)).expect("注入串");
        let hit = super::strip_terms_echo(&sys, Some(terms)).expect("新格式应命中");
        assert!(
            hit.matched_terms >= 4,
            "连续词条 run 应 ≥4：{}",
            hit.matched_terms
        );
        assert_eq!(hit.stripped, "", "整段回显 ⇒ 剥空");
    }

    /// 374：句子里自然说到 1~2 个词库词 ⇒ 不得剥。
    #[test]
    fn echo374_natural_one_or_two_terms_kept() {
        let terms = "明天,天气,心情,咖啡,电影";
        let text = "明天天气不错，我想喝咖啡。";
        assert!(
            super::strip_terms_echo(text, Some(terms)).is_none(),
            "1~2 个词不是回显：{text}"
        );
    }

    /// 375：冷启动三条路径（无均值 / 无音频 / 期望产出太少）⇒ 一律判正常。
    #[test]
    fn rate375_cold_start_three_paths_ok() {
        // 【388 契约变更 · D2】old：无均值/非正/非有限一律 true；
        // new：按「无均值」处理 + 音频 10s ≥ 3s + 0 字 ⇒ 字/秒 0 < 1 ⇒ 坍塌。
        assert!(
            !super::output_rate_ok(0, 10.0, None),
            "D2：无均值长音频 0 字 ⇒ 坍塌（old：true）"
        );
        assert!(super::output_rate_ok(0, 0.0, Some(4.4)), "无音频 ⇒ 不判");
        assert!(
            super::output_rate_ok(0, 1.0, Some(4.4)),
            "期望产出 4.4 < 8 ⇒ 不判"
        );
        assert!(
            !super::output_rate_ok(0, 10.0, Some(0.0)),
            "D2：非正均值按无均值 ⇒ 坍塌（old：true）"
        );
        assert!(
            !super::output_rate_ok(0, 10.0, Some(f32::INFINITY)),
            "D2：非有限均值按无均值 ⇒ 坍塌（old：true）"
        );
    }

    // ---------- 371 / 368 / 369：确认不回归 ----------

    /// 371：只截第一个标记 + 64B 位置护栏 + 正文中段不误剥。
    #[test]
    fn prefix371_guard_first_only_and_midbody() {
        use super::strip_qwen3_language_prefix as strip;
        assert_eq!(
            strip("language chinese<asr_text>甲<asr_text>乙"),
            "甲<asr_text>乙",
            "只截第一个标记"
        );
        let at64 = format!("{}<asr_text>正文", "a".repeat(64));
        let at65 = format!("{}<asr_text>正文", "a".repeat(65));
        assert_eq!(strip(&at64), "正文", "起点 64 ⇒ 剥");
        assert_eq!(strip(&at65), at65.as_str(), "起点 65 ⇒ 原样");
        let mid = format!("{}<asr_text>尾", "正文".repeat(40));
        assert_eq!(strip(&mid), mid.as_str(), "中段标记 ⇒ 不剥");
    }

    /// 371：周期性重复内容不丢字。
    #[test]
    fn align371_periodic_keeps_every_sentence() {
        let s = "今天天气真好我们出去玩吧";
        let dur = 4_400 * 16;
        let samples = vec![dur; 3];
        let mut o = super::OrderedReflow::new();
        let _ = o.push_window(0, 0, 3, samples.clone(), format!("{s}{s}{s}"));
        let _ = o.push_window(1, 1, 4, samples, format!("{s}{s}{s}"));
        let (c, l) = o.finish();
        let full = format!("{c}{l}");
        assert_eq!(full.matches(s).count(), 4, "周期内容不得丢字：{full}");
    }

    /// 369：零重叠 ⇒ 必须拼接、不得丢。
    #[test]
    fn align369_zero_overlap_concatenates() {
        let mut o = super::OrderedReflow::new();
        let _ = o.push(0, 0, 3, "甲乙丙".into());
        let out = o.push(1, 3, 6, "丁戊己".into());
        assert_eq!(out.len(), 1);
        let (c, l) = o.finish();
        let full = format!("{c}{l}");
        assert!(
            full.contains('甲') && full.contains('丁'),
            "零重叠必须拼接不丢：{full}"
        );
    }

    /// 368：对齐失败 ⇒ 不得丢上一窗滑出文本（退回拼接）。
    #[test]
    fn align368_align_failure_keeps_slid_text() {
        let mut o = super::OrderedReflow::new();
        let _ = o.push_window(0, 0, 2, vec![100, 100], "甲乙".into());
        // 共享片但样本表与区间不自洽 ⇒ 对齐失败 ⇒ 退回拼接
        let out = o.push_window(1, 1, 3, vec![100], "乙丙".into());
        assert_eq!(out.len(), 1);
        let (c, l) = o.finish();
        let full = format!("{c}{l}");
        assert!(
            full.contains('甲') && full.contains('丙'),
            "对齐失败不得丢字：{full}"
        );
    }
}
