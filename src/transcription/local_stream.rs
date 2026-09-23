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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use sherpa_onnx::{OnlineParaformerModelConfig, OnlineRecognizer, OnlineRecognizerConfig};

use super::qwen_inference::{StreamingAsrState, WordTiming};
use super::vad::{VadSegmenter, LOCALRT_VAD_MIN_SILENCE_SECS};
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

/// PUNCT-PREVIEW-SEMANTIC-349：预览打点的静默阈值（Gavin 2026-09-22）。
///
/// 沿革：269-B 静默 800ms 打点 + 269 的 4s 定时 + 284 shadow 400ms 强制 —— **三个触发都与语义无关**，
/// 会在用户还没说完时对「半句」打点；CT-Transformer 对未完成文本必在**末尾补终止符**，
/// 该终止符经 `punct_cache_reuse` 复用后落到句中（因果取证见
/// `collab/evidence/20260922-punct349/causal-evidence.md`）⇒ Gavin 报「标点打在句子中间，掐断句意」。
///
/// ⇒ 349 改成**只认静默 ≥ 1200ms**（与 346 同一口径）：4s 定时与 shadow 400ms 强制**一并删除**。
/// 用户停顿 1.2s = 语义边界，此时打点落在真正的停顿处；连续不停顿期间预览保持裸文本（无标点）。
///
/// 🔴 **与 sherpa endpoint 完全独立**：本阈值只影响**显示层**刷新（判早了只是标点早出现），
/// 绝不 `reset()` / 切句 / `sentence_id += 1`。切句仍由 `LOCAL_STREAM_RULE2=2.0` 的
/// sherpa endpoint 决定（说话中间换气不会被切句）。两者是不同层的东西，不要混用。
const PUNCT_SILENCE_TRIGGER_MS: f32 = 1200.0;

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

/// LOCALRT-PARALLEL-ACC-298 / ACC-DISPATCH-SILENCE-ONLY-346：派发静音阈值默认值。
///
/// 沿革：298 初版 800ms（Gavin 拍板，不是 400ms）→ **ACC-DISPATCH-SILENCE-ONLY-346 Gavin
/// 2026-09-22 改为 1200ms，且「只判断静默，不按时长来切片」**。原「或 未派发累计 ≥5s（长度支）」
/// 已按 DEC-077（零收益机制即移除）连根删除，常量 `ACC_MIN_SEGMENT_MS_DEFAULT` 一并移除。
///
/// 🔴 **与 sherpa endpoint（rule2=2.0s）完全解耦**：本阈值只管「把已说完的一段派给 accuracy
/// 并行转写」，**绝不触发** `reset()` / 切句 / `sentence_id += 1`（那是显示层的事）。
///
/// 🔴 **本阈值必须读 acc 专用计数器 `acc_silent_ms`，不能读显示层的 `silent_ms`**：
/// 显示层标点打点后会把 `silent_ms` 清零（`if silence_due { silent_ms = 0.0; }`）。
/// 346 落地时标点阈值 `PUNCT_SILENCE_TRIGGER_MS=800` < 本阈值 1200 ⇒ 若读共享 `silent_ms`，
/// 静默每到 800ms 就被标点路径归零，**1200ms 永远到不了**，叠加删长度支后 accuracy 在录音中
/// **一次都不会被派发**（298/342-D 整个废掉）。
///
/// PUNCT-PREVIEW-SEMANTIC-349 起标点阈值也抬到 **1200ms**，与本阈值相等：此时读共享计数器
/// 「恰好」也能工作（acc 检查在标点清零之前，同一帧先派后清），但**依赖两条阈值恒相等 +
/// 循环内检查顺序**，是脆弱耦合。独立计数器让两个消费者互不影响（与 shadow/acc 各自 latch
/// 同构），故**保留** —— 阈值今后各自调整都不会互相踩踏。
const ACC_DISPATCH_SILENCE_MS_DEFAULT: f32 = 1200.0;

/// LOCALRT-SEAM-337：自适应定界的「文本停止增长」窗口（具名，非魔数）。
/// 依据：流式模型按 `config.yaml: chunk_length: 500`（ms）+ `chunk_shift_ratio: 0.5`（步进 250ms）
/// 分块处理 ⇒ **一个整块内无任何新输出**即可判「滞后补字已吐完」。
const ACC_BOUNDARY_STABLE_MS: f32 = 500.0;

/// LOCALRT-SEAM-337：定界硬上限（兜底，防无限等）。
/// 依据：`seam337_lookahead_probe` 25 点实测最大 `last_grow_after_P = 1840ms` + 余量。
const ACC_BOUNDARY_CAP_MS: f32 = 2000.0;

/// LOCALRT-PARALLEL-ACC-298 / ACC-DISPATCH-SILENCE-ONLY-346：accuracy 并行派发配置。
///
/// `enabled` 总开关（关 = 逐位退回串行行为）、`silence_ms` 派发静音阈值。
/// ACC-DISPATCH-SILENCE-ONLY-346：原长度支（最小片长 `min_seg_ms`）已按 DEC-077 连根删除，
/// 不再有对应字段 —— 派发**只判静默**。
#[derive(Debug, Clone, Copy)]
pub struct AccDispatchConfig {
    pub enabled: bool,
    pub silence_ms: f32,
}

impl AccDispatchConfig {
    /// 🔴 常量构造，**无 env 覆盖**（Gavin 2026-09-21 定：不允许开发端与用户端行为不一致）。
    /// 用户机器上不存在任何环境变量；env 覆盖等于给开发机开一条用户永远走不到的路径，
    /// 且测试时 env 残留会让结论失真。要改参数就改常量重新构建 —— 构建出来的就是用户跑的那一份。
    pub fn new() -> Self {
        let enabled = true;
        let silence_ms = ACC_DISPATCH_SILENCE_MS_DEFAULT;
        log::debug!(
            "[LocalRT-DBG-298] acc parallel cfg: enabled={} silence_ms={}",
            enabled,
            silence_ms
        );
        Self {
            enabled,
            silence_ms,
        }
    }
}

impl Default for AccDispatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            silence_ms: ACC_DISPATCH_SILENCE_MS_DEFAULT,
        }
    }
}

/// PARALLEL-ACC-298 / ACC-DISPATCH-SILENCE-ONLY-346：是否把「当前未派发区间」派发给 accuracy 并行 worker。
///
/// 🔴 设计口径（Gavin 2026-09-22 原话）：**「改成只判断静默1200ms，不按时长来切片」**。
/// 原 342-D 的 OR 口径（静默 ≥800ms **或** 未派发累计 ≥5s）已废：长度支按 DEC-077
/// （零收益机制即移除）连根删除，`min_seg_ms` 参数与对应分支一并消失。
///
/// 判据：`silent_ms >= silence_ms` 即派，受 `done_for_pause` 约束（同一停顿只派一次）。
///
/// 🔴 调用方传入的 `silent_ms` 必须是 **acc 专用计数器 `acc_silent_ms`**，不是显示层那个会被
/// 标点路径在 800ms 清零的 `silent_ms`（理由见 `ACC_DISPATCH_SILENCE_MS_DEFAULT` 常量文档）。
/// 参数名保留 `silent_ms` —— 纯函数这一层它只是「一个静默毫秒值」，语义由调用方保证。
///
/// 🔴 `has_speech` 护栏（保留）：**纯静音区间不派** —— 送 accuracy 会返回空 ⇒ 上层
/// `all_native` 翻 false ⇒ 本地档多跑一遍标点引擎（路由漂移）。
fn should_dispatch_acc(
    enabled: bool,
    silent_ms: f32,
    pending_samples: usize,
    done_for_pause: bool,
    has_speech: bool,
    silence_ms: f32,
) -> bool {
    if !enabled || !has_speech || pending_samples == 0 {
        return false;
    }
    silent_ms >= silence_ms && !done_for_pause
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
    /// 🔴 PUNCT-349：同时充当「是否打过点」的判据 —— `raw_full.len() > raw_len` 即「有新内容」。
    raw_len: usize,
}

/// PUNCT-PREVIEW-SEMANTIC-349：预览打点是否触发 —— **只认静默 ≥ 阈值 + 有新内容**。
///
/// 349 删除了原先另外两条触发（4s 定时、284 shadow 400ms 强制）：两者都与语义无关，
/// 会在用户没说完时对「半句」打点 ⇒ CT-Transformer 在半句末尾补终止符、经缓存复用落到句中
/// （因果取证见 `collab/evidence/20260922-punct349/causal-evidence.md`）。
/// 现口径与 346 一致：只按语义停顿（静默 ≥1200ms）打点。
fn should_repunctuate_preview(silent_ms: f32, has_new: bool, threshold_ms: f32) -> bool {
    silent_ms >= threshold_ms && has_new
}

/// LOCALRT-PUNCT-TIMER-269：把**原始无标点全文**转成 overlay 预览文本。
///
/// - `force`（由 `should_repunctuate_preview` 给出，即静默 ≥1200ms）：对 `raw_full`
///   **整体重打**一次标点并更新缓存。全量重打能随上下文修正先前标点，比逐句打更准（Gavin 指定）。
/// - 否则：`上次标点文本 + raw_full 新增后缀` —— 新字即时上屏，已出现的标点不闪回。
/// - `engine` 为 `None`（用户关标点）：直接返回 `raw_full`，行为与未启用标点一致，不引入额外开销。
///
/// 🔴 关键：输入始终是**原始裸文本**，绝不把已打点文本再次送引擎 ⇒ 不会「标点重复 / 错乱」
/// （FIX-252 场景不复发）。
///
/// 🔴 PUNCT-349：**不再有 4s 周期重打**（原 `PUNCT_REFRESH_INTERVAL` 已删）。打点只在
/// 「静默 ≥1200ms 的语义停顿」触发；触发前持续静默不会反复重打同一段（`has_new` 护栏）。
fn preview_display(
    raw_full: &str,
    engine: Option<&mut PunctuationEngine>,
    cache: &mut PunctPreviewCache,
    force: bool,
) -> String {
    let Some(engine) = engine else {
        return raw_full.to_string();
    };
    // 有新内容才打：raw 单调增长（greedy 0 回退），长度增长即新内容。
    let has_new = raw_full.len() > cache.raw_len;
    if force && has_new {
        let t = Instant::now();
        cache.prefix = engine
            .add_punctuation(raw_full)
            .unwrap_or_else(|| raw_full.to_string());
        cache.raw_len = raw_full.len();
        log::debug!(
            "LOCALRT-PUNCT-TIMER-269: repunctuated {} chars in {:.1}ms",
            raw_full.chars().count(),
            t.elapsed().as_secs_f64() * 1000.0
        );
    }
    // 🔴 LOCALRT-CHARBOUNDARY-344（P0 崩溃修复）：`cache.raw_len` 是**字节**长度，
    //    而 `raw_full` 在 `main_view` 与 `shadow_view` 之间**按更长者切换**（见调用处），
    //    上一帧从 A 串缓存的字节位置，落到本帧 B 串里可能**正好切在中文字符中间**
    //    ⇒ `&raw_full[cache.raw_len..]` 直接 panic。
    //    实测崩溃：`byte index 235 is not a char boundary; inside 斯`
    //    （`core::str::slice_error_fail`，BUILD-341 端测 crash.json）。
    //    原注释「raw 单调增长（greedy 0 回退）」只对**同一个串**成立，切串就不成立。
    // ⇒ 复用 `punct_cache_reuse`（内含 `is_char_boundary` O(1) 守卫）；不是字符边界就整串原样返回。
    punct_cache_reuse(&cache.prefix, raw_full, cache.raw_len)
}

