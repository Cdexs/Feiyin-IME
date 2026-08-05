//! macOS platform implementation.
//!
//! Implemented today:
//! - global hotkey listening via `CGEventTap + CFRunLoop`
//! - text injection via `enigo`
//! - clipboard helpers via `pbcopy` / `pbpaste`
//! - status bar tray via `NSStatusItem` (MACOS-P4-TRAY-001)
//!
//! Still pending:
//! - focused text snapshot / readback
//! - auto-launch

pub mod accessibility;
pub mod event_loop;
mod hotkey;
mod injection;
mod overlay;
mod scene;
mod tray;

use anyhow::{anyhow, Result};
use std::sync::{Arc, RwLock};

use crate::config::AppConfig;

use super::WindowId;

pub use hotkey::{HotkeyEvent, HotkeyListener};
pub use injection::{
    capture_focused_text_snapshot, copy_text_to_clipboard, inject_text, read_text_from_hwnd,
    FocusedTextSnapshot,
};
pub use tray::{
    build_tray, clear_tray_handle, poll_pending_tray_states, register_tray_handle,
    request_tray_state, StatusBarTray, TrayCommand,
};

// OVERLAY-WIRE-001: recording overlay (NSPanel + CGContext) — 跨线程请求通道。
// request_overlay 任意线程可调；poll_pending_overlay 走 event_loop 内部路径，
// 不在此导出（仿 tray 模式）。
pub use overlay::{init_overlay_levels, request_overlay, shutdown_overlay, OverlayRequest};

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
/// MACOS-P4-SCENE-001-CORE: 真实实现（stub → real）。委托给 `scene` 模块
/// （AX `kAXFocusedApplicationAttribute` + `proc_pidpath` + AX title）。
/// 失败降级返回 `None` → Unknown 场景，与 Windows 侧语义一致。
pub use scene::{capture_scene_signals, capture_scene_signals_by_id};

/// MACOS-P4-NEUTRAL-001: 平台中立 by-id 版本，供 `run_pipeline_core` 调用。
/// 已在 `scene::capture_scene_signals_by_id` 实现并 re-export（见上）。

/// MACOS-P4-NEUTRAL-001: 返回当前前台窗口的不透明标识。
///
/// SCENE-001-D-1 真实现：返回前台应用 pid（`WindowId = usize`），取不到返回 0。
/// 与 Windows 侧 `target_hwnd` 为 HWND 的语义对齐——pid 进程级唯一且稳定，
/// 供 `run_pipeline_core` 的 `focus_lost` 判定与 `capture_scene_signals_by_id` 使用。
pub fn foreground_window_id() -> WindowId {
    scene::foreground_window_id()
}

#[cfg(test)]
mod macos_platform_contract_tests {
    use super::*;

    /// P0-3: foreground_window_id 契约测试（MACOS-P4-SCENE-002 二次换锚：pid → CGWindowID）。
    ///
    /// 契约：返回 0 = 无法判定焦点（无 GUI 会话 / 无前台应用 / 降级失败）；
    /// 返回非 0 = 合法的前台**窗口** CGWindowID（`WindowId = usize`，窗口级唯一稳定）。
    ///
    /// 🔴 为什么不用 `assert_ne!(0)`：测试进程在无 GUI 会话（ssh / 无窗口服务器 /
    /// 将来 CI）下没有 frontmost app，返回 0 是**合法降级**，`assert_ne!(0)` 会随环境
    /// 随机红。故锚定为契约本身：
    ///   ① 紧邻两次调用返回一致值（前台窗口微秒级内不可能切换；若都为 0 也一致）——
    ///      抓住「返回一个随机/不稳定的假 id」这类坏实现；
    ///   ② 非 0 时必须在合法 CGWindowID 范围（`<= u32::MAX`，CGWindowID 是 `u32`）；
    ///   ②b 存在性用**窗口列表**验证（非进程表）：在 `CGWindowListCopyWindowInfo`
    ///     （OnScreenOnly | ExcludeDesktopElements）结果中反查该 id 的
    ///     `kCGWindowNumber` 条目，并进一步断言其 `kCGWindowLayer == 0`
    ///     （正是 scene.rs `frontmost_cg_window_id` 的筛选条件 → 闭环护栏）。
    ///
    /// ⚠️ 为何不是 pid 探测：SCENE-002 已把返回值从 pid 提升为 CGWindowID（窗口号）。
    /// 拿窗口号去 `kill(pid, 0)` 探测进程，语义不成立（本机绿色只是巧合撞上存活 pid，
    /// 换机/重启/窗口号增长后可能 ESRCH 无缘无故变红）——假绿现在、随机红以后，比没有
    /// 测试更坏。故换成语义正确的窗口列表反查。
    ///
    /// 判据说明：采用 3.3 的「窗口列表反查」强判据（而非退化 `u32::MAX` 上界）——
    /// `core-graphics` 是直接依赖（Cargo.toml:113），`copy_window_info`/`kCGWindow*`
    /// 是公共 API，测试可直接调用，无需改 scene.rs 生产可见性。
    #[test]
    fn foreground_window_id_contract() {
        let wid_a = foreground_window_id();
        let wid_b = foreground_window_id();
        // ① 紧邻两次调用一致（0 或合法 CGWindowID 均须自洽）
        assert_eq!(
            wid_a, wid_b,
            "紧邻两次 foreground_window_id 应返回一致值（前台 CGWindowID 或 0）: a={wid_a} b={wid_b}"
        );
        let wid = wid_a;
        if wid == 0 {
            return; // 合法降级：无法判定焦点（无 GUI 会话）
        }
        // ②a 合法 CGWindowID 范围（u32，非 i32）
        assert!(
            wid <= u32::MAX as usize,
            "非 0 返回超出合法 CGWindowID 范围（u32）: {wid}"
        );
        // ②b 在窗口列表中反查（存在性 + layer == 0 闭环护栏）
        let (found, layer) = window_layer_by_id(wid);
        assert!(
            found,
            "CGWindowID {wid} 不在 CGWindowListCopyWindowInfo 结果中，违反 foreground_window_id 契约"
        );
        assert_eq!(
            layer, 0,
            "CGWindowID {wid} 的 kCGWindowLayer 应为 0（scene.rs 筛选条件），实际为 {layer}"
        );
    }

