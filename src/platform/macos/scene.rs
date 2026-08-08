//! MACOS-P4-SCENE-002-CORE: 场景信号采集（macOS 真实实现，焦点身份 = 窗口级 CGWindowID）
//!
//! 录音启动瞬间同步采集前台窗口进程名 + 窗口标题（微秒级，不阻塞热键线程）。
//! 失败一律降级 None（→ Unknown 安全降级），与 Windows 侧 `capture_scene_signals`
//! 语义一致：exe 取不到 → 整体 None；title 取不到 → 空字符串。
//!
//! 焦点身份（SCENE-002 从 pid 提升到 CGWindowID）：
//!   - `foreground_window_id()` 返回**窗口级**标识（`kCGWindowNumber`），同应用
//!     多窗口（如两个 Word 文档）前台切换产生不同 id，`focus_lost` 可正确判定。
//!     路径：frontmost pid（NSWorkspace/AX）→ `CGWindowListCopyWindowInfo`
//!     → 第一个 `owner_pid == frontmost && layer == 0` 条目的 `kCGWindowNumber`。
//!   - `capture_scene_signals(id)` 用 `id != 0` 的 CGWindowID **反查 owner pid**
//!     （录音开始时捕获、管线后期用它取场景，对齐 Windows HWND 语义）；
//!     `id == 0` 或窗口已关闭 → 降级实时查 frontmost。
//!
//! 权限边界（SCENE-002 核心约束）：
//!   - 窗口标识 `kCGWindowNumber`/`kCGWindowOwnerPID`/`kCGWindowLayer`：无需任何权限。
//!   - 🔴 绝不读 `kCGWindowName`（macOS 10.15+ 需要「屏幕录制」权限，吓人的权限，
//!     为一个场景分类字段不值得，会抬高安装门槛）。
//!   - 标题继续走 AX 路径（`kAXFocusedWindowAttribute`→`kAXTitleAttribute`），
//!     用的是 SCENE-001 已有的辅助功能权限。
//!
//! 已知残差（SCENE-002 明令本轮不修）：标题仍是「该应用当前 AX 焦点窗口」的标题，
//! 而非 CGWindowID 指定的那个窗口。同应用多窗口时可能不一致——但已被 A 项大幅削弱：
//! 用户切窗口后 `focus_lost` 正确触发走 FocusLost 分支，不再默默用错窗口场景。
//! 要做到标题也精确到窗口需私有 API `_AXUIElementGetWindow`（不稳定/有分发风险），
//! 明令不做。
//!
//! 实现路径（全程不依赖 `NSWorkspace` feature，避免改 Cargo.toml）：
//!   - pid 来源：`NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier`
//!     （objc2 `msg_send` 动态调用，无需辅助功能权限）；失败降级
//!     `AX kAXFocusedApplicationAttribute` → `AXUIElementGetPid`。
//!   - exe 名：pid → `proc_pidpath`（libSystem）取可执行文件全路径 → basename。
//!     （与 `NSRunningApplication.executableURL().lastPathComponent()` 等价——
//!     含空格的可执行名如 "Google Chrome" 会原样保留，见 scene-rules.toml 追加表。）
//!   - 标题：`AXUIElementCreateApplication(pid)` → `kAXFocusedWindowAttribute`
//!     → `kAXTitleAttribute`。
//!
//! 契约（对齐 Windows `capture_scene_signals`）：
//!   - exe 取不到 → 整体返回 None（→ Unknown 场景）
//!   - title 取不到 → 返回空字符串（title_keywords 兜底可用空标题→跳过）
//!   - AX 调用一律 `AXUIElementSetMessagingTimeout(0.5)`，避免无响应 App 阻塞
//!     管线线程（理由同 AXINJECT-001）。
//!
//! 头部/独立 FFI 结构仿 `injection.rs`（MACOS-P4-AXINJECT-001 boundary rule）。
//! AX 常量（kAX*）在 SDK 中是 `#define CFSTR("...")` 宏、非导出符号，故用
//! `CFString::new` 内联字面量（语义与 CFSTR 宏完全等价）。

use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::window::{
    copy_window_info, kCGWindowLayer, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly, kCGWindowNumber, kCGWindowOwnerPID, CGWindowID,
};
use std::ffi::{c_int, c_void};
use std::ptr;

type pid_t = c_int;
type AXUIElementRef = *const c_void;
type AXError = i32;

const AX_ERR_SUCCESS: AXError = 0;
const AX_MSG_TIMEOUT_SECS: f32 = 0.5;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: pid_t) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut pid_t) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeoutInSeconds: f32) -> AXError;
}

/// `proc_pidpath` 位于 libSystem，自动链接，无需显式 `#[link]`。
extern "C" {
    fn proc_pidpath(pid: pid_t, buffer: *mut c_void, buffersize: u32) -> c_int;
}

