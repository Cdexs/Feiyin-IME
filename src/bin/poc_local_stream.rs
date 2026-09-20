// POC-LOCAL-STREAM-235 · 本地流式 ASR 可行性 PoC（独立 bin，禁碰生产代码）
//
// 回答两个问题：
//   ① 流式 paraformer（trilingual zh-cantonese-en）纯 CPU 上首字延迟 / RTF 能不能用？
//   ② 2pass 后端 performance vs accuracy，松键后等待时间差多少？
//
// 只读模型、不改任何生产文件。参考 src/bin/poc_funasr_nano.rs 的离线配置写法。
//
// 用法：
//   poc_local_stream <wav...> [--stream-dir D] [--perf-dir D] [--acc-dir D]
//                   [--threads 1,2,4] [--chunk-ms 100] [--enable-endpoint] [--skip-offline]

use sherpa_onnx::{
    OfflineFunASRNanoModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    OfflineSenseVoiceModelConfig, OnlineParaformerModelConfig, OnlineRecognizer,
    OnlineRecognizerConfig, Wave,
};
use std::env;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const USAGE: &str =
    "Usage: poc_local_stream <wav...> [--stream-dir D] [--perf-dir D] [--acc-dir D] \
[--threads 1,2,4] [--chunk-ms 100] [--enable-endpoint] [--skip-offline] \
[--hotwords-file F] [--hotwords-score S] [--decoding-method greedy_search|modified_beam_search] \
[--hr-lexicon F] [--hr-rule-fsts F] [--probe offline-hw-perf|online-hw|hr]";

