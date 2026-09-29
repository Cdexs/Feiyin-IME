#![cfg(test)]
// REPLAY-436 · 433 vs 436 全量录音离线对照（单线程简化回放，只测不改生产）
//
// 目标（主控 2026-09-26 派发）：上一轮 436 只有日志里 5 个接缝、split 硬编码 0.5；本单用
// `diag437_wavs` 同一去重口径的 10 段录音、真实 `interior_split_frac`，逐接缝对照两方案。
//
// 回放几何（简化，但 433/436 **共用同一几何** ⇒ 对比公平）：
//   1. 切片：`diag437_dispatch_slices`（生产「静默 ≥1200ms 派发」口径，复刻自 vad.rs 测试）
//      + `vad::plan_gap_cuts`（含 437 R1 三级切点，commit `4b884a4`）⇒ 片 `[start, end)`。
//   2. 组窗：每窗 = 紧邻前片的**末尾后缀**（生产 `take_context_suffix` → `find_tail_cut`，
//      回溯 `tail_backtrack_secs(3.0)` ≥2s）+ 本片；首片无前文。
//      span 用**全局片下标**：首窗 `(0,1)`，此后 `(i-1, i+1)` ⇒ `ws < prev_end` 恒成立、
//      `shared = prev_end − ws = 1` ⇒ `OrderedReflow` 共享片判定与生产 413/427 完全一致。
//      （生产 `build_sliding_segments_with_spans` 走「VAD raw → plan_sliding_cuts」另一条
//       组片管线；本回放按任务书用「dispatch + plan_gap_cuts」等价方式得到片区间与 span。）
//   3. 每窗：VAD `speech_ranges` → `trim_to_speech(pad=LOCALRT_TRIM_PAD_SECS)` →
//      `decode_accuracy_allow_empty`（1.7B 生产同参、无注入）⇒ 精解；
//      流式原文 = 流式小模型（`local_stream::create_local_stream_recognizer`）对该窗音频跑一遍。
//   4. `split_frac` = 生产接线函数 `crate::window_overlap_split`（内部即
//      `local_stream::interior_split_frac`，重叠区音频 = 本窗开头共享片）。
//   5. 同一份解码结果分别 `push_window_streaming(..., None)`（433）与 `Some(split)`（436）。
//
// 🔴 简化（报告必须写明）：无线程时序 / 无 pending 重派发 / 无尾窗 `emit_tail_window!` /
//    无按片流式拼接基线（`cs.baseline`，改为整窗跑流式）/ 无词库·上下文注入。
//
// 运行：cargo test --bin feiyin-ime replay436 -- --ignored --nocapture

use super::*;
use crate::config::ChineseScript;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

const RATE: usize = 16_000;
/// 冷启动字速（生产 `tail_backtrack_secs` 缺省；回放不维护运行均值 ⇒ 全程同一取值，
/// 两方案一致 ⇒ 不影响公平性）。
const RATE_CPS: f32 = 3.0;
/// 伪参考取接缝两侧各 ~5s。
const PSEUDO_SIDE_SECS: usize = 5;
/// 接缝判胜负的近似平局阈值（本地 CER 差 < 2 个百分点记平）。
const TIE_EPS: f32 = 0.02;
/// 参考文本本身不可用（两方案本地 CER 都 ≥60%）⇒ 待回听。
const REF_FLOOR: f32 = 0.6;

// ========================================================================
// 日志捕获：接缝字段（arb / keep_prev / anchor / interior_why / R）只在
// `[DBG-416] seam` 里，且 `push_inner` 受 `log_enabled!(Debug)` 守卫 ⇒ 装一个只收
// 该前缀的 logger（进程内唯一，`log::set_logger` 幂等失败时打印警告）。
// ========================================================================
static CAPTURE: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct SeamLogger;

impl log::Log for SeamLogger {
    fn enabled(&self, meta: &log::Metadata<'_>) -> bool {
        meta.level() <= log::Level::Debug
    }
    fn log(&self, record: &log::Record<'_>) {
        let m = record.args().to_string();
        if m.contains("[DBG-416] seam:") {
            if let Ok(mut g) = CAPTURE.lock() {
                if g.len() < 100_000 {
                    g.push(m);
                }
            }
        }
    }
    fn flush(&self) {}
}

static LOGGER: SeamLogger = SeamLogger;

fn install_capture() -> bool {
    let ok = log::set_logger(&LOGGER).is_ok();
    log::set_max_level(log::LevelFilter::Debug);
    ok
}

fn take_logs() -> Vec<String> {
    match CAPTURE.lock() {
        Ok(mut g) => std::mem::take(&mut *g),
        Err(_) => Vec::new(),
    }
}

// ========================================================================
// 录音收集 / 读取（`vad::tests::diag437_wavs` 同款口径；该函数在 `vad::tests`
// 私有、子模块不可见 ⇒ 按任务书复制，不改其可见性）
// ========================================================================

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `collab/evidence/gavin-sessions/session-*.wav` + `target/release/debug-audio/session-*.wav`
/// （只读，不删不移），按文件名去重（同名两处各存一份 ⇒ 内容相同）。
fn collect_wavs(root: &Path) -> Vec<PathBuf> {
    let mut by_name: std::collections::BTreeMap<String, PathBuf> =
        std::collections::BTreeMap::new();
    for dir in [
        root.join("collab/evidence/gavin-sessions"),
        root.join("target/release/debug-audio"),
    ] {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let name = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            if p.extension().and_then(|s| s.to_str()) == Some("wav") && name.starts_with("session-")
            {
                by_name.entry(name).or_insert(p);
            }
        }
    }
    by_name.into_values().collect()
}

fn read_wav(path: &Path) -> Option<(Vec<f32>, usize)> {
    let s = path.to_str()?;
    let w = sherpa_onnx::Wave::read(s)?;
    Some((w.samples().to_vec(), w.sample_rate() as usize))
}

