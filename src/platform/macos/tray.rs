//! macOS status bar tray icon (NSStatusItem) implementation.
//!
//! Bypasses `tray-icon` crate which doesn't visibly create status items under our
//! self-managed NSApplication + CFRunLoop event host (verified in MACOS-P4-HOST-001).
//! This module directly uses `NSStatusBar` / `NSStatusItem` / `NSMenu`.
//!
//! Windows 对齐点：
//! - 菜单结构：设置 / 分隔线 / 退出（与 Windows 托盘 1:1）
//! - 文案走 `i18n::get(ui_language)`，不硬编码
//! - 左键点击弹菜单（macOS 惯例），双击图标打开设置
//! - 退出复用 `platform::request_stop()` 链（MACOS-P4-EXIT-001）

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{declare_class, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use objc2_app_kit::{NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use objc2_foundation::{MainThreadMarker, NSData, NSString};
use std::ffi::c_void;
use std::io::Cursor;
use std::sync::OnceLock;

use crate::config::UiLanguage;
use crate::i18n;
use crate::ui::tray::TrayState;

/// Commands that can be sent from the tray menu / status item back to the controller.
#[derive(Debug, Clone, Copy)]
pub enum TrayCommand {
    OpenSettings,
    Exit,
}

/// A pending tray state update requested from any thread, applied on the main
/// thread by `poll_pending_tray_states` (called from the 15ms CFRunLoop timer).
#[derive(Debug, Clone, Copy)]
struct TrayStateUpdate {
    state: TrayState,
    ui_language: UiLanguage,
}

static TRAY_STATE_CHANNEL: OnceLock<(
    Sender<TrayStateUpdate>,
    crossbeam_channel::Receiver<TrayStateUpdate>,
)> = OnceLock::new();

// 主线程 tray 单例。TRAY-FIX-001: 原 `TRAY_HANDLE` 裸指针存 `&tray`（main.rs 局部变量），
// build_tray 返回后 tray 被 move 进 Option，指针立即悬垂 → 15ms timer 每秒 66 次解引用
// 野指针，objc2 msg_send_check 断言失败即 SIGABRT（Gavin 端测崩溃）。
// 改为 thread_local 持有所有权（与 overlay.rs 的 `OVERLAY: RefCell<Option<_>>` 完全同构，
// 那套已验证可用）：天然保证「只有主线程能碰」，规避 `Retained<NSStatusItem>` 非 Send/Sync
// 的静态存储限制，且所有权随 thread_local 生命周期托管，不存在悬垂窗口。
thread_local! {
    static TRAY: std::cell::RefCell<Option<StatusBarTray>> = const { std::cell::RefCell::new(None) };
}

/// Handle to the macOS status bar item. Dropping it removes the icon.
pub struct StatusBarTray {
    status_item: Retained<NSStatusItem>,
    _bridge: Retained<TrayBridge>,
}

impl Drop for StatusBarTray {
    fn drop(&mut self) {
        unsafe {
            NSStatusBar::systemStatusBar().removeStatusItem(&self.status_item);
        }
    }
}

/// Build and show the status bar tray icon.
///
/// Must be called on the main thread (NSStatusBar requirement).
pub fn build_tray(
    ui_language: UiLanguage,
    command_tx: Sender<TrayCommand>,
) -> Result<StatusBarTray> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| anyhow!("build_tray must be called on the main thread"))?;

    let bridge = TrayBridge::new(command_tx, mtm);

    let status_item =
        unsafe { NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength) };

    // Tooltip + icon. `NSStatusItem` deprecated setters mirror the button's,
    // and are exposed on the generated binding directly.
    let tooltip = TrayState::Idle.tooltip(ui_language);
    let icon = tray_state_icon(TrayState::Idle, mtm);
    unsafe {
        status_item.setToolTip(Some(&NSString::from_str(tooltip)));
        status_item.setImage(Some(&icon));
        // Menu on left/right click is automatic once `menu` is set.
        status_item.setTarget(Some(&*bridge));
        // Double-click opens settings.
        status_item.setDoubleAction(Some(sel!(openSettings:)));
    }

    // Menu: Settings / Separator / Exit.
    let menu = build_tray_menu(ui_language, &bridge, mtm);
    unsafe {
        status_item.setMenu(Some(&menu));
        status_item.setVisible(true);
    }

    // Initialize the state channel used by the main-thread poller.
    TRAY_STATE_CHANNEL.get_or_init(|| crossbeam_channel::unbounded());

    log::info!("macOS status bar tray created (NSStatusItem + menu)");

    Ok(StatusBarTray {
        status_item,
        _bridge: bridge,
    })
}