fn main() {
    let args: Vec<String> = env::args().collect();
    let cfg = match parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            eprintln!("{}", USAGE);
            std::process::exit(1);
        }
    };

    println!("=== POC-LOCAL-STREAM-235 · 本地流式 ASR 可行性 ===");
    println!("stream_dir     : {}", cfg.stream_dir.display());
    println!("perf_dir       : {}", cfg.perf_dir.display());
    println!("acc_dir        : {}", cfg.acc_dir.display());
    println!("threads        : {:?}", cfg.threads);
    println!("chunk_ms       : {}", cfg.chunk_ms);
    println!("enable_endpoint: {}", cfg.enable_endpoint);
    println!();

    let mut wavs: Vec<(String, Wave)> = Vec::new();
    for w in &cfg.wavs {
        match Wave::read(w) {
            Some(wave) => {
                let dur = wave.samples().len() as f64 / wave.sample_rate() as f64;
                println!(
                    "[wav] {} rate={} ch=mono dur={:.2}s",
                    w,
                    wave.sample_rate(),
                    dur
                );
                wavs.push((w.clone(), wave));
            }
            None => eprintln!("[wav] FAILED to read {}", w),
        }
    }
    if wavs.is_empty() {
        eprintln!("no wav loaded");
        std::process::exit(2);
    }

    // ---------------- Probe modes (追加5/6/7), isolated, one config each ----------------
    if let Some(probe) = cfg.probe.clone() {
        run_probe(&cfg, &probe, &wavs);
        println!("\n=== DONE (probe {}) ===", probe);
        return;
    }

    // ---------------- Part 1/2/3: streaming ----------------
    for &threads in &cfg.threads {
        println!("\n########## STREAMING num_threads={} ##########", threads);
        let rec = match build_online(&cfg, threads, None, "greedy_search") {
            Some(r) => r,
            None => {
                eprintln!(
                    "[FATAL] online recognizer create failed (threads={})",
                    threads
                );
                continue;
            }
        };
        for (name, wave) in &wavs {
            let rate = wave.sample_rate() as usize;
            let dur = wave.samples().len() as f64 / rate as f64;
            let chunk = (rate * cfg.chunk_ms as usize) / 1000;
            let base = short_name(name);
            println!(
                "\n--- {} ({:.2}s, chunk={} samples/{}ms) ---",
                name, dur, chunk, cfg.chunk_ms
            );

            // (2) RTF pass: feed as fast as possible, sum decode time
            let rtf = run_stream(
                &rec,
                wave.samples(),
                wave.sample_rate(),
                chunk,
                0,
                false,
                None,
            );
            println!(
                "[RTF] threads={} audio={:.3}s decode_sum={:.3}s wall={:.3}s RTF={:.4}",
                threads,
                dur,
                rtf.decode_secs,
                rtf.wall_secs,
                rtf.decode_secs / dur
            );
            println!("[RTF-final] {}", rtf.final_text);

            // (1) first-char latency + (3) jitter snapshots, paced 100ms
            let mut snap_path = cfg.snapshot_dir.clone();
            let _ = std::fs::create_dir_all(&snap_path);
            if snap_path.as_os_str().is_empty() {
                snap_path = PathBuf::from(".");
            }
            let snap_file = snap_path.join(format!("snap_{}threads_{}.log", threads, base));
            let lat = run_stream(
                &rec,
                wave.samples(),
                wave.sample_rate(),
                chunk,
                cfg.chunk_ms as u64,
                true,
                Some(snap_file.clone()),
            );
            match lat.first_char_ms {
                Some(ms) => println!(
                    "[FIRST-CHAR] threads={} latency={:.0}ms audio_consumed={}ms",
                    threads, ms, lat.first_char_audio_ms
                ),
                None => println!("[FIRST-CHAR] threads={} no text produced", threads),
            }
            println!(
                "[JITTER] threads={} snapshots={} rewrites={} shrinks={} (log: {})",
                threads,
                lat.snapshots.len(),
                lat.rewrites,
                lat.shrinks,
                snap_file.display()
            );
            println!("[STREAM-final] {}", lat.final_text);
        }
    }

    // ---------------- Part 4: offline 2pass backend ----------------
    if !cfg.skip_offline {
        println!("\n########## 2PASS OFFLINE BACKEND ##########");
        let perf = build_offline_sensevoice(&cfg, None, "greedy_search", None);
        let acc = build_offline_funasr_nano(&cfg);
        if perf.is_none() {
            eprintln!("[2PASS] performance recognizer unavailable");
        }
        if acc.is_none() {
            eprintln!("[2PASS] accuracy recognizer unavailable");
        }
        for (name, wave) in &wavs {
            let dur = wave.samples().len() as f64 / wave.sample_rate() as f64;
            println!("\n--- {} ({:.2}s) ---", name, dur);
            if let Some(rec) = &perf {
                let t0 = Instant::now();
                let stream = rec.create_stream();
                stream.accept_waveform(wave.sample_rate(), wave.samples());
                rec.decode(&stream);
                let secs = t0.elapsed().as_secs_f64();
                let text = stream.get_result().map(|r| r.text).unwrap_or_default();
                println!(
                    "[2PASS-performance] {:.3}s RTF={:.4} text={}",
                    secs,
                    secs / dur,
                    text.replace('\n', " ")
                );
            }
            if let Some(rec) = &acc {
                let t0 = Instant::now();
                let stream = rec.create_stream();
                stream.accept_waveform(wave.sample_rate(), wave.samples());
                rec.decode(&stream);
                let secs = t0.elapsed().as_secs_f64();
                let text = stream.get_result().map(|r| r.text).unwrap_or_default();
                println!(
                    "[2PASS-accuracy] {:.3}s RTF={:.4} text={}",
                    secs,
                    secs / dur,
                    text.replace('\n', " ")
                );
            }
        }
    }

    println!("\n=== DONE ===");
}

// ---------------- probe modes (追加5/6/7) ----------------

