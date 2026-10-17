//! POC-ASR-SERVER-491：一套模型同时服务「本机 + 远程」时本机受多大影响（只测不改生产）。
//!
//! Gavin 2026-10-17：「Poc 吧」；「多个终端如果同时调用的时候要可以并行处理。我记得之前……转录模型的调用是串行的吧？」
//! 方法：按真实说话节奏（墙钟）回放录音，每路会话两条线程 ——
//! - 预览：每 0.6s 把「当前句」（上一派发点到当前已到音频，最长 15s）送 SenseVoice 重解；
//! - 精解：音频到达派发点（生产同口径 `dispatch_slices`）后，把「上一片 + 本片」（≤20s）送 1.7B 精解 + 0.6B 强制对齐。
//!   精解 / 对齐引擎是生产同一份（内部各一把锁 ⇒ **现状串行**）。
//! 场景：M0 本机单独；M1 本机 + 2 路远程（共用同一份预览模型，排队）；M2 本机 + 2 路远程（远程另开一份 2 线程预览模型）。
//! 指标：本机每窗「派发点 → 精解 + 对齐完成」延迟、每次预览重解耗时（含排队）。结果写 `collab/evidence/491/poc491-concurrency.md`。
//! 运行：`cargo test --bin feiyin-ime poc491 -- --ignored --nocapture`

use super::local_stream::{create_local_stream_recognizer, preview_decode};
use super::{
    create_qwen3_recognizer, decode_accuracy_allow_empty, max_new_tokens_for, ChineseScript,
};
use sherpa_onnx::{
    OfflineModelConfig, OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const RATE: usize = 16000;
const PREVIEW_STEP: Duration = Duration::from_millis(600);
const PREVIEW_MAX_SECS: usize = 15;
const WINDOW_MAX_SECS: usize = 20;

/// sherpa 预览 recognizer 不保证同一实例可并发 ⇒ 一律经 `Mutex` 串行（与 `SendPreviewRecognizerRef` 同源论证）。
struct Rec(OfflineRecognizer);
// SAFETY：只经外层 Mutex 独占访问；sherpa C++ 对象可跨线程调用（同 local_stream.rs SendPreviewRecognizerRef）。
unsafe impl Send for Rec {}
unsafe impl Sync for Rec {}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn pick_wavs(n: usize) -> Vec<(PathBuf, Vec<f32>)> {
    let mut all = Vec::new();
    for dir in [
        root().join("collab/evidence/gavin-sessions"),
        root().join("target/release/debug-audio"),
    ] {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("wav") {
                continue;
            }
            let Some(w) = p.to_str().and_then(sherpa_onnx::Wave::read) else {
                continue;
            };
            if w.sample_rate() as usize != RATE {
                continue;
            }
            let secs = w.samples().len() / RATE;
            if (20..=90).contains(&secs) {
                all.push((p, w.samples().to_vec()));
            }
        }
    }
    all.sort_by_key(|(_, s)| std::cmp::Reverse(s.len()));
    all.truncate(n);
    all
}