/// 生产「静默 ≥1200ms 派发」口径（复刻 `vad::tests::diag437_dispatch_slices`）：
/// 10ms 块 RMS ≤ 0.01 累加、否则清零；派发点即 `plan_gap_cuts` 在生产收到的 `[start, end)`。
fn dispatch_slices(audio: &[f32]) -> Vec<(usize, usize)> {
    const SILENCE_MS: f32 = 1200.0;
    const THRESH: f32 = 0.01;
    const CHUNK: usize = 160; // 10ms @16k
    let mut slices: Vec<(usize, usize)> = Vec::new();
    let mut dispatched_end = 0usize;
    let mut silent_ms = 0.0f32;
    let mut done_for_pause = false;
    let mut pos = 0usize;
    while pos < audio.len() {
        let end = (pos + CHUNK).min(audio.len());
        let c = &audio[pos..end];
        let rms = (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt();
        if rms <= THRESH {
            silent_ms += (end - pos) as f32 / 16.0;
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
    if dispatched_end < audio.len() {
        slices.push((dispatched_end, audio.len()));
    }
    slices
}

// ========================================================================
// 文本度量（CER / 重复 / 本地对齐 CER）
// ========================================================================

fn edit_distance(a: &[char], b: &[char]) -> usize {
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut cur = vec![0usize; n + 1];
    for i in 1..=m {
        cur[0] = i;
        for j in 1..=n {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[n]
}

/// 全文 CER（有效字口径 = 生产对齐字母表：去空白与标点）。
fn cer(pred: &str, reference: &str) -> f32 {
    let a = effective_chars(pred);
    let b = effective_chars(reference);
    if a.is_empty() {
        return if b.is_empty() { 0.0 } else { 1.0 };
    }
    edit_distance(&a, &b) as f32 / a.len().max(b.len()) as f32
}

/// 接缝局部 CER：在参考文本上滑动取**等长窗**，取最小编辑距离 / 区域有效字数。
/// （窗口取 `a.len()` 而非 `a.len()+slack` —— 多留 slack 会把「多出的参考字符」算成必删，
///  每条接缝恒被多罚 slack 个字符，见 REPLAY-436 首批校准。）
fn local_cer(region: &str, reference: &str) -> f32 {
    let a = effective_chars(region);
    let b = effective_chars(reference);
    if a.is_empty() || b.is_empty() {
        return REF_FLOOR + 1.0;
    }
    let mut best = usize::MAX;
    for o in 0..b.len() {
        let end = b.len().min(o + a.len());
        let d = edit_distance(&a, &b[o..end]);
        if d < best {
            best = d;
            if best == 0 {
                break;
            }
        }
    }
    best as f32 / a.len() as f32
}

/// ≥8 连续有效字重复（`有效字` 口径，窗口滑动查重）。
fn has_repeat8(text: &str) -> bool {
    const W: usize = 8;
    let e = effective_chars(text);
    let n = e.len();
    if n < 2 * W {
        return false;
    }
    for i in 0..=(n - 2 * W) {
        for j in (i + W)..=(n - W) {
            if e[i..i + W] == e[j..j + W] {
                return true;
            }
        }
    }
    false
}

fn fmt_t(secs: f32) -> String {
    format!("{}:{:04.1}", (secs / 60.0) as u32, secs % 60.0)
}

// ========================================================================
// 接缝日志解析
// ========================================================================

fn val_space(s: &str, key: &str) -> String {
    let Some(i) = s.find(key) else {
        return String::new();
    };
    let rest = &s[i + key.len()..];
    rest.split(' ').next().unwrap_or("").to_string()
}

fn val_quoted(s: &str, key: &str) -> String {
    let Some(i) = s.find(key) else {
        return String::new();
    };
    let rest = &s[i + key.len()..];
    rest.split('"').next().unwrap_or("").to_string()
}

#[derive(Default, Clone)]
struct SeamFields {
    layer: String,
    keep_prev: String,
    arb: String,
    r: String,
    a_kept: String,
    b_from: String,
    why: String,
    anchor: String,
}

fn parse_seam(line: &str) -> SeamFields {
    SeamFields {
        layer: val_space(line, "layer="),
        keep_prev: val_space(line, "keep_prev="),
        arb: val_space(line, " arb="),
        r: val_quoted(line, "R=\""),
        a_kept: val_space(line, " a_kept="),
        b_from: val_space(line, " b_from="),
        why: val_space(line, "interior_why="),
        anchor: val_quoted(line, "anchor=\""),
    }
}

// ========================================================================
// 行结构
// ========================================================================

struct SeamRow {
    rec: String,
    idx: usize,
    t: f32,
    prev_overlap: String,
    region33: String,
    region36: String,
    split: Option<f32>,
    f33: SeamFields,
    f36: SeamFields,
    /// R：前一窗流式按共享样本占比取的末尾（回放自算，非日志 `R` 字段）。
    r: String,
    verdict: String,
    judge: String,
    cer33: Option<f32>,
    cer36: Option<f32>,
    dup33: bool,
    dup36: bool,
    ref_src: String,
    new_eff: usize,
}

struct RecRow {
    name: String,
    secs: f32,
    slices: usize,
    pieces: usize,
    windows: usize,
    seams: usize,
    empty: usize,
    has_ref: bool,
    text33: String,
    text36: String,
    cer33: Option<f32>,
    cer36: Option<f32>,
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn write_doc(path: &Path, body: &str) {
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    if let Err(e) = std::fs::write(path, body) {
        println!("[436] 写 {} 失败: {}", path.display(), e);
    }
}

// ========================================================================
// 主用例
// ========================================================================

#[test]
#[ignore = "REPLAY-436 全量回放：需 1.7B + 流式模型 + 10 段录音；cargo test --bin feiyin-ime replay436 -- --ignored --nocapture"]
fn replay436_full_corpus_433_vs_436() {
    let root = manifest_dir();
    let models = root.join("models");
    let ev = root.join("collab/evidence/436/replay");

    let installed = install_capture();
    println!("[436] logger capture installed={installed}");

    println!("[436] 建 1.7B accuracy recognizer …");
    let acc_rec = create_qwen3_recognizer(&models).expect("1.7B 模型须在位");
    println!("[436] 建流式 paraformer recognizer …");
    let st_rec = crate::transcription::local_stream::create_local_stream_recognizer(&models)
        .expect("流式模型须在位");
    let vad = VadSegmenter::try_new_for_local_trim(&models);
    if vad.is_none() {
        println!("[436] ⚠ VAD 不可用 ⇒ 每窗不剪静音（整窗送解）");
    }
    let pad = (vad::LOCALRT_TRIM_PAD_SECS * RATE as f32) as usize;

    // 生产同参解码闭包（diag425 `prod_decode` 同口径：390 token cap、无注入）。
    let decode_acc = |samples: &[f32]| -> String {
        if samples.is_empty() {
            return String::new();
        }
        let cap = max_new_tokens_for(samples.len() as f32 / RATE as f32);
        match decode_accuracy_allow_empty(
            &acc_rec,
            samples,
            None,
            ChineseScript::Simplified,
            Some(cap),
            Default::default(),
        ) {
            Ok((t, _)) => t,
            Err(e) => {
                println!("[436] accuracy 解码失败: {e}");
                String::new()
            }
        }
    };

    // 流式原文：生产同款建流（`create_local_stream_recognizer`），喂整窗 → flush 取终局文本。
    let run_stream = |audio: &[f32]| -> String {
        if audio.is_empty() {
            return String::new();
        }
        let stream = st_rec.create_stream();
        let chunk = 1600usize; // 100ms @16k（与生产音频回调量级一致）
        let mut pos = 0usize;
        while pos < audio.len() {
            let end = (pos + chunk).min(audio.len());
            stream.accept_waveform(RATE as i32, &audio[pos..end]);
            while st_rec.is_ready(&stream) {
                st_rec.decode(&stream);
            }
            pos = end;
        }
        stream.input_finished();
        while st_rec.is_ready(&stream) {
            st_rec.decode(&stream);
        }
        st_rec
            .get_result(&stream)
            .map(|r| r.text.clone())
            .unwrap_or_default()
    };

    let mut wavs = collect_wavs(&root);
    println!(
        "[436] 录音 {} 段（gavin-sessions + debug-audio，按文件名去重）",
        wavs.len()
    );
    assert!(wavs.len() >= 5, "去重后录音数异常: {}", wavs.len());
    // 首批只跑单段（主控 20 分钟内看第一批接缝）：REPLAY436_ONLY=175022
    if let Ok(only) = std::env::var("REPLAY436_ONLY") {
        wavs.retain(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.contains(&only))
                .unwrap_or(false)
        });
        println!("[436] REPLAY436_ONLY={only} ⇒ {} 段", wavs.len());
        assert!(!wavs.is_empty(), "REPLAY436_ONLY={only} 未匹配到录音");
    }

    let mut geo = String::from(
        "# REPLAY-436 · 01 几何（片 / 窗 / 接缝）\n\n> 单线程简化回放：dispatch 切片 + `plan_gap_cuts`(437 R1) + 后缀组窗。\n> span 用全局片下标，`shared = prev_end − ws = 1` ⇒ 与生产 413/427 共享片判定一致。\n\n",
    );
    let mut dec = String::from(
        "# REPLAY-436 · 02 逐窗解码（精解 / 流式 / split_frac / 接缝输出）\n\n> 精解 = 1.7B `decode_accuracy_allow_empty`（无注入、390 cap）；流式 = 流式 paraformer 整窗。\n\n",
    );
    let mut seams: Vec<SeamRow> = Vec::new();
    let mut recs: Vec<RecRow> = Vec::new();
    let corpus_start = Instant::now();

    for (wi, wav) in wavs.iter().enumerate() {
        let name = wav
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let t0 = Instant::now();
        let Some((audio, rate)) = read_wav(wav) else {
            println!("[436] ({wi}) {name}: 读取失败，跳过");
            geo.push_str(&format!("- **{name}**：读取失败，跳过\n"));
            continue;
        };
        if rate as usize != RATE {
            println!("[436] ({wi}) {name}: 采样率 {rate} ≠ 16000，跳过");
            geo.push_str(&format!("- **{name}**：采样率 {rate} ≠ 16000，跳过\n"));
            continue;
        }

        // ① 切片：dispatch 口径 + plan_gap_cuts（437 R1）
        let _ = take_logs(); // 清掉切片期的 [DBG-437] 日志
        let slices = dispatch_slices(&audio);
        let mut pieces: Vec<(usize, usize)> = Vec::new();
        for &(s, e) in &slices {
            pieces.extend(vad::plan_gap_cuts(&audio, s, e));
        }

        // 参考文本
        let ref_path = wav.with_file_name(format!("{name}.ref.txt"));
        let ref_txt = std::fs::read_to_string(&ref_path).ok();

        geo.push_str(&format!(
            "\n## {name}\n\n- 时长 {:.1}s | 派发片 {} | 切片后片 {} | 参考 {}\n- 片边界(s)：{}\n\n| win | span | 片长(s) | 后缀(s) | split_frac |\n|---:|---|---:|---:|---:|\n",
            audio.len() as f32 / RATE as f32,
            slices.len(),
            pieces.len(),
            if ref_txt.is_some() { "有" } else { "无" },
            pieces
                .iter()
                .map(|&(s, e)| format!("{:.1}~{:.1}", s as f32 / RATE as f32, e as f32 / RATE as f32))
                .collect::<Vec<_>>()
                .join(" ")
        ));
        dec.push_str(&format!(
            "\n## {name}\n\n| win | span | 窗长(s) | 精解 | 流式 | split | 接缝 433 | 接缝 436 |\n|---:|---|---:|---|---|---:|---|---|\n"
        ));

        let mut r433 = OrderedReflow::new();
        let mut r436 = OrderedReflow::new();
        let mut prev_acc = String::new();
        let mut prev_stream_txt = String::new();
        let mut prev_total = 0usize;
        let mut empty = 0usize;
        let mut seams_here = 0usize;
        let mut text33_last = String::new();
        let mut text36_last = String::new();

        for (i, &(ps, pe)) in pieces.iter().enumerate() {
            // ② 组窗：前片末尾后缀（生产 take_context_suffix）+ 本片
            let mut waudio: Vec<f32> = Vec::new();
            let mut wsamples: Vec<usize> = Vec::new();
            if i >= 1 {
                let prev = &audio[pieces[i - 1].0..pieces[i - 1].1];
                // `prev_stream` / `must_stream` 只喂 `ContextSuffix.baseline`（仅生产日志用），
                // 回放不用该字段 ⇒ 传空；`cut`/`suffix_samples` 只依赖音频与回溯时长。
                let cs = crate::take_context_suffix(prev, "", "", RATE_CPS);
                waudio.extend_from_slice(&prev[cs.cut.min(prev.len())..]);
                wsamples.push(cs.suffix_samples);
            }
            waudio.extend_from_slice(&audio[ps..pe]);
            wsamples.push(pe - ps);

            let span_start = i.saturating_sub(1);
            let span_end = i + 1;
            let shared_samples = wsamples.first().copied().unwrap_or(0);

            // ④ split_frac：生产接线函数（重叠区音频 = 本窗开头共享片）
            let split = crate::window_overlap_split(
                span_start,
                if i >= 1 { Some(i) } else { None },
                &waudio,
                &wsamples,
            );

            // ③ 精解（VAD 剪静音 → 1.7B）+ 流式（整窗）
            let trimmed = match &vad {
                Some(v) => trim_to_speech(&waudio, &v.speech_ranges(&waudio), pad),
                None => waudio.clone(),
            };
            let used: &[f32] = if trimmed.is_empty() {
                &waudio
            } else {
                &trimmed
            };
            let acc_text = decode_acc(used);
            if acc_text.trim().is_empty() {
                empty += 1;
            }
            let stream_text = run_stream(&waudio);

            geo.push_str(&format!(
                "| {i} | [{span_start}, {span_end}) | {:.2} | {:.2} | {} |\n",
                (pe - ps) as f32 / RATE as f32,
                shared_samples as f32 / RATE as f32,
                split
                    .map(|f| format!("{f:.3}"))
                    .unwrap_or_else(|| "na".into())
            ));

            // 前窗重叠文字（按共享样本占比推得的前窗尾部；生产不落日志 ⇒ 明确标注推导口径）
            let prev_overlap = if i >= 1 && !prev_acc.is_empty() && prev_total > 0 {
                let peff = effective_chars(&prev_acc);
                let ratio = (shared_samples as f32 / prev_total as f32).clamp(0.0, 1.0);
                let n = ((peff.len() as f32) * ratio).round() as usize;
                peff[peff.len().saturating_sub(n.min(peff.len()))..]
                    .iter()
                    .collect::<String>()
            } else {
                String::new()
            };

            // ⑤ 同一份解码 → 433 / 436 各跑一遍
            // R（433 裁判基准）= 前一窗流式按共享样本占比取的末尾。日志里的 `R` 字段在
            // 「436 锚点命中 / concat 兜底」路径不回填（`r_dbg` 未赋值）⇒ 回放自算，保证 R 列恒有值。
            let r_real = if i >= 1 {
                streaming_overlap_region(&prev_stream_txt, prev_total, shared_samples)
            } else {
                String::new()
            };
            let cb33 = r433.committed.len();
            let cb36 = r436.committed.len();
            let _ = take_logs();
            let out33 = r433.push_window_streaming(
                i,
                span_start,
                span_end,
                wsamples.clone(),
                acc_text.clone(),
                stream_text.clone(),
                None,
            );
            let l33 = take_logs();
            let out36 = r436.push_window_streaming(
                i,
                span_start,
                span_end,
                wsamples.clone(),
                acc_text.clone(),
                stream_text.clone(),
                split,
            );
            let l36 = take_logs();

            let full33 = out33.last().cloned().unwrap_or_default();
            let full36 = out36.last().cloned().unwrap_or_default();
            // 空解码窗 ⇒ `out` 为空 ⇒ **不得**用空串覆盖上一窗全文（否则末窗为空即全文清零）。
            if !full33.is_empty() {
                text33_last = full33.clone();
            }
            if !full36.is_empty() {
                text36_last = full36.clone();
            }

            if i >= 1 && !full33.is_empty() && !full36.is_empty() {
                let region33 = full33.get(cb33..).unwrap_or("").to_string();
                let region36 = full36.get(cb36..).unwrap_or("").to_string();
                let s33 = l33
                    .iter()
                    .find(|l| l.contains("[DBG-416] seam:"))
                    .map(|l| parse_seam(l));
                let s36 = l36
                    .iter()
                    .find(|l| l.contains("[DBG-416] seam:"))
                    .map(|l| parse_seam(l));
                let (f33, f36) = match (s33, s36) {
                    (Some(a), Some(b)) => (a, b),
                    _ => (SeamFields::default(), SeamFields::default()),
                };
                seams_here += 1;
                seams.push(SeamRow {
                    rec: name.clone(),
                    idx: i,
                    t: ps as f32 / RATE as f32,
                    prev_overlap,
                    region33,
                    region36,
                    split,
                    f33,
                    f36,
                    r: r_real,
                    verdict: String::new(),
                    judge: String::new(),
                    cer33: None,
                    cer36: None,
                    dup33: false,
                    dup36: false,
                    ref_src: String::new(),
                    new_eff: effective_chars(&acc_text).len(),
                });
            }

            dec.push_str(&format!(
                "| {i} | [{span_start}, {span_end}) | {:.2} | {} | {} | {} | {} | {} |\n",
                waudio.len() as f32 / RATE as f32,
                md_escape(&acc_text),
                md_escape(&stream_text),
                split
                    .map(|f| format!("{f:.3}"))
                    .unwrap_or_else(|| "na".into()),
                md_escape(
                    &text33_last
                        .chars()
                        .rev()
                        .take(60)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>()
                ),
                md_escape(
                    &text36_last
                        .chars()
                        .rev()
                        .take(60)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>()
                )
            ));

            prev_acc = acc_text;
            prev_stream_txt = stream_text;
            prev_total = wsamples.iter().sum();
        }

        // 收尾取定稿全文：`finish()` = `(committed, last_window_text)`（空解码窗不影响已定稿部分）
        let (fc33, lw33) = r433.finish();
        let (fc36, lw36) = r436.finish();
        let (c33, c36) = (format!("{fc33}{lw33}"), format!("{fc36}{lw36}"));
        let (ref_cer33, ref_cer36) = match &ref_txt {
            Some(r) => (Some(cer(&c33, r)), Some(cer(&c36, r))),
            None => (None, None),
        };
        geo.push_str(&format!(
            "\n接缝 {} | 空解码窗 {} | 全文 {:?} / {:?}\n",
            seams_here,
            empty,
            c33.chars().count(),
            c36.chars().count()
        ));
        recs.push(RecRow {
            name,
            secs: audio.len() as f32 / RATE as f32,
            slices: slices.len(),
            pieces: pieces.len(),
            windows: pieces.len(),
            seams: seams_here,
            empty,
            has_ref: ref_txt.is_some(),
            text33: c33,
            text36: c36,
            cer33: ref_cer33,
            cer36: ref_cer36,
        });
        // 中间结果随时落盘（防上下文压缩丢数据）
        write_doc(&ev.join("01-geometry.md"), &geo);
        write_doc(&ev.join("02-decodes.md"), &dec);
        println!(
            "[436] ({wi}) {}: {:.1}s | 片 {} | 接缝 {} | 空窗 {} | 用时 {:.1}s",
            recs.last().map(|r| r.name.clone()).unwrap_or_default(),
            recs.last().map(|r| r.secs).unwrap_or(0.0),
            pieces.len(),
            seams_here,
            empty,
            t0.elapsed().as_secs_f32()
        );
    }

    // ---- 判对错：逐接缝 ----
    for s in seams.iter_mut() {
        let rec = recs.iter().find(|r| r.name == s.rec);
        let ref_txt = rec.filter(|r| r.has_ref).and_then(|_| read_ref_for(&s.rec));
        s.dup33 = has_repeat8(&s.region33);
        s.dup36 = has_repeat8(&s.region36);

        if s.region33 == s.region36 {
            s.verdict = "平".into();
            s.judge = "两方案输出相同".into();
            s.cer33 = Some(0.0);
            s.cer36 = Some(0.0);
            s.ref_src = if ref_txt.is_some() { "ref" } else { "-" }.into();
            continue;
        }

        let (reference, src) = match ref_txt {
            Some(r) if !effective_chars(&r).is_empty() => (Some(r), "ref"),
            _ => {
                // 无 ref ⇒ 以接缝为中心 ±5s 重解一窗作伪参考（1.7B）
                match pseudo_ref_for(&s.rec, s.t, &decode_acc) {
                    Some(p) if !effective_chars(&p).is_empty() => (Some(p), "pseudo"),
                    _ => (None, "none"),
                }
            }
        };

        match reference {
            Some(r) => {
                let c33 = local_cer(&s.region33, &r);
                let c36 = local_cer(&s.region36, &r);
                s.cer33 = Some(c33);
                s.cer36 = Some(c36);
                s.ref_src = src.into();
                println!(
                    "[436] seam {}#{} c33={c33:.3} c36={c36:.3} reg_eff={} ref_eff={} src={src}",
                    s.rec.replace("session-20260925-", ""),
                    s.idx,
                    effective_chars(&s.region33).len(),
                    effective_chars(&r).len()
                );
                if c33 >= REF_FLOOR && c36 >= REF_FLOOR {
                    s.verdict = "待回听".into();
                    s.judge = format!(
                        "参考不可用（两方案本地CER均 ≥{}%）",
                        (REF_FLOOR * 100.0) as u32
                    );
                } else if (c33 - c36).abs() < TIE_EPS {
                    s.verdict = "平".into();
                    s.judge = format!("本地CER 差 <{}%（近似平）", (TIE_EPS * 100.0) as u32);
                } else if c36 < c33 {
                    s.verdict = "436胜".into();
                    s.judge = format!("本地CER 436 {c36:.3} < 433 {c33:.3}（{src}）");
                } else {
                    s.verdict = "433胜".into();
                    s.judge = format!("本地CER 433 {c33:.3} < 436 {c36:.3}（{src}）");
                }
            }
            None => {
                s.verdict = "待回听".into();
                s.judge = "无 ref 且伪参考解码为空".into();
                s.ref_src = "none".into();
            }
        }
    }

    // ---- 表格 ----
    let mut table = String::from(
        "# REPLAY-436 · 433 vs 436 逐接缝对照表\n\n\
口径：切片 = dispatch(静默≥1200ms) + `plan_gap_cuts`(437 R1)；组窗 = 前片末尾后缀(`take_context_suffix`, 3.0 字/s ⇒ 回溯 4s) + 本片；\n\
精解 = 1.7B `decode_accuracy_allow_empty`(无注入)；流式 R = 流式 paraformer 整窗；`split_frac` = `window_overlap_split`(真实重叠区音频)。\n\
几何简化：无线程时序 / 无 pending 重派发 / 无尾窗；**433 与 436 共用同一几何与同一份解码结果 ⇒ 对比公平**。\n\n\
`前窗重叠文字` = 前窗精解按**共享样本占比**取的尾部（生产不落日志，标注为推导值）。\n\
`判据`：两方案输出相同 ⇒ 平；否则有 ref 用全文 ref 做局部 CER 对齐，无 ref 用接缝 ±5s 伪参考；差 <2pp 记平，参考两方案均 ≥60% ⇒ 待回听。\n\n"
    );
    table.push_str(
        "| # | 录音 | 时间点 | 前窗重叠文字 | 433 接缝输出 | 436 接缝输出 | split_frac | 锚点 | 436 兜底 | 433 arb/keep | 433本地CER | 436本地CER | 判据 | 判定 | 重复≥8 |\n|---:|---|---|---|---|---|---:|---|---|---|---:|---:|---|---|---|\n",
    );
    for (n, s) in seams.iter().enumerate() {
        let dup = match (s.dup33, s.dup36) {
            (true, true) => "两方案",
            (true, false) => "仅433",
            (false, true) => "仅436",
            _ => "-",
        };
        let why = if s.f36.why == "hit" {
            "hit".to_string()
        } else {
            format!(
                "兜底:{}",
                if s.f36.why.is_empty() {
                    "na"
                } else {
                    &s.f36.why
                }
            )
        };
        table.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | **{}** | {} |\n",
            n + 1,
            s.rec.replace("session-20260925-", ""),
            fmt_t(s.t),
            md_escape(&clip(&s.prev_overlap, 40)),
            md_escape(&clip(&s.region33, 60)),
            md_escape(&clip(&s.region36, 60)),
            s.split
                .map(|f| format!("{f:.3}"))
                .unwrap_or_else(|| "na".into()),
            if s.f36.anchor.is_empty() {
                "-".into()
            } else {
                md_escape(&s.f36.anchor)
            },
            why,
            if s.f33.arb.is_empty() && s.f33.keep_prev.is_empty() {
                "-".into()
            } else {
                format!("{}/{}", s.f33.arb, s.f33.keep_prev)
            },
            s.cer33
                .map(|c| format!("{c:.3}"))
                .unwrap_or_else(|| "-".into()),
            s.cer36
                .map(|c| format!("{c:.3}"))
                .unwrap_or_else(|| "-".into()),
            md_escape(&clip(&s.judge, 70)),
            s.verdict,
            dup
        ));
    }
    write_doc(&ev.join("table.md"), &table);

    // ---- 03：接缝原始日志字段（layer / arb / keep_prev / R / a_kept / b_from）----
    let mut slog = String::from(
        "# REPLAY-436 · 03 接缝原始日志字段\n\n> 取自 `[DBG-416] seam`（`push_inner` 的 Debug 门），433/436 各一行。\n> `R` = 回放自算的流式重叠区文本（前一窗流式按共享样本占比取末尾）；日志内 `R` 字段在\n> 「436 锚点命中 / concat 兜底」路径不回填（`r_dbg` 未赋值）⇒ 故另列 `日志R` 佐证该现象。\n\n| # | 录音 | win | 433 layer | 433 arb | 433 keep | 436 layer | 436 arb | 436 keep | 436 why | anchor | a_kept | b_from | R | 日志R(436) |\n|---:|---|---:|---|---|---|---|---|---|---|---|---:|---:|---|---|\n",
    );
    for (n, s) in seams.iter().enumerate() {
        slog.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            n + 1,
            s.rec.replace("session-20260925-", ""),
            s.idx,
            s.f33.layer,
            s.f33.arb,
            s.f33.keep_prev,
            s.f36.layer,
            s.f36.arb,
            s.f36.keep_prev,
            s.f36.why,
            if s.f36.anchor.is_empty() {
                "-".into()
            } else {
                md_escape(&s.f36.anchor)
            },
            s.f36.a_kept,
            s.f36.b_from,
            md_escape(&clip(&s.r, 50)),
            md_escape(&clip(&s.f36.r, 30))
        ));
    }
    write_doc(&ev.join("03-seams-log.md"), &slog);

    // ---- 04：两方案全文（逐段）----
    let mut ft = String::from("# REPLAY-436 · 04 两方案全文（逐段，供复核 CER）\n\n");
    for r in &recs {
        ft.push_str(&format!(
            "\n## {}（{:.1}s，派发片 {}，切片后片 {}，空解码窗 {}，ref {}）\n\n### 433 全文（{} 字，CER {}）\n\n{}\n\n### 436 全文（{} 字，CER {}）\n\n{}\n\n",
            r.name,
            r.secs,
            r.slices,
            r.pieces,
            r.empty,
            if r.has_ref { "有" } else { "无" },
            r.text33.chars().count(),
            r.cer33.map(|c| format!("{c:.4}")).unwrap_or_else(|| "-".into()),
            r.text33,
            r.text36.chars().count(),
            r.cer36.map(|c| format!("{c:.4}")).unwrap_or_else(|| "-".into()),
            r.text36
        ));
    }
    write_doc(&ev.join("04-fulltexts.md"), &ft);

    // ---- 汇总 ----
    let n = seams.len();
    let n_tie = seams.iter().filter(|s| s.verdict == "平").count();
    let n_33 = seams.iter().filter(|s| s.verdict == "433胜").count();
    let n_36 = seams.iter().filter(|s| s.verdict == "436胜").count();
    let n_wait = seams.iter().filter(|s| s.verdict == "待回听").count();
    let dup33 = seams.iter().filter(|s| s.dup33).count();
    let dup36 = seams.iter().filter(|s| s.dup36).count();
    let mut why_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for s in &seams {
        let k = if s.f36.why.is_empty() {
            "na".into()
        } else {
            s.f36.why.clone()
        };
        *why_counts.entry(k).or_default() += 1;
    }
    let n_fallback = why_counts
        .iter()
        .filter(|(k, _)| k.as_str() != "hit")
        .map(|(_, v)| *v)
        .sum::<usize>();
    let n_hit = why_counts.get("hit").copied().unwrap_or(0);

    // 分歧归类：433 的 keep_prev / arb vs 436 的 a_kept/b_from（前窗 vs 后窗主导）
    let mut trust = std::collections::BTreeMap::<String, usize>::new();
    for s in &seams {
        if s.region33 == s.region36 {
            continue;
        }
        // 433：keep_prev=1 ⇒ 保留前窗；=0 ⇒ 取后窗
        let t33 = match s.f33.keep_prev.as_str() {
            "1" => "433 信前窗",
            "0" => "433 信后窗",
            _ => "433 未定",
        };
        *trust.entry(t33.into()).or_default() += 1;
        // 436：锚点把重叠区切成「前窗取 a_kept 字 + 后窗取 (new_eff − b_from) 字」
        let a36: usize = s.f36.a_kept.parse().unwrap_or(0);
        let b36: usize = s.f36.b_from.parse().unwrap_or(0);
        let new_taken = s.new_eff.saturating_sub(b36);
        let share = a36 as f32 / (a36 + new_taken).max(1) as f32;
        let t36 = if new_taken == 0 {
            "436 信前窗(整段)"
        } else if share >= 0.5 {
            "436 前窗主导"
        } else {
            "436 信后窗为主"
        };
        *trust.entry(t36.into()).or_default() += 1;
    }

    let mut summary = format!(
        "# REPLAY-436 · 汇总\n\n## 1. 接缝判定\n\n| 指标 | 值 |\n|---|---:|\n| 录音数 | {} |\n| 窗数 | {} |\n| 接缝总数 | {n} |\n| 平 | {n_tie} |\n| 433 胜 | {n_33} |\n| 436 胜 | {n_36} |\n| 待回听 | {n_wait} |\n\n",
        recs.len(),
        recs.iter().map(|r| r.windows).sum::<usize>()
    );
    summary.push_str(&format!(
        "## 2. 重复与兜底\n\n| 指标 | 值 |\n|---|---:|\n| ≥8 字重复（433） | {dup33} |\n| ≥8 字重复（436） | {dup36} |\n| 436 锚点命中 | {n_hit} |\n| 436 兜底 | {n_fallback} |\n\n436 兜底原因分布：{}\n\n",
        why_counts
            .iter()
            .map(|(k, v)| format!("`{k}` × {v}"))
            .collect::<Vec<_>>()
            .join("，")
    ));
    summary.push_str(
        "## 3. 逐段全文 CER（有 ref 者对 `.ref.txt`；无 ref 记 -）\n\n| 录音 | 时长 | 派发片 | 切片后片 | 窗 | 接缝 | 空窗 | 433 CER | 436 CER |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|\n",
    );
    for r in &recs {
        summary.push_str(&format!(
            "| {} | {:.1}s | {} | {} | {} | {} | {} | {} | {} |\n",
            r.name,
            r.secs,
            r.slices,
            r.pieces,
            r.windows,
            r.seams,
            r.empty,
            r.cer33
                .map(|c| format!("{c:.4}"))
                .unwrap_or_else(|| "-".into()),
            r.cer36
                .map(|c| format!("{c:.4}"))
                .unwrap_or_else(|| "-".into())
        ));
    }
    summary.push_str("\n## 4. 分歧归类（仅两方案输出不同的接缝）\n\n");
    for (k, v) in &trust {
        summary.push_str(&format!("- {k}: {v}\n"));
    }
    summary.push_str(&format!(
        "\n总用时 {:.1}s\n",
        corpus_start.elapsed().as_secs_f32()
    ));
    write_doc(&ev.join("summary.md"), &summary);

    println!("[436] ============ 汇总 ============");
    println!(
        "[436] 录音 {} | 窗 {} | 接缝 {} | 平 {} | 433胜 {} | 436胜 {} | 待回听 {}",
        recs.len(),
        recs.iter().map(|r| r.windows).sum::<usize>(),
        n,
        n_tie,
        n_33,
        n_36,
        n_wait
    );
    println!(
        "[436] 重复≥8：433={dup33} 436={dup36} | 436 命中={n_hit} 兜底={n_fallback} {why_counts:?}"
    );
    for (k, v) in &trust {
        println!("[436] 分歧 {k}: {v}");
    }
    for r in &recs {
        println!(
            "[436] CER {} 433={} 436={}",
            r.name,
            r.cer33
                .map(|c| format!("{c:.4}"))
                .unwrap_or_else(|| "-".into()),
            r.cer36
                .map(|c| format!("{c:.4}"))
                .unwrap_or_else(|| "-".into())
        );
    }
    println!(
        "[436] 产物：{}/table.md  01-geometry.md  02-decodes.md  summary.md",
        ev.display()
    );

    log::set_max_level(log::LevelFilter::Off);

    assert!(!seams.is_empty(), "零接缝：几何或日志捕获异常");
    assert!(
        seams.iter().any(|s| !s.f36.why.is_empty()),
        "436 接缝日志未捕获（log_enabled! 未生效？）"
    );
}

