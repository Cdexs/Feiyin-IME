//! ASR-SERVER-491：识别服务设置（`asr-service.json`）。
//!
//! - `enabled` = 唯一的总开关（DEC-031 / 设计 §8.1）：设置页「启动 / 停止」与托盘菜单写它，内核据此拉起 / 结束网关。
//! - 暂停**不**存这里（设计 §16.2：暂停只是临时的，重启输入法恢复为运行中）。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 默认端口（TCP；`wss://` 与二期 LiveKit 信令共用入口）。
pub const DEFAULT_PORT: u16 = 7860;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ServiceSettings {
    /// 总开关。
    pub enabled: bool,
    /// 本机监听端口（内网直接用；外网经路由器端口映射到它）。
    pub port: u16,
    /// 外网 IP 或域名（选填；空 = 只能内网接入）。
    pub wan_host: String,
    /// 外网端口（0 = 与 `port` 相同）。
    pub wan_port: u16,
}

impl Default for ServiceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            wan_host: String::new(),
            wan_port: 0,
        }
    }
}

impl ServiceSettings {
    pub fn path() -> PathBuf {
        super::data_dir().join("asr-service.json")
    }

    /// 读设置；文件不存在 / 损坏 ⇒ 默认值（服务关）。
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

    /// 外网 `主机:端口`（未填 ⇒ `None`）。
    pub fn wan_endpoint(&self) -> Option<String> {
        let host = self.wan_host.trim();
        if host.is_empty() {
            return None;
        }
        let port = if self.wan_port == 0 {
            self.port
        } else {
            self.wan_port
        };
        Some(if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wan_endpoint_rules() {
        let mut s = ServiceSettings::default();
        assert_eq!(s.wan_endpoint(), None, "未填 ⇒ 只能内网");
        s.wan_host = "asr.example.com".into();
        assert_eq!(s.wan_endpoint().as_deref(), Some("asr.example.com:7860"));
        s.wan_port = 443;
        assert_eq!(s.wan_endpoint().as_deref(), Some("asr.example.com:443"));
        s.wan_host = "2001:db8::1".into();
        assert_eq!(
            s.wan_endpoint().as_deref(),
            Some("[2001:db8::1]:443"),
            "IPv6 加方括号"
        );
    }

    #[test]
    fn missing_fields_default() {
        let s: ServiceSettings = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(s.enabled);
        assert_eq!(s.port, DEFAULT_PORT);
    }
}
