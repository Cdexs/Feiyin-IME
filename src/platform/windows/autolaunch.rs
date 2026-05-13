//! Windows Auto-Launch (开机自启) using Registry

use anyhow::{anyhow, Result};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_WRITE, REG_SZ,
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_NAME: &str = "voice-ime";

/// Enable auto-launch on system startup
pub fn enable() -> Result<()> {
    let exe_path = std::env::current_exe()?;
    let exe_path_str = exe_path.to_string_lossy().to_string();

    let key_path = encode_wide(RUN_KEY);
    let mut hkey: HKEY = HKEY::default();

    unsafe {
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey,
        );
        if result.is_err() {
            return Err(anyhow!("RegOpenKeyExW failed"));
        }

        let value_name = encode_wide(APP_NAME);
        let value_data: Vec<u8> = exe_path_str
            .encode_utf16()
            .flat_map(|c| [c as u8, (c >> 8) as u8])
            .collect();

        let result = RegSetValueExW(
            hkey,
            PCWSTR(value_name.as_ptr()),
            0,
            REG_SZ,
            Some(&value_data),
        );
        if result.is_err() {
            let _ = RegCloseKey(hkey);
            return Err(anyhow!("RegSetValueExW failed"));
        }

        let _ = RegCloseKey(hkey);
    }

    log::info!("Auto-launch enabled: {}", exe_path_str);
    Ok(())
}

/// Disable auto-launch
pub fn disable() -> Result<()> {
    let key_path = encode_wide(RUN_KEY);
    let mut hkey: HKEY = HKEY::default();

    unsafe {
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey,
        );
        if result.is_err() {
            return Err(anyhow!("RegOpenKeyExW failed"));
        }

        let value_name = encode_wide(APP_NAME);
        let _ = RegDeleteValueW(hkey, PCWSTR(value_name.as_ptr()));

        let _ = RegCloseKey(hkey);
    }

    log::info!("Auto-launch disabled");
    Ok(())
}

/// Check if auto-launch is enabled
pub fn is_enabled() -> bool {
    let key_path = encode_wide(RUN_KEY);
    let mut hkey: HKEY = HKEY::default();

    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey,
        )
        .is_err()
        {
            return false;
        }

        let value_name = encode_wide(APP_NAME);
        let result = RegQueryValueExW(hkey, PCWSTR(value_name.as_ptr()), None, None, None, None);

        let _ = RegCloseKey(hkey);
        result.is_ok()
    }
}

fn encode_wide(text: &str) -> Vec<u16> {
    OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
