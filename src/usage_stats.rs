//! STATS-475：使用统计（设置界面「我的」页的本周统计）。
//!
//! Gavin 2026-10-04 规则：
//! 1. 时长 = 用户输入语音的长度累加；
//! 2. 字数 = 最终输出文字的字数（中日韩按字，英文按单词）；
//! 3. 优化 LLM 调用次数 = 格式化输出 LLM 调用次数；
//! 4. 周统计 = 固定本周一到周日为一个周期。
//!
//! 主程序每产出一次最终文字记一条（后台线程），设置界面经 `#[path]` 复用本文件只读汇总。
//! 数据放 exe 同目录 `usage-stats.sqlite`（与词库库文件分开，互不影响建表 / 迁移；升级不覆盖）。

use chrono::{DateTime, Datelike, Duration, Local, TimeZone};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::PathBuf;

/// 「我的」页三项分项：本地快速 / 在线 ASR / 本地流式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageCategory {
    LocalFast,
    OnlineAsr,
    LocalStreaming,
}

impl UsageCategory {
    fn as_str(self) -> &'static str {
        match self {
            UsageCategory::LocalFast => "local_fast",
            UsageCategory::OnlineAsr => "online_asr",
            UsageCategory::LocalStreaming => "local_streaming",
        }
    }
}

/// 中日韩文字（汉字 / 假名 / 谚文）：每个字记 1。
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F // 汉字
        | 0x3040..=0x309F | 0x30A0..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9F // 假名
        | 0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF // 谚文
    )
}

/// 规则 2：最终输出文字的字数 —— 中日韩每字记 1；其余字母 / 数字连成一串记 1 个单词；
/// 词内连接符不断词：`'` `’` `-` 两侧都是字母数字（don't、state-of-the-art），
/// `.` `:` `,` 两侧都是数字（3.5、3:30、1,000）；标点、空白不计。
pub fn count_output_words(text: &str) -> u64 {
    let chars: Vec<char> = text.chars().collect();
    let word_char = |c: char| c.is_alphanumeric() && !is_cjk(c);
    let mut n = 0u64;
    let mut in_word = false;
    for (i, &c) in chars.iter().enumerate() {
        if is_cjk(c) {
            n += 1;
            in_word = false;
        } else if c.is_alphanumeric() {
            if !in_word {
                n += 1;
                in_word = true;
            }
        } else {
            let prev = i.checked_sub(1).map(|j| chars[j]);
            let next = chars.get(i + 1).copied();
            let joins = in_word
                && match c {
                    '\'' | '’' | '-' => next.is_some_and(word_char),
                    '.' | ':' | ',' => {
                        prev.is_some_and(|p| p.is_ascii_digit())
                            && next.is_some_and(|d| d.is_ascii_digit())
                    }
                    _ => false,
                };
            if !joins {
                in_word = false;
            }
        }
    }
    n
}

