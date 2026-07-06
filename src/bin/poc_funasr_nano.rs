// POC-QWEN3ASR-002A · FunASR Nano PoC bin
//
// 独立 PoC，不动主线代码。支持两套模型：
//   --model-type sensevoice  : 179MB CTC 兼容版（OfflineSenseVoiceModelConfig，无 hotwords）
//   --model-type funasr-nano : 802.7MB 原生版（OfflineFunASRNanoModelConfig，有 hotwords）
//
// 参考：rust-api-examples/examples/qwen3_asr.rs + offline_asr.rs

use sherpa_onnx::{
    OfflineFunASRNanoModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    OfflineSenseVoiceModelConfig, Wave,
};
use std::env;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    let parsed = parse_args(&args);
    let cfg = match parsed {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            print_usage();
            std::process::exit(1);
        }
    };

    let model_root = match &cfg.model_dir {
        Some(d) => PathBuf::from(d),
        None => resolve_model_root(&cfg.model_type),
    };
    if !model_root.exists() {
        eprintln!("Model dir not found: {:?}", model_root);
        std::process::exit(2);
    }

    println!("=== POC FunASR Nano ===");
    println!("model_type : {}", cfg.model_type);
    println!("model_root : {}", model_root.display());
    println!("threads    : {}", cfg.threads);
    println!("repeat     : {}", cfg.repeat);
    println!("hotwords   : {:?}", cfg.hotwords);
    println!("blank_penalty: {}", cfg.blank_penalty);
    println!();

    // Build recognizer config by model type
    let mut recognizer_config = OfflineRecognizerConfig::default();
    recognizer_config.model_config.num_threads = cfg.threads;
    recognizer_config.model_config.provider = Some("cpu".to_string());
    recognizer_config.model_config.debug = false;

    if cfg.model_type == "sensevoice" {
        let model_path = model_root.join("model.int8.onnx");
        let tokens_path = model_root.join("tokens.txt");
        recognizer_config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
            model: Some(model_path.to_str().unwrap().to_string()),
            language: Some("auto".to_string()),
            use_itn: true,
        };
        recognizer_config.model_config.tokens =
            Some(tokens_path.to_str().unwrap().to_string());
        recognizer_config.blank_penalty = cfg.blank_penalty;
    } else {
        // funasr-nano native
        let enc = model_root.join("encoder_adaptor.int8.onnx");
        let llm = model_root.join("llm.int8.onnx");
        let emb = model_root.join("embedding.int8.onnx");
        let tok = model_root.join("Qwen3-0.6B");
        recognizer_config.model_config.funasr_nano = OfflineFunASRNanoModelConfig {
                encoder_adaptor: Some(enc.to_str().unwrap().to_string()),
                llm: Some(llm.to_str().unwrap().to_string()),
                embedding: Some(emb.to_str().unwrap().to_string()),
                tokenizer: Some(tok.to_str().unwrap().to_string()),
                system_prompt: Some("You are a helpful assistant.".to_string()),
                user_prompt: Some("语音转写:".to_string()),
                max_new_tokens: 0, // 0 = use model default
            temperature: 1.0,
            top_p: 1.0,
            seed: 42,
            language: None, // None = auto detect
            itn: 1,
            hotwords: cfg.hotwords.clone(),
        };
        // tokens 字段对 funasr-nano 原生版非必需（tokenizer 目录已含），留空避免冲突
        recognizer_config.model_config.tokens = Some(String::new());
    }

    println!("Creating recognizer ...");
    let t_create = Instant::now();
    let recognizer =
        OfflineRecognizer::create(&recognizer_config).expect("Failed to create recognizer");
    let create_secs = t_create.elapsed().as_secs_f64();
    println!("Recognizer created in {:.3} s", create_secs);
    println!();

    for wav in &cfg.wavs {
        let wave = match Wave::read(wav) {
            Some(w) => w,
            None => {
                eprintln!("Failed to read wav: {}", wav);
                continue;
            }
        };
        let audio_dur = wave.samples().len() as f64 / wave.sample_rate() as f64;
        println!("--- wav: {} ({:.2}s) ---", wav, audio_dur);

        let mut sum_secs = 0.0f64;
        for r in 0..cfg.repeat {
            let t0 = Instant::now();
            // hotwords 通过 config 层注入（OfflineFunASRNanoModelConfig.hotwords），
            // create_stream_with_hotwords 仅支持 transducer 模型，对 funasr-nano 报错
            // "Only transducer models support contextual biasing"，故一律用普通 stream。
            let stream = recognizer.create_stream();
            stream.accept_waveform(wave.sample_rate(), wave.samples());
            recognizer.decode(&stream);
            let secs = t0.elapsed().as_secs_f64();
            sum_secs += secs;
            let result = stream.get_result();
            let text = result
                .as_ref()
                .map(|r| r.text.clone())
                .unwrap_or_default();
            let rtf = secs / audio_dur;
            println!(
                "  run {}: {:.3}s, RTF={:.4}, text={}",
                r + 1,
                secs,
                rtf,
                text.replace('\n', " ")
            );
        }
        let avg = sum_secs / cfg.repeat as f64;
        let avg_rtf = avg / audio_dur;
        println!(
            "  AVG: {:.3}s, RTF={:.4}, audio={:.2}s, load={:.3}s",
            avg, avg_rtf, audio_dur, create_secs
        );
        println!();
    }
}

