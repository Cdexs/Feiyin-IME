use std::{
    cell::RefCell,
    ffi::{c_char, c_int, c_long, c_void, CStr, CString, NulError},
    path::{Path, PathBuf},
    ptr::{self, NonNull},
};

use anyhow::{anyhow, Context, Result};
use ctranslate2::{ComputeType, Device, TranslationOptions, TranslatorConfig};
use ctranslate2_sys::{
    free_pointer_array, translation_result_free, translation_result_output_at,
    translation_result_output_size, translation_result_score, translator_create,
    translator_destroy, translator_translate_batch_with_target_prefix, CTranslationOptions,
    CTranslationResult, CTranslator,
};
use sentencepiece::SentencePieceProcessor;

use crate::config::TranslationLanguage;
use crate::text_normalizer::contains_han;

// ===========================================================================
// TRANS-NLLB-AND-SENTENCE-BATCH-394：离线翻译改用 NLLB-200-distilled-600M（CT2 int8）。
//   Gavin 2026-09-23：逐句训练的 Marian 长段易「精简 / 偏离」⇒ 换 NLLB + 逐句批量 + 漏译守卫。
//   🔴 不做对比、不留 opus-mt 回退（代码中不再加载 opus-mt）；opus-mt 目录保留（删除须 Gavin 确认）。
// ===========================================================================

/// NLLB 模型子目录（下载落点；`<exe_dir>/models/<本名>`）。
const NLLB_SUBDIR: &str = "nllb-200-distilled-600M-ct2-int8";
/// NLLB CT2 int8 权重来源（HuggingFace 仓库；手工挑选 model.bin≈594MB 的 int8 导出）。
const NLLB_CT2_BASE_URL: &str =
    "https://huggingface.co/mijuanlo/nllb-200-distilled-600M-ct2-int8/resolve/main";

// NLLB 运行期文件（CT2 加载 + 我们的 SentencePiece 分词）。
const CONFIG_JSON: &str = "config.json";
const MODEL_BIN: &str = "model.bin";
const SHARED_VOCABULARY_JSON: &str = "shared_vocabulary.json";
const SENTENCEPIECE_MODEL: &str = "sentencepiece.bpe.model";
const EOS_TOKEN: &str = "</s>";

// NLLB Flores-200 语种码（中=zho_Hans，英=eng_Latn）。
const LANG_ZH: &str = "zho_Hans";
const LANG_EN: &str = "eng_Latn";

// ---- 解码参数（TUNE-394）---------------------------------------------------
/// beam 4（官方示例默认 2；4 提升质量，边际收益已足够，避免 6 的额外耗时）。
const BEAM_SIZE: usize = 4;
/// 长度惩罚 1.0（NLLB 官方默认；>1 会主动拉长、正是旧版「离散 / 编造」的来源之一）。
const LENGTH_PENALTY: f32 = 1.0;
/// 重复 3-gram 抑制（防 NLLB 复读；NLLB 官方生成配置为 3）。
const NO_REPEAT_NGRAM_SIZE: usize = 3;
/// 重复惩罚 1.1（温和抑制复读）。
const REPETITION_PENALTY: f32 = 1.1;
/// 单句重译参数（疑似漏译时）：更强 beam + 略长惩罚，提升召回。
const RETRY_BEAM_SIZE: usize = 6;
const RETRY_LENGTH_PENALTY: f32 = 1.2;
/// 单句 `max_decoding_length` ≈ 源 token 数 × 本系数 + 本余量，上限 [`MAX_DECODE_STEPS`]。
const MAX_NEW_TOKENS_FACTOR: usize = 2;
const MAX_NEW_TOKENS_PAD: usize = 16;
/// 生成硬上限（防单句跑飞；旧值 512 过大）。
const MAX_DECODE_STEPS: usize = 256;
/// 单次 batch 的最大句数（C 接口一次喂入；超出分期）。
const BATCH_MAX_SENTENCES: usize = 64;
/// CTranslate2 `batch_type`：0 = examples。
const BATCH_TYPE_EXAMPLES: c_int = 0;

// ---- 漏译守卫阈值（TUNE-394，Gavin 硬要求：长文本不许被精简）----------------
/// 中→英：英文词数 < 中文有效字数 × 本比例 ⇒ 疑似漏译（0.35；中文 1 字通常译 1 词左右，
/// 正常缩写（数字/专名）最多减半，低于 35% 基本可判丢内容）。
const OMIT_EN_PER_ZH: f32 = 0.35;
/// 英→中：中文有效字数 < 英文词数 × 本比例 ⇒ 疑似漏译（0.6；英文 1 词通常译 1~2 汉字，
/// 「词→字」正常可低至 1:1 以下，取 0.6 作下界，宁少误报）。
const OMIT_ZH_PER_EN: f32 = 0.6;

// ---- 分句（TUNE-394）-------------------------------------------------------
/// 中文单句超过此字数 ⇒ 再按逗号/顿号切子句（~80 字以内 Marian/NLLB 单句最稳）。
const ZH_MAX_SENT_CHARS: usize = 80;
/// 英文单句超过此词数 ⇒ 再按逗号/分号切子句。
const EN_MAX_SENT_WORDS: usize = 60;
/// 子句下限：中文 ≥10 字，短于则并入相邻子句（避免碎片化破坏语义）。
const ZH_MIN_CLAUSE_CHARS: usize = 10;
/// 子句下限：英文 ≥6 词。
const EN_MIN_CLAUSE_WORDS: usize = 6;

/// 中文句末终止符（分句用；`，、` 不是句子边界，只在过长时切子句）。
const ZH_SENT_TERMINATORS: [char; 5] = ['。', '！', '？', '；', '…'];
/// 英文句末终止符。
const EN_SENT_TERMINATORS: [char; 3] = ['.', '?', '!'];
/// 英文不当作句末的常见缩写词（小写、不含点）。
/// TRANS-394-REWORK R3：删除 `am` / `pm`（`a.m.` / `p.m.` 已由「含点」规则覆盖；
/// 保留反而让 `I am. You are.` 不断句）；`no` 保留但另见 [`ends_with_abbreviation`] 的数字后置条件。
const EN_ABBREVIATIONS: [&str; 19] = [
    "mr", "mrs", "ms", "dr", "st", "prof", "sr", "jr", "vs", "etc", "no", "vol", "fig", "inc",
    "ltd", "co", "dept", "univ", "approx",
];

/// TRANS-BIDIR-001 / REFACTOR-DERIVE-TARGET-001: Derive translation target direction
/// from content. Platform-neutral business logic (no cfg gating) — macOS side
/// can reuse directly, zero drift (DEC-033).
///
/// - Contains Han characters (incl. Japanese kanji) → translate to English
/// - Other (English, pure digits, etc.) → translate to Chinese
///
/// Known semantic boundary (documented, not solved here):
/// - Japanese kanji triggers `contains_han` → judged as Chinese → translated to
///   English. Better than the old "silently skip" but imperfect; LANG-MIXED-001
///   kana/hangul probes exist for future refinement.
/// - Pure digits/punctuation with no Han and no Latin → judged Chinese direction,
///   harmless no-op (no content to translate).
pub fn derive_translation_target(text: &str) -> TranslationLanguage {
    if contains_han(text) {
        TranslationLanguage::English
    } else {
        TranslationLanguage::Chinese
    }
}

/// NLLB 模型（**一个模型服务中↔英双向**；方向由调用方按 `TranslationLanguage` 指定）。
struct NllbModel {
    path: PathBuf,
    translator: Ct2Translator,
    tokenizer: NllbTokenizer,
}

/// TRANS-394-REWORK R1-b：引擎持有**进程级** `&'static NllbModel`（由 [`shared_model`] 提供）。
/// 双向共享同一已加载模型；切换方向只换 `direction`、不重载 594MB 权重，
/// 且**任何 drop 都不会卸载模型**（`translator_destroy` 永不触发，规避 CT2 销毁死锁）。
/// 内存取舍：全程仅驻留一份 NLLB（int8 ≈ 600MB），关闭翻译后亦常驻，直到进程退出。
pub struct TranslationEngine {
    model: &'static NllbModel,
    direction: TranslationLanguage,
}

// TRANS-394-REWORK R1-b：**进程级 NLLB 模型** —— 每个模型目录至多加载一次，且**永不析构**
//（`Box::leak`；不调用 `translator_destroy`，规避 CT2 销毁死锁，见 `collab/troubleshooting.md`）。
//
// 方案选择：`thread_local!` + `Box::leak`（**未**采用 `static Mutex<Arc<NllbModel>>`）。理由：
// 1. 生产侧翻译只在**单个 worker 线程**串行执行（`main.rs` 的 `cached_translation` 是 worker 闭包内
//    局部变量，全部调用顺序发生）⇒ thread-local 缓存恰好对应「一个 worker 一份模型」，无需跨线程共享；
// 2. 因而不需要 `unsafe impl Send/Sync for NllbModel`（只有把 `&'static NllbModel` 放进 `static`
//    或 `Arc` 跨线程共享时，才要求 `NllbModel: Sync`），也就不必依赖尚未核实的 CT2 C 封装线程安全声明；
// 3. 代价：非 worker 线程（仅测试路径）会各自独立加载；本工程生产路径不存在多线程翻译。
thread_local! {
    static NLLB_MODEL: RefCell<Option<(PathBuf, &'static NllbModel)>> = RefCell::new(None);
}

/// 取进程级 NLLB 模型：命中同路径即复用；否则加载并 `Box::leak` 常驻。
/// 加载失败**不缓存**（文件补齐后可重试）；路径变化（生产不会发生）只打 warn、旧模型也不释放。
fn shared_model(model_dir: &Path) -> Result<&'static NllbModel> {
    let path = model_dir.join(NLLB_SUBDIR);
    NLLB_MODEL.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some((cached_path, model)) = slot.as_ref() {
            if cached_path == &path {
                return Ok(*model);
            }
            log::warn!(
                "NLLB model path changed ({} -> {}); keeping the previously loaded model resident \
                 (process-level models are never destroyed)",
                cached_path.display(),
                path.display()
            );
        }
        let model: &'static NllbModel = Box::leak(Box::new(NllbModel::new(model_dir)?));
        log::info!("NLLB model loaded once and cached for the process lifetime");
        *slot = Some((path, model));
        Ok(model)
    })
}

