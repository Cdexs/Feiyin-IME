use anyhow::Result;
use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

use crate::i18n;

/// 最大录音时长（秒），硬编码，不可通过 config 修改
pub const MAX_RECORD_SECONDS: u64 = 300;
/// 最长静默间隔（毫秒），超过此时长无声音则自动停止录音
pub const SILENCE_DURATION_MS: u64 = 30_000;

/// Get default system prompt (unified English version).
/// OPT-001: System prompt unified to English, model can understand English instructions regardless of input language.
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
    /// Unified system prompt (English). OPT-001: Single prompt for all languages.
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    /// Legacy field for config migration (deprecated, not used at runtime)
    #[serde(default, skip_serializing)]
    pub system_prompt_zh: Option<String>,
    /// Legacy field for config migration (deprecated, not used at runtime)
    #[serde(default, skip_serializing)]
    pub system_prompt_en: Option<String>,
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
            system_prompt_zh: None,
            system_prompt_en: None,
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

/// Target language for translation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TranslationLanguage {
    /// Chinese output. Simplified/traditional follows audio.chinese_script.
    Chinese,
    English,
}

impl Default for TranslationLanguage {
    fn default() -> Self {
        TranslationLanguage::Chinese
    }
}

/// Translation hotkey and target configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranslationConfig {
    pub enabled: bool,
    /// Virtual key code for the translation modifier. 0 = unset.
    pub vk_code: u32,
    pub display_name: String,
    /// TRANS-BIDIR-001: direction is now auto-derived from content (contains_han).
    /// This field is retained as a "last-used direction cache" for engine preloading
    /// at startup — it is NOT a user-facing setting and no longer gates translation.
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
    /// ASR 模型选择（DEC-025 + DEC-028）：
    /// "performance"(默认,179MB CTC) | "accuracy"(972MB native+hotwords) | "qwen3_online"(在线 ASR)
    /// 旧配置无此字段时 serde default 等效于 "performance"，行为与直换前完全一致
    #[serde(default = "default_asr_model")]
    pub asr_model: String,
    /// Qwen3 在线 ASR API Key（DEC-028，仅 qwen3_online 模式用）
    #[serde(default)]
    pub qwen3_api_key: String,
    /// Qwen3 在线 ASR 服务 URL（DEC-028，仅配置文件持有，不在 UI 显示）
    /// 默认北京 region：wss://dashscope.aliyuncs.com/api-ws/v1/realtime
    #[serde(default = "default_qwen3_asr_url")]
    pub qwen3_asr_url: String,
    /// Qwen3 在线 ASR 模型 ID（DEC-028，2026-07-07 从硬编码改为配置读取）
    /// 默认：qwen3-asr-flash-realtime
    #[serde(default = "default_qwen3_asr_model")]
    pub qwen3_asr_model: String,
}

fn default_overlay_opacity() -> f32 {
    1.0
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
}

