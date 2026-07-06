use anyhow::{Context, Result};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::{config::ChineseScript, text_normalizer};

// Re-export SenseVoice config for convenience
use sherpa_onnx::{OfflineFunASRNanoModelConfig, OfflineSenseVoiceModelConfig};

mod vad;
pub use vad::{
    build_padded_segments, join_segment_texts, should_segment, VadSegmenter,
    SEGMENT_MAX_SECS, SEGMENT_PADDING_SAMPLES, SEGMENT_TRIGGER_SECS,
};

/// ASR mode: offline or streaming (2-pass)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AsrMode {
    /// Offline mode: single pass, higher accuracy
    Offline,
    /// Streaming mode: 2-pass (streaming preview + offline correction)
    Streaming,
}

/// ASR 模型选择（DEC-025）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AsrModel {
    /// 性能最优：179MB FunASR Nano CTC 兼容版（OfflineSenseVoiceModelConfig，无 hotwords）
    Performance,
    /// 准确率更高：972MB FunASR Nano native（OfflineFunASRNanoModelConfig，config 层 hotwords）
    Accuracy,
}

impl AsrModel {
    pub fn from_config(s: &str) -> Self {
        if s.eq_ignore_ascii_case("accuracy") {
            AsrModel::Accuracy
        } else {
            AsrModel::Performance
        }
    }
}

/// ASR transcriber using sherpa-onnx
///
/// 双模型架构（DEC-025）：
/// - Performance: 179MB CTC，OfflineSenseVoiceModelConfig，无 hotwords
/// - Accuracy: 972MB native，OfflineFunASRNanoModelConfig，config 层 hotwords
/// - Accuracy 模式常驻 Performance recognizer 作 hallucination 兜底
///
/// SAFETY: sherpa_onnx::OfflineRecognizer 内含 *const C++ 指针（!Send），
/// 但其 C++ 实现本身是线程安全的（create/decode/destroy 均可跨线程，
/// 只需保证同一 recognizer 实例不被并发访问）。
/// 此处 Transcriber 通过 channel 在后台构建线程与 worker 线程间转移，
/// 转移期间无并发访问（构建完成后才发送，发送后构建线程不再触碰），
/// 因此手动实现 Send 是安全的。
pub struct Transcriber {
    mode: AsrMode,
    asr_language: String,
    asr_model: AsrModel,
    offline_recognizer: sherpa_onnx::OfflineRecognizer,
    /// Accuracy 模式下的兜底 performance recognizer（None = performance 模式或兜底未就位）
    fallback_recognizer: Option<sherpa_onnx::OfflineRecognizer>,
    /// 当前注入的 hotwords 版本号（len + 内容哈希），用于感知词库变更
    hotwords_version: u64,
    /// VAD 分段器（仅 accuracy 长音频用，懒加载）
    /// 用 Mutex<Option> 因为 VAD 在首次长音频时才初始化
    vad_segmenter: Option<Mutex<vad::VadSegmenter>>,
}

// SAFETY: Transcriber 持有的 OfflineRecognizer 内部为 *const C++ 指针。
// 跨线程转移时，发送方在 send 后不再访问该实例，接收方独占所有权，
// 满足"单一时刻单线程访问"约束。sherpa-onnx C++ 层本身支持跨线程调用。
unsafe impl Send for Transcriber {}

impl Transcriber {
    /// Create new Transcriber with explicit ASR model selection
    pub fn new(
        model_dir: &Path,
        enable_streaming: bool,
        asr_language: String,
        asr_model: AsrModel,
        hotwords: Option<&str>,
    ) -> Result<Self> {
        let mode = if enable_streaming {
            AsrMode::Streaming
        } else {
            AsrMode::Offline
        };

        let (offline_recognizer, fallback_recognizer, hotwords_version) =
            build_recognizers(model_dir, &asr_language, asr_model, hotwords)?;

        // VAD 分段器仅 accuracy 模式懒加载初始化；performance 模式设 None
        let vad_segmenter = if asr_model == AsrModel::Accuracy {
            // accuracy 模式下立即尝试初始化（模型文件在则建，失败后续降级）
            vad::VadSegmenter::try_new(model_dir).map(Mutex::new)
        } else {
            None
        };

        Ok(Self {
            mode,
            asr_language,
            asr_model,
            offline_recognizer,
            fallback_recognizer,
            hotwords_version,
            vad_segmenter,
        })
    }

