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
    /// ASR 模型选择（DEC-025 + DEC-028 + ASR-041 + ASR-056）：
    /// "performance"(默认,179MB CTC) | "accuracy"(972MB native+hotwords)
    /// | "qwen_audio_online"(在线流式 ASR, qwen-audio-3.0-asr-flash-streaming)
    /// | "fun_asr_realtime"(在线流式 ASR, fun-asr-realtime)
    /// 旧配置无此字段时 serde default 等效于 "performance"，行为与直换前完全一致
    /// ASR-041: "qwen3_online" 已被 "qwen_audio_online" 替代，存量配置自动迁移（见 load/load_from）
    /// ASR-056: "fun_asr_realtime" 新增，与 qwen_audio_online 共用同一 WS 端点与 Inference 协议
    #[serde(default = "default_asr_model")]
    pub asr_model: String,
    /// 在线 ASR API Key（DEC-028，ASR-041-B 改名为通用名，与具体模型代号解耦）
    /// #[serde(alias)] 保证存量 config.toml 里的 `qwen3_api_key` 仍能正确读入
    /// ASR-056: qwen_audio_online 与 fun_asr_realtime 两族共用同一 API Key（同 workspace）
    #[serde(default, alias = "qwen3_api_key")]
    pub asr_online_api_key: String,
    /// 在线 ASR 服务 URL（ASR-041-B 改名为通用名，与具体模型代号解耦）
    /// 默认北京 region：wss://{WorkspaceId}.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference
    /// #[serde(alias)] 免疫含 038-A 字段名的存量配置
    /// ASR-056: qwen_audio_online 与 fun_asr_realtime 两族共用同一端点（官方文档并列两模型）
    #[serde(default = "default_asr_online_url", alias = "qwen_asr_url")]
    pub asr_online_url: String,
    /// 在线 ASR 模型 ID（ASR-041-B 改名为通用名）
    /// ASR-056: 此字段为「当前生效的在线模型串」，由 asr_model 顶层选择器决定族，
    /// 加载时经 resolve_online_model_id 守卫：配置串前缀必须与所选族匹配，否则回落到族默认。
    /// - asr_model="qwen_audio_online" → 默认 "qwen-audio-3.0-asr-flash-streaming"
    /// - asr_model="fun_asr_realtime"  → 默认 "fun-asr-realtime"
    /// 前缀守卫防「UI 选 fun-asr 但配置串还是 qwen 的」静默串味（主控 2026-08-18 裁决）
    #[serde(default = "default_asr_online_model", alias = "qwen_asr_model")]
    pub asr_online_model: String,
    /// ASR-056: VAD 断句静音阈值（ms），config.toml 隐藏字段（不进 UI，DEC-031）。
    ///
    /// 主控 2026-08-18 验收裁决：默认 800ms 保持基线（与 qwen 既有行为一致），
    /// 让 Gavin 的 A/B 对比能分清「fun-asr 更快」是模型带来的还是 silence 带来的。
    /// 500 vs 800 可以作为独立一轴单独 A/B——改 config.toml 即可，不用重新出包。
    /// 官方文档默认 1300ms，范围 200-6000ms。
    #[serde(default = "default_asr_online_max_sentence_silence")]
    pub asr_online_max_sentence_silence: i64,
}

fn default_overlay_opacity() -> f32 {
    1.0
}

fn default_asr_model() -> String {
    "performance".to_string()
}

fn default_asr_online_url() -> String {
    // ASR-038-B: Inference API 端点主机名须为 {WorkspaceId}.{region}.maas.aliyuncs.com
    // （035 研究文档 A6）。现有项目 WorkspaceId = llm-kudx4dj2bfqn4gr2（北京 region），
    // 与旧 Realtime API 同一 workspace 同一 key，仅路径 /realtime → /inference。
    "wss://llm-kudx4dj2bfqn4gr2.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference".to_string()
}

