//! ASR-SERVER-491：终端配对（`asr-terminals.json`，设计 §8.2 / §8.4 / §16.3）。
//!
//! - 每台终端一把 key；文件里只存 key 的 SHA-256，明文只在「添加终端」那一刻给出（二维码 / 连接码）。
//! - **首次使用即绑定**：key 生成后 10 分钟内须被某台终端用上，用上即与该终端的设备代码绑定；
//!   之后设备代码不一致 ⇒ 拒绝（key 被抄走、换台设备也用不了）。
//! - 撤销：立即作废（网关同时断开该终端）；记录保留，列表里显示「已撤销」，统计不丢。

use ring::digest;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 新 key 等待首次绑定的时限（秒）。
pub const BIND_WINDOW_SECS: i64 = 600;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Terminal {
    /// 内部编号（统计库的 `terminal_id`）。
    pub id: String,
    /// key 的 SHA-256（十六进制）。
    pub key_hash: String,
    /// 绑定的设备代码（空 = 尚未绑定）。
    pub device_id: String,
    /// 终端名称（对方传来；每次连接按最新值更新）。
    pub device_name: String,
    pub created_ts: i64,
    pub last_login_ts: i64,
    pub last_logout_ts: i64,
    /// 最近一次接入方式：`lan` / `wan`。
    pub last_net: String,
    pub revoked: bool,
}

impl Terminal {
    pub fn bound(&self) -> bool {
        !self.device_id.is_empty()
    }
}

/// 鉴权失败原因（网关据此回错误码；第三方客户端照文档处理）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    /// key 不存在 / 格式不对。
    InvalidKey,
    /// 未绑定的 key 已过 10 分钟。
    Expired,
    /// 已撤销。
    Revoked,
    /// 设备代码与绑定的不一致。
    DeviceMismatch,
    /// 缺设备代码 / 名称。
    MissingDevice,
}

impl AuthError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidKey => "InvalidKey",
            Self::Expired => "KeyExpired",
            Self::Revoked => "KeyRevoked",
            Self::DeviceMismatch => "DeviceMismatch",
            Self::MissingDevice => "MissingDeviceInfo",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalStore {
    pub terminals: Vec<Terminal>,
}

fn sha256_hex(s: &str) -> String {
    sha256_hex_bytes(s.as_bytes())
}