impl Default for UiLanguage {
    fn default() -> Self {
        UiLanguage::Chinese
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PunctuationConfig {
    #[serde(default = "default_punctuation_enabled")]
    pub enabled: bool,
}

fn default_punctuation_enabled() -> bool {
    true
}

impl Default for PunctuationConfig {
    fn default() -> Self {
        Self {
            enabled: default_punctuation_enabled(),
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
    /// SCENE-SENSE-001-CORE (DEC-031-⑤): 场景感知配置。
    /// 纯本地采集（进程名+窗口标题），默认开启；未命中安全降级 Unknown。
    /// send_window_title 控制窗口标题是否上送 LLM（隐私敏感，默认关）。
    /// 字段定义与 src-tauri/src/config.rs 的 SceneConfig 保持一致。
    #[serde(default)]
    pub scene: SceneConfig,
}

/// SCENE-SENSE-001-CORE (DEC-031-⑤): 场景感知配置。
/// 纯本地采集（进程名+窗口标题），默认开启；未命中安全降级 Unknown。
/// send_window_title 控制窗口标题是否上送 LLM（隐私敏感，默认关）。
/// 与 src-tauri/src/config.rs 的 SceneConfig 字段保持一致。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneConfig {
    /// 场景感知总开关。false 时完全走 Phase 1 现状路径（含 flatten），零行为变化。
    #[serde(default = "default_scene_enabled")]
    pub enabled: bool,
    /// 窗口标题上送 LLM（隐私敏感，默认关）。
    /// true 时 F4 段可含截断标题（上限 50 字符）以提升场景判断。
    #[serde(default)]
    pub send_window_title: bool,
}

fn default_scene_enabled() -> bool {
    true
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

        // OPT-001: Migration - if system_prompt is empty but legacy fields exist, migrate
        if cfg.llm.system_prompt.is_empty() {
            // Prefer English prompt from legacy field, fallback to default
            cfg.llm.system_prompt = cfg
                .llm
                .system_prompt_en
                .clone()
                .filter(|s| !s.is_empty())
                .or_else(|| cfg.llm.system_prompt_zh.clone().filter(|s| !s.is_empty()))
                .unwrap_or_else(default_system_prompt);
        }

        // Clear legacy fields after migration (they won't be serialized due to skip_serializing)
        cfg.llm.system_prompt_zh = None;
        cfg.llm.system_prompt_en = None;
        if cfg.auto_learn_threshold == 0 {
            cfg.auto_learn_threshold = default_auto_learn_threshold();
        }

        // ASR-HIDE-ACCURACY-001-CORE: 存量 accuracy 配置静默迁移为 performance。
        // 背景：UI 已隐藏 accuracy 下拉选项，存量用户配置若仍是 accuracy 会致下拉空白+后台跑 accuracy 不一致。
        // 迁移：load 时检测到 asr_model=="accuracy" 静默改写为 "performance" 并落盘保存。
        if cfg.audio.asr_model == "accuracy" {
            log::info!(
                "ASR-HIDE-ACCURACY-001: migrating legacy asr_model='accuracy' -> 'performance'"
            );
            cfg.audio.asr_model = "performance".to_string();
            // 落盘保存迁移后的配置（避免下次启动重复迁移 + UI 与后台一致）
            if let Err(e) = cfg.save() {
                log::warn!(
                    "ASR-HIDE-ACCURACY-001: failed to persist migrated config: {}",
                    e
                );
            }
        }

        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        self.save_to(&path)
    }

    /// TRANS-BIDIR-001 / REFACTOR-SHARE-TRANSDIR-001: Update the cached last-used
    /// translation direction. Platform-neutral (DEC-033) — macOS side can call this
    /// directly. Returns true if the value changed (caller may use this to decide
    /// whether to save).
    ///
    /// This does NOT save to disk — saving is the caller's responsibility (platform
    /// runtime concern: Windows uses config_path(), macOS may differ).
    pub fn remember_translation_direction(&mut self, direction: TranslationLanguage) -> bool {
        if self.translation.target_language != direction {
            self.translation.target_language = direction;
            true
        } else {
            false
        }
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

        // OPT-001: Migration - if system_prompt is empty but legacy fields exist, migrate
        if cfg.llm.system_prompt.is_empty() {
            cfg.llm.system_prompt = cfg
                .llm
                .system_prompt_en
                .clone()
                .filter(|s| !s.is_empty())
                .or_else(|| cfg.llm.system_prompt_zh.clone().filter(|s| !s.is_empty()))
                .unwrap_or_else(default_system_prompt);
        }

        // Clear legacy fields after migration
        cfg.llm.system_prompt_zh = None;
        cfg.llm.system_prompt_en = None;
        if cfg.auto_learn_threshold == 0 {
            cfg.auto_learn_threshold = default_auto_learn_threshold();
        }

        // ASR-HIDE-ACCURACY-001-CORE: 存量 accuracy 配置静默迁移为 performance（load_from 同 load）。
        let migrated_from_accuracy = cfg.audio.asr_model == "accuracy";
        if migrated_from_accuracy {
            log::info!(
                "ASR-HIDE-ACCURACY-001: migrating legacy asr_model='accuracy' -> 'performance' (load_from)"
            );
            cfg.audio.asr_model = "performance".to_string();
            // 落盘保存迁移后的配置（与 load 保持一致）
            if let Err(e) = cfg.save_to(path) {
                log::warn!(
                    "ASR-HIDE-ACCURACY-001: failed to persist migrated config: {}",
                    e
                );
            }
        }

        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // ============================================================
    // TEST-SYNC-CONFIG-WATCHER-001 测试对齐
    // P1: atomic save 单元测试
    // P2: config file watcher 手动验证占位（待 coder-1 实现）
    // ============================================================

    /// 测试用临时目录，确保不污染用户真实配置
    struct TestEnv {
        temp_dir: PathBuf,
    }

    impl TestEnv {
        fn new() -> Self {
            let temp_dir =
                std::env::temp_dir().join(format!("voice-ime-test-{}", std::process::id()));
            std::fs::create_dir_all(&temp_dir).unwrap();
            Self { temp_dir }
        }

        fn config_path(&self) -> PathBuf {
            self.temp_dir.join("config.toml")
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.temp_dir);
        }
    }

    // 使用 Mutex 保护 AppConfig::config_path 的全局状态
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    /// ATOMIC-SAVE-001: 保存后文件存在且为有效 TOML
    /// 验证 atomic save 的写入结果可被正确读取
    #[test]
    fn atomic_save_creates_valid_toml_file() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();
        let cfg = AppConfig::default();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        assert!(
            env.config_path().exists(),
            "config file should exist after save"
        );

        let content = std::fs::read_to_string(env.config_path()).expect("file should be readable");
        let loaded: AppConfig = toml::from_str(&content).expect("content should be valid TOML");

        // Verify default values survived roundtrip
        assert_eq!(loaded.hotkey.vk_code, 0x78);
        assert_eq!(loaded.audio.transcription_language, "zh");
        assert_eq!(loaded.translation, TranslationConfig::default());
    }

    /// ATOMIC-SAVE-002: 保存前后配置值一致性验证
    /// 验证 save → load 往返后配置值不变
    #[test]
    fn save_load_roundtrip_preserves_values() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.hotkey.vk_code = 0x70; // F1 instead of F9
        cfg.audio.transcription_language = "en".to_string();
        cfg.llm.api_url = "https://api.example.com/v1".to_string();
        cfg.injection.use_clipboard = false;
        cfg.auto_learn_threshold = 5;
        cfg.translation.enabled = true;
        cfg.translation.vk_code = 0x12; // Alt
        cfg.translation.display_name = "Alt".to_string();
        cfg.translation.target_language = TranslationLanguage::English;

        cfg.save_to(&env.config_path())
            .expect("save should succeed");

        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");

        assert_eq!(loaded.hotkey.vk_code, 0x70, "vk_code should be preserved");
        assert_eq!(
            loaded.audio.transcription_language, "en",
            "language should be preserved"
        );
        assert_eq!(
            loaded.llm.api_url, "https://api.example.com/v1",
            "api_url should be preserved"
        );
        assert_eq!(
            loaded.injection.use_clipboard, false,
            "clipboard mode should be preserved"
        );
        assert_eq!(
            loaded.auto_learn_threshold, 5,
            "threshold should be preserved"
        );
        assert_eq!(
            loaded.translation.enabled, true,
            "translation enabled should be preserved"
        );
        assert_eq!(
            loaded.translation.vk_code, 0x12,
            "translation vk should be preserved"
        );
        assert_eq!(
            loaded.translation.display_name, "Alt",
            "translation display name should be preserved"
        );
        assert_eq!(
            loaded.translation.target_language,
            TranslationLanguage::English,
            "translation target should be preserved"
        );
    }

