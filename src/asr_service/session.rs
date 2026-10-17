//! ASR-SERVER-491 S3：一路远程会话（设计 §4「远程」列）。
//!
//! 与本机本地实时分支（`main.rs` `is_local_realtime_desired`）同构：预览线程跑
//! `transcribe_streaming_local`、精解线程跑 `localrt_session::run_acc_windows`，事件经
//! [`RemoteComposer`] 合成显示文本；收尾与本机同一路径（`committed + last_window` ⇒ 去口水重复 ⇒
//! 繁简规整）。与本机的差异全部由「角色」带出，不在这里另写判定：
//! - 精解用该终端自己的**远程推理通道**（`create_slot`）⇒ 本机优先让行、不做声纹剔除 / 注册
//!  （`transcribe_acc_ctx` 按通道角色判）；
//! - 预览 `PreviewRole::Remote` ⇒ 自己一份 VAD、关近场门、不读写本机音量沿用；
//! - 不进 `run_pipeline_core` ⇒ 不写本机使用统计、不自动学习词库、不上屏、不做 LLM 后处理
//!  （后处理按节点由终端点选，S10）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::compose::{RemoteComposer, REMOTE_GEN};
use super::frame::{EndPayload, OpenParams, ResultPayload};
use crate::config::AppConfig;
use crate::transcription::{self, AccEngine, RemoteEngines};
use crate::{apply_authoritative_filler_dedup, localrt_session, text_normalizer, wordbook};
use crate::{AccInput, PipelineEvent};

/// 会话结束错误码（见 [`EndPayload::error`]）。
pub(crate) const ERR_INTERNAL: &str = "internal";

/// 本地词库词条 + 终端追加词表（追加在后；去重保序）⇒ 精解注入串。
fn session_terms(config: &AppConfig, vocabulary: &[String]) -> Option<String> {
    let model = transcription::AsrModel::from_config(&config.audio.asr_model);
    let mut words: Vec<String> = if model.uses_accuracy_engine() {
        wordbook::Wordbook::open()
            .and_then(|wb| wb.list_all())
            .map(|v| v.into_iter().map(|e| e.word).collect())
            .unwrap_or_else(|e| {
                log::warn!("[ASR-SERVER-491] wordbook unavailable for remote session: {e}");
                Vec::new()
            })
    } else {
        Vec::new()
    };
    for v in vocabulary {
        let v = v.trim();
        if !v.is_empty() && !words.iter().any(|w| w == v) {
            words.push(v.to_string());
        }
    }
    let s = transcription::build_hotwords_string(&words);
    (!s.is_empty()).then_some(s)
}

