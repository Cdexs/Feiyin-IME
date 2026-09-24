// POC-SPEAKER-VERIFY-404 · 声纹判别模型选型实测（纯 `#[cfg(test)]`，不碰生产逻辑）
//
// 背景：BUILD-399 端测「背景人声被近场门放行」——本人段峰值 0.0258/0.0088/0.0162/0.0130，
// 背景人声 0.0079/0.0072，只差约 1dB，靠音量从原理上分不开 ⇒ 改为「按声纹认人」。
//
// 🔴 产品约束（Gavin）：「采集的是使用人的声纹（不是只为我定制）」
//   ⇒ 评测**不能只拿 Gavin 当注册人**。本 PoC 对**全部说话人轮流当「使用人」**（每人注册、其余人当背景），
//      池化所有 genuine/imposter 分数定**一个全局阈值**，再逐人统计 FAR/FRR 并给出**最差的人**。
//      Gavin 仅是说话人之一，不作为调参对象。
//
// 运行：
//   cargo test --bin feiyin-ime -- --ignored --nocapture poc_speaker_404_models
//   cargo test --bin feiyin-ime -- --ignored --nocapture poc_speaker_404_overlay
//   cargo test --bin feiyin-ime -- --ignored --nocapture poc_speaker_404_timing
//
// 数据：模型/外部说话人样本在 `models/speaker-404-scratch/`（gitignore，不入仓库、不进 Publish）；
//       Gavin 样本在 `collab/research/audio-real-gavin/processed/`。

use sherpa_onnx::{SpeakerEmbeddingExtractor, SpeakerEmbeddingExtractorConfig, Wave};
use std::path::{Path, PathBuf};
use std::time::Instant;

const RATE: i32 = 16_000;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn scratch() -> PathBuf {
    repo().join("models").join("speaker-404-scratch")
}
fn gavin_dir() -> PathBuf {
    repo()
        .join("collab")
        .join("research")
        .join("audio-real-gavin")
        .join("processed")
}

fn load(path: &Path) -> Option<Vec<f32>> {
    let w = Wave::read(path.to_str()?)?;
    if w.sample_rate() != RATE {
        return None; // 本 PoC 数据均 16k
    }
    Some(w.samples().to_vec())
}

fn l2(v: &[f32]) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    v.iter().map(|x| x / n).collect()
}
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}
fn mean(v: &[f32]) -> f32 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f32>() / v.len() as f32
    }
}
fn centroid(embs: &[Vec<f32>]) -> Vec<f32> {
    let d = embs[0].len();
    let mut acc = vec![0f32; d];
    for e in embs {
        for (i, x) in e.iter().enumerate() {
            acc[i] += *x;
        }
    }
    l2(&acc)
}

fn embed(ex: &SpeakerEmbeddingExtractor, s: &[f32]) -> Option<Vec<f32>> {
    let st = ex.create_stream()?;
    st.accept_waveform(RATE, s);
    st.input_finished();
    if !ex.is_ready(&st) {
        return None;
    }
    ex.compute(&st).map(|v| l2(&v))
}

fn create(model_file: &str, threads: i32) -> Option<SpeakerEmbeddingExtractor> {
    let p = scratch().join(model_file);
    if !p.is_file() {
        return None;
    }
    SpeakerEmbeddingExtractor::create(&SpeakerEmbeddingExtractorConfig {
        model: Some(p.to_string_lossy().to_string()),
        num_threads: threads,
        debug: false,
        provider: Some("cpu".to_string()),
    })
}

fn windows(s: &[f32], len: usize) -> Vec<Vec<f32>> {
    s.chunks(len)
        .filter(|c| c.len() == len)
        .map(|c| c.to_vec())
        .collect()
}

