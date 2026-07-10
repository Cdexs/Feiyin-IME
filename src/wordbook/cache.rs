use anyhow::{bail, Result};
use chrono::Utc;
use std::collections::HashMap;

use super::db;

#[derive(Debug, Clone)]
pub struct WordbookCache {
    entries: HashMap<String, WordbookEntry>,
    stats: WordbookStats,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WordbookStats {
    pub total: usize,
    pub system_count: usize,
    pub user_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordbookEntry {
    pub word: String,
    pub source: String,
    pub created_at: String,
}

impl WordbookCache {
    pub fn load_from_db() -> Result<Self> {
        let entries = db::load_entries()?;
        Ok(Self::from_entries(entries))
    }

    pub fn add_entry(&mut self, word: &str, source: &str) -> Result<bool> {
        let word = validate_entry(word, source)?;

        if self.entries.contains_key(&word) {
            return Ok(false);
        }

        let entry = WordbookEntry {
            word: word.clone(),
            source: source.to_string(),
            created_at: Utc::now().to_rfc3339(),
        };

        if db::insert_entry(&entry)? {
            self.entries.insert(word, entry);
            self.recalculate_stats();
            return Ok(true);
        }

        Ok(false)
    }

    pub fn remove_entry(&mut self, word: &str) -> Result<bool> {
        let word = word.trim().to_string();

        log::debug!("[wordbook] remove_entry: word='{}'", word);

        let removed_from_db = db::delete_entry(&word)?;
        let removed_from_cache = self.entries.remove(&word).is_some();

        log::debug!(
            "[wordbook] remove_result: db={}, cache={}",
            removed_from_db,
            removed_from_cache
        );

        if removed_from_db || removed_from_cache {
            self.recalculate_stats();
            return Ok(true);
        }

        Ok(false)
    }

    pub fn get_all_words(&self) -> Vec<WordbookEntry> {
        let mut entries: Vec<_> = self.entries.values().cloned().collect();
        entries.sort_by(|a, b| {
            a.word
                .cmp(&b.word)
                .then_with(|| a.source.cmp(&b.source))
        });
        entries
    }

    #[allow(dead_code)]
    pub fn exists(&self, word: &str) -> bool {
        self.entries.contains_key(word.trim())
    }

    #[allow(dead_code)]
    pub fn get_stats(&self) -> WordbookStats {
        self.stats.clone()
    }

    fn from_entries(entries: Vec<WordbookEntry>) -> Self {
        let mut cache = Self {
            entries: entries
                .into_iter()
                .map(|entry| (entry.word.clone(), entry))
                .collect(),
            stats: WordbookStats::default(),
        };
        cache.recalculate_stats();
        cache
    }

    fn recalculate_stats(&mut self) {
        let mut stats = WordbookStats {
            total: self.entries.len(),
            ..WordbookStats::default()
        };

        for entry in self.entries.values() {
            match entry.source.as_str() {
                "system" => stats.system_count += 1,
                "user" => stats.user_count += 1,
                _ => {}
            }
        }

        self.stats = stats;
    }
}

fn validate_entry(word: &str, source: &str) -> Result<String> {
    let word = word.trim().to_string();
    if word.is_empty() {
        bail!("wordbook word cannot be empty");
    }
    if !matches!(source, "system" | "user") {
        bail!("wordbook source must be either system or user");
    }
    Ok(word)
}