fn run_probe(cfg: &PocConfig, probe: &str, wavs: &[(String, Wave)]) {
    match probe {
        "offline-hw-perf" => {
            println!("\n##### PROBE offline-hw-perf: SenseVoice(performance) + hotwords #####");
            println!(
                "hotwords_file={:?} score={} decoding={}",
                cfg.hotwords_file, cfg.hotwords_score, cfg.decoding_method
            );
            let base = build_offline_sensevoice(cfg, None, "greedy_search", None);
            let hw = cfg.hotwords();
            let variant = build_offline_sensevoice(cfg, hw.as_ref(), &cfg.decoding_method, None);
            println!(
                "[probe] baseline_create={} variant_create={}",
                base.is_some(),
                variant.is_some()
            );
            for (name, wave) in wavs {
                let b = base.as_ref().and_then(|r| offline_text(r, wave));
                let v = variant.as_ref().and_then(|r| offline_text(r, wave));
                report_ab("offline-hw-perf", name, b, v);
            }
        }
        "online-hw" => {
            println!("\n##### PROBE online-hw: streaming paraformer + hotwords #####");
            println!(
                "hotwords_file={:?} score={} decoding={}",
                cfg.hotwords_file, cfg.hotwords_score, cfg.decoding_method
            );
            let base = build_online(cfg, 4, None, "greedy_search");
            let hw = cfg.hotwords();
            let variant = match &hw {
                Some(h) => build_online(cfg, 4, Some(h), &cfg.decoding_method),
                None => None,
            };
            println!(
                "[probe] baseline_create={} variant_create={}",
                base.is_some(),
                variant.is_some()
            );
            let chunk = 1600usize; // 100ms @ 16k
            for (name, wave) in wavs {
                let b = base.as_ref().map(|r| online_text(r, wave, chunk));
                let v = variant.as_ref().map(|r| online_text(r, wave, chunk));
                report_ab("online-hw", name, b, v);
            }
        }
        "hr" => {
            println!("\n##### PROBE hr: SenseVoice + HomophoneReplacer #####");
            println!(
                "hr_lexicon={:?} hr_rule_fsts={:?}",
                cfg.hr_lexicon, cfg.hr_rule_fsts
            );
            let base = build_offline_sensevoice(cfg, None, "greedy_search", None);
            let hr = cfg.hr();
            let variant = build_offline_sensevoice(cfg, None, "greedy_search", hr.as_ref());
            println!(
                "[probe] baseline_create={} variant_create={}",
                base.is_some(),
                variant.is_some()
            );
            for (name, wave) in wavs {
                let b = base.as_ref().and_then(|r| offline_text(r, wave));
                let v = variant.as_ref().and_then(|r| offline_text(r, wave));
                report_ab("hr", name, b, v);
            }
        }
        _ => unreachable!(),
    }
}

fn offline_text(rec: &OfflineRecognizer, wave: &Wave) -> Option<String> {
    let stream = rec.create_stream();
    stream.accept_waveform(wave.sample_rate(), wave.samples());
    rec.decode(&stream);
    stream.get_result().map(|r| r.text)
}

fn online_text(rec: &OnlineRecognizer, wave: &Wave, chunk: usize) -> String {
    run_stream(
        rec,
        wave.samples(),
        wave.sample_rate(),
        chunk,
        0,
        false,
        None,
    )
    .final_text
}

fn report_ab(tag: &str, name: &str, baseline: Option<String>, variant: Option<String>) {
    let b_ok = baseline.is_some();
    let v_ok = variant.is_some();
    let b = baseline.unwrap_or_else(|| "<none>".to_string());
    let v = variant.unwrap_or_else(|| "<create-failed/none>".to_string());
    let changed = if b_ok && v_ok {
        if b != v {
            "CHANGED"
        } else {
            "UNCHANGED"
        }
    } else {
        "N/A"
    };
    println!("[{}] {} baseline={}", tag, name, b.replace('\n', " "));
    println!("[{}] {} variant ={}", tag, name, v.replace('\n', " "));
    println!("[{}] {} => {}", tag, name, changed);
}

// ---------------- streaming run helper ----------------

struct StreamRun {
    first_char_ms: Option<f64>,
    first_char_audio_ms: usize,
    decode_secs: f64,
    wall_secs: f64,
    final_text: String,
    snapshots: Vec<String>,
    rewrites: usize,
    shrinks: usize,
}

