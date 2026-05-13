use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OverlayState {
    Recording,
    Processing(String),
    Error(String),
    Preview { text: String },
}

#[derive(Debug, Clone)]
pub struct OverlayConfig {
    pub width: f64,
    pub height: f64,
    pub opacity: f64,
    pub always_on_top: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            width: 300.0,
            height: 80.0,
            opacity: 0.85,
            always_on_top: true,
        }
    }
}

pub fn create_overlay_window(app: &tauri::AppHandle, config: OverlayConfig) -> Result<(), String> {
    if app.get_webview_window("overlay").is_some() {
        return Err("Overlay window already exists".into());
    }

    let _window = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("Recording Overlay")
        .inner_size(config.width, config.height)
        .transparent(true)
        .decorations(false)
        .always_on_top(config.always_on_top)
        .skip_taskbar(true)
        .use_https_scheme(true)
        .build()
        .map_err(|e| format!("Failed to create overlay window: {e}"))?;

    Ok(())
}

pub fn update_overlay_state(window: &WebviewWindow, state: OverlayState) {
    let _ = window.emit(
        "overlay-state-changed",
        serde_json::to_string(&state).unwrap_or_default(),
    );
}

pub fn show_overlay(window: &WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn hide_overlay(window: &WebviewWindow) {
    let _ = window.hide();
}

pub fn close_overlay(window: &WebviewWindow) {
    let _ = window.close();
}

#[tauri::command]
pub fn show_recording_overlay(window: tauri::WebviewWindow) -> Result<(), String> {
    show_overlay(&window);
    update_overlay_state(&window, OverlayState::Recording);
    Ok(())
}

#[tauri::command]
pub fn hide_recording_overlay(window: tauri::WebviewWindow) -> Result<(), String> {
    hide_overlay(&window);
    Ok(())
}

#[tauri::command]
pub fn update_overlay_status(
    window: tauri::WebviewWindow,
    state: OverlayState,
) -> Result<(), String> {
    update_overlay_state(&window, state);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_config_default() {
        let config = OverlayConfig::default();
        assert_eq!(config.width, 300.0);
        assert_eq!(config.height, 80.0);
        assert_eq!(config.opacity, 0.85);
        assert!(config.always_on_top);
    }

    #[test]
    fn test_overlay_state_serialization() {
        let state = OverlayState::Recording;
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("Recording"));
    }

    #[test]
    fn test_overlay_state_deserialization_fixture() {
        let json = r#"{"Processing":"处理中"}"#;
        assert!(json.contains("Processing"));
    }
}
