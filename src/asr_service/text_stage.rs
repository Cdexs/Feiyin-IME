//! ASR-SERVER-491 S10：远程会话的服务端文字处理（设计 §13 + 「Gavin 10-17 修正」）。
//!
//! 终端只能开关三项（`OpenParams.itn / punctuation / translate`，阿里同名字段）；同音纠错、去口水、
//! 繁简等固定内置。节点顺序、函数与本机 `run_pipeline_core`（不走大模型格式化那条分支）一致，
//! 每个节点「调现有函数 + `char_times::transfer` 搬时间」，函数本身一行不改：
//!
//! | 顺序 | 节点 | 预览 | 终稿 |
//! | --- | --- | --- | --- |
//! | 0 | 剥光标点（DEC-112，参数开标点时；FLASH-P0-508） | — | ✅ |
//! | 1 | 无效文本（只有口水词 ⇒ 空） | — | ✅ |
//! | 2 | 同音纠错 | ✅ | ✅ |
//! | 3 | 数字规整（参数） | ✅ | ✅ |
//! | 4 | 输出优化（参数，大模型；FLASH-P0-510）／ 否则繁简 + 英文大小写规整 | — | ✅ |
//! | 5 | 单位补丁（数字规整补丁通道，DEC-036；FLASH-P0-508） | — | ✅ |
//! | 6 | 补标点（参数开、大模型未接手）—— 剥光后由标点模型整段按语义重打 | — | ✅ |
//! | 7 | 标点收尾（关 ⇒ 全剥；开 ⇒ 短句剥句末；参数 `strip_trailing` ⇒ 剥句末） | 只做「关 ⇒ 全剥」 | ✅ |
//!
//! FLASH-P0-508：终稿顺序与本机 `run_pipeline_core` 本地实时分支逐项对齐（本机：会话收尾繁简 ⇒
//! `strip_punctuation_node` ⇒ 有效文本 ⇒ 同音 ⇒ 数字规整 ⇒ `run_llm_stage` ⇒ 单位补丁 ⇒
//! `apply_local_punctuation` ⇒ L2）。繁简已由会话收尾先做一遍（`session.rs`），与本机同。
//! 去口水：本地实时管线回灌前已过一遍（441 `DoneUpstream`），远程同一路径 ⇒ 不再过第二遍。
//! 翻译另出一份（[`translate`]），不改原文。

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use super::frame::TranslationPayload;
use crate::config::{AppConfig, ChineseScript, TranslationLanguage};
use crate::transcription::char_times::{self, CharTime};
use crate::{homophone, itn, punctuation, text_normalizer, translation};

/// 终端可控的三项之外、处理文字要用的会话参数。
pub(crate) struct TextOpts {
    pub itn: bool,
    pub punctuation: bool,
    pub script: ChineseScript,
    pub model_dir: PathBuf,
    /// FLASH-P0-508：去句末标点（终端参数）。
    pub strip_trailing: bool,
    /// FLASH-P0-510：输出优化（终端参数；大模型用服务端统一配置）。
    pub optimize: bool,
}

type Times = Vec<Option<CharTime>>;

/// 一个节点：文字变了才搬时间（没变 ⇒ 原样，零开销）。
fn step(text: String, times: Times, f: impl FnOnce(&str) -> String) -> (String, Times) {
    let out = f(&text);
    if out == text {
        return (text, times);
    }
    let t = char_times::transfer(&text, &times, &out);
    (out, t)
}

/// 预览帧：同音纠错 ⇒ 数字规整（参数）⇒ 关标点时全剥。
pub(crate) fn apply_preview(
    text: &str,
    times: &[Option<CharTime>],
    o: &TextOpts,
) -> (String, Times) {
    let cur = (text.to_string(), times.to_vec());
    let cur = step(cur.0, cur.1, |t| {
        homophone::apply_homophone_fix(t.to_string(), true)
    });
    let cur = if o.itn {
        step(cur.0, cur.1, itn::normalize_numbers)
    } else {
        cur
    };
    if o.punctuation {
        cur
    } else {
        step(cur.0, cur.1, |t| {
            punctuation::apply_l2_postprocess(t, false, false).0
        })
    }
}

