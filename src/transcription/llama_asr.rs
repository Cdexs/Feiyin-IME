//! ACC-ENGINE-LLAMACPP-452：Qwen3-ASR 1.7B 精解引擎（llama.cpp，Vulkan / Metal / CPU 自动选择）。
//!
//! Gavin 2026-09-27：「换，不保留目前的 sherpa onnx 调用方式」「一定要用最优的方式来调用，最大化的使用它的功能和性能」
//! 「用 Q8+f16」「换新的模型和调用框架一定不能影响现在的管线功能」。
//!
//! - 适配层 `native/llama_asr/shim.cpp`（build.rs 编译进主程序）运行时动态加载 exe 同目录的 llama.cpp 官方预编译库
//!  （b11207：`ggml-base` / `ggml` / `llama` / `mtmd` + `ggml-vulkan` / `ggml-cpu-*` / `libomp`）。
//! - 调参依据 `collab/research/gpu-accel-451.md`（104 片实测）：Flash Attention 必开、音频编码器放 GPU、
//!   KV q8_0 + ubatch 128（MEM-453 省显存）、贪心；CPU 回落时生成 / 预填充线程分设。
//! - 对外只替换「一段音频 → 一段原始文本」这一步；输出形态与原 sherpa 相同（`language X<asr_text>正文`），
//!   下游剥离 / 规整 / 护栏 / 拼接等管线逻辑一律不动。

use anyhow::{bail, Context, Result};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 模型目录（`models/` 下）与文件名。LLM Q8_0；音频编码器 MEM-453 由 f16 改 Q8_0（显存 612→~340MiB）。
/// 无 f16 存量用户（Gavin 09-29），不做回退。
pub(crate) const LLAMA_ASR_MODEL_SUBDIR: &str = "qwen3-asr-1.7b-gguf";
pub(crate) const LLAMA_ASR_MODEL_FILE: &str = "Qwen3-ASR-1.7B-Q8_0.gguf";
pub(crate) const LLAMA_ASR_MMPROJ_FILE: &str = "mmproj-Qwen3-ASR-1.7B-Q8_0.gguf";

/// 上下文长度上限：音频（~12.5 token/s）+ 上下文 / 词库 + 生成。与原 sherpa `max_total_len=4096` 同额度。
const N_CTX: i32 = 4096;
/// MEM-TRIM-457 ②：初始上下文（KV 按此分配，q8_0 4096→1024 省 ~178MiB）；某窗放不下时 shim 自动重建为
/// 更大（翻倍，封顶 [`N_CTX`]）。常规窗（词库 ~80 字 + 音频 ≤20s + 生成 ≤256）远小于 1024。
const N_CTX_INIT: i32 = 1024;
/// 未指定上限时的生成长度（与原 sherpa 全局 `max_new_tokens: 256` 一致）。
pub(crate) const DEFAULT_MAX_NEW_TOKENS: i32 = 256;
/// 草稿推测解码每步最多验证的草稿 token 数。
pub(crate) const DRAFT_MAX: i32 = 8;

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub(crate) struct LasStats {
    pub prefill_ms: f64,
    pub gen_ms: f64,
    pub n_prompt: i32,
    pub n_gen: i32,
    pub n_draft_accepted: i32,
    pub n_draft_proposed: i32,
    /// LLAMA-TUNE-458：音频编码耗时（仅调试开关 `LAS_SPLIT_ENC` 下拆分；否则 0）。
    pub enc_ms: f64,
}