/// EER + 阈值。genuine = 使用人本人，imposter = 他人。FAR=他人被判本人（误收），FRR=本人被判他人（误拒）。
fn eer(genuine: &[f32], imposter: &[f32]) -> Option<(f32, f32)> {
    if genuine.is_empty() || imposter.is_empty() {
        return None;
    }
    let mut ts: Vec<f32> = genuine.iter().chain(imposter.iter()).copied().collect();
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.dedup();
    let ng = genuine.len() as f32;
    let ni = imposter.len() as f32;
    let mut best = (f32::INFINITY, 0f32, 0f32);
    for &t in &ts {
        let far = imposter.iter().filter(|&&s| s >= t).count() as f32 / ni;
        let frr = genuine.iter().filter(|&&s| s < t).count() as f32 / ng;
        if (far - frr).abs() < best.0 {
            best = ((far - frr).abs(), t, far + frr);
        }
    }
    Some((best.2 / 2.0, best.1))
}

const MODELS: [(&str, &str); 6] = [
    (
        "CAM++ zh-cn",
        "3dspeaker_speech_campplus_sv_zh-cn_16k-common.onnx",
    ),
    (
        "ERes2NetV2 zh-cn",
        "3dspeaker_speech_eres2netv2_sv_zh-cn_16k-common.onnx",
    ),
    (
        "ERes2Net-large zh-cn",
        "3dspeaker_speech_eres2net_large_sv_zh-cn_3dspeaker_16k.onnx",
    ),
    (
        "WeSpeaker ResNet34 cn-celeb",
        "wespeaker_zh_cnceleb_resnet34.onnx",
    ),
    (
        "WeSpeaker ResNet293 en-vox",
        "wespeaker_en_voxceleb_resnet293_LM.onnx",
    ),
    ("NeMo TitaNet-large en", "nemo_en_titanet_large.onnx"),
];

const BUCKETS: [(&str, usize); 4] = [
    ("<1s", 9_600),
    ("1-2s", 24_000),
    ("2-5s", 48_000),
    ("5-10s", 112_000),
];

const SIL: usize = 4_000; // 0.25s 间隔，拼接同一说话人多个文件

fn concat(files: &[PathBuf]) -> Vec<f32> {
    let mut out = Vec::new();
    for f in files {
        if let Some(mut s) = load(f) {
            if !out.is_empty() {
                out.extend(std::iter::repeat(0f32).take(SIL));
            }
            out.append(&mut s);
        }
    }
    out
}

/// 全部说话人：Gavin + 7 名外部（CN: fangjun/leijun/liudehua；kr0~kr3 假定 4 名不同说话人）。
fn speakers() -> Vec<(String, Vec<f32>)> {
    let g = gavin_dir();
    let sc = scratch();
    let mut out = vec![(
        "Gavin".to_string(),
        concat(&[
            g.join("para1.wav"),
            g.join("para2.wav"),
            g.join("para3.wav"),
            g.join("vad_segs/seg0.wav"),
            g.join("vad_segs/seg1.wav"),
            g.join("full.wav"),
        ]),
    )];
    for name in ["fangjun", "leijun", "liudehua"] {
        let mut fs: Vec<PathBuf> = std::fs::read_dir(&sc)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().and_then(|x| x.to_str()) == Some("wav")
                    && p.file_name()
                        .map(|n| n.to_string_lossy().starts_with(name))
                        .unwrap_or(false)
            })
            .collect();
        fs.sort();
        out.push((name.to_string(), concat(&fs)));
    }
    let kr = repo().join("models").join("korean-testwavs");
    for i in 0..4 {
        let f = kr.join(format!("{i}.wav"));
        out.push((format!("kr{i}"), concat(&[f])));
    }
    out
}

const MAX_WIN_PER_SPK: usize = 40;
const MAX_PER_OTHER: usize = 24;

struct Rr {
    pooled_eer: f32,
    thr: f32,
    per: Vec<(String, usize, usize, f32, f32, f32, f32)>, // name,nGen,nImp,meanGen,meanImp,frr,far
    hc_thr: f32,                                          // 高置信剔除阈值：池化 FRR<=1% 的分位
    hc_far: f32,                                          // 该阈值下被剔除的背景比例
    hc_worst_frr: f32,                                    // 该阈值下逐人最差 FRR
}