/// Feed audio in `chunk`-sized frames. `pace_ms > 0` sleeps to keep real-time
/// pacing (simulates a live microphone). `collect` records every changed
/// `get_result()` snapshot; when `snap_file` is given they are also written out.
fn run_stream(
    rec: &OnlineRecognizer,
    samples: &[f32],
    rate: i32,
    chunk: usize,
    pace_ms: u64,
    collect: bool,
    snap_file: Option<PathBuf>,
) -> StreamRun {
    let stream = rec.create_stream();
    let start = Instant::now();
    let mut decode_secs = 0.0f64;
    let mut first_char_ms: Option<f64> = None;
    let mut first_char_audio_ms = 0usize;
    let mut snapshots: Vec<String> = Vec::new();
    let mut rewrites = 0usize;
    let mut shrinks = 0usize;
    let mut last = String::new();

    let n = samples.len();
    let chunk_ms = if rate > 0 {
        chunk as u64 * 1000 / rate as u64
    } else {
        0
    };
    let mut pos = 0usize;
    let mut fed_ms = 0u64;

    while pos < n {
        let end = (pos + chunk).min(n);
        stream.accept_waveform(rate, &samples[pos..end]);
        pos = end;
        fed_ms += chunk_ms;

        while rec.is_ready(&stream) {
            let t = Instant::now();
            rec.decode(&stream);
            decode_secs += t.elapsed().as_secs_f64();
        }

        if let Some(r) = rec.get_result(&stream) {
            if !r.text.is_empty() {
                if first_char_ms.is_none() {
                    first_char_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
                    first_char_audio_ms = fed_ms as usize;
                }
                if collect && r.text != last {
                    let mut flags = String::new();
                    if !last.is_empty() && !r.text.starts_with(&last) {
                        flags.push_str(" <REWRITE>");
                        rewrites += 1;
                    }
                    if !last.is_empty() && r.text.len() < last.len() {
                        flags.push_str(" <SHRINK>");
                        shrinks += 1;
                    }
                    snapshots.push(format!(
                        "[snap @feed={}ms wall={:.0}ms]{} {}",
                        fed_ms,
                        start.elapsed().as_secs_f64() * 1000.0,
                        flags,
                        r.text
                    ));
                    last = r.text.clone();
                }
            }
        }

        if pace_ms > 0 {
            let target = Duration::from_millis(fed_ms);
            let elapsed = start.elapsed();
            if elapsed < target {
                std::thread::sleep(target - elapsed);
            }
        }
    }

    stream.input_finished();
    while rec.is_ready(&stream) {
        let t = Instant::now();
        rec.decode(&stream);
        decode_secs += t.elapsed().as_secs_f64();
    }
    let final_text = rec.get_result(&stream).map(|r| r.text).unwrap_or_default();

    if let Some(p) = snap_file {
        let mut out = String::new();
        for s in &snapshots {
            out.push_str(s);
            out.push('\n');
        }
        out.push_str(&format!("[final] {}\n", final_text));
        let _ = std::fs::write(&p, out);
    }

    StreamRun {
        first_char_ms,
        first_char_audio_ms,
        decode_secs,
        wall_secs: start.elapsed().as_secs_f64(),
        final_text,
        snapshots,
        rewrites,
        shrinks,
    }
}

// ---------------- recognizer builders ----------------

fn build_online(
    cfg: &PocConfig,
    threads: i32,
    hw: Option<&Hotwords>,
    decoding: &str,
) -> Option<OnlineRecognizer> {
    let enc = cfg.stream_dir.join("encoder.int8.onnx");
    let dec = cfg.stream_dir.join("decoder.int8.onnx");
    let tok = cfg.stream_dir.join("tokens.txt");
    for p in [&enc, &dec, &tok] {
        if !p.exists() {
            eprintln!("[online] missing model file: {}", p.display());
            return None;
        }
    }
    let mut c = OnlineRecognizerConfig::default();
    c.model_config.paraformer = OnlineParaformerModelConfig {
        encoder: Some(enc.to_string_lossy().to_string()),
        decoder: Some(dec.to_string_lossy().to_string()),
    };
    c.model_config.tokens = Some(tok.to_string_lossy().to_string());
    c.model_config.num_threads = threads;
    c.model_config.provider = Some("cpu".to_string());
    c.model_config.debug = false;
    c.decoding_method = Some(decoding.to_string());
    if decoding == "modified_beam_search" {
        c.max_active_paths = 4;
    }
    if let Some(hw) = hw {
        c.hotwords_file = Some(hw.file.to_string_lossy().to_string());
        c.hotwords_score = hw.score;
    }
    c.enable_endpoint = cfg.enable_endpoint;
    if cfg.enable_endpoint {
        c.rule1_min_trailing_silence = 2.4;
        c.rule2_min_trailing_silence = 1.2;
        c.rule3_min_utterance_length = 20.0;
    }
    OnlineRecognizer::create(&c)
}

