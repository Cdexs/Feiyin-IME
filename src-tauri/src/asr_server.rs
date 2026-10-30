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
/// FLASH-P3-C：本机没装 flashasr（没有本机配对文件）。
const ERR_NO_LOCAL: &str = "no_local";

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
    /// FLASH-P3-C：连的是本机 flashasr（语音页据此显示「本机」还是哪台电脑）。
    pub local: bool,
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
        local: p.local,
    }
}

/// 新配对落盘：沿用本机设备代码 / 名称 / 内外网模式。
/// FLASH-P3-C：之前连的是本机 flashasr（模式固定「只用内网」）⇒ 改连别的电脑时模式回到「自动」，
/// 否则只有外网地址的连接码会因「只用内网」一个地址都不试。
fn save_pairing(p: RemotePairing) -> Result<RemoteView, String> {
    let old = RemotePairing::load();
    let mut p = p.adopt_identity(&old).map_err(|_| ERR_IO.to_string())?;
    if old.local {
        p.mode = NetMode::Auto;
    }
    p.save().map_err(|_| ERR_IO.to_string())?;
    Ok(remote_view(&p))
}

/// FLASH-P3-C：取「录音时实际会用」的配对 —— 同主程序 `load_for_use`（已确认的本机 ⇒ 按本机配对文件刷新；
/// 没配过不自动连本机），让语音页显示与测试连接和录音时一致。
#[tauri::command]
pub fn asr_remote_get() -> RemoteView {
    remote_view(&RemotePairing::load_for_use())
}

/// FLASH-P3-C：本机是否装了 flashasr（读本机配对文件，协议 §1.5）。
#[tauri::command]
pub fn asr_local_detect() -> bool {
    asr_shared::remote::local_flashasr_pairing().is_some()
}

/// FLASH-P3-C：用户**确认**连本机 flashasr（向导「本机识别服务 → 使用」/ 语音页「改用本机」）才写本机配对：
/// 读本机配对文件 → 沿用本机身份 → 记为本机、只走内网 → 落盘（`RemotePairing::use_local`）。
/// 档位 `asr_model = feiyin_server` 由界面走现有保存配置流程写。本机没装 ⇒ `no_local`。
/// Gavin 10-30：「如果发现本机有 Flash ASR 服务，不要自动连接，要让用户确认再连接」。
#[tauri::command]
pub fn asr_local_use() -> Result<RemoteView, String> {
    match RemotePairing::use_local() {
        Ok(Some(p)) => Ok(remote_view(&p)),
        Ok(None) => Err(ERR_NO_LOCAL.to_string()),
        Err(_) => Err(ERR_IO.to_string()),
    }
}

/// FLASH-P3-C：界面判「连接能不能用」所需的现状（判据与主程序启动弹向导同一个纯函数）。
#[derive(Serialize)]
pub struct ConnectionState {
    /// 连接可用（直连阿里有 Key / 连 flashasr 已配对；本机有 flashasr 但用户没确认不算）。
    pub ready: bool,
    /// 本机装了 flashasr。
    pub local_available: bool,
}

#[tauri::command]
pub fn asr_connection_state(asr_model: String, api_key: String) -> ConnectionState {
    ConnectionState {
        ready: asr_shared::remote::connection_ready_now(&asr_model, &api_key),
        local_available: asr_local_detect(),
    }
}

/// FLASH-P3-C：设置界面是否由主程序以「直接打开首次连接向导」方式拉起（启动参数 `--wizard`，
/// 与主程序 `SETTINGS_WIZARD_ARG` 同一字面量）。
#[tauri::command]
pub fn asr_launch_wizard() -> bool {
    std::env::args().any(|a| a == "--wizard")
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
    // FLASH-P3-C：与录音同口径取配对（已确认的本机 ⇒ 按本机配对文件刷新；不自动连本机）。
    let p = RemotePairing::load_for_use();
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

/// FLASH-P3-C：flashasr 服务状态（输出优化页显示「服务端大模型 已配置 / 未配置」）。
#[derive(Serialize)]
pub struct ServerStatusView {
    /// `lan` / `wan`
    pub net: String,
    #[serde(flatten)]
    pub status: tls_pin::ServerStatus,
}

/// FLASH-P3-C：调 flashasr `GET /v1/status`（同端口同 TLS，先钉扎指纹再发 key，协议 §5.5）。
/// 地址按当前模式逐个试，第一个答上来的即结果。失败码同 [`asr_remote_test`]：`not_paired` / `unreachable` /
/// `fingerprint_mismatch` / 服务端错误码（401 ⇒ `InvalidKey`）。
#[tauri::command]
pub async fn asr_remote_status() -> Result<ServerStatusView, String> {
    let p = RemotePairing::load_for_use();
    if !p.is_paired() {
        return Err("not_paired".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        for (ep, net) in p.candidates() {
            match tls_pin::fetch_status(ep, &p.fingerprint, &p.key, PROBE_TIMEOUT) {
                Ok(tls_pin::StatusCheck::Ok(status)) => {
                    return Ok(ServerStatusView {
                        net: net.to_string(),
                        status,
                    })
                }
                Ok(tls_pin::StatusCheck::FingerprintMismatch) => {
                    return Err(ERR_FINGERPRINT.to_string())
                }
                Ok(tls_pin::StatusCheck::Rejected(_, code)) if !code.is_empty() => {
                    return Err(code)
                }
                Ok(tls_pin::StatusCheck::Rejected(..)) | Err(_) => {}
            }
        }
        Err(ERR_UNREACHABLE.to_string())
    })
    .await
    .map_err(|_| ERR_IO.to_string())?
}