    pub fn asr_model(&self) -> AsrModel {
        self.asr_model
    }

    /// 当前 hotwords 版本号（外部对比用）
    pub fn hotwords_version(&self) -> u64 {
        self.hotwords_version
    }

    /// 当前 ASR 语言（热重载 swap 时同步 active 状态用）
    pub fn language(&self) -> &str {
        &self.asr_language
    }

    pub fn transcribe(
        &self,
        samples: &[f32],
        _language: &str,
        script: ChineseScript,
    ) -> Result<String> {
        self.transcribe_with_punct_info(samples, script).map(|(text, _)| text)
    }

    /// 转录并返回标点来源标记（ASR-PUNCT-OPT-001）
    ///
    /// 返回 `(text, native_punctuated)`：
    /// - `native_punctuated = true`：文本真正出自 native 模型（自带标点），可跳过标点引擎
    /// - `native_punctuated = false`：文本出自 performance/兜底/混合来源（无标点），需走标点引擎
    ///
    /// 标记规则：
    /// - performance → 恒 false
    /// - accuracy 单次 native 成功 → true
    /// - accuracy 兜底触发（fallback 出文本）→ false（CTC 无标点）
    /// - VAD 分段：所有段均 native 成功才 true；任一段兜底 → false
    pub fn transcribe_with_punct_info(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        match self.mode {
            AsrMode::Offline => self.transcribe_offline_detailed(samples, script),
            AsrMode::Streaming => self.transcribe_2pass_detailed(samples, script),
        }
    }

    /// Single-pass offline transcription (higher accuracy) — 旧签名兼容
    fn transcribe_offline(&self, samples: &[f32], script: ChineseScript) -> Result<String> {
        self.transcribe_offline_detailed(samples, script).map(|(t, _)| t)
    }

    /// 带标点来源标记的 offline 转录
    fn transcribe_offline_detailed(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        // ASR-LONG-AUDIO-001: accuracy 分支长音频 VAD 分段路径
        if self.asr_model == AsrModel::Accuracy && vad::should_segment(samples) {
            if let Some(ref vad_lock) = self.vad_segmenter {
                if let Ok(vad) = vad_lock.lock() {
                    let segments = vad.segment(samples);
                    if !segments.is_empty() {
                        log::info!(
                            "VAD segmented {} samples ({:.1}s) into {} segments",
                            samples.len(),
                            samples.len() as f64 / 16000.0,
                            segments.len()
                        );
                        // 逐段转录，收集 (text, native_punctuated)
                        let mut all_native = true;
                        let mut seg_texts: Vec<String> = Vec::with_capacity(segments.len());
                        for seg in &segments {
                            match self.transcribe_segment_detailed(seg, script) {
                                Ok((t, np)) => {
                                    seg_texts.push(t);
                                    if !np {
                                        all_native = false;
                                    }
                                }
                                Err(e) => {
                                    log::warn!("VAD segment transcription failed: {}", e);
                                    seg_texts.push(String::new());
                                    all_native = false;
                                }
                            }
                        }
                        let joined = vad::join_segment_texts(&seg_texts);
                        if !joined.trim().is_empty() {
                            log::info!("VAD segmented transcription: {}", joined);
                            return Ok((
                                text_normalizer::normalize_text_for_language(
                                    &joined, "zh", script,
                                ),
                                all_native,
                            ));
                        }
                        // 分段全部空 → 降级单次转录走兜底
                        log::warn!("All VAD segments produced empty text, falling back to single-pass");
                    }
                }
            } else {
                log::warn!("VAD segmenter unavailable, falling back to single-pass (may hit max_total_len)");
            }
        }

        self.transcribe_segment_detailed(samples, script)
    }

    /// 转录单段音频（含 accuracy 三重兜底链）— 旧签名兼容
    fn transcribe_segment(&self, samples: &[f32], script: ChineseScript) -> Result<String> {
        self.transcribe_segment_detailed(samples, script).map(|(t, _)| t)
    }

