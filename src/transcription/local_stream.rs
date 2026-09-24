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
/// 循环内检查顺序**，是脆弱耦合。独立计数器让两个消费者互不影响（与 acc 自身 latch
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

/// LOCALRT-PERF-405（F-C-01）：显示文本增量缓存。
///
/// `StreamingAsrState::display_text()` 每次调用都 `confirmed_sentences.join("") + current`
/// （O(总长) 分配 + 拷贝）；流式循环每 chunk（~100/s）调一次，长录音下随文本线性增长。
/// 本结构在**切句时**才更新 confirmed 区，当前句中间结果只替换尾部 current 区：
/// - `on_confirm(text)`：丢弃 current、把 `text` 追加进 confirmed 区（切句事件，低频）；
/// - `on_current(text)`：截断到 confirmed 末尾、写入 current（每 chunk，O(current)）。
///
/// 🔴 与 `StreamingAsrState::display_text()` 在**同一批 `on_result` 调用序列**下**逐字等价**，
/// 由 `tests::display_cache_matches_state_display_text` 随机验证。`confirmed_bytes` 恒为 confirmed
/// 区字节长度、也是合法 char 边界 ⇒ `truncate` 安全（不 panic）。
#[derive(Default)]
struct DisplayCache {
    buf: String,
    confirmed_bytes: usize,
}

