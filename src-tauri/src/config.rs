use anyhow::Result;
use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

use crate::i18n;

/// 最大录音时长（秒），硬编码，不可通过 config 修改
pub const MAX_RECORD_SECONDS: u64 = 180;
/// 最长静默间隔（毫秒），超过此时长无声音则自动停止录音
pub const SILENCE_DURATION_MS: u64 = 8_000;

/// Get the unified English default system prompt.
pub fn default_system_prompt() -> String {
    let strings = i18n::get(UiLanguage::English);
    strings.default_system_prompt_en.to_string()
}

fn default_auto_learn_threshold() -> u32 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmConfig {
    pub api_url: String,
    pub api_key: String,
    pub model: String,
    /// Current system prompt (unified English default).
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    pub enabled: bool,
    #[serde(default)]
    pub connectivity_verified: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
            system_prompt: default_system_prompt(),
            enabled: true,
            connectivity_verified: false,
        }
    }
}

/// How the hotkey triggers recording.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum HotkeyMode {
    /// Hold key → record; release → process
    PushToTalk,
    /// First press → start; second press → process
    Toggle,
}

impl Default for HotkeyMode {
    fn default() -> Self {
        HotkeyMode::Toggle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HotkeyConfig {
    /// Virtual key code (Windows VK_ constants)
    pub vk_code: u32,
    /// Modifier flags: 0x0001=Alt, 0x0002=Ctrl, 0x0004=Shift, 0x0008=Win
    pub modifiers: u32,
    pub display_name: String,
    /// Recording trigger mode
    pub mode: HotkeyMode,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        // Default: F9 (no modifiers), Toggle mode
        Self {
            vk_code: 0x78, // VK_F9
            modifiers: 0,
            display_name: "F9".to_string(),
            mode: HotkeyMode::Toggle,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioConfig {
    pub silence_threshold: f32,
    /// ASR transcription language: "zh", "en", "ja", "ko", "auto", etc.
    pub transcription_language: String,
    /// Chinese output script (only applies when transcription_language is "zh").
    #[serde(default)]
    pub chinese_script: ChineseScript,
    /// Opacity of the recording overlay window (0.3 – 1.0).  Default 0.75.
    #[serde(default = "default_overlay_opacity")]
    pub overlay_opacity: f32,
    /// Selected audio input device name (empty = use system default)
    #[serde(default)]
    pub input_device: String,
    /// Enable streaming ASR mode (2-pass: streaming + offline correction)
    #[serde(default)]
    pub enable_streaming: bool,
}

fn default_overlay_opacity() -> f32 {
    0.75
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            silence_threshold: 0.01,
            transcription_language: "zh".to_string(),
            chinese_script: ChineseScript::Simplified,
            overlay_opacity: default_overlay_opacity(),
            input_device: String::new(),
            enable_streaming: false, // 默认使用 offline 模式
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InjectionConfig {
    /// true = clipboard+Ctrl+V, false = SendInput char-by-char
    pub use_clipboard: bool,
    /// ms to wait before restoring clipboard
    pub clipboard_delay_ms: u64,
}

impl Default for InjectionConfig {
    fn default() -> Self {
        Self {
            use_clipboard: true,
            clipboard_delay_ms: 150,
        }
    }
}

/// Which Chinese character script to output (only used when transcription_language is "zh").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ChineseScript {
    Simplified,
    Traditional,
}

impl Default for ChineseScript {
    fn default() -> Self {
        ChineseScript::Simplified
    }
}

/// UI display language.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum UiLanguage {
    Chinese,
    TraditionalChinese,
    English,
}

impl Default for UiLanguage {
    fn default() -> Self {
        UiLanguage::Chinese
    }
}

/// Target language for translation feature.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TranslationLanguage {
    Chinese,
    English,
}

impl Default for TranslationLanguage {
    fn default() -> Self {
        TranslationLanguage::Chinese
    }
}

/// Translation hotkey configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranslationConfig {
    /// Whether translation feature is enabled.
    pub enabled: bool,
    /// Virtual key code for translation hotkey (0 = not set).
    pub vk_code: u32,
    /// Display name for the hotkey (e.g., "Left Ctrl").
    pub display_name: String,
    /// Target language for translation.
    pub target_language: TranslationLanguage,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            vk_code: 0,
            display_name: String::new(),
            target_language: TranslationLanguage::default(),
        }
    }
}

/// Punctuation restoration configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PunctuationConfig {
    /// Enable punctuation restoration after transcription.
    pub enabled: bool,
}

impl Default for PunctuationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub llm: LlmConfig,
    pub hotkey: HotkeyConfig,
    pub audio: AudioConfig,
    pub injection: InjectionConfig,
    pub ui_language: UiLanguage,
    #[serde(default = "default_auto_learn_threshold")]
    pub auto_learn_threshold: u32,
    #[serde(default)]
    pub auto_start: bool, // 开机自动启动
    #[serde(default)]
    pub translation: TranslationConfig,
    #[serde(default)]
    pub punctuation: PunctuationConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            llm: LlmConfig::default(),
            hotkey: HotkeyConfig::default(),
            audio: AudioConfig::default(),
            injection: InjectionConfig::default(),
            ui_language: UiLanguage::default(),
            auto_learn_threshold: default_auto_learn_threshold(),
            auto_start: false,
            translation: TranslationConfig::default(),
            punctuation: PunctuationConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            let cfg = AppConfig::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let content = std::fs::read_to_string(&path)?;
        let mut cfg: AppConfig = toml::from_str(&content)?;

        if cfg.llm.system_prompt.is_empty() {
            cfg.llm.system_prompt = default_system_prompt();
        }
        if cfg.auto_learn_threshold == 0 {
            cfg.auto_learn_threshold = default_auto_learn_threshold();
        }

        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        self.save_to(&path)
    }

    /// Save config to an explicit path (useful for tests and tooling).
    pub fn save_to(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        let mut file = AtomicWriteFile::options().open(path)?;
        file.write_all(content.as_bytes())?;
        file.commit()?;
        Ok(())
    }

    /// Load config from an explicit path (useful for tests and tooling).
    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            let cfg = AppConfig::default();
            cfg.save_to(path)?;
            return Ok(cfg);
        }
        let content = std::fs::read_to_string(path)?;
        let mut cfg: AppConfig = toml::from_str(&content)?;

        if cfg.llm.system_prompt.is_empty() {
            cfg.llm.system_prompt = default_system_prompt();
        }
        if cfg.auto_learn_threshold == 0 {
            cfg.auto_learn_threshold = default_auto_learn_threshold();
        }

        Ok(cfg)
    }
}