/// 规则 4：`now` 所在的本周 [周一 0 点, 下周一 0 点)（本地时区）→ unix 秒。
pub fn week_bounds(now: DateTime<Local>) -> (i64, i64) {
    let today = now.date_naive();
    let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let next_monday = monday + Duration::days(7);
    let at_midnight = |d: chrono::NaiveDate| {
        let naive = d.and_hms_opt(0, 0, 0).expect("00:00:00 有效");
        Local
            .from_local_datetime(&naive)
            .earliest()
            .unwrap_or_else(|| Local.from_utc_datetime(&naive))
            .timestamp()
    };
    (at_midnight(monday), at_midnight(next_monday))
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CategoryUsage {
    pub speech_ms: u64,
    pub words: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct WeekUsage {
    /// 本周一日期（YYYY-MM-DD）与周日日期，供界面显示统计区间。
    pub week_start: String,
    pub week_end: String,
    pub speech_ms: u64,
    pub words: u64,
    pub llm_calls: u64,
    pub local_fast: CategoryUsage,
    pub online_asr: CategoryUsage,
    pub local_streaming: CategoryUsage,
}

const BUSY_TIMEOUT_MS: u64 = 3000;

fn db_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("usage-stats.sqlite")
}

fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS usage_sessions (
             id INTEGER PRIMARY KEY,
             ts INTEGER NOT NULL,
             category TEXT NOT NULL,
             speech_ms INTEGER NOT NULL,
             words INTEGER NOT NULL,
             llm_calls INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_usage_sessions_ts ON usage_sessions(ts);
         CREATE TABLE IF NOT EXISTS terminal_sessions (
             id INTEGER PRIMARY KEY,
             terminal_id TEXT NOT NULL,
             login_ts INTEGER NOT NULL,
             logout_ts INTEGER
         );
         CREATE INDEX IF NOT EXISTS idx_terminal_sessions ON terminal_sessions(terminal_id, login_ts);",
    )?;
    // ASR-SERVER-491 S5（设计 §16.4）：加「终端」一维。旧库与新库走同一条迁移：列不存在就加，
    // 默认值 `local` ⇒ 已有记录原样算本机，本机写入路径（不带该列）一行不改。
    let has_terminal: bool = conn
        .prepare("SELECT 1 FROM pragma_table_info('usage_sessions') WHERE name = 'terminal_id'")?
        .exists([])?;
    if !has_terminal {
        if let Err(e) = conn.execute_batch(&format!(
            "ALTER TABLE usage_sessions ADD COLUMN terminal_id TEXT NOT NULL DEFAULT '{LOCAL_TERMINAL}'"
        )) {
            // 主程序与设置界面同时首开旧库：另一方刚加上 ⇒ 忽略「重复列」。
            if !e.to_string().contains("duplicate column") {
                return Err(e);
            }
        }
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_usage_sessions_terminal ON usage_sessions(terminal_id, ts);",
    )
}

/// 本机记录的终端编号（`usage_sessions.terminal_id` 默认值）。
pub const LOCAL_TERMINAL: &str = "local";

fn open() -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path())?;
    // 主程序写、设置界面读是两个进程（同词库库文件做法）。
    conn.busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS))?;
    init_schema(&conn)?;
    Ok(conn)
}

fn record_in(
    conn: &Connection,
    ts: i64,
    category: UsageCategory,
    speech_ms: u64,
    words: u64,
    llm_calls: u64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO usage_sessions (ts, category, speech_ms, words, llm_calls)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            ts,
            category.as_str(),
            speech_ms as i64,
            words as i64,
            llm_calls as i64
        ],
    )?;
    Ok(())
}

fn week_usage_in(conn: &Connection, start: i64, end: i64) -> rusqlite::Result<WeekUsage> {
    let mut usage = WeekUsage::default();
    let mut stmt = conn.prepare(
        "SELECT category, SUM(speech_ms), SUM(words), SUM(llm_calls)
         FROM usage_sessions WHERE ts >= ?1 AND ts < ?2 AND terminal_id = 'local'
         GROUP BY category",
    )?;
    let rows = stmt.query_map(params![start, end], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    for row in rows {
        let (category, speech_ms, words, llm_calls) = row?;
        let (speech_ms, words, llm_calls) = (
            speech_ms.max(0) as u64,
            words.max(0) as u64,
            llm_calls.max(0) as u64,
        );
        usage.speech_ms += speech_ms;
        usage.words += words;
        usage.llm_calls += llm_calls;
        let slot = match category.as_str() {
            "local_fast" => &mut usage.local_fast,
            "online_asr" => &mut usage.online_asr,
            "local_streaming" => &mut usage.local_streaming,
            _ => continue,
        };
        slot.speech_ms += speech_ms;
        slot.words += words;
    }
    Ok(usage)
}

/// 主程序：记一次最终输出（调用方放后台线程，失败只记日志）。
#[allow(dead_code)] // 设置界面经 #[path] 复用本文件，只用读接口
pub fn record(
    category: UsageCategory,
    speech_ms: u64,
    final_text: &str,
    llm_calls: u64,
) -> rusqlite::Result<()> {
    let conn = open()?;
    record_in(
        &conn,
        Local::now().timestamp(),
        category,
        speech_ms,
        count_output_words(final_text),
        llm_calls,
    )
}

// ---------------------------------------------------------------------------
// ASR-SERVER-491 S5：远程终端统计（设计 §16.4）。「我的」页只看本机（上面按 `local` 过滤），
// 远程终端只在「识别服务」页看。字数 / 时长口径与本机同一套函数。
// ---------------------------------------------------------------------------

/// 远程终端出一次终稿记一条（内核统计线程调用）。
#[allow(dead_code)] // 设置界面只读
pub fn record_terminal(
    conn: &Connection,
    terminal_id: &str,
    speech_ms: u64,
    final_text: &str,
) -> rusqlite::Result<()> {
    record_terminal_in(
        conn,
        Local::now().timestamp(),
        terminal_id,
        speech_ms,
        count_output_words(final_text),
    )
}

fn record_terminal_in(
    conn: &Connection,
    ts: i64,
    terminal_id: &str,
    speech_ms: u64,
    words: u64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO usage_sessions (ts, category, speech_ms, words, llm_calls, terminal_id)
         VALUES (?1, 'remote', ?2, ?3, 0, ?4)",
        params![ts, speech_ms as i64, words as i64, terminal_id],
    )?;
    Ok(())
}

