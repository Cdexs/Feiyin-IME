//! Platform Abstraction Layer
//!
//! This module provides platform-independent interfaces for:
//! - Global hotkey registration and listening
//! - Text injection into focused applications
//! - Auto-launch (开机自启)
//!
//! Platform-specific implementations are in submodules:
//! - `windows/` - Windows implementation using Win32 API
//! - `macos/` - macOS placeholder (Phase 3)

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

use std::sync::{Arc, RwLock};

use crate::config::AppConfig;

/// Create platform-specific hotkey listener
#[cfg(target_os = "macos")]
pub fn create_hotkey_listener(config: Arc<RwLock<AppConfig>>) -> HotkeyListener {
    HotkeyListener::new(config)
}

/// Create Windows hotkey listener that wakes the controller window when events arrive.
#[cfg(target_os = "windows")]
pub fn create_hotkey_listener_with_controller_wakeup(
    config: Arc<RwLock<AppConfig>>,
    controller_hwnd: ::windows::Win32::Foundation::HWND,
    wake_message: u32,
) -> HotkeyListener {
    HotkeyListener::new_with_controller_wakeup(config, controller_hwnd, wake_message)
}
