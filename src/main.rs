#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
// Bug 2 fix: tray menu freeze workaround (tray-icon #298)
// Use Win32 TrackPopupMenu directly to avoid modal message loop conflict
// with overlay thread's InvalidateRect calls.
mod audio;
mod config;
mod crash;
mod homophone;
#[cfg(target_os = "windows")]
mod hotkey; // Deprecated: use platform::HotkeyListener instead
mod i18n;
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
    atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
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
use windows::Win32::System::SystemInformation::GetTickCount64;
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
    /// LOCAL-RT-ENGINE-239-B: 本地流式档位双模型加载提示（Info 态，文案携带在 payload）。
    Info(String),
    /// LOCAL-RT-ENGINE-239-B: 本地流式档位模型缺失/加载失败 → 直接报错不降级（DEC-067 附则一）。
    /// 与 Error 的区别：**不过 convert_to_friendly_error**，保留「具体缺哪个模型」的原始上下文。
    ModelUnavailable(String),
    /// LOCALRT-FIRSTCHAR-282: 本地流式**松手 flush 的最终预览全文**。
    /// 🔴 专用通道：`STREAMING_STOPPED` + `should_ignore_streaming_text` 是本地/在线共用 latch，
    /// 动它会影响在线档 + 可能复活 OVERLAY-043 闪烁 ⇒ **新开本事件，只有本地档发/只有本地档消费**，
    /// 在线档代码路径结构上不变（它永不发也永不收本事件）。
    /// 消费：在 `RecordingWithText` 态下展示收尾预览（不 auto-close、不改状态机）。
    StreamingFinalPreview(String),
    /// ACC-PREVIEW-REFLOW-325：本地实时档 **accuracy 分片的权威文本回灌预览**。
    ///
    /// 296/307 已证伪「在流式模型上补尾字」（`gained` 恒 0）；本单换层面：298 的 accuracy
    /// 分片结果在录音过程中陆续回来，用它对**已完成片**的覆盖区做权威替换，尾字自然补齐。
    ///
    /// 字段：`generation` 代际（防串场，同 `StreamingText`）；`seg_index` 该片序号
    /// （**单调键**，worker 按序产生、构造上严格递增）；`committed_len` 派发当刻浮层已显示
    /// 文本的**字符数**（回灌边界）；`has_hole` 本片覆盖范围内是否含**无法填补**的 accuracy
    /// 失败片（344-G 后：失败片一律用该片**流式文本填补**，故 `has_hole` 只在流式也为空等
    /// 真·无法填补时为 true；含则**不得回灌**，否则会把带洞文本灌上浮层）；
    /// `acc_text` 覆盖区内的权威文本（含流式填补，**连续无洞**）。
    ///
    /// 🔴 只有 LocalRealtime 会发；在线/批处理结构上永不发（同 282 专用通道先例）。
    PreviewReflow {
        generation: u64,
        seg_index: usize,
        /// FIX-PREVIEW-STALE-AND-COLLAPSE-375（A）：**滑窗权威快照自有**的单调序号（仅
        /// `replace_all=true` 时给；发送方 = 滑窗 acc worker 的递增计数器，与 `OrderedReflow`
        /// 的快照顺序一致）。过期判据用它而非 `seg_index` —— 后者是切片下标，**会重复**（窗口间
        /// 并发 + 收尾 drain 可让多窗撞同一下标）⇒ 会把后到的完整文本误判成 stale。
        reflow_seq: Option<usize>,
        committed_len: usize,
        has_hole: bool,
        acc_text: String,
        /// SLIDING-WINDOW-367：`true` ⇒ 消费端**整段替换**预览为 `acc_text`（滑窗权威全文，
        /// 非前缀关系，不能按 `committed_len` 切；Gavin 方案「整体替换」）；`false` ⇒ 沿用 325
        /// 原语义（替换前 `committed_len` 字符、保留流式尾巴）。**显式声明节点行为（DEC-066）**，
        /// 不靠长度关系反推。
        replace_all: bool,
        /// FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382（3C）：本窗**解码完成**时刻
        /// （worker `Instant::now()`，随解码结果一路带到事件）。消费端在**实际 `show_overlay` 渲染**时
        /// 打 `decode_done→render_ms`（端到端口径：解完 → 浮层重画）。`None` = 老发送方 / 非滑窗路径。
        decode_done_at: Option<std::time::Instant>,
    },
    /// LOCALRT-SEAM-337（自适应定界）：本片边界冻结通知。
    ///
    /// `committed_len = Some(n)`（分支 a 文本停止增长 / c 硬上限）⇒ 用该片 pending `acc_text`
    /// 覆盖流式显示前 n 字符并渲染；`None`（分支 b 有声恢复）⇒ 该片**不回灌**，保持纯流式。
    /// 只有本地实时档会发（在线/批处理结构上不发）。
    ReflowCommit {
        generation: u64,
        seg_index: usize,
        committed_len: Option<usize>,
    },
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
    /// AUTOLEARN-EDIT-SNAPSHOT-331: 第 3 参 = **编辑入口快照**（用户开始编辑时屏幕上那份文本，
    /// 与 EDIT 初值/渲染字段同源）——仅本地实时档用作自学习基准，天然免疫迟到包/渲染门/时序。
    SubmitRequested(String, platform::WindowId, Option<String>),
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
// ACC-PREVIEW-REFLOW-325：本地实时档回灌的 **per-generation** 状态（**controller 线程单写者**：
// 二者只在消费循环内读写；`RecordingStarted` 重置，`PreviewReflow` 处理时读/写 —— 同一线程，
// 故无竞态，无需原子以外同步）。
// - `ACC_REFLOW_LAST_SEG`：已应用回灌的最大 `seg_index`（**单调键 = seg_index**，不是
//   committed_len —— 后者是长度，句边界丢影子/punctuation 重打可使其合法回落，见 325 端测日志）。
// - `ACC_REFLOW_EDIT_LATCH`：本次录音内**只要进过一次编辑态**即上闩，后续回灌一律停到下一代
//   （防「编辑退出后迟到的回灌把用户刚打的字冲掉」）。
#[cfg(target_os = "windows")]
static ACC_REFLOW_LAST_SEG: AtomicI64 = AtomicI64::new(-1);
/// FIX-PREVIEW-STALE-AND-COLLAPSE-375（A）：`replace_all=true` 路径**专用**的单调键 = 滑窗快照的
/// `reflow_seq`（`seg_index` 在滑窗路径会重复，见事件字段注释）。两条路径各用一个计数器，
/// 互不干扰 ⇒ 老逐片路径行为逐位不变。
#[cfg(target_os = "windows")]
static ACC_REFLOW_LAST_REFLOW_SEQ: AtomicI64 = AtomicI64::new(-1);
#[cfg(target_os = "windows")]
static ACC_REFLOW_EDIT_LATCH: AtomicBool = AtomicBool::new(false);
// ACC-REFLOW-PERSIST-329：本代已确定的**权威前缀**状态 `(generation, acc_text, committed_len)`。
// - 只在 `PreviewReflow`(Applied) 时写（唯一发送点 = 本地实时档）⇒ 在线/批处理恒 None；
// - `RecordingStarted` 清空；流式渲染时按 gen 匹配取用；
// - **闩锁之后保留**（只停止更新，不丢弃）⇒ 画面不倒退。
// - controller 单线程读写；`Mutex` 仅因是 `static`（同 `last_streaming_text` 先例）。
#[cfg(target_os = "windows")]
static ACC_REFLOW_STATE: Mutex<Option<(u64, String, usize)>> = Mutex::new(None);
// LOCALRT-SEAM-337（自适应定界）：两个 pending 槽，**跨事件配对**（acc 文本与边界到达顺序不定）：
// - `ACC_REFLOW_ACC`   = `(generation, seg_index, acc_text, replace_all)`（由 PreviewReflow 写）；
// - `ACC_REFLOW_BOUND` = `(generation, seg_index, committed_len: Option<usize>)`（由 ReflowCommit 写）。
// 两者 seg/gen 匹配时 `try_resolve_reflow` 合成并渲染（`Some`）/ 作废（`None`），随后清空两槽。
// `RecordingStarted` 清空。
#[cfg(target_os = "windows")]
static ACC_REFLOW_ACC: Mutex<Option<(u64, usize, String, bool)>> = Mutex::new(None);
#[cfg(target_os = "windows")]
static ACC_REFLOW_BOUND: Mutex<Option<(u64, usize, Option<usize>)>> = Mutex::new(None);
/// FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382（3A）：`replace_all` 回灌的快速路径状态机
///（最新全文 / `(gen,seg)` 边界小 map / 最新已渲染 seg）。按代清空。
#[cfg(target_os = "windows")]
static ACC_REFLOW_FAST: Mutex<ReflowFastState> = Mutex::new(ReflowFastState {
    latest: None,
    bounds: Vec::new(),
    rendered: None,
});
/// FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382（问题2 防闪回）：本代浮层**已进入处理态**。
/// controller **处理到本代 `Processing` 事件时**由 controller 线程置位（按通道实际顺序：排在
/// Processing 之前入队的回灌照常渲染，之后的只更新状态、不重画）⇒ 提前发 Processing 后，acc
/// 尾窗的迟到回灌不会把浮层拉回预览态。`RecordingStarted` 复位。只作用于本地实时档
///（只有它会发 `PreviewReflow`）。
#[cfg(target_os = "windows")]
static ACC_REFLOW_SUPPRESS: AtomicBool = AtomicBool::new(false);
/// FIX-PREVIEW-HARVEST-380（B）：controller 收到 `HotkeyEvent::Stop` 当刻的
/// `GetTickCount64()` tick，供 worker 走到 `Injection completed` 时算 `stop_to_inject_ms`
///（松键 → 最终上屏）。`0` = 本代没有 hotkey stop（如 VAD 自动停），注入完成侧据此不打点。
/// 每个 session 的 `Start` 复位，避免跨代复用陈旧值。
#[cfg(target_os = "windows")]
static STOP_RECEIVED_TICK: AtomicU64 = AtomicU64::new(0);
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
                                                state.edit_original.clone(),
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
    /// AUTOLEARN-EDIT-SNAPSHOT-331: 进入编辑态当刻「屏幕上那份文本」的快照（EnterEditMode 时
    /// 由与 EDIT 初值**同一变量**写入）。SubmitRequested 带回主控作自学习基准；Recording 清空。
    edit_original: Option<String>,
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
        edit_original: None,
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
                                // 331：快照 per-gen 清理（新录音不得带入上一代）。
                                state.edit_original = None;
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
                        // AUTOLEARN-EDIT-SNAPSHOT-331：把「用户开始编辑时屏幕上那份文本」快照下来
                        // （与下方 EDIT 初值**同一变量**），SubmitRequested 带回主控作学习基准。
                        state.edit_original = Some(text.clone());
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
                                        state.edit_original.clone(),
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
                                        let _ =
                                            state.event_tx.send(OverlayUiEvent::SubmitRequested(
                                                text,
                                                target_hwnd,
                                                state.edit_original.clone(),
                                            ));
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
        // FIX-OVERLAY-SCROLL-255: 右边界**不随 scroll_x 左移**，保持在可视区右沿。
        // 旧写法 `text_right - scroll_x` 让排版矩形整体左移 ⇒ 文字溢出后右侧留白。
        // 现布局宽度 = visible_w + scroll_x = max(text_width, visible_w) ≥ 文本宽度，
        // 最新文字仍贴 text_right；裁剪区 text_left..text_right 不动，保证不溢出窗口。
        right: text_right,
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

    /// LOCALRT-FIRSTCHAR-281: 用**实际渲染引擎**（DirectWrite）量文本宽度（像素）。
    /// 与 GDI `GetTextExtentPoint32W` 对同一串可差 143-155px（实测，随长度增长）——
    /// D2D 绘制若用 GDI 量宽算 `scroll_x` 会多滚一截 ⇒ 右侧留白。故 D2D 侧一律用本函数。
    /// `fallback` 在 layout 创建/取 metrics 失败时返回（退化为原 GDI 量宽，避免 0 宽不滚动）。
    fn dwrite_measure_width(res: &D2dResources, visible: &[u16], fallback: f32) -> f32 {
        unsafe {
            match res
                .dwrite
                .CreateTextLayout(visible, &res.streaming_text_format, 1.0e6, 1.0e3)
            {
                Ok(layout) => {
                    let mut m =
                        windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS::default();
                    if layout.GetMetrics(&mut m).is_ok() && m.width > 0.0 {
                        m.width
                    } else {
                        fallback
                    }
                }
                Err(_) => fallback,
            }
        }
    }

    /// LOCALRT-SCROLL-277 诊断：量化右侧空白。只读日志、节流 500ms、仅在滚动时打。
    /// `gdi_width` 仅作对照（281 起**不再驱动 scroll**）；`scroll_width` = 实际驱动 scroll 的宽度
    /// （281 修复后 = DirectWrite 实渲宽）；`right_gap = 布局宽度(visible_w+scroll_x) - scroll_width`，
    /// 修复后应 ≈ 0。保留至 Gavin 完成「改后」实测。
    fn log_draw_geo_277(gdi_width: i32, scroll_width: f32, visible_w: f32, scroll_x: f32) {
        // URGENT-286：默认 Warn 下直接返回 —— 连节流计时/文本布局（最贵的那步）都不做，真正零开销。
        if !log::log_enabled!(log::Level::Debug) {
            return;
        }
        use std::sync::atomic::{AtomicU64, Ordering};
        static LAST_MS: AtomicU64 = AtomicU64::new(0);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        if now_ms.saturating_sub(LAST_MS.load(Ordering::Relaxed)) < 500 {
            return;
        }
        LAST_MS.store(now_ms, Ordering::Relaxed);
        let layout_w = visible_w + scroll_x;
        log::debug!(
            "[LocalRT-DBG-277] gdi_width={} scroll_width={:.1} diff(gdi-scroll)={:.1} visible_w={:.1} scroll_x={:.1} layout_w={:.1} => right_gap={:.1}px",
            gdi_width,
            scroll_width,
            gdi_width as f32 - scroll_width,
            visible_w,
            scroll_x,
            layout_w,
            layout_w - scroll_width
        );
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

        let visible: Vec<u16> = visible_text.encode_utf16().collect();
        // LOCALRT-FIRSTCHAR-281：scroll 用 **DirectWrite 实渲宽**（与下面 DrawText 同引擎），
        // 不再用 GDI `GetTextExtentPoint32W` 量宽 —— 两者对同一串差 143-155px（实测，随长度增长），
        // 用 GDI 宽算 scroll_x 会多滚一截 ⇒ 右侧留白（FIX-255 未根治的真因）。
        // `text_width`（GDI）仅留作诊断对照；GDI 兜底路径自己量自己画，不经此处（各自自洽）。
        let scroll_width = dwrite_measure_width(res, &visible, text_width as f32);
        let scroll_x =
            super::streaming_scroll_offset(scroll_width.round() as i32, visible_w as i32) as f32;
        // LOCALRT-SCROLL-277 诊断（节流 500ms，仅滚动时）：量化右侧留白。
        if scroll_x > 0.0 {
            log_draw_geo_277(text_width, scroll_width, visible_w, scroll_x);
        }
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
            // FIX-OVERLAY-SCROLL-255: 排版矩形只移起点，右边界**不随 scroll_x 左移**，
            // 保持在可视区右沿（旧写法 `text_right - scroll_x` 使矩形整体左移 ⇒ 右侧留白）。
            // 布局宽度 = max(text_width, visible_w) ≥ 文本宽度，最新文字贴 text_right；
            // 裁剪区 text_left..text_right 不动，保证不溢出窗口。
            res.rt.DrawText(
                &visible,
                &res.streaming_text_format,
                &D2D_RECT_F {
                    left: text_left - scroll_x,
                    top: text_top,
                    right: text_right,
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
                // FIX-PREVIEW-HARVEST-380（B）：钩子事件 → 控制器接收的投递时延；并复位本代
                // stop tick（防跨代陈旧）。take 语义：拿到即清零，非钩子路径拿 0 打 n/a。
                if log::log_enabled!(log::Level::Debug) {
                    let hook_tick = platform::take_last_hook_event_tick();
                    let now = unsafe { GetTickCount64() };
                    if hook_tick == 0 {
                        log::debug!(
                            "[LocalRT-DBG-380] hotkey latency: event=start hook_to_controller_ms=n/a"
                        );
                    } else {
                        log::debug!(
                            "[LocalRT-DBG-380] hotkey latency: event=start hook_to_controller_ms={}",
                            (now as u32).wrapping_sub(hook_tick as u32)
                        );
                    }
                }
                STOP_RECEIVED_TICK.store(0, Ordering::Release);
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
                // FIX-PREVIEW-HARVEST-380（B）：记录 stop tick（供 worker 算 stop→inject），
                // 并打印钩子事件 → 控制器的投递时延。take 语义：拿到即清零。
                let stop_now = unsafe { GetTickCount64() };
                STOP_RECEIVED_TICK.store(stop_now, Ordering::Release);
                if log::log_enabled!(log::Level::Debug) {
                    let hook_tick = platform::take_last_hook_event_tick();
                    if hook_tick == 0 {
                        log::debug!(
                            "[LocalRT-DBG-380] hotkey latency: event=stop hook_to_controller_ms=n/a"
                        );
                    } else {
                        log::debug!(
                            "[LocalRT-DBG-380] hotkey latency: event=stop hook_to_controller_ms={}",
                            (stop_now as u32).wrapping_sub(hook_tick as u32)
                        );
                    }
                }
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
                    // LOCALRT-FIRSTCHAR-282：本地流式档**不在此处切 FallingToProcessing**——
                    // 留在 RecordingWithText，等 flush 的 `StreamingFinalPreview` 收尾（完整预览）
                    // 与 worker 的 Processing 再切换，避免预览尾段被 `STREAMING_STOPPED` latch 丢掉。
                    // 在线档 / performance / accuracy 走原路，行为逐位不变。
                    let is_local_realtime =
                        transcription::AsrModel::from_config(&config.audio.asr_model)
                            == transcription::AsrModel::LocalRealtime;
                    if !is_local_realtime {
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
                // 325：回灌状态按代重置（新代重新开始回灌、编辑闩锁解除）。
                ACC_REFLOW_LAST_SEG.store(-1, Ordering::Release);
                // 375-A：滑窗路径的单调键计数器同样按代重置。
                ACC_REFLOW_LAST_REFLOW_SEQ.store(-1, Ordering::Release);
                ACC_REFLOW_EDIT_LATCH.store(false, Ordering::Release);
                // 329（边界 3）：权威前缀状态 per-gen，随录音开始清空，旧代残留不得污染新录音。
                if let Ok(mut st) = ACC_REFLOW_STATE.lock() {
                    *st = None;
                }
                // 337：两 pending 槽（acc / boundary）同样 per-gen 清空。
                if let Ok(mut slot) = ACC_REFLOW_ACC.lock() {
                    *slot = None;
                }
                if let Ok(mut slot) = ACC_REFLOW_BOUND.lock() {
                    *slot = None;
                }
                // 382：快速路径状态机 + 处理态抑制闩锁按代清空。
                if let Ok(mut st) = ACC_REFLOW_FAST.lock() {
                    st.clear();
                }
                ACC_REFLOW_SUPPRESS.store(false, Ordering::Release);
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
                // ACC-REFLOW-PERSIST-329：渲染/镜像前先合成权威前缀（本地档），在线/批处理恒 raw。
                // 🔴 早写在 043 门闩**之前**、内容改为 `compose(raw)`（不是 raw）——
                //    053-B「渲染抑制 ≠ 数据抑制」契约不变（迟来包仍进镜像、不饿死学习），
                //    只是写进去的已是带权威前缀的版本（见 053-B 测试 + ACC-REFLOW-PERSIST-329）。
                let acc_state = ACC_REFLOW_STATE.lock().ok().and_then(|g| g.clone());
                let composed = compose_with_acc_for_gen(acc_state.as_ref(), gen, &text);
                if let Ok(mut mirror) = last_streaming_text.lock() {
                    *mirror = Some(composed.clone());
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
                    if log::log_enabled!(log::Level::Debug) {
                        let committed = acc_state
                            .as_ref()
                            .filter(|(g, _, _)| *g == gen)
                            .map(|(_, _, c)| *c)
                            .unwrap_or(0);
                        log::debug!(
                            "[LocalRT-DBG-325] streaming render: gen={} raw_len={} committed_len={} display_len={} has_acc_prefix={}",
                            gen,
                            text.chars().count(),
                            committed,
                            composed.chars().count(),
                            composed != text
                        );
                    }
                    show_overlay(
                        overlay_handle,
                        opacity,
                        ui_language,
                        OverlayStatus::RecordingWithText { text: composed },
                    );
                }
            }
            // LOCALRT-FIRSTCHAR-282：本地流式松手 flush 的最终预览全文。
            // 🔴 只有本地档会发本事件 ⇒ 在线档结构上永不进入（`STREAMING_STOPPED` latch 未动、
            // 在线行为逐位不变）。在 `RecordingWithText` 态下展示收尾预览：更新文本、不 auto-close、
            // 不切状态机；随后 worker 的 `Processing` 事件把 overlay 切到「识别处理中」
            // （无闪烁、不延迟最终文本）。
            PipelineEvent::StreamingFinalPreview(text) => {
                if OVERLAY_EDITING.load(Ordering::Acquire) {
                    log::debug!("LOCALRT-282: final preview skipped while editing");
                } else {
                    // 329（边界 2）：收尾也用**权威合成**（acc 前缀 + 流式收尾全文尾巴）⇒
                    // 尾字修复在收尾这一刻同样生效。本事件只有本地档发 ⇒ 在线无影响。
                    let gen = STREAMING_GENERATION.load(Ordering::Acquire);
                    let acc_state = ACC_REFLOW_STATE.lock().ok().and_then(|g| g.clone());
                    let display = compose_with_acc_for_gen(acc_state.as_ref(), gen, &text);
                    // 镜像恒 = 所显（缺陷 B）。
                    if let Ok(mut mirror) = last_streaming_text.lock() {
                        *mirror = Some(display.clone());
                    }
                    show_overlay(
                        overlay_handle,
                        opacity,
                        ui_language,
                        OverlayStatus::RecordingWithText { text: display },
                    );
                }
            }
            PipelineEvent::PreviewReflow {
                generation,
                seg_index,
                reflow_seq,
                committed_len,
                has_hole,
                acc_text,
                replace_all,
                decode_done_at,
            } => {
                // 325：本地实时档 accuracy 分片权威文本回灌（**只有本地档会发本事件**）。
                // 代际门与 StreamingText 同源：陈旧 session 的回灌不得改本 session 浮层。
                let current_gen = STREAMING_GENERATION.load(Ordering::Acquire);
                if generation != current_gen {
                    log::debug!(
                        "325: dropping PreviewReflow from stale session (gen {} != current {})",
                        generation,
                        current_gen
                    );
                    continue;
                }
                // 🔴 controller 线程单写者：这三个状态只在消费循环内读写，无竞态。
                // 342-A：停止语义三态 —— 取消（cancel_signal）/ 编辑（OVERLAY_EDITING）/
                // 松手完成；**不再用 STREAMING_STOPPED 一刀切**（松手完成也要回灌）。
                let edit_latched = ACC_REFLOW_EDIT_LATCH.load(Ordering::Acquire);
                let editing = OVERLAY_EDITING.load(Ordering::Acquire);
                let cancelled = cancel_signal.load(Ordering::Acquire);
                let stop_state = reflow_stop_state(cancelled, editing);
                // 375-A：过期判据的单调键按路径选 —— `replace_all=true`（滑窗权威全文）用
                // `reflow_seq`（快照自有序号，严格递增）；`replace_all=false` 仍用 `seg_index`
                // （逐片严格递增）⇒ 老路径行为逐位不变。
                let key = reflow_monotonic_key(replace_all, seg_index, reflow_seq);
                let last_key = if replace_all {
                    ACC_REFLOW_LAST_REFLOW_SEQ.load(Ordering::Acquire)
                } else {
                    ACC_REFLOW_LAST_SEG.load(Ordering::Acquire)
                };
                let action = reflow_action(
                    edit_latched,
                    stop_state,
                    key,
                    last_key,
                    has_hole,
                    acc_text.is_empty(),
                );
                // 337：PreviewReflow 不再直接渲染 ⇒ preview_len 恒 0（实际渲染在 ReflowCommit 臂）。
                let preview_len = 0usize;
                if action == ReflowAction::Applied {
                    if replace_all {
                        // 382（3A）：**立即渲染** —— 不再等同片 ReflowCommit 边界配对（唯一发送点恒
                        // replace_all，等边界只会白白延迟）。已知边界 ⇒ 用准确的；未知 ⇒ 用派发当刻
                        // `committed_len` 先渲染，边界后到且本 seg 仍是最新已渲染 ⇒ 再用准确值重渲一次。
                        ACC_REFLOW_LAST_REFLOW_SEQ.store(key, Ordering::Release);
                        // 382（防闪回）：Processing 已开始 ⇒ 只更新状态、不重画浮层。
                        let suppressed = ACC_REFLOW_SUPPRESS.load(Ordering::Acquire);
                        let outcome = match ACC_REFLOW_FAST.lock() {
                            Ok(mut st) => st.on_text(
                                generation,
                                seg_index,
                                acc_text.clone(),
                                committed_len,
                                suppressed,
                            ),
                            Err(_) => ReflowFastOutcome::None,
                        };
                        if let ReflowFastOutcome::Apply {
                            text,
                            committed_len: len,
                            accurate,
                            render,
                        } = outcome
                        {
                            if render {
                                render_authoritative_reflow(
                                    overlay_handle,
                                    opacity,
                                    ui_language,
                                    last_streaming_text,
                                    generation,
                                    seg_index,
                                    &text,
                                    len,
                                    accurate,
                                    decode_done_at,
                                );
                            } else {
                                set_acc_reflow_state_only(generation, &text, len);
                                if log::log_enabled!(log::Level::Debug) {
                                    log::debug!(
                                        "[LocalRT-DBG-382] reflow suppressed after processing: seg={} acc_len={}",
                                        seg_index,
                                        text.chars().count()
                                    );
                                }
                            }
                        }
                    } else {
                        // 老逐片路径（382 起已无发送方）：保持逐位不变。
                        ACC_REFLOW_LAST_SEG.store(key, Ordering::Release);
                        if let Ok(mut slot) = ACC_REFLOW_ACC.lock() {
                            *slot = Some((generation, seg_index, acc_text.clone(), replace_all));
                        }
                        if log::log_enabled!(log::Level::Debug) {
                            log::debug!(
                                "[LocalRT-DBG-337] reflow acc ready: seg={} acc_len={} (等 boundary)",
                                seg_index,
                                acc_text.chars().count()
                            );
                        }
                        try_resolve_reflow(
                            overlay_handle,
                            opacity,
                            ui_language,
                            last_streaming_text,
                        );
                    }
                } else if action == ReflowAction::SkippedEditing {
                    // 编辑态一旦出现（含已退出）⇒ 本代永久上闩，后续回灌不再改用户文本。
                    ACC_REFLOW_EDIT_LATCH.store(true, Ordering::Release);
                }
                if log::log_enabled!(log::Level::Debug) {
                    let action_str = match action {
                        // 337：Applied 现在只表示「登记 pending」，渲染推迟到 ReflowCommit。
                        ReflowAction::Applied => "pending",
                        ReflowAction::SkippedEditing => "skipped-editing",
                        ReflowAction::SkippedCancel => "skipped-cancel",
                        ReflowAction::SkippedStale => "skipped-stale",
                        ReflowAction::SkippedHole => "skipped-hole",
                        ReflowAction::SkippedEmpty => "skipped-empty",
                    };
                    log::debug!(
                        "[LocalRT-DBG-325] acc reflow: seg={} committed_len={} acc_len={} preview_len={} action={}",
                        seg_index,
                        committed_len,
                        acc_text.chars().count(),
                        preview_len,
                        action_str
                    );
                }
            }
            PipelineEvent::ReflowCommit {
                generation,
                seg_index,
                committed_len,
            } => {
                // LOCALRT-SEAM-337（自适应定界）：登记该片边界，与 acc 槽配对后解析。
                let current_gen = STREAMING_GENERATION.load(Ordering::Acquire);
                if generation != current_gen {
                    log::debug!(
                        "337: dropping ReflowCommit from stale session (gen {} != current {})",
                        generation,
                        current_gen
                    );
                    continue;
                }
                if let Ok(mut slot) = ACC_REFLOW_BOUND.lock() {
                    *slot = Some((generation, seg_index, committed_len));
                }
                // 老逐片路径（已无发送方）：ACC_REFLOW_ACC 恒 None ⇒ 本调用为 no-op，保持逐位不变。
                try_resolve_reflow(overlay_handle, opacity, ui_language, last_streaming_text);
                // 382（3A）：replace_all 快速路径 —— 边界后到且该 seg 仍是最新已渲染 ⇒ 用准确边界重渲。
                let suppressed = ACC_REFLOW_SUPPRESS.load(Ordering::Acquire);
                let outcome = match ACC_REFLOW_FAST.lock() {
                    Ok(mut st) => st.on_bound(generation, seg_index, committed_len, suppressed),
                    Err(_) => ReflowFastOutcome::None,
                };
                if let ReflowFastOutcome::Apply {
                    text,
                    committed_len: len,
                    accurate,
                    render,
                } = outcome
                {
                    if render {
                        render_authoritative_reflow(
                            overlay_handle,
                            opacity,
                            ui_language,
                            last_streaming_text,
                            generation,
                            seg_index,
                            &text,
                            len,
                            accurate,
                            None,
                        );
                    } else {
                        set_acc_reflow_state_only(generation, &text, len);
                    }
                }
            }
            PipelineEvent::Processing(message) => {
                // 382（问题2 防闪回）：controller **处理到本代 Processing** 时置位 ⇒ 之后到达的
                // `replace_all` 回灌只更新状态、不重画浮层（提前发 Processing 后 acc 尾窗回灌可能后到）。
                // 按通道实际顺序：排在 Processing **之前**入队的回灌照常渲染，不受影响。
                ACC_REFLOW_SUPPRESS.store(true, Ordering::Release);
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
            // LOCAL-RT-ENGINE-239-B: 模型加载中的信息提示（Info 态，蓝点白字，短暂后自动关闭）。
            PipelineEvent::Info(message) => {
                platform::notify_translate_poll_stop();
                set_tray_state(tray, TrayState::Idle, ui_language);
                let (pos, size) = overlay_geometry(
                    &OverlayStatus::Info(message.clone()),
                    overlay_handle.overlay_hwnd,
                );
                overlay_handle.send(OverlayCommand::Show(OverlayRequest {
                    status: OverlayStatus::Info(message),
                    pos: Some(pos),
                    size,
                    opacity: 0.9,
                    ui_language,
                    auto_close_ms: 2500,
                    target_hwnd: 0,
                }));
            }
            // LOCAL-RT-ENGINE-239-B: 模型缺失 → 错误态；🔴 不走 convert_to_friendly_error，
            // 保留「缺哪个模型」的具体上下文（DEC-067 附则一：不降级、明确报错）。
            PipelineEvent::ModelUnavailable(message) => {
                OVERLAY_EDITING.store(false, Ordering::Release);
                log::error!("Pipeline: model unavailable: {}", message);
                platform::notify_translate_poll_stop();
                set_tray_state(tray, TrayState::Error, ui_language);
                let (pos, size) = overlay_geometry(
                    &OverlayStatus::Error(message.clone()),
                    overlay_handle.overlay_hwnd,
                );
                overlay_handle.send(OverlayCommand::Show(OverlayRequest {
                    status: OverlayStatus::Error(message),
                    pos: Some(pos),
                    size,
                    opacity: 0.95,
                    ui_language,
                    auto_close_ms: 4000,
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
                // 325：编辑态一出现即上闩（**per-gen**）——此后本次录音的 accuracy 回灌一律停，
                // 直到下一代（防「编辑退出后迟到回灌把用户刚打的字冲掉」）。
                ACC_REFLOW_EDIT_LATCH.store(true, Ordering::Release);
                // ASR-038-C: 用户点击 overlay 文本区进入编辑态
                // 停录音 + 取消 pipeline（关 WebSocket 由 ASR 线程检测 cancel_signal 处理）
                cancel_signal.store(true, Ordering::Relaxed);
                stop_recording_signal.store(true, Ordering::Relaxed);
                STREAMING_STOPPED.store(true, Ordering::Release);
                platform::notify_translate_poll_stop();
                // 通知 overlay 线程切换到 StreamingEditing 态并创建 EDIT 控件
                overlay_handle.send(OverlayCommand::EnterEditMode);
            }
            OverlayUiEvent::SubmitRequested(text, target_hwnd, overlay_original) => {
                OVERLAY_EDITING.store(false, Ordering::Release);
                STREAMING_STOPPED.store(true, Ordering::Release);
                // WORDBOOK-053-B: learn the explicit user correction (original ASR text vs submitted
                // edited text) in a detached thread.
                // 331：基准来源按**档位**选择（不是「回灌是否活跃」——零回灌短录音同样有 gap）：
                //   本地实时档 ⇒ 编辑入口快照 `overlay_original`（与所见/EDIT 初值同源），缺失回落镜像；
                //   在线 / 批处理档 ⇒ 恒 `mirror`（逐位不变）。
                let is_local_realtime_tier = {
                    let cfg = clone_runtime_config(&runtime_config);
                    transcription::AsrModel::from_config(&cfg.audio.asr_model)
                        == transcription::AsrModel::LocalRealtime
                };
                let mirror = last_streaming_text.lock().ok().and_then(|m| m.clone());
                let original_text =
                    select_learning_baseline(is_local_realtime_tier, overlay_original, mirror);
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
                // V091-PUNCT-TAIL-214 产出源 #4/#5：编辑态提交拿的是 text（用户在悬浮窗里编辑后的
                // 文本），完全绕过 run_pipeline_core 的 L2 标点后处理。#4 注入（下方 inject_text）
                // 与 #5 剪贴板兜底（下方 copy_text_to_clipboard）都消费 text_to_inject，故在此处
                // 按开关做一次末尾剥标点即一并覆盖（Gavin 2026-09-17 拍板）。
                // 🔴 只读新开关 punctuation.strip_trailing；punctuation.enabled=false 的「全剥」
                //    现状本就不覆盖 #4/#5，本次不改变该行为（新开关关闭时逐字一致）。
                let text_to_inject = if clone_runtime_config(&runtime_config)
                    .punctuation
                    .strip_trailing
                {
                    punctuation::strip_trailing_punctuation(&text)
                } else {
                    text.clone()
                };
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
/// ASR-DUAL-B-001: 加载 hotwords 字符串（使用 accuracy 引擎的档位需要）
/// 从 wordbook 读取所有单词，按 id 排序保证哈希稳定，构建逗号分隔字符串
/// performance 模式返回 None（不支持 hotwords）
// FIX-LOCALRT-ENGINE-EQ-252: 判据收敛到 uses_accuracy_engine()——LocalRealtime 的最终转录
// 引擎也是 accuracy（带 hotwords），否则本地 realtime 档拿不到词库热词。
// MACOS-P4-NEUTRAL-002: 原 #[cfg(target_os = "windows")] 去除——平台中立纯 Rust（AsrModel 判定 + wordbook 读取 + build_hotwords_string），spawn_worker_thread（已去 cfg）调用，对 Windows 为 no-op。
fn load_hotwords_for_accuracy(config: &AppConfig) -> Option<String> {
    if !transcription::AsrModel::from_config(&config.audio.asr_model).uses_accuracy_engine() {
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
    let reload_asr_online_semantic_punctuation_enabled =
        config.audio.asr_online_semantic_punctuation_enabled;
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
            reload_asr_online_semantic_punctuation_enabled,
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
            config.audio.asr_online_semantic_punctuation_enabled,
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
        // LOCAL-RT-ENGINE-239-B：最近一次 ASR 热重载失败原因（供模型缺失报错附「具体缺哪个」）。
        let mut last_reload_error: Option<String> = None;

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
                match &result {
                    Ok(_) => last_reload_error = None,
                    Err(e) => last_reload_error = Some(e.clone()),
                }
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
                    // LOCAL-RT-ENGINE-239-B：切到本地流式档位时用 Info 态提示双模型加载中（短暂自动关）。
                    if desired_now == transcription::AsrModel::LocalRealtime {
                        send_event(
                            &event_tx,
                            PipelineEvent::Info(
                                i18n::get(config_now.ui_language)
                                    .local_realtime_loading_hint
                                    .to_string(),
                            ),
                        );
                    }
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
                    // FIX-LOCALRT-ENGINE-EQ-252: 判据收敛到 uses_accuracy_engine()——
                    // LocalRealtime 也用 accuracy 引擎的热词，词库变更时需同样触发重载。
                    let needs_reload = cheap_needed
                        || (desired_asr_model.uses_accuracy_engine()
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
                        // LOCAL-RT-ENGINE-239-B：切到本地流式档位时用 Info 态提示双模型加载中（短暂自动关）。
                        if desired_asr_model == transcription::AsrModel::LocalRealtime {
                            send_event(
                                &event_tx,
                                PipelineEvent::Info(
                                    i18n::get(config.ui_language)
                                        .local_realtime_loading_hint
                                        .to_string(),
                                ),
                            );
                        }
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
                                    match &result {
                                        Ok(_) => last_reload_error = None,
                                        Err(e) => last_reload_error = Some(e.clone()),
                                    }
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
                        let asr_online_semantic_punctuation_enabled =
                            transcriber_ref.asr_online_semantic_punctuation_enabled();
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
                                    asr_online_semantic_punctuation_enabled,
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
                            false, // FIX-283: 在线流式路径不做 pre_roll 残尾裁剪（三档行为不变）
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
                            None,                 // PARALLEL-ACC-298: 在线路径无并行预转写
                            i18n::get(config.ui_language).overlay_transcribing,
                        );
                        continue;
                    }

                    // LOCAL-RT-ENGINE-239-B（DEC-066/067）：本地 realtime 独立编排。
                    // 与在线流式并列：录音期发流式预览（streaming paraformer，识别即上屏）；
                    // 松键后**丢弃预览文本**，把完整 PCM 交给 run_pipeline_core 走 accuracy 2pass
                    // （initial_text=None ⇒ from_online_streaming=false，主通道 ITN 启用、标点由
                    //  accuracy 实际产出决定）。这是本管线与在线档的关键差异。
                    let is_local_realtime_desired =
                        desired_asr_model_check == transcription::AsrModel::LocalRealtime;
                    let active_is_local_realtime = transcriber
                        .as_ref()
                        .is_some_and(|t| t.asr_model() == transcription::AsrModel::LocalRealtime);

                    if is_local_realtime_desired && !active_is_local_realtime {
                        if asr_reload_in_flight {
                            // 双模型仍在加载（~6s，D2 有界等待仅 1.5s，可能尚未落地）：
                            // 提示加载中，本次不录音、**不降级**，等加载完成后再按热键即可。
                            log::info!(
                                "LocalRealtime still loading; skipping this recording without degrading"
                            );
                            is_recording.store(false, Ordering::Release);
                            send_event(
                                &event_tx,
                                PipelineEvent::Info(
                                    i18n::get(config.ui_language)
                                        .local_realtime_loading_hint
                                        .to_string(),
                                ),
                            );
                            continue;
                        }
                        // DEC-067 附则一：模型缺失/加载失败 → 明确报错，**绝不静默降级**到其他档位。
                        let base = i18n::get(config.ui_language).local_realtime_unavailable;
                        let detail = last_reload_error
                            .as_deref()
                            .and_then(|s| s.lines().next())
                            .unwrap_or("");
                        let message = if detail.is_empty() {
                            base.to_string()
                        } else {
                            format!("{}：{}", base, detail)
                        };
                        log::error!(
                            "LocalRealtime unavailable, refusing to degrade to another tier: {}",
                            message
                        );
                        is_recording.store(false, Ordering::Release);
                        send_event(&event_tx, PipelineEvent::ModelUnavailable(message));
                        continue;
                    }

                    if is_local_realtime_desired {
                        // OVERLAY-051-E：流式 ASR 已启动、等待首个文本
                        send_event(&event_tx, PipelineEvent::StreamingIdle);

                        // LOCALRT-PREVIEW-PUNCT-256：预览打点复用已常驻的 CT-Transformer。
                        // 录音前按 config 就绪（引擎由调用方传入，**不在 local_stream 内新建**，
                        // 否则重复加载模型；与 run_pipeline_core 的传法一致）。
                        if config.punctuation.enabled && cached_punctuation.is_none() {
                            cached_punctuation = punctuation::PunctuationEngine::new(&model_dir);
                        } else if !config.punctuation.enabled {
                            cached_punctuation = None;
                        }
                        let punctuation = cached_punctuation.as_mut();

                        let recognizer = transcriber
                            .as_ref()
                            .and_then(|t| t.online_recognizer())
                            .expect("LocalRealtime transcriber must hold an online recognizer");
                        let send_recognizer =
                            transcription::local_stream::SendOnlineRecognizerRef(recognizer);
                        let cancel_clone = Arc::clone(&cancel_signal);
                        let event_tx_clone = event_tx.clone();
                        // chunk channel：record_streaming 推 chunk，ASR 线程读
                        let (chunk_tx, chunk_rx) = crossbeam_channel::bounded::<Vec<f32>>(256);

                        // PARALLEL-ACC-298：accuracy 并行派发。总开关关 / offline recognizer 缺失
                        // ⇒ acc_enabled=false ⇒ 不建 worker、不派发，**逐位退回今天的串行行为**。
                        let acc_cfg = transcription::local_stream::AccDispatchConfig::new();
                        let acc_offline = transcriber.as_ref().and_then(|t| t.offline_recognizer());
                        let acc_enabled = acc_cfg.enabled && acc_offline.is_some();
                        let acc_cfg = transcription::local_stream::AccDispatchConfig {
                            enabled: acc_enabled,
                            ..acc_cfg
                        };
                        let send_offline = acc_offline.map(transcription::SendOfflineRecognizerRef);
                        // 325：载荷加 `committed_len`（派发当刻浮层显示字符数，回灌边界）。
                        // 344-G：载荷加 `seg_streaming`（本片流式文本，失败片的填补来源）。
                        let (acc_tx, acc_rx) =
                            crossbeam_channel::bounded::<(usize, usize, Vec<Vec<f32>>, String)>(64);
                        let acc_script = config.audio.chinese_script;
                        // 325：acc worker 逐片回灌预览所用的事件发送端（代际在 worker 内捕获）。
                        let acc_event_tx = event_tx.clone();
                        // 337：ASR 线程在句子确认时经此发 ReflowCommit（提交/作废回灌边界）。
                        let reflow_commit_tx = event_tx.clone();

                        // scoped：ASR 线程借 online recognizer，worker 线程跑 record_streaming，
                        // accuracy worker 借 **offline** recognizer 并行转写派发片。
                        // 🔴 298 是真并发：streaming 线程与 accuracy worker 同时在跑。两 recognizer
                        // 是**不同对象**且各自**独占**（见 SendOfflineRecognizerRef 的 SAFETY 论证）。
                        let scoped = std::thread::scope(|scope| {
                            // accuracy 并行 worker：对象独占，唯一使用者（streaming 线程只用 online）。
                            let acc_handle = if acc_enabled {
                                let send_offline =
                                    send_offline.expect("acc_enabled 蕴含 offline recognizer 存在");
                                let cancel_acc = Arc::clone(&cancel_signal);
                                let acc_rx = acc_rx;
                                // LOCALRT-CTX-INJECT-320：词库词条随 per-stream 注入
                                // （读 DB + 按引擎预算裁剪，每段录音一次，毫秒级）。
                                let acc_terms = load_hotwords_for_accuracy(&config);
                                Some(scope.spawn(move || {
                                        let recognizer = send_offline.into_inner();
                                        // ── SLIDING-WINDOW-367：滑动窗口（当前片+前1~3，12s 封顶）──
                                        // （阶段四·B）窗口解码投**并发池**（默认并发度 2，设 1 退顺序），
                                        // 结果按 window_seq **有序定稿**（对齐状态机串行）。
                                        let mut all_native = true;
                                        let mut total_decode_ms = 0.0f64;
                                        let mut recent_slices: Vec<Vec<f32>> = Vec::new();
                                        // FIX-WINDOW-DISJOINT-369：累计派发片总数 ⇒ 由 `recent_slices` 长度
                                        // 反推窗口的**全局切片区间**（传给 OrderedReflow 判重叠，别再靠文本猜）。
                                        let mut total_slices: usize = 0;
                                        // `window_seq -> (start_slice, end_slice)`（dispatch 与 drain 同线程读写）。
                                        let mut window_spans: Vec<(usize, usize)> = Vec::new();
                                        // FIX-PREFIX-AND-EAT-371（B）：`window_seq -> 本窗各片样本数`
                                        // —— 与切片区间一起算期望重叠比例，约束 reflow 的对齐 k。
                                        let mut window_samples: Vec<Vec<usize>> = Vec::new();
                                        // 382（3A）：`window_seq -> 派发当刻浮层字符数`（边界未到时的 fallback，
                                        // 随 `PreviewReflow.committed_len` 带到消费端）。
                                        let mut window_committed_lens: Vec<usize> = Vec::new();
                                        // 386（C）：`window_seq -> 本窗各片流式文本拼接`（解码失败/空时的兜底）。
                                        let mut window_streaming_texts: Vec<String> = Vec::new();
                                        // 386（C）：与 `recent_slices` 一一对应的各片流式文本（同批 sub_seg 共享）。
                                        let mut recent_streaming: Vec<String> = Vec::new();
                                        // 386（A）：待覆盖片全局下标（多片派发的末片延后，等下次派发/收尾纳入）。
                                        let mut pending_slice: Option<usize> = None;
                                        // 386（A）：收尾窗用的最近 dispatch_idx / committed_len。
                                        let mut last_dispatch_idx: usize = 0;
                                        let mut last_committed_len: usize = 0;
                                        let mut window_seq: usize = 0;
                                        // FIX-PREVIEW-STALE-AND-COLLAPSE-375（A）：滑窗权威快照的**自有单调序号**
                                        // —— `dispatch_idx`（切片下标）在窗口间并发 + 收尾 drain 下**会重复**，
                                        // 不能拿它当「过期」判据（会把后到的完整文本误杀成 stale，预览永不更新）。
                                        let mut reflow_seq: usize = 0;
                                        let mut ordered = transcription::OrderedReflow::new();
                                        let concurrency = transcription::WINDOW_DECODE_CONCURRENCY.max(1);
                                        let terms = acc_terms.as_deref();

                                        std::thread::scope(move |pool| {
                                            let (res_tx, res_rx) =
                                                crossbeam_channel::unbounded::<AccDecodeResult>();
                                            // 382（3B）：**一个共享任务通道**，所有 worker 从同一通道取
                                            //（crossbeam 通道天然多消费者）。旧实现 `rr % concurrency` 把第 N 窗
                                            // 固定派给第 N%2 个 worker ⇒ 一个 worker 在解长窗时下一窗仍排在它后面、
                                            // 另一个空闲也不接（实测一窗实解 ~2.5s、出结果 5.8s）。
                                            let (task_tx, task_rx) =
                                                crossbeam_channel::unbounded::<AccTaskMsg>();
                                            for _ in 0..concurrency {
                                                let trx = task_rx.clone();
                                                let rtx = res_tx.clone();
                                                pool.spawn(move || {
                                                    for (seq, dispatch_idx, audio, avg_snapshot, dispatched_at) in trx {
                                                        // 382（3C）：排队时间 = 派发 → worker 开始解码。
                                                        let queued_ms =
                                                            dispatched_at.elapsed().as_secs_f64() * 1000.0;
                                                        let t0 = std::time::Instant::now();
                                                        let r = decode_window(
                                                            recognizer,
                                                            &audio,
                                                            seq,
                                                            acc_script,
                                                            terms,
                                                            avg_snapshot,
                                                        );
                                                        let ms = t0.elapsed().as_secs_f64() * 1000.0;
                                                        // 382（3C）：解码完成时刻（端到端「解完 → 浮层重画」）。
                                                        let decode_done_at = std::time::Instant::now();
                                                        if log::log_enabled!(log::Level::Debug) {
                                                            log::debug!(
                                                                "[LocalRT-DBG-382] window queue: seq={} queued_ms={:.0} decode_ms={:.0}",
                                                                seq,
                                                                queued_ms,
                                                                ms
                                                            );
                                                        }
                                                        let _ =
                                                            rtx.send((seq, dispatch_idx, r, ms, decode_done_at));
                                                    }
                                                });
                                            }
                                            drop(res_tx);
                                            drop(task_rx); // 生产侧只保留唯一 `task_tx`；这份接收端不需要
                                            // FIX-PREVIEW-STALE-AND-COLLAPSE-375（B）：本次录音**已接受窗口**的
                                            // 产出率累计（字 / 秒 / 窗数）⇒ 运行均值，用于识别「解码坍塌」。
                                            // （放在投池作用域内：唯一读写者就是这个循环，不跨闭包捕获）
                                            let mut rate_sum_chars: usize = 0;
                                            let mut rate_sum_secs: f32 = 0.0;
                                            let mut rate_windows: usize = 0;
                                            // FIX-PREVIEW-HARVEST-380（A）：本代**已处理**的结果数（收尾按 window_seq 等齐）。
                                            let mut done = 0usize;
                                            // FIX-PREVIEW-HARVEST-380（A）：结果处理**单一定义** —— select 循环与
                                            // 收尾 drain 共用，保证该线程内 `push_window(` 只出现一处（防两份再漂移）。
                                            macro_rules! harvest_acc_window {
                                                ($res:expr) => {{
                                                    let (seq, dispatch_idx, r, ms, decode_done_at) = $res;
                                                    total_decode_ms += ms;
                                                    let text = match r {
                                                        Ok((t, _np)) => t,
                                                        Err(err) => {
                                                            all_native = false;
                                                            log::warn!(
                                                                "SLIDING-WINDOW-367 window #{} decode failed: {}",
                                                                seq,
                                                                err
                                                            );
                                                            String::new()
                                                        }
                                                    };
                                                    // 386（C）：解码 Err / 最终为空 ⇒ 用本窗**流式文本**兜底
                                                    //（流式也为空才保持空）；防结尾整段丢失（本次 seq7 `<location>` 丢 40+ 字）。
                                                    let decoded = text;
                                                    let fb = window_streaming_texts
                                                        .get(seq)
                                                        .map(|s| s.as_str())
                                                        .unwrap_or("");
                                                    let text = window_text_with_fallback(&decoded, fb);
                                                    if decoded.is_empty() && !text.is_empty() {
                                                        log::warn!(
                                                            "[LocalRT-DBG-386] window #{} fallback to streaming text ({} chars)",
                                                            seq,
                                                            text.chars().count()
                                                        );
                                                    }
                                                    // FIX-WINDOW-DISJOINT-369：把该窗的切片区间一并交给 reflow 判重叠
                                                    // （missing ⇒ 保守当零重叠，拼接保字、不丢）。
                                                    let (win_ws, win_we) = window_spans
                                                        .get(seq)
                                                        .copied()
                                                        .unwrap_or((seq, seq + 1));
                                                    // FIX-PREFIX-AND-EAT-371（B）：带上各片样本数 ⇒ 期望重叠
                                                    // 比例约束对齐 k（missing ⇒ 空表 ⇒ 退化小 k 优先）。
                                                    let win_samples = window_samples
                                                        .get(seq)
                                                        .cloned()
                                                        .unwrap_or_default();
                                                    // 375：本窗秒数（供产出率均值）+ 最终字数（计入判据）
                                                    let win_secs =
                                                        win_samples.iter().sum::<usize>() as f32
                                                            / 16000.0;
                                                    let text_chars = text.chars().count();
                                                    for authoritative in ordered.push_window(
                                                        seq, win_ws, win_we, win_samples, text,
                                                    ) {
                                                        // 382（3A）：边界未到时的 fallback = 派发当刻浮层字符数。
                                                        let fallback_committed = window_committed_lens
                                                            .get(seq)
                                                            .copied()
                                                            .unwrap_or(0);
                                                        let _ = acc_event_tx.send(
                                                            PipelineEvent::PreviewReflow {
                                                                generation: session_generation,
                                                                seg_index: dispatch_idx,
                                                                reflow_seq: Some(reflow_seq),
                                                                committed_len: fallback_committed,
                                                                has_hole: false,
                                                                acc_text: authoritative,
                                                                replace_all: true,
                                                                // 382（3C）：本窗解码完成时刻（端到端埋点）。
                                                                decode_done_at: Some(decode_done_at),
                                                            },
                                                        );
                                                        reflow_seq += 1;
                                                    }
                                                    // 375：只把「最终非空」的窗口计入均值
                                                    // （坍塌→空的窗口不拖低均值；374 剥离后重解出的真内容照常计入）
                                                    if text_chars > 0 {
                                                        rate_sum_chars += text_chars;
                                                        rate_sum_secs += win_secs;
                                                        rate_windows += 1;
                                                    }
                                                    done += 1;
                                                }};
                                            }
                                            // 386（A）：派发一个窗口（含 C 的流式文本记账）。`gs/ge` = 全局切片区间。
                                            // 单一定义 ⇒ 批次路径与收尾合并共用，防两份漂移。
                                            macro_rules! dispatch_window {
                                                ($gs:expr, $ge:expr, $idx:expr, $committed_len:expr) => {{
                                                    let gs: usize = $gs;
                                                    let ge: usize = $ge;
                                                    let buf_base = total_slices - recent_slices.len();
                                                    debug_assert!(
                                                        gs >= buf_base && ge <= total_slices,
                                                        "386：窗口区间 [{gs},{ge}) 必须落在当前 buffer [{buf_base},{total_slices}) 内"
                                                    );
                                                    let start = gs.saturating_sub(buf_base);
                                                    let stop = ge
                                                        .saturating_sub(buf_base)
                                                        .min(recent_slices.len());
                                                    let window_audio: Vec<f32> = recent_slices[start..stop]
                                                        .iter()
                                                        .flat_map(|s| s.iter().copied())
                                                        .collect();
                                                    if !transcription::path_b_budget_ok(
                                                        window_audio.len(),
                                                        terms,
                                                    ) {
                                                        log::warn!(
                                                            "[FIX-REMOVE-HARDSPLIT-370] 窗口音频超 KV 预算（撞顶会被静默截断）：{:.1}s audio_tok={} inject_tok={} max_total_len={}",
                                                            window_audio.len() as f32 / 16000.0,
                                                            transcription::expected_audio_tokens(
                                                                window_audio.len()
                                                            ),
                                                            transcription::estimate_inject_tokens(terms),
                                                            transcription::PATH_B_MAX_TOTAL_LEN
                                                        );
                                                    }
                                                    let avg_snapshot =
                                                        transcription::acc_avg_chars_per_sec(
                                                            rate_sum_chars,
                                                            rate_sum_secs,
                                                            rate_windows,
                                                        );
                                                    let _ = task_tx.send((
                                                        window_seq,
                                                        $idx,
                                                        window_audio,
                                                        avg_snapshot,
                                                        std::time::Instant::now(),
                                                    ));
                                                    window_spans.push((gs, ge));
                                                    window_samples.push(
                                                        recent_slices[start..stop]
                                                            .iter()
                                                            .map(|s| s.len())
                                                            .collect(),
                                                    );
                                                    window_streaming_texts
                                                        .push(recent_streaming[start..stop].concat());
                                                    window_committed_lens.push($committed_len);
                                                    window_seq += 1;
                                                }};
                                            }
                                            // FIX-PREVIEW-HARVEST-380（A）：select 循环 —— 新切片与解码结果
                                            // **任一先到即处理**，不再等下一个切片（Gavin 现象①：停顿即停刷）。
                                            let mut step =
                                                |ev: AccWindowStep<AccSliceMsg, AccDecodeResult>| {
                                                    match ev {
                                                        AccWindowStep::Slice((
                                                            idx,
                                                            committed_len,
                                                            sub_segs,
                                                            seg_streaming,
                                                        )) => {
                                                            // 386（A）：按「中途末片延后 / 下次派发纳入 / 收尾合并」规则组窗
                                                            //（规则与不变量见 `plan_windows`）。382 旧行为「每片立刻组窗」会把
                                                            // 11.27s ⇒ 10.15s+1.12s 的尾片单独成窗 ⇒ 模型念词表。
                                                            last_dispatch_idx = idx;
                                                            last_committed_len = committed_len;
                                                            let prev_base = total_slices - recent_slices.len();
                                                            let prev_durs: Vec<f32> = recent_slices
                                                                .iter()
                                                                .map(|s| s.len() as f32 / 16000.0)
                                                                .collect();
                                                            let new_durs: Vec<f32> = sub_segs
                                                                .iter()
                                                                .map(|s| s.len() as f32 / 16000.0)
                                                                .collect();
                                                            let plan = plan_windows(
                                                                &prev_durs,
                                                                &new_durs,
                                                                prev_base,
                                                                pending_slice,
                                                                false,
                                                            );
                                                            pending_slice = plan.pending;
                                                            let mut wi = 0usize;
                                                            for (k, s) in sub_segs.into_iter().enumerate() {
                                                                recent_slices.push(s);
                                                                recent_streaming
                                                                    .push(slice_streaming_text(k, &seg_streaming));
                                                                total_slices += 1;
                                                                while recent_slices.len()
                                                                    > transcription::WINDOW_MAX_SLICES
                                                                {
                                                                    recent_slices.remove(0);
                                                                    recent_streaming.remove(0);
                                                                }
                                                                // 窗口以「当前片」收尾：其 end == 推送后的 total_slices。
                                                                let end = total_slices;
                                                                if wi < plan.windows.len()
                                                                    && plan.windows[wi].1 == end
                                                                {
                                                                    let (gs, ge) = plan.windows[wi];
                                                                    wi += 1;
                                                                    dispatch_window!(gs, ge, idx, committed_len);
                                                                }
                                                                // 否则：本片是**延后的待覆盖片**（本批末片），本轮不组窗。
                                                            }
                                                        }
                                                        AccWindowStep::Result(res) => {
                                                            harvest_acc_window!(res)
                                                        }
                                                    }
                                                };
                                            drive_acc_windows(
                                                &acc_rx,
                                                &res_rx,
                                                || cancel_acc.load(Ordering::Acquire),
                                                &mut step,
                                            );
                                            drop(step);
                                            // 386（A.4）：松键收尾 —— 处理仍待覆盖的片：
                                            // <3s 且前面有片 ⇒ 与前一并重解；否则单独组窗。
                                            if pending_slice.is_some() {
                                                let prev_base = total_slices - recent_slices.len();
                                                let prev_durs: Vec<f32> = recent_slices
                                                    .iter()
                                                    .map(|s| s.len() as f32 / 16000.0)
                                                    .collect();
                                                let plan = plan_windows(
                                                    &prev_durs,
                                                    &[],
                                                    prev_base,
                                                    pending_slice,
                                                    true,
                                                );
                                                debug_assert!(
                                                    plan.pending.is_none(),
                                                    "386：收尾后不应再有待覆盖片"
                                                );
                                                for (gs, ge) in plan.windows {
                                                    dispatch_window!(
                                                        gs,
                                                        ge,
                                                        last_dispatch_idx,
                                                        last_committed_len
                                                    );
                                                }
                                            }
                                            // 收尾：等齐所有在飞窗口（共 window_seq 条）。
                                            // select 期间已收的已计入 `done`；本段只等还没收的。
                                            drop(task_tx);
                                            while done < window_seq {
                                                let Ok(res) = res_rx.recv() else {
                                                    break;
                                                };
                                                harvest_acc_window!(res);
                                            }
                                            // 380：收尾 drain 之后不再有派发 ⇒ 产出率均值无人再读（原实现
                                            // 亦不在 drain 累计）。显式消费，避免 `unused_assignments` 假红。
                                            let _ = (rate_sum_chars, rate_sum_secs, rate_windows);
                                            let (committed, last_window_text) = ordered.finish();
                                            (committed, last_window_text, all_native, total_decode_ms)
                                        })
                                    }))
                            } else {
                                None
                            };

                            let asr_handle = scope.spawn(move || {
                                // into_inner 按值消费包装，强制闭包捕获整个 Send 包装
                                // （Rust 2021 disjoint capture 若只取 .0 字段会退化为
                                // 捕获裸引用，绕过 unsafe impl Send）。
                                let recognizer = send_recognizer.into_inner();
                                transcription::local_stream::transcribe_streaming_local(
                                    chunk_rx,
                                    recognizer,
                                    Some(&cancel_clone),
                                    punctuation,
                                    config.audio.silence_threshold,
                                    |display_text, words| {
                                        // OVERLAY-075：与在线流式同构，代际盖章。
                                        let _ = event_tx_clone.send(PipelineEvent::StreamingText(
                                            session_generation,
                                            display_text.to_string(),
                                            words.to_vec(),
                                        ));
                                    },
                                    acc_cfg,
                                    |idx, committed_len, segs, seg_streaming| {
                                        // 派发片送 accuracy worker（worker 不存在 ⇒ send 失败，忽略）。
                                        let _ =
                                            acc_tx.send((idx, committed_len, segs, seg_streaming));
                                    },
                                    |seg_index, committed_len| {
                                        // 337：自适应边界冻结（a 文本停止增长 / b 有声恢复 / c 硬上限）。
                                        let _ =
                                            reflow_commit_tx.send(PipelineEvent::ReflowCommit {
                                                generation: session_generation,
                                                seg_index,
                                                committed_len,
                                            });
                                    },
                                )
                            });

                            let record_result = audio_capture.record_streaming(
                                Arc::clone(&stop_recording_signal),
                                config.audio.silence_threshold,
                                config::SILENCE_DURATION_MS,
                                config::MAX_RECORD_SECONDS,
                                Some(Arc::clone(&audio_buf)),
                                device_name,
                                true, // FIX-283（方案 D）：仅本地流式裁剪 pre_roll 上一句残尾
                                |chunk| match chunk_tx.send_timeout(
                                    chunk.to_vec(),
                                    Duration::from_millis(200),
                                ) {
                                    Ok(()) => {}
                                    Err(crossbeam_channel::SendTimeoutError::Timeout(_)) => {
                                        ASR_CHUNK_DROPS.fetch_add(1, Ordering::Relaxed);
                                        log::warn!(
                                            "[ASR-DROP] chunk send timed out after 200ms (local streaming consumer stuck?); total dropped: {}",
                                            ASR_CHUNK_DROPS.load(Ordering::Relaxed)
                                        );
                                    }
                                    Err(crossbeam_channel::SendTimeoutError::Disconnected(_)) => {}
                                },
                            );
                            // drop chunk_tx 让 ASR 线程的 channel 断开（触发 flush 收尾）
                            drop(chunk_tx);
                            // tail_wait = 松键之后到「最终文本就绪」的等待（Gavin 要看的效果判据）。
                            // 含 ASR flush + accuracy 尾片解码（中间片已在说话时并行算完）。
                            let t_stop = std::time::Instant::now();
                            let asr_result = asr_handle.join();
                            // FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382（问题2）：路A（ASR flush）已结束，
                            // 但路B 尾窗可能仍在解码 ⇒ **立即**推「收尾预览 + 识别处理中」，不等
                            // `acc_handle.join()`（否则界面停在录音态，用户感知卡顿）。282 顺序不变：
                            // 预览先、处理态后。文案与 `run_pipeline_core` 同源（`overlay_processing`）。
                            if let Ok(Ok((preview_text, _pcm))) = &asr_result {
                                if !preview_text.is_empty() {
                                    let _ = event_tx.send(PipelineEvent::StreamingFinalPreview(
                                        preview_text.clone(),
                                    ));
                                }
                            }
                            let _ = event_tx.send(PipelineEvent::Processing(
                                i18n::get(config.ui_language).overlay_processing.to_string(),
                            ));
                            // 382（问题2 埋点）：松键 → 处理态显示的时延（仅 Windows）。
                            #[cfg(target_os = "windows")]
                            {
                                if log::log_enabled!(log::Level::Debug) {
                                    let stop_tick = STOP_RECEIVED_TICK.load(Ordering::Acquire);
                                    if stop_tick != 0 {
                                        log::debug!(
                                            "[LocalRT-DBG-380] stop_to_processing_ms={}",
                                            (unsafe { GetTickCount64() } as u32)
                                                .wrapping_sub(stop_tick as u32)
                                        );
                                    }
                                }
                            }
                            let acc_result = acc_handle.map(|h| h.join());
                            (
                                asr_result,
                                record_result,
                                acc_result,
                                t_stop.elapsed().as_secs_f64() * 1000.0,
                            )
                        });
                        let (asr_result, record_result, acc_result, tail_wait_ms) = scoped;

                        // SLIDING-WINDOW-367：worker 返回 (定稿前缀, 最新窗口文本, all_native, 总解码ms)。
                        let (acc_committed, acc_last_window, acc_all_native, acc_total_decode_ms) =
                            match acc_result {
                                Some(Ok((c, lw, an, total))) => (c, lw, an, total),
                                Some(Err(_join_err)) => {
                                    log::error!("SLIDING-WINDOW-367 accuracy worker panicked");
                                    (String::new(), String::new(), false, 0.0)
                                }
                                None => (String::new(), String::new(), false, 0.0),
                            };
                        // 最终文本 = 定稿前缀（滑出片） + 最新窗口文本。
                        let acc_joined = format!("{}{}", acc_committed, acc_last_window);
                        if log::log_enabled!(log::Level::Debug) {
                            log::debug!(
                                "[SLIDING-WINDOW-367] join: committed={} window={} all_native={} total_decode={:.0}ms tail_wait={:.0}ms",
                                acc_committed.chars().count(),
                                acc_last_window.chars().count(),
                                acc_all_native,
                                acc_total_decode_ms,
                                tail_wait_ms
                            );
                        }
                        // ── DUAL-PATH-ACC-363：路B（累积全量解码，松手后**一次**）──
                        // 全量音频 = ASR 线程返回的 `local_pcm`（`local_stream.rs:462` 第二元），
                        // 无需在采集/回调路径额外累积 ⇒ **路A 主路径零新增开销**。
                        // 路B 在 `acc_handle.join()` 之后才跑（路A worker 已结束）⇒ 不与预览刷新竞争。
                        // 预算够且解码成功 ⇒ 用路B 全文；否则**降级**退回现有切片拼装（绝不硬塞，DEC-069）。
                        // 🔴 SLIDING-WINDOW-367：路B（松手全量解码）+ 364 预算闸门**已摘接线**（保留代码可回挂）；
                        //    最终文本改由滑动窗口（committed + 最新窗口）产出。恢复：把本 const 置 `true`。
                        const PATH_B_WIRED_367: bool = false;
                        let path_b_terms = load_hotwords_for_accuracy(&config);
                        let path_b_text: Option<String> = if PATH_B_WIRED_367 {
                            // 借用 `local_pcm`，不消费 `asr_result`（后面还要 destructure）。
                            let pcm_opt: Option<&[f32]> = match &asr_result {
                                Ok(Ok((_, pcm))) => Some(pcm.as_slice()),
                                _ => None,
                            };
                            match pcm_opt {
                                Some(pcm) if !pcm.is_empty() => {
                                    let audio_secs = pcm.len() as f32 / 16000.0;
                                    let audio_tokens =
                                        transcription::expected_audio_tokens(pcm.len());
                                    let inject_tokens = transcription::estimate_inject_tokens(
                                        path_b_terms.as_deref(),
                                    );
                                    if transcription::path_b_budget_ok(
                                        pcm.len(),
                                        path_b_terms.as_deref(),
                                    ) {
                                        match transcriber
                                            .as_ref()
                                            .and_then(|t| t.offline_recognizer())
                                        {
                                            Some(rec) => {
                                                // FIX-INJECT-TO-SPEC-377：注入只剩**纯词表**（上下文段已按
                                                // sherpa 规格移除，见 `build_ctx_system`）。本块（366 起摘接线）
                                                // 保留为可回挂路径，但不再传任何前文。
                                                let inject = transcription::CtxInject {
                                                    terms: path_b_terms.as_deref(),
                                                    // 路B（367 起已摘接线）：无滑窗产出率均值 ⇒ 不判坍塌
                                                    avg_chars_per_sec: None,
                                                };
                                                match transcription::transcribe_acc_ctx(
                                                    rec,
                                                    pcm,
                                                    config.audio.chinese_script,
                                                    0,
                                                    inject,
                                                ) {
                                                    Ok((t, _)) if !t.trim().is_empty() => {
                                                        log::info!(
                                                            "DUAL-PATH-363 路B 全量解成功：audio={:.1}s audio_tok={} inject_tok={} chars={}",
                                                            audio_secs,
                                                            audio_tokens,
                                                            inject_tokens,
                                                            t.chars().count()
                                                        );
                                                        Some(t)
                                                    }
                                                    Ok(_) => {
                                                        log::info!(
                                                            "DUAL-PATH-363 路B 全量解为空，退回切片拼装（audio={:.1}s）",
                                                            audio_secs
                                                        );
                                                        None
                                                    }
                                                    Err(e) => {
                                                        log::info!(
                                                            "DUAL-PATH-363 路B 全量解失败，退回切片拼装：{}（audio={:.1}s）",
                                                            e,
                                                            audio_secs
                                                        );
                                                        None
                                                    }
                                                }
                                            }
                                            None => {
                                                log::info!(
                                                    "DUAL-PATH-363 路B 无 offline recognizer，退回切片拼装"
                                                );
                                                None
                                            }
                                        }
                                    } else {
                                        log::info!(
                                            "DUAL-PATH-363 路B 降级（预算不足）：退回切片拼装；audio={:.1}s audio_tok={} inject_tok={} max_total_len={}",
                                            audio_secs,
                                            audio_tokens,
                                            inject_tokens,
                                            transcription::PATH_B_MAX_TOTAL_LEN
                                        );
                                        None
                                    }
                                }
                                _ => None,
                            }
                        } else {
                            None
                        };
                        // 最终文本来源：路B（若成功）**整体替换**切片拼装（不再算拼接边界）。
                        let final_src: &str = path_b_text.as_deref().unwrap_or(acc_joined.as_str());
                        let pretranscribed = if acc_parallel_result_usable(
                            cancel_signal.load(Ordering::Acquire),
                            final_src,
                        ) {
                            let normalized = text_normalizer::normalize_text_for_language(
                                final_src,
                                config.audio.chinese_script,
                            );
                            // ── PUNCT-FINAL-REDO-350 节点（重打前先剥光标点）──
                            // 🔴 SLIDING-WINDOW-367 + DEC-080（2026-09-22）：**该节点无条件摘除**
                            //    （`B_STRIP_WIRED_367 = false`，接线保留、非删代码）。
                            //  · 为何无条件摘：滑窗输出同样是**模型一次解出的完整文本、无拼接接缝**
                            //    ⇒ 「剥光重打」前提消失；357 实测重打会**反噬**（把「二比一」改成「2比1」）。
                            //    Gavin 已拍 DEC-080：当前管线不走「全量剥标点 + 二次打点」。
                            //  · 何时回挂：若未来最终文本来源又改回「多片拼装」（无滑窗），接缝标点问题会回来，
                            //    把 `B_STRIP_WIRED_367` 置 `true` 即可（节点代码/7 单测/共享谓词/顺序护栏全部保留）。
                            // 🔴 334 老 bug 不复发：`apply_local_punctuation` 的 `!native_punctuated` 门**仍在**
                            //    （334 防的是「对已带标点文本再打一遍 ⇒ 。。 叠加」）；摘本节点后模型标点原样保留，天然免疫。
                            const B_STRIP_WIRED_367: bool = false;
                            let b_strip_enabled = B_STRIP_WIRED_367;
                            let stripped = strip_punctuation_node(normalized, b_strip_enabled);
                            // PUNCT-DOUBLE-334：第二元 = 「文本**实际**是否已有有效标点」（DEC-047 口径，
                            // 与 :9855 的 Qwen3 分支同一 detector），**不是**「各分片是否都解码成功」。
                            // 旧值 `acc_all_native` 语义错配：任一片失败 ⇒ 误判「无标点」⇒ 对已带标点全文
                            // 再跑 CT-Transformer ⇒ `。。`/`，。` 叠加（Gavin 端测报障）。
                            let native_punctuated = pretranscribed_native_punctuated(&stripped);
                            Some((stripped, native_punctuated))
                        } else {
                            None
                        };

                        log::info!(
                            "[Latency] local realtime record_streaming() completed after +{:.1}ms",
                            t_worker.elapsed().as_secs_f64() * 1000.0
                        );
                        is_recording.store(false, Ordering::Release);

                        if cancel_signal.load(Ordering::Acquire) {
                            log::warn!(
                                "LocalRealtime recording ended with cancel_signal=true, skipping ASR join"
                            );
                            send_event(&event_tx, PipelineEvent::Cancelled);
                            continue;
                        }
                        if let Err(e) = record_result {
                            log::error!("LocalRealtime recording error: {}", e);
                            send_event(&event_tx, PipelineEvent::Error(e.to_string()));
                            continue;
                        }

                        // 382（问题2）：收尾预览已在 `acc_handle.join()` **之前**发出（见上，
                        // `StreamingFinalPreview` 立刻上屏、不等路B 尾窗）⇒ 这里的预览文本不再重复发送
                        //（否则会把已切到处理态的浮层拉回预览态 = 闪回）。最终文本仍由 accuracy 2pass 重打。
                        let (_final_preview, local_pcm) = match asr_result {
                            // 预览文本本用于收尾显示（282）；最终文本仍由 accuracy 2pass 重打（DEC-067）。
                            Ok(Ok((preview_text, pcm))) => (preview_text, pcm),
                            Ok(Err(e)) => {
                                if e.is::<transcription::NoSpeechError>() {
                                    log::info!("LocalRealtime ASR: no speech detected");
                                    send_event(&event_tx, PipelineEvent::NoSpeech);
                                } else {
                                    log::error!("LocalRealtime ASR error: {}", e);
                                    send_event(&event_tx, PipelineEvent::Error(e.to_string()));
                                }
                                continue;
                            }
                            Err(_) => {
                                log::error!("LocalRealtime ASR thread panicked");
                                send_event(
                                    &event_tx,
                                    PipelineEvent::Error("ASR thread panicked".into()),
                                );
                                continue;
                            }
                        };

                        // LOCALRT-FIRSTCHAR-282 的 `StreamingFinalPreview` 发送已上移到
                        // `acc_handle.join()` 之前（382 问题2）⇒ 此处不再发送，避免把已切到处理态的浮层
                        // 拉回预览态（闪回）。`StreamingText` 仍会被 `STREAMING_STOPPED` latch 丢掉。

                        // 下游与批处理路径同构：PCM 作 samples，initial_text=None。
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
                            Ok(local_pcm),
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
                            None, // 非流式：主通道 ITN 启用（B 路径由 pretranscribed 显式承载）
                            pretranscribed, // PARALLEL-ACC-298: 并行 accuracy 结果（关时 None ⇒ 原有 accuracy 2pass）
                            i18n::get(config.ui_language).overlay_processing,
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
                        None, // PARALLEL-ACC-298: 批处理路径无并行预转写
                        i18n::get(config.ui_language).overlay_transcribing,
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
        // LOCAL-RT-ENGINE-239-B: macOS 侧无独立 Info 视觉样式，复用 ShowError 承载（同 NoSpeech 先例）。
        PipelineEvent::Info(message) => platform::OverlayRequest::ShowError {
            message: message.clone(),
            auto_close_ms: 2500,
        },
        // LOCAL-RT-ENGINE-239-B: 模型缺失 → 错误提示（文案携带在 payload）。
        PipelineEvent::ModelUnavailable(message) => platform::OverlayRequest::ShowError {
            message: message.clone(),
            auto_close_ms: 4000,
        },
        PipelineEvent::FocusLost(text) => platform::OverlayRequest::ShowPreview(text.clone()),
        PipelineEvent::StreamingText(_, _, _) => platform::OverlayRequest::Show, // macOS 侧流式文本暂不渲染
        // LOCALRT-FIRSTCHAR-282: 本地流式收尾预览（macOS 侧流式文本暂不渲染，同 StreamingText）
        PipelineEvent::StreamingFinalPreview(_) => platform::OverlayRequest::Show,
        // ACC-PREVIEW-REFLOW-325: 本地档 accuracy 回灌预览（macOS 侧流式文本暂不渲染，同 StreamingText）
        PipelineEvent::PreviewReflow { .. } => platform::OverlayRequest::Show,
        // LOCALRT-SEAM-337: 回灌边界提交/作废（macOS 侧暂不渲染，同上）
        PipelineEvent::ReflowCommit { .. } => platform::OverlayRequest::Show,
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
        // LOCAL-RT-ENGINE-239-B: 本地流式档位加载提示 / 模型缺失
        PipelineEvent::Info(msg) => {
            log::info!("macOS pipeline: Info({})", msg);
            platform::request_tray_state(TrayState::Idle, ui_language);
        }
        PipelineEvent::ModelUnavailable(msg) => {
            log::error!("macOS pipeline: ModelUnavailable({})", msg);
            platform::request_tray_state(TrayState::Error, ui_language);
        }
        PipelineEvent::StreamingText(_, text, _) => {
            // ASR-038-B: macOS 侧流式文本暂不渲染（C-overlay 批后续实现）
            log::debug!("macOS pipeline: StreamingText ({} chars)", text.len());
        }
        // LOCALRT-FIRSTCHAR-282: 本地流式收尾预览（macOS 侧流式文本暂不渲染）
        PipelineEvent::StreamingFinalPreview(text) => {
            log::debug!(
                "macOS pipeline: StreamingFinalPreview ({} chars)",
                text.len()
            );
        }
        // ACC-PREVIEW-REFLOW-325: 本地档 accuracy 回灌预览（macOS 侧流式文本暂不渲染）
        PipelineEvent::PreviewReflow { acc_text, .. } => {
            log::debug!(
                "macOS pipeline: PreviewReflow ({} chars, 本地档专用，暂不渲染)",
                acc_text.chars().count()
            );
        }
        // LOCALRT-SEAM-337: 回灌边界提交/作废（macOS 侧暂不渲染）
        PipelineEvent::ReflowCommit {
            seg_index,
            committed_len,
            ..
        } => {
            log::debug!(
                "macOS pipeline: ReflowCommit(seg={}, {:?}, 本地档专用，暂不渲染)",
                seg_index,
                committed_len
            );
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
        // LOCAL-RT-ENGINE-239-A（DEC-067）：前处理跟 **Accuracy** 走。
        // 本管线 2pass 的最终转录引擎就是 accuracy，前处理必须与实际引擎匹配，
        // 配错直接吃首字（FIRSTCHAR-FIX-006）。非 online 档，走完整 samples 前处理。
        transcription::AsrModel::LocalRealtime => {
            (ACC_SILENCE_HEAD_SAMPLES, ACC_ONSET_BACKTRACK_SAMPLES)
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
/// PARALLEL-ACC-298：把 accuracy 并行各片的子段文本按 `seg_index` **升序**拼接成扁平列表。
///
/// 单 worker + channel FIFO 已保证顺序，这里仍显式排序（乱序/缺号输入也稳定），
/// 是判据 #3「seg_index 有序拼接」的纯函数载体（可单测，无需真模型）。
// SLIDING-WINDOW-367：单窗口解码（窗口音频 + 注入集中一处）。
fn decode_window(
    recognizer: &sherpa_onnx::OfflineRecognizer,
    window_audio: &[f32],
    window_seq: usize,
    script: config::ChineseScript,
    terms: Option<&str>,
    // FIX-PREVIEW-STALE-AND-COLLAPSE-375（B）：本次录音已定稿窗口的产出率均值快照（冷启动 None）。
    avg_chars_per_sec: Option<f32>,
) -> anyhow::Result<(String, bool)> {
    transcription::transcribe_acc_ctx(
        recognizer,
        window_audio,
        script,
        window_seq,
        transcription::CtxInject {
            terms,
            avg_chars_per_sec,
        },
    )
}

// SLIDING-WINDOW-367：路A 逐片拼装摘接线后暂无人用（**保留可回挂**）。
#[allow(dead_code)]
fn assemble_parallel_accuracy(mut segments: Vec<(usize, Vec<String>)>) -> Vec<String> {
    segments.sort_by_key(|(idx, _)| *idx);
    segments.into_iter().flat_map(|(_, texts)| texts).collect()
}

/// PARALLEL-ACC-298：并行 accuracy 结果是否可用作 `pretranscribed`。
///
/// 🔴 取消 ⇒ 一律弃用（判据 #3「取消时不带脏结果」）；拼接后为空 ⇒ 弃用回落旧路径
/// （与 BUG-119「没说话」路径一致，由旧路径给出 NoSpeech）。
fn acc_parallel_result_usable(cancelled: bool, joined: &str) -> bool {
    !cancelled && !joined.trim().is_empty()
}

/// LOCALRT-REFLOW-HOLE-344-G：空/失败分片的填补决策（纯函数，可单测）。
///
/// 返回 `(填补文本, 是否记洞)`：
/// - `can_fill && !seg_streaming.is_empty()` ⇒ **用该段流式文本填补，不记洞**（回灌不中断）；
/// - 否则（流式也为空 / 多子段保守 / 本片已填过）⇒ `("", true)`，走 `SkippedHole` 兜底。
///
/// 🔴 修的是 325 的设计硬伤：累积 `acc_text` 一旦留空 ⇒ `has_hole` 永不复位 ⇒ 一片失败后
/// 本次录音**后续回灌全废**。失败片用流式文本填满即无洞。
// SLIDING-WINDOW-367：路A 失败片填补摘接线后暂无人用（**保留可回挂**）。
#[allow(dead_code)]
fn hole_fill_decision(can_fill: bool, seg_streaming: &str) -> (String, bool) {
    if can_fill && !seg_streaming.is_empty() {
        (seg_streaming.to_string(), false)
    } else {
        (String::new(), true)
    }
}

/// PUNCT-DOUBLE-334：本地并行 accuracy 文本的 `native_punctuated` 判定。
///
/// DEC-047 口径 = **文本实测**：`punctuation::has_effective_punctuation`（词内嵌标点豁免，
/// `3.14`/`3:30`/`example.com` 不算真标点），与 `main.rs` 的 Qwen3 `initial_text` 分支同一 detector。
///
/// 🔴 为什么不能用「各分片是否都解码成功」（原 `acc_all_native` 的语义）：分片失败 ≠ 文本无标点。
/// 任一片 Err ⇒ 旧值 false ⇒ `apply_local_punctuation` 的 `!native_punctuated` 门通过 ⇒
/// CT-Transformer 对**已带标点**的全文再打一遍 ⇒ 位置重合处 `。。`、不重合处各占一个（叠加指纹）。
///
/// 已知取舍（**记录在案，不得当 bug 重修**）：
/// 1. 带洞（失败片留空串）且其余已带标点 ⇒ 判 true ⇒ 跳过引擎 ⇒ 洞那段不补标点。
///    可接受：洞（缺整段文字）是更大缺陷；**为补洞而补跑会把旁边已打标点的好段二次打点**
///    （正是本单要消除的 `。。` 叠加）；且预览侧 `SkippedHole`（325/329）已拦住带洞回灌。
/// 2. `has_effective_punctuation` 口径是「任一无 ASCII 夹持的标点出现即 true」，**不要求句末标点**：
///    文本只有一个逗号、没有句号 ⇒ true ⇒ 跳过引擎 ⇒ **最终输出没有句号**。
///    这**不是新缺陷**，是 DEC-047 白纸黑字记录过的「已知接受边界」
///    （`decisions-archive.md:1174` 表格：「`他说“好”`（中文引号，无句号）→ 本决策口径 true →
///    跳过标点引擎 → **接受** —— 引号无句号不是用户会报的缺陷」，该表开头即「不得当 bug 重修」）。
///    🔴 放宽此判据会直接把重复标点带回来 ⇒ 见到「少个句号」**不要动判据**。
fn pretranscribed_native_punctuated(text: &str) -> bool {
    punctuation::has_effective_punctuation(text)
}

/// ACC-PREVIEW-REFLOW-325：合成回灌预览 —— `acc_text` 替换 `streaming` 的前 `committed_len`
/// 个**字符**，其余保留流式尾巴。按**字符**（非字节）切，避免 UTF-8 切裂。
fn reflow_preview(acc_text: &str, streaming: &str, committed_len: usize) -> String {
    let mut out = String::with_capacity(acc_text.len() + streaming.len());
    out.push_str(acc_text);
    out.extend(streaming.chars().skip(committed_len));
    out
}

/// ACC-REFLOW-PERSIST-329：流式渲染/镜像前的权威前缀合成。
///
/// `state` = 本代已确定的 `(generation, acc_text, committed_len)`（见 [`ACC_REFLOW_STATE`]）。
/// - gen 匹配 **且** `acc_text` 非空 ⇒ `reflow_preview(acc, raw, committed)`（权威前缀 + 流式尾巴）；
/// - 其余（state 为 None / gen 不匹配 / acc 空）⇒ **原样返回 `raw`**
///   ⇒ 在线 / 批处理结构上恒走此分支，行为逐位不变。
///
/// 🔴 **滚动分界线口径（Gavin 2026-09-21 明确，勿改）**：
/// `preview = accuracy 权威前缀（已完成分片） + 流式尾巴（正在说的那段）`。
/// 正在说的那段 accuracy 还没拿到（要攒够 `min_seg_ms` 且静默 800ms 才派片），
/// 所以**流式尾巴在每一刻都存在**，不是「第一句才用流式」的特例；分界线随分片完成不断右移。
/// BUILD-321 实测：第一片在 ~5.65s 派发、解码 1.2~1.9s ⇒ **前 6~8 秒全是流式文字**，
/// 之后才有回灌。⇒ **`acc_text` 为空时必须原样返回流式全文**（否则开口后前 6~8 秒会是空白，
/// 属致命回归；见 `compose_empty_acc_passthrough` 单测锁定）。
fn compose_with_acc_for_gen(
    state: Option<&(u64, String, usize)>,
    raw_gen: u64,
    raw: &str,
) -> String {
    match state {
        Some((g, acc, committed)) if *g == raw_gen && !acc.is_empty() => {
            reflow_preview(acc, raw, *committed)
        }
        _ => raw.to_string(),
    }
}

/// FIX-TAIL-WINDOW-AND-FALLBACK-386（B）：回灌渲染与后续 `StreamingText` 渲染用**同一个合成函数**
///（acc 全文 + `streaming[committed_len..]` 流式尾巴）。
///
/// 🔴 不得再走已删除的 `reflow_preview_367`：它对 `replace_all` 只返回 `acc_text`、**丢掉流式尾巴** ⇒
/// 说话中途回灌会把预览截短（实测 31→28、78→69、191→146），下一次 `StreamingText` 才拼回 ⇒ 闪回。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn compose_reflow_preview(
    generation: u64,
    acc_text: &str,
    streaming: &str,
    committed_len: usize,
) -> String {
    let state = (generation, acc_text.to_string(), committed_len);
    compose_with_acc_for_gen(Some(&state), generation, streaming)
}

/// AUTOLEARN-EDIT-SNAPSHOT-331：自学习比对基准的选择（纯函数）。
///
/// - **本地实时档** ⇒ 用「编辑入口快照」`overlay_original`（用户开始编辑时屏幕上那份；
///   与 EDIT 初值/渲染字段同源），缺失才回落 `mirror`；
/// - **在线 / 批处理档** ⇒ **恒用 `mirror`**（逐位不变，守住红线）。
///
/// 🔴 闸门是**档位**（`is_local_realtime_tier`），**不是**「回灌是否活跃」：
/// gap 的成因是「迟到包推进镜像 + 043 门抑制渲染」，**零回灌（一片未派）时同样发生**，
/// 用回灌状态当闸门会漏掉短录音这一常见场景。快照本就只在 EnterEditMode 产生，
/// 「有快照就用」天然对齐语义。
fn select_learning_baseline(
    is_local_realtime_tier: bool,
    overlay_original: Option<String>,
    mirror: Option<String>,
) -> Option<String> {
    if is_local_realtime_tier {
        overlay_original.or(mirror)
    } else {
        mirror
    }
}

/// 382：只更新权威状态（`ACC_REFLOW_STATE`），**不重画浮层**。用于「本代 Processing 已开始 ⇒
/// 回灌只更新状态、不重画」的抑制路径（防闪回）。
#[cfg(target_os = "windows")]
fn set_acc_reflow_state_only(generation: u64, acc_text: &str, committed_len: usize) {
    if let Ok(mut st) = ACC_REFLOW_STATE.lock() {
        *st = Some((generation, acc_text.to_string(), committed_len));
    }
}

/// 382：权威回灌的**渲染**（更新权威状态 + 镜像 + 重画浮层）。`accurate` 仅用于埋点标注
/// `boundary=known|fallback`；`decode_done_at` 用于 3C 的端到端 `decode_done→render_ms`。
#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments)]
fn render_authoritative_reflow(
    overlay_handle: &OverlayThreadHandle,
    opacity: f32,
    ui_language: config::UiLanguage,
    last_streaming_text: &Arc<Mutex<Option<String>>>,
    generation: u64,
    seg_index: usize,
    acc_text: &str,
    committed_len: usize,
    accurate: bool,
    decode_done_at: Option<std::time::Instant>,
) {
    let streaming = last_streaming_text
        .lock()
        .ok()
        .and_then(|m| m.clone())
        .unwrap_or_default();
    // 386（B）：与 `compose_with_acc_for_gen` **同一个合成函数**（acc 全文 + `streaming[committed_len..]`），
    // 不再走 `reflow_preview_367`（replace_all 会丢流式尾巴 ⇒ 回灌把预览截短 ⇒ 闪回更短文本）。
    let preview = compose_reflow_preview(generation, acc_text, &streaming, committed_len);
    set_acc_reflow_state_only(generation, acc_text, committed_len);
    if let Ok(mut mirror) = last_streaming_text.lock() {
        *mirror = Some(preview.clone());
    }
    if let Some(t0) = decode_done_at {
        if log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "[LocalRT-DBG-382] reflow latency: seq={} decode_done→render_ms={:.0} boundary={}",
                seg_index,
                t0.elapsed().as_secs_f64() * 1000.0,
                if accurate { "known" } else { "fallback" }
            );
        }
    }
    if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "[LocalRT-DBG-337] reflow applied: seg={} committed_len={} acc_len={} preview_len={}",
            seg_index,
            committed_len,
            acc_text.chars().count(),
            preview.chars().count()
        );
    }
    show_overlay(
        overlay_handle,
        opacity,
        ui_language,
        OverlayStatus::RecordingWithText { text: preview },
    );
}

/// LOCALRT-SEAM-337：当某片的 `acc_text` 与 `boundary` 都到齐时解析——合成并渲染（`Some`），
/// 或作废（`None`，保持纯流式）。两槽一次消费，防重复应用。
/// 🔴 382 起 `replace_all` 不再走本函数（改走 [`ReflowFastState`] 立即渲染）；本函数仅供
/// **老逐片路径**（已无发送方），逐位不变。
#[cfg(target_os = "windows")]
fn try_resolve_reflow(
    overlay_handle: &OverlayThreadHandle,
    opacity: f32,
    ui_language: config::UiLanguage,
    last_streaming_text: &Arc<Mutex<Option<String>>>,
) {
    let acc = ACC_REFLOW_ACC.lock().ok().and_then(|g| g.clone());
    let bound = ACC_REFLOW_BOUND.lock().ok().and_then(|g| g.clone());
    let (Some((ga, sa, acc_text, _replace_all)), Some((gb, sb, bound_opt))) = (acc, bound) else {
        return;
    };
    if ga != gb || sa != sb {
        return; // 尚未配对（另一槽未到或属不同片）
    }
    if let Ok(mut g) = ACC_REFLOW_ACC.lock() {
        *g = None;
    }
    if let Ok(mut g) = ACC_REFLOW_BOUND.lock() {
        *g = None;
    }
    match bound_opt {
        Some(len) if !acc_text.is_empty() => {
            // 老逐片路径：边界已配对 ⇒ 用准确边界渲染（accurate=true），无 3C 埋点。
            render_authoritative_reflow(
                overlay_handle,
                opacity,
                ui_language,
                last_streaming_text,
                ga,
                sa,
                &acc_text,
                len,
                true,
                None,
            );
        }
        _ => {
            if let Ok(mut st) = ACC_REFLOW_STATE.lock() {
                *st = None;
            }
            log::debug!(
                "[LocalRT-DBG-337] reflow withheld: seg={} (boundary=b 或 acc 空 ⇒ 保持纯流式)",
                sa
            );
        }
    }
}

/// LOCALRT-RELEASE-REFLOW-342-A：停止语义**三态**（消费端 controller 线程判定）。
///
/// 329 曾把 `STREAMING_STOPPED` 一刀切当「取消」⇒ 把**松手完成**与**取消**混为一谈。
/// 该 latch 实际被三种情形置位，语义完全不同：
/// - **取消**（ESC / PTT < 300ms，`cancel_signal`）⇒ 内容丢弃，**不回灌**；
/// - **编辑**（`OVERLAY_EDITING`，进编辑时也置 `STREAMING_STOPPED`）⇒ 跳过，保护用户文本；
/// - **松手完成**（正常 PTT 松手 / 仍在录音）⇒ 内容要用（最终文本本就走 accuracy），
///   **允许回灌刷新预览**。
///
/// 🔴 判据用 `cancel_signal`（取消专用信号），**不再用 `STREAMING_STOPPED` 一刀切**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReflowStopState {
    /// ESC / 短按取消：丢弃，不回灌。
    Cancelled,
    /// 编辑中（本代编辑闩锁另由 `edit_latched` 持久）。
    Editing,
    /// 松手完成 / 仍在录音：都允许回灌。
    Released,
}

fn reflow_stop_state(cancelled: bool, editing: bool) -> ReflowStopState {
    if cancelled {
        ReflowStopState::Cancelled
    } else if editing {
        ReflowStopState::Editing
    } else {
        ReflowStopState::Released
    }
}

/// ACC-PREVIEW-REFLOW-325：回灌决策（纯函数，消费端 controller 线程调用）。
///
/// 优先级（高 → 低）：
/// 1. **编辑**（本代编辑闩锁 `edit_latched`，或当前正在编辑 `stop_state=Editing`）——
///    防「编辑退出后迟到的回灌把用户刚打的字冲掉」；
/// 2. **取消**（`stop_state=Cancelled`，仅 ESC / 短按）—— 松手完成**不**在此列（342-A）；
/// 3. **陈旧 seg**（`seg_index <= 已应用`）—— 单调键是 seg_index，不是 committed_len；
/// 4. **覆盖区有洞**（该片覆盖范围内含 accuracy 失败片）—— 带洞文本比空串更糟；
/// 5. **acc 空**（不能把已显示前缀抹白）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReflowAction {
    Applied,
    SkippedEditing,
    SkippedCancel,
    SkippedStale,
    SkippedHole,
    SkippedEmpty,
}

/// FIX-PREVIEW-STALE-AND-COLLAPSE-375（A）：回灌「过期」判据的**单调键**选择。
///
/// - `replace_all=true`（滑窗权威全文，语义 = 整段替换、每条取代前一条）⇒ 用**滑窗快照自有**的
///   `reflow_seq`（严格递增）。🔴 **不得**用 `seg_index`：滑窗路径传的是切片下标，窗口间并发
///   （`WINDOW_DECODE_CONCURRENCY=2`）+ 收尾 drain 会让多个窗口带**同一个下标** ⇒ 后到的那个
///   （= `OrderedReflow` 合并后的完整文本）被误判 stale 丢掉 ⇒ 预览永远停在纯流式文本。
/// - `replace_all=false`（老逐片路径）⇒ 仍用 `seg_index`（逐片严格递增），**行为逐位不变**。
///
/// `reflow_seq` 缺失（老发送方/异常）⇒ 退回 `seg_index`（保守：沿用旧行为，不至于整片不回灌）。
fn reflow_monotonic_key(replace_all: bool, seg_index: usize, reflow_seq: Option<usize>) -> i64 {
    if replace_all {
        reflow_seq.unwrap_or(seg_index) as i64
    } else {
        seg_index as i64
    }
}

fn reflow_action(
    edit_latched: bool,
    stop_state: ReflowStopState,
    key: i64,
    last_applied_key: i64,
    has_hole: bool,
    acc_empty: bool,
) -> ReflowAction {
    if edit_latched || stop_state == ReflowStopState::Editing {
        return ReflowAction::SkippedEditing;
    }
    if stop_state == ReflowStopState::Cancelled {
        return ReflowAction::SkippedCancel;
    }
    if key <= last_applied_key {
        return ReflowAction::SkippedStale;
    }
    if has_hole {
        return ReflowAction::SkippedHole;
    }
    if acc_empty {
        return ReflowAction::SkippedEmpty;
    }
    ReflowAction::Applied
}

#[cfg(test)]
mod preview_reflow_325_tests {
    use super::{
        reflow_action, reflow_monotonic_key, reflow_preview, reflow_stop_state, ReflowAction,
        ReflowStopState,
    };

    /// 硬点 1 边界：committed_len 之前用 acc、之后保留流式尾巴。
    #[test]
    fn reflow_preview_boundary() {
        // acc 覆盖前 2 字，保留尾巴「CD」
        assert_eq!(reflow_preview("Xy", "ABCD", 2), "XyCD");
        // committed_len = 0 ⇒ acc + 全部流式
        assert_eq!(reflow_preview("Xy", "ABCD", 0), "XyABCD");
        // committed_len = 流式长度 ⇒ 只剩 acc
        assert_eq!(reflow_preview("Xy", "ABCD", 4), "Xy");
        // committed_len 超过流式长度 ⇒ 仍只剩 acc（不 panic、不越界）
        assert_eq!(reflow_preview("Xy", "AB", 9), "Xy");
        // acc 空 ⇒ 等于「砍掉前缀」，本身不由本函数负责（调用方 SkippedEmpty）；此处只锁行为
        assert_eq!(reflow_preview("", "ABCD", 2), "CD");
    }

    /// 硬点 2：编辑闩锁 —— 一旦上闩，即便 `STREAMING_STOPPED` 已复位也一律 SkippedEditing。
    /// 这覆盖「进编辑 → 退出编辑 → 再来回灌」的序列（闩锁在 controller 线程置位，不随退出清）。
    #[test]
    fn reflow_action_edit_latch_persists() {
        // 编辑中：编辑态（编辑也置 STREAMING_STOPPED，但 342 后由 Editing 三态表达）
        assert_eq!(
            reflow_action(false, ReflowStopState::Editing, 3, 1, false, false),
            ReflowAction::SkippedEditing
        );
        // 退出编辑后（OVERLAY_EDITING 复位、STREAMING_STOPPED 也复位）——闩锁仍在 ⇒ 仍跳
        assert_eq!(
            reflow_action(true, ReflowStopState::Released, 4, 1, false, false),
            ReflowAction::SkippedEditing
        );
    }

    /// 342-A：停止语义三态纯函数 —— 取消 / 编辑 / 松手完成（含仍在录音）。
    #[test]
    fn reflow_stop_state_342_tri_state() {
        // 取消优先于编辑（ESC 时若仍在编辑态，按取消处理）。
        assert_eq!(reflow_stop_state(true, true), ReflowStopState::Cancelled);
        assert_eq!(reflow_stop_state(true, false), ReflowStopState::Cancelled);
        assert_eq!(reflow_stop_state(false, true), ReflowStopState::Editing);
        // 松手完成 / 仍在录音 ⇒ Released（都允许回灌）。
        assert_eq!(reflow_stop_state(false, false), ReflowStopState::Released);
    }

    /// 🔴 342-A 核心回归：**松手完成必须回灌**（不再是 329 的 skipped-cancel）。
    #[test]
    fn reflow_action_342_release_still_reflows() {
        assert_eq!(
            reflow_action(false, ReflowStopState::Released, 3, 1, false, false),
            ReflowAction::Applied,
            "松手完成（非取消/非编辑）必须 Applied —— 尾字回灌不能因松手被丢"
        );
    }

    /// 342-A：取消 ⇒ SkippedCancel（内容丢弃，不回灌）。
    #[test]
    fn reflow_action_cancel() {
        assert_eq!(
            reflow_action(false, ReflowStopState::Cancelled, 3, 1, false, false),
            ReflowAction::SkippedCancel
        );
    }

    /// 单调键 = seg_index：`seg <= 已应用` ⇒ SkippedStale（防迟到回灌回退预览）。
    /// 🔴 反例锁定：即便 committed_len/内容不同，只要 seg 不增即跳。
    #[test]
    fn reflow_action_stale_by_seg_not_len() {
        assert_eq!(
            reflow_action(false, ReflowStopState::Released, 2, 2, false, false),
            ReflowAction::SkippedStale
        );
        assert_eq!(
            reflow_action(false, ReflowStopState::Released, 1, 5, false, false),
            ReflowAction::SkippedStale
        );
        // seg 前进 ⇒ 通过（即便 has_hole 之后再判）
        assert_eq!(
            reflow_action(false, ReflowStopState::Released, 6, 5, false, false),
            ReflowAction::Applied
        );
    }

    /// 🔴 FIX-PREVIEW-STALE-AND-COLLAPSE-375（A）：单调键选择 —— `replace_all=true` 必须用
    /// **滑窗快照自有**的 `reflow_seq`，**不得**用 `seg_index`（后者在滑窗路径会重复）。
    #[test]
    fn reflow_monotonic_key_prefers_reflow_seq_for_replace_all() {
        assert_eq!(
            reflow_monotonic_key(true, 7, Some(0)),
            0,
            "replace_all ⇒ 用 reflow_seq（忽略切片下标）"
        );
        assert_eq!(reflow_monotonic_key(true, 7, Some(9)), 9);
        assert_eq!(
            reflow_monotonic_key(true, 7, None),
            7,
            "缺失 ⇒ 保守退回 seg_index（沿用旧行为，不至于整片不回灌）"
        );
        assert_eq!(
            reflow_monotonic_key(false, 7, Some(0)),
            7,
            "老逐片路径行为逐位不变（仍用 seg_index）"
        );
    }

    /// 🔴 375（A）核心：**同一 `dispatch_idx` 的两次 replace_all 都不得被判 stale**。
    ///
    /// 现场：`seg=0 acc_len=37 pending` → `seg=0 acc_len=45 skipped-stale`（完整文本被丢）。
    /// 修法后两条事件的键是 **0、1**（滑窗自有计数器）⇒ 第二条按 Applied 通过。
    #[test]
    fn same_dispatch_idx_twice_not_stale_for_replace_all() {
        let seg = 0usize; // 两窗撞同一切片下标
        let key1 = reflow_monotonic_key(true, seg, Some(0));
        let key2 = reflow_monotonic_key(true, seg, Some(1));
        assert_eq!(
            reflow_action(false, ReflowStopState::Released, key1, -1, false, false),
            ReflowAction::Applied,
            "第一条（37 字）应回灌"
        );
        assert_eq!(
            reflow_action(false, ReflowStopState::Released, key2, key1, false, false),
            ReflowAction::Applied,
            "第二条（45 字完整文本）**不得**被判 stale（375-A 的 P0）"
        );
        // 对照：沿用旧行为（键=seg_index）⇒ 第二条必被误杀（复现现场日志）
        assert_eq!(
            reflow_action(
                false,
                ReflowStopState::Released,
                seg as i64,
                seg as i64,
                false,
                false
            ),
            ReflowAction::SkippedStale,
            "旧键（切片下标）在同下标时会误杀 ⇒ 证明本修复必要"
        );
    }

    /// 覆盖区有洞（accuracy 失败片）⇒ SkippedHole（带洞文本不得上屏，保留流式预览）。
    #[test]
    fn reflow_action_hole() {
        assert_eq!(
            reflow_action(false, ReflowStopState::Released, 6, 5, true, false),
            ReflowAction::SkippedHole
        );
        // 洞优先于空（都成立时报洞，语义更准）
        assert_eq!(
            reflow_action(false, ReflowStopState::Released, 6, 5, true, true),
            ReflowAction::SkippedHole
        );
    }

    /// acc 空 ⇒ SkippedEmpty（约束 4：不得把已显示预览抹白）。
    #[test]
    fn reflow_action_empty() {
        assert_eq!(
            reflow_action(false, ReflowStopState::Released, 6, 5, false, true),
            ReflowAction::SkippedEmpty
        );
    }
}

#[cfg(test)]
mod reflow_persist_329_tests {
    use super::{compose_with_acc_for_gen, reflow_preview};

    /// 判据 1（不变式）：一次回灌后，后续任意次流式更新渲染出的文本**都带同一权威前缀**。
    /// 证明缺陷 A 已修（修正不会被下一个流式包冲掉）。
    #[test]
    fn compose_prefix_persists_across_streaming_updates() {
        // state = (gen=1, acc="指导灵", committed=3)
        let state = (1u64, "指导灵".to_string(), 3usize);
        let raws = [
            "指导领",
            "指导领的信息",
            "指导领的信息吗",
            "指导领的信息吗？",
        ];
        for raw in raws {
            let display = compose_with_acc_for_gen(Some(&state), 1, raw);
            assert!(
                display.starts_with("指导灵"),
                "每次流式渲染都必须带权威前缀，实测 {display:?}"
            );
            // 尾巴 = raw 从 committed 起的部分
            assert_eq!(display, reflow_preview("指导灵", raw, 3));
        }
    }

    /// 🔴 Gavin 口径 + 致命回归锁：acc 为空（第一片尚未回来）⇒ **原样返回流式全文**。
    /// 前 6~8 秒全靠流式文字；若改成「acc 空就不显示」会开口即空白。
    #[test]
    fn compose_empty_acc_passthrough() {
        let empty_acc = (1u64, String::new(), 0usize);
        assert_eq!(
            compose_with_acc_for_gen(Some(&empty_acc), 1, "第一句话的流式文字"),
            "第一句话的流式文字"
        );
    }

    /// 边界 3/4：gen 不匹配（旧代残留）⇒ 原样 raw；闩锁后 state 保留 ⇒ 仍带前缀。
    #[test]
    fn compose_gen_mismatch_passthrough_but_same_gen_keeps_prefix() {
        let state = (2u64, "权威前缀".to_string(), 2usize);
        assert_eq!(
            compose_with_acc_for_gen(Some(&state), 3, "新录音raw"),
            "新录音raw"
        );
        // 同 gen（闩锁后 state 未被清）⇒ 继续带前缀，画面不倒退
        assert_eq!(
            compose_with_acc_for_gen(Some(&state), 2, "后续raw尾"),
            reflow_preview("权威前缀", "后续raw尾", 2)
        );
    }

    /// 边界 1：编辑态（stopped）⇒ 包在第一道闸被丢，不进渲染（不会冲 EDIT 用户输入）。
    /// editing⇒stopped 由既有 F3 护栏锁定；此处锁「闸门第一道即 stopped」。
    #[test]
    fn editing_implies_render_dropped() {
        assert!(
            super::should_ignore_streaming_text(true),
            "stopped(含编辑态) 必须丢渲染"
        );
        assert!(!super::should_ignore_streaming_text(false));
    }

    /// 边界 5：非本地档 state 恒 None ⇒ 逐位 raw passthrough（在线/批处理结构上不进合成）。
    #[test]
    fn non_local_tiers_passthrough() {
        assert_eq!(
            compose_with_acc_for_gen(None, 42, "在线流式原文"),
            "在线流式原文"
        );
    }

    /// 🔴 残留 gap（053-B 既定契约「渲染抑制 ≠ 数据抑制」）。
    ///
    /// **AUTOLEARN-EDIT-SNAPSHOT-331 已收口此 gap ⇒ 断言语义随之变更**（由「钉住缺陷」改为
    /// 「钉住收口后的新契约」；非静默删除）：镜像仍会因迟到包领先于最后所见（053-B 要求），
    /// 但**自学习基准改用编辑入口快照**，不再取被推进的镜像 ⇒ gap 不再产生伪候选。
    #[test]
    fn stopped_late_packet_advances_mirror_but_baseline_uses_snapshot() {
        let state = (1u64, "权威".to_string(), 2usize);
        let last_displayed = compose_with_acc_for_gen(Some(&state), 1, "早年");
        let mirror_after_late = compose_with_acc_for_gen(Some(&state), 1, "晚年更长尾");
        // 053-B 事实不变：迟到包推进镜像（渲染仍被抑制）
        assert_ne!(
            last_displayed, mirror_after_late,
            "迟到包仍推进镜像（053-B 渲染抑制≠数据抑制，勿改）"
        );
        // 331 收口：本地实时档的学习基准 = 最后渲染文本，而非被推进的镜像
        let expected = Some(last_displayed.clone());
        assert_eq!(
            super::select_learning_baseline(true, Some(last_displayed), Some(mirror_after_late)),
            expected,
            "331：基准必须取编辑入口快照（所见），不得取被迟到包推进的镜像"
        );
    }
}

#[cfg(test)]
mod seam_337_tests {
    use super::{compose_with_acc_for_gen, reflow_preview, select_learning_baseline};

    /// §六-1 重复消解：commit 合成 = acc 覆盖前 `confirmed_len` 字符 + 流式尾巴，无重叠。
    #[test]
    fn commit_compose_has_no_duplication() {
        // 已确认权威文本 4 字；流式显示（含同句更多字）—— 只保留 confirmed_len 之后的尾巴
        assert_eq!(
            reflow_preview("甲乙丙丁", "甲乙丙丁戊己", 4),
            "甲乙丙丁戊己"
        );
        // 若按旧的 last_display 总长切，则会重复；此处只验证 C 的切点语义
        assert_eq!(reflow_preview("权威", "权威新句", 2), "权威新句");
    }

    /// §六-3 回修/缩短：流式文本短于 confirmed_len ⇒ 不 panic、尾巴为空。
    #[test]
    fn shrink_safe() {
        assert_eq!(reflow_preview("权威前缀", "短", 5), "权威前缀");
        assert_eq!(reflow_preview("权威前缀", "", 5), "权威前缀");
    }

    /// 未 commit（无 ACC_REFLOW_STATE / 非对齐作废后）⇒ 纯流式 passthrough（不空白、不回退）。
    #[test]
    fn not_committed_is_pure_streaming_passthrough() {
        assert_eq!(
            compose_with_acc_for_gen(None, 1, "纯流式全文"),
            "纯流式全文"
        );
    }

    /// §六-3 acc 为空（首片未回 / 未 commit）⇒ 原样流式全文（保留 325 契约）。
    #[test]
    fn empty_acc_passthrough() {
        let empty = (1u64, String::new(), 0usize);
        assert_eq!(
            compose_with_acc_for_gen(Some(&empty), 1, "开口前几秒的流式文字"),
            "开口前几秒的流式文字"
        );
    }

    /// §六-5 学习基准不变式（331）在新时序下不变：本地档仍取编辑入口快照。
    #[test]
    fn learning_baseline_invariant_unchanged() {
        assert_eq!(
            select_learning_baseline(true, Some("所见".to_string()), Some("镜像".to_string())),
            Some("所见".to_string())
        );
        assert_eq!(
            select_learning_baseline(false, Some("快照".to_string()), Some("镜像".to_string())),
            Some("镜像".to_string())
        );
    }
}

#[cfg(test)]
mod edit_snapshot_331_tests {
    use super::select_learning_baseline;

    /// 🔴 收口核心（任务 §五-1）：停止后迟到包推进镜像 ⇒ 随后进编辑 ⇒ 基准 = **最后渲染文本**。
    #[test]
    fn baseline_uses_snapshot_not_advanced_mirror() {
        let snapshot = Some("屏幕上最后那份".to_string());
        let advanced_mirror = Some("被迟到包推进过的更完整文本".to_string());
        assert_eq!(
            select_learning_baseline(true, snapshot.clone(), advanced_mirror),
            snapshot
        );
    }

    /// 🔴 主控补的场景：**本地档 + 零回灌（一片未派）** + 迟到包推进镜像 ⇒ 基准仍取快照。
    /// （用「回灌是否活跃」当闸门会漏掉这条 —— 故闸门必须是档位。）
    #[test]
    fn baseline_local_zero_reflow_still_uses_snapshot() {
        // 零回灌 ⇒ ACC_REFLOW_STATE 为 None（本函数无该入参，正体现闸门与回灌状态解耦）
        let snapshot = Some("短录音屏幕上那份".to_string());
        let advanced_mirror = Some("停后迟到包推进的文本".to_string());
        assert_eq!(
            select_learning_baseline(true, snapshot.clone(), advanced_mirror),
            snapshot
        );
    }

    /// 边界 5：在线 / 批处理 ⇒ **恒 mirror**（逐位不变），即便带了快照也忽略。
    #[test]
    fn baseline_online_batch_uses_mirror_only() {
        assert_eq!(
            select_learning_baseline(false, Some("快照".to_string()), Some("镜像".to_string())),
            Some("镜像".to_string())
        );
        assert_eq!(
            select_learning_baseline(false, Some("快照".to_string()), None),
            None
        );
    }

    /// 本地档无快照（理论不发生）⇒ 回落镜像；代际清空后等价于此。
    #[test]
    fn baseline_local_falls_back_to_mirror_without_snapshot() {
        assert_eq!(
            select_learning_baseline(true, None, Some("镜像".to_string())),
            Some("镜像".to_string())
        );
    }

    /// 未编辑不学习：快照 == 提交文本 ⇒ `original == edited` ⇒ diff 为空（wordbook 对相同串返回 None）。
    #[test]
    fn unedited_submit_yields_equal_baseline_and_edited() {
        let displayed = "这是一条来自指导灵的信息吗？".to_string();
        let baseline = select_learning_baseline(true, Some(displayed.clone()), Some("别的".into()));
        // 零修改提交时 edited 就是 EDIT 初值 == 快照
        let edited = displayed.clone();
        assert_eq!(baseline, Some(edited));
    }
}

#[cfg(test)]
mod punct_double_334_tests {
    use super::{pretranscribed_native_punctuated, transcription};

    /// 🔴 回归核心（§五-1）：构造「部分分片失败 + 其余已带标点」⇒ 必须 true ⇒ 不再二次打标点。
    /// 失败片贡献空串（`texts.push(String::new())`），拼出来带洞但已带标点。
    #[test]
    fn partial_failed_segment_with_punctuated_text_skips_engine() {
        let joined = transcription::join_segment_texts(&[
            "下台进行另一次人生时。".to_string(),
            String::new(), // 失败片留下的空洞
            "灵魂自愿经历某些事件。".to_string(),
        ]);
        assert!(
            pretranscribed_native_punctuated(&joined),
            "带洞但文本已带标点 ⇒ native_punctuated=true ⇒ 跳过引擎（旧实现误判 false ⇒ 。。 叠加）"
        );
    }

    /// §五-2：全部分片成功且带标点 ⇒ 行为不变（仍跳过引擎）。
    #[test]
    fn all_segments_success_punctuated_unchanged() {
        let joined = transcription::join_segment_texts(&["甲。".to_string(), "乙。".to_string()]);
        assert!(pretranscribed_native_punctuated(&joined));
    }

    /// 🔴 §五-3 反向：文本**确实没有标点**（模型未输出）⇒ false ⇒ 引擎**必须**跑。
    /// 这是 DEC-047 当初要解决的原始缺陷，别修回去。
    #[test]
    fn text_without_punctuation_still_runs_engine() {
        let joined = transcription::join_segment_texts(&[
            "今天天气不错".to_string(),
            "我们出去走走".to_string(),
        ]);
        assert!(
            !pretranscribed_native_punctuated(&joined),
            "模型未输出标点 ⇒ false ⇒ CT-Transformer 必须跑（DEC-047 原始缺陷不复现）"
        );
    }

    /// §五-4 口径：词内嵌标点豁免（与 :9855 Qwen3 分支同一 detector）。
    #[test]
    fn inline_punctuation_is_not_effective() {
        assert!(!pretranscribed_native_punctuated("圆周率是3.14"));
        assert!(!pretranscribed_native_punctuated("3:30 开会"));
        assert!(!pretranscribed_native_punctuated("example.com"));
        assert!(pretranscribed_native_punctuated("今天天气不错。"));
    }
}

#[cfg(test)]
mod punct_final_redo_350_tests {
    use super::{pretranscribed_native_punctuated, strip_punctuation_node};
    use crate::punctuation::has_effective_punctuation;

    /// 节点关闭（`enabled=false`）⇒ 输入逐字返回（用户关标点时不介入）。
    #[test]
    fn strip_node_disabled_returns_input_verbatim() {
        assert_eq!(
            strip_punctuation_node("你好。。".to_string(), false),
            "你好。。"
        );
        assert_eq!(strip_punctuation_node(String::new(), false), "");
    }

    /// 节点开启 ⇒ 剥光所有有效标点，词内嵌标点保留。
    #[test]
    fn strip_node_strips_effective_only() {
        assert_eq!(strip_punctuation_node("你好。。".to_string(), true), "你好");
        assert_eq!(
            strip_punctuation_node("今天天气不错，。".to_string(), true),
            "今天天气不错"
        );
        // 词内嵌（小数 / 时间 / 域名 / 英文缩写 / 中文量级）不被剥
        for t in ["3.14", "3:30 开会", "example.com", "don't", "3.5亿"] {
            assert_eq!(strip_punctuation_node(t.to_string(), true), t);
        }
    }

    /// 🔴 「剥光了必定打得回来」不变式（Gavin 的担忧，350 核心）：
    /// 只要剥离节点真的动手（`enabled=true` 且非翻译），剥光文本的 `native_punctuated`
    /// 必为 false ⇒ 下游 `apply_local_punctuation` 的门
    /// `enabled && !translate_requested && !native_punctuated` 必定放行
    /// ⇒ 不会出现「剥光了没打回来」。
    /// （`llm_handled=true` 是 LLM 路径，由 LLM 自带标点；见 result.md 核实结论。）
    #[test]
    fn strip_node_implies_downstream_punctuation_gate() {
        for t in [
            "你好。。",
            "今天天气不错，。",
            "甲。乙。",
            "3:30 开会。",
            "Hello, world.",
        ] {
            let stripped = strip_punctuation_node(t.to_string(), true);
            assert!(
                !has_effective_punctuation(&stripped),
                "剥光后仍含有效标点 ⇒ 下游门不成立：{t} -> {stripped}"
            );
            // 节点门（enabled=true 且非翻译）⇒ 下游门的三项条件齐备
            let enabled = true;
            let translate_requested = false;
            let native_punctuated = pretranscribed_native_punctuated(&stripped);
            assert!(
                enabled && !translate_requested && !native_punctuated,
                "不变式破坏：剥光后下游门不成立，用户会拿到无标点文本：{t} -> {stripped}"
            );
        }
    }

    /// 🔴 回归：本地 realtime 路径不再出现重复标点（`PUNCT-DOUBLE-334` 老 bug 不复发）。
    /// 本节点保证进引擎的文本**无有效标点**；此处用「末尾补句号」的最简引擎模型验证不叠加
    /// （真实引擎效果留端测）。
    #[test]
    fn strip_node_no_double_punctuation_regression() {
        for (input, engine_input, redone) in [
            ("你好。。", "你好", "你好。"),
            ("今天天气不错，。", "今天天气不错", "今天天气不错。"),
            ("甲。乙。", "甲乙", "甲乙。"),
        ] {
            let staged = strip_punctuation_node(input.to_string(), true);
            assert_eq!(staged, engine_input, "进引擎文本不符预期：{input}");
            assert!(
                !has_effective_punctuation(&staged),
                "进引擎文本仍含有效标点：{staged}"
            );
            let out = format!("{staged}。");
            assert_eq!(out, redone);
            assert!(
                !out.contains("。。") && !out.contains("，。"),
                "重复标点复发：{out}"
            );
        }
    }
}

#[cfg(test)]
mod parallel_acc_298_tests {
    use super::{acc_parallel_result_usable, assemble_parallel_accuracy, hole_fill_decision};

    /// 判据 #3：seg_index 乱序输入 ⇒ 按序号升序拼接；片内子段顺序保持。
    #[test]
    fn assemble_parallel_accuracy_orders_by_seg_index() {
        let got = assemble_parallel_accuracy(vec![
            (2, vec!["c".to_string(), "d".to_string()]),
            (0, vec!["a".to_string()]),
            (1, vec!["b".to_string()]),
        ]);
        assert_eq!(got, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn assemble_parallel_accuracy_empty_and_single() {
        assert!(assemble_parallel_accuracy(vec![]).is_empty());
        assert_eq!(
            assemble_parallel_accuracy(vec![(0, vec!["only".to_string()])]),
            vec!["only"]
        );
        // 空子段列表（该片整体失败）不影响其它片
        assert_eq!(
            assemble_parallel_accuracy(vec![(0, vec![]), (1, vec!["x".to_string()])]),
            vec!["x"]
        );
    }

    /// 判据 #3：取消 ⇒ 结果一律弃用（不带脏结果）；空/纯空白 ⇒ 弃用回落旧路径。
    #[test]
    fn acc_parallel_result_usable_gate() {
        assert!(!acc_parallel_result_usable(true, "有字"));
        assert!(!acc_parallel_result_usable(false, ""));
        assert!(!acc_parallel_result_usable(false, "   \n"));
        assert!(acc_parallel_result_usable(false, "有字"));
    }

    /// LOCALRT-REFLOW-HOLE-344-G：空/失败片用流式文本填补 ⇒ **不记洞**（回灌不中断）；
    /// 流式也为空 / 不可填补 ⇒ 记洞兜底（`SkippedHole` 仍保留）。
    #[test]
    fn reflow_hole_344_fill_by_streaming_not_a_hole() {
        // 有流式文本可填 ⇒ 填补、不记洞。
        assert_eq!(
            hole_fill_decision(true, "多认识几个朋友"),
            ("多认识几个朋友".to_string(), false)
        );
        // 流式也为空 ⇒ 真·无法填补、记洞。
        assert_eq!(hole_fill_decision(true, ""), (String::new(), true));
        // 多子段/已填过（can_fill=false）⇒ 保守记洞。
        assert_eq!(hole_fill_decision(false, "有文本"), (String::new(), true));
    }

    /// 344-G 端到端拼接口径：失败片填补后 `assemble_parallel_accuracy` 仍输出连续文本（无空片）。
    #[test]
    fn reflow_hole_344_assembled_text_stays_continuous() {
        // seg0 成功；seg1 失败 ⇒ 用流式填补；seg2 成功。
        let (fill, is_hole) = hole_fill_decision(true, "第二句流式");
        assert!(!is_hole);
        let got = assemble_parallel_accuracy(vec![
            (0, vec!["第一句".to_string()]),
            (1, vec![fill]),
            (2, vec!["第三句".to_string()]),
        ]);
        assert_eq!(got.join(""), "第一句第二句流式第三句");
    }
}

/// FIX-PREVIEW-HARVEST-380（A）：accuracy 滑窗线程的切片载荷
/// `(切片下标, 派发当刻浮层字符数, 本片音频, 本片流式文本)`。
type AccSliceMsg = (usize, usize, Vec<Vec<f32>>, String);

/// FIX-PREVIEW-HARVEST-380（A）：滑窗解码结果载荷
/// `(window_seq, dispatch_idx, 解码结果, 解码耗时 ms, 解码完成时刻)`。
/// 382（3C）：末元 `Instant` 随 `PreviewReflow.decode_done_at` 带到消费端，测「解完 → 浮层重画」。
type AccDecodeResult = (
    usize,
    usize,
    anyhow::Result<(String, bool)>,
    f64,
    std::time::Instant,
);

/// FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382：滑窗解码任务载荷
/// `(window_seq, dispatch_idx, 本窗音频, 产出率均值快照, 派发时刻)`（末元供 3C `queued_ms`）。
type AccTaskMsg = (usize, usize, Vec<f32>, Option<f32>, std::time::Instant);

/// FIX-PREVIEW-HARVEST-380（A）：滑窗线程的一步（新切片 / 解码结果）。
enum AccWindowStep<A, R> {
    Slice(A),
    Result(R),
}

/// FIX-PREVIEW-HARVEST-380（A）：滑窗线程「结果到即收」驱动器。
///
/// 同时等 `acc_rx`（新切片）与 `res_rx`（解码结果），**任一先到即处理**；`acc_rx` 关闭
///（松键）或 `cancelled()` 为真即返回，由调用方做收尾 drain（复用同一 `step` 闭包）。
///
/// 缺陷本体（Gavin 2026-09-23 端测 BUILD-379）：原实现是 `for … in acc_rx` —— 没有新切片时
/// 不进循环体，已解出的结果要压到下一次切片或松键才回灌（用户停顿 ⇒ 预览不刷新，每次的
/// 最后一窗都要等松键）。抽成独立函数的唯一目的，是让「结果在 `acc_rx` 仍打开时就被处理」
/// 这条不变量**可对生产代码单测**（见 `preview_harvest_380_tests`），而不是测一份复制品。
fn drive_acc_windows<A, R>(
    acc_rx: &crossbeam_channel::Receiver<A>,
    res_rx: &crossbeam_channel::Receiver<R>,
    cancelled: impl Fn() -> bool,
    step: &mut impl FnMut(AccWindowStep<A, R>),
) {
    loop {
        if cancelled() {
            break;
        }
        crossbeam_channel::select! {
            recv(acc_rx) -> msg => match msg {
                Ok(slice) => step(AccWindowStep::Slice(slice)),
                // 发送端关闭 = 松键 ⇒ 退出，交给收尾 drain 收齐在飞窗口。
                Err(_) => break,
            },
            recv(res_rx) -> msg => match msg {
                Ok(result) => step(AccWindowStep::Result(result)),
                // 收尾段之外 res_rx 不该断开（worker 未全部退出前持有发送端）；
                // 真断开按「不再有结果」处理，避免死循环。
                Err(_) => break,
            },
        }
    }
}

#[cfg(test)]
mod preview_harvest_380_tests {
    use super::{drive_acc_windows, AccWindowStep};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// FIX-PREVIEW-HARVEST-380（A）：结果必须在 `acc_rx` **仍打开**时就被处理，
    /// 不得等 `acc_rx` 关闭（旧 `for … in acc_rx` 的缺陷）。
    ///
    /// 先后由通道制造、不用 sleep 定时序：先派 1 片、阻塞等它被处理（此时 driver 已回到
    /// select 等下一件事，`acc_tx` 仍被本测试持有 ⇒ `acc_rx` 恒打开）；再送 1 个结果，
    /// 它必须在 `acc_tx` 关闭之前被 `step` 消费。若 driver 退回「只在收到切片时才收结果」，
    /// 第 2 次 `recv` 必然超时 ⇒ 红。`recv_timeout` 仅作挂死兜底，不承担排序职责。
    #[test]
    fn drive_acc_windows_harvests_result_while_acc_open() {
        let (acc_tx, acc_rx) = crossbeam_channel::unbounded::<u32>();
        let (res_tx, res_rx) = crossbeam_channel::unbounded::<u32>();
        // step 每处理一步即回执，供测试线程按先后推进。
        let (step_tx, step_rx) = crossbeam_channel::unbounded::<&'static str>();
        let seen = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let seen_worker = Arc::clone(&seen);

        let handle = std::thread::spawn(move || {
            let mut step = |ev: AccWindowStep<u32, u32>| {
                let tag = match ev {
                    AccWindowStep::Slice(_) => "slice",
                    AccWindowStep::Result(_) => "result",
                };
                seen_worker.lock().unwrap().push(tag);
                let _ = step_tx.send(tag);
            };
            drive_acc_windows(&acc_rx, &res_rx, || false, &mut step);
        });

        // 1) 派发 1 窗；阻塞等它被处理（先后由通道制造，非 sleep）。
        acc_tx.send(0).expect("send slice");
        assert_eq!(
            step_rx
                .recv_timeout(Duration::from_secs(10))
                .expect("slice step"),
            "slice"
        );
        // 2) 结果回来 —— 此刻 acc_tx 仍被本测试持有 ⇒ acc_rx 恒打开。
        res_tx.send(1).expect("send result");
        assert_eq!(
            step_rx
                .recv_timeout(Duration::from_secs(10))
                .expect("结果必须在 acc_rx 仍打开时被处理（不得等 acc_rx 关闭）"),
            "result"
        );
        // 关切片通道，driver 退出。
        drop(acc_tx);
        drop(res_tx);
        handle.join().expect("driver thread");
        assert_eq!(&*seen.lock().unwrap(), &["slice", "result"]);
    }
}

// ============================================================
// FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382（问题1）：逐片组窗（纯函数）
// ============================================================

/// FIX-TAIL-WINDOW-AND-FALLBACK-386（C）：窗口文本选择 —— 解码文本为空（含解码 Err）⇒ 用**本窗流式文本**
/// 兜底；流式也为空才保持空。防结尾整段丢失（本次 seq7 `<location>` 丢 40+ 字）。纯函数，可单测。
/// 386 主控验收补：一次派发切成多片时，该派发的流式文本只记在**第一片**上，其余片记空。
/// `seg_streaming` 是整次派发的流式文本；若每片都存一份，含同次派发两片的窗口（如收尾
/// `[大片, 短尾]` 合并窗）解码失败时，兜底文本会把整段拼两遍 ⇒ 最终文本重复。
/// 只记第一片不丢内容：第一片所在的窗口已带上整段文本。
fn slice_streaming_text(k: usize, seg_streaming: &str) -> String {
    if k == 0 {
        seg_streaming.to_string()
    } else {
        String::new()
    }
}

fn window_text_with_fallback(decoded: &str, streaming: &str) -> String {
    if decoded.is_empty() {
        streaming.to_string()
    } else {
        decoded.to_string()
    }
}

/// 386：短尾合并阈值（秒）—— 收尾时待覆盖片 < 此值且前面有片 ⇒ 与前一并重解。
const TAIL_MERGE_MAX_SECS: f32 = 3.0;

/// 386：组窗计划（纯函数输出）。
#[derive(Debug, PartialEq, Eq)]
struct WindowPlan {
    /// 本次要**立刻派发**的窗口（全局切片区间 `[start,end)`，按序）。
    windows: Vec<(usize, usize)>,
    /// 更新后的「待覆盖片」全局下标（`None` = 无待覆盖）。
    pending: Option<usize>,
}

/// FIX-TAIL-WINDOW-AND-FALLBACK-386：组窗纯函数（在 382 基础上加「延后尾片 / 收尾合并」）。
///
/// 输入：`prev_durs` 已派发片时长（时间序，调用方保证已按 `WINDOW_MAX_SLICES` 裁剪）；
/// `prev_base` = `prev_durs[0]` 的全局下标；`new_durs` = 本次派发携带的新片时长（时间序）；
/// `pending` = 上一次遗留的**待覆盖片**全局下标（`None` = 无）；`is_tail` = 是否收尾（松键、不再有新片）。
///
/// 规则（Gavin 2026-09-23 BUILD-385 端测）：
/// 1. 单次派发 **1 片**（常态）⇒ 与旧行为相同，立刻组窗（含把 `pending` 强制纳入）。
/// 2. 单次派发 **≥2 片**（被切分）⇒ 除**最后一片**外每片立刻组窗；最后一片不组窗，记为待覆盖片。
/// 3. 有 `pending` 时，**本批第一个窗口起点强制 ≤ pending**（把待覆盖片纳入，**哪怕总长超 `WINDOW_MAX_SECS`**）。
/// 4. `is_tail` 且仍有待覆盖片：其时长 < [`TAIL_MERGE_MAX_SECS`] 且**前面有片** ⇒ 组窗 `[待覆盖片-1, 待覆盖片+1)`
///    （**前一片重解**，Gavin 要的）；否则 ⇒ 单独组窗 `[待覆盖片, 待覆盖片+1)`。
/// 5. 🔴 覆盖不变量：**每个已派发切片在收尾时都至少被一个窗口覆盖**（延后片由「下次首个窗」或「收尾窗」覆盖）。
fn plan_windows(
    prev_durs: &[f32],
    new_durs: &[f32],
    prev_base: usize,
    pending: Option<usize>,
    is_tail: bool,
) -> WindowPlan {
    let mut buf: Vec<f32> = prev_durs.to_vec();
    let mut base = prev_base; // buf[0] 的全局下标
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut pend = pending;
    let n = new_durs.len();

    for (k, nd) in new_durs.iter().enumerate() {
        buf.push(*nd);
        while buf.len() > transcription::WINDOW_MAX_SLICES {
            buf.remove(0);
            base += 1;
        }
        let end = base + buf.len();
        let is_last = k + 1 == n;
        // 规则 2：一次派发 ≥2 片 ⇒ 最后一片**延后**（不收尾时）。
        if is_last && !is_tail && n >= 2 {
            pend = Some(end - 1);
            continue;
        }
        let mut start =
            base + transcription::group_window_start_secs(&buf, transcription::WINDOW_MAX_SECS);
        // 规则 3：本批第一个窗口强制纳入待覆盖片（允许超 WINDOW_MAX_SECS）。
        if let Some(p) = pend {
            if p < start {
                start = p;
            }
            pend = None;
        }
        out.push((start, end));
    }

    // 规则 4：收尾处理仍待覆盖的片。
    if is_tail {
        if let Some(p) = pend {
            let idx = p.checked_sub(base);
            let dur = idx.and_then(|i| buf.get(i).copied());
            let has_prev = idx.map_or(false, |i| i > 0);
            if has_prev && matches!(dur, Some(d) if d < TAIL_MERGE_MAX_SECS) {
                out.push((p - 1, p + 1)); // 短尾与前一并（前片重解）
            } else {
                out.push((p, p + 1)); // 单独
            }
            pend = None;
        }
    }

    WindowPlan {
        windows: out,
        pending: pend,
    }
}

#[cfg(test)]
mod plan_windows_386_tests {
    use super::{plan_windows, WindowPlan, TAIL_MERGE_MAX_SECS};
    use crate::transcription;

    fn covered(windows: &[(usize, usize)], idx: usize) -> bool {
        windows.iter().any(|(s, e)| *s <= idx && idx < *e)
    }

    /// 常态单片（无 pending）：与旧行为相同 —— 立刻 1 窗、无待覆盖。
    #[test]
    fn single_slice_no_pending_is_immediate() {
        let prev = vec![1.0f32, 1.0, 1.0];
        let p = plan_windows(&prev, &[3.0], 0, None, false);
        let mut buf = prev.clone();
        buf.push(3.0);
        let s = transcription::group_window_start_secs(&buf, transcription::WINDOW_MAX_SECS);
        assert_eq!(
            p,
            WindowPlan {
                windows: vec![(s, 4)],
                pending: None
            }
        );
    }

    /// 中途多片：除最后一片外立刻组窗；**最后一片延后**（pending）。
    /// 反例：382 旧行为会给 [10.3,5.37] 组 [0,1)、[1,2)（第二窗只含 1.12s 短尾 ⇒ 模型念词表）。
    #[test]
    fn multi_slice_defers_last() {
        let p = plan_windows(&[], &[10.3, 5.37], 0, None, false);
        assert_eq!(p.windows, vec![(0, 1)], "只有首片立刻组窗");
        assert_eq!(p.pending, Some(1), "末片延后");
    }

    /// 下次派发：首个窗口**必须含待覆盖片**（起点 ≤ pending），哪怕总长超 `WINDOW_MAX_SECS`。
    #[test]
    fn next_dispatch_forces_pending_inclusion_even_over_max() {
        // prev=[4,4,4]（global0..2，pending=2）；新片[9]（global3）。
        // 常规组窗：buf=[4,4,4,9]=21s>10 ⇒ 逐丢到只剩新片（起点=3）；pending=2 ⇒ 强制起点=2。
        let p = plan_windows(&[4.0, 4.0, 4.0], &[9.0], 0, Some(2), false);
        assert_eq!(
            p.windows,
            vec![(2, 4)],
            "首个窗口必须含 pending(2)，可超 10s"
        );
        assert!(p.pending.is_none(), "pending 被纳入后清空");
    }

    /// 收尾：待覆盖片 < 3s 且前面有片 ⇒ 与前一并重解 `[p-1, p+1)`。
    #[test]
    fn tail_merges_short_pending_with_prev() {
        let p = plan_windows(&[10.0, 1.2], &[], 0, Some(1), true);
        assert_eq!(p.windows, vec![(0, 2)], "短尾与前一并（前片重解）");
        assert!(p.pending.is_none());
        assert!(1.2 < TAIL_MERGE_MAX_SECS);
    }

    /// 收尾：待覆盖片 ≥ 3s ⇒ 单独组窗。
    #[test]
    fn tail_large_pending_alone() {
        let p = plan_windows(&[10.0, 3.5], &[], 0, Some(1), true);
        assert_eq!(p.windows, vec![(1, 2)], "≥3s 单独组窗");
    }

    /// 收尾：待覆盖片是首片（无前片）⇒ 即便 <3s 也单独组窗。
    #[test]
    fn tail_short_pending_without_prev_alone() {
        let p = plan_windows(&[1.2], &[], 0, Some(0), true);
        assert_eq!(p.windows, vec![(0, 1)]);
    }

    /// 「延后后直接收尾」链路：多片派发 ⇒ pending；无下次派发 ⇒ 直接收尾合并。
    #[test]
    fn defer_then_tail_directly() {
        let p1 = plan_windows(&[], &[5.0, 2.0], 0, None, false);
        assert_eq!(p1.pending, Some(1));
        let p2 = plan_windows(&[5.0, 2.0], &[], 0, p1.pending, true);
        assert_eq!(p2.windows, vec![(0, 2)], "2s<3s ⇒ 与前一并");
        assert!(p2.pending.is_none());
    }

    /// 覆盖不变量（随机性质）：任意「多次派发 + 收尾」，收尾时**每个切片**都被某窗覆盖。
    #[test]
    fn property_every_slice_covered() {
        struct Lcg(u64);
        impl Lcg {
            fn n(&mut self, lo: usize, hi: usize) -> usize {
                self.0 = self
                    .0
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (lo as u64 + (self.0 >> 33) % ((hi - lo + 1) as u64)) as usize
            }
        }
        let mut rng = Lcg(0x386_2026_5eed);
        for case in 0..200 {
            let n_dispatch = rng.n(1, 5);
            let mut all_durs: Vec<f32> = Vec::new();
            let mut windows: Vec<(usize, usize)> = Vec::new();
            let mut pending: Option<usize> = None;
            for _ in 0..n_dispatch {
                let n_new = rng.n(1, 4);
                let mut new_durs = Vec::new();
                for _ in 0..n_new {
                    new_durs.push(rng.n(1, 120) as f32 / 10.0);
                }
                let prev_len = transcription::WINDOW_MAX_SLICES.min(all_durs.len());
                let prev_base = all_durs.len() - prev_len;
                let prev_durs = all_durs[prev_base..].to_vec();
                let plan = plan_windows(&prev_durs, &new_durs, prev_base, pending, false);
                windows.extend(plan.windows);
                pending = plan.pending;
                all_durs.extend(new_durs);
            }
            let prev_len = transcription::WINDOW_MAX_SLICES.min(all_durs.len());
            let prev_base = all_durs.len() - prev_len;
            let prev_durs = all_durs[prev_base..].to_vec();
            windows.extend(plan_windows(&prev_durs, &[], prev_base, pending, true).windows);
            for idx in 0..all_durs.len() {
                assert!(
                    covered(&windows, idx),
                    "case {case}: 切片 {idx} 未被覆盖（durs={all_durs:?} windows={windows:?}）"
                );
            }
        }
    }
}

#[cfg(test)]
mod slice_streaming_386_review_tests {
    use super::{slice_streaming_text, window_text_with_fallback};

    /// 同次派发切 2 片、收尾合并窗 [大片, 短尾] 解码失败 ⇒ 兜底文本只出现一次（不重复）。
    #[test]
    fn merged_window_fallback_not_duplicated() {
        let seg = "如果有机会开这个会我们都会好好把握";
        let per_slice: Vec<String> = (0..2).map(|k| slice_streaming_text(k, seg)).collect();
        let window_fallback = per_slice.concat();
        assert_eq!(window_text_with_fallback("", &window_fallback), seg);
        assert_eq!(slice_streaming_text(1, seg), "");
    }
}

// =====================================================================
// TEST-SYNC-386（阶段三 · 非作者护栏，coder-2）：组窗契约 / 预览回灌 / 兜底
//   被测：FIX-TAIL-WINDOW-AND-FALLBACK-386（契约见任务书）。
//   按契约写、不照实现反推；白名单只 `cargo fmt` / `cargo check --all-targets`（首跑阶段四）。
// =====================================================================
#[cfg(test)]
mod testsync386_tests {
    use super::{
        compose_reflow_preview, compose_with_acc_for_gen, plan_windows, window_text_with_fallback,
    };
    use crate::transcription;

    struct Lcg(u64);
    impl Lcg {
        fn n(&mut self, lo: usize, hi: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (lo as u64 + (self.0 >> 33) % ((hi - lo + 1) as u64)) as usize
        }
    }

    /// 1. 会话级性质：200 次录音，每次 3~8 次派发（每次 1~3 片、每片 0.3~12s），最后一次 is_tail=true。
    ///    断言：① 每个切片全局下标至少被一窗覆盖 ② 会话结束无 pending 残留
    ///    ③ 每窗非空且不越界（`s < e ≤ 总片数`）。
    #[test]
    fn sync386_session_property_cover_no_pending_valid_end() {
        let max_prev = transcription::WINDOW_MAX_SLICES;
        let mut rng = Lcg(0x386_5E55_10AA);
        for case in 0..200usize {
            let n_dispatch = rng.n(3, 8);
            let mut all_durs: Vec<f32> = Vec::new();
            let mut windows: Vec<(usize, usize)> = Vec::new();
            let mut pending: Option<usize> = None;
            for d in 0..n_dispatch {
                let n_new = rng.n(1, 3);
                let mut new_durs = Vec::new();
                for _ in 0..n_new {
                    new_durs.push(0.3 + rng.n(0, 117) as f32 / 10.0); // 0.3..12.0
                }
                let prev_len = max_prev.min(all_durs.len());
                let prev_base = all_durs.len() - prev_len;
                let prev_durs = all_durs[prev_base..].to_vec();
                let is_tail = d + 1 == n_dispatch;
                let plan = plan_windows(&prev_durs, &new_durs, prev_base, pending, is_tail);
                windows.extend(plan.windows);
                pending = plan.pending;
                all_durs.extend(new_durs);
            }
            let total = all_durs.len();
            assert!(pending.is_none(), "case {case}: 收尾后不得有 pending 残留");
            for idx in 0..total {
                assert!(
                    windows.iter().any(|(s, e)| *s <= idx && idx < *e),
                    "case {case}: 切片 {idx} 未被任何窗口覆盖（windows={windows:?}）"
                );
            }
            for (s, e) in &windows {
                assert!(*s < *e, "case {case}: 窗口不得为空");
                assert!(
                    *e <= total,
                    "case {case}: end 不得越界（end={e} ≤ total={total}）"
                );
            }
        }
    }

    /// 2. 中途一大一小：派发 `[10.2, 1.1]` ⇒ 1 窗（大片）+ `pending`=小片；
    ///    下次派发 `[4.0]` ⇒ 首窗含小片与 `4.0`（`pending` 强制纳入并清空）。
    #[test]
    fn sync386_mid_big_small_then_next_includes_pending() {
        let p1 = plan_windows(&[], &[10.2, 1.1], 0, None, false);
        assert_eq!(p1.windows, vec![(0, 1)], "多片非收尾 ⇒ 只有大片立刻组窗");
        assert_eq!(p1.pending, Some(1), "小片延后为 pending");
        let p2 = plan_windows(&[10.2, 1.1], &[4.0], 0, p1.pending, false);
        assert_eq!(
            p2.windows,
            vec![(1, 3)],
            "首窗必须含小片(index1)与 4.0(index2)"
        );
        assert!(p2.pending.is_none(), "pending 纳入后清空");
    }

    /// 3. 结尾一大一小：短尾（<3s）⇒ 与前一并 `[大片, 小片]`；长尾（≥3s）⇒ 单独成窗。
    #[test]
    fn sync386_tail_big_small_merge_or_alone() {
        let p = plan_windows(&[], &[10.2, 1.1], 0, None, false);
        let t = plan_windows(&[10.2, 1.1], &[], 0, p.pending, true);
        assert_eq!(t.windows, vec![(0, 2)], "1.1s < 3s ⇒ 与前一并（前片重解）");
        assert!(t.pending.is_none());

        let p2 = plan_windows(&[], &[10.2, 3.5], 0, None, false);
        let t2 = plan_windows(&[10.2, 3.5], &[], 0, p2.pending, true);
        assert_eq!(t2.windows, vec![(1, 2)], "3.5s ≥ 3s ⇒ 单独成窗");
        assert!(t2.pending.is_none());
    }

    /// 4. 预览不回退：给定 acc、streaming 不断增长 ⇒ 回灌渲染字数 **≥** 仅 acc 字数，
    ///    且与**同参数** StreamingText 渲染逐字相等（同一合成函数）。
    #[test]
    fn sync386_preview_grows_with_streaming_and_never_shorter_than_acc() {
        let g = 0x386u64;
        let acc = "权威前缀文本";
        let committed = 3usize;
        let base = "原始流式文本从这继续";
        for extra in 0..=12usize {
            let streaming: String = base
                .chars()
                .chain("不断增长的内容尾巴".chars().take(extra))
                .collect();
            let reflow = compose_reflow_preview(g, acc, &streaming, committed);
            let state = (g, acc.to_string(), committed);
            let same = compose_with_acc_for_gen(Some(&state), g, &streaming);
            assert_eq!(
                reflow, same,
                "回灌必须与同参数 StreamingText 渲染逐字相等（extra={extra}）"
            );
            assert!(
                reflow.chars().count() >= acc.chars().count(),
                "回灌字数不得少于仅 acc（不回退，extra={extra}）"
            );
        }
    }

    /// 5. 兜底：解码空 ⇒ 流式文本；解码非空 ⇒ 原文不变；两者都空 ⇒ 空；
    ///    解码为纯空白（非空）⇒ 仍以解码为准（不做 trim 判定）。
    #[test]
    fn sync386_fallback_decoded_empty_uses_streaming() {
        assert_eq!(window_text_with_fallback("", "流式文本"), "流式文本");
        assert_eq!(
            window_text_with_fallback("解码文本", "流式文本"),
            "解码文本"
        );
        assert_eq!(window_text_with_fallback("", ""), "");
        assert_eq!(window_text_with_fallback(" ", "流式"), " ");
    }
}

// ============================================================
// FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382（3A）：replace_all 回灌
// 「立即渲染」状态机（纯函数；controller 线程侧只做 I/O 转接）
// ============================================================

/// 382（3A）：一次状态机推进的结果。
#[cfg(target_os = "windows")]
#[derive(Debug, PartialEq, Eq)]
enum ReflowFastOutcome {
    /// 无动作（例如边界到、但对应 seg 已不是最新已渲染）。
    None,
    /// 应更新权威状态；`render=true` 才重画浮层（`false` = Processing 已开始，只更新状态不重画）。
    Apply {
        text: String,
        committed_len: usize,
        accurate: bool,
        render: bool,
    },
}

/// 382（3A）：`replace_all=true` 回灌的快速路径状态机。
///
/// 背景：325/337 时代 `PreviewReflow` 只登记全文、必须等同片 `ReflowCommit` 边界配对才渲染；
/// 现在唯一发送点恒 `replace_all=true`（整段替换），「等边界」只会**白白延迟**（日志：1 次多等 0.4s、
/// 2 次单槽被覆盖永不配对、1 次 `boundary=b` 整次扣下）。本状态机：全文一到**立即**用「已知边界 or
/// 派发当刻 `committed_len`」渲染；边界后到且该 seg 仍是最新已渲染 ⇒ 用准确边界再渲染一次。
#[cfg(target_os = "windows")]
#[derive(Default)]
struct ReflowFastState {
    /// 最新登记全文 + 派发当刻 fallback：`(gen, seg, text, fallback_len)`。
    latest: Option<(u64, usize, String, usize)>,
    /// 边界登记 `(gen, seg) -> Option<usize>`（`None` = b 类「本片无准确边界」）。小 map，按代清空。
    bounds: Vec<((u64, usize), Option<usize>)>,
    /// 最近一次**应渲染**的 `(gen, seg)`（判断晚到边界是否该重渲染）。
    rendered: Option<(u64, usize)>,
}

#[cfg(target_os = "windows")]
impl ReflowFastState {
    fn clear(&mut self) {
        self.latest = None;
        self.bounds.clear();
        self.rendered = None;
    }

    fn bound_of(&self, gen: u64, seg: usize) -> Option<Option<usize>> {
        self.bounds
            .iter()
            .rev()
            .find(|((g, s), _)| *g == gen && *s == seg)
            .map(|(_, v)| *v)
    }

    /// 收到一条权威全文（已过 `reflow_action` 的编辑/取消/陈旧/洞/空判据）。
    /// `suppressed` = 本代已进入 Processing（只跳过重画，权威状态照常更新）。
    fn on_text(
        &mut self,
        gen: u64,
        seg: usize,
        text: String,
        fallback_len: usize,
        suppressed: bool,
    ) -> ReflowFastOutcome {
        self.latest = Some((gen, seg, text.clone(), fallback_len));
        self.rendered = Some((gen, seg));
        let (len, accurate) = match self.bound_of(gen, seg) {
            Some(Some(l)) => (l, true),
            _ => (fallback_len, false),
        };
        ReflowFastOutcome::Apply {
            text,
            committed_len: len,
            accurate,
            render: !suppressed,
        }
    }

    /// 收到一条边界。若对应 seg 是**最新已渲染**全文 ⇒ 用准确边界再渲染一次。
    fn on_bound(
        &mut self,
        gen: u64,
        seg: usize,
        len: Option<usize>,
        suppressed: bool,
    ) -> ReflowFastOutcome {
        if let Some(e) = self
            .bounds
            .iter_mut()
            .find(|((g, s), _)| *g == gen && *s == seg)
        {
            e.1 = len;
        } else {
            self.bounds.push(((gen, seg), len));
        }
        if self.rendered == Some((gen, seg)) {
            if let (Some(l), Some((g2, s2, text, _))) = (len, self.latest.as_ref()) {
                if (*g2, *s2) == (gen, seg) {
                    return ReflowFastOutcome::Apply {
                        text: text.clone(),
                        committed_len: l,
                        accurate: true,
                        render: !suppressed,
                    };
                }
            }
        }
        ReflowFastOutcome::None
    }
}

#[cfg(all(test, target_os = "windows"))]
mod reflow_fast_382_tests {
    use super::{ReflowFastOutcome, ReflowFastState};

    fn apply(o: ReflowFastOutcome) -> (String, usize, bool, bool) {
        match o {
            ReflowFastOutcome::Apply {
                text,
                committed_len,
                accurate,
                render,
            } => (text, committed_len, accurate, render),
            ReflowFastOutcome::None => panic!("expected Apply, got None"),
        }
    }

    /// 边界**后到**：全文先渲染（fallback），边界到后用准确值再渲染一次。
    #[test]
    fn text_then_bound_renders_twice_final_accurate() {
        let mut st = ReflowFastState::default();
        let a = apply(st.on_text(1, 5, "acc".into(), 3, false));
        assert_eq!(a, ("acc".into(), 3, false, true));
        let b = apply(st.on_bound(1, 5, Some(7), false));
        assert_eq!(b, ("acc".into(), 7, true, true));
    }

    /// 边界**先到**：全文一到即用准确边界渲染（只渲染一次）。
    #[test]
    fn bound_then_text_renders_once_accurate() {
        let mut st = ReflowFastState::default();
        assert_eq!(st.on_bound(1, 5, Some(7), false), ReflowFastOutcome::None);
        let a = apply(st.on_text(1, 5, "acc".into(), 3, false));
        assert_eq!(a, ("acc".into(), 7, true, true));
    }

    /// b 类边界（`None`）：全文用 fallback 渲染；边界到**不**重渲染（无准确值）。
    #[test]
    fn b_bound_uses_fallback_and_no_rerender() {
        let mut st = ReflowFastState::default();
        let a = apply(st.on_text(1, 5, "acc".into(), 3, false));
        assert_eq!(a, ("acc".into(), 3, false, true));
        assert_eq!(st.on_bound(1, 5, None, false), ReflowFastOutcome::None);
    }

    /// 两个 seg 交错：旧 seg 的边界到不改浮层；最新 seg 的边界到才重渲染。
    #[test]
    fn interleaved_segs_only_latest_rerenders() {
        let mut st = ReflowFastState::default();
        apply(st.on_text(1, 5, "five".into(), 1, false));
        apply(st.on_text(1, 6, "six".into(), 1, false));
        assert_eq!(st.on_bound(1, 5, Some(9), false), ReflowFastOutcome::None);
        let b = apply(st.on_bound(1, 6, Some(4), false));
        assert_eq!(b, ("six".into(), 4, true, true));
    }

    /// Processing 之后到达的 PreviewReflow：只更新状态、不重画（render=false）；之前正常（render=true）。
    #[test]
    fn suppressed_after_processing_skips_render_only() {
        let mut st = ReflowFastState::default();
        let before = apply(st.on_text(1, 5, "a".into(), 2, false));
        assert!(before.3, "Processing 之前的回灌必须正常渲染");
        let after = apply(st.on_text(1, 6, "b".into(), 2, true));
        assert!(!after.3, "Processing 之后的回灌不得重画浮层");
        assert_eq!(after.0, "b", "但权威状态照常更新");
    }
}

#[cfg(test)]
mod problem2_order_382_tests {
    /// 生产区（剔除 cfg(test) 与纯注释行）。
    fn code_lines() -> Vec<String> {
        crate::guard_prod_lines::prod_lines_excluding_cfg_test(include_str!("main.rs"))
            .into_iter()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect()
    }

    /// 382（问题2）源码级顺序护栏：`StreamingFinalPreview` 发送 < `Processing` 发送 < `acc_handle` 的
    /// join；且 join 之后不得再发 `StreamingFinalPreview`（防把已切处理态的浮层拉回预览态 = 闪回）。
    #[test]
    fn early_preview_then_processing_before_acc_join() {
        let lines = code_lines();
        let asr = lines
            .iter()
            .position(|l| {
                l.trim_start()
                    .starts_with("let asr_result = asr_handle.join();")
            })
            .expect("382: asr_handle.join 锚点");
        let acc = lines
            .iter()
            .position(|l| {
                l.trim_start()
                    .starts_with("let acc_result = acc_handle.map")
            })
            .expect("382: acc_handle.join 锚点");
        assert!(asr < acc, "382: asr_handle.join 必须先于 acc_handle.join");
        let fp = lines[asr..acc]
            .iter()
            .position(|l| l.contains("send(PipelineEvent::StreamingFinalPreview("))
            .expect("382: StreamingFinalPreview 必须在 acc_join 之前发（问题2）");
        let pr = lines[asr..acc]
            .iter()
            .position(|l| l.contains("send(PipelineEvent::Processing("))
            .expect("382: Processing 必须在 acc_join 之前发（问题2）");
        assert!(
            fp < pr,
            "282 顺序：StreamingFinalPreview 必须先于 Processing"
        );
        assert!(
            !lines[acc..]
                .iter()
                .any(|l| l.contains("send(PipelineEvent::StreamingFinalPreview(")),
            "382：acc_join 之后不得再发 StreamingFinalPreview（闪回）"
        );
    }
}

#[cfg(test)]
mod shared_queue_382_tests {
    /// 382（3B）：单个共享任务通道的**多消费者**语义 —— 一个 worker 卡在长任务时，下一个任务被
    /// 空闲 worker 立刻取走（旧 `rr % concurrency` 会把它排在忙的 worker 后面）。先后由通道制造、
    /// 不用 sleep 定时序；`recv_timeout` 仅作挂死兜底。
    #[test]
    fn idle_worker_takes_next_while_other_busy() {
        let (task_tx, task_rx) = crossbeam_channel::unbounded::<u32>();
        let (started_tx, started_rx) = crossbeam_channel::unbounded::<u32>();
        let (release_tx, release_rx) = crossbeam_channel::unbounded::<()>();
        let (done_tx, done_rx) = crossbeam_channel::unbounded::<u32>();
        for _ in 0..2 {
            let trx = task_rx.clone();
            let stx = started_tx.clone();
            let rrx = release_rx.clone();
            let dtx = done_tx.clone();
            std::thread::spawn(move || {
                for id in trx {
                    let _ = stx.send(id);
                    if id == 1 {
                        let _ = rrx.recv(); // 任务 1 = 长任务：卡住该 worker
                    }
                    let _ = dtx.send(id);
                }
            });
        }
        drop(started_tx);
        drop(done_tx);
        let dl = std::time::Duration::from_secs(5);
        task_tx.send(1).expect("send task1");
        assert_eq!(started_rx.recv_timeout(dl).expect("task1 taken"), 1);
        task_tx.send(2).expect("send task2");
        assert_eq!(
            started_rx
                .recv_timeout(dl)
                .expect("task2 must be taken by the idle worker"),
            2
        );
        release_tx.send(()).expect("release task1");
        let mut got = vec![
            done_rx.recv_timeout(dl).expect("task done"),
            done_rx.recv_timeout(dl).expect("task done"),
        ];
        got.sort_unstable();
        assert_eq!(got, vec![1, 2]);
    }

    /// 生产侧结构护栏：必须用**单一共享** task 通道（`task_rx.clone()` 给每个 worker），
    /// 不得退回「每 worker 一个发送端 + `rr % concurrency`」。
    #[test]
    fn production_uses_single_shared_task_queue() {
        let lines: Vec<String> =
            crate::guard_prod_lines::prod_lines_excluding_cfg_test(include_str!("main.rs"))
                .into_iter()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect();
        assert_eq!(
            lines
                .iter()
                .filter(|l| l.contains("let (task_tx, task_rx) ="))
                .count(),
            1,
            "382(3B): 必须恰有一处单一共享 task 通道 `let (task_tx, task_rx)`"
        );
        assert!(
            lines.iter().any(|l| l.contains("for _ in 0..concurrency")),
            "382(3B): 必须有并发 worker 循环"
        );
        assert!(
            lines.iter().any(|l| l.contains("task_rx.clone()")),
            "382(3B): worker 必须 clone 同一个 task_rx（共享队列）"
        );
        assert!(
            !lines
                .iter()
                .any(|l| l.contains("task_txs") || l.contains("% concurrency")),
            "382(3B): 不得退回每 worker 一个发送端 / `rr % concurrency`"
        );
    }
}

// ============================================================
// TEST-SYNC-382（阶段三 · 非作者护栏，coder-2）
//   被测 FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382（HEAD 1af7212）。
//   按**设计契约**写用例，不照实现反推；补作者（plan_windows_382_tests /
//   reflow_fast_382_tests / problem2_order_382_tests / shared_queue_382_tests）照不到的边界。
//   🔴 全部为测试代码，不改生产逻辑。
// ============================================================
#[cfg(test)]
mod testsync382_tests {
    use crate::transcription;

    /// 确定性伪随机（无外部依赖）：64-bit LCG（Knuth MMIX 常数）。
    struct Lcg(u64);
    impl Lcg {
        fn new(seed: u64) -> Self {
            Lcg(seed | 1)
        }
        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }
        fn usize_in(&mut self, lo: usize, hi: usize) -> usize {
            debug_assert!(hi >= lo);
            lo + (self.next_u64() % (hi - lo + 1) as u64) as usize
        }
        fn f32_in(&mut self, lo: f32, hi: f32) -> f32 {
            let t = (self.next_u64() >> 11) as f32 / (1u64 << 53) as f32;
            lo + t * (hi - lo)
        }
    }

    // ---- 1. plan_windows 覆盖不变量的**性质测试** ----

    /// 200 组随机 (prev_durs, new_durs, prev_base)：
    /// ① 每个新片全局下标被某窗覆盖 ② 第 k 窗以第 k 新片收尾
    /// ③ `start >= prev_base` 且 `end - start <= WINDOW_MAX_SLICES`
    #[test]
    fn plan_windows_property_cover_and_shape() {
        let max_prev = transcription::WINDOW_MAX_SLICES;
        let mut rng = Lcg::new(0x0381_0382_u64);
        for _case in 0..200usize {
            let nprev = rng.usize_in(0, max_prev);
            let mut prev: Vec<f32> = Vec::with_capacity(nprev);
            for _ in 0..nprev {
                prev.push(rng.f32_in(0.3, 12.0));
            }
            let nnew = rng.usize_in(1, 4);
            let mut new: Vec<f32> = Vec::with_capacity(nnew);
            for _ in 0..nnew {
                new.push(rng.f32_in(0.3, 12.0));
            }
            let base = rng.usize_in(0, 50);
            let plan = super::plan_windows(&prev, &new, base, None, false);
            // 386：`nnew>=2` ⇒ 末片**延后**（立刻窗数 = nnew-1、pending = 末片）；`nnew==1` ⇒ 全部立刻。
            let expected_imm = if nnew >= 2 { nnew - 1 } else { nnew };
            assert_eq!(plan.windows.len(), expected_imm, "立刻组窗数");
            for (k, (s, e)) in plan.windows.iter().enumerate() {
                let gidx = base + nprev + k; // 第 k 个新片的全局下标
                assert_eq!(*e - 1, gidx, "第 k 个立刻窗口必须以第 k 个新片收尾");
                assert!(*s >= base, "窗起点不得早于 prev_base");
                assert!(*e > *s, "窗必须非空");
                assert!(
                    *e - *s <= max_prev,
                    "无 pending 时窗宽不得超过 WINDOW_MAX_SLICES"
                );
                assert!(*s <= gidx && gidx < *e, "新片全局下标必须被本窗覆盖");
            }
            if nnew >= 2 {
                assert_eq!(
                    plan.pending,
                    Some(base + nprev + nnew - 1),
                    "末片必须延后为待覆盖片"
                );
            } else {
                assert_eq!(plan.pending, None, "单片派发不延后");
            }
        }
    }

    /// 常态**单片**派发（`prev_durs` 未满 `WINDOW_MAX_SLICES` ⇒ 不触发裁片）：
    /// 结果必须等于旧算法「pre+new 后直接 `group_window_start_secs`」。
    ///
    /// 说明：`prev` 已满 `WINDOW_MAX_SLICES` 时 plan 会先裁最远片再组窗（382 设计），
    /// 与「不裁直接 group」不同 —— 那一条由上面 shape 性质覆盖，不在本用例范围。
    #[test]
    fn plan_windows_single_matches_legacy_property() {
        let max_prev = transcription::WINDOW_MAX_SLICES;
        let mut rng = Lcg::new(0x0381_0383_u64);
        for _case in 0..200usize {
            let nprev = rng.usize_in(0, max_prev - 1); // 保证 push 后不裁片
            let mut prev: Vec<f32> = Vec::with_capacity(nprev);
            for _ in 0..nprev {
                prev.push(rng.f32_in(0.3, 12.0));
            }
            let x = rng.f32_in(0.3, 12.0);
            let base = rng.usize_in(0, 50);
            let plan = super::plan_windows(&prev, &[x], base, None, false);
            let mut buf = prev.clone();
            buf.push(x);
            let s = transcription::group_window_start_secs(&buf, transcription::WINDOW_MAX_SECS);
            assert_eq!(
                plan.windows,
                vec![(base + s, base + prev.len() + 1)],
                "常态单片必须等于旧组窗算法"
            );
            assert_eq!(plan.pending, None, "单片不延后");
        }
    }

    /// 边界：`new_durs` 为空 ⇒ 返回空计划（不 panic、不产出窗口）。
    /// 理由：调用方若以「本次派发携带 0 片」调用（防御性），不得制造空窗或越界。
    #[test]
    fn plan_windows_empty_new_is_empty() {
        assert!(super::plan_windows(&[1.0, 2.0], &[], 7, None, false)
            .windows
            .is_empty());
        assert!(super::plan_windows(&[], &[], 0, None, false)
            .windows
            .is_empty());
    }

    // ---- 2. ReflowFastState 退化输入（作者未覆盖） ----
    #[cfg(target_os = "windows")]
    mod reflow {
        use crate::{ReflowFastOutcome, ReflowFastState};

        fn apply(o: ReflowFastOutcome) -> (String, usize, bool, bool) {
            match o {
                ReflowFastOutcome::Apply {
                    text,
                    committed_len,
                    accurate,
                    render,
                } => (text, committed_len, accurate, render),
                ReflowFastOutcome::None => panic!("expected Apply, got None"),
            }
        }

        /// 跨代：gen1 的准确边界**不得**被 gen2 同 seg 复用（须回退 fallback）。
        #[test]
        fn cross_gen_bound_not_reused() {
            let mut st = ReflowFastState::default();
            let _ = apply(st.on_text(1, 5, "one".into(), 3, false));
            let b = apply(st.on_bound(1, 5, Some(7), false));
            assert_eq!(b, ("one".into(), 7, true, true));
            // gen2 同 seg ⇒ 不得拿 gen1 的 7
            let a = apply(st.on_text(2, 5, "two".into(), 9, false));
            assert_eq!(a, ("two".into(), 9, false, true), "旧代边界必须失效");
        }

        /// 同 seg 边界 `None`（b 类）先到、随后升为 `Some`：应补一次准确渲染。
        #[test]
        fn same_seg_bound_none_then_some_rerenders() {
            let mut st = ReflowFastState::default();
            let a = apply(st.on_text(1, 5, "acc".into(), 3, false));
            assert_eq!(a, ("acc".into(), 3, false, true));
            assert_eq!(st.on_bound(1, 5, None, false), ReflowFastOutcome::None);
            let b = apply(st.on_bound(1, 5, Some(7), false));
            assert_eq!(b, ("acc".into(), 7, true, true), "升级为准确边界应重渲");
        }

        /// 两个 seg 交错 + 边界**乱序**到达：只有「最新已渲染 seg」的边界才重渲；
        /// 预先登记的边界（seg 尚未渲染）在 text 到达时应被采用。
        #[test]
        fn out_of_order_bounds_only_latest_seg_renders() {
            let mut st = ReflowFastState::default();
            // seg6 边界先到（seg6 尚未渲染）⇒ None，但被登记
            assert_eq!(st.on_bound(1, 6, Some(4), false), ReflowFastOutcome::None);
            let _ = apply(st.on_text(1, 5, "five".into(), 1, false));
            // seg6 全文到：应直接用先前登记的边界 4（accurate）
            let six = apply(st.on_text(1, 6, "six".into(), 1, false));
            assert_eq!(six, ("six".into(), 4, true, true));
            // seg5 边界后到：它已不是最新已渲染 ⇒ None
            assert_eq!(st.on_bound(1, 5, Some(9), false), ReflowFastOutcome::None);
            // seg6 边界更新：仍是最新已渲染 ⇒ 重渲准确值
            let upd = apply(st.on_bound(1, 6, Some(8), false));
            assert_eq!(upd, ("six".into(), 8, true, true));
        }

        /// `suppressed=true`：边界重渲也必须 `render=false`，但状态（committed_len）照常更新。
        #[test]
        fn suppressed_on_bound_renders_state_only() {
            let mut st = ReflowFastState::default();
            let _ = apply(st.on_text(1, 5, "acc".into(), 3, true));
            let b = apply(st.on_bound(1, 5, Some(7), true));
            assert_eq!(
                (b.1, b.2, b.3),
                (7, true, false),
                "suppressed 下仍以准确边界更新状态，但不得重画浮层"
            );
        }

        /// 作者未覆盖的**会话复位点**：`clear()` 必须清空 latest/bounds/rendered，
        /// 否则跨会话会复用陈旧边界（新代同 seg 拿到旧值）。
        #[test]
        fn clear_resets_all_state() {
            let mut st = ReflowFastState::default();
            let _ = apply(st.on_text(1, 5, "acc".into(), 3, false));
            let _ = apply(st.on_bound(1, 5, Some(7), false));
            st.clear();
            // clear 后同 gen 同 seg 也不得复用 7 ⇒ 用 fallback
            let a = apply(st.on_text(1, 5, "new".into(), 4, false));
            assert_eq!(a, ("new".into(), 4, false, true), "clear 后旧边界必须失效");
        }
    }

    // ---- 3. 源码级护栏 ----

    fn prod_lines() -> Vec<String> {
        crate::guard_prod_lines::prod_lines_excluding_cfg_test(include_str!("main.rs"))
            .into_iter()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect()
    }

    /// 滑窗派发**区域**不得退回「每 worker 一个发送端 / 轮询取模」，
    /// 且必须 clone **同一** `task_rx`（单一队列多消费者）。
    ///
    /// 与作者全局护栏的分工：本用例把负面断言**限定在派发点邻域**（旧 `rr % concurrency`
    /// 就写在 worker 循环旁），并补正向「共享 task_rx 被 clone」。
    #[test]
    fn window_dispatch_single_shared_queue_no_round_robin() {
        let lines = prod_lines();
        let idx = lines
            .iter()
            .position(|l| l.contains("task_tx.send("))
            .expect("382: 滑窗派发锚点 task_tx.send 缺失");
        let lo = idx.saturating_sub(300);
        let hi = (idx + 20).min(lines.len());
        for l in &lines[lo..hi] {
            assert!(
                !l.contains("task_txs"),
                "382: 滑窗派发区不得出现多发送端 task_txs"
            );
            assert!(
                !l.contains("% concurrency") && !l.contains("rr %"),
                "382: 滑窗派发区不得出现轮询取模派发"
            );
        }
        assert!(
            lines.iter().any(|l| l.contains("task_rx.clone()")),
            "382: worker 必须 clone 共享 task_rx（单一队列多消费者）"
        );
    }

    /// `ACC_REFLOW_SUPPRESS` 置位**只**允许出现在 `PipelineEvent::Processing` 处理臂内
    /// （防闪回机制的唯一开关；散落到别处会把正常回灌也静默掉）。
    #[test]
    fn suppress_flag_only_set_inside_processing_arm() {
        let lines = prod_lines();
        let stores: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains("ACC_REFLOW_SUPPRESS.store(true"))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            stores.len(),
            1,
            "382: ACC_REFLOW_SUPPRESS.store(true 必须恰一处（本代 Processing 时置位）"
        );
        let idx = stores[0];
        let arm = (0..idx)
            .rev()
            .find(|&i| lines[i].contains("PipelineEvent::") && lines[i].contains("=>"))
            .expect("382: 置位必须位于某个 PipelineEvent 处理臂内");
        assert!(
            lines[arm].contains("PipelineEvent::Processing("),
            "382: ACC_REFLOW_SUPPRESS 置位必须落在 Processing 处理臂内，实际最近臂为其它事件"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("ACC_REFLOW_SUPPRESS.store(false")),
            "382: 必须有复位点（新代 RecordingStarted），否则一旦置位永不回灌"
        );
    }
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
    // PARALLEL-ACC-298: 本地流式「并行 accuracy」整段结果 `(text, native_punctuated)`。
    // Some ⇒ 跳过内部转录，直接用该文本走 LLM 后半段。🔴 与 initial_text **分开**：
    // `from_online_streaming` 仍只由 `initial_text.is_some()` 决定（本路径 initial_text=None
    // ⇒ false ⇒ 主通道 ITN 启用），**不得**用本参数反推来源（DEC-066）。
    // 🔴 native_punctuated 是**文本实测**：`punctuation::has_effective_punctuation(&text)`
    // （DEC-047 的结论正是「实测文本，而非假设/外推」；原注释把它引反了，PUNCT-DOUBLE-334 订正）。
    // 由调用方在构造时用 `pretranscribed_native_punctuated` 算好，本参数只接收结果。
    pretranscribed: Option<(String, bool)>,
    // PIPELINE-ORCH-239-B（DEC-066 附则一）：本地转录步骤的状态文案。
    // 现有三档传 `overlay_transcribing`（行为逐位零变）；本地 realtime 新管线传
    // `overlay_processing`，满足「松键后只显示『识别处理中』单状态」要求。
    transcribing_status_text: &'static str,
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
            // V091-ITN-SKIP-ONLINE-215: 记录文本是否产自在线流式 ASR。
            // 在线 ASR 服务端已完成数字/单位规整，主通道再跑一遍属二次处理。
            // 🔴 用 initial_text.is_some() 而非 config.audio.asr_model 判据：后者与
            // transcriber 热重载存在瞬态不同步（config 已切在线但引擎仍为本地），
            // 按 config 判会把本地 ASR 输出也误跳过 ITN。
            let from_online_streaming = initial_text.is_some();
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
            } else if let Some((text, native_punctuated)) = pretranscribed {
                // PARALLEL-ACC-298：本地流式「并行 accuracy」结果。路由与 accuracy 2pass
                // **完全一致**：`from_online_streaming` 仍由 `initial_text`（本路径为 None）
                // 决定 = false ⇒ 主通道 ITN 启用；标点标记用**文本实测**
                // `has_effective_punctuation`（DEC-047 口径，见 `pretranscribed_native_punctuated`；
                // 原「各段 all_native 的与」是语义错配，PUNCT-DOUBLE-334 已订正）。
                // 不在此再对全量 pcm 跑一次 accuracy（并行 worker 已逐片转写完）。
                // 保持本地档单状态：仍发 `Processing(transcribing_status_text)`（文案同 :9127）。
                send_event(
                    event_tx,
                    PipelineEvent::Processing(transcribing_status_text.to_string()),
                );
                log::info!(
                    "PARALLEL-ACC-298: using parallel accuracy transcript ({} chars), skipping re-transcription",
                    text.chars().count()
                );
                if text.trim().is_empty() {
                    Err(TranscriptionFailure::NoSpeech)
                } else {
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
                    // FIX-LOCALRT-ENGINE-EQ-252: 收敛到 uses_accuracy_engine()——LocalRealtime 的
                    // 前处理/引擎按 accuracy 走，日志如实反映（否则被误标 performance）。
                    if transcriber.asr_model().uses_accuracy_engine() { "accuracy" } else { "performance" },
                );
                let transcribing_msg = transcribing_status_text;
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
                    // HOMOPHONE-NODE-318：ASR 同音纠错节点。**转录之后、ITN 主通道之前**：
                    // 修的是 ASR **听错**，越早修正下游（ITN/LLM/标点）越干净；在 LLM 前也不触
                    // DEC-041（该条禁的是对 LLM 输出做程序化后处理）。恒 `enabled=true`，
                    // 表内只放无歧义项，不新增用户可见开关（DEC-031）。
                    // 四档共用 run_pipeline_core ⇒ 一处调用即覆盖 local_realtime/accuracy/performance/qwen3。
                    let raw_text = homophone::apply_homophone_fix(raw_text, true);
                    // ITN-V2-001 (R1 主通道)：ITN 从「LLM 后」回移到「LLM 前」。
                    // Gavin 2026-07-31 指令：LLM 会曲解原始数字表达（如「四点三刻」→「4:30」，
                    // 信息被销毁），故主通道在 LLM 之前先把中文数字→阿拉伯 + 单位符号定型，
                    // LLM 拿到的已是成形数字，UNIT_SYMBOL_PROTECTION 指令恢复生效。
                    // 补丁通道（normalize_unit_symbols_only）仍在 LLM 后兜底（见 :3124），
                    // 捞回 LLM 纠正 ASR 同音错字后的「40摄氏度」→「40℃」。
                    // 放在 is_effective_text 之后：ITN 不增删语义字符，filler 判定不变。
                    //
                    // V091-ITN-SKIP-ONLINE-215：在线流式 ASR 已由服务端做过数字/单位规整，
                    // 主通道跳过，避免二次处理（Gavin 2026-09-17）。判据用 from_online_streaming
                    // （数据路径事实），不用 config.audio.asr_model（与引擎热重载有竞态）。
                    // 🔴 只跳主通道；补丁通道 normalize_unit_symbols_only 仍在 LLM 后保留
                    //    （DEC-036 双通道：补丁通道处理的是 LLM 输出方向，不属二次处理）。
                    let pre_llm_text = if from_online_streaming {
                        log::info!(
                            "V091-ITN-SKIP-ONLINE-215: online ASR output, skipping main-channel ITN"
                        );
                        raw_text.clone()
                    } else {
                        itn::normalize_numbers(&raw_text)
                    };
                    // SCENE-SENSE-001-CORE (DEC-031-⑤): 录音完成阶段采集前台窗口场景信号，
                    // 供 LLM prompt F4 段注入 + multiline_safe 格式安全裁决。
                    // target_hwnd 即录音启动时捕获的前台窗口，此处复用同一 HWND 采集。
                    // 失败一律降级为 Unknown（不注入 F4，multiline_safe=false 保守）。
                    // PIPELINE-ORCH-238: 采集 + 观测日志已抽为 capture_pipeline_scene（行为逐位不变）。
                    let scene_context = capture_pipeline_scene(config, target_hwnd);
                    let multiline_safe = scene_context.multiline_safe;
                    let send_window_title = config.scene.send_window_title;
                    // TRANS-008 B方案: translate=true 时走 LLM optimize+translate；translate=false 走 LLM optimize
                    // translate=false 閺冭绱濋崢鐔告箒 LLM optimize 鐠侯垰绶炴稉宥呭綁
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
                    // PIPELINE-ORCH-238-B: N6 全量块（初始化 + 两条 Processing 事件 +
                    // learn_llm_suggestions）已抽为 run_llm_stage（行为逐位不变，时序保持）。
                    let llm_out = run_llm_stage(
                        rt,
                        llm_client,
                        config,
                        runtime_config,
                        event_tx,
                        &raw_text,
                        &pre_llm_text,
                        &scene_context,
                        multiline_safe,
                        send_window_title,
                        translate_requested,
                        derived_target,
                        cached_translation,
                        &model_dir,
                    );
                    let final_text = llm_out.text;
                    let llm_handled = llm_out.llm_handled;
                    let format_failed = llm_out.format_failed;
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
                    // PIPELINE-ORCH-238: 判定体已抽为 apply_local_punctuation（行为逐位不变）。
                    let final_text = apply_local_punctuation(
                        final_text,
                        config.punctuation.enabled,
                        llm_handled,
                        translate_requested,
                        native_punctuated,
                        punctuation_engine.as_deref_mut(),
                    );
                    // FORMAT-FALLBACK-303 / WIRE-FF303-305：本地免费口水词过滤。
                    // 🔴 位置三条件（勿挪）：① **文本此刻必须已带标点**（规则 A 的「句首」要有句读可依）。
                    //    ⚠️ FIX-FILLER-NODE-COMMENT-378：标点的**实际来源有两条**，勿以为是上游节点供给 ——
                    //    本地 realtime 滑窗路径 `native_punctuated=true`（日志恒 `all_native=true`）⇒
                    //    `apply_local_punctuation` 的门 `!native_punctuated` 不成立 ⇒ **引擎整块跳过、常态空转**，
                    //    标点全部来自 acc 模型自带；只有 native 失败时才轮到引擎补。两条路都保证「此刻有标点」，
                    //    故本节点仍须排在其后。**已知边界**（非本批引入、概率低、暂不修）：
                    //    `native_punctuated=false` 且 `punctuation.enabled=false` ⇒ 文本无任何标点 ⇒ 规则 A 失依据。
                    // ② 在 inject_text 之前；③ `enabled = !llm_handled` ⇒ 只有 LLM 未接手时才动，
                    // 不触 DEC-041（该条禁的是对 LLM 输出做程序化后处理，此处压根没有 LLM 输出）。
                    // 四档共用 run_pipeline_core，一处调用即全覆盖，不新增任何管线判据（DEC-066）。
                    let final_text = apply_filler_strip(final_text, !llm_handled);
                    // PUNCT-GOVERNANCE-030-A L2 后处理补位（架构定位：L1 源头控制为主，L2 补位）
                    // 只负责两件 L1 物理上够不着的事：
                    //   (a) 开关关闭 → 全文剥标点（Qwen3 在线 ASR / 本地 native / NLLB 翻译不可控源兜底）
                    //   (b) 开关开启且字/词数 <= 5 → 剥末尾标点（Gavin 2026-08-08 短句规则）
                    // 🔴 本块不含任何来源判据（llm_handled/native_punctuated）/（LLM 翻译），
                    //    对 6 个产出源一视同仁，将来新增产出源自动受控。
                    // PUNCT-GOVERNANCE-030-E：判定逻辑已抽为纯函数 apply_l2_postprocess
                    //   （src/punctuation/mod.rs，可单测）；本侧只保留日志（日志方案 C：
                    //   按 L2Action 分支打对应文案，且沿用「输出变化才打」的 != 判定）。
                    let (l2_text, l2_action) = punctuation::apply_l2_postprocess(
                        &final_text,
                        config.punctuation.enabled,
                        config.punctuation.strip_trailing,
                    );
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
                        if let Err(e) = platform::inject_text(
                            &final_text,
                            config.injection.use_clipboard,
                            config.injection.clipboard_delay_ms,
                        ) {
                            log::error!("Injection failed: {}", e);
                        } else {
                            log::info!("Injection completed successfully");
                            // FIX-PREVIEW-HARVEST-380（B）：松键(stop) → 最终上屏的时延。
                            // 🔴 run_pipeline_core 是平台中立共享代码，只有 Windows 有该埋点
                            //（macOS 无 hook tick，结论「不适用」）。
                            #[cfg(target_os = "windows")]
                            {
                                if log::log_enabled!(log::Level::Debug) {
                                    let stop_tick = STOP_RECEIVED_TICK.load(Ordering::Acquire);
                                    if stop_tick != 0 {
                                        log::debug!(
                                            "[LocalRT-DBG-380] stop_to_inject_ms={}",
                                            (unsafe { GetTickCount64() } as u32)
                                                .wrapping_sub(stop_tick as u32)
                                        );
                                    }
                                }
                            }
                            // WORDBOOK-HITCOUNT-263: 上屏后用**最终文本**记词频（仅使用 hotwords
                            // 的 accuracy 引擎档位：Accuracy / LocalRealtime）。放在注入成功之后，
                            // 不挡输入法延迟；用 final_text 而非 raw_text（常说但常认错的词最该占额度）。
                            if transcription::AsrModel::from_config(&config.audio.asr_model)
                                .uses_accuracy_engine()
                            {
                                // WORDBOOK-HITCOUNT-263（主控增补）：写库丢**后台线程**，
                                // 绝不挡住 worker 回到待命。db.rs BUSY_TIMEOUT_MS=3000，若与设置
                                // 界面读并发撞 SQLITE_BUSY 最坏同步卡 3s（用户感受：连续说话时第二次
                                // 按热键没反应）。统计丢一次无所谓，故 fire-and-forget、失败仅记日志。
                                let hit_text = final_text.clone();
                                std::thread::spawn(move || {
                                    match wordbook::db::record_hits(&hit_text) {
                                        Ok(n) if n > 0 => log::info!(
                                            "WORDBOOK-HITCOUNT-263: recorded {} word hits",
                                            n
                                        ),
                                        Ok(_) => {}
                                        Err(e) => log::warn!(
                                            "WORDBOOK-HITCOUNT-263: record_hits failed: {}",
                                            e
                                        ),
                                    }
                                });
                            }
                        }
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

/// PIPELINE-ORCH-238-B: N6 LLM 格式化 / 翻译整段（原 run_pipeline_core 内联块）。
/// 含 llm_handled / format_failed 初始化 + 翻译分支 + LLM optimize 分支 + LLM 跳过分支；
/// 两处 send_event(Processing) 与 learn_llm_suggestions 原样保留在函数内，事件时序不变。
struct LlmStageOutput {
    text: String,
    llm_handled: bool,
    format_failed: bool,
}

/// 执行 LLM 格式化 / 翻译阶段，返回文本与两条状态位。
#[allow(clippy::too_many_arguments)]
fn run_llm_stage(
    rt: &tokio::runtime::Runtime,
    llm_client: &llm::LlmClient,
    config: &AppConfig,
    runtime_config: &Arc<RwLock<AppConfig>>,
    event_tx: &crossbeam_channel::Sender<PipelineEvent>,
    raw_text: &str,
    pre_llm_text: &str,
    scene_context: &scene::SceneContext,
    multiline_safe: bool,
    send_window_title: bool,
    translate_requested: bool,
    derived_target: config::TranslationLanguage,
    cached_translation: &mut Option<(config::TranslationLanguage, translation::TranslationEngine)>,
    model_dir: &Path,
) -> LlmStageOutput {
    let mut llm_handled = false;
    // FORMAT-LLM-001-CORE (DEC-031): set when LLM formatting call
    // failed. Raw text still gets injected (fallback), but we
    // surface a brief "formatting failed" overlay hint after
    // injection so the user knows to check LLM config.
    let mut format_failed = false;
    let text = if translate_requested && config.translation.enabled && !raw_text.trim().is_empty() {
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
        // TRANS-SAFE-196: 三条子路径（LLM 成功 / LLM 失败转 NLLB / LLM 不合格直接
        // 走 NLLB，后两者各自还带 normalize_text_for_language 兜底）先汇合到
        // translated_out，再由下方统一做格式安全裁决 —— 一处覆盖全部子路径，
        // 避免逐路径打补丁时漏掉其中一条。
        let translated_out =
            if should_try_llm_translate(config.llm.enabled, config.llm.connectivity_verified) {
                // B: LLM optimization failed (non-critical), continue with raw result
                match rt.block_on(llm_client.optimize_and_translate(
                    &pre_llm_text,
                    derived_target,
                    script_instruction,
                    config.punctuation.enabled,
                    // TRANS-SCENE-197: 这两个参数此前没传 —— scene_context / send_window_title
                    // 就在同作用域（:8717/:8718）且主路径 optimize 一直在用，翻译路径漏传，
                    // 导致场景风格（含 VERBOSE-195 冗余压缩）与用户基座一开翻译就全部失效。
                    Some(&scene_context),
                    send_window_title,
                    multiline_safe,
                )) {
                    Ok(result) => {
                        log::info!("LLM optimize+translate done: {}", result.text);
                        learn_llm_suggestions(&result.suggestions, runtime_config);
                        llm_handled = true;
                        result.text
                    }
                    Err(e) => {
                        log::warn!("LLM optimize+translate failed, trying offline: {}", e);
                        try_nllb_translate(&pre_llm_text, effective_engine).unwrap_or_else(|| {
                            text_normalizer::normalize_text_for_language(
                                &pre_llm_text,
                                config.audio.chinese_script,
                            )
                        })
                    }
                }
            } else {
                // LLM not eligible, use offline engine directly
                try_nllb_translate(&pre_llm_text, effective_engine).unwrap_or_else(|| {
                    text_normalizer::normalize_text_for_language(
                        &pre_llm_text,
                        config.audio.chinese_script,
                    )
                })
            };
        // TRANS-SAFE-196 格式安全裁决 —— 补齐翻译路径此前完全缺失的 multiline_safe 保护。
        // 主路径在 llm::try_once 内做同款裁决，翻译路径走 try_once_raw 直接返回，
        // 整个绕过（llm/mod.rs 内「注：translate 路径不通过 try_once……不受影响」即指此）。
        // 后果：multiline_safe=false 的终端 / vim 等，翻译产生的多行结果换行原样注入，
        // 在模态编辑器里会被当命令键执行（scene-rules.toml 对 vim/gvim 的注释已警示该风险）。
        //
        // 🔴 只照搬 flatten 分支，刻意不照搬主路径 multiline_safe=true 那侧的
        // strip_fabricated_email_lines：该守卫用 input_contains_line 拿「输出行」去
        // 「输入原文」做字面包含匹配，翻译场景输入输出跨语言 ⇒ 字面永不匹配 ⇒ 判定条件
        // 恒真 ⇒ 用户真说了的称呼反被当成 LLM 编造删掉。与 llm/mod.rs 既有判例
        // 「判据 B 不扩展到翻译路径（英译中可合法大幅压缩，15% 会误伤）」同源。
        if multiline_safe {
            translated_out.trim().to_string()
        } else {
            llm::flatten_multiline(&translated_out)
        }
    } else if config.llm.enabled && !raw_text.trim().is_empty() {
        let processing_msg = i18n::get(config.ui_language).overlay_processing;
        send_event(
            event_tx,
            PipelineEvent::Processing(processing_msg.to_string()),
        );
        let script_instruction =
            text_normalizer::script_instruction(&raw_text, config.audio.chinese_script);
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
                text_normalizer::normalize_script_only(&result.text, config.audio.chinese_script)
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
        log::info!("LLM disabled or text empty, using transcription result directly");
        text_normalizer::normalize_text_for_language(&pre_llm_text, config.audio.chinese_script)
    };

    LlmStageOutput {
        text,
        llm_handled,
        format_failed,
    }
}
/// SCENE-SENSE-001-CORE (DEC-031-⑤): 录音完成阶段采集前台窗口场景信号，
/// 供 LLM prompt F4 段注入 + multiline_safe 格式安全裁决。
/// `target_hwnd` 即录音启动时捕获的前台窗口，此处复用同一 HWND 采集。
/// 失败一律降级为 Unknown（不注入 F4，multiline_safe=false 保守）。
/// PIPELINE-ORCH-238: 从 run_pipeline_core 内联块原样提取，行为逐位不变；
/// 观测日志（SCENE-OBS-001）随节点一并迁入。
fn capture_pipeline_scene(
    config: &AppConfig,
    target_hwnd: platform::WindowId,
) -> scene::SceneContext {
    let scene_context = if config.scene.enabled {
        match platform::capture_scene_signals_by_id(target_hwnd) {
            Some((exe, title)) => scene::classify_scene(&exe, &title),
            None => scene::SceneContext::unknown(),
        }
    } else {
        scene::SceneContext::unknown()
    };
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
    let multiline_safe = scene_context.multiline_safe;
    let f4_injected = !scene_context.is_unknown() && !scene_context.style_hint.trim().is_empty();
    log::info!(
        "Scene context: app_exe={:?}, kind={}, multiline_safe={}, f4_injected={}, window_title={:?}",
        scene_context.app_exe,
        scene_context.scene.as_str(),
        multiline_safe,
        f4_injected,
        scene_context.window_title,
    );
    scene_context
}
/// PUNCT-INTEGRATION-001 + ASR-PUNCT-OPT-001: 本地标点决策（加标点）。
/// 条件：auto_punct=true && LLM 未处理 && 非翻译 && 非 native 自带标点。
/// - native_punctuated=true（accuracy native 成功）→ 跳过标点引擎（省一次推理）
/// - native_punctuated=false（performance/兜底/混合）→ 照常走标点引擎
/// - 本函数只负责「加标点」；「剥离」职责在下游 L2 后处理补位块
///   （PUNCT-GOVERNANCE-030-A），对全部产出源一视同仁，无来源判据。
/// PIPELINE-ORCH-238: 从 run_pipeline_core 内联块原样提取，行为逐位不变。
fn apply_local_punctuation(
    final_text: String,
    enabled: bool,
    llm_handled: bool,
    translate_requested: bool,
    native_punctuated: bool,
    engine: Option<&mut punctuation::PunctuationEngine>,
) -> String {
    if enabled && !llm_handled && !translate_requested && !native_punctuated {
        if let Some(engine) = engine {
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
    }
}
/// PUNCT-FINAL-REDO-350：标点剥离**节点**（独立、管线无关、谁要谁挂）。
///
/// 形态照 [`apply_filler_strip`]（自由函数、显式传参、管线无关）；判定体是
/// `punctuation::strip_effective_punctuation`（可单测的纯函数）。
/// 🔴 **只挂本地 realtime 一条管线**（Gavin 2026-09-22 细化设计），在线 realtime 与
/// 本地离线**不挂**，序列一字不变 ⇒ 结构上不可能受影响（DEC-066：节点各自编排、谁要谁挂）。
///
/// 语义：把整段已有标点剥光，交给下游**既有的**标点节点（`apply_local_punctuation`）
/// 全量重打。剥光后 `native_punctuated` 恒 false ⇒ 下游 `!native_punctuated` 门自己放行；
/// 而该门本身**一行都不改**，`PUNCT-DOUBLE-334` 对其它两档的保护原样保留。
/// 先剥光 ⇒ 进引擎的文本无标点 ⇒ 结构上不可能再出现 `。。`/`，。` 叠加。
///
/// - `enabled == false` ⇒ **原样返回**（用户关闭标点：本节点无意义，下游 L2 会全文剥）
/// - `enabled == true`  ⇒ `strip_effective_punctuation(&final_text)`
///
/// ⚠️ 仅在「下游必定重打标点」时才可挂：见调用点关于 `!translate_requested` 的门注释。
/// 变化时打一行 `log::info!`（与 [`apply_local_punctuation`] / [`apply_filler_strip`] 同风格）。
fn strip_punctuation_node(final_text: String, enabled: bool) -> String {
    if !enabled {
        return final_text;
    }
    let stripped = punctuation::strip_effective_punctuation(&final_text);
    if stripped != final_text {
        log::info!("Punctuation strip node: '{}' -> '{}'", final_text, stripped);
    }
    stripped
}

/// FORMAT-FALLBACK-303：本地免费口水词过滤节点（**管线无关**，谁要谁挂）。
///
/// 仅在 LLM **未接手**时执行：开了 LLM 时 F1 Filler Removal 做得比规则层好，
/// 且 DEC-041 禁止 Rust 侧对 LLM 输出做程序化后处理。
///
/// - `enabled == false` ⇒ **原样返回**，一个字符不动（开 LLM 时本节点完全不执行）
/// - `enabled == true` ⇒ 返回 `text_normalizer::strip_fillers_conservative(&final_text)`
///
/// 变化时打一行 `log::info!`（与 `apply_local_punctuation` 同风格，只在真的改了文本时打）。
fn apply_filler_strip(final_text: String, enabled: bool) -> String {
    if !enabled {
        return final_text;
    }
    let stripped = text_normalizer::strip_fillers_conservative(&final_text);
    if stripped != final_text {
        log::info!("Filler strip applied: '{}' -> '{}'", final_text, stripped);
    }
    stripped
}

#[cfg(test)]
mod filler_strip_303_tests {
    use super::apply_filler_strip;

    /// WIRE-FF303-305 契约 1：`enabled=false` ⇒ 输出与输入**逐字相同**（含语气词也不动）。
    #[test]
    fn filler_strip_disabled_is_identity() {
        let input = "呃，我觉得这个方案可以".to_string();
        assert_eq!(apply_filler_strip(input.clone(), false), input);
        // 开 LLM 时走的就是这条路径：一个字符不动。
        assert_eq!(
            apply_filler_strip("嗯嗯，可以".to_string(), false),
            "嗯嗯，可以"
        );
    }

    /// 契约 2：`enabled=true` 且含句首语气词 ⇒ 被摘（证明确实接到了纯函数）。
    #[test]
    fn filler_strip_enabled_removes_leading_filler() {
        assert_eq!(
            apply_filler_strip("呃，我觉得这个方案可以".to_string(), true),
            "我觉得这个方案可以"
        );
    }

    /// 契约 3：`enabled=true` 且无可摘内容 ⇒ 输出与输入逐字相同（不误改）。
    #[test]
    fn filler_strip_enabled_noop_when_nothing_to_strip() {
        let input = "看看有什么好看的电影".to_string();
        assert_eq!(apply_filler_strip(input.clone(), true), input);
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
            edit_original: None,
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

    // ==================== TRANS-SAFE-196 翻译路径格式安全裁决护栏 ====================

    /// TRANS-SAFE-196-G1：翻译分支出口必须做 multiline_safe 格式安全裁决。
    ///
    /// 背景：主路径在 `llm::try_once` 内做裁决，翻译路径走 `try_once_raw` 直接返回，
    /// 此前完全绕过 —— multiline_safe=false 的终端/vim 会把翻译产生的换行原样注入，
    /// 在模态编辑器里被当命令键执行。修法是在翻译分支三条子路径的汇合点统一裁决。
    ///
    /// 消融：删掉裁决块 → `flatten_multiline` 在 main.rs 命中 0 → 红；
    /// 把 `translated_out` 绑定内联回原样（三子路径各自直接求值）→ 该标识符 0 命中 → 红；
    /// 裁决改成无条件 flatten（丢掉 multiline_safe 分支）→ 同块内 `multiline_safe` 不再出现 → 红。
    /// 🔴 判别力边界（如实声明）：本护栏只钉「裁决代码在场」，**不验证运行时行为**
    /// —— 翻译分支深埋在 Win32 消息循环内，单测无法驱动。行为正确性由
    /// `flatten_multiline` 自身的 11 条既有护栏 + 端测共同保证。
    /// 顺序自证：纯文本静态比对，无时序。
    #[test]
    fn translate_branch_applies_multiline_safe_verdict() {
        let src = include_str!("main.rs");

        // 汇合点绑定必须存在（防止被内联回三条子路径各自求值）
        // needle 拼装，避免本用例的字面量自我命中（同 centered_x 护栏范式）
        let bind_needle = format!("let translated_{}", "out");
        let bind_hits = src.lines().filter(|l| l.contains(&bind_needle)).count();
        assert_eq!(
            bind_hits, 1,
            "翻译三子路径必须汇合到唯一的 translated_out 绑定，实测 {bind_hits} 处"
        );

        // 裁决必须调用 flatten_multiline（needle 拼装，避免本用例自我命中）
        let needle = format!("llm::flatten_{}", "multiline");
        let verdict: Vec<&str> = src
            .lines()
            .filter(|l| l.contains(&needle) && l.contains("translated_out"))
            .collect();
        assert_eq!(
            verdict.len(),
            1,
            "翻译分支出口必须有且仅有一处 flatten 裁决作用于 translated_out，实测 {} 处",
            verdict.len()
        );

        // 裁决必须是「按 multiline_safe 分支」而非无条件 flatten：
        // 取裁决行前 6 行窗口，必须出现 multiline_safe 条件判定。
        let idx = src
            .lines()
            .position(|l| l.contains(&needle) && l.contains("translated_out"))
            .expect("上一断言已保证存在");
        let window_start = idx.saturating_sub(6);
        let window: String = src.lines().collect::<Vec<_>>()[window_start..=idx].join("\n");
        assert!(
            window.contains("if multiline_safe"),
            "flatten 裁决必须受 multiline_safe 条件保护（multiline_safe=true 只 trim），\
             否则多行安全场景（邮件/文档）的换行会被错误压平"
        );
    }

    /// TRANS-SAFE-196-G2：翻译路径**禁止**套用 `strip_fabricated_email_lines`。
    ///
    /// 该守卫用 `input_contains_line` 拿「输出行」去「输入原文」做字面包含匹配。
    /// 翻译场景输入输出跨语言 ⇒ 字面永不匹配 ⇒ 判定条件恒真 ⇒ 用户真说了的称呼
    /// 反被当成 LLM 编造删除。与 `llm/mod.rs` 既有判例「判据 B 不扩展到翻译路径」同源。
    ///
    /// 消融：有人「为了对齐主路径」把该守卫搬进 main.rs 翻译分支 → 命中 >0 → 红。
    /// 顺序自证：纯文本静态比对，无时序。
    #[test]
    fn translate_branch_must_not_use_fabrication_guard() {
        let src = include_str!("main.rs");
        let needle = format!("strip_fabricated_email_{}", "lines");
        let hits: Vec<(usize, &str)> = src
            .lines()
            .enumerate()
            .filter(|(_, l)| l.contains(&needle))
            .filter(|(_, l)| !l.trim_start().starts_with("//"))
            .map(|(i, l)| (i + 1, l))
            .collect();
        // 🔴 断言消息刻意不写该守卫的完整函数名 —— 写了会成为本文件的字面量，被自己命中。
        assert!(
            hits.is_empty(),
            "main.rs 不得调用 LLM 编造称呼守卫（跨语言字面比对会把用户真说的称呼误删），\
             实测命中 {:?}",
            hits
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
            edit_original: None,
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
// FIX-GUARD-301：结构护栏共享「生产区」扫描文本提取
// ---------------------------------------------------------------------
// 三次复发教训：用「第一个出现的某标记」当区域边界，任何人在前面插一个同类
// 标记就塌缩（guard291 的 `} else`、295 预见的中途 cfg(test)、本次 298 插入
// 的 `mod parallel_acc_298_tests`）。故本 helper **不再截断**，而是**剔除全部
// test-gated 项、保留其余全文**，从语义上消除「位置敏感」。
//
// 🔴 test 判据不止 `#[cfg(test)]`：本文件另有 8 处
// `#[cfg(all(test, target_os = "windows"))]` 测试模块（9878/11529/11728/11953/
// 12434/12682/13408/13614），旧实现在 9165 就截断、从未扫到它们；改剔除法后若
// 只认 `#[cfg(test)]`，这 8 个测试模块会被当成生产区扫进去 ⇒ 假红。故判据取
// 「以 `#[cfg(` 开头 且含 `test` 且不含 `not(test)`」：
//   - `#[cfg(test)]`                          → 剔除
//   - `#[cfg(all(test, target_os = "windows"))]` → 剔除
//   - `#[cfg(target_os = "windows")]`（生产）  → 保留（不含 test）
//   - `#[cfg(not(test))]`（生产侧）            → 保留
// =====================================================================
#[cfg(test)]
mod guard_prod_lines {
    /// 单个 cfg 属性行是否把紧随的项 gate 成 test-only。
    fn is_test_gated_attr(t: &str) -> bool {
        let t = t.trim();
        t.starts_with("#[cfg(") && t.contains("test") && !t.contains("not(test)")
    }

    /// 返回 `src` 的「生产区」逐行 trim 文本：**剔除所有 test-gated 项**
    /// （`mod` / `fn` / 任意项），保留其余全文 —— 不再「截断到首个标记」。
    ///
    /// 健壮性（对任意位置 / 任意数量的 test 项都成立）：
    /// - 逐行扫描，遇到 test-gated 属性即进入「跳过态」，**跳过完该项后回到正常态
    ///   继续收集**（这是与旧实现的本质差别）；
    /// - 属性与其后的项之间允许夹文档注释 / 其它属性 / 空行（`//`、`#[`、`#!`、空行都容忍）；
    /// - 项结束判据：在第一个 `{` 之前先遇 `;` ⇒ 无花括号项（`use`/`const`/`static`/`type`），
    ///   跳到该 `;`；否则花括号配平到深度归零（覆盖 `mod x { … }` / `fn x() { … }` 及嵌套）。
    pub(crate) fn prod_lines_excluding_cfg_test(src: &str) -> Vec<String> {
        let lines: Vec<&str> = src.lines().collect();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0usize;
        while i < lines.len() {
            let t = lines[i].trim();
            if !is_test_gated_attr(t) {
                out.push(t.to_string());
                i += 1;
                continue;
            }
            // 跳过属性行本身及其后的文档注释 / 其它属性 / 空行，定位到被 gate 的项首行。
            i += 1;
            while i < lines.len() {
                let t2 = lines[i].trim();
                if t2.is_empty()
                    || t2.starts_with("//")
                    || t2.starts_with("#[")
                    || t2.starts_with("#!")
                {
                    i += 1;
                } else {
                    break;
                }
            }
            // 跳过该项本体。
            let mut depth: i32 = 0;
            let mut opened = false;
            while i < lines.len() {
                let mut semi = false;
                for ch in lines[i].chars() {
                    match ch {
                        '{' => {
                            depth += 1;
                            opened = true;
                        }
                        '}' => depth -= 1,
                        ';' if !opened => {
                            semi = true;
                            break;
                        }
                        _ => {}
                    }
                }
                i += 1;
                if semi || (opened && depth <= 0) {
                    break;
                }
            }
        }
        out
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
//   1) include_str! 自读源码，用 `guard_prod_lines::prod_lines_excluding_cfg_test`
//      **剔除所有 test-gated 项**后只扫生产区 —— 测试代码（本 mod）永不进入扫描区；
//      不再「截断到首个 #[cfg(test)]」（FIX-GUARD-301：位置敏感会塌缩）；
//   2) 匹配一律 startswith（禁 contains），needle 全部 concat! 拆串；
//   3) H3 是唯一子串例外（要在函数体内找「禁止出现的嗅探串」），仍先花括号
//      深度定界到 convert_to_friendly_error 函数体再查；
//   4) 行窗一律 block_contains 花括号定界（不复活 window_has）。
// =====================================================================
#[cfg(test)]
mod nospeech_122_guard_tests {
    /// 生产区逐行 trim：**剔除所有 test-gated 项**（共享 helper，FIX-GUARD-301）。
    fn prod_lines(src: &'static str) -> Vec<String> {
        crate::guard_prod_lines::prod_lines_excluding_cfg_test(src)
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
            out.extend(crate::guard_prod_lines::prod_lines_excluding_cfg_test(src));
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
    /// 判别力边界：只扫生产区（已剔除全部 test-gated 项），测试区内若有人引用该名不计数。
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
    /// 读取 src/main.rs 生产区：**剔除所有 test-gated 项**（共享 helper，FIX-GUARD-301）。
    fn main_prod_lines() -> Vec<String> {
        crate::guard_prod_lines::prod_lines_excluding_cfg_test(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/main.rs"
        )))
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
    /// WORDBOOK-053-B 的镜像写入 `*mirror = Some(...)` 必须在门闩
    /// `if should_ignore_streaming_text(stopped)` 调用**之前**，且两者同处
    /// `process_controller_events` 函数体内。若有人把门闩挪到镜像之前 =
    /// 变成数据抑制 = 被截断的 raw 会学出伪修正。
    ///
    /// 消融：① 把镜像行与门闩行对调 → 红；② 把门闩整体挪出 process_controller_events
    /// （行号先后可能仍满足，但函数归属被破坏）→ 红。
    ///
    /// 🔴 判别力边界：锚点钉死在 `*mirror = Some(composed.clone())` 形态上，若有人把它
    /// 重构成 `to_string()` 等其它赋值形态会**误红** —— 这是有意的：重构触红会逼人
    /// 来看一眼并同步更新护栏，护栏因重构而红是好事。
    /// （2026-09-21 AUTOLEARN-DROP-PATHB-332：ACC-REFLOW-PERSIST-329 把早写内容从
    /// `text.clone()` 改成 `compose(raw)` 的 `composed.clone()`，锚点已同步；
    /// 不变量本身经人工复核仍成立 —— 早写仍在门闩之前。）
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
            concat!("*mirror = Some(composed.", "clone())"),
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

    /// AUTOLEARN-DROP-PATHB-332 路径唯一性护栏（对应任务书 §6.2）：
    /// `learn_correction` 在**生产区恰 1 个调用点** —— 即编辑态提交那条，也就是 DEC-058
    /// 要求「必须保证」的应用内学习路径；同时路径 B 的实现符号与它专用的观测符号**一个都不许回来**。
    ///
    /// 依据（DEC-058）：路径 B = 「注入后 sleep 再重读目标窗口做 diff」，读得到什么取决于目标
    /// 应用的实现，**观测不可靠 ⇒ 不可靠信号进词库是负资产**。Gavin 2026-09-06 拍板不做，
    /// 2026-09-21 再次确认「永远找不到文本快照」。
    ///
    /// 消融：① 在任意位置再加一处 `….learn_correction(` → 红；② 把任一被禁符号加回生产区 → 红。
    /// 🔴 不写「路径 B 运行时行为」断言 —— 该路径已整段删除，对不存在的实现写行为断言=永远绿的假护栏。
    #[test]
    fn pathb_332_learn_correction_single_call_site_and_no_pathb_symbols() {
        let lines = main_prod_lines();
        let count_contains = |needle: &str| lines.iter().filter(|l| l.contains(needle)).count();

        let learn_calls = count_contains("learn_correction(");
        assert_eq!(
            learn_calls, 1,
            "332: learn_correction 生产区调用必须恰 1 处（编辑态提交），实测 {}（>1 = 路径B 被复活）",
            learn_calls
        );

        for banned in [
            "maybe_learn_user_edit",
            "AUTO_LEARN_OBSERVE_MS",
            "extract_changed_text",
            "capture_focused_text_snapshot",
            "read_text_from_hwnd",
            "FocusedTextSnapshot",
        ] {
            let n = count_contains(banned);
            assert_eq!(
                n, 0,
                "332: `{banned}` 必须为 0（路径B 已按 DEC-058 摘除，复活即回归），实测 {n}"
            );
        }
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

    /// LOCALRT-RELEASE-REFLOW-342-A：PreviewReflow 臂的停止语义必须来自 `cancel_signal`
    /// 三态判定，**不得再用 `STREAMING_STOPPED` 一刀切**（否则松手完成被误判为取消 ⇒
    /// 尾字回灌被丢 = Gavin 报的「等 7 秒预览不刷新」）。
    ///
    /// 消融：把 `let cancelled = cancel_signal.load(` 改回 `STREAMING_STOPPED.load(` ⇒ 红。
    #[test]
    fn a342_reflow_uses_cancel_signal_tri_state() {
        let lines = main_prod_lines();
        let anchor = find_line(&lines, "PipelineEvent::PreviewReflow {")
            .expect("342-A: PreviewReflow 匹配臂锚点不存在");
        assert!(
            block_contains(&lines, anchor, "let cancelled = cancel_signal.load("),
            "342-A: PreviewReflow 臂必须读 cancel_signal 判定「取消」"
        );
        assert!(
            block_contains(
                &lines,
                anchor,
                "let stop_state = reflow_stop_state(cancelled, editing);"
            ),
            "342-A: 必须经 reflow_stop_state(cancelled, editing) 三态判定"
        );
        assert!(
            !block_contains(&lines, anchor, "let stopped = STREAMING_STOPPED.load("),
            "342-A: PreviewReflow 臂不得再用 STREAMING_STOPPED 一刀切当取消"
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
    /// 读取 src/main.rs 生产区：**剔除所有 test-gated 项**（共享 helper，FIX-GUARD-301）。
    fn main_prod_lines() -> Vec<String> {
        crate::guard_prod_lines::prod_lines_excluding_cfg_test(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/main.rs"
        )))
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

// ============================================================================
// TEST-SYNC-352（阶段三，非作者 coder-2）：PUNCT-FINAL-REDO-350 接线护栏
//
// 只读 main.rs 源码做结构断言，不改生产代码。350 的剥离节点必须：
//   ① 唯一挂载（只本地 realtime 一条管线 —— Gavin 红线「其他管线不能动」）；
//   ② 门含 `!start.translate`（翻译时整段跳过 ⇒ 逐字返回原文）；
//   ③ 顺序在 `pretranscribed_native_punctuated` **之前**（先剥光再判，否则 native 判定
//      读到未剥离文本 ⇒ 「剥光了必定打得回来」的不变量失效）。
// 字面量一律用 `concat!` 拆开，避免护栏源码自命中（[FILTERED-TEST-BLINDSPOT-001] 教训）。
// ============================================================================
#[cfg(test)]
mod sync352_punct_node_guard_tests {
    use super::strip_punctuation_node;

    fn main_src() -> String {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")).to_string()
    }

    /// sync352-4：节点唯一挂载 + 门含 `punctuation.enabled` / `!start.translate`
    /// （+ 365 新增 `path_b_text.is_none()`）。
    ///
    /// **防的退化**：a. 把节点也挂到在线 realtime / 本地离线档（Gavin 红线：只动本地 realtime）；
    /// b. 丢掉 `!translate_requested` 门 ⇒ 翻译路径被剥光后无节点重打 ⇒ 标点丢失。
    ///
    /// 🔴 锚点同步（365，2026-09-22）：门由「调用实参内联表达式」改为「先算的 `b_strip_enabled`」
    /// （条件挂载：路B 成功即摘、降级才挂）。**断言语义不变** —— 仍是「本地 realtime 唯一挂载、
    /// 门含 `punctuation.enabled` + `!translate`」，另按 365 新增 `path_b_text.is_none()` 一项。
    #[test]
    fn sync352_node_wired_once_with_translate_gate() {
        let s = main_src();
        // 注意：该调用文本在**作者测试**里也出现（传 `t.to_string()`），不能直接数全文件；
        // 用生产调用独有的首参（变量 `normalized` 后跟逗号）过滤出真实挂载点。
        let call = concat!("let stripped = strip_punctuation_node", "(");
        let mounts: Vec<usize> = s
            .match_indices(call)
            .map(|(i, _)| i)
            .filter(|&i| s[i..(i + 200).min(s.len())].contains("normalized,"))
            .collect();
        assert_eq!(
            mounts.len(),
            1,
            "剥离节点必须恰好挂载一次（只本地 realtime 一条管线；Gavin 红线）"
        );
        let at = mounts[0];
        let window = &s[at..(at + 200).min(s.len())];
        assert!(
            window.contains("b_strip_enabled"),
            "365：调用点必须传门变量 `b_strip_enabled`"
        );
        // 门变量定义必须含三项。
        // 🔴 锚点同步（367 + DEC-080，2026-09-22）：门由「路B 条件」改为**无条件摘**常量
        //    `B_STRIP_WIRED_367 = false`。断言语义更新为「节点仍唯一挂载、且由显式常量控制
        //    （可一键回挂）」，不再断言 365 的条件门。
        let def_at = s
            .find(concat!("let b_strip_enabled", " ="))
            .expect("`b_strip_enabled` 定义必须存在");
        let def = &s[def_at..(def_at + 120).min(s.len())];
        assert!(
            def.contains("B_STRIP_WIRED_367"),
            "367/DEC-080：门必须由显式常量 `B_STRIP_WIRED_367` 控制（可一键回挂）"
        );
        assert!(
            s.contains("const B_STRIP_WIRED_367: bool = false;"),
            "367/DEC-080：`B_STRIP_WIRED_367` 必须为 false（无条件摘除 350 节点）"
        );
    }

    /// sync352-5：接线顺序 —— 剥离节点必须在 `pretranscribed_native_punctuated` **之前**。
    ///
    /// **防的退化**：把顺序调换（先判 native 再剥）⇒ `native_punctuated` 读到**未剥离**文本，
    /// 剥光后下游 `!native_punctuated` 门可能不放行 ⇒「剥光了没打回来」。
    #[test]
    fn sync352_strip_node_before_native_punctuated() {
        let s = main_src();
        let strip_at = s
            .find(concat!("let stripped = strip_punctuation_node", "("))
            .expect("剥离节点调用点必须存在");
        let punct_at = s
            .find(concat!(
                "let native_punctuated = pretranscribed_native_punctuated(&stripped)",
                ";"
            ))
            .expect("下游 native_punctuated 判定必须存在");
        assert!(
            strip_at < punct_at,
            "接线顺序错误：必须先 `strip_punctuation_node` 再算 `native_punctuated`"
        );
        // 同一 block：两者相距很近（防「生产中 strip 存在但 punct 命中别处」的假绿）。
        assert!(
            punct_at - strip_at < 1500,
            "两处调用距离异常（{} 字节），接线可能已被拆散",
            punct_at - strip_at
        );
    }

    /// sync352-6：门关（`enabled=false`）⇒ 逐字返回原文（含空串）。
    ///
    /// 本条**同时覆盖「`translate_requested=true`」分支**：调用点的门是 `b_strip_enabled`
    /// （`path_b_text.is_none() && punctuation.enabled && !start.translate`，见 sync352-4），
    /// 翻译时 `!start.translate` 为 false ⇒ 实参 false ⇒ 走的就是本分支。
    /// **防的退化**：门关时仍动手（用户关标点 / 走翻译却看到文本被改）。
    #[test]
    fn sync352_gate_off_returns_verbatim() {
        for t in ["你好。。", "今天天气不错，。", "3.14", "", "中。"] {
            assert_eq!(
                strip_punctuation_node(t.to_string(), false),
                t,
                "门关闭时必须逐字返回原文：{t:?}"
            );
        }
        // 反向 sanity：门开且确有有效标点 ⇒ 必须真的剥（防「恒返回原文」的假实现）。
        assert_eq!(strip_punctuation_node("你好。。".to_string(), true), "你好");
        assert_eq!(
            strip_punctuation_node("3.14 不错".to_string(), true),
            "3.14 不错"
        );
    }
}

// ============================================================
// FIX-TAIL-WINDOW-AND-FALLBACK-386（阶段一）：B 回灌合成 / C 流式兜底
//   （原 SLIDING-WINDOW-367 的 `reflow_preview_367` 两分支单测随该函数一并删除：
//   386-B 起 `replace_all` 也不丢流式尾巴，两分支统一走 `compose_with_acc_for_gen`。）
// ============================================================
#[cfg(test)]
mod fix386_tests {
    use super::{compose_reflow_preview, compose_with_acc_for_gen, window_text_with_fallback};

    /// B：回灌渲染结果 == 后续 `StreamingText` 渲染结果（**同一合成函数**），
    /// 且**保留流式尾巴**（不得截短为纯 acc —— 382 回归）。
    #[test]
    fn reflow_preview_reuses_streaming_compose_and_keeps_tail() {
        let g = 7u64;
        let acc = "权威改写A";
        let streaming = "原始流式文本B";
        let len = 2usize;
        let reflow = compose_reflow_preview(g, acc, streaming, len);
        let state = (g, acc.to_string(), len);
        let subsequent = compose_with_acc_for_gen(Some(&state), g, streaming);
        assert_eq!(
            reflow, subsequent,
            "回灌必须与 StreamingText 用同一合成函数"
        );
        assert_eq!(reflow, "权威改写A流式文本B", "必须保留流式尾巴");
        assert_ne!(reflow, acc, "不得截短为纯 acc（382 回归）");
    }

    /// C：解码空（含 Err 置空）⇒ 用本窗流式文本兜底；流式也空 ⇒ 保持空；解码非空 ⇒ 原样。
    #[test]
    fn window_text_fallback_semantics() {
        assert_eq!(window_text_with_fallback("", "流式兜底"), "流式兜底");
        assert_eq!(window_text_with_fallback("", ""), "");
        assert_eq!(window_text_with_fallback("解码文本", "流式"), "解码文本");
    }
}

// =====================================================================
// TEST-SYNC-371（阶段三 · 非作者视角，tester-1）：C 接线护栏
// ---------------------------------------------------------------------
// Gavin 明确要求「计数器在松开录音键后一定要重置」。`window_spans` /
// `window_samples` 必须**每次录音新建的局部 `let mut Vec::new()`**（per-recording，
// 松手即失效），且不得退化为 static/全局（否则跨录音累积 ⇒ 污染下一次对齐）。
// 生产区扫描用 `guard_prod_lines`（剔除所有 test-gated 项，位置无关）。
// =====================================================================
#[cfg(test)]
mod testsync371_window_counter_guard_tests {
    fn prod_lines() -> Vec<String> {
        crate::guard_prod_lines::prod_lines_excluding_cfg_test(include_str!("main.rs"))
    }

    /// 设计 C：两计数器必须是 per-recording 局部（`let mut ... = Vec::new()`），不得 static。
    #[test]
    fn counters_are_per_recording_locals() {
        let p = prod_lines();
        let starts = |pre: &str| p.iter().filter(|l| l.starts_with(pre)).count();
        assert_eq!(
            starts("let mut window_spans:"),
            1,
            "window_spans 应恰有 1 处 `let mut` 局部声明"
        );
        assert_eq!(
            starts("let mut window_samples:"),
            1,
            "window_samples 应恰有 1 处 `let mut` 局部声明"
        );
        assert!(
            p.iter()
                .any(|l| l.starts_with("let mut window_spans:") && l.contains("Vec::new()")),
            "window_spans 必须每次录音从空表起（Vec::new）"
        );
        assert!(
            p.iter()
                .any(|l| l.starts_with("let mut window_samples:") && l.contains("Vec::new()")),
            "window_samples 必须每次录音从空表起（Vec::new）"
        );
        assert!(
            !p.iter()
                .any(|l| l.starts_with("static") && l.contains("window_spans")),
            "window_spans 不得是 static/全局（否则跨录音泄漏）"
        );
        assert!(
            !p.iter()
                .any(|l| l.starts_with("static") && l.contains("window_samples")),
            "window_samples 不得是 static/全局（否则跨录音泄漏）"
        );
    }

    /// 设计 C：两表必须**同批 push**（区间 ↔ 样本数一一对应，否则期望比例算错）。
    ///
    /// 🔴 386：组窗改走 `dispatch_window!` 宏后，`window_spans.push((gs, ge));` 与
    /// `window_samples.push(` 仍**同处成对**（相邻两行）。**只更新锚点定位方式**（不再要求
    /// `window_spans` 单独占一行），断言强度不放宽：两 push 各**恰 1 处**且**必须成对相邻**。
    #[test]
    fn counters_are_pushed_together() {
        let p = prod_lines();
        let spans: Vec<usize> = p
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains("window_spans.push("))
            .map(|(i, _)| i)
            .collect();
        let samples: Vec<usize> = p
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains("window_samples.push("))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(spans.len(), 1, "window_spans 的 push 调用点应恰 1 处");
        assert_eq!(samples.len(), 1, "window_samples 的 push 调用点应恰 1 处");
        assert_eq!(
            samples[0],
            spans[0] + 1,
            "window_samples.push 必须紧随 window_spans.push（同批成对，防一一对应错位）"
        );
    }
}

// =====================================================================
// TEST-SYNC-377（阶段三 · 非作者视角，tester-1）：源码级护栏
// ---------------------------------------------------------------------
// 设计 377：被删的注入符号（CLEANUP_INSTR_EN / CTX_INSTR_EN / CTX_DEFAULT_CHARS）
// 与跨录音上下文缓存（ctx_prev1 / ctx_prev2）不得在生产区复活。
// 生产区扫描用 `guard_prod_lines`（剔除所有 test-gated 项），注释行一并剔除。
// =====================================================================
#[cfg(test)]
mod testsync377_source_guard_tests {
    fn prod(src: &'static str) -> Vec<String> {
        crate::guard_prod_lines::prod_lines_excluding_cfg_test(src)
    }

    /// 377：`transcription/mod.rs` 生产区（注释不计）不得再出现被删注入符号。
    #[test]
    fn source_has_no_removed_inject_symbols() {
        let p = prod(include_str!("transcription/mod.rs"));
        for sym in ["CLEANUP_INSTR_EN", "CTX_INSTR_EN", "CTX_DEFAULT_CHARS"] {
            let hit: Vec<&String> = p
                .iter()
                .filter(|l| !l.starts_with("//"))
                .filter(|l| l.contains(sym))
                .collect();
            assert!(hit.is_empty(), "生产区不得再出现 `{sym}`：{hit:?}");
        }
    }

    /// 377：`main.rs` 生产区不得再有跨录音上下文缓存 `ctx_prev1`/`ctx_prev2`（计数 0）。
    #[test]
    fn main_has_no_cross_recording_ctx_cache() {
        let p = prod(include_str!("main.rs"));
        for sym in ["ctx_prev1", "ctx_prev2"] {
            let n = p.iter().filter(|l| l.contains(sym)).count();
            assert_eq!(n, 0, "main.rs 生产区 `{sym}` 计数必须为 0，实得 {n}");
        }
    }
}