/// LOCALRT-CHARBOUNDARY-344：标点缓存复用的**边界安全**实现（纯函数，便于回归采样）。
///
/// `cache.raw_len` 是字节长度。仅当：前缀非空、`raw` 未变短、且 `raw_len` **恰为 `raw` 的
/// 合法 char 边界**时，才拼接「前缀 + `raw[raw_len..]`」；否则**整串原样返回**。
///
/// 🔴 `is_char_boundary` 是 O(1)（只查 UTF-8 起始字节），不违反 Gavin「主路径不加拖累」约束。
/// 退化时本帧少一次标点缓存复用（下次重打即恢复）；宁可这一帧不带标点，也**绝不能崩**。
fn punct_cache_reuse(prefix: &str, raw: &str, raw_len: usize) -> String {
    if !prefix.is_empty() && raw.len() >= raw_len && raw.is_char_boundary(raw_len) {
        let mut display = String::with_capacity(prefix.len() + raw.len() - raw_len);
        display.push_str(prefix);
        display.push_str(&raw[raw_len..]);
        display
    } else {
        raw.to_string()
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

/// FIX-LOCALRT-TAILCHAR-291：切句 / 收尾确认文本的**两方取最长**（不回退）。
///
/// 291 的「flush 前/后取长者」语义扩为两方（**不新造一套**）：
/// - `main`：主 stream flush 后结果（调用方已做「after 空/更短则回落 before」再传进来）
/// - `shadow`：400ms 影子快照（可空），仅作补充候选
///
/// 🔴 LOCALRT-ROLLBACK-344：原第三方 `full`（307 整句全量重解码）已移除 —— 每次断句 ~104.5ms
/// 且 26/26 `gained=0`，主路径零收益纯开销。择优语义（更长者胜）不变。
///
/// 返回字符数**严格更多**者；等长不切换（优先级 main → shadow）⇒ 结果长度恒 ≥ `main`，
/// **绝不回退**且同长输入不抖动。全空返回空串（调用方据此走 EMPTY 分支）。
fn endpoint_confirm_text<'a>(main: &'a str, shadow: Option<&'a str>) -> &'a str {
    let mut best = main;
    let best_len = main.chars().count();
    if let Some(sh) = shadow {
        if sh.chars().count() > best_len {
            best = sh;
        }
    }
    best
}

/// LOCALRT-REFLOW-HOLE-344-G：取「第 i 片对应的流式文本」——
/// `display` 去掉前 `prev_committed` 个**字符**后的后缀。
///
/// 🔴 **必须按 char 计数切片，绝不用字节下标**（`&display[prev_committed..]` 会在中文中途 panic，
/// 见 LOCALRT-CHARBOUNDARY-344）。`chars().skip(n).collect()` 天然字符安全。
fn segment_streaming_text(display: &str, prev_committed: usize) -> String {
    display.chars().skip(prev_committed).collect()
}

/// LOCALRT-ENDPOINT-EMPTY-342：endpoint 分支的确认决策（纯函数，便于钉死 §判据）。
///
/// 触发场景：主 stream 换流后只喂了静音，2s 后 rule2 又给出一个 endpoint —— 这是
/// **静音流上的假 endpoint**，不是真正的句子边界。判据「本句自上次 endpoint 是否有声」
/// 复用现有能量口径（`chunk_rms > silence_threshold`），此处只做纯逻辑映射。
#[derive(Debug, PartialEq, Eq)]
enum EndpointAction {
    /// 真 endpoint + 有确认文本：正常确认本句。
    Confirm,
    /// 真 endpoint 但三方全空：只记日志，不确认（原行为）。
    Empty,
    /// 假 endpoint（静音段）：不确认、不推进 `sentence_id` / `sentence_pcm_start`、
    /// 静音流文本一律丢弃（幻字抑制，F3）。
    SuppressSilence,
}

fn endpoint_action(segment_has_speech: bool, confirm_text_empty: bool) -> EndpointAction {
    if !segment_has_speech {
        EndpointAction::SuppressSilence
    } else if confirm_text_empty {
        EndpointAction::Empty
    } else {
        EndpointAction::Confirm
    }
}

/// FIX-SLICE-CUT-AT-GAP-381：滑窗（accuracy 派发）路径的片构建 —— 超限片按**字缝**切。
///
/// 原实现走 `build_padded_segments`（>20s 按 20s 硬切，native/FunASR 时代遗留）；
/// 370 改为固定 13s 硬切 —— 但**硬切点可能正好落在字上，把字切碎导致识别出错**
/// （Gavin 2026-09-23）。本单改为：单片剩余 > 10s 时，从「本片起点 + 10s」往后
/// 找第一个**字缝**（能量低谷）切，最晚 12s 兜底（[`super::vad::plan_gap_cuts`]）。
///
/// 🔴 口径统一（Gavin 2026-09-23）：**窗口上限与切片起搜点都改为 10s**
/// （`WINDOW_MAX_SECS` 12→10、删除 `SLIDING_SLICE_MAX_SECS` 13s）。
///
/// 只保留 `build_padded_segments` 的 200ms 边界 padding 与 `FIX-VAD-STATE-RESET-001`
/// 边界过滤/clamp（start 越界丢弃并 warn、end 超界 clamp）。
fn build_dispatch_segment(
    start: usize,
    len: usize,
    total_samples: usize,
    pcm: &[f32],
) -> Vec<Vec<f32>> {
    super::build_sliding_segments(&[(start, len)], total_samples, pcm)
}

// ===========================================================================
// LOCALRT-VAD-SILENCE-384：本地 realtime 静默判定的「是否有人声」唯一判定 + 进程级 VAD 缓存
//   （Gavin 2026-09-23：环境背景有声音时音量阈值判静默失效 ⇒ 上 silero VAD，原音量阈值作兜底。
//    只服务本地 realtime 管线，不碰其他管线。）
// ===========================================================================

/// 本地 realtime VAD 的**进程级缓存**（Gavin：模型只加载一次、跨录音复用）。
///
/// - 按模型目录缓存：目录不变则复用同一实例；每次录音开始只 `reset_for_new_session()` 清状态。
/// - 首次加载失败**记住**（`vad=None`），不每次录音重试。
static LOCALRT_VAD_CACHE: OnceLock<Mutex<LocalRtVadCache>> = OnceLock::new();

struct LocalRtVadCache {
    /// 已尝试加载的模型目录（`None` = 从未尝试）。
    dir: Option<PathBuf>,
    /// `Some` = 可用；`None` = 该目录加载失败（记住，不重试）。
    vad: Option<VadSegmenter>,
}

/// 取本次录音的本地 realtime VAD（返回持有进程级缓存的 guard；`vad=None` ⇒ 用音量兜底）。
///
/// 首次调用加载模型（约几十 ms，在 ASR 消费线程内，**不阻塞录音线程**）；之后复用。
fn localrt_vad_session(model_dir: &Path) -> MutexGuard<'static, LocalRtVadCache> {
    let cell = LOCALRT_VAD_CACHE.get_or_init(|| {
        Mutex::new(LocalRtVadCache {
            dir: None,
            vad: None,
        })
    });
    let mut guard = cell.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.dir.as_deref() != Some(model_dir) {
        guard.vad = VadSegmenter::try_new_for_local_silence(model_dir);
        if guard.vad.is_none() {
            log::warn!(
                "[LocalRT-DBG-384] silero VAD unavailable at {:?}; fallback to energy threshold",
                model_dir
            );
        }
        guard.dir = Some(model_dir.to_path_buf());
    }
    if let Some(v) = guard.vad.as_ref() {
        v.reset_for_new_session();
    }
    guard
}

/// LOCALRT-VAD-SILENCE-384 / LOCALRT-NEARFIELD-GATE-389：本 chunk「是否有人声」的**唯一判定**。
///
/// VAD 可用 ⇒ `feed_is_speech`（silero）再过**整句近场段门**（389）；不可用 ⇒ 退回**音量阈值**
/// （口径与 384 逐位相同，**不加近场门**）。返回 `(最终 has_speech, VAD 原始 vad_speech)`。
/// 🔴 389：VAD 分支的门用段状态机 `SegmentGate`（比较/学习用 300ms 平滑音量）；兜底分支仍用原始 `chunk_rms`。
/// 🔴 显示层打点 / acc 派发 / 影子 / 端点 / 337 边界 b **全部**用同一个 `has_speech`，不许分叉。
fn chunk_has_speech(
    vad: Option<&VadSegmenter>,
    chunk: &[f32],
    chunk_rms: f32,
    smooth_rms: f32,
    now_ms: f32,
    silence_threshold: f32,
    gate: &mut SegmentGate,
) -> ChunkJudgment {
    match vad {
        Some(v) => {
            let vad_speech = v.feed_is_speech(chunk);
            let has_speech = gate.update(vad_speech, smooth_rms, now_ms);
            ChunkJudgment {
                has_speech,
                vad_speech,
            }
        }
        None => ChunkJudgment {
            has_speech: chunk_rms > silence_threshold,
            vad_speech: false,
        },
    }
}

/// LOCALRT-VAD-SILENCE-384：人声 → 无人声**转换 chunk** 的静默补偿毫秒数。
///
/// silero 在人声结束后须过 `LOCALRT_VAD_MIN_SILENCE_SECS` 才报「无人声」，若从该刻才计时 ⇒
/// 实际要停 1.5s 才到 1200ms。故在转换的那个 chunk 把 `silent_ms` / `acc_silent_ms` 直接
/// **补记**这么多，使「从真实停顿起算满 1200ms」与改前一致。
fn localrt_vad_seed_ms() -> f32 {
    LOCALRT_VAD_MIN_SILENCE_SECS * 1000.0
}

// ===========================================================================
// LOCALRT-NEARFIELD-GATE-389：近场门改「按整句（VAD 段）判定」
//   根因（BUILD-387）：逐 10ms / 300ms 平滑仍比「录音人音量 × 0.25」，一句话内轻读字 / 句尾 /
//   清辅音比响亮字低 15~20dB ⇒ 句内被切成有声/静音多段（仍挡 ~40%）。背景人声与录音人的真正
//   区别是**整句响度**：录音人每句必有响亮的字，远处人声整句都弱。
//   ⇒ 段内峰值越过「level × 0.3」即整句确认为录音人，其后轻音/句尾全算有声；只学已确认段峰值。
//   已知局限：背景人若与录音人一样近、一样大，分不开。
// ===========================================================================

/// 近场段判：录音人整句峰值估计的滑动窗口（秒）。
const NEARFIELD_HISTORY_SECS: f32 = 30.0;
/// 近场段判：通过阈值 = 录音人整句峰值估计 × 本比例（≈ −10.5dB）。
const NEARFIELD_PEAK_RATIO: f32 = 0.3;

/// LOCALRT-NEARFIELD-GATE-389（C2）：跨录音沿用录音人音量的**最长时效**。
const CARRY_MAX_AGE: Duration = Duration::from_secs(600); // 10 分钟
/// LOCALRT-NEARFIELD-GATE-389（C2）：开头连续这么多**段被拒**即丢弃沿用 seed（防误把别人当录音人）。
const CARRY_DROP_AFTER_REJECTS: u32 = 2;

/// LOCALRT-NEARFIELD-GATE-389（C2）：跨录音沿用的录音人音量（**进程内存，不写配置文件**）。
struct CarryLevel {
    /// 录音设备 key（`config.audio.input_device` 原样；空串 = 系统默认）。
    device: String,
    /// 上次录音结束时的录音人整句峰值估计（中位数）。
    level: f32,
    /// 写回时刻（判断 10 分钟时效）。
    updated: Instant,
}

/// 进程内单例：本地 realtime 跨录音沿用录音人音量（同设备、10 分钟内有效）。
static LOCALRT_CARRY_LEVEL: Mutex<Option<CarryLevel>> = Mutex::new(None);

/// LOCALRT-NEARFIELD-GATE-389（C2）：沿用判据（纯函数，可单测）——**同设备**且**未超时效**。
fn seed_usable(carry_device: &str, age: Duration, device: &str) -> bool {
    carry_device == device && age <= CARRY_MAX_AGE
}

/// LOCALRT-NEARFIELD-GATE-387（E）/ 389：段峰值所用的**平滑音量窗口**（毫秒）。
///
/// 根因：chunk=10ms，逐 10ms 原始 RMS 在一句话内随辅音/字缝剧烈波动 ⇒ 段峰值与门都用 300ms 平滑值。
/// 取 300ms：覆盖约 1~2 个音节周期，既抹平字缝又不至于跨句滞后。
const NEARFIELD_SMOOTH_MS: f32 = 300.0;

/// LOCALRT-NEARFIELD-GATE-387（E）：滑动窗口 RMS（O(1) 增量）。
///
/// 维护最近 `NEARFIELD_SMOOTH_MS` 内 chunk 的能量和与样本数：`rms = sqrt(Σx² / Σn)`。
/// 只做增量加减，无排序/无重扫；窗口未满时即用当前已入部分（前 300ms 渐进预热）。
struct EnergySmoother {
    win: std::collections::VecDeque<(f64, u64, f32)>, // (能量和, 样本数, ms)
    sum_sq: f64,
    n: u64,
    ms: f32,
}

impl EnergySmoother {
    fn new() -> Self {
        Self {
            win: std::collections::VecDeque::new(),
            sum_sq: 0.0,
            n: 0,
            ms: 0.0,
        }
    }

    /// 推入一个 chunk 的能量和/样本数/时长，返回**更新后**的平滑 RMS。
    fn push(&mut self, sum_sq: f64, n: u64, chunk_ms: f32) -> f32 {
        self.win.push_back((sum_sq, n, chunk_ms));
        self.sum_sq += sum_sq;
        self.n += n;
        self.ms += chunk_ms;
        while self.ms > NEARFIELD_SMOOTH_MS {
            match self.win.pop_front() {
                Some((s, c, m)) => {
                    self.sum_sq -= s;
                    self.n = self.n.saturating_sub(c);
                    self.ms -= m;
                }
                None => break,
            }
        }
        if self.n == 0 {
            0.0
        } else {
            // 主控验收补：长时间加减的浮点累计误差可使 sum_sq 落为极小负数 ⇒ sqrt 得 NaN ⇒ 门比较失效。钳到 ≥0。
            (self.sum_sq.max(0.0) / self.n as f64).sqrt() as f32
        }
    }
}

/// LOCALRT-NEARFIELD-GATE-389：录音人**整句峰值**估计（本次录音为主；可选跨录音 seed，见 C2）。
///
/// 存 `(段峰值, 入样会话 ms)`；按**会话音频时间** 30s 过期（沿用 385 防锁死）；`estimate()` = **中位数**
/// （本次样本就绪后）或 seed（就绪前）；`ready()` = 本次样本 ≥ 2 **或**有 seed。
/// 只收录**已确认段**（先判门后更新）⇒ 背景人声进不了估计。
/// C2：seed 沿用期若**开头连续 `CARRY_DROP_AFTER_REJECTS` 段被拒** ⇒ 丢弃 seed（回到未就绪、整句直接确认）。
struct SegmentPeakLevel {
    samples: std::collections::VecDeque<(f32, f32)>, // (段峰值, 入样会话 ms)
    /// C2：跨录音沿用的录音人音量（本次样本就绪后不再参与估计）。
    seed: Option<f32>,
    /// C2：seed 沿用期的连续被拒段计数。
    consecutive_rejects: u32,
}

impl SegmentPeakLevel {
    /// C2：带跨录音 seed 新建（`Some` ⇒ 立即就绪、`estimate`=seed，直到本次样本就绪）。
    fn with_seed(seed: Option<f32>) -> Self {
        Self {
            samples: std::collections::VecDeque::new(),
            seed,
            consecutive_rejects: 0,
        }
    }

    /// 收录一个**已确认段**的峰值：`now_ms` = 入样会话毫秒（会话音频时长累加）。
    fn push(&mut self, peak: f32, now_ms: f32) {
        self.samples.push_back((peak, now_ms));
    }

    /// 按会话音频时间剔除过期样本（`now_ms − t > 30s`）。**只作用于本次样本，不动 seed**。
    fn prune(&mut self, now_ms: f32) {
        let limit = NEARFIELD_HISTORY_SECS * 1000.0;
        while let Some(&(_, t_ms)) = self.samples.front() {
            if now_ms - t_ms > limit {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }

    /// 本次录音样本是否就绪（≥ 2）。
    fn samples_ready(&self) -> bool {
        self.samples.len() >= 2
    }

    /// C2：seed 是否仍在参与估计（有 seed 且本次样本未就绪）。
    fn seed_in_use(&self) -> bool {
        self.seed.is_some() && !self.samples_ready()
    }

    /// 就绪：本次样本 ≥ 2 或 有跨录音 seed（就绪前整句直接确认，保开头不丢字）。
    fn ready(&self) -> bool {
        self.samples_ready() || self.seed.is_some()
    }

    /// C2：段结束通知 —— 维护 seed 沿用期的连续拒段；达阈值则丢弃 seed。返回是否**刚丢弃**。
    fn end_segment(&mut self, confirmed: bool) -> bool {
        if !self.seed_in_use() {
            return false;
        }
        if confirmed {
            self.consecutive_rejects = 0;
            return false;
        }
        self.consecutive_rejects += 1;
        if self.consecutive_rejects >= CARRY_DROP_AFTER_REJECTS {
            self.seed = None;
            self.consecutive_rejects = 0;
            return true;
        }
        false
    }

    /// 录音人整句峰值估计：本次样本就绪 ⇒ **中位数**（seed 不再参与）；否则 seed；否则 0。
    /// 须先 `prune(now_ms)`。
    fn estimate(&self) -> f32 {
        if self.samples_ready() {
            let mut v: Vec<f32> = self.samples.iter().map(|(p, _)| *p).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v[v.len() / 2]
        } else {
            self.seed.unwrap_or(0.0)
        }
    }
}

/// 单个 chunk 的判定结果：`has_speech`（最终，含近场段门）/ `vad_speech`（VAD 原始判定）。
struct ChunkJudgment {
    has_speech: bool,
    vad_speech: bool,
}

/// LOCALRT-NEARFIELD-GATE-389：**按整句（VAD 段）的段门状态机**（纯逻辑，可单测）。
///
/// 规则（见节首）：
/// - 段起始（vad 由假变真的 chunk）：`seg_confirmed = !level.ready()`（未就绪 ⇒ 整句直接确认）。
/// - 段内：更新 `seg_peak`；一旦平滑音量 ≥ `level.estimate() × NEARFIELD_PEAK_RATIO` ⇒ 整句确认。
/// - `has_speech = vad && seg_confirmed`（确认后本句**剩余部分**全算有声，直到 VAD 段结束）。
/// - 段结束（vad 由真变假）：**已确认段**才 `level.push(seg_peak)`（防背景自我放行）；计数 + 清状态。
struct SegmentGate {
    in_seg: bool,
    seg_confirmed: bool,
    seg_peak: f32,
    level: SegmentPeakLevel,
    /// 埋点：段总数 / 已确认 / 被拒（背景）。
    segments: u64,
    confirmed: u64,
    rejected: u64,
    /// 389 主控验收补：本次录音中 seed 因连续被拒而丢弃过 ⇒ 录音结束时清掉进程级 carry，
    /// 否则下次录音还会拿同一个坏 seed、再被拒两句。
    seed_dropped: bool,
}

impl SegmentGate {
    /// C2：带跨录音 seed 建门（`Some` ⇒ 立即就绪、estimate=seed）。
    fn with_seed(seed: Option<f32>) -> Self {
        Self {
            in_seg: false,
            seg_confirmed: false,
            seg_peak: 0.0,
            level: SegmentPeakLevel::with_seed(seed),
            segments: 0,
            confirmed: 0,
            rejected: 0,
            seed_dropped: false,
        }
    }

    /// 推进一帧，返回本 chunk 最终 `has_speech`。`smooth_rms` = 300ms 平滑音量。
    fn update(&mut self, vad_speech: bool, smooth_rms: f32, now_ms: f32) -> bool {
        self.level.prune(now_ms);
        if vad_speech && !self.in_seg {
            // 段起始：未就绪 ⇒ 整句直接确认（保开头不丢字）。
            self.in_seg = true;
            self.seg_peak = 0.0;
            self.seg_confirmed = !self.level.ready();
        }
        if vad_speech {
            if smooth_rms > self.seg_peak {
                self.seg_peak = smooth_rms;
            }
            if !self.seg_confirmed && smooth_rms >= self.level.estimate() * NEARFIELD_PEAK_RATIO {
                self.seg_confirmed = true;
            }
        }
        let has_speech = vad_speech && self.seg_confirmed;
        if !vad_speech && self.in_seg {
            // 段结束：只学**已确认段**峰值（背景段被拒 ⇒ 不污染 level）。
            self.segments += 1;
            if self.seg_confirmed {
                self.confirmed += 1;
                self.level.push(self.seg_peak, now_ms);
            } else {
                self.rejected += 1;
            }
            // C2：seed 沿用期连续被拒 ⇒ 达阈值丢弃 seed（回到未就绪、整句直接确认）。
            let dropped = self.level.end_segment(self.seg_confirmed);
            if dropped {
                self.seed_dropped = true;
            }
            if dropped && log::log_enabled!(log::Level::Debug) {
                log::debug!(
                    "[LocalRT-DBG-389] carry level dropped after {} rejects (seed={:?})",
                    CARRY_DROP_AFTER_REJECTS,
                    self.level.seed
                );
            }
            if log::log_enabled!(log::Level::Debug) {
                log::debug!(
                    "[LocalRT-DBG-389] segment end: peak={:.5} level={:.5} confirmed={}",
                    self.seg_peak,
                    self.level.estimate(),
                    self.seg_confirmed
                );
            }
            self.in_seg = false;
            self.seg_confirmed = false;
            self.seg_peak = 0.0;
        }
        has_speech
    }
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
///   同 `config.audio.silence_threshold`）。用于**独立**统计连续静默（≥ `PUNCT_SILENCE_TRIGGER_MS`，
///   349 起 1200ms）触发显示层打点；
///   🔴 与 sherpa endpoint 无关，不触发 reset/切句。
/// - `vad_device`：LOCALRT-NEARFIELD-GATE-389（C2）录音设备 key（`config.audio.input_device` 原样；
///   空串 = 系统默认）—— 供**跨录音沿用录音人音量**（同设备、10 分钟内）。
/// - `on_result`：文本变化回调，传 `(display_text, display_words)`，与 qwen 路径同构
/// - `acc_cfg`：LOCALRT-PARALLEL-ACC-298 派发配置（`enabled`/`silence_ms`；346 起无长度支）；
///   `enabled=false` 时本函数**完全不派发**（逐位退回串行行为）
/// - `on_segment`：accuracy 并行派发回调，传
///   `(seg_index, committed_len, 已加 padding 的 16k f32 子段列表, seg_streaming_text)`。
///   `committed_len` = **派发当刻浮层已显示文本的字符数**（325 回灌边界；取 `last_display`，
///   与消费侧 `last_streaming_text` 镜像同源）。
///   `seg_streaming_text` = 本片对应的**流式文本**（`last_display` 自上一片 `committed_len`
///   起的字符后缀，**按 char 切片**）—— LOCALRT-REFLOW-HOLE-344-G：worker 解出空/失败时用它
///   填补，避免累积 `acc_text` 留洞导致后续回灌**永久失效**。
///   子段列表通常 1 个；`FIX-SLICE-CUT-AT-GAP-381` 起单片剩余 >10s 即按**字缝**切
///   （从 10s 起找能量低谷、最晚 12s 兜底，`vad::plan_gap_cuts`），切点严格相接、不丢内容。
///   🔴 只在 `acc_cfg.enabled` 且静默 ≥ `acc_cfg.silence_ms`(1200ms) 且未上 latch 时被调用
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
    vad_device: &str,
    mut on_result: impl FnMut(&str, &[WordTiming]),
    acc_cfg: AccDispatchConfig,
    mut on_segment: impl FnMut(usize, usize, Vec<Vec<f32>>, String),
    // LOCALRT-SEAM-337（自适应定界，主控定案）：派发点 P 后边界在**三者最先发生**时冻结：
    //   a. 文本停止增长（静默中连续 `ACC_BOUNDARY_STABLE_MS` 无新增）⇒ `Some(当前显示长度)`，允许回灌；
    //   b. 有声 chunk 恢复 ⇒ `None`，该 seg **不回灌**（保持纯流式；宁可这轮不修，也不吐残留重复）；
    //   c. 硬上限 `ACC_BOUNDARY_CAP_MS` ⇒ 同 a（兜底）并记日志。
    // 参数：`(seg_index, Option<committed_len>)`。
    mut on_reflow_commit: impl FnMut(usize, Option<usize>),
) -> Result<(String, Vec<f32>)> {
    let is_cancelled = || {
        cancel_signal
            .map(|s| s.load(Ordering::Relaxed))
            .unwrap_or(false)
    };

    // LOCALRT-VAD-SILENCE-384：本地 realtime VAD（进程级缓存，只加载一次、跨录音复用）。
    // guard 持有缓存到本函数结束；`session_vad=None` ⇒ 音量阈值兜底（逐位同改前）。
    let _vad_guard = localrt_vad_session(&super::model_dir());
    let session_vad: Option<&VadSegmenter> = _vad_guard.vad.as_ref();
    let vad_on = session_vad.is_some();
    if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "[LocalRT-DBG-384] silence detector={}",
            if vad_on { "vad" } else { "energy" }
        );
    }
    let mut prev_has_speech = false;
    // LOCALRT-NEARFIELD-GATE-389（C2）：跨录音沿用录音人音量（同设备、10 分钟内）。
    let (carry_seed, carry_age_s) = LOCALRT_CARRY_LEVEL
        .lock()
        .ok()
        .and_then(|c| {
            c.as_ref().map(|cl| {
                let age = cl.updated.elapsed();
                let usable = seed_usable(&cl.device, age, vad_device);
                (usable.then_some(cl.level), Some(age.as_secs_f32()))
            })
        })
        .unwrap_or((None, None));
    // LOCALRT-NEARFIELD-GATE-389：近场段门状态机（整句判定；段峰值估计在其内；C2 可选 seed）。
    let mut segment_gate = SegmentGate::with_seed(carry_seed);
    if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "[LocalRT-DBG-389] carry level: seed={:?} device={:?} age_s={:?}",
            carry_seed,
            vad_device,
            carry_age_s
        );
    }
    // LOCALRT-NEARFIELD-GATE-387（E）/ 389：段峰值用的**平滑音量**（最近 300ms 滑动窗口 RMS）。
    let mut energy_smoother = EnergySmoother::new();
    // 会话已处理音频毫秒数（chunk 时长累加，不用系统时钟）——段峰值窗口按此过期。
    let mut session_ms = 0.0f32;
    let mut vad_total_ms = 0.0f64;
    let mut vad_max_ms = 0.0f64;
    let mut vad_chunks = 0u64;

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
    };
    // LOCALRT-PUNCT-TIMER-269-B：连续静默累计（ms）。独立于 sherpa endpoint，只驱动显示层打点。
    // 🔴 ACC-DISPATCH-SILENCE-ONLY-346：本计数器会被**标点路径**在打点后清零（见函数内
    // `if silence_due { silent_ms = 0.0; }`），故 accuracy 派发**不得**复用它。
    let mut silent_ms: f32 = 0.0;
    // ACC-DISPATCH-SILENCE-ONLY-346：accuracy 派发专用静默累计（ms）。仅与 `silent_ms` 在
    // 「静音累加 / 语音归零」两处同步，**不被标点清零** —— 保证 1200ms 阈值可达。
    let mut acc_silent_ms: f32 = 0.0;

    // LOCALRT-ENDPOINT-284（方案 B）：影子收尾（只动显示层）。
    let shadow_trigger_ms: f32 = SHADOW_FINALIZE_MS_DEFAULT;
    log::debug!("[LocalRT-DBG-284] shadow_trigger_ms={}", shadow_trigger_ms);
    // 当前句音频在 `pcm` 中的起点（上次 endpoint reset 之后）。
    let mut sentence_pcm_start: usize = 0;
    // LOCALRT-ENDPOINT-EMPTY-342：自上次 endpoint 以来是否出现过**有声** chunk
    // （复用现有能量判据 `chunk_rms > silence_threshold`，不另抄阈值）。
    // 用于区分「真 endpoint（本句有语音）」与「静音流上的假 endpoint」：后者不确认、
    // 不推进游标、不并入预览（F3 幻字抑制）。
    let mut speech_since_last_reset: bool = false;
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
    // LOCALRT-REFLOW-HOLE-344-G：上一片派发时的 `committed_len`（= 该片流式文本的起点字符）。
    // 第 i 片对应的流式文本 = `last_display` 的第 `acc_prev_committed` 个字符起的一段。
    let mut acc_prev_committed: usize = 0;
    // LOCALRT-SEAM-337：**自适应定界**状态（见 `on_reflow_commit` 参数文档）。
    // `bound_seg = Some(i)` ⇒ 第 i 片已派发，正在等「文本停止增长 / 有声恢复 / 硬上限」最先发生。
    let mut bound_seg: Option<usize> = None;
    let mut bound_prev_len: usize = 0;
    let mut bound_stable_ms: f32 = 0.0;
    let mut bound_waited_ms: f32 = 0.0;
    // 337 埋点：a/b/c 三分支累计计数（b 的占比 = 本方案收益折损，端测直接读）。
    let mut bound_hit_a: u32 = 0;
    let mut bound_hit_b: u32 = 0;
    let mut bound_hit_c: u32 = 0;

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

        // LOCALRT-PUNCT-TIMER-269-B / LOCALRT-VAD-SILENCE-384 / LOCALRT-NEARFIELD-GATE-389：
        // 本 chunk「是否有人声」—— VAD 可用走 silero + 整句近场段门，不可用退回音量阈值。
        // 🔴 显示层打点、acc 派发、影子、端点、337 边界 b **全部**用这同一个判定。
        let chunk_ms = chunk.len() as f32 / SAMPLE_RATE as f32 * 1000.0;
        // 387（E）：能量和（= 原 chunk_rms 的分子，保持 chunk_rms 逐位不变），并推入 300ms 平滑窗。
        let chunk_energy_sum: f32 = chunk.iter().map(|s| s * s).sum::<f32>();
        let chunk_rms = (chunk_energy_sum / chunk.len() as f32).sqrt();
        let smooth_rms =
            energy_smoother.push(chunk_energy_sum as f64, chunk.len() as u64, chunk_ms);
        // 385：会话音频时间（本 chunk 结束时刻）；供段峰值窗口按时间过期。
        session_ms += chunk_ms;
        let t_vad0 = Instant::now();
        let judgment = chunk_has_speech(
            session_vad,
            &chunk,
            chunk_rms,
            smooth_rms,
            session_ms,
            silence_threshold,
            &mut segment_gate,
        );
        let has_speech = judgment.has_speech;
        let vad_speech = judgment.vad_speech;
        if vad_on {
            let dt_ms = t_vad0.elapsed().as_secs_f64() * 1000.0;
            vad_total_ms += dt_ms;
            vad_chunks += 1;
            if dt_ms > vad_max_ms {
                vad_max_ms = dt_ms;
            }
        }
        if has_speech {
            silent_ms = 0.0;
            acc_silent_ms = 0.0;
            // LOCALRT-ENDPOINT-EMPTY-342：本句出现过有声 chunk ⇒ 后续 endpoint 是真切句。
            speech_since_last_reset = true;
            // LOCALRT-ENDPOINT-284：有新语音进来 → 允许本轮停顿结束后再触发影子。
            // 不立即清 shadow_current：显示侧用「取更长者」避免瞬时缩短/闪烁（见下 base 选择）。
            shadow_done_for_pause = false;
            // PARALLEL-ACC-298：有新语音 → 允许本轮停顿结束后再派发，并标记待派发区间含语音。
            acc_done_for_pause = false;
            acc_pending_has_speech = true;
        } else if vad_on && prev_has_speech && !vad_speech {
            // LOCALRT-VAD-SILENCE-384/389：补偿**只针对 VAD 自身**的「人声→无人声」翻转
            // （silero 过了 min_silence(0.3s) 才报无人声）⇒ 把这段已静时间补记回来。
            // 🔴 只有**已确认段**结束才走到这里（prev_has_speech=true）；未确认段（背景）全程
            //    has_speech=false ⇒ 走下面的普通累加、不补记。
            silent_ms = localrt_vad_seed_ms();
            acc_silent_ms = localrt_vad_seed_ms();
        } else {
            silent_ms += chunk_ms;
            // 346：acc 专用计数器同步累加（标点路径清 silent_ms 时不动它）。
            acc_silent_ms += chunk_ms;
        }
        // LOCALRT-VAD-SILENCE-384 埋点：人声 ↔ 静默切换（端测对照噪声环境下两种判定的差异）。
        if has_speech != prev_has_speech && log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "[LocalRT-DBG-384] speech={} rms={:.5}",
                if has_speech { "on" } else { "off" },
                chunk_rms
            );
        }
        prev_has_speech = has_speech;

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

        // LOCALRT-SEAM-337：自适应定界推进（派发点 P 之后，a/b/c 最先者冻结边界）。
        if let Some(seg) = bound_seg {
            bound_waited_ms += chunk_ms;
            if has_speech {
                // b：有声恢复 ⇒ 立即冻结、**不回灌**（宁可这轮不修，也不吐残留重复）。
                bound_hit_b += 1;
                log::debug!(
                    "[LocalRT-DBG-337] boundary=b (speech resumed) seg={} waited={:.0}ms stable={:.0}ms",
                    seg,
                    bound_waited_ms,
                    bound_stable_ms
                );
                on_reflow_commit(seg, None);
                bound_seg = None;
            } else {
                let cur = last_display.chars().count();
                if cur > bound_prev_len {
                    bound_prev_len = cur;
                    bound_stable_ms = 0.0;
                } else {
                    bound_stable_ms += chunk_ms;
                }
                if bound_stable_ms >= ACC_BOUNDARY_STABLE_MS {
                    // a：静默 + 文本停止增长 ⇒ 滞后补字已吐完，精确边界。
                    bound_hit_a += 1;
                    log::debug!(
                        "[LocalRT-DBG-337] boundary=a (stable {:.0}ms) seg={} committed_len={} (a/b/c={}/{}/{})",
                        bound_stable_ms,
                        seg,
                        cur,
                        bound_hit_a,
                        bound_hit_b,
                        bound_hit_c
                    );
                    on_reflow_commit(seg, Some(cur));
                    bound_seg = None;
                } else if bound_waited_ms >= ACC_BOUNDARY_CAP_MS {
                    // c：硬上限兜底。
                    bound_hit_c += 1;
                    log::debug!(
                        "[LocalRT-DBG-337] boundary=c (cap {:.0}ms) seg={} committed_len={} (a/b/c={}/{}/{})",
                        ACC_BOUNDARY_CAP_MS,
                        seg,
                        cur,
                        bound_hit_a,
                        bound_hit_b,
                        bound_hit_c
                    );
                    on_reflow_commit(seg, Some(cur));
                    bound_seg = None;
                }
            }
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
            // LOCALRT-ROLLBACK-344：307「整句全量重解码」已移除 —— 每次断句多解一整句
            // ~104.5ms，而 `[DBG-307]` **26/26 `gained=0`**（一个字都没捞回），属语音输入主路径
            // 纯开销（Gavin 2026-09-22 约束）。290 的 flush 回落（`flush_text`）保留：它零额外解码，
            // 只做「after 空/更短则用 before」的**不回退保护**。
            // FIX-LOCALRT-TAILCHAR-291：两方取最长（main=flush / shadow）。
            let confirm_text: &str = endpoint_confirm_text(flush_text, shadow_current.as_deref());
            // LOCALRT-ENDPOINT-EMPTY-342（F1+F3）：自上次 endpoint 无有声 chunk ⇒ 静音流上的
            // **假 endpoint**。不确认、不推进游标、不并入预览（静音流吐出的字一律丢弃，幻字抑制）。
            let segment_has_speech = speech_since_last_reset;
            let action = endpoint_action(segment_has_speech, confirm_text.is_empty());
            if action == EndpointAction::SuppressSilence {
                log::debug!(
                    "[LocalRT-DBG-342] endpoint on silence-only segment: suppressed (main_len={} shadow_len={} suppress_len={})",
                    flush_text.chars().count(),
                    shadow_current
                        .as_ref()
                        .map(|s| s.chars().count())
                        .unwrap_or(0),
                    confirm_text.chars().count()
                );
            } else if action == EndpointAction::Confirm {
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
                    // ROLLBACK-344：307 移除后只剩 main / shadow 两源（按指针判定胜出者）。
                    let used = if shadow_current
                        .as_deref()
                        .is_some_and(|s| std::ptr::eq(confirm_text, s))
                    {
                        "shadow"
                    } else {
                        "main"
                    };
                    log::debug!(
                        "[LocalRT-DBG-289] endpoint confirm: main_len={} shadow_len={} used={}",
                        flush_text.chars().count(),
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
            // LOCALRT-SEAM-337：endpoint 若仍有未冻结的边界（罕见：cap 未到但已切句），
            // 按 c 兜底冻结（用确认前的显示长度，与镜像同源），避免悬置。
            if let Some(seg) = bound_seg {
                let cur = last_display.chars().count();
                bound_hit_c += 1;
                log::debug!(
                    "[LocalRT-DBG-337] boundary=c (endpoint flush) seg={} committed_len={} (a/b/c={}/{}/{})",
                    seg,
                    cur,
                    bound_hit_a,
                    bound_hit_b,
                    bound_hit_c
                );
                on_reflow_commit(seg, Some(cur));
                bound_seg = None;
            }
            // LOCALRT-ENDPOINT-284 诊断（方案 A 取证）：每次切句打一行，行数=切句次数。
            // 342：`has_speech=false` 的假 endpoint 不推进 sentence_id（游标不动）。
            log::debug!(
                "[LocalRT-DBG-284] endpoint fired: sentence_id {} -> {} (rule2 cut, has_speech={})",
                sentence_id,
                sentence_id + segment_has_speech as i64,
                segment_has_speech
            );
            // 🔴 input_finished() 之后的 stream 不可复用，下一句换新流（create_stream 廉价：
            // 影子逻辑每 400ms 就建一次，decode 0.5~0.7ms 起步，无性能顾虑）。
            // 342：假 endpoint 也换新流（清 endpoint 闩锁），但**不推进** sentence_id / sentence_pcm_start。
            stream = recognizer.create_stream();
            // 342：假 endpoint（本句无声）**不推进** sentence_id / sentence_pcm_start。
            sentence_id += segment_has_speech as i64;
            let next_sentence_start = if segment_has_speech {
                pcm.len()
            } else {
                sentence_pcm_start
            };
            // 新句从当前总音频长度起算（342：假 endpoint 保持不动）；作废影子。
            sentence_pcm_start = next_sentence_start;
            shadow_current = None;
            shadow_done_for_pause = false;
            speech_since_last_reset = false;
        } else if let Some(r) = recognizer.get_result(&stream) {
            if !r.text.is_empty() && speech_since_last_reset {
                // LOCALRT-LASTCHAR-276：诊断——每次识别文本变化时打印，用于判断「最后一个字
                // 是否在任何一帧 get_result 里出现过」。只在变化时打，有界。
                // URGENT-286：整块（比较 + clone + 格式化）仅在 Debug 级启用时执行 ⇒ 默认 Warn 下零开销。
                if log::log_enabled!(log::Level::Debug) && r.text != last_result_text {
                    log::debug!(
                        "[LocalRT-DBG-276] result='{}' (chars={}) endpoint=false will_reset=false",
                        r.text,
                        r.text.chars().count()
                    );
                    // LOCALRT-TIMESTAMP-336：**只读探针** —— 流式模型到底给不给 token 时间戳。
                    // 🔴 只观测：**不改 `.map(|r| r.text)` 取用行为**，本阶段不碰预览合成/切分。
                    // 量具自检：`text_chars` 恒 >0（本块已在 `!is_empty()` 内）⇒ 行确实执行到，
                    // 「ts=none」才是模型不给（而非日志没跑）。
                    let ts_desc = match &r.timestamps {
                        Some(v) => format!("len={} first_ts={:?}", v.len(), &v[..v.len().min(3)]),
                        None => "none".to_string(),
                    };
                    log::debug!(
                        "[LocalRT-DBG-336] result probe: text_chars={} tokens={} ts={} is_final={} segment={:?} start_time={:?}",
                        r.text.chars().count(),
                        r.tokens.len(),
                        ts_desc,
                        r.is_final,
                        r.segment,
                        r.start_time
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
            } else if !r.text.is_empty() && log::log_enabled!(log::Level::Debug) {
                // LOCALRT-ENDPOINT-EMPTY-342（F3）：静音段（自上次 endpoint 无有声 chunk）
                // 的流式文本一律丢弃，不并入预览（幻字抑制）。
                log::debug!(
                    "[LocalRT-DBG-342] non-endpoint text on silence-only segment: suppressed '{}' (chars={})",
                    r.text,
                    r.text.chars().count()
                );
            }
        }

        // LOCALRT-ENDPOINT-284（方案 B）：静默 ≥ 阈值 → 影子 stream 收尾当前句（**只动显示**）。
        // 影子 = 另起 OnlineStream 喂当前句音频 + input_finished()，拿完整结果；主 stream 不受影响。
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
            } else if !shadow_audio.is_empty() && speech_since_last_reset {
                // URGENT-286：decode 计时只为日志用 ⇒ 仅 Debug 级才取样（默认 Warn 下零开销）。
                let t_shadow = log::log_enabled!(log::Level::Debug).then(Instant::now);
                let shadow = recognizer.create_stream();
                shadow.accept_waveform(SAMPLE_RATE, shadow_audio);
                // LOCALRT-ROLLBACK-344：340 的「shadow 补 500ms 静音」已移除 ——
                // `[DBG-289]` 实测 13/13 `main_len==full_len==shadow_len`，补静音一字未增，
                // 却让**每次停顿**多解 500ms（主路径纯开销）。shadow 机制本身保留（Gavin 明令）。
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
            } else if !shadow_audio.is_empty() {
                // LOCALRT-ENDPOINT-EMPTY-342（F3）：静音段不跑影子（避免静音流幻字进入预览）。
                log::debug!(
                    "[LocalRT-DBG-342] shadow skipped: silence-only segment (no speech since last endpoint)"
                );
            }
        }

        // PARALLEL-ACC-298 / ACC-DISPATCH-SILENCE-ONLY-346：**静默 ≥1200ms 即把这一段派给 accuracy**。
        // 🔴 只推进 `acc_dispatched_end`，**不 reset / 不切句 / 不动 sentence_id**（与 endpoint 解耦）。
        // FIX-SLICE-CUT-AT-GAP-381：经 `build_dispatch_segment`（超 10s 按**字缝**切、最晚 12s 兜底）——
        // 自动加 200ms 边界 padding；常态片（静默切出，通常 <10s）原样解码，>10s 才从 10s 起找字缝切（不丢弃）。
        let acc_pending = pcm.len().saturating_sub(acc_dispatched_end);
        if should_dispatch_acc(
            acc_cfg.enabled,
            // 346：必须用 acc 专用计数器（不被标点清零），不能用显示层 silent_ms。
            acc_silent_ms,
            acc_pending,
            acc_done_for_pause,
            acc_pending_has_speech,
            acc_cfg.silence_ms,
        ) {
            let pending = pcm.len() - acc_dispatched_end;
            let padded = build_dispatch_segment(acc_dispatched_end, pending, pcm.len(), &pcm);
            if !padded.is_empty() {
                if log::log_enabled!(log::Level::Debug) {
                    log::debug!(
                        "[LocalRT-DBG-298] seg dispatch #{}: silence={:.0}ms seg_audio={:.2}s pcm_pos={}",
                        acc_seg_index,
                        acc_silent_ms,
                        pending as f32 / SAMPLE_RATE as f32,
                        acc_dispatched_end
                    );
                }
                // ACC-PREVIEW-REFLOW-325：记下**派发当刻浮层已显示文本的字符数**（committed_len）。
                // 🔴 用 `last_display`（实际已上屏、含标点的串，与镜像 `last_streaming_text` 同源），
                //    不用 `state.display_text()`（裸文本，标点差会造成回填边界偏移）。
                let committed_len = last_display.chars().count();
                // 344-G：本片流式文本（失败片的填补来源），按 char 切片，字符安全。
                let seg_streaming = segment_streaming_text(&last_display, acc_prev_committed);
                acc_prev_committed = committed_len;
                on_segment(acc_seg_index, committed_len, padded, seg_streaming);
                // 337：派发点 P 起，等自适应边界冻结（a/b/c 最先者）。
                bound_seg = Some(acc_seg_index);
                bound_prev_len = committed_len;
                bound_stable_ms = 0.0;
                bound_waited_ms = 0.0;
                acc_seg_index += 1;
            }
            // 无论 padded 是否为空都推进起点 + 上 latch：避免同一停顿反复尝试。
            acc_dispatched_end = pcm.len();
            acc_pending_has_speech = false;
            acc_done_for_pause = true;
        }

        // PUNCT-PREVIEW-SEMANTIC-349：显示刷新 —— 打点**只认静默 ≥1200ms**（读显示层 `silent_ms`）
        // 且有新内容（4s 定时与 shadow 400ms 强制已删，见 `PUNCT_SILENCE_TRIGGER_MS` 与
        // `should_repunctuate_preview`）。🔴 本处打点后会把 `silent_ms` 清零（见下），故 accuracy 派发
        // 用的是另一个不被清零的 `acc_silent_ms`（346），两者**不共用**。
        // 无论打不打点都**只刷新显示**，不动状态机（不 reset / 不切句 / 不改 sentence_id）。
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
            // 有新内容 = raw 比上次打点时长（`raw_len` 初值 0 且 raw 非空 ⇒ 首次恒 true）。
            let has_new = raw_full.len() > punct_cache.raw_len;
            let silence_due =
                should_repunctuate_preview(silent_ms, has_new, PUNCT_SILENCE_TRIGGER_MS);
            let display = preview_display(
                &raw_full,
                punctuation_engine.as_deref_mut(),
                &mut punct_cache,
                silence_due,
            );
            if silence_due {
                // 打完重置**显示层**静默计数。
                // 🔴 346：只清 `silent_ms`，**绝不动 `acc_silent_ms`**（动了 accuracy 的 1200ms 就不可达）。
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
    // 本段未在静默 1200ms 前派出的音频（含短录音全量）在此一次性交出 ⇒ 等价「单片=全量」。
    // 🔴 只有「本段出现过语音」才派（纯静音尾片会把 all_native 翻 false）；有语音的尾巴绝不吞。
    if should_dispatch_tail(
        acc_cfg.enabled,
        pcm.len().saturating_sub(acc_dispatched_end),
        acc_pending_has_speech,
    ) {
        let pending = pcm.len() - acc_dispatched_end;
        let padded = build_dispatch_segment(acc_dispatched_end, pending, pcm.len(), &pcm);
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
            // 344-G：尾片流式文本（同 mid-loop，按 char 切片）。
            let seg_streaming = segment_streaming_text(&last_display, acc_prev_committed);
            on_segment(acc_seg_index, committed_len, padded, seg_streaming);
        }
        // 尾片是最后一次派发：函数即将返回，无需再推进 `acc_dispatched_end`/`acc_seg_index`
        // （推进了也无人读 ⇒ 触发 unused_assignments）。
    }

    // flush 尾部：input_finished 后把剩余可解码帧吐完，再取最终结果。
    // LOCALRT-ROLLBACK-344：340 的「收尾补 2000ms 静音」已移除 —— 与 shadow 补静音同证零收益，
    // 且让**每次录音收尾**多解 2000ms（主路径纯开销）。
    stream.input_finished();
    while recognizer.is_ready(&stream) {
        recognizer.decode(&stream);
    }
    // LOCALRT-SEAM-337（补 1，自适应版）：松手收尾若仍有未冻结边界，按 a 冻结（流式已 flush，
    // 滞后补字必已吐完）⇒ 末态带 acc 前缀。已冻结或从未派发则不动。
    if let Some(seg) = bound_seg {
        let cur = last_display.chars().count();
        bound_hit_a += 1;
        if log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "[LocalRT-DBG-337] boundary=a (recording end flush) seg={} committed_len={} (a/b/c={}/{}/{})",
                seg,
                cur,
                bound_hit_a,
                bound_hit_b,
                bound_hit_c
            );
        }
        on_reflow_commit(seg, Some(cur));
        // 函数即将返回，`bound_seg` 不必再清（清了也无人读 ⇒ unused_assignments）。
    }
    let main_final = recognizer
        .get_result(&stream)
        .map(|r| r.text)
        .unwrap_or_default();
    // LOCALRT-ROLLBACK-344：307「收尾整句全量重解码」已移除（同 endpoint 分支，26/26 gained=0）。
    // flush 结果即最终句文本；**「不得变短 / 整句消失」回落保护**由下方
    // `final_preview = last_display.clone()` + `if !final_seg.is_empty()` 保证 ——
    // main_final 为空/更短时预览保持已显示文本，绝不回退。
    let final_seg = main_final;
    // LOCALRT-FIRSTCHAR-282：最终（含标点）预览全文；供调用方以 `StreamingFinalPreview` 收尾显示。
    // （239-B 丢弃本预览文本、以 pcm 走 accuracy 2pass；此返回值只服务于收尾显示。）
    let mut final_preview = last_display.clone();
    if !final_seg.is_empty() {
        state.on_result(sentence_id, &final_seg, false, &[]);
        // 收尾强制打点一次（录音结束 = 真边界，不属「句中断点」），保证关闭前的预览带标点。
        let raw_full = state.display_text();
        let display = preview_display(
            &raw_full,
            punctuation_engine.as_deref_mut(),
            &mut punct_cache,
            true,
        );
        if !display.is_empty() {
            if display != last_display {
                on_result(&display, &state.display_words());
            }
            final_preview = display;
        }
    }

    // LOCALRT-VAD-SILENCE-384：本次录音 VAD 开销（证明可忽略；Debug 守卫，DEC-077）。
    if vad_on && log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "[LocalRT-DBG-384] vad cost: total_ms={:.1} chunks={} max_chunk_ms={:.1}",
            vad_total_ms,
            vad_chunks,
            vad_max_ms
        );
    }

    // LOCALRT-NEARFIELD-GATE-389：近场段门汇总（Debug 守卫）。
    if vad_on && log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "[LocalRT-DBG-389] nearfield summary: segments={} confirmed={} rejected={} level={:.5}",
            segment_gate.segments,
            segment_gate.confirmed,
            segment_gate.rejected,
            segment_gate.level.estimate()
        );
    }

    // LOCALRT-NEARFIELD-GATE-389（C2）：录音结束写回录音人音量（进程内存），供下次**同设备**沿用。
    // 389 主控验收补（C2-5）：只有**本次录音自己学到**（段峰值样本 ≥2）才写回；
    // 仅靠 seed 就绪时不写回（否则旧 seed 被刷新时间戳、永不过 10 分钟期）。
    // seed 本次被丢弃且没学到新值 ⇒ 清掉 carry（防下次录音重复使用同一坏 seed）。
    if vad_on {
        if segment_gate.level.samples_ready() {
            let level = segment_gate.level.estimate();
            if let Ok(mut c) = LOCALRT_CARRY_LEVEL.lock() {
                *c = Some(CarryLevel {
                    device: vad_device.to_string(),
                    level,
                    updated: Instant::now(),
                });
            }
        } else if segment_gate.seed_dropped {
            if let Ok(mut c) = LOCALRT_CARRY_LEVEL.lock() {
                *c = None;
            }
        }
    }

    Ok((final_preview, pcm))
}

