//! 设置界面「连识别服务」配对后端（语音页「连识别服务」档位，设计 §8.2 / §8.3 / §8.5）。
//!
//! FLASH-P3-A（DEC-114）：本机当识别服务的「识别服务」页（总开关 / 外网地址 / 暂停 / 添加终端 / 撤销 /
//! 会话记录）随本机识别服务删除 —— 识别服务已是独立应用 flashasr，有自己的管理界面。
//! 只保留本机作为「终端」连识别服务的 `asr_remote_*` 命令。

use serde::Serialize;

#[allow(dead_code)]
#[path = "../../src/asr_shared/mod.rs"]
mod asr_shared;

const ERR_IO: &str = "io";

// ===========================================================================
// S8：本机当「终端」连另一台电脑的识别服务（语音页「自家识别服务」档位，设计 §8.2 / §8.3 / §8.5）
// ===========================================================================

use asr_shared::remote::{NetMode, RemotePairing};
use asr_shared::tls_pin::{self, AccessCheck};
use std::time::{Duration, Instant};

/// 设置界面的探测 / 测试超时（比识别建连的 5 秒短：用户在点按钮等结果）。
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const ERR_UNREACHABLE: &str = "unreachable";
const ERR_FINGERPRINT: &str = "fingerprint_mismatch";

/// 语音页看到的配对状态（key 不回传界面）。
#[derive(Serialize)]
pub struct RemoteView {
    pub paired: bool,
    pub lan: String,
    pub wan: String,
    pub fingerprint: String,
    pub mode: NetMode,
    pub device_name: String,
    /// 名称留空时实际发送的计算机名（输入框占位提示用）。
    pub computer_name: String,
}

fn remote_view(p: &RemotePairing) -> RemoteView {
    RemoteView {
        paired: p.is_paired(),
        lan: p.lan.clone(),
        wan: p.wan.clone(),
        fingerprint: p.fingerprint.clone(),
        mode: p.mode,
        device_name: p.device_name.clone(),
        computer_name: asr_shared::remote::computer_name(),
    }
}

/// 新配对落盘：沿用本机设备代码 / 名称 / 内外网模式。
fn save_pairing(p: RemotePairing) -> Result<RemoteView, String> {
    let p = p
        .adopt_identity(&RemotePairing::load())
        .map_err(|_| ERR_IO.to_string())?;
    p.save().map_err(|_| ERR_IO.to_string())?;
    Ok(remote_view(&p))
}

#[tauri::command]
pub fn asr_remote_get() -> RemoteView {
    remote_view(&RemotePairing::load())
}

/// 粘贴连接码配对（连接码自带指纹，不用再确认）。
#[tauri::command]
pub fn asr_remote_pair_code(code: String) -> Result<RemoteView, String> {
    let p = RemotePairing::from_pair_uri(&code).map_err(|e| e.code().to_string())?;
    save_pairing(p)
}

#[derive(Serialize)]
pub struct Probed {
    pub fingerprint: String,
    /// 取到指纹的那个地址（`主机:端口`）。
    pub endpoint: String,
}

/// 手输第一步：校验输入 + 连上去取对方证书指纹，交给用户确认（§8.5，同 SSH 首次连接）。
#[tauri::command]
pub async fn asr_remote_probe(lan: String, wan: String, key: String) -> Result<Probed, String> {
    let p = RemotePairing::manual(&lan, &wan, &key, "").map_err(|e| e.code().to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        // 内网在前：用户在哪边能连上就取哪边（同一台服务，指纹相同）。
        let mut auto = p;
        auto.mode = NetMode::Auto;
        auto.candidates()
            .into_iter()
            .find_map(|(ep, _)| {
                tls_pin::probe_fingerprint(ep, PROBE_TIMEOUT)
                    .ok()
                    .map(|fingerprint| Probed {
                        fingerprint,
                        endpoint: ep.to_string(),
                    })
            })
            .ok_or_else(|| ERR_UNREACHABLE.to_string())
    })
    .await
    .map_err(|_| ERR_IO.to_string())?
}

/// 手输第二步：用户确认指纹后保存。
#[tauri::command]
pub fn asr_remote_pair_manual(
    lan: String,
    wan: String,
    key: String,
    fingerprint: String,
) -> Result<RemoteView, String> {
    if fingerprint.trim().is_empty() {
        return Err("bad_fingerprint".to_string());
    }
    let p =
        RemotePairing::manual(&lan, &wan, &key, &fingerprint).map_err(|e| e.code().to_string())?;
    save_pairing(p)
}

/// 改内外网模式 / 本机名称（`mode`: `auto` / `lan` / `wan`）。
#[tauri::command]
pub fn asr_remote_update(mode: NetMode, device_name: String) -> Result<RemoteView, String> {
    let mut p = RemotePairing::load();
    p.mode = mode;
    p.device_name = device_name.trim().to_string();
    p.save().map_err(|_| ERR_IO.to_string())?;
    Ok(remote_view(&p))
}

#[derive(Serialize)]
pub struct Tested {
    /// `lan` / `wan`
    pub net: String,
    pub ms: u64,
}

/// 测试连接：按当前模式逐个地址试「证书 + key」，第一个通过的即结果。
/// 失败码：`unreachable` / `fingerprint_mismatch` / 网关错误码（`InvalidKey` / `KeyRevoked` / `KeyExpired` / `ServicePaused`）。
#[tauri::command]
pub async fn asr_remote_test() -> Result<Tested, String> {
    let p = RemotePairing::load();
    if !p.is_paired() {
        return Err("not_paired".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        for (ep, net) in p.candidates() {
            let t0 = Instant::now();
            match tls_pin::check_access(ep, &p.fingerprint, &p.key, PROBE_TIMEOUT) {
                Ok(AccessCheck::Ok) => {
                    return Ok(Tested {
                        net: net.to_string(),
                        ms: t0.elapsed().as_millis() as u64,
                    })
                }
                // 证书 / key 问题换个地址也一样：直接报。
                Ok(AccessCheck::FingerprintMismatch) => return Err(ERR_FINGERPRINT.to_string()),
                Ok(AccessCheck::Rejected(_, code)) if !code.is_empty() => return Err(code),
                Ok(AccessCheck::Rejected(..)) | Err(_) => {}
            }
        }
        Err(ERR_UNREACHABLE.to_string())
    })
    .await
    .map_err(|_| ERR_IO.to_string())?
}
