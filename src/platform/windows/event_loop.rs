//! Windows Event Loop Implementation
//!
//! Provides the hidden controller window and Win32 message pump for tray-first architecture.

use anyhow::Result;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::UpdateWindow;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostQuitMessage,
    RegisterClassW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HMENU,
    MSG, SW_HIDE, WM_DESTROY, WNDCLASSW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};

const CONTROLLER_CLASS_NAME: &str = "voice-ime-controller-window";

/// Create the hidden controller window for tray-first architecture
pub fn create_controller_window() -> Result<HWND> {
    let hinstance = unsafe { GetModuleHandleW(None)? };
    let class_name = encode_wide(CONTROLLER_CLASS_NAME);
    let wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
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
        let _ = UpdateWindow(hwnd);
    }

    log::info!("Controller window created (hwnd={:?})", hwnd.0);
    Ok(hwnd)
}

/// Destroy the controller window
pub fn destroy_controller_window(hwnd: HWND) -> Result<()> {
    unsafe {
        DestroyWindow(hwnd)?;
    }
    log::info!("Controller window destroyed");
    Ok(())
}

/// Run the Win32 message loop
pub fn run_message_loop() -> Result<()> {
    let mut msg = MSG::default();

    loop {
        let ret = unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) };
        if ret.0 <= 0 {
            log::info!("Message loop exiting (ret={})", ret.0);
            break;
        }

        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

/// Window procedure for the controller window
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

fn encode_wide(text: &str) -> Vec<u16> {
    OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
