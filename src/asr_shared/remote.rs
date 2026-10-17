//! ASR-SERVER-491 S8：本机当「终端」去连另一台电脑的识别服务（`asr-remote.json`，设计 §8.2 / §8.3 / §16.3）。
//!
//! - 配对信息存独立文件，不进 `config.toml`（§8.8）。key 明文要落盘：它是本机连接用的长期凭证（同保存的密码）。
//! - 设备代码：本机第一次配对时随机生成，之后固定（重新配对 / 换服务也沿用），服务端据此绑定 key。
//! - 内网 / 外网：默认「自动」= 两个地址都试、内网优先（§8.3）。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::pairing::{base32, normalize_key, random_bytes};
use super::settings::DEFAULT_PORT;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetMode {
    #[default]
    Auto,
    Lan,
    Wan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RemotePairing {
    /// 内网 `主机:端口`（空 = 没有）。
    pub lan: String,
    /// 外网 `主机:端口`（空 = 没有）。
    pub wan: String,
    /// key（已规整：大写、无空白 / 连字符）。
    pub key: String,
    /// 服务端证书指纹（SHA-256 十六进制小写）。每次连接都比对（证书钉扎）。
    pub fingerprint: String,
    pub mode: NetMode,
    /// 本机设备代码（首次生成后固定）。
    pub device_id: String,
    /// 本机名称（空 ⇒ 取计算机名）。
    pub device_name: String,
}

/// 连接码 / 手输内容不合格的原因（界面按码取 i18n 文案）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairError {
    /// 不是 `feiyin-asr://pair?` 开头，或版本不认识。
    BadCode,
    /// 内网、外网地址一个都没有 / 格式不对。
    BadAddress,
    MissingKey,
    /// 指纹不是 64 位十六进制。
    BadFingerprint,
}

impl PairError {
    pub fn code(self) -> &'static str {
        match self {
            Self::BadCode => "bad_code",
            Self::BadAddress => "bad_address",
            Self::MissingKey => "missing_key",
            Self::BadFingerprint => "bad_fingerprint",
        }
    }
}

/// 规整 `主机:端口`：去空白；没写端口补默认端口；IPv6 须带方括号。空串 ⇒ `Ok("")`。
pub fn normalize_endpoint(s: &str) -> Result<String, PairError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(String::new());
    }
    if s.contains(char::is_whitespace) || s.contains('/') {
        return Err(PairError::BadAddress);
    }
    let (host, port) = if let Some(rest) = s.strip_prefix('[') {
        let (h, tail) = rest.split_once(']').ok_or(PairError::BadAddress)?;
        let port = match tail.strip_prefix(':') {
            Some(p) => Some(p),
            None if tail.is_empty() => None,
            None => return Err(PairError::BadAddress),
        };
        (format!("[{h}]"), port)
    } else {
        match s.rsplit_once(':') {
            // 两个以上冒号 = 没加方括号的 IPv6，端口有歧义。
            Some((h, _)) if h.contains(':') => return Err(PairError::BadAddress),
            Some((h, p)) => (h.to_string(), Some(p)),
            None => (s.to_string(), None),
        }
    };
    if host.is_empty() || host == "[]" {
        return Err(PairError::BadAddress);
    }
    let port = match port {
        None => DEFAULT_PORT,
        Some(p) => match p.parse::<u16>() {
            Ok(n) if n > 0 => n,
            _ => return Err(PairError::BadAddress),
        },
    };
    Ok(format!("{host}:{port}"))
}