/// 预览的「末尾未稳定字数」换算到处理后的文字（数字规整等会改字数，如「一百二十」⇒「120」）。
/// 做法：每个原字的「时间」记成它的下标，过一遍同样的节点，处理后末尾那些落在未稳定区的字即未稳定。
/// 被替换的一组字按比例分摊原字位置（`transfer` 均分），落点在未稳定区的算未稳定：「我有一百二十」
/// 末 2 字未稳 ⇒「我有120」末 2 字（「20」）未稳。末尾无位置的字（插入）一律算未稳。
pub(crate) fn preview_tentative(text: &str, tentative: usize, o: &TextOpts) -> usize {
    if tentative == 0 {
        return 0;
    }
    let n = text.chars().count();
    let stable = n.saturating_sub(tentative) as f32;
    let marks: Times = (0..n)
        .map(|i| {
            Some(CharTime {
                begin: i as f32,
                end: i as f32,
                src: char_times::TsSource::Aligned,
            })
        })
        .collect();
    let (_, m) = apply_preview(text, &marks, o);
    m.iter()
        .rev()
        .take_while(|t| t.map_or(true, |t| t.end >= stable))
        .count()
}

/// 终稿：全部节点（顺序见模块头）。只有口水词 ⇒ 空文本。`config` 只供输出优化取服务端统一大模型配置。
pub(crate) fn apply_final(
    text: &str,
    times: &[Option<CharTime>],
    o: &TextOpts,
    config: &AppConfig,
) -> (String, Times) {
    // FLASH-P0-508（DEC-112）：开标点 ⇒ 先剥光，交下游标点模型整段按语义重打（同本机 `strip_punctuation_node`）。
    let cur = if o.punctuation {
        step(text.to_string(), times.to_vec(), |t| {
            punctuation::strip_effective_punctuation(t)
        })
    } else {
        (text.to_string(), times.to_vec())
    };
    if !text_normalizer::is_effective_text(&cur.0) {
        if !cur.0.trim().is_empty() {
            log::info!("[ASR-SERVER-491] remote final not effective (filler only): {text:?}");
        }
        return (String::new(), Vec::new());
    }
    let cur = step(cur.0, cur.1, |t| {
        homophone::apply_homophone_fix(t.to_string(), true)
    });
    // 输出优化的繁简指令按「同音纠错后、数字规整前」的文字定（同本机 `run_llm_stage` 的 `raw_text`）。
    let raw_text = cur.0.clone();
    let cur = if o.itn {
        step(cur.0, cur.1, itn::normalize_numbers)
    } else {
        cur
    };
    let optimized = o
        .optimize
        .then(|| llm_optimize(&cur.0, &raw_text, config, o))
        .flatten();
    let llm_handled = optimized.is_some();
    let cur = match optimized {
        Some(t) => step(cur.0, cur.1, |_| t),
        None => step(cur.0, cur.1, |t| {
            text_normalizer::normalize_text_for_language(t, o.script)
        }),
    };
    let cur = if o.itn {
        step(cur.0, cur.1, itn::normalize_unit_symbols_only)
    } else {
        cur
    };
    let cur = if o.punctuation && !llm_handled {
        step(cur.0, cur.1, |t| punctuate(&o.model_dir, t))
    } else {
        cur
    };
    step(cur.0, cur.1, |t| {
        punctuation::apply_l2_postprocess(t, o.punctuation, o.strip_trailing).0
    })
}

