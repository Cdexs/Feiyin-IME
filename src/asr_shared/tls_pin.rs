//! ASR-SERVER-491 S8：终端侧证书钉扎（设计 §8.5）。
//!
//! 网关用自签证书，不走 CA 链：终端只认配对时记下的那一个指纹（证书 DER 的 SHA-256）。
//! 签名仍按 TLS 规范校验（证明对方确实持有该证书的私钥），只是「信谁」换成了指纹。
//! 手输配对时还不知道指纹 ⇒ [`probe_fingerprint`] 先取回来给用户确认一次（同 SSH 首次连接）。

use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, ClientConfig, DigitallySignedStruct, SignatureScheme};

use super::pairing::fingerprint;

/// 只认一个指纹的校验器。`want = None` ⇒ 什么证书都收（只用于取指纹），并记下看到的指纹。
#[derive(Debug)]
pub struct PinnedCert {
    want: Option<String>,
    seen: Mutex<Option<String>>,
    algs: WebPkiSupportedAlgorithms,
}

impl PinnedCert {
    pub fn new(want: Option<&str>) -> Arc<Self> {
        Arc::new(Self {
            want: want.map(str::to_string),
            seen: Mutex::new(None),
            algs: provider().signature_verification_algorithms,
        })
    }

    /// 握手时对方出示的证书指纹（还没握手 ⇒ `None`）。
    pub fn seen(&self) -> Option<String> {
        self.seen.lock().ok().and_then(|g| g.clone())
    }

    /// 握手失败后判断：是不是因为证书换了（与配对时不一致）。
    pub fn mismatched(&self) -> bool {
        matches!((&self.want, self.seen()), (Some(w), Some(s)) if *w != s)
    }
}

impl ServerCertVerifier for PinnedCert {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fp = fingerprint(end_entity.as_ref());
        if let Ok(mut g) = self.seen.lock() {
            *g = Some(fp.clone());
        }
        match &self.want {
            Some(w) if *w != fp => Err(rustls::Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            )),
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

/// 显式用 ring（与全工程一致，不依赖进程级默认 provider 是否装过）。
fn provider() -> CryptoProvider {
    rustls::crypto::ring::default_provider()
}

/// 终端 TLS 配置：只走 TLS 1.3（网关只开 1.3），证书按指纹认。
pub fn client_config(verifier: Arc<PinnedCert>) -> io::Result<Arc<ClientConfig>> {
    let cfg = ClientConfig::builder_with_provider(Arc::new(provider()))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(io::Error::other)?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Ok(Arc::new(cfg))
}

/// TLS 用的服务器名：`主机:端口` 里的主机（IPv6 去方括号）。证书按指纹认，名字只是协议必填项。
pub fn server_name(endpoint: &str) -> io::Result<ServerName<'static>> {
    let host = endpoint
        .rsplit_once(':')
        .map_or(endpoint, |(h, _)| h)
        .trim_start_matches('[')
        .trim_end_matches(']');
    ServerName::try_from(host.to_string())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad host"))
}

/// TCP 连接：主机名解析出的地址逐个试（IPv6 / IPv4 都可能有），每个地址 `timeout`。
pub fn tcp_connect(endpoint: &str, timeout: Duration) -> io::Result<TcpStream> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, "no address");
    for addr in endpoint.to_socket_addrs()? {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(s) => return Ok(s),
            Err(e) => last = e,
        }
    }
    Err(last)
}

type TlsStream = rustls::StreamOwned<rustls::ClientConnection, TcpStream>;

/// TCP + TLS 握手（`want = None` 不校验指纹）。握手失败时校验器里仍留着对方指纹，供判断「证书换了」。
fn tls_connect(
    endpoint: &str,
    verifier: &Arc<PinnedCert>,
    timeout: Duration,
) -> io::Result<TlsStream> {
    let config = client_config(Arc::clone(verifier))?;
    let mut tcp = tcp_connect(endpoint, timeout)?;
    tcp.set_read_timeout(Some(timeout))?;
    tcp.set_write_timeout(Some(timeout))?;
    let mut conn =
        rustls::ClientConnection::new(config, server_name(endpoint)?).map_err(io::Error::other)?;
    while conn.is_handshaking() {
        conn.complete_io(&mut tcp)?;
    }
    Ok(rustls::StreamOwned::new(conn, tcp))
}

/// 连上去只做 TLS 握手，取回对方证书指纹（手输配对的首次确认用；不发 key、不升级 WebSocket）。
pub fn probe_fingerprint(endpoint: &str, timeout: Duration) -> io::Result<String> {
    let verifier = PinnedCert::new(None);
    let mut s = tls_connect(endpoint, &verifier, timeout)?;
    s.conn.send_close_notify();
    let _ = s.conn.complete_io(&mut s.sock);
    verifier
        .seen()
        .ok_or_else(|| io::Error::other("no certificate"))
}

