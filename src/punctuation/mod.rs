use std::path::{Path, PathBuf};

use crate::transcription;

const PUNCT_MODEL_SUBDIR: &str = "punct-ct-transformer-zh";

pub struct PunctuationEngine {
    punct: sherpa_onnx::OfflinePunctuation,
}

impl PunctuationEngine {
    pub fn new(model_dir: &Path) -> Option<Self> {
        let punct_dir = model_dir.join(PUNCT_MODEL_SUBDIR);
        let model_path = punct_dir.join("model.onnx");

        if !model_path.exists() {
            log::warn!(
                "Punctuation model not found at {:?}, disabling punctuation",
                model_path
            );
            return None;
        }

        let model_path_str = model_path.to_str()?.to_string();

        let mut config = sherpa_onnx::OfflinePunctuationConfig::default();
        config.model.ct_transformer = Some(model_path_str);
        config.model.num_threads = 1;
        config.model.debug = false;
        config.model.provider = Some("cpu".to_string());

        match sherpa_onnx::OfflinePunctuation::create(&config) {
            Some(punct) => {
                log::info!("Punctuation model loaded from {:?}", punct_dir);
                Some(Self { punct })
            }
            None => {
                log::error!("Failed to create OfflinePunctuation from {:?}", punct_dir);
                None
            }
        }
    }

    pub fn add_punctuation(&mut self, text: &str) -> Option<String> {
        let result = self.punct.add_punctuation(text);
        result.map(|s| convert_punctuation_for_english(&s))
    }

    pub fn model_dir() -> PathBuf {
        transcription::model_dir()
    }
}

fn convert_punctuation_for_english(text: &str) -> String {
    let ascii_letter_count = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
    let total_chars = text.chars().filter(|c| c.is_alphanumeric() || *c == ' ').count();
    if total_chars == 0 {
        return text.to_string();
    }
    let ratio = ascii_letter_count as f64 / total_chars as f64;
    if ratio > 0.5 {
        text.replace('，', ",")
            .replace('。', ".")
            .replace('？', "?")
            .replace('、', ",")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_punctuation_model_subdir_constant() {
        assert_eq!(PUNCT_MODEL_SUBDIR, "punct-ct-transformer-zh");
    }

    #[test]
    fn test_model_dir_delegates_to_transcription() {
        let dir = PunctuationEngine::model_dir();
        assert!(dir.ends_with("models") || dir.to_string_lossy().contains("models"));
    }

    #[test]
    fn test_convert_punctuation_pure_english() {
        let input = "Hello world，this is a test。How are you？";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, "Hello world,this is a test.How are you?");
    }

    #[test]
    fn test_convert_punctuation_mixed_chinese_no_convert() {
        let input = "今天天气真好，我们去公园玩吧。";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_convert_punctuation_mixed_english_heavy() {
        let input = "We used Python，built an API，and deployed it。OK？";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, "We used Python,built an API,and deployed it.OK?");
    }

    #[test]
    fn test_convert_punctuation_boundary_ratio_exactly_50() {
        // 2 ASCII letters + 2 Chinese chars = 4 alphanumeric total, ratio = 2/4 = 0.5
        // Threshold is > 0.5, so exactly 0.5 should NOT convert
        let input = "ab中文";
        // First add some punctuation marks to verify they stay
        let input_punct = "ab，中文。";
        let output = convert_punctuation_for_english(input_punct);
        assert_eq!(output, input_punct, "Exactly 50% ratio should NOT convert (threshold is >0.5)");
    }

    #[test]
    fn test_convert_punctuation_empty_text() {
        let input = "";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, "", "Empty text should return empty");
    }

    #[test]
    fn test_convert_punctuation_only_punctuation() {
        // Only punctuation marks: total_chars == 0, ratio would divide by zero
        // Function should return text as-is
        let input = "，。？！";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, input, "Punctuation-only text should be returned unchanged");
    }

    #[test]
    fn test_convert_punctuation_just_above_threshold() {
        // 7 letters + 1 number + 1 space + 2 punct = 11 chars, 7 letters = 63.6%
        let input = "abcdefg，h1。";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, "abcdefg,h1.", "Ratio > 0.5 should convert to half-width");
    }

    #[test]
    fn test_convert_punctuation_numbers_not_counted_as_letters() {
        // Numbers don't count as ASCII letters in the ratio
        let input = "123，456。789？";
        let output = convert_punctuation_for_english(input);
        assert_eq!(output, input, "Numbers-only should not trigger conversion");
    }
}