//! ASR-SERVER-491 S4c：内核看护识别服务（设计 §2 / §16.2）。
//!
//! 一个常驻线程每秒看一次 `asr-service.json`（文件没变只读一次修改时间）：
//! - 总开关开 ⇒ 建本机 IPC（[`AsrService`]）+ 拉起网关 `feiyin-asr-gateway`（口令走环境变量，不上命令行）；
//!   网关意外退出 ⇒ 5 秒后重拉（IPC 不动，同一口令）；端口改了 ⇒ 整体重启。
//! - 总开关关 ⇒ **先结束网关、再停 IPC**（反过来 `AsrService::stop` 会等着网关断开）。
//! - 设置界面 / 托盘只改这个文件，状态天然一致（DEC-031 单开关）。
//!
//! 不用 Job 对象 / 进程组：网关读到 IPC 断开就自行退出，内核崩溃也不会留下孤儿进程。

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime};

use ring::rand::{SecureRandom, SystemRandom};

use super::AsrService;
use crate::asr_shared::control::GatewayInfo;
use crate::asr_shared::settings::ServiceSettings;
use crate::config::AppConfig;

const POLL: Duration = Duration::from_secs(1);
const RESPAWN_DELAY: Duration = Duration::from_secs(5);

#[cfg(target_os = "windows")]
const GATEWAY_EXE: &str = "feiyin-asr-gateway.exe";
#[cfg(not(target_os = "windows"))]
const GATEWAY_EXE: &str = "feiyin-asr-gateway";

/// 总开关当前值（看护线程每次读设置文件后更新；托盘读它定文案，不碰文件）。
static ENABLED: AtomicBool = AtomicBool::new(false);

/// 运行时配置（`spawn` 时登记），托盘判「可启动」用。
static CONFIG: OnceLock<Arc<RwLock<AppConfig>>> = OnceLock::new();

/// S7 托盘单项（设计 §16.7）：`(文案, 可点)`。开着（含暂停中）⇒「停止识别服务」；关着 ⇒「启动识别服务」，
/// 语音输入没选本地流式实时模型时置灰。
/// TRAY-ASR-503（Gavin 10-19「程序刚启动的时候，托盘菜单里面的启动asr服务的菜单会变成灰显，不可点击。
/// 另外，要去掉菜单显示文字里的“（需本地流式实时模型）”」）：
/// - 判据改为与设置页「识别服务」同一条（配置选了本地流式实时模型，`AsrServer.tsx` `ready`），
///   不再看模型是否已加载完 —— 原判据在启动加载模型的几秒里恒为否 ⇒ 菜单灰显。
///   模型加载中开服务是安全的：远程会话在模型就绪前回 `not_ready`（设计 §7）。
/// - 置灰时文案就是「启动识别服务」，不带括号说明。
pub(crate) fn tray_item(ui_language: crate::config::UiLanguage) -> (&'static str, bool) {
    let enabled = ENABLED.load(Ordering::Relaxed);
    tray_item_for(enabled, !enabled && local_realtime_selected(), ui_language)
}

/// `tray_item` 的纯函数部分（可单测）：开着 ⇒ 停止项可点；关着 ⇒ 启动项，选了本地流式实时模型才可点。
fn tray_item_for(
    enabled: bool,
    local_realtime: bool,
    ui_language: crate::config::UiLanguage,
) -> (&'static str, bool) {
    let s = crate::i18n::get(ui_language);
    if enabled {
        (s.tray_menu_asr_stop, true)
    } else {
        (s.tray_menu_asr_start, local_realtime)
    }
}

/// 语音输入是否选了本地流式实时模型（与设置页识别服务「就绪」同一判据）。
/// 看护线程未起（`CONFIG` 未登记）时退回「模型已加载」判据。
fn local_realtime_selected() -> bool {
    match CONFIG.get() {
        Some(cfg) => {
            let cfg = cfg.read().unwrap_or_else(|e| e.into_inner());
            crate::transcription::AsrModel::from_config(&cfg.audio.asr_model)
                == crate::transcription::AsrModel::LocalRealtime
        }
        None => crate::transcription::remote_engines_ready(),
    }
}

/// TRAY-ICON-500：菜单项当前是否为「停止识别服务」——图标跟 `tray_item` 给出的文案同源判定，
/// 不另读开关（避免两次读之间开关翻转导致图标与文案不一致）。
pub(crate) fn tray_item_is_stop(label: &str, ui_language: crate::config::UiLanguage) -> bool {
    label == crate::i18n::get(ui_language).tray_menu_asr_stop
}

/// 托盘点击：翻转总开关并写回设置文件（与设置页同一个开关，看护线程 1 秒内跟上）。
pub(crate) fn toggle_from_tray() {
    let mut s = ServiceSettings::load();
    s.enabled = !s.enabled;
    match s.save() {
        Ok(()) => ENABLED.store(s.enabled, Ordering::Relaxed),
        Err(e) => log::warn!("[ASR-SERVER-491] save service switch failed: {e}"),
    }
}

/// 起看护线程（进程生命周期内常驻；进程退出时网关随 IPC 断开自行退出）。
pub(crate) fn spawn(runtime_config: Arc<RwLock<AppConfig>>) {
    let _ = CONFIG.set(Arc::clone(&runtime_config));
    let r = std::thread::Builder::new()
        .name("asr-supervisor".into())
        .spawn(move || run(runtime_config));
    if let Err(e) = r {
        log::warn!("[ASR-SERVER-491] supervisor thread failed: {e}");
    }
}

struct Running {
    svc: AsrService,
    token: String,
    port: u16,
    child: Option<Child>,
    /// 网关退出后的重拉时刻。
    respawn_at: Option<Instant>,
}