/// 生产「静默 ≥1200ms 派发」口径（同 replay436_tests::dispatch_slices）。
fn dispatch_slices(audio: &[f32]) -> Vec<(usize, usize)> {
    const CHUNK: usize = 160;
    let (mut slices, mut done_end, mut silent_ms, mut fired, mut pos) =
        (Vec::new(), 0, 0.0f32, false, 0);
    while pos < audio.len() {
        let end = (pos + CHUNK).min(audio.len());
        let c = &audio[pos..end];
        let rms = (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt();
        if rms <= 0.01 {
            silent_ms += (end - pos) as f32 / 16.0;
            if silent_ms >= 1200.0 && !fired && end > done_end {
                slices.push((done_end, end));
                done_end = end;
                fired = true;
            }
        } else {
            silent_ms = 0.0;
            fired = false;
        }
        pos = end;
    }
    if done_end < audio.len() {
        slices.push((done_end, audio.len()));
    }
    slices
}

fn preview_rec(models: &Path, threads: i32) -> OfflineRecognizer {
    let dir = models.join(super::SENSEVOICE_MODEL_SUBDIR);
    let c = OfflineRecognizerConfig {
        model_config: OfflineModelConfig {
            sense_voice: OfflineSenseVoiceModelConfig {
                model: Some(dir.join("model.int8.onnx").to_string_lossy().to_string()),
                language: Some("auto".to_string()),
                use_itn: false,
            },
            tokens: Some(dir.join("tokens.txt").to_string_lossy().to_string()),
            num_threads: threads,
            provider: Some("cpu".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    OfflineRecognizer::create(&c).expect("预览模型")
}

#[derive(Default)]
struct Stats {
    /// 派发点 → 精解 + 对齐完成（ms）
    win_latency: Vec<f64>,
    /// 其中精解 / 对齐各自耗时（含排队）
    decode_ms: Vec<f64>,
    align_ms: Vec<f64>,
    /// 每次预览重解耗时（含排队）
    preview_ms: Vec<f64>,
}

fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s[((s.len() - 1) as f64 * p).round() as usize]
}

fn line(name: &str, v: &[f64]) -> String {
    format!(
        "| {name} | {} | {:.0} | {:.0} | {:.0} |",
        v.len(),
        pct(v, 0.5),
        pct(v, 0.9),
        pct(v, 1.0)
    )
}

/// 本机优先闸门（方案 §4「本机优先调度」的最小实现）：一窗「精解 + 对齐」作为一个单元占用显卡；
/// 本机有窗在等时，远程不许插队；正在解的那一窗不打断。
#[derive(Default)]
struct Gate {
    m: Mutex<(bool, usize)>, // (显卡占用中, 本机等待数)
    cv: std::sync::Condvar,
}

impl Gate {
    fn acquire(&self, local: bool) {
        let mut g = self.m.lock().unwrap();
        if local {
            g.1 += 1;
            while g.0 {
                g = self.cv.wait(g).unwrap();
            }
            g.1 -= 1;
        } else {
            while g.0 || g.1 > 0 {
                g = self.cv.wait(g).unwrap();
            }
        }
        g.0 = true;
    }
    fn release(&self) {
        self.m.lock().unwrap().0 = false;
        self.cv.notify_all();
    }
}

/// 一路会话：墙钟回放；返回统计。`gate` = `Some((闸门, 是否本机))` ⇒ 每窗精解 + 对齐经闸门。
fn run_session(
    audio: &[f32],
    start: Instant,
    acc: &super::AccEngine,
    prev: &Mutex<Rec>,
    gate: Option<(&Gate, bool)>,
) -> Stats {
    let slices = dispatch_slices(audio);
    let stats = Mutex::new(Stats::default());
    let at = |samples: usize| start + Duration::from_secs_f64(samples as f64 / RATE as f64);
    std::thread::scope(|s| {
        // 精解线程
        s.spawn(|| {
            for (i, &(st, en)) in slices.iter().enumerate() {
                let due = at(en);
                if let Some(d) = due.checked_duration_since(Instant::now()) {
                    std::thread::sleep(d);
                }
                let ws = if i > 0 { slices[i - 1].0 } else { st };
                let ws = ws.max(en.saturating_sub(WINDOW_MAX_SECS * RATE));
                let win = &audio[ws..en];
                let t0 = Instant::now();
                if let Some((g, local)) = gate {
                    g.acquire(local);
                }
                let cap = max_new_tokens_for(win.len() as f32 / RATE as f32);
                let text = decode_accuracy_allow_empty(
                    acc,
                    win,
                    None,
                    ChineseScript::Simplified,
                    Some(cap),
                    Default::default(),
                )
                .map(|(t, _)| t)
                .unwrap_or_default();
                let t1 = Instant::now();
                if !text.trim().is_empty() {
                    let _ = acc.align(win, &text);
                }
                if let Some((g, _)) = gate {
                    g.release();
                }
                let t2 = Instant::now();
                let mut g = stats.lock().unwrap();
                g.decode_ms.push((t1 - t0).as_secs_f64() * 1000.0);
                g.align_ms.push((t2 - t1).as_secs_f64() * 1000.0);
                g.win_latency.push((t2 - due).as_secs_f64() * 1000.0);
            }
        });
        // 预览线程
        s.spawn(|| {
            let mut tick = start + PREVIEW_STEP;
            loop {
                if let Some(d) = tick.checked_duration_since(Instant::now()) {
                    std::thread::sleep(d);
                }
                let avail = ((tick - start).as_secs_f64() * RATE as f64) as usize;
                if avail >= audio.len() {
                    break;
                }
                let sent = slices
                    .iter()
                    .map(|&(_, e)| e)
                    .filter(|&e| e <= avail)
                    .max()
                    .unwrap_or(0);
                let from = sent.max(avail.saturating_sub(PREVIEW_MAX_SECS * RATE));
                if avail - from >= RATE * 3 / 10 {
                    let t0 = Instant::now();
                    let g = prev.lock().unwrap();
                    let _ = preview_decode(&g.0, &audio[from..avail]);
                    drop(g);
                    stats
                        .lock()
                        .unwrap()
                        .preview_ms
                        .push(t0.elapsed().as_secs_f64() * 1000.0);
                }
                tick += PREVIEW_STEP;
            }
        });
    });
    stats.into_inner().unwrap()
}

/// 491-P2：同一批窗口，「1 份引擎串行」vs「2 份引擎各一线程并行」的总耗时 —— 回答
/// 「这块显卡上两路同时解，总吞吐能不能涨」。两份引擎 = 多推理通道的**性能上限**（多占内存仅测试时）。
#[test]
#[ignore = "POC-ASR-SERVER-491：需真模型 + 录音；--ignored 运行"]
fn poc491_parallel_engines() {
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let models = root().join("models");
    let load = || {
        let mut a = create_qwen3_recognizer(&models).expect("1.7B 须在位");
        a.attach_aligner(&models);
        a
    };
    let e1 = load();
    let e2 = load();
    let wavs = pick_wavs(3);
    let mut windows: Vec<Vec<f32>> = Vec::new();
    for (_, a) in &wavs {
        let sl = dispatch_slices(a);
        for i in 0..sl.len() {
            let ws = if i > 0 { sl[i - 1].0 } else { sl[i].0 };
            let ws = ws.max(sl[i].1.saturating_sub(WINDOW_MAX_SECS * RATE));
            windows.push(a[ws..sl[i].1].to_vec());
        }
    }
    let audio_secs: f64 = windows.iter().map(|w| w.len() as f64 / RATE as f64).sum();
    let job = |e: &super::AccEngine, w: &[f32]| -> (f64, f64) {
        let t0 = Instant::now();
        let cap = max_new_tokens_for(w.len() as f32 / RATE as f32);
        let text = decode_accuracy_allow_empty(
            e,
            w,
            None,
            ChineseScript::Simplified,
            Some(cap),
            Default::default(),
        )
        .map(|(t, _)| t)
        .unwrap_or_default();
        let t1 = Instant::now();
        if !text.trim().is_empty() {
            let _ = e.align(w, &text);
        }
        (
            (t1 - t0).as_secs_f64() * 1000.0,
            t1.elapsed().as_secs_f64() * 1000.0,
        )
    };
    // 预热两份
    for e in [&e1, &e2] {
        let _ = job(e, &windows[0]);
    }
    let mut md = format!(
        "\n# 491-P2 两路同时解码的吞吐（{}）\n\n{} 个窗口，合计音频 {:.1}s\n\n| 方式 | 总耗时 s | 每窗精解中位 ms | 每窗对齐中位 ms |\n| --- | --- | --- | --- |\n",
        e1.device(),
        windows.len(),
        audio_secs
    );
    for round in 0..2 {
        // 串行：一份引擎依次解全部窗口
        let t = Instant::now();
        let (mut d, mut a) = (Vec::new(), Vec::new());
        for w in &windows {
            let (x, y) = job(&e1, w);
            d.push(x);
            a.push(y);
        }
        let serial = t.elapsed().as_secs_f64();
        let _ = writeln!(
            md,
            "| 第{}轮 串行（1 份引擎） | {:.1} | {:.0} | {:.0} |",
            round + 1,
            serial,
            pct(&d, 0.5),
            pct(&a, 0.5)
        );
        // 并行：两份引擎从共享队列取窗
        let next = std::sync::atomic::AtomicUsize::new(0);
        let res = Mutex::new((Vec::new(), Vec::new()));
        let t = Instant::now();
        std::thread::scope(|s| {
            for e in [&e1, &e2] {
                let (next, res, windows) = (&next, &res, &windows);
                s.spawn(move || loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if i >= windows.len() {
                        break;
                    }
                    let (x, y) = job(e, &windows[i]);
                    let mut g = res.lock().unwrap();
                    g.0.push(x);
                    g.1.push(y);
                });
            }
        });
        let par = t.elapsed().as_secs_f64();
        let (d, a) = res.into_inner().unwrap();
        let _ = writeln!(
            md,
            "| 第{}轮 并行（2 份引擎） | {:.1} | {:.0} | {:.0} |",
            round + 1,
            par,
            pct(&d, 0.5),
            pct(&a, 0.5)
        );
        let _ = writeln!(
            md,
            "| 第{}轮 吞吐倍数 | {:.2}× | | |",
            round + 1,
            serial / par
        );
    }
    let out = root().join("collab/evidence/491");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("poc491-parallel.md"), &md).unwrap();
    println!("{md}");
}

/// 491-P2b：每窗耗时拆分（音频编码 / 提示预填充 / 逐字生成 / 对齐）—— 判断「批量并行」能省到哪一段。
/// 批量并行只对逐字生成有效（各路共用一次权重读取）；编码与预填充本来就是大矩阵、已吃满显卡。
/// 须带 `LAS_SPLIT_ENC=1` 运行（否则编码耗时含在预填充里）。
#[test]
#[ignore = "POC-ASR-SERVER-491：需真模型 + 录音；--ignored 运行"]
fn poc491_breakdown() {
    let models = root().join("models");
    let mut acc = create_qwen3_recognizer(&models).expect("1.7B 须在位");
    acc.attach_aligner(&models);
    let wavs = pick_wavs(3);
    let mut md = format!("\n# 491-P2b 每窗耗时拆分（{}）\n\n| 窗 | 音频 s | 编码 ms | 预填充(不含编码) ms | 生成 ms | 生成字数 | 对齐 ms |\n| --- | --- | --- | --- | --- | --- | --- |\n", acc.device());
    let (mut se, mut sp, mut sg, mut sa) = (0.0, 0.0, 0.0, 0.0);
    let mut k = 0;
    for (_, a) in &wavs {
        let sl = dispatch_slices(a);
        for i in 0..sl.len() {
            let ws = if i > 0 { sl[i - 1].0 } else { sl[i].0 };
            let ws = ws.max(sl[i].1.saturating_sub(WINDOW_MAX_SECS * RATE));
            let w = &a[ws..sl[i].1];
            let cap = max_new_tokens_for(w.len() as f32 / RATE as f32);
            let Ok(out) = acc.decode(w, None, None, Some(cap), Default::default()) else {
                continue;
            };
            let text = out.raw.split("<asr_text>").last().unwrap_or("").to_string();
            let t = Instant::now();
            if !text.trim().is_empty() {
                let _ = acc.align(w, &text);
            }
            let al = t.elapsed().as_secs_f64() * 1000.0;
            let st = out.stats;
            if k > 0 {
                // 首窗含图构建 / 冷启动，不计入均值
                se += st.enc_ms;
                sp += st.prefill_ms - st.enc_ms;
                sg += st.gen_ms;
                sa += al;
            }
            let _ = writeln!(
                md,
                "| {k} | {:.1} | {:.0} | {:.0} | {:.0} | {} | {:.0} |",
                w.len() as f32 / RATE as f32,
                st.enc_ms,
                st.prefill_ms - st.enc_ms,
                st.gen_ms,
                st.n_gen,
                al
            );
            k += 1;
        }
    }
    let n = (k - 1).max(1) as f64;
    let tot = se + sp + sg + sa;
    let _ = writeln!(md, "\n均值（去首窗）：编码 {:.0} / 预填充 {:.0} / 生成 {:.0} / 对齐 {:.0} ms；**生成占 {:.0}%**", se / n, sp / n, sg / n, sa / n, 100.0 * sg / tot);
    let out = root().join("collab/evidence/491");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("poc491-breakdown.md"), &md).unwrap();
    println!("{md}");
}

