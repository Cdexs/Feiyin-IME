//! macOS Accessibility permission check and request

use anyhow::Result;

/// Check if the app has Accessibility permission
/// Returns true if granted
pub fn is_accessibility_granted() -> bool {
    // FFI: AXIsProcessTrusted()
    // from ApplicationServices.framework
    unsafe { ax_is_process_trusted() }
}

/// Request Accessibility permission with system prompt
/// This will open System Settings → Privacy & Security → Accessibility
pub fn request_accessibility_permission() {
    // FFI: AXIsProcessTrustedWithOptions({ AXTrustedCheckOptionPrompt: true })
    unsafe {
        ax_is_process_trusted_with_prompt();
    }
}

/// Check at startup; if not granted, show our dialog then system prompt
pub fn ensure_accessibility_at_startup() -> Result<()> {
    if !is_accessibility_granted() {
        log::warn!("Accessibility permission not granted");
        // TODO: show in-app dialog before calling system prompt
        // For now, directly trigger system prompt
        request_accessibility_permission();
    }
    Ok(())
}

// Raw FFI bindings to ApplicationServices
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

unsafe fn ax_is_process_trusted() -> bool {
    AXIsProcessTrusted()
}

unsafe fn ax_is_process_trusted_with_prompt() {
    // TODO: call AXIsProcessTrustedWithOptions with kAXTrustedCheckOptionPrompt=true
    // Requires CoreFoundation CFDictionary binding
    // Stub for now — will be implemented when macOS build env is ready
    log::info!("ax_is_process_trusted_with_prompt: stub, opening System Settings manually");
}
