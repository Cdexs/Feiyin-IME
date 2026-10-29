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

/// FLASH-P0-511：某终端的声纹存档 `<数据目录>/voiceprints/<终端编号>.bin`（本机主人仍是 `voiceprint.bin`）。
/// 终端编号是配对时生成的小写 base32；含其它字符（不该出现）⇒ `None`，调用方按「无声纹」处理。
pub fn terminal_voiceprint_path(terminal_id: &str) -> Option<PathBuf> {
    let id = terminal_id.trim();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(data_dir().join("voiceprints").join(format!("{id}.bin")))
}

/// FLASH-P0-511：吊销终端时删其声纹 —— 声纹是生物特征数据，不得在终端吊销后保留。
/// 两个吊销入口（网关在跑 ⇒ 网关控制通道；网关没跑 ⇒ 设置界面直接改配对文件）都调它。没有文件 ⇒ `Ok`。
pub fn purge_terminal_voiceprint(terminal_id: &str) -> std::io::Result<()> {
    let Some(p) = terminal_voiceprint_path(terminal_id) else {
        return Ok(());
    };
    match std::fs::remove_file(&p) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
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

#[cfg(test)]
mod flash_p0_511_tests {
    use super::*;

    #[test]
    fn terminal_voiceprint_path_validates_id() {
        let p = terminal_voiceprint_path("abc234").expect("合规编号");
        assert!(p.ends_with(std::path::Path::new("voiceprints").join("abc234.bin")));
        for bad in ["", " ", "..", "a/b", r"a\b", "a.b", "a b"] {
            assert!(terminal_voiceprint_path(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn purge_missing_is_ok() {
        assert!(purge_terminal_voiceprint("flashp0none").is_ok());
        assert!(
            purge_terminal_voiceprint("../evil").is_ok(),
            "不合规编号 ⇒ 什么都不删"
        );
    }
}