/// 采集场景信号（进程 exe 名 + 窗口标题）。
///
/// `id` 为录音开始时捕获的 frontmost **CGWindowID**（`foreground_window_id()` 返回值）：
///   - `id != 0` → 用该 CGWindowID 反查 owner pid 取 exe 名 + AX 标题（与 Windows
///     HWND 语义对齐，管线后期即使焦点已切走，场景仍是录音开始时的窗口）。
///   - `id == 0` 或窗口已关闭 → 降级为实时查 frontmost（兜底路径）。
///
/// 返回 None 时调用方降级为 Unknown 场景。
pub fn capture_scene_signals(id: usize) -> Option<(String, String)> {
    let pid = if id != 0 {
        window_id_to_owner_pid(id as CGWindowID).or_else(frontmost_application_pid)
    } else {
        frontmost_application_pid()
    };

    let pid = pid?;
    let exe = process_exe_name(pid)?;
    let title = window_title_of_pid(pid);
    Some((exe, title))
}

/// MACOS-P4-NEUTRAL-001: 平台中立 by-id 版本，供 `run_pipeline_core` 调用。
/// 委托给 `capture_scene_signals`（`WindowId` 即 frontmost CGWindowID，见 `foreground_window_id`）。
pub fn capture_scene_signals_by_id(id: crate::platform::WindowId) -> Option<(String, String)> {
    capture_scene_signals(id)
}

/// 返回当前前台**窗口**的 CGWindowID（`WindowId = usize` 语义）。取不到返回 0。
///
/// MACOS-P4-SCENE-002：从 pid（应用级）提升到 CGWindowID（窗口级）——同应用多窗口
/// 前台切换产生不同 id，`focus_lost` 可正确判定（Gavin 指令「力度必须要提到窗口级」）。
/// 路径：frontmost pid → `CGWindowListCopyWindowInfo` → 第一个
/// `owner_pid == frontmost && layer == 0` 条目的 `kCGWindowNumber`。
/// `layer == 0` 是普通窗口层，排除菜单栏/Dock/浮层/系统 UI（非 0 一律跳过）。
pub fn foreground_window_id() -> crate::platform::WindowId {
    frontmost_cg_window_id().unwrap_or(0) as crate::platform::WindowId
}

/// 用 CGWindowID 反查 owner pid（B 项时序语义：录音开始捕获的窗口，管线后期仍用它）。
/// 找不到（窗口已关闭）→ None（调用方降级实时查 frontmost）。
fn window_id_to_owner_pid(window_id: CGWindowID) -> Option<pid_t> {
    let list = copy_window_info(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        0,
    )?;
    for item in list.iter() {
        let raw: *const c_void = *item;
        let cf_type = unsafe { CFType::wrap_under_get_rule(raw) };
        let dict = cf_type.downcast::<CFDictionary<*const c_void, *const c_void>>();
        let dict = match dict {
            Some(d) => d,
            None => continue,
        };
        let (key_number, key_pid) = unsafe {
            (
                kCGWindowNumber as *const c_void,
                kCGWindowOwnerPID as *const c_void,
            )
        };
        let num_val = |key: *const c_void| {
            dict.find(key)
                .map(|v| unsafe { CFType::wrap_under_get_rule(*v) })
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_i32())
        };
        if num_val(key_number) == Some(window_id as i32) {
            let pid = num_val(key_pid)?;
            if pid > 0 {
                return Some(pid);
            }
        }
    }
    None
}

/// frontmost pid 对应的前台窗口 CGWindowID。取不到 → None。
fn frontmost_cg_window_id() -> Option<CGWindowID> {
    let frontmost = frontmost_application_pid()?;
    let list = copy_window_info(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        0,
    )?;
    for item in list.iter() {
        let raw: *const c_void = *item;
        let cf_type = unsafe { CFType::wrap_under_get_rule(raw) };
        let dict = cf_type.downcast::<CFDictionary<*const c_void, *const c_void>>();
        let dict = match dict {
            Some(d) => d,
            None => continue,
        };
        let (key_number, key_layer, key_pid) = unsafe {
            (
                kCGWindowNumber as *const c_void,
                kCGWindowLayer as *const c_void,
                kCGWindowOwnerPID as *const c_void,
            )
        };
        let num_val = |key: *const c_void| {
            dict.find(key)
                .map(|v| unsafe { CFType::wrap_under_get_rule(*v) })
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_i32())
        };
        let pid = num_val(key_pid);
        let layer = num_val(key_layer).unwrap_or(i32::MAX);
        if pid == Some(frontmost) && layer == 0 {
            let window_id = num_val(key_number)?;
            if window_id > 0 {
                return Some(window_id as CGWindowID);
            }
        }
    }
    None
}

/// 前台应用 pid。先试 NSWorkspace（无需辅助功能权限），再试 AX，均失败 → None。
fn frontmost_application_pid() -> Option<pid_t> {
    ns_workspace_frontmost_pid().or_else(ax_frontmost_pid)
}

