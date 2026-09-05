//! Windows Hotkey Implementation using RegisterHotKey and low-level keyboard hook

use anyhow::{anyhow, Result};
use std::sync::{
    atomic::{AtomicBool, AtomicIsize, AtomicPtr, AtomicU32, AtomicU64, Ordering},
    Arc, RwLock,
};
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL,
    MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, KillTimer, MsgWaitForMultipleObjects, PeekMessageW, PostMessageW,
    PostThreadMessageW, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    MSG, PM_REMOVE, QS_ALLINPUT, WH_KEYBOARD_LL, WM_HOTKEY, WM_KEYDOWN, WM_KEYUP, WM_QUIT,
    WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
};

use crate::config::{AppConfig, HotkeyMode};

const HOTKEY_ID: i32 = 1;
const CONFIG_TIMER_ID: usize = 1;
const CONFIG_POLL_MS: u32 = 250;
const PTT_POLL_MS: u64 = 15;
const TRANSLATE_POLL_MS: u64 = 10;

/// HOTKEY-SYNC-IMMEDIATE-001: Atomic flag for immediate config change notification
/// Replaces unreliable WM_TIMER polling with instant notification from config watcher.
static CONFIG_CHANGED: AtomicBool = AtomicBool::new(false);

/// Notify the hotkey thread that config has changed (called by config watcher).
/// HOTKEY-SYNC-IMMEDIATE-001: This provides instant notification instead of WM_TIMER polling.
pub fn notify_config_changed() {
    CONFIG_CHANGED.store(true, Ordering::Release);
    log::info!("Hotkey config change notification set");
}

/// Static variables for low-level keyboard hook communication
static TARGET_VK: AtomicU32 = AtomicU32::new(0);
static TARGET_MODS: AtomicU32 = AtomicU32::new(0);
static TARGET_MODE: AtomicU32 = AtomicU32::new(0);
static TRANSLATION_VK: AtomicU32 = AtomicU32::new(0);
/// STOP flag for translate poll thread; set true when recording ends.
static TRANSLATE_POLL_STOP: AtomicBool = AtomicBool::new(false);
static HOOK_SENDER: AtomicPtr<crossbeam_channel::Sender<HotkeyEvent>> =
    AtomicPtr::new(std::ptr::null_mut());
static HOOK_WAKE_HWND: AtomicIsize = AtomicIsize::new(0);
static HOOK_WAKE_MSG: AtomicU32 = AtomicU32::new(0);
static CURRENT_HOOK: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static PTT_ACTIVE: AtomicBool = AtomicBool::new(false);
/// HOTKEY-115: Toggle 模式的录音进行中状态（与 PTT_ACTIVE 分开，不复用）。
/// PTT_ACTIVE 语义是「PTT 按住中」且被 poll_ptt_release_thread 的 while 自旋消费，
/// 复用会让 Toggle 录音生命周期与 PTT 释放检测互相干扰；分开后 B1（PTT 零回归）
/// 由构造保证，B2（两路径互不污染）由 uses_hook 互斥 + 绑定切换时的显式复位保证。
/// 复位点：notify_translate_poll_stop（B3，外部结束路径）+ install/uninstall/sync_binding（B4）。
static TOGGLE_ACTIVE: AtomicBool = AtomicBool::new(false);
/// HOTKEY-115-B: 目标键的物理按下状态（钩子路径的 auto-repeat 抑制判据）。
/// DOWN 置 true、UP 清 false；DOWN 之间没有 UP 就是系统 repeat，直接忽略。
/// 独立于 PTT_ACTIVE / TOGGLE_ACTIVE（它是「键」的状态，不是「模式语义」的状态），
/// 复位点与另两态一致：install / uninstall / sync_binding（防长按中重绑定后残留）。
static KEY_PHYSICALLY_DOWN: AtomicBool = AtomicBool::new(false);
/// HOTKEY-115-C: 上次目标键 DOWN 的时刻（GetTickCount64 毫秒）。
/// 给 KEY_PHYSICALLY_DOWN 提供陈旧自愈判据：标志停着但距上次 DOWN 超过阈值 ⇒
/// 之前必有一次钩子没看到的 KEYUP（Win+L 锁屏把 KEYUP 送安全桌面 / 上游吞 KEYUP /
/// 钩子摘除重装窗口），按首次按下处理，不再永久吃掉后续所有按下。
static LAST_TARGET_DOWN_TICKS: AtomicU64 = AtomicU64::new(0);
/// HOTKEY-115-C: 陈旧标志阈值。🔴 必须 > Windows 键盘「重复延迟」上限 1000ms
///（控制面板四档 250/500/750/1000ms —— 若只按 repeat 间隔 ~33ms 取阈值，长延迟
/// 设置下首个 repeat 会被误判陈旧、抑制失效退回翻转）。取 2 倍余量 2000ms；
/// 仍远小于丢 KEYUP 后用户重按的典型间隔（秒级，如锁屏→解锁→重按）。
/// 残余窗口如实声明：吞 UP + 2s 内重按仍会被当 repeat 吞掉一次 —— 有界自愈
///（≤2s）远优于无判据时的永久失灵；两态（TOGGLE_ACTIVE 等）由 B3 收口与
/// KEYUP 复位保障语义正确，故误吞一次后下一次按压行为自动恢复正常。
const STALE_DOWN_THRESHOLD_MS: u64 = 2000;

#[derive(Debug, Clone, Copy)]
struct WakeTarget {
    hwnd: isize,
    message: u32,
}

fn post_wake_message(wake_target: Option<WakeTarget>) {
    let Some(wake_target) = wake_target else {
        return;
    };
    if wake_target.hwnd == 0 || wake_target.message == 0 {
        return;
    }

    unsafe {
        let _ = PostMessageW(
            HWND(wake_target.hwnd as _),
            wake_target.message,
            WPARAM(0),
            LPARAM(0),
        );
    }
}

fn send_hotkey_event(
    sender: &crossbeam_channel::Sender<HotkeyEvent>,
    event: HotkeyEvent,
    wake_target: Option<WakeTarget>,
) {
    let _ = sender.send(event);
    post_wake_message(wake_target);
}

