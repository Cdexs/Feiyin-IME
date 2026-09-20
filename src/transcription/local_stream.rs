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

use sherpa_onnx::{OnlineParaformerModelConfig, OnlineRecognizer, OnlineRecognizerConfig};

use super::qwen_inference::{StreamingAsrState, WordTiming};

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

        pcm.extend_from_slice(&chunk);
        stream.accept_waveform(SAMPLE_RATE, &chunk);
        while recognizer.is_ready(&stream) {
            recognizer.decode(&stream);
        }

        let endpoint = recognizer.is_endpoint(&stream);
        if let Some(r) = recognizer.get_result(&stream) {
            if !r.text.is_empty() {
                // endpoint=true → 该句确认进 confirmed；false → 替换当前句中间结果。
                state.on_result(sentence_id, &r.text, endpoint, &[]);
                let display = state.display_text();
                if !display.is_empty() && display != last_display {
                    on_result(&display, &state.display_words());
                    last_display = display;
                }
            }
        }

        if endpoint {
            // 一句结束：reset 让下一句从空开始（sherpa 要求），句号与状态机同步递增。
            recognizer.reset(&stream);
            sentence_id += 1;
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
            let display = state.display_text();
            if !display.is_empty() && display != last_display {
                on_result(&display, &state.display_words());
            }
        }
    }

    Ok((state.final_text(), pcm))
}
