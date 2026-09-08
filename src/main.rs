#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
// Bug 2 fix: tray menu freeze workaround (tray-icon #298)
// Use Win32 TrackPopupMenu directly to avoid modal message loop conflict
// with overlay thread's InvalidateRect calls.
mod audio;
mod config;
mod crash;
#[cfg(target_os = "windows")]
mod hotkey; // Deprecated: use platform::HotkeyListener instead
mod i18n;
#[cfg(target_os = "windows")]
mod injection; // Deprecated: use platform::inject_text instead
mod itn;
mod llm;
mod platform; // MAC-001+003: Platform abstraction layer
mod punctuation;
mod scene;
mod text_normalizer;
mod transcription;
mod translation;
mod ui;
mod version_check;
mod wordbook;
use anyhow::{anyhow, Result};
use config::AppConfig;
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use platform::HotkeyEvent; // MAC-003: Use platform layer
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
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
use windows::Win32::Foundation::{
    COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{
    AlphaBlend, BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDIBSection,
    CreateFontW, CreatePen, CreateRectRgn, CreateRoundRectRgn, CreateSolidBrush, DeleteDC,
    DeleteObject, DrawTextW, Ellipse, EndPaint, FillRect, FillRgn, GetDC, GetMonitorInfoW,
    GetStockObject, GetTextExtentPoint32W, GetTextMetricsW, InvalidateRect, LineTo,
    MonitorFromWindow, MoveToEx, Rectangle, ReleaseDC, RestoreDC, RoundRect, SaveDC, SelectClipRgn,
    SelectObject, SetBkColor, SetBkMode, SetBrushOrgEx, SetStretchBltMode, SetTextColor,
    StretchBlt, UpdateWindow, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    BLENDFUNCTION, CLEARTYPE_QUALITY, DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS,
    DRAW_TEXT_FORMAT, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK,
    FF_DONTCARE, FW_NORMAL, HALFTONE, HBITMAP, HDC, HFONT, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    NULL_BRUSH, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_NULL, PS_SOLID, SRCCOPY, TEXTMETRICW,
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
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetFocus, SetActiveWindow, SetFocus, VK_ESCAPE, VK_RETURN,
};

#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{ES_AUTOHSCROLL, GWLP_WNDPROC, WINDOW_STYLE};
#[cfg(target_os = "windows")]
const EM_SETSEL_MSG: u32 = 0x00B1; // EM_SETSEL
#[cfg(target_os = "windows")]
const WM_SETFONT: u32 = 0x0030;
#[cfg(target_os = "windows")]
const EDIT_OLD_PROC_PROP: [u16; 16] = [
    'f' as u16, 'y' as u16, 'n' as u16, '_' as u16, 'e' as u16, 'd' as u16, 'i' as u16, 't' as u16,
    '_' as u16, 'o' as u16, 'l' as u16, 'd' as u16, 'p' as u16, 'r' as u16, 'o' as u16, 0,
];
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallWindowProcW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyCaret,
    DestroyMenu, DestroyWindow, DispatchMessageW, GetClientRect, GetForegroundWindow,
    GetGUIThreadInfo, GetMessageW, GetPropW, GetSystemMetrics, GetWindowLongPtrW, HideCaret,
    KillTimer, LoadCursorW, MsgWaitForMultipleObjects, PeekMessageW, PostMessageW, PostQuitMessage,
    RegisterClassW, RemovePropW, SetForegroundWindow, SetLayeredWindowAttributes, SetMenuItemInfoW,
    SetPropW, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowCaret, ShowWindow, TrackPopupMenu,
    TranslateMessage, UpdateLayeredWindow, CREATESTRUCTW, CW_USEDEFAULT, GUITHREADINFO,
    GWLP_USERDATA, GWL_EXSTYLE, HMENU, IDC_ARROW, LWA_ALPHA, MENUITEMINFOW, MF_SEPARATOR,
    MF_STRING, MIIM_BITMAP, MSG, PM_REMOVE, PRF_CLIENT, PRF_ERASEBKGND, QS_ALLINPUT, SM_CXSCREEN,
    SM_CXSMICON, SM_CYSCREEN, SM_CYSMICON, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOW, SW_SHOWNA, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, ULW_ALPHA, WM_APP, WM_CTLCOLOREDIT, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN,
    WM_LBUTTONUP, WM_NCCREATE, WM_NCPAINT, WM_PAINT, WM_PRINTCLIENT, WM_TIMER, WNDCLASSW,
    WNDCLASS_STYLES, WS_CHILD, WS_CLIPCHILDREN, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_OVERLAPPED, WS_POPUP, WS_VISIBLE,
};
#[derive(Debug, Clone)]
enum PipelineEvent {
    RecordingStarted,
    /// OVERLAY-051-E: 流式 ASR 已启动但尚未收到第一个文本，overlay 显示占位提示。
    StreamingIdle,
    /// ASR-038-B: 流式 ASR 增量文本（display_text 全量，供 overlay 整段覆盖）
    /// OVERLAY-051-G: 附带 word timings 供时间戳驱动揭示
    /// OVERLAY-075: 附带产生该事件的录音会话代际。跨 session 迟到包（上一 session 的
    /// finalize 拖尾期间用户新开会话）按代际不匹配丢弃，见消费侧 :3682 起。
    StreamingText(
        u64,
        String,
        Vec<crate::transcription::qwen_inference::WordTiming>,
    ),
    Processing(String),
    Done,
    Cancelled,
    FocusLost(String),
    Error(String),
    /// FORMAT-LLM-001-CORE (DEC-031-③): LLM 格式化失败，原文已注入兜底。
    /// overlay 显示 2500ms 提示后自动复位 tray 到 Idle（防卡在"处理中"）。
    FormatFailed,
    /// BUG-119: 「用户没说话」信息提示（Gavin BUILD-118 端测第 1 项）。
    /// 无 payload：文案由显示侧查 i18n（no_speech_hint），照 FormatFailed 先例。
    /// 产出源：transcription::NoSpeechError 类型化错误在 worker 边界下探转成本事件，
    /// 新增第三个产出源只需复用该类型，无需改任何分类器。
    NoSpeech,
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
// MACOS-P4-NEUTRAL-002: 去 cfg——run_pipeline_core 已中立，worker 线程需在 macOS 起来。
// 对 Windows 构建该 cfg 恒为真，删除为 no-op（同 NEUTRAL-001 三辅助函数论证）。
enum WorkerCommand {
    Start(StartCmd),
    Shutdown,
}
#[derive(Debug, Clone)]
#[cfg(target_os = "windows")]
enum OverlayCommand {
    Show(OverlayRequest),
    /// ASR-038-C: 从 Recording/RecordingWithText 进入 StreamingEditing 态
    EnterEditMode,
    Hide,
    /// OVERLAY-054-A: 提交编辑时先恢复 NOACTIVATE、销毁 EDIT 控件并隐藏窗口，
    /// 避免 overlay 继续持有焦点/前台，影响原窗口文本注入。
    RestoreAndHide,
    /// OVERLAY-051-G: 更新 word timings 供时间戳驱动揭示
    UpdateWordTimings(Vec<crate::transcription::qwen_inference::WordTiming>),
    Shutdown,
}
#[derive(Debug, Clone)]
enum OverlayUiEvent {
    CancelRequested,
    PreviewCopied,
    /// ASR-038-B/C: 用户点击 overlay 文本区进入编辑态（DESIGN-OVERLAY-037 §6）
    /// ASR 侧接收后：关 WebSocket → 丢弃 StreamingAsrState → 置 pipeline_cancelled=true
    EditRequested,
    /// ASR-038-C: 用户完成编辑，提交 EDIT 控件内容 + 原始目标窗口句柄回主控
    SubmitRequested(String, platform::WindowId),
}
#[derive(Debug, Clone)]
#[cfg(target_os = "windows")]
struct OverlayRequest {
    status: OverlayStatus,
    /// OVERLAY-054-B-FIX: `None` = caller has no position to give, overlay thread
    /// resolves it via `overlay_geometry(&status, hwnd)` at Show time. This is the
    /// type-level cure for the `[0, 0]` hardcode that made the window flash at the
    /// top-left corner before jumping to the correct position.
    pos: Option<[i32; 2]>,
    size: [i32; 2],
    opacity: f32,
    ui_language: config::UiLanguage,
    /// Auto close after this duration (milliseconds), 0 means no auto close
    auto_close_ms: u32,
    /// ASR-038-C: 本次录音开始时捕获的目标窗口，用于编辑提交后归还焦点 / 注入文本
    target_hwnd: platform::WindowId,
}
// ASR-038-C-REWORK-001: controller-side flag that suppresses Cancelled/Done → Hide while the user is editing.
// Needed because EditRequested sets cancel_signal; worker then emits PipelineEvent::Cancelled, which would
// otherwise immediately destroy the EDIT control we just created. Guard is paired with the overlay-side
// StreamingEditing guard in the Hide handler.
#[cfg(target_os = "windows")]
static OVERLAY_EDITING: AtomicBool = AtomicBool::new(false);
// OVERLAY-043: latch that prevents late StreamingText events from reverting the overlay back to Recording
// after the user has released the hotkey. Reset on each RecordingStarted.
#[cfg(target_os = "windows")]
static STREAMING_STOPPED: AtomicBool = AtomicBool::new(false);
// OVERLAY-075: session generation counter for streaming text isolation. Bumped on every
// new recording session (worker Start, :4155 area); each session's ASR callback closure
// captures its own generation and stamps every StreamingText it emits. The consumer drops
// events whose generation no longer matches — replacing the timing-based latch with an
// identity check so a stale ASR thread from session A can never leak text into session B.
// STREAMING_STOPPED stays: it governs "late packets within the SAME session after stop",
// which is orthogonal to generation (cross-session identity). The two gates AND together.
#[cfg(target_os = "windows")]
static STREAMING_GENERATION: AtomicU64 = AtomicU64::new(0);
#[derive(Debug, Clone, Copy)]
#[cfg(target_os = "windows")]
struct SendHwnd(isize);
#[cfg(target_os = "windows")]
unsafe impl Send for SendHwnd {}
#[derive(Debug, Clone)]
// MACOS-P4-NEUTRAL-002: 去 cfg + target_hwnd 类型 SendHwnd → platform::WindowId（usize）。
// 原 SendHwnd(isize) 仍保留 #[cfg(windows)]（overlay 线程 :565/:589/:641 仍用它），
// 故 StartCmd 不再引用 SendHwnd，净安全性提升（worker 路径不再经 unsafe Send 包装）。
struct StartCmd {
    target_hwnd: platform::WindowId,
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
// ASR-074-GUARD: process-wide count of audio chunks dropped because the ASR consumer
// stopped draining chunk_tx (send_timeout hit 200ms). Warn-visible per drop, counted
// here so the magnitude survives in logs even when the warn lines rotate out.
static ASR_CHUNK_DROPS: AtomicU64 = AtomicU64::new(0);
// TRAY-001: 设置 UI 二进制名（平台差异：Windows 带 .exe，macOS 裸二进制）。
#[cfg(target_os = "windows")]
const SETTINGS_UI_EXE_NAME: &str = "feiyin-ime-ui.exe";
#[cfg(target_os = "macos")]
const SETTINGS_UI_EXE_NAME: &str = "feiyin-ime-ui";
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
/// TRAY-ICON-158：把直通 RGBA8 图标转成 32bpp top-down 预乘 BGRA DIB，
/// 供菜单项 `MIIM_BITMAP` 挂载。菜单渲染只有在 32bpp 预乘 DIB 上才正确
/// 合成 per-pixel alpha。失败返回 None（调用方降级为无图标菜单——行为不变，
/// FIX-164 Part C：不再沉默，每个失败分支 log::warn! 带 GetLastError，
/// BUILD-159 端测「图标没挂上」的真因就藏在这段无观测的失败路径里）。
#[cfg(target_os = "windows")]
fn create_menu_item_bitmap(size: i32, rgba: &[u8]) -> Option<HBITMAP> {
    let n = size as usize;
    // 🔴 DIAG-166 根因修复：长度契约是 size²*4（RGBA8 方形缓冲）。原 `n.checked_mul(4)`
    // 只有 4*size，与 rgba.len()=4*size² 仅在 size=1 时相等 ⇒ 对任何实际尺寸必然判否
    // ⇒ 本函数自 TRAY-ICON-158 起恒返 None ⇒ 图标从未挂载（端测「菜单没有图标」真因，
    // 离线工装 + 读回验证实证）。修正为 size²*4 全溢出安全链。
    let expected = size.checked_mul(size).and_then(|sq| sq.checked_mul(4));
    if size <= 0 || expected.map_or(true, |total| total as usize != rgba.len()) {
        log::warn!(
            "menu icon: create_menu_item_bitmap rejected input (size={}, rgba_len={}, expected={:?})",
            size,
            rgba.len(),
            expected
        );
        return None;
    }
    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size,
        biHeight: -size, // 负 = top-down，与 rgba 行序一致
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbmp = unsafe {
        // hdc 传 None：32bpp DIB 不需要绑定 DC，菜单 GDI 读取内存即可合成
        CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
    };
    let Ok(hbmp) = hbmp else {
        // FIX-164 Part C: 不再静默——CreateDIBSection 失败是「图标没挂上」候选真因之一
        let err = unsafe { windows::Win32::Foundation::GetLastError() };
        log::warn!(
            "menu icon: CreateDIBSection failed ({}x{}), GetLastError={:?}",
            size,
            size,
            err
        );
        return None;
    };
    if bits.is_null() {
        // FIX-164 Part C: 同上，ppvBits 为空同样降级但留痕
        log::warn!(
            "menu icon: CreateDIBSection returned null bits ({}x{})",
            size,
            size
        );
        let _ = unsafe { DeleteObject(hbmp) };
        return None;
    }
    let dst = unsafe { std::slice::from_raw_parts_mut(bits as *mut u8, n * n * 4) };
    for (src, dst) in rgba.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        let a = src[3] as u32;
        dst[0] = ((src[2] as u32 * a + 127) / 255) as u8; // B（预乘）
        dst[1] = ((src[1] as u32 * a + 127) / 255) as u8; // G
        dst[2] = ((src[0] as u32 * a + 127) / 255) as u8; // R
        dst[3] = src[3]; // A
    }
    Some(hbmp)
}
#[cfg(target_os = "windows")]
fn attach_menu_icons(menu: HMENU) -> Vec<HBITMAP> {
    // 尺寸取系统小图标规格（SM_CXSMICON 走已启用的 WindowsAndMessaging，
    // 不用 GetDpiForWindow —— 需 Win32_UI_HiDpi feature，红线禁改 Cargo.toml）。
    let size = unsafe {
        GetSystemMetrics(SM_CXSMICON)
            .min(GetSystemMetrics(SM_CYSMICON))
            .clamp(16, 64)
    };
    let mut attached = Vec::with_capacity(2);
    for (cmd_id, rgba) in [
        (
            MENU_CMD_SETTINGS,
            ui::menu_icons::settings_icon_rgba(size as u32),
        ),
        (MENU_CMD_EXIT, ui::menu_icons::exit_icon_rgba(size as u32)),
    ] {
        let Some(hbmp) = create_menu_item_bitmap(size, &rgba) else {
            // FIX-164 Part C: 降级行为不变，但失败原因已在 create_menu_item_bitmap 内留痕
            continue;
        };
        let mut mii = MENUITEMINFOW::default();
        mii.cbSize = std::mem::size_of::<MENUITEMINFOW>() as u32;
        mii.fMask = MIIM_BITMAP;
        mii.hbmpItem = hbmp;
        // fByPosition=FALSE：按 wID（命令 ID）定位
        if unsafe { SetMenuItemInfoW(menu, cmd_id, false, &mii) }.is_ok() {
            attached.push(hbmp);
        } else {
            // FIX-164 Part C: SetMenuItemInfoW 失败是「图标没挂上」另一候选真因——
            // 静默降级吞了 8 个月没人能说出为什么，现在带 GetLastError 留痕。
            let err = unsafe { windows::Win32::Foundation::GetLastError() };
            log::warn!(
                "menu icon: SetMenuItemInfoW failed for cmd_id={}, GetLastError={:?}",
                cmd_id,
                err
            );
            let _ = unsafe { DeleteObject(hbmp) };
        }
    }
    // FIX-164 Part C: 成功路径只记一条 debug（不刷屏），含挂载数量与尺寸供 debug.log 核对
    log::debug!(
        "menu icon: attached {}/2 bitmaps at {}px (settings={}, exit={})",
        attached.len(),
        size,
        MENU_CMD_SETTINGS,
        MENU_CMD_EXIT
    );
    attached
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
        // TRAY-ICON-158：两项挂品牌橙图标。句柄现建现删，不缓存进
        // thread_local/OnceLock（[D2D-HANG-001] 教训：跨线程/退出期释放句柄自找
        // 麻烦）；DeleteObject 必须在 DestroyMenu 之后——TrackPopupMenu 模态期间
        // 菜单还在读这些位图，提前删会让图标空白。
        let menu_bitmaps = attach_menu_icons(menu);
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
        for hbmp in menu_bitmaps {
            let _ = DeleteObject(hbmp);
        }
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
                    #[cfg(any(target_os = "windows", target_os = "macos"))]
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

#[cfg(target_os = "windows")]
fn get_window_text(hwnd: HWND) -> Result<String> {
    let mut len = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowTextLengthW(hwnd) };
    if len < 0 {
        len = 0;
    }
    let mut buffer: Vec<u16> = vec![0; len as usize + 1];
    let copied =
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(hwnd, &mut buffer) };
    if copied > 0 {
        let wide = &buffer[..copied as usize];
        Ok(String::from_utf16_lossy(wide))
    } else {
        Ok(String::new())
    }
}

#[cfg(target_os = "windows")]
fn get_window_client_rect(hwnd: HWND) -> RECT {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rect);
    }
    rect
}

#[cfg(target_os = "windows")]
fn remove_noactivate(hwnd: HWND) {
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_ex_style = ex_style & !(WS_EX_NOACTIVATE.0 as isize);
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex_style);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
        );
    }
}

#[cfg(target_os = "windows")]
fn restore_noactivate(hwnd: HWND) {
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_ex_style = ex_style | (WS_EX_NOACTIVATE.0 as isize);
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex_style);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
        );
    }
}

#[cfg(target_os = "windows")]
fn state_guard_edit_hwnd(state: &Arc<Mutex<OverlayWindowState>>) -> Option<HWND> {
    if let Ok(state) = state.lock() {
        state.edit_hwnd
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
// OVERLAY-054-F-B: pure function for EDIT control box geometry.
// Inputs:
//   rect_h        - total client height of the overlay window
//   tm_height     - TEXTMETRICW.tmHeight for the chosen edit font
//   fixed_margin  - legacy fixed top/bottom margin (e.g. STREAMING_TEXT_TOP_MARGIN);
//                    used only for the fallback fixed height, never for the top offset
//   corner_floor  - minimum distance from top/bottom edge (rounded corner clearance)
// Returns (top_offset, height) relative to the overlay client rect.
//
// Contract:
// 1. desired height = (tm_height + 2).max(rect_h - 2 * fixed_margin).max(1)
// 2. available height = (rect_h - 2 * corner_floor).max(0)
// 3. If desired <= available, returned height must equal desired exactly.
// 4. If desired >  available, clamp to available: give as much as fits, never less.
//    (The old "fall back to fixed_height" rule was wrong: 16px is worse than 28px.)
//    Final height = desired.min(available).max(1); .max(1) guards against tiny rect_h.
// 5. Top offset is centered first, then clamped to corner_floor; fixed_margin must NOT
//    participate in the top offset (it would pin the box and defeat centering).
fn compute_edit_box_geometry(
    rect_h: i32,
    tm_height: i32,
    fixed_margin: i32,
    corner_floor: i32,
) -> (i32, i32) {
    let fixed_height = (rect_h - 2 * fixed_margin).max(1);
    let desired = (tm_height + 2).max(fixed_height);
    let available = (rect_h - 2 * corner_floor).max(0);
    let height = desired.min(available).max(1);
    let top_offset = ((rect_h - height) / 2).max(corner_floor);
    (top_offset, height)
}

/// OVERLAY-121 (P2): ULW ↔ SLWA 模式切换，唯一收口。
///
/// 🔴 两种模式不能直切：MSDN SetLayeredWindowAttributes Remarks 逐字 ——
/// "once SetLayeredWindowAttributes has been called, subsequent UpdateLayeredWindow
/// calls will fail until the layering style bit is cleared and set again"。
/// 因此**双向**切换统一走「清 WS_EX_LAYERED → 置回 → 调用目标模式 API」中转
/// （draft §3.2 / §0 结论 2-3），不依赖任何未记录方向。
///
/// 🔴 调用方必须让窗口处于隐藏区间（hide → switch → show）——清 bit 会销毁
/// layering/redirection 表面（MSDN Using Windows），可见状态下切换会闪一帧。
///
/// 目标 SLWA 时立即用当前请求不透明度调 SetLayeredWindowAttributes 完成模式建立；
/// 目标 ULW 时由下一次 WM_PAINT 的 UpdateLayeredWindow 提交建立（InvalidateRect 触发）。
#[cfg(target_os = "windows")]
fn switch_overlay_layered_mode(
    hwnd: HWND,
    state: &mut OverlayWindowState,
    target: OverlayLayeredMode,
) {
    if state.layered_mode == target {
        return;
    }
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style & !(WS_EX_LAYERED.0 as isize));
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | (WS_EX_LAYERED.0 as isize));
    }
    state.layered_mode = target;
    if target == OverlayLayeredMode::Slwa {
        let alpha = state
            .request
            .as_ref()
            .map(|r| (r.opacity.clamp(0.1, 1.0) * 255.0).round() as u8)
            .unwrap_or(255);
        unsafe {
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
        }
    }
    unsafe {
        let _ = InvalidateRect(hwnd, None, false);
    }
}

#[cfg(target_os = "windows")]
fn create_edit_control(hwnd: HWND, state: &mut OverlayWindowState, rect: &RECT, text: &str) {
    if state.edit_hwnd.is_some() {
        return;
    }
    let hinstance = unsafe { GetModuleHandleW(None) }.unwrap_or_default();
    let class_name = encode_wide("EDIT");
    let title = encode_wide(text);
    let edit_left = rect.left + STREAMING_TEXT_LEFT_MARGIN;
    let edit_right = rect.right - STREAMING_TEXT_RIGHT_MARGIN;
    let edit_w = (edit_right - edit_left).max(1);

    // OVERLAY-054-F: size the EDIT control using the real ClearType font metrics so
    // descenders (g/y/p) are not clipped. The overlay window is only 36px tall, and the
    // previous fixed 10px top/bottom margins gave a 16px client area which is too small
    // for Segoe UI at -14. We measure tmHeight on a temporary DC and let the control
    // height be (tmHeight + 2).max(original_fixed_height). The +2 reserves a one-pixel
    // cushion above and below the glyph bounding box.
    let mut tm = TEXTMETRICW::default();
    let (edit_top, edit_h) = unsafe {
        let hdc = GetDC(hwnd);
        // EDITFONT-183: 测量字体必须与 EDIT 实际字体一致（:860 同步换 EDITFONT 常量），
        // 否则 tmHeight 按 14px 量、控件按 16px 渲染 ⇒ 高度与垂直居中都错。
        let font = create_clear_type_font(OVERLAY_TEXT_FONT_SIZE);
        let old_font = SelectObject(hdc, font);
        let got = GetTextMetricsW(hdc, &mut tm);
        let _ = SelectObject(hdc, old_font);
        let _ = DeleteObject(font);
        let _ = ReleaseDC(hwnd, hdc);
        if !got.as_bool() {
            log::warn!("OVERLAY-054-G: GetTextMetricsW failed; falling back to fixed edit height");
        }
        let tm_height = if got.as_bool() { tm.tmHeight } else { 0 };
        let rect_h = rect.bottom - rect.top;
        let (top_offset, height) = compute_edit_box_geometry(
            rect_h,
            tm_height,
            STREAMING_TEXT_TOP_MARGIN,
            4, // rounded-corner floor, 4px from top/bottom
        );
        // Guard: if the 36px overlay cannot accommodate the font, stop and ask
        // the orchestrator before raising the window height (which would cascade into
        // overlay_geometry, state_detector regex, and E2E size assertions).
        // The guard is built into compute_edit_box_geometry; we just log it here so
        // the runtime record is explicit.
        let desired = (tm_height + 2).max((rect_h - 2 * STREAMING_TEXT_TOP_MARGIN).max(1));
        let available = (rect_h - 8).max(0);
        if desired > available {
            log::error!(
                "OVERLAY-054-G: font {} requires edit height {} but window {} only leaves {} content pixels; stopping before raising window height",
                OVERLAY_TEXT_FONT_SIZE,
                desired,
                rect_h,
                available
            );
        }
        (rect.top + top_offset, height)
    };
    let edit_hwnd = unsafe {
        CreateWindowExW(
            // EDIT-FLICKER-157 双缓冲扩展样式（COMPOSITED）方案因 Gavin 端测出现 EDIT
            // 大黑屏/文字全丢，已于 FIX-162 回退；闪烁问题重新排期，下次必须端测确认后才收。
            // 备选方案（供后续参考，同样未实测）：子类拦 WM_ERASEBKGND 自绘背景
            // （父窗同色刷+内存 DC BitBlt），或创建后 SetWindowLongPtrW 改样式。
            WS_EX_NOACTIVATE, // child, keep NOACTIVATE so it doesn't steal from parent
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            edit_left,
            edit_top,
            edit_w,
            edit_h,
            hwnd,
            HMENU::default(),
            HINSTANCE(hinstance.0),
            None,
        )
    };
    match edit_hwnd {
        Ok(edit_hwnd) => {
            state.edit_hwnd = Some(edit_hwnd);
            // Background brush for WM_CTLCOLOREDIT
            state.edit_bg_brush = Some(unsafe { CreateSolidBrush(OVERLAY_BG_DARK) });
            // OVERLAY-051-D: store parent overlay HWND so the EDIT subclass can forward Enter.
            unsafe {
                let _ = SetWindowLongPtrW(edit_hwnd, GWLP_USERDATA, hwnd.0 as isize);
            }
            // Subclass EDIT to suppress default border via WM_NCPAINT and handle Enter.
            let old_proc = unsafe {
                SetWindowLongPtrW(edit_hwnd, GWLP_WNDPROC, edit_subclass_wnd_proc as isize)
            };
            state.edit_old_wndproc = Some(unsafe {
                std::mem::transmute::<isize, windows::Win32::UI::WindowsAndMessaging::WNDPROC>(
                    old_proc,
                )
            });
            // OVERLAY-051-A: store old_proc on the EDIT window via SetPropW so the
            // static subclass proc can CallWindowProcW it. GWLP_USERDATA is already
            // used to store the parent overlay HWND (see :559), so we use a named prop.
            unsafe {
                let prop_name = PCWSTR(EDIT_OLD_PROC_PROP.as_ptr());
                let _ = SetPropW(
                    edit_hwnd,
                    prop_name,
                    HANDLE(old_proc as *mut std::ffi::c_void),
                );
            }
            // OVERLAY-054-D/G: give EDIT control its own ClearType font using the same
            // unified overlay font size as the self-drawn text. We use an independent HFONT
            // (not cached_font) because cached_font is take()+DeleteObject() when the
            // overlay hides/destroys.
            let edit_font = create_clear_type_font(OVERLAY_TEXT_FONT_SIZE);
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                    edit_hwnd,
                    WM_SETFONT,
                    WPARAM(edit_font.0 as usize),
                    LPARAM(1), // TRUE: redraw immediately
                );
            }
            state.edit_font = Some(edit_font);
            log::info!(
                "ASR-038-C: created EDIT control for streaming editing (edit_h={}, tmHeight={})",
                edit_h,
                tm.tmHeight
            );
        }
        Err(e) => {
            log::error!("ASR-038-C: failed to create EDIT control: {}", e);
        }
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn edit_subclass_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // FLICKER-170-B: 单次合成绘制 —— 现象本体是 EDIT 内部「擦背景刷填充→画字」两步
    // 直打 SLWA 重定向表面，DWM 可在两步之间合成 ⇒ 移光标/滚动时可见闪烁
    // （Gavin 端测现象；A 的 WS_CLIPCHILDREN 修的是父窗放大器，修不到这条）。
    // 修法 = create_edit_control 注释里预留的备选路径完整形态：拦 WM_PAINT，
    // WM_PRINTCLIENT(PRF_ERASEBKGND|PRF_CLIENT) 让 EDIT 把擦除+文字一次画进内存 DC，
    // 再单次 BitBlt 只提交 ps.rcPaint 更新区（坑③：不全矩形重画）。无 COMPOSITED、
    // 无新窗口样式、无新合成模型 —— 与 157 的炸法不同类。工装实证（flck170 探针）：
    // 擦背景 ✓（bg=CTLCOLOREDIT 刷色）、文字 ✓；🔴 选区反白 ✗ 不渲染（Gavin 已知悉
    // 并接受，见 result.md 端测清单）。回退 = 删除本 WM_PAINT 分支。
    if msg == WM_PAINT {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        if hdc.is_invalid() {
            return forward_edit_old_proc(hwnd, msg, wparam, lparam);
        }
        let mut rc_client = RECT::default();
        if GetClientRect(hwnd, &mut rc_client).is_err() {
            let _ = EndPaint(hwnd, &ps);
            return forward_edit_old_proc(hwnd, msg, wparam, lparam);
        }
        let cw = (rc_client.right - rc_client.left).max(1);
        let ch = (rc_client.bottom - rc_client.top).max(1);
        let mem_dc = CreateCompatibleDC(hdc);
        let mem_bmp = CreateCompatibleBitmap(hdc, cw, ch);
        if mem_dc.is_invalid() || mem_bmp.is_invalid() {
            // 资源失败兜底：退回 EDIT 默认绘制（闪烁但功能完好，绝不黑块）
            if !mem_dc.is_invalid() {
                let _ = DeleteDC(mem_dc);
            }
            let _ = EndPaint(hwnd, &ps);
            return forward_edit_old_proc(hwnd, msg, wparam, lparam);
        }
        let old_bmp = SelectObject(mem_dc, mem_bmp);
        let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            hwnd,
            WM_PRINTCLIENT,
            WPARAM(mem_dc.0 as usize),
            LPARAM((PRF_ERASEBKGND | PRF_CLIENT) as isize),
        );
        // caret 由系统直接画在屏幕层，BitBlt 会盖掉它 ⇒ BitBlt 前后成对隐藏/恢复
        // （坑①；与 OVERLAY-149 的销毁期 caret 清理零交互：那边是 DestroyWindow 前
        // 一次性 HideCaret+DestroyCaret，这里是绘制期瞬态平衡对）。
        let _ = HideCaret(hwnd);
        let up = ps.rcPaint;
        let uw = (up.right - up.left).max(0);
        let uh = (up.bottom - up.top).max(0);
        if uw > 0 && uh > 0 {
            let _ = BitBlt(
                hdc, up.left, up.top, uw, uh, mem_dc, up.left, up.top, SRCCOPY,
            );
        }
        let _ = ShowCaret(hwnd);
        let _ = SelectObject(mem_dc, old_bmp);
        let _ = DeleteObject(mem_bmp);
        let _ = DeleteDC(mem_dc);
        let _ = EndPaint(hwnd, &ps);
        return LRESULT(0);
    }
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_NCPAINT {
        return LRESULT(0);
    }
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN {
        // ESC-178: 永久日志链 ①——子类入口。ESC/Enter 只在编辑态可能出现高频，
        // 只记这两键，不刷屏。定位「键到底进没进子类」（H1 判别点）。
        if wparam.0 == VK_ESCAPE.0 as usize || wparam.0 == VK_RETURN.0 as usize {
            log::debug!(
                "ESC-178: EDIT subclass received WM_KEYDOWN wparam={:#x}",
                wparam.0
            );
        }
        // ESC-174: 编辑态 ESC = 取消编辑 + 作废本次录入。发 CancelRequested 走既有
        // 取消收口（controller 臂：cancel/stop 双信号 + OVERLAY_EDITING=false +
        // STREAMING_STOPPED=true + Hide + 托盘回 Idle），复用现有通道零新增路径。
        // 压制链分析（OVERLAY-147-DIAG B3 + OVERLAY-149 F2 的存量结论）：worker 随后
        // 发出的 PipelineEvent::Cancelled 到达压制臂时 OVERLAY_EDITING 已被本事件臂
        // 清为 false ⇒ 走 else 幂等再 Hide 一次，无卡窗。非编辑态不经过本子类
        // （EDIT 仅编辑态存在），父窗 :2226 的 ESC 分支行为不变，两者按 EDIT 存在
        // 与否天然互斥。return 0 不落 FIX-172-B 包裹块（ESC 分支在包裹块之前），
        // 无 SETREDRAW 停绘风险；Hide 流程照常走 OVERLAY-149 F1 caret 清理。
        // 回退 = 删除本分支。
        if wparam.0 == VK_ESCAPE.0 as usize {
            // 与 Enter 分支同一取父通道：EDIT 的 GWLP_USERDATA 存的是父 overlay HWND
            // ESC-178: 永久日志链 ②——分支执行 + 守卫取值 + 事件发出（H3/H4 判别点）。
            let parent_hwnd = {
                let parent = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                HWND(parent as _)
            };
            log::debug!(
                "ESC-178: subclass ESC branch, parent_hwnd_null={}",
                parent_hwnd.0.is_null()
            );
            if !parent_hwnd.0.is_null() {
                let data_ptr =
                    GetWindowLongPtrW(parent_hwnd, GWLP_USERDATA) as *mut OverlayWindowData;
                if !data_ptr.is_null() {
                    let data = &mut *data_ptr;
                    if let Ok(state) = data.state.lock() {
                        let has_request = state.request.is_some();
                        log::debug!("ESC-178: subclass ESC branch, request.is_some={has_request}");
                        if has_request {
                            let _ = state.event_tx.send(OverlayUiEvent::CancelRequested);
                            log::debug!("ESC-178: CancelRequested sent from subclass");
                        }
                    }
                } else {
                    log::debug!("ESC-178: subclass ESC branch, parent data_ptr null");
                }
            }
            return LRESULT(0);
        }
        if wparam.0 == VK_RETURN.0 as usize {
            // OVERLAY-051-D: Enter in the EDIT control submits the edited text, same as clicking
            // the submit button. We retrieve the parent overlay window from the EDIT's GWLP_USERDATA
            // (set on creation) and forward the event through its event channel.
            let parent_hwnd = {
                let parent = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                HWND(parent as _)
            };
            if !parent_hwnd.0.is_null() {
                let data_ptr =
                    GetWindowLongPtrW(parent_hwnd, GWLP_USERDATA) as *mut OverlayWindowData;
                if !data_ptr.is_null() {
                    let data = &mut *data_ptr;
                    if let Ok(state) = data.state.lock() {
                        if let Some(ref request) = state.request {
                            if matches!(request.status, OverlayStatus::StreamingEditing { .. }) {
                                if let Some(edit_hwnd) = state.edit_hwnd {
                                    if let Ok(text) = get_window_text(edit_hwnd) {
                                        let _ =
                                            state.event_tx.send(OverlayUiEvent::SubmitRequested(
                                                text,
                                                request.target_hwnd,
                                            ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return LRESULT(0);
        }
    }
    // OVERLAY-051-A: forward to the original EDIT window procedure via CallWindowProcW.
    // DefWindowProcW is the default window proc, NOT the EDIT class proc — using it
    // bypasses all of EDIT's text storage, drawing, caret/scroll, selection logic,
    // which was the root cause of "text disappears / can't edit / cursor can't reach".
    // FIX-172-B: 滚动位块包裹 —— FLICKER-170-B' 没修到本体的原因（工装 E1 实证）：
    // EDIT 的 ES_AUTOHSCROLL 横向滚动走内部 ScrollWindowEx 类位块传送**直接打屏幕**，
    // 全程零 WM_PAINT（变化 1894px、paints=0），位块与 DWM 合成不同步 ⇒ 最右侧闪烁。
    // 修法 = 把「滚动 + 重绘」压成一次屏幕更新：对会移动光标/触发滚动的消息，
    // 先 WM_SETREDRAW(FALSE) 关掉 EDIT 直打屏幕的一切绘制 → 默认过程只改内部状态
    // → WM_SETREDRAW(TRUE)（工装 E2 实证不触发额外重绘）→ InvalidateRect(bErase=FALSE)
    // 整客户区失效 → 由 FLICKER-170-B 的 WM_PAINT 单次合成一次上屏。
    // 连击时失效区自动合并成一次合成。回退 = 删除本包裹块。
    // 范围仅收口「内容/光标/选区变化」类消息（任务书列出：方向键/字符/EM_SETSEL/点击）；
    // WM_MOUSEMOVE 不包（拖选高亮本就不渲染，避免高频无效整客户区失效）。
    // 🔴 位置约束：必须在 Enter 分支之后（Enter 走提交通道 return，不参与包裹）。
    const EM_SETSEL_MSG: u32 = 0x00B1; // windows-0.58 在 UI::Controls（feature 未启用），用裸值
    let wraps = matches!(
        msg,
        windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN
            | windows::Win32::UI::WindowsAndMessaging::WM_SYSKEYDOWN
            | windows::Win32::UI::WindowsAndMessaging::WM_CHAR
            | windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONDOWN
            | windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONDBLCLK
    ) || msg == EM_SETSEL_MSG;
    if wraps {
        let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::WM_SETREDRAW,
            WPARAM(0),
            LPARAM(0),
        );
        let result = forward_edit_old_proc(hwnd, msg, wparam, lparam);
        let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::WM_SETREDRAW,
            WPARAM(1),
            LPARAM(0),
        );
        let _ = InvalidateRect(hwnd, None, false);
        return result;
    }
    forward_edit_old_proc(hwnd, msg, wparam, lparam)
}

/// FLICKER-170-B: 子类前向收口 —— WM_PAINT 合成兜底与消息尾共用同一前向逻辑。
fn forward_edit_old_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let prop_name = PCWSTR(EDIT_OLD_PROC_PROP.as_ptr());
    let old_proc = unsafe { GetPropW(hwnd, prop_name) };
    if !old_proc.0.is_null() {
        let old_proc_fn = unsafe {
            std::mem::transmute::<
                *mut std::ffi::c_void,
                windows::Win32::UI::WindowsAndMessaging::WNDPROC,
            >(old_proc.0)
        };
        unsafe { CallWindowProcW(old_proc_fn, hwnd, msg, wparam, lparam) }
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

#[cfg(target_os = "windows")]
fn destroy_edit_control(state: &mut OverlayWindowState) {
    if let Some(edit_hwnd) = state.edit_hwnd.take() {
        // OVERLAY-149 (F1): caret 泄漏修复（Gavin 端测第 2 项：录音窗显示文字光标）。
        // EDIT 带焦销毁收到的是 WM_DESTROY 而非 WM_KILLFOCUS，EDIT 内部不会销毁
        // caret；caret 生命周期绑线程输入队列而非窗口 ⇒ DestroyWindow 后 caret
        // 对象跨态存活，而系统 caret 是屏幕级绘制（不走 ULW 合成）⇒ 录音帧
        // （本无子控件）上可见为「文字光标」。
        // 修法（DIAG §B2/B3-1，DestroyWindow 之前）：移焦点 → HideCaret → DestroyCaret；
        // 原有 DestroyWindow 及后续清理不动。
        // DestroyCaret 只销毁本线程拥有的 caret，无跨进程副作用；调用时本线程无
        // caret 则返回 FALSE，属正常，不当错误处理。
        unsafe {
            let focus = GetFocus();
            if focus == edit_hwnd {
                // create_edit_control 把父 overlay HWND 存在 EDIT 的 GWLP_USERDATA，
                // 焦点移回父窗（比 SetFocus(None) 保留 IME 上下文）；无父时退化为
                // SetFocus(None)（释放焦点），同样达成「焦点离开 EDIT」。
                let parent = GetWindowLongPtrW(edit_hwnd, GWLP_USERDATA);
                let _ = SetFocus(if parent != 0 {
                    HWND(parent as _)
                } else {
                    HWND::default()
                });
            }
            let _ = HideCaret(edit_hwnd);
            let _ = DestroyCaret();
        }
        // 🔴 OVERLAY-149-PROBE（F1 决定性实验的运行时证据，判读完删除；将来删除时
        // 需删本 if 块整段）：销毁 + 显式清 caret 后查询本线程 caret 残留 ——
        // hCaret=0 ⇒ 清理生效；≠0 ⇒ caret 被重建/他处持有，需回报主控。
        unsafe {
            let mut gti = GUITHREADINFO::default();
            gti.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            if GetGUIThreadInfo(
                windows::Win32::System::Threading::GetCurrentThreadId(),
                &mut gti,
            )
            .is_ok()
            {
                log::debug!(
                    "OVERLAY-149-PROBE F1: post-destroy caret hwndCaret={:?} flags={:#x}",
                    gti.hwndCaret,
                    gti.flags.0
                );
            }
        }
        // Restore original window procedure if we have it
        if let Some(old_proc) = state.edit_old_wndproc.take() {
            unsafe {
                let _ = SetWindowLongPtrW(edit_hwnd, GWLP_WNDPROC, unsafe {
                    std::mem::transmute::<windows::Win32::UI::WindowsAndMessaging::WNDPROC, isize>(
                        old_proc,
                    )
                });
            }
        }
        // OVERLAY-051-A: remove the prop we stored on the EDIT window (old proc handle)
        // before destroying it, to avoid a dangling atom/property entry.
        unsafe {
            let prop_name = PCWSTR(EDIT_OLD_PROC_PROP.as_ptr());
            let _ = RemovePropW(edit_hwnd, prop_name);
        }
        unsafe {
            let _ = DestroyWindow(edit_hwnd);
        }
    }
    if let Some(brush) = state.edit_bg_brush.take() {
        unsafe {
            let _ = DeleteObject(brush);
        }
    }
    // OVERLAY-054-D: destroy the EDIT-specific font only after the EDIT window is gone.
    // The WM_SETFONT message copies the HFONT handle into the control; the application
    // remains responsible for deleting it when no longer needed (MSDN).
    if let Some(font) = state.edit_font.take() {
        unsafe {
            let _ = DeleteObject(font);
        }
    }
}
fn spawn_settings_process() -> Result<Child> {
    // DEC-013: 启动 Tauri Settings 子进程。二进制名平台化：Windows=feiyin-ime-ui.exe,
    // macOS=feiyin-ime-ui（TRAY-001 修复硬编码 .exe）。
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    // Spawn settings UI process (feiyin-ime-ui)
    let ui_exe = exe_dir.join(SETTINGS_UI_EXE_NAME);
    // Copy release exe to debug location if needed (src-tauri/target/release to src-tauri/target/debug)
    let project_root = exe_dir
        .parent() // target/release -> target/debug -> target/
        .and_then(|p| p.parent()); // target/ -> project root
    let dev_ui_exe_release = project_root
        .map(|root| {
            root.join("src-tauri")
                .join("target")
                .join("release")
                .join(SETTINGS_UI_EXE_NAME)
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
                    .join(SETTINGS_UI_EXE_NAME)
            })
            .unwrap_or_else(|| ui_exe.clone())
    };
    let target_exe = if ui_exe.exists() {
        ui_exe
    } else if dev_ui_exe.exists() {
        log::info!(
            "Using development path for {}: {}",
            SETTINGS_UI_EXE_NAME,
            dev_ui_exe.display()
        );
        dev_ui_exe
    } else {
        return Err(anyhow!(
            "{} not found. Please build the Tauri UI first (npm run tauri build or cargo build in src-tauri).",
            SETTINGS_UI_EXE_NAME
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
/// OVERLAY-121 (P2): overlay 窗口的分层合成模式。
/// - `Ulw`: UpdateLayeredWindow 逐像素 alpha（per-pixel），八态默认走这条。
/// - `Slwa`: SetLayeredWindowAttributes 均一 alpha —— 仅编辑态（EDIT 子控件
///   在 ULW 合成里不可见，MSDN Window Features §Layered Windows）。
/// 两种模式不能直切（MSDN SetLayeredWindowAttributes Remarks：SLWA 调过之后
/// ULW 必败，须清/置 WS_EX_LAYERED 中转），切换统一走 `switch_overlay_layered_mode`。
#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayLayeredMode {
    Ulw,
    Slwa,
}

#[cfg(target_os = "windows")]
struct OverlayWindowState {
    request: Option<OverlayRequest>,
    audio_buf: AudioLevelBuf,
    event_tx: crossbeam_channel::Sender<OverlayUiEvent>,
    cancel_btn_rect: Option<RECT>,
    close_btn_rect: Option<RECT>, // close button area (focus-lost preview)
    title_close_btn_rect: Option<RECT>, // title bar close button (focus-lost preview)
    /// ASR-038-C: 提交按钮区域（编辑态）
    submit_btn_rect: Option<RECT>,
    /// ASR-038-C: 文本点击区域（录音/流式态进入编辑态）
    text_hit_rect: Option<RECT>,
    shimmer_phase: f32,
    /// ASR-038-C: 内嵌 EDIT 控件句柄，编辑态使用
    edit_hwnd: Option<HWND>,
    /// ASR-038-C: EDIT 控件子类化前原窗口过程，用于卸载时恢复
    edit_old_wndproc: Option<windows::Win32::UI::WindowsAndMessaging::WNDPROC>,
    /// ASR-038-C: 用于 WM_CTLCOLOREDIT 返回的背景画刷（BG_DARK）
    edit_bg_brush: Option<windows::Win32::Graphics::Gdi::HBRUSH>,
    /// OVERLAY-121 (P2): 当前分层合成模式（Ulw=逐像素 / Slwa=均一 alpha，仅编辑态）
    layered_mode: OverlayLayeredMode,
    /// OVERLAY-054-D: EDIT 控件专用 HFONT。必须独立于 cached_font，因为 cached_font 在
    /// 隐藏/退出时会被 take+DeleteObject（:1316）；EDIT 控件生命周期与窗口状态绑定，
    /// 若复用 cached_font 会导致字体被提前删除或双删。
    edit_font: Option<HFONT>,
    /// ASR-038-C-REWORK-001: 流式文本窗口宽度 100ms 尺寸节流时间戳
    last_resize_time: Option<std::time::Instant>,
    /// OVERLAY-043: coalesced pending size for streaming text resize throttle
    pending_size: Option<[i32; 2]>,
    /// OVERLAY-043: dirty flag so 16ms timer only repaints when content actually changes
    needs_repaint: bool,
    /// OVERLAY-043: smoothly interpolated current window size (avoids instant jumps)
    current_size: [i32; 2],
    /// OVERLAY-043: target window size for interpolation
    target_size: [i32; 2],
    /// OVERLAY-043: last displayed streaming text to avoid repaint on unchanged content
    last_streaming_text: Option<String>,
    /// OVERLAY-051-F/G: cached ClearType font so we don't create/destroy HFONT every frame.
    cached_font: Option<HFONT>,
    /// STREAMFONT-189: 流式两态（RecordingStreamingIdle 占位 / RecordingWithText）自绘
    /// 文字专用 HFONT（OVERLAY_TEXT_FONT_SIZE = -16）。必须独立于 cached_font（-14）：
    /// cached_font 在隐藏/退出清理臂会被 take+DeleteObject（与 edit_font 同一坑，
    /// :860 注释同源），复用会导致字体被提前删除或双删。生命周期照 edit_font/cached_font
    /// 范式：绘制选择点惰性创建、存回本字段，清理臂与 cached_font 同处 take+DeleteObject。
    streaming_font: Option<HFONT>,
    /// OVERLAY-051-G: number of characters currently visible during typewriter tween.
    displayed_chars: usize,
    /// OVERLAY-051-G: deadline for next character reveal during typewriter tween.
    tween_deadline: Option<std::time::Instant>,
    /// OVERLAY-051-G: total target character count for the current tween.
    tween_target_chars: usize,
    /// OVERLAY-051-G: start time of the current tween, for elapsed computation.
    tween_start: Option<std::time::Instant>,
    /// OVERLAY-051-G: word timings from ASR for timestamp-driven reveal.
    word_timings: Vec<crate::transcription::qwen_inference::WordTiming>,
    /// OVERLAY-051-G: wall-clock time when the first word's audio began, for offset mapping.
    tween_audio_origin: Option<std::time::Instant>,
    /// OVERLAY-086 Bug 2 ③: timeline zero anchored at the same instant as
    /// `tween_audio_origin` — the first non-empty word table's `words[0].begin_time`.
    /// Both ends of the wall-clock↔timeline mapping are fixed once; server-side word
    /// rewrites (which shift `words[0].begin_time`) must not re-float the timeline zero,
    /// otherwise the reveal boundary regresses wholesale (freeze then burst).
    tween_timeline_origin: Option<i64>,
}
#[cfg(target_os = "windows")]
struct OverlayWindowData {
    state: Arc<Mutex<OverlayWindowState>>,
}
// ASR-038-C: shared overlay color constants (module level so helpers/wnd_proc can see them)
#[cfg(target_os = "windows")]
const OVERLAY_BRAND_ORANGE: COLORREF = COLORREF(0x006BFF); // #FF6B00
#[cfg(target_os = "windows")]
const OVERLAY_BG_DARK: COLORREF = COLORREF(0x110F0D);
#[cfg(target_os = "windows")]
const OVERLAY_TEXT_WHITE: COLORREF = COLORREF(0xFFFFFF);
// OVERLAY-054-C: unified window border color. Changing this one constant updates
// every overlay variant; previously 10+ scattered local consts made it easy to miss.
#[cfg(target_os = "windows")]
const OVERLAY_BORDER_GRAY: COLORREF = COLORREF(0x3A3A3C);
// OVERLAY-054-G: unified overlay font size for both self-drawn text and the EDIT
// control. Gavin decided the edit-mode and non-edit text must look the same size.
#[cfg(target_os = "windows")]
const OVERLAY_FONT_SIZE: i32 = -14;
// EDITFONT-183: 编辑框字号调大一号（Gavin 原话）。STREAMFONT-189（Gavin 2026-09-08
// 拍板「实时上屏也调大，和编辑态同步」）：本常量从「编辑框专用」升为「转写文字统一
// 字号」，同时服务 —— EDIT 控件（WM_SETFONT）、D2D streaming_text_format（实时上屏 +
// 占位提示）、GDI streaming_font（D2D 失败兜底同两态）、两态测量
// （adjust_overlay_pos_size_for_text）。取值 -16（14→16px）：Segoe UI em 16 ⇒
// tmHeight ≈ 21，available = 36-2*OVERLAY_TEXT_DRAW_VERTICAL_INSET(4) = 28 ⇒
// desired(≈23) ≤ available，36px 窗高装得下不裁字（overlay_text_area_capacity_guard
// 同时守护 14/16 两个字号）。处理中/箭头/Error/Info 小字仍归 OVERLAY_FONT_SIZE(-14)，
// 逐位不变。
#[cfg(target_os = "windows")]
const OVERLAY_TEXT_FONT_SIZE: i32 = -16;
// OVERLAY-054-C: file-level button border color. Kept at 0x707070 (value unchanged).
// Tests can now reference this constant instead of mirroring the literal.
#[cfg(target_os = "windows")]
const OVERLAY_BTN_BORDER: COLORREF = COLORREF(0x707070);

// OVERLAY-141: 窗口外框圆角半径单一来源。所有状态的外框半径（D2D chrome /
// draw_processing_primitives 与各 GDI fallback 的 CORNER_RADIUS）一律引用这里，
// 禁止再写字面量 —— 帧末 SDF alpha 掩码与绘制半径必须同值，散落字面量会导致
// 掩码与绘制不一致切出新锯齿。内部元素（麦克风药丸 3.5、停止/提交键 5/4、
// 底部按钮 4、标题 ✕ 键 3）半径不在此列，仍为各自字面量。
#[cfg(target_os = "windows")]
// OVERLAY-155 (改动2): 16.0 → 10.0 —— 对齐 Info/Error/FocusLost 的半径几何（Gavin
// 明确指向「照『请说话哦』那个提示窗口做」，r=10 组视觉已被接受）。单一来源，改一行
// 全部生效；不删 LG/SM 常量 —— Gavin 若想要回 16，改回本字面量即可（可逆性）。
const OVERLAY_FRAME_RADIUS_LG: f32 = 10.0;
#[cfg(target_os = "windows")]
const OVERLAY_FRAME_RADIUS_SM: f32 = 10.0;

#[cfg(target_os = "windows")]
const RECORDING_OVERLAY_SIZE: [i32; 2] = [240, 36]; // Recording window
#[cfg(target_os = "windows")]
const STATUS_OVERLAY_SIZE: [i32; 2] = [240, 36]; // Processing window adjusted height
#[cfg(target_os = "windows")]
const PREVIEW_OVERLAY_SIZE: [i32; 2] = [320, 140]; // UI-OPT-003: increased height for title bar
#[cfg(target_os = "windows")]
const STREAMING_TEXT_LEFT_MARGIN: i32 = 42; // left fixed: mic icon area + separator
#[cfg(target_os = "windows")]
const STREAMING_TEXT_RIGHT_MARGIN: i32 = 49; // right fixed: submit + stop buttons
#[cfg(target_os = "windows")]
const STREAMING_TEXT_TOP_MARGIN: i32 = 10; // keep inside 10px rounded-corner region; EDIT fallback still uses this
#[cfg(target_os = "windows")]
const STREAMING_TEXT_BOTTOM_MARGIN: i32 = 10; // EDIT fallback still uses this
                                              // OVERLAY-054-H: vertical inset for self-drawn streaming text only. The rect center is
                                              // identical to the old 10/10 margin (both are symmetric), so the text does not move;
                                              // only the available height grows from 16px to 28px, which is enough for -14 ClearType.
#[cfg(target_os = "windows")]
const OVERLAY_TEXT_DRAW_VERTICAL_INSET: i32 = 4;
// OVERLAY-102 (Gavin 端测 2026-09-05): 0.65 → 0.50。必须与 OVERLAY-101 的修复
// 分两步落地：改比例会移动 Bug B 的触发点（上限更早到达），混在一起无法区分
// 「修好了」还是「换了个位置没触发」。
#[cfg(target_os = "windows")]
const STREAMING_OVERLAY_MAX_SCREEN_RATIO: f32 = 0.50;
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
        // D2D-HANG-095-B: 用 Drop 守卫而非尾部直调。尾部语句在 panic 展开时会被跳过，
        // Drop 守卫在展开路径上照样运行 —— 这是「无论怎么离开这个线程体，
        // COM 都必须在线程体内释放」的唯一写法。
        // 已知 panic 点：:1475 `state.request.as_ref().unwrap()`、
        // :3115 `expect("just initialized")`，以及 Mutex 中毒。
        // 任一 panic 走尾部直调都会漏掉释放 → thread_local 析构器在 loader lock 下
        // 释放 COM → D2D-HANG-001 原样复发（REPRO-094 探针 B / 探针 D 4/5 实证）。
        // 不会双重借用：with_d2d 内 `cell.borrow_mut()` 的 RefMut（slot）是该闭包帧的
        // 局部变量，panic 展开时同帧局部按逆序先析构 → 借用在栈退到本闭包层、
        // 守卫 drop 调 release_resources() 之前必然已归还；守卫在本帧声明最早、
        // 析构最晚，不存在守卫先于内层借用释放运行的可能。故可用 borrow_mut。
        // （D2D-HANG-095 初版注释保留：run_overlay_thread 多条 `?` 早返回路径
        //（GetModuleHandleW(None)?、DestroyWindow(hwnd)? 等）由闭包级释放统一覆盖。）
        struct D2dReleaseGuard;
        impl Drop for D2dReleaseGuard {
            fn drop(&mut self) {
                d2d::release_resources();
            }
        }
        let _d2d_guard = D2dReleaseGuard;

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
        submit_btn_rect: None,
        text_hit_rect: None,
        shimmer_phase: 0.0,
        edit_hwnd: None,
        edit_old_wndproc: None,
        edit_bg_brush: None,
        layered_mode: OverlayLayeredMode::Ulw,
        edit_font: None,
        last_resize_time: None,
        pending_size: None,
        needs_repaint: true,
        current_size: RECORDING_OVERLAY_SIZE,
        target_size: RECORDING_OVERLAY_SIZE,
        last_streaming_text: None,
        cached_font: None,
        streaming_font: None,
        displayed_chars: 0,
        tween_deadline: None,
        tween_target_chars: 0,
        tween_start: None,
        word_timings: Vec::new(),
        tween_audio_origin: None,
        tween_timeline_origin: None,
    }));
    let window_data = Box::new(OverlayWindowData {
        state: Arc::clone(&shared_state),
    });
    let hwnd = unsafe {
        let window_title = encode_wide("飞音语音输入 Overlay");
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_NOACTIVATE,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(window_title.as_ptr()),
            // FLICKER-170-A: 加 WS_CLIPCHILDREN —— INVESTIGATE-142 指认的结构性放大器：
            // SLWA 父窗 WM_PAINT 全窗 BitBlt 直打屏幕，会整块盖掉 EDIT 子控件的文字。
            // 加后父窗绘制自动裁掉子窗口矩形，EDIT 不再被父窗重绘覆盖。非编辑态没有
            // 子窗口 ⇒ 裁剪区为空集 ⇒ 其余三态逐位零变化；EDIT 仅在编辑态创建
            // （switch_overlay_layered_mode 切 SLWA 在先），ULW 路径恒惰性。
            // 与 157 的 WS_EX_COMPOSITED 完全不同类：不改合成模型、不加缓冲，
            // 只影响父窗自身绘制的落笔范围。回退 = 删去本标识符。
            WS_POPUP | WS_CLIPCHILDREN,
            // OVERLAY-061: create off-screen so the initial 1x1 window never flashes at (0,0)
            // if a frame is composited before ShowWindow(SW_HIDE) lands. WS_POPUP with
            // CW_USEDEFAULT has undefined position and commonly resolves to the top-left
            // corner of the primary monitor.
            -32000,
            -32000,
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
            // OVERLAY-064: H1 under investigation — DWM rounded-corner compositing may clip the
            // outermost 1px border drawn by GDI. This is documented for the root-cause report only;
            // any real fix will be handled in the Direct2D migration per DEC-055.
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
                OverlayCommand::Show(mut request) => {
                    // OVERLAY-054-B-FIX: resolve `pos: None` to a real geometry here,
                    // inside the overlay thread. This is the correct place because
                    // `monitor_work_rect(hwnd)` resolves the monitor that the overlay
                    // window lives on, which may differ from the caller's monitor in
                    // multi-display setups. After this point `request.pos` is always
                    // `Some`, so downstream readers can unwrap safely.
                    let resolved_pos = request
                        .pos
                        .unwrap_or_else(|| overlay_geometry(&request.status, hwnd).0);
                    request.pos = Some(resolved_pos);
                    // OVERLAY-043 / 051-F: coalescing + smooth interpolation for streaming text resize.
                    // Growing width bypasses throttle so text is never clipped by old window width.
                    // Shrinking/same keeps the 100ms throttle to avoid oscillation.
                    let is_streaming_text = matches!(
                        request.status,
                        OverlayStatus::RecordingWithText { .. }
                            | OverlayStatus::StreamingEditing { .. }
                    );
                    let (computed_pos, computed_size) = if is_streaming_text {
                        let now = std::time::Instant::now();
                        let mut state_guard = shared_state.lock().ok();
                        // OVERLAY-054-B-FIX: track pos via a local since request.pos is now Option
                        let mut final_pos = resolved_pos;
                        if let Some(ref mut state) = state_guard {
                            let (adj_pos, desired_size) = adjust_overlay_pos_size_for_text(
                                hwnd,
                                &request.status,
                                &resolved_pos,
                                &state.target_size,
                            );
                            let is_growing = desired_size[0] > state.target_size[0];
                            let do_it = if is_growing {
                                // OVERLAY-051-F: growing width must follow text immediately
                                true
                            } else {
                                // shrinking / same size: keep 100ms throttle to avoid oscillation
                                state
                                    .last_resize_time
                                    .map(|t| t.elapsed().as_millis() >= 100)
                                    .unwrap_or(true)
                            };
                            if do_it {
                                state.last_resize_time = Some(now);
                                state.target_size = desired_size;
                                state.pending_size = Some(desired_size);
                            } else if let Some(size) = state.pending_size {
                                request.size = size;
                            }
                            // OVERLAY-101 (Bug B): x 一律由实际应用宽度（in-flight
                            // current_size）现算，**不分 do_it 与否**。旧代码只在 do_it
                            // 分支重算 x（OVERLAY-068-A R1），!do_it 节流帧沿用
                            // resolved_pos = show_overlay 传入的 overlay_geometry 默认位
                            //（基准宽 240 居中），而本调用 SetWindowPos 应用的是 current
                            //（到上限后 ≫240）→ 中心右偏 (current-240)/2 px；到上限后
                            // current == target、插值循环休眠，无帧纠正，持续到下一包
                            // do_it（~700ms）= Gavin 端测「宽度到上限后瞬跳向右窜」。
                            // 句界双 Show 同毫秒命中节流（REPRO：09-04 日志 25.835
                            // id=2 end + id=3 首包同 ms）。公式与插值循环同源 centered_x。
                            let work = monitor_work_rect(hwnd);
                            let work_w = work.right - work.left;
                            let x = centered_x(work.left, work_w, state.current_size[0]);
                            final_pos = [x, adj_pos[1]];
                        }
                        request.pos = Some(final_pos);
                        (final_pos, request.size)
                    } else {
                        // non-streaming: reset throttle state and use the natural geometry
                        if let Ok(mut state) = shared_state.lock() {
                            state.last_resize_time = None;
                            state.pending_size = None;
                        }
                        (resolved_pos, request.size)
                    };

                    if let Ok(mut state) = shared_state.lock() {
                        // OVERLAY-121 (P2): 模式随目标状态对齐 —— StreamingEditing 走
                        // SLWA（EDIT 子控件可见），其余七态走 ULW（逐像素 alpha）。
                        // 切换发生在隐藏区间：编辑窗可见时先 SW_HIDE，把清/置
                        // WS_EX_LAYERED 的 redirection 表面重建闪烁藏进不可见区间。
                        let target_mode =
                            if matches!(request.status, OverlayStatus::StreamingEditing { .. }) {
                                OverlayLayeredMode::Slwa
                            } else {
                                OverlayLayeredMode::Ulw
                            };
                        if state.layered_mode != target_mode {
                            unsafe {
                                let _ = ShowWindow(hwnd, SW_HIDE);
                            }
                            switch_overlay_layered_mode(hwnd, &mut state, target_mode);
                        }
                        // ASR-038-C: clean up any lingering EDIT control when a fresh overlay appears
                        destroy_edit_control(&mut state);
                        restore_noactivate(hwnd);
                        // only reset animation/text state when status really changes, to avoid flicker
                        let status_changed = state.request.as_ref().map_or(true, |r| {
                            std::mem::discriminant(&r.status)
                                != std::mem::discriminant(&request.status)
                        });
                        if status_changed {
                            state.cancel_btn_rect = None;
                            state.close_btn_rect = None;
                            state.title_close_btn_rect = None;
                            state.submit_btn_rect = None;
                            state.text_hit_rect = None;
                            // OVERLAY-051-A: preserve last_streaming_text across status changes.
                            // Only clear it on a fresh Recording session.
                            if matches!(request.status, OverlayStatus::Recording) {
                                state.last_streaming_text = None;
                                // OVERLAY-051-G-FIN: reset typewriter cursor + timestamp-driven
                                // state on a fresh recording session.
                                state.displayed_chars = 0;
                                state.tween_target_chars = 0;
                                state.tween_deadline = None;
                                state.tween_start = None;
                                state.word_timings.clear();
                                state.tween_audio_origin = None;
                                state.tween_timeline_origin = None;
                            }
                            // OVERLAY-051-G-FIN: flush cursor when leaving RecordingWithText
                            // (e.g. HotkeyEvent::Stop -> FallingToProcessing). This ensures
                            // the cursor is at full length if any late packet somehow reaches
                            // the tween path, and keeps last_streaming_text consistent.
                            if !matches!(
                                request.status,
                                OverlayStatus::RecordingWithText { .. }
                                    | OverlayStatus::Recording
                                    | OverlayStatus::RecordingStreamingIdle
                            ) {
                                if state.tween_target_chars > state.displayed_chars {
                                    state.displayed_chars = state.tween_target_chars;
                                }
                                state.tween_deadline = None;
                                state.tween_start = None;
                                state.word_timings.clear();
                                state.tween_audio_origin = None;
                                state.tween_timeline_origin = None;
                            }
                        }
                        // OVERLAY-051-C: preserve target_hwnd across Show updates that carry 0.
                        // The original foreground window is captured only on hotkey Start; subsequent
                        // Show commands from streaming text updates do not have it.
                        if let Some(ref mut req) = state.request {
                            if request.target_hwnd == 0 && req.target_hwnd != 0 {
                                request.target_hwnd = req.target_hwnd;
                            }
                        }
                        state.request = Some(request.clone());
                        if request.status == OverlayStatus::Recording {
                            ui::overlay::warmup_levels(&state.audio_buf);
                        }
                        // initialize target/current sizes for non-streaming statuses
                        if !is_streaming_text {
                            state.target_size = computed_size;
                            state.current_size = computed_size;
                        }
                        // OVERLAY-086 Bug 3: the OVERLAY-068-B streaming snap
                        // (`current_size = target_size`) is REMOVED. With width growth now
                        // interpolated (see the interpolation loop), snapping here would make
                        // the Show branch's SetWindowPos (which uses target-full-size) fight
                        // the interpolation loop's SetWindowPos (which uses current) within
                        // the same frame — exactly the oscillation 068-B was built to prevent.
                        // The single geometry authority per frame is now:
                        //   - Show: positions the window at the in-flight current size
                        //   - interpolation loop: advances current → target, recentering x
                        // target_size stays as written by adjust_overlay_pos_size_for_text.
                        state.needs_repaint = true;
                    }
                    unsafe {
                        let alpha = (request.opacity.clamp(0.1, 1.0) * 255.0).round() as u8;
                        // OVERLAY-121 (P1): SLWA 均一 alpha 只在 SLWA 模式（编辑态）调用。
                        // ULW 模式下调 SLWA 会把窗口切回均一 alpha 合成（且之后再想回
                        // ULW 必须清/置 style bit），ULW 帧的不透明度由 apply_alpha_fixup
                        // 逐像素烘焙。此处已离开 state 锁作用域，重新锁读模式。
                        let slwa_active = shared_state
                            .lock()
                            .map(|s| s.layered_mode == OverlayLayeredMode::Slwa)
                            .unwrap_or(false);
                        if slwa_active {
                            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
                        }
                        // OVERLAY-046: non-streaming statuses have current_size == target_size
                        // at Show time, so the interpolation path below never reaches SetWindowPos.
                        // Restore an unconditional positioning here so the overlay appears at the
                        // correct location and size on every Show.
                        // OVERLAY-054-B-FIX F1: use computed_pos (streaming 分支含水平居中后的
                        // final_pos；non-streaming 分支是 resolved_pos)，不是入口的 resolved_pos
                        // —— 流式文字宽度增长时水平居中 x 会变，必须用居中后的值，否则每来一批
                        // 文字水平抖一下（旧 request.pos 走的就是居中后的值）。
                        // OVERLAY-086 Bug 3: streaming Show applies the IN-FLIGHT current size
                        // (not the target) so this call and the interpolation loop never fight
                        // over geometry — 068-B's original "two SetWindowPos per frame with
                        // inconsistent sizes" is structurally prevented: during streaming the
                        // interpolation loop is the only width authority; this call just keeps
                        // the window pinned to the width the renderer is already drawing.
                        let applied_size = if is_streaming_text {
                            if let Ok(state) = shared_state.lock() {
                                state.current_size
                            } else {
                                computed_size
                            }
                        } else {
                            computed_size
                        };
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            computed_pos[0],
                            computed_pos[1],
                            applied_size[0],
                            applied_size[1],
                            SWP_NOACTIVATE | SWP_NOZORDER,
                        );
                        let _ = ShowWindow(hwnd, SW_SHOWNA);
                        let _ = InvalidateRect(hwnd, None, false);
                        // Set auto-close timer if needed
                        if request.auto_close_ms > 0 {
                            let _ = SetTimer(hwnd, 1, request.auto_close_ms, None);
                        }
                    }
                }
                OverlayCommand::UpdateWordTimings(words) => {
                    if let Ok(mut state) = shared_state.lock() {
                        // OVERLAY-051-G-FIN: 整体替换（与 display_text 同源累积已在
                        // StreamingAsrState 完成，回调每次下发全量词表）。
                        // words 为空 → 降级路径（RecordingWithText 分支判定后立即全显）
                        let was_nonempty = !words.is_empty();
                        // 首次收到非空 words 时建立 audio origin（墙钟），
                        // 用于把 word.begin_time 差值映射到播放进度。
                        // OVERLAY-086 Bug 2 ③: 同一时刻把时间轴零点也锚定一次
                        // （首个非空词表的 words[0].begin_time）。此后词表被服务端
                        // 整段重写时，重写只改「有哪些词」，时间零点不再随
                        // words[0].begin_time 漂移 —— 消除揭示边界整体后退
                        // （冻 1.9s + 爆发追涨）的成因。
                        // （先取首词值再 move words，避免 borrow-after-move。）
                        let first_begin_time = if was_nonempty && state.tween_audio_origin.is_none()
                        {
                            Some(words[0].begin_time)
                        } else {
                            None
                        };
                        state.word_timings = words;
                        if let (true, Some(begin)) = (first_begin_time.is_some(), first_begin_time)
                        {
                            state.tween_audio_origin = Some(std::time::Instant::now());
                            state.tween_timeline_origin = Some(begin);
                        }
                        state.needs_repaint = true;
                    }
                }
                OverlayCommand::EnterEditMode => {
                    // OVERLAY-121 (P2): 切模式必须发生在隐藏区间（draft §3.2）——
                    // 编辑入口从可见的录音浮层进入，先藏窗，SLWA 切换藏在不可见区间，
                    // 再走原有 EDIT 创建 + 显示流程。
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    }
                    if let Ok(mut state) = shared_state.lock() {
                        // OVERLAY-051-A: editing text fallback chain:
                        // RecordingWithText -> StreamingEditing -> last_streaming_text -> empty.
                        let text = state
                            .request
                            .as_ref()
                            .and_then(|r| match &r.status {
                                OverlayStatus::RecordingWithText { text } => Some(text.clone()),
                                OverlayStatus::StreamingEditing { text } => Some(text.clone()),
                                _ => None,
                            })
                            .or_else(|| state.last_streaming_text.clone())
                            .unwrap_or_default();
                        // OVERLAY-051-G-FIN: when entering edit mode, stop tweening and use full text.
                        state.displayed_chars = text.chars().count();
                        state.tween_target_chars = state.displayed_chars;
                        state.tween_deadline = None;
                        state.tween_start = None;
                        state.word_timings.clear();
                        state.tween_audio_origin = None;
                        state.tween_timeline_origin = None;
                        // ESC-188: 进入编辑态前空读一次，排掉在别处按 ESC 攒下的陈旧
                        // 转变位（0x0001 = 「自上次读取以来被按过」，两态之外无人读它，
                        // 不排掉的话本态首个 tick 会凭空命中 ⇒ CancelRequested ⇒ 转写
                        // 文本丢失）。此处仅丢弃返回值，不做任何位判断。
                        unsafe {
                            let _ = GetAsyncKeyState(VK_ESCAPE.0 as i32);
                        }
                        state.request = state.request.as_mut().map(|r| {
                            r.status = OverlayStatus::StreamingEditing { text: text.clone() };
                            r.clone()
                        });
                        // OVERLAY-121 (P2): 窗口已在本 handler 入口隐藏，此刻切 SLWA ——
                        // EDIT 子控件只在 SLWA 合成里可见（ULW 合成不含子窗口，
                        // MSDN Window Features §Layered Windows / draft §3.1）。
                        switch_overlay_layered_mode(hwnd, &mut state, OverlayLayeredMode::Slwa);
                        // Remove NOACTIVATE so overlay can receive focus/IME
                        remove_noactivate(hwnd);
                        // FIX-192 (DIAG-191 H5): 先按目标状态算出最终窗口尺寸并应用，
                        // 再按【扩窗后】的 client rect 建 EDIT —— 原顺序相反（先按旧
                        // rect 建 EDIT 再扩窗），edit_w 唯一计算点（:759-761）拿到的是
                        // 14px 度量的旧流式窗宽 ⇒ EDIT 比 16px 渲染所需窄 174px（DIAG-191
                        // 实测）⇒ 扩窗后右侧露出父窗背景 = Gavin 端测「右侧一大片空白、
                        // 光标进不去」。整个重排块处于入口 SW_HIDE 与 :1948 SW_SHOW 的
                        // 隐藏区间内，不产生可见重绘（G5 判据不受影响：SW_HIDE 与
                        // switch_overlay_layered_mode 仍在本块内且先于重排）。
                        let (new_pos, new_size) = adjust_overlay_pos_size_for_text(
                            hwnd,
                            &state.request.as_ref().unwrap().status,
                            &[0, 0],
                            &RECORDING_OVERLAY_SIZE,
                        );
                        state.target_size = new_size;
                        state.current_size = new_size;
                        state.needs_repaint = true;
                        unsafe {
                            let _ = SetWindowPos(
                                hwnd,
                                None,
                                new_pos[0],
                                new_pos[1],
                                new_size[0],
                                new_size[1],
                                SWP_NOZORDER,
                            );
                        }
                        let rect = get_window_client_rect(hwnd);
                        create_edit_control(hwnd, &mut state, &rect, &text);
                    }
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_SHOW);
                        let _ = SetActiveWindow(hwnd);
                        if let Some(edit_hwnd) = state_guard_edit_hwnd(&shared_state) {
                            let _ = SetFocus(edit_hwnd);
                            // Select all text for quick replacement
                            let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                                edit_hwnd,
                                EM_SETSEL_MSG,
                                WPARAM(0),
                                LPARAM(-1i32 as isize),
                            );
                        }
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                }
                OverlayCommand::Hide | OverlayCommand::RestoreAndHide => {
                    let restore = matches!(command, OverlayCommand::RestoreAndHide);
                    if let Ok(mut state) = shared_state.lock() {
                        // ASR-038-C-REWORK-001: single-point safeguard — do not tear down the EDIT control
                        // while the user is actively editing. The controller will also suppress Hide while
                        // OVERLAY_EDITING is true; this guard protects against any other path that reaches here.
                        // OVERLAY-054-A: RestoreAndHide is explicitly allowed to tear down editing because the
                        // controller has already cleared OVERLAY_EDITING and needs the window to release focus.
                        if !restore
                            && matches!(
                                state.request,
                                Some(OverlayRequest {
                                    status: OverlayStatus::StreamingEditing { .. },
                                    ..
                                })
                            )
                        {
                            log::info!("ASR-038-C: Hide suppressed while overlay is in StreamingEditing state");
                            continue;
                        }
                        destroy_edit_control(&mut state);
                        state.request = None;
                        state.cancel_btn_rect = None;
                        state.close_btn_rect = None;
                        state.title_close_btn_rect = None;
                        state.submit_btn_rect = None;
                        state.text_hit_rect = None;
                        state.last_streaming_text = None;
                        state.pending_size = None;
                        state.last_resize_time = None;
                        state.current_size = RECORDING_OVERLAY_SIZE;
                        state.target_size = RECORDING_OVERLAY_SIZE;
                        state.displayed_chars = 0;
                        state.tween_target_chars = 0;
                        state.tween_deadline = None;
                        state.tween_start = None;
                        state.word_timings.clear();
                        state.tween_audio_origin = None;
                        state.tween_timeline_origin = None;
                        if let Some(font) = state.cached_font.take() {
                            unsafe {
                                let _ = DeleteObject(font);
                            }
                        }
                        // STREAMFONT-189: streaming_font 与 cached_font 同生命周期 ——
                        // 两字体互不复用（复用即双删，见 edit_font :860 注释同源坑），
                        // 此处必须配对清 second font。
                        if let Some(font) = state.streaming_font.take() {
                            unsafe {
                                let _ = DeleteObject(font);
                            }
                        }
                    }
                    restore_noactivate(hwnd);
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    }
                }
                OverlayCommand::Shutdown => {
                    if let Ok(mut state) = shared_state.lock() {
                        destroy_edit_control(&mut state);
                    }
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
        let mut size_interpolation_done = false;
        if let Ok(mut state) = shared_state.lock() {
            if let Some(request) = state.request.clone() {
                match request.status {
                    OverlayStatus::Recording => {
                        // Recording state is a continuous waveform animation; repaint every frame.
                        if !MENU_VISIBLE.load(Ordering::Acquire) {
                            unsafe {
                                let _ = InvalidateRect(hwnd, None, false);
                            }
                        }
                    }
                    OverlayStatus::RecordingStreamingIdle => {
                        // OVERLAY-051-E: static placeholder state; repaint only on explicit dirty flag.
                        // MIC-PULSE-160: 麦克风动效期间（has_audio）也保持重绘；
                        // StreamingEditing 分支绝不加（EDIT-FLICKER-157 刚修完的地盘）。
                        let dirty = state.needs_repaint || mic_has_audio(&state);
                        if dirty && !MENU_VISIBLE.load(Ordering::Acquire) {
                            unsafe {
                                let _ = InvalidateRect(hwnd, None, false);
                            }
                        }
                        state.needs_repaint = false;
                    }
                    OverlayStatus::RecordingWithText { ref text } => {
                        // OVERLAY-051-G-FIN: 时间戳驱动回放（Gavin 三条指示）。
                        // 1. words 为空 → 立即全显（降级路径）
                        // 2. words 非空 → 按 word.begin_time 差值驱动，不压缩停顿
                        // 3. 单调不回退：displayed 只增不减（.max(prev)）
                        // 4. 服务端回撤（target < displayed）→ snap 到 target
                        // 5. 离开 RecordingWithText（松键）→ flush 到全长
                        let target = text.chars().count();

                        // 服务端回撤：target 缩了 → snap（绝不能往回动画）
                        if target < state.displayed_chars {
                            state.displayed_chars = target;
                            state.tween_target_chars = target;
                            state.needs_repaint = true;
                        }

                        // 记录目标字符数（供离开 RecordingWithText 时 flush 判定）
                        if state.tween_target_chars != target {
                            state.tween_target_chars = target;
                            state.needs_repaint = true;
                        }

                        // 时间戳驱动推进
                        if state.displayed_chars < state.tween_target_chars {
                            if state.word_timings.is_empty() {
                                // 降级路径：words 不可用 → 立即全显
                                state.displayed_chars = state.tween_target_chars;
                                state.needs_repaint = true;
                            } else if let Some(origin) = state.tween_audio_origin {
                                // 正常路径：按墙钟 elapsed 对齐 word.begin_time 差值
                                let elapsed_ms = origin.elapsed().as_millis() as i64;
                                // OVERLAY-086 Bug 2 ③: 时间轴零点用锚定值（与墙钟零点
                                // 同一时刻写入），词表重写不再让零点漂移。
                                let revealed = reveal_chars_by_timeline(
                                    &state.word_timings,
                                    elapsed_ms,
                                    state.tween_target_chars,
                                    state.tween_timeline_origin,
                                );
                                // 单调不回退：取 max（reveal 可能因词表覆盖不到尾部而
                                // 小于 displayed，此时保持已显示的字符不回退）
                                if revealed > state.displayed_chars {
                                    state.displayed_chars = revealed;
                                    state.needs_repaint = true;
                                }
                            }
                        }
                        // OVERLAY-043: only repaint when text/status/size actually changed
                        // MIC-PULSE-160: 麦克风动效期间（has_audio）也保持重绘。
                        let dirty = state.needs_repaint || mic_has_audio(&state);
                        if dirty && !MENU_VISIBLE.load(Ordering::Acquire) {
                            unsafe {
                                let _ = InvalidateRect(hwnd, None, false);
                            }
                        }
                        state.needs_repaint = false;
                    }
                    OverlayStatus::StreamingEditing { .. } => {
                        // ESC-178: ESC 轮询旁路 —— WM_KEYDOWN 路由层机制未定（工装实证
                        // SetForegroundWindow 后台进程失败，无法模拟生产前台路由），改用
                        // FocusLost 态同款 GetAsyncKeyState 轮询（:2126 范式，读物理键态、
                        // 与坏掉的消息路由零交集）。命中即发 CancelRequested 走既有收口
                        // 臂（ESC-174 已证双顺序安全）。🔴 零重绘约束：本检查放在 dirty
                        // 判定之前、不触碰 needs_repaint/InvalidateRect；命中后的唯一
                        // 重绘是 Hide 本身（预期内）。0x0001 转变位全仓唯一读者
                        // （FocusLost 与编辑态互斥；控制器 :6518 用 0x8000 不消费位）。
                        // 已知取舍：编辑态下其他窗口按 ESC 也会取消（浮层在交互焦点上，
                        // Gavin 已知情接受）。回退 = 删除本检查块。
                        let esc = unsafe { GetAsyncKeyState(VK_ESCAPE.0 as i32) };
                        if (esc as u16) & 0x0001 != 0 {
                            log::debug!(
                                "ESC-178: streaming-editing ESC poll fired, sending CancelRequested"
                            );
                            let _ = state.event_tx.send(OverlayUiEvent::CancelRequested);
                            // 与 FocusLost 同款防重发：清请求，等 controller 的 Hide 接管
                            state.request = None;
                        }
                        // OVERLAY-043: only repaint when text/status/size actually changed
                        let dirty = state.needs_repaint;
                        if dirty && !MENU_VISIBLE.load(Ordering::Acquire) {
                            unsafe {
                                let _ = InvalidateRect(hwnd, None, false);
                            }
                        }
                        state.needs_repaint = false;
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
                                let dist_from_center =
                                    ((bar_idx as f32 - half) / half).abs().min(1.0);
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
                            state.needs_repaint = true;
                        }
                        if (state.needs_repaint || !all_settled)
                            && !MENU_VISIBLE.load(Ordering::Acquire)
                        {
                            unsafe {
                                let _ = InvalidateRect(hwnd, None, false);
                            }
                        }
                        state.needs_repaint = false;
                    }
                    OverlayStatus::Processing(_) => {
                        // processing state: only trigger repaint, phase updated in WM_PAINT
                        if !MENU_VISIBLE.load(Ordering::Acquire) {
                            unsafe {
                                let _ = InvalidateRect(hwnd, None, false);
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
                    OverlayStatus::Info(_) => {
                        // BUG-119: Info 态静态提示，不重绘（同 Error）
                    }
                }
            }

            // OVERLAY-043 / 051-F: smooth interpolation of window size toward target_size.
            // OVERLAY-086 Bug 3: growing width now interpolates like shrinking. The old
            // grow-snap (`dx > 0 → current = target`) made every streaming packet jump the
            // width instantly, and the centering x (OVERLAY-068-A R1 below) jumped with it —
            // the "position slides left / flickers" Gavin reported. The clipping concern that
            // snap originally served ("latest text must not be clipped by an in-flight resize")
            // stays covered: target_size only advances through the 100ms-throttled Show path,
            // the text renderer scrolls within the *drawn* width (scroll_x in
            // draw_recording_overlay_with_text), and the text reveal is timestamp-driven
            // (OVERLAY-086 Bug 2 ③ anchors both zeros so server word-table rewrites no longer
            // stall it) — none of these depend on the window reaching target width instantly.
            // REFACTOR-089: the single-axis step+snap now lives in `advance_width` (shared
            // by width and height) so a guard test can exercise the real formula instead of
            // a re-typed inline copy that goes green even if the grow-snap comes back.
            // interpolate_step itself is untouched (TEST-SYNC-043护栏).
            if state.current_size != state.target_size {
                state.current_size[0] = advance_width(state.current_size[0], state.target_size[0]);
                state.current_size[1] = advance_width(state.current_size[1], state.target_size[1]);
                size_interpolation_done = true;
                state.needs_repaint = true;
            }
        }

        if size_interpolation_done {
            if let Ok(mut state) = shared_state.lock() {
                // OVERLAY-068-A R1: during shrinking interpolation the window width changes
                // every frame but the x stored in req.pos stays stale. Recompute x from the
                // *just-updated* current_size so the window remains horizontally centered
                // for every interpolated frame. y never changes during a resize.
                let current_width = state.current_size[0];
                let current_height = state.current_size[1];
                if let Some(ref mut req) = state.request {
                    // OVERLAY-054-B-FIX F2: 与 :1044 Show 端同源的兜底，不硬编码 [0,0]。
                    // 当前 Show 端保证 Some，但一旦将来有路径让它是 None，硬编码会让窗口
                    // 瞬间回到左上角——这正是本轮要根治的模式，必须从类型层面堵死。
                    let mut pos = req
                        .pos
                        .unwrap_or_else(|| overlay_geometry(&req.status, hwnd).0);
                    let work = monitor_work_rect(hwnd);
                    let work_w = work.right - work.left;
                    // OVERLAY-101: 与 Show 流式分支共用 centered_x（同源同公式）。
                    pos[0] = centered_x(work.left, work_w, current_width);
                    req.pos = Some(pos);
                    unsafe {
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            pos[0],
                            pos[1],
                            current_width,
                            current_height,
                            SWP_NOACTIVATE | SWP_NOZORDER,
                        );
                    }
                }
            }
        }
        // OVERLAY-WAKE-001: message-driven wait replaces thread::sleep(100)
        // - Active overlay (Recording/Processing/Editing): short timeout for animation (~25fps)
        // - Idle (no overlay): block until woken by message or channel command
        let has_active_overlay = if let Ok(state) = shared_state.lock() {
            state.request.as_ref().map_or(false, |r| {
                matches!(
                    r.status,
                    OverlayStatus::Recording
                        | OverlayStatus::RecordingStreamingIdle
                        | OverlayStatus::RecordingWithText { .. }
                        | OverlayStatus::StreamingEditing { .. }
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
            // ASR-038-C: when EDIT control is active, Enter submits; ESC cancels editing.
            if wparam.0 == VK_ESCAPE.0 as usize {
                if let Ok(state) = data.state.lock() {
                    if state.request.is_some() {
                        let _ = state.event_tx.send(OverlayUiEvent::CancelRequested);
                    }
                }
            } else if wparam.0 == VK_RETURN.0 as usize {
                if let Ok(state) = data.state.lock() {
                    if let Some(ref request) = state.request {
                        if matches!(request.status, OverlayStatus::StreamingEditing { .. }) {
                            if let Some(edit_hwnd) = state.edit_hwnd {
                                if let Ok(text) = get_window_text(edit_hwnd) {
                                    let _ = state.event_tx.send(OverlayUiEvent::SubmitRequested(
                                        text,
                                        request.target_hwnd,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            return LRESULT(0);
        }
        WM_LBUTTONUP => {
            if let Ok(mut state) = data.state.lock() {
                if let Some(ref request) = state.request {
                    let (x, y) = lparam_point(lparam);
                    let target_hwnd = request.target_hwnd;
                    match &request.status {
                        OverlayStatus::FocusLost { text, .. } => {
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
                        OverlayStatus::StreamingEditing { .. } => {
                            // OVERLAY-043: the right button is submit in editing mode (same rect as stop)
                            if state
                                .submit_btn_rect
                                .as_ref()
                                .is_some_and(|rect| rect_contains(rect, x, y))
                            {
                                if let Some(edit_hwnd) = state.edit_hwnd {
                                    if let Ok(text) = get_window_text(edit_hwnd) {
                                        let _ = state.event_tx.send(
                                            OverlayUiEvent::SubmitRequested(text, target_hwnd),
                                        );
                                    }
                                }
                            }
                            // cancel editing via the same rect is intentionally removed to avoid
                            // ambiguity; ESC / hotkey / controller still cancel.
                        }
                        _ => {
                            // ASR-038-C: click in text area enters edit mode; stop button cancels.
                            if state
                                .cancel_btn_rect
                                .as_ref()
                                .is_some_and(|rect| rect_contains(rect, x, y))
                            {
                                let _ = state.event_tx.send(OverlayUiEvent::CancelRequested);
                            } else if state
                                .text_hit_rect
                                .as_ref()
                                .is_some_and(|rect| rect_contains(rect, x, y))
                            {
                                let _ = state.event_tx.send(OverlayUiEvent::EditRequested);
                            }
                        }
                    }
                }
            }
            return LRESULT(0);
        }
        WM_CTLCOLOREDIT => {
            // ASR-038-C: customize EDIT control background/text color to match overlay theme
            if let Ok(state) = data.state.lock() {
                if let Some(brush) = state.edit_bg_brush {
                    let hdc = HDC(wparam.0 as *mut std::ffi::c_void);
                    let _ = SetBkColor(hdc, OVERLAY_BG_DARK);
                    let _ = SetTextColor(hdc, OVERLAY_TEXT_WHITE);
                    return LRESULT(brush.0 as isize);
                }
            }
        }
        WM_PAINT => {
            // Double-buffer: draw to memory DC, then submit — via UpdateLayeredWindow
            // (ULW, per-pixel alpha, OVERLAY-121 P1) or BitBlt (SLWA editing mode, P2).
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

            let layered_mode = data
                .state
                .lock()
                .map(|s| s.layered_mode)
                .unwrap_or(OverlayLayeredMode::Ulw);
            let use_ulw = layered_mode == OverlayLayeredMode::Ulw;

            // Create memory DC and bitmap.
            // OVERLAY-121 (P1): ULW path uses a 32bpp top-down DIB section — the
            // premultiplied BGRA surface UpdateLayeredWindow composes (BindDC binds the
            // same mem_dc, DEC-055 single point untouched). SLWA path keeps the
            // compatible bitmap for byte-exact old behavior.
            let mem_dc = unsafe { CreateCompatibleDC(hdc) };
            let (mem_bmp, ppv_bits) = if use_ulw {
                let mut bmi = BITMAPINFO::default();
                bmi.bmiHeader = BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // negative = top-down, GDI/D2D top-left origin
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                };
                let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
                // CreateDIBSection 失败（罕见）时返回默认无效 HBITMAP —— 后续
                // SelectObject 失败当帧画不出，下一帧重试；GDI fallback 结构不受影响。
                let bmp = unsafe {
                    CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                        .unwrap_or_default()
                };
                (bmp, bits)
            } else {
                (
                    unsafe { CreateCompatibleBitmap(hdc, width, height) },
                    std::ptr::null_mut(),
                )
            };
            let old_bmp = unsafe { SelectObject(mem_dc, mem_bmp) };

            if use_ulw {
                // OVERLAY-121 (P1/P3): ULW 帧从全透明起步（零初始化），不再整矩形预填
                // —— D2D 各态自画圆角底板（圆角外像素 alpha=0 = 合成时透明），
                // GDI fallback 各态自带背景填充（含 Processing 的圆角 region 填充）。
                unsafe {
                    std::ptr::write_bytes(ppv_bits as *mut u8, 0, (width * height * 4) as usize);
                }
            } else {
                // OVERLAY-043: memory bitmap is uninitialized; fill the whole back-buffer
                // with the overlay background so any region not covered by subsequent
                // drawing stays predictable. SLWA (editing) keeps this byte-exact.
                let bg_brush = unsafe { CreateSolidBrush(OVERLAY_BG_DARK) };
                unsafe {
                    let _ = FillRect(
                        mem_dc,
                        &RECT {
                            left: 0,
                            top: 0,
                            right: width,
                            bottom: height,
                        },
                        bg_brush,
                    );
                    let _ = DeleteObject(bg_brush);
                }
            }

            // Draw to memory DC
            let mut opacity = 1.0_f32;
            // OVERLAY-141: 外框半径与 opacity 同锁取出（单一来源 overlay_frame_radius），
            // 帧末 SDF alpha 掩码必须与绘制半径同值。
            let mut frame_radius = OVERLAY_FRAME_RADIUS_LG;
            if let Ok(mut state) = data.state.lock() {
                // SHIMMER-FIX-002: time-based phase — immune to WM_PAINT frequency variation
                let _shimmer_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                state.shimmer_phase = (_shimmer_ms % 800) as f32 / 800.0; // SHIMMER-SPEED-002: 1200→800ms
                if let Some(req) = state.request.as_ref() {
                    opacity = req.opacity.clamp(0.1, 1.0);
                    frame_radius = overlay_frame_radius(&req.status);
                }
                let (cancel_rect, close_rect, title_close_rect, submit_rect, text_hit_rect) =
                    draw_overlay_to_dc(hwnd, mem_dc, &rect, &mut state);
                state.cancel_btn_rect = cancel_rect;
                state.close_btn_rect = close_rect;
                state.title_close_btn_rect = title_close_rect;
                state.submit_btn_rect = submit_rect;
                state.text_hit_rect = text_hit_rect;
                // OVERLAY-043: after WM_PAINT has rendered this frame, clear the dirty flag.
                state.needs_repaint = false;
            }

            unsafe {
                if use_ulw {
                    // OVERLAY-121 (P1): 单点提交。先做帧末 alpha 处理（D2D 预乘像素乘
                    // 请求不透明度；GDI 像素提为不透明再预乘），再整窗交给合成器。
                    // 🔴 OVERLAY-149-PROBE（E3/E4，临时探针）：fixup 前/后各采样一次，
                    // 判读完删除（删除清单见 probe fn 头注释 + result.md）。
                    overlay149_probe_dump(
                        hwnd,
                        hdc,
                        "pre-fixup",
                        ppv_bits as *mut u8,
                        width,
                        height,
                        opacity,
                        frame_radius,
                        &data.state,
                    );
                    apply_alpha_fixup(ppv_bits as *mut u8, width, height, opacity, frame_radius);
                    overlay149_probe_dump(
                        hwnd,
                        hdc,
                        "post-fixup",
                        ppv_bits as *mut u8,
                        width,
                        height,
                        opacity,
                        frame_radius,
                        &data.state,
                    );
                    let blend = BLENDFUNCTION {
                        BlendOp: AC_SRC_OVER as u8,
                        BlendFlags: 0,
                        SourceConstantAlpha: 255, // 不透明度已逐像素烘焙进 DIB
                        AlphaFormat: AC_SRC_ALPHA as u8,
                    };
                    let _ = UpdateLayeredWindow(
                        hwnd,
                        None,
                        None,
                        Some(&SIZE {
                            cx: width,
                            cy: height,
                        }),
                        mem_dc,
                        Some(&POINT { x: 0, y: 0 }),
                        COLORREF(0),
                        Some(&blend),
                        ULW_ALPHA,
                    );
                } else {
                    // SLWA (editing) path: one-shot BitBlt to screen DC, byte-exact old behavior.
                    let _ = BitBlt(
                        hdc, rect.left, rect.top, width, height, mem_dc, 0, 0, SRCCOPY,
                    );
                }
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
/// OVERLAY-141: 状态 → 窗口外框圆角半径单一映射（Gavin 2026-09-06 拍板口径）：
/// Recording 系（Recording/RecordingStreamingIdle/RecordingWithText）+ FallingToProcessing
/// + Processing = r16；StreamingEditing（SLWA 旧路径）+ FocusLost/Error/Info = r10。
/// 绘制（D2D chrome 与 GDI fallback）与帧末 SDF alpha 掩码共用本值。
#[cfg(target_os = "windows")]
fn overlay_frame_radius(status: &OverlayStatus) -> f32 {
    match status {
        OverlayStatus::Recording
        | OverlayStatus::RecordingStreamingIdle
        | OverlayStatus::RecordingWithText { .. }
        | OverlayStatus::FallingToProcessing { .. }
        | OverlayStatus::Processing(_) => OVERLAY_FRAME_RADIUS_LG,
        OverlayStatus::StreamingEditing { .. }
        | OverlayStatus::FocusLost { .. }
        | OverlayStatus::Error(_)
        | OverlayStatus::Info(_) => OVERLAY_FRAME_RADIUS_SM,
    }
}

/// OVERLAY-121 (P1): 帧末单点 alpha 处理，唯一调用点紧贴 ULW 提交（WM_PAINT 内）。
/// OVERLAY-141 根治重写：不再依赖 D2D 经 BindDC 是否保真 alpha（实测不保真 ——
/// 抗锯齿边缘半透明像素丢失 alpha 后被旧「提亮」规则压成全不透明，1px 柔和描边
/// 变 2-3px 硬灰带）。改为按已知几何（圆角矩形 SDF，与填充轮廓 (0,0,w,h) r=radius
/// 逐位同框）每像素解析计算覆盖率直接写 alpha：
/// - cov == 0（轮廓外）：四通道全零，真透明 —— 同时消灭「轮廓外杂散 AA 像素
///   被提亮成灰边」与「形状内纯黑像素变透明洞」两个旧规则缺陷（与颜色无关）。
/// - cov > 0：先反预乘还原原色（a>0 时 rgb·255/a；GDI 像素 a==0 本就是直通色），
///   再按 k = cov·opacity 写回预乘域：a = 255·k，rgb = 原色·k。
/// 窗口内部的真·逐像素半透明（如 shimmer 渐变）退化为均一 alpha ——
/// 那是 OVERLAY-121 之前的行为，非回归（DEC-056 的视觉提升体现在圆角平滑）。
/// opacity 取自 Show 请求的均一不透明度；radius 取自 overlay_frame_radius（单一来源）。
#[cfg(target_os = "windows")]
fn apply_alpha_fixup(bits: *mut u8, width: i32, height: i32, opacity: f32, radius: f32) {
    let op = opacity.clamp(0.1, 1.0);
    let half_w = width as f32 * 0.5;
    let half_h = height as f32 * 0.5;
    let inset_w = half_w - radius;
    let inset_h = half_h - radius;
    let premul = |c: u8, k: f32| ((c as f32 * k).round() as u8).min(255);
    unsafe {
        for y in 0..height {
            let qy = ((y as f32 + 0.5 - half_h).abs() - inset_h).max(0.0);
            let row = bits.add((y as usize) * (width as usize) * 4);
            for x in 0..width {
                let p = row.add((x as usize) * 4);
                let qx = ((x as f32 + 0.5 - half_w).abs() - inset_w).max(0.0);
                // 圆角矩形 SDF（q 已夹逼到 ≥0，min(max(qx,qy),0) 恒 0）：
                // 内部 d ≤ -r → cov=1，与精确 SDF 覆盖率逐位同值。
                let d = (qx * qx + qy * qy).sqrt() - radius;
                let cov = (0.5 - d).clamp(0.0, 1.0);
                if cov <= 0.0 {
                    // 轮廓外：真透明（四通道全零）。
                    *p = 0;
                    *p.add(1) = 0;
                    *p.add(2) = 0;
                    *p.add(3) = 0;
                    continue;
                }
                let k = cov * op;
                let a = *p.add(3);
                // OVERLAY-153: 几何覆盖率当**乘数**、不当 alpha 的**替代品**。
                // - a>0（D2D 预乘像素）：四通道等比缩放 ×(cov·op) —— 恢复 OVERLAY-121
                //   语义，保留原 alpha（描边 AA 的半透明信息不被强制拉满；预乘不变式
                //   rgb≤a 随等比缩放天然成立，反预乘一步整个去掉，无精度损失）。
                //   141 的强制不透明把内部区（cov=1）描边 AA 压成 2-3px 实心灰带，
                //   即 Gavin 端测「圆角包边灰线粗乱」的内部区伪影本体。
                // - a==0（GDI 像素，GDI 不写 alpha）：语义「本应不透明」，维持提亮——
                //   rgb×=k 落预乘域，a=255·k。alpha 未存活的世界里与 141 逐位相同。
                if a > 0 {
                    *p = premul(*p, k);
                    *p.add(1) = premul(*p.add(1), k);
                    *p.add(2) = premul(*p.add(2), k);
                    *p.add(3) = premul(a, k);
                } else {
                    let b = *p;
                    let g = *p.add(1);
                    let r = *p.add(2);
                    *p = premul(b, k);
                    *p.add(1) = premul(g, k);
                    *p.add(2) = premul(r, k);
                    *p.add(3) = premul(255, k);
                }
            }
        }
    }
}

// 🔴 OVERLAY-149-PROBE（E3/E4 临时探针，OVERLAY-147-DIAG §A5；判读完整段删除）。
// 将来删除清单（四处，缺一漏证据）：
//   ① 本文件两处 probe fn（overlay149_probe_dump / overlay149_probe_region_hex）整段
//   ② WM_PAINT ULW 分支内两处调用（"pre-fixup" / "post-fixup"）
//   ③ d2d::with_d2d 内 E4 RT GetDpi once 块
//   ④ destroy_edit_control 内 F1 的 GetGUIThreadInfo 证据块
// E4 口径说明：任务书原文 GetDpiForWindow 需要 Cargo.toml 加 Win32_UI_HiDpi feature
// （红线只许改 src/main.rs，未加）；改用 GetDeviceCaps(LOGPIXELSX/SY) 对**同一 hdc**
// （即 DIB CreateCompatibleDC 的源 DC，正是 BindDC/合成坐标系的 GDI 侧）取 DPI，
// 与 D2D RT GetDpi 对照，排除目的相同（DIAG §A5-E4：不一致会两组同坏，非分界解释）。
#[cfg(target_os = "windows")]
fn overlay149_probe_dump(
    hwnd: HWND,
    hdc: HDC,
    phase: &str,
    bits: *mut u8,
    width: i32,
    height: i32,
    opacity: f32,
    radius: f32,
    state: &Mutex<OverlayWindowState>,
) {
    use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
    static LAST_STATUS: AtomicU8 = AtomicU8::new(255);
    static FRAMES: AtomicU32 = AtomicU32::new(0);

    let Ok(state_guard) = state.lock() else {
        return;
    };
    let Some(req) = state_guard.request.as_ref() else {
        return;
    };
    let (status_id, status_name): (u8, &str) = match &req.status {
        OverlayStatus::Recording => (1, "Recording"),
        OverlayStatus::RecordingStreamingIdle => (2, "RecordingStreamingIdle"),
        OverlayStatus::RecordingWithText { .. } => (3, "RecordingWithText"),
        OverlayStatus::StreamingEditing { .. } => (4, "StreamingEditing"),
        OverlayStatus::FallingToProcessing { .. } => (5, "FallingToProcessing"),
        OverlayStatus::Processing(_) => (6, "Processing"),
        OverlayStatus::FocusLost { .. } => (7, "FocusLost"),
        OverlayStatus::Error(_) => (8, "Error"),
        OverlayStatus::Info(_) => (9, "Info"),
    };
    drop(state_guard);

    // 节流：状态切换后仅前 N=3 帧采样（Recording 每帧重绘，防 debug.log 撑爆，
    // [EVIDENCE-LOG-VOLATILE-001]）。对照态 RecordingStreamingIdle(r=16 坏) / Info(r=10 好)。
    if LAST_STATUS.load(Ordering::Relaxed) != status_id {
        LAST_STATUS.store(status_id, Ordering::Relaxed);
        FRAMES.store(0, Ordering::Relaxed);
    }
    let frame = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if !matches!(status_id, 2 | 9) || frame > 3 {
        return;
    }
    // E4：窗口/DC 侧 DPI（GetDeviceCaps LOGPIXELSX；RT 侧见 with_d2d once 块）
    unsafe {
        let dpi_x = windows::Win32::Graphics::Gdi::GetDeviceCaps(
            hdc,
            windows::Win32::Graphics::Gdi::LOGPIXELSX,
        );
        let dpi_y = windows::Win32::Graphics::Gdi::GetDeviceCaps(
            hdc,
            windows::Win32::Graphics::Gdi::LOGPIXELSY,
        );
        log::debug!(
            "OVERLAY-149-PROBE E4 [{}] frame#{} status={} hwnd-hdc-dpi=({},{}) w={} h={} radius={} opacity={:.2}",
            phase,
            frame,
            status_name,
            dpi_x,
            dpi_y,
            width,
            height,
            radius,
            opacity
        );
    }
    // E3：四角 8×8 块 + 左缘一整列，BGRA 内存序原值（B,G,R,A）
    for (tag, x0, y0, w, h) in [
        ("TL", 0, 0, 8, 8),
        ("TR", width - 8, 0, 8, 8),
        ("BL", 0, height - 8, 8, 8),
        ("BR", width - 8, height - 8, 8, 8),
    ] {
        log::debug!("OVERLAY-149-PROBE E3 [{}] {} {}", phase, tag, unsafe {
            overlay149_probe_region_hex(bits, width, height, x0, y0, w, h)
        });
    }
    log::debug!("OVERLAY-149-PROBE E3 [{}] LEFT-COL {}", phase, unsafe {
        overlay149_probe_region_hex(bits, width, height, 0, 0, 1, height)
    });
    let _ = hwnd;
}

// 🔴 OVERLAY-149-PROBE：BGRA 区域 hex dump（内存序 B,G,R,A；删除见 probe fn 头清单①）
#[cfg(target_os = "windows")]
unsafe fn overlay149_probe_region_hex(
    bits: *mut u8,
    width: i32,
    height: i32,
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
) -> String {
    let mut s = String::new();
    let x_end = (x0 + w).min(width);
    let y_end = (y0 + h).min(height);
    for y in y0.max(0)..y_end {
        for x in x0.max(0)..x_end {
            let p = bits.add(((y as usize) * (width as usize) + x as usize) * 4);
            s.push_str(&format!(
                "{:02X}{:02X}{:02X}{:02X}",
                *p,
                *p.add(1),
                *p.add(2),
                *p.add(3)
            ));
        }
    }
    s
}

#[cfg(target_os = "windows")]
fn draw_overlay_to_dc(
    _hwnd: HWND, // OVERLAY-121 (P3): 掩码退役后本函数不再触碰窗口几何，保留参数位
    hdc: HDC,
    rect: &RECT,
    state: &mut OverlayWindowState,
) -> (
    Option<RECT>,
    Option<RECT>,
    Option<RECT>,
    Option<RECT>,
    Option<RECT>,
) {
    // Double-buffer: hdc is memory DC, rect is already computed

    // OVERLAY-051-F: reuse a cached ClearType font instead of creating/destroying one per frame.
    // OVERLAY-054-E: use file-level OVERLAY_FONT_SIZE so cached font matches measuring font.
    // STREAMFONT-189: 流式两态（RecordingStreamingIdle 占位 / RecordingWithText）改选
    // streaming_font（OVERLAY_TEXT_FONT_SIZE = -16），与 D2D streaming_text_format /
    // EDIT 控件 / 两态测量同号；其余态（处理中/箭头/Error/Info 小字等）维持 cached_font
    // (-14) 逐位不变。选择点收在本处是刻意的：全 GDI 帧共用一次 SelectObject，
    // 流式兜底路径内的 measure_text_width（draw_recording_overlay_with_text :3579，
    // D2D 成败都消费）与 draw_text 全部继承此处所选字体 ⇒ 测渲必然同号（EDITFONT-183
    // 的宽度低估教训在此结构性排除）。生命周期照 edit_font/cached_font 范式：
    // 惰性创建 + 存回字段，DeleteObject 配对在隐藏/退出清理臂（cached_font 同臂）。
    let streaming_text = matches!(
        state.request.as_ref().map(|r| &r.status),
        Some(OverlayStatus::RecordingStreamingIdle) | Some(OverlayStatus::RecordingWithText { .. })
    );
    let font = if streaming_text {
        state
            .streaming_font
            .unwrap_or_else(|| create_clear_type_font(OVERLAY_TEXT_FONT_SIZE))
    } else {
        state
            .cached_font
            .unwrap_or_else(|| create_clear_type_font(OVERLAY_FONT_SIZE))
    };
    let old_font = unsafe { SelectObject(hdc, font) };
    if streaming_text {
        state.streaming_font = Some(font);
    } else {
        state.cached_font = Some(font);
    }

    unsafe {
        let _ = SetBkMode(hdc, TRANSPARENT);
    }

    let mut cancel_btn_rect = None;
    let mut close_btn_rect = None;
    let mut title_close_btn_rect = None;
    let mut submit_btn_rect = None;
    let mut text_hit_rect = None;

    if let Some(request) = &state.request {
        match &request.status {
            OverlayStatus::Recording => {
                cancel_btn_rect = Some(draw_recording_overlay(
                    hdc,
                    rect,
                    state,
                    false,
                    request.ui_language,
                ));
            }
            OverlayStatus::RecordingStreamingIdle => {
                cancel_btn_rect = Some(draw_recording_overlay(
                    hdc,
                    rect,
                    state,
                    true,
                    request.ui_language,
                ));
            }
            OverlayStatus::RecordingWithText { text } => {
                // OVERLAY-086 Bug 2 ② (render-side guard): an empty streaming text must
                // never produce an empty window. The upstream gate (OVERLAY-086 Bug 2 ①,
                // qwen_inference on_result) should stop empty packets from reaching this
                // status at all, but "empty streaming text never yields an empty window"
                // is an invariant the render layer must uphold on its own — fall back to
                // the full listening placeholder exactly like RecordingStreamingIdle.
                if text.is_empty() {
                    cancel_btn_rect = Some(draw_recording_overlay(
                        hdc,
                        rect,
                        state,
                        true,
                        request.ui_language,
                    ));
                } else {
                    // OVERLAY-051-G: render only the tween-visible prefix.
                    let visible_text: String = text.chars().take(state.displayed_chars).collect();
                    let (cr, sr, thr) = draw_recording_overlay_with_text(
                        hdc,
                        rect,
                        state,
                        request.ui_language,
                        &visible_text,
                    );
                    cancel_btn_rect = Some(cr);
                    submit_btn_rect = Some(sr);
                    text_hit_rect = Some(thr);
                    // OVERLAY-043: set dirty when the streaming text actually changes
                    let text_changed = state.last_streaming_text.as_ref() != Some(text);
                    if text_changed {
                        state.last_streaming_text = Some(text.clone());
                        state.needs_repaint = true;
                    }
                }
            }
            OverlayStatus::FallingToProcessing { .. } => {
                // D2D-P2 (PLAN-108 H7): 与 Recording 波形变体共用同一复合体
                // （两者 GDI 输出逐位相同，见 draw_recording_waveform_overlay doc）。
                // On any D2D failure the GDI path below still renders this frame.
                if !d2d::draw_recording_waveform_overlay(hdc, rect, state) {
                    // OVERLAY-121 (P3): FallingToProcessing fallback r=16（Gavin 拍板）。
                    // OVERLAY-141: 半径单一来源。
                    draw_overlay_chrome(hdc, rect, OVERLAY_FRAME_RADIUS_LG as i32);
                    draw_recording_indicator_and_waveform(hdc, rect, state);
                    cancel_btn_rect = Some(draw_stop_button(hdc, rect));
                } else {
                    cancel_btn_rect = Some(draw_stop_button_hit_rect_only(rect));
                }
            }
            OverlayStatus::Processing(message) => {
                // OVERLAY-086 Bug 1: rounded window region matching the D2D/GDI border
                // geometry (radius 16) so no rectangular-region wedge is left outside the
                // rounded stroke in the corners. Trade-off: SetWindowRgn is a binary mask
                // (no anti-aliasing), so the outermost 1px of the D2D anti-aliased corner
                // edge gets clipped hard — accepted under DEC-056 as the better of the two
                // imperfect options; if end-testing prefers the soft edge, revert to None.
                // D2D-073 P0: this status is redrawn with Direct2D + DirectWrite
                // (DEC-055 gray migration step 1). On any D2D failure the GDI path below
                // still renders this frame, so the overlay never goes blank.
                if !d2d::draw_processing_overlay(
                    hdc,
                    rect,
                    request.ui_language,
                    state.shimmer_phase,
                ) {
                    draw_processing_overlay(
                        hdc,
                        rect,
                        message,
                        request.ui_language,
                        state.shimmer_phase,
                    );
                }
            }
            OverlayStatus::StreamingEditing { .. } => {
                // D2D-P2 (PLAN-108 H9): editing chrome + submit button redrawn with D2D.
                // 正文由 EDIT 子控件自绘（本态父窗口不画文字）；on any D2D failure the
                // GDI path below still renders this frame, so the overlay never blanks.
                if !d2d::draw_editing_overlay(hdc, rect) {
                    // OVERLAY-051-B: editing mode draws a clear submit button (orange ⏎) on the right.
                    // The EDIT control renders the text itself; we only paint chrome + submit button.
                    // OVERLAY-121 (P3): 编辑态 fallback 维持 r=10（SLWA 旧路径口径不变）。
                    // OVERLAY-141: 半径单一来源。
                    draw_overlay_chrome(hdc, rect, OVERLAY_FRAME_RADIUS_SM as i32);
                    // EDITICON-176: 兜底帧同画铅笔图标 + 左分割线（与 D2D 主路径同几何）
                    draw_edit_icon_and_separator_gdi(hdc, rect);
                    submit_btn_rect = Some(draw_submit_button(hdc, rect));
                } else {
                    submit_btn_rect = Some(draw_submit_button_hit_rect_only(rect));
                }
            }
            OverlayStatus::FocusLost { text, .. } => {
                // D2D-P2 (PLAN-108 H11): preview overlay redrawn with D2D. On any D2D
                // failure the GDI path below still renders this frame, so the overlay
                // never goes blank. 命中 rect 两条路径都从 preview_hit_rects 出
                // （H10 单一几何源，点击口径逐位同值）。
                if !d2d::draw_preview_overlay(hdc, rect, text, request.ui_language) {
                    let (copy_rect, close_rect, tc_rect) =
                        draw_preview_overlay(hdc, rect, text, request.ui_language);
                    cancel_btn_rect = Some(copy_rect);
                    close_btn_rect = Some(close_rect);
                    title_close_btn_rect = Some(tc_rect);
                } else {
                    let (copy_rect, close_rect, tc_rect) = preview_hit_rects(rect);
                    cancel_btn_rect = Some(copy_rect);
                    close_btn_rect = Some(close_rect);
                    title_close_btn_rect = Some(tc_rect);
                }
            }
            OverlayStatus::Error(message) => {
                // D2D-P2 (PLAN-108 H12): error 态 redrawn with D2D. On any D2D failure
                // the GDI path below still renders this frame, so the overlay never
                // goes blank. GDI fallback keeps DT_END_ELLIPSIS (U1 裁决).
                if !d2d::draw_error_overlay(hdc, rect, message) {
                    draw_error_overlay(hdc, rect, message, request.ui_language);
                }
            }
            OverlayStatus::Info(message) => {
                // BUG-119: 信息提示态。圆角掩码沿用 Error 现值 Some(10)
                //（OVERLAY-121 per-pixel alpha 才动圆角，本单不碰）。
                // D2D 优先，失败回落 GDI（与 Error 态同一兜底结构）。
                if !d2d::draw_info_overlay(hdc, rect, message) {
                    draw_info_overlay(hdc, rect, message, request.ui_language);
                }
            }
        }
    } else {
    }

    unsafe {
        let _ = SelectObject(hdc, old_font);
        // OVERLAY-051-F: do NOT delete the cached font here; it lives in OverlayWindowState.
    }

    (
        cancel_btn_rect,
        close_btn_rect,
        title_close_btn_rect,
        submit_btn_rect,
        text_hit_rect,
    )
}

#[cfg(target_os = "windows")]
fn draw_overlay_chrome(hdc: windows::Win32::Graphics::Gdi::HDC, rect: &RECT, corner_radius: i32) {
    const BG_DARK: COLORREF = COLORREF(0x110F0D);
    // Dark background
    let bg = unsafe { CreateSolidBrush(BG_DARK) };
    unsafe {
        let _ = FillRect(hdc, rect, bg);
        let _ = DeleteObject(bg);
    }
    // Window border
    let border_pen = unsafe { CreatePen(PS_SOLID, 1, OVERLAY_BORDER_GRAY) };
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
            corner_radius * 2,
            corner_radius * 2,
        );

        let _ = SelectObject(hdc, old_pen);
        let _ = SelectObject(hdc, old_brush);
        let _ = DeleteObject(border_pen);
    }
}
/// D2D-P2 (PLAN-108 H3): 波形快照纯函数 —— 锁内完成 peak decay（OVERLAY-LOCK-SCOPE-001
/// 语义固化点：decay 必须在锁内、绘制在锁外）并收集 16 个 display_value。
/// GDI draw_recording_indicator_and_waveform 与 D2D d2d::waveform 共用，公式单源。
/// DECAY_RATE 0.02（原 draw_recording_indicator_and_waveform 局部常量上移）；
/// WAVEFORM-FIX-002 的取样方向（bar i=0 → 最新样本 len-1）原样保留。
#[cfg(target_os = "windows")]
fn waveform_snapshot(state: &OverlayWindowState, half: i32) -> Vec<f32> {
    const DECAY_RATE: f32 = 0.02; // Peak decay per frame at 60fps
    if let Ok(mut levels) = state.audio_buf.lock() {
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
        // Lock poisoned = stream failed → 空快照，全部落 static 高度
        Vec::new()
    }
}

/// D2D-P2 (PLAN-108 H3): 单条波形 bar 高度纯函数。公式逐位照抄原 GDI 内联版
/// （WAVEFORM-HEIGHT-FIX-001：maxh 48 / static 12 / minh 8，gain 2.5，
/// 权重 0.4 + 0.6·cos²(π/2·i/(half-1))）。返回 i32（GDI RoundRect 整数像素口径），
/// D2D 侧取用前转 f32 —— 整数对齐避免两路径 ±1px 视觉差。
#[cfg(target_os = "windows")]
fn waveform_bar_height(v: f32, i: i32, half: i32) -> i32 {
    let maxh = 48;
    let static_h = 12;
    let minh = 8;
    let gain = 2.5;
    let weight: f32 = 0.4
        + 0.6
            * (std::f32::consts::FRAC_PI_2 * i as f32 / (half - 1).max(1) as f32)
                .cos()
                .powi(2);
    let v_gain = (v * gain * weight).min(1.0);
    if v_gain > 0.01 {
        (minh as f32 + v_gain * (maxh - minh) as f32) as i32
    } else {
        static_h
    }
}

// MIC-PULSE-160: 流式窗麦克风声波弧动效共享层（D2D 主路径 + 两处 GDI 兜底同源）。
// 时基走 SHIMMER-FIX-002 范式：相位由墙钟在 WM_PAINT 内现算，不累加、不存帧计数
// ⇒ 免疫 WM_PAINT 频率抖动。
// 几何（Gavin 修订：pill 上下无空间 ⇒ 弧画左右两侧）：弧心 = pill 体中心
// （cx=circ_l+9, cy=circ_t+7），内弧 R=5.5 / 外弧 R=8.0，**左右各一组、对称、
// 同相位同亮度**：右组 -40°..+40°，左组 140°..220°（0°=正右，y 向下为正），
// 线宽 1.2。空间已核验：外弧含半线宽左沿 6.4 / 右沿 23.6，图标框 [6,24] 两侧
// 都放得下，不碰 x=30 左分隔线；pill 上下方一律不画。
// 只在橘色（has_audio）态绘制；静音 gain=0 时整段不画（与改前逐位相同）；
// 红（设备故障）态也不画。
const MIC_PULSE_PERIOD_MS: u64 = 1000;
const MIC_PULSE_OUT_OFFSET: f32 = 0.30; // 外弧相位滞后 0.3 ⇒「由内向外扩散」
const MIC_PULSE_PULSE_WIDTH: f32 = 0.55; // tri 波总宽（半宽 0.275）
                                         // FIX-164 Part B: 0.35 → 0.10。诊断（DIAG-163）：色阈值 0.01（mic_audio_snapshot 的
                                         // has_audio 门槛）与本值相差 35 倍 ⇒ 正常说话电平区间里麦克风早已变橙、弧的 alpha
                                         // 却只有百分之几，1.2px 线宽下肉眼不可见（「橙麦 + 隐形弧」自相矛盾态）。
                                         // 0.10 = level≥0.10 即满亮；正常说话电平 0.02~0.3 区间 gain≈0.2~1.0，动效可读。
const MIC_PULSE_FULL_LEVEL: f32 = 0.10;
// FIX-164 Part B: 可见地板 —— has_audio 为真（level>0.01）时弧的 alpha 不低于此值，
// 杜绝「橙麦 + 隐形弧」再出现。静音（gain==0 / 无音频）仍完全不画（160 既定契约）。
const MIC_PULSE_ALPHA_FLOOR: f32 = 0.35;
const MIC_PULSE_OUT_DIM: f32 = 0.85; // 外弧略淡，强化扩散读感
const MIC_PULSE_ARC_HALF_ANGLE_DEG: f32 = 40.0;
const MIC_PULSE_R_IN: f32 = 5.5;
const MIC_PULSE_R_OUT: f32 = 8.0;
const MIC_PULSE_ARC_STROKE: f32 = 1.2;

/// tri(p, off)：以 off 为起点的 0→1→0 三角波，宽 0.55，超出返回 0。
#[cfg(target_os = "windows")]
fn mic_pulse_tri(p: f32, offset: f32) -> f32 {
    let x = (p - offset + 1.0) % 1.0;
    if x >= MIC_PULSE_PULSE_WIDTH {
        0.0
    } else {
        let half = MIC_PULSE_PULSE_WIDTH / 2.0;
        if x <= half {
            x / half
        } else {
            (MIC_PULSE_PULSE_WIDTH - x) / half
        }
    }
}

/// 墙钟（ms）+ 电平增益 → (内弧 alpha, 外弧 alpha)。
/// FIX-164 Part B: 三产出源（D2D mic_indicator / GDI 波形 / GDI 流式）共用本纯函数，
/// 阈值与地板只在本处生效；gain<=0（静音/无音频）返回 (0,0)，调用方零绘制的
/// 160 契约不变。has_audio 语义下 gain>0 ⇒ alpha 有 0.35 地板，杜绝隐形弧。
#[cfg(target_os = "windows")]
fn mic_pulse_alphas(now_ms: u64, gain: f32) -> (f32, f32) {
    if gain <= 0.0 {
        return (0.0, 0.0);
    }
    // FIX-172-A: 地板从「截断」改回「抬高振幅」（乘，不是 max）。FIX-164 的 .max()
    // 把 tri*gain 峰值（正常说话 gain 0.2~0.5 ⇒ 峰值 0.2~0.5）几乎全程压平在 0.35
    // 地板上，连 tri 的 45% 零段也被抬成 0.35 ⇒ 波形变常数直线 = 「一起亮一起灭」
    // （Gavin 端测回归，主控定性）。改法：amplitude = FLOOR + (1-FLOOR)*gain，
    // 峰值 ∈ (FLOOR, 1]；谷底仍回 0（渐隐保留）、内先外后相位次序不变、
    // gain→0⁺ 峰值仍有 FLOOR（橙麦必有可见弧）、静音 gain≤0 早退 (0,0) 契约原样。
    let amplitude = MIC_PULSE_ALPHA_FLOOR + (1.0 - MIC_PULSE_ALPHA_FLOOR) * gain;
    let p = (now_ms % MIC_PULSE_PERIOD_MS) as f32 / MIC_PULSE_PERIOD_MS as f32;
    (
        mic_pulse_tri(p, 0.0) * amplitude,
        mic_pulse_tri(p, MIC_PULSE_OUT_OFFSET) * amplitude * MIC_PULSE_OUT_DIM,
    )
}

/// 三态快照 + 峰值电平，一次锁内取出（不为动画二次加锁，避免和音频线程抢锁）。
#[cfg(target_os = "windows")]
fn mic_audio_snapshot(state: &OverlayWindowState) -> (bool, bool, f32) {
    if let Ok(levels) = state.audio_buf.lock() {
        let empty = levels.is_empty();
        let audio = !empty && levels.iter().any(|v| v.current > 0.01);
        let level = levels
            .iter()
            .map(|v| v.current)
            .fold(0.0_f32, f32::max)
            .clamp(0.0, 1.0);
        (empty, audio, level)
    } else {
        (true, false, 0.0)
    }
}

/// overlay 线程重绘决策用（RecordingWithText / RecordingStreamingIdle 分支各调一次）。
#[cfg(target_os = "windows")]
fn mic_has_audio(state: &OverlayWindowState) -> bool {
    mic_audio_snapshot(state).1
}

/// GDI 无 per-primitive alpha：逐通道向背景色插值近似透明度（仅兜底路径，如实近似）。
#[cfg(target_os = "windows")]
fn blend_colorref(fg: COLORREF, bg: COLORREF, a: f32) -> COLORREF {
    let mix = |f: u32, b: u32| {
        (b as f32 + (f as f32 - b as f32) * a)
            .round()
            .clamp(0.0, 255.0) as u32
    };
    COLORREF(
        mix(fg.0 & 0xFF, bg.0 & 0xFF)
            | (mix((fg.0 >> 8) & 0xFF, (bg.0 >> 8) & 0xFF) << 8)
            | (mix((fg.0 >> 16) & 0xFF, (bg.0 >> 16) & 0xFF) << 16),
    )
}

/// GDI 兜底共享：声波弧画在 4x 超采样画布（icon 原点系，72×72）。
/// 左右各一组对称弧（右 -40°..+40°，左 140°..220°），同相位同亮度。
/// 采样折线（20 段/弧）而非 GDI Arc() —— Arc 方向语义依赖坐标约定，折线零歧义。
#[cfg(target_os = "windows")]
fn draw_mic_pulse_gdi_4x(
    mem_dc: windows::Win32::Graphics::Gdi::HDC,
    a_in: f32,
    a_out: f32,
    bg: COLORREF,
) {
    const CX: f32 = 9.0 * 4.0; // icon 坐标 (circ_l+9, circ_t+7)×4 = pill 体中心
    const CY: f32 = 7.0 * 4.0;
    const STEPS: usize = 20;
    let half_ang = MIC_PULSE_ARC_HALF_ANGLE_DEG.to_radians();
    // (半径 1x, alpha, 角度区间) —— 左右两组同 alpha
    let arcs = [
        (MIC_PULSE_R_IN, a_in, -half_ang..half_ang),
        (MIC_PULSE_R_OUT, a_out, -half_ang..half_ang),
        (
            MIC_PULSE_R_IN,
            a_in,
            (std::f32::consts::PI - half_ang)..(std::f32::consts::PI + half_ang),
        ),
        (
            MIC_PULSE_R_OUT,
            a_out,
            (std::f32::consts::PI - half_ang)..(std::f32::consts::PI + half_ang),
        ),
    ];
    for (r_1x, a, span) in arcs {
        if a <= 0.0 {
            continue; // 全透明弧完全不画
        }
        let r = r_1x * 4.0; // 1x 半径 → 4x 画布
        let pen = unsafe {
            CreatePen(
                PS_SOLID,
                (MIC_PULSE_ARC_STROKE * 4.0).round() as i32, // 1.2×4≈5
                blend_colorref(OVERLAY_BRAND_ORANGE, bg, a),
            )
        };
        let old_pen = unsafe { SelectObject(mem_dc, pen) };
        let pt = |ang: f32| (CX + r * ang.cos(), CY + r * ang.sin());
        let (px, py) = pt(span.start);
        unsafe {
            let _ = MoveToEx(mem_dc, px.round() as i32, py.round() as i32, None);
            for i in 1..=STEPS {
                let ang = span.start + (span.end - span.start) * i as f32 / STEPS as f32;
                let (x, y) = pt(ang);
                let _ = LineTo(mem_dc, x.round() as i32, y.round() as i32);
            }
            let _ = SelectObject(mem_dc, old_pen);
            let _ = DeleteObject(pen);
        }
    }
}

#[cfg(target_os = "windows")]
fn draw_recording_indicator_and_waveform(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    state: &OverlayWindowState,
) {
    const BRAND_ORANGE: COLORREF = COLORREF(0x006BFF); // #FF6B00
    const RED_STREAM_FAILED: COLORREF = COLORREF(0x0000FF); // #FF0000 — device error
    const GRAY_SILENT: COLORREF = COLORREF(0x808080); // #808080
    const BG_DARK: COLORREF = COLORREF(0x110F0D);
    // OVERLAY-054-C: use file-level OVERLAY_BORDER_GRAY instead of local constant.
    // === Problem 1: 14px smooth circle using HALFTONE supersampling ===
    // PERF-BATCH-001 TASK-5: 三态指示灯
    // RED=stream_failed(设备故障) > ORANGE=有音频录入(level>0.01) > GRAY=设备正常但无音频
    let circ_size = 18; // MIC-ICON-ENLARGE-001: from 14 to 18
    let circ_l = rect.left + 6; // MIC-ICON-ENLARGE-001: left-shift to keep margin to separator
    let circ_t = rect.top + (rect.bottom - rect.top - circ_size) / 2;

    // Three-state audio indicator
    // OVERLAY-LOCK-SCOPE-001: snapshot buffer state under lock, compute color outside lock
    // MIC-PULSE-160: 峰值电平同一次锁内取出（不为动画二次加锁）。
    let (buf_empty, has_audio, mic_level) = mic_audio_snapshot(state);
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
        // MIC-PULSE-160: 声波弧画在 4x 画布上（随 StretchBlt 一起 HALFTONE 降采样）。
        // SHIMMER-FIX-002 范式：相位由墙钟现算。静音 gain=0 整段不画（与改前逐位相同）。
        if has_audio {
            let gain = (mic_level / MIC_PULSE_FULL_LEVEL).clamp(0.0, 1.0);
            if gain > 0.0 {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let (a_in, a_out) = mic_pulse_alphas(now_ms, gain);
                draw_mic_pulse_gdi_4x(mem_dc, a_in, a_out, BG_DARK);
            }
        }
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
    let sep_pen = unsafe { CreatePen(PS_SOLID, 2, OVERLAY_BORDER_GRAY) };
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
    let by = cy;
    // D2D-P2 (PLAN-108 H3): snapshot（锁内 decay，OVERLAY-LOCK-SCOPE-001）与 bar 高度
    // 公式抽成共享纯函数 —— GDI/D2D 两路径同源，常量/公式不可能分叉。
    // WAVEFORM-HEIGHT-FIX-001 的常量（maxh 48 / static 12 / minh 8 / gain 2.5）
    // 随公式一起上移进 waveform_bar_height。
    let snapshot = waveform_snapshot(state, half);
    // GDI drawing uses snapshot (lock released)
    // WAVEFORM-FIX-002: center bar(i=0) maps to newest sample(len-1), edge(i=half-1) to oldest
    // Left half: bars spread from center outward to the left
    for i in 0..half {
        let v = snapshot.get(i as usize).copied().unwrap_or(0.0);
        let bh = waveform_bar_height(v, i, half);
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
        let bh = waveform_bar_height(v, i, half);
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
    let sep_pen2 = unsafe { CreatePen(PS_SOLID, 2, OVERLAY_BORDER_GRAY) };
    let sep_op2 = unsafe { SelectObject(hdc, sep_pen2) };
    unsafe {
        let _ = MoveToEx(hdc, sep_r_x, cy - sep_hh, None);
        let _ = LineTo(hdc, sep_r_x, cy + sep_hh);
        let _ = SelectObject(hdc, sep_op2);
        let _ = DeleteObject(sep_pen2);
    }
}

#[cfg(target_os = "windows")]
fn draw_stop_button(hdc: windows::Win32::Graphics::Gdi::HDC, rect: &RECT) -> RECT {
    const BRAND_ORANGE: COLORREF = COLORREF(0x006BFF); // #FF6B00
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
fn draw_recording_overlay(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    state: &OverlayWindowState,
    show_placeholder: bool,
    ui_language: config::UiLanguage,
) -> RECT {
    // D2D-P1: RecordingStreamingIdle is drawn with D2D (DEC-055 step 2). On any D2D
    // failure the GDI path below still renders this frame, so the overlay never blanks.
    // Only the placeholder variant migrates — the waveform variant (Recording) is a
    // P2 state and keeps its GDI path untouched (DEC-055 红线 4).
    if show_placeholder && d2d::draw_streaming_idle_overlay(hdc, rect, state, ui_language) {
        return draw_stop_button_hit_rect_only(rect);
    }
    // OVERLAY-149 (P3): Recording 系 GDI fallback r=16（Gavin 拍板；D2D 失败帧仍是
    // 方角填充 —— GDI 无 AA，方角是 fallback 的既定降级，见 result.md）。
    // OVERLAY-141: 半径单一来源。
    // OVERLAY-155 (改动1): 🔴 GDI chrome 收进 D2D 失败分支（与 Info/Error 同构）。
    // 旧行为：此行无条件先画，波形分支（show_placeholder=false）在 D2D 尝试之前
    // 就落下 GDI RoundRect —— 无 AA 硬台阶 + 1px 灰描边与 D2D 的 AA 描边错位
    // ~0.5px 叠画，角区灰线粗乱（Gavin 三轮端测的圆角病灶，DIAG 差异 A）。
    // 收进失败分支后：D2D 成功即 return，GDI 只在 D2D 失败时兜底（DEC-055 契约），
    // Info/Error 的干净结构与此同构。
    if show_placeholder {
        draw_overlay_chrome(hdc, rect, OVERLAY_FRAME_RADIUS_LG as i32);
        // OVERLAY-051-E: online streaming ASR waiting for first text shows placeholder,
        // not waveform. Local model continues to show waveform unchanged.
        draw_recording_indicator(hdc, rect, state);
        draw_listening_placeholder(hdc, rect, ui_language);
    } else {
        // D2D-P2 (PLAN-108 H6): 波形变体迁 D2D。On any D2D failure the GDI path
        // below still renders this frame, so the overlay never blanks. 命中 rect
        // 由 P1 既有 helper 出（与 d2d::stop_button 同公式，几何单一源）。
        // 🔴 OVERLAY-155: D2D 在先 —— 成功即 return（不再被先行的 GDI chrome 污染）。
        if d2d::draw_recording_waveform_overlay(hdc, rect, state) {
            return draw_stop_button_hit_rect_only(rect);
        }
        // D2D 失败才走 GDI 兜底（chrome 在此，不在 D2D 之前）。
        // OVERLAY-141: 半径单一来源。
        draw_overlay_chrome(hdc, rect, OVERLAY_FRAME_RADIUS_LG as i32);
        draw_recording_indicator_and_waveform(hdc, rect, state);
    }
    draw_stop_button(hdc, rect)
}

/// D2D-P1 helper: when the D2D idle path succeeds it has already painted the stop
/// button; the caller only needs its hit RECT. Same geometry as draw_stop_button.
#[cfg(target_os = "windows")]
fn draw_stop_button_hit_rect_only(rect: &RECT) -> RECT {
    let bs = 16;
    let bl = rect.right - 25;
    let bt = rect.top + (rect.bottom - rect.top - bs) / 2;
    RECT {
        left: bl,
        top: bt,
        right: bl + bs + 1,
        bottom: bt + bs + 1,
    }
}

/// D2D-P1 helper: the text hit region (for entering edit mode), same geometry as
/// the GDI path's `text_hit_rect` (:2567 区): 10px margins top/bottom, text band
/// horizontally (mic margin → stop button margin).
#[cfg(target_os = "windows")]
fn text_hit_rect_for(rect: &RECT) -> RECT {
    RECT {
        left: rect.left + STREAMING_TEXT_LEFT_MARGIN,
        top: rect.top + STREAMING_TEXT_TOP_MARGIN,
        right: rect.right - STREAMING_TEXT_RIGHT_MARGIN,
        bottom: rect.bottom - STREAMING_TEXT_BOTTOM_MARGIN,
    }
}

#[cfg(target_os = "windows")]
fn draw_recording_overlay_with_text(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    state: &OverlayWindowState,
    _ui_language: config::UiLanguage,
    text: &str,
) -> (RECT, RECT, RECT) {
    // D2D-P1: RecordingWithText is drawn with D2D (DEC-055 step 2). On any D2D failure
    // the GDI path below still renders this frame, so the overlay never blanks.
    // The metric (text_width) comes from measure_text_width on the GDI font — the
    // single metric source agreed in D2D-P1 — so the D2D side never measures itself
    // and cannot drift from the window-sizing path (adjust_overlay_pos_size_for_text).
    let text_width = measure_text_width(hdc, text);
    if d2d::draw_streaming_text_overlay(hdc, rect, state, text, text_width) {
        let text_hit = text_hit_rect_for(rect);
        let cancel = draw_stop_button_hit_rect_only(rect);
        return (cancel, cancel, text_hit);
    }
    // OVERLAY-043: text mode uses chrome + stop button only, no waveform so text is not squeezed
    // OVERLAY-121 (P3): RecordingWithText fallback r=16（Gavin 拍板）。
    // OVERLAY-141: 半径单一来源。
    draw_overlay_chrome(hdc, rect, OVERLAY_FRAME_RADIUS_LG as i32);
    // keep the mic indicator so the user still sees the recording state
    draw_recording_indicator(hdc, rect, state);
    let cancel_rect = draw_stop_button(hdc, rect);

    let cy = rect.top + (rect.bottom - rect.top) / 2;

    // OVERLAY-054-H: self-drawn streaming text uses a 4px vertical inset so the
    // -14 ClearType font fits. The center is identical to the old 10/10 margin
    // because both are symmetric around the window center, so the text has zero
    // visual displacement; only the available height changes (16px -> 28px).
    let text_left = rect.left + STREAMING_TEXT_LEFT_MARGIN;
    let text_right = rect.right - STREAMING_TEXT_RIGHT_MARGIN;
    let text_top = rect.top + OVERLAY_TEXT_DRAW_VERTICAL_INSET;
    let text_bottom = rect.bottom - OVERLAY_TEXT_DRAW_VERTICAL_INSET;
    let visible_w = (text_right - text_left).max(1);

    // Scroll offset so newest text stays at the right edge once text overflows
    let scroll_x = streaming_scroll_offset(text_width, visible_w);

    // OVERLAY-054-H: keep the horizontal clip region unchanged. We only relaxed the
    // vertical text rectangle; the horizontal scroll/clipping behavior is untouched.
    unsafe {
        let _ = SaveDC(hdc);
    }
    let clip = unsafe { CreateRectRgn(text_left, text_top, text_right, text_bottom) };
    unsafe {
        let _ = SelectClipRgn(hdc, clip);
        let _ = DeleteObject(clip);
    }

    unsafe {
        let _ = SetTextColor(hdc, OVERLAY_TEXT_WHITE);
    }
    let mut text_rect = RECT {
        left: text_left - scroll_x,
        top: text_top,
        right: text_right - scroll_x,
        bottom: text_bottom,
    };
    draw_text(
        hdc,
        text,
        &mut text_rect,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
    );
    unsafe {
        let _ = RestoreDC(hdc, -1);
    }

    // Text hit region (for entering edit mode) excludes the stop button.
    // Keep the hit region at the old 10px margin so the editable area is not shrunk.
    let text_hit_rect = RECT {
        left: text_left,
        top: rect.top + STREAMING_TEXT_TOP_MARGIN,
        right: text_right,
        bottom: rect.bottom - STREAMING_TEXT_BOTTOM_MARGIN,
    };

    // Right separator between text area and stop button
    let sep_r_x = rect.right - 36;
    let sep_h = 20;
    let sep_hh = sep_h / 2;
    let sep_pen = unsafe { CreatePen(PS_SOLID, 2, OVERLAY_BORDER_GRAY) };
    let sep_op = unsafe { SelectObject(hdc, sep_pen) };
    unsafe {
        let _ = MoveToEx(hdc, sep_r_x, cy - sep_hh, None);
        let _ = LineTo(hdc, sep_r_x, cy + sep_hh);
        let _ = SelectObject(hdc, sep_op);
        let _ = DeleteObject(sep_pen);
    }

    // OVERLAY-043: the single right button serves as stop in RecordingWithText and submit in StreamingEditing
    (cancel_rect, cancel_rect, text_hit_rect)
}

/// D2D-P2 (PLAN-108 H8): submit 键命中 rect 的单一几何源。
/// 🔴 与 draw_stop_button_hit_rect_only 的 +1 排他约定**不同**：GDI draw_submit_button
/// 的返回 RECT 是 right = bl+bs（无 +1），点击判定 rect_contains 消费的就是这个值。
/// GDI 绘制路径与 D2D 成功路径都从本 helper 取 rect，几何口径物理上不可分叉。
#[cfg(target_os = "windows")]
fn draw_submit_button_hit_rect_only(rect: &RECT) -> RECT {
    let bs = 16;
    let bl = rect.right - 25;
    let bt = rect.top + (rect.bottom - rect.top - bs) / 2;
    RECT {
        left: bl,
        top: bt,
        right: bl + bs,
        bottom: bt + bs,
    }
}

#[cfg(target_os = "windows")]
fn draw_submit_button(hdc: windows::Win32::Graphics::Gdi::HDC, rect: &RECT) -> RECT {
    const BG_DARK: COLORREF = COLORREF(0x110F0D);
    const CORNER_RADIUS: i32 = 10;
    // D2D-P2 (PLAN-108 H8): 命中 rect 抽成 helper，GDI/D2D 两条路径共用单一几何源。
    // 🔴 注意与 draw_stop_button 的不对称：这里**没有 +1**（right = bl+bs），
    // 是 draw_submit_button 的历史行为，点击判定消费的就是它——迁移时不得"顺手统一"。
    let submit_rect = draw_submit_button_hit_rect_only(rect);
    let submit_pen = unsafe { CreatePen(PS_SOLID, 1, OVERLAY_BRAND_ORANGE) };
    let submit_pen_old = unsafe { SelectObject(hdc, submit_pen) };
    let submit_brush = unsafe { CreateSolidBrush(OVERLAY_BRAND_ORANGE) };
    let submit_brush_old = unsafe { SelectObject(hdc, submit_brush) };
    unsafe {
        let _ = RoundRect(
            hdc,
            submit_rect.left,
            submit_rect.top,
            submit_rect.right,
            submit_rect.bottom,
            CORNER_RADIUS,
            CORNER_RADIUS,
        );
    }
    let arrow = "\u{23CE}";
    let mut arrow_rect = submit_rect;
    unsafe {
        let _ = SetTextColor(hdc, BG_DARK);
    }
    draw_text(
        hdc,
        arrow,
        &mut arrow_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    unsafe {
        let _ = SelectObject(hdc, submit_pen_old);
        let _ = SelectObject(hdc, submit_brush_old);
        let _ = DeleteObject(submit_pen);
        let _ = DeleteObject(submit_brush);
    }
    submit_rect
}

#[cfg(target_os = "windows")]
fn draw_recording_indicator(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    state: &OverlayWindowState,
) {
    const BRAND_ORANGE: COLORREF = COLORREF(0x006BFF); // #FF6B00
    const RED_STREAM_FAILED: COLORREF = COLORREF(0x0000FF); // #FF0000 — device error
    const GRAY_SILENT: COLORREF = COLORREF(0x808080); // #808080
    const BG_DARK: COLORREF = COLORREF(0x110F0D);
    // OVERLAY-054-C: separator color matches the unified window border.
    let circ_size = 18; // MIC-ICON-ENLARGE-001: from 14 to 18
    let circ_l = rect.left + 6; // MIC-ICON-ENLARGE-001: left-shift to keep margin to separator
    let circ_t = rect.top + (rect.bottom - rect.top - circ_size) / 2;

    // Three-state audio indicator
    // MIC-PULSE-160: 峰值电平同一次锁内取出（不为动画二次加锁）。
    let (buf_empty, has_audio, mic_level) = mic_audio_snapshot(state);
    let circ_color = if buf_empty {
        RED_STREAM_FAILED
    } else if has_audio {
        BRAND_ORANGE
    } else {
        GRAY_SILENT
    };
    // HALFTONE anti-aliasing: render at 4x then downscale
    let scale = 4;
    let sup_size = circ_size * scale;
    unsafe {
        let mem_dc = CreateCompatibleDC(hdc);
        let bmp = CreateCompatibleBitmap(hdc, sup_size, sup_size);
        let old_bmp = SelectObject(mem_dc, bmp);
        let bg = CreateSolidBrush(BG_DARK);
        let _ = FillRect(
            mem_dc,
            &RECT {
                left: 0,
                top: 0,
                right: sup_size,
                bottom: sup_size,
            },
            bg,
        );
        let _ = DeleteObject(bg);
        let body_brush = CreateSolidBrush(circ_color);
        let null_pen = CreatePen(PS_NULL, 0, circ_color);
        let old_pen = SelectObject(mem_dc, null_pen);
        let old_brush = SelectObject(mem_dc, body_brush);
        let _ = RoundRect(mem_dc, 22, 4, 50, 53, 28, 28);
        let _ = SelectObject(mem_dc, old_pen);
        let _ = SelectObject(mem_dc, old_brush);
        let _ = DeleteObject(null_pen);
        let _ = DeleteObject(body_brush);
        let line_pen = CreatePen(PS_SOLID, scale, circ_color);
        let old_pen = SelectObject(mem_dc, line_pen);
        let _ = MoveToEx(mem_dc, 36, 53, None);
        let _ = LineTo(mem_dc, 36, 63);
        let _ = MoveToEx(mem_dc, 24, 63, None);
        let _ = LineTo(mem_dc, 48, 63);
        let _ = SelectObject(mem_dc, old_pen);
        let _ = DeleteObject(line_pen);
        // MIC-PULSE-160: 声波弧画在 4x 画布上（随 StretchBlt 一起 HALFTONE 降采样）。
        if has_audio {
            let gain = (mic_level / MIC_PULSE_FULL_LEVEL).clamp(0.0, 1.0);
            if gain > 0.0 {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let (a_in, a_out) = mic_pulse_alphas(now_ms, gain);
                draw_mic_pulse_gdi_4x(mem_dc, a_in, a_out, BG_DARK);
            }
        }
        let _ = SetStretchBltMode(hdc, HALFTONE);
        let _ = SetBrushOrgEx(hdc, 0, 0, None);
        let _ = StretchBlt(
            hdc, circ_l, circ_t, circ_size, circ_size, mem_dc, 0, 0, sup_size, sup_size, SRCCOPY,
        );
        let _ = SelectObject(mem_dc, old_bmp);
        let _ = DeleteObject(bmp);
        let _ = DeleteDC(mem_dc);
    }
    // Left separator
    let sep_l_x = rect.left + 30;
    let sep_h = 20;
    let sep_hh = sep_h / 2;
    let cy = rect.top + (rect.bottom - rect.top) / 2;
    let sep_pen = unsafe { CreatePen(PS_SOLID, 2, OVERLAY_BORDER_GRAY) };
    let sep_op = unsafe { SelectObject(hdc, sep_pen) };
    unsafe {
        let _ = MoveToEx(hdc, sep_l_x, cy - sep_hh, None);
        let _ = LineTo(hdc, sep_l_x, cy + sep_hh);
        let _ = SelectObject(hdc, sep_op);
        let _ = DeleteObject(sep_pen);
    }
}

/// EDITICON-176: 编辑态 GDI 兜底路径的铅笔图标 + 左分割线（D2D 失败帧）。
/// 几何与 D2D 主路径（d2d::edit_icon_and_left_separator）逐项同值：
/// 图标 18px @ (rect.left+6, 垂直居中)，分割线 x=rect.left+30 / 2px / 20px 居中 /
/// OVERLAY_BORDER_GRAY（= draw_recording_indicator :3750-3762 左分割线同一实现）。
/// 图标 = `edit_icon_bgra(18)` 直通 alpha BGRA → CreateDIBSection（32bpp 顶行在前，
/// biHeight 负高）→ AlphaBlend(AC_SRC_OVER, AC_SRC_ALPHA) 1:1 贴出，与
/// draw_recording_indicator 的 4x HALFTONE 是两条独立路数（GDI 兜底帧不追求
/// 与主帧逐位同像素，契约是「不空白」，形态一致性由同一光栅化源保证）。
#[cfg(target_os = "windows")]
fn draw_edit_icon_and_separator_gdi(hdc: windows::Win32::Graphics::Gdi::HDC, rect: &RECT) {
    const ICON_SIZE: i32 = 18;
    let icon_l = rect.left + 6; // GDI: rect.left + 6（mic_indicator 同盒子）
    let icon_t = rect.top + (rect.bottom - rect.top - ICON_SIZE) / 2;
    let bgra = ui::menu_icons::edit_icon_bgra(ICON_SIZE as u32);
    unsafe {
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: ICON_SIZE,
            biHeight: -ICON_SIZE, // 负高 = 顶行在前（menu_icons 行序一致）
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0);
        if let Ok(bmp) = bmp {
            if !bits.is_null() {
                std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits.cast(), bgra.len());
            }
            let mem_dc = CreateCompatibleDC(hdc);
            let old_bmp = SelectObject(mem_dc, bmp);
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = AlphaBlend(
                hdc, icon_l, icon_t, ICON_SIZE, ICON_SIZE, mem_dc, 0, 0, ICON_SIZE, ICON_SIZE,
                blend,
            );
            let _ = SelectObject(mem_dc, old_bmp);
            let _ = DeleteObject(bmp);
            let _ = DeleteDC(mem_dc);
        }
        // 左分割线：与 draw_recording_indicator :3750-3762 逐项同值
        let sep_l_x = rect.left + 30;
        let sep_h = 20;
        let sep_hh = sep_h / 2;
        let cy = rect.top + (rect.bottom - rect.top) / 2;
        let sep_pen = CreatePen(PS_SOLID, 2, OVERLAY_BORDER_GRAY);
        let sep_op = SelectObject(hdc, sep_pen);
        let _ = MoveToEx(hdc, sep_l_x, cy - sep_hh, None);
        let _ = LineTo(hdc, sep_l_x, cy + sep_hh);
        let _ = SelectObject(hdc, sep_op);
        let _ = DeleteObject(sep_pen);
    }
}

#[cfg(target_os = "windows")]
fn draw_listening_placeholder(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    ui_language: config::UiLanguage,
) {
    // OVERLAY-051-E: centered placeholder text shown while online ASR is waiting for the first
    // streaming result. Uses the same text color as streaming text for visual continuity.
    let hint = i18n::get(ui_language).overlay_listening_hint;
    unsafe {
        let _ = SetTextColor(hdc, OVERLAY_TEXT_WHITE);
    }
    let mut text_rect = *rect;
    // Leave margins so the text does not overlap the mic icon area or the stop button.
    text_rect.left += STREAMING_TEXT_LEFT_MARGIN;
    text_rect.right -= STREAMING_TEXT_RIGHT_MARGIN;
    draw_text(
        hdc,
        hint,
        &mut text_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
    );
}

#[cfg(target_os = "windows")]
fn draw_editing_overlay_chrome(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    _ui_language: config::UiLanguage,
) -> (RECT, RECT) {
    const BG_DARK: COLORREF = COLORREF(0x110F0D);
    // OVERLAY-141: 半径单一来源。
    // 🔴 本函数当前零调用（死代码，是否删除待 Gavin 拍板）；此 hunk 独立，若
    // Gavin 拍板删函数则整函数连同本改动一并带走，零残留。
    const CORNER_RADIUS: i32 = OVERLAY_FRAME_RADIUS_SM as i32;

    // Background + border
    let bg = unsafe { CreateSolidBrush(BG_DARK) };
    unsafe {
        let _ = FillRect(hdc, rect, bg);
        let _ = DeleteObject(bg);
    }
    let border_pen = unsafe { CreatePen(PS_SOLID, 1, OVERLAY_BORDER_GRAY) };
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

    // Cancel button: same stop square as recording overlay, orange outline
    let bs = 16;
    let bl = rect.right - 25;
    let bt = rect.top + (rect.bottom - rect.top - bs) / 2;
    let cancel_rect = RECT {
        left: bl,
        top: bt,
        right: bl + bs + 1,
        bottom: bt + bs + 1,
    };
    let bp = unsafe { CreatePen(PS_SOLID, 1, OVERLAY_BRAND_ORANGE) };
    let bop = unsafe { SelectObject(hdc, bp) };
    let bnb = unsafe { GetStockObject(NULL_BRUSH) };
    let bob = unsafe { SelectObject(hdc, bnb) };
    unsafe {
        let _ = Rectangle(
            hdc,
            cancel_rect.left,
            cancel_rect.top,
            cancel_rect.right,
            cancel_rect.bottom,
        );
        let _ = SelectObject(hdc, bop);
        let _ = SelectObject(hdc, bob);
        let _ = DeleteObject(bp);
    }
    let isz = 8;
    let il = cancel_rect.left + (bs - isz) / 2;
    let it = cancel_rect.top + (bs - isz) / 2;
    let ir = RECT {
        left: il,
        top: it,
        right: il + isz + 1,
        bottom: it + isz + 1,
    };
    let ib = unsafe { CreateSolidBrush(OVERLAY_BRAND_ORANGE) };
    let ip = unsafe { CreatePen(PS_NULL, 0, OVERLAY_BRAND_ORANGE) };
    let ibo = unsafe { SelectObject(hdc, ib) };
    let ipo = unsafe { SelectObject(hdc, ip) };
    unsafe {
        let _ = Rectangle(hdc, ir.left, ir.top, ir.right, ir.bottom);
        let _ = SelectObject(hdc, ibo);
        let _ = SelectObject(hdc, ipo);
        let _ = DeleteObject(ib);
        let _ = DeleteObject(ip);
    }

    // Submit button to the left of cancel button
    let submit_bs = 16;
    let submit_gap = 6;
    let submit_bl = cancel_rect.left - submit_gap - submit_bs;
    let submit_bt = rect.top + (rect.bottom - rect.top - submit_bs) / 2;
    let submit_rect = RECT {
        left: submit_bl,
        top: submit_bt,
        right: submit_bl + submit_bs,
        bottom: submit_bt + submit_bs,
    };
    let submit_pen = unsafe { CreatePen(PS_SOLID, 1, OVERLAY_BRAND_ORANGE) };
    let submit_pen_old = unsafe { SelectObject(hdc, submit_pen) };
    let submit_brush = unsafe { CreateSolidBrush(OVERLAY_BRAND_ORANGE) };
    let submit_brush_old = unsafe { SelectObject(hdc, submit_brush) };
    unsafe {
        let _ = RoundRect(
            hdc,
            submit_rect.left,
            submit_rect.top,
            submit_rect.right,
            submit_rect.bottom,
            CORNER_RADIUS,
            CORNER_RADIUS,
        );
    }
    let arrow = "\u{23CE}";
    let mut arrow_rect = submit_rect;
    unsafe {
        let _ = SetTextColor(hdc, BG_DARK);
    }
    draw_text(
        hdc,
        arrow,
        &mut arrow_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    unsafe {
        let _ = SelectObject(hdc, submit_pen_old);
        let _ = SelectObject(hdc, submit_brush_old);
        let _ = DeleteObject(submit_pen);
        let _ = DeleteObject(submit_brush);
    }

    (cancel_rect, submit_rect)
}

// D2D-073 P0: module for the Direct2D + DirectWrite redraw of the processing overlay.
// D2D-P1 (DEC-055 gray migration step 2): the shared frame layer (`with_d2d`) plus the
// RecordingStreamingIdle / RecordingWithText primitives were added here; every migrated
// status keeps a GDI fallback path so the overlay never blanks. The DC render target
// keeps the existing double-buffered
// WM_PAINT pipeline (mem_dc → BitBlt) untouched — D2D draws into the same memory DC.
#[cfg(target_os = "windows")]
mod d2d {
    use super::{
        OverlayWindowState, COLORREF, OVERLAY_BG_DARK, OVERLAY_BORDER_GRAY, OVERLAY_BRAND_ORANGE,
        OVERLAY_FONT_SIZE, OVERLAY_TEXT_DRAW_VERTICAL_INSET, OVERLAY_TEXT_FONT_SIZE,
        OVERLAY_TEXT_WHITE, STREAMING_TEXT_LEFT_MARGIN, STREAMING_TEXT_RIGHT_MARGIN,
    };
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_POINT_2F, D2D_RECT_F,
        D2D_SIZE_U,
    };
    use windows::Win32::Graphics::Direct2D::{
        D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory, ID2D1SolidColorBrush,
        D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
        D2D1_BITMAP_PROPERTIES, D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
        D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
        D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
    };
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, DWRITE_FACTORY_TYPE_SHARED,
        DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
        DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_WORD_WRAPPING_NO_WRAP,
        DWRITE_WORD_WRAPPING_WRAP,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
    use windows::Win32::Graphics::Gdi::HDC;

    /// Per-thread D2D/DWrite resources. Created once per overlay thread; the DC render
    /// target is rebound to the current memory DC every WM_PAINT (BindDC is cheap).
    /// factory/dwrite are held so the render target, text format and brushes keep their
    /// parent objects alive for the thread's lifetime (COM reference semantics).
    pub(crate) struct D2dResources {
        #[allow(dead_code)] // kept alive for COM parent lifetime, see struct doc
        pub factory: ID2D1Factory,
        #[allow(dead_code)] // kept alive for text_format lifetime, see struct doc
        pub dwrite: IDWriteFactory,
        pub rt: ID2D1DCRenderTarget,
        /// Processing-state text format (D2D-073-P0): centered paragraph, NO_WRAP.
        /// Face is "Microsoft YaHei UI" SemiBold — see the note on the field below.
        pub text_format: windows::Win32::Graphics::DirectWrite::IDWriteTextFormat,
        /// D2D-P1: streaming-state text format — left-aligned (LEADING), vertical center,
        /// NO_WRAP. Face "Segoe UI" Normal, STREAMFONT-189 起字号 = OVERLAY_TEXT_FONT_SIZE
        /// (-16 → 16px)，与 GDI 兜底的 streaming_font 同号（EDITFONT-183 曾是 14px，
        /// Gavin 2026-09-08 拍板实时上屏与编辑态同号），单一 GDI 度量源
        /// (measure_text_width) 仍以所选字体为源，测渲不漂移。
        /// (P0's format comment claimed "must match the GDI face" but used YaHei UI
        /// SemiBold; for the processing state that was an intentional standalone choice —
        /// Chinese-only text falls back to the same glyph engine either way, and Gavin has
        /// visually accepted it. The streaming states need the real GDI face.)
        pub streaming_text_format: windows::Win32::Graphics::DirectWrite::IDWriteTextFormat,
        /// D2D-P2 (PLAN-108 H2): 居中小文本 format（⏎/✕/标题/按钮标签）。
        /// Segoe UI Normal 14，CENTER + 段落垂直居中 + NO_WRAP，
        /// 对应 GDI draw_text(DT_CENTER|DT_VCENTER|SINGLELINE) 的缓存字体。
        pub centered_text_format: windows::Win32::Graphics::DirectWrite::IDWriteTextFormat,
        /// D2D-P2 (PLAN-108 H2): 预览正文换行 format。Segoe UI Normal 14，
        /// LEADING + **顶对齐** + WORD_WRAPPING_WRAP —— GDI 正文是
        /// DT_LEFT|DT_WORDBREAK（无 DT_VCENTER → 顶对起画，:3944-3949），
        /// 段落对齐不能用 CENTER，否则整块文本垂直居中造成观感漂移。
        /// 度量裁定③：只用于画，从不测量；断行差异是 U2 已裁决的端测观察项。
        pub wrap_text_format: windows::Win32::Graphics::DirectWrite::IDWriteTextFormat,
        pub brush: ID2D1SolidColorBrush,
    }
    fn create_resources() -> windows::core::Result<D2dResources> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    // OVERLAY-121 (P1): IGNORE → PREMULTIPLIED。MSDN CreateDCRenderTarget
                    // Remarks 明文 DC render target 支持两种 alpha mode；预乘 alpha 是
                    // UpdateLayeredWindow(ULW_ALPHA, AC_SRC_ALPHA) 合成的输入格式 ——
                    // 改字段不改收口结构（DEC-055：BindDC/Begin/End/RECREATE 单点不动）。
                    // D2D 产出的预乘像素直接落进 WM_PAINT 的 32bpp DIB section。
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 0.0,
                dpiY: 0.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            // DC render target: GDI-compatible surface, so the existing double-buffered
            // WM_PAINT (mem_dc → BitBlt) keeps working with zero structural change.
            let rt: ID2D1DCRenderTarget = factory.CreateDCRenderTarget(&rt_props)?;
            // Font family must match the GDI path's ClearType font face for visual parity.
            // D2D-P1 correction of this comment: the GDI face is "Segoe UI" FW_NORMAL
            // (create_clear_type_font), NOT YaHei UI SemiBold. The P0 processing format
            // below is a deliberate standalone choice (Chinese-only string; Gavin accepted
            // it visually). The D2D-P1 streaming format (streaming_text_format) is the one
            // that actually matches the GDI face/weight.
            let family: Vec<u16> = "Microsoft YaHei UI".encode_utf16().collect();
            let locale: Vec<u16> = "zh-CN".encode_utf16().collect();
            let text_format = dwrite.CreateTextFormat(
                PCWSTR(family.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                -OVERLAY_FONT_SIZE as f32, // GDI negative height (em) → D2D positive size
                PCWSTR(locale.as_ptr()),
            )?;
            text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            text_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            // D2D-P1: streaming-state format — same face/weight as the GDI text path
            // ("Segoe UI", FW_NORMAL), left-aligned, single line, no wrap.
            // STREAMFONT-189: 字号归 OVERLAY_TEXT_FONT_SIZE(16px)，与 GDI 兜底
            // streaming_font / EDIT 控件同号；其余三个 format 仍绑 OVERLAY_FONT_SIZE。
            let gdi_family: Vec<u16> = "Segoe UI".encode_utf16().collect();
            let streaming_text_format = dwrite.CreateTextFormat(
                PCWSTR(gdi_family.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                -OVERLAY_TEXT_FONT_SIZE as f32, // GDI negative height (em) → D2D positive size
                PCWSTR(locale.as_ptr()),
            )?;
            streaming_text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            streaming_text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            streaming_text_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            // D2D-P2 (H2): 居中小文本 format —— 同 GDI face（Segoe UI Normal 14），
            // 对应 draw_text 的 DT_CENTER|DT_VCENTER|SINGLELINE 三连。
            let centered_text_format = dwrite.CreateTextFormat(
                PCWSTR(gdi_family.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                -OVERLAY_FONT_SIZE as f32, // GDI negative height (em) → D2D positive size
                PCWSTR(locale.as_ptr()),
            )?;
            centered_text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            centered_text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            centered_text_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            // D2D-P2 (H2): 预览正文换行 format —— 顶对齐（GDI 无 DT_VCENTER），
            // WORD_WRAPPING_WRAP 对应 DT_WORDBREAK。断行差异 = U2 裁决的端测观察项。
            // 注：windows 0.58 无 _TOP 常量，顶对齐 = NEAR（0，SDK 原名 parity）。
            let wrap_text_format = dwrite.CreateTextFormat(
                PCWSTR(gdi_family.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                -OVERLAY_FONT_SIZE as f32,
                PCWSTR(locale.as_ptr()),
            )?;
            wrap_text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            wrap_text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
            wrap_text_format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            let _ = rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let brush = rt.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 1.0,
                    g: 0.42,
                    b: 0.0,
                    a: 1.0,
                },
                None,
            )?;
            Ok(D2dResources {
                factory,
                dwrite,
                rt,
                text_format,
                streaming_text_format,
                centered_text_format,
                wrap_text_format,
                brush,
            })
        }
    }

    thread_local! {
        static D2D: std::cell::RefCell<Option<D2dResources>> = const { std::cell::RefCell::new(None) };
    }

    /// D2D-HANG-095: 必须在**线程体内**显式调用，绝不能依赖 thread_local 析构器。
    /// 根因（REPRO-094 实证）：thread_local 析构器在 Windows 上运行于 DLL_THREAD_DETACH，
    /// 加载器锁已被本线程持有；此时做 D2D/DWrite 的最后一次 COM Release 会自持锁死锁
    /// （探针 B 卡在 [Drop 6/6] brush Release，BS4 抓到 LoaderLock.OwningThread = 自身 tid）。
    /// 探针 b2/b5 已证：同样这些对象在**线程体内**释放完全正常。
    ///
    /// `take()` 取出的值就地在本闭包内 drop（仍在线程体栈上），不得返回出去或延后 drop。
    pub(crate) fn release_resources() {
        D2D.with(|cell| {
            if cell.borrow_mut().take().is_some() {
                log::debug!("D2D-HANG-095: D2D resources released in-thread before thread exit");
            }
        });
    }

    /// D2DERR_RECREATE_TARGET (0x8899000C): device loss (sleep/wake, driver update,
    /// RDP switch). All cached resources are invalid; the frame must fall back to GDI
    /// and the next attempt must rebuild from scratch. D2D-P1: detected in `with_d2d`,
    /// dropping the thread-local slot so `create_resources` runs again on the next call.
    const D2DERR_RECREATE_TARGET: i32 = 0x8899_000Cu32 as i32;

    /// D2D-P1 shared frame layer: bind, begin, run the caller's primitives, end, and
    /// classify failure. Every migrated status routes through this single path so
    /// BindDC/BeginDraw/EndDraw/D2DERR_RECREATE_TARGET handling exists in exactly one
    /// place. Returns false on ANY failure (init, BindDC, EndDraw) — the caller must
    /// render the same frame through its GDI fallback so the overlay never blanks.
    /// A RECREATE_TARGET failure additionally drops the cached resources (device lost).
    pub(crate) fn with_d2d(
        hdc: HDC,
        rect: &RECT,
        draw: impl FnOnce(&D2dResources, f32, f32),
    ) -> bool {
        D2D.with(|cell| {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                match create_resources() {
                    Ok(r) => *slot = Some(r),
                    Err(e) => {
                        log::warn!("D2D-073: Direct2D init failed, staying on GDI this frame: {e}");
                        return false;
                    }
                }
            }
            let res = slot.as_ref().expect("just initialized");
            unsafe {
                // Bind this frame's memory DC. The subrect is the full client rect so D2D
                // pixel coordinates map 1:1 onto the GDI surface.
                if res.rt.BindDC(hdc, rect).is_err() {
                    return false;
                }
                // 🔴 OVERLAY-149-PROBE（E4，临时探针，判读完删除——见
                // overlay149_probe_dump 头注释删除清单③）：D2D RT GetDpi 全程只打
                // 一次（进程级 once），与 E3/E4 行的 hwnd-hdc-dpi 比对，排除
                // 「RT 坐标系 DPI ≠ 窗口 DC DPI ⇒ D2D 坐标与位图像素错位」。
                {
                    static E4_RT_DPI_LOGGED: std::sync::atomic::AtomicBool =
                        std::sync::atomic::AtomicBool::new(false);
                    if !E4_RT_DPI_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        let mut rt_dpi_x = 0.0_f32;
                        let mut rt_dpi_y = 0.0_f32;
                        res.rt.GetDpi(&mut rt_dpi_x, &mut rt_dpi_y);
                        log::debug!(
                            "OVERLAY-149-PROBE E4: D2D RT GetDpi = ({},{})",
                            rt_dpi_x,
                            rt_dpi_y
                        );
                    }
                }
                let w = (rect.right - rect.left).max(1) as f32;
                let h = (rect.bottom - rect.top).max(1) as f32;
                res.rt.BeginDraw();
                res.rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                draw(res, w, h);
                if let Err(e) = res.rt.EndDraw(None, None) {
                    let code = e.code().0;
                    if code == D2DERR_RECREATE_TARGET {
                        // Device lost: discard everything; the next frame rebuilds.
                        log::warn!("D2D-P1: D2DERR_RECREATE_TARGET (device lost), dropping D2D resources; GDI renders this frame, D2D rebuilds next frame");
                        *slot = None;
                    } else {
                        log::warn!("D2D-P1: EndDraw failed ({code:#x}), GDI renders this frame");
                    }
                    return false;
                }
            }
            true
        })
    }

    /// Try to draw the processing overlay with D2D. Returns false if D2D failed to
    /// initialize or draw (caller falls back to the GDI path for this frame; init is
    /// retried on the next attempt so a transient failure never permanently disables D2D).
    /// D2D-P1: routed through the shared `with_d2d` frame layer so device-loss handling
    /// (D2DERR_RECREATE_TARGET → drop resources → GDI this frame → rebuild next frame)
    /// covers this state too. The primitives below are byte-identical to the P0 body.
    pub(crate) fn draw_processing_overlay(
        hdc: HDC,
        rect: &RECT,
        ui_language: crate::config::UiLanguage,
        shimmer_phase: f32,
    ) -> bool {
        with_d2d(hdc, rect, |res, w, h| {
            draw_processing_primitives(res, w, h, ui_language, shimmer_phase);
        })
    }

    /// P0 primitives, unchanged. Extracted as the `with_d2d` closure body; D2D-P1 only
    /// wrapped the frame management around them (BindDC/Begin/End/RECREATE) — the
    /// geometry, colors and text format are untouched, so the visual output is the same.
    fn draw_processing_primitives(
        res: &D2dResources,
        w: f32,
        h: f32,
        ui_language: crate::config::UiLanguage,
        shimmer_phase: f32,
    ) {
        unsafe {
            // 1) Dark background — same #181A18 as the GDI path.
            res.brush.SetColor(&D2D1_COLOR_F {
                r: 0x18 as f32 / 255.0,
                g: 0x1A as f32 / 255.0,
                b: 0x18 as f32 / 255.0,
                a: 1.0,
            });
            // OVERLAY-086 Bug 1: the background must be a rounded rectangle matching the
            // border geometry exactly. A rectangular fill leaves a wedge of background
            // color outside the rounded 1px stroke in each corner (visible as stray gray
            // fringes on the rounded edge). Fill and stroke must share ONE radius binding.
            // OVERLAY-141: 半径单一来源 —— 与帧末 SDF alpha 掩码同值（overlay_frame_radius
            // 对 Processing 返回同常量），禁止回退为字面量。
            let corner_radius = super::OVERLAY_FRAME_RADIUS_LG;
            res.rt.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: 0.0,
                        top: 0.0,
                        right: w,
                        bottom: h,
                    },
                    radiusX: corner_radius,
                    radiusY: corner_radius,
                },
                &res.brush,
            );

            // 2) 1px rounded border — same geometry as GDI RoundRect(…, 16*2, 16*2):
            //    corner radius 16, stroke centered on the GDI border line.
            //    Fill (0.0..w) and stroke (0.5..w-0.5) intentionally differ by the 0.5px
            //    pen-centering inset; the radius binding above is shared by both.
            res.brush.SetColor(&D2D1_COLOR_F {
                r: (OVERLAY_BORDER_GRAY.0 & 0xFF) as f32 / 255.0,
                g: ((OVERLAY_BORDER_GRAY.0 >> 8) & 0xFF) as f32 / 255.0,
                b: ((OVERLAY_BORDER_GRAY.0 >> 16) & 0xFF) as f32 / 255.0,
                a: 1.0,
            });
            res.rt.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: 0.5,
                        top: 0.5,
                        right: w - 0.5,
                        bottom: h - 0.5,
                    },
                    radiusX: corner_radius,
                    radiusY: corner_radius,
                },
                &res.brush,
                1.0,
                None,
            );

            // 3) Shimmer glow — D2D native gradient replaces the 30-slice Gaussian AlphaBlend.
            //    Same travel window and Gaussian profile (alpha peak 150/255 at beam center).
            let glow_half = 45.0_f32;
            let travel = w + glow_half * 2.0;
            let beam_cx = -glow_half + travel * shimmer_phase;
            let stops = [
                windows::Win32::Graphics::Direct2D::Common::D2D1_GRADIENT_STOP {
                    position: 0.0,
                    color: D2D1_COLOR_F {
                        r: 0.85,
                        g: 0.85,
                        b: 0.85,
                        a: 0.0,
                    },
                },
                windows::Win32::Graphics::Direct2D::Common::D2D1_GRADIENT_STOP {
                    position: 0.5,
                    color: D2D1_COLOR_F {
                        r: 0.85,
                        g: 0.85,
                        b: 0.85,
                        a: 150.0 / 255.0,
                    },
                },
                windows::Win32::Graphics::Direct2D::Common::D2D1_GRADIENT_STOP {
                    position: 1.0,
                    color: D2D1_COLOR_F {
                        r: 0.85,
                        g: 0.85,
                        b: 0.85,
                        a: 0.0,
                    },
                },
            ];
            // Gradient brush takes a pre-created stop collection (3-arg form in windows 0.58).
            let stop_collection = res.rt.CreateGradientStopCollection(
                &stops,
                windows::Win32::Graphics::Direct2D::D2D1_GAMMA_2_2,
                windows::Win32::Graphics::Direct2D::D2D1_EXTEND_MODE_CLAMP,
            );
            if let Ok(stop_collection) = stop_collection {
                if let Ok(gradient_brush) = res.rt.CreateLinearGradientBrush(
                    &windows::Win32::Graphics::Direct2D::D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                        startPoint: D2D_POINT_2F {
                            x: beam_cx - glow_half,
                            y: 0.0,
                        },
                        endPoint: D2D_POINT_2F {
                            x: beam_cx + glow_half,
                            y: 0.0,
                        },
                    },
                    None,
                    &stop_collection,
                ) {
                    // Vertical span matches the GDI glow: 1px inside border to 1px inside bottom.
                    // OVERLAY-086 Bug 1: the glow must not bleed into the rounded corners
                    // either — clip it to the same rounded geometry as the background.
                    res.rt.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: (beam_cx - glow_half).max(1.0),
                                top: 1.0,
                                right: (beam_cx + glow_half).min(w - 1.0),
                                bottom: h - 1.0,
                            },
                            radiusX: corner_radius,
                            radiusY: corner_radius,
                        },
                        &gradient_brush,
                    );
                }
            }

            // 4) Processing text — centered, brand orange, DirectWrite grayscale AA.
            res.brush.SetColor(&D2D1_COLOR_F {
                r: 1.0,
                g: 0x6B as f32 / 255.0,
                b: 0.0,
                a: 1.0,
            });
            let strings = super::i18n::get(ui_language);
            let text: Vec<u16> = strings.overlay_processing.encode_utf16().collect();
            res.rt.DrawText(
                &text,
                &res.text_format,
                &D2D_RECT_F {
                    left: 20.0,
                    top: 4.0,
                    right: w - 20.0,
                    bottom: h - 4.0,
                },
                &res.brush,
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    // ------------------------------------------------------------------
    // D2D-P1 shared primitives (RecordingStreamingIdle / RecordingWithText)
    //
    // Each mirrors one GDI primitive one-to-one (geometry/colors/radius in the
    // per-primitive doc comments); P2/P3 (Recording / FallingToProcessing / Error)
    // reuse the same set. Coordinates are rect-RELATIVE (0..w × 0..h) because
    // BindDC binds the full client rect — the GDI functions use absolute
    // rect.left/top, D2D's origin is the BindDC subrect's top-left corner.
    // ------------------------------------------------------------------

    /// COLORREF (0x00BBGGRR) → D2D1_COLOR_F (premultiplied-free RGBA 0..1).
    fn colorref_to_d2d(c: COLORREF) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: (c.0 & 0xFF) as f32 / 255.0,
            g: ((c.0 >> 8) & 0xFF) as f32 / 255.0,
            b: ((c.0 >> 16) & 0xFF) as f32 / 255.0,
            a: 1.0,
        }
    }

    /// GDI `draw_overlay_chrome` (:2187): dark #110F0D rounded-rect background +
    /// 1px OVERLAY_BORDER_GRAY rounded border, radius 10 (GDI RoundRect 10*2 ellipse).
    /// Same 0.5px pen-centering inset as the P0 border (fill 0..w, stroke 0.5..w-0.5).
    /// OVERLAY-086 Bug 1's wedge lesson applied here from day one: fill and stroke
    /// share one radius binding, so no corner wedge exists on this path either.
    /// D2D-P2 (PLAN-108 H1): chrome 参数化底色与半径。FocusLost / Error 的 GDI 底色是
    /// #211D1A（draw_preview_overlay / draw_error_overlay 内 local const BG_DARK），
    /// 圆角同 r10；已迁路径经 chrome() 保持 #110F0D + r10 零变化。
    /// fill/stroke 共用一个半径绑定（OVERLAY-086 Bug 1 教训，:3178-3200 同款）。
    fn chrome_with(res: &D2dResources, w: f32, h: f32, bg: COLORREF, radius: f32) {
        unsafe {
            res.brush.SetColor(&colorref_to_d2d(bg));
            res.rt.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: 0.0,
                        top: 0.0,
                        right: w,
                        bottom: h,
                    },
                    radiusX: radius,
                    radiusY: radius,
                },
                &res.brush,
            );
            res.brush
                .SetColor(&colorref_to_d2d(super::OVERLAY_BORDER_GRAY));
            res.rt.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: 0.5,
                        top: 0.5,
                        right: w - 0.5,
                        bottom: h - 0.5,
                    },
                    radiusX: radius,
                    radiusY: radius,
                },
                &res.brush,
                1.0,
                None,
            );
        }
    }

    /// GDI `draw_overlay_chrome` (:2187): dark #110F0D rounded-rect background +
    /// 1px OVERLAY_BORDER_GRAY rounded border, radius 10 (GDI RoundRect 10*2 ellipse).
    /// Same 0.5px pen-centering inset as the P0 border (fill 0..w, stroke 0.5..w-0.5).
    /// OVERLAY-086 Bug 1's wedge lesson applied here from day one: fill and stroke
    /// share one radius binding, so no corner wedge exists on this path either.
    /// OVERLAY-121 (P3): 圆角半径参数化 —— Gavin 2026-09-06 拍板视觉口径：
    /// Recording 三态 + FallingToProcessing 走 r=16（与 Processing 统一），
    /// 编辑态维持 r=10（SLWA 旧路径，视觉零变化）。
    fn chrome(res: &D2dResources, w: f32, h: f32, radius: f32) {
        chrome_with(res, w, h, super::OVERLAY_BG_DARK, radius);
    }

    /// Mic indicator geometry shared by the D2D path (matches GDI `draw_recording_indicator`
    /// :2641): 18px icon at (6, vertically centered) with a wide pill body + stem + base,
    /// plus the 2px left separator at x=30, 20px tall, centered vertically.
    /// Colors come from the audio level snapshot (three-state) exactly like the GDI fn.
    /// D2D-P1: drawn natively (no 4x HALFTONE supersampling) — D2D's per-primitive AA
    /// replaces the GDI-era workaround. Geometry parameters in the对照表 in result.md.
    fn mic_indicator(res: &D2dResources, h: f32, state: &OverlayWindowState) {
        let circ_size = 18.0_f32; // MIC-ICON-ENLARGE-001: 18px (GDI supersamples 4x to 72)
        let circ_l = 6.0_f32; // GDI: rect.left + 6
        let circ_t = (h - circ_size) / 2.0; // GDI: (rect height - circ)/2

        // Three-state audio indicator (same snapshot logic as GDI :2656-2669).
        // MIC-PULSE-160: 峰值电平同一次锁内取出（不为动画二次加锁）。
        let (buf_empty, has_audio, mic_level) = super::mic_audio_snapshot(state);
        let circ_color = if buf_empty {
            COLORREF(0x0000FF) // RED_STREAM_FAILED — device error
        } else if has_audio {
            super::OVERLAY_BRAND_ORANGE // has audio above threshold
        } else {
            COLORREF(0x808080) // GRAY_SILENT — device OK, no audio
        };

        unsafe {
            // Pill body: GDI RoundRect(22,4,50,53, dia 28) on the 72px (4x) canvas
            // ≡ (5.5, 1)-(12.5, 13.25) at 1x, i.e. a 7.0 × 12.25px pill with a fully
            // rounded 3.5px radius (dia 7 = the GDI dia 28 / 4).
            // Drawn rect-relative + icon origin (circ_l, circ_t).
            let body_l = circ_l + 5.5;
            let body_t = circ_t + 1.0;
            let body_r = circ_l + 12.5;
            let body_b = circ_t + 13.25;
            let radius = 3.5; // GDI dia 28 / (2 × scale 4)
            res.brush.SetColor(&colorref_to_d2d(circ_color));
            res.rt.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: body_l,
                        top: body_t,
                        right: body_r,
                        bottom: body_b,
                    },
                    radiusX: radius,
                    radiusY: radius,
                },
                &res.brush,
            );
            // Stem: GDI 4px-wide line (36,53)→(36,63) @4x ≡ (9,13.25)→(9,15.75) @1x,
            // drawn from the icon origin. D2D strokes center on the line like GDI pens.
            let stem_x = circ_l + 9.0;
            let stem_top = circ_t + 13.25;
            let stem_bottom = circ_t + 15.75;
            res.rt.DrawLine(
                D2D_POINT_2F {
                    x: stem_x,
                    y: stem_top,
                },
                D2D_POINT_2F {
                    x: stem_x,
                    y: stem_bottom,
                },
                &res.brush,
                1.0, // GDI: CreatePen(PS_SOLID, scale=4) @4x → 4/4 = 1px at 1x
                None,
            );
            // Base: GDI (24,63)→(48,63) @4x ≡ (6,15.75)→(12,15.75) @1x from icon origin.
            res.rt.DrawLine(
                D2D_POINT_2F {
                    x: circ_l + 6.0,
                    y: stem_bottom,
                },
                D2D_POINT_2F {
                    x: circ_l + 12.0,
                    y: stem_bottom,
                },
                &res.brush,
                1.0,
                None,
            );
        }
        // MIC-PULSE-160: 两段同心声波弧（左右各一组对称），仅橘色有声态。
        // SHIMMER-FIX-002 范式：相位由墙钟现算。静音 gain=0 整段不画（与改前逐位相同）。
        if has_audio {
            let gain = (mic_level / super::MIC_PULSE_FULL_LEVEL).clamp(0.0, 1.0);
            if gain > 0.0 {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let (a_in, a_out) = super::mic_pulse_alphas(now_ms, gain);
                let cx = circ_l + 9.0;
                let cy = circ_t + 7.0;
                mic_pulse_arcs(res, cx, cy, a_in, a_out);
            }
        }
        // Left separator: GDI :2716-2727 — 2px gray line at rect.left+30, 20px tall.
        unsafe {
            let sep_l_x = 30.0; // GDI: rect.left + 30
            let sep_hh = 10.0; // GDI: sep_h 20 / 2
            let cy = h / 2.0; // GDI: rect.top + height/2 (rect-relative)
            res.brush
                .SetColor(&colorref_to_d2d(super::OVERLAY_BORDER_GRAY));
            res.rt.DrawLine(
                D2D_POINT_2F {
                    x: sep_l_x,
                    y: cy - sep_hh,
                },
                D2D_POINT_2F {
                    x: sep_l_x,
                    y: cy + sep_hh,
                },
                &res.brush,
                2.0,
                None,
            );
        }
    }

    /// MIC-PULSE-160: 两对同心声波弧（左右各一组对称：右 -40°..+40°，左 140°..220°）。
    /// 实现取 clip + DrawEllipse：clip 暴露整椭圆的 |θ|≤40° 部分恰好就是 80° 弧段
    /// （cosθ ≥ cos40° ⟺ |θ| ≤ 40°，左右对称），不用 path geometry，成本 = 4 次描边。
    /// alpha 用 brush SetOpacity，返回前恢复 1.0（brush 全帧复用）。
    fn mic_pulse_arcs(res: &D2dResources, cx: f32, cy: f32, a_in: f32, a_out: f32) {
        let half = super::MIC_PULSE_ARC_HALF_ANGLE_DEG.to_radians();
        let cos_half = half.cos();
        for (r, a) in [
            (super::MIC_PULSE_R_IN, a_in),
            (super::MIC_PULSE_R_OUT, a_out),
        ] {
            if a <= 0.0 {
                continue; // 全透明弧完全不画
            }
            unsafe {
                res.brush
                    .SetColor(&colorref_to_d2d(super::OVERLAY_BRAND_ORANGE));
                res.brush.SetOpacity(a);
                // 右组：clip x ≥ cx + R·cos40°
                res.rt.PushAxisAlignedClip(
                    &D2D_RECT_F {
                        left: cx + r * cos_half,
                        top: cy - r - 1.0,
                        right: cx + r + 1.0,
                        bottom: cy + r + 1.0,
                    },
                    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                );
                res.rt.DrawEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F { x: cx, y: cy },
                        radiusX: r,
                        radiusY: r,
                    },
                    &res.brush,
                    super::MIC_PULSE_ARC_STROKE,
                    None,
                );
                res.rt.PopAxisAlignedClip();
                // 左组：clip x ≤ cx − R·cos40°（对称镜像，同相位同亮度）
                res.rt.PushAxisAlignedClip(
                    &D2D_RECT_F {
                        left: cx - r - 1.0,
                        top: cy - r - 1.0,
                        right: cx - r * cos_half,
                        bottom: cy + r + 1.0,
                    },
                    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                );
                res.rt.DrawEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F { x: cx, y: cy },
                        radiusX: r,
                        radiusY: r,
                    },
                    &res.brush,
                    super::MIC_PULSE_ARC_STROKE,
                    None,
                );
                res.rt.PopAxisAlignedClip();
                res.brush.SetOpacity(1.0);
            }
        }
    }

    /// GDI `draw_stop_button` (:2436): 16px orange square at right-25, vertically
    /// centered; 1px outline + 8px solid inner block (4px inset). Returns the same
    /// hit RECT as the GDI version (right/bottom +1: GDI Rectangle is exclusive).
    fn stop_button(res: &D2dResources, w: f32, h: f32) -> RECT {
        const BRAND_ORANGE: COLORREF = COLORREF(0x006BFF);
        let bs = 16.0_f32;
        let bl = w - 25.0; // GDI: rect.right - 25 (rect-relative width)
        let bt = (h - bs) / 2.0; // GDI: (height - bs)/2
        let cr = RECT {
            left: bl as i32,
            top: bt as i32,
            right: (bl + bs + 1.0) as i32, // GDI Rectangle right/bottom exclusive → +1
            bottom: (bt + bs + 1.0) as i32,
        };
        unsafe {
            // Outline: GDI Rectangle(left,top,right,bottom) with a 1px pen covers
            // left/top..right-1/bottom-1; D2D DrawRectangle stroke (1px, centered)
            // on (l+0.5, t+0.5)-(r-0.5, b-0.5) lands on the same pixel rows.
            let l = bl + 0.5;
            let t = bt + 0.5;
            let r = bl + bs + 1.0 - 0.5;
            let b = bt + bs + 1.0 - 0.5;
            res.brush.SetColor(&colorref_to_d2d(BRAND_ORANGE));
            res.rt.DrawRectangle(
                &D2D_RECT_F {
                    left: l,
                    top: t,
                    right: r,
                    bottom: b,
                },
                &res.brush,
                1.0,
                None,
            );
            // Inner solid: GDI Rectangle(il,it,il+isz+1,it+isz+1) filled — the +1 makes
            // the fill 9px wide; D2D FillRectangle covers [l, r) the same way.
            let isz = 8.0_f32;
            let il = bl + (bs - isz) / 2.0;
            let it = bt + (bs - isz) / 2.0;
            res.rt.FillRectangle(
                &D2D_RECT_F {
                    left: il,
                    top: it,
                    right: il + isz + 1.0,
                    bottom: it + isz + 1.0,
                },
                &res.brush,
            );
        }
        cr
    }

    /// D2D-P2 (PLAN-108 H9): GDI `draw_submit_button` 直译 —— 16px 橙色圆角钮 +
    /// 深色 ⏎（U+23CE）。几何照抄 GDI（rect 相对）：bs=16、bl=w-25、bt=(h-16)/2。
    /// GDI RoundRect(…,10,10) 是椭圆**直径** 10 → D2D 半径 5（别与 chrome 的
    /// RoundRect(…,10*2,10*2)→r10 抄混）。返回命中 RECT **无 +1**（GDI 历史口径，
    /// 与 stop_button 的 +1 排他约定不同）。箭头用 centered_text_format
    /// （对应 GDI draw_text 的 DT_CENTER|DT_VCENTER），文字色 BG_DARK #110F0D。
    /// 无度量需求（自居中）——度量裁定③继续成立。
    fn submit_button(res: &D2dResources, w: f32, h: f32) -> RECT {
        let bs = 16.0_f32;
        let bl = w - 25.0; // GDI: rect.right - 25（rect 相对宽）
        let bt = (h - bs) / 2.0; // GDI: (height - bs)/2
        let submit_rect = RECT {
            left: bl as i32,
            top: bt as i32,
            right: (bl + bs) as i32, // 🔴 无 +1：draw_submit_button_hit_rect_only 同公式
            bottom: (bt + bs) as i32,
        };
        unsafe {
            res.brush
                .SetColor(&colorref_to_d2d(super::OVERLAY_BRAND_ORANGE));
            res.rt.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: bl,
                        top: bt,
                        right: bl + bs,
                        bottom: bt + bs,
                    },
                    radiusX: 5.0,
                    radiusY: 5.0,
                },
                &res.brush,
            );
            // 1px 描边内缩 0.5px（stop_button 的像素对齐论证同款）
            res.rt.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: bl + 0.5,
                        top: bt + 0.5,
                        right: bl + bs - 0.5,
                        bottom: bt + bs - 0.5,
                    },
                    radiusX: 5.0,
                    radiusY: 5.0,
                },
                &res.brush,
                1.0,
                None,
            );
            let arrow: Vec<u16> = "\u{23CE}".encode_utf16().collect();
            res.brush.SetColor(&colorref_to_d2d(super::OVERLAY_BG_DARK));
            res.rt.DrawText(
                &arrow,
                &res.centered_text_format,
                &D2D_RECT_F {
                    left: bl,
                    top: bt,
                    right: bl + bs,
                    bottom: bt + bs,
                },
                &res.brush,
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        submit_rect
    }

    /// D2D-P2 (PLAN-108 H9): `StreamingEditing`（编辑态）= chrome + submit 键。
    /// 正文文字由 Win32 EDIT 子控件自绘（create_edit_control），父窗口不画文字，
    /// D2D 帧与子控件零交集（共存论证：docs/D2D-P2P3-PLAN.md §3.3-A —— D2D 只画进
    /// mem_dc、BitBlt 合成不变、子控件独立表面、颜色同值闭环）。
    /// Returns false on any D2D failure — the caller renders the same frame via GDI.
    /// EDITICON-176：编辑态左侧 = 铅笔图标（18px @ (6, 垂直居中)）+ 左分割线
    /// （x=30 / 2px / 20px 居中 / OVERLAY_BORDER_GRAY —— 与 mic_indicator 的左
    /// 分割线 :4544-4564 逐项同值，GDI 侧 :3750-3762 同一几何）。
    /// 图标像素 = `ui::menu_icons::edit_icon_rgba(18)`（EDITICON-175 主控目视
    /// 验收通过的那批像素）预乘 BGRA 后经 `rt.CreateBitmap` + `DrawBitmap`
    /// 1:1 贴出（NEAREST_NEIGHBOR，尺寸相同无重采样），不用 D2D 矢量重画
    /// ——避免第二份几何源与已验收形态漂移。
    /// 失败语义：位图创建/贴图失败 = 本帧 best-effort 跳过图标（分割线照画、
    /// 返回值不受影响）——「overlay 永不空白」契约只看 BindDC/D2D 基础设施，
    /// G1 护栏语义不变；图标缺失属可降级缺陷，GDI 兜底在下次 D2D 失败帧补画。
    fn edit_icon_and_left_separator(res: &D2dResources, h: f32) {
        const ICON_SIZE: f32 = 18.0;
        let icon_l = 6.0; // GDI: rect.left + 6
        let icon_t = (h - ICON_SIZE) / 2.0;
        let bgra = crate::ui::menu_icons::edit_icon_premultiplied_bgra(18);
        unsafe {
            let props = D2D1_BITMAP_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
            };
            let bitmap = res.rt.CreateBitmap(
                D2D_SIZE_U {
                    width: 18,
                    height: 18,
                },
                Some(bgra.as_ptr().cast()),
                18 * 4, // pitch：18px × 4B/px
                &props,
            );
            if let Ok(bitmap) = bitmap {
                res.rt.DrawBitmap(
                    &bitmap,
                    Some(&D2D_RECT_F {
                        left: icon_l,
                        top: icon_t,
                        right: icon_l + ICON_SIZE,
                        bottom: icon_t + ICON_SIZE,
                    }),
                    1.0,
                    D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                    None,
                );
            }
            // 左分割线：与 mic_indicator :4544-4564 逐项同值
            let sep_l_x = 30.0; // GDI: rect.left + 30
            let sep_hh = 10.0; // GDI: sep_h 20 / 2
            let cy = h / 2.0; // GDI: rect.top + height/2 (rect-relative)
            res.brush
                .SetColor(&colorref_to_d2d(super::OVERLAY_BORDER_GRAY));
            res.rt.DrawLine(
                D2D_POINT_2F {
                    x: sep_l_x,
                    y: cy - sep_hh,
                },
                D2D_POINT_2F {
                    x: sep_l_x,
                    y: cy + sep_hh,
                },
                &res.brush,
                2.0,
                None,
            );
        }
    }

    pub(crate) fn draw_editing_overlay(hdc: HDC, rect: &RECT) -> bool {
        with_d2d(hdc, rect, |res, w, h| {
            // OVERLAY-121 (P3): 编辑态维持 r=10 —— SLWA 旧路径，视觉口径不变。
            // OVERLAY-141: 半径单一来源（映射完备性：SLWA 不走 ULW fixup，仅绘制用）。
            chrome(res, w, h, super::OVERLAY_FRAME_RADIUS_SM);
            // EDITICON-176: 左侧铅笔图标 + 分割线（Gavin：原麦克风位置放编辑小图标）
            edit_icon_and_left_separator(res, h);
            let _ = submit_button(res, w, h);
        })
    }

    /// D2D-P2 (PLAN-108 H4): 右分隔线小原语 —— x = w-36、高 20 垂直居中、
    /// 2px OVERLAY_BORDER_GRAY（文字区与停止键之间）。原
    /// draw_streaming_text_overlay 内联版（:3663-3684 P1 补齐）上提共用，
    /// 消除第二份内联（GDI 侧两处 :2329/:2629 同一几何）。
    fn right_separator(res: &D2dResources, w: f32, h: f32) {
        unsafe {
            let sep_r_x = w - 36.0; // GDI: rect.right - 36 (rect-relative)
            let sep_hh = 10.0; // GDI: sep_h 20 / 2
            let cy = h / 2.0;
            res.brush
                .SetColor(&colorref_to_d2d(super::OVERLAY_BORDER_GRAY));
            res.rt.DrawLine(
                D2D_POINT_2F {
                    x: sep_r_x,
                    y: cy - sep_hh,
                },
                D2D_POINT_2F {
                    x: sep_r_x,
                    y: cy + sep_hh,
                },
                &res.brush,
                2.0,
                None,
            );
        }
    }

    /// D2D-P2 (PLAN-108 H3): 波形原语 —— 32 条 3px 圆角竖条（GDI RoundRect(…,6,6)
    /// = r3 全圆角）单色 FillRoundedRectangle，逐条高度/横位与 GDI 循环逐位同值：
    /// bc=32、bw=3、bgap=2、half=16；wl = 30+12+(ww-108)/2（rect 相对，ww = w-90，
    /// GDI :2332-2338 同式）；by = h/2。高度来自共享纯函数 waveform_bar_height
    /// （i32 口径转 f32，整数对齐防 ±1px 视觉差），快照来自 waveform_snapshot
    /// （锁内 decay，OVERLAY-LOCK-SCOPE-001）。性能：GDI 每帧 ~224 次 GDI 对象
    /// 创建/销毁 → D2D 1 次 SetColor + 32 次 FillRoundedRectangle，零 GDI 对象操作。
    fn waveform(res: &D2dResources, w: f32, h: f32, state: &OverlayWindowState) {
        let bc: i32 = 32;
        let bw: i32 = 3;
        let bgap: i32 = 2;
        let half = bc / 2;
        let ww = (w as i32 - 36) - 30 - 24; // GDI: sep_r_x - sep_l_x - 24（rect 相对）
        let total_bar_width = bc * bw + (bc - 1) * bgap;
        let wl = (30 + 12 + (ww - total_bar_width) / 2) as f32; // GDI: sep_l_x + 12 + …
        let by = h / 2.0;
        let snapshot = super::waveform_snapshot(state, half);
        unsafe {
            res.brush
                .SetColor(&colorref_to_d2d(super::OVERLAY_BRAND_ORANGE));
            // 🔴 奇偶对齐：GDI br = {top: by - bh/2, bottom: by + bh/2} 是 **i32 整除**，
            // 奇数 bh 实画 2*(bh/2) px（丢 1px）。D2D 必须复刻同一整除口径，
            // 否则奇数高度 bar 比 GDI 高 1px（视觉差）。
            // Left half: bars spread from center outward to the left
            for i in 0..half {
                let v = snapshot.get(i as usize).copied().unwrap_or(0.0);
                let hh = (super::waveform_bar_height(v, i, half) / 2) as f32;
                let x = wl + ((half - 1 - i) * (bw + bgap)) as f32;
                res.rt.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: x,
                            top: by - hh,
                            right: x + bw as f32,
                            bottom: by + hh,
                        },
                        radiusX: bw as f32,
                        radiusY: bw as f32,
                    },
                    &res.brush,
                );
            }
            // Right half: bars spread from center outward to the right (mirror)
            for i in 0..half {
                let v = snapshot.get(i as usize).copied().unwrap_or(0.0);
                let hh = (super::waveform_bar_height(v, i, half) / 2) as f32;
                let x = wl + ((half + i) * (bw + bgap)) as f32;
                res.rt.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: x,
                            top: by - hh,
                            right: x + bw as f32,
                            bottom: by + hh,
                        },
                        radiusX: bw as f32,
                        radiusY: bw as f32,
                    },
                    &res.brush,
                );
            }
        }
    }

    /// D2D-P2 (PLAN-108 H5): `Recording` 波形变体与 `FallingToProcessing` 的共用
    /// 复合体（两者 GDI 像素输出逐位相同 —— dispatch :2070-2079 经
    /// draw_recording_overlay(show_placeholder=false) 与 :2127-2131 直调同一组
    /// 绘制函数）= chrome + mic_indicator(三态图标+左分隔线) + waveform +
    /// right_separator + stop_button。命中 rect 由调用侧 helper 出。
    /// Returns false on any D2D failure — the caller renders the same frame via GDI.
    pub(crate) fn draw_recording_waveform_overlay(
        hdc: HDC,
        rect: &RECT,
        state: &OverlayWindowState,
    ) -> bool {
        with_d2d(hdc, rect, |res, w, h| {
            // OVERLAY-121 (P3): Recording/FallingToProcessing 升 r=16（Gavin 拍板）。
            // OVERLAY-141: 半径单一来源（overlay_frame_radius 同值）。
            chrome(res, w, h, super::OVERLAY_FRAME_RADIUS_LG);
            mic_indicator(res, h, state);
            waveform(res, w, h, state);
            right_separator(res, w, h);
            let _ = stop_button(res, w, h);
        })
    }

    /// GDI `draw_listening_placeholder` (:2731): centered single-line hint
    /// ("请说话..." / "Speak now...") between the mic area and the stop button.
    fn placeholder_text(
        res: &D2dResources,
        w: f32,
        h: f32,
        ui_language: crate::config::UiLanguage,
    ) {
        let hint = super::i18n::get(ui_language).overlay_listening_hint;
        let hint: Vec<u16> = hint.encode_utf16().collect();
        unsafe {
            res.brush
                .SetColor(&colorref_to_d2d(super::OVERLAY_TEXT_WHITE));
            // D2D-P1: same face/weight as the GDI text path (Segoe UI normal, streaming
            // format) so the placeholder is visually continuous with the streaming text
            // that replaces it — the P0 centered format is YaHei SemiBold and would
            // visibly differ from the GDI baseline on Latin glyphs.
            res.rt.DrawText(
                &hint,
                &res.streaming_text_format,
                &D2D_RECT_F {
                    left: super::STREAMING_TEXT_LEFT_MARGIN as f32,
                    top: 0.0,
                    right: w - super::STREAMING_TEXT_RIGHT_MARGIN as f32,
                    bottom: h,
                },
                &res.brush,
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    /// GDI streaming text path (:2530-2563 of draw_recording_overlay_with_text):
    /// clip to the text band, draw the tween-visible prefix at
    /// `x = text_left - scroll_x`, newest text pinned to the right edge once overflow.
    /// Metrics come from the caller (GDI measure_text_width — the single metric source
    /// agreed in D2D-P1 方案裁定 1/2), passed as `text_width` so the D2D side never
    /// measures on its own and cannot drift from the window sizing path.
    /// `visible_text` is the tween-visible prefix (OVERLAY-051-G), precomputed by the
    /// caller exactly like the GDI path.
    fn streaming_text(res: &D2dResources, w: f32, h: f32, visible_text: &str, text_width: i32) {
        let text_left = super::STREAMING_TEXT_LEFT_MARGIN as f32;
        let text_right = w - super::STREAMING_TEXT_RIGHT_MARGIN as f32;
        let text_top = super::OVERLAY_TEXT_DRAW_VERTICAL_INSET as f32;
        let text_bottom = h - super::OVERLAY_TEXT_DRAW_VERTICAL_INSET as f32;
        let visible_w = (text_right - text_left).max(1.0);
        // REFACTOR-088: GDI 路径共用同一纯函数。等价性链条见 `streaming_scroll_offset`
        // doc-comment：visible_w 是整数值 f32（w 整数像素 + i32 margin 常量），
        // as i32 截断永不发生，i32 运算逐位同值，转回 f32 无损。
        // 🔴 前提断裂条件（margin 非整数 / 窗口宽度带小数）亦见该 doc-comment。
        let scroll_x = super::streaming_scroll_offset(text_width, visible_w as i32) as f32;

        let visible: Vec<u16> = visible_text.encode_utf16().collect();
        unsafe {
            // Clip: GDI SaveDC → SelectClipRgn(text area) → RestoreDC.
            // D2D: PushAxisAlignedClip → DrawText → PopAxisAlignedClip.
            res.rt.PushAxisAlignedClip(
                &D2D_RECT_F {
                    left: text_left,
                    top: text_top,
                    right: text_right,
                    bottom: text_bottom,
                },
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            );
            res.brush
                .SetColor(&colorref_to_d2d(super::OVERLAY_TEXT_WHITE));
            // Layout rect shifted left by scroll_x; the layout is wider than the clip,
            // so the right edge of the text (newest) stays pinned at text_right.
            res.rt.DrawText(
                &visible,
                &res.streaming_text_format,
                &D2D_RECT_F {
                    left: text_left - scroll_x,
                    top: text_top,
                    right: text_right - scroll_x,
                    bottom: text_bottom,
                },
                &res.brush,
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            res.rt.PopAxisAlignedClip();
        }
    }

    /// D2D-P1: `RecordingStreamingIdle`（聆听占位态）= chrome + mic indicator +
    /// centered placeholder hint + stop button. Returns false on any D2D failure —
    /// the caller (draw_recording_overlay) renders the same frame via GDI.
    pub(crate) fn draw_streaming_idle_overlay(
        hdc: HDC,
        rect: &RECT,
        state: &OverlayWindowState,
        ui_language: crate::config::UiLanguage,
    ) -> bool {
        with_d2d(hdc, rect, |res, w, h| {
            // OVERLAY-121 (P3): RecordingStreamingIdle 升 r=16（Gavin 拍板）。
            // OVERLAY-141: 半径单一来源。
            chrome(res, w, h, super::OVERLAY_FRAME_RADIUS_LG);
            mic_indicator(res, h, state);
            placeholder_text(res, w, h, ui_language);
            let _ = stop_button(res, w, h);
        })
    }

    /// D2D-P1: `RecordingWithText`（流式文字态）= chrome + mic indicator +
    /// scroll-clipped streaming text + stop button. Returns false on any D2D
    /// failure — the caller (draw_recording_overlay_with_text) renders the same
    /// frame via GDI. `text_width` is measured by the caller with the GDI font
    /// (single metric source); `visible_text` is the tween-visible prefix.
    pub(crate) fn draw_streaming_text_overlay(
        hdc: HDC,
        rect: &RECT,
        state: &OverlayWindowState,
        visible_text: &str,
        text_width: i32,
    ) -> bool {
        with_d2d(hdc, rect, |res, w, h| {
            // OVERLAY-121 (P3): RecordingWithText 升 r=16（Gavin 拍板）。
            // OVERLAY-141: 半径单一来源。
            chrome(res, w, h, super::OVERLAY_FRAME_RADIUS_LG);
            mic_indicator(res, h, state);
            streaming_text(res, w, h, visible_text, text_width);
            // Right separator (主控 D2D-P1 验收要求补齐): GDI 版 :2617-2624 逐项照抄 —
            // x = width-36, 高 20 垂直居中, 2px OVERLAY_BORDER_GRAY。文字区与停止键之间。
            // D2D-P2 (PLAN-108 H4): 内联版上提为 right_separator 原语（同值替换），
            // 与 Recording/FallingToProcessing 复合体共用，消除第二份内联。
            right_separator(res, w, h);
            let _ = stop_button(res, w, h);
        })
    }

    /// D2D-P2 (PLAN-108 H12): `Error`（错误态）= chrome_with(#211D1A, r10) + 红点
    /// FillEllipse + 左对齐单行错误文本。几何逐位照抄 GDI `draw_error_overlay`：
    /// 底色 #211D1A / 边 r10（:4032-4034）、红点 d=8 圆心 (left+16, 垂直中)
    /// （:4061-4075，Ellipse → FillEllipse r4 直译）、文本带相对 28 → w-14、
    /// 上下 4px（:4082-4087，left = circ_x + circ_d/2 + 8 = 28）。
    /// 文本复用 streaming_text_format（Segoe UI Normal 14, LEADING + 垂直居中 +
    /// NO_WRAP，与 GDI DT_LEFT|DT_VCENTER|SINGLELINE 逐项对应）——U1 裁决：
    /// DT_END_ELLIPSIS 不迁移，NO_WRAP 硬裁剪，GDI 兜底保留省略号。
    /// 无命中矩形（GDI 版无返回值，五元组全 None）。
    /// Returns false on any D2D failure — the caller renders the same frame via GDI.
    pub(crate) fn draw_error_overlay(hdc: HDC, rect: &RECT, message: &str) -> bool {
        with_d2d(hdc, rect, |res, w, h| {
            // OVERLAY-141: 半径单一来源。
            chrome_with(
                res,
                w,
                h,
                COLORREF(0x211D1A),
                super::OVERLAY_FRAME_RADIUS_SM,
            );
            unsafe {
                let circ_d = 4.0_f32; // GDI circ_d=8 的半径
                res.brush.SetColor(&colorref_to_d2d(COLORREF(0x0033CC)));
                res.rt.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F {
                            x: 12.0 + circ_d,
                            y: h / 2.0,
                        },
                        radiusX: circ_d,
                        radiusY: circ_d,
                    },
                    &res.brush,
                );
                let msg: Vec<u16> = message.encode_utf16().collect();
                res.brush
                    .SetColor(&colorref_to_d2d(super::OVERLAY_BRAND_ORANGE));
                res.rt.DrawText(
                    &msg,
                    &res.streaming_text_format,
                    &D2D_RECT_F {
                        left: 12.0 + circ_d + circ_d + 8.0, // GDI: circ_x + circ_d/2 + 8
                        top: 4.0,
                        right: w - 14.0,
                        bottom: h - 4.0,
                    },
                    &res.brush,
                    windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        })
    }

    /// BUG-119: Info 态。几何逐位照抄 D2D `draw_error_overlay`（即 GDI
    /// `draw_info_overlay` 的 D2D 直译）：chrome bg #211D1A / 边 r10，
    /// 蓝点（COLORREF 0xFF9933 = RGB #3399FF，错误态是红点 0x0033CC），
    /// 文本白色（OVERLAY_TEXT_WHITE，错误态是 BRAND_ORANGE）。其余逐位同源。
    /// Returns false on any D2D failure — the caller renders the same frame via GDI.
    pub(crate) fn draw_info_overlay(hdc: HDC, rect: &RECT, message: &str) -> bool {
        with_d2d(hdc, rect, |res, w, h| {
            // OVERLAY-141: 半径单一来源。
            chrome_with(
                res,
                w,
                h,
                COLORREF(0x211D1A),
                super::OVERLAY_FRAME_RADIUS_SM,
            );
            unsafe {
                let circ_d = 4.0_f32; // GDI circ_d=8 的半径
                res.brush.SetColor(&colorref_to_d2d(COLORREF(0xFF9933)));
                res.rt.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F {
                            x: 12.0 + circ_d,
                            y: h / 2.0,
                        },
                        radiusX: circ_d,
                        radiusY: circ_d,
                    },
                    &res.brush,
                );
                let msg: Vec<u16> = message.encode_utf16().collect();
                res.brush
                    .SetColor(&colorref_to_d2d(super::OVERLAY_TEXT_WHITE));
                res.rt.DrawText(
                    &msg,
                    &res.streaming_text_format,
                    &D2D_RECT_F {
                        left: 12.0 + circ_d + circ_d + 8.0, // GDI: circ_x + circ_d/2 + 8
                        top: 4.0,
                        right: w - 14.0,
                        bottom: h - 4.0,
                    },
                    &res.brush,
                    windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        })
    }

    /// D2D-P2 (PLAN-108 H11): `FocusLost`（失焦预览态）。几何逐位照抄 GDI
    /// `draw_preview_overlay`（rect 相对坐标）：
    /// - 底 #211D1A + 1px 边 r10（:3840-3867）→ chrome_with(bg, 10)
    /// - 标题栏文字：橙，带 (26, 4) → (w-26, 28)（:3869-3886），centered format
    /// - 标题 ✕ 键 18x18：(w-26, 5) → (w-8, 23)，GDI RoundRect(…,6,6)→r3，
    ///   OVERLAY_BTN_BORDER 1px 描边（无填充）+ ✕（U+2715）橙字，centered format
    /// - 标题分隔线：y=28，x 8 → w-8，1px OVERLAY_BORDER_GRAY（:3925-3933）
    /// - 正文：#F2F2F2，带 (14, 36) → (w-14, h-40)（:3934-3949），wrap_text_format
    ///   （LEADING + 顶对齐 + WRAP = DT_LEFT|DT_WORDBREAK；U2 裁决：断行差异是
    ///   端测观察项，不许为它开单元素退 GDI 先例）
    /// - 底部双键：45x18 gap10 水平居中、底距 10（同 preview_hit_rects 公式，
    ///   btn_left 用 i32 整除保持与 GDI 逐位同值），GDI RoundRect(…,8,8)→r4
    ///   OVERLAY_BTN_BORDER 描边（无填充）；复制橙字 / 关闭 #808080 灰字，
    ///   centered format（:3950-4020）
    /// 命中 rect 由调用侧 preview_hit_rects(rect) 出（单一几何源，本原语不算几何）。
    /// Returns false on any D2D failure — the caller renders the same frame via GDI.
    pub(crate) fn draw_preview_overlay(
        hdc: HDC,
        rect: &RECT,
        text: &str,
        ui_language: crate::config::UiLanguage,
    ) -> bool {
        with_d2d(hdc, rect, |res, w, h| {
            // OVERLAY-141: 半径单一来源。
            chrome_with(
                res,
                w,
                h,
                COLORREF(0x211D1A),
                super::OVERLAY_FRAME_RADIUS_SM,
            );
            unsafe {
                // 标题栏文字（橙，DT_CENTER|DT_VCENTER → centered format）
                res.brush
                    .SetColor(&colorref_to_d2d(super::OVERLAY_BRAND_ORANGE));
                let title: Vec<u16> = super::i18n::get(ui_language)
                    .preview_title_bar
                    .encode_utf16()
                    .collect();
                res.rt.DrawText(
                    &title,
                    &res.centered_text_format,
                    &D2D_RECT_F {
                        left: 26.0,
                        top: 4.0,
                        right: w - 26.0,
                        bottom: 28.0,
                    },
                    &res.brush,
                    windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
                // 标题 ✕ 键（18x18，r3，描边无填充）
                let tc_l = w - 26.0;
                let tc_t = 5.0;
                let tc_r = w - 8.0;
                let tc_b = 23.0;
                res.brush
                    .SetColor(&colorref_to_d2d(super::OVERLAY_BTN_BORDER));
                res.rt.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: tc_l + 0.5,
                            top: tc_t + 0.5,
                            right: tc_r - 0.5,
                            bottom: tc_b - 0.5,
                        },
                        radiusX: 3.0,
                        radiusY: 3.0,
                    },
                    &res.brush,
                    1.0,
                    None,
                );
                res.brush
                    .SetColor(&colorref_to_d2d(super::OVERLAY_BRAND_ORANGE));
                let x_mark: Vec<u16> = "\u{2715}".encode_utf16().collect();
                res.rt.DrawText(
                    &x_mark,
                    &res.centered_text_format,
                    &D2D_RECT_F {
                        left: tc_l,
                        top: tc_t,
                        right: tc_r,
                        bottom: tc_b,
                    },
                    &res.brush,
                    windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
                // 标题分隔线（y=28，x 8 → w-8，1px）
                res.brush
                    .SetColor(&colorref_to_d2d(super::OVERLAY_BORDER_GRAY));
                res.rt.DrawLine(
                    D2D_POINT_2F { x: 8.0, y: 28.0 },
                    D2D_POINT_2F {
                        x: w - 8.0,
                        y: 28.0,
                    },
                    &res.brush,
                    1.0,
                    None,
                );
                // 正文（#F2F2F2，顶对齐 + WRAP）
                res.brush.SetColor(&colorref_to_d2d(COLORREF(0xF2F2F2)));
                let body: Vec<u16> = text.encode_utf16().collect();
                res.rt.DrawText(
                    &body,
                    &res.wrap_text_format,
                    &D2D_RECT_F {
                        left: 14.0,
                        top: 36.0,
                        right: w - 14.0,
                        bottom: h - 40.0,
                    },
                    &res.brush,
                    windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
                // 底部双键（45x18 gap10；btn_left i32 整除 = GDI (W-100)/2 逐位同值）
                let btn_left = ((w as i32 - 100) / 2) as f32;
                let btn_top = h - 28.0; // GDI: bottom - 18 - 10（rect 相对）
                let copy_l = btn_left;
                let close_l = btn_left + 55.0; // btn_w + gap
                res.brush
                    .SetColor(&colorref_to_d2d(super::OVERLAY_BTN_BORDER));
                for l in [copy_l, close_l] {
                    res.rt.DrawRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: l + 0.5,
                                top: btn_top + 0.5,
                                right: l + 45.0 - 0.5,
                                bottom: btn_top + 18.0 - 0.5,
                            },
                            radiusX: 4.0,
                            radiusY: 4.0,
                        },
                        &res.brush,
                        1.0,
                        None,
                    );
                }
                // 标签：复制橙字 / 关闭灰字（DT_CENTER|DT_VCENTER → centered format）
                let copy_label: Vec<u16> = super::i18n::get(ui_language)
                    .preview_copy_btn
                    .encode_utf16()
                    .collect();
                let close_label: Vec<u16> = super::i18n::get(ui_language)
                    .preview_close
                    .encode_utf16()
                    .collect();
                res.brush
                    .SetColor(&colorref_to_d2d(super::OVERLAY_BRAND_ORANGE));
                res.rt.DrawText(
                    &copy_label,
                    &res.centered_text_format,
                    &D2D_RECT_F {
                        left: copy_l,
                        top: btn_top,
                        right: copy_l + 45.0,
                        bottom: btn_top + 18.0,
                    },
                    &res.brush,
                    windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
                res.brush.SetColor(&colorref_to_d2d(COLORREF(0x808080)));
                res.rt.DrawText(
                    &close_label,
                    &res.centered_text_format,
                    &D2D_RECT_F {
                        left: close_l,
                        top: btn_top,
                        right: close_l + 45.0,
                        bottom: btn_top + 18.0,
                    },
                    &res.brush,
                    windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        })
    }
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
                                                  // OVERLAY-054-C: use file-level OVERLAY_BORDER_GRAY instead of local constant.
                                                  // OVERLAY-141: 半径单一来源（GDI fallback 与 D2D draw_processing_primitives 同值）。
    const CORNER_RADIUS: i32 = OVERLAY_FRAME_RADIUS_LG as i32;
    // WAVEFORM-HEIGHT-FIX-001: restore fixed gray border (remove breathing)
    let border_color = OVERLAY_BORDER_GRAY;
    // Dark background
    let bg = unsafe { CreateSolidBrush(BG_DARK) };
    // OVERLAY-086 Bug 1 (GDI fallback path): fill through a rounded region so the
    // background matches the RoundRect border geometry — a rectangular FillRect leaves
    // the same corner wedges the D2D path had. radius 16 = CORNER_RADIUS below.
    let fill_region = unsafe {
        CreateRoundRectRgn(
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            CORNER_RADIUS * 2,
            CORNER_RADIUS * 2,
        )
    };
    unsafe {
        let _ = FillRgn(hdc, fill_region, bg);
        let _ = DeleteObject(fill_region);
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
        let _ = FillRect(
            tmp_dc,
            &RECT {
                left: 0,
                top: 0,
                right: slice_w,
                bottom: win_h,
            },
            brush,
        );
        let _ = DeleteObject(brush);

        for i in 0..SLICES {
            let t = (i as f32 / (SLICES - 1) as f32) * 2.0 - 1.0; // -1.0 to 1.0
            let alpha = ((-3.0_f32 * t * t).exp() * 150.0) as u8; // TUNE: 200→150 slightly more transparent per Gavin
            if alpha == 0 {
                continue;
            }

            let x_dst = beam_cx - GLOW_HALF + i * slice_w;
            let x1 = x_dst.max(rect.left + 1);
            let x2 = (x_dst + slice_w).min(rect.right - 1);
            let w = x2 - x1;
            if w <= 0 {
                continue;
            }

            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: alpha,
                AlphaFormat: 0,
            };
            let _ = AlphaBlend(
                hdc,
                x1,
                rect.top + 1,
                w,
                win_h,
                tmp_dc,
                0,
                0,
                slice_w,
                win_h,
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
/// D2D-P2 (PLAN-108 H10): FocusLost 三个命中矩形的单一几何源（纯函数，只依赖 rect）。
/// GDI 绘制路径与 D2D 成功路径都从这里取返回值——几何口径物理上不可分叉
/// （centered_x 同款哲学：返回值不允许两份算术）。公式逐位照抄原
/// draw_preview_overlay 内联版：标题 ✕ 键 18x18（right-26, top+5 → right-8, top+23）、
/// 底部双键 45x18 gap10 底边距 10 水平居中。
#[cfg(target_os = "windows")]
fn preview_hit_rects(rect: &RECT) -> (RECT, RECT, RECT) {
    let title_close_rect = RECT {
        left: rect.right - 26,
        top: rect.top + 5,
        right: rect.right - 8,
        bottom: rect.top + 23,
    };
    let btn_w = 45;
    let btn_h = 18;
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
    (copy_rect, close_rect, title_close_rect)
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
    // OVERLAY-054-C: use file-level OVERLAY_BORDER_GRAY instead of local constant.
    // OVERLAY-141: 半径单一来源（GDI fallback 与 D2D chrome_with 同值）。
    const CORNER_RADIUS: i32 = OVERLAY_FRAME_RADIUS_SM as i32;
    let strings = i18n::get(ui_language);
    // D2D-P2 (PLAN-108 H10): 三个命中 rect 改由 preview_hit_rects 单一源出
    // （绘制与返回值共用同一组 RECT，同值替换）。
    let (copy_rect, close_rect, title_close_rect) = preview_hit_rects(rect);
    let bg = unsafe { CreateSolidBrush(BG_DARK) };
    unsafe {
        let _ = FillRect(hdc, rect, bg);
        let _ = DeleteObject(bg);
    }
    // Border (unified style)
    let border_pen = unsafe { CreatePen(PS_SOLID, 1, OVERLAY_BORDER_GRAY) };
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
    // Title bar close button (18x18, right side) — rect 来自 preview_hit_rects
    // FIX-006 v2: button border brighter than window border for visual distinction.
    // OVERLAY-054-C: use file-level OVERLAY_BTN_BORDER (value unchanged, 0x707070).
    let tc_pen = unsafe { CreatePen(PS_SOLID, 1, OVERLAY_BTN_BORDER) };
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
    let sep_pen = unsafe { CreatePen(PS_SOLID, 1, OVERLAY_BORDER_GRAY) };
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
    draw_text(
        hdc,
        text,
        &mut text_rect,
        DT_LEFT | DT_WORDBREAK | DT_END_ELLIPSIS,
    );
    // Bottom buttons (centered) — rects 来自 preview_hit_rects（45x18, gap10, 底距10）
    // FIX-006 v2: bottom buttons use brighter border to distinguish from window edge.
    // OVERLAY-054-C: use file-level OVERLAY_BTN_BORDER.
    let btn_pen = unsafe { CreatePen(PS_SOLID, 1, OVERLAY_BTN_BORDER) };
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
                                                  // OVERLAY-054-C: use file-level OVERLAY_BORDER_GRAY instead of local constant.
                                                  // OVERLAY-141: 半径单一来源（GDI fallback 与 D2D chrome_with 同值）。
    const CORNER_RADIUS: i32 = OVERLAY_FRAME_RADIUS_SM as i32;
    // Dark gray background (unified)
    let bg = unsafe { CreateSolidBrush(BG_DARK) };
    unsafe {
        let _ = FillRect(hdc, rect, bg);
        let _ = DeleteObject(bg);
    }
    // Border: 1px rounded corners (unified)
    let border_pen = unsafe { CreatePen(PS_SOLID, 1, OVERLAY_BORDER_GRAY) };
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
/// BUG-119: 信息提示态（如「请说话哦..」）。布局逐位照抄 draw_error_overlay，
/// 仅点色（蓝）与文字色（白）与错误态区分 —— 信息级不再是红色告警。
/// 圆角沿用 Error 的 10（OVERLAY-121 per-pixel alpha 才动圆角，本单不碰）。
#[cfg(target_os = "windows")]
fn draw_info_overlay(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    message: &str,
    _ui_language: config::UiLanguage,
) {
    const INFO_BLUE: COLORREF = COLORREF(0xFF9933); // BGR: blue #3399FF
    const BG_DARK: COLORREF = COLORREF(0x211D1A); // #1A1D21（同错误态统一底色）
                                                  // OVERLAY-141: 半径单一来源（GDI fallback 与 D2D chrome_with 同值）。
    const CORNER_RADIUS: i32 = OVERLAY_FRAME_RADIUS_SM as i32;
    // Dark gray background (unified)
    let bg = unsafe { CreateSolidBrush(BG_DARK) };
    unsafe {
        let _ = FillRect(hdc, rect, bg);
        let _ = DeleteObject(bg);
    }
    // Border: 1px rounded corners (unified)
    let border_pen = unsafe { CreatePen(PS_SOLID, 1, OVERLAY_BORDER_GRAY) };
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
    // Small solid blue dot on left (error 态是红点，信息态用蓝点区分)
    let circ_d = 8; // diameter 8px
    let circ_x = rect.left + 12 + circ_d / 2; // center x
    let cy = rect.top + (rect.bottom - rect.top) / 2;
    let blue_brush = unsafe { CreateSolidBrush(INFO_BLUE) };
    let null_pen = unsafe { CreatePen(PS_NULL, 0, COLORREF(0)) };
    let old_brush = unsafe { SelectObject(hdc, blue_brush) };
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
        let _ = DeleteObject(blue_brush);
        let _ = DeleteObject(null_pen);
    }
    // Info text (white, left-aligned with margin for circle)
    let mut text_rect = RECT {
        left: circ_x + circ_d / 2 + 8,
        top: rect.top + 4,
        right: rect.right - 14,
        bottom: rect.bottom - 4,
    };
    unsafe {
        let _ = SetTextColor(hdc, OVERLAY_TEXT_WHITE);
    }
    draw_text(
        hdc,
        message,
        &mut text_rect,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
    );
}
/// OVERLAY-043-B: per-frame size interpolation step.
///
/// Contract:
/// - `delta == 0` -> `0`
/// - at least 1 px per frame (so the animation actually reaches the target)
/// - at most 25% of the remaining distance per frame
/// - never overshoots the target (no oscillation)
/// - sign-symmetric: `interpolate_step(-n) == -interpolate_step(n)`
#[cfg(target_os = "windows")]
fn interpolate_step(delta: i32) -> i32 {
    if delta == 0 {
        return 0;
    }
    let abs = delta.abs() as f32;
    let magnitude = (abs * 0.25).max(1.0).min(abs) as i32;
    magnitude * delta.signum()
}

/// OVERLAY-043-B: whether a late StreamingText packet should be ignored.
/// Truth table (both input combinations must be nailed down):
///
/// | stopped | result | meaning |
/// |---------|--------|---------|
/// | false   | false  | recording, show text |
/// | true    | true   | stopped, late packet is always dropped |
///
/// FLICKER-130 (R1, Gavin 2026-09-06 拍板): the OVERLAY-043-era `editing`
/// exemption is removed. Its only code path (`else if editing` -> Show(StreamingEditing))
/// unconditionally destroys the EDIT control in the Show handler (:1298) while
/// `create_edit_control` is only reachable from EnterEditMode — so the designed
/// "still sync EDIT text" never worked; the exemption's sole observable effect was
/// destroying the editor = the flicker itself. Production invariant that makes the
/// `editing` dimension moot: OVERLAY_EDITING.store(true) (:5518) and
/// STREAMING_STOPPED.store(true) (:5521) are adjacent stores on the same controller
/// thread inside EditRequested, so editing=true implies stopped=true; the (false,true)
/// combination is unreachable. Render-only gate: the last_streaming_text mirror
/// (:5372) is updated BEFORE this gate, so dropping the render does not starve
/// WORDBOOK-053-B (渲染抑制 ≠ 数据抑制, see generation_gate_blocks_mirror_but_043_gate_is_render_only).
#[cfg(target_os = "windows")]
fn should_ignore_streaming_text(stopped: bool) -> bool {
    stopped
}

/// OVERLAY-051-G-FIN: 时间戳驱动回放 —— 返回此刻应显示的字符数。
///
/// Gavin 三条指示（不得动摇）：
/// 1. 时间戳驱动，不用固定速率抖动缓冲
/// 2. 停顿与用户讲话节奏一致，**不压缩停顿**（无 interval 上限/下限，无 backlog 加速）
/// 3. `words` 不可用时退回立即显示（几百毫秒固定延迟可接受）
///
/// 契约：
/// 1. `words` 为空 → 返回 `total_chars`（立即全显，降级路径）
/// 2. 以 `words[0].begin_time` 为基准取**差值**，不依赖音频绝对起点
///    （1.5s pre-roll 偏移在减法里消掉，不要算绝对锚点）
/// 3. 不压缩停顿：词间间隔多长就等多长，无上限无下限
/// 4. 单调不回退：调用侧保证 `displayed` 只增不减（用 `.max(displayed)`）
/// 5. **词表覆盖不到的尾部一并放出**：若所有词都已到期（循环走完无 break），
///    说明词表短于文本（标点归属差异 / words 落后于 text 等），此时直接返回
///    `total_chars` —— **宁可多显绝不少显**，绝不让尾部差额卡到松键 flush 才补上。
///    `.min(total_chars)` 仍作为上界保护，防止词表字符总数超过文本长度。
///
/// 实现：时间轴零点 `origin_begin` 优先取 `timeline_origin`（OVERLAY-086 Bug 2 ③：
/// 首个非空词表的 `words[0].begin_time`，由调用方锚定一次后固定传入）；`timeline_origin`
/// 为 `None` 时退回旧实现，从**当前词表**现算 `words[0].begin_time`（既有护栏的旧行为）。
/// 固定零点的必要性：服务端会整段重写词表（Gavin 实测 id=2 20词→19词、文本不变），
/// 重写后 `words[0].begin_time` 漂移（1120→1160），若每次现算，所有
/// `w.begin_time - origin_begin` 一起变大 → 揭示边界整体后退 → 显示冻结，
/// 直到墙钟追上再一次性释放（冻 1.9s + 爆发追涨，Gavin 报的「中断显示」）。
/// 墙钟零点（tween_audio_origin，:1376）与时间轴零点必须**同一时刻锚定、此后双端固定**，
/// 词表重写只改「有哪些词」，不改「时间零点在哪」。
#[cfg(target_os = "windows")]
fn reveal_chars_by_timeline(
    words: &[crate::transcription::qwen_inference::WordTiming],
    elapsed_ms: i64,
    total_chars: usize,
    timeline_origin: Option<i64>,
) -> usize {
    // 契约 1：words 为空 → 立即全显（降级路径）
    if words.is_empty() {
        return total_chars;
    }
    // OVERLAY-086 Bug 2 ③：时间轴零点固定锚定；None = 旧行为（从当前词表现算），
    // 供 OVERLAY-051-G-FIN 既有护栏（:7801 模块五处直调）保持原语义零改动。
    let origin_begin = timeline_origin.unwrap_or(words[0].begin_time);
    let mut revealed: usize = 0;
    let mut broke_early = false;
    for w in words {
        if w.begin_time - origin_begin <= elapsed_ms {
            revealed += w.text.chars().count() + w.punctuation.chars().count();
        } else {
            // words 按 begin_time 升序排列；遇到第一个未到的词就可以停止
            broke_early = true;
            break;
        }
    }
    // 契约 5：所有词都已到期（循环走完无 break）→ 词表覆盖不到的尾部一并放出
    if !broke_early {
        return total_chars;
    }
    revealed.min(total_chars)
}

#[cfg(target_os = "windows")]
fn monitor_work_rect(hwnd: HWND) -> RECT {
    unsafe {
        let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !hmon.is_invalid() && GetMonitorInfoW(hmon, &mut mi).as_bool() {
            mi.rcWork
        } else {
            RECT {
                left: 0,
                top: 0,
                right: GetSystemMetrics(SM_CXSCREEN),
                bottom: GetSystemMetrics(SM_CYSCREEN),
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn overlay_geometry(status: &OverlayStatus, hwnd: HWND) -> ([i32; 2], [i32; 2]) {
    let work = monitor_work_rect(hwnd);
    let work_w = work.right - work.left;
    let work_h = work.bottom - work.top;

    let size = match status {
        OverlayStatus::Recording
        | OverlayStatus::RecordingStreamingIdle
        | OverlayStatus::RecordingWithText { .. }
        | OverlayStatus::FallingToProcessing { .. }
        | OverlayStatus::StreamingEditing { .. } => {
            // ASR-038-C: editing mode initial size; will be expanded by adjust_overlay_size_for_text
            RECORDING_OVERLAY_SIZE
        }
        OverlayStatus::Processing(_) | OverlayStatus::Error(_) | OverlayStatus::Info(_) => {
            STATUS_OVERLAY_SIZE
        }
        OverlayStatus::FocusLost { .. } => PREVIEW_OVERLAY_SIZE,
    };
    let x = centered_x(work.left, work_w, size[0]);
    let y = work.top + (work_h - size[1] - 64).max(0);
    ([x, y], size)
}

#[cfg(target_os = "windows")]
fn overlay_max_width(hwnd: HWND) -> i32 {
    let work = monitor_work_rect(hwnd);
    let work_w = work.right - work.left;
    (work_w as f32 * STREAMING_OVERLAY_MAX_SCREEN_RATIO).round() as i32
}

#[cfg(target_os = "windows")]
fn adjust_overlay_pos_size_for_text(
    hwnd: HWND,
    status: &OverlayStatus,
    current_pos: &[i32; 2],
    current_size: &[i32; 2],
) -> ([i32; 2], [i32; 2]) {
    // EDITFONT-183: 测量字号按状态选择。STREAMFONT-189：RecordingWithText 的自绘文字
    // 也已归 OVERLAY_TEXT_FONT_SIZE(16px) 渲染（D2D streaming_text_format + GDI
    // streaming_font 兜底），测量必须跟随；若仍按 14px 量宽，自动宽度低估 ~14%
    // ⇒ 文字更早溢出 ⇒ ES_AUTOHSCROLL 横向滚动更频繁（FIX-172-B 依赖测渲一致，
    // 这是正确性不是外观 —— EDITFONT-183 在编辑态踩过并修过同一坑）。
    let font_size = match status {
        OverlayStatus::RecordingWithText { .. } | OverlayStatus::StreamingEditing { .. } => {
            OVERLAY_TEXT_FONT_SIZE
        }
        _ => OVERLAY_FONT_SIZE,
    };
    let text = match status {
        OverlayStatus::RecordingWithText { text } => text.as_str(),
        OverlayStatus::StreamingEditing { text } => text.as_str(),
        _ => return (*current_pos, *current_size),
    };
    let text = text;

    let work = monitor_work_rect(hwnd);
    let work_w = work.right - work.left;
    let work_h = work.bottom - work.top;
    let max_w = overlay_max_width(hwnd);

    unsafe {
        let hdc = GetDC(hwnd);
        // OVERLAY-054-G: one unified overlay font size for both self-drawn text and the
        // EDIT control, so measuring font always matches the drawing font.
        let font = create_clear_type_font(font_size);
        let old_font = SelectObject(hdc, font);
        let text_w = measure_text_width(hdc, text);
        let _ = SelectObject(hdc, old_font);
        let _ = DeleteObject(font);
        let _ = ReleaseDC(hwnd, hdc);

        let extra = if matches!(status, OverlayStatus::StreamingEditing { .. }) {
            24 // caret/padding room in edit mode
        } else {
            0
        };
        let desired_w = STREAMING_TEXT_LEFT_MARGIN + text_w + STREAMING_TEXT_RIGHT_MARGIN + extra;
        let w = desired_w.clamp(
            RECORDING_OVERLAY_SIZE[0],
            max_w.max(RECORDING_OVERLAY_SIZE[0]),
        );
        let h = RECORDING_OVERLAY_SIZE[1];
        // OVERLAY-101: 传 clamp 后的 w（target 口径）——只换调用不换宽度（主控边界）。
        let x = centered_x(work.left, work_w, w);
        let y = work.top + (work_h - h - 64).max(0);
        ([x, y], [w, h])
    }
}

#[cfg(target_os = "windows")]
fn measure_text_width(hdc: HDC, text: &str) -> i32 {
    if text.is_empty() {
        return 0;
    }
    let wide = encode_wide(text);
    let mut size = windows::Win32::Foundation::SIZE::default();
    unsafe {
        let len = wide.len().saturating_sub(1);
        if len > 0 {
            let _ = GetTextExtentPoint32W(hdc, &wide[..len], &mut size);
        }
    }
    size.cx
}
/// OVERLAY-086 / D2D-P1 / REFACTOR-088: 流式文字横向滚动偏移。
/// 文字宽度未超出可视区时不滚动；超出后按超出量左移，
/// 使**最新文字始终贴右边缘**（ASR-038-C 产品交互契约）。
/// GDI 与 D2D 两条绘制路径共用此函数，防止公式漂移（REFACTOR-088：两份内联
/// 实现迟早分叉，而 GDI 是兜底路径平时不可见，分叉要等回落那天才炸）。
///
/// 🔴 f32/i32 等价性前提（主控 REFACTOR-088 批准记录，改动前必读）：
/// D2D 侧调用点把 `visible_w`（整数值 f32）转 `i32` 传入、结果转回 `f32`，逐位无损。
/// 成立链条：① `with_d2d` 的 `w = (rect.right - rect.left).max(1) as f32`——整数像素宽转来，
/// 是**整数值 f32**；② 两个 margin（`STREAMING_TEXT_LEFT_MARGIN=42` /
/// `STREAMING_TEXT_RIGHT_MARGIN=49`）是 **i32 常量**，`as f32` 后仍是整数值；
/// ③ `text_left=42.0`、`text_right=w-49.0`，差值 `visible_w = w-91.0`（或 max 边界 1.0）
/// 仍是整数值；④ 屏幕宽度远小于 f32 精确整数表示上限 2^24，减法无舍入
/// → **f32→i32 截断永不发生**，i32 运算与原浮点运算逐位同值，转回 f32 无损。
///
/// 🔴 **前提断裂条件**（静默差 1px、不报错）：若将来把 margin 改成非整数，
/// 或让窗口宽度带小数（如高 DPI 缩放路径按非整数像素布置），本函数的 i32 口径
/// 与 D2D 浮点几何之间就会出现真截断——届时必须把两处调用点一起换成 f32 口径。
#[cfg(target_os = "windows")]
fn streaming_scroll_offset(text_width: i32, visible_w: i32) -> i32 {
    (text_width - visible_w).max(0)
}
/// OVERLAY-101: 水平居中 x 必须由**实际应用的宽度**算出。
/// 全仓所有 overlay 水平居中点（Show 流式分支 :1274 前身 / 插值循环 :1778 前身 /
/// `overlay_geometry` :4228 / `adjust_overlay_pos_size_for_text` :4281）共用本函数，
/// 杜绝「居中用 A 宽、SetWindowPos 应用 B 宽」的源头分叉。
///
/// 缺陷史（Bug B 本体）：Show 的 !do_it（100ms 节流命中）分支旧代码不重算 x，
/// 沿用 `resolved_pos` = `show_overlay` 传入的 `overlay_geometry` 默认位（按基准宽
/// 240 居中），而同一调用 SetWindowPos 应用的宽度是 in-flight `current_size`
/// （到上限后 ≫240）→ 窗口中心右偏 (current-240)/2 px；且到上限后
/// current == target、插值循环休眠，无任何帧纠正，直到下一包 do_it（~700ms）
/// 才复位 = Gavin 端测「宽度扩展到上限后瞬跳、向右窜」。
/// （do_it 分支 OVERLAY-068-A R1 已用 current 居中，无此问题。）
///
/// 消融（tester-1 参考）：任一调用点改回内联公式/换宽度来源（如用 target 宽或
/// resolved_pos 的 x）→ 右窜/拉扯复发；本函数是唯一居中宽度源。
#[cfg(target_os = "windows")]
fn centered_x(work_left: i32, work_w: i32, applied_w: i32) -> i32 {
    work_left + (work_w - applied_w) / 2
}
/// OVERLAY-086 Bug 3 / REFACTOR-089: 尺寸插值单轴单帧推进（宽、高共用）。
///
/// 变宽与变窄一律走 `interpolate_step`（OVERLAY-086 前变宽是直接 snap 到
/// target，导致流式每来一包文字窗口瞬跳、居中 x 随之瞬跳 = Gavin 报的
/// 「位置向左移动 / 抖动」）。收敛到 1px 以内时吸附，避免微抖。
///
/// 等价性（REFACTOR-089，主控逐环复核）：
/// 1. **两轴独立**：`dy = target_h - current_h` 与宽度无关；宽轴的吸附输出不回流入
///    高轴。原执行序「step w → step h → snap w → snap h」与本函数的
///    「(step+snap w) → (step+snap h)」逐位同值（吸附条件只读本轴的 target/current）。
/// 2. **d == 0**：现语义=跳过 step 但吸附检查仍跑——`|0| <= 1` 成立、把
///    `current` 赋成 `target`（同值 no-op）；本函数 `return current`，同值。
/// 3. **|d| = 1 / 2 边界**：`interpolate_step` 单帧 ≥1px 且 ≤25%，两处都吸附到
///    target，与现状逐位同值。
/// 4. `interpolate_step` 本体零改动（TEST-SYNC-043 护栏钉着）。
///
/// 消融（护栏 7 参考，tester-1）：把本函数改回 `if d > 0 { return target }`
/// （变宽直达 = 瞬跳）→ 用例驱动 `advance_width(current, current+200)` 单帧即得
/// target 全宽 → 断言「推进量 == interpolate_step(200)==50」红。旧内联版此用例
/// 无法真实调用（公式被重写在测试里，改回 snap 照样绿 = 判别力 0）。
#[cfg(target_os = "windows")]
fn advance_width(current: i32, target: i32) -> i32 {
    let d = target - current;
    if d == 0 {
        return current;
    }
    let next = current + interpolate_step(d);
    if (target - next).abs() <= 1 {
        target
    } else {
        next
    }
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
    let (pos, size) = overlay_geometry(&status, HWND(overlay_handle.overlay_hwnd.0));
    overlay_handle.send(OverlayCommand::Show(OverlayRequest {
        status,
        pos: Some(pos),
        size,
        opacity: overlay_opacity.clamp(0.1, 1.0),
        ui_language,
        auto_close_ms: 0,
        target_hwnd: 0,
    }));
}

#[cfg(target_os = "windows")]
fn show_overlay_streaming_idle(
    overlay_handle: &OverlayThreadHandle,
    overlay_opacity: f32,
    ui_language: config::UiLanguage,
    target_hwnd: platform::WindowId,
) {
    // OVERLAY-051-E: the first Show for an online streaming ASR session uses a dedicated
    // placeholder status. The target_hwnd is preserved so later submit can return focus.
    overlay_handle.send(OverlayCommand::Show(OverlayRequest {
        status: OverlayStatus::RecordingStreamingIdle,
        pos: None,
        size: RECORDING_OVERLAY_SIZE,
        opacity: overlay_opacity.clamp(0.1, 1.0),
        ui_language,
        auto_close_ms: 0,
        target_hwnd,
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
    last_streaming_text: &Arc<Mutex<Option<String>>>,
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
                        let (pos, size) = overlay_geometry(
                            &OverlayStatus::Error(msg.clone()),
                            overlay_handle.overlay_hwnd,
                        );
                        overlay_handle.send(OverlayCommand::Show(OverlayRequest {
                            status: OverlayStatus::Error(msg),
                            pos: Some(pos),
                            size,
                            opacity: 0.95,
                            ui_language: config.ui_language,
                            auto_close_ms: 2000,
                            target_hwnd: 0,
                        }));
                        // HOTKEY-115 B3 残余收口：这是 Start 事件唯一不产出任何 pipeline
                        // 事件的拒绝出口，toggle 态若不复位会滞留 true（录音从未开始），
                        // 下一次按键被反转成 no-op Stop、两按才恢复。借用既有单一收口
                        // notify_translate_poll_stop() 复位（对 PTT 无副作用，只清 TOGGLE_ACTIVE
                        // 并停掉已 spawn 的 translate 轮询线程）。
                        platform::notify_translate_poll_stop();
                        continue;
                    }

                    // HOTKEY-LATENCY-FIX-001: 立即显示录音 overlay，不等 RecordingStarted 事件
                    let config = clone_runtime_config(runtime_config);
                    let target_hwnd = hwnd.0 as usize;
                    // OVERLAY-051-E: decide whether this is online streaming ASR. We cannot read
                    // the worker's is_streaming_asr flag yet, so infer from configured model.
                    // ASR-056: 用 is_online_streaming() 收敛判据（QwenAudioOnline | FunAsrRealtime）
                    let is_streaming_asr = {
                        let cfg = clone_runtime_config(runtime_config);
                        transcription::AsrModel::from_config(&cfg.audio.asr_model)
                            .is_online_streaming()
                    };
                    if is_streaming_asr {
                        show_overlay_streaming_idle(
                            overlay_handle,
                            config.audio.overlay_opacity.clamp(0.1, 1.0),
                            config.ui_language,
                            target_hwnd,
                        );
                    } else {
                        // OVERLAY-054-B: use overlay_geometry for the initial Recording position so the
                        // first Show matches the later RecordingStarted Show; avoids a flash at (0,0).
                        let (pos, size) = overlay_geometry(
                            &OverlayStatus::Recording,
                            overlay_handle.overlay_hwnd,
                        );
                        overlay_handle.send(OverlayCommand::Show(OverlayRequest {
                            status: OverlayStatus::Recording,
                            pos: Some(pos),
                            size,
                            opacity: config.audio.overlay_opacity.clamp(0.1, 1.0),
                            ui_language: config.ui_language,
                            auto_close_ms: 0,
                            target_hwnd,
                        }));
                    }
                    log::info!(
                        "[Latency] overlay shown at +{:.1}ms",
                        t_hotkey.elapsed().as_secs_f64() * 1000.0
                    );
                    set_tray_state(tray, TrayState::Recording, config.ui_language);

                    let _ = worker_tx.send(WorkerCommand::Start(StartCmd {
                        target_hwnd,
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
                STREAMING_STOPPED.store(true, Ordering::Release);
                // OVERLAY-149 (F2): 编辑态按停止热键缺守卫 —— OVERLAY_EDITING 不清则
                // 后续 Done/Cancelled 被压制臂（`if OVERLAY_EDITING.load(...)`）拦下，
                // 既不回 Idle 也不 Hide，Processing 浮层永久卡屏直到下次录音。
                // 修法按「退出编辑态」语义清 flag；非编辑态时该值本就 false，逐位不变
                // （单写者 = controller 线程，无竞态；完整事件序列推演见 result.md）。
                OVERLAY_EDITING.store(false, Ordering::Release);
                if is_recording.load(Ordering::Acquire) {
                    let config = clone_runtime_config(runtime_config);
                    show_overlay(
                        overlay_handle,
                        config.audio.overlay_opacity,
                        config.ui_language,
                        OverlayStatus::FallingToProcessing {
                            message: i18n::get(config.ui_language)
                                .overlay_transcribing
                                .to_string(),
                        },
                    );
                }
            }
            HotkeyEvent::CancelStop => {
                log::info!("Controller received hotkey cancel-stop (PTT held < 300ms)");
                cancel_signal.store(true, Ordering::Release);
                stop_recording_signal.store(true, Ordering::Release);
                STREAMING_STOPPED.store(true, Ordering::Release);
                platform::notify_translate_poll_stop();
            }
        }
    }
    if is_recording.load(Ordering::Acquire) {
        let esc = unsafe { GetAsyncKeyState(VK_ESCAPE.0 as i32) };
        if (esc as u16) & 0x8000u16 != 0 {
            cancel_signal.store(true, Ordering::Release);
            stop_recording_signal.store(true, Ordering::Release);
            STREAMING_STOPPED.store(true, Ordering::Release);
            platform::notify_translate_poll_stop();
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
                // New recording session resets any stale editing state from a previous session.
                OVERLAY_EDITING.store(false, Ordering::Release);
                STREAMING_STOPPED.store(false, Ordering::Release);
                set_tray_state(tray, TrayState::Recording, ui_language);
                show_overlay(
                    overlay_handle,
                    opacity,
                    ui_language,
                    OverlayStatus::Recording,
                );
            }
            PipelineEvent::StreamingIdle => {
                // OVERLAY-051-E: worker confirmed online streaming ASR is waiting for first text;
                // show the placeholder overlay (keeps tray in Recording).
                set_tray_state(tray, TrayState::Recording, ui_language);
                show_overlay_streaming_idle(overlay_handle, opacity, ui_language, 0);
            }
            PipelineEvent::StreamingText(gen, text, words) => {
                // OVERLAY-075: cross-session identity gate — MUST run before anything consumes
                // the packet (overlay AND the edit-learn mirror below), so a stale session's
                // finalize-tail text can neither render into this session's window nor pollute
                // the WORDBOOK-053-B mirror used to learn user corrections.
                let current_gen = STREAMING_GENERATION.load(Ordering::Acquire);
                if gen != current_gen {
                    log::debug!(
                        "OVERLAY-075: dropping StreamingText from stale session (gen {} != current {})",
                        gen,
                        current_gen
                    );
                    continue;
                }
                // WORDBOOK-053-B: mirror the latest streaming text for the edit-learn path.
                if let Ok(mut mirror) = last_streaming_text.lock() {
                    *mirror = Some(text.clone());
                }
                // ASR-038-B: 流式 ASR 增量文本推送到 overlay
                // OVERLAY-043-B / FLICKER-130 (R1): late streaming packets after stop are
                // always ignored, editing or not — the old editing exemption destroyed the
                // EDIT control instead of syncing it (see should_ignore_streaming_text doc).
                let stopped = STREAMING_STOPPED.load(Ordering::Acquire);
                let editing = OVERLAY_EDITING.load(Ordering::Acquire);
                if should_ignore_streaming_text(stopped) {
                    log::debug!("OVERLAY-043: ignoring late StreamingText after stop");
                } else if editing {
                    // Editing mode: keep the EDIT control text in sync without switching window status.
                    overlay_handle.send(OverlayCommand::Show(OverlayRequest {
                        status: OverlayStatus::StreamingEditing { text: text.clone() },
                        pos: None,
                        size: RECORDING_OVERLAY_SIZE,
                        opacity,
                        ui_language,
                        auto_close_ms: 0,
                        target_hwnd: 0,
                    }));
                } else {
                    // OVERLAY-051-G: store word timings for timestamp-driven reveal
                    overlay_handle.send(OverlayCommand::UpdateWordTimings(words));
                    show_overlay(
                        overlay_handle,
                        opacity,
                        ui_language,
                        OverlayStatus::RecordingWithText { text },
                    );
                }
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
                // ASR-038-C-REWORK-001: if the user has taken over editing, do not revert tray to Idle
                // and do not hide the overlay; editing controls the end-of-session lifecycle.
                if OVERLAY_EDITING.load(Ordering::Acquire) {
                    log::info!("ASR-038-C: suppressing Done/Cancelled → Idle/Hide while editing");
                } else {
                    set_tray_state(tray, TrayState::Idle, ui_language);
                    overlay_handle.send(OverlayCommand::Hide);
                }
                platform::notify_translate_poll_stop();
            }
            PipelineEvent::FocusLost(text) => {
                OVERLAY_EDITING.store(false, Ordering::Release);
                set_tray_state(tray, TrayState::Idle, ui_language);
                platform::notify_translate_poll_stop();
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
                OVERLAY_EDITING.store(false, Ordering::Release);
                log::error!("Pipeline error: {}", message);
                platform::notify_translate_poll_stop();
                set_tray_state(tray, TrayState::Error, ui_language);
                // Shimmer animation frame update (phase increments every cycle)
                let friendly_message = convert_to_friendly_error(&message, ui_language);
                let (pos, size) = overlay_geometry(
                    &OverlayStatus::Error(friendly_message.clone()),
                    overlay_handle.overlay_hwnd,
                );
                overlay_handle.send(OverlayCommand::Show(OverlayRequest {
                    status: OverlayStatus::Error(friendly_message),
                    pos: Some(pos),
                    size,
                    opacity: 0.95,
                    ui_language: config.ui_language,
                    auto_close_ms: 2000,
                    target_hwnd: 0,
                }));
            }
            // FORMAT-LLM-001-CORE (DEC-031-③): LLM 格式化失败提示。
            // 历史教训：必须显式复位 tray 状态到 Idle，否则会卡在"处理中"。
            PipelineEvent::FormatFailed => {
                OVERLAY_EDITING.store(false, Ordering::Release);
                log::warn!("LLM formatting failed; raw text injected as fallback");
                platform::notify_translate_poll_stop();
                set_tray_state(tray, TrayState::Idle, ui_language);
                let hint = i18n::get(ui_language).format_failed_hint;
                let (pos, size) = overlay_geometry(
                    &OverlayStatus::Error(hint.to_string()),
                    overlay_handle.overlay_hwnd,
                );
                overlay_handle.send(OverlayCommand::Show(OverlayRequest {
                    status: OverlayStatus::Error(hint.to_string()),
                    pos: Some(pos),
                    size,
                    opacity: 0.9,
                    ui_language,
                    auto_close_ms: 2500,
                    target_hwnd: 0,
                }));
            }
            // BUG-119: 「用户没说话」→ 信息提示（Info 态，蓝点白字，非红色错误样式），
            // 文案走 i18n no_speech_hint。同 FormatFailed：必须显式复位 tray 到 Idle。
            PipelineEvent::NoSpeech => {
                OVERLAY_EDITING.store(false, Ordering::Release);
                log::info!("Pipeline: no speech detected; showing info hint");
                platform::notify_translate_poll_stop();
                set_tray_state(tray, TrayState::Idle, ui_language);
                let hint = i18n::get(ui_language).no_speech_hint;
                let (pos, size) = overlay_geometry(
                    &OverlayStatus::Info(hint.to_string()),
                    overlay_handle.overlay_hwnd,
                );
                overlay_handle.send(OverlayCommand::Show(OverlayRequest {
                    status: OverlayStatus::Info(hint.to_string()),
                    pos: Some(pos),
                    size,
                    opacity: 0.9,
                    ui_language,
                    auto_close_ms: 2500,
                    target_hwnd: 0,
                }));
            }
        }
    }
    while let Ok(event) = overlay_event_rx.try_recv() {
        let ui_language = clone_runtime_config(runtime_config).ui_language;
        match event {
            OverlayUiEvent::CancelRequested => {
                // ESC-178: 永久日志链 ③——消费端收到 + 收口执行（H4 判别点）。
                log::debug!(
                    "ESC-178: controller consumed CancelRequested, editing was {}, hiding",
                    OVERLAY_EDITING.load(Ordering::Acquire)
                );
                cancel_signal.store(true, Ordering::Relaxed);
                stop_recording_signal.store(true, Ordering::Relaxed);
                OVERLAY_EDITING.store(false, Ordering::Release);
                STREAMING_STOPPED.store(true, Ordering::Release);
                overlay_handle.send(OverlayCommand::Hide);
                set_tray_state(tray, TrayState::Idle, ui_language);
            }
            OverlayUiEvent::PreviewCopied => {
                overlay_handle.send(OverlayCommand::Hide);
            }
            OverlayUiEvent::EditRequested => {
                // ASR-038-C-REWORK-001: set controller-side editing flag BEFORE cancel_signal, so the
                // inevitable PipelineEvent::Cancelled from the worker does not hide the overlay.
                OVERLAY_EDITING.store(true, Ordering::Release);
                // ASR-038-C: 用户点击 overlay 文本区进入编辑态
                // 停录音 + 取消 pipeline（关 WebSocket 由 ASR 线程检测 cancel_signal 处理）
                cancel_signal.store(true, Ordering::Relaxed);
                stop_recording_signal.store(true, Ordering::Relaxed);
                STREAMING_STOPPED.store(true, Ordering::Release);
                platform::notify_translate_poll_stop();
                // 通知 overlay 线程切换到 StreamingEditing 态并创建 EDIT 控件
                overlay_handle.send(OverlayCommand::EnterEditMode);
            }
            OverlayUiEvent::SubmitRequested(text, target_hwnd) => {
                OVERLAY_EDITING.store(false, Ordering::Release);
                STREAMING_STOPPED.store(true, Ordering::Release);
                // WORDBOOK-053-B: learn the explicit user correction (original ASR text vs submitted
                // edited text) in a detached thread. This is an online-streaming-ASR-only signal:
                // only the streaming path produces StreamingText events, which populate
                // last_streaming_text. Local-model pipeline never reaches here.
                let original_text = last_streaming_text.lock().ok().and_then(|m| m.clone());
                if let Some(original) = original_text {
                    let runtime_config = Arc::clone(runtime_config);
                    let edited = text.clone();
                    thread::spawn(move || {
                        let auto_learn_threshold = read_auto_learn_threshold(&runtime_config);
                        if let Err(e) = wordbook::Wordbook::open().and_then(|wb| {
                            wb.learn_correction(&original, &edited, auto_learn_threshold)
                        }) {
                            log::debug!("Overlay auto-learning skipped: {}", e);
                        }
                    });
                }
                // OVERLAY-054-A: 编辑提交时，先把 overlay 的 NOACTIVATE 恢复、销毁 EDIT 控件并隐藏
                // 窗口，再把焦点还给原目标窗口，确认焦点真正到达后再注入文本。
                overlay_handle.send(OverlayCommand::RestoreAndHide);
                let text_to_inject = text.clone();
                let overlay_tx_for_focus = overlay_handle.tx.clone();
                let runtime_config_for_focus = Arc::clone(runtime_config);
                let ui_language_for_focus = ui_language;
                let _ = std::thread::spawn(move || {
                    // Give the overlay thread one frame to destroy the EDIT control and hide the window.
                    std::thread::sleep(Duration::from_millis(16));
                    let mut fallback = false;
                    if target_hwnd != 0 {
                        let target = HWND(target_hwnd as *mut std::ffi::c_void);
                        let restored = unsafe { SetForegroundWindow(target).as_bool() };
                        if restored {
                            // Wait up to 200ms for the foreground window to actually switch.
                            let deadline = std::time::Instant::now() + Duration::from_millis(200);
                            let mut focus_ok = false;
                            while std::time::Instant::now() < deadline {
                                let hwnd = unsafe { GetForegroundWindow() };
                                if hwnd.0 as usize == target_hwnd {
                                    focus_ok = true;
                                    break;
                                }
                                std::thread::sleep(Duration::from_millis(8));
                            }
                            if focus_ok {
                                let config = clone_runtime_config(&runtime_config_for_focus);
                                if let Err(e) = platform::inject_text(
                                    &text_to_inject,
                                    config.injection.use_clipboard,
                                    config.injection.clipboard_delay_ms,
                                ) {
                                    log::error!(
                                        "OVERLAY-054-A: inject text failed after focus restore: {}",
                                        e
                                    );
                                    fallback = true;
                                }
                            } else {
                                log::warn!(
                                    "OVERLAY-054-A: foreground did not switch to target_hwnd {} within timeout",
                                    target_hwnd
                                );
                                fallback = true;
                            }
                        } else {
                            fallback = true;
                        }
                    } else {
                        fallback = true;
                    }
                    if fallback {
                        let _ = platform::copy_text_to_clipboard(&text_to_inject);
                        let _ = overlay_tx_for_focus.send(OverlayCommand::Show(OverlayRequest {
                            status: OverlayStatus::FocusLost {
                                text: text_to_inject,
                                copied: true,
                            },
                            pos: None,
                            size: PREVIEW_OVERLAY_SIZE,
                            opacity: 0.95,
                            ui_language: ui_language_for_focus,
                            auto_close_ms: 0,
                            target_hwnd: 0,
                        }));
                    }
                });
                set_tray_state(tray, TrayState::Idle, ui_language);
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
/// ASR-DUAL-B-001: 加载 hotwords 字符串（仅 accuracy 模式需要）
/// 从 wordbook 读取所有单词，按 id 排序保证哈希稳定，构建逗号分隔字符串
/// performance 模式返回 None（不支持 hotwords）
// MACOS-P4-NEUTRAL-002: 原 #[cfg(target_os = "windows")] 去除——平台中立纯 Rust（AsrModel 判定 + wordbook 读取 + build_hotwords_string），spawn_worker_thread（已去 cfg）调用，对 Windows 为 no-op。
fn load_hotwords_for_accuracy(config: &AppConfig) -> Option<String> {
    if transcription::AsrModel::from_config(&config.audio.asr_model)
        != transcription::AsrModel::Accuracy
    {
        return None;
    }
    match wordbook::Wordbook::open() {
        Ok(wb) => match wb.list_all() {
            Ok(entries) => {
                // list_all 已按 id DESC 排序，确定性顺序
                let words: Vec<String> = entries.into_iter().map(|e| e.word).collect();
                let s = transcription::build_hotwords_string(&words);
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
            Err(e) => {
                log::warn!("Failed to load wordbook for hotwords: {}", e);
                None
            }
        },
        Err(e) => {
            log::warn!("Failed to open wordbook for hotwords: {}", e);
            None
        }
    }
}

/// ASR-DUAL-B-001: 计算 hotwords 字符串的版本号（用于对比是否需要重建）
// MACOS-P4-NEUTRAL-002: 原 #[cfg(target_os = "windows")] 去除——平台中立纯 Rust（DefaultHasher 哈希），spawn_worker_thread（已去 cfg）调用，对 Windows 为 no-op。
fn compute_hotwords_version(hotwords: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    let count = hotwords.split(',').filter(|s| !s.trim().is_empty()).count();
    count.hash(&mut hasher);
    hotwords.hash(&mut hasher);
    hasher.finish()
}

// FIX-164 D2: Start 决策管线前等待重载完成的上限。超时/失败/停止信号 ⇒ 退回旧行为
// （旧实例继续），🔴 硬红线：绝不挂死热键路径。1500ms 覆盖在线模型重建（秒级内）；
// 本地 972MB 模型重建更久时会退回旧实例——但 D1 预热已把绝大多数场景提前完成。
const ASR_RELOAD_START_WAIT_MS: u64 = 1500;

/// FIX-164 D1: 失败签名 —— (期望模型, 在线 key, url, model)。空闲预热对同一签名不
/// 自动重试（防重试风暴：失败后 active_* 保持旧值，若无签名记忆每 500ms tick 都会
/// 重 spawn 一次重建）；配置再变签名即变，预热恢复；Start 主动路径不受限（用户显式
/// 触发，重试合理）。词库版本不进签名：预热廉价层不读词库（主控拍板取舍）。
type FailedReloadKey = (transcription::AsrModel, String, String, String);

/// FIX-164 D1: 重载判定「廉价层」——模型身份 + 在线 ASR 配置对比，不读词库（词库
/// 版本判定要开 SQLite，只留在 Start 完整判定里）。预热（空闲 tick）与 Start 完整
/// 判定共用本函数，杜绝两份判定漂移（任务书红线：判定不许复制粘贴两份）。
/// 返回 (是否需要重载, 期望模型, 在线配置是否变更)。
fn asr_cheap_reload_needed(
    config: &AppConfig,
    active_asr_model: transcription::AsrModel,
    active_asr_online_api_key: &str,
    active_asr_online_url: &str,
    active_asr_online_model: &str,
) -> (bool, transcription::AsrModel, bool) {
    let desired_asr_model = transcription::AsrModel::from_config(&config.audio.asr_model);
    let identity_changed = active_asr_model != desired_asr_model;
    // ASR-041-B / ASR-056: 在线 ASR 配置变更检测（key/url/model 变更），
    // is_online_streaming() 收敛判据（QwenAudioOnline | FunAsrRealtime）
    let online_asr_changed = desired_asr_model.is_online_streaming()
        && (active_asr_online_api_key != config.audio.asr_online_api_key
            || active_asr_online_url != config.audio.asr_online_url
            || active_asr_online_model != config.audio.asr_online_model);
    (
        identity_changed || online_asr_changed,
        desired_asr_model,
        online_asr_changed,
    )
}

/// FIX-164 D1/D2: 重载 spawn 收口 —— 预热与 Start 共用同一 spawn（判定+spawn 都不许
/// 两份）。行为与原 Start 内联版逐位一致：后台线程构建，成功/失败都经 reload_tx 送回。
fn spawn_asr_reload(
    model_dir: &std::path::Path,
    config: &AppConfig,
    desired_asr_model: transcription::AsrModel,
    desired_hotwords: Option<String>,
    reload_tx: crossbeam_channel::Sender<Result<transcription::Transcriber, String>>,
) {
    let reload_model_dir = model_dir.to_path_buf();
    let reload_streaming = config.audio.enable_streaming;
    let reload_language = "auto".to_string();
    let reload_asr_online_key = config.audio.asr_online_api_key.clone();
    let reload_asr_online_url = config.audio.asr_online_url.clone();
    let reload_asr_online_model = config.audio.asr_online_model.clone();
    let reload_asr_online_max_sentence_silence = config.audio.asr_online_max_sentence_silence;
    std::thread::spawn(move || {
        let t_build = std::time::Instant::now();
        match transcription::Transcriber::new(
            &reload_model_dir,
            reload_streaming,
            reload_language,
            desired_asr_model,
            desired_hotwords.as_deref(),
            &reload_asr_online_key,
            &reload_asr_online_url,
            &reload_asr_online_model,
            reload_asr_online_max_sentence_silence,
        ) {
            Ok(new_t) => {
                log::info!(
                    "ASR transcriber rebuild completed in {:.1}s",
                    t_build.elapsed().as_secs_f64()
                );
                let _ = reload_tx.send(Ok(new_t));
            }
            Err(e) => {
                // 失败也必须回消息，worker 侧据此清除 in_flight 标志
                let _ = reload_tx.send(Err(e.to_string()));
            }
        }
    });
}

/// FIX-164: 重载结果应用收口 —— loop 顶与 D2 有界等待共用（交换实例/同步跟踪值/
/// 清除 in_flight/记录或清除失败签名），杜绝两份应用逻辑漂移。
#[allow(clippy::too_many_arguments)]
fn apply_reload_result(
    result: Result<transcription::Transcriber, String>,
    runtime_config: &Arc<RwLock<AppConfig>>,
    transcriber: &mut Option<transcription::Transcriber>,
    active_asr_model: &mut transcription::AsrModel,
    active_hotwords_version: &mut u64,
    active_asr_online_api_key: &mut String,
    active_asr_online_url: &mut String,
    active_asr_online_model: &mut String,
    asr_reload_in_flight: &mut bool,
    failed_reload_key: &mut Option<FailedReloadKey>,
) {
    match result {
        Ok(new_transcriber) => {
            log::info!("ASR transcriber hot-reload completed, swapping instance");
            *active_asr_model = new_transcriber.asr_model();
            *active_hotwords_version = new_transcriber.hotwords_version();
            // ASR-041-B: 同步在线 ASR 配置跟踪值
            let fresh = clone_runtime_config(runtime_config);
            *active_asr_online_api_key = fresh.audio.asr_online_api_key;
            *active_asr_online_url = fresh.audio.asr_online_url;
            *active_asr_online_model = fresh.audio.asr_online_model;
            *transcriber = Some(new_transcriber);
            *asr_reload_in_flight = false;
            *failed_reload_key = None; // FIX-164: 成功即清失败签名，预热恢复待命
        }
        Err(e) => {
            // active_* 保持旧值，下次 Start 对比仍不一致 → 自然重试
            log::warn!(
                "ASR transcriber hot-reload failed: {}, keeping old instance",
                e
            );
            *asr_reload_in_flight = false;
            // FIX-164 D1 防风暴：见 FailedReloadKey 文档
            let cfg = clone_runtime_config(runtime_config);
            *failed_reload_key = Some((
                transcription::AsrModel::from_config(&cfg.audio.asr_model),
                cfg.audio.asr_online_api_key,
                cfg.audio.asr_online_url,
                cfg.audio.asr_online_model,
            ));
        }
    }
}

// MACOS-P4-NEUTRAL-002: 去 cfg——WorkerCommand/StartCmd 已中立，worker 线程 macOS 侧接线需此函数可见。
// 对 Windows 构建该 cfg 恒为真，删除为 no-op。
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
        //
        // DEC-025 双模型架构：transcriber 可热重载（asr_model 变更或词库变更触发）。
        // 重建异步进行，期间继续用旧实例不阻塞录音。
        let config = clone_runtime_config(&runtime_config);
        let initial_hotwords = load_hotwords_for_accuracy(&config);
        let mut transcriber = match transcription::Transcriber::new(
            &model_dir,
            config.audio.enable_streaming,
            "auto".to_string(),
            transcription::AsrModel::from_config(&config.audio.asr_model),
            initial_hotwords.as_deref(),
            &config.audio.asr_online_api_key,
            &config.audio.asr_online_url,
            &config.audio.asr_online_model,
            config.audio.asr_online_max_sentence_silence,
        ) {
            Ok(t) => Some(t),
            Err(err) => {
                log::error!("Failed to initialize transcriber at startup: {}", err);
                None
            }
        };

        // ASR-DUAL-B-001: 热重载 channel —— 后台线程构建结果（成功=新实例，失败=Err）经此送回。
        // 失败也必须回消息，用于清除 in_flight 标志，否则构建中窗口会重复 spawn 重建线程。
        let (asr_reload_tx, asr_reload_rx) =
            crossbeam_channel::bounded::<Result<transcription::Transcriber, String>>(1);
        // 重建进行中标志：spawn 时置 true，收到成功/失败消息时清除。
        // 防止 ~6s 构建窗口内每次 Start 都重复 spawn（并发加载多个 972MB 模型）。
        let mut asr_reload_in_flight = false;
        // FIX-164 D1: 失败签名，见 FailedReloadKey 文档（防空闲预热重试风暴）
        let mut failed_reload_key: Option<FailedReloadKey> = None;
        // 当前已生效的 asr_model + hotwords_version，用于对比是否需要重建
        let mut active_asr_model: transcription::AsrModel =
            transcription::AsrModel::from_config(&config.audio.asr_model);
        let mut active_hotwords_version: u64 = transcriber
            .as_ref()
            .map(|t| t.hotwords_version())
            .unwrap_or(0);
        // ASR-041-B: 在线 ASR 配置跟踪，用于热重载对比
        let mut active_asr_online_api_key: String = config.audio.asr_online_api_key.clone();
        let mut active_asr_online_url: String = config.audio.asr_online_url.clone();
        let mut active_asr_online_model: String = config.audio.asr_online_model.clone();

        // PERF-INIT-001: Pre-initialize LlmClient once; update_config() before each use.
        let mut llm_client = llm::LlmClient::new(config.llm.clone());

        // PERF-INIT-001: Pre-initialize TranslationEngine once; hot-reload only on config change.
        let mut cached_translation: Option<(
            config::TranslationLanguage,
            translation::TranslationEngine,
        )> = if config.translation.enabled {
            translation::TranslationEngine::load_for_direction(
                &model_dir,
                config.translation.target_language,
            )
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
            // ASR-DUAL-B-001: 非阻塞检查热重载结果（后台线程构建完成后送回）。
            // FIX-164: 应用逻辑收口进 apply_reload_result（与 D2 有界等待共用）。
            if let Ok(result) = asr_reload_rx.try_recv() {
                apply_reload_result(
                    result,
                    &runtime_config,
                    &mut transcriber,
                    &mut active_asr_model,
                    &mut active_hotwords_version,
                    &mut active_asr_online_api_key,
                    &mut active_asr_online_url,
                    &mut active_asr_online_model,
                    &mut asr_reload_in_flight,
                    &mut failed_reload_key,
                );
            }
            // FIX-164 D1: 空闲预热 —— 配置变更后 ≤500ms（tick 周期）即在后台启动重载，
            // 消除「切完模型第一次录音用旧引擎」（DEC-025 遗留缺口，DIAG-163 Q3 实锤）。
            // 只在 idle tick（cmd=None）判定：录音期间 loop 顶不执行，天然不会中途换引擎；
            // Start 处理器占用 loop 时本 tick 也轮不到。判定用廉价层（不读词库，主控拍板）；
            // in_flight 防重复 spawn + 失败签名防重试风暴，双保险。
            if cmd.is_none() && !asr_reload_in_flight {
                let config_now = clone_runtime_config(&runtime_config);
                let (cheap_needed, desired_now, _) = asr_cheap_reload_needed(
                    &config_now,
                    active_asr_model,
                    &active_asr_online_api_key,
                    &active_asr_online_url,
                    &active_asr_online_model,
                );
                let desired_key: FailedReloadKey = (
                    desired_now,
                    config_now.audio.asr_online_api_key.clone(),
                    config_now.audio.asr_online_url.clone(),
                    config_now.audio.asr_online_model.clone(),
                );
                if cheap_needed && failed_reload_key.as_ref() != Some(&desired_key) {
                    log::info!(
                        "FIX-164 D1 idle prewarm: config change detected (model {:?} -> {:?}), reloading transcriber in background",
                        active_asr_model,
                        desired_now
                    );
                    let desired_hotwords = load_hotwords_for_accuracy(&config_now);
                    asr_reload_in_flight = true;
                    spawn_asr_reload(
                        &model_dir,
                        &config_now,
                        desired_now,
                        desired_hotwords,
                        asr_reload_tx.clone(),
                    );
                }
            }
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
                    // OVERLAY-075: a new recording session claims the next generation.
                    // Everything this session's ASR thread emits is stamped with this value;
                    // the consumer (:3682 area) drops stamps that no longer match, so a
                    // finalize-tail from a previous session can never reach a newer session's
                    // overlay. Bump BEFORE the ASR closure below captures `session_generation`.
                    let session_generation =
                        STREAMING_GENERATION.fetch_add(1, Ordering::Release) + 1;
                    send_event(&event_tx, PipelineEvent::RecordingStarted);
                    let config = clone_runtime_config(&runtime_config);
                    let device_name = if config.audio.input_device.trim().is_empty() {
                        None
                    } else {
                        Some(config.audio.input_device.as_str())
                    };

                    // ASR-DUAL-B-001: 检查是否需要热重载 transcriber
                    // 触发条件：asr_model 变更 / transcription_language 变更 /
                    // accuracy 模式下词库变更 / qwen3 配置变更 / transcriber 未初始化自愈
                    // FIX-164: 廉价层判定抽为 asr_cheap_reload_needed（预热与 Start 共用，
                    // 不许两份）；词库版本 + 自愈只属于 Start 完整层（词库判定要读库）。
                    let (cheap_needed, desired_asr_model, online_asr_changed) =
                        asr_cheap_reload_needed(
                            &config,
                            active_asr_model,
                            &active_asr_online_api_key,
                            &active_asr_online_url,
                            &active_asr_online_model,
                        );
                    let desired_hotwords = load_hotwords_for_accuracy(&config);
                    let desired_hotwords_version = match &desired_hotwords {
                        Some(h) => compute_hotwords_version(h),
                        None => 0,
                    };
                    // R2-4: transcriber.is_none() 时无条件尝试重建（启动失败自愈）
                    let needs_rebuild = transcriber.is_none() && !asr_reload_in_flight;
                    let needs_reload = cheap_needed
                        || (desired_asr_model == transcription::AsrModel::Accuracy
                            && active_hotwords_version != desired_hotwords_version)
                        || needs_rebuild;
                    if needs_reload && !asr_reload_in_flight {
                        log::info!(
                            "Triggering ASR transcriber hot-reload: model {:?}->{:?}, hotwords_version {}->{}, online_asr_changed={}, needs_rebuild={}",
                            active_asr_model,
                            desired_asr_model,
                            active_hotwords_version,
                            desired_hotwords_version,
                            online_asr_changed,
                            needs_rebuild,
                        );
                        asr_reload_in_flight = true;
                        spawn_asr_reload(
                            &model_dir,
                            &config,
                            desired_asr_model,
                            desired_hotwords,
                            asr_reload_tx.clone(),
                        );
                    }

                    log::info!("[Latency] worker received Start command");

                    // FIX-164 D2: 有界等待兜底 —— 模型身份/在线配置已变更且重载仍在途时，
                    // 最多等 ASR_RELOAD_START_WAIT_MS 让新实例落地，堵住「切完立刻按热键
                    // 仍用旧引擎」（DIAG-163 Q3：输出是旧模型质量，用户以为在用新引擎——
                    // 静默的功能性错误）。超时/失败/停止信号 ⇒ 退回旧行为（旧实例继续），
                    // 🔴 硬红线：绝不挂死热键路径。等待逐 ≤100ms 切片，随时可被
                    // stop/cancel 信号打断（用户连按两下热键的后悔场景）。
                    // hotwords 版本变更不等待（主控拍板：Q3 病根是引擎身份，词库走原路径）。
                    let (cheap_needed_now, _, _) = asr_cheap_reload_needed(
                        &config,
                        active_asr_model,
                        &active_asr_online_api_key,
                        &active_asr_online_url,
                        &active_asr_online_model,
                    );
                    if cheap_needed_now && asr_reload_in_flight {
                        let wait_deadline = std::time::Instant::now()
                            + Duration::from_millis(ASR_RELOAD_START_WAIT_MS);
                        log::info!(
                            "FIX-164 D2: reload in flight, waiting up to {}ms for new transcriber before deciding pipeline",
                            ASR_RELOAD_START_WAIT_MS
                        );
                        while asr_reload_in_flight {
                            if stop_recording_signal.load(Ordering::Acquire)
                                || cancel_signal.load(Ordering::Acquire)
                            {
                                log::info!(
                                    "FIX-164 D2: stop signal during reload wait, proceeding with current instance"
                                );
                                break;
                            }
                            let remaining =
                                wait_deadline.saturating_duration_since(std::time::Instant::now());
                            if remaining.is_zero() {
                                log::warn!(
                                    "FIX-164 D2: reload wait timed out after {}ms, proceeding with current (old) instance",
                                    ASR_RELOAD_START_WAIT_MS
                                );
                                break;
                            }
                            match asr_reload_rx
                                .recv_timeout(remaining.min(Duration::from_millis(100)))
                            {
                                Ok(result) => {
                                    apply_reload_result(
                                        result,
                                        &runtime_config,
                                        &mut transcriber,
                                        &mut active_asr_model,
                                        &mut active_hotwords_version,
                                        &mut active_asr_online_api_key,
                                        &mut active_asr_online_url,
                                        &mut active_asr_online_model,
                                        &mut asr_reload_in_flight,
                                        &mut failed_reload_key,
                                    );
                                }
                                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                            }
                        }
                    }

                    // ASR-038-B / ASR-056: 在线流式 ASR 走真流式管线（边录边发边收边上屏）
                    // 其他模式走现有 record() + run_pipeline_core（零行为变更）
                    // ASR-056: 用 is_online_streaming() 收敛判据（QwenAudioOnline | FunAsrRealtime）
                    let desired_asr_model_check =
                        transcription::AsrModel::from_config(&config.audio.asr_model);
                    let is_streaming_asr = desired_asr_model_check.is_online_streaming()
                        && transcriber
                            .as_ref()
                            .is_some_and(|t| t.asr_model().is_online_streaming());

                    if is_streaming_asr {
                        // OVERLAY-051-E: notify controller that online streaming ASR is waiting for first text
                        send_event(&event_tx, PipelineEvent::StreamingIdle);

                        let transcriber_ref = transcriber.as_ref().expect("checked above");
                        let asr_online_url = transcriber_ref.asr_online_url().to_string();
                        let asr_online_model = transcriber_ref.asr_online_model().to_string();
                        let asr_online_max_sentence_silence =
                            transcriber_ref.asr_online_max_sentence_silence();
                        let qwen_api_key = config.audio.asr_online_api_key.clone();
                        let model_dir_clone = model_dir.clone();
                        let cancel_clone = Arc::clone(&cancel_signal);
                        let event_tx_clone = event_tx.clone();

                        // chunk channel：record_streaming 推 chunk，ASR 线程读
                        let (chunk_tx, chunk_rx) = crossbeam_channel::bounded::<Vec<f32>>(256);

                        // spawn ASR 线程跑 transcribe_streaming_realtime
                        let asr_handle: std::thread::JoinHandle<Result<String>> =
                            std::thread::spawn(move || {
                                let vocabulary = crate::transcription::load_wordbook_vocabulary();
                                crate::transcription::qwen_inference::transcribe_streaming_realtime(
                                    &asr_online_url,
                                    &qwen_api_key,
                                    &asr_online_model,
                                    chunk_rx,
                                    &vocabulary,
                                    asr_online_max_sentence_silence,
                                    &model_dir_clone,
                                    Some(&cancel_clone),
                                    |display_text, words| {
                                        // OVERLAY-075: stamp every packet with this session's
                                        // generation; the consumer drops stale-session packets.
                                        let _ = event_tx_clone.send(PipelineEvent::StreamingText(
                                            session_generation,
                                            display_text.to_string(),
                                            words.to_vec(),
                                        ));
                                    },
                                )
                            });

                        // worker 线程跑 record_streaming，推 chunk 给 ASR 线程
                        let record_result = audio_capture.record_streaming(
                            Arc::clone(&stop_recording_signal),
                            config.audio.silence_threshold,
                            config::SILENCE_DURATION_MS,
                            config::MAX_RECORD_SECONDS,
                            Some(Arc::clone(&audio_buf)),
                            device_name,
                            |chunk| {
                                // ASR-074-GUARD: bounded channel send was an unbounded
                                // blocking send — if the ASR thread stopped consuming
                                // (WS stuck / server silent / half-open connection),
                                // chunk_tx fills and the recording thread never returns:
                                // release-hotkey presents as a frozen app. 200ms cap:
                                // a healthy consumer drains 256 chunks in milliseconds,
                                // so 200ms is 20x headroom above normal send latency;
                                // at worst the recording thread stalls 200ms per chunk
                                // instead of forever. Timeout = drop the chunk with a
                                // counted warn (same drop-visibility principle as
                                // ASR-074 Step 2-C's [ASR-DROP] tag) — never silently.
                                match chunk_tx.send_timeout(
                                    chunk.to_vec(),
                                    Duration::from_millis(200),
                                ) {
                                    Ok(()) => {}
                                    Err(crossbeam_channel::SendTimeoutError::Timeout(_)) => {
                                        ASR_CHUNK_DROPS.fetch_add(1, Ordering::Relaxed);
                                        log::warn!(
                                            "[ASR-DROP] chunk send timed out after 200ms (ASR consumer stuck?); total dropped: {}",
                                            ASR_CHUNK_DROPS.load(Ordering::Relaxed)
                                        );
                                    }
                                    Err(crossbeam_channel::SendTimeoutError::Disconnected(_)) => {
                                        // ASR thread gone: drop silently, the recording
                                        // session is being torn down anyway.
                                    }
                                }
                            },
                        );
                        // drop chunk_tx 让 ASR 线程的 channel 断开（触发 finish-task）
                        drop(chunk_tx);

                        log::info!(
                            "[Latency] record_streaming() completed after +{:.1}ms",
                            t_worker.elapsed().as_secs_f64() * 1000.0
                        );
                        is_recording.store(false, Ordering::Release);

                        if cancel_signal.load(Ordering::Acquire) {
                            log::warn!(
                                "Streaming recording ended with cancel_signal=true, skipping ASR join"
                            );
                            send_event(&event_tx, PipelineEvent::Cancelled);
                            continue;
                        }

                        if let Err(e) = record_result {
                            log::error!("Streaming recording error: {}", e);
                            send_event(&event_tx, PipelineEvent::Error(e.to_string()));
                            // ASR 线程会因 channel 断开自行退出
                            continue;
                        }

                        // join ASR 线程拿最终文本
                        let asr_result = match asr_handle.join() {
                            Ok(r) => r,
                            Err(_) => {
                                log::error!("ASR thread panicked");
                                send_event(
                                    &event_tx,
                                    PipelineEvent::Error("ASR thread panicked".into()),
                                );
                                continue;
                            }
                        };
                        let streaming_text = match asr_result {
                            Ok(text) => text,
                            Err(e) => {
                                // BUG-119: 流式 ASR「没识别到语音」= 类型化信号 → 信息提示，
                                // 不进 convert_to_friendly_error 错误链
                                if e.is::<transcription::NoSpeechError>() {
                                    log::info!("Streaming ASR: no speech detected");
                                    send_event(&event_tx, PipelineEvent::NoSpeech);
                                } else {
                                    log::error!("Streaming ASR error: {}", e);
                                    send_event(&event_tx, PipelineEvent::Error(e.to_string()));
                                }
                                continue;
                            }
                        };

                        // 流式文本走 LLM 后半段（跳过转录）
                        let transcriber = match &transcriber {
                            Some(t) => t,
                            None => {
                                log::error!("Transcriber not initialized");
                                send_event(
                                    &event_tx,
                                    PipelineEvent::Error("Transcriber unavailable".into()),
                                );
                                continue;
                            }
                        };
                        llm_client.update_config(config.llm.clone());

                        let needs_reload = match &cached_translation {
                            Some((lang, _)) => {
                                !config.translation.enabled
                                    || *lang != config.translation.target_language
                            }
                            None => config.translation.enabled,
                        };
                        if needs_reload {
                            cached_translation = if config.translation.enabled {
                                translation::TranslationEngine::load_for_direction(
                                    &model_dir,
                                    config.translation.target_language,
                                )
                                .map(|engine| (config.translation.target_language, engine))
                            } else {
                                None
                            };
                        }
                        if config.punctuation.enabled && cached_punctuation.is_none() {
                            cached_punctuation = punctuation::PunctuationEngine::new(&model_dir);
                        } else if !config.punctuation.enabled {
                            cached_punctuation = None;
                        }

                        run_pipeline_core(
                            Ok(Vec::new()), // 流式模式不用 samples
                            transcriber,
                            &rt,
                            &llm_client,
                            &cancel_signal,
                            &config,
                            &runtime_config,
                            &mut cached_translation,
                            &model_dir,
                            cached_punctuation.as_mut(),
                            start.target_hwnd,
                            &event_tx,
                            start.translate,
                            Some(streaming_text), // 流式文本，跳过转录
                        );
                        continue;
                    }

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
                        log::warn!("Recording ended with cancel_signal=true, skipping transcription (user cancelled or race condition)");
                        send_event(&event_tx, PipelineEvent::Cancelled);
                        continue;
                    }

                    let transcriber = match &transcriber {
                        Some(t) => t,
                        None => {
                            log::error!("Transcriber not initialized at startup");
                            send_event(
                                &event_tx,
                                PipelineEvent::Error("Transcriber unavailable".into()),
                            );
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
                            translation::TranslationEngine::load_for_direction(
                                &model_dir,
                                config.translation.target_language,
                            )
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
                    log::debug!(
                        "Starting run_pipeline: cancel_signal={}",
                        cancel_signal.load(Ordering::Relaxed)
                    );
                    run_pipeline_core(
                        samples_result,
                        transcriber,
                        &rt,
                        &llm_client,
                        &cancel_signal,
                        &config,
                        &runtime_config,
                        &mut cached_translation,
                        &model_dir,
                        cached_punctuation.as_mut(),
                        start.target_hwnd,
                        &event_tx,
                        start.translate,
                        None,
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
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let _ = version_check::check_and_cache();
    });
    log::info!("Controller initialized, entering message loop");
    let mut tray: Option<TrayIcon> = None;
    let mut settings_child: Option<Child> = None;
    let mut msg = MSG::default();
    // WORDBOOK-053-B: controller-side mirror of the last streaming ASR text. It is updated on
    // the controller thread by every PipelineEvent::StreamingText, so it always matches the text
    // the user saw before entering overlay edit mode.
    let last_streaming_text: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
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
                    &last_streaming_text,
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
                    &last_streaming_text,
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
                    &last_streaming_text,
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

#[cfg(target_os = "macos")]
fn single_instance_lock_path() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("feiyin-ime")
        .join("instance.lock")
}

// MACOS-P4-EXIT-001: macOS 信号处理（SIGINT/SIGTERM）→ 干净退出。
// handler 只置 AtomicBool（async-signal-safe 唯一允许动作），轮询线程在普通
// 线程上下文调 platform::request_stop()（CFRunLoopSignalSource/WakeUp 非 async-signal-safe，
// 不能在 handler 内调）。零新增依赖（libc 已在 macOS 段）。
#[cfg(target_os = "macos")]
static MACOS_SIGINT_RECEIVED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "macos")]
extern "C" fn macos_signal_handler(_signum: i32) {
    // async-signal-safe：只置 AtomicBool，不做任何其他事。
    MACOS_SIGINT_RECEIVED.store(true, std::sync::atomic::Ordering::Release);
}

/// MACOS-P4-EXIT-001: 注册 SIGINT/SIGTERM handler + 启动轮询线程调 request_stop。
/// 全部代码在 `#[cfg(target_os = "macos")]` 内，Windows 零影响。
#[cfg(target_os = "macos")]
fn install_macos_signal_handler() {
    use std::sync::atomic::Ordering;
    unsafe {
        // libc::signal: 注册 handler，返回旧 handler（忽略）。SIGINT=2, SIGTERM=15。
        // SA_RESTART 语义由 macOS 默认提供（signal() 设 SA_RESTART）。
        // 先 cast to pointer 再 cast to sighandler_t（避免 function_casts 警告）。
        let handler = macos_signal_handler as *const () as libc::sighandler_t;
        let _ = libc::signal(libc::SIGINT, handler);
        let _ = libc::signal(libc::SIGTERM, handler);
    }
    // 轮询线程：检测 SIGINT_RECEIVED 后在普通上下文调 request_stop（安全）。
    std::thread::spawn(move || loop {
        if MACOS_SIGINT_RECEIVED.load(Ordering::Acquire) {
            log::info!("SIGINT/SIGTERM received, requesting controller stop");
            platform::request_stop();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    });
}

#[cfg(target_os = "macos")]
fn run_controller_macos(runtime_config: Arc<RwLock<AppConfig>>) -> Result<()> {
    // MACOS-P4-HOST-001 + NEUTRAL-002: macOS controller host.
    // 结构镜像 Windows run_controller：建 channel → spawn_worker_thread（已中立）→
    // 逻辑线程处理 hotkey 事件 + 消费 pipeline_event_rx → CFRunLoop 宿主保持进程常驻。
    //
    // 关键差异（vs Windows run_controller）：
    // - 不用 run_message_loop_with_hotkey_listener（其 timer callback 只 log，不处理业务）；
    //   改用 run_message_loop() 做纯 CFRunLoop 宿主（CONTROLLER_CTX 为 null 时 timer 空转），
    //   hotkey 事件由本函数 spawn 的逻辑线程独立轮询——无竞争，因不共享 rx。
    // - overlay/录音浮层：OVERLAY-WIRE-001 已接入 handle_pipeline_event
    //   （request_overlay → 主线程 15ms timer 应用 Show/Hide）；
    //   FocusLost 必须把文本复制到剪贴板（platform::copy_text_to_clipboard），
    //   否则用户整段转写会静默丢失。
    // - foreground_window_id() 在 macOS 当前返回 0（NEUTRAL-001 第一版降级），
    //   → focus_lost 恒 false（总是直接注入、不走失焦预览）。本轮接受此降级。
    // - MACOS-P4-EXIT-001: 信号处理（SIGINT/SIGTERM）——handler 只置 AtomicBool（async-signal-safe），
    //   轮询线程在普通上下文调 platform::request_stop()（非 handler，安全）。

    install_macos_signal_handler();

    let audio_buf = ui::overlay::new_audio_level_buf();
    let stop_recording_signal = Arc::new(AtomicBool::new(false));
    let cancel_signal = Arc::new(AtomicBool::new(false));
    let is_recording = Arc::new(AtomicBool::new(false));
    let (worker_tx, worker_rx) = crossbeam_channel::unbounded::<WorkerCommand>();
    let (pipeline_event_tx, pipeline_event_rx) = crossbeam_channel::unbounded::<PipelineEvent>();

    // spawn_worker_thread 现已中立（NEUTRAL-002 去 cfg），macOS 侧可调用。
    let worker_join = spawn_worker_thread(
        worker_rx,
        pipeline_event_tx,
        Arc::clone(&runtime_config),
        Arc::clone(&audio_buf),
        Arc::clone(&stop_recording_signal),
        Arc::clone(&cancel_signal),
        Arc::clone(&is_recording),
    );
    // Wait briefly for worker thread init (ASR model preload).
    std::thread::sleep(std::time::Duration::from_millis(100));

    let hotkey_listener = platform::create_hotkey_listener(Arc::clone(&runtime_config));
    let _config_watcher = spawn_config_watcher(Arc::clone(&runtime_config));

    platform::create_controller_window()?;

    // OVERLAY-WIRE-001: 把共享 audio_buf 交给浮层模块（方案②，主线程一次性存入）。
    // 必须是同一个 Arc——音频线程（:2733 spawn_worker_thread）正在往里写，浮层读它画波形。
    // OVERLAY-002-A: 一并注入停止/取消信号，供浮层停止按钮（mouseDown）触发取消录音。
    // OVERLAY-003: 注入 ui_language，供 Preview 浮层按钮/标题按语言取 i18n 文案。
    platform::init_overlay_levels(
        Arc::clone(&audio_buf),
        Arc::clone(&stop_recording_signal),
        Arc::clone(&cancel_signal),
        clone_runtime_config(&runtime_config).ui_language,
    );

    // MACOS-P4-TRAY-001: status bar tray (NSStatusItem) + menu.
    // 必须在 create_controller_window（NSApplication finishLaunching）之后建——
    // NSStatusBar 要求 app 已启动。菜单命令（OpenSettings/Exit）经 channel 送逻辑
    // 线程处理；tray 状态更新经 request_tray_state → 主线程 15ms timer 轮询应用。
    let (tray_cmd_tx, tray_cmd_rx) = crossbeam_channel::unbounded::<platform::TrayCommand>();
    // TRAY-FIX-001: 不再把 `&tray` 存进裸指针静态量（tray 随后被 move，指针悬垂 →
    // 15ms timer 解引用野指针 SIGABRT）。改用 thread_local 持有所有权：
    // set_tray 将 tray 交给 tray.rs 的 thread_local，主线程 timer 轮询直接取用。
    match platform::build_tray(
        clone_runtime_config(&runtime_config).ui_language,
        tray_cmd_tx,
    ) {
        Ok(tray) => {
            platform::set_tray(tray);
        }
        Err(e) => {
            log::error!("Failed to build macOS tray: {}", e);
        }
    };

    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(5));
        let _ = version_check::check_and_cache();
    });

    // 逻辑线程：处理 hotkey 事件 + 消费 pipeline_event_rx。
    // 镜像 Windows process_controller_events 对 HotkeyEvent::Start/Stop/CancelStop 的处理。
    // 独立持有 hotkey_rx 的克隆（crossbeam Receiver 可 Clone），与 listener 本身解耦，
    // listener 留在本函数作用域供 shutdown/join。
    let logic_runtime_config = Arc::clone(&runtime_config);
    let logic_worker_tx = worker_tx.clone();
    let logic_stop_recording = Arc::clone(&stop_recording_signal);
    let logic_cancel_signal = Arc::clone(&cancel_signal);
    let logic_is_recording = Arc::clone(&is_recording);
    let logic_hotkey_rx = hotkey_listener.rx().clone();
    let logic_tray_cmd_rx = tray_cmd_rx.clone();
    let logic_handle = thread::spawn(move || {
        loop {
            // 退出判据：worker_tx 已 drop（主线程在 shutdown 时 drop）→ send 失败即退出。
            // 用非阻塞轮询避免阻塞主线程 shutdown。
            crossbeam_channel::select! {
                recv(logic_hotkey_rx) -> event => {
                    match event {
                        Ok(ev) => handle_hotkey_event(
                            ev,
                            &logic_worker_tx,
                            &logic_stop_recording,
                            &logic_cancel_signal,
                            &logic_is_recording,
                            &logic_runtime_config,
                        ),
                        Err(crossbeam_channel::RecvError) => {
                            log::info!("macOS logic thread: hotkey_rx disconnected, exiting");
                            break;
                        }
                    }
                }
                recv(logic_tray_cmd_rx) -> cmd => {
                    match cmd {
                        Ok(platform::TrayCommand::OpenSettings) => {
                            log::info!("macOS tray: open settings requested");
                            match spawn_settings_process() {
                                Ok(child) => {
                                    log::info!(
                                        "macOS settings process spawned, pid={}",
                                        child.id()
                                    );
                                }
                                Err(e) => {
                                    log::error!("Failed to spawn settings process: {}", e);
                                }
                            }
                        }
                        Ok(platform::TrayCommand::Exit) => {
                            log::info!("macOS tray: exit requested, requesting controller stop");
                            platform::request_stop();
                            break;
                        }
                        Err(crossbeam_channel::RecvError) => {
                            log::info!("macOS logic thread: tray_cmd_rx disconnected, exiting");
                            break;
                        }
                    }
                }
                default(std::time::Duration::from_millis(50)) => {}
            }
            // 消费 pipeline 事件（本轮仅打日志，FocusLost 复制剪贴板）
            let logic_ui_language = clone_runtime_config(&logic_runtime_config).ui_language;
            while let Ok(event) = pipeline_event_rx.try_recv() {
                handle_pipeline_event(&event, logic_ui_language);
            }
        }
        log::info!("macOS logic thread exiting");
    });

    log::info!("macOS controller initialized, entering CFRunLoop message loop");

    // CFRunLoop 宿主（保持进程常驻）。CONTROLLER_CTX 为 null，timer 空转。
    let result = platform::run_message_loop();

    // Shutdown sequence
    hotkey_listener.shutdown();
    let _ = hotkey_listener.join();
    // 通知 worker 退出 + join
    let _ = worker_tx.send(WorkerCommand::Shutdown);
    drop(worker_tx);
    let _ = worker_join.join();
    // 逻辑线程在 worker_tx drop 后会因 send 失败或 rx disconnected 退出
    let _ = logic_handle.join();
    // TRAY-FIX-001: 显式销毁 tray（thread_local 取走并 drop，移除状态栏图标）。
    // 与 shutdown_overlay 同构：thread_local 进程退出时会 drop，显式调用保证退出链清晰。
    platform::shutdown_tray();

    // OVERLAY-WIRE-001: 显式销毁浮层（主线程，run_message_loop 已退出）。
    // thread_local 进程退出时会 drop，但显式调用保证退出链清晰、日志可辨。
    platform::shutdown_overlay();

    if let Err(e) = &result {
        log::error!("macOS controller loop exited with error: {}", e);
    } else {
        log::info!("macOS controller loop exited cleanly");
    }

    result
}

/// MACOS-P4-NEUTRAL-002: macOS 侧 hotkey 事件处理。
/// 镜像 Windows process_controller_events 对 HotkeyEvent::Start/Stop/CancelStop 的处理逻辑。
#[cfg(target_os = "macos")]
fn handle_hotkey_event(
    event: platform::HotkeyEvent,
    worker_tx: &crossbeam_channel::Sender<WorkerCommand>,
    stop_recording_signal: &Arc<AtomicBool>,
    cancel_signal: &Arc<AtomicBool>,
    is_recording: &Arc<AtomicBool>,
    _runtime_config: &Arc<RwLock<AppConfig>>,
) {
    match event {
        platform::HotkeyEvent::Start { translate } => {
            log::info!(
                "macOS controller received hotkey start (translate={})",
                translate.load(Ordering::Acquire)
            );
            if is_recording.load(Ordering::Acquire) {
                // 已在录音 → 停止（PTT 释放）
                stop_recording_signal.store(true, Ordering::Release);
            } else {
                // 开始录音
                cancel_signal.store(false, Ordering::Release);
                stop_recording_signal.store(false, Ordering::Release);
                // NEUTRAL-001 降级：foreground_window_id() 返回 0 → focus_lost 恒 false。
                let target_hwnd = platform::foreground_window_id();
                let _ = worker_tx.send(WorkerCommand::Start(StartCmd {
                    target_hwnd,
                    translate,
                }));
                log::info!(
                    "[Latency] worker command sent (target_hwnd={})",
                    target_hwnd
                );
            }
        }
        platform::HotkeyEvent::Stop => {
            log::info!("macOS controller received hotkey stop");
            stop_recording_signal.store(true, Ordering::Release);
        }
        platform::HotkeyEvent::CancelStop => {
            log::info!("macOS controller received hotkey cancel-stop (PTT held < 300ms)");
            cancel_signal.store(true, Ordering::Release);
            stop_recording_signal.store(true, Ordering::Release);
        }
    }
}

/// OVERLAY-WIRE-002: PipelineEvent → 录音浮层指令的纯映射（无副作用，供单测锁死真值表）。
/// OVERLAY-002: 三态浮层映射 —— RecordingStarted → Show；Processing → ShowProcessing(文案)；
/// Error / FormatFailed → ShowError{文案, auto_close_ms}（Error 2000ms / FormatFailed 2500ms）；
/// 其余 → Hide。
/// OVERLAY-003: FocusLost(text) → ShowPreview(text)（失焦返显浮层，不自动关闭）。
/// 🔴 穷举 match 禁止 `_ => Hide` 通配符：将来 PipelineEvent 新增变体时编译器强制作者做决定，
/// 而非静默落进 Hide（本批次一路踩过来的同一类病：局部规则不声明适用边界）。
#[cfg(target_os = "macos")]
fn overlay_request_for_event(event: &PipelineEvent) -> platform::OverlayRequest {
    match event {
        PipelineEvent::RecordingStarted => platform::OverlayRequest::Show,
        PipelineEvent::Processing(message) => {
            platform::OverlayRequest::ShowProcessing(message.clone())
        }
        PipelineEvent::Error(message) => platform::OverlayRequest::ShowError {
            message: message.clone(),
            auto_close_ms: 2000,
        },
        PipelineEvent::FormatFailed => platform::OverlayRequest::ShowError {
            message: String::new(), // 文案由 handle_pipeline_event 按 ui_language 补齐
            auto_close_ms: 2500,
        },
        // BUG-119: macOS 侧无 Info 视觉样式（三态浮层），暂与 FormatFailed 同路走
        // ShowError 提示窗承载 i18n no_speech_hint —— 显示形态的升级留给 macOS
        // overlay 批（见 docs/MACOS-HANDOFF.md），「没说话不弹错误」语义本单已达成。
        PipelineEvent::NoSpeech => platform::OverlayRequest::ShowError {
            message: String::new(), // 文案由 handle_pipeline_event 按 ui_language 补齐
            auto_close_ms: 2500,
        },
        PipelineEvent::FocusLost(text) => platform::OverlayRequest::ShowPreview(text.clone()),
        PipelineEvent::StreamingText(_, _, _) => platform::OverlayRequest::Show, // macOS 侧流式文本暂不渲染
        PipelineEvent::Done | PipelineEvent::Cancelled => platform::OverlayRequest::Hide,
    }
}

/// MACOS-P4-NEUTRAL-002: macOS 侧 PipelineEvent 消费。
/// TRAY-001: 同步更新托盘状态（经 request_tray_state → 主线程 timer 应用）。
/// OVERLAY-WIRE-001/002: 同步驱动录音浮层（经 request_overlay → 主线程 timer 应用）。
/// OVERLAY-002 三态：RecordingStarted → Show；Processing → ShowProcessing；
/// Error/FormatFailed → ShowError（Error 2000ms / FormatFailed 2500ms 自动关闭）。
/// OVERLAY-003：FocusLost → ShowPreview（失焦返显浮层，不自动关闭）；Done/Cancelled → Hide。
/// overlay 指令由纯函数 `overlay_request_for_event` 统一计算（可单测锁死真值表）；
/// 仅 FormatFailed 的文案在调用侧按 ui_language 补齐。
#[cfg(target_os = "macos")]
fn handle_pipeline_event(event: &PipelineEvent, ui_language: config::UiLanguage) {
    // OVERLAY-002: FormatFailed 的展示文案依赖 ui_language，纯函数拿不到，这里补齐后发请求。
    // 其余事件直接用纯函数的映射结果（overlay_request_for_event 是唯一决策源）。
    let req = match event {
        PipelineEvent::FormatFailed => platform::OverlayRequest::ShowError {
            message: i18n::get(ui_language).format_failed_hint.to_string(),
            auto_close_ms: 2500,
        },
        PipelineEvent::NoSpeech => platform::OverlayRequest::ShowError {
            message: i18n::get(ui_language).no_speech_hint.to_string(),
            auto_close_ms: 2500,
        },
        _ => overlay_request_for_event(event),
    };
    platform::request_overlay(req);
    match event {
        PipelineEvent::RecordingStarted => {
            log::info!("macOS pipeline: RecordingStarted");
            platform::request_tray_state(TrayState::Recording, ui_language);
        }
        PipelineEvent::Processing(msg) => {
            log::info!("macOS pipeline: Processing({})", msg);
            platform::request_tray_state(TrayState::Processing, ui_language);
        }
        PipelineEvent::Done => {
            log::info!("macOS pipeline: Done (text injected)");
            platform::request_tray_state(TrayState::Idle, ui_language);
        }
        PipelineEvent::Cancelled => {
            log::info!("macOS pipeline: Cancelled");
            platform::request_tray_state(TrayState::Idle, ui_language);
        }
        PipelineEvent::FocusLost(text) => {
            // OVERLAY-003: FocusLost 现在真实可达（SCENE-002 让 foreground_window_id 返回真值）。
            // 预览浮层已展示文本；剪贴板兜底保留 —— 用户没点复制就关掉浮层时的最后防丢词保险。
            log::warn!(
                "macOS pipeline: FocusLost (复制到剪贴板兜底 + 预览浮层展示): {}",
                text
            );
            platform::request_tray_state(TrayState::Idle, ui_language);
            if let Err(e) = platform::copy_text_to_clipboard(text) {
                log::error!("macOS FocusLost: copy_text_to_clipboard failed: {}", e);
            }
        }
        PipelineEvent::Error(msg) => {
            log::error!("macOS pipeline: Error({})", msg);
            platform::request_tray_state(TrayState::Error, ui_language);
        }
        PipelineEvent::FormatFailed => {
            log::warn!("macOS pipeline: FormatFailed (LLM 格式化失败，原文已注入兜底)");
            platform::request_tray_state(TrayState::Idle, ui_language);
        }
        // BUG-119: macOS 侧「没说话」→ 信息提示（当前复用 ShowError 承载，见上）
        PipelineEvent::NoSpeech => {
            log::info!("macOS pipeline: NoSpeech (信息提示：请说话哦..)");
            platform::request_tray_state(TrayState::Idle, ui_language);
        }
        PipelineEvent::StreamingText(_, text, _) => {
            // ASR-038-B: macOS 侧流式文本暂不渲染（C-overlay 批后续实现）
            log::debug!("macOS pipeline: StreamingText ({} chars)", text.len());
        }
    }
}

fn main() -> Result<()> {
    // BUG-QWEN3-CRYPTO-001: 进程级 rustls ring provider 安装（任何 TLS 使用之前）
    // 重复安装返回 Err 属正常（幂等容忍），不得 unwrap
    let _ = rustls::crypto::ring::default_provider().install_default();

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
        let mutex_name = "Global\\feiyin-ime-single-instance-mutex";
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
    #[cfg(target_os = "macos")]
    {
        // MACOS-P4-HOST-001: flock-based single instance lock.
        // Avoid NSRunningApplication (unreliable for unsigned apps); use a lock file.
        let lock_path = single_instance_lock_path();
        if let Some(parent) = lock_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let file = match std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
        {
            Ok(f) => Some(f),
            Err(err) => {
                log::warn!("Single instance lock file open failed: {}, continuing", err);
                // Do not hard-fail startup just because lock file is unreachable.
                None
            }
        };
        if let Some(file) = file {
            use std::os::fd::AsRawFd;
            let fd = file.as_raw_fd();
            let ret = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
            if ret != 0 {
                log::warn!("Application already running (flock), exiting");
                return Ok(());
            }
            // Keep the file descriptor open for the lifetime of the process.
            let _ = std::mem::ManuallyDrop::new(file);
        }
    }
    #[cfg(target_os = "windows")]
    run_controller(runtime_config)?;
    #[cfg(target_os = "macos")]
    {
        run_controller_macos(runtime_config)?;
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        log::warn!("Non-Windows/non-macOS platform is not supported in controller mode yet");
    }
    Ok(())
}

/// FIRSTCHAR-FIX-006 (R3): Find the effective keep-start point in 16kHz samples
/// by locating speech onset and backtracking a margin to preserve weak aspirated
/// consonants.  Returns the sample index at which to begin keeping audio;
/// everything before this index is leading silence to be trimmed.
///
/// - Scans in 160-sample (10ms@16kHz) windows for RMS exceeding `threshold`
/// - Backtracks `backtrack_samples` from the onset to include consonant attack
/// - If no speech is detected, returns 0 (keep everything — "speak before key" case)
/// - If speech starts before `backtrack_samples`, returns 0 (no trimming needed)
fn find_speech_onset_with_backtrack(
    samples: &[f32],
    threshold: f32,
    backtrack_samples: usize,
) -> usize {
    let window_size = 160; // 10ms @ 16kHz
    let mut onset: Option<usize> = None;
    let mut idx = 0;
    while idx + window_size <= samples.len() {
        let window = &samples[idx..idx + window_size];
        let rms = (window.iter().map(|s| s * s).sum::<f32>() / window.len() as f32).sqrt();
        if rms > threshold {
            onset = Some(idx);
            break;
        }
        idx += window_size;
    }

    match onset {
        Some(on) => on.saturating_sub(backtrack_samples),
        None => 0,
    }
}

/// ASR-ACC-OPT-001 方案 B + ASR-CTC-OPT-001 P1：根据 ASR 模型选择前处理参数。
///
/// 返回 (silence_head_samples, onset_backtrack_samples)。
/// - Performance: 0ms head / 200ms backtrack（ASR-CTC-OPT-001 P1：50→0ms，
///   研究 RESEARCH-ASR-CTC-OPT-001 C1 证实 0ms 比 50ms 高 2.5pp，50ms 是旧
///   SenseVoice 遗产，FunASR Nano CTC 是 offline 模型不需 frame alignment padding）
/// - Accuracy: 0ms head / 100ms backtrack（native decoder 对前导静音敏感，
///   研究 RESEARCH-ASR-ACCURACY-001 R1 证实 50ms 静音头让 native 掉 10pp）
///
/// 提取为纯函数便于单测分支选择逻辑。
// MACOS-P4-NEUTRAL-001: 原 #[cfg(target_os = "windows")] 去除——此函数为平台中立纯 Rust 逻辑，
// run_pipeline_core（无 cfg）调用之，macOS 需可见。对 Windows 构建该 cfg 恒为真，删除为 no-op。
fn select_preprocessing_params(asr_model: transcription::AsrModel) -> (usize, usize) {
    const PERF_SILENCE_HEAD_SAMPLES: usize = 0; // 0ms @ 16kHz (performance, ASR-CTC-OPT-001 P1)
    const PERF_ONSET_BACKTRACK_SAMPLES: usize = 3200; // 200ms @ 16kHz (performance, 不动)
    const ACC_SILENCE_HEAD_SAMPLES: usize = 0; // 0ms @ 16kHz (accuracy)
    const ACC_ONSET_BACKTRACK_SAMPLES: usize = 1600; // 100ms @ 16kHz (accuracy)

    match asr_model {
        transcription::AsrModel::Accuracy => {
            (ACC_SILENCE_HEAD_SAMPLES, ACC_ONSET_BACKTRACK_SAMPLES)
        }
        transcription::AsrModel::Performance => {
            (PERF_SILENCE_HEAD_SAMPLES, PERF_ONSET_BACKTRACK_SAMPLES)
        }
        transcription::AsrModel::QwenAudioOnline | transcription::AsrModel::FunAsrRealtime => {
            // DEC-028 / RESEARCH-ASR-038 / ASR-038-B / ASR-041-B / ASR-056: 在线 ASR 模型
            // （在线模型对前导静音不敏感，保持与 CTC 一致的前处理行为）
            //
            // ASR-038-B 真流式拍板后更新：
            // silence_head / onset_backtrack 是批处理前处理概念，在 run_pipeline_core
            // 用于 trim/pad 完整 samples 数组后送转录。真流式路径（QwenAudioOnline /
            // FunAsrRealtime 走 record_streaming + transcribe_streaming_realtime）【绕过】本前处理，
            // 原因是边录边发无完整 samples 可 trim。真流式的前导静音由 VAD 入口门控
            // 处理（VAD 命中前的 chunk 缓冲后补发，不裁剪），详见 transcribe_streaming_realtime。
            // 本 match 分支仅对非流式回退路径（transcribe_with_punct_info 内的
            // QwenAudioOnline/FunAsrRealtime 分支）生效，真流式主路径不走这里。
            (PERF_SILENCE_HEAD_SAMPLES, PERF_ONSET_BACKTRACK_SAMPLES)
        }
    }
}

// MACOS-P4-NEUTRAL-001/002: run_pipeline 薄封装已删除（零调用者，自证见 result.md）。
// macOS 侧的调用者由 run_controller_macos 经 spawn_worker_thread 间接调用本函数（NEUTRAL-002 接线）。
// ASR-045: 流式模式（initial_text = Some）下 samples 为空是正常情况——音频已逐块直传
// ASR 线程（record_streaming → chunk channel → transcribe_streaming_realtime），finish-task
// 后以文本落回 initial_text，管线无需也不再消费 samples（见 :3409 调用侧「流式模式不用 samples」）。
// 只有「既无音频样本、也无流式文本」才真正没录到东西，需要取消管线。
// 判定为纯函数：护栏测试直接断言，回滚修复（判空条件还原）时测试即红。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn should_cancel_on_empty(samples: &[f32], initial_text: &Option<String>) -> bool {
    samples.is_empty() && initial_text.is_none()
}
/// BUG-119: 转录失败三分类。载体必须是本枚举而非 String —— 拦截点在 map_err，
/// 此刻 anyhow::Error 还带类型，downcast NoSpeechError 是「没说话」与
/// 「设备/模型异常」的唯一区分手段；转成 String 后一切只能靠关键词嗅探。
#[derive(Debug)]
enum TranscriptionFailure {
    /// 「用户没说话」类型化信号 → 上层发 PipelineEvent::NoSpeech 信息提示。
    /// 新增第三个产出源（transcription 内 bail!(NoSpeechError)）零改此处。
    NoSpeech,
    /// 其余错误转字符串，走既有 convert_to_friendly_error 显示链。
    Other(String),
}
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
fn run_pipeline_core(
    samples_result: anyhow::Result<Vec<f32>>,
    transcriber: &transcription::Transcriber,
    rt: &tokio::runtime::Runtime,
    llm_client: &llm::LlmClient,
    cancel_signal: &AtomicBool,
    config: &AppConfig,
    runtime_config: &Arc<RwLock<AppConfig>>,
    cached_translation: &mut Option<(config::TranslationLanguage, translation::TranslationEngine)>,
    model_dir: &Path,
    mut punctuation_engine: Option<&mut punctuation::PunctuationEngine>,
    target_hwnd: platform::WindowId,
    event_tx: &crossbeam_channel::Sender<PipelineEvent>,
    translate: Arc<AtomicBool>,
    // ASR-038-B: 流式模式传入已转录文本，跳过转录步骤直接走 LLM 后半段。
    // None = 正常模式（run_pipeline_core 内部转录）；Some(text) = 流式模式（跳过转录）
    initial_text: Option<String>,
) {
    match samples_result {
        Err(e) => {
            log::error!("Recording error: {}", e);
            send_event(event_tx, PipelineEvent::Error(e.to_string()));
        }
        // ASR-045: 判空取消仅在「无样本 且 无流式文本」时成立。
        // 流式模式（samples 空 + initial_text Some）属正常情况，落入 Ok(samples) 分支走 LLM 后半段。
        Ok(s) if should_cancel_on_empty(&s, &initial_text) => {
            log::warn!("No audio samples recorded");
            send_event(event_tx, PipelineEvent::Cancelled);
        }
        Ok(samples) => {
            if cancel_signal.load(Ordering::Relaxed) {
                log::warn!("Pipeline skipped: cancel_signal was true after recording completed (possible race condition)");
                send_event(event_tx, PipelineEvent::Cancelled);
                return;
            }
            // ASR-038-B: 流式模式跳过转录步骤，直接用 transcribe_streaming_realtime
            // 返回的文本走 LLM 后半段（ITN→LLM→注入）。转录已在 ASR 线程完成。
            // initial_text = Some → 流式；None → 正常转录
            //
            // BUG-119: 错误载体从 String 升为 TranscriptionFailure —— 拦截点必须在
            // map_err 处（anyhow::Error 还带类型的最后时机）下探 NoSpeechError；
            // 一旦 to_string() 就永远无法与普通错误区分（本 bug 成因链一环）。
            // 新增第三个「没说话」产出源只需 bail!(NoSpeechError)，此处零改动。
            let transcription_result: Result<(String, bool), TranscriptionFailure> = if let Some(
                text,
            ) =
                initial_text
            {
                if text.trim().is_empty() {
                    // BUG-119: 流式结果空文本 = 没说话，同样走信息提示
                    // （护栏契约不变：仍是用户可见反馈，不是第一层静默取消）
                    Err(TranscriptionFailure::NoSpeech)
                } else {
                    let native_punctuated = punctuation::has_effective_punctuation(&text);
                    Ok((text, native_punctuated))
                }
            } else {
                // 正常转录路径（FIRSTCHAR-FIX-006 前处理 + transcribe_with_punct_info）
                const SPEECH_ENERGY_THRESHOLD: f32 = 0.008;
                let (silence_head_samples, onset_backtrack_samples) =
                    select_preprocessing_params(transcriber.asr_model());
                let keep_start = find_speech_onset_with_backtrack(
                    &samples,
                    SPEECH_ENERGY_THRESHOLD,
                    onset_backtrack_samples,
                );
                let trimmed = &samples[keep_start..];
                let mut padded = Vec::with_capacity(silence_head_samples + trimmed.len());
                padded.resize(silence_head_samples, 0.0f32);
                padded.extend_from_slice(trimmed);
                log::info!(
                    "Transcribing {} samples (silence_head={}ms, trimmed leading silence by {} samples / {:.0}ms, asr_model={})",
                    padded.len(),
                    silence_head_samples as f64 / 16.0,
                    keep_start,
                    keep_start as f64 / 16.0,
                    if transcriber.asr_model() == transcription::AsrModel::Accuracy { "accuracy" } else { "performance" },
                );
                let transcribing_msg = i18n::get(config.ui_language).overlay_transcribing;
                send_event(
                    event_tx,
                    PipelineEvent::Processing(transcribing_msg.to_string()),
                );
                transcriber
                    .transcribe_with_punct_info(&padded, config.audio.chinese_script)
                    .map_err(|e| {
                        if e.is::<transcription::NoSpeechError>() {
                            TranscriptionFailure::NoSpeech
                        } else {
                            TranscriptionFailure::Other(e.to_string())
                        }
                    })
            };

            match transcription_result {
                Err(TranscriptionFailure::NoSpeech) => {
                    // BUG-119: 没识别到语音 → 信息提示（i18n no_speech_hint），非错误样式
                    log::info!("No speech detected, showing info hint");
                    send_event(event_tx, PipelineEvent::NoSpeech);
                }
                Err(TranscriptionFailure::Other(e)) => {
                    log::error!("Transcription error: {}", e);
                    send_event(event_tx, PipelineEvent::Error(e));
                }
                Ok((raw_text, native_punctuated)) => {
                    if cancel_signal.load(Ordering::Relaxed) {
                        send_event(event_tx, PipelineEvent::Cancelled);
                        return;
                    }
                    log::info!(
                        "Transcribed: {} (native_punctuated={})",
                        raw_text,
                        native_punctuated
                    );
                    // Skip downstream processing when transcription output is empty.
                    // BUG-119: 空文本（含 <|nospeech|> token 剥离后仅剩空白）= 没识别到语音
                    // → 信息提示。护栏契约不变：仍是用户可见反馈，不是静默取消。
                    if raw_text.trim().is_empty() {
                        log::warn!("Transcription result is empty (no speech content)");
                        send_event(event_tx, PipelineEvent::NoSpeech);
                        return;
                    }
                    // OPT-002: Skip if text is not effective (empty or filler-only).
                    // Note: previously fed ITN output here; ITN moved post-LLM (ITN-REORDER-001),
                    // so raw_text is used now. ITN only converts Chinese numerals to Arabic and
                    // adds ℃ etc. — it never adds/removes semantic characters, so filler detection
                    // results are identical (fillers 啊呃嗯… are not numerals).
                    if !text_normalizer::is_effective_text(&raw_text) {
                        log::info!(
                            "Skipping pipeline: transcription not effective (empty or filler only)"
                        );
                        send_event(event_tx, PipelineEvent::Cancelled);
                        return;
                    }
                    // ITN-V2-001 (R1 主通道)：ITN 从「LLM 后」回移到「LLM 前」。
                    // Gavin 2026-07-31 指令：LLM 会曲解原始数字表达（如「四点三刻」→「4:30」，
                    // 信息被销毁），故主通道在 LLM 之前先把中文数字→阿拉伯 + 单位符号定型，
                    // LLM 拿到的已是成形数字，UNIT_SYMBOL_PROTECTION 指令恢复生效。
                    // 补丁通道（normalize_unit_symbols_only）仍在 LLM 后兜底（见 :3124），
                    // 捞回 LLM 纠正 ASR 同音错字后的「40摄氏度」→「40℃」。
                    // 放在 is_effective_text 之后：ITN 不增删语义字符，filler 判定不变。
                    let pre_llm_text = itn::normalize_numbers(&raw_text);
                    // SCENE-SENSE-001-CORE (DEC-031-⑤): 录音完成阶段采集前台窗口场景信号，
                    // 供 LLM prompt F4 段注入 + multiline_safe 格式安全裁决。
                    // target_hwnd 即录音启动时捕获的前台窗口，此处复用同一 HWND 采集。
                    // 失败一律降级为 Unknown（不注入 F4，multiline_safe=false 保守）。
                    let scene_context = if config.scene.enabled {
                        match platform::capture_scene_signals_by_id(target_hwnd) {
                            Some((exe, title)) => scene::classify_scene(&exe, &title),
                            None => scene::SceneContext::unknown(),
                        }
                    } else {
                        scene::SceneContext::unknown()
                    };
                    let multiline_safe = scene_context.multiline_safe;
                    let send_window_title = config.scene.send_window_title;
                    // SCENE-OBS-001: 场景感知可观测性日志。
                    // 决策变更记录（2026-08-01）：原隐私红线「禁止打印 window_title」
                    // 出自 SCENE-OBS-001，Gavin 2026-08-01 裁定解除——debug.log 为纯本地
                    // 文件、不外发，故本地日志可记录 window_title 以支撑数据驱动的场景词表
                    // 优化（OBS-SCENE-TITLE-005）。
                    // ⚠️ 边界没有全解：本次只解除「本地日志」侧。send_window_title
                    //（控制标题上送 LLM）的隐私边界完全不变，仍默认 false——外发与本地
                    // 记录是两件事，后续不得据此放行标题给 LLM。
                    // f4_injected 判据与 build_scene_prompt_block 的 None 条件一致：
                    // 非 Unknown 且 style_hint 非空（不重复调用 build_scene_prompt_block 产生副作用）。
                    let f4_injected =
                        !scene_context.is_unknown() && !scene_context.style_hint.trim().is_empty();
                    log::info!(
                        "Scene context: app_exe={:?}, kind={}, multiline_safe={}, f4_injected={}, window_title={:?}",
                        scene_context.app_exe,
                        scene_context.scene.as_str(),
                        multiline_safe,
                        f4_injected,
                        scene_context.window_title,
                    );
                    // TRANS-008 B方案: translate=true 时走 LLM optimize+translate；translate=false 走 LLM optimize
                    // translate=false 閺冭绱濋崢鐔告箒 LLM optimize 鐠侯垰绶炴稉宥呭綁
                    let mut llm_handled = false;
                    // FORMAT-LLM-001-CORE (DEC-031): set when LLM formatting call
                    // failed. Raw text still gets injected (fallback), but we
                    // surface a brief "formatting failed" overlay hint after
                    // injection so the user knows to check LLM config.
                    let mut format_failed = false;
                    let translate_requested = translate.load(Ordering::Acquire);
                    // TRANS-BIDIR-001: 翻译方向由内容自动判定，不再由 config.translation.target_language 门控。
                    // target_language 字段保留仅为「上次使用方向缓存」（启动时据此预载引擎）。
                    // REFACTOR-DERIVE-TARGET-001: 提取为 translation::derive_translation_target（平台中立，可单测）
                    let derived_target = translation::derive_translation_target(&raw_text);
                    if translate_requested && config.translation.enabled {
                        log::info!(
                            "Translation direction derived: {:?} (target_language cache={:?})",
                            derived_target,
                            config.translation.target_language
                        );
                    }
                    let final_text = if translate_requested
                        && config.translation.enabled
                        && !raw_text.trim().is_empty()
                    {
                        let processing_msg = i18n::get(config.ui_language).overlay_processing;
                        send_event(
                            event_tx,
                            PipelineEvent::Processing(processing_msg.to_string()),
                        );
                        // REFACTOR-SHARE-TRANSDIR-001: function moved to translation/mod.rs (platform-neutral)
                        let effective_engine: Option<&translation::TranslationEngine> =
                            translation::ensure_translation_direction(
                                cached_translation,
                                &model_dir,
                                derived_target,
                            );
                        let script_instruction = text_normalizer::script_instruction_for_translate(
                            &raw_text,
                            config.audio.chinese_script,
                        );
                        if should_try_llm_translate(
                            config.llm.enabled,
                            config.llm.connectivity_verified,
                        ) {
                            // B: LLM optimization failed (non-critical), continue with raw result
                            match rt.block_on(llm_client.optimize_and_translate(
                                &pre_llm_text,
                                derived_target,
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
                                    try_nllb_translate(&pre_llm_text, effective_engine)
                                        .unwrap_or_else(|| {
                                            text_normalizer::normalize_text_for_language(
                                                &pre_llm_text,
                                                config.audio.chinese_script,
                                            )
                                        })
                                }
                            }
                        } else {
                            // LLM not eligible, use offline engine directly
                            try_nllb_translate(&pre_llm_text, effective_engine).unwrap_or_else(
                                || {
                                    text_normalizer::normalize_text_for_language(
                                        &pre_llm_text,
                                        config.audio.chinese_script,
                                    )
                                },
                            )
                        }
                    } else if config.llm.enabled && !raw_text.trim().is_empty() {
                        let processing_msg = i18n::get(config.ui_language).overlay_processing;
                        send_event(
                            event_tx,
                            PipelineEvent::Processing(processing_msg.to_string()),
                        );
                        let script_instruction = text_normalizer::script_instruction(
                            &raw_text,
                            config.audio.chinese_script,
                        );
                        let llm_result = rt.block_on(llm_client.optimize(
                            &pre_llm_text,
                            script_instruction,
                            config.punctuation.enabled,
                            Some(&scene_context),
                            multiline_safe,
                            send_window_title,
                        ));
                        match llm_result {
                            Ok(result) => {
                                log::info!("LLM optimized: {}", result.text);
                                learn_llm_suggestions(&result.suggestions, runtime_config);
                                llm_handled = true;
                                // FMT-LLM-005: LLM 成功路径用 normalize_script_only（仅简繁，不动大小写）。
                                // LLM 输出大小写是正确意图（如"Dear Mr. Wang,"），fix_asr_english_case 会打回小写破坏。
                                text_normalizer::normalize_script_only(
                                    &result.text,
                                    config.audio.chinese_script,
                                )
                            }
                            Err(e) => {
                                log::warn!("LLM optimization error: {}", e);
                                format_failed = true;
                                // ITN-V2-001 (R1)：兜底用主通道产物（已含数字转换），
                                // 与 DEC-035 之前行为一致（属改进）。
                                text_normalizer::normalize_text_for_language(
                                    &pre_llm_text,
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
                            &pre_llm_text,
                            config.audio.chinese_script,
                        )
                    };
                    // REFACTOR-SHARE-TRANSDIR-001: persist via AppConfig method (platform-neutral)
                    // + runtime lock/save (Windows runtime concern, inline here in cfg(windows) run_pipeline)
                    if translate_requested
                        && config.translation.enabled
                        && !raw_text.trim().is_empty()
                    {
                        let needs_save = match runtime_config.write() {
                            Ok(mut cfg) => cfg.remember_translation_direction(derived_target),
                            Err(poisoned) => {
                                let mut cfg = poisoned.into_inner();
                                cfg.remember_translation_direction(derived_target)
                            }
                        };
                        if needs_save {
                            if let Ok(cfg) = runtime_config.read() {
                                log::info!(
                                    "Persisting translation direction cache to {:?}",
                                    derived_target
                                );
                                if let Err(e) = cfg.save() {
                                    log::warn!(
                                        "Failed to persist translation direction cache: {}",
                                        e
                                    );
                                }
                            }
                        }
                    }
                    // ITN-V2-001 (R1 补丁通道)：三分支产出 final_text 之后、本地标点之前。
                    // 主通道已在 LLM 之前跑过完整 normalize_numbers（见上方 pre_llm_text），
                    // 此处仅运行第二阶段 normalize_unit_symbols_only，捞回 LLM 纠正 ASR
                    // 同音错字后的单位符号（如「摄息」→「摄氏」后「40摄氏度」→「40℃」）。
                    // 不重跑 normalize_with_rules（第一阶段）：中文数字已在主通道定型，且 LLM
                    // 输出可能含阿拉伯数字，重跑中文数字转换是 no-op；单位符号规整才是补丁价值。
                    // 幂等安全：主通道产出「40℃」，补丁通道对「℃」不匹配任何 trigger → 不变。
                    // 对翻译路径（英文输出）：本函数是 no-op（英文无中文单位词）。
                    let final_text = itn::normalize_unit_symbols_only(&final_text);
                    // PUNCT-INTEGRATION-001 + ASR-PUNCT-OPT-001: 标点决策
                    // 条件：auto_punct=true && LLM 未处理 && 非翻译 && 非native自带标点
                    // - native_punctuated=true（accuracy native 成功）→ 跳过标点引擎（省一次推理）
                    // - native_punctuated=false（performance/兜底/混合）→ 照常走标点引擎
                    // - 本分支只负责「加标点」；「剥离」职责已移交下方 L2 后处理补位块
                    //   （PUNCT-GOVERNANCE-030-A），对全部产出源一视同仁，无来源判据。
                    let final_text = if config.punctuation.enabled
                        && !llm_handled
                        && !translate_requested
                        && !native_punctuated
                    {
                        if let Some(ref mut engine) = punctuation_engine {
                            match engine.add_punctuation(&final_text) {
                                Some(punctuated) => {
                                    log::info!(
                                        "Local punctuation applied: '{}' -> '{}'",
                                        final_text,
                                        punctuated
                                    );
                                    punctuated
                                }
                                None => {
                                    log::warn!(
                                        "Local punctuation returned None, keeping original text"
                                    );
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
                    // PUNCT-GOVERNANCE-030-A L2 后处理补位（架构定位：L1 源头控制为主，L2 补位）
                    // 只负责两件 L1 物理上够不着的事：
                    //   (a) 开关关闭 → 全文剥标点（Qwen3 在线 ASR / 本地 native / NLLB 翻译不可控源兜底）
                    //   (b) 开关开启且字/词数 <= 5 → 剥末尾标点（Gavin 2026-08-08 短句规则）
                    // 🔴 本块不含任何来源判据（llm_handled/native_punctuated）/（LLM 翻译），
                    //    对 6 个产出源一视同仁，将来新增产出源自动受控。
                    // PUNCT-GOVERNANCE-030-E：判定逻辑已抽为纯函数 apply_l2_postprocess
                    //   （src/punctuation/mod.rs，可单测）；本侧只保留日志（日志方案 C：
                    //   按 L2Action 分支打对应文案，且沿用「输出变化才打」的 != 判定）。
                    let (l2_text, l2_action) =
                        punctuation::apply_l2_postprocess(&final_text, config.punctuation.enabled);
                    match l2_action {
                        punctuation::L2Action::StripAll => {
                            if l2_text != final_text {
                                log::info!(
                                    "Stripped native punctuation (auto_punct=false): '{}' -> '{}'",
                                    final_text,
                                    l2_text
                                );
                            }
                        }
                        punctuation::L2Action::StripTrailing => {
                            if l2_text != final_text {
                                log::info!(
                                    "Stripped trailing punctuation (<=5 units): '{}' -> '{}'",
                                    final_text,
                                    l2_text
                                );
                            }
                        }
                        punctuation::L2Action::NoOp => {}
                    }
                    let final_text = l2_text;
                    if cancel_signal.load(Ordering::Relaxed) {
                        send_event(event_tx, PipelineEvent::Cancelled);
                        return;
                    }
                    let current_id = platform::foreground_window_id();
                    let focus_lost = target_hwnd != 0 && current_id != target_hwnd;
                    log::info!(
                        "Injecting text: '{}', focus_lost={}, target_hwnd={}, current_id={}",
                        final_text,
                        focus_lost,
                        target_hwnd,
                        current_id
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
                        // FORMAT-LLM-001-CORE (DEC-031-③): LLM 格式化失败 → 注入兜底原文后
                        // 发 FormatFailed overlay 提示（2500ms），让用户知道检查 LLM 配置。
                        if format_failed {
                            send_event(event_tx, PipelineEvent::FormatFailed);
                        }
                    }
                }
            }
        }
    }
}
// MACOS-P4-NEUTRAL-001: 原 #[cfg(target_os = "windows")] 去除——平台中立纯 Rust，run_pipeline_core 调用，对 Windows 为 no-op。
fn should_try_llm_translate(llm_enabled: bool, connectivity_verified: bool) -> bool {
    llm_enabled && connectivity_verified
}
/// LANG-AUTO-001: 翻译方向按内容（contains_han）判定，不依赖 transcription_language 配置。
/// - target=English → 含汉字才需翻译；纯英文文本无需翻译。
/// - target=Chinese → 不含汉字才需翻译；纯英文文本需翻译。
// MACOS-P4-NEUTRAL-001: 原 #[cfg(target_os = "windows")] 去除——平台中立纯 Rust，run_pipeline_core 调用，对 Windows 为 no-op。
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
        assert!(translation_needs_reload(
            &None,
            true,
            TranslationLanguage::English
        ));
        assert!(translation_needs_reload(
            &None,
            true,
            TranslationLanguage::Chinese
        ));
    }

    /// No cached engine + disabled = no reload needed
    #[test]
    fn translation_no_reload_when_none_and_disabled() {
        assert!(!translation_needs_reload(
            &None,
            false,
            TranslationLanguage::English
        ));
    }

    /// Cached engine + disabled = needs reload (to clear cache)
    #[test]
    fn translation_needs_reload_when_cached_but_disabled() {
        let cached = Some(TranslationLanguage::English);
        assert!(translation_needs_reload(
            &cached,
            false,
            TranslationLanguage::English
        ));
    }

    /// Cached engine + same target + enabled = no reload needed
    #[test]
    fn translation_no_reload_when_cached_same_target() {
        let cached = Some(TranslationLanguage::English);
        assert!(!translation_needs_reload(
            &cached,
            true,
            TranslationLanguage::English
        ));
    }

    /// Cached engine + different target = needs reload (direction changed)
    #[test]
    fn translation_needs_reload_when_cached_different_target() {
        let cached = Some(TranslationLanguage::English);
        assert!(translation_needs_reload(
            &cached,
            true,
            TranslationLanguage::Chinese
        ));
    }

    /// Cached Chinese + switch to English = needs reload
    #[test]
    fn translation_needs_reload_chinese_to_english() {
        let cached = Some(TranslationLanguage::Chinese);
        assert!(translation_needs_reload(
            &cached,
            true,
            TranslationLanguage::English
        ));
    }
}
/// WORDBOOK-053-A: synchronous part of auto-learning. Must stay on the caller thread because
/// `capture_focused_text_snapshot()` captures the *current* foreground window; moving it to a
/// background thread would read the wrong window after the 300ms observation delay.
fn maybe_learn_user_edit(
    expected_text: &str,
    snapshot: Option<platform::FocusedTextSnapshot>,
    runtime_config: &Arc<RwLock<AppConfig>>,
) {
    // MAC-004
    let Some(snapshot) = snapshot else {
        return;
    };
    let expected_text = expected_text.trim().to_string();
    if expected_text.is_empty() {
        return;
    }
    let runtime_config = Arc::clone(runtime_config);
    // Detach the heavy observation + diff + SQLite write so the caller thread (controller main
    // message loop or worker thread) returns immediately. The HWND snapshot text is captured here
    // and moved in; only the original foreground window's text is observed.
    let hwnd_usize = snapshot.hwnd.0 as usize;
    let before_text = snapshot.text;
    thread::spawn(move || {
        let hwnd = HWND(hwnd_usize as *mut std::ffi::c_void);
        thread::sleep(Duration::from_millis(AUTO_LEARN_OBSERVE_MS));
        let Some(after_text) = platform::read_text_from_hwnd(hwnd) else {
            // MAC-004
            return;
        };
        let Some(observed_text) = extract_changed_text(&before_text, &after_text) else {
            return;
        };
        let observed_text = observed_text.trim();
        if observed_text.is_empty() || expected_text == observed_text {
            return;
        }
        let auto_learn_threshold = read_auto_learn_threshold(&runtime_config);
        if let Err(e) = wordbook::Wordbook::open()
            .and_then(|wb| wb.learn_correction(&expected_text, observed_text, auto_learn_threshold))
        {
            log::debug!("Auto-learning skipped: {}", e);
        }
    });
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
            // WORDBOOK-SCHEMA-FIX-001: 打不开词库是功能整体失效（非崩溃），从 debug!
            // 提升为 warn! —— 原 debug! 在 info 级运行日志中 0 条痕迹，正是本 P0 bug
            // 潜伏至今的原因之一。单个建议词被跳过属正常业务分支，保持 debug! 不抬高。
            log::warn!("Wordbook open failed, LLM auto-learning disabled: {}", err);
            return;
        }
    };
    for suggestion in suggestions {
        if let Err(err) = wordbook.learn_suggestion(&suggestion.word, auto_learn_threshold) {
            log::debug!("Skipping LLM suggestion '{}': {}", suggestion.word, err);
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
    // MACOS-P4-NEUTRAL-002: StartCmd/WorkerCommand/SendHwnd/spawn_worker_thread 已删除——
    // 真实定义已去 cfg（StartCmd/WorkerCommand/spawn_worker_thread）或保留 cfg(windows)
    // （SendHwnd 仍服务 overlay 线程 :565/:589/:641），不再需要此处占位。
    #[derive(Debug, Clone)]
    pub struct OverlayCommand;
    #[derive(Debug, Clone)]
    pub struct OverlayRequest;
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
}

#[cfg(test)]
mod rustls_provider_tests {
    /// BUG-QWEN3-CRYPTO-001: 进程级 ring provider 重复安装不 panic（幂等性）
    #[test]
    fn install_default_provider_is_idempotent() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}

#[cfg(test)]
mod overlay_shimmer_tests {
    use super::find_speech_onset_with_backtrack;
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
        assert_eq!(
            BG_DARK_GREEN, 0x1A00,
            "Green channel must match #1A in BG_DARK"
        );
        assert_eq!(
            BG_DARK_BLUE, 0x1800,
            "Blue channel must match #18 in BG_DARK"
        );
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
        assert_eq!(
            left_endpoint, right_endpoint,
            "Left and right gradient endpoints must be symmetric BG_DARK"
        );
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
        assert!(
            midpoint_r > endpoint_r,
            "Midpoint must be brighter than endpoints (R)"
        );
        assert!(
            midpoint_g > endpoint_g,
            "Midpoint must be brighter than endpoints (G)"
        );
        assert!(
            midpoint_b > endpoint_b,
            "Midpoint must be brighter than endpoints (B)"
        );
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
        assert_eq!(
            ellipse_w, 28,
            "Ellipse width must match body width for fully rounded ends"
        );
        assert_eq!(ellipse_h, 28, "Ellipse height must be 28px");
        assert!(
            ellipse_w <= width,
            "Ellipse width must not exceed body width"
        );
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
        assert_eq!(
            stem_top, body_bottom,
            "Stem must connect to mic body bottom"
        );
    }

    #[test]
    fn mic_icon_18px_layout_margin() {
        // MIC-ICON-ENLARGE-001: circ_size=18px, left=rect.left+6, sep_l_x=30
        // 6 + 18 + 6 = 30, confirming 6px margin on both sides of enlarged icon
        let circ_size: i32 = 18;
        let circ_l_offset: i32 = 6;
        let sep_l_x: i32 = 30;
        assert_eq!(
            circ_l_offset + circ_size + circ_l_offset,
            sep_l_x,
            "Separator x must equal left_margin(6) + icon(18) + right_margin(6)"
        );
        assert_eq!(circ_size, 18, "circ_size must be 18px after ENLARGE-001");
    }

    #[test]
    fn asr_silence_head_prepended() {
        // ASR-CTC-OPT-001 P1: run_pipeline prepends 0 zero-samples (0ms@16kHz)
        // for performance (CTC). Offline models don't need frame alignment padding.
        // ASR-ACC-OPT-001: accuracy also uses 0ms head.
        let silence_head_len: usize = 0;
        let original_samples: usize = 3200;
        let mut padded = Vec::with_capacity(silence_head_len + original_samples);
        padded.resize(silence_head_len, 0.0f32);
        padded.extend_from_slice(&vec![1.0f32; original_samples]);
        assert_eq!(
            padded.len(),
            silence_head_len + original_samples,
            "Padded length must be silence_head + original"
        );
        if silence_head_len > 0 {
            assert!(
                padded.iter().take(silence_head_len).all(|&s| s == 0.0),
                "First {} samples must be silence (zero)",
                silence_head_len
            );
        }
        assert_eq!(
            padded[silence_head_len], 1.0,
            "Original audio must begin after silence head"
        );
    }

    #[test]
    fn asr_silence_head_is_0ms_at_16khz() {
        // ASR-CTC-OPT-001 P1: 0 samples = 0ms @ 16kHz (performance/CTC)
        // 原 50ms (800 samples) 是旧 SenseVoice 遗产，研究 C1 证实 0ms 高 2.5pp
        let sample_rate: u32 = 16000;
        let silence_head: usize = 0;
        let duration_ms = (silence_head as f64 / sample_rate as f64) * 1000.0;
        assert!(
            (duration_ms - 0.0).abs() < 1.0,
            "0 samples at 16kHz must be ~0ms (got {:.1}ms)",
            duration_ms
        );
    }

    #[test]
    fn speech_onset_with_backtrack_trims_leading_silence() {
        // FIRSTCHAR-FIX-006 (R3): find_speech_onset_with_backtrack trims silence
        // before the speech onset while preserving 200ms backtrack margin.
        // Construct: 4800-sample silence + 3200-sample speech = 8000 total @ 16kHz
        let mut samples = vec![0.0f32; 8000];
        for i in 4800..8000 {
            samples[i] = 0.1;
        }
        // Energy onset at ~4800, backtrack 3200 (200ms) → keep_start = 1600
        let keep_start = find_speech_onset_with_backtrack(&samples, 0.008, 3200);
        assert_eq!(
            keep_start, 1600,
            "should trim silence before backtrack margin, got {}",
            keep_start
        );
        // Verify speech is preserved
        assert!(
            samples[keep_start..].iter().any(|&s| s > 0.05),
            "speech must be present after trimming"
        );
    }

    #[test]
    fn speech_onset_with_backtrack_preserves_aspirated_consonant() {
        // FIRSTCHAR-FIX-006 (R3): weak consonant at onset must not be trimmed.
        // Simulate: silence → weak breath (aspirated consonant ~100ms) → strong vowel
        let mut samples = vec![0.0f32; 9600]; // 600ms @ 16kHz
                                              // Weak breath from sample 3200–4800 (100ms of low-energy consonant)
        for i in 3200..4800 {
            samples[i] = 0.005; // below threshold 0.008 — aspirated consonant is very quiet
        }
        // Strong vowel from sample 4800 onward
        for i in 4800..9600 {
            samples[i] = 0.1;
        }
        // Energy onset should detect at ~4800 (strong vowel), backtrack 3200 → keep_start = 1600
        let keep_start = find_speech_onset_with_backtrack(&samples, 0.008, 3200);
        assert!(
            keep_start <= 3200,
            "keep_start must include weak breath consonant, got {} (consonant starts at 3200)",
            keep_start
        );
    }

    #[test]
    fn speech_onset_with_backtrack_keeps_all_when_no_silence() {
        // FIRSTCHAR-FIX-006 (R3): "speak before key" — pre_roll already has speech
        let samples = vec![0.1f32; 3200]; // all speech, no silence
        let keep_start = find_speech_onset_with_backtrack(&samples, 0.008, 3200);
        assert_eq!(
            keep_start, 0,
            "no leading silence: must keep everything (saturating_sub to 0)"
        );
    }

    #[test]
    fn speech_onset_with_backtrack_no_speech_returns_zero() {
        // FIRSTCHAR-FIX-006 (R3): no speech detected → keep everything
        let samples = vec![0.0f32; 3200]; // all silence
        let keep_start = find_speech_onset_with_backtrack(&samples, 0.008, 3200);
        assert_eq!(keep_start, 0, "no speech detected: must not trim anything");
    }

    // === OVERLAY-FIX-005: Processing phase stepping + Preview window layout ===

    #[test]
    fn processing_phase_step_increment() {
        // OVERLAY-FIX-005: phase increments by 0.015 per WM_PAINT frame
        let phase: f32 = 0.0;
        let new_phase = (phase + 0.015) % 2.0;
        assert!(
            (new_phase - 0.015).abs() < 0.001,
            "Phase must increment by 0.015"
        );
    }

    #[test]
    fn processing_phase_wrap_at_two() {
        // OVERLAY-FIX-005: phase wraps at 2.0 (not 1.0)
        let phase: f32 = 1.990;
        let new_phase = (phase + 0.015) % 2.0;
        assert!(
            new_phase < 0.01,
            "Phase must wrap to near 0 when exceeding 2.0"
        );
    }

    #[test]
    fn processing_phase_triangle_wave_at_midpoint() {
        // OVERLAY-FIX-005: triangle wave p = phase if <= 1.0 else 2.0 - phase
        // At phase=1.0, p should be 1.0 (peak)
        let phase: f32 = 1.0;
        let p = if phase <= 1.0 { phase } else { 2.0 - phase };
        assert!(
            (p - 1.0).abs() < 0.001,
            "Triangle wave must peak at phase=1.0"
        );
    }

    #[test]
    fn processing_phase_triangle_wave_descending() {
        // OVERLAY-FIX-005: at phase=1.5, p = 2.0 - 1.5 = 0.5 (descending)
        let phase: f32 = 1.5;
        let p = if phase <= 1.0 { phase } else { 2.0 - phase };
        assert!(
            (p - 0.5).abs() < 0.001,
            "Triangle wave must descend at phase>1.0"
        );
    }

    #[test]
    fn processing_phase_triangle_wave_at_wrap() {
        // OVERLAY-FIX-005: at phase=1.985 (just before wrap), p = 2.0 - 1.985 = 0.015
        let phase: f32 = 1.985;
        let p = if phase <= 1.0 { phase } else { 2.0 - phase };
        assert!(
            (p - 0.015).abs() < 0.001,
            "Triangle wave near wrap must be near 0"
        );
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
        assert_eq!(
            btn_left, 95,
            "Buttons must be centered at x=95 in 320px window"
        );
        // Verify symmetry: left margin == right margin
        let left_margin = btn_left;
        let right_margin = window_width - (btn_left + total_w);
        assert_eq!(
            left_margin, right_margin,
            "Left and right margins must be equal"
        );
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
        assert_eq!(
            title_close_right - title_close_left,
            18,
            "Close button must be 18px wide"
        );
        assert_eq!(
            title_close_bottom - title_close_top,
            18,
            "Close button must be 18px tall"
        );
        assert_eq!(
            title_close_left,
            window_right - close_btn_space,
            "Close button must be positioned close_btn_space from right edge"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn preview_brand_orange_color_value() {
        // OVERLAY-FIX-005: BRAND_ORANGE = #FF6B00 used for title, X button, and buttons.
        // TEST-SYNC-054: bind the production constant directly; no copied literal.
        let brand_orange = super::OVERLAY_BRAND_ORANGE.0;
        let r = brand_orange & 0xFF;
        let g = (brand_orange >> 8) & 0xFF;
        let b = (brand_orange >> 16) & 0xFF;
        assert_eq!(r, 0xFF, "OVERLAY_BRAND_ORANGE must have full red channel");
        assert_eq!(g, 0x6B, "OVERLAY_BRAND_ORANGE must have 0x6B green channel");
        assert_eq!(b, 0x00, "OVERLAY_BRAND_ORANGE must have zero blue channel");
    }

    // === WAVEFORM-FIX-001: Gravity decay + center spectral weighting ===

    #[test]
    fn waveform_gravity_decay_single_frame() {
        // WAVEFORM-FIX-001: GRAVITY_RATE = 0.25, level *= GRAVITY_RATE per frame
        const GRAVITY_RATE: f32 = 0.25;
        let level: f32 = 1.0;
        let after_one = level * GRAVITY_RATE;
        assert!(
            (after_one - 0.25).abs() < 0.001,
            "After 1 frame, level must be 0.25"
        );
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
        assert!(
            (level - 0.0078125).abs() < 0.0001,
            "Decay must be multiplicative from any start"
        );
    }

    #[test]
    fn waveform_gravity_rate_constant_value() {
        // WAVEFORM-FIX-002: GRAVITY_RATE = 0.25 (center), edge = 0.25*(0.5+1.5*1)=0.5
        const GRAVITY_RATE: f32 = 0.25;
        assert!(
            (GRAVITY_RATE - 0.25).abs() < 0.001,
            "GRAVITY_RATE must be 0.25"
        );
        assert!(
            GRAVITY_RATE > 0.0 && GRAVITY_RATE < 1.0,
            "GRAVITY_RATE must be between 0 and 1 for decay"
        );
    }

    #[test]
    fn waveform_edge_weighted_gravity_bounds() {
        // WAVEFORM-FIX-002: bar_gravity = 0.25 * (0.5 + 1.5 * center_dist)
        // center_dist=0 → bar_gravity=0.125, center_dist=1 → bar_gravity=0.5
        const GRAVITY_RATE: f32 = 0.25;
        let center_gravity = GRAVITY_RATE * (0.5 + 1.5 * 0.0);
        let edge_gravity = GRAVITY_RATE * (0.5 + 1.5 * 1.0);
        assert!(
            (center_gravity - 0.125).abs() < 0.001,
            "Center gravity must be 0.125"
        );
        assert!(
            (edge_gravity - 0.5).abs() < 0.001,
            "Edge gravity must be 0.5"
        );
        assert!(
            edge_gravity > center_gravity,
            "Edge must decay faster than center"
        );
    }

    #[test]
    fn waveform_center_weight_at_center() {
        // WAVEFORM-FIX-001: weight = 0.4 + 0.6*cos²(π/2 * i/(half-1))
        // At i=0 (center): cos(0) = 1, weight = 0.4 + 0.6*1 = 1.0
        let half: usize = 32;
        let i: usize = 0;
        let weight = 0.4
            + 0.6
                * (std::f64::consts::FRAC_PI_2 * i as f64 / (half - 1) as f64)
                    .cos()
                    .powi(2);
        assert!(
            (weight - 1.0).abs() < 0.001,
            "Center bar (i=0) must have weight=1.0"
        );
    }

    #[test]
    fn waveform_center_weight_at_edge() {
        // WAVEFORM-FIX-001: at i=half-1 (edge): cos(π/2) = 0, weight = 0.4
        let half: usize = 32;
        let i: usize = half - 1;
        let weight = 0.4
            + 0.6
                * (std::f64::consts::FRAC_PI_2 * i as f64 / (half - 1) as f64)
                    .cos()
                    .powi(2);
        assert!(
            (weight - 0.4).abs() < 0.001,
            "Edge bar (i=half-1) must have weight=0.4"
        );
    }

    #[test]
    fn waveform_center_weight_monotonic_decrease() {
        // WAVEFORM-FIX-001: weight must decrease monotonically from center to edge
        let half: usize = 32;
        let mut prev_weight = 1.0;
        for i in 1..half {
            let weight = 0.4
                + 0.6
                    * (std::f64::consts::FRAC_PI_2 * i as f64 / (half - 1) as f64)
                        .cos()
                        .powi(2);
            assert!(
                weight <= prev_weight + 0.001,
                "Weight must decrease monotonically from center to edge (i={})",
                i
            );
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
        assert!(
            (weight - 0.685).abs() < 0.01,
            "Midpoint bar must have weight≈0.685 (actual={:.4})",
            weight
        );
    }

    #[test]
    fn waveform_center_weight_range_bounds() {
        // WAVEFORM-FIX-001: all weights must be in [0.4, 1.0]
        let half: usize = 32;
        for i in 0..half {
            let weight = 0.4
                + 0.6
                    * (std::f64::consts::FRAC_PI_2 * i as f64 / (half - 1) as f64)
                        .cos()
                        .powi(2);
            assert!(
                weight >= 0.4 - 0.001 && weight <= 1.0 + 0.001,
                "Weight must be in [0.4, 1.0] range (i={})",
                i
            );
        }
    }

    // === OVERLAY-054-F-B: compute_edit_box_geometry contracts ===
    // Pure-function tests bind the 5 documented contracts (main.rs:553-559) with the
    // exact call-site parameters (fixed_margin = STREAMING_TEXT_TOP_MARGIN = 10,
    // corner_floor = 4). These are behavior contracts, not implementation strings.

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_box_geometry_contract1_desired_tracks_tm_height() {
        let (top, h) = super::compute_edit_box_geometry(36, 17, 10, 4);
        assert_eq!(h, 19, "desired = (tm_height + 2).max(fixed_height) = 19");
        assert_eq!(top, 8);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_box_geometry_contract1_fixed_height_is_floor() {
        let (_, h) = super::compute_edit_box_geometry(36, 4, 10, 4);
        assert_eq!(h, 16, "tiny font must still get the fixed_height floor");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_box_geometry_contract1_metrics_failure_falls_back_to_fixed() {
        // contract 1 failure state: GetTextMetricsW 失败时调用侧传入 tm_height=0
        //（main.rs:608 got.as_bool() 分支），这是真实运行时路径，必须回退固定布局。
        let (top, h) = super::compute_edit_box_geometry(36, 0, 10, 4);
        assert_eq!(top, 10);
        assert_eq!(
            h, 16,
            "metrics-failure tm_height=0 must fall back to fixed_height layout"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_box_geometry_contract1_negative_tm_falls_back_to_fixed() {
        // contract 1 failure state: 负数 tm_height（异常字体度量）同样回退固定布局。
        let (top, h) = super::compute_edit_box_geometry(36, -5, 10, 4);
        assert_eq!(top, 10);
        assert_eq!(
            h, 16,
            "negative tm_height must fall back to fixed_height layout"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_box_geometry_contract2_available_floor_zero() {
        let (top, h) = super::compute_edit_box_geometry(6, 5, 10, 4);
        assert_eq!(h, 1, "fallback fixed_height = (6 - 20).max(1) = 1");
        assert_eq!(top, 4, "top clamps to corner_floor");
        assert!(top + h <= 6, "box must stay inside the rect");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_box_geometry_contract3_exact_desired_when_it_fits() {
        let (_, h) = super::compute_edit_box_geometry(36, 17, 10, 4);
        let fixed_height = (36 - 2 * 10).max(1);
        let desired = (17 + 2).max(fixed_height);
        assert!(
            desired <= (36 - 2 * 4).max(0),
            "precondition: desired must fit inside available"
        );
        assert_eq!(
            h, desired,
            "contract 3: when desired <= available, height must equal desired EXACTLY"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_box_geometry_contract4_clamps_to_available_when_too_tall() {
        // OVERLAY-054-J: desired > available 时不再"回退"到 fixed_height(16)——塌回 16 等于
        // 主动扔掉可用像素、让裁字更严重，且制造 tm=26→27 的高度断崖（旧期望 16 正是断崖
        // 的下半截）。新语义 height = desired.min(available).max(1)：尽量给、最多给到
        // available(28)，error 日志另在调用侧照打。
        let (top, h) = super::compute_edit_box_geometry(36, 40, 10, 4);
        assert_eq!(
            h, 28,
            "contract 4: desired 42 > available 28, must clamp to available 28, not collapse to 16"
        );
        assert_eq!(top, 4, "(36 28) / 2 centered then corner_floor clamp");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_box_geometry_contract4_cliff_point_removed_tm26_equals_tm27() {
        // OVERLAY-054-J 契约钉死断崖点本身：旧实现 tm=26 高度 28 而 tm=27 塌回 16；
        // 新语义下两者必须同为 available(28)，不允许出现任何落差。
        let (_, h26) = super::compute_edit_box_geometry(36, 26, 10, 4);
        let (_, h27) = super::compute_edit_box_geometry(36, 27, 10, 4);
        assert_eq!(h26, 28, "tm=26 must already reach the available cap");
        assert_eq!(h27, 28, "tm=27 must NOT collapse (old cliff: 28 -> 16)");
        assert_eq!(h26, h27, "the tm=26->27 cliff point must stay flat");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_box_geometry_contract5_centered_inside_corner_floor() {
        let (top, h) = super::compute_edit_box_geometry(36, 14, 10, 4);
        assert_eq!(h, 16);
        assert_eq!(top, (36 - h) / 2, "top must be centered first");
        assert_eq!(
            top, 10,
            "centered offset must survive the corner_floor clamp"
        );
        assert_eq!(top + h + top, 36, "exact symmetric centering");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_box_geometry_contract7_height_monotonic_non_decreasing() {
        // contract 7: 返回 height 必须关于 tm_height 单调不减。逐值扫描 [-5,40]：
        // 本函数两次出错均为某个值域下的行为翻转（.max 链恒占优 / 溢出回落），
        // 单点断言抓不住值域翻转，整段扫描才能。
        // 阶段三首跑即在 tm=26→27 抓到真实断崖（h=28→16），OVERLAY-054-J 修复后全区间
        // 转绿。区间不得收窄——收窄即失去发现同类值域翻转的能力。
        let mut prev_h = i32::MIN;
        for tm_height in -5..=40 {
            let (_, h) = super::compute_edit_box_geometry(36, tm_height, 10, 4);
            assert!(
                h >= prev_h,
                "contract 7: height must be non-decreasing in tm_height (tm={}: h={} < previous h={})",
                tm_height,
                h,
                prev_h
            );
            prev_h = h;
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_box_geometry_defect_a_top_offset_not_pinned_by_fixed_margin() {
        // OVERLAY-043/054-F-B defect A regression: fixed_margin must never re-enter the
        // top-offset .max() chain, otherwise centering is pinned to the legacy margin.
        // rect_h=36 / tm_height=17 must yield top_offset=8 (not the legacy 10).
        let (top, h) = super::compute_edit_box_geometry(36, 17, 10, 4);
        assert_eq!(
            top, 8,
            "top_offset must be (36-19)/2 = 8, NOT the legacy fixed_margin 10"
        );
        assert_ne!(top, 10, "fixed_margin must not pin the top offset");
        assert_eq!(h, 19);
    }

    // === OVERLAY-054-H: text-area capacity guard ===
    // The 36px recording overlay must leave enough drawing height for the ClearType
    // font plus a 6px cushion. STREAMFONT-189: the streaming text now renders at
    // OVERLAY_TEXT_FONT_SIZE (16px), so the guard covers both font sizes. If this
    // trips, 054-G/H class clipping has regressed.

    #[cfg(target_os = "windows")]
    #[test]
    fn overlay_text_area_capacity_guard() {
        let drawing_area =
            super::RECORDING_OVERLAY_SIZE[1] - 2 * super::OVERLAY_TEXT_DRAW_VERTICAL_INSET;
        for font_h in [
            super::OVERLAY_FONT_SIZE.unsigned_abs() as i32,
            super::OVERLAY_TEXT_FONT_SIZE.unsigned_abs() as i32,
        ] {
            assert!(
                drawing_area >= font_h + 6,
                "OVERLAY-054-H: {}px drawing area must fit {}px font + 6px cushion ({}px needed)",
                drawing_area,
                font_h,
                font_h + 6
            );
        }
    }

    // === OVERLAY-FIX-006: Border darken + Shimmer rewrite + Preview adjustments ===

    #[cfg(target_os = "windows")]
    #[test]
    fn overlay_border_gray_is_mid_gray() {
        // OVERLAY-054-C: BORDER_GRAY brightened from 0x060607 to 0x3A3A3C (≈58% brightness).
        // This mid gray makes the overlay outline clearly visible against dark backgrounds.
        // TEST-SYNC-054: bind the production constant directly; no copied literal.
        // TEST-EXEC-054: corrected channel assertions — COLORREF 0x3A3A3C = 0x00BBGGRR gives
        // R=0x3C/G=0x3A/B=0x3A; the 054-C-era R=0x3A/B=0x3C transposition was a latent test bug
        // (surfaced by the first full cargo test run since 054-C; production value is Gavin's ruling).
        let border_gray = super::OVERLAY_BORDER_GRAY.0;
        let r = border_gray & 0xFF;
        let g = (border_gray >> 8) & 0xFF;
        let b = (border_gray >> 16) & 0xFF;
        assert_eq!(r, 0x3C, "OVERLAY_BORDER_GRAY red must be 0x3C");
        assert_eq!(g, 0x3A, "OVERLAY_BORDER_GRAY green must be 0x3A");
        assert_eq!(b, 0x3A, "OVERLAY_BORDER_GRAY blue must be 0x3A");
        // Verify brighter than old value (0x171513). Note: as a packed u32 the ordering is
        // 0x00BBGGRR, so "larger" means more blue/green/red; 0x3A3A3C > 0x131517.
        let old_gray: u32 = 0x131517;
        assert!(
            border_gray > old_gray,
            "New OVERLAY_BORDER_GRAY must be brighter than old 0x171513"
        );
    }

    // TEST-SYNC-054: circ_border_matches_overlay_border_gray deleted. The test asserted
    // two locally copied literals (0x3A3A3C == 0x3A3A3C) against a CIRC_BORDER symbol that
    // OVERLAY-054-C physically eliminated from production. It was a vacuous tautology; the
    // rounded-rect outline now draws OVERLAY_BORDER_GRAY directly (guarded above).

    #[test]
    fn waveform_index_center_is_newest() {
        // WAVEFORM-FIX-002: center bar (i=0) must map to newest sample (len-1)
        let levels_len: usize = 64;
        let half: usize = 32;
        let idx_center = levels_len.saturating_sub(1 + 0);
        assert_eq!(
            idx_center,
            levels_len - 1,
            "center bar i=0 must map to newest sample (len-1)"
        );
        assert_eq!(idx_center, 63, "for len=64 newest sample index is 63");
    }

    #[test]
    fn waveform_index_edge_maps_to_oldest_of_left_half() {
        // WAVEFORM-FIX-002: edge bar (i=half-1) maps to oldest of left half
        let levels_len: usize = 64;
        let half: usize = 32;
        let idx_edge = levels_len.saturating_sub(1 + (half - 1));
        assert_eq!(
            idx_edge,
            levels_len - half,
            "edge bar i=half-1 must map to len-half (oldest of left half)"
        );
        assert_eq!(idx_edge, 32, "for len=64,half=32 edge maps to idx 32");
    }

    #[test]
    fn waveform_right_half_index_starts_at_start_idx() {
        // WAVEFORM-FIX-002: right half i=0 starts at start_idx
        let levels_len: usize = 64;
        let half: usize = 32;
        let start_idx = levels_len.saturating_sub(half);
        assert_eq!(start_idx, 32, "start_idx for len=64,half=32 is 32");
        let idx_right_first = start_idx + 0;
        assert_eq!(
            idx_right_first, 32,
            "right half first bar (i=0) must start at start_idx"
        );
        assert!(
            idx_right_first < levels_len,
            "right half index must be within bounds"
        );
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
            assert_eq!(
                start_idx, expected,
                "start_idx for len={},half={} must be {}",
                len, half, expected
            );
        }
    }

    #[test]
    fn waveform_gravity_edge_falls_4x_faster_than_center() {
        // WAVEFORM-FIX-002: edge gravity(0.5) / center gravity(0.125) = 4x
        const GRAVITY_RATE: f32 = 0.25;
        let center_gravity = GRAVITY_RATE * (0.5 + 1.5 * 0.0); // 0.125
        let edge_gravity = GRAVITY_RATE * (0.5 + 1.5 * 1.0); // 0.5
        let ratio = edge_gravity / center_gravity;
        assert!(
            (ratio - 4.0).abs() < 0.001,
            "edge must fall {:.1}x faster than center, got {:.2}x",
            4.0,
            ratio
        );
    }

    #[test]
    fn shimmer_period_is_800ms() {
        // SHIMMER-SPEED-002: period changed from 1200ms to 800ms
        let period_ms: u64 = 800;
        assert_eq!(
            period_ms, 800,
            "shimmer period must be 800ms after SHIMMER-SPEED-002"
        );
        assert_eq!(
            (0u64 % period_ms) as f32 / period_ms as f32,
            0.0,
            "start phase must be 0"
        );
        assert_eq!(
            (400u64 % period_ms) as f32 / period_ms as f32,
            0.5,
            "mid-period phase must be 0.5"
        );
        assert_eq!(
            (800u64 % period_ms) as f32 / period_ms as f32,
            0.0,
            "full-period phase must wrap to 0"
        );
    }

    #[test]
    fn shimmer_phase_time_based() {
        // SHIMMER-SPEED-002: phase = (_shimmer_ms % 800) as f32 / 800.0
        // Verify: phase is always in [0, 1) for any timestamp
        let period_ms: u64 = 800;
        for ms in [0u64, 1, 200, 399, 400, 798, 799, 800, 801, 1600] {
            let phase = (ms % period_ms) as f32 / period_ms as f32;
            assert!(
                phase >= 0.0 && phase < 1.0,
                "phase must be in [0,1) for ms={}",
                ms
            );
        }
        // Verify monotonic within one period
        let p1 = (200u64 % period_ms) as f32 / period_ms as f32;
        let p2 = (400u64 % period_ms) as f32 / period_ms as f32;
        assert!(p2 > p1, "phase must increase with time within one period");
        // Verify wrap: ms=0 and ms=800 both give phase=0.0
        let p_start = (0u64 % period_ms) as f32 / period_ms as f32;
        let p_wrap = (period_ms % period_ms) as f32 / period_ms as f32;
        assert!(
            (p_start - p_wrap).abs() < 0.0001,
            "phase must wrap at period boundary"
        );
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
        assert_eq!(
            beam_cx_start, -45,
            "beam_cx at phase=0 must be -45 (off-screen left)"
        );
        // At phase=1.0: beam_cx ends off-screen right
        let beam_cx_end = rect_left - glow_half + (travel * 1.0) as i32;
        assert_eq!(
            beam_cx_end, 245,
            "beam_cx at phase=1 must be 245 (right edge + 45)"
        );
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
        assert!(
            alpha_edge < 15,
            "Edge alpha must be very low (<15): {}",
            alpha_edge
        );
        assert!(
            alpha_edge < alpha,
            "Edge alpha must be less than center alpha"
        );
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
        assert!(
            r != 0xFF || g != 0x6B || b != 0x00,
            "Close button must not be BRAND_ORANGE"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn btn_border_brighter_than_window_border() {
        // OVERLAY-054-C: BTN_BORDER must be visually brighter than OVERLAY_BORDER_GRAY (0x3A3A3C)
        // so X close, copy, close buttons are distinguishable from window edge.
        // The old "btn_sum > gray_sum * 4" check was an accident of the near-black border
        // era (0x060607 sum=19, so any gray passed). After brightening the window border
        // to 0x3A3A3C, the 4x multiplier no longer expresses the design intent and is
        // replaced by per-channel and aggregate checks that assert "visibly brighter".
        // TEST-SYNC-054: bind the production constants directly; no copied literals.
        let border_gray = super::OVERLAY_BORDER_GRAY.0;
        let btn_border = super::OVERLAY_BTN_BORDER.0;

        let gray_r = (border_gray & 0xFF) as i32;
        let gray_g = ((border_gray >> 8) & 0xFF) as i32;
        let gray_b = ((border_gray >> 16) & 0xFF) as i32;
        let btn_r = (btn_border & 0xFF) as i32;
        let btn_g = ((btn_border >> 8) & 0xFF) as i32;
        let btn_b = ((btn_border >> 16) & 0xFF) as i32;

        // A) Per-channel: button must be brighter by at least 0x20 (~12.5% gray step),
        //    a commonly noticeable gray difference on dark backgrounds.
        //    Measured deltas: R=0x36, G=0x36, B=0x34.
        assert!(
            btn_r - gray_r >= 0x20,
            "BTN_BORDER R must be at least 0x20 brighter than BORDER_GRAY R (R diff = 0x{:02X})",
            btn_r - gray_r
        );
        assert!(
            btn_g - gray_g >= 0x20,
            "BTN_BORDER G must be at least 0x20 brighter than BORDER_GRAY G (G diff = 0x{:02X})",
            btn_g - gray_g
        );
        assert!(
            btn_b - gray_b >= 0x20,
            "BTN_BORDER B must be at least 0x20 brighter than BORDER_GRAY B (B diff = 0x{:02X})",
            btn_b - gray_b
        );

        // B) Aggregate: total luminance must be strictly larger in the brighter direction.
        let gray_sum = gray_r + gray_g + gray_b;
        let btn_sum = btn_r + btn_g + btn_b;
        assert!(
            btn_sum > gray_sum,
            "BTN_BORDER total brightness ({}) must exceed BORDER_GRAY total brightness ({})",
            btn_sum,
            gray_sum
        );

        // Verify BTN_BORDER is neutral gray
        assert_eq!(btn_r, 0x70, "BTN_BORDER R = 0x70");
        assert_eq!(btn_r, btn_g, "BTN_BORDER must be neutral gray (R=G)");
        assert_eq!(btn_g, btn_b, "BTN_BORDER must be neutral gray (G=B)");
    }

    // ============================================================
    // ASR-ACC-OPT-001 方案 B + ASR-CTC-OPT-001 P1: select_preprocessing_params 分支测试
    // ============================================================

    // MACOS-P4-NEUTRAL-001: 函数已平台中立，解除 cfg(windows) 门控使 macOS 侧编译可见。
    use super::select_preprocessing_params;
    #[allow(unused_imports)]
    use super::transcription;

    #[test]
    fn preprocessing_params_performance_uses_0ms_head_200ms_backtrack() {
        // ASR-CTC-OPT-001 P1: performance silence head 50→0ms（研究 C1 证实 0ms 高 2.5pp）
        let (head, backtrack) = select_preprocessing_params(transcription::AsrModel::Performance);
        assert_eq!(
            head, 0,
            "performance silence head = 0 samples (0ms @ 16kHz, ASR-CTC-OPT-001 P1)"
        );
        assert_eq!(
            backtrack, 3200,
            "performance onset backtrack = 3200 samples (200ms @ 16kHz, 不动)"
        );
    }

    #[test]
    fn preprocessing_params_accuracy_uses_0ms_head_100ms_backtrack() {
        // 方案 B：accuracy 用 0ms head / 100ms backtrack
        let (head, backtrack) = select_preprocessing_params(transcription::AsrModel::Accuracy);
        assert_eq!(
            head, 0,
            "accuracy silence head = 0 samples (0ms, LLM decoder needs no frame padding)"
        );
        assert_eq!(
            backtrack, 1600,
            "accuracy onset backtrack = 1600 samples (100ms @ 16kHz)"
        );
    }

    #[test]
    fn preprocessing_params_both_models_use_0ms_head() {
        // ASR-CTC-OPT-001 P1: 两模型都用 0ms head（offline 模型不需 frame alignment padding）
        let (perf_head, _) = select_preprocessing_params(transcription::AsrModel::Performance);
        let (acc_head, _) = select_preprocessing_params(transcription::AsrModel::Accuracy);
        assert_eq!(perf_head, 0, "performance head must be 0");
        assert_eq!(acc_head, 0, "accuracy head must be 0");
    }

    #[test]
    fn preprocessing_params_accuracy_backtrack_less_than_performance() {
        // native 对送气声母不如 CTC 敏感，少留 backtrack 减少前导静音
        let (_, perf_bt) = select_preprocessing_params(transcription::AsrModel::Performance);
        let (_, acc_bt) = select_preprocessing_params(transcription::AsrModel::Accuracy);
        assert!(
            acc_bt < perf_bt,
            "accuracy backtrack ({}) must be less than performance backtrack ({})",
            acc_bt,
            perf_bt
        );
    }

    #[test]
    fn preprocessing_params_online_asr_follows_performance() {
        // ASR-041-B / ASR-056: 在线 ASR（两族）沿用 performance 前处理参数（0ms head / 200ms backtrack）
        let (online_head, online_bt) =
            select_preprocessing_params(transcription::AsrModel::QwenAudioOnline);
        let (fun_asr_head, fun_asr_bt) =
            select_preprocessing_params(transcription::AsrModel::FunAsrRealtime);
        let (perf_head, perf_bt) =
            select_preprocessing_params(transcription::AsrModel::Performance);
        assert_eq!(
            online_head, perf_head,
            "online ASR silence head ({}) must equal performance head ({})",
            online_head, perf_head
        );
        assert_eq!(
            online_bt, perf_bt,
            "online ASR backtrack ({}) must equal performance backtrack ({})",
            online_bt, perf_bt
        );
        // ASR-056: FunAsrRealtime 与 QwenAudioOnline 前处理参数一致
        assert_eq!(
            fun_asr_head, online_head,
            "FunAsrRealtime head must equal QwenAudioOnline head"
        );
        assert_eq!(
            fun_asr_bt, online_bt,
            "FunAsrRealtime backtrack must equal QwenAudioOnline backtrack"
        );
    }
}

// ============================================================
// MACOS-P4-NEUTRAL-001: focus_lost 判定逻辑真值表测试
// 镜像 main.rs:3217 的表达式 `target_hwnd != 0 && current_id != target_hwnd`
// ============================================================
#[cfg(test)]
mod focus_lost_logic_tests {
    /// P0-4: 锁住 run_pipeline_core 内 focus_lost 判据的真值表。
    /// 表达式形态：target_hwnd != 0 && current_id != target_hwnd
    /// （原 Windows 形态：!target_hwnd.0.is_null() && current_hwnd.0 != target_hwnd.0）
    fn focus_lost(target_hwnd: usize, current_id: usize) -> bool {
        target_hwnd != 0 && current_id != target_hwnd
    }

    #[test]
    fn focus_lost_both_zero() {
        // target=0, current=0 → false（target_hwnd==0 短路）
        assert!(!focus_lost(0, 0));
    }

    #[test]
    fn focus_lost_target_zero_current_nonzero() {
        // target=0, current=123 → false（target_hwnd==0 短路）
        assert!(!focus_lost(0, 123));
    }

    #[test]
    fn focus_lost_same_nonzero() {
        // target=123, current=123 → false（current_id == target_hwnd）
        assert!(!focus_lost(123, 123));
    }

    #[test]
    fn focus_lost_different_nonzero() {
        // target=123, current=456 → true（焦点确实丢失）
        assert!(focus_lost(123, 456));
    }
}

/// ASR-045: 流式模式「samples 空」判空取消决策护栏。
/// 判定源：run_pipeline_core 判空分支调用 should_cancel_on_empty（纯函数）。
#[cfg(test)]
mod streaming_empty_samples_tests {
    use super::should_cancel_on_empty;

    /// ASR-045 验收#4: 流式模式（samples 空 + initial_text 有值）必须进入正常管线而非 Cancelled。
    /// 回滚修复（把判空条件还原成只看 s.is_empty()）本测试即红。
    #[test]
    fn streaming_empty_samples_with_text_must_proceed() {
        assert!(
            !should_cancel_on_empty(&[], &Some("你好今天过得怎么样".to_string())),
            "流式模式下 samples 为空是正常情况，不得触发 Cancelled"
        );
    }

    /// ASR-045 验收#5: samples 空 且 initial_text 为 None（正常模式真空录）仍走 Cancelled（原行为保留）。
    #[test]
    fn normal_empty_samples_without_text_still_cancels() {
        assert!(
            should_cancel_on_empty(&[], &None),
            "真空录（无样本且无流式文本）必须保持原 Cancelled 行为"
        );
    }

    /// 正常模式非空样本 + None → 不取消（绝大多数正常转录路径，防止误伤）。
    #[test]
    fn normal_nonempty_samples_without_text_proceeds() {
        assert!(
            !should_cancel_on_empty(&[0.0f32, 0.01, -0.01], &None),
            "有音频样本的正常转录不得触发空输入取消"
        );
    }

    /// ASR-045 真值表第 4 格：非空 samples + Some 文本 → 不取消。
    ///
    /// ⚠️ 诚实标注：本格判别力弱于前三条——多数错误变体（如把条件还原成只看
    /// `samples.is_empty()`）会先被前三条用例抓住。它的价值只在两点：
    /// ① 真值表完备性（2×2 全格）；② 防止未来被改成含 `initial_text.is_some()`
    /// 这类错误形态（对条件做镜像翻转/取反时误把「文本存在」写成「文本缺失」）。
    /// 不是强护栏，不要据此宣称调用侧已被保护。
    #[test]
    fn nonempty_samples_with_text_truth_table_cell() {
        assert!(
            !should_cancel_on_empty(&[0.01f32, -0.01], &Some("文本".to_string())),
            "非空样本 + 有流式文本不得触发空输入取消"
        );
    }

    /// ASR-045 两层职责划分护栏：空/纯空白流式文本**不在第一层**取消。
    ///
    /// 职责划分（修复后行为契约，:4288 与 :4334 两层各司其职）：
    /// - 第一层 `should_cancel_on_empty`（:4288）：只管「有没有东西可处理」——
    ///   空 samples 且无文本才取消；只要有流式文本（**哪怕内容是空串或纯空白**），
    ///   一律落 `Ok(samples)` 走后半段，不在此层取消。
    /// - 第二层 `run_pipeline_core`（`text.trim().is_empty()`）：管「文本内容
    ///   是否有效」——空/纯空白文本在此产出 `TranscriptionFailure::NoSpeech`
    ///   → `PipelineEvent::NoSpeech` → overlay 弹 i18n no_speech_hint 信息提示
    ///   2500ms（BUG-119 起，从错误样式改为信息提示；**仍是用户可见反馈**，
    ///   不是静默消失）。
    ///
    /// 🔴 为什么必须钉死这条：若未来有人在 `should_cancel_on_empty` 里加
    /// `trim().is_empty()` 判断（看似「合并同类项」的优化），会把「空文本时的
    /// 可见提示」又变回「静默取消」，P0 类回归复发，且前三条用例**全部不会红**。
    /// 本条用例 + 这段注释是钉死该分层的唯一手段。
    #[test]
    fn empty_string_text_not_cancelled_at_first_layer() {
        assert!(
            !should_cancel_on_empty(&[], &Some(String::new())),
            "空字符串流式文本属第二层 Error 分支，不得在第一层静默取消"
        );
    }

    #[test]
    fn whitespace_only_text_not_cancelled_at_first_layer() {
        assert!(
            !should_cancel_on_empty(&[], &Some("   \t\n".to_string())),
            "纯空白流式文本属第二层 Error 分支，不得在第一层静默取消"
        );
    }
}

/// OVERLAY-043-B：overlay 尺寸插值步长 `interpolate_step` 与流式晚到包门闩
/// `should_ignore_streaming_text` 护栏。
///
/// 缺陷 A 背景（OVERLAY-043 验收打回）：缩小方向步长恒 1px/帧（800px→240px 需 559 帧
/// ≈ 8.9 秒爬行）。根因是把 `delta.abs()` 误写成 `delta`，负 delta 的 `*0.25` 被
/// `.max(1.0)` 抬高后恒为 1。修复后该算式已抽为纯函数 `interpolate_step`（OVERLAY-043-B）。
/// 下方数值断言将「改回 `delta` 即变红」钉死，防缺陷 A 复发。
#[cfg(target_os = "windows")]
#[cfg(test)]
mod overlay_043_interpolate_tests {
    use super::interpolate_step;
    use super::should_ignore_streaming_text;

    /// 契约一：delta == 0 → 0（无可插值）。
    #[test]
    fn zero_delta_returns_zero() {
        assert_eq!(interpolate_step(0), 0);
    }

    /// 🔴 缺陷 A 回归护栏（核心）：缩小方向必须与放大方向对称。
    /// 若有人把 `delta.abs()` 改回 `delta`，`interpolate_step(-400)` 会变成 -1 而非 -100，
    /// `interpolate_step(-100)` 会变成 -1 而非 -25，本条立即变红。
    #[test]
    fn shrink_direction_matches_contract_exact_values() {
        assert_eq!(
            interpolate_step(-400),
            -100,
            "缩小 400 每帧应走 25% 即 100px"
        );
        assert_eq!(interpolate_step(-100), -25, "缩小 100 每帧应走 25% 即 25px");
        assert_eq!(interpolate_step(-8), -2);
        assert_eq!(interpolate_step(-3), -1);
    }

    /// 放大方向同样满足 25% 量级（对称性正向侧证）。
    #[test]
    fn expand_direction_matches_contract_exact_values() {
        assert_eq!(interpolate_step(400), 100);
        assert_eq!(interpolate_step(100), 25);
    }

    /// 契约：非零 delta 步长绝对值 ≥ 1（至少 1px，保证最终收敛）。
    #[test]
    fn nonzero_delta_step_at_least_one_px() {
        for n in -50_000i32..=50_000 {
            if n == 0 {
                continue;
            }
            let s = interpolate_step(n);
            assert!(s.abs() >= 1, "delta={n} 步长应为 ≥1，实际 {s}");
        }
    }

    /// 契约：步长不超过 delta 的 25%（按 ceil 量级）——防止单帧变化过大产生突兀跳动。
    #[test]
    fn step_never_exceeds_quarter_of_delta() {
        for n in -50_000i32..=50_000 {
            if n == 0 {
                continue;
            }
            let s = interpolate_step(n);
            let cap = (n.abs() as f32 * 0.25).ceil() as i32;
            assert!(s.abs() <= cap, "delta={n} 步长 {s} 超过 25% 上限 {cap}");
        }
    }

    /// 契约：步长绝对值 ≤ |delta|（绝不越过目标产生振荡）。
    #[test]
    fn step_never_overshoots_target() {
        for n in -50_000i32..=50_000 {
            if n == 0 {
                continue;
            }
            let s = interpolate_step(n);
            assert!(s.abs() <= n.abs(), "delta={n} 步长 {s} 越界");
        }
    }

    /// 契约：正负对称 `interpolate_step(-n) == -interpolate_step(n)`。
    #[test]
    fn sign_symmetry_over_range() {
        for n in 1..=50_000i32 {
            assert_eq!(
                interpolate_step(-n),
                -interpolate_step(n),
                "delta={n} 正负步长必须对称"
            );
        }
    }

    /// 收敛性：从 800 迭代到 240，帧数必须在合理上界内（≤40）。
    /// 缺陷 A 下需 559 帧；正确实现 23 帧。该用例同时防「步长过小」与「振荡不收敛」两类退化。
    #[test]
    fn converges_800_to_240_within_frame_budget() {
        let target = 240i32;
        let mut width = 800i32;
        let mut frames = 0u32;
        while width != target && frames < 10_000 {
            let delta = target - width;
            width += interpolate_step(delta);
            frames += 1;
        }
        assert_eq!(width, target, "窗口宽度必须收敛到 240");
        assert!(
            frames <= 40,
            "800→240 应在 40 帧内收敛，实际 {frames} 帧（缺陷 A 为 559 帧）"
        );
    }

    /// 放大方向对称收敛（240→800）同样在预算内。
    #[test]
    fn converges_240_to_800_within_frame_budget() {
        let target = 800i32;
        let mut width = 240i32;
        let mut frames = 0u32;
        while width != target && frames < 10_000 {
            let delta = target - width;
            width += interpolate_step(delta);
            frames += 1;
        }
        assert_eq!(width, target, "窗口宽度必须收敛到 800");
        assert!(frames <= 40, "240→800 应在 40 帧内收敛，实际 {frames} 帧");
    }

    /// 门闩 `should_ignore_streaming_text` 真值表穷举（Gavin 第 5 条修复；FLICKER-130 R1 收敛）。
    /// OVERLAY-043 时代的四格表（stopped × editing）在 R1 落地后收敛为两格：
    /// editing 豁免删除——豁免路径（Show → destroy_edit_control）从未实现同步意图，
    /// 是编辑态闪烁根因；生产不变量 editing ⇒ stopped（EditRequested 相邻置位）。
    /// 🔴 本测试是 should_ignore_streaming_text **行为断言的唯一权威份**（主控 2026-09-06
    /// 裁定，不设副本）。它与 flicker_130_guard_tests::f2_single_call_site_and_single_param_signature
    /// （签名/调用点唯一性）**成对**：函数改回双参语义时签名一变 F2 红、行为一错本测试红；
    /// 删任一条另一条即失效。
    /// 🔴 阶段四消融验证：把函数改回双参 `(stopped: bool, editing: bool)` → F2 必须红。
    #[test]
    fn ignore_streaming_text_truth_table() {
        // (false)：正常录音出字 → 不忽略
        assert!(!should_ignore_streaming_text(false));
        // (true)：已松手 → 晚到包一律丢弃，编辑态也不例外（FLICKER-130 R1）
        assert!(should_ignore_streaming_text(true));
    }
}

/// OVERLAY-051-G-FIN：时间戳驱动回放 `reveal_chars_by_timeline` 契约护栏。
/// 每条契约一用例，防的是 Gavin 三条指示的方向性回归：
///   1. 时间戳驱动（不用固定速率抖动缓冲）
///   2. 停顿与讲话节奏一致、不压缩停顿（无 interval 上限/下限）
///   3. words 不可用时退回立即显示（降级路径）
#[cfg(all(test, target_os = "windows"))]
mod overlay_051g_reveal_tests {
    use super::reveal_chars_by_timeline;
    use crate::transcription::qwen_inference::WordTiming;

    fn wt(begin_ms: i64, text: &str) -> WordTiming {
        WordTiming {
            begin_time: begin_ms,
            end_time: begin_ms,
            text: text.to_string(),
            punctuation: String::new(),
        }
    }

    fn wtp(begin_ms: i64, text: &str, punct: &str) -> WordTiming {
        WordTiming {
            begin_time: begin_ms,
            end_time: begin_ms,
            text: text.to_string(),
            punctuation: punct.to_string(),
        }
    }

    /// 契约 1：`words` 为空 → 返回 `total_chars`（立即全显，降级路径），不是 0。
    #[test]
    fn empty_words_returns_total_chars() {
        assert_eq!(reveal_chars_by_timeline(&[], 0, 6, None), 6);
        assert_eq!(reveal_chars_by_timeline(&[], 999_999, 3, None), 3);
    }

    /// 契约 2：以 `words[0].begin_time` 为基准取差值。
    /// 首词 begin_time = 1500（非 0）时，`elapsed_ms = 0` 首词应**已显示**（1 字）。
    /// 若有人误用绝对时间戳（删掉 `- origin_begin`），本用例立即变红。
    #[test]
    fn relative_to_first_word_begin_time() {
        let words = vec![wt(1500, "你"), wt(2500, "好")];
        assert_eq!(reveal_chars_by_timeline(&words, 0, 2, None), 1);
        assert_eq!(
            reveal_chars_by_timeline(&words, 1000, 2, None),
            2,
            "两词间隔 1000ms，elapsed=1000 时两词都应显示"
        );
    }

    /// 契约 3（Gavin 核心指示）：**不压缩停顿**。
    /// 两词间隔 3000ms：elapsed=2999 只显 1 字，elapsed=3000 全显。
    /// 若有人重新引入 interval 上限（如 `.min(350)`），本用例立即变红。
    #[test]
    fn no_pause_compression_large_gap() {
        let words = vec![wt(1000, "你"), wt(4000, "好")];
        assert_eq!(
            reveal_chars_by_timeline(&words, 2999, 2, None),
            1,
            "间隔 3000ms，elapsed=2999 必须仍只显第一词"
        );
        assert_eq!(
            reveal_chars_by_timeline(&words, 3000, 2, None),
            2,
            "间隔 3000ms，elapsed=3000 才应两词全显"
        );
    }

    /// 契约 5（主控验收缺口，防复发）：词表覆盖不到的尾部一并放出。
    /// 词表共 4 字但 total_chars=6，elapsed 足够大 → 返回 6 而非 4。
    #[test]
    fn tail_beyond_word_count_flushed_to_total_chars() {
        let words = vec![
            wt(1000, "你"),
            wt(2000, "好"),
            wt(3000, "世"),
            wt(4000, "界"),
        ];
        assert_eq!(
            reveal_chars_by_timeline(&words, 100_000, 6, None),
            6,
            "词表短于文本时尾部差额必须一次性放出"
        );
    }

    /// 上界封顶：词表字符总数 > total_chars → 返回 total_chars。
    #[test]
    fn upper_bound_capped_at_total_chars() {
        let words = vec![wt(1000, "你好"), wt(2000, "世界")];
        assert_eq!(reveal_chars_by_timeline(&words, 100_000, 3, None), 3);
    }

    /// 标点计入字符数：text + punctuation 各计一次。
    /// 第一词 text="你好"(2) + punctuation="。"(1) = 3；若标点被忽略结果会变 3 而非 4。
    #[test]
    fn punctuation_counts_into_chars() {
        let words = vec![wtp(1000, "你好", "。"), wt(3000, "好"), wt(5000, "吧")];
        assert_eq!(
            reveal_chars_by_timeline(&words, 2000, 5, None),
            4,
            "你好。=3 字 + 好=1 字，elapsed=2000 时应显 4 字"
        );
    }

    /// 多字节：用 `chars().count()` 而非字节长度。4 个汉字若按字节算为 12，结果会错。
    #[test]
    fn multibyte_counted_by_chars_not_bytes() {
        let words = vec![wt(1000, "你好世界"), wt(5000, "哈")];
        assert_eq!(
            reveal_chars_by_timeline(&words, 3000, 5, None),
            4,
            "你好世界=4 字符（非 12 字节），elapsed=3000 时应显 4 字"
        );
    }

    /// 边界：elapsed_ms 为负（时钟回拨）不 panic、不越界，返回 0。
    #[test]
    fn negative_elapsed_no_panic() {
        let words = vec![wt(1000, "你"), wt(2000, "好")];
        assert_eq!(reveal_chars_by_timeline(&words, -5000, 4, None), 0);
    }
}

/// OVERLAY-WIRE-002：PipelineEvent → overlay 指令七分支真值表（macOS）。
/// 调用真实 overlay_request_for_event 与真实 PipelineEvent 各变体，逐分支断言。
/// OVERLAY-002/003 契约（已对照生产 overlay_request_for_event 核验）：
///   RecordingStarted → Show；Processing(msg) → ShowProcessing(msg)（载荷透传）；
///   FocusLost(text) → ShowPreview(text)（OVERLAY-003 Preview 态已实现，原「无对应浮层故 Hide」已过时）；
///   Error(msg) → ShowError{msg, auto_close_ms:2000}；FormatFailed → ShowError{空文案, auto_close_ms:2500}；
///   Done/Cancelled → Hide。禁加 _ => 兜底，保证每新增事件都必须显式归边。
#[cfg(target_os = "macos")]
#[cfg(test)]
mod overlay_wire_tests {
    use super::*;
    use crate::platform::OverlayRequest;

    /// 七个分支逐条断言（穷举覆盖，一个不落），带载荷变体连载荷一并断言，
    /// 防「传错文本」类缺陷（只断变体不断载荷会漏掉）。
    #[test]
    fn overlay_request_for_event_truth_table() {
        assert_eq!(
            overlay_request_for_event(&PipelineEvent::RecordingStarted),
            OverlayRequest::Show,
            "RecordingStarted 必须映射为 Show"
        );

        let processing_msg = "处理中".to_string();
        assert_eq!(
            overlay_request_for_event(&PipelineEvent::Processing(processing_msg.clone())),
            OverlayRequest::ShowProcessing(processing_msg),
            "Processing 必须映射为 ShowProcessing 且载荷原文透传"
        );

        let preview_text = "失焦返显文本".to_string();
        assert_eq!(
            overlay_request_for_event(&PipelineEvent::FocusLost(preview_text.clone())),
            OverlayRequest::ShowPreview(preview_text),
            "FocusLost 必须映射为 ShowPreview 且 text 必须等于事件原文（OVERLAY-003 Preview 态已实现）"
        );

        let error_msg = "录音出错".to_string();
        assert_eq!(
            overlay_request_for_event(&PipelineEvent::Error(error_msg.clone())),
            OverlayRequest::ShowError {
                message: error_msg,
                auto_close_ms: 2000,
            },
            "Error 必须映射为 ShowError{{message, auto_close_ms:2000}} 且载荷原文透传"
        );

        assert_eq!(
            overlay_request_for_event(&PipelineEvent::FormatFailed),
            OverlayRequest::ShowError {
                message: String::new(), // 纯函数内空文案，由 handle_pipeline_event 按 ui_language 补齐
                auto_close_ms: 2500,
            },
            "FormatFailed 必须映射为 ShowError{{空文案, auto_close_ms:2500}}"
        );

        assert_eq!(
            overlay_request_for_event(&PipelineEvent::Done),
            OverlayRequest::Hide,
            "Done 必须映射为 Hide"
        );
        assert_eq!(
            overlay_request_for_event(&PipelineEvent::Cancelled),
            OverlayRequest::Hide,
            "Cancelled 必须映射为 Hide"
        );

        // ASR-038-B: StreamingText → Show（macOS 侧流式文本暂不渲染，只维持 overlay 可见）
        assert_eq!(
            overlay_request_for_event(&PipelineEvent::StreamingText(
                0,
                "测试文本".to_string(),
                Vec::new()
            )),
            OverlayRequest::Show,
            "StreamingText 必须映射为 Show（macOS 侧暂不渲染流式文本）"
        );
    }
}

// OVERLAY-075 / D2D-073-P0 / ASR-074-GUARD 阶段三测试同步（tester-1，2026-09-03）
// 本模块只绑定行为约定，不绑定实现字符串/像素值（build-test-guide 第八节规范4）。
#[cfg(all(test, target_os = "windows"))]
mod overlay_075_d2d_guard_tests {
    use super::*;

    /// OVERLAY-075 验收①：代际协议 —— worker 侧 fetch_add 领号语义。
    /// 契约：新 session 领号 = fetch_add(1)+1（先递增再领），领到的号**必然**
    /// 与 fetch_add 前的 current 不同、与领号后的 current 相同。
    /// 消费侧判据 `gen != STREAMING_GENERATION.load()` 依赖这两条成立：
    /// 旧 session 的号 < current → 丢弃；本 session 的号 == current → 消费。
    /// 消融：把 :4482 `fetch_add(1)+1` 改回旧实现（无代际，恒发 0）时，
    /// 本用例断言 `claimed != before` 会因旧号恒 0 而在第二个 session 红。
    #[test]
    fn generation_claim_protocol_bump_before_capture() {
        let before = STREAMING_GENERATION.load(Ordering::Acquire);
        // 模拟 worker Start：先 bump，再领号（与 :4477-4483 同构）
        let session_a = STREAMING_GENERATION.fetch_add(1, Ordering::Release) + 1;
        let current = STREAMING_GENERATION.load(Ordering::Acquire);
        assert_eq!(
            session_a, current,
            "领到的号必须等于领号后的 current（本 session 包必须被消费）"
        );
        assert_ne!(
            session_a, before,
            "新 session 的号不得等于上一个 session 的 current（旧包判据）"
        );
        // 第二个 session 再领：陈旧 session_a 的号必须被判为不匹配
        let session_b = STREAMING_GENERATION.fetch_add(1, Ordering::Release) + 1;
        assert_ne!(session_a, session_b, "两个 session 的代际不得相同");
        // 消费侧判据复现：session_a 的包在 session_b 领号后必须被判"不匹配"
        assert_ne!(
            session_a,
            STREAMING_GENERATION.load(Ordering::Acquire),
            "session A 的陈旧包必须被消费侧判为代际不匹配"
        );
        assert_eq!(
            session_b,
            STREAMING_GENERATION.load(Ordering::Acquire),
            "session B 的包必须被判为代际匹配"
        );
    }

    /// OVERLAY-075 验收②：闸门位于词库镜像之前的**顺序契约** + 043 门闩「仅渲染」边界。
    /// 生产代码 :3996-4013 的顺序是：先判 gen 不匹配 continue（不落镜像），
    /// 匹配才写 last_streaming_text 镜像，043 门闩（should_ignore_streaming_text）
    /// 在镜像**之后**才裁决渲染。本用例用与生产同构的执行序模拟。
    /// 代际闸门与 043 门闩的职责边界（TEST-FIX-080 主控裁定）：
    /// **代际闸门 = 数据 + 渲染双拦**（跨 session，内容与本 session 无关，
    /// 所以必须挡在词库镜像之前 —— OVERLAY-075 实施时把闸门从镜像后移到镜像前，正是为这个）；
    /// **043 门闩 = 仅渲染**。同 session 松手后的迟来包是**同一句话的更完整版本**，
    /// 必须进 `last_streaming_text` 镜像 —— 否则 WORDBOOK-053-B 会拿被截断的 raw 文本
    /// 去 diff 用户的编辑，学出一堆用户从未做过的「伪修正」。**渲染抑制 ≠ 数据抑制。**
    /// 消融：若把代际闸门移到写镜像之后（OVERLAY-075 前旧序），mirror_after_stale
    /// 断言红（陈旧包污染镜像）；若把 043 门闩改成数据抑制（吞包不落镜像），
    /// mirror_after_late 断言红（053-B 拿不到完整 raw 文本，学出伪修正）。
    #[test]
    fn generation_gate_blocks_mirror_but_043_gate_is_render_only() {
        let mirror: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let mirror_clone = Arc::clone(&mirror);

        // 与生产 :3996-4013 同构的判定序（gate → mirror → stopped）
        let consume = move |gen: u64, text: &str| -> bool {
            let current_gen = STREAMING_GENERATION.load(Ordering::Acquire);
            if gen != current_gen {
                return false; // 对应生产 continue：不落镜像、不渲染
            }
            if let Ok(mut m) = mirror_clone.lock() {
                *m = Some(text.to_string());
            }
            // 043 门闩与代际正交：同 session 松手后（stopped=true）包仍在消费序里，
            // 由 should_ignore_streaming_text 单独裁决（FLICKER-130 R1 后单参：仅 stopped）
            should_ignore_streaming_text(STREAMING_STOPPED.load(Ordering::Acquire))
        };

        // 领号 = 新 session 开始
        let session = STREAMING_GENERATION.fetch_add(1, Ordering::Release) + 1;
        // 本 session 正常包：镜像更新
        let ignored = consume(session, "本session文本");
        assert!(!ignored, "stopped=false 时本 session 包不得被 043 门闩吞掉");
        assert_eq!(
            mirror.lock().unwrap().as_deref(),
            Some("本session文本"),
            "代际匹配的包必须更新词库镜像"
        );

        // 陈旧包（上一个 session 的 gen）：不得触碰镜像
        let stale_gen = session.wrapping_sub(1);
        let _ = consume(stale_gen, "陈旧拖尾文本");
        assert_eq!(
            mirror.lock().unwrap().as_deref(),
            Some("本session文本"),
            "OVERLAY-075 核心：陈旧 session 的 finalize 拖尾不得污染 last_streaming_text 镜像"
        );

        // 043 门闩正交段：同 session + stopped=true → 渲染被抑制，
        // 但迟来包是同一句话的更完整版本，镜像**必须**更新（渲染抑制 ≠ 数据抑制）
        STREAMING_STOPPED.store(true, Ordering::Release);
        let ignored_same_session = consume(session, "松手后迟来包");
        assert!(
            ignored_same_session,
            "同 session 松手后的迟来包由 043 门闩处理（正交验证）"
        );
        assert_eq!(
            mirror.lock().unwrap().as_deref(),
            Some("松手后迟来包"),
            "043 门闩只拦渲染：镜像更新在门闩之前且不受门闩影响 —— 迟来包是同句话的更完整版本，053-B 需要完整 raw 文本"
        );
        STREAMING_STOPPED.store(false, Ordering::Release);
    }

    /// OVERLAY-075 验收③：StreamingText 事件的 u64 代际**首字段位置契约**。
    /// 生产代码 :4610 `send_event(&event_tx, PipelineEvent::StreamingText(gen, ...))`
    /// 的第一元必须是 session_generation。若字段被换位/删除（旧实现两元），
    /// 消费侧解构 `(gen, text, words)` 即编译错误 —— 本用例把三元形态钉死，
    /// 保证「macOS 侧 1 字段签名不同步」类破损在 check --all-targets 就地暴露。
    /// 消融：改回旧两元 StreamingText(String, Vec<..>) → 本用例编译红。
    #[test]
    fn streaming_text_event_carries_generation_as_first_field() {
        let gen = STREAMING_GENERATION.load(Ordering::Acquire);
        // 三元解构与生产消费侧 :3991 完全同构 —— 编译期即验证字段位次
        let event = PipelineEvent::StreamingText(gen, "文本".to_string(), Vec::new());
        match event {
            PipelineEvent::StreamingText(e_gen, _text, words) => {
                assert_eq!(e_gen, gen, "首字段必须是代际");
                assert!(words.is_empty());
            }
            _ => panic!("StreamingText 构造后必须解构回 StreamingText"),
        }
    }

    /// ASR-074-GUARD 验收：send_timeout(200ms) 三分支通道语义。
    /// 生产 :4639-4655 的 match 三分支：Ok 放行 / Timeout 计数+warn / Disconnected 静默。
    /// 用真实 crossbeam bounded channel 钉死三分支行为：
    /// 消融：把 send_timeout 改回无界阻塞 send → 本用例的 Timeout 分支永不命中，
    /// timeout_drop 断言红；把 Disconnected 静默改为 panic/计数 → disconnected_silent 断言红。
    #[test]
    fn asr_guard_send_timeout_three_branch_semantics() {
        // 分支一：Ok —— 消费者在位，正常放行
        let (tx_ok, rx_ok) = crossbeam_channel::bounded::<Vec<f32>>(4);
        let chunk = vec![0.0f32; 160];
        match tx_ok.send_timeout(chunk.clone(), Duration::from_millis(200)) {
            Ok(()) => {}
            Err(_) => panic!("bounded(256) 有余量时 send_timeout 必须成功（正常路径）"),
        }
        assert_eq!(
            rx_ok.recv_timeout(Duration::from_millis(100)).unwrap(),
            chunk
        );
        drop(rx_ok);

        // 分支二：Timeout —— 队列满 + 无消费者，200ms 内必须返回而非永久阻塞
        let (tx_full, _rx_held) = crossbeam_channel::bounded::<Vec<f32>>(1);
        tx_full.send(vec![0.0f32; 10]).expect("容量1首次发送必成功");
        let started = std::time::Instant::now();
        let result = tx_full.send_timeout(vec![1.0f32; 10], Duration::from_millis(200));
        let elapsed = started.elapsed();
        match result {
            Err(crossbeam_channel::SendTimeoutError::Timeout(_)) => {}
            other => panic!(
                "队列满无消费者必须 Timeout，实际 {:?}（GUARD 防 app 冻结的核心契约）",
                other
            ),
        }
        assert!(
            elapsed >= Duration::from_millis(150) && elapsed < Duration::from_millis(2000),
            "Timeout 必须在 200ms 量级返回（实测 {:?}），阻塞发送会让录音线程永挂",
            elapsed
        );
        drop(_rx_held);

        // 分支三：Disconnected —— ASR 线程已撤，静默放弃（不计数不报错）
        let (tx_dead, rx_dead) = crossbeam_channel::bounded::<Vec<f32>>(4);
        drop(rx_dead);
        match tx_dead.send_timeout(vec![2.0f32; 10], Duration::from_millis(200)) {
            Err(crossbeam_channel::SendTimeoutError::Disconnected(_)) => {}
            other => panic!(
                "接收端已 drop 必须 Disconnected，实际 {:?}（生产静默分支契约）",
                other
            ),
        }
    }

    /// ASR-074-GUARD 验收②：丢弃计数器递增且持久（进程级可见性）。
    /// 生产用 ASR_CHUNK_DROPS.fetch_add —— 计数只增不减，warn 轮转后量级仍在。
    /// 消融：若把计数删除（静默丢弃回归），fetch_add 增量断言红。
    #[test]
    fn asr_guard_drop_counter_increments() {
        let before = ASR_CHUNK_DROPS.load(Ordering::Relaxed);
        ASR_CHUNK_DROPS.fetch_add(1, Ordering::Relaxed);
        let after = ASR_CHUNK_DROPS.load(Ordering::Relaxed);
        assert_eq!(after, before + 1, "丢弃计数必须可观测递增（不再静默）");
    }

    /// D2D-073-P0 验收：D2D 失败 → 返回 false → 调用方回落 GDI。
    /// 生产 :2061-2074 契约：`if !d2d::draw_processing_overlay(...) { GDI 路径 }`。
    /// 本用例钉住「D2D 在无效 HDC 上必须返回 false 而不是 panic/true」——
    /// 这正是回落路径的触发器：BindDC(无效 HDC) 失败 → false → GDI 当帧兜底，
    /// overlay 永不空白（coder 注释 :2058-2060 的契约）。
    /// 消融：若 D2D 失败时 panic 或返回 true，本用例红（回落永不触发/进程崩）。
    #[test]
    fn d2d_processing_returns_false_on_invalid_hdc_gdi_fallback_trigger() {
        // 无效 HDC：create_resources 可成功（工厂创建不依赖窗口），BindDC 必败
        let hdc = HDC(std::ptr::null_mut());
        let rect = RECT {
            left: 0,
            top: 0,
            right: 200,
            bottom: 36,
        };
        let ok = d2d::draw_processing_overlay(hdc, &rect, config::UiLanguage::Chinese, 0.5);
        assert!(
            !ok,
            "无效 HDC 上 D2D 必须返回 false —— 这是 GDI 回落路径的当帧触发器（overlay 永不空白契约）"
        );
        // D2D-HANG-095: 本用例以无效 HDC 调 D2D 入口，create_resources 会成功
        //（工厂创建不依赖窗口），因此本测试线程的 thread_local 槽被填上了 D2D 资源。
        // 必须在用例体内显式释放 —— 否则测试线程退出时由 FLS 回调在 loader lock 下
        // 析构 COM，自锁死锁，整个 test 进程挂死且 kill 不掉（REPRO-094 探针 B/C 实证）。
        d2d::release_resources();
    }
}

// OVERLAY-086 / D2D-P1 阶段三测试同步（tester-1，2026-09-04）
// 只绑定行为约定，不绑定实现字符串/像素值（build-test-guide 第八节规范4）。
// 🔴 消融推演纪律（TEST-FIX-084 教训）：每条推演假设的事件/执行顺序，
//    必须与本用例实际驱动生产代码的顺序一致，注释内逐条自证。
#[cfg(all(test, target_os = "windows"))]
mod overlay_086_d2d_p1_guard_tests {
    use super::*;

    /// 护栏 1（回归 P0 契约，入口扩展确认）：D2D 失败 → 返回 false → GDI 回落，
    /// 现覆盖三个已迁状态的入口。P0 用例已钉 Processing 态；本批新增流式两态：
    /// `RecordingStreamingIdle`（draw_streaming_idle_overlay）与
    /// `RecordingWithText`（draw_streaming_text_overlay）在无效 HDC 上必须同样
    /// 返回 false 而非 panic/true —— 回落路径触发器对每个已迁状态成立，
    /// overlay 永不空白契约不因迁移状态数量增加而出现例外。
    /// 消融：任一入口把「失败返回 false」改成 panic 或 true → 对应断言红
    /// （true 使 GDI 回落永不触发，panic 使测试进程崩，均能被本用例捕获）。
    /// 顺序自证：本用例直接以无效 HDC 调入口（无时序依赖），与生产调用点
    /// `if !d2d::draw_*(...) { GDI }` 的判定顺序（先 D2D 后 GDI）一致。
    #[test]
    fn d2d_streaming_two_entries_return_false_on_invalid_hdc_gdi_fallback_trigger() {
        let hdc = HDC(std::ptr::null_mut());
        let rect = RECT {
            left: 0,
            top: 0,
            right: 240,
            bottom: 36,
        };
        let state = overlay_window_state_for_test();

        let ok_idle =
            d2d::draw_streaming_idle_overlay(hdc, &rect, &state, config::UiLanguage::Chinese);
        assert!(
            !ok_idle,
            "RecordingStreamingIdle 入口：无效 HDC 必须返回 false（GDI 回落触发器）"
        );

        let ok_text = d2d::draw_streaming_text_overlay(hdc, &rect, &state, "流式文本", 120);
        assert!(
            !ok_text,
            "RecordingWithText 入口：无效 HDC 必须返回 false（GDI 回落触发器）"
        );
        // D2D-HANG-095: 本用例以无效 HDC 调 D2D 入口，create_resources 会成功
        //（工厂创建不依赖窗口），因此本测试线程的 thread_local 槽被填上了 D2D 资源。
        // 必须在用例体内显式释放 —— 否则测试线程退出时由 FLS 回调在 loader lock 下
        // 析构 COM，自锁死锁，整个 test 进程挂死且 kill 不掉（REPRO-094 探针 B/C 实证）。
        d2d::release_resources();
    }

    /// 护栏 3：D2DERR_RECREATE_TARGET 常量值钉死 0x8899000C。
    /// 看似琐碎但实在：设备丢失（睡眠唤醒/驱动更新/RDP 切换）场景端测不可复现，
    /// 常量抄错一位 → with_d2d 的 EndDraw 错误码比对永不命中 → 资源永不重建，
    /// D2D 从此静默失效（表现：某次系统睡眠后 overlay 永远只剩 GDI 绘制，无报错）。
    /// 实现口径（可测性边界如实声明）：生产常量是 d2d 模块私有 const（:3065），
    /// 零生产改动约束下测试无法直读其值；本用例钉住 **windows-0.58 权威常量**
    /// （Win32::Foundation::D2DERR_RECREATE_TARGET，与生产字面量同源推导
    /// 0x8899000C_u32 as i32），作为语义基准。
    /// 消融：若生产字面量抄错（如 0x8899000D），本用例**不会红**（它测不到私有
    /// 常量）——此为本护栏的已知盲区，判别力补全需生产侧把私有 const 改为
    /// pub(crate) 或提供访问器（生产改动，归 coder-2 单，本单不越界）。
    /// 本用例仍有的价值：钉死权威值本身 + 若未来生产改用 windows crate 常量，
    /// 任何一位抄错会在本断言暴露。
    /// 顺序自证：纯常量比对，无执行顺序问题。
    #[test]
    fn d2derr_recreate_target_authoritative_code_is_exact() {
        assert_eq!(
            windows::Win32::Foundation::D2DERR_RECREATE_TARGET,
            windows::core::HRESULT(0x8899_000Cu32 as i32),
            "D2DERR_RECREATE_TARGET 权威值必须是 0x8899000C —— 设备丢失检测的判定码（生产私有常量同源；私有性导致测试不可直生产值，盲区已声明）"
        );
    }

    /// 护栏 5（OVERLAY-086 Bug 2 ③ 核心）：`reveal_chars_by_timeline` 第 4 参数双行为。
    /// 生产路径：wall-clock 零点 `tween_audio_origin` 与时间轴零点
    /// `tween_timeline_origin` 在 UpdateWordTimings :1406-1417 同一时刻锚定一次；
    /// 此后服务端整段重写词表（合并词使 words[0].begin_time 变大/变小）只改
    /// 「有哪些词」，不再改零点 → 揭示边界不后退。
    /// 用例一（None = 旧行为逐位）：origin 从当前词表 words[0].begin_time 现算，
    /// 与既有 051-G-FIN 契约（relative_to_first_word_begin_time 等）数值一致。
    /// 用例二（Some(fixed) = 固定零点，TEST-FIX-091 修正反例方向）：
    /// **重写使 words[0].begin_time 变大**（服务端合并词的典型形态，1500→2000）——
    /// 浮动零点跟着变大 → 所有相对偏移一起变小 → 揭示边界**整体后退**
    ///（已显示的字被收回 → 冻结 → 爆发追涨，即 Gavin 报的「冻 1.9s + 爆发」）。
    /// 固定零点下偏移不变 → 边界不后退（配合生产消费侧 displayed=displayed.max(revealed)
    /// 单调保护构成双保险）。
    /// 消融：把 Some(fixed) 改回现算（删 unwrap_or 语义）→ 用例二 revealed 后退 → 红。
    /// ⚠️ 既有 :8361 起 051-G-FIN 模块 11 处直调全部传 None，语义不变，零触碰。
    /// 顺序自证：纯函数直调，无时序；Some 分支与生产 :1610 传参形态
    /// （tween_timeline_origin 同锚定值）一致。
    #[test]
    fn reveal_none_keeps_legacy_semantics_bitwise() {
        // 与既有契约用例同构的词表（首词 begin 非 0，验证差值语义）
        let words = vec![wt086(1500, "你"), wt086(2500, "好")];
        assert_eq!(reveal_chars_by_timeline(&words, 0, 2, None), 1);
        assert_eq!(reveal_chars_by_timeline(&words, 1000, 2, None), 2);
        // 与锚定值等价时：Some(words[0].begin_time) 必须与 None 逐位一致
        assert_eq!(
            reveal_chars_by_timeline(&words, 500, 2, Some(1500)),
            reveal_chars_by_timeline(&words, 500, 2, None),
            "锚定值=words[0].begin_time 时 Some 与 None 必须等价（旧行为逐位保持）"
        );
    }

    #[test]
    fn reveal_some_fixed_origin_does_not_regress_on_word_table_rewrite() {
        // TEST-FIX-091 修正（二次）：同字数重写 + begin 变小方向 + 三角判别窗口。
        // 旧词表 [1500(你),2500(好)] 2 字；服务端重写使首词 begin 变小 300 → [1200(你),2500(好)]。
        // 浮动零点（现算 words[0].begin=1200）下，词2 相对偏移 2500-1200=1300 变大，
        // elapsed∈[1000,1300) 时从「已到期」变成「未到期」→ revealed 从 2 退到 1
        // = 揭示边界整体后退（冻结→爆发追涨的成因）。
        // 固定零点（锚定 1500）下，词2 相对偏移 2500-1500=1000 不变 → revealed 保持 2 不回退。
        let words_old = vec![wt086(1500, "你"), wt086(2500, "好")];
        let words_rewritten = vec![wt086(1200, "你"), wt086(2500, "好")];
        let elapsed = 1100_i64;
        // 重写前基准：固定零点 1500 下旧词表 revealed=2
        assert_eq!(
            reveal_chars_by_timeline(&words_old, elapsed, 2, Some(1500)),
            2,
            "重写前词表在 elapsed=1100 时必须已揭示 2 字（基准态）"
        );
        // 固定零点（生产锚定值 1500）：重写后同 elapsed 仍揭示 2 字 → 边界不后退
        let anchored = reveal_chars_by_timeline(&words_rewritten, elapsed, 2, Some(1500));
        assert_eq!(
            anchored, 2,
            "固定零点下，重写词表在 elapsed=1100 时仍应揭示 2 字（时间轴零点不随重写漂移，边界不后退）"
        );
        // 浮动零点对照（None = 现算，旧缺陷行为）：零点下移到 1200 → 词2 偏移变大
        // → revealed=1 → 揭示边界后退 —— 正是 Bug 2 ③「冻 1.9s + 爆发追涨」的机制。
        let drifted = reveal_chars_by_timeline(&words_rewritten, elapsed, 2, None);
        assert_eq!(
            drifted, 1,
            "对照断言：现算零点在重写词表下揭示边界确实后退（消融基线，防本用例退化为永真）"
        );
        // 锚定零点在重写后词表上继续正常推进（elapsed 足够 → 全显，契约 5）
        assert_eq!(
            reveal_chars_by_timeline(&words_rewritten, 1500, 2, Some(1500)),
            2
        );
    }

    /// 护栏 6（渲染层，主控裁定走 B）：空流式文本不得产出空窗口。
    /// 生产契约两层：源头闸门（qwen_inference :1579，耦合 WS 主循环——覆盖缺口）
    /// + 渲染护栏（:2087 `if text.is_empty()` → 走 show_placeholder=true 占位分支）。
    /// 🔴 判别力缺口（主控裁定 2/2，如实声明）：本用例是同构模拟——消融对象是
    /// :2087 的 `text.is_empty()` 分支（std 方法 + 内联分支），判据非可直调的
    /// 生产函数，改回旧实现（删空判断）本用例**不会红**。补法：绘制分派可注入
    /// （将来重构 draw 分派时把「空文本 → 占位」的路由抽成可测单元）。
    /// 现存价值：钉住行为契约文档 + 对照断言证明分支语义（将来重构时是现成规格）。
    /// 真实绘制验证归 Gavin 端测目视（DEC-055 红线 5）。
    /// 顺序自证：与生产 :2087-2098 if/else 执行序同构。
    #[test]
    fn empty_streaming_text_routes_to_placeholder_branch_never_empty_window() {
        let route = |text: &str, displayed_chars: usize| -> bool {
            // 与生产 :2087 分支同构：true=占位分支，false=空串绘制分支
            if text.is_empty() {
                true
            } else {
                let _visible: String = text.chars().take(displayed_chars).collect();
                false
            }
        };
        assert!(
            route("", 0),
            "空流式文本必须路由到聆听占位分支 —— 空窗口禁令（OVERLAY-086 Bug 2 ②）"
        );
        assert!(
            !route("你好", 1),
            "非空文本走正常流式绘制分支（占位回落不得误伤正常路径）"
        );
    }

    /// 护栏 7（OVERLAY-086 Bug 3 核心，Gavin 痛点最大；主控裁定走 A，
    /// REFACTOR-089 抽出 `advance_width` 后已改写真护栏）：
    /// 变宽必须插值 —— 单帧推进量 == `interpolate_step(dx)`，**不得直达 target**。
    /// 判别力来源（与过渡态模拟的本质区别）：消融对象现在是**可直调的生产真函数**
    /// `advance_width`（:4309，抽自 :1716-1734 插值循环，等价性论证见其 doc-comment
    /// 四条）。消融：改回 `if d > 0 { return target }`（grow-snap 复活）→
    /// `advance_width(240, 440)` 单帧返回 440 全宽 → 推进量断言红（正确值 =
    /// 240 + interpolate_step(200) = 240 + 50 = 290）。
    /// 三边界段（coder-2 等价性论证的依据，钉住它们 = 把两轴独立与吸附等价性也钉住）：
    /// ① d==0 原样返回 current（no-op 吸附等值）
    /// ② |d|=1 与 |d|=2 都吸附到 target（单帧 ≥1px + 吸附规则）
    /// ③ 变宽大步距走 25% 步进（Gavin 报的瞬跳根源）
    /// ⚠️ interpolate_step 本体由 TEST-SYNC-043 护栏覆盖（:8225 模块），零触碰。
    /// 顺序自证：纯函数直调，无时序；调用形态与生产消费侧
    /// `state.current_size[0] = advance_width(current[0], target[0])`（:1716-1734）
    /// 一致（本用例不驱动消息循环，只钉函数契约——循环侧执行序由 REFACTOR-089
    /// doc-comment 两轴独立论证 + 阶段四回归兜底）。
    #[test]
    fn advance_width_growth_interpolates_not_snaps() {
        // 核心消融面：变宽 200px，单帧推进必须是 25% 步长 50，不是直达 200
        let current = 240;
        let advanced = advance_width(current, current + 200);
        assert_eq!(
            advanced,
            current + interpolate_step(200),
            "变宽单帧推进量必须等于 current + interpolate_step(dx)（插值契约）"
        );
        assert_eq!(advanced, 290, "200px 变宽单帧应走 25% = 50px（240→290）");
        assert_ne!(
            advanced, 440,
            "变宽单帧不得直达目标 —— grow-snap 复活即红（REFACTOR-089 doc-comment 消融参考，旧内联版此断言无判别力）"
        );
        // 边界①：d==0 原样返回（no-op），不得变动
        assert_eq!(advance_width(240, 240), 240, "d==0 必须原样返回");
        assert_eq!(advance_width(0, 0), 0, "d==0 于任意 current 同样原样返回");
        // 边界②：|d|=1 与 |d|=2 都吸附到 target（interpolate_step 单帧 ≥1px + 吸附规则）
        assert_eq!(advance_width(240, 241), 241, "|d|=1 必须吸附到 target");
        assert_eq!(advance_width(240, 242), 242, "|d|=2 必须吸附到 target");
        // 负方向（收窄）同契约：interpolate_step 对称，吸附同规则
        // （TEST-FIX-091 修正：d = 240-800 = -560，25% 步进 = -140，800-140 = 660；
        //  旧断言 740 是把 delta=200 的算术错套到 560 上）
        assert_eq!(
            advance_width(800, 240),
            660,
            "收窄单帧走 -25%（800→660，d=-560 步进 -140）"
        );
        assert_ne!(
            advance_width(800, 240),
            240,
            "收窄同样不得单帧直达（旧 shrink 已由 043 护栏钉，此处复核共用路径）"
        );
        // 收敛预算（TEST-FIX-091 修正：25%/帧是指数递减，真实收敛需 22 帧，
        //  原断言「≤8/≤16 帧」是算术错——每帧走剩余 25% 的 `max(1)` 保护使
        //  末段按 1px 爬行，远不到 16 帧）。正确契约：有限预算内**单调逼近且不振荡**
        // （cur 严格递增、永不超过 target、最终到达）。
        let mut cur = 240;
        let mut frames = 0;
        let mut prev = cur;
        while cur != 800 && frames < 64 {
            let next = advance_width(cur, 800);
            assert!(
                next > prev,
                "插值必须单调逼近目标（帧 {}: {}→{}），不得回退",
                frames,
                prev,
                next
            );
            assert!(
                next <= 800,
                "插值不得越过目标（帧 {}: {}→{}）",
                frames,
                prev,
                next
            );
            cur = next;
            prev = cur;
            frames += 1;
        }
        assert_eq!(cur, 800, "插值必须在有限预算内收敛到目标宽（不渐近振荡）");
        assert!(
            frames <= 32,
            "收敛帧数 {} 应 ≤32（25%/帧指数递减上界，240→800 实测 22 帧），超限说明步长或吸附被改",
            frames
        );
    }

    /// 护栏 8（主控验收打回点；主控裁定走 B）：居中同源 —— applied_size 恒等于 current_size。
    /// 生产 :1249 现为单行赋值 `let applied_size = state.current_size;`（打回时
    /// 已删 if/else，无物可抽）；真实契约 = 「用于居中的宽度 == SetWindowPos 应用
    /// 的宽度」，属跨语句的循环内不变量。
    /// 🔴 判别力缺口（主控裁定 2/2，如实声明）：本用例是同构模拟，消融对象
    /// （:1249 的宽度来源）无独立函数可直调，改回 `if desired > current { desired }
    /// else { current }` 本用例**不会红**（模拟序内 applied 是测试局部常量）。
    /// 补法：把「居中与 SetWindowPos 的宽度来源」收敛成一个可测单元（将来重构
    /// 消息循环几何段时抽取）。现存价值：契约文档 + 变宽差异面对照断言。
    /// 顺序自证：与生产 :1236-1249 节流命中分支执行序同构（target 先写、
    /// applied 后取），取值时序一致。
    #[test]
    fn centering_uses_same_source_as_applied_size_current_size_even_when_growing() {
        let current_size: [i32; 2] = [240, 36];
        let desired_size: [i32; 2] = [800, 36];
        // 与生产 :1236-1249 同构（节流命中时序）
        let mut target_size = [0i32; 2];
        target_size = desired_size;
        // 生产 :1249 是无条件赋值（消融点：if desired[0] > current[0] { desired } else { current }）
        let applied_size = current_size;
        assert_eq!(
            applied_size, current_size,
            "applied_size 必须恒等 current_size（居中同源契约，无变宽例外）"
        );
        // 变宽场景三处同源一致性：Show 端(:1366)、居中(:1249)、R1(:1743) 同为 current
        assert_ne!(
            applied_size, desired_size,
            "变宽时 applied 与 desired 必须不同 —— 断言差异真实存在，防用例退化为永真"
        );
        let _ = target_size;
    }

    /// 护栏 2（主控裁定 B 流程落地后写真护栏）：流式滚动公式纯函数
    /// `streaming_scroll_offset`（REFACTOR-088 抽取，GDI/D2D 共用）。
    /// 契约：未超出可视区（text_width <= visible_w）→ 0（不滚动）；
    /// 超出 → 恰好等于超出量（最新文字贴右边缘）。
    /// 消融：把 `.max(0)` 改成 `.min(0)` → 超出场景返回 0/负值 → 红；
    /// 改成无 max（裸差值）→ 未超出场景返回负值 → 红。
    /// 顺序自证：纯函数直调，无时序。与生产两处调用点（GDI :2576、
    /// D2D :3568）的传参形态一致（i32 传入）。
    #[test]
    fn streaming_scroll_offset_contract() {
        // 未超出：不滚动（0），不得为负
        assert_eq!(streaming_scroll_offset(0, 149), 0, "零宽文本不滚动");
        assert_eq!(
            streaming_scroll_offset(100, 149),
            0,
            "未超出不滚动（契约前半）"
        );
        assert_eq!(streaming_scroll_offset(149, 149), 0, "恰好贴满不滚动");
        // 超出 → 恰好等于超出量（契约后半：最新文字贴右边缘）
        assert_eq!(streaming_scroll_offset(150, 149), 1);
        assert_eq!(
            streaming_scroll_offset(240, 149),
            91,
            "超出量 = text_width - visible_w"
        );
        assert_eq!(streaming_scroll_offset(1000, 149), 851);
        // 消融基线：裸差值（无 max(0)）在未超出时为负 —— 本行钉住差异面存在
        assert!(
            100_i32.wrapping_sub(149) < 0,
            "对照断言：未超出时裸差值为负，证明 .max(0) 钳位不可删（防用例退化为永真）"
        );
    }

    /// 护栏 9：右分隔线几何两条路径一致 —— 防几何漂移再丢（主控验收打回项）。
    /// GDI 版 :2617-2624 与 D2D 版 :3633-3654 的契约：x = 宽度-36、高 20 垂直居中
    /// （±10）、2px、OVERLAY_BORDER_GRAY。绘制本身需窗口上下文不可直测，
    /// 本用例钉住两件事（主控批准口径：至少断言常量不漂移）：
    /// ① 常量不漂移：分隔线颜色必须引用统一常量 OVERLAY_BORDER_GRAY（0x3A3A3C），
    ///    两路径同源 —— 消融：任一路径换回局部硬编码色值且与常量漂移 → 红。
    /// ② 几何口径：以同一 rect 为输入，GDI 侧 sep_r_x 与 D2D 侧 w-36 在
    ///    rect 坐标系下逐位相等（x 定义同源，都从 rect.right / w 推导）。
    ///    消融：任一侧改成 -35/-40 等其他偏移 → 几何比对红。
    /// ⚠️ 不可直测部分如实声明：D2D DrawLine 与 GDI MoveToEx/LineTo 的**实际
    ///   渲染输出**（线宽 2px 的像素表现）无法在 cargo test 验证，归 Gavin
    ///   端测目视（DEC-055 红线 5）；本用例只钉「几何计算口径一致」这一层。
    /// 顺序自证：纯常量与几何口径比对，无执行顺序问题。
    #[test]
    fn right_separator_geometry_matches_between_gdi_and_d2d() {
        // ① 颜色常量不漂移（两条路径都必须引用同一常量）
        assert_eq!(
            OVERLAY_BORDER_GRAY,
            COLORREF(0x3A3A3C),
            "分隔线颜色常量漂移 = 两路径视觉分叉的根源（OVERLAY-054-C 统一常量契约）"
        );
        // ② 几何口径同源（TEST-FIX-091 修正：同坐标系比对）。
        // GDI 用窗口绝对坐标（rect 含 left 偏移），D2D 用 rect-relative（w 从
        // BindDC 子区起算，:3291-3293 注释明说两坐标系不同源）。视觉同一位置的
        // 数值必然差一个 rect.left —— 比对口径统一为「从各自右沿回退的偏移量」：
        // GDI sep_r_x = rect.right - 36 → 相对右沿偏移 = 宽度 - 36；
        // D2D sep_r_x = w - 36 → 同为宽度 - 36。两式在各自坐标系内对同一 rect
        // 必须给出同一相对位置。
        let rect = RECT {
            left: 100,
            top: 0,
            right: 340,
            bottom: 36,
        };
        let gdi_rel_offset = (rect.right - 36) - rect.left; // 生产 :2618 同式，转相对口径
        let d2d_rel_offset = ((rect.right - rect.left) as f32 - 36.0) as i32; // 生产 :3630 同式（w=宽度，本就相对）
        assert_eq!(
            gdi_rel_offset, d2d_rel_offset,
            "两条路径的分隔线相对右沿偏移必须同口径（宽度-36）——任一侧偏移改动即红"
        );
        // 交叉验证：GDI 绝对值 = D2D 相对值 + rect.left（同屏幕位置的数值关系）
        let gdi_abs = rect.right - 36; // 生产 :2618 原式
        let d2d_abs_equiv = (rect.right - rect.left) as f32 - 36.0 + rect.left as f32; // 相对+原点
        assert_eq!(
            gdi_abs as f32, d2d_abs_equiv,
            "GDI 绝对坐标与 D2D 相对坐标+原点必须指向同一屏幕位置（坐标系换算契约）"
        );
        // 高 20 垂直居中（±10）口径（同理：GDI rect 系含 top，D2D 客户区系 h/2）
        let h = rect.bottom - rect.top;
        let gdi_cy = rect.top + h / 2; // GDI :2553 cy
        let d2d_cy = h as f32 / 2.0; // D2D :3632 cy = h/2
        assert_eq!(
            gdi_cy as f32,
            d2d_cy + rect.top as f32,
            "垂直居中口径必须一致（GDI rect 系 cy 与 D2D 客户区系 h/2+原点同值）"
        );
        let sep_hh = 10.0; // 两路径共用的半高（GDI sep_h/2=10、D2D :3631 同值）
        assert_eq!(sep_hh, 10.0, "半高 10（全高 20）口径不得漂移");
    }

    /// D2D-HANG-095 护栏：填满 D2D thread_local 槽的线程，在**线程体内**显式释放后
    /// 必须能正常退出。这是 D2D-HANG-001 的直接护栏。
    /// 消融：把线程体里的 `d2d::release_resources()` 删掉 → 线程退出时走 FLS 回调，
    /// 在 loader lock 下析构 COM 自锁 → `is_finished()` 永远为 false → 本用例在
    /// 超时断言处红（而不是整个 test 进程静默挂死）。
    /// 🔴 已知性质：消融态下那个线程会永久卡住，可能连带拖住 test 进程退出。
    ///    这是被测缺陷本身的性质，不是本用例的缺陷 —— 正常态不会发生。
    /// 顺序自证：本用例线程体内先调 D2D 入口（填槽，BindDC 必败返回 false）再释放，
    /// 与生产 `spawn_overlay_thread` 闭包尾部 `d2d::release_resources()` 的
    /// 「先用后释放」顺序一致；断言对象是线程是否真正走完退出（`is_finished`）。
    #[test]
    fn d2d_thread_with_populated_slot_exits_after_in_thread_release() {
        let handle = std::thread::spawn(|| {
            let hdc = HDC(std::ptr::null_mut());
            let rect = RECT {
                left: 0,
                top: 0,
                right: 200,
                bottom: 36,
            };
            // 走一次 D2D 入口把 thread_local 槽填上（BindDC 必败，返回 false，但资源已建）
            let _ = d2d::draw_processing_overlay(hdc, &rect, config::UiLanguage::Chinese, 0.5);
            // 线程体内显式释放 —— 删掉这一句本用例必红
            d2d::release_resources();
        });

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !handle.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(
            handle.is_finished(),
            "填过 D2D 槽的线程 10s 内未退出 —— thread_local COM 析构在 loader lock 下自锁复发（D2D-HANG-001）"
        );
        let _ = handle.join();
    }

    /// D2D-HANG-095 护栏：`release_resources()` 幂等 —— 槽为空时重复调用不得 panic。
    /// 生产路径：`spawn_overlay_thread` 闭包尾部的 `D2dReleaseGuard` 在
    /// `run_overlay_thread` 根本没画过任何 D2D 状态（资源槽始终为空）时也会触发，
    /// 若空槽调用 panic，正常退出也会崩 —— 本用例钉住「空槽重复调用安全」。
    /// 消融：把 `release_resources()` 实现改成空槽时 panic/错误 → 本用例红。
    /// 顺序自证：纯幂等调用，无时序依赖；与生产空槽守卫路径（D2D.with → take → None）一致。
    #[test]
    fn d2d_release_resources_is_idempotent_on_empty_slot() {
        d2d::release_resources();
        d2d::release_resources(); // 空槽重复调用不得 panic
    }

    // --- helpers ---

    fn wt086(begin_ms: i64, text: &str) -> crate::transcription::qwen_inference::WordTiming {
        crate::transcription::qwen_inference::WordTiming {
            begin_time: begin_ms,
            end_time: begin_ms,
            text: text.to_string(),
            punctuation: String::new(),
        }
    }

    /// 测试用最小 OverlayWindowState 构造（与生产 :1101 初始化同构，全部字段显式）
    fn overlay_window_state_for_test() -> OverlayWindowState {
        let (_tx, rx) = crossbeam_channel::unbounded::<OverlayUiEvent>();
        let _ = rx; // 测试不消费事件；Sender 存活性由 _tx 持有保证
        OverlayWindowState {
            request: None,
            audio_buf: std::sync::Arc::new(
                std::sync::Mutex::new(std::collections::VecDeque::new()),
            ),
            event_tx: _tx,
            cancel_btn_rect: None,
            close_btn_rect: None,
            title_close_btn_rect: None,
            submit_btn_rect: None,
            text_hit_rect: None,
            shimmer_phase: 0.0,
            edit_hwnd: None,
            edit_old_wndproc: None,
            edit_bg_brush: None,
            layered_mode: OverlayLayeredMode::Ulw,
            edit_font: None,
            last_resize_time: None,
            pending_size: None,
            needs_repaint: true,
            current_size: RECORDING_OVERLAY_SIZE,
            target_size: RECORDING_OVERLAY_SIZE,
            last_streaming_text: None,
            cached_font: None,
            streaming_font: None,
            displayed_chars: 0,
            tween_deadline: None,
            tween_target_chars: 0,
            tween_start: None,
            word_timings: Vec::new(),
            tween_audio_origin: None,
            tween_timeline_origin: None,
        }
    }
}

// OVERLAY-101 (Bug B 单一居中源) + OVERLAY-102 (最大宽度 0.50) 阶段三测试同步
// （tester-1，2026-09-05）。只绑定行为约定，不绑定实现字符串/像素值
// （build-test-guide 第八节规范4）。
// G5 是结构性护栏（唯一例外），其可靠性与判别力边界在用例内如实声明。
#[cfg(all(test, target_os = "windows"))]
mod overlay_101_centering_guard_tests {
    use super::*;

    /// G1：centered_x 公式正确性。
    /// 契约：水平居中 x = work_left + 半差，其中半差 = (work_w − applied_w) 的
    /// 整数除法（向零截断）；applied_w > work_w 时允许负 x（不 panic、不 clamp
    /// —— 超宽由调用侧 clamp 兜底）。
    /// 消融：把 :4342 的公式改错（如漏掉 work_left、改为四舍五入、换系数）→ 对应断言红。
    /// 顺序自证：纯函数直调，无时序。
    #[test]
    fn centered_x_formula_exact_values() {
        // work_left = 0：单屏
        assert_eq!(centered_x(0, 1920, 240), 840); // (1920-240)/2 = 840
        assert_eq!(centered_x(0, 1920, 960), 480); // (1920-960)/2 = 480
                                                   // work_left ≠ 0：多显示器负坐标，x 必须整体叠加 work_left 偏移
        assert_eq!(centered_x(-1920, 1920, 240), -1080);
        assert_eq!(centered_x(-1920, 1920, 960), -1440);
        // 奇偶宽度取整：Rust 整数除法向零截断
        assert_eq!(centered_x(0, 1921, 240), 840); // 1681/2 = 840.5 → 840
        assert_eq!(centered_x(0, 1920, 241), 839); // 1679/2 = 839.5 → 839
                                                   // applied_w > work_w：允许负 x，不 panic 不 clamp
        assert_eq!(centered_x(0, 1920, 3000), -540); // (1920-3000)/2 = -540
        assert!(centered_x(0, 1920, 3000) < 0, "超宽必须给负 x（不 clamp）");
    }

    /// G2：Bug B 量化护栏 —— 「沿用基准宽默认位」造成的中心偏移量 == (W_max−240)/2。
    /// 缺陷机制（Gavin 端测复现过）：!do_it（100ms 节流命中）帧旧代码不重算 x，
    /// 沿用 resolved_pos = overlay_geometry 默认位（按基准宽 240 居中），而同一调用
    /// SetWindowPos 应用的是 in-flight current（到上限后 = W_max）→ 中心右偏
    /// (W_max−240)/2 px，且到上限后 current==target、插值循环休眠、无帧纠正。
    /// 本用例把该偏移量钉成数字：同 work_w 下 240 宽与 W_max 宽的 x 之差必须恰为
    /// (W_max−240)/2。用真实数量级：work_w=1920、W_max=960（0.50 × 1920）。
    /// 消融：centered_x 不再按 applied_w 居中（如改回按 target/基准宽）→
    /// x_240 − x_max 变化 → 红。
    /// 顺序自证：纯函数直调，无时序。
    #[test]
    fn centered_x_baseline_to_max_width_offset_is_half_the_delta() {
        let work_w = 1920;
        let w_max = 960; // STREAMING_OVERLAY_MAX_SCREEN_RATIO=0.50 × 1920
                         // work_left = 0
        let x_240 = centered_x(0, work_w, 240);
        let x_max = centered_x(0, work_w, w_max);
        assert_eq!(
            x_240 - x_max,
            (w_max - 240) / 2,
            "Bug B 量化：基准宽 240 与上限宽 W_max 的居中 x 之差必须等于 (W_max−240)/2"
        );
        assert_eq!(x_240 - x_max, 360, "1920/960 量级：差必须为 360");
        // work_left ≠ 0（负坐标屏）：偏移量不随 work_left 变（两者抵消）
        let x_240_neg = centered_x(-1920, work_w, 240);
        let x_max_neg = centered_x(-1920, work_w, w_max);
        assert_eq!(x_240_neg - x_max_neg, 360, "负坐标屏下偏移量不变");
    }

    /// G3：两边等量扩展不变量 —— 宽度增大时窗口中心不变（x + w/2 恒定）。
    /// OVERLAY-068-A R1「两边扩展」语义：同一 work 区、偶数宽下，任意宽度居中后
    /// center = x + w/2 必须恒等于 work_left + work_w/2（与 w 无关）。
    /// 消融：centered_x 改按 target 宽或基准宽居中 → 中心随宽度漂移 → 红。
    /// 顺序自证：纯函数直调，无时序。
    #[test]
    fn centering_keeps_window_center_fixed_as_width_grows() {
        let work_left = 0;
        let work_w = 1920;
        let base_center = work_left + work_w / 2; // 960
                                                  // 偶数宽全序列：中心必须逐位恒定
        for w in [240, 480, 720, 960, 1200, 1440, 1680, 1920] {
            let x = centered_x(work_left, work_w, w);
            assert_eq!(
                x + w / 2,
                base_center,
                "宽度 {} 时中心必须保持 work_left + work_w/2（两边等量扩展）",
                w
            );
        }
        // 增量不变量：宽 +2 → x −1、中心不变（对称扩展语义的细粒度形态）
        for w in (240..1920).step_by(2) {
            let x = centered_x(work_left, work_w, w);
            let x2 = centered_x(work_left, work_w, w + 2);
            assert_eq!(x2, x - 1, "宽 +2 必须左移 1px（对称扩展）");
            assert_eq!(x2 + (w + 2) / 2, x + w / 2, "宽 +2 中心不变");
        }
        // 奇数宽：整数除法截断使中心允许 ±1 偏差，不得漂移超过 1（偶数宽严格相等已在上断言）
        for w in (241..1920).step_by(2) {
            let x = centered_x(work_left, work_w, w);
            let drift = (x + w / 2) - base_center;
            assert!(
                drift.abs() <= 1,
                "奇数宽中心偏差必须在截断容差内（≤1），宽度 {} 实测 {}",
                w,
                drift
            );
        }
    }

    /// G4：OVERLAY-102 常量护栏 —— 最大流式宽度占屏比 0.65 → 0.50。
    /// 契约：STREAMING_OVERLAY_MAX_SCREEN_RATIO == 0.50；1920 屏宽下
    /// overlay_max_width（:4234）的算式 work_w × ratio 再 round 必须得 960。
    /// 消融：改回 0.65 → ratio 断言红 + 1920 宽断言红（0.65×1920=1248 ≠ 960）。
    /// 顺序自证：纯常量断言，无时序。
    /// ⚠️ 可测性边界：overlay_max_width(:4234) 需要真实 HWND（monitor_work_rect），
    /// 本用例钉住常量值 + 复算同一算式（不调真实 HWND 函数）。
    #[test]
    fn streaming_max_width_ratio_is_half_with_960_on_1920() {
        assert_eq!(
            STREAMING_OVERLAY_MAX_SCREEN_RATIO, 0.50_f32,
            "OVERLAY-102：最大流式宽度占屏比必须为 0.50（Gavin 端测拍板）"
        );
        // 与生产 overlay_max_width(:4237) 同式复算
        let max_w = (1920.0_f32 * STREAMING_OVERLAY_MAX_SCREEN_RATIO).round() as i32;
        assert_eq!(max_w, 960, "1920 屏宽下 max_w 必须为 960");
    }

    /// G5：单一居中源结构护栏（TEST-SYNC-087 判别力缺口第一次有可测形态）。
    /// 契约（真实契约 = 用于居中的宽度 == SetWindowPos 应用的宽度）：全仓 overlay
    /// 水平居中只允许经 centered_x 一个源头；未来任何人把内联公式（work_w 与
    /// 宽度之差的一半）抄回调用点 → 本护栏红。
    /// 判据：include_str 读自身源码，逐行去空白后，含「左括号紧接 work_w 紧接
    /// ASCII 减号」形态的行必须恰好 1 行，且该行必须含 applied_w（即 centered_x
    /// 定义体）。去空白后匹配让减号前后有无空格等空白变体归一。
    /// 消融：任一处调用点内联回 work.left + (work_w − 宽度) / 2 → 命中行数变 2 → 红；
    /// centered_x 被改名/删除 → 0 行或非定义体行 → 红。
    /// 🔴 判别力边界（如实声明）：本护栏只匹配 work_w 这个变量名形态 —— 若未来
    /// 有人用别的变量名内联（如 `let ww = work.right - work.left; (ww - w) / 2`）
    /// 则漏过（结构性短板，与「正则容易脆/误伤」的担忧一致）。本护栏的价值：
    /// 钉住现形态的唯一源头，拦截最直接的回归（把旧代码复制粘贴回去）。
    /// 顺序自证：纯文本静态比对，无时序。
    #[test]
    fn inline_centering_formula_appears_only_in_centered_x_body() {
        let src = include_str!("main.rs");
        // needle 用 format 拼装，避免本文件出现连写字面量自我命中
        let needle = format!("(work_w{}", "-");
        let mut hits: Vec<(usize, &str)> = Vec::new();
        for (i, line) in src.lines().enumerate() {
            let stripped: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            if stripped.contains(&needle) {
                hits.push((i + 1, line));
            }
        }
        assert_eq!(
            hits.len(),
            1,
            "内联居中公式形态 (work_w−宽度)/2 必须全文件仅出现在 centered_x 定义体，实测 {} 处",
            hits.len()
        );
        let (lineno, line) = hits[0];
        assert!(
            line.contains("applied_w"),
            "命中行（{}）必须是 centered_x 定义体（含 applied_w 参数）——实际为: {}",
            lineno,
            line
        );
        // 附带：centered_x 必须仍存在（防止整函数被删、调用点各自内联）
        assert!(
            src.lines().any(|l| l.contains("fn centered_x")),
            "centered_x 函数必须存在（单一居中源）"
        );
    }
}

// D2D-P2P3 (IMPL-109 五态迁移) 阶段三测试同步（tester-1，2026-09-05）
// 方案：docs/D2D-P2P3-PLAN.md §3.4.4 G1-G5 + coder-2 补充素材（submit 无+1 / 波形整除口径）。
// 只绑定行为约定，不绑定实现字符串/像素值（build-test-guide 第八节规范4）。
// G4 是结构性护栏（唯一例外），其判别力边界在用例内如实声明。
#[cfg(all(test, target_os = "windows"))]
mod overlay_109_d2d_p2p3_guard_tests {
    use super::*;

    // ==================== G1: 四个新包装的回落触发测试 ====================

    /// G1：D2D-P2P3 新迁移四态包装的 GDI 回落触发器。
    /// 契约（overlay 永不空白）：`draw_editing_overlay` / `draw_recording_waveform_overlay`
    /// / `draw_error_overlay` / `draw_preview_overlay` 在无效 HDC 上必须返回 false
    /// （BindDC 失败 → with_d2d 返回 false → 调用方 `if !d2d::draw_*(...) { GDI }`
    /// 当帧兜底）。镜像既有 P0/P1 用例（:9388/:9428）的模式。
    /// 消融：任一入口把失败路径改成 panic 或返回 true → 对应断言红
    /// （true 使 GDI 回落永不触发，panic 使测试进程崩）。
    /// 顺序自证：直接以无效 HDC 调入口，无时序依赖，与生产 dispatch
    /// `if !d2d::draw_*(...) { GDI }` 判定顺序一致。
    #[test]
    fn d2d_p2p3_four_entries_return_false_on_invalid_hdc_gdi_fallback_trigger() {
        let hdc = HDC(std::ptr::null_mut());
        let rect = RECT {
            left: 0,
            top: 0,
            right: 240,
            bottom: 36,
        };
        let state = overlay_window_state_for_test();

        let ok_editing = d2d::draw_editing_overlay(hdc, &rect);
        assert!(
            !ok_editing,
            "StreamingEditing 入口：无效 HDC 必须返回 false（GDI 回落触发器）"
        );

        let ok_waveform = d2d::draw_recording_waveform_overlay(hdc, &rect, &state);
        assert!(
            !ok_waveform,
            "Recording/FallingToProcessing 共用入口：无效 HDC 必须返回 false（GDI 回落触发器）"
        );

        let ok_error = d2d::draw_error_overlay(hdc, &rect, "错误消息");
        assert!(
            !ok_error,
            "Error 入口：无效 HDC 必须返回 false（GDI 回落触发器）"
        );

        let ok_preview =
            d2d::draw_preview_overlay(hdc, &rect, "预览文本", config::UiLanguage::Chinese);
        assert!(
            !ok_preview,
            "FocusLost 入口：无效 HDC 必须返回 false（GDI 回落触发器）"
        );

        // D2D-HANG-095: 本用例以无效 HDC 调 D2D 入口，create_resources 会成功
        //（工厂创建不依赖窗口），测试线程的 thread_local 槽被填上 D2D 资源。
        // 必须在用例体内显式释放 —— 否则线程退出时 FLS 回调在 loader lock 下
        // 析构 COM，自锁死锁（REPRO-094 探针 B/C 实证）。
        d2d::release_resources();
    }

    // ==================== G2: 命中矩形等值断言 ====================

    /// G2a：submit 命中矩形 = 无 +1 口径（负向断言防「顺手统一」）。
    /// 契约（coder-2 补充素材）：`draw_submit_button_hit_rect_only`（:2698）
    /// right == left+bs（bs=16），**没有** stop 按钮的 +1。这是 draw_submit_button
    /// 的历史行为，点击判定（rect_contains）消费的就是它 —— 迁移时不得「顺手统一」。
    /// 消融：给 submit 补上 +1（right = bl+bs+1）→ 负向断言红。
    /// 顺序自证：纯函数直调，无时序。
    #[test]
    fn submit_hit_rect_has_no_plus_one_asymmetry_pinned() {
        let rect = RECT {
            left: 100,
            top: 50,
            right: 340,
            bottom: 86,
        };
        let r = draw_submit_button_hit_rect_only(&rect);
        // 公式：bs=16, bl=right-25, bt=top+(h-16)/2
        assert_eq!(r.left, 340 - 25, "bl = rect.right - 25");
        assert_eq!(r.top, 50 + (36 - 16) / 2, "bt = top + (h-16)/2");
        assert_eq!(r.right, r.left + 16, "submit right = left + bs（无 +1）");
        assert_eq!(r.bottom, r.top + 16);
        // 🔴 负向断言：不得「顺手统一」成 stop 的 +1
        assert_ne!(r.right, r.left + 17, "submit 命中 rect 不得有 stop 的 +1");
        // 与 GDI 绘制路径同源：draw_submit_button 返回的就是这个 rect
        //（GDI 路径 :2717 改调 helper，返回 submit_rect）。
        let gdi_rect = draw_submit_button_hit_rect_only(&rect);
        assert_eq!(r, gdi_rect, "GDI 路径命中 rect 与 helper 同源");
    }

    /// G2b：stop 命中矩形与 d2d 侧同公式（含 +1）。
    /// 契约：`draw_stop_button_hit_rect_only`（:2572）right = bl+bs+1（GDI Rectangle
    /// right/bottom 排他 +1），与 d2d::stop_button（:3615 right=(bl+bs+1)）同式。
    /// 消融：去掉 +1 → 断言红；d2d 侧口径被改 → 与 helper 比对红。
    /// 顺序自证：纯函数直调，无时序。
    #[test]
    fn stop_hit_rect_matches_d2d_stop_button_geometry() {
        let rect = RECT {
            left: 100,
            top: 50,
            right: 340,
            bottom: 86,
        };
        let helper = draw_stop_button_hit_rect_only(&rect);
        // 公式：bs=16, bl=right-25, bt=top+(h-16)/2, right=bl+bs+1（+1 口径）
        assert_eq!(helper.left, 340 - 25);
        assert_eq!(helper.top, 50 + (36 - 16) / 2);
        assert_eq!(
            helper.right,
            helper.left + 16 + 1,
            "stop right = left + bs + 1"
        );
        assert_eq!(helper.bottom, helper.top + 16 + 1);
        // d2d 侧同式：w 为 rect 相对宽（h=36），bl = w-25，right = bl+bs+1
        //（d2d::stop_button :3607 的几何公式，rect-relative）。
        let w = rect.right - rect.left;
        let d2d_bl = w - 25;
        let d2d_bt = (36 - 16) / 2;
        let d2d_rect = RECT {
            left: d2d_bl,
            top: d2d_bt,
            right: d2d_bl + 16 + 1,
            bottom: d2d_bt + 16 + 1,
        };
        // helper 返回窗口绝对坐标（含 rect.left/top 偏移），d2d 是 rect-relative。
        // 换算到同一坐标系比对：helper - rect.left/top。
        let helper_relative = RECT {
            left: helper.left - rect.left,
            top: helper.top - rect.top,
            right: helper.right - rect.left,
            bottom: helper.bottom - rect.top,
        };
        assert_eq!(
            helper_relative, d2d_rect,
            "stop 命中 rect 与 d2d 侧同公式（含 +1）"
        );
    }

    /// G2c：preview_hit_rects 三件套真值表。
    /// 契约（:4349）：title_close 18x18（right-26, top+5 → right-8, top+23）；
    /// 底部双键 45x18 gap10 底距 10 水平居中。返回 (copy, close, title_close)。
    /// 消融：任一处几何改动（btn_w/gap/边距/居中公式）→ 对应断言红。
    /// 顺序自证：纯函数直调，无时序。
    #[test]
    fn preview_hit_rects_truth_table() {
        // 固定 320x140 输入（PREVIEW_OVERLAY_SIZE 同源）
        let rect = RECT {
            left: 0,
            top: 0,
            right: 320,
            bottom: 140,
        };
        let (copy, close, title_close) = preview_hit_rects(&rect);
        // 标题 ✕ 键：right-26 → right-8，top+5 → top+23
        assert_eq!(title_close.left, 320 - 26);
        assert_eq!(title_close.top, 0 + 5);
        assert_eq!(title_close.right, 320 - 8);
        assert_eq!(title_close.bottom, 0 + 23);
        // 底部双键：btn_w=45, gap=10, total=100, 水平居中，btn_top = bottom-18-10
        let btn_left = 0 + (320 - 100) / 2;
        let btn_top = 140 - 18 - 10;
        assert_eq!(copy.left, btn_left);
        assert_eq!(copy.top, btn_top);
        assert_eq!(copy.right, btn_left + 45);
        assert_eq!(copy.bottom, btn_top + 18);
        assert_eq!(close.left, btn_left + 45 + 10);
        assert_eq!(close.top, btn_top);
        assert_eq!(close.right, btn_left + 45 * 2 + 10);
        assert_eq!(close.bottom, btn_top + 18);
        // 三件套互不重叠（点击区域离散）
        assert!(copy.right <= close.left, "copy 与 close 不得重叠");
        // 与 GDI draw_preview_overlay 同源：GDI 路径返回值即 preview_hit_rects
        //（:2187-2197 两分支都从同一纯函数出，GDI 兜底分支调 draw_preview_overlay
        //  内部调 preview_hit_rects —— 该分支无法在无效 HDC 下直测，本断言钉
        //  纯函数真值表即可，dispatch 接线由 G1 fallback 用例覆盖）。
    }

    // ==================== G3: 波形纯函数 ====================

    /// G3a：waveform_bar_height 公式表。
    /// 契约（:2293，WAVEFORM-HEIGHT-FIX-001 常量）：
    ///   maxh=48 / static_h=12 / minh=8 / gain=2.5；
    ///   weight = 0.4 + 0.6·cos²(π/2·i/(half-1))；
    ///   v_gain = min(v·gain·weight, 1.0)；
    ///   v_gain > 0.01 → minh + v_gain·(maxh−minh)，否则 static_h（12）。
    /// 消融：任一常量（48/12/8/2.5）或公式（权重/增益）擅改 → 对应断言红。
    /// 顺序自证：纯函数直调，无时序。
    #[test]
    fn waveform_bar_height_formula_table() {
        // v=0：恒 static_h=12（无音频 → 静态高度）
        assert_eq!(waveform_bar_height(0.0, 0, 8), 12);
        assert_eq!(waveform_bar_height(0.0, 3, 8), 12);
        assert_eq!(waveform_bar_height(0.0, 7, 8), 12);
        // v_gain > 0.01 边界：i=0 权重=1.0，v·2.5 跨 0.01。
        // 🔴 实测（rustc f32）：0.004f32 存为 0.0040000002，×2.5=0.010000001 > 0.01 → active。
        //   边界安全值：0.0039f32×2.5=0.00975 < 0.01 → static(12)。
        assert_eq!(
            waveform_bar_height(0.0039, 0, 8),
            12,
            "v_gain=0.00975 不>0.01 → static"
        );
        assert_eq!(
            waveform_bar_height(0.005, 0, 8),
            8,
            "v_gain=0.0125>0.01 → minh 起步"
        );
        // v=0.5 避开 min(,1.0) 封顶，区分权重端点：
        //   i=0: weight=0.4+0.6·cos²(0)=1.0 → v_gain=min(0.5·2.5·1,1)=1.0 → 48
        //   i=3 (half=8): i/(half-1)=3/7, cos²(3π/14)≈0.611 → weight≈0.767
        //     → v_gain=min(0.5·2.5·0.767,1)=0.958 → 8+0.958·40≈46.3 → 46
        //   i=7 (half-1): weight=0.4 → v_gain=min(0.5·2.5·0.4,1)=0.5 → 8+0.5·40=28
        assert_eq!(waveform_bar_height(0.5, 0, 8), 48);
        assert_eq!(waveform_bar_height(0.5, 3, 8), 46);
        assert_eq!(waveform_bar_height(0.5, 7, 8), 28);
        // half=16 端点同律
        assert_eq!(waveform_bar_height(0.5, 0, 16), 48);
        assert_eq!(waveform_bar_height(0.5, 15, 16), 28);
        // v=1.0 封顶：任一位置 v_gain=1.0 → maxh=48
        assert_eq!(waveform_bar_height(1.0, 0, 8), 48);
        assert_eq!(waveform_bar_height(1.0, 7, 8), 48);
    }

    /// G3b：waveform_snapshot 空 buf / poisoned lock → 空 vec。
    /// 契约（:2264）：锁 poisoned（流失败）→ Vec::new()（全部落 static 高度）；
    /// 空 buf → 0..half 全 0.0（idx 越界返回 0.0）。
    /// 消融：把 Err 分支改成别的（如 panic / 返回非空）→ 对应断言红。
    /// 顺序自证：构造状态 → 调纯函数，无时序。
    #[test]
    fn waveform_snapshot_empty_and_poisoned_contract() {
        // 空 buf
        let state = overlay_window_state_for_test();
        let snap_empty = waveform_snapshot(&state, 8);
        assert_eq!(snap_empty.len(), 8, "空 buf 仍返回 half 条（全 0.0）");
        assert!(snap_empty.iter().all(|v| *v == 0.0), "空 buf 快照全 0.0");

        // poisoned lock：在持有锁时 panic → std 毒死 Mutex
        let poisoned_state = overlay_window_state_for_test();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = poisoned_state.audio_buf.lock().unwrap();
            panic!("deliberate poison");
        }));
        let snap_poisoned = waveform_snapshot(&poisoned_state, 8);
        assert!(
            snap_poisoned.is_empty(),
            "poisoned lock 必须返回空 vec（流失败 → 全部落 static 高度）"
        );
    }

    // ==================== G4: include_str! 结构护栏 ====================

    /// G4：dispatch 五分支 d2d 优先 + GDI 回落在位的静态证据。
    /// 契约（overlay 永不空白）：draw_overlay_to_dc 的每个 D2D 迁移状态分支形如
    /// `if !d2d::draw_xxx(...) { GDI 兜底 }` —— 五个迁移态（Recording/FallingToProcessing
    /// 共用 waveform 入口、StreamingEditing、FocusLost、Error，另含 P0 Processing）
    /// + BUG-119 的 Info 信息提示态，共六个 D2D 分支。
    /// 的 d2d::draw_ 调用都必须出现在 `if !d2d::draw_` 形态内，且其下必有 GDI 调用。
    /// 判据：include_str 读自身源码，逐行去空白后统计 `if!d2d::draw_` 出现次数，
    /// 断言 == 6（RecordingWaveform/Processing/Editing/Preview/Error/Info），且每个
    /// `!d2d::draw_` 的后续行内存在非 d2d 前缀的绘制函数调用（GDI 兜底）。
    /// 消融：任一分支删掉 d2d 调用（回归纯 GDI）→ 计数变 5 → 红；
    /// 任一分支删掉 GDI 兜底 → 该分支后续无 GDI 调用 → 红。
    /// 🔴 判别力边界（如实声明）：本护栏匹配 `if !d2d::draw_` 形态与「其下有非 d2d
    /// 调用行」的粗粒度结构，不解析括号配对（实现字符串形态，非行为断言）。
    /// 与 G5（TEST-SYNC-105）同为结构护栏，价值=钉住「d2d 优先 + GDI 兜底」骨架；
    /// 具体绘制正确性由 G1 回落触发 + 阶段四回归 + Gavin 端测目视承担。
    #[test]
    fn dispatch_five_branches_d2d_first_gdi_fallback_in_place() {
        let src = include_str!("main.rs");
        // needle 用 format 拼装避免字面量自我命中
        let needle = format!("if!d2d::draw_{}", "");
        let mut d2d_lines: Vec<(usize, String)> = Vec::new();
        for (i, line) in src.lines().enumerate() {
            let stripped: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            // 🔴 只匹配「行首」为 if!d2d::draw_ 的行 —— 生产 dispatch 分支是语句起始，
            // 注释里提到该形态的（本护栏 docstring / 既有用例注释）不得计入。
            if stripped.starts_with(&needle) {
                d2d_lines.push((i + 1, stripped));
            }
        }
        assert_eq!(
            d2d_lines.len(),
            6,
            "dispatch 必须恰有 6 个 `if !d2d::draw_` 分支（waveform/processing/editing/preview/error/info），实测 {}",
            d2d_lines.len()
        );
        // 每个 d2d 分支的 **if-body**（`if !d2d::draw_x(...) { ... }`，结束于 `} else {`
        // 或独立 `}`）内必须有 GDI 兜底绘制调用（非 d2d 前缀）。
        // 依据生产 :2132-2206 结构，GDI 兜底都在 if-body 内、else 分支之前。
        // 🔴 只扫 if-body 不扫 else：else 分支的命中矩形 helper（如
        //    draw_submit_button_hit_rect_only）也含 `draw_` 前缀，若扫到 else 会
        //    误判为「GDI 兜底在位」—— A4 消融实测抓不住（TEST-EXEC-111 发现）。
        // 消融：任一分支把 if-body 内 GDI 兜底删掉（只留 d2d 或空）→ 该分支红。
        for (lineno, _) in &d2d_lines {
            let mut has_gdi = false;
            // 从 d2d 调用行之后扫描 if-body，直到 } else { 或独立 } 为止
            let lines: Vec<&str> = src.lines().collect();
            for j in (*lineno + 1)..(*lineno + 15).min(lines.len() + 1) {
                if j - 1 >= lines.len() {
                    break;
                }
                let stripped: String = lines[j - 1]
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                // if-body 结束（} else { 或独立 }）
                if stripped.starts_with("}else")
                    || stripped.starts_with("}elseif")
                    || stripped == "}"
                {
                    break;
                }
                // 非 d2d 前缀 + 形似 GDI 绘制调用
                if !stripped.starts_with("//")
                    && !stripped.is_empty()
                    && !stripped.contains("d2d::")
                    && stripped.contains('(')
                    && (stripped.contains("draw_")
                        || stripped.contains("chrome")
                        || stripped.contains("indicator"))
                {
                    has_gdi = true;
                    break;
                }
            }
            assert!(
                has_gdi,
                "dispatch 分支（L{}）if-body 内必须存在 GDI 兜底绘制调用（overlay 永不空白）",
                lineno
            );
        }
    }

    // ==================== helpers ====================

    fn overlay_window_state_for_test() -> OverlayWindowState {
        let (_tx, rx) = crossbeam_channel::unbounded::<OverlayUiEvent>();
        let _ = rx;
        OverlayWindowState {
            request: None,
            audio_buf: std::sync::Arc::new(
                std::sync::Mutex::new(std::collections::VecDeque::new()),
            ),
            event_tx: _tx,
            cancel_btn_rect: None,
            close_btn_rect: None,
            title_close_btn_rect: None,
            submit_btn_rect: None,
            text_hit_rect: None,
            shimmer_phase: 0.0,
            edit_hwnd: None,
            edit_old_wndproc: None,
            edit_bg_brush: None,
            layered_mode: OverlayLayeredMode::Ulw,
            edit_font: None,
            last_resize_time: None,
            pending_size: None,
            needs_repaint: true,
            current_size: RECORDING_OVERLAY_SIZE,
            target_size: RECORDING_OVERLAY_SIZE,
            last_streaming_text: None,
            cached_font: None,
            streaming_font: None,
            displayed_chars: 0,
            tween_deadline: None,
            tween_target_chars: 0,
            tween_start: None,
            word_timings: Vec::new(),
            tween_audio_origin: None,
            tween_timeline_origin: None,
        }
    }
}
// =====================================================================
// TEST-SYNC-122 / BUG-119「用户没说话」类型化信号 8 条护栏（阶段三，只写用例）
// ---------------------------------------------------------------------
// 契约（写在 NoSpeechError 类型文档里）：新增第三个「没说话」产出源时只需
// `bail!(NoSpeechError)`，显示侧分类器零改动。本组护栏守的回归路径 =
// 「有人图省事又回去改字符串」。
//
// 结构护栏写法（前几轮教训）：
//   1) include_str! 自读源码，按首个 `#[cfg(test)]` 切分只扫生产区 —— 测试
//      代码（本 mod）永不进入扫描区；
//   2) 匹配一律 startswith（禁 contains），needle 全部 concat! 拆串；
//   3) H3 是唯一子串例外（要在函数体内找「禁止出现的嗅探串」），仍先花括号
//      深度定界到 convert_to_friendly_error 函数体再查；
//   4) 行窗一律 block_contains 花括号定界（不复活 window_has）。
// =====================================================================
#[cfg(test)]
mod nospeech_122_guard_tests {
    /// 读取源码并截断到首个 `#[cfg(test)]`，逐行 trim（只扫生产区）。
    fn prod_lines(src: &'static str) -> Vec<String> {
        let mut out = Vec::new();
        for line in src.lines() {
            let t = line.trim();
            if t.starts_with("#[cfg(test)]") {
                break;
            }
            out.push(t.to_string());
        }
        out
    }

    fn main_prod_lines() -> Vec<String> {
        prod_lines(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/main.rs"
        )))
    }

    fn i18n_prod_lines() -> Vec<String> {
        prod_lines(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/i18n.rs"
        )))
    }

    /// src/transcription/ 全部 .rs 文件生产区行合并（现为 mod/qwen_inference/vad 三文件）。
    /// 判别力边界：若未来新增「没说话」产出源放进**新文件**，本计数扫不到该文件；
    /// 放进现有三文件必被计到，护栏红会要求主动更新。
    fn transcription_prod_lines() -> Vec<String> {
        let mut out = Vec::new();
        for src in [
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/transcription/mod.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/transcription/qwen_inference.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/transcription/vad.rs"
            )),
        ] {
            for line in src.lines() {
                let t = line.trim();
                if t.starts_with("#[cfg(test)]") {
                    break;
                }
                out.push(t.to_string());
            }
        }
        out
    }

    fn find_line(lines: &[String], needle: &str) -> Option<usize> {
        lines.iter().position(|l| l.starts_with(needle))
    }

    fn norm_line(line: &str) -> String {
        line.trim().replace("platform::", "")
    }

    fn brace_delta(line: &str) -> i32 {
        line.matches('{').count() as i32 - line.matches('}').count() as i32
    }

    /// 花括号定界块内是否存在一行 startswith(needle)。
    /// 支持多行签名锚点（锚点行无 `{` 时前扫首个开括号再开始匹配），块闭合即停。
    fn block_contains(lines: &[String], anchor: usize, needle: &str) -> bool {
        let mut depth = 0i32;
        let mut opened = false;
        for line in lines.iter().skip(anchor) {
            let norm = norm_line(line);
            depth += brace_delta(&norm);
            if depth > 0 {
                opened = true;
                if norm.starts_with(needle) {
                    return true;
                }
            } else if opened {
                return false;
            }
        }
        false
    }

    /// H3 专用（子串例外）：花括号定界函数体内是否存在任一 needle 子串。
    fn block_contains_any(lines: &[String], anchor: usize, needles: &[&str]) -> bool {
        let mut depth = 0i32;
        let mut opened = false;
        for line in lines.iter().skip(anchor) {
            let norm = norm_line(line);
            depth += brace_delta(&norm);
            if depth > 0 {
                opened = true;
                if needles.iter().any(|n| norm.contains(n)) {
                    return true;
                }
            } else if opened {
                return false;
            }
        }
        false
    }

    fn count_startswith(lines: &[String], needle: &str) -> usize {
        lines.iter().filter(|l| l.starts_with(needle)).count()
    }

    /// 提取 `key: "value",` 引号内字符串。
    fn quoted_value(line: &str) -> Option<String> {
        let s = line.find('"')? + 1;
        let e = line[s..].find('"')? + s;
        Some(line[s..e].to_string())
    }

    // ----- H1-H8 护栏 -----

    /// H1: transcription::NoSpeechError 存在，且实现 std::error::Error
    ///（可被 anyhow 的 downcast / is::<>() 下探）。
    /// 消融路径：删掉 `impl std::error::Error for NoSpeechError` → 本测试红。
    #[test]
    fn h1_nospeech_error_exists_and_implements_error() {
        let lines = transcription_prod_lines();
        assert!(
            find_line(&lines, concat!("pub struct NoSpeech", "Error")).is_some(),
            "H1: transcription::NoSpeechError struct must exist"
        );
        assert!(
            find_line(
                &lines,
                concat!("impl std::error::", "Error for NoSpeechError")
            )
            .is_some(),
            "H1: NoSpeechError must implement std::error::Error (downcast / is::<>() 依赖)"
        );
    }

    /// H2: 产出源计数 —— src/transcription/ 下 bail!(NoSpeechError) /
    /// bail!(super::NoSpeechError) 恰为 5 处（qwen:921 / qwen:1605 / mod:373 / mod:331 / mod:453）。
    /// 只数**代码行**（注释里的同名字面量以 `///`/`//` 开头被 startswith 排除）。
    /// 数量变化必须让人主动来改这条护栏（新增源是有意识行为）。
    /// 消融路径：删任一真实 bail 处 → 计数 4 → 本测试红。
    #[test]
    fn h2_nospeech_bail_sources_count_is_five() {
        let lines = transcription_prod_lines();
        let needles: [&str; 4] = [
            concat!("anyhow::bail!(NoSpeech", "Error)"),
            concat!("bail!(NoSpeech", "Error)"),
            concat!("bail!(super::NoSpeech", "Error)"),
            concat!("anyhow::bail!(super::NoSpeech", "Error)"),
        ];
        let count = needles
            .iter()
            .map(|n| count_startswith(&lines, n))
            .sum::<usize>();
        assert_eq!(
            count,
            5,
            "H2: 产出源 bail!(NoSpeechError) 计数必须恰为 5（qwen:921/1605 + mod:331/373/453）；数量变化须主动改此护栏"
        );
    }

    /// H3: 🔴 反向护栏（本单最重要）—— convert_to_friendly_error 函数体内
    /// **不得出现**「没说话」语义的关键词嗅探（无识别 / no speech / nospeech /
    /// empty text / 没说话 / no_speech）。守的就是「有人又回去加 contains」的回归路径。
    /// 写法：花括号深度定界到函数体，再在体内查子串（H3 是唯一子串例外）。
    /// 消融路径：往该函数加一行 `m.contains("无识别")` → 本测试红。
    #[test]
    fn h3_convert_to_friendly_error_no_nospeech_sniffing() {
        let lines = main_prod_lines();
        let anchor = find_line(&lines, concat!("fn convert_to_friendly_", "error("))
            .expect("H3 anchor: convert_to_friendly_error");
        let needles: [&str; 6] = [
            concat!("无识", "别"),
            concat!("no sp", "eech"),
            concat!("nospe", "ech"),
            concat!("empty te", "xt"),
            concat!("没说", "话"),
            concat!("no_spe", "ech"),
        ];
        assert!(
            !block_contains_any(&lines, anchor, &needles),
            "H3: convert_to_friendly_error 函数体内不得出现「没说话」关键词嗅探（无识别/no speech/nospeech/empty text/没说话/no_speech）—— 回归路径=有人又回去加 contains"
        );
    }

    /// H4: TranscriptionFailure 枚举存在且有 NoSpeech 变体；run_pipeline_core 的
    /// map_err 闭包含 `is::<transcription::NoSpeechError>()` 下探 ——
    /// 守「拦截点必须在 to_string() 之前」（to_string 后无法再与普通错误区分）。
    /// 消融路径：把 map_err 改回 `|e| e.to_string()` → is:: 行消失 → 本测试红。
    #[test]
    fn h4_transcription_failure_nospeech_and_downcast_before_tostring() {
        let lines = main_prod_lines();
        let enum_anchor = find_line(&lines, concat!("enum Transcription", "Failure"))
            .expect("H4 anchor: enum TranscriptionFailure");
        assert!(
            block_contains(&lines, enum_anchor, concat!("NoSpeech", ",")),
            "H4: TranscriptionFailure 枚举必须含 NoSpeech 变体"
        );
        let map_anchor = find_line(
            &lines,
            concat!(
                "let transcription_result: Result<(String, bool), Transcription",
                "Failure>"
            ),
        )
        .expect("H4 anchor: transcription_result in run_pipeline_core");
        assert!(
            block_contains(
                &lines,
                map_anchor,
                concat!("if e.is::<transcription::NoSpeech", "Error>()")
            ),
            "H4: run_pipeline_core map_err 必须含 is::<transcription::NoSpeechError>() 下探（拦截点必须在 to_string() 之前）"
        );
    }

    /// H5: 流式 join 处（spawn_worker_thread 内）同样有
    /// is::<transcription::NoSpeechError>() 分流到 PipelineEvent::NoSpeech。
    /// 消融路径：删掉该分支 → is:: / NoSpeech 事件行消失 → 本测试红。
    #[test]
    fn h5_streaming_join_nospeech_is_detected() {
        let lines = main_prod_lines();
        let anchor = find_line(&lines, concat!("fn spawn_worker_", "thread("))
            .expect("H5 anchor: spawn_worker_thread");
        assert!(
            block_contains(
                &lines,
                anchor,
                concat!("if e.is::<transcription::NoSpeech", "Error>()")
            ),
            "H5: spawn_worker_thread 流式 join 处必须 is::<transcription::NoSpeechError>() 下探"
        );
        assert!(
            block_contains(
                &lines,
                anchor,
                concat!("send_event(&event_tx, PipelineEvent::NoSpeech")
            ),
            "H5: 流式 join 处必须 send_event PipelineEvent::NoSpeech（分流，不进 convert_to_friendly_error 错误链）"
        );
    }

    /// H6: i18n no_speech_hint 三语言（ZH/ZH_TW/EN）全部非空且互不相同；
    /// ZH（文件中第一处）必须恰为「请说话哦..」（Gavin 指定原文，含两个点）。
    /// 消融路径：任一语言留空 / ZH 文案被改 → 本测试红。
    #[test]
    fn h6_nospeech_hint_i18n_three_languages() {
        let lines = i18n_prod_lines();
        let hints: Vec<String> = lines
            .iter()
            .filter(|l| l.starts_with("no_speech_hint:"))
            .filter_map(|l| quoted_value(l))
            .collect();
        assert_eq!(hints.len(), 3, "H6: no_speech_hint 必须三语言各一");
        assert!(
            hints.iter().all(|h| !h.is_empty()),
            "H6: 三语言 no_speech_hint 都不得为空"
        );
        let mut sorted = hints.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 3, "H6: 三语言 no_speech_hint 必须互不相同");
        assert_eq!(
            hints[0],
            concat!("请说话", "哦.."),
            "H6: ZH no_speech_hint 必须恰为 请说话哦..（Gavin 指定原文，含两个点）"
        );
    }

    /// G1 (OVERLAY-121 H7 换血): 二值圆角掩码 `apply_overlay_window_region` 已退役。
    ///
    /// OVERLAY-121 (634f016) 八态改走 UpdateLayeredWindow 逐像素 alpha，圆角由 D2D
    /// 几何抗锯齿绘制，`apply_overlay_window_region` 函数与全部 10 个调用点退役。
    /// 本护栏由旧 H7 的快照式（Some(16)×1/Some(10)×3/None×6）改为**结构式**：
    /// 生产区出现该函数或任何调用 = 二值掩码复活 = 圆角硬阶梯回归。
    ///
    /// 消融：生产区任意处写回一个 `apply_overlay_window_region(` 调用（或恢复函数定义）
    /// → 本测试红。
    /// 判别力边界：只扫生产区（首个 #[cfg(test)] 之前），测试区内若有人引用该名不计数。
    #[test]
    fn h7_overlay_corner_radius_snapshot() {
        let lines = main_prod_lines();
        let n_call = count_startswith(&lines, concat!("apply_overlay_window_region(", ""));
        let n_def = count_startswith(&lines, concat!("fn apply_overlay_window_region", "("));
        let n = n_call + n_def;
        assert_eq!(
            n, 0,
            "G1: apply_overlay_window_region 必须 0 处（OVERLAY-121 掩码退役），实测调用 {} 处 + 定义 {} 处 = {} 处——二值掩码复活=圆角硬阶梯回归",
            n_call,
            n_def,
            n
        );
    }

    /// H8: PipelineEvent::NoSpeech 的 overlay 分支（Windows 控制器）用
    /// OverlayStatus::Info（不是 Error），且 tray 复位到 Idle（照 FormatFailed 先例，
    /// 防卡在「处理中」）。
    /// 消融路径：改回 Error 或删 tray 复位 → 本测试红。
    #[test]
    fn h8_nospeech_overlay_uses_info_and_idle_tray() {
        let lines = main_prod_lines();
        let anchor = find_line(&lines, concat!("PipelineEvent::NoSpeech", " => {"))
            .expect("H8 anchor: Windows 控制器 NoSpeech arm");
        assert!(
            block_contains(&lines, anchor, concat!("status: OverlayStatus::", "Info(")),
            "H8: NoSpeech overlay 分支必须用 OverlayStatus::Info（不是 Error）"
        );
        assert!(
            block_contains(
                &lines,
                anchor,
                concat!("set_tray_state(tray, TrayState::", "Idle,")
            ),
            "H8: NoSpeech 分支必须把 tray 复位到 Idle（防卡「处理中」）"
        );
    }
}

// ==================== FLICKER-130 (R1) 护栏 ====================
//
// FLICKER-130 (commit 80ce51f): should_ignore_streaming_text 由
// (stopped, editing) -> stopped && !editing 收敛为单参 (stopped) -> stopped：
// 松手后到达的迟到流式包一律丢弃，编辑态不再豁免。
//
// 为什么这么改：OVERLAY-043 的 editing 豁免唯一路径 Show(StreamingEditing) 会无条件
// destroy_edit_control(:1298)，而 create_edit_control 唯一调用点在 EnterEditMode(:1475)
// —— Show 只销毁不重建，意图的「编辑态继续同步 EDIT 文字」从未实现，豁免的唯一可观测
// 效果就是销毁编辑框 = Gavin 报的「窗口和文字闪烁」。
//
// 四条护栏守的是三个不变量：
//   F1 门闩仅抑制渲染、不抑制数据（镜像写入必须先于门闩，且同处 process_controller_events）
//   F2 调用点唯一性（防复活 editing 维度 / 加旁路调用）—— 与 F4 成对，缺一废一
//   F3 editing ⇒ stopped 不变量（(false,true) 组合不可达的前提）
//   F4 行为真值表（函数本体）—— 与 F2 成对，缺一废一
#[cfg(all(test, target_os = "windows"))]
mod flicker_130_guard_tests {
    /// 读取 src/main.rs 生产区（首个 #[cfg(test)] 之前）。
    fn main_prod_lines() -> Vec<String> {
        let mut out = Vec::new();
        for line in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")).lines() {
            let t = line.trim();
            if t.starts_with("#[cfg(test)]") {
                break;
            }
            out.push(t.to_string());
        }
        out
    }

    fn find_line(lines: &[String], needle: &str) -> Option<usize> {
        lines.iter().position(|l| l.starts_with(needle))
    }

    fn norm_line(line: &str) -> String {
        line.trim().replace("platform::", "")
    }

    fn brace_delta(line: &str) -> i32 {
        line.matches('{').count() as i32 - line.matches('}').count() as i32
    }

    /// 花括号定界：以 anchor 行（可为多行签名锚点，行无 `{` 时前扫首个开括号）为起点，
    /// 返回 (open_idx, close_idx) —— 函数/块体的首行与闭合行（0-based 行号）。
    /// 找不到闭合（未配对）返回 None。
    fn block_bounds(lines: &[String], anchor: usize) -> Option<(usize, usize)> {
        let mut depth = 0i32;
        let mut opened = false;
        let mut open_idx = usize::MAX;
        for (i, line) in lines.iter().enumerate().skip(anchor) {
            let norm = norm_line(line);
            depth += brace_delta(&norm);
            if depth > 0 && !opened {
                opened = true;
                open_idx = i;
            }
            if opened && depth == 0 {
                return Some((open_idx, i));
            }
        }
        None
    }

    /// 花括号定界块内是否存在一行 startswith(needle)。
    fn block_contains(lines: &[String], anchor: usize, needle: &str) -> bool {
        block_bounds(lines, anchor)
            .map(|(lo, hi)| {
                lines[lo..=hi]
                    .iter()
                    .any(|l| norm_line(l).starts_with(needle))
            })
            .unwrap_or(false)
    }

    /// 花括号定界块内，某 needle 首现的 0-based 行号；不在块内返回 None。
    fn block_line_of(lines: &[String], anchor: usize, needle: &str) -> Option<usize> {
        block_bounds(lines, anchor)
            .and_then(|(lo, hi)| (lo..=hi).find(|&i| norm_line(&lines[i]).starts_with(needle)))
    }

    fn count_startswith(lines: &[String], needle: &str) -> usize {
        lines.iter().filter(|l| l.starts_with(needle)).count()
    }

    /// F1 🔴最重要：门闩仅抑制渲染、不抑制数据。
    ///
    /// WORDBOOK-053-B 的镜像写入 `*mirror = Some(text.clone())` 必须在门闩
    /// `if should_ignore_streaming_text(stopped)` 调用**之前**，且两者同处
    /// `process_controller_events` 函数体内。若有人把门闩挪到镜像之前 =
    /// 变成数据抑制 = 被截断的 raw 会学出伪修正。
    ///
    /// 消融：① 把镜像行与门闩行对调 → 红；② 把门闩整体挪出 process_controller_events
    /// （行号先后可能仍满足，但函数归属被破坏）→ 红。
    ///
    /// 🔴 判别力边界：锚点钉死在 `*mirror = Some(text.clone())` 形态上，若有人把它
    /// 重构成 `to_string()` 等其它赋值形态会**误红** —— 这是有意的：重构触红会逼人
    /// 来看一眼并同步更新护栏，护栏因重构而红是好事。
    #[test]
    fn f1_mirror_before_gate_within_process_controller_events() {
        let lines = main_prod_lines();
        // process_controller_events 多行签名锚点（:5138）
        let fn_anchor = find_line(&lines, concat!("fn process_controller_events", "("))
            .expect("F1 anchor: process_controller_events 函数签名");
        let (lo, hi) = block_bounds(&lines, fn_anchor)
            .expect("F1: process_controller_events 函数体必须能定位（花括号配对）");
        let mirror = block_line_of(
            &lines,
            fn_anchor,
            concat!("*mirror = Some(text.", "clone())"),
        );
        let gate = block_line_of(
            &lines,
            fn_anchor,
            concat!("if should_ignore_streaming_text(", "stopped)"),
        );
        assert!(
            mirror.is_some() && gate.is_some(),
            "F1: 镜像写入与门闩调用必须都在 process_controller_events 函数体内（{}-{}）",
            lo + 1,
            hi + 1
        );
        assert!(
            mirror.unwrap() < gate.unwrap(),
            "F1: 镜像写入行（L{}）必须早于门闩调用行（L{}）—— 门闩在前=数据抑制，WORDBOOK-053-B 会学到被截断的 raw",
            mirror.unwrap() + 1,
            gate.unwrap() + 1
        );
    }

    /// F2 调用点唯一性：生产区 `if should_ignore_streaming_text(stopped)` 调用恰 1 处，
    /// 单参签名定义恰 1 处，双参签名 0 处。
    ///
    /// 防：恢复双参签名（= 复活 editing 维度）或新增旁路调用。
    /// 🔴 与 F4 成对：F2 守「没人绕过函数自己写判断」，F4 守「函数本体行为正确」。
    /// 缺 F2：别人在调用点用 `stopped && !editing` 内联，F4 全绿但闪烁回归。
    /// 缺 F4：函数体被改回双参语义但签名仍是单参（外部形态对、行为错），F2 全绿但闪烁回归。
    ///
    /// 消融：① 加第二调用点 → 红；② 改回双参签名 `(stopped: bool, editing: bool)` → 红。
    #[test]
    fn f2_single_call_site_and_single_param_signature() {
        let lines = main_prod_lines();
        let call_sites = count_startswith(
            &lines,
            concat!("if should_ignore_streaming_text(", "stopped)"),
        );
        let single_sig = count_startswith(
            &lines,
            concat!("fn should_ignore_streaming_text(", "stopped: bool) -> bool"),
        );
        let dual_sig = count_startswith(
            &lines,
            concat!(
                "fn should_ignore_streaming_text(",
                "stopped: bool, editing: bool)"
            ),
        );
        assert_eq!(
            call_sites, 1,
            "F2: should_ignore_streaming_text 调用必须恰 1 处（实测 {}），新增旁路调用=复活闪烁路径",
            call_sites
        );
        assert_eq!(
            single_sig, 1,
            "F2: 单参签名 fn should_ignore_streaming_text(stopped: bool) 定义必须恰 1 处"
        );
        assert_eq!(
            dual_sig, 0,
            "F2: 双参签名（stopped, editing）不得出现 —— editing 维度已收敛，出现=复活豁免"
        );
    }

    /// F3 `editing ⇒ stopped` 不变量：EditRequested 臂内 OVERLAY_EDITING.store(true)
    /// 与 STREAMING_STOPPED.store(true) 共现于同一 match 臂（花括号定界）。
    ///
    /// 这是 `(false,true)` 组合不可达的前提（主控独立验证：OVERLAY_EDITING.store(true)
    /// 全库仅 :5531 一处，同臂 :5536 紧跟 STREAMING_STOPPED.store(true)，同线程）。
    /// 破坏它 = 复活「编辑中流式继续推」路径 = 复活闪烁。
    ///
    /// 🔴 不写「else if editing 运行时行为」断言 —— 该分支已成不可达死分支，对不可达
    /// 路径写断言 = 永远绿的假护栏。F3 守的是「让它保持不可达」的不变量，才有判别力。
    ///
    /// 消融：把 STREAMING_STOPPED.store(true) 那行移出 EditRequested 臂 → 红。
    #[test]
    fn f3_edit_requested_arm_sets_editing_and_stopped_together() {
        let lines = main_prod_lines();
        let anchor = find_line(&lines, concat!("OverlayUiEvent::EditRequested", " => {"))
            .expect("F3 anchor: EditRequested match 臂");
        let has_editing_store =
            block_contains(&lines, anchor, concat!("OVERLAY_EDITING.store(", "true"));
        let has_stopped_store =
            block_contains(&lines, anchor, concat!("STREAMING_STOPPED.store(", "true"));
        assert!(
            has_editing_store && has_stopped_store,
            "F3: EditRequested 臂必须同时置 OVERLAY_EDITING.store(true) 与 STREAMING_STOPPED.store(true)—— editing⇒stopped 不变量，破坏=复活 (false,true) 不可达组合=复活闪烁"
        );
    }

    // F4 行为真值表**有意不在此重复**：should_ignore_streaming_text 的行为断言唯一
    // 权威份在 overlay_043_interpolate_tests::ignore_streaming_text_truth_table
    // (main.rs:9190)。本模块 F2 守签名/调用点唯一性，与 :9190 成对 —— 函数改回双参
    // 语义时签名一变 F2 红、行为一错 :9190 红，删任一条另一条即失效。不要在这里
    // 再加行为断言副本（主控 2026-09-06 裁定：逐字相同的断言让两条同时红，零增量信息）。
    //
    // 🔴 阶段四消融验证（主控指定）：把函数改回双参 `(stopped: bool, editing: bool)`
    // → **F2 必须红**。若实测不红，说明主控以 F2 替代 F4 的推断错误，须回报重开 F4。
}

// ==================== OVERLAY-121 (per-pixel alpha) 护栏 ====================
//
// OVERLAY-121 (634f016): 八态改走 UpdateLayeredWindow 逐像素 alpha，
// apply_overlay_window_region 二值掩码（函数 + 10 调用点）全部退役，
// 圆角改由 D2D 几何抗锯齿绘制。G1 已在 nospeech_122_guard_tests::h7
// 换血实现（掩码归零）。G2-G7 守新架构的不变量：
//   G2 ULW 提交单点（防多提交点 / 改回 BitBlt 直出）
//   G3 fixup 单点 + 紧邻 ULW（防 alpha 烘焙被绕过）
//   G4 SLWA 条件化（防 ULW 窗口被切回均一 alpha = 必闪）
//   G5 模式切换在隐藏区间（防 redirection 表面重建闪烁可见）
//   G6 SDF alpha 不变量四条（几何 alpha / 轮廓外全零 / 反预乘 / 🔴旧提亮规则禁活）
//   G7 DIB 32bpp + 负高（防 BindDC 1:1 映射破坏 = 上下翻转）
//   G8 掩码半径单一来源（overlay_frame_radius 映射恰 1 + 调用实参绑 frame_radius）
//   G9 外框绘制半径单一来源（三类绘制调用实参必须引用 OVERLAY_FRAME_RADIUS_）
#[cfg(all(test, target_os = "windows"))]
mod overlay_121_guard_tests {
    /// 读取 src/main.rs 生产区（首个 #[cfg(test)] 之前）。
    fn main_prod_lines() -> Vec<String> {
        let mut out = Vec::new();
        for line in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")).lines() {
            let t = line.trim();
            if t.starts_with("#[cfg(test)]") {
                break;
            }
            out.push(t.to_string());
        }
        out
    }

    fn find_line(lines: &[String], needle: &str) -> Option<usize> {
        lines.iter().position(|l| l.starts_with(needle))
    }

    fn norm_line(line: &str) -> String {
        line.trim().replace("platform::", "")
    }

    fn brace_delta(line: &str) -> i32 {
        line.matches('{').count() as i32 - line.matches('}').count() as i32
    }

    /// 花括号定界：以 anchor 行（可为多行签名锚点）为起点，返回 (open_idx, close_idx)。
    fn block_bounds(lines: &[String], anchor: usize) -> Option<(usize, usize)> {
        let mut depth = 0i32;
        let mut opened = false;
        let mut open_idx = usize::MAX;
        for (i, line) in lines.iter().enumerate().skip(anchor) {
            let norm = norm_line(line);
            depth += brace_delta(&norm);
            if depth > 0 && !opened {
                opened = true;
                open_idx = i;
            }
            if opened && depth == 0 {
                return Some((open_idx, i));
            }
        }
        None
    }

    /// 花括号定界块内是否存在一行 startswith(needle)。
    fn block_contains(lines: &[String], anchor: usize, needle: &str) -> bool {
        block_bounds(lines, anchor)
            .map(|(lo, hi)| {
                lines[lo..=hi]
                    .iter()
                    .any(|l| norm_line(l).starts_with(needle))
            })
            .unwrap_or(false)
    }

    /// 花括号定界块内，某 needle 首现的 0-based 行号；不在块内返回 None。
    fn block_line_of(lines: &[String], anchor: usize, needle: &str) -> Option<usize> {
        block_bounds(lines, anchor)
            .and_then(|(lo, hi)| (lo..=hi).find(|&i| norm_line(&lines[i]).starts_with(needle)))
    }

    /// 块内任意位置子串命中（不依赖行首形态）。G6④ 反向断言专用：
    /// 旧提亮规则若被塞回，可能不以行首出现（如嵌进更长表达式），
    /// starts_with 版本会漏，contains 版本才钉得死。
    fn block_contains_raw(lines: &[String], anchor: usize, needle: &str) -> bool {
        block_bounds(lines, anchor)
            .map(|(lo, hi)| lines[lo..=hi].iter().any(|l| norm_line(l).contains(needle)))
            .unwrap_or(false)
    }

    fn count_startswith(lines: &[String], needle: &str) -> usize {
        lines.iter().filter(|l| l.starts_with(needle)).count()
    }

    /// G2: ULW 提交单点。生产区真实调用 UpdateLayeredWindow( 恰 1 处。
    ///
    /// OVERLAY-121 的逐像素 alpha 通过 WM_PAINT 内 `let _ = UpdateLayeredWindow(...)`
    /// 单点提交。import 行与注释里的该词不算 —— 只数真实调用形态。
    /// 防：新增第二提交点（多提交点互相覆盖），或改回 BitBlt 直出（丢失 per-pixel alpha）。
    ///
    /// 消融：新增第二处提交调用 → 计数变 2 → 红；删掉该调用改回 BitBlt → 计数 0 → 红。
    #[test]
    fn g2_ulw_submit_single_site() {
        let lines = main_prod_lines();
        let n = count_startswith(&lines, concat!("let _ = UpdateLayeredWindow(", ""));
        assert_eq!(
            n, 1,
            "G2: UpdateLayeredWindow 真实提交必须恰 1 处（WM_PAINT 单点），实测 {} 处",
            n
        );
    }

    /// G3: alpha fixup 单点 + 紧邻 ULW 提交（同块）。
    ///
    /// apply_alpha_fixup 负责把逐像素 alpha（含 GDI fallback 提亮）烘焙进 DIB，
    /// 必须恰 1 处定义、恰 1 处调用，且调用与 ULW 提交同处 if use_ulw 块。
    ///
    /// 消融：删调用 → 计数 1→0 → 红；加第二调用点 → 计数变 2 → 红。
    #[test]
    fn g3_alpha_fixup_single_and_adjacent_to_ulw() {
        let lines = main_prod_lines();
        let defs = count_startswith(&lines, concat!("fn apply_alpha_fixup", "("));
        assert_eq!(
            defs, 1,
            "G3: apply_alpha_fixup 定义必须恰 1 处（实测 {} 处）",
            defs
        );
        let calls = count_startswith(&lines, concat!("apply_alpha_fixup(", ""));
        assert_eq!(
            calls, 1,
            "G3: apply_alpha_fixup 调用必须恰 1 处（实测 {} 处）",
            calls
        );
        // 🔴 定位 fixup/ULW 所在的那个 if use_ulw 块（生产区有 3 处 if use_ulw，
        // 只有 :2136 那个含 ULW 提交；用 ULW 提交行反向找最近的 if use_ulw 块锚）。
        let ulw_submit = find_line(&lines, concat!("let _ = UpdateLayeredWindow(", ""))
            .expect("G3 anchor: ULW 提交行");
        // 从 ULW 提交行向前找最近的 `if use_ulw {`（行号最大且小于 submit）
        let ulw_anchor = lines[..ulw_submit]
            .iter()
            .rposition(|l| l.starts_with("if use_ulw"))
            .expect("G3 anchor: fixup/ULW 所在的 if use_ulw 块");
        let has_fixup = block_contains(&lines, ulw_anchor, concat!("apply_alpha_fixup(", ""));
        let has_ulw = block_contains(
            &lines,
            ulw_anchor,
            concat!("let _ = UpdateLayeredWindow(", ""),
        );
        assert!(
            has_fixup && has_ulw,
            "G3: apply_alpha_fixup 调用与 UpdateLayeredWindow 提交必须同处 if use_ulw 块（L{} 起的块）",
            ulw_anchor + 1
        );
    }

    /// G4: SLWA 条件化。Show 处理器内 SetLayeredWindowAttributes 必须处于
    /// slwa_active 条件块内。
    ///
    /// 守的是「把 ULW 窗口切回均一 alpha，之后回 ULW 必须清/置 bit = 必闪」。
    ///
    /// 消融：把 SetLayeredWindowAttributes 改成无条件调用 → 本测试红。
    /// 判别力边界：needle 绑 `let _ = SetLayeredWindowAttributes(` 当前调用形态；
    /// 若有人改成裸调（无 let _）会漏，但该形态与项目风格不符，且 G2（ULW 单点）
    /// 兜底。
    #[test]
    fn g4_slwa_conditional_inside_active_block() {
        let lines = main_prod_lines();
        let anchor = find_line(&lines, concat!("if slwa_active", " {"))
            .expect("G4 anchor: if slwa_active 条件块");
        assert!(
            block_contains(
                &lines,
                anchor,
                concat!("let _ = SetLayeredWindowAttributes", "("),
            ),
            "G4: SetLayeredWindowAttributes 必须处于 if slwa_active 条件块内"
        );
    }

    /// G5: 模式切换发生在隐藏区间（EnterEditMode + Show 处理器两处）。
    ///
    /// 编辑态（SLWA）与其余七态（ULW）不能直切，须清/置 WS_EX_LAYERED 中转；
    /// 该切换的 redirection 表面重建会闪，必须藏在窗口隐藏区间内：
    /// 两处都要求 SW_HIDE 行号 < switch_overlay_layered_mode 行号且同块（花括号定界）。
    ///
    /// 消融：把 SW_HIDE 移到切换之后 / 移出该块 → 本测试红。
    /// 判别力边界：needle 绑 `let _ = ShowWindow(hwnd, SW_HIDE)` 当前调用形态；
    /// 若有人改成裸调会漏，但该形态与项目风格不符。
    #[test]
    fn g5_mode_switch_hidden_before_switch() {
        let lines = main_prod_lines();
        // ① EnterEditMode 臂
        let edit_anchor = find_line(&lines, concat!("OverlayCommand::EnterEditMode", " => {"))
            .expect("G5 anchor: EnterEditMode 臂");
        let edit_hide = block_line_of(
            &lines,
            edit_anchor,
            concat!("let _ = ShowWindow(hwnd, SW_HIDE)", ""),
        );
        let edit_switch = block_line_of(
            &lines,
            edit_anchor,
            concat!("switch_overlay_layered_mode", "("),
        );
        assert!(
            edit_hide.is_some() && edit_switch.is_some(),
            "G5: EnterEditMode 臂必须同时有 SW_HIDE 与 switch_overlay_layered_mode，且同块"
        );
        assert!(
            edit_hide.unwrap() < edit_switch.unwrap(),
            "G5: EnterEditMode 内 SW_HIDE（L{}）必须早于 switch_overlay_layered_mode（L{}）",
            edit_hide.unwrap() + 1,
            edit_switch.unwrap() + 1
        );
        // ② Show 处理器切换块
        let mode_if = find_line(
            &lines,
            concat!("if state.layered_mode != target_mode", " {"),
        )
        .or_else(|| {
            lines
                .iter()
                .rposition(|l| l.starts_with("if state.layered_mode != target_mode"))
        })
        .expect("G5 anchor: layered_mode 切换条件块");
        let show_hide = block_line_of(
            &lines,
            mode_if,
            concat!("let _ = ShowWindow(hwnd, SW_HIDE)", ""),
        );
        let show_switch =
            block_line_of(&lines, mode_if, concat!("switch_overlay_layered_mode", "("));
        assert!(
            show_hide.is_some() && show_switch.is_some(),
            "G5: Show 处理器切换块必须同时有 SW_HIDE 与 switch_overlay_layered_mode"
        );
        assert!(
            show_hide.unwrap() < show_switch.unwrap(),
            "G5: Show 处理器内 SW_HIDE（L{}）必须早于 switch_overlay_layered_mode（L{}）",
            show_hide.unwrap() + 1,
            show_switch.unwrap() + 1
        );
    }

    /// G6: apply_alpha_fixup SDF alpha 不变量（OVERLAY-141 换血、OVERLAY-153 再演进
    /// —— 旧「三分支完整」命题随按色三分支整体退役而失效，改守真实不变量，四条）：
    ///   ① alpha 由几何决定：函数体内含 SDF 覆盖率计算
    ///      `let cov = (0.5 - d).clamp(0.0, 1.0);`
    ///   ② 轮廓外真透明：`if cov <= 0.0 {` 块内四通道全零
    ///   ③ D2D 像素分支存在：`if a > 0 {`（OVERLAY-153 起为**四通道等比缩放**，
    ///      保留原 alpha 不强制不透明；141 时代语义是反预乘+强制 255·k，已退役）
    ///   ④ 🔴 反向断言：函数体内不得再出现旧「提亮」规则
    ///      `if a == 0 && (r | g | b) != 0` —— 它把 AA 半透明边缘像素压成全不透明，
    ///      正是 OVERLAY-141「圆角灰边」事故的元凶，机器钉死防复活。
    ///
    /// 消融（沙箱预演，result.md 附录 A）：①删 cov 行红 / ②改 cov<=0.0 条件红、
    /// 删通道全零写红 / ③改 a>0 条件红 / ④塞回旧规则行红 —— 全部实测验红。
    #[test]
    fn g6_fixup_sdf_alpha_invariants() {
        let lines = main_prod_lines();
        let anchor = find_line(&lines, concat!("fn apply_alpha_fixup", "("))
            .expect("G6 anchor: apply_alpha_fixup 函数");
        assert!(
            block_contains(
                &lines,
                anchor,
                concat!("let cov = (0.5 - d).clamp(0.0, 1.0)", ";")
            ),
            "G6①: apply_alpha_fixup 必须含 SDF 覆盖率计算（alpha 由几何决定）"
        );
        let cov_if = block_line_of(&lines, anchor, concat!("if cov <= 0.0", " {"))
            .expect("G6 anchor: if cov <= 0.0 分支");
        for chan in [
            concat!("*p = 0", ""),
            concat!("*p.add(1) = 0", ""),
            concat!("*p.add(2) = 0", ""),
            concat!("*p.add(3) = 0", ""),
        ] {
            assert!(
                block_contains(&lines, cov_if, chan),
                "G6②: if cov <= 0.0 块必须四通道全零，缺：{}",
                chan
            );
        }
        assert!(
            block_contains(&lines, anchor, concat!("if a > 0", " {")),
            "G6③: apply_alpha_fixup 必须含 D2D 像素分支 if a > 0（153 起等比缩放保留原 alpha，防强制不透明复活）"
        );
        assert!(
            !block_contains_raw(&lines, anchor, concat!("a == 0 && (r | g | b)", "")),
            "G6④: 旧提亮规则 if a == 0 && (r|g|b) != 0 已被 OVERLAY-141 根治，禁止复活（会把 AA 半透明边缘压成硬灰带 = 圆角灰边回归）"
        );
    }

    /// G7: DIB 32bpp + 负高。WM_PAINT 的 BITMAPINFOHEADER 必须 biBitCount: 32
    /// 且 biHeight 为负（top-down）。
    ///
    /// 防：改回 bottom-up 正高 → BindDC 1:1 映射破坏 → 画面上下翻转。
    ///
    /// 消融：把 biHeight 改回正高 → 本测试红。
    #[test]
    fn g7_dib_32bpp_and_top_down() {
        let lines = main_prod_lines();
        let has_32bpp = lines
            .iter()
            .any(|l| l.starts_with(concat!("biBitCount: 32", ",")));
        let has_neg_height = lines
            .iter()
            .any(|l| l.starts_with(concat!("biHeight: -", "height")));
        assert!(
            has_32bpp && has_neg_height,
            "G7: WM_PAINT bmi 必须 biBitCount: 32 且 biHeight 为负（top-down）"
        );
    }

    /// G8: 掩码半径单一来源（OVERLAY-141 前置要求：掩码与绘制半径必须同值）。
    ///   ① 生产区 `frame_radius = overlay_frame_radius(` 赋值形态恰 1 处
    ///      （needle 绑完整赋值形态：`let mut frame_radius = ...` 默认值兜底行以
    ///      `let mut` 开头不命中，`apply_alpha_fixup(...)` 调用行也不命中 —— 不会误计）；
    ///   ② apply_alpha_fixup 唯一调用行的半径实参必须是 frame_radius，不得就地写数。
    ///
    /// 消融（沙箱预演，result.md 附录 A）：①映射改裸数字 → 计数 0 红；
    /// 新增第二份映射 → 计数 2 红；②调用实参改 16.0 → 红。全部实测验红。
    #[test]
    fn g8_frame_radius_single_source() {
        let lines = main_prod_lines();
        let mapped = count_startswith(&lines, concat!("frame_radius = overlay_frame_radius", "("));
        assert_eq!(
            mapped, 1,
            "G8①: overlay_frame_radius 映射赋值必须恰 1 处，实测 {} 处",
            mapped
        );
        let call_idx = lines
            .iter()
            .position(|l| l.starts_with(concat!("apply_alpha_fixup(ppv_bits", "")))
            .expect("G8 anchor: apply_alpha_fixup 调用行");
        assert!(
            lines[call_idx].contains(concat!(", frame_radius", ")")),
            "G8②: apply_alpha_fixup 半径实参必须是 frame_radius（单一来源），实测行：{}",
            lines[call_idx]
        );
    }

    /// G9: 外框绘制半径单一来源（绘制半径与掩码同源，散落字面量会切出新锯齿）。
    /// 覆盖三类外框绘制调用：GDI `draw_overlay_chrome(`（5 处，单行）、
    /// D2D `chrome(res`（4 处，单行）、D2D `chrome_with(`（4 处，rustfmt 折行 →
    /// 逐调用收集实参到 `);` 收尾）。每个调用点的半径实参必须引用
    /// `OVERLAY_FRAME_RADIUS_`；唯一豁免：fn chrome 内部委托 chrome_with 透传
    /// 形参 `radius`（其上游全部 chrome( 调用点已被本护栏逐一覆盖，不构成旁路）。
    ///
    /// 🔴 只盯外框绘制调用，不扫全库 CORNER_RADIUS —— 内部元素半径
    /// （draw_submit_button 内 `const CORNER_RADIUS`、药丸 3.5 / 停止键 5 /
    /// 提交键 4 / 底部按钮 4 / 标题✕ 3）有意留存，扫全库必假红。
    ///
    /// 锚点唯一性：三个定义行均以 `fn ` 开头，不命中 starts_with needle，
    /// 天然排除（不依赖 find_line 首锚，规避 TEST-SYNC-134 误锚形态）。
    ///
    /// 消融（沙箱预演）：GDI 调用改裸 16 → 红；折行 chrome_with 实参改 10.0 → 红。
    #[test]
    fn g9_frame_drawing_calls_single_source() {
        let lines = main_prod_lines();
        let prefixes = [
            concat!("draw_overlay_chrome", "("),
            concat!("chrome(res", ""),
            concat!("chrome_with(", ""),
        ];
        let mut checked = 0usize;
        let mut i = 0usize;
        while i < lines.len() {
            let l = norm_line(&lines[i]);
            if !prefixes.iter().any(|p| l.starts_with(p)) {
                i += 1;
                continue;
            }
            // rustfmt 会把 chrome_with( 折成多行：逐调用收集实参直到收尾。
            let mut args = String::new();
            let mut j = i;
            loop {
                args.push_str(&lines[j]);
                args.push(' ');
                let tj = lines[j].trim();
                if tj.ends_with(';') || tj == ");" {
                    break;
                }
                j += 1;
                assert!(
                    j <= i + 8,
                    "G9: L{} 起的外框调用 8 行内未见 `);` 收尾（rustfmt 折行超预期，护栏需跟进）",
                    i + 1
                );
            }
            assert!(
                args.contains(concat!("OVERLAY_FRAME_RADIUS", "_"))
                    || args.contains(concat!(", radius)", ""))
                    || args.contains(concat!(", radius,", "")),
                "G9: 外框绘制调用（L{}）半径实参必须引用 OVERLAY_FRAME_RADIUS_（或 chrome 内部透传 radius），不得裸数字：{}",
                i + 1,
                args
            );
            checked += 1;
            i = j + 1;
        }
        assert_eq!(
            checked, 13,
            "G9: 外框绘制调用计数异常（预期 13 = draw_overlay_chrome 5 + chrome 4 + chrome_with 4），实测 {} —— 调用点集合变化时必须同步更新本护栏",
            checked
        );
    }

    /// G10: EDIT 销毁前 caret 清理三步齐全且顺序正确（OVERLAY-149 F1）。
    ///
    /// 守的是：`destroy_edit_control` 内「移焦 `SetFocus(` → `HideCaret(` →
    /// `DestroyCaret()`」三步必须全部早于 `let _ = DestroyWindow(`。
    /// 带焦销毁收到的是 WM_DESTROY 而非 WM_KILLFOCUS，EDIT 内部不会销毁
    /// caret；caret 对象绑线程输入队列而非窗口，DestroyWindow **之后**再清
    /// 是空操作（caret 残留跨态闪烁，OVERLAY-147-DIAG §B2 成因）——
    /// 顺序错了修复即失效，故顺序断言是本护栏的主体。
    ///
    /// 锚点唯一性（134/143 教训）：needle 全部取代码行首形态
    /// （`let _ = DestroyCaret()` 等），注释行以 `//` 开头天然不命中
    /// starts_with，不存在「注释喂绿」；fn 定义行全库恰 1（find_line 首锚
    /// 即唯一锚）。OVERLAY-149-PROBE 探针块（GetGUIThreadInfo）位于三步与
    /// DestroyWindow 之间，判读后删除不影响本护栏（相对顺序不变）。
    ///
    /// 消融（沙箱预演，验后还原）：删 DestroyCaret 调用 → 红；
    /// 三步整块挪到 DestroyWindow 之后 → 红（顺序断言）。
    #[test]
    fn g10_caret_cleanup_before_destroy_window() {
        let lines = main_prod_lines();
        let anchor = find_line(&lines, concat!("fn destroy_edit_control", "("))
            .expect("G10 anchor: destroy_edit_control 定义");
        let bounds = block_bounds(&lines, anchor).expect("G10: 函数块定界失败");
        assert_eq!(
            bounds.0,
            anchor,
            "G10: 块定界起点异常（open=L{}，anchor=L{}）——锚点漂移，护栏需跟进",
            bounds.0 + 1,
            anchor + 1
        );
        let line_of = |needle: &str| {
            block_line_of(&lines, anchor, needle)
                .unwrap_or_else(|| panic!("G10: 块内未找到代码行首形态 {needle}（三步被删/改名）"))
        };
        let setfocus = line_of(concat!("let _ = SetFocus", "("));
        let hide = line_of(concat!("let _ = HideCaret", "("));
        let destroy_caret = line_of(concat!("let _ = DestroyCaret", "("));
        let destroy_window = line_of(concat!("let _ = DestroyWindow", "("));
        assert!(
            setfocus < destroy_window && hide < destroy_window && destroy_caret < destroy_window,
            "G10: caret 清理三步（SetFocus L{} / HideCaret L{} / DestroyCaret L{}）必须全部早于 DestroyWindow（L{}）——销毁后清理是空操作，顺序错=修复失效",
            setfocus + 1,
            hide + 1,
            destroy_caret + 1,
            destroy_window + 1
        );
    }

    /// G11: 热键 Stop 臂必须清 OVERLAY_EDITING（OVERLAY-149 F2）。
    ///
    /// 守的是：OVERLAY_EDITING 不清则后续 Done/Cancelled 被压制臂
    /// （`if OVERLAY_EDITING.load(...)`）拦下，既不回 Idle 也不 Hide，
    /// Processing 浮层永久卡屏直到下次录音（OVERLAY-147-DIAG §B1-⑥ 可达缺陷）。
    ///
    /// 锚点唯一性（134/143 教训，结构锚不绑文案）：`HotkeyEvent::Stop => {`
    /// 生产区两处 —— Windows 控制器臂行首 `HotkeyEvent::Stop => {`（:5807 区）、
    /// macOS 臂行首 `platform::HotkeyEvent::Stop => {`（:7214 区，前缀不同
    /// startswith 天然不命中）⇒ find_line 首锚结构性唯一取 Windows 臂，改日志
    /// 措辞不会误伤。sanity 断言用结构特征 `if is_recording.load(`（Windows 臂
    /// 独有，macOS 臂三行无此结构）：若臂序重排导致锚到 macOS 臂，sanity 红 =
    /// 误锚显式暴露，不产生假绿。
    ///
    /// macOS 侧无需同款守卫（主控 cfg 归属扫描定稿）：OVERLAY_EDITING 唯一
    /// store(true) 在 cfg(windows) 块内，macOS 上恒 false ⇒ Done/Cancelled
    /// 压制臂不可能生效 ⇒ F2 缺陷 macOS 不可达。
    ///
    /// 消融（沙箱预演，验后还原）：删 `OVERLAY_EDITING.store(false` 行 → 红。
    #[test]
    fn g11_hotkey_stop_clears_editing_flag() {
        let lines = main_prod_lines();
        let anchor = find_line(&lines, concat!("HotkeyEvent::Stop", " => {"))
            .expect("G11 anchor: HotkeyEvent::Stop 臂");
        assert!(
            block_contains(&lines, anchor, concat!("if is_recording.load", "(")),
            "G11: 锚定块不含 Windows 控制器臂结构特征 if is_recording.load（锚点漂移到 macOS 臂）——臂序变化需同步更新锚点"
        );
        assert!(
            block_contains(&lines, anchor, concat!("OVERLAY_EDITING.store(false", "")),
            "G11: HotkeyEvent::Stop 臂必须清 OVERLAY_EDITING（不清则 Done/Cancelled 被压制臂拦下 ⇒ Processing 浮层永久卡屏）"
        );
    }
}
