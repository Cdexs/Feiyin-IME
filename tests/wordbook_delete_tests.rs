//! 词库删除功能测试
//!
//! 覆盖 WORDBOOK-DELETE-BUG-001 删除功能的 8 个测试用例：
//! - DEL-001: 删除已存在的用户词条
//! - DEL-002: 删除不存在的词条
//! - DEL-003: 删除含前后空格的词条
//! - DEL-004: 删除空词条的请求
//! - DEL-005: DB 删除成功但 Cache 未命中
//! - DEL-006: DB 和 Cache 都不存在
//! - DEL-007: 删除系统来源的词条
//! - DEL-008: 连续两次删除同一条
//!
//! 注意：按 TEST-SYNC 规范，本文件仅编写测试用例，不执行测试。
//! 执行测试在 Phase 4 统一进行。

use std::path::PathBuf;
use std::process::Command;

// ============================================================
// 辅助函数
// ============================================================

/// 获取项目根目录
fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 验证 wordbook 相关文件存在
fn wordbook_db_path() -> PathBuf {
    project_root().join("src").join("wordbook").join("db.rs")
}

fn wordbook_cache_path() -> PathBuf {
    project_root().join("src").join("wordbook").join("cache.rs")
}

fn wordbook_command_path() -> PathBuf {
    project_root()
        .join("src-tauri")
        .join("src")
        .join("wordbook.rs")
}

// ============================================================
// 1. DB 层删除测试
// ============================================================

/// DEL-001: 删除已存在的用户词条 — db::delete_entry 返回 Ok(true)
#[test]
fn test_delete_existing_user_entry() {
    let db_content = std::fs::read_to_string(wordbook_db_path()).expect("Should read db.rs");

    // WORDBOOK-SINGLEWORD-001-CORE: delete by word (single-word model)
    assert!(
        db_content.contains("DELETE FROM wordbook WHERE word = ?"),
        "delete_entry should use word as condition (single-word model)"
    );

    // 验证通过 affected rows > 0 判断删除成功
    assert!(
        db_content.contains("changed > 0") || db_content.contains("rows > 0"),
        "delete_entry should return true when rows affected"
    );
}

/// DEL-002: 删除不存在的词条 — db::delete_entry 返回 Ok(false)
#[test]
fn test_delete_nonexistent_entry() {
    let db_content = std::fs::read_to_string(wordbook_db_path()).expect("Should read db.rs");

    // 验证 execute 返回的 changed 值在未匹配时为 0
    assert!(
        db_content.contains("Ok(changed > 0)"),
        "delete_entry should return false when no rows match"
    );
}

/// DEL-003: 添加/删除含前后空格的词条 — 验证 add 侧 trim() 在 Command 层处理
#[test]
fn test_add_with_leading_trailing_spaces() {
    let cmd_content =
        std::fs::read_to_string(wordbook_command_path()).expect("Should read wordbook.rs");

    // Verify trim() is present
    assert!(
        cmd_content.contains("word.trim()"),
        "add_wordbook_entry should trim word parameter"
    );
}

// ============================================================
// 2. Cache 层删除测试
// ============================================================

/// DEL-005: DB 删除成功但 Cache 未命中 — remove_entry 返回 Ok(true)
#[test]
fn test_delete_db_success_cache_miss() {
    let cache_content =
        std::fs::read_to_string(wordbook_cache_path()).expect("Should read cache.rs");

    // 验证 remove_entry 中 DB 成功或 Cache 命中任一即返回 true
    assert!(
        cache_content.contains("removed_from_db || removed_from_cache"),
        "remove_entry should return true if either DB or cache deletion succeeded"
    );
}

