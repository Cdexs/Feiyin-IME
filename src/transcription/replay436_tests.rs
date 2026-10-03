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

/// FORCED-ALIGN-456：回放用接缝推送 —— 与生产同一做法：本窗文字 × 整窗音频强制对齐 → 窗内秒换算到
/// 虚拟时间轴（`crate::window_abs_geometry`，片首尾相接）→ `push_window_timed`。
#[allow(clippy::too_many_arguments)]
fn push_timed_456(
    reflow: &mut OrderedReflow,
    acc: &AccEngine,
    cum_end: &[usize],
    seq: usize,
    span_start: usize,
    end: usize,
    wsamples: &[usize],
    waudio: &[f32],
    text: String,
    split: Option<f32>,
    with_times: bool,
) -> Vec<String> {
    let prev_end = if seq >= 1 { Some(seq) } else { None };
    let (abs, split_abs) =
        crate::window_abs_geometry(cum_end, end, span_start, prev_end, wsamples, split);
    let times = if with_times && !text.trim().is_empty() {
        acc.align(waudio, &text).map(|o| {
            o.char_times
                .into_iter()
                .map(|(a, b)| (abs + a, abs + b))
                .collect()
        })
    } else {
        None
    };
    reflow.push_window_timed(
        seq,
        span_start,
        end,
        wsamples.to_vec(),
        text,
        times,
        abs,
        split_abs,
    )
}

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
    let mut acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
    acc.attach_aligner(&models); // FORCED-ALIGN-456：与生产一致，接缝按逐字时间拼接
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
        // FORCED-ALIGN-456：各片累计结束样本（虚拟时间轴，与生产 `slice_cum_end` 同义）。
        let cum_end: Vec<usize> = pieces
            .iter()
            .scan(0usize, |acc, &(ps, pe)| {
                *acc += pe - ps;
                Some(*acc)
            })
            .collect();
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
                let out = push_timed_456(
                    &mut reflow,
                    &acc,
                    &cum_end,
                    i,
                    span_start,
                    i + 1,
                    &wsamples,
                    &waudio,
                    text,
                    split,
                    true,
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
    let mut acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
    acc.attach_aligner(&models); // FORCED-ALIGN-456：与生产一致，接缝按逐字时间拼接
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
        // FORCED-ALIGN-456：各片累计结束样本（虚拟时间轴，与生产 `slice_cum_end` 同义）。
        let cum_end: Vec<usize> = pieces
            .iter()
            .scan(0usize, |acc, &(ps, pe)| {
                *acc += pe - ps;
                Some(*acc)
            })
            .collect();
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
                if let Some(c) = push_timed_456(
                    &mut probe,
                    &acc,
                    &cum_end,
                    i,
                    span_start,
                    i + 1,
                    &wsamples,
                    &waudio,
                    partial,
                    split,
                    false,
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
            let out = push_timed_456(
                &mut reflow,
                &acc,
                &cum_end,
                i,
                span_start,
                i + 1,
                &wsamples,
                &waudio,
                text,
                split,
                true,
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
    let mut acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
    acc.attach_aligner(&models); // FORCED-ALIGN-456：与生产一致，接缝按逐字时间拼接
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
        // FORCED-ALIGN-456：各片累计结束样本（虚拟时间轴，与生产 `slice_cum_end` 同义）。
        let cum_end: Vec<usize> = pieces
            .iter()
            .scan(0usize, |acc, &(ps, pe)| {
                *acc += pe - ps;
                Some(*acc)
            })
            .collect();
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
                let Some(c) = push_timed_456(
                    &mut probe,
                    &acc,
                    &cum_end,
                    i,
                    span_start,
                    i + 1,
                    &wsamples,
                    &waudio,
                    p,
                    split,
                    false,
                )
                .last()
                .map(|c| crate::apply_authoritative_filler_dedup(c)) else {
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
            if let Some(f) = push_timed_456(
                &mut reflow,
                &acc,
                &cum_end,
                i,
                span_start,
                i + 1,
                &wsamples,
                &waudio,
                text,
                split,
                true,
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

// ========================================================================
// POC-ALIGN-455 第二步：按时间拼接 vs 现行接缝（回放 A/B，生产零改动）
//
// Gavin 2026-09-29「做吧」（对齐模型评估 → 选 CTC 路线 → 回放验值不值）。
// 每窗精解一次（生产配置：草稿 + 词库），同一结果两种拼法：
//   cur   = 生产 `OrderedReflow::push_window_timed`（FORCED-ALIGN-456：0.6B 对齐逐字时间拼接，无时间回落 371 比例）
//   timed = 同窗音频跑性能档 FunASR Nano CTC 取逐字时间 → 与精解文字全局对齐把时间搬过去 →
//           上一窗留「时间 < 分界」、新窗接「时间 ≥ 分界」（分界同 436：重叠区内部真停顿）。
// 输出：有参考录音 CER、≥8 字重复录音数、CTC 耗时、逐录音全文对照 → collab/evidence/455/。
// 运行：cargo test --bin feiyin-ime replay455 -- --ignored --nocapture
// ========================================================================

/// 对齐用「内容字」：汉字 / 字母 / 数字（小写），标点与空白不参与。
fn content_char_455(c: char) -> Option<char> {
    if c.is_alphanumeric() {
        c.to_lowercase().next()
    } else {
        None
    }
}

/// CTC 结果 → 逐内容字 (字, 窗内秒)。特殊记号（`<|…|>`）与空白丢弃；多字记号各字同一时间。
fn ctc_chars_455(tokens: &[String], ts: &[f32]) -> Vec<(char, f32)> {
    let mut out = Vec::new();
    for (t, &x) in tokens.iter().zip(ts.iter()) {
        if t.starts_with("<|") {
            continue;
        }
        for c in t.chars().filter_map(content_char_455) {
            out.push((c, x));
        }
    }
    out
}

/// 全局对齐（Needleman-Wunsch，匹配 +2 / 替换 −1 / 空位 −1）把 CTC 时间搬到精解文字每个字上。
/// 返回 (每字窗内秒, 匹配率)；未对上的内容字按前后已知时间线性插值，标点取前一字时间。
fn qwen_char_times_455(text: &str, ctc: &[(char, f32)]) -> (Vec<f32>, f32) {
    let chars: Vec<char> = text.chars().collect();
    let content: Vec<(usize, char)> = chars
        .iter()
        .enumerate()
        .filter_map(|(i, &c)| content_char_455(c).map(|k| (i, k)))
        .collect();
    let (n, m) = (content.len(), ctc.len());
    let mut assigned: Vec<Option<f32>> = vec![None; n];
    let mut matched = 0usize;
    if n > 0 && m > 0 {
        let mut dp = vec![vec![0i32; m + 1]; n + 1];
        for (i, row) in dp.iter_mut().enumerate() {
            row[0] = -(i as i32);
        }
        for j in 0..=m {
            dp[0][j] = -(j as i32);
        }
        for i in 1..=n {
            for j in 1..=m {
                let s = if content[i - 1].1 == ctc[j - 1].0 {
                    2
                } else {
                    -1
                };
                dp[i][j] = (dp[i - 1][j - 1] + s)
                    .max(dp[i - 1][j] - 1)
                    .max(dp[i][j - 1] - 1);
            }
        }
        let (mut i, mut j) = (n, m);
        while i > 0 && j > 0 {
            let s = if content[i - 1].1 == ctc[j - 1].0 {
                2
            } else {
                -1
            };
            if dp[i][j] == dp[i - 1][j - 1] + s {
                assigned[i - 1] = Some(ctc[j - 1].1);
                if s == 2 {
                    matched += 1;
                }
                i -= 1;
                j -= 1;
            } else if dp[i][j] == dp[i - 1][j] - 1 {
                i -= 1;
            } else {
                j -= 1;
            }
        }
    }
    // 插值：未对上的内容字取前后已知时间的线性插值（两端缺则取最近已知 / 0）。
    let known: Vec<(usize, f32)> = assigned
        .iter()
        .enumerate()
        .filter_map(|(k, t)| t.map(|t| (k, t)))
        .collect();
    let filled: Vec<f32> = (0..n)
        .map(|k| {
            if let Some(t) = assigned[k] {
                return t;
            }
            let prev = known.iter().rev().find(|(p, _)| *p < k);
            let next = known.iter().find(|(p, _)| *p > k);
            match (prev, next) {
                (Some(&(pi, pt)), Some(&(ni, nt))) => {
                    pt + (nt - pt) * (k - pi) as f32 / (ni - pi) as f32
                }
                (Some(&(_, pt)), None) => pt,
                (None, Some(&(_, nt))) => nt,
                (None, None) => 0.0,
            }
        })
        .collect();
    // 回到全部字符：内容字用 filled，标点 / 空白取前一内容字时间（开头取第一个内容字时间）。
    let mut times = vec![0.0f32; chars.len()];
    let mut ci = 0usize;
    let mut last = filled.first().copied().unwrap_or(0.0);
    for (i, t) in times.iter_mut().enumerate() {
        if ci < n && content[ci].0 == i {
            last = filled[ci];
            ci += 1;
        }
        *t = last;
    }
    let rate = if n == 0 {
        1.0
    } else {
        matched as f32 / n as f32
    };
    (times, rate)
}

#[test]
#[ignore = "POC-ALIGN-455 回放 A/B：需 GGUF + 流式 + FunASR Nano CTC + 录音；cargo test --bin feiyin-ime replay455 -- --ignored --nocapture"]
fn replay455_timed_seam() {
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let root = manifest_dir();
    let models = root.join("models");
    let mut acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
    acc.attach_aligner(&models); // FORCED-ALIGN-456：与生产一致，接缝按逐字时间拼接
    let st_rec = crate::transcription::local_stream::create_local_stream_recognizer(&models)
        .expect("流式模型须在位");
    let ctc = create_sensevoice_recognizer(&models, "auto").expect("FunASR Nano CTC 须在位");
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
    let (mut cer_cur, mut cer_timed, mut cer_n) = (0f32, 0f32, 0usize);
    let (mut rep_cur, mut rep_timed) = (0usize, 0usize);
    let (mut ctc_ms, mut windows, mut low_match, mut rate_sum) = (0f64, 0usize, 0usize, 0f32);
    let mut report = String::from("# POC-ALIGN-455 按时间拼接 vs 现行接缝\n");
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
        // FORCED-ALIGN-456：各片累计结束样本（虚拟时间轴，与生产 `slice_cum_end` 同义）。
        let cum_end: Vec<usize> = pieces
            .iter()
            .scan(0usize, |acc, &(ps, pe)| {
                *acc += pe - ps;
                Some(*acc)
            })
            .collect();
        let reference = read_ref_for(&name);
        let mut reflow = OrderedReflow::new();
        let mut cur_full = String::new();
        // timed：已拼接的 (字, 录音绝对秒)。
        let mut timed: Vec<(char, f32)> = Vec::new();
        let mut seams: Vec<String> = Vec::new();
        for (i, &(ps, pe)) in pieces.iter().enumerate() {
            let mut waudio: Vec<f32> = Vec::new();
            let mut wsamples: Vec<usize> = Vec::new();
            let mut win_start = ps;
            if i >= 1 {
                let (pps, ppe) = pieces[i - 1];
                let prev = &audio[pps..ppe];
                let cs = crate::take_context_suffix(prev, "", "", RATE_CPS);
                let cut = cs.cut.min(prev.len());
                waudio.extend_from_slice(&prev[cut..]);
                wsamples.push(cs.suffix_samples);
                win_start = pps + cut;
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
            let inject = CtxInject {
                terms: terms.as_deref(),
                avg_chars_per_sec: None,
                speech_ranges: None,
                streaming_nonempty: !stream_text.trim().is_empty(),
                new_slice_from: wsamples[..wsamples.len() - 1].iter().sum(),
                assist: crate::transcription::llama_asr::DecodeAssist::draft(Some(
                    stream_text.as_str(),
                )),
            };
            let text = transcribe_acc_ctx(&acc, &waudio, ChineseScript::Simplified, i, inject)
                .map(|(t, _, _)| t)
                .unwrap_or_default();
            windows += 1;
            // ---- timed：CTC 逐字时间 → 精解文字 ----
            let s = ctc.create_stream();
            s.accept_waveform(RATE as i32, &waudio);
            let t0 = Instant::now();
            ctc.decode(&s);
            ctc_ms += t0.elapsed().as_secs_f64() * 1000.0;
            let r = s.get_result().expect("ctc result");
            let cc = ctc_chars_455(&r.tokens, &r.timestamps.clone().unwrap_or_default());
            let (times, mrate) = qwen_char_times_455(&text, &cc);
            rate_sum += mrate;
            if mrate < 0.6 {
                low_match += 1;
            }
            let win_abs = win_start as f32 / RATE as f32;
            // 分界（绝对秒）：有重叠 ⇒ 重叠区内部真停顿（436 同款）；无重叠 ⇒ 本窗起点。
            let t_split = match (i >= 1, split) {
                (true, Some(f)) => win_abs + f * wsamples[0] as f32 / RATE as f32,
                (true, None) => ps as f32 / RATE as f32,
                _ => win_abs,
            };
            // DUMP455=<目录>：导出每窗音频 / 精解文字 / 几何，供外部对齐器（0.6B ForcedAligner）离线复算。
            if let Ok(dir) = std::env::var("DUMP455") {
                let dir = PathBuf::from(dir);
                std::fs::create_dir_all(&dir).unwrap();
                let stem = format!("{name}_{i:02}");
                write_wav16_455(&dir.join(format!("{stem}.wav")), &waudio);
                std::fs::write(dir.join(format!("{stem}.txt")), &text).unwrap();
                if i == 0 {
                    if let Some(r) = reference.as_deref() {
                        std::fs::write(dir.join(format!("{name}.ref.txt")), r).unwrap();
                    }
                }
                let ov = if i >= 1 {
                    wsamples[0] as f32 / waudio.len().max(1) as f32
                } else {
                    0.0
                };
                use std::io::Write as _;
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(dir.join("meta.jsonl"))
                    .unwrap();
                writeln!(
                    f,
                    "{{\"session\":\"{name}\",\"i\":{i},\"stem\":\"{stem}\",\"win_abs\":{win_abs},\"t_split\":{t_split},\"ov\":{ov},\"ctc_rate\":{mrate}}}"
                )
                .unwrap();
            }
            if !text.trim().is_empty() {
                let before = timed.len();
                let tchars: Vec<char> = text.chars().collect();
                let low = mrate < 0.6;
                let new_chars: Vec<(char, f32)> = if low && i >= 1 {
                    // 兜底：时间搬不过去 ⇒ 上一窗全留，新窗按「非重叠占比」取尾部字数（371 同款比例估算）。
                    let ov = wsamples[0] as f32 / waudio.len().max(1) as f32;
                    let keep = ((tchars.len() as f32) * (1.0 - ov)).round() as usize;
                    let from = tchars.len().saturating_sub(keep);
                    tchars[from..]
                        .iter()
                        .map(|&c| (c, t_split + 0.001))
                        .collect()
                } else {
                    timed.retain(|&(_, t)| t < t_split);
                    let mut nc: Vec<(char, f32)> = tchars
                        .iter()
                        .copied()
                        .zip(times.iter().map(|t| win_abs + t))
                        .filter(|&(_, t)| t >= t_split)
                        .collect();
                    // 接缝抖动去重：分界两侧同一个内容字、时间差 <0.25s ⇒ 只留一个（前后窗时间误差几十 ms）。
                    let last = timed
                        .iter()
                        .rev()
                        .find_map(|&(c, t)| content_char_455(c).map(|k| (k, t)));
                    let first = nc.iter().position(|&(c, _)| content_char_455(c).is_some());
                    if let (Some((lc, lt)), Some(fi)) = (last, first) {
                        let (fc, ft) = nc[fi];
                        if content_char_455(fc) == Some(lc) && (ft - lt).abs() < 0.25 {
                            nc.drain(..=fi);
                        }
                    }
                    nc
                };
                let dropped = before - timed.len();
                seams.push(format!(
                    "- 窗{i} 分界 {t_split:.2}s 匹配率 {mrate:.2}：上窗留 {} 字 / 删 {dropped} 字，新窗接 {} 字",
                    timed.len(),
                    new_chars.len()
                ));
                timed.extend(new_chars);
            }
            // ---- cur：现行接缝 ----
            let out = push_timed_456(
                &mut reflow,
                &acc,
                &cum_end,
                i,
                span_start,
                i + 1,
                &wsamples,
                &waudio,
                text,
                split,
                true,
            );
            if let Some(f) = out.last().filter(|f| !f.is_empty()) {
                cur_full = f.clone();
            }
        }
        let (fc, lw) = reflow.finish();
        let cur_text = {
            let t = format!("{fc}{lw}");
            if t.trim().is_empty() {
                cur_full
            } else {
                t
            }
        };
        let timed_text: String = timed.iter().map(|&(c, _)| c).collect();
        if has_repeat8(&cur_text) {
            rep_cur += 1;
        }
        if has_repeat8(&timed_text) {
            rep_timed += 1;
        }
        let (cc, ct) = match reference.as_deref() {
            Some(r) => {
                let (a, b) = (cer(&cur_text, r), cer(&timed_text, r));
                cer_cur += a;
                cer_timed += b;
                cer_n += 1;
                (format!("{a:.4}"), format!("{b:.4}"))
            }
            None => ("-".into(), "-".into()),
        };
        report.push_str(&format!(
            "\n## {name}（{} 窗）\n- **cur** CER={cc} | {cur_text}\n- **timed** CER={ct} | {timed_text}\n- 相同：{}\n{}\n",
            pieces.len(),
            cur_text == timed_text,
            seams.join("\n")
        ));
        println!("[455] {name} done");
    }
    let summary = format!(
        "\n## 汇总\n\n| 拼法 | 平均 CER（{cer_n} 段有参考） | ≥8 字重复录音 |\n|---|---:|---:|\n| cur（现行） | {:.4} | {rep_cur} |\n| timed（按时间） | {:.4} | {rep_timed} |\n\nCTC 共 {windows} 窗、总耗时 {:.1}s（{:.0}ms/窗，单线程）；平均匹配率 {:.2}，匹配率 <0.6 的窗 {low_match} 个。\n",
        cer_cur / cer_n.max(1) as f32,
        cer_timed / cer_n.max(1) as f32,
        ctc_ms / 1000.0,
        ctc_ms / windows.max(1) as f64,
        rate_sum / windows.max(1) as f32
    );
    print!("{summary}");
    report.push_str(&summary);
    let dir = root.join("collab/evidence/455");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("replay455.md"), report).unwrap();
}

/// 16-bit PCM 单声道 16kHz WAV（DUMP455 导出用）。
fn write_wav16_455(path: &Path, s: &[f32]) {
    let data: Vec<u8> = s
        .iter()
        .flat_map(|&x| ((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
        .collect();
    let mut b: Vec<u8> = Vec::with_capacity(44 + data.len());
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&(RATE as u32).to_le_bytes());
    b.extend_from_slice(&(RATE as u32 * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(data.len() as u32).to_le_bytes());
    b.extend_from_slice(&data);
    std::fs::write(path, b).unwrap();
}

// ========================================================================
// PIPE-SPEED-457 / MEM-TRIM-457 · 管线提速 + 省内存回放（只测不改生产）
//
// Gavin 2026-09-29「把这一次加对齐模型多出来的 0.25 秒能够消化掉。甚至能够更快一些，包括预览和精确的
// 最终结果能够更快一些」「显存和内存的占用……不能影响功能和性能」。
// 几何 / 注入同 replay452 生产配置（full = 草稿 + 词库），同一窗口解两遍：
//   A 遍 `LAS_NO_PREFIX_CACHE=1`（每窗从头预填充）   B 遍 前缀 KV 复用（生产 ③）
// ⇒ 逐窗精解文字 A vs B（③ 预期只剩 GPU 自身浮动，见 `poc457_prefix_determinism`）+ 解码耗时对比。
// 同一份 B 遍文字 × 对齐时间，三种拼接：
//   old = 带时间直接拼（456）    all = 先无时间拼、再按对齐时间重拼（457 ①，须与 old 全同）
//   new = 同 all 但最后一窗不对齐（457 ②，生产）
// 统计：终稿与 old 不同的录音数、CER、≥8 字重复、重拼改动回灌的窗数（预览约 0.25s 后微调）、对齐耗时。
// 运行：cargo test --bin feiyin-ime replay457 -- --ignored --nocapture
// ========================================================================

#[test]
#[ignore = "PIPE-SPEED-457：cargo test --bin feiyin-ime replay457 -- --ignored --nocapture"]
fn replay457_pipeline() {
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let root = manifest_dir();
    let models = root.join("models");
    let mut acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
    acc.attach_aligner(&models);
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
    let (mut n_win, mut n_diff_ab) = (0usize, 0usize);
    let mut ms_ab = [0f64; 2];
    let (mut align_ms, mut align_n) = (0f64, 0usize);
    let (mut refine_n, mut refine_changed) = (0usize, 0usize);
    let (mut sess, mut all_ne_old, mut new_ne_old) = (0usize, 0usize, 0usize);
    let (mut cer_old, mut cer_new, mut cer_n) = (0f32, 0f32, 0usize);
    let (mut rep_old, mut rep_new) = (0usize, 0usize);
    let mut report = String::from("# PIPE-SPEED-457 / MEM-TRIM-457 回放\n");
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
        let (cum_end, wins) = windows_457(&audio, &run_stream);
        let mut texts: [Vec<String>; 2] = [Vec::new(), Vec::new()];
        for pass in 0..2 {
            if pass == 0 {
                std::env::set_var("LAS_NO_PREFIX_CACHE", "1");
            } else {
                std::env::remove_var("LAS_NO_PREFIX_CACHE");
            }
            for (i, w) in wins.iter().enumerate() {
                let t0 = Instant::now();
                let text = decode_457(&acc, w, i, terms.as_deref());
                ms_ab[pass] += t0.elapsed().as_secs_f64() * 1000.0;
                texts[pass].push(text);
            }
        }
        for (a, b) in texts[0].iter().zip(&texts[1]) {
            n_win += 1;
            if a != b {
                n_diff_ab += 1;
                report.push_str(&format!("- ③ 差异 {name}：A=「{a}」 B=「{b}」\n"));
            }
        }
        let n = wins.len();
        let (mut r_old, mut r_all, mut r_new) = (
            OrderedReflow::new(),
            OrderedReflow::new(),
            OrderedReflow::new(),
        );
        for (i, w) in wins.iter().enumerate() {
            let text = texts[1][i].clone();
            let prev_end = if i >= 1 { Some(i) } else { None };
            let (abs, split_abs) = crate::window_abs_geometry(
                &cum_end,
                w.end,
                w.span_start,
                prev_end,
                &w.wsamples,
                w.split,
            );
            let times: Option<Vec<(f32, f32)>> = if text.trim().is_empty() {
                None
            } else {
                let t0 = Instant::now();
                let o = acc.align(&w.waudio, &text);
                align_ms += t0.elapsed().as_secs_f64() * 1000.0;
                align_n += 1;
                o.map(|o| {
                    o.char_times
                        .into_iter()
                        .map(|(a, b)| (abs + a, abs + b))
                        .collect()
                })
            };
            let args = |t: Option<Vec<(f32, f32)>>| {
                (
                    i,
                    w.span_start,
                    w.end,
                    w.wsamples.clone(),
                    text.clone(),
                    t,
                    abs,
                    split_abs,
                )
            };
            let (a0, a1, a2, a3, a4, a5, a6, a7) = args(times.clone());
            r_old.push_window_timed(a0, a1, a2, a3, a4, a5, a6, a7);
            let (a0, a1, a2, a3, a4, a5, a6, a7) = args(None);
            let shown = r_all.push_window_timed(a0, a1, a2, a3, a4, a5, a6, a7);
            if let Some(t) = &times {
                if let Some(f) = r_all.refine_times(i, t.clone()) {
                    refine_n += 1;
                    if shown.last() != Some(&f) {
                        refine_changed += 1;
                        report.push_str(&format!(
                            "- ① 重拼改动 {name} #{i}：「{}」→「{}」\n",
                            tail_chars(shown.last().map_or("", |s| s.as_str()), 16),
                            tail_chars(&f, 16)
                        ));
                    }
                }
            }
            let (a0, a1, a2, a3, a4, a5, a6, a7) = args(None);
            r_new.push_window_timed(a0, a1, a2, a3, a4, a5, a6, a7);
            if i + 1 < n {
                if let Some(t) = &times {
                    r_new.refine_times(i, t.clone());
                }
            }
        }
        let fin = |r: OrderedReflow| {
            let (c, l) = r.finish();
            format!("{c}{l}")
        };
        let (f_old, f_all, f_new) = (fin(r_old), fin(r_all), fin(r_new));
        sess += 1;
        if f_all != f_old {
            all_ne_old += 1;
            report.push_str(&format!(
                "- 🔴 ① 终稿不等 {name}\n  - old：{f_old}\n  - all：{f_all}\n"
            ));
        }
        if f_new != f_old {
            new_ne_old += 1;
            report.push_str(&format!(
                "- ② 末窗不对齐终稿不同 {name}：old 尾「{}」 / new 尾「{}」\n",
                tail_chars(&f_old, 24),
                tail_chars(&f_new, 24)
            ));
        }
        rep_old += has_repeat8(&f_old) as usize;
        rep_new += has_repeat8(&f_new) as usize;
        if let Some(r) = read_ref_for(&name) {
            cer_old += cer(&f_old, &r);
            cer_new += cer(&f_new, &r);
            cer_n += 1;
        }
        println!("[457] {name} done ({n} 窗)");
    }
    let summary = format!(
        "\n## 汇总\n\n\
         - ③ 前缀 KV 复用：{n_win} 窗文字不同 {n_diff_ab}；解码总耗时 从头预填充 {:.1}s → 复用 {:.1}s（{:+.1}%）\n\
         - 对齐：{align_n} 次，均 {:.0}ms/窗\n\
         - ① 先回灌后重拼：{sess} 段终稿与 456 不同 {all_ne_old}（须 0）；重拼 {refine_n} 窗，其中回灌文字有改动 {refine_changed}\n\
         - ② 末窗不对齐：{sess} 段终稿与 456 不同 {new_ne_old}；CER（{cer_n} 段有参考）456 {:.4} / 457 {:.4}；≥8 字重复 456 {rep_old} / 457 {rep_new}\n",
        ms_ab[0] / 1000.0,
        ms_ab[1] / 1000.0,
        (ms_ab[1] / ms_ab[0].max(1.0) - 1.0) * 100.0,
        align_ms / align_n.max(1) as f64,
        cer_old / cer_n.max(1) as f32,
        cer_new / cer_n.max(1) as f32,
    );
    print!("{summary}");
    report.push_str(&summary);
    let out = root.join(format!(
        "collab/evidence/457/replay457{}.md",
        std::env::var("REPLAY_TAG").unwrap_or_default()
    ));
    let _ = std::fs::create_dir_all(out.parent().unwrap());
    std::fs::write(&out, report).unwrap();
    println!("[457] 报告：{}", out.display());
    assert_eq!(all_ne_old, 0, "① 先回灌后重拼的终稿须与 456 逐位相同");
    // ③ 不设硬断言：同一窗不复用前缀连解 3 次也会出 2 种结果（GPU 自身浮动，poc457_prefix_determinism 取证），
    // 差异窗数只报告、逐条列出供人工核对。
}

/// 457 回放窗口（几何同 replay452：dispatch 切片 + plan_gap_cuts + 前片后缀组窗）。
struct Win457 {
    span_start: usize,
    end: usize,
    wsamples: Vec<usize>,
    waudio: Vec<f32>,
    split: Option<f32>,
    stream_text: String,
}

/// 返回 `(各片累计结束样本, 窗口)`。
fn windows_457(audio: &[f32], run_stream: &dyn Fn(&[f32]) -> String) -> (Vec<usize>, Vec<Win457>) {
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    for (s, e) in dispatch_slices(audio) {
        pieces.extend(vad::plan_gap_cuts(audio, s, e));
    }
    let cum_end: Vec<usize> = pieces
        .iter()
        .scan(0usize, |acc, &(ps, pe)| {
            *acc += pe - ps;
            Some(*acc)
        })
        .collect();
    let mut wins = Vec::new();
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
        wins.push(Win457 {
            span_start,
            end: i + 1,
            wsamples,
            waudio,
            split,
            stream_text,
        });
    }
    (cum_end, wins)
}

/// 生产配置解一窗（草稿 + 词库，走生产 `transcribe_acc_ctx`）。
fn decode_457(acc: &AccEngine, w: &Win457, seq: usize, terms: Option<&str>) -> String {
    let inject = CtxInject {
        terms,
        avg_chars_per_sec: None,
        speech_ranges: None,
        streaming_nonempty: !w.stream_text.trim().is_empty(),
        new_slice_from: w.wsamples[..w.wsamples.len() - 1].iter().sum(),
        assist: crate::transcription::llama_asr::DecodeAssist::draft(Some(w.stream_text.as_str())),
    };
    transcribe_acc_ctx(acc, &w.waudio, ChineseScript::Simplified, seq, inject)
        .map(|(t, _, _)| t)
        .unwrap_or_default()
}

/// 本进程显存 / 内存（MB）：`(专用显存, 共享显存, 工作集)`（Windows 性能计数器；取不到 ⇒ 0）。
fn proc_mem_mb_457() -> (f64, f64, f64) {
    let pid = std::process::id();
    let cmd = format!(
        "$d=0;$s=0;$c=Get-Counter -Counter '\\GPU Process Memory(pid_{pid}_*)\\Dedicated Usage','\\GPU Process Memory(pid_{pid}_*)\\Shared Usage' -ErrorAction SilentlyContinue;\
         foreach($x in $c.CounterSamples){{if($x.Path -like '*dedicated*'){{$d+=$x.CookedValue}}else{{$s+=$x.CookedValue}}}};\
         $w=(Get-Process -Id {pid}).WorkingSet64;\"$d $s $w\""
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &cmd])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let v: Vec<f64> = out
        .split_whitespace()
        .filter_map(|x| x.parse::<f64>().ok())
        .map(|b| b / 1048576.0)
        .collect();
    (
        v.first().copied().unwrap_or(0.0),
        v.get(1).copied().unwrap_or(0.0),
        v.get(2).copied().unwrap_or(0.0),
    )
}

/// 字级编辑距离（逐窗与 bf16 参考比）。
fn char_edit_457(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().filter(|c| !c.is_whitespace()).collect();
    let b: Vec<char> = b.chars().filter(|c| !c.is_whitespace()).collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1)
                .min(cur[j - 1] + 1)
                .min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
        }
        prev = cur;
    }
    prev[b.len()]
}

// ========================================================================
// QUANT-AB-457 · 1.7B 精解模型量化对比（只测不改生产）
//
// Gavin 2026-09-29「我看一下 Q6K 的量化是不是比 Q8 的量化精确度要高？……是不是用 Q6K 的量化加上 FP16 的
// 编解码器？」「如果……确实测试下来，精确度和节省内存方面确实是比之前有改进……要确定要采纳的」。
// 仅 1.7B 精解模型（0.6B 对齐模型不动）。组合：主模型 {Q8_0, Q6_K} × 编码器 {Q8_0, f16}，参考 = bf16 + f16。
// 每组同一批窗口、同一生产解码路径（草稿 + 词库），指标：
//   ① 逐窗与 bf16 参考的字差（编辑距离 / 参考字数）与完全一致窗数  ② 5 段人工参考全文 CER（接缝按 457 生产拼接）
//   ③ 解码总耗时  ④ 加载 + 解码后本进程专用 / 共享显存、工作集增量
// 环境：`QUANT457_DIR` = 含 `Qwen3-ASR-1.7B-bf16.gguf` / `Qwen3-ASR-1.7B-Q6_K.gguf` 的目录；
//       `QUANT457_COMBOS` 可选（逗号分隔组名，缺省全跑）。
// 运行：cargo test --bin feiyin-ime replay457_quant -- --ignored --nocapture
// ========================================================================

#[test]
#[ignore = "QUANT-AB-457：cargo test --bin feiyin-ime replay457_quant -- --ignored --nocapture"]
fn replay457_quant() {
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let root = manifest_dir();
    let models = root.join("models");
    let qdir = PathBuf::from(std::env::var("QUANT457_DIR").expect("QUANT457_DIR"));
    let mdir = models.join(crate::transcription::llama_asr::LLAMA_ASR_MODEL_SUBDIR);
    let q8 = mdir.join("Qwen3-ASR-1.7B-Q8_0.gguf");
    let mm_q8 = mdir.join("mmproj-Qwen3-ASR-1.7B-Q8_0.gguf");
    let mm_f16 = mdir.join("mmproj-Qwen3-ASR-1.7B-f16.gguf");
    let q6 = qdir.join("Qwen3-ASR-1.7B-Q6_K.gguf");
    let bf16 = qdir.join("Qwen3-ASR-1.7B-bf16.gguf");
    let combos: Vec<(&str, &PathBuf, &PathBuf)> = vec![
        ("ref-bf16+f16", &bf16, &mm_f16),
        ("Q8+Q8(现行)", &q8, &mm_q8),
        ("Q8+f16", &q8, &mm_f16),
        ("Q6K+Q8", &q6, &mm_q8),
        ("Q6K+f16", &q6, &mm_f16),
    ];
    let only: Option<Vec<String>> = std::env::var("QUANT457_COMBOS")
        .ok()
        .map(|v| v.split(',').map(|x| x.trim().to_string()).collect());
    let aligner =
        crate::transcription::llama_asr::LlamaAligner::load(&models).expect("对齐模型须在位");
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
    // 录音 → 窗口（各组共用）。
    let mut sessions: Vec<(String, Vec<usize>, Vec<Win457>, Option<String>)> = Vec::new();
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
        let (cum_end, wins) = windows_457(&audio, &run_stream);
        let r = read_ref_for(&name);
        sessions.push((name, cum_end, wins, r));
    }
    let n_windows: usize = sessions.iter().map(|s| s.2.len()).sum();
    println!("[Q457] {} 段 {} 窗", sessions.len(), n_windows);
    let mut report = format!(
        "# QUANT-AB-457 · 1.7B 量化对比（{} 段 {} 窗，生产解码配置：草稿 + 词库）\n\n",
        sessions.len(),
        n_windows
    );
    let mut ref_texts: Option<Vec<Vec<String>>> = None;
    let mut rows = String::new();
    for (label, model, mmproj) in combos {
        if only.as_ref().is_some_and(|o| !o.iter().any(|x| x == label)) && !label.starts_with("ref")
        {
            continue;
        }
        if !model.is_file() || !mmproj.is_file() {
            println!("[Q457] 跳过 {label}：文件缺失");
            continue;
        }
        let (d0, s0, w0) = proc_mem_mb_457();
        let t_load = Instant::now();
        let acc = AccEngine::load_files(model, mmproj, true).expect("引擎加载");
        let load_ms = t_load.elapsed().as_millis();
        let mut texts: Vec<Vec<String>> = Vec::new();
        let mut ms = 0f64;
        for (_, _, wins, _) in &sessions {
            let mut t = Vec::new();
            for (i, w) in wins.iter().enumerate() {
                let t0 = Instant::now();
                t.push(decode_457(&acc, w, i, terms.as_deref()));
                ms += t0.elapsed().as_secs_f64() * 1000.0;
            }
            texts.push(t);
        }
        let (d1, s1, w1) = proc_mem_mb_457();
        let device = acc.device().to_string();
        drop(acc);
        // ② 全文 CER（接缝按 457 生产：除最后一窗外先无时间拼再按对齐时间重拼）。
        let (mut cer_sum, mut cer_n) = (0f32, 0usize);
        for ((name, cum_end, wins, reference), tx) in sessions.iter().zip(&texts) {
            let Some(reference) = reference else {
                continue;
            };
            let mut r = OrderedReflow::new();
            let n = wins.len();
            for (i, w) in wins.iter().enumerate() {
                let text = tx[i].clone();
                let prev_end = if i >= 1 { Some(i) } else { None };
                let (abs, split_abs) = crate::window_abs_geometry(
                    cum_end,
                    w.end,
                    w.span_start,
                    prev_end,
                    &w.wsamples,
                    w.split,
                );
                let times = if text.trim().is_empty() || i + 1 == n {
                    None
                } else {
                    aligner.align(&w.waudio, &text).ok().flatten().map(|o| {
                        o.char_times
                            .into_iter()
                            .map(|(a, b)| (abs + a, abs + b))
                            .collect::<Vec<_>>()
                    })
                };
                r.push_window_timed(
                    i,
                    w.span_start,
                    w.end,
                    w.wsamples.clone(),
                    text,
                    None,
                    abs,
                    split_abs,
                );
                if let Some(t) = times {
                    r.refine_times(i, t);
                }
            }
            let (c, l) = r.finish();
            let full = format!("{c}{l}");
            let c = cer(&full, reference);
            cer_sum += c;
            cer_n += 1;
            report.push_str(&format!("- {label} · {name} CER={c:.4}\n"));
        }
        // ① 逐窗与 bf16 参考。
        let (mut same, mut edits, mut ref_chars) = (0usize, 0usize, 0usize);
        if let Some(rt) = &ref_texts {
            for (a, b) in texts.iter().flatten().zip(rt.iter().flatten()) {
                let e = char_edit_457(a, b);
                edits += e;
                ref_chars += b.chars().filter(|c| !c.is_whitespace()).count();
                same += usize::from(e == 0);
            }
        }
        let row = format!(
            "| {label} | {} | {} | {:.4} | {:.1}s | {:.0} | {:.0} | {:.0} | {load_ms}ms | {device} |\n",
            if ref_texts.is_some() { format!("{same}/{n_windows}") } else { "参考".into() },
            if ref_texts.is_some() { format!("{:.2}%", edits as f64 * 100.0 / ref_chars.max(1) as f64) } else { "-".into() },
            cer_sum / cer_n.max(1) as f32,
            ms / 1000.0,
            d1 - d0,
            s1 - s0,
            w1 - w0,
        );
        print!("{row}");
        rows.push_str(&row);
        if label.starts_with("ref") {
            ref_texts = Some(texts);
        }
    }
    report.push_str("\n## 汇总\n\n| 组合 | 与 bf16 逐窗全同 | 与 bf16 字差 | 5 段 CER | 解码总耗时 | 专用显存 MB | 共享显存 MB | 工作集 MB | 加载 | 设备 |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---|\n");
    report.push_str(&rows);
    let out = root.join(format!(
        "collab/evidence/457/quant457{}.md",
        std::env::var("REPLAY_TAG").unwrap_or_default()
    ));
    let _ = std::fs::create_dir_all(out.parent().unwrap());
    std::fs::write(&out, report).unwrap();
    println!("[Q457] 报告：{}", out.display());
}