    #[test]
    fn missing_translation_config_deserializes_to_default() {
        let toml = r#"
ui_language = "Chinese"
auto_learn_threshold = 2
auto_start = false

[llm]
api_url = "https://api.openai.com/v1"
api_key = ""
model = "gpt-4o-mini"
system_prompt = "prompt"
enabled = true
connectivity_verified = false

[hotkey]
vk_code = 120
modifiers = 0
display_name = "F9"
mode = "Toggle"

[audio]
silence_threshold = 0.01
transcription_language = "zh"
chinese_script = "Simplified"
overlay_opacity = 1.0
input_device = ""
enable_streaming = false

[injection]
use_clipboard = true
clipboard_delay_ms = 150
"#;

        let cfg: AppConfig = toml::from_str(toml).expect("legacy config should deserialize");

        assert_eq!(cfg.translation, TranslationConfig::default());
    }

    /// ATOMIC-SAVE-003: 并发保存不会损坏文件（原子性验证）
    /// 验证快速连续两次 save 不会产生部分写入或空文件
    #[test]
    fn concurrent_saves_do_not_corrupt_file() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let cfg1 = AppConfig::default();
        let mut cfg2 = AppConfig::default();
        cfg2.hotkey.vk_code = 0x79; // Different value

        cfg1.save_to(&env.config_path())
            .expect("first save should succeed");
        cfg2.save_to(&env.config_path())
            .expect("second save should succeed");

