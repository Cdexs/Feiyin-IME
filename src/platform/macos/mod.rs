//! macOS platform implementation.
//!
//! Implemented today:
//! - global hotkey listening via `CGEventTap + CFRunLoop`
//! - text injection via `enigo`
//! - clipboard helpers via `pbcopy` / `pbpaste`
//!
//! Still pending:
//! - focused text snapshot / readback
//! - auto-launch
//! - controller/event-loop host
//! - scene signal capture (MACOS-COMPAT-001-CORE stub only)

pub mod accessibility;
mod hotkey;
mod injection;

use anyhow::{anyhow, Result};
use std::sync::{Arc, RwLock};

use crate::config::AppConfig;

pub use hotkey::{HotkeyEvent, HotkeyListener};
pub use injection::{
    capture_focused_text_snapshot, copy_text_to_clipboard, inject_text, read_text_from_hwnd,
    FocusedTextSnapshot,
};

/// macOS auto-launch stubs (Phase 3)
pub fn enable() -> Result<()> {
    Err(anyhow!("macOS auto-launch not implemented (Phase 3)"))
}

pub fn disable() -> Result<()> {
    Err(anyhow!("macOS auto-launch not implemented (Phase 3)"))
}

pub fn is_enabled() -> bool {
    false
}

/// macOS Event Loop stub (Phase 3)
/// Phase 3 will use Tauri event host or CFRunLoop
pub fn create_controller_window() -> Result<()> {
    log::warn!("macOS controller window not implemented (Phase 3)");
    Ok(())
}

pub fn destroy_controller_window() -> Result<()> {
    Ok(())
}

pub fn run_message_loop() -> Result<()> {
    // Check accessibility at startup
    accessibility::ensure_accessibility_at_startup()?;
    log::warn!("macOS message loop not implemented (Phase 3 - Tauri event host)");
    Ok(())
}

/// Notify the hotkey listener that config has changed.
///
/// MACOS-COMPAT-001-CORE stub. 与 Windows 侧 `notify_config_changed()` 形状一致
///（无入参、无返回），Windows 侧设 atomic flag + log，macOS 侧目前仅 log，
/// 真实实现（唤醒 CFRunLoop 或通知 Tauri 事件宿主）由 macOS 团队负责。
// TODO(macOS team): implement real config-change notification (CFRunLoop wake / Tauri event).
pub fn notify_config_changed() {
    log::warn!("macOS notify_config_changed: stub called, real implementation pending");
}

/// Capture scene signals (process exe name + window title) from the foreground window.
///
/// MACOS-COMPAT-001-CORE stub. 签名与 Windows 侧 `capture_scene_signals(hwnd: HWND)`
/// 形状一致（arity=1），仅入参类型平台化（`HWND` → `usize`）。当前返回 `None`
/// 调用方降级为 Unknown 场景，与 Windows 侧失败降级语义一致。
///
/// `hwnd: usize` 的语义由 macOS 团队决定（CGWindowID / AXUIElement 指针 / 或忽略不用），
/// 此处只保证接缝形状，不预设语义。
// TODO(macOS team): implement real scene signal capture (NSWorkspace frontmostApp + AXUIElement).
pub fn capture_scene_signals(_hwnd: usize) -> Option<(String, String)> {
    log::warn!("macOS capture_scene_signals: stub called, returning None");
    None
}
