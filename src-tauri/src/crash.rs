use crate::config::AppConfig;
use chrono::Local;
use std::backtrace::Backtrace;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

#[path = "../../src/crash/storage.rs"]
mod storage;

pub use storage::{save_crash_report, CrashReport, CrashType, RuntimeInfo, SystemInfo};

static EXPECTED_EXIT: AtomicBool = AtomicBool::new(false);
static CRASH_REPORTED: AtomicBool = AtomicBool::new(false);

pub fn install_panic_hook() {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        previous_hook(panic_info);
        report_panic(panic_info);
    }));
}

pub fn mark_expected_exit() {
    EXPECTED_EXIT.store(true, Ordering::SeqCst);
}

pub fn report_unexpected_exit(reason: &str) {
    if EXPECTED_EXIT.load(Ordering::SeqCst) || CRASH_REPORTED.swap(true, Ordering::SeqCst) {
        return;
    }

    let report = create_report(
        CrashType::Win32Exception,
        reason.to_string(),
        "main".to_string(),
    );
    let _ = save_crash_report(&report);
    spawn_crash_reporter();
}

fn report_panic(panic_info: &std::panic::PanicHookInfo<'_>) {
    if CRASH_REPORTED.swap(true, Ordering::SeqCst) {
        return;
    }

    let crash_message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic".to_string()
    };

    let thread = std::thread::current()
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|| "main".to_string());

    let report = create_report(CrashType::Panic, crash_message, thread);
    let _ = save_crash_report(&report);
    spawn_crash_reporter();
}

fn create_report(crash_type: CrashType, crash_message: String, thread: String) -> CrashReport {
    let config = AppConfig::load().unwrap_or_default();
    CrashReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        crash_type,
        crash_message,
        thread,
        backtrace: capture_backtrace_lines(),
        system: collect_system_info(),
        runtime: collect_runtime_info(&config),
        recent_logs: Vec::new(),
    }
}

fn capture_backtrace_lines() -> Vec<String> {
    Backtrace::force_capture()
        .to_string()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect()
}

fn collect_system_info() -> SystemInfo {
    let os_version = std::env::var("OS").unwrap_or_else(|_| std::env::consts::OS.to_string());
    let language = std::env::var("LANG")
        .or_else(|_| std::env::var("SystemLanguage"))
        .unwrap_or_else(|_| "zh-CN".to_string());

    SystemInfo {
        os_version,
        arch: std::env::consts::ARCH.to_string(),
        language,
    }
}

fn collect_runtime_info(config: &AppConfig) -> RuntimeInfo {
    let mode = if config.audio.enable_streaming {
        "settings-ui (streaming-config)"
    } else {
        "settings-ui"
    };

    RuntimeInfo {
        mode: mode.to_string(),
        llm_enabled: config.llm.enabled,
        llm_verified: config.llm.connectivity_verified,
        asr_model: config.audio.transcription_language.clone(),
    }
}

fn spawn_crash_reporter() {
    let Some(exe_path) = locate_reporter_exe() else {
        eprintln!("Failed to locate standalone crash-reporter executable");
        return;
    };

    let _ = Command::new(exe_path).spawn();
}

fn locate_reporter_exe() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    let exe_dir = current_exe.parent()?;
    let reporter_name = if cfg!(target_os = "windows") {
        "crash-reporter.exe"
    } else {
        "crash-reporter"
    };
    let mut candidates = vec![exe_dir.join(reporter_name)];

    if let Some(project_root) = find_project_root(exe_dir) {
        if let Some(profile) = exe_dir.file_name() {
            candidates.push(
                project_root
                    .join("target")
                    .join(profile)
                    .join(reporter_name),
            );
        }
        candidates.push(
            project_root
                .join("target")
                .join("release")
                .join(reporter_name),
        );
        candidates.push(
            project_root
                .join("target")
                .join("debug")
                .join(reporter_name),
        );
    }

    candidates.into_iter().find(|path| path.exists())
}

fn find_project_root(exe_dir: &Path) -> Option<&Path> {
    exe_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|path| path.join("src-tauri").exists())
}
