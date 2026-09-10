//! # PROMPT-LAB · 系统提示词实验室
//!
//! 专供**提示词与提示词模块调优**使用的常驻调试模块（Gavin 2026-09-10 提议）。
//!
//! ## 为什么要有它
//!
//! 2026-09-10 排查「翻译上屏不出无序列表」时，主控反复手写临时探针又删掉，
//! 一天之内重复实现了四遍「多轮采样 + 指标统计」，并因 Rust 字符串尾部反斜杠
//! 与 Windows 路径转义踩坑 4 次。更严重的是**前两次结论都建立在单次采样上而被推翻**
//! （详见 `logs/20260910.md` VERIFY-205 → VERIFY-206）。本模块把那些能力固化下来。
//!
//! ## 🔴 本模块存在的第一原则：LLM 的单次输出不能定论
//!
//! `temperature` 非 0 时同一配置会跑出相反结果。**任何结论都必须基于多轮采样**，
//! 因此本模块的在线用例一律内建 N 轮循环与「输出种数」稳定性指标，不提供单次接口。
//!
//! ## 用法
//!
//! 全部用例标了 `#[ignore]`，`cargo test` 常规回归不会跑到。手动触发：
//!
//! ```text
//! # 离线（不花钱）：看 prompt 渲染结果、分段体量、前缀缓存命中
//! cargo test --bin feiyin-ime lab_dump_prompt   -- --ignored --nocapture
//! cargo test --bin feiyin-ime lab_cache_prefix  -- --ignored --nocapture
//!
//! # 在线（🔴 调真实 API，花钱）：多轮采样 / A-B 对照
//! cargo test --bin feiyin-ime lab_sample -- --ignored --nocapture
//! cargo test --bin feiyin-ime lab_ab     -- --ignored --nocapture
//! ```
//!
//! 参数走环境变量（不用改代码）：
//!
//! | 变量 | 默认 | 说明 |
//! | --- | --- | --- |
//! | `LAB_ROUNDS` | 5 | 每个配置采样轮数。**不建议低于 3** |
//! | `LAB_EXE` | `notepad.exe` | 目标应用 exe，决定场景与 `multiline_safe` |
//! | `LAB_TEMP` | 生产值 | 覆盖 temperature（如 `0.1`）；不设则用生产装配值 |
//! | `LAB_SAMPLE` | 全部 | 只跑指定样本 id（如 `enum_explicit`） |
//! | `LAB_TRANSLATE` | `0` | `1` = 走翻译路径（`optimize_and_translate`） |
//!
//! ## 🔴 安全红线
//!
//! 从生产 `config.toml` 读取 api_key，**任何情况下不打印、不写入文件**
//! （`[SECRET-IN-REPO-001]`：2026-08-14 曾因 dump 配置把 key 写进仓库并遭盗刷）。
//! 本模块只输出 prompt 文本与模型响应。

use super::*;
use std::collections::BTreeSet;
use std::time::Duration;

/// 生产配置路径。只读取，绝不回写。
const PROD_CONFIG: &str = "D:/Workspace/CodeLab/voice-ime/Publish/config.toml";

// ============================================================
// 期望行为与样本集
// ============================================================

/// 样本的期望形态。用于把「跑一遍看看」变成「有判据的实验」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expect {
    /// 应当输出列表（≥2 个 bullet）
    List,
    /// 应当保持段落 —— 过度列表化同样是回归（F3 明载）
    Paragraph,
}

struct Sample {
    id: &'static str,
    /// 关注点，打印在结果里，避免看到数字想不起来在测什么
    focus: &'static str,
    text: &'static str,
    expect: Expect,
}

/// 内置样本集：每条都对应一个真实踩过的坑或一条必须守住的规则。
/// 🔴 调优时**固定跑这一整套**，只看单条容易顾此失彼（列表修好了却把叙述也列表化）。
const SAMPLES: &[Sample] = &[
    Sample {
        id: "enum_explicit",
        focus: "显式无序标记（比如/再比如/还有比如）→ 必须成列表。2026-09-10 Gavin 报障原型",
        text: "你可以多培养一些自己的爱好和兴趣，比如说平时可以看看书，从书的内容里面多学一些东西；\
再比如说可以去看电影，一个人待着放松放松；还有比如说可以去旅游，看看外面的世界。这些爱好和兴趣都是非常好的。",
        expect: Expect::List,
    },
    Sample {
        id: "narration",
        focus: "流水叙述、无并列关系 → 必须保持段落。防「修好列表却过度列表化」的反向回归",
        text: "今天下午要开个会，然后还要写一份项目报告，晚上得把那个方案改完。",
        expect: Expect::Paragraph,
    },
    Sample {
        id: "short_nouns",
        focus: "并列但都是短名词（无谓语、无内部标点、≤6 字）→ 走 INLINE 不成列表（F3-item form）",
        text: "今天出去买菜了，买了三斤土豆，一个西瓜，二十斤大米，还有三斤香蕉。",
        expect: Expect::Paragraph,
    },
    Sample {
        id: "redundancy",
        focus: "同一点说两遍 → 应压缩为一次（VERBOSE-195 CONDENSE 条款）",
        text: "帮我把登录那个 bug 修一下，就是登录的那个 bug，用户点了登录之后没反应，点了之后没有任何反应。",
        expect: Expect::Paragraph,
    },
    Sample {
        id: "brake",
        focus: "🔴 压缩刹车：自我更正只留最终值，但「只/别动/30 秒」一个都不许丢（HARD LIMIT）",
        text: "只重启 web 容器，别动数据库，超时改成三十秒，不是三秒是三十秒。",
        expect: Expect::Paragraph,
    },
];

