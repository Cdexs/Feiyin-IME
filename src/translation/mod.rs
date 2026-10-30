//! 翻译方向判定（FLASH-P3-A，DEC-114 10-30：翻译只走大模型 `llm::optimize_and_translate`）。
//!
//! 原离线翻译（NLLB-200 / CTranslate2 + sentencepiece，更早的 opus-mt）随本地模型整体删除；
//! 本模块只留平台中立的纯函数。

use crate::config::TranslationLanguage;
use crate::text_normalizer::contains_han;

/// TRANS-BIDIR-001 / REFACTOR-DERIVE-TARGET-001: Derive translation target direction
/// from content. Platform-neutral business logic (no cfg gating) — macOS side
/// can reuse directly, zero drift (DEC-033).
///
/// - Contains Han characters (incl. Japanese kanji) → translate to English
/// - Other (English, pure digits, etc.) → translate to Chinese
///
/// Known semantic boundary (documented, not solved here):
/// - Japanese kanji triggers `contains_han` → judged as Chinese → translated to
///   English. Better than the old "silently skip" but imperfect; LANG-MIXED-001
///   kana/hangul probes exist for future refinement.
/// - Pure digits/punctuation with no Han and no Latin → judged Chinese direction,
///   harmless no-op (no content to translate).
pub fn derive_translation_target(text: &str) -> TranslationLanguage {
    if contains_han(text) {
        TranslationLanguage::English
    } else {
        TranslationLanguage::Chinese
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_target_chinese_text_returns_english() {
        assert_eq!(
            derive_translation_target("你好世界"),
            TranslationLanguage::English
        );
        assert_eq!(
            derive_translation_target("温度是四十摄氏度"),
            TranslationLanguage::English
        );
    }

    #[test]
    fn derive_target_english_text_returns_chinese() {
        assert_eq!(
            derive_translation_target("hello world"),
            TranslationLanguage::Chinese
        );
        assert_eq!(
            derive_translation_target("The temperature is 40 degrees."),
            TranslationLanguage::Chinese
        );
    }

    #[test]
    fn derive_target_mixed_text_returns_english() {
        assert_eq!(
            derive_translation_target("你好 world"),
            TranslationLanguage::English
        );
    }

    #[test]
    fn derive_target_japanese_kanji_returns_english_known_boundary() {
        // 已知语义边界：日文汉字被 contains_han 判定为真 → English。
        // 非 bug，是已知取舍。未来需结合 contains_kana 精化。
        assert_eq!(
            derive_translation_target("日本語の漢字"),
            TranslationLanguage::English
        );
    }

    #[test]
    fn derive_target_non_han_returns_chinese() {
        assert_eq!(
            derive_translation_target("12345"),
            TranslationLanguage::Chinese
        );
        assert_eq!(
            derive_translation_target("。，！"),
            TranslationLanguage::Chinese
        );
        assert_eq!(derive_translation_target(""), TranslationLanguage::Chinese);
    }

    // ---- 模型可用性 / 文件清单（NLLB） ----
}
