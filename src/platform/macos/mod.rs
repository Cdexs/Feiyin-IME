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
pub mod event_loop;
mod hotkey;
mod injection;

use anyhow::{anyhow, Result};
use std::sync::{Arc, RwLock};

use crate::config::AppConfig;

use super::WindowId;

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

/// macOS Event Loop
///
/// Delegate to `event_loop` module. Windows 侧返回 HWND，macOS 侧无对应物，故签名
/// 保持为 `Result<()>`（平台化差异，按 MACOS-COMPAT-001-CORE 契约保留）。
pub use event_loop::{
    create_controller_window, destroy_controller_window, request_stop, run_message_loop,
    run_message_loop_with_hotkey_listener,
};

/// Notify the controller that config has changed.
///
/// Wakes the main CFRunLoop so the next 15ms timer tick sees the new config.
/// 与 Windows 侧 `notify_config_changed()` 形状一致（无入参、无返回）。
pub fn notify_config_changed() {
    event_loop::notify_config_changed();
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

/// MACOS-P4-NEUTRAL-001: 平台中立 by-id 版本，供 `run_pipeline_core` 调用。
/// 委托给既有 `capture_scene_signals`（当前为 stub，返回 None）。
pub fn capture_scene_signals_by_id(id: WindowId) -> Option<(String, String)> {
    capture_scene_signals(id)
}

/// MACOS-P4-NEUTRAL-001: 返回当前前台窗口的不透明标识。
///
/// 第一版返回 0 表示「无法判定焦点」，使 `run_pipeline_core` 内的
/// `focus_lost` 判定恒为 false（`target_hwnd != 0 && current_id != target_hwnd`
/// 因 `current_id == 0` 且 `target_hwnd` 非 0 时为 false）。
/// 与 Windows 侧 `target_hwnd` 为空（`is_null()`）时的降级语义一致
/// （见 main.rs 原 :3179 的 is_null 判据）。
// TODO(macOS team): 用 NSWorkspace.frontmostApplication / AXUIElement 实现真实前台窗口标识。
pub fn foreground_window_id() -> WindowId {
    0
}

#[cfg(test)]
mod macos_platform_contract_tests {
    use super::*;

    /// P0-3: foreground_window_id 契约测试。
    /// 第一版返回 0 表示「无法判定焦点」。
    /// ⚠️ 将来 MACOS-P4-HOST-001 实现真实版本时，本测试会变红，
    ///    强制实现者回来更新契约（返回值不再是 0）。
    #[test]
    fn foreground_window_id_returns_zero_in_first_version() {
        let id = foreground_window_id();
        assert_eq!(id, 0, "MACOS-P4-NEUTRAL-001 第一版: foreground_window_id 应返回 0 表示无法判定焦点");
    }

    /// P1: capture_scene_signals_by_id 降级语义。
    /// 当前委托给 stub（恒返 None），锁住「失败降级为 Unknown 场景」的契约。
    #[test]
    fn capture_scene_signals_by_id_returns_none_for_zero_id() {
        let result = capture_scene_signals_by_id(0);
        assert!(
            result.is_none(),
            "MACOS-COMPAT-001-CORE stub: capture_scene_signals_by_id(0) 应返回 None（降级为 Unknown 场景）"
        );
    }
}
