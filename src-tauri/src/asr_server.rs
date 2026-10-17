//! ASR-SERVER-491 S6：设置界面「识别服务」页后端（设计 §16）。
//!
//! - 启动 / 停止 = 写 `asr-service.json` 的总开关（DEC-031 单开关；内核看护线程据此拉起 / 结束网关，托盘同此）。
//! - 暂停 / 继续 / 添加终端 / 撤销：网关在运行 ⇒ 经网关本机控制通道（运行期间终端表只由网关写）；
//!   网关没在运行 ⇒ 撤销直接改文件（此时无并发写者），添加终端要求服务在运行（二维码要带证书指纹）。
//! - 统计数字：直接只读 `usage-stats.sqlite`（同「我的」页）。

use serde::Serialize;

#[allow(dead_code)]
#[path = "../../src/asr_shared/mod.rs"]
mod asr_shared;

use asr_shared::control::{self, Added, Request, Status};
use asr_shared::pairing::{self, TerminalStore};
use asr_shared::settings::ServiceSettings;

use crate::usage::usage_stats_core as stats;

/// 错误码（界面按码取 i18n 文案，不把内部错误原文上屏）。
const ERR_NOT_RUNNING: &str = "not_running";
const ERR_IO: &str = "io";
const ERR_NOT_FOUND: &str = "not_found";

#[derive(Serialize)]
pub struct TerminalRow {
    pub id: String,
    pub device_name: String,
    pub device_id: String,
    pub bound: bool,
    pub revoked: bool,
    pub created_ts: i64,
    pub last_login_ts: i64,
    pub last_logout_ts: i64,
    /// `lan` / `wan`：在线取当前连接，离线取最近一次。
    pub net: String,
    pub online: bool,
    pub week_speech_ms: u64,
    pub week_words: u64,
    pub total_speech_ms: u64,
    pub total_words: u64,
}

#[derive(Serialize)]
pub struct Overview {
    pub enabled: bool,
    pub port: u16,
    pub wan_host: String,
    pub wan_port: u16,
    /// 网关在运行（控制通道连得上）。
    pub running: bool,
    pub paused: bool,
    /// 内网 `地址:端口`（自动）。
    pub lan: String,
    pub fingerprint: String,
    pub online_count: usize,
    pub paired_count: usize,
    pub terminals: Vec<TerminalRow>,
}

fn status() -> Option<Status> {
    control::call(&Request::Status).ok().and_then(|r| r.status)
}

#[tauri::command]
pub fn asr_server_overview() -> Result<Overview, String> {
    let settings = ServiceSettings::load();
    let st = status();
    let store = TerminalStore::load();
    let usage = stats::terminal_usage().unwrap_or_default();
    let online = st.as_ref().map(|s| s.online.as_slice()).unwrap_or(&[]);
    let mut terminals: Vec<TerminalRow> = store
        .terminals
        .iter()
        // 从没绑定过的 key（添加了没去连）不进列表：过期即清，未过期只是在等扫码。
        .filter(|t| t.bound())
        .map(|t| {
            let on = online.iter().find(|o| o.terminal_id == t.id);
            let u = usage.iter().find(|u| u.terminal_id == t.id);
            TerminalRow {
                id: t.id.clone(),
                device_name: t.device_name.clone(),
                device_id: t.device_id.clone(),
                bound: t.bound(),
                revoked: t.revoked,
                created_ts: t.created_ts,
                last_login_ts: t.last_login_ts,
                last_logout_ts: t.last_logout_ts,
                net: on
                    .map(|o| o.net.clone())
                    .unwrap_or_else(|| t.last_net.clone()),
                online: on.is_some(),
                week_speech_ms: u.map_or(0, |u| u.week.speech_ms),
                week_words: u.map_or(0, |u| u.week.words),
                total_speech_ms: u.map_or(0, |u| u.total.speech_ms),
                total_words: u.map_or(0, |u| u.total.words),
            }
        })
        .collect();
    // 在线在前，其次按最近登录。
    terminals.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then(b.last_login_ts.cmp(&a.last_login_ts))
    });
    Ok(Overview {
        enabled: settings.enabled,
        port: settings.port,
        wan_host: settings.wan_host.clone(),
        wan_port: settings.wan_port,
        running: st.is_some(),
        paused: st.as_ref().is_some_and(|s| s.paused),
        lan: settings.lan_endpoint().unwrap_or_default(),
        fingerprint: st
            .as_ref()
            .map(|s| s.fingerprint.clone())
            .or_else(pairing::cert_fingerprint)
            .unwrap_or_default(),
        online_count: terminals.iter().filter(|t| t.online).count(),
        paired_count: terminals.iter().filter(|t| !t.revoked).count(),
        terminals,
    })
}

