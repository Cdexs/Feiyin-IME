//! ASR-SERVER-491 S3：一路远程会话（设计 §4「远程」列）。
//!
//! 与本机本地实时分支（`main.rs` `is_local_realtime_desired`）同构：预览线程跑
//! `transcribe_streaming_local`、精解线程跑 `localrt_session::run_acc_windows`，事件经
//! [`RemoteComposer`] 合成显示文本；收尾与本机同一路径（`committed + last_window` ⇒ 去口水重复 ⇒
//! 繁简规整）。与本机的差异全部由「角色」带出，不在这里另写判定：
//! - 精解用该终端自己的**远程推理通道**（`create_slot`）⇒ 本机优先让行、不做声纹剔除 / 注册
//!  （`transcribe_acc_ctx` 按通道角色判）；
//! - 预览 `PreviewRole::Remote` ⇒ 自己一份 VAD、关近场门、不读写本机音量沿用；
//! - 不进 `run_pipeline_core` ⇒ 不写本机使用统计、不自动学习词库、不上屏、不做 LLM 格式化
//!  （文字处理见 `text_stage`：内置节点固定开，终端只开关数字规整 / 补标点 / 翻译，S10）。
//! - S9 逐字时间：精解时间在「虚拟时间轴」（各派发片首尾相接，含补零）⇒ 按片映射换算成录音时间；
//!   文字处理（去口水重复 / 繁简规整）改了字 ⇒ `char_times::transfer` 搬运；补不出的标 `estimated`。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::compose::{RemoteComposer, REMOTE_GEN};
use super::frame::{EndPayload, OpenParams, ResultPayload, ResultWord};
use super::text_stage::{self, TextOpts};
use crate::config::AppConfig;
use crate::transcription::char_times::{self, CharTime, SliceMap, TsSource};
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

/// S9（T4）：整段兜底补对齐的录音时长上限（秒）。更长的录音退回预览粗略时间。
const WHOLE_ALIGN_MAX_SECS: f32 = 30.0;

/// S9：逐字时间（`None` 补估算，`hi` = 末尾估算上限秒）⇒ 阿里格式逐词。
fn result_words(text: &str, times: &[Option<CharTime>], hi: f32) -> Vec<ResultWord> {
    let filled = char_times::fill_missing(times, 0.0, hi);
    char_times::group_words(text, &filled)
        .into_iter()
        .map(|w| ResultWord {
            begin_time: w.begin_ms,
            end_time: w.end_ms,
            text: w.text,
            punctuation: w.punctuation,
            source: w.src.as_str().to_string(),
        })
        .collect()
}