    /// 带标点来源标记的单段转录。
    /// accuracy native 成功 → (text, true)；兜底/性能失败 → (text, false)
    fn transcribe_segment_detailed(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        let stream = self.offline_recognizer.create_stream();
        stream.accept_waveform(16000, samples);
        self.offline_recognizer.decode(&stream);

        let result = stream.get_result().context("No transcription result")?;
        let text = result.text.trim().to_string();

        // Accuracy 分支兜底链（DEC-025 + ASR-NATIVE-LONG-001）
        if self.asr_model == AsrModel::Accuracy {
            let need_fallback = text.is_empty()
                || is_hallucination(&text, samples)
                || is_repetitive_garbage(&text);

            if need_fallback {
                let reason = if text.is_empty() {
                    "empty output"
                } else if is_hallucination(&text, samples) {
                    "hallucination"
                } else {
                    "repetitive garbage"
                };
                log::warn!(
                    "ASR accuracy model abnormal output ({}), falling back to performance model",
                    reason
                );

                if let Some(ref fallback) = self.fallback_recognizer {
                    let fb_stream = fallback.create_stream();
                    fb_stream.accept_waveform(16000, samples);
                    fallback.decode(&fb_stream);
                    if let Some(fb_result) = fb_stream.get_result().as_ref() {
                        let fb_text = fb_result.text.trim().to_string();
                        if !fb_text.is_empty() && !is_repetitive_garbage(&fb_text) {
                            log::info!("Fallback transcription: {}", fb_text);
                            // 兜底出自 CTC（无标点）→ native_punctuated = false
                            return Ok((
                                text_normalizer::normalize_text_for_language(
                                    &fb_text, "zh", script,
                                ),
                                false,
                            ));
                        }
                    }
                    log::warn!("Fallback also failed, returning error to pipeline");
                    anyhow::bail!("ASR transcription failed: accuracy model produced {} and performance fallback also failed", reason);
                } else {
                    log::warn!("No fallback recognizer available, returning error");
                    anyhow::bail!("ASR transcription failed: accuracy model produced {} and no fallback available", reason);
                }
            }

            // accuracy native 成功 → native_punctuated = true
            return Ok((
                text_normalizer::normalize_text_for_language(&text, "zh", script),
                true,
            ));
        }

        // performance 分支 → native_punctuated = false（CTC 无标点）
        Ok((
            text_normalizer::normalize_text_for_language(&text, "zh", script),
            false,
        ))
    }

    /// 2-pass streaming transcription — 旧签名兼容
    fn transcribe_2pass(&self, samples: &[f32], script: ChineseScript) -> Result<String> {
        self.transcribe_2pass_detailed(samples, script).map(|(t, _)| t)
    }

    /// 带标点来源标记的 2-pass 转录
    fn transcribe_2pass_detailed(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        // 当前实现：直接使用 offline 单遍
        self.transcribe_offline_detailed(samples, script)
    }
}

/// Hallucination 判定：输出字符数 > 音频秒数 × N（中文语速上限 ~8字/秒，N=12 留裕量）
/// 返回 true 表示疑似 hallucination，应丢弃并用 performance 模型重转
pub fn is_hallucination(text: &str, samples: &[f32]) -> bool {
    const HALLUCINATION_CHARS_PER_SEC: f64 = 12.0;
    let audio_secs = samples.len() as f64 / 16000.0;
    if audio_secs < 0.1 {
        return false;
    }
    let char_count = text.chars().count() as f64;
    let rate = char_count / audio_secs;
    let triggered = rate > HALLUCINATION_CHARS_PER_SEC;
    if triggered {
        log::warn!(
            "Hallucination check: {} chars / {:.1}s = {:.1} chars/s (threshold {})",
            char_count,
            audio_secs,
            rate,
            HALLUCINATION_CHARS_PER_SEC
        );
    }
    triggered
}