fn normalize_fingerprint(fp: &str) -> Result<String, PairError> {
    let fp: String = fp
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':')
        .flat_map(|c| c.to_lowercase())
        .collect();
    if fp.len() == 64 && fp.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(fp)
    } else {
        Err(PairError::BadFingerprint)
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        match (
            b[i],
            b.get(i + 1).copied().and_then(hex),
            b.get(i + 2).copied().and_then(hex),
        ) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (c, _, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl RemotePairing {
    pub fn path() -> PathBuf {
        super::data_dir().join("asr-remote.json")
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

    /// 配齐了才能连：有地址、有 key、有指纹。
    pub fn is_paired(&self) -> bool {
        (!self.lan.is_empty() || !self.wan.is_empty())
            && !self.key.is_empty()
            && !self.fingerprint.is_empty()
    }

    /// 解析连接码（二维码的文字版，§8.2）。只填地址 / key / 指纹，模式与本机身份由调用方保留。
    pub fn from_pair_uri(code: &str) -> Result<Self, PairError> {
        let query = code
            .trim()
            .strip_prefix("feiyin-asr://pair?")
            .ok_or(PairError::BadCode)?;
        let mut p = Self::default();
        let mut version_ok = false;
        for kv in query.split('&') {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            let v = percent_decode(v);
            match k {
                "v" => version_ok = v == "1",
                "lan" => p.lan = v,
                "wan" => p.wan = v,
                "key" => p.key = v,
                "fp" => p.fingerprint = v,
                _ => {} // 以后加的字段：老版本忽略
            }
        }
        if !version_ok {
            return Err(PairError::BadCode);
        }
        let p = p.validated()?;
        if p.fingerprint.is_empty() {
            return Err(PairError::BadFingerprint);
        }
        Ok(p)
    }

    /// 手输：内网 / 外网地址至少一个，key 必填，指纹可空（空 = 第一次连接时让用户确认，§8.5）。
    pub fn manual(lan: &str, wan: &str, key: &str, fingerprint: &str) -> Result<Self, PairError> {
        Self {
            lan: lan.to_string(),
            wan: wan.to_string(),
            key: key.to_string(),
            fingerprint: fingerprint.to_string(),
            ..Default::default()
        }
        .validated()
    }

    /// 指纹为空时保持为空（手输、待首次确认）；连接码必带指纹，由 `is_paired` 兜住。
    fn validated(mut self) -> Result<Self, PairError> {
        self.lan = normalize_endpoint(&self.lan)?;
        self.wan = normalize_endpoint(&self.wan)?;
        if self.lan.is_empty() && self.wan.is_empty() {
            return Err(PairError::BadAddress);
        }
        self.key = normalize_key(&self.key);
        if self.key.is_empty() {
            return Err(PairError::MissingKey);
        }
        if !self.fingerprint.trim().is_empty() {
            self.fingerprint = normalize_fingerprint(&self.fingerprint)?;
        }
        Ok(self)
    }

    /// 新配对信息落盘：沿用旧文件里的本机设备代码 / 名称 / 内外网模式；设备代码没有就生成。
    pub fn adopt_identity(mut self, old: &Self) -> std::io::Result<Self> {
        self.device_id = if old.device_id.is_empty() {
            base32(&random_bytes::<10>()?).to_lowercase()
        } else {
            old.device_id.clone()
        };
        self.device_name = old.device_name.clone();
        self.mode = old.mode;
        Ok(self)
    }

    /// 按模式排好的待试地址：`(主机:端口, "lan" | "wan")`。自动 = 内网在前。
    pub fn candidates(&self) -> Vec<(&str, &'static str)> {
        let lan = (!self.lan.is_empty()).then_some((self.lan.as_str(), "lan"));
        let wan = (!self.wan.is_empty()).then_some((self.wan.as_str(), "wan"));
        match self.mode {
            NetMode::Auto => [lan, wan].into_iter().flatten().collect(),
            NetMode::Lan => lan.into_iter().collect(),
            NetMode::Wan => wan.into_iter().collect(),
        }
    }

    /// 发给服务端的终端名称：用户填的，没填取计算机名。
    pub fn effective_device_name(&self) -> String {
        let n = self.device_name.trim();
        if n.is_empty() {
            computer_name()
        } else {
            n.to_string()
        }
    }
}

/// 计算机名（取不到 ⇒ "Feiyin"）。进程内只取一次（macOS 要起 `scutil` 子进程）。
pub fn computer_name() -> String {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(read_computer_name).clone()
}

fn read_computer_name() -> String {
    #[cfg(target_os = "windows")]
    let name = std::env::var("COMPUTERNAME").ok();
    #[cfg(target_os = "macos")]
    let name = std::process::Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let name = std::env::var("HOSTNAME").ok();
    name.map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Feiyin".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FP: &str = "ab01ab01ab01ab01ab01ab01ab01ab01ab01ab01ab01ab01ab01ab01ab01ab01";

    #[test]
    fn pair_uri_round_trip_with_server_format() {
        let uri = super::super::pairing::pair_uri(
            "192.168.1.5:7860",
            Some("asr.example.com:17860"),
            "ABCD-efgh",
            FP,
        );
        let p = RemotePairing::from_pair_uri(&format!("  {uri}\n")).unwrap();
        assert_eq!(p.lan, "192.168.1.5:7860");
        assert_eq!(p.wan, "asr.example.com:17860");
        assert_eq!(p.key, "ABCDEFGH");
        assert_eq!(p.fingerprint, FP);
        assert!(p.is_paired());
    }

    #[test]
    fn pair_uri_rejects_bad_input() {
        assert_eq!(
            RemotePairing::from_pair_uri("http://x?v=1"),
            Err(PairError::BadCode)
        );
        assert_eq!(
            RemotePairing::from_pair_uri("feiyin-asr://pair?v=2&lan=a:1&key=K&fp=00"),
            Err(PairError::BadCode),
            "不认识的版本"
        );
        assert_eq!(
            RemotePairing::from_pair_uri(&format!("feiyin-asr://pair?v=1&key=K&fp={FP}")),
            Err(PairError::BadAddress)
        );
        assert_eq!(
            RemotePairing::from_pair_uri(&format!("feiyin-asr://pair?v=1&lan=a&fp={FP}")),
            Err(PairError::MissingKey)
        );
        assert_eq!(
            RemotePairing::from_pair_uri("feiyin-asr://pair?v=1&lan=a&key=K&fp=12"),
            Err(PairError::BadFingerprint)
        );
    }

    #[test]
    fn endpoint_normalization() {
        assert_eq!(normalize_endpoint(" 10.0.0.2 ").unwrap(), "10.0.0.2:7860");
        assert_eq!(normalize_endpoint("a.com:443").unwrap(), "a.com:443");
        assert_eq!(normalize_endpoint("[fe80::1]").unwrap(), "[fe80::1]:7860");
        assert_eq!(normalize_endpoint("[::1]:9").unwrap(), "[::1]:9");
        assert_eq!(normalize_endpoint("").unwrap(), "");
        for bad in [
            "fe80::1", "a.com:0", "a.com:x", "a b", "a.com/x", "[::1]x", ":80",
        ] {
            assert_eq!(normalize_endpoint(bad), Err(PairError::BadAddress), "{bad}");
        }
    }

    #[test]
    fn manual_entry_fingerprint_optional() {
        let p = RemotePairing::manual("", "a.com", "k-1", "").unwrap();
        assert_eq!(p.wan, "a.com:7860");
        assert_eq!(p.key, "K1");
        assert!(p.fingerprint.is_empty());
        assert!(!p.is_paired(), "没确认指纹不能连");
        let p = RemotePairing::manual("10.0.0.2", "", "k", &FP.to_uppercase()).unwrap();
        assert_eq!(p.fingerprint, FP);
        assert_eq!(
            RemotePairing::manual("", "", "k", ""),
            Err(PairError::BadAddress)
        );
    }

    #[test]
    fn identity_kept_across_repairing_and_candidates_order() {
        let fresh = RemotePairing::manual("10.0.0.2", "a.com", "k", FP).unwrap();
        let first = fresh
            .clone()
            .adopt_identity(&RemotePairing::default())
            .unwrap();
        assert_eq!(first.device_id.len(), 16);
        let mut old = first.clone();
        old.mode = NetMode::Wan;
        old.device_name = "书房".into();
        let again = fresh.adopt_identity(&old).unwrap();
        assert_eq!(again.device_id, first.device_id, "重新配对沿用设备代码");
        assert_eq!(again.device_name, "书房");
        assert_eq!(again.candidates(), vec![("a.com:7860", "wan")]);
        let mut auto = again;
        auto.mode = NetMode::Auto;
        assert_eq!(
            auto.candidates(),
            vec![("10.0.0.2:7860", "lan"), ("a.com:7860", "wan")]
        );
    }
}