/// 将建好的 tray 交予主线程 thread_local 持有。所有权随 thread_local 托管，
/// 规避裸指针悬垂（TRAY-FIX-001）。必须主线程调用（build_tray 同为要求）。
pub fn set_tray(tray: StatusBarTray) {
    TRAY.with(|cell| {
        if cell.borrow().is_some() {
            log::warn!("macOS tray: set_tray called with an existing tray, replacing it");
        }
        *cell.borrow_mut() = Some(tray);
    });
}

/// 主线程退出前显式销毁 tray（run_controller_macos shutdown 段调用）。
/// thread_local 在进程退出时本会 drop，但显式调用保证退出链清晰、日志可辨
/// （与 overlay.rs `shutdown_overlay` 同构）。
pub fn shutdown_tray() {
    TRAY.with(|cell| {
        if cell.borrow_mut().take().is_some() {
            log::info!("macOS tray destroyed on shutdown");
        }
    });
}

/// Request a tray state update from any thread. Applied on the main thread by
/// `poll_pending_tray_states` (called from the 15ms CFRunLoop timer).
pub fn request_tray_state(state: TrayState, ui_language: UiLanguage) {
    if let Some((tx, _)) = TRAY_STATE_CHANNEL.get() {
        let _ = tx.send(TrayStateUpdate { state, ui_language });
    }
}

/// Drain pending tray state updates and apply them to the tray.
///
/// Must be called on the main thread. Called from the CFRunLoop timer callback.
pub fn poll_pending_tray_states() {
    let Some((_, rx)) = TRAY_STATE_CHANNEL.get() else {
        return;
    };
    TRAY.with(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(tray) = slot.as_mut() else {
            // Tray not registered yet; drain so we don't apply stale states later.
            while rx.try_recv().is_ok() {}
            return;
        };
        while let Ok(update) = rx.try_recv() {
            set_tray_state(tray, update.state, update.ui_language);
        }
    });
}

/// Update tooltip and icon for the given tray state.
pub fn set_tray_state(tray: &StatusBarTray, state: TrayState, ui_language: UiLanguage) {
    let mtm = match MainThreadMarker::new() {
        Some(m) => m,
        None => {
            log::warn!("set_tray_state must be called on the main thread");
            return;
        }
    };

    let icon = tray_state_icon(state, mtm);
    unsafe {
        tray.status_item
            .setToolTip(Some(&NSString::from_str(state.tooltip(ui_language))));
        tray.status_item.setImage(Some(&icon));
    }
}

fn build_tray_menu(
    ui_language: UiLanguage,
    bridge: &Retained<TrayBridge>,
    mtm: MainThreadMarker,
) -> Retained<NSMenu> {
    let strings = i18n::get(ui_language);
    let menu = NSMenu::new(mtm);

    let settings = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc::<NSMenuItem>(),
            &NSString::from_str(strings.tray_menu_settings),
            Some(sel!(openSettings:)),
            &NSString::from_str(""),
        )
    };
    unsafe {
        settings.setTarget(Some(&**bridge));
    }
    menu.addItem(&settings);

    let separator = NSMenuItem::separatorItem(mtm);
    menu.addItem(&separator);
    let exit = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc::<NSMenuItem>(),
            &NSString::from_str(strings.tray_menu_exit),
            Some(sel!(exit:)),
            &NSString::from_str(""),
        )
    };
    unsafe {
        exit.setTarget(Some(&**bridge));
    }
    menu.addItem(&exit);

    menu
}

// ============================================================================
// Icon generation (macOS NSImage from RGBA, aligned to Windows tray colors)
// ============================================================================

