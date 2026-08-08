//! macOS text injection implementation using enigo and native clipboard tools.
//!
//! MACOS-P4-AXINJECT-001: 新增 Accessibility API 直写第一级（AX → clipboard → enigo），
//! 解决剪贴板注入覆盖用户非纯文本剪贴板内容、污染剪贴板历史等问题。
//! 仅对 macOS 生效，Windows 走 src/platform/windows/injection.rs，零影响。

use anyhow::{anyhow, Context, Result};
use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_foundation_sys::base::Boolean;
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct FocusedTextSnapshot {
    pub hwnd: usize,
    pub text: String,
}

pub fn inject_text(text: &str, use_clipboard: bool, delay_ms: u64) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    // Tier 1: Accessibility API direct write — never touches the clipboard.
    match inject_via_ax(text) {
        Ok(role) => {
            log::info!("text injected via AX (role={role}, clipboard untouched)");
            return Ok(());
        }
        Err(err) => log::info!(
            "AX injection unavailable ({}), falling back to clipboard",
            err
        ),
    }

    if use_clipboard {
        match inject_via_clipboard(text, delay_ms) {
            Ok(()) => return Ok(()),
            Err(err) => log::warn!(
                "macOS clipboard injection failed: {}, falling back to enigo.text()",
                err
            ),
        }
    }

    inject_via_enigo_text(text)
}

pub fn copy_text_to_clipboard(text: &str) -> Result<()> {
    set_clipboard_text(text)
}

pub fn capture_focused_text_snapshot() -> Option<FocusedTextSnapshot> {
    None
}

pub fn read_text_from_hwnd(_hwnd: usize) -> Option<String> {
    None
}

/// Tier 1: inject text via macOS Accessibility API using kAXSelectedTextAttribute.
///
/// - Uses `kAXSelectedTextAttribute` (insert / replace current selection), never
///   `kAXValueAttribute` (which would overwrite the entire field).
/// - Sets a 0.5s messaging timeout on both the system-wide element and the focused
///   element so a hung target app cannot block the pipeline worker thread.
/// - Returns Ok(role) only when `kAXSelectedTextAttribute` is reported as settable
///   and `AXUIElementSetAttributeValue` reports `kAXErrorSuccess`.
/// - The returned `role` is a best-effort description of the focused element's
///   `AXRole`, used for diagnostic logging only.
/// - All CF objects created with Create/Copy semantics are wrapped with
///   `CFType::wrap_under_create_rule` and released on drop.
fn inject_via_ax(text: &str) -> Result<String> {
    unsafe {
        // System-wide element: Create rule → caller owns the reference.
        let system_wide_ref = AXUIElementCreateSystemWide();
        if system_wide_ref.is_null() {
            return Err(anyhow!("AXUIElementCreateSystemWide returned null"));
        }
        let _system_wide = CFType::wrap_under_create_rule(system_wide_ref as CFTypeRef);

        // Global messaging timeout; positive values only.
        let err = AXUIElementSetMessagingTimeout(system_wide_ref, AX_MSG_TIMEOUT_SECS);
        if err != AX_ERR_SUCCESS {
            return Err(anyhow!(
                "AXUIElementSetMessagingTimeout(system-wide) failed: {err}"
            ));
        }

        // Copy the currently focused UI element (Copy rule → caller owns).
        // AX 常量在 SDK 中是 `#define CFSTR(...)` 宏非导出符号，改用字面量（同 scene.rs）。
        let focused_attr = CFString::new("AXFocusedUIElement");
        let mut focused_ref: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(
            system_wide_ref,
            focused_attr.as_concrete_TypeRef(),
            &mut focused_ref,
        );
        if err != AX_ERR_SUCCESS {
            return Err(anyhow!(
                "AXUIElementCopyAttributeValue(kAXFocusedUIElementAttribute) failed: {err}"
            ));
        }
        if focused_ref.is_null() {
            return Err(anyhow!("focused UI element is null"));
        }
        let _focused = CFType::wrap_under_create_rule(focused_ref);

        // Per-element messaging timeout.
        let err =
            AXUIElementSetMessagingTimeout(focused_ref as AXUIElementRef, AX_MSG_TIMEOUT_SECS);
        if err != AX_ERR_SUCCESS {
            return Err(anyhow!(
                "AXUIElementSetMessagingTimeout(focused) failed: {err}"
            ));
        }

        // Pre-check: does the focused element actually support writing selected text?
        // Some apps (Electron / Java Swing) partially support AX and return success on
        // SetAttributeValue while silently dropping the text. IsAttributeSettable is
        // an earlier, more reliable gate than the post-write return code.
        let selected_attr = CFString::new("AXSelectedText");
        let mut settable: Boolean = 0;
        let err = AXUIElementIsAttributeSettable(
            focused_ref as AXUIElementRef,
            selected_attr.as_concrete_TypeRef(),
            &mut settable,
        );
        if err != AX_ERR_SUCCESS || settable == 0 {
            return Err(anyhow!(
                "kAXSelectedTextAttribute is not settable (err={err}, settable={settable})"
            ));
        }

        // Set selected text to the desired value.
        let value = CFString::new(text);
        let err = AXUIElementSetAttributeValue(
            focused_ref as AXUIElementRef,
            selected_attr.as_concrete_TypeRef(),
            value.as_concrete_TypeRef() as CFTypeRef,
        );
        if err != AX_ERR_SUCCESS {
            return Err(anyhow!(
                "AXUIElementSetAttributeValue(kAXSelectedTextAttribute) failed: {err}"
            ));
        }

        // Best-effort role lookup for diagnostic logging; never fails the injection.
        Ok(ax_role_description(focused_ref as AXUIElementRef))
    }
}

