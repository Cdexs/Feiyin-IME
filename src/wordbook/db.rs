use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;

use super::cache::WordbookEntry;

const MIGRATION_001: &str = include_str!("../../migrations/001_wordbook.sql");
const MIGRATION_002: &str = include_str!("../../migrations/002_wordbook_candidates.sql");
const MIGRATION_003: &str = include_str!("../../migrations/003_wordbook_singleword.sql");

#[derive(Debug, Clone)]
pub struct StoredWordbookEntry {
    pub id: i64,
    pub word: String,
    pub source: String,
    pub created_at: String,
}

pub fn load_entries() -> Result<Vec<WordbookEntry>> {
    let conn = open_connection()?;
    let mut stmt =
        conn.prepare("SELECT word, source, created_at FROM wordbook ORDER BY id ASC")?;
    let entries = stmt
        .query_map([], |row| {
            Ok(WordbookEntry {
                word: row.get(0)?,
                source: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

pub fn load_word_entries() -> Result<Vec<StoredWordbookEntry>> {
    let conn = open_connection()?;
    let mut stmt = conn
        .prepare("SELECT id, word, source, created_at FROM wordbook ORDER BY id DESC")?;
    let entries = stmt
        .query_map([], |row| {
            Ok(StoredWordbookEntry {
                id: row.get(0)?,
                word: row.get(1)?,
                source: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

pub fn get_entry_by_id(id: i64) -> Result<Option<StoredWordbookEntry>> {
    let conn = open_connection()?;
    let entry = conn
        .query_row(
            "SELECT id, word, source, created_at FROM wordbook WHERE id = ?1",
            params![id],
            |row| {
                Ok(StoredWordbookEntry {
                    id: row.get(0)?,
                    word: row.get(1)?,
                    source: row.get(2)?,
                    created_at: row.get(3)?,
                })
            },
        )
        .optional()?;
    Ok(entry)
}

pub fn insert_entry(entry: &WordbookEntry) -> Result<bool> {
    let conn = open_connection()?;
    let changed = conn.execute(
        "INSERT OR IGNORE INTO wordbook (word, source, created_at)
         VALUES (?1, ?2, ?3)",
        params![entry.word, entry.source, entry.created_at],
    )?;
    Ok(changed > 0)
}

pub fn delete_entry(word: &str) -> Result<bool> {
    let conn = open_connection()?;
    let changed = conn.execute("DELETE FROM wordbook WHERE word = ?1", params![word])?;
    Ok(changed > 0)
}

pub fn delete_entry_by_id(id: i64) -> Result<bool> {
    let conn = open_connection()?;
    let changed = conn.execute("DELETE FROM wordbook WHERE id = ?1", params![id])?;
    Ok(changed > 0)
}

pub fn upsert_candidate(word: &str) -> Result<u32> {
    let conn = open_connection()?;
    upsert_candidate_in_conn(&conn, word)
}

pub fn get_candidate_count(word: &str) -> Result<u32> {
    let conn = open_connection()?;
    get_candidate_count_in_conn(&conn, word)
}

pub fn delete_candidate(word: &str) -> Result<bool> {
    let conn = open_connection()?;
    delete_candidate_in_conn(&conn, word)
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
    conn.execute_batch(MIGRATION_003)?;
    finalize_singleword_migration(conn)?;
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

    // Legacy 'words' table has raw/corrected columns
    // Insert into wordbook (raw, corrected) form — migration 003 will convert to single-word
    conn.execute(
        "INSERT OR IGNORE INTO wordbook (raw, corrected, source, created_at)
         SELECT raw, corrected, 'user', datetime(created_at, 'unixepoch')
         FROM words
         WHERE raw IS NOT NULL AND corrected IS NOT NULL",
        [],
    )?;

    conn.execute_batch("DROP TABLE IF EXISTS words;")?;

    Ok(())
}

/// WORDBOOK-SINGLEWORD-001-CORE: Conditional table replacement
/// Only replaces old wordbook table (with raw/corrected columns) with new single-word table.
/// If already migrated (no raw column), cleans up stale temp tables.
/// Data migration is done in Rust (not SQL) to avoid column-reference errors on re-run.
fn finalize_singleword_migration(conn: &Connection) -> Result<()> {
    let has_raw_column: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('wordbook') WHERE name = 'raw'",
        [],
        |row| row.get(0),
    )?;

    if has_raw_column == 0 {
        // Already migrated: wordbook table has 'word' column, not 'raw'
        // Clean up any stale wordbook_new/wordbook_candidates_new tables from interrupted runs
        conn.execute_batch("DROP TABLE IF EXISTS wordbook_new;")?;
        conn.execute_batch("DROP TABLE IF EXISTS wordbook_candidates_new;")?;
        return Ok(());
    }

    // Old table has raw/corrected columns -> migrate data then replace

    // 1. Migrate wordbook data: take corrected side, trim, dedup (INSERT OR IGNORE handles dedup)
    conn.execute(
        "INSERT OR IGNORE INTO wordbook_new (word, source, created_at)
         SELECT DISTINCT TRIM(corrected), source, created_at
         FROM wordbook
         WHERE TRIM(corrected) != ''",
        [],
    )?;

    // 2. Migrate candidates: take corrected side, trim, dedup (count takes MAX)
    conn.execute(
        "INSERT OR IGNORE INTO wordbook_candidates_new (word, count, last_seen)
         SELECT TRIM(corrected), MAX(count), last_seen
         FROM wordbook_candidates
         WHERE TRIM(corrected) != ''
         GROUP BY TRIM(corrected)",
        [],
    )?;

    // 3. Replace old tables with new ones
    conn.execute_batch("DROP TABLE IF EXISTS wordbook_candidates;")?;
    conn.execute_batch("DROP TABLE IF EXISTS wordbook;")?;
    conn.execute_batch("ALTER TABLE wordbook_new RENAME TO wordbook;")?;
    conn.execute_batch("ALTER TABLE wordbook_candidates_new RENAME TO wordbook_candidates;")?;
    Ok(())
}

fn db_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("wordbook.sqlite")
}

fn upsert_candidate_in_conn(conn: &Connection, word: &str) -> Result<u32> {
    conn.execute(
        "INSERT INTO wordbook_candidates (word, count, last_seen)
         VALUES (?1, 1, datetime('now'))
         ON CONFLICT(word) DO UPDATE SET
             count = wordbook_candidates.count + 1,
             last_seen = datetime('now')",
        params![word],
    )?;

    get_candidate_count_in_conn(conn, word)
}

fn get_candidate_count_in_conn(conn: &Connection, word: &str) -> Result<u32> {
    let count = conn
        .query_row(
            "SELECT count FROM wordbook_candidates WHERE word = ?1",
            params![word],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0);
    Ok(count)
}

fn delete_candidate_in_conn(conn: &Connection, word: &str) -> Result<bool> {
    let changed = conn.execute(
        "DELETE FROM wordbook_candidates WHERE word = ?1",
        params![word],
    )?;
    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(MIGRATION_001).expect("migration 001");
        conn.execute_batch(MIGRATION_002)
            .expect("candidate migration 002");
        // Simulate the init flow: legacy import then migration 003 + finalize
        conn.execute_batch(MIGRATION_003).expect("migration 003");
        finalize_singleword_migration(&conn).expect("finalize singleword migration");
        conn
    }

    fn insert(conn: &Connection, word: &str) -> i64 {
        conn.execute(
            "INSERT INTO wordbook (word, source, created_at) \
             VALUES (?1, 'user', '2024-01-01T00:00:00Z')",
            rusqlite::params![word],
        )
        .expect("insert");
        conn.last_insert_rowid()
    }

    #[test]
    fn test_delete_by_id_removes_entry() {
        let conn = setup();
        let id = insert(&conn, "测试词");

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
        let id1 = insert(&conn, "词条A");
        let _id2 = insert(&conn, "词条B");

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

        let first = upsert_candidate_in_conn(&conn, "测试词").expect("first insert");
        let second = upsert_candidate_in_conn(&conn, "测试词").expect("second insert");

        assert_eq!(first, 1, "first observation should create count=1");
        assert_eq!(second, 2, "second observation should increment count");
        assert_eq!(
            get_candidate_count_in_conn(&conn, "测试词").expect("count"),
            2
        );
    }

    #[test]
    fn test_delete_candidate_clears_row() {
        let conn = setup();
        upsert_candidate_in_conn(&conn, "测试词").expect("insert");

        let deleted = delete_candidate_in_conn(&conn, "测试词").expect("delete");
        assert!(deleted, "candidate row should be deleted");
        assert_eq!(
            get_candidate_count_in_conn(&conn, "测试词").expect("count"),
            0
        );
    }

    // ============================================================
    // Frequency counter threshold tests (single-word version)
    // ============================================================

    #[test]
    fn freq_001_first_detection_count_1_no_write() {
        let conn = setup();

        let count = upsert_candidate_in_conn(&conn, "测试词").expect("first record");
        assert_eq!(count, 1, "first detection should have count=1");

        let wordbook_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            wordbook_count, 0,
            "wordbook should NOT contain entry before threshold"
        );
    }

    #[test]
    fn freq_002_second_detection_count_2_no_write() {
        let conn = setup();

        let count1 = upsert_candidate_in_conn(&conn, "测试词").expect("record 1");
        assert_eq!(count1, 1);

        let count2 = upsert_candidate_in_conn(&conn, "测试词").expect("record 2");
        assert_eq!(count2, 2, "second detection should have count=2");

        let candidate_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook_candidates", [], |r| r.get(0))
            .expect("count");
        assert_eq!(candidate_count, 1, "should have exactly 1 candidate row");

        let wordbook_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            wordbook_count, 0,
            "wordbook should NOT contain entry before threshold"
        );
    }

    #[test]
    fn freq_003_third_detection_count_3_reaches_threshold() {
        let conn = setup();

        upsert_candidate_in_conn(&conn, "测试词").expect("record 1");
        upsert_candidate_in_conn(&conn, "测试词").expect("record 2");

        let count3 = upsert_candidate_in_conn(&conn, "测试词").expect("record 3");
        assert_eq!(count3, 3, "third detection should have count=3");

        conn.execute(
            "INSERT INTO wordbook (word, source, created_at) \
             VALUES (?1, 'auto_learn', datetime('now'))",
            rusqlite::params!["测试词"],
        )
        .expect("insert to wordbook");

        delete_candidate_in_conn(&conn, "测试词").expect("cleanup candidate");

        let wordbook_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            wordbook_count, 1,
            "wordbook should contain entry after threshold"
        );

        let candidate_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook_candidates", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            candidate_count, 0,
            "candidate table should be cleaned up after write"
        );
    }

    #[test]
    fn freq_004_configurable_threshold_5() {
        let conn = setup();

        for i in 1..=4 {
            let count =
                upsert_candidate_in_conn(&conn, "测试词").expect(&format!("record {}", i));
            assert_eq!(count, i as u32, "detection {} should have count={}", i, i);
        }

        let wordbook_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            wordbook_count, 0,
            "wordbook should NOT contain entry before reaching threshold=5"
        );

        let count5 = upsert_candidate_in_conn(&conn, "测试词").expect("record 5");
        assert_eq!(count5, 5, "fifth detection should have count=5");
    }

    #[test]
    fn freq_extra_different_words_independent() {
        let conn = setup();

        upsert_candidate_in_conn(&conn, "词A").expect("A1");
        upsert_candidate_in_conn(&conn, "词A").expect("A2");

        let count_b = upsert_candidate_in_conn(&conn, "词B").expect("B1");
        assert_eq!(count_b, 1, "B should have count=1 (independent from A)");

        let candidate_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook_candidates", [], |r| r.get(0))
            .expect("count");
        assert_eq!(candidate_count, 2, "should have 2 candidate rows (A and B)");
    }

    // ============================================================
    // WORDBOOK-SINGLEWORD-001-CORE: Migration idempotency tests
    // ============================================================

    #[test]
    fn migration_idempotent_repeated_init_no_duplicate() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(MIGRATION_001).expect("migration 001");
        conn.execute_batch(MIGRATION_002).expect("migration 002");

        // Insert old-format data
        conn.execute(
            "INSERT INTO wordbook (raw, corrected, source, created_at) \
             VALUES ('原词1', '修正词1', 'user', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert old format 1");
        conn.execute(
            "INSERT INTO wordbook (raw, corrected, source, created_at) \
             VALUES ('原词2', '修正词2', 'user', '2024-01-02T00:00:00Z')",
            [],
        )
        .expect("insert old format 2");
        // Duplicate corrected (should dedupe)
        conn.execute(
            "INSERT INTO wordbook (raw, corrected, source, created_at) \
             VALUES ('原词3', '修正词1', 'user', '2024-01-03T00:00:00Z')",
            [],
        )
        .expect("insert dup corrected");

        // Run migration 003 + finalize
        conn.execute_batch(MIGRATION_003).expect("migration 003");
        finalize_singleword_migration(&conn).expect("finalize");

        // Verify: 2 unique words (修正词1 deduped, 修正词2)
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 2, "should have 2 unique words after dedup");

        // Verify columns are single-word
        let has_raw: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('wordbook') WHERE name = 'raw'",
                [],
                |r| r.get(0),
            )
            .expect("check raw");
        assert_eq!(has_raw, 0, "raw column should be gone after migration");

        let has_word: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('wordbook') WHERE name = 'word'",
                [],
                |r| r.get(0),
            )
            .expect("check word");
        assert_eq!(has_word, 1, "word column should exist after migration");
    }

    #[test]
    fn migration_idempotent_second_run_no_data_loss() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(MIGRATION_001).expect("migration 001");
        conn.execute_batch(MIGRATION_002).expect("migration 002");
        conn.execute_batch(MIGRATION_003).expect("migration 003");
        finalize_singleword_migration(&conn).expect("first finalize");

        // Insert single-word data
        conn.execute(
            "INSERT INTO wordbook (word, source, created_at) \
             VALUES ('测试词', 'user', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert");

        // Run migration 003 again + finalize
        conn.execute_batch(MIGRATION_003).expect("migration 003 second");
        finalize_singleword_migration(&conn).expect("second finalize");

        // Data should be preserved
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1, "data must survive idempotent re-run");

        let word: String = conn
            .query_row("SELECT word FROM wordbook", [], |r| r.get(0))
            .expect("word");
        assert_eq!(word, "测试词");
    }

    #[test]
    fn migration_deleted_word_not_revived() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(MIGRATION_001).expect("migration 001");
        conn.execute_batch(MIGRATION_002).expect("migration 002");

        // Insert old format data
        conn.execute(
            "INSERT INTO wordbook (raw, corrected, source, created_at) \
             VALUES ('原词', '要删的词', 'user', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert");
        conn.execute(
            "INSERT INTO wordbook (raw, corrected, source, created_at) \
             VALUES ('原词2', '保留词', 'user', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert 2");

        // Run migration 003 + finalize
        conn.execute_batch(MIGRATION_003).expect("migration 003");
        finalize_singleword_migration(&conn).expect("finalize");

        // Delete one word
        conn.execute("DELETE FROM wordbook WHERE word = '要删的词'", [])
            .expect("delete");

        // Verify deletion
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1, "one word should remain after delete");

        // Re-run migration (simulating app restart)
        conn.execute_batch(MIGRATION_003).expect("migration 003 re-run");
        finalize_singleword_migration(&conn).expect("finalize re-run");

        // Deleted word must not revive
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1, "deleted word must not revive on re-run");

        let word: String = conn
            .query_row("SELECT word FROM wordbook", [], |r| r.get(0))
            .expect("word");
        assert_eq!(word, "保留词", "only the kept word should remain");
    }

    #[test]
    fn migration_candidate_table_corrected_dedup() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(MIGRATION_001).expect("migration 001");
        conn.execute_batch(MIGRATION_002).expect("migration 002");

        // Insert old format candidate data
        conn.execute(
            "INSERT INTO wordbook_candidates (raw, corrected, count, last_seen) \
             VALUES ('原1', '修正词A', 3, '2024-01-01')",
            [],
        )
        .expect("insert candidate 1");
        conn.execute(
            "INSERT INTO wordbook_candidates (raw, corrected, count, last_seen) \
             VALUES ('原2', '修正词A', 5, '2024-01-02')",
            [],
        )
        .expect("insert candidate 2 (same corrected)");
        conn.execute(
            "INSERT INTO wordbook_candidates (raw, corrected, count, last_seen) \
             VALUES ('原3', '修正词B', 2, '2024-01-03')",
            [],
        )
        .expect("insert candidate 3");

        // Run migration 003 + finalize
        conn.execute_batch(MIGRATION_003).expect("migration 003");
        finalize_singleword_migration(&conn).expect("finalize");

        // Verify: 2 unique words in candidates
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook_candidates", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 2, "should have 2 unique candidate words after dedup");

        // 修正词A should have count=5 (MAX via GROUP BY + INSERT OR IGNORE keeps first row)
        let count_a: i64 = conn
            .query_row(
                "SELECT count FROM wordbook_candidates WHERE word = '修正词A'",
                [],
                |r| r.get(0),
            )
            .expect("count A");
        assert_eq!(count_a, 5, "修正词A should have MAX(count)=5");
    }
}