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
//! - 同时把收到的 PCM 累积进可选 `pcm_out`（给 2pass 离线纠错用；16kHz f32 ≈ 64KB/s）。
//! - `cancel_signal` 置位即提前收尾（`Relaxed` 读，与 qwen 路径同款）。
//!
//! 端点语义：paraformer streaming 在一次 endpoint 后需 `reset()` 才能开始下一句；
//! reset 后 `get_result()` 从空串重新累积，与 `StreamingAsrState` 的句切换天然对齐。
//!
//! ⚠️ 阶段一（本单）：函数尚未接线（枚举变体与 `Transcriber` 构建分支在 239-A 第二阶段），
//! 故暂标 `#[allow(dead_code)]`；二阶段接线时移除。

use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};

use sherpa_onnx::OnlineRecognizer;

use super::qwen_inference::{StreamingAsrState, WordTiming};

/// 本地流式 recognizer 采样率（streaming paraformer trilingual 固定 16kHz 单声道）。
const SAMPLE_RATE: i32 = 16000;

/// 本地真流式转录（边收音频边解码），与 `transcribe_streaming_realtime` 平行。
///
/// # 参数
/// - `chunk_rx`：实时音频块（16kHz f32 单声道）；channel 关闭 = 录音结束
/// - `recognizer`：常驻的流式 paraformer recognizer（由 `Transcriber` 预加载，跨调用复用）
/// - `cancel_signal`：置位则提前停止
/// - `on_result`：文本变化回调，传 `(display_text, display_words)`，与 qwen 路径同构
/// - `pcm_out`：可选输出缓冲，累积本次全部 PCM（16k f32），供 2pass 离线纠错复用
///
/// # 返回
/// `StreamingAsrState::final_text()` = confirmed + current（ASR-070：不丢尾字）。
#[allow(dead_code)]
pub fn transcribe_streaming_local(
    chunk_rx: crossbeam_channel::Receiver<Vec<f32>>,
    recognizer: &OnlineRecognizer,
    cancel_signal: Option<&AtomicBool>,
    mut on_result: impl FnMut(&str, &[WordTiming]),
    mut pcm_out: Option<&mut Vec<f32>>,
) -> Result<String> {
    let is_cancelled = || {
        cancel_signal
            .map(|s| s.load(Ordering::Relaxed))
            .unwrap_or(false)
    };

    let stream = recognizer.create_stream();
    let mut state = StreamingAsrState::new();
    let mut sentence_id: i64 = 0;
    let mut last_display = String::new();

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

        if let Some(buf) = pcm_out.as_deref_mut() {
            buf.extend_from_slice(&chunk);
        }

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

    Ok(state.final_text())
}