/// Best-effort focused element role description for logs.
/// Returns "unknown" if the role attribute is unavailable or not a string.
fn ax_role_description(element: AXUIElementRef) -> String {
    unsafe {
        let role_attr = CFString::new("AXRole");
        let mut role_ref: CFTypeRef = std::ptr::null();
        let err =
            AXUIElementCopyAttributeValue(element, role_attr.as_concrete_TypeRef(), &mut role_ref);
        if err != AX_ERR_SUCCESS || role_ref.is_null() {
            return "unknown".to_string();
        }
        let role = CFString::wrap_under_create_rule(role_ref as CFStringRef);
        role.to_string()
    }
}

fn inject_via_clipboard(text: &str, delay_ms: u64) -> Result<()> {
    let old_content = get_clipboard_text().ok();
    set_clipboard_text(text)?;

    thread::sleep(Duration::from_millis(50));
    let send_result = send_command_v();
    if send_result.is_ok() {
        thread::sleep(Duration::from_millis(delay_ms.max(50)));
    }

    if let Some(old) = old_content {
        if let Err(e) = set_clipboard_text(&old) {
            log::warn!(
                "clipboard restore FAILED, user's original clipboard content is lost: {}",
                e
            );
        }
    }

    send_result
}

fn inject_via_enigo_text(text: &str) -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|err| anyhow!("failed to initialize enigo: {err}"))?;
    enigo
        .text(text)
        .map_err(|err| anyhow!("failed to inject text via enigo: {err}"))?;
    Ok(())
}

fn send_command_v() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|err| anyhow!("failed to initialize enigo: {err}"))?;

    enigo
        .key(Key::Meta, Press)
        .map_err(|err| anyhow!("failed to press Command: {err}"))?;
    enigo
        .key(Key::Unicode('v'), Click)
        .map_err(|err| anyhow!("failed to send v key: {err}"))?;
    enigo
        .key(Key::Meta, Release)
        .map_err(|err| anyhow!("failed to release Command: {err}"))?;

    Ok(())
}

fn set_clipboard_text(text: &str) -> Result<()> {
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn pbcopy")?;

    let mut stdin = child.stdin.take().context("pbcopy stdin unavailable")?;
    stdin
        .write_all(text.as_bytes())
        .context("failed to write text to pbcopy")?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .context("failed waiting for pbcopy")?;
    if !output.status.success() {
        return Err(anyhow!(
            "pbcopy failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

fn get_clipboard_text() -> Result<String> {
    let output = Command::new("pbpaste")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("failed to run pbpaste")?;

    if !output.status.success() {
        return Err(anyhow!(
            "pbpaste failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// Raw FFI bindings to ApplicationServices/HIServices.
// Self-contained in this file per MACOS-P4-AXINJECT-001 boundary rule.
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeoutInSeconds: f32) -> AXError;
    fn AXUIElementIsAttributeSettable(
        element: AXUIElementRef,
        attribute: CFStringRef,
        settable: *mut Boolean,
    ) -> AXError;
}

type AXUIElementRef = *const std::ffi::c_void;
type AXError = i32;

const AX_ERR_SUCCESS: AXError = 0;
const AX_MSG_TIMEOUT_SECS: f32 = 0.5;
