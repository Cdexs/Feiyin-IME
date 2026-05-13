//! 崩溃报告模块
//!
//! 提供崩溃信息收集、本地存储，以及拉起独立 crash reporter 的能力。

mod storage;

pub use storage::{save_crash_report, CrashReport, CrashType, RuntimeInfo, SystemInfo};

/// 创建崩溃报告（用于 panic hook）
pub fn create_report_from_panic(
    panic_info: &std::panic::PanicHookInfo<'_>,
    version: &str,
    runtime: RuntimeInfo,
    recent_logs: Vec<String>,
) -> CrashReport {
    use backtrace::Backtrace;
    use chrono::Local;

    // 获取 panic 消息
    let crash_message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic".to_string()
    };

    // 获取线程名
    let thread = std::thread::current()
        .name()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "main".to_string());

    // 获取堆栈追踪
    let backtrace = Backtrace::new();
    let backtrace_lines: Vec<String> = backtrace
        .frames()
        .iter()
        .flat_map(|frame| {
            frame.symbols().iter().map(|sym| {
                let name = sym
                    .name()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());
                let file = sym
                    .filename()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());
                let line = sym
                    .lineno()
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());
                format!("{} @ {}:{}", name, file, line)
            })
        })
        .collect();

    CrashReport {
        version: version.to_string(),
        timestamp: Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        crash_type: CrashType::Panic,
        crash_message,
        thread,
        backtrace: backtrace_lines,
        system: get_system_info(),
        runtime,
        recent_logs,
    }
}

/// 获取系统信息
fn get_system_info() -> SystemInfo {
    // 获取 OS 版本信息
    let os_version = get_windows_version();

    // 获取架构
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "unknown"
    };

    // 获取系统语言
    let language = std::env::var("LANG").unwrap_or_else(|_| {
        // Windows 默认使用系统区域设置
        std::env::var("SystemLanguage").unwrap_or_else(|_| "zh-CN".to_string())
    });

    SystemInfo {
        os_version,
        arch: arch.to_string(),
        language,
    }
}

/// 获取 Windows 版本信息（通过注册表，Win7 兼容）
fn get_windows_version() -> String {
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
    };

    let sub_key = w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");

    unsafe {
        let mut h_key = HKEY::default();

        // 打开注册表键
        let result = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(sub_key.as_ptr()),
            0,
            KEY_READ,
            &mut h_key,
        );

        if result.is_err() {
            return "Windows (version unknown)".to_string();
        }

        // 读取 CurrentBuild 或 BuildLabEx
        let mut buffer: [u16; 256] = [0; 256];
        let mut buf_size = 256u32;
        let build_result = RegQueryValueExW(
            h_key,
            PCWSTR(w!("CurrentBuild").as_ptr()),
            None,
            None,
            Some(buffer.as_mut_ptr() as *mut u8),
            Some(&mut buf_size),
        );

        // 读取 DisplayVersion (如 21H2)
        let mut display_ver: [u16; 64] = [0; 64];
        let mut display_size = 64u32;
        let display_result = RegQueryValueExW(
            h_key,
            PCWSTR(w!("DisplayVersion").as_ptr()),
            None,
            None,
            Some(display_ver.as_mut_ptr() as *mut u8),
            Some(&mut display_size),
        );

        // 关闭键
        let _ = RegCloseKey(h_key);

        let build = if build_result.is_ok() && buf_size > 0 {
            let len = (buf_size / 2) as usize;
            String::from_utf16_lossy(&buffer[..len.saturating_sub(1)]) // 去掉末尾 null
        } else {
            "unknown".to_string()
        };

        let display = if display_result.is_ok() && display_size > 0 {
            let len = (display_size / 2) as usize;
            String::from_utf16_lossy(&display_ver[..len.saturating_sub(1)])
        } else {
            String::new()
        };

        if display.is_empty() {
            format!("Windows Build {}", build)
        } else {
            format!("Windows {} (Build {})", display, build)
        }
    }
}

/// 启动独立 crash reporter 进程。
///
/// 如果 reporter 可执行文件不存在，调用方应降级为仅保留本地 crash.json。
pub fn spawn_reporter_process() -> std::io::Result<()> {
    let current_exe = std::env::current_exe()?;
    let reporter_path = current_exe.with_file_name(if cfg!(target_os = "windows") {
        "crash-reporter.exe"
    } else {
        "crash-reporter"
    });

    if !reporter_path.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Crash reporter not found: {}", reporter_path.display()),
        ));
    }

    std::process::Command::new(reporter_path)
        .spawn()
        .map(|_| ())
}
