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

/// 流式解码线程数。对齐 POC-LOCAL-STREAM-235 的实测最优（4 线程 RTF 最好）。
const LOCAL_STREAM_NUM_THREADS: i32 = 4;

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
    c.model_config.num_threads = LOCAL_STREAM_NUM_THREADS;
    c.model_config.provider = Some("cpu".to_string());
    c.model_config.debug = false;
    // DEC-067：本地预览用 greedy_search（streaming paraformer 仅支持 greedy）
    c.decoding_method = Some("greedy_search".to_string());
    // 端点：不开则 is_endpoint() 永不触发，sentence_end 分支成死代码
    c.enable_endpoint = true;
    c.rule1_min_trailing_silence = LOCAL_STREAM_RULE1_MIN_TRAILING_SILENCE;
    c.rule2_min_trailing_silence = LOCAL_STREAM_RULE2_MIN_TRAILING_SILENCE;
    c.rule3_min_utterance_length = LOCAL_STREAM_RULE3_MIN_UTTERANCE_LENGTH;

    OnlineRecognizer::create(&c).context("创建本地流式 (paraformer) recognizer 失败")
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
///
/// # 返回
/// `(final_text, pcm)`：
/// - `final_text` = `StreamingAsrState::final_text()` = confirmed + current（ASR-070 不丢尾字）
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
) -> Result<(String, Vec<f32>)> {
    let is_cancelled = || {
        cancel_signal
            .map(|s| s.load(Ordering::Relaxed))
            .unwrap_or(false)
    };

    let stream = recognizer.create_stream();
    let mut state = StreamingAsrState::new();
    let mut sentence_id: i64 = 0;
    let mut last_display = String::new();
    let mut pcm: Vec<f32> = Vec::new();
    // LOCALRT-PUNCT-TIMER-269：预览标点节流缓存（只对裸文本打点，杜绝重复）。
    let mut punct_cache = PunctPreviewCache {
        prefix: String::new(),
        raw_len: 0,
        last_punct_at: None,
    };
    // LOCALRT-PUNCT-TIMER-269-B：连续静默累计（ms）。独立于 sherpa endpoint，只驱动显示层打点。
    let mut silent_ms: f32 = 0.0;

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
        if let Some(r) = recognizer.get_result(&stream) {
            if !r.text.is_empty() {
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
                // endpoint=true → 该句确认进 confirmed；false → 替换当前句中间结果。
                state.on_result(sentence_id, &r.text, endpoint, &[]);
            }
        }

        if endpoint {
            // 一句结束：reset 让下一句从空开始（sherpa 要求），句号与状态机同步递增。
            recognizer.reset(&stream);
            sentence_id += 1;
        }

        // LOCALRT-PUNCT-TIMER-269-B：显示刷新，两条件「或」——
        //   ① 距上次打点 ≥4s（preview_display 内判 due）
        //   ② 连续静默 ≥800ms（本处独立计数）且自上次打点后有新内容
        // 两条件都**只刷新显示**，不动状态机（不 reset / 不切句 / 不改 sentence_id）。
        let raw_full = state.display_text();
        if !raw_full.is_empty() {
            let has_new =
                punct_cache.last_punct_at.is_none() || raw_full.len() > punct_cache.raw_len;
            let silence_due = silent_ms >= PUNCT_SILENCE_TRIGGER_MS && has_new;
            let display = preview_display(
                &raw_full,
                punctuation_engine.as_deref_mut(),
                &mut punct_cache,
                PUNCT_REFRESH_INTERVAL,
                silence_due,
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
                }
                on_result(&display, &state.display_words());
                last_display = display;
            }
        }
    }

    // flush 尾部：input_finished 后把剩余可解码帧吐完，再取最终结果（不丢尾字）。
    stream.input_finished();
    while recognizer.is_ready(&stream) {
        recognizer.decode(&stream);
    }
    if let Some(r) = recognizer.get_result(&stream) {
        if !r.text.is_empty() {
            state.on_result(sentence_id, &r.text, false, &[]);
            // 收尾强制打点一次，保证关闭前的预览带标点。
            let raw_full = state.display_text();
            let display = preview_display(
                &raw_full,
                punctuation_engine.as_deref_mut(),
                &mut punct_cache,
                PUNCT_REFRESH_INTERVAL,
                true,
            );
            if !display.is_empty() && display != last_display {
                on_result(&display, &state.display_words());
            }
        }
    }

    Ok((state.final_text(), pcm))
}