/// FLASH-P0-510：输出优化（同本机 `run_llm_stage` 非翻译分支）：大模型用服务端统一配置；场景未知、
/// 不多行（远程不知道落点窗口）、不外发窗口标题；学习建议不写服务端本机词库（P2 起随结果回给终端）。
/// 服务端没开大模型 / 调用失败 ⇒ `None`（调用方照常走繁简规整 + 标点模型，同本机失败兜底）。
fn llm_optimize(
    pre_llm_text: &str,
    raw_text: &str,
    config: &AppConfig,
    o: &TextOpts,
) -> Option<String> {
    if !config.llm.enabled {
        log::info!(
            "[FLASH-P0-510] output optimization requested but server LLM not enabled; skipped"
        );
        return None;
    }
    if pre_llm_text.trim().is_empty() {
        return None;
    }
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            log::warn!("[FLASH-P0-510] remote LLM optimize: runtime failed: {e}");
            return None;
        }
    };
    let client = crate::llm::LlmClient::new(config.llm.clone());
    let script_instruction = text_normalizer::script_instruction(raw_text, o.script);
    let scene = crate::scene::SceneContext::unknown();
    match rt.block_on(client.optimize(
        pre_llm_text,
        script_instruction,
        o.punctuation,
        Some(&scene),
        scene.multiline_safe,
        false,
    )) {
        Ok(r) if !r.text.trim().is_empty() => {
            // 同本机 FMT-LLM-005：大模型输出只做繁简（不动大小写）。
            Some(text_normalizer::normalize_script_only(&r.text, o.script))
        }
        Ok(_) => {
            log::warn!("[FLASH-P0-510] remote LLM optimize returned empty text");
            None
        }
        Err(e) => {
            log::warn!("[FLASH-P0-510] remote LLM optimize failed, falling back: {e}");
            None
        }
    }
}

/// 补标点：已有有效标点（1.7B 原生 / 精解线程补过）⇒ 不动（防 334 `。。` 叠加，同本机
/// `pretranscribed_native_punctuated`）；日韩 ⇒ 日韩专用标点；其余 ⇒ 进程级标点服务
/// （不碰本机录音线程独占的 CT 实例）。失败一律原样。
fn punctuate(model_dir: &Path, text: &str) -> String {
    if punctuation::has_effective_punctuation(text) {
        return text.to_string();
    }
    if punctuation::ct_unsupported_script(text) {
        return punctuation::jako::punctuate(text).unwrap_or_else(|| text.to_string());
    }
    crate::punctuate_via_service(model_dir, true, text)
}

/// 终稿译文（方向按内容自动判定，DEC-082）。都失败 ⇒ `None`（不给译文，原文照常）。
///
/// FLASH-P0-513（Gavin 10-29「如果开了输出优化，那翻译不是应该是用输出优化的l l m来做吗？」）：
/// 引擎由**该终端的输出优化开关**决定，与本机输入法同一规则（本机「启用输出优化」即 `llm.enabled`）——
/// `via_llm`（终端开了输出优化）⇒ 服务端统一大模型**一次**做「优化 + 翻译」（失败 / 服务端未配 ⇒ 退离线）；
/// 否则 ⇒ 离线 NLLB。开翻译时原文不再单独过大模型（调用方关掉原文的输出优化），每段最多一次大模型调用。
pub(crate) fn translate(
    text: &str,
    config: &AppConfig,
    o: &TextOpts,
    via_llm: bool,
) -> Option<TranslationPayload> {
    if text.trim().is_empty() {
        return None;
    }
    let target = translation::derive_translation_target(text);
    let mut out = None;
    if via_llm
        && crate::should_try_llm_translate(config.llm.enabled, config.llm.connectivity_verified)
    {
        out = llm_translate(text, target, config, o);
    }
    if out.is_none() {
        out = nllb_translate(text, target, &o.model_dir);
    }
    let Some(t) = out else {
        log::warn!("[ASR-SERVER-491] remote translation failed; result sent without translation");
        return None;
    };
    // 与本机翻译分支同一收尾：多行保留（远程不知道落点窗口，按多行安全）⇒ 标点收尾。
    let t = punctuation::apply_l2_postprocess(t.trim(), o.punctuation, false).0;
    if t.trim().is_empty() {
        return None;
    }
    let lang = match target {
        TranslationLanguage::English => "en",
        TranslationLanguage::Chinese => "zh",
    };
    Some(TranslationPayload {
        lang: lang.to_string(),
        text: t,
    })
}

/// 大模型翻译：场景未知、多行安全；词库建议不学（远程会话不写本机词库）。
fn llm_translate(
    text: &str,
    target: TranslationLanguage,
    config: &AppConfig,
    o: &TextOpts,
) -> Option<String> {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            log::warn!("[ASR-SERVER-491] remote LLM translate: runtime failed: {e}");
            return None;
        }
    };
    let client = crate::llm::LlmClient::new(config.llm.clone());
    let script_instruction =
        text_normalizer::script_instruction_for_translate(text, config.audio.chinese_script);
    let scene = crate::scene::SceneContext::unknown();
    match rt.block_on(client.optimize_and_translate(
        text,
        target,
        script_instruction,
        o.punctuation,
        Some(&scene),
        false,
        true,
    )) {
        Ok(r) if !r.text.trim().is_empty() => Some(r.text),
        Ok(_) => {
            log::warn!("[ASR-SERVER-491] remote LLM translate returned empty text");
            None
        }
        Err(e) => {
            log::warn!("[ASR-SERVER-491] remote LLM translate failed, trying offline: {e}");
            None
        }
    }
}

