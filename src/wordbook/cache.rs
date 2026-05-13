use anyhow::{bail, Result};
use chrono::Utc;
use std::collections::HashMap;

use super::db;

#[derive(Debug, Clone)]
pub struct WordbookCache {
    entries: HashMap<(String, String), WordbookEntry>,
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
    pub raw: String,
    pub corrected: String,
    pub source: String,
    pub created_at: String,
}

impl WordbookCache {
    pub fn load_from_db() -> Result<Self> {
        let entries = db::load_entries()?;
        Ok(Self::from_entries(entries))
    }

    pub fn add_entry(&mut self, raw: &str, corrected: &str, source: &str) -> Result<bool> {
        let (raw, corrected) = validate_entry(raw, corrected, source)?;

        let key = key(&raw, &corrected);
        if self.entries.contains_key(&key) {
            return Ok(false);
        }

        let entry = WordbookEntry {
            raw: raw.clone(),
            corrected: corrected.clone(),
            source: source.to_string(),
            created_at: Utc::now().to_rfc3339(),
        };

        if db::insert_entry(&entry)? {
            self.entries.insert(key, entry);
            self.recalculate_stats();
            return Ok(true);
        }

        Ok(false)
    }

    pub fn remove_entry(&mut self, raw: &str, corrected: &str) -> Result<bool> {
        let raw = raw.trim().to_string();
        let corrected = corrected.trim().to_string();

        log::debug!(
            "[wordbook] remove_entry: raw='{}', corrected='{}'",
            raw,
            corrected
        );

        let removed_from_db = db::delete_entry(&raw, &corrected)?;
        let removed_from_cache = self.entries.remove(&key(&raw, &corrected)).is_some();

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

    pub fn get_all_mappings(&self) -> Vec<WordbookEntry> {
        let mut entries: Vec<_> = self.entries.values().cloned().collect();
        entries.sort_by(|a, b| {
            a.raw
                .cmp(&b.raw)
                .then_with(|| a.corrected.cmp(&b.corrected))
                .then_with(|| a.source.cmp(&b.source))
        });
        entries
    }

    #[allow(dead_code)]
    pub fn exists(&self, raw: &str, corrected: &str) -> bool {
        self.entries.contains_key(&key(raw, corrected))
    }

    #[allow(dead_code)]
    pub fn get_stats(&self) -> WordbookStats {
        self.stats.clone()
    }

    fn from_entries(entries: Vec<WordbookEntry>) -> Self {
        let mut cache = Self {
            entries: entries
                .into_iter()
                .map(|entry| ((entry.raw.clone(), entry.corrected.clone()), entry))
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

fn validate_entry(raw: &str, corrected: &str, source: &str) -> Result<(String, String)> {
    let raw = raw.trim().to_string();
    let corrected = corrected.trim().to_string();
    if raw.is_empty() {
        bail!("wordbook raw text cannot be empty");
    }
    if corrected.is_empty() {
        bail!("wordbook corrected text cannot be empty");
    }
    if !matches!(source, "system" | "user") {
        bail!("wordbook source must be either system or user");
    }
    Ok((raw, corrected))
}

fn key(raw: &str, corrected: &str) -> (String, String) {
    (raw.to_string(), corrected.to_string())
}