fn sha256_hex_bytes(b: &[u8]) -> String {
    digest::digest(&digest::SHA256, b)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// RFC 4648 base32（大写、无填充）。16 字节 ⇒ 26 字符：手输也不至于太长。
fn base32(bytes: &[u8]) -> String {
    const A: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in bytes {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(A[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(A[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

fn random_bytes<const N: usize>() -> std::io::Result<[u8; N]> {
    let mut b = [0u8; N];
    SystemRandom::new()
        .fill(&mut b)
        .map_err(|_| std::io::Error::other("system random unavailable"))?;
    Ok(b)
}

/// 规整用户输入的 key：去空白与连字符、转大写（手输容错）。
pub fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(|c| c.to_uppercase())
        .collect()
}

impl TerminalStore {
    pub fn path() -> PathBuf {
        super::data_dir().join("asr-terminals.json")
    }

    pub fn load() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        super::write_atomic(&Self::path(), &bytes)
    }

    /// 添加终端：生成新 key，返回 `(终端编号, key 明文)`。明文不落盘。
    pub fn create(&mut self, now: i64) -> std::io::Result<(String, String)> {
        let key = base32(&random_bytes::<16>()?);
        let id = base32(&random_bytes::<5>()?).to_lowercase();
        self.terminals.push(Terminal {
            id: id.clone(),
            key_hash: sha256_hex(&key),
            created_ts: now,
            ..Default::default()
        });
        Ok((id, key))
    }

    /// 只认 key（WebSocket 握手时用，此时还不知道设备代码）：存在、未撤销、未过绑定时限。
    pub fn check_key(&self, key: &str, now: i64) -> Result<&Terminal, AuthError> {
        let h = sha256_hex(&normalize_key(key));
        let t = self
            .terminals
            .iter()
            .find(|t| t.key_hash == h)
            .ok_or(AuthError::InvalidKey)?;
        if t.revoked {
            return Err(AuthError::Revoked);
        }
        if !t.bound() && now - t.created_ts > BIND_WINDOW_SECS {
            return Err(AuthError::Expired);
        }
        Ok(t)
    }

    /// 识别请求时：校验 key + 设备代码；未绑定 ⇒ 绑定；名称变了 ⇒ 更新。返回是否改了数据（调用方据此保存）。
    pub fn authorize(
        &mut self,
        key: &str,
        device_id: &str,
        device_name: &str,
        now: i64,
    ) -> Result<(String, bool), AuthError> {
        let (device_id, device_name) = (device_id.trim(), device_name.trim());
        if device_id.is_empty() || device_name.is_empty() {
            return Err(AuthError::MissingDevice);
        }
        let id = self.check_key(key, now)?.id.clone();
        let t = self
            .terminals
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(AuthError::InvalidKey)?;
        let mut changed = false;
        if !t.bound() {
            t.device_id = device_id.to_string();
            changed = true;
        } else if t.device_id != device_id {
            return Err(AuthError::DeviceMismatch);
        }
        if t.device_name != device_name {
            t.device_name = device_name.to_string();
            changed = true;
        }
        Ok((id, changed))
    }

    pub fn revoke(&mut self, id: &str) -> bool {
        match self.terminals.iter_mut().find(|t| t.id == id) {
            Some(t) if !t.revoked => {
                t.revoked = true;
                true
            }
            _ => false,
        }
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Terminal> {
        self.terminals.iter_mut().find(|t| t.id == id)
    }

    /// 清掉从未绑定且已过时限的 key（添加了终端却没去连）。
    pub fn prune_expired(&mut self, now: i64) -> bool {
        let before = self.terminals.len();
        self.terminals
            .retain(|t| t.bound() || now - t.created_ts <= BIND_WINDOW_SECS);
        self.terminals.len() != before
    }
}

/// 网关自签证书（DER）。网关首次启动生成，设置界面读它算指纹放进二维码。
pub fn cert_path() -> PathBuf {
    super::data_dir().join("asr-cert.der")
}

/// 网关证书私钥（PKCS#8 DER；只有网关读）。
pub fn key_path() -> PathBuf {
    super::data_dir().join("asr-key.der")
}

/// 证书指纹 = 证书 DER 的 SHA-256（十六进制小写）。终端配对时记下，之后每次连接都比对（证书钉扎）。
pub fn fingerprint(cert_der: &[u8]) -> String {
    sha256_hex_bytes(cert_der)
}

/// 读本机证书指纹（网关还没生成过证书 ⇒ `None`）。
pub fn cert_fingerprint() -> Option<String> {
    std::fs::read(cert_path()).ok().map(|d| fingerprint(&d))
}

/// 二维码 / 连接码内容（设计 §8.2）。`lan` / `wan` 为 `主机:端口`；`fp` = 证书 SHA-256 指纹（十六进制）。
pub fn pair_uri(lan: &str, wan: Option<&str>, key: &str, fp: &str) -> String {
    let mut s = format!("feiyin-asr://pair?v=1&lan={lan}");
    if let Some(w) = wan {
        s.push_str("&wan=");
        s.push_str(w);
    }
    s.push_str("&key=");
    s.push_str(key);
    s.push_str("&fp=");
    s.push_str(fp);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_known_vectors() {
        assert_eq!(base32(b""), "");
        assert_eq!(base32(b"f"), "MY");
        assert_eq!(base32(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32(&[0u8; 16]).len(), 26);
    }

    #[test]
    fn bind_once_then_device_locked() {
        let mut st = TerminalStore::default();
        let (id, key) = st.create(1000).unwrap();
        assert!(st.terminals[0].key_hash != key, "只存哈希");
        // 手输容错：小写 + 空格。
        let typed = key.to_lowercase();
        let (got, changed) = st.authorize(&typed, "dev-A", "手机", 1100).unwrap();
        assert_eq!(got, id);
        assert!(changed, "首次绑定要保存");
        assert_eq!(
            st.authorize(&key, "dev-B", "手机", 1200),
            Err(AuthError::DeviceMismatch),
            "换设备拒绝"
        );
        let (_, changed) = st.authorize(&key, "dev-A", "新名字", 99_999).unwrap();
        assert!(changed, "已绑定不受 10 分钟限制；改名要保存");
        assert_eq!(st.terminals[0].device_name, "新名字");
    }

    #[test]
    fn unbound_key_expires_and_revoke() {
        let mut st = TerminalStore::default();
        let (id, key) = st.create(0).unwrap();
        assert_eq!(
            st.authorize(&key, "d", "n", BIND_WINDOW_SECS + 1),
            Err(AuthError::Expired)
        );
        assert!(st.prune_expired(BIND_WINDOW_SECS + 1));
        assert!(st.terminals.is_empty());
        let (id2, key2) = st.create(0).unwrap();
        assert_ne!(id, id2);
        assert!(st.revoke(&id2));
        assert_eq!(st.check_key(&key2, 1).err(), Some(AuthError::Revoked));
        assert_eq!(
            st.authorize("NOPE", "d", "n", 1),
            Err(AuthError::InvalidKey)
        );
        assert_eq!(
            st.authorize(&key2, "", "n", 1),
            Err(AuthError::MissingDevice)
        );
    }

    #[test]
    fn pair_uri_shape() {
        assert_eq!(
            pair_uri("192.168.1.5:7860", Some("a.com:7860"), "KEY", "ab"),
            "feiyin-asr://pair?v=1&lan=192.168.1.5:7860&wan=a.com:7860&key=KEY&fp=ab"
        );
        assert_eq!(
            pair_uri("10.0.0.2:7860", None, "K", "f"),
            "feiyin-asr://pair?v=1&lan=10.0.0.2:7860&key=K&fp=f"
        );
    }
}