extern "C" {
    fn las_set_log(f: extern "C" fn(c_int, *const c_char));
    fn las_load(dir: *const c_char, err: *mut c_char, err_len: c_int) -> c_int;
    fn las_create(
        model: *const c_char,
        mmproj: *const c_char,
        gpu: c_int,
        n_threads: c_int,
        n_threads_batch: c_int,
        n_ctx: c_int,
        n_ctx_max: c_int,
        info: *mut c_char,
        info_len: c_int,
        err: *mut c_char,
        err_len: c_int,
    ) -> *mut c_void;
    fn las_free(e: *mut c_void);
    fn las_count_tokens(e: *mut c_void, text: *const c_char) -> c_int;
    fn las_decode(
        e: *mut c_void,
        pcm: *const f32,
        n_samples: c_int,
        system: *const c_char,
        prefix: *const c_char,
        draft: *const c_char,
        max_new_tokens: c_int,
        draft_max: c_int,
        out: *mut c_char,
        out_len: c_int,
        st: *mut LasStats,
        err: *mut c_char,
        err_len: c_int,
        on_partial: Option<extern "C" fn(*mut c_void, *const c_char)>,
        user: *mut c_void,
    ) -> c_int;
    fn las_align_create(
        backbone: *const c_char,
        mmproj: *const c_char,
        head: *const c_char,
        gpu: c_int,
        n_threads: c_int,
        n_ctx: c_int,
        info: *mut c_char,
        info_len: c_int,
        err: *mut c_char,
        err_len: c_int,
    ) -> *mut c_void;
    fn las_align_free(a: *mut c_void);
    fn las_align(
        a: *mut c_void,
        pcm: *const f32,
        n_samples: c_int,
        units_joined: *const c_char,
        n_units: c_int,
        out_start: *mut f32,
        out_end: *mut f32,
        ms_out: *mut f64,
        err: *mut c_char,
        err_len: c_int,
    ) -> c_int;
}

/// ACC-452 ②⑤：解码辅助（不改变解码结果的两项能力）。
/// - `draft`：草稿文字（推测解码，只省时间）；
/// - `on_partial`：逐字回调（流式回灌），参数为「预填 + 截至目前的生成内容」原文（含 `language X<asr_text>`）。
#[derive(Clone, Copy, Default)]
pub(crate) struct DecodeAssist<'a> {
    pub draft: Option<&'a str>,
    pub on_partial: Option<&'a (dyn Fn(&str) + Sync)>,
}

impl<'a> DecodeAssist<'a> {
    /// 只带草稿（无逐字回调；测试 / 回放用）。
    #[cfg(test)]
    pub(crate) fn draft(draft: Option<&'a str>) -> Self {
        Self {
            draft,
            on_partial: None,
        }
    }
}

extern "C" fn partial_trampoline(user: *mut c_void, text: *const c_char) {
    if user.is_null() || text.is_null() {
        return;
    }
    // SAFETY：user 指向 `decode_with_prefix` 栈上的 `&dyn Fn(&str)`，调用期间存活；text 为 NUL 结尾。
    let f = unsafe { &*(user as *const &(dyn Fn(&str) + Sync)) };
    let s = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    f(&s);
}

extern "C" fn forward_log(level: c_int, text: *const c_char) {
    if text.is_null() {
        return;
    }
    // SAFETY：shim 传入以 NUL 结尾的 C 字符串，生命周期覆盖本次回调。
    let s = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    let s = s.trim_end();
    if level >= 4 {
        log::error!("[llama.cpp] {s}");
    } else {
        log::warn!("[llama.cpp] {s}");
    }
}

fn buf_str(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

fn cstr(s: &str) -> Result<CString> {
    CString::new(s).context("string contains NUL")
}

/// 运行库目录：exe 同目录；`cargo test` 的测试 exe 在 `target/<profile>/deps/` ⇒ 再找上一级。
fn runtime_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    for d in [Some(dir.clone()), dir.parent().map(Path::to_path_buf)]
        .into_iter()
        .flatten()
    {
        let probe = if cfg!(target_os = "windows") {
            d.join("llama.dll")
        } else {
            d.join("libllama.dylib")
        };
        if probe.is_file() {
            return Some(d);
        }
    }
    None
}

/// 加载 llama.cpp 运行库（精解引擎与对齐器共用；shim 内幂等）。
fn ensure_runtime() -> Result<()> {
    let rt = runtime_dir()
        .context("llama.cpp runtime libraries (llama.dll/mtmd.dll) not found next to exe")?;
    let mut err = [0u8; 512];
    // SAFETY：传入有效 NUL 结尾字符串与可写缓冲。
    unsafe { las_set_log(forward_log) };
    let rt_c = cstr(&rt.to_string_lossy())?;
    let rc = unsafe {
        las_load(
            rt_c.as_ptr(),
            err.as_mut_ptr() as *mut c_char,
            err.len() as c_int,
        )
    };
    if rc != 0 {
        bail!("llama.cpp runtime load failed: {}", buf_str(&err));
    }
    Ok(())
}