/// NLLB 使用**单个** SentencePiece 模型（共享词表，非 source/target 两份）。
struct NllbTokenizer {
    sp: SentencePieceProcessor,
}

struct Ct2Translator {
    inner: NonNull<CTranslator>,
}

struct OwnedTranslationResult {
    inner: *mut CTranslationResult,
}

#[derive(Debug)]
enum Ct2TranslatorError {
    NulInPath(NulError),
    CreationFailed,
    NulInToken { token: String, source: NulError },
    NullResults,
}

impl std::fmt::Display for Ct2TranslatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NulInPath(err) => write!(f, "invalid model path (contains null byte): {}", err),
            Self::CreationFailed => write!(f, "failed to create the CT2 translator"),
            Self::NulInToken { token, source } => {
                write!(
                    f,
                    "invalid source token {:?} (contains null byte): {}",
                    token, source
                )
            }
            Self::NullResults => write!(f, "CT2 returned a null results pointer"),
        }
    }
}

impl std::error::Error for Ct2TranslatorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NulInPath(err) => Some(err),
            Self::NulInToken { source, .. } => Some(source),
            Self::CreationFailed | Self::NullResults => None,
        }
    }
}

/// TRANS-394-REWORK：`translator_destroy` 在**已完成推理**的 translator 上会**死锁**
/// （进程 CPU 停止增长、进程无法退出；取证见 `collab/troubleshooting.md` 与 result.md 的 R1-a）。
/// 生产路径下模型由 [`shared_model`] `Box::leak` 常驻 ⇒ 本 `Drop` **永不触发**。
/// 保留实现仅用于「将来若出现非泄漏构造仍能正确释放」；**切勿**在会被 drop 的路径构造 `Ct2Translator`。
impl Drop for Ct2Translator {
    fn drop(&mut self) {
        unsafe {
            translator_destroy(self.inner.as_ptr());
        }
    }
}

impl Drop for OwnedTranslationResult {
    fn drop(&mut self) {
        unsafe {
            translation_result_free(self.inner);
        }
    }
}

impl OwnedTranslationResult {
    fn output(&self) -> Vec<String> {
        unsafe {
            let len = translation_result_output_size(self.inner);
            let mut out = Vec::with_capacity(len);
            for idx in 0..len {
                let ptr = translation_result_output_at(self.inner, idx);
                out.push(CStr::from_ptr(ptr).to_string_lossy().to_string());
            }
            out
        }
    }

    #[allow(dead_code)]
    fn score(&self) -> f32 {
        unsafe { translation_result_score(self.inner) }
    }
}

impl Ct2Translator {
    fn new<P: AsRef<Path>>(
        model_path: P,
        config: &TranslatorConfig,
    ) -> Result<Self, Ct2TranslatorError> {
        let c_model = CString::new(model_path.as_ref().to_string_lossy().into_owned())
            .map_err(Ct2TranslatorError::NulInPath)?;

        let raw = unsafe {
            translator_create(
                c_model.as_ptr(),
                config.device as c_int,
                config.compute_type as c_int,
                config.device_indices.as_ptr(),
                config.device_indices.len(),
                config.tensor_parallel as c_int,
                config.num_threads_per_replica,
                config.max_queued_batches as c_long,
                config.cpu_core_offset as c_int,
            )
        };

        let inner = NonNull::new(raw).ok_or(Ct2TranslatorError::CreationFailed)?;
        Ok(Self { inner })
    }

