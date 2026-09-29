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
