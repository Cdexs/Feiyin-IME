//! LOCAL-RT-ENGINE-239-A / DEC-067 / STREAM-SV-463：本地实时预览 ASR。
//!
//! 与 [`crate::transcription::qwen_inference::transcribe_streaming_realtime`] **平行**的本地实现：
//! 不走 WebSocket，直接处理实时音频 chunk。
//!
//! STREAM-SV-463（Gavin 2026-10-03「好，那替换吧」，DEC-100）：预览模型由三语流式 paraformer
//! （不支持日韩）换成官方 SenseVoice-Small 2024-07-17（中英日韩粤）**模拟流式** ——
//! 说话中每 [`SIM_STEP_MS`] 把「当前句」整段重解一次（非流式模型，靠重解出增量）。
//! - 当前句 = `pcm[sim_start..]`；句首静音不入窗（开口前只留 [`ONGOING_ONSET_MARGIN`]）。
//! - 🔴 **每个派发点都是冻结点**：派发前当前句结果确认进显示缓存、此后不再改写 ——
//!   下游按「字符坐标」回灌（325/337/438/442）的前提是派发点之前的预览文字不变。
//!   另两处冻结：句末静默 [`SIM_SENTENCE_END_MS`]（精解关闭时的切句）、窗口硬上限 [`SIM_WINDOW_MAX_MS`]。
//! - 预览末尾「相邻两次重解不一致」的字报给显示层画浅色（`on_result` 第三参），已定的字正常色。
//!
//! 其余设计（对齐主控方案）：
//! - 复用 [`qwen_inference::StreamingAsrState`] 做「确认句 + 当前句」状态机，**不另写状态机**。
//! - 文本或浅色尾巴变化才回调，避免无谓刷新。
//! - 返回 `(final_text, pcm)`：`pcm` 为本次全部音频（16kHz f32），供 2pass 离线纠错复用。
//! - `cancel_signal` 置位即提前收尾（`Relaxed` 读，与 qwen 路径同款）。

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use sherpa_onnx::{
    OfflineModelConfig, OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
};

use super::qwen_inference::{StreamingAsrState, WordTiming};
use super::vad::{VadSegmenter, LOCALRT_VAD_MIN_SILENCE_SECS};
use crate::punctuation::{strip_trailing_punctuation, PunctuationEngine};

/// 本地预览采样率（SenseVoice 固定 16kHz 单声道）。
const SAMPLE_RATE: i32 = 16000;

/// LOCAL-RT-ENGINE-239-B：把预览 recognizer 引用送进 ASR 工作线程的 Send 包装。
///
/// SAFETY：与 `Transcriber` 的 `unsafe impl Send` 同源论证——sherpa-onnx C++ recognizer
/// 的 create/decode/destroy 可在不同线程调用，只需保证**同一实例不被并发访问**。
/// 本包装仅在 `std::thread::scope` 内把引用 move 给 ASR 线程；该线程运行期间，持有者
/// （worker 线程）阻塞在 `record_streaming`，不触碰 recognizer；ASR 线程 join 之后才继续
/// 使用（run_pipeline_core 走 offline_recognizer，对象不同且串行）。故无并发访问。
pub struct SendPreviewRecognizerRef<'a>(pub &'a OfflineRecognizer);
unsafe impl<'a> Send for SendPreviewRecognizerRef<'a> {}

impl<'a> SendPreviewRecognizerRef<'a> {
    /// 取出内部引用。**按值消费包装**，使跨线程闭包必须捕获整个 Send 包装
    /// （Rust 2021 disjoint capture 下若直接取 `.0` 字段会退化为捕获裸引用，
    /// 绕过 `unsafe impl Send`，故以方法消费强制整体捕获）。
    pub fn into_inner(self) -> &'a OfflineRecognizer {
        self.0
    }
}

/// STREAM-SV-463：预览解码线程数 = **本机物理核数**（Gavin 2026-10-03「开成跟现在 CPU 内核数一样」）。
///
/// 实测（`poc462_simstream`，7840HS 8 核 16 线程，证据 `collab/evidence/463/simstream463.md`）：
/// 单次重解最慢 1 线程 407ms / **8 线程 173ms** / 16 线程 190ms，20s 长句 910 / **329** / 437ms ——
/// 物理核最快，超线程反而变慢，故取物理核不取逻辑核。
///
/// 沿革：paraformer 时代按 `available_parallelism().min(4)`（Gavin 2026-09-25 封顶 4，
/// LOCALRT-STREAM-THREADS-417）；换 SenseVoice 后由 Gavin 改为跟核数一致。
fn local_stream_num_threads() -> i32 {
    physical_core_count() as i32
}

/// STREAM-SV-463：本机物理核数；系统查询失败 ⇒ 回落逻辑核数（再失败 ⇒ 4）。
fn physical_core_count() -> usize {
    #[cfg(target_os = "windows")]
    if let Some(n) = win_physical_cores() {
        return n;
    }
    #[cfg(target_os = "macos")]
    if let Some(n) = mac_physical_cores() {
        return n;
    }
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// Windows：`GetLogicalProcessorInformationEx(RelationProcessorCore)` 每条记录 = 一个物理核。
#[cfg(target_os = "windows")]
fn win_physical_cores() -> Option<usize> {
    use windows::Win32::System::SystemInformation::{
        GetLogicalProcessorInformationEx, RelationProcessorCore,
        SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
    };
    let mut len = 0u32;
    // 第一次调用只取所需缓冲长度（必然报 ERROR_INSUFFICIENT_BUFFER，忽略）。
    unsafe {
        let _ = GetLogicalProcessorInformationEx(RelationProcessorCore, None, &mut len);
    }
    if len == 0 {
        return None;
    }
    // u64 缓冲保证 8 字节对齐；记录头按字节读（Relationship u32 + Size u32）。
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    unsafe {
        GetLogicalProcessorInformationEx(
            RelationProcessorCore,
            Some(buf.as_mut_ptr() as *mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX),
            &mut len,
        )
        .ok()?;
    }
    let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, len as usize) };
    let (mut off, mut n) = (0usize, 0usize);
    while off + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[off + 4..off + 8].try_into().ok()?) as usize;
        if size == 0 {
            break;
        }
        n += 1;
        off += size;
    }
    (n > 0).then_some(n)
}

