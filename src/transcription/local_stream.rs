//! LOCAL-RT-ENGINE-239-A / DEC-067：本地真流式 ASR（streaming paraformer trilingual）。
//!
//! 与 [`crate::transcription::qwen_inference::transcribe_streaming_realtime`] **平行**的本地实现：
//! 不走 WebSocket，直接把实时音频 chunk 喂给常驻的 [`sherpa_onnx::OnlineRecognizer`]。
//!
//! 设计要点（对齐主控方案）：
//! - 复用 [`qwen_inference::StreamingAsrState`] 做「确认句 + 当前句」状态机，
//!   把 sherpa 的 `is_endpoint()` 映射为 `sentence_end = true`，**不另写状态机**。
//! - 回调签名与 qwen 路径同构：`on_result(&display_text, &display_words)`。
//! - 每次 `get_result()` 文本变化才回调（与 qwen 路径「空包不转发」同向），避免无谓刷新。
//! - 返回 `(final_text, pcm)`：`pcm` 为本次全部音频（16kHz f32），供 2pass 离线纠错复用。
//! - `cancel_signal` 置位即提前收尾（`Relaxed` 读，与 qwen 路径同款）。
//!
//! 端点语义：paraformer streaming 在一次 endpoint 后需 `reset()` 才能开始下一句；
//! reset 后 `get_result()` 从空串重新累积，与 `StreamingAsrState` 的句切换天然对齐。

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sherpa_onnx::{OnlineParaformerModelConfig, OnlineRecognizer, OnlineRecognizerConfig};

use super::qwen_inference::{StreamingAsrState, WordTiming};
use crate::punctuation::PunctuationEngine;

/// 本地流式 recognizer 采样率（streaming paraformer trilingual 固定 16kHz 单声道）。
const SAMPLE_RATE: i32 = 16000;

/// LOCAL-RT-ENGINE-239-B：把 `&OnlineRecognizer` 送进 ASR 工作线程的 Send 包装。
///
/// SAFETY：与 `Transcriber` 的 `unsafe impl Send` 同源论证——sherpa-onnx C++ OnlineRecognizer
/// 的 create/decode/destroy 可在不同线程调用，只需保证**同一实例不被并发访问**。
/// 本包装仅在 `std::thread::scope` 内把引用 move 给 ASR 线程；该线程运行期间，持有者
/// （worker 线程）阻塞在 `record_streaming`，不触碰 recognizer；ASR 线程 join 之后才继续
/// 使用（run_pipeline_core 走 offline_recognizer，对象不同且串行）。故无并发访问。
pub struct SendOnlineRecognizerRef<'a>(pub &'a OnlineRecognizer);
unsafe impl<'a> Send for SendOnlineRecognizerRef<'a> {}

impl<'a> SendOnlineRecognizerRef<'a> {
    /// 取出内部引用。**按值消费包装**，使跨线程闭包必须捕获整个 Send 包装
    /// （Rust 2021 disjoint capture 下若直接取 `.0` 字段会退化为捕获裸引用，
    /// 绕过 `unsafe impl Send`，故以方法消费强制整体捕获）。
    pub fn into_inner(self) -> &'a OnlineRecognizer {
        self.0
    }
}

/// 流式解码线程数：**按运行机器的 CPU 核数取，不写死**（Gavin 2026-09-21 定）。
///
/// 写死具体数字的问题：8 在 4 核机器上就是超订两倍；不同用户机器核数不同，
/// 开发机上的最优值搬到用户机器上可能恰是最差值。
///
/// 口径与 accuracy 侧 `default_acc_num_threads()` **完全一致**：
/// `available_parallelism().min(8)`，取不到时回落 4。
/// 🔴 上限 8 的依据：271 本机实测 accuracy 线程曲线，长音频 0/1/2/4/**8**/12/**16** 线程 =
/// 30.9/31.1/20.9/22.4/**17.0**/19.8/**34.2** 秒 —— 8 最优，**16 比 8 慢一倍**、甚至慢于单线程。
/// 故机器核再多也不超过 8。
///
/// 沿革：POC-LOCAL-STREAM-235 定 4（独占运行前提）→ TUNE-STREAM-317 加 env →
/// Gavin 提到 8 → 本次改为按核数动态取，并删除 env（开发端与用户端行为必须一致）。
fn local_stream_num_threads() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(8) as i32)
        .unwrap_or(4)
}

// 🔴 TUNE-STREAM-317 / TEST-SWEEP-319：`blank_penalty` 对本模型**完全无效**，已撤。
// 实测（tester-1，证据 collab/evidence/20260921-sweep319-blankpenalty/）：
// 0 / -0.5 / -1.0 / -1.5 / +0.5 五档输出**逐字节相同**，追加极端值 ±100 仍逐字节相同；
// 且已用临时 println 自证 env 确实被应用 ⇒ 不是没传进去，是流式 paraformer 路径不消费该字段。
// 属 [PROVIDER-SILENT-FALLBACK-001] 同族：参数收下、静默不生效。
// **不要再加回来**；要改流式的吐字倾向，得换别的机制。

/// LOCAL-RT-ENGINE-239-A（DEC-067）：端点检测参数。
///
/// `rule1=2.4` / `rule3=20.0` 取 sherpa 默认；🔴 `rule2=2.0` 对齐 ASR-SEG-229
/// 定的在线档 `asr_online_max_sentence_silence=2000ms` —— 官方默认 1.2 太激进，
/// 会重现「说话稍一停顿就被判句尾、输出切碎」的端测缺陷，**勿改回 1.2**。
const LOCAL_STREAM_RULE1_MIN_TRAILING_SILENCE: f32 = 2.4;
const LOCAL_STREAM_RULE2_MIN_TRAILING_SILENCE: f32 = 2.0;
const LOCAL_STREAM_RULE3_MIN_UTTERANCE_LENGTH: f32 = 20.0;

/// LOCALRT-PUNCT-TIMER-269：预览标点刷新间隔。
///
/// 实测 `add_punctuation`（CT-Transformer，单线程 CPU）约 **0.15ms/字**：
/// 20/50/100/200/400 字 = **3.6 / 8.2 / 15.6 / 30.0 / 58.4 ms**（各 10 次中位）。
/// 4s 一次即使 400 字文本也仅 ~58ms，摊到 4s 窗口对 RTF 影响 <1.5%（可忽略）；
/// 故取 Gavin 参考值 4s。
const PUNCT_REFRESH_INTERVAL: Duration = Duration::from_millis(4000);

/// LOCALRT-PUNCT-TIMER-269-B：静默触发打点的阈值（Gavin：静默 800ms 也打一次）。
///
/// 🔴 **与 sherpa endpoint 完全独立**：本阈值只影响**显示层**刷新（判早了只是标点早出现），
/// 绝不 `reset()` / 切句 / `sentence_id += 1`。切句仍由 `LOCAL_STREAM_RULE2=2.0` 的
/// sherpa endpoint 决定（说话中间换气不会被切句）。两者是不同层的东西，不要混用。
const PUNCT_SILENCE_TRIGGER_MS: f32 = 800.0;

/// LOCALRT-ENDPOINT-284（方案 B）：静默多久触发「影子收尾」——
/// 另起一个 `OnlineStream` 把**当前句**音频喂进去 + `input_finished()`，拿完整结果，
/// **只用于显示**（不 reset / 不切句 / 不动 `sentence_id`）；主 stream 完全不受影响。
///
/// 目的：Gavin 说完停顿 ~300-500ms 最后一个字就出现（不必等 rule2=2.0 的切句）。
/// 默认 400ms，可经 env `LOCAL_RT_SHADOW_MS` 覆盖以实测选值。
const SHADOW_FINALIZE_MS_DEFAULT: f32 = 400.0;

/// LOCALRT-ENDPOINT-284（方案 B）安全上限：影子只收尾「当前句」，若当前句音频超过本值
/// （说明长时间连续说话、rule2=2.0 一直没切句）则跳过本次影子，避免每停顿一次就重解一段
/// 越来越长的音频（O(n²) 开销）。被跳过时打 warn 便于实测评估。
const SHADOW_MAX_AUDIO_SECS: f32 = 12.0;

