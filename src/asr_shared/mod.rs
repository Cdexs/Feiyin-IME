//! ASR-SERVER-491：内核、网关、设置界面三方共用的识别服务数据（平台中立，只依赖 std + serde + ring + rustls）。
//!
//! 各方用 `#[path]` 引入同一份源码（网关、设置界面是独立程序）。数据放可执行文件同目录
//!（与词库、使用统计同规则），**不进 `config.toml`**（设计 §8.8：用户升级可能整份替换配置）。

pub mod control;
pub mod pairing;
pub mod remote;
pub mod settings;
pub mod tls_pin;

use std::path::PathBuf;

/// 数据目录 = 可执行文件所在目录（取不到 ⇒ 当前目录）。
pub fn data_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 当前 Unix 秒。
pub fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 原子写：先写同目录临时文件再改名（写到一半断电不会留下半截文件）。
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}