/// 离线翻译请求：原文、方向、回信。
type NllbReq = (
    String,
    TranslationLanguage,
    crossbeam_channel::Sender<Option<String>>,
);

/// 进程级远程翻译服务（NLLB 是线程本地缓存、CT2 线程安全未核实 ⇒ 固定一个线程串行处理）。
/// 首次用到才起线程、首个请求才加载模型（约 600MB，与本机 worker 那份各占一份）。
static NLLB_SERVICE: OnceLock<Mutex<Option<crossbeam_channel::Sender<NllbReq>>>> = OnceLock::new();

/// 单次离线翻译最长等待（含首次加载模型）。超时 ⇒ 不给译文。
const NLLB_TIMEOUT: Duration = Duration::from_secs(60);

fn nllb_translate(text: &str, target: TranslationLanguage, model_dir: &Path) -> Option<String> {
    let cell = NLLB_SERVICE.get_or_init(|| Mutex::new(None));
    let (reply_tx, reply_rx) = crossbeam_channel::bounded(1);
    {
        let mut g = cell.lock().unwrap_or_else(|e| e.into_inner());
        let tx = g.get_or_insert_with(|| spawn_nllb_service(model_dir.to_path_buf()));
        if tx.send((text.to_string(), target, reply_tx)).is_err() {
            // 线程已退出 ⇒ 下次重建。
            *g = None;
            log::warn!("[ASR-SERVER-491] remote NLLB service gone; will restart on next request");
            return None;
        }
    }
    match reply_rx.recv_timeout(NLLB_TIMEOUT) {
        Ok(r) => r,
        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
            log::warn!("[ASR-SERVER-491] remote NLLB translate timed out");
            None
        }
        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
            *cell.lock().unwrap_or_else(|e| e.into_inner()) = None;
            log::warn!("[ASR-SERVER-491] remote NLLB service died during request");
            None
        }
    }
}