fn tail_chars(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}

/// PIPE-SPEED-457 ③ 取证：同一窗「从头预填充」与「前缀 KV 复用」各解 3 次，看差异来自复用还是 GPU 本身的浮动。
/// 运行：P457_SESSION=<录音名> cargo test --bin feiyin-ime poc457_prefix -- --ignored --nocapture
#[test]
#[ignore = "PIPE-SPEED-457 ③ 取证：--ignored 运行"]
fn poc457_prefix_determinism() {
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
    let target = std::env::var("P457_SESSION").unwrap_or_else(|_| "session-20260927-221031".into());
    for wav in collect_wavs(&root) {
        if wav.file_stem().and_then(|s| s.to_str()) != Some(target.as_str()) {
            continue;
        }
        let Some((audio, _)) = read_wav(&wav) else {
            continue;
        };
        let (_, wins) = windows_457(&audio, &run_stream);
        for (i, w) in wins.iter().enumerate() {
            std::env::set_var("LAS_NO_PREFIX_CACHE", "1");
            let a: Vec<String> = (0..3)
                .map(|_| decode_457(&acc, w, i, terms.as_deref()))
                .collect();
            std::env::remove_var("LAS_NO_PREFIX_CACHE");
            let b: Vec<String> = (0..3)
                .map(|_| decode_457(&acc, w, i, terms.as_deref()))
                .collect();
            let mut all = a.clone();
            all.extend(b.clone());
            all.sort();
            all.dedup();
            println!(
                "#{i} 从头 {}种 / 复用 {}种 / 合计 {}种{}",
                {
                    let mut x = a.clone();
                    x.sort();
                    x.dedup();
                    x.len()
                },
                {
                    let mut x = b.clone();
                    x.sort();
                    x.dedup();
                    x.len()
                },
                all.len(),
                if all.len() > 1 {
                    format!("：{all:?}")
                } else {
                    String::new()
                }
            );
        }
    }
}