/// 线程数（依据 451/452 实测）：
/// - GPU：计算都在显卡，CPU 线程只做调度 / 采样 / 梅尔谱 ⇒ 4。
/// - CPU 回落：生成取 min(逻辑核/2, 8)（≈物理核，实测 6~8 最快、16 反慢）；
///   预填充取 min(逻辑核 - 4, 12)（实测 12 最快，留 4 核给流式预览）。
fn thread_plan(gpu: bool) -> (i32, i32) {
    let logical = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8) as i32;
    if gpu {
        (4, 4)
    } else {
        ((logical / 2).clamp(1, 8), (logical - 4).clamp(1, 12))
    }
}

/// 一次解码的输出：原始文本（含 `language X<asr_text>` 前缀，与原 sherpa 结果同形态）+ 计时统计。
pub(crate) struct LlamaDecodeOut {
    pub raw: String,
    pub stats: LasStats,
}

/// Qwen3-ASR 精解引擎。内部串行（`Mutex`）：同一时刻只解一窗（生产 `WINDOW_DECODE_CONCURRENCY=1`）。
pub(crate) struct LlamaAsr {
    ptr: *mut c_void,
    lock: Mutex<()>,
    device: String,
    /// FORCED-ALIGN-456：强制对齐器（仅本地实时加载；`None` = 未加载 / 加载失败 ⇒ 接缝回落比例估算）。
    aligner: Option<LlamaAligner>,
}

// SAFETY：引擎句柄只经 `lock` 串行访问；llama.cpp 上下文可在线程间移动（单线程使用）。
unsafe impl Send for LlamaAsr {}
unsafe impl Sync for LlamaAsr {}

/// MEM-TRIM-457 ③：当前已加载的 1.7B 引擎句柄，**只用于数词库 token**（读词表，不碰解码上下文）。
/// 与解码锁 `lock` 分开 —— 录音开始时数 token 不必等正在跑的解码；`Drop` 在同一把锁下先注销再释放引擎，
/// 数 token 期间引擎不会被释放。
struct CountHandle(*mut c_void);
// SAFETY：只经 `COUNT_ENGINE` 互斥访问；llama.cpp 的词表加载后只读，分词可与解码并行（llama-server 同用法）。
unsafe impl Send for CountHandle {}
static COUNT_ENGINE: Mutex<Option<CountHandle>> = Mutex::new(None);

/// MEM-TRIM-457 ③：用已加载的 1.7B 引擎词表数 token（与原单独加载的 Qwen3 `tokenizer.json` 是同一套词表、
/// 同口径 `add_special=false`，42 个真实词条逐个比对计数全同）。引擎未加载 / 失败 ⇒ `None`
/// （调用方按 UTF-8 字节上界估算，与原 tokenizer 缺失时一致）。
pub(crate) fn count_tokens(text: &str) -> Option<usize> {
    let c = cstr(text).ok()?;
    let g = COUNT_ENGINE.lock().unwrap_or_else(|e| e.into_inner());
    let h = g.as_ref()?;
    // SAFETY：句柄在锁内有效（Drop 先在同锁下注销）；c 在调用期间存活。
    let n = unsafe { las_count_tokens(h.0, c.as_ptr()) };
    (n >= 0).then_some(n as usize)
}

impl Drop for LlamaAsr {
    fn drop(&mut self) {
        {
            let mut g = COUNT_ENGINE.lock().unwrap_or_else(|e| e.into_inner());
            if g.as_ref().is_some_and(|h| h.0 == self.ptr) {
                *g = None;
            }
        }
        // SAFETY：ptr 由 las_create 返回且只在此处释放一次。
        unsafe { las_free(self.ptr) };
    }
}

