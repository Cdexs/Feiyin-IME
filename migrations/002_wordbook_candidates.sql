CREATE TABLE IF NOT EXISTS wordbook_candidates (
    raw TEXT NOT NULL,
    corrected TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 1,
    last_seen TEXT NOT NULL,
    PRIMARY KEY (raw, corrected)
);