fn hook_wake_target() -> Option<WakeTarget> {
    let hwnd = HOOK_WAKE_HWND.load(Ordering::Relaxed);
    let message = HOOK_WAKE_MSG.load(Ordering::Relaxed);
    (hwnd != 0 && message != 0).then_some(WakeTarget { hwnd, message })
}

fn translation_pressed() -> bool {
    let translate_vk = TRANSLATION_VK.load(Ordering::Relaxed);
    translate_vk != 0 && key_pressed(translate_vk as i32)
}

fn spawn_translate_poll_thread(translate_flag: Arc<AtomicBool>) {
    thread::spawn(move || {
        let start = std::time::Instant::now();
        // Hard ceiling: poll thread must never outlive the maximum recording length.
        // This is a safety net independent of the TRANSLATE_POLL_STOP signal.
        let hard_deadline =
            start + std::time::Duration::from_secs(crate::config::MAX_RECORD_SECONDS + 5);
        loop {
            if TRANSLATE_POLL_STOP.load(Ordering::Relaxed) {
                break;
            }
            if translate_flag.load(Ordering::Relaxed) {
                break;
            }
            if start.elapsed() >= hard_deadline.duration_since(start) {
                log::warn!(
                    "Translate poll thread hit hard deadline ({}s), forcing stop",
                    crate::config::MAX_RECORD_SECONDS + 5
                );
                break;
            }
            if translation_pressed() {
                translate_flag.store(true, Ordering::Release);
                log::info!(
                    "Translation hotkey pressed after {:.0}ms of recording, translate enabled",
                    start.elapsed().as_millis()
                );
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(TRANSLATE_POLL_MS));
        }
    });
}

/// Notify the translate poll thread to stop (e.g. ESC cancel or abnormal end of recording).
/// This is a cross-module signal because the ESC cancel path lives in the controller loop.
/// HOTKEY-115 B3：控制器在所有「录音非热键结束」路径（PipelineEvent Done/Cancelled/
/// FocusLost/Error/FormatFailed、EditRequested、CancelStop、ESC）都汇聚调用本函数，
/// 因此这里同时复位 TOGGLE_ACTIVE —— 录音被 VAD 静音/最大时长/浮层按钮/取消等途径
/// 结束后，Toggle 状态随之归零，下一次按键不会被反转成 Stop。零控制器侧改动收口 B3。
pub fn notify_translate_poll_stop() {
    TRANSLATE_POLL_STOP.store(true, Ordering::Relaxed);
    TOGGLE_ACTIVE.store(false, Ordering::Relaxed);
    log::info!("Translate poll stop notified");
}

/// TRANS-HOTKEY-039-D: HotkeyMode → u32 映射，消除 install_keyboard_hook 与
/// keyboard_hook_proc 两处各自硬编码 1/0 的漂移风险。
/// PTT=1, Toggle=0。这个 u32 存入 TARGET_MODE 全局静态，钩子回调读出后判定。
fn hotkey_mode_to_u32(mode: HotkeyMode) -> u32 {
    match mode {
        HotkeyMode::PushToTalk => 1,
        HotkeyMode::Toggle => 0,
    }
}

/// TRANS-HOTKEY-039-D: 主热键抬起时是否应停止翻译键轮询。
/// PTT(1)：抬起 = 录音结束 → 停止。
/// Toggle(0)：抬起只是松手，录音仍在继续 → **不得停止**（否则根因 B 复活）。
fn should_stop_translate_poll_on_keyup(mode: u32) -> bool {
    mode == hotkey_mode_to_u32(HotkeyMode::PushToTalk)
}

