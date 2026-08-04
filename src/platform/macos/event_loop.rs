//! macOS Event Loop Implementation
//!
//! Provides a hidden controller host for tray-first architecture on macOS using
//! NSApplication (accessory activation policy, no Dock icon) + CFRunLoop.
//!
//! Windows 侧对应物：
//! - 隐藏 controller 窗口 -> NSApplication with .accessory
//! - GetMessageW 消息泵   -> CFRunLoopRunInMode
//! - 15ms SetTimer        -> 15ms CFRunLoopTimer
//! - WM_APP_* 自定义消息  -> CFRunLoopSource (signal + wake up)
//! - PostMessageW 唤醒    -> CFRunLoopSourceSignal + CFRunLoopWakeUp

use anyhow::{anyhow, Result};
use core_foundation_sys::base::kCFAllocatorDefault;
use core_foundation_sys::runloop::{
    kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoopAddSource, CFRunLoopAddTimer,
    CFRunLoopGetCurrent, CFRunLoopRef, CFRunLoopRunInMode, CFRunLoopSourceContext,
    CFRunLoopSourceCreate, CFRunLoopSourceRef, CFRunLoopSourceSignal, CFRunLoopStop,
    CFRunLoopTimerContext, CFRunLoopTimerCreate, CFRunLoopTimerRef, CFRunLoopWakeUp,
};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::MainThreadMarker;
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;

use crate::platform::macos::hotkey::{HotkeyEvent, HotkeyListener};

const CONTROLLER_TIMER_INTERVAL_SECS: f64 = 0.015; // 15ms, align with Windows SetTimer

static RUN_LOOP: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static WAKE_SOURCE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Context passed to the timer callback so it can poll the hotkey listener.
struct ControllerContext {
    hotkey_rx: crossbeam_channel::Receiver<HotkeyEvent>,
    stop_signal: Arc<AtomicBool>,
}

static CONTROLLER_CTX: AtomicPtr<ControllerContext> = AtomicPtr::new(ptr::null_mut());

/// Create the controller host.
///
/// On macOS we do not create a physical window; instead we initialize NSApplication
/// with `accessory` activation policy so the process stays resident without appearing
/// in the Dock or Cmd+Tab switcher. This is the macOS equivalent of Windows'
/// hidden `WS_EX_TOOLWINDOW` controller window.
pub fn create_controller_window() -> Result<()> {
    let _mtm = MainThreadMarker::new()
        .ok_or_else(|| anyhow!("create_controller_window must be called on the main thread"))?;
    let app = NSApplication::sharedApplication(_mtm);
    // Accessory policy: tray-first, no main window, no Dock icon.
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    // Finish launching so AppKit services (e.g. tray-icon, status bar) are available.
    unsafe { app.finishLaunching() };
    log::info!("macOS controller host initialized (NSApplication accessory)");
    Ok(())
}

/// Destroy the controller host.
pub fn destroy_controller_window() -> Result<()> {
    request_stop();
    log::info!("macOS controller host destroyed");
    Ok(())
}