// ========================================================================
// 参考 / 伪参考
// ========================================================================

fn read_ref_for(stem: &str) -> Option<String> {
    let root = manifest_dir();
    let p = root
        .join("collab/evidence/gavin-sessions")
        .join(format!("{stem}.ref.txt"));
    std::fs::read_to_string(p).ok()
}

/// 无 ref 段：以接缝为中心（前一片末 / 本片首交界）两侧各 ~5s 重解一窗作伪参考。
fn pseudo_ref_for(stem: &str, t_secs: f32, decode: &dyn Fn(&[f32]) -> String) -> Option<String> {
    let root = manifest_dir();
    let wav = root
        .join("collab/evidence/gavin-sessions")
        .join(format!("{stem}.wav"));
    let wav = if wav.exists() {
        wav
    } else {
        root.join("target/release/debug-audio")
            .join(format!("{stem}.wav"))
    };
    let (audio, rate) = read_wav(&wav)?;
    if rate as usize != RATE {
        return None;
    }
    let center = (t_secs * RATE as f32) as usize;
    let side = PSEUDO_SIDE_SECS * RATE;
    let s = center.saturating_sub(side);
    let e = (center + side).min(audio.len());
    if e <= s {
        return None;
    }
    Some(decode(&audio[s..e]))
}