/// FRR<=target 的阈值（genuine 的 target 分位；越小越保守）。
fn highconf_thr(gen: &[f32], target: f32) -> f32 {
    if gen.is_empty() {
        return f32::NAN;
    }
    let mut g = gen.to_vec();
    g.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((g.len() as f32 * target).ceil() as usize).min(g.len() - 1);
    g[idx]
}

/// 跨说话人轮流注册：每人注册（enroll_n 段平均 = 声纹），其余人为背景；池化定全局阈值。
fn round_robin(
    ex: &SpeakerEmbeddingExtractor,
    spks: &[(String, Vec<f32>)],
    len: usize,
    enroll_n: usize,
) -> Rr {
    round_robin_cap(ex, spks, len, enroll_n, MAX_WIN_PER_SPK, MAX_PER_OTHER)
}

/// 同上，但窗口/背景上限可调（404B 语料说话人 utt 多，需另定上限）。
fn round_robin_cap(
    ex: &SpeakerEmbeddingExtractor,
    spks: &[(String, Vec<f32>)],
    len: usize,
    enroll_n: usize,
    win_cap: usize,
    per_other: usize,
) -> Rr {
    // 每个说话人窗口嵌入只算一次（缓存）
    let cache: Vec<(String, Vec<Vec<f32>>)> = spks
        .iter()
        .map(|(n, a)| {
            let ws: Vec<Vec<f32>> = windows(a, len).into_iter().take(win_cap).collect();
            let embs: Vec<Vec<f32>> = ws.iter().filter_map(|w| embed(ex, w)).collect();
            (n.clone(), embs)
        })
        .collect();

    let mut all_gen: Vec<f32> = Vec::new();
    let mut all_imp: Vec<f32> = Vec::new();
    let mut raw: Vec<(String, Vec<f32>, Vec<f32>)> = Vec::new(); // name, gen scores, imp scores
    for i in 0..cache.len() {
        let embs = &cache[i].1;
        if embs.len() < 2 {
            continue;
        }
        let e = enroll_n.min(embs.len() - 1);
        let cent = centroid(&embs[..e]);
        let gen: Vec<f32> = embs[e..].iter().map(|x| dot(&cent, x)).collect();
        let mut imp: Vec<f32> = Vec::new();
        for j in 0..cache.len() {
            if i == j {
                continue;
            }
            for x in cache[j].1.iter().take(per_other) {
                imp.push(dot(&cent, x));
            }
        }
        all_gen.extend_from_slice(&gen);
        all_imp.extend_from_slice(&imp);
        raw.push((cache[i].0.clone(), gen, imp));
    }
    let Some((pooled_eer, thr)) = eer(&all_gen, &all_imp) else {
        return Rr {
            pooled_eer: f32::NAN,
            thr: f32::NAN,
            per: Vec::new(),
            hc_thr: f32::NAN,
            hc_far: f32::NAN,
            hc_worst_frr: f32::NAN,
        };
    };
    // 高置信剔除阈值：池化 FRR<=1%；report 该阈值下背景剔除率与该阈值下逐人最差 FRR
    let hc_thr = highconf_thr(&all_gen, 0.01);
    let hc_far = all_imp.iter().filter(|&&s| s >= hc_thr).count() as f32 / all_imp.len() as f32;
    let hc_worst_frr = raw
        .iter()
        .map(|(_, gen, _)| {
            if gen.is_empty() {
                0f32
            } else {
                gen.iter().filter(|&&s| s < hc_thr).count() as f32 / gen.len() as f32
            }
        })
        .fold(0f32, f32::max);
    let per = raw
        .into_iter()
        .map(|(n, gen, imp)| {
            let frr = if gen.is_empty() {
                f32::NAN
            } else {
                gen.iter().filter(|&&s| s < thr).count() as f32 / gen.len() as f32
            };
            let far = if imp.is_empty() {
                f32::NAN
            } else {
                imp.iter().filter(|&&s| s >= thr).count() as f32 / imp.len() as f32
            };
            (n, gen.len(), imp.len(), mean(&gen), mean(&imp), frr, far)
        })
        .collect();
    Rr {
        pooled_eer,
        thr,
        per,
        hc_thr,
        hc_far,
        hc_worst_frr,
    }
}

