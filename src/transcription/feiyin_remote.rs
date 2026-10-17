//! ASR-SERVER-491 S8：本机连另一台电脑的飞音识别服务（设计 §8.3 / §8.5 / §13.3）。
//!
//! 只管「建连」和 `run-task` 扩展字段；会话收发与阿里在线识别共用
//! [`super::qwen_inference::transcribe_streaming_feiyin`]（同一个 `run-task` 协议）。
//!
//! - **内外网**：「自动」= 两个地址同时试 TCP（只试 TCP，不带 key，不惊动对方鉴权）；
//!   内网通了直接用；外网先通则再等内网 [`LAN_GRACE`]，内网没来就用外网。
//!   在家时内网几毫秒就通，出门时内网不通也不用干等超时。
//! - **证书**：只认配对时记下的指纹（`asr_shared::tls_pin`）；对不上 ⇒ `FingerprintMismatch`，提示重新配对。
//! - **错误**：对方的错误码原样带进错误串（`服务端错误：<码>`），由 `convert_to_friendly_error` 转人话。

use std::net::TcpStream;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use tungstenite::http::Uri;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{
    client_tls_with_config, ClientRequestBuilder, Connector, Error as WsError, HandshakeError,
    WebSocket,
};

use crate::asr_shared::remote::RemotePairing;
use crate::asr_shared::tls_pin::{self, PinnedCert};

/// `run-task` 的 `model`（对方服务端不看，只进日志 / 统计摘要）。
pub const MODEL_NAME: &str = "feiyin-local-realtime";

/// 自动模式下外网先连上后，再给内网的等待时间（内网优先，§8.3）。
const LAN_GRACE: Duration = Duration::from_millis(300);

/// 本机没配对 / 配对不全（界面提示去设置里配对）。
pub const ERR_NOT_PAIRED: &str = "NotPaired";
/// 对方证书与配对时记下的指纹不一致（对方重装 / 被冒充 ⇒ 重新配对）。
pub const ERR_FINGERPRINT: &str = "FingerprintMismatch";

/// `run-task` 的 `parameters.feiyin`：本机身份 + 请对方做的文字节点。
///
/// `postprocess`：本机走在线识别时跳过数字规整主通道（V091-ITN-SKIP-ONLINE-215，默认服务端做过），
/// 所以请对方做同音纠错 + 数字规整，与对阿里服务的假设一致（设计 §13.3 第 4 条）。
pub fn run_task_ext(p: &RemotePairing) -> serde_json::Value {
    serde_json::json!({
        "device_id": p.device_id,
        "device_name": p.effective_device_name(),
        "postprocess": ["homophone", "itn"],
    })
}

/// 建连：挑地址 → TCP → TLS（指纹钉扎）→ WebSocket（Bearer key）。
pub fn connect(
    p: &RemotePairing,
    timeout: Duration,
) -> Result<WebSocket<MaybeTlsStream<TcpStream>>> {
    if !p.is_paired() || p.device_id.is_empty() {
        bail!("服务端错误：{ERR_NOT_PAIRED}");
    }
    let t0 = Instant::now();
    let (tcp, endpoint, net) = race_tcp(&p.candidates(), timeout)?;
    tcp.set_read_timeout(Some(timeout))
        .context("网络失败：设置读取超时失败")?;
    tcp.set_write_timeout(Some(timeout))
        .context("网络失败：设置写入超时失败")?;
    log::info!(
        "[Latency] ASR feiyin TCP connected via {net} ({endpoint}) in {:.0}ms",
        t0.elapsed().as_millis()
    );

    let t_tls = Instant::now();
    let verifier = PinnedCert::new(Some(&p.fingerprint));
    let config = tls_pin::client_config(Arc::clone(&verifier))
        .map_err(|e| anyhow!("网络失败：TLS 配置失败 - {e}"))?;
    let uri: Uri = format!("wss://{endpoint}/")
        .parse()
        .context("网络失败：地址无效")?;
    let request =
        ClientRequestBuilder::new(uri).with_header("Authorization", format!("Bearer {}", p.key));
    let (ws, _) = client_tls_with_config(request, tcp, None, Some(Connector::Rustls(config)))
        .map_err(|e| match e {
            HandshakeError::Failure(e) => map_handshake_error(&e, &verifier),
            HandshakeError::Interrupted(_) => anyhow!("网络失败：TLS 握手意外中断"),
        })?;
    log::info!(
        "[Latency] ASR feiyin TLS+WS handshake in {:.0}ms",
        t_tls.elapsed().as_millis()
    );
    Ok(ws)
}

