#[path = "../../src/wordbook/mod.rs"]
mod wordbook_core;

use serde::Serialize;
use wordbook_core::WordbookCache;

#[derive(Debug, Clone, Serialize)]
pub struct WordbookEntry {
    pub id: i64,
    pub raw: String,
    pub corrected: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WordbookStats {
    pub total: usize,
    pub system_count: usize,
    pub user_count: usize,
}

#[tauri::command]
pub fn get_wordbook_entries() -> Result<Vec<WordbookEntry>, String> {
    let wordbook =
        wordbook_core::Wordbook::open().map_err(|err| format!("读取词库失败：{}", err))?;
    let entries = wordbook
        .list_all()
        .map_err(|err| format!("读取词库条目失败：{}", err))?;

    Ok(entries
        .into_iter()
        .map(|entry| WordbookEntry {
            id: entry.id,
            raw: entry.raw,
            corrected: entry.corrected,
            source: entry.source,
            created_at: entry.created_at,
        })
        .collect())
}

#[tauri::command]
pub fn get_wordbook_stats() -> Result<WordbookStats, String> {
    let stats = WordbookCache::load_from_db()
        .map_err(|err| format!("读取词库统计失败：{}", err))?
        .get_stats();

    Ok(WordbookStats {
        total: stats.total,
        system_count: stats.system_count,
        user_count: stats.user_count,
    })
}

#[tauri::command]
pub fn add_wordbook_entry(raw: String, corrected: String) -> Result<(), String> {
    let raw = raw.trim();
    let corrected = corrected.trim();
    validate_pair(raw, corrected)?;

    let mut cache =
        WordbookCache::load_from_db().map_err(|err| format!("打开词库失败：{}", err))?;
    let inserted = cache
        .add_entry(raw, corrected, "user")
        .map_err(|err| format!("添加词库条目失败：{}", err))?;

    if !inserted {
        return Err("词库条目已存在，请勿重复添加。".to_string());
    }

    Ok(())
}

#[tauri::command]
pub fn delete_wordbook_entry(raw: String, corrected: String) -> Result<(), String> {
    let raw = raw.trim();
    let corrected = corrected.trim();
    validate_pair(raw, corrected)?;

    let mut cache =
        WordbookCache::load_from_db().map_err(|err| format!("打开词库失败：{}", err))?;
    let removed = cache
        .remove_entry(raw, corrected)
        .map_err(|err| format!("删除词库条目失败：{}", err))?;

    if !removed {
        return Err("词库条目不存在或已被删除。".to_string());
    }

    Ok(())
}

#[tauri::command]
pub fn delete_wordbook_entry_by_id(id: i64) -> Result<(), String> {
    let removed = wordbook_core::db::delete_entry_by_id(id)
        .map_err(|err| format!("删除词库条目失败：{}", err))?;
    if !removed {
        return Err("词库条目不存在或已被删除。".to_string());
    }
    Ok(())
}

fn validate_pair(raw: &str, corrected: &str) -> Result<(), String> {
    if raw.is_empty() {
        return Err("原词不能为空。".to_string());
    }
    if corrected.is_empty() {
        return Err("修正词不能为空。".to_string());
    }
    Ok(())
}
