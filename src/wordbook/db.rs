use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::time::Duration;

use super::cache::WordbookEntry;

const MIGRATION_001: &str = include_str!("../../migrations/001_wordbook.sql");
const MIGRATION_002: &str = include_str!("../../migrations/002_wordbook_candidates.sql");
const MIGRATION_003: &str = include_str!("../../migrations/003_wordbook_singleword.sql");

/// WORDBOOK-SCHEMA-FIX-001: 全新库直接建 word 模式 schema（不执行 001/002 的旧表 DDL、
/// 不执行 legacy import）。复用 003 的 word 列定义但用最终表名，避免两处 schema 定义漂移。
const WORD_SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS wordbook (\
\n    id INTEGER PRIMARY KEY AUTOINCREMENT,\
\n    word TEXT NOT NULL,\
\n    source TEXT NOT NULL,\
\n    created_at TEXT NOT NULL\
\n);\
\n\
\nCREATE UNIQUE INDEX IF NOT EXISTS idx_wordbook_new_unique ON wordbook(word);\
\n\
\nCREATE TABLE IF NOT EXISTS wordbook_candidates (\
\n    word TEXT NOT NULL,\
\n    count INTEGER NOT NULL DEFAULT 1,\
\n    last_seen TEXT NOT NULL,\
\n    PRIMARY KEY (word)\
\n);\
\n";

/// WORDBOOK-SCHEMA-FIX-001: 并发保护。主程序自动学习写入与设置界面 UI 读取是两个
/// 进程访问同一库文件，写事务期间对方读会拿到 SQLITE_BUSY 而立即失败（无重试）。
/// 3000ms 足以覆盖正常的写事务时长，且不会让进程长时间挂起。
const BUSY_TIMEOUT_MS: u64 = 3000;

#[derive(Debug, Clone)]
pub struct StoredWordbookEntry {
    pub id: i64,
    pub word: String,
    pub source: String,
    pub created_at: String,
}

pub fn load_entries() -> Result<Vec<WordbookEntry>> {
    let conn = open_connection()?;
    let mut stmt = conn.prepare("SELECT word, source, created_at FROM wordbook ORDER BY id ASC")?;
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
    let mut stmt =
        conn.prepare("SELECT id, word, source, created_at FROM wordbook ORDER BY id DESC")?;
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
    // WORDBOOK-SCHEMA-FIX-001: 并发保护。主程序自动学习写入与设置界面 UI 读取是两个
    // 进程访问同一库文件，写事务期间对方读会拿到 SQLITE_BUSY 而立即失败。3s 超时
    // 足以覆盖正常写事务，且不让进程长时间挂起。仅加 busy_timeout，不动 journal 模式
    // （WAL 会产生 -wal/-shm 文件涉及 Publish 产物清单，本次边界外）。
    conn.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))?;
    init_schema(&conn)?;
    Ok(conn)
}