        // File should not be empty or partially written
        assert!(env.config_path().exists(), "file should exist");
        let content = std::fs::read_to_string(&env.config_path()).expect("file should be readable");
        assert!(!content.is_empty(), "file should not be empty");

        // Should parse as valid TOML
        let loaded =
            AppConfig::load_from(&env.config_path()).expect("content should be valid TOML");

        // Should have the second config's values (last write wins with atomic save)
        assert_eq!(
            loaded.hotkey.vk_code, 0x79,
            "should have cfg2 values after concurrent saves"
        );
    }

    /// WATCHER-001: 配置文件变更后 watcher 触发 reload
    /// 手动验证占位：需要真实文件 watcher 运行环境
    #[test]
    #[ignore = "requires notify file watcher implementation (TEST-SYNC-CONFIG-WATCHER-001 P2)"]
    fn watcher_reloads_on_external_config_change() {
        // 预期手动验证步骤：
        // 1. 启动 voice-ime.exe -debug
        // 2. 修改 config.toml（如修改 hotkey.vk_code）
        // 3. 观察日志中是否出现 "Config file changed, reloading..."
        // 4. 验证新配置值在 Arc 中生效
        // 5. 验证无 panic 或 crash
        //
        // E2E 验证脚本（PowerShell）：
        //   $configPath = "$env:APPDATA\voice-ime\config.toml"
        //   # 修改 vk_code 为 0x70 (F1)
        //   (Get-Content $configPath) -replace 'vk_code = 0x78', 'vk_code = 0x70' | Set-Content $configPath
        //   # 观察日志
        assert!(
            true,
            "Manual E2E verification required for config file watcher"
        );
    }

    /// UI-I18N-001: TraditionalChinese 序列化/反序列化正确
    #[test]
    fn ui_language_traditional_chinese_serializes_correctly() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();
        let mut cfg = AppConfig::default();
        cfg.ui_language = UiLanguage::TraditionalChinese;

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");

        assert_eq!(loaded.ui_language, UiLanguage::TraditionalChinese);

        // Verify TOML contains the expected string value
        let content = std::fs::read_to_string(env.config_path()).expect("file should be readable");
        assert!(
            content.contains(r#"ui_language = "TraditionalChinese""#),
            "TOML should contain TraditionalChinese variant name, got:\n{}",
            content
        );
    }

    /// WATCHER-002: 频繁修改不会导致重复 reload（debounce 验证）
    #[test]
    #[ignore = "requires notify file watcher with debounce (TEST-SYNC-CONFIG-WATCHER-001 P2)"]
    fn watcher_debounces_rapid_changes() {
        // 预期手动验证步骤：
        // 1. 启动 voice-ime.exe -debug
        // 2. 在 500ms 内快速修改 config.toml 5 次
        // 3. 观察日志：应该只触发 1 次 reload（而非 5 次）
        // 4. 验证 debounce 间隔配置（默认 ~500ms）

        assert!(
            true,
            "Manual E2E verification required for watcher debounce"
        );
    }

    // ============================================================
    // TEST-SYNC-TRANS-001 翻译功能测试同步
    // ============================================================

    /// TRANS-CONFIG-001: TranslationConfig 默认值正确
    #[test]
    fn translation_config_default_values() {
        let cfg = AppConfig::default();
        assert!(
            !cfg.translation.enabled,
            "translation should be disabled by default"
        );
        assert_eq!(
            cfg.translation.vk_code, 0,
            "translation vk_code should be 0 (not set)"
        );
        assert!(cfg.translation.display_name.is_empty());
        assert_eq!(
            cfg.translation.target_language,
            TranslationLanguage::Chinese
        );
    }

    /// TRANS-CONFIG-002: TranslationConfig save/load 往返正确（不破坏现有字段）
    #[test]
    fn translation_config_roundtrip_preserves_values() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.translation.enabled = true;
        cfg.translation.vk_code = 0xA2; // Left Ctrl
        cfg.translation.display_name = "Left Ctrl".to_string();
        cfg.translation.target_language = TranslationLanguage::English;

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");

        assert!(loaded.translation.enabled);
        assert_eq!(loaded.translation.vk_code, 0xA2);
        assert_eq!(loaded.translation.display_name, "Left Ctrl");
        assert_eq!(
            loaded.translation.target_language,
            TranslationLanguage::English
        );
    }

    /// TRANS-CONFIG-003: 旧 config.toml（无 translation 字段）加载时使用默认值
    #[test]
    fn translation_config_missing_field_uses_default() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        // 写入一个不含 translation 字段的旧格式 config
        let old_toml = r#"