fn default_asr_online_model() -> String {
    // ASR-056: 此默认值是 qwen_audio_online 族的默认模型串。
    // fun_asr_realtime 族的默认模型串见 default_online_model_for_family。
    "qwen-audio-3.0-asr-flash-streaming".to_string()
}

/// ASR-056: VAD 断句静音阈值默认值（ms）
///
/// 主控 2026-08-18 验收裁决：保持 800ms 基线（与 qwen 既有行为一致），
/// 让 Gavin 的 A/B 对比能分清「fun-asr 更快」是模型带来的还是 silence 带来的。
/// 500 vs 800 可作为独立一轴单独 A/B（改 config.toml 即可，不用重新出包）。
fn default_asr_online_max_sentence_silence() -> i64 {
    800
}

/// ASR-056: 每个在线 ASR 族的默认模型串（前缀守卫回落用）
///
/// 族由 `asr_model` 顶层选择器决定（"qwen_audio_online" / "fun_asr_realtime"）。
/// 前缀守卫规则（主控 2026-08-18 裁决）：
/// - 配置里的 `asr_online_model` 串前缀必须与所选族的默认串前缀相符
///   （前缀 = 第一个 `-` 之前的部分，如 "qwen-audio..." → "qwen-audio"，
///   "fun-asr..." → "fun-asr"）。这样 `fun-asr-realtime-2025-11-07` 快照版
///   仍能覆盖默认 `fun-asr-realtime`。
/// - 冲突（UI 选 fun-asr 但配置串还是 qwen 的）→ 打警告 + 回落到族默认串
/// - 两族都不匹配（自建/代理/未知前缀）→ 照用并记一条日志
pub fn default_online_model_for_family(asr_model: &str) -> &'static str {
    match asr_model {
        "fun_asr_realtime" => "fun-asr-realtime",
        // qwen_audio_online 或其他任何值（含 performance/accuracy 等非在线模式）都回落到 qwen 族默认
        // —— 非在线模式不会走在线路径，此返回值不会被使用，但保持一致性
        _ => "qwen-audio-3.0-asr-flash-streaming",
    }
}

/// ASR-056: 提取模型串的族前缀（前两段，用 `-` 分割取 2 段再 join，小写化）
///
/// 主控 2026-08-18 验收裁决：取第一个 `-` 之前的部分会得到 "qwen"/"fun"，
/// 而 known_prefixes 是 ["qwen-audio","fun-asr"]，永远匹配不上 → 守卫失效。
/// 修法：取前**两段**，与注释、known_prefixes、测试期望一致。
///
/// 例："qwen-audio-3.0-asr-flash-streaming" → "qwen-audio"
///     "fun-asr-realtime" → "fun-asr"
///     "fun-asr-realtime-2025-11-07" → "fun-asr"
///     "my-custom-model" → "my-custom"
///     "nodash" → "nodash"
///     "single-dash" → "single-dash"
fn model_family_prefix(model_id: &str) -> String {
    // 🔴 必须是「前两段」而不是 splitn(2)：splitn 会把余下全部留在第二段
    // （"qwen-audio-3.0-asr" → ["qwen", "audio-3.0-asr"]），拼回去等于原串，
    // 于是与 known_prefixes 永远比不上、守卫静默失效。用 split + take(2)。
    let mut it = model_id.split('-');
    match (it.next(), it.next()) {
        (Some(first), Some(second)) => {
            format!("{}-{}", first.to_lowercase(), second.to_lowercase())
        }
        (Some(first), None) => first.to_lowercase(),
        _ => String::new(),
    }
}