/// 终端登录（连接通过鉴权）：返回记录号，退出时用它补退出时间。
#[allow(dead_code)]
pub fn terminal_login(conn: &Connection, terminal_id: &str, ts: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO terminal_sessions (terminal_id, login_ts) VALUES (?1, ?2)",
        params![terminal_id, ts],
    )?;
    Ok(conn.last_insert_rowid())
}

#[allow(dead_code)]
pub fn terminal_logout(conn: &Connection, row: i64, ts: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE terminal_sessions SET logout_ts = ?2 WHERE id = ?1 AND logout_ts IS NULL",
        params![row, ts],
    )?;
    Ok(())
}

/// 内核统计线程用：打开（必要时创建）统计库。
#[allow(dead_code)]
pub fn open_for_write() -> rusqlite::Result<Connection> {
    open()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TerminalUsage {
    pub terminal_id: String,
    /// 本周（周一 ~ 周日）。
    pub week: CategoryUsage,
    /// 累计。
    pub total: CategoryUsage,
}

fn terminal_usage_in(
    conn: &Connection,
    start: i64,
    end: i64,
) -> rusqlite::Result<Vec<TerminalUsage>> {
    let mut stmt = conn.prepare(
        "SELECT terminal_id,
                SUM(CASE WHEN ts >= ?1 AND ts < ?2 THEN speech_ms ELSE 0 END),
                SUM(CASE WHEN ts >= ?1 AND ts < ?2 THEN words ELSE 0 END),
                SUM(speech_ms), SUM(words)
         FROM usage_sessions WHERE terminal_id <> 'local'
         GROUP BY terminal_id ORDER BY terminal_id",
    )?;
    let u = |v: i64| v.max(0) as u64;
    let rows = stmt.query_map(params![start, end], |r| {
        Ok(TerminalUsage {
            terminal_id: r.get(0)?,
            week: CategoryUsage {
                speech_ms: u(r.get(1)?),
                words: u(r.get(2)?),
            },
            total: CategoryUsage {
                speech_ms: u(r.get(3)?),
                words: u(r.get(4)?),
            },
        })
    })?;
    rows.collect()
}

/// 设置界面「识别服务」页：每个远程终端的本周 / 累计时长与字数（库不存在 ⇒ 空，不建库）。
#[allow(dead_code)]
pub fn terminal_usage() -> rusqlite::Result<Vec<TerminalUsage>> {
    if !db_path().is_file() {
        return Ok(Vec::new());
    }
    let (start, end) = week_bounds(Local::now());
    terminal_usage_in(&open()?, start, end)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TerminalSession {
    pub login_ts: i64,
    /// `None` = 仍在线，或上次内核异常退出没记到退出时间。
    pub logout_ts: Option<i64>,
}

fn terminal_sessions_in(
    conn: &Connection,
    terminal_id: &str,
    limit: u32,
) -> rusqlite::Result<Vec<TerminalSession>> {
    let mut stmt = conn.prepare(
        "SELECT login_ts, logout_ts FROM terminal_sessions WHERE terminal_id = ?1
         ORDER BY login_ts DESC, id DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![terminal_id, limit], |r| {
        Ok(TerminalSession {
            login_ts: r.get(0)?,
            logout_ts: r.get(1)?,
        })
    })?;
    rows.collect()
}

/// 设置界面：某终端最近若干次登录 / 退出（新的在前）。
#[allow(dead_code)]
pub fn terminal_sessions(terminal_id: &str, limit: u32) -> rusqlite::Result<Vec<TerminalSession>> {
    if !db_path().is_file() {
        return Ok(Vec::new());
    }
    terminal_sessions_in(&open()?, terminal_id, limit)
}

/// 设置界面：本周（周一 ~ 周日）汇总。
#[allow(dead_code)] // 主程序只写不读
pub fn week_usage() -> rusqlite::Result<WeekUsage> {
    let now = Local::now();
    let (start, end) = week_bounds(now);
    // BUILD-478 D1（主控定）：读不产生副作用 —— 库还不存在（从没输出过文字）⇒ 全 0，不建空库；
    // 库只在主程序首次记录时创建（与 `record` 一致）。
    let mut usage = if db_path().is_file() {
        week_usage_in(&open()?, start, end)?
    } else {
        WeekUsage::default()
    };
    let monday = Local
        .timestamp_opt(start, 0)
        .single()
        .map(|d| d.date_naive())
        .unwrap_or_else(|| now.date_naive());
    usage.week_start = monday.format("%Y-%m-%d").to_string();
    usage.week_end = (monday + Duration::days(6)).format("%Y-%m-%d").to_string();
    Ok(usage)
}

#[cfg(test)]
mod stats475_tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn stats475_words_cjk_by_char_english_by_word() {
        assert_eq!(
            count_output_words("今天天气不错。"),
            6,
            "中文按字，标点不计"
        );
        assert_eq!(count_output_words("Hello, world!"), 2, "英文按单词");
        assert_eq!(count_output_words("我用 Claude 写 v2 代码，OK？"), 8);
        assert_eq!(
            count_output_words("こんにちは、カタカナ"),
            9,
            "日文假名按字"
        );
        assert_eq!(count_output_words("안녕하세요 세계"), 7, "韩文谚文按字");
        assert_eq!(
            count_output_words("don't stop state-of-the-art"),
            3,
            "词内连接符不断词"
        );
        assert_eq!(
            count_output_words("会议在 3:30 开始，预算 2026 元"),
            10,
            "3:30 记 1 个"
        );
        assert_eq!(count_output_words("增长 3.5%，共 1,000 人。"), 6);
        assert_eq!(
            count_output_words("Hi, Bob. OK"),
            3,
            "非数字两侧的逗号句号照常断词"
        );
        assert_eq!(count_output_words("  ，。！？ -- '' "), 0, "只有标点空白");
        assert_eq!(count_output_words(""), 0);
    }

    #[test]
    fn stats475_week_is_monday_to_sunday() {
        // 2026-10-04 是周日 ⇒ 本周 = 09-28（周一）~ 10-04（周日）。
        let sunday_night = Local.with_ymd_and_hms(2026, 10, 4, 23, 59, 59).unwrap();
        let (s, e) = week_bounds(sunday_night);
        let s_dt = Local.timestamp_opt(s, 0).unwrap();
        let e_dt = Local.timestamp_opt(e, 0).unwrap();
        assert_eq!(s_dt.date_naive().to_string(), "2026-09-28");
        assert_eq!((s_dt.hour(), s_dt.minute()), (0, 0));
        assert_eq!(
            e_dt.date_naive().to_string(),
            "2026-10-05",
            "下周一 0 点（不含）"
        );
        // 周一 0 点整属于新的一周。
        let monday = Local.with_ymd_and_hms(2026, 10, 5, 0, 0, 0).unwrap();
        assert_eq!(week_bounds(monday).0, e);
        assert!(sunday_night.timestamp() < e && sunday_night.timestamp() >= s);
    }

    /// ASR-SERVER-491 S5：旧库迁移不丢数据；「我的」页只算本机；远程终端分开汇总；登录 / 退出成对。
    #[test]
    fn asr491_terminal_dimension_migrates_and_isolates() {
        let conn = Connection::open_in_memory().unwrap();
        // 旧库：只有 S5 之前的表结构与一条本机记录。
        conn.execute_batch(
            "CREATE TABLE usage_sessions (id INTEGER PRIMARY KEY, ts INTEGER NOT NULL,
               category TEXT NOT NULL, speech_ms INTEGER NOT NULL, words INTEGER NOT NULL,
               llm_calls INTEGER NOT NULL);
             INSERT INTO usage_sessions (ts, category, speech_ms, words, llm_calls)
               VALUES (1000100, 'local_fast', 4000, 7, 1);",
        )
        .unwrap();
        init_schema(&conn).unwrap();
        init_schema(&conn).unwrap(); // 重复打开不再加列
        let (s, e) = (1_000_000, 1_604_800);
        record_in(&conn, s + 1, UsageCategory::LocalStreaming, 1_000, 2, 0).unwrap();
        record_terminal_in(&conn, s + 2, "t1", 5_000, 9).unwrap();
        record_terminal_in(&conn, s - 5, "t1", 1_000, 1).unwrap(); // 上周
        record_terminal_in(&conn, s + 3, "t2", 2_000, 3).unwrap();

        let mine = week_usage_in(&conn, s, e).unwrap();
        assert_eq!(
            (mine.speech_ms, mine.words),
            (5_000, 9),
            "旧记录算本机，远程不进「我的」"
        );

        let t = terminal_usage_in(&conn, s, e).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].terminal_id, "t1");
        assert_eq!(
            t[0].week,
            CategoryUsage {
                speech_ms: 5_000,
                words: 9
            }
        );
        assert_eq!(
            t[0].total,
            CategoryUsage {
                speech_ms: 6_000,
                words: 10
            }
        );

        let r1 = terminal_login(&conn, "t1", 10).unwrap();
        terminal_login(&conn, "t1", 20).unwrap(); // 仍在线
        terminal_logout(&conn, r1, 15).unwrap();
        terminal_logout(&conn, r1, 99).unwrap(); // 已记过不覆盖
        let ss = terminal_sessions_in(&conn, "t1", 10).unwrap();
        assert_eq!(
            ss,
            vec![
                TerminalSession {
                    login_ts: 20,
                    logout_ts: None
                },
                TerminalSession {
                    login_ts: 10,
                    logout_ts: Some(15)
                },
            ]
        );
    }

    #[test]
    fn stats475_week_usage_sums_by_category_and_window() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let (s, e) = (1_000_000, 1_604_800);
        record_in(&conn, s, UsageCategory::LocalFast, 3_000, 10, 1).unwrap();
        record_in(&conn, s + 10, UsageCategory::LocalFast, 2_000, 5, 0).unwrap();
        record_in(&conn, s + 20, UsageCategory::OnlineAsr, 60_000, 100, 1).unwrap();
        record_in(&conn, e - 1, UsageCategory::LocalStreaming, 30_000, 50, 1).unwrap();
        record_in(&conn, s - 1, UsageCategory::LocalFast, 99_000, 999, 9).unwrap(); // 上周
        record_in(&conn, e, UsageCategory::OnlineAsr, 99_000, 999, 9).unwrap(); // 下周
        let u = week_usage_in(&conn, s, e).unwrap();
        assert_eq!(u.speech_ms, 95_000);
        assert_eq!(u.words, 165);
        assert_eq!(u.llm_calls, 3);
        assert_eq!(
            u.local_fast,
            CategoryUsage {
                speech_ms: 5_000,
                words: 15
            }
        );
        assert_eq!(
            u.online_asr,
            CategoryUsage {
                speech_ms: 60_000,
                words: 100
            }
        );
        assert_eq!(
            u.local_streaming,
            CategoryUsage {
                speech_ms: 30_000,
                words: 50
            }
        );
    }
}