/// 启动 / 停止（总开关）。内核看护线程 1 秒内跟上。
#[tauri::command]
pub fn asr_server_set_enabled(enabled: bool) -> Result<(), String> {
    let mut s = ServiceSettings::load();
    s.enabled = enabled;
    s.save().map_err(|_| ERR_IO.to_string())
}

/// 保存外网地址（选填；端口 0 = 与本机端口相同）。
#[tauri::command]
pub fn asr_server_save_wan(wan_host: String, wan_port: u16) -> Result<(), String> {
    let mut s = ServiceSettings::load();
    s.wan_host = wan_host.trim().to_string();
    s.wan_port = wan_port;
    s.save().map_err(|_| ERR_IO.to_string())
}

#[tauri::command]
pub fn asr_server_set_paused(paused: bool) -> Result<(), String> {
    let req = if paused {
        Request::Pause
    } else {
        Request::Resume
    };
    control::call(&req).map_err(|_| ERR_NOT_RUNNING.to_string())?;
    Ok(())
}

#[derive(Serialize)]
pub struct AddedTerminal {
    #[serde(flatten)]
    pub added: Added,
    /// 二维码（SVG 文本）。
    pub qr_svg: String,
    pub fingerprint: String,
}

#[tauri::command]
pub fn asr_server_add_terminal() -> Result<AddedTerminal, String> {
    let resp = control::call(&Request::AddTerminal).map_err(|_| ERR_NOT_RUNNING.to_string())?;
    let added = match (resp.error.is_empty(), resp.added) {
        (true, Some(a)) => a,
        _ => return Err(ERR_IO.to_string()),
    };
    let qr_svg = qrcode::QrCode::new(added.uri.as_bytes())
        .map(|q| {
            q.render::<qrcode::render::svg::Color>()
                .min_dimensions(220, 220)
                .quiet_zone(true)
                .build()
        })
        .unwrap_or_default();
    Ok(AddedTerminal {
        added,
        qr_svg,
        fingerprint: status().map(|s| s.fingerprint).unwrap_or_default(),
    })
}

#[tauri::command]
pub fn asr_server_revoke(terminal_id: String) -> Result<(), String> {
    match control::call(&Request::Revoke {
        terminal_id: terminal_id.clone(),
    }) {
        Ok(r) if r.error.is_empty() => Ok(()),
        Ok(_) => Err(ERR_NOT_FOUND.to_string()),
        // 网关没在运行：直接改文件（此时没有别的写者）。
        Err(_) => {
            let mut st = TerminalStore::load();
            if !st.revoke(&terminal_id) {
                return Err(ERR_NOT_FOUND.to_string());
            }
            st.save().map_err(|_| ERR_IO.to_string())
        }
    }
}

#[derive(Serialize)]
pub struct SessionRow {
    pub login_ts: i64,
    pub logout_ts: Option<i64>,
}

/// 某终端最近 20 次登录 / 退出。
#[tauri::command]
pub fn asr_server_sessions(terminal_id: String) -> Result<Vec<SessionRow>, String> {
    stats::terminal_sessions(&terminal_id, 20)
        .map(|v| {
            v.into_iter()
                .map(|s| SessionRow {
                    login_ts: s.login_ts,
                    logout_ts: s.logout_ts,
                })
                .collect()
        })
        .map_err(|_| ERR_IO.to_string())
}
