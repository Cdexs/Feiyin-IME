use std::{
    ffi::{c_char, c_int, c_long, c_void, CStr, CString, NulError},
    path::{Path, PathBuf},
    ptr::{self, NonNull},
};

use anyhow::{anyhow, Context, Result};
use ctranslate2::{ComputeType, Device, TranslationOptions, TranslatorConfig};
use ctranslate2_sys::{
    free_pointer_array, translation_result_free, translation_result_output_at,
    translation_result_output_size, translation_result_score, translator_create,
    translator_destroy, translator_translate_batch, CTranslationOptions, CTranslationResult,
    CTranslator,
};
use sentencepiece::SentencePieceProcessor;

use crate::config::TranslationLanguage;

const ZH_EN_SUBDIR: &str = "opus-mt-zh-en";
const EN_ZH_SUBDIR: &str = "opus-mt-en-zh";

const ZH_EN_CT2_BASE_URL: &str =
    "https://huggingface.co/gaudi/opus-mt-zh-en-ctranslate2/resolve/main";
const EN_ZH_CT2_BASE_URL: &str =
    "https://huggingface.co/gaudi/opus-mt-en-zh-ctranslate2/resolve/main";
const ZH_EN_SPM_BASE_URL: &str = "https://huggingface.co/Helsinki-NLP/opus-mt-zh-en/resolve/main";
const EN_ZH_SPM_BASE_URL: &str = "https://huggingface.co/Helsinki-NLP/opus-mt-en-zh/resolve/main";

const CONFIG_JSON: &str = "config.json";
const MODEL_BIN: &str = "model.bin";
const SHARED_VOCABULARY_JSON: &str = "shared_vocabulary.json";
const SOURCE_SPM: &str = "source.spm";
const TARGET_SPM: &str = "target.spm";
const EOS_TOKEN: &str = "</s>";

const MAX_DECODE_STEPS: usize = 512;
const BEAM_WIDTH: usize = 6;
const NO_REPEAT_NGRAM_SIZE: usize = 0;
const LENGTH_PENALTY_ALPHA: f64 = 1.5;
const COVERAGE_PENALTY: f64 = 0.05;
const MIN_SEGMENT_CHARS: usize = 120;
const MAX_SEGMENT_CHARS: usize = 200;
const MAX_SENTENCES_PER_SEGMENT: usize = 3;
const SINGLE_BATCH_SIZE: usize = 1;
const BATCH_TYPE_EXAMPLES: c_int = 0;

struct MarianModel {
    path: PathBuf,
    translator: Ct2Translator,
    tokenizer: MarianTokenizer,
}

pub struct TranslationEngine {
    model: MarianModel,
    direction: TranslationLanguage,
}

struct MarianTokenizer {
    encoder: SentencePieceProcessor,
    decoder: SentencePieceProcessor,
}

struct Ct2Translator {
    inner: NonNull<CTranslator>,
}

struct OwnedTranslationResult {
    inner: *mut CTranslationResult,
}

#[derive(Debug)]
enum Ct2TranslatorError {
    NulInPath(NulError),
    CreationFailed,
    NulInToken { token: String, source: NulError },
    NullResults,
}

impl std::fmt::Display for Ct2TranslatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NulInPath(err) => write!(f, "invalid model path (contains null byte): {}", err),
            Self::CreationFailed => write!(f, "failed to create the CT2 translator"),
            Self::NulInToken { token, source } => {
                write!(
                    f,
                    "invalid source token {:?} (contains null byte): {}",
                    token, source
                )
            }
            Self::NullResults => write!(f, "CT2 returned a null results pointer"),
        }
    }
}

impl std::error::Error for Ct2TranslatorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NulInPath(err) => Some(err),
            Self::NulInToken { source, .. } => Some(source),
            Self::CreationFailed | Self::NullResults => None,
        }
    }
}

impl Drop for Ct2Translator {
    fn drop(&mut self) {
        unsafe {
            translator_destroy(self.inner.as_ptr());
        }
    }
}

impl Drop for OwnedTranslationResult {
    fn drop(&mut self) {
        unsafe {
            translation_result_free(self.inner);
        }
    }
}

impl OwnedTranslationResult {
    fn output(&self) -> Vec<String> {
        unsafe {
            let len = translation_result_output_size(self.inner);
            let mut out = Vec::with_capacity(len);
            for idx in 0..len {
                let ptr = translation_result_output_at(self.inner, idx);
                out.push(CStr::from_ptr(ptr).to_string_lossy().to_string());
            }
            out
        }
    }

