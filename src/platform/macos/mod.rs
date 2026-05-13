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