/// WORDBOOK-SCHEMA-FIX-001: 按库的实际 schema 状态条件化执行。
///
/// 三态判定（pragma_table_info + sqlite_master 双查）：
/// - **A 全新库**（wordbook 表不存在）→ 直接建 word 模式 schema，不执行 001/002/legacy import
/// - **B 旧库（词对）**（有 raw 列）→ 完整迁移链 001→002→import_legacy_words→source 归一化→003→finalize→source 归一化
/// - **C 已迁移**（有 word 列）→ 完全跳过 001/002/legacy import，只做幂等保障
///
/// 状态 C 必须零写事务（除真的缺索引/缺表时）——原实现每次 open 都跑 DDL + 两条
/// UPDATE，属写放大，且与主程序并发时抬高锁冲突概率。source 归一化改为先 SELECT
/// 判断有无非法值，有才 UPDATE。
///
/// 索引名保持 `idx_wordbook_new_unique` 不变（003 迁移的历史产物，现存库就叫这个名字，
/// 不要为"名字好看"去 drop/recreate，风险大收益低）。
///
/// **残留临时表救援（主控修法一）**：003 迁移中途崩溃可能留下 wordbook_new /
/// wordbook_candidates_new 残留。清理前必须先判断：若真表 wordbook 不存在而
/// wordbook_new 存在 → 这是「崩在 DROP 之后 RENAME 之前」的状态，正确动作是
/// ALTER TABLE wordbook_new RENAME TO wordbook 救回数据，而非 DROP（否则唯一副本
/// 销毁，用户词库静默清空不可恢复）。只有真表存在时，wordbook_new 才是无用半成品，
/// 方可安全 DROP。candidates 侧独立判断同理。
fn init_schema(conn: &Connection) -> Result<()> {
    // 残留临时表救援：先判断真表是否存在，不存在而 _new 存在 → 救回，而非 DROP
    recover_stale_temp_tables(conn)?;

    // 三态判定：先查 wordbook 表是否存在
    let table_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'wordbook'",
        [],
        |row| row.get(0),
    )?;

    if table_exists == 0 {
        // 状态 A：全新库 → 直接建 word 模式 schema，不执行 001/002/legacy import
        conn.execute_batch(WORD_SCHEMA)?;
        return Ok(());
    }

    // 表存在 → 查列名判 B（有 raw）vs C（有 word）
    let has_raw: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('wordbook') WHERE name = 'raw'",
        [],
        |row| row.get(0),
    )?;

    if has_raw > 0 {
        // 状态 B：旧库（词对）→ 完整迁移链
        // 001/002：表已存在会跳过 DDL，索引存在故 CREATE INDEX 也安全（幂等）
        conn.execute_batch(MIGRATION_001)?;
        conn.execute_batch(MIGRATION_002)?;
        import_legacy_words(conn)?;
        normalize_source(conn)?;
        conn.execute_batch(MIGRATION_003)?;
        finalize_singleword_migration(conn)?;
        normalize_source(conn)?;
        return Ok(());
    }

    // 状态 C：已迁移（有 word 列，无 raw 列）→ 幂等保障，不执行 001/002/legacy import
    // 1. 确保 word 模式唯一索引存在（idx_wordbook_new_unique，003 历史产物名字保持不变）
    conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_wordbook_new_unique ON wordbook(word);",
    )?;
    // 2. 确保 wordbook_candidates(word) 表存在
    let candidates_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'wordbook_candidates'",
        [],
        |row| row.get(0),
    )?;
    if candidates_exists == 0 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS wordbook_candidates (\
             \n    word TEXT NOT NULL,\
             \n    count INTEGER NOT NULL DEFAULT 1,\
             \n    last_seen TEXT NOT NULL,\
             \n    PRIMARY KEY (word)\
             \n);",
        )?;
    }
    // 3. source 归一化：先 SELECT 判断有无非法值，有才 UPDATE（避免写放大）
    normalize_source(conn)?;

    Ok(())
}

/// WORDBOOK-SCHEMA-FIX-001（主控修法一）：残留临时表救援。
/// 003 迁移中途崩溃可能留下 wordbook_new / wordbook_candidates_new 残留。清理逻辑：
/// - 真表 wordbook 不存在 而 wordbook_new 存在 → 救回（RENAME _new → 真表），非 DROP
/// - 真表 wordbook 存在 而 wordbook_new 存在 → _new 是无用半成品副本，安全 DROP
/// candidates 侧独立判断同理（可能只有一侧处于中间态）
fn recover_stale_temp_tables(conn: &Connection) -> Result<()> {
    // wordbook 侧
    let wordbook_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='wordbook'",
        [],
        |r| r.get(0),
    )?;
    let wordbook_new_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='wordbook_new'",
        [],
        |r| r.get(0),
    )?;
    if wordbook_new_exists > 0 {
        if wordbook_exists == 0 {
            // 真表不存在而 _new 有数据 → 救回，不 DROP（否则唯一副本销毁）
            log::warn!(
                "WORDBOOK-SCHEMA-FIX-001: recovering wordbook_new (interrupted migration detected, true table missing) — renaming to wordbook to rescue data"
            );
            conn.execute_batch("ALTER TABLE wordbook_new RENAME TO wordbook;")?;
        } else {
            // 真表存在 → _new 是半成品副本，安全 DROP
            conn.execute_batch("DROP TABLE IF EXISTS wordbook_new;")?;
        }
    }

    // candidates 侧（独立判断，可能只有一侧处于中间态）
    let cand_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='wordbook_candidates'",
        [],
        |r| r.get(0),
    )?;
    let cand_new_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='wordbook_candidates_new'",
        [],
        |r| r.get(0),
    )?;
    if cand_new_exists > 0 {
        if cand_exists == 0 {
            log::warn!(
                "WORDBOOK-SCHEMA-FIX-001: recovering wordbook_candidates_new (interrupted migration detected, true table missing) — renaming to wordbook_candidates to rescue data"
            );
            conn.execute_batch(
                "ALTER TABLE wordbook_candidates_new RENAME TO wordbook_candidates;",
            )?;
        } else {
            conn.execute_batch("DROP TABLE IF EXISTS wordbook_candidates_new;")?;
        }
    }
    Ok(())
}