    #[allow(dead_code)]
    fn score(&self) -> f32 {
        unsafe { translation_result_score(self.inner) }
    }
}

impl Ct2Translator {
    fn new<P: AsRef<Path>>(
        model_path: P,
        config: &TranslatorConfig,
    ) -> Result<Self, Ct2TranslatorError> {
        let c_model = CString::new(model_path.as_ref().to_string_lossy().into_owned())
            .map_err(Ct2TranslatorError::NulInPath)?;

        let raw = unsafe {
            translator_create(
                c_model.as_ptr(),
                config.device as c_int,
                config.compute_type as c_int,
                config.device_indices.as_ptr(),
                config.device_indices.len(),
                config.tensor_parallel as c_int,
                config.num_threads_per_replica,
                config.max_queued_batches as c_long,
                config.cpu_core_offset as c_int,
            )
        };

        let inner = NonNull::new(raw).ok_or(Ct2TranslatorError::CreationFailed)?;
        Ok(Self { inner })
    }

    fn translate_single(
        &self,
        tokens: &[String],
        options: &TranslationOptions,
    ) -> Result<OwnedTranslationResult, Ct2TranslatorError> {
        let c_tokens = tokens
            .iter()
            .map(|token| {
                CString::new(token.as_str()).map_err(|source| Ct2TranslatorError::NulInToken {
                    token: token.clone(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut token_ptrs: Vec<*const c_char> =
            c_tokens.iter().map(|token| token.as_ptr()).collect();
        token_ptrs.push(ptr::null());
        let mut sources = [token_ptrs.as_ptr()];

        let c_options = to_c_translation_options(options);
        let mut out_num_translations = 0usize;

        let results_ptr = unsafe {
            // Safety:
            // - `c_tokens` owns every CString and lives until the FFI call returns.
            // - `token_ptrs` contains pointers into `c_tokens` plus a trailing null sentinel.
            // - `sources` points at `token_ptrs` and also stays alive for the full call.
            // - We intentionally use `max_batch_size = 1` because this code path only supports
            //   a single source sentence and the C wrapper rejects `0`.
            translator_translate_batch(
                self.inner.as_ptr(),
                sources.as_mut_ptr() as *mut *mut *const c_char,
                SINGLE_BATCH_SIZE,
                &c_options,
                SINGLE_BATCH_SIZE,
                BATCH_TYPE_EXAMPLES,
                &mut out_num_translations,
            )
        };

        if results_ptr.is_null() {
            return Err(Ct2TranslatorError::NullResults);
        }

        let results = unsafe { take_translation_results(results_ptr, out_num_translations) };
        results
            .into_iter()
            .next()
            .ok_or(Ct2TranslatorError::NullResults)
    }
}

unsafe fn take_translation_results(
    results_ptr: *mut *mut CTranslationResult,
    len: usize,
) -> Vec<OwnedTranslationResult> {
    let raw_results = std::slice::from_raw_parts(results_ptr, len).to_vec();
    free_pointer_array(results_ptr as *mut *mut c_void);
    raw_results
        .into_iter()
        .map(|inner| OwnedTranslationResult { inner })
        .collect()
}

fn to_c_translation_options(options: &TranslationOptions) -> CTranslationOptions {
    CTranslationOptions {
        beam_size: options.beam_size,
        patience: options.patience,
        length_penalty: options.length_penalty,
        coverage_penalty: options.coverage_penalty,
        repetition_penalty: options.repetition_penalty,
        no_repeat_ngram_size: options.no_repeat_ngram_size,
        disable_unk: if options.disable_unk { 1 } else { 0 },
        max_input_length: options.max_input_length,
        max_decoding_length: options.max_decoding_length,
        min_decoding_length: options.min_decoding_length,
        sampling_topk: options.sampling_topk,
        return_end_token: options.return_end_token,
        prefix_bias_beta: options.prefix_bias_beta,
        sampling_topp: options.sampling_topp,
        sampling_temperature: options.sampling_temperature,
        use_vmap: if options.use_vmap { 1 } else { 0 },
        num_hypotheses: options.num_hypotheses,
        return_scores: if options.return_scores { 1 } else { 0 },
        return_attention: if options.return_attention { 1 } else { 0 },
        return_logits_vocab: if options.return_logits_vocab { 1 } else { 0 },
        return_alternatives: if options.return_alternatives { 1 } else { 0 },
        min_alternative_expansion_prob: options.min_alternative_expansion_prob,
        replace_unknowns: if options.replace_unknowns { 1 } else { 0 },
    }
}

impl MarianTokenizer {
    fn new(path: &Path, label: &str) -> Result<Self> {
        let source_path = path.join(SOURCE_SPM);
        let target_path = path.join(TARGET_SPM);

        let encoder = SentencePieceProcessor::open(&source_path).map_err(|err| {
            anyhow!(
                "failed to load {} source sentencepiece model from {}: {}",
                label,
                source_path.display(),
                err
            )
        })?;
        let decoder = SentencePieceProcessor::open(&target_path).map_err(|err| {
            anyhow!(
                "failed to load {} target sentencepiece model from {}: {}",
                label,
                target_path.display(),
                err
            )
        })?;

        Ok(Self { encoder, decoder })
    }

    fn encode(&self, input: &str) -> Result<Vec<String>> {
        let mut tokens: Vec<String> = self
            .encoder
            .encode(input)
            .map_err(|err| anyhow!("failed to encode sentencepiece input: {}", err))?
            .into_iter()
            .map(|piece| piece.piece)
            .collect();
        tokens.push(EOS_TOKEN.to_string());
        Ok(tokens)
    }

    fn decode(&self, tokens: &[String]) -> Result<String> {
        let filtered: Vec<&str> = tokens
            .iter()
            .map(String::as_str)
            .filter(|token| !matches!(*token, "<pad>" | "</s>" | "<s>"))
            .collect();

        self.decoder
            .decode_pieces(&filtered)
            .map_err(|err| anyhow!("failed to decode sentencepiece output: {}", err))
    }
}

fn segment_text(text: &str) -> Vec<String> {
    let sentences: Vec<&str> = text
        .split_inclusive(&['。', '！', '？', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if sentences.len() <= 1 {
        return vec![text.to_string()];
    }

    let mut segments = Vec::new();
    let mut buf = String::new();
    let mut sentence_count = 0;

    for s in sentences {
        buf.push_str(s);
        sentence_count += 1;
        if buf.chars().count() >= MAX_SEGMENT_CHARS
            || sentence_count >= MAX_SENTENCES_PER_SEGMENT
        {
            segments.push(buf.clone());
            buf.clear();
            sentence_count = 0;
        }
    }

    if !buf.is_empty() {
        if let Some(last) = segments.last_mut() {
            last.push(' ');
            last.push_str(&buf);
        } else {
            segments.push(buf);
        }
    }

    segments
}

impl MarianModel {
    fn new(model_dir: &Path, subdir: &str, label: &str) -> Result<Self> {
        let path = model_dir.join(subdir);
        validate_runtime_files(&path, label)?;
        let tokenizer = MarianTokenizer::new(&path, label)?;

        let config = TranslatorConfig {
            device: Device::Cpu,
            compute_type: ComputeType::Default,
            ..TranslatorConfig::default()
        };
        let translator = Ct2Translator::new(&path, &config)
            .map_err(|err| anyhow!("failed to initialize {} CT2 translator: {}", label, err))?;

        log::info!("{} CT2 model initialized at {}", label, path.display());

        Ok(Self {
            path,
            translator,
            tokenizer,
        })
    }

    fn translate(&self, text: &str) -> Result<String> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(String::new());
        }

        let chars = text.chars().count();
        let sentence_count = text.matches(&['。', '！', '？']).count();
        let needs_segmentation = chars > MIN_SEGMENT_CHARS && sentence_count >= 2;

        if needs_segmentation {
            let segments = segment_text(text);
            log::info!(
                "CT2 long text segmented: {} parts from {} chars / {} sentences",
                segments.len(),
                chars,
                sentence_count,
            );
            let parts: Vec<String> = segments
                .iter()
                .enumerate()
                .filter_map(|(i, seg)| {
                    match self.translate_segment(seg) {
                        Ok(translated) if !translated.is_empty() => {
                            log::info!("CT2 segment {}/{} done", i + 1, segments.len());
                            Some(translated)
                        }
                        Ok(_) => {
                            log::warn!("CT2 segment {}/{} returned empty", i + 1, segments.len());
                            None
                        }
                        Err(e) => {
                            log::error!("CT2 segment {}/{} failed: {}", i + 1, segments.len(), e);
                            None
                        }
                    }
                })
                .collect();

            if parts.is_empty() {
                anyhow::bail!("all {} segments failed to translate", segments.len());
            }
            Ok(parts.join(" "))
        } else {
            self.translate_segment(text)
        }
    }

    fn translate_segment(&self, text: &str) -> Result<String> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(String::new());
        }

        log::debug!("CT2 translating segment with model at {}", self.path.display());
        let source_tokens = self.tokenizer.encode(text)?;
        log::info!("CT2 source tokens: {:?}", source_tokens);

        let options = TranslationOptions {
            beam_size: BEAM_WIDTH,
            length_penalty: LENGTH_PENALTY_ALPHA as f32,
            coverage_penalty: COVERAGE_PENALTY as f32,
            min_decoding_length: std::cmp::max(1, source_tokens.len() / 2),
            no_repeat_ngram_size: NO_REPEAT_NGRAM_SIZE,
            max_decoding_length: MAX_DECODE_STEPS,
            max_input_length: 0,
            ..TranslationOptions::default()
        };
        let result = self
            .translator
            .translate_single(&source_tokens, &options)
            .with_context(|| {
                format!(
                    "failed to translate with CT2 model at {}",
                    self.path.display()
                )
            })?;
        let result_outputs = result.output();
        log::info!("CT2 result.output_size = {}", result_outputs.len());
        for (i, token) in result_outputs.iter().enumerate() {
            log::info!("CT2 token {}: {:?}", i, token);
        }
        let decoded = self
            .tokenizer
            .decode(&result_outputs)
            .context("failed to decode translation output")?;
        log::info!("CT2 decoded output: {:?}", decoded);
        Ok(decoded)
    }
}

impl TranslationEngine {
    pub fn new(model_dir: &Path, target: TranslationLanguage) -> Result<Self> {
        let (subdir, label) = match target {
            TranslationLanguage::English => (ZH_EN_SUBDIR, "zh-en"),
            TranslationLanguage::Chinese => (EN_ZH_SUBDIR, "en-zh"),
        };

        let model = MarianModel::new(model_dir, subdir, label)?;
        Ok(Self {
            model,
            direction: target,
        })
    }

    pub fn translate(&self, text: &str) -> Result<String> {
        self.model.translate(text)
    }

    pub fn is_available(model_dir: &Path, target: TranslationLanguage) -> bool {
        let subdir = match target {
            TranslationLanguage::English => ZH_EN_SUBDIR,
            TranslationLanguage::Chinese => EN_ZH_SUBDIR,
        };
        let path = model_dir.join(subdir);

        minimum_runtime_files()
            .iter()
            .all(|relative| path.join(relative).is_file())
    }

    pub fn model_files() -> Vec<(String, String)> {
        let mut files = Vec::new();
        append_model_files(
            &mut files,
            ZH_EN_SUBDIR,
            ZH_EN_CT2_BASE_URL,
            ZH_EN_SPM_BASE_URL,
        );
        append_model_files(
            &mut files,
            EN_ZH_SUBDIR,
            EN_ZH_CT2_BASE_URL,
            EN_ZH_SPM_BASE_URL,
        );
        files
    }

    pub fn direction(&self) -> TranslationLanguage {
        self.direction
    }
}

fn append_model_files(
    files: &mut Vec<(String, String)>,
    subdir: &str,
    ct2_base_url: &str,
    spm_base_url: &str,
) {
    for filename in [CONFIG_JSON, MODEL_BIN, SHARED_VOCABULARY_JSON] {
        files.push((
            format!("{subdir}/{filename}"),
            format!("{ct2_base_url}/{filename}"),
        ));
    }
    for filename in [SOURCE_SPM, TARGET_SPM] {
        files.push((
            format!("{subdir}/{filename}"),
            format!("{spm_base_url}/{filename}"),
        ));
    }
}

fn minimum_runtime_files() -> [&'static str; 3] {
    [MODEL_BIN, SOURCE_SPM, TARGET_SPM]
}

fn required_runtime_files() -> [&'static str; 5] {
    [
        CONFIG_JSON,
        MODEL_BIN,
        SHARED_VOCABULARY_JSON,
        SOURCE_SPM,
        TARGET_SPM,
    ]
}

fn validate_runtime_files(path: &Path, label: &str) -> Result<()> {
    for relative in required_runtime_files() {
        let file = path.join(relative);
        if !file.is_file() {
            return Err(anyhow!(
                "missing {} runtime file for {} model: {}",
                relative,
                label,
                file.display()
            ));
        }
    }

    Ok(())
}

fn normalize_translation_output(text: &str) -> String {
    text.replace("<pad>", "")
        .replace("</s>", "")
        .replace("<s>", "")
        .replace('\u{2581}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let counter = UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!("voice-ime-{prefix}-{timestamp}-{counter}"))
    }

    fn write_placeholder_files(model_dir: &Path, subdir: &str, files: &[&str]) {
        let base = model_dir.join(subdir);
        fs::create_dir_all(&base).expect("create model base directory");

        for relative in files {
            let file = base.join(relative);
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(file, b"placeholder").expect("write placeholder file");
        }
    }

    #[test]
    fn translate_returns_err_without_model_files() {
        let dir = unique_temp_dir("ct2-no-model");
        assert!(!TranslationEngine::is_available(
            &dir,
            TranslationLanguage::English
        ));
        assert!(TranslationEngine::new(&dir, TranslationLanguage::English).is_err());
    }

    #[test]
    fn model_files_covers_both_translation_directions() {
        let files = TranslationEngine::model_files();
        let has_zh_en = files
            .iter()
            .any(|(name, url)| name.contains(ZH_EN_SUBDIR) && url.contains("opus-mt-zh-en"));
        let has_en_zh = files
            .iter()
            .any(|(name, url)| name.contains(EN_ZH_SUBDIR) && url.contains("opus-mt-en-zh"));

        assert!(has_zh_en, "should include zh-en model files");
        assert!(has_en_zh, "should include en-zh model files");
    }

    #[test]
    fn each_direction_has_five_model_files() {
        let files = TranslationEngine::model_files();
        let zh_en_files: Vec<_> = files
            .iter()
            .filter(|(name, _)| name.contains(ZH_EN_SUBDIR))
            .collect();
        let en_zh_files: Vec<_> = files
            .iter()
            .filter(|(name, _)| name.contains(EN_ZH_SUBDIR))
            .collect();

        assert_eq!(zh_en_files.len(), 5, "zh-en should have 5 files");
        assert_eq!(en_zh_files.len(), 5, "en-zh should have 5 files");
    }

    #[test]
    fn model_files_use_ct2_and_sentencepiece_sources() {
        let files = TranslationEngine::model_files();
        let zh_en_source_spm = files
            .iter()
            .find(|(name, _)| name == &format!("{ZH_EN_SUBDIR}/{SOURCE_SPM}"))
            .expect("zh-en source.spm entry");
        let zh_en_target_spm = files
            .iter()
            .find(|(name, _)| name == &format!("{ZH_EN_SUBDIR}/{TARGET_SPM}"))
            .expect("zh-en target.spm entry");
        let en_zh_source_spm = files
            .iter()
            .find(|(name, _)| name == &format!("{EN_ZH_SUBDIR}/{SOURCE_SPM}"))
            .expect("en-zh source.spm entry");
        let en_zh_target_spm = files
            .iter()
            .find(|(name, _)| name == &format!("{EN_ZH_SUBDIR}/{TARGET_SPM}"))
            .expect("en-zh target.spm entry");

        assert!(
            zh_en_source_spm.1.contains("Helsinki-NLP/opus-mt-zh-en"),
            "zh-en source.spm should come from Helsinki"
        );
        assert!(
            zh_en_target_spm.1.contains("Helsinki-NLP/opus-mt-zh-en"),
            "zh-en target.spm should come from Helsinki"
        );
        assert!(
            en_zh_source_spm.1.contains("Helsinki-NLP/opus-mt-en-zh"),
            "en-zh source.spm should come from Helsinki"
        );
        assert!(
            en_zh_target_spm.1.contains("Helsinki-NLP/opus-mt-en-zh"),
            "en-zh target.spm should come from Helsinki"
        );
        assert!(files.iter().any(|(name, url)| {
            name == &format!("{ZH_EN_SUBDIR}/{MODEL_BIN}")
                && url.contains("gaudi/opus-mt-zh-en-ctranslate2")
        }));
        assert!(files.iter().any(|(name, url)| {
            name == &format!("{EN_ZH_SUBDIR}/{MODEL_BIN}")
                && url.contains("gaudi/opus-mt-en-zh-ctranslate2")
        }));
    }

    #[test]
    fn is_available_requires_model_bin_and_sentencepiece_models() {
        let dir = unique_temp_dir("ct2-availability");
        write_placeholder_files(&dir, ZH_EN_SUBDIR, &minimum_runtime_files());

        assert!(
            TranslationEngine::is_available(&dir, TranslationLanguage::English),
            "model.bin + source.spm + target.spm should satisfy availability"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn is_available_is_direction_specific() {
        let dir = unique_temp_dir("ct2-direction");
        write_placeholder_files(&dir, ZH_EN_SUBDIR, &minimum_runtime_files());

        assert!(TranslationEngine::is_available(
            &dir,
            TranslationLanguage::English
        ));
        assert!(!TranslationEngine::is_available(
            &dir,
            TranslationLanguage::Chinese
        ));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn normalize_translation_output_strips_special_tokens() {
        let output = normalize_translation_output("<pad> hello world </s>");
        assert_eq!(output, "hello world");
    }

    #[test]
    fn normalize_translation_output_reconstructs_metaspace_word_boundaries() {
        let output = normalize_translation_output("<pad>▁I▁am▁happy</s>");
        assert_eq!(output, "I am happy");
    }

    #[test]
    fn required_runtime_files_include_sentencepiece_assets() {
        assert_eq!(
            minimum_runtime_files(),
            [MODEL_BIN, SOURCE_SPM, TARGET_SPM],
            "minimum availability should require CT2 weights plus both sentencepiece files"
        );
        assert_eq!(
            required_runtime_files(),
            [
                CONFIG_JSON,
                MODEL_BIN,
                SHARED_VOCABULARY_JSON,
                SOURCE_SPM,
                TARGET_SPM,
            ],
            "runtime validation should require config, weights, vocabulary, and both sentencepiece files"
        );
    }

    #[test]
    fn beam_width_is_six() {
        assert_eq!(BEAM_WIDTH, 6);
    }

    #[test]
    fn length_penalty_alpha_is_one_point_five() {
        assert!((LENGTH_PENALTY_ALPHA - 1.5_f64).abs() < f64::EPSILON);
    }

    #[test]
    fn translation_options_all_parameters_set() {
        let options = TranslationOptions {
            beam_size: BEAM_WIDTH,
            length_penalty: LENGTH_PENALTY_ALPHA as f32,
            coverage_penalty: COVERAGE_PENALTY as f32,
            min_decoding_length: 10,
            no_repeat_ngram_size: NO_REPEAT_NGRAM_SIZE,
            max_decoding_length: MAX_DECODE_STEPS,
            max_input_length: 0,
            ..TranslationOptions::default()
        };

        assert_eq!(options.beam_size, 6, "beam size should be 6");
        assert!(
            (options.length_penalty - 1.5).abs() < f32::EPSILON,
            "length penalty alpha should be 1.5"
        );
        assert!(
            (options.coverage_penalty - 0.05).abs() < f32::EPSILON,
            "coverage penalty should be 0.05"
        );
        assert_eq!(
            options.min_decoding_length, 10,
            "min decoding length should be set"
        );
        assert_eq!(
            options.no_repeat_ngram_size, 0,
            "no repeat ngram size should be 0 (disabled) to avoid premature translation termination"
        );
        assert_eq!(
            options.max_decoding_length, 512,
            "max decoding length should be 512"
        );
        assert_eq!(
            options.max_input_length, 0,
            "max input length should be 0 (unlimited, input bounded by MAX_RECORD_SECONDS)"
        );
    }

    #[test]
    fn no_repeat_ngram_size_is_zero() {
        assert_eq!(NO_REPEAT_NGRAM_SIZE, 0);
    }

    #[test]
    fn max_decoding_length_is_512() {
        assert_eq!(MAX_DECODE_STEPS, 512);
    }

    // ============================================================
    // TRANS-SEGMENT-001: segmentation + parameter tuning tests
    // ============================================================

    #[test]
    fn segment_text_returns_single_segment_for_short_text() {
        let text = "你好世界。";
        let segments = segment_text(text);
        assert_eq!(segments.len(), 1, "short single-sentence should not be segmented");
    }

    #[test]
    fn segment_text_returns_single_segment_for_two_short_sentences() {
        let text = "你好。再见。";
        let segments = segment_text(text);
        assert_eq!(segments.len(), 1, "two short sentences under char threshold should merge");
    }

    #[test]
    fn segment_text_splits_long_text_into_multiple_segments() {
        let text = "今天天气很好。我去公园散步了。看见了很多花。非常漂亮。我很开心。明天还要去。";
        let segments = segment_text(text);
        assert!(segments.len() >= 2, "long text should be split into multiple segments");
        for seg in &segments {
            assert!(
                seg.chars().count() <= MAX_SEGMENT_CHARS + 20,
                "each segment should not greatly exceed MAX_SEGMENT_CHARS"
            );
        }
    }

    #[test]
    fn segment_text_preserves_all_input_chars() {
        let text = "第一句话。第二句话。第三句话。第四句话。";
        let segments = segment_text(text);
        let joined: String = segments.join(" ");
        let joined_trimmed: String = joined
            .chars()
            .filter(|c| !c.is_whitespace() && *c != ' ')
            .collect();
        let original_trimmed: String = text
            .chars()
            .filter(|c| !c.is_whitespace() && *c != ' ')
            .collect();
        assert_eq!(
            joined_trimmed, original_trimmed,
            "segmented+joined text chars should match original"
        );
    }

    #[test]
    fn segment_text_empty_input_returns_single_empty() {
        let segments = segment_text("");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0], "");
    }

    #[test]
    fn segment_text_single_short_sentence_no_punctuation() {
        let text = "你好世界";
        let segments = segment_text(text);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0], "你好世界");
    }

    #[test]
    fn coverage_penalty_is_configured() {
        assert!(
            (COVERAGE_PENALTY - 0.05_f64).abs() < f64::EPSILON,
            "coverage penalty should be 0.05"
        );
    }

    #[test]
    fn min_segment_chars_and_max_segment_chars_are_reasonable() {
        assert!(MIN_SEGMENT_CHARS < MAX_SEGMENT_CHARS);
        assert!(MIN_SEGMENT_CHARS >= 60);
        assert!(MAX_SEGMENT_CHARS <= 500);
    }

    #[test]
    fn length_penalty_updated_to_one_point_five() {
        assert!(
            (LENGTH_PENALTY_ALPHA - 1.5_f64).abs() < f64::EPSILON,
            "LENGTH_PENALTY_ALPHA should be updated from 1.2 to 1.5"
        );
    }

    // ============================================================
    // TEST-SYNC-TRANS-SEGMENT-001: coverage gap tests
    // ============================================================

    #[test]
    fn translate_skips_segmentation_when_text_is_short() {
        // 构造 <120 字符的两句文本，验证 segment_text 返回 1 个 segment。
        // 对应 needs_segmentation 条件中 chars > MIN_SEGMENT_CHARS 的左侧短路行为。
        let text = "今天天气很好。我去公园散步了。";
        assert!(
            text.chars().count() <= MIN_SEGMENT_CHARS,
            "test input should be <= MIN_SEGMENT_CHARS(120)"
        );
        let segments = segment_text(text);
        assert_eq!(
            segments.len(),
            1,
            "short text under MIN_SEGMENT_CHARS should not be segmented even with 2 sentences"
        );
    }

    #[test]
    fn translate_skips_segmentation_when_single_sentence() {
        // 构造单一很长且无断句标点的句子（>120 字符），
        // 验证 segment_text 因 sentences.len() <= 1 而返回 1 个 segment。
        let text = "这段测试文本超过一百二十个字符且没有任何句号问号叹号或换行符属于单一整句因此即使其总长度远超最小分段阈值分段函数也必须将其整体返回为单一片段不进行任何切割处理这是因为分段的触发条件之一就是文本必须包含至少两个独立完整的句子才能被判断为需要分段";
        assert!(
            text.chars().count() > MIN_SEGMENT_CHARS,
            "test input should exceed MIN_SEGMENT_CHARS(120)"
        );
        let segments = segment_text(text);
        assert_eq!(
            segments.len(),
            1,
            "single sentence without punctuation should not be segmented"
        );
    }

    #[test]
    fn segment_text_splits_on_max_sentences_per_segment() {
        // 构造 6 个简短句（每句 <40 字符，总 <200 字符），
        // 验证在 MAX_SENTENCES_PER_SEGMENT=3 时被切割为 >=2 个 segments。
        let text = "第一句。第二句。第三句。第四句。第五句。第六句。";
        let segments = segment_text(text);
        assert!(
            segments.len() >= 2,
            "6 sentences should split into at least 2 segments due to MAX_SENTENCES_PER_SEGMENT=3"
        );
    }
}
