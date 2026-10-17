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
use std::sync::{Arc, RwLock};
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

/// S7 托盘单项（设计 §16.7）：`(文案, 可点)`。开着（含暂停中）⇒「停止识别服务」；关着 ⇒「启动识别服务」，
/// 本机没加载本地实时模型（服务未就绪）时置灰并在文案里说明原因（Win32 / macOS 菜单项都没有悬停提示）。
pub(crate) fn tray_item(ui_language: crate::config::UiLanguage) -> (&'static str, bool) {
    let s = crate::i18n::get(ui_language);
    if ENABLED.load(Ordering::Relaxed) {
        (s.tray_menu_asr_stop, true)
    } else if crate::transcription::remote_engines_ready() {
        (s.tray_menu_asr_start, true)
    } else {
        (s.tray_menu_asr_unavailable, false)
    }
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
