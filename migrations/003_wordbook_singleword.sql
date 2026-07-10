-- WORDBOOK-SINGLEWORD-001-CORE: 词库从词对(raw→corrected)改为单词(word)模式
-- 此迁移必须幂等：重复执行不重复导入，已删单词不复活
--
-- 分两阶段：
--   SQL（本文件）：仅创建新表（IF NOT EXISTS 保证幂等）
--   Rust（init_schema / migrate_wordbook_data）：条件性数据迁移 + DROP旧表 + RENAME新表

-- 1. 创建新表（IF NOT EXISTS 保证幂等）
CREATE TABLE IF NOT EXISTS wordbook_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    word TEXT NOT NULL,
    source TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_wordbook_new_unique
ON wordbook_new(word);

-- 2. 创建新的候选表（单词模式）
CREATE TABLE IF NOT EXISTS wordbook_candidates_new (
    word TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 1,
    last_seen TEXT NOT NULL,
    PRIMARY KEY (word)
);