    /// FIX-394：**批量**翻译（带 per-sentence `target_prefix`），一次 FFI 调用喂入 `sentences`。
    /// `sentences[j]` = 第 j 句的源 token 串（**含** `[src_lang]` 前缀与末尾 `</s>`）；
    /// `target_prefix` = 每句共用的前缀 token（NLLB = `[tgt_lang]`）。
    /// 返回结果数 = `sentences.len()`（每句一条）。
    fn translate_batch_with_prefix(
        &self,
        sentences: &[Vec<String>],
        target_prefix: &[String],
        options: &TranslationOptions,
    ) -> Result<Vec<OwnedTranslationResult>, Ct2TranslatorError> {
        if sentences.is_empty() {
            return Ok(Vec::new());
        }
        // 源 token 的 CString 全部持有到 FFI 返回。
        let sent_cstrings: Vec<Vec<CString>> = sentences
            .iter()
            .map(|row| {
                row.iter()
                    .map(|token| {
                        CString::new(token.as_str()).map_err(|source| {
                            Ct2TranslatorError::NulInToken {
                                token: token.clone(),
                                source,
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let prefix_cstrings: Vec<CString> = target_prefix
            .iter()
            .map(|token| {
                CString::new(token.as_str()).map_err(|source| Ct2TranslatorError::NulInToken {
                    token: token.clone(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // 每句一个以 null 结尾的 token 指针数组。
        let sent_ptrs: Vec<Vec<*const c_char>> = sent_cstrings
            .iter()
            .map(|row| {
                let mut v: Vec<*const c_char> = row.iter().map(|c| c.as_ptr()).collect();
                v.push(ptr::null());
                v
            })
            .collect();
        let mut sources: Vec<*const *const c_char> = sent_ptrs.iter().map(|v| v.as_ptr()).collect();

        // 每句共用的 target prefix（同一前缀数组被 N 句复用）。
        let mut prefix_ptrs: Vec<*const c_char> =
            prefix_cstrings.iter().map(|c| c.as_ptr()).collect();
        prefix_ptrs.push(ptr::null());
        let mut prefixes: Vec<*const *const c_char> =
            (0..sentences.len()).map(|_| prefix_ptrs.as_ptr()).collect();

        let c_options = to_c_translation_options(options);
        let mut out_num_translations = 0usize;
        let n = sentences.len();

        let results_ptr = unsafe {
            // Safety:
            // - 所有 CString 数组（sent_cstrings / prefix_cstrings / sent_ptrs / sources /
            //   prefix_ptrs / prefixes）在本调用期间全部存活。
            // - `sources` / `prefixes` 是 `const char***` 对应的 Rust 视图。
            translator_translate_batch_with_target_prefix(
                self.inner.as_ptr(),
                sources.as_mut_ptr() as *mut *mut *const c_char,
                prefixes.as_mut_ptr() as *mut *mut *const c_char,
                n,
                &c_options,
                n,
                BATCH_TYPE_EXAMPLES,
                &mut out_num_translations,
            )
        };

        if results_ptr.is_null() {
            return Err(Ct2TranslatorError::NullResults);
        }

        Ok(unsafe { take_translation_results(results_ptr, out_num_translations) })
    }
}

unsafe fn take_translation_results(
    results_ptr: *mut *mut CTranslationResult,
    len: usize,
) -> Vec<OwnedTranslationResult> {
    let raw_results = std::slice::from_raw_parts(results_ptr, len).to_vec();
    free_pointer_array(results_ptr as *mut *mut c_void);
    raw_results
        .into_iter()
        .map(|inner| OwnedTranslationResult { inner })
        .collect()
}

fn to_c_translation_options(options: &TranslationOptions) -> CTranslationOptions {
    CTranslationOptions {
        beam_size: options.beam_size,
        patience: options.patience,
        length_penalty: options.length_penalty,
        coverage_penalty: options.coverage_penalty,
        repetition_penalty: options.repetition_penalty,
        no_repeat_ngram_size: options.no_repeat_ngram_size,
        disable_unk: if options.disable_unk { 1 } else { 0 },
        max_input_length: options.max_input_length,
        max_decoding_length: options.max_decoding_length,
        min_decoding_length: options.min_decoding_length,
        sampling_topk: options.sampling_topk,
        return_end_token: options.return_end_token,
        prefix_bias_beta: options.prefix_bias_beta,
        sampling_topp: options.sampling_topp,
        sampling_temperature: options.sampling_temperature,
        use_vmap: if options.use_vmap { 1 } else { 0 },
        num_hypotheses: options.num_hypotheses,
        return_scores: if options.return_scores { 1 } else { 0 },
        return_attention: if options.return_attention { 1 } else { 0 },
        return_logits_vocab: if options.return_logits_vocab { 1 } else { 0 },
        return_alternatives: if options.return_alternatives { 1 } else { 0 },
        min_alternative_expansion_prob: options.min_alternative_expansion_prob,
        replace_unknowns: if options.replace_unknowns { 1 } else { 0 },
    }
}

impl NllbTokenizer {
    fn new(path: &Path) -> Result<Self> {
        let spm_path = path.join(SENTENCEPIECE_MODEL);
        let sp = SentencePieceProcessor::open(&spm_path).map_err(|err| {
            anyhow!(
                "failed to load NLLB sentencepiece model from {}: {}",
                spm_path.display(),
                err
            )
        })?;
        Ok(Self { sp })
    }

    /// 编码为 SentencePiece 词片（**不含** `src_lang` 前缀与 `</s>`；由 [`build_source_tokens`] 组装）。
    fn encode(&self, input: &str) -> Result<Vec<String>> {
        Ok(self
            .sp
            .encode(input)
            .map_err(|err| anyhow!("failed to encode sentencepiece input: {}", err))?
            .into_iter()
            .map(|piece| piece.piece)
            .collect())
    }

    fn decode(&self, tokens: &[String]) -> Result<String> {
        let filtered: Vec<&str> = tokens
            .iter()
            .map(String::as_str)
            .filter(|token| !matches!(*token, "<pad>" | "</s>" | "<s>"))
            .collect();
        self.sp
            .decode_pieces(&filtered)
            .map_err(|err| anyhow!("failed to decode sentencepiece output: {}", err))
    }
}

/// TUNE-394：组装 NLLB 源 token 串 —— `[src_lang] + sp 词片 + ["</s>"]`。
///
/// 依据 CTranslate2 官方 NLLB 指南（`guides/transformers.html` §NLLB）：
/// `source = tokenizer.convert_ids_to_tokens(tokenizer.encode("Hello world!"))`（该 HF 分词器在
/// `src_lang` 下自动加 `src_lang` 前缀与 `</s>`），`target_prefix = [tgt_lang]`，
/// `results[0].hypotheses[0][1:]`（去掉首个 target 语种 token）。
/// 我们不用 HF tokenizers，故手工补这两个特殊 token（见指南「Special tokens in translation」）。
fn build_source_tokens(src_lang: &str, pieces: Vec<String>) -> Vec<String> {
    let mut tokens = Vec::with_capacity(pieces.len() + 2);
    tokens.push(src_lang.to_string());
    tokens.extend(pieces);
    tokens.push(EOS_TOKEN.to_string());
    tokens
}

/// TUNE-394：剥掉译文首个 `tgt_lang` token（NLLB 输出以目标语种 token 开头）。
fn strip_target_prefix(tokens: &[String], tgt_lang: &str) -> Vec<String> {
    match tokens.split_first() {
        Some((head, rest)) if head == tgt_lang => rest.to_vec(),
        _ => tokens.to_vec(),
    }
}

/// TUNE-394：中文「有效字数」= 字母/数字/汉字数（去空白与标点）—— 漏译判据的源/目标计数。
fn zh_effective_chars(text: &str) -> usize {
    text.chars().filter(|c| c.is_alphanumeric()).count()
}

/// TUNE-394：英文「词数」= 含字母/数字的空白分隔 token 数。
fn en_word_count(text: &str) -> usize {
    text.split_whitespace()
        .filter(|w| w.chars().any(|c| c.is_alphanumeric()))
        .count()
}

/// TUNE-394：**漏译检测**（纯函数）—— 译句相对源句过短即疑似被「精简 / 截断」。
///
/// - 中→英（target=English）：英文词数 < 中文有效字数 × [`OMIT_EN_PER_ZH`] ⇒ 真；
/// - 英→中（target=Chinese）：中文有效字数 < 英文词数 × [`OMIT_ZH_PER_EN`] ⇒ 真；
/// - 空结果 ⇒ 真。
fn looks_truncated(src: &str, dst: &str, direction: TranslationLanguage) -> bool {
    if dst.trim().is_empty() {
        return true;
    }
    match direction {
        TranslationLanguage::English => {
            (en_word_count(dst) as f32) < (zh_effective_chars(src) as f32) * OMIT_EN_PER_ZH
        }
        TranslationLanguage::Chinese => {
            (zh_effective_chars(dst) as f32) < (en_word_count(src) as f32) * OMIT_ZH_PER_EN
        }
    }
}

/// TUNE-394：单句可重试收口（纯函数，重译闭包注入 ⇒ 可单测「至多一次」）。
/// 返回 `(最终译文, 重译调用次数, 重译后仍疑似漏译)`：疑似漏译才调 `retry` 一次，取更长者。
fn finalize_sentence(
    src: &str,
    dst: String,
    direction: TranslationLanguage,
    retry: impl FnOnce() -> Option<String>,
) -> (String, usize, bool) {
    let mut best = dst;
    let mut calls = 0usize;
    if looks_truncated(src, &best, direction) {
        calls += 1;
        if let Some(candidate) = retry() {
            if candidate.chars().count() > best.chars().count() {
                best = candidate;
            }
        }
    }
    let still = looks_truncated(src, &best, direction);
    (best, calls, still)
}

/// TUNE-394：按方向拼接译文 —— 中→英用单空格；英→中用**空串**（中文不加空格）。
fn join_parts(parts: &[String], direction: TranslationLanguage) -> String {
    let joined = match direction {
        TranslationLanguage::English => parts.join(" "),
        TranslationLanguage::Chinese => parts.concat(),
    };
    // 只在空格维度归一（保留 `\n`）：避免空译文产生多余空格。
    let mut out = String::with_capacity(joined.len());
    let mut prev_space = false;
    for c in joined.chars() {
        if c == ' ' {
            if !prev_space {
                out.push(c);
            }
            prev_space = true;
        } else {
            prev_space = false;
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// TUNE-394：分句（纯函数）—— 源语言由**目标方向**决定（target=English ⇒ 源=中文；反之源=英文）。
///
/// - 中文：按 `。！？；……` 切（保留标点在句尾）；过长（> [`ZH_MAX_SENT_CHARS`]）再按 `，、` 切子句；
/// - 英文：按 `. ? !`（必须后接空白/结尾）切，避开 `Mr./Dr./e.g./i.e./U.S./小数点`；过长
///   （> [`EN_MAX_SENT_WORDS`] 词）再按 `,;` 切子句；
/// - 子句过短（中文 <10 字 / 英文 <6 词）并入相邻子句；空句丢弃。
///
/// 段落换行由调用方（`translate` 逐行处理）保证，本函数不处理 `\n`。
fn split_sentences(text: &str, direction: TranslationLanguage) -> Vec<String> {
    let source_is_chinese = matches!(direction, TranslationLanguage::English);
    let raw: Vec<String> = if source_is_chinese {
        split_zh_sentences(text)
    } else {
        split_en_sentences(text)
    };
    let mut out: Vec<String> = Vec::new();
    for sentence in raw {
        let sentence = sentence.trim();
        if sentence.is_empty() {
            continue;
        }
        if source_is_chinese && sentence.chars().count() > ZH_MAX_SENT_CHARS {
            out.extend(split_and_merge(
                sentence,
                &['，', '、'],
                ZH_MIN_CLAUSE_CHARS,
                true,
            ));
        } else if !source_is_chinese && en_word_count(sentence) > EN_MAX_SENT_WORDS {
            out.extend(split_and_merge(
                sentence,
                &[',', ';'],
                EN_MIN_CLAUSE_WORDS,
                false,
            ));
        } else {
            out.push(sentence.to_string());
        }
    }
    out
}

fn split_zh_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev = '\0';
    for c in text.chars() {
        cur.push(c);
        let is_break = if c == '…' {
            // `……` 视为一个终止符：只在第二个 `…` 处断。
            prev == '…'
        } else {
            ZH_SENT_TERMINATORS.contains(&c)
        };
        if is_break {
            out.push(std::mem::take(&mut cur));
        }
        prev = c;
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

fn split_en_sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        cur.push(c);
        if !EN_SENT_TERMINATORS.contains(&c) {
            continue;
        }
        let next_is_boundary = chars.get(i + 1).map_or(true, |n| n.is_whitespace());
        // 下一个非空白字符（供 `No. 5` 这类「缩写后接数字」判据使用）。
        let next_nonspace = chars[i + 1..].iter().copied().find(|c| !c.is_whitespace());
        let is_abbrev = c == '.' && ends_with_abbreviation(&cur, next_nonspace);
        if next_is_boundary && !is_abbrev {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// 判断以 `.` 结尾的 `cur` 是否命中英文缩写（`Mr.` / `e.g.` / `U.S.` 等）。
/// `next_nonspace` = 该句点之后第一个非空白字符（无则 `None`），用于 `no` 的后置条件。
fn ends_with_abbreviation(cur: &str, next_nonspace: Option<char>) -> bool {
    // 去掉末尾的 '.'，向前收集 [A-Za-z.] 组成 token。
    let before = &cur[..cur.len() - 1];
    let token: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphabetic() || *c == '.')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if token.is_empty() {
        return false;
    }
    let lower = token.to_ascii_lowercase();
    // 含点 ⇒ 多段缩写（e.g. / i.e. / U.S.）。
    if token.contains('.') {
        return true;
    }
    // 单个大写字母 ⇒ 姓名首字母（U. / J.）。
    if token.len() == 1 && token.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        return true;
    }
    // TRANS-394-REWORK R3：`no` 仅在**下一个非空白字符是数字**时视为缩写（`No. 5`）；
    // 否则「The answer is no. We left.」应在 `no.` 处断句。
    if lower == "no" {
        return next_nonspace.is_some_and(|c| c.is_ascii_digit());
    }
    EN_ABBREVIATIONS.contains(&lower.as_str())
}

/// 按子句分隔符切分，并把过短子句并入相邻子句（保持顺序、不丢内容）。
fn split_and_merge(text: &str, delims: &[char], min: usize, by_chars: bool) -> Vec<String> {
    let mut pieces: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        cur.push(c);
        if delims.contains(&c) {
            pieces.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        pieces.push(cur);
    }

    let len = |s: &String| {
        if by_chars {
            s.chars().count()
        } else {
            en_word_count(s)
        }
    };

    let mut out: Vec<String> = Vec::new();
    for piece in pieces {
        if len(&piece) < min {
            if let Some(last) = out.last_mut() {
                last.push_str(&piece);
                continue;
            }
        }
        out.push(piece);
    }
    // 首个子句过短 ⇒ 并入第二个（若存在）。
    if out.len() > 1 && len(&out[0]) < min {
        let first = out.remove(0);
        out[0] = format!("{first}{}", out[0]);
    }
    out
}

/// TUNE-394：数字 / 专名保留检查（**只记日志、不改结果**）。
fn log_token_carry(sent_idx: usize, src: &str, dst: &str) {
    if !log::log_enabled!(log::Level::Debug) {
        return;
    }
    let mut carried: Vec<String> = Vec::new();
    // 阿拉伯数字串。
    let mut num = String::new();
    for c in src.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else if !num.is_empty() {
            carried.push(std::mem::take(&mut num));
        }
    }
    if !num.is_empty() {
        carried.push(num);
    }
    // 英文大写词 + `%`。
    for w in src.split_whitespace() {
        let core: String = w.chars().filter(|c| c.is_ascii_alphabetic()).collect();
        if core.len() >= 2 && core.chars().all(|c| c.is_ascii_uppercase()) {
            carried.push(core);
        }
    }
    if src.contains('%') {
        carried.push("%".to_string());
    }
    for token in carried {
        if token == "%" {
            if !dst.contains('%') {
                log::debug!("[TRANS-394] token not carried: sent#{sent_idx} %");
            }
        } else if !dst.contains(&token) {
            log::debug!("[TRANS-394] token not carried: sent#{sent_idx} {token}");
        }
    }
}

impl NllbModel {
    fn new(model_dir: &Path) -> Result<Self> {
        let path = model_dir.join(NLLB_SUBDIR);
        validate_runtime_files(&path)?;
        let tokenizer = NllbTokenizer::new(&path)?;

        let config = TranslatorConfig {
            device: Device::Cpu,
            compute_type: ComputeType::Default,
            ..TranslatorConfig::default()
        };
        let translator = Ct2Translator::new(&path, &config)
            .map_err(|err| anyhow!("failed to initialize NLLB CT2 translator: {}", err))?;

        log::info!("NLLB CT2 model initialized at {}", path.display());

        Ok(Self {
            path,
            translator,
            tokenizer,
        })
    }

    /// 逐句批量翻译：一次（或分期）喂入所有句子，返回**与 `sentences` 等长**的译文（逐句一一对应）。
    fn translate_sentences(
        &self,
        sentences: &[String],
        src_lang: &str,
        tgt_lang: &str,
        beam_size: usize,
        length_penalty: f32,
    ) -> Result<Vec<String>> {
        let mut out: Vec<String> = Vec::with_capacity(sentences.len());
        for chunk in sentences.chunks(BATCH_MAX_SENTENCES) {
            let encoded: Vec<Vec<String>> = chunk
                .iter()
                .map(|s| {
                    let pieces = self.tokenizer.encode(s)?;
                    Ok::<_, anyhow::Error>(build_source_tokens(src_lang, pieces))
                })
                .collect::<Result<Vec<_>>>()?;
            let max_src_tokens = encoded.iter().map(Vec::len).max().unwrap_or(0);
            let max_decoding_length =
                (max_src_tokens * MAX_NEW_TOKENS_FACTOR + MAX_NEW_TOKENS_PAD).min(MAX_DECODE_STEPS);
            let options = TranslationOptions {
                beam_size,
                length_penalty,
                coverage_penalty: 0.0,
                repetition_penalty: REPETITION_PENALTY,
                no_repeat_ngram_size: NO_REPEAT_NGRAM_SIZE,
                // TUNE-394：删除 `min_decoding_length = 源/2` 的强制（旧版据此逼模型续写/编造）。
                min_decoding_length: 0,
                max_decoding_length: max_decoding_length.max(1),
                max_input_length: 0,
                ..TranslationOptions::default()
            };
            let results = self
                .translator
                .translate_batch_with_prefix(&encoded, &[tgt_lang.to_string()], &options)
                .map_err(|err| {
                    anyhow!(
                        "failed to translate with NLLB model at {}: {}",
                        self.path.display(),
                        err
                    )
                })?;
            if results.len() != chunk.len() {
                anyhow::bail!(
                    "NLLB returned {} results for {} sentences",
                    results.len(),
                    chunk.len()
                );
            }
            for result in results {
                let tokens = result.output();
                let stripped = strip_target_prefix(&tokens, tgt_lang);
                out.push(
                    self.tokenizer
                        .decode(&stripped)
                        .context("failed to decode NLLB output")?,
                );
            }
        }
        Ok(out)
    }
}

impl TranslationEngine {
    pub fn new(model_dir: &Path, target: TranslationLanguage) -> Result<Self> {
        Ok(Self {
            model: shared_model(model_dir)?,
            direction: target,
        })
    }

    /// TRANS-394-REWORK：复用同一个**进程级** NLLB，仅切换方向（不重载权重、不析构）。
    fn with_direction(&self, target: TranslationLanguage) -> Self {
        Self {
            model: self.model,
            direction: target,
        }
    }

    pub fn translate(&self, text: &str) -> Result<String> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(String::new());
        }
        let (src_lang, tgt_lang) = match self.direction {
            TranslationLanguage::English => (LANG_ZH, LANG_EN),
            TranslationLanguage::Chinese => (LANG_EN, LANG_ZH),
        };

        // 逐「行」（段落）处理 ⇒ 保留原文换行（行内多句批量翻译、行间以 `\n` 还原）。
        let mut out_lines: Vec<String> = Vec::new();
        for line in text.split('\n') {
            if line.trim().is_empty() {
                out_lines.push(String::new());
                continue;
            }
            let sentences = split_sentences(line, self.direction);
            if sentences.is_empty() {
                out_lines.push(String::new());
                continue;
            }
            let base = self.model.translate_sentences(
                &sentences,
                src_lang,
                tgt_lang,
                BEAM_SIZE,
                LENGTH_PENALTY,
            )?;
            // 🔴 392/394：逐句一一对应 —— 每句都产出一条译文，任何一句都不在拼接时被丢弃。
            let mut parts: Vec<String> = Vec::with_capacity(sentences.len());
            for (i, sentence) in sentences.iter().enumerate() {
                let primary = base.get(i).cloned().unwrap_or_default();
                let (best, retries, still) =
                    finalize_sentence(sentence, primary, self.direction, || {
                        self.model
                            .translate_sentences(
                                std::slice::from_ref(sentence),
                                src_lang,
                                tgt_lang,
                                RETRY_BEAM_SIZE,
                                RETRY_LENGTH_PENALTY,
                            )
                            .ok()
                            .and_then(|mut v| v.drain(..).next())
                    });
                debug_assert!(retries <= 1, "重译至多一次");
                if still {
                    log::warn!(
                        "[TRANS-394] possible omission: sent#{} src_len={} dst_len={}",
                        i,
                        sentence.chars().count(),
                        best.chars().count()
                    );
                }
                log_token_carry(i, sentence, &best);
                parts.push(best);
            }
            out_lines.push(join_parts(&parts, self.direction));
        }
        Ok(out_lines.join("\n"))
    }

    /// NLLB 单模型服务两方向 ⇒ 可用性只看单目录（`target` 仅保留签名兼容）。
    pub fn is_available(model_dir: &Path, _target: TranslationLanguage) -> bool {
        let path = model_dir.join(NLLB_SUBDIR);
        minimum_runtime_files()
            .iter()
            .all(|relative| path.join(relative).is_file())
    }

    pub fn model_files() -> Vec<(String, String)> {
        let mut files = Vec::new();
        append_model_files(&mut files, NLLB_SUBDIR, NLLB_CT2_BASE_URL);
        files
    }

    pub fn direction(&self) -> TranslationLanguage {
        self.direction
    }

    /// REFACTOR-SHARE-TRANSDIR-001: Load an engine for the given direction, with
    /// panic-guard and availability check. Platform-neutral (DEC-033). Returns
    /// None if model files missing or load fails (best-effort, non-panicking).
    pub fn load_for_direction(model_dir: &Path, target: TranslationLanguage) -> Option<Self> {
        if !Self::is_available(model_dir, target) {
            log::info!(
                "Translation model files not found for {:?}, offline engine disabled",
                target
            );
            return None;
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::new(model_dir, target)
        }));
        match result {
            Ok(Ok(engine)) => {
                log::info!("Offline translation engine loaded ({:?})", target);
                Some(engine)
            }
            Ok(Err(e)) => {
                log::warn!("Translation engine load failed for {:?}: {}", target, e);
                None
            }
            Err(_) => {
                log::warn!("Translation engine load panicked for {:?}", target);
                None
            }
        }
    }
}

/// TRANS-BIDIR-001 / REFACTOR-SHARE-TRANSDIR-001: Ensure the cached offline
/// translation engine matches the derived target direction.
///
/// TRANS-394-REWORK: NLLB is **one model for both directions** ⇒ direction change reuses the same
/// **process-level** `&'static NllbModel` (no 594MB reload, no destroy). Only when nothing is cached
/// do we load from disk. If load fails, return None (caller injects original text — never drops user
/// speech, never injects garbage from a wrong-direction engine).
///
/// Platform-neutral (DEC-033) — macOS side can reuse directly. All platform
/// specifics (worker thread, model_dir resolution) stay at the caller.
///
/// Concurrency: caller must ensure single-threaded access to cached_translation
/// (the Windows caller does this via worker thread sequential execution).
pub fn ensure_translation_direction<'a>(
    cached_translation: &'a mut Option<(TranslationLanguage, TranslationEngine)>,
    model_dir: &Path,
    derived_target: TranslationLanguage,
) -> Option<&'a TranslationEngine> {
    // 方向一致 ⇒ 直接复用。
    if let Some((lang, _)) = cached_translation.as_ref() {
        if *lang == derived_target {
            return cached_translation.as_ref().map(|(_, e)| e);
        }
    }
    // 有缓存但方向不同 ⇒ 复用同一个进程级 NllbModel，仅换方向（不重载权重、不析构）。
    if let Some((_, engine)) = cached_translation.as_ref() {
        let new_engine = engine.with_direction(derived_target);
        log::info!(
            "Translation direction switched to {:?} (NLLB model reused, no reload)",
            derived_target
        );
        *cached_translation = Some((derived_target, new_engine));
        return cached_translation.as_ref().map(|(_, e)| e);
    }
    // 无缓存 ⇒ 从磁盘加载。
    let t_start = std::time::Instant::now();
    let new_engine = TranslationEngine::load_for_direction(model_dir, derived_target);
    let elapsed = t_start.elapsed();
    match new_engine {
        Some(engine) => {
            log::info!(
                "Translation engine loaded for {:?} in {:.2}s",
                derived_target,
                elapsed.as_secs_f64()
            );
            *cached_translation = Some((derived_target, engine));
            cached_translation.as_ref().map(|(_, e)| e)
        }
        None => {
            log::warn!(
                "Translation engine load failed for {:?} after {:.2}s; \
                 skipping translation (original text will be injected)",
                derived_target,
                elapsed.as_secs_f64()
            );
            None
        }
    }
}

fn append_model_files(files: &mut Vec<(String, String)>, subdir: &str, base_url: &str) {
    for filename in [
        CONFIG_JSON,
        MODEL_BIN,
        SHARED_VOCABULARY_JSON,
        SENTENCEPIECE_MODEL,
    ] {
        files.push((
            format!("{subdir}/{filename}"),
            format!("{base_url}/{filename}"),
        ));
    }
}

fn minimum_runtime_files() -> [&'static str; 2] {
    [MODEL_BIN, SENTENCEPIECE_MODEL]
}

fn required_runtime_files() -> [&'static str; 4] {
    [
        CONFIG_JSON,
        MODEL_BIN,
        SHARED_VOCABULARY_JSON,
        SENTENCEPIECE_MODEL,
    ]
}

fn validate_runtime_files(path: &Path) -> Result<()> {
    for relative in required_runtime_files() {
        let file = path.join(relative);
        if !file.is_file() {
            return Err(anyhow!("missing NLLB runtime file: {}", file.display()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let counter = UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!("voice-ime-{prefix}-{timestamp}-{counter}"))
    }

    fn write_placeholder_files(model_dir: &Path, subdir: &str, files: &[&str]) {
        let base = model_dir.join(subdir);
        fs::create_dir_all(&base).expect("create model base directory");
        for relative in files {
            let file = base.join(relative);
            fs::write(file, b"placeholder").expect("write placeholder file");
        }
    }

    // ---- 方向判定（沿用，未改） ----

    #[test]
    fn derive_target_chinese_text_returns_english() {
        assert_eq!(
            derive_translation_target("你好世界"),
            TranslationLanguage::English
        );
        assert_eq!(
            derive_translation_target("温度是四十摄氏度"),
            TranslationLanguage::English
        );
    }

    #[test]
    fn derive_target_english_text_returns_chinese() {
        assert_eq!(
            derive_translation_target("hello world"),
            TranslationLanguage::Chinese
        );
        assert_eq!(
            derive_translation_target("The temperature is 40 degrees."),
            TranslationLanguage::Chinese
        );
    }

    #[test]
    fn derive_target_mixed_text_returns_english() {
        assert_eq!(
            derive_translation_target("你好 world"),
            TranslationLanguage::English
        );
    }

    #[test]
    fn derive_target_japanese_kanji_returns_english_known_boundary() {
        // 已知语义边界：日文汉字被 contains_han 判定为真 → English。
        // 非 bug，是已知取舍。未来需结合 contains_kana 精化。
        assert_eq!(
            derive_translation_target("日本語の漢字"),
            TranslationLanguage::English
        );
    }

    #[test]
    fn derive_target_non_han_returns_chinese() {
        assert_eq!(
            derive_translation_target("12345"),
            TranslationLanguage::Chinese
        );
        assert_eq!(
            derive_translation_target("。，！"),
            TranslationLanguage::Chinese
        );
        assert_eq!(derive_translation_target(""), TranslationLanguage::Chinese);
    }

    // ---- 模型可用性 / 文件清单（NLLB） ----

    #[test]
    fn translate_returns_err_without_model_files() {
        let dir = unique_temp_dir("nllb-no-model");
        assert!(!TranslationEngine::is_available(
            &dir,
            TranslationLanguage::English
        ));
        assert!(TranslationEngine::new(&dir, TranslationLanguage::English).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn model_files_is_nllb_single_dir() {
        let files = TranslationEngine::model_files();
        assert_eq!(files.len(), 4, "NLLB 单目录 4 个文件");
        for (name, url) in &files {
            assert!(
                name.starts_with(NLLB_SUBDIR),
                "文件应在 {NLLB_SUBDIR} 下：{name}"
            );
            assert!(
                url.contains("mijuanlo/nllb-200-distilled-600M-ct2-int8"),
                "URL 命中 NLLB 仓库：{url}"
            );
        }
        for expected in [
            CONFIG_JSON,
            MODEL_BIN,
            SHARED_VOCABULARY_JSON,
            SENTENCEPIECE_MODEL,
        ] {
            assert!(
                files
                    .iter()
                    .any(|(n, _)| n == &format!("{NLLB_SUBDIR}/{expected}")),
                "应包含 {expected}"
            );
        }
    }

    #[test]
    fn is_available_requires_model_bin_and_sentencepiece() {
        let dir = unique_temp_dir("nllb-availability");
        write_placeholder_files(&dir, NLLB_SUBDIR, &minimum_runtime_files());
        assert!(
            TranslationEngine::is_available(&dir, TranslationLanguage::English),
            "model.bin + sentencepiece.bpe.model 应满足可用性"
        );
        // NLLB 单模型服务两方向 ⇒ 中→英同样可用。
        assert!(TranslationEngine::is_available(
            &dir,
            TranslationLanguage::Chinese
        ));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn required_runtime_files_include_all_nllb_assets() {
        assert_eq!(minimum_runtime_files(), [MODEL_BIN, SENTENCEPIECE_MODEL]);
        assert_eq!(
            required_runtime_files(),
            [
                CONFIG_JSON,
                MODEL_BIN,
                SHARED_VOCABULARY_JSON,
                SENTENCEPIECE_MODEL
            ]
        );
    }

    // ---- 分句 ----

    #[test]
    fn split_sentences_zh_splits_comma_long_sentence_into_clauses() {
        // 一句无句末标点、以逗号连写、超过 80 字 ⇒ 按逗号切子句。
        let long = "今天天气非常好，我们决定一起出门去公园散步，顺便看看那边的花，然后找个地方坐下来聊聊天，再吃点东西，最后慢慢走回家".repeat(2);
        assert!(
            long.chars().count() > ZH_MAX_SENT_CHARS,
            "用例输入须 >80 字，实测 {}",
            long.chars().count()
        );
        let sents = split_sentences(&long, TranslationLanguage::English);
        assert!(
            sents.len() >= 2,
            "逗号长句应切成多子句，实测 {}",
            sents.len()
        );
        // 拼接（去逗号）应与原文（去逗号）一致（不丢字）。
        let joined: String = sents.concat();
        assert_eq!(
            joined.chars().filter(|c| *c != '，').count(),
            long.chars().filter(|c| *c != '，').count()
        );
    }

    #[test]
    fn split_sentences_en_two_sentences_with_abbrev_and_decimal() {
        let text = "Mr. Smith paid 3.14 dollars. Then he left!";
        let sents = split_sentences(text, TranslationLanguage::Chinese);
        assert_eq!(sents.len(), 2, "应为 2 句，实测 {sents:?}");
        assert!(sents[0].contains("3.14"), "小数点不切");
        assert!(sents[0].contains("Mr."), "Mr. 缩写不切");
        assert!(sents[1].starts_with("Then"));
    }

    #[test]
    fn split_sentences_en_keeps_acronym() {
        let text = "The U.S. is large. It has 50 states.";
        let sents = split_sentences(text, TranslationLanguage::Chinese);
        assert_eq!(sents.len(), 2, "U.S. 缩写不切，实测 {sents:?}");
    }

    #[test]
    fn split_sentences_en_no_and_am_are_not_sentence_end_abbrev() {
        // R3：`no` / `am` 不再误判为缩写，句末应断句。
        let no = split_sentences("The answer is no. We left.", TranslationLanguage::Chinese);
        assert_eq!(no.len(), 2, "`no.` 处应断句，实测 {no:?}");
        let am = split_sentences("I am. You are.", TranslationLanguage::Chinese);
        assert_eq!(am.len(), 2, "`am.` 处应断句，实测 {am:?}");
    }

    #[test]
    fn split_sentences_en_no_before_number_is_abbreviation() {
        // R3：`No. 5` 后接数字 ⇒ 仍视为缩写，不断句；`ready.` 处断句。
        let sents = split_sentences("No. 5 is ready. Go.", TranslationLanguage::Chinese);
        assert_eq!(sents.len(), 2, "应为 2 句，实测 {sents:?}");
        assert!(sents[0].contains("No. 5"), "首句应含 No. 5：{sents:?}");
        assert!(
            sents[1].trim_start().starts_with("Go"),
            "次句应含 Go：{sents:?}"
        );
    }

    #[test]
    fn split_sentences_zh_sentence_terminators() {
        let sents = split_sentences("你好。再见！走吧？", TranslationLanguage::English);
        assert_eq!(sents.len(), 3);
    }

    #[test]
    fn split_sentences_drops_empty_and_merges_short_clause() {
        // 全空/空白句丢弃。
        assert!(split_sentences("   ", TranslationLanguage::English).is_empty());
        // 子句下限：一个 <10 字的碎子句应并入相邻（切后各段不少于 10 字）。
        let text = "这是一个很长很长需要被切开的中文句子用来测试子句合并的边界行为加长一点点哦";
        let sents = split_sentences(&text, TranslationLanguage::English);
        for s in &sents {
            assert!(!s.trim().is_empty());
        }
    }

    // ---- 拼接 ----

    #[test]
    fn join_parts_zh_to_en_uses_space_en_to_zh_no_space() {
        let parts = vec!["Hello".to_string(), "world".to_string()];
        assert_eq!(
            join_parts(&parts, TranslationLanguage::English),
            "Hello world"
        );
        let zh = vec!["你好".to_string(), "世界".to_string()];
        assert_eq!(join_parts(&zh, TranslationLanguage::Chinese), "你好世界");
        // 空译文不产生多余空格。
        let mixed = vec!["Hello".to_string(), String::new(), "world".to_string()];
        assert_eq!(
            join_parts(&mixed, TranslationLanguage::English),
            "Hello world"
        );
    }

    // ---- token 构造 / 去前缀 ----

    #[test]
    fn build_source_tokens_prepends_lang_appends_eos() {
        let t = build_source_tokens(LANG_ZH, vec!["你好".to_string(), "世界".to_string()]);
        assert_eq!(t, vec![LANG_ZH, "你好", "世界", EOS_TOKEN]);
        let t = build_source_tokens(LANG_EN, vec!["Hello".to_string()]);
        assert_eq!(t, vec![LANG_EN, "Hello", EOS_TOKEN]);
    }

    #[test]
    fn strip_target_prefix_removes_first_target_lang_token() {
        let toks = vec![
            LANG_EN.to_string(),
            "Hello".to_string(),
            "world".to_string(),
        ];
        assert_eq!(strip_target_prefix(&toks, LANG_EN), vec!["Hello", "world"]);
        // 首 token 不是目标语种 ⇒ 原样（防御）。
        let toks = vec!["Hello".to_string()];
        assert_eq!(strip_target_prefix(&toks, LANG_EN), vec!["Hello"]);
    }

    // ---- 漏译检测 + 重译至多一次 ----

    #[test]
    fn looks_truncated_boundaries() {
        // zh→en：4 字阈值 = 1.4 词
        assert!(!looks_truncated(
            "你好世界",
            "Hello world",
            TranslationLanguage::English
        ));
        assert!(looks_truncated(
            "你好世界",
            "Hi",
            TranslationLanguage::English
        ));
        assert!(looks_truncated(
            "你好世界",
            "",
            TranslationLanguage::English
        ));
        assert!(looks_truncated(
            "你好世界",
            "   ",
            TranslationLanguage::English
        ));
        // en→zh：2 词阈值 = 1.2 字
        assert!(!looks_truncated(
            "hello world",
            "你好",
            TranslationLanguage::Chinese
        ));
        assert!(looks_truncated(
            "hello world",
            "好",
            TranslationLanguage::Chinese
        ));
        assert!(looks_truncated(
            "hello world",
            "",
            TranslationLanguage::Chinese
        ));
    }

    #[test]
    fn finalize_sentence_retries_once_and_keeps_longer() {
        use std::cell::Cell;
        // 空结果 ⇒ 触发重译一次，取更长者。
        let calls = Cell::new(0);
        let (best, retries, still) = finalize_sentence(
            "你好世界",
            String::new(),
            TranslationLanguage::English,
            || {
                calls.set(calls.get() + 1);
                Some("Hello world".to_string())
            },
        );
        assert_eq!(calls.get(), 1, "空结果应触发重译恰一次");
        assert_eq!(retries, 1);
        assert_eq!(best, "Hello world");
        assert!(!still, "重译后不再疑似漏译");
        // 正常结果 ⇒ 不重译。
        let calls = Cell::new(0);
        let (best, retries, still) = finalize_sentence(
            "你好世界",
            "Hello world".to_string(),
            TranslationLanguage::English,
            || {
                calls.set(calls.get() + 1);
                Some("不应调用".to_string())
            },
        );
        assert_eq!(calls.get(), 0, "正常结果不得重译");
        assert_eq!(retries, 0);
        assert_eq!(best, "Hello world");
        assert!(!still);
    }

    #[test]
    fn finalize_sentence_warns_when_retry_still_short() {
        // 重译仍过短 ⇒ still=true（上层 warn），保留较长者，重译至多一次。
        use std::cell::Cell;
        let calls = Cell::new(0);
        let (best, retries, still) = finalize_sentence(
            "你好世界",
            "Hi".to_string(),
            TranslationLanguage::English,
            || {
                calls.set(calls.get() + 1);
                Some("Yo".to_string())
            },
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(retries, 1, "至多一次");
        assert_eq!(best, "Hi", "取较长者（Hi 2 > Yo 2 时保留原值）");
        assert!(still, "重译仍短 ⇒ 仍疑似漏译");
    }

    #[test]
    fn finalize_sentence_no_drop_all_sentences_kept() {
        // 拼接不丢句：N 句 ⇒ N 段（含空段）全部保留。
        let srcs = ["你好世界", "今天天气好", "再见"];
        let parts: Vec<String> = srcs
            .iter()
            .map(|s| finalize_sentence(s, s.to_string(), TranslationLanguage::English, || None).0)
            .collect();
        assert_eq!(parts.len(), 3, "逐句一一对应、不得丢句");
    }

    // ---- ensure_translation_direction ----

    #[test]
    fn ensure_direction_returns_none_when_rebuild_unavailable() {
        let mut cached: Option<(TranslationLanguage, TranslationEngine)> = None;
        let model_dir = Path::new("/nonexistent/voice-ime-models");
        let result =
            ensure_translation_direction(&mut cached, model_dir, TranslationLanguage::English);
        assert!(
            result.is_none(),
            "must return None when load fails — never pass a wrong-direction engine"
        );
    }

    // ---- 源码护栏：不再引用 opus-mt；不再有 min_decoding_length 强制 ----

    #[test]
    fn source_guard_no_opus_mt_and_no_forced_min_decoding() {
        let src = include_str!("mod.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap();
        let code: String = prod
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("opus-mt"),
            "生产代码不得再引用 opus-mt（仅历史注释可保留）"
        );
        assert!(
            !code.contains("min_decoding_length: std::cmp::max(1, source_tokens.len() / 2)"),
            "禁止强制 min_decoding_length = 源/2"
        );
        assert!(
            code.contains("min_decoding_length: 0"),
            "min_decoding_length 应设 0"
        );
        assert!(
            code.contains("translator_translate_batch_with_target_prefix"),
            "必须走带 target_prefix 的批量接口"
        );
    }

    #[test]
    fn source_guard_process_level_model_never_destroyed() {
        // R1-c：生产代码中 `NllbModel::new(` 只应出现一次（在 `shared_model` 内）；
        // 模型 `Box::leak` 常驻、`translator_destroy` 不再被生产路径触发。
        let src = include_str!("mod.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap();
        let code: String = prod
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            code.matches("NllbModel::new(").count(),
            1,
            "NllbModel::new 只应在 shared_model 内出现一次（进程级单次加载）"
        );
        assert!(
            code.contains("fn shared_model("),
            "应有 shared_model 加载器"
        );
        assert!(
            code.contains("Box::leak"),
            "模型应 Box::leak 常驻、永不析构"
        );
        assert!(
            code.contains("thread_local!"),
            "应使用 thread_local 进程级缓存（见 shared_model 注释）"
        );
    }

    // ---- TEST-SYNC-394 护栏（非作者；纯函数，无需模型）----
    // 🔴 每条期望值均**手算**写进注释（上一单 `ts393c_lens_shorter` 期望值算错的教训）。

    /// S394-1 不丢字不变式：`split_sentences` 只切句、不丢字、不改字 ——
    /// 各句去掉**所有空白**后拼接 == 输入去掉所有空白。4 类易丢字形态。
    #[test]
    fn s394_split_sentences_preserves_all_non_whitespace() {
        let strip_ws = |s: &str| -> String { s.chars().filter(|c| !c.is_whitespace()).collect() };
        // 中文逗号连写 88 字（>80）：按 `，` 切 8 个子句（各 11 字 ≥ 下限 10）。
        let long_zh = "一二三四五六七八九十，".repeat(8);
        // (输入, 方向, 手算句数)
        let cases: [(&str, TranslationLanguage, usize); 4] = [
            // `。！？；……。` ⇒ 6 个句末（`……` 记 1）⇒ 6 句。
            (
                "第一句。第二句！第三句？第四句；第五句……第六句。",
                TranslationLanguage::English,
                6,
            ),
            // 8 子句（见上）⇒ 8 句。
            (long_zh.as_str(), TranslationLanguage::English, 8),
            // 英文 17 词 ≤ 60，且 Mr./Dr./3.14/p.m./U.S./No. 5 全不断 ⇒ 1 句。
            (
                "Mr. Smith met Dr. Jones at 3.14 p.m. He lives in the U.S. See No. 5 today.",
                TranslationLanguage::Chinese,
                1,
            ),
            // `。` 断两次 ⇒ 2 句；`Node.js` 的 `.` 非中文终止符。
            (
                "我使用Node.js开发。它是JavaScript运行时。",
                TranslationLanguage::English,
                2,
            ),
        ];
        for (text, dir, expect_n) in cases {
            let sents = split_sentences(text, dir);
            assert_eq!(sents.len(), expect_n, "句数不符：{text:?}");
            assert_eq!(
                strip_ws(&sents.concat()),
                strip_ws(text),
                "分句丢字/改字：{text:?}"
            );
        }
    }

    /// S394-2：`split_zh_sentences` —— 单个 `…` 不断；`……` 断在第二个 `…` 后；
    /// 中文句中夹英文 `Node.js 3.14` 不在 `.` 处断。
    #[test]
    fn s394_split_zh_single_vs_double_ellipsis_and_dot() {
        // 单个 `…`：无终止符 ⇒ 1 句。
        assert_eq!(
            split_zh_sentences("前半…后半"),
            vec!["前半…后半".to_string()]
        );
        // `……`：第二个 `…` 处断 ⇒ 2 句。
        assert_eq!(
            split_zh_sentences("前半……后半"),
            vec!["前半……".to_string(), "后半".to_string()]
        );
        // `……` 收尾：断后 cur 为空 ⇒ 不产空句 ⇒ 1 句。
        assert_eq!(split_zh_sentences("前半……"), vec!["前半……".to_string()]);
        // 夹 `Node.js 3.14`：`.` 非中文终止符 ⇒ 只按 `。` ⇒ 1 句。
        assert_eq!(
            split_zh_sentences("我喜欢Node.js 3.14版本的功能测试。"),
            vec!["我喜欢Node.js 3.14版本的功能测试。".to_string()]
        );
    }

    /// S394-3：英文分句 + `ends_with_abbreviation`（`no` 的数字后置条件）。
    #[test]
    fn s394_split_en_no_abbrev_and_sentence_count() {
        // `no.` 后接 `W`（非数字）⇒ 断句 ⇒ 2 句。
        let two: Vec<String> = split_en_sentences("The answer is no. We left.")
            .iter()
            .map(|s| s.trim().to_string())
            .collect();
        assert_eq!(
            two,
            vec!["The answer is no.".to_string(), "We left.".to_string()]
        );
        // `No. 5` 后接数字 ⇒ 缩写不断 ⇒ 1 句。
        assert_eq!(split_en_sentences("See No. 5 today.").len(), 1);
        // `U.S.` 含点 ⇒ 缩写不断 ⇒ 1 句。
        assert_eq!(split_en_sentences("I live in the U.S. now.").len(), 1);
        // 直接测判定：`no.` 的数字后置条件 + 含点缩写 + 普通句末。
        assert!(!ends_with_abbreviation("no.", Some('W')));
        assert!(ends_with_abbreviation("no.", Some('5')));
        assert!(ends_with_abbreviation("U.S.", None));
        assert!(ends_with_abbreviation("Mr.", Some('S')));
        assert!(!ends_with_abbreviation("today.", None));
    }

    /// S394-4：`split_and_merge` 全部子句 < 下限 ⇒ 合成 1 句且内容 == 原文。
    #[test]
    fn s394_split_and_merge_all_short_collapses_to_original() {
        // 手算：子句 "短，"(2) / "很短，"(3) / "也短"(2) 均 <10 ⇒ 依次并入 ⇒ 1 句 == 原文。
        let src = "短，很短，也短";
        assert_eq!(
            split_and_merge(src, &['，'], ZH_MIN_CLAUSE_CHARS, true),
            vec![src.to_string()]
        );
        // 对照：两子句 "一二三四五六七八九十，"(11) / "甲乙丙丁戊己庚辛壬癸"(10) 均 ≥10 ⇒ 不合并 ⇒ 2 句。
        assert_eq!(
            split_and_merge(
                "一二三四五六七八九十，甲乙丙丁戊己庚辛壬癸",
                &['，'],
                ZH_MIN_CLAUSE_CHARS,
                true
            ),
            vec![
                "一二三四五六七八九十，".to_string(),
                "甲乙丙丁戊己庚辛壬癸".to_string()
            ]
        );
    }

    /// S394-5：`finalize_sentence` —— 重译至多一次；`None`（出错）保留原译；更短保留原译；正常不调用。
    #[test]
    fn s394_finalize_sentence_retry_once_keep_or_skip() {
        use std::cell::Cell;
        // src "你好世界" 4 字 ⇒ 阈值 4×0.35=1.4；"Hi" 1 词 <1.4 ⇒ 疑似漏译。
        let src = "你好世界";
        // (a) 重译返回 None（出错）⇒ 保留 "Hi"、calls=1、仍疑似。
        let calls_a = Cell::new(0usize);
        let (out_a, n_a, still_a) =
            finalize_sentence(src, "Hi".to_string(), TranslationLanguage::English, || {
                calls_a.set(calls_a.get() + 1);
                None
            });
        assert_eq!((out_a.as_str(), n_a, still_a), ("Hi", 1, true));
        assert_eq!(calls_a.get(), 1, "重译应恰 1 次");
        // (b) 重译更短（"H" 1 字 < "Hi" 2 字）⇒ 保留 "Hi"、calls=1。
        let (out_b, n_b, _) =
            finalize_sentence(src, "Hi".to_string(), TranslationLanguage::English, || {
                Some("H".to_string())
            });
        assert_eq!((out_b.as_str(), n_b), ("Hi", 1));
        // (c) 原译正常（"你好" 2 字 ⇒ 阈值 0.7；"Hello world" 2 词 ≥0.7）⇒ 不调用重译（闭包 panic 即证）。
        let (out_c, n_c, still_c) = finalize_sentence(
            "你好",
            "Hello world".to_string(),
            TranslationLanguage::English,
            || panic!("原译正常时不得调用重译"),
        );
        assert_eq!((out_c.as_str(), n_c, still_c), ("Hello world", 0, false));
    }

    /// S394-6：`strip_target_prefix` —— 首 token 非目标语种 ⇒ 原样（不丢首词）；空 ⇒ 空。
    #[test]
    fn s394_strip_target_prefix_keeps_first_when_not_target() {
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // 首 token == 目标 ⇒ 剥掉。
        assert_eq!(
            strip_target_prefix(&t(&["eng_Latn", "Hello"]), "eng_Latn"),
            t(&["Hello"])
        );
        // 首 token != 目标 ⇒ 原样，**不丢首词**。
        assert_eq!(
            strip_target_prefix(&t(&["Hello", "world"]), "eng_Latn"),
            t(&["Hello", "world"])
        );
        assert_eq!(
            strip_target_prefix(&t(&["zho_Hans", "你好"]), "eng_Latn"),
            t(&["zho_Hans", "你好"])
        );
        // 空输入 ⇒ 空。
        assert_eq!(strip_target_prefix(&[], "eng_Latn"), Vec::<String>::new());
    }

    /// S394-7：`join_parts` —— 含空串 ⇒ 中→英无连续/首尾空格；英→中直连无空格。
    #[test]
    fn s394_join_parts_no_extra_spaces() {
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // 中→英：空串夹中间 ⇒ join(" ") 得 "Hello  world"（两空格）⇒ 归一为 1 空格。
        assert_eq!(
            join_parts(&t(&["Hello", "", "world"]), TranslationLanguage::English),
            "Hello world"
        );
        // 首空 ⇒ 无前导空格。
        assert_eq!(
            join_parts(&t(&["", "Hello"]), TranslationLanguage::English),
            "Hello"
        );
        // 尾空 ⇒ 无尾随空格。
        assert_eq!(
            join_parts(&t(&["Hello", ""]), TranslationLanguage::English),
            "Hello"
        );
        // 英→中：直连、无空格。
        assert_eq!(
            join_parts(&t(&["你", "好"]), TranslationLanguage::Chinese),
            "你好"
        );
    }

    /// S394-8 源码护栏（非作者；剔除注释行）：进程级模型永不析构的**结构不变式**。
    #[test]
    fn s394_source_guard_model_resident_and_no_arc_no_forget() {
        let src = include_str!("mod.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap();
        let code: String = prod
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("Arc<NllbModel>"),
            "生产区不得用 Arc<NllbModel>（thread_local + Box::leak 常驻）"
        );
        assert!(
            !code.contains("mem::forget"),
            "生产区不得 mem::forget（R1-c 已全删）"
        );
        assert!(
            code.contains("model: &'static NllbModel,"),
            "TranslationEngine 模型字段类型必须是 &'static NllbModel"
        );
        let sm = code
            .split("fn shared_model(")
            .nth(1)
            .expect("shared_model 锚点缺失");
        let new_pos = sm
            .find("NllbModel::new(model_dir)?")
            .expect("shared_model 内应有 NllbModel::new(model_dir)?");
        let slot_pos = sm
            .find("*slot = Some(")
            .expect("shared_model 内应有 *slot = Some(");
        assert!(
            new_pos < slot_pos,
            "加载失败不得写缓存：NllbModel::new(...)? 必须在 *slot = Some( 之前"
        );
    }

    // ---- 真模型实跑（#[ignore]） ----

    /// TRANS-394 验收：真 NLLB 实跑 —— 中→英 3 段（短 / ≥5 句长段 / 逗号连写 100+ 字）、
    /// 英→中 2 段（短 / 长）；断言**原句数 == 译句数**、每句过 `looks_truncated`、数字全部保留，
    /// 并打印逐句对照表。运行：
    /// `cargo test --bin feiyin-ime -- --ignored --nocapture trans394_real_model`
    #[test]
    #[ignore = "requires NLLB model; cargo test --bin feiyin-ime -- --ignored --nocapture trans394_real_model"]
    fn trans394_real_model() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = root.join("models");
        if !TranslationEngine::is_available(&model_dir, TranslationLanguage::English) {
            eprintln!("skip: NLLB model 不在位 at {}", model_dir.display());
            return;
        }

        // 共享同一份已加载 NLLB（with_direction 只换方向、不重载 594MB 权重）。
        let engine =
            TranslationEngine::new(&model_dir, TranslationLanguage::English).expect("load NLLB");
        let zh_to_en = &engine;
        let en_to_zh = engine.with_direction(TranslationLanguage::Chinese);

        let zh_short = "今天温度是 25 度。";
        let zh_long = "上周我们去了一个很大的博物馆。那里展出了很多古代文物。我最喜欢的是那些青铜器。它们上面刻着复杂的纹饰。讲解员说这些文物有三千多年历史。参观结束后我们还买了纪念品。";
        let zh_comma = "这次会议我们讨论了三个重要的议题，第一个是明年的预算安排，大家一致觉得应该继续增加研发方面的投入，第二个是关于人员招聘的具体计划，需要在明年春季之前全部完成，第三个是办公场地的搬迁时间，大概定在六月以后，具体的日期还要再确认一下，另外还需要确定每个项目的负责人和联系方式";
        let en_short = "There are 42 items in stock.";
        let en_long = "The project has been running for several months. We have collected a large amount of data. The results so far look promising. However, there are still some issues to solve. We need more time to finish the analysis. I believe we can deliver the report next week.";

        let zh_cases = [zh_short, zh_long, zh_comma];
        let mut failures = 0usize;
        for (case, src) in zh_cases.iter().enumerate() {
            let sents = split_sentences(src, TranslationLanguage::English);
            let translated = zh_to_en.translate(src).expect("translate zh→en");
            let out_parts = split_sentences(&translated, TranslationLanguage::Chinese); // 仅用于打印
            println!("\n===== ZH→EN case#{case} （原 {src}）");
            println!(
                "  sentences({}) => translated => {}",
                sents.len(),
                translated
            );
            for (i, s) in sents.iter().enumerate() {
                println!("   [{i}] 原句: {s}");
            }
            for (i, o) in out_parts.iter().enumerate() {
                println!("   [{i}] 译文: {o}");
            }
            // 原句数 == 译句数（逐句一一对应；translate 内部按拆分后的句逐条产出）。
            assert!(!translated.trim().is_empty(), "case#{case} 译文不得为空");
            match case {
                0 => assert_eq!(sents.len(), 1, "短句应 1 句"),
                1 => {
                    assert!(sents.len() >= 5, "长段应 ≥5 句，实测 {}", sents.len());
                    assert_eq!(
                        out_parts.len(),
                        sents.len(),
                        "长段：原句数({}) == 译句数({})",
                        sents.len(),
                        out_parts.len()
                    );
                }
                2 => {
                    assert!(
                        zh_effective_chars(src) > 100,
                        "逗号长段应 >100 有效字，实测 {}",
                        zh_effective_chars(src)
                    );
                    assert!(
                        sents.len() >= 2,
                        "逗号连写长段应切成多子句（子句数 {}）",
                        sents.len()
                    );
                }
                _ => {}
            }
            // 每句译文过 looks_truncated 检查（对整句译文再核）。
            if looks_truncated(src, &translated, TranslationLanguage::English) {
                // 允许因分句导致的整体比例偏差，但若明显过短则记失败。
                let src_eff = zh_effective_chars(src);
                let dst_words = en_word_count(&translated);
                if (dst_words as f32) < (src_eff as f32) * OMIT_EN_PER_ZH {
                    eprintln!("  ⚠️ case#{case} possible omission: src_eff={src_eff} dst_words={dst_words}");
                    failures += 1;
                }
            }
            for num in extract_numbers(src) {
                if !translated.contains(&num) {
                    eprintln!("  ⚠️ case#{case} number not carried: {num}");
                    failures += 1;
                }
            }
        }

        let en_cases = [en_short, en_long];
        for (case, src) in en_cases.iter().enumerate() {
            let sents = split_sentences(src, TranslationLanguage::Chinese);
            let translated = en_to_zh.translate(src).expect("translate en→zh");
            println!("\n===== EN→ZH case#{case} （原 {src}）");
            println!(
                "  sentences({}) => translated => {}",
                sents.len(),
                translated
            );
            for (i, s) in sents.iter().enumerate() {
                println!("   [{i}] 原句: {s}");
            }
            assert!(!translated.trim().is_empty(), "case#{case} 译文不得为空");
            if looks_truncated(src, &translated, TranslationLanguage::Chinese) {
                eprintln!(
                    "  ⚠️ case#{case} possible omission: src_words={} dst_chars={}",
                    en_word_count(src),
                    zh_effective_chars(&translated)
                );
                failures += 1;
            }
            // 数字保留：原文阿拉伯数字串应出现在译文。
            for num in extract_numbers(src) {
                if !translated.contains(&num) {
                    eprintln!("  ⚠️ case#{case} number not carried: {num}");
                    failures += 1;
                }
            }
        }

        assert_eq!(
            failures, 0,
            "TRANS-394 验收：存在漏译/未保留数字（见上方 ⚠️）"
        );

        // TRANS-394-REWORK：不再需要 `std::mem::forget`。模型由 `shared_model` 进程级 `Box::leak`
        // 常驻，引擎可正常 drop（不触发 `translator_destroy`）——能干净退出本身就是修复的证据。
        drop(en_to_zh);
        drop(engine);
    }

    /// TRANS-394-REWORK R1-c：进程级模型共享 + 干净 drop（`#[ignore]` 真模型）。
    /// 断言两次 `TranslationEngine::new` 命中**同一** `NllbModel`（`ptr::eq`）、第二次 <50ms，
    /// 且两个引擎可正常 drop（无 `std::mem::forget`）。运行：
    /// `cargo test --bin feiyin-ime -- --ignored --nocapture --test-threads=1 trans394_rework_shared_model`
    #[test]
    #[ignore = "requires NLLB model; cargo test --bin feiyin-ime -- --ignored --nocapture --test-threads=1 trans394_rework_shared_model"]
    fn trans394_rework_shared_model_pointer_and_speed() {
        let model_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");
        if !TranslationEngine::is_available(&model_dir, TranslationLanguage::English) {
            eprintln!("skip: NLLB model 不在位");
            return;
        }
        let first = TranslationEngine::new(&model_dir, TranslationLanguage::English)
            .expect("first NLLB load");
        let t = std::time::Instant::now();
        let second = TranslationEngine::new(&model_dir, TranslationLanguage::Chinese)
            .expect("second NLLB load");
        let second_elapsed = t.elapsed();
        eprintln!(
            "shared NllbModel ptr={:p}; 第二次 new 耗时 {:.3}ms",
            first.model,
            second_elapsed.as_secs_f64() * 1000.0
        );
        assert!(
            std::ptr::eq(first.model, second.model),
            "两次 new 必须命中同一进程级 NllbModel"
        );
        assert!(
            second_elapsed < std::time::Duration::from_millis(50),
            "第二次 new 应为缓存命中（<50ms），实测 {second_elapsed:?}"
        );
        // 修复后引擎可正常 drop（无 mem::forget）——干净退出即「不再调用 translator_destroy」的证据。
        drop(first);
        drop(second);
        eprintln!("✅ 两个引擎均已正常 drop，测试正常结束");
    }

    /// TRANS-394-REWORK R1-a：`translator_destroy` 挂死取证（`#[ignore]` 真模型）。
    ///
    /// 在**普通线程**内（非进程 teardown）加载引擎 → 可选翻译 → `drop(engine)`，主线程轮询 30s；
    /// 期间每 5s 采样本进程 CPU 时间，用于区分「忙等自旋（CPU≈全核）」与「死锁阻塞（CPU≈0）」。
    /// 超时线程**故意不 join**（泄漏），保证测试进程自身能退出。
    fn run_drop_probe(label: &str, model_dir: &Path, do_translate: bool) {
        use std::sync::mpsc::{channel, RecvTimeoutError};
        use std::time::{Duration, Instant};

        let (tx, rx) = channel::<()>();
        let dir = model_dir.to_path_buf();
        let cpu_before = process_cpu_seconds();
        let wall = Instant::now();
        let handle = std::thread::Builder::new()
            .name(format!("probe-{label}"))
            .spawn(move || {
                let engine = TranslationEngine::new(&dir, TranslationLanguage::English)
                    .expect("load NLLB for probe");
                if do_translate {
                    let _ = engine.translate("今天天气很好。");
                }
                drop(engine);
                let _ = tx.send(());
            })
            .expect("spawn probe thread");

        let mut returned_at = None;
        for sec in 1..=30u64 {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(()) => {
                    returned_at = Some(sec);
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {
                    if sec % 5 == 0 {
                        eprintln!(
                            "  [{label}] t={sec}s 未返回；进程 CPU 增 {:.2}s",
                            process_cpu_seconds() - cpu_before
                        );
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        let cpu_delta = process_cpu_seconds() - cpu_before;
        match returned_at {
            Some(sec) => eprintln!(
                "  ✅ [{label}] drop 在 {sec}s 内返回；wall={:.2}s；CPU 增 {:.2}s",
                wall.elapsed().as_secs_f64(),
                cpu_delta
            ),
            None => eprintln!(
                "  🔴 [{label}] 30s 内 drop 未返回（疑似 translator_destroy 挂死）；CPU 增 {:.2}s",
                cpu_delta
            ),
        }
        // 不 join：正常完成线程已退出；挂死线程直接泄漏，避免拖住测试进程退出。
        drop(handle);
    }

    #[test]
    #[ignore = "requires NLLB model; cargo test --bin feiyin-ime -- --ignored --nocapture trans394_rework_drop_probe_translated"]
    fn trans394_rework_drop_probe_translated() {
        let model_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");
        if !TranslationEngine::is_available(&model_dir, TranslationLanguage::English) {
            eprintln!("skip: NLLB model 不在位");
            return;
        }
        run_drop_probe("A:load+translate+drop", &model_dir, true);
    }

    #[test]
    #[ignore = "requires NLLB model; cargo test --bin feiyin-ime -- --ignored --nocapture trans394_rework_drop_probe_loadonly"]
    fn trans394_rework_drop_probe_loadonly() {
        let model_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");
        if !TranslationEngine::is_available(&model_dir, TranslationLanguage::English) {
            eprintln!("skip: NLLB model 不在位");
            return;
        }
        run_drop_probe("B:load-only+drop", &model_dir, false);
    }

    #[cfg(target_os = "windows")]
    fn process_cpu_seconds() -> f64 {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
        unsafe {
            let mut creation = FILETIME::default();
            let mut exit = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            if GetProcessTimes(
                GetCurrentProcess(),
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user,
            )
            .is_err()
            {
                return 0.0;
            }
            let ticks = |t: FILETIME| ((t.dwHighDateTime as u64) << 32) | (t.dwLowDateTime as u64);
            (ticks(kernel) + ticks(user)) as f64 / 10_000_000.0
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn process_cpu_seconds() -> f64 {
        0.0
    }

    fn extract_numbers(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for c in text.chars() {
            if c.is_ascii_digit() {
                cur.push(c);
            } else if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    }
}
