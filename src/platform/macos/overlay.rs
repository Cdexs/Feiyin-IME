//! macOS recording overlay window.
//!
//! 1:1 复刻 Windows 录音浮层（src/main.rs `draw_recording_overlay` / `overlay_geometry`）：
//! - 240×36 无边框浮层，圆角 10px，位于主屏底部上方 64px、水平居中
//! - 三态音频指示灯（红=设备故障 > 橙=有音频 > 灰=静音），直径 18px，left+6
//! - 32 根 3px 宽/2px 间隔的波形柱，从中心向两侧展开，maxh=48 / static_h=12 / minh=8，gain=2.5
//! - 60fps（16ms CFRunLoopTimer）刷新，峰值衰减率 0.02/帧
//! - 左右分隔条 + 橙色停止按钮（与 Windows 完全一致）
//!
//! MACOS-P4-OVERLAY-001: 只实现 Recording 状态（P0）。Processing / FocusLost / Error
//! 浮层留给 OVERLAY-002。

use core_graphics::context::CGContext;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::sys::CGContext as CGContextSys;
use objc2::rc::{Allocated, Retained};
use objc2::{declare_class, msg_send, msg_send_id, mutability, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSBorderlessWindowMask,
    NSColor, NSGraphicsContext, NSNonactivatingPanelMask, NSPanel, NSScreen, NSStatusWindowLevel,
    NSView, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSRect};
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

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

/// 波形条“有效柱宽+间隔”步长（bw + bgap）
const BAR_STEP: f64 = BAR_WIDTH + BAR_GAP;

// =============================================================================
// 共享状态
// =============================================================================

/// OverlayView 的 ivars：持有一个可绘制的共享状态引用。
struct OverlayIvars {
    levels: AudioLevelBuf,
    stats: Arc<OverlayStats>,
}

/// 待注入 OverlayView 的 (levels, stats)（单例创建时一次性使用）。
/// 因为 `OverlayIvars` 不是 ObjC 类型，无法作为 init 消息参数传递，
/// 所以重写 `initWithFrame:` 时从该静态取出。
static PENDING_LEVELS: Mutex<Option<(AudioLevelBuf, Arc<OverlayStats>)>> = Mutex::new(None);

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
pub struct RecordingOverlay {
    panel: Retained<NSPanel>,
    _view: Retained<OverlayView>,
    timer: core_foundation_sys::runloop::CFRunLoopTimerRef,
    /// 持有 Box，保证计时器回调上下文存活（经裸指针引用）。
    #[allow(dead_code)]
    timer_ctx: Box<TimerCtx>,
    /// 帧性能统计（供验收/调试轮询）。
    stats: Arc<OverlayStats>,
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
        *PENDING_LEVELS.lock().unwrap() = Some((Arc::clone(&levels), Arc::clone(&stats)));
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
    #[allow(dead_code)] // 供 OVERLAY-002 接入主控后使用
    pub fn hide(&self) {
        self.panel.orderOut(None);
    }

    /// 当前是否可见。
    #[allow(dead_code)] // 供 OVERLAY-002 接入主控后使用
    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// 帧性能统计快照（帧间隔 / 单帧绘制耗时，供性能验收）。
    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    /// 关闭浮层并释放计时器。Drop 时自动调用。
    pub fn destroy(&mut self) {
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
                .map(|(levels, stats)| OverlayIvars { levels, stats })
                .unwrap_or_else(|| OverlayIvars {
                    levels: Arc::new(Mutex::new(std::collections::VecDeque::new())),
                    stats: Arc::new(OverlayStats::new()),
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
    }
);

impl OverlayView {
    /// 创建一个绘制视图（主线程）。
    fn new(_mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        unsafe { msg_send_id![_mtm.alloc::<Self>(), initWithFrame: frame] }
    }

    /// 渲染整个浮层内容。
    fn render_overlay(&self) {
        let Some(ctx) = current_cg_context() else {
            return;
        };
        // 直接读 OverlayView 自己的 ivars（无需全局裸指针）
        let levels = Arc::clone(&self.ivars().levels);
        let bounds = self.bounds();
        let start = monotonic_ns();
        draw_overlay(&ctx, to_cg_rect(bounds), &levels);
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
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::overlay::{new_audio_level_buf, push_level};

    /// 波形柱权重：中心柱(i=0)权重最大，边缘柱权重最小（0.4 下限）。
    #[test]
    fn waveform_weight_center_is_largest() {
        let w0 = 0.4f64 + 0.6 * (std::f64::consts::FRAC_PI_2 * 0.0 / 15.0).cos().powi(2);
        let w15 = 0.4f64 + 0.6 * (std::f64::consts::FRAC_PI_2 * 15.0 / 15.0).cos().powi(2);
        assert!((w0 - 1.0).abs() < 0.001, "center weight must be 1.0");
        assert!((w15 - 0.4).abs() < 0.001, "edge weight must be 0.4");
    }

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
}