#[cfg(test)]
mod tests {
    use super::{
        build_dispatch_segment, chunk_has_speech, endpoint_action, endpoint_confirm_text,
        local_stream_num_threads, localrt_vad_seed_ms, punct_cache_reuse, seed_usable,
        segment_streaming_text, should_dispatch_acc, should_dispatch_tail,
        should_repunctuate_preview, EndpointAction, EnergySmoother, SegmentGate, SegmentPeakLevel,
        LOCALRT_VAD_MIN_SILENCE_SECS, SAMPLE_RATE,
    };
    use std::time::Duration;

    /// LOCALRT-REFLOW-HOLE-344-G：片段流式文本按**字符**切片，绝不字节切片（中文安全、不 panic）。
    #[test]
    fn reflow_hole_344_segment_streaming_text_is_char_safe() {
        // committed_len 按字符计：跳过前 2 个字符 ⇒ 取「的世界」。
        assert_eq!(segment_streaming_text("你好，的世界", 2), "，的世界");
        // prev=0 ⇒ 全文；prev >= 字符数 ⇒ 空。
        assert_eq!(segment_streaming_text("你好世界", 0), "你好世界");
        assert_eq!(segment_streaming_text("你好世界", 4), "");
        assert_eq!(segment_streaming_text("你好世界", 99), "");
        // 多字节安全：按字符跳过不会拦腰截断（对比字节下标会 panic 的场景）。
        let d = "斯人若彩虹";
        assert_eq!(segment_streaming_text(d, 1), "人若彩虹");
    }