// ========================================================================
// LLAMA-TUNE-458 · 1.7B 精解 + 0.6B 对齐器调用参数评估（只测不改生产）
//
// Gavin 2026-09-30「你再仔细分析评估一下现在用 llama.cpp 调用 1.7B 精确模型的调用方式，看有没有可以再优化的参数，
// 调用规格方面有没有可以再优化的地方和空间」「另外对齐模型也是，也评估分析一下」。
// 同 replay457 几何 / 生产解码路径（草稿 + 词库），每个候选跑满 120 窗；调试开关（shim 环境变量，生产不设）：
//   LAS_SPLIT_ENC（编码拆分计时，各变体都开）/ LAS_NO_DRAFT_LANG（458 起语种草稿默认开） / LAS_DRAFT_MAX / LAS_UBATCH / LAS_MTMD_THREADS / LAS_ALIGN_UBATCH
// 1.7B 变体：base / lang（草稿带语种前缀）/ lang+prev（再加上一窗精解全文）/ +draft16 / +ub256 / +ub512 / +mtmd8
// 对齐器变体：整窗（现行）/ 剪静音后对齐再映射回窗内时间 / ubatch 256
// 运行：cargo test --bin feiyin-ime replay458 -- --ignored --nocapture
// ========================================================================

static DECODE_LOG_458: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct DecodeLogger458;