impl DisplayCache {
    fn new() -> Self {
        Self::default()
    }
    /// 切句确认：`confirm` 并入 confirmed 区，current 清空（对应 `on_result(.., true, ..)`）。
    fn on_confirm(&mut self, confirm: &str) {
        self.buf.truncate(self.confirmed_bytes);
        self.buf.push_str(confirm);
        self.confirmed_bytes = self.buf.len();
    }
    /// 当前句中间结果：替换 current 区、confirmed 区不变（对应 `on_result(.., false, ..)`）。
    fn on_current(&mut self, current: &str) {
        self.buf.truncate(self.confirmed_bytes);
        self.buf.push_str(current);
    }
    fn text(&self) -> &str {
        &self.buf
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
/// VAD-393（A3）：生产派发改走 [`build_dispatch_segment_with_spans`]（带时间线映射）；本无 spans
/// 版本保留供单测（对照音频逐位相等）⇒ 生产非测试构建无调用点，允许 dead_code。
#[allow(dead_code)]
fn build_dispatch_segment(
    start: usize,
    len: usize,
    total_samples: usize,
    pcm: &[f32],
) -> Vec<Vec<f32>> {
    super::build_sliding_segments(&[(start, len)], total_samples, pcm)
}

/// VAD-393（A2）：同 [`build_dispatch_segment`]，**额外**返回每片 `(pcm_start, pcm_end, pad_before)`。
/// 供把实时 VAD 时间线映射为片内坐标（复用实时判断、不再对每窗重跑 VAD）。
fn build_dispatch_segment_with_spans(
    start: usize,
    len: usize,
    total_samples: usize,
    pcm: &[f32],
) -> (Vec<Vec<f32>>, Vec<(usize, usize, usize)>) {
    super::build_sliding_segments_with_spans(&[(start, len)], total_samples, pcm)
}

/// VAD-393（A2）：把会话语音时间线（`pcm` 绝对坐标）按各片的 `spans`（`(pcm_start, pcm_end, pad_before)`）
/// 映射为**片内坐标**：与 `[pcm_start, pcm_end)` 相交的部分平移 `- pcm_start + pad_before`。
/// 与某片无交 ⇒ 该片为空表。纯函数，可单测。
fn slice_ranges_from_timeline(
    timeline: &[(usize, usize)],
    spans: &[(usize, usize, usize)],
) -> Vec<Vec<(usize, usize)>> {
    spans
        .iter()
        .map(|&(ps, pe, pad_before)| {
            let mut out = Vec::new();
            for &(ts, te) in timeline {
                let s = ts.max(ps);
                let e = te.min(pe);
                if s < e {
                    out.push((s - ps + pad_before, e - ps + pad_before));
                }
            }
            out
        })
        .collect()
}

/// VAD-393-R1：派发片的片内区间。VAD 不可用，**或派发时刻 VAD 仍在语音段中** ⇒ `None`
///（调用方回退 391 自跑 VAD，整窗重判，不会漏掉进行中段）。
///
/// 依据（sherpa-onnx 1.13.8 `voice-activity-detector.cc`）：`IsSpeechDetected()` 返回 `start_ != -1`
///（:196）—— 进行中段的标志；段只在**非语音分支**入队（:110-120）或 `Flush()`（:187）里产出，
/// 且同一次处理里 `start_ = -1`（:134 或 :190）。C API `SherpaOnnxVoiceActivityDetectorDetected`
/// → `impl->IsSpeechDetected()`（`c-api.cc:1391-1398`）。⇒ `vad_speech == true` 时该段尚未进
/// `timeline`，据残缺时间线剪静音会把它剪掉（吞字），故回退自跑 VAD。
fn dispatch_slice_ranges(
    vad_on: bool,
    vad_mid_speech: bool,
    timeline: &[(usize, usize)],
    spans: &[(usize, usize, usize)],
) -> Option<Vec<Vec<(usize, usize)>>> {
    if !vad_on || vad_mid_speech {
        return None;
    }
    Some(slice_ranges_from_timeline(timeline, spans))
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
    // VAD-393（A2）：会话语音时间线（`pcm` 绝对坐标），喂入同时收集已完成段。
    timeline: &mut Vec<(usize, usize)>,
) -> ChunkJudgment {
    match vad {
        Some(v) => {
            // VAD-393：feed_speech = 逐 512 块喂 + 收集已完成段（time）；返回值等价 feed_is_speech。
            let vad_speech = v.feed_speech(chunk, timeline);
            let has_speech = gate.update(vad_speech, smooth_rms, now_ms);
            ChunkJudgment {
                has_speech,
                vad_speech,
            }
        }
        None => {
            // GATE-TIMING-ONLY-392：VAD 不可用时 `vad_speech` 取**音量兜底判定**（= has_speech）⇒
            // 「内容去留看 vad_speech」的整条判定与 384 **逐位相同**（门不参与）。
            let energy_speech = chunk_rms > silence_threshold;
            ChunkJudgment {
                has_speech: energy_speech,
                vad_speech: energy_speech,
            }
        }
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
    /// GATE-TIMING-ONLY-392：本次录音的**第一个已确认段不学习**（避开按键声 / 起音高峰）；
    /// 该段仍按规则确认、仍算有声，只是不 `push` 峰值。
    first_learn_skipped: bool,
}

impl SegmentPeakLevel {
    /// C2：带跨录音 seed 新建（`Some` ⇒ 立即就绪、`estimate`=seed，直到本次样本就绪）。
    fn with_seed(seed: Option<f32>) -> Self {
        Self {
            samples: std::collections::VecDeque::new(),
            seed,
            consecutive_rejects: 0,
            first_learn_skipped: false,
        }
    }

    /// GATE-TIMING-ONLY-392：收录一个**已确认段**峰值到学习样本；**第一个已确认段跳过**（不学习）。
    /// 返回本次是否真的写入（供埋点 `learned=`）。
    fn learn_segment(&mut self, peak: f32, now_ms: f32) -> bool {
        if !self.first_learn_skipped {
            self.first_learn_skipped = true;
            return false; // 392：第一段不学习（避开按键声/起音高峰）
        }
        self.push(peak, now_ms);
        true
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
    ///
    /// 🔴 GATE-TIMING-ONLY-392：偶数样本取**下中位数**（`v[(len-1)/2]`）—— 两样本时取**较小者**。
    /// BUILD-390 现场 `[peak=0.0966(含按键声), 0.0215]` 取上中位数得 0.0966 ⇒ 门限 0.029 ⇒ 录音人后续整句被误判。
    /// 须先 `prune(now_ms)`。
    fn estimate(&self) -> f32 {
        if self.samples_ready() {
            let mut v: Vec<f32> = self.samples.iter().map(|(p, _)| *p).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v[(v.len() - 1) / 2]
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
            // 段结束：只学**已确认段**峰值（背景段被拒 ⇒ 不污染 level）；392：第一段不学习。
            self.segments += 1;
            let mut learned = false;
            if self.seg_confirmed {
                self.confirmed += 1;
                learned = self.level.learn_segment(self.seg_peak, now_ms);
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
                    "[LocalRT-DBG-389] segment end: peak={:.5} level={:.5} confirmed={} learned={}",
                    self.seg_peak,
                    self.level.estimate(),
                    self.seg_confirmed,
                    learned
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
    // VAD-393（A2）：新增第 5 参 `slice_ranges`：**片内坐标**的语音区间（VAD 时间线映射而来）；
    // VAD 不可用 ⇒ `None`（调用方回退自行跑 VAD）。
    mut on_segment: impl FnMut(usize, usize, Vec<Vec<f32>>, String, Option<Vec<Vec<(usize, usize)>>>),
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
    // GATE-TIMING-ONLY-392 埋点：VAD 判人声但门判非近场（被门拒）的 chunk 数。
    let mut vad_only_speech_chunks = 0u64;

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

    // 当前句音频在 `pcm` 中的起点（上次 endpoint reset 之后）。
    let mut sentence_pcm_start: usize = 0;
    // LOCALRT-ENDPOINT-EMPTY-342：自上次 endpoint 以来是否出现过**有声** chunk
    // （复用现有能量判据 `chunk_rms > silence_threshold`，不另抄阈值）。
    // 用于区分「真 endpoint（本句有语音）」与「静音流上的假 endpoint」：后者不确认、
    // 不推进游标、不并入预览（F3 幻字抑制）。
    let mut speech_since_last_reset: bool = false;
    // VAD-393（A2）：会话语音时间线（`pcm` 绝对坐标，按序）。实时 VAD 逐 chunk 产出，派发时
    // 映射为片内 ranges ⇒ **复用实时判断、不再对每个窗口重跑 VAD**（且窗口开头有前文、结论与实时一致）。
    let mut speech_timeline: Vec<(usize, usize)> = Vec::new();
    // LOCALRT-PERF-405（F-C-01）：显示文本增量缓存（替代每 chunk `state.display_text()` 的
    // 全量 join + 分配）。切句时更新 confirmed 区，当前句只替换尾部 current 区。
    let mut display_cache = DisplayCache::new();

    // PARALLEL-ACC-298：accuracy 并行派发状态（与端点**完全独立**）。
    // 已派发到的 `pcm` 位置（下一片从这里起算）。
    let mut acc_dispatched_end: usize = 0;
    // 本轮静默是否已派发过（latch 写法）。
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
        // F-A-02（LOCALRT-PERF-405）：VAD 计时**仅喂 Debug 汇总**（见函数尾 `[DBG-384] vad cost`）⇒
        // 非 Debug 下连 `Instant::now()` 都不取（DEC-077：诊断埋点不拖累实时主路径）。
        let t_vad0 = (vad_on && log::log_enabled!(log::Level::Debug)).then(Instant::now);
        let judgment = chunk_has_speech(
            session_vad,
            &chunk,
            chunk_rms,
            smooth_rms,
            session_ms,
            silence_threshold,
            &mut segment_gate,
            &mut speech_timeline, // VAD-393（A2）：喂入同时收集已完成段
        );
        let has_speech = judgment.has_speech;
        let vad_speech = judgment.vad_speech;
        if let Some(t0v) = t_vad0 {
            let dt_ms = t0v.elapsed().as_secs_f64() * 1000.0;
            vad_total_ms += dt_ms;
            vad_chunks += 1;
            if dt_ms > vad_max_ms {
                vad_max_ms = dt_ms;
            }
        }
        // GATE-TIMING-ONLY-392：**时序**（停顿计时 / 派发时机 / 预览打点 / 337 边界 b）继续用
        // **过门结果** `has_speech`；**内容去留**（`speech_since_last_reset` / `acc_pending_has_speech` /
        // `*_done_for_pause` 复位）改用 **VAD 原始判定** `vad_speech` ——
        // 门误判时最坏只是停顿判断不准，**绝不丢录音人的话**（BUILD-390：门误判 ⇒ 342 丢流式文本 +
        // 298 不派发 ⇒ 预览/最终输出卡在 0 或前半段）。代价：VAD 判人声的背景说话可能被识别进结果。
        // 🔴 VAD 不可用时 `vad_speech` == 音量兜底判定 ⇒ 整条判定与 384 逐位相同。
        if !has_speech {
            if vad_on && prev_has_speech && !vad_speech {
                // LOCALRT-VAD-SILENCE-384/389：VAD 自身「人声→无人声」翻转（silero 过 min_silence 才报）
                // ⇒ 把这段已静时间补记回来。只有**已确认段**结束才走到这里（prev_has_speech=true）。
                silent_ms = localrt_vad_seed_ms();
                acc_silent_ms = localrt_vad_seed_ms();
            } else {
                silent_ms += chunk_ms;
                // 346：acc 专用计数器同步累加（标点路径清 silent_ms 时不动它）。
                acc_silent_ms += chunk_ms;
            }
        } else {
            // 392 主控验收补：「本轮停顿已派发」的复位属于**时序**，必须跟静默计时同源（has_speech）。
            // 若随 vad_speech 复位：背景人声被门拒 ⇒ acc_silent_ms 持续累加不清零，而 done 标记每个 chunk
            // 被清 ⇒ 静默已 ≥1200ms ⇒ **每个 chunk（~10ms）派发一次**，串行解码队列被碎片窗口淹没。
            // 改回 has_speech：背景人声只随一次派发送出，之后要等录音人开口。
            // 392 主控验收补（续）：done 复位写在计时清零**之前**，使 346 护栏（归零紧邻 silent_ms、
            // 其后 3 行内出现 speech_since_last_reset）与 392 护栏（done 复位在 else 分支）同时成立。
            acc_done_for_pause = false;
            silent_ms = 0.0;
            acc_silent_ms = 0.0;
        }
        if vad_speech {
            speech_since_last_reset = true;
            acc_pending_has_speech = true;
            // LOCALRT-ENDPOINT-EMPTY-342（392 契约变更）：本句 VAD 判有人声 ⇒ 342 后续 endpoint 文本不丢；
            // 门只管时序（见上）；门误判时最坏只是停顿判断不准，**绝不丢录音人的话**。
        }
        // 392 埋点：VAD 判人声但门判非近场（被门拒）的 chunk 数 ⇒ 端测观察背景占比（可观测性）。
        // F-A-02（405）：只喂 Debug 汇总（`[DBG-389] nearfield summary`）⇒ Debug 守卫。
        if vad_speech && !has_speech && log::log_enabled!(log::Level::Debug) {
            vad_only_speech_chunks += 1;
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
                // F-A-02（405）：bound_hit_b 仅喂 DBG-337 日志 ⇒ Debug 守卫。
                if log::log_enabled!(log::Level::Debug) {
                    bound_hit_b += 1;
                    log::debug!(
                        "[LocalRT-DBG-337] boundary=b (speech resumed) seg={} waited={:.0}ms stable={:.0}ms",
                        seg,
                        bound_waited_ms,
                        bound_stable_ms
                    );
                }
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
                    // F-A-02（405）：bound_hit_a 仅喂 DBG-337 日志 ⇒ Debug 守卫。
                    if log::log_enabled!(log::Level::Debug) {
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
                    }
                    on_reflow_commit(seg, Some(cur));
                    bound_seg = None;
                } else if bound_waited_ms >= ACC_BOUNDARY_CAP_MS {
                    // c：硬上限兜底。
                    // F-A-02（405）：bound_hit_c 仅喂 DBG-337 日志 ⇒ Debug 守卫。
                    if log::log_enabled!(log::Level::Debug) {
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
                    }
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
            // flush 前文本仅作**回落**用（after 空/更短则用 before，见下方 `flush_text`）。
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
            // FIX-LOCALRT-TAILCHAR-291：flush 前/后取长者（`flush_text` 已在上面算好，绝不回退）。
            // DEC-086：影子收尾已移除，原「main / shadow 两方取长」的辅助函数一并删除。
            let confirm_text: &str = flush_text;
            // LOCALRT-ENDPOINT-EMPTY-342（F1+F3）：自上次 endpoint 无有声 chunk ⇒ 静音流上的
            // **假 endpoint**。不确认、不推进游标、不并入预览（静音流吐出的字一律丢弃，幻字抑制）。
            let segment_has_speech = speech_since_last_reset;
            let action = endpoint_action(segment_has_speech, confirm_text.is_empty());
            if action == EndpointAction::SuppressSilence {
                log::debug!(
                    "[LocalRT-DBG-342] endpoint on silence-only segment: suppressed (main_len={} suppress_len={})",
                    flush_text.chars().count(),
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
                    // DEC-086：影子收尾已移除 ⇒ `[DBG-289]`（main/shadow 取舍）日志随之删除；只剩主解。
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
                // F-C-01（405）：同步增量缓存（confirmed 追加 confirm_text、current 清空）。
                display_cache.on_confirm(confirm_text);
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
                // F-A-02（405）：bound_hit_c 仅喂 DBG-337 日志 ⇒ Debug 守卫。
                if log::log_enabled!(log::Level::Debug) {
                    bound_hit_c += 1;
                    log::debug!(
                        "[LocalRT-DBG-337] boundary=c (endpoint flush) seg={} committed_len={} (a/b/c={}/{}/{})",
                        seg,
                        cur,
                        bound_hit_a,
                        bound_hit_b,
                        bound_hit_c
                    );
                }
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
                // F-C-01（405）：同步增量缓存（替换 current 区；confirmed 区不变）。
                display_cache.on_current(&r.text);
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

        // DEC-086（LOCALRT-PERF-405）：原「静默 ≥400ms 影子收尾（另起 stream 重解当前句）」分支
        // **整段移除** —— 实测零净收益（endpoint 定稿极少采用影子结果、尾字也补不回），却在本流式
        // 解码线程内同步重解整句（单次峰值 1267ms），严重阻塞预览实时性。末字补全改由后续机制解决。

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
            let (padded, spans) =
                build_dispatch_segment_with_spans(acc_dispatched_end, pending, pcm.len(), &pcm);
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
                // VAD-393（A2/R1）：派发由**静默 1200ms**（门 `has_speech`）触发，不保证派发当刻
                // VAD 段已结束（门拒背景人声 / 误拒录音人轻声 / 本句与背景声无缝相接 ⇒ VAD 仍在段中）。
                // VAD 已结束 ⇒ 时间线完整、可用；VAD 仍在段中（进行中段尚未入队）⇒ 回退 391 自跑 VAD
                //（`dispatch_slice_ranges` 返回 None），绝不据缺失时间线剪掉这一段（= 吞字）。
                let slice_ranges =
                    dispatch_slice_ranges(vad_on, vad_speech, &speech_timeline, &spans);
                if slice_ranges.is_none() && vad_on && vad_speech {
                    if log::log_enabled!(log::Level::Debug) {
                        log::debug!(
                            "[LocalRT-DBG-393] timeline fallback: vad mid-speech at dispatch seg={}",
                            acc_seg_index
                        );
                    }
                }
                on_segment(
                    acc_seg_index,
                    committed_len,
                    padded,
                    seg_streaming,
                    slice_ranges,
                );
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
        // 且有新内容（4s 定时与 shadow 400ms 强制已删；整个影子机制按 DEC-086 亦已移除，见
        // `PUNCT_SILENCE_TRIGGER_MS` 与 `should_repunctuate_preview`）。🔴 本处打点后会把 `silent_ms` 清零（见下），故 accuracy 派发
        // 用的是另一个不被清零的 `acc_silent_ms`（346），两者**不共用**。
        // 无论打不打点都**只刷新显示**，不动状态机（不 reset / 不切句 / 不改 sentence_id）。
        // 显示基文本（LOCALRT-PERF-405 F-C-01 增量缓存）：`confirmed` 区 + 当前句 current 区。
        // DEC-086：影子收尾已移除，不再有 main/shadow 取长切换。
        let raw_full: &str = display_cache.text();
        if !raw_full.is_empty() {
            // 有新内容 = raw 比上次打点时长（`raw_len` 初值 0 且 raw 非空 ⇒ 首次恒 true）。
            let has_new = raw_full.len() > punct_cache.raw_len;
            let silence_due =
                should_repunctuate_preview(silent_ms, has_new, PUNCT_SILENCE_TRIGGER_MS);
            let display = preview_display(
                raw_full,
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
    // VAD-393（A2）：🔴 尾片派发**之前** flush 实时 VAD，把尾句剩余语音补进时间线
    //（否则尾句未达 min_silence、不在 timeline ⇒ ranges 缺尾句）。
    if let Some(v) = session_vad {
        v.flush_speech(&mut speech_timeline);
    }
    if should_dispatch_tail(
        acc_cfg.enabled,
        pcm.len().saturating_sub(acc_dispatched_end),
        acc_pending_has_speech,
    ) {
        let pending = pcm.len() - acc_dispatched_end;
        let (padded, spans) =
            build_dispatch_segment_with_spans(acc_dispatched_end, pending, pcm.len(), &pcm);
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
            // VAD-393（A2/R1）：尾片前已 `flush_speech` ⇒ 无进行中段 ⇒ `mid_speech=false`
            //（`vad_on=false` ⇒ None，回退 391 自跑 VAD，与中途派发同口径）。
            let slice_ranges = dispatch_slice_ranges(vad_on, false, &speech_timeline, &spans);
            on_segment(
                acc_seg_index,
                committed_len,
                padded,
                seg_streaming,
                slice_ranges,
            );
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
        // F-A-02（405）：bound_hit_a 仅喂 DBG-337 日志 ⇒ Debug 守卫。
        if log::log_enabled!(log::Level::Debug) {
            bound_hit_a += 1;
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
        // F-C-01（405）：同步增量缓存（替换 current 区）。
        display_cache.on_current(&final_seg);
        // 收尾强制打点一次（录音结束 = 真边界，不属「句中断点」），保证关闭前的预览带标点。
        let raw_full = display_cache.text();
        let display = preview_display(
            raw_full,
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
            "[LocalRT-DBG-389] nearfield summary: segments={} confirmed={} rejected={} level={:.5} vad_only_speech_chunks={}",
            segment_gate.segments,
            segment_gate.confirmed,
            segment_gate.rejected,
            segment_gate.level.estimate(),
            vad_only_speech_chunks
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

/// VAD-393（A2）：实时 VAD 时间线 → 片内坐标映射（纯函数，无需模型）。
#[cfg(test)]
mod timeline393_tests {
    use super::{
        build_dispatch_segment_with_spans, dispatch_slice_ranges, slice_ranges_from_timeline,
        VadSegmenter,
    };

    /// 393-R1：`dispatch_slice_ranges` 四种组合 —— `!vad_on` 或 `mid_speech` ⇒ `None`（回退自跑）。
    #[test]
    fn ts393_dispatch_slice_ranges_fallback_when_mid_speech() {
        let timeline = [(100usize, 200usize)];
        let spans = [(0usize, 400usize, 0usize)];
        assert!(dispatch_slice_ranges(false, false, &timeline, &spans).is_none());
        assert!(dispatch_slice_ranges(false, true, &timeline, &spans).is_none());
        assert!(dispatch_slice_ranges(true, true, &timeline, &spans).is_none());
        assert_eq!(
            dispatch_slice_ranges(true, false, &timeline, &spans),
            Some(vec![vec![(100, 200)]])
        );
    }

    /// 393-R2 源码护栏：`transcribe_streaming_local` 生产区（剔除注释行）中，尾片派发前必须
    /// `flush_speech(&mut speech_timeline)` —— 出现在 `if should_dispatch_tail(` **之前**。
    #[test]
    fn ts393_flush_speech_before_tail_dispatch_source_guard() {
        let src = include_str!("local_stream.rs");
        let body = src
            .split("pub fn transcribe_streaming_local(")
            .nth(1)
            .expect("transcribe_streaming_local 锚点缺失");
        // 生产区 = 到第一个 `#[cfg(test)]` 之前。
        let prod = body.split("#[cfg(test)]").next().unwrap();
        let code: String = prod
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let flush = code
            .find("flush_speech(&mut speech_timeline)")
            .expect("flush_speech 调用缺失");
        let tail = code
            .find("if should_dispatch_tail(")
            .expect("should_dispatch_tail 调用缺失");
        assert!(flush < tail, "flush_speech 必须在尾片派发之前");
    }

    /// 单片段：时间线落在片段内 ⇒ 平移 `-pcm_start + pad_before`。
    #[test]
    fn ts393_slice_ranges_maps_single_span() {
        // 片段 pcm 绝对 [1000, 4000)，前置 pad 200 ⇒ 语音 [1500,3000) → 片内 [700,2200)。
        let timeline = [(1500usize, 3000usize)];
        let spans = [(1000usize, 4000usize, 200usize)];
        assert_eq!(
            slice_ranges_from_timeline(&timeline, &spans),
            vec![vec![(700, 2200)]]
        );
    }

    /// 无交 ⇒ 该片空表；部分相交 ⇒ 裁剪到片段边界后平移。
    #[test]
    fn ts393_slice_ranges_clips_and_empties() {
        let timeline = [(0usize, 100usize), (1500, 9000)];
        let spans = [(1000usize, 2000usize, 0usize)];
        // (0,100) 无交 ⇒ 丢；(1500,9000)∩(1000,2000)=(1500,2000) ⇒ 平移 -1000 = (500,1000)。
        assert_eq!(
            slice_ranges_from_timeline(&timeline, &spans),
            vec![vec![(500, 1000)]]
        );
    }

    /// 多片段：跨片段的时间线分别落到各片（各片独立平移）。
    #[test]
    fn ts393_slice_ranges_multi_span() {
        let timeline = [(1200usize, 1800usize), (2500, 3200)];
        let spans = [
            (1000usize, 2000usize, 0usize),
            (2000usize, 3000usize, 0usize),
        ];
        assert_eq!(
            slice_ranges_from_timeline(&timeline, &spans),
            vec![vec![(200, 800)], vec![(500, 1000)]]
        );
    }

    /// `build_dispatch_segment_with_spans` 与 `build_dispatch_segment` 音频逐位一致，
    /// 且 `spans.len() == padded.len()`、`pcm_start == pad_before`（片内语音起点）。
    #[test]
    fn ts393_dispatch_with_spans_matches_plain_audio() {
        let pcm: Vec<f32> = (0..16000).map(|i| (i as f32) * 0.001).collect();
        let (padded, spans) = build_dispatch_segment_with_spans(0, pcm.len(), pcm.len(), &pcm);
        let plain = super::build_dispatch_segment(0, pcm.len(), pcm.len(), &pcm);
        assert_eq!(padded.len(), plain.len());
        assert_eq!(spans.len(), padded.len());
        for &(ps, pe, pad_before) in &spans {
            assert!(ps <= pe);
            assert_eq!(pad_before, 0, "整段无前置 pad ⇒ pad_before=0");
        }
    }

    /// E2E（需 silero v6 模型 + full.wav）：**实时时间线复用**端到端 ——
    /// 逐 512 样本喂 `feed_speech` 收集时间线 → 组一个整段派发 → 映射为片内区间 →
    /// `trim_to_speech` 剪静音。断言：① 时间线非空；② 映射区间都落在片段内且有序非空；
    /// ③ 剪后保留语音（不增长）且**短于**原片段（首尾 2s 静音被剪）。
    #[test]
    #[ignore = "requires silero model + full.wav; cargo test --bin feiyin-ime -- --ignored --nocapture timeline393"]
    fn timeline393_realtime_timeline_trim_e2e() {
        const RATE: usize = 16_000;
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = root.join("models");
        let wav = root.join("collab/research/audio-real-gavin/processed/full.wav");
        let Some(wave) = sherpa_onnx::Wave::read(wav.to_str().unwrap()) else {
            eprintln!("skip: full.wav 缺失");
            return;
        };
        let Some(vad) = VadSegmenter::try_new_for_local_silence(&model_dir) else {
            eprintln!("skip: silero VAD 不可用");
            return;
        };
        // 首尾各 2s 静音 ⇒ 保证剪静音有可剪之处；中间为真人声 full.wav。
        let mut audio = vec![0.0f32; 2 * RATE];
        audio.extend_from_slice(wave.samples());
        audio.extend(std::iter::repeat(0.0f32).take(2 * RATE));
        // 逐 512 喂入（与生产 realtime 一致），收集已完成段为时间线。
        let mut timeline: Vec<(usize, usize)> = Vec::new();
        for chunk in audio.chunks(512) {
            vad.feed_speech(chunk, &mut timeline);
        }
        vad.flush_speech(&mut timeline);
        assert!(!timeline.is_empty(), "v6 时间线应检测到语音段");
        // 组一个整段派发（span 覆盖全量）→ 映射为**每子段**的片内区间。
        let (padded, spans) =
            build_dispatch_segment_with_spans(0, audio.len(), audio.len(), &audio);
        let slice_ranges = slice_ranges_from_timeline(&timeline, &spans);
        assert_eq!(slice_ranges.len(), padded.len(), "每子段一组区间");
        assert!(
            slice_ranges.iter().any(|r| !r.is_empty()),
            "至少一个子段应含语音区间"
        );
        // 模拟 main.rs `shift_and_concat_ranges`：拼窗 + 各子段区间偏移到窗内坐标。
        let lens: Vec<usize> = padded.iter().map(|s| s.len()).collect();
        let window_audio: Vec<f32> = padded.concat();
        let mut flat: Vec<(usize, usize)> = Vec::new();
        let mut off = 0usize;
        for (j, r) in slice_ranges.iter().enumerate() {
            for &(s, e) in r {
                flat.push((s + off, e + off));
            }
            off += lens[j];
        }
        let mut prev = 0usize;
        for &(s, e) in &flat {
            assert!(
                s < e && e <= window_audio.len(),
                "区间越界/倒置: {s}..{e} / {}",
                window_audio.len()
            );
            assert!(s >= prev, "区间应有序");
            prev = e;
        }
        let pad = (crate::transcription::vad::LOCALRT_TRIM_PAD_SECS * RATE as f32) as usize;
        let trimmed = crate::transcription::trim_to_speech(&window_audio, &flat, pad);
        let in_secs = window_audio.len() as f32 / RATE as f32;
        let out_secs = trimmed.len() as f32 / RATE as f32;
        println!(
            "timeline393 e2e: in={in_secs:.2}s out={out_secs:.2}s timeline={} subsegs={} ranges={}",
            timeline.len(),
            padded.len(),
            flat.len()
        );
        assert!(!trimmed.is_empty(), "剪后不得为空");
        assert!(out_secs <= in_secs + 1e-3, "剪静音不应增长");
        assert!(out_secs < in_secs, "首尾 2s 静音应被剪 ⇒ out 应短于 in");
    }

    /// 393-R3：对一个派发片逐子片做「实时时间线剪静音」(tl) vs「391 自跑 VAD 剪静音」(vad) 对照。
    /// 追加行到 `rows`；超差 / 吞字记入 `fails`。模块级（无闭包捕获），供 R3 E2E 调用。
    fn check_dispatch_393(
        seg: usize,
        pcm: &[f32],
        start: usize,
        end: usize,
        mid_speech: bool,
        timeline: &[(usize, usize)],
        probe: &VadSegmenter,
        pad: usize,
        rows: &mut Vec<String>,
        fails: &mut Vec<String>,
    ) {
        if end <= start {
            return;
        }
        let (slices, spans) = build_dispatch_segment_with_spans(start, end - start, pcm.len(), pcm);
        if slices.is_empty() {
            return;
        }
        let ranges = dispatch_slice_ranges(true, mid_speech, timeline, &spans);
        for (k, slice) in slices.iter().enumerate() {
            let in_secs = slice.len() as f32 / 16_000.0;
            let vad_out =
                crate::transcription::trim_to_speech(slice, &probe.speech_ranges(slice), pad);
            let vad_secs = vad_out.len() as f32 / 16_000.0;
            match &ranges {
                Some(rs) => {
                    let tl_out = crate::transcription::trim_to_speech(slice, &rs[k], pad);
                    let tl_secs = tl_out.len() as f32 / 16_000.0;
                    let diff = (tl_secs - vad_secs).abs();
                    rows.push(format!(
                        "seg={seg} sub={k} in={in_secs:.2} tl_out={tl_secs:.2} vad_out={vad_secs:.2} diff={diff:.2}"
                    ));
                    if diff > 0.3 {
                        fails.push(format!(
                            "seg={seg} sub={k} |tl-vad|={diff:.2}>0.3 (tl={tl_secs:.2} vad={vad_secs:.2})"
                        ));
                    }
                    if vad_secs > 0.0 && tl_secs <= 0.0 {
                        fails.push(format!(
                            "seg={seg} sub={k} vad_out={vad_secs:.2}>0 但 tl_out=0（时间线少剪出语音）"
                        ));
                    }
                }
                None => rows.push(format!(
                    "seg={seg} sub={k} in={in_secs:.2} tl_out=fallback vad_out={vad_secs:.2}"
                )),
            }
        }
    }

    /// 393-R3：真模型 E2E —— 按 1200ms 静默切派发片，逐子片对照「实时时间线剪静音」(tl) 与
    /// 「391 自跑 VAD 剪静音」(vad)。断言非回退子片 `|tl − vad| ≤ 0.3s`，且 vad 有语音时 tl 不得为 0。
    /// 原 `timeline393_realtime_timeline_trim_e2e` **保留不删**（本测为其加强版）。
    #[test]
    #[ignore = "requires silero model + full.wav; cargo test --bin feiyin-ime -- --ignored --nocapture timeline393r3"]
    fn timeline393_r3_per_dispatch_vs_391_e2e() {
        const RATE: usize = 16_000;
        const CHUNK: usize = 160; // 10ms，与生产 realtime 同粒度喂入
        const SILENCE_TRIGGER_MS: f32 = 1200.0;

        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = root.join("models");
        let wav = root.join("collab/research/audio-real-gavin/processed/full.wav");
        let Some(wave) = sherpa_onnx::Wave::read(wav.to_str().unwrap()) else {
            eprintln!("skip: full.wav 缺失");
            return;
        };
        let Some(vad) = VadSegmenter::try_new_for_local_silence(&model_dir) else {
            eprintln!("skip: silero VAD 不可用");
            return;
        };
        let Some(probe) = VadSegmenter::try_new_for_local_trim(&model_dir) else {
            eprintln!("skip: silero trim VAD 不可用");
            return;
        };
        // 首尾各 2s 静音 ⇒ 保证有可剪静音 + 尾片。
        let mut audio = vec![0.0f32; 2 * RATE];
        audio.extend_from_slice(wave.samples());
        audio.extend(std::iter::repeat(0.0f32).take(2 * RATE));

        let pad = (crate::transcription::vad::LOCALRT_TRIM_PAD_SECS * RATE as f32) as usize;
        let min_silence_ms = crate::transcription::vad::LOCALRT_VAD_MIN_SILENCE_SECS * 1000.0;

        let mut timeline: Vec<(usize, usize)> = Vec::new();
        let mut pcm: Vec<f32> = Vec::with_capacity(audio.len());
        let mut dispatched_end = 0usize;
        let mut silent_ms = 0.0f32;
        let mut prev_vad = false;
        let mut seg_has_speech = false;
        let mut seg = 0usize;
        let mut rows: Vec<String> = Vec::new();
        let mut fails: Vec<String> = Vec::new();

        for off in (0..audio.len()).step_by(CHUNK) {
            let end = (off + CHUNK).min(audio.len());
            let chunk = &audio[off..end];
            pcm.extend_from_slice(chunk);
            let vad_speech = vad.feed_speech(chunk, &mut timeline);
            let cms = chunk.len() as f32 / RATE as f32 * 1000.0;
            if vad_speech {
                silent_ms = 0.0;
                seg_has_speech = true;
            } else if prev_vad {
                // LOCALRT-VAD-SILENCE-384：VAD「人声→无人声」翻转补 min_silence（同生产）。
                silent_ms = min_silence_ms;
            } else {
                silent_ms += cms;
            }
            prev_vad = vad_speech;
            // 静默 ≥1200ms 且本段有语音 ⇒ 派发；`vad_speech` 作为 mid-speech 判据（本测不模拟近场门）。
            if silent_ms >= SILENCE_TRIGGER_MS && seg_has_speech && pcm.len() > dispatched_end {
                check_dispatch_393(
                    seg,
                    &pcm,
                    dispatched_end,
                    pcm.len(),
                    vad_speech,
                    &timeline,
                    &probe,
                    pad,
                    &mut rows,
                    &mut fails,
                );
                seg += 1;
                dispatched_end = pcm.len();
                seg_has_speech = false;
            }
        }
        // 尾片：flush 后无进行中段 ⇒ mid_speech=false。
        vad.flush_speech(&mut timeline);
        if pcm.len() > dispatched_end && seg_has_speech {
            check_dispatch_393(
                seg,
                &pcm,
                dispatched_end,
                pcm.len(),
                false,
                &timeline,
                &probe,
                pad,
                &mut rows,
                &mut fails,
            );
        }

        println!(
            "timeline393-r3 per-dispatch comparison ({} rows):",
            rows.len()
        );
        for r in &rows {
            println!("  {r}");
        }
        for f in &fails {
            eprintln!("  FAIL: {f}");
        }
        assert!(!rows.is_empty(), "应至少产生一个派发子片");
        assert!(fails.is_empty(), "存在超差/吞字子片：{}", fails.join("; "));
    }

    /// 393-A2①（TEST-SYNC）：一条时间线段**横跨两个子片** ⇒ 两片各得截断后的部分、平移正确；
    /// 首片 `pad_before > 0` 时区间起点 = `pad_before + (ts − pcm_start)`。
    #[test]
    fn ts393c_one_segment_spans_two_slices_truncates_and_translates() {
        // 片0 pcm [1000,2000) 前置 pad 200；片1 pcm [2000,3000) 前置 pad 50。
        let spans = [(1000usize, 2000usize, 200usize), (2000, 3000, 50)];
        let timeline = [(1500usize, 2500usize)]; // 一条段横跨两片
        let out = slice_ranges_from_timeline(&timeline, &spans);
        // 片0：∩[1500,2000) ⇒ 起点 = 200 + (1500-1000) = 700；终点 = 200 + (2000-1000) = 1200。
        // 片1：∩[2000,2500) ⇒ 起点 = 50 + (2000-2000) = 50；终点 = 50 + (2500-2000) = 550。
        assert_eq!(out, vec![vec![(700, 1200)], vec![(50, 550)]]);
        assert_eq!(
            out[0][0].0,
            200 + (1500 - 1000),
            "首片起点必须 = pad_before + (ts − pcm_start)"
        );
        for (k, ranges) in out.iter().enumerate() {
            for &(s, e) in ranges {
                assert!(s < e, "片{k}: 区间必须非空");
            }
        }
    }

    /// 393-A2②（TEST-SYNC）：时间线段与子片**首尾相接**（`te == ps` 或 `ts == pe`）⇒ 不产生空区间。
    #[test]
    fn ts393c_touching_boundaries_produce_no_empty_ranges() {
        let spans = [(1000usize, 2000usize, 0usize)];
        // 前段 te == ps(1000)；后段 ts == pe(2000) —— 均只「相接」不「相交」。
        let timeline = [(500usize, 1000usize), (2000, 2500)];
        let out = slice_ranges_from_timeline(&timeline, &spans);
        assert_eq!(out, vec![Vec::<(usize, usize)>::new()]);
        assert!(
            out[0].iter().all(|&(s, e)| e > s),
            "首尾相接不得产生空区间（仅 s<e 才 push）"
        );
        // 对照：真正相交（ts < pe 且 te > ps）才产出。
        let overlapping = [(1500usize, 2500usize)];
        assert_eq!(
            slice_ranges_from_timeline(&overlapping, &spans),
            vec![vec![(500, 1000)]]
        );
    }

    /// 393-R1 源码护栏（TEST-SYNC，非作者）：`transcribe_streaming_local` 生产区（剔除注释行）中，
    /// **中途派发**只以 `dispatch_slice_ranges(vad_on, vad_speech,` 调用（VAD 进行中段 ⇒ 回退自跑）；
    /// **尾片**只以 `dispatch_slice_ranges(vad_on, false,` 调用（flush 后无进行中段）。各恰 1 处。
    #[test]
    fn ts393c_dispatch_slice_ranges_two_call_sites_source_guard() {
        let src = include_str!("local_stream.rs");
        // 生产区 = 首个 `#[cfg(test)]` 之前；剔除注释行（含 `///` 文档）防误命中。
        let prod = src.split("#[cfg(test)]").next().unwrap();
        let code: String = prod
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            code.matches("dispatch_slice_ranges(vad_on, vad_speech,")
                .count(),
            1,
            "中途派发必须恰 1 处以 vad_speech 传入（进行中段回退）"
        );
        assert_eq!(
            code.matches("dispatch_slice_ranges(vad_on, false,").count(),
            1,
            "尾片派发必须恰 1 处以 false 传入（flush 后无进行中段）"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_dispatch_segment, chunk_has_speech, endpoint_action, local_stream_num_threads,
        localrt_vad_seed_ms, punct_cache_reuse, seed_usable, segment_streaming_text,
        should_dispatch_acc, should_dispatch_tail, should_repunctuate_preview, DisplayCache,
        EndpointAction, EnergySmoother, SegmentGate, SegmentPeakLevel,
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

    // LOCALRT-PERF-405（DEC-086）：影子收尾移除后，`endpoint_confirm_text`（main/shadow 两方取长）
    // 及其两条测试一并删除。「flush 前/后取长、绝不回退」语义仍由生产调用方的 `flush_text`
    // （`after_text` 空/更短则回落 `before_text`）保证，行为由流式回归与 §`flush` 结构覆盖。

    /// LOCALRT-PERF-405（F-C-01）：`DisplayCache` 与 `StreamingAsrState::display_text()` **逐字等价**。
    ///
    /// 随机（确定性 LCG）生成一批 `on_result` 事件序列（切句 / 同句中间结果 / 新句），
    /// 分别喂给 `DisplayCache` 与 `StreamingAsrState`，断言两者全文恒等（含中文多字节边界）。
    #[test]
    fn display_cache_matches_state_display_text() {
        use super::StreamingAsrState;
        let mut seed: u64 = 0x1234_5678_9abc_def0;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            seed
        };
        let frags = [
            "你好",
            "世界，",
            "今天天气",
            "不错。",
            "",
            "测",
            "试一下",
            "。",
        ];
        let mut cache = DisplayCache::new();
        let mut state = StreamingAsrState::new();
        let mut sid: i64 = 1;
        for i in 0..5000 {
            let text = frags[(next() % frags.len() as u64) as usize];
            let end = next() % 4 == 0;
            if end {
                state.on_result(sid, text, true, &[]);
                cache.on_confirm(text);
                sid += 1;
            } else {
                state.on_result(sid, text, false, &[]);
                cache.on_current(text);
            }
            assert_eq!(
                cache.text(),
                state.display_text(),
                "iter={i} end={end} text={text:?} sid={sid}: 缓存版与全量版显示文本必须逐字相等"
            );
        }
    }

    /// LOCALRT-PERF-405（DEC-086）源码护栏：`local_stream.rs` **生产区**不得再出现「影子重解」。
    ///
    /// 扫 `local_stream.rs` 自身源码（剔除全部 `#[cfg(test)]` 起至文件尾的测试区），断言无
    /// `shadow`/`SHADOW`/`endpoint_confirm_text` 字样 ⇒ 影子收尾不会被人加回。
    #[test]
    fn guard405_no_shadow_in_production() {
        let src = include_str!("local_stream.rs");
        // 生产区 = 第一个 `#[cfg(test)]` 之前（本文件测试集中在文件后段的 `mod tests`）。
        let prod = src.split("#[cfg(test)]").next().unwrap();
        // 只查**机制标识符**（历史注释里的散文 'shadow' 不算）。
        for needle in [
            "SHADOW_FINALIZE_MS_DEFAULT",
            "SHADOW_MAX_AUDIO_SECS",
            "shadow_trigger_ms",
            "shadow_current",
            "shadow_done_for_pause",
            "shadow_count",
            "endpoint_confirm_text",
        ] {
            assert!(
                !prod.contains(needle),
                "LOCALRT-PERF-405：生产区不得再出现 `{needle}`（影子收尾已按 DEC-086 移除）"
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
    /// 🔴 FIX-GUARD-297：原实现用「首个 `} else`」截断，会命中分支内**嵌套**的 `if … {} else {}`
    /// （如 `endpoint_action` 的判据分支），把真正的 `recognizer.create_stream()` 排除出扫描区 ⇒ G3 假红。
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
    //   三方签名；307 全量重解码与补静音移除后签名收为 `(main, shadow)`；LOCALRT-PERF-405
    //   （DEC-086）影子整体移除后该函数亦删除 ⇒ 断言无对象。
    //   「不得让文本变短或整句消失」的不变量**未失守**：由调用方 `flush_text`（after 空/更短则回落
    //   before）与收尾 `final_preview = last_display.clone()` + `if !final_seg.is_empty()` 结构保证。
    // - ② `tailpad340_local_only_and_order`：守的是 `feed_tail_silence` 调用顺序红线；
    //   该函数已随 340 机制整体删除（阴影收尾每次停顿多解 500ms、收尾多解 2000ms，`[DBG-289]`
    //   13/13 实测零收益），守的对象不存在 ⇒ 一并移除，**不是放宽或删除断言**。
    // ========================================================================

    // ========================================================================
    // PUNCT-PREVIEW-SEMANTIC-349 · 预览打点口径（只认静默 ≥1200ms）
    //   因果取证（真实 CT-Transformer 探针 + 实机日志）见
    //   `collab/evidence/20260922-punct349/causal-evidence.md`（探针按 DEC-077 已删）。
    // ========================================================================

    /// PUNCT-349：预览打点**只认静默 ≥1200ms 且有新内容**。
    /// 改前另有「4s 定时」与「阴影收尾 400ms 强制」两条与语义无关的触发 —— 已删除。
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
            let j = chunk_has_speech(None, &chunk, rms, rms, 100.0, THR, &mut g, &mut Vec::new());
            assert_eq!(
                j.has_speech,
                rms > THR,
                "VAD 不可用 ⇒ 判定必须等于 `chunk_rms > silence_threshold`（amp={amp}）"
            );
            assert_eq!(
                j.vad_speech,
                rms > THR,
                "392 契约变更：VAD 不可用时 vad_speech 取音量兜底判定（⇒ 内容去留与 384 逐位相同）"
            );
        }
        let chunk = vec![THR; 1600];
        assert!(
            !chunk_has_speech(None, &chunk, THR, THR, 100.0, THR, &mut g, &mut Vec::new())
                .has_speech,
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

    /// 测 #1（392 契约变更）：`SegmentPeakLevel` — 样本 <2 未就绪；估计取**下中位数**
    /// （偶数样本取较小者）；按会话时间 30s 过期。
    #[test]
    fn seg389_level_ready_median_and_window() {
        let mut lv = SegmentPeakLevel::with_seed(None);
        assert!(!lv.ready(), "0 样本未就绪");
        lv.push(1.0, 0.0);
        assert!(!lv.ready(), "1 样本仍未就绪（需 ≥2）");
        lv.push(0.5, 10.0);
        assert!(lv.ready());
        // 392：2 样本 [0.5,1.0] 取**下中位数** idx (len-1)/2=0 ⇒ 0.5（较小者）。
        assert!(
            (lv.estimate() - 0.5).abs() < 1e-3,
            "392：偶数样本取下中位（较小者）"
        );
        lv.push(0.2, 20.0);
        // 3 样本 [0.2,0.5,1.0] ⇒ 中位 0.5。
        assert!((lv.estimate() - 0.5).abs() < 1e-3, "奇数样本取中位 0.5");
        // 会话时间过期：now=30001 ⇒ t=0 过期；t=10/20 保留。
        lv.prune(30_001.0);
        assert_eq!(lv.samples.len(), 2, "t=0 应过期");
        assert!(
            (lv.estimate() - 0.2).abs() < 1e-3,
            "剩 [0.2,0.5] 下中位 = 0.2"
        );
    }

    /// 测 #2：段内出现响亮峰值 ⇒ 整句确认；其后**轻音/句尾**全算有声（不再句中切静默）。
    ///
    /// 392 契约变更：本次录音**第一个已确认段不学习** ⇒ 需 **3** 个已确认段才就绪（2 个学习样本）。
    #[test]
    fn seg389_confirms_on_peak_then_holds_through_light_tail() {
        let mut g = SegmentGate::with_seed(None);
        let mut now = 0.0f32;
        // 三段响亮（peak 1.0）：第一段不学习、后两段学习 ⇒ 就绪（未就绪期段直接确认）。
        for _ in 0..3 {
            for _ in 0..3 {
                now += 100.0;
                assert!(g.update(true, 1.0, now));
            }
            now += 100.0;
            assert!(!g.update(false, 0.0, now));
        }
        assert!(g.level.ready(), "392：3 段（首段不学习）后应就绪");
        assert!((g.level.estimate() - 1.0).abs() < 1e-3);
        // 段 4：响亮起始确认，随后轻音尾巴仍算有声。
        now += 100.0;
        assert!(g.update(true, 1.0, now), "响亮起始应确认整句");
        now += 100.0;
        assert!(g.update(true, 0.05, now), "轻音字仍算有声（已确认）");
        now += 100.0;
        assert!(g.update(true, 0.05, now), "句尾/清辅音仍算有声（已确认）");
        now += 100.0;
        assert!(!g.update(false, 0.0, now));
        assert_eq!(g.confirmed, 4);
    }

    /// 测 #3：背景段（整句峰值均 < `level × 0.3`）⇒ 全程 has_speech=false，**不得污染 level**（防自我放行）。
    ///
    /// 392 契约变更：首段不学习 ⇒ 需 3 个已确认段才就绪。
    #[test]
    fn seg389_background_segment_rejected_no_self_admit() {
        let mut g = SegmentGate::with_seed(None);
        let mut now = 0.0f32;
        for _ in 0..3 {
            for _ in 0..3 {
                now += 100.0;
                g.update(true, 1.0, now);
            }
            now += 100.0;
            g.update(false, 0.0, now);
        }
        assert!(g.level.ready(), "392：3 段后应就绪");
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
    ///
    /// 392 契约变更：首段不学习 ⇒ 需 3 个已确认段才就绪。
    #[test]
    fn seg389_window_expires_then_relearns() {
        let mut g = SegmentGate::with_seed(None);
        let mut now = 0.0f32;
        for _ in 0..3 {
            for _ in 0..3 {
                now += 100.0;
                g.update(true, 1.0, now);
            }
            now += 100.0;
            g.update(false, 0.0, now);
        }
        assert!(g.level.ready(), "392：3 段后应就绪");
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
    ///
    /// 392 契约变更：首段不学习 ⇒ 需 3 个已确认段才就绪。
    #[test]
    fn ts389_segment_gate_syllabic_all_speech_background_all_silent() {
        let mut g = SegmentGate::with_seed(None);
        let mut now = 0.0f32;
        // 学三段响亮（peak 1.0；首段不学习）⇒ 就绪。
        for _ in 0..3 {
            for _ in 0..3 {
                now += 100.0;
                g.update(true, 1.0, now);
            }
            now += 100.0;
            g.update(false, 0.0, now);
        }
        assert!(g.level.ready(), "392：3 段后应就绪");
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
    ///
    /// 392 契约变更：首段不学习 ⇒ 需 3 个已确认段（2 学习样本）才 `samples_ready`。
    #[test]
    fn seg389c2_seed_makes_ready_and_used_until_samples_ready() {
        let mut g = SegmentGate::with_seed(Some(1.0));
        assert!(g.level.ready(), "有 seed ⇒ 立即就绪（第一句即按 seed 判）");
        assert!((g.level.estimate() - 1.0).abs() < 1e-3, "estimate=seed");
        assert!(g.level.seed_in_use());
        let mut now = 0.0f32;
        // 第 1 句响亮（1.0 ≥ 0.3×seed）⇒ 确认（392：首段不学习 ⇒ 0 样本）。
        now += 100.0;
        assert!(g.update(true, 1.0, now));
        now += 100.0;
        assert!(!g.update(false, 0.0, now));
        assert!(g.level.seed_in_use(), "392：首段不学习 ⇒ seed 仍参与");
        // 第 2 句响亮 ⇒ 1 学习样本（<2）⇒ seed 仍参与。
        now += 100.0;
        assert!(g.update(true, 1.0, now));
        now += 100.0;
        assert!(!g.update(false, 0.0, now));
        assert!(g.level.seed_in_use(), "本次 1 样本 <2 ⇒ seed 仍参与");
        // 第 3 句响亮 ⇒ 2 学习样本就绪 ⇒ seed 不再参与估计。
        now += 100.0;
        assert!(g.update(true, 1.0, now));
        now += 100.0;
        assert!(!g.update(false, 0.0, now));
        assert!(!g.level.seed_in_use(), "本次样本就绪（2）⇒ seed 不再参与");
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
        for _ in 0..3 {
            now += 100.0;
            g.update(true, 1.0, now);
            now += 100.0;
            g.update(false, 0.0, now);
        }
        assert!(g.level.ready(), "392：3 段（首段不学习）⇒ 就绪 ⇒ 写回");
    }

    // ========================================================================
    // TEST-SYNC-389（阶段三 · 非作者护栏 · coder-1）：SegmentGate / SegmentPeakLevel / seed
    //   契约见任务书；不与作者 `ts389_*` / `seg389c2_*` 重复。
    // ========================================================================

    /// 学习三段峰值 1.0（392 契约变更：首段不学习）⇒ 就绪、estimate=1.0（返回门，便于后续用例复用）。
    fn ts389n_ready_level_1() -> SegmentGate {
        let mut g = SegmentGate::with_seed(None);
        let mut now = 0.0f32;
        for _ in 0..3 {
            now += 100.0;
            g.update(true, 1.0, now);
            now += 100.0;
            g.update(false, 0.0, now);
        }
        assert!(g.level.ready() && (g.level.estimate() - 1.0).abs() < 1e-3);
        g
    }

    /// 测 #1 整句不切：就绪 level=1.0，段内序列 [0.8, 0.05×10, 0.7, 0.02×20]（VAD 全真）
    /// ⇒ **第一个高值后到段结束全部有声**（轻音/句尾近零都不切）。
    #[test]
    fn ts389n_whole_sentence_not_cut_after_first_peak() {
        let mut g = ts389n_ready_level_1();
        let mut now = 0.0f32;
        let seq = std::iter::once(0.8f32)
            .chain(std::iter::repeat(0.05).take(10))
            .chain(std::iter::once(0.7))
            .chain(std::iter::repeat(0.02).take(20));
        let mut n = 0usize;
        for sm in seq {
            now += 10.0;
            assert!(g.update(true, sm, now), "第 n={n} 帧（sm={sm}）应为有声");
            n += 1;
        }
        assert_eq!(n, 32, "序列长度 1+10+1+20");
        now += 10.0;
        assert!(!g.update(false, 0.0, now), "段结束后应静默");
    }

    /// 测 #2 背景整句挡住：峰值恒 0.2 < 0.3×1.0 ⇒ 全 false，且该段峰值**不进学习样本**。
    #[test]
    fn ts389n_background_sentence_all_silent_and_peak_not_learned() {
        let mut g = ts389n_ready_level_1();
        let mut now = 0.0f32;
        let samples_before = g.level.samples.len();
        let rejected_before = g.rejected;
        for _ in 0..50 {
            now += 10.0;
            assert!(!g.update(true, 0.2, now), "背景 0.2 应判静默");
        }
        now += 10.0;
        g.update(false, 0.0, now);
        assert_eq!(
            g.level.samples.len(),
            samples_before,
            "背景段峰值不得进学习样本"
        );
        assert_eq!(g.rejected, rejected_before + 1, "背景段应计为被拒");
    }

    /// 测 #3 防锁死：学到 1.0 后录音人整句 0.25（<0.3）连续 35s（逐段结束）：
    /// 30s 内样本未过期 ⇒ 每段被拒；样本按**会话时间**过期后回到未就绪 ⇒ 整句确认并重学 ≈0.25。
    #[test]
    fn ts389n_lock_recovery_after_sample_expiry() {
        let mut g = ts389n_ready_level_1();
        let mut now = 0.0f32;
        let seg_start = now;
        let mut rejected_early = 0usize;
        let mut whole_confirmed_seen = 0usize;
        while now - seg_start < 35_000.0 {
            now += 100.0;
            let sp = g.update(true, 0.25, now);
            if sp {
                whole_confirmed_seen += 1;
            } else if now - seg_start < 29_000.0 {
                rejected_early += 1;
            }
            now += 100.0;
            g.update(false, 0.0, now);
        }
        assert!(rejected_early > 0, "30s 内样本未过期 ⇒ 0.25 段应被拒");
        assert!(whole_confirmed_seen > 0, "样本过期后应回未就绪 ⇒ 整句确认");
        assert!(g.level.samples_ready(), "重学后样本应就绪");
        assert!(
            (g.level.estimate() - 0.25).abs() < 1e-3,
            "应重学到 ≈0.25，实测 {:.4}",
            g.level.estimate()
        );
    }

    /// 测 #4 seed 流程：seed=1.0 第一句峰值 0.5（≥0.3×seed）⇒ 确认；
    /// seed=1.0 连续两句 0.1 ⇒ `seed_dropped`、第三句未就绪直接整句确认；
    /// `seed_usable`：同设备 600s true、601s / 换设备 false。
    #[test]
    fn ts389n_seed_flow_and_usability_boundary() {
        // (a) seed=1.0 ⇒ 第一句 0.5 确认、seed 不丢
        let mut g = SegmentGate::with_seed(Some(1.0));
        let mut now = 0.0f32;
        now += 100.0;
        assert!(g.update(true, 0.5, now), "0.5 ≥ 0.3×seed ⇒ 确认");
        now += 100.0;
        g.update(false, 0.0, now);
        assert!(!g.seed_dropped, "确认句不得丢 seed");

        // (b) seed=1.0 ⇒ 两句 0.1 被拒 ⇒ seed_dropped；第三句直接确认
        let mut g2 = SegmentGate::with_seed(Some(1.0));
        let mut t = 0.0f32;
        for _ in 0..2 {
            t += 100.0;
            assert!(!g2.update(true, 0.1, t), "0.1 < 0.3×seed ⇒ 静默");
            t += 100.0;
            g2.update(false, 0.0, t);
        }
        assert!(g2.seed_dropped, "连续两句被拒 ⇒ seed_dropped");
        assert!(!g2.level.ready(), "丢 seed 后回到未就绪");
        t += 100.0;
        assert!(g2.update(true, 0.05, t), "未就绪 ⇒ 第三句直接整句确认");

        // (c) seed_usable 边界
        assert!(
            seed_usable("mic", Duration::from_secs(600), "mic"),
            "600s ⇒ true"
        );
        assert!(
            !seed_usable("mic", Duration::from_secs(601), "mic"),
            "601s ⇒ false"
        );
        assert!(
            !seed_usable("mic", Duration::from_secs(1), "other"),
            "换设备 ⇒ false"
        );
    }
}

// ========================================================================
// GATE-TIMING-ONLY-392：门只管时序、内容去留只看 VAD；首段不学习；下中位数
// ========================================================================
#[cfg(test)]
mod gate392_tests {
    use super::{
        chunk_has_speech, localrt_vad_seed_ms, should_dispatch_acc, SegmentGate, SegmentPeakLevel,
    };

    /// 1. 复现 BUILD-390 峰值序列 [0.0966, 0.0215, 0.0157, 0.0228, 0.0244]（VAD 全真）：
    ///    **第一段不学习**（0 样本），后续段**全部确认**；level = 后 4 段下中位 = 0.0215。
    #[test]
    fn gate392_repro_peak_sequence_first_not_learned_all_confirmed() {
        let mut g = SegmentGate::with_seed(None);
        let mut now = 0.0f32;
        let peaks = [0.0966f32, 0.0215, 0.0157, 0.0228, 0.0244];
        let mut samples_after = Vec::new();
        for (i, &p) in peaks.iter().enumerate() {
            for _ in 0..3 {
                now += 100.0;
                assert!(g.update(true, p, now), "段 {i}（peak={p}）应确认整句");
            }
            now += 100.0;
            assert!(!g.update(false, 0.0, now));
            samples_after.push(g.level.samples.len());
        }
        assert_eq!(samples_after[0], 0, "392：第一段不学习");
        assert_eq!(samples_after[1], 1);
        assert_eq!(samples_after[4], 4);
        assert_eq!(g.confirmed, 5, "后续段全部确认");
        assert_eq!(g.rejected, 0, "无段被误判为背景");
        let want = 0.0215f32; // sorted [0.0157,0.0215,0.0228,0.0244] 下中位 idx1
        assert!(
            (g.level.estimate() - want).abs() < 1e-3,
            "392：下中位应为 0.0215，实测 {}",
            g.level.estimate()
        );
    }

    /// 2. 门拒绝的段（背景）⇒ **内容标志仍置位**（不丢录音人内容），**时序照常累加**。
    ///    复刻生产三分支 + 内容块（396 契约）。并源码级护栏：内容标志挂在 `if vad_speech` 下。
    #[test]
    fn gate392_gated_segment_sets_content_flags_and_accumulates_timing() {
        let step = |has_speech: bool,
                    vad_speech: bool,
                    prev_has_speech: bool,
                    vad_on: bool,
                    silent0: f32,
                    sfr0: bool,
                    aph0: bool|
         -> (f32, bool, bool) {
            let mut silent = silent0;
            if !has_speech {
                if vad_on && prev_has_speech && !vad_speech {
                    silent = localrt_vad_seed_ms();
                } else {
                    silent += 100.0;
                }
            } else {
                silent = 0.0;
            }
            let (mut sfr, mut aph) = (sfr0, aph0);
            if vad_speech {
                sfr = true;
                aph = true;
            }
            (silent, sfr, aph)
        };
        // 门拒但 VAD 真（has_speech=false, vad_speech=true）：内容标志置位、时序累加。
        let (mut silent, mut sfr, mut aph) = (0.0f32, false, false);
        for _ in 0..5 {
            let (s, a, b) = step(false, true, false, true, silent, sfr, aph);
            silent = s;
            sfr = a;
            aph = b;
        }
        assert!(
            sfr,
            "392：门拒段仍应置 speech_since_last_reset（342 文本不丢）"
        );
        assert!(
            aph,
            "392：门拒段仍应置 acc_pending_has_speech（要派发解码）"
        );
        assert_eq!(silent, 500.0, "时序照常累加 5×100ms");
        // VAD 也判静默 ⇒ 内容标志不置、时序累加。
        let (s2, sfr2, aph2) = step(false, false, false, true, 0.0, false, false);
        assert!(!sfr2 && !aph2, "VAD 无人的段不置内容标志");
        assert_eq!(s2, 100.0);
        // 过门有声 ⇒ 时序清零。
        let (s3, _, _) = step(true, true, false, true, 999.0, true, true);
        assert_eq!(s3, 0.0, "过门有声 ⇒ 时序清零");

        // 源码级护栏：内容标志必须挂在 `if vad_speech {` 下（不是 `if has_speech`）。
        let src = include_str!("local_stream.rs");
        let body = src
            .split("pub fn transcribe_streaming_local(")
            .nth(1)
            .expect("transcribe_streaming_local 锚点缺失");
        let lines: Vec<&str> = body.lines().collect();
        let sfr_line = lines
            .iter()
            .position(|l| l.trim() == "speech_since_last_reset = true;")
            .expect("speech_since_last_reset 锚点缺失");
        let nearest_if = lines[..sfr_line]
            .iter()
            .rposition(|l| l.trim_start().starts_with("if "))
            .expect("内容块应有 enclosing if");
        assert!(
            lines[nearest_if]
                .trim_start()
                .starts_with("if vad_speech {"),
            "392：内容标志必须挂在 `if vad_speech {{` 下，实测 enclosing={:?}",
            lines[nearest_if]
        );
    }

    /// 392 主控验收补：「本轮停顿已派发」`acc_done_for_pause` 的复位属于**时序**，必须在
    /// `if !has_speech { … } else { <这里> }` 的 else 分支（过门有声），**不得**挂在 `if vad_speech {` 下。
    /// 否则背景人声被门拒时 acc_silent_ms 持续 ≥1200ms 而 done 每 chunk 被清 ⇒ 每 ~10ms 派发一次。
    #[test]
    fn gate392_review_done_reset_is_timing_not_content() {
        let src = include_str!("local_stream.rs");
        let body = src
            .split("pub fn transcribe_streaming_local(")
            .nth(1)
            .expect("transcribe_streaming_local 锚点缺失");
        let lines: Vec<&str> = body.lines().collect();
        let done = lines
            .iter()
            .position(|l| l.trim() == "acc_done_for_pause = false;")
            .expect("acc_done_for_pause 复位锚点缺失");
        let nearest_branch = lines[..done]
            .iter()
            .rposition(|l| {
                let t = l.trim_start();
                t.starts_with("if ") || t.starts_with("} else")
            })
            .expect("复位应有 enclosing 分支");
        assert_eq!(
            lines[nearest_branch].trim(),
            "} else {",
            "done 复位必须在 `if !has_speech` 的 else（过门有声）分支，实测 {:?}",
            lines[nearest_branch]
        );
        let enclosing_if = lines[..nearest_branch]
            .iter()
            .rposition(|l| l.trim_start().starts_with("if !has_speech {"))
            .expect("应在 `if !has_speech {` 之后");
        assert!(enclosing_if < nearest_branch);
        // 背景人声（门拒、VAD 真）连续 5s：派发判据只在静默首次满 1200ms 时成立一次。
        let mut acc_silent = 0.0f32;
        let mut done = false;
        let mut pending_speech = false;
        let mut dispatches = 0;
        for _ in 0..500 {
            let (has_speech, vad_speech) = (false, true);
            if !has_speech {
                acc_silent += 10.0;
            } else {
                acc_silent = 0.0;
                done = false;
            }
            if vad_speech {
                pending_speech = true;
            }
            if super::should_dispatch_acc(true, acc_silent, 160, done, pending_speech, 1200.0) {
                dispatches += 1;
                done = true;
                pending_speech = false;
            }
        }
        assert_eq!(
            dispatches, 1,
            "背景人声持续 5s 只应派发一次，实测 {dispatches}"
        );
    }

    /// 3. `estimate`：偶数样本取**下中位数**（两样本取较小者）；奇数样本取中位。
    #[test]
    fn gate392_estimate_lower_median_even_middle_odd() {
        let mut lv = SegmentPeakLevel::with_seed(None);
        lv.push(0.9, 0.0);
        lv.push(0.1, 1.0);
        assert!(
            (lv.estimate() - 0.1).abs() < 1e-3,
            "392：两样本取下中位（较小者）"
        );
        lv.push(0.5, 2.0);
        assert!((lv.estimate() - 0.5).abs() < 1e-3, "奇数样本取中位 0.5");
    }

    /// 4. VAD 不可用 ⇒ `vad_speech == has_speech`（音量兜底判定）⇒ 内容去留与 384 逐位一致。
    #[test]
    fn gate392_vad_unavailable_content_flags_match_384() {
        let mut g = SegmentGate::with_seed(None);
        const THR: f32 = 0.01;
        for rms in [0.0f32, 0.005, THR, 0.02, 0.5] {
            let chunk = vec![rms; 160];
            let j = chunk_has_speech(None, &chunk, rms, rms, 0.0, THR, &mut g, &mut Vec::new());
            assert_eq!(j.has_speech, rms > THR);
            assert_eq!(
                j.vad_speech, j.has_speech,
                "392：VAD 不可用 ⇒ vad_speech 必须等于 has_speech（内容去留与 384 逐位相同）"
            );
        }
    }

    // ========================================================================
    // TEST-SYNC-392（阶段三 · 非作者护栏 · coder-1）：门只管时序、内容只看 VAD
    //   契约见任务书；不与作者 392 用例重复。
    // ========================================================================

    /// 392 契约状态机（测试内复刻生产标志更新 `:1046-1090` + 派发/复位 `:1432-1472`）。
    struct Flags392 {
        silent_ms: f32,
        acc_silent_ms: f32,
        acc_done_for_pause: bool,
        speech_since_last_reset: bool,
        acc_pending_has_speech: bool,
        prev_has_speech: bool,
        dispatches: usize,
    }
    impl Flags392 {
        fn new() -> Self {
            Self {
                silent_ms: 0.0,
                acc_silent_ms: 0.0,
                acc_done_for_pause: false,
                speech_since_last_reset: false,
                acc_pending_has_speech: false,
                prev_has_speech: false,
                dispatches: 0,
            }
        }
        /// 推进一帧。`vad_on` = VAD 可用；`silence_ms` = 派发静音阈值(1200)。
        fn step(
            &mut self,
            has_speech: bool,
            vad_speech: bool,
            chunk_ms: f32,
            vad_on: bool,
            silence_ms: f32,
            pending_samples: usize,
        ) {
            // 时序（has_speech）
            if !has_speech {
                if vad_on && self.prev_has_speech && !vad_speech {
                    self.silent_ms = localrt_vad_seed_ms();
                    self.acc_silent_ms = localrt_vad_seed_ms();
                } else {
                    self.silent_ms += chunk_ms;
                    self.acc_silent_ms += chunk_ms;
                }
            } else {
                self.silent_ms = 0.0;
                self.acc_silent_ms = 0.0;
                self.acc_done_for_pause = false;
            }
            // 内容（vad_speech）
            if vad_speech {
                self.speech_since_last_reset = true;
                self.acc_pending_has_speech = true;
            }
            // 派发（should_dispatch_acc + 上 latch）
            if should_dispatch_acc(
                true,
                self.acc_silent_ms,
                pending_samples,
                self.acc_done_for_pause,
                self.acc_pending_has_speech,
                silence_ms,
            ) {
                self.dispatches += 1;
                self.acc_pending_has_speech = false;
                self.acc_done_for_pause = true;
            }
            self.prev_has_speech = has_speech;
        }
    }

    /// 测 #1（392）：门误判（has_speech 恒 false、vad_speech 真）一整句 ⇒
    /// 内容标志 `speech_since_last_reset` / `acc_pending_has_speech` 被置位；
    /// 其后静默满 1200ms ⇒ `should_dispatch_acc` 成立**恰一次**（不丢录音人内容、不碎片化）。
    #[test]
    fn ts392n_gate_misjudge_keeps_content_and_dispatches_once() {
        let mut f = Flags392::new();
        // 一整句 0.8s：门恒拒（has_speech=false），VAD 真。
        for _ in 0..80 {
            f.step(false, true, 10.0, true, 1200.0, 16000);
        }
        assert!(f.acc_pending_has_speech, "内容：门误判也不得丢录音人话");
        assert!(f.speech_since_last_reset, "内容标志须置位");
        // 随后 2s 静默（VAD 转假）。
        for _ in 0..200 {
            f.step(false, false, 10.0, true, 1200.0, 16000);
        }
        assert_eq!(f.dispatches, 1, "全程恰派发一次（内容不丢、也不碎片化）");
    }

    /// 测 #2（392）：背景人声（门拒、VAD 真）持续 10s + 录音人开口 2s + 停顿 1.5s ⇒
    /// `should_dispatch_acc` 全程**恰 2 次**（背景一次、录音人一次）。
    #[test]
    fn ts392n_background_then_person_dispatches_exactly_twice() {
        let mut f = Flags392::new();
        for _ in 0..1000 {
            f.step(false, true, 10.0, true, 1200.0, 16000); // 背景 10s
        }
        assert_eq!(f.dispatches, 1, "背景只随一次派发（done latch 生效）");
        for _ in 0..200 {
            f.step(true, true, 10.0, true, 1200.0, 16000); // 录音人 2s
        }
        for _ in 0..150 {
            f.step(false, false, 10.0, true, 1200.0, 16000); // 停顿 1.5s
        }
        assert_eq!(f.dispatches, 2, "录音人一次 + 背景一次 = 恰 2 次");
    }

    /// 测 #3（392）：`SegmentPeakLevel` 首个确认段不学习 + 偶数样本取**下中位**。
    #[test]
    fn ts392n_peak_level_first_skipped_lower_median() {
        let mut lv = SegmentPeakLevel::with_seed(None);
        // [0.09(首段=按键高峰), 0.02, 0.02, 0.025]：首段不学。
        assert!(!lv.learn_segment(0.09, 0.0), "首个确认段不得学习");
        assert!(lv.learn_segment(0.02, 100.0));
        assert!(lv.learn_segment(0.02, 200.0));
        assert!(lv.learn_segment(0.025, 300.0));
        assert_eq!(lv.samples.len(), 3, "首个 0.09 未进样本");
        assert!(
            (lv.estimate() - 0.02).abs() < 1e-6,
            "三样本取下中位 = 0.02，实测 {}",
            lv.estimate()
        );
        // 偶数样本取下中位：push [0.3, 0.1] ⇒ 0.1。
        let mut lv2 = SegmentPeakLevel::with_seed(None);
        lv2.push(0.3, 0.0);
        lv2.push(0.1, 100.0);
        assert!(
            (lv2.estimate() - 0.1).abs() < 1e-6,
            "偶数样本取下中位 = 0.1，实测 {}",
            lv2.estimate()
        );
    }

    /// 测 #4（392）：VAD 不可用 ⇒ `chunk_has_speech(None)` 的 `has_speech == vad_speech == rms>thr`；
    /// 随机 300 组「内容/时序标志更新」与「均取 rms>thr」**逐位一致**（与 384 同）。
    #[test]
    fn ts392n_vad_unavailable_flags_bit_identical_to_energy() {
        struct Lcg(u64);
        impl Lcg {
            fn next(&mut self) -> u64 {
                self.0 = self
                    .0
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                self.0
            }
        }
        let mut rng = Lcg(0x392_5eed);
        let mut gate = SegmentGate::with_seed(None);
        let mut fa = Flags392::new(); // 用 chunk_has_speech(None) 的判定
        let mut fb = Flags392::new(); // 参考：两者皆取 rms>thr
        for k in 0..300usize {
            let rms = (rng.next() % 2000) as f32 / 10_000.0;
            let thr = (rng.next() % 2000) as f32 / 10_000.0;
            let chunk: &[f32] = &[];
            let j = chunk_has_speech(
                None,
                chunk,
                rms,
                rms,
                k as f32 * 10.0,
                thr,
                &mut gate,
                &mut Vec::new(),
            );
            let e = rms > thr;
            assert_eq!(j.has_speech, e, "k={k}: 兜底 has_speech == rms>thr");
            assert_eq!(j.vad_speech, e, "k={k}: 兜底 vad_speech == rms>thr");
            assert_eq!(j.has_speech, j.vad_speech, "兜底两标志必须相等");
            fa.step(j.has_speech, j.vad_speech, 10.0, false, 1200.0, 16000);
            fb.step(e, e, 10.0, false, 1200.0, 16000);
            assert_eq!(fa.silent_ms, fb.silent_ms, "k={k}: silent_ms 逐位一致");
            assert_eq!(fa.acc_silent_ms, fb.acc_silent_ms, "k={k}: acc_silent_ms");
            assert_eq!(
                fa.speech_since_last_reset, fb.speech_since_last_reset,
                "k={k}: speech_since_last_reset"
            );
            assert_eq!(
                fa.acc_pending_has_speech, fb.acc_pending_has_speech,
                "k={k}: acc_pending_has_speech"
            );
            assert_eq!(
                fa.acc_done_for_pause, fb.acc_done_for_pause,
                "k={k}: acc_done_for_pause"
            );
            assert_eq!(fa.dispatches, fb.dispatches, "k={k}: 派发次数");
        }
    }
}