fn run(runtime_config: Arc<RwLock<AppConfig>>) {
    let mut running: Option<Running> = None;
    let mut seen: Option<Option<SystemTime>> = None;
    let mut want = ServiceSettings::default();
    let mut retry_at = Instant::now();
    loop {
        let mtime = std::fs::metadata(ServiceSettings::path())
            .and_then(|m| m.modified())
            .ok();
        if seen != Some(mtime) {
            seen = Some(mtime);
            want = ServiceSettings::load();
            ENABLED.store(want.enabled, Ordering::Relaxed);
        }
        // 端口变了 ⇒ 停掉重来。
        if running
            .as_ref()
            .is_some_and(|r| !want.enabled || r.port != want.port)
        {
            if let Some(r) = running.take() {
                stop(r);
            }
        }
        if want.enabled {
            match running.as_mut() {
                None if Instant::now() >= retry_at => match start(&runtime_config, want.port) {
                    Ok(r) => running = Some(r),
                    Err(e) => {
                        log::warn!("[ASR-SERVER-491] service start failed: {e}");
                        retry_at = Instant::now() + RESPAWN_DELAY;
                    }
                },
                Some(r) => watch_child(r),
                None => {}
            }
        }
        std::thread::sleep(POLL);
    }
}

fn start(runtime_config: &Arc<RwLock<AppConfig>>, port: u16) -> std::io::Result<Running> {
    let token = new_token()?;
    let svc = AsrService::start(Arc::clone(runtime_config), token.clone())?;
    let child = spawn_gateway(svc.endpoint(), &token);
    if let Err(e) = &child {
        log::warn!("[ASR-SERVER-491] gateway spawn failed: {e}");
    }
    log::info!("[ASR-SERVER-491] service started (port {port})");
    Ok(Running {
        svc,
        token,
        port,
        respawn_at: child.is_err().then(|| Instant::now() + RESPAWN_DELAY),
        child: child.ok(),
    })
}

/// 网关退了 ⇒ 记下 5 秒后重拉；到点就拉。
fn watch_child(r: &mut Running) {
    if let Some(c) = r.child.as_mut() {
        match c.try_wait() {
            Ok(None) => return,
            Ok(Some(st)) => log::warn!("[ASR-SERVER-491] gateway exited: {st}"),
            Err(e) => log::warn!("[ASR-SERVER-491] gateway wait failed: {e}"),
        }
        r.child = None;
        r.respawn_at = Some(Instant::now() + RESPAWN_DELAY);
    }
    if r.respawn_at.is_some_and(|t| Instant::now() >= t) {
        match spawn_gateway(r.svc.endpoint(), &r.token) {
            Ok(c) => {
                r.child = Some(c);
                r.respawn_at = None;
            }
            Err(e) => {
                log::warn!("[ASR-SERVER-491] gateway respawn failed: {e}");
                r.respawn_at = Some(Instant::now() + RESPAWN_DELAY);
            }
        }
    }
}

fn stop(mut r: Running) {
    if let Some(mut c) = r.child.take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    r.svc.stop();
    // 让设置界面立刻看到「已停止」（不必等连接超时）。
    let _ = std::fs::remove_file(GatewayInfo::path());
    log::info!("[ASR-SERVER-491] service stopped");
}

fn new_token() -> std::io::Result<String> {
    let mut b = [0u8; 32];
    SystemRandom::new()
        .fill(&mut b)
        .map_err(|_| std::io::Error::other("system random unavailable"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

fn gateway_path() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| std::io::Error::other("no exe dir"))?;
    Ok(dir.join(GATEWAY_EXE))
}

fn spawn_gateway(endpoint: &str, token: &str) -> std::io::Result<Child> {
    let log = std::fs::File::create(crate::asr_shared::data_dir().join("asr-gateway.log"))
        .map(Stdio::from)
        .unwrap_or_else(|_| Stdio::null());
    let mut cmd = Command::new(gateway_path()?);
    cmd.arg("--core")
        .arg(endpoint)
        .env("FEIYIN_ASR_TOKEN", token)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd.spawn()?;
    log::info!("[ASR-SERVER-491] gateway spawned (pid {})", child.id());
    Ok(child)
}

#[cfg(test)]
mod tray503_tests {
    use super::*;
    use crate::config::UiLanguage;

    /// TRAY-ASR-503：选了本地流式实时模型 ⇒ 启动项可点（不管模型是否已加载完）；
    /// 没选 ⇒ 置灰但文案仍是「启动识别服务」；任何情形文案都不带括号说明。
    #[test]
    fn tray_item_follows_model_choice_and_plain_label() {
        for lang in [
            UiLanguage::Chinese,
            UiLanguage::TraditionalChinese,
            UiLanguage::English,
        ] {
            let s = crate::i18n::get(lang);
            assert_eq!(
                tray_item_for(false, true, lang),
                (s.tray_menu_asr_start, true)
            );
            assert_eq!(
                tray_item_for(false, false, lang),
                (s.tray_menu_asr_start, false)
            );
            assert_eq!(
                tray_item_for(true, false, lang),
                (s.tray_menu_asr_stop, true)
            );
            for label in [s.tray_menu_asr_start, s.tray_menu_asr_stop] {
                assert!(!label.contains('（') && !label.contains('('), "{label}");
            }
        }
    }

    /// 源码护栏：托盘判据不得退回「模型已加载」（启动初期恒否 ⇒ 灰显）。
    #[test]
    fn tray_item_does_not_gate_on_loaded_engines() {
        let src = include_str!("supervisor.rs");
        let body = src
            .split("pub(crate) fn tray_item(")
            .nth(1)
            .expect("tray_item 锚点");
        let body = &body[..body
            .find(
                "
}
",
            )
            .expect("函数尾")];
        assert!(!body.contains("remote_engines_ready"));
    }
}
