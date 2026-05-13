//! macOS hotkey implementation based on CGEventTap + CFRunLoop.

use anyhow::{anyhow, Result};
use core_foundation::base::TCFType;
use core_foundation::runloop::{
    kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoop, CFRunLoopRef,
};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CallbackResult, EventField, KeyCode,
};
use crossbeam_channel::Receiver;
use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicBool, AtomicPtr, Ordering},
    Arc, RwLock,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config::{AppConfig, HotkeyMode};

const CONFIG_POLL_INTERVAL_SECS: f64 = 0.25;
const SHORT_PRESS_THRESHOLD: Duration = Duration::from_millis(300);
const KEYBOARD_EVENTS: [CGEventType; 5] = [
    CGEventType::KeyDown,
    CGEventType::KeyUp,
    CGEventType::FlagsChanged,
    CGEventType::TapDisabledByTimeout,
    CGEventType::TapDisabledByUserInput,
];

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    Start { translate: Arc<AtomicBool> },
    Stop,
    CancelStop,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HotkeyBinding {
    vk_code: u32,
    mac_keycode: u16,
    modifiers: u32,
    mode: HotkeyMode,
    primary_modifier_flag: Option<CGEventFlags>,
}

struct TapContext {
    sender: crossbeam_channel::Sender<HotkeyEvent>,
    binding: Option<HotkeyBinding>,
    primary_pressed: bool,
    press_started_at: Option<Instant>,
    needs_reenable: bool,
}

impl TapContext {
    fn new(sender: crossbeam_channel::Sender<HotkeyEvent>) -> Self {
        Self {
            sender,
            binding: None,
            primary_pressed: false,
            press_started_at: None,
            needs_reenable: false,
        }
    }

    fn sync_binding(&mut self, shared_config: &Arc<RwLock<AppConfig>>) {
        let config = clone_hotkey_config(shared_config);
        let new_binding = resolve_binding(&config);

        if self.binding == new_binding {
            return;
        }

        if new_binding.is_none() && config.vk_code != 0 {
            log::warn!(
                "macOS hotkey listener: unsupported vk_code={} display_name={}",
                config.vk_code,
                config.display_name
            );
        } else if let Some(binding) = new_binding {
            log::info!(
                "macOS hotkey binding updated: vk={} -> keycode={} modifiers={} mode={:?}",
                binding.vk_code,
                binding.mac_keycode,
                binding.modifiers,
                binding.mode
            );
        }

        self.binding = new_binding;
        self.primary_pressed = false;
        self.press_started_at = None;
    }

    fn handle_event(&mut self, event_type: CGEventType, event: &CGEvent) -> CallbackResult {
        match event_type {
            CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                self.needs_reenable = true;
                return CallbackResult::Keep;
            }
            CGEventType::KeyDown | CGEventType::KeyUp | CGEventType::FlagsChanged => {}
            _ => return CallbackResult::Keep,
        }

        let Some(binding) = self.binding else {
            return CallbackResult::Keep;
        };

        let event_keycode =
            event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
        if event_keycode != binding.mac_keycode {
            return CallbackResult::Keep;
        }

        let flags = event.get_flags();
        let primary_now_pressed =
            primary_key_pressed(event_type, flags, binding.primary_modifier_flag);
        let modifiers_ok = modifiers_match(flags, binding.modifiers, binding.primary_modifier_flag);
        let is_repeat = event_type == CGEventType::KeyDown
            && event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) != 0;

        match binding.mode {
            HotkeyMode::Toggle => {
                if primary_now_pressed && !self.primary_pressed && modifiers_ok && !is_repeat {
                    self.primary_pressed = true;
                    self.press_started_at = Some(Instant::now());
                    let _ = self.sender.send(HotkeyEvent::Start {
                        translate: Arc::new(AtomicBool::new(false)),
                    });
                    return CallbackResult::Drop;
                }

                if !primary_now_pressed && self.primary_pressed {
                    self.primary_pressed = false;
                    self.press_started_at = None;
                    return CallbackResult::Drop;
                }
            }
            HotkeyMode::PushToTalk => {
                if primary_now_pressed && !self.primary_pressed && modifiers_ok && !is_repeat {
                    self.primary_pressed = true;
                    self.press_started_at = Some(Instant::now());
                    let _ = self.sender.send(HotkeyEvent::Start {
                        translate: Arc::new(AtomicBool::new(false)),
                    });
                    return CallbackResult::Drop;
                }

                if !primary_now_pressed && self.primary_pressed {
                    let held_for = self
                        .press_started_at
                        .map(|started| started.elapsed())
                        .unwrap_or_default();
                    self.primary_pressed = false;
                    self.press_started_at = None;

                    let event = if held_for < SHORT_PRESS_THRESHOLD {
                        HotkeyEvent::CancelStop
                    } else {
                        HotkeyEvent::Stop
                    };
                    let _ = self.sender.send(event);
                    return CallbackResult::Drop;
                }
            }
        }

        if self.primary_pressed || primary_now_pressed {
            CallbackResult::Drop
        } else {
            CallbackResult::Keep
        }
    }
}

