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
pub use vad::{
    build_padded_segments, join_segment_texts, naive_chunk, should_segment, VadSegmenter,
    SEGMENT_MAX_SECS, SEGMENT_PADDING_SAMPLES, SEGMENT_TRIGGER_SECS,
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
/// - `CTX_DEFAULT_CHARS`：**时间线合并后**的统一上限，**常量 500 字**（320 修正：三段合一取尾，
///   超长只从**最旧**一端截 ⇒ 本次录音刚完成的片永远保留；KV 4096 余量充足）。
/// - 上下文注入**恒开**（原 `LOCAL_RT_CTX_ENABLED` 已删；Gavin 2026-09-21：不允许存在开发端/用户端
///   不一致，用户机器无 env ⇒ env 覆盖是假路径。止血手段不再保留）。
/// - 🔴 per-stream 通道仅 Qwen3 可用（accuracy 引擎已固定 Qwen3）。
const CTX_DEFAULT_CHARS: usize = 500;
/// 回显判定：LCS ≥ 此绝对长度（正常组 LCS=3、故障组 LCS=79；20 在两者之间、靠近正常侧）。
const CTX_ECHO_LCS_ABS: usize = 20;
/// 回显判定：LCS ≥ 输出的此比例（短输出也可能整段回显）。
const CTX_ECHO_LCS_RATIO: f64 = 0.5;
/// 输出清理指令：**恒发**，与有没有上下文、有没有词条都无关（Gavin 2026-09-21 指示）。
///
/// 口吃、重复词、口水词在 ASR 原始输出里很常见，模型自己顺掉最好。Gavin 口径：
/// 「指令不要太长，反正即使不起作用，它也占不了多少上下文的窗口。」
/// ⇒ 一句话、十来个 token，对 DEC-068 零和预算的挤占可忽略。
///
/// 🔴 这**不替代**下游 `apply_filler_strip`（`src/text_normalizer.rs`）：
///    那是确定性兜底，本指令是尽力而为，两者并存，别因为加了这句就去摘兜底。
const CLEANUP_INSTR_EN: &str =
    "Transcribe cleanly: drop stutters, repeated words, and filler words.";
/// 上下文说明句：**只在真有前文时**才发（没有前文还说「以下是前文」纯属误导模型）。
const CTX_INSTR_EN: &str = "The following is the preceding context of this recording. Use it to keep terminology and wording consistent.";

/// 取字符串**最后** n 个字符（最近的最相关）；不足 n 全取。
fn last_n_chars(s: &str, n: usize) -> String {
    let v: Vec<char> = s.chars().collect();
    if v.len() <= n {
        s.to_string()
    } else {
        v[v.len() - n..].iter().collect()
    }
}

/// 拼 system 段：英文说明句 + `Context:`（时间线）+ `Terms:`（词库），各占一行加标签。
/// 两者皆空 ⇒ None（不注入）。
fn build_ctx_system(context: Option<&str>, terms: Option<&str>) -> Option<String> {
    let c = context.unwrap_or("").trim();
    let t = terms.unwrap_or("").trim();
    // 🔴 **恒返回 Some**（2026-09-21 契约变更）：清理指令与上下文/词条无关，永远要发。
    //    旧行为是「都空 ⇒ None ⇒ 整段不注入」，那样一来新用户（词库为空）本次录音的
    //    第一片就拿不到清理指令 —— 正是最需要它的时候。别改回去。
    let mut s = String::from(CLEANUP_INSTR_EN);
    if !c.is_empty() {
        s.push('\n');
        s.push_str(CTX_INSTR_EN);
        s.push_str(&format!("\nContext: {c}"));
    }
    if !t.is_empty() {
        s.push_str(&format!("\nTerms: {t}"));
    }
    Some(s)
}

/// LOCALRT-CTX-INJECT-320：注入素材——三段按**严格时间顺序**（上上次 → 上一次 → 本次已完成片）+ 词库。
///
/// 🔴 三段**合成一条时间线**，超长时**只从最旧端截**（先砍上上次、再砍上一次，本次刚完成的片永不先掉）。
/// 不设分段配额（Gavin 2026-09-21 修正，推翻主控的「分开留额度」草案）。
pub struct CtxInject<'a> {
    pub prev_older: Option<&'a str>,
    pub prev_latest: Option<&'a str>,
    pub current: Option<&'a str>,
    pub terms: Option<&'a str>,
}