// ============================================================
// 参数与工具
// ============================================================

fn env_str(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_f32(key: &str) -> Option<f32> {
    std::env::var(key).ok().and_then(|v| v.parse().ok())
}

/// 粗估 token：ASCII 约 4 字符/token，CJK 约 1.5 字符/token。
/// 只用于横向比较段落体量，不作计费依据。
fn est_tokens(s: &str) -> usize {
    let ascii = s.chars().filter(|c| c.is_ascii()).count();
    let cjk = s.chars().count() - ascii;
    ascii / 4 + cjk * 2 / 3
}

fn bullet_lines(s: &str) -> usize {
    s.lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("- ") || t.starts_with("* ") || t.starts_with("1. ")
        })
        .count()
}

/// 读取生产配置。🔴 只取所需字段，**绝不打印 api_key**。
fn load_prod_config() -> Option<crate::config::AppConfig> {
    let raw = std::fs::read_to_string(PROD_CONFIG).ok()?;
    let cfg: crate::config::AppConfig = toml::from_str(&raw).ok()?;
    if cfg.llm.api_key.trim().is_empty() {
        println!("LAB 跳过：生产 config.toml 未配置 api_key");
        return None;
    }
    println!(
        "LAB 模型 ={} api_url={}（api_key 已读取，按红线不打印）",
        cfg.llm.model, cfg.llm.api_url
    );
    Some(cfg)
}

fn scene_for(exe: &str) -> crate::scene::SceneContext {
    crate::scene::classify_scene(exe, "")
}

/// 一个配置下的多轮采样结果。
struct Rounds {
    outputs: Vec<String>,
}

impl Rounds {
    fn hit_rate(&self, expect: Expect) -> (usize, usize) {
        let n = self.outputs.len();
        let hit = self
            .outputs
            .iter()
            .filter(|o| match expect {
                Expect::List => bullet_lines(o) >= 2,
                Expect::Paragraph => bullet_lines(o) == 0,
            })
            .count();
        (hit, n)
    }

    /// 不同输出的种数。1 = 完全稳定；>1 说明存在随机抖动。
    fn distinct(&self) -> usize {
        self.outputs.iter().collect::<BTreeSet<_>>().len()
    }

    fn first(&self) -> String {
        self.outputs
            .first()
            .cloned()
            .unwrap_or_default()
            .replace(char::from(10), " / ")
    }
}

/// 跑 N 轮真实 API。`temp_override` 为 `None` 时使用生产装配的 temperature。
fn sample_rounds(
    client: &LlmClient,
    rt: &tokio::runtime::Runtime,
    scene: &crate::scene::SceneContext,
    text: &str,
    rounds: usize,
    temp_override: Option<f32>,
    translate: bool,
) -> Rounds {
    let url = client.chat_completions_url();
    let mut outputs = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        if translate {
            // 翻译路径参数多，直接走公开入口；temperature 覆盖对该路径不生效，
            // 需要覆盖时请改用非翻译路径，或临时改生产常量后重跑（并在报告里注明）。
            let r = rt.block_on(client.optimize_and_translate(
                text,
                crate::config::TranslationLanguage::English,
                None,
                true,
                Some(scene),
                false,
                scene.multiline_safe,
            ));
            outputs.push(match r {
                Ok(v) => v.text.trim().to_string(),
                Err(e) => format!("ERR {e}"),
            });
        } else {
            let mut body = client.build_optimize_request(
                text,
                None,
                true,
                Some(scene),
                scene.multiline_safe,
                false,
            );
            if let Some(t) = temp_override {
                body.temperature = Some(t);
            }
            let r = rt.block_on(client.try_once_raw(&url, &body, Duration::from_secs(30)));
            outputs.push(match r {
                Ok(resp) => parse_suggestions_from_response(&resp)
                    .text
                    .trim()
                    .to_string(),
                Err(e) => format!("ERR {e}"),
            });
        }
    }
    Rounds { outputs }
}

