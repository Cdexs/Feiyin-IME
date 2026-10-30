//! ASR-SERVER-491：主程序与设置界面共用的「连识别服务」数据（平台中立，只依赖 std + serde + ring + rustls）。
//!
//! 设置界面用 `#[path]` 引入同一份源码。FLASH-P3-A：本机识别服务（网关 / 内核侧）已删除，
//! 服务端专用的 `control`（网关控制通道）/ `settings`（服务设置）与终端声纹清理随之删除。数据放可执行文件同目录
//!（与词库、使用统计同规则），**不进 `config.toml`**（设计 §8.8：用户升级可能整份替换配置）。

pub mod pairing;
pub mod remote;
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