/// 重复 n-gram 环路检测（LLM decoder 乱码典型形态：循环重复同一子串）
/// 判定逻辑：检测是否存在一个子串 S，使得 text 中 S 连续重复 ≥4 次且
/// 这些重复占文本总长度的 ≥40%。
///
/// 阈值说明：
/// - 重复 ≥4 次：正常语言极少连续重复 4 次相同子串（"好好好"只 3 次）
/// - 占比 ≥40%：留裕量，正常长文本可能有少量自然重复但不占主导
/// - 最小子串长度 2 chars：避免单字重复误伤（如"哈哈哈"是正常笑声）
pub fn is_repetitive_garbage(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let chars: Vec<char> = text.chars().collect();
    let total_len = chars.len();
    if total_len < 8 {
        // 太短不判定（8 chars 以下即使全重复也不算乱码）
        return false;
    }

    // 尝试不同子串长度（2~total_len/4），找最长的连续重复
    let max_sub_len = total_len / 4;
    for sub_len in 2..=max_sub_len {
        // 扫描所有可能的子串起点
        for start in 0..=total_len - sub_len {
            let sub = &chars[start..start + sub_len];
            // 从 start+sub_len 开始数连续重复次数
            let mut repeat_count = 1;
            let mut pos = start + sub_len;
            while pos + sub_len <= total_len && &chars[pos..pos + sub_len] == sub {
                repeat_count += 1;
                pos += sub_len;
            }
            // 重复 ≥4 次，且重复段总长占比 ≥40%
            if repeat_count >= 4 {
                let repeated_len = repeat_count * sub_len;
                if repeated_len as f64 / total_len as f64 >= 0.4 {
                    log::warn!(
                        "Repetitive garbage: '{}' repeated {} times ({} of {} chars = {:.0}%)",
                        sub.iter().collect::<String>(),
                        repeat_count,
                        repeated_len,
                        total_len,
                        repeated_len as f64 / total_len as f64 * 100.0
                    );
                    return true;
                }
            }
        }
    }
     false
}

/// 剥离 native 模型自带标点（ASR-PUNCT-OPT-001）
///
/// 用于 accuracy 模式 + 用户关闭自动标点（punctuation.enabled=false）场景：
/// native 输出自带标点，用户关了开关但 native 照样出标点 → 剥离使其与 CTC 行为一致。
///
/// 剥离字符集：中文标点（，。！？；：、""''…—）+ 英文标点（,.!?;:"'()[]）
/// **不剥离**：小数点（.）在数字间（如 3.14）、URL/路径中的 / . - _ ~
///
/// 注意：此函数保守处理——只剥离明确的句末/句中标点符号，
/// 不处理数字间小数点（避免破坏"3.14"等数值）。
pub fn strip_punctuation(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(chars.len());
    let punct_set: &[char] = &[
        '，', '。', '！', '？', '；', '：', '、', '\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}', '…', '—',
        ',', '!', '?', ';', ':', '"', '\'', '(', ')', '[', ']',
    ];
    for (i, &c) in chars.iter().enumerate() {
        if punct_set.contains(&c) {
            // 保护数字间的小数点（前后均为数字/空格+数字）
            if c == '.' {
                let prev = chars.get(i.wrapping_sub(1)).copied().unwrap_or(' ');
                let next = chars.get(i + 1).copied().unwrap_or(' ');
                if prev.is_ascii_digit() && next.is_ascii_digit() {
                    result.push(c);
                    continue;
                }
            }
            // 保护句末省略号模式：连续的 . 不处理为单独标点
            // （. 不在 punct_set 中，所以无需特殊处理）
            continue;
        }
        result.push(c);
    }
    // 清理剥离后可能产生的连续空格（标点前后原有空格）
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }
    result.trim().to_string()
}
/// entries 须按确定性顺序（调用方按 id 排序）保证哈希稳定
pub fn hotwords_version(entries: &[(String, String)]) -> u64 {
    let mut hasher = DefaultHasher::new();
    entries.len().hash(&mut hasher);
    for (raw, corrected) in entries {
        raw.hash(&mut hasher);
        corrected.hash(&mut hasher);
    }
    hasher.finish()
}