/// LOCALRT-PARALLEL-ACC-298：派发静音阈值默认值（Gavin 拍板 800ms，不是 400ms）。
///
/// 🔴 **与 sherpa endpoint（rule2=2.0s）完全解耦**：本阈值只管「把已说完的一段派给 accuracy
/// 并行转写」，**绝不触发** `reset()` / 切句 / `sentence_id += 1`（那是显示层的事）。
const ACC_DISPATCH_SILENCE_MS_DEFAULT: f32 = 800.0;

/// LOCALRT-PARALLEL-ACC-298：最小派发片长默认值（**5s**，Gavin 2026-09-21 最终定值）。
///
/// 沿革：初版 3s（主控拍的，未经实测）→ **5s（Gavin 最终定值：切碎了偏差大）**；期间一度议到 4s，最终仍取 5s。
///
/// 800ms 停顿在口语里很密，不设下限会切出 0.5s 碎片（每片固定解码开销 + 上下文过短伤精度）。
/// 片越短，模型可用的声学/语言上下文越少、偏差越大；5s 是「够长不碎、又不至于让短句拿不到并行」
/// 的折中。配套的静默判据 `ACC_DISPATCH_SILENCE_MS_DEFAULT` = 800ms 不变。
///
/// 🔴 注意：LOCALRT-CTX-INJECT-320 之后，片与片之间**已经有上下文传递**
/// （Qwen3 per-stream 注入前序分片文本），故「片短 ⇒ 无上下文」这条旧理由已不完全成立；
/// 但声学上下文仍随片长增加，故仍设下限。
///
/// 仍做成 env 可调（`LOCAL_RT_ACC_MIN_SEG_MS`），端测可比对不同取值。
const ACC_MIN_SEGMENT_MS_DEFAULT: u64 = 5000;

/// LOCALRT-PARALLEL-ACC-298：accuracy 并行派发配置（三个 env 开关，端测可调）。
///
/// | env | 默认 | 用途 |
/// | --- | --- | --- |
/// | `LOCAL_RT_ACC_PARALLEL` | `1`（开） | 总开关；`0` = 逐位退回今天的串行行为 |
/// | `LOCAL_RT_ACC_SILENCE_MS` | `800` | 派发静音阈值 |
/// | `LOCAL_RT_ACC_MIN_SEG_MS` | `3000` | 最小片长 |
#[derive(Debug, Clone, Copy)]
pub struct AccDispatchConfig {
    pub enabled: bool,
    pub silence_ms: f32,
    pub min_seg_ms: u64,
}

impl AccDispatchConfig {
    /// 🔴 常量构造，**无 env 覆盖**（Gavin 2026-09-21 定：不允许开发端与用户端行为不一致）。
    /// 用户机器上不存在任何环境变量；env 覆盖等于给开发机开一条用户永远走不到的路径，
    /// 且测试时 env 残留会让结论失真。要改参数就改常量重新构建 —— 构建出来的就是用户跑的那一份。
    pub fn new() -> Self {
        let enabled = true;
        let silence_ms = ACC_DISPATCH_SILENCE_MS_DEFAULT;
        let min_seg_ms = ACC_MIN_SEGMENT_MS_DEFAULT;
        log::debug!(
            "[LocalRT-DBG-298] acc parallel cfg: enabled={} silence_ms={} min_seg_ms={}",
            enabled,
            silence_ms,
            min_seg_ms
        );
        Self {
            enabled,
            silence_ms,
            min_seg_ms,
        }
    }
}

impl Default for AccDispatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            silence_ms: ACC_DISPATCH_SILENCE_MS_DEFAULT,
            min_seg_ms: ACC_MIN_SEGMENT_MS_DEFAULT,
        }
    }
}

/// PARALLEL-ACC-298：是否把「当前未派发区间」派发给 accuracy 并行 worker。
///
/// 三个条件同时成立：总开关开 且 本轮停顿未派过（latch，防同一停顿重复派）且
/// 连续静默 ≥ `silence_ms`(800) 且 未派发区间样本数 ≥ `min_seg_ms`(3s)。
fn should_dispatch_acc(
    enabled: bool,
    silent_ms: f32,
    pending_samples: usize,
    done_for_pause: bool,
    silence_ms: f32,
    min_seg_ms: u64,
) -> bool {
    if !enabled || done_for_pause {
        return false;
    }
    if silent_ms < silence_ms {
        return false;
    }
    let min_samples = min_seg_ms as usize * SAMPLE_RATE as usize / 1000;
    pending_samples >= min_samples
}

/// PARALLEL-ACC-298：录音结束时是否派发尾片。
///
/// 🔴 **必须有语音才派**：纯静音尾片送 accuracy 会返回空 ⇒ 上层 `all_native` 翻 false ⇒
/// 本地档多跑一遍标点引擎（路由漂移）。但**有语音的尾巴绝不能吞**——宁可多打一次标点也不丢字。
fn should_dispatch_tail(enabled: bool, pending_samples: usize, has_speech: bool) -> bool {
    enabled && has_speech && pending_samples > 0
}

/// LOCALRT-PUNCT-TIMER-269：预览标点的节流缓存。
///
/// 只缓存**标点后的全量文本**与其对应的**原始字节长度**，不缓存原始文本本身
/// （原始全文每帧由 `StreamingAsrState::display_text()` 提供，且恒为裸文本）。
struct PunctPreviewCache {
    /// 上次打点产出的全量标点文本（对应 raw[..raw_len]）。
    prefix: String,
    /// `prefix` 覆盖的原始字节长度（raw 的合法 char 边界）。
    raw_len: usize,
    /// 上次实际调用标点引擎的时刻（`None` = 尚未打过点）。
    last_punct_at: Option<Instant>,
}

/// LOCALRT-PUNCT-TIMER-269：把**原始无标点全文**转成 overlay 预览文本。
///
/// - 距上次打点 ≥ `interval`（或 `force`）：对 `raw_full` **整体重打**一次标点并更新缓存。
///   全量重打能随上下文修正先前标点，比逐句打更准（Gavin 指定）。
/// - 否则：`上次标点文本 + raw_full 新增后缀` —— 新字即时上屏，已出现的标点不闪回。
/// - `engine` 为 `None`（用户关标点）：直接返回 `raw_full`，行为与未启用标点一致，不引入额外开销。
///
/// 🔴 关键：输入始终是**原始裸文本**，绝不把已打点文本再次送引擎 ⇒ 不会「标点重复 / 错乱」
/// （FIX-252 场景不复发）。
///
/// 触发均要求「自上次打点后**有新内容**」：持续静默不会反复重打同一段（269-B 条件③）。
fn preview_display(
    raw_full: &str,
    engine: Option<&mut PunctuationEngine>,
    cache: &mut PunctPreviewCache,
    interval: Duration,
    force: bool,
) -> String {
    let Some(engine) = engine else {
        return raw_full.to_string();
    };
    let now = Instant::now();
    let due = cache
        .last_punct_at
        .map_or(true, |t| now.duration_since(t) >= interval);
    // 有新内容才打：raw 单调增长（greedy 0 回退），长度增长即新内容。
    let has_new = cache.last_punct_at.is_none() || raw_full.len() > cache.raw_len;
    if (force || due) && has_new {
        let t = Instant::now();
        cache.prefix = engine
            .add_punctuation(raw_full)
            .unwrap_or_else(|| raw_full.to_string());
        cache.raw_len = raw_full.len();
        cache.last_punct_at = Some(now);
        log::debug!(
            "LOCALRT-PUNCT-TIMER-269: repunctuated {} chars in {:.1}ms",
            raw_full.chars().count(),
            t.elapsed().as_secs_f64() * 1000.0
        );
    }
    if !cache.prefix.is_empty() && raw_full.len() >= cache.raw_len {
        let mut display = cache.prefix.clone();
        display.push_str(&raw_full[cache.raw_len..]);
        display
    } else {
        raw_full.to_string()
    }
}