    /// LOCALRT-CHARBOUNDARY-344（P0 回归）：`raw_len` 落在中文字符**中间**（字节下标非 char
    /// 边界）⇒ **不 panic**，且**返回完整原串**（不带缓存前缀）。
    ///
    /// 复现 BUILD-341 `crash.json` 的 `byte index ... is not a char boundary` 场景：
    /// 上一帧在 A 串缓存了字节位置，本帧切到 B 串且该位置正落在多字节字符内部。
    #[test]
    fn charboundary344_mid_char_raw_len_does_not_panic() {
        // "你好世界"：你(0..3) 好(3..6) 世(6..9) 界(9..12)。raw_len=7 落在「世」内部。
        let raw = "你好世界";
        assert!(!raw.is_char_boundary(7), "自检：7 应是非法 char 边界");
        // 不 panic，且返回完整原串（丢弃缓存前缀，绝不截断）。
        assert_eq!(punct_cache_reuse("你好。", raw, 7), raw);
        assert_eq!(punct_cache_reuse("前缀", raw, 1), raw);
        // raw_len 恰在边界（6 = 「世」起点）⇒ 正常拼接。
        assert_eq!(punct_cache_reuse("你好。", raw, 6), "你好。世界");
        // raw_len 在首字符内部（2）⇒ 同样原样返回。
        assert_eq!(punct_cache_reuse("X。", raw, 2), raw);
    }