/// Run the CFRunLoop-based message pump.
///
/// - Installs a custom CFRunLoopSource used to wake the loop from other threads.
/// - Installs a 15ms CFRunLoopTimer that polls the hotkey listener and logs events.
/// - Runs until `request_stop()` is called.
pub fn run_message_loop() -> Result<()> {
    // Keep the original accessibility check from the stub.
    crate::platform::macos::accessibility::ensure_accessibility_at_startup()?;

    let run_loop = unsafe { CFRunLoopGetCurrent() };
    if run_loop.is_null() {
        return Err(anyhow!("CFRunLoopGetCurrent returned null"));
    }
    RUN_LOOP.store(run_loop as *mut c_void, Ordering::Release);

    // Custom source: only used as a wake-up signal (perform is a no-op).
    let mut source_ctx = CFRunLoopSourceContext {
        version: 0,
        info: ptr::null_mut(),
        retain: None,
        release: None,
        copyDescription: None,
        equal: None,
        hash: None,
        schedule: None,
        cancel: None,
        perform: wake_source_perform,
    };
    let source = unsafe { CFRunLoopSourceCreate(kCFAllocatorDefault, 0, &mut source_ctx) };
    if source.is_null() {
        return Err(anyhow!("CFRunLoopSourceCreate returned null"));
    }
    WAKE_SOURCE.store(source as *mut c_void, Ordering::Release);
    unsafe {
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
    }

    // 15ms timer to poll hotkey events, matching Windows WM_TIMER interval.
    let mut timer_ctx = CFRunLoopTimerContext {
        version: 0,
        info: ptr::null_mut(),
        retain: None,
        release: None,
        copyDescription: None,
    };
    let timer = unsafe {
        CFRunLoopTimerCreate(
            kCFAllocatorDefault,
            0.0, // fire date in the past => fires immediately
            CONTROLLER_TIMER_INTERVAL_SECS,
            0,
            0,
            controller_timer_callback,
            &mut timer_ctx,
        )
    };
    if timer.is_null() {
        return Err(anyhow!("CFRunLoopTimerCreate returned null"));
    }
    unsafe {
        CFRunLoopAddTimer(run_loop, timer, kCFRunLoopDefaultMode);
    }

    log::info!("macOS message loop started (CFRunLoop, 15ms timer)");

    // Run the loop. Return every 250ms to check STOP_REQUESTED, then re-enter.
    while !STOP_REQUESTED.load(Ordering::Acquire) {
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.25, 0);
        }
    }

    log::info!("macOS message loop stopping");

    unsafe {
        CFRunLoopStop(run_loop);
    }
    RUN_LOOP.store(ptr::null_mut(), Ordering::Release);
    WAKE_SOURCE.store(ptr::null_mut(), Ordering::Release);
    Ok(())
}

/// Run the loop with a hotkey listener attached to the timer callback.
///
/// This is the macOS equivalent of Windows `run_controller`; it wires the
/// `HotkeyListener` receiver into the controller context so the 15ms timer
/// can poll and log events.
pub fn run_message_loop_with_hotkey_listener(listener: &HotkeyListener) -> Result<()> {
    let ctx = Box::new(ControllerContext {
        hotkey_rx: listener.rx().clone(),
        stop_signal: Arc::new(AtomicBool::new(false)),
    });
    let ctx_ptr = Box::into_raw(ctx);
    CONTROLLER_CTX.store(ctx_ptr, Ordering::Release);

    let result = run_message_loop();

    CONTROLLER_CTX.store(ptr::null_mut(), Ordering::Release);
    unsafe {
        let _ = Box::from_raw(ctx_ptr);
    }
    result
}

/// Wake the run loop. Called from other threads when something needs attention.
pub fn notify_config_changed() {
    if let Some(source) = wake_source_ref() {
        unsafe { CFRunLoopSourceSignal(source) };
    }
    if let Some(run_loop) = run_loop_ref() {
        unsafe { CFRunLoopWakeUp(run_loop) };
    }
}

/// Request the run loop to stop. Safe to call from any thread and from signal handlers.
pub fn request_stop() {
    STOP_REQUESTED.store(true, Ordering::Release);
    notify_config_changed();
}

extern "C" fn wake_source_perform(_info: *const c_void) {
    // No-op: the source only exists to wake CFRunLoop via Signal+WakeUp.
}

extern "C" fn controller_timer_callback(_timer: CFRunLoopTimerRef, _info: *mut c_void) {
    // TRAY-001: apply pending tray state updates (requested from any thread).
    crate::platform::macos::tray::poll_pending_tray_states();

    let ctx_ptr = CONTROLLER_CTX.load(Ordering::Acquire);
    if ctx_ptr.is_null() {
        return;
    }
    let ctx = unsafe { &*ctx_ptr };

    // Poll hotkey events without blocking (same semantic as Windows WM_TIMER).
    while let Ok(event) = ctx.hotkey_rx.try_recv() {
        log::info!("macOS controller received HotkeyEvent: {:?}", event);
    }

    // If stop was requested externally (e.g. signal handler), the outer loop will notice.
}

fn run_loop_ref() -> Option<CFRunLoopRef> {
    let p = RUN_LOOP.load(Ordering::Acquire);
    if p.is_null() {
        None
    } else {
        Some(p as CFRunLoopRef)
    }
}

fn wake_source_ref() -> Option<CFRunLoopSourceRef> {
    let p = WAKE_SOURCE.load(Ordering::Acquire);
    if p.is_null() {
        None
    } else {
        Some(p as CFRunLoopSourceRef)
    }
}