/// 「测试连接」的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessCheck {
    /// 握手通过（key 有效、服务在运行）。
    Ok,
    /// 证书与配对时记下的不一致。
    FingerprintMismatch,
    /// 网关拒绝：HTTP 状态 + 正文里的错误码（`InvalidKey` / `KeyRevoked` / `KeyExpired` / `ServicePaused`）。
    Rejected(u16, String),
}

/// 测试连接：证书钉扎 + WebSocket 升级握手（带 key），拿到 101 即断开。
///
/// 只走到握手：网关在握手里验 key，设备绑定与登录记录在 `run-task` 时才发生，所以测试不会记一次登录。
/// 网络不通 ⇒ `Err`。
pub fn check_access(
    endpoint: &str,
    fingerprint: &str,
    key: &str,
    timeout: Duration,
) -> io::Result<AccessCheck> {
    use std::io::{Read, Write};
    let verifier = PinnedCert::new(Some(fingerprint));
    let mut s = match tls_connect(endpoint, &verifier, timeout) {
        Ok(s) => s,
        Err(_) if verifier.mismatched() => return Ok(AccessCheck::FingerprintMismatch),
        Err(e) => return Err(e),
    };
    // 固定的 Sec-WebSocket-Key 即可：只看状态行，不继续走 WebSocket。
    let req = format!(
        "GET / HTTP/1.1\r\nHost: {endpoint}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\
         Authorization: Bearer {key}\r\n\r\n"
    );
    s.write_all(req.as_bytes())?;
    s.flush()?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    // 读到头部结束；拒绝时正文很短（错误码），网关随即断开，读到 EOF 为止。
    loop {
        match s.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if buf.is_empty() => return Err(e),
            Err(_) => break,
        }
        if let Some(end) = find_header_end(&buf) {
            if status_code(&buf) == Some(101) || buf.len() > end + 64 || buf.len() > 8192 {
                break;
            }
        }
    }
    s.conn.send_close_notify();
    let _ = s.conn.complete_io(&mut s.sock);
    let code = status_code(&buf).ok_or_else(|| io::Error::other("bad response"))?;
    if code == 101 {
        return Ok(AccessCheck::Ok);
    }
    let body = find_header_end(&buf)
        .map(|e| String::from_utf8_lossy(&buf[e..]).trim().to_string())
        .unwrap_or_default();
    Ok(AccessCheck::Rejected(code, body))
}

fn find_header_end(b: &[u8]) -> Option<usize> {
    b.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

fn status_code(b: &[u8]) -> Option<u16> {
    let line = b.split(|&c| c == b'\n').next()?;
    std::str::from_utf8(line)
        .ok()?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_status_parsing() {
        let r = b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 10\r\n\r\nKeyRevoked";
        assert_eq!(status_code(r), Some(401));
        let e = find_header_end(r).unwrap();
        assert_eq!(&r[e..], b"KeyRevoked");
        assert_eq!(
            status_code(b"HTTP/1.1 101 Switching Protocols\r\n"),
            Some(101)
        );
        assert_eq!(status_code(b"garbage"), None);
        assert_eq!(find_header_end(b"HTTP/1.1 101\r\n"), None);
    }

    #[test]
    fn server_name_from_endpoint() {
        assert!(matches!(
            server_name("192.168.1.5:7860").unwrap(),
            ServerName::IpAddress(_)
        ));
        assert!(matches!(
            server_name("[fe80::1]:7860").unwrap(),
            ServerName::IpAddress(_)
        ));
        assert!(matches!(
            server_name("asr.example.com:7860").unwrap(),
            ServerName::DnsName(_)
        ));
    }

    #[test]
    fn pinned_verifier_accepts_only_its_fingerprint() {
        let cert = CertificateDer::from(vec![1u8, 2, 3]);
        let name = ServerName::try_from("a.com").unwrap();
        let right = fingerprint(&[1, 2, 3]);
        let ok = PinnedCert::new(Some(&right));
        assert!(ok
            .verify_server_cert(&cert, &[], &name, &[], UnixTime::now())
            .is_ok());
        assert!(!ok.mismatched());
        let bad = PinnedCert::new(Some("00"));
        assert!(bad
            .verify_server_cert(&cert, &[], &name, &[], UnixTime::now())
            .is_err());
        assert!(bad.mismatched(), "证书换了要能认出来（提示重新配对）");
        let probe = PinnedCert::new(None);
        assert!(probe
            .verify_server_cert(&cert, &[], &name, &[], UnixTime::now())
            .is_ok());
        assert_eq!(probe.seen().as_deref(), Some(right.as_str()));
    }
}
