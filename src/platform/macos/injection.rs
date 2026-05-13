//! macOS text injection implementation using enigo and native clipboard tools.

use anyhow::{anyhow, Context, Result};
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

fn inject_via_clipboard(text: &str, delay_ms: u64) -> Result<()> {
    let old_content = get_clipboard_text().ok();
    set_clipboard_text(text)?;

    thread::sleep(Duration::from_millis(50));
    let send_result = send_command_v();
    if send_result.is_ok() {
        thread::sleep(Duration::from_millis(delay_ms.max(50)));
    }

    if let Some(old) = old_content {
        let _ = set_clipboard_text(&old);
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