/// LOCAL-RT-ENGINE-239-A：构建本地流式 paraformer recognizer（greedy_search）。
///
/// 模型目录（DEC-011，exe 同级 models）：`sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en/`，
/// 用 `encoder.int8.onnx` + `decoder.int8.onnx` + `tokens.txt`。
///
/// 🔴 任一模型文件缺失 ⇒ `Err`（DEC-067 附则一：不降级，整档不可用）。
pub fn create_local_stream_recognizer(model_dir: &Path) -> Result<OnlineRecognizer> {
    let dir = model_dir.join("sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en");
    let enc = dir.join("encoder.int8.onnx");
    let dec = dir.join("decoder.int8.onnx");
    let tok = dir.join("tokens.txt");
    for p in [&enc, &dec, &tok] {
        if !p.exists() {
            anyhow::bail!("本地流式模型文件缺失: {}", p.display());
        }
    }

    let mut c = OnlineRecognizerConfig::default();
    c.model_config.paraformer = OnlineParaformerModelConfig {
        encoder: Some(enc.to_string_lossy().to_string()),
        decoder: Some(dec.to_string_lossy().to_string()),
    };
    c.model_config.tokens = Some(tok.to_string_lossy().to_string());
    // TUNE-STREAM-317：线程数常量（8，沿革与 271 反向数据见常量处）。无 env 覆盖。
    let num_threads = local_stream_num_threads();
    c.model_config.num_threads = num_threads;
    c.model_config.provider = Some("cpu".to_string());
    c.model_config.debug = false;
    // DEC-067：本地预览用 greedy_search（streaming paraformer 仅支持 greedy）
    c.decoding_method = Some("greedy_search".to_string());
    // 端点：不开则 is_endpoint() 永不触发，sentence_end 分支成死代码
    c.enable_endpoint = true;
    // LOCALRT-ENDPOINT-284：rule2 已定案为 2.0（ASR-SEG-229 防复发值）。
    // 原实验用的 env `LOCAL_RT_RULE2` 按「定案后移除」的约定已删除。
    let rule2 = LOCAL_STREAM_RULE2_MIN_TRAILING_SILENCE;
    c.rule1_min_trailing_silence = LOCAL_STREAM_RULE1_MIN_TRAILING_SILENCE;
    c.rule2_min_trailing_silence = rule2;
    c.rule3_min_utterance_length = LOCAL_STREAM_RULE3_MIN_UTTERANCE_LENGTH;
    log::debug!(
        "[LocalRT-DBG-284] rule1={} rule2={} rule3={}",
        LOCAL_STREAM_RULE1_MIN_TRAILING_SILENCE,
        rule2,
        LOCAL_STREAM_RULE3_MIN_UTTERANCE_LENGTH
    );
    // TUNE-STREAM-317：启动打三个调优点的**实际取值**，供端测对账。
    // 第三项 HomophoneReplacer 本单定性为「不做」（rule_fsts 需预编译 FST，见 result），固定 off。
    log::debug!(
        "[LocalRT-DBG-317] stream tuning: num_threads={} homophone_replacer=off provider=cpu decoding=greedy_search endpoint=true",
        num_threads,
    );

    OnlineRecognizer::create(&c).context("创建本地流式 (paraformer) recognizer 失败")
}

/// FIX-LOCALRT-TAILCHAR-291/307：切句 / 收尾确认文本的**三方取最长**（不回退）。
///
/// 291 的「flush 前/后取长者」语义扩为三方（**不新造一套**）：
/// - `main`：主 stream flush 后结果（调用方已做「after 空/更短则回落 before」再传进来）
/// - `full`：**307 整句全量重解码**结果（另起 stream 重喂整句，能补出分块末尾解不出的尾字）
/// - `shadow`：400ms 影子快照（可空），仅作补充候选
///
/// 返回字符数**严格更多**者；等长不切换（优先级 main → full → shadow）⇒ 结果长度恒 ≥ `main`，
/// **绝不回退**且同长输入不抖动。全空返回空串（调用方据此走 EMPTY 分支）。
fn endpoint_confirm_text<'a>(main: &'a str, full: &'a str, shadow: Option<&'a str>) -> &'a str {
    let mut best = main;
    let mut best_len = main.chars().count();
    if full.chars().count() > best_len {
        best = full;
        best_len = full.chars().count();
    }
    if let Some(sh) = shadow {
        if sh.chars().count() > best_len {
            best = sh;
        }
    }
    best
}