    /// CHARBOUNDARY-344 边界补测：前缀空 / raw 变短 / 全合法边界等退化输入。
    #[test]
    fn charboundary344_degenerate_inputs_fall_back_to_raw() {
        assert_eq!(punct_cache_reuse("", "你好", 3), "你好", "空前缀 ⇒ 原样");
        assert_eq!(
            punct_cache_reuse("你好。", "你", 6),
            "你",
            "raw 比 raw_len 短 ⇒ 原样（不越界）"
        );
        assert_eq!(
            punct_cache_reuse("你好。", "你好世界", 12),
            "你好。",
            "raw_len == raw.len() ⇒ 仅前缀"
        );
    }

    /// FIX-LOCALRT-TAILCHAR-291（344 修订为两方）：两方取最长（main / shadow），结果恒 ≥ main。
    ///
    /// 覆盖：shadow 更长取 shadow；shadow 为空/更短/等长回落 main（绝不回退）；
    /// 全空返回空（调用方走 EMPTY 分支）。307 的 `full` 来源已按 344 移除。
    #[test]
    fn endpoint_confirm_text_takes_longest_of_two() {
        // shadow 为空 ⇒ 回落 main，句子不消失。
        assert_eq!(endpoint_confirm_text("端测发现的问", None), "端测发现的问");
        // shadow 更短 ⇒ 回落 main（绝不回退）。
        assert_eq!(endpoint_confirm_text("你好世界", Some("你好")), "你好世界");
        // shadow 更长 ⇒ 取 shadow。
        assert_eq!(
            endpoint_confirm_text("看看有什么好看的电", Some("看看有什么好看的电影")),
            "看看有什么好看的电影"
        );
        // 等长 ⇒ 取 main（不抖动）。
        assert_eq!(
            endpoint_confirm_text("你好", Some("您好")),
            "你好",
            "等长取 main"
        );
        // 全空 ⇒ 空（调用方据此走 EMPTY 分支，不做确认）。
        assert_eq!(endpoint_confirm_text("", None), "");
    }

    /// 判据 #4 的不变量：两方取最长后，长度恒 ≥ main（按字符计）。
    #[test]
    fn endpoint_confirm_text_len_not_below_main() {
        let cases: [(&str, Option<&str>); 6] = [
            ("", None),
            ("", Some("尾")),
            ("端测发现的问", None),
            ("端测发现的问", Some("端测发现的问题")),
            ("长一点的前文", Some("更短")),
            ("ab", Some("abc")),
        ];
        for (main, shadow) in cases {
            let chosen = endpoint_confirm_text(main, shadow);
            assert!(
                chosen.chars().count() >= main.chars().count(),
                "main={main:?} shadow={shadow:?} chosen={chosen:?}"
            );
        }
    }

    // ========================================================================
    // LOCALRT-ENDPOINT-EMPTY-342 · F1+F3 决策纯函数
    //   「假 endpoint 不确认/不推进游标 + 静音流幻字不入预览」的行为由 `endpoint_action`
    //   纯函数钉死；端到端时序由源码护栏（下方 guard342_*）钉死。
    // ========================================================================

    /// 判据 #1：假 endpoint（本句无声）即使三方有非空文本也一律抑制（幻字抑制）。
    #[test]
    fn endpoint_action_342_fake_endpoint_suppressed_even_with_text() {
        // 无声 + 有文本 ⇒ 抑制（静音流幻字绝不并入预览）。
        assert_eq!(
            endpoint_action(false, false),
            EndpointAction::SuppressSilence
        );
        // 无声 + 空 ⇒ 同样抑制（不确认、不推进游标）。
        assert_eq!(
            endpoint_action(false, true),
            EndpointAction::SuppressSilence
        );
    }

    /// 判据 #2：真 endpoint（本句有语音）行为逐位不变 —— 有文本确认、空则只记 EMPTY。
    #[test]
    fn endpoint_action_342_real_endpoint_unchanged() {
        assert_eq!(endpoint_action(true, false), EndpointAction::Confirm);
        assert_eq!(endpoint_action(true, true), EndpointAction::Empty);
    }

    // ========================================================================
    // LOCALRT-ENDPOINT-EMPTY-342 · 源码级结构护栏
    // ========================================================================

    /// 判据 #3：endpoint 分支「推进游标」必须被 `segment_has_speech` 门控；
    /// 非 endpoint 分支的 `on_result` 必须被 `speech_since_last_reset` 门控；
    /// 有声判据在函数体内**恰好一处置位**（复用现有能量分支，不另抄阈值）。
    #[test]
    fn guard342_silence_segment_does_not_advance_or_render() {
        let (lines, (lo, hi), (fn_lo, fn_hi)) = endpoint_guard_regions();

        // endpoint 分支：游标推进必须是条件式（假 endpoint 不推进）。
        assert_eq!(
            count_starts(&lines, lo, hi, "sentence_id += segment_has_speech as i64;"),
            1,
            "342: sentence_id 必须按 segment_has_speech 条件推进"
        );
        assert_eq!(
            count_starts(
                &lines,
                lo,
                hi,
                "let next_sentence_start = if segment_has_speech {"
            ),
            1,
            "342: sentence_pcm_start 必须按 segment_has_speech 条件取值"
        );
        assert_eq!(
            count_starts(&lines, lo, hi, "sentence_pcm_start = next_sentence_start;"),
            1,
            "342: sentence_pcm_start 必须赋条件值"
        );
        assert_eq!(
            count_starts(&lines, lo, hi, "sentence_id += 1;"),
            0,
            "342: 不得无条件推进 sentence_id"
        );
        assert_eq!(
            count_starts(&lines, lo, hi, "sentence_pcm_start = pcm.len();"),
            0,
            "342: 不得无条件推进 sentence_pcm_start"
        );

        // 非 endpoint 分支：文本入预览必须与有声音号同条件。
        let sup = (fn_lo..=fn_hi)
            .find(|&i| lines[i].starts_with("if !r.text.is_empty() && speech_since_last_reset {"))
            .expect(
                "342: 非 endpoint 文本必须按 `!r.text.is_empty() && speech_since_last_reset` 门控",
            );
        let onres = (fn_lo..=fn_hi)
            .find(|&i| lines[i].starts_with("state.on_result(sentence_id, &r.text, false"))
            .expect("342: 非 endpoint 中间结果 on_result 行必须存在");
        assert!(
            sup < onres,
            "342: 非 endpoint 的 on_result 必须被 speech_since_last_reset 门控，实测 sup={sup} onres={onres}"
        );

        let n_set = count_starts(&lines, fn_lo, fn_hi, "speech_since_last_reset = true;");
        assert_eq!(
            n_set, 1,
            "342: 有声判据应恰 1 处置位（复用现有能量分支 chunk_rms>silence_threshold），实测 {n_set}"
        );
    }