/// ASR-056: 在线模型串前缀守卫（纯函数，可单测）
///
/// 根据顶层 `asr_model` 选择器（族权威）与配置里的 `asr_online_model` 串（可能过期），
/// 返回实际生效的模型串。
///
/// 规则：
/// - 族前缀匹配 → 照用配置串（允许快照版/自定义版覆盖族默认）
/// - 族前缀不匹配且配置串不属于任何已知族 → 照用并记日志（自建/代理场景）
/// - 族前缀不匹配且配置串属于另一个已知族 → 回落到族默认串 + warn
/// - 配置串为空 → 回落到族默认串（serde default 路径）
pub fn resolve_online_model_id(asr_model: &str, configured_model_id: &str) -> String {
    let family_default = default_online_model_for_family(asr_model);
    let family_prefix = model_family_prefix(family_default);

    let configured_trimmed = configured_model_id.trim();
    if configured_trimmed.is_empty() {
        return family_default.to_string();
    }

    let configured_prefix = model_family_prefix(configured_trimmed);
    if configured_prefix == family_prefix {
        // 族匹配，照用配置串（可能是快照版/自定义版）
        return configured_trimmed.to_string();
    }

    // 检查配置串是否属于另一个已知族
    let known_prefixes = ["qwen-audio", "fun-asr"];
    if known_prefixes.contains(&configured_prefix.as_str()) {
        // 属于另一个已知族 → 静默串味风险，回落 + warn
        log::warn!(
            "ASR-056: online model id '{}' prefix '{}' does not match family '{}' (asr_model='{}'); \
             falling back to family default '{}'",
            configured_trimmed,
            configured_prefix,
            family_prefix,
            asr_model,
            family_default
        );
        return family_default.to_string();
    }

    // 不属于任何已知族（自建/代理）→ 照用并记日志
    log::info!(
        "ASR-056: online model id '{}' has unknown prefix '{}' (not in {:?}); \
         using as-is for family '{}'",
        configured_trimmed,
        configured_prefix,
        known_prefixes,
        asr_model
    );
    configured_trimmed.to_string()
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
            asr_online_api_key: String::new(),
            asr_online_url: default_asr_online_url(),
            asr_online_model: default_asr_online_model(),
            asr_online_max_sentence_silence: default_asr_online_max_sentence_silence(),
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

        // ASR-041: 存量 qwen3_online 配置静默迁移为 qwen_audio_online。
        // 背景：UI 下拉选项已从 qwen3_online 切换为 qwen_audio_online（新引擎替代旧引擎），
        // 存量用户配置若仍是 qwen3_online 会致下拉不匹配任何 option → 静默落到 performance。
        // 迁移：load 时检测到 asr_model=="qwen3_online" 静默改写为 "qwen_audio_online" 并落盘保存。
        if cfg.audio.asr_model == "qwen3_online" {
            log::info!("ASR-041: migrating legacy asr_model='qwen3_online' -> 'qwen_audio_online'");
            cfg.audio.asr_model = "qwen_audio_online".to_string();
            if let Err(e) = cfg.save() {
                log::warn!("ASR-041: failed to persist migrated config: {}", e);
            }
        }

        // ASR-056: 在线模型串前缀守卫——防止 UI 选 fun-asr 但配置串还是 qwen 的静默串味。
        // 只对在线流式族（qwen_audio_online / fun_asr_realtime）生效，非在线模式不动。
        if cfg.audio.asr_model == "qwen_audio_online" || cfg.audio.asr_model == "fun_asr_realtime" {
            let resolved =
                resolve_online_model_id(&cfg.audio.asr_model, &cfg.audio.asr_online_model);
            if resolved != cfg.audio.asr_online_model {
                log::warn!(
                    "ASR-056: online model id resolved '{}' -> '{}' (asr_model='{}'), persisting",
                    cfg.audio.asr_online_model,
                    resolved,
                    cfg.audio.asr_model
                );
                cfg.audio.asr_online_model = resolved;
                if let Err(e) = cfg.save() {
                    log::warn!("ASR-056: failed to persist resolved online model id: {}", e);
                }
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

        // ASR-041: 存量 qwen3_online 配置静默迁移为 qwen_audio_online（load_from 同 load）。
        let migrated_from_qwen3 = cfg.audio.asr_model == "qwen3_online";
        if migrated_from_qwen3 {
            log::info!(
                "ASR-041: migrating legacy asr_model='qwen3_online' -> 'qwen_audio_online' (load_from)"
            );
            cfg.audio.asr_model = "qwen_audio_online".to_string();
            if let Err(e) = cfg.save_to(path) {
                log::warn!("ASR-041: failed to persist migrated config: {}", e);
            }
        }

        // ASR-056: 在线模型串前缀守卫（load_from 同 load）。
        if cfg.audio.asr_model == "qwen_audio_online" || cfg.audio.asr_model == "fun_asr_realtime" {
            let resolved =
                resolve_online_model_id(&cfg.audio.asr_model, &cfg.audio.asr_online_model);
            if resolved != cfg.audio.asr_online_model {
                log::warn!(
                    "ASR-056: online model id resolved '{}' -> '{}' (asr_model='{}'), persisting (load_from)",
                    cfg.audio.asr_online_model,
                    resolved,
                    cfg.audio.asr_model
                );
                cfg.audio.asr_online_model = resolved;
                if let Err(e) = cfg.save_to(path) {
                    log::warn!("ASR-056: failed to persist resolved online model id: {}", e);
                }
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
            "performance must not be affected by migrations"
        );
    }

    // =====================================================================
    // ASR-041: 存量 qwen3_online 配置静默迁移为 qwen_audio_online
    // =====================================================================

    /// ASR-041: 存量 qwen3_online 配置加载时静默迁移为 qwen_audio_online
    #[test]
    fn asr_model_qwen3_online_migrates_to_qwen_audio_online() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.audio.asr_model = "qwen3_online".to_string();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        // 迁移后应是 qwen_audio_online（不是 qwen3_online）
        assert_eq!(
            loaded.audio.asr_model, "qwen_audio_online",
            "ASR-041: legacy 'qwen3_online' must migrate to 'qwen_audio_online' on load"
        );
        // 落盘后再次加载应保持 qwen_audio_online（不重复迁移）
        let reloaded = AppConfig::load_from(&env.config_path()).expect("reload should succeed");
        assert_eq!(
            reloaded.audio.asr_model, "qwen_audio_online",
            "migrated config should persist 'qwen_audio_online' on disk"
        );
    }

    /// ASR-041: qwen_audio_online 值零回归（不受迁移影响）
    #[test]
    fn asr_model_qwen_audio_online_unchanged_by_migration() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.audio.asr_model = "qwen_audio_online".to_string();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.asr_model, "qwen_audio_online",
            "qwen_audio_online must not be affected by qwen3_online migration"
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
            loaded.audio.asr_online_api_key, "",
            "asr_online_api_key default should be empty"
        );
    }

    /// QWEN3-CONFIG-002: asr_online_api_key 非空值往返正确
    #[test]
    fn asr_online_api_key_custom_value_roundtrip() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.audio.asr_online_api_key = "sk-test-key-123456".to_string();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.asr_online_api_key, "sk-test-key-123456",
            "asr_online_api_key custom value should survive roundtrip"
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
            loaded.audio.asr_online_api_key, "",
            "missing asr_online_api_key should default to empty"
        );
    }

    /// ASR-041-B: serde alias 实测 —— 含旧字段名 `qwen3_api_key` 的 toml 能正确读入
    /// 这是本单的生死线：没有 alias，Gavin 的 API key 会被静默丢弃
    #[test]
    fn asr_online_api_key_serde_alias_reads_legacy_qwen3_api_key() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();
        // 先用 default 保存完整配置（确保所有必填段都在）
        let mut cfg = AppConfig::default();
        cfg.audio.asr_online_api_key = "sk-legacy-key".to_string();
        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        // 然后把保存的 toml 里的 asr_online_api_key 改回旧字段名 qwen3_api_key
        let toml_content = std::fs::read_to_string(&env.config_path()).expect("read toml");
        let patched = toml_content.replace("asr_online_api_key", "qwen3_api_key");
        std::fs::write(&env.config_path(), patched).expect("write patched toml");
        // 加载 —— alias 应该把旧字段名 qwen3_api_key 读入 asr_online_api_key
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.asr_online_api_key, "sk-legacy-key",
            "serde alias must read legacy 'qwen3_api_key' field into 'asr_online_api_key'"
        );
    }

    /// ASR-041-B: 含旧字段 qwen3_asr_url / qwen3_asr_model 的 toml 仍能正常解析
    /// 任务书验收项6：这条必须实测，不能推断。
    /// 这两个字段已从结构体删除，serde 默认忽略未知字段（未设 deny_unknown_fields）。
    /// Gavin 的 config.toml 实打实有这两行，必须确认不会解析失败。
    #[test]
    fn legacy_qwen3_asr_fields_ignored_without_error() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();
        // 用 default 保存完整配置
        let cfg = AppConfig::default();
        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        // 在 toml 里追加旧字段（模拟 Gavin 的存量 config）
        let toml_content = std::fs::read_to_string(&env.config_path()).expect("read toml");
        let patched = format!(
            "{}qwen3_asr_url = \"wss://dashscope.aliyuncs.com/api-ws/v1/realtime\"\nqwen3_asr_model = \"qwen3-asr-flash-realtime\"\n",
            toml_content
        );
        std::fs::write(&env.config_path(), patched).expect("write patched toml");
        // 加载 —— 旧字段应被忽略，不报错，其余字段正确读入
        let loaded =
            AppConfig::load_from(&env.config_path()).expect("load with legacy fields must succeed");
        // 确认关键字段未受影响
        assert_eq!(
            loaded.audio.asr_model, cfg.audio.asr_model,
            "asr_model must not be affected by legacy qwen3_asr_* fields"
        );
        assert_eq!(
            loaded.audio.asr_online_api_key, cfg.audio.asr_online_api_key,
            "asr_online_api_key must not be affected by legacy qwen3_asr_* fields"
        );
    }

    /// ASR-041-B-补: `asr_online_url` 的 serde alias —— 含旧字段名 `qwen_asr_url` 的 toml
    /// 必须把 URL 读入 `asr_online_url`（否则 038-A 时代配置在设置界面保存后静默丢失）。
    /// 对照已验证的 `asr_online_api_key` alias（QWEN3-CONFIG-004 同族）补齐 url 侧。
    #[test]
    fn asr_online_url_serde_alias_reads_legacy_qwen_asr_url() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();
        let mut cfg = AppConfig::default();
        cfg.audio.asr_online_url =
            "wss://llm-kudx4dj2bfqn4gr2.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference"
                .to_string();
        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        // 把保存的 toml 里的 asr_online_url 改回旧字段名 qwen_asr_url
        let toml_content = std::fs::read_to_string(&env.config_path()).expect("read toml");
        let patched = toml_content.replace("asr_online_url", "qwen_asr_url");
        std::fs::write(&env.config_path(), patched).expect("write patched toml");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.asr_online_url,
            "wss://llm-kudx4dj2bfqn4gr2.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference",
            "serde alias must read legacy 'qwen_asr_url' field into 'asr_online_url'"
        );
    }

    /// ASR-041-B-补: `asr_online_model` 的 serde alias —— 含旧字段名 `qwen_asr_model` 的 toml
    /// 必须把模型名读入 `asr_online_model`（与 url alias 同族，一并对齐防漂移）。
    #[test]
    fn asr_online_model_serde_alias_reads_legacy_qwen_asr_model() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();
        let mut cfg = AppConfig::default();
        cfg.audio.asr_online_model = "qwen-audio-3.0-asr-flash-streaming".to_string();
        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        // 把保存的 toml 里的 asr_online_model 改回旧字段名 qwen_asr_model
        let toml_content = std::fs::read_to_string(&env.config_path()).expect("read toml");
        let patched = toml_content.replace("asr_online_model", "qwen_asr_model");
        std::fs::write(&env.config_path(), patched).expect("write patched toml");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.asr_online_model, "qwen-audio-3.0-asr-flash-streaming",
            "serde alias must read legacy 'qwen_asr_model' field into 'asr_online_model'"
        );
    }

    /// ASR-041-补: load() 路径的 qwen3_online → qwen_audio_online 迁移。
    /// load_from() 已由 `asr_model_qwen3_online_migrates_to_qwen_audio_online` 覆盖，
    /// 但 load() 是独立实现（自身读文件 + 迁移 + save 落盘），必须独立测，防两处漂移。
    /// 注意：load() 用 config_path()（current_exe 同级 config.toml），测试进程的
    /// current_exe 是 target/debug/deps/ 下的测试二进制，写这里不污染用户配置；
    /// 用 TEST_MUTEX 串行化防并行冲突。
    #[test]
    fn asr_model_qwen3_online_migrates_via_load_path() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let path = crate::config::AppConfig::config_path();
        let existed = path.exists();
        // 写一份 asr_model=qwen3_online 的 toml
        let mut cfg = AppConfig::default();
        cfg.audio.asr_model = "qwen3_online".to_string();
        cfg.save_to(&path).expect("save should succeed");
        let loaded = AppConfig::load().expect("load should succeed");
        // 迁移生效
        assert_eq!(
            loaded.audio.asr_model, "qwen_audio_online",
            "load() path must migrate legacy 'qwen3_online' to 'qwen_audio_online'"
        );
        // 落盘后再次 load 应保持 qwen_audio_online（迁移幂等）
        let reloaded = AppConfig::load().expect("reload should succeed");
        assert_eq!(
            reloaded.audio.asr_model, "qwen_audio_online",
            "load() migration must be idempotent on disk"
        );
        // 清理：恢复现场（删除或还原为默认配置）
        if existed {
            let _ = AppConfig::default().save_to(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }

    // ============================================================
    // ASR-038-B: QwenAudioOnline Inference API 配置（独立于 Qwen3Online Realtime）
    // 三条端点缺陷的回归防护：
    //   ① default_asr_online_url 必须含 WorkspaceId（非 dashscope.aliyuncs.com）
    //   ② default_asr_online_url 路径必须为 /api-ws/v1/inference（非 /realtime）
    //   ③ asr_online_model 默认值为 qwen-audio-3.0-asr-flash-streaming
    // ============================================================

    /// ASR-038-B-001: default_asr_online_url 必须是 Inference API 端点（含 WorkspaceId + /inference 路径）
    #[test]
    fn asr_online_url_default_is_inference_endpoint_with_workspace_id() {
        let cfg = AppConfig::default();
        let url = &cfg.audio.asr_online_url;
        // 缺陷③回归防护：主机名不得为 dashscope.aliyuncs.com（那是 Realtime API 的主机）
        assert!(
            !url.contains("dashscope.aliyuncs.com"),
            "asr_online_url default must not use dashscope.aliyuncs.com (Realtime API host), got: {}",
            url
        );
        // 必须含 WorkspaceId 子域（035 研究文档 A6：{WorkspaceId}.cn-beijing.maas.aliyuncs.com）
        assert!(
            url.contains("llm-kudx4dj2bfqn4gr2.cn-beijing.maas.aliyuncs.com"),
            "asr_online_url default must contain WorkspaceId subdomain, got: {}",
            url
        );
        // 路径必须是 /api-ws/v1/inference（不是 /realtime）
        assert!(
            url.ends_with("/api-ws/v1/inference"),
            "asr_online_url default must end with /api-ws/v1/inference, got: {}",
            url
        );
    }

    /// ASR-038-B-002: default_asr_online_model 必须是 qwen-audio-3.0-asr-flash-streaming
    #[test]
    fn asr_online_model_default_is_streaming_model() {
        let cfg = AppConfig::default();
        assert_eq!(
            cfg.audio.asr_online_model, "qwen-audio-3.0-asr-flash-streaming",
            "asr_online_model default must be qwen-audio-3.0-asr-flash-streaming"
        );
    }

    /// ASR-038-B-003: asr_online_url 自定义值往返正确
    #[test]
    fn asr_online_url_custom_value_roundtrip() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.audio.asr_online_url =
            "wss://my-workspace.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference".to_string();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.asr_online_url,
            "wss://my-workspace.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference",
            "asr_online_url custom value should survive roundtrip"
        );
    }

    /// ASR-038-B-004: asr_online_model 自定义值往返正确
    #[test]
    fn asr_online_model_custom_value_roundtrip() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let env = TestEnv::new();

        let mut cfg = AppConfig::default();
        cfg.audio.asr_online_model = "qwen-audio-3.0-asr-demo".to_string();

        cfg.save_to(&env.config_path())
            .expect("save should succeed");
        let loaded = AppConfig::load_from(&env.config_path()).expect("load should succeed");
        assert_eq!(
            loaded.audio.asr_online_model, "qwen-audio-3.0-asr-demo",
            "asr_online_model custom value should survive roundtrip"
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

    /// ASR-038-B-008: MAX_RECORD_SECONDS 是录音/轮询硬上限的唯一权威来源，
    /// 039 的 translate poll 硬上限 = MAX_RECORD_SECONDS + 5 依赖此值不被随意改动。
    #[test]
    fn max_record_seconds_is_300() {
        assert_eq!(
            MAX_RECORD_SECONDS, 300,
            "MAX_RECORD_SECONDS must stay 300s (translate poll hard cap derives from it)"
        );
    }

    // ============================================================
    // ASR-056: 在线模型串前缀守卫纯函数测试
    // 防止 UI 选 fun-asr 但配置串还是 qwen 的静默串味（主控 2026-08-18 裁决）
    // ============================================================

    #[test]
    fn asr_056_family_default_qwen() {
        assert_eq!(
            default_online_model_for_family("qwen_audio_online"),
            "qwen-audio-3.0-asr-flash-streaming"
        );
    }

    #[test]
    fn asr_056_family_default_fun_asr() {
        assert_eq!(
            default_online_model_for_family("fun_asr_realtime"),
            "fun-asr-realtime"
        );
    }

    #[test]
    fn asr_056_family_default_non_online_falls_back_to_qwen() {
        // 非在线模式不会走在线路径，但保持一致性
        assert_eq!(
            default_online_model_for_family("performance"),
            "qwen-audio-3.0-asr-flash-streaming"
        );
        assert_eq!(
            default_online_model_for_family("accuracy"),
            "qwen-audio-3.0-asr-flash-streaming"
        );
        assert_eq!(
            default_online_model_for_family("unknown"),
            "qwen-audio-3.0-asr-flash-streaming"
        );
    }

    #[test]
    fn asr_056_resolve_prefix_match_uses_configured() {
        // 族匹配 → 照用配置串
        assert_eq!(
            resolve_online_model_id("qwen_audio_online", "qwen-audio-3.0-asr-flash-streaming"),
            "qwen-audio-3.0-asr-flash-streaming"
        );
        assert_eq!(
            resolve_online_model_id("fun_asr_realtime", "fun-asr-realtime"),
            "fun-asr-realtime"
        );
    }

    #[test]
    fn asr_056_resolve_prefix_match_allows_snapshot_version() {
        // 快照版/自定义版覆盖族默认（前缀匹配即可）
        assert_eq!(
            resolve_online_model_id("fun_asr_realtime", "fun-asr-realtime-2025-11-07"),
            "fun-asr-realtime-2025-11-07"
        );
        assert_eq!(
            resolve_online_model_id("qwen_audio_online", "qwen-audio-3.0-asr-flash-streaming-v2"),
            "qwen-audio-3.0-asr-flash-streaming-v2"
        );
    }

    #[test]
    fn asr_056_resolve_prefix_mismatch_known_family_falls_back() {
        // UI 选 fun-asr 但配置串还是 qwen 的 → 回落到 fun-asr 默认
        assert_eq!(
            resolve_online_model_id("fun_asr_realtime", "qwen-audio-3.0-asr-flash-streaming"),
            "fun-asr-realtime"
        );
        // 反向：UI 选 qwen 但配置串是 fun-asr → 回落到 qwen 默认
        assert_eq!(
            resolve_online_model_id("qwen_audio_online", "fun-asr-realtime"),
            "qwen-audio-3.0-asr-flash-streaming"
        );
    }

    #[test]
    fn asr_056_resolve_empty_configured_falls_back_to_family_default() {
        assert_eq!(
            resolve_online_model_id("qwen_audio_online", ""),
            "qwen-audio-3.0-asr-flash-streaming"
        );
        assert_eq!(
            resolve_online_model_id("fun_asr_realtime", "   "),
            "fun-asr-realtime"
        );
    }

    #[test]
    fn asr_056_resolve_unknown_prefix_uses_as_is() {
        // 自建/代理场景：照用并记日志（这里只验证返回值，日志在运行时观察）
        assert_eq!(
            resolve_online_model_id("qwen_audio_online", "my-custom-model"),
            "my-custom-model"
        );
        assert_eq!(
            resolve_online_model_id("fun_asr_realtime", "proxy-asr-v1"),
            "proxy-asr-v1"
        );
    }

    #[test]
    fn asr_056_resolve_case_insensitive_prefix() {
        // 前缀比较小写化
        assert_eq!(
            resolve_online_model_id("fun_asr_realtime", "Fun-ASR-Realtime"),
            "Fun-ASR-Realtime"
        );
    }

    #[test]
    fn asr_056_model_family_prefix_extracts_correctly() {
        assert_eq!(
            model_family_prefix("qwen-audio-3.0-asr-flash-streaming"),
            "qwen-audio"
        );
        assert_eq!(model_family_prefix("fun-asr-realtime"), "fun-asr");
        assert_eq!(
            model_family_prefix("fun-asr-realtime-2025-11-07"),
            "fun-asr"
        );
        assert_eq!(model_family_prefix("my-custom-model"), "my-custom");
        assert_eq!(model_family_prefix("nodash"), "nodash");
    }

    /// ASR-056: load() 路径前缀守卫集成——选 fun_asr_realtime 但配置串是 qwen 的应回落
    #[test]
    fn asr_056_load_resolves_mismatched_online_model_id() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let path = crate::config::AppConfig::config_path();
        let existed = path.exists();
        let mut cfg = AppConfig::default();
        cfg.audio.asr_model = "fun_asr_realtime".to_string();
        // 故意写一个 qwen 的串——模拟用户从 0.8.0 升上来、UI 选了 fun-asr 但配置串没改
        cfg.audio.asr_online_model = "qwen-audio-3.0-asr-flash-streaming".to_string();
        cfg.save_to(&path).expect("save should succeed");
        let loaded = AppConfig::load().expect("load should succeed");
        // 守卫应回落到 fun-asr-realtime
        assert_eq!(
            loaded.audio.asr_online_model, "fun-asr-realtime",
            "ASR-056: load() must resolve mismatched online model id to family default"
        );
        // 落盘后再次 load 应保持 fun-asr-realtime（幂等）
        let reloaded = AppConfig::load().expect("reload should succeed");
        assert_eq!(
            reloaded.audio.asr_online_model, "fun-asr-realtime",
            "ASR-056: resolved model id must persist on disk"
        );
        // 清理
        if existed {
            let _ = AppConfig::default().save_to(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}
