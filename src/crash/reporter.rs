//! Crash Report Reporter UI
//!
//! Standalone reporter executable entry.
//! Uses eframe to display a simple interface.

use super::email::send_crash_email;
use super::storage::{delete_crash_report, load_crash_report, CrashReport};
use crate::config::{AppConfig, UiLanguage};
use eframe::egui;
use std::path::Path;

/// Strings for crash reporter UI
struct CrashStrings {
    title: &'static str,
    prompt: &'static str,
    privacy: &'static str,
    sending: &'static str,
    success: &'static str,
    close: &'static str,
    send_report: &'static str,
    send_failed: &'static str,
    no_report: &'static str,
}

fn get_crash_strings(lang: UiLanguage) -> CrashStrings {
    match lang {
        UiLanguage::Chinese | UiLanguage::TraditionalChinese => CrashStrings {
            title: "程式意外退出",
            prompt: "是否傳送崩潰報告幫助我們改進產品？",
            privacy: "匿名傳送，不含語音內容或個人資料",
            sending: "正在傳送...",
            success: "已傳送，感謝您的幫助！",
            close: "關閉",
            send_report: "傳送報告",
            send_failed: "傳送失敗",
            no_report: "沒有找到崩潰報告",
        },
        UiLanguage::English => CrashStrings {
            title: "Unexpected Exit",
            prompt: "Send crash report to help us improve?",
            privacy: "Anonymous, no voice content or personal data",
            sending: "Sending...",
            success: "Sent successfully, thank you!",
            close: "Close",
            send_report: "Send Report",
            send_failed: "Send failed",
            no_report: "No crash report found",
        },
    }
}

/// Reporter application state
struct ReporterApp {
    /// Loaded crash report
    report: Option<CrashReport>,
    /// Current state
    state: ReporterState,
    /// Error message (if any)
    error_msg: String,
    /// UI strings based on language
    strings: CrashStrings,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ReporterState {
    /// Initial state: ask to send
    AskToSend,
    /// Sending
    Sending,
    /// Sent successfully
    SentSuccess,
    /// Sent failed
    SentFailed,
}

impl ReporterApp {
    fn new() -> Self {
        // Try to load crash report
        let report = load_crash_report().ok().flatten();

        // Load config to get UI language
        let config = AppConfig::load().unwrap_or_default();
        let strings = get_crash_strings(config.ui_language);

        Self {
            report,
            state: ReporterState::AskToSend,
            error_msg: String::new(),
            strings,
        }
    }
}

impl eframe::App for ReporterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Use white background to match design
        let mut theme = egui::Visuals::light();
        theme.panel_fill = egui::Color32::WHITE;
        ctx.set_visuals(theme);

        // Handle sending state
        self.handle_sending(ctx);

        // Brand orange color
        let brand_orange = egui::Color32::from_rgb(255, 107, 53);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::WHITE)
                    .inner_margin(32.0),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);

                    // === Icon area (orange circle + white exclamation mark) ===
                    let (icon_rect, _) =
                        ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
                    // Orange background circle
                    ui.painter()
                        .circle_filled(icon_rect.center(), 32.0, brand_orange);
                    // White exclamation mark
                    ui.painter().text(
                        icon_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "!",
                        egui::FontId::proportional(32.0),
                        egui::Color32::WHITE,
                    );

                    ui.add_space(20.0);

                    // === Title ===
                    ui.label(
                        egui::RichText::new(self.strings.title)
                            .size(22.0)
                            .strong()
                            .color(egui::Color32::from_rgb(51, 51, 51)),
                    );

                    ui.add_space(8.0);

                    // === Prompt ===
                    ui.label(
                        egui::RichText::new(self.strings.prompt)
                            .size(14.0)
                            .color(egui::Color32::from_rgb(51, 51, 51)),
                    );

                    ui.add_space(4.0);

                    // === Privacy notice ===
                    ui.label(
                        egui::RichText::new(self.strings.privacy)
                            .size(12.0)
                            .color(egui::Color32::from_rgb(153, 153, 153)),
                    );

                    ui.add_space(24.0);

                    // === Buttons row ===
                    // Show different content based on state
                    match self.state {
                        ReporterState::AskToSend => {
                            self.show_buttons(ui, brand_orange);
                        }
                        ReporterState::Sending => {
                            ui.label(
                                egui::RichText::new(self.strings.sending)
                                    .size(14.0)
                                    .color(egui::Color32::from_rgb(153, 153, 153)),
                            );
                        }
                        ReporterState::SentSuccess => {
                            ui.label(
                                egui::RichText::new(self.strings.success)
                                    .size(14.0)
                                    .color(brand_orange),
                            );

                            ui.add_space(20.0);

                            // Close button
                            if ui
                                .button(egui::RichText::new(self.strings.close).size(14.0))
                                .clicked()
                            {
                                // Delete crash report
                                let _ = delete_crash_report();
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                        ReporterState::SentFailed => {
                            ui.label(
                                egui::RichText::new(self.strings.send_failed)
                                    .size(14.0)
                                    .color(egui::Color32::from_rgb(220, 38, 38)),
                            );

                            ui.add_space(5.0);

                            ui.label(
                                egui::RichText::new(&self.error_msg)
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(153, 153, 153)),
                            );

                            ui.add_space(20.0);

                            // Close button
                            if ui
                                .button(egui::RichText::new(self.strings.close).size(14.0))
                                .clicked()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    }
                });
            });
    }
}

