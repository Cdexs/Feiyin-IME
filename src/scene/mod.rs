//! SCENE-SENSE-001-CORE 场景感知模块（Phase 2，DEC-031-⑤）
//!
//! 场景 = 注入目标窗口的语义。录音启动瞬间采集前台窗口信号（进程名+标题，<1ms 同步），
//! 本地分类为场景类别，给 LLM prompt 注入 F4 场景风格段 + 做格式安全裁决（multiline_safe）。
//!
//! 隐私边界：默认只把「场景类别+风格指令」写进 prompt，exe 名/窗口标题不上送 LLM。
//! 算法与规则数据分离：规则在 scene-rules.toml（include_str! 内置默认，运行时可覆盖）。

use std::sync::OnceLock;

use serde::Deserialize;

/// 内置默认规则文件
const BUILTIN_RULES: &str = include_str!("../../scene-rules.toml");

// ============================================================
// 公共数据结构
// ============================================================

/// 场景类别。决定 multiline_safe 默认值与 F4 风格段。
/// Unknown = 未命中规则，行为等同 Phase 1（不注入 F4，multiline_safe=false 保守）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneKind {
    Chat,
    Email,
    Doc,
    IdeTerminal,
    Browser,
    Unknown,
}

impl SceneKind {
    /// 用于 F4 prompt 段的场景类别标签（语义描述，不含 exe 名/标题）
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Email => "email",
            Self::Doc => "document",
            Self::IdeTerminal => "IDE/terminal",
            Self::Browser => "browser",
            Self::Unknown => "unknown",
        }
    }
}

/// 采集到的场景上下文。随录音会话传递到 LLM 阶段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneContext {
    pub scene: SceneKind,
    /// 进程 exe 名（仅本地使用，默认不上送 LLM）
    pub app_exe: String,
    /// 窗口标题（仅本地使用；send_window_title=true 时截断后可上送）
    pub window_title: String,
    /// 多行注入是否安全（本地词表确定性判定，猜错一次即事故，只信词表）
    pub multiline_safe: bool,
    /// 规则表匹配出的风格指令（写入 LLM prompt F4 段）
    pub style_hint: String,
}

impl SceneContext {
    /// Unknown 场景的安全降级：不注入 F4，multiline_safe=false（等同 Phase 1）。
    pub fn unknown() -> Self {
        Self {
            scene: SceneKind::Unknown,
            app_exe: String::new(),
            window_title: String::new(),
            multiline_safe: false,
            style_hint: String::new(),
        }
    }

    /// Unknown 场景判断
    pub fn is_unknown(&self) -> bool {
        self.scene == SceneKind::Unknown
    }
}

// ============================================================
// 规则数据结构（反序列化）
// ============================================================

#[derive(Debug, Clone, Deserialize)]
struct Rules {
    #[serde(default)]
    scene: Vec<SceneRule>,
}

#[derive(Debug, Clone, Deserialize)]
struct SceneRule {
    kind: String,
    #[serde(default)]
    style: String,
    #[serde(default)]
    multiline_safe: bool,
    #[serde(default)]
    exe: Vec<String>,
    #[serde(default)]
    title_keywords: Vec<String>,
    /// macOS 预留字段（Phase 4 实施，当前忽略）
    #[serde(default)]
    #[allow(dead_code)]
    bundle_id: Vec<String>,
}

// ============================================================
// 编译后规则
// ============================================================

#[derive(Debug, Clone)]
struct CompiledRule {
    kind: SceneKind,
    style: String,
    multiline_safe: bool,
    /// exe 名转小写后的集合（不区分大小写精确匹配）
    exe_lower: std::collections::HashSet<String>,
    title_keywords: Vec<String>,
}

#[derive(Debug, Clone)]
struct CompiledRules {
    rules: Vec<CompiledRule>,
}

impl CompiledRules {
    fn from_rules(r: Rules) -> Self {
        let rules = r
            .scene
            .into_iter()
            .map(|sr| CompiledRule {
                kind: parse_kind(&sr.kind),
                style: sr.style,
                multiline_safe: sr.multiline_safe,
                exe_lower: sr.exe.iter().map(|e| e.to_lowercase()).collect(),
                // SCENE-TITLE-CASE-001: title_keywords 编译期统一 to_lowercase 存储，
                // 匹配时 title 也 to_lowercase 后比较，实现大小写不敏感匹配。
                title_keywords: sr.title_keywords.iter().map(|k| k.to_lowercase()).collect(),
            })
            .collect();
        Self { rules }
    }