    /// 在 `CGWindowListCopyWindowInfo`（OnScreenOnly | ExcludeDesktopElements）结果中
    /// 反查指定 CGWindowID，返回 `(是否找到, kCGWindowLayer)`。
    /// 与 scene.rs `frontmost_cg_window_id` 同源遍历（公共 core-graphics API，无需
    /// 触碰生产可见性）。找不到 → `(false, i32::MAX)`（layer 值无意义）。
    fn window_layer_by_id(window_id: usize) -> (bool, i32) {
        use core_foundation::base::{CFType, CFTypeRef, TCFType};
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::number::CFNumber;
        use core_graphics::window::{
            copy_window_info, kCGWindowLayer, kCGWindowListExcludeDesktopElements,
            kCGWindowListOptionOnScreenOnly, kCGWindowNumber,
        };
        use std::ffi::c_void;

        let Some(list) = copy_window_info(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            0,
        ) else {
            return (false, i32::MAX);
        };
        for item in list.iter() {
            let raw: *const c_void = *item;
            let cf_type = unsafe { CFType::wrap_under_get_rule(raw) };
            let dict = match cf_type.downcast::<CFDictionary<*const c_void, *const c_void>>() {
                Some(d) => d,
                None => continue,
            };
            let (key_number, key_layer) = unsafe {
                (
                    kCGWindowNumber as *const c_void,
                    kCGWindowLayer as *const c_void,
                )
            };
            let num_val = |key: *const c_void| {
                dict.find(key)
                    .map(|v| unsafe { CFType::wrap_under_get_rule(*v) })
                    .and_then(|v| v.downcast::<CFNumber>())
                    .and_then(|n| n.to_i32())
            };
            if num_val(key_number) == Some(window_id as i32) {
                return (true, num_val(key_layer).unwrap_or(i32::MAX));
            }
        }
        (false, i32::MAX)
    }

    /// P1: capture_scene_signals_by_id 降级语义（MACOS-P4-SCENE-001 真实现后换锚）。
    ///
    /// 契约（对齐 Windows `capture_scene_signals`）：`id == 0` 会降级为实时查
    /// frontmost（scene.rs:62），有 GUI 时可能返回 `Some((exe, title))`。
    ///
    /// 🔴 为什么不断言 `is_some()`：无 GUI 会话下 frontmost 取不到 → 合法返回 None，
    /// `is_some()` 会随环境随机红。也**不**能断言 `is_none()`（就锁回 stub 行为）。
    /// 锚定为契约本身：**返回 `Some((exe, _))` 时 `exe` 必须非空**——Windows 侧契约是
    /// 「exe 取不到才整体 None，标题取不到给空串」。此条与环境无关且真有判别力：
    /// 能抓住「返回 Some 但 exe 是空串」这种违反契约的实现。
    #[test]
    fn capture_scene_signals_by_id_nonempty_exe_or_none() {
        let result = capture_scene_signals_by_id(0);
        match result {
            None => {} // 合法降级（无 GUI / 取不到 exe）
            Some((exe, _title)) => {
                assert!(
                    !exe.is_empty(),
                    "capture_scene_signals_by_id(0) 返回 Some 但 exe 为空串，违反契约（exe 取不到应整体 None）"
                );
            }
        }
    }
}
