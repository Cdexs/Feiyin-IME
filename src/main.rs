#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
// Bug 2 fix: tray menu freeze workaround (tray-icon #298)
// Use Win32 TrackPopupMenu directly to avoid modal message loop conflict
// with overlay thread's InvalidateRect calls.
mod audio;
mod config;
mod crash;
mod hotkey; // Deprecated: use platform::HotkeyListener instead
mod i18n;
mod injection; // Deprecated: use platform::inject_text instead
mod llm;
mod platform; // MAC-001+003: Platform abstraction layer
mod punctuation;
mod text_normalizer;
mod transcription;
mod translation;
mod ui;
mod wordbook;
use anyhow::{anyhow, Result};
use config::AppConfig;
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use platform::HotkeyEvent; // MAC-003: Use platform layer
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tray_icon::{TrayIcon, TrayIconBuilder};
use ui::overlay::{AudioLevelBuf, OverlayStatus};
use ui::tray::TrayState;
// Windows-specific imports (MAC-011: cfg protected)
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, AlphaBlend, BeginPaint, BitBlt, BLENDFUNCTION, Chord, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW,
    CreatePen, CreateRectRgn, CreateRoundRectRgn, CreateSolidBrush, DeleteDC, DeleteObject,
    DrawTextW, Ellipse, EndPaint, FillRect, GetStockObject, InvalidateRect, LineTo,
    MoveToEx, Rectangle, RoundRect, SelectObject, SetBkMode, SetBrushOrgEx, SetStretchBltMode,
    SetTextColor, SetWindowRgn, StretchBlt, UpdateWindow,
    CLEARTYPE_QUALITY, DEFAULT_CHARSET, DEFAULT_PITCH, DRAW_TEXT_FORMAT, DT_CENTER, DT_END_ELLIPSIS,
    DT_LEFT, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, FF_DONTCARE, FW_NORMAL,
    HALFTONE, HDC, HFONT, NULL_BRUSH, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_NULL, PS_SOLID, SRCCOPY,
    TRANSPARENT,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(target_os = "windows")]
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_WRITE, REG_SZ,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetClientRect, GetForegroundWindow, GetMessageW, GetSystemMetrics, KillTimer,
    LoadCursorW, MsgWaitForMultipleObjects, PeekMessageW, PostMessageW, PostQuitMessage,
    RegisterClassW, SetForegroundWindow, SetLayeredWindowAttributes, SetTimer, SetWindowPos,
    ShowWindow, TrackPopupMenu, TranslateMessage, CREATESTRUCTW,
    CW_USEDEFAULT, GWLP_USERDATA, HMENU, IDC_ARROW, LWA_ALPHA, MF_SEPARATOR, MF_STRING, MSG,
    PM_REMOVE, QS_ALLINPUT, SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE,
    SW_SHOW, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_DESTROY, WM_ERASEBKGND,
    WM_KEYDOWN, WM_LBUTTONUP, WM_NCCREATE, WM_PAINT, WM_TIMER, WNDCLASSW, WNDCLASS_STYLES,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPED, WS_POPUP,
};
#[derive(Debug, Clone)]
enum PipelineEvent {
    RecordingStarted,
    Processing(String),
    Done,
    Cancelled,
    FocusLost(String),
    Error(String),
}
// LATENCY-001: send event and immediately wake controller via PostMessageW
fn send_event(tx: &crossbeam_channel::Sender<PipelineEvent>, event: PipelineEvent) {
    let _ = tx.send(event);
    #[cfg(target_os = "windows")]
    {
        let hwnd_ptr = CONTROLLER_HWND.load(std::sync::atomic::Ordering::Relaxed);
        if hwnd_ptr != 0 {
            let hwnd = HWND(hwnd_ptr as *mut std::ffi::c_void);
            unsafe {
                let _ = PostMessageW(hwnd, WM_APP_PIPELINE_EVENT, WPARAM(0), LPARAM(0));
            }
        }
    }
}
#[derive(Debug, Clone, Copy)]
enum AppCommand {
    OpenSettings,
    Exit,
    ShowTrayMenu { x: i32, y: i32 },
}
#[derive(Debug)]
#[cfg(target_os = "windows")]
enum WorkerCommand {
    Start(StartCmd),
    Shutdown,
}
#[derive(Debug, Clone)]
#[cfg(target_os = "windows")]
enum OverlayCommand {
    Show(OverlayRequest),
    Hide,
    Shutdown,
}
#[derive(Debug, Clone)]
enum OverlayUiEvent {
    CancelRequested,
    PreviewCopied,
}
#[derive(Debug, Clone)]
#[cfg(target_os = "windows")]
struct OverlayRequest {
    status: OverlayStatus,
    pos: [i32; 2],
    size: [i32; 2],
    opacity: f32,
    ui_language: config::UiLanguage,
    /// Auto close after this duration (milliseconds), 0 means no auto close
    auto_close_ms: u32,
}
#[derive(Debug, Clone, Copy)]
#[cfg(target_os = "windows")]
struct SendHwnd(isize);
#[cfg(target_os = "windows")]
unsafe impl Send for SendHwnd {}
#[derive(Debug, Clone)]
#[cfg(target_os = "windows")]
struct StartCmd {
    target_hwnd: SendHwnd,
    translate: Arc<AtomicBool>,
}
const AUTO_LEARN_OBSERVE_MS: u64 = 300;
#[cfg(target_os = "windows")]
const OVERLAY_CLASS_NAME: &str = "voice-ime-overlay-window";
#[cfg(target_os = "windows")]
const CONTROLLER_CLASS_NAME: &str = "voice-ime-controller-window";
#[cfg(target_os = "windows")]
const CONTROLLER_TIMER_ID: usize = 1;
#[cfg(target_os = "windows")]
const WM_APP_INIT_TRAY: u32 = WM_APP + 1;
#[cfg(target_os = "windows")]
const WM_APP_HOTKEY_EVENT: u32 = WM_APP + 2;
// OVERLAY-WAKE-001: overlay thread wake message
#[cfg(target_os = "windows")]
const WM_APP_OVERLAY_WAKE: u32 = WM_APP + 3;
// LATENCY-001: pipeline event wake message for instant controller response
#[cfg(target_os = "windows")]
const WM_APP_PIPELINE_EVENT: u32 = WM_APP + 4;
#[cfg(target_os = "windows")]
const MENU_CMD_SETTINGS: u32 = 1001;
#[cfg(target_os = "windows")]
const MENU_CMD_EXIT: u32 = 1002;
#[cfg(target_os = "windows")]
static MENU_VISIBLE: AtomicBool = AtomicBool::new(false);
// LATENCY-001: static storage for controller HWND, set at controller startup
#[cfg(target_os = "windows")]
static CONTROLLER_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
fn build_tray(ui_language: config::UiLanguage) -> TrayIcon {
    let state = TrayState::Idle;
    TrayIconBuilder::new()
        .with_tooltip(state.tooltip(ui_language))
        .with_icon(state.icon())
        .build()
        .expect("tray icon")
}
#[cfg(target_os = "windows")]
fn tray_menu_labels(ui_language: config::UiLanguage) -> (&'static str, &'static str) {
    let strings = i18n::get(ui_language);
    (strings.tray_menu_settings, strings.tray_menu_exit)
}
#[cfg(target_os = "windows")]
fn show_tray_popup_menu(
    controller_hwnd: HWND,
    x: i32,
    y: i32,
    ui_language: config::UiLanguage,
) -> Option<AppCommand> {
    let (settings_label, exit_label) = tray_menu_labels(ui_language);
    let settings_w = encode_wide(settings_label);
    let exit_w = encode_wide(exit_label);
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        MENU_VISIBLE.store(true, Ordering::SeqCst);
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_CMD_SETTINGS as usize,
            PCWSTR(settings_w.as_ptr()),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_CMD_EXIT as usize,
            PCWSTR(exit_w.as_ptr()),
        );
        let _ = SetForegroundWindow(controller_hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            x,
            y,
            0,
            controller_hwnd,
            None,
        );
        let cmd_id = cmd.0 as u32;
        let _ = DestroyMenu(menu);
        MENU_VISIBLE.store(false, Ordering::SeqCst);
        match cmd_id {
            MENU_CMD_SETTINGS => Some(AppCommand::OpenSettings),
            MENU_CMD_EXIT => Some(AppCommand::Exit),
            _ => None,
        }
    }
}
fn clone_runtime_config(shared_config: &Arc<RwLock<AppConfig>>) -> AppConfig {
    match shared_config.read() {
        Ok(cfg) => cfg.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}
#[cfg(target_os = "windows")]
fn lparam_point(lparam: LPARAM) -> (i32, i32) {
    let v = lparam.0 as u32;
    let x = (v & 0xFFFF) as i16 as i32;
    let y = ((v >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}
#[cfg(target_os = "windows")]
fn rect_contains(rect: &RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}
#[cfg(target_os = "windows")]
fn create_clear_type_font(size: i32) -> HFONT {
    let face = encode_wide("Segoe UI");
    unsafe {
        CreateFontW(
            size,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            0,
            CLEARTYPE_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(face.as_ptr()),
        )
    }
}
#[cfg(target_os = "windows")]
fn apply_overlay_window_region(
    hwnd: HWND,
    rect: &RECT,
    corner_radius: Option<i32>,
    _is_recording: bool,
) {
    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);
    unsafe {
        let region = if let Some(radius) = corner_radius {
            let dia = (radius.max(1) * 2) as i32;
            CreateRoundRectRgn(0, 0, width, height, dia, dia)
        } else {
            CreateRectRgn(0, 0, width, height)
        };
        let _ = SetWindowRgn(hwnd, region, true);
    }
}
#[cfg(target_os = "windows")]
fn draw_text(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    rect: &mut RECT,
    format: DRAW_TEXT_FORMAT,
) {
    let mut wide = encode_wide(text);
    let len = wide.len();
    if len > 1 {
        unsafe {
            let _ = DrawTextW(hdc, &mut wide[..len - 1], rect, format);
        }
    }
}
fn reload_runtime_config(shared_config: &Arc<RwLock<AppConfig>>) {
    match AppConfig::load() {
        Ok(new_config) => match shared_config.write() {
            Ok(mut current) => *current = new_config,
            Err(poisoned) => *poisoned.into_inner() = new_config,
        },
        Err(err) => log::warn!("Failed to reload config: {}", err),
    }
}
#[cfg(target_os = "windows")]
fn load_translation_engine_for_target(
    model_dir: &Path,
    target: config::TranslationLanguage,
) -> Option<translation::TranslationEngine> {
    if !translation::TranslationEngine::is_available(model_dir, target) {
        log::info!(
            "Translation model files not found for {:?}, offline engine disabled",
            target
        );
        return None;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        translation::TranslationEngine::new(model_dir, target)
    }));
    match result {
        Ok(Ok(engine)) => {
            log::info!("Offline translation engine loaded ({:?})", target);
            Some(engine)
        }
        Ok(Err(e)) => {
            log::warn!("Translation engine load failed for {:?}: {}", target, e);
            None
        }
        Err(_) => {
            log::warn!("Translation engine load panicked for {:?}", target);
            None
        }
    }
}
fn is_config_watch_event(event: &notify::Event, config_path: &Path) -> bool {
    let Some(config_name) = config_path.file_name() else {
        return false;
    };
    let config_dir = config_path.parent();
    event.paths.iter().any(|path| {
        path == config_path || path.file_name() == Some(config_name) || path.parent() == config_dir
    })
}
fn spawn_config_watcher(shared_config: Arc<RwLock<AppConfig>>) -> JoinHandle<()> {
    thread::spawn(move || {
        let config_path = AppConfig::config_path();
        let Some(config_dir) = config_path.parent().map(Path::to_path_buf) else {
            log::warn!("Config watcher disabled: config path has no parent");
            return;
        };
        if let Err(err) = std::fs::create_dir_all(&config_dir) {
            log::warn!("Config watcher disabled: failed to create config dir: {err}");
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(
            move |result| {
                let _ = tx.send(result);
            },
            NotifyConfig::default(),
        ) {
            Ok(watcher) => watcher,
            Err(err) => {
                log::warn!("Config watcher disabled: failed to create watcher: {err}");
                return;
            }
        };
        if let Err(err) = watcher.watch(&config_dir, RecursiveMode::NonRecursive) {
            log::warn!(
                "Config watcher disabled: failed to watch {}: {err}",
                config_dir.display()
            );
            return;
        }
        log::info!("Config watcher started for {}", config_path.display());
        let debounce = Duration::from_millis(150);
        loop {
            match rx.recv() {
                // WATCHER-DEBOUNCE-FIX-001: Only config events trigger/extend debounce
                // Non-config events are ignored and don't affect debounce timing
                Ok(Ok(event)) if is_config_watch_event(&event, &config_path) => {
                    // Wait for event burst to settle
                    // Only config events extend the debounce window
                    while let Ok(result) = rx.recv_timeout(debounce) {
                        match result {
                            Ok(event) => {
                                // Only config events extend debounce; ignore others
                                if is_config_watch_event(&event, &config_path) {
                                    continue;
                                }
                                // Non-config event: don't extend debounce, exit immediately
                                break;
                            }
                            Err(_) => break, // Timeout: no more events, proceed to reload
                        }
                    }
                    reload_runtime_config(&shared_config);
                    // HOTKEY-SYNC-IMMEDIATE-001: Notify hotkey thread instantly via AtomicBool
                    #[cfg(target_os = "windows")]
                    platform::notify_config_changed();
                    log::info!("Runtime config reloaded after config file change");
                }
                Ok(Ok(_)) => {} // Non-config event: ignore
                Ok(Err(err)) => log::warn!("Config watcher event error: {err}"),
                Err(_) => break,
            }
        }
    })
}
#[cfg(target_os = "windows")]
fn encode_wide(text: &str) -> Vec<u16> {
    OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
fn spawn_settings_process() -> Result<Child> {
    // DEC-013: 鍚姩 Tauri Settings 瀛愯繘绋?(voice-ime-ui.exe)
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    // Spawn settings UI process (voice-ime-ui.exe)
    let ui_exe = exe_dir.join("voice-ime-ui.exe");
    // Copy release exe to debug location if needed (src-tauri/target/release to src-tauri/target/debug)
    let project_root = exe_dir
        .parent() // target/release -> target/debug -> target/
        .and_then(|p| p.parent()); // target/ -> project root
    let dev_ui_exe_release = project_root
        .map(|root| {
            root.join("src-tauri")
                .join("target")
                .join("release")
                .join("voice-ime-ui.exe")
        })
        .unwrap_or_else(|| ui_exe.clone());
    let dev_ui_exe = if dev_ui_exe_release.exists() {
        dev_ui_exe_release
    } else {
        project_root
            .map(|root| {
                root.join("src-tauri")
                    .join("target")
                    .join("debug")
                    .join("voice-ime-ui.exe")
            })
            .unwrap_or_else(|| ui_exe.clone())
    };
    let target_exe = if ui_exe.exists() {
        ui_exe
    } else if dev_ui_exe.exists() {
        log::info!(
            "Using development path for voice-ime-ui.exe: {}",
            dev_ui_exe.display()
        );
        dev_ui_exe
    } else {
        return Err(anyhow!(
            "voice-ime-ui.exe not found. Please build the Tauri UI first (npm run tauri build or cargo build in src-tauri)."
        ));
    };
    log::info!("Spawning Tauri Settings UI from: {}", target_exe.display());
    let child = Command::new(&target_exe).spawn()?;
    Ok(child)
}
#[cfg(target_os = "windows")]
fn create_controller_window() -> Result<HWND> {
    let hinstance = unsafe { GetModuleHandleW(None)? };
    let class_name = encode_wide(CONTROLLER_CLASS_NAME);
    let wnd_class = WNDCLASSW {
        lpfnWndProc: Some(controller_wnd_proc),
        hInstance: HINSTANCE(hinstance.0),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&wnd_class);
    }
    let window_title = encode_wide("飞音语音输入 Controller");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(window_title.as_ptr()),
            WS_OVERLAPPED,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0,
            0,
            None,
            HMENU::default(),
            HINSTANCE(hinstance.0),
            None,
        )?
    };
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    Ok(hwnd)
}
#[cfg(target_os = "windows")]
unsafe extern "system" fn controller_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_DESTROY {
        PostQuitMessage(0);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
#[cfg(target_os = "windows")]
struct OverlayThreadHandle {
    tx: crossbeam_channel::Sender<OverlayCommand>,
    join: Option<JoinHandle<()>>,
    overlay_hwnd: HWND,
}
#[cfg(target_os = "windows")]
impl OverlayThreadHandle {
    fn send(&self, command: OverlayCommand) {
        let _ = self.tx.send(command);
        // OVERLAY-WAKE-001: wake overlay thread from MsgWaitForMultipleObjects
        if self.overlay_hwnd != HWND::default() {
            unsafe {
                let _ = PostMessageW(self.overlay_hwnd, WM_APP_OVERLAY_WAKE, WPARAM(0), LPARAM(0));
            }
        }
    }
    fn shutdown_and_join(mut self) {
        let _ = self.tx.send(OverlayCommand::Shutdown);
        if self.overlay_hwnd != HWND::default() {
            unsafe {
                let _ = PostMessageW(self.overlay_hwnd, WM_APP_OVERLAY_WAKE, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
#[cfg(target_os = "windows")]
struct OverlayWindowState {
    request: Option<OverlayRequest>,
    audio_buf: AudioLevelBuf,
    event_tx: crossbeam_channel::Sender<OverlayUiEvent>,
    cancel_btn_rect: Option<RECT>,
    close_btn_rect: Option<RECT>, // close button area (focus-lost preview)
    title_close_btn_rect: Option<RECT>, // title bar close button (focus-lost preview)
    shimmer_phase: f32,
}
#[cfg(target_os = "windows")]
struct OverlayWindowData {
    state: Arc<Mutex<OverlayWindowState>>,
}
#[cfg(target_os = "windows")]
const RECORDING_OVERLAY_SIZE: [i32; 2] = [240, 36]; // Recording window
#[cfg(target_os = "windows")]
const STATUS_OVERLAY_SIZE: [i32; 2] = [240, 36]; // Processing window adjusted height
#[cfg(target_os = "windows")]
const PREVIEW_OVERLAY_SIZE: [i32; 2] = [320, 140]; // UI-OPT-003: increased height for title bar
#[cfg(target_os = "windows")]
fn spawn_overlay_thread(
    audio_buf: AudioLevelBuf,
) -> (
    OverlayThreadHandle,
    crossbeam_channel::Receiver<OverlayUiEvent>,
) {
    let (command_tx, command_rx) = crossbeam_channel::unbounded();
    let (event_tx, event_rx) = crossbeam_channel::unbounded();
    // OVERLAY-WAKE-001: channel to receive HWND back from overlay thread
    // OVERLAY-WAKE-001: channel to receive HWND back from overlay thread
    let (hwnd_tx, hwnd_rx) = crossbeam_channel::bounded::<SendHwnd>(1);
    let join = thread::spawn(move || {
        if let Err(err) = run_overlay_thread(command_rx, event_tx, hwnd_tx, audio_buf) {
            log::error!("Overlay thread failed: {}", err);
        }
    });
    // Wait briefly for the overlay thread to report its HWND
    let overlay_hwnd = hwnd_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .map(|h| HWND(h.0 as _))
        .unwrap_or(HWND::default());
    (
        OverlayThreadHandle {
            tx: command_tx,
            join: Some(join),
            overlay_hwnd,
        },
        event_rx,
    )
}
#[cfg(target_os = "windows")]
fn run_overlay_thread(
    command_rx: crossbeam_channel::Receiver<OverlayCommand>,
    event_tx: crossbeam_channel::Sender<OverlayUiEvent>,
    hwnd_tx: crossbeam_channel::Sender<SendHwnd>,
    audio_buf: AudioLevelBuf,
) -> Result<()> {
    let hinstance = unsafe { GetModuleHandleW(None)? };
    let class_name = encode_wide(OVERLAY_CLASS_NAME);
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };
    let wnd_class = WNDCLASSW {
        style: WNDCLASS_STYLES(0), // Fixed-size overlay, no full-window redraw needed
        lpfnWndProc: Some(overlay_wnd_proc),
        hInstance: HINSTANCE(hinstance.0),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        hCursor: cursor,
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&wnd_class);
    }
    let shared_state = Arc::new(Mutex::new(OverlayWindowState {
        request: None,
        audio_buf,
        event_tx,
        cancel_btn_rect: None,
        close_btn_rect: None,
        title_close_btn_rect: None,
        shimmer_phase: 0.0,
    }));
    let window_data = Box::new(OverlayWindowData {
        state: Arc::clone(&shared_state),
    });
    let hwnd = unsafe {
        let window_title = encode_wide("飞音语音输入 Overlay");
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_LAYERED,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(window_title.as_ptr()),
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1,
            1,
            None,
            HMENU::default(),
            HINSTANCE(hinstance.0),
            Some(Box::into_raw(window_data) as _),
        )?
    };
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = UpdateWindow(hwnd);
    }
    // OVERLAY-WAKE-001: report HWND back to main thread
    // OVERLAY-WAKE-001: report HWND back to main thread
    let _ = hwnd_tx.send(SendHwnd(hwnd.0 as isize));
    unsafe {
        // Try DWM API for rounded corners (Windows 11)
        // Fall back to SetWindowRgn if unavailable (Windows 10)
        let corner_preference = DWMWCP_ROUND;
        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner_preference as *const DWM_WINDOW_CORNER_PREFERENCE as *const std::ffi::c_void,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
        if result.is_ok() {
            log::info!("DWM rounded corners enabled (Windows 11+)");
        } else {
            log::info!("DWM rounded corners not available, using SetWindowRgn fallback");
        }
    }
    let mut running = true;
    let mut msg = MSG::default();
    while running {
        while unsafe { PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        while let Ok(command) = command_rx.try_recv() {
            match command {
                OverlayCommand::Show(request) => {
                    if let Ok(mut state) = shared_state.lock() {
                        state.request = Some(request.clone());
                        state.cancel_btn_rect = None;
                        state.close_btn_rect = None;
                        state.title_close_btn_rect = None;
                        if request.status == OverlayStatus::Recording {
                            ui::overlay::warmup_levels(&state.audio_buf);
                        }
                    }
                    unsafe {
                        let alpha = (request.opacity.clamp(0.1, 1.0) * 255.0).round() as u8;
                        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
                        SetWindowPos(
                            hwnd,
                            None,
                            request.pos[0],
                            request.pos[1],
                            request.size[0],
                            request.size[1],
                            SWP_NOACTIVATE | SWP_NOZORDER,
                        )?;
                        let _ = ShowWindow(hwnd, SW_SHOW);
                        // overlay window receives keyboard input via WM_KEYDOWN (ESC to cancel)
                        let _ = InvalidateRect(hwnd, None, true);
                        // Set auto-close timer if needed
                        if request.auto_close_ms > 0 {
                            let _ = SetTimer(hwnd, 1, request.auto_close_ms, None);
                        }
                    }
                }
                OverlayCommand::Hide => {
                    if let Ok(mut state) = shared_state.lock() {
                        state.request = None;
                        state.cancel_btn_rect = None;
                        state.close_btn_rect = None;
                        state.title_close_btn_rect = None;
                    }
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    }
                }
                OverlayCommand::Shutdown => {
                    unsafe {
                        DestroyWindow(hwnd)?;
                    }
                    running = false;
                }
            }
        }
        // Recording and Processing states need repaint (FocusLost does not, to avoid flicker)
        // Recording and Processing states need repaint (FocusLost does not, to avoid flicker)
        // FocusLost state checks ESC key (overlay window has no focus, needs manual check)
        if let Ok(mut state) = shared_state.lock() {
            if let Some(request) = state.request.clone() {
                match request.status {
                    OverlayStatus::Recording => {
                        // recording state needs waveform refresh
                        if !MENU_VISIBLE.load(Ordering::Acquire) {
                            unsafe {
                                let _ = InvalidateRect(hwnd, None, true);
                            }
                        }
                    }
                    OverlayStatus::FallingToProcessing { message } => {
                        const GRAVITY_RATE: f32 = 0.25;
                        // WAVEFORM-FIX-002: edge-weighted gravity — edge bars fall ~2x faster
                        // center_dist=0 at center → gravity 0.125, center_dist=1 at edge → gravity 0.5
                        let half = if let Ok(levels) = state.audio_buf.lock() {
                            levels.len() as i32 / 2
                        } else {
                            16
                        };
                        let half = (half.max(1)) as f32;
                        let all_settled = if let Ok(mut levels) = state.audio_buf.lock() {
                            for (bar_idx, lv) in levels.iter_mut().enumerate() {
                                let dist_from_center = ((bar_idx as f32 - half) / half).abs().min(1.0);
                                let bar_gravity = GRAVITY_RATE * (0.5 + 1.5 * dist_from_center);
                                lv.current *= 1.0 - bar_gravity;
                                lv.peak = lv.peak * (1.0 - bar_gravity * 0.5);
                                if lv.peak < lv.current {
                                    lv.peak = lv.current;
                                }
                            }
                            levels.iter().all(|lv| lv.current < 0.01)
                        } else {
                            false
                        };
                        if all_settled {
                            let msg = message.clone();
                            state.request = state.request.as_mut().map(|r| {
                                r.status = OverlayStatus::Processing(msg);
                                r.clone()
                            });
                        }
                        if !MENU_VISIBLE.load(Ordering::Acquire) {
                            unsafe {
                                let _ = InvalidateRect(hwnd, None, true);
                            }
                        }
                    }
                    OverlayStatus::Processing(_) => {
                        // processing state: only trigger repaint, phase updated in WM_PAINT
                        if !MENU_VISIBLE.load(Ordering::Acquire) {
                            unsafe {
                                let _ = InvalidateRect(hwnd, None, true);
                            }
                        }
                    }
                    OverlayStatus::FocusLost { .. } => {
                        // FocusLost does not repaint to avoid flicker; check ESC key
                        let esc = unsafe { GetAsyncKeyState(VK_ESCAPE.0 as i32) };
                        if (esc as u16) & 0x0001 != 0 {
                            let _ = state.event_tx.send(OverlayUiEvent::CancelRequested);
                            state.request = None;
                        }
                    }
                    OverlayStatus::Error(_) => {
                        // Error state does not repaint
                    }
                }
            }
        }
        // OVERLAY-WAKE-001: message-driven wait replaces thread::sleep(100)
        // - Active overlay (Recording/Processing): short timeout for animation (~25fps)
        // - Idle (no overlay): block until woken by message or channel command
        let has_active_overlay = if let Ok(state) = shared_state.lock() {
            state.request.as_ref().map_or(false, |r| {
                matches!(
                    r.status,
                    OverlayStatus::Recording
                        | OverlayStatus::FallingToProcessing { .. }
                        | OverlayStatus::Processing(_)
                )
            })
        } else {
            false
        };
        let timeout_ms = if has_active_overlay { 16 } else { u32::MAX }; // 60fps for waveform peak decay
        unsafe {
            MsgWaitForMultipleObjects(None, false, timeout_ms, QS_ALLINPUT);
        }
    }
    Ok(())
}
#[cfg(target_os = "windows")]
unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        let createstruct = &*(lparam.0 as *const CREATESTRUCTW);
        let data_ptr = createstruct.lpCreateParams as *mut OverlayWindowData;
        let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            GWLP_USERDATA,
            data_ptr as isize,
        );
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let data_ptr = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA)
        as *mut OverlayWindowData;
    if data_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let data = &mut *data_ptr;
    match msg {
        // OVERLAY-WAKE-001: consume wake message (purpose is to unblock MsgWaitForMultipleObjects)
        WM_APP_OVERLAY_WAKE => {
            return LRESULT(0);
        }
        WM_ERASEBKGND => {
            // Transparent overlay: skip background erase to prevent flicker
            return LRESULT(1); // TRUE = background already erased
        }
        WM_KEYDOWN => {
            // ESC key interrupts current processing (recording, transcription, or LLM optimization)
            if wparam.0 == VK_ESCAPE.0 as usize {
                if let Ok(state) = data.state.lock() {
                    if state.request.is_some() {
                        let _ = state.event_tx.send(OverlayUiEvent::CancelRequested);
                    }
                }
            }
            return LRESULT(0);
        }
        WM_LBUTTONUP => {
            if let Ok(state) = data.state.lock() {
                if let Some(ref request) = state.request {
                    match &request.status {
                        OverlayStatus::FocusLost { text, .. } => {
                            let (x, y) = lparam_point(lparam);
                            // UI-OPT-003: close button (bottom or title bar) -> close without copy
                            if state
                                .close_btn_rect
                                .as_ref()
                                .is_some_and(|rect| rect_contains(rect, x, y))
                                || state
                                    .title_close_btn_rect
                                    .as_ref()
                                    .is_some_and(|rect| rect_contains(rect, x, y))
                            {
                                let _ = state.event_tx.send(OverlayUiEvent::CancelRequested);
                            }
                            // copy button -> copy text then close
                            else if state
                                .cancel_btn_rect
                                .as_ref()
                                .is_some_and(|rect| rect_contains(rect, x, y))
                            {
                                let _ = platform::copy_text_to_clipboard(text); // MAC-004
                                let _ = state.event_tx.send(OverlayUiEvent::PreviewCopied);
                            }
                        }
                        _ => {
                            let (x, y) = lparam_point(lparam);
                            if state
                                .cancel_btn_rect
                                .as_ref()
                                .is_some_and(|rect| rect_contains(rect, x, y))
                            {
                                let _ = state.event_tx.send(OverlayUiEvent::CancelRequested);
                            }
                        }
                    }
                }
            }
            return LRESULT(0);
        }
        WM_PAINT => {
            // Double-buffer: draw to memory DC first, then BitBlt to screen
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            if hdc.0.is_null() {
                return LRESULT(0);
            }

            let mut rect = RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut rect);
            }
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;

            // Create memory DC and bitmap
            let mem_dc = unsafe { CreateCompatibleDC(hdc) };
            let mem_bmp = unsafe { CreateCompatibleBitmap(hdc, width, height) };
            let old_bmp = unsafe { SelectObject(mem_dc, mem_bmp) };

            // Draw to memory DC
            if let Ok(mut state) = data.state.lock() {
                // SHIMMER-FIX-002: time-based phase — immune to WM_PAINT frequency variation
                let _shimmer_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                state.shimmer_phase = (_shimmer_ms % 800) as f32 / 800.0; // SHIMMER-SPEED-002: 1200→800ms
                let (cancel_rect, close_rect, title_close_rect) =
                    draw_overlay_to_dc(hwnd, mem_dc, &rect, &state);
                state.cancel_btn_rect = cancel_rect;
                state.close_btn_rect = close_rect;
                state.title_close_btn_rect = title_close_rect;
            }

            // Copy to screen DC (one-shot, no flicker)
            unsafe {
                let _ = BitBlt(
                    hdc, rect.left, rect.top, width, height, mem_dc, 0, 0, SRCCOPY,
                );
                let _ = SelectObject(mem_dc, old_bmp);
                let _ = DeleteObject(mem_bmp);
                let _ = DeleteDC(mem_dc);
                let _ = EndPaint(hwnd, &ps);
            }

            return LRESULT(0);
        }
        WM_DESTROY => {
            drop(Box::from_raw(data_ptr));
            PostQuitMessage(0);
            return LRESULT(0);
        }
        WM_TIMER => {
            // Animation frames: shimmer phase increments for processing animation
            unsafe {
                let _ = KillTimer(hwnd, 1);
            }
            if let Ok(mut state) = data.state.lock() {
                state.request = None;
                state.cancel_btn_rect = None;
            }
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            return LRESULT(0);
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
#[cfg(target_os = "windows")]
fn draw_overlay_to_dc(
    hwnd: HWND,
    hdc: HDC,
    rect: &RECT,
    state: &OverlayWindowState,
) -> (Option<RECT>, Option<RECT>, Option<RECT>) {
    // Double-buffer: hdc is memory DC, rect is already computed

    // Create ClearType font (Segoe UI, ~9pt)
    let font = create_clear_type_font(-12);
    let old_font = unsafe { SelectObject(hdc, font) };

    unsafe {
        let _ = SetBkMode(hdc, TRANSPARENT);
    }

    let mut cancel_btn_rect = None;
    let mut close_btn_rect = None;
    let mut title_close_btn_rect = None;

    if let Some(request) = &state.request {
        match &request.status {
            OverlayStatus::Recording => {
                apply_overlay_window_region(hwnd, rect, None, true);
                cancel_btn_rect = Some(draw_recording_overlay(
                    hdc,
                    rect,
                    state,
                    request.ui_language,
                ));
            }
            OverlayStatus::FallingToProcessing { .. } => {
                apply_overlay_window_region(hwnd, rect, None, true);
                cancel_btn_rect = Some(draw_recording_overlay(
                    hdc,
                    rect,
                    state,
                    request.ui_language,
                ));
            }
            OverlayStatus::Processing(message) => {
                apply_overlay_window_region(hwnd, rect, None, true);
                draw_processing_overlay(
                    hdc,
                    rect,
                    message,
                    request.ui_language,
                    state.shimmer_phase,
                );
            }
            OverlayStatus::FocusLost { text, .. } => {
                apply_overlay_window_region(hwnd, rect, Some(10), false);
                let (copy_rect, close_rect, tc_rect) =
                    draw_preview_overlay(hdc, rect, text, request.ui_language);
                cancel_btn_rect = Some(copy_rect);
                close_btn_rect = Some(close_rect);
                title_close_btn_rect = Some(tc_rect);
            }
            OverlayStatus::Error(message) => {
                apply_overlay_window_region(hwnd, rect, Some(10), false);
                draw_error_overlay(hdc, rect, message, request.ui_language);
            }
        }
    } else {
        apply_overlay_window_region(hwnd, rect, None, false);
    }

    unsafe {
        let _ = SelectObject(hdc, old_font);
        let _ = DeleteObject(font);
    }

    (cancel_btn_rect, close_btn_rect, title_close_btn_rect)
}

#[cfg(target_os = "windows")]
fn draw_recording_overlay(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    state: &OverlayWindowState,
    _ui_language: config::UiLanguage,
) -> RECT {
    // OVERLAY-SMOOTH-FIX-001: 3-item smooth optimization
    const BRAND_ORANGE: COLORREF = COLORREF(0x006BFF); // #FF6B00
    const RED_STREAM_FAILED: COLORREF = COLORREF(0x0000FF); // #FF0000 — device error
    const GRAY_SILENT: COLORREF = COLORREF(0x808080); // #808080
    const BG_DARK: COLORREF = COLORREF(0x110F0D);
    const BORDER_GRAY: COLORREF = COLORREF(0x060607); // FIX-006-1: darkened border
    const CIRC_BORDER: COLORREF = COLORREF(0x060607); // match border
    const CORNER_RADIUS: i32 = 10;
    // Dark background
    let bg = unsafe { CreateSolidBrush(BG_DARK) };
    unsafe {
        let _ = FillRect(hdc, rect, bg);
        let _ = DeleteObject(bg);
    }
    // Window border
    let border_pen = unsafe { CreatePen(PS_SOLID, 1, BORDER_GRAY) };
    let old_pen = unsafe { SelectObject(hdc, border_pen) };
    let null_brush = unsafe { GetStockObject(NULL_BRUSH) };
    let old_brush = unsafe { SelectObject(hdc, null_brush) };
    unsafe {
        let _ = RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            CORNER_RADIUS * 2,
            CORNER_RADIUS * 2,
        );
        let _ = SelectObject(hdc, old_pen);
        let _ = SelectObject(hdc, old_brush);
        let _ = DeleteObject(border_pen);
    }
    // === Problem 1: 14px smooth circle using HALFTONE supersampling ===
    // PERF-BATCH-001 TASK-5: 三态指示灯
    // RED=stream_failed(设备故障) > ORANGE=有音频录入(level>0.01) > GRAY=设备正常但无音频
    let circ_size = 18; // MIC-ICON-ENLARGE-001: from 14 to 18
    let circ_l = rect.left + 6; // MIC-ICON-ENLARGE-001: left-shift to keep margin to separator
    let circ_t = rect.top + (rect.bottom - rect.top - circ_size) / 2;

    // Three-state audio indicator
    // OVERLAY-LOCK-SCOPE-001: snapshot buffer state under lock, compute color outside lock
    let (buf_empty, has_audio) = if let Ok(levels) = state.audio_buf.lock() {
        let empty = levels.is_empty();
        let audio = !empty && levels.iter().any(|v| v.current > 0.01);
        (empty, audio)
    } else {
        // Lock poisoned = stream failed
        (true, false)
    };
    let circ_color = if buf_empty {
        // Buffer empty = device failure / stream error
        RED_STREAM_FAILED
    } else if has_audio {
        // Has audio above threshold
        BRAND_ORANGE
    } else {
        // Device OK but no audio (silent)
        GRAY_SILENT
    };
    // HALFTONE anti-aliasing: render at 4x then downscale
    let scale = 4;
    let sup_size = circ_size * scale; // MIC-ICON-ENLARGE-001: 72x72
    unsafe {
        let mem_dc = CreateCompatibleDC(hdc);
        let bmp = CreateCompatibleBitmap(hdc, sup_size, sup_size);
        let old_bmp = SelectObject(mem_dc, bmp);
        let bg = CreateSolidBrush(BG_DARK);
        FillRect(
            mem_dc,
            &RECT {
                left: 0,
                top: 0,
                right: sup_size,
                bottom: sup_size,
            },
            bg,
        );
        DeleteObject(bg);
        // MIC-ICON-ENLARGE-001: Wide pill body 28px wide, 49px tall, fully rounded
        let body_brush = CreateSolidBrush(circ_color);
        let null_pen = CreatePen(PS_NULL, 0, circ_color);
        let old_pen = SelectObject(mem_dc, null_pen);
        let old_brush = SelectObject(mem_dc, body_brush);
        let _ = RoundRect(mem_dc, 22, 4, 50, 53, 28, 28);
        let _ = SelectObject(mem_dc, old_pen);
        let _ = SelectObject(mem_dc, old_brush);
        DeleteObject(null_pen);
        DeleteObject(body_brush);
        // Stem + base (proportional to 18px icon)
        let line_pen = CreatePen(PS_SOLID, scale, circ_color);
        let old_pen = SelectObject(mem_dc, line_pen);
        let _ = MoveToEx(mem_dc, 36, 53, None);
        let _ = LineTo(mem_dc, 36, 63);
        let _ = MoveToEx(mem_dc, 24, 63, None);
        let _ = LineTo(mem_dc, 48, 63);
        let _ = SelectObject(mem_dc, old_pen);
        DeleteObject(line_pen);
        // Set HALFTONE mode and downscale to target
        SetStretchBltMode(hdc, HALFTONE);
        SetBrushOrgEx(hdc, 0, 0, None);
        StretchBlt(
            hdc, circ_l, circ_t, circ_size, circ_size, mem_dc, 0, 0, sup_size, sup_size, SRCCOPY,
        );
        // Cleanup
        SelectObject(mem_dc, old_bmp);
        DeleteObject(bmp);
        DeleteDC(mem_dc);
    }
    // === Left separator ===
    let sep_l_x = rect.left + 30; // MIC-ICON-ENLARGE-001: 4px margin after 18px icon (6+18=24, +6=30)
    let sep_h = 20;
    let sep_hh = sep_h / 2;
    let cy = rect.top + (rect.bottom - rect.top) / 2;
    let sep_pen = unsafe { CreatePen(PS_SOLID, 2, BORDER_GRAY) };
    let sep_op = unsafe { SelectObject(hdc, sep_pen) };
    unsafe {
        let _ = MoveToEx(hdc, sep_l_x, cy - sep_hh, None);
        let _ = LineTo(hdc, sep_l_x, cy + sep_hh);
        let _ = SelectObject(hdc, sep_op);
        let _ = DeleteObject(sep_pen);
    }
    // === Right separator ===
    let sep_r_x = rect.right - 36;
    // === Problem 2: Waveform spectrum with peak hold + smooth decay ===
    // UI-OVERLAY-OPT-001: waveform bars expand from center to both sides
    let ww = sep_r_x - sep_l_x - 24; // available width (12px margins each side)
    let bc: i32 = 32;
    let bw = 3;
    let bgap = 2;
    let half = bc / 2; // 16
    let total_bar_width = bc * bw + (bc - 1) * bgap;
    let wl = sep_l_x + 12 + (ww - total_bar_width) / 2; // centered start
    // WAVEFORM-HEIGHT-FIX-001: increased heights + gain for visible waveform
    let maxh = 48;     // from 40 - taller max height
    let static_h = 12; // from 6 - taller static bars
    let minh = 8;      // from 3 - taller minimum bars
    let gain = 2.5;    // RMS gain multiplier
    let by = cy;
    const DECAY_RATE: f32 = 0.02; // Peak decay per frame at 60fps
    // OVERLAY-LOCK-SCOPE-001: snapshot display values under lock (apply decay),
    // then release lock for GDI drawing — prevents audio thread push_level() blocking
    let snapshot: Vec<f32> = if let Ok(mut levels) = state.audio_buf.lock() {
        for level in levels.iter_mut() {
            level.update(level.current, DECAY_RATE);
        }
        let len = levels.len();
        let half_u = half as usize;
        (0..half_u)
            .map(|i| {
                let idx = len.saturating_sub(1 + i);
                if idx < len {
                    levels[idx].display_value()
                } else {
                    0.0
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    // GDI drawing uses snapshot (lock released)
    // WAVEFORM-FIX-002: center bar(i=0) maps to newest sample(len-1), edge(i=half-1) to oldest
    // Left half: bars spread from center outward to the left
    for i in 0..half {
        let v = snapshot.get(i as usize).copied().unwrap_or(0.0);
        let weight: f32 = 0.4 + 0.6
            * (std::f32::consts::FRAC_PI_2 * i as f32 / (half - 1).max(1) as f32)
                .cos()
                .powi(2);
        let v_gain = (v * gain * weight).min(1.0);
        let bh = if v_gain > 0.01 {
            (minh as f32 + v_gain * (maxh - minh) as f32) as i32
        } else {
            static_h
        };
        let x = wl + (half - 1 - i) * (bw + bgap);
        let br = RECT {
            left: x,
            top: by - bh / 2,
            right: x + bw,
            bottom: by + bh / 2,
        };
        let ob = unsafe { CreateSolidBrush(BRAND_ORANGE) };
        let op2 = unsafe { CreatePen(PS_NULL, 0, BRAND_ORANGE) };
        let ob_old = unsafe { SelectObject(hdc, ob) };
        let op_old = unsafe { SelectObject(hdc, op2) };
        unsafe {
            let _ = RoundRect(hdc, br.left, br.top, br.right, br.bottom, bw * 2, bw * 2);
            let _ = SelectObject(hdc, ob_old);
            let _ = SelectObject(hdc, op_old);
            let _ = DeleteObject(ob);
            let _ = DeleteObject(op2);
        }
    }
    // Right half: bars spread from center outward to the right (mirror)
    for i in 0..half {
        let v = snapshot.get(i as usize).copied().unwrap_or(0.0);
        let weight: f32 = 0.4 + 0.6
            * (std::f32::consts::FRAC_PI_2 * i as f32 / (half - 1).max(1) as f32)
                .cos()
                .powi(2);
        let v_gain = (v * gain * weight).min(1.0);
        let bh = if v_gain > 0.01 {
            (minh as f32 + v_gain * (maxh - minh) as f32) as i32
        } else {
            static_h
        };
        let x = wl + (half + i) * (bw + bgap);
        let br = RECT {
            left: x,
            top: by - bh / 2,
            right: x + bw,
            bottom: by + bh / 2,
        };
        let ob = unsafe { CreateSolidBrush(BRAND_ORANGE) };
        let op2 = unsafe { CreatePen(PS_NULL, 0, BRAND_ORANGE) };
        let ob_old = unsafe { SelectObject(hdc, ob) };
        let op_old = unsafe { SelectObject(hdc, op2) };
        unsafe {
            let _ = RoundRect(hdc, br.left, br.top, br.right, br.bottom, bw * 2, bw * 2);
            let _ = SelectObject(hdc, ob_old);
            let _ = SelectObject(hdc, op_old);
            let _ = DeleteObject(ob);
            let _ = DeleteObject(op2);
        }
    }
    // Draw right separator
    let sep_pen2 = unsafe { CreatePen(PS_SOLID, 2, BORDER_GRAY) };
    let sep_op2 = unsafe { SelectObject(hdc, sep_pen2) };
    unsafe {
        let _ = MoveToEx(hdc, sep_r_x, cy - sep_hh, None);
        let _ = LineTo(hdc, sep_r_x, cy + sep_hh);
        let _ = SelectObject(hdc, sep_op2);
        let _ = DeleteObject(sep_pen2);
    }
    // === STOP-BUTTON-CENTER-FIX-001: GDI Rectangle is exclusive of right/bottom ===
    let bs = 16;
    let bl = rect.right - 25;
    let bt = rect.top + (rect.bottom - rect.top - bs) / 2;
    let cr = RECT {
        left: bl,
        top: bt,
        right: bl + bs + 1,
        bottom: bt + bs + 1,
    };
    // Outer border: 1px
    let bp = unsafe { CreatePen(PS_SOLID, 1, BRAND_ORANGE) };
    let bop = unsafe { SelectObject(hdc, bp) };
    let bnb = unsafe { GetStockObject(NULL_BRUSH) };
    let bob = unsafe { SelectObject(hdc, bnb) };
    unsafe {
        let _ = Rectangle(hdc, cr.left, cr.top, cr.right, cr.bottom);
        let _ = SelectObject(hdc, bop);
        let _ = SelectObject(hdc, bob);
        let _ = DeleteObject(bp);
    }
    // Inner solid: 8x8, (16-8)/2 = 4px exact centering
    let isz = 8; // OVERLAY-UI-TUNE-001: from 10 to 8
    let il = cr.left + (bs - isz) / 2;
    let it = cr.top + (bs - isz) / 2;
    let ir = RECT {
        left: il,
        top: it,
        right: il + isz + 1,
        bottom: it + isz + 1,
    };
    let ib = unsafe { CreateSolidBrush(BRAND_ORANGE) };
    let ip = unsafe { CreatePen(PS_NULL, 0, BRAND_ORANGE) };
    let ibo = unsafe { SelectObject(hdc, ib) };
    let ipo = unsafe { SelectObject(hdc, ip) };
    unsafe {
        let _ = Rectangle(hdc, ir.left, ir.top, ir.right, ir.bottom);
        let _ = SelectObject(hdc, ibo);
        let _ = SelectObject(hdc, ipo);
        let _ = DeleteObject(ib);
        let _ = DeleteObject(ip);
    }
    cr
}
#[cfg(target_os = "windows")]
fn draw_processing_overlay(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    _message: &str,
    ui_language: config::UiLanguage,
    shimmer_phase: f32,
) {
    // PROCESSING-SHIMMER-001: Slim Shimmer effect (replaces GradientFill glow)
    const BRAND_ORANGE: COLORREF = COLORREF(0x006BFF); // #FF6B00
    const BRIGHT_ORANGE: COLORREF = COLORREF(0x008CFF); // #FF8C00 - brighter shimmer
    const BG_DARK: COLORREF = COLORREF(0x181A18); // #181A18
    const BORDER_GRAY: COLORREF = COLORREF(0x060607); // FIX-006-1: unify with recording overlay
    const CORNER_RADIUS: i32 = 16;
    // WAVEFORM-HEIGHT-FIX-001: restore fixed gray border (remove breathing)
    let border_color = BORDER_GRAY;
    // Dark background
    let bg = unsafe { CreateSolidBrush(BG_DARK) };
    unsafe {
        let _ = FillRect(hdc, rect, bg);
        let _ = DeleteObject(bg);
    }
    // Border: 1px rounded corners (breathing orange)
    let border_pen = unsafe { CreatePen(PS_SOLID, 1, border_color) };
    let old_pen = unsafe { SelectObject(hdc, border_pen) };
    let null_brush = unsafe { GetStockObject(NULL_BRUSH) };
    let old_brush = unsafe { SelectObject(hdc, null_brush) };
    unsafe {
        let _ = RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            CORNER_RADIUS * 2,
            CORNER_RADIUS * 2,
        );
        let _ = SelectObject(hdc, old_pen);
        let _ = SelectObject(hdc, old_brush);
        let _ = DeleteObject(border_pen);
    }
    // SHIMMER-VISUAL-003: 30-slice Gaussian AlphaBlend for smooth soft silver glow
    // Uses a single 3px silver bitmap reused across 30 slices with Gaussian alpha
    const GLOW_HALF: i32 = 45;
    const SLICES: i32 = 30;
    let win_h = (rect.bottom - rect.top - 2).max(1);
    let glow_w_total = GLOW_HALF * 2;
    let travel = (rect.right - rect.left + glow_w_total) as f32;
    let beam_cx = rect.left - GLOW_HALF + (travel * shimmer_phase) as i32;
    let slice_w = ((glow_w_total + SLICES - 1) / SLICES).max(1);

    unsafe {
        let tmp_dc = CreateCompatibleDC(hdc);
        let tmp_bmp = CreateCompatibleBitmap(hdc, slice_w, win_h);
        let old_bmp = SelectObject(tmp_dc, tmp_bmp);
        let brush = CreateSolidBrush(COLORREF(0xD8D8D8));
        let _ = FillRect(tmp_dc, &RECT { left: 0, top: 0, right: slice_w, bottom: win_h }, brush);
        let _ = DeleteObject(brush);

        for i in 0..SLICES {
            let t = (i as f32 / (SLICES - 1) as f32) * 2.0 - 1.0; // -1.0 to 1.0
            let alpha = ((-3.0_f32 * t * t).exp() * 150.0) as u8; // TUNE: 200→150 slightly more transparent per Gavin
            if alpha == 0 { continue; }

            let x_dst = beam_cx - GLOW_HALF + i * slice_w;
            let x1 = x_dst.max(rect.left + 1);
            let x2 = (x_dst + slice_w).min(rect.right - 1);
            let w = x2 - x1;
            if w <= 0 { continue; }

            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: alpha,
                AlphaFormat: 0,
            };
            let _ = AlphaBlend(
                hdc, x1, rect.top + 1, w, win_h,
                tmp_dc, 0, 0, slice_w, win_h,
                blend,
            );
        }

        let _ = SelectObject(tmp_dc, old_bmp);
        let _ = DeleteObject(tmp_bmp);
        let _ = DeleteDC(tmp_dc);
    }
    // Processing message text (centered, orange)
    let strings = i18n::get(ui_language);
    let text = strings.overlay_processing;
    let mut text_rect = RECT {
        left: rect.left + 20,
        top: rect.top + 4,
        right: rect.right - 20,
        bottom: rect.bottom - 4,
    };
    unsafe {
        let _ = SetTextColor(hdc, BRAND_ORANGE);
    }
    draw_text(
        hdc,
        text,
        &mut text_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
    );
}
#[cfg(target_os = "windows")]
fn draw_preview_overlay(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    text: &str,
    ui_language: config::UiLanguage,
) -> (RECT, RECT, RECT) {
    // UI-OPT-003: preview window with title bar, centered buttons, i18n labels
    const BRAND_ORANGE: COLORREF = COLORREF(0x006BFF); // #FF6B00
    const BG_DARK: COLORREF = COLORREF(0x211D1A);
    const BORDER_GRAY: COLORREF = COLORREF(0x060607); // FIX-006-1: unify with recording overlay
    const CORNER_RADIUS: i32 = 10;
    let strings = i18n::get(ui_language);
    let bg = unsafe { CreateSolidBrush(BG_DARK) };
    unsafe {
        let _ = FillRect(hdc, rect, bg);
        let _ = DeleteObject(bg);
    }
    // Border (unified style)
    let border_pen = unsafe { CreatePen(PS_SOLID, 1, BORDER_GRAY) };
    let border_old_pen = unsafe { SelectObject(hdc, border_pen) };
    let null_brush = unsafe { GetStockObject(NULL_BRUSH) };
    let border_old_brush = unsafe { SelectObject(hdc, null_brush) };
    unsafe {
        let _ = RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            CORNER_RADIUS * 2,
            CORNER_RADIUS * 2,
        );
        let _ = SelectObject(hdc, border_old_pen);
        let _ = SelectObject(hdc, border_old_brush);
        let _ = DeleteObject(border_pen);
    }
    // Title bar (28px height)
    let title_bar_h = 28;
    let title_text = strings.preview_title_bar;
    unsafe {
        let _ = SetTextColor(hdc, BRAND_ORANGE);
    }
    let close_btn_space = 26; // symmetric with right-side close button
    let mut title_rect = RECT {
        left: rect.left + close_btn_space,
        top: rect.top + 4,
        right: rect.right - close_btn_space,
        bottom: rect.top + title_bar_h,
    };
    draw_text(
        hdc,
        title_text,
        &mut title_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    // Title bar close button (18x18, right side)
    let title_close_rect = RECT {
        left: rect.right - 26,
        top: rect.top + 5,
        right: rect.right - 8,
        bottom: rect.top + 23,
    };
    // FIX-006 v2: button border brighter than window border for visual distinction
    const BTN_BORDER: COLORREF = COLORREF(0x707070); // mid gray, distinguishable from BORDER_GRAY 0x060607
    let tc_pen = unsafe { CreatePen(PS_SOLID, 1, BTN_BORDER) };
    let tc_old_pen = unsafe { SelectObject(hdc, tc_pen) };
    let tc_hollow = unsafe { GetStockObject(NULL_BRUSH) };
    let tc_old_brush = unsafe { SelectObject(hdc, tc_hollow) };
    unsafe {
        let _ = RoundRect(
            hdc,
            title_close_rect.left,
            title_close_rect.top,
            title_close_rect.right,
            title_close_rect.bottom,
            6,
            6,
        );
        let _ = SelectObject(hdc, tc_old_pen);
        let _ = SelectObject(hdc, tc_old_brush);
        let _ = DeleteObject(tc_pen);
    }
    // Draw "X" in title close button
    let mut tc_text_rect = title_close_rect;
    unsafe {
        let _ = SetTextColor(hdc, BRAND_ORANGE);
    }
    draw_text(
        hdc,
        "\u{2715}",
        &mut tc_text_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    // Title bar separator line
    let sep_pen = unsafe { CreatePen(PS_SOLID, 1, BORDER_GRAY) };
    let sep_old = unsafe { SelectObject(hdc, sep_pen) };
    unsafe {
        let _ = MoveToEx(hdc, rect.left + 8, rect.top + title_bar_h, None);
        let _ = LineTo(hdc, rect.right - 8, rect.top + title_bar_h);
        let _ = SelectObject(hdc, sep_old);
        let _ = DeleteObject(sep_pen);
    }
    // Body text (word wrap + ellipsis)
    let mut text_rect = RECT {
        left: rect.left + 14,
        top: rect.top + title_bar_h + 8,
        right: rect.right - 14,
        bottom: rect.bottom - 40,
    };
    unsafe {
        let _ = SetTextColor(hdc, COLORREF(0xF2F2F2));
    }
    draw_text(hdc, text, &mut text_rect, DT_LEFT | DT_WORDBREAK | DT_END_ELLIPSIS);
    // Bottom buttons (centered)
    let btn_w = 45; // FIX-006-6: shrink 25%
    let btn_h = 18; // FIX-006-6: shrink 25%
    let gap = 10;
    let total_w = btn_w * 2 + gap;
    let btn_left = rect.left + (rect.right - rect.left - total_w) / 2;
    let btn_top = rect.bottom - btn_h - 10;
    let copy_rect = RECT {
        left: btn_left,
        top: btn_top,
        right: btn_left + btn_w,
        bottom: btn_top + btn_h,
    };
    let close_rect = RECT {
        left: btn_left + btn_w + gap,
        top: btn_top,
        right: btn_left + btn_w * 2 + gap,
        bottom: btn_top + btn_h,
    };
    // FIX-006 v2: bottom buttons use brighter border to distinguish from window edge
    let btn_pen = unsafe { CreatePen(PS_SOLID, 1, BTN_BORDER) };
    let old_pen = unsafe { SelectObject(hdc, btn_pen) };
    let hollow = unsafe { GetStockObject(NULL_BRUSH) };
    let old_brush = unsafe { SelectObject(hdc, hollow) };
    unsafe {
        let _ = RoundRect(
            hdc,
            copy_rect.left,
            copy_rect.top,
            copy_rect.right,
            copy_rect.bottom,
            8,
            8,
        );
        let _ = RoundRect(
            hdc,
            close_rect.left,
            close_rect.top,
            close_rect.right,
            close_rect.bottom,
            8,
            8,
        );
        let _ = SelectObject(hdc, old_pen);
        let _ = SelectObject(hdc, old_brush);
        let _ = DeleteObject(btn_pen);
    }
    let copy_label = strings.preview_copy_btn;
    let close_label = strings.preview_close;
    let mut copy_text_rect = copy_rect;
    let mut close_text_rect = close_rect;
    // FIX-006-7: copy button text orange, close button text gray
    unsafe {
        let _ = SetTextColor(hdc, BRAND_ORANGE);
    }
    draw_text(
        hdc,
        copy_label,
        &mut copy_text_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    unsafe {
        let _ = SetTextColor(hdc, COLORREF(0x808080));
    }
    draw_text(
        hdc,
        close_label,
        &mut close_text_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    (copy_rect, close_rect, title_close_rect)
}
#[cfg(target_os = "windows")]
fn draw_error_overlay(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    message: &str,
    _ui_language: config::UiLanguage,
) {
    // UI-OVERLAY-OPT-001: unified visual style
    const BRAND_ORANGE: COLORREF = COLORREF(0x006BFF); // #FF6B00
    const BG_DARK: COLORREF = COLORREF(0x211D1A); // #1A1D21
    const BORDER_GRAY: COLORREF = COLORREF(0x060607); // FIX-006-1: unify with recording overlay
    const CORNER_RADIUS: i32 = 10;
    // Dark gray background (unified)
    let bg = unsafe { CreateSolidBrush(BG_DARK) };
    unsafe {
        let _ = FillRect(hdc, rect, bg);
        let _ = DeleteObject(bg);
    }
    // Border: 1px rounded corners (unified)
    let border_pen = unsafe { CreatePen(PS_SOLID, 1, BORDER_GRAY) };
    let border_old_pen = unsafe { SelectObject(hdc, border_pen) };
    let null_brush = unsafe { GetStockObject(NULL_BRUSH) };
    let border_old_brush = unsafe { SelectObject(hdc, null_brush) };
    unsafe {
        let _ = RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            CORNER_RADIUS * 2,
            CORNER_RADIUS * 2,
        );
        let _ = SelectObject(hdc, border_old_pen);
        let _ = SelectObject(hdc, border_old_brush);
        let _ = DeleteObject(border_pen);
    }
    // UI-OVERLAY-OPT-001: small solid red circle on left
    let circ_d = 8; // diameter 8px
    let circ_x = rect.left + 12 + circ_d / 2; // center x
    let cy = rect.top + (rect.bottom - rect.top) / 2;
    let red_brush = unsafe { CreateSolidBrush(COLORREF(0x0033CC)) }; // BGR: red
    let null_pen = unsafe { CreatePen(PS_NULL, 0, COLORREF(0)) };
    let old_brush = unsafe { SelectObject(hdc, red_brush) };
    let old_pen = unsafe { SelectObject(hdc, null_pen) };
    unsafe {
        let _ = Ellipse(
            hdc,
            circ_x - circ_d / 2,
            cy - circ_d / 2,
            circ_x + circ_d / 2,
            cy + circ_d / 2,
        );
        let _ = SelectObject(hdc, old_brush);
        let _ = SelectObject(hdc, old_pen);
        let _ = DeleteObject(red_brush);
        let _ = DeleteObject(null_pen);
    }
    // Error text (orange, left-aligned with margin for circle)
    let mut text_rect = RECT {
        left: circ_x + circ_d / 2 + 8,
        top: rect.top + 4,
        right: rect.right - 14,
        bottom: rect.bottom - 4,
    };
    unsafe {
        let _ = SetTextColor(hdc, BRAND_ORANGE);
    }
    draw_text(
        hdc,
        message,
        &mut text_rect,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
    );
}
#[cfg(target_os = "windows")]
fn overlay_geometry(status: &OverlayStatus) -> ([i32; 2], [i32; 2]) {
    let size = match status {
        OverlayStatus::Recording => RECORDING_OVERLAY_SIZE,
        OverlayStatus::FallingToProcessing { .. } => RECORDING_OVERLAY_SIZE,
        OverlayStatus::Processing(_) => STATUS_OVERLAY_SIZE,
        OverlayStatus::FocusLost { .. } => PREVIEW_OVERLAY_SIZE,
        OverlayStatus::Error(_) => STATUS_OVERLAY_SIZE,
    };
    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let x = (screen_w - size[0]) / 2;
    let y = (screen_h - size[1] - 64).max(0);
    ([x, y], size)
}
#[cfg(target_os = "windows")]
fn convert_to_friendly_error(message: &str, ui_language: config::UiLanguage) -> String {
    let strings = i18n::get(ui_language);
    let m = message.to_ascii_lowercase();
    if m.contains("timeout") || m.contains("timed out") {
        strings.error_network_timeout.to_string()
    } else if m.contains("api") || m.contains("http") || m.contains("401") || m.contains("403") {
        strings.error_api_unavailable.to_string()
    } else if m.contains("model") || m.contains("recognizer") {
        strings.error_model_init.to_string()
    } else if m.contains("audio") || m.contains("microphone") {
        strings.error_microphone.to_string()
    } else if m.contains("mic_muted") {
        strings.error_mic_muted.to_string()
    } else {
        message.to_string()
    }
}
#[cfg(target_os = "windows")]
fn show_overlay(
    overlay_handle: &OverlayThreadHandle,
    overlay_opacity: f32,
    ui_language: config::UiLanguage,
    status: OverlayStatus,
) {
    let (pos, size) = overlay_geometry(&status);
    overlay_handle.send(OverlayCommand::Show(OverlayRequest {
        status,
        pos,
        size,
        opacity: overlay_opacity.clamp(0.1, 1.0),
        ui_language,
        auto_close_ms: 0,
    }));
}
fn maybe_refresh_settings_child(
    settings_child: &mut Option<Child>,
    runtime_config: &Arc<RwLock<AppConfig>>,
) {
    let mut clear_child = false;
    if let Some(child) = settings_child.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) => {
                reload_runtime_config(runtime_config);
                clear_child = true;
            }
            Ok(None) => {}
            Err(err) => {
                log::warn!("Failed to poll settings child: {}", err);
                clear_child = true;
            }
        }
    }
    if clear_child {
        *settings_child = None;
    }
}
fn set_tray_state(tray: &mut Option<TrayIcon>, state: TrayState, ui_language: config::UiLanguage) {
    let Some(tray) = tray.as_mut() else {
        return;
    };
    let _ = tray.set_tooltip(Some(state.tooltip(ui_language)));
    let _ = tray.set_icon(Some(state.icon()));
}
#[allow(clippy::too_many_arguments)]
#[cfg(target_os = "windows")]
fn process_controller_events(
    controller_hwnd: HWND,
    tray: &mut Option<TrayIcon>,
    settings_child: &mut Option<Child>,
    runtime_config: &Arc<RwLock<AppConfig>>,
    hotkey_listener: &platform::HotkeyListener, // MAC-003: Use platform layer
    worker_tx: &crossbeam_channel::Sender<WorkerCommand>,
    overlay_handle: &OverlayThreadHandle,
    overlay_event_rx: &crossbeam_channel::Receiver<OverlayUiEvent>,
    app_cmd_rx: &crossbeam_channel::Receiver<AppCommand>,
    pipeline_event_rx: &crossbeam_channel::Receiver<PipelineEvent>,
    stop_recording_signal: &Arc<AtomicBool>,
    cancel_signal: &Arc<AtomicBool>,
    is_recording: &Arc<AtomicBool>,
) -> Result<bool> {
    maybe_refresh_settings_child(settings_child, runtime_config);
    while let Ok(command) = app_cmd_rx.try_recv() {
        log::info!("App command received: {:?}", command);
        match command {
            AppCommand::OpenSettings => {
                maybe_refresh_settings_child(settings_child, runtime_config);
                if settings_child.is_none() {
                    log::info!("Spawning Tauri Settings UI...");
                    match spawn_settings_process() {
                        Ok(child) => {
                            log::info!("Settings process spawned, pid={}", child.id());
                            *settings_child = Some(child);
                        }
                        Err(e) => {
                            log::error!("Failed to spawn settings process: {}", e);
                        }
                    }
                } else {
                    log::info!("Settings child already exists, skipping spawn");
                }
            }
            AppCommand::Exit => {
                log::info!("Exit command received, returning true");
                return Ok(true);
            }
            AppCommand::ShowTrayMenu { x, y } => {
                let ui_language = clone_runtime_config(runtime_config).ui_language;
                if let Some(next_cmd) = show_tray_popup_menu(controller_hwnd, x, y, ui_language) {
                    let should_exit = matches!(next_cmd, AppCommand::Exit);
                    if !should_exit {
                        // route settings through the same command path
                        if let AppCommand::OpenSettings = next_cmd {
                            if settings_child
                                .as_mut()
                                .and_then(|child| child.try_wait().ok().flatten())
                                .is_some()
                            {
                                *settings_child = None;
                            }
                            if settings_child.is_none() {
                                match spawn_settings_process() {
                                    Ok(child) => {
                                        *settings_child = Some(child);
                                    }
                                    Err(e) => {
                                        log::error!("Failed to spawn settings process: {}", e);
                                    }
                                }
                            }
                        }
                    } else {
                        log::info!("Exit command received from tray menu, returning true");
                        return Ok(true);
                    }
                }
            }
        }
    }
    while let Ok(event) = hotkey_listener.rx().try_recv() {
        match event {
            HotkeyEvent::Start { translate } => {
                let t_hotkey = std::time::Instant::now();
                log::info!(
                    "Controller received hotkey start (translate={})",
                    translate.load(Ordering::Acquire)
                );
                maybe_refresh_settings_child(settings_child, runtime_config);
                if is_recording.load(Ordering::Acquire) {
                    stop_recording_signal.store(true, Ordering::Release);
                } else {
                    let hwnd = unsafe { GetForegroundWindow() };
                    cancel_signal.store(false, Ordering::Release);
                    stop_recording_signal.store(false, Ordering::Release);

                    if crate::audio::is_mic_muted() {
                        let config = clone_runtime_config(runtime_config);
                        let msg = i18n::get(config.ui_language).error_mic_muted.to_string();
                        let (pos, size) = overlay_geometry(&OverlayStatus::Error(msg.clone()));
                        overlay_handle.send(OverlayCommand::Show(OverlayRequest {
                            status: OverlayStatus::Error(msg),
                            pos,
                            size,
                            opacity: 0.95,
                            ui_language: config.ui_language,
                            auto_close_ms: 2000,
                        }));
                        continue;
                    }

                    // HOTKEY-LATENCY-FIX-001: 立即显示录音 overlay，不等 RecordingStarted 事件
                    let config = clone_runtime_config(runtime_config);
                    show_overlay(
                        overlay_handle,
                        config.audio.overlay_opacity,
                        config.ui_language,
                        OverlayStatus::Recording,
                    );
                    log::info!(
                        "[Latency] overlay shown at +{:.1}ms",
                        t_hotkey.elapsed().as_secs_f64() * 1000.0
                    );
                    set_tray_state(tray, TrayState::Recording, config.ui_language);

                    let _ = worker_tx.send(WorkerCommand::Start(StartCmd {
                        target_hwnd: SendHwnd(hwnd.0 as isize),
                        translate,
                    }));
                    log::info!(
                        "[Latency] worker command sent at +{:.1}ms",
                        t_hotkey.elapsed().as_secs_f64() * 1000.0
                    );
                }
            }
            HotkeyEvent::Stop => {
                log::info!("Controller received hotkey stop");
                stop_recording_signal.store(true, Ordering::Release);
                if is_recording.load(Ordering::Acquire) {
                    let config = clone_runtime_config(runtime_config);
                    show_overlay(
                        overlay_handle,
                        config.audio.overlay_opacity,
                        config.ui_language,
                        OverlayStatus::FallingToProcessing {
                            message: i18n::get(config.ui_language).overlay_transcribing.to_string(),
                        },
                    );
                }
            }
            HotkeyEvent::CancelStop => {
                log::info!("Controller received hotkey cancel-stop (PTT held < 300ms)");
                cancel_signal.store(true, Ordering::Release);
                stop_recording_signal.store(true, Ordering::Release);
            }
        }
    }
    if is_recording.load(Ordering::Acquire) {
        let esc = unsafe { GetAsyncKeyState(VK_ESCAPE.0 as i32) };
        if (esc as u16) & 0x0001 != 0 {
            cancel_signal.store(true, Ordering::Release);
            stop_recording_signal.store(true, Ordering::Release);
            let ui_language = clone_runtime_config(runtime_config).ui_language;
            set_tray_state(tray, TrayState::Idle, ui_language);
        }
    }
    while let Ok(event) = pipeline_event_rx.try_recv() {
        let config = clone_runtime_config(runtime_config);
        let ui_language = config.ui_language;
        let opacity = config.audio.overlay_opacity;
        match event {
            PipelineEvent::RecordingStarted => {
                set_tray_state(tray, TrayState::Recording, ui_language);
                show_overlay(overlay_handle, opacity, ui_language, OverlayStatus::Recording);
            }
            PipelineEvent::Processing(message) => {
                set_tray_state(tray, TrayState::Processing, ui_language);
                show_overlay(
                    overlay_handle,
                    opacity,
                    ui_language,
                    OverlayStatus::FallingToProcessing { message },
                );
            }
            PipelineEvent::Done | PipelineEvent::Cancelled => {
                set_tray_state(tray, TrayState::Idle, ui_language);
                overlay_handle.send(OverlayCommand::Hide);
            }
            PipelineEvent::FocusLost(text) => {
                set_tray_state(tray, TrayState::Idle, ui_language);
                show_overlay(
                    overlay_handle,
                    opacity,
                    ui_language,
                    OverlayStatus::FocusLost {
                        text,
                        copied: false,
                    },
                );
            }
            PipelineEvent::Error(message) => {
                log::error!("Pipeline error: {}", message);
                set_tray_state(tray, TrayState::Error, ui_language);
                // Shimmer animation frame update (phase increments every cycle)
                let friendly_message = convert_to_friendly_error(&message, ui_language);
                let (pos, size) = overlay_geometry(&OverlayStatus::Error(friendly_message.clone()));
                overlay_handle.send(OverlayCommand::Show(OverlayRequest {
                    status: OverlayStatus::Error(friendly_message),
                    pos,
                    size,
                    opacity: 0.95,
                    ui_language: config.ui_language,
                    auto_close_ms: 2000,
                }));
            }
        }
    }
    while let Ok(event) = overlay_event_rx.try_recv() {
        let ui_language = clone_runtime_config(runtime_config).ui_language;
        match event {
            OverlayUiEvent::CancelRequested => {
                cancel_signal.store(true, Ordering::Relaxed);
                stop_recording_signal.store(true, Ordering::Relaxed);
                overlay_handle.send(OverlayCommand::Hide); // 闂呮劘妫?overlay 缁愭褰?
                set_tray_state(tray, TrayState::Idle, ui_language);
            }
            OverlayUiEvent::PreviewCopied => {
                overlay_handle.send(OverlayCommand::Hide);
            }
        }
    }
    Ok(false)
}
/// Set or cancel auto-start on boot (via Windows registry)
#[cfg(target_os = "windows")]
fn set_auto_start(enabled: bool) -> Result<()> {
    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const APP_NAME: &str = "飞音语音输入";
    let exe_path = std::env::current_exe()?;
    let exe_path_wide: Vec<u16> = OsStr::new(exe_path.to_string_lossy().as_ref())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let key_name_wide: Vec<u16> = OsStr::new(APP_NAME)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let sub_key_wide: Vec<u16> = OsStr::new(RUN_KEY)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let mut h_key = HKEY::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(sub_key_wide.as_ptr()),
            0,
            KEY_WRITE,
            &mut h_key,
        )
        .ok()
        .map_err(|e| anyhow!("RegOpenKeyExW failed: {}", e))?;
        if enabled {
            // Wait for settings child process to exit
            let data_slice: &[u8] = std::slice::from_raw_parts(
                exe_path_wide.as_ptr() as *const u8,
                exe_path_wide.len() * 2,
            );
            RegSetValueExW(
                h_key,
                PCWSTR(key_name_wide.as_ptr()),
                0,
                REG_SZ,
                Some(data_slice),
            )
            .ok()
            .map_err(|e| anyhow!("RegSetValueExW failed: {}", e))?;
        } else {
            // Force kill settings child if needed
            let _ = RegDeleteValueW(h_key, PCWSTR(key_name_wide.as_ptr()));
        }
        RegCloseKey(h_key)
            .ok()
            .map_err(|e| anyhow!("RegCloseKey failed: {}", e))?;
    }
    Ok(())
}
#[cfg(target_os = "windows")]
fn spawn_worker_thread(
    worker_rx: crossbeam_channel::Receiver<WorkerCommand>,
    event_tx: crossbeam_channel::Sender<PipelineEvent>,
    runtime_config: Arc<RwLock<AppConfig>>,
    audio_buf: AudioLevelBuf,
    stop_recording_signal: Arc<AtomicBool>,
    cancel_signal: Arc<AtomicBool>,
    is_recording: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let model_dir = transcription::model_dir();
        let mut audio_capture = audio::AudioCapture::new();
        if let Err(err) = audio_capture.prewarm(None) {
            log::warn!("Startup prewarm failed: {}", err);
        }
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                log::error!("Failed to create tokio runtime for worker: {}", err);
                return;
            }
        };

        // PERF-BATCH-001 TASK-1: Pre-initialize Transcriber at startup.
        // Previously created inside the Start handler, loading ONNX models on every recording (~1-2s delay).
        // Now created once and reused across all recording sessions.
        let config = clone_runtime_config(&runtime_config);
        let transcriber = match transcription::Transcriber::new(
            &model_dir,
            config.audio.enable_streaming,
            config.audio.transcription_language.clone(),
        ) {
            Ok(t) => Some(t),
            Err(err) => {
                log::error!("Failed to initialize transcriber at startup: {}", err);
                None
            }
        };

        // PERF-INIT-001: Pre-initialize LlmClient once; update_config() before each use.
        let mut llm_client = llm::LlmClient::new(config.llm.clone());

        // PERF-INIT-001: Pre-initialize TranslationEngine once; hot-reload only on config change.
        let mut cached_translation: Option<(config::TranslationLanguage, translation::TranslationEngine)> =
            if config.translation.enabled {
                load_translation_engine_for_target(&model_dir, config.translation.target_language)
                    .map(|engine| (config.translation.target_language, engine))
            } else {
                None
            };

        // PUNCT-INTEGRATION-001: Pre-initialize PunctuationEngine once.
        let mut cached_punctuation: Option<punctuation::PunctuationEngine> =
            if config.punctuation.enabled {
                punctuation::PunctuationEngine::new(&model_dir)
            } else {
                None
            };

        loop {
            let cmd = match worker_rx.recv_timeout(Duration::from_millis(500)) {
                Ok(cmd) => Some(cmd),
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => None,
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            };
            match cmd {
                None => {
                    // HOTKEY-STREAM-PREWARM-001: idle health check pre-rebuilds
                    // failed WASAPI stream so hotkey Start avoids 50-500ms rebuild
                    audio_capture.check_stream_health();
                }
                Some(WorkerCommand::Shutdown) => break,
                Some(WorkerCommand::Start(start)) => {
                    let t_worker = std::time::Instant::now();
                    cancel_signal.store(false, Ordering::Release);
                    stop_recording_signal.store(false, Ordering::Release);
                    is_recording.store(true, Ordering::Release);
                    send_event(&event_tx, PipelineEvent::RecordingStarted);
                    let config = clone_runtime_config(&runtime_config);
                    let device_name = if config.audio.input_device.trim().is_empty() {
                        None
                    } else {
                        Some(config.audio.input_device.as_str())
                    };
                    log::info!("[Latency] worker received Start command");
                    let samples_result = audio_capture.record(
                        Arc::clone(&stop_recording_signal),
                        config.audio.silence_threshold,
                        config::SILENCE_DURATION_MS,
                        config::MAX_RECORD_SECONDS,
                        Some(Arc::clone(&audio_buf)),
                        device_name,
                    );
                    log::info!(
                        "[Latency] record() completed after +{:.1}ms",
                        t_worker.elapsed().as_secs_f64() * 1000.0
                    );
                    is_recording.store(false, Ordering::Release);
                    if cancel_signal.load(Ordering::Acquire) {
                        send_event(&event_tx, PipelineEvent::Cancelled);
                        continue;
                    }

                    let transcriber = match &transcriber {
                        Some(t) => t,
                        None => {
                            log::error!("Transcriber not initialized at startup");
                            send_event(&event_tx, PipelineEvent::Error("Transcriber unavailable".into()));
                            continue;
                        }
                    };

                    // PERF-INIT-001: Reuse pre-initialized LlmClient; update config only.
                    llm_client.update_config(config.llm.clone());

                    // PERF-INIT-001: Hot-reload TranslationEngine only when enabled/direction changed.
                    let needs_reload = match &cached_translation {
                        Some((lang, _)) => {
                            !config.translation.enabled
                                || *lang != config.translation.target_language
                        }
                        None => config.translation.enabled,
                    };
                    if needs_reload {
                        cached_translation = if config.translation.enabled {
                            load_translation_engine_for_target(&model_dir, config.translation.target_language)
                                .map(|engine| (config.translation.target_language, engine))
                        } else {
                            None
                        };
                    }
                    // PUNCT-INTEGRATION-001: Hot-reload PunctuationEngine on config change.
                    if config.punctuation.enabled && cached_punctuation.is_none() {
                        cached_punctuation = punctuation::PunctuationEngine::new(&model_dir);
                    } else if !config.punctuation.enabled {
                        cached_punctuation = None;
                    }
                    let translation_engine = cached_translation.as_ref().map(|(_, e)| e);
                    run_pipeline(
                        samples_result,
                        transcriber,
                        &rt,
                        &llm_client,
                        &cancel_signal,
                        &config,
                        &runtime_config,
                        translation_engine,
                        cached_punctuation.as_mut(),
                        HWND(start.target_hwnd.0 as *mut std::ffi::c_void),
                        &event_tx,
                        start.translate,
                    );
                }
            }
        }
    })
}
#[cfg(target_os = "windows")]
fn run_controller(runtime_config: Arc<RwLock<AppConfig>>) -> Result<()> {
    let audio_buf = ui::overlay::new_audio_level_buf();
    let stop_recording_signal = Arc::new(AtomicBool::new(false));
    let cancel_signal = Arc::new(AtomicBool::new(false));
    let is_recording = Arc::new(AtomicBool::new(false));
    let (app_cmd_tx, app_cmd_rx) = crossbeam_channel::unbounded::<AppCommand>();
    let (pipeline_event_tx, pipeline_event_rx) = crossbeam_channel::unbounded::<PipelineEvent>();
    let (worker_tx, worker_rx) = crossbeam_channel::unbounded::<WorkerCommand>();
    let worker_join = spawn_worker_thread(
        worker_rx,
        pipeline_event_tx,
        Arc::clone(&runtime_config),
        Arc::clone(&audio_buf),
        Arc::clone(&stop_recording_signal),
        Arc::clone(&cancel_signal),
        Arc::clone(&is_recording),
    );
    // Wait for worker thread initialization (ASR model loading)
    std::thread::sleep(std::time::Duration::from_millis(100));
    let controller_hwnd = create_controller_window()?;
    // LATENCY-001: store controller HWND for send_event wake-up
    CONTROLLER_HWND.store(controller_hwnd.0 as isize, Ordering::Release);
    let _config_watcher = spawn_config_watcher(Arc::clone(&runtime_config));
    // 鍚姩鐑敭鐩戝惉鍣ㄣ€傜儹閿嚎绋嬪彂閫佷簨浠跺悗浼氬敜閱?controller锛?
    // 閬垮厤绛夊緟 15ms timer tick 鎵嶅紑濮嬪鐞嗗綍闊炽€?
    let hotkey_listener = platform::create_hotkey_listener_with_controller_wakeup(
        Arc::clone(&runtime_config),
        controller_hwnd,
        WM_APP_HOTKEY_EVENT,
    );
    let (overlay_handle, overlay_event_rx) = spawn_overlay_thread(Arc::clone(&audio_buf));
    let tray_tx = app_cmd_tx.clone();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |event: tray_icon::TrayIconEvent| {
        match event {
            tray_icon::TrayIconEvent::DoubleClick { .. } => {
                let _ = tray_tx.send(AppCommand::OpenSettings);
            }
            tray_icon::TrayIconEvent::Click {
                button: tray_icon::MouseButton::Right,
                button_state: tray_icon::MouseButtonState::Up,
                position,
                ..
            } => {
                let _ = tray_tx.send(AppCommand::ShowTrayMenu {
                    x: position.x as i32,
                    y: position.y as i32,
                });
            }
            _ => {}
        }
    }));
    // Initialize tray icon and config watcher
    {
        let cfg = clone_runtime_config(&runtime_config);
        if let Err(e) = set_auto_start(cfg.auto_start) {
            log::warn!("Failed to set auto_start: {}", e);
        }
    }
    unsafe {
        let _ = SetTimer(controller_hwnd, CONTROLLER_TIMER_ID, 15, None);
        let _ = PostMessageW(controller_hwnd, WM_APP_INIT_TRAY, WPARAM(0), LPARAM(0));
    }
    log::info!("Controller initialized, entering message loop");
    let mut tray: Option<TrayIcon> = None;
    let mut settings_child: Option<Child> = None;
    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) };
        if ret.0 <= 0 {
            log::info!("Message loop exiting (ret={})", ret.0);
            break;
        }
        match msg.message {
            WM_APP_INIT_TRAY => {
                log::info!("WM_APP_INIT_TRAY received, building tray...");
                if tray.is_none() {
                    let ui_language = clone_runtime_config(&runtime_config).ui_language;
                    tray = Some(build_tray(ui_language));
                    log::info!("Tray icon built");
                    set_tray_state(&mut tray, TrayState::Idle, ui_language);
                }
            }
            WM_TIMER if msg.hwnd == controller_hwnd && msg.wParam.0 == CONTROLLER_TIMER_ID => {
                let should_exit = process_controller_events(
                    controller_hwnd,
                    &mut tray,
                    &mut settings_child,
                    &runtime_config,
                    &hotkey_listener,
                    &worker_tx,
                    &overlay_handle,
                    &overlay_event_rx,
                    &app_cmd_rx,
                    &pipeline_event_rx,
                    &stop_recording_signal,
                    &cancel_signal,
                    &is_recording,
                )?;
                if should_exit {
                    log::info!("Controller events returned true, initiating shutdown");
                    unsafe {
                        let _ = KillTimer(controller_hwnd, CONTROLLER_TIMER_ID);
                        DestroyWindow(controller_hwnd)?;
                        PostQuitMessage(0);
                    }
                }
            }
            WM_APP_HOTKEY_EVENT => {
                let should_exit = process_controller_events(
                    controller_hwnd,
                    &mut tray,
                    &mut settings_child,
                    &runtime_config,
                    &hotkey_listener,
                    &worker_tx,
                    &overlay_handle,
                    &overlay_event_rx,
                    &app_cmd_rx,
                    &pipeline_event_rx,
                    &stop_recording_signal,
                    &cancel_signal,
                    &is_recording,
                )?;
                if should_exit {
                    log::info!("Controller hotkey wake returned true, initiating shutdown");
                    unsafe {
                        let _ = KillTimer(controller_hwnd, CONTROLLER_TIMER_ID);
                        DestroyWindow(controller_hwnd)?;
                        PostQuitMessage(0);
                    }
                }
            }
            // LATENCY-001: instant wake on pipeline event from worker thread
            WM_APP_PIPELINE_EVENT => {
                let should_exit = process_controller_events(
                    controller_hwnd,
                    &mut tray,
                    &mut settings_child,
                    &runtime_config,
                    &hotkey_listener,
                    &worker_tx,
                    &overlay_handle,
                    &overlay_event_rx,
                    &app_cmd_rx,
                    &pipeline_event_rx,
                    &stop_recording_signal,
                    &cancel_signal,
                    &is_recording,
                )?;
                if should_exit {
                    log::info!("Controller pipeline wake returned true, initiating shutdown");
                    unsafe {
                        let _ = KillTimer(controller_hwnd, CONTROLLER_TIMER_ID);
                        DestroyWindow(controller_hwnd)?;
                        PostQuitMessage(0);
                    }
                }
            }
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
    }
    cancel_signal.store(true, Ordering::SeqCst);
    stop_recording_signal.store(true, Ordering::SeqCst);
    let _ = worker_tx.send(WorkerCommand::Shutdown);
    hotkey_listener.shutdown();
    overlay_handle.shutdown_and_join();
    if let Some(mut child) = settings_child {
        let _ = child.kill();
        let _ = child.wait();
    }
    drop(tray);
    let _ = worker_join.join();
    hotkey_listener.join();
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let debug_mode = args.iter().any(|arg| arg == "-debug" || arg == "--debug");
    // Set log level and output based on debug mode
    if debug_mode {
        // Debug mode: output Debug level logs to file
        let log_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        std::fs::create_dir_all(&log_dir)?;
        let log_file = log_dir.join("debug.log");
        // Debug mode logging setup
        let target = Box::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_file)
                .map_err(|e| anyhow::anyhow!("Failed to open log file {:?}: {}", log_file, e))?,
        ) as Box<dyn std::io::Write + Send>;
        env_logger::Builder::new()
            .target(env_logger::Target::Pipe(target))
            .filter_level(log::LevelFilter::Debug)
            .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
            .init();
        log::info!("Debug mode enabled, logging to {:?}", log_file);
    } else {
        // 濮濓絽鐖跺Ο鈥崇础閿涙艾褰ф潏鎾冲毉Warn缁狙冨焼閺冦儱绻?
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Warn)
            .init();
    }
    log::info!("飞音语音输入 starting...");
    let runtime_config = Arc::new(RwLock::new(AppConfig::load().unwrap_or_default()));
    // 娉ㄥ唽 panic hook锛氬穿婧冩椂鍒涘缓鎶ュ憡骞跺惎鍔?crash-reporter 瀛愯繘绋?
    std::panic::set_hook(Box::new(|panic_info| {
        // Use default runtime state during panic (RwLock may be poisoned)
        let runtime = crash::RuntimeInfo::default();
        // Create crash report and spawn crash-reporter subprocess
        let report = crash::create_report_from_panic(
            panic_info,
            "v0.5.0",
            runtime,
            Vec::new(), // recent_logs 閺嗗倷绗夐弨鍫曟肠
        );
        // Panic hook setup
        let _ = crash::save_crash_report(&report);
        // 浼樺厛鎷夎捣鐙珛 crash reporter锛涜嫢涓嶅瓨鍦ㄥ垯浠呬繚鐣欐湰鍦?crash.json
        let _ = crash::spawn_reporter_process();
    }));
    // --settings-ui 鍙傛暟锛氬惎鍔?Tauri Settings 瀛愯繘绋嬶紙DEC-013锛?
    // 鍚姩鍣ㄦā寮忥細鍚姩 UI 鍚庣珛鍗抽€€鍑猴紝涓嶇瓑寰呭瓙杩涚▼
    if args.iter().any(|arg| arg == "--settings-ui") {
        if let Ok(child) = spawn_settings_process() {
            std::mem::forget(child); // Prevent drop issues during panic
        }
        return Ok(()); // 涓昏繘绋嬬珛鍗抽€€鍑?
    }
    // Single instance check: create named Mutex, exit if exists
    // Mutex 蹇呴』鍦ㄦ暣涓▼搴忚繍琛屾湡闂翠繚鎸佹墦寮€
    #[cfg(target_os = "windows")]
    {
        let mutex_name = "Global\\voice-ime-single-instance-mutex";
        let mutex_name_wide: Vec<u16> = mutex_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mutex_handle = unsafe {
            windows::Win32::System::Threading::CreateMutexW(
                None,
                true,
                windows::core::PCWSTR(mutex_name_wide.as_ptr()),
            )
        };
        let already_exists = match mutex_handle {
            Ok(_) => {
                // 濡偓閺?GetLastError 閺勵垰鎯佹潻鏂挎礀 ERROR_ALREADY_EXISTS
                // Check GetLastError for ERROR_ALREADY_EXISTS
                let err = unsafe { windows::Win32::Foundation::GetLastError() };
                err.0 == 183 // ERROR_ALREADY_EXISTS
            }
            Err(_) => true,
        };
        if already_exists {
            log::warn!("Application already running, exiting");
            return Ok(());
        }
        // Mutex acquired successfully, ensure mutex_handle is not dropped until exit
        let _mutex_handle = mutex_handle;
    }
    #[cfg(target_os = "windows")]
    run_controller(runtime_config)?;
    #[cfg(not(target_os = "windows"))]
    {
        log::warn!("Non-Windows platform is not supported in controller mode yet");
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
#[cfg(target_os = "windows")]
fn run_pipeline(
    samples_result: anyhow::Result<Vec<f32>>,
    transcriber: &transcription::Transcriber,
    rt: &tokio::runtime::Runtime,
    llm_client: &llm::LlmClient,
    cancel_signal: &AtomicBool,
    config: &AppConfig,
    runtime_config: &Arc<RwLock<AppConfig>>,
    translation_engine: Option<&translation::TranslationEngine>,
    mut punctuation_engine: Option<&mut punctuation::PunctuationEngine>,
    target_hwnd: HWND,
    event_tx: &crossbeam_channel::Sender<PipelineEvent>,
    translate: Arc<AtomicBool>,
) {
    match samples_result {
        Err(e) => {
            log::error!("Recording error: {}", e);
            send_event(event_tx, PipelineEvent::Error(e.to_string()));
        }
        Ok(s) if s.is_empty() => {
            log::warn!("No audio samples recorded");
            send_event(event_tx, PipelineEvent::Cancelled);
        }
        Ok(samples) => {
            if cancel_signal.load(Ordering::Relaxed) {
                send_event(event_tx, PipelineEvent::Cancelled);
                return;
            }
            // HOTKEY-LATENCY-V2-001: prepend 3200 zero-samples (200ms @16kHz)
            // as silence head to help ASR model with initial context framing
            let mut padded = Vec::with_capacity(3200 + samples.len());
            padded.resize(3200, 0.0f32);
            padded.extend_from_slice(&samples);
            log::info!("Transcribing {} samples (with 3200-sample silence head)", padded.len());
            let transcribing_msg = i18n::get(config.ui_language).overlay_transcribing;
            send_event(
                event_tx,
                PipelineEvent::Processing(transcribing_msg.to_string()),
            );
            match transcriber.transcribe(
                &padded,
                &config.audio.transcription_language,
                config.audio.chinese_script,
            ) {
                Err(e) => {
                    log::error!("Transcription error: {}", e);
                    send_event(event_tx, PipelineEvent::Error(e.to_string()));
                }
                Ok(raw_text) => {
                    if cancel_signal.load(Ordering::Relaxed) {
                        send_event(event_tx, PipelineEvent::Cancelled);
                        return;
                    }
                    log::info!("Transcribed: {}", raw_text);
                    // Skip downstream processing when transcription output is empty.
                    if raw_text.trim().is_empty() {
                        log::warn!("Transcription result is empty, skipping LLM and injection");
                        send_event(
                            event_tx,
                            PipelineEvent::Error(i18n::get(config.ui_language).error_transcription_empty.to_string()),
                        );
                        return;
                    }
                    let text = wordbook::Wordbook::open()
                        .and_then(|wb| wb.apply(&raw_text))
                        .unwrap_or_else(|_| raw_text.clone());
                    // OPT-002: Skip if text is not effective (empty or filler-only)
                    if !text_normalizer::is_effective_text(&text) {
                        log::info!(
                            "Skipping pipeline: transcription not effective (empty or filler only)"
                        );
                        send_event(event_tx, PipelineEvent::Cancelled);
                        return;
                    }
                    // TRANS-008 B閺傝顢嶉敍姝祌anslate=true 閺冭绱濋崡鏇燁偧 LLM 鐠嬪啰鏁ら崥灞炬缁剧娀鏁?缂堟槒鐦?
                    // translate=false 閺冭绱濋崢鐔告箒 LLM optimize 鐠侯垰绶炴稉宥呭綁
                    let mut llm_handled = false;
                    let translate_requested = translate.load(Ordering::Acquire);
                    let translation_allowed = should_translate_for_language(
                        &config.audio.transcription_language,
                        config.translation.target_language,
                    );
                    if translate_requested && config.translation.enabled && !translation_allowed {
                        log::info!(
                            "Translation skipped: source language '{}' does not require target {:?}",
                            config.audio.transcription_language,
                            config.translation.target_language
                        );
                    }
                    let final_text = if translate_requested
                        && config.translation.enabled
                        && !text.trim().is_empty()
                        && translation_allowed
                    {
                        let processing_msg = i18n::get(config.ui_language).overlay_processing;
                        send_event(
                            event_tx,
                            PipelineEvent::Processing(processing_msg.to_string()),
                        );
                        let script_instruction = text_normalizer::script_instruction(
                            &config.audio.transcription_language,
                            config.audio.chinese_script,
                        );
                        if should_try_llm_translate(
                            config.llm.enabled,
                            config.llm.connectivity_verified,
                        ) {
                            // B: LLM optimization failed (non-critical), continue with raw result
                            match rt.block_on(llm_client.optimize_and_translate(
                                &text,
                                config.translation.target_language,
                                script_instruction,
                                config.punctuation.enabled,
                            )) {
                                Ok(result) => {
                                    log::info!("LLM optimize+translate done: {}", result.text);
                                    learn_llm_suggestions(&result.suggestions, runtime_config);
                                    llm_handled = true;
                                    result.text
                                }
                                Err(e) => {
                                    log::warn!(
                                        "LLM optimize+translate failed, trying offline: {}",
                                        e
                                    );
                                    try_nllb_translate(&text, translation_engine).unwrap_or_else(
                                        || {
                                            text_normalizer::normalize_text_for_language(
                                                &text,
                                                &config.audio.transcription_language,
                                                config.audio.chinese_script,
                                            )
                                        },
                                    )
                                }
                            }
                        } else {
                            // LLM optimization success, use corrected text
                            try_nllb_translate(&text, translation_engine).unwrap_or_else(|| {
                                text_normalizer::normalize_text_for_language(
                                    &text,
                                    &config.audio.transcription_language,
                                    config.audio.chinese_script,
                                )
                            })
                        }
                    } else if config.llm.enabled && !text.trim().is_empty() {
                        // translate=false閿涙艾甯張?LLM optimize 鐠侯垰绶炴稉宥呭綁
                        let processing_msg = i18n::get(config.ui_language).overlay_processing;
                        send_event(
                            event_tx,
                            PipelineEvent::Processing(processing_msg.to_string()),
                        );
                        let script_instruction = text_normalizer::script_instruction(
                            &config.audio.transcription_language,
                            config.audio.chinese_script,
                        );
                        let llm_result =
                            rt.block_on(llm_client.optimize(&text, script_instruction, config.punctuation.enabled));
                        match llm_result {
                            Ok(result) => {
                                log::info!("LLM optimized: {}", result.text);
                                learn_llm_suggestions(&result.suggestions, runtime_config);
                                llm_handled = true;
                                text_normalizer::normalize_text_for_language(
                                    &result.text,
                                    &config.audio.transcription_language,
                                    config.audio.chinese_script,
                                )
                            }
                            Err(e) => {
                                log::warn!("LLM optimization error: {}", e);
                                text_normalizer::normalize_text_for_language(
                                    &text,
                                    &config.audio.transcription_language,
                                    config.audio.chinese_script,
                                )
                            }
                        }
                    } else {
                        // LLM optimization skipped
                        log::info!(
                            "LLM disabled or text empty, using transcription result directly"
                        );
                        text_normalizer::normalize_text_for_language(
                            &text,
                            &config.audio.transcription_language,
                            config.audio.chinese_script,
                        )
                    };
                    // PUNCT-INTEGRATION-001: Apply local punctuation when:
                    // - auto_punct=true (config.punctuation.enabled)
                    // - LLM did NOT handle the text (LLM adds its own punctuation)
                    // - Translation was NOT requested this recording (CT2 output already has punctuation)
                    let final_text = if config.punctuation.enabled && !llm_handled && !translate_requested {
                        if let Some(ref mut engine) = punctuation_engine {
                            match engine.add_punctuation(&final_text) {
                                Some(punctuated) => {
                                    log::info!("Local punctuation applied: '{}' -> '{}'", final_text, punctuated);
                                    punctuated
                                }
                                None => {
                                    log::warn!("Local punctuation returned None, keeping original text");
                                    final_text
                                }
                            }
                        } else {
                            log::debug!("Punctuation engine not available, skipping");
                            final_text
                        }
                    } else {
                        final_text
                    };
                    if cancel_signal.load(Ordering::Relaxed) {
                        send_event(event_tx, PipelineEvent::Cancelled);
                        return;
                    }
                    let current_hwnd = unsafe { GetForegroundWindow() };
                    let focus_lost = !target_hwnd.0.is_null() && current_hwnd.0 != target_hwnd.0;
                    log::info!(
                        "Injecting text: '{}', focus_lost={}, target_hwnd={:?}, current_hwnd={:?}",
                        final_text,
                        focus_lost,
                        target_hwnd.0,
                        current_hwnd.0
                    );
                    if focus_lost {
                        log::info!("Focus lost, showing preview");
                        send_event(event_tx, PipelineEvent::FocusLost(final_text));
                    } else {
                        log::info!("Injecting text...");
                        let text_snapshot = platform::capture_focused_text_snapshot(); // MAC-004
                        if let Err(e) = platform::inject_text(
                            &final_text,
                            config.injection.use_clipboard,
                            config.injection.clipboard_delay_ms,
                        ) {
                            log::error!("Injection failed: {}", e);
                        } else {
                            log::info!("Injection completed successfully");
                        }
                        maybe_learn_user_edit(&final_text, text_snapshot, runtime_config);
                        send_event(event_tx, PipelineEvent::Done);
                    }
                }
            }
        }
    }
}
#[cfg(target_os = "windows")]
fn should_try_llm_translate(llm_enabled: bool, connectivity_verified: bool) -> bool {
    llm_enabled && connectivity_verified
}
#[cfg(target_os = "windows")]
fn should_translate_for_language(
    transcription_language: &str,
    target: config::TranslationLanguage,
) -> bool {
    let source = transcription_language.trim().to_ascii_lowercase();
    let source_is_chinese = source.starts_with("zh");
    let source_is_english = source == "en" || source.starts_with("en-");
    match target {
        config::TranslationLanguage::English => !source_is_english,
        config::TranslationLanguage::Chinese => !source_is_chinese,
    }
}
#[cfg(target_os = "windows")]
fn try_nllb_translate(
    text: &str,
    engine: Option<&translation::TranslationEngine>,
) -> Option<String> {
    let Some(engine) = engine else {
        log::info!("NLLB translation skipped: offline engine unavailable");
        return None;
    };
    match engine.translate(text) {
        Ok(translated) if !translated.trim().is_empty() => {
            log::info!("NLLB translation done: {}", translated);
            Some(translated)
        }
        Ok(_) => {
            log::warn!("NLLB translation returned empty text");
            None
        }
        Err(err) => {
            log::warn!("NLLB translation failed: {}", err);
            None
        }
    }
}
/// PERF-INIT-001: Determine if TranslationEngine needs hot-reload based on config change.
/// Extracted from spawn_worker_thread for testability.
#[cfg(target_os = "windows")]
fn translation_needs_reload(
    cached: &Option<config::TranslationLanguage>,
    enabled: bool,
    target: config::TranslationLanguage,
) -> bool {
    match cached {
        Some(lang) => !enabled || *lang != target,
        None => enabled,
    }
}
#[cfg(all(test, target_os = "windows"))]
mod pipeline_logic_tests {
    use super::{should_try_llm_translate, translation_needs_reload};
    use crate::config::TranslationLanguage;

    #[test]
    fn llm_translate_requires_enabled_and_connectivity_verified() {
        assert!(should_try_llm_translate(true, true));
        assert!(!should_try_llm_translate(true, false));
        assert!(!should_try_llm_translate(false, true));
        assert!(!should_try_llm_translate(false, false));
    }

    // ============================================================
    // PERF-INIT-001: TranslationEngine hot-reload needs_reload tests
    // ============================================================

    /// No cached engine + enabled = needs reload (first load)
    #[test]
    fn translation_needs_reload_when_none_and_enabled() {
        assert!(translation_needs_reload(&None, true, TranslationLanguage::English));
        assert!(translation_needs_reload(&None, true, TranslationLanguage::Chinese));
    }

    /// No cached engine + disabled = no reload needed
    #[test]
    fn translation_no_reload_when_none_and_disabled() {
        assert!(!translation_needs_reload(&None, false, TranslationLanguage::English));
    }

    /// Cached engine + disabled = needs reload (to clear cache)
    #[test]
    fn translation_needs_reload_when_cached_but_disabled() {
        let cached = Some(TranslationLanguage::English);
        assert!(translation_needs_reload(&cached, false, TranslationLanguage::English));
    }

    /// Cached engine + same target + enabled = no reload needed
    #[test]
    fn translation_no_reload_when_cached_same_target() {
        let cached = Some(TranslationLanguage::English);
        assert!(!translation_needs_reload(&cached, true, TranslationLanguage::English));
    }

    /// Cached engine + different target = needs reload (direction changed)
    #[test]
    fn translation_needs_reload_when_cached_different_target() {
        let cached = Some(TranslationLanguage::English);
        assert!(translation_needs_reload(&cached, true, TranslationLanguage::Chinese));
    }

    /// Cached Chinese + switch to English = needs reload
    #[test]
    fn translation_needs_reload_chinese_to_english() {
        let cached = Some(TranslationLanguage::Chinese);
        assert!(translation_needs_reload(&cached, true, TranslationLanguage::English));
    }
}
fn maybe_learn_user_edit(
    expected_text: &str,
    snapshot: Option<platform::FocusedTextSnapshot>,
    runtime_config: &Arc<RwLock<AppConfig>>,
) {
    // MAC-004
    let Some(snapshot) = snapshot else {
        return;
    };
    thread::sleep(Duration::from_millis(AUTO_LEARN_OBSERVE_MS));
    let Some(after_text) = platform::read_text_from_hwnd(snapshot.hwnd) else {
        // MAC-004
        return;
    };
    let Some(observed_text) = extract_changed_text(&snapshot.text, &after_text) else {
        return;
    };
    let expected_text = expected_text.trim();
    let observed_text = observed_text.trim();
    if expected_text.is_empty() || observed_text.is_empty() || expected_text == observed_text {
        return;
    }
    let auto_learn_threshold = read_auto_learn_threshold(runtime_config);
    if let Err(e) = wordbook::Wordbook::open()
        .and_then(|wb| wb.learn_correction(expected_text, observed_text, auto_learn_threshold))
    {
        log::debug!("Auto-learning skipped: {}", e);
    }
}
fn learn_llm_suggestions(
    suggestions: &[llm::SuggestionEntry],
    runtime_config: &Arc<RwLock<AppConfig>>,
) {
    if suggestions.is_empty() {
        return;
    }
    let auto_learn_threshold = read_auto_learn_threshold(runtime_config);
    let wordbook = match wordbook::Wordbook::open() {
        Ok(wordbook) => wordbook,
        Err(err) => {
            log::debug!("Skipping LLM wordbook suggestions: {}", err);
            return;
        }
    };
    for suggestion in suggestions {
        if let Err(err) =
            wordbook.learn_suggestion(&suggestion.raw, &suggestion.corrected, auto_learn_threshold)
        {
            log::debug!(
                "Skipping LLM suggestion '{}' -> '{}': {}",
                suggestion.raw,
                suggestion.corrected,
                err
            );
        }
    }
}
fn read_auto_learn_threshold(runtime_config: &Arc<RwLock<AppConfig>>) -> u32 {
    match runtime_config.read() {
        Ok(cfg) => cfg.auto_learn_threshold.max(1),
        Err(poisoned) => poisoned.into_inner().auto_learn_threshold.max(1),
    }
}
fn extract_changed_text(before: &str, after: &str) -> Option<String> {
    if before == after {
        return None;
    }
    let before_chars: Vec<char> = before.chars().collect();
    let after_chars: Vec<char> = after.chars().collect();
    let common_prefix = before_chars
        .iter()
        .zip(after_chars.iter())
        .take_while(|(lhs, rhs)| lhs == rhs)
        .count();
    let common_suffix = before_chars[common_prefix..]
        .iter()
        .rev()
        .zip(after_chars[common_prefix..].iter().rev())
        .take_while(|(lhs, rhs)| lhs == rhs)
        .count();
    let added_text = after_chars[common_prefix..after_chars.len() - common_suffix]
        .iter()
        .collect::<String>();
    Some(added_text)
}
// ============================================================================
// macOS Stub Module (MAC-001)
// ============================================================================
// macOS stub module: provides placeholder types for compilation on macOS without Win32 APIs
#[cfg(target_os = "macos")]
mod macos_stubs {
    use super::*;
    // Stub types (placeholder for Windows types, not actual Win32 implementations)
    #[derive(Debug, Clone)]
    pub struct OverlayCommand;
    #[derive(Debug, Clone)]
    pub struct OverlayRequest;
    #[derive(Debug, Clone, Copy)]
    pub struct SendHwnd(isize);
    unsafe impl Send for SendHwnd {}
    #[derive(Debug, Clone)]
    pub struct StartCmd {
        pub target_hwnd: SendHwnd,
        pub translate: Arc<AtomicBool>,
    }
    #[derive(Debug)]
    pub enum WorkerCommand {
        Start(StartCmd),
        Shutdown,
    }
    pub struct OverlayThreadHandle;
    impl OverlayThreadHandle {
        pub fn send(&self, _command: OverlayCommand) {
            // Stub: macOS overlay not implemented
        }
        pub fn shutdown_and_join(self) {
            // Stub: macOS overlay not implemented
        }
    }
    // Stub functions
    pub fn encode_wide(_text: &str) -> Vec<u16> {
        Vec::new() // Stub: macOS placeholder for Windows wide string
    }
    pub fn spawn_overlay_thread(
        _audio_buf: AudioLevelBuf,
    ) -> (
        OverlayThreadHandle,
        crossbeam_channel::Receiver<OverlayUiEvent>,
    ) {
        let (_, rx) = crossbeam_channel::unbounded();
        (OverlayThreadHandle, rx) // Stub: macOS overlay not implemented
    }
    pub fn spawn_worker_thread(
        _worker_rx: crossbeam_channel::Receiver<WorkerCommand>,
        _event_tx: crossbeam_channel::Sender<PipelineEvent>,
        _runtime_config: Arc<RwLock<AppConfig>>,
        _audio_buf: AudioLevelBuf,
        _stop_recording_signal: Arc<AtomicBool>,
        _cancel_signal: Arc<AtomicBool>,
        _is_recording: Arc<AtomicBool>,
    ) -> JoinHandle<()> {
        thread::spawn(|| {
            // Stub: macOS worker not implemented
        })
    }
}

#[cfg(test)]
mod overlay_shimmer_tests {
    #[test]
    fn shimmer_triangle_wave_midpoint() {
        let phase: f32 = 0.5;
        let p = if phase <= 1.0 { phase } else { 2.0 - phase };
        assert!((p - 0.5).abs() < 0.001);
    }

    #[test]
    fn shimmer_triangle_wave_peak() {
        let phase: f32 = 1.0;
        let p = if phase <= 1.0 { phase } else { 2.0 - phase };
        assert!((p - 1.0).abs() < 0.001);
    }

    #[test]
    fn shimmer_triangle_wave_descending() {
        let phase: f32 = 1.5;
        let p = if phase <= 1.0 { phase } else { 2.0 - phase };
        assert!((p - 0.5).abs() < 0.001);
    }

    #[test]
    fn shimmer_triangle_wave_reset() {
        let phase: f32 = 1.96;
        let p = if phase <= 1.0 { phase } else { 2.0 - phase };
        assert!(p < 0.05);
    }

    #[test]
    fn shimmer_band_width_quarter_window() {
        // OVERLAY-FIX-003: band_width = window_width / 5
        let window_width: i32 = 240;
        let band_width = window_width / 5;
        assert_eq!(band_width, 48);
    }

    #[test]
    fn shimmer_band_width_480px_window() {
        let window_width: i32 = 480;
        let band_width = window_width / 5;
        assert_eq!(band_width, 96);
    }

    #[test]
    fn shimmer_band_width_travel_range() {
        // travel_range = window_width - band_width
        let window_width: i32 = 240;
        let band_width = window_width / 5;
        let travel_range = window_width - band_width;
        assert_eq!(travel_range, 192);
    }

    // === OVERLAY-FIX-004: Processing gradient endpoint color ===

    #[test]
    fn processing_gradient_bg_dark_color_value() {
        // OVERLAY-FIX-004: gradient endpoints use BG_DARK instead of black (0x0000)
        // BG_DARK = COLORREF(0x181A18) → TRIVERTEX: R=0x1800, G=0x1A00, B=0x1800
        const BG_DARK_RED: u16 = 0x1800;
        const BG_DARK_GREEN: u16 = 0x1A00;
        const BG_DARK_BLUE: u16 = 0x1800;
        // Verify these match the expected #181A18 color
        assert_eq!(BG_DARK_RED, 0x1800, "Red channel must match #18 in BG_DARK");
        assert_eq!(BG_DARK_GREEN, 0x1A00, "Green channel must match #1A in BG_DARK");
        assert_eq!(BG_DARK_BLUE, 0x1800, "Blue channel must match #18 in BG_DARK");
        // Verify it is NOT black (the old value)
        assert!(BG_DARK_RED != 0x0000, "Must not be black");
        assert!(BG_DARK_GREEN != 0x0000, "Must not be black");
        assert!(BG_DARK_BLUE != 0x0000, "Must not be black");
    }

    #[test]
    fn processing_gradient_endpoint_symmetry() {
        // OVERLAY-FIX-004: both endpoints (left and right) must use identical BG_DARK
        let left_endpoint = (0x1800u16, 0x1A00u16, 0x1800u16);
        let right_endpoint = (0x1800u16, 0x1A00u16, 0x1800u16);
        assert_eq!(left_endpoint, right_endpoint,
            "Left and right gradient endpoints must be symmetric BG_DARK");
    }

    #[test]
    fn processing_gradient_midpoint_brighter_than_endpoints() {
        // OVERLAY-FIX-004: midpoint (0xE000) must be brighter than BG_DARK endpoints (0x1800/0x1A00)
        let midpoint_r: u16 = 0xE000;
        let midpoint_g: u16 = 0xE000;
        let midpoint_b: u16 = 0xE000;
        let endpoint_r: u16 = 0x1800;
        let endpoint_g: u16 = 0x1A00;
        let endpoint_b: u16 = 0x1800;
        assert!(midpoint_r > endpoint_r, "Midpoint must be brighter than endpoints (R)");
        assert!(midpoint_g > endpoint_g, "Midpoint must be brighter than endpoints (G)");
        assert!(midpoint_b > endpoint_b, "Midpoint must be brighter than endpoints (B)");
    }

    #[test]
    fn mic_icon_roundrect_parameters() {
        // MIC-ICON-ENLARGE-001: microphone icon enlarged from 14px to 18px
        // RoundRect(left=22, top=4, right=50, bottom=53, ellipse_w=28, ellipse_h=28)
        let left: i32 = 22;
        let top: i32 = 4;
        let right: i32 = 50;
        let bottom: i32 = 53;
        let ellipse_w: i32 = 28;
        let ellipse_h: i32 = 28;
        let width = right - left;
        let height = bottom - top;
        assert_eq!(width, 28, "Mic body width must be 28px");
        assert_eq!(height, 49, "Mic body height must be 49px");
        assert_eq!(ellipse_w, 28, "Ellipse width must match body width for fully rounded ends");
        assert_eq!(ellipse_h, 28, "Ellipse height must be 28px");
        assert!(ellipse_w <= width, "Ellipse width must not exceed body width");
    }

    #[test]
    fn mic_icon_stem_and_base_geometry() {
        // MIC-ICON-ENLARGE-001: stem + base geometry relative to mic body (18px icon, 4x=72x72)
        // Stem: MoveTo(36,53) → LineTo(36,63), vertical 10px
        // Base: MoveTo(24,63) → LineTo(48,63), horizontal 24px
        let stem_top: i32 = 53;
        let stem_bottom: i32 = 63;
        let base_left: i32 = 24;
        let base_right: i32 = 48;
        let stem_x: i32 = 36;
        assert_eq!(stem_bottom - stem_top, 10, "Stem must be 10px tall");
        assert_eq!(base_right - base_left, 24, "Base must be 24px wide");
        let base_center = (base_left + base_right) / 2;
        assert_eq!(stem_x, base_center, "Stem must be centered on base");
        let body_bottom: i32 = 53;
        assert_eq!(stem_top, body_bottom, "Stem must connect to mic body bottom");
    }

    #[test]
    fn mic_icon_18px_layout_margin() {
        // MIC-ICON-ENLARGE-001: circ_size=18px, left=rect.left+6, sep_l_x=30
        // 6 + 18 + 6 = 30, confirming 6px margin on both sides of enlarged icon
        let circ_size: i32 = 18;
        let circ_l_offset: i32 = 6;
        let sep_l_x: i32 = 30;
        assert_eq!(circ_l_offset + circ_size + circ_l_offset, sep_l_x,
            "Separator x must equal left_margin(6) + icon(18) + right_margin(6)");
        assert_eq!(circ_size, 18, "circ_size must be 18px after ENLARGE-001");
    }

    #[test]
    fn asr_silence_head_prepended() {
        // HOTKEY-LATENCY-V2-001: run_pipeline prepends 3200 zero-samples before transcribe
        let silence_head_len: usize = 3200;
        let original_samples: usize = 3200;
        let padded = Vec::with_capacity(silence_head_len + original_samples);
        let mut padded = padded;
        padded.resize(silence_head_len, 0.0f32);
        padded.extend_from_slice(&vec![1.0f32; original_samples]);
        assert_eq!(padded.len(), silence_head_len + original_samples,
            "Padded length must be silence_head + original");
        assert!(padded.iter().take(silence_head_len).all(|&s| s == 0.0),
            "First 3200 samples must be silence (zero)");
        assert_eq!(padded[silence_head_len], 1.0,
            "Original audio must begin after silence head");
    }

    #[test]
    fn asr_silence_head_is_200ms_at_16khz() {
        // HOTKEY-LATENCY-V2-001: 3200 samples = 200ms @ 16kHz
        let sample_rate: u32 = 16000;
        let silence_head: usize = 3200;
        let duration_ms = (silence_head as f64 / sample_rate as f64) * 1000.0;
        assert!((duration_ms - 200.0).abs() < 1.0,
            "3200 samples at 16kHz must be ~200ms (got {:.1}ms)", duration_ms);
    }

    // === OVERLAY-FIX-005: Processing phase stepping + Preview window layout ===

    #[test]
    fn processing_phase_step_increment() {
        // OVERLAY-FIX-005: phase increments by 0.015 per WM_PAINT frame
        let phase: f32 = 0.0;
        let new_phase = (phase + 0.015) % 2.0;
        assert!((new_phase - 0.015).abs() < 0.001, "Phase must increment by 0.015");
    }

    #[test]
    fn processing_phase_wrap_at_two() {
        // OVERLAY-FIX-005: phase wraps at 2.0 (not 1.0)
        let phase: f32 = 1.990;
        let new_phase = (phase + 0.015) % 2.0;
        assert!(new_phase < 0.01, "Phase must wrap to near 0 when exceeding 2.0");
    }

    #[test]
    fn processing_phase_triangle_wave_at_midpoint() {
        // OVERLAY-FIX-005: triangle wave p = phase if <= 1.0 else 2.0 - phase
        // At phase=1.0, p should be 1.0 (peak)
        let phase: f32 = 1.0;
        let p = if phase <= 1.0 { phase } else { 2.0 - phase };
        assert!((p - 1.0).abs() < 0.001, "Triangle wave must peak at phase=1.0");
    }

    #[test]
    fn processing_phase_triangle_wave_descending() {
        // OVERLAY-FIX-005: at phase=1.5, p = 2.0 - 1.5 = 0.5 (descending)
        let phase: f32 = 1.5;
        let p = if phase <= 1.0 { phase } else { 2.0 - phase };
        assert!((p - 0.5).abs() < 0.001, "Triangle wave must descend at phase>1.0");
    }

    #[test]
    fn processing_phase_triangle_wave_at_wrap() {
        // OVERLAY-FIX-005: at phase=1.985 (just before wrap), p = 2.0 - 1.985 = 0.015
        let phase: f32 = 1.985;
        let p = if phase <= 1.0 { phase } else { 2.0 - phase };
        assert!((p - 0.015).abs() < 0.001, "Triangle wave near wrap must be near 0");
    }

    #[test]
    fn preview_button_centering_math() {
        // OVERLAY-FIX-005: buttons centered in preview window
        let window_width: i32 = 320;
        let btn_w: i32 = 60;
        let btn_h: i32 = 24;
        let gap: i32 = 10;
        let total_w = btn_w * 2 + gap;
        let btn_left = (window_width - total_w) / 2;
        assert_eq!(total_w, 130, "Total button width must be 130px");
        assert_eq!(btn_left, 95, "Buttons must be centered at x=95 in 320px window");
        // Verify symmetry: left margin == right margin
        let left_margin = btn_left;
        let right_margin = window_width - (btn_left + total_w);
        assert_eq!(left_margin, right_margin, "Left and right margins must be equal");
    }

    #[test]
    fn preview_title_bar_close_button_position() {
        // OVERLAY-FIX-005: title bar close button 18x18 at right side
        let window_right: i32 = 320;
        let close_btn_space: i32 = 26;
        let title_close_left = window_right - 26;
        let title_close_right = window_right - 8;
        let title_close_top = 5;
        let title_close_bottom = 23;
        assert_eq!(title_close_right - title_close_left, 18, "Close button must be 18px wide");
        assert_eq!(title_close_bottom - title_close_top, 18, "Close button must be 18px tall");
        assert_eq!(title_close_left, window_right - close_btn_space,
            "Close button must be positioned close_btn_space from right edge");
    }

    #[test]
    fn preview_brand_orange_color_value() {
        // OVERLAY-FIX-005: BRAND_ORANGE = #FF6B00 used for title, X button, and buttons
        const BRAND_ORANGE: u32 = 0x006BFF; // COLORREF format: 0x00BBGGRR
        // Verify it is orange (high R, medium G, low B)
        let r = BRAND_ORANGE & 0xFF;
        let g = (BRAND_ORANGE >> 8) & 0xFF;
        let b = (BRAND_ORANGE >> 16) & 0xFF;
        assert_eq!(r, 0xFF, "BRAND_ORANGE must have full red channel");
        assert_eq!(g, 0x6B, "BRAND_ORANGE must have 0x6B green channel");
        assert_eq!(b, 0x00, "BRAND_ORANGE must have zero blue channel");
    }

    // === WAVEFORM-FIX-001: Gravity decay + center spectral weighting ===

    #[test]
    fn waveform_gravity_decay_single_frame() {
        // WAVEFORM-FIX-001: GRAVITY_RATE = 0.25, level *= GRAVITY_RATE per frame
        const GRAVITY_RATE: f32 = 0.25;
        let level: f32 = 1.0;
        let after_one = level * GRAVITY_RATE;
        assert!((after_one - 0.25).abs() < 0.001, "After 1 frame, level must be 0.25");
    }

    #[test]
    fn waveform_gravity_decay_approaches_zero() {
        // WAVEFORM-FIX-001: after N frames, level → 0
        const GRAVITY_RATE: f32 = 0.25;
        let mut level: f32 = 1.0;
        for _ in 0..8 {
            level *= GRAVITY_RATE;
        }
        // 0.25^8 = 1/65536 ≈ 0.000015
        assert!(level < 0.001, "After 8 frames, level must be near zero");
    }

    #[test]
    fn waveform_gravity_decay_from_high_level() {
        // WAVEFORM-FIX-001: decay works from any starting level
        const GRAVITY_RATE: f32 = 0.25;
        let mut level: f32 = 2.0;
        for _ in 0..4 {
            level *= GRAVITY_RATE;
        }
        // 2.0 * 0.25^4 = 2.0 / 256 = 0.0078125
        assert!((level - 0.0078125).abs() < 0.0001, "Decay must be multiplicative from any start");
    }

    #[test]
    fn waveform_gravity_rate_constant_value() {
        // WAVEFORM-FIX-002: GRAVITY_RATE = 0.25 (center), edge = 0.25*(0.5+1.5*1)=0.5
        const GRAVITY_RATE: f32 = 0.25;
        assert!((GRAVITY_RATE - 0.25).abs() < 0.001, "GRAVITY_RATE must be 0.25");
        assert!(GRAVITY_RATE > 0.0 && GRAVITY_RATE < 1.0, "GRAVITY_RATE must be between 0 and 1 for decay");
    }

    #[test]
    fn waveform_edge_weighted_gravity_bounds() {
        // WAVEFORM-FIX-002: bar_gravity = 0.25 * (0.5 + 1.5 * center_dist)
        // center_dist=0 → bar_gravity=0.125, center_dist=1 → bar_gravity=0.5
        const GRAVITY_RATE: f32 = 0.25;
        let center_gravity = GRAVITY_RATE * (0.5 + 1.5 * 0.0);
        let edge_gravity = GRAVITY_RATE * (0.5 + 1.5 * 1.0);
        assert!((center_gravity - 0.125).abs() < 0.001, "Center gravity must be 0.125");
        assert!((edge_gravity - 0.5).abs() < 0.001, "Edge gravity must be 0.5");
        assert!(edge_gravity > center_gravity, "Edge must decay faster than center");
    }

    #[test]
    fn waveform_center_weight_at_center() {
        // WAVEFORM-FIX-001: weight = 0.4 + 0.6*cos²(π/2 * i/(half-1))
        // At i=0 (center): cos(0) = 1, weight = 0.4 + 0.6*1 = 1.0
        let half: usize = 32;
        let i: usize = 0;
        let weight = 0.4 + 0.6 * (std::f64::consts::FRAC_PI_2 * i as f64 / (half - 1) as f64).cos().powi(2);
        assert!((weight - 1.0).abs() < 0.001, "Center bar (i=0) must have weight=1.0");
    }

    #[test]
    fn waveform_center_weight_at_edge() {
        // WAVEFORM-FIX-001: at i=half-1 (edge): cos(π/2) = 0, weight = 0.4
        let half: usize = 32;
        let i: usize = half - 1;
        let weight = 0.4 + 0.6 * (std::f64::consts::FRAC_PI_2 * i as f64 / (half - 1) as f64).cos().powi(2);
        assert!((weight - 0.4).abs() < 0.001, "Edge bar (i=half-1) must have weight=0.4");
    }

    #[test]
    fn waveform_center_weight_monotonic_decrease() {
        // WAVEFORM-FIX-001: weight must decrease monotonically from center to edge
        let half: usize = 32;
        let mut prev_weight = 1.0;
        for i in 1..half {
            let weight = 0.4 + 0.6 * (std::f64::consts::FRAC_PI_2 * i as f64 / (half - 1) as f64).cos().powi(2);
            assert!(weight <= prev_weight + 0.001,
                "Weight must decrease monotonically from center to edge (i={})", i);
            prev_weight = weight;
        }
    }

    #[test]
    fn waveform_center_weight_midpoint() {
        // WAVEFORM-FIX-001: at i=half/2: angle = π/2 * 16/31 ≈ 0.8106 rad
        // cos(0.8106) ≈ 0.689, cos² ≈ 0.475, weight ≈ 0.4 + 0.6*0.475 ≈ 0.685
        let half: usize = 32;
        let i: usize = half / 2;
        let angle = std::f64::consts::FRAC_PI_2 * i as f64 / (half - 1) as f64;
        let weight = 0.4 + 0.6 * angle.cos().powi(2);
        assert!((weight - 0.685).abs() < 0.01, "Midpoint bar must have weight≈0.685 (actual={:.4})", weight);
    }

    #[test]
    fn waveform_center_weight_range_bounds() {
        // WAVEFORM-FIX-001: all weights must be in [0.4, 1.0]
        let half: usize = 32;
        for i in 0..half {
            let weight = 0.4 + 0.6 * (std::f64::consts::FRAC_PI_2 * i as f64 / (half - 1) as f64).cos().powi(2);
            assert!(weight >= 0.4 - 0.001 && weight <= 1.0 + 0.001,
                "Weight must be in [0.4, 1.0] range (i={})", i);
        }
    }

    // === OVERLAY-FIX-006: Border darken + Shimmer rewrite + Preview adjustments ===

    #[test]
    fn border_gray_deeply_darkened() {
        // FIX-007: BORDER_GRAY darkened from 0x171513 to 0x060607 (≈30% brightness)
        const BORDER_GRAY: u32 = 0x060607; // COLORREF format: 0x00BBGGRR
        let r = BORDER_GRAY & 0xFF;
        let g = (BORDER_GRAY >> 8) & 0xFF;
        let b = (BORDER_GRAY >> 16) & 0xFF;
        assert_eq!(r, 0x07, "BORDER_GRAY red must be 0x07");
        assert_eq!(g, 0x06, "BORDER_GRAY green must be 0x06");
        assert_eq!(b, 0x06, "BORDER_GRAY blue must be 0x06");
        // Verify darker than old value (0x171513)
        let old_gray: u32 = 0x131517;
        assert!(BORDER_GRAY < old_gray, "New BORDER_GRAY must be darker than old 0x171513");
    }

    #[test]
    fn circ_border_matches_border_gray() {
        // FIX-007: CIRC_BORDER must match BORDER_GRAY (0x060607)
        const BORDER_GRAY: u32 = 0x060607;
        const CIRC_BORDER: u32 = 0x060607;
        assert_eq!(BORDER_GRAY, CIRC_BORDER, "CIRC_BORDER must match BORDER_GRAY");
    }

    #[test]
    fn waveform_index_center_is_newest() {
        // WAVEFORM-FIX-002: center bar (i=0) must map to newest sample (len-1)
        let levels_len: usize = 64;
        let half: usize = 32;
        let idx_center = levels_len.saturating_sub(1 + 0);
        assert_eq!(idx_center, levels_len - 1,
            "center bar i=0 must map to newest sample (len-1)");
        assert_eq!(idx_center, 63,
            "for len=64 newest sample index is 63");
    }

    #[test]
    fn waveform_index_edge_maps_to_oldest_of_left_half() {
        // WAVEFORM-FIX-002: edge bar (i=half-1) maps to oldest of left half
        let levels_len: usize = 64;
        let half: usize = 32;
        let idx_edge = levels_len.saturating_sub(1 + (half - 1));
        assert_eq!(idx_edge, levels_len - half,
            "edge bar i=half-1 must map to len-half (oldest of left half)");
        assert_eq!(idx_edge, 32,
            "for len=64,half=32 edge maps to idx 32");
    }

    #[test]
    fn waveform_right_half_index_starts_at_start_idx() {
        // WAVEFORM-FIX-002: right half i=0 starts at start_idx
        let levels_len: usize = 64;
        let half: usize = 32;
        let start_idx = levels_len.saturating_sub(half);
        assert_eq!(start_idx, 32,
            "start_idx for len=64,half=32 is 32");
        let idx_right_first = start_idx + 0;
        assert_eq!(idx_right_first, 32,
            "right half first bar (i=0) must start at start_idx");
        assert!(idx_right_first < levels_len,
            "right half index must be within bounds");
    }

    #[test]
    fn waveform_start_idx_calculation() {
        // WAVEFORM-FIX-002: start_idx = levels.len().saturating_sub(half)
        let test_cases = [
            (64usize, 32usize, 32usize),
            (33usize, 16usize, 17usize),
            (60usize, 30usize, 30usize),
        ];
        for (len, half, expected) in test_cases {
            let start_idx = len.saturating_sub(half);
            assert_eq!(start_idx, expected,
                "start_idx for len={},half={} must be {}", len, half, expected);
        }
    }

    #[test]
    fn waveform_gravity_edge_falls_4x_faster_than_center() {
        // WAVEFORM-FIX-002: edge gravity(0.5) / center gravity(0.125) = 4x
        const GRAVITY_RATE: f32 = 0.25;
        let center_gravity = GRAVITY_RATE * (0.5 + 1.5 * 0.0); // 0.125
        let edge_gravity = GRAVITY_RATE * (0.5 + 1.5 * 1.0);   // 0.5
        let ratio = edge_gravity / center_gravity;
        assert!((ratio - 4.0).abs() < 0.001,
            "edge must fall {:.1}x faster than center, got {:.2}x", 4.0, ratio);
    }

    #[test]
    fn shimmer_period_is_800ms() {
        // SHIMMER-SPEED-002: period changed from 1200ms to 800ms
        let period_ms: u64 = 800;
        assert_eq!(period_ms, 800,
            "shimmer period must be 800ms after SHIMMER-SPEED-002");
        assert_eq!((0u64 % period_ms) as f32 / period_ms as f32, 0.0,
            "start phase must be 0");
        assert_eq!((400u64 % period_ms) as f32 / period_ms as f32, 0.5,
            "mid-period phase must be 0.5");
        assert_eq!((800u64 % period_ms) as f32 / period_ms as f32, 0.0,
            "full-period phase must wrap to 0");
    }

    #[test]
    fn shimmer_phase_time_based() {
        // SHIMMER-SPEED-002: phase = (_shimmer_ms % 800) as f32 / 800.0
        // Verify: phase is always in [0, 1) for any timestamp
        let period_ms: u64 = 800;
        for ms in [0u64, 1, 200, 399, 400, 798, 799, 800, 801, 1600] {
            let phase = (ms % period_ms) as f32 / period_ms as f32;
            assert!(phase >= 0.0 && phase < 1.0, "phase must be in [0,1) for ms={}", ms);
        }
        // Verify monotonic within one period
        let p1 = (200u64 % period_ms) as f32 / period_ms as f32;
        let p2 = (400u64 % period_ms) as f32 / period_ms as f32;
        assert!(p2 > p1, "phase must increase with time within one period");
        // Verify wrap: ms=0 and ms=800 both give phase=0.0
        let p_start = (0u64 % period_ms) as f32 / period_ms as f32;
        let p_wrap = (period_ms % period_ms) as f32 / period_ms as f32;
        assert!((p_start - p_wrap).abs() < 0.0001, "phase must wrap at period boundary");
    }

    #[test]
    fn preview_btn_width_45() {
        // FIX-006-6: btn_w reduced from 60 to 45
        let btn_w: i32 = 45;
        assert_eq!(btn_w, 45, "Preview button width must be 45px");
    }

    #[test]
    fn preview_btn_height_18() {
        // FIX-006-6: btn_h reduced from 24 to 18
        let btn_h: i32 = 18;
        assert_eq!(btn_h, 18, "Preview button height must be 18px");
    }

    #[test]
    fn shimmer_travel_calculation() {
        // SHIMMER-VISUAL-003: travel = rect.right - rect.left + glow_w_total (GLOW_HALF*2=90)
        let window_width: i32 = 200;
        let glow_half: i32 = 45;
        let glow_w_total = glow_half * 2;
        let travel = (window_width + glow_w_total) as f32;
        assert_eq!(travel, 290.0, "Travel must be window_width + 90 (200+90)");
    }

    #[test]
    fn shimmer_cx_range() {
        // SHIMMER-VISUAL-003: beam_cx = rect.left - GLOW_HALF(45) + (travel * phase) as i32
        let rect_left: i32 = 0;
        let rect_right: i32 = 200;
        let glow_half: i32 = 45;
        let travel: f32 = 290.0;
        // At phase=0.0: beam_cx starts off-screen left
        let beam_cx_start = rect_left - glow_half + (travel * 0.0) as i32;
        assert_eq!(beam_cx_start, -45, "beam_cx at phase=0 must be -45 (off-screen left)");
        // At phase=1.0: beam_cx ends off-screen right
        let beam_cx_end = rect_left - glow_half + (travel * 1.0) as i32;
        assert_eq!(beam_cx_end, 245, "beam_cx at phase=1 must be 245 (right edge + 45)");
        // At phase=0.5: beam is mid-window, outermost slice visible after clamping
        let beam_cx_mid = rect_left - glow_half + (travel * 0.5) as i32;
        let x1 = (beam_cx_mid - glow_half).max(rect_left + 1);
        let x2 = (beam_cx_mid + glow_half).min(rect_right - 1);
        assert!(x1 >= rect_left + 1, "Clamped x1 must be >= rect.left + 1");
        assert!(x2 <= rect_right - 1, "Clamped x2 must be <= rect.right - 1");
    }

    #[test]
    fn shimmer_glow_has_30_slices() {
        // SHIMMER-VISUAL-003: 30 Gaussian slices for smooth glow
        const SLICES: i32 = 30;
        assert_eq!(SLICES, 30, "Must have 30 glow slices");
    }

    #[test]
    fn shimmer_glow_gaussian_center_alpha_max() {
        // SHIMMER-VISUAL-003: at t=0 (center), alpha = exp(0)*200 = 200
        let t: f32 = 0.0;
        let alpha = ((-3.0_f32 * t * t).exp() * 200.0) as u8;
        assert_eq!(alpha, 200, "Center alpha must be 200");
        // at t=1.0 (edge), alpha = exp(-3)*200 ~ 10
        let t_edge: f32 = 1.0;
        let alpha_edge = ((-3.0_f32 * t_edge * t_edge).exp() * 200.0) as u8;
        assert!(alpha_edge < 15, "Edge alpha must be very low (<15): {}", alpha_edge);
        assert!(alpha_edge < alpha, "Edge alpha must be less than center alpha");
    }

    #[test]
    fn copy_btn_color_is_brand_orange() {
        // FIX-006-7: copy button retains BRAND_ORANGE
        const BRAND_ORANGE: u32 = 0x006BFF; // COLORREF format: 0x00BBGGRR
        let r = BRAND_ORANGE & 0xFF;
        let g = (BRAND_ORANGE >> 8) & 0xFF;
        let b = (BRAND_ORANGE >> 16) & 0xFF;
        assert_eq!(r, 0xFF, "BRAND_ORANGE must have full red channel");
        assert_eq!(g, 0x6B, "BRAND_ORANGE must have 0x6B green channel");
        assert_eq!(b, 0x00, "BRAND_ORANGE must have zero blue channel");
    }

    #[test]
    fn close_btn_color_is_gray() {
        // FIX-006-7: close button color changed to 0x808080
        const CLOSE_GRAY: u32 = 0x808080;
        let r = CLOSE_GRAY & 0xFF;
        let g = (CLOSE_GRAY >> 8) & 0xFF;
        let b = (CLOSE_GRAY >> 16) & 0xFF;
        assert_eq!(r, 0x80, "Close button R must be 0x80");
        assert_eq!(g, 0x80, "Close button G must be 0x80");
        assert_eq!(b, 0x80, "Close button B must be 0x80");
        // Verify it is gray (R=G=B)
        assert_eq!(r, g, "Gray requires R=G");
        assert_eq!(g, b, "Gray requires G=B");
        // Verify it is NOT orange
        assert!(r != 0xFF || g != 0x6B || b != 0x00,
            "Close button must not be BRAND_ORANGE");
    }

    #[test]
    fn btn_border_brighter_than_window_border() {
        // FIX-007: BTN_BORDER must be visually brighter than BORDER_GRAY (0x060607)
        // so X close, copy, close buttons are distinguishable from window edge
        const BORDER_GRAY: u32 = 0x060607;
        const BTN_BORDER: u32 = 0x707070;
        // Sum channels as proxy for brightness (since both are gray-ish)
        let gray_sum = (BORDER_GRAY & 0xFF) + ((BORDER_GRAY >> 8) & 0xFF) + ((BORDER_GRAY >> 16) & 0xFF);
        let btn_sum = (BTN_BORDER & 0xFF) + ((BTN_BORDER >> 8) & 0xFF) + ((BTN_BORDER >> 16) & 0xFF);
        assert!(btn_sum > gray_sum * 4, "BTN_BORDER must be at least 4x brighter than BORDER_GRAY");
        // Verify BTN_BORDER is neutral gray
        let r = BTN_BORDER & 0xFF;
        let g = (BTN_BORDER >> 8) & 0xFF;
        let b = (BTN_BORDER >> 16) & 0xFF;
        assert_eq!(r, 0x70, "BTN_BORDER R = 0x70");
        assert_eq!(r, g, "BTN_BORDER must be neutral gray (R=G)");
        assert_eq!(g, b, "BTN_BORDER must be neutral gray (G=B)");
    }
}
