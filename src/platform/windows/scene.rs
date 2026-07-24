//! SCENE-SENSE-001-CORE: 场景信号采集（Windows 实现）
//!
//! 录音启动瞬间从前台窗口同步采集进程名 + 窗口标题（微秒级，不阻塞热键线程）。
//! 失败一律降级 None（→ Unknown 安全降级）。

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
};

/// 从前台窗口 HWND 同步采集场景信号（进程 exe 名 + 窗口标题）。
/// 返回 None 时调用方降级为 Unknown 场景。
///
/// 性能：GetWindowThreadProcessId + OpenProcess + QueryFullProcessImageNameW + GetWindowTextW
/// 均为微秒级 Win32 调用，不阻塞热键线程。
pub fn capture_scene_signals(hwnd: HWND) -> Option<(String, String)> {
    if hwnd.0.is_null() {
        return None;
    }

    let exe = capture_process_exe(hwnd)?;
    let title = capture_window_title(hwnd);
    Some((exe, title))
}

/// 获取窗口所属进程的 exe 全路径，再提取文件名。
fn capture_process_exe(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
        if pid == 0 {
            return None;
        }

        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        // HANDLE 是 Copy 类型无 Drop，必须显式 CloseHandle，否则每次录音泄漏一个进程句柄
        let exe_path = query_full_process_image_name(handle);
        let _ = CloseHandle(handle);
        extract_exe_name(&exe_path?)
    }
}

unsafe fn query_full_process_image_name(
    handle: windows::Win32::Foundation::HANDLE,
) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len: u32 = buf.len() as u32;
    let result = QueryFullProcessImageNameW(
        handle,
        PROCESS_NAME_FORMAT(0),
        windows::core::PWSTR(buf.as_mut_ptr()),
        &mut len as *mut u32,
    );
    if result.is_err() || len == 0 {
        return None;
    }
    let os_str = OsString::from_wide(&buf[..len as usize]);
    os_str.to_str().map(|s| s.to_string())
}

/// 从完整路径提取 exe 文件名（如 "C:\...\WeChat.exe" → "WeChat.exe"）。
fn extract_exe_name(path: &str) -> Option<String> {
    path.rsplit(|c| c == '\\' || c == '/')
        .next()
        .map(|s| s.to_string())
}

/// 获取窗口标题（UTF-16 → String）。失败返回空字符串（title_keywords 兜底仍可用空标题→跳过）。
fn capture_window_title(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len as usize) + 1];
        let copied = GetWindowTextW(hwnd, &mut buf);
        if copied <= 0 {
            return String::new();
        }
        let os_str = OsString::from_wide(&buf[..copied as usize]);
        os_str.to_string_lossy().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_exe_name_from_full_path() {
        assert_eq!(
            extract_exe_name(r"C:\Program Files\WeChat\WeChat.exe"),
            Some("WeChat.exe".to_string())
        );
        assert_eq!(
            extract_exe_name(r"C:\Windows\System32\cmd.exe"),
            Some("cmd.exe".to_string())
        );
        assert_eq!(
            extract_exe_name("WeChat.exe"),
            Some("WeChat.exe".to_string())
        );
        assert_eq!(extract_exe_name(""), Some("".to_string()));
    }

    #[test]
    fn extract_exe_name_handles_forward_slash() {
        assert_eq!(extract_exe_name("/usr/bin/code"), Some("code".to_string()));
    }
}
