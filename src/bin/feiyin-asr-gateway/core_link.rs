//! 网关 ↔ 内核 IPC 客户端（帧格式与内核端共用 `asr_service/frame.rs`，设计 §3）。
//!
//! - 一条连接多路复用所有会话：写线程串行写帧（异步侧只往 channel 里丢，不阻塞运行时），
//!   读线程按会话号把 `Result` / `End` 分发给对应会话。
//! - 内核断开 ⇒ 网关直接退出：模型都在内核里，网关单独活着没有意义（内核重启时会重新拉起网关）。

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::frame::{read_frame, write_frame, Frame, Kind, OpenParams};

type Routes = Arc<Mutex<HashMap<u64, UnboundedSender<Frame>>>>;

pub struct CoreLink {
    tx: mpsc::Sender<Frame>,
    routes: Routes,
    next: AtomicU64,
}

impl CoreLink {
    /// 连内核并握手。`endpoint` = 内核给的管道基名（Windows）/ socket 路径（Unix）。
    pub fn connect(endpoint: &str, token: &str) -> io::Result<Self> {
        let (reader, mut writer) = open(endpoint)?;
        write_frame(
            &mut writer,
            &Frame::new(Kind::Hello, 0, token.as_bytes().to_vec()),
        )?;
        let (tx, rx) = mpsc::channel::<Frame>();
        std::thread::Builder::new()
            .name("asr-core-write".into())
            .spawn(move || {
                while let Ok(f) = rx.recv() {
                    if let Err(e) = write_frame(&mut writer, &f) {
                        log::error!("[asr-gateway] 写内核失败，退出：{e}");
                        std::process::exit(1);
                    }
                }
            })?;
        let routes: Routes = Arc::default();
        let r2 = routes.clone();
        std::thread::Builder::new()
            .name("asr-core-read".into())
            .spawn(move || read_loop(reader, r2))?;
        Ok(Self {
            tx,
            routes,
            next: AtomicU64::new(1),
        })
    }

    /// 开一个会话：先登记路由再发 `Open`（内核回得再快也不会丢第一帧）。
    pub fn open(&self, params: &OpenParams) -> (u64, UnboundedReceiver<Frame>) {
        let sid = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = unbounded_channel();
        lock(&self.routes).insert(sid, tx);
        self.send(Frame::json(Kind::Open, sid, params));
        (sid, rx)
    }

    pub fn send(&self, f: Frame) {
        // 写线程只会因写失败退出整个进程，所以这里发不出去的情况不存在。
        let _ = self.tx.send(f);
    }

    /// 会话结束（收到 `End` 或终端断开）：撤路由。断开时调用方另发 `Cancel`。
    pub fn close(&self, sid: u64) {
        lock(&self.routes).remove(&sid);
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn read_loop(mut reader: Box<dyn Read + Send>, routes: Routes) {
    loop {
        match read_frame(&mut reader) {
            Ok(Some(f)) => {
                let end = f.kind == Kind::End;
                let mut map = lock(&routes);
                if let Some(tx) = map.get(&f.session) {
                    let _ = tx.send(f.clone());
                }
                if end {
                    map.remove(&f.session);
                }
            }
            Ok(None) => {
                log::info!("[asr-gateway] 内核已断开，退出");
                std::process::exit(0);
            }
            Err(e) => {
                log::error!("[asr-gateway] 读内核失败，退出：{e}");
                std::process::exit(1);
            }
        }
    }
}

type Pair = (Box<dyn Read + Send>, Box<dyn Write + Send>);

/// Windows：先开 `.in`（写）——内核在它上面等握手——再开 `.out`（读）。两条管道内核都已建好，
/// 打开顺序与内核 `accept` 的等待顺序一致。
#[cfg(target_os = "windows")]
fn open(endpoint: &str) -> io::Result<Pair> {
    use std::fs::OpenOptions;
    let w = OpenOptions::new()
        .write(true)
        .open(format!("{endpoint}.in"))?;
    let r = OpenOptions::new()
        .read(true)
        .open(format!("{endpoint}.out"))?;
    Ok((Box::new(r), Box::new(w)))
}

#[cfg(unix)]
fn open(endpoint: &str) -> io::Result<Pair> {
    let s = std::os::unix::net::UnixStream::connect(endpoint)?;
    let r = s.try_clone()?;
    Ok((Box::new(r), Box::new(s)))
}
