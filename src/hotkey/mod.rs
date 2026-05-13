use anyhow::{anyhow, Result};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, RwLock,
};
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL,
    MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, PeekMessageW, PostThreadMessageW, SetTimer, MSG, PM_REMOVE, WM_HOTKEY, WM_QUIT,
    WM_TIMER,
};

use crate::config::{AppConfig, HotkeyMode};

const HOTKEY_ID: i32 = 1;
const CONFIG_TIMER_ID: usize = 1;
const CONFIG_POLL_MS: u32 = 250;
const PTT_POLL_MS: u64 = 15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HotkeyEvent {
    Start,
    Stop,
    CancelStop, // PTT 按住时间 < 300ms，取消处理
}

pub struct HotkeyListener {
    rx: crossbeam_channel::Receiver<HotkeyEvent>,
    thread_id: Arc<AtomicU32>,
    join: Option<JoinHandle<()>>,
}

impl HotkeyListener {
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct HotkeyBinding {
    vk_code: u32,
    modifiers: u32,
    mode: HotkeyMode,
}

/// VK 码是否需要轮询检测（RegisterHotKey 不支持左右变体）
fn needs_polling(vk_code: u32) -> bool {
    matches!(vk_code, 0xA0..=0xA5) // VK_LSHIFT/RSHIFT/LCONTROL/RCONTROL/LMENU/RMENU
}

#[derive(Debug, Clone, Copy, Default)]
struct ListenerState {
    current_binding: Option<HotkeyBinding>,
    polling_key_pressed: bool, // 轮询热键的上一帧按键状态（防重复触发）
}

pub fn spawn_hotkey_listener(shared_config: Arc<RwLock<AppConfig>>) -> HotkeyListener {
    let (tx, rx) = crossbeam_channel::bounded::<HotkeyEvent>(8);
    let thread_id = Arc::new(AtomicU32::new(0));
    let thread_id_clone = Arc::clone(&thread_id);

    let join = thread::spawn(move || {
        if let Err(err) = run_listener(shared_config, tx, thread_id_clone) {
            log::error!("Hotkey listener: {}", err);
        }
    });

    HotkeyListener {
        rx,
        thread_id,
        join: Some(join),
    }
}

fn run_listener(
    shared_config: Arc<RwLock<AppConfig>>,
    tx: crossbeam_channel::Sender<HotkeyEvent>,
    thread_id: Arc<AtomicU32>,
) -> Result<()> {
    let thread_id_value = unsafe { GetCurrentThreadId() };
    thread_id.store(thread_id_value, Ordering::Release);

    let mut state = ListenerState::default();
    // PTT 活动状态（共享给轮询线程）
    let ptt_active = Arc::new(AtomicBool::new(false));
    // PTT 释放检测线程的停止信号
    let ptt_poll_stop = Arc::new(AtomicBool::new(false));

    unsafe {
        let _ = SetTimer(HWND::default(), CONFIG_TIMER_ID, CONFIG_POLL_MS, None);
    }
    sync_binding(&shared_config, &tx, &mut state, &ptt_active, &ptt_poll_stop)?;

    log::info!("Hotkey thread started (id={})", thread_id_value);

    let mut msg = MSG::default();
    let mut running = true;
    while running {
        while unsafe { PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE) }.as_bool() {
            if msg.message == WM_QUIT {
                running = false;
                break;
            }

            match msg.message {
                WM_HOTKEY if msg.wParam.0 == HOTKEY_ID as usize => {
                    sync_binding(&shared_config, &tx, &mut state, &ptt_active, &ptt_poll_stop)?;
                    handle_hotkey_trigger(&tx, &mut state, &ptt_active, &ptt_poll_stop);
                }
                WM_TIMER if msg.wParam.0 == CONFIG_TIMER_ID => {
                    // Reload from disk to pick up hotkey changes saved by Tauri UI
                    if let Ok(new_config) = AppConfig::load() {
                        // BUG-024: 添加日志确认配置重载
                        log::info!(
                            "CONFIG_TIMER: disk reload vk={}, mod={}, mode={:?}",
                            new_config.hotkey.vk_code,
                            new_config.hotkey.modifiers,
                            new_config.hotkey.mode
                        );
                        if let Ok(mut cfg) = shared_config.write() {
                            *cfg = new_config;
                        }
                    } else {
                        log::warn!("CONFIG_TIMER: failed to reload config from disk");
                    }
                    sync_binding(&shared_config, &tx, &mut state, &ptt_active, &ptt_poll_stop)?;
                }
                _ => {}
            }
        }

        // 轮询路径：VK_RCONTROL/VK_RMENU 等 RegisterHotKey 不支持的 VK 码
        if let Some(binding) = state.current_binding {
            if needs_polling(binding.vk_code) {
                let pressed = binding_pressed(binding);
                if pressed && !state.polling_key_pressed {
                    state.polling_key_pressed = true;
                    handle_hotkey_trigger(&tx, &mut state, &ptt_active, &ptt_poll_stop);
                } else if !pressed {
                    state.polling_key_pressed = false;
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // 停止 PTT 轮询线程
    ptt_poll_stop.store(true, Ordering::Release);

    unsafe {
        let _ = KillTimer(HWND::default(), CONFIG_TIMER_ID);
    }
    if let Some(binding) = state.current_binding.take() {
        if !needs_polling(binding.vk_code) {
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
    ptt_active: &Arc<AtomicBool>,
    ptt_poll_stop: &Arc<AtomicBool>,
) -> Result<()> {
    let binding = clone_hotkey_binding(shared_config);

    // BUG-024: 添加调试日志，追踪热键配置变化
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

    // 配置变化时停止 PTT 轮询
    if ptt_active.load(Ordering::Acquire) {
        ptt_poll_stop.store(true, Ordering::Release);
        ptt_active.store(false, Ordering::Release);
        let _ = sender.send(HotkeyEvent::Stop);
    }

    // unregister 旧热键：仅当不需要轮询时才调用 RegisterHotKey 路径的 unregister
    if let Some(previous) = state.current_binding.take() {
        if !needs_polling(previous.vk_code) {
            unregister_binding(previous);
        }
    }

    if binding.vk_code != 0 {
        if needs_polling(binding.vk_code) {
            // 轮询路径：跳过 register_binding，直接绑定
            state.current_binding = Some(binding);
            // 初始化按键状态（防止刚切换时误触发）
            state.polling_key_pressed = key_pressed(binding.vk_code as i32);
            log::info!("Hotkey set to polling mode for vk_code={}", binding.vk_code);
        } else {
            // RegisterHotKey 路径
            if let Err(err) = register_binding(binding) {
                // BUG-023: 失败时线程不崩，仅输出警告
                log::warn!("Failed to register hotkey: {}", err);
                // state.current_binding 保持 None，热键暂时无效
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
    ptt_active: &Arc<AtomicBool>,
    ptt_poll_stop: &Arc<AtomicBool>,
) {
    let Some(binding) = state.current_binding else {
        return;
    };

    match binding.mode {
        HotkeyMode::Toggle => {
            let _ = sender.send(HotkeyEvent::Start);
        }
        HotkeyMode::PushToTalk => {
            if !ptt_active.load(Ordering::Acquire) {
                let _ = sender.send(HotkeyEvent::Start);
                ptt_active.store(true, Ordering::Release);

                // 启动专门的线程来轮询按键释放状态
                let sender_clone = sender.clone();
                let binding_clone = binding;
                let ptt_active_clone = Arc::clone(ptt_active);
                let stop_signal = Arc::clone(ptt_poll_stop);
                stop_signal.store(false, Ordering::Release);

                thread::spawn(move || {
                    poll_ptt_release_thread(
                        sender_clone,
                        binding_clone,
                        ptt_active_clone,
                        stop_signal,
                    );
                });
            }
        }
    }
}

/// PTT 释放检测线程 - 在独立线程中轮询按键状态，检测到释放后发送 Stop/CancelStop 事件
fn poll_ptt_release_thread(
    sender: crossbeam_channel::Sender<HotkeyEvent>,
    binding: HotkeyBinding,
    ptt_active: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
) {
    // 记录 PTT 按住开始时间
    let ptt_start = std::time::Instant::now();

    log::info!(
        "PTT poll thread started for binding: vk_code={}",
        binding.vk_code
    );

    while !stop_signal.load(Ordering::Acquire) {
        if !binding_pressed(binding) {
            // 按住时间 < 300ms 则取消处理
            if ptt_start.elapsed() < std::time::Duration::from_millis(300) {
                log::info!("PTT release detected (< 300ms), sending CancelStop event");
                let _ = sender.send(HotkeyEvent::CancelStop);
            } else {
                log::info!("PTT release detected, sending Stop event");
                let _ = sender.send(HotkeyEvent::Stop);
            }
            ptt_active.store(false, Ordering::Release);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(PTT_POLL_MS));
    }

    log::info!("PTT poll thread exiting");
}

fn register_binding(binding: HotkeyBinding) -> Result<()> {
    let modifiers = hotkey_modifiers(binding.modifiers);
    log::info!(
        "Registering hotkey: vk_code={}, modifiers={}, mode={:?}",
        binding.vk_code,
        binding.modifiers,
        binding.mode
    );

    // 尝试多次注册热键，处理异常退出后热键可能仍被占用的情况
    const MAX_RETRIES: usize = 3;
    const RETRY_DELAY_MS: u64 = 100;

    for attempt in 1..=MAX_RETRIES {
        // 先尝试注销（可能之前异常退出时未释放）
        unsafe {
            let _ = UnregisterHotKey(HWND::default(), HOTKEY_ID);
        }

        // 如果不是第一次尝试，等待一小段时间让系统清理
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

fn clone_hotkey_binding(shared_config: &Arc<RwLock<AppConfig>>) -> HotkeyBinding {
    let config = match shared_config.read() {
        Ok(cfg) => cfg.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };

    HotkeyBinding {
        vk_code: config.hotkey.vk_code,
        modifiers: config.hotkey.modifiers,
        mode: config.hotkey.mode,
    }
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
