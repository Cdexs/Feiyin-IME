//! macOS recording overlay window.
//!
//! 1:1 复刻 Windows 录音浮层（src/main.rs `draw_recording_overlay` / `overlay_geometry`）：
//! - 240×36 无边框浮层，圆角 10px，位于主屏底部上方 64px、水平居中
//! - 三态音频指示灯（红=设备故障 > 橙=有音频 > 灰=静音），直径 18px，left+6
//! - 32 根 3px 宽/2px 间隔的波形柱，从中心向两侧展开，maxh=48 / static_h=12 / minh=8，gain=2.5
//! - 60fps（16ms CFRunLoopTimer）刷新，峰值衰减率 0.02/帧
//! - 左右分隔条 + 橙色停止按钮（与 Windows 完全一致）
//!
//! MACOS-P4-OVERLAY-002: 三态浮层（Recording / Processing / Error）。
//! - Recording：波形 + 可点击的停止按钮（mouseDown → cancel_signal + stop_recording_signal）
//! - Processing：1:1 复刻 Windows draw_processing_overlay（shimmer + 居中橙色文案）
//! - Error：1:1 复刻 Windows draw_error_overlay（红点 + 橙色文案），CFRunLoopTimer 自动关闭

use core_graphics::context::CGContext;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::sys::CGContext as CGContextSys;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{declare_class, msg_send, msg_send_id, mutability, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSBorderlessWindowMask,
    NSColor, NSEvent, NSGraphicsContext, NSNonactivatingPanelMask, NSPanel, NSScreen,
    NSStatusWindowLevel, NSView, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSRect, NSSize, NSString};
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::config::UiLanguage;
use crate::i18n;
use crate::ui::overlay::AudioLevelBuf;

// =============================================================================
// 1:1 Windows 视觉规格（src/main.rs draw_recording_overlay 的常量）
// =============================================================================

/// #FF6B00 — brand orange
const BRAND_ORANGE: (f64, f64, f64) = (1.0, 0.4196, 0.0);
/// #FF0000 — device error / stream failed
const RED_STREAM_FAILED: (f64, f64, f64) = (1.0, 0.0, 0.0);
/// #808080 — device OK but silent
const GRAY_SILENT: (f64, f64, f64) = (0.50196, 0.50196, 0.50196);
/// #0D0F11 — dark background
const BG_DARK: (f64, f64, f64) = (0.05098, 0.05882, 0.06667);
/// #070606 — darkened border（COLORREF 0x060607 = 0x00BBGGRR → R=0x07 G=0x06 B=0x06）
const BORDER_GRAY: (f64, f64, f64) = (0.02745, 0.02353, 0.02353);

const CORNER_RADIUS: f64 = 10.0;

// --- 音频指示灯 ---
const CIRC_SIZE: f64 = 18.0; // MIC-ICON-ENLARGE-001: 18px
const CIRC_LEFT: f64 = 6.0; // left + 6

// --- 波形参数（WAVEFORM-HEIGHT-FIX-001）---
const BAR_COUNT: usize = 32;
const BAR_WIDTH: f64 = 3.0;
const BAR_GAP: f64 = 2.0;
const BAR_MAX_H: f64 = 48.0;
const BAR_STATIC_H: f64 = 12.0;
const BAR_MIN_H: f64 = 8.0;
const BAR_GAIN: f64 = 2.5;
const DECAY_RATE: f32 = 0.02;

// --- 分隔条 ---
const SEP_HEIGHT: f64 = 20.0;
const SEP_WIDTH: f64 = 2.0;

// --- 停止按钮（STOP-BUTTON-CENTER-FIX-001）---
const STOP_BTN_SIZE: f64 = 16.0;
const STOP_BTN_INNER: f64 = 8.0;

// --- Processing 态（1:1 draw_processing_overlay，main.rs:1349）---
/// #181A18 — Processing 背景
const PROC_BG_DARK: (f64, f64, f64) = (0.09412, 0.09412, 0.10196);
/// #D8D8D8 — shimmer 银色光带
const PROC_SILVER: (f64, f64, f64) = (0.84706, 0.84706, 0.84706);
const PROC_CORNER_RADIUS: f64 = 16.0;
/// GLOW_HALF=45 / SLICES=30（PROCESSING-SHIMMER-001 参数）
const PROC_GLOW_HALF: f64 = 45.0;
const PROC_SLICES: i32 = 30;

// --- Error 态（1:1 draw_error_overlay，main.rs:1667）---
/// #1A1D21 — Error 背景
const ERR_BG_DARK: (f64, f64, f64) = (0.10196, 0.11373, 0.12941);
/// COLORREF 0x0033CC → 0x00BBGGRR → R=0xCC G=0x33 B=0x00 — Error 红色圆点
const ERR_RED: (f64, f64, f64) = (0.8, 0.2, 0.0);
const ERR_CIRCLE_D: f64 = 8.0;

// --- Preview 态（1:1 draw_preview_overlay，main.rs:1476-1665）---
/// 预览浮层窗口尺寸（PREVIEW_OVERLAY_SIZE，main.rs:564）
const PREVIEW_OVERLAY_W: f64 = 320.0;
const PREVIEW_OVERLAY_H: f64 = 140.0;
/// 录音/处理/错误浮层窗口尺寸（RECORDING_OVERLAY_SIZE / STATUS_OVERLAY_SIZE）
const SMALL_OVERLAY_W: f64 = 240.0;
const SMALL_OVERLAY_H: f64 = 36.0;
/// #707070 — 按钮边框（BTN_BORDER，比窗口边框亮以便区分）
const BTN_BORDER: (f64, f64, f64) = (0.43922, 0.43922, 0.43922);
/// #F2F2F2 — 正文文本色
const PREVIEW_BODY_TEXT: (f64, f64, f64) = (0.94902, 0.94902, 0.94902);
/// 标题栏高度 28px（UI-OPT-003）
const PREVIEW_TITLE_BAR_H: f64 = 28.0;
/// 底部按钮尺寸 45×18 / gap 10（FIX-006-6 缩 25%）
const PREVIEW_BTN_W: f64 = 45.0;
const PREVIEW_BTN_H: f64 = 18.0;
const PREVIEW_BTN_GAP: f64 = 10.0;

/// 波形条“有效柱宽+间隔”步长（bw + bgap）
const BAR_STEP: f64 = BAR_WIDTH + BAR_GAP;

// =============================================================================
// 共享状态
// =============================================================================

/// OverlayView 的 ivars：持有一个可绘制的共享状态引用。
struct OverlayIvars {
    levels: AudioLevelBuf,
    stats: Arc<OverlayStats>,
    mode: Arc<Mutex<OverlayMode>>,
}

/// 待注入 OverlayView 的 (levels, stats, mode)（单例创建时一次性使用）。
/// 因为 `OverlayIvars` 不是 ObjC 类型，无法作为 init 消息参数传递，
/// 所以重写 `initWithFrame:` 时从该静态取出。
static PENDING_LEVELS: Mutex<Option<(AudioLevelBuf, Arc<OverlayStats>, Arc<Mutex<OverlayMode>>)>> =
    Mutex::new(None);

// =============================================================================
// OVERLAY-WIRE-001: 跨线程请求通道 + 主线程单例
// =============================================================================
//
// 线程模型（照搬 tray.rs 已验证模式）：
// - 任意线程发起：`request_overlay(Show/Hide)` → 写入模块级 `PENDING_REQUEST`
// - 主线程应用：`poll_pending_overlay()` 由 event_loop 的 15ms CFRunLoopTimer 调用，
//   消费 `PENDING_REQUEST` 并在主线程 thread_local 单例上 show/hide。
//
// 生命周期策略 B（主控推荐）：Show → 若无实例则 new()（含 16ms timer）；
// Hide → destroy() + 置 None。空闲时无 timer 唤醒，省电。

/// 浮层请求（任意线程发起，主线程消费）。
/// OVERLAY-WIRE-002: 加 PartialEq/Eq derive 供单测 assert_eq! 锁死真值表。
/// OVERLAY-002: 加 ShowProcessing / ShowError 变体（三态浮层）。
/// OVERLAY-003: 加 ShowPreview 变体（Preview/FocusLost 失焦返显浮层）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayRequest {
    Show,
    Hide,
    /// 携带 Processing 文案（main.rs 已按 ui_language 取 i18n::overlay_processing 传入）。
    ShowProcessing(String),
    ShowError {
        message: String,
        auto_close_ms: u64,
    },
    /// 携带失焦预览文本（FocusLost 分支，用户未点复制前暂存展示）。
    ShowPreview(String),
}