#[test]
#[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_speaker_404_models"]
fn poc_speaker_404_models() {
    let spks = speakers();
    println!("\n===== POC-SPEAKER-404 v2 · 跨说话人轮流注册 =====");
    println!(
        "说话人 {} 名（每人轮流当「使用人」，其余为背景）：",
        spks.len()
    );
    for (n, a) in &spks {
        println!("   {n:8} {:.1}s", a.len() as f32 / RATE as f32);
    }

    println!("\n===== 模型加载（大小 / load / dim）=====");
    for (label, file) in MODELS {
        let p = scratch().join(file);
        let mb = std::fs::metadata(&p)
            .map(|m| m.len() as f64 / 1048576.0)
            .unwrap_or(0.0);
        let t = Instant::now();
        match create(file, 1) {
            Some(ex) => println!(
                "{label}\t{mb:.1}MB\tdim={}\tload={:.0}ms",
                ex.dim(),
                t.elapsed().as_secs_f64() * 1000.0
            ),
            None => println!("{label}\tMISSING"),
        }
    }

    println!("\n===== 池化 EER（全局阈值，每人轮流注册）=====");
    println!("model\tenroll\tbucket\t#spk\t#gen/#imp\tEER\tthr\tworstFRR\tworstFAR\thcThr(FRR1%)\thcFAR(剔除背景)\thcWorstFRR");
    for (label, file) in MODELS {
        let Some(ex) = create(file, 1) else { continue };
        for (em, en) in [("5seg", 5usize), ("1seg", 1usize)] {
            for (bl, len) in BUCKETS {
                let r = round_robin(&ex, &spks, len, en);
                if r.per.is_empty() {
                    println!("{label}\t{em}\t{bl}\tn/a");
                    continue;
                }
                let worst_frr = r
                    .per
                    .iter()
                    .map(|p| p.5)
                    .filter(|x| x.is_finite())
                    .fold(0f32, f32::max);
                let worst_far = r
                    .per
                    .iter()
                    .map(|p| p.6)
                    .filter(|x| x.is_finite())
                    .fold(0f32, f32::max);
                let ng: usize = r.per.iter().map(|p| p.1).sum();
                let ni: usize = r.per.iter().map(|p| p.2).sum();
                println!(
                    "{label}\t{em}\t{bl}\t{}\t{}/{}\t{:.2}%\t{:.3}\t{:.1}%\t{:.1}%\t{:.3}\t{:.1}%\t{:.1}%",
                    r.per.len(),
                    ng,
                    ni,
                    r.pooled_eer * 100.0,
                    r.thr,
                    worst_frr * 100.0,
                    worst_far * 100.0,
                    r.hc_thr,
                    r.hc_far * 100.0,
                    r.hc_worst_frr * 100.0
                );
            }
        }
    }

    // 逐人明细（推荐候选 CAM++ / ERes2NetV2，2-5s，5seg）
    for (label, file) in [
        ("CAM++ zh-cn", MODELS[0].1),
        ("ERes2NetV2 zh-cn", MODELS[1].1),
    ] {
        let Some(ex) = create(file, 1) else { continue };
        let r = round_robin(&ex, &spks, 48_000, 5);
        println!(
            "\n===== 逐人明细 · {label} · 2-5s · 5seg · 全局thr={:.3} =====",
            r.thr
        );
        println!("speaker\tnGen\tnImp\tmeanGen\tmeanImp\tFRR@thr\tFAR@thr");
        for (n, ng, ni, mg, mi, frr, far) in &r.per {
            println!(
                "{n}\t{ng}\t{ni}\t{mg:.3}\t{mi:.3}\t{:.1}%\t{:.1}%",
                frr * 100.0,
                far * 100.0
            );
        }
    }
}

