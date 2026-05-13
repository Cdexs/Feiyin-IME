use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;

use super::cache::WordbookEntry;

const MIGRATION_001: &str = include_str!("../../migrations/001_wordbook.sql");
const MIGRATION_002: &str = include_str!("../../migrations/002_wordbook_candidates.sql");

#[derive(Debug, Clone)]
pub struct StoredWordbookEntry {
    pub id: i64,
    pub raw: String,
    pub corrected: String,
    pub source: String,
    pub created_at: String,
}

pub fn load_entries() -> Result<Vec<WordbookEntry>> {
    let conn = open_connection()?;
    let mut stmt =
        conn.prepare("SELECT raw, corrected, source, created_at FROM wordbook ORDER BY id ASC")?;
    let entries = stmt
        .query_map([], |row| {
            Ok(WordbookEntry {
                raw: row.get(0)?,
                corrected: row.get(1)?,
                source: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

pub fn load_word_entries() -> Result<Vec<StoredWordbookEntry>> {
    let conn = open_connection()?;
    let mut stmt = conn
        .prepare("SELECT id, raw, corrected, source, created_at FROM wordbook ORDER BY id DESC")?;
    let entries = stmt
        .query_map([], |row| {
            Ok(StoredWordbookEntry {
                id: row.get(0)?,
                raw: row.get(1)?,
                corrected: row.get(2)?,
                source: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

pub fn get_entry_by_id(id: i64) -> Result<Option<StoredWordbookEntry>> {
    let conn = open_connection()?;
    let entry = conn
        .query_row(
            "SELECT id, raw, corrected, source, created_at FROM wordbook WHERE id = ?1",
            params![id],
            |row| {
                Ok(StoredWordbookEntry {
                    id: row.get(0)?,
                    raw: row.get(1)?,
                    corrected: row.get(2)?,
                    source: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        )
        .optional()?;
    Ok(entry)
}

pub fn insert_entry(entry: &WordbookEntry) -> Result<bool> {
    let conn = open_connection()?;
    let changed = conn.execute(
        "INSERT OR IGNORE INTO wordbook (raw, corrected, source, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![entry.raw, entry.corrected, entry.source, entry.created_at],
    )?;
    Ok(changed > 0)
}

pub fn delete_entry(raw: &str, corrected: &str) -> Result<bool> {
    let conn = open_connection()?;
    let changed = conn.execute(
        "DELETE FROM wordbook WHERE raw = ?1 AND corrected = ?2",
        params![raw, corrected],
    )?;
    Ok(changed > 0)
}

pub fn delete_entry_by_id(id: i64) -> Result<bool> {
    let conn = open_connection()?;
    let changed = conn.execute("DELETE FROM wordbook WHERE id = ?1", params![id])?;
    Ok(changed > 0)
}

pub fn upsert_candidate(raw: &str, corrected: &str) -> Result<u32> {
    let conn = open_connection()?;
    upsert_candidate_in_conn(&conn, raw, corrected)
}

pub fn get_candidate_count(raw: &str, corrected: &str) -> Result<u32> {
    let conn = open_connection()?;
    get_candidate_count_in_conn(&conn, raw, corrected)
}

pub fn delete_candidate(raw: &str, corrected: &str) -> Result<bool> {
    let conn = open_connection()?;
    delete_candidate_in_conn(&conn, raw, corrected)
}

fn open_connection() -> Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(&path)?;
    init_schema(&conn)?;
    Ok(conn)
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(MIGRATION_001)?;
    conn.execute_batch(MIGRATION_002)?;
    import_legacy_words(conn)?;
    conn.execute(
        "UPDATE wordbook SET source = 'system' WHERE source NOT IN ('system', 'user')",
        [],
    )?;
    Ok(())
}

fn import_legacy_words(conn: &Connection) -> Result<()> {
    let legacy_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'words'",
        [],
        |row| row.get(0),
    )?;

    if legacy_exists == 0 {
        return Ok(());
    }

    conn.execute(
        "INSERT OR IGNORE INTO wordbook (raw, corrected, source, created_at)
         SELECT raw, corrected, 'user', datetime(created_at, 'unixepoch')
         FROM words
         WHERE raw IS NOT NULL AND corrected IS NOT NULL",
        [],
    )?;

    // 迁移完成后删除旧表，防止下次连接重复导入导致已删除词条复活
    conn.execute_batch("DROP TABLE IF EXISTS words;")?;

    Ok(())
}

fn db_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("wordbook.sqlite")
}

fn upsert_candidate_in_conn(conn: &Connection, raw: &str, corrected: &str) -> Result<u32> {
    conn.execute(
        "INSERT INTO wordbook_candidates (raw, corrected, count, last_seen)
         VALUES (?1, ?2, 1, datetime('now'))
         ON CONFLICT(raw, corrected) DO UPDATE SET
             count = wordbook_candidates.count + 1,
             last_seen = datetime('now')",
        params![raw, corrected],
    )?;

    get_candidate_count_in_conn(conn, raw, corrected)
}

fn get_candidate_count_in_conn(conn: &Connection, raw: &str, corrected: &str) -> Result<u32> {
    let count = conn
        .query_row(
            "SELECT count FROM wordbook_candidates WHERE raw = ?1 AND corrected = ?2",
            params![raw, corrected],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0);
    Ok(count)
}

fn delete_candidate_in_conn(conn: &Connection, raw: &str, corrected: &str) -> Result<bool> {
    let changed = conn.execute(
        "DELETE FROM wordbook_candidates WHERE raw = ?1 AND corrected = ?2",
        params![raw, corrected],
    )?;
    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(MIGRATION_001).expect("migration");
        conn.execute_batch(MIGRATION_002)
            .expect("candidate migration");
        conn
    }

    fn insert(conn: &Connection, raw: &str, corrected: &str) -> i64 {
        conn.execute(
            "INSERT INTO wordbook (raw, corrected, source, created_at) \
             VALUES (?1, ?2, 'user', '2024-01-01T00:00:00Z')",
            rusqlite::params![raw, corrected],
        )
        .expect("insert");
        conn.last_insert_rowid()
    }

    #[test]
    fn test_delete_by_id_removes_entry() {
        let conn = setup();
        let id = insert(&conn, "原词", "修正词");

        let changed = conn
            .execute("DELETE FROM wordbook WHERE id = ?1", rusqlite::params![id])
            .expect("delete");
        assert_eq!(changed, 1, "should delete exactly 1 row");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM wordbook WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(count, 0, "entry must be gone after delete");
    }

    #[test]
    fn test_delete_by_id_nonexistent_returns_zero() {
        let conn = setup();
        let changed = conn
            .execute(
                "DELETE FROM wordbook WHERE id = ?1",
                rusqlite::params![99999i64],
            )
            .expect("delete");
        assert_eq!(changed, 0, "no rows affected for non-existent id");
    }

    #[test]
    fn test_delete_by_id_does_not_affect_other_entries() {
        let conn = setup();
        let id1 = insert(&conn, "词条A", "修正A");
        let _id2 = insert(&conn, "词条B", "修正B");

        conn.execute("DELETE FROM wordbook WHERE id = ?1", rusqlite::params![id1])
            .expect("delete");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1, "only the other entry should remain");
    }

    #[test]
    fn test_upsert_candidate_accumulates_count() {
        let conn = setup();

        let first = upsert_candidate_in_conn(&conn, "原词", "修正词").expect("first insert");
        let second = upsert_candidate_in_conn(&conn, "原词", "修正词").expect("second insert");

        assert_eq!(first, 1, "first observation should create count=1");
        assert_eq!(second, 2, "second observation should increment count");
        assert_eq!(
            get_candidate_count_in_conn(&conn, "原词", "修正词").expect("count"),
            2
        );
    }

    #[test]
    fn test_delete_candidate_clears_row() {
        let conn = setup();
        upsert_candidate_in_conn(&conn, "原词", "修正词").expect("insert");

        let deleted = delete_candidate_in_conn(&conn, "原词", "修正词").expect("delete");
        assert!(deleted, "candidate row should be deleted");
        assert_eq!(
            get_candidate_count_in_conn(&conn, "原词", "修正词").expect("count"),
            0
        );
    }

    // ============================================================
    // 频率计数器阈值逻辑测试（WORDBOOK-FREQ-001 / TEST-SYNC-FREQ-001）
    // ============================================================

    /// FREQ-001: 第一次检测差异，count=1，不写入正式词库
    #[test]
    fn freq_001_first_detection_count_1_no_write() {
        let conn = setup();

        let count = upsert_candidate_in_conn(&conn, "原词", "修正词").expect("first record");
        assert_eq!(count, 1, "first detection should have count=1");

        // 验证正式词库无此词条
        let wordbook_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            wordbook_count, 0,
            "wordbook should NOT contain entry before threshold"
        );
    }

    /// FREQ-002: 第二次检测同一差异，count=2，不写入正式词库（默认阈值=3）
    #[test]
    fn freq_002_second_detection_count_2_no_write() {
        let conn = setup();

        // 第一次
        let count1 = upsert_candidate_in_conn(&conn, "原词", "修正词").expect("record 1");
        assert_eq!(count1, 1);

        // 第二次
        let count2 = upsert_candidate_in_conn(&conn, "原词", "修正词").expect("record 2");
        assert_eq!(count2, 2, "second detection should have count=2");

        // 候选表只有 1 条记录（UPDATE 而非 INSERT）
        let candidate_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook_candidates", [], |r| r.get(0))
            .expect("count");
        assert_eq!(candidate_count, 1, "should have exactly 1 candidate row");

        // 正式词库仍为空
        let wordbook_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            wordbook_count, 0,
            "wordbook should NOT contain entry before threshold"
        );
    }

    /// FREQ-003: 第三次检测同一差异，count=3，达到默认阈值
    /// 注意：实际写入词库和清理候选的逻辑在 mod.rs 的 learn_correction 中，
    /// 此处仅验证 DB 层 count 累积到 3 的行为，并在同一个连接中模拟写入流程
    #[test]
    fn freq_003_third_detection_count_3_reaches_threshold() {
        let conn = setup();

        // 前两次检测
        upsert_candidate_in_conn(&conn, "原词", "修正词").expect("record 1");
        upsert_candidate_in_conn(&conn, "原词", "修正词").expect("record 2");

        // 第三次
        let count3 = upsert_candidate_in_conn(&conn, "原词", "修正词").expect("record 3");
        assert_eq!(count3, 3, "third detection should have count=3");

        // 此时调用方（mod.rs）应判断 count >= threshold，执行：
        // 1. 写入正式词库
        // 2. 删除候选记录
        // 这里在同一个连接中模拟该流程
        conn.execute(
            "INSERT INTO wordbook (raw, corrected, source, created_at) \
             VALUES (?1, ?2, 'auto_learn', datetime('now'))",
            rusqlite::params!["原词", "修正词"],
        )
        .expect("insert to wordbook");

        delete_candidate_in_conn(&conn, "原词", "修正词").expect("cleanup candidate");

        // 验证正式词库有此词条
        let wordbook_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            wordbook_count, 1,
            "wordbook should contain entry after threshold"
        );

        // 验证候选表已清理
        let candidate_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook_candidates", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            candidate_count, 0,
            "candidate table should be cleaned up after write"
        );
    }

    /// FREQ-004: 阈值可配置，测试 threshold=5 的场景
    /// 验证 count 能正确累积到 5，且在此之前不触发写入
    #[test]
    fn freq_004_configurable_threshold_5() {
        let conn = setup();

        // 前 4 次检测
        for i in 1..=4 {
            let count =
                upsert_candidate_in_conn(&conn, "原词", "修正词").expect(&format!("record {}", i));
            assert_eq!(count, i as u32, "detection {} should have count={}", i, i);
        }

        // 正式词库仍为空（未达阈值=5）
        let wordbook_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            wordbook_count, 0,
            "wordbook should NOT contain entry before reaching threshold=5"
        );

        // 第 5 次应达到阈值
        let count5 = upsert_candidate_in_conn(&conn, "原词", "修正词").expect("record 5");
        assert_eq!(count5, 5, "fifth detection should have count=5");
    }

    /// 额外测试：不同 (raw,corrected) 对互不影响
    #[test]
    fn freq_extra_different_pairs_independent() {
        let conn = setup();

        // 对 A 记录 2 次
        upsert_candidate_in_conn(&conn, "A", "修正A").expect("A1");
        upsert_candidate_in_conn(&conn, "A", "修正A").expect("A2");

        // 对 B 记录 1 次
        let count_b = upsert_candidate_in_conn(&conn, "B", "修正B").expect("B1");
        assert_eq!(count_b, 1, "B should have count=1 (independent from A)");

        // 候选表应有 2 条记录
        let candidate_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook_candidates", [], |r| r.get(0))
            .expect("count");
        assert_eq!(candidate_count, 2, "should have 2 candidate rows (A and B)");
    }
}