fn clip(s: &str, n: usize) -> String {
    let e = effective_chars(s);
    if e.len() <= n {
        return s.to_string();
    }
    let head: String = e[..n / 2].iter().collect();
    let tail: String = e[e.len() - n + n / 2..].iter().collect();
    format!("{head}…{tail}")
}

// ========================================================================
// ACC-ENGINE-LLAMACPP-452 · 多变体回放 A/B（只测不改生产）
//
// Gavin 2026-09-27：「回灌刷新你可以直接做」「注入之前的文本让它续写，还有注入词库……直接接入吧」
// 「换新的模型和调用框架一定不能影响现在的管线功能」。
// 几何与 REPLAY-436 相同（dispatch 切片 + plan_gap_cuts + 前片后缀组窗 + 436 接缝），每窗走
// **生产** `transcribe_acc_ctx`（剪静音 / 处置阶梯 / 426 重解全在内），只切换注入素材：
//   base  = 无注入（= 现生产输入）            draft = + 本窗流式文字作草稿
//   terms = + 用户真实词库（target/release/wordbook）  full = draft + terms
//   （上文注入变体已移除：带回显护栏时 ctx 4.70% / draft+terms+ctx 4.80%，均不如 terms 4.63% ⇒ 未采用）
// 声纹经测试开关关闭（各变体条件相同，防注册顺序污染）。
// 输出：每变体 5 段有参考录音的全文 CER、全部录音的空窗 / ≥8 字重复 / 解码耗时。
// 运行：cargo test --bin feiyin-ime replay452 -- --ignored --nocapture
// ========================================================================