/// 跑完一路远程会话（阻塞到音频结束且收尾完成）。
///
/// - `audio_rx`：16k 单声道 PCM；发送端全部释放 = 终端说完（`Finish`）。
/// - `cancel`：置位 = 取消（不出终稿）。
/// - `emit`：推送结果（预览反复推、终稿最后推一次）。
pub(crate) fn run_remote_session(
    engines: &RemoteEngines,
    slot: &AccEngine,
    vad: Option<&transcription::VadSegmenter>,
    config: &AppConfig,
    params: &OpenParams,
    audio_rx: crossbeam_channel::Receiver<Vec<f32>>,
    cancel: Arc<AtomicBool>,
    mut emit: impl FnMut(ResultPayload),
) -> EndPayload {
    let model_dir = transcription::model_dir();
    let script = config.audio.chinese_script;
    let silence_threshold = config.audio.silence_threshold;
    // 预览打点：远程不另开一份标点模型（CT-Transformer 常驻实例由本机录音线程独占）⇒ 预览无标点；
    // 终稿用 1.7B 原生标点 / 精解线程的进程级标点服务，与本机一致。
    let acc_cfg = transcription::local_stream::AccDispatchConfig::new();
    let acc_terms = session_terms(config, &params.vocabulary);
    let acc_punct_enabled = config.punctuation.enabled;
    // 预览线程「设备」标识：只用于近场门的音量沿用判据，远程已关门；带上设备代码便于日志区分。
    let vad_device = format!("remote:{}", params.device_id);

    let (event_tx, event_rx) = crossbeam_channel::unbounded::<PipelineEvent>();
    let (acc_tx, acc_rx) = crossbeam_channel::bounded::<AccInput>(64);
    // `PreviewModel` 是 Sync 包装（裸 recognizer 引用不能跨线程）；进线程后再取内部引用。
    let preview: &transcription::PreviewModel = &engines.preview;

    let mut composer = RemoteComposer::default();
    let (preview_result, acc_result) = std::thread::scope(|scope| {
        let acc_handle = acc_cfg.enabled.then(|| {
            let cancel_acc = Arc::clone(&cancel);
            let acc_event_tx = event_tx.clone();
            let acc_punct_dir = model_dir.clone();
            let acc_terms = acc_terms.clone();
            scope.spawn(move || {
                localrt_session::run_acc_windows(
                    slot,
                    cancel_acc,
                    acc_rx,
                    acc_terms,
                    acc_punct_dir,
                    acc_punct_enabled,
                    script,
                    acc_event_tx,
                    REMOTE_GEN,
                )
            })
        });

        let preview_tx = event_tx.clone();
        let cancel_prev = Arc::clone(&cancel);
        let preview_handle = scope.spawn(move || {
            let text_tx = preview_tx.clone();
            let commit_tx = preview_tx.clone();
            let acc_tx_long = acc_tx.clone();
            let r = transcription::local_stream::transcribe_streaming_local(
                audio_rx,
                &preview.0,
                Some(&cancel_prev),
                None,
                silence_threshold,
                vad_device.as_str(),
                |display_text, words, tentative| {
                    let _ = text_tx.send(PipelineEvent::StreamingText(
                        REMOTE_GEN,
                        display_text.to_string(),
                        words.to_vec(),
                        tentative,
                    ));
                },
                acc_cfg,
                |idx, committed_len, segs, seg_streaming, seg_ranges, seg_far| {
                    let _ = acc_tx.send(AccInput::Slice((
                        idx,
                        committed_len,
                        segs,
                        seg_streaming,
                        seg_ranges,
                        seg_far,
                    )));
                },
                |pcm_pos| {
                    let _ = acc_tx_long.send(AccInput::LongSilence(pcm_pos));
                },
                |seg_index, committed_len| {
                    let _ = commit_tx.send(PipelineEvent::ReflowCommit {
                        generation: REMOTE_GEN,
                        seg_index,
                        committed_len,
                    });
                },
                transcription::local_stream::PreviewRole::Remote { vad },
            );
            // 预览收尾（= 本机 `StreamingFinalPreview` + `Processing`）：只发信号，音频文本留在返回值里。
            if let Ok((text, _)) = &r {
                let _ = preview_tx.send(PipelineEvent::StreamingFinalPreview(text.clone()));
            }
            r
        });
        // 本线程不持发送端 ⇒ 预览线程与精解线程都结束后，事件通道自然断开、下面的循环退出。
        drop(event_tx);

        for ev in event_rx.iter() {
            if cancel.load(Ordering::Acquire) {
                continue;
            }
            let push = match ev {
                PipelineEvent::StreamingFinalPreview(text) => {
                    composer.final_preview(&text);
                    composer.suppress();
                    true
                }
                ev => composer.on_event(ev),
            };
            if push {
                emit(ResultPayload {
                    is_final: false,
                    text: composer.display().to_string(),
                    tentative: composer.tentative(),
                });
            }
        }
        (preview_handle.join(), acc_handle.map(|h| h.join()))
    });

    if cancel.load(Ordering::Acquire) {
        return EndPayload::default();
    }
    let (preview_text, pcm) = match preview_result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) if e.is::<transcription::NoSpeechError>() => (String::new(), Vec::new()),
        Ok(Err(e)) => {
            log::error!("[ASR-SERVER-491] remote preview failed: {e:#}");
            return EndPayload {
                error: ERR_INTERNAL.into(),
                ..Default::default()
            };
        }
        Err(_) => {
            log::error!("[ASR-SERVER-491] remote preview thread panicked");
            return EndPayload {
                error: ERR_INTERNAL.into(),
                ..Default::default()
            };
        }
    };
    let (committed, last_window, vp_dropped_all) = match acc_result {
        Some(Ok((c, lw, _, _, vp))) => (c, lw, vp),
        Some(Err(_)) => {
            log::error!("[ASR-SERVER-491] remote accuracy worker panicked");
            (String::new(), String::new(), false)
        }
        None => (String::new(), String::new(), false),
    };
    // 与本机同一收尾：滑窗终稿去口水重复 ⇒ 繁简规整。精解为空（失败 / 无语音）⇒ 退回预览收尾文本。
    let joined = apply_authoritative_filler_dedup(&format!("{committed}{last_window}"));
    let final_src = if !joined.trim().is_empty() {
        joined
    } else if vp_dropped_all {
        String::new()
    } else {
        preview_text
    };
    let text = text_normalizer::normalize_text_for_language(final_src.trim(), script);
    let chars = text.chars().filter(|c| !c.is_whitespace()).count();
    emit(ResultPayload {
        is_final: true,
        text,
        tentative: 0,
    });
    EndPayload {
        error: String::new(),
        audio_secs: pcm.len() as f32 / 16000.0,
        chars,
    }
}