/// 本地真流式转录（边收音频边解码），与 `transcribe_streaming_realtime` 平行。
///
/// # 参数
/// - `chunk_rx`：实时音频块（16kHz f32 单声道）；channel 关闭 = 录音结束
/// - `recognizer`：常驻的流式 paraformer recognizer（由 `Transcriber` 预加载，跨调用复用）
/// - `cancel_signal`：置位则提前停止
/// - `punctuation_engine`：LOCALRT-PREVIEW-PUNCT-256，调用方传入的已常驻 CT-Transformer
///   （`None` = 用户关标点）。**仅对已确认句（endpoint=true）打点**；中间句逐帧在变，
///   打点会拖垮 RTF，故不打。引擎不在此新建（否则重复加载 972MB 级模型）。
/// - `silence_threshold`：LOCALRT-PUNCT-TIMER-269-B，静默判定口径（RMS ≤ 阈值即静音，
///   同 `config.audio.silence_threshold`）。用于**独立**统计连续静默 ≥800ms 触发显示层打点；
///   🔴 与 sherpa endpoint 无关，不触发 reset/切句。
/// - `on_result`：文本变化回调，传 `(display_text, display_words)`，与 qwen 路径同构
/// - `acc_cfg`：LOCALRT-PARALLEL-ACC-298 派发配置（`enabled`/`silence_ms`/`min_seg_ms`）；
///   `enabled=false` 时本函数**完全不派发**（逐位退回串行行为）
/// - `on_segment`：accuracy 并行派发回调，传
///   `(seg_index, committed_len, 已加 padding 的 16k f32 子段列表)`。
///   `committed_len` = **派发当刻浮层已显示文本的字符数**（325 回灌边界；取 `last_display`，
///   与消费侧 `last_streaming_text` 镜像同源）。
///   子段列表通常 1 个；未派发区间超 20s 时由 `build_padded_segments` 硬切为多个。
///   🔴 只在 `acc_cfg.enabled` 且满足 800ms/3s/latch 条件时被调用
///
/// # 返回
/// `(final_preview, pcm)`：
/// - `final_preview` = 收尾时最终（含标点）预览全文（282：供调用方以 `StreamingFinalPreview`
///   收尾显示；239-B 丢弃预览文本、以 `pcm` 走 accuracy 2pass，本值不参与最终文本）
/// - `pcm` = 本次收到的全部音频（16kHz f32），供 2pass 离线纠错复用（≈64KB/s）
///
/// 采用**返回值**而非出参传 PCM：2pass 的 PCM 是必需环节，出参漏传编译器抓不到。
pub fn transcribe_streaming_local(
    chunk_rx: crossbeam_channel::Receiver<Vec<f32>>,
    recognizer: &OnlineRecognizer,
    cancel_signal: Option<&AtomicBool>,
    mut punctuation_engine: Option<&mut PunctuationEngine>,
    silence_threshold: f32,
    mut on_result: impl FnMut(&str, &[WordTiming]),
    acc_cfg: AccDispatchConfig,
    mut on_segment: impl FnMut(usize, usize, Vec<Vec<f32>>),
) -> Result<(String, Vec<f32>)> {
    let is_cancelled = || {
        cancel_signal
            .map(|s| s.load(Ordering::Relaxed))
            .unwrap_or(false)
    };

    let mut stream = recognizer.create_stream();
    let mut state = StreamingAsrState::new();
    let mut sentence_id: i64 = 0;
    let mut last_display = String::new();
    // LOCALRT-LASTCHAR-276 诊断用：上一次 get_result 的文本（只在变化时打日志，避免每帧刷屏）。
    let mut last_result_text = String::new();
    let mut pcm: Vec<f32> = Vec::new();
    // LOCALRT-PUNCT-TIMER-269：预览标点节流缓存（只对裸文本打点，杜绝重复）。
    let mut punct_cache = PunctPreviewCache {
        prefix: String::new(),
        raw_len: 0,
        last_punct_at: None,
    };
    // LOCALRT-PUNCT-TIMER-269-B：连续静默累计（ms）。独立于 sherpa endpoint，只驱动显示层打点。
    let mut silent_ms: f32 = 0.0;

    // LOCALRT-ENDPOINT-284（方案 B）：影子收尾（只动显示层）。
    let shadow_trigger_ms: f32 = SHADOW_FINALIZE_MS_DEFAULT;
    log::debug!("[LocalRT-DBG-284] shadow_trigger_ms={}", shadow_trigger_ms);
    // 当前句音频在 `pcm` 中的起点（上次 endpoint reset 之后）。
    let mut sentence_pcm_start: usize = 0;
    // 影子收尾产出的当前句文本（仅显示；新语音进来即作废）。
    let mut shadow_current: Option<String> = None;
    // 本轮静默是否已跑过影子（同一停顿不重复解码）。
    let mut shadow_done_for_pause = false;
    // 影子触发计数（284 实测触发频率用）。
    let mut shadow_count: u32 = 0;

    // PARALLEL-ACC-298：accuracy 并行派发状态（与影子/端点**完全独立**）。
    // 已派发到的 `pcm` 位置（下一片从这里起算）。
    let mut acc_dispatched_end: usize = 0;
    // 本轮静默是否已派发过（同 shadow_done_for_pause 的 latch 写法）。
    let mut acc_done_for_pause = false;
    // 本次未派发区间内是否出现过语音（尾片「有语音才派」的判据）。
    let mut acc_pending_has_speech = false;
    // 已派发片数（= 下一片的 seg_index）。
    let mut acc_seg_index: usize = 0;

    // LOCALRT-FIRSTCHAR-272：首字延迟埋点（**永久观测**，对齐现有 `[Latency] … at +N.Nms` 风格）。
    // t0 = 本函数进入时刻（ASR 线程起跑），把首字延迟拆成三段：
    //   等音频（chunk/accept）→ 等推理（is_ready→get_result 非空）→ 等回调（on_result）。
    // 🔴 仅在各自**首次**触发时打（bool 门），不每 chunk 打（否则日志爆）；纯 bool 检查，无性能影响。
    let t0 = Instant::now();
    let mut first_chunk_seen = false;
    let mut first_accept_seen = false;
    let mut first_ready_seen = false;
    let mut first_result_seen = false;
    let mut first_callback_seen = false;
    let mut t_chunk_ms = 0.0f64;
    let mut t_accept_ms = 0.0f64;
    let mut t_ready_ms = 0.0f64;
    let mut t_result_ms = 0.0f64;

    loop {
        if is_cancelled() {
            log::info!("Local streaming ASR: cancelled");
            break;
        }

        // channel 关闭 = 录音结束，跳出后走 flush。
        let chunk = match chunk_rx.recv() {
            Ok(c) => c,
            Err(_) => break,
        };
        if chunk.is_empty() {
            continue;
        }
        if !first_chunk_seen {
            first_chunk_seen = true;
            t_chunk_ms = t0.elapsed().as_secs_f64() * 1000.0;
            log::info!(
                "[Latency] local_stream first_chunk at +{:.1}ms ({} samples)",
                t_chunk_ms,
                chunk.len()
            );
        }

        // LOCALRT-PUNCT-TIMER-269-B：独立静默计数（口径同 config.audio.silence_threshold：
        // RMS ≤ 阈值即静音）。🔴 只驱动显示层打点，**绝不触发 sherpa endpoint / reset / 切句**——
        // 切句仍由 rule2=2.0 的端点在静默 2s 时决定，说话中间换气（~1s）不会被切。
        let chunk_ms = chunk.len() as f32 / SAMPLE_RATE as f32 * 1000.0;
        let chunk_rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
        if chunk_rms <= silence_threshold {
            silent_ms += chunk_ms;
        } else {
            silent_ms = 0.0;
            // LOCALRT-ENDPOINT-284：有新语音进来 → 允许本轮停顿结束后再触发影子。
            // 不立即清 shadow_current：显示侧用「取更长者」避免瞬时缩短/闪烁（见下 base 选择）。
            shadow_done_for_pause = false;
            // PARALLEL-ACC-298：有新语音 → 允许本轮停顿结束后再派发，并标记待派发区间含语音。
            acc_done_for_pause = false;
            acc_pending_has_speech = true;
        }

        pcm.extend_from_slice(&chunk);
        stream.accept_waveform(SAMPLE_RATE, &chunk);
        if !first_accept_seen {
            first_accept_seen = true;
            t_accept_ms = t0.elapsed().as_secs_f64() * 1000.0;
            log::info!(
                "[Latency] local_stream first_accept_waveform at +{:.1}ms",
                t_accept_ms
            );
        }
        while recognizer.is_ready(&stream) {
            if !first_ready_seen {
                first_ready_seen = true;
                t_ready_ms = t0.elapsed().as_secs_f64() * 1000.0;
                log::info!(
                    "[Latency] local_stream first_is_ready at +{:.1}ms (audio buffered enough to decode)",
                    t_ready_ms
                );
            }
            recognizer.decode(&stream);
        }

        let endpoint = recognizer.is_endpoint(&stream);

        if endpoint {
            // FIX-LOCALRT-TAILCHAR-291：endpoint 切句过去直接 `reset()`，而 reset 之前从未
            // `input_finished()` ⇒ sherpa 解码器里压着的最后一个 token 被直接丢掉，**每一个
            // 被 endpoint 切掉的句子都结构性少尾字**（Gavin 端测「中间句丢尾字」的根因）。
            // 最后一句因 loop 之后的 flush 而完整，这正是「只有中间句丢」的原因。
            //
            // 修法：reset 之前先 flush 主 stream，取到真正完整的句子文本再确认。
            // flush 前文本仅作**回落**用（见 `endpoint_confirm_text`）。
            let before_text = recognizer
                .get_result(&stream)
                .map(|r| r.text)
                .unwrap_or_default();
            // 🔴 input_finished() 之后的 stream 不能再喂音频 ⇒ 本句结束即换新流（见下）。
            stream.input_finished();
            while recognizer.is_ready(&stream) {
                recognizer.decode(&stream);
            }
            let after_text = recognizer
                .get_result(&stream)
                .map(|r| r.text)
                .unwrap_or_default();
            if log::log_enabled!(log::Level::Debug) {
                log::debug!(
                    "[LocalRT-DBG-291] endpoint flush: before_len={} after_len={} gained={}",
                    before_text.chars().count(),
                    after_text.chars().count(),
                    after_text.chars().count() as i64 - before_text.chars().count() as i64
                );
            }
            // 🔴 显式回落：flush 后为空 / 更短则用 flush 前，绝不让整句消失或回退。
            let flush_text: &str = if after_text.chars().count() >= before_text.chars().count() {
                &after_text
            } else {
                &before_text
            };
            // FIX-LOCALRT-TAILCHAR-307：整句全量重解码。291 的 `[LocalRT-DBG-291] … gained`
            // 实测**恒 0** ⇒ 流式分块编码下，末尾不足一块的音频靠 `input_finished()` 信号补不出；
            // 真正能补出尾字的是**另起全新 stream 重喂整句重解码**（分块边界不同）。
            // 🔴 不设时长上限：句子最长受 rule3=20s 约束（20s 解码约 700ms，且已在静默 2s 之后，
            // 用户基本无感），且只影响预览（最终文本走 accuracy 2pass）；SHADOW_MAX_AUDIO_SECS
            // 是给 400ms 影子用的，**不适用于此处**。
            let t_full = log::log_enabled!(log::Level::Debug).then(Instant::now);
            let full = recognizer.create_stream();
            full.accept_waveform(SAMPLE_RATE, &pcm[sentence_pcm_start..]);
            full.input_finished();
            while recognizer.is_ready(&full) {
                recognizer.decode(&full);
            }
            let full_text = recognizer
                .get_result(&full)
                .map(|r| r.text)
                .unwrap_or_default();
            if log::log_enabled!(log::Level::Debug) {
                log::debug!(
                    "[LocalRT-DBG-307] endpoint full-decode: main_len={} full_len={} gained={} decode={:.1}ms",
                    flush_text.chars().count(),
                    full_text.chars().count(),
                    full_text.chars().count() as i64 - flush_text.chars().count() as i64,
                    t_full.map(|t| t.elapsed().as_secs_f64() * 1000.0).unwrap_or(0.0)
                );
            }
            // FIX-LOCALRT-TAILCHAR-291/307：三方取最长（main=flush / full=全量重解码 / shadow）。
            // 307 后预期 full 多数时候最长 ⇒ 尾字不再等下一句。影子保留（服务「停顿中提前显示」）。
            let confirm_text: &str =
                endpoint_confirm_text(flush_text, &full_text, shadow_current.as_deref());
            if !confirm_text.is_empty() {
                // LOCALRT-FIRSTCHAR-272：首次拿到非空识别文本（首次推理产出）。
                if !first_result_seen {
                    first_result_seen = true;
                    t_result_ms = t0.elapsed().as_secs_f64() * 1000.0;
                    log::info!(
                        "[Latency] local_stream first_nonempty_result at +{:.1}ms ({} chars)",
                        t_result_ms,
                        confirm_text.chars().count()
                    );
                }
                // URGENT-286：诊断块仅在 Debug 级启用时执行 ⇒ 默认 Warn 下零开销。
                if log::log_enabled!(log::Level::Debug) {
                    // 307：used 扩为 main / full / shadow 三源（按指针判定胜出者）。
                    let used = if std::ptr::eq(confirm_text, full_text.as_str()) {
                        "full"
                    } else if shadow_current
                        .as_deref()
                        .is_some_and(|s| std::ptr::eq(confirm_text, s))
                    {
                        "shadow"
                    } else {
                        "main"
                    };
                    log::debug!(
                        "[LocalRT-DBG-289] endpoint confirm: main_len={} full_len={} shadow_len={} used={}",
                        flush_text.chars().count(),
                        full_text.chars().count(),
                        shadow_current
                            .as_ref()
                            .map(|s| s.chars().count())
                            .unwrap_or(0),
                        used
                    );
                    if confirm_text != last_result_text {
                        log::debug!(
                            "[LocalRT-DBG-276] result='{}' (chars={}) endpoint=true will_reset=true",
                            confirm_text,
                            confirm_text.chars().count()
                        );
                        last_result_text = confirm_text.to_string();
                    }
                }
                // 🔴 始终把**原始无标点**文本喂状态机，且本句**只确认这一次**（避免 FIX-252 重复打点）。
                state.on_result(sentence_id, confirm_text, true, &[]);
            } else {
                log::debug!(
                    "[LocalRT-DBG-276] endpoint=true but result EMPTY (prev='{}')",
                    last_result_text
                );
            }
            // LOCALRT-ENDPOINT-284 诊断（方案 A 取证）：每次切句打一行，行数=切句次数。
            log::debug!(
                "[LocalRT-DBG-284] endpoint fired: sentence_id {} -> {} (rule2 cut)",
                sentence_id,
                sentence_id + 1
            );
            // 🔴 input_finished() 之后的 stream 不可复用，下一句换新流（create_stream 廉价：
            // 影子逻辑每 400ms 就建一次，decode 0.5~0.7ms 起步，无性能顾虑）。
            stream = recognizer.create_stream();
            sentence_id += 1;
            // 新句从当前总音频长度起算；作废影子。
            sentence_pcm_start = pcm.len();
            shadow_current = None;
            shadow_done_for_pause = false;
        } else if let Some(r) = recognizer.get_result(&stream) {
            if !r.text.is_empty() {
                // LOCALRT-LASTCHAR-276：诊断——每次识别文本变化时打印，用于判断「最后一个字
                // 是否在任何一帧 get_result 里出现过」。只在变化时打，有界。
                // URGENT-286：整块（比较 + clone + 格式化）仅在 Debug 级启用时执行 ⇒ 默认 Warn 下零开销。
                if log::log_enabled!(log::Level::Debug) && r.text != last_result_text {
                    log::debug!(
                        "[LocalRT-DBG-276] result='{}' (chars={}) endpoint=false will_reset=false",
                        r.text,
                        r.text.chars().count()
                    );
                    last_result_text = r.text.clone();
                }
                // LOCALRT-FIRSTCHAR-272：首次拿到非空识别文本（首次推理产出）。
                if !first_result_seen {
                    first_result_seen = true;
                    t_result_ms = t0.elapsed().as_secs_f64() * 1000.0;
                    log::info!(
                        "[Latency] local_stream first_nonempty_result at +{:.1}ms ({} chars)",
                        t_result_ms,
                        r.text.chars().count()
                    );
                }
                // 🔴 始终把**原始无标点**文本喂状态机：confirmed/current 均保持裸文本，
                // 杜绝「对已打点文本二次打点 / 标点重复」（FIX-252 场景）。
                // 非 endpoint → 仅替换当前句中间结果（本函数内唯一另一处 on_result）。
                state.on_result(sentence_id, &r.text, false, &[]);
            }
        }

        // LOCALRT-ENDPOINT-284（方案 B）：静默 ≥ 阈值 → 影子 stream 收尾当前句（**只动显示**）。
        // 影子 = 另起 OnlineStream 喂当前句音频 + input_finished()，拿完整结果；主 stream 不受影响。
        let mut shadow_fired = false;
        if silent_ms >= shadow_trigger_ms && !shadow_done_for_pause {
            shadow_done_for_pause = true;
            let shadow_audio = &pcm[sentence_pcm_start..];
            let shadow_secs = shadow_audio.len() as f32 / SAMPLE_RATE as f32;
            if !shadow_audio.is_empty() && shadow_secs > SHADOW_MAX_AUDIO_SECS {
                log::debug!(
                    "[LocalRT-DBG-284] shadow skipped: current sentence {:.1}s > cap {:.1}s (no endpoint yet)",
                    shadow_secs,
                    SHADOW_MAX_AUDIO_SECS
                );
            } else if !shadow_audio.is_empty() {
                // URGENT-286：decode 计时只为日志用 ⇒ 仅 Debug 级才取样（默认 Warn 下零开销）。
                let t_shadow = log::log_enabled!(log::Level::Debug).then(Instant::now);
                let shadow = recognizer.create_stream();
                shadow.accept_waveform(SAMPLE_RATE, shadow_audio);
                shadow.input_finished();
                while recognizer.is_ready(&shadow) {
                    recognizer.decode(&shadow);
                }
                // FIX-289：影子结果为空 ⇒ 置 None（不保留上一轮陈旧值，避免被误当「更完整」）。
                shadow_current = recognizer
                    .get_result(&shadow)
                    .map(|r| r.text)
                    .filter(|t| !t.is_empty());
                shadow_count += 1;
                if let Some(t0) = t_shadow {
                    log::debug!(
                        "[LocalRT-DBG-284] shadow finalize #{}: silence={:.0}ms sentence_audio={:.2}s decode={:.1}ms text_len={}",
                        shadow_count,
                        silent_ms,
                        shadow_audio.len() as f32 / SAMPLE_RATE as f32,
                        t0.elapsed().as_secs_f64() * 1000.0,
                        shadow_current
                            .as_ref()
                            .map(|s| s.chars().count())
                            .unwrap_or(0)
                    );
                }
                shadow_fired = true;
            }
        }

        // PARALLEL-ACC-298：静默 ≥800ms 且当前未派发区间 ≥3s ⇒ 把这一段派给 accuracy 并行 worker。
        // 🔴 只推进 `acc_dispatched_end`，**不 reset / 不切句 / 不动 sentence_id**（与 endpoint 解耦）。
        // 复用 `build_padded_segments`：自动加 200ms 边界 padding，且 >20s 的片会硬切成多个子段
        // （native `max_total_len=512` 的硬限制，禁止整段 >20s 喂 accuracy）。
        if should_dispatch_acc(
            acc_cfg.enabled,
            silent_ms,
            pcm.len().saturating_sub(acc_dispatched_end),
            acc_done_for_pause,
            acc_cfg.silence_ms,
            acc_cfg.min_seg_ms,
        ) {
            let pending = pcm.len() - acc_dispatched_end;
            let raw = [(acc_dispatched_end, pending)];
            let padded = super::build_padded_segments(&raw, pcm.len(), &pcm);
            if !padded.is_empty() {
                if log::log_enabled!(log::Level::Debug) {
                    log::debug!(
                        "[LocalRT-DBG-298] seg dispatch #{}: silence={:.0}ms seg_audio={:.2}s pcm_pos={}",
                        acc_seg_index,
                        silent_ms,
                        pending as f32 / SAMPLE_RATE as f32,
                        acc_dispatched_end
                    );
                }
                // ACC-PREVIEW-REFLOW-325：记下**派发当刻浮层已显示文本的字符数**（committed_len）。
                // 🔴 用 `last_display`（实际已上屏、含标点的串，与镜像 `last_streaming_text` 同源），
                //    不用 `state.display_text()`（裸文本，标点差会造成回填边界偏移）。
                let committed_len = last_display.chars().count();
                on_segment(acc_seg_index, committed_len, padded);
                acc_seg_index += 1;
            }
            // 无论 padded 是否为空都推进起点 + 上 latch：避免同一停顿反复尝试。
            acc_dispatched_end = pcm.len();
            acc_pending_has_speech = false;
            acc_done_for_pause = true;
        }

        // LOCALRT-PUNCT-TIMER-269-B：显示刷新，两条件「或」——
        //   ① 距上次打点 ≥4s（preview_display 内判 due）
        //   ② 连续静默 ≥800ms（本处独立计数）且自上次打点后有新内容
        // 两条件都**只刷新显示**，不动状态机（不 reset / 不切句 / 不改 sentence_id）。
        // 显示基文本：影子收尾（confirmed + 影子当前句）与主 stream（confirmed + current）**取更长者**，
        // 避免新语音进来后主文本还没追上时显示瞬时缩短/闪烁。
        let main_view = state.display_text();
        let shadow_view = shadow_current.as_ref().map(|sh| {
            let mut s = state.confirmed_text();
            s.push_str(sh);
            s
        });
        let raw_full = match shadow_view {
            Some(sv) if sv.len() > main_view.len() => sv,
            _ => main_view,
        };
        if !raw_full.is_empty() {
            let has_new =
                punct_cache.last_punct_at.is_none() || raw_full.len() > punct_cache.raw_len;
            let silence_due = silent_ms >= PUNCT_SILENCE_TRIGGER_MS && has_new;
            let display = preview_display(
                &raw_full,
                punctuation_engine.as_deref_mut(),
                &mut punct_cache,
                PUNCT_REFRESH_INTERVAL,
                shadow_fired || silence_due,
            );
            if silence_due {
                // 打完重置静默计数；4s 计时由 preview_display 内的 last_punct_at 一并重置。
                silent_ms = 0.0;
            }
            if display != last_display {
                // LOCALRT-FIRSTCHAR-272：首次回调 = 用户看到第一个字；此处打「首字延迟拆解」汇总行。
                if !first_callback_seen {
                    first_callback_seen = true;
                    let t_callback_ms = t0.elapsed().as_secs_f64() * 1000.0;
                    log::info!(
                        "[Latency] local_stream FIRST-CHAR breakdown: chunk=+{:.1}ms accept=+{:.1}ms ready=+{:.1}ms result=+{:.1}ms callback=+{:.1}ms | wait_audio(chunk-t0)={:.1}ms wait_infer(ready->result)={:.1}ms wait_callback(result->callback)={:.1}ms",
                        t_chunk_ms,
                        t_accept_ms,
                        t_ready_ms,
                        t_result_ms,
                        t_callback_ms,
                        t_chunk_ms,
                        t_result_ms - t_ready_ms,
                        t_callback_ms - t_result_ms
                    );
                    // RESEARCH-ACC-FIRSTCHAR-278 埋点（只读）：流式首字文本 + 到达时刻。
                    // URGENT-286：降为 debug 级——默认 max_level=Warn 下连参数求值都跳过。
                    // 参数 t_callback_ms/display 均为已有廉价值（且 t_callback_ms 被上方
                    // [Latency] info! 复用），故无需再包 log_enabled! 守卫。
                    log::debug!(
                        "[LocalRT-DBG-278] local streaming first text @+{:.0}ms: {}",
                        t_callback_ms,
                        display
                    );
                }
                on_result(&display, &state.display_words());
                last_display = display;
            }
        }
    }

    // PARALLEL-ACC-298：尾片 —— 录音结束，把剩余未派发区间作为最后一片派给 accuracy。
    // 短录音（总时长 < min_seg_ms）时未派发区间 = 全量 ⇒ 等价今天的「单片=全量」。
    // 🔴 只有「本段出现过语音」才派（纯静音尾片会把 all_native 翻 false）；有语音的尾巴绝不吞。
    if should_dispatch_tail(
        acc_cfg.enabled,
        pcm.len().saturating_sub(acc_dispatched_end),
        acc_pending_has_speech,
    ) {
        let pending = pcm.len() - acc_dispatched_end;
        let raw = [(acc_dispatched_end, pending)];
        let padded = super::build_padded_segments(&raw, pcm.len(), &pcm);
        if !padded.is_empty() {
            if log::log_enabled!(log::Level::Debug) {
                log::debug!(
                    "[LocalRT-DBG-298] seg dispatch #{} (tail): seg_audio={:.2}s pcm_pos={}",
                    acc_seg_index,
                    pending as f32 / SAMPLE_RATE as f32,
                    acc_dispatched_end
                );
            }
            let committed_len = last_display.chars().count();
            on_segment(acc_seg_index, committed_len, padded);
        }
        // 尾片是最后一次派发：函数即将返回，无需再推进 `acc_dispatched_end`/`acc_seg_index`
        // （推进了也无人读 ⇒ 触发 unused_assignments）。
    }

    // flush 尾部：input_finished 后把剩余可解码帧吐完，再取最终结果（不丢尾字）。
    stream.input_finished();
    while recognizer.is_ready(&stream) {
        recognizer.decode(&stream);
    }
    let main_final = recognizer
        .get_result(&stream)
        .map(|r| r.text)
        .unwrap_or_default();
    // FIX-LOCALRT-TAILCHAR-307（Gavin 第二症状：录音最后一个字不显示）：收尾段与 endpoint
    // 是**同一失效机制**（291 的 `gained` 恒 0）⇒ 同样对**最后一句**做整句全量重解码，
    // 与收尾 flush 结果取更长者。
    let t_full = log::log_enabled!(log::Level::Debug).then(Instant::now);
    let full = recognizer.create_stream();
    full.accept_waveform(SAMPLE_RATE, &pcm[sentence_pcm_start..]);
    full.input_finished();
    while recognizer.is_ready(&full) {
        recognizer.decode(&full);
    }
    let full_final = recognizer
        .get_result(&full)
        .map(|r| r.text)
        .unwrap_or_default();
    if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "[LocalRT-DBG-307] final full-decode: main_len={} full_len={} gained={} decode={:.1}ms",
            main_final.chars().count(),
            full_final.chars().count(),
            full_final.chars().count() as i64 - main_final.chars().count() as i64,
            t_full
                .map(|t| t.elapsed().as_secs_f64() * 1000.0)
                .unwrap_or(0.0)
        );
    }
    // 取更长者（与 endpoint 同一「不回退」语义）；全空则跳过（沿用原行为）。
    let final_seg = if full_final.chars().count() > main_final.chars().count() {
        full_final
    } else {
        main_final
    };
    // LOCALRT-FIRSTCHAR-282：最终（含标点）预览全文；供调用方以 `StreamingFinalPreview` 收尾显示。
    // （239-B 丢弃本预览文本、以 pcm 走 accuracy 2pass；此返回值只服务于收尾显示。）
    let mut final_preview = last_display.clone();
    if !final_seg.is_empty() {
        state.on_result(sentence_id, &final_seg, false, &[]);
        // 收尾强制打点一次，保证关闭前的预览带标点。
        let raw_full = state.display_text();
        let display = preview_display(
            &raw_full,
            punctuation_engine.as_deref_mut(),
            &mut punct_cache,
            PUNCT_REFRESH_INTERVAL,
            true,
        );
        if !display.is_empty() {
            if display != last_display {
                on_result(&display, &state.display_words());
            }
            final_preview = display;
        }
    }

    Ok((final_preview, pcm))
}