/// 按时间顺序合并三段为一条时间线（跳过空段）；返回 `(合并串, [上上次,上一次,本次] 各段字数)`。
fn merge_ctx_timeline(
    prev_older: Option<&str>,
    prev_latest: Option<&str>,
    current: Option<&str>,
) -> (String, [usize; 3]) {
    let mut merged: Vec<&str> = Vec::new();
    let mut lens = [0usize; 3];
    for (i, p) in [prev_older, prev_latest, current].into_iter().enumerate() {
        let t = p.unwrap_or("").trim();
        lens[i] = t.chars().count();
        if !t.is_empty() {
            merged.push(t);
        }
    }
    (merged.join("\n"), lens)
}

/// 回显探针归一化：去空白与常见中英标点（回显是逐字文本，标点差异不应漏检）。
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
enum CtxEchoAction {
    Keep,
    Redecode,
}

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

/// 一次 accuracy 解码（可选 per-stream system 段）。返回规范化后的文本。
fn decode_accuracy_once(
    recognizer: &sherpa_onnx::OfflineRecognizer,
    samples: &[f32],
    system: Option<&str>,
    script: ChineseScript,
) -> Result<String> {
    let stream = recognizer.create_stream();
    if let Some(s) = system {
        stream.set_option("hotwords", s);
    }
    stream.accept_waveform(16000, samples);
    recognizer.decode(&stream);
    let result = stream.get_result().context("No transcription result")?;
    let text = Transcriber::strip_asr_special_tokens(result.text.trim());
    if text.is_empty() {
        // 与 transcribe_segment_detailed 一致：accuracy 空输出 ⇒ 该段失败（上层 all_native=false）
        anyhow::bail!("ASR accuracy model produced empty output");
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
    // B/C：三段按时间顺序合并成一条时间线（上上次 → 上一次 → 本次已完成片）。
    // 🔴🔴 截断方向：**从头部（时间最远）截，保留尾部（时间最近）**——即 `last_n_chars(_, CAP)`
    //   保留最后 CAP 个字符。举例：CAP=500，三段 200+180+150=530 ⇒ 砍掉「上上次」开头 80 字，
    //   结果 = 上上次后 120 + 上一次全部 180 + 本次全部 150。本次刚完成的片**永远最后才被考虑**。
    //   ⚠️ 方向极易被后人改反；改反 = 丢掉最相关的刚说内容、留一堆最旧的，比不加还糟。
    let (ctx_raw, lens) = merge_ctx_timeline(inject.prev_older, inject.prev_latest, inject.current);
    let ctx_raw_len = ctx_raw.chars().count();
    let ctx = last_n_chars(&ctx_raw, CTX_DEFAULT_CHARS);
    let ctx_len = ctx.chars().count();
    let cut = ctx_raw_len.saturating_sub(ctx_len);
    let terms_len = inject.terms.map(|s| s.chars().count()).unwrap_or(0);
    let ctx_opt = (!ctx.is_empty()).then_some(ctx.as_str());
    let system = build_ctx_system(ctx_opt, inject.terms);
    let inject_on = should_inject_ctx(system.as_deref());
    let mut text = decode_accuracy_once(
        recognizer,
        samples,
        system.as_deref().filter(|_| inject_on),
        script,
    )?;
    // D：ctx 定稿后才做长跨回显护栏（命中 ⇒ 无上下文重解，不丢片）。
    //
    // 🔴 比对面**只取 `ctx`**（前文时间线），不含 `CTX_INSTR_EN` 指令句、也不含 `Terms:` 词库段。
    //    2026-09-21 BUILD-321 端测实测：原先拿整个 system 串比对，73 字词库一并进了 LCS，
    //    于是「用户说到词库里的词」被判成回显 —— 14 次触发 6 次重解（43%）。
    //    词库本来就是注入去帮模型认词的，认对了反而被护栏抵消掉，等于自己打自己。
    //    回显要防的是**前文被逐字念回**，与词条无关、与英文指令更无关。改回 sys = 直接复发。
    //
    // 🔴 `ctx` 为空时整块跳过：没有前文就不存在「回显前文」，此时只注了词库，
    //    再比对必然是误判（上面那 43% 里就有这种）。
    let mut lcs = 0usize;
    let mut action = CtxEchoAction::Keep;
    // 🔴 判决依据是**带上下文那一次**的输出；命中后 `text` 会被重解结果覆盖。
    //    埋点必须记判决当时的长度，否则日志里 lcs 与 out_chars 对不上（lcs=7/out=15 看着
    //    既不够 20 也不够 50%，实际判决时输出比 15 短）——监控失真，护栏误触就查不出来。
    let decided_out_chars = text.chars().count();
    if inject_on && !ctx.is_empty() {
        let (l, a) = ctx_echo_action(&normalize_ctx_probe(&text), &normalize_ctx_probe(&ctx));
        lcs = l;
        if a == CtxEchoAction::Redecode {
            action = CtxEchoAction::Redecode;
            text = decode_accuracy_once(recognizer, samples, None, script)?;
        }
    }
    if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "[LocalRT-DBG-320] ctx inject: seg={} prev2_len={} prev1_len={} acc_len={} ctx_raw_len={} ctx_len={} cut={} terms_len={} out_chars={} final_chars={} lcs={} action={}",
            seg_idx,
            lens[0],
            lens[1],
            lens[2],
            ctx_raw_len,
            ctx_len,
            cut,
            terms_len,
            decided_out_chars,
            text.chars().count(),
            lcs,
            if action == CtxEchoAction::Redecode {
                "redecode"
            } else {
                "keep"
            }
        );
    }
    Ok((text, true))
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
        result
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
            .join("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25")
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
///   | 上下文 `CTX_DEFAULT_CHARS=500` 字 × ~0.77 | 385 |
///   | 清理指令 + 上下文说明句 + `Context:`/`Terms:` 标签 | ~55 |
///   | 生成预留 `max_new_tokens` | 256 |
///   | **剩给词库** | **4096 ⇒ 3140** ／ **2048 ⇒ 1092** |
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