impl LlamaAsr {
    /// 模型文件所在目录（`<models>/qwen3-asr-1.7b-gguf/`）是否齐全。
    /// FORCED-ALIGN-456：对齐模型与 1.7B 一起作为标配下载（Gavin 09-29）⇒ 三件套也计入就位判据。
    pub(crate) fn model_ready(models_root: &Path) -> (bool, PathBuf) {
        let dir = models_root.join(LLAMA_ASR_MODEL_SUBDIR);
        let ok = dir.join(LLAMA_ASR_MODEL_FILE).is_file()
            && dir.join(LLAMA_ASR_MMPROJ_FILE).is_file()
            && LlamaAligner::model_ready(models_root).0;
        (ok, dir)
    }

    /// 加载引擎：优先 GPU（Vulkan / Metal），GPU 不可用或创建失败 ⇒ 自动回落 CPU。
    pub(crate) fn load(models_root: &Path) -> Result<Self> {
        let (ok, dir) = Self::model_ready(models_root);
        if !ok {
            bail!("Qwen3-ASR GGUF model not found under {}", dir.display());
        }
        Self::create(
            &dir.join(LLAMA_ASR_MODEL_FILE),
            &dir.join(LLAMA_ASR_MMPROJ_FILE),
            &[true, false],
        )
    }

    /// QUANT-AB-457：测试用 —— 指定主模型 / 编码器文件与设备（量化方案对比）。
    #[cfg(test)]
    pub(crate) fn load_files(model: &Path, mmproj: &Path, gpu: bool) -> Result<Self> {
        Self::create(model, mmproj, &[gpu])
    }

    fn create(model: &Path, mmproj: &Path, devices: &[bool]) -> Result<Self> {
        ensure_runtime()?;
        let mut err;
        let model = cstr(&model.to_string_lossy())?;
        let mmproj = cstr(&mmproj.to_string_lossy())?;
        let mut last_err = String::new();
        for &gpu in devices {
            let (nt, ntb) = thread_plan(gpu);
            let mut info = [0u8; 256];
            err = [0u8; 512];
            let t0 = std::time::Instant::now();
            // SAFETY：同上；返回空指针表示失败。
            let ptr = unsafe {
                las_create(
                    model.as_ptr(),
                    mmproj.as_ptr(),
                    gpu as c_int,
                    nt,
                    ntb,
                    N_CTX_INIT,
                    N_CTX,
                    info.as_mut_ptr() as *mut c_char,
                    info.len() as c_int,
                    err.as_mut_ptr() as *mut c_char,
                    err.len() as c_int,
                )
            };
            if !ptr.is_null() {
                let device = buf_str(&info);
                log::info!(
                    "[ACC-452] Qwen3-ASR llama.cpp engine loaded: device={} threads={}/{} ctx={}(max {}) in {}ms",
                    device,
                    nt,
                    ntb,
                    N_CTX_INIT,
                    N_CTX,
                    t0.elapsed().as_millis()
                );
                *COUNT_ENGINE.lock().unwrap_or_else(|e| e.into_inner()) = Some(CountHandle(ptr));
                return Ok(Self {
                    ptr,
                    lock: Mutex::new(()),
                    device,
                    aligner: None,
                });
            }
            last_err = buf_str(&err);
            log::warn!("[ACC-452] engine create failed (gpu={gpu}): {last_err}");
        }
        bail!("Qwen3-ASR llama.cpp engine create failed: {last_err}")
    }

    /// 实际使用的设备描述（如 `AMD Radeon 780M Graphics` / `CPU`）。
    pub(crate) fn device(&self) -> &str {
        &self.device
    }

    /// FORCED-ALIGN-456：挂载强制对齐器（只在本地实时调用；精确档批量识别不需要、不占显存）。
    /// 加载失败只记警告（接缝回落 371 比例估算，精解不受影响）。
    pub(crate) fn attach_aligner(&mut self, models_root: &Path) {
        match LlamaAligner::load(models_root) {
            Ok(a) => {
                log::info!("[ALIGN-456] aligner attached (device={})", a.device());
                self.aligner = Some(a);
            }
            Err(e) => log::warn!(
                "[ALIGN-456] aligner unavailable, seams fall back to ratio estimate: {e:#}"
            ),
        }
    }