#[cfg(test)]
mod tests {
    use super::{
        endpoint_confirm_text, local_stream_num_threads, should_dispatch_acc, should_dispatch_tail,
        SAMPLE_RATE,
    };

    /// FIX-LOCALRT-TAILCHAR-291/307：三方取最长（main / full / shadow），结果恒 ≥ main。
    ///
    /// 覆盖：full 更长取 full（307 主路径）；full 为空/更短回落 main；shadow 更长取 shadow；
    /// 三者等长不抖动（取 main）；全空返回空（调用方走 EMPTY 分支）。
    #[test]
    fn endpoint_confirm_text_takes_longest_of_three() {
        // full 更长（307 主路径：整句全量重解码补出尾字）。
        assert_eq!(
            endpoint_confirm_text("看看有什么好看的电", "看看有什么好看的电影", None),
            "看看有什么好看的电影"
        );
        // full 为空 ⇒ 回落 main，句子不消失。
        assert_eq!(
            endpoint_confirm_text("端测发现的问", "", None),
            "端测发现的问"
        );
        // full 更短 ⇒ 回落 main（绝不回退）。
        assert_eq!(endpoint_confirm_text("你好世界", "你好", None), "你好世界");
        // shadow 更长 ⇒ 取 shadow。
        assert_eq!(
            endpoint_confirm_text("看看有什么好看的电", "", Some("看看有什么好看的电影")),
            "看看有什么好看的电影"
        );
        // 三者等长 ⇒ 取 main（不抖动；main 与 full 等长时不被 full 顶掉）。
        assert_eq!(endpoint_confirm_text("你好", "您好", Some("您好")), "你好");
        // 全空 ⇒ 空（调用方据此走 EMPTY 分支，不做确认）。
        assert_eq!(endpoint_confirm_text("", "", None), "");
    }