fn build_offline_sensevoice(
    cfg: &PocConfig,
    hw: Option<&Hotwords>,
    decoding: &str,
    hr: Option<&Hr>,
) -> Option<OfflineRecognizer> {
    let model = cfg.perf_dir.join("model.int8.onnx");
    let tok = cfg.perf_dir.join("tokens.txt");
    if !model.exists() || !tok.exists() {
        eprintln!("[offline-perf] missing: {}", model.display());
        return None;
    }
    let mut c = OfflineRecognizerConfig::default();
    c.model_config.num_threads = 4;
    c.model_config.provider = Some("cpu".to_string());
    c.model_config.sense_voice = OfflineSenseVoiceModelConfig {
        model: Some(model.to_string_lossy().to_string()),
        language: Some("auto".to_string()),
        use_itn: true,
    };
    c.model_config.tokens = Some(tok.to_string_lossy().to_string());
    c.decoding_method = Some(decoding.to_string());
    if decoding == "modified_beam_search" {
        c.max_active_paths = 4;
    }
    if let Some(hw) = hw {
        c.hotwords_file = Some(hw.file.to_string_lossy().to_string());
        c.hotwords_score = hw.score;
    }
    if let Some(hr) = hr {
        c.hr.lexicon = Some(hr.lexicon.to_string_lossy().to_string());
        if let Some(f) = &hr.rule_fsts {
            c.hr.rule_fsts = Some(f.to_string_lossy().to_string());
        }
    }
    OfflineRecognizer::create(&c)
}

fn build_offline_funasr_nano(cfg: &PocConfig) -> Option<OfflineRecognizer> {
    let enc = cfg.acc_dir.join("encoder_adaptor.int8.onnx");
    let llm = cfg.acc_dir.join("llm.int8.onnx");
    let emb = cfg.acc_dir.join("embedding.int8.onnx");
    let tok = cfg.acc_dir.join("Qwen3-0.6B");
    for p in [&enc, &llm, &emb, &tok] {
        if !p.exists() {
            eprintln!("[offline-acc] missing: {}", p.display());
            return None;
        }
    }
    let mut c = OfflineRecognizerConfig::default();
    c.model_config.num_threads = 4;
    c.model_config.provider = Some("cpu".to_string());
    c.model_config.funasr_nano = OfflineFunASRNanoModelConfig {
        encoder_adaptor: Some(enc.to_string_lossy().to_string()),
        llm: Some(llm.to_string_lossy().to_string()),
        embedding: Some(emb.to_string_lossy().to_string()),
        tokenizer: Some(tok.to_string_lossy().to_string()),
        system_prompt: Some("You are a helpful assistant.".to_string()),
        user_prompt: Some("语音转写:".to_string()),
        max_new_tokens: 0,
        temperature: 1.0,
        top_p: 1.0,
        seed: 42,
        language: None,
        itn: 1,
        hotwords: None,
    };
    c.model_config.tokens = Some(String::new());
    OfflineRecognizer::create(&c)
}

// ---------------- config / args ----------------

#[derive(Clone)]
struct Hotwords {
    file: PathBuf,
    score: f32,
}

#[derive(Clone)]
struct Hr {
    lexicon: PathBuf,
    rule_fsts: Option<PathBuf>,
}

struct PocConfig {
    stream_dir: PathBuf,
    perf_dir: PathBuf,
    acc_dir: PathBuf,
    wavs: Vec<String>,
    threads: Vec<i32>,
    chunk_ms: u32,
    enable_endpoint: bool,
    skip_offline: bool,
    snapshot_dir: PathBuf,
    hotwords_file: Option<PathBuf>,
    hotwords_score: f32,
    decoding_method: String,
    hr_lexicon: Option<PathBuf>,
    hr_rule_fsts: Option<PathBuf>,
    probe: Option<String>,
}

impl PocConfig {
    fn hotwords(&self) -> Option<Hotwords> {
        self.hotwords_file.as_ref().map(|f| Hotwords {
            file: f.clone(),
            score: self.hotwords_score,
        })
    }
    fn hr(&self) -> Option<Hr> {
        self.hr_lexicon.as_ref().map(|l| Hr {
            lexicon: l.clone(),
            rule_fsts: self.hr_rule_fsts.clone(),
        })
    }
}