    /// FORCED-ALIGN-456：对齐一段音频与其文字 ⇒ 每个字符（与 `text.chars()` 一一对应）的（起, 止）秒。
    /// 未挂载 / 失败 / 时间戳不合格 ⇒ `None`（调用方回落比例估算）。
    pub(crate) fn align(&self, samples: &[f32], text: &str) -> Option<AlignOut> {
        let al = self.aligner.as_ref()?;
        match al.align(samples, text) {
            Ok(o) => o,
            Err(e) => {
                log::warn!("[ALIGN-456] align failed: {e:#}");
                None
            }
        }
    }

    /// 解码一段 16k 单声道 PCM。
    /// - `system`：上下文 / 词库（原 sherpa per-stream `hotwords` 通道；`None` = 不加 system 消息）。
    /// - `language`：强制语种（Qwen3 官方取值 `Chinese` 等；原 sherpa `language` 通道）⇒ 预填
    ///   `language X<asr_text>`；`None` = 模型自判语种。
    /// - `max_new_tokens`：生成上限（`None` = [`DEFAULT_MAX_NEW_TOKENS`]）。
    /// - `draft`：草稿文字（推测解码；`None` = 关闭）。
    pub(crate) fn decode(
        &self,
        samples: &[f32],
        system: Option<&str>,
        language: Option<&str>,
        max_new_tokens: Option<i32>,
        assist: DecodeAssist<'_>,
    ) -> Result<LlamaDecodeOut> {
        if samples.is_empty() {
            return Ok(LlamaDecodeOut {
                raw: String::new(),
                stats: LasStats::default(),
            });
        }
        let prefix = language
            .map(|l| format!("language {l}<asr_text>"))
            .unwrap_or_default();
        self.decode_with_prefix(samples, system, &prefix, max_new_tokens, assist)
    }

    /// ACC-452 ④ 续写（**不采用**：452 回放 CER 8.09% vs 接缝拼接 5.20%、重复更多 ⇒ 仅供回放实验，测试编译）：assistant 预填 `language {language}<asr_text>{prior}`（`prior` = 本次录音已定稿文字末尾），
    /// 模型从音频中「prior 之后」的位置接着写。返回**只含新生成部分**的文本（去掉预填）+ 统计。
    #[cfg(test)]
    pub(crate) fn decode_continue(
        &self,
        samples: &[f32],
        system: Option<&str>,
        language: &str,
        prior: &str,
        max_new_tokens: Option<i32>,
        assist: DecodeAssist<'_>,
    ) -> Result<LlamaDecodeOut> {
        // 上文以句末标点结尾时，模型会判「已说完」直接吐结束符（452 回放：120 窗中 100 窗空）⇒ 预填前去掉末尾标点，
        // 让模型从「未完的句子」接着写（标点由模型按音频重新决定）。
        let prior = prior
            .trim_end_matches(|c: char| c.is_whitespace() || "。！？，、；：…,.!?;:".contains(c));
        let prefix = format!("language {language}<asr_text>{prior}");
        let o = self.decode_with_prefix(samples, system, &prefix, max_new_tokens, assist)?;
        let gen = o
            .raw
            .strip_prefix(prefix.as_str())
            .unwrap_or(&o.raw)
            .to_string();
        Ok(LlamaDecodeOut {
            raw: gen,
            stats: o.stats,
        })
    }