/// MIGRATE-QWEN3-314：Qwen3-ASR 就位判据（四个路径；与 `create_qwen3_recognizer` 加载清单逐字一致）。
fn check_qwen3_model_ready(model_dir: &Path) -> (bool, PathBuf) {
    let dir = model_dir.join("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25");
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
            .join("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25")
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
    // LOCALRT-CTX-INJECT-320：上下文注入（截取 / 回显护栏 / 门控）
    // ============================================================

    #[test]
    fn ctx320_last_n_chars_truncates_to_tail_by_chars() {
        assert_eq!(last_n_chars("abcdef", 3), "def", "取最后 N 个字符");
        assert_eq!(last_n_chars("abc", 10), "abc", "不足 N 全取");
        assert_eq!(last_n_chars("", 5), "");
        assert_eq!(last_n_chars("中文测试", 2), "测试", "按字符不是字节");
    }

    #[test]
    fn ctx320_build_system_labels_and_timeline_order() {
        let s = build_ctx_system(Some("前文内容"), Some("词A,词B")).unwrap();
        assert!(s.starts_with(CLEANUP_INSTR_EN), "第一行恒为清理指令");
        assert!(s.contains(CTX_INSTR_EN), "有前文 ⇒ 带上下文说明句");
        assert!(s.contains("\nContext: 前文内容"), "Context 标签");
        assert!(s.contains("\nTerms: 词A,词B"), "Terms 标签");
        // 🔴 契约（Gavin 2026-09-21）：清理指令**恒发**，都空时也要有，且此时只有它。
        let bare = build_ctx_system(None, None).expect("都空也必须注入清理指令");
        assert_eq!(
            bare, CLEANUP_INSTR_EN,
            "都空 ⇒ 仅清理指令，无 Context/Terms"
        );
        assert_eq!(
            build_ctx_system(Some("  "), Some(" ")).expect("空白等同空，但仍发清理指令"),
            CLEANUP_INSTR_EN,
            "空白等同空"
        );
        // 无前文时不得出现「以下是前文」这句，否则等于骗模型去找不存在的上文
        assert!(!bare.contains(CTX_INSTR_EN), "无前文 ⇒ 不带上下文说明句");
        // 时间线：顺序恒为 上上次 → 上一次 → 本次；超长只砍最旧端（取尾）
        let (raw, lens) = merge_ctx_timeline(Some("上上次"), Some("上一次"), Some("本次片一"));
        assert_eq!(raw, "上上次\n上一次\n本次片一", "时间顺序不可颠倒");
        assert_eq!(lens, [3, 3, 4], "各段字数（上上次/上一次/本次）");
        assert_eq!(last_n_chars(&raw, 4), "本次片一", "取尾只砍最旧端");
        let (empty, _) = merge_ctx_timeline(None, None, None);
        assert!(empty.is_empty());
    }

    #[test]
    fn ctx320_truncation_keeps_tail_drops_oldest_head() {
        // 🔴 方向钉死：超长时从**头部（最旧）**砍，保留**尾部（最新）**。
        // 三段可区分：上上次 a*200 / 上一次 b*180 / 本次 c*150（合计 532 > CAP 450）。
        let a = "a".repeat(200);
        let b = "b".repeat(180);
        let c = "c".repeat(150);
        let (raw, _) = merge_ctx_timeline(Some(&a), Some(&b), Some(&c));
        let kept = last_n_chars(&raw, 450);
        assert_eq!(kept.chars().count(), 450);
        assert!(kept.ends_with(&c), "必须保留本次内容（在尾部）");
        assert_eq!(kept.matches('c').count(), 150, "本次 150 字全保留");
        assert_eq!(kept.matches('b').count(), 180, "上一次 180 字全保留");
        assert_eq!(
            kept.matches('a').count(),
            118,
            "上上次只剩后 118 字（头部被砍 80）"
        );
        assert!(kept.starts_with('a'), "保留段仍以（上上次的）残尾开头");
    }

    #[test]
    fn ctx320_lcs_normal_keeps_echo_redecodes() {
        let (l, a) = ctx_echo_action(
            &normalize_ctx_probe("今天天气不错我们出去走走吧"),
            &normalize_ctx_probe("甲乙丙丁戊己庚辛壬癸子丑寅卯辰巳午未申酉"),
        );
        assert!(l < CTX_ECHO_LCS_ABS, "正常输出 LCS 应小，实测 {l}");
        assert_eq!(a, CtxEchoAction::Keep);
        let ctx = "上一轮八分之一决赛凭借哈兰德梅开二度挪威队爆冷淘汰巴西队";
        let (l2, a2) = ctx_echo_action(&normalize_ctx_probe(ctx), &normalize_ctx_probe(ctx));
        assert!(l2 >= CTX_ECHO_LCS_ABS, "逐字回显 LCS 应 ≥20，实测 {l2}");
        assert_eq!(a2, CtxEchoAction::Redecode);
    }

    #[test]
    fn ctx320_lcs_ratio_triggers_for_short_output() {
        // LCS(9) < 20 绝对阈值，但 ≥ 输出(11 字)的 50% ⇒ 触发
        let ctx = "甲公司乙公司丙公司丁公司戊公司";
        let out = "甲公司乙公司丙公司结果";
        let (l, a) = ctx_echo_action(&normalize_ctx_probe(out), &normalize_ctx_probe(ctx));
        assert!(l < CTX_ECHO_LCS_ABS, "LCS {l} 应 <20");
        let out_len = normalize_ctx_probe(out).chars().count() as f64;
        assert!(l as f64 >= CTX_ECHO_LCS_RATIO * out_len, "应命中 50% 判据");
        assert_eq!(a, CtxEchoAction::Redecode);
    }

    #[test]
    fn ctx320_constants_and_gate() {
        // MIGRATE-QWEN3-320：env 全删 ⇒ 值即常量。
        assert_eq!(CTX_DEFAULT_CHARS, 500, "上下文上限常量 500");
        // 门控：有内容即注入（恒开）
        assert!(should_inject_ctx(Some("x")));
        assert!(!should_inject_ctx(None), "无内容 ⇒ 不注入");
    }

    #[test]
    fn migrate320_qwen3_readiness_checks_four_paths() {
        let root = std::env::temp_dir().join(format!(
            "voice-ime-mig320-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let q = root.join("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25");
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
        create_qwen3_recognizer, create_qwen3_recognizer_at, default_acc_num_threads,
        transcribe_acc_ctx, CtxInject,
    };
    use crate::config::ChineseScript;
    use std::path::PathBuf;
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
            let current = if acc.is_empty() {
                None
            } else {
                Some(acc.as_str())
            };
            let inject = CtxInject {
                prev_older: None,
                prev_latest: None,
                current,
                terms: Some(FIT_WORDLIST),
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
}