    /// 分类匹配：exe 精确匹配（不区分大小写）→ 标题关键词 → Unknown。
    /// 隐私：exe 名/标题只用于本地匹配，不进入返回的 style_hint。
    ///
    /// SCENE-SENSE-001-CORE 浏览器细分：当 exe 匹配到 Browser 时，检查其他场景
    /// （email/doc）的 title_keywords 是否命中，命中则覆盖为更具体场景。
    /// 设计依据（DESIGN-FORMAT-SCENE-001 3.3 节 + DEC-031-⑤）：
    /// "浏览器细分靠 title_keywords 兜邮箱/文档场景"——浏览器 exe 命中后，
    /// 标题含"收件箱/Gmail/Google Docs/腾讯文档"等 → 重分类为 email/doc
    /// （享受对应 multiline_safe 与风格）。
    fn classify(&self, exe: &str, title: &str) -> SceneContext {
        let exe_lower = exe.to_lowercase();
        // SCENE-TITLE-CASE-001: title 只 to_lowercase 一次，避免在关键词循环里重复分配
        let title_lower = title.to_lowercase();

        // 优先级 1：exe 精确匹配
        for rule in &self.rules {
            if rule.exe_lower.contains(&exe_lower) {
                // 浏览器细分：exe 命中 Browser 后，检查其他场景 title_keywords 覆盖
                if rule.kind == SceneKind::Browser && !title.is_empty() {
                    for other_rule in &self.rules {
                        if other_rule.kind == SceneKind::Browser {
                            continue; // 跳过浏览器自身的 title_keywords
                        }
                        for kw in &other_rule.title_keywords {
                            // SCENE-TITLE-CASE-001: 大小写不敏感匹配
                            if title_lower.contains(kw.as_str()) {
                                return SceneContext {
                                    scene: other_rule.kind,
                                    app_exe: exe.to_string(),
                                    window_title: title.to_string(),
                                    multiline_safe: other_rule.multiline_safe,
                                    style_hint: other_rule.style.clone(),
                                };
                            }
                        }
                    }
                }
                return SceneContext {
                    scene: rule.kind,
                    app_exe: exe.to_string(),
                    window_title: title.to_string(),
                    multiline_safe: rule.multiline_safe,
                    style_hint: rule.style.clone(),
                };
            }
        }

        // 优先级 2：标题关键词（exe 未命中任何规则时）
        if !title.is_empty() {
            for rule in &self.rules {
                for kw in &rule.title_keywords {
                    // SCENE-TITLE-CASE-001: 大小写不敏感匹配
                    if title_lower.contains(kw.as_str()) {
                        return SceneContext {
                            scene: rule.kind,
                            app_exe: exe.to_string(),
                            window_title: title.to_string(),
                            multiline_safe: rule.multiline_safe,
                            style_hint: rule.style.clone(),
                        };
                    }
                }
            }
        }

        // 优先级 3：Unknown 安全降级
        SceneContext {
            scene: SceneKind::Unknown,
            app_exe: exe.to_string(),
            window_title: title.to_string(),
            multiline_safe: false,
            style_hint: String::new(),
        }
    }
}

fn parse_kind(s: &str) -> SceneKind {
    match s.trim().to_lowercase().as_str() {
        "chat" => SceneKind::Chat,
        "email" => SceneKind::Email,
        "doc" => SceneKind::Doc,
        "ide_terminal" | "ide-terminal" | "ide/terminal" => SceneKind::IdeTerminal,
        "browser" => SceneKind::Browser,
        _ => SceneKind::Unknown,
    }
}

// ============================================================
// 规则加载（OnceLock 缓存）
// ============================================================

static RULES: OnceLock<CompiledRules> = OnceLock::new();

fn load_rules() -> CompiledRules {
    // 尝试从 exe 同级目录加载
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("scene-rules.toml");
            if let Ok(content) = std::fs::read_to_string(&path) {
                match toml::from_str::<Rules>(&content) {
                    Ok(r) => {
                        log::info!("Scene rules loaded from {:?}", path);
                        return CompiledRules::from_rules(r);
                    }
                    Err(e) => {
                        log::warn!(
                            "Scene rules parse error in {:?}, falling back to builtin: {}",
                            path,
                            e
                        );
                    }
                }
            }
        }
    }

    // 内置默认
    match toml::from_str::<Rules>(BUILTIN_RULES) {
        Ok(r) => CompiledRules::from_rules(r),
        Err(e) => {
            log::warn!("Scene builtin rules parse error: {}", e);
            // 降级：无规则，所有场景均为 Unknown
            CompiledRules { rules: Vec::new() }
        }
    }
}