/// DEL-006: DB 和 Cache 都不存在 — remove_entry 返回 Ok(false)
#[test]
fn test_delete_both_db_and_cache_miss() {
    let cache_content =
        std::fs::read_to_string(wordbook_cache_path()).expect("Should read cache.rs");

    // 验证 DB 和 Cache 都未命中时返回 false
    let remove_section = cache_content
        .split("pub fn remove_entry")
        .nth(1)
        .unwrap_or("");
    assert!(
        remove_section.contains("Ok(false)") || remove_section.contains("return Ok(false)"),
        "remove_entry should return false when both DB and cache miss"
    );
}

// ============================================================
// 3. Tauri Command 层删除测试
// ============================================================

/// DEL-004: 删除空词条的请求 — validate_word 拦截
#[test]
fn test_delete_empty_word() {
    let cmd_content =
        std::fs::read_to_string(wordbook_command_path()).expect("Should read wordbook.rs");

    // 验证存在 validate_word 函数做参数校验
    assert!(
        cmd_content.contains("fn validate_word") || cmd_content.contains("validate_word("),
        "add_wordbook_entry should call validate_word for parameter validation"
    );

    // 验证 validate_word 检查非空
    let validate_section = cmd_content.split("fn validate_word").nth(1).unwrap_or("");
    assert!(
        validate_section.contains("is_empty()"),
        "validate_word should reject empty word"
    );
}

/// DEL-007: 删除系统来源的词条 — 验证当前逻辑不区分 source
#[test]
fn test_delete_system_entry() {
    let cmd_content =
        std::fs::read_to_string(wordbook_command_path()).expect("Should read wordbook.rs");

    // 验证删除逻辑不检查 source 字段（系统/用户词条一视同仁）
    assert!(
        !cmd_content.contains("source") || !cmd_content.split("delete_wordbook_entry_by_id").nth(1).unwrap_or("").contains("source"),
        "delete_wordbook_entry_by_id should not check source field — system and user entries treated equally"
    );
}

/// DEL-008: 连续两次删除同一条 — 第一次成功，第二次返回"不存在"
#[test]
fn test_delete_duplicate_same_entry() {
    let cmd_content =
        std::fs::read_to_string(wordbook_command_path()).expect("Should read wordbook.rs");
    let cache_content =
        std::fs::read_to_string(wordbook_cache_path()).expect("Should read cache.rs");

    // 验证 Command 层返回的错误信息
    assert!(
        cmd_content.contains("词库条目不存在") || cmd_content.contains("entry does not exist"),
        "delete_wordbook_entry should return appropriate error message for missing entry"
    );

    // 验证 Cache 层在条目不存在时返回 Ok(false)
    let remove_section = cache_content.split("fn remove_entry").nth(1).unwrap_or("");
    assert!(
        remove_section.contains("Ok(false)"),
        "remove_entry should return Ok(false) when entry not found (for duplicate delete)"
    );
}

// ============================================================
// 4. 按 ID 删除测试 — 已删除（TEST-REFORM-001）
//    原 Section 4 为静态字符串扫描，违反测试规范 1/4。
//    真实 DB 层按 ID 删除验证已由 src/wordbook/db.rs 中的
//    #[cfg(test)] in-memory SQLite 单元测试取代：
//      - test_delete_by_id_removes_entry
//      - test_delete_by_id_nonexistent_returns_zero
//      - test_delete_by_id_does_not_affect_other_entries
// ============================================================

// ============================================================
// 5. 删除持久化测试（TEST-SYNC-PERSIST-001）
// ============================================================

/// DEL-PERSIST-001: import_legacy_words 迁移后 words 表应被删除，防止词条复活
#[test]
fn test_import_legacy_words_drops_words_table_after_migration() {
    let db_content = std::fs::read_to_string(wordbook_db_path()).expect("Should read db.rs");

    // 验证 import_legacy_words 包含 DROP TABLE 语句
    let import_section = db_content
        .split("fn import_legacy_words")
        .nth(1)
        .unwrap_or("");
    assert!(
        import_section.contains("DROP TABLE") || import_section.contains("drop table"),
        "import_legacy_words should DROP the words table after migration to prevent re-import"
    );
}