fn parse_args(args: &[String]) -> Result<PocConfig, String> {
    let models = resolve_models_dir();
    let mut cfg = PocConfig {
        stream_dir: models.join("sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en"),
        perf_dir: models.join("sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17"),
        acc_dir: models.join("sherpa-onnx-funasr-nano-int8-2025-12-30"),
        wavs: Vec::new(),
        threads: vec![1, 2, 4],
        chunk_ms: 100,
        enable_endpoint: false,
        skip_offline: false,
        snapshot_dir: PathBuf::from("."),
        hotwords_file: None,
        hotwords_score: 2.0,
        decoding_method: "greedy_search".to_string(),
        hr_lexicon: None,
        hr_rule_fsts: None,
        probe: None,
    };

    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--stream-dir" => {
                i += 1;
                cfg.stream_dir = PathBuf::from(need(args, i, "--stream-dir")?);
            }
            "--perf-dir" => {
                i += 1;
                cfg.perf_dir = PathBuf::from(need(args, i, "--perf-dir")?);
            }
            "--acc-dir" => {
                i += 1;
                cfg.acc_dir = PathBuf::from(need(args, i, "--acc-dir")?);
            }
            "--threads" => {
                i += 1;
                let v = need(args, i, "--threads")?;
                cfg.threads = v
                    .split(',')
                    .map(|s| {
                        s.trim()
                            .parse::<i32>()
                            .map_err(|_| format!("bad thread: {}", s))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            }
            "--chunk-ms" => {
                i += 1;
                cfg.chunk_ms = need(args, i, "--chunk-ms")?
                    .parse()
                    .map_err(|_| "bad --chunk-ms".to_string())?;
            }
            "--enable-endpoint" => cfg.enable_endpoint = true,
            "--skip-offline" => cfg.skip_offline = true,
            "--snapshot-dir" => {
                i += 1;
                cfg.snapshot_dir = PathBuf::from(need(args, i, "--snapshot-dir")?);
            }
            "--hotwords-file" => {
                i += 1;
                cfg.hotwords_file = Some(PathBuf::from(need(args, i, "--hotwords-file")?));
            }
            "--hotwords-score" => {
                i += 1;
                cfg.hotwords_score = need(args, i, "--hotwords-score")?
                    .parse()
                    .map_err(|_| "bad --hotwords-score".to_string())?;
            }
            "--decoding-method" => {
                i += 1;
                let v = need(args, i, "--decoding-method")?;
                if v != "greedy_search" && v != "modified_beam_search" {
                    return Err(
                        "--decoding-method must be greedy_search or modified_beam_search".into(),
                    );
                }
                cfg.decoding_method = v.to_string();
            }
            "--hr-lexicon" => {
                i += 1;
                cfg.hr_lexicon = Some(PathBuf::from(need(args, i, "--hr-lexicon")?));
            }
            "--hr-rule-fsts" => {
                i += 1;
                cfg.hr_rule_fsts = Some(PathBuf::from(need(args, i, "--hr-rule-fsts")?));
            }
            "--probe" => {
                i += 1;
                let v = need(args, i, "--probe")?;
                if !["offline-hw-perf", "online-hw", "hr"].contains(&v) {
                    return Err("--probe must be offline-hw-perf | online-hw | hr".into());
                }
                cfg.probe = Some(v.to_string());
            }
            "-h" | "--help" => {
                println!("{}", USAGE);
                std::process::exit(0);
            }
            _ => {
                if a.starts_with("--") {
                    return Err(format!("unknown flag: {}", a));
                }
                cfg.wavs.push(a.clone());
            }
        }
        i += 1;
    }
    if cfg.wavs.is_empty() {
        return Err("no wav files provided".into());
    }
    Ok(cfg)
}

fn need<'a>(args: &'a [String], i: usize, flag: &str) -> Result<&'a str, String> {
    args.get(i)
        .map(|s| s.as_str())
        .ok_or_else(|| format!("{} requires value", flag))
}

fn resolve_models_dir() -> PathBuf {
    if let Ok(d) = env::var("POC_MODELS_DIR") {
        return PathBuf::from(d);
    }
    let exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    // target/{debug,release}/poc_local_stream.exe -> project root
    let root = exe
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("models")
}

fn short_name(path: &str) -> String {
    PathBuf::from(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "wav".to_string())
}
