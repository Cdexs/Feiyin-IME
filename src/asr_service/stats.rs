//! ASR-SERVER-491 S5：远程终端统计写库（设计 §16.4）。
//!
//! 每条网关连接一个统计线程：登录 / 退出 / 终稿都丢给它，IPC 读线程与会话线程不碰数据库、不跑 VAD。
//! 库在第一次真有事可记时才打开（服务开着但没人用 ⇒ 不建库）。网关断开 ⇒ 线程把还开着的登录记录补上退出时间。

use std::collections::HashMap;
use std::thread::JoinHandle;

use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::{transcription, usage_stats};

pub(super) enum StatsOp {
    /// 网关连接号 `conn` 上的终端通过鉴权。
    Login {
        conn: u64,
        terminal: String,
    },
    Logout {
        conn: u64,
    },
    /// 一路远程会话出了终稿：时长 = 有效语音（去静音，与「我的」页同口径）。
    Final {
        terminal: String,
        text: String,
        pcm: Vec<f32>,
    },
}

pub(super) struct Stats {
    tx: Option<Sender<StatsOp>>,
    handle: Option<JoinHandle<()>>,
}

impl Stats {
    pub(super) fn start() -> Self {
        let (tx, rx) = unbounded();
        let handle = std::thread::Builder::new()
            .name("asr-stats".into())
            .spawn(move || run(rx))
            .map_err(|e| log::warn!("[ASR-SERVER-491] stats thread failed: {e}"))
            .ok();
        Self {
            tx: handle.is_some().then_some(tx),
            handle,
        }
    }

    pub(super) fn sender(&self) -> Option<Sender<StatsOp>> {
        self.tx.clone()
    }

    pub(super) fn send(&self, op: StatsOp) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(op);
        }
    }
}

impl Drop for Stats {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn run(rx: Receiver<StatsOp>) {
    let mut db: Option<rusqlite::Connection> = None;
    let mut open_rows: HashMap<u64, i64> = HashMap::new();
    let now = || chrono::Local::now().timestamp();
    for op in rx.iter() {
        if db.is_none() {
            match usage_stats::open_for_write() {
                Ok(c) => db = Some(c),
                Err(e) => {
                    log::warn!("[ASR-SERVER-491] stats db unavailable: {e}");
                    continue;
                }
            }
        }
        let Some(conn) = db.as_ref() else { continue };
        let r = match op {
            StatsOp::Login { conn: c, terminal } => {
                usage_stats::terminal_login(conn, &terminal, now()).map(|row| {
                    open_rows.insert(c, row);
                })
            }
            StatsOp::Logout { conn: c } => match open_rows.remove(&c) {
                Some(row) => usage_stats::terminal_logout(conn, row, now()),
                None => Ok(()),
            },
            StatsOp::Final {
                terminal,
                text,
                pcm,
            } => {
                let speech_ms = transcription::effective_speech_ms_isolated(&pcm)
                    .unwrap_or(pcm.len() as u64 * 1000 / 16_000);
                usage_stats::record_terminal(conn, &terminal, speech_ms, &text)
            }
        };
        if let Err(e) = r {
            log::warn!("[ASR-SERVER-491] stats write failed: {e}");
        }
    }
    // 网关断开（含网关崩溃）：还在线的都记退出。
    if let Some(conn) = db.as_ref() {
        let ts = now();
        for row in open_rows.into_values() {
            let _ = usage_stats::terminal_logout(conn, row, ts);
        }
    }
}