struct PocConfig {
    model_type: String,
    model_dir: Option<String>,
    wavs: Vec<String>,
    hotwords: Option<String>,
    threads: i32,
    repeat: usize,
    blank_penalty: f32,
}

fn parse_args(args: &[String]) -> Result<PocConfig, String> {
    let mut model_type = String::from("funasr-nano");
    let mut model_dir: Option<String> = None;
    let mut wavs: Vec<String> = Vec::new();
    let mut hotwords: Option<String> = None;
    let mut threads: i32 = 4;
    let mut repeat: usize = 1;
    let mut blank_penalty: f32 = 0.0;

    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--model-type" => {
                i += 1;
                if i >= args.len() {
                    return Err("--model-type requires value".into());
                }
                let v = args[i].clone();
                if v != "sensevoice" && v != "funasr-nano" {
                    return Err("--model-type must be sensevoice or funasr-nano".into());
                }
                model_type = v;
            }
            "--model-dir" => {
                i += 1;
                if i >= args.len() {
                    return Err("--model-dir requires value".into());
                }
                model_dir = Some(args[i].clone());
            }
            "--hotwords" => {
                i += 1;
                if i >= args.len() {
                    return Err("--hotwords requires value".into());
                }
                hotwords = Some(args[i].clone());
            }
            "--threads" => {
                i += 1;
                if i >= args.len() {
                    return Err("--threads requires value".into());
                }
                threads = args[i].parse().map_err(|_| "invalid --threads".to_string())?;
            }
            "--repeat" => {
                i += 1;
                if i >= args.len() {
                    return Err("--repeat requires value".into());
                }
                repeat = args[i]
                    .parse()
                    .map_err(|_| "invalid --repeat".to_string())?;
            }
            "--blank-penalty" => {
                i += 1;
                if i >= args.len() {
                    return Err("--blank-penalty requires value".into());
                }
                blank_penalty = args[i]
                    .parse()
                    .map_err(|_| "invalid --blank-penalty".to_string())?;
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {
                if a.starts_with("--") {
                    return Err(format!("unknown flag: {}", a));
                }
                wavs.push(a.clone());
            }
        }
        i += 1;
    }

    if wavs.is_empty() {
        return Err("no wav files provided".into());
    }

    Ok(PocConfig {
        model_type,
        model_dir,
        wavs,
        hotwords,
        threads,
        repeat,
        blank_penalty,
    })
}

fn resolve_model_root(model_type: &str) -> PathBuf {
    // 相对 exe 所在目录的 models/ 子目录
    let exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    // target/release/poc_funasr_nano.exe -> 项目根需向上 3 级（release, target, root）
    let project_root = exe
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let models_dir = project_root.join("models");

    if model_type == "sensevoice" {
        models_dir.join("sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17")
    } else {
        models_dir.join("sherpa-onnx-funasr-nano-int8-2025-12-30")
    }
}

fn print_usage() {
    eprintln!(
        "Usage: poc_funasr_nano [wav...] [--model-type sensevoice|funasr-nano] [--model-dir <path>] [--hotwords \"w1,w2\"] [--threads N] [--repeat N] [--blank-penalty F]"
    );
}