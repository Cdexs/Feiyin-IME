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
    /// Deprecated (LANG-AUTO-001): kept for serde compatibility only. Runtime no
    /// longer reads this field; language is auto-detected from the transcribed text.
    pub transcription_language: String,
    /// Chinese output script. Used for zhconv normalization based on actual text content.
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
    /// ASR 模型选择（DEC-025 / DEC-028）："performance"(默认) | "accuracy" | "qwen3_online"
    /// 必须与主程序 src/config/mod.rs AudioConfig.asr_model 同步，
    /// 否则配置界面保存时会静默丢弃主程序写入的 asr_model（round-trip 数据丢失）
    #[serde(default = "default_asr_model")]
    pub asr_model: String,
    /// Qwen3 online ASR API key (DEC-028)
    #[serde(default)]
    pub qwen3_api_key: String,
    /// Qwen3 online ASR service URL. Stored in config only; not exposed in UI.
    #[serde(default = "default_qwen3_asr_url")]
    pub qwen3_asr_url: String,
    #[serde(default = "default_qwen3_asr_model")]
    pub qwen3_asr_model: String,
}

fn default_overlay_opacity() -> f32 {
    0.75
}

fn default_asr_model() -> String {
    "performance".to_string()
}

fn default_qwen3_asr_url() -> String {
    "wss://dashscope.aliyuncs.com/api-ws/v1/realtime".to_string()
}

fn default_qwen3_asr_model() -> String {
    "qwen3-asr-flash-realtime".to_string()
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
            asr_model: default_asr_model(),
            qwen3_api_key: String::new(),
            qwen3_asr_url: default_qwen3_asr_url(),
            qwen3_asr_model: default_qwen3_asr_model(),
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
    Japanese,
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
    /// Deprecated (LANG-AUTO-001): kept for serde compatibility only. Runtime no
    /// longer reads this field; translation direction is inferred from the
    /// transcribed text content.
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
        Self { enabled: true }
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
    #[serde(default)]
    pub scene: SceneConfig,
}

fn default_scene_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneConfig {
    #[serde(default = "default_scene_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub send_window_title: bool,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            enabled: default_scene_enabled(),
            send_window_title: false,
        }
    }
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
            scene: SceneConfig::default(),
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
        // ASR-HIDE-ACCURACY-001-UI: deprecated "accuracy" model is no longer exposed in UI.
        // Migrate any existing config silently to "performance" so that UI and main process agree.
        if cfg.audio.asr_model == "accuracy" {
            cfg.audio.asr_model = default_asr_model();
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
        // ASR-HIDE-ACCURACY-001-UI: mirror migration for load_from path.
        if cfg.audio.asr_model == "accuracy" {
            cfg.audio.asr_model = default_asr_model();
        }

        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_minimal_cfg_with_asr_model(asr_model: &str) -> AppConfig {
        AppConfig {
            audio: AudioConfig {
                silence_threshold: 0.01,
                transcription_language: "zh".to_string(),
                chinese_script: ChineseScript::Simplified,
                overlay_opacity: 0.75,
                input_device: String::new(),
                enable_streaming: false,
                asr_model: asr_model.to_string(),
                qwen3_api_key: String::new(),
                qwen3_asr_url: default_qwen3_asr_url(),
                qwen3_asr_model: default_qwen3_asr_model(),
            },
            llm: LlmConfig::default(),
            hotkey: HotkeyConfig::default(),
            injection: InjectionConfig::default(),
            ui_language: UiLanguage::Chinese,
            auto_learn_threshold: default_auto_learn_threshold(),
            auto_start: false,
            translation: TranslationConfig::default(),
            punctuation: PunctuationConfig::default(),
            scene: SceneConfig::default(),
        }
    }

    fn temp_config_path(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("voice-ime-config-test-{}", name));
        path
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_from_migrates_accuracy_to_performance() {
        let path = temp_config_path("accuracy");
        cleanup(&path);
        let mut cfg = make_minimal_cfg_with_asr_model("accuracy");
        cfg.llm.system_prompt = default_system_prompt();
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path).unwrap();
        cleanup(&path);
        assert_eq!(loaded.audio.asr_model, "performance");
    }

    #[test]
    fn load_from_keeps_performance_untouched() {
        let path = temp_config_path("performance");
        cleanup(&path);
        let mut cfg = make_minimal_cfg_with_asr_model("performance");
        cfg.llm.system_prompt = default_system_prompt();
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path).unwrap();
        cleanup(&path);
        assert_eq!(loaded.audio.asr_model, "performance");
    }

    #[test]
    fn load_from_keeps_qwen3_online_untouched() {
        let path = temp_config_path("qwen3_online");
        cleanup(&path);
        let mut cfg = make_minimal_cfg_with_asr_model("qwen3_online");
        cfg.llm.system_prompt = default_system_prompt();
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path).unwrap();
        cleanup(&path);
        assert_eq!(loaded.audio.asr_model, "qwen3_online");
    }
}
