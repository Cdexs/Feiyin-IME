//! ASR-SERVER-491 S4b：网关本机控制通道（设置界面 ↔ 网关，设计 §16.5）。
//!
//! - 网关启动时把控制端点写进 `asr-gateway.json`（名字带进程号，别人抢注不了）；设置界面读它来连。
//! - 一次连接一问一答：请求一行 JSON、回复一行 JSON。仅当前用户可连（Windows 管道 DACL / Unix 0600）。
//! - 网关运行期间，终端表**只由网关写**（添加 / 撤销都走这里）；网关没在运行时设置界面才直接写文件。
//! - 连不上 / `asr-gateway.json` 指向已退出的进程 ⇒ 视为「已停止」。

use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::time::Duration;

/// 请求 / 回复单行上限。
pub const MAX_LINE: u64 = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    /// 运行状态 + 在线终端。
    Status,
    /// 暂停：进行中的识别做完，新的识别请求回「服务已暂停」，不接受新终端接入（§16.2）。
    Pause,
    Resume,
    /// 添加终端：生成 key，回二维码内容。
    AddTerminal,
    /// 撤销配对：key 作废，该终端在线连接立即断开。
    Revoke {
        terminal_id: String,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Online {
    pub terminal_id: String,
    /// `lan` / `wan`。
    pub net: String,
    pub login_ts: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Status {
    pub paused: bool,
    pub port: u16,
    pub fingerprint: String,
    /// 在线终端（同一终端多条连接只算一条，取最早登录）。
    pub online: Vec<Online>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Added {
    pub terminal_id: String,
    /// key 明文（只在此刻出现一次）。
    pub key: String,
    /// 内网 `主机:端口`（取不到本机内网地址 ⇒ 空）。
    pub lan: String,
    /// 外网 `主机:端口`（没填 ⇒ 空）。
    pub wan: String,
    /// 二维码 / 连接码内容。
    pub uri: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Response {
    /// 空 = 成功。
    pub error: String,
    pub status: Option<Status>,
    pub added: Option<Added>,
}

impl Response {
    pub fn err(e: impl Into<String>) -> Self {
        Self {
            error: e.into(),
            ..Default::default()
        }
    }
}

/// `asr-gateway.json`：网关运行信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct GatewayInfo {
    pub pid: u32,
    /// 控制通道端点（Windows 管道名 / Unix socket 路径）。
    pub control: String,
}

impl GatewayInfo {
    pub fn path() -> PathBuf {
        super::data_dir().join("asr-gateway.json")
    }

    pub fn load() -> Option<Self> {
        std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
    }

    pub fn save(&self) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        super::write_atomic(&Self::path(), &bytes)
    }
}

/// 读一行 JSON（超长 / 非法 ⇒ `InvalidData`）。
pub fn read_json<T: for<'de> Deserialize<'de>>(r: impl Read) -> io::Result<T> {
    let mut line = String::new();
    BufReader::new(r.take(MAX_LINE)).read_line(&mut line)?;
    serde_json::from_str(line.trim_end()).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn write_json<T: Serialize>(mut w: impl Write, v: &T) -> io::Result<()> {
    let mut b = serde_json::to_vec(v).map_err(io::Error::other)?;
    b.push(b'\n');
    w.write_all(&b)?;
    w.flush()
}

/// 设置界面调用：连网关发一条命令。网关没在运行 ⇒ `NotFound` / 连接错误。
pub fn call(req: &Request) -> io::Result<Response> {
    let info = GatewayInfo::load().ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    let mut s = connect(&info.control)?;
    write_json(&mut s, req)?;
    read_json(&mut s)
}

/// Windows：同一时刻只有一个管道实例，忙（ERROR_PIPE_BUSY = 231）时短暂重试。
#[cfg(target_os = "windows")]
fn connect(endpoint: &str) -> io::Result<std::fs::File> {
    let mut last = io::Error::from(io::ErrorKind::NotFound);
    for _ in 0..20 {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(endpoint)
        {
            Ok(f) => return Ok(f),
            Err(e) if e.raw_os_error() == Some(231) => {
                last = e;
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(e),
        }
    }
    Err(last)
}

#[cfg(unix)]
fn connect(endpoint: &str) -> io::Result<std::os::unix::net::UnixStream> {
    let s = std::os::unix::net::UnixStream::connect(endpoint)?;
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_wire_format() {
        assert_eq!(
            serde_json::to_string(&Request::Revoke {
                terminal_id: "ab".into()
            })
            .unwrap(),
            r#"{"cmd":"revoke","terminal_id":"ab"}"#
        );
        let r: Request = serde_json::from_str(r#"{"cmd":"add_terminal"}"#).unwrap();
        assert_eq!(r, Request::AddTerminal);
    }

    #[test]
    fn line_roundtrip_and_limit() {
        let mut buf = Vec::new();
        write_json(&mut buf, &Request::Status).unwrap();
        assert_eq!(
            read_json::<Request>(buf.as_slice()).unwrap(),
            Request::Status
        );
        let huge = vec![b'x'; MAX_LINE as usize + 10];
        assert!(
            read_json::<Request>(huge.as_slice()).is_err(),
            "超长按非法处理"
        );
    }
}
