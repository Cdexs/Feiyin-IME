// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod crash;
mod i18n;
mod llm;
mod overlay;
mod version_check;
mod wordbook;

use audio::get_input_devices;
use config::{AppConfig, LlmConfig};
use llm::LlmClient;
use tauri::Manager;

#[tauri::command]
fn get_config() -> Result<AppConfig, String> {
    AppConfig::load().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_config(config: AppConfig) -> Result<(), String> {
    config.save().map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_llm_connection(config: LlmConfig) -> Result<String, String> {
    let client = LlmClient::new(config);
    client.probe().await.map_err(|e| e.to_string())
}

#[tauri::command]
fn get_audio_devices() -> Result<Vec<String>, String> {
    get_input_devices().map_err(|e| e.to_string())
}

#[tauri::command]
fn check_hotkey_available(vk_code: u32, modifiers: u32) -> bool {
    // polling 路径�?VK 码（右侧修饰键变体）无法通过 RegisterHotKey 检测，视为可用
    if matches!(vk_code, 0xA0..=0xA5) {
        return true;
    }
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
        MOD_SHIFT, MOD_WIN,
    };
    let mut flags = HOT_KEY_MODIFIERS(0);
    if modifiers & 0x0001 != 0 {
        flags |= MOD_ALT;
    }
    if modifiers & 0x0002 != 0 {
        flags |= MOD_CONTROL;
    }
    if modifiers & 0x0004 != 0 {
        flags |= MOD_SHIFT;
    }
    if modifiers & 0x0008 != 0 {
        flags |= MOD_WIN;
    }
    flags |= MOD_NOREPEAT;
    const TEST_ID: i32 = 9998;
    unsafe {
        let ok = RegisterHotKey(HWND::default(), TEST_ID, flags, vk_code).is_ok();
        if ok {
            let _ = UnregisterHotKey(HWND::default(), TEST_ID);
        }
        ok
    }
}

#[cfg(target_os = "windows")]
fn is_main_process_running() -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return false;
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut found = false;
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&ch| ch == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if name.eq_ignore_ascii_case("feiyin-ime.exe") {
                    found = true;
                    break;
                }

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
        found
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    if !is_main_process_running() {
        std::process::exit(1);
    }

    crash::install_panic_hook();

    let app = tauri::Builder::default()
        .setup(|_app| {
            if let Some(main) = _app.get_webview_window("main") {
                let _ = main.set_maximizable(false);
                if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png")) {
                    let _ = main.set_icon(icon);
                }
            }

            // MAC-013: macOS 透明 Overlay 配置
            // macOS 上 WebView 窗口默认有阴影，需要显式移除以确保 overlay 完全透明
            #[cfg(target_os = "macos")]
            {
                if let Some(overlay) = _app.get_webview_window("overlay") {
                    let _ = overlay.set_shadow(false);
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = _app;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            test_llm_connection,
            get_audio_devices,
            check_hotkey_available,
            version_check::get_version_info,
            version_check::force_check_latest_version,
            version_check::open_url_in_browser,
            wordbook::get_wordbook_entries,
            wordbook::get_wordbook_stats,
            wordbook::add_wordbook_entry,
            wordbook::delete_wordbook_entry,
            wordbook::delete_wordbook_entry_by_id,
            overlay::show_recording_overlay,
            overlay::hide_recording_overlay,
            overlay::update_overlay_status
        ])
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }

            if let tauri::WindowEvent::CloseRequested { .. } = event {
                crash::mark_expected_exit();
                if let Some(overlay) = window.app_handle().get_webview_window("overlay") {
                    let _ = overlay.close();
                }
                window.app_handle().exit(0);
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            crash::report_unexpected_exit(
                "Unexpected settings UI exit (possible WebView2 renderer crash)",
            );
        }
    });
}