    fn decode_with_prefix(
        &self,
        samples: &[f32],
        system: Option<&str>,
        prefix: &str,
        max_new_tokens: Option<i32>,
        assist: DecodeAssist<'_>,
    ) -> Result<LlamaDecodeOut> {
        let sys = system.map(cstr).transpose()?;
        let pre = cstr(prefix)?;
        let dr = assist
            .draft
            .filter(|d| !d.is_empty())
            .map(cstr)
            .transpose()?;
        let mut out = vec![0u8; 64 * 1024];
        let mut err = [0u8; 512];
        let mut st = LasStats::default();
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        // ⑤ 逐字回调：把 `&dyn Fn` 的地址交给 shim，经 `partial_trampoline` 转回（调用期间栈上存活）。
        let cb_ref: Option<&(dyn Fn(&str) + Sync)> = assist.on_partial;
        let (cb_fn, cb_user): (
            Option<extern "C" fn(*mut c_void, *const c_char)>,
            *mut c_void,
        ) = match cb_ref.as_ref() {
            Some(r) => (
                Some(partial_trampoline),
                r as *const &(dyn Fn(&str) + Sync) as *mut c_void,
            ),
            None => (None, std::ptr::null_mut()),
        };
        // SAFETY：指针均有效且在调用期间存活；out / err 为可写缓冲；空 prefix ⇒ shim 视为不预填。
        let rc = unsafe {
            las_decode(
                self.ptr,
                samples.as_ptr(),
                samples.len() as c_int,
                sys.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                pre.as_ptr(),
                dr.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                max_new_tokens.unwrap_or(DEFAULT_MAX_NEW_TOKENS),
                if dr.is_some() { DRAFT_MAX } else { 0 },
                out.as_mut_ptr() as *mut c_char,
                out.len() as c_int,
                &mut st,
                err.as_mut_ptr() as *mut c_char,
                err.len() as c_int,
                cb_fn,
                cb_user,
            )
        };
        if rc != 0 {
            bail!("llama.cpp decode failed ({rc}): {}", buf_str(&err));
        }
        Ok(LlamaDecodeOut {
            raw: buf_str(&out),
            stats: st,
        })
    }
}

// ============================================================================================
// FORCED-ALIGN-456：Qwen3-ForcedAligner-0.6B 强制对齐器（音频 + 精解文字 ⇒ 每个字的起止秒）。
//
// Gavin 2026-09-29「就上 0.6B」「对齐模型要作为本地实时模式的标配下载（和 1.7B 模型一起）」。
// 模型 = 42ailab/Qwen3-ForcedAligner-0.6B-GGUF 三件套（主干本地转 Q8_0）；POC-455 实测前后窗同一字时间差
// 中位 0.03s、98.4% ≤0.1s。与精解引擎共用同一套 llama.cpp 运行库（shim `las_align_*`）。
// ============================================================================================

/// 对齐模型目录（`models/` 下）与文件名。
pub(crate) const ALIGNER_SUBDIR: &str = "qwen3-forcedaligner-0.6b-gguf";
pub(crate) const ALIGNER_BACKBONE_FILE: &str = "aligner-backbone-q8_0.gguf";
pub(crate) const ALIGNER_MMPROJ_FILE: &str = "aligner-mmproj-q8_0.gguf";
pub(crate) const ALIGNER_HEAD_FILE: &str = "aligner-head.bin";
/// 对齐上下文：音频 12.5 token/s + 每字（字 + 2 槽）≈3 token；本地实时窗 ≤ 约 20s ⇒ 远小于 2048。
const ALIGNER_N_CTX: i32 = 1024;

/// 对齐用「单元」：汉字 / 假名等逐字；连续 ASCII 字母数字为一个词；标点与空白不入单元。
/// 返回 (单元文字, 单元对应的字符下标区间 [起, 止))。
pub(crate) fn align_units(text: &str) -> Vec<(String, std::ops::Range<usize>)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphanumeric() {
            let s = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '\'') {
                i += 1;
            }
            out.push((chars[s..i].iter().collect(), s..i));
        } else if c.is_alphanumeric() {
            out.push((c.to_string(), i..i + 1));
            i += 1;
        } else {
            i += 1;
        }
    }
    out
}