fn spawn_nllb_service(model_dir: PathBuf) -> crossbeam_channel::Sender<NllbReq> {
    let (tx, rx) = crossbeam_channel::unbounded::<NllbReq>();
    let spawned = std::thread::Builder::new()
        .name("remote-nllb".into())
        .spawn(move || {
            let mut cached: Option<(TranslationLanguage, translation::TranslationEngine)> = None;
            for (text, target, reply) in rx.iter() {
                let out = crate::try_nllb_translate(
                    &text,
                    translation::ensure_translation_direction(&mut cached, &model_dir, target),
                );
                let _ = reply.send(out);
            }
        });
    if let Err(e) = spawned {
        // 起不来 ⇒ 接收端随闭包一起丢掉 ⇒ 调用方 send 失败 ⇒ 不给译文。
        log::warn!("[ASR-SERVER-491] remote NLLB service spawn failed: {e}");
    }
    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::char_times::TsSource;

    fn opts(itn: bool, punctuation: bool) -> TextOpts {
        TextOpts {
            itn,
            punctuation,
            script: ChineseScript::Simplified,
            // 不存在的路径：本组测试不得走到加载标点 / 翻译模型。
            model_dir: std::env::temp_dir().join("feiyin-text-stage-test-no-such-models"),
            strip_trailing: false,
            optimize: false,
        }
    }

    /// 服务端配置：大模型显式关（输出优化请求一律跳过，不发网络请求；缺省配置本身是开的）。
    fn cfg() -> AppConfig {
        let mut c = AppConfig::default();
        c.llm.enabled = false;
        c
    }

    /// 每字一个时间（等宽 100ms），用于校验「时间与字数一一对应」。
    fn times_for(text: &str) -> Times {
        text.chars()
            .enumerate()
            .map(|(i, _)| {
                Some(CharTime {
                    begin: i as f32 * 0.1,
                    end: (i + 1) as f32 * 0.1,
                    src: TsSource::Aligned,
                })
            })
            .collect()
    }

    fn assert_aligned(text: &str, times: &Times) {
        assert_eq!(
            times.len(),
            text.chars().count(),
            "时间与字数必须一一对应：{text:?}"
        );
    }

    const NUM: &str = "重量三点五公斤";

    #[test]
    fn itn_switch_preview_and_final() {
        // 先确认 itn 本身会改这句，否则下面的断言没有意义。
        let norm = itn::normalize_numbers(NUM);
        assert_ne!(norm, NUM, "前提：itn 会改写 {NUM:?}");

        let t = times_for(NUM);
        let (on, on_t) = apply_preview(NUM, &t, &opts(true, true));
        assert_eq!(on, norm);
        assert_aligned(&on, &on_t);
        let (off, off_t) = apply_preview(NUM, &t, &opts(false, true));
        assert_eq!(off, NUM, "itn 关 ⇒ 不规整");
        assert_aligned(&off, &off_t);

        // 终稿：用已带标点的文本避开补标点模型。
        let text = format!("{NUM}。");
        let t = times_for(&text);
        let (on, on_t) = apply_final(&text, &t, &opts(true, true), &cfg());
        assert!(on.contains(&norm), "{on:?}");
        assert_aligned(&on, &on_t);
        let (off, off_t) = apply_final(&text, &t, &opts(false, true), &cfg());
        assert!(off.contains("三点五"), "{off:?}");
        assert_aligned(&off, &off_t);
    }

    #[test]
    fn punctuation_off_strips_all_punctuation() {
        let text = "今天天气很好，我们出去玩吧。";
        let t = times_for(text);
        let (p, pt) = apply_preview(text, &t, &opts(true, false));
        assert!(!p.contains('，') && !p.contains('。'), "{p:?}");
        assert_aligned(&p, &pt);
        let (f, ft) = apply_final(text, &t, &opts(true, false), &cfg());
        assert!(!f.contains('，') && !f.contains('。'), "{f:?}");
        assert!(f.contains("今天天气很好"), "{f:?}");
        assert_aligned(&f, &ft);
    }

    #[test]
    fn preview_with_punctuation_on_keeps_marks() {
        let text = "今天天气很好，我们出去玩吧。";
        let t = times_for(text);
        let (p, pt) = apply_preview(text, &t, &opts(true, true));
        assert!(p.contains('，') && p.contains('。'), "{p:?}");
        assert_aligned(&p, &pt);
    }

    /// FLASH-P0-508（DEC-112）：开标点 ⇒ 终稿先剥光已有标点、再交标点模型整段重打
    ///（同本机 `strip_punctuation_node`）。不叠加 `。。`；结果 == 「剥光 ⇒ 补标点」手工串联。
    #[test]
    fn final_strips_then_repunctuates_like_local() {
        let text = "今天天气很好，我们出去玩吧。";
        let t = times_for(text);
        let (f, ft) = apply_final(text, &t, &opts(true, true), &cfg());
        let stripped = punctuation::strip_effective_punctuation(text);
        assert!(
            !punctuation::has_effective_punctuation(&stripped),
            "前提：剥光"
        );
        let o = opts(true, true);
        let expected =
            punctuation::apply_l2_postprocess(&punctuate(&o.model_dir, &stripped), true, false).0;
        assert_eq!(f, expected);
        assert!(!f.contains("。。") && !f.contains("，。"), "{f:?}");
        assert_aligned(&f, &ft);
    }

    /// FLASH-P0-508：`strip_trailing` ⇒ 去句末标点（同本机快捷方式开关）；不影响句中标点。
    #[test]
    fn strip_trailing_param_strips_final_mark() {
        let text = "今天天气很好，我们出去玩吧。";
        let mut o = opts(true, false);
        o.strip_trailing = true;
        // 标点关 ⇒ 全剥（strip_trailing 无从生效，与本机 L2 同）。
        let (f, _) = apply_final(text, &times_for(text), &o, &cfg());
        assert!(!punctuation::has_effective_punctuation(&f), "{f:?}");
        // 标点开：L2 与本机同参 ⇒ 结果 == apply_l2_postprocess(_, true, true)。
        let mut o = opts(true, true);
        o.strip_trailing = true;
        let (f, ft) = apply_final(text, &times_for(text), &o, &cfg());
        let stripped = punctuation::strip_effective_punctuation(text);
        let expected =
            punctuation::apply_l2_postprocess(&punctuate(&o.model_dir, &stripped), true, true).0;
        assert_eq!(f, expected);
        assert_aligned(&f, &ft);
    }

    /// FLASH-P0-510：服务端没开大模型 ⇒ 输出优化请求被跳过，终稿与不请求时逐位相同。
    #[test]
    fn optimize_without_server_llm_is_noop() {
        for text in [
            NUM,
            "今天天气很好，我们出去玩吧。",
            "他很满头大汉。",
            "好的。",
        ] {
            for punc in [true, false] {
                let base = opts(true, punc);
                let mut opt = opts(true, punc);
                opt.optimize = true;
                let t = times_for(text);
                assert_eq!(
                    apply_final(text, &t, &base, &cfg()),
                    apply_final(text, &t, &opt, &cfg()),
                    "{text:?} punc={punc}"
                );
            }
        }
    }

    /// FLASH-P0-508：单位补丁节点在线（数字规整补丁通道，DEC-036），且只在数字规整开时跑。
    #[test]
    fn unit_patch_node_follows_itn_switch() {
        let src = include_str!("text_stage.rs");
        let prod = crate::guard_prod_lines::prod_lines_excluding_cfg_test(src);
        let call = concat!("itn::", "normalize_unit_symbols_only");
        let idx: Vec<usize> = prod
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains(call))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(idx.len(), 1, "{call} 应恰 1 处");
        let before = prod[idx[0].saturating_sub(2)..idx[0]].join(
            "
",
        );
        assert!(before.contains("if o.itn"), "{call} 必须在 `if o.itn` 内");
    }

    #[test]
    fn filler_only_final_is_empty() {
        for text in ["嗯嗯啊", "嗯。", "   "] {
            let t = times_for(text);
            let (f, ft) = apply_final(text, &t, &opts(true, true), &cfg());
            if text_normalizer::is_effective_text(text) {
                continue; // 词表若认为有效，不属于本断言范围
            }
            assert!(f.is_empty(), "{text:?} ⇒ {f:?}");
            assert!(ft.is_empty());
        }
        let (f, _) = apply_final("嗯嗯啊", &times_for("嗯嗯啊"), &opts(true, true), &cfg());
        assert!(f.is_empty(), "只有口水词 ⇒ 终稿为空");
    }

    /// 硬不变量：任何组合下，输出时间条数 == 输出字数。
    #[test]
    fn times_always_match_char_count() {
        let samples = [
            NUM,
            "今天天气很好，我们出去玩吧。",
            "温度二十五度，湿度百分之五十。",
            "好的。",
            "",
        ];
        for text in samples {
            for itn_on in [true, false] {
                let t = times_for(text);
                let (p, pt) = apply_preview(text, &t, &opts(itn_on, false));
                assert_aligned(&p, &pt);
                let (p, pt) = apply_preview(text, &t, &opts(itn_on, true));
                assert_aligned(&p, &pt);
                // 终稿只用带标点或标点关的组合，避免触发补标点模型。
                let (f, ft) = apply_final(text, &t, &opts(itn_on, false), &cfg());
                assert_aligned(&f, &ft);
                if punctuation::has_effective_punctuation(text) {
                    let (f, ft) = apply_final(text, &t, &opts(itn_on, true), &cfg());
                    assert_aligned(&f, &ft);
                }
            }
        }
    }

    fn out_len(text: &str, o: &TextOpts) -> usize {
        apply_preview(text, &times_for(text), o).0.chars().count()
    }

    #[test]
    fn preview_tentative_zero_is_zero() {
        for text in [NUM, "我有一百二十", "今天天气很好", ""] {
            for itn_on in [true, false] {
                assert_eq!(preview_tentative(text, 0, &opts(itn_on, true)), 0);
            }
        }
    }

    #[test]
    fn preview_tentative_unchanged_text_keeps_value() {
        let text = "今天天气很好我们出去玩";
        let o = opts(true, true);
        assert_eq!(
            apply_preview(text, &times_for(text), &o).0,
            text,
            "前提：文字不变"
        );
        for tent in 1..=text.chars().count() {
            assert_eq!(preview_tentative(text, tent, &o), tent, "tentative={tent}");
        }
    }

    #[test]
    fn preview_tentative_number_in_stable_zone() {
        let text = "一百二十个苹果";
        let o = opts(true, true);
        assert_eq!(
            itn::normalize_numbers(text),
            "120个苹果",
            "前提：itn 把数字规整成 120"
        );
        // 末尾「苹果」未稳定，「一百二十个」稳定 ⇒ 处理后仍是「苹果」2 字。
        assert_eq!(preview_tentative(text, 2, &o), 2);
    }

    #[test]
    fn preview_tentative_number_fully_unstable() {
        let text = "我有一百二十";
        let o = opts(true, true);
        assert_eq!(
            itn::normalize_numbers(text),
            "我有120",
            "前提：itn 把数字规整成 120"
        );
        // 「一百二十」4 字全部未稳定 ⇒ 处理后「120」3 字全部未稳定。
        assert_eq!(preview_tentative(text, 4, &o), 3);
    }

    #[test]
    fn preview_tentative_number_across_boundary() {
        let text = "我有一百二十";
        let o = opts(true, true);
        let out = out_len(text, &o); // 「我有120」= 5
        let r = preview_tentative(text, 2, &o);
        // 「一百二十」⇒「120」按比例分摊：未稳的「二十」对应「20」⇒ 2（out = 5，「我有120」）。
        assert_eq!(r, 2, "out={out}");
    }

    #[test]
    fn preview_tentative_never_exceeds_output_len() {
        let samples = [
            NUM,
            "一百二十个苹果",
            "我有一百二十",
            "今天天气很好，我们出去玩吧。",
            "温度二十五度，湿度百分之五十。",
        ];
        for text in samples {
            let n = text.chars().count();
            for itn_on in [true, false] {
                for punc_on in [true, false] {
                    let o = opts(itn_on, punc_on);
                    let out = out_len(text, &o);
                    for tent in 0..=n + 2 {
                        let r = preview_tentative(text, tent, &o);
                        assert!(
                            r <= out,
                            "{text:?} itn={itn_on} punc={punc_on} tent={tent}: {r} > {out}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn translate_empty_text_is_none() {
        let cfg = AppConfig::default();
        let o = opts(true, true);
        for via_llm in [true, false] {
            assert!(translate("", &cfg, &o, via_llm).is_none());
            assert!(translate("   \n", &cfg, &o, via_llm).is_none());
        }
    }

    // ─── TEST-SYNC-491 / S10 护栏 1-3 ──────────────────────────────────────

    /// S10 护栏 1：`apply_final` 与「本机同一批函数逐位串联」零变更。
    /// 5 组样例 × (itn on/off)，每组断言 `apply_final(text).0` == 手工串联结果。
    /// FLASH-P0-508 起手工链与本机本地实时同序（见 `manual_final`）；补标点在手工链里调同一个 `punctuate`。
    #[test]
    fn s10_final_matches_manual_node_chain() {
        use crate::transcription::char_times::transfer as ct_transfer;

        // FLASH-P0-508：本机本地实时同序（剥光 ⇒ 同音 ⇒ 数字规整 ⇒ 繁简 ⇒ 单位补丁 ⇒ 补标点 ⇒ L2）。
        fn manual_final(text: &str, o: &TextOpts) -> String {
            let t0 = if o.punctuation {
                punctuation::strip_effective_punctuation(text)
            } else {
                text.to_string()
            };
            let t1 = homophone::apply_homophone_fix(t0, true);
            let t2 = if o.itn {
                itn::normalize_numbers(&t1)
            } else {
                t1
            };
            let t3 = text_normalizer::normalize_text_for_language(&t2, o.script);
            let t4 = if o.itn {
                itn::normalize_unit_symbols_only(&t3)
            } else {
                t3
            };
            let t5 = if o.punctuation {
                punctuate(&o.model_dir, &t4)
            } else {
                t4
            };
            punctuation::apply_l2_postprocess(&t5, o.punctuation, o.strip_trailing).0
        }

        let samples = [
            "重量三点五公斤。",               // 中文数字 → ITN
            "他很满头大汉。",                 // 同音纠错：满头大汉 → 满头大汗
            "我用Python写代码，很高效。",     // 混排 CN/EN
            "今天天气很好，我们出去玩吧。",   // 长句，带，和。
            "温度二十五度，湿度百分之五十。", // 数字 + 多标点
        ];
        for text in samples {
            for itn_on in [true, false] {
                for punc_on in [true, false] {
                    let o = opts(itn_on, punc_on);
                    let times = times_for(text);
                    let (actual, actual_times) = apply_final(text, &times, &o, &cfg());
                    let expected = manual_final(text, &o);
                    assert_eq!(
                        actual, expected,
                        "apply_final != manual chain: text={text:?} itn={itn_on} punc={punc_on}"
                    );
                    // 时间长度与输出文字对齐（零变更时 transfer 不搬）。
                    let expected_times: Times = if actual == text {
                        times.to_vec()
                    } else {
                        ct_transfer(text, &times, &expected)
                    };
                    assert_eq!(
                        actual_times.len(),
                        actual.chars().count(),
                        "times len mismatch: text={text:?} itn={itn_on} punc={punc_on}"
                    );
                    // 只对 itn=false 且 punc=true 且未触发同音/繁简时逐项比对时间。
                    if itn_on == false && punc_on == true && actual == text {
                        assert_eq!(actual_times, expected_times);
                    }
                }
            }
        }
    }

    /// S10 护栏 2（源码）：`text_stage.rs` 生产区不得引用 `apply_filler_strip`；
    /// `apply_homophone_fix` 的第二参（enabled）必须恒为 `true`（内置固定开）。
    #[test]
    fn s10_text_stage_prod_no_filler_strip_and_homophone_always_true() {
        let src = include_str!("text_stage.rs");
        let prod = crate::guard_prod_lines::prod_lines_excluding_cfg_test(src);

        // 1. 生产区不得出现 apply_filler_strip（锚点用 concat! 拆防自匹配假绿）。
        let filler = concat!("apply_", "filler", "_strip");
        assert!(
            !prod.iter().any(|l| l.contains(filler)),
            "text_stage.rs 生产区不得引用 {filler}"
        );

        // 2. apply_homophone_fix 第二参恒 true：所有调用行包含 apply_homophone_fix，
        //    且该行（去空白）包含 ", true)" 或 "true"。
        let call = concat!("apply_", "homophone", "_fix");
        let calls: Vec<&String> = prod.iter().filter(|l| l.contains(call)).collect();
        assert!(!calls.is_empty(), "生产区应有 {call} 调用");
        for line in calls {
            let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            // 期望形如 apply_homophone_fix(t.to_string(), true)
            assert!(
                compact.contains(",true)") || compact.contains("true"),
                "{call} 第二参必须恒 true，实际行: {line}"
            );
            assert!(!compact.contains(",false)"), "{call} 不得传 false: {line}");
        }
    }

    /// S10 护栏 3：`apply_final` 输出行为。
    /// (a) `punctuation=false` ⇒ 输出无有效标点；
    /// (b) 短句（≤5 字）带 `punctuation=true` ⇒ 句末标点被剥（L2 短句规则）。
    #[test]
    fn s10_apply_final_punctuation_behavior() {
        // (a) punc=false 全剥标点。
        for text in ["你好，世界。", "OK，好的。"] {
            let o = opts(true, false);
            let times = times_for(text);
            let (out, _) = apply_final(text, &times, &o, &cfg());
            assert!(
                !punctuation::has_effective_punctuation(&out),
                "punc=false 仍含有效标点: text={text:?} out={out:?}"
            );
        }
        // (b) 短句 punc=true ⇒ 句末标点剥掉。
        let short = "好的。";
        assert_eq!(punctuation::count_units(short), 2, "好的。 应为 2 个单位");
        let o = opts(true, true);
        let times = times_for(short);
        let (out, _) = apply_final(short, &times, &o, &cfg());
        assert_eq!(out, "好的", "短句 punc=true 应剥句末标点: {out:?}");
    }
}