impl log::Log for DecodeLogger458 {
    fn enabled(&self, meta: &log::Metadata<'_>) -> bool {
        meta.level() <= log::Level::Debug
    }
    fn log(&self, record: &log::Record<'_>) {
        let m = record.args().to_string();
        if m.contains("[ACC-452] decode:") {
            DECODE_LOG_458.lock().unwrap().push(m);
        }
    }
    fn flush(&self) {}
}

/// `[ACC-452] decode:` 行 ⇒ (prefill, enc, gen_ms, n_gen, 草稿接受, 草稿提出)。
fn parse_decode_458(line: &str) -> (f64, f64, f64, f64, f64, f64) {
    let (mut pre, mut enc, mut gms, mut ng, mut da, mut dp) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let toks: Vec<&str> = line.split_whitespace().collect();
    for (i, t) in toks.iter().enumerate() {
        if let Some(v) = t.strip_prefix("prefill=") {
            pre = v.split("ms").next().unwrap_or("0").parse().unwrap_or(0.0);
            if let Some(n) = toks.get(i + 1) {
                enc = n.trim_end_matches(')').parse().unwrap_or(0.0);
            }
        } else if let Some(v) = t.strip_prefix("gen=") {
            if let Some(x) = v.strip_suffix("ms") {
                gms = x.parse().unwrap_or(0.0);
            } else {
                ng = v.parse().unwrap_or(0.0);
            }
        } else if let Some(v) = t.strip_prefix("draft=") {
            let mut it = v.split('/');
            da = it.next().unwrap_or("0").parse().unwrap_or(0.0);
            dp = it.next().unwrap_or("0").parse().unwrap_or(0.0);
        }
    }
    (pre, enc, gms, ng, da, dp)
}