fn rules() -> &'static CompiledRules {
    RULES.get_or_init(load_rules)
}

/// 从指定内容加载规则（测试用）
#[cfg(test)]
fn compile_rules_from_content(content: &str) -> CompiledRules {
    match toml::from_str::<Rules>(content) {
        Ok(r) => CompiledRules::from_rules(r),
        Err(_) => CompiledRules { rules: Vec::new() },
    }
}

// ============================================================
// 公共 API
// ============================================================

/// 根据采集到的 exe 名 + 窗口标题分类场景。
/// 返回的 SceneContext 不含 exe 名/标题在 style_hint 中（隐私边界）。
pub fn classify_scene(exe: &str, title: &str) -> SceneContext {
    rules().classify(exe, title)
}

/// 生成 F4 场景风格段（写入 LLM prompt）。
/// Unknown/空 style_hint → None（不注入）。
/// send_window_title=true 且有标题时，追加截断标题（上限 50 字符）以提升场景判断。
pub fn build_scene_prompt_block(scene: &SceneContext, send_window_title: bool) -> Option<String> {
    if scene.is_unknown() || scene.style_hint.trim().is_empty() {
        return None;
    }

    let kind_label = scene.scene.as_str();
    let mut block = format!(
        "Scene Context (F4): The user is typing into a {} application. Adapt tone accordingly.\n{}",
        kind_label,
        scene.style_hint.trim()
    );

    if send_window_title && !scene.window_title.trim().is_empty() {
        let truncated: String = scene.window_title.trim().chars().take(50).collect();
        block.push_str(&format!("\n(Reference title context: \"{}\")", truncated));
    }

    Some(block)
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn classify_with(content: &str, exe: &str, title: &str) -> SceneContext {
        let r = compile_rules_from_content(content);
        r.classify(exe, title)
    }

    // 使用内置规则测试
    fn classify_builtin(exe: &str, title: &str) -> SceneContext {
        rules().classify(exe, title)
    }

    // ============================================================
    // 分类匹配
    // ============================================================

    #[test]
    fn classify_wechat_to_chat() {
        let scene = classify_builtin("WeChat.exe", "微信");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(!scene.multiline_safe, "chat must be multiline_safe=false");
        assert!(!scene.style_hint.is_empty());
    }

    #[test]
    fn classify_exe_case_insensitive() {
        // exe 匹配不区分大小写
        let scene = classify_builtin("wechat.exe", "");
        assert_eq!(scene.scene, SceneKind::Chat);
        let scene = classify_builtin("WECHAT.EXE", "");
        assert_eq!(scene.scene, SceneKind::Chat);
    }

    #[test]
    fn classify_outlook_to_email() {
        let scene = classify_builtin("OUTLOOK.EXE", "收件箱 - Outlook");
        assert_eq!(scene.scene, SceneKind::Email);
        assert!(scene.multiline_safe, "email must be multiline_safe=true");
    }

    #[test]
    fn classify_vscode_to_ide() {
        let scene = classify_builtin("Code.exe", "main.rs - VSCode");
        assert_eq!(scene.scene, SceneKind::IdeTerminal);
        assert!(
            !scene.multiline_safe,
            "IDE/terminal must be multiline_safe=false"
        );
    }

    #[test]
    fn classify_word_to_doc() {
        let scene = classify_builtin("WINWORD.EXE", "Document1 - Word");
        assert_eq!(scene.scene, SceneKind::Doc);
        assert!(scene.multiline_safe, "doc must be multiline_safe=true");
    }

    #[test]
    fn classify_chrome_to_browser() {
        let scene = classify_builtin("chrome.exe", "Google - Google Chrome");
        assert_eq!(scene.scene, SceneKind::Browser);
        assert!(
            !scene.multiline_safe,
            "browser must be multiline_safe=false (conservative)"
        );
    }

    #[test]
    fn classify_unknown_when_no_match() {
        let scene = classify_builtin("UnknownApp.exe", "");
        assert_eq!(scene.scene, SceneKind::Unknown);
        assert!(
            !scene.multiline_safe,
            "Unknown must be multiline_safe=false (conservative)"
        );
        assert!(scene.style_hint.is_empty());
        assert!(scene.is_unknown());
    }

    #[test]
    fn classify_title_keyword_fallback() {
        // 非浏览器 exe 未命中 → 标题关键词兜底（用 UnknownApp 测真实 fallback 路径）
        let scene = classify_builtin("UnknownApp.exe", "收件箱 - Outlook");
        assert_eq!(
            scene.scene,
            SceneKind::Email,
            "title keyword 'Outlook' should classify as email"
        );
        assert!(scene.multiline_safe);
    }

    #[test]
    fn classify_exe_match_takes_priority_over_title() {
        // WeChat.exe + 标题含"邮件" → 应匹配 exe（chat）而非 title（email）
        let scene = classify_builtin("WeChat.exe", "邮件通知 - Gmail");
        assert_eq!(
            scene.scene,
            SceneKind::Chat,
            "exe match must take priority over title"
        );
    }

    /// SCENE-SENSE-001-CORE: 浏览器细分——chrome.exe + 标题含"Gmail" → email（非 browser）
    #[test]
    fn classify_browser_subclass_to_email_via_title() {
        let scene = classify_builtin("chrome.exe", "收件箱 - Gmail - Google Chrome");
        assert_eq!(
            scene.scene,
            SceneKind::Email,
            "browser exe + email title → email"
        );
        assert!(scene.multiline_safe, "email must be multiline_safe=true");
    }

    #[test]
    fn classify_browser_subclass_to_doc_via_title() {
        let scene = classify_builtin("chrome.exe", "文档1 - Google Docs - Google Chrome");
        assert_eq!(scene.scene, SceneKind::Doc, "browser exe + doc title → doc");
        assert!(scene.multiline_safe, "doc must be multiline_safe=true");
    }

    #[test]
    fn classify_browser_no_title_match_stays_browser() {
        // chrome.exe + 普通搜索标题 → Browser
        let scene = classify_builtin("chrome.exe", "Rust documentation - Google Search");
        assert_eq!(scene.scene, SceneKind::Browser);
        assert!(
            !scene.multiline_safe,
            "browser must be multiline_safe=false"
        );
    }

    // ============================================================
    // SCENE-TITLE-CASE-001: 标题关键词大小写不敏感匹配
    // ============================================================

    #[test]
    fn scene_title_case_001_inbox_title_case_insensitive() {
        // 三形态标题均命中 email（词表写 "Inbox"，标题可为 inbox/INBOX/Inbox）
        assert_eq!(
            classify_builtin("UnknownApp.exe", "inbox - Outlook").scene,
            SceneKind::Email,
            "lowercase 'inbox' should match keyword 'Inbox'"
        );
        assert_eq!(
            classify_builtin("UnknownApp.exe", "INBOX - Outlook").scene,
            SceneKind::Email,
            "uppercase 'INBOX' should match keyword 'Inbox'"
        );
        assert_eq!(
            classify_builtin("UnknownApp.exe", "Inbox - Outlook").scene,
            SceneKind::Email,
            "mixed-case 'Inbox' should match keyword 'Inbox'"
        );
    }

    #[test]
    fn scene_title_case_001_gmail_title_case_insensitive() {
        // GMAIL 全大写也应命中 email
        assert_eq!(
            classify_builtin("UnknownApp.exe", "收件箱 - GMAIL - Google").scene,
            SceneKind::Email,
            "uppercase 'GMAIL' should match keyword 'Gmail'"
        );
    }

    #[test]
    fn scene_title_case_001_chinese_keyword_unaffected() {
        // 中文关键词无大小写问题，不受影响（回归验证）
        assert_eq!(
            classify_builtin("UnknownApp.exe", "收件箱 - 邮箱").scene,
            SceneKind::Email,
            "Chinese keywords should still match (no case issue)"
        );
    }

    #[test]
    fn scene_title_case_001_browser_subclass_case_insensitive() {
        // 浏览器细分路径同样大小写不敏感：
        // chrome.exe + "GMAIL - inbox" → email（浏览器细分路径）
        let scene = classify_builtin("chrome.exe", "GMAIL - inbox - Google Chrome");
        assert_eq!(
            scene.scene,
            SceneKind::Email,
            "browser subclass path must be case-insensitive (GMAIL/inbox → email)"
        );
        assert!(scene.multiline_safe, "email must be multiline_safe=true");
    }

    #[test]
    fn scene_title_case_001_browser_subclass_docs_case_insensitive() {
        // chrome.exe + "google docs" 小写 → doc（浏览器细分路径大小写不敏感）
        let scene = classify_builtin("chrome.exe", "document - google docs - Chrome");
        assert_eq!(
            scene.scene,
            SceneKind::Doc,
            "browser subclass path must be case-insensitive (google docs → doc)"
        );
    }

    // ============================================================
    // F4 段生成 + 隐私断言
    // ============================================================

    #[test]
    fn build_scene_prompt_block_unknown_returns_none() {
        let scene = SceneContext::unknown();
        assert!(build_scene_prompt_block(&scene, false).is_none());
        assert!(build_scene_prompt_block(&scene, true).is_none());
    }

    #[test]
    fn build_scene_prompt_block_chat_contains_style_not_exe() {
        let scene = classify_builtin("WeChat.exe", "微信");
        let block = build_scene_prompt_block(&scene, false).expect("chat should produce F4 block");
        assert!(block.contains("chat"));
        assert!(block.contains("Scene Context (F4)"));
        // 隐私断言：默认不含 exe 名与标题
        assert!(
            !block.contains("WeChat.exe"),
            "exe name must not leak into prompt"
        );
        assert!(
            !block.contains("微信"),
            "window title must not leak by default"
        );
    }

    #[test]
    fn build_scene_prompt_block_send_title_includes_truncated_title() {
        let scene = classify_builtin("WeChat.exe", "微信");
        let block =
            build_scene_prompt_block(&scene, true).expect("should produce block with title");
        assert!(block.contains("Reference title context"));
        assert!(
            block.contains("微信"),
            "send_window_title=true should include title"
        );
    }

    #[test]
    fn build_scene_prompt_block_title_truncated_to_50_chars() {
        let long_title = "这是一段非常长的窗口标题用于测试截断功能是否在50个字符处停止".repeat(2);
        let scene = SceneContext {
            scene: SceneKind::Chat,
            app_exe: "WeChat.exe".to_string(),
            window_title: long_title.clone(),
            multiline_safe: false,
            style_hint: "test".to_string(),
        };
        let block = build_scene_prompt_block(&scene, true).expect("should produce block");
        // 找到截断标题部分，验证不超过 50 字符
        let start = block.find("\"").map(|i| i + 1).unwrap_or(0);
        let end = block.rfind("\"").unwrap_or(block.len());
        let title_in_block: String = block[start..end].to_string();
        assert!(
            title_in_block.chars().count() <= 50,
            "title must be truncated to <=50 chars, got {}",
            title_in_block.chars().count()
        );
    }

    // ============================================================
    // toml 解析失败降级
    // ============================================================

    #[test]
    fn fallback_empty_rules_all_unknown() {
        let scene = classify_with("", "WeChat.exe", "微信");
        assert_eq!(scene.scene, SceneKind::Unknown);
        assert!(!scene.multiline_safe);
    }

    #[test]
    fn fallback_invalid_toml_all_unknown() {
        let bad = "this is not valid toml {{{{";
        let scene = classify_with(bad, "WeChat.exe", "微信");
        assert_eq!(scene.scene, SceneKind::Unknown);
    }

    #[test]
    fn custom_rules_classify_correctly() {
        let custom = r#"
[[scene]]
kind = "chat"
style = "Casual tone."
multiline_safe = false
exe = ["MyApp.exe"]
"#;
        let scene = classify_with(custom, "MyApp.exe", "");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert_eq!(scene.style_hint, "Casual tone.");
        assert!(!scene.multiline_safe);
    }

    #[test]
    fn unknown_kind_string_maps_to_unknown() {
        let custom = r#"
[[scene]]
kind = "nonexistent_kind"
style = "test"
multiline_safe = true
exe = ["X.exe"]
"#;
        let scene = classify_with(custom, "X.exe", "");
        assert_eq!(scene.scene, SceneKind::Unknown);
    }

    // ============================================================
    // SceneContext::unknown + is_unknown
    // ============================================================

    #[test]
    fn unknown_scene_context_defaults() {
        let s = SceneContext::unknown();
        assert!(s.is_unknown());
        assert!(!s.multiline_safe);
        assert!(s.app_exe.is_empty());
        assert!(s.window_title.is_empty());
        assert!(s.style_hint.is_empty());
    }

    #[test]
    fn scene_kind_as_str() {
        assert_eq!(SceneKind::Chat.as_str(), "chat");
        assert_eq!(SceneKind::Email.as_str(), "email");
        assert_eq!(SceneKind::Doc.as_str(), "document");
        assert_eq!(SceneKind::IdeTerminal.as_str(), "IDE/terminal");
        assert_eq!(SceneKind::Browser.as_str(), "browser");
        assert_eq!(SceneKind::Unknown.as_str(), "unknown");
    }

    // ============================================================
    // TEST-SYNC-SCENE-001 补缺
    // ============================================================

    /// SCENE-BROWSER-EDGE-001: Browser exe + 空标题 → Browser 不细分
    #[test]
    fn classify_browser_empty_title_stays_browser() {
        let scene = classify_builtin("chrome.exe", "");
        assert_eq!(scene.scene, SceneKind::Browser);
        assert!(!scene.multiline_safe);
    }

    /// SCENE-BROWSER-EDGE-002: Browser exe + 命中多个非 browser title → 取第一命中
    #[test]
    fn classify_browser_multi_title_first_match_wins() {
        let custom = r#"
[[scene]]
kind = "email"
style = "Formal tone."
multiline_safe = true
title_keywords = ["邮件", "收件箱"]

[[scene]]
kind = "doc"
style = "Professional tone."
multiline_safe = true
title_keywords = ["文档"]

[[scene]]
kind = "browser"
style = "Neutral tone."
multiline_safe = false
exe = ["browser.exe"]
"#;
        // browser.exe + 标题同时匹配 email 和 doc → 取 email（第一个定义在 rules 中）
        let scene = classify_with(custom, "browser.exe", "文档-收件箱");
        assert_eq!(
            scene.scene,
            SceneKind::Email,
            "first-matching non-browser rule must win"
        );
        assert!(scene.multiline_safe);
    }

    /// SCENE-BUILD-BLOCK-EDGE-001: style_hint 纯空白（trim 后空）→ None
    #[test]
    fn build_scene_block_whitespace_style_hint_returns_none() {
        let scene = SceneContext {
            scene: SceneKind::Chat,
            app_exe: "App.exe".to_string(),
            window_title: "".to_string(),
            multiline_safe: false,
            style_hint: "   ".to_string(),
        };
        assert!(build_scene_prompt_block(&scene, false).is_none());
        assert!(build_scene_prompt_block(&scene, true).is_none());
    }

    /// SCENE-BUILD-BLOCK-EDGE-002: send_window_title=true 但标题为空 → 不追加标题行
    #[test]
    fn build_scene_block_empty_title_no_title_line() {
        let scene = SceneContext {
            scene: SceneKind::Chat,
            app_exe: "App.exe".to_string(),
            window_title: "".to_string(),
            multiline_safe: false,
            style_hint: "Casual.".to_string(),
        };
        let block = build_scene_prompt_block(&scene, true).expect("style_hint non-empty -> Some");
        assert!(!block.contains("Reference title context"));
        assert!(!block.contains("App.exe"));
    }

    /// SCENE-BUILD-BLOCK-EDGE-003: send_window_title=true 但标题纯空白 → 不追加
    #[test]
    fn build_scene_block_whitespace_title_no_title_line() {
        let scene = SceneContext {
            scene: SceneKind::Chat,
            app_exe: "App.exe".to_string(),
            window_title: "   ".to_string(),
            multiline_safe: false,
            style_hint: "Casual.".to_string(),
        };
        let block = build_scene_prompt_block(&scene, true).expect("style_hint non-empty -> Some");
        assert!(!block.contains("Reference title context"));
    }

    // ============================================================
    // TEST-SYNC-SCENE-002: AI Agent 词表分类
    // ============================================================

    /// SCENE-AI-AGENT-001: AI Agent exe 精确命中 Claude.exe → chat
    #[test]
    fn ai_agent_exe_claude_classify_chat() {
        let scene = classify_builtin("Claude.exe", "Claude AI");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(
            !scene.multiline_safe,
            "AI agent must be multiline_safe=false"
        );
        assert!(!scene.style_hint.is_empty());
    }

    /// SCENE-AI-AGENT-002: AI Agent exe 精确命中 ChatGPT.exe → chat
    #[test]
    fn ai_agent_exe_chatgpt_classify_chat() {
        let scene = classify_builtin("ChatGPT.exe", "ChatGPT");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(!scene.multiline_safe);
        assert!(!scene.style_hint.is_empty());
    }

    /// SCENE-AI-AGENT-003: AI Agent exe 精确命中 Codex.exe → chat
    #[test]
    fn ai_agent_exe_codex_classify_chat() {
        let scene = classify_builtin("Codex.exe", "Codex");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(!scene.multiline_safe);
        assert!(!scene.style_hint.is_empty());
    }

    /// SCENE-AI-AGENT-004: AI Agent exe 精确命中 yuanbao.exe → chat
    #[test]
    fn ai_agent_exe_yuanbao_classify_chat() {
        let scene = classify_builtin("yuanbao.exe", "元宝");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(!scene.multiline_safe);
        assert!(!scene.style_hint.is_empty());
    }

    /// SCENE-AI-AGENT-005: title_keywords 兜底——UnknownApp + 标题含 ChatGPT → chat
    #[test]
    fn ai_agent_title_chatgpt_fallback() {
        let scene = classify_builtin("UnknownApp.exe", "ChatGPT 对话");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(!scene.multiline_safe);
        assert!(!scene.style_hint.is_empty());
    }

    /// SCENE-AI-AGENT-006: title_keywords 兜底——UnknownApp + 标题含 Claude → chat
    #[test]
    fn ai_agent_title_claude_fallback() {
        let scene = classify_builtin("UnknownApp.exe", "Claude 3.5 Sonnet");
        assert_eq!(scene.scene, SceneKind::Chat);
    }

    /// SCENE-AI-AGENT-007: title_keywords 兜底——浏览器细分 chrome + 标题含 元宝 → chat
    #[test]
    fn ai_agent_browser_subclass_yuanbao() {
        let scene = classify_builtin("chrome.exe", "元宝 - AI助手 - Google Chrome");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(!scene.multiline_safe);
    }

    /// SCENE-AI-AGENT-008: title_keywords 兜底——浏览器细分 msedge + 标题含 DeepSeek → chat
    #[test]
    fn ai_agent_browser_subclass_deepseek() {
        let scene = classify_builtin("msedge.exe", "DeepSeek 对话 - Edge");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(!scene.multiline_safe);
    }

    /// SCENE-AI-AGENT-009: ide_terminal 补条——conhost.exe → ide_terminal
    #[test]
    fn ai_agent_conhost_to_ide_terminal() {
        let scene = classify_builtin("conhost.exe", "C:\\Windows\\System32");
        assert_eq!(scene.scene, SceneKind::IdeTerminal);
        assert!(!scene.multiline_safe);
    }

    /// SCENE-AI-AGENT-010: ide_terminal 补条——OpenConsole.exe → ide_terminal
    #[test]
    fn ai_agent_openconsole_to_ide_terminal() {
        let scene = classify_builtin("OpenConsole.exe", "PowerShell 7");
        assert_eq!(scene.scene, SceneKind::IdeTerminal);
        assert!(!scene.multiline_safe);
    }

    /// SCENE-AI-AGENT-011: 大小写不敏感——小写 claude.exe → chat（回归 SCENE-TITLE-CASE-001）
    #[test]
    fn ai_agent_exe_case_insensitive() {
        let scene = classify_builtin("claude.exe", "");
        assert_eq!(scene.scene, SceneKind::Chat);
    }

    /// SCENE-AI-AGENT-012: 大小写不敏感——标题小写 chatgpt 兜底命中（回归 SCENE-TITLE-CASE-001）
    #[test]
    fn ai_agent_title_case_insensitive() {
        let scene = classify_builtin("UnknownApp.exe", "chatgpt conversation");
        assert_eq!(scene.scene, SceneKind::Chat);
    }

    /// SCENE-AI-AGENT-013: 大小写不敏感——浏览器细分小写 claude 标题命中（回归 SCENE-TITLE-CASE-001）
    #[test]
    fn ai_agent_browser_subclass_case_insensitive() {
        let scene = classify_builtin("chrome.exe", "claude - Google Chrome");
        assert_eq!(scene.scene, SceneKind::Chat);
    }

    /// SCENE-AI-AGENT-014: 大小写不敏感——CONHOST.EXE 大写 → ide_terminal（回归 SCENE-TITLE-CASE-001）
    #[test]
    fn ai_agent_conhost_uppercase() {
        let scene = classify_builtin("CONHOST.EXE", "");
        assert_eq!(scene.scene, SceneKind::IdeTerminal);
    }
}
