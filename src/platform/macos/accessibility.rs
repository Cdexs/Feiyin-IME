//! macOS Accessibility permission check and request
//!
//! MACOS-P4-PERM-001: 真实现 `AXIsProcessTrustedWithOptions` 弹窗。
//! 原实现 `ax_is_process_trusted_with_prompt` 为 stub（仅 log，未调用任何 API），
//! 导致用户未授权时看不到系统弹窗、热键静默失效。本文件补全 FFI 绑定与调用。
//!
//! 纯 macOS 平台代码，对 Windows 无影响（见 docs/MACOS-HANDOFF.md §2.10）。

use anyhow::Result;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringRef};

/// Check if the app has Accessibility permission
/// Returns true if granted
pub fn is_accessibility_granted() -> bool {
    unsafe { ax_is_process_trusted() }
}

/// Request Accessibility permission with system prompt
/// This will open System Settings → Privacy & Security → Accessibility
pub fn request_accessibility_permission() {
    unsafe {
        ax_is_process_trusted_with_prompt();
    }
}

/// Check at startup; if not granted, trigger system prompt.
///
/// MACOS-P4-PERM-001: 不阻断启动（与 Windows 侧对齐——RegisterHotKey 失败也不阻断）。
/// macOS 授权后通常需重启应用才生效，阻断没有意义。
pub fn ensure_accessibility_at_startup() -> Result<()> {
    if !is_accessibility_granted() {
        log::warn!(
            "Accessibility permission not granted; system prompt will be shown. \
             Hotkeys will NOT work until accessibility is granted in \
             System Settings → Privacy & Security → Accessibility, then the app is restarted."
        );
        request_accessibility_permission();
    }
    Ok(())
}

// Raw FFI bindings to ApplicationServices
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

unsafe fn ax_is_process_trusted() -> bool {
    AXIsProcessTrusted()
}

/// MACOS-P4-PERM-001: 调用 `AXIsProcessTrustedWithOptions`，传入
/// `{ kAXTrustedCheckOptionPrompt: kCFBooleanTrue }` 触发系统授权弹窗。
///
/// 弹窗含「打开系统设置」按钮，用户可直达 Privacy & Security → Accessibility。
/// 此函数仅触发弹窗，不阻断；授权需用户在系统设置中操作后重启应用才生效。
unsafe fn ax_is_process_trusted_with_prompt() {
    // 构造 CFString key（kAXTrustedCheckOptionPrompt 是 framework 导出的全局常量，
    // 用 wrap_under_get_rule 包成 CFString，不获取所有权——它是进程级静态量）。
    let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
    let value = CFBoolean::true_value();
    // CFDictionary::from_CFType_pairs 接受 &[(K, V)]，K/V 需为 CFType。
    // CFString 与 CFBoolean 均实现 TCFType，转 CFType 作为字典元素类型。
    let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
    // AXIsProcessTrustedWithOptions 接受 CFDictionaryRef（裸指针），
    // CFDictionary 实现 TCFType，as_CFType() 返回 CFType，as_concrete_TypeRef() 取裸指针。
    let _trusted = AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as CFDictionaryRef);
    // 返回值表示调用时刻是否已授权；此处仅用于触发弹窗，忽略返回值
    // （调用方 ensure_accessibility_at_startup 已先检查 is_accessibility_granted）。
    log::info!("AXIsProcessTrustedWithOptions called (system prompt triggered)");
}

/// 裸 CFDictionaryRef 类型别名（与 core-foundation-sys 的裸指针签名对齐）。
type CFDictionaryRef = *const std::ffi::c_void;