pub struct HotkeyListener {
    rx: Receiver<HotkeyEvent>,
    stop_signal: Arc<AtomicBool>,
    run_loop: Arc<AtomicPtr<c_void>>,
    join: Option<JoinHandle<()>>,
}

impl HotkeyListener {
    pub fn new(shared_config: Arc<RwLock<AppConfig>>) -> Self {
        let (tx, rx) = crossbeam_channel::bounded::<HotkeyEvent>(8);
        let stop_signal = Arc::new(AtomicBool::new(false));
        let run_loop = Arc::new(AtomicPtr::new(ptr::null_mut()));

        let stop_signal_clone = Arc::clone(&stop_signal);
        let run_loop_clone = Arc::clone(&run_loop);

        let join = thread::spawn(move || {
            if let Err(err) = run_listener(shared_config, tx, stop_signal_clone, run_loop_clone) {
                log::error!("macOS hotkey listener: {}", err);
            }
        });

        Self {
            rx,
            stop_signal,
            run_loop,
            join: Some(join),
        }
    }

    pub fn rx(&self) -> &Receiver<HotkeyEvent> {
        &self.rx
    }

    pub fn shutdown(&self) {
        self.stop_signal.store(true, Ordering::Release);

        let run_loop = self.run_loop.load(Ordering::Acquire);
        if !run_loop.is_null() {
            unsafe { CFRunLoop::wrap_under_get_rule(run_loop as CFRunLoopRef) }.stop();
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
    stop_signal: Arc<AtomicBool>,
    run_loop_ptr: Arc<AtomicPtr<c_void>>,
) -> Result<()> {
    super::accessibility::ensure_accessibility_at_startup()?;

    let context = Rc::new(RefCell::new(TapContext::new(tx)));
    context.borrow_mut().sync_binding(&shared_config);

    let callback_context = Rc::clone(&context);
    let tap = unsafe {
        CGEventTap::new_unchecked(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            KEYBOARD_EVENTS.to_vec(),
            move |_proxy, event_type, event| callback_context.borrow_mut().handle_event(event_type, event),
        )
    }
    .map_err(|_| anyhow!("CGEventTapCreate returned NULL; accessibility permission or tap location may be invalid"))?;

    let source = tap
        .mach_port()
        .create_runloop_source(0)
        .ok_or_else(|| anyhow!("CFMachPort create_runloop_source failed"))?;

    let run_loop = CFRunLoop::get_current();
    run_loop_ptr.store(
        run_loop.as_concrete_TypeRef() as *mut c_void,
        Ordering::Release,
    );
    run_loop.add_source(&source, unsafe { kCFRunLoopCommonModes });
    tap.enable();

    log::info!("macOS hotkey listener started with CGEventTap(Session)");

    while !stop_signal.load(Ordering::Acquire) {
        context.borrow_mut().sync_binding(&shared_config);

        let needs_reenable = { context.borrow().needs_reenable };
        if needs_reenable {
            context.borrow_mut().needs_reenable = false;
            tap.enable();
            log::warn!(
                "macOS hotkey listener re-enabled event tap after timeout/user-input disable"
            );
        }

        let _ = CFRunLoop::run_in_mode(
            unsafe { kCFRunLoopDefaultMode },
            Duration::from_secs_f64(CONFIG_POLL_INTERVAL_SECS),
            false,
        );
    }

    run_loop_ptr.store(ptr::null_mut(), Ordering::Release);
    Ok(())
}

fn clone_hotkey_config(shared_config: &Arc<RwLock<AppConfig>>) -> crate::config::HotkeyConfig {
    let config = match shared_config.read() {
        Ok(cfg) => cfg.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    config.hotkey
}

fn resolve_binding(config: &crate::config::HotkeyConfig) -> Option<HotkeyBinding> {
    if config.vk_code == 0 {
        return None;
    }

    let mac_keycode = vk_to_mac_keycode(config.vk_code)?;
    Some(HotkeyBinding {
        vk_code: config.vk_code,
        mac_keycode,
        modifiers: config.modifiers,
        mode: config.mode,
        primary_modifier_flag: vk_to_primary_modifier_flag(config.vk_code),
    })
}

fn modifiers_match(
    flags: CGEventFlags,
    modifiers: u32,
    primary_modifier_flag: Option<CGEventFlags>,
) -> bool {
    let mut required = CGEventFlags::empty();
    if modifiers & 0x0001 != 0 {
        required |= CGEventFlags::CGEventFlagAlternate;
    }
    if modifiers & 0x0002 != 0 {
        required |= CGEventFlags::CGEventFlagControl;
    }
    if modifiers & 0x0004 != 0 {
        required |= CGEventFlags::CGEventFlagShift;
    }
    if modifiers & 0x0008 != 0 {
        required |= CGEventFlags::CGEventFlagCommand;
    }

    let modifier_mask = CGEventFlags::CGEventFlagAlternate
        | CGEventFlags::CGEventFlagControl
        | CGEventFlags::CGEventFlagShift
        | CGEventFlags::CGEventFlagCommand;

    let mut actual = flags & modifier_mask;
    if let Some(primary_flag) = primary_modifier_flag {
        actual.remove(primary_flag);
        required.remove(primary_flag);
    }

    actual == required
}

fn primary_key_pressed(
    event_type: CGEventType,
    flags: CGEventFlags,
    primary_modifier_flag: Option<CGEventFlags>,
) -> bool {
    match event_type {
        CGEventType::KeyDown => true,
        CGEventType::KeyUp => false,
        CGEventType::FlagsChanged => primary_modifier_flag
            .map(|flag| flags.contains(flag))
            .unwrap_or(false),
        _ => false,
    }
}

fn vk_to_primary_modifier_flag(vk_code: u32) -> Option<CGEventFlags> {
    match vk_code {
        0xA0 | 0xA1 => Some(CGEventFlags::CGEventFlagShift),
        0xA2 | 0xA3 => Some(CGEventFlags::CGEventFlagControl),
        0xA4 | 0xA5 => Some(CGEventFlags::CGEventFlagAlternate),
        0x5B | 0x5C => Some(CGEventFlags::CGEventFlagCommand),
        _ => None,
    }
}

fn vk_to_mac_keycode(vk_code: u32) -> Option<u16> {
    Some(match vk_code {
        0x08 => KeyCode::DELETE,
        0x09 => KeyCode::TAB,
        0x0D => KeyCode::RETURN,
        0x1B => KeyCode::ESCAPE,
        0x20 => KeyCode::SPACE,
        0x23 => KeyCode::END,
        0x24 => KeyCode::HOME,
        0x21 => KeyCode::PAGE_UP,
        0x22 => KeyCode::PAGE_DOWN,
        0x25 => KeyCode::LEFT_ARROW,
        0x26 => KeyCode::UP_ARROW,
        0x27 => KeyCode::RIGHT_ARROW,
        0x28 => KeyCode::DOWN_ARROW,
        0x2E => KeyCode::FORWARD_DELETE,
        0x30 => KeyCode::ANSI_0,
        0x31 => KeyCode::ANSI_1,
        0x32 => KeyCode::ANSI_2,
        0x33 => KeyCode::ANSI_3,
        0x34 => KeyCode::ANSI_4,
        0x35 => KeyCode::ANSI_5,
        0x36 => KeyCode::ANSI_6,
        0x37 => KeyCode::ANSI_7,
        0x38 => KeyCode::ANSI_8,
        0x39 => KeyCode::ANSI_9,
        0x41 => KeyCode::ANSI_A,
        0x42 => KeyCode::ANSI_B,
        0x43 => KeyCode::ANSI_C,
        0x44 => KeyCode::ANSI_D,
        0x45 => KeyCode::ANSI_E,
        0x46 => KeyCode::ANSI_F,
        0x47 => KeyCode::ANSI_G,
        0x48 => KeyCode::ANSI_H,
        0x49 => KeyCode::ANSI_I,
        0x4A => KeyCode::ANSI_J,
        0x4B => KeyCode::ANSI_K,
        0x4C => KeyCode::ANSI_L,
        0x4D => KeyCode::ANSI_M,
        0x4E => KeyCode::ANSI_N,
        0x4F => KeyCode::ANSI_O,
        0x50 => KeyCode::ANSI_P,
        0x51 => KeyCode::ANSI_Q,
        0x52 => KeyCode::ANSI_R,
        0x53 => KeyCode::ANSI_S,
        0x54 => KeyCode::ANSI_T,
        0x55 => KeyCode::ANSI_U,
        0x56 => KeyCode::ANSI_V,
        0x57 => KeyCode::ANSI_W,
        0x58 => KeyCode::ANSI_X,
        0x59 => KeyCode::ANSI_Y,
        0x5A => KeyCode::ANSI_Z,
        0x70 => KeyCode::F1,
        0x71 => KeyCode::F2,
        0x72 => KeyCode::F3,
        0x73 => KeyCode::F4,
        0x74 => KeyCode::F5,
        0x75 => KeyCode::F6,
        0x76 => KeyCode::F7,
        0x77 => KeyCode::F8,
        0x78 => KeyCode::F9,
        0x79 => KeyCode::F10,
        0x7A => KeyCode::F11,
        0x7B => KeyCode::F12,
        0xA0 => KeyCode::SHIFT,
        0xA1 => KeyCode::RIGHT_SHIFT,
        0xA2 => KeyCode::CONTROL,
        0xA3 => KeyCode::RIGHT_CONTROL,
        0xA4 => KeyCode::OPTION,
        0xA5 => KeyCode::RIGHT_OPTION,
        0x5B => KeyCode::COMMAND,
        0x5C => KeyCode::RIGHT_COMMAND,
        0xC0 => KeyCode::ANSI_GRAVE,
        _ => return None,
    })
}