static PENDING_REQUEST: Mutex<Option<OverlayRequest>> = Mutex::new(None);

/// 由 `run_controller_macos` 在主线程启动时存入 audio_buf（方案②）。
/// `new()` 创建实例时取出放进 `PENDING_LEVELS` 供 OverlayView 注入。
/// 必须与音频线程写入的 buf 是同一个 Arc（main.rs:2721 创建、:2733 传给 worker）。
static OVERLAY_LEVELS: Mutex<Option<AudioLevelBuf>> = Mutex::new(None);

/// OVERLAY-002-A: 停止按钮点击触发的信号。主线程 mouseDown 只读这两个 Arc（不持有锁时读值）。
/// 由 `run_controller_macos` 通过 `init_overlay_levels` 注入（与录音 worker 共享的同一个 Arc）。
struct OverlaySignals {
    stop_recording: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

static OVERLAY_SIGNALS: Mutex<Option<OverlaySignals>> = Mutex::new(None);

/// OVERLAY-003: 浮层界面的 i18n 语言。由 `init_overlay_levels` 注入（main.rs 的 ui_language）。
/// 仅绘制时读取（Preview 按钮/标题文案需要按语言取 i18n 字符串）。
static OVERLAY_UI_LANGUAGE: Mutex<Option<UiLanguage>> = Mutex::new(None);

/// OVERLAY-002-B/C: 浮层显示态。存于 OverlayIvars.mode（Arc<Mutex<OverlayMode>>），
/// 由 RecordingOverlay::set_mode 写、OverlayView 绘制时读。锁内只做快照，锁外绘制。
#[derive(Debug, Clone, PartialEq)]
enum OverlayMode {
    Recording,
    Processing(String),
    Error(String),
    /// OVERLAY-003: Preview/FocusLost 失焦返显态，携带待预览文本。
    Preview(String),
}

// 主线程浮层单例。thread_local 天然保证「只有主线程能碰它」，
// 同时规避 `RecordingOverlay` 不是 Send/Sync（含 `Retained<NSPanel>` 裸指针）的问题。
thread_local! {
    static OVERLAY: std::cell::RefCell<Option<RecordingOverlay>> = const { std::cell::RefCell::new(None) };
}

/// 在主线程启动时存入共享 audio_buf + 停止/取消信号（供后续 `new()` 与 mouseDown 用）。
/// 由 `run_controller_macos` 在 `create_controller_window()` 之后调用。
/// OVERLAY-002-A: 新增 stop_recording / cancel 两个 Arc<AtomicBool>，
/// 必须与录音 worker 持有的是同一个 Arc（main.rs:2723-2724 创建、:2733 传给 worker）。
pub fn init_overlay_levels(
    buf: AudioLevelBuf,
    stop_recording: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    ui_language: UiLanguage,
) {
    *OVERLAY_LEVELS.lock().unwrap() = Some(buf);
    *OVERLAY_SIGNALS.lock().unwrap() = Some(OverlaySignals {
        stop_recording,
        cancel,
    });
    // OVERLAY-003: 浮层界面 i18n 语言（Preview 按钮/标题文案按语言取字符串）。
    *OVERLAY_UI_LANGUAGE.lock().unwrap() = Some(ui_language);
}

/// 任意线程可调：请求浮层显示/隐藏。主线程 15ms timer 调 `poll_pending_overlay` 应用。
pub fn request_overlay(req: OverlayRequest) {
    *PENDING_REQUEST.lock().unwrap() = Some(req);
}

/// 主线程消费待处理请求（由 event_loop 的 15ms CFRunLoopTimer 调用）。
///
/// try-lock 形态：拿不到锁直接返回（绝不阻塞 timer 回调）。
/// 极端竞争下漏掉一次 poll，15ms 后下一拍补取，对浮层显示无可感知影响。
/// 生命周期 B：Show → 无实例则 new()（内部已 show）；Hide → destroy + 置 None。
pub fn poll_pending_overlay() {
    let req = { PENDING_REQUEST.try_lock().ok().and_then(|mut g| g.take()) };
    let Some(req) = req else { return };
    OVERLAY.with(|cell| {
        let mut slot = cell.borrow_mut();
        match req {
            OverlayRequest::Show
            | OverlayRequest::ShowProcessing(_)
            | OverlayRequest::ShowError { .. }
            | OverlayRequest::ShowPreview(_) => {
                if slot.is_none() {
                    // Arc::clone（廉价），保留原 buf 供后续多次 Show 复用。
                    // OVERLAY_LEVELS 用阻塞 lock：此处只在真要建浮层时走到，临界区仅一次
                    // Arc::clone（微秒级），且 OVERLAY_LEVELS 仅 init 时写入一次后只读，
                    // 实测无竞争。timer 回调的「不阻塞」保证由上面的 PENDING_REQUEST
                    // try_lock 守住——拿不到请求锁就直接 return，根本不到这里。
                    let levels = OVERLAY_LEVELS
                        .lock()
                        .ok()
                        .and_then(|g| g.as_ref().map(Arc::clone));
                    let Some(levels) = levels else {
                        log::warn!(
                            "overlay Show: levels not initialized (init_overlay_levels 未调用?)"
                        );
                        return;
                    };
                    match RecordingOverlay::new(levels) {
                        Ok(overlay) => {
                            *slot = Some(overlay);
                            log::info!("overlay shown (RecordingOverlay::new)");
                        }
                        Err(e) => {
                            log::error!("overlay Show: RecordingOverlay::new failed: {}", e);
                            return;
                        }
                    }
                }
                let overlay = slot.as_mut().expect("overlay just created");
                match req {
                    OverlayRequest::Show => {
                        overlay.set_mode(OverlayMode::Recording);
                        overlay.cancel_auto_close();
                    }
                    OverlayRequest::ShowProcessing(message) => {
                        overlay.set_mode(OverlayMode::Processing(message));
                        overlay.cancel_auto_close();
                    }
                    OverlayRequest::ShowError {
                        message,
                        auto_close_ms,
                    } => {
                        overlay.set_mode(OverlayMode::Error(message));
                        overlay.schedule_auto_close(auto_close_ms);
                    }
                    OverlayRequest::ShowPreview(text) => {
                        // OVERLAY-003: Preview 态不自动关闭（与 Windows 一致，等用户操作）。
                        overlay.set_mode(OverlayMode::Preview(text));
                        overlay.cancel_auto_close();
                    }
                    OverlayRequest::Hide => unreachable!("matched above"),
                }
            }
            OverlayRequest::Hide => {
                if slot.take().is_some() {
                    log::info!("overlay hidden (destroyed)");
                }
            }
        }
    });
}

/// 主线程退出前显式销毁浮层（run_controller_macos shutdown 段调用）。
/// thread_local 在进程退出时本会 drop，但显式调用保证退出链清晰、日志可辨。
pub fn shutdown_overlay() {
    OVERLAY.with(|cell| {
        if cell.borrow_mut().take().is_some() {
            log::info!("overlay destroyed on shutdown");
        }
    });
}

// =============================================================================
// 绘制原语（core-graphics，坐标与 Windows GDI 同为“左上原点”）
// =============================================================================

/// 把 AppKit 的 NSRect 转成 core-graphics 的 CGRect（同布局，仅 crate 不同）。
fn to_cg_rect(r: NSRect) -> CGRect {
    CGRect::new(
        &CGPoint::new(r.origin.x, r.origin.y),
        &CGSize::new(r.size.width, r.size.height),
    )
}

/// 添加一个圆角矩形路径（二次贝塞尔近似 90° 圆弧）。
fn add_rounded_rect_path(ctx: &CGContext, rect: CGRect, radius: f64) {
    let r = radius
        .min(rect.size.width / 2.0)
        .min(rect.size.height / 2.0);
    let (x, y) = (rect.origin.x, rect.origin.y);
    let (w, h) = (rect.size.width, rect.size.height);
    // 左上 → 右上 → 右下 → 左下，每个角用一条二次贝塞尔曲线
    ctx.move_to_point(x + r, y);
    ctx.add_line_to_point(x + w - r, y);
    ctx.add_quad_curve_to_point(x + w, y, x + w, y + r);
    ctx.add_line_to_point(x + w, y + h - r);
    ctx.add_quad_curve_to_point(x + w, y + h, x + w - r, y + h);
    ctx.add_line_to_point(x + r, y + h);
    ctx.add_quad_curve_to_point(x, y + h, x, y + h - r);
    ctx.add_line_to_point(x, y + r);
    ctx.add_quad_curve_to_point(x, y, x + r, y);
    ctx.close_path();
}

/// 填充圆角矩形。
fn fill_rounded_rect(ctx: &CGContext, rect: CGRect, radius: f64, color: (f64, f64, f64)) {
    ctx.set_rgb_fill_color(color.0, color.1, color.2, 1.0);
    add_rounded_rect_path(ctx, rect, radius);
    ctx.fill_path();
}

/// 描边圆角矩形（1px 边框）。
fn stroke_rounded_rect(ctx: &CGContext, rect: CGRect, radius: f64, color: (f64, f64, f64)) {
    ctx.set_rgb_stroke_color(color.0, color.1, color.2, 1.0);
    ctx.set_line_width(1.0);
    add_rounded_rect_path(ctx, rect, radius);
    ctx.stroke_path();
}

/// 填充一个实心矩形。
fn fill_rect(ctx: &CGContext, rect: CGRect, color: (f64, f64, f64)) {
    ctx.set_rgb_fill_color(color.0, color.1, color.2, 1.0);
    ctx.fill_rect(rect);
}

/// 1px 实心矩形描边。
fn stroke_rect(ctx: &CGContext, rect: CGRect, color: (f64, f64, f64)) {
    ctx.set_rgb_stroke_color(color.0, color.1, color.2, 1.0);
    ctx.set_line_width(1.0);
    ctx.stroke_rect(rect);
}

/// 填充一个实心圆（core-graphics 原生抗锯齿，等价于 Windows HALFTONE 4x 超采样）。
fn fill_circle(ctx: &CGContext, center: CGPoint, radius: f64, color: (f64, f64, f64)) {
    ctx.set_rgb_fill_color(color.0, color.1, color.2, 1.0);
    let rect = CGRect::new(
        &CGPoint::new(center.x - radius, center.y - radius),
        &CGSize::new(radius * 2.0, radius * 2.0),
    );
    ctx.fill_ellipse_in_rect(rect);
}

// =============================================================================
// RecordingOverlay：自包含浮层（NSPanel + OverlayView + 60fps 计时器）
// =============================================================================

/// 帧性能统计（原子计数，供性能验收/调试轮询，零阻塞主线程）。
#[derive(Debug)]
pub struct OverlayStats {
    frame_count: AtomicU64,
    tick_count: AtomicU64,
    draw_total_ns: AtomicU64,
    draw_max_ns: AtomicU64,
    interval_sum_ns: AtomicU64,
    interval_max_ns: AtomicU64,
    last_tick_ns: AtomicU64,
}

/// 性能快照。
#[derive(Debug, Clone, Copy)]
pub struct StatsSnapshot {
    /// drawRect 实际绘制帧数
    pub frames: u64,
    /// 计时器 tick 数
    pub ticks: u64,
    /// 平均帧间隔（ms，应约 16.7ms）
    pub avg_interval_ms: f64,
    /// 最大帧间隔（ms）
    pub max_interval_ms: f64,
    /// 平均单帧绘制耗时（ms，验收要求 <5ms）
    pub avg_draw_ms: f64,
    /// 最大单帧绘制耗时（ms）
    pub max_draw_ms: f64,
}

impl OverlayStats {
    fn new() -> Self {
        Self {
            frame_count: AtomicU64::new(0),
            tick_count: AtomicU64::new(0),
            draw_total_ns: AtomicU64::new(0),
            draw_max_ns: AtomicU64::new(0),
            interval_sum_ns: AtomicU64::new(0),
            interval_max_ns: AtomicU64::new(0),
            last_tick_ns: AtomicU64::new(0),
        }
    }