ui_language = "Chinese"
auto_learn_threshold = 2
auto_start = false

[llm]
api_url = "https://api.openai.com/v1"
api_key = ""
model = "gpt-4o-mini"
enabled = true
connectivity_verified = false

[hotkey]
vk_code = 120
modifiers = 0
display_name = "F9"
mode = "Toggle"

[audio]
silence_threshold = 0.01
transcription_language = "zh"
overlay_opacity = 1.0

[injection]
use_clipboard = true
clipboard_delay_ms = 150
"#;
        std::fs::write(&env.config_path(), old_toml).unwrap();
        let loaded = AppConfig::load_from(&env.config_path()).expect("should load old config");
        assert!(!loaded.translation.enabled, "default should be disabled");
        assert_eq!(loaded.translation.vk_code, 0);
    }

    // ============================================================
    // TEST-SYNC-PUNCT-001 标点功能测试同步
    // ============================================================

    /// PUNCT-CONFIG-001: PunctuationConfig 默认值为 true
    #[test]
    fn punctuation_config_default_enabled() {
        let cfg = PunctuationConfig::default();
        assert!(
            cfg.enabled,
            "PunctuationConfig should be enabled by default"
        );
    }

    /// PUNCT-CONFIG-002: AppConfig 默认包含 PunctuationConfig
    #[test]
    fn app_config_default_includes_punctuation() {
        let cfg = AppConfig::default();
        assert!(
            cfg.punctuation.enabled,
            "AppConfig default should have punctuation enabled"
        );
    }

    /// PUNCT-CONFIG-003: PunctuationConfig save/load 往返正确
    #[test]
    fn punctuation_config_roundtrip_preserves_values() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.punctuation.enabled = false;

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");

        assert!(
            !loaded.punctuation.enabled,
            "disabled punctuation should survive roundtrip"
        );
    }

    /// PUNCT-CONFIG-004: 旧 config.toml（无 punctuation 字段）加载时使用默认值 enabled=true
    #[test]
    fn punctuation_config_missing_field_uses_default_enabled() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let old_toml = r#"
ui_language = "Chinese"
auto_learn_threshold = 2
auto_start = false

[llm]
api_url = "https://api.openai.com/v1"
api_key = ""
model = "gpt-4o-mini"
enabled = true
connectivity_verified = false

[hotkey]
vk_code = 120
modifiers = 0
display_name = "F9"
mode = "Toggle"

[audio]
silence_threshold = 0.01
transcription_language = "zh"
chinese_script = "Simplified"
overlay_opacity = 1.0
input_device = ""
enable_streaming = false