/// Check if modifiers are pressed (used in hook callback)
unsafe fn modifiers_pressed_from_hook() -> bool {
    let modifiers = TARGET_MODS.load(Ordering::Relaxed);
    let alt_ok =
        modifiers & 0x0001 == 0 || (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;
    let ctrl_ok =
        modifiers & 0x0002 == 0 || (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
    let shift_ok =
        modifiers & 0x0004 == 0 || (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
    let win_ok = modifiers & 0x0008 == 0
        || (GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000) != 0
        || (GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000) != 0;
    alt_ok && ctrl_ok && shift_ok && win_ok
}

/// Low-level keyboard hook callback
unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let target_vk = TARGET_VK.load(Ordering::Relaxed);

        if kb.vkCode == target_vk {
            let msg_type = wparam.0 as u32;
            let mode = TARGET_MODE.load(Ordering::Relaxed);

            if msg_type == WM_KEYDOWN || msg_type == WM_SYSKEYDOWN {
                if modifiers_pressed_from_hook() {
                    // HOTKEY-115-B: 结构性 auto-repeat 抑制 —— 「DOWN 之间没有 UP 就是
                    // repeat」。钩子能同时看到 DOWN 和 UP，物理按键状态是精确判据，
                    // 不用计时窗。长按>0.5s 进入 ~30 次/秒 repeat，不加此闸 Toggle 会
                    // Start/Stop 高速交替、终态随机（Gavin「有时出现有时不出现」的另一半）。
                    // PTT 路径原有的 !PTT_ACTIVE 门控本已使 repeat 为 no-op，此闸对其
                    // 行为零影响；Toggle 路径的翻转由此封死。
                    //
                    // HOTKEY-115-C: 陈旧自愈 —— KEY_PHYSICALLY_DOWN 是唯一 DOWN 闸门，
                    // 一次未被看到的 KEYUP（锁屏安全桌面/上游吞 KEYUP/钩子重装）会让它
                    // 永久停在 true、热键失灵到重绑定。故标志已置位时不直接当 repeat
                    // 丢弃：距上次 DOWN 超过阈值（> 重复延迟上限 1000ms，见
                    // STALE_DOWN_THRESHOLD_MS 注释）判为陈旧，按首次按下处理。
                    // 真 repeat 的相邻 DOWN 间隔 ≤ 重复延迟上限 + ~33ms，恒 < 阈值，
                    // 不会被误判（阈值时间戳随每次 DOWN 刷新，长按期间间隔始终是
                    // repeat 间隔本身）。GetAsyncKeyState 不能做这个判别：DOWN 到达时
                    // 两种场景下键都处于按下态，判别力为零（其 LSB 转变位文档明示
                    // 多进程轮询下不可靠）。
                    let was_down = KEY_PHYSICALLY_DOWN.swap(true, Ordering::AcqRel);
                    let now = unsafe { GetTickCount64() } as u64;
                    let last = LAST_TARGET_DOWN_TICKS.swap(now, Ordering::AcqRel);
                    if !was_down || now.saturating_sub(last) > STALE_DOWN_THRESHOLD_MS {
                        // HOTKEY-115: 两种模式各自独立的状态机，不再共用 PTT_ACTIVE 做 DOWN 门控。
                        if mode == hotkey_mode_to_u32(HotkeyMode::PushToTalk) {
                            if !PTT_ACTIVE.load(Ordering::Relaxed) {
                                PTT_ACTIVE.store(true, Ordering::Relaxed);
                                TRANSLATE_POLL_STOP.store(false, Ordering::Relaxed);
                                let sender_ptr = HOOK_SENDER.load(Ordering::Relaxed);
                                if !sender_ptr.is_null() {
                                    let sender = &*sender_ptr;
                                    let translate_flag =
                                        Arc::new(AtomicBool::new(translation_pressed()));
                                    spawn_translate_poll_thread(Arc::clone(&translate_flag));
                                    send_hotkey_event(
                                        sender,
                                        HotkeyEvent::Start {
                                            translate: translate_flag,
                                        },
                                        hook_wake_target(),
                                    );
                                }
                            }
                        } else {
                            // HOTKEY-115 缺陷 1 修复：Toggle 状态由 TOGGLE_ACTIVE 承载，
                            // 不再借用 PTT_ACTIVE —— 原实现在 KEYUP 无条件把 PTT_ACTIVE 清零，
                            // 第二次 DOWN 永远走不到下面的 Stop 分支（该分支实际不可达）。
                            if !TOGGLE_ACTIVE.swap(true, Ordering::AcqRel) {
                                TRANSLATE_POLL_STOP.store(false, Ordering::Relaxed);
                                let sender_ptr = HOOK_SENDER.load(Ordering::Relaxed);
                                if !sender_ptr.is_null() {
                                    let sender = &*sender_ptr;
                                    let translate_flag =
                                        Arc::new(AtomicBool::new(translation_pressed()));
                                    spawn_translate_poll_thread(Arc::clone(&translate_flag));
                                    send_hotkey_event(
                                        sender,
                                        HotkeyEvent::Start {
                                            translate: translate_flag,
                                        },
                                        hook_wake_target(),
                                    );
                                }
                            } else {
                                // Toggle mode: second press ends recording
                                TOGGLE_ACTIVE.store(false, Ordering::Relaxed);
                                TRANSLATE_POLL_STOP.store(true, Ordering::Relaxed);
                                let sender_ptr = HOOK_SENDER.load(Ordering::Relaxed);
                                if !sender_ptr.is_null() {
                                    let sender = &*sender_ptr;
                                    send_hotkey_event(
                                        sender,
                                        HotkeyEvent::Stop,
                                        hook_wake_target(),
                                    );
                                }
                            }
                        }
                    }
                }
            } else if msg_type == WM_KEYUP || msg_type == WM_SYSKEYUP {
                // HOTKEY-115-B: 物理松键，repeat 抑制复位 —— 必须在模式判定之前，
                // 两模式的 KEYUP 都要清（KEYUP 分支本就无 modifiers 门控）。
                KEY_PHYSICALLY_DOWN.store(false, Ordering::Relaxed);
                // HOTKEY-115 缺陷 1 修复：PTT_ACTIVE.store(false) 从这里移进
                // should_stop_translate_poll_on_keyup 块内 —— 原来它在该 if 之外，
                // Toggle 模式 KEYUP 也会重置 PTT_ACTIVE，导致下一次 DOWN 被当首次按下。
                if should_stop_translate_poll_on_keyup(mode) {
                    TRANSLATE_POLL_STOP.store(true, Ordering::Relaxed);
                    let sender_ptr = HOOK_SENDER.load(Ordering::Relaxed);
                    if !sender_ptr.is_null() {
                        let sender = &*sender_ptr;
                        send_hotkey_event(sender, HotkeyEvent::Stop, hook_wake_target());
                    }
                    PTT_ACTIVE.store(false, Ordering::Relaxed);
                }
            }
            return LRESULT(1); // Consume the event
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Install low-level keyboard hook for a specific VK code
fn install_keyboard_hook(
    vk_code: u32,
    modifiers: u32,
    mode: HotkeyMode,
    sender: &crossbeam_channel::Sender<HotkeyEvent>,
    wake_target: Option<WakeTarget>,
) -> Result<()> {
    // Remove existing hook if any
    uninstall_keyboard_hook();

    TARGET_VK.store(vk_code, Ordering::Relaxed);
    TARGET_MODS.store(modifiers, Ordering::Relaxed);
    TARGET_MODE.store(hotkey_mode_to_u32(mode), Ordering::Relaxed);
    HOOK_SENDER.store(sender as *const _ as *mut _, Ordering::Relaxed);
    HOOK_WAKE_HWND.store(
        wake_target.map(|target| target.hwnd).unwrap_or(0),
        Ordering::Relaxed,
    );
    HOOK_WAKE_MSG.store(
        wake_target.map(|target| target.message).unwrap_or(0),
        Ordering::Relaxed,
    );
    // HOTKEY-115 B4：重绑定/重装钩子时两态归零，防旧模式残留状态污染新绑定。
    PTT_ACTIVE.store(false, Ordering::Relaxed);
    TOGGLE_ACTIVE.store(false, Ordering::Relaxed);
    KEY_PHYSICALLY_DOWN.store(false, Ordering::Relaxed);

    unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0)
            .map_err(|e| anyhow!("SetWindowsHookExW failed: {}", e))?;
        CURRENT_HOOK.store(hook.0, Ordering::Relaxed);
    }

    log::info!("Low-level keyboard hook installed for vk_code={}", vk_code);
    Ok(())
}

/// Uninstall low-level keyboard hook
fn uninstall_keyboard_hook() {
    let hook_ptr = CURRENT_HOOK.load(Ordering::Relaxed);
    if !hook_ptr.is_null() {
        unsafe {
            let hook = HHOOK(hook_ptr);
            let _ = UnhookWindowsHookEx(hook);
        }
        CURRENT_HOOK.store(std::ptr::null_mut(), Ordering::Relaxed);
        log::info!("Low-level keyboard hook uninstalled");
    }
    TARGET_VK.store(0, Ordering::Relaxed);
    TARGET_MODS.store(0, Ordering::Relaxed);
    TARGET_MODE.store(0, Ordering::Relaxed);
    HOOK_SENDER.store(std::ptr::null_mut(), Ordering::Relaxed);
    HOOK_WAKE_HWND.store(0, Ordering::Relaxed);
    HOOK_WAKE_MSG.store(0, Ordering::Relaxed);
    // HOTKEY-115 B4：卸钩子复位三态。
    PTT_ACTIVE.store(false, Ordering::Relaxed);
    TOGGLE_ACTIVE.store(false, Ordering::Relaxed);
    KEY_PHYSICALLY_DOWN.store(false, Ordering::Relaxed);
    TRANSLATE_POLL_STOP.store(true, Ordering::Relaxed);
}

/// Hotkey event types
#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    Start { translate: Arc<AtomicBool> },
    Stop,
    CancelStop,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HotkeyBinding {
    vk_code: u32,
    modifiers: u32,
    mode: HotkeyMode,
}

/// VK 鐮佹槸鍚﹂渶瑕佽疆璇㈡娴嬶紙RegisterHotKey 涓嶆敮鎸佸乏鍙冲彉浣擄級
fn needs_polling(vk_code: u32) -> bool {
    matches!(vk_code, 0xA0..=0xA5) // VK_LSHIFT/RSHIFT/LCONTROL/RCONTROL/LMENU/RMENU
}

#[derive(Debug, Clone, Copy, Default)]
struct ListenerState {
    current_binding: Option<HotkeyBinding>,
    uses_hook: bool, // Whether current binding uses low-level hook
}

/// Windows HotkeyListener implementation
pub struct HotkeyListener {
    rx: crossbeam_channel::Receiver<HotkeyEvent>,
    thread_id: Arc<AtomicU32>,
    join: Option<JoinHandle<()>>,
}

impl HotkeyListener {
    #[allow(dead_code)]
    pub fn new(shared_config: Arc<RwLock<AppConfig>>) -> Self {
        Self::new_with_wake_target(shared_config, None)
    }

    pub fn new_with_controller_wakeup(
        shared_config: Arc<RwLock<AppConfig>>,
        controller_hwnd: HWND,
        wake_message: u32,
    ) -> Self {
        let wake_target = WakeTarget {
            hwnd: controller_hwnd.0 as isize,
            message: wake_message,
        };
        Self::new_with_wake_target(shared_config, Some(wake_target))
    }

    fn new_with_wake_target(
        shared_config: Arc<RwLock<AppConfig>>,
        wake_target: Option<WakeTarget>,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::bounded::<HotkeyEvent>(8);
        let thread_id = Arc::new(AtomicU32::new(0));
        let thread_id_clone = Arc::clone(&thread_id);

        let join = thread::spawn(move || {
            if let Err(err) = run_listener(shared_config, tx, thread_id_clone, wake_target) {
                log::error!("Hotkey listener: {}", err);
            }
        });

        Self {
            rx,
            thread_id,
            join: Some(join),
        }
    }

    pub fn rx(&self) -> &crossbeam_channel::Receiver<HotkeyEvent> {
        &self.rx
    }

    pub fn shutdown(&self) {
        let thread_id = self.thread_id.load(Ordering::Acquire);
        if thread_id != 0 {
            let _ = unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        }
    }

    pub fn join(mut self) {
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run_listener(
    shared_config: Arc<RwLock<AppConfig>>,
    tx: crossbeam_channel::Sender<HotkeyEvent>,
    thread_id: Arc<AtomicU32>,
    wake_target: Option<WakeTarget>,
) -> Result<()> {
    let thread_id_value = unsafe { GetCurrentThreadId() };
    thread_id.store(thread_id_value, Ordering::Release);

    let mut state = ListenerState::default();

    // HOTKEY-SYNC-IMMEDIATE-001: Keep timer as fallback (safety net if watcher fails)
    // but rely primarily on AtomicBool notification for instant sync.
    unsafe {
        let _ = SetTimer(HWND::default(), CONFIG_TIMER_ID, CONFIG_POLL_MS, None);
    }
    sync_binding(&shared_config, &tx, &mut state, wake_target)?;

    log::info!("Hotkey thread started (id={})", thread_id_value);

    let mut msg = MSG::default();
    let mut running = true;
    while running {
        // HOTKEY-SYNC-IMMEDIATE-001: Check AtomicBool for instant config change notification
        if CONFIG_CHANGED.load(Ordering::Acquire) {
            CONFIG_CHANGED.store(false, Ordering::Release);
            log::info!("Hotkey thread: instant config sync triggered by AtomicBool");
            sync_binding(&shared_config, &tx, &mut state, wake_target)?;
        }

        // PERF-BATCH-001 TASK-2: Use MsgWaitForMultipleObjects instead of PeekMessageW+sleep.
        // Blocks with zero CPU during idle, wakes instantly on any window message.
        let _ = unsafe { MsgWaitForMultipleObjects(Some(&[]), false, 10, QS_ALLINPUT) };

        while unsafe { PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE) }.as_bool() {
            if msg.message == WM_QUIT {
                running = false;
                break;
            }

            match msg.message {
                WM_HOTKEY if msg.wParam.0 == HOTKEY_ID as usize => {
                    sync_binding(&shared_config, &tx, &mut state, wake_target)?;
                    handle_hotkey_trigger(&tx, &mut state, wake_target);
                }
                // HOTKEY-SYNC-IMMEDIATE-001: Timer is now fallback only (safety net)
                WM_TIMER if msg.wParam.0 == CONFIG_TIMER_ID => {
                    // Fallback sync if AtomicBool notification missed
                    if CONFIG_CHANGED.load(Ordering::Acquire) {
                        CONFIG_CHANGED.store(false, Ordering::Release);
                        log::info!("Hotkey thread: fallback config sync via WM_TIMER");
                        sync_binding(&shared_config, &tx, &mut state, wake_target)?;
                    }
                }
                _ => {}
            }
        }
    }

    unsafe {
        let _ = KillTimer(HWND::default(), CONFIG_TIMER_ID);
    }
    if let Some(binding) = state.current_binding.take() {
        if state.uses_hook {
            uninstall_keyboard_hook();
        } else {
            unregister_binding(binding);
        }
    }
    thread_id.store(0, Ordering::Release);
    Ok(())
}

fn sync_binding(
    shared_config: &Arc<RwLock<AppConfig>>,
    sender: &crossbeam_channel::Sender<HotkeyEvent>,
    state: &mut ListenerState,
    wake_target: Option<WakeTarget>,
) -> Result<()> {
    let (binding, translation_vk) = clone_hotkey_binding(shared_config);
    TRANSLATION_VK.store(translation_vk, Ordering::Relaxed);

    if let Some(current) = state.current_binding {
        log::info!(
            "sync_binding: current vk={}, mod={}, mode={:?}; new vk={}, mod={}, mode={:?}",
            current.vk_code,
            current.modifiers,
            current.mode,
            binding.vk_code,
            binding.modifiers,
            binding.mode
        );
    } else {
        log::info!(
            "sync_binding: no current binding, new vk={}, mod={}, mode={:?}",
            binding.vk_code,
            binding.modifiers,
            binding.mode
        );
    }

    if state.current_binding == Some(binding) {
        log::info!("sync_binding: binding unchanged, skipping");
        return Ok(());
    }
    log::info!("sync_binding: binding changed, updating...");

    // Clean up previous binding
    if let Some(previous) = state.current_binding.take() {
        if state.uses_hook {
            uninstall_keyboard_hook();
        } else {
            unregister_binding(previous);
        }
        state.uses_hook = false;
    }
    // HOTKEY-115 B2/B4：绑定切换（含 PTT↔Toggle、hook↔RegisterHotKey）时三态归零。
    // uninstall_keyboard_hook 已复位，但 RegisterHotKey 侧的 unregister_binding
    // 不复位静态量，这里统一兜底，保证任何路径切换后不残留旧状态。
    PTT_ACTIVE.store(false, Ordering::Relaxed);
    TOGGLE_ACTIVE.store(false, Ordering::Relaxed);
    KEY_PHYSICALLY_DOWN.store(false, Ordering::Relaxed);

    if binding.vk_code != 0 {
        if needs_polling(binding.vk_code) {
            // Use low-level keyboard hook for keys not supported by RegisterHotKey
            if let Err(err) = install_keyboard_hook(
                binding.vk_code,
                binding.modifiers,
                binding.mode,
                sender,
                wake_target,
            ) {
                log::warn!("Failed to install keyboard hook: {}", err);
            } else {
                state.current_binding = Some(binding);
                state.uses_hook = true;
            }
        } else {
            if let Err(err) = register_binding(binding) {
                log::warn!("Failed to register hotkey: {}", err);
            } else {
                state.current_binding = Some(binding);
            }
        }
    }

    Ok(())
}

fn handle_hotkey_trigger(
    sender: &crossbeam_channel::Sender<HotkeyEvent>,
    state: &mut ListenerState,
    wake_target: Option<WakeTarget>,
) {
    // This function is only called for RegisterHotKey bindings (non-hook bindings)
    // Hook-based bindings are handled directly by keyboard_hook_proc
    if state.current_binding.is_none() || state.uses_hook {
        return;
    }

    let binding = match state.current_binding {
        Some(binding) => binding,
        None => return,
    };

    match binding.mode {
        HotkeyMode::Toggle => {
            // HOTKEY-115 缺陷 2 修复：原实现恒发 Start（无任何 toggle 状态），
            // 靠控制器「Start 时已在录音则转 Stop」的兜底路径生存；录音被外部途径
            // 结束后该兜底失效（第二次按会重新开录）。改为 TOGGLE_ACTIVE 翻转，
            // 与钩子路径同一状态量，语义一致。
            if !TOGGLE_ACTIVE.swap(true, Ordering::AcqRel) {
                TRANSLATE_POLL_STOP.store(false, Ordering::Relaxed);
                let translate_flag = Arc::new(AtomicBool::new(translation_pressed()));
                spawn_translate_poll_thread(Arc::clone(&translate_flag));
                send_hotkey_event(
                    sender,
                    HotkeyEvent::Start {
                        translate: translate_flag,
                    },
                    wake_target,
                );
            } else {
                TOGGLE_ACTIVE.store(false, Ordering::Relaxed);
                TRANSLATE_POLL_STOP.store(true, Ordering::Relaxed);
                send_hotkey_event(sender, HotkeyEvent::Stop, wake_target);
            }
        }
        HotkeyMode::PushToTalk => {
            if !PTT_ACTIVE.swap(true, Ordering::AcqRel) {
                TRANSLATE_POLL_STOP.store(false, Ordering::Relaxed);
                let translate_flag = Arc::new(AtomicBool::new(translation_pressed()));
                spawn_translate_poll_thread(Arc::clone(&translate_flag));
                send_hotkey_event(
                    sender,
                    HotkeyEvent::Start {
                        translate: translate_flag,
                    },
                    wake_target,
                );
                let sender = sender.clone();
                thread::spawn(move || poll_ptt_release_thread(sender, binding, wake_target));
            }
        }
    }
}

fn register_binding(binding: HotkeyBinding) -> Result<()> {
    let modifiers = hotkey_modifiers(binding.modifiers);
    log::info!(
        "Registering hotkey: vk_code={}, modifiers={}, mode={:?}",
        binding.vk_code,
        binding.modifiers,
        binding.mode
    );

    const MAX_RETRIES: usize = 3;
    const RETRY_DELAY_MS: u64 = 100;

    for attempt in 1..=MAX_RETRIES {
        unsafe {
            let _ = UnregisterHotKey(HWND::default(), HOTKEY_ID);
        }

        if attempt > 1 {
            std::thread::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS));
            log::info!("Retrying hotkey registration (attempt {})", attempt);
        }

        let result = unsafe {
            RegisterHotKey(
                HWND::default(),
                HOTKEY_ID,
                modifiers | MOD_NOREPEAT,
                binding.vk_code,
            )
        };

        match result {
            Ok(_) => {
                log::info!("Hotkey registered successfully");
                return Ok(());
            }
            Err(err) => {
                if attempt == MAX_RETRIES {
                    return Err(anyhow!(
                        "RegisterHotKey failed after {} attempts: {}",
                        MAX_RETRIES,
                        err
                    ));
                }
                log::warn!("RegisterHotKey attempt {} failed: {}", attempt, err);
            }
        }
    }

    Err(anyhow!(
        "RegisterHotKey failed after {} attempts",
        MAX_RETRIES
    ))
}

fn unregister_binding(binding: HotkeyBinding) {
    if binding.vk_code == 0 {
        return;
    }

    log::info!("Unregistering hotkey: vk_code={}", binding.vk_code);
    unsafe {
        let result = UnregisterHotKey(HWND::default(), HOTKEY_ID);
        if result.is_ok() {
            log::info!("Hotkey unregistered successfully");
        } else {
            log::warn!("Failed to unregister hotkey");
        }
    }
}

fn hotkey_modifiers(modifiers: u32) -> HOT_KEY_MODIFIERS {
    let mut result = HOT_KEY_MODIFIERS(0);
    if modifiers & 0x0001 != 0 {
        result |= MOD_ALT;
    }
    if modifiers & 0x0002 != 0 {
        result |= MOD_CONTROL;
    }
    if modifiers & 0x0004 != 0 {
        result |= MOD_SHIFT;
    }
    if modifiers & 0x0008 != 0 {
        result |= MOD_WIN;
    }
    result
}

fn clone_hotkey_binding(shared_config: &Arc<RwLock<AppConfig>>) -> (HotkeyBinding, u32) {
    let config = match shared_config.read() {
        Ok(cfg) => cfg.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };

    let translation_vk = if config.translation.enabled {
        config.translation.vk_code
    } else {
        0
    };

    (
        HotkeyBinding {
            vk_code: config.hotkey.vk_code,
            modifiers: config.hotkey.modifiers,
            mode: config.hotkey.mode,
        },
        translation_vk,
    )
}

fn poll_ptt_release_thread(
    sender: crossbeam_channel::Sender<HotkeyEvent>,
    binding: HotkeyBinding,
    wake_target: Option<WakeTarget>,
) {
    let ptt_start = std::time::Instant::now();

    log::info!("PTT release poll started for vk_code={}", binding.vk_code);

    while PTT_ACTIVE.load(Ordering::Acquire) {
        if !binding_pressed(binding) {
            if ptt_start.elapsed() < std::time::Duration::from_millis(300) {
                log::info!("PTT release detected (< 300ms), sending CancelStop");
                send_hotkey_event(&sender, HotkeyEvent::CancelStop, wake_target);
            } else {
                log::info!("PTT release detected, sending Stop");
                send_hotkey_event(&sender, HotkeyEvent::Stop, wake_target);
            }
            TRANSLATE_POLL_STOP.store(true, Ordering::Relaxed);
            PTT_ACTIVE.store(false, Ordering::Release);
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(PTT_POLL_MS));
    }

    // Ensure stop flag is set if thread exits for any other reason
    TRANSLATE_POLL_STOP.store(true, Ordering::Relaxed);
}

fn binding_pressed(binding: HotkeyBinding) -> bool {
    key_pressed(binding.vk_code as i32) && modifiers_pressed(binding.modifiers)
}

fn modifiers_pressed(modifiers: u32) -> bool {
    let alt_ok = modifiers & 0x0001 == 0 || key_pressed(VK_MENU.0 as i32);
    let ctrl_ok = modifiers & 0x0002 == 0 || key_pressed(VK_CONTROL.0 as i32);
    let shift_ok = modifiers & 0x0004 == 0 || key_pressed(VK_SHIFT.0 as i32);
    let win_ok =
        modifiers & 0x0008 == 0 || key_pressed(VK_LWIN.0 as i32) || key_pressed(VK_RWIN.0 as i32);

    alt_ok && ctrl_ok && shift_ok && win_ok
}

fn key_pressed(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> AppConfig {
        let mut cfg = AppConfig::default();
        cfg.hotkey.vk_code = 0x78; // F9
        cfg.hotkey.modifiers = 0x0002; // Ctrl
        cfg.hotkey.mode = HotkeyMode::Toggle;
        cfg
    }

    #[test]
    fn clone_hotkey_binding_reads_updated_config_from_arc() {
        let config = Arc::new(RwLock::new(default_config()));

        let (binding, translation_vk) = clone_hotkey_binding(&config);
        assert_eq!(binding.vk_code, 0x78);
        assert_eq!(binding.modifiers, 0x0002);
        assert_eq!(binding.mode, HotkeyMode::Toggle);
        assert_eq!(translation_vk, 0);

        {
            let mut guard = config.write().unwrap();
            guard.hotkey.vk_code = 0x70; // F1
            guard.hotkey.modifiers = 0x0004; // Shift
            guard.hotkey.mode = HotkeyMode::PushToTalk;
            guard.translation.enabled = true;
            guard.translation.vk_code = 0x12; // Alt
        }

        let (binding, translation_vk) = clone_hotkey_binding(&config);
        assert_eq!(binding.vk_code, 0x70);
        assert_eq!(binding.modifiers, 0x0004);
        assert_eq!(binding.mode, HotkeyMode::PushToTalk);
        assert_eq!(translation_vk, 0x12);
    }

    #[test]
    fn clone_hotkey_binding_waits_for_write_lock_release() {
        let config = Arc::new(RwLock::new(default_config()));
        let config_clone = Arc::clone(&config);

        let handle = std::thread::spawn(move || {
            let _guard = config_clone.write().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(50));
        });

        let (binding, _) = clone_hotkey_binding(&config);
        assert_eq!(binding.vk_code, 0x78);

        handle.join().unwrap();
    }

    #[test]
    fn clone_hotkey_binding_recovers_from_poisoned_lock() {
        let config = Arc::new(RwLock::new(default_config()));

        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = config.write().unwrap();
            panic!("poisoning lock");
        }));

        let (binding, _) = clone_hotkey_binding(&config);
        assert_eq!(binding.vk_code, 0x78);
    }

    #[test]
    fn hook_wake_target_reads_configured_static_target() {
        HOOK_WAKE_HWND.store(0x1234, Ordering::Relaxed);
        HOOK_WAKE_MSG.store(0x8002, Ordering::Relaxed);

        let wake_target = hook_wake_target().expect("wake target should be present");
        assert_eq!(wake_target.hwnd, 0x1234);
        assert_eq!(wake_target.message, 0x8002);

        HOOK_WAKE_HWND.store(0, Ordering::Relaxed);
        HOOK_WAKE_MSG.store(0, Ordering::Relaxed);
        assert!(hook_wake_target().is_none());
    }

    #[test]
    fn send_hotkey_event_enqueues_without_wake_target() {
        let (tx, rx) = crossbeam_channel::bounded::<HotkeyEvent>(1);

        let flag = Arc::new(AtomicBool::new(false));
        send_hotkey_event(
            &tx,
            HotkeyEvent::Start {
                translate: Arc::clone(&flag),
            },
            None,
        );

        match rx.try_recv().unwrap() {
            HotkeyEvent::Start { translate } => assert!(!translate.load(Ordering::Acquire)),
            _ => panic!("expected Start event"),
        }
    }

    #[test]
    fn clone_hotkey_binding_ignores_disabled_translation_vk() {
        let config = Arc::new(RwLock::new(default_config()));
        {
            let mut guard = config.write().unwrap();
            guard.translation.enabled = false;
            guard.translation.vk_code = 0x12;
        }

        let (_, translation_vk) = clone_hotkey_binding(&config);

        assert_eq!(translation_vk, 0);
    }

    // HOTKEY-SYNC-IMMEDIATE-001: AtomicBool notification tests
    #[test]
    fn notify_config_changed_sets_atomic_flag() {
        // Reset flag before test
        CONFIG_CHANGED.store(false, Ordering::Release);

        notify_config_changed();

        assert!(
            CONFIG_CHANGED.load(Ordering::Acquire),
            "flag should be set after notify"
        );
    }

    #[test]
    fn config_changed_flag_can_be_consumed() {
        CONFIG_CHANGED.store(true, Ordering::Release);

        // Simulate hotkey thread consuming the flag
        let was_changed = CONFIG_CHANGED.load(Ordering::Acquire);
        CONFIG_CHANGED.store(false, Ordering::Release);

        assert!(was_changed, "flag should have been true");
        assert!(
            !CONFIG_CHANGED.load(Ordering::Acquire),
            "flag should be cleared after consumption"
        );
    }

    #[test]
    fn multiple_notify_calls_are_coalesced() {
        CONFIG_CHANGED.store(false, Ordering::Release);

        // Multiple rapid notifications
        notify_config_changed();
        notify_config_changed();
        notify_config_changed();

        // Still just one flag to consume
        assert!(CONFIG_CHANGED.load(Ordering::Acquire));
        CONFIG_CHANGED.store(false, Ordering::Release);
        assert!(
            !CONFIG_CHANGED.load(Ordering::Acquire),
            "flag should be cleared after single consumption"
        );
    }

    // ============================================================
    // TEST-SYNC-TRANS-001 翻译功能测试同步（Arc<AtomicBool> 版本）
    // ============================================================

    /// TRANS-HOTKEY-001: TRANSLATION_VK 初始值为 0
    #[test]
    fn translation_vk_default_is_zero() {
        // TRANSLATION_VK 静态变量默认值应为 0（未配置）
        assert_eq!(
            TRANSLATION_VK.load(Ordering::Relaxed),
            0,
            "should default to 0"
        );
    }

    /// TRANS-HOTKEY-002: HotkeyEvent::Start 携带 translate 字段（Arc<AtomicBool>）
    #[test]
    fn hotkey_event_start_carries_translate_flag() {
        let flag_true = Arc::new(AtomicBool::new(true));
        let flag_false = Arc::new(AtomicBool::new(false));

        let event_with_translate = HotkeyEvent::Start {
            translate: flag_true,
        };
        let event_without_translate = HotkeyEvent::Start {
            translate: flag_false,
        };

        match event_with_translate {
            HotkeyEvent::Start { translate } => assert!(translate.load(Ordering::Acquire)),
            _ => panic!("expected Start event"),
        }
        match event_without_translate {
            HotkeyEvent::Start { translate } => assert!(!translate.load(Ordering::Acquire)),
            _ => panic!("expected Start event"),
        }
    }

    /// TRANS-HOTKEY-003: translate=true/false 两种 Start 事件通过 channel 传递
    #[test]
    fn hotkey_event_start_translate_transmitted_via_channel() {
        let (tx, rx) = crossbeam_channel::bounded::<HotkeyEvent>(2);

        let flag_false = Arc::new(AtomicBool::new(false));
        let flag_true = Arc::new(AtomicBool::new(true));
        let _ = tx.send(HotkeyEvent::Start {
            translate: flag_false,
        });
        let _ = tx.send(HotkeyEvent::Start {
            translate: flag_true,
        });

        match rx.try_recv().unwrap() {
            HotkeyEvent::Start { translate } => assert!(!translate.load(Ordering::Acquire)),
            _ => panic!(),
        }
        match rx.try_recv().unwrap() {
            HotkeyEvent::Start { translate } => assert!(translate.load(Ordering::Acquire)),
            _ => panic!(),
        }
    }

    /// TRANS-HOTKEY-IMPROVE-001：每次 Start 事件携带独立 AtomicBool，互不影响
    #[test]
    fn each_start_event_has_independent_translate_flag() {
        let (tx, rx) = crossbeam_channel::bounded::<HotkeyEvent>(2);

        let flag1 = Arc::new(AtomicBool::new(false));
        let flag2 = Arc::new(AtomicBool::new(false));

        let _ = tx.send(HotkeyEvent::Start {
            translate: Arc::clone(&flag1),
        });
        let _ = tx.send(HotkeyEvent::Start {
            translate: Arc::clone(&flag2),
        });

        // 模拟第 1 次 session：500ms 窗口内翻译键被按下
        flag1.store(true, Ordering::Release);

        // 读取第 1 个事件
        match rx.try_recv().unwrap() {
            HotkeyEvent::Start { translate } => assert!(
                translate.load(Ordering::Acquire),
                "session 1 should translate"
            ),
            _ => panic!(),
        }

        // 第 2 个事件：翻译键未被按下，flag2 仍为 false
        match rx.try_recv().unwrap() {
            HotkeyEvent::Start { translate } => assert!(
                !translate.load(Ordering::Acquire),
                "session 2 should not translate"
            ),
            _ => panic!(),
        }
    }

    /// TRANS-HOTKEY-IMPROVE-002：AtomicBool 初始值反映触发瞬间按键状态
    #[test]
    fn translate_flag_initial_value_reflects_key_state_at_trigger() {
        // translate key not held → initial false
        let flag_not_held = Arc::new(AtomicBool::new(false));
        assert!(!flag_not_held.load(Ordering::Acquire));

        // translate key held → initial true
        let flag_held = Arc::new(AtomicBool::new(true));
        assert!(flag_held.load(Ordering::Acquire));
    }

    /// TRANS-HOTKEY-IMPROVE-003：Arc clone 共享同一底层值（poll 线程写，pipeline 读）
    #[test]
    fn translate_flag_shared_between_poll_thread_and_pipeline() {
        let flag = Arc::new(AtomicBool::new(false));
        let flag_for_poll = Arc::clone(&flag);
        let flag_for_pipeline = Arc::clone(&flag);

        // 模拟 poll 线程在 500ms 内检测到翻译键被按下
        let handle = std::thread::spawn(move || {
            flag_for_poll.store(true, Ordering::Release);
        });
        handle.join().unwrap();

        // pipeline 读取到 true
        assert!(flag_for_pipeline.load(Ordering::Acquire));
    }

    // ============================================================
    // TEST-SYNC-038 / TRANS-HOTKEY-039 硬上限(真护栏) + 模式映射(意图文档化,见用例内 ⚠️)
    // ============================================================

    /// TRANS-HOTKEY-039 (覆盖点6): 翻译轮询线程硬上限 = MAX_RECORD_SECONDS + 5。
    /// spawn_translate_poll_thread 中 hard_deadline 依赖此算式（:107），
    /// 若有人把 MAX_RECORD_SECONDS 改掉或把 +5 内联成具体秒数，此测试即失败。
    #[test]
    fn translate_poll_hard_deadline_is_max_record_plus_5() {
        assert_eq!(
            crate::config::MAX_RECORD_SECONDS + 5,
            305,
            "hard deadline must stay MAX_RECORD_SECONDS+5 = 305s"
        );
        // 轮询常量不可漂移（spawn_translate_poll_thread :130 与 main.rs 依赖）。
        assert_eq!(TRANSLATE_POLL_MS, 10, "poll interval must stay 10ms");
        assert_eq!(PTT_POLL_MS, 15, "PTT poll interval must stay 15ms");
        assert_eq!(CONFIG_POLL_MS, 250, "config poll interval must stay 250ms");
    }

    // =====================================================================
    // TRANS-HOTKEY-039-D: 真回归护栏（生产侧纯函数，非闭包自述）
    // 与 tester-1 那版的区别：本测试调 `hotkey_mode_to_u32` 和
    // `should_stop_translate_poll_on_keyup` 两个**生产函数**，
    // 不是自己定义闭包再断言闭包。把 `store(true)` 移出 `if` 门控、
    // 或把映射改错，这些测试会变红。
    // =====================================================================

    #[test]
    fn hotkey_mode_to_u32_maps_ptt_to_1_toggle_to_0() {
        assert_eq!(
            hotkey_mode_to_u32(HotkeyMode::PushToTalk),
            1,
            "PTT must map to 1"
        );
        assert_eq!(
            hotkey_mode_to_u32(HotkeyMode::Toggle),
            0,
            "Toggle must map to 0"
        );
    }

    #[test]
    fn should_stop_translate_poll_on_keyup_ptt_returns_true() {
        // PTT 抬起 = 录音结束 → 停止轮询
        let ptt_mode = hotkey_mode_to_u32(HotkeyMode::PushToTalk);
        assert!(
            should_stop_translate_poll_on_keyup(ptt_mode),
            "PTT keyup must stop translate poll"
        );
    }

    #[test]
    fn should_stop_translate_poll_on_keyup_toggle_returns_false() {
        // Toggle 抬起只是松手，录音仍在继续 → 不得停止（根因 B 回归护栏）
        let toggle_mode = hotkey_mode_to_u32(HotkeyMode::Toggle);
        assert!(
            !should_stop_translate_poll_on_keyup(toggle_mode),
            "Toggle keyup must NOT stop translate poll (bug regression guard)"
        );
    }

    #[test]
    fn mode_mapping_and_keyup_predicate_are_semantically_consistent() {
        // 防两处漂移：映射函数输出喂给判据函数，结果必须符合预期
        let ptt = hotkey_mode_to_u32(HotkeyMode::PushToTalk);
        let toggle = hotkey_mode_to_u32(HotkeyMode::Toggle);
        assert_eq!(should_stop_translate_poll_on_keyup(ptt), true);
        assert_eq!(should_stop_translate_poll_on_keyup(toggle), false);
    }
}
