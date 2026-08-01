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
                // 浏览器细分：exe 命中 Browser 后，检查其他场景 title_keywords 覆盖。
                // FIX-SCENE-TITLE-LONGEST-001：改确定性最长匹配（见 find_longest_title_rule
                // 注释，与 ITN-V2-ENGINE-002 filter+max() 同源）。
                if rule.kind == SceneKind::Browser && !title.is_empty() {
                    if let Some(other_rule) = self.find_longest_title_rule(&title_lower, true) {
                        return SceneContext {
                            scene: other_rule.kind,
                            app_exe: exe.to_string(),
                            window_title: title.to_string(),
                            multiline_safe: other_rule.multiline_safe,
                            style_hint: other_rule.style.clone(),
                        };
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
        // FIX-SCENE-TITLE-LONGEST-001：改确定性最长匹配。此路径**不排除 Browser**——
        // exe 未命中任何规则时，browser 自身的 title_keywords 是合法候选（与改动 1
        // 浏览器细分「跳过自身」的语义差异见 find_longest_title_rule 注释）。
        if !title.is_empty() {
            if let Some(matched) = self.find_longest_title_rule(&title_lower, false) {
                return SceneContext {
                    scene: matched.kind,
                    app_exe: exe.to_string(),
                    window_title: title.to_string(),
                    multiline_safe: matched.multiline_safe,
                    style_hint: matched.style.clone(),
                };
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

    /// 在全部 rule 的 title_keywords 中做**确定性最长匹配**，返回命中且字符数最长的那条所属 rule。
    ///
    /// FIX-SCENE-TITLE-LONGEST-001：此前的遍历是「按 toml 块顺序首匹配即返回」，短词
    /// （chat 块的 `钉钉`/`飞书`）排在长词（doc 块的 `钉钉文档`/`飞书文档`）之前时，
    /// 长词永远轮不到，导致 `chrome.exe + 钉钉文档 - 协作` 被误判为 chat。
    ///
    /// 处置方式与 ITN-V2-ENGINE-002 / [ITN-PREFIX-SHADOW-001] 同源：`src/itn.rs`
    /// `check_protection` 对 `十一月`⊂`十一` 这类前缀遮蔽也是改为确定性最长匹配
    /// （`filter` + `max()`），规则性碰撞用规则解决，不用词表补丁（DEC-038）。
    ///
    /// - `exclude_browser=true`：跳过 Browser 块 —— SCENE-SENSE-001 浏览器细分语义
    ///   「浏览器不参与自身细分」（改动 1 用）。
    /// - `exclude_browser=false`：不排除 Browser —— exe 未命中任何规则时 browser 自身
    ///   的 title_keywords 是合法候选（改动 2 / 优先级 2 兜底用）。
    ///
    /// 比较用 `kw.chars().count()`（字符数）而非字节数，中文关键词必须正确计长。
    /// `title_lower` 只计算一次由调用方传入（SCENE-TITLE-CASE-001 既有优化，不退化）。
    ///
    /// 平局打破规则（主控 2026-08-01 裁定，方案 A）：browser 是**通用保守兜底场景**，
    /// 它的 title_keywords 多与 email/doc 等具体场景共享（见 browser 块注释「邮箱场景
    /// 兜底」）。两处候选同长时若仍按「取迭代靠后的块」，browser 块排在 toml 末尾必然
    /// 胜出，`UnknownApp + 收件箱 - Outlook` 会被误判为 browser。故：**同长时具体场景
    /// 优先于 browser，browser 仅当其关键词严格更长才胜出**。与 DEC-038「规则性问题用
    /// 规则解决」一致，且可推广到未来任何「通用兜底块」。
    ///
    /// 平局确定性：比较键为 `(len, 非browser=1/browser=0)`，`>` 严格序、无 `>=` 覆盖，
    /// 迭代顺序稳定 → 结果确定可预期，不随机漂移。
    fn find_longest_title_rule(
        &self,
        title_lower: &str,
        exclude_browser: bool,
    ) -> Option<&CompiledRule> {
        let mut best: Option<(&CompiledRule, usize, bool)> = None;
        for rule in &self.rules {
            if exclude_browser && rule.kind == SceneKind::Browser {
                continue;
            }
            for kw in &rule.title_keywords {
                if title_lower.contains(kw.as_str()) {
                    let len = kw.chars().count();
                    let is_browser = rule.kind == SceneKind::Browser;
                    // 比较键 (len, !is_browser)：len 主序，平局时非 browser 优先
                    let better = match best {
                        None => true,
                        Some((_, best_len, best_is_browser)) => {
                            len > best_len || (len == best_len && !is_browser && best_is_browser)
                        }
                    };
                    if better {
                        best = Some((rule, len, is_browser));
                    }
                }
            }
        }
        best.map(|(rule, _, _)| rule)
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
        // TEST-SYNC-SCENE-MD-003：Gavin 2026-08-01 裁定代码/文字编辑生成类软件
        // （VS Code/Cursor/JetBrains 全家等）放开多行 → ide_terminal(true) 块。
        // 旧断言「IDE/terminal must be multiline_safe=false」随设计变更作废。
        let scene = classify_builtin("Code.exe", "main.rs - VSCode");
        assert_eq!(scene.scene, SceneKind::IdeTerminal);
        assert!(
            scene.multiline_safe,
            "IDE/editor must be multiline_safe=true"
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

    // ============================================================
    // TEST-SYNC-SCENE-COVERAGE-001: scene-rules.toml 解析 + 新增条目 + 浏览器细分回归
    // ============================================================

    /// P0-1: BUILTIN_RULES 解析必须成功（直接 toml::from_str，不用 compile_rules_from_content）
    #[test]
    fn builtin_rules_parse_ok() {
        let parsed = toml::from_str::<Rules>(BUILTIN_RULES);
        assert!(
            parsed.is_ok(),
            "scene-rules.toml 解析失败，场景感知会静默降级为全 Unknown: {:?}",
            parsed.err()
        );
        let rules = parsed.unwrap();
        assert_eq!(rules.scene.len(), 9, "[[scene]] 块数应为 9");
        let total: usize = rules.scene.iter().map(|s| s.exe.len()).sum();
        assert!(
            total >= 160,
            "exe 条目总数异常偏少（当前应为 165），疑似数组被截断: {}",
            total
        );
    }

    /// P0-2: 特殊字符条目——The Bat!.exe（含 !）→ Email
    #[test]
    fn special_char_bang_exe_classify_email() {
        let rules = compile_rules_from_content(BUILTIN_RULES);
        let scene = rules.classify("The Bat!.exe", "");
        assert_eq!(scene.scene, SceneKind::Email);
        assert!(scene.multiline_safe);
    }

    /// P0-2: 特殊字符条目——Koodo Reader.exe（含空格）→ Doc
    #[test]
    fn special_char_space_exe_classify_doc() {
        let rules = compile_rules_from_content(BUILTIN_RULES);
        let scene = rules.classify("Koodo Reader.exe", "");
        assert_eq!(scene.scene, SceneKind::Doc);
        assert!(scene.multiline_safe);
    }

    /// P0-3: doc 块 title_keywords 浏览器细分——chrome + Jira 标题 → Doc
    #[test]
    fn browser_subclass_jira_to_doc() {
        let scene = classify_builtin("chrome.exe", "PROJ-123 · Jira");
        assert_eq!(scene.scene, SceneKind::Doc);
        assert!(scene.multiline_safe);
    }

    /// P0-3: chrome + TAPD 标题 → Doc
    #[test]
    fn browser_subclass_tapd_to_doc() {
        let scene = classify_builtin("chrome.exe", "TAPD - 迭代需求");
        assert_eq!(scene.scene, SceneKind::Doc);
        assert!(scene.multiline_safe);
    }

    /// P0-3: chrome + 禅道 标题 → Doc
    #[test]
    fn browser_subclass_zentao_to_doc() {
        let scene = classify_builtin("chrome.exe", "禅道 · 任务 #456");
        assert_eq!(scene.scene, SceneKind::Doc);
        assert!(scene.multiline_safe);
    }

    /// P0-3: chrome + Teambition 标题 → Doc
    #[test]
    fn browser_subclass_teambition_to_doc() {
        let scene = classify_builtin("chrome.exe", "Teambition - 项目文档");
        assert_eq!(scene.scene, SceneKind::Doc);
        assert!(scene.multiline_safe);
    }

    /// P0-4: 反向护栏——browser 自身的 title_keywords 不参与浏览器细分
    /// 已知：browser 块的 title_keywords 与 email/doc 块完全重叠，无唯一词。
    /// 改用构造内联 fixture：browser 块含独有关键词 X，chrome + X 仍为 Browser。
    #[test]
    fn browser_own_title_keyword_does_not_subclass() {
        let custom = r#"
[[scene]]
kind = "browser"
style = "Web tone."
multiline_safe = false
exe = ["browser.exe"]
title_keywords = ["UniqueBrowserOnlyKeyword"]

[[scene]]
kind = "doc"
style = "Doc tone."
multiline_safe = true
title_keywords = ["doc_keyword"]
"#;
        // browser.exe + 标题含 browser 独有的关键词 → 仍为 Browser（非 doc）
        let rules = compile_rules_from_content(custom);
        let scene = rules.classify("browser.exe", "UniqueBrowserOnlyKeyword page");
        assert_eq!(
            scene.scene,
            SceneKind::Browser,
            "browser own title_keywords must not trigger reclassification"
        );
        assert!(!scene.multiline_safe);
    }

    /// P0-5: Figma.exe → Browser（Gavin 拍板归 browser，非 ide_terminal）
    #[test]
    fn figma_classify_browser() {
        let scene = classify_builtin("Figma.exe", "");
        assert_eq!(scene.scene, SceneKind::Browser);
        assert!(!scene.multiline_safe);
    }

    /// P0-5: ChatGLM.exe → Chat（新旧名并存）
    #[test]
    fn chatglm_classify_chat() {
        let scene = classify_builtin("ChatGLM.exe", "");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(!scene.multiline_safe);
    }

    /// P0-5: GLM.exe → Chat（新旧名并存）
    #[test]
    fn glm_classify_chat() {
        let scene = classify_builtin("GLM.exe", "");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(!scene.multiline_safe);
    }

    /// P0-5: Zoom.exe → Chat 且 multiline_safe=false（视频会议 Enter=发送）
    #[test]
    fn zoom_classify_chat() {
        let scene = classify_builtin("Zoom.exe", "");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(!scene.multiline_safe);
    }

    /// P0-5: wemeetapp.exe → Chat 且 multiline_safe=false（视频会议 Enter=发送）
    #[test]
    fn wemeetapp_classify_chat() {
        let scene = classify_builtin("wemeetapp.exe", "");
        assert_eq!(scene.scene, SceneKind::Chat);
        assert!(!scene.multiline_safe);
    }

    /// P1: 大小写不敏感——OneNote.exe 与 ONENOTE.EXE 均命中 Doc
    #[test]
    fn case_insensitive_onenote_doc() {
        let scene_lower = classify_builtin("OneNote.exe", "");
        let scene_upper = classify_builtin("ONENOTE.EXE", "");
        assert_eq!(scene_lower.scene, SceneKind::Doc);
        assert_eq!(scene_upper.scene, SceneKind::Doc);
        assert_eq!(scene_lower.scene, scene_upper.scene);
        assert!(scene_lower.multiline_safe);
    }

    // ============================================================
    // SCENE-MD-003（IMPL-SCENE-MULTILINE-002）：ide_terminal 双块全覆盖 + 互斥验证
    // 原名 temp_*（曾被视为临时），主控裁定这批是永久保留：28 条逐条实测除写测试
    // 外无他法，任务书「零 Rust 改动」与「28 条实测」规格自相矛盾，责任在主控。
    // 已改为正式命名 scene_md003_* / SCENE_MD003_*。
    // ============================================================

    /// ide_terminal(multiline_safe=true) 块：代码/文字编辑生成类软件（Gavin 2026-08-01 裁定放开多行）
    const SCENE_MD003_TRUE_EXES: &[&str] = &[
        "notepad++.exe",
        "sublime_text.exe",
        "SublimeText.exe",
        "Code.exe",
        "Code - Insiders.exe",
        "cursor.exe",
        "Windsurf.exe",
        "Zed.exe",
        "HBuilderX.exe",
        "devenv.exe",
        "idea64.exe",
        "idea.exe",
        "pycharm64.exe",
        "pycharm.exe",
        "webstorm64.exe",
        "webstorm.exe",
        "goland64.exe",
        "goland.exe",
        "rustrover64.exe",
        "rustrover.exe",
        "clion64.exe",
        "clion.exe",
        "phpstorm64.exe",
        "phpstorm.exe",
        "rubymine64.exe",
        "rubymine.exe",
        "rider64.exe",
        "rider.exe",
        "sourceinsight4.exe",
        "Insight4.exe",
        "Insight3.exe",
        "SourceInsight.exe",
    ];

    /// ide_terminal(multiline_safe=false) 块：纯终端 + 模态编辑器（vim/gvim）
    /// ⚠️ 28 条全量：Xshell6/Xshell7/Xagent/MobaXterm1 必须与 scene-rules.toml 逐条对齐
    const SCENE_MD003_FALSE_EXES: &[&str] = &[
        "WindowsTerminal.exe",
        "cmd.exe",
        "powershell.exe",
        "pwsh.exe",
        "ConEmu64.exe",
        "ConEmu.exe",
        "vim.exe",
        "gvim.exe",
        "Cmder.exe",
        "Alacritty.exe",
        "wezterm-gui.exe",
        "mintty.exe",
        "putty.exe",
        "Xshell.exe",
        "Xshell6.exe",
        "Xshell7.exe",
        "Xshell8.exe",
        "Xagent.exe",
        "SecureCRT.exe",
        "MobaXterm.exe",
        "MobaXterm1.exe",
        "Tabby.exe",
        "Hyper.exe",
        "conhost.exe",
        "OpenConsole.exe",
        "wezterm.exe",
        "git-bash.exe",
        "Nu.exe",
    ];

    /// doc 块新增条目（Markdown 笔记 / Todo / 便签，multiline_safe=true）
    /// 含 Linear.exe（Gavin 2026-08-01 改判：项目/任务管理工具非聊天工具，从 chat 移入 doc）
    const SCENE_MD003_DOC_TRUE_EXES: &[&str] = &[
        "Notepad.exe",
        "siyuan.exe",
        "wps.exe",
        "Obsidian.exe",
        "Zettlr.exe",
        "vnote.exe",
        "trilium.exe",
        "Standard Notes.exe",
        "Boostnote.exe",
        "Inkdrop.exe",
        "YoudaoNote.exe",
        "WizNote.exe",
        "wiz.exe",
        "Todoist.exe",
        "TickTick.exe",
        "TodoApp.exe",
        "ClickUp.exe",
        "Any.do.exe",
        "Focalboard.exe",
        "Linear.exe",
        "SimpleStickyNotes.exe",
        "Simple Sticky Notes.exe",
        "stickies.exe",
        "notezilla.exe",
        "PNotes.exe",
        "7StickyNotes.exe",
        "jingyeqian.exe",
        "StickyNotes.exe",
        "StickyNotesStub.exe",
        "Microsoft.Notes.exe",
    ];

    #[test]
    fn scene_md003_true_block_all() {
        for exe in SCENE_MD003_TRUE_EXES {
            let s = classify_builtin(exe, "");
            assert_eq!(s.scene, SceneKind::IdeTerminal, "{exe} 应 ide_terminal");
            assert!(s.multiline_safe, "{exe} 应 multiline_safe=true（新块生效）");
        }
    }

    #[test]
    fn scene_md003_false_block_all() {
        for exe in SCENE_MD003_FALSE_EXES {
            let s = classify_builtin(exe, "");
            assert_eq!(s.scene, SceneKind::IdeTerminal, "{exe} 应 ide_terminal");
            assert!(!s.multiline_safe, "{exe} 应 multiline_safe=false");
        }
    }

    #[test]
    fn scene_md003_doc_untouched() {
        for exe in SCENE_MD003_DOC_TRUE_EXES {
            let s = classify_builtin(exe, "");
            assert_eq!(s.scene, SceneKind::Doc, "{exe} 应 doc");
            assert!(s.multiline_safe, "{exe} doc 应 true");
        }
    }

    /// TEST-EXEC-SCENE-MD-003 Step 0：Linear.exe 改判 doc（Gavin 2026-08-01）。
    /// Linear 是项目/任务管理工具非聊天工具，原归 chat(false) 属分类错误，
    /// 现与 Todoist/TickTick/ClickUp/Any.do 同列 doc 块 Todo 段。
    #[test]
    fn scene_md003_linear_reclassified_to_doc() {
        let s = classify_builtin("Linear.exe", "");
        assert_eq!(s.scene, SceneKind::Doc, "Linear.exe 应重分类为 doc");
        assert!(s.multiline_safe, "Linear.exe doc 应 true");
        // 反向护栏：不得回归 chat
        assert_ne!(s.scene, SceneKind::Chat, "Linear.exe 不得再归 chat");
    }

    /// ⭐ 成败关键：两个 ide_terminal 块的 exe 集合必须互不相交。
    /// classify（src/scene/mod.rs:157）exe 首个匹配即返回 —— 任一 exe 同时出现在
    /// 两块中，TRUE 块就永远轮不到。主控本次用 `comm` 手工验证为空集，但必须变成
    /// 测试，否则将来有人往 FALSE 块补一条就会静默失效。
    #[test]
    fn scene_md003_ide_terminal_blocks_disjoint() {
        let r = toml::from_str::<Rules>(BUILTIN_RULES).unwrap();
        let mut true_exes = std::collections::HashSet::new();
        let mut false_exes = std::collections::HashSet::new();
        for s in &r.scene {
            if s.kind != "ide_terminal" {
                continue;
            }
            let set = if s.multiline_safe {
                &mut true_exes
            } else {
                &mut false_exes
            };
            for e in &s.exe {
                set.insert(e.to_lowercase());
            }
        }
        assert!(!true_exes.is_empty(), "multiline_safe=true 块不应为空");
        assert!(!false_exes.is_empty(), "multiline_safe=false 块不应为空");
        let overlap: Vec<&String> = true_exes.intersection(&false_exes).collect();
        assert!(
            overlap.is_empty(),
            "ide_terminal 两个块 exe 集合不得相交: {:?}（相交 = TRUE 块永不命中）",
            overlap
        );
    }

    /// TEST-SYNC-SCENE-MD-003 C3：关键反向护栏（不得回归）
    /// 纯终端/模态编辑器（vim/gvim）仍 false（FALSE_EXES 已逐条覆盖，此处补显式
    /// 断言语义）；chat 应用仍 false；Figma 仍 browser。
    #[test]
    fn scene_md003_c3_reverse_guards() {
        // 模态编辑器：注入字符在 normal 模式被当命令键执行 → 不放开
        for exe in ["vim.exe", "gvim.exe"] {
            let s = classify_builtin(exe, "");
            assert_eq!(s.scene, SceneKind::IdeTerminal, "{exe} 应 ide_terminal");
            assert!(!s.multiline_safe, "{exe} 模态编辑器必须 false");
        }
        // 纯终端代表（其余 24 条由 SCENE_MD003_FALSE_EXES 覆盖）
        for exe in [
            "cmd.exe",
            "powershell.exe",
            "WindowsTerminal.exe",
            "putty.exe",
        ] {
            let s = classify_builtin(exe, "");
            assert_eq!(s.scene, SceneKind::IdeTerminal, "{exe} 应 ide_terminal");
            assert!(!s.multiline_safe, "{exe} 纯终端必须 false");
        }
        // chat 应用（Gavin 2026-07-28 决策 3 延续）：微信/QQ/钉钉/飞书 → false
        for exe in ["WeChat.exe", "QQ.exe", "DingTalk.exe", "Feishu.exe"] {
            let s = classify_builtin(exe, "");
            assert_eq!(s.scene, SceneKind::Chat, "{exe} 应 chat");
            assert!(!s.multiline_safe, "{exe} chat 必须 false");
        }
        // Figma：Gavin 2026-07-28 决策 3 归 browser（本批未改）
        let figma = classify_builtin("Figma.exe", "");
        assert_eq!(figma.scene, SceneKind::Browser, "Figma 应 browser");
        assert!(!figma.multiline_safe);
    }

    #[test]
    fn scene_md003_chrome_title_subclass() {
        // TEST-SYNC-SCENE-MD-003 C1：doc 块 IMPL 新增 title_keywords 全量覆盖（23 条逐一入测）。
        // 浏览器细分靠 doc 块 title_keywords（src/scene/mod.rs:160-177 浏览器 exe 命中后
        // 查其他场景关键词）—— 漏测一条 = 那个词的网页版静默不重分类。
        for (title, expect_kind) in [
            ("Google Keep - 我的笔记", SceneKind::Doc),
            ("金山文档 - 我的表格", SceneKind::Doc),
            ("Confluence - 团队空间", SceneKind::Doc),
            ("HackMD - 协作笔记", SceneKind::Doc),
            ("StackEdit - Markdown", SceneKind::Doc),
            ("Dillinger - editor", SceneKind::Doc),
            ("Trilium - my notes", SceneKind::Doc),
            ("Standard Notes - web", SceneKind::Doc),
            ("SiYuan - 思源笔记", SceneKind::Doc),
            ("思源笔记 - 我的文档", SceneKind::Doc),
            ("钉钉文档 - 协作", SceneKind::Doc),
            ("Obsidian Publish - 我的站点", SceneKind::Doc),
            ("Roam Research - 我的图谱", SceneKind::Doc),
            ("Anytype - 空间", SceneKind::Doc),
            ("Todoist - 任务清单", SceneKind::Doc),
            ("TickTick - Tasks", SceneKind::Doc),
            ("滴答清单 - 我的待办", SceneKind::Doc),
            ("Trello - 看板", SceneKind::Doc),
            ("Asana - projects", SceneKind::Doc),
            ("ClickUp - tasks", SceneKind::Doc),
            ("Google Tasks - 我的待办", SceneKind::Doc),
            ("Microsoft To Do - 待办", SceneKind::Doc),
            ("Any.do - todos", SceneKind::Doc),
        ] {
            let s = classify_builtin("chrome.exe", title);
            assert_eq!(s.scene, expect_kind, "title={title}");
            assert!(s.multiline_safe, "title={title} 重分类为 doc 应 true");
        }
        // 反向护栏：chrome + 普通标题 → 仍 browser，false
        let normal = classify_builtin("chrome.exe", "随便一个网页");
        assert_eq!(normal.scene, SceneKind::Browser, "普通标题应维持 browser");
        assert!(!normal.multiline_safe);
    }

    #[test]
    fn scene_md003_sticky_notes_parse() {
        let r = toml::from_str::<Rules>(BUILTIN_RULES).unwrap();
        let doc = r
            .scene
            .iter()
            .find(|s| s.kind == "doc")
            .expect("doc 块存在");
        assert!(
            doc.exe
                .iter()
                .any(|e| e.eq_ignore_ascii_case("StickyNotesStub.exe")),
            "StickyNotesStub.exe 应在 doc 词表"
        );
        assert!(
            doc.exe
                .iter()
                .any(|e| e.eq_ignore_ascii_case("Microsoft.Notes.exe")),
            "Microsoft.Notes.exe 应在 doc 词表"
        );
    }

    /// TEST-SYNC 追补 0a：Mastodon 永久反向护栏。
    /// ⚠️ 本测试存在的唯一理由：DATA-SCENE-GENERIC-008 曾想把裸 `todo` 收为 doc 泛化关键词，
    /// 主控实证否决——`Mastodon` 含子串 m-as-**todo**-n（大小写不敏感），是社交网络而非文档，
    /// 收裸 `todo` 会把它判成 doc（multiline_safe=true），方向完全反了；西语/葡语 `todo`（=「全部」）
    /// 也是高频误伤。最终改收 `To Do`（带空格）+ `待办`。
    /// 若将来有人把裸 `todo` 加进 `title_keywords`，此测试立刻撞红——这是本测试唯一的守卫价值，
    /// 注释不明者勿删（看似冗余实则防回归）。
    #[test]
    fn scene_md005_mastodon_not_doc() {
        for title in ["Mastodon - 首页", "Mastodon Social"] {
            let s = classify_builtin("chrome.exe", title);
            assert_ne!(
                s.scene,
                SceneKind::Doc,
                "title={title} 含 todo 子串（Mastodon）但不得判 doc"
            );
            assert!(!s.multiline_safe, "title={title} 不得放开多行");
        }
        // 正向对照：带空格的 To Do 仍应命中 doc/true——证明没因噎废食把真目标也挡掉
        let s = classify_builtin("chrome.exe", "Microsoft To Do - 我的待办");
        assert_eq!(s.scene, SceneKind::Doc, "Microsoft To Do 应 doc");
        assert!(s.multiline_safe, "Microsoft To Do 应 true");
    }

    /// TEST-SYNC 追补 0b：本批新增关键词覆盖（DATA-SCENE-GENERIC-008 + FIX-SCENE-WEBTITLE-007）。
    /// chrome.exe + 标题 → doc/true；UnknownApp + 已发送 → email/true。
    #[test]
    fn scene_md005_new_doc_keywords() {
        for title in [
            "飞书云文档 - 协作",
            "WPS云文档 - 我的表格",
            "腾讯云文档 - 共享",
            "便签 - 我的备忘",
            "待办 - 今日任务",
            "Google 文档 - 报告",
            "报告 - Word Online",
            "Microsoft 365 - 首页",
        ] {
            let s = classify_builtin("chrome.exe", title);
            assert_eq!(s.scene, SceneKind::Doc, "title={title} 应 doc");
            assert!(s.multiline_safe, "title={title} 应 true");
        }
        // 云文档 泛化兜底（DEC-038 规则性泛化）：3 字 > 钉钉/飞书(2) 最长匹配胜出
        let s = classify_builtin("chrome.exe", "华为云文档 - 团队空间");
        assert_eq!(s.scene, SceneKind::Doc, "云文档 泛化应 doc");
        // UnknownApp + 已发送 → email/true（中文邮箱发件夹标题审计实证）
        // 注：标题避开邮件客户端 exe 名（如 Thunderbird），确保走「已发送」关键词路径
        let s = classify_builtin("UnknownApp.exe", "已发送");
        assert_eq!(s.scene, SceneKind::Email, "已发送 应 email");
        assert!(s.multiline_safe, "已发送 email 应 true");
    }
}