    /// 判据 #4 的不变量：三方取最长后，长度恒 ≥ main（按字符计）。
    #[test]
    fn endpoint_confirm_text_len_not_below_main() {
        let cases: [(&str, &str, Option<&str>); 6] = [
            ("", "", None),
            ("", "尾", None),
            ("端测发现的问", "", None),
            ("端测发现的问", "端测发现的问题", None),
            ("长一点的前文", "短", Some("更短")),
            ("ab", "abcd", Some("abc")),
        ];
        for (main, full, shadow) in cases {
            let chosen = endpoint_confirm_text(main, full, shadow);
            assert!(
                chosen.chars().count() >= main.chars().count(),
                "main={main:?} full={full:?} shadow={shadow:?} chosen={chosen:?}"
            );
        }
    }

    // ========================================================================
    // GUARD-291 · FIX-LOCALRT-TAILCHAR-291 源码级结构护栏（交叉，非作者：coder-1）
    //
    // 行为级护栏在这里做不了：`OnlineRecognizer` 需要真模型，单测里起不来。
    // 故照仓库既有源码结构护栏写法（`src/main.rs::overlay_121_guard_tests`）：
    // 读本文件生产区源码 → needle 计数 → 花括号定界取块。
    //
    // 钉死的四条不变式（I1~I4 见 291 handoff）：
    //   I1 endpoint 分支 `stream.input_finished()` 必须早于「取 after_text 的 get_result」
    //   I2 endpoint 分支不得再出现 `recognizer.reset(`
    //   I3 endpoint 分支必须用 `recognizer.create_stream()` 换新流（307 后为 2 处：全量重解码 + 换流）
    //   I4 本句 `state.on_result(..., true, ...)` 只确认一次
    // ========================================================================

