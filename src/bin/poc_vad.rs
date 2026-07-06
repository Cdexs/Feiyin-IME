// ASR-LONG-AUDIO-001: 独立 VAD PoC bin，验证 silero VAD 切分长音频
use sherpa_onnx::{VadModelConfig, VoiceActivityDetector};
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: poc_vad <model_dir> <wav> [wav...]");
        eprintln!("  model_dir: path containing silero-vad/silero_vad.onnx");
        std::process::exit(1);
    }
    let model_dir = PathBuf::from(&args[1]);
    let wavs = &args[2..];

    let vad_model = model_dir.join("silero-vad").join("silero_vad.onnx");
    if !vad_model.exists() {
        eprintln!("VAD model not found: {}", vad_model.display());
        std::process::exit(2);
    }

    let config = VadModelConfig {
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
    println!("Creating VAD detector ...");
    let detector = match VoiceActivityDetector::create(&config, 300.0) {
        Some(d) => d,
        None => {
            eprintln!("Failed to create VAD detector");
            std::process::exit(3);
        }
    };
    println!("VAD detector created");

    for wav in wavs {
        let wav_path = PathBuf::from(wav);
        let samples = read_wav_mono(&wav_path);
        let audio_secs = samples.len() as f64 / 16000.0;
        println!("--- {} ({:.1}s) ---", wav, audio_secs);

        // Feed samples in 512-sample windows
        let win = 512usize;
        let mut offset = 0;
        while offset < samples.len() {
            let end = (offset + win).min(samples.len());
            detector.accept_waveform(&samples[offset..end]);
            offset = end;
        }
        detector.flush();

        // Collect segments
        let mut seg_count = 0;
        let mut max_seg_secs = 0.0f64;
        loop {
            match detector.front() {
                Some(seg) => {
                    let start = seg.start();
                    let n = seg.n();
                    let seg_secs = n as f64 / 16000.0;
                    let start_secs = start as f64 / 16000.0;
                    println!("  seg {}: start={:.2}s, dur={:.2}s", seg_count, start_secs, seg_secs);
                    if seg_secs > max_seg_secs {
                        max_seg_secs = seg_secs;
                    }
                    detector.pop();
                    seg_count += 1;
                }
                None => break,
            }
        }
        detector.clear();
        println!("  total {} segments, max segment {:.2}s", seg_count, max_seg_secs);
    }
}

fn read_wav_mono(path: &std::path::Path) -> Vec<f32> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).expect("open wav");
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).expect("read wav");
    let data = &buf[44..];
    let mut samples = Vec::with_capacity(data.len() / 2);
    for chunk in data.chunks_exact(2) {
        let v = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0;
        samples.push(v);
    }
    samples
}