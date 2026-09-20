-- WORDBOOK-HITCOUNT-263: 热词按使用频率筛选
-- 迁移库：加两列（ALTER 非幂等，调用方须先查 pragma_table_info 确认列缺失再执行，见 db.rs::ensure_hitcount_columns）
-- 全新库：不执行本文件，列已写进 db.rs::WORD_SCHEMA（避免两处 schema 漂移）
ALTER TABLE wordbook ADD COLUMN hit_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE wordbook ADD COLUMN last_used_at TEXT;
