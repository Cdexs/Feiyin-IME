//! POC-GPU-451：GPU 加速离线测试的 CPU 基线（只测不改生产）。
//!
//! Gavin 2026-09-27：「另外你也评估下GPU加速方案呢？」「GPU加速方案要同时支持amd 、intel、nvidia显卡」→「测」。
//! 方法：真录音按 10s 切片 → 生产同款 VAD（`try_new_for_local_trim` + `speech_ranges`）剪静音拼接 →
//! 写出片段 wav（供 llama.cpp Vulkan 用**同一份输入**解码）→ 生产同款 1.7B `decode_accuracy_allow_empty`
//!（无热词、`max_new_tokens_for` 上限，同 DEC-083 现状）计时解码。结果写 `../poc-451/sherpa.tsv`。
//! 运行：`cargo test --bin feiyin-ime poc_gpu_451 -- --ignored --nocapture`

use super::vad::VadSegmenter;
use super::{
    create_qwen3_recognizer, decode_accuracy_allow_empty, max_new_tokens_for, ChineseScript,
};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const RATE: usize = 16000;
const CHUNK_SECS: usize = 10;

const SESSIONS: &[&str] = &[
    "target/release/debug-audio/session-20260927-010234.wav",
    "target/release/debug-audio/session-20260927-010110.wav",
    "target/release/debug-audio/session-20260927-010059.wav",
    "target/release/debug-audio/session-20260927-010048.wav",
    "target/release/debug-audio/session-20260927-005945.wav",
    "collab/evidence/444/session-20260926-224100.wav",
    "collab/evidence/444/session-20260926-224214.wav",
    "collab/evidence/444/session-20260926-232245.wav",
    "collab/evidence/444/session-20260926-230039.wav",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn write_wav16(path: &Path, s: &[f32]) {
    let mut b: Vec<u8> = Vec::with_capacity(44 + s.len() * 2);
    let data_len = (s.len() * 2) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&(RATE as u32).to_le_bytes());
    b.extend_from_slice(&(RATE as u32 * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for &x in s {
        let v = (x.clamp(-1.0, 1.0) * 32767.0) as i16;
        b.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(path, b).expect("write clip wav");
}

#[test]
#[ignore = "POC-GPU-451：需真模型 + 录音；--ignored 运行"]
fn poc_gpu_451_sherpa_baseline() {
    let models = root().join("models");
    let vad = VadSegmenter::try_new_for_local_trim(&models).expect("VAD 须在位");
    let acc = create_qwen3_recognizer(&models).expect("1.7B 须在位");
    let out_dir = root().join("../poc-451");
    let clip_dir = out_dir.join("clips");
    std::fs::create_dir_all(&clip_dir).unwrap();
    let decode = |s: &[f32]| -> String {
        let cap = max_new_tokens_for(s.len() as f32 / RATE as f32);
        decode_accuracy_allow_empty(
            &acc,
            s,
            None,
            ChineseScript::Simplified,
            Some(cap),
            Default::default(),
        )
        .map(|(t, _)| t)
        .unwrap_or_default()
    };
    let mut tsv = String::from("clip\taudio_secs\tms\ttext\n");
    let mut warmed = false;
    for rel in SESSIONS {
        let path = root().join(rel);
        let Some(w) = sherpa_onnx::Wave::read(path.to_str().unwrap()) else {
            println!("skip {rel}");
            continue;
        };
        assert_eq!(w.sample_rate() as usize, RATE);
        let samples = w.samples().to_vec();
        let stem = path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .replace("session-", "");
        for (k, chunk) in samples.chunks(CHUNK_SECS * RATE).enumerate() {
            let used: Vec<f32> = vad
                .speech_ranges(chunk)
                .iter()
                .flat_map(|&(a, b)| chunk[a..b.min(chunk.len())].iter().copied())
                .collect();
            if used.len() < RATE / 2 {
                continue;
            }
            let name = format!("{stem}_{k:02}.wav");
            write_wav16(&clip_dir.join(&name), &used);
            if !warmed {
                let _ = decode(&used);
                warmed = true;
            }
            let t0 = std::time::Instant::now();
            let text = decode(&used);
            let ms = t0.elapsed().as_millis();
            let secs = used.len() as f32 / RATE as f32;
            println!("{name}\t{secs:.2}s\t{ms}ms\t{text}");
            let _ = writeln!(tsv, "{name}\t{secs:.2}\t{ms}\t{text}");
        }
    }
    std::fs::write(out_dir.join("sherpa.tsv"), tsv).unwrap();
}

/// 同一批 `clips/*.wav`，sherpa 首解即指定 `language=Chinese`（对齐 llama 带语种的条件；
/// 生产只在首解异常后重解时才指定，见 426）。写 `../poc-451/sherpa-lang.tsv`。
#[test]
#[ignore = "POC-GPU-451：需真模型 + 先跑 baseline 生成 clips；--ignored 运行"]
fn poc_gpu_451_sherpa_lang_zh() {
    let models = root().join("models");
    let acc = create_qwen3_recognizer(&models).expect("1.7B 须在位");
    let out_dir = root().join("../poc-451");
    let base = std::fs::read_to_string(out_dir.join("sherpa.tsv")).unwrap();
    let mut tsv = String::from("clip\taudio_secs\tms\ttext\n");
    let mut warmed = false;
    for line in base.lines().skip(1) {
        let name = line.split('\t').next().unwrap();
        let p = out_dir.join("clips").join(name);
        let w = sherpa_onnx::Wave::read(p.to_str().unwrap()).expect("clip");
        let s = w.samples().to_vec();
        let cap = max_new_tokens_for(s.len() as f32 / RATE as f32);
        let run = || {
            super::decode_accuracy_allow_empty_lang(
                &acc,
                &s,
                None,
                ChineseScript::Simplified,
                Some(cap),
                Some("Chinese"),
                Default::default(),
            )
            .map(|(t, _)| t)
            .unwrap_or_default()
        };
        if !warmed {
            let _ = run();
            warmed = true;
        }
        let t0 = std::time::Instant::now();
        let text = run();
        let ms = t0.elapsed().as_millis();
        let secs = s.len() as f32 / RATE as f32;
        println!("{name}\t{secs:.2}s\t{ms}ms\t{text}");
        let _ = writeln!(tsv, "{name}\t{secs:.2}\t{ms}\t{text}");
    }
    std::fs::write(out_dir.join("sherpa-lang.tsv"), tsv).unwrap();
}

/// 扩大样本：扫 `target/release/debug-audio` 与 `collab/evidence/**` 全部 `session-*.wav`（按文件名去重），
/// 同 baseline 口径切片剪静音 + sherpa 基线解码。写 `../poc-451/clips-all/` 与 `sherpa-all.tsv`。
#[test]
#[ignore = "POC-GPU-451：需真模型 + 录音；--ignored 运行"]
fn poc_gpu_451_sherpa_baseline_all() {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.file_name().is_some_and(|n| {
                n.to_string_lossy().starts_with("session-") && n.to_string_lossy().ends_with(".wav")
            }) {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root().join("target/release/debug-audio"), &mut files);
    walk(&root().join("collab/evidence"), &mut files);
    files.sort_by_key(|p| p.file_name().unwrap().to_owned());
    files.dedup_by_key(|p| p.file_name().unwrap().to_owned());
    let models = root().join("models");
    let vad = VadSegmenter::try_new_for_local_trim(&models).expect("VAD 须在位");
    let acc = create_qwen3_recognizer(&models).expect("1.7B 须在位");
    let out_dir = root().join("../poc-451");
    let clip_dir = out_dir.join("clips-all");
    std::fs::create_dir_all(&clip_dir).unwrap();
    let mut tsv = String::from("clip\taudio_secs\tms\ttext\n");
    let mut warmed = false;
    for path in &files {
        let Some(w) = sherpa_onnx::Wave::read(path.to_str().unwrap()) else {
            continue;
        };
        if w.sample_rate() as usize != RATE {
            continue;
        }
        let samples = w.samples().to_vec();
        let stem = path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .replace("session-", "");
        for (k, chunk) in samples.chunks(CHUNK_SECS * RATE).enumerate() {
            let used: Vec<f32> = vad
                .speech_ranges(chunk)
                .iter()
                .flat_map(|&(a, b)| chunk[a..b.min(chunk.len())].iter().copied())
                .collect();
            if used.len() < RATE / 2 {
                continue;
            }
            let name = format!("{stem}_{k:02}.wav");
            write_wav16(&clip_dir.join(&name), &used);
            let cap = max_new_tokens_for(used.len() as f32 / RATE as f32);
            let run = || {
                decode_accuracy_allow_empty(
                    &acc,
                    &used,
                    None,
                    ChineseScript::Simplified,
                    Some(cap),
                    Default::default(),
                )
                .map(|(t, _)| t)
                .unwrap_or_default()
            };
            if !warmed {
                let _ = run();
                warmed = true;
            }
            let t0 = std::time::Instant::now();
            let text = run();
            let ms = t0.elapsed().as_millis();
            let _ = writeln!(
                tsv,
                "{name}\t{:.2}\t{ms}\t{text}",
                used.len() as f32 / RATE as f32
            );
        }
    }
    println!("files={}", files.len());
    std::fs::write(out_dir.join("sherpa-all.tsv"), tsv).unwrap();
}

/// ACC-ENGINE-LLAMACPP-452 引擎自检：新引擎（进程内 llama.cpp）解 `clips-all`，
/// 与 llama-server 贪心结果（`llama-g-q8mf16.tsv`）逐字比对；再以 sherpa 基线文字作草稿跑推测解码，
/// 验证「草稿不改变输出」与提速。写 `../poc-451/engine-plain.tsv` / `engine-draft.tsv`。
#[test]
#[ignore = "ACC-452：需 GGUF 模型 + llama.cpp 运行库；--ignored 运行"]
fn acc452_engine_equivalence() {
    use super::llama_asr::LlamaAsr;
    let eng = LlamaAsr::load(&root().join("models")).expect("engine");
    println!("device={}", eng.device());
    let out_dir = root().join("../poc-451");
    let base = std::fs::read_to_string(out_dir.join("sherpa-all.tsv")).unwrap();
    for (label, use_draft) in [("engine-plain", false), ("engine-draft", true)] {
        let mut tsv = String::from("clip\taudio_secs\tms\ttext\n");
        let (mut tot, mut pre, mut gen, mut acc, mut prop) = (0u128, 0f64, 0f64, 0i64, 0i64);
        for (i, line) in base.lines().skip(1).enumerate() {
            let c: Vec<&str> = line.split('\t').collect();
            let w = sherpa_onnx::Wave::read(out_dir.join("clips-all").join(c[0]).to_str().unwrap())
                .unwrap();
            let s = w.samples().to_vec();
            let secs: f32 = c[1].parse().unwrap();
            let cap = (secs * 12.0).ceil() as i32 + 16;
            let draft = if use_draft { c.get(3).copied() } else { None };
            if i == 0 {
                let _ = eng.decode(
                    &s,
                    None,
                    None,
                    Some(cap.max(32)),
                    crate::transcription::llama_asr::DecodeAssist::draft(draft),
                );
            }
            let t0 = std::time::Instant::now();
            let o = eng
                .decode(
                    &s,
                    None,
                    None,
                    Some(cap.max(32)),
                    crate::transcription::llama_asr::DecodeAssist::draft(draft),
                )
                .expect("decode");
            let ms = t0.elapsed().as_millis();
            tot += ms;
            pre += o.stats.prefill_ms;
            gen += o.stats.gen_ms;
            acc += o.stats.n_draft_accepted as i64;
            prop += o.stats.n_draft_proposed as i64;
            let _ = writeln!(
                tsv,
                "{}\t{}\t{ms}\t{}",
                c[0],
                c[1],
                o.raw.replace(['\n', '\t'], " ")
            );
        }
        println!(
            "{label}: total={tot}ms prefill={pre:.0}ms gen={gen:.0}ms draft_accept={acc}/{prop}"
        );
        std::fs::write(out_dir.join(format!("{label}.tsv")), tsv).unwrap();
    }
}

/// POC-ALIGN-455：Performance 档 FunASR Nano CTC（SenseVoice 配置）在 sherpa 1.13.8 下是否填 `timestamps`，
/// 以及每 10s 窗 CPU 耗时 —— 决定「小模型出带时间戳文本 + 全局对齐」路线是否成立（372 立项前必验）。
#[test]
#[ignore = "POC-ALIGN-455：需 FunASR Nano CTC 模型 + 回放录音；--ignored 运行"]
fn poc_align455_ctc_timestamps() {
    let rec = super::create_sensevoice_recognizer(&root().join("models"), "auto").expect("ctc");
    for name in [
        "session-20260925-175022",
        "session-20260925-175535",
        "session-20260925-173634",
    ] {
        let w = sherpa_onnx::Wave::read(
            root()
                .join("collab/evidence/gavin-sessions")
                .join(format!("{name}.wav"))
                .to_str()
                .unwrap(),
        )
        .expect("wav");
        assert_eq!(w.sample_rate(), 16000);
        let all = w.samples();
        for (k, win) in all.chunks(16000 * 10).take(3).enumerate() {
            let s = rec.create_stream();
            s.accept_waveform(16000, win);
            let t0 = std::time::Instant::now();
            rec.decode(&s);
            let ms = t0.elapsed().as_millis();
            let r = s.get_result().expect("result");
            let ts = r.timestamps.clone().unwrap_or_default();
            let pairs: Vec<String> = r
                .tokens
                .iter()
                .zip(ts.iter())
                .map(|(t, x)| format!("{t}@{x:.2}"))
                .collect();
            println!(
                "[455] {name}#{k} secs={:.1} ms={ms} tokens={} ts={} text={}\n      {}",
                win.len() as f32 / 16000.0,
                r.tokens.len(),
                ts.len(),
                r.text,
                pairs.join(" ")
            );
        }
    }
}