#[test]
#[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_speaker_404_overlay"]
fn poc_speaker_404_overlay() {
    let spks = speakers();
    let len = 48_000; // 3s
                      // 以每个说话人为「使用人」，叠加其余人声（等量级 0.85 / 1.0），看 score 变化
    println!("\n===== 两人叠加（使用人 + 他人，3s；ratio=他人 RMS/使用人 RMS）=====");
    for (label, file) in MODELS {
        let Some(ex) = create(file, 1) else { continue };
        let mut clean_all = Vec::new();
        let mut mix085 = Vec::new();
        let mut mix100 = Vec::new();
        for i in 0..spks.len() {
            let embs: Vec<Vec<f32>> = windows(&spks[i].1, len)
                .into_iter()
                .take(6)
                .filter_map(|w| embed(&ex, &w))
                .collect();
            if embs.len() < 2 {
                continue;
            }
            let cent = centroid(&embs[..1]);
            for (k, g) in spks[i].1.chunks(len).take(4).enumerate() {
                if g.len() != len {
                    continue;
                }
                let j = (i + 1 + k) % spks.len();
                let Some(imp) = windows(&spks[j].1, len).into_iter().next() else {
                    continue;
                };
                if let Some(e) = embed(&ex, g) {
                    clean_all.push(dot(&cent, &e));
                }
                for (ratio, sink) in [(0.85f32, &mut mix085), (1.0f32, &mut mix100)] {
                    let scale = ratio * rms(g) / rms(&imp).max(1e-9);
                    let mixed: Vec<f32> = g
                        .iter()
                        .zip(imp.iter())
                        .map(|(a, b)| a + b * scale)
                        .collect();
                    if let Some(e) = embed(&ex, &mixed) {
                        sink.push(dot(&cent, &e));
                    }
                }
            }
        }
        println!(
            "{label}: clean={:.3}  +他人0.85={:.3}  +他人1.0={:.3}  (n={}/{})",
            mean(&clean_all),
            mean(&mix085),
            mean(&mix100),
            clean_all.len(),
            mix085.len()
        );
    }
}

#[test]
#[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_speaker_404_timing"]
fn poc_speaker_404_timing() {
    let gtest = load(&gavin_dir().join("full.wav")).expect("full.wav");
    println!("\n===== 单段提取耗时（ms，5 次均值；本机 16 逻辑核）=====");
    println!("model\tthreads\tdur=1s\tdur=5s\tdur=10s");
    for (label, file) in MODELS {
        for threads in [1, 2, 4] {
            let Some(ex) = create(file, threads) else {
                continue;
            };
            let mut cells = Vec::new();
            for dur in [1usize, 5, 10] {
                let len = dur * RATE as usize;
                let w = gtest
                    .get(0..len)
                    .map(|s| s.to_vec())
                    .unwrap_or_else(|| gtest.clone());
                let _ = embed(&ex, &w);
                let t = Instant::now();
                for _ in 0..5 {
                    let _ = embed(&ex, &w);
                }
                cells.push(t.elapsed().as_secs_f64() * 1000.0 / 5.0);
            }
            println!(
                "{label}\t{threads}\t{:.0}\t{:.0}\t{:.0}",
                cells[0], cells[1], cells[2]
            );
        }
    }
}

// ===========================================================================
// 404B：四语语料（zh/en/ko 已建；ja 无公开可用「带说话人标注的自然人」语料 ⇒ 缺口）
// ===========================================================================