/// WORDBOOK-SCHEMA-FIX-001: source 归一化（先 SELECT 判断有无非法值，有才 UPDATE）。
/// 原实现每次 open 都无条件执行两条 UPDATE，属写放大且与主程序并发时抬高锁冲突概率。
/// 保留既有兜底能力（'auto' 等非法值 → 'system'），不因条件化而丢。
fn normalize_source(conn: &Connection) -> Result<()> {
    let bad_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM wordbook WHERE source NOT IN ('system', 'user')",
        [],
        |row| row.get(0),
    )?;
    if bad_count > 0 {
        conn.execute(
            "UPDATE wordbook SET source = 'system' WHERE source NOT IN ('system', 'user')",
            [],
        )?;
    }
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

    // 3. Replace old tables with new ones — 用事务包裹消除「旧表已删、新表未改名」中间态
    // WORDBOOK-SCHEMA-FIX-001（主控修法二）：SQLite DDL 支持事务，用 unchecked_transaction
    // 把四步 DROP+RENAME 包成一个原子单元，若中途崩溃会回滚，中间态不持久化到磁盘。
    // 原实现四条独立 execute_batch，若进程在 DROP wordbook 与 RENAME wordbook_new 之间被杀，
    // wordbook 表不存在而 wordbook_new holding 唯一数据副本 → 不可逆数据丢失。
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("DROP TABLE IF EXISTS wordbook_candidates;")?;
    tx.execute_batch("DROP TABLE IF EXISTS wordbook;")?;
    tx.execute_batch("ALTER TABLE wordbook_new RENAME TO wordbook;")?;
    tx.execute_batch("ALTER TABLE wordbook_candidates_new RENAME TO wordbook_candidates;")?;
    tx.commit()?;
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
            let count = upsert_candidate_in_conn(&conn, "测试词").expect(&format!("record {}", i));
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
        conn.execute_batch(MIGRATION_003)
            .expect("migration 003 second");
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
        conn.execute_batch(MIGRATION_003)
            .expect("migration 003 re-run");
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

    // ============================================================
    // WORDBOOK-SCHEMA-FIX-001: init_schema 三态条件化测试
    // ============================================================

    /// 辅助：断言当前 schema 是 word 模式（有 word 列、无 raw 列、有 idx_wordbook_new_unique）
    fn assert_word_schema(conn: &Connection) {
        let has_word: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('wordbook') WHERE name = 'word'",
                [],
                |r| r.get(0),
            )
            .expect("check word column");
        assert_eq!(has_word, 1, "word column must exist");

        let has_raw: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('wordbook') WHERE name = 'raw'",
                [],
                |r| r.get(0),
            )
            .expect("check raw column");
        assert_eq!(has_raw, 0, "raw column must NOT exist in word mode");

        let has_idx: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_wordbook_new_unique'",
                [],
                |r| r.get(0),
            )
            .expect("check index");
        assert_eq!(has_idx, 1, "idx_wordbook_new_unique must exist");

        // 旧索引 idx_wordbook_unique 不应存在（防有人把旧索引又建回来）
        let has_old_idx: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_wordbook_unique'",
                [],
                |r| r.get(0),
            )
            .expect("check old index");
        assert_eq!(has_old_idx, 0, "legacy idx_wordbook_unique must NOT exist");
    }

    /// 状态 C 幂等：建好 word 模式库 → 连续 init_schema 两次 → 均 Ok，且第二次无 schema 变化
    /// **这条直接锁死本 bug 不复发，最重要**
    #[test]
    fn fix001_state_c_idempotent_consecutive_init() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        // 先建好 word 模式库（模拟已迁移状态）
        conn.execute_batch(WORD_SCHEMA).expect("build word schema");
        conn.execute(
            "INSERT INTO wordbook (word, source, created_at) \
             VALUES ('风无心', 'system', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert data");

        // 第一次 init_schema（状态 C）
        init_schema(&conn).expect("first init on migrated db");
        assert_word_schema(&conn);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1, "data must survive first init");

        // 第二次 init_schema（状态 C，验证幂等）
        init_schema(&conn).expect("second init must be idempotent");
        assert_word_schema(&conn);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1, "data must survive second init");
    }

    /// 状态 C 修复本 bug 的核心断言：模拟真实失败场景
    /// 旧实现会执行 MIGRATION_001 的 CREATE INDEX ON wordbook(raw,corrected) → no such column
    #[test]
    fn fix001_state_c_does_not_execute_migration_001_index() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        // 模拟 003 迁移完成后的真实状态：wordbook(word) + idx_wordbook_new_unique
        conn.execute_batch(WORD_SCHEMA).expect("build word schema");

        // 旧实现会在这里因 MIGRATION_001 的 CREATE INDEX ON wordbook(raw,corrected) 失败
        // 新实现状态 C 必须跳过 001/002，不报错
        init_schema(&conn).expect("init on migrated db must not fail");
        assert_word_schema(&conn);
    }

    /// 状态 B 迁移：构造 raw/corrected 旧表 + 若干词对数据 → init_schema → 断言变为 word 模式、
    /// 数据按 corrected 侧去重导入、候选表 count 保留
    #[test]
    fn fix001_state_b_migrates_old_pair_table_to_word_mode() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        // 构造旧词对库（001 + 002）
        conn.execute_batch(MIGRATION_001).expect("migration 001");
        conn.execute_batch(MIGRATION_002).expect("migration 002");
        // 插入旧词对数据（含重复 corrected 测去重）
        conn.execute(
            "INSERT INTO wordbook (raw, corrected, source, created_at) \
             VALUES ('风无星', '风无心', 'user', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert 1");
        conn.execute(
            "INSERT INTO wordbook (raw, corrected, source, created_at) \
             VALUES ('风无腥', '风无心', 'user', '2024-01-02T00:00:00Z')",
            [],
        )
        .expect("insert 2 (dup corrected)");
        conn.execute(
            "INSERT INTO wordbook (raw, corrected, source, created_at) \
             VALUES ('吉皮提', 'GPT', 'user', '2024-01-03T00:00:00Z')",
            [],
        )
        .expect("insert 3");
        // 插入旧候选数据
        conn.execute(
            "INSERT INTO wordbook_candidates (raw, corrected, count, last_seen) \
             VALUES ('风无星', '风无心', 3, '2024-01-01')",
            [],
        )
        .expect("insert candidate 1");

        // 执行 init_schema（状态 B → 完整迁移链）
        init_schema(&conn).expect("init on old pair db");

        // 断言：word 模式
        assert_word_schema(&conn);
        // 断言：2 个唯一词（风无心 去重 + GPT）
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 2, "should have 2 unique words after dedup");
        // 断言：风无心 在库
        let has_word: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM wordbook WHERE word = '风无心'",
                [],
                |r| r.get(0),
            )
            .expect("check word");
        assert_eq!(has_word, 1);
        // 断言：候选表 count 保留
        let cand_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook_candidates", [], |r| r.get(0))
            .expect("cand count");
        assert_eq!(cand_count, 1, "candidates should be migrated with count");
        let cand_word_count: i64 = conn
            .query_row(
                "SELECT count FROM wordbook_candidates WHERE word = '风无心'",
                [],
                |r| r.get(0),
            )
            .expect("cand count for word");
        assert_eq!(cand_word_count, 3, "candidate count must be preserved");
    }

    /// 状态 A 全新：空库 → init_schema → 直接是 word 模式，且不存在 idx_wordbook_unique
    #[test]
    fn fix001_state_a_fresh_db_gets_word_mode_directly() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        // 空库，无任何表 → 状态 A
        init_schema(&conn).expect("init on fresh db");
        assert_word_schema(&conn);
        // 确认 wordbook_candidates 表存在
        let cand_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='wordbook_candidates'",
                [],
                |r| r.get(0),
            )
            .expect("check candidates");
        assert_eq!(cand_exists, 1, "wordbook_candidates must exist in fresh db");
    }

    /// 状态 C 下 source 归一化：插入一条 source='auto' 的记录 → init_schema → 变成 'system'
    /// （保留既有兜底能力，不能因条件化而丢）
    #[test]
    fn fix001_state_c_normalizes_invalid_source() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(WORD_SCHEMA).expect("build word schema");
        // 插入非法 source
        conn.execute(
            "INSERT INTO wordbook (word, source, created_at) \
             VALUES ('测试词', 'auto', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert bad source");

        init_schema(&conn).expect("init with bad source");
        let source: String = conn
            .query_row(
                "SELECT source FROM wordbook WHERE word = '测试词'",
                [],
                |r| r.get(0),
            )
            .expect("source");
        assert_eq!(
            source, "system",
            "invalid source 'auto' must be normalized to 'system'"
        );
    }

    /// 状态 C 下无非法 source 时 init_schema 不写事务（幂等保障，避免写放大）
    #[test]
    fn fix001_state_c_no_write_when_source_already_valid() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(WORD_SCHEMA).expect("build word schema");
        conn.execute(
            "INSERT INTO wordbook (word, source, created_at) \
             VALUES ('测试词', 'system', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert valid source");

        // 连续 init_schema 两次，均不应报错（幂等）
        init_schema(&conn).expect("first init");
        init_schema(&conn).expect("second init");

        // 数据不变
        let source: String = conn
            .query_row(
                "SELECT source FROM wordbook WHERE word = '测试词'",
                [],
                |r| r.get(0),
            )
            .expect("source");
        assert_eq!(source, "system");
    }

    /// 第四种状态：003 迁移中途崩溃留下 wordbook_new 残留 + 旧 wordbook(raw) 表并存
    /// init_schema 应先清理残留临时表，再走完整 B 迁移链
    #[test]
    fn fix001_state_d_interrupted_migration_with_stale_temp_tables() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        // 构造旧词对库
        conn.execute_batch(MIGRATION_001).expect("migration 001");
        conn.execute_batch(MIGRATION_002).expect("migration 002");
        conn.execute(
            "INSERT INTO wordbook (raw, corrected, source, created_at) \
             VALUES ('原词', '保留词', 'user', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert old data");
        // 模拟 003 部分执行后崩溃：wordbook_new 临时表残留（003 已建表但未完成 finalize）
        conn.execute_batch(MIGRATION_003)
            .expect("migration 003 partial");
        // 此时 wordbook_new + wordbook_candidates_new 残留 + 旧 wordbook(raw) 并存

        // init_schema 应先清理残留临时表，再走 B 迁移链
        init_schema(&conn).expect("init with stale temp tables");

        assert_word_schema(&conn);
        // 数据正确迁移
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1, "data must be migrated despite stale temp tables");
        let word: String = conn
            .query_row("SELECT word FROM wordbook", [], |r| r.get(0))
            .expect("word");
        assert_eq!(word, "保留词");
        // 残留临时表已清理
        let temp_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE '%_new'",
                [],
                |r| r.get(0),
            )
            .expect("temp count");
        assert_eq!(temp_count, 0, "stale _new temp tables must be cleaned up");
    }

    // ============================================================
    // WORDBOOK-SCHEMA-FIX-001：真实文件系统上的 init_schema 端到端测试
    // ============================================================

    /// 真实文件库的 init_schema 端到端测试。
    /// 使用临时文件代替 in-memory db，验证 SQLite 文件 I/O 路径下的三态逻辑。
    ///
    /// 限制：open_connection() 的 db_path() 决议（exe 同级硬编码，DEC-011/DEC-032）
    /// 无法在测试中注入自定义路径。本条通过 Connection::open(file) + init_schema 直接调用，
    /// 模拟完整的文件 I/O 路径。`load_entries()` 等 public API 的端到端连通性受此覆盖：
    /// init_schema 不报错 → 后续 SELECT/INSERT 均可正常执行。
    #[test]
    fn fix001_real_file_init_schema_persistence() {
        let tmp = std::env::temp_dir().join(format!("wordbook_test_{}", std::process::id()));
        let path = tmp.join("wordbook.sqlite");
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        // State A: 全新文件库 → init_schema 建 word 模式
        {
            let conn = Connection::open(&path).expect("open fresh file db");
            conn.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
                .expect("busy timeout");
            init_schema(&conn).expect("init_schema on fresh file");
            assert_word_schema(&conn);
        }

        // State C: 重开已迁移文件库 → init_schema 幂等
        {
            let conn = Connection::open(&path).expect("re-open file db");
            conn.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
                .expect("busy timeout");
            init_schema(&conn).expect("init_schema idempotent on file");
            assert_word_schema(&conn);
        }

        // 数据持久化验证：文件关闭后重开，数据仍可读
        {
            let conn = Connection::open(&path).expect("open for insert");
            conn.execute(
                "INSERT INTO wordbook (word, source, created_at) \
                 VALUES ('端到端测试词', 'system', '2024-01-01T00:00:00Z')",
                [],
            )
            .expect("insert data");
        }
        {
            let conn = Connection::open(&path).expect("open for verify");
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM wordbook WHERE word = '端到端测试词'",
                    [],
                    |r| r.get(0),
                )
                .expect("count");
            assert_eq!(count, 1, "data must persist across re-open on real file");
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ============================================================
    // WORDBOOK-SCHEMA-FIX-001 主控修法一：残留临时表救援测试
    // ============================================================

    /// ① 构造「wordbook 不存在 + wordbook_new 有数据」→ init_schema → 数据被救回 wordbook
    /// **直接锁死数据丢失**：原实现会 DROP wordbook_new 销毁唯一副本
    #[test]
    fn fix001_recover_wordbook_new_when_true_table_missing() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        // 模拟崩在「DROP wordbook 之后 RENAME 之前」：wordbook 不存在，wordbook_new 有数据
        conn.execute_batch(WORD_SCHEMA).expect("build word schema");
        conn.execute_batch("ALTER TABLE wordbook RENAME TO wordbook_new;")
            .expect("simulate post-DROP pre-RENAME");
        // 此时 wordbook 表不存在，wordbook_new holding 唯一数据副本
        conn.execute(
            "INSERT INTO wordbook_new (word, source, created_at) \
             VALUES ('风无心', 'system', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert into wordbook_new");

        // init_schema 必须救回（RENAME），而非 DROP
        init_schema(&conn).expect("init must rescue wordbook_new");

        // 断言：数据已救回到 wordbook，条目数正确
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1, "data must be rescued to wordbook");
        let word: String = conn
            .query_row("SELECT word FROM wordbook", [], |r| r.get(0))
            .expect("word");
        assert_eq!(word, "风无心");
        // 残留临时表已不存在
        let temp_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='wordbook_new'",
                [],
                |r| r.get(0),
            )
            .expect("temp count");
        assert_eq!(temp_count, 0, "wordbook_new must be gone after rescue");
    }

    /// ② 构造「wordbook 真表存在 + wordbook_new 残留」→ init_schema → 残留被清理且真表数据未受影响
    #[test]
    fn fix001_drop_wordbook_new_when_true_table_present() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        // 真表 wordbook 存在且有数据
        conn.execute_batch(WORD_SCHEMA).expect("build word schema");
        conn.execute(
            "INSERT INTO wordbook (word, source, created_at) \
             VALUES ('真表数据', 'system', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert into wordbook");
        // 构造残留 wordbook_new（半成品副本）
        conn.execute_batch(
            "CREATE TABLE wordbook_new (id INTEGER PRIMARY KEY, word TEXT, source TEXT, created_at TEXT);",
        )
        .expect("create stale wordbook_new");
        conn.execute(
            "INSERT INTO wordbook_new (word, source, created_at) \
             VALUES ('半成品', 'user', '2024-01-02T00:00:00Z')",
            [],
        )
        .expect("insert into stale wordbook_new");

        // init_schema 应 DROP 残留 wordbook_new（真表存在 → _new 是无用半成品）
        init_schema(&conn).expect("init with stale wordbook_new and true table present");

        // 真表数据未受影响
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1, "true table data must be preserved");
        let word: String = conn
            .query_row("SELECT word FROM wordbook", [], |r| r.get(0))
            .expect("word");
        assert_eq!(word, "真表数据");
        // 残留已清理
        let temp_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='wordbook_new'",
                [],
                |r| r.get(0),
            )
            .expect("temp count");
        assert_eq!(temp_count, 0, "stale wordbook_new must be dropped");
    }

    /// ③ candidates 侧同构造：wordbook_candidates 不存在 + wordbook_candidates_new 有数据 → 救回
    #[test]
    fn fix001_recover_candidates_new_when_true_table_missing() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        // 模拟崩在「DROP candidates 之后 RENAME 之前」
        conn.execute_batch(WORD_SCHEMA).expect("build word schema");
        // wordbook 真表存在（不受影响），但 candidates 被 DROP 了，candidates_new holding 数据
        conn.execute_batch("ALTER TABLE wordbook_candidates RENAME TO wordbook_candidates_new;")
            .expect("simulate candidates post-DROP pre-RENAME");
        conn.execute(
            "INSERT INTO wordbook_candidates_new (word, count, last_seen) \
             VALUES ('风无心', 3, '2024-01-01')",
            [],
        )
        .expect("insert into candidates_new");
        // wordbook 也有数据（不应受 candidates 救援影响）
        conn.execute(
            "INSERT INTO wordbook (word, source, created_at) \
             VALUES ('风无心', 'system', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert into wordbook");

        // init_schema 应救回 candidates（RENAME），而非 DROP
        init_schema(&conn).expect("init must rescue candidates_new");

        // 断言：candidates 数据已救回
        let cand_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook_candidates", [], |r| r.get(0))
            .expect("cand count");
        assert_eq!(cand_count, 1, "candidates data must be rescued");
        let cand_word_count: i64 = conn
            .query_row(
                "SELECT count FROM wordbook_candidates WHERE word = '风无心'",
                [],
                |r| r.get(0),
            )
            .expect("cand word count");
        assert_eq!(cand_word_count, 3, "candidate count must be preserved");
        // wordbook 真表未受影响
        let wb_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM wordbook", [], |r| r.get(0))
            .expect("wb count");
        assert_eq!(wb_count, 1, "wordbook true table must be unaffected");
        // 残留已不存在
        let temp_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='wordbook_candidates_new'",
                [],
                |r| r.get(0),
            )
            .expect("temp count");
        assert_eq!(temp_count, 0, "candidates_new must be gone after rescue");
    }

    // ============================================================
    // WORDBOOK-SCHEMA-FIX-001：schema 定义漂移防护对齐单测
    // ============================================================

    /// 对齐单测：分别构造状态 A（空库）与状态 B（旧词对库）跑完 init_schema，
    /// 对比两者最终 schema 是否一致。用 pragma_table_info(wordbook) 列名有序列表 +
    /// sqlite_master 中该表的索引名集合做断言。将来任何人只改一处 schema 定义，
    /// 测试立刻红，漂移风险被机制化消除。
    #[test]
    fn fix001_schema_alignment_state_a_equals_state_b() {
        // 状态 A：空库 → init_schema
        let conn_a = Connection::open_in_memory().expect("in-memory db A");
        init_schema(&conn_a).expect("init state A");
        let cols_a: Vec<String> = conn_a
            .prepare("SELECT name FROM pragma_table_info('wordbook') ORDER BY cid")
            .expect("prepare A")
            .query_map([], |r| r.get::<_, String>(0))
            .expect("query A")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect A");
        let idx_a: Vec<String> = conn_a
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='wordbook' ORDER BY name")
            .expect("prepare idx A")
            .query_map([], |r| r.get::<_, String>(0))
            .expect("query idx A")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect idx A");

        // 状态 B：旧词对库 → init_schema
        let conn_b = Connection::open_in_memory().expect("in-memory db B");
        conn_b.execute_batch(MIGRATION_001).expect("migration 001");
        conn_b.execute_batch(MIGRATION_002).expect("migration 002");
        conn_b
            .execute(
                "INSERT INTO wordbook (raw, corrected, source, created_at) \
                 VALUES ('原词', '保留词', 'user', '2024-01-01T00:00:00Z')",
                [],
            )
            .expect("insert old data");
        init_schema(&conn_b).expect("init state B");
        let cols_b: Vec<String> = conn_b
            .prepare("SELECT name FROM pragma_table_info('wordbook') ORDER BY cid")
            .expect("prepare B")
            .query_map([], |r| r.get::<_, String>(0))
            .expect("query B")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect B");
        let idx_b: Vec<String> = conn_b
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='wordbook' ORDER BY name")
            .expect("prepare idx B")
            .query_map([], |r| r.get::<_, String>(0))
            .expect("query idx B")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect idx B");

        assert_eq!(
            cols_a, cols_b,
            "wordbook columns must be identical between state A and state B"
        );
        assert_eq!(
            idx_a, idx_b,
            "wordbook index names must be identical between state A and state B"
        );
    }
}
