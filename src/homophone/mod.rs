//! HOMOPHONE-NODE-318：同音纠错**可挂载管线节点**。
//!
//! 形态照 `apply_filler_strip`(305) / `apply_local_punctuation`(238 N9)：**自由函数、显式传参、
//! 不吃 `&self`、不碰管线状态**。谁要谁调一行。Gavin 口径「做成单独的节点，可以挂载到管线上」——
//! 故**不**用 sherpa 的 HomophoneReplacer（它绑在流式 recognizer 的 config 上，只能改预览）。
//!
//! 规则外置照 `itn-rules.toml`(DEC-030)：`homophone-rules.toml` **exe 同级优先**，
//! 缺失/解析失败回退 `include_str!` 内置默认（零配置可用）⇒ 以后加词条只换 toml、不用出包。
//!
//! 🔴 只修 **ASR 听错**（同音同调的单向替换），表内每条都经「单向无歧义 + 变体非词 +
//! 变体不嵌于更长词 + 无繁体/语法助词」四道过滤（详见 `homophone-rules.toml` 头注）。

use std::collections::HashSet;
use std::sync::OnceLock;

/// 内置默认规则（`include_str!` 编译期嵌入；exe 同级同名文件存在则覆盖）。
const BUILTIN_RULES: &str = include_str!("../../homophone-rules.toml");

#[derive(Debug, Clone, serde::Deserialize)]
struct FixEntry {
    from: String,
    to: String,
    #[serde(default)]
    #[allow(dead_code)] // 保留来源标注，供审计/后续工具读取
    src: String,
}

#[derive(Debug, Default, serde::Deserialize)]
struct RuleFile {
    #[serde(default)]
    fix: Vec<FixEntry>,
}

/// 编译后的替换表（按 `from` 长度降序，保证最长匹配优先）。
struct CompiledRules {
    entries: Vec<(String, String)>,
}

impl CompiledRules {
    fn from_rules(file: RuleFile) -> Self {
        let mut entries: Vec<(String, String)> = file
            .fix
            .into_iter()
            .filter(|e| !e.from.is_empty() && e.from != e.to)
            .map(|e| (e.from, e.to))
            .collect();
        // 最长优先；同长按 from 字典序（稳定）。
        entries.sort_by(|a, b| {
            b.0.chars()
                .count()
                .cmp(&a.0.chars().count())
                .then_with(|| a.0.cmp(&b.0))
        });
        // 同一 from 只保留首个（去重）。
        let mut seen: HashSet<String> = HashSet::new();
        entries.retain(|(f, _)| seen.insert(f.clone()));
        Self { entries }
    }

    /// 单趟最长匹配替换（不重叠）。
    fn apply_once(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut i = 0usize;
        while i < text.len() {
            let rest = &text[i..];
            let mut matched = false;
            for (from, to) in &self.entries {
                if rest.starts_with(from.as_str()) {
                    out.push_str(to);
                    i += from.len();
                    matched = true;
                    break;
                }
            }
            if !matched {
                let ch = rest.chars().next().expect("non-empty rest");
                out.push(ch);
                i += ch.len_utf8();
            }
        }
        out
    }

    /// 应用规则 = 单趟最长匹配替换。
    ///
    /// 🔴 **幂等性由构造保证**：单趟从左到右、命中即前进，绝不回扫已产出的 `to`；
    /// 只要「任一 `to` 不含任何 `from`」（表不变量，见 `t8` 护栏），输出里就不残留 `from`
    /// ⇒ `f(f(x)) == f(x)`。**不用迭代到不动点**——那会在病态表下反复改写 `to`（`t7` 曾踩到）。
    fn apply(&self, text: &str) -> String {
        self.apply_once(text)
    }
}

static RULES: OnceLock<CompiledRules> = OnceLock::new();

fn load_rules() -> CompiledRules {
    // 1) exe 同级 `homophone-rules.toml` 优先（DEC-011）。
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("homophone-rules.toml");
            if let Ok(content) = std::fs::read_to_string(&path) {
                match toml::from_str::<RuleFile>(&content) {
                    Ok(r) => {
                        log::info!("Homophone rules loaded from {:?}", path);
                        return CompiledRules::from_rules(r);
                    }
                    Err(e) => {
                        log::warn!(
                            "Homophone rules parse error in {:?}, falling back to builtin: {}",
                            path,
                            e
                        );
                    }
                }
            }
        }
    }
    // 2) 内置默认（缺失/解析失败都降级，零配置可用）。
    match toml::from_str::<RuleFile>(BUILTIN_RULES) {
        Ok(r) => CompiledRules::from_rules(r),
        Err(e) => {
            log::warn!("Homophone builtin rules parse error: {}", e);
            CompiledRules::from_rules(RuleFile::default())
        }
    }
}

fn rules() -> &'static CompiledRules {
    RULES.get_or_init(load_rules)
}