[injection]
use_clipboard = true
clipboard_delay_ms = 150
"#;
        std::fs::write(&env.config_path(), old_toml).unwrap();
        let loaded = AppConfig::load_from(&env.config_path()).expect("should load old config");
        assert!(
            loaded.punctuation.enabled,
            "missing punctuation field should default to true"
        );
    }

    /// PUNCT-CONFIG-005: PunctuationConfig 显式 enabled=true 可正常序列化
    #[test]
    fn punctuation_config_explicit_true_roundtrip() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.punctuation.enabled = true;

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert!(
            loaded.punctuation.enabled,
            "explicit true should survive roundtrip"
        );
    }

    // ============================================================
    // ASR-DUAL-B-001 双模型配置兼容性测试
    // ============================================================

    /// ASR-CONFIG-001: 默认 asr_model 为 "performance"
    #[test]
    fn asr_model_default_is_performance() {
        let cfg = AppConfig::default();
        assert_eq!(
            cfg.audio.asr_model, "performance",
            "default asr_model must be 'performance'"
        );
    }

    /// ASR-CONFIG-002: 旧 config.toml（无 asr_model 字段）加载时使用默认 "performance"
    /// 保证向后兼容：行为与直换前完全一致（除默认模型本体更换外）
    #[test]
    fn asr_model_missing_field_uses_default_performance() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let old_toml = r#"
ui_language = "Chinese"
auto_learn_threshold = 2
auto_start = false

[llm]
api_url = "https://api.openai.com/v1"
api_key = ""
model = "gpt-4o-mini"
system_prompt = "prompt"
enabled = true
connectivity_verified = false

[hotkey]
vk_code = 120
modifiers = 0
display_name = "F9"
mode = "Toggle"

[audio]
silence_threshold = 0.01
transcription_language = "zh"
chinese_script = "Simplified"
overlay_opacity = 1.0
input_device = ""
enable_streaming = false

[injection]
use_clipboard = true
clipboard_delay_ms = 150
"#;
        std::fs::write(&env.config_path(), old_toml).unwrap();
        let loaded = AppConfig::load_from(&env.config_path()).expect("should load old config");
        assert_eq!(
            loaded.audio.asr_model, "performance",
            "missing asr_model field should default to 'performance'"
        );
    }

    /// ASR-HIDE-ACCURACY-001-CORE: 存量 accuracy 配置加载时静默迁移为 performance
    /// 原断言"accuracy 原样往返保留"方向是错的（UI 已隐藏 accuracy，存量配置应迁移）。
    #[test]
    fn asr_model_accuracy_migrates_to_performance() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.audio.asr_model = "accuracy".to_string();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        // 迁移后应是 performance（不是 accuracy）
        assert_eq!(
            loaded.audio.asr_model, "performance",
            "ASR-HIDE-ACCURACY-001: legacy 'accuracy' must migrate to 'performance' on load"
        );
        // 落盘后再次加载应保持 performance（不重复迁移）
        let reloaded = AppConfig::load_from(&env.config_path()).expect("reload should succeed");
        assert_eq!(
            reloaded.audio.asr_model, "performance",
            "migrated config should persist 'performance' on disk"
        );
    }

    /// ASR-HIDE-ACCURACY-001-CORE: performance 值零回归（不受迁移影响）
    #[test]
    fn asr_model_performance_unchanged_by_migration() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.audio.asr_model = "performance".to_string();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.asr_model, "performance",
            "performance must be unchanged by accuracy migration logic"
        );
    }

    /// ASR-HIDE-ACCURACY-001-CORE: qwen3_online 值零回归（不受迁移影响）
    #[test]
    fn asr_model_qwen3_online_unchanged_by_migration() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.audio.asr_model = "qwen3_online".to_string();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.asr_model, "qwen3_online",
            "qwen3_online must be unchanged by accuracy migration logic"
        );
    }

    // ============================================================
    // TEST-SYNC-QWEN3-001: Qwen3 配置字段 serde 测试
    // ============================================================

    /// QWEN3-CONFIG-001: qwen3 三个字段默认值序列化/反序列化正确
    #[test]
    fn qwen3_fields_default_values_roundtrip() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let cfg = AppConfig::default();
        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");

        assert_eq!(
            loaded.audio.qwen3_api_key, "",
            "qwen3_api_key default should be empty"
        );
        assert_eq!(
            loaded.audio.qwen3_asr_url, "wss://dashscope.aliyuncs.com/api-ws/v1/realtime",
            "qwen3_asr_url default should match config/mod.rs"
        );
        assert_eq!(
            loaded.audio.qwen3_asr_model, "qwen3-asr-flash-realtime",
            "qwen3_asr_model default should match config/mod.rs"
        );
    }

    /// QWEN3-CONFIG-002: qwen3_api_key 非空值往返正确
    #[test]
    fn qwen3_api_key_custom_value_roundtrip() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.audio.qwen3_api_key = "sk-test-key-123456".to_string();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.qwen3_api_key, "sk-test-key-123456",
            "qwen3_api_key custom value should survive roundtrip"
        );
    }

    /// QWEN3-CONFIG-003: 旧 config.toml（无 qwen3 字段）加载时使用默认值
    /// 保证向后兼容：DEC-028 新增字段不破坏现有配置
    #[test]
    fn qwen3_fields_missing_in_old_config_uses_defaults() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let old_toml = r#"
