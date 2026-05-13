//! Windows Text Injection using SendInput and Clipboard

use anyhow::{anyhow, Result};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::PWSTR;
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND, LPARAM, WPARAM};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_CONTROL,
    VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, SendMessageW, GUITHREADINFO,
    WM_GETTEXT, WM_GETTEXTLENGTH,
};

/// Snapshot of focused text field (hwnd + text content)
#[derive(Debug, Clone)]
pub struct FocusedTextSnapshot {
    pub hwnd: HWND,
    pub text: String,
}

/// Inject text into the currently focused input field.
pub fn inject_text(text: &str, use_clipboard: bool, delay_ms: u64) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    if use_clipboard {
        match inject_via_clipboard(text, delay_ms) {
            Ok(_) => return Ok(()),
            Err(e) => log::warn!(
                "Clipboard injection failed: {}, falling back to SendInput",
                e
            ),
        }
    }
    inject_via_send_input(text)
}

/// Copy text to system clipboard (alias for main.rs compatibility)
pub fn copy_text_to_clipboard(text: &str) -> Result<()> {
    set_clipboard_text(text)
}

/// Capture current focused text field as snapshot (hwnd + text)
pub fn capture_focused_text_snapshot() -> Option<FocusedTextSnapshot> {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.0.is_null() {
        return None;
    }

    let thread_id = unsafe { GetWindowThreadProcessId(foreground, None) };
    if thread_id == 0 {
        return None;
    }

    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };

    let hwnd = unsafe {
        if GetGUIThreadInfo(thread_id, &mut info).is_ok() && !info.hwndFocus.0.is_null() {
            info.hwndFocus
        } else {
            foreground
        }
    };

    Some(FocusedTextSnapshot {
        hwnd,
        text: read_text_from_hwnd(hwnd)?,
    })
}

/// Read text from a specific window handle
pub fn read_text_from_hwnd(hwnd: HWND) -> Option<String> {
    if hwnd.0.is_null() {
        return None;
    }

    unsafe {
        let len = SendMessageW(hwnd, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0))
            .0
            .max(0) as usize;
        let mut buffer = vec![0u16; len.saturating_add(1)];
        let copied = SendMessageW(
            hwnd,
            WM_GETTEXT,
            WPARAM(buffer.len()),
            LPARAM(PWSTR(buffer.as_mut_ptr()).0 as isize),
        )
        .0
        .max(0) as usize;
        buffer.truncate(copied);
        Some(String::from_utf16_lossy(&buffer))
    }
}

fn inject_via_clipboard(text: &str, delay_ms: u64) -> Result<()> {
    let old_content = get_clipboard_text();
    set_clipboard_text(text)?;
    std::thread::sleep(std::time::Duration::from_millis(50));
    send_ctrl_v()?;
    std::thread::sleep(std::time::Duration::from_millis(delay_ms.max(50)));
    if let Some(old) = old_content {
        let _ = set_clipboard_text(&old);
    }
    Ok(())
}

fn set_clipboard_text(text: &str) -> Result<()> {
    let wide: Vec<u16> = OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let byte_len = wide.len() * 2;

    unsafe {
        OpenClipboard(HWND::default())?;

        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err(anyhow!("EmptyClipboard failed"));
        }

        let hglobal = GlobalAlloc(GMEM_MOVEABLE, byte_len).map_err(|e| {
            let _ = CloseClipboard();
            anyhow!("GlobalAlloc failed: {}", e)
        })?;

        let ptr = GlobalLock(hglobal) as *mut u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err(anyhow!("GlobalLock returned null"));
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        let _ = GlobalUnlock(hglobal);

        // CF_UNICODETEXT = 13
        if SetClipboardData(13, HANDLE(hglobal.0)).is_err() {
            let _ = CloseClipboard();
            return Err(anyhow!("SetClipboardData failed"));
        }
        CloseClipboard()?;
    }
    Ok(())
}

fn get_clipboard_text() -> Option<String> {
    unsafe {
        OpenClipboard(HWND::default()).ok()?;
        let handle = GetClipboardData(13).ok()?;
        let hglobal: HGLOBAL = std::mem::transmute(handle);
        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return None;
        }
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let text = String::from_utf16_lossy(slice);
        let _ = GlobalUnlock(hglobal);
        let _ = CloseClipboard();
        Some(text)
    }
}

fn send_ctrl_v() -> Result<()> {
    let inputs = [
        make_vk_input(VK_CONTROL.0, false),
        make_vk_input(VK_V.0, false),
        make_vk_input(VK_V.0, true),
        make_vk_input(VK_CONTROL.0, true),
    ];
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    Ok(())
}

fn make_vk_input(vk: u16, key_up: bool) -> INPUT {
    let mut input = INPUT {
        r#type: INPUT_KEYBOARD,
        ..Default::default()
    };
    input.Anonymous.ki.wVk = VIRTUAL_KEY(vk);
    if key_up {
        input.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
    }
    input
}

fn inject_via_send_input(text: &str) -> Result<()> {
    let inputs: Vec<INPUT> = text
        .encode_utf16()
        .flat_map(|wc| {
            let mut down = INPUT {
                r#type: INPUT_KEYBOARD,
                ..Default::default()
            };
            let mut up = INPUT {
                r#type: INPUT_KEYBOARD,
                ..Default::default()
            };
            down.Anonymous.ki.wScan = wc;
            down.Anonymous.ki.dwFlags = KEYEVENTF_UNICODE;
            up.Anonymous.ki.wScan = wc;
            up.Anonymous.ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP;
            [down, up]
        })
        .collect();

    if !inputs.is_empty() {
        unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    }
    Ok(())
}