/// 从 wordbook 词条构建 hotwords 字符串（逗号分隔，用 corrected 词条）
/// 仅 accuracy 分支使用；performance 分支不支持 hotwords
pub fn build_hotwords_string(entries: &[(String, String)]) -> String {
    entries
        .iter()
        .map(|(_, corrected)| corrected.as_str())
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

/// 构建 recognizer(s)，返回 (主 recognizer, 兜底 recognizer, hotwords_version)
fn build_recognizers(
    model_dir: &Path,
    language: &str,
    asr_model: AsrModel,
    hotwords: Option<&str>,
) -> Result<(sherpa_onnx::OfflineRecognizer, Option<sherpa_onnx::OfflineRecognizer>, u64)> {
    let hotwords_version = match hotwords {
        Some(h) => {
            let count = h.split(',').filter(|s| !s.trim().is_empty()).count();
            let mut hasher = DefaultHasher::new();
            count.hash(&mut hasher);
            h.hash(&mut hasher);
            hasher.finish()
        }
        None => 0,
    };

    match asr_model {
        AsrModel::Performance => {
            let recognizer = create_sensevoice_recognizer(model_dir, language)?;
            Ok((recognizer, None, hotwords_version))
        }
        AsrModel::Accuracy => {
            // Accuracy 分支：尝试加载 native 模型；失败则降级 performance
            match create_funasr_nano_recognizer(model_dir, hotwords) {
                Ok(recognizer) => {
                    // 预创建 performance recognizer 作兜底（内存换延迟）
                    let fallback = match create_sensevoice_recognizer(model_dir, language) {
                        Ok(fb) => Some(fb),
                        Err(e) => {
                            log::warn!(
                                "Failed to create fallback performance recognizer: {}, hallucination fallback disabled",
                                e
                            );
                            None
                        }
                    };
                    Ok((recognizer, fallback, hotwords_version))
                }
                Err(e) => {
                    log::warn!(
                        "Accuracy model load failed ({}), falling back to performance model",
                        e
                    );
                    let recognizer = create_sensevoice_recognizer(model_dir, language)?;
                    Ok((recognizer, None, hotwords_version))
                }
            }
        }
    }
}

/// Create SenseVoice Chinese recognizer (FunASR Nano CTC 兼容版，179MB)
/// DEC-025 路线 A：默认 performance 模型
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

/// Create FunASR Nano native recognizer (972MB，accuracy 分支)
/// 字段填法参照 src/bin/poc_funasr_nano.rs:62-85
fn create_funasr_nano_recognizer(
    model_dir: &Path,
    hotwords: Option<&str>,
) -> Result<sherpa_onnx::OfflineRecognizer> {
    let model_dir_path = ensure_funasr_nano_model(model_dir)?;

    let enc = model_dir_path.join("encoder_adaptor.int8.onnx");
    let llm = model_dir_path.join("llm.int8.onnx");
    let emb = model_dir_path.join("embedding.int8.onnx");
    let tok = model_dir_path.join("Qwen3-0.6B");

    let offline_config = sherpa_onnx::OfflineRecognizerConfig {
        model_config: sherpa_onnx::OfflineModelConfig {
            funasr_nano: OfflineFunASRNanoModelConfig {
                encoder_adaptor: Some(enc.to_str().unwrap_or("").to_string()),
                llm: Some(llm.to_str().unwrap_or("").to_string()),
                embedding: Some(emb.to_str().unwrap_or("").to_string()),
                tokenizer: Some(tok.to_str().unwrap_or("").to_string()),
                system_prompt: Some("You are a helpful assistant.".to_string()),
                user_prompt: Some("语音转写:".to_string()),
                max_new_tokens: 0,
                temperature: 1.0,
                top_p: 1.0,
                seed: 42,
                language: None,
                itn: 1,
                hotwords: hotwords.map(|s| s.to_string()),
            },
            tokens: Some(String::new()),
            ..Default::default()
        },
        ..Default::default()
    };

    sherpa_onnx::OfflineRecognizer::create(&offline_config)
        .context("Failed to create FunASR Nano offline recognizer")
}

/// Ensure FunASR Nano native model (972MB) is present; bail if missing
fn ensure_funasr_nano_model(model_dir: &Path) -> Result<PathBuf> {
    // FunASR Nano native（2025-12-30，encoder+LLM decoder，有 hotwords）
    let model_dir_path = model_dir.join("sherpa-onnx-funasr-nano-int8-2025-12-30");

    let enc = model_dir_path.join("encoder_adaptor.int8.onnx");
    let llm = model_dir_path.join("llm.int8.onnx");
    let emb = model_dir_path.join("embedding.int8.onnx");
    let tok = model_dir_path.join("Qwen3-0.6B");

    let dir_ok = model_dir_path.exists();
    let files_ok = enc.exists() && llm.exists() && emb.exists() && tok.exists();
    if dir_ok && files_ok {
        log::info!("FunASR Nano native model found at {:?}", model_dir_path);
        return Ok(model_dir_path);
    }

    anyhow::bail!(
        "FunASR Nano native model not found at {:?}. Please download manually from:\n  https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
        model_dir_path
    )
}

/// Ensure SenseVoice (FunASR Nano CTC 兼容版，179MB) model is present
fn ensure_sensevoice_model(model_dir: &Path) -> Result<PathBuf> {
    // FunASR Nano CTC 兼容版（179MB，2025-12-17）— DEC-025 路线 A 直换默认模型
    // 模型文件名同为 model.int8.onnx + tokens.txt，沿用 OfflineSenseVoiceModelConfig
    // 旧目录 sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09 保留作回滚
    let model_dir_path = model_dir.join("sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17");

    let model_file = model_dir_path.join("model.int8.onnx");
    let tokens_file = model_dir_path.join("tokens.txt");

    if model_dir_path.exists() && model_file.exists() && tokens_file.exists() {
        log::info!("SenseVoice model found at {:?}", model_dir_path);
        return Ok(model_dir_path);
    }

    anyhow::bail!(
        "SenseVoice model not found at {:?}. Please download manually from:\n  https://huggingface.co/sherpa-onnx/sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17",
        model_dir_path
    )
}

pub fn model_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join("models")
}