/// 候选地址同时试 TCP；排在前面的优先（自动模式 = 内网在前）。返回 `(连接, 地址, "lan"|"wan")`。
fn race_tcp(
    candidates: &[(&str, &'static str)],
    timeout: Duration,
) -> Result<(TcpStream, String, &'static str)> {
    if candidates.is_empty() {
        bail!("服务端错误：{ERR_NOT_PAIRED}");
    }
    let (tx, rx) = mpsc::channel();
    for (rank, (endpoint, net)) in candidates.iter().enumerate() {
        let (tx, endpoint, net) = (tx.clone(), endpoint.to_string(), *net);
        // 输了的那路连上后随线程结束自然关闭（只建了 TCP，对方看到的是一次空连接）。
        std::thread::spawn(move || {
            let r = tls_pin::tcp_connect(&endpoint, timeout);
            let _ = tx.send((rank, endpoint, net, r));
        });
    }
    drop(tx);
    let mut best: Option<(TcpStream, String, &'static str)> = None;
    let mut deadline: Option<Instant> = None;
    let mut last_err = String::new();
    loop {
        let msg = match deadline {
            None => rx.recv().ok(),
            Some(d) => rx
                .recv_timeout(d.saturating_duration_since(Instant::now()))
                .ok(),
        };
        let Some((rank, endpoint, net, r)) = msg else {
            break;
        };
        match r {
            Ok(s) if rank == 0 => return Ok((s, endpoint, net)),
            Ok(s) => {
                if best.is_none() {
                    best = Some((s, endpoint, net));
                    deadline = Some(Instant::now() + LAN_GRACE);
                }
            }
            Err(e) => {
                log::info!("[ASR-SERVER-491] {net} {endpoint} connect failed: {e}");
                last_err = e.to_string();
            }
        }
    }
    best.ok_or_else(|| anyhow!("网络失败：连接失败 - {last_err}"))
}

fn map_handshake_error(e: &WsError, verifier: &PinnedCert) -> anyhow::Error {
    if verifier.mismatched() {
        return anyhow!("服务端错误：{ERR_FINGERPRINT}");
    }
    if let WsError::Http(resp) = e {
        // 网关握手拒绝时正文就是错误码（InvalidKey / KeyRevoked / KeyExpired / ServicePaused）。
        let code = resp
            .body()
            .as_deref()
            .map(String::from_utf8_lossy)
            .unwrap_or_default();
        return anyhow!(
            "服务端错误：{} (HTTP {})",
            code.trim(),
            resp.status().as_u16()
        );
    }
    anyhow!("网络失败：连接失败 - {e}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn run_task_ext_carries_identity_and_postprocess() {
        let p = RemotePairing {
            device_id: "dev1".into(),
            device_name: "书房".into(),
            ..Default::default()
        };
        let v = run_task_ext(&p);
        assert_eq!(v["device_id"], "dev1");
        assert_eq!(v["device_name"], "书房");
        assert_eq!(v["postprocess"], serde_json::json!(["homophone", "itn"]));
    }

    #[test]
    fn unpaired_fails_before_network() {
        let err = connect(&RemotePairing::default(), Duration::from_millis(50))
            .unwrap_err()
            .to_string();
        assert!(err.contains(ERR_NOT_PAIRED), "{err}");
    }

    /// 内网优先：两路都通 ⇒ 用排在前面的；前面不通 ⇒ 用后面的。
    #[test]
    fn race_prefers_first_candidate_and_falls_back() {
        let a = TcpListener::bind("127.0.0.1:0").unwrap();
        let b = TcpListener::bind("127.0.0.1:0").unwrap();
        let (ea, eb) = (
            a.local_addr().unwrap().to_string(),
            b.local_addr().unwrap().to_string(),
        );
        let t = Duration::from_secs(2);
        let (_, ep, net) = race_tcp(&[(&ea, "lan"), (&eb, "wan")], t).unwrap();
        assert_eq!((ep.as_str(), net), (ea.as_str(), "lan"));
        // 关掉内网那个端口 ⇒ 落到外网。
        drop(a);
        let (_, ep, net) = race_tcp(&[(&ea, "lan"), (&eb, "wan")], t).unwrap();
        assert_eq!((ep.as_str(), net), (eb.as_str(), "wan"));
        drop(b);
        let err = race_tcp(&[(&ea, "lan"), (&eb, "wan")], t).unwrap_err();
        assert!(err.to_string().starts_with("网络失败"), "{err}");
    }
}