    /// 本文件生产区（首个 `#[cfg(test)]` 之前）的逐行 trim 文本。
    fn ls_prod_lines() -> Vec<String> {
        let mut out = Vec::new();
        for line in include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/transcription/local_stream.rs"
        ))
        .lines()
        {
            let t = line.trim();
            if t.starts_with("#[cfg(test)]") {
                break;
            }
            out.push(t.to_string());
        }
        out
    }

    fn brace_delta_291(line: &str) -> i32 {
        line.matches('{').count() as i32 - line.matches('}').count() as i32
    }

    /// 花括号定界：以 anchor 行为起点，返回 (open_idx, close_idx)。
    fn block_bounds_291(lines: &[String], anchor: usize) -> (usize, usize) {
        let mut depth = 0i32;
        let mut opened = false;
        let mut open_idx = usize::MAX;
        for (i, line) in lines.iter().enumerate().skip(anchor) {
            depth += brace_delta_291(line);
            if depth > 0 && !opened {
                opened = true;
                open_idx = i;
            }
            if opened && depth == 0 {
                return (open_idx, i);
            }
        }
        panic!("GUARD-291: 未能用花括号定界 anchor={anchor} 起的块（源码结构已变）");
    }

    /// 生产区中首个 `startswith(needle)` 的 0-based 行号。
    fn first_line(lines: &[String], needle: &str) -> usize {
        lines
            .iter()
            .position(|l| l.starts_with(needle))
            .unwrap_or_else(|| panic!("GUARD-291: 定位锚点不存在: {needle}"))
    }

    /// 定界 endpoint **真分支**（`if endpoint {` 到其闭合 `}` 的前一行）。
    ///
    /// 🔴 FIX-GUARD-297：原实现用「首个 `} else`」截断，会命中分支内**嵌套**的
    /// `let confirm_text = if use_shadow { … } else { … }`（`:400`），把真正的
    /// `recognizer.create_stream()`（`:450`）排除出扫描区 ⇒ G3 假红。
    ///
    /// 现改为**单遍花括号游标**：从 `if endpoint {` 起维护相对 `depth`；当 `depth == 1`
    /// 且行首为 `}` 时，该 `}` 就是真分支的闭合花括号（Rust 的 `} else if … {` 里第一个
    /// `}` 正是关真分支的），分支体止于其前一行。对任意层嵌套都成立——因为嵌套的 `}`
    /// 出现时 `depth ≥ 2`，不会被 `depth == 1` 命中。
    fn endpoint_branch_bounds(lines: &[String], ep_anchor: usize) -> (usize, usize) {
        let mut depth = 0i32;
        let mut open_idx = usize::MAX;
        for (i, line) in lines.iter().enumerate().skip(ep_anchor) {
            if open_idx != usize::MAX && depth == 1 && line.starts_with('}') {
                return (open_idx, i - 1);
            }
            depth += brace_delta_291(line);
            if open_idx == usize::MAX && depth > 0 {
                open_idx = i;
            }
        }
        panic!("GUARD-297: 未能定界 endpoint 真分支（anchor={ep_anchor}，源码结构已变）");
    }

    /// 返回 (`生产区全部行`, `endpoint 真分支范围`, `函数体范围`)。
    fn endpoint_guard_regions() -> (Vec<String>, (usize, usize), (usize, usize)) {
        let lines = ls_prod_lines();
        let fn_line = first_line(&lines, "pub fn transcribe_streaming_local(");
        let (fn_lo, fn_hi) = block_bounds_291(&lines, fn_line);
        let ep_anchor = fn_lo
            + lines[fn_lo..=fn_hi]
                .iter()
                .position(|l| l.starts_with("if endpoint {"))
                .expect("GUARD-291: endpoint 分支锚点 `if endpoint {` 不存在");
        let (ep_lo, ep_hi) = endpoint_branch_bounds(&lines, ep_anchor);
        // 自证区间「既不短也不长」：首行是 if 锚点；区间末行的下一行必须以 `}` 开头
        // （即真分支的闭合花括号行）。任一不满足 ⇒ 区域定界又错了 ⇒ 立刻红。
        assert!(
            lines[ep_lo].starts_with("if endpoint {"),
            "GUARD-297: endpoint 区间首行应为 `if endpoint {{`，实测: {}",
            lines[ep_lo]
        );
        assert!(
            ep_hi + 1 < lines.len() && lines[ep_hi + 1].starts_with('}'),
            "GUARD-297: endpoint 区间末行的下一行应为闭合花括号，实测: {}",
            lines.get(ep_hi + 1).map(String::as_str).unwrap_or("<EOF>")
        );
        (lines, (ep_lo, ep_hi), (fn_lo, fn_hi))
    }

    fn count_starts(lines: &[String], lo: usize, hi: usize, needle: &str) -> usize {
        lines[lo..=hi]
            .iter()
            .filter(|l| l.starts_with(needle))
            .count()
    }

    fn count_contains(lines: &[String], lo: usize, hi: usize, needle: &str) -> usize {
        lines[lo..=hi].iter().filter(|l| l.contains(needle)).count()
    }

    /// G1（I1）：endpoint 分支内 `stream.input_finished()` 必须夹在两次
    /// `.get_result(&stream)` 之间（flush 前 before_text ＜ flush ＜ flush 后 after_text）。
    ///
    /// **改错怎么红**：把 flush 删掉 ⇒ 找不到 `stream.input_finished()` ⇒ panic 红；
    /// 把 flush 挪到取 after_text 之后 ⇒ after 侧不再有 get_result（计数/顺序断言红）。
    #[test]
    fn guard291_g1_flush_before_after_text() {
        let (lines, (lo, hi), _) = endpoint_guard_regions();
        let fi = (lo..=hi)
            .find(|&i| lines[i].starts_with("stream.input_finished()"))
            .expect("G1: endpoint 分支内必须有 stream.input_finished()（flush 被删 ⇒ 红）");
        let grs: Vec<usize> = (lo..=hi)
            .filter(|&i| lines[i].starts_with(".get_result(&stream)"))
            .collect();
        assert_eq!(
            grs.len(),
            2,
            "G1: endpoint 分支内 .get_result(&stream) 应恰 2 处（before_text / after_text），实测 {:?}",
            grs
        );
        assert!(
            grs[0] < fi && fi < grs[1],
            "G1: input_finished() 必须位于两次 get_result 之间（before<flush<after），实测 flush={fi} gets={grs:?}"
        );
    }

    /// G2（I2）：`transcribe_streaming_local` 函数体内 `recognizer.reset(` 计数必须为 0。
    ///
    /// **改错怎么红**：任何地方把换新流改回 `recognizer.reset(` ⇒ 计数 ≥1 ⇒ 红。
    #[test]
    fn guard291_g2_no_recognizer_reset() {
        let (lines, _, (fn_lo, fn_hi)) = endpoint_guard_regions();
        let n = count_contains(&lines, fn_lo, fn_hi, "recognizer.reset(");
        assert_eq!(
            n, 0,
            "G2: input_finished() 后旧流不可复用 ⇒ 函数体内不得出现 recognizer.reset(，实测 {n} 处"
        );
    }

    /// G3（I3，307 后修订）：endpoint 分支内 `recognizer.create_stream()` 恰 **2** 处 ——
    /// ① 307 整句全量重解码（另起 stream 重喂整句，补分块末尾尾字）
    /// ② 291 句末换新流（`input_finished()` 后旧流不可复用）。
    ///
    /// **改错怎么红**：删任一（全量重解码 / 换流）⇒ 计数 1 ⇒ 红；分支内再多建流 ⇒ 3 ⇒ 红。
    #[test]
    fn guard291_g3_endpoint_creates_new_stream_once() {
        let (lines, (lo, hi), _) = endpoint_guard_regions();
        let n = count_contains(&lines, lo, hi, "recognizer.create_stream()");
        assert_eq!(
            n, 2,
            "G3: endpoint 分支内应有 2 处 create_stream（307 全量重解码 + 291 句末换流），实测 {n}"
        );
    }

    /// G4（I4）：`state.on_result(` 函数体内恰 3 处 ——
    /// ① endpoint 确认（true）② 非 endpoint 中间结果（false）③ loop 后 flush 收尾（false）；
    /// 且带 `true` 的确认恰 1 处（防 FIX-252 重复确认）。
    ///
    /// **改错怎么红**：endpoint 分支再补一次确认 ⇒ 带 true 的计数变 2 ⇒ 红；
    /// 少一路（如删收尾）⇒ 总数变 2 ⇒ 红。
    #[test]
    fn guard291_g4_on_result_sites() {
        let (lines, _, (fn_lo, fn_hi)) = endpoint_guard_regions();
        let total = count_starts(&lines, fn_lo, fn_hi, "state.on_result(");
        assert_eq!(
            total, 3,
            "G4: state.on_result( 应恰 3 处（endpoint true / 非 endpoint false / 收尾 false），实测 {total}"
        );
        let confirms = (fn_lo..=fn_hi)
            .filter(|&i| lines[i].starts_with("state.on_result(") && lines[i].contains(", true,"))
            .count();
        assert_eq!(
            confirms, 1,
            "G4: endpoint 确认（on_result ..., true, ...）必须恰 1 处（防重复确认），实测 {confirms}"
        );
    }

    // ========================================================================
    // PARALLEL-ACC-298 · 派发触发条件纯函数
    //   真实 recognizer 起不来 ⇒ 把触发判据抽纯函数钉死（判据 #3）。
    // ========================================================================

    /// 判据 #3-a：800ms + ≥3s + 本轮未派 三条件缺一不可。
    #[test]
    fn acc_should_dispatch_requires_all_three_conditions() {
        let min3s = 3000u64;
        let three_s = 3 * SAMPLE_RATE as usize;
        // 全满足 ⇒ true
        assert!(should_dispatch_acc(
            true, 800.0, three_s, false, 800.0, min3s
        ));
        // 静默差 1ms ⇒ false
        assert!(!should_dispatch_acc(
            true, 799.0, three_s, false, 800.0, min3s
        ));
        // 区间差 1 样本 ⇒ false
        assert!(!should_dispatch_acc(
            true,
            800.0,
            three_s - 1,
            false,
            800.0,
            min3s
        ));
        // 本轮已派（latch）⇒ false
        assert!(!should_dispatch_acc(
            true,
            5000.0,
            three_s * 10,
            true,
            800.0,
            min3s
        ));
        // 总开关关 ⇒ false
        assert!(!should_dispatch_acc(
            false,
            5000.0,
            three_s * 10,
            false,
            800.0,
            min3s
        ));
    }

    /// 判据 #3-a2：两个阈值 env 可调（传进去即生效）。
    #[test]
    fn acc_should_dispatch_honors_env_thresholds() {
        // silence 调到 400：400ms 即派
        assert!(should_dispatch_acc(
            true,
            400.0,
            3 * SAMPLE_RATE as usize,
            false,
            400.0,
            3000
        ));
        // min_seg 调到 6000：3s 不够，6s 才够
        assert!(!should_dispatch_acc(
            true,
            800.0,
            3 * SAMPLE_RATE as usize,
            false,
            800.0,
            6000
        ));
        assert!(should_dispatch_acc(
            true,
            800.0,
            6 * SAMPLE_RATE as usize,
            false,
            800.0,
            6000
        ));
    }

    /// 判据 #3-b：尾片判据 —— 🔴 有语音必须派（哪怕只剩 1 样本，绝不吞尾字）；
    /// 纯静音尾片不派（避免 all_native 翻 false）；空区间/开关关不派。
    #[test]
    fn acc_should_dispatch_tail_requires_speech() {
        assert!(should_dispatch_tail(true, 1, true), "有语音的尾巴绝不能吞");
        assert!(!should_dispatch_tail(true, 5 * SAMPLE_RATE as usize, false));
        assert!(!should_dispatch_tail(true, 0, true));
        assert!(!should_dispatch_tail(false, 5 * SAMPLE_RATE as usize, true));
    }

    // ============================================================
    // TUNE-STREAM-317 / Gavin 2026-09-21：env 覆盖已全部删除，线程数按机器核数取。
    // ============================================================

    /// 线程数须**随机器核数变化**且封顶 8，不得写死。
    #[test]
    fn stream_num_threads_follows_machine_cores_and_caps_at_8() {
        let n = local_stream_num_threads();
        assert!(n >= 1, "至少 1；取不到核数时回落 4");
        assert!(
            n <= 8,
            "封顶 8（271 实测：16 线程比 8 慢一倍，甚至慢于单线程）"
        );
        let expected = std::thread::available_parallelism()
            .map(|c| c.get().min(8) as i32)
            .unwrap_or(4);
        assert_eq!(
            n, expected,
            "口径须与 accuracy 侧 default_acc_num_threads 完全一致"
        );
    }
}
