//! ASR-SERVER-491 S1：本地实时会话的「精解窗口」工作线程（原 `main.rs` `spawn_worker_thread` 内联闭包，原样搬出）。
//!
//! 职责：收 `transcribe_streaming_local` 派发的切片 / 长静默信号 ⇒ 组窗（含前文后缀、末尾窗）⇒ 投解码池
//! （1.7B 精解 + 0.6B 强制对齐）⇒ 406 / 声纹 / 远场兜底 ⇒ `OrderedReflow` 有序拼接（按逐字时间）⇒
//! 经 `acc_event_tx` 发 `PreviewReflow` 回灌 ⇒ 结束时返回（定稿前缀, 最新窗口文本, all_native, 总解码 ms, 声纹全剔,
//! 远程会话的全文逐字时间）。
//!
//! 搬出的目的：本机录音会话与远程终端会话（DEC-111）共用**同一份**窗口逻辑，禁止复制第二份。
//! 函数体与搬出前逐字相同（只把闭包捕获改成参数），本机路径行为不变。

use super::*;

/// 精解窗口工作线程主体。参数即原闭包的 9 个捕获（名字不变）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_acc_windows(
    recognizer: &transcription::AccEngine,
    cancel_acc: Arc<AtomicBool>,
    acc_rx: crossbeam_channel::Receiver<AccInput>,
    acc_terms: Option<String>,
    acc_punct_dir: std::path::PathBuf,
    acc_punct_enabled: bool,
    acc_script: config::ChineseScript,
    acc_event_tx: crossbeam_channel::Sender<PipelineEvent>,
    session_generation: u64,
) -> (String, String, bool, f64, bool, AccTimes) {
    // ── SLIDING-WINDOW-367：滑动窗口（当前片+前1~3，12s 封顶）──
    // （阶段四·B）窗口解码投**并发池**（默认并发度 2，设 1 退顺序），
    // 结果按 window_seq **有序定稿**（对齐状态机串行）。
    let mut all_native = true;
    let mut total_decode_ms = 0.0f64;
    // VOICEPRINT-LEAK-460：本次录音各窗声纹保留 / 剔除的语音秒数累计。
    let (mut vp_kept_secs, mut vp_dropped_secs) = (0f32, 0f32);
    let mut recent_slices: Vec<Vec<f32>> = Vec::new();
    // FIX-WINDOW-DISJOINT-369：累计派发片总数 ⇒ 由 `recent_slices` 长度
    // 反推窗口的**全局切片区间**（传给 OrderedReflow 判重叠，别再靠文本猜）。
    let mut total_slices: usize = 0;
    // FORCED-ALIGN-456：各全局片的**累计结束样本**（虚拟时间轴：片首尾相接）。窗口音频 =
    // 某片后缀 + 若干整片、总在第 `ge-1` 片末尾结束 ⇒ 窗口绝对起点 = 该累计值 − 窗口样本和。
    let mut slice_cum_end: Vec<usize> = Vec::new();
    // FORCED-ALIGN-456：`window_seq -> (窗口绝对起点秒, 与上一窗分界绝对秒)`。
    let mut window_abs: Vec<(f32, f32)> = Vec::new();
    // `window_seq -> (start_slice, end_slice)`（dispatch 与 drain 同线程读写）。
    let mut window_spans: Vec<(usize, usize)> = Vec::new();
    // FIX-PREFIX-AND-EAT-371（B）：`window_seq -> 本窗各片样本数`
    // —— 与切片区间一起算期望重叠比例，约束 reflow 的对齐 k。
    let mut window_samples: Vec<Vec<usize>> = Vec::new();
    // 382（3A）：`window_seq -> 派发当刻浮层字符数`（边界未到时的 fallback，
    // 随 `PreviewReflow.committed_len` 带到消费端）。
    let mut window_committed_lens: Vec<usize> = Vec::new();
    // FIX-NEARFIELD-BY-SEGMENT-AND-PREVIEW-389（D3）：`window_seq -> 本窗
    // committed_len 是否准确边界`（部分窗 = false）。随 `PreviewReflow.boundary_usable`。
    let mut window_boundary_usable: Vec<bool> = Vec::new();
    // 386（C）：`window_seq -> 本窗各片流式文本拼接`（解码失败/空时的兜底）。
    let mut window_streaming_texts: Vec<String> = Vec::new();
    // FIX-TAIL-GUARD-410：`window_seq -> 末尾窗 pending 回灌元数据`（非末尾窗 None）。
    let mut window_tail_pending: Vec<Option<TailPending>> = Vec::new();
    // SEAM-INTERIOR-ONLY-436：`window_seq -> 本窗与上一窗重叠区的内部分界比例`
    // （相对重叠区起点；无共享 ⇒ None）。mod.rs 接线待第二阶段，本阶段仅计算 + 日志。
    let mut window_split_fracs: Vec<Option<f32>> = Vec::new();
    // 386（C）：与 `recent_slices` 一一对应的各片流式文本（同批 sub_seg 共享）。
    let mut recent_streaming: Vec<String> = Vec::new();
    // VAD-393（A3）：与 `recent_slices` 一一对应的各片**片内语音区间**
    //（`None` = VAD 不可用 / 该片未给区间 ⇒ 组窗后整窗回退自跑 VAD）。
    let mut recent_slice_ranges: Vec<Option<Vec<(usize, usize)>>> = Vec::new();
    // FALLBACK-EXCLUDE-FARFIELD-446：与 `recent_slices` 一一对应的片内远场区间。
    let mut recent_slice_far: Vec<Vec<(usize, usize)>> = Vec::new();
    // 446：`window_seq -> 窗内坐标远场区间`（兜底剔除用）。
    let mut window_far_ranges: Vec<Vec<(usize, usize)>> = Vec::new();
    // 386（A）：待覆盖片全局下标（多片派发的末片延后，等下次派发/收尾纳入）。
    let mut pending_slice: Option<usize> = None;
    // 386（A）：收尾窗用的最近 dispatch_idx / committed_len。
    let mut last_dispatch_idx: usize = 0;
    let mut last_committed_len: usize = 0;
    let mut window_seq: usize = 0;
    // FIX-PREVIEW-STALE-AND-COLLAPSE-375（A）：滑窗权威快照的**自有单调序号**
    // —— `dispatch_idx`（切片下标）在窗口间并发 + 收尾 drain 下**会重复**，
    // 不能拿它当「过期」判据（会把后到的完整文本误杀成 stale，预览永不更新）。
    let mut reflow_seq: usize = 0;
    // ACC-452 ⑤：本窗流式回灌已显示到的字数（分发循环收到正式结果即清零）。
    let mut partial_shown_len: usize = 0;
    let mut ordered = transcription::OrderedReflow::new();
    // LOCALRT-PREVIEW-HIDE-NONUSER-429：最近一次「已定稿权威全文」
    //（`OrderedReflow` 产出），供 full_drop 窗剔除合成使用。
    let mut last_authoritative = String::new();
    // ASR-SERVER-491 S9：远程会话才算逐字时间（随回灌带出、收尾返回）；本机恒不算 ⇒ 行为与开销不变。
    let timed = recognizer.is_remote();
    // S9：与 `last_authoritative` 逐字对应的时间（full_drop 窗复用上一份权威全文时一并复用）。
    let mut last_authoritative_times: AccTimes = None;
    let concurrency = transcription::WINDOW_DECODE_CONCURRENCY.max(1);
    let terms = acc_terms.as_deref();

    // PIPE-SPEED-457 ②：松键收尾时写入本次窗口总数（`usize::MAX` = 仍在录音）。
    // 解码线程据此认出「最后一窗」⇒ 不对齐，最终结果不等这 ~0.25s。
    let final_windows = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(usize::MAX));
    std::thread::scope(move |pool| {
        let (res_tx, res_rx) = crossbeam_channel::unbounded::<AccWorkerMsg>();
        // ACC-452 ⑤：解码中的半截结果 `(seq, dispatch_idx, 原文)`（流式回灌）。
        let (partial_tx, partial_rx) = crossbeam_channel::unbounded::<(usize, usize, String)>();
        // 382（3B）：**一个共享任务通道**，所有 worker 从同一通道取
        //（crossbeam 通道天然多消费者）。旧实现 `rr % concurrency` 把第 N 窗
        // 固定派给第 N%2 个 worker ⇒ 一个 worker 在解长窗时下一窗仍排在它后面、
        // 另一个空闲也不接（实测一窗实解 ~2.5s、出结果 5.8s）。
        let (task_tx, task_rx) = crossbeam_channel::unbounded::<AccTaskMsg>();
        for _ in 0..concurrency {
            let trx = task_rx.clone();
            let rtx = res_tx.clone();
            let ptx = partial_tx.clone();
            let final_windows = final_windows.clone();
            pool.spawn(move || {
                                                    // DRAFT-LANG-PREV-458：上一窗精解文字（本窗开头与上一窗尾部是同一段重叠音频）。
                                                    let mut prev_text = String::new();
                                                    for (
                                                        seq,
                                                        dispatch_idx,
                                                        audio,
                                                        avg_snapshot,
                                                        dispatched_at,
                                                        window_ranges,
                                                        streaming_nonempty,
                                                        new_slice_from,
                                                        hints,
                                                    ) in trx
                                                    {
                                                        // DRAFT-LANG-PREV-458：上一窗精解文字接在草稿后 ⇒ 重叠区推测解码可直接命中
                                                        //（草稿逐位验证，不改输出；回放连同语种标记每窗 −80ms）。
                                                        let mut hints = hints;
                                                        hints.draft.push_str(&prev_text);
                                                        // 382（3C）：排队时间 = 派发 → worker 开始解码。
                                                        let queued_ms =
                                                            dispatched_at.elapsed().as_secs_f64() * 1000.0;
                                                        let t0 = std::time::Instant::now();
                                                        // ACC-452 ⑤：本窗半截结果的限频计时（首次在解码开始后一个间隔才发）。
                                                        let partial_last =
                                                            std::sync::Mutex::new(std::time::Instant::now());
                                                        let r = decode_window(
                                                            recognizer,
                                                            &audio,
                                                            seq,
                                                            acc_script,
                                                            terms,
                                                            avg_snapshot,
                                                            window_ranges.as_deref(),
                                                            streaming_nonempty,
                                                            new_slice_from,
                                                            &hints,
                                                            Some(&|raw: &str| {
                                                                // ACC-452 ⑤：限频 ≥ PARTIAL_REFLOW_INTERVAL 才发一次（每字都发会刷屏）。
                                                                let mut last = partial_last
                                                                    .lock()
                                                                    .unwrap_or_else(|e| e.into_inner());
                                                                if last.elapsed() >= PARTIAL_REFLOW_INTERVAL {
                                                                    *last = std::time::Instant::now();
                                                                    let _ = ptx.send((seq, dispatch_idx, raw.to_string()));
                                                                }
                                                            }),
                                                        );
                                                        let ms = t0.elapsed().as_secs_f64() * 1000.0;
                                                        // 382（3C）：解码完成时刻（端到端「解完 → 浮层重画」）。
                                                        let decode_done_at = std::time::Instant::now();
                                                        if log::log_enabled!(log::Level::Debug) {
                                                            log::debug!(
                                                                "[LocalRT-DBG-382] window queue: seq={} queued_ms={:.0} decode_ms={:.0}",
                                                                seq,
                                                                queued_ms,
                                                                ms
                                                            );
                                                        }
                                                        // PIPE-SPEED-457 ①：先送精解结果（收割侧立即回灌，接缝暂按上一窗时间切），
                                                        // 再对齐；对齐时间随后送达、收割侧按时间重拼（结果与先对齐再拼逐位相同）。
                                                        let align_text = match &r {
                                                            Ok((t, _, _)) if !t.trim().is_empty() => Some(t.clone()),
                                                            _ => None,
                                                        };
                                                        if let Ok((t, _, _)) = &r {
                                                            prev_text = t.clone();
                                                        }
                                                        let _ = rtx.send(AccWorkerMsg::Decoded((
                                                            seq,
                                                            dispatch_idx,
                                                            r,
                                                            ms,
                                                            decode_done_at,
                                                        )));
                                                        let Some(t) = align_text else {
                                                            continue;
                                                        };
                                                        // PIPE-SPEED-457 ②：已松键且本窗是最后一窗 ⇒ 不对齐（最终结果不等这 ~0.25s；
                                                        // 该接缝按上一窗逐字时间切，456 半截结果同一路径）。
                                                        // ASR-SERVER-491 S9（T2）：远程会话要最后一句的逐字时间 ⇒ 照常对齐（本机不变）。
                                                        if !recognizer.is_remote()
                                                            && seq + 1 == final_windows.load(Ordering::Acquire)
                                                        {
                                                            log::debug!("[PIPE-457] last window: skip align seq={}", seq);
                                                            continue;
                                                        }
                                                        // FORCED-ALIGN-456：精解文字 × 本窗音频强制对齐（同一解码线程串行，GPU 不争抢）。
                                                        // 用整窗音频（非剪静音后）⇒ 时间直接是窗内坐标；收割侧若改用兜底文字则不用这份时间。
                                                        let t_align = std::time::Instant::now();
                                                        let aligned = recognizer.align(&audio, &t);
                                                        if log::log_enabled!(log::Level::Debug) {
                                                            let st = aligned.as_ref().map_or([0.0; 4], |o| o.ms);
                                                            log::debug!(
                                                                "[ALIGN-456] window align: seq={} ok={} ms={:.0}（编码 {:.0} / 主干 {:.0} / 头 {:.0}）",
                                                                seq,
                                                                aligned.is_some(),
                                                                t_align.elapsed().as_secs_f64() * 1000.0,
                                                                st[1],
                                                                st[2],
                                                                st[3]
                                                            );
                                                        }
                                                        if let Some(o) = aligned {
                                                            let _ = rtx.send(AccWorkerMsg::Aligned(seq, o.char_times));
                                                        }
                                                    }
                                                });
        }
        drop(res_tx);
        drop(task_rx); // 生产侧只保留唯一 `task_tx`；这份接收端不需要
                       // FIX-PREVIEW-STALE-AND-COLLAPSE-375（B）：本次录音**已接受窗口**的
                       // 产出率累计（字 / 秒 / 窗数）⇒ 运行均值，用于识别「解码坍塌」。
                       // （放在投池作用域内：唯一读写者就是这个循环，不跨闭包捕获）
        let mut rate_sum_chars: usize = 0;
        let mut rate_sum_secs: f32 = 0.0;
        let mut rate_windows: usize = 0;
        // FIX-PREVIEW-HARVEST-380（A）：本代**已处理**的结果数（收尾按 window_seq 等齐）。
        let mut done = 0usize;
        // PIPE-SPEED-457 ①：最近拼接、可按对齐时间重拼的窗 `(seq, dispatch_idx, 窗口绝对起点秒)`。
        let mut refine_meta: Option<(usize, usize, f32)> = None;
        // PHANTOM-406-459 ②：已派发未收割窗口的整窗音频（兜底文本对齐用；收割即移除）。
        let mut window_audio_keep: std::collections::HashMap<usize, Vec<f32>> =
            std::collections::HashMap::new();
        // FIX-PREVIEW-HARVEST-380（A）：结果处理**单一定义** —— select 循环与
        // 收尾 drain 共用，保证该线程内 `push_window(` 只出现一处（防两份再漂移）。
        // ACC-ENGINE-LLAMACPP-452 ⑤：流式回灌（Gavin「回灌刷新你可以直接做」）。
        // 解码中的半截结果 ⇒ 在 `OrderedReflow` **副本**上试拼接（真实状态不动）；
        // 仅当它是下一个待拼窗口、且试拼出的全文比当前显示更长（模型已写过重叠部分、
        // 开始出新内容）才发 `PreviewReflow`。整窗结果到达后照原流程（406 / 声纹 / 兜底）
        // 正式回灌覆盖；中途显示不参与任何判定，不改变最终文字。
        macro_rules! stream_partial_reflow {
                                                ($seq:expr, $idx:expr, $raw:expr) => {{
                                                    let seq: usize = $seq;
                                                    let dispatch_idx: usize = $idx;
                                                    let raw: String = $raw;
                                                    if seq == ordered.next_seq()
                                                        && !cancel_acc.load(Ordering::Acquire)
                                                        // 语种前缀须已完整（否则「language Chinese<asr」会被当正文，452 回放样例）。
                                                        && raw.contains("<asr_text>")
                                                    {
                                                        let stripped =
                                                            transcription::Transcriber::strip_asr_special_tokens(raw.trim());
                                                        let partial = text_normalizer::normalize_text_for_language(
                                                            stripped.trim(),
                                                            acc_script,
                                                        );
                                                        // 窗口开头是重识的前片后缀：半截结果须已越过重叠区（有效字 > 估算 + 余量）
                                                        // 才可能是新内容；否则试拼会把重叠字当新字追加（452 回放闪烁主因）。
                                                        let win_stream_ref = window_streaming_texts
                                                            .get(seq)
                                                            .map(|s| s.as_str())
                                                            .unwrap_or("");
                                                        let past_overlap = window_samples.get(seq).map_or(true, |smp| {
                                                            transcription::partial_past_overlap(&partial, win_stream_ref, smp)
                                                        });
                                                        if !partial.is_empty() && past_overlap {
                                                            let (ws, we) = window_spans
                                                                .get(seq)
                                                                .copied()
                                                                .unwrap_or((seq, seq + 1));
                                                            let smp = window_samples.get(seq).cloned().unwrap_or_default();
                                                            // 半截结果无逐字时间 ⇒ 用上一窗的逐字时间切（`prev_timed_cut`，与正式结果切点一致）。
                                                            let (win_abs_start, win_split_abs) =
                                                                window_abs.get(seq).copied().unwrap_or((0.0, 0.0));
                                                            let mut probe = ordered.clone();
                                                            let out = probe.push_window_timed(
                                                                seq, ws, we, smp, partial, None, win_abs_start, win_split_abs,
                                                            );
                                                            if let Some(raw_candidate) = out.last() {
                                                                let candidate = apply_authoritative_filler_dedup(raw_candidate);
                                                                let cand_len = candidate.chars().count();
                                                                let shown = partial_shown_len
                                                                    .max(last_authoritative.chars().count());
                                                                // 替换边界不在这里定：事件带 `partial: true`，`committed_len` 只作本窗
                                                                // 边界**上限**；浮层按 `partial_reflow_boundary` 现算精解写到预览的位置
                                                                //（BUILD-452 端测回缩：曾按整窗边界替换，5 字半截吞掉 21 字预览）。
                                                                if cand_len > shown {
                                                                    partial_shown_len = cand_len;
                                                                    if log::log_enabled!(log::Level::Debug) {
                                                                        log::debug!(
                                                                            "[ACC-452] partial reflow: seq={} chars={} (shown {})",
                                                                            seq,
                                                                            cand_len,
                                                                            shown
                                                                        );
                                                                    }
                                                                    let _ = acc_event_tx.send(PipelineEvent::PreviewReflow {
                                                                        generation: session_generation,
                                                                        seg_index: dispatch_idx,
                                                                        reflow_seq: Some(reflow_seq),
                                                                        committed_len: window_committed_lens
                                                                            .get(seq)
                                                                            .copied()
                                                                            .unwrap_or(0),
                                                                        boundary_usable: window_boundary_usable
                                                                            .get(seq)
                                                                            .copied()
                                                                            .unwrap_or(false),
                                                                        has_hole: false,
                                                                        // S9：半截结果本身无时间（`None` 字由会话侧估算）。
                                                                        acc_times: timed.then(|| {
                                                                            acc_times_for(raw_candidate, probe.full_times(), &candidate)
                                                                        }),
                                                                        acc_text: candidate,
                                                                        replace_all: true,
                                                                        decode_done_at: None,
                                                                        non_user_hide: false,
                                                                        partial: true,
                                                                    });
                                                                    reflow_seq += 1;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }};
                                            }
        macro_rules! harvest_acc_window {
                                                ($res:expr) => {{
                                                    let (seq, dispatch_idx, r, ms, decode_done_at) = $res;
                                                    total_decode_ms += ms;
                                                    // SPEAKER-VERIFY-408B：本窗声纹剔除统计（供 406 放宽）。
                                                    let win_drop = match &r {
                                                        Ok((_, _, stats)) => stats.clone(),
                                                        Err(_) => {
                                                            transcription::AccDropStats::default()
                                                        }
                                                    };
                                                    vp_kept_secs += win_drop.kept_speech_secs;
                                                    vp_dropped_secs += win_drop.dropped_speech_secs;
                                                    let text = match r {
                                                        Ok((t, _np, _stats)) => t,
                                                        Err(err) => {
                                                            all_native = false;
                                                            log::warn!(
                                                                "SLIDING-WINDOW-367 window #{} decode failed: {}",
                                                                seq,
                                                                err
                                                            );
                                                            String::new()
                                                        }
                                                    };
                                                    // 386（C）：解码 Err / 最终为空 ⇒ 用本窗**流式文本**兜底
                                                    //（流式也为空才保持空）；防结尾整段丢失（本次 seq7 `<location>` 丢 40+ 字）。
                                                    let decoded = text;
                                                    let fb_raw = window_streaming_texts
                                                        .get(seq)
                                                        .map(|s| s.as_str())
                                                        .unwrap_or("");
                                                    // FIX-VOICEPRINT-FALLBACK-421：① 整窗被声纹**全剔** ⇒ 该窗输出空，
                                                    // **绝不走流式兜底**（否则把他人语音放回）。仅限「因声纹剔除而空」；
                                                    // 模型自身解码为空（未剔除）仍按 386-C 兜底。
                                                    let full_drop = win_drop.dropped_speech_secs > 0.0
                                                        && win_drop.kept_speech_secs <= 0.0;
                                                    // ② 部分剔除 ⇒ 406 基准与兜底文本都只取**保留区间**对应流式。
                                                    let win_total: usize = window_samples
                                                        .get(seq)
                                                        .map(|v| v.iter().sum())
                                                        .unwrap_or(0);
                                                    let fb_owned = if win_drop.dropped_speech_secs > 0.0 {
                                                        kept_streaming_text(
                                                            fb_raw,
                                                            win_total,
                                                            0,
                                                            &win_drop.kept_ranges,
                                                            &win_drop.dropped_ranges,
                                                            None,
                                                        )
                                                    } else {
                                                        fb_raw.to_string()
                                                    };
                                                    if win_drop.dropped_speech_secs > 0.0
                                                        && log::log_enabled!(log::Level::Debug)
                                                    {
                                                        log::debug!(
                                                            "[LocalRT-DBG-421] kept streaming: seq={} dropped={:.2}s kept={:.2}s kept_ranges={} chars {}→{} (est, no-ts)",
                                                            seq,
                                                            win_drop.dropped_speech_secs,
                                                            win_drop.kept_speech_secs,
                                                            win_drop.kept_ranges.len(),
                                                            fb_raw.chars().count(),
                                                            fb_owned.chars().count()
                                                        );
                                                    }
                                                    let fb: &str = &fb_owned;
                                                    // FIX-ACC-MISMATCH-GUARD-406：精解非空时与**同窗流式文本**
                                                    // 比对内容保留度；不通过 ⇒ 用流式替换精解。🔴 仅本地实时滑窗路B。
                                                    // FIX-VOICEPRINT-FALLBACK-421：基准已收窄到保留部分 ⇒ **回到统一
                                                    // 406 门槛**（不再按剔除比例 scale 放宽，见 408B 的 clamp 已删）。
                                                    let verdict = transcription::acc_vs_streaming(&decoded, fb);
                                                    // 406 验收补：Debug 档下**每窗**打一行 verdict + 双方原文
                                                    // （各截前 80 字），供端测校准阈值 —— 此前缺的正是 acc 字符串不落盘。
                                                    if log::log_enabled!(log::Level::Debug) {
                                                        let trunc =
                                                            |s: &str| -> String { s.chars().take(80).collect() };
                                                        log::debug!(
                                                            "[LocalRT-DBG-406] verdict window #{}: retention={:.2} len_ratio={:.2} accept={} dropped={:.2}s acc_chars={} stream_chars={} acc=\"{}\" stream=\"{}\"",
                                                            seq,
                                                            verdict.retention,
                                                            verdict.len_ratio,
                                                            verdict.accept,
                                                            win_drop.dropped_speech_secs,
                                                            decoded.chars().count(),
                                                            fb.chars().count(),
                                                            trunc(&decoded),
                                                            trunc(fb)
                                                        );
                                                    }
                                                    let from_streaming =
                                                        !decoded.is_empty() && !verdict.accept;
                                                    if from_streaming {
                                                        log::warn!(
                                                            "[LocalRT-DBG-406] window #{} mismatch: retention={:.2} len_ratio={:.2} acc_chars={} stream_chars={} ⇒ keep streaming",
                                                            seq,
                                                            verdict.retention,
                                                            verdict.len_ratio,
                                                            decoded.chars().count(),
                                                            fb.chars().count()
                                                        );
                                                    }
                                                    // FIX-VOICEPRINT-FALLBACK-421：全剔窗 ⇒ 空且 `is_fallback=false`
                                                    //（既不走流式兜底、也不让 410 pending 兜底接手）。
                                                    // FALLBACK-EXCLUDE-FARFIELD-446：兜底文本再剔除「远场」区间对应的流式字
                                                    //（VAD 判人声但近场门判非近场 = 旁边播放 / 远处人声）。只作用于兜底，
                                                    // 406 比对基准与解码前剪裁不变（392：门不定内容去留，这里只在精解已失败时用）。
                                                    let win_far: &[(usize, usize)] = window_far_ranges
                                                        .get(seq)
                                                        .map(|v| v.as_slice())
                                                        .unwrap_or(&[]);
                                                    let fb_final: String = match fallback_ranges_with_far(
                                                        &win_drop.kept_ranges,
                                                        &win_drop.dropped_ranges,
                                                        win_far,
                                                    ) {
                                                        Some((k2, d2)) => {
                                                            kept_streaming_text(fb_raw, win_total, 0, &k2, &d2, None)
                                                        }
                                                        None => fb_owned.clone(),
                                                    };
                                                    if fb_final != fb_owned && log::log_enabled!(log::Level::Debug) {
                                                        log::debug!(
                                                            "[DBG-446] fallback far-field excluded: seq={} far_ranges={} chars {}→{}",
                                                            seq,
                                                            win_far.len(),
                                                            fb_owned.chars().count(),
                                                            fb_final.chars().count()
                                                        );
                                                    }
                                                    let (is_fallback, text) = window_final_text(
                                                        &decoded,
                                                        &fb_final,
                                                        verdict.accept,
                                                        full_drop,
                                                    );
                                                    if decoded.is_empty() && !text.is_empty() {
                                                        log::warn!(
                                                            "[LocalRT-DBG-386] window #{} fallback to streaming text ({} chars)",
                                                            seq,
                                                            text.chars().count()
                                                        );
                                                    }
                                                    // 410 item2/3：末尾窗被拒/空 ⇒ **只兜底 pending 片**
                                                    //（span (p,p+1)、文本 = pending 流式经标点服务打点）；前片结果一字不动。
                                                    let tail_meta =
                                                        window_tail_pending.get(seq).and_then(|o| o.as_ref());
                                                    let use_tail = tail_meta.is_some() && is_fallback;
                                                    let (win_ws, win_we, win_samples, text) = if use_tail {
                                                        let tp = tail_meta.unwrap();
                                                        // 421：pending 兜底同样只取**保留部分**（pending 段在窗内
                                                        // 偏移 = 窗总样本 − pending 样本）。
                                                        let tp_total: usize = tp.samples.iter().sum();
                                                        let tp_offset = win_total.saturating_sub(tp_total);
                                                        // 446：pending 兜底同样剔除远场区间（与常规窗同一函数）。
                                                        let pending_kept = match fallback_ranges_with_far(
                                                            &win_drop.kept_ranges,
                                                            &win_drop.dropped_ranges,
                                                            win_far,
                                                        ) {
                                                            Some((k2, d2)) => kept_streaming_text(
                                                                &tp.streaming,
                                                                tp_total,
                                                                tp_offset,
                                                                &k2,
                                                                &d2,
                                                                None,
                                                            ),
                                                            None if win_drop.dropped_speech_secs > 0.0 => {
                                                                kept_streaming_text(
                                                                    &tp.streaming,
                                                                    tp_total,
                                                                    tp_offset,
                                                                    &win_drop.kept_ranges,
                                                                    &win_drop.dropped_ranges,
                                                                    None,
                                                                )
                                                            }
                                                            None => tp.streaming.clone(),
                                                        };
                                                        let punct = punctuate_via_service(
                                                            &acc_punct_dir,
                                                            acc_punct_enabled,
                                                            &pending_kept,
                                                        );
                                                        if log::log_enabled!(log::Level::Debug) {
                                                            log::debug!(
                                                                "[LocalRT-DBG-410] fallback scope=pending punctuated={} seq={} chars={} kept={}",
                                                                punct != pending_kept,
                                                                seq,
                                                                punct.chars().count(),
                                                                pending_kept.chars().count()
                                                            );
                                                        }
                                                        (tp.span.0, tp.span.1, tp.samples.clone(), punct)
                                                    } else {
                                                        let (ws, we) = window_spans
                                                            .get(seq)
                                                            .copied()
                                                            .unwrap_or((seq, seq + 1));
                                                        let smp = window_samples
                                                            .get(seq)
                                                            .cloned()
                                                            .unwrap_or_default();
                                                        // 常规窗兜底（流式替换 / 精解空）⇒ 先打标点；精解被采纳 ⇒ 不动。
                                                        let t = if is_fallback {
                                                            let punct = punctuate_via_service(
                                                                &acc_punct_dir,
                                                                acc_punct_enabled,
                                                                &text,
                                                            );
                                                            if log::log_enabled!(log::Level::Debug) {
                                                                log::debug!(
                                                                    "[LocalRT-DBG-410] fallback scope=window punctuated={} seq={} chars={}",
                                                                    punct != text,
                                                                    seq,
                                                                    punct.chars().count()
                                                                );
                                                            }
                                                            punct
                                                        } else {
                                                            text
                                                        };
                                                        (ws, we, smp, t)
                                                    };
                                                    // 375：本窗秒数（供产出率均值）+ 最终字数（计入判据）
                                                    let win_secs =
                                                        win_samples.iter().sum::<usize>() as f32
                                                            / 16000.0;
                                                    let text_chars = text.chars().count();
                                                    // FORCED-ALIGN-456 / PIPE-SPEED-457 ①：精解结果先按无时间拼接（上一窗有时间 ⇒ 按其时间切）
                                                    // 立即回灌；本窗文字即精解原文（非流式 / pending 兜底）时，对齐时间晚到后
                                                    //（窗内秒 + 窗口绝对起点 = 录音虚拟时间轴）经 `refine_acc_window!` 按时间重拼。
                                                    let (win_abs_start, win_split_abs) =
                                                        window_abs.get(seq).copied().unwrap_or((0.0, 0.0));
                                                    refine_meta = (!is_fallback && !use_tail && text == decoded)
                                                        .then_some((seq, dispatch_idx, win_abs_start));
                                                    // PHANTOM-406-459 ②：兜底文本（流式）也做强制对齐 ⇒ 与下一窗的接缝按时间拼接（原无时间 ⇒
                                                    // 371 比例估算，端测曾多切 13 字）。仅常规窗、兜底文本未经声纹 / 远场剔除（与整窗音频一一对应）。
                                                    let win_audio = window_audio_keep.remove(&seq);
                                                    let fb_times: Option<Vec<(f32, f32)>> = if is_fallback
                                                        && !use_tail
                                                        && fb_final == fb_raw
                                                        && win_drop.dropped_speech_secs <= 0.0
                                                        && !text.trim().is_empty()
                                                    {
                                                        win_audio.as_deref().and_then(|a| recognizer.align(a, &text)).map(|o| {
                                                            o.char_times
                                                                .into_iter()
                                                                .map(|(a, b)| (win_abs_start + a, win_abs_start + b))
                                                                .collect()
                                                        })
                                                    } else {
                                                        None
                                                    };
                                                    if fb_times.is_some() && log::log_enabled!(log::Level::Debug) {
                                                        log::debug!("[PHANTOM-459] fallback text aligned: seq={} chars={}", seq, text.chars().count());
                                                    }
                            for authoritative in ordered.push_window_timed(
                                seq, win_ws, win_we, win_samples, text,
                                fb_times, win_abs_start, win_split_abs,
                            ) {
                                // REFLOW-FILLER-ONCE-441：回灌**前**去重（此处文本 = 对齐后的
                                // 权威全文 `committed + last_window`）；下方 `last_authoritative`
                                // （全剔窗复用）与 `acc_text` 同取这一份已去重文本。
                                // S9：只有「拼接器当前全文」才取得到时间（一次排空多窗时，中间几条无时间）。
                                // 只有远程才留一份去重前原文（本机不多一次拷贝）。
                                let raw = if timed {
                                    ordered
                                        .timed_snapshot(&authoritative)
                                        .map(|t| (authoritative.clone(), t))
                                } else {
                                    None
                                };
                                let authoritative =
                                    apply_authoritative_filler_dedup(&authoritative);
                                let acc_times = timed.then(|| match raw {
                                    Some((r, t)) => acc_times_for(&r, t, &authoritative),
                                    None => vec![None; authoritative.chars().count()],
                                });
                                last_authoritative_times = acc_times.clone();
                                // 382（3A）：边界未到时的 fallback = 派发当刻浮层字符数。
                                let fallback_committed = window_committed_lens
                                                            .get(seq)
                                                            .copied()
                                                            .unwrap_or(0);
                                                        // 429：记录最新权威全文（供 full_drop 窗的剔除合成用）。
                                                        last_authoritative = authoritative.clone();
                                                        let _ = acc_event_tx.send(
                                                            PipelineEvent::PreviewReflow {
                                                                generation: session_generation,
                                                                seg_index: dispatch_idx,
                                                                reflow_seq: Some(reflow_seq),
                                                                committed_len: fallback_committed,
                                                                boundary_usable:
                                                                    window_boundary_usable
                                                                        .get(seq)
                                                                        .copied()
                                                                        .unwrap_or(false),
                                                                has_hole: false,
                                                                acc_text: authoritative,
                                                                acc_times,
                                                                replace_all: true,
                                                                // 382（3C）：本窗解码完成时刻（端到端埋点）。
                                                                decode_done_at: Some(decode_done_at),
                                                                non_user_hide: false,
                                                                partial: false,
                                                            },
                                                        );
                                                        reflow_seq += 1;
                                                    }
                                                    // LOCALRT-PREVIEW-HIDE-NONUSER-429：声纹**整窗全剔**（421）
                                                    // ⇒ 立即刷新预览为「已定稿权威文本 + 该段之后的流式尾巴」，
                                                    // 剔除该段对应的流式文字（否则要等下一次回灌 / 松键才消失）。
                                                    // `OrderedReflow` 对空文本不动状态 ⇒ `last_authoritative` 仍是
                                                    // 上一窗的权威全文；`committed_len` = 派发当刻边界（本段末）。
                                                    if full_drop {
                                                        let fallback_committed = window_committed_lens
                                                            .get(seq)
                                                            .copied()
                                                            .unwrap_or(0);
                                                        let _ = acc_event_tx.send(
                                                            PipelineEvent::PreviewReflow {
                                                                generation: session_generation,
                                                                seg_index: dispatch_idx,
                                                                reflow_seq: Some(reflow_seq),
                                                                committed_len: fallback_committed,
                                                                boundary_usable: true,
                                                                has_hole: false,
                                                                acc_text: last_authoritative.clone(),
                                                                acc_times: last_authoritative_times.clone(),
                                                                replace_all: true,
                                                                decode_done_at: Some(decode_done_at),
                                                                non_user_hide: true,
                                                                partial: false,
                                                            },
                                                        );
                                                        reflow_seq += 1;
                                                        if log::log_enabled!(log::Level::Debug) {
                                                            log::debug!(
                                                                "[DBG-429] hide non-user segment: seq={} chars={}",
                                                                seq,
                                                                fallback_committed
                                                            );
                                                        }
                                                    }
                                                    // 375：只把「最终非空」的窗口计入均值
                                                    // （坍塌→空的窗口不拖低均值；374 剥离后重解出的真内容照常计入）
                                                    if text_chars > 0 {
                                                        rate_sum_chars += text_chars;
                                                        rate_sum_secs += win_secs;
                                                        rate_windows += 1;
                                                    }
                                                    done += 1;
                                                }};
                                            }
        // PIPE-SPEED-457 ①：第 `seq` 窗的对齐时间到达 ⇒ 按时间重拼该窗接缝；全文有变才回灌。
        // 只对「最近拼接、且文字即精解原文」的窗生效（`refine_meta` / `refine_times` 双重核对）。
        macro_rules! refine_acc_window {
            ($seq:expr, $times:expr) => {{
                let seq: usize = $seq;
                let times: Vec<(f32, f32)> = $times;
                if let Some((_, dispatch_idx, abs0)) = refine_meta.filter(|m| m.0 == seq) {
                    refine_meta = None;
                    let abs: Vec<(f32, f32)> =
                        times.iter().map(|&(a, b)| (abs0 + a, abs0 + b)).collect();
                    if let Some(raw_full) = ordered.refine_times(seq, abs) {
                        let full = apply_authoritative_filler_dedup(&raw_full);
                        // S9：重拼后时间才齐。文字没变不另发回灌（回灌节奏逐位不变），
                        // 时间留到下一次回灌 / 收尾 `finish_timed` 带出。
                        let acc_times =
                            timed.then(|| acc_times_for(&raw_full, ordered.full_times(), &full));
                        if timed {
                            // 同一段文字的更准时间 ⇒ full_drop 窗复用时也拿这份。
                            last_authoritative_times = acc_times.clone();
                        }
                        if full != last_authoritative {
                            if log::log_enabled!(log::Level::Debug) {
                                log::debug!(
                                    "[PIPE-457] seam refined: seq={} chars {}→{}",
                                    seq,
                                    last_authoritative.chars().count(),
                                    full.chars().count()
                                );
                            }
                            last_authoritative = full.clone();
                            let _ = acc_event_tx.send(PipelineEvent::PreviewReflow {
                                generation: session_generation,
                                seg_index: dispatch_idx,
                                reflow_seq: Some(reflow_seq),
                                committed_len: window_committed_lens.get(seq).copied().unwrap_or(0),
                                boundary_usable: window_boundary_usable
                                    .get(seq)
                                    .copied()
                                    .unwrap_or(false),
                                has_hole: false,
                                acc_text: full,
                                acc_times,
                                replace_all: true,
                                decode_done_at: None,
                                non_user_hide: false,
                                partial: false,
                            });
                            reflow_seq += 1;
                        }
                    }
                }
            }};
        }
        // 386（A）：派发一个窗口（含 C 的流式文本记账）。`gs/ge` = 全局切片区间。
        // 单一定义 ⇒ 批次路径与收尾合并共用，防两份漂移。
        macro_rules! dispatch_window {
                                                ($gs:expr, $ge:expr, $must_start:expr, $idx:expr, $win_committed:expr, $usable:expr) => {{
                                                    let gs: usize = $gs;
                                                    let ge: usize = $ge;
                                                    let must_start: usize = $must_start;
                                                    let buf_base = total_slices - recent_slices.len();
                                                    debug_assert!(
                                                        gs >= buf_base && ge <= total_slices,
                                                        "386：窗口区间 [{gs},{ge}) 必须落在当前 buffer [{buf_base},{total_slices}) 内"
                                                    );
                                                    let start = gs.saturating_sub(buf_base);
                                                    let stop = ge
                                                        .saturating_sub(buf_base)
                                                        .min(recent_slices.len());
                                                    // LOCALRT-SHORT-CONTEXT-413 / ALWAYS-CONTEXT-427：常规窗前文
                                                    // 只保留**紧邻 must_start 前一片**的后缀（同一套
                                                    // `take_context_suffix`），更早的上下文片整片丢弃 —— 避免末片
                                                    // 2.85s 也带上整段前文送解（BUILD-399 9.33s / decode 2194ms）。
                                                    // 427（Gavin 2026-09-25）：新片长致 `gs == must_start` 时，
                                                    // **仍**取紧邻前一片后缀（只要它还在缓冲内）⇒ 所有常规窗都带前文。
                                                    let ctx_global = short_context_span(ge, must_start, buf_base);
                                                    let must_local =
                                                        must_start.saturating_sub(buf_base).clamp(start, stop);
                                                    let (window_audio, window_samples_vec, window_ranges, window_streaming, span_start, tail_pending, window_far) =
                                                        if let Some(ctx_global) = ctx_global {
                                                            let ci = ctx_global.saturating_sub(buf_base);
                                                            let must_stream: String =
                                                                recent_streaming[must_local..stop].concat();
                                                            let rate_cps =
                                                                transcription::acc_avg_chars_per_sec(
                                                                    rate_sum_chars,
                                                                    rate_sum_secs,
                                                                    rate_windows,
                                                                )
                                                                .unwrap_or(3.0);
                                                            let cs = take_context_suffix(
                                                                &recent_slices[ci],
                                                                &recent_streaming[ci],
                                                                &must_stream,
                                                                rate_cps,
                                                            );
                                                            let mut audio: Vec<f32> =
                                                                recent_slices[ci][cs.cut..].to_vec();
                                                            let mut samples: Vec<usize> =
                                                                Vec::with_capacity(1 + stop - must_local);
                                                            samples.push(cs.suffix_samples);
                                                            for s in &recent_slices[must_local..stop] {
                                                                audio.extend_from_slice(s);
                                                                samples.push(s.len());
                                                            }
                                                            // VAD 区间：前文后缀的片内区间裁剪平移后，与 must 片
                                                            // 区间一起按样本数偏移拼成窗内坐标（任一源 None ⇒ 整窗 None）。
                                                            let ctx_ranges = recent_slice_ranges[ci]
                                                                .as_ref()
                                                                .map(|r| trim_and_shift_ranges(r, cs.cut));
                                                            let mut range_slices: Vec<
                                                                Option<Vec<(usize, usize)>>,
                                                            > = Vec::with_capacity(1 + stop - must_local);
                                                            range_slices.push(ctx_ranges);
                                                            range_slices.extend_from_slice(
                                                                &recent_slice_ranges[must_local..stop],
                                                            );
                                                            let ranges =
                                                                shift_and_concat_ranges(&range_slices, &samples);
                                                            if log::log_enabled!(log::Level::Debug) {
                                                                log::debug!(
                                                                    "[LocalRT-DBG-413] prev_cut_secs={:.2} window_secs={:.2} must_slices={} gap={} rate_cps={:.2}",
                                                                    cs.cut as f32 / 16000.0,
                                                                    audio.len() as f32 / 16000.0,
                                                                    stop - must_local,
                                                                    if cs.gap_found { "found" } else { "no_gap" },
                                                                    rate_cps
                                                                );
                                                            }
                                                            // 410 item2（413 R1）：带前文的常规窗若精解被 406 拒 / 为空，
                                                            // **只兜底 must 片**（`context_tail_pending`）；前文后缀是上一窗
                                                            // 已精解过的准确文本，不得被流式顶掉。与 407 末尾窗同一条 410 路径。
                                                            let tail_pending = Some(context_tail_pending(
                                                                must_start,
                                                                ge,
                                                                samples[1..].to_vec(),
                                                                must_stream,
                                                            ));
                                                            // 446：远场区间与 VAD 区间同法拼成窗内坐标。
                                                            let mut far_slices: Vec<Option<Vec<(usize, usize)>>> =
                                                                vec![Some(trim_and_shift_ranges(&recent_slice_far[ci], cs.cut))];
                                                            far_slices.extend(
                                                                recent_slice_far[must_local..stop].iter().cloned().map(Some),
                                                            );
                                                            let window_far =
                                                                shift_and_concat_ranges(&far_slices, &samples).unwrap_or_default();
                                                            (audio, samples, ranges, cs.baseline, ctx_global, tail_pending, window_far)
                                                        } else {
                                                            let audio: Vec<f32> = recent_slices[start..stop]
                                                                .iter()
                                                                .flat_map(|s| s.iter().copied())
                                                                .collect();
                                                            let samples: Vec<usize> = recent_slices
                                                                [start..stop]
                                                                .iter()
                                                                .map(|s| s.len())
                                                                .collect();
                                                            let ranges = shift_and_concat_ranges(
                                                                &recent_slice_ranges[start..stop],
                                                                &samples,
                                                            );
                                                            let streaming =
                                                                recent_streaming[start..stop].concat();
                                                            if log::log_enabled!(log::Level::Debug) {
                                                                log::debug!(
                                                                    "[LocalRT-DBG-413] prev_cut_secs=0.00 window_secs={:.2} must_slices={} gap=none",
                                                                    audio.len() as f32 / 16000.0,
                                                                    stop - start
                                                                );
                                                            }
                                                            // 无前文 ⇒ 整窗兜底（行为不变）。
                                                            let far_slices: Vec<Option<Vec<(usize, usize)>>> =
                                                                recent_slice_far[start..stop].iter().cloned().map(Some).collect();
                                                            let window_far =
                                                                shift_and_concat_ranges(&far_slices, &samples).unwrap_or_default();
                                                            (audio, samples, ranges, streaming, gs, None, window_far)
                                                        };
                                                    if !transcription::path_b_budget_ok(
                                                        window_audio.len(),
                                                        terms,
                                                    ) {
                                                        log::warn!(
                                                            "[FIX-REMOVE-HARDSPLIT-370] 窗口音频超 KV 预算（撞顶会被静默截断）：{:.1}s audio_tok={} inject_tok={} max_total_len={}",
                                                            window_audio.len() as f32 / 16000.0,
                                                            transcription::expected_audio_tokens(
                                                                window_audio.len()
                                                            ),
                                                            transcription::estimate_inject_tokens(terms),
                                                            transcription::PATH_B_MAX_TOTAL_LEN
                                                        );
                                                    }
                                                    let avg_snapshot =
                                                        transcription::acc_avg_chars_per_sec(
                                                            rate_sum_chars,
                                                            rate_sum_secs,
                                                            rate_windows,
                                                        );
                                                    let streaming_nonempty =
                                                        !window_streaming.trim().is_empty();
                                                    // 408B：本窗**最后一片**在窗内的起点样本下标（声纹注册只计新片）；
                                                    // 413 起窗内样本表已含前文后缀 ⇒ 按实际窗口样本累计，不能再用全局片长。
                                                    let new_slice_from: usize = window_samples_vec
                                                        [..window_samples_vec.len().saturating_sub(1)]
                                                        .iter()
                                                        .sum();
                                                    // SEAM-INTERIOR-ONLY-436：重叠区分界（`window_audio` move 进
                                                    // task 前先算；上一窗 we 取自已派发窗）。
                                                    let win_split_frac = window_overlap_split(
                                                        span_start,
                                                        window_spans.last().map(|&(_, e)| e),
                                                        &window_audio,
                                                        &window_samples_vec,
                                                    );
                                                    window_audio_keep.insert(window_seq, window_audio.clone());
                                                    let _ = task_tx.send((
                                                        window_seq,
                                                        $idx,
                                                        window_audio,
                                                        avg_snapshot,
                                                        std::time::Instant::now(),
                                                        window_ranges,
                                                        streaming_nonempty,
                                                        new_slice_from,
                                                        AccHints {
                                                            draft: window_streaming.clone(),
                                                        },
                                                    ));
                                                    // 413：窗实际只含 `[must_start-1, ge)`（无前文时 = `[gs,ge)`）
                                                    // ⇒ 对齐 span 同步收窄，`window_samples.len() == ge - span_start`。
                                                    window_abs.push(window_abs_geometry(
                                                        &slice_cum_end,
                                                        ge,
                                                        span_start,
                                                        window_spans.last().map(|&(_, e)| e),
                                                        &window_samples_vec,
                                                        win_split_frac,
                                                    ));
                                                    window_split_fracs.push(win_split_frac);
                                                    window_spans.push((span_start, ge));
                                                    window_samples.push(window_samples_vec);
                                                    window_streaming_texts.push(window_streaming);
                                                    window_far_ranges.push(window_far);
                                                    window_committed_lens.push($win_committed);
                                                    window_boundary_usable.push($usable);
                                                    // 410：带前文的常规窗 = 只兜底 must 片（见上）；无前文 = None（整窗兜底）。
                                                    window_tail_pending.push(tail_pending);
                                                    window_seq += 1;
                                                }};
                                            }
        // LOCALRT-TAIL-WINDOW-407：派发「末尾窗」——与 `dispatch_window!`
        // 同套记账，但**音频由调用方给定**（前片 `[cut..]` + pending，非整片），
        // `span` 仍以 (p-1,p+1) 交对齐。`win_samples` = [前片后缀, pending]。
        // VAD-REUSE-458：VAD 区间由调用方按常规窗同法拼好（前片区间 `trim_and_shift_ranges`
        // 裁到后缀 + pending 区间）；原固定传 `None` ⇒ 每个末尾窗都整窗重跑 VAD（~39ms 挡在解码前）。
        macro_rules! dispatch_tail_window {
                                                ($gs:expr, $ge:expr, $audio:expr, $samples:expr, $streaming:expr, $pending_samples:expr, $pending_streaming:expr, $far:expr, $ranges:expr) => {{
                                                    let gs: usize = $gs;
                                                    let ge: usize = $ge;
                                                    let audio: Vec<f32> = $audio;
                                                    let samples: Vec<usize> = $samples;
                                                    let streaming: String = $streaming;
                                                    let pending_samples: Vec<usize> = $pending_samples;
                                                    let pending_streaming: String = $pending_streaming;
                                                    if !transcription::path_b_budget_ok(audio.len(), terms) {
                                                        log::warn!(
                                                            "[LocalRT-TAIL-WINDOW-407] tail window over KV budget: {:.1}s audio_tok={} inject_tok={} max_total_len={}",
                                                            audio.len() as f32 / 16000.0,
                                                            transcription::expected_audio_tokens(audio.len()),
                                                            transcription::estimate_inject_tokens(terms),
                                                            transcription::PATH_B_MAX_TOTAL_LEN
                                                        );
                                                    }
                                                    let avg_snapshot = transcription::acc_avg_chars_per_sec(
                                                        rate_sum_chars,
                                                        rate_sum_secs,
                                                        rate_windows,
                                                    );
                                                    let streaming_nonempty = !streaming.trim().is_empty();
                                                    // 408B：末尾窗新片 = pending（末段）；前缀部分样本数 = samples[0]。
                                                    let new_slice_from =
                                                        if samples.len() >= 2 { samples[0] } else { 0 };
                                                    // SEAM-INTERIOR-ONLY-436：重叠区分界（`audio` move 进
                                                    // task 前先算；上一窗 we 取自已派发窗）。
                                                    let win_split_frac = window_overlap_split(
                                                        gs,
                                                        window_spans.last().map(|&(_, e)| e),
                                                        &audio,
                                                        &samples,
                                                    );
                                                    window_audio_keep.insert(window_seq, audio.clone());
                                                    let _ = task_tx.send((
                                                        window_seq,
                                                        last_dispatch_idx,
                                                        audio,
                                                        avg_snapshot,
                                                        std::time::Instant::now(),
                                                        $ranges,
                                                        streaming_nonempty,
                                                        new_slice_from,
                                                        AccHints {
                                                            draft: streaming.clone(),
                                                        },
                                                    ));
                                                    window_abs.push(window_abs_geometry(
                                                        &slice_cum_end,
                                                        ge,
                                                        gs,
                                                        window_spans.last().map(|&(_, e)| e),
                                                        &samples,
                                                        win_split_frac,
                                                    ));
                                                    window_split_fracs.push(win_split_frac);
                                                    window_spans.push((gs, ge));
                                                    window_samples.push(samples);
                                                    window_streaming_texts.push(streaming);
                                                    window_far_ranges.push($far);
                                                    window_committed_lens.push(last_committed_len);
                                                    window_boundary_usable.push(true);
                                                    // 410：末尾窗的 pending 回灌元数据（被拒/空时只兜底 pending）。
                                                    window_tail_pending.push(Some(TailPending {
                                                        span: (ge - 1, ge),
                                                        samples: pending_samples,
                                                        streaming: pending_streaming,
                                                    }));
                                                    window_seq += 1;
                                                }};
                                            }
        // LOCALRT-STOP-TAIL-UNIFY-423：**末尾窗统一组装**（长静默 407 与松键
        // 收尾共用，禁两份）。有前片 ⇒ 前片末尾后缀（`take_context_suffix`，
        // ≥2s/≥12 字、字缝切）+ pending；无前片（pending 是首片）⇒ 单独解 pending。
        // 410 pending 兜底元数据由 `dispatch_tail_window!` 统一填。
        macro_rules! emit_tail_window {
                                                ($p:expr, $pcm_pos:expr, $src:expr) => {{
                                                    let p: usize = $p;
                                                    let prev_base = total_slices - recent_slices.len();
                                                    if let Some((gs, ge, i)) =
                                                        tail_window_span(Some(p), prev_base, recent_slices.len())
                                                    {
                                                        let prev_slice = &recent_slices[i - 1];
                                                        let rate_cps = transcription::acc_avg_chars_per_sec(
                                                            rate_sum_chars,
                                                            rate_sum_secs,
                                                            rate_windows,
                                                        )
                                                        .unwrap_or(3.0);
                                                        let cs = take_context_suffix(
                                                            prev_slice,
                                                            &recent_streaming[i - 1],
                                                            &recent_streaming[i],
                                                            rate_cps,
                                                        );
                                                        let mut audio: Vec<f32> =
                                                            prev_slice[cs.cut..].to_vec();
                                                        audio.extend_from_slice(&recent_slices[i]);
                                                        let samples = vec![
                                                            cs.suffix_samples,
                                                            recent_slices[i].len(),
                                                        ];
                                                        let streaming = cs.baseline;
                                                        if log::log_enabled!(log::Level::Debug) {
                                                            let gap = if cs.gap_found { "found" } else { "no_gap" };
                                                            log::debug!(
                                                                "[LocalRT-DBG-407] tail window (src={}): pending={} prev_cut_secs={:.2} prev_suffix_chars={} backtrack_secs={:.2} rate_cps={:.2} gap={} window_secs={:.2} pcm_pos={}",
                                                                $src,
                                                                p,
                                                                cs.cut as f32 / 16000.0,
                                                                cs.suffix_chars,
                                                                cs.backtrack_secs,
                                                                rate_cps,
                                                                gap,
                                                                audio.len() as f32 / 16000.0,
                                                                $pcm_pos
                                                            );
                                                        }
                                                        // 446：前片后缀远场 + pending 远场，拼成窗内坐标（samples 传入宏前先算）。
                                                        let tail_far = shift_and_concat_ranges(
                                                                &[
                                                                    Some(trim_and_shift_ranges(&recent_slice_far[i - 1], cs.cut)),
                                                                    Some(recent_slice_far[i].clone()),
                                                                ],
                                                                &samples,
                                                            )
                                                            .unwrap_or_default();
                                                        // VAD-REUSE-458：前片后缀区间 + pending 区间（任一 None ⇒ 整窗 None，回退自跑 VAD）。
                                                        let tail_ranges = shift_and_concat_ranges(
                                                            &[
                                                                recent_slice_ranges[i - 1]
                                                                    .as_ref()
                                                                    .map(|r| trim_and_shift_ranges(r, cs.cut)),
                                                                recent_slice_ranges[i].clone(),
                                                            ],
                                                            &samples,
                                                        );
                                                        dispatch_tail_window!(
                                                            gs,
                                                            ge,
                                                            audio,
                                                            samples,
                                                            streaming,
                                                            vec![recent_slices[i].len()],
                                                            recent_streaming[i].clone(),
                                                            tail_far,
                                                            tail_ranges
                                                        );
                                                    } else if let Some((gs, ge)) =
                                                        tail_window_alone(Some(p), prev_base, recent_slices.len())
                                                    {
                                                        // 423：pending 是首片（无前片）⇒ 单独解（与 407 同）。
                                                        let i0 = p - prev_base;
                                                        let audio: Vec<f32> = recent_slices[i0].clone();
                                                        let samples = vec![recent_slices[i0].len()];
                                                        let streaming = recent_streaming[i0].clone();
                                                        if log::log_enabled!(log::Level::Debug) {
                                                            log::debug!(
                                                                "[LocalRT-DBG-423] tail window alone (src={}): pending={} window_secs={:.2}",
                                                                $src,
                                                                p,
                                                                audio.len() as f32 / 16000.0
                                                            );
                                                        }
                                                        dispatch_tail_window!(
                                                            gs,
                                                            ge,
                                                            audio,
                                                            samples,
                                                            streaming,
                                                            vec![recent_slices[i0].len()],
                                                            recent_streaming[i0].clone(),
                                                            recent_slice_far[i0].clone(),
                                                            recent_slice_ranges[i0].clone()
                                                        );
                                                    }
                                                }};
                                            }
        // FIX-PREVIEW-HARVEST-380（A）：select 循环 —— 新切片与解码结果
        // **任一先到即处理**，不再等下一个切片（Gavin 现象①：停顿即停刷）。
        let mut step = |ev: AccWindowStep<AccInput, AccWorkerMsg, (usize, usize, String)>| {
            match ev {
                AccWindowStep::Slice(input) => match input {
                    AccInput::Slice((
                        idx,
                        committed_len,
                        sub_segs,
                        seg_streaming,
                        seg_ranges,
                        seg_far,
                    )) => {
                        // 386（A）：按「中途末片延后 / 下次派发纳入 / 收尾合并」规则组窗
                        //（规则与不变量见 `plan_windows`）。382 旧行为「每片立刻组窗」会把
                        // 11.27s ⇒ 10.15s+1.12s 的尾片单独成窗 ⇒ 模型念词表。
                        // 389（D3）：本次派发前的 committed_len（会话首次 0），供部分窗折算。
                        let prev_committed = last_committed_len;
                        last_dispatch_idx = idx;
                        last_committed_len = committed_len;
                        let prev_base = total_slices - recent_slices.len();
                        let prev_durs: Vec<f32> = recent_slices
                            .iter()
                            .map(|s| s.len() as f32 / 16000.0)
                            .collect();
                        let new_lens: Vec<usize> = sub_segs.iter().map(|s| s.len()).collect();
                        let total_new: usize = new_lens.iter().sum();
                        let n_new = sub_segs.len();
                        let new_durs: Vec<f32> =
                            sub_segs.iter().map(|s| s.len() as f32 / 16000.0).collect();
                        // 413：保存本批前遗留的待覆盖片（用于算首个窗口的 must_start）。
                        let pending_before = pending_slice;
                        let plan = plan_windows(&prev_durs, &new_durs, prev_base, pending_slice);
                        pending_slice = plan.pending;
                        // FIX-SPLIT-SLICE-STREAMING-411 埋点（Debug 守卫，仅多片）：
                        // 各片按样本占比分配到的流式字数 + 样本数。
                        if n_new >= 2 && log::log_enabled!(log::Level::Debug) {
                            let chars: Vec<usize> = (0..n_new)
                                .map(|k| {
                                    slice_streaming_text(k, &seg_streaming, &new_lens)
                                        .chars()
                                        .count()
                                })
                                .collect();
                            log::debug!(
                                                                    "[LocalRT-DBG-411] split streaming: slices={} chars={:?} samples={:?}",
                                                                    n_new,
                                                                    chars,
                                                                    new_lens
                                                                );
                        }
                        let mut wi = 0usize;
                        let mut cum_new = 0usize;
                        // 413：本批**首个**窗口可能要纳入 pending ⇒ must_start 不同。
                        let mut saw_window = false;
                        for (k, s) in sub_segs.into_iter().enumerate() {
                            cum_new += new_lens[k];
                            slice_cum_end
                                .push(slice_cum_end.last().copied().unwrap_or(0) + s.len());
                            recent_slices.push(s);
                            recent_slice_ranges
                                .push(seg_ranges.as_ref().and_then(|v| v.get(k)).cloned());
                            recent_slice_far.push(seg_far.get(k).cloned().unwrap_or_default());
                            recent_streaming.push(slice_streaming_text(
                                k,
                                &seg_streaming,
                                &new_lens,
                            ));
                            total_slices += 1;
                            while recent_slices.len() > transcription::WINDOW_MAX_SLICES {
                                recent_slices.remove(0);
                                recent_slice_ranges.remove(0);
                                recent_slice_far.remove(0);
                                recent_streaming.remove(0);
                            }
                            // 窗口以「当前片」收尾：其 end == 推送后的 total_slices。
                            let end = total_slices;
                            if wi < plan.windows.len() && plan.windows[wi].1 == end {
                                let (gs, ge) = plan.windows[wi];
                                wi += 1;
                                // 413：首个窗口若并入上一批待覆盖片 ⇒ must_start = 该片；
                                // 其余窗口 must_start = 窗末片（只有新片必须完整解）。
                                let must_start =
                                    must_start_for_window(gs, ge, pending_before, !saw_window);
                                saw_window = true;
                                // 389（D3）：含本次派发末片 ⇒ 准确边界；
                                // 部分窗 ⇒ 按前 0..=k 片样本占比折算 committed_len。
                                let usable = k + 1 == n_new;
                                let win_committed = if usable {
                                    committed_len
                                } else {
                                    partial_win_committed(
                                        prev_committed,
                                        committed_len,
                                        cum_new,
                                        total_new,
                                    )
                                };
                                dispatch_window!(gs, ge, must_start, idx, win_committed, usable);
                            }
                            // 否则：本片是**延后的待覆盖片**（本批末片），本轮不组窗。
                        }
                    }
                    // LOCALRT-TAIL-WINDOW-407：长静默（≥1900ms 无新语音）
                    // ⇒ 有 pending 就组「末尾窗」（前片后缀 + pending），
                    // 无需停止键；无 pending 什么都不做（1200ms 已处理）。
                    AccInput::LongSilence(pcm_pos) => {
                        let has_pending = pending_slice.is_some();
                        if log::log_enabled!(log::Level::Debug) {
                            log::debug!(
                                "[LocalRT-DBG-407] long silence 1900ms: pending={}",
                                if has_pending { "yes" } else { "no" }
                            );
                        }
                        if let Some(p) = pending_slice {
                            // 423：长静默与松键收尾**共用同一末尾窗组装**。
                            emit_tail_window!(p, pcm_pos, "long_silence");
                            // pending 已被末尾窗覆盖 ⇒ 清空；松键收尾不再重复处理。
                            pending_slice = None;
                        }
                    }
                },
                AccWindowStep::Result(AccWorkerMsg::Decoded(res)) => {
                    // ACC-452 ⑤：正式结果到达 ⇒ 流式回灌计数清零（以正式回灌为准）。
                    partial_shown_len = 0;
                    harvest_acc_window!(res)
                }
                AccWindowStep::Result(AccWorkerMsg::Aligned(seq, times)) => {
                    refine_acc_window!(seq, times)
                }
                AccWindowStep::Partial((seq, idx, raw)) => {
                    stream_partial_reflow!(seq, idx, raw)
                }
            }
        };
        drive_acc_windows(
            &acc_rx,
            &res_rx,
            &partial_rx,
            || cancel_acc.load(Ordering::Acquire),
            &mut step,
        );
        drop(step);
        // LOCALRT-STOP-TAIL-UNIFY-423：松键收尾有 pending ⇒ **一律**走与
        // 407 长静默相同的末尾窗（前片末尾后缀 + pending；pending 首片 ⇒
        // 单独解），不再按 <3s / ≥3s 分叉（Gavin 2026-09-25：「无论 1900ms
        // 触发还是松键触发，都该走同样的末尾组窗机制」）。若 407 已在
        // 1900ms 处理过 pending ⇒ 此处 `pending_slice` 已为 `None` ⇒ 不重复组窗。
        if let Some(p) = pending_slice.take() {
            emit_tail_window!(p, 0usize, "stop");
        }
        // 收尾：等齐所有在飞窗口（共 window_seq 条）。
        // select 期间已收的已计入 `done`；本段只等还没收的。
        drop(task_tx);
        // PIPE-SPEED-457 ②：告诉解码线程本次共几窗 ⇒ 最后一窗不对齐。
        final_windows.store(window_seq, Ordering::Release);
        // PIPE-SPEED-457 ①：收到通道关闭为止（= 解码线程全部退出，scope 本来就要等到这一刻，
        // 不额外等待）⇒ 在飞的对齐时间也一并重拼。
        while let Ok(msg) = res_rx.recv() {
            match msg {
                AccWorkerMsg::Decoded(res) => harvest_acc_window!(res),
                AccWorkerMsg::Aligned(seq, times) => refine_acc_window!(seq, times),
            }
        }
        if done < window_seq {
            log::warn!(
                "[PIPE-457] acc drain ended with {}/{} windows harvested",
                done,
                window_seq
            );
        }
        // 380：收尾 drain 之后不再有派发 ⇒ 产出率均值无人再读（原实现
        // 亦不在 drain 累计）。显式消费，避免 `unused_assignments` 假红。
        let _ = (rate_sum_chars, rate_sum_secs, rate_windows);
        // S9：远程会话带出全文逐字时间（虚拟时间轴；会话侧换算成录音时间、搬运到终稿）。
        let (committed, last_window_text, raw_times) = ordered.finish_timed();
        let final_times: AccTimes = timed.then(|| {
            raw_times
                .into_iter()
                .map(|o| o.map(aligned_char_time))
                .collect()
        });
        (
            committed,
            last_window_text,
            all_native,
            total_decode_ms,
            voiceprint_dropped_all(vp_kept_secs, vp_dropped_secs),
            final_times,
        )
    })
}

/// ASR-SERVER-491 S9：远程会话随回灌 / 收尾带出的逐字时间（虚拟时间轴；本机恒 `None`）。
pub(crate) type AccTimes = Option<Vec<Option<transcription::char_times::CharTime>>>;

fn aligned_char_time((begin, end): (f32, f32)) -> transcription::char_times::CharTime {
    transcription::char_times::CharTime {
        begin,
        end,
        src: transcription::char_times::TsSource::Aligned,
    }
}

/// S9（T6）：拼接器全文 `raw` 的逐字对齐时间 ⇒ 回灌文本 `shown`（去口水重复后）的逐字时间。
fn acc_times_for(
    raw: &str,
    times: Vec<Option<(f32, f32)>>,
    shown: &str,
) -> Vec<Option<transcription::char_times::CharTime>> {
    let t: Vec<_> = times
        .into_iter()
        .map(|o| o.map(aligned_char_time))
        .collect();
    if raw == shown {
        t
    } else {
        transcription::char_times::transfer(raw, &t, shown)
    }
}