/// 单元起止 → 每个字符的起止秒：单元内各字同一时间；标点 / 空白取前一内容字（开头取第一个单元）。
pub(crate) fn char_times_from_units(
    text: &str,
    units: &[(String, std::ops::Range<usize>)],
    spans: &[(f32, f32)],
) -> Vec<(f32, f32)> {
    let n = text.chars().count();
    let mut out = vec![(0.0f32, 0.0f32); n];
    let mut last = spans.first().copied().unwrap_or((0.0, 0.0));
    let mut ui = 0usize;
    for (i, slot) in out.iter_mut().enumerate() {
        while ui < units.len() && units[ui].1.end <= i {
            ui += 1;
        }
        if ui < units.len() && units[ui].1.contains(&i) {
            last = spans[ui];
        }
        *slot = last;
    }
    out
}

/// 时间戳是否可用：非降（shim 已修复，这里兜底再验）且非「多数塌成零长度」。
pub(crate) fn spans_valid(spans: &[(f32, f32)]) -> bool {
    if spans.is_empty() {
        return false;
    }
    if spans.windows(2).any(|w| w[1].0 + 1e-4 < w[0].0) {
        return false;
    }
    let zero = spans.iter().filter(|(s, e)| e - s <= 1e-4).count();
    !(spans.len() >= 4 && zero * 2 > spans.len())
}

/// 一次对齐的输出：每个字符的（起, 止）秒（与 `text.chars()` 一一对应）+ 耗时。
pub(crate) struct AlignOut {
    pub char_times: Vec<(f32, f32)>,
    /// 毫秒：[总, 音频编码, 主干前向, 时间戳头]。
    pub ms: [f64; 4],
}

/// 对齐器。内部串行（与精解同线程顺序调用）。
pub(crate) struct LlamaAligner {
    ptr: *mut c_void,
    lock: Mutex<()>,
    device: String,
}

// SAFETY：句柄只经 `lock` 串行访问；llama.cpp 上下文可在线程间移动（单线程使用）。
unsafe impl Send for LlamaAligner {}
unsafe impl Sync for LlamaAligner {}

impl Drop for LlamaAligner {
    fn drop(&mut self) {
        // SAFETY：ptr 由 las_align_create 返回且只在此处释放一次。
        unsafe { las_align_free(self.ptr) };
    }
}

impl LlamaAligner {
    /// 三件套是否齐全（`<models>/qwen3-forcedaligner-0.6b-gguf/`）。
    pub(crate) fn model_ready(models_root: &Path) -> (bool, PathBuf) {
        let dir = models_root.join(ALIGNER_SUBDIR);
        let ok = [
            ALIGNER_BACKBONE_FILE,
            ALIGNER_MMPROJ_FILE,
            ALIGNER_HEAD_FILE,
        ]
        .iter()
        .all(|f| dir.join(f).is_file());
        (ok, dir)
    }

    /// 加载：优先 GPU，失败回落 CPU（同精解引擎）。
    pub(crate) fn load(models_root: &Path) -> Result<Self> {
        let (ok, dir) = Self::model_ready(models_root);
        if !ok {
            bail!("forced aligner model not found under {}", dir.display());
        }
        ensure_runtime()?;
        let backbone = cstr(&dir.join(ALIGNER_BACKBONE_FILE).to_string_lossy())?;
        let mmproj = cstr(&dir.join(ALIGNER_MMPROJ_FILE).to_string_lossy())?;
        let head = cstr(&dir.join(ALIGNER_HEAD_FILE).to_string_lossy())?;
        let mut last_err = String::new();
        for gpu in [true, false] {
            let (_, ntb) = thread_plan(gpu);
            let mut info = [0u8; 256];
            let mut err = [0u8; 512];
            let t0 = std::time::Instant::now();
            // SAFETY：指针有效、缓冲可写；返回空指针表示失败。
            let ptr = unsafe {
                las_align_create(
                    backbone.as_ptr(),
                    mmproj.as_ptr(),
                    head.as_ptr(),
                    gpu as c_int,
                    ntb,
                    ALIGNER_N_CTX,
                    info.as_mut_ptr() as *mut c_char,
                    info.len() as c_int,
                    err.as_mut_ptr() as *mut c_char,
                    err.len() as c_int,
                )
            };
            if !ptr.is_null() {
                let al = Self {
                    ptr,
                    lock: Mutex::new(()),
                    device: buf_str(&info),
                };
                // 预热：首次前向会现场编译 GPU 着色器（实测首窗 2.5s），加载时用 1s 静音 + 1 字先跑一次。
                let warm = al.align(&[0.0f32; 16000], "嗯").is_ok();
                log::info!(
                    "[ALIGN-456] forced aligner loaded: device={} ctx={} warm={} in {}ms",
                    al.device,
                    ALIGNER_N_CTX,
                    warm,
                    t0.elapsed().as_millis()
                );
                return Ok(al);
            }
            last_err = buf_str(&err);
            log::warn!("[ALIGN-456] aligner create failed (gpu={gpu}): {last_err}");
        }
        bail!("forced aligner create failed: {last_err}")
    }