fn load_real_wordbook_terms() -> Option<String> {
    // Gavin 实际运行目录（debug.log / 声纹均在此）；Publish 那份为 0 条。
    let p = manifest_dir().join("target/release/wordbook.sqlite");
    let conn =
        rusqlite::Connection::open_with_flags(&p, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .ok()?;
    let mut st = conn
        .prepare("SELECT word FROM wordbook ORDER BY id DESC")
        .ok()?;
    let words: Vec<String> = st
        .query_map([], |r| r.get::<_, String>(0))
        .ok()?
        .filter_map(|w| w.ok())
        .collect();
    let s = build_hotwords_string(&words);
    (!s.is_empty()).then_some(s)
}

#[test]
#[ignore = "ACC-452 多变体回放：需 GGUF + 流式模型 + 录音；cargo test --bin feiyin-ime replay452 -- --ignored --nocapture"]
fn replay452_variants() {
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let root = manifest_dir();
    let models = root.join("models");
    let acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
    let st_rec = crate::transcription::local_stream::create_local_stream_recognizer(&models)
        .expect("流式模型须在位");
    let terms = load_real_wordbook_terms();
    println!(
        "[452] device={} wordbook_terms={} chars",
        acc.device(),
        terms.as_deref().map_or(0, |t| t.chars().count())
    );
    let run_stream = |audio: &[f32]| -> String {
        let stream = st_rec.create_stream();
        for c in audio.chunks(1600) {
            stream.accept_waveform(RATE as i32, c);
            while st_rec.is_ready(&stream) {
                st_rec.decode(&stream);
            }
        }
        stream.input_finished();
        while st_rec.is_ready(&stream) {
            st_rec.decode(&stream);
        }
        st_rec
            .get_result(&stream)
            .map(|r| r.text.clone())
            .unwrap_or_default()
    };
    const VARIANTS: [&str; 4] = ["base", "draft", "terms", "full"];
    struct Agg {
        cer_sum: f32,
        cer_n: usize,
        empty: usize,
        rep8: usize,
        ms: f64,
        windows: usize,
    }
    let mut agg: Vec<Agg> = VARIANTS
        .iter()
        .map(|_| Agg {
            cer_sum: 0.0,
            cer_n: 0,
            empty: 0,
            rep8: 0,
            ms: 0.0,
            windows: 0,
        })
        .collect();
    let mut report = String::from("# ACC-452 多变体回放\n");
    for wav in collect_wavs(&root) {
        let name = wav
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        let Some((audio, rate)) = read_wav(&wav) else {
            continue;
        };
        if rate as usize != RATE {
            continue;
        }
        let mut pieces: Vec<(usize, usize)> = Vec::new();
        for (s, e) in dispatch_slices(&audio) {
            pieces.extend(vad::plan_gap_cuts(&audio, s, e));
        }
        let reference = read_ref_for(&name);
        report.push_str(&format!(
            "\n## {name}（{} 窗，参考 {}）\n",
            pieces.len(),
            reference.is_some()
        ));
        for (vi, v) in VARIANTS.iter().enumerate() {
            let mut reflow = OrderedReflow::new();
            let mut full_text = String::new();
            for (i, &(ps, pe)) in pieces.iter().enumerate() {
                let mut waudio: Vec<f32> = Vec::new();
                let mut wsamples: Vec<usize> = Vec::new();
                if i >= 1 {
                    let prev = &audio[pieces[i - 1].0..pieces[i - 1].1];
                    let cs = crate::take_context_suffix(prev, "", "", RATE_CPS);
                    waudio.extend_from_slice(&prev[cs.cut.min(prev.len())..]);
                    wsamples.push(cs.suffix_samples);
                }
                waudio.extend_from_slice(&audio[ps..pe]);
                wsamples.push(pe - ps);
                let span_start = i.saturating_sub(1);
                let split = crate::window_overlap_split(
                    span_start,
                    if i >= 1 { Some(i) } else { None },
                    &waudio,
                    &wsamples,
                );
                let stream_text = run_stream(&waudio);
                let use_draft = matches!(*v, "draft" | "full");
                let use_terms = matches!(*v, "terms" | "full");
                let inject = CtxInject {
                    terms: if use_terms { terms.as_deref() } else { None },
                    avg_chars_per_sec: None,
                    speech_ranges: None,
                    streaming_nonempty: !stream_text.trim().is_empty(),
                    new_slice_from: wsamples[..wsamples.len() - 1].iter().sum(),
                    assist: crate::transcription::llama_asr::DecodeAssist::draft(
                        use_draft.then_some(stream_text.as_str()),
                    ),
                };
                let t0 = Instant::now();
                let text = transcribe_acc_ctx(&acc, &waudio, ChineseScript::Simplified, i, inject)
                    .map(|(t, _, _)| t)
                    .unwrap_or_default();
                agg[vi].ms += t0.elapsed().as_secs_f64() * 1000.0;
                agg[vi].windows += 1;
                if text.trim().is_empty() {
                    agg[vi].empty += 1;
                }
                let out = reflow.push_window_streaming(
                    i,
                    span_start,
                    i + 1,
                    wsamples.clone(),
                    text,
                    stream_text,
                    split,
                );
                if let Some(f) = out.last().filter(|f| !f.is_empty()) {
                    full_text = f.clone();
                }
            }
            let (fc, lw) = reflow.finish();
            let final_text = format!("{fc}{lw}");
            let final_text = if final_text.trim().is_empty() {
                full_text
            } else {
                final_text
            };
            if has_repeat8(&final_text) {
                agg[vi].rep8 += 1;
            }
            let c = reference.as_deref().map(|r| cer(&final_text, r));
            if let Some(c) = c {
                agg[vi].cer_sum += c;
                agg[vi].cer_n += 1;
            }
            report.push_str(&format!(
                "- **{v}** CER={} | {}\n",
                c.map_or("-".into(), |c| format!("{c:.4}")),
                final_text
            ));
        }
        println!("[452] {name} done");
    }
    report.push_str("\n## 汇总\n\n| 变体 | 平均 CER（有参考段） | 空窗 | ≥8 字重复录音 | 解码总耗时 | 窗数 |\n|---|---:|---:|---:|---:|---:|\n");
    for (vi, v) in VARIANTS.iter().enumerate() {
        let a = &agg[vi];
        let line = format!(
            "| {v} | {} | {} | {} | {:.1}s | {} |\n",
            if a.cer_n > 0 {
                format!("{:.4}", a.cer_sum / a.cer_n as f32)
            } else {
                "-".into()
            },
            a.empty,
            a.rep8,
            a.ms / 1000.0,
            a.windows
        );
        print!("{line}");
        report.push_str(&line);
    }
    let out = root.join(format!(
        "collab/evidence/452/replay452{}.md",
        std::env::var("REPLAY_TAG").unwrap_or_default()
    ));
    std::fs::write(&out, report).unwrap();
    println!("[452] 报告：{}", out.display());
}

/// ACC-452 ④ 续写 A/B：同一几何，对照「现接缝拼接（base）」与「续写」：
/// 续写窗 = assistant 预填 `language Chinese<asr_text>` + 本变体已出全文末尾 `CONT_PRIOR_CHARS` 字，
/// 新窗文字 = 模型接着生成的部分，直接追加（不经接缝裁判）。首窗无前文 ⇒ 普通解码。
/// 变体：base / cont / cont+draft / cont+terms。剪静音同生产（自跑 VAD + pad）。
#[test]
#[ignore = "ACC-452 续写回放：cargo test --bin feiyin-ime replay452_continue -- --ignored --nocapture"]
fn replay452_continue() {
    const CONT_PRIOR_CHARS: usize = 60;
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let root = manifest_dir();
    let models = root.join("models");
    let acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
    let st_rec = crate::transcription::local_stream::create_local_stream_recognizer(&models)
        .expect("流式模型须在位");
    let vad = VadSegmenter::try_new_for_local_trim(&models).expect("VAD");
    let pad = (vad::LOCALRT_TRIM_PAD_SECS * RATE as f32) as usize;
    let terms = load_real_wordbook_terms();
    println!(
        "[452c] device={} wordbook_terms_chars={} entries={}",
        acc.device(),
        terms.as_deref().map_or(0, |t| t.chars().count()),
        terms.as_deref().map_or(0, |t| t.split(',').count())
    );
    let run_stream = |audio: &[f32]| -> String {
        let stream = st_rec.create_stream();
        for c in audio.chunks(1600) {
            stream.accept_waveform(RATE as i32, c);
            while st_rec.is_ready(&stream) {
                st_rec.decode(&stream);
            }
        }
        stream.input_finished();
        while st_rec.is_ready(&stream) {
            st_rec.decode(&stream);
        }
        st_rec
            .get_result(&stream)
            .map(|r| r.text.clone())
            .unwrap_or_default()
    };
    // cont = 重叠对齐（整窗音频 + 预填重叠文字）；cont_new = **不做重叠**（只送新片音频 + 预填上文末尾 60 字，
    // 不经接缝裁判直接追加）—— 回答 Gavin「用续写就不用自己做窗口重叠和文字覆盖」。
    const VARIANTS: [&str; 4] = ["base", "cont", "cont_new", "cont_new+terms"];
    let mut cer_sum = [0f32; 4];
    let mut cer_n = [0usize; 4];
    let mut rep8 = [0usize; 4];
    let mut empty = [0usize; 4];
    let mut ms = [0f64; 4];
    let mut report = String::from("# ACC-452 续写回放\n");
    for wav in collect_wavs(&root) {
        let name = wav
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        let Some((audio, rate)) = read_wav(&wav) else {
            continue;
        };
        if rate as usize != RATE {
            continue;
        }
        let mut pieces: Vec<(usize, usize)> = Vec::new();
        for (s, e) in dispatch_slices(&audio) {
            pieces.extend(vad::plan_gap_cuts(&audio, s, e));
        }
        let reference = read_ref_for(&name);
        report.push_str(&format!(
            "\n## {name}（{} 窗，参考 {}）\n",
            pieces.len(),
            reference.is_some()
        ));
        for (vi, v) in VARIANTS.iter().enumerate() {
            let mut reflow = OrderedReflow::new();
            let mut full = String::new();
            let mut prev_win_text = String::new();
            let mut prev_total = 0usize;
            for (i, &(ps, pe)) in pieces.iter().enumerate() {
                let mut waudio: Vec<f32> = Vec::new();
                let mut wsamples: Vec<usize> = Vec::new();
                if i >= 1 {
                    let prev = &audio[pieces[i - 1].0..pieces[i - 1].1];
                    let cs = crate::take_context_suffix(prev, "", "", RATE_CPS);
                    waudio.extend_from_slice(&prev[cs.cut.min(prev.len())..]);
                    wsamples.push(cs.suffix_samples);
                }
                waudio.extend_from_slice(&audio[ps..pe]);
                wsamples.push(pe - ps);
                let used = trim_to_speech(&waudio, &vad.speech_ranges(&waudio), pad);
                let used: &[f32] = if used.is_empty() { &waudio } else { &used };
                let cap = max_new_tokens_for(used.len() as f32 / RATE as f32);
                let t0 = Instant::now();
                if *v == "base" {
                    let span_start = i.saturating_sub(1);
                    let split = crate::window_overlap_split(
                        span_start,
                        if i >= 1 { Some(i) } else { None },
                        &waudio,
                        &wsamples,
                    );
                    let stream_text = run_stream(&waudio);
                    // base 走**生产** transcribe_acc_ctx（含处置阶梯 / 426 重解），与 replay452 base 同口径。
                    let _ = (used, cap);
                    let inject = CtxInject {
                        terms: None,
                        avg_chars_per_sec: None,
                        speech_ranges: None,
                        streaming_nonempty: !stream_text.trim().is_empty(),
                        new_slice_from: wsamples[..wsamples.len() - 1].iter().sum(),
                        assist: Default::default(),
                    };
                    let text =
                        transcribe_acc_ctx(&acc, &waudio, ChineseScript::Simplified, i, inject)
                            .map(|(t, _, _)| t)
                            .unwrap_or_default();
                    ms[vi] += t0.elapsed().as_secs_f64() * 1000.0;
                    if text.trim().is_empty() {
                        empty[vi] += 1;
                    }
                    let out = reflow.push_window_streaming(
                        i,
                        span_start,
                        i + 1,
                        wsamples.clone(),
                        text,
                        stream_text,
                        split,
                    );
                    if let Some(f) = out.last().filter(|f| !f.is_empty()) {
                        full = f.clone();
                    }
                } else {
                    // 续写：cont = 重叠对齐（整窗音频 + 预填前片后缀**对应的文字**）；cont_new = 只送新片 + 预填上文末尾
                    //（按共享样本占上一窗样本比例估算字数，同 433 估算层），模型从重叠结束处接着写。
                    let draft_txt: Option<String> = None;
                    let sys = if v.ends_with("+terms") {
                        terms.as_deref()
                    } else {
                        None
                    };
                    let no_overlap = v.starts_with("cont_new");
                    let new_used =
                        trim_to_speech(&audio[ps..pe], &vad.speech_ranges(&audio[ps..pe]), pad);
                    let used: &[f32] = if no_overlap && i > 0 {
                        if new_used.is_empty() {
                            &audio[ps..pe]
                        } else {
                            &new_used
                        }
                    } else {
                        used
                    };
                    let cap = max_new_tokens_for(used.len() as f32 / RATE as f32);
                    let t0 = Instant::now();
                    let prior: String = if no_overlap {
                        let n = full.chars().count();
                        full.chars()
                            .skip(n.saturating_sub(CONT_PRIOR_CHARS))
                            .collect()
                    } else if i == 0 || prev_win_text.is_empty() || prev_total == 0 {
                        String::new()
                    } else {
                        let shared = wsamples.first().copied().unwrap_or(0);
                        let eff_n = effective_chars(&prev_win_text).len();
                        let want =
                            ((eff_n as f32) * (shared as f32 / prev_total as f32)).round() as usize;
                        // 从 prev_win_text 末尾往前取，直到含 `want` 个有效字（保留其间标点）。
                        let cs: Vec<char> = prev_win_text.chars().collect();
                        let (mut k, mut got) = (cs.len(), 0usize);
                        while k > 0 && got < want {
                            k -= 1;
                            if effective_chars(&cs[k].to_string()).len() == 1 {
                                got += 1;
                            }
                        }
                        cs[k..].iter().collect()
                    };
                    let gen = if prior.is_empty() {
                        acc.decode(
                            used,
                            sys,
                            None,
                            Some(cap),
                            crate::transcription::llama_asr::DecodeAssist::draft(
                                draft_txt.as_deref(),
                            ),
                        )
                        .map(|o| Transcriber::strip_asr_special_tokens(o.raw.trim()))
                        .unwrap_or_default()
                    } else {
                        acc.decode_continue(
                            used,
                            sys,
                            "Chinese",
                            &prior,
                            Some(cap),
                            crate::transcription::llama_asr::DecodeAssist::draft(
                                draft_txt.as_deref(),
                            ),
                        )
                        .map(|o| o.raw)
                        .unwrap_or_default()
                    };
                    ms[vi] += t0.elapsed().as_secs_f64() * 1000.0;
                    let gen = crate::text_normalizer::normalize_text_for_language(
                        gen.trim(),
                        ChineseScript::Simplified,
                    );
                    if gen.trim().is_empty() {
                        empty[vi] += 1;
                    }
                    prev_win_text = format!("{prior}{gen}");
                    prev_total = waudio.len();
                    full.push_str(&gen);
                }
            }
            let final_text = if *v == "base" {
                let (fc, lw) = reflow.finish();
                let f = format!("{fc}{lw}");
                if f.trim().is_empty() {
                    full.clone()
                } else {
                    f
                }
            } else {
                full.clone()
            };
            if has_repeat8(&final_text) {
                rep8[vi] += 1;
            }
            let c = reference.as_deref().map(|r| cer(&final_text, r));
            if let Some(c) = c {
                cer_sum[vi] += c;
                cer_n[vi] += 1;
            }
            report.push_str(&format!(
                "- **{v}** CER={} | {}\n",
                c.map_or("-".into(), |c| format!("{c:.4}")),
                final_text
            ));
        }
        println!("[452c] {name} done");
    }
    report.push_str(
        "\n| 变体 | 平均 CER | 空窗 | ≥8 字重复录音 | 解码总耗时 |\n|---|---:|---:|---:|---:|\n",
    );
    for (vi, v) in VARIANTS.iter().enumerate() {
        let line = format!(
            "| {v} | {:.4} | {} | {} | {:.1}s |\n",
            cer_sum[vi] / cer_n[vi].max(1) as f32,
            empty[vi],
            rep8[vi],
            ms[vi] / 1000.0
        );
        print!("{line}");
        report.push_str(&line);
    }
    std::fs::write(root.join("collab/evidence/452/replay452c.md"), report).unwrap();
}

/// ACC-452 ⑤ 流式回灌回放：完全按生产逻辑（150ms 限频 / 副本试拼接 / 比当前显示长才显示）模拟中途显示，
/// 统计：① 闪烁 = 中途显示的全文**不是**本窗正式全文的前缀（有效字口径）的次数；② 提前量 = 首次显示新内容
/// 相对整窗解完提前的毫秒数。
#[test]
#[ignore = "ACC-452 流式回灌回放：cargo test --bin feiyin-ime replay452_stream -- --ignored --nocapture"]
fn replay452_stream() {
    use std::sync::Mutex as StdMutex;
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let root = manifest_dir();
    let models = root.join("models");
    let acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
    let st_rec = crate::transcription::local_stream::create_local_stream_recognizer(&models)
        .expect("流式模型须在位");
    let terms = load_real_wordbook_terms();
    let run_stream = |audio: &[f32]| -> String {
        let stream = st_rec.create_stream();
        for c in audio.chunks(1600) {
            stream.accept_waveform(RATE as i32, c);
            while st_rec.is_ready(&stream) {
                st_rec.decode(&stream);
            }
        }
        stream.input_finished();
        while st_rec.is_ready(&stream) {
            st_rec.decode(&stream);
        }
        st_rec
            .get_result(&stream)
            .map(|r| r.text.clone())
            .unwrap_or_default()
    };
    let (mut shown_n, mut flicker_n, mut windows_with_partial, mut windows) =
        (0usize, 0usize, 0usize, 0usize);
    let mut lead_ms_sum = 0f64;
    let mut examples = String::new();
    for wav in collect_wavs(&root) {
        let Some((audio, rate)) = read_wav(&wav) else {
            continue;
        };
        if rate as usize != RATE {
            continue;
        }
        let mut pieces: Vec<(usize, usize)> = Vec::new();
        for (s, e) in dispatch_slices(&audio) {
            pieces.extend(vad::plan_gap_cuts(&audio, s, e));
        }
        let mut reflow = OrderedReflow::new();
        let mut last_auth = String::new();
        for (i, &(ps, pe)) in pieces.iter().enumerate() {
            let mut waudio: Vec<f32> = Vec::new();
            let mut wsamples: Vec<usize> = Vec::new();
            if i >= 1 {
                let prev = &audio[pieces[i - 1].0..pieces[i - 1].1];
                let cs = crate::take_context_suffix(prev, "", "", RATE_CPS);
                waudio.extend_from_slice(&prev[cs.cut.min(prev.len())..]);
                wsamples.push(cs.suffix_samples);
            }
            waudio.extend_from_slice(&audio[ps..pe]);
            wsamples.push(pe - ps);
            let span_start = i.saturating_sub(1);
            let split = crate::window_overlap_split(
                span_start,
                if i >= 1 { Some(i) } else { None },
                &waudio,
                &wsamples,
            );
            let stream_text = run_stream(&waudio);
            // 收集半截结果（带时间戳），生产同款 150ms 限频。
            let partials: StdMutex<Vec<(f64, String)>> = StdMutex::new(Vec::new());
            let t0 = Instant::now();
            let last = StdMutex::new(Instant::now());
            let cb = |raw: &str| {
                let mut l = last.lock().unwrap();
                if l.elapsed() >= std::time::Duration::from_millis(150) {
                    *l = Instant::now();
                    partials
                        .lock()
                        .unwrap()
                        .push((t0.elapsed().as_secs_f64() * 1000.0, raw.to_string()));
                }
            };
            let inject = CtxInject {
                terms: terms.as_deref(),
                avg_chars_per_sec: None,
                speech_ranges: None,
                streaming_nonempty: !stream_text.trim().is_empty(),
                new_slice_from: wsamples[..wsamples.len() - 1].iter().sum(),
                assist: crate::transcription::llama_asr::DecodeAssist {
                    draft: Some(stream_text.as_str()),
                    on_partial: Some(&cb),
                },
            };
            let text = transcribe_acc_ctx(&acc, &waudio, ChineseScript::Simplified, i, inject)
                .map(|(t, _, _)| t)
                .unwrap_or_default();
            let done_ms = t0.elapsed().as_secs_f64() * 1000.0;
            windows += 1;
            // 生产同款试拼接：副本 + 过滤 + 比当前显示长才显示。
            let mut shown_len = last_auth.chars().count();
            let mut displayed: Vec<String> = Vec::new();
            let mut first_ms: Option<f64> = None;
            for (ms, raw) in partials.lock().unwrap().iter() {
                if !raw.contains("<asr_text>") {
                    continue;
                }
                let stripped = Transcriber::strip_asr_special_tokens(raw.trim());
                let partial = crate::text_normalizer::normalize_text_for_language(
                    stripped.trim(),
                    ChineseScript::Simplified,
                );
                if partial.is_empty() || !partial_past_overlap(&partial, &stream_text, &wsamples) {
                    continue;
                }
                let mut probe = reflow.clone();
                if let Some(c) = probe
                    .push_window_streaming(
                        i,
                        span_start,
                        i + 1,
                        wsamples.clone(),
                        partial,
                        stream_text.clone(),
                        split,
                    )
                    .last()
                {
                    let c = crate::apply_authoritative_filler_dedup(c);
                    if c.chars().count() > shown_len {
                        shown_len = c.chars().count();
                        first_ms.get_or_insert(*ms);
                        displayed.push(c);
                    }
                }
            }
            let out = reflow.push_window_streaming(
                i,
                span_start,
                i + 1,
                wsamples.clone(),
                text,
                stream_text,
                split,
            );
            let final_full = out
                .last()
                .map(|f| crate::apply_authoritative_filler_dedup(f))
                .unwrap_or_else(|| last_auth.clone());
            let fe = effective_chars(&final_full);
            if !displayed.is_empty() {
                windows_with_partial += 1;
                lead_ms_sum += done_ms - first_ms.unwrap_or(done_ms);
            }
            for d in &displayed {
                shown_n += 1;
                let de = effective_chars(d);
                if !(de.len() <= fe.len() && fe[..de.len()] == de[..]) {
                    flicker_n += 1;
                    if examples.lines().count() < 20 {
                        examples.push_str(&format!(
                            "- 中途：…{}\n  最终：…{}\n",
                            clip_tail(d, 30),
                            clip_tail(&final_full, 30)
                        ));
                    }
                }
            }
            if !final_full.is_empty() {
                last_auth = final_full;
            }
        }
    }
    let summary = format!(
        "窗 {windows}，有中途显示的窗 {windows_with_partial}，中途显示 {shown_n} 次，其中被最终改掉（闪烁）{flicker_n} 次（{:.1}%），新内容平均提前 {:.0}ms\n",
        flicker_n as f64 * 100.0 / shown_n.max(1) as f64,
        lead_ms_sum / windows_with_partial.max(1) as f64
    );
    print!("[452s] {summary}");
    std::fs::write(
        root.join("collab/evidence/452/replay452s.md"),
        format!("# ACC-452 流式回灌回放\n\n{summary}\n## 闪烁样例\n{examples}"),
    )
    .unwrap();
}

fn clip_tail(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}

/// ACC-452 ⑤ 回缩回放：按浮层真实合成（`reflow_preview`）模拟每次半截显示前后的显示长度。
/// 原始预览 ≈ 各片流式文字依次拼接（生产为整段流式，近似）；整窗边界 = 截至本片的预览字数。
/// 对照：旧做法（按整窗边界替换）vs 新做法（`partial_reflow_boundary` 现算边界）的回缩次数。
#[test]
#[ignore = "ACC-452 回缩回放：cargo test --bin feiyin-ime replay452_shrink -- --ignored --nocapture"]
fn replay452_shrink() {
    use std::sync::Mutex as StdMutex;
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let root = manifest_dir();
    let models = root.join("models");
    let acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
    let st_rec =
        crate::transcription::local_stream::create_local_stream_recognizer(&models).expect("流式");
    let terms = load_real_wordbook_terms();
    let run_stream = |audio: &[f32]| -> String {
        let stream = st_rec.create_stream();
        for c in audio.chunks(1600) {
            stream.accept_waveform(RATE as i32, c);
            while st_rec.is_ready(&stream) {
                st_rec.decode(&stream);
            }
        }
        stream.input_finished();
        while st_rec.is_ready(&stream) {
            st_rec.decode(&stream);
        }
        st_rec
            .get_result(&stream)
            .map(|r| r.text.clone())
            .unwrap_or_default()
    };
    let (mut events, mut shrink_old, mut shrink_new, mut shown_new) =
        (0usize, 0usize, 0usize, 0usize);
    let (mut big_old, mut big_new, mut max_new) = (0usize, 0usize, 0usize);
    for wav in collect_wavs(&root) {
        let Some((audio, rate)) = read_wav(&wav) else {
            continue;
        };
        if rate as usize != RATE {
            continue;
        }
        let mut pieces: Vec<(usize, usize)> = Vec::new();
        for (s, e) in dispatch_slices(&audio) {
            pieces.extend(vad::plan_gap_cuts(&audio, s, e));
        }
        let mut reflow = OrderedReflow::new();
        let (mut raw, mut state_acc, mut state_len, mut last_auth) =
            (String::new(), String::new(), 0usize, String::new());
        for (i, &(ps, pe)) in pieces.iter().enumerate() {
            raw.push_str(&run_stream(&audio[ps..pe]));
            let window_end = raw.chars().count();
            let mut waudio: Vec<f32> = Vec::new();
            let mut wsamples: Vec<usize> = Vec::new();
            if i >= 1 {
                let prev = &audio[pieces[i - 1].0..pieces[i - 1].1];
                let cs = crate::take_context_suffix(prev, "", "", RATE_CPS);
                waudio.extend_from_slice(&prev[cs.cut.min(prev.len())..]);
                wsamples.push(cs.suffix_samples);
            }
            waudio.extend_from_slice(&audio[ps..pe]);
            wsamples.push(pe - ps);
            let span_start = i.saturating_sub(1);
            let split = crate::window_overlap_split(
                span_start,
                if i >= 1 { Some(i) } else { None },
                &waudio,
                &wsamples,
            );
            let stream_text = run_stream(&waudio);
            let partials: StdMutex<Vec<String>> = StdMutex::new(Vec::new());
            let last = StdMutex::new(Instant::now());
            let cb = |r: &str| {
                let mut l = last.lock().unwrap();
                if l.elapsed() >= std::time::Duration::from_millis(150) {
                    *l = Instant::now();
                    partials.lock().unwrap().push(r.to_string());
                }
            };
            let inject = CtxInject {
                terms: terms.as_deref(),
                avg_chars_per_sec: None,
                speech_ranges: None,
                streaming_nonempty: !stream_text.trim().is_empty(),
                new_slice_from: wsamples[..wsamples.len() - 1].iter().sum(),
                assist: crate::transcription::llama_asr::DecodeAssist {
                    draft: Some(stream_text.as_str()),
                    on_partial: Some(&cb),
                },
            };
            let text = transcribe_acc_ctx(&acc, &waudio, ChineseScript::Simplified, i, inject)
                .map(|(t, _, _)| t)
                .unwrap_or_default();
            let mut shown_len = last_auth.chars().count();
            for r in partials.lock().unwrap().iter() {
                if !r.contains("<asr_text>") {
                    continue;
                }
                let p = crate::text_normalizer::normalize_text_for_language(
                    Transcriber::strip_asr_special_tokens(r.trim()).trim(),
                    ChineseScript::Simplified,
                );
                if p.is_empty() || !partial_past_overlap(&p, &stream_text, &wsamples) {
                    continue;
                }
                let mut probe = reflow.clone();
                let Some(c) = probe
                    .push_window_streaming(
                        i,
                        span_start,
                        i + 1,
                        wsamples.clone(),
                        p,
                        stream_text.clone(),
                        split,
                    )
                    .last()
                    .map(|c| crate::apply_authoritative_filler_dedup(c))
                else {
                    continue;
                };
                if c.chars().count() <= shown_len {
                    continue;
                }
                shown_len = c.chars().count();
                events += 1;
                let before = crate::reflow_preview(&state_acc, &raw, state_len)
                    .chars()
                    .count();
                let after_old = crate::reflow_preview(&c, &raw, window_end).chars().count();
                if after_old < before {
                    shrink_old += 1;
                    if before - after_old > 2 {
                        big_old += 1;
                    }
                }
                // 同生产最后一道保护：新显示比当前短 > 2 字 ⇒ 跳过本次。
                if let Some(b) =
                    crate::partial_reflow_boundary(&state_acc, state_len, &c, &raw, window_end)
                        .filter(|&b| {
                            crate::reflow_preview(&c, &raw, b).chars().count() + 2 >= before
                        })
                {
                    shown_new += 1;
                    let after_new = crate::reflow_preview(&c, &raw, b).chars().count();
                    if after_new < before {
                        shrink_new += 1;
                        max_new = max_new.max(before - after_new);
                        if before - after_new > 2 {
                            big_new += 1;
                        }
                    }
                    state_acc = c;
                    state_len = b;
                }
            }
            if let Some(f) = reflow
                .push_window_streaming(
                    i,
                    span_start,
                    i + 1,
                    wsamples.clone(),
                    text,
                    stream_text,
                    split,
                )
                .last()
            {
                last_auth = crate::apply_authoritative_filler_dedup(f);
                state_acc = last_auth.clone();
                state_len = window_end;
            }
        }
    }
    println!("[452r] 半截事件 {events}：旧做法回缩 {shrink_old} 次（>2 字 {big_old}）；新做法显示 {shown_new} 次、回缩 {shrink_new} 次（>2 字 {big_new}，最大 {max_new} 字）");
}
