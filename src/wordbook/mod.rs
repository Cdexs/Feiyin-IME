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

    /// Manually add or update a word pair as a user-sourced mapping.
    pub fn add(&self, raw: &str, corrected: &str) -> Result<()> {
        self.cache.borrow_mut().add_entry(raw, corrected, "user")?;
        Ok(())
    }

    /// Delete a word pair by id.
    #[allow(dead_code)]
    pub fn delete(&self, id: i64) -> Result<()> {
        if let Some(entry) = db::get_entry_by_id(id)? {
            self.cache
                .borrow_mut()
                .remove_entry(&entry.raw, &entry.corrected)?;
        }
        Ok(())
    }

    /// List all word pairs for UI display.
    #[allow(dead_code)]
    pub fn list_all(&self) -> Result<Vec<WordEntry>> {
        let entries = db::load_word_entries()?;
        Ok(entries
            .into_iter()
            .map(|entry| WordEntry {
                id: entry.id,
                raw: entry.raw,
                corrected: entry.corrected,
                source: entry.source,
                created_at: entry.created_at,
            })
            .collect())
    }

    /// Apply word substitutions to a text string.
    pub fn apply(&self, text: &str) -> Result<String> {
        let mut entries = self.cache.borrow().get_all_mappings();
        entries.sort_by(|a, b| {
            b.raw
                .chars()
                .count()
                .cmp(&a.raw.chars().count())
                .then_with(|| a.raw.cmp(&b.raw))
                .then_with(|| a.corrected.cmp(&b.corrected))
        });

        let mut result = text.to_string();
        for entry in &entries {
            if !entry.raw.is_empty() {
                result = result.replace(&entry.raw, &entry.corrected);
            }
        }
        Ok(result)
    }

    /// Learn a correction by comparing original ASR output with edited text.
    #[allow(dead_code)]
    pub fn learn_correction(&self, original: &str, edited: &str, threshold: u32) -> Result<()> {
        let Some((raw_part, corrected_part)) = extract_correction_pair(original, edited) else {
            return Ok(());
        };

        self.learn_suggestion(&raw_part, &corrected_part, threshold)
    }

    pub fn learn_suggestion(&self, raw: &str, corrected: &str, threshold: u32) -> Result<()> {
        let raw = raw.trim();
        let corrected = corrected.trim();
        if raw.is_empty() || corrected.is_empty() || raw == corrected {
            return Ok(());
        }

        if self.cache.borrow().exists(raw, corrected) {
            let _ = db::delete_candidate(raw, corrected);
            return Ok(());
        }

        let threshold = threshold.max(1);
        let count = db::upsert_candidate(raw, corrected)?;
        if count < threshold {
            log::info!(
                "Auto-learn candidate observed: '{}' -> '{}' ({}/{})",
                raw,
                corrected,
                count,
                threshold
            );
            return Ok(());
        }

        log::info!(
            "Auto-learning promoted after threshold: '{}' -> '{}' ({}/{})",
            raw,
            corrected,
            count,
            threshold
        );
        self.cache
            .borrow_mut()
            .add_entry(raw, corrected, "system")?;
        let _ = db::delete_candidate(raw, corrected);

        Ok(())
    }
}

fn extract_correction_pair(original: &str, edited: &str) -> Option<(String, String)> {
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

    let raw_part = orig_words[common_prefix..orig_mid_end].join("");
    let corrected_part = edit_words[common_prefix..edit_mid_end].join("");

    if raw_part.is_empty() || corrected_part.is_empty() || raw_part == corrected_part {
        return None;
    }

    Some((raw_part, corrected_part))
}

fn tokenize(text: &str) -> Vec<String> {
    text.chars().map(|c| c.to_string()).collect()
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WordEntry {
    pub id: i64,
    pub raw: String,
    pub corrected: String,
    pub source: String,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::extract_correction_pair;

    #[test]
    fn test_extract_correction_pair_uses_changed_middle_segment() {
        let pair = extract_correction_pair("我想用微型免", "我想用voice ime");
        assert_eq!(pair, Some(("微型免".to_string(), "voice ime".to_string())));
    }

    #[test]
    fn test_extract_correction_pair_returns_none_for_identical_text() {
        assert_eq!(extract_correction_pair("一样", "一样"), None);
    }
}
