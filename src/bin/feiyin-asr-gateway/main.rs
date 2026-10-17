//! ASR-SERVER-491 S4：飞音识别服务网关（设计 §2 / §4 / §8）。
//!
//! 独立进程，由输入法内核拉起（S4c）：`feiyin-asr-gateway --core <IPC 端点>`，口令经环境变量
//! `FEIYIN_ASR_TOKEN` 传入（不出现在命令行，别的进程看不到）。
//!
//! 只做对外的事：TLS、终端鉴权、`run-task` 协议转换。**不加载任何模型**——识别全在内核里，
//! 网关崩了不影响本机输入，内核断开网关即退出。

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[allow(dead_code)]
#[path = "../../asr_shared/mod.rs"]
mod asr_shared;
#[allow(dead_code)]
#[path = "../../asr_service/frame.rs"]
mod frame;

mod core_link;
mod tls;
mod ws;

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio_rustls::TlsAcceptor;

use asr_shared::settings::ServiceSettings;

/// 同时在线连接上限（每路一个 TLS 会话 + 少量缓冲；推理通道上限由内核另管）。
const MAX_CONNS: usize = 64;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Err(e) = run() {
        log::error!("[asr-gateway] 启动失败：{e}");
        std::process::exit(2);
    }
}

fn run() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let mut core = None;
    while let Some(a) = args.next() {
        if a == "--core" {
            core = args.next();
        }
    }
    let core = core.ok_or_else(|| std::io::Error::other("missing --core <endpoint>"))?;
    let token = std::env::var("FEIYIN_ASR_TOKEN")
        .map_err(|_| std::io::Error::other("missing FEIYIN_ASR_TOKEN"))?;
    std::env::remove_var("FEIYIN_ASR_TOKEN");

    let settings = ServiceSettings::load();
    let (tls, fp) = tls::load_or_create()?;
    log::info!("[asr-gateway] 证书指纹 {fp}");
    let link = core_link::CoreLink::connect(&core, &token)?;
    let shared = Arc::new(ws::Shared::new(link));
    shared.prune();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(serve(shared, TlsAcceptor::from(tls), settings.port))
}

/// 监听 `[::]` 与 `0.0.0.0`。双栈系统（macOS / Linux 默认）上 `[::]` 已覆盖 IPv4，第二个绑定会失败——忽略即可；
/// Windows 的 IPv6 套接字默认只收 IPv6，两个都绑得上。
async fn serve(shared: Arc<ws::Shared>, acceptor: TlsAcceptor, port: u16) -> std::io::Result<()> {
    let mut listeners = Vec::new();
    for addr in [
        SocketAddr::from((Ipv6Addr::UNSPECIFIED, port)),
        SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)),
    ] {
        match TcpListener::bind(addr).await {
            Ok(l) => {
                log::info!("[asr-gateway] 监听 {addr}");
                listeners.push(l);
            }
            Err(e) => log::info!("[asr-gateway] 未监听 {addr}：{e}"),
        }
    }
    if listeners.is_empty() {
        return Err(std::io::Error::other(format!("port {port} unavailable")));
    }
    let limit = Arc::new(Semaphore::new(MAX_CONNS));
    let mut tasks = Vec::new();
    for l in listeners {
        let (shared, acceptor, limit) = (shared.clone(), acceptor.clone(), limit.clone());
        tasks.push(tokio::spawn(async move {
            loop {
                let (tcp, peer) = match l.accept().await {
                    Ok(x) => x,
                    Err(_) => {
                        // 句柄耗尽之类：稍等再接，避免空转吃满 CPU。
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        continue;
                    }
                };
                // 满了直接断：不排队，客户端按网络错误重试。
                let Ok(permit) = limit.clone().try_acquire_owned() else {
                    continue;
                };
                let (shared, acceptor) = (shared.clone(), acceptor.clone());
                tokio::spawn(async move {
                    ws::serve_conn(shared, acceptor, tcp, peer).await;
                    drop(permit);
                });
            }
        }));
    }
    for t in tasks {
        let _ = t.await;
    }
    Ok(())
}