fn tray_state_icon(state: TrayState, _mtm: MainThreadMarker) -> Retained<objc2_app_kit::NSImage> {
    // Reproduce Windows tray.rs color semantics per state.
    let color: [u8; 3] = match state {
        TrayState::Idle => [0xFF, 0x6B, 0x35],
        TrayState::Recording => [0xFF, 0x3B, 0x30],
        TrayState::Processing => [0xFF, 0xA5, 0x00],
        TrayState::Error => [0xFF, 0x33, 0x33],
    };

    let (width, height, rgba) = make_tray_icon_rgba(color);
    nsimage_from_rgba(width, height, &rgba).unwrap_or_else(|| fallback_image())
}

fn make_tray_icon_rgba(color: [u8; 3]) -> (u32, u32, Vec<u8>) {
    const S: u32 = 32;
    let mut px = vec![0u8; (S * S * 4) as usize];

    for y in 0..S {
        for x in 0..S {
            let i = ((y * S + x) * 4) as usize;
            let (fx, fy) = (x as f32 - 15.5, y as f32 - 15.5);
            if fx * fx + fy * fy > 225.0 {
                continue;
            }

            px[i] = color[0];
            px[i + 1] = color[1];
            px[i + 2] = color[2];
            px[i + 3] = 0xFF;

            let mx = x as i32 - 16;
            let my = y as i32 - 16;
            let in_body = mx.abs() <= 3 && (-7..=3).contains(&my);
            let in_cap = ((mx * mx + (my + 7) * (my + 7)) as f32).sqrt() <= 3.5 && my <= -7;
            let in_stand = mx == 0 && (3..=8).contains(&my);
            let in_base = mx.abs() <= 4 && my == 8;

            if in_body || in_cap || in_stand || in_base {
                px[i] = 0xFF;
                px[i + 1] = 0xFF;
                px[i + 2] = 0xFF;
                px[i + 3] = 0xFF;
            }
        }
    }

    (S, S, px)
}

fn nsimage_from_rgba(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Option<Retained<objc2_app_kit::NSImage>> {
    // Encode RGBA as PNG in memory, then load via NSImage::initWithData.
    // This avoids needing NSBitmapImageRep / NSImageRep features.
    let mut png_buf = Vec::new();
    {
        let mut cursor = Cursor::new(&mut png_buf);
        if image::write_buffer_with_format(
            &mut cursor,
            rgba,
            width,
            height,
            image::ExtendedColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .is_err()
        {
            return None;
        }
    }

    unsafe {
        let data =
            NSData::dataWithBytes_length(png_buf.as_ptr() as *mut c_void, png_buf.len() as _);
        objc2_app_kit::NSImage::initWithData(objc2_app_kit::NSImage::alloc(), &data)
    }
}

fn fallback_image() -> Retained<objc2_app_kit::NSImage> {
    // Solid 16x16 orange square as ultimate fallback.
    let (w, h, rgba) = make_tray_icon_rgba([0xFF, 0x6B, 0x35]);
    nsimage_from_rgba(w, h, &rgba).expect("fallback NSImage must work")
}

// ============================================================================
// Objective-C bridge object that forwards menu actions to Rust channel
// ============================================================================

struct TrayBridgeIvars {
    tx: Sender<TrayCommand>,
}

declare_class!(
    struct TrayBridge;

    // SAFETY:
    // - Superclass NSObject has no subclassing requirements.
    // - Interior mutability is a safe default; `Sender` is `Send + Sync`.
    // - `TrayBridge` does not implement `Drop`.
    unsafe impl ClassType for TrayBridge {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "FeiyinTrayBridge";
    }

    impl DeclaredClass for TrayBridge {
        type Ivars = TrayBridgeIvars;
    }

    unsafe impl TrayBridge {
        #[method(openSettings:)]
        fn open_settings(&self, _sender: *mut objc2::runtime::AnyObject) {
            self.ivars().tx.send(TrayCommand::OpenSettings).ok();
        }

        #[method(exit:)]
        fn exit(&self, _sender: *mut objc2::runtime::AnyObject) {
            self.ivars().tx.send(TrayCommand::Exit).ok();
        }
    }
);

impl TrayBridge {
    fn new(tx: Sender<TrayCommand>, _mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc().set_ivars(TrayBridgeIvars { tx });
        unsafe { msg_send_id![super(this), init] }
    }
}