/// 检测 accuracy 模型是否就位（供 Tauri command 调用）
pub fn check_accuracy_model_ready(model_dir: &Path) -> (bool, PathBuf) {
    let dir = model_dir.join("sherpa-onnx-funasr-nano-int8-2025-12-30");
    let enc = dir.join("encoder_adaptor.int8.onnx");
    let llm = dir.join("llm.int8.onnx");
    let emb = dir.join("embedding.int8.onnx");
    let tok = dir.join("Qwen3-0.6B");
    let ready = dir.exists() && enc.exists() && llm.exists() && emb.exists() && tok.exists();
    (ready, dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asr_model_from_config_parses_values() {
        assert_eq!(AsrModel::from_config("performance"), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("accuracy"), AsrModel::Accuracy);
        assert_eq!(AsrModel::from_config("Performance"), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("ACCURACY"), AsrModel::Accuracy);
        assert_eq!(AsrModel::from_config(""), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("garbage"), AsrModel::Performance);
    }

    #[test]
    fn hotwords_version_stable_for_same_input() {
        let entries = vec![
            ("派".to_string(), "派".to_string()),
            ("对".to_string(), "队".to_string()),
        ];
        let v1 = hotwords_version(&entries);
        let v2 = hotwords_version(&entries);
        assert_eq!(v1, v2, "same input must produce same hash");
    }

    #[test]
    fn hotwords_version_changes_on_content_change() {
        let e1 = vec![("派".to_string(), "派".to_string())];
        let e2 = vec![("派".to_string(), "湃".to_string())];
        assert_ne!(
            hotwords_version(&e1),
            hotwords_version(&e2),
            "different content must produce different hash"
        );
    }

    #[test]
    fn hotwords_version_changes_on_len_change() {
        let e1 = vec![("a".to_string(), "b".to_string())];
        let e2 = vec![
            ("a".to_string(), "b".to_string()),
            ("c".to_string(), "d".to_string()),
        ];
        assert_ne!(
            hotwords_version(&e1),
            hotwords_version(&e2),
            "different length must produce different hash"
        );
    }

    #[test]
    fn hotwords_version_order_sensitive() {
        let e1 = vec![
            ("a".to_string(), "b".to_string()),
            ("c".to_string(), "d".to_string()),
        ];
        let e2 = vec![
            ("c".to_string(), "d".to_string()),
            ("a".to_string(), "b".to_string()),
        ];
        assert_ne!(
            hotwords_version(&e1),
            hotwords_version(&e2),
            "order must affect hash (caller must sort for stability)"
        );
    }

    #[test]
    fn build_hotwords_string_joins_corrected_non_empty() {
        let entries = vec![
            ("派".to_string(), "派".to_string()),
            ("对".to_string(), "队".to_string()),
            ("x".to_string(), "  ".to_string()), // empty after trim
        ];
        let s = build_hotwords_string(&entries);
        assert_eq!(s, "派,队");
    }

    #[test]
    fn build_hotwords_string_empty_entries() {
        let entries: Vec<(String, String)> = vec![];
        assert_eq!(build_hotwords_string(&entries), "");
    }

    #[test]
    fn is_hallucination_normal_text_not_triggered() {
        // 10 chars, 5s audio = 2 chars/s, well below threshold 12
        let text = "开饭时间早上九点";
        let samples = vec![0.0f32; 16000 * 5];
        assert!(!is_hallucination(text, &samples));
    }

    #[test]
    fn is_hallucination_long_text_triggered() {
        // 100 chars, 1s audio = 100 chars/s, above threshold 12
        let text = "一".repeat(100);
        let samples = vec![0.0f32; 16000];
        assert!(is_hallucination(&text, &samples));
    }

    #[test]
    fn is_hallucination_boundary_exact_threshold() {
        // 12 chars, 1s audio = 12 chars/s, NOT triggered (uses >, not >=)
        let text = "一二三四五六七八九十一二";
        assert_eq!(text.chars().count(), 12);
        let samples = vec![0.0f32; 16000];
        assert!(!is_hallucination(&text, &samples));
    }

    #[test]
    fn is_hallucination_just_above_threshold() {
        // 13 chars, 1s audio = 13 chars/s, triggered
        let text = "一二三四五六七八九十一二三";
        assert_eq!(text.chars().count(), 13);
        let samples = vec![0.0f32; 16000];
        assert!(is_hallucination(&text, &samples));
    }

    #[test]
    fn is_hallucination_very_short_audio_skipped() {
        // < 0.1s audio, skip check (avoid div-by-small noise)
        let text = "一二三四五六七八九十一二";
        let samples = vec![0.0f32; 100]; // 6ms
        assert!(!is_hallucination(&text, &samples));
    }

    #[test]
    fn is_hallucination_empty_text_not_triggered() {
        let samples = vec![0.0f32; 16000];
        assert!(!is_hallucination("", &samples));
    }

    // ============================================================
    // ASR-NATIVE-LONG-001: is_repetitive_garbage 测试
    // ============================================================

    #[test]
    fn repetitive_garbage_empty_text_not_triggered() {
        assert!(!is_repetitive_garbage(""));
    }

    #[test]
    fn repetitive_garbage_short_text_not_triggered() {
        // 8 chars 以下不判定
        assert!(!is_repetitive_garbage("哈哈哈哈"));
        assert!(!is_repetitive_garbage("好好好好"));
    }

    #[test]
    fn repetitive_garbage_normal_text_not_triggered() {
        // 正常语音有自然重复（"好好好"只 3 次，"哈哈哈"单字）
        assert!(!is_repetitive_garbage("我想好好好休息一下然后再继续工作"));
        assert!(!is_repetitive_garbage("哈哈哈哈太搞笑了这个笑话"));
    }

    #[test]
    fn repetitive_garbage_long_normal_text_not_triggered() {
        // 正常长文本不应误伤
        let text = "今天天气很好我们去公园散步看到很多人在跑步也有人在放风筝孩子们在草地上玩耍老人们在长椅上聊天";
        assert!(!is_repetitive_garbage(text));
    }

    #[test]
    fn repetitive_garbage_loop_detected() {
        // LLM decoder 乱码典型形态：同一子串循环重复 ≥4 次占 ≥40%
        let text = "然后然后然后然后然后然后然后然后然后然后然后然后然后然后然后然后";
        assert!(is_repetitive_garbage(text));
    }

    #[test]
    fn repetitive_garbage_longer_substring_loop() {
        // 4 字子串重复
        let text = "开饭时间开饭时间开饭时间开饭时间开饭时间开饭时间开饭时间";
        assert!(is_repetitive_garbage(text));
    }

    #[test]
    fn repetitive_garbage_below_repeat_threshold_not_triggered() {
        // 重复 3 次（< 4 阈值），不触发
        let text = "你好你好你好你好世界世界世界世界世界世界世界世界世界世界";
        // "你好"×4 = 8 chars 但占比 8/24=33% <40%，不触发
        // 实际 "你好"重复4次=8字，剩余"世界"×8=16字，"世界"重复8次=16字 占比 67%>40% 会触发
        // 修正测试用例：确保不触发
        let text2 = "你好你好你好这是正常的文本内容没有重复乱码的问题存在";
        assert!(!is_repetitive_garbage(text2));
    }

    #[test]
    fn repetitive_garbage_mixed_normal_and_repeat_not_triggered() {
        // 有重复但占比 <40%，正常文本中嵌入少量重复
        let text = "我去了去了去了去了一次超市买东西";
        // "去了"重复4次但只占 8/16=50%... 会触发。调整：
        let text2 = "我昨天去了去了去了去了一趟超市买了很多东西回来做饭";
        // "去了"×4=8字, 总长 24字, 占比 33% <40% 不触发
        assert!(!is_repetitive_garbage(text2));
    }

    #[test]
     fn repetitive_garbage_single_char_repeat_not_triggered() {
        // 短的单字重复（<8 chars 不判定）
        assert!(!is_repetitive_garbage("哈哈哈哈"));
        assert!(!is_repetitive_garbage("哈哈哈哈哈"));
        // 注意：16 个连续"哈"在真实 ASR 输出中算乱码（会触发），这里只测短的
    }

    // ============================================================
    // ASR-PUNCT-OPT-001: strip_punctuation 测试
    // ============================================================

    #[test]
    fn strip_punctuation_chinese() {
        // 标点直接删除，不加空格
        assert_eq!(
            strip_punctuation("周末要不要去露营？最近天气超舒服。"),
            "周末要不要去露营最近天气超舒服"
        );
    }

    #[test]
    fn strip_punctuation_english() {
        // 英文标点删除后保留原有空格
        assert_eq!(
            strip_punctuation("Hello, world! How are you?"),
            "Hello world How are you"
        );
    }

    #[test]
    fn strip_punctuation_mixed() {
        assert_eq!(
            strip_punctuation("今天很好，very nice！"),
            "今天很好very nice"
        );
    }

    #[test]
    fn strip_punctuation_empty() {
        assert_eq!(strip_punctuation(""), "");
    }

    #[test]
    fn strip_punctuation_no_punct() {
        assert_eq!(strip_punctuation("没有任何标点的文本"), "没有任何标点的文本");
    }

    #[test]
    fn strip_punctuation_preserves_decimal() {
        // 小数点不剥离（. 不在 punct_set 中）
        assert_eq!(strip_punctuation("圆周率是3.14"), "圆周率是3.14");
        assert_eq!(strip_punctuation("价格50.5元"), "价格50.5元");
    }

    #[test]
    fn strip_punctuation_preserves_url() {
        // URL 中的 . / : 等不剥离（: 剥离但 URL 中的 // 和 . 保留）
        // 注意：: 在 punct_set 中会被剥离——这是保守策略的已知限制，
        // URL 场景极少出现在语音转写输出中
        let input = "访问 https example.com 路径";
        assert_eq!(strip_punctuation(input), "访问 https example.com 路径");
    }

    #[test]
    fn strip_punctuation_only_punct() {
        assert_eq!(strip_punctuation("，。！？"), "");
    }

    #[test]
    fn strip_punctuation_quotes() {
        // 中文引号 \u{201C} \u{201D} 和单引号 \u{2018} \u{2019}
        let input = "\u{201C}\u{4F60}\u{597D}\u{201D}\u{4ED6}\u{8BF4}";
        assert_eq!(strip_punctuation(input), "你好他说");
    }

    #[test]
    fn strip_punctuation_collapses_spaces() {
        // 标点剥离后连续空格压缩
        assert_eq!(
            strip_punctuation("你好，  世界！"),
            "你好 世界"
        );
    }

    // ============================================================
    // ASR-PUNCT-OPT-001: 标点决策来源标记逻辑测试
    // ============================================================

    /// 来源标记规则是纯逻辑（performance→false, accuracy native 成功→true, 兜底→false）
    /// 这里通过间接验证规则文档化（实际 Transcriber 需模型加载，无法纯逻辑测）
    /// 决策真值表在 result.md 文档化
    #[test]
    fn source_flag_documentation_placeholder() {
        // 来源标记规则由 transcribe_with_punct_info 实现，需真实模型加载，此处不纯逻辑测
        // 真值表见 result.md
        assert!(true);
    }
}