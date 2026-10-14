//! SC-LLM-485 / SC-PUNCT-486：功能开关快捷方式在设置界面一侧的两件事。
//!
//! 1. **录制期间让主程序放行按键**（[`set_shortcut_capture`]）：快捷方式对话框在录新组合时持有
//!    具名互斥量 `Local\FeiyinIME.ShortcutCapture`；主程序的键盘钩子见到它存在就不拦截
//!    （`src/platform/windows/hotkey.rs::shortcut_capture_active`），否则按下当前快捷方式 / 录音键
//!    会被主程序吃掉、对话框收不到。设置进程退出时系统自动回收句柄，不会让主程序永久放行。
//! 2. **配置被主程序改了，界面跟着刷新**（[`spawn_config_watcher`]）：用户在别处按快捷方式切开关，
//!    主程序写回 config.toml；设置界面若不刷新，下一次任意保存会把旧开关状态写回去（界面保存整份配置）。
//!    自己保存的内容不回推（[`save_config_tracked`] 记下写入内容）。

use crate::config::AppConfig;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter};

/// 前端监听的事件名（`ui/src/App.tsx`）。
pub const CONFIG_CHANGED_EVENT: &str = "config-changed";
const POLL_INTERVAL: Duration = Duration::from_millis(700);

/// 最近一次「已知」的 config.toml 内容：本进程写入的，或已经推给界面的。
static LAST_KNOWN: Mutex<Option<String>> = Mutex::new(None);

fn last_known() -> std::sync::MutexGuard<'static, Option<String>> {
    LAST_KNOWN.lock().unwrap_or_else(|e| e.into_inner())
}

/// 界面保存配置：写盘并记下内容，监视线程据此不把自己的保存当成外部改动。
pub fn save_config_tracked(config: &AppConfig) -> anyhow::Result<()> {
    let mut known = last_known();
    config.save()?;
    *known = std::fs::read_to_string(AppConfig::config_path()).ok();
    Ok(())
}

/// 文件内容与已知内容不同 ⇒ 是外部改动。
fn is_external_change(known: Option<&str>, now: &str) -> bool {
    known != Some(now)
}

/// 每 0.7 秒看一次 config.toml 的修改时间，变了且内容不是自己写的 ⇒ 重新加载并推 `config-changed`。
pub fn spawn_config_watcher(app: AppHandle) {
    let path = AppConfig::config_path();
    {
        let mut known = last_known();
        if known.is_none() {
            *known = std::fs::read_to_string(&path).ok();
        }
    }
    let mtime = |p: &std::path::Path| -> Option<SystemTime> {
        std::fs::metadata(p).and_then(|m| m.modified()).ok()
    };
    let _ = std::thread::Builder::new()
        .name("config-watch".into())
        .spawn(move || {
            let mut last_mtime = mtime(&path);
            loop {
                std::thread::sleep(POLL_INTERVAL);
                let now_mtime = mtime(&path);
                if now_mtime == last_mtime {
                    continue;
                }
                // 先拿锁再读：界面保存也持此锁，避免读到保存前的旧内容、把它当成外部改动推回去。
                let mut known = last_known();
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if !is_external_change(known.as_deref(), &content) {
                    last_mtime = now_mtime;
                    continue;
                }
                // 解析失败（半截文件等）不更新已知状态，下一轮重试。
                let Ok(cfg) = AppConfig::load() else {
                    continue;
                };
                *known = Some(content);
                last_mtime = now_mtime;
                drop(known);
                let _ = app.emit(CONFIG_CHANGED_EVENT, cfg);
            }
        });
}

#[cfg(target_os = "windows")]
mod capture {
    use std::sync::Mutex;
    use windows::core::w;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Threading::CreateMutexW;

    /// 持有中的互斥量句柄（`HANDLE` 不是 Send，存原始值）。
    static HELD: Mutex<Option<isize>> = Mutex::new(None);

    pub fn set(active: bool) {
        let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            if active {
                if held.is_none() {
                    if let Ok(h) = CreateMutexW(None, false, w!("Local\\FeiyinIME.ShortcutCapture"))
                    {
                        *held = Some(h.0 as isize);
                    }
                }
            } else if let Some(raw) = held.take() {
                let _ = CloseHandle(HANDLE(raw as *mut core::ffi::c_void));
            }
        }
    }
}

/// 快捷方式对话框开始 / 结束录制新组合。
#[tauri::command]
pub fn set_shortcut_capture(active: bool) {
    #[cfg(target_os = "windows")]
    capture::set(active);
    // TODO(macOS team)：主程序快捷方式监听实现后，在此提供同等的「录制期间放行」信号。
    #[cfg(not(target_os = "windows"))]
    let _ = active;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_save_is_not_external() {
        assert!(!is_external_change(Some("a = 1\n"), "a = 1\n"));
    }

    #[test]
    fn different_content_is_external() {
        assert!(is_external_change(Some("a = 1\n"), "a = 2\n"));
        assert!(is_external_change(None, "a = 1\n"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn capture_mutex_visible_while_held() {
        use windows::core::w;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
        let visible = || unsafe {
            match OpenMutexW(
                SYNCHRONIZATION_SYNCHRONIZE,
                false,
                w!("Local\\FeiyinIME.ShortcutCapture"),
            ) {
                Ok(h) => {
                    let _ = CloseHandle(h);
                    true
                }
                Err(_) => false,
            }
        };
        set_shortcut_capture(true);
        set_shortcut_capture(true); // 重复开始不叠加句柄
        assert!(visible());
        set_shortcut_capture(false);
        assert!(!visible());
        set_shortcut_capture(false); // 重复结束无副作用
    }
}