#[test]
#[ignore = "POC-ASR-SERVER-491：需真模型 + 录音；--ignored 运行"]
fn poc491_concurrency() {
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let models = root().join("models");
    let mut acc = create_qwen3_recognizer(&models).expect("1.7B 须在位");
    acc.attach_aligner(&models);
    let local_prev = Mutex::new(Rec(
        create_local_stream_recognizer(&models).expect("预览模型须在位")
    ));
    let wavs = pick_wavs(3);
    assert_eq!(wavs.len(), 3, "需要 3 段 20~90s 录音");
    let mut md = String::new();
    let _ = writeln!(
        md,
        "# POC-ASR-SERVER-491 并发实测\n\n设备：`{}`\n",
        acc.device()
    );
    for (p, a) in &wavs {
        let _ = writeln!(
            md,
            "- `{}` {:.1}s，{} 个派发片",
            p.file_name().unwrap().to_string_lossy(),
            a.len() as f32 / RATE as f32,
            dispatch_slices(a).len()
        );
    }
    // 预热（首窗编码器 / 图构建不计入）
    let warm = &wavs[0].1[..RATE * 5];
    let _ = decode_accuracy_allow_empty(
        &acc,
        warm,
        None,
        ChineseScript::Simplified,
        Some(16),
        Default::default(),
    );
    let _ = preview_decode(&local_prev.lock().unwrap().0, warm);

    let remote_prev = Mutex::new(Rec(preview_rec(&models, 2)));
    let _ = preview_decode(&remote_prev.lock().unwrap().0, warm);

    // 本机 / 远程录音分配：`POC491_LOCAL`（默认 0）指定哪段当本机，其余两段当远程。
    let li: usize = std::env::var("POC491_LOCAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let ri: Vec<usize> = (0..3).filter(|&i| i != li).collect();
    let _ = writeln!(md, "\n本机 = 第 {li} 段，远程 = 第 {:?} 段", ri);
    let gate = Gate::default();
    // `POC491_MODES`：逗号分隔要跑的场景（默认 M0,M1,M2）。
    let modes = std::env::var("POC491_MODES").unwrap_or_else(|_| "M0,M1,M2".into());
    for (key, mode, remotes, remote_own_prev, prio) in [
        ("M0", "M0 本机单独", 0usize, false, false),
        ("M1", "M1 本机+2路远程（共用预览模型）", 2, false, false),
        (
            "M2",
            "M2 本机+2路远程（远程另开 2 线程预览模型）",
            2,
            true,
            false,
        ),
        (
            "M3",
            "M3 本机+2路远程（共用预览模型 + 本机优先闸门）",
            2,
            false,
            true,
        ),
    ] {
        if !modes.split(',').any(|m| m == key) {
            continue;
        }
        let start = Instant::now() + Duration::from_millis(200);
        let g = prio.then_some(&gate);
        let results: Vec<Stats> = std::thread::scope(|s| {
            let la = &wavs[li].1;
            let (accr, lp) = (&acc, &local_prev);
            let mut hs =
                vec![s.spawn(move || run_session(la, start, accr, lp, g.map(|g| (g, true))))];
            for (r, &idx) in ri.iter().enumerate().take(remotes) {
                let a = &wavs[idx].1;
                let pv = if remote_own_prev {
                    &remote_prev
                } else {
                    &local_prev
                };
                let acc = &acc;
                let st = start + Duration::from_millis(300 + 400 * r as u64);
                hs.push(s.spawn(move || run_session(a, st, acc, pv, g.map(|g| (g, false)))));
            }
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let _ = writeln!(md, "\n## {mode}\n\n| 指标（ms） | 次数 | 中位 | P90 | 最大 |\n| --- | --- | --- | --- | --- |");
        for (i, st) in results.iter().enumerate() {
            let who = if i == 0 {
                "本机".to_string()
            } else {
                format!("远程{i}")
            };
            let _ = writeln!(
                md,
                "{}",
                line(&format!("{who} 派发→定稿延迟"), &st.win_latency)
            );
            let _ = writeln!(md, "{}", line(&format!("{who} 精解耗时"), &st.decode_ms));
            let _ = writeln!(md, "{}", line(&format!("{who} 对齐耗时"), &st.align_ms));
            let _ = writeln!(md, "{}", line(&format!("{who} 预览重解"), &st.preview_ms));
        }
        println!("[poc491] {mode} done");
    }
    let out = root().join("collab/evidence/491");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("poc491-concurrency.md"), &md).unwrap();
    println!("{md}");
}