/// 预览帧的估算上限：已知时间里最晚的终点（末尾暂无时间的字挤在它之后，下一帧再更新）。
fn known_end(times: &[Option<CharTime>]) -> f32 {
    times.iter().flatten().fold(0.0, |m, t| m.max(t.end))
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
) -> RemoteOutcome {
    let model_dir = transcription::model_dir();
    let script = config.audio.chinese_script;
    let silence_threshold = config.audio.silence_threshold;
    // 预览打点：远程不另开一份标点模型（CT-Transformer 常驻实例由本机录音线程独占）⇒ 预览无标点；
    // 终稿用 1.7B 原生标点 / 精解线程的进程级标点服务，与本机一致。
    let acc_cfg = transcription::local_stream::AccDispatchConfig::new();
    let acc_terms = session_terms(config, &params.vocabulary);
    // S10：补标点跟终端参数（精解失败片的兜底打点也一样），不跟本机设置。
    let acc_punct_enabled = params.punctuation;
    let opts = TextOpts {
        itn: params.itn,
        punctuation: params.punctuation,
        script,
        model_dir: model_dir.clone(),
    };
    // 预览线程「设备」标识：只用于近场门的音量沿用判据，远程已关门；带上设备代码便于日志区分。
    let vad_device = format!("remote:{}", params.device_id);

    let (event_tx, event_rx) = crossbeam_channel::unbounded::<PipelineEvent>();
    let (acc_tx, acc_rx) = crossbeam_channel::bounded::<AccInput>(64);
    // `PreviewModel` 是 Sync 包装（裸 recognizer 引用不能跨线程）；进线程后再取内部引用。
    let preview: &transcription::PreviewModel = &engines.preview;

    let mut composer = RemoteComposer::default();
    // S9：派发片录音位置（预览线程派发前登记，本线程换算精解时间时读）。
    let slices = Mutex::new(SliceMap::default());
    let slices_ref = &slices;
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
                transcription::local_stream::PreviewRole::Remote {
                    vad,
                    slices: slices_ref,
                },
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
                mut ev => {
                    // S9：精解时间（虚拟轴）⇒ 录音时间，再交给合成器。
                    if let PipelineEvent::PreviewReflow {
                        acc_times: Some(t), ..
                    } = &mut ev
                    {
                        slices_ref
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .map_times(t);
                    }
                    composer.on_event(ev)
                }
            };
            if push {
                // S10：预览也过内置节点（同音 / 数字规整 / 关标点全剥）。
                let (text, times) =
                    text_stage::apply_preview(composer.display(), composer.display_times(), &opts);
                emit(ResultPayload {
                    is_final: false,
                    words: result_words(&text, &times, known_end(&times)),
                    tentative: text_stage::preview_tentative(
                        composer.display(),
                        composer.tentative(),
                        &opts,
                    ),
                    text,
                    translation: None,
                });
            }
        }
        (preview_handle.join(), acc_handle.map(|h| h.join()))
    });

    if cancel.load(Ordering::Acquire) {
        return EndPayload::default().into();
    }
    let (preview_text, pcm) = match preview_result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) if e.is::<transcription::NoSpeechError>() => (String::new(), Vec::new()),
        Ok(Err(e)) => {
            log::error!("[ASR-SERVER-491] remote preview failed: {e:#}");
            return EndPayload {
                error: ERR_INTERNAL.into(),
                ..Default::default()
            }
            .into();
        }
        Err(_) => {
            log::error!("[ASR-SERVER-491] remote preview thread panicked");
            return EndPayload {
                error: ERR_INTERNAL.into(),
                ..Default::default()
            }
            .into();
        }
    };
    let (committed, last_window, vp_dropped_all, acc_times) = match acc_result {
        Some(Ok((c, lw, _, _, vp, t))) => (c, lw, vp, t),
        Some(Err(_)) => {
            log::error!("[ASR-SERVER-491] remote accuracy worker panicked");
            (String::new(), String::new(), false, None)
        }
        None => (String::new(), String::new(), false, None),
    };
    // 与本机同一收尾：滑窗终稿去口水重复 ⇒ 繁简规整。精解为空（失败 / 无语音）⇒ 退回预览收尾文本。
    let raw_joined = format!("{committed}{last_window}");
    let joined = apply_authoritative_filler_dedup(&raw_joined);
    let pcm_secs = pcm.len() as f32 / 16000.0;
    // S9：终稿来源文字的逐字录音时间（精解 = 换算 + 去口水搬运；整段兜底 = T4）。
    let (final_src, src_times) = if !joined.trim().is_empty() {
        let n = raw_joined.chars().count();
        let mut t = acc_times
            .filter(|t| t.len() == n)
            .unwrap_or_else(|| vec![None; n]);
        slices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .map_times(&mut t);
        let t = char_times::transfer(&raw_joined, &t, &joined);
        (joined, t)
    } else if vp_dropped_all {
        (String::new(), Vec::new())
    } else {
        let t = whole_fallback_times(slot, &pcm, &preview_text, &composer);
        (preview_text, t)
    };
    let text = text_normalizer::normalize_text_for_language(final_src.trim(), script);
    let times = char_times::transfer(&final_src, &src_times, &text);
    // S10：内置节点 + 终端三项参数；要了翻译 ⇒ 另出一份译文（失败不给）。
    let (text, times) = text_stage::apply_final(&text, &times, &opts);
    let translation = if params.translate {
        text_stage::translate(&text, config, &opts)
    } else {
        None
    };
    let chars = text.chars().filter(|c| !c.is_whitespace()).count();
    emit(ResultPayload {
        is_final: true,
        text: text.clone(),
        tentative: 0,
        words: result_words(&text, &times, pcm_secs),
        translation,
    });
    RemoteOutcome {
        end: EndPayload {
            error: String::new(),
            audio_secs: pcm.len() as f32 / 16000.0,
            chars,
        },
        text,
        pcm,
    }
}

/// S9（T4）：精解为空、终稿退回预览收尾文本时的逐字时间 —— 录音不长 ⇒ 整段强制对齐一次（录音时间轴，
/// 无需换算）；对不上 / 太长 ⇒ 从合成器的显示时间（预览粗略）搬过来；再没有 ⇒ `None`（输出前补估算）。
fn whole_fallback_times(
    slot: &AccEngine,
    pcm: &[f32],
    text: &str,
    composer: &RemoteComposer,
) -> Vec<Option<CharTime>> {
    if !text.trim().is_empty() && pcm.len() as f32 / 16000.0 <= WHOLE_ALIGN_MAX_SECS {
        if let Some(o) = slot.align(pcm, text) {
            if o.char_times.len() == text.chars().count() {
                return o
                    .char_times
                    .into_iter()
                    .map(|(begin, end)| {
                        Some(CharTime {
                            begin,
                            end,
                            src: TsSource::Aligned,
                        })
                    })
                    .collect();
            }
        }
    }
    char_times::transfer(composer.display(), composer.display_times(), text)
}

