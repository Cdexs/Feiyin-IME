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

/// 上下文长度：音频（~12.5 token/s）+ 上下文 / 词库 + 生成。与原 sherpa `max_total_len=4096` 同额度。
const N_CTX: i32 = 4096;
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
        info: *mut c_char,
        info_len: c_int,
        err: *mut c_char,
        err_len: c_int,
    ) -> *mut c_void;
    fn las_free(e: *mut c_void);
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
}

// SAFETY：引擎句柄只经 `lock` 串行访问；llama.cpp 上下文可在线程间移动（单线程使用）。
unsafe impl Send for LlamaAsr {}
unsafe impl Sync for LlamaAsr {}

impl Drop for LlamaAsr {
    fn drop(&mut self) {
        // SAFETY：ptr 由 las_create 返回且只在此处释放一次。
        unsafe { las_free(self.ptr) };
    }
}

impl LlamaAsr {
    /// 模型文件所在目录（`<models>/qwen3-asr-1.7b-gguf/`）是否齐全。
    pub(crate) fn model_ready(models_root: &Path) -> (bool, PathBuf) {
        let dir = models_root.join(LLAMA_ASR_MODEL_SUBDIR);
        let ok =
            dir.join(LLAMA_ASR_MODEL_FILE).is_file() && dir.join(LLAMA_ASR_MMPROJ_FILE).is_file();
        (ok, dir)
    }

    /// 加载引擎：优先 GPU（Vulkan / Metal），GPU 不可用或创建失败 ⇒ 自动回落 CPU。
    pub(crate) fn load(models_root: &Path) -> Result<Self> {
        let (ok, dir) = Self::model_ready(models_root);
        if !ok {
            bail!("Qwen3-ASR GGUF model not found under {}", dir.display());
        }
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
        let model = cstr(&dir.join(LLAMA_ASR_MODEL_FILE).to_string_lossy())?;
        let mmproj = cstr(&dir.join(LLAMA_ASR_MMPROJ_FILE).to_string_lossy())?;
        let mut last_err = String::new();
        for gpu in [true, false] {
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
                    "[ACC-452] Qwen3-ASR llama.cpp engine loaded: device={} threads={}/{} ctx={} in {}ms",
                    device,
                    nt,
                    ntb,
                    N_CTX,
                    t0.elapsed().as_millis()
                );
                return Ok(Self {
                    ptr,
                    lock: Mutex::new(()),
                    device,
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
