pub mod cache;
pub mod db;

use anyhow::Result;
use std::cell::RefCell;

#[allow(unused_imports)]
pub use cache::{WordbookCache, WordbookEntry, WordbookStats};

/// Compatibility wrapper for the existing main pipeline.
///
/// New code should use `WordbookCache` directly so reads can stay in memory.
pub struct Wordbook {
    cache: RefCell<WordbookCache>,
}

impl Wordbook {
    pub fn open() -> Result<Self> {
        Ok(Self {
            cache: RefCell::new(WordbookCache::load_from_db()?),
        })
    }

    /// Manually add a word as a user-sourced entry.
    pub fn add(&self, word: &str) -> Result<()> {
        self.cache.borrow_mut().add_entry(word, "user")?;
        Ok(())
    }

    /// Delete a word by id.
    #[allow(dead_code)]
    pub fn delete(&self, id: i64) -> Result<()> {
        if let Some(entry) = db::get_entry_by_id(id)? {
            self.cache.borrow_mut().remove_entry(&entry.word)?;
        }
        Ok(())
    }

    /// List all words for UI display.
    #[allow(dead_code)]
    pub fn list_all(&self) -> Result<Vec<WordEntry>> {
        let entries = db::load_word_entries()?;
        Ok(entries
            .into_iter()
            .map(|entry| WordEntry {
                id: entry.id,
                word: entry.word,
                source: entry.source,
                created_at: entry.created_at,
            })
            .collect())
    }

    /// Learn a correction by comparing original ASR output with edited text.
    /// WORDBOOK-SINGLEWORD-001-CORE: produces corrected-side word for single-word model.
    #[allow(dead_code)]
    pub fn learn_correction(&self, original: &str, edited: &str, threshold: u32) -> Result<()> {
        let Some(corrected_part) = extract_correction_word(original, edited) else {
            return Ok(());
        };

        self.learn_suggestion(&corrected_part, threshold)
    }

    pub fn learn_suggestion(&self, word: &str, threshold: u32) -> Result<()> {
        let word = word.trim();
        if word.is_empty() {
            return Ok(());
        }

        if self.cache.borrow().exists(word) {
            let _ = db::delete_candidate(word);
            return Ok(());
        }

        let threshold = threshold.max(1);
        let count = db::upsert_candidate(word)?;
        if count < threshold {
            log::info!(
                "Auto-learn candidate observed: '{}' ({}/{})",
                word,
                count,
                threshold
            );
            return Ok(());
        }

        log::info!(
            "Auto-learning promoted after threshold: '{}' ({}/{})",
            word,
            count,
            threshold
        );
        self.cache.borrow_mut().add_entry(word, "system")?;
        let _ = db::delete_candidate(word);

        Ok(())
    }
}

/// WORDBOOK-SINGLEWORD-001-CORE: Extract corrected-side word from original vs edited text.
/// Preserves the diff extraction logic but returns only the corrected part (the word to learn).
/// This is for future WORDBOOK-CORRECTION-UI-001 纠错入口 reuse.
fn extract_correction_word(original: &str, edited: &str) -> Option<String> {
    if original == edited {
        return None;
    }

    let orig_words = tokenize(original);
    let edit_words = tokenize(edited);

    let common_prefix = orig_words
        .iter()
        .zip(edit_words.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let common_suffix = orig_words
        .iter()
        .rev()
        .zip(edit_words.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    let orig_mid_end = orig_words.len().saturating_sub(common_suffix);
    let edit_mid_end = edit_words.len().saturating_sub(common_suffix);

    if common_prefix >= orig_mid_end || common_prefix >= edit_mid_end {
        return None;
    }

    let corrected_part = edit_words[common_prefix..edit_mid_end].join("");

    if corrected_part.is_empty() {
        return None;
    }

    let trimmed = corrected_part.trim().to_string();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed)
}

fn tokenize(text: &str) -> Vec<String> {
    text.chars().map(|c| c.to_string()).collect()
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WordEntry {
    pub id: i64,
    pub word: String,
    pub source: String,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::extract_correction_word;

    #[test]
    fn test_extract_correction_word_uses_changed_middle_segment() {
        let word = extract_correction_word("我想用微型免", "我想用voice ime");
        assert_eq!(word, Some("voice ime".to_string()));
    }

    #[test]
    fn test_extract_correction_word_returns_none_for_identical_text() {
        assert_eq!(extract_correction_word("一样", "一样"), None);
    }

    #[test]
    fn test_extract_correction_word_returns_corrected_side() {
        let word = extract_correction_word("我吃了苹果", "我吃了梨");
        assert_eq!(word, Some("梨".to_string()));
    }

    #[test]
    fn test_extract_correction_word_trims_whitespace() {
        let word = extract_correction_word("测试", " 测试词 ");
        assert_eq!(word, Some("测试词".to_string()));
    }

    #[test]
    fn test_extract_correction_word_empty_corrected_returns_none() {
        let word = extract_correction_word("测试", "  ");
        assert_eq!(word, None);
    }
}