fn make_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
}

fn selected_samples() -> Vec<&'static Sample> {
    match std::env::var("LAB_SAMPLE") {
        Ok(id) => SAMPLES.iter().filter(|s| s.id == id).collect(),
        Err(_) => SAMPLES.iter().collect(),
    }
}

// ============================================================
// 离线用例（不调 API，不花钱）
// ============================================================

/// 打印真实渲染的 system prompt：总体量 + 逐段体量 + token 估算。
///
/// 用途：改提示词前后看哪一段在膨胀；定位「某条规则到底进没进 prompt」。
#[test]
#[ignore]
fn lab_dump_prompt() {
    let exe = env_str("LAB_EXE", "notepad.exe");
    let sc = scene_for(&exe);
    let scene_block = crate::scene::build_scene_prompt_block(&sc, false);
    println!(
        "LAB exe={exe} kind={} multiline_safe={}",
        sc.scene.as_str(),
        sc.multiline_safe
    );

    let layers = build_prompt_layers(
        SYSTEM_BASE_PROMPT,
        None,
        None,
        scene_block,
        sc.multiline_safe,
        true,
    );
    let full = render(&layers);
    println!(
        "LAB 总体量 {} chars / ~{} tok",
        full.len(),
        est_tokens(&full)
    );
    println!("LAB ---- 逐段 ----");
    for layer in &layers {
        for rule in &layer.rules {
            println!(
                "LAB   L{} {:<22} {:>6} chars / ~{:>5} tok",
                layer.level,
                rule.id,
                rule.text.len(),
                est_tokens(&rule.text)
            );
        }
    }
    if std::env::var("LAB_FULL").is_ok() {
        println!("LAB ---- 全文 ----\n{full}\n LAB ---- 全文结束 ----");
    } else {
        println!("LAB （设 LAB_FULL=1 打印 prompt 全文）");
    }
}

/// 前缀缓存分析：不同场景之间逐字节相同的前缀有多长。
///
/// 背景：PROMPT-OPT-202 把 L3 的 `f3_lists` 提到 `scene_f4` 之前，
/// 同 `multiline_safe` 下切场景的公共前缀从 9,427B(64%) 升到 13,521B(92%)。
/// 改动装配顺序、或往靠前的层塞随场景变化的内容时，用本用例看代价。
#[test]
#[ignore]
fn lab_cache_prefix() {
    let exes = [
        "Claude.exe",
        "WindowsTerminal.exe",
        "WeChat.exe",
        "notepad.exe",
    ];
    let mut rendered: Vec<(String, String)> = Vec::new();
    for exe in exes {
        let sc = scene_for(exe);
        let sb = crate::scene::build_scene_prompt_block(&sc, false);
        let p = render(&build_prompt_layers(
            SYSTEM_BASE_PROMPT,
            None,
            None,
            sb,
            sc.multiline_safe,
            true,
        ));
        println!(
            "LAB {exe:<22} kind={:<14} ml={:<5} {} chars",
            sc.scene.as_str(),
            sc.multiline_safe,
            p.len()
        );
        rendered.push((exe.to_string(), p));
    }
    println!("LAB ---- 两两公共前缀 ----");
    for i in 0..rendered.len() {
        for j in (i + 1)..rendered.len() {
            let (na, a) = &rendered[i];
            let (nb, b) = &rendered[j];
            let common = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
            println!(
                "LAB   {na:<22} vs {nb:<22} {common:>6} B ({:.0}% of {})",
                common as f64 * 100.0 / a.len() as f64,
                a.len()
            );
        }
    }
}

// ============================================================
// 在线用例（🔴 调真实 API，花钱）
// ============================================================

/// 多轮采样：在当前提示词下跑完整样本集，给出命中率与稳定性。
///
/// 🔴 这是提示词改动的**基线记录手段** —— 改动前先跑一次存下数字，改完再跑一次对照。
#[test]
#[ignore]
fn lab_sample() {
    let Some(cfg) = load_prod_config() else {
        return;
    };
    let rounds = env_usize("LAB_ROUNDS", 5);
    let exe = env_str("LAB_EXE", "notepad.exe");
    let temp = env_f32("LAB_TEMP");
    let translate = env_str("LAB_TRANSLATE", "0") == "1";
    let sc = scene_for(&exe);
    let client = LlmClient::new(cfg.llm.clone());
    let rt = make_runtime();

    println!(
        "LAB 采样 rounds={rounds} exe={exe} kind={} ml={} temp={} translate={translate}",
        sc.scene.as_str(),
        sc.multiline_safe,
        temp.map(|t| t.to_string())
            .unwrap_or_else(|| "生产值".into())
    );
    for s in selected_samples() {
        let r = sample_rounds(&client, &rt, &sc, s.text, rounds, temp, translate);
        let (hit, n) = r.hit_rate(s.expect);
        let flag = if hit == n {
            "✅"
        } else if hit == 0 {
            "🔴"
        } else {
            "⚠️"
        };
        println!(
            "LAB {flag} {:<14} 期望={:<9} 命中 {hit}/{n}  输出种数 {}/{n}",
            s.id,
            format!("{:?}", s.expect),
            r.distinct()
        );
        println!("LAB      关注点: {}", s.focus);
        println!("LAB      首轮输出: {}", r.first());
    }
}