/// 一路远程会话的结果：`end` 回网关；终稿文本与整段音频留给统计（S5）。
pub(crate) struct RemoteOutcome {
    pub end: EndPayload,
    pub text: String,
    pub pcm: Vec<f32>,
}

impl From<EndPayload> for RemoteOutcome {
    fn from(end: EndPayload) -> Self {
        Self {
            end,
            text: String::new(),
            pcm: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ct(b: f32, e: f32, src: TsSource) -> Option<CharTime> {
        Some(CharTime {
            begin: b,
            end: e,
            src,
        })
    }

    /// 各词 `text + punctuation` 依次拼接 == 原文（电脑客户端词表与文本逐字对齐的前提）。
    fn joined(words: &[ResultWord]) -> String {
        words
            .iter()
            .map(|w| format!("{}{}", w.text, w.punctuation))
            .collect()
    }

    /// S9：混合中文 / 英文 / 数字 / 标点（含开头标点、连续标点、空白）⇒ 词表拼回原文；时间全缺也成立。
    #[test]
    fn s9_result_words_concat_equals_text() {
        for text in [
            "你好，world 2026年！",
            "Hello, 世界。",
            "，开头标点也要保留",
            "结尾是省略号……",
            "a1b2 c3",
            "纯中文没有标点",
            "“引号”和（括号）",
            "12:30 开会",
            "！！！",
        ] {
            let n = text.chars().count();
            // 全部缺时间 ⇒ 补成估算，词表照样拼回原文。
            let w = result_words(text, &vec![None; n], 3.0);
            assert_eq!(joined(&w), text, "全缺时间 text={text:?}");
            assert!(
                w.iter().all(|w| w.source == "estimated"),
                "全缺 ⇒ 全 estimated：{w:?}"
            );
            // 逐字都有对齐时间。
            let times: Vec<_> = (0..n)
                .map(|i| ct(i as f32 * 0.1, (i + 1) as f32 * 0.1, TsSource::Aligned))
                .collect();
            let w = result_words(text, &times, 9.9);
            assert_eq!(joined(&w), text, "全有时间 text={text:?}");
            assert!(w.iter().all(|w| w.source == "aligned"));
            // 词时间单调（begin <= end）。
            assert!(w.iter().all(|w| w.begin_time <= w.end_time), "{w:?}");
        }
    }

    /// S9：`None` 的字补成 `estimated`，已知字保留原来源；中间缺的字夹在相邻已知字之间。
    #[test]
    fn s9_result_words_none_gets_estimated_and_known_kept() {
        // 「甲乙丙」：乙无时间，夹在甲终点 0.2s 与丙起点 0.6s 之间。
        let times = vec![
            ct(0.1, 0.2, TsSource::Aligned),
            None,
            ct(0.6, 0.8, TsSource::Rough),
        ];
        let w = result_words("甲乙丙", &times, 5.0);
        assert_eq!(w.len(), 3);
        assert_eq!((w[0].begin_time, w[0].end_time), (100, 200));
        assert_eq!(w[0].source, "aligned");
        assert_eq!(w[1].source, "estimated");
        assert!(w[1].begin_time >= 200 && w[1].end_time <= 600, "{:?}", w[1]);
        assert_eq!((w[2].begin_time, w[2].end_time), (600, 800));
        assert_eq!(w[2].source, "rough");
        // 英文连写词取各字最差来源。
        let times = vec![
            ct(0.0, 0.1, TsSource::Aligned),
            ct(0.1, 0.2, TsSource::Rough),
            ct(0.2, 0.3, TsSource::Aligned),
        ];
        let w = result_words("abc", &times, 1.0);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].text, "abc");
        assert_eq!(w[0].source, "rough", "词来源 = 各字最差");
        assert_eq!((w[0].begin_time, w[0].end_time), (0, 300));
    }