/// macOS：`sysctlbyname("hw.physicalcpu")`。
#[cfg(target_os = "macos")]
fn mac_physical_cores() -> Option<usize> {
    let mut v: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    let r = unsafe {
        libc::sysctlbyname(
            c"hw.physicalcpu".as_ptr(),
            &mut v as *mut libc::c_int as *mut libc::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (r == 0 && v > 0).then_some(v as usize)
}

/// STREAM-SV-463：说话中重解当前句的间隔（原型测定 0.6s：首字 0.43s、单核约 0.2 实时率）。
const SIM_STEP_MS: f32 = 600.0;
/// STREAM-SV-463：开口后首次重解的等待（实时 VAD 判出开口比真实起点晚约 0.1s ⇒ 首解约在开口后 0.4s，
/// 与原型「段起点 0.6s 首解、段起点含 0.2s 前留白」同口径）。
const SIM_FIRST_MS: f32 = 300.0;
/// STREAM-SV-463：句末冻结 —— 当前句有人声且其后连续无人声（VAD）达此时长。
/// 精解开启时派发（静默 1200ms）先冻结，本条只在精解关闭时起作用；取原 rule2=2.0s 同值（ASR-SEG-229）。
const SIM_SENTENCE_END_MS: f32 = 2000.0;
/// STREAM-SV-463：重解窗口硬上限。精解开启时 442 回看最晚 12s 切片（切片即冻结），本条是兜底：
/// 单次重解随长度超线性增长（8 线程 20s 329ms、30s 592ms），超过此长度在上次重解覆盖处冻结。
const SIM_WINDOW_MAX_MS: f32 = 15000.0;

/// PREVIEW-PUNCT-LIVE-438：预览打点的**间隔**阈值（Gavin 2026-09-26 拍板 3.5s）。
///
/// 沿革：269-B 静默 800ms 打点 + 269 的 4s 定时 + 284 shadow 400ms 强制 —— **三个触发都与语义无关**，
/// 会在用户还没说完时对「半句」打点；CT-Transformer 对未完成文本必在**末尾补终止符**，
/// 该终止符经 `punct_cache_reuse` 复用后落到句中（因果取证见
/// `collab/evidence/20260922-punct349/causal-evidence.md`）⇒ Gavin 报「标点打在句子中间，掐断句意」。
///
/// ⇒ 349 改成**只认静默 ≥ 1200ms**（原 `PUNCT_SILENCE_TRIGGER_MS`，与 346 同一口径）。
/// ⇒ 438 改成**单一的间隔触发**（Gavin：「隔 3.5 秒…预览窗口上只保留一个就好了」）：
///    距上次打点 ≥3.5s 且有新字 ⇒ 重打一次；1200ms 静默触发连同显示层 `silent_ms` 计数器
///    **一并删除**（DEC-077 不留死代码）。间隔计时 `punct_interval_ms` 与语音/静默无关，
///    按流式线程每 chunk 的时间推进无条件累加，仅在**实际完成一次打点**时归零。
///
/// 🔴 **只打「精解尚未覆盖的尾巴」（补充 R1，主控定案）**：最近一次派发的 `committed_len`
/// 边界之前的显示文本**冻结不重打**，3.5s 只重打边界之后的裸尾巴 ⇒ 冻结前缀逐字不变、
/// `streaming[committed_len..]` 坐标不因标点增减漂移，也不与精解标点打架。
///
/// 🔴 **与切句完全独立**：本阈值只影响**显示层**刷新（判早了只是标点早出现），
/// 绝不切句 / 不改 `sentence_id`。两者是不同层的东西，不要混用。
///
/// PREVIEW-PUNCT-464（Gavin 2026-10-03「能不能前端的这个模拟的流式预览带标点符号？这样看起来体验更好」，
/// 方案确认「按你的方案来」）：间隔 3.5s → **0（预览文字一变就打）**。模拟流式下文字只在每次重解
///（≥0.6s 一次）后变化，`has_new` 门让引擎每次重解至多跑一次；实测末尾未打点平均 40 字 → 21 字，
/// 56s 录音总耗时 +5%（证据 `collab/evidence/464/preview-punct.md`）。349「句末终止符不入缓存」照旧；
/// 停顿派发 / 录音结束时另补句末标点（`preview_display` 的 `sentence_end`）。
const PUNCT_PREVIEW_INTERVAL_MS: f32 = 0.0;

/// LOCALRT-PARALLEL-ACC-298 / ACC-DISPATCH-SILENCE-ONLY-346：派发静音阈值默认值。
///
/// 沿革：298 初版 800ms（Gavin 拍板，不是 400ms）→ **ACC-DISPATCH-SILENCE-ONLY-346 Gavin
/// 2026-09-22 改为 1200ms，且「只判断静默，不按时长来切片」**。原「或 未派发累计 ≥5s（长度支）」
/// 已按 DEC-077（零收益机制即移除）连根删除，常量 `ACC_MIN_SEGMENT_MS_DEFAULT` 一并移除。
///
/// 🔴 **与 sherpa endpoint（rule2=2.0s）完全解耦**：本阈值只管「把已说完的一段派给 accuracy
/// 并行转写」，**绝不触发** `reset()` / 切句 / `sentence_id += 1`（那是显示层的事）。
///
/// 🔴 **本阈值必须读 acc 专用计数器 `acc_silent_ms`，不得读任何会被打点动作影响的计数器**：
/// 346 落地时标点阈值 `PUNCT_SILENCE_TRIGGER_MS=800` < 本阈值 1200，而显示层打点会把
/// 它自己的 `silent_ms` 清零（当年的 `if silence_due { silent_ms = 0.0; }`）⇒ 若读共享
/// `silent_ms`，静默每到 800ms 就被标点路径归零，**1200ms 永远到不了**，叠加删长度支后
/// accuracy 在录音中**一次都不会被派发**（298/342-D 整个废掉）。
///
/// PUNCT-PREVIEW-SEMANTIC-349 起标点阈值也抬到 **1200ms**，与本阈值相等：此时读共享计数器
/// 「恰好」也能工作（acc 检查在标点清零之前，同一帧先派后清），但**依赖两条阈值恒相等 +
/// 循环内检查顺序**，是脆弱耦合。独立计数器让两个消费者互不影响（与 acc 自身 latch
/// 同构），故**保留** —— 阈值今后各自调整都不会互相踩踏。
///
/// PREVIEW-PUNCT-LIVE-438：显示层 `silent_ms` 已随 1200ms 打点触发**整体删除**，打点改为
/// 只读自己的间隔计数器 `punct_interval_ms` ⇒ 「标点清零静默计数」这一耦合从结构上消失，
/// 本计数器的独立性由 `guard346_acc_counter_wiring` 源码级护栏继续钉死。
const ACC_DISPATCH_SILENCE_MS_DEFAULT: f32 = 1200.0;

/// LOCALRT-SEAM-337：自适应定界的「文本停止增长」窗口（具名，非魔数）。
/// 依据：流式模型按 `config.yaml: chunk_length: 500`（ms）+ `chunk_shift_ratio: 0.5`（步进 250ms）
/// 分块处理 ⇒ **一个整块内无任何新输出**即可判「滞后补字已吐完」。
const ACC_BOUNDARY_STABLE_MS: f32 = 500.0;

/// LOCALRT-TAIL-WINDOW-407：长静默（不依赖停止键）触发「末尾组窗」的阈值。
///
/// Gavin 2026-09-24 原话：「不用等用户按停止键，我们等到 **1900ms 无输入**自动触发末尾组窗，
/// 将末尾一片和前一片（2s+）组窗来解码」「现在有两种静默间隔触发组窗：常规 1200ms 触发常规派发，
/// 长时静默 1900ms 触发末尾组窗机制」。二者并存：1200ms 已把片派出，1900ms 再让 main 侧据
/// 遗留的**待覆盖片**组「末尾窗」并即时回灌。仅本地实时流式路B 使用。
pub(crate) const LONG_SILENCE_TAIL_MS: f32 = 1900.0;

/// LOCALRT-TAIL-WINDOW-407：是否发「长静默」信号 —— 启用 acc 派发、静默 ≥1900ms、且本段静默未发过。
/// 纯函数，便于单测「只发一次 / 1200~1900 不发 / 恢复说话后复位」。
fn should_signal_long_silence(enabled: bool, acc_silent_ms: f32, done_for_pause: bool) -> bool {
    enabled && acc_silent_ms >= LONG_SILENCE_TAIL_MS && !done_for_pause
}

/// TAIL-CUT-REAL-PAUSE-434：前一片后缀切点的**三态**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TailCutKind {
    /// 切在**真停顿**（连续 ≥ [`TAIL_CUT_MIN_PAUSE_MS`] 低能量帧）的中点。
    RealPause,
    /// 只有单帧能量低谷（旧 407 判据）⇒ 沿用旧切点（保守，字内低谷也可能命中）。
    WeakGap,
    /// 区间内无任何字缝 ⇒ 回落 `len-back` 按时长（旧行为）。
    NoGap,
}

/// TAIL-CUT-REAL-PAUSE-434：真停顿最短时长（ms）。Gavin 2026-09-25「切点只切在真正的停顿处」
/// ⇒ 要求**连续 ≥120ms（6×20ms 帧）**低能量，排除「获取」韵母尾这类字内单帧低谷。
const TAIL_CUT_MIN_PAUSE_MS: u32 = 120;
/// TAIL-CUT-REAL-PAUSE-434：首轮区间找不到真停顿时**往前扩大**搜索的倍数（最多回溯 2×back）。
const TAIL_CUT_EXPAND_FACTOR: usize = 2;

/// LOCALRT-TAIL-WINDOW-407 / TAIL-CUT-REAL-PAUSE-434：在前一片末尾**回溯 ≥2s** 的区间内找切点，
/// 供末尾组窗取「前片后缀」。返回 `(片内样本切点, 是否找到字缝)`（三态见 [`TailCutKind`]；
/// `gap_found = kind != NoGap`，**调用方 `take_context_suffix` 签名/语义不变**）。
///
/// 434 起优先取**真停顿**（连续 ≥120ms 低能量段的中点）：先在 `[len-back, len-frame/2]` 取**最早**
/// 真停顿（与旧「最早」口径一致 ⇒ 后缀不短于回溯要求）；无则**往前扩大**到 `len-2×back`（封顶整片）
/// 取**离 `len-back` 最近**的真停顿（后缀变长但不超整片）；再无 ⇒ 退回旧单帧字缝 [`WeakGap`] /
/// 按时长 [`NoGap`]。
pub(crate) fn find_tail_cut(prev: &[f32], back_samples: usize) -> (usize, bool) {
    let (cut, kind) = find_tail_cut_ex(prev, back_samples);
    (cut, kind != TailCutKind::NoGap)
}

/// TAIL-CUT-REAL-PAUSE-434：`find_tail_cut` 的三态版（供单测与日志区分）。
pub(crate) fn find_tail_cut_ex(prev: &[f32], back_samples: usize) -> (usize, TailCutKind) {
    if prev.len() <= back_samples {
        return (0, TailCutKind::NoGap); // 整片
    }
    let frame = super::vad::GAP_FRAME_SAMPLES;
    let lower = prev.len() - back_samples;
    let upper = prev.len().saturating_sub(frame / 2);
    if upper <= lower {
        return (lower, TailCutKind::NoGap);
    }
    let min_frames = ((TAIL_CUT_MIN_PAUSE_MS as usize * 16 + frame - 1) / frame).max(1); // 120ms/20ms=6
                                                                                         // ① 首轮区间取**最早**真停顿（离 len-back 最近）。
    let first = super::vad::real_pause_cut_candidates(prev, lower, upper, frame, min_frames);
    if let Some(&(cut, frames)) = first.first() {
        log_tail_cut(cut, frame, frames, back_samples, prev.len(), "real_pause");
        return (cut, TailCutKind::RealPause);
    }
    // ② 往前扩大最多 2×back，取**离 len-back 最近**（首轮已确认 [lower,upper] 无 ⇒ 候选均 < lower，
    //    取最大者）的真停顿。
    let lower2 = prev
        .len()
        .saturating_sub(back_samples.saturating_mul(TAIL_CUT_EXPAND_FACTOR));
    if lower2 < lower {
        let wide = super::vad::real_pause_cut_candidates(prev, lower2, upper, frame, min_frames);
        if let Some(&(cut, frames)) = wide
            .iter()
            .filter(|(c, _)| *c < lower)
            .max_by_key(|(c, _)| *c)
        {
            log_tail_cut(cut, frame, frames, back_samples, prev.len(), "real_pause");
            return (cut, TailCutKind::RealPause);
        }
    }
    // ③ 退回旧行为：单帧字缝 ⇒ WeakGap；无 ⇒ 按时长 NoGap。
    match super::vad::find_gap_cut_gap_only(prev, 0, lower, upper, frame) {
        Some(cut) => {
            log_tail_cut(cut, frame, 0, back_samples, prev.len(), "weak_gap");
            (cut, TailCutKind::WeakGap)
        }
        None => {
            log_tail_cut(lower, frame, 0, back_samples, prev.len(), "no_gap");
            (lower, TailCutKind::NoGap)
        }
    }
}

/// TAIL-CUT-REAL-PAUSE-434：Debug 守卫日志（`gap_found` 三态）。
fn log_tail_cut(cut: usize, frame: usize, frames: usize, back: usize, len: usize, kind: &str) {
    if log::log_enabled!(log::Level::Debug) {
        let pause_ms = frames * frame * 1000 / 16000;
        log::debug!(
            "[DBG-434] tail cut: kind={} cut_secs={:.3} pause_ms={} searched_back_secs={:.3}",
            kind,
            cut as f32 / 16000.0,
            pause_ms,
            (len - back.min(len)) as f32 / 16000.0
        );
    }
}

/// SEAM-INTERIOR-ONLY-436：重叠区音频的**内部分界比例** —— 中间 40%~60% 范围内、
/// 离中点最近的真停顿中点（相对 `overlap` 起点）；找不到真停顿 ⇒ 中点。
/// 复用 434 口径（`GAP_FRAME_SAMPLES` 帧、连续 ≥ `TAIL_CUT_MIN_PAUSE_MS`(120ms)，
/// 同 `real_pause_cut_candidates` 中位数口径）。返回比例 ∈ (0,1)；
/// `overlap.len() <= 1` ⇒ 0.5。纯函数（供 main.rs 派发时计算）。
pub(crate) fn interior_split_frac(overlap: &[f32]) -> f32 {
    const LO_FRAC: f32 = 0.4;
    const HI_FRAC: f32 = 0.6;
    let n = overlap.len();
    if n <= 1 {
        return 0.5;
    }
    let frame = super::vad::GAP_FRAME_SAMPLES;
    let min_frames = ((TAIL_CUT_MIN_PAUSE_MS as usize * 16 + frame - 1) / frame).max(1);
    let mid = n / 2;
    let lo = (n as f32 * LO_FRAC) as usize;
    let hi = ((n as f32 * HI_FRAC) as usize).max(lo + 1).min(n);
    if hi <= lo {
        return 0.5;
    }
    let cands = super::vad::real_pause_cut_candidates(overlap, lo, hi, frame, min_frames);
    // 离中点最近；打平取较小（靠前，确定性）。
    let mut best: Option<usize> = None;
    for &(c, _) in &cands {
        let d = c.abs_diff(mid);
        match best {
            None => best = Some(c),
            Some(b) if d < b.abs_diff(mid) => best = Some(c),
            _ => {}
        }
    }
    match best {
        Some(c) => (c as f32 / n as f32).clamp(f32::EPSILON, 1.0 - f32::EPSILON),
        None => mid as f32 / n as f32,
    }
}

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
/// 🔴 调用方传入的 `silent_ms` 必须是 **acc 专用计数器 `acc_silent_ms`**（PUNCT-LIVE-438 起
/// 打点只动自己的 `punct_interval_ms`，不再清任何静默计数器，但两个消费者仍各用各的计数器，
/// 不得合用 —— 阈值各自调整互不踩踏的理由见 `ACC_DISPATCH_SILENCE_MS_DEFAULT` 常量文档）。
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

/// DISPATCH-LONG-SPEECH-442（Gavin 2026-09-26：「不做派发、只做后端解码对于体验来说有啥意义？赶紧改」
/// 「应该是 1200ms 静默、或 10s 时长都可触发切片」）：派发触发只有两个 —— 静默 ≥1200ms（346 原有），
/// 或待派语音满此时长 ⇒ 用 437 同一函数**回看**找切点（120ms 真停顿只是找切点的内部判据）。
const LONG_SPEECH_LOOKBACK_MS: f32 = 10000.0;
/// DISPATCH-LONG-SPEECH-442：回看切点上限（片内 11s）⇒ 派出的片 <11s，落在派发后兜底切片的
/// 尾巴保护内（`vad::plan_gap_cuts` 剩余 <11s 不切），**不会被二次切**。
const LONG_SPEECH_LOOKBACK_UPPER_MS: f32 = 11000.0;
/// DISPATCH-LONG-SPEECH-442：待派片满此时长仍无停顿 ⇒ 允许字缝 / 最低能量兜底切（在 [10s,11s) 内）。
const LONG_SPEECH_FALLBACK_MS: f32 = 12000.0;
/// DISPATCH-LONG-SPEECH-442：回看判定节流（每 200ms 一次）。
const LONG_SPEECH_CHECK_EVERY_MS: f32 = 200.0;
/// DISPATCH-LONG-SPEECH-442：回看切点对应的流式文字 = 切点后此时长那一刻的显示。
/// STREAM-SV-463：paraformer 时代 400ms 是「吐字滞后」补偿；模拟流式每次重解覆盖到重解时刻、无吐字滞后，
/// 改取半个重解步长（300ms）⇒ 选中的那次结果覆盖到切点前后各至多 0.3s，两侧误差对称
///（误差只在精解回灌前短暂可见：回灌按 committed_len 替换前半，后半由新句重解）。
const LONG_SPEECH_TEXT_LAG_MS: f32 = 300.0;

/// FIX-LATE-STREAM-TAIL-445：冻结边界后「迟到字」跟踪的一步（**纯函数**）。
///
/// - `late` = 已冻结的最近一片边界 `(seg, len)`；`None` ⇒ 不跟踪。
/// - 开口（`pending_speech`）或已有新派发在定界（`bound_pending`）⇒ 停止跟踪，不吸收。
/// - 否则显示字数 `cur > len` ⇒ 返回 `absorb = Some((seg, cur))`（调用方重登记边界），并把跟踪长度推到 `cur`。
///
/// 返回 `(新的跟踪状态, 本步要重登记的边界)`。
fn late_bound_step(
    late: Option<(usize, usize)>,
    pending_speech: bool,
    bound_pending: bool,
    cur: usize,
) -> (Option<(usize, usize)>, Option<(usize, usize)>) {
    match late {
        None => (None, None),
        Some(_) if pending_speech || bound_pending => (None, None),
        Some((seg, len)) if cur > len => (Some((seg, cur)), Some((seg, cur))),
        Some(l) => (Some(l), None),
    }
}

/// DISPATCH-LONG-SPEECH-442：带标点的显示文本里，覆盖到「裸文本前 `raw_prefix_bytes` 字节」为止的字符数。
///
/// 显示文本 = 裸文本 + 标点引擎插入的标点（只插不改字；英文大小写宽松比较）⇒ 逐字对齐：
/// 与裸文本当前字相同 ⇒ 两边都前进；不同（插入的标点）⇒ 只数显示字。裸文本前缀耗尽即停。
fn display_chars_for_raw_prefix(display: &str, raw: &str, raw_prefix_bytes: usize) -> usize {
    let end = raw_prefix_bytes.min(raw.len());
    let end = (0..=end)
        .rev()
        .find(|&b| raw.is_char_boundary(b))
        .unwrap_or(0);
    let target: Vec<char> = raw[..end].chars().collect();
    let mut ti = 0usize;
    let mut n = 0usize;
    for dc in display.chars() {
        if ti >= target.len() {
            break;
        }
        n += 1;
        if dc == target[ti] || dc.to_lowercase().eq(target[ti].to_lowercase()) {
            ti += 1;
        }
    }
    n
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
    /// STREAM-SV-463：`prefix` 覆盖的裸文本原文（= raw[..raw_len] 打点当刻的内容）。
    /// 模拟流式会改写当前句 ⇒ 复用前必须核对它没变（[`punct_cache_view`]），否则会显示改写前的旧字。
    raw: String,
}

/// PREVIEW-PUNCT-LIVE-438：预览打点是否触发 —— **距上次打点 ≥ 间隔 + 有新内容**。
///
/// 438 把触发从「静默 ≥1200ms」（PUNCT-349 口径）改为**单一的 3.5s 间隔**（Gavin：预览窗口上
/// 只保留一个打点机制）：与语音/静默无关，计时由 `punct_interval_ms` 每 chunk 无条件累加、
/// 实际完成打点后归零。`has_new` 护栏保留 —— 没有新字不重复打（省算力）。
fn should_repunctuate_preview(interval_ms: f32, has_new: bool, threshold_ms: f32) -> bool {
    interval_ms >= threshold_ms && has_new
}

/// PREVIEW-PUNCT-LIVE-438：把**原始无标点全文**转成 overlay 预览文本。
///
/// - `force`（由 `should_repunctuate_preview` 给出，即距上次打点 ≥3.5s）：**只对
///   `raw_full[tail_start..]` 这条裸尾巴**重打一次标点并更新缓存；`tail_start` 之前、
///   已派发给 accuracy 的显示文本（冻结前缀）**逐字节保留** ⇒ `committed_len` 坐标不漂移、
///   不与精解标点打架，且长文本下只打尾巴省算力（补充 R1，主控定案）。
/// - 否则：`上次标点文本 + raw_full 新增后缀` —— 新字即时上屏，已出现的标点不闪回。
/// - `engine` 为 `None`（用户关标点）：直接返回 `raw_full`，行为与未启用标点一致，不引入额外开销。
///
/// 🔴 关键：输入始终是**原始裸文本**，绝不把已打点文本再次送引擎 ⇒ 不会「标点重复 / 错乱」
/// （FIX-252 场景不复发）。
///
/// 🔴 PUNCT-349 防复发（438 落实）：打点结果**存缓存前**剥掉末尾句末终止符（与 `punctuation`
/// 模块 [`strip_trailing_punctuation`] 同一套判据），CT-Transformer 给未说完的半句补的「。」
/// 不再经 `punct_cache_reuse` 落到句中；句中标点照常保留。最后一句的句尾标点由 accuracy
/// 回灌 / 最终输出给出。
///
/// # 参数（补充 R1 双坐标，与 `committed_len` 同刻同源捕获）
/// - `tail_start`：最近一次派发当刻的 `raw` 字节长 ⇒ 尾巴打点起点。
/// - `head_chars`：最近一次派发当刻显示文本的字符数（= `committed_len`）⇒ 冻结前缀的字符数。
///   二者为 0（acc 未启用 / 尚未派发）⇒ 整段重打，与改前口径一致。
/// - `sentence_end`（PREVIEW-PUNCT-464）：此刻句子已说完（停顿派发 / 句末冻结 / 录音结束）⇒ 不论有无新字
///   都把尾巴重打一次并**保留句末标点**（说话中的 349「句末终止符不入缓存」只针对没说完的半句）。
///
/// KOJA-PUNCT-464：尾巴含日文假名 / 韩文 ⇒ 不送标点模型（中英词表；韩文空格会被吃光），原样显示。
fn preview_display(
    raw_full: &str,
    engine: Option<&mut PunctuationEngine>,
    cache: &mut PunctPreviewCache,
    force: bool,
    tail_start: usize,
    head_chars: usize,
    sentence_end: bool,
) -> String {
    let Some(engine) = engine else {
        return raw_full.to_string();
    };
    // 有新内容才打。STREAM-SV-463：模拟流式会改写当前句（长度不变也可能字变了）⇒ 按「与上次打点时
    // 的裸文本不同」判（只追加时与原「变长」口径等价）。
    let has_new = raw_full != cache.raw;
    // 尾巴起点夹紧到当前 raw 内，并要求 char 边界（raw 经 endpoint 确认可能变短再长，
    // 历史字节位置不保证仍是当前串的合法切点；不合法就本帧不打，复用路径原样显示，下周期恢复）。
    let tail_start = tail_start.min(raw_full.len());
    let tail_ok = tail_start < raw_full.len()
        && raw_full.is_char_boundary(tail_start)
        && !crate::punctuation::ct_unsupported_script(&raw_full[tail_start..]);
    if tail_ok && ((force && has_new) || sentence_end) {
        let head = punct_head(&cache.prefix, raw_full, &cache.raw, tail_start, head_chars);
        let t = Instant::now();
        let tail = engine
            .add_punctuation(&raw_full[tail_start..])
            .unwrap_or_else(|| raw_full[tail_start..].to_string());
        // PUNCT-349 防复发：没说完的半句，句末终止符不进缓存（句中标点照旧保留）；
        // PREVIEW-PUNCT-464：句子已说完 ⇒ 句末标点保留。
        cache.prefix = if sentence_end {
            format!("{head}{tail}")
        } else {
            build_punct_prefix(&head, &tail)
        };
        cache.raw_len = raw_full.len();
        cache.raw = raw_full.to_string();
        log::debug!(
            "LOCALRT-PUNCT-LIVE-438: repunctuated tail {} chars (head={} chars) in {:.1}ms",
            raw_full[tail_start..].chars().count(),
            head_chars,
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
    // STREAM-SV-463：复用前先核对缓存覆盖的裸文本没被改写（`punct_cache_view`）。
    punct_cache_view(&cache.prefix, &cache.raw, raw_full)
}

/// STREAM-SV-463：标点缓存的**校验复用** —— 模拟流式会改写当前句，缓存覆盖的裸文本 `cache_raw` 可能已变：
/// - 未变（`raw` 以它开头）⇒ 原 344 边界安全拼法 [`punct_cache_reuse`]；
/// - 变了 ⇒ 只保留与当前裸文本**公共前缀**那段的标点文本，其后接当前裸文本（下次 3.5s 打点再补标点）。
///
/// 冻结区（已派发、此后不再改写）恒在公共前缀内 ⇒ 冻结前缀逐字节不变（438 R1 不受影响）。
fn punct_cache_view(prefix: &str, cache_raw: &str, raw: &str) -> String {
    if raw.starts_with(cache_raw) {
        return punct_cache_reuse(prefix, raw, cache_raw.len());
    }
    let common = stable_prefix_bytes(cache_raw, raw);
    let keep = display_chars_for_raw_prefix(prefix, cache_raw, common);
    let mut s = char_prefix(prefix, keep);
    s.push_str(&raw[common..]);
    s
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

/// PREVIEW-PUNCT-LIVE-438（R1）：重打时的**冻结前缀** —— 覆盖 `raw[..tail_start]` 的已显示文本。
///
/// - `tail_start >= cache_raw.len()`（常态：缓存前缀只覆盖到上次打点位置）：前缀续上
///   `raw[cache_raw.len()..tail_start]` 的逐字尾巴即为冻结前缀（经 `punct_cache_view` 校验后拼；
///   前缀为空时退化为裸文本前缀，与「尚未打过点」口径一致）。
/// - `tail_start < cache_raw.len()`（上次打点发生在最近一次派发**之后**）：此时缓存前缀 =
///   冻结前缀（`head_chars` 字符）+ 其后的尾巴标点 ⇒ 按**字符**截出冻结前缀
///   （越界则整段保留，绝不切裂）。
///
/// 🔴 返回值与重打前的已显示前缀**逐字节相同** ⇒ `committed_len` 坐标不漂移（单测
/// `fix438_prefix_bytes_stable_across_repunct` 钉死）。
fn punct_head(
    prefix: &str,
    raw: &str,
    cache_raw: &str,
    tail_start: usize,
    head_chars: usize,
) -> String {
    debug_assert!(raw.is_char_boundary(tail_start));
    if tail_start >= cache_raw.len() {
        // STREAM-SV-463：缓存打在派发之前、所覆盖的字可能在冻结前被改写 ⇒ 校验后再拼。
        punct_cache_view(prefix, cache_raw, &raw[..tail_start])
    } else {
        char_prefix(prefix, head_chars)
    }
}

/// PREVIEW-PUNCT-LIVE-438：按**字符数**截前缀（越界返回整串）—— char 边界安全，绝不切裂 UTF-8。
fn char_prefix(s: &str, n: usize) -> String {
    match s.char_indices().nth(n) {
        Some((i, _)) => s[..i].to_string(),
        None => s.to_string(),
    }
}

/// PREVIEW-PUNCT-LIVE-438：拼出**存进缓存的新前缀** = 冻结前缀 + 剥掉末尾终止符的尾巴打点结果。
///
/// 🔴 PUNCT-349 防复发的唯一入口：CT-Transformer 给未说完的半句补的句末终止符
/// （`。？！.?!` 等，判据复用 `punctuation::strip_trailing_punctuation` 同一套集合）
/// 在这里被剥掉，**永不进入缓存**；句中标点（`，、；` 等）照旧保留。
fn build_punct_prefix(head: &str, tail_punct: &str) -> String {
    let mut s = String::with_capacity(head.len() + tail_punct.len());
    s.push_str(head);
    s.push_str(&strip_trailing_punctuation(tail_punct));
    s
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
    /// STREAM-SV-463：已确认区（冻结、此后不再改写的部分）。
    fn confirmed(&self) -> &str {
        &self.buf[..self.confirmed_bytes]
    }
}

/// STREAM-SV-463：构建本地实时预览 recognizer = 官方 SenseVoice-Small 2024-07-17（模拟流式用）。
///
/// 模型目录（DEC-011，exe 同级 models）：[`super::SENSEVOICE_MODEL_SUBDIR`]（与 performance 档**同一份**，
/// Gavin 2026-10-03「不用留两个版本」），用 `model.int8.onnx` + `tokens.txt`。
/// - `language = auto`：中英日韩粤自动判；`use_itn = false`：只要裸文本（标点 / 数字仍走产品自己的规则，
///   406 比对 / 精解草稿的口径与改前一致）。
/// - 线程数 = 物理核数（[`local_stream_num_threads`]）。
///
/// 🔴 任一模型文件缺失 ⇒ `Err`（DEC-067 附则一：不降级，整档不可用）。
pub fn create_local_stream_recognizer(model_dir: &Path) -> Result<OfflineRecognizer> {
    let dir = model_dir.join(super::SENSEVOICE_MODEL_SUBDIR);
    let model = dir.join("model.int8.onnx");
    let tok = dir.join("tokens.txt");
    for p in [&model, &tok] {
        if !p.exists() {
            anyhow::bail!("本地预览模型文件缺失: {}", p.display());
        }
    }
    let num_threads = local_stream_num_threads();
    let c = OfflineRecognizerConfig {
        model_config: OfflineModelConfig {
            sense_voice: OfflineSenseVoiceModelConfig {
                model: Some(model.to_string_lossy().to_string()),
                language: Some("auto".to_string()),
                use_itn: false,
            },
            tokens: Some(tok.to_string_lossy().to_string()),
            num_threads,
            provider: Some("cpu".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    log::debug!(
        "[STREAM-SV-463] preview model: sensevoice num_threads={} step_ms={} provider=cpu",
        num_threads,
        SIM_STEP_MS
    );
    OfflineRecognizer::create(&c).context("创建本地预览 (SenseVoice) recognizer 失败")
}

/// STREAM-SV-463：模拟流式的一次重解 —— 把当前句音频整段送 SenseVoice，取裸文本。
/// 回放测试用（与生产同一解码，流式原文口径一致）。
#[allow(dead_code)]
pub(crate) fn preview_decode(recognizer: &OfflineRecognizer, audio: &[f32]) -> String {
    preview_decode_timed(recognizer, audio).0
}

/// STREAM-SV-463：同 [`preview_decode`]，另给逐 token 的 `(起始秒, 该 token 在文本里的结束字节)`。
/// 时间戳对不上文本（模型未给 / 拼写规则不同）⇒ 空表，调用方回退旧口径。
fn preview_decode_timed(
    recognizer: &OfflineRecognizer,
    audio: &[f32],
) -> (String, Vec<(f32, usize)>) {
    if audio.is_empty() {
        return (String::new(), Vec::new());
    }
    let s = recognizer.create_stream();
    s.accept_waveform(SAMPLE_RATE, audio);
    recognizer.decode(&s);
    let Some(r) = s.get_result() else {
        return (String::new(), Vec::new());
    };
    let text = r.text.trim().to_string();
    let marks = match &r.timestamps {
        Some(ts) if ts.len() == r.tokens.len() => token_text_ends(&text, &r.tokens, ts),
        _ => Vec::new(),
    };
    (text, marks)
}

/// STREAM-SV-463：把 token 顺序对到文本上，得每个 token 的结束字节。SentencePiece 的词首 `▁` 记作空格；
/// 任何一个对不上（间隔超过一个空格）⇒ 整体放弃（空表），绝不给出错位的切点。
fn token_text_ends(text: &str, tokens: &[String], ts: &[f32]) -> Vec<(f32, usize)> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut pos = 0usize;
    for (tok, &t) in tokens.iter().zip(ts) {
        let piece = tok.replace('\u{2581}', " ");
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }
        match text[pos..].find(piece) {
            Some(off) if text[pos..pos + off].trim().is_empty() && off <= 1 => {
                pos += off + piece.len();
                out.push((t, pos));
            }
            _ => return Vec::new(),
        }
    }
    out
}

/// STREAM-SV-463：按切点时间切分当前句结果 —— 起始时刻早于 `cut_secs` 的 token 归前半，返回前半结束字节。
/// 无时间戳 ⇒ `None`（调用方回退「切点那一刻的文字长度」旧口径）。
fn split_bytes_at_time(marks: &[(f32, usize)], cut_secs: f32) -> Option<usize> {
    if marks.is_empty() {
        return None;
    }
    Some(
        marks
            .iter()
            .take_while(|(t, _)| *t < cut_secs)
            .last()
            .map(|&(_, end)| end)
            .unwrap_or(0),
    )
}

/// STREAM-SV-463：两次重解结果的公共前缀字节长（落在 char 边界）——显示层「已定」的部分。
fn stable_prefix_bytes(prev: &str, cur: &str) -> usize {
    prev.char_indices()
        .zip(cur.chars())
        .take_while(|((_, a), b)| a == b)
        .last()
        .map(|((i, a), _)| i + a.len_utf8())
        .unwrap_or(0)
}

/// STREAM-SV-463：新句文本接在已确认文本后是否要补空格（两边都是拉丁字母 / 数字 ⇒ 英文单词不粘连）。
fn sentence_join_space(confirmed: &str, next: &str) -> bool {
    matches!(
        (confirmed.chars().next_back(), next.chars().next()),
        (Some(a), Some(b)) if a.is_ascii_alphanumeric() && b.is_ascii_alphanumeric()
    )
}

/// STREAM-SV-463：`s` 中不超过 `bytes` 的最大 char 边界。
fn floor_char_boundary(s: &str, bytes: usize) -> usize {
    let mut b = bytes.min(s.len());
    while !s.is_char_boundary(b) {
        b -= 1;
    }
    b
}

/// LOCALRT-REFLOW-HOLE-344-G：取「第 i 片对应的流式文本」——
/// `display` 去掉前 `prev_committed` 个**字符**后的后缀。
///
/// 🔴 **必须按 char 计数切片，绝不用字节下标**（`&display[prev_committed..]` 会在中文中途 panic，
/// 见 LOCALRT-CHARBOUNDARY-344）。`chars().skip(n).collect()` 天然字符安全。
fn segment_streaming_text(display: &str, prev_committed: usize) -> String {
    display.chars().skip(prev_committed).collect()
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

// STREAM-SV-463：PHANTOM-406-459 ①（剥派发片开头的静音幻字）随 paraformer 一并移除 —— 模拟流式只在
// 当前句**听到人声后**才重解、且句首静音不入窗，结构上不再有「静音里吐出的字」（DEC-077 不留死代码）。

/// VAD-REUSE-458：实时 VAD 判出「开始说话」比真实起点晚（silero 需连续 ≥0.1s 语音、窗口 32ms）⇒
/// 进行中段起点按判定位置往前留 0.5s 余量（其后剪静音再各扩 0.2s）。
const ONGOING_ONSET_MARGIN: usize = 8_000;

/// VAD-393-R1 / VAD-REUSE-458：派发片的片内区间。VAD 不可用 ⇒ `None`（调用方回退 391 自跑 VAD）。
///
/// **派发时刻 VAD 仍在语音段中**：进行中段尚未进时间线（依据见下）。它开始于时间线里最后一个完整段
/// 结束之后，且不晚于实时 VAD 判出「开始说话」的位置 `ongoing_onset` ⇒ 从
/// `max(最后完整段结束, ongoing_onset − ONGOING_ONSET_MARGIN)` 到片尾全算语音（绝不吞字；
/// 只多留判定延迟那一小段，长停顿仍剪掉——长静音是 388 念词表 / 坍塌的诱因）。
/// 原先此时返回 `None` 让精解线程整窗重跑 VAD：Gavin 09-30 真实 24 窗中 9 窗走此路，每窗 ~39ms 挡在解码前，
/// 且一个 `None` 片会连带下一窗（作前文后缀时）也重跑。
///
/// 依据（sherpa-onnx 1.13.8 `voice-activity-detector.cc`）：`IsSpeechDetected()` 返回 `start_ != -1`
///（:196）—— 进行中段的标志；段只在**非语音分支**入队（:110-120）或 `Flush()`（:187）里产出，
/// 且同一次处理里 `start_ = -1`（:134 或 :190）。C API `SherpaOnnxVoiceActivityDetectorDetected`
/// → `impl->IsSpeechDetected()`（`c-api.cc:1391-1398`）。⇒ `vad_speech == true` 时该段尚未进
/// `timeline`，据残缺时间线剪静音会把它剪掉（吞字），故回退自跑 VAD。
fn dispatch_slice_ranges(
    vad_on: bool,
    vad_mid_speech: bool,
    ongoing_onset: Option<usize>,
    timeline: &[(usize, usize)],
    spans: &[(usize, usize, usize)],
) -> Option<Vec<Vec<(usize, usize)>>> {
    if !vad_on {
        return None;
    }
    if !vad_mid_speech {
        return Some(slice_ranges_from_timeline(timeline, spans));
    }
    let last_end = timeline.iter().map(|&(_, e)| e).max().unwrap_or(0);
    let ongoing_from = ongoing_onset.map_or(last_end, |o| {
        o.saturating_sub(ONGOING_ONSET_MARGIN).max(last_end)
    });
    let mut tl = timeline.to_vec();
    tl.push((ongoing_from, usize::MAX));
    Some(slice_ranges_from_timeline(&tl, spans))
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
/// 实际要停 1.5s 才到 1200ms。故在转换的那个 chunk 把 `acc_silent_ms` 直接
/// **补记**这么多，使「从真实停顿起算满 1200ms」与改前一致
///（438 起显示层 `silent_ms` 已删，本函数只服务 acc 计数器与测试模型）。
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
///   （`None` = 用户关标点）。PREVIEW-PUNCT-LIVE-438：预览打点**只按 3.5s 间隔**触发、
///   且只送「精解未覆盖的裸尾巴」（间隔 + `has_new` 双护栏 ⇒ 引擎不会每帧被调用，不拖 RTF）。
///   引擎不在此新建（否则重复加载 972MB 级模型）。
/// - `silence_threshold`：LOCALRT-PUNCT-TIMER-269-B，静默判定口径（RMS ≤ 阈值即静音，
///   同 `config.audio.silence_threshold`）。用于能量/VAD 判「是否有人声」（打点、acc 派发、
///   端点、近场门共用这同一判定）；
///   🔴 与 sherpa endpoint 无关，不触发 reset/切句；438 起**不再**用于触发显示层打点
///   （打点改按 `PUNCT_PREVIEW_INTERVAL_MS` 间隔，与静默无关）。
/// - `vad_device`：LOCALRT-NEARFIELD-GATE-389（C2）录音设备 key（`config.audio.input_device` 原样；
///   空串 = 系统默认）—— 供**跨录音沿用录音人音量**（同设备、10 分钟内）。
/// - `on_result`：文本变化回调，传 `(display_text, display_words, tentative_chars)`；
///   STREAM-SV-463：`tentative_chars` = 显示文本**末尾**尚未稳定（相邻两次重解不一致）的字数，
///   显示层画浅色；已冻结 / 已稳定的字正常色。按「末尾字数」传，前缀被精解回灌替换也不错位。
/// - `acc_cfg`：LOCALRT-PARALLEL-ACC-298 派发配置（`enabled`/`silence_ms`；346 起无长度支）；
///   `enabled=false` 时本函数**完全不派发**（逐位退回串行行为）
/// - `on_segment`：accuracy 并行派发回调，传
///   `(seg_index, committed_len, 已加 padding 的 16k f32 子段列表, seg_streaming_text)`。
///   `committed_len` = **派发当刻浮层已显示文本的字符数**（325 回灌边界；取 `last_display`，
///   与消费侧 `last_streaming_text` 镜像同源；数的是**带标点**显示文本 —— 438 查清的坐标系）。
///   PREVIEW-PUNCT-LIVE-438（R1）：同一时刻把当刻 `raw` 字节长一并存为 `punct_tail_start`，
///   3.5s 重打据此**只打边界之后的裸尾巴**、冻结前缀逐字节保留 ⇒ `streaming[committed_len..]`
///   不因标点增减漂移（不多字、不丢字）。
///   `seg_streaming_text` = 本片对应的**流式文本**（`last_display` 自上一片 `committed_len`
///   起的字符后缀，**按 char 切片**）—— LOCALRT-REFLOW-HOLE-344-G：worker 解出空/失败时用它
///   填补，避免累积 `acc_text` 留洞导致后续回灌**永久失效**。
///   子段列表通常 1 个；`FIX-SLICE-CUT-AT-GAP-381` 起单片剩余 >10s 即按**字缝**切
///   （从 10s 起找能量低谷、最晚 12s 兜底，`vad::plan_gap_cuts`），切点严格相接、不丢内容。
///   🔴 只在 `acc_cfg.enabled` 且静默 ≥ `acc_cfg.silence_ms`(1200ms) 且未上 latch 时被调用
/// - `on_long_silence`：LOCALRT-TAIL-WINDOW-407 长静默回调，传当刻 `pcm` 样本位置；
///   `acc_cfg.enabled` 且静默 ≥ [`LONG_SILENCE_TAIL_MS`]（1900ms）且本段静默未发过时**只发一次**，
///   恢复说话后复位。供 main 侧组「末尾窗」（不依赖停止键）。
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
    recognizer: &OfflineRecognizer,
    cancel_signal: Option<&AtomicBool>,
    mut punctuation_engine: Option<&mut PunctuationEngine>,
    silence_threshold: f32,
    vad_device: &str,
    mut on_result: impl FnMut(&str, &[WordTiming], usize),
    acc_cfg: AccDispatchConfig,
    // VAD-393（A2）：新增第 5 参 `slice_ranges`：**片内坐标**的语音区间（VAD 时间线映射而来）；
    // VAD 不可用 ⇒ `None`（调用方回退自行跑 VAD）。
    // FALLBACK-EXCLUDE-FARFIELD-446：第 6 参 = 各子片**片内坐标**的「远场」区间（VAD 判人声但近场门判非近场），
    // 仅供精解失败 / 被拒时的**流式兜底**剔除背景文字；不参与解码前剪裁（392：门不定内容去留）。
    mut on_segment: impl FnMut(
        usize,
        usize,
        Vec<Vec<f32>>,
        String,
        Option<Vec<Vec<(usize, usize)>>>,
        Vec<Vec<(usize, usize)>>,
    ),
    // LOCALRT-TAIL-WINDOW-407：长静默（同一段静默 ≥1900ms 且此后无新语音）**只发一次**信号，
    // 携带当刻 `pcm` 样本位置。main 侧据此把「待覆盖片 + 前一片」组末尾窗即时解码回灌，
    // 无需用户按停止键（Gavin 2026-09-24）。恢复说话即复位、可再次触发。
    mut on_long_silence: impl FnMut(usize),
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

    // STREAM-SV-463：模拟流式「当前句」状态（当前句 = `pcm[sim_start..]`，未冻结部分）。
    let ms_samples = |ms: f32| (ms * SAMPLE_RATE as f32 / 1000.0) as usize;
    let mut sim_start: usize = 0;
    // 下一次重解不早于此 pcm 位置（开口后 SIM_FIRST_MS、此后每 SIM_STEP_MS）；`None` = 本句尚无人声。
    let mut sim_next_at: Option<usize> = None;
    // 上次重解覆盖到的 pcm 位置（窗口硬上限在此冻结，不丢音频）。
    let mut sim_decoded_end: usize = 0;
    // 上次重解之后又听到人声（VAD）⇒ 值得再解；纯静音不重解。
    let mut sim_dirty = false;
    // 当前句最新结果（裸文本，必要时带句首分隔空格）及其「相邻两次一致」的前缀字节长。
    let mut sim_hyp = String::new();
    let mut sim_stable: usize = 0;
    // 当前句结果逐 token 的 `(起始秒, 结束字节)`（442 回看按切点时间切分用；对不上 ⇒ 空）。
    let mut sim_marks: Vec<(f32, usize)> = Vec::new();
    // 当前句末尾连续无人声（VAD）时长 ⇒ 句末冻结。
    let mut sim_silence_ms: f32 = 0.0;
    // 上次回调给显示层的浅色尾巴字数（文本不变、只是尾巴变稳也要刷新）。
    let mut last_tentative: usize = 0;
    let mut sim_decodes: u64 = 0;
    let mut sim_decode_ms_total = 0.0f64;
    let mut sim_decode_ms_max = 0.0f64;
    let mut state = StreamingAsrState::new();
    let mut sentence_id: i64 = 0;
    let mut last_display = String::new();
    // PREVIEW-PUNCT-LIVE-438（R1）：`last_display` 所覆盖的 `raw` 字节长 —— 与 `last_display`
    // **同刻赋值**（同一次 `last_display = display;`），保证派发时捕获的双坐标
    // （`committed_len` 字符数 + 本值）描述的是**同一个显示串**（不可用当刻 `display_cache.text()`：
    // 本迭代 raw 可能已先于 last_display 变长，两者会错一拍）。
    let mut last_display_raw_len: usize = 0;
    let mut pcm: Vec<f32> = Vec::new();
    // VAD-REUSE-458：见 `dispatch_slice_ranges` 的 `ongoing_onset`。
    let mut vad_seg_onset: Option<usize> = None;
    // LOCALRT-PUNCT-TIMER-269：预览标点节流缓存（只对裸文本打点，杜绝重复）。
    let mut punct_cache = PunctPreviewCache {
        prefix: String::new(),
        raw_len: 0,
        raw: String::new(),
    };
    // PREVIEW-PUNCT-LIVE-438：预览打点**间隔**计时（ms）。与语音/静默无关 —— 每 chunk 按
    // `chunk_ms` 无条件累加（见下方时间推进处），仅在**实际完成一次打点**时归零。
    // 取代 349 的静默触发 + 显示层 `silent_ms` 计数器（DEC-077：旧机制连根删除，不留死代码）。
    let mut punct_interval_ms: f32 = 0.0;
    // PREVIEW-PUNCT-464：下一次显示刷新按「句子已说完」补句末标点（停顿派发 / 句末冻结 / 录音结束置位）。
    let mut punct_sentence_end = false;
    // PREVIEW-PUNCT-LIVE-438（R1）：acc 派发边界的**双坐标**，与 `committed_len` 同刻同源捕获：
    //   *_chars = 派发当刻已显示文本的字符数（= `committed_len`；冻结前缀按字符数截取的依据）
    //   *_raw   = 派发当刻 `raw` 的字节长（3.5s 重打时尾巴的起点）
    // 二者恒为 0（acc 未启用 / 尚未派发）⇒ 整段重打，与改前口径一致。
    let mut punct_head_chars: usize = 0;
    let mut punct_tail_start: usize = 0;
    // ACC-DISPATCH-SILENCE-ONLY-346：accuracy 派发专用静默累计（ms）。438 删除显示层
    // `silent_ms` 后，本计数器是静默计时在本函数内的**唯一**消费者；打点只动自己的
    // `punct_interval_ms` ⇒ 1200ms 派发时机与改前**逐位不变**（护栏 `guard346_*`）。
    let mut acc_silent_ms: f32 = 0.0;

    // STREAM-SV-463：当前句（`pcm[sim_start..]`）是否已听到人声（VAD 原始判定，392 内容口径）。
    // 没有人声的句子不重解、不显示（沿用 342「静音不出字」的意图）；冻结即复位。
    let mut speech_since_last_reset: bool = false;
    // VAD-393（A2）：会话语音时间线（`pcm` 绝对坐标，按序）。实时 VAD 逐 chunk 产出，派发时
    // 映射为片内 ranges ⇒ **复用实时判断、不再对每个窗口重跑 VAD**（且窗口开头有前文、结论与实时一致）。
    let mut speech_timeline: Vec<(usize, usize)> = Vec::new();
    // FALLBACK-EXCLUDE-FARFIELD-446：会话 pcm 坐标的远场区间（VAD 判人声 && 近场门判非近场，逐 chunk 合并）。
    let mut far_timeline: Vec<(usize, usize)> = Vec::new();
    // LOCALRT-PERF-405（F-C-01）：显示文本增量缓存（替代每 chunk `state.display_text()` 的
    // 全量 join + 分配）。切句时更新 confirmed 区，当前句只替换尾部 current 区。
    let mut display_cache = DisplayCache::new();

    // PARALLEL-ACC-298：accuracy 并行派发状态（与端点**完全独立**）。
    // 已派发到的 `pcm` 位置（下一片从这里起算）。
    let mut acc_dispatched_end: usize = 0;
    // 本轮静默是否已派发过（latch 写法）。
    let mut acc_done_for_pause = false;
    // LOCALRT-TAIL-WINDOW-407：本轮静默是否已发过「长静默末尾组窗」信号（同一停顿只发一次）。
    let mut long_silence_done_for_pause = false;
    // 本次未派发区间内是否出现过语音（尾片「有语音才派」的判据）。
    let mut acc_pending_has_speech = false;
    // DISPATCH-LONG-SPEECH-442：本次未派发区间内**首个语音 chunk** 所在 pcm 位置（长段计时起点）。
    let mut acc_pending_speech_start: Option<usize> = None;
    // DISPATCH-LONG-SPEECH-442：上次回看判定时的 pcm 位置（节流 200ms）。
    let mut long_lookback_checked_at: usize = 0;
    // DISPATCH-LONG-SPEECH-442：`(pcm 位置, 当刻裸文本字节长)` 短历史（约 4s）——回看切点在过去，
    // 按切点那一刻的流式文字定精解覆盖边界，而不是按「现在」的整段显示。
    let mut raw_len_history: std::collections::VecDeque<(usize, usize)> =
        std::collections::VecDeque::new();
    // 已派发片数（= 下一片的 seg_index）。
    let mut acc_seg_index: usize = 0;
    // LOCALRT-REFLOW-HOLE-344-G：上一片派发时的 `committed_len`（= 该片流式文本的起点字符）。
    // 第 i 片对应的流式文本 = `last_display` 的第 `acc_prev_committed` 个字符起的一段。
    let mut acc_prev_committed: usize = 0;
    // LOCALRT-SEAM-337：**自适应定界**状态（见 `on_reflow_commit` 参数文档）。
    // `bound_seg = Some(i)` ⇒ 第 i 片已派发，正在等「文本停止增长 / 有声恢复 / 硬上限」最先发生。
    let mut bound_seg: Option<usize> = None;
    // FIX-LATE-STREAM-TAIL-445：已冻结（a / c）的最近一片边界 `(seg, len)`；此后 VAD 未再听到语音时
    // 流式迟到吐出的字归入该片（边界后移 + 重登记），开口 / 新派发即停止跟踪。
    let mut late_bound: Option<(usize, usize)> = None;
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

    // STREAM-SV-463：模拟流式的三个动作（宏 = 循环内与录音结束后共用同一段代码，变量在定义处解析）。
    // ① 重解当前句：`pcm[sim_start..]` 整段送 SenseVoice，结果替换显示缓存的当前句区。
    //    `sim_decode_text!` 只更新文本（冻结前补解用，排期由冻结重置）；`sim_decode!` 另排下一次重解。
    macro_rules! sim_decode_text {
        () => {{
            let td = Instant::now();
            let (text, marks) = preview_decode_timed(recognizer, &pcm[sim_start..]);
            let dt = td.elapsed().as_secs_f64() * 1000.0;
            sim_decodes += 1;
            sim_decode_ms_total += dt;
            sim_decode_ms_max = sim_decode_ms_max.max(dt);
            if !first_ready_seen {
                first_ready_seen = true;
                t_ready_ms = t0.elapsed().as_secs_f64() * 1000.0;
                log::info!(
                    "[Latency] local_stream first_decode at +{:.1}ms (cost {:.0}ms)",
                    t_ready_ms,
                    dt
                );
            }
            // 英文单词跨句不粘连：新句首词前补空格（随结果一起存 ⇒ 稳定前缀与坐标口径不变）。
            let (text, marks) = if sentence_join_space(display_cache.confirmed(), &text) {
                (
                    format!(" {text}"),
                    marks.into_iter().map(|(t, end)| (t, end + 1)).collect(),
                )
            } else {
                (text, marks)
            };
            sim_marks = marks;
            sim_stable = stable_prefix_bytes(&sim_hyp, &text);
            if log::log_enabled!(log::Level::Debug) {
                log::debug!(
                    "[STREAM-SV-463] decode win={:.2}s cost={:.0}ms chars={} stable_bytes={} text='{}'",
                    (pcm.len() - sim_start) as f32 / SAMPLE_RATE as f32,
                    dt,
                    text.chars().count(),
                    sim_stable,
                    text
                );
            }
            if text != sim_hyp {
                if !text.is_empty() && !first_result_seen {
                    first_result_seen = true;
                    t_result_ms = t0.elapsed().as_secs_f64() * 1000.0;
                    log::info!(
                        "[Latency] local_stream first_nonempty_result at +{:.1}ms ({} chars)",
                        t_result_ms,
                        text.chars().count()
                    );
                }
                sim_hyp = text;
                state.on_result(sentence_id, &sim_hyp, false, &[]);
                display_cache.on_current(&sim_hyp);
            }
        }};
    }
    macro_rules! sim_decode {
        () => {{
            sim_decode_text!();
            sim_decoded_end = pcm.len();
            sim_next_at = Some(pcm.len() + ms_samples(SIM_STEP_MS));
            sim_dirty = false;
        }};
    }
    // ② 确认：当前句结果并入显示缓存的已确认区（此后不再改写）。
    macro_rules! sim_confirm {
        () => {{
            if !sim_hyp.is_empty() {
                state.on_result(sentence_id, &sim_hyp, true, &[]);
                display_cache.on_confirm(&sim_hyp);
                sentence_id += 1;
            }
        }};
    }
    // ③ 冻结当前句：确认后新句从 `$next` 起；`$ongoing` = 冻结时仍在说话（新句已有人声，尽快重解）。
    macro_rules! sim_freeze {
        ($next:expr, $ongoing:expr) => {{
            let next: usize = $next;
            let ongoing: bool = $ongoing;
            sim_confirm!();
            if log::log_enabled!(log::Level::Debug) {
                log::debug!(
                    "[STREAM-SV-463] freeze at {:.2}s (sentence {:.2}s) chars={} ongoing={}",
                    next as f32 / SAMPLE_RATE as f32,
                    next.saturating_sub(sim_start) as f32 / SAMPLE_RATE as f32,
                    sim_hyp.chars().count(),
                    ongoing
                );
            }
            sim_hyp.clear();
            sim_marks.clear();
            sim_stable = 0;
            sim_start = next;
            sim_decoded_end = next;
            sim_silence_ms = 0.0;
            speech_since_last_reset = ongoing;
            sim_dirty = ongoing;
            sim_next_at = ongoing.then(|| next + ms_samples(SIM_FIRST_MS));
        }};
    }
    // ④ 显示刷新（循环每轮末尾 + 录音结束后各一次）。
    macro_rules! refresh_preview {
        () => {{
            // PREVIEW-PUNCT-LIVE-438 / PREVIEW-PUNCT-464：显示刷新 —— 打点按 `PUNCT_PREVIEW_INTERVAL_MS`
            //（464 起 = 0：预览文字一变就打；读独立计数器 `punct_interval_ms`，DEC-077）且有新内容；
            // **只重打精解尚未覆盖的裸尾巴**（起点 `punct_tail_start`，与 `committed_len`
            // 同刻同源捕获 ⇒ 冻结前缀逐字节不变、坐标不漂移、不与精解标点打架）。
            // 无论打不打点都**只刷新显示**，不动状态机（不 reset / 不切句 / 不改 sentence_id）。
            // 显示基文本（LOCALRT-PERF-405 F-C-01 增量缓存）：`confirmed` 区 + 当前句 current 区。
            // DEC-086：影子收尾已移除，不再有 main/shadow 取长切换。
            // PREVIEW-PUNCT-464：调用方置位 `punct_sentence_end` ⇒ 本次刷新按「句子已说完」补句末标点（用后即清）。
            let sentence_end = std::mem::take(&mut punct_sentence_end);
            let raw_full: &str = display_cache.text();
            // DISPATCH-LONG-SPEECH-442：记 `(pcm 位置, 裸文本字节长)` 短历史（只在长度变化时记，保留约 4s）。
            if raw_len_history.back().map(|(_, l)| *l) != Some(raw_full.len()) {
                raw_len_history.push_back((pcm.len(), raw_full.len()));
            }
            while raw_len_history
                .front()
                .is_some_and(|(p, _)| pcm.len().saturating_sub(*p) > 4 * SAMPLE_RATE as usize)
                && raw_len_history.len() > 1
            {
                raw_len_history.pop_front();
            }
            if !raw_full.is_empty() {
                // 有新内容 = raw 比上次打点时长（`raw_len` 初值 0 且 raw 非空 ⇒ 首次恒 true）。
                let has_new = raw_full != punct_cache.raw;
                let repunct_due =
                    should_repunctuate_preview(punct_interval_ms, has_new, PUNCT_PREVIEW_INTERVAL_MS);
                let display = preview_display(
                    raw_full,
                    punctuation_engine.as_deref_mut(),
                    &mut punct_cache,
                    repunct_due,
                    punct_tail_start,
                    punct_head_chars,
                    sentence_end,
                );
                if repunct_due {
                    // 打完重置**间隔**计时（438：打点不再触碰任何静默计数器）。
                    // 🔴 346：`acc_silent_ms` 与此完全无关（动了 accuracy 的 1200ms 就不可达）。
                    punct_interval_ms = 0.0;
                }
                // STREAM-SV-463：末尾浅色字数 = 显示文本里「已确认区 + 当前句稳定前缀」之后的字数
                //（整段都稳定 ⇒ 0，连同标点引擎补在末尾的标点也算已定）。
                let stable_raw = display_cache.confirmed().len() + sim_stable;
                let tentative = if stable_raw >= raw_full.len() {
                    0
                } else {
                    display.chars().count()
                        - display_chars_for_raw_prefix(&display, raw_full, stable_raw)
                };
                if display != last_display || tentative != last_tentative {
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
                    on_result(&display, &state.display_words(), tentative);
                    last_tentative = tentative;
                    last_display = display;
                    // PREVIEW-PUNCT-LIVE-438（R1）：与 last_display **同刻**记录其覆盖的 raw 字节长，
                    // 供下次派发捕获双坐标（committed_len 字符数 + 本值必须描述同一个显示串）。
                    last_display_raw_len = raw_full.len();
                }
            }
        }};
    }

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
        // PREVIEW-PUNCT-LIVE-438：预览打点**间隔**计时无条件推进（与语音/静默无关）；
        // 只在实际完成一次打点时归零（见下方打点处 `if repunct_due`）。
        punct_interval_ms += chunk_ms;
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
        // VAD-REUSE-458：进行中语音段被实时 VAD 判出的位置（本 chunk 起点）；段结束即清。
        if vad_speech {
            vad_seg_onset.get_or_insert(pcm.len());
        } else {
            vad_seg_onset = None;
        }
        if let Some(t0v) = t_vad0 {
            let dt_ms = t0v.elapsed().as_secs_f64() * 1000.0;
            vad_total_ms += dt_ms;
            vad_chunks += 1;
            if dt_ms > vad_max_ms {
                vad_max_ms = dt_ms;
            }
        }
        // GATE-TIMING-ONLY-392：**时序**（停顿计时 / 派发时机 / 337 边界 b）继续用
        // **过门结果** `has_speech`；**内容去留**（`speech_since_last_reset` / `acc_pending_has_speech` /
        // `*_done_for_pause` 复位）改用 **VAD 原始判定** `vad_speech` ——
        // 门误判时最坏只是停顿判断不准，**绝不丢录音人的话**（BUILD-390：门误判 ⇒ 342 丢流式文本 +
        // 298 不派发 ⇒ 预览/最终输出卡在 0 或前半段）。代价：VAD 判人声的背景说话可能被识别进结果。
        // 🔴 VAD 不可用时 `vad_speech` == 音量兜底判定 ⇒ 整条判定与 384 逐位相同。
        // 🔴 PREVIEW-PUNCT-LIVE-438：预览打点**已不在本判定的时序消费者之列**（改按 3.5s 间隔、
        //    与 has_speech 无关，见 `punct_interval_ms`）。
        if !has_speech {
            if vad_on && prev_has_speech && !vad_speech {
                // LOCALRT-VAD-SILENCE-384/389：VAD 自身「人声→无人声」翻转（silero 过 min_silence 才报）
                // ⇒ 把这段已静时间补记回来。只有**已确认段**结束才走到这里（prev_has_speech=true）。
                acc_silent_ms = localrt_vad_seed_ms();
            } else {
                // 346：acc 专用计数器累加（438 起打点只动 punct_interval_ms，不存在会被标点清零的
                // 静默计数器）。
                acc_silent_ms += chunk_ms;
            }
        } else {
            // 392 主控验收补：「本轮停顿已派发」的复位属于**时序**，必须跟静默计时同源（has_speech）。
            // 若随 vad_speech 复位：背景人声被门拒 ⇒ acc_silent_ms 持续累加不清零，而 done 标记每个 chunk
            // 被清 ⇒ 静默已 ≥1200ms ⇒ **每个 chunk（~10ms）派发一次**，串行解码队列被碎片窗口淹没。
            // 改回 has_speech：背景人声只随一次派发送出，之后要等录音人开口。
            // 392 主控验收补（续）：done 复位写在计时清零**之前**，使 346 护栏（`acc_silent_ms = 0.0;`
            // 归零紧邻其上一行、其后 3 行内出现 speech_since_last_reset）与 392 护栏（done 复位在
            // else 分支）同时成立。
            acc_done_for_pause = false;
            // 407：恢复说话 ⇒ 复位长静默 latch（同一停顿只发一次；下次停顿可再发）。
            long_silence_done_for_pause = false;
            acc_silent_ms = 0.0;
        }
        if vad_speech {
            speech_since_last_reset = true;
            acc_pending_has_speech = true;
            if acc_pending_speech_start.is_none() {
                acc_pending_speech_start = Some(pcm.len());
            }
            // LOCALRT-ENDPOINT-EMPTY-342（392 契约变更）：本句 VAD 判有人声 ⇒ 342 后续 endpoint 文本不丢；
            // 门只管时序（见上）；门误判时最坏只是停顿判断不准，**绝不丢录音人的话**。
        }
        // FALLBACK-EXCLUDE-FARFIELD-446：记录本 chunk 的远场区间（pcm 此刻尚未追加本 chunk）。
        if vad_speech && !has_speech {
            let (a, b) = (pcm.len(), pcm.len() + chunk.len());
            match far_timeline.last_mut() {
                Some(last) if last.1 == a => last.1 = b,
                _ => far_timeline.push((a, b)),
            }
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

        let chunk_start = pcm.len();
        pcm.extend_from_slice(&chunk);
        // STREAM-SV-463：当前句的人声 / 静默推进（VAD 原始判定，与 speech_since_last_reset 同口径）。
        if vad_speech {
            sim_dirty = true;
            sim_silence_ms = 0.0;
            // 本句首次听到人声 ⇒ SIM_FIRST_MS 后首解。
            sim_next_at.get_or_insert(chunk_start + ms_samples(SIM_FIRST_MS));
        } else {
            sim_silence_ms += chunk_ms;
        }
        // 句首静音不入窗：本句尚无人声时，句起点跟着往后挪，只在开口前留 ONGOING_ONSET_MARGIN
        //（实时 VAD 判出开口比真实起点晚，458 同一余量）。
        if !speech_since_last_reset {
            sim_start = sim_start.max(pcm.len().saturating_sub(ONGOING_ONSET_MARGIN));
        }
        if !first_accept_seen {
            first_accept_seen = true;
            t_accept_ms = t0.elapsed().as_secs_f64() * 1000.0;
            log::info!(
                "[Latency] local_stream first_audio at +{:.1}ms",
                t_accept_ms
            );
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
                late_bound = None;
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
                    late_bound = Some((seg, cur));
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
                    late_bound = Some((seg, cur));
                }
            }
        }

        // FIX-LATE-STREAM-TAIL-445（Gavin 2026-09-26「最终输出文本和预览文本有差别，证明没有及时回灌刷新」）：
        // 337 边界按「文本停止增长 500ms」冻结，但流式模型偶有更长的滞后（15:01 端测：冻结后 0.2~0.8s 又吐
        // 「算了」）。冻结后 VAD 未再听到语音 ⇒ 这些字只能来自已派发的那片音频，精解已对它负责 ⇒
        // 归入该片：边界后移并重登记（主线程 `on_bound` 对最新已渲染片重渲），438 打点冻结坐标与下一片
        // 流式起点同步后移。开口（`acc_pending_has_speech`）或新派发（`bound_seg`）即停止跟踪。
        let (next_late, absorb) = late_bound_step(
            late_bound,
            acc_pending_has_speech,
            bound_seg.is_some(),
            last_display.chars().count(),
        );
        if let Some((seg, cur)) = absorb {
            if log::log_enabled!(log::Level::Debug) {
                log::debug!(
                    "[DBG-445] late stream chars absorbed: seg={} committed_len {}→{}",
                    seg,
                    late_bound.map(|(_, l)| l).unwrap_or(0),
                    cur
                );
            }
            on_reflow_commit(seg, Some(cur));
            punct_head_chars = cur;
            punct_tail_start = last_display_raw_len;
            acc_prev_committed = cur;
        }
        late_bound = next_late;

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
            // 346：必须用 acc 专用计数器（438 起打点只动 punct_interval_ms，不再清任何静默计数器）。
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
                        "[LocalRT-DBG-298] seg dispatch #{}: silence={:.0}ms seg_audio={:.2}s pcm_pos={} reason=silence",
                        acc_seg_index,
                        acc_silent_ms,
                        pending as f32 / SAMPLE_RATE as f32,
                        acc_dispatched_end
                    );
                }
                // ACC-PREVIEW-REFLOW-325：记下**派发当刻浮层已显示文本的字符数**（committed_len）。
                // 🔴 用 `last_display`（实际已上屏、含标点的串，与镜像 `last_streaming_text` 同源），
                //    不用 `state.display_text()`（裸文本，标点差会造成回填边界偏移）。
                // STREAM-SV-463：派发点即冻结点 —— 当前句结果确认（此后不再改写），新句从此刻起。
                // VAD 在上次重解后又听到人声（VAD 判停比门晚）⇒ 先补解到此刻，免得句末最后一个字被劈进下一句
                //（端到端实测「淘汰。赛。」）。补解改写的字由下面的刷新显示上屏，committed_len 在刷新**之后**数
                // ⇒ 与冻结内容同源（PREVIEW-PUNCT-464 起派发前必刷新，463 初版不补解的顾虑不再成立）。
                if sim_dirty {
                    sim_decode_text!();
                }
                sim_freeze!(pcm.len(), vad_speech);
                // PREVIEW-PUNCT-464：停顿 ≥1.2s = 这句说完了 ⇒ 补句末标点再派发（committed_len 按补完的显示数，
                // 与 last_display_raw_len 同刻同源，438 R1 双坐标不变）。
                punct_sentence_end = true;
                refresh_preview!();
                let committed_len = last_display.chars().count();
                // PREVIEW-PUNCT-LIVE-438（R1）：同刻捕获 raw 侧边界 —— 与 committed_len 描述
                // 同一个显示串（`last_display_raw_len` 随 last_display 同赋值）⇒ 3.5s 重打
                // 只打边界后的裸尾巴、冻结前缀逐字节保留（不多字、不丢字）。
                punct_head_chars = committed_len;
                punct_tail_start = last_display_raw_len;
                // 344-G：本片流式文本（失败片的填补来源），按 char 切片，字符安全。
                let seg_streaming = segment_streaming_text(&last_display, acc_prev_committed);
                acc_prev_committed = committed_len;
                // VAD-393（A2/R1）：派发由**静默 1200ms**（门 `has_speech`）触发，不保证派发当刻
                // VAD 段已结束（门拒背景人声 / 误拒录音人轻声 / 本句与背景声无缝相接 ⇒ VAD 仍在段中）。
                // VAD 已结束 ⇒ 时间线完整、可用；VAD 仍在段中（进行中段尚未入队）⇒ 回退 391 自跑 VAD
                //（`dispatch_slice_ranges` 返回 None），绝不据缺失时间线剪掉这一段（= 吞字）。
                let slice_ranges = dispatch_slice_ranges(
                    vad_on,
                    vad_speech,
                    vad_seg_onset,
                    &speech_timeline,
                    &spans,
                );
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
                    slice_ranges_from_timeline(&far_timeline, &spans),
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
            acc_pending_speech_start = None;
            acc_done_for_pause = true;
        }

        // DISPATCH-LONG-SPEECH-442（回看切点）：满 10s 仍未派（没等到 120ms 静默）⇒ 用派发后兜底切片
        // **同一个函数** `vad::slice_cut_at` 在待派音频上找切点，只接受后续音频不会改变的切点：
        // [8,10]s 真停顿（取最靠近 10s）/ [10s, min(11s, 现在-1s)] 最早真停顿 / 满 12s 才允许字缝·最低能量兜底。
        // 切点在过去 ⇒ 只派 [已派到, 切点)，其后继续攒；精解覆盖边界按切点那一刻的流式文字定。
        let ms_to_samples = |ms: f32| (ms * SAMPLE_RATE as f32 / 1000.0) as usize;
        // 片起点 = 已派到位置（与派发后兜底切片同口径）；仅当前导静音 ≥8s 时改从「开口前 0.5s」起算，
        // 免得 [8,10]s 窗落进长静音里。
        let lookback_origin = match acc_pending_speech_start {
            Some(s) if s.saturating_sub(acc_dispatched_end) >= ms_to_samples(8000.0) => {
                s.saturating_sub(ms_to_samples(500.0))
            }
            _ => acc_dispatched_end,
        };
        if acc_cfg.enabled
            && acc_pending_has_speech
            && acc_pending_speech_start.is_some()
            && pcm.len().saturating_sub(lookback_origin) >= ms_to_samples(LONG_SPEECH_LOOKBACK_MS)
            && (pcm.len().saturating_sub(long_lookback_checked_at) as f32 * 1000.0
                / SAMPLE_RATE as f32)
                >= LONG_SPEECH_CHECK_EVERY_MS
        {
            long_lookback_checked_at = pcm.len();
            let start = acc_dispatched_end;
            let upper = (lookback_origin + ms_to_samples(LONG_SPEECH_LOOKBACK_UPPER_MS))
                .min(pcm.len().saturating_sub(ms_to_samples(1000.0)));
            let allow_fallback =
                pcm.len().saturating_sub(lookback_origin) >= ms_to_samples(LONG_SPEECH_FALLBACK_MS);
            if let Some((cut, kind, pause_ms)) =
                super::vad::slice_cut_at(&pcm, lookback_origin, upper, allow_fallback)
            {
                if cut > start && cut < pcm.len() {
                    // 切点那一刻（+流式滞后补偿）的裸文本长度 ⇒ 显示文本里的覆盖字符数。
                    let at = (cut + ms_to_samples(LONG_SPEECH_TEXT_LAG_MS)).min(pcm.len());
                    let raw_now = display_cache.text();
                    // STREAM-SV-463：当前句结果带逐 token 时间 ⇒ 起始早于切点的 token 归已派发片（精确到字）；
                    // 没有时间戳才回退「切点后 LONG_SPEECH_TEXT_LAG_MS 那一刻的文字长度」。
                    let by_time = (cut > sim_start)
                        .then(|| {
                            split_bytes_at_time(
                                &sim_marks,
                                (cut - sim_start) as f32 / SAMPLE_RATE as f32,
                            )
                        })
                        .flatten()
                        .map(|b| display_cache.confirmed().len() + b);
                    if log::log_enabled!(log::Level::Debug) {
                        log::debug!(
                            "[STREAM-SV-463] lookback split by {}",
                            if by_time.is_some() {
                                "token time"
                            } else {
                                "text length history"
                            }
                        );
                    }
                    let raw_at = by_time
                        .or_else(|| {
                            raw_len_history
                                .iter()
                                .rev()
                                .find(|(p, _)| *p <= at)
                                .map(|(_, l)| *l)
                        })
                        .unwrap_or(punct_tail_start)
                        .max(punct_tail_start)
                        .min(raw_now.len());
                    let committed_len =
                        display_chars_for_raw_prefix(&last_display, raw_now, raw_at)
                            .max(acc_prev_committed);
                    let (padded, spans) =
                        build_dispatch_segment_with_spans(start, cut - start, pcm.len(), &pcm);
                    if !padded.is_empty() {
                        if log::log_enabled!(log::Level::Debug) {
                            log::debug!(
                                "[LocalRT-DBG-298] seg dispatch #{}: reason=lookback kind={} pause_ms={} seg_audio={:.2}s pcm_pos={} committed_len={}",
                                acc_seg_index,
                                kind,
                                pause_ms,
                                (cut - start) as f32 / SAMPLE_RATE as f32,
                                start,
                                committed_len
                            );
                        }
                        punct_head_chars = committed_len;
                        punct_tail_start = raw_at;
                        let seg_streaming: String = last_display
                            .chars()
                            .skip(acc_prev_committed)
                            .take(committed_len.saturating_sub(acc_prev_committed))
                            .collect();
                        acc_prev_committed = committed_len;
                        // STREAM-SV-463：回看切点即冻结点 —— 当前句结果在 raw_at 处一分为二：前半确认
                        //（对应已派发的 [start, cut)，此后不再改写），后半暂作新句显示、下一 chunk 按
                        // [cut, now) 重解替换。显示文本逐字不变 ⇒ 上面按 last_display 算的坐标照样成立。
                        let cut_in_hyp = floor_char_boundary(
                            &sim_hyp,
                            raw_at.saturating_sub(display_cache.confirmed().len()),
                        );
                        let tail = sim_hyp.split_off(cut_in_hyp);
                        sim_stable = sim_stable.saturating_sub(cut_in_hyp).min(tail.len());
                        sim_confirm!();
                        sim_hyp = tail;
                        state.on_result(sentence_id, &sim_hyp, false, &[]);
                        display_cache.on_current(&sim_hyp);
                        // 新句 = [cut, now)，仍在说话 ⇒ 下一 chunk 即重解。
                        sim_start = cut;
                        sim_dirty = true;
                        sim_next_at = Some(pcm.len());
                        // 切点在过去、VAD 可能仍在段中 ⇒ 进行中段按判定位置保守补齐（VAD-REUSE-458，不吞字）。
                        let slice_ranges = dispatch_slice_ranges(
                            vad_on,
                            vad_speech,
                            vad_seg_onset,
                            &speech_timeline,
                            &spans,
                        );
                        on_segment(
                            acc_seg_index,
                            committed_len,
                            padded,
                            seg_streaming,
                            slice_ranges,
                            slice_ranges_from_timeline(&far_timeline, &spans),
                        );
                        // 边界已知（切点那一刻的流式文字）⇒ 直接登记，不走 337 自适应定界。
                        on_reflow_commit(acc_seg_index, Some(committed_len));
                        acc_seg_index += 1;
                    }
                    acc_dispatched_end = cut;
                    // 切点之后的剩余音频仍在说话 ⇒ 仍有语音，长段计时从切点重新起算。
                    acc_pending_has_speech = true;
                    acc_pending_speech_start = Some(cut);
                }
            }
        }

        // LOCALRT-TAIL-WINDOW-407：长静默（≥1900ms 无新语音）**只发一次**「末尾组窗」信号。
        // 与 1200ms 常规派发并存：1200 已把片派出；1900 让 main 侧据遗留的待覆盖片组末尾窗并即时回灌
        //（Gavin：不必等停止键）。恢复说话时由上方 else 分支复位 latch。
        if should_signal_long_silence(acc_cfg.enabled, acc_silent_ms, long_silence_done_for_pause) {
            long_silence_done_for_pause = true;
            if log::log_enabled!(log::Level::Debug) {
                log::debug!(
                    "[LocalRT-DBG-407] long silence {:.0}ms: signal tail compose (pcm_pos={})",
                    acc_silent_ms,
                    pcm.len()
                );
            }
            on_long_silence(pcm.len());
        }

        // STREAM-SV-463：重解 / 冻结放在本轮所有派发**之后**、刷新显示之前 —— 派发时的显示缓存与 last_display
        // 是同一份文字（双坐标同源，438 R1）；重解若先于派发，改写过的字会让 committed_len 与切分位置错位。
        // 句末冻结 —— 当前句有人声、其后连续无人声达 SIM_SENTENCE_END_MS（精解关闭时的切句；
        // 精解开启时静默 1200ms 派发已先冻结，这里只剩空句）。冻结前若有未解的人声先补解一次。
        if speech_since_last_reset && sim_silence_ms >= SIM_SENTENCE_END_MS {
            if sim_dirty {
                sim_decode_text!();
            }
            sim_freeze!(pcm.len(), false);
            // PREVIEW-PUNCT-464：句子说完 ⇒ 本轮刷新补句末标点。
            punct_sentence_end = true;
        } else if speech_since_last_reset
            && pcm.len().saturating_sub(sim_start) >= ms_samples(SIM_WINDOW_MAX_MS)
        {
            // 窗口硬上限：在上次重解覆盖处冻结（其后的音频归新句，不丢）。
            sim_freeze!(sim_decoded_end, true);
        }
        // STREAM-SV-463：说话中按步长重解当前句（纯静音不重解）。
        if sim_dirty && sim_next_at.is_some_and(|t| pcm.len() >= t) {
            sim_decode!();
        }

        refresh_preview!();
    }

    // STREAM-SV-463：录音结束 ⇒ 当前句按最后一个样本收尾重解，并按循环内同一口径刷新一次显示 ——
    // 尾片派发捕获的 committed_len / 流式文本由此包含句末最后几个字（paraformer 时代此处是 flush）。
    // 宏里推进的计时 / 首次标记等在函数末尾不再读取 ⇒ 仅此处豁免 unused_assignments。
    #[allow(unused_assignments)]
    let () = {
        if speech_since_last_reset && sim_dirty {
            sim_decode!();
        }
        // PREVIEW-PUNCT-464：录音结束 = 最后一句说完 ⇒ 补句末标点。
        punct_sentence_end = true;
        refresh_preview!();
    };

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
            // PREVIEW-PUNCT-LIVE-438（R1）：尾片派发同样同刻捕获双坐标（与中途派发同口径）
            // ⇒ 收尾强制打点的尾巴起点推进到本边界（尾巴为空 ⇒ 本地不再补终止符，
            //    按任务第 3 条由 accuracy 回灌 / 最终输出给出；acc 关闭时坐标恒 0 ⇒ 仍整段打）。
            punct_head_chars = committed_len;
            punct_tail_start = last_display_raw_len;
            // 344-G：尾片流式文本（同 mid-loop，按 char 切片）。
            let seg_streaming = segment_streaming_text(&last_display, acc_prev_committed);
            // VAD-393（A2/R1）：尾片前已 `flush_speech` ⇒ 无进行中段 ⇒ `mid_speech=false`
            //（`vad_on=false` ⇒ None，回退 391 自跑 VAD，与中途派发同口径）。
            let slice_ranges = dispatch_slice_ranges(vad_on, false, None, &speech_timeline, &spans);
            on_segment(
                acc_seg_index,
                committed_len,
                padded,
                seg_streaming,
                slice_ranges,
                slice_ranges_from_timeline(&far_timeline, &spans),
            );
        }
        // 尾片是最后一次派发：函数即将返回，无需再推进 `acc_dispatched_end`/`acc_seg_index`
        // （推进了也无人读 ⇒ 触发 unused_assignments）。
    }

    // LOCALRT-SEAM-337（补 1，自适应版）：松手收尾若仍有未冻结边界，按 a 冻结（收尾重解已完成，
    // 预览文字不会再变）⇒ 末态带 acc 前缀。已冻结或从未派发则不动。
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
    // STREAM-SV-463：当前句最终结果 = 收尾重解的结果（上面已解过；空 ⇒ 预览保持已显示文本，绝不回退）。
    let final_seg = sim_hyp.clone();
    // LOCALRT-FIRSTCHAR-282：最终（含标点）预览全文；供调用方以 `StreamingFinalPreview` 收尾显示。
    // （239-B 丢弃本预览文本、以 pcm 走 accuracy 2pass；此返回值只服务于收尾显示。）
    let mut final_preview = last_display.clone();
    if !final_seg.is_empty() {
        // STREAM-SV-463：显示缓存的当前句区已是 `final_seg`（收尾重解时写入），不再重复写。
        // 收尾强制打点一次（录音结束 = 真边界，不属「句中断点」）。
        // PREVIEW-PUNCT-LIVE-438（R1）：仍然只打「精解边界之后的尾巴」—— 尾片已派发时坐标
        // 已推进到 raw 末尾 ⇒ 尾巴为空、本地不补终止符（由回灌/最终输出给出）；
        // acc 未启用 / 未派发 ⇒ 坐标为 0 ⇒ 整段重打，关闭前的预览照常带标点。
        let raw_full = display_cache.text();
        let display = preview_display(
            raw_full,
            punctuation_engine.as_deref_mut(),
            &mut punct_cache,
            true,
            punct_tail_start,
            punct_head_chars,
            true,
        );
        if !display.is_empty() {
            // 录音结束 = 全部已定 ⇒ 浅色尾巴清零。
            if display != last_display || last_tentative != 0 {
                on_result(&display, &state.display_words(), 0);
            }
            final_preview = display;
        }
    }
    if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "[STREAM-SV-463] preview decodes={} avg_ms={:.0} max_ms={:.0} audio_s={:.1}",
            sim_decodes,
            sim_decode_ms_total / sim_decodes.max(1) as f64,
            sim_decode_ms_max,
            pcm.len() as f32 / SAMPLE_RATE as f32
        );
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

    /// 393-R1 / VAD-REUSE-458：`dispatch_slice_ranges` 四种组合 —— `!vad_on` ⇒ `None`（回退自跑）；
    /// `mid_speech` ⇒ 时间线 + 「最后完整段结束 → 片尾」保守补齐（不再回退）。
    #[test]
    fn ts393_dispatch_slice_ranges_fallback_when_mid_speech() {
        let timeline = [(100usize, 200usize)];
        let spans = [(0usize, 400usize, 0usize)];
        assert!(dispatch_slice_ranges(false, false, None, &timeline, &spans).is_none());
        assert!(dispatch_slice_ranges(false, true, None, &timeline, &spans).is_none());
        assert_eq!(
            dispatch_slice_ranges(true, true, None, &timeline, &spans),
            Some(vec![vec![(100, 200), (200, 400)]])
        );
        assert_eq!(
            dispatch_slice_ranges(true, false, None, &timeline, &spans),
            Some(vec![vec![(100, 200)]])
        );
    }

    /// VAD-REUSE-458：说话中途派发 —— 进行中段一定在最后完整段之后开始 ⇒ 补齐区间必须**覆盖**
    /// 「最后完整段结束 → 片尾」全部样本（多片 / 带前置 pad / 时间线为空 / 时间线乱序各一例），且不少于非中途结果。
    #[test]
    fn t458_mid_speech_ranges_cover_ongoing_tail() {
        // 两个子片：[0,300) 与 [300,700)，第二片带 50 样本前置 pad。
        let spans = [(0usize, 300usize, 0usize), (300usize, 700usize, 50usize)];
        let timeline = [(20usize, 120usize), (150usize, 250usize)];
        let got = dispatch_slice_ranges(true, true, None, &timeline, &spans).unwrap();
        assert_eq!(got[0], vec![(20, 120), (150, 250), (250, 300)]);
        assert_eq!(
            got[1],
            vec![(50, 450)],
            "第二片整片都在进行中段之后 ⇒ 全算语音（片内坐标含 pad）"
        );
        let not_mid = dispatch_slice_ranges(true, false, None, &timeline, &spans).unwrap();
        for (a, b) in got.iter().zip(&not_mid) {
            let cover = |v: &Vec<(usize, usize)>| v.iter().map(|&(s, e)| e - s).sum::<usize>();
            assert!(cover(a) >= cover(b), "中途补齐不得少于非中途结果");
        }
        // 时间线为空（本次录音还没有完整段）⇒ 整片算语音。
        assert_eq!(
            dispatch_slice_ranges(true, true, None, &[], &[(0usize, 300usize, 0usize)]),
            Some(vec![vec![(0, 300)]])
        );
        // 有「开始说话」判定位置 ⇒ 从 onset − 0.5s 起算（长停顿剪掉），但不早于最后完整段结束。
        let onset = 30_000usize;
        assert_eq!(
            dispatch_slice_ranges(
                true,
                true,
                Some(onset),
                &[],
                &[(0usize, 40_000usize, 0usize)]
            ),
            Some(vec![vec![(onset - super::ONGOING_ONSET_MARGIN, 40_000)]])
        );
        assert_eq!(
            dispatch_slice_ranges(
                true,
                true,
                Some(onset),
                &[(1_000usize, 25_000usize)],
                &[(0usize, 40_000usize, 0usize)]
            ),
            Some(vec![vec![(1_000, 25_000), (25_000, 40_000)]]),
            "onset − 余量 早于最后完整段结束 ⇒ 取完整段结束"
        );
        // 时间线未按时间排序也取最晚结束点。
        assert_eq!(
            dispatch_slice_ranges(
                true,
                true,
                None,
                &[(150usize, 250usize), (20, 120)],
                &[(0usize, 300usize, 0usize)]
            ),
            Some(vec![vec![(150, 250), (20, 120), (250, 300)]])
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
        let ranges = dispatch_slice_ranges(true, mid_speech, None, timeline, &spans);
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
                    // VAD-REUSE-458：说话中途的片是保守补齐（会多留静音）⇒ 只要求不比自跑 VAD 少（不吞字）。
                    let over = if mid_speech { vad_secs - tl_secs } else { diff };
                    if over > 0.3 {
                        fails.push(format!(
                            "seg={seg} sub={k} mid={mid_speech} |tl-vad|={diff:.2}>0.3 (tl={tl_secs:.2} vad={vad_secs:.2})"
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

    /// VAD-REUSE-458：真模型 E2E —— **说话进行中**每 ~4s 切一片（模拟长句回溯派发），片内区间用
    /// `dispatch_slice_ranges(.., mid_speech=true, ..)` 保守补齐；逐片对照整片自跑 VAD：自跑 VAD 判为语音的样本
    /// 必须几乎全被覆盖（漏 ≤0.1s），剪后时长不得比自跑短 >0.3s（不吞字）。
    #[test]
    #[ignore = "requires silero model + full.wav; cargo test --bin feiyin-ime -- --ignored --nocapture t458_mid_speech"]
    fn t458_mid_speech_dispatch_e2e() {
        const RATE: usize = 16_000;
        const CHUNK: usize = 160;
        const CUT_EVERY: usize = 4 * RATE;
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = root.join("models");
        let wav = root.join("collab/research/audio-real-gavin/processed/full.wav");
        let Some(wave) = sherpa_onnx::Wave::read(wav.to_str().unwrap()) else {
            eprintln!("skip: full.wav 缺失");
            return;
        };
        let vad = VadSegmenter::try_new_for_local_silence(&model_dir).expect("silero VAD");
        let probe = VadSegmenter::try_new_for_local_trim(&model_dir).expect("silero trim VAD");
        let mut audio = vec![0.0f32; 2 * RATE];
        audio.extend_from_slice(wave.samples());
        audio.extend(std::iter::repeat(0.0f32).take(2 * RATE));
        let pad = (crate::transcription::vad::LOCALRT_TRIM_PAD_SECS * RATE as f32) as usize;
        let mut timeline: Vec<(usize, usize)> = Vec::new();
        let mut dispatched_end = 0usize;
        let mut onset: Option<usize> = None;
        let (mut n_mid, mut worst_uncovered, mut worst_short) = (0usize, 0f32, 0f32);
        let mut fails: Vec<String> = Vec::new();
        for off in (0..audio.len()).step_by(CHUNK) {
            let end = (off + CHUNK).min(audio.len());
            let vad_speech = vad.feed_speech(&audio[off..end], &mut timeline);
            if vad_speech {
                onset.get_or_insert(off);
            } else {
                onset = None;
            }
            if !(vad_speech && end - dispatched_end >= CUT_EVERY) {
                continue;
            }
            let spans = [(dispatched_end, end, 0usize)];
            let slice = &audio[dispatched_end..end];
            let tl = dispatch_slice_ranges(true, true, onset, &timeline, &spans)
                .expect("中途派发不应回退")[0]
                .clone();
            let vr = probe.speech_ranges(slice);
            // 自跑 VAD 语音样本中未被补齐区间覆盖的部分。
            let mut uncovered = 0usize;
            for &(a, b) in &vr {
                let mut covered = 0usize;
                for &(x, y) in &tl {
                    let (s0, e0) = (a.max(x), b.min(y));
                    if s0 < e0 {
                        covered += e0 - s0;
                    }
                }
                uncovered += (b - a).saturating_sub(covered);
            }
            let tl_secs =
                crate::transcription::trim_to_speech(slice, &tl, pad).len() as f32 / RATE as f32;
            let vad_secs =
                crate::transcription::trim_to_speech(slice, &vr, pad).len() as f32 / RATE as f32;
            let unc = uncovered as f32 / RATE as f32;
            worst_uncovered = worst_uncovered.max(unc);
            worst_short = worst_short.max(vad_secs - tl_secs);
            println!(
                "mid #{n_mid} slice={:.2}s tl_out={tl_secs:.2} vad_out={vad_secs:.2} uncovered={unc:.2}s",
                slice.len() as f32 / RATE as f32
            );
            if unc > 0.1 || vad_secs - tl_secs > 0.3 {
                fails.push(format!(
                    "mid #{n_mid}: uncovered={unc:.2} short={:.2}",
                    vad_secs - tl_secs
                ));
            }
            n_mid += 1;
            dispatched_end = end;
        }
        println!("t458 mid-speech: {n_mid} 片，最大漏覆盖 {worst_uncovered:.2}s，最大少剪出 {worst_short:.2}s");
        assert!(n_mid >= 5, "应有足够的中途派发样本，实得 {n_mid}");
        assert!(fails.is_empty(), "中途派发保守补齐漏语音：{fails:?}");
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
    /// **中途派发**只以 `dispatch_slice_ranges(vad_on, vad_speech, vad_seg_onset,` 调用（VAD 进行中段 ⇒
    /// VAD-REUSE-458 按判定位置保守补齐）；**尾片**只以 `dispatch_slice_ranges(vad_on, false, None,` 调用
    ///（flush 后无进行中段）。比对前去掉全部空白（不受 rustfmt 换行影响）。
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
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        // 录音中的派发点有两个：静默 1200ms 派发 + 442 满 10s 回看派发，均须以 vad_speech + 判定位置传入。
        assert_eq!(
            code.matches("dispatch_slice_ranges(vad_on,vad_speech,vad_seg_onset,")
                .count(),
            2,
            "录音中两个派发点（静默 / 442 回看）都必须以 vad_speech + vad_seg_onset 传入（进行中段保守补齐）"
        );
        assert_eq!(
            code.matches("dispatch_slice_ranges(vad_on,false,None,")
                .count(),
            1,
            "尾片派发必须恰 1 处以 false 传入（flush 后无进行中段）"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_dispatch_segment, build_punct_prefix, chunk_has_speech, find_tail_cut,
        localrt_vad_seed_ms, preview_display, punct_cache_reuse, punct_head, seed_usable,
        segment_streaming_text, should_dispatch_acc, should_dispatch_tail,
        should_repunctuate_preview, should_signal_long_silence, DisplayCache, EnergySmoother,
        PunctPreviewCache, SegmentGate, SegmentPeakLevel, LOCALRT_VAD_MIN_SILENCE_SECS,
        LONG_SILENCE_TAIL_MS, PUNCT_PREVIEW_INTERVAL_MS, SAMPLE_RATE,
    };

    // ========================================================================
    // LOCALRT-TAIL-WINDOW-407：长静默触发（只发一次 / 恢复复位）+ 尾部字缝切点
    // ========================================================================

    /// 407 触发：<1900 不发；≥1900 且未发过 ⇒ 发；已发过 ⇒ 不再发；未启用 ⇒ 不发。
    #[test]
    fn ts407_long_silence_triggers_once_then_resets() {
        assert!(!should_signal_long_silence(true, 1200.0, false));
        assert!(!should_signal_long_silence(true, 1899.0, false));
        assert!(should_signal_long_silence(true, 1900.0, false));
        assert!(should_signal_long_silence(true, 5000.0, false));
        // 同一停顿已发过 ⇒ 不再发（只发一次）。
        assert!(!should_signal_long_silence(true, 5000.0, true));
        // 未启用 ⇒ 不发。
        assert!(!should_signal_long_silence(false, 5000.0, false));
        assert_eq!(LONG_SILENCE_TAIL_MS, 1900.0);
    }

    /// 407 字缝：有字缝 ⇒ (中心, true)（落在回溯区间内的静音处）；无字缝 ⇒ (len-back, false)；
    /// 前片不足 back ⇒ (0, false)（整片）。
    #[test]
    fn ts407_find_tail_cut_gap_and_fallback() {
        let rate = SAMPLE_RATE as usize;
        // 前片 6s：4.0~4.2s 静音（字缝），其余 440Hz 正弦（幅度 0.5）。
        let mut prev = vec![0f32; 6 * rate];
        for (i, s) in prev.iter_mut().enumerate() {
            let t = i as f32 / rate as f32;
            *s = if (4.0..=4.2).contains(&t) {
                0.0
            } else {
                (t * 440.0 * std::f32::consts::TAU).sin() * 0.5
            };
        }
        let (cut, found) = find_tail_cut(&prev, 2 * rate);
        assert!(found, "有静音字缝应判 found");
        let cut_secs = cut as f32 / rate as f32;
        assert!(
            (4.0..4.25).contains(&cut_secs),
            "切点应落在 4.0~4.2s 字缝附近，实测 {cut_secs:.3}s"
        );

        // 全程有声（无字缝）⇒ 回落 len-back、found=false。
        let uniform: Vec<f32> = (0..6 * rate)
            .map(|i| (i as f32 * 0.13).sin() * 0.5)
            .collect();
        assert_eq!(
            find_tail_cut(&uniform, 2 * rate),
            (4 * rate, false),
            "无字缝 ⇒ 回落 len-back"
        );

        // 前片不足 back ⇒ 整片（0）、found=false。
        assert_eq!(find_tail_cut(&vec![0.3f32; rate], 2 * rate), (0, false));
    }

    /// 407 真模型：真实连续语音（>10s）+ 3s 尾静默 ⇒ 长静默信号应在**静默满 ~1900ms** 时触发一次
    /// （不依赖停止键）；打印触发时刻（wall）与静默起点（音频时间）。
    /// 运行：`cargo test --bin feiyin-ime -- --ignored --nocapture poc_tailwindow_407`
    #[test]
    #[ignore = "requires preview model; cargo test --bin feiyin-ime -- --ignored --nocapture poc_tailwindow_407"]
    fn poc_tailwindow_407() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let Ok(recognizer) = super::create_local_stream_recognizer(&root.join("models")) else {
            eprintln!("skip: 预览 SenseVoice 模型不在位");
            return;
        };
        let wav = root.join("collab/research/audio-real-gavin/processed/full.wav");
        let Some(w) = sherpa_onnx::Wave::read(wav.to_str().unwrap()) else {
            eprintln!("skip: full.wav 不在位");
            return;
        };
        // 12s 真实语音 + 3s 尾静默。
        let mut audio: Vec<f32> = w.samples().iter().copied().take(12 * 16_000).collect();
        let speech_secs = audio.len() as f32 / 16_000.0;
        audio.extend(std::iter::repeat(0f32).take(3 * 16_000));
        let (tx, rx) = crossbeam_channel::unbounded::<Vec<f32>>();
        let feeder = std::thread::spawn(move || {
            for c in audio.chunks(1_600) {
                let _ = tx.send(c.to_vec());
            }
        });
        let t0 = std::time::Instant::now();
        let mut signal_ms: Option<f64> = None;
        let _ = super::transcribe_streaming_local(
            rx,
            &recognizer,
            None,
            None,
            0.01,
            "",
            |_text, _words, _tentative| {},
            super::AccDispatchConfig::new(),
            |_a, _b, _c, _d, _e, _f| {},
            |_pcm_pos| {
                if signal_ms.is_none() {
                    signal_ms = Some(t0.elapsed().as_secs_f64() * 1000.0);
                }
            },
            |_a, _b| {},
        );
        feeder.join().ok();
        println!(
            "\n[407] 语音 {speech_secs:.1}s + 尾静默 3s ⇒ 长静默信号于喂入后 {:.0}ms（音频时间≈静默起点+1900ms）",
            signal_ms.unwrap_or(f64::NAN)
        );
        assert!(signal_ms.is_some(), "长静默信号必须触发一次");
    }
    use std::time::Duration;

    /// STREAM-SV-463：稳定前缀 = 两次重解的公共前缀，按 char 边界（中文不切半个字）。
    #[test]
    fn sv463_stable_prefix_bytes_char_safe() {
        use super::stable_prefix_bytes as sp;
        assert_eq!(sp("今天天气", "今天天晴"), "今天天".len());
        assert_eq!(sp("", "abc"), 0);
        assert_eq!(sp("abc", "abc"), 3);
        assert_eq!(sp("我们", "你们"), 0);
        assert_eq!(sp("hello wor", "hello world"), 9);
        assert_eq!(sp("今天天气不错", "今天"), "今天".len());
    }

    /// STREAM-SV-463：只有两侧都是拉丁字母 / 数字才补空格（中文、标点结尾都不补）。
    #[test]
    fn sv463_sentence_join_space_only_between_latin() {
        use super::sentence_join_space as j;
        assert!(j("hello", "world"));
        assert!(j("version 2", "3 times"));
        assert!(!j("你好", "world"));
        assert!(!j("hello", "世界"));
        assert!(!j("", "world"));
        assert!(!j("hello.", "world"));
        assert!(!j("hello", ""));
    }

    /// STREAM-SV-463：token 顺序对到文本（`▁` = 词首空格）；任何一处对不上 ⇒ 整体放弃。
    #[test]
    fn sv463_token_text_ends_align_or_give_up() {
        use super::token_text_ends as te;
        let toks = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            te("今天天气", &toks(&["今", "天天", "气"]), &[0.1, 0.3, 0.5]),
            vec![(0.1, 3), (0.3, 9), (0.5, 12)]
        );
        assert_eq!(
            te(
                "hello world",
                &toks(&["\u{2581}hel", "lo", "\u{2581}world"]),
                &[0.0, 0.2, 0.6]
            ),
            vec![(0.0, 3), (0.2, 5), (0.6, 11)]
        );
        assert!(
            te("今天天气", &toks(&["今", "地"]), &[0.1, 0.2]).is_empty(),
            "对不上 ⇒ 空"
        );
    }

    /// STREAM-SV-463：起始早于切点的 token 归前半；无时间戳 ⇒ None（回退旧口径）。
    #[test]
    fn sv463_split_bytes_at_time() {
        use super::split_bytes_at_time as sp;
        let marks = [(0.1, 3), (0.3, 9), (0.5, 12)];
        assert_eq!(sp(&marks, 0.4), Some(9));
        assert_eq!(sp(&marks, 0.05), Some(0), "切点在首字之前 ⇒ 前半为空");
        assert_eq!(sp(&marks, 9.0), Some(12));
        assert_eq!(sp(&[], 1.0), None);
    }

    #[test]
    fn sv463_floor_char_boundary() {
        let s = "a中b";
        assert_eq!(
            super::floor_char_boundary(s, 2),
            1,
            "落在「中」中间 ⇒ 退到其起点"
        );
        assert_eq!(super::floor_char_boundary(s, 4), 4);
        assert_eq!(super::floor_char_boundary(s, 99), s.len());
        assert_eq!(super::floor_char_boundary("", 3), 0);
    }

    /// STREAM-SV-463：预览线程数 = 物理核数（Gavin 2026-10-03「跟 CPU 内核数一样」；超线程实测更慢）。
    #[test]
    fn sv463_preview_threads_are_physical_cores() {
        let n = super::local_stream_num_threads();
        let logical = std::thread::available_parallelism()
            .map(|c| c.get())
            .unwrap_or(4);
        assert!(n >= 1, "至少 1");
        assert!(
            n as usize <= logical,
            "物理核数不会超过逻辑核数：{n} > {logical}"
        );
        assert_eq!(n as usize, super::physical_core_count());
    }

    /// STREAM-SV-463 护栏：**每个派发点都是冻结点**（下游按字符坐标回灌的前提）。
    ///
    /// **改错怎么红**：
    /// - 静默派发删掉 / 挪到 committed_len 之后的 `sim_freeze!` ⇒ 顺序断言红；
    /// - 442 回看删掉切分确认（`sim_confirm!` + `sim_start = cut;`）⇒ 红；
    /// - 尾片派发前不再收尾重解 + 刷新显示 ⇒ 红；
    /// - 生产区重新出现 paraformer 流式接口 ⇒ 红。
    #[test]
    fn guard463_every_dispatch_point_freezes() {
        let lines = ls_prod_lines();
        let find_after = |from: usize, pred: &dyn Fn(&str) -> bool| -> usize {
            (from..lines.len())
                .find(|&i| pred(&lines[i]))
                .unwrap_or(usize::MAX)
        };
        // ① 静默派发：should_dispatch_acc → sim_freeze → committed_len
        let call = find_after(0, &|l| l.starts_with("if should_dispatch_acc("));
        assert!(call != usize::MAX, "静默派发调用点缺失");
        let freeze = find_after(call, &|l| l == "sim_freeze!(pcm.len(), vad_speech);");
        let commit = find_after(call, &|l| {
            l.starts_with("let committed_len = last_display.chars().count();")
        });
        assert!(
            freeze < commit && commit != usize::MAX,
            "静默派发必须先冻结当前句、再取 committed_len（freeze={freeze} commit={commit}）"
        );
        // ② 442 回看：slice_cut_at → sim_confirm + sim_start = cut → on_segment
        let cut = find_after(0, &|l| l.contains("super::vad::slice_cut_at("));
        assert!(cut != usize::MAX, "442 回看切点调用缺失");
        let confirm = find_after(cut, &|l| l == "sim_confirm!();");
        let restart = find_after(cut, &|l| l == "sim_start = cut;");
        let seg = find_after(cut, &|l| l.starts_with("on_segment("));
        assert!(
            confirm < seg && restart < seg && seg != usize::MAX,
            "442 回看派发前必须在切点确认前半、新句从切点起（confirm={confirm} restart={restart} on_segment={seg}）"
        );
        // ③ 尾片：派发前收尾重解 + 刷新显示
        let tail = find_after(0, &|l| l.starts_with("if should_dispatch_tail("));
        assert!(tail != usize::MAX, "尾片派发调用点缺失");
        let lo = tail.saturating_sub(20);
        assert!(
            lines[lo..tail].iter().any(|l| l == "sim_decode!();")
                && lines[lo..tail].iter().any(|l| l == "refresh_preview!();"),
            "尾片派发前必须收尾重解并刷新显示（committed_len / 流式文本含句末最后几个字）"
        );
        // ④ paraformer 流式接口不得回到生产区
        let code: Vec<&String> = lines.iter().filter(|l| !l.starts_with("//")).collect();
        for needle in [
            "OnlineRecognizer",
            "is_endpoint(",
            "input_finished(",
            "is_ready(",
        ] {
            assert!(
                !code.iter().any(|l| l.contains(needle)),
                "STREAM-SV-463：生产区不得再出现 paraformer 流式接口 `{needle}`"
            );
        }
    }

    /// STREAM-SV-463 端到端（真模型 + 真录音）：派发点之前的预览文字**此后一字不变**。
    ///
    /// 喂 full.wav（+3s 尾静默）跑完整 `transcribe_streaming_local`：每次 `on_reflow_commit(seg, Some(len))`
    /// 之后的所有显示文本，前 `len` 个字必须与登记当刻逐字相同（325/337/438/442 按字符坐标回灌的前提）；
    /// 浅色尾巴字数不超过显示长度；收尾回调尾巴为 0。
    /// 运行：`cargo test --bin feiyin-ime -- --ignored --nocapture sv463_preview_freeze_e2e`
    #[test]
    #[ignore = "requires preview model + full.wav; --ignored 运行"]
    fn sv463_preview_freeze_e2e() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let Ok(recognizer) = super::create_local_stream_recognizer(&root.join("models")) else {
            eprintln!("skip: 预览 SenseVoice 模型不在位");
            return;
        };
        let wav = root.join("collab/research/audio-real-gavin/processed/full.wav");
        let Some(w) = sherpa_onnx::Wave::read(wav.to_str().unwrap()) else {
            eprintln!("skip: full.wav 不在位");
            return;
        };
        let mut audio: Vec<f32> = w.samples().to_vec();
        let speech_secs = audio.len() as f32 / 16_000.0;
        // 442 按切点时间切分的前提：模型给逐 token 时间且能对上文本。
        let (t5, m5) =
            super::preview_decode_timed(&recognizer, &audio[..audio.len().min(5 * 16_000)]);
        println!(
            "[463] 前 5s 逐 token 时间 {} 个 / 文本 {} 字：{t5}",
            m5.len(),
            t5.chars().count()
        );
        assert!(!m5.is_empty(), "SenseVoice 应给出可对齐的逐 token 时间");
        audio.extend(std::iter::repeat(0f32).take(3 * 16_000));
        let (tx, rx) = crossbeam_channel::unbounded::<Vec<f32>>();
        for c in audio.chunks(1_600) {
            let _ = tx.send(c.to_vec());
        }
        drop(tx);
        let displays = std::cell::RefCell::new(Vec::<(String, usize)>::new());
        let commits = std::cell::RefCell::new(Vec::<(usize, usize)>::new()); // (显示序号, len)
        let dispatches = std::cell::Cell::new(0usize);
        let events = std::cell::RefCell::new(Vec::<String>::new());
        // PREVIEW-PUNCT-464：以句末标点收尾的派发片数（停顿派发前补句末标点）。
        let closed = std::cell::Cell::new(0usize);
        // 生产同款预览标点（464：一变就打、停顿补句末标点；只打边界之后的尾巴）—— 冻结前缀必须经得起重打。
        let mut punct = crate::punctuation::PunctuationEngine::new(&root.join("models"));
        println!(
            "[463] 预览标点引擎：{}",
            if punct.is_some() {
                "在位"
            } else {
                "缺失（不打点）"
            }
        );
        let t0 = std::time::Instant::now();
        let (final_preview, _pcm) = super::transcribe_streaming_local(
            rx,
            &recognizer,
            None,
            punct.as_mut(),
            0.01,
            "",
            |text, _words, tentative| {
                assert!(
                    tentative <= text.chars().count(),
                    "浅色尾巴不得超过显示长度：{tentative} > {}",
                    text.chars().count()
                );
                let tail: String = {
                    let v: Vec<char> = text.chars().collect();
                    v[v.len().saturating_sub(14)..].iter().collect()
                };
                events.borrow_mut().push(format!(
                    "显示#{} 字数={} 浅色={} …{tail}",
                    displays.borrow().len(),
                    text.chars().count(),
                    tentative
                ));
                displays.borrow_mut().push((text.to_string(), tentative));
            },
            super::AccDispatchConfig::new(),
            |i, len, _segs, st, _r, _f| {
                dispatches.set(dispatches.get() + 1);
                if st.ends_with(['。', '？', '！', '.', '?', '!']) {
                    closed.set(closed.get() + 1);
                }
                events
                    .borrow_mut()
                    .push(format!("派发#{i} committed_len={len} 片流式文本={st}"));
            },
            |_pcm_pos| {},
            |seg, len| {
                events
                    .borrow_mut()
                    .push(format!("登记 seg={seg} len={len:?}"));
                if let Some(len) = len {
                    let at = displays.borrow().len();
                    commits.borrow_mut().push((at, len));
                }
            },
        )
        .expect("transcribe_streaming_local");
        let displays = displays.into_inner();
        let commits = commits.into_inner();
        for e in events.into_inner() {
            println!("[463-ev] {e}");
        }
        println!(
            "\n[463] 语音 {speech_secs:.1}s 处理 {:.1}s ｜ 显示刷新 {} 次 ｜ 派发 {} 片 ｜ 登记边界 {} 个",
            t0.elapsed().as_secs_f64(),
            displays.len(),
            dispatches.get(),
            commits.len()
        );
        println!("[463] 收尾预览：{final_preview}");
        println!(
            "[463] 以句末标点收尾的派发片 {} / {}（停顿派发补句末标点，PREVIEW-PUNCT-464）",
            closed.get(),
            dispatches.get()
        );
        if punct.is_some() {
            assert!(
                closed.get() >= 1,
                "开着标点时，停顿派发的片应以句末标点收尾"
            );
        }
        assert!(!displays.is_empty(), "必须有预览输出");
        assert!(dispatches.get() >= 1, "必须至少派发一片");
        for &(at, len) in &commits {
            let Some((base, _)) = displays[..at].last() else {
                continue;
            };
            let head: String = base.chars().take(len).collect();
            for (later, _) in &displays[at..] {
                let later_head: String = later.chars().take(len).collect();
                assert_eq!(
                    later_head, head,
                    "登记边界 len={len} 之前的预览文字被改写（第 {at} 次显示之后）"
                );
            }
        }
        assert_eq!(
            displays.last().map(|d| d.1),
            Some(0),
            "收尾回调的浅色尾巴必须为 0"
        );

        // 日 / 韩：实时管线预览能出假名 / 谚文（STREAM-KOJA-462 的出发点）。
        let sv_dir = root
            .join("models")
            .join(crate::transcription::SENSEVOICE_MODEL_SUBDIR);
        let kana = |c: char| ('\u{3040}'..='\u{30ff}').contains(&c);
        let hangul = |c: char| ('\u{ac00}'..='\u{d7a3}').contains(&c);
        for (lang, ok) in [("ja", &kana as &dyn Fn(char) -> bool), ("ko", &hangul)] {
            let wav = sv_dir.join(format!("test_wavs/{lang}.wav"));
            let Some(w) = sherpa_onnx::Wave::read(wav.to_str().unwrap()) else {
                eprintln!("skip: {lang}.wav 不在位");
                continue;
            };
            let (tx, rx) = crossbeam_channel::unbounded::<Vec<f32>>();
            for c in w.samples().chunks(1_600) {
                let _ = tx.send(c.to_vec());
            }
            for _ in 0..20 {
                let _ = tx.send(vec![0f32; 1_600]);
            }
            drop(tx);
            let (preview, _) = super::transcribe_streaming_local(
                rx,
                &recognizer,
                None,
                punct.as_mut(),
                0.01,
                "",
                |_t, _w, _n| {},
                super::AccDispatchConfig::new(),
                |_i, _len, _segs, _st, _r, _f| {},
                |_pcm_pos| {},
                |_seg, _len| {},
            )
            .expect("transcribe_streaming_local");
            println!("[463] {lang} 预览：{preview}");
            assert!(
                preview.chars().any(ok),
                "{lang} 预览应含本语种文字：{preview}"
            );
            if lang == "ko" {
                assert!(
                    preview.contains(' '),
                    "KOJA-PUNCT-464：韩文预览空格不得被标点模型吃掉：{preview}"
                );
            }
        }
    }

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
    // 源码级结构护栏的公共工具（读本文件生产区源码 → needle 定位 → 花括号定界取块）。
    // STREAM-SV-463：paraformer 专用的 342（假 endpoint）/ 291（句末 flush）护栏随旧模型移除，
    // 新机制「每个派发点都是冻结点」由 `guard463_every_dispatch_point_freezes` 钉死。
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

    /// 🔴 ACC-DISPATCH-SILENCE-ONLY-346 **核心判据**：acc 计数器不得复用「会被打点动作清零」的
    /// 静默计数器 ⇒ 1200ms 可达。
    ///
    /// 346 当年的生产时序（独立计数器正是因它而生）：
    ///   静音累加 → acc 检查（本函数）→ … → 标点检查（静默 ≥800 打点后清显示层 `silent_ms`）。
    /// 若 acc 阈值读那个显示层 `silent_ms`，则每 800ms 被清零 ⇒ 永远到不了 1200 ⇒ **录音中一次都不派**。
    ///
    /// PREVIEW-PUNCT-LIVE-438：显示层 `silent_ms` 已随静默打点**整体删除**，打点改读自己的
    /// `punct_interval_ms`（间隔计时，不清任何静默计数器）⇒ 该坑从结构上消失。本测试保留为
    /// **正/反两面的逻辑证明**（谁让 acc 改读「会被打点清零」的计数器，反证分支即其后果），
    /// 生产接线由源码级护栏 `guard346_acc_counter_wiring` 钉死。
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
            // 标点路径（346 当年的历史口径：静默 ≥800 打点即清显示层计数器；
            // 438 已删除该机制，此处仅作「误用显示层计数器」的反证模型）。
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
    /// - 调用点改回别的计数器（显示层 `silent_ms,` / 438 的 `punct_interval_ms,`）⇒ 接线断言失败；
    /// - `acc_silent_ms` 不再在静音支累加 ⇒ 「累加恰 1 处」失败；
    /// - 在标点路径（`if repunct_due`，438 起为间隔打点）里清 `acc_silent_ms` ⇒ 「代码行不触碰」失败。
    ///
    /// PREVIEW-PUNCT-LIVE-438 锚点变更（**意图不变、断言更严**，非遗漏）：
    /// - 「归零紧邻显示层 `silent_ms = 0.0;`」→「归零紧邻 `long_silence_done_for_pause = false;`」
    ///   （显示层静默计数器已删，语音支仍以长静默 latch 复位行为锚定位）；
    /// - 「`if silence_due {` 标点清零块」→「`if repunct_due {` 间隔打点归零块」（块内仍不得碰
    ///   `acc_silent_ms`），并新增「调用点不得传 `punct_interval_ms`」。
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
        // 归零恰 1 处，且必须落在**语音支**（紧邻长静默 latch 复位；438 起上一行不再是
        // 显示层 `silent_ms = 0.0;`，该计数器已删）。
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
            lines[r - 1].starts_with("long_silence_done_for_pause = false;"),
            "346: acc_silent_ms 归零应紧邻语音支的 latch 复位（prev={:?}）",
            lines[r - 1]
        );
        assert!(
            lines[r + 1..(r + 4).min(lines.len())]
                .iter()
                .any(|l| l.starts_with("speech_since_last_reset = true;")),
            "346: acc_silent_ms 归零必须落在语音支（后续应出现 speech_since_last_reset = true）"
        );
        // 🔴 打点归零块（438 起为 `if repunct_due {`）内**代码行**不得出现 acc_silent_ms（注释行不算）。
        let sd = (0..lines.len())
            .find(|&i| lines[i].starts_with("if repunct_due {"))
            .expect("346: 未找到打点归零块 `if repunct_due {`（438 间隔打点）");
        let sd_end = sd
            + (sd..lines.len())
                .find(|&i| lines[i] == "}")
                .map(|i| i - sd)
                .expect("346: 打点归零块未闭合");
        let n = (sd..=sd_end)
            .filter(|&i| !lines[i].starts_with("//") && lines[i].contains("acc_silent_ms"))
            .count();
        assert_eq!(
            n, 0,
            "346: 标点路径不得触碰 acc_silent_ms（否则 1200ms 不可达），实测 {n} 处"
        );
        // 调用点必须传 acc_silent_ms，不得传显示层 silent_ms（历史）或 438 的间隔计数器。
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
        assert!(
            !block.iter().any(|l| l.starts_with("punct_interval_ms,")),
            "346/438: 调用点不得传打点间隔计数器 `punct_interval_ms,`（会被打点归零），实测块={:?}",
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
    // LOCALRT-STREAM-THREADS-417（Gavin 2026-09-25）：预览侧封顶 4，与精解侧刻意分口径。
    // ============================================================

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
    // PUNCT-PREVIEW-SEMANTIC-349 → PREVIEW-PUNCT-LIVE-438 · 预览打点口径
    //   349：只认静默 ≥1200ms 且有新内容（因果取证见
    //   `collab/evidence/20260922-punct349/causal-evidence.md`，探针按 DEC-077 已删）。
    //   438（Gavin 2026-09-26）：改为**单一的 3.5s 间隔**触发、只打精解未覆盖的尾巴，
    //   1200ms 静默触发与显示层 `silent_ms` 计数器**一并删除**（预览窗口只留一个打点机制）。
    // ========================================================================

    /// 269/349 → 438 行为变化（不放宽，只换口径）：
    /// - 349 版：`punct349_preview_only_on_1200ms_silence`，钉「静默 ≥1200ms 才打点」。
    /// - 438 版：钉「距上次打点 ≥ `PUNCT_PREVIEW_INTERVAL_MS`(3.5s) 才打点」——
    ///   触发量由静默时长改为间隔时长，`has_new` 护栏原样保留；
    ///   「1.2s 静默不再触发」由 `fix438_silence_1200ms_no_longer_triggers` 单独钉死。
    /// PREVIEW-PUNCT-464：生产阈值改为 0（预览文字一变就打，Gavin 10-03）；门槛机制本身用 3.5s 样例继续钉死。
    #[test]
    fn punct438_preview_only_on_interval() {
        assert_eq!(
            PUNCT_PREVIEW_INTERVAL_MS, 0.0,
            "PREVIEW-PUNCT-464：Gavin 10-03 要预览带标点 ⇒ 一变就打"
        );
        let t = 3500.0;
        // 间隔未满 ⇒ 不打点（哪怕有新内容）。
        assert!(!should_repunctuate_preview(3499.0, true, t));
        assert!(!should_repunctuate_preview(400.0, true, t));
        assert!(!should_repunctuate_preview(0.0, true, t));
        // 无新内容 ⇒ 不打点（间隔再长也不反复重打同一段，省算力）。
        assert!(!should_repunctuate_preview(9000.0, false, t));
        // 间隔 ≥3.5s 且有新内容 ⇒ 打点。
        assert!(should_repunctuate_preview(3500.0, true, t));
        assert!(should_repunctuate_preview(9000.0, true, t));
    }

    /// PUNCT-349 根因表征：触发时刻补的**末尾终止符**会经 `punct_cache_reuse` 落到**句中**
    /// —— 这正是 Gavin 报的「标点打在句子中间」的机制。锁定该复用行为本身
    /// （`punct_cache_reuse` 契约未变），438 的防线是**存缓存前剥末尾终止符**
    /// （`preview_display` → `strip_trailing_punctuation`，见 `fix438_strip_terminal_keeps_interior`）
    /// —— 即把「前置条件」从「只在语义停顿打点」升级为「终止符根本不入缓存」。
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
    // PREVIEW-PUNCT-LIVE-438 · 3.5s 间隔打点 + 冻结前缀（补充 R1）
    //   验收 ①-⑨（任务书「验收」节）；生产接线另由 `fix438_source_guards` 源码级钉死。
    // ========================================================================

    /// ① 3.5s 内有新字**不**重打；≥3.5s 且有新字重打；打点后重新计时（逐 chunk 推进模拟，
    /// 与生产「每 chunk `+= chunk_ms`、实际打点归零」同构）。
    #[test]
    fn fix438_interval_gate_refires_after_reset() {
        // 门槛机制本身（任意阈值，此处用 438 的 3.5s 样例）；生产阈值见 `punct438_preview_only_on_interval`。
        let thr = 3500.0;
        let mut fires: Vec<f32> = Vec::new();
        let mut t = 0.0f32; // 距上次打点的间隔计数器（生产 punct_interval_ms）
        let mut elapsed = 0.0f32; // 全程累计时间，仅用于断言复打节奏
        for _ in 0..1200 {
            // 每 chunk 10ms，全程都有新字（has_new 恒真，只考验间隔闸门）。
            t += 10.0;
            elapsed += 10.0;
            if should_repunctuate_preview(t, true, thr) {
                fires.push(elapsed);
                t = 0.0; // 生产：实际完成打点后归零
            }
        }
        assert_eq!(
            fires,
            vec![3500.0, 7000.0, 10500.0],
            "首打在恰 3500ms（3.5s 内一次都不打），其后每 3.5s 复打一次"
        );
        // 边界：3499ms 有新字不打，3500ms 打。
        assert!(!should_repunctuate_preview(3499.0, true, thr));
        assert!(should_repunctuate_preview(3500.0, true, thr));
    }

    /// ② 无新字 ⇒ 不重打（哪怕间隔再长）—— 省算力护栏。
    #[test]
    fn fix438_no_new_chars_no_punct() {
        let thr = PUNCT_PREVIEW_INTERVAL_MS;
        assert!(!should_repunctuate_preview(thr, false, thr));
        assert!(!should_repunctuate_preview(thr + 9000.0, false, thr));
        // 生产同判据：`raw_full.len() > punct_cache.raw_len`（见 preview_display）。
        let raw = "今天天气不错";
        let cache_raw_len = raw.len();
        assert!(
            !(raw.len() > cache_raw_len),
            "raw 未变长 ⇒ has_new=false ⇒ 不送引擎"
        );
    }

    /// ③ 打点结果存缓存前剥掉**末尾**句终标点；句中标点（含句中「，。」）保留；
    /// 冻结前缀一字不动。
    #[test]
    fn fix438_strip_terminal_keeps_interior() {
        // 末尾句终标点被剥（含连续终止符）。
        assert_eq!(
            build_punct_prefix("", "今天天气不错，我们准备开始讨论这个项目。"),
            "今天天气不错，我们准备开始讨论这个项目"
        );
        assert_eq!(build_punct_prefix("", "他说好？！"), "他说好");
        // 句中「，。」保留：只剥最后一枚终止符。
        assert_eq!(
            build_punct_prefix("", "我们先看方案，然后动手。明天开工。"),
            "我们先看方案，然后动手。明天开工"
        );
        // 尾巴无终止符 ⇒ 原样；冻结前缀不动。
        assert_eq!(
            build_punct_prefix("已打点的前缀，", "还没说完的尾巴"),
            "已打点的前缀，还没说完的尾巴"
        );
        // 判据与 punctuation 模块同源（TRAILING_PUNCT_CHARS）。
        assert_eq!(build_punct_prefix("", "结尾，"), "结尾");
    }

    /// ④ 两次打点之间新增的字直接接在缓存之后；已显示标点不闪回、句终标点不入缓存。
    #[test]
    fn fix438_new_chars_append_after_cache_no_flashback() {
        // 首次打点：引擎输出带句终 ⇒ 入缓存前剥掉。
        let raw1 = "今天天气不错我们准备开始";
        let cache_prefix = build_punct_prefix("", "今天天气不错，我们准备开始。");
        assert_eq!(cache_prefix, "今天天气不错，我们准备开始");
        // 两次打点之间新增的字：接在缓存后（punct_cache_reuse 口径）。
        let raw2 = "今天天气不错我们准备开始讨论这个项目";
        let shown = punct_cache_reuse(&cache_prefix, raw2, raw1.len());
        assert_eq!(shown, "今天天气不错，我们准备开始讨论这个项目");
        assert!(shown.contains('，'), "已显示的句中标点不闪回");
        assert!(!shown.contains('。'), "缓存里没有任何句终标点（349 防线）");
        // 再次重打后前缀仍含原逗号（冻结前缀不回退）。
        let head = punct_head(
            &cache_prefix,
            raw2,
            raw1,
            raw1.len(),
            cache_prefix.chars().count(),
        );
        assert_eq!(head.as_bytes(), cache_prefix.as_bytes());
        let again = build_punct_prefix(&head, "讨论这个项目，明天开工。");
        assert!(again.starts_with("今天天气不错，我们准备开始讨论这个项目，明天开工"));
    }

    /// ⑤ PREVIEW-PUNCT-464：生产阈值 0 ⇒ **有新字即打**，与静默 / 间隔长短无关；没新字不打（省算力）。
    #[test]
    fn fix464_punct_on_every_change_independent_of_silence() {
        let thr = PUNCT_PREVIEW_INTERVAL_MS;
        for ms in [0.0, 10.0, 1200.0, 3499.0, 9000.0] {
            assert!(
                should_repunctuate_preview(ms, true, thr),
                "{ms}ms 有新字应打"
            );
            assert!(
                !should_repunctuate_preview(ms, false, thr),
                "{ms}ms 无新字不打"
            );
        }
    }

    /// ⑥ 用户关标点（`engine = None`）⇒ 原样返回裸文本，且不写缓存（行为与改前一致）。
    #[test]
    fn fix438_engine_none_returns_raw_asis() {
        let mut cache = PunctPreviewCache {
            prefix: String::new(),
            raw_len: 0,
            raw: String::new(),
        };
        let raw = "今天天气不错我们准备开始讨论";
        assert_eq!(
            preview_display(raw, None, &mut cache, true, 0, 0, false),
            raw
        );
        // 已派发（非零坐标）+ force 同样原样返回。
        assert_eq!(
            preview_display(raw, None, &mut cache, false, 7, 3, true),
            raw
        );
        assert_eq!(cache.raw_len, 0, "engine=None 不写打点缓存");
        assert!(cache.prefix.is_empty());
    }

    /// ⑦ `acc_silent_ms` 派发时机与改前**逐位一致**：打点间隔归零与否，派发帧序列完全相同。
    ///（改前是「静默打点清显示层计数器」、改后是「间隔打点清 punct_interval_ms」，都不得影响 acc。）
    #[test]
    fn fix438_acc_dispatch_timing_unaffected_by_punct_interval() {
        let pending = SAMPLE_RATE as usize; // 有音频待派（>0）
                                            // 场景：0-2s 有声 → 2-4s 静默 → 4-6s 有声 → 6-12s 静默。
        let run = |punct_resets: bool| -> Vec<usize> {
            let mut acc_silent_ms = 0.0f32;
            let mut punct_interval_ms = 0.0f32;
            let mut done = false;
            let mut fires = Vec::new();
            for i in 0..1200usize {
                let speaking = (0..200).contains(&i) || (400..600).contains(&i);
                if speaking {
                    acc_silent_ms = 0.0;
                    done = false;
                } else {
                    acc_silent_ms += 10.0;
                }
                punct_interval_ms += 10.0;
                if should_dispatch_acc(true, acc_silent_ms, pending, done, true, 1200.0) {
                    fires.push(i);
                    done = true;
                }
                if punct_resets
                    && should_repunctuate_preview(
                        punct_interval_ms,
                        true,
                        PUNCT_PREVIEW_INTERVAL_MS,
                    )
                {
                    punct_interval_ms = 0.0;
                }
                debug_assert!(punct_interval_ms >= 0.0);
            }
            fires
        };
        let with_reset = run(true);
        let no_reset = run(false);
        assert_eq!(
            with_reset, no_reset,
            "打点间隔归零不得改变 acc 派发时机（⑦）"
        );
        assert_eq!(
            with_reset,
            vec![319, 719],
            "两段静默各在满 1200ms 时派发一次（i=200+119 / 600+119）"
        );
    }

    /// ⑧ 已被精解覆盖的前缀在重打前后**逐字节不变**（两种坐标形态都钉）：
    /// A) 派发边界 ≥ 缓存覆盖位（常态）；B) 上次打点发生在派发之后（缓存覆盖位 > 边界）。
    #[test]
    fn fix438_prefix_bytes_stable_across_repunct() {
        // A：保留头 = 缓存前缀（引擎打点结果、剥尾），逐字节原样。
        let raw1 = "今天天气不错我们准备开始讨论这个项目";
        let prefix = build_punct_prefix("", "今天天气不错，我们准备开始讨论这个项目。");
        let cache_raw_len = raw1.len();
        let head = punct_head(&prefix, raw1, raw1, cache_raw_len, prefix.chars().count());
        assert_eq!(head.as_bytes(), prefix.as_bytes(), "A: 冻结前缀逐字节不变");

        // A2：派发边界在缓存覆盖位之后 ⇒ 保留头 = 前缀 + raw 逐字尾巴（不引入任何新标点）。
        let raw2 = "今天天气不错我们准备开始讨论这个项目的具体细节";
        let head2 = punct_head(&prefix, raw2, raw1, raw2.len(), 0);
        let expect = format!("{}{}", prefix, &raw2[cache_raw_len..]);
        assert_eq!(head2.as_bytes(), expect.as_bytes());
        assert_eq!(&head2[..prefix.len()], prefix, "A2: 前缀部分逐字节不变");

        // B：缓存覆盖位 > 派发边界（派发之后又打过点）⇒ 按字符截出冻结前缀。
        let dispatch_display = "今天天气不错，我们准备"; // 派发当刻已显示文本
        let head_chars = dispatch_display.chars().count();
        let prefix_b = format!("{}{}", dispatch_display, "开始讨论这个项目。"); // 之后一次打点的缓存
        let raw_b = "今天天气不错我们准备开始讨论这个项目";
        let tail_start_b = "今天天气不错我们准备".len(); // 派发当刻的 raw 字节长
        let head_b = punct_head(&prefix_b, raw_b, raw_b, tail_start_b, head_chars);
        assert_eq!(
            head_b.as_bytes(),
            dispatch_display.as_bytes(),
            "B: 冻结前缀逐字节不变"
        );
    }

    /// STREAM-SV-463：标点缓存校验复用 —— 缓存覆盖的裸文本被改写时，只保留公共前缀那段的标点，
    /// 其后接当前裸文本；绝不显示改写前的旧字，也不把改写后的字接在旧字后面（重复）。
    #[test]
    fn sv463_punct_cache_view_never_shows_rewritten_text() {
        use super::punct_cache_view as v;
        let cache_raw = "近期威威队内出现了疾病";
        let prefix = "近期，威威队内出现了疾病";
        // 未改写（只追加）⇒ 与 344 口径一致：缓存 + 新增后缀。
        assert_eq!(
            v(prefix, cache_raw, "近期威威队内出现了疾病传播"),
            "近期，威威队内出现了疾病传播"
        );
        // 改写了「威威」→「挪威」：只保留公共前缀「近期」那段（紧跟其后的标点是按旧字打的，一并不留，
        // 下次 3.5s 打点再补），其后全是当前裸文本。
        assert_eq!(
            v(prefix, cache_raw, "近期挪威队内出现了疾病传播"),
            "近期挪威队内出现了疾病传播"
        );
        // 公共前缀之内的标点照常保留。
        assert_eq!(
            v("今天，天气不错", "今天天气不错", "今天天气真好"),
            "今天，天气真好"
        );
        // 改写后变短：不留旧尾巴、不重复。
        assert_eq!(v(prefix, cache_raw, "近期挪威"), "近期挪威");
        // 空缓存 ⇒ 原样。
        assert_eq!(v("", "", "你好"), "你好");
    }

    /// ⑨ 重打后 `reflow_preview(acc, streaming, committed_len)` 不多字、不丢字：
    /// 冻结前缀使 `committed_len` 切位始终落在原处，尾巴只追加、acc 前缀只出现一次。
    #[test]
    fn fix438_reflow_after_repunct_no_dup_no_drop() {
        let display1 = "今天天气不错，我们准备"; // 派发当刻已显示文本
        let committed = display1.chars().count(); // = 12
        let l1 = "今天天气不错我们准备".len();
        let raw2 = "今天天气不错我们准备开始讨论这个项目";
        // 首次间隔重打：冻结前缀 = display1，尾巴补打并剥尾。
        let head = punct_head(display1, raw2, &raw2[..l1], l1, committed);
        let display2 = build_punct_prefix(&head, "开始讨论这个项目。");
        assert_eq!(display2, "今天天气不错，我们准备开始讨论这个项目");
        assert!(display2.starts_with(display1), "⑧/⑨ 前缀冻结 ⇒ 切位不漂");

        let acc = "这是精解权威前缀。";
        let out = crate::reflow_preview(acc, &display2, committed);
        let tail: String = display2.chars().skip(committed).collect();
        assert_eq!(
            out,
            format!("{}{}", acc, tail),
            "reflow = acc + 尾巴，无重排"
        );
        assert_eq!(tail, "开始讨论这个项目", "尾巴一字不多一字不少");

        // 第二次重打（尾巴更长）后再 reflow：切位仍在 committed，尾巴只增不重。
        let display3 = build_punct_prefix(&head, "开始讨论这个项目的具体细节。");
        let out3 = crate::reflow_preview(acc, &display3, committed);
        let tail3: String = display3.chars().skip(committed).collect();
        assert_eq!(out3, format!("{}{}", acc, tail3));
        assert!(tail3.starts_with(&tail), "尾巴只追加，旧尾巴不丢");
        assert_eq!(out3.matches(acc).count(), 1, "acc 前缀只出现一次（不多字）");
    }

    /// ⑤/接线 源码级护栏：旧机制删干净、新机制接线钉死（读生产区源码，纯函数证不了接线）。
    #[test]
    fn fix438_source_guards() {
        let lines = ls_prod_lines();
        let code = |i: usize| !lines[i].starts_with("//");
        let count_code = |needle: &str| -> usize {
            (0..lines.len())
                .filter(|&i| code(i) && lines[i].contains(needle))
                .count()
        };
        let count_line =
            |exact: &str| -> usize { (0..lines.len()).filter(|&i| lines[i] == exact).count() };
        // 旧机制删干净（DEC-077 不留死代码）：生产**代码行**不得再引用旧常量/旧计数器。
        assert_eq!(
            count_code("PUNCT_SILENCE_TRIGGER_MS"),
            0,
            "438: 生产代码不得再引用 PUNCT_SILENCE_TRIGGER_MS（仅注释可作沿革）"
        );
        assert_eq!(
            (0..lines.len())
                .filter(|&i| code(i) && lines[i].starts_with("let mut silent_ms"))
                .count(),
            0,
            "438: 显示层 silent_ms 声明应已删除"
        );
        // 新常量 + 独立间隔计时：声明 / 每 chunk 推进 / 打点归零 各恰 1 处。
        assert_eq!(
            count_code("const PUNCT_PREVIEW_INTERVAL_MS: f32 = 0.0;"),
            1,
            "438/464: 间隔常量须恰 1 处且 = 0.0（464 起一变就打）"
        );
        assert_eq!(
            count_line("punct_interval_ms += chunk_ms;"),
            1,
            "438: 间隔计时每 chunk 恰推进 1 处"
        );
        assert_eq!(
            count_line("punct_interval_ms = 0.0;"),
            1,
            "438: 仅实际打点处归零 1 处"
        );
        // 双坐标：每个派发点各捕获一次 —— 静默派发 + 尾片派发（同刻同源：committed_len /
        // last_display_raw_len）+ 442 满 10s 回看派发（切点那一刻：committed_len / raw_at）。
        assert_eq!(
            count_line("punct_head_chars = committed_len;"),
            3,
            "438/442: 静默/尾片/回看三个派发点各捕获 committed_len"
        );
        // 静默 / 尾片派发各捕获「当刻」raw 侧边界；445 迟到字归入上一片时边界后移，同刻同源再捕获一次。
        assert_eq!(
            count_line("punct_tail_start = last_display_raw_len;"),
            3,
            "438/445: 静默/尾片派发 + 迟到字吸收各捕获「当刻」raw 侧边界"
        );
        assert_eq!(
            count_line("punct_head_chars = cur;"),
            1,
            "445: 迟到字吸收时冻结前缀随边界后移（与 raw 侧同刻）"
        );
        assert_eq!(
            count_line("punct_tail_start = raw_at;"),
            1,
            "442: 回看派发捕获「切点那一刻」raw 侧边界"
        );
        // 打点归零块存在且不碰 acc 计数器（与 guard346 同口径，防有人把归零接到 acc 上）。
        assert_eq!(
            count_line("if repunct_due {"),
            1,
            "438: 打点归零块须恰 1 处"
        );
        let sd = lines
            .iter()
            .position(|l| l == "if repunct_due {")
            .expect("438: 打点归零块存在");
        let end = (sd..lines.len())
            .find(|&i| lines[i] == "}")
            .expect("438: 归零块闭合");
        assert!(
            !(sd..=end).any(|i| !lines[i].starts_with("//") && lines[i].contains("acc_silent_ms")),
            "438/346: 打点归零块不得触碰 acc_silent_ms"
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

// =====================================================================
// TEST-SYNC-405-407（阶段三 · 非作者护栏，coder-2）—— 405 流式性能（DEC-086 / 审计 403）
// =====================================================================
#[cfg(test)]
mod testsync405_407_tests {
    use super::{
        should_signal_long_silence, DisplayCache, StreamingAsrState, LONG_SILENCE_TAIL_MS,
    };

    /// 405 契约1（源码）：生产区不得再出现「另起 stream 重解整句」的额外解码 ——
    /// STREAM-SV-463 起「整句重解」本身就是模拟流式的机制，收口为：`create_stream()` 恰 1 处
    /// （在 `preview_decode` 内），`preview_decode(` 恰 1 个调用点（`sim_decode_text!` 宏）；无 `shadow`。
    #[test]
    fn ts405b_no_extra_decode_stream_source_guard() {
        let src = include_str!("local_stream.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap();
        let code: String = prod
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            code.matches("create_stream()").count(),
            1,
            "生产区只应有 1 处 create_stream()（preview_decode 内），影子 / 额外重解不得回归"
        );
        assert_eq!(
            code.matches("preview_decode_timed(recognizer, &pcm")
                .count(),
            1,
            "重解只能经 sim_decode_text! 一个入口（按步长 / 冻结前补解 / 收尾），不得另开解码点"
        );
        assert!(!code.contains("shadow"), "生产区不得出现 shadow（DEC-086）");
    }

    /// 405 契约2（独立性质）：`DisplayCache` 与 `StreamingAsrState::display_text()` 在同一 `on_result`
    /// 序列下**逐字相等**。独立夹具：中英混排 + emoji + 空 current + 4 字节字符；含「全切句 / 全中间」两端。
    #[test]
    fn ts405b_display_cache_equiv_independent() {
        let frags = [
            "中文",
            "English ",
            "🙂",
            "",
            "混合mixed",
            "，。",
            "𠀀", // 4 字节码点（多字节边界）
            "a",
        ];
        let run = |mode: u8| {
            let mut cache = DisplayCache::new();
            let mut state = StreamingAsrState::new();
            let mut seed = 0xDEAD_BEEF_u64;
            let mut sid = 1i64;
            for i in 0..3000 {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let text = frags[(seed >> 33) as usize % frags.len()];
                let end = match mode {
                    0 => (seed & 3) == 0,
                    1 => true,
                    _ => false,
                };
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
                    "mode={mode} iter={i} text={text:?}"
                );
            }
        };
        run(0);
        run(1); // 全切句
        run(2); // 全中间
    }

    /// 405 契约3（源码）：只为日志的计时/计数**不得进入任何判定调用**。
    /// 取 `should_dispatch_acc(` / `should_signal_long_silence(` / `should_dispatch_tail(` 的实参串，
    /// 断言其中不含任何计时/计数变量（判定只许由功能量驱动）。
    #[test]
    fn ts405b_timing_counters_not_in_decisions_source_guard() {
        let src = include_str!("local_stream.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap();
        let code: String = prod
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let counters = [
            "vad_total_ms",
            "vad_chunks",
            "vad_max_ms",
            "vad_only_speech_chunks",
            "bound_hit_a",
            "bound_hit_b",
            "bound_hit_c",
        ];
        for name in [
            "should_dispatch_acc(",
            "should_signal_long_silence(",
            "should_dispatch_tail(",
        ] {
            let mut from = 0usize;
            while let Some(pos) = code[from..].find(name) {
                let start = from + pos + name.len();
                // 实参内无嵌套括号/字符串 ⇒ 简单括号配平取实参串。
                let bytes = code.as_bytes();
                let mut depth = 1i32;
                let mut j = start;
                while j < bytes.len() && depth > 0 {
                    match bytes[j] {
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        _ => {}
                    }
                    if depth == 0 {
                        break;
                    }
                    j += 1;
                }
                let args = &code[start..j];
                for c in counters {
                    assert!(
                        !args.contains(c),
                        "{name}...{args}... 实参不得含日志计时/计数 {c}"
                    );
                }
                from = j + 1;
            }
        }
    }

    /// 407 契约4（触发时序模拟）：同一段静默只在首次 ≥1900ms 发一次；1200~1900 不发；
    /// 恢复说话复位后，下一段静默可再发。
    #[test]
    fn ts407_trigger_sequence_once_per_pause() {
        let mut done = false;
        let step = |silent_ms: f32, speaking: bool, done: &mut bool| -> bool {
            if speaking {
                *done = false;
                return false;
            }
            let fire = should_signal_long_silence(true, silent_ms, *done);
            if fire {
                *done = true;
            }
            fire
        };
        assert!(!step(1200.0, false, &mut done));
        assert!(!step(1800.0, false, &mut done));
        assert_eq!(LONG_SILENCE_TAIL_MS, 1900.0);
        assert!(
            step(LONG_SILENCE_TAIL_MS, false, &mut done),
            "恰 1900 发一次"
        );
        assert!(!step(1899.0, false, &mut done));
        assert!(!step(5000.0, false, &mut done), "同段静默只发一次");
        // 恢复说话 ⇒ 复位。
        assert!(!step(0.0, true, &mut done));
        // 下一段静默可再发。
        assert!(!step(1899.0, false, &mut done));
        assert!(step(2000.0, false, &mut done), "复位后下一段可再发");
        // 未启用 ⇒ 永不发。
        assert!(!should_signal_long_silence(false, 9999.0, false));
    }
}

// =====================================================================
// TAIL-CUT-REAL-PAUSE-434：前片后缀切点只落在真停顿（字内单帧低谷不再算字缝）
// =====================================================================
#[cfg(test)]
mod tail_cut_434_tests {
    use super::{find_tail_cut, find_tail_cut_ex, TailCutKind};

    const RATE: usize = 16000;

    /// 语音段（440Hz 正弦，幅度 0.5）赋值到 `[a,b)` 秒。
    fn tone(a: &mut [f32], a_s: f32, b_s: f32) {
        for i in (a_s * RATE as f32) as usize..(b_s * RATE as f32) as usize {
            let t = i as f32 / RATE as f32;
            a[i] = (t * 440.0 * std::f32::consts::TAU).sin() * 0.5;
        }
    }
    fn silence(a: &mut [f32], a_s: f32, b_s: f32) {
        for i in (a_s * RATE as f32) as usize..(b_s * RATE as f32) as usize {
            a[i] = 0.0;
        }
    }

    /// 字音—40ms 低谷—字音—150ms 静音—字音 ⇒ 选 150ms 真停顿（不是 40ms 字内低谷）。
    #[test]
    fn tc434_prefers_real_pause_over_intra_char_dip() {
        let mut a = vec![0f32; 4 * RATE];
        tone(&mut a, 0.0, 1.5);
        silence(&mut a, 1.5, 1.54); // 40ms 字内低谷（2 帧）
        tone(&mut a, 1.54, 3.0);
        silence(&mut a, 3.0, 3.15); // 150ms 真停顿
        tone(&mut a, 3.15, 4.0);
        let (cut, kind) = find_tail_cut_ex(&a, 2 * RATE);
        assert_eq!(kind, TailCutKind::RealPause, "应取真停顿");
        let cs = cut as f32 / RATE as f32;
        assert!(
            (3.0..=3.2).contains(&cs),
            "切点应在 150ms 停顿内，实测 {cs:.3}s"
        );
    }

    /// 只有 40ms 字内低谷（无 ≥120ms 停顿）⇒ 退回 WeakGap（旧单帧判据），不改旧切点语义。
    #[test]
    fn tc434_intra_char_dip_is_weak_gap() {
        let mut a = vec![0f32; 4 * RATE];
        tone(&mut a, 0.0, 3.0);
        silence(&mut a, 3.0, 3.04); // 40ms 低谷
        tone(&mut a, 3.04, 4.0);
        let (_, kind) = find_tail_cut_ex(&a, 2 * RATE);
        assert_eq!(kind, TailCutKind::WeakGap, "仅字内低谷 ⇒ weak_gap 退回");
    }

    /// 首轮区间无真停顿、往前扩大后找到 ⇒ RealPause，后缀变长但不超整片。
    #[test]
    fn tc434_expands_back_when_no_pause_in_first_range() {
        let mut a = vec![0f32; 6 * RATE];
        tone(&mut a, 0.0, 2.5);
        silence(&mut a, 2.5, 2.65); // 150ms 真停顿（< len-back=4s）
        tone(&mut a, 2.65, 6.0);
        let back = 2 * RATE;
        let lower = a.len() - back;
        let (cut, kind) = find_tail_cut_ex(&a, back);
        assert_eq!(kind, TailCutKind::RealPause, "扩大后应找到真停顿");
        assert!(cut < lower, "切点应在 len-back 之前（后缀变长）");
        assert!(cut >= 2 * RATE, "最多回溯 2×back（不超整片）");
        let suffix = a.len() - cut;
        assert!(suffix > back && suffix <= a.len(), "后缀 >back 且 ≤整片");
    }

    /// 全程有声（无任何停顿）⇒ NoGap，回落 len-back；不足 back ⇒ (0, NoGap)。
    #[test]
    fn tc434_no_pause_falls_back() {
        let a: Vec<f32> = (0..6 * RATE)
            .map(|i| (i as f32 * 0.13).sin() * 0.5)
            .collect();
        let back = 2 * RATE;
        assert_eq!(
            find_tail_cut_ex(&a, back),
            (a.len() - back, TailCutKind::NoGap)
        );
        assert_eq!(find_tail_cut(&a, back), (a.len() - back, false));
        // 兼容旧口径：不足 back ⇒ 整片、gap_found=false。
        assert_eq!(find_tail_cut(&vec![0.3f32; RATE], 2 * RATE), (0, false));
    }

    /// `gap_found` 兼容语义：RealPause / WeakGap 均为 true（旧调用方零改动）。
    #[test]
    fn tc434_gap_found_compat() {
        let mut a = vec![0f32; 4 * RATE];
        tone(&mut a, 0.0, 3.0);
        silence(&mut a, 3.0, 3.15);
        tone(&mut a, 3.15, 4.0);
        assert_eq!(
            find_tail_cut(&a, 2 * RATE).1,
            true,
            "真停顿 ⇒ gap_found=true"
        );
        let mut b = vec![0f32; 4 * RATE];
        tone(&mut b, 0.0, 3.0);
        silence(&mut b, 3.0, 3.04);
        tone(&mut b, 3.04, 4.0);
        assert_eq!(
            find_tail_cut(&b, 2 * RATE).1,
            true,
            "weak_gap ⇒ gap_found=true（兼容）"
        );
    }
}

// =====================================================================
// TAIL-CUT-REAL-PAUSE-434 实测：真实录音 旧/新 切点对比（只读诊断）
//   运行：cargo test --bin feiyin-ime -- --ignored --nocapture diag434_cut
// =====================================================================
#[cfg(test)]
mod diag434_tests {
    use super::find_tail_cut_ex;
    use crate::transcription::vad::{find_gap_cut_gap_only, GAP_FRAME_SAMPLES, GAP_RMS_RATIO};
    use std::path::PathBuf;

    const RATE: usize = 16000;

    fn frame_rms(a: &[f32], s: usize) -> f32 {
        let e = (s + GAP_FRAME_SAMPLES).min(a.len());
        if e <= s {
            return 0.0;
        }
        let seg = &a[s..e];
        (seg.iter().map(|x| x * x).sum::<f32>() / seg.len() as f32).sqrt()
    }

    /// 切点处连续低能量帧数（±1 帧内按含 cut 的连续段）。
    fn pause_frames_at(a: &[f32], cut: usize, thr: f32) -> usize {
        let half = GAP_FRAME_SAMPLES / 2;
        let j0 = cut.saturating_sub(half) / GAP_FRAME_SAMPLES;
        let nf = a.len() / GAP_FRAME_SAMPLES;
        let low = |j: usize| j < nf && frame_rms(a, j * GAP_FRAME_SAMPLES) <= thr;
        let mut j = j0.min(nf.saturating_sub(1));
        while j > 0 && low(j - 1) {
            j -= 1;
        }
        let mut cnt = 0usize;
        while low(j) {
            cnt += 1;
            j += 1;
        }
        cnt
    }

    #[test]
    #[ignore = "diag434: cargo test --bin feiyin-ime -- --ignored --nocapture diag434_cut"]
    fn diag434_cut_compare() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("collab/evidence/gavin-sessions");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "wav").unwrap_or(false))
            .collect();
        files.sort();
        for f in files {
            let wav = sherpa_onnx::Wave::read(f.to_str().unwrap())
                .unwrap()
                .samples()
                .to_vec();
            let dur = wav.len() as f32 / RATE as f32;
            // 阈值：全片前 10s 帧 RMS 中位数 ×0.3（与生产同口径）。
            let m_end = wav.len().min(10 * RATE);
            let mut rms: Vec<f32> = Vec::new();
            let mut s = 0;
            while s + GAP_FRAME_SAMPLES <= m_end {
                rms.push(frame_rms(&wav, s));
                s += GAP_FRAME_SAMPLES;
            }
            rms.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let thr = GAP_RMS_RATIO
                * if rms.is_empty() {
                    0.0
                } else {
                    rms[rms.len() / 2]
                };
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            // 21:16 窗#1 复刻：prev = 窗#0 片(0..10.05s)，back=4s（rate 3.0 → tail_backtrack 4.0s）。
            if name.contains("211641") {
                let plen = (10.05 * RATE as f32) as usize;
                if wav.len() > plen {
                    let prev = &wav[..plen];
                    let back = 4 * RATE;
                    let lo = prev.len() - back;
                    let hi = prev.len().saturating_sub(GAP_FRAME_SAMPLES / 2);
                    let old =
                        find_gap_cut_gap_only(prev, 0, lo, hi, GAP_FRAME_SAMPLES).unwrap_or(lo);
                    let (nc, nk) = find_tail_cut_ex(prev, back);
                    eprintln!(
                        "[DIAG434-CASE] 211641 win#1 prev=0..10.05s back=4s old_cut={:.3}s(old_sil={}ms) new={:?} cut={:.3}s(new_sil={}ms) suffixΔ={:.3}s",
                        old as f32 / RATE as f32,
                        pause_frames_at(prev, old, thr) * GAP_FRAME_SAMPLES * 1000 / RATE,
                        nk,
                        nc as f32 / RATE as f32,
                        pause_frames_at(prev, nc, thr) * GAP_FRAME_SAMPLES * 1000 / RATE,
                        (old as f32 - nc as f32) / RATE as f32
                    );
                }
            }
            for back_s in [2.0f32, 4.0] {
                let back = (back_s * RATE as f32) as usize;
                if wav.len() <= back {
                    continue;
                }
                let lower = wav.len() - back;
                let upper = wav.len().saturating_sub(GAP_FRAME_SAMPLES / 2);
                let old = find_gap_cut_gap_only(&wav, 0, lower, upper, GAP_FRAME_SAMPLES)
                    .unwrap_or(lower);
                let (new_cut, kind) = find_tail_cut_ex(&wav, back);
                eprintln!(
                    "[DIAG434] {name} dur={dur:.1}s back={back_s} old_cut={:.3}s(old_sil={}ms) | new={:?} cut={:.3}s(new_sil={}ms) suffixΔ={:.3}s",
                    old as f32 / RATE as f32,
                    pause_frames_at(&wav, old, thr) * GAP_FRAME_SAMPLES * 1000 / RATE,
                    kind,
                    new_cut as f32 / RATE as f32,
                    pause_frames_at(&wav, new_cut, thr) * GAP_FRAME_SAMPLES * 1000 / RATE,
                    (old as f32 - new_cut as f32) / RATE as f32
                );
            }
        }
    }
}

// =====================================================================
// TEST-SYNC-430-434（阶段三·非作者护栏 · coder-1）：434 真停顿切点契约，
// 独立推导 + 定点合成信号（直流 0.5 有声 / 0.0 静音，帧 RMS 精确可算）。
// 生产零改动；只覆盖 `find_tail_cut_ex` / `real_pause_cut_candidates` 行为
// 与 381 共享内核未动锚点。白名单仅 fmt/check（未跑 cargo test）。
// =====================================================================
#[cfg(test)]
mod testsync434_tests {
    use super::{find_tail_cut, find_tail_cut_ex, TailCutKind, TAIL_CUT_MIN_PAUSE_MS};
    use crate::transcription::vad::{real_pause_cut_candidates, GAP_FRAME_SAMPLES};

    const RATE: usize = 16000;
    const FRAME: usize = GAP_FRAME_SAMPLES; // 320 = 20ms
                                            // 120ms ⇒ 6 帧（与生产 `min_frames` 公式一致：((120*16+320-1)/320).max(1)）。
    const MIN_FRAMES: usize = 6;

    /// 定点合成：`[0,len)` 直流有声（RMS 恒 0.5），`sil` 区间逐段置 0（静音）。
    /// 中位数必落有声帧 ⇒ 阈值 0.3×0.5=0.15，静音帧恒低于阈值。
    fn synth(len: usize, sil: &[(usize, usize)]) -> Vec<f32> {
        let mut a = vec![0.5f32; len];
        for &(s, e) in sil {
            for x in a.iter_mut().take(e).skip(s) {
                *x = 0.0;
            }
        }
        a
    }
    /// 第 `j` 帧（320 样本）样本区间。
    fn fr(j: usize) -> (usize, usize) {
        (j * FRAME, (j + 1) * FRAME)
    }
    /// 真停顿段（`cnt` 帧，自 `j0` 起）的期望中点样本。
    fn mid(j0: usize, cnt: usize) -> usize {
        (j0 * FRAME + FRAME / 2 + (j0 + cnt - 1) * FRAME + FRAME / 2) / 2
    }

    /// 契约 1a：连续 120ms（6 帧）低能量 ⇒ 真停顿（RealPause）。
    #[test]
    fn tsync434_pause_120ms_is_real() {
        // 6s 音频、回溯 2s ⇒ lower=4s（帧 200），upper≈6s-160。
        let back = 2 * RATE;
        let (s, e) = (fr(210).0, fr(215).1); // 恰 6 帧 120ms
        let a = synth(6 * RATE, &[(s, e)]);
        let (cut, kind) = find_tail_cut_ex(&a, back);
        assert_eq!(kind, TailCutKind::RealPause, "120ms 连续低能量须为真停顿");
        assert_eq!(cut, mid(210, 6), "切点须为静音段中点");
    }

    /// 契约 1b：仅 100ms（5 帧）低谷 ⇒ 不算真停顿（`real_pause` 候选为空）。
    #[test]
    fn tsync434_pause_100ms_not_real() {
        let back = 2 * RATE;
        let (s, e) = (fr(210).0, fr(214).1); // 恰 5 帧 100ms
        let a = synth(6 * RATE, &[(s, e)]);
        let lower = a.len() - back;
        let upper = a.len() - FRAME / 2;
        let cands = real_pause_cut_candidates(&a, lower, upper, FRAME, MIN_FRAMES);
        assert!(
            cands.is_empty(),
            "100ms < 120ms 门槛，不得入选真停顿，实测 {cands:?}"
        );
        let (_, kind) = find_tail_cut_ex(&a, back);
        assert_ne!(kind, TailCutKind::RealPause, "100ms 低谷不得走 RealPause");
    }

    /// 契约 1c：切点为静音段中点（200ms 段，10 帧，返回帧数亦断言）。
    #[test]
    fn tsync434_cut_is_pause_midpoint() {
        let back = 2 * RATE;
        let (s, e) = (fr(210).0, fr(219).1); // 恰 10 帧 200ms
        let a = synth(6 * RATE, &[(s, e)]);
        let lower = a.len() - back;
        let upper = a.len() - FRAME / 2;
        let cands = real_pause_cut_candidates(&a, lower, upper, FRAME, MIN_FRAMES);
        assert_eq!(cands.len(), 1, "单段静音应恰一候选");
        assert_eq!(cands[0].0, mid(210, 10), "候选中点须精确");
        assert_eq!(cands[0].1, 10, "段内帧数须为 10");
        let (cut, kind) = find_tail_cut_ex(&a, back);
        assert_eq!(kind, TailCutKind::RealPause);
        assert_eq!(cut, mid(210, 10));
    }

    /// 契约 2a：首轮区间多个真停顿 ⇒ 取最早者（离 len-back 最近，后缀不短于回溯）。
    #[test]
    fn tsync434_first_range_takes_earliest() {
        let back = 2 * RATE;
        let a = synth(6 * RATE, &[(fr(210).0, fr(215).1), (fr(250).0, fr(259).1)]);
        let (cut, kind) = find_tail_cut_ex(&a, back);
        assert_eq!(kind, TailCutKind::RealPause);
        assert_eq!(cut, mid(210, 6), "两停顿取首轮最早者，不得取靠后者");
    }

    /// 契约 2b：首轮区间无、2×回溯内有 ⇒ 取扩展区内离 len-back 最近者；后缀变长但不超整片。
    #[test]
    fn tsync434_expanded_takes_nearest_lower() {
        // 8s 音频、回溯 2s ⇒ lower=6s；停顿放在 5.0~5.16s（8 帧，仅扩展区可见）。
        let back = 2 * RATE;
        let (s, e) = (fr(250).0, fr(257).1);
        let a = synth(8 * RATE, &[(s, e)]);
        let lower = a.len() - back;
        let (cut, kind) = find_tail_cut_ex(&a, back);
        assert_eq!(kind, TailCutKind::RealPause, "扩展区应找到真停顿");
        assert_eq!(cut, mid(250, 8), "取扩展区内离 lower 最近者");
        assert!(cut < lower, "切点应在 len-back 之前（后缀变长）");
        let suffix = a.len() - cut;
        assert!(
            suffix > back && suffix <= a.len(),
            "后缀须 >back 且 ≤整片，实测 {suffix}"
        );
    }

    /// 契约 2c：首轮/扩展区皆无 ⇒ weak_gap / no_gap 退回旧行为（非 RealPause 即退回）。
    #[test]
    fn tsync434_no_pause_anywhere_falls_back() {
        // 4s 全程有声 ⇒ 无任何低能量段。
        let a: Vec<f32> = (0..4 * RATE)
            .map(|i| (i as f32 * 0.13).sin() * 0.5)
            .collect();
        let back = 2 * RATE;
        let lower = a.len() - back;
        let upper = a.len() - FRAME / 2;
        let cands = real_pause_cut_candidates(&a, lower, upper, FRAME, MIN_FRAMES);
        assert!(cands.is_empty(), "全程有声不得有真停顿候选");
        let wide = real_pause_cut_candidates(&a, 0, upper, FRAME, MIN_FRAMES);
        assert!(wide.is_empty(), "扩展区亦不得有候选");
        let (cut, kind) = find_tail_cut_ex(&a, back);
        assert_ne!(kind, TailCutKind::RealPause, "须退回旧行为");
        if kind == TailCutKind::NoGap {
            assert_eq!(cut, lower, "NoGap 回落 len-back");
        }
    }

    /// 契约 3：prev 短于回溯 ⇒ (0, NoGap) 整片；`find_tail_cut` bool 兼容。
    #[test]
    fn tsync434_short_prev_zero_nogap() {
        let a = vec![0.5f32; 1000];
        assert_eq!(
            find_tail_cut_ex(&a, 2 * RATE),
            (0, TailCutKind::NoGap),
            "不足 back ⇒ 整片"
        );
        assert_eq!(find_tail_cut(&a, 2 * RATE), (0, false));
    }

    /// 契约 3b：`gap_found` 兼容 —— RealPause ⇒ true；NoGap ⇒ false。
    #[test]
    fn tsync434_gap_found_compat() {
        let back = 2 * RATE;
        let (s, e) = (fr(210).0, fr(215).1);
        let a = synth(6 * RATE, &[(s, e)]);
        assert!(find_tail_cut(&a, back).1, "真停顿 ⇒ gap_found=true");
        let b: Vec<f32> = (0..4 * RATE)
            .map(|i| (i as f32 * 0.13).sin() * 0.5)
            .collect();
        let (cut_b, found_b) = find_tail_cut(&b, back);
        let kind_b = find_tail_cut_ex(&b, back).1;
        assert_eq!(
            found_b,
            kind_b != TailCutKind::NoGap,
            "bool 须恒等于 kind != NoGap"
        );
        let _ = cut_b;
    }

    /// 契约 4（源码锚点）：381 共享内核 `find_gap_cut_impl` / `find_gap_cut_gap_only`
    /// 未被改动 —— 生产区含定义 + 关键行（`concat!` 拆字面量防自匹配）。
    #[test]
    fn tsync434_source_anchors() {
        let prod = crate::guard_prod_lines::prod_lines_excluding_cfg_test(include_str!("vad.rs"))
            .join("\n");
        assert!(
            prod.contains(concat!("fn find_gap_cut_", "impl(")),
            "381 内核定义须在"
        );
        assert!(
            prod.contains(concat!(
                "let search_start = (SLIDING_CUT_SEARCH_START_SECS * 16000.0)",
                " as usize;"
            )),
            "381 基线关键行须在（前 10s 中位数）"
        );
        assert!(
            prod.contains(concat!("fn find_gap_cut_", "gap_only(")),
            "旧单帧入口定义须在"
        );
        assert!(
            prod.contains(concat!(
                "find_gap_cut_impl(audio, piece_start, lower, upper, frame)",
                ".0"
            )),
            "gap_only 须仍直调 impl（未改道）"
        );
        assert!(
            prod.contains(concat!("fn real_pause_cut_", "candidates(")),
            "434 新函数定义须在"
        );
    }

    /// 契约 0：120ms 门槛常量自证（`TAIL_CUT_MIN_PAUSE_MS=120`，Gavin 原话）。
    #[test]
    fn tsync434_threshold_constant() {
        assert_eq!(TAIL_CUT_MIN_PAUSE_MS, 120, "真停顿门槛须为 120ms");
        assert_eq!(
            (TAIL_CUT_MIN_PAUSE_MS as usize * 16 + FRAME - 1) / FRAME,
            MIN_FRAMES,
            "120ms/20ms ⇒ 6 帧，与生产公式一致"
        );
    }
}

// =====================================================================
// DISPATCH-LONG-SPEECH-442：满 10s 回看切点派发（与派发后兜底切片同一函数）
// =====================================================================
#[cfg(test)]
mod dispatch_long_speech_442_tests {
    use super::display_chars_for_raw_prefix;

    /// 生产区源码（截到本测试模块之前，防自匹配）。
    fn prod_src() -> String {
        let src = include_str!("local_stream.rs");
        let cut = src
            .find(concat!("mod dispatch_long_speech_442", "_tests"))
            .unwrap_or(src.len());
        src[..cut].to_string()
    }

    /// 切点那一刻的裸文本前缀 ⇒ 带标点显示文本里的覆盖字符数（插入的标点一并计入）。
    #[test]
    fn t442_display_chars_skip_inserted_punct() {
        let raw = "今天天气很好我们一起去公园散步然后";
        let display = "今天天气很好，我们一起去公园散步然后";
        let upto = "今天天气很好我们一起去公园散步".len();
        // 15 个裸字 + 1 个插入的逗号 = 16
        assert_eq!(display_chars_for_raw_prefix(display, raw, upto), 16);
        // 「然后」不被计入 ⇒ 回灌后仍作为流式尾巴保留（不回缩）。
        let n = display_chars_for_raw_prefix(display, raw, upto);
        let tail: String = display.chars().skip(n).collect();
        assert_eq!(tail, "然后");
    }

    /// 前缀为 0 / 超长 / 落在字中间（非 char 边界）都不 panic，且单调合理。
    #[test]
    fn t442_display_chars_boundary_safe() {
        let raw = "你好世界";
        let display = "你好，世界。";
        assert_eq!(display_chars_for_raw_prefix(display, raw, 0), 0);
        assert_eq!(display_chars_for_raw_prefix(display, raw, 999), 5);
        // 7 字节落在「世」中间 ⇒ 退到 6（「你好」）⇒ 2 字
        assert_eq!(display_chars_for_raw_prefix(display, raw, 7), 2);
    }

    /// 英文：标点引擎可能改大小写，宽松比较。
    #[test]
    fn t442_display_chars_case_insensitive() {
        let raw = "hello world again";
        let display = "Hello world, again.";
        assert_eq!(
            display_chars_for_raw_prefix(display, raw, "hello world".len()),
            11
        );
    }

    /// 接线：满 10s 回看、用与兜底切片同一函数、上限 11s、12s 才允许兜底、边界直接登记。
    #[test]
    fn t442_lookback_wiring() {
        let p = prod_src();
        assert!(p.contains(concat!("const LONG_SPEECH_LOOKBACK_MS: f32 = ", "10000.0")));
        assert!(p.contains(concat!(
            "const LONG_SPEECH_LOOKBACK_UPPER_MS: f32 = ",
            "11000.0"
        )));
        assert!(p.contains(concat!("const LONG_SPEECH_FALLBACK_MS: f32 = ", "12000.0")));
        assert!(p.contains(concat!(
            "super::vad::slice_cut_at(&pcm, lookback_origin",
            ", upper, allow_fallback)"
        )));
        assert!(p.contains(concat!(
            "on_reflow_commit(acc_seg_index, ",
            "Some(committed_len))"
        )));
        assert!(p.contains(concat!("acc_dispatched_end = ", "cut;")));
        // 已删除的「满 8s 遇 120ms 当场派发」不得复活（120ms 只是找切点的内部判据）。
        assert!(!p.contains(concat!("fn long_speech", "_dispatch(")));
    }

    /// 1200ms 静默派发原样保留（只认静默；与 10s 时长并列、谁先满足谁先派）。
    #[test]
    fn t442_silence_dispatch_kept() {
        let p = prod_src();
        assert!(p.contains(concat!("silent_ms >= silence_ms && ", "!done_for_pause")));
        assert!(p.contains(concat!(
            "const ACC_DISPATCH_SILENCE_MS_DEFAULT: f32 = ",
            "1200.0"
        )));
    }
}

// =====================================================================
// FIX-LATE-STREAM-TAIL-445：冻结边界后流式迟到字归入上一片
// =====================================================================
#[cfg(test)]
mod late_stream_tail_445_tests {
    use super::late_bound_step;

    fn prod_src() -> String {
        let src = include_str!("local_stream.rs");
        let cut = src
            .find(concat!("mod late_stream_tail_445", "_tests"))
            .unwrap_or(src.len());
        src[..cut].to_string()
    }

    /// 15:01 端测复现：边界 63 冻结后静默中又吐「算」「了」⇒ 两次吸收，边界 63→64→65。
    #[test]
    fn t445_absorbs_late_chars_while_silent() {
        let s0 = Some((3usize, 63usize));
        let (s1, a1) = late_bound_step(s0, false, false, 64);
        assert_eq!(a1, Some((3, 64)));
        let (s2, a2) = late_bound_step(s1, false, false, 65);
        assert_eq!(a2, Some((3, 65)));
        assert_eq!(s2, Some((3, 65)));
        // 无新字 ⇒ 不重登记、继续跟踪
        let (s3, a3) = late_bound_step(s2, false, false, 65);
        assert_eq!((s3, a3), (Some((3, 65)), None));
    }

    /// 开口（VAD 听到语音）⇒ 立即停止跟踪，新字属于新的话，不吸收。
    #[test]
    fn t445_stops_on_speech() {
        assert_eq!(
            late_bound_step(Some((3, 63)), true, false, 70),
            (None, None)
        );
    }

    /// 已有新派发在定界 ⇒ 停止跟踪（交给新片的 337 定界）。
    #[test]
    fn t445_stops_on_new_dispatch() {
        assert_eq!(
            late_bound_step(Some((3, 63)), false, true, 70),
            (None, None)
        );
    }

    #[test]
    fn t445_idle_when_not_tracking() {
        assert_eq!(late_bound_step(None, false, false, 99), (None, None));
    }

    /// 接线：a / c 两处冻结都开始跟踪；b（开口）清空；吸收时同步 438 坐标与下一片起点。
    /// STREAM-SV-463：原第三处（paraformer endpoint 分支的 c 兜底）随旧端点移除 —— 模拟流式的句末冻结
    /// 不改显示文本，未冻结的边界照常由 337 的 a / c 收口。
    #[test]
    fn t445_wiring() {
        let p = prod_src();
        assert_eq!(
            p.matches(concat!("late_bound = Some((seg, ", "cur));"))
                .count(),
            2
        );
        assert!(p.contains(concat!("late_bound = ", "None;")));
        for needle in [
            concat!("punct_head_chars = ", "cur;"),
            concat!("punct_tail_start = last_display", "_raw_len;"),
            concat!("acc_prev_committed = ", "cur;"),
        ] {
            assert!(p.contains(needle), "{needle}");
        }
    }
}

// =====================================================================
// FALLBACK-EXCLUDE-FARFIELD-446：流式线程记录远场区间并随每个派发点带出
// =====================================================================
#[cfg(test)]
mod far_timeline_446_tests {
    fn prod_src() -> String {
        let src = include_str!("local_stream.rs");
        let cut = src
            .find(concat!("mod far_timeline_446", "_tests"))
            .unwrap_or(src.len());
        src[..cut].to_string()
    }

    #[test]
    fn t446_far_recorded_and_sent_at_all_dispatch_points() {
        let p = prod_src();
        assert!(p.contains(concat!(
            "if vad_speech && !has_speech {\n            let (a, b) = (pcm.len(), ",
            "pcm.len() + chunk.len());"
        )));
        assert_eq!(
            p.matches(concat!(
                "slice_ranges_from_timeline(&far_timeline, ",
                "&spans)"
            ))
            .count(),
            3,
            "静默 / 回看 / 尾片三个派发点都带远场区间"
        );
    }
}
