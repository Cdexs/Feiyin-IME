// FIX-SLICE-CUT-AT-GAP-381 · 实测 PoC（纯 `#[cfg(test)]`，不碰生产逻辑）
//
// §4 固定 10s 硬切 vs 新字缝切：忽略静默强行连续切整段 → 每片走生产解码
//    `transcribe_acc_ctx`（= main.rs `decode_window` 包装的函数；[POC-BYPASSES-PROD-WRAPPER-001]）
//    → 按序拼接 → 与参考文本算 CER。
// §5 窗口 12s vs 10s 质量代价：生产口径静默 1200ms 切片 → 新字缝切 → 组窗（12/10）
//    → 每窗解码 → `OrderedReflow::push_window` 合并 → CER。
//
// 运行：cargo test --bin feiyin-ime -- --ignored --nocapture poc_slice_cut_381_cer
//       cargo test --bin feiyin-ime -- --ignored --nocapture poc_window_10s_381
// 🔴 无 lib target ⇒ PoC 无法放 src/bin/ 或 tests/（拿不到 crate 内部：`transcribe_acc_ctx` 是
//    `pub(crate)`、`create_qwen3_recognizer` 是私有 fn），故与既有 `poc_qwen3_17b_351` /
//    `poc_366_threads` 同型放 `#[cfg(test)]`（主控 2026-09-23 认可）。

use super::{
    build_sliding_segments, create_qwen3_recognizer, group_window_start_secs, transcribe_acc_ctx,
    CtxInject, OrderedReflow, WINDOW_MAX_SECS, WINDOW_MAX_SLICES,
};
use crate::config::ChineseScript;
use std::path::{Path, PathBuf};
use std::time::Instant;

const ANSWERS: [&str; 3] = [
    "上一轮1/8决赛，凭借哈兰德梅开二度，挪威队2比1爆冷淘汰五届世界杯冠军巴西队，历史性闯入世界杯八强。如今，他们将在迈阿密挑战英格兰队。",
    "然而，据报道，近期挪威队内出现了疾病传播，多名球员受到发烧、咳嗽等症状困扰，球队正与时间赛跑，希望能在比赛前恢复最佳状态。",
    "水晶宫前锋约根·斯特兰德·拉尔森因发烧缺席了世界杯首战对阵伊拉克队前的训练，并最终无缘那场比赛。效力于意甲萨索洛的马库斯·霍尔姆格伦·佩德森虽然在小组赛第二轮对阵塞内加尔队时取得进球，但由于生病，缺席了上一场对阵巴西队的淘汰赛。",
];

const RATE: usize = 16_000;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_wav(root: &Path, rel: &str) -> (Vec<f32>, usize) {
    let w = sherpa_onnx::Wave::read(root.join(rel).to_str().expect("utf8")).expect("read wav");
    (w.samples().to_vec(), w.sample_rate() as usize)
}

// ---- CER（与既有 313/351 逐字相同口径：去标点空白后编辑距离）----
fn normalize_for_cer(text: &str) -> String {
    const PUNCT: &[char] = &[
        '。', '，', '、', '！', '？', '；', '：', '「', '」', '『', '』', '“', '”', '‘', '’', '…',
        '—', '.', ',', '!', '?', ';', ':', '"', '\'', '(', ')', '[', ']', ' ', '\t', '\n', '\r',
    ];
    text.chars().filter(|c| !PUNCT.contains(c)).collect()
}

