// RESEARCH-ASR-HALLUC-ROOT-001: VAD 分段 + native ASR + 解码参数控制 PoC
// 用法: poc_halluc <wav> [--temperature F] [--top-p F] [--seed N] [--max-new-tokens N] [--hotwords "w1,w2"]
use sherpa_onnx::{
    OfflineFunASRNanoModelConfig, OfflineRecognizer, OfflineRecognizerConfig, VadModelConfig,
    VoiceActivityDetector, Wave,
};
use std::env;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: poc_halluc <wav> [wav...] [--temperature F] [--top-p F] [--seed N] [--max-new-tokens N] [--hotwords \"w1,w2\"] [--threads N]");
        std::process::exit(1);
    }

    let mut wavs: Vec<String> = Vec::new();
    let mut temperature: f32 = 1.0;
    let mut top_p: f32 = 1.0;
    let mut seed: i32 = 42;
    let mut max_new_tokens: i32 = 0;
    let mut hotwords: Option<String> = None;
    let mut threads: i32 = 4;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--temperature" => {
                i += 1;
                temperature = args[i].parse().unwrap_or(1.0);
            }
            "--top-p" => {
                i += 1;
                top_p = args[i].parse().unwrap_or(1.0);
            }
            "--seed" => {
                i += 1;
                seed = args[i].parse().unwrap_or(42);
            }
            "--max-new-tokens" => {
                i += 1;
                max_new_tokens = args[i].parse().unwrap_or(0);
            }
            "--hotwords" => {
                i += 1;
                hotwords = Some(args[i].clone());
            }
            "--threads" => {
                i += 1;
                threads = args[i].parse().unwrap_or(4);
            }
            _ => wavs.push(args[i].clone()),
        }
        i += 1;
    }

    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let exe_dir = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let project_root = exe_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let models_dir = project_root.join("models");
    let native_model = models_dir.join("sherpa-onnx-funasr-nano-int8-2025-12-30");
    let vad_model = models_dir.join("silero-vad").join("silero_vad.onnx");

    println!("=== RESEARCH-ASR-HALLUC-ROOT-001 PoC ===");
    println!(
        "temperature: {}, top_p: {}, seed: {}, max_new_tokens: {}",
        temperature, top_p, seed, max_new_tokens
    );
    println!("hotwords: {:?}", hotwords);
    println!("native_model: {}", native_model.display());
    println!();

    // Create native recognizer
    let mut recognizer_config = OfflineRecognizerConfig::default();
    recognizer_config.model_config.num_threads = threads;
    recognizer_config.model_config.provider = Some("cpu".to_string());
    recognizer_config.model_config.funasr_nano = OfflineFunASRNanoModelConfig {
        encoder_adaptor: Some(
            native_model
                .join("encoder_adaptor.int8.onnx")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        llm: Some(
            native_model
                .join("llm.int8.onnx")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        embedding: Some(
            native_model
                .join("embedding.int8.onnx")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        tokenizer: Some(
            native_model
                .join("Qwen3-0.6B")
                .to_str()
                .unwrap()
                .to_string(),
        ),
        system_prompt: Some("You are a helpful assistant.".to_string()),
        user_prompt: Some("语音转写:".to_string()),
        max_new_tokens,
        temperature,
        top_p,
        seed,
        language: None,
        itn: 1,
        hotwords: hotwords.clone(),
    };
    recognizer_config.model_config.tokens = Some(String::new());

    println!("Creating native recognizer ...");
    let t0 = Instant::now();
    let recognizer =
        OfflineRecognizer::create(&recognizer_config).expect("Failed to create recognizer");
    println!("Recognizer created in {:.3}s", t0.elapsed().as_secs_f64());
    println!();

    // Create VAD detector (生产参数: threshold 0.5, min_silence 0.3, max_speech 20s)
    let vad_config = VadModelConfig {
        silero_vad: sherpa_onnx::SileroVadModelConfig {
            model: Some(vad_model.to_str().unwrap().to_string()),
            threshold: 0.5,
            min_silence_duration: 0.3,
            min_speech_duration: 0.1,
            window_size: 512,
            max_speech_duration: 20.0,
        },
        ten_vad: sherpa_onnx::TenVadModelConfig::default(),
        sample_rate: 16000,
        num_threads: 1,
        provider: Some("cpu".to_string()),
        debug: false,
    };
    let mut detector =
        VoiceActivityDetector::create(&vad_config, 300.0).expect("Failed to create VAD");

    for wav in &wavs {
        let wave = Wave::read(wav).expect("Failed to read wav");
        let samples = wave.samples();
        let sr = wave.sample_rate();
        let total_secs = samples.len() as f64 / sr as f64;
        println!("=== wav: {} ({:.1}s) ===", wav, total_secs);

        // VAD 分段（参考 poc_vad.rs 窗口喂法）
        let mut segments: Vec<Vec<f32>> = Vec::new();
        {
            // 用 window 512 喂 VAD
            let window = 512;
            let mut offset = 0usize;
            while offset + window <= samples.len() {
                let end = offset + window;
                detector.accept_waveform(&samples[offset..end]);
                offset = end;
                // 取出已检测的段（先复制 samples 再 pop，避免借用冲突）
                loop {
                    if detector.is_empty() {
                        break;
                    }
                    let seg_data: Option<Vec<f32>> = {
                        match detector.front() {
                            Some(seg) => {
                                let n = seg.n();
                                let start = seg.start();
                                let seg_secs = n as f64 / 16000.0;
                                let start_secs = start as f64 / 16000.0;
                                println!(
                                    "  VAD segment: start={:.2}s, dur={:.2}s",
                                    start_secs, seg_secs
                                );
                                Some(seg.samples().to_vec())
                            }
                            None => None,
                        }
                    };
                    match seg_data {
                        Some(d) => {
                            segments.push(d);
                            detector.pop();
                        }
                        None => break,
                    }
                }
            }
            // 喂剩余 + flush
            if offset < samples.len() {
                detector.accept_waveform(&samples[offset..]);
            }
            detector.flush();
            loop {
                if detector.is_empty() {
                    break;
                }
                let seg_data: Option<Vec<f32>> = {
                    match detector.front() {
                        Some(seg) => {
                            let n = seg.n();
                            let seg_secs = n as f64 / 16000.0;
                            println!("  VAD segment (flush): dur={:.2}s", seg_secs);
                            Some(seg.samples().to_vec())
                        }
                        None => None,
                    }
                };
                match seg_data {
                    Some(d) => {
                        segments.push(d);
                        detector.pop();
                    }
                    None => break,
                }
            }
        }
        println!("  Total segments: {}", segments.len());

        // 逐段 ASR
        let mut full_text = String::new();
        for (idx, seg) in segments.iter().enumerate() {
            let seg_secs = seg.len() as f64 / 16000.0;
            let t1 = Instant::now();
            let stream = recognizer.create_stream();
            stream.accept_waveform(16000, seg);
            recognizer.decode(&stream);
            let dt = t1.elapsed().as_secs_f64();
            let text = stream
                .get_result()
                .as_ref()
                .map(|r| r.text.clone())
                .unwrap_or_default();
            let char_count = text.chars().count();
            let rate = if seg_secs > 0.1 {
                char_count as f64 / seg_secs
            } else {
                0.0
            };
            println!(
                "  seg[{}]: {:.2}s, {:.3}s, rate={:.1}c/s, text={}",
                idx,
                seg_secs,
                dt,
                rate,
                text.replace('\n', " ")
            );
            full_text.push_str(&text);
        }
        println!("  FULL: {}", full_text.replace('\n', " "));
        println!();
    }
}