/// HOMOPHONE-NODE-318：本地同音纠错节点（**管线无关**，谁要谁挂）。
///
/// - `enabled == false` ⇒ **原样返回**，一个字符不动
/// - `enabled == true` ⇒ 按规则表做最长匹配替换（迭代到不动点，保幂等）
///
/// **纯函数契约**：规则在**首次调用**时经 `OnceLock` 加载一次（不在每次调用做 IO），
/// 无全局可变状态。变化时打一行 `log::info!`（同 `apply_filler_strip` 风格）。
pub fn apply_homophone_fix(text: String, enabled: bool) -> String {
    if !enabled {
        return text;
    }
    let fixed = rules().apply(&text);
    if fixed != text {
        log::info!("Homophone fix applied: '{}' -> '{}'", text, fixed);
    }
    fixed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用：从内容编译（避开 exe 同级残留 toml，[TOML-STALE-001]）。
    fn compile_from_content(content: &str) -> CompiledRules {
        match toml::from_str::<RuleFile>(content) {
            Ok(r) => CompiledRules::from_rules(r),
            Err(_) => CompiledRules::from_rules(RuleFile::default()),
        }
    }

    fn builtin() -> CompiledRules {
        compile_from_content(BUILTIN_RULES)
    }

    /// 正例：表内各类确实被改（含首字/中字/末字位置）。
    #[test]
    fn t1_positive_cases_are_applied() {
        let r = builtin();
        assert_eq!(r.apply("这个泄药不能吃"), "这个泻药不能吃");
        assert_eq!(r.apply("我一但迟到就麻烦了"), "我一旦迟到就麻烦了");
        assert_eq!(r.apply("他朋有不多"), "他朋友不多");
        assert_eq!(r.apply("请座下"), "请坐下");
    }

    /// 幂等：`f(f(x)) == f(x)`（硬约束 #1）。
    #[test]
    fn t2_idempotent() {
        let r = builtin();
        let mixed = "我一但迟到，就先吃泄药，然后朋有请座下，座火车去台湾。";
        let once = r.apply(mixed);
        assert_eq!(r.apply(&once), once, "节点必须幂等");
    }

    /// 表内容护栏：明确不许收的**双向歧义词**若被人加进表 → 红。
    /// 判据：这些 `from` 不得出现在表内（任务 §五 边界外用例）。
    #[test]
    fn t3_ambiguous_words_must_not_be_in_table() {
        let r = builtin();
        const FORBIDDEN: [&str; 4] = ["不单", "惟独", "做美", "洗炼"];
        for f in FORBIDDEN {
            assert!(
                !r.entries.iter().any(|(from, _)| from == f),
                "表内容护栏：双向歧义词 {f:?} 不许收进同音替换表"
            );
        }
        // 语法助词单字不得作 from（会误伤面极大）。
        for ch in ["的", "地", "得", "着", "了", "过"] {
            assert!(
                !r.entries.iter().any(|(from, _)| from == ch),
                "表内容护栏：语法助词 {ch:?} 不许作 from"
            );
        }
    }

    /// 表规模落在目标区间（宁少勿滥；越界提醒复核，不静默）。
    #[test]
    fn t4_table_size_in_target_range() {
        let n = builtin().entries.len();
        assert!(
            (100..=250).contains(&n),
            "同音表条目数 {n} 超出 100~250 目标区间"
        );
    }

    /// toml 缺失/解析失败 → 降级内置默认仍可用（不会 panic、不会空转）。
    #[test]
    fn t5_missing_or_bad_toml_degrades_to_builtin() {
        // 解析失败内容 → 空表，但绝不 panic。
        let bad = compile_from_content("fix = [ { this is not toml ,,, ]");
        assert_eq!(bad.apply("随便什么文本"), "随便什么文本");
        // 空内容 → 空表。
        let empty = compile_from_content("");
        assert_eq!(empty.apply("一但"), "一但");
        // 内置默认本身可解析且非空。
        assert!(!builtin().entries.is_empty());
    }

    /// 挂载点契约：`enabled=false` 时逐字不变（开 LLM 或用户关掉时原样透传）。
    #[test]
    fn t6_disabled_is_identity() {
        let s = "这个泄药不能吃，我一但迟到就麻烦".to_string();
        assert_eq!(apply_homophone_fix(s.clone(), false), s);
    }

    /// 最长匹配优先：短规则不得吃掉长规则的首段。
    #[test]
    fn t7_longest_match_wins() {
        let r = compile_from_content(
            "[[fix]]\nfrom=\"成\"\nto=\"X\"\n[[fix]]\nfrom=\"成工\"\nto=\"成功\"\n",
        );
        assert_eq!(r.apply("成工"), "成功");
    }

    /// 表不变量（幂等的地基）：**任一 `to` 不得包含任何 `from`**。
    /// 违反 ⇒ 单趟替换后的正确文本会被另一条规则二次改写，`f(f(x)) != f(x)`。
    /// 新增词条若踩此线，本测试变红要求人工处理（改 to 或删 from）。
    #[test]
    fn t8_no_to_contains_any_from() {
        let r = builtin();
        for (from, to) in &r.entries {
            for (f2, _) in &r.entries {
                assert!(
                    !to.contains(f2.as_str()),
                    "表不变量违反：to={to:?}（来自 from={from:?}）包含另一条 from={f2:?} ⇒ 破坏幂等"
                );
            }
        }
    }
}