fn edit_distance(a: &[char], b: &[char]) -> usize {
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    for i in 1..=m {
        let mut cur = vec![i; n + 1];
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[n]
}

fn cer(hyp: &str, reference: &str) -> f64 {
    let h: Vec<char> = normalize_for_cer(hyp).chars().collect();
    let r: Vec<char> = normalize_for_cer(reference).chars().collect();
    if r.is_empty() {
        return if h.is_empty() { 0.0 } else { 1.0 };
    }
    edit_distance(&h, &r) as f64 / r.len() as f64
}

/// §4(a) 固定秒数硬切（首刀 = `offset_secs + hard_secs`，之后每 `hard_secs` 一刀）。
fn fixed_cuts(total: usize, hard_secs: f64, offset_secs: f64) -> Vec<(usize, usize)> {
    let step = (hard_secs * RATE as f64) as usize;
    let shift = (offset_secs * RATE as f64) as usize;
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut k = 0usize;
    loop {
        let cut = shift + (k + 1) * step;
        if cut >= total {
            out.push((pos, total));
            break;
        }
        out.push((pos, cut));
        pos = cut;
        k += 1;
    }
    out
}

/// 逐片走**生产解码函数** `transcribe_acc_ctx`（与 `decode_window` 同参：Simplified / terms / avg=None），
/// 返回 (拼接文本, 每片文本, 解码秒)。打印每片区间与文本。
fn decode_cuts(
    rec: &sherpa_onnx::OfflineRecognizer,
    audio: &[f32],
    cuts: &[(usize, usize)],
    terms: Option<&str>,
    tag: &str,
) -> (String, Vec<String>, f64) {
    let mut all = String::new();
    let mut pieces = Vec::new();
    let mut secs = 0.0;
    for (i, &(a, b)) in cuts.iter().enumerate() {
        let inject = CtxInject {
            terms,
            avg_chars_per_sec: None,
            speech_ranges: None,
            streaming_nonempty: false,
        };
        let t0 = Instant::now();
        let (text, _) = transcribe_acc_ctx(rec, &audio[a..b], ChineseScript::Simplified, i, inject)
            .unwrap_or_default();
        secs += t0.elapsed().as_secs_f64();
        println!(
            "[POC381-4] {tag} piece{i} {:.2}-{:.2}s out={}字 text={}",
            a as f64 / RATE as f64,
            b as f64 / RATE as f64,
            text.chars().count(),
            text.replace('\n', " ")
        );
        all.push_str(&text);
        pieces.push(text);
    }
    (all, pieces, secs)
}

/// §5：复刻生产**静默 1200ms 切片**（无 lib/未抽函数，故按行复刻；见 result.md 引用的文件:行）。
/// - 累加：`local_stream.rs:596-610`（chunk RMS ≤ `silence_threshold` 累加，否则清零）
/// - 常量：`local_stream.rs:131` `ACC_DISPATCH_SILENCE_MS_DEFAULT = 1200.0`
/// - 派发：`local_stream.rs:950-960`（`silent_ms >= silence_ms && !done_for_pause && pending>0`）
/// - 阈值：`config.audio.silence_threshold` 默认 `0.01`（`config/mod.rs:326`）
fn production_silence_slices(audio: &[f32]) -> Vec<(usize, usize)> {
    const SILENCE_MS: f32 = 1200.0;
    const THRESH: f32 = 0.01;
    const CHUNK: usize = RATE / 10; // 100ms（audio 线程块粒度的代表值）
    let mut slices = Vec::new();
    let mut dispatched_end = 0usize;
    let mut silent_ms = 0.0f32;
    let mut done_for_pause = false;
    let mut pos = 0usize;
    while pos < audio.len() {
        let end = (pos + CHUNK).min(audio.len());
        let c = &audio[pos..end];
        let rms = (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt();
        if rms <= THRESH {
            silent_ms += 100.0;
            if silent_ms >= SILENCE_MS && !done_for_pause && end > dispatched_end {
                slices.push((dispatched_end, end));
                dispatched_end = end;
                done_for_pause = true;
            }
        } else {
            silent_ms = 0.0;
            done_for_pause = false;
        }
        pos = end;
    }
    // 松手后尾部 pending（生产在收尾时解；此处不丢）
    if dispatched_end < audio.len() {
        slices.push((dispatched_end, audio.len()));
    }
    slices
}

/// §5：把静默切片经**新字缝切**（`build_sliding_segments`）成 padded 子片，供组窗使用。
fn slice_into_subsegments(audio: &[f32]) -> Vec<Vec<f32>> {
    let total = audio.len();
    let mut sub = Vec::new();
    for (s, e) in production_silence_slices(audio) {
        sub.extend(build_sliding_segments(&[(s, e - s)], total, audio));
    }
    sub
}

/// §5：复刻 main.rs 逐片组窗 + `OrderedReflow::push_window` 合并（单线程串行解码）。
/// 返回 (最终文本, 窗口数, 平均窗秒, 最大窗秒, 零重叠拼接次数)。
fn run_windows(
    rec: &sherpa_onnx::OfflineRecognizer,
    sub_segs: &[Vec<f32>],
    cap: f32,
    terms: Option<&str>,
    tag: &str,
) -> (String, usize, f64, f64, usize) {
    let mut recent: Vec<Vec<f32>> = Vec::new();
    let mut total_slices = 0usize;
    let mut seq = 0usize;
    let mut ordered = OrderedReflow::new();
    let mut wins = 0usize;
    let mut dur_sum = 0.0f64;
    let mut dur_max = 0.0f64;
    let mut zero_overlap = 0usize;
    let mut prev_end: Option<usize> = None;
    for s in sub_segs {
        recent.push(s.clone());
        total_slices += 1;
        while recent.len() > WINDOW_MAX_SLICES {
            recent.remove(0);
        }
        let durs: Vec<f32> = recent
            .iter()
            .map(|x| x.len() as f32 / RATE as f32)
            .collect();
        let start = group_window_start_secs(&durs, cap);
        let base = total_slices - recent.len();
        let ws = base + start;
        let we = total_slices;
        let window_audio: Vec<f32> = recent[start..]
            .iter()
            .flat_map(|x| x.iter().copied())
            .collect();
        let samples: Vec<usize> = recent[start..].iter().map(|x| x.len()).collect();
        let secs = window_audio.len() as f64 / RATE as f64;
        let t0 = Instant::now();
        let (text, _) = transcribe_acc_ctx(
            rec,
            &window_audio,
            ChineseScript::Simplified,
            seq,
            CtxInject {
                terms,
                avg_chars_per_sec: None,
                speech_ranges: None,
                streaming_nonempty: false,
            },
        )
        .unwrap_or_default();
        let dt = t0.elapsed().as_secs_f64();
        if let Some(pe) = prev_end {
            if ws >= pe {
                zero_overlap += 1;
            }
        }
        prev_end = Some(we);
        dur_sum += secs;
        dur_max = dur_max.max(secs);
        println!(
            "[POC381-5] {tag} win{seq} span=[{ws},{we}) {:.2}s decode={:.2}s out={}字",
            secs,
            dt,
            text.chars().count()
        );
        ordered.push_window(seq, ws, we, samples, text);
        seq += 1;
        wins += 1;
    }
    let (c, l) = ordered.finish();
    (
        format!("{c}{l}"),
        wins,
        dur_sum / wins.max(1) as f64,
        dur_max,
        zero_overlap,
    )
}

/// §4：固定 10s 硬切 vs 新字缝切，4 组起点偏移的 CER 对照。
#[test]
#[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_slice_cut_381_cer"]
fn poc_slice_cut_381_cer() {
    let root = manifest_dir();
    let rec = create_qwen3_recognizer(&root.join("models")).expect("create Qwen3 recognizer");
    let (audio, rate) = read_wav(&root, "collab/research/audio-real-gavin/processed/full.wav");
    assert_eq!(rate, RATE);
    let total = audio.len();
    let reftext = ANSWERS.concat();
    println!(
        "[POC381-4] machine cores={} dur={:.2}s",
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(0),
        total as f64 / RATE as f64
    );

    // (b) 新字缝切（自然切点；网格无关 ⇒ 只跑一次）
    let gap_cuts = super::vad::plan_gap_cuts(&audio, 0, total);
    let (gap_text, _, gap_secs) = decode_cuts(&rec, &audio, &gap_cuts, None, "GAP");
    let gap_cer = cer(&gap_text, &reftext);
    println!(
        "[POC381-4] GAP cuts={:?} decode={:.1}s CER={:.4}",
        gap_cuts
            .iter()
            .map(|(a, _)| format!("{:.2}", *a as f64 / RATE as f64))
            .collect::<Vec<_>>(),
        gap_secs,
        gap_cer
    );
    println!("[POC381-4] GAP full={}", gap_text.replace('\n', " "));

    // (a) 固定 10s 硬切，起点偏移 0 / 1.3 / 2.7 / 4.1s
    for off in [0.0f64, 1.3, 2.7, 4.1] {
        let cuts = fixed_cuts(total, 10.0, off);
        let (text, _, secs) = decode_cuts(&rec, &audio, &cuts, None, "FIXED");
        let c = cer(&text, &reftext);
        println!(
            "[POC381-4] FIXED off={off:.1} cuts={:?} decode={:.1}s CER={:.4} delta_vs_GAP={:+.4}",
            cuts.iter()
                .map(|(a, _)| format!("{:.2}", *a as f64 / RATE as f64))
                .collect::<Vec<_>>(),
            secs,
            c,
            c - gap_cer
        );
        println!(
            "[POC381-4] FIXED off={off:.1} full={}",
            text.replace('\n', " ")
        );
    }
}

/// §5：同一份 full.wav，生产口径完整走一遍，比较 WINDOW_MAX_SECS=12 vs 10 的 CER。
#[test]
#[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_window_10s_381"]
fn poc_window_10s_381() {
    let root = manifest_dir();
    let rec = create_qwen3_recognizer(&root.join("models")).expect("create Qwen3 recognizer");
    let (audio, rate) = read_wav(&root, "collab/research/audio-real-gavin/processed/full.wav");
    assert_eq!(rate, RATE);
    let reftext = ANSWERS.concat();

    let silence = production_silence_slices(&audio);
    println!(
        "[POC381-5] 生产静默切片 {} 片: {:?}",
        silence.len(),
        silence
            .iter()
            .map(|(a, b)| format!(
                "{:.2}-{:.2}",
                *a as f64 / RATE as f64,
                *b as f64 / RATE as f64
            ))
            .collect::<Vec<_>>()
    );
    let sub = slice_into_subsegments(&audio);
    println!(
        "[POC381-5] 字缝切后子片 {} 片: {:?}",
        sub.len(),
        sub.iter()
            .map(|s| format!("{:.2}s", s.len() as f64 / RATE as f64))
            .collect::<Vec<_>>()
    );

    let (t12, w12, avg12, max12, z12) = run_windows(&rec, &sub, 12.0, None, "CAP12");
    let c12 = cer(&t12, &reftext);
    let (t10, w10, avg10, max10, z10) = run_windows(&rec, &sub, WINDOW_MAX_SECS, None, "CAP10");
    let c10 = cer(&t10, &reftext);

    println!(
        "[POC381-5] RESULT cap12 CER={:.4} wins={w12} avg={:.2}s max={:.2}s zero_overlap={z12}",
        c12, avg12, max12
    );
    println!(
        "[POC381-5] RESULT cap10 CER={:.4} wins={w10} avg={:.2}s max={:.2}s zero_overlap={z10}",
        c10, avg10, max10
    );
    println!(
        "[POC381-5] delta_CER(10-12)={:+.4} (判据 ≤ +0.0100)",
        c10 - c12
    );
    println!("[POC381-5] cap12 full={}", t12.replace('\n', " "));
    println!("[POC381-5] cap10 full={}", t10.replace('\n', " "));
}