    /// 判据 #4：endpoint 分支必须走 `endpoint_action(...)` 决策，且抑制分支存在一次。
    #[test]
    fn guard342_uses_endpoint_action() {
        let (lines, (lo, hi), _) = endpoint_guard_regions();
        let n_call = count_contains(
            &lines,
            lo,
            hi,
            "endpoint_action(segment_has_speech, confirm_text.is_empty())",
        );
        assert_eq!(
            n_call, 1,
            "342: endpoint 分支应调用 endpoint_action 恰 1 次，实测 {n_call}"
        );
        let n_sup = count_contains(&lines, lo, hi, "EndpointAction::SuppressSilence");
        assert_eq!(n_sup, 1, "342: 抑制分支应恰 1 处，实测 {n_sup}");
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
    //   I3 endpoint 分支必须用 `recognizer.create_stream()` 换新流（344 移除 307 后为 1 处：仅换流）
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

    /// G3（I3，LOCALRT-ROLLBACK-344 后修订）：endpoint 分支内 `recognizer.create_stream()`
    /// 恰 **1** 处 —— 仅剩 291 句末换新流（`input_finished()` 后旧流不可复用）。
    ///
    /// 沿革：307 全量重解码引入后曾为 2 处；344 移除 307（26/26 gained=0、纯开销）⇒ 回到 1 处。
    ///
    /// **改错怎么红**：删换流 ⇒ 计数 0 ⇒ 红；分支内再多建流（如重新引入全量重解码）⇒ 2 ⇒ 红。
    #[test]
    fn guard291_g3_endpoint_creates_new_stream_once() {
        let (lines, (lo, hi), _) = endpoint_guard_regions();
        let n = count_contains(&lines, lo, hi, "recognizer.create_stream()");
        assert_eq!(
            n, 1,
            "G3: endpoint 分支内应恰 1 处 create_stream（291 句末换流；307 已按 344 移除），实测 {n}"
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
    // PARALLEL-ACC-298 → ACC-DISPATCH-SILENCE-ONLY-346 · 派发触发条件纯函数 + 接线护栏
    //   真实 recognizer 起不来 ⇒ 把触发判据抽纯函数钉死（判据 #3）。
    //
    //   346 按 Gavin「只判断静默1200ms，不按时长来切片」删除长度支：
    //   原 `acc_should_dispatch_or_semantics` 的 3s/5s 长度用例及
    //   `acc_should_dispatch_honors_env_thresholds`（两阈值 env 早已在 320 删除，测试名已失真）
    //   一并删除 —— 它们断言的是已按 DEC-077 移除的机制，保留会锁死一个不该存在的分支。
    // ========================================================================

    /// ACC-DISPATCH-SILENCE-ONLY-346：**只判静默阈值**。
    /// 静默 <1200 不派 / ≥1200 且未上 latch 则派 / 已上 latch 不派 / 纯静音不派 /
    /// 空区间不派 / 总开关关不派；自定义阈值传入即生效。
    #[test]
    fn acc346_should_dispatch_silence_only() {
        let noise = 1usize;
        // 静默 <1200 ⇒ 不派（哪怕区间很长：长度不再参与判据）。
        assert!(!should_dispatch_acc(
            true, 1199.0, noise, false, true, 1200.0
        ));
        // 静默 ≥1200 且未上 latch ⇒ 派。
        assert!(should_dispatch_acc(
            true, 1200.0, noise, false, true, 1200.0
        ));
        // 已上 latch ⇒ 同一停顿不重复派（即使静默继续增长）。
        assert!(!should_dispatch_acc(
            true, 5000.0, noise, true, true, 1200.0
        ));
        // 纯静音区间不派（has_speech=false），无论静默多久。
        assert!(!should_dispatch_acc(
            true, 5000.0, noise, false, false, 1200.0
        ));
        // 空区间不派。
        assert!(!should_dispatch_acc(true, 5000.0, 0, false, true, 1200.0));
        // 总开关关 ⇒ 不派。
        assert!(!should_dispatch_acc(
            false, 5000.0, noise, false, true, 1200.0
        ));
        // 阈值参数传入即生效（400ms 阈值下 400ms 即派）。
        assert!(should_dispatch_acc(true, 400.0, noise, false, true, 400.0));
    }

    /// 🔴 ACC-DISPATCH-SILENCE-ONLY-346 **核心判据**：acc 计数器不被标点路径清零 ⇒ 1200ms 可达。
    ///
    /// 生产时序（见 `transcribe_streaming_local` 循环）：
    ///   静音累加 → acc 检查（本函数）→ … → 标点检查（静默 ≥800 打点后 `silent_ms = 0.0`）。
    /// 若 acc 阈值读显示层 `silent_ms`，则每 800ms 被清零 ⇒ 永远到不了 1200 ⇒ **录音中一次都不派**。
    /// 本测试逐 chunk 模拟该时序，正/反两面各证一次。
    #[test]
    fn acc346_acc_counter_survives_punct_reset() {
        let chunk_ms = 100.0f32;
        let pending = SAMPLE_RATE as usize; // 有音频待派（>0）

        // 正：读 acc 专用计数器（不被标点清零）⇒ 1200ms 可达并派发。
        let mut silent_ms = 0.0f32;
        let mut acc_silent_ms = 0.0f32;
        let mut dispatched = false;
        for _ in 0..20 {
            silent_ms += chunk_ms;
            acc_silent_ms += chunk_ms;
            // acc 检查先于标点（与生产循环同序）。
            if should_dispatch_acc(true, acc_silent_ms, pending, dispatched, true, 1200.0) {
                dispatched = true;
            }
            // 标点路径：静默 ≥800 打点后清显示层计数器（生产 `if silence_due { silent_ms = 0.0; }`）。
            if silent_ms >= 800.0 {
                silent_ms = 0.0;
            }
        }
        assert!(
            dispatched,
            "acc_silent_ms 不被标点清零 ⇒ 1200ms 必须可达（本单核心）"
        );

        // 反：误用被标点清零的 silent_ms ⇒ 永远到不了 1200（复现本单要防的坑）。
        let mut silent_ms = 0.0f32;
        let mut dispatched_wrong = false;
        for _ in 0..20 {
            silent_ms += chunk_ms;
            if should_dispatch_acc(true, silent_ms, pending, dispatched_wrong, true, 1200.0) {
                dispatched_wrong = true;
            }
            if silent_ms >= 800.0 {
                silent_ms = 0.0;
            }
        }
        assert!(
            !dispatched_wrong,
            "误用显示层 silent_ms 则 800ms 被清零 ⇒ 1200ms 不可达（反证）"
        );
    }

    /// 🔴 ACC-DISPATCH-SILENCE-ONLY-346 源码级护栏：钉死「acc 派发读 `acc_silent_ms`、
    /// 且该计数器不被标点路径清零」这一不变式。纯函数测试证不了生产接线，故读源码。
    ///
    /// **改错怎么红**：
    /// - 调用点改回 `silent_ms` ⇒ 接线断言失败；
    /// - `acc_silent_ms` 不再在静音支累加 ⇒ 「累加恰 1 处」失败；
    /// - 在标点路径（`if silence_due`）里清 `acc_silent_ms` ⇒ 「代码行不触碰」失败。
    #[test]
    fn guard346_acc_counter_wiring() {
        let lines = ls_prod_lines();
        // 声明恰 1 处，且初值 0。
        let decl: Vec<usize> = (0..lines.len())
            .filter(|&i| lines[i].starts_with("let mut acc_silent_ms: f32 = 0.0;"))
            .collect();
        assert_eq!(
            decl.len(),
            1,
            "346: acc_silent_ms 声明应恰 1 处，实测 {:?}",
            decl
        );
        // 静音支同步累加恰 1 处。
        let inc: Vec<usize> = (0..lines.len())
            .filter(|&i| lines[i].starts_with("acc_silent_ms += chunk_ms;"))
            .collect();
        assert_eq!(
            inc.len(),
            1,
            "346: acc_silent_ms 应在静音支恰 1 处累加，实测 {:?}",
            inc
        );
        // 归零恰 1 处，且必须落在**语音支**（紧邻显示层 silent_ms 归零）。
        let reset: Vec<usize> = (0..lines.len())
            .filter(|&i| lines[i].starts_with("acc_silent_ms = 0.0;"))
            .collect();
        assert_eq!(
            reset.len(),
            1,
            "346: acc_silent_ms 应恰 1 处归零（且只在语音支），实测 {:?}",
            reset
        );
        let r = reset[0];
        assert!(
            lines[r - 1].starts_with("silent_ms = 0.0;"),
            "346: acc_silent_ms 归零应紧邻显示层 silent_ms 归零（prev={:?}）",
            lines[r - 1]
        );
        assert!(
            lines[r + 1..(r + 4).min(lines.len())]
                .iter()
                .any(|l| l.starts_with("speech_since_last_reset = true;")),
            "346: acc_silent_ms 归零必须落在语音支（后续应出现 speech_since_last_reset = true）"
        );
        // 🔴 标点清零块内**代码行**不得出现 acc_silent_ms（注释行不算）。
        let sd = (0..lines.len())
            .find(|&i| lines[i].starts_with("if silence_due {"))
            .expect("346: 未找到标点清零块 `if silence_due {`");
        let sd_end = sd
            + (sd..lines.len())
                .find(|&i| lines[i] == "}")
                .map(|i| i - sd)
                .expect("346: 标点清零块未闭合");
        let n = (sd..=sd_end)
            .filter(|&i| !lines[i].starts_with("//") && lines[i].contains("acc_silent_ms"))
            .count();
        assert_eq!(
            n, 0,
            "346: 标点路径不得触碰 acc_silent_ms（否则 1200ms 不可达），实测 {n} 处"
        );
        // 调用点必须传 acc_silent_ms 而非显示层 silent_ms。
        let call = (0..lines.len())
            .find(|&i| lines[i].starts_with("if should_dispatch_acc("))
            .expect("346: 未找到 should_dispatch_acc 调用点");
        let block = &lines[call..(call + 12).min(lines.len())];
        assert!(
            block.iter().any(|l| l.starts_with("acc_silent_ms,")),
            "346: should_dispatch_acc 调用点必须传 `acc_silent_ms,`，实测块={:?}",
            block
        );
        assert!(
            !block.iter().any(|l| l.starts_with("silent_ms,")),
            "346: 调用点不得再传显示层 `silent_ms,`，实测块={:?}",
            block
        );
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

    /// LOCALRT-SEAM-337：**lookahead（滞后量）实测** —— 针对音频位置 P 的文本，还需再喂多少
    /// 音频才不再增长。手法：喂真实 wav 到 P，然后**喂静音**（静音不产新字）继续解码，
    /// 观察文本何时停止增长 ⇒ 该增长即为「音频 ≤ P 的滞后补字」，其耗时即 lookahead 上界。
    ///
    /// 手工跑：`cargo test --bin feiyin-ime seam337_lookahead -- --ignored --nocapture`
    /// 对照：`config.yaml` `chunk_length: 500` × `chunk_shift_ratio: 0.5` ⇒ 块 500ms / 步进 250ms
    /// ⇒ 理论滞后上界 = 一个块长 500ms。
    #[test]
    #[ignore = "手工：337 lookahead 实测（需模型 + wav）"]
    fn seam337_lookahead_probe() {
        use sherpa_onnx::Wave;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let rec = super::create_local_stream_recognizer(&root.join("models"))
            .expect("create_local_stream_recognizer（模型须在位）");
        let p = root.join("models/kv259/kv_long.wav");
        let ps = p.to_string_lossy();
        let wave = Wave::read(&ps).expect("kv_long.wav");
        let rate = wave.sample_rate();
        let samples = wave.samples();
        let step = (rate as usize / 50).max(1); // 20ms
        let cap_ms = 2000usize;
        for p_ms in (2000..=50000).step_by(2000) {
            let p_samples = rate as usize * p_ms / 1000;
            if p_samples >= samples.len() {
                continue;
            }
            let stream = rec.create_stream();
            let mut pos = 0usize;
            while pos < p_samples {
                let end = (pos + step).min(p_samples);
                stream.accept_waveform(rate, &samples[pos..end]);
                while rec.is_ready(&stream) {
                    rec.decode(&stream);
                }
                pos = end;
            }
            let len_at_p = rec
                .get_result(&stream)
                .map(|r| r.text)
                .unwrap_or_default()
                .chars()
                .count();
            let zeros = vec![0.0f32; step];
            let mut extra_ms = 0usize;
            let mut last_grow_ms = 0usize;
            let mut prev = len_at_p;
            while extra_ms < cap_ms {
                stream.accept_waveform(rate, &zeros);
                while rec.is_ready(&stream) {
                    rec.decode(&stream);
                }
                extra_ms += 20;
                let l = rec
                    .get_result(&stream)
                    .map(|r| r.text)
                    .unwrap_or_default()
                    .chars()
                    .count();
                if l > prev {
                    last_grow_ms = extra_ms;
                    prev = l;
                }
            }
            println!(
                "[337-LA] P={p_ms}ms len_at_P={len_at_p} final_len={prev} growth={} last_grow_after_P={last_grow_ms}ms",
                prev - len_at_p
            );
        }
    }

    /// LOCALRT-SEAM-337：**尾巴流方案**可行性实测（CPU + 无左上下文质量）。
    ///
    /// 手工跑：`cargo test --bin feiyin-ime seam337_tail_feasibility -- --ignored --nocapture`
    /// 做法：主流式连续喂全 wav 取增量文本；每 5s 音频模拟一次「派发」，同时**新开一条尾巴流**
    /// 只喂该 5s 区间并计时，比较「尾巴流文本」vs「主流式同区间的增量文本」。
    /// 🔴 只读实测，不改任何生产行为。
    #[test]
    #[ignore = "手工：337 尾巴流可行性实测（需模型 + wav）"]
    fn seam337_tail_feasibility_probe() {
        use sherpa_onnx::Wave;
        use std::time::Instant;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let recognizer = super::create_local_stream_recognizer(&root.join("models"))
            .expect("create_local_stream_recognizer（模型须在位）");
        for rel in [
            "models/kv259/kv_long.wav",
            "models/kv259/kv_long_204.wav",
            "models/kv259/kv_short.wav",
        ] {
            let p = root.join(rel);
            let ps = p.to_string_lossy();
            let Some(wave) = Wave::read(&ps) else {
                println!("[337] skip missing {rel}");
                continue;
            };
            let rate = wave.sample_rate();
            let samples = wave.samples();
            let chunk = (rate as usize / 10).max(1); // ≈100ms
            let seg_samples = (rate as usize * 5).max(1); // 模拟派发间隔 = 5s 音频
            let main = recognizer.create_stream();
            let mut tail = recognizer.create_stream();
            let mut main_text = String::new();
            let mut prev_main_len = 0usize;
            let mut tail_decode_ms = 0.0f64;
            let mut tail_audio_ms = 0.0f64;
            let mut pos = 0usize;
            let mut next = seg_samples;
            let mut seg = 0usize;
            while pos < samples.len() {
                let end = (pos + chunk).min(samples.len());
                main.accept_waveform(rate, &samples[pos..end]);
                while recognizer.is_ready(&main) {
                    recognizer.decode(&main);
                }
                tail.accept_waveform(rate, &samples[pos..end]);
                let t = Instant::now();
                while recognizer.is_ready(&tail) {
                    recognizer.decode(&tail);
                }
                tail_decode_ms += t.elapsed().as_secs_f64() * 1000.0;
                tail_audio_ms += (end - pos) as f64 / rate as f64 * 1000.0;
                pos = end;
                if pos >= next || pos == samples.len() {
                    let mt = recognizer
                        .get_result(&main)
                        .map(|r| r.text)
                        .unwrap_or_default();
                    let tt = recognizer
                        .get_result(&tail)
                        .map(|r| r.text)
                        .unwrap_or_default();
                    let delta: String = mt.chars().skip(prev_main_len).collect();
                    let head: String = tt.chars().take(8).collect();
                    println!(
                        "[337] {rel} seg{seg}: audio={:.0}ms tail_decode={:.0}ms rtf={:.3} tail_len={} main_delta_len={} tail_eq_delta={} delta_starts_with_tail_head={}",
                        tail_audio_ms,
                        tail_decode_ms,
                        tail_decode_ms / tail_audio_ms.max(1.0),
                        tt.chars().count(),
                        delta.chars().count(),
                        !tt.is_empty() && tt == delta,
                        !head.is_empty() && delta.starts_with(&head),
                    );
                    println!("[337]   tail_text={tt}");
                    println!("[337]   main_delta={delta}");
                    prev_main_len = mt.chars().count();
                    main_text = mt;
                    tail = recognizer.create_stream(); // 重建：丢弃旧尾巴
                    tail_decode_ms = 0.0;
                    tail_audio_ms = 0.0;
                    next += seg_samples;
                    seg += 1;
                }
            }
            println!("[337] {rel} main_total_chars={}", main_text.chars().count());
        }
    }

    /// LOCALRT-TIMESTAMP-336：**离线**验证流式 paraformer 是否给 token 时间戳。
    ///
    /// 手工跑（不会进常规回归）：
    /// `cargo test --bin feiyin-ime localrt_timestamp_336 -- --ignored --nocapture`
    ///
    /// 做法：复用生产建流参数 [`super::create_local_stream_recognizer`] 构造 recognizer，
    /// 把仓库现成 wav（中文长句）按 ~100ms chunk 喂进 OnlineStream，与生产主循环同序
    /// （`accept_waveform → while is_ready decode → get_result`），每次文本变化打印一行探针。
    /// 🔴 **只读观测**：不改 `.map(|r| r.text)` 取用行为，不碰预览合成。
    /// 🔴 量具自检：同一行打印 `text_chars`（此处恒 >0）⇒「ts=none」可区分「模型不给」与「行未执行」。
    #[test]
    #[ignore = "手工：需模型 + wav；离线验证 336（不接受麦克风）"]
    fn localrt_timestamp_336_probe_offline() {
        use sherpa_onnx::Wave;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let recognizer = super::create_local_stream_recognizer(&root.join("models"))
            .expect("create_local_stream_recognizer（模型须在位）");
        for rel in [
            "models/kv259/kv_long.wav",
            "models/kv259/kv_long_204.wav",
            "models/kv259/kv_short.wav",
            "models/kv259/colloq.wav",
        ] {
            let path = root.join(rel);
            let path_str = path.to_string_lossy();
            let Some(wave) = Wave::read(&path_str) else {
                println!("[336] skip missing {rel}");
                continue;
            };
            let rate = wave.sample_rate();
            let samples = wave.samples();
            let stream = recognizer.create_stream();
            // chunk ≈ 100ms（与生产音频回调量级一致；建流参数由 create_local_stream_recognizer 提供）
            let chunk = (rate as usize / 10).max(1);
            let mut last = String::new();
            let mut printed = 0usize;
            let mut pos = 0usize;
            while pos < samples.len() {
                let end = (pos + chunk).min(samples.len());
                stream.accept_waveform(rate, &samples[pos..end]);
                while recognizer.is_ready(&stream) {
                    recognizer.decode(&stream);
                }
                pos = end;
                if let Some(r) = recognizer.get_result(&stream) {
                    if !r.text.is_empty() && r.text != last {
                        let ts_desc = match &r.timestamps {
                            Some(v) => {
                                format!("len={} first_ts={:?}", v.len(), &v[..v.len().min(3)])
                            }
                            None => "none".to_string(),
                        };
                        println!(
                            "[LocalRT-DBG-336] result probe: text_chars={} tokens={} ts={} is_final={} segment={:?} start_time={:?}",
                            r.text.chars().count(),
                            r.tokens.len(),
                            ts_desc,
                            r.is_final,
                            r.segment,
                            r.start_time
                        );
                        last = r.text.clone();
                        printed += 1;
                        if printed > 20 {
                            break; // 有界，避免刷屏
                        }
                    }
                }
            }
            println!("[336] {rel}: probes_printed={printed}");
        }
    }

    // ========================================================================
    // LOCALRT-ROLLBACK-344：原 LOCALRT-TAILPAD-340「补静音」两条契约**连同机制一并移除**
    //
    // 移除时间/原因（2026-09-22，Gavin 最高约束「主路径不得加拖累性能的机制」）：
    // - ① `tailpad340_padded_never_shorter`：依赖 `endpoint_confirm_text(main, full, shadow)`
    //   三方签名；307 全量重解码与补静音移除后签名收为 `(main, shadow)`，故该断言随签名消失。
    //   「不得让文本变短或整句消失」的不变量**未失守**：改由
    //   `endpoint_confirm_text_len_not_below_main` + `endpoint_confirm_text_takes_longest_of_two`
    //   覆盖（同一条「恒 ≥ main、绝不回退」判据），另由收尾 flush 的
    //   `final_preview = last_display.clone()` + `if !final_seg.is_empty()` 结构保证。
    // - ② `tailpad340_local_only_and_order`：守的是 `feed_tail_silence` 调用顺序红线；
    //   该函数已随 340 机制整体删除（shadow 每次停顿多解 500ms、收尾多解 2000ms，`[DBG-289]`
    //   13/13 实测零收益），守的对象不存在 ⇒ 一并移除，**不是放宽或删除断言**。
    // ========================================================================

    // ========================================================================
    // PUNCT-PREVIEW-SEMANTIC-349 · 预览打点口径（只认静默 ≥1200ms）
    //   因果取证（真实 CT-Transformer 探针 + 实机日志）见
    //   `collab/evidence/20260922-punct349/causal-evidence.md`（探针按 DEC-077 已删）。
    // ========================================================================

    /// PUNCT-349：预览打点**只认静默 ≥1200ms 且有新内容**。
    /// 改前另有「4s 定时」与「shadow 400ms 强制」两条与语义无关的触发 —— 已删除。
    /// 本测试钉死「无 1200ms 静默不打点」（核心新判据）。
    #[test]
    fn punct349_preview_only_on_1200ms_silence() {
        let t = 1200.0f32;
        // 无 1200ms 静默 ⇒ 不打点（哪怕有新内容：4s 定时 / shadow 400ms 已不再触发）。
        assert!(!should_repunctuate_preview(1199.0, true, t));
        assert!(!should_repunctuate_preview(400.0, true, t));
        assert!(!should_repunctuate_preview(0.0, true, t));
        // 无新内容 ⇒ 不打点（持续静默不反复重打同一段）。
        assert!(!should_repunctuate_preview(5000.0, false, t));
        // 静默 ≥1200 且有新内容 ⇒ 打点。
        assert!(should_repunctuate_preview(1200.0, true, t));
        assert!(should_repunctuate_preview(3000.0, true, t));
    }

    /// PUNCT-349 根因表征：触发时刻补的**末尾终止符**会经 `punct_cache_reuse` 落到**句中**
    /// —— 这正是 Gavin 报的「标点打在句子中间」的机制。锁定该复用行为，
    /// 保证「只认 1200ms 语义停顿」这一前置条件不被绕过。
    #[test]
    fn punct349_cache_reuse_carries_terminal_into_midsentence() {
        // 上一次打点发生在「半句」上（触发与语义无关时），引擎在末尾补了「。」：
        let prefix = "今天天气不错，我们准备开始。";
        let raw = "今天天气不错我们准备开始讨论这个项目的具";
        let shown = punct_cache_reuse(prefix, raw, "今天天气不错我们准备开始".len());
        assert_eq!(shown, "今天天气不错，我们准备开始。讨论这个项目的具");
        assert!(
            shown.contains("开始。讨论"),
            "触发时刻的末尾终止符会落到句中（本单要防的现象）"
        );
    }

    // ========================================================================
    // FIX-SLICE-CUT-AT-GAP-381 · 滑窗（accuracy 派发）路径的片构建契约
    //   `build_dispatch_segment`：单片剩余 >10s ⇒ 从 10s 起找**字缝**切、最晚 12s 兜底；
    //   常态片（静默切出，通常 <10s）原样解码。
    // ========================================================================

    /// 🔴 核心：**≤10s 的片不被切**（常态片覆盖：2/4/8s + 恰好 10s 边界）。
    #[test]
    fn sliding_slice_381_short_slices_are_not_split() {
        let sec = SAMPLE_RATE as usize;
        for dsecs in [2usize, 4, 8, 10] {
            let total = dsecs * sec;
            let pcm = vec![0.1f32; total];
            let segs = build_dispatch_segment(0, total, pcm.len(), &pcm);
            assert_eq!(
                segs.len(),
                1,
                "{dsecs}s 片（≤10s）不得被切（per Gavin：只按语义停顿/字缝切）"
            );
            assert_eq!(
                segs[0].len(),
                total,
                "{dsecs}s 整片原样（首段前 padding 被 0 夹住）"
            );
        }
    }

    /// 🔴 超 10s：**按字缝切**（无字缝时最晚 12s 兜底），切点严格相接、不丢样本。
    #[test]
    fn sliding_slice_381_over_10s_uses_gap_cut() {
        let sec = SAMPLE_RATE as usize;
        let total = 200 * sec;
        let pcm = vec![0.1f32; total]; // 恒定能量 ⇒ 无字缝 ⇒ 兜底取 RMS 最低帧（全相等取最早）
        let segs = build_dispatch_segment(0, total, pcm.len(), &pcm);
        assert!(segs.len() >= 2, "200s 必被切：{}", segs.len());
        let lens: Vec<usize> = segs.iter().map(|s| s.len()).collect();
        assert_eq!(lens.iter().sum::<usize>(), total, "切分不丢样本");
        for (i, &l) in lens.iter().enumerate() {
            if i + 1 < lens.len() {
                assert!(
                    l >= 10 * sec && l <= 12 * sec,
                    "非尾片 {i} 长度 {:.2}s 应 ∈[10,12]s",
                    l as f64 / sec as f64
                );
            } else {
                assert!(l < 11 * sec, "尾片 {:.2}s 应 <11s", l as f64 / sec as f64);
            }
        }
    }

    /// 🔴 边界护栏不丢（FIX-VAD-STATE-RESET-001）：start 越界 ⇒ 丢弃（不 panic）；
    /// end 越界 ⇒ clamp 并补 200ms padding。
    #[test]
    fn sliding_slice_381_keeps_vad_state_reset_guards() {
        let sec = SAMPLE_RATE as usize;
        let total = 30 * sec;
        let pcm = vec![0.2f32; total];
        assert!(
            build_dispatch_segment(total, 1000, pcm.len(), &pcm).is_empty(),
            "start >= total_samples ⇒ 丢弃并 warn（不 panic）"
        );
        let segs = build_dispatch_segment(total - 1000, 5000, pcm.len(), &pcm);
        assert_eq!(segs.len(), 1);
        assert_eq!(
            segs[0].len(),
            1000 + crate::transcription::SEGMENT_PADDING_SAMPLES,
            "end 超界 clamp 到 total，再补前向 200ms padding"
        );
    }

    // ========================================================================
    // LOCALRT-VAD-SILENCE-384：本地 realtime 静默判定改 VAD（音量阈值兜底）
    // ========================================================================

    /// 测 #2：VAD 不可用 ⇒ 判定与 `chunk_rms > silence_threshold` **逐位相同**，
    /// 且**不加近场段门**（389 只在 VAD 分支加门）。
    #[test]
    fn localrt384_fallback_matches_energy_threshold() {
        const THR: f32 = 0.01;
        let mut g = SegmentGate::with_seed(None);
        for k in 0..200usize {
            let amp = (k % 60) as f32 / 1000.0; // 0.000..0.059，跨越阈值两侧
            let chunk: Vec<f32> = (0..1600)
                .map(|i| if i % 2 == 0 { amp } else { -amp })
                .collect();
            let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / 1600.0).sqrt();
            let j = chunk_has_speech(None, &chunk, rms, rms, 100.0, THR, &mut g);
            assert_eq!(
                j.has_speech,
                rms > THR,
                "VAD 不可用 ⇒ 判定必须等于 `chunk_rms > silence_threshold`（amp={amp}）"
            );
            assert!(!j.vad_speech, "VAD 不可用 ⇒ vad_speech 恒 false");
        }
        let chunk = vec![THR; 1600];
        assert!(
            !chunk_has_speech(None, &chunk, THR, THR, 100.0, THR, &mut g).has_speech,
            "rms 恰为阈值 ⇒ false（> 而非 >=）"
        );
    }

    /// 测 #3a：计时补偿值 = `min_silence` 毫秒数（0.3s ⇒ 300ms）。
    #[test]
    fn localrt384_seed_is_min_silence_ms() {
        assert_eq!(localrt_vad_seed_ms(), 300.0);
        assert_eq!(localrt_vad_seed_ms(), LOCALRT_VAD_MIN_SILENCE_SECS * 1000.0);
    }

    /// 测 #3b：静默计时三分支（复刻生产 `transcribe_streaming_local` 内联逻辑）。
    ///
    /// ⚠️ 生产逻辑内联在函数体内（受 `guard346` 源码护栏约束、不可抽成函数），故此处**逐字复刻**
    /// 三分支：`has_speech ⇒ 双清零` / `VAD 自身翻转且 vad_on ⇒ 双补 min_silence` / `否则累加`。
    #[test]
    fn localrt384_silence_timing_seeds_then_accumulates() {
        const CHUNK_MS: f32 = 100.0;
        let step = |has_speech: bool,
                    prev: bool,
                    vad_speech: bool,
                    vad_on: bool,
                    silent: f32,
                    acc: f32|
         -> (f32, f32) {
            if has_speech {
                (0.0, 0.0)
            } else if vad_on && prev && !vad_speech {
                (localrt_vad_seed_ms(), localrt_vad_seed_ms())
            } else {
                (silent + CHUNK_MS, acc + CHUNK_MS)
            }
        };

        let mut silent = 0.0f32;
        let mut acc = 0.0f32;
        let mut prev = false;
        // 3 个有声 chunk ⇒ 立即清零。
        for _ in 0..3 {
            let (s, a) = step(true, prev, true, true, silent, acc);
            silent = s;
            acc = a;
            prev = true;
        }
        assert_eq!((silent, acc), (0.0, 0.0));
        // VAD 自身翻转（prev=true、vad_speech=false）⇒ 补记 min_silence 毫秒数。
        let (s, a) = step(false, prev, false, true, silent, acc);
        silent = s;
        acc = a;
        assert_eq!(
            (silent, acc),
            (300.0, 300.0),
            "VAD 翻转 chunk 必须补记 min_silence(300ms)"
        );
        // 之后 9 个静默 chunk（vad_speech=false）⇒ 从真实停顿起算满 1200ms。
        for _ in 0..9 {
            let (s, a) = step(false, false, false, true, silent, acc);
            silent = s;
            acc = a;
        }
        assert_eq!((silent, acc), (1200.0, 1200.0));
        // 有人声 ⇒ 立即清零。
        assert_eq!(step(true, false, true, true, silent, acc), (0.0, 0.0));
        // 389：VAD 仍为人声但段门未确认（vad_speech=true、has_speech=false）⇒ 普通累加、不补。
        assert_eq!(
            step(false, true, true, true, 300.0, 300.0),
            (400.0, 400.0),
            "段门造成的静默是即时的，按普通累加（不补）"
        );
        // 兜底（vad_on=false）永不补记 ⇒ 逐位同 384。
        assert_eq!(
            step(false, true, false, false, 0.0, 0.0),
            (CHUNK_MS, CHUNK_MS)
        );
    }

    /// 测 #4：源码级护栏 ——「是否有人声」判定**只有** `chunk_has_speech` 一处；
    /// 音量阈值比较不得散落在 `transcribe_streaming_local` 函数体内。
    #[test]
    fn guard384_single_speech_judgment_via_chunk_has_speech() {
        let lines = ls_prod_lines();
        let is_code = |i: usize| !lines[i].starts_with("//");
        let fn_line = first_line(&lines, "pub fn transcribe_streaming_local(");
        let (fn_lo, fn_hi) = block_bounds_291(&lines, fn_line);
        let cmp = (fn_lo..=fn_hi)
            .filter(|&i| {
                is_code(i)
                    && (lines[i].contains("chunk_rms > silence_threshold")
                        || lines[i].contains("chunk_rms <= silence_threshold"))
            })
            .count();
        assert_eq!(
            cmp, 0,
            "384: transcribe_streaming_local 函数体（剔除注释）内不得直接比较音量阈值（必须走 chunk_has_speech）"
        );
        let n_call = (fn_lo..=fn_hi)
            .filter(|&i| is_code(i) && lines[i].contains("chunk_has_speech("))
            .count();
        assert_eq!(
            n_call, 1,
            "384: 函数体应恰调用一次共享判定 chunk_has_speech"
        );
        // 兜底判定函数内恰含一处音量阈值比较（VAD 不可用时的唯一来源）。
        let ch_line = first_line(&lines, "fn chunk_has_speech(");
        let (ch_lo, ch_hi) = block_bounds_291(&lines, ch_line);
        let n_cmp = (ch_lo..=ch_hi)
            .filter(|&i| is_code(i) && lines[i].contains("chunk_rms > silence_threshold"))
            .count();
        assert_eq!(
            n_cmp, 1,
            "384: 兜底判定 chunk_has_speech 必须恰含一处音量阈值比较"
        );
    }

    // ========================================================================
    // LOCALRT-NEARFIELD-GATE-389：近场门改「按整句（VAD 段）判定」——段峰值估计 + 段门状态机
    // ========================================================================

    /// 测 #1：`SegmentPeakLevel` — 样本 <2 未就绪；估计取**中位数**；按会话时间 30s 过期。
    #[test]
    fn seg389_level_ready_median_and_window() {
        let mut lv = SegmentPeakLevel::with_seed(None);
        assert!(!lv.ready(), "0 样本未就绪");
        lv.push(1.0, 0.0);
        assert!(!lv.ready(), "1 样本仍未就绪（需 ≥2）");
        lv.push(0.5, 10.0);
        assert!(lv.ready());
        // 2 样本 [0.5,1.0]：中位数索引 len/2=1 ⇒ 1.0（上中位）。
        assert!((lv.estimate() - 1.0).abs() < 1e-3);
        lv.push(0.2, 20.0);
        // 3 样本 [0.2,0.5,1.0] ⇒ 中位 0.5。
        assert!((lv.estimate() - 0.5).abs() < 1e-3, "中位数应为 0.5");
        // 会话时间过期：now=30001 ⇒ t=0 过期；t=10/20 保留。
        lv.prune(30_001.0);
        assert_eq!(lv.samples.len(), 2, "t=0 应过期");
        assert!(
            (lv.estimate() - 0.5).abs() < 1e-3,
            "剩 [0.2,0.5] 上中位 = 0.5"
        );
    }

    /// 测 #2：段内出现响亮峰值 ⇒ 整句确认；其后**轻音/句尾**全算有声（不再句中切静默）。
    #[test]
    fn seg389_confirms_on_peak_then_holds_through_light_tail() {
        let mut g = SegmentGate::with_seed(None);
        let mut now = 0.0f32;
        // 两段响亮（peak 1.0）⇒ 就绪（未就绪期段直接确认）。
        for _ in 0..2 {
            for _ in 0..3 {
                now += 100.0;
                assert!(g.update(true, 1.0, now));
            }
            now += 100.0;
            assert!(!g.update(false, 0.0, now));
        }
        assert!(g.level.ready(), "2 段后应就绪");
        assert!((g.level.estimate() - 1.0).abs() < 1e-3);
        // 段 3：响亮起始确认，随后轻音尾巴仍算有声。
        now += 100.0;
        assert!(g.update(true, 1.0, now), "响亮起始应确认整句");
        now += 100.0;
        assert!(g.update(true, 0.05, now), "轻音字仍算有声（已确认）");
        now += 100.0;
        assert!(g.update(true, 0.05, now), "句尾/清辅音仍算有声（已确认）");
        now += 100.0;
        assert!(!g.update(false, 0.0, now));
        assert_eq!(g.confirmed, 3);
    }

    /// 测 #3：背景段（整句峰值均 < `level × 0.3`）⇒ 全程 has_speech=false，**不得污染 level**（防自我放行）。
    #[test]
    fn seg389_background_segment_rejected_no_self_admit() {
        let mut g = SegmentGate::with_seed(None);
        let mut now = 0.0f32;
        for _ in 0..2 {
            for _ in 0..3 {
                now += 100.0;
                g.update(true, 1.0, now);
            }
            now += 100.0;
            g.update(false, 0.0, now);
        }
        assert!(g.level.ready());
        let lvl_before = g.level.estimate();
        // 背景段：vad 全程 true、平滑音量恒 0.2（< 0.3×1.0）⇒ 整段静默。
        let mut silent_chunks = 0usize;
        for _ in 0..10 {
            now += 100.0;
            if !g.update(true, 0.2, now) {
                silent_chunks += 1;
            }
        }
        now += 100.0;
        assert!(!g.update(false, 0.0, now));
        assert_eq!(silent_chunks, 10, "背景段全程应判静默（不再句中切活）");
        assert_eq!(g.rejected, 1, "背景段应计入 rejected");
        assert!(
            (g.level.estimate() - lvl_before).abs() < 1e-3,
            "背景段不得污染 level"
        );
    }

    /// 测 #4：冷启动（level 未就绪）⇒ 整句直接确认，极轻也放行（保开头不丢字）。
    #[test]
    fn seg389_cold_start_segment_passes_through() {
        let mut g = SegmentGate::with_seed(None);
        let mut now = 0.0f32;
        now += 100.0;
        assert!(g.update(true, 0.01, now), "未就绪 ⇒ 整句放行");
        now += 100.0;
        assert!(g.update(true, 0.01, now));
        now += 100.0;
        assert!(!g.update(false, 0.0, now));
    }

    /// 测 #5：会话时间过期 —— 30s 无已确认段后旧样本过期 ⇒ 未就绪 ⇒ 重新热身（最坏锁定 ≤30s）。
    #[test]
    fn seg389_window_expires_then_relearns() {
        let mut g = SegmentGate::with_seed(None);
        let mut now = 0.0f32;
        for _ in 0..2 {
            for _ in 0..3 {
                now += 100.0;
                g.update(true, 1.0, now);
            }
            now += 100.0;
            g.update(false, 0.0, now);
        }
        assert!(g.level.ready());
        // 30s 静默（vad=false）⇒ 旧样本按会话时间过期。
        for _ in 0..310 {
            now += 100.0;
            assert!(!g.update(false, 0.0, now));
        }
        assert!(!g.level.ready(), "30s 后旧样本过期 ⇒ 未就绪（锁定 ≤30s）");
        // 录音人降音量后重新说话 ⇒ 未就绪 ⇒ 整句放行（不丢字）。
        now += 100.0;
        assert!(g.update(true, 0.2, now), "未就绪 ⇒ 整句直接放行");
    }

    // ========================================================================
    // TEST-SYNC-389（阶段一 · 新算法单测）：整句段门 / 平滑音量
    // ========================================================================

    /// 平滑音量：音节（150ms 高能 + 50ms 近零）交替 ⇒ 平滑波动 < 逐块波动；
    /// 30 万块长跑仍有限非负（浮点累计误差不得产出 NaN/负数）。
    #[test]
    fn ts389_smoother_fluctuation_lt_raw_and_300k_blocks_stable() {
        let n = 160u64;
        let hi = (1.0f64) * n as f64;
        let lo = (0.001f64).powi(2) * n as f64;
        let mut sm = EnergySmoother::new();
        let (mut raw, mut smoothed) = (Vec::new(), Vec::new());
        for k in 0..600usize {
            let high = (k % 20) < 15;
            let (sum, rms) = if high { (hi, 1.0f32) } else { (lo, 0.001f32) };
            smoothed.push(sm.push(sum, n, 10.0));
            raw.push(rms);
        }
        let span = |v: &[f32]| -> f32 {
            v.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
                - v.iter().cloned().fold(f32::INFINITY, f32::min)
        };
        assert!(
            span(&smoothed[30..]) < span(&raw[30..]),
            "平滑波动 {} 应 < 逐块波动 {}",
            span(&smoothed[30..]),
            span(&raw[30..])
        );
        let mut sm2 = EnergySmoother::new();
        for k in 0..300_000usize {
            let high = (k % 20) < 15;
            let sum = if high { hi } else { lo };
            let v = sm2.push(sum, n, 10.0);
            assert!(
                v.is_finite() && v >= 0.0,
                "k={k}: 平滑值 {v} 非法（NaN/负数）"
            );
        }
    }

    /// 整句段门契约：录音人音节语音（VAD 全真、平滑音量在响亮 1.0 / 轻音 0.05 间波动）
    /// ⇒ **整段全部判有声**（不再句中切静默）；背景段（整句峰值 0.1×level）⇒ **整段判静默**。
    #[test]
    fn ts389_segment_gate_syllabic_all_speech_background_all_silent() {
        let mut g = SegmentGate::with_seed(None);
        let mut now = 0.0f32;
        // 学两段响亮（peak 1.0）⇒ 就绪。
        for _ in 0..2 {
            for _ in 0..3 {
                now += 100.0;
                g.update(true, 1.0, now);
            }
            now += 100.0;
            g.update(false, 0.0, now);
        }
        assert!(g.level.ready());
        // 录音人音节段：响亮峰值 + 轻音交替 ⇒ 整段有声。
        let mut speech = 0usize;
        let mut total = 0usize;
        for i in 0..300usize {
            now += 10.0;
            let sm = if (i % 20) < 15 { 1.0 } else { 0.05 };
            total += 1;
            if g.update(true, sm, now) {
                speech += 1;
            }
        }
        now += 10.0;
        g.update(false, 0.0, now);
        assert_eq!(speech, total, "整句段门：录音人音节语音应全部判有声");
        // 背景段：整句峰值 0.1×level ⇒ 全静默。
        let mut silent = 0usize;
        let mut btotal = 0usize;
        for _ in 0..100usize {
            now += 10.0;
            btotal += 1;
            if !g.update(true, 0.1, now) {
                silent += 1;
            }
        }
        now += 10.0;
        g.update(false, 0.0, now);
        assert_eq!(silent, btotal, "背景整句应全部判静默");
    }

    // ========================================================================
    // LOCALRT-NEARFIELD-GATE-389（C2）：跨录音沿用录音人音量
    // ========================================================================

    /// 389 主控验收补（C2-5）：seed 连续两句被拒 ⇒ `seed_dropped` 置位且未学到新值
    ///（录音结束据此清 carry）；仅靠 seed 就绪时 `samples_ready()` 为假（不写回）。
    #[test]
    fn seg389c2_review_seed_dropped_and_no_writeback_without_samples() {
        let mut g = SegmentGate::with_seed(Some(1.0));
        assert!(
            g.level.ready() && !g.level.samples_ready(),
            "仅 seed 就绪 ⇒ 不应写回"
        );
        let mut t = 0.0f32;
        for _ in 0..2 {
            // 一段低于 0.3×seed 的人声（被拒），随后静音结束该段
            for _ in 0..10 {
                g.update(true, 0.1, t);
                t += 10.0;
            }
            g.update(false, 0.0, t);
            t += 10.0;
        }
        assert!(g.seed_dropped, "连续两句被拒必须标记 seed 已丢弃");
        assert!(!g.level.samples_ready(), "未学到新值");
    }

    /// C2-①：沿用判据 = 同设备 且 未超 10 分钟。
    #[test]
    fn seg389c2_seed_usable_same_device_within_age() {
        assert!(
            seed_usable("mic-A", Duration::from_secs(60), "mic-A"),
            "同设备 1 分钟 ⇒ 沿用"
        );
        assert!(
            seed_usable("", Duration::from_secs(599), ""),
            "空串=系统默认，同 key ⇒ 沿用"
        );
        assert!(
            !seed_usable("mic-A", Duration::from_secs(601), "mic-A"),
            "超 10 分钟 ⇒ 不沿用"
        );
        assert!(
            !seed_usable("mic-A", Duration::from_secs(1), "mic-B"),
            "换设备 ⇒ 不沿用"
        );
    }

    /// C2-②：有 seed ⇒ 立即就绪、estimate=seed；本次样本就绪后 seed 不再参与。
    #[test]
    fn seg389c2_seed_makes_ready_and_used_until_samples_ready() {
        let mut g = SegmentGate::with_seed(Some(1.0));
        assert!(g.level.ready(), "有 seed ⇒ 立即就绪（第一句即按 seed 判）");
        assert!((g.level.estimate() - 1.0).abs() < 1e-3, "estimate=seed");
        assert!(g.level.seed_in_use());
        let mut now = 0.0f32;
        // 第 1 句响亮（1.0 ≥ 0.3×seed）⇒ 确认。
        now += 100.0;
        assert!(g.update(true, 1.0, now));
        now += 100.0;
        assert!(!g.update(false, 0.0, now));
        assert!(g.level.seed_in_use(), "本次仅 1 样本 ⇒ seed 仍参与");
        // 第 2 句响亮 ⇒ 本次 2 样本就绪 ⇒ seed 不再参与估计。
        now += 100.0;
        assert!(g.update(true, 1.0, now));
        now += 100.0;
        assert!(!g.update(false, 0.0, now));
        assert!(!g.level.seed_in_use(), "本次样本就绪 ⇒ seed 不再参与");
        assert!((g.level.estimate() - 1.0).abs() < 1e-3);
    }

    /// C2-③：seed 沿用期开头连续 2 句被拒 ⇒ 丢弃 seed，回到未就绪（整句直接确认）。
    #[test]
    fn seg389c2_two_consecutive_rejects_drop_seed() {
        let mut g = SegmentGate::with_seed(Some(1.0));
        let mut now = 0.0f32;
        for _ in 0..2 {
            now += 100.0;
            assert!(!g.update(true, 0.1, now), "背景句（0.1×seed）应被挡");
            now += 100.0;
            assert!(!g.update(false, 0.0, now));
        }
        assert!(
            g.level.seed.is_none() && !g.level.seed_in_use(),
            "连续 2 拒 ⇒ 丢弃 seed"
        );
        assert!(!g.level.ready(), "丢弃后回到未就绪");
        // 未就绪 ⇒ 下一整句直接放行（重新热身）。
        now += 100.0;
        assert!(g.update(true, 0.05, now), "未就绪 ⇒ 整句直接确认");
    }

    /// C2-④：录音结束写回条件 = 本次 level 就绪（学满 2 段）。
    #[test]
    fn seg389c2_writeback_ready_condition() {
        let mut g = SegmentGate::with_seed(None);
        assert!(!g.level.ready(), "未学到段 ⇒ 不就绪 ⇒ 不写回");
        let mut now = 0.0f32;
        for _ in 0..2 {
            now += 100.0;
            g.update(true, 1.0, now);
            now += 100.0;
            g.update(false, 0.0, now);
        }
        assert!(g.level.ready(), "学到 2 段 ⇒ 就绪 ⇒ 写回");
    }
}