    pub(crate) fn device(&self) -> &str {
        &self.device
    }

    /// 对齐一段 16k 单声道 PCM 与其文字。无内容单元 ⇒ `Ok(None)`；时间戳不合格 ⇒ `Ok(None)`（调用方回落）。
    pub(crate) fn align(&self, samples: &[f32], text: &str) -> Result<Option<AlignOut>> {
        let units = align_units(text);
        if units.is_empty() || samples.is_empty() {
            return Ok(None);
        }
        let joined = cstr(
            &units
                .iter()
                .map(|(u, _)| u.as_str())
                .collect::<Vec<_>>()
                .join("\u{1f}"),
        )?;
        let n = units.len();
        let mut st = vec![0f32; n];
        let mut en = vec![0f32; n];
        let mut ms = [0f64; 4];
        let mut err = [0u8; 512];
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY：缓冲长度 = n_units；指针在调用期间有效。
        let rc = unsafe {
            las_align(
                self.ptr,
                samples.as_ptr(),
                samples.len() as c_int,
                joined.as_ptr(),
                n as c_int,
                st.as_mut_ptr(),
                en.as_mut_ptr(),
                ms.as_mut_ptr(),
                err.as_mut_ptr() as *mut c_char,
                err.len() as c_int,
            )
        };
        if rc != 0 {
            bail!("forced align failed ({rc}): {}", buf_str(&err));
        }
        let spans: Vec<(f32, f32)> = st.into_iter().zip(en).collect();
        if !spans_valid(&spans) {
            return Ok(None);
        }
        Ok(Some(AlignOut {
            char_times: char_times_from_units(text, &units, &spans),
            ms,
        }))
    }
}

#[cfg(test)]
mod align456_tests {
    use super::{align_units, char_times_from_units, spans_valid};

    #[test]
    fn t456_units_cjk_per_char_ascii_per_word_punct_skipped() {
        let u = align_units("我用 Claude 写v2代码，OK！");
        let words: Vec<&str> = u.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(words, ["我", "用", "Claude", "写", "v2", "代", "码", "OK"]);
        assert_eq!(u[2].1, 3..9);
    }

    #[test]
    fn t456_char_times_punct_takes_previous() {
        let text = "你好，世界。";
        let u = align_units(text);
        let spans = [(0.1, 0.2), (0.2, 0.3), (1.0, 1.1), (1.1, 1.2)];
        let t = char_times_from_units(text, &u, &spans);
        assert_eq!(t.len(), 6);
        assert_eq!(t[2], (0.2, 0.3), "逗号取前一字");
        assert_eq!(t[5], (1.1, 1.2), "句号取前一字");
        let lead = char_times_from_units("「好", &align_units("「好"), &[(0.5, 0.6)]);
        assert_eq!(lead[0], (0.5, 0.6), "开头标点取第一个单元");
    }

    #[test]
    fn t456_spans_valid_rejects_backwards_and_collapsed() {
        assert!(spans_valid(&[(0.0, 0.1), (0.1, 0.2), (0.3, 0.4)]));
        assert!(!spans_valid(&[(0.5, 0.6), (0.1, 0.2)]), "倒退");
        assert!(!spans_valid(&[(1.0, 1.0); 5]), "塌成零长度");
        assert!(!spans_valid(&[]));
    }
}
