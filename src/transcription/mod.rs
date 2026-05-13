use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::{config::ChineseScript, text_normalizer};

// Re-export SenseVoice config for convenience
use sherpa_onnx::OfflineSenseVoiceModelConfig;

/// ASR mode: offline or streaming (2-pass)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AsrMode {
    /// Offline mode: single pass, higher accuracy
    Offline,
    /// Streaming mode: 2-pass (streaming preview + offline correction)
    Streaming,
}

/// ASR transcriber using sherpa-onnx SenseVoice
pub struct Transcriber {
    mode: AsrMode,
    asr_language: String,
    offline_recognizer: sherpa_onnx::OfflineRecognizer,
}

impl Transcriber {
    /// Create new SenseVoice Transcriber
    pub fn new(model_dir: &Path, enable_streaming: bool, asr_language: String) -> Result<Self> {
        let mode = if enable_streaming {
            AsrMode::Streaming
        } else {
            AsrMode::Offline
        };

        let offline_recognizer = create_sensevoice_recognizer(model_dir, &asr_language)?;

        Ok(Self {
            mode,
            asr_language,
            offline_recognizer,
        })
    }

    pub fn transcribe(
        &self,
        samples: &[f32],
        _language: &str,
        script: ChineseScript,
    ) -> Result<String> {
        match self.mode {
            AsrMode::Offline => self.transcribe_offline(samples, script),
            AsrMode::Streaming => self.transcribe_2pass(samples, script),
        }
    }

    /// Single-pass offline transcription (higher accuracy)
    fn transcribe_offline(&self, samples: &[f32], script: ChineseScript) -> Result<String> {
        let stream = self.offline_recognizer.create_stream();
        stream.accept_waveform(16000, samples);
        self.offline_recognizer.decode(&stream);

        let result = stream.get_result().context("No transcription result")?;
        let text = result.text.trim().to_string();

        // SenseVoice 输出始终应用繁简转换（含中文时用 zh 规则兜底）
        Ok(text_normalizer::normalize_text_for_language(
            &text, "zh", script,
        ))
    }

    /// 2-pass streaming transcription
    /// 第一遍：实时流式预览（暂时用 offline 模拟，未来可扩展为 OnlineRecognizer）
    /// 第二遍：offline 离线修正，提高准确率
    fn transcribe_2pass(&self, samples: &[f32], script: ChineseScript) -> Result<String> {
        // 当前实现：直接使用 offline 单遍
        // 未来可以扩展：
        // 1. 第一遍用 OnlineRecognizer 实时输出预览
        // 2. 录音结束后用 OfflineRecognizer 做第二遍修正
        //
        // 由于 sherpa-onnx 的 streaming Paraformer 模型需要额外下载，
        // 当前版本先用 offline 单遍实现，后续可扩展为真正的 2-pass

        self.transcribe_offline(samples, script)
    }
}

/// Create SenseVoice Chinese recognizer, with configurable ASR language
fn create_sensevoice_recognizer(
    model_dir: &Path,
    language: &str,
) -> Result<sherpa_onnx::OfflineRecognizer> {
    let model_dir_path = ensure_sensevoice_model(model_dir)?;

    let model_path = model_dir_path.join("model.int8.onnx");
    let tokens_path = model_dir_path.join("tokens.txt");

    let offline_config = sherpa_onnx::OfflineRecognizerConfig {
        model_config: sherpa_onnx::OfflineModelConfig {
            sense_voice: OfflineSenseVoiceModelConfig {
                model: Some(model_path.to_str().unwrap_or("").to_string()),
                language: Some(language.to_string()),
                use_itn: true,
            },
            tokens: Some(tokens_path.to_str().unwrap_or("").to_string()),
            ..Default::default()
        },
        blank_penalty: 0.5,
        ..Default::default()
    };

    sherpa_onnx::OfflineRecognizer::create(&offline_config)
        .context("Failed to create SenseVoice offline recognizer")
}

/// Download SenseVoice multilingual model if not already present (~150MB)
fn ensure_sensevoice_model(model_dir: &Path) -> Result<PathBuf> {
    // SenseVoice multilingual model (zh/en/ja/ko/yue) - ~150MB
    // 模型目录名称：sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09
    // 模型需预先放置在 models/ 目录下，不再自动下载
    let model_dir_path = model_dir.join("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09");

    let model_file = model_dir_path.join("model.int8.onnx");
    let tokens_file = model_dir_path.join("tokens.txt");

    if model_dir_path.exists() && model_file.exists() && tokens_file.exists() {
        log::info!("SenseVoice model found at {:?}", model_dir_path);
        return Ok(model_dir_path);
    }

    anyhow::bail!(
        "SenseVoice model not found at {:?}. Please download manually from:\n  https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09",
        model_dir_path
    );
}

pub fn model_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join("models")
}