/// A/B 对照：同一样本集，两种基座提示词。
///
/// 默认 A = 当前生产基座常量，B = `LAB_BASE_B_FILE` 指向的文本文件。
/// 用于回答「换掉这段提示词到底有没有变好」——**这是 DEC-059 要求的实证形式**。
///
/// 用例来源：2026-09-10 正是用这个方法定位到「老 config.toml 陈旧基座」才是列表不生效的根因
/// （旧基座 2/10、新基座 9/10），此前基于单次采样的两个结论都被推翻。
#[test]
#[ignore]
fn lab_ab() {
    let Some(cfg) = load_prod_config() else {
        return;
    };
    let rounds = env_usize("LAB_ROUNDS", 5);
    let exe = env_str("LAB_EXE", "notepad.exe");
    let sc = scene_for(&exe);
    let rt = make_runtime();

    let base_b = match std::env::var("LAB_BASE_B_FILE") {
        Ok(path) => std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("读取 LAB_BASE_B_FILE={path} 失败: {e}")),
        Err(_) => {
            println!("LAB 未设 LAB_BASE_B_FILE，B 组回落为「空基座」以观察基座整体贡献");
            String::new()
        }
    };

    let mut a_cfg = cfg.llm.clone();
    let mut b_cfg = cfg.llm.clone();
    // 基座已是编译期常量（PROMPT-BASE-207），A/B 通过替换 config 里的字段无法生效，
    // 故此处改用 extra_instruction 之外的唯一可注入点：直接构造两个 client 并在
    // 装配时传入不同基座。为保持简单，B 组走 build_prompt_layers 手工装配对照。
    a_cfg.api_key = cfg.llm.api_key.clone();
    b_cfg.api_key = cfg.llm.api_key.clone();
    let client = LlmClient::new(a_cfg);

    println!(
        "LAB A/B rounds={rounds} exe={exe}  A=生产基座({} chars)  B=自定义({} chars)",
        SYSTEM_BASE_PROMPT.len(),
        base_b.len()
    );

    for s in selected_samples() {
        // A 组：生产装配（基座 = SYSTEM_BASE_PROMPT）
        let ra = sample_rounds(&client, &rt, &sc, s.text, rounds, None, false);
        let (ha, na) = ra.hit_rate(s.expect);

        // B 组：手工装配，仅替换基座
        let url = client.chat_completions_url();
        let sb = crate::scene::build_scene_prompt_block(&sc, false);
        let layers = build_prompt_layers(&base_b, None, None, sb, sc.multiline_safe, true);
        let system_prompt = render(&layers);
        let mut outs = Vec::with_capacity(rounds);
        for _ in 0..rounds {
            let body = ChatRequest {
                model: client.config.model.clone(),
                messages: vec![
                    RequestMessage {
                        role: "system".to_string(),
                        content: system_prompt.clone(),
                    },
                    RequestMessage {
                        role: "user".to_string(),
                        content: format!("<speech>{}</speech>", s.text),
                    },
                ],
                temperature: Some(0.3),
                max_tokens: Some(2048),
                stream: None,
                enable_thinking: Some(false),
                thinking: Some(ThinkingConfig {
                    thinking_type: "disabled".to_string(),
                }),
            };
            let r = rt.block_on(client.try_once_raw(&url, &body, Duration::from_secs(30)));
            outs.push(match r {
                Ok(resp) => parse_suggestions_from_response(&resp)
                    .text
                    .trim()
                    .to_string(),
                Err(e) => format!("ERR {e}"),
            });
        }
        let rb = Rounds { outputs: outs };
        let (hb, nb) = rb.hit_rate(s.expect);

        println!(
            "LAB {:<14} 期望={:<9}  A {ha}/{na}(种数{})   B {hb}/{nb}(种数{})",
            s.id,
            format!("{:?}", s.expect),
            ra.distinct(),
            rb.distinct()
        );
    }
    println!("LAB 🔴 判读提醒：命中率差距 <2/N 时不要下结论，加大 LAB_ROUNDS 再跑。");
}
