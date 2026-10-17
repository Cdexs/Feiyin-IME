//! 自签证书 + TLS 1.3 服务端配置（设计 §8.3）。
//!
//! - 首次启动生成一张自签证书（ECDSA P-256），之后一直复用：指纹不变，已配对的终端不用重配。
//! - 终端靠「证书指纹钉扎」认服务端（配对时从二维码拿到指纹），不依赖 CA，内网 IP / 外网域名都能用。
//! - 只开 TLS 1.3；加密实现用 ring（与工程里已有的 rustls 同一套，不引入 aws-lc / OpenSSL）。

use std::io;
use std::sync::Arc;

use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::rustls::{self, ServerConfig};

use crate::asr_shared::{pairing, write_atomic};

/// 读已有证书，没有（或读坏了）⇒ 生成新的。返回 `(TLS 配置, 指纹)`。
pub fn load_or_create() -> io::Result<(Arc<ServerConfig>, String)> {
    let (cert, key) = match (
        std::fs::read(pairing::cert_path()),
        std::fs::read(pairing::key_path()),
    ) {
        (Ok(c), Ok(k)) if !c.is_empty() && !k.is_empty() => (c, k),
        _ => generate()?,
    };
    let fp = pairing::fingerprint(&cert);
    match server_config(cert.clone(), key) {
        Ok(cfg) => Ok((cfg, fp)),
        Err(e) => {
            // 文件损坏：重新生成（已配对终端需重配，但总比服务起不来好）。
            log::warn!("[asr-gateway] 证书不可用（{e}），重新生成");
            let (cert, key) = generate()?;
            let fp = pairing::fingerprint(&cert);
            Ok((server_config(cert, key)?, fp))
        }
    }
}

fn generate() -> io::Result<(Vec<u8>, Vec<u8>)> {
    let ck = rcgen::generate_simple_self_signed(vec!["feiyin-asr".to_string()])
        .map_err(io::Error::other)?;
    let cert = ck.cert.der().to_vec();
    let key = ck.key_pair.serialize_der();
    write_atomic(&pairing::key_path(), &key)?;
    restrict(&pairing::key_path());
    write_atomic(&pairing::cert_path(), &cert)?;
    Ok((cert, key))
}

/// 私钥文件只给当前用户读（Unix 0600）。Windows 上数据目录本身在用户目录下，继承用户 ACL。
fn restrict(_path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(_path, std::fs::Permissions::from_mode(0o600));
    }
}

fn server_config(cert: Vec<u8>, key: Vec<u8>) -> io::Result<Arc<ServerConfig>> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    // 不设 ALPN：客户端带不带、带什么都能握手（WebSocket 走 HTTP/1.1 升级）。
    let cfg = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(io::Error::other)?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(cert)],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)),
        )
        .map_err(io::Error::other)?;
    Ok(Arc::new(cfg))
}
