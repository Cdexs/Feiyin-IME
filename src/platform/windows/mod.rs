//! Windows Platform Implementation

mod autolaunch;
mod event_loop;
mod hotkey;
mod injection;
mod scene;

pub use autolaunch::{disable, enable, is_enabled};
pub use event_loop::{create_controller_window, destroy_controller_window, run_message_loop};
pub use hotkey::{notify_config_changed, HotkeyEvent, HotkeyListener};
pub use injection::{
    capture_focused_text_snapshot, copy_text_to_clipboard, inject_text, read_text_from_hwnd,
    FocusedTextSnapshot,
};
pub use scene::capture_scene_signals;
// MACOS-P4-NEUTRAL-001: 新增 re-export，保持既有 use 行逐字不动（Windows 侧纯新增红线）。
pub use event_loop::foreground_window_id;
pub use scene::capture_scene_signals_by_id;