    /// S9：末尾缺时间的字以 `hi` 为终点；空文本 ⇒ 空词表。
    #[test]
    fn s9_result_words_tail_uses_hi_and_empty_text() {
        let times = vec![ct(0.0, 0.5, TsSource::Aligned), None, None];
        let w = result_words("甲乙丙", &times, 2.5);
        assert_eq!(w.len(), 3);
        assert_eq!(w[0].end_time, 500);
        assert_eq!(w[1].begin_time, 500);
        assert_eq!(w[2].end_time, 2500, "末尾估算以 hi 为终点");
        assert!(result_words("", &[], 1.0).is_empty());
    }

    /// S9：`known_end` = 已知时间里最晚的终点；全缺 / 空 ⇒ 0。
    #[test]
    fn s9_known_end() {
        assert_eq!(known_end(&[]), 0.0);
        assert_eq!(known_end(&[None, None]), 0.0);
        let t = [
            ct(0.0, 1.5, TsSource::Aligned),
            None,
            ct(1.0, 2.25, TsSource::Rough),
            ct(0.5, 0.75, TsSource::Estimated),
        ];
        assert_eq!(known_end(&t), 2.25, "取最大终点，不看顺序");
    }

    // ─── TEST-SYNC-491 / S10 护栏 4-5 ──────────────────────────────────────

    /// S10 护栏 4（源码）：`session.rs` 生产区参数接线 + 节点调用次数。
    /// - `acc_punct_enabled = params.punctuation`（补标点跟终端参数，不跟本机 config）；
    /// - `apply_preview` 恰 1 次、`preview_tentative` 恰 1 次、`apply_final` 恰 1 次；
    /// - `translate` 只出现在 `if params.translate` 分支内。
    #[test]
    fn s10_session_param_wiring_and_call_counts() {
        let src = include_str!("session.rs");
        let prod = crate::guard_prod_lines::prod_lines_excluding_cfg_test(src);

        // 1. acc_punct_enabled = params.punctuation（锚点拆开防自匹配）。
        let acc_punct = concat!("let acc_punct_enabled = ", "params.punctuation");
        assert!(
            prod.iter().any(|l| l.contains(acc_punct)),
            "生产区应有 {acc_punct}"
        );

        // 2. 节点调用次数：apply_preview / preview_tentative / apply_final 各恰 1 次。
        let apply_preview = concat!("text_stage::", "apply_preview", "(");
        let preview_tentative = concat!("text_stage::", "preview_tentative", "(");
        let apply_final = concat!("text_stage::", "apply_final", "(");
        let n_preview = prod.iter().filter(|l| l.contains(apply_preview)).count();
        let n_tentative = prod
            .iter()
            .filter(|l| l.contains(preview_tentative))
            .count();
        let n_final = prod.iter().filter(|l| l.contains(apply_final)).count();
        assert_eq!(n_preview, 1, "{apply_preview} 应恰 1 次，实际 {n_preview}");
        assert_eq!(
            n_tentative, 1,
            "{preview_tentative} 应恰 1 次，实际 {n_tentative}"
        );
        assert_eq!(n_final, 1, "{apply_final} 应恰 1 次，实际 {n_final}");

        // 3. translate 只在 `if params.translate` 分支内：
        //    找到 translate 调用行，向上回溯应能看到 `if params.translate`。
        let translate_call = concat!("text_stage::", "translate", "(");
        let lines: Vec<&String> = prod.iter().collect();
        let pos = lines
            .iter()
            .position(|l| l.contains(translate_call))
            .expect("生产区应有 translate 调用");
        // 回溯 10 行找 if params.translate。
        let lo = pos.saturating_sub(10);
        let found = lines[lo..=pos]
            .iter()
            .any(|l| l.contains("if") && l.contains("params.translate"));
        assert!(
            found,
            "{translate_call} 必须在 if params.translate 分支内（向上回溯 10 行未找到）"
        );
    }

    /// S10 护栏 5（源码）：`session.rs` 生产区不得引用 `run_pipeline_core`。
    /// 该名字出现在文件顶部 doc 注释里 —— 扫描时先剔除 `//` 注释行。
    #[test]
    fn s10_session_prod_no_run_pipeline_core() {
        let src = include_str!("session.rs");
        let prod = crate::guard_prod_lines::prod_lines_excluding_cfg_test(src);
        let needle = concat!("run_", "pipeline_", "core");
        let code_only: Vec<&String> = prod
            .iter()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect();
        let hits: Vec<&String> = code_only
            .iter()
            .copied()
            .filter(|l| l.contains(needle))
            .collect();
        assert!(
            hits.is_empty(),
            "session.rs 生产区代码不得引用 {needle}，命中行: {hits:?}"
        );
    }
}