/// `NSWorkspace.sharedWorkspace.frontmostApplication` → `processIdentifier`。
/// 用 objc2 `msg_send` 动态调用，避免依赖 `NSWorkspace` feature。
fn ns_workspace_frontmost_pid() -> Option<pid_t> {
    use objc2::runtime::AnyObject;
    use objc2::{msg_send, msg_send_id};

    unsafe {
        let cls = objc2::runtime::AnyClass::get("NSWorkspace")?;
        let shared: objc2::rc::Retained<AnyObject> = msg_send_id![cls, sharedWorkspace];
        let frontmost: objc2::rc::Retained<AnyObject> = msg_send_id![&shared, frontmostApplication];
        let pid: i32 = msg_send![&frontmost, processIdentifier];
        if pid > 0 {
            Some(pid as pid_t)
        } else {
            None
        }
    }
}

/// `AX kAXFocusedApplicationAttribute` → `AXUIElementGetPid`（NSWorkspace 不可用的降级路径）。
fn ax_frontmost_pid() -> Option<pid_t> {
    unsafe {
        let system_wide_ref = AXUIElementCreateSystemWide();
        if system_wide_ref.is_null() {
            return None;
        }
        let system_wide = CFType::wrap_under_create_rule(system_wide_ref as CFTypeRef);
        let _ = AXUIElementSetMessagingTimeout(
            system_wide.as_concrete_TypeRef() as AXUIElementRef,
            AX_MSG_TIMEOUT_SECS,
        );

        let attr = CFString::new("AXFocusedApplication");
        let mut app_ref: CFTypeRef = ptr::null();
        if AXUIElementCopyAttributeValue(
            system_wide.as_concrete_TypeRef() as AXUIElementRef,
            attr.as_concrete_TypeRef(),
            &mut app_ref,
        ) != AX_ERR_SUCCESS
            || app_ref.is_null()
        {
            return None;
        }
        let app = CFType::wrap_under_create_rule(app_ref);

        let mut pid: pid_t = 0;
        if AXUIElementGetPid(app.as_concrete_TypeRef() as AXUIElementRef, &mut pid)
            != AX_ERR_SUCCESS
            || pid <= 0
        {
            return None;
        }
        Some(pid)
    }
}

/// pid → 可执行文件路径 basename（如 ".../MacOS/WeChat" → "WeChat"、
/// ".../MacOS/Google Chrome" → "Google Chrome"）。失败 → None。
fn process_exe_name(pid: pid_t) -> Option<String> {
    let mut buf = [0u8; 4096];
    let n = unsafe { proc_pidpath(pid, buf.as_mut_ptr() as *mut c_void, buf.len() as u32) };
    if n <= 0 {
        return None;
    }
    let path = std::str::from_utf8(&buf[..n as usize]).ok()?;
    let name = path.rsplit('/').next().unwrap_or("").to_string();
    Some(name)
}

/// 指定 pid 的窗口标题。拿不到 → 空字符串（title_keywords 兜底可用空标题→跳过）。
fn window_title_of_pid(pid: pid_t) -> String {
    unsafe {
        let app_ref = AXUIElementCreateApplication(pid);
        if app_ref.is_null() {
            return String::new();
        }
        let app = CFType::wrap_under_create_rule(app_ref as CFTypeRef);
        let _ = AXUIElementSetMessagingTimeout(
            app.as_concrete_TypeRef() as AXUIElementRef,
            AX_MSG_TIMEOUT_SECS,
        );

        let wattr = CFString::new("AXFocusedWindow");
        let mut window_ref: CFTypeRef = ptr::null();
        if AXUIElementCopyAttributeValue(
            app.as_concrete_TypeRef() as AXUIElementRef,
            wattr.as_concrete_TypeRef(),
            &mut window_ref,
        ) != AX_ERR_SUCCESS
            || window_ref.is_null()
        {
            return String::new();
        }
        let window = CFType::wrap_under_create_rule(window_ref);

        let tattr = CFString::new("AXTitle");
        let mut title_ref: CFTypeRef = ptr::null();
        if AXUIElementCopyAttributeValue(
            window.as_concrete_TypeRef() as AXUIElementRef,
            tattr.as_concrete_TypeRef(),
            &mut title_ref,
        ) != AX_ERR_SUCCESS
            || title_ref.is_null()
        {
            return String::new();
        }
        let title = CFString::wrap_under_create_rule(title_ref as CFStringRef);
        title.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_exe_name_from_full_path() {
        let cases = [
            (
                "/System/Applications/Safari.app/Contents/MacOS/Safari",
                "Safari",
            ),
            ("/Applications/WeChat.app/Contents/MacOS/WeChat", "WeChat"),
            (
                "/Applications/Visual Studio Code.app/Contents/MacOS/Electron",
                "Electron",
            ),
            ("WeChat", "WeChat"),
        ];
        for (path, expected) in cases {
            assert_eq!(path.rsplit('/').next().unwrap(), expected);
        }
    }

    #[test]
    fn executable_name_with_spaces_preserved() {
        assert_eq!(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
                .rsplit('/')
                .next(),
            Some("Google Chrome")
        );
        assert_eq!(
            "/Applications/Sublime Text.app/Contents/MacOS/Sublime Text"
                .rsplit('/')
                .next(),
            Some("Sublime Text")
        );
    }

    #[test]
    fn empty_path_yields_empty_name() {
        assert_eq!("".rsplit('/').next(), Some(""));
    }
}