ui_language = "Chinese"
auto_learn_threshold = 2
auto_start = false

[llm]
api_url = "https://api.openai.com/v1"
api_key = ""
model = "gpt-4o-mini"
system_prompt = "prompt"
enabled = true
connectivity_verified = false

[hotkey]
vk_code = 120
modifiers = 0
display_name = "F9"
mode = "Toggle"

[audio]
silence_threshold = 0.01
transcription_language = "zh"
chinese_script = "Simplified"
overlay_opacity = 1.0
input_device = ""
enable_streaming = false
asr_model = "performance"

[injection]
use_clipboard = true
clipboard_delay_ms = 150
"#;
        std::fs::write(&env.config_path(), old_toml).unwrap();
        let loaded = AppConfig::load_from(&env.config_path()).expect("should load old config");
        assert_eq!(
            loaded.audio.qwen3_api_key, "",
            "missing qwen3_api_key should default to empty"
        );
        assert_eq!(
            loaded.audio.qwen3_asr_url, "wss://dashscope.aliyuncs.com/api-ws/v1/realtime",
            "missing qwen3_asr_url should default to config/mod.rs"
        );
        assert_eq!(
            loaded.audio.qwen3_asr_model, "qwen3-asr-flash-realtime",
            "missing qwen3_asr_model should default to config/mod.rs"
        );
    }

    /// QWEN3-CONFIG-004: qwen3_asr_url 自定义值往返正确
    #[test]
    fn qwen3_asr_url_custom_value_roundtrip() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.audio.qwen3_asr_url = "wss://custom.example.com/realtime".to_string();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.qwen3_asr_url, "wss://custom.example.com/realtime",
            "qwen3_asr_url custom value should survive roundtrip"
        );
    }

    /// QWEN3-CONFIG-005: qwen3_asr_model 自定义值往返正确
    #[test]
    fn qwen3_asr_model_custom_value_roundtrip() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.audio.qwen3_asr_model = "qwen3-asr-flash-demo".to_string();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.qwen3_asr_model, "qwen3-asr-flash-demo",
            "qwen3_asr_model custom value should survive roundtrip"
        );
    }

    // ============================================================
    // TEST-SYNC-ITN-TRANS-001: AppConfig::remember_translation_direction
    // 搬迁自 main.rs（REFACTOR-SHARE-TRANSDIR-001）
    // ============================================================

    /// 方向未变时返回 false（调用方据此跳过 save）
    #[test]
    fn remember_direction_returns_false_when_unchanged() {
        let mut cfg = AppConfig::default();
        cfg.translation.target_language = TranslationLanguage::English;
        assert!(
            !cfg.remember_translation_direction(TranslationLanguage::English),
            "same direction should return false"
        );
        assert_eq!(
            cfg.translation.target_language,
            TranslationLanguage::English
        );
    }

    /// 方向变化时更新字段并返回 true
    #[test]
    fn remember_direction_updates_and_returns_true_when_changed() {
        let mut cfg = AppConfig::default();
        cfg.translation.target_language = TranslationLanguage::English;
        assert!(
            cfg.remember_translation_direction(TranslationLanguage::Chinese),
            "changed direction should return true"
        );
        assert_eq!(
            cfg.translation.target_language,
            TranslationLanguage::Chinese
        );
    }
}
