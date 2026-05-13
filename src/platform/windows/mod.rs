//! Windows Platform Implementation

mod autolaunch;
mod event_loop;
mod hotkey;
mod injection;

pub use autolaunch::{disable, enable, is_enabled};
pub use event_loop::{create_controller_window, destroy_controller_window, run_message_loop};
pub use hotkey::{notify_config_changed, HotkeyEvent, HotkeyListener};
pub use injection::{
    capture_focused_text_snapshot, copy_text_to_clipboard, inject_text, read_text_from_hwnd,
    FocusedTextSnapshot,
};