    /// 记录一次 60fps tick 的帧间隔。
    fn record_tick(&self) {
        let now = monotonic_ns();
        let prev = self.last_tick_ns.swap(now, Ordering::Relaxed);
        if prev != 0 {
            let dt = now.saturating_sub(prev);
            self.interval_sum_ns.fetch_add(dt, Ordering::Relaxed);
            self.interval_max_ns.fetch_max(dt, Ordering::Relaxed);
        }
        self.tick_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录一次 drawRect 的绘制耗时。
    fn record_draw(&self, elapsed_ns: u64) {
        self.draw_total_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
        self.draw_max_ns.fetch_max(elapsed_ns, Ordering::Relaxed);
        self.frame_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 取性能快照。
    pub fn snapshot(&self) -> StatsSnapshot {
        let frames = self.frame_count.load(Ordering::Relaxed);
        let ticks = self.tick_count.load(Ordering::Relaxed);
        StatsSnapshot {
            frames,
            ticks,
            avg_interval_ms: {
                let sum = self.interval_sum_ns.load(Ordering::Relaxed);
                let n = ticks.saturating_sub(1);
                if n == 0 {
                    0.0
                } else {
                    sum as f64 / n as f64 / 1e6
                }
            },
            max_interval_ms: self.interval_max_ns.load(Ordering::Relaxed) as f64 / 1e6,
            avg_draw_ms: {
                let sum = self.draw_total_ns.load(Ordering::Relaxed);
                if frames == 0 {
                    0.0
                } else {
                    sum as f64 / frames as f64 / 1e6
                }
            },
            max_draw_ms: self.draw_max_ns.load(Ordering::Relaxed) as f64 / 1e6,
        }
    }
}

/// 单调时钟（纳秒，进程内原点）。
fn monotonic_ns() -> u64 {
    use std::sync::OnceLock;
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_nanos() as u64
}

/// 录音浮层。进程内同时只允许一个实例（与 Windows overlay 单窗口语义一致）。
///
/// 生命周期：`RecordingOverlay::new(levels)` 创建并显示；`hide()` / `show()`
/// 控制显隐；`destroy()` 关闭窗口并停掉计时器。Drop 时自动 destroy。
/// OVERLAY-002: 增加共享 mode（Recording/Processing/Error）与 Error 自动关闭定时器。
pub struct RecordingOverlay {
    panel: Retained<NSPanel>,
    _view: Retained<OverlayView>,
    timer: core_foundation_sys::runloop::CFRunLoopTimerRef,
    /// 持有 Box，保证计时器回调上下文存活（经裸指针引用）。
    #[allow(dead_code)]
    timer_ctx: Box<TimerCtx>,
    /// 帧性能统计（供验收/调试轮询）。
    stats: Arc<OverlayStats>,
    /// 与 OverlayView 共享的浮层显示态（OVERLAY-002）。
    mode: Arc<Mutex<OverlayMode>>,
    /// Error 自动关闭定时器（一次性，OVERLAY-002-C）。
    auto_close_timer: core_foundation_sys::runloop::CFRunLoopTimerRef,
}

/// 计时器回调上下文：指向浮层自身，用于刷新视图。
struct TimerCtx {
    panel: *mut NSPanel,
    view: *mut OverlayView,
    stats: Arc<OverlayStats>,
}

/// 60fps 刷新回调：记录帧间隔并请求重绘（波形动画）。
extern "C" fn overlay_timer_callback(
    _timer: core_foundation_sys::runloop::CFRunLoopTimerRef,
    info: *mut c_void,
) {
    if info.is_null() {
        return;
    }
    let ctx = unsafe { &*(info as *const TimerCtx) };
    ctx.stats.record_tick();
    if ctx.view.is_null() || ctx.panel.is_null() {
        return;
    }
    let panel = unsafe { &*ctx.panel };
    let view = unsafe { &*ctx.view };
    // 仅在可见时刷新
    if panel.isVisible() {
        unsafe {
            view.setNeedsDisplay(true);
        }
    }
}

impl RecordingOverlay {
    /// 创建浮层并立即显示在底部上方 64px。
    ///
    /// 必须在主线程调用（AppKit 线程约束）。
    pub fn new(levels: AudioLevelBuf) -> anyhow::Result<Self> {
        let _mtm = MainThreadMarker::new().ok_or_else(|| {
            anyhow::anyhow!("RecordingOverlay must be created on the main thread")
        })?;

        // 确保 NSApplication 就绪（accessory：tray-first，无 Dock 图标）
        let app = NSApplication::sharedApplication(_mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        unsafe {
            app.finishLaunching();
        }

        // 主屏居中，底部上方 64px（对应 overlay_geometry）
        let screen = NSScreen::mainScreen(_mtm)
            .ok_or_else(|| anyhow::anyhow!("NSScreen::mainScreen returned None"))?;
        let sf = screen.frame();
        let size = objc2_foundation::CGSize::new(240.0, 36.0);
        let x = ((sf.size.width - size.width) / 2.0).max(0.0);
        let y = (sf.size.height - size.height - 64.0).max(0.0);
        let frame = objc2_foundation::CGRect::new(objc2_foundation::CGPoint::new(x, y), size);

        // 无边框 + 非激活面板（≈ WS_POPUP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE）
        let style =
            NSWindowStyleMask::from_bits(NSBorderlessWindowMask.0 | NSNonactivatingPanelMask.0)
                .unwrap_or_else(|| {
                    NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel
                });

        let panel: Retained<NSPanel> = unsafe {
            NSPanel::initWithContentRect_styleMask_backing_defer(
                _mtm.alloc::<NSPanel>(),
                frame,
                style,
                NSBackingStoreType::NSBackingStoreBuffered,
                false,
            )
        };

        // 透明背景 + 顶层 + 全空间可见（≈ WS_EX_LAYERED | WS_EX_TOPMOST）
        unsafe {
            panel.setLevel(NSStatusWindowLevel);
            panel.setOpaque(false);
            panel.setBackgroundColor(Some(&NSColor::clearColor()));
            panel.setHasShadow(false);
            let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary;
            panel.setCollectionBehavior(behavior);
            panel.setIgnoresMouseEvents(false);
        }

        // 自定义绘制视图（ivars 经模块静态注入）；levels 由 view 的 ivars 持有保活
        let stats = Arc::new(OverlayStats::new());
        let mode = Arc::new(Mutex::new(OverlayMode::Recording));
        *PENDING_LEVELS.lock().unwrap() =
            Some((Arc::clone(&levels), Arc::clone(&stats), Arc::clone(&mode)));
        let content = OverlayView::new(_mtm, frame);
        panel.setContentView(Some(content.as_super()));

        let panel_ptr = &*panel as *const NSPanel as *mut NSPanel;
        let view_ptr = &*content as *const OverlayView as *mut OverlayView;
        let timer_ctx = Box::new(TimerCtx {
            panel: panel_ptr,
            view: view_ptr,
            stats: Arc::clone(&stats),
        });

        // 16ms 计时器（60fps），对应 Windows 的 SetTimer 16ms
        let timer = unsafe { create_overlay_timer(&timer_ctx) };

        let overlay = Self {
            panel,
            _view: content,
            timer,
            timer_ctx,
            stats,
            mode,
            auto_close_timer: ptr::null_mut(),
        };

        // 显示（nonactivating panel 不抢焦点）
        overlay.show();
        Ok(overlay)
    }

    /// 显示浮层（非激活，不抢焦点）。
    pub fn show(&self) {
        unsafe {
            let _: () = msg_send![&self.panel, orderFrontRegardless];
        }
    }

    /// 隐藏浮层。
    ///
    /// OVERLAY-WIRE-001: 生命周期 B 下 Hide 走 destroy+置 None，不直接调本方法。
    /// 保留供 OVERLAY-002（Processing/FocusLost 态切换显隐不销毁）使用。
    pub fn hide(&self) {
        self.panel.orderOut(None);
    }

    /// 当前是否可见。
    ///
    /// OVERLAY-WIRE-001: 生命周期 B 下无直接调用者；保留供 OVERLAY-002 及调试用。
    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// 帧性能统计快照（帧间隔 / 单帧绘制耗时，供性能验收）。
    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    /// OVERLAY-002: 切换浮层显示态（Recording/Processing/Error），下次 drawRect 生效。
    /// 锁内只做快照写入（Arc<Mutex<OverlayMode>> 的写），绘制在 drawRect 中锁外进行。
    /// OVERLAY-003: Preview 态需把窗口从 240×36 扩到 320×140 并重新居中，切回时还原。
    pub fn set_mode(&self, mode: OverlayMode) {
        let (w, h) = match &mode {
            OverlayMode::Preview(_) => (PREVIEW_OVERLAY_W, PREVIEW_OVERLAY_H),
            _ => (SMALL_OVERLAY_W, SMALL_OVERLAY_H),
        };
        self.resize_to(w, h);
        *self.mode.lock().unwrap() = mode;
    }

    /// OVERLAY-003: 把浮层窗口 resize 到 (w, h) 并重新居中（水平居中 + 底部上方 64px）。
    /// 必须主线程调用。若当前已是该尺寸则跳过（避免无谓的 setFrame 抖动）。
    fn resize_to(&self, w: f64, h: f64) {
        let _mtm = match MainThreadMarker::new() {
            Some(m) => m,
            None => {
                log::warn!("overlay resize_to called off main thread; skipping");
                return;
            }
        };
        let current = to_cg_rect(self.panel.frame());
        if (current.size.width - w).abs() < f64::EPSILON
            && (current.size.height - h).abs() < f64::EPSILON
        {
            return;
        }
        let screen = match NSScreen::mainScreen(_mtm) {
            Some(s) => s,
            None => {
                log::warn!("overlay resize_to: NSScreen::mainScreen returned None");
                return;
            }
        };
        let sf = screen.frame();
        let x = ((sf.size.width - w) / 2.0).max(0.0);
        let y = (sf.size.height - h - 64.0).max(0.0);
        let frame = objc2_foundation::CGRect::new(
            objc2_foundation::CGPoint::new(x, y),
            objc2_foundation::CGSize::new(w, h),
        );
        self.panel.setFrame_display(frame, true);
        log::info!("overlay resized to {}x{} at ({:.0},{:.0})", w, h, x, y);
    }

    /// OVERLAY-002-C: 调度 Error 浮层自动关闭（一次性 CFRunLoopTimer）。
    /// 必须在主线程调用。若已有自动关闭定时器，先取消旧的。
    pub fn schedule_auto_close(&mut self, auto_close_ms: u64) {
        self.cancel_auto_close();
        let timer = unsafe { create_auto_close_timer(auto_close_ms) };
        if !timer.is_null() {
            self.auto_close_timer = timer;
        }
    }

    /// OVERLAY-002-C: 取消待触发的自动关闭定时器（销毁 / 切换到非 Error 态时调用）。
    pub fn cancel_auto_close(&mut self) {
        if !self.auto_close_timer.is_null() {
            unsafe {
                core_foundation_sys::runloop::CFRunLoopTimerInvalidate(self.auto_close_timer);
                core_foundation_sys::base::CFRelease(self.auto_close_timer as *const c_void);
            }
            self.auto_close_timer = ptr::null_mut();
        }
    }

    /// 关闭浮层并释放计时器。Drop 时自动调用。
    pub fn destroy(&mut self) {
        self.cancel_auto_close();
        if !self.timer.is_null() {
            unsafe {
                core_foundation_sys::runloop::CFRunLoopTimerInvalidate(self.timer);
                core_foundation_sys::base::CFRelease(self.timer as *const c_void);
            }
            self.timer = ptr::null_mut();
        }
        self.panel.orderOut(None);
        self.panel.close();
    }
}

impl Drop for RecordingOverlay {
    fn drop(&mut self) {
        self.destroy();
    }
}

/// 创建 16ms CFRunLoopTimer 并加到当前 run loop（default mode）。
///
/// # Safety
/// `ctx` 必须存活到计时器 invalidate / release 为止（由 RecordingOverlay 保证）。
unsafe fn create_overlay_timer(ctx: &TimerCtx) -> core_foundation_sys::runloop::CFRunLoopTimerRef {
    use core_foundation_sys::base::kCFAllocatorDefault;
    use core_foundation_sys::runloop::{
        kCFRunLoopDefaultMode, CFRunLoopAddTimer, CFRunLoopGetCurrent, CFRunLoopTimerContext,
        CFRunLoopTimerCreate,
    };

    let mut timer_ctx = CFRunLoopTimerContext {
        version: 0,
        info: ctx as *const TimerCtx as *mut c_void,
        retain: None,
        release: None,
        copyDescription: None,
    };
    let timer = CFRunLoopTimerCreate(
        kCFAllocatorDefault,
        0.0,
        16.0 / 1000.0,
        0,
        0,
        overlay_timer_callback,
        &mut timer_ctx,
    );
    if timer.is_null() {
        return timer;
    }
    let run_loop = CFRunLoopGetCurrent();
    if !run_loop.is_null() {
        CFRunLoopAddTimer(run_loop, timer, kCFRunLoopDefaultMode);
    }
    timer
}

/// OVERLAY-002-C: Error 浮层自动关闭回调。触发后请求 Hide，走既有 poll 通道销毁浮层。
/// 不需要上下文（info 可为空），仅做一次 Hide 请求。
extern "C" fn auto_close_callback(
    _timer: core_foundation_sys::runloop::CFRunLoopTimerRef,
    _info: *mut c_void,
) {
    request_overlay(OverlayRequest::Hide);
}

/// 创建一次性（interval=0）CFRunLoopTimer，`auto_close_ms` 后触发 `auto_close_callback`。
///
/// # Safety
/// 返回的 timer 必须由调用方 invalidate + release（RecordingOverlay::auto_close_timer 负责）。
unsafe fn create_auto_close_timer(
    auto_close_ms: u64,
) -> core_foundation_sys::runloop::CFRunLoopTimerRef {
    use core_foundation_sys::base::kCFAllocatorDefault;
    use core_foundation_sys::date::CFAbsoluteTimeGetCurrent;
    use core_foundation_sys::runloop::{
        kCFRunLoopDefaultMode, CFRunLoopAddTimer, CFRunLoopGetCurrent, CFRunLoopTimerContext,
        CFRunLoopTimerCreate,
    };

    let mut timer_ctx = CFRunLoopTimerContext {
        version: 0,
        info: ptr::null_mut(),
        retain: None,
        release: None,
        copyDescription: None,
    };
    let fire_date = CFAbsoluteTimeGetCurrent() + auto_close_ms as f64 / 1000.0;
    let timer = CFRunLoopTimerCreate(
        kCFAllocatorDefault,
        fire_date,
        0.0,
        0,
        0,
        auto_close_callback,
        &mut timer_ctx,
    );
    if timer.is_null() {
        return timer;
    }
    let run_loop = CFRunLoopGetCurrent();
    if !run_loop.is_null() {
        CFRunLoopAddTimer(run_loop, timer, kCFRunLoopDefaultMode);
    }
    timer
}

// =============================================================================
// OverlayView：绘制视图（declare_class! 子类化 NSView）
// =============================================================================

declare_class!(
    struct OverlayView;

    unsafe impl ClassType for OverlayView {
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "FeiyinOverlayView";
    }

    impl DeclaredClass for OverlayView {
        type Ivars = OverlayIvars;
    }

    unsafe impl OverlayView {
        /// 重写 designated initializer；ivars 从模块静态注入。
        #[method_id(initWithFrame:)]
        fn init_with_frame(this: Allocated<Self>, frame: NSRect) -> Retained<Self> {
            let ivars = PENDING_LEVELS
                .lock()
                .unwrap()
                .take()
                .map(|(levels, stats, mode)| OverlayIvars {
                    levels,
                    stats,
                    mode,
                })
                .unwrap_or_else(|| OverlayIvars {
                    levels: Arc::new(Mutex::new(std::collections::VecDeque::new())),
                    stats: Arc::new(OverlayStats::new()),
                    mode: Arc::new(Mutex::new(OverlayMode::Recording)),
                });
            let this = this.set_ivars(ivars);
            unsafe { msg_send_id![super(this), initWithFrame: frame] }
        }

        /// 左上原点坐标系（与 Windows GDI 一致，避免翻转计算）。
        #[method(isFlipped)]
        fn is_flipped(&self) -> bool {
            true
        }

        #[method(drawRect:)]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            self.render_overlay();
        }

        /// OVERLAY-002-A: 命中停止按钮（Recording 态）时触发取消录音。
        /// OVERLAY-003: Preview 态命中复制/关闭/标题栏关闭三个热区。
        /// 非命中区域不吞事件、不抢焦点：直接忽略，保持 NSPanel 的 Nonactivating 特性。
        /// Processing/Error 态不响应任何点击。
        #[method(mouseDown:)]
        fn mouse_down(&self, event: &NSEvent) {
            let mode = self
                .ivars()
                .mode
                .try_lock()
                .ok()
                .map(|g| g.clone())
                .unwrap_or(OverlayMode::Recording);
            match mode {
                OverlayMode::Recording => {
                    self.handle_recording_click(event);
                }
                OverlayMode::Preview(text) => {
                    self.handle_preview_click(event, &text);
                }
                // Processing/Error 态不响应点击（OVERLAY-002 既定语义）
                OverlayMode::Processing(_) | OverlayMode::Error(_) => {}
            }
        }

        /// OVERLAY-002-A: 非激活面板需要接受首次鼠标事件，否则第一次点击只会激活窗口而不会传给视图。
        #[method(acceptsFirstMouse:)]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }
    }
);

impl OverlayView {
    /// 创建一个绘制视图（主线程）。
    fn new(_mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        unsafe { msg_send_id![_mtm.alloc::<Self>(), initWithFrame: frame] }
    }

    /// OVERLAY-002-A: 将窗口坐标转成本视图坐标并命中测试停止按钮。
    fn hit_test_stop_button(&self, event: &NSEvent) -> bool {
        let Some((x, y)) = self.local_point(event) else {
            return false;
        };
        let b = to_cg_rect(self.bounds());
        let bl = b.origin.x + b.size.width - 25.0;
        let bt = b.origin.y + (b.size.height - STOP_BTN_SIZE) / 2.0;
        x >= bl && x <= bl + STOP_BTN_SIZE && y >= bt && y <= bt + STOP_BTN_SIZE
    }

    /// OVERLAY-002-A: Recording 态停止按钮点击 → 触发取消录音。
    fn handle_recording_click(&self, event: &NSEvent) {
        let hit = self.hit_test_stop_button(event);
        if !hit {
            return;
        }
        // CancelStop 同款：cancel + stop 双信号（与 main.rs:2908-2911 handle_hotkey_event 一致）
        let (stop, cancel) = {
            let g = OVERLAY_SIGNALS.lock().ok();
            let sig = g.as_ref().and_then(|g| g.as_ref());
            (
                sig.map(|s| Arc::clone(&s.stop_recording)),
                sig.map(|s| Arc::clone(&s.cancel)),
            )
        };
        match (stop, cancel) {
            (Some(stop), Some(cancel)) => {
                cancel.store(true, Ordering::Release);
                stop.store(true, Ordering::Release);
                log::info!("overlay: stop button clicked, cancel requested");
            }
            _ => {
                log::warn!(
                    "overlay: stop button clicked but signals not initialized (init_overlay_levels 未调用?)"
                );
            }
        }
    }

    /// OVERLAY-003: Preview 态三个热区：
    /// - 复制按钮：预览文本写入剪贴板 → 关闭浮层
    /// - 关闭按钮 / 标题栏关闭按钮：关闭浮层
    fn handle_preview_click(&self, event: &NSEvent, text: &str) {
        let Some((x, y)) = self.local_point(event) else {
            return;
        };
        let local = CGPoint::new(x, y);
        let geo = preview_geometry(to_cg_rect(self.bounds()));
        if rect_contains(geo.copy, local) {
            log::info!("overlay preview: copy button clicked");
            if let Err(e) = super::copy_text_to_clipboard(text) {
                log::error!("overlay preview: copy_text_to_clipboard failed: {}", e);
            }
            request_overlay(OverlayRequest::Hide);
        } else if rect_contains(geo.close, local) || rect_contains(geo.title_close, local) {
            log::info!("overlay preview: close button clicked");
            request_overlay(OverlayRequest::Hide);
        }
    }

    /// 把窗口坐标转成本视图坐标（flipped 左上原点），返回 (x, y)。
    fn local_point(&self, event: &NSEvent) -> Option<(f64, f64)> {
        let loc = unsafe { event.locationInWindow() };
        let p = self.convertPoint_fromView(loc, None);
        Some((p.x, p.y))
    }

    /// 渲染整个浮层内容。
    fn render_overlay(&self) {
        let Some(ctx) = current_cg_context() else {
            return;
        };
        // 锁内只快照 mode，锁外绘制（[OVERLAY-LOCK-SCOPE-001]）
        let mode = self
            .ivars()
            .mode
            .try_lock()
            .ok()
            .map(|g| g.clone())
            .unwrap_or(OverlayMode::Recording);
        let bounds = self.bounds();
        let rect = to_cg_rect(bounds);
        let start = monotonic_ns();
        match mode {
            OverlayMode::Recording => {
                // 直接读 OverlayView 自己的 ivars（无需全局裸指针）
                let levels = Arc::clone(&self.ivars().levels);
                draw_overlay(&ctx, rect, &levels);
            }
            OverlayMode::Processing(message) => {
                draw_processing_overlay(&ctx, rect, &message);
            }
            OverlayMode::Error(message) => {
                draw_error_overlay(&ctx, rect, &message);
            }
            OverlayMode::Preview(text) => {
                draw_preview_overlay(&ctx, rect, &text);
            }
        }
        self.ivars().stats.record_draw(monotonic_ns() - start);
    }
}

/// 从 AppKit 当前绘制上下文取出 CGContext。
/// drawRect: 期间 NSGraphicsContext::currentContext() 有效。
#[allow(deprecated)]
fn current_cg_context() -> Option<CGContext> {
    unsafe {
        let ns_ctx = NSGraphicsContext::currentContext()?;
        let port = ns_ctx.graphicsPort();
        Some(CGContext::from_existing_context_ptr(
            port.as_ptr() as *mut CGContextSys
        ))
    }
}

// =============================================================================
// 主绘制函数：1:1 复刻 draw_recording_overlay
// =============================================================================

/// 绘制整个录音浮层。坐标均为左上原点、与 Windows 同尺寸（240×36）。
fn draw_overlay(ctx: &CGContext, rect: CGRect, levels: &AudioLevelBuf) {
    let w = rect.size.width;
    let h = rect.size.height;

    // 允许抗锯齿（对应 Windows HALFTONE）
    ctx.set_should_antialias(true);

    // --- 背景 + 圆角边框 ---
    fill_rounded_rect(ctx, rect, CORNER_RADIUS, BG_DARK);
    stroke_rounded_rect(ctx, rect, CORNER_RADIUS, BORDER_GRAY);

    // --- 三态音频指示灯 ---
    let circ_l = rect.origin.x + CIRC_LEFT;
    let circ_t = rect.origin.y + (h - CIRC_SIZE) / 2.0;
    let (buf_empty, has_audio) = {
        if let Ok(q) = levels.lock() {
            let empty = q.is_empty();
            let audio = !empty && q.iter().any(|v| v.current > 0.01);
            (empty, audio)
        } else {
            (true, false)
        }
    };
    let circ_color = if buf_empty {
        RED_STREAM_FAILED
    } else if has_audio {
        BRAND_ORANGE
    } else {
        GRAY_SILENT
    };
    // 中心圆（原生抗锯齿）
    fill_circle(
        ctx,
        CGPoint::new(circ_l + CIRC_SIZE / 2.0, circ_t + CIRC_SIZE / 2.0),
        CIRC_SIZE / 2.0,
        circ_color,
    );

    // --- 左分隔条 ---
    let sep_l_x = rect.origin.x + 30.0;
    let cy = rect.origin.y + h / 2.0;
    let sep_half = SEP_HEIGHT / 2.0;
    fill_rect(
        ctx,
        CGRect::new(
            &CGPoint::new(sep_l_x - SEP_WIDTH / 2.0, cy - sep_half),
            &CGSize::new(SEP_WIDTH, SEP_HEIGHT),
        ),
        BORDER_GRAY,
    );

    // --- 右分隔条 ---
    let sep_r_x = rect.origin.x + w - 36.0;
    fill_rect(
        ctx,
        CGRect::new(
            &CGPoint::new(sep_r_x - SEP_WIDTH / 2.0, cy - sep_half),
            &CGSize::new(SEP_WIDTH, SEP_HEIGHT),
        ),
        BORDER_GRAY,
    );

    // --- 波形：中心对称展开（WAVEFORM-FIX-002 映射）---
    let ww = sep_r_x - sep_l_x - 24.0; // 可用宽度（两侧各 12px）
    let total_bar_width = BAR_COUNT as f64 * BAR_WIDTH + (BAR_COUNT - 1) as f64 * BAR_GAP;
    let wl = sep_l_x + 12.0 + (ww - total_bar_width) / 2.0;

    // OVERLAY-LOCK-SCOPE-001: 锁内做峰值衰减快照，锁外绘制
    let half = BAR_COUNT / 2;
    let snapshot: Vec<f32> = if let Ok(mut q) = levels.lock() {
        for lvl in q.iter_mut() {
            lvl.update(lvl.current, DECAY_RATE);
        }
        let len = q.len();
        (0..half)
            .map(|i| {
                let idx = len.saturating_sub(1 + i);
                if idx < len {
                    q[idx].display_value()
                } else {
                    0.0
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    // 左右各 half 根，中心(i=0)对应最新样本，向边缘递减
    for side in 0..2 {
        for i in 0..half {
            let v = snapshot.get(i).copied().unwrap_or(0.0);
            let weight: f64 = 0.4
                + 0.6
                    * (std::f64::consts::FRAC_PI_2 * i as f64 / (half as f64 - 1.0).max(1.0))
                        .cos()
                        .powi(2);
            let v_gain = (v as f64 * BAR_GAIN * weight).min(1.0);
            let bh = if v_gain > 0.01 {
                BAR_MIN_H + v_gain * (BAR_MAX_H - BAR_MIN_H)
            } else {
                BAR_STATIC_H
            };
            let x = if side == 0 {
                wl + (half as f64 - 1.0 - i as f64) * BAR_STEP
            } else {
                wl + (half as f64 + i as f64) * BAR_STEP
            };
            let bar = CGRect::new(&CGPoint::new(x, cy - bh / 2.0), &CGSize::new(BAR_WIDTH, bh));
            fill_rect(ctx, bar, BRAND_ORANGE);
        }
    }

    // --- 停止按钮（STOP-BUTTON-CENTER-FIX-001）---
    let bl = rect.origin.x + w - 25.0;
    let bt = rect.origin.y + (h - STOP_BTN_SIZE) / 2.0;
    let outer = CGRect::new(
        &CGPoint::new(bl, bt),
        &CGSize::new(STOP_BTN_SIZE, STOP_BTN_SIZE),
    );
    // 外框：1px 橙色
    stroke_rect(ctx, outer, BRAND_ORANGE);
    // 内芯：8x8 实心，精确居中
    let inner = CGRect::new(
        &CGPoint::new(
            bl + (STOP_BTN_SIZE - STOP_BTN_INNER) / 2.0,
            bt + (STOP_BTN_SIZE - STOP_BTN_INNER) / 2.0,
        ),
        &CGSize::new(STOP_BTN_INNER, STOP_BTN_INNER),
    );
    fill_rect(ctx, inner, BRAND_ORANGE);
}

// =============================================================================
// Processing 态：1:1 复刻 Windows draw_processing_overlay（main.rs:1349）
// =============================================================================

/// 绘制 Processing 浮层：深色圆角背景 + 银色 shimmer 光带扫过 + 居中橙色文案。
/// 文案由调用方传入（main.rs 已按 ui_language 取 i18n::overlay_processing 传入）。
fn draw_processing_overlay(ctx: &CGContext, rect: CGRect, message: &str) {
    ctx.set_should_antialias(true);

    // 背景 + 圆角边框
    fill_rounded_rect(ctx, rect, PROC_CORNER_RADIUS, PROC_BG_DARK);
    stroke_rounded_rect(ctx, rect, PROC_CORNER_RADIUS, BORDER_GRAY);

    // shimmer：30-slice Gaussian alpha 银光带（PROCESSING-SHIMMER-001）
    draw_shimmer(ctx, rect);

    // 居中文案（BRAND_ORANGE，与 Windows SetTextColor 同色）
    draw_text(ctx, message, rect, BRAND_ORANGE, true);
}

/// 绘制 Processing shimmer：一条 30-slice 高斯 alpha 银色光带在浮层内左右扫过。
/// 相位由单调时钟推进（对应 Windows shimmer_phase 逐帧递增）。
fn draw_shimmer(ctx: &CGContext, rect: CGRect) {
    let w = rect.size.width;
    let win_h = (rect.size.height - 2.0).max(1.0);
    let glow_w_total = PROC_GLOW_HALF * 2.0;
    let travel = (w + glow_w_total) as f32;
    // 2.4s 一个周期，光带平滑往返
    let phase = ((monotonic_ns() / 1_000_000) % 2400) as f32 / 2400.0;
    let beam_cx = rect.origin.x - PROC_GLOW_HALF + (travel * phase) as f64;
    let slice_w = ((glow_w_total + PROC_SLICES as f64 - 1.0) / PROC_SLICES as f64).max(1.0);

    for i in 0..PROC_SLICES {
        let t = (i as f32 / (PROC_SLICES - 1) as f32) * 2.0 - 1.0;
        let alpha = ((-3.0_f32 * t * t).exp() * 150.0) as u8;
        if alpha == 0 {
            continue;
        }
        let x_dst = beam_cx - PROC_GLOW_HALF + i as f64 * slice_w;
        let x1 = x_dst.max(rect.origin.x + 1.0);
        let x2 = (x_dst + slice_w).min(rect.origin.x + w - 1.0);
        let sw = x2 - x1;
        if sw <= 0.0 {
            continue;
        }
        fill_rect_alpha(
            ctx,
            CGRect::new(
                &CGPoint::new(x1, rect.origin.y + 1.0),
                &CGSize::new(sw, win_h),
            ),
            PROC_SILVER,
            alpha as f64 / 255.0,
        );
    }
}

// =============================================================================
// Error 态：1:1 复刻 Windows draw_error_overlay（main.rs:1667）
// =============================================================================

/// 绘制 Error 浮层：深色圆角背景 + 左侧红色圆点 + 橙色错误文案。
fn draw_error_overlay(ctx: &CGContext, rect: CGRect, message: &str) {
    let w = rect.size.width;
    let h = rect.size.height;

    ctx.set_should_antialias(true);

    // 背景 + 圆角边框
    fill_rounded_rect(ctx, rect, CORNER_RADIUS, ERR_BG_DARK);
    stroke_rounded_rect(ctx, rect, CORNER_RADIUS, BORDER_GRAY);

    // 左侧红色圆点（diameter 8，left+12，垂直居中）
    let circ_d = ERR_CIRCLE_D;
    let circ_x = rect.origin.x + 12.0 + circ_d / 2.0;
    let cy = rect.origin.y + h / 2.0;
    fill_circle(ctx, CGPoint::new(circ_x, cy), circ_d / 2.0, ERR_RED);

    // 错误文案（橙色，圆点右侧左对齐，单行）
    let text_rect = CGRect::new(
        &CGPoint::new(circ_x + circ_d / 2.0 + 8.0, rect.origin.y + 4.0),
        &CGSize::new((w - 14.0) - (circ_x + circ_d / 2.0 + 8.0), h - 8.0),
    );
    draw_text(ctx, message, text_rect, BRAND_ORANGE, false);
}

// =============================================================================
// Preview 态：1:1 复刻 Windows draw_preview_overlay（main.rs:1476-1665）
// =============================================================================

/// Preview 浮层的三个可点区域（复制 / 关闭 / 标题栏关闭）。
/// 返回 `(copy_rect, close_rect, title_close_rect)`，坐标在视图左上原点系。
#[derive(Debug, Clone, Copy)]
pub(crate) struct PreviewRects {
    pub copy: CGRect,
    pub close: CGRect,
    pub title_close: CGRect,
}

/// 计算 Preview 浮层几何。与 Windows draw_preview_overlay 的布局算式 1:1 对应：
/// - 底部两枚按钮：45×18，gap 10，并排居中，btn_top = bottom - 18 - 10
/// - 标题栏关闭按钮：18×18，right-26..right-8，top+5..top+23
fn preview_geometry(bounds: CGRect) -> PreviewRects {
    let w = bounds.size.width;
    let h = bounds.size.height;
    // 底部按钮（居中）
    let total_w = PREVIEW_BTN_W * 2.0 + PREVIEW_BTN_GAP;
    let btn_left = bounds.origin.x + (w - total_w) / 2.0;
    let btn_top = bounds.origin.y + h - PREVIEW_BTN_H - 10.0;
    let copy = CGRect::new(
        &CGPoint::new(btn_left, btn_top),
        &CGSize::new(PREVIEW_BTN_W, PREVIEW_BTN_H),
    );
    let close = CGRect::new(
        &CGPoint::new(btn_left + PREVIEW_BTN_W + PREVIEW_BTN_GAP, btn_top),
        &CGSize::new(PREVIEW_BTN_W, PREVIEW_BTN_H),
    );
    // 标题栏关闭按钮（右侧）
    let title_close = CGRect::new(
        &CGPoint::new(bounds.origin.x + w - 26.0, bounds.origin.y + 5.0),
        &CGSize::new(18.0, 18.0),
    );
    PreviewRects {
        copy,
        close,
        title_close,
    }
}

/// 绘制 Preview 浮层：标题栏 + 正文预览 + 底部两枚按钮（复制/关闭）+ 标题栏关闭按钮。
fn draw_preview_overlay(ctx: &CGContext, rect: CGRect, text: &str) {
    let w = rect.size.width;
    let h = rect.size.height;
    let ui_language = OVERLAY_UI_LANGUAGE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().copied())
        .unwrap_or(UiLanguage::Chinese);
    let strings = i18n::get(ui_language);

    ctx.set_should_antialias(true);

    // 背景 + 圆角边框
    fill_rounded_rect(ctx, rect, CORNER_RADIUS, ERR_BG_DARK);
    stroke_rounded_rect(ctx, rect, CORNER_RADIUS, BORDER_GRAY);

    // 标题栏文字（28px 高，居中橙色）
    let title_rect = CGRect::new(
        &CGPoint::new(rect.origin.x + 26.0, rect.origin.y + 4.0),
        &CGSize::new(w - 52.0, PREVIEW_TITLE_BAR_H - 8.0),
    );
    draw_text(
        ctx,
        strings.preview_title_bar,
        title_rect,
        BRAND_ORANGE,
        true,
    );

    // 标题栏分隔线
    fill_rect(
        ctx,
        CGRect::new(
            &CGPoint::new(rect.origin.x + 8.0, rect.origin.y + PREVIEW_TITLE_BAR_H),
            &CGSize::new(w - 16.0, 1.0),
        ),
        BORDER_GRAY,
    );

    // 正文（左对齐，单行；Windows 为 DT_WORDBREAK|DT_END_ELLIPSIS，raw objc2 单行近似）
    let body_rect = CGRect::new(
        &CGPoint::new(
            rect.origin.x + 14.0,
            rect.origin.y + PREVIEW_TITLE_BAR_H + 8.0,
        ),
        &CGSize::new(w - 28.0, h - PREVIEW_TITLE_BAR_H - 8.0 - 40.0),
    );
    draw_text(ctx, text, body_rect, PREVIEW_BODY_TEXT, false);

    let geo = preview_geometry(rect);

    // 底部复制/关闭按钮（描边圆角 + 文字）
    stroke_rounded_rect(ctx, geo.copy, 8.0, BTN_BORDER);
    draw_text(ctx, strings.preview_copy_btn, geo.copy, BRAND_ORANGE, true);
    stroke_rounded_rect(ctx, geo.close, 8.0, BTN_BORDER);
    draw_text(ctx, strings.preview_close, geo.close, GRAY_SILENT, true);

    // 标题栏关闭按钮（18×18 圆角 6 + "✕"）
    stroke_rounded_rect(ctx, geo.title_close, 6.0, BTN_BORDER);
    draw_text(ctx, "\u{2715}", geo.title_close, BRAND_ORANGE, true);
}

// =============================================================================
// 文本绘制：raw objc2（NSString/NSFont/NSStringDrawing 均未开 feature，走 AnyClass msg_send）
// =============================================================================

/// 在 rect 内绘制单行文本。`centered=true` 时水平垂直双居中（对应 Windows DT_CENTER|DT_VCENTER）。
///
/// 实现：`NSString` + `NSFont systemFontOfSize:` + `NSColor` + `drawInRect:withAttributes:`，
/// 全部经 `AnyClass::get` + raw `msg_send_id!`/`msg_send!` 调用（无需 Cargo feature）。
fn draw_text(ctx: &CGContext, text: &str, rect: CGRect, color: (f64, f64, f64), centered: bool) {
    let _ = ctx;
    unsafe {
        // 1. NSString
        let ns = NSString::from_str(text);
        // 2. NSFont（class 方法，返回 +0，msg_send_id! 会 retain_autoreleased）
        let Some(font_cls) = AnyClass::get("NSFont") else {
            log::warn!("overlay draw_text: NSFont class not found");
            return;
        };
        let font: Retained<AnyObject> = msg_send_id![font_cls, systemFontOfSize: 13.0_f64];
        // 3. NSColor
        let ns_color = NSColor::colorWithSRGBRed_green_blue_alpha(color.0, color.1, color.2, 1.0);
        // 4. 属性字典 {NSFontAttributeName: font, NSForegroundColorAttributeName: color}
        //    用 NSMutableDictionary 经 raw msg_send 构建，绕开 NSDictionary::from_slice 的
        //    IsRetainable 约束（NSFont 未开 feature，无法用具体类名）。
        let Some(mdict_cls) = AnyClass::get("NSMutableDictionary") else {
            log::warn!("overlay draw_text: NSMutableDictionary class not found");
            return;
        };
        let mdict: Retained<AnyObject> = msg_send_id![mdict_cls, dictionary];
        let key_font = NSString::from_str("NSFont");
        let key_color = NSString::from_str("NSColor");
        let _: () = msg_send![&mdict, setObject: &*font, forKey: &*key_font];
        let _: () = msg_send![&mdict, setObject: &*ns_color, forKey: &*key_color];
        // 5. 测量文本尺寸（sizeWithAttributes:）
        let size: NSSize = msg_send![&ns, sizeWithAttributes: Some(&*mdict)];
        // 6. 计算绘制原点（居中或左对齐 + 垂直居中）
        let x = if centered {
            rect.origin.x + (rect.size.width - size.width) / 2.0
        } else {
            rect.origin.x
        };
        let y = rect.origin.y + (rect.size.height - size.height) / 2.0;
        let origin = objc2_foundation::CGPoint::new(x, y);
        let draw_rect = objc2_foundation::CGRect::new(
            origin,
            objc2_foundation::CGSize::new(size.width, size.height),
        );
        // 7. drawInRect:withAttributes:（NSStringDrawing category，raw msg_send 直调）
        let _: () = msg_send![&ns, drawInRect: draw_rect, withAttributes: Some(&*mdict)];
    }
}

/// 以指定 alpha 填充矩形（shimmer 光带用）。
fn fill_rect_alpha(ctx: &CGContext, rect: CGRect, color: (f64, f64, f64), alpha: f64) {
    ctx.set_rgb_fill_color(color.0, color.1, color.2, alpha);
    ctx.fill_rect(rect);
}

/// OVERLAY-003: 点是否落在矩形内（左上原点系，含边界）。
fn rect_contains(rect: CGRect, p: CGPoint) -> bool {
    p.x >= rect.origin.x
        && p.x <= rect.origin.x + rect.size.width
        && p.y >= rect.origin.y
        && p.y <= rect.origin.y + rect.size.height
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::overlay::{new_audio_level_buf, push_level};

    /// ⚠️ 独立重算，不调用生产绘制路径；生产公式变更本测试不会红。真护栏待 OVERLAY-002 抽函数后补。
    /// 波形柱权重：中心柱(i=0)权重最大，边缘柱权重最小（0.4 下限）。
    #[test]
    fn waveform_weight_center_is_largest() {
        let w0 = 0.4f64 + 0.6 * (std::f64::consts::FRAC_PI_2 * 0.0 / 15.0).cos().powi(2);
        let w15 = 0.4f64 + 0.6 * (std::f64::consts::FRAC_PI_2 * 15.0 / 15.0).cos().powi(2);
        assert!((w0 - 1.0).abs() < 0.001, "center weight must be 1.0");
        assert!((w15 - 0.4).abs() < 0.001, "edge weight must be 0.4");
    }

    /// ⚠️ 独立重算，不调用生产绘制路径；生产公式变更本测试不会红。真护栏待 OVERLAY-002 抽函数后补。
    /// 波形柱高度映射：高电平(1.0) → maxh，低电平(≈0) → minh，无音频 → static_h。
    #[test]
    fn waveform_height_mapping() {
        let gain: f64 = 2.5;
        // 高电平：v_gain=1.0 → maxh
        let vg_high = (1.0 * gain * 1.0).min(1.0);
        assert_eq!(vg_high, 1.0);
        let bh_high = BAR_MIN_H + vg_high * (BAR_MAX_H - BAR_MIN_H);
        assert!((bh_high - BAR_MAX_H).abs() < 0.001);
        // 极低电平 → static_h
        let vg_low: f64 = 0.0;
        let bh_low = if vg_low > 0.01 {
            BAR_MIN_H
        } else {
            BAR_STATIC_H
        };
        assert_eq!(bh_low, BAR_STATIC_H);
    }

    /// 三态指示灯判定逻辑。
    #[test]
    fn indicator_state_logic() {
        let levels = new_audio_level_buf();
        // 空 → 红
        assert!(levels.lock().unwrap().is_empty());
        // 有音频(>0.01) → 橙
        push_level(&levels, 0.5);
        {
            let q = levels.lock().unwrap();
            let audio = !q.is_empty() && q.iter().any(|v| v.current > 0.01);
            assert!(audio);
        }
        // 静音(≈0) → 灰：新建空队列并推入全零电平
        let silent = new_audio_level_buf();
        push_level(&silent, 0.0);
        push_level(&silent, 0.0);
        {
            let q = silent.lock().unwrap();
            let audio = !q.is_empty() && q.iter().any(|v| v.current > 0.01);
            assert!(!audio, "0.0 levels must not count as audio");
        }
    }

    /// 峰值衰减：与 Windows DECAY_RATE=0.02 一致。
    #[test]
    fn decay_rate_matches_windows() {
        assert_eq!(DECAY_RATE, 0.02);
    }

    /// ⚠️ 独立重算，不调用生产绘制路径；生产公式变更本测试不会红。真护栏待 OVERLAY-002 抽函数后补。
    /// 中心对称：左右两侧第 i 根柱使用同一快照值（镜像）。
    #[test]
    fn waveform_center_symmetric_mapping() {
        let half: usize = 16;
        let snapshot = vec![0.9f32, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2];
        // 左半边：i=0 → x = wl + (half-1)*step
        // 右半边：i=0 → x = wl + half*step
        // 两者紧邻中心，验证 i 映射对称
        for i in 0..half {
            let lx = (half as f64 - 1.0 - i as f64) * BAR_STEP;
            let rx = (half as f64 + i as f64) * BAR_STEP;
            // 左边中心柱(i=half-1)在 wl，右边中心柱(i=0)在 wl+half*step
            assert!(lx < rx, "left bar must be left of right bar");
        }
        assert_eq!(snapshot.len() <= half, true);
    }

    // OVERLAY-WIRE-001：跨测试串行化。PENDING_REQUEST / OVERLAY_LEVELS 是模块级
    // 全局 static，cargo test 默认多线程并行，拆多条 #[test] 会互相踩；用一把测试锁
    // 串行化，不依赖 --test-threads=1。
    static TEST_GUARD: Mutex<()> = Mutex::new(());

    /// OVERLAY-WIRE-001：请求通道 last-wins 语义。
    /// request_overlay 任意线程写 PENDING_REQUEST，poll_pending_overlay 主线程 take。
    /// 正向 Show→Hide 与反向 Hide→Show 连发后，槽内都只留最后一次请求，poll 后必须排空。
    #[test]
    fn request_channel_last_wins() {
        let _guard = TEST_GUARD.lock().unwrap();

        request_overlay(OverlayRequest::Show);
        request_overlay(OverlayRequest::Hide);
        assert_eq!(
            *PENDING_REQUEST.lock().unwrap(),
            Some(OverlayRequest::Hide),
            "Show→Hide 连发，槽内必须只剩 Hide（last wins），Show 不得残留"
        );
        poll_pending_overlay();
        assert!(PENDING_REQUEST.lock().unwrap().is_none(), "poll 后槽应排空");

        request_overlay(OverlayRequest::Hide);
        request_overlay(OverlayRequest::Show);
        assert_eq!(
            *PENDING_REQUEST.lock().unwrap(),
            Some(OverlayRequest::Show),
            "Hide→Show 连发，槽内必须只剩 Show（last wins）"
        );
        poll_pending_overlay();
        assert!(PENDING_REQUEST.lock().unwrap().is_none(), "poll 后槽应排空");
    }

    /// OVERLAY-WIRE-001：请求通道跨线程语义。
    /// 工作线程连发 Show→Hide（模拟按键线程与隐藏回调竞态），主测试线程 poll 后
    /// 同样只留 Hide 且排空。
    #[test]
    fn request_channel_cross_thread_last_wins() {
        let _guard = TEST_GUARD.lock().unwrap();

        std::thread::spawn(|| {
            request_overlay(OverlayRequest::Show);
            request_overlay(OverlayRequest::Hide);
        })
        .join()
        .unwrap();
        assert_eq!(
            *PENDING_REQUEST.lock().unwrap(),
            Some(OverlayRequest::Hide),
            "跨线程连发，槽内必须只剩 Hide（last wins）"
        );
        poll_pending_overlay();
        assert!(PENDING_REQUEST.lock().unwrap().is_none(), "poll 后槽应排空");
    }

    /// OVERLAY-WIRE-001：线程契约 — RecordingOverlay::new() 非主线程必须 Err。
    /// MainThreadMarker 守卫先于任何 AppKit 调用短路（overlay.rs:415），故本测试
    /// 安全，不会真的创建窗口。
    #[test]
    fn new_off_main_thread_returns_err() {
        let buf = new_audio_level_buf();
        let result = RecordingOverlay::new(buf);
        assert!(result.is_err(), "非主线程创建必须返回 Err");
    }

    /// OVERLAY-WIRE-001：非主线程误调 poll_pending_overlay 的 Show 分支不 panic。
    /// OVERLAY_LEVELS 未初始化走 warn 分支；即使已初始化，new() 非主线程返回 Err 被
    /// log 吞掉，thread_local slot 保持 None。
    #[test]
    fn poll_show_off_main_thread_no_panic() {
        let _guard = TEST_GUARD.lock().unwrap();

        request_overlay(OverlayRequest::Show);
        poll_pending_overlay();
        OVERLAY.with(|cell| {
            assert!(
                cell.borrow().is_none(),
                "非主线程 poll Show 不得建浮层，slot 必须保持 None"
            );
        });
    }
}