const MODELS_404B: [(&str, &str); 6] = [
    (
        "CAM++ zh-cn(中)",
        "3dspeaker_speech_campplus_sv_zh-cn_16k-common.onnx",
    ),
    (
        "ERes2NetV2 zh-cn(中)",
        "3dspeaker_speech_eres2netv2_sv_zh-cn_16k-common.onnx",
    ),
    (
        "CAM++ zh_en(中英)",
        "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx",
    ),
    (
        "CAM++ en-vox(英)",
        "3dspeaker_speech_campplus_sv_en_voxceleb_16k.onnx",
    ),
    (
        "WeSpeaker R34 en-vox(英)",
        "wespeaker_en_voxceleb_resnet34.onnx",
    ),
    (
        "ERes2Net en-vox(英)",
        "3dspeaker_speech_eres2net_sv_en_voxceleb_16k.onnx",
    ),
];

/// 读 `<scratch>/corpus/<lang>/<spk>/*.wav`，按说话人拼接（上限 cap_secs）。
fn corpus_lang(lang: &str, cap_secs: usize, max_spk: usize) -> Vec<(String, Vec<f32>)> {
    let d = scratch().join("corpus").join(lang);
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&d) else {
        return out;
    };
    let mut spks: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    spks.sort();
    for sp in spks.into_iter().take(max_spk) {
        let name = sp.file_name().unwrap().to_string_lossy().to_string();
        let mut wavs: Vec<PathBuf> = std::fs::read_dir(&sp)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("wav"))
            .collect();
        wavs.sort();
        let cap = cap_secs * RATE as usize;
        let mut audio = Vec::new();
        for w in wavs {
            if audio.len() >= cap {
                break;
            }
            if let Some(mut s) = load(&w) {
                audio.append(&mut s);
            }
        }
        if audio.len() >= 3 * RATE as usize {
            out.push((name, audio));
        }
    }
    out
}

#[test]
#[ignore = "PoC: cargo test --bin feiyin-ime -- --ignored --nocapture poc_speaker_404b_corpus"]
fn poc_speaker_404b_corpus() {
    println!("\n===== POC-SPEAKER-404B · 四语语料（zh/en/ko；ja 缺口）=====");
    for lang in ["zh", "en", "ko"] {
        let spks = corpus_lang(lang, 120, 10);
        let total: usize = spks.iter().map(|(_, a)| a.len()).sum::<usize>() / RATE as usize;
        println!("\n### 语言 {lang}: {} 说话人，共 {}s", spks.len(), total);
        println!("model\tbucket\t#spk\t#gen/#imp\tEER\tthr\tworstFRR\tworstFAR\thc剔除率");
        for (label, file) in MODELS_404B {
            let Some(ex) = create(file, 1) else {
                println!("{label}\tMISSING");
                continue;
            };
            for (bl, len) in [("1-2s", 24_000usize), ("2-5s", 48_000), ("5-10s", 112_000)] {
                let r = round_robin_cap(&ex, &spks, len, 5, 20, 16);
                if r.per.is_empty() {
                    println!("{label}\t{bl}\tn/a");
                    continue;
                }
                let worst_frr = r
                    .per
                    .iter()
                    .map(|p| p.5)
                    .filter(|x| x.is_finite())
                    .fold(0f32, f32::max);
                let worst_far = r
                    .per
                    .iter()
                    .map(|p| p.6)
                    .filter(|x| x.is_finite())
                    .fold(0f32, f32::max);
                let ng: usize = r.per.iter().map(|p| p.1).sum();
                let ni: usize = r.per.iter().map(|p| p.2).sum();
                println!(
                    "{label}\t{bl}\t{}\t{}/{}\t{:.2}%\t{:.3}\t{:.1}%\t{:.1}%\t{:.1}%",
                    r.per.len(),
                    ng,
                    ni,
                    r.pooled_eer * 100.0,
                    r.thr,
                    worst_frr * 100.0,
                    worst_far * 100.0,
                    (1.0 - r.hc_far) * 100.0
                );
            }
        }
    }
}
