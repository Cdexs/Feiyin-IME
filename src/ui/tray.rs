use crate::config::UiLanguage;
use crate::i18n;
use tray_icon::Icon;

/// Tray state communicated from the pipeline to the tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Idle,
    Recording,
    Processing,
    Error,
}

impl TrayState {
    pub fn tooltip(self, ui_language: UiLanguage) -> &'static str {
        let strings = i18n::get(ui_language);
        match self {
            TrayState::Idle => strings.tray_idle,
            TrayState::Recording => strings.tray_recording,
            TrayState::Processing => strings.tray_processing,
            TrayState::Error => strings.tray_error,
        }
    }

    pub fn icon(self) -> Icon {
        load_embedded_tray_icon()
    }
}

/// Embedded tray icon (orange-black microphone, 16x16 PNG)
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../src-tauri/icons/tray-16x16.png");

fn load_embedded_tray_icon() -> Icon {
    if let Ok(img) = image::load_from_memory(TRAY_ICON_PNG) {
        let rgba = img.to_rgba8();
        let raw = rgba.into_raw();
        if let Ok(icon) = Icon::from_rgba(raw, 16, 16) {
            return icon;
        }
    }
    make_tray_icon_image([0xFF, 0x6B, 0x35])
}

fn make_tray_icon_image(color: [u8; 3]) -> Icon {
    const S: u32 = 32;
    let mut px = vec![0u8; (S * S * 4) as usize];

    for y in 0..S {
        for x in 0..S {
            let i = ((y * S + x) * 4) as usize;
            let (fx, fy) = (x as f32 - 15.5, y as f32 - 15.5);
            if fx * fx + fy * fy > 225.0 {
                continue;
            }

            px[i] = color[0];
            px[i + 1] = color[1];
            px[i + 2] = color[2];
            px[i + 3] = 0xFF;

            let mx = x as i32 - 16;
            let my = y as i32 - 16;
            let in_body = mx.abs() <= 3 && (-7..=3).contains(&my);
            let in_cap = ((mx * mx + (my + 7) * (my + 7)) as f32).sqrt() <= 3.5 && my <= -7;
            let in_stand = mx == 0 && (3..=8).contains(&my);
            let in_base = mx.abs() <= 4 && my == 8;

            if in_body || in_cap || in_stand || in_base {
                px[i] = 0xFF;
                px[i + 1] = 0xFF;
                px[i + 2] = 0xFF;
                px[i + 3] = 0xFF;
            }
        }
    }

    Icon::from_rgba(px, S, S).expect("tray icon rgba")
}