/// 剪静音拼接（同生产 `trim_to_speech`：两侧各扩 pad、合并）后的各段 `(窗内起点, 长度)`，用于把对齐时间映射回窗内。
fn trim_spans_458(ranges: &[(usize, usize)], pad: usize, len: usize) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = ranges
        .iter()
        .map(|&(s, e)| (s.saturating_sub(pad), e.saturating_add(pad).min(len)))
        .filter(|&(s, e)| e > s)
        .collect();
    spans.sort_by_key(|&(s, _)| s);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in spans {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 {
                last.1 = last.1.max(e);
                continue;
            }
        }
        merged.push((s, e));
    }
    merged.into_iter().map(|(s, e)| (s, e - s)).collect()
}

fn map_trim_time_458(spans: &[(usize, usize)], t_secs: f32) -> f32 {
    let mut k = (t_secs.max(0.0) * RATE as f32) as usize;
    for &(st, ln) in spans {
        if k < ln {
            return (st + k) as f32 / RATE as f32;
        }
        k -= ln;
    }
    spans
        .last()
        .map_or(t_secs, |&(st, ln)| (st + ln) as f32 / RATE as f32)
}

#[test]
#[ignore = "LLAMA-TUNE-458：cargo test --bin feiyin-ime replay458 -- --ignored --nocapture"]
fn replay458_tune() {
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let _ = log::set_boxed_logger(Box::new(DecodeLogger458));
    log::set_max_level(log::LevelFilter::Debug);
    let root = manifest_dir();
    let models = root.join("models");
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
    let mut sessions: Vec<(String, Vec<usize>, Vec<Win457>, Option<String>)> = Vec::new();
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
        let (cum_end, wins) = windows_457(&audio, &run_stream);
        let r = read_ref_for(&name);
        sessions.push((name, cum_end, wins, r));
    }
    let n_windows: usize = sessions.iter().map(|s| s.2.len()).sum();
    let mut report = format!(
        "# LLAMA-TUNE-458 · 调用参数评估（{} 段 {} 窗）\n\n",
        sessions.len(),
        n_windows
    );
    // ---------------- 1.7B ----------------
    // (名称, 环境变量, 是否加上一窗精解全文作草稿, 是否需重建引擎)
    let variants: Vec<(&str, Vec<(&str, &str)>, bool)> = vec![
        (
            "base（458 前：无语种草稿）",
            vec![("LAS_NO_DRAFT_LANG", "1")],
            false,
        ),
        (
            "无前缀复用（复核 457）",
            vec![("LAS_NO_PREFIX_CACHE", "1"), ("LAS_NO_DRAFT_LANG", "1")],
            false,
        ),
        ("lang", vec![], false),
        ("lang+prev", vec![], true),
        ("lang+prev+draft16", vec![("LAS_DRAFT_MAX", "16")], true),
        ("lang+prev+ub256", vec![("LAS_UBATCH", "256")], true),
        ("lang+prev+ub512", vec![("LAS_UBATCH", "512")], true),
        ("lang+prev+mtmd8", vec![("LAS_MTMD_THREADS", "8")], true),
        (
            "base（复测，量 GPU 自身浮动）",
            vec![("LAS_NO_DRAFT_LANG", "1")],
            false,
        ),
    ];
    let all_keys = [
        "LAS_NO_DRAFT_LANG",
        "LAS_DRAFT_MAX",
        "LAS_UBATCH",
        "LAS_MTMD_THREADS",
        "LAS_NO_PREFIX_CACHE",
    ];
    std::env::set_var("LAS_SPLIT_ENC", "1");
    let mut base_texts: Option<Vec<Vec<String>>> = None;
    let mut rows = String::new();
    for (label, envs, with_prev) in &variants {
        for k in all_keys {
            std::env::remove_var(k);
        }
        for (k, v) in envs {
            std::env::set_var(k, v);
        }
        let (d0, s0, w0) = proc_mem_mb_457();
        let acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
        // 预热：首段首窗解一次不计。
        if let Some((_, _, wins, _)) = sessions.first() {
            if let Some(w) = wins.first() {
                let _ = decode_457(&acc, w, 0, terms.as_deref());
            }
        }
        DECODE_LOG_458.lock().unwrap().clear();
        let t0 = Instant::now();
        let mut texts: Vec<Vec<String>> = Vec::new();
        for (_, _, wins, _) in &sessions {
            let mut tx: Vec<String> = Vec::new();
            for (i, w) in wins.iter().enumerate() {
                let draft = if *with_prev && i >= 1 {
                    format!("{}{}", w.stream_text, tx[i - 1])
                } else {
                    w.stream_text.clone()
                };
                let inject = CtxInject {
                    terms: terms.as_deref(),
                    avg_chars_per_sec: None,
                    speech_ranges: None,
                    streaming_nonempty: !w.stream_text.trim().is_empty(),
                    new_slice_from: w.wsamples[..w.wsamples.len() - 1].iter().sum(),
                    assist: crate::transcription::llama_asr::DecodeAssist::draft(Some(
                        draft.as_str(),
                    )),
                };
                tx.push(
                    transcribe_acc_ctx(&acc, &w.waudio, ChineseScript::Simplified, i, inject)
                        .map(|(t, _, _)| t)
                        .unwrap_or_default(),
                );
            }
            texts.push(tx);
        }
        let wall = t0.elapsed().as_secs_f64();
        let (d1, s1, w1) = proc_mem_mb_457();
        drop(acc);
        let lines = std::mem::take(&mut *DECODE_LOG_458.lock().unwrap());
        let (mut pre, mut enc, mut gms, mut ng, mut da, mut dp) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for l in &lines {
            let v = parse_decode_458(l);
            pre += v.0;
            enc += v.1;
            gms += v.2;
            ng += v.3;
            da += v.4;
            dp += v.5;
        }
        let steps = ng - da;
        let diff = match &base_texts {
            Some(b) => {
                let (mut nw, mut ed) = (0usize, 0usize);
                for (a, b) in texts.iter().flatten().zip(b.iter().flatten()) {
                    if a != b {
                        nw += 1;
                        ed += char_edit_457(a, b);
                    }
                }
                format!("{nw} 窗 / {ed} 字")
            }
            None => "基线".into(),
        };
        let row = format!(
            "| {label} | {:.1}s | {:.1}s | {:.1}s | {:.1}s | {:.0} | {:.0} | {:.0}% | {:.1}ms | {diff} | {:.0} / {:.0} / {:.0} |\n",
            wall,
            pre / 1000.0,
            enc / 1000.0,
            gms / 1000.0,
            ng,
            steps,
            da * 100.0 / dp.max(1.0),
            gms / steps.max(1.0),
            d1 - d0,
            s1 - s0,
            w1 - w0,
        );
        print!("{row}");
        rows.push_str(&row);
        if base_texts.is_none() {
            base_texts = Some(texts);
        }
    }
    for k in all_keys {
        std::env::remove_var(k);
    }
    std::env::remove_var("LAS_SPLIT_ENC");
    report.push_str("## 1.7B 精解\n\n| 变体 | 总耗时 | 预填充 | 其中编码 | 生成 | 生成 token | 生成步数 | 草稿接受率 | 每步 | 与现行文字不同窗数 | 显存专用 / 共享 / 工作集 MB |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|\n");
    report.push_str(&rows);

    // ---------------- 0.6B 对齐器 ----------------
    let texts = base_texts.expect("基线文字");
    let pad = (vad::LOCALRT_TRIM_PAD_SECS * RATE as f32) as usize;
    let mut arows = String::new();
    let mut finals: Vec<Vec<String>> = Vec::new();
    for (label, ub, trim) in [
        ("整窗（现行）", "512", false),
        ("剪静音后对齐", "512", true),
        ("整窗 ubatch256", "256", false),
    ] {
        std::env::set_var("LAS_ALIGN_UBATCH", ub);
        let (d0, s0, w0) = proc_mem_mb_457();
        let aligner =
            crate::transcription::llama_asr::LlamaAligner::load(&models).expect("对齐模型须在位");
        // 预热一窗。
        if let (Some((_, _, wins, _)), Some(tx)) = (sessions.first(), texts.first()) {
            if let (Some(w), Some(t)) = (wins.first(), tx.first()) {
                if !t.trim().is_empty() {
                    let _ = aligner.align(&w.waudio, t);
                }
            }
        }
        let (mut ms, mut enc, mut bb, mut n, mut audio_s) = (0.0, 0.0, 0.0, 0usize, 0.0);
        let (mut cer_sum, mut cer_n) = (0f32, 0usize);
        let mut fin_v: Vec<String> = Vec::new();
        for ((name, cum_end, wins, reference), tx) in sessions.iter().zip(&texts) {
            let nw = wins.len();
            let mut r = OrderedReflow::new();
            for (i, w) in wins.iter().enumerate() {
                let text = tx[i].clone();
                let prev_end = if i >= 1 { Some(i) } else { None };
                let (abs, split_abs) = crate::window_abs_geometry(
                    cum_end,
                    w.end,
                    w.span_start,
                    prev_end,
                    &w.wsamples,
                    w.split,
                );
                let times = if text.trim().is_empty() || i + 1 == nw {
                    None
                } else {
                    let (audio, spans) = if trim {
                        let rs = self_vad_ranges(&w.waudio).unwrap_or_default();
                        if rs.is_empty() {
                            (w.waudio.clone(), vec![(0, w.waudio.len())])
                        } else {
                            (
                                trim_to_speech(&w.waudio, &rs, pad),
                                trim_spans_458(&rs, pad, w.waudio.len()),
                            )
                        }
                    } else {
                        (w.waudio.clone(), vec![(0, w.waudio.len())])
                    };
                    audio_s += audio.len() as f64 / RATE as f64;
                    let o = aligner.align(&audio, &text).ok().flatten();
                    if let Some(o) = &o {
                        ms += o.ms[0];
                        enc += o.ms[1];
                        bb += o.ms[2];
                        n += 1;
                    }
                    o.map(|o| {
                        o.char_times
                            .into_iter()
                            .map(|(a, b)| {
                                (
                                    abs + map_trim_time_458(&spans, a),
                                    abs + map_trim_time_458(&spans, b),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                };
                r.push_window_timed(
                    i,
                    w.span_start,
                    w.end,
                    w.wsamples.clone(),
                    text,
                    None,
                    abs,
                    split_abs,
                );
                if let Some(t) = times {
                    r.refine_times(i, t);
                }
            }
            let (c, l) = r.finish();
            let full = format!("{c}{l}");
            if let Some(rf) = reference {
                cer_sum += cer(&full, rf);
                cer_n += 1;
            }
            let _ = name;
            fin_v.push(full);
        }
        let (d1, s1, w1) = proc_mem_mb_457();
        drop(aligner);
        let diff = match finals.first() {
            Some(b) => {
                let d = fin_v.iter().zip(b).filter(|(a, b)| a != b).count();
                for ((a, b), (name, ..)) in fin_v.iter().zip(b).zip(&sessions) {
                    if a != b {
                        report.push_str(&format!(
                            "- 对齐「{label}」终稿不同 {name}：\n  - 现行：{b}\n  - 本变体：{a}\n"
                        ));
                    }
                }
                d.to_string()
            }
            None => "基线".into(),
        };
        let row = format!(
            "| {label} | {n} | {:.1}s | {:.0}ms | {:.0}ms | {:.0}ms | {:.4} | {diff} | {:.0} / {:.0} / {:.0} |\n",
            audio_s,
            ms / n.max(1) as f64,
            enc / n.max(1) as f64,
            bb / n.max(1) as f64,
            cer_sum / cer_n.max(1) as f32,
            d1 - d0,
            s1 - s0,
            w1 - w0,
        );
        print!("{row}");
        arows.push_str(&row);
        finals.push(fin_v);
    }
    std::env::remove_var("LAS_ALIGN_UBATCH");
    report.push_str("\n## 0.6B 对齐器（接缝按 457 生产拼接）\n\n| 变体 | 对齐窗数 | 送入音频总长 | 每窗均耗时 | 编码 | 主干 | 5 段 CER | 终稿与现行不同段数 | 显存专用 / 共享 / 工作集 MB |\n|---|---:|---:|---:|---:|---:|---:|---:|---|\n");
    report.push_str(&arows);
    let out = root.join(format!(
        "collab/evidence/458/replay458{}.md",
        std::env::var("REPLAY_TAG").unwrap_or_default()
    ));
    let _ = std::fs::create_dir_all(out.parent().unwrap());
    std::fs::write(&out, report).unwrap();
    println!("[458] 报告：{}", out.display());
}

/// LLAMA-TUNE-458 复核：现行 vs「草稿带语种前缀 + 上一窗精解全文」的终稿质量（5 段人工参考 CER、≥8 字重复），
/// 接缝按 457 生产拼接；另计每窗「整窗重跑 VAD」（`self_vad_ranges`）耗时。
/// 运行：cargo test --bin feiyin-ime poc458_quality -- --ignored --nocapture
#[test]
#[ignore = "LLAMA-TUNE-458 复核：--ignored 运行"]
fn poc458_quality() {
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let root = manifest_dir();
    let models = root.join("models");
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
    let mut sessions: Vec<(String, Vec<usize>, Vec<Win457>, Option<String>)> = Vec::new();
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
        let (cum_end, wins) = windows_457(&audio, &run_stream);
        let r = read_ref_for(&name);
        sessions.push((name, cum_end, wins, r));
    }
    // 整窗重跑 VAD 的耗时（CPU，与 GPU 解码无关）。
    let (mut vad_ms, mut vad_n, mut vad_audio) = (0f64, 0usize, 0f64);
    for (_, _, wins, _) in &sessions {
        for w in wins {
            let t0 = Instant::now();
            let _ = self_vad_ranges(&w.waudio);
            vad_ms += t0.elapsed().as_secs_f64() * 1000.0;
            vad_n += 1;
            vad_audio += w.waudio.len() as f64 / RATE as f64;
        }
    }
    println!(
        "[458] 整窗重跑 VAD：{vad_n} 窗，均 {:.1}ms/窗（窗均 {:.1}s，{:.1}ms/秒音频）",
        vad_ms / vad_n.max(1) as f64,
        vad_audio / vad_n.max(1) as f64,
        vad_ms / vad_audio.max(1.0)
    );
    let acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
    let mut all: Vec<Vec<Vec<String>>> = Vec::new();
    for with_lang_prev in [false, true] {
        if with_lang_prev {
            std::env::remove_var("LAS_NO_DRAFT_LANG");
        } else {
            std::env::set_var("LAS_NO_DRAFT_LANG", "1");
        }
        let mut texts: Vec<Vec<String>> = Vec::new();
        for (_, _, wins, _) in &sessions {
            let mut tx: Vec<String> = Vec::new();
            for (i, w) in wins.iter().enumerate() {
                let draft = if with_lang_prev && i >= 1 {
                    format!("{}{}", w.stream_text, tx[i - 1])
                } else {
                    w.stream_text.clone()
                };
                let inject = CtxInject {
                    terms: terms.as_deref(),
                    avg_chars_per_sec: None,
                    speech_ranges: None,
                    streaming_nonempty: !w.stream_text.trim().is_empty(),
                    new_slice_from: w.wsamples[..w.wsamples.len() - 1].iter().sum(),
                    assist: crate::transcription::llama_asr::DecodeAssist::draft(Some(
                        draft.as_str(),
                    )),
                };
                tx.push(
                    transcribe_acc_ctx(&acc, &w.waudio, ChineseScript::Simplified, i, inject)
                        .map(|(t, _, _)| t)
                        .unwrap_or_default(),
                );
            }
            texts.push(tx);
        }
        all.push(texts);
    }
    std::env::remove_var("LAS_NO_DRAFT_LANG");
    drop(acc);
    let aligner =
        crate::transcription::llama_asr::LlamaAligner::load(&models).expect("对齐模型须在位");
    let mut report = String::from("# LLAMA-TUNE-458 复核：终稿质量\n\n");
    let mut finals: Vec<Vec<String>> = Vec::new();
    for (vi, label) in ["现行", "语种前缀+上一窗草稿"].iter().enumerate() {
        let (mut cer_sum, mut cer_n, mut rep) = (0f32, 0usize, 0usize);
        let mut fv = Vec::new();
        for ((name, cum_end, wins, reference), tx) in sessions.iter().zip(&all[vi]) {
            let nw = wins.len();
            let mut r = OrderedReflow::new();
            for (i, w) in wins.iter().enumerate() {
                let text = tx[i].clone();
                let prev_end = if i >= 1 { Some(i) } else { None };
                let (abs, split_abs) = crate::window_abs_geometry(
                    cum_end,
                    w.end,
                    w.span_start,
                    prev_end,
                    &w.wsamples,
                    w.split,
                );
                let times = if text.trim().is_empty() || i + 1 == nw {
                    None
                } else {
                    aligner.align(&w.waudio, &text).ok().flatten().map(|o| {
                        o.char_times
                            .into_iter()
                            .map(|(a, b)| (abs + a, abs + b))
                            .collect::<Vec<_>>()
                    })
                };
                r.push_window_timed(
                    i,
                    w.span_start,
                    w.end,
                    w.wsamples.clone(),
                    text,
                    None,
                    abs,
                    split_abs,
                );
                if let Some(t) = times {
                    r.refine_times(i, t);
                }
            }
            let (c, l) = r.finish();
            let full = format!("{c}{l}");
            rep += has_repeat8(&full) as usize;
            if let Some(rf) = reference {
                let c = cer(&full, rf);
                cer_sum += c;
                cer_n += 1;
                report.push_str(&format!("- {label} · {name} CER={c:.4}\n"));
            }
            fv.push(full);
        }
        let line = format!(
            "- **{label}**：5 段 CER {:.4}；≥8 字重复 {rep} 段\n",
            cer_sum / cer_n.max(1) as f32
        );
        print!("{line}");
        report.push_str(&line);
        finals.push(fv);
    }
    for ((a, b), (name, ..)) in finals[0].iter().zip(&finals[1]).zip(&sessions) {
        if a != b {
            report.push_str(&format!(
                "\n### 终稿不同：{name}\n- 现行：{a}\n- 新草稿：{b}\n"
            ));
        }
    }
    let out = root.join("collab/evidence/458/quality458.md");
    let _ = std::fs::create_dir_all(out.parent().unwrap());
    std::fs::write(&out, report).unwrap();
    println!("[458] 报告：{}", out.display());
}

/// PHANTOM-406-459 ②：兜底窗（406 判精解不可信 / 精解空 ⇒ 用流式文本）也做强制对齐，接缝按时间拼接，
/// 对照原「兜底窗无时间 ⇒ 下一窗 371 比例估算」。生产解码路径 + 同一 406 判据；兜底文本取整窗流式原文（回放无标点服务）。
/// 运行：cargo test --bin feiyin-ime poc459_fallback_align -- --ignored --nocapture
#[test]
#[ignore = "PHANTOM-406-459 ②：--ignored 运行"]
fn poc459_fallback_align() {
    crate::transcription::speaker::TEST_EXTRACTOR_MISSING.with(|c| c.set(true));
    let root = manifest_dir();
    let models = root.join("models");
    let mut acc = create_qwen3_recognizer(&models).expect("Qwen3 GGUF 须在位");
    acc.attach_aligner(&models);
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
    let (mut n_fb, mut n_fb_aligned, mut n_win) = (0usize, 0usize, 0usize);
    let (mut cer_old, mut cer_new, mut cer_n, mut rep_old, mut rep_new, mut diff) =
        (0f32, 0f32, 0usize, 0usize, 0usize, 0usize);
    let mut report = String::from("# PHANTOM-406-459 ② 兜底窗对齐回放\n\n");
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
        let (cum_end, wins) = windows_457(&audio, &run_stream);
        let n = wins.len();
        let (mut r_old, mut r_new) = (OrderedReflow::new(), OrderedReflow::new());
        for (i, w) in wins.iter().enumerate() {
            n_win += 1;
            let decoded = decode_457(&acc, w, i, terms.as_deref());
            let verdict = crate::transcription::acc_vs_streaming(&decoded, &w.stream_text);
            let is_fallback = decoded.is_empty() || !verdict.accept;
            let text = if is_fallback {
                w.stream_text.clone()
            } else {
                decoded.clone()
            };
            let prev_end = if i >= 1 { Some(i) } else { None };
            let (abs, split_abs) = crate::window_abs_geometry(
                &cum_end,
                w.end,
                w.span_start,
                prev_end,
                &w.wsamples,
                w.split,
            );
            let to_abs = |o: crate::transcription::llama_asr::AlignOut| -> Vec<(f32, f32)> {
                o.char_times
                    .into_iter()
                    .map(|(a, b)| (abs + a, abs + b))
                    .collect()
            };
            if is_fallback && !text.trim().is_empty() {
                n_fb += 1;
                report.push_str(&format!(
                    "- 兜底窗 {name} #{i}：精解「{decoded}」 流式「{}」 retention={:.2}\n",
                    w.stream_text, verdict.retention
                ));
                // 旧：兜底窗无时间。新：兜底文本对齐后带时间拼接。
                r_old.push_window_timed(
                    i,
                    w.span_start,
                    w.end,
                    w.wsamples.clone(),
                    text.clone(),
                    None,
                    abs,
                    split_abs,
                );
                let t = acc.align(&w.waudio, &text).map(to_abs);
                n_fb_aligned += t.is_some() as usize;
                r_new.push_window_timed(
                    i,
                    w.span_start,
                    w.end,
                    w.wsamples.clone(),
                    text,
                    t,
                    abs,
                    split_abs,
                );
            } else {
                // 常规窗：两边同为 457 生产拼接（最后一窗不对齐）。
                let t = if text.trim().is_empty() || i + 1 == n {
                    None
                } else {
                    acc.align(&w.waudio, &text).map(to_abs)
                };
                for r in [&mut r_old, &mut r_new] {
                    r.push_window_timed(
                        i,
                        w.span_start,
                        w.end,
                        w.wsamples.clone(),
                        text.clone(),
                        None,
                        abs,
                        split_abs,
                    );
                    if let Some(t) = &t {
                        r.refine_times(i, t.clone());
                    }
                }
            }
        }
        let fin = |r: OrderedReflow| {
            let (c, l) = r.finish();
            format!("{c}{l}")
        };
        let (fo, fnw) = (fin(r_old), fin(r_new));
        rep_old += has_repeat8(&fo) as usize;
        rep_new += has_repeat8(&fnw) as usize;
        if fo != fnw {
            diff += 1;
            report.push_str(&format!(
                "\n### 终稿不同：{name}\n- 旧（比例估算）：{fo}\n- 新（对齐拼接）：{fnw}\n"
            ));
        }
        if let Some(rf) = read_ref_for(&name) {
            cer_old += cer(&fo, &rf);
            cer_new += cer(&fnw, &rf);
            cer_n += 1;
        }
        println!("[459] {name} done");
    }
    let summary = format!(
        "\n## 汇总\n\n- {n_win} 窗中兜底窗 {n_fb}（对齐成功 {n_fb_aligned}）；终稿不同 {diff} 段\n- 5 段 CER 旧 {:.4} / 新 {:.4}；≥8 字重复 旧 {rep_old} / 新 {rep_new}\n",
        cer_old / cer_n.max(1) as f32,
        cer_new / cer_n.max(1) as f32
    );
    print!("{summary}");
    report.push_str(&summary);
    let out = root.join("collab/evidence/459/fallback_align459.md");
    let _ = std::fs::create_dir_all(out.parent().unwrap());
    std::fs::write(&out, report).unwrap();
    println!("[459] 报告：{}", out.display());
}

/// KOJA-406-461 取证：日 / 韩语音频走生产精解 + 流式预览 + 406 判据，看精解是否被换成流式乱码。
/// 运行：cargo test --bin feiyin-ime poc461_koja -- --ignored --nocapture
#[test]
#[ignore = "KOJA-406-461：需模型 + 日韩测试音频；--ignored 运行"]
fn poc461_koja_406() {
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
    let mut wavs = vec![
        models.join("sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17/test_wavs/ja.wav"),
        models.join("sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17/test_wavs/ko.wav"),
        models.join("sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17/test_wavs/zh.wav"),
        models.join("sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17/test_wavs/en.wav"),
    ];
    for k in 0..4 {
        wavs.push(models.join(format!("korean-testwavs/{k}.wav")));
    }
    for wav in wavs {
        let Some((audio, rate)) = read_wav(&wav) else {
            println!("[461] 跳过 {}", wav.display());
            continue;
        };
        if rate as usize != RATE {
            println!("[461] 跳过（{rate}Hz）{}", wav.display());
            continue;
        }
        let stream_text = run_stream(&audio);
        let w = Win457 {
            span_start: 0,
            end: 1,
            wsamples: vec![audio.len()],
            waudio: audio,
            split: None,
            stream_text: stream_text.clone(),
        };
        let decoded = decode_457(&acc, &w, 0, terms.as_deref());
        let v = crate::transcription::acc_vs_streaming(&decoded, &stream_text);
        let used = if decoded.is_empty() || !v.accept {
            "流式（兜底）"
        } else {
            "精解"
        };
        println!(
            "[461] {}\n   精解：{decoded}\n   流式：{stream_text}\n   406：retention={:.2} len_ratio={:.2} accept={} ⇒ 该窗用 {used}",
            wav.file_name().and_then(|s| s.to_str()).unwrap_or("?"),
            v.retention,
            v.len_ratio,
            v.accept
        );
    }
}
