//! Platform Abstraction Layer
//!
//! This module provides platform-independent interfaces for:
//! - Global hotkey registration and listening
//! - Text injection into focused applications
//! - Auto-launch (开机自启)
//!
//! Platform-specific implementations are submodules:
//! - `windows/` - Windows implementation using Win32 API
//! - `macos/` - macOS implementation (Phase 3)
//!
//! ## MACOS-COMPAT-001-CORE · 平台导出契约
//!
//! 本层两侧（`windows` / `macos`）**必须**各自提供以下公开符号：
//!
//! | 模块 | 符号 |
//! |---|---|
//! | autolaunch | `enable`, `disable`, `is_enabled` |
//! | event_loop | `create_controller_window`, `destroy_controller_window`, `run_message_loop`, `foreground_window_id` |
//! | hotkey | `notify_config_changed`, `HotkeyEvent`, `HotkeyListener` |
//! | injection | `capture_focused_text_snapshot`, `copy_text_to_clipboard`, `inject_text`, `read_text_from_hwnd`, `FocusedTextSnapshot` |
//! | scene | `capture_scene_signals`, `capture_scene_signals_by_id` |
//!
//! ### 平台中立符号（两侧签名完全一致，共享代码可直接调用）
//!
//! - `WindowId` — 不透明窗口标识类型别名（`pub type WindowId = usize;`），
//!   两侧语义各自定义。Windows 侧是 `HWND.0 as usize`，macOS 侧第一版返回 0
//!   表示「无法判定焦点」（见 `foreground_window_id`）。
//! - `capture_scene_signals_by_id(id: WindowId) -> Option<(String, String)>`
//!   — 由 `capture_scene_signals` 派生的 by-id 版本，供平台中立代码调用。
//! - `foreground_window_id() -> WindowId`
//!   — 返回当前前台窗口的 `WindowId`。macOS 第一版返回 0（降级语义）。
//!
//! ### 平台相关类型差异（刻意保留，不是遗漏）
//!
//! 以下 API 在两侧签名不同，调用方**必须在 `#[cfg]` 分支内使用**：
//!
//! - `create_controller_window()`
//!   - Windows: `Result<HWND>` / macOS: `Result<()>`
//! - `destroy_controller_window(hwnd)`
//!   - Windows: 入参 `HWND` / macOS: 无入参
//! - `FocusedTextSnapshot.hwnd`
//!   - Windows: `HWND` / macOS: `usize`
//! - `read_text_from_hwnd(hwnd)`
//!   - Windows: 入参 `HWND` / macOS: 入参 `usize`
//! - `capture_scene_signals(hwnd)`
//!   - Windows: 入参 `HWND` / macOS: 入参 `usize`（MACOS-COMPAT-001-CORE stub）
//!
//! ### 为什么不统一类型
//!
//! 统一 `HWND`/`usize` 需改 `src/injection/mod.rs` 与 `platform/windows/*`，
//! 属 Windows 已交付路径（v0.7.2），违反 DEC-033 第 4 条硬约束
//! "代码重构不得影响任何 Windows 代码功能"。故刻意保留差异，靠契约注释 + 双平台 CI 兜底。
//!
//! ### 为什么不用 trait 抽象
//!
//! trait 只约束当前编译目标上的实现；`#[cfg(...)]` 切掉的另一侧实现编译器从 AST 移除、
//! 不做类型检查（✅官方 Rust Reference）。trait 防不住 cfg 掉的那一侧漂移。
//! 真正的防线是显式清单 + 双平台 CI（coder-2 并行任务在建）。
//!
//! ### stub 设计原则【MACOS-COMPAT-001-CORE】
//!
//! 新增 macOS stub 一律优先保证 **名称 + arity 与 Windows 侧相同，仅参数类型平台化**
//!（如 `HWND` → `usize`）。这样两侧函数形状一致，调用点未来有机会去掉 `#[cfg]` 变成共享代码。
//! arity 差异是最后手段——既有 `destroy_controller_window`（Windows 收 `HWND` / macOS 无参）
//! 属历史遗留，按红线不动，但**不要以它为范本**。

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{
    capture_focused_text_snapshot, capture_scene_signals, capture_scene_signals_by_id,
    copy_text_to_clipboard, create_controller_window, destroy_controller_window, disable, enable,
    foreground_window_id, inject_text, is_enabled, notify_config_changed, read_text_from_hwnd,
    run_message_loop, FocusedTextSnapshot, HotkeyEvent, HotkeyListener,
};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{
    capture_focused_text_snapshot, capture_scene_signals, capture_scene_signals_by_id,
    copy_text_to_clipboard, create_controller_window, destroy_controller_window, disable, enable,
    foreground_window_id, inject_text, is_enabled, notify_config_changed, read_text_from_hwnd,
    request_stop, run_message_loop, run_message_loop_with_hotkey_listener, FocusedTextSnapshot,
    HotkeyEvent, HotkeyListener,
};

use std::sync::{Arc, RwLock};

use crate::config::AppConfig;

/// 不透明窗口标识类型（两侧语义各自定义）。
///
/// - Windows 侧：`HWND.0 as usize`（裸指针转数值）。
/// - macOS 侧：第一版返回 0 表示「无法判定焦点」，与 Windows 侧
///   `target_hwnd` 为空（`is_null()`）时的降级语义一致。
///
/// 供平台中立代码（如 `run_pipeline_core`）使用，避免直接接触 `HWND`。
pub type WindowId = usize;

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