impl ReporterApp {
    fn show_buttons(&mut self, ui: &mut egui::Ui, brand_orange: egui::Color32) {
        ui.horizontal_centered(|ui| {
            // Close button — white background + gray border + dark gray text
            let close_button = egui::Button::new(
                egui::RichText::new(self.strings.close)
                    .size(14.0)
                    .color(egui::Color32::from_rgb(51, 51, 51)),
            )
            .fill(egui::Color32::WHITE)
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(200, 200, 200),
            ))
            .min_size(egui::vec2(120.0, 36.0));

            if ui.add(close_button).clicked() {
                // crash.json preserved (user chose not to send)
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }

            ui.add_space(12.0);

            // Send report button — orange fill + white text
            let send_button = egui::Button::new(
                egui::RichText::new(self.strings.send_report)
                    .size(14.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            )
            .fill(brand_orange)
            .min_size(egui::vec2(120.0, 36.0));

            if ui.add(send_button).clicked() {
                self.state = ReporterState::Sending;
            }
        });
    }

    fn handle_sending(&mut self, ctx: &egui::Context) {
        if self.state == ReporterState::Sending {
            if let Some(report) = self.report.clone() {
                // Try to send email
                match send_crash_email(&report) {
                    Ok(_) => {
                        self.state = ReporterState::SentSuccess;
                        // Delete report after successful send
                        let _ = delete_crash_report();
                    }
                    Err(e) => {
                        self.state = ReporterState::SentFailed;
                        self.error_msg = e.to_string();
                    }
                }
            } else {
                self.state = ReporterState::SentFailed;
                self.error_msg = self.strings.no_report.to_string();
            }
            ctx.request_repaint();
        }
    }
}

/// Run Reporter UI
pub fn run() {
    // Get UI language from config for window title
    let config = AppConfig::load().unwrap_or_default();
    let title = match config.ui_language {
        UiLanguage::Chinese => "飞音语音输入 - 崩溃报告",
        UiLanguage::TraditionalChinese => "飛音語音輸入 - 崩潰報告",
        UiLanguage::English => "Feiyin Voice Input - Crash Report",
    };

    // Load icon from assets (platform-specific)
    #[cfg(target_os = "windows")]
    let icon_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons/app.ico");

    #[cfg(target_os = "macos")]
    let icon_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons/app.png");

    let icon = if icon_path.exists() {
        if let Ok(img) = image::open(&icon_path) {
            let rgba = img.into_rgba8();
            let (w, h) = rgba.dimensions();
            Some(egui::IconData {
                rgba: rgba.to_vec(),
                width: w,
                height: h,
            })
        } else {
            None
        }
    } else {
        None
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([400.0, 300.0])
        .with_resizable(false)
        .with_title(title);

    // Set icon if available
    if let Some(icon_data) = icon {
        viewport = viewport.with_icon(icon_data);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Crash Reporter",
        options,
        Box::new(|cc| {
            // Load platform-specific font for proper CJK display
            let mut fonts = egui::FontDefinitions::default();

            #[cfg(target_os = "windows")]
            {
                fonts.font_data.insert(
                    "MicrosoftYaHei".to_owned(),
                    egui::FontData::from_static(include_bytes!("C:/Windows/Fonts/msyh.ttc")),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "MicrosoftYaHei".to_owned());
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .push("MicrosoftYaHei".to_owned());
            }

            #[cfg(target_os = "macos")]
            {
                // macOS uses PingFang SC for Simplified Chinese
                let font_path = "/System/Library/Fonts/PingFang.ttc";
                if std::path::Path::new(font_path).exists() {
                    if let Ok(font_data) = std::fs::read(font_path) {
                        fonts.font_data.insert(
                            "PingFangSC".to_owned(),
                            egui::FontData::from_bytes(font_data)
                                .ok()
                                .unwrap_or_default(),
                        );
                        fonts
                            .families
                            .entry(egui::FontFamily::Proportional)
                            .or_default()
                            .insert(0, "PingFangSC".to_owned());
                        fonts
                            .families
                            .entry(egui::FontFamily::Monospace)
                            .or_default()
                            .push("PingFangSC".to_owned());
                    }
                }
            }

            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(ReporterApp::new()))
        }),
    );
}
