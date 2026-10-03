// ACC-ENGINE-LLAMACPP-452：Qwen3-ASR 1.7B 精解引擎的 llama.cpp 适配层（C++，由 build.rs 经 cc 编译进主程序）。
//
// 设计要点：
// - **运行时动态加载**官方预编译库（Windows: LoadLibrary / macOS: dlopen），函数指针类型取自同版本（b11207）
//   官方头文件（decltype），结构体布局由编译器保证与 DLL 一致；库缺失时主程序照常启动，las_load 返回错误。
// - 只暴露 5 个扁平 C 接口给 Rust（las_load / las_create / las_decode / las_free / las_set_log）。
// - 提示词按模型内置 ChatML 模板：[system 上下文] + user(音频标记) + assistant 预填(prefix)。
// - 贪心逐字生成；可选「草稿推测解码」：草稿 token 由调用方给的文字（如预览）n-gram 查表得出，
//   目标模型一次批量验证，只接受与贪心 argmax 一致的前缀 ⇒ 输出与逐字贪心一致（仅省时间）。

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <string>
#include <thread>
#include <vector>

#include "llama.h"
#include "mtmd.h"
#include "mtmd-helper.h"

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
typedef HMODULE lib_t;
static lib_t lib_open(const std::string & dir, const char * name) {
    std::string p = dir + "\\" + name + ".dll";
    return LoadLibraryExA(p.c_str(), nullptr, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
}
static void * lib_sym(lib_t h, const char * s) { return (void *) GetProcAddress(h, s); }
#else
#include <dlfcn.h>
typedef void * lib_t;
static lib_t lib_open(const std::string & dir, const char * name) {
    std::string p = dir + "/lib" + name + ".dylib";
    return dlopen(p.c_str(), RTLD_NOW | RTLD_GLOBAL);
}
static void * lib_sym(lib_t h, const char * s) { return dlsym(h, s); }
#endif

// ---- 函数指针表（类型由官方头文件推导）----
#define LAS_FN(name) static decltype(&name) p_##name = nullptr;
#define LAS_FNS(X) \
    X(ggml_backend_load_all_from_path) X(ggml_backend_dev_count) X(ggml_backend_dev_get) \
    X(ggml_backend_dev_type) X(ggml_backend_dev_description) X(ggml_log_set) \
    X(llama_backend_init) X(llama_log_set) X(llama_model_default_params) X(llama_context_default_params) \
    X(llama_model_load_from_file) X(llama_model_free) X(llama_init_from_model) X(llama_free) \
    X(llama_model_get_vocab) X(llama_vocab_n_tokens) X(llama_vocab_is_eog) X(llama_token_to_piece) \
    X(llama_tokenize) X(llama_get_memory) X(llama_memory_clear) X(llama_memory_seq_rm) \
    X(llama_batch_get_one) X(llama_batch_init) X(llama_batch_free) X(llama_decode) X(llama_get_logits_ith) \
    X(mtmd_context_params_default) X(mtmd_init_from_file) X(mtmd_free) X(mtmd_default_marker) \
    X(mtmd_bitmap_init_from_audio) X(mtmd_bitmap_free) X(mtmd_input_chunks_init) X(mtmd_input_chunks_free) \
    X(mtmd_tokenize) X(mtmd_helper_eval_chunks) X(mtmd_helper_log_set) \
    /* FORCED-ALIGN-456：对齐器（embeddings / 逐 chunk 解码）*/ \
    X(llama_model_n_embd) X(llama_model_n_embd_inp) X(llama_get_embeddings_ith) X(llama_set_causal_attn) \
    X(mtmd_input_chunks_size) X(mtmd_input_chunks_get) X(mtmd_input_chunk_get_type) \
    X(mtmd_input_chunk_get_n_tokens) X(mtmd_input_chunk_get_tokens_text) X(mtmd_input_chunk_get_n_pos) \
    X(mtmd_encode_chunk) X(mtmd_get_output_embd) X(mtmd_decode_use_non_causal) X(mtmd_support_audio) \
    X(mtmd_helper_decode_image_chunk) X(llama_set_embeddings) X(mtmd_helper_eval_chunk_single)
LAS_FNS(LAS_FN)

typedef void (*las_log_fn)(int level, const char * text);
// 逐字回调（ACC-452 ⑤ 流式回灌）：每轮生成后回调「预填 + 截至目前的生成内容」。
typedef void (*las_partial_fn)(void * user, const char * text);
static las_log_fn g_log = nullptr;
static void log_cb(enum ggml_log_level level, const char * text, void *) {
    // MEM-469 调试：LAS_LOG_INFO=1（须在进程启动前设置）时 INFO / CONT 原样写 stderr（看各后端缓冲区大小）。
    static const bool info = [] { const char * v = getenv("LAS_LOG_INFO"); return v && *v == '1'; }();
    if (info && text && (level == GGML_LOG_LEVEL_INFO || level == GGML_LOG_LEVEL_CONT)) { fputs(text, stderr); return; }
    // 只转发 WARN / ERROR（INFO/DEBUG 刷屏）。
    if (g_log && text && (level == GGML_LOG_LEVEL_WARN || level == GGML_LOG_LEVEL_ERROR)) g_log((int) level, text);
}
static void set_err(char * err, int n, const std::string & s) {
    if (err && n > 0) { snprintf(err, (size_t) n, "%s", s.c_str()); }
}


extern "C" void las_set_log(las_log_fn f) { g_log = f; }

// 加载 dir 下的 ggml-base / ggml / llama / mtmd，并让 ggml 在 dir 下按硬件挑选后端（Vulkan / Metal / 最优 CPU 变体）。
// 返回 0 = 成功；非 0 = 失败（err 写原因）。可重复调用（已加载则直接返回 0）。
extern "C" int las_load(const char * dir, char * err, int err_len) {
    static bool loaded = false;
    if (loaded) return 0;
    std::string d(dir ? dir : ".");
    lib_t h_base = lib_open(d, "ggml-base");
    lib_t h_ggml = lib_open(d, "ggml");
    lib_t h_llama = lib_open(d, "llama");
    lib_t h_mtmd = lib_open(d, "mtmd");
    if (!h_base || !h_ggml || !h_llama || !h_mtmd) {
        set_err(err, err_len, "llama.cpp runtime libraries not found in " + d);
        return 1;
    }
    lib_t libs[] = {h_mtmd, h_llama, h_ggml, h_base};
    std::string missing;
#define LAS_RESOLVE(name) \
    for (lib_t h : libs) { if (!p_##name) p_##name = (decltype(&name)) lib_sym(h, #name); } \
    if (!p_##name) missing += std::string(#name) + " ";
    LAS_FNS(LAS_RESOLVE)
    if (!missing.empty()) {
        set_err(err, err_len, "missing symbols: " + missing);
        return 2;
    }
    p_llama_log_set(log_cb, nullptr);
    p_ggml_log_set(log_cb, nullptr);
    p_mtmd_helper_log_set(log_cb, nullptr);
    p_ggml_backend_load_all_from_path(d.c_str());
    p_llama_backend_init();
    loaded = true;
    return 0;
}

// LLAMA-TUNE-458：调试开关（回放 A/B 用，不进配置；未设 = 生产默认值）。
// Windows 上必须用 GetEnvironmentVariableA：进程运行中由 Rust `std::env::set_var`（SetEnvironmentVariableW）
// 改的值，CRT `getenv` 读的是启动时的副本、看不到（[CRT-GETENV-STALE-458]）。
static bool las_env_get(const char * k, std::string & out) {
#ifdef _WIN32
    char buf[64];
    const DWORD n = GetEnvironmentVariableA(k, buf, (DWORD) sizeof(buf));
    if (n == 0 || n >= sizeof(buf)) return false;
    out.assign(buf, n);
    return true;
#else
    const char * v = std::getenv(k);
    if (!v) return false;
    out = v;
    return true;
#endif
}
static bool las_env_set(const char * k) {
    std::string v;
    return las_env_get(k, v);
}
static int las_env_int(const char * k, int def) {
    std::string v;
    return (las_env_get(k, v) && !v.empty()) ? atoi(v.c_str()) : def;
}

struct las_engine {
    llama_model * model = nullptr;
    llama_context * ctx = nullptr;
    mtmd_context * mctx = nullptr;
    const llama_vocab * vocab = nullptr;
    int n_vocab = 0;
    int n_batch = 512;
    // MEM-TRIM-457 ②：上下文按需分配 —— 先按 las_create 的 n_ctx 建，放不下时重建为更大（≤ n_ctx_max）。
    llama_context_params cp{};
    int n_ctx_max = 4096;
    // PIPE-SPEED-457 ③：首个文字段（system + 词库 + user 头）的 token 与其 KV 长度；下窗相同则复用，不重算。
    std::vector<llama_token> prefix_tokens;
    llama_pos prefix_pos = 0;
    // DRAFT-LANG-PREV-458：上一次模型自写的语种标记（`language X<asr_text>` 的 token），作下一窗草稿开头。
    std::vector<llama_token> lang_draft;
    llama_token asr_text_tok = -1;
};

struct las_stats {
    double prefill_ms;
    double gen_ms;
    int32_t n_prompt;
    int32_t n_gen;
    int32_t n_draft_accepted;
    int32_t n_draft_proposed;
    // LLAMA-TUNE-458：音频编码耗时（仅 LAS_SPLIT_ENC 调试开关下拆分计时；否则 0，含在 prefill_ms 里）。
    double enc_ms;
};

// 创建引擎。gpu=1：模型与音频编码器全部放 GPU（有可用 GPU 设备时）；gpu=0：纯 CPU。
// info 写入实际使用的设备描述。返回 nullptr = 失败（err 写原因）。
extern "C" las_engine * las_create(const char * model_path, const char * mmproj_path, int gpu, int n_threads,
                        int n_threads_batch, int n_ctx, int n_ctx_max, char * info, int info_len, char * err,
                        int err_len) {
    // 设备：gpu=1 时选第一块 GPU / iGPU 设备。
    ggml_backend_dev_t gpu_dev = nullptr;
    std::string dev_desc = "CPU";
    if (gpu) {
        for (size_t i = 0; i < p_ggml_backend_dev_count(); i++) {
            ggml_backend_dev_t dv = p_ggml_backend_dev_get(i);
            enum ggml_backend_dev_type t = p_ggml_backend_dev_type(dv);
            if (t == GGML_BACKEND_DEVICE_TYPE_GPU || t == GGML_BACKEND_DEVICE_TYPE_IGPU) {
                gpu_dev = dv;
                dev_desc = p_ggml_backend_dev_description(dv);
                break;
            }
        }
    }
    ggml_backend_dev_t devs[2] = {gpu_dev, nullptr};

    llama_model_params mp = p_llama_model_default_params();
    mp.n_gpu_layers = gpu_dev ? 999 : 0;
    mp.devices = gpu_dev ? devs : nullptr;
    llama_model * model = p_llama_model_load_from_file(model_path, mp);
    if (!model) { set_err(err, err_len, std::string("load model failed: ") + model_path); return nullptr; }

    llama_context_params cp = p_llama_context_default_params();
    cp.n_ctx = (uint32_t) n_ctx;
    cp.n_batch = 512;
    // MEM-453：ubatch 512→128 计算缓冲 305→76MiB，KV f16→q8_0 448→238MiB（容量仍 4096）。
    // 780M 实测预填充 ≤+3%、生成不变；28 片回放 ub128 / kvq8 均与基线逐字一致。
    cp.n_ubatch = (uint32_t) las_env_int("LAS_UBATCH", 128);
    cp.type_k = GGML_TYPE_Q8_0;
    cp.type_v = GGML_TYPE_Q8_0;
    cp.n_seq_max = 1;
    cp.n_threads = n_threads;
    cp.n_threads_batch = n_threads_batch;
    cp.flash_attn_type = LLAMA_FLASH_ATTN_TYPE_ENABLED;
    cp.no_perf = true;
    // MEM-TRIM-457 ①：输出上限 = 草稿验证最大批（1 + draft_max ≤ 9）的余量 16；预填充只取末 token。
    // 计算缓冲按上限预留 15 万维 logits ⇒ 512 → 16 省 ~65MiB，结果与速度不变。
    cp.n_outputs_max = (uint32_t) (std::max)(16, las_env_int("LAS_DRAFT_MAX", 0) + 1); // 调试 LAS_DRAFT_MAX>15 时放宽
    llama_context * ctx = p_llama_init_from_model(model, cp);
    if (!ctx) { p_llama_model_free(model); set_err(err, err_len, "init context failed"); return nullptr; }

    mtmd_context_params mcp = p_mtmd_context_params_default();
    mcp.use_gpu = gpu_dev != nullptr;
    mcp.device = gpu_dev;
    mcp.n_threads = las_env_int("LAS_MTMD_THREADS", n_threads_batch);
    mcp.print_timings = false;
    mcp.flash_attn_type = LLAMA_FLASH_ATTN_TYPE_ENABLED;
    // MEM-ENC-470（Gavin 10-03 选 A）：编码器不按 30s 最长音频预留计算缓冲（按实际音频长度按需分配，
    // 同对齐器 456）。86 窗回放：文字逐字相同，每窗 +5.5ms（≈1%），显存省 ≈0.4G。调试 LAS_ENC_WARMUP=1 恢复预留。
    mcp.warmup = las_env_int("LAS_ENC_WARMUP", 0) != 0;
    mtmd_context * mctx = p_mtmd_init_from_file(mmproj_path, model, mcp);
    if (!mctx) {
        p_llama_free(ctx); p_llama_model_free(model);
        set_err(err, err_len, std::string("load mmproj failed: ") + mmproj_path);
        return nullptr;
    }
    las_engine * e = new las_engine();
    e->model = model; e->ctx = ctx; e->mctx = mctx;
    e->cp = cp;
    e->n_ctx_max = n_ctx_max > n_ctx ? n_ctx_max : n_ctx;
    e->vocab = p_llama_model_get_vocab(model);
    e->n_vocab = p_llama_vocab_n_tokens(e->vocab);
    set_err(info, info_len, dev_desc);
    return e;
}

extern "C" void las_free(las_engine * e) {
    if (!e) return;
    if (e->mctx) p_mtmd_free(e->mctx);
    if (e->ctx) p_llama_free(e->ctx);
    if (e->model) p_llama_model_free(e->model);
    delete e;
}

static int argmax(const float * logits, int n) {
    int best = 0;
    float bv = logits[0];
    for (int i = 1; i < n; i++) { if (logits[i] > bv) { bv = logits[i]; best = i; } }
    return best;
}

static std::vector<llama_token> tokenize(const llama_vocab * v, const std::string & s) {
    if (s.empty()) return {};
    int n = -p_llama_tokenize(v, s.c_str(), (int32_t) s.size(), nullptr, 0, false, false);
    std::vector<llama_token> out((size_t) (n > 0 ? n : 0));
    if (n > 0) p_llama_tokenize(v, s.c_str(), (int32_t) s.size(), out.data(), n, false, false);
    return out;
}

static void append_piece(const llama_vocab * v, llama_token t, std::string & out) {
    char buf[256];
    int n = p_llama_token_to_piece(v, t, buf, sizeof(buf), 0, true);
    if (n > 0) out.append(buf, (size_t) n);
}

// 草稿查表：在 draft 里找与 hist 末尾 n-gram（n=3→1）匹配的最晚位置，返回其后最多 k 个 token。
static std::vector<llama_token> lookup_draft(const std::vector<llama_token> & draft,
                                             const std::vector<llama_token> & hist, int k) {
    for (int n = 3; n >= 1; n--) {
        if ((int) hist.size() < n) continue;
        for (int i = (int) draft.size() - n; i >= 0; i--) {
            bool ok = true;
            for (int j = 0; j < n; j++) {
                if (draft[(size_t) i + j] != hist[hist.size() - n + j]) { ok = false; break; }
            }
            if (ok) {
                int s = i + n;
                int e = s + k < (int) draft.size() ? s + k : (int) draft.size();
                if (s < e) return std::vector<llama_token>(draft.begin() + s, draft.begin() + e);
            }
        }
    }
    return {};
}

// 解码一段 16k 单声道 PCM。
// system：上下文 / 词库（nullptr 或空 = 不加 system 消息）；prefix：assistant 预填（如 "language Chinese<asr_text>"）；
// draft：草稿文字（nullptr 或空 = 关闭推测解码）；max_new_tokens：生成上限。
// 输出 out = prefix + 生成内容（原样，含 "language X<asr_text>" 等特殊 token 文本）。返回 0 = 成功。
extern "C" int las_decode(las_engine * e, const float * pcm, int n_samples, const char * system, const char * prefix,
               const char * draft, int max_new_tokens, int draft_max, char * out, int out_len,
               las_stats * st, char * err, int err_len, las_partial_fn on_partial, void * user) {
    using clk = std::chrono::steady_clock;
    las_stats s{};
    if (draft_max > 0) draft_max = las_env_int("LAS_DRAFT_MAX", draft_max);
    const bool split_enc = las_env_set("LAS_SPLIT_ENC");
    std::string prompt;
    if (system && *system) prompt += std::string("<|im_start|>system\n") + system + "<|im_end|>\n";
    prompt += std::string("<|im_start|>user\n") + p_mtmd_default_marker() + "<|im_end|>\n<|im_start|>assistant\n";
    std::string pre = prefix ? prefix : "";
    prompt += pre;

    auto t0 = clk::now();
    mtmd_bitmap * bmp = p_mtmd_bitmap_init_from_audio((size_t) n_samples, pcm);
    mtmd_input_chunks * chunks = p_mtmd_input_chunks_init();
    mtmd_input_text txt{prompt.c_str(), prompt.size(), true, true};
    const mtmd_bitmap * bmps[1] = {bmp};
    int32_t rc = p_mtmd_tokenize(e->mctx, chunks, &txt, bmps, 1);
    if (rc != 0) {
        p_mtmd_input_chunks_free(chunks); p_mtmd_bitmap_free(bmp);
        set_err(err, err_len, "mtmd_tokenize failed: " + std::to_string(rc));
        return 1;
    }
    const size_t n_chunks = p_mtmd_input_chunks_size(chunks);
    // MEM-TRIM-457 ②：本次所需上下文 = 提示 + 生成上限 + 草稿余量；放不下 ⇒ 重建更大的上下文（封顶 n_ctx_max，
    // 与原固定 4096 同上限；更大仍放不下则与原来一样由解码报错）。重建会丢前缀缓存。
    {
        size_t need = (size_t) max_new_tokens + (size_t) draft_max + 16;
        for (size_t i = 0; i < n_chunks; i++) need += p_mtmd_input_chunk_get_n_pos(p_mtmd_input_chunks_get(chunks, i));
        const uint32_t cur = e->cp.n_ctx;
        if (need > cur && cur < (uint32_t) e->n_ctx_max) {
            uint32_t n = cur;
            while (n < need && n < (uint32_t) e->n_ctx_max) n *= 2;
            if (n > (uint32_t) e->n_ctx_max) n = (uint32_t) e->n_ctx_max;
            llama_context_params cp = e->cp;
            cp.n_ctx = n;
            llama_context * nc = p_llama_init_from_model(e->model, cp);
            if (nc) {
                p_llama_free(e->ctx);
                e->ctx = nc;
                e->cp = cp;
                e->prefix_tokens.clear();
                e->prefix_pos = 0;
                if (g_log) g_log((int) GGML_LOG_LEVEL_WARN, ("[MEM-TRIM-457] llama context grown to " + std::to_string(n) + "\n").c_str());
            }
        }
    }
    llama_memory_t mem = p_llama_get_memory(e->ctx);
    llama_pos n_past = 0;
    size_t first = 0;
    // PIPE-SPEED-457 ③：首个文字段（system + 词库 + user 头）与上一窗 token 完全相同 ⇒ 保留其 KV，只删其后；
    // 同样 token、同样位置 ⇒ KV 相同 ⇒ 结果不变，省掉该段预填充（回放 120 窗解码 −7.4%；1 窗差 1 字经取证是
    // GPU 自身浮动：同窗不复用连解 3 次也出 2 种结果，见 replay436_tests::poc457_prefix_determinism）。
    const mtmd_input_chunk * c0 = n_chunks > 0 ? p_mtmd_input_chunks_get(chunks, 0) : nullptr;
    std::vector<llama_token> c0_tokens;
    if (c0 && p_mtmd_input_chunk_get_type(c0) == MTMD_INPUT_CHUNK_TYPE_TEXT && n_chunks > 1) {
        size_t nt = 0;
        const llama_token * tk = p_mtmd_input_chunk_get_tokens_text(c0, &nt);
        c0_tokens.assign(tk, tk + nt);
    }
    // 调试开关 LAS_NO_PREFIX_CACHE（回放 A/B 用）：每窗都从头预填充。
    if (!c0_tokens.empty() && c0_tokens == e->prefix_tokens && e->prefix_pos > 0 && !las_env_set("LAS_NO_PREFIX_CACHE")) {
        p_llama_memory_seq_rm(mem, 0, e->prefix_pos, -1);
        n_past = e->prefix_pos;
        first = 1;
    } else {
        p_llama_memory_clear(mem, true);
        e->prefix_tokens.clear();
        e->prefix_pos = 0;
    }
    rc = 0;
    for (size_t i = first; i < n_chunks && rc == 0; i++) {
        llama_pos np = n_past;
        const mtmd_input_chunk * ch = p_mtmd_input_chunks_get(chunks, i);
        if (split_enc && p_mtmd_input_chunk_get_type(ch) != MTMD_INPUT_CHUNK_TYPE_TEXT) {
            // 调试：编码与写入 KV 分开计时（与 eval_chunk_single 内部同一顺序）。
            auto te = clk::now();
            rc = p_mtmd_encode_chunk(e->mctx, ch);
            s.enc_ms += std::chrono::duration<double, std::milli>(clk::now() - te).count();
            if (rc == 0)
                rc = p_mtmd_helper_decode_image_chunk(e->mctx, e->ctx, ch, p_mtmd_get_output_embd(e->mctx), n_past, 0,
                                                      e->n_batch, &np, nullptr, nullptr);
        } else {
            rc = p_mtmd_helper_eval_chunk_single(e->mctx, e->ctx, ch, n_past, 0, e->n_batch, i + 1 == n_chunks, &np);
        }
        n_past = np;
        if (rc == 0 && i == 0 && !c0_tokens.empty()) {
            e->prefix_tokens = c0_tokens;
            e->prefix_pos = n_past;
        }
    }
    p_mtmd_input_chunks_free(chunks);
    p_mtmd_bitmap_free(bmp);
    if (rc != 0) {
        e->prefix_tokens.clear();
        e->prefix_pos = 0;
        set_err(err, err_len, "eval prompt failed: " + std::to_string(rc));
        return 2;
    }
    s.n_prompt = (int32_t) n_past;
    auto t1 = clk::now();

    std::vector<llama_token> dtoks = (draft && *draft && draft_max > 0) ? tokenize(e->vocab, draft) : std::vector<llama_token>{};
    // DRAFT-LANG-PREV-458：模型每窗先自写语种标记（`language Chinese<asr_text>`，3~4 个 token）⇒ 草稿开头补上它，
    // 这几步也能被草稿命中（逐位贪心验证，猜错只是不采纳，不改输出）。用上一次实际生成的标记（说英文的用户自动
    // 变成 English），首次默认 Chinese。回放：连同上一窗精解文字作草稿，生成步数 −28%、每窗 −80ms、CER 不变。
    // 调试开关 LAS_NO_DRAFT_LANG 关闭（回放 A/B 用）。
    if (!dtoks.empty() && pre.empty() && !las_env_set("LAS_NO_DRAFT_LANG")) {
        if (e->lang_draft.empty()) {
            static const char * lp = "language Chinese<asr_text>";
            const int32_t nl = -p_llama_tokenize(e->vocab, lp, (int32_t) strlen(lp), nullptr, 0, false, true);
            if (nl > 0) {
                e->lang_draft.resize((size_t) nl);
                p_llama_tokenize(e->vocab, lp, (int32_t) strlen(lp), e->lang_draft.data(), nl, false, true);
                e->asr_text_tok = e->lang_draft.back();
            }
        }
        dtoks.insert(dtoks.begin(), e->lang_draft.begin(), e->lang_draft.end());
    }
    std::vector<llama_token> hist;
    std::string gen = pre;
    llama_token cur = (llama_token) argmax(p_llama_get_logits_ith(e->ctx, -1), e->n_vocab);
    llama_batch batch = p_llama_batch_init(draft_max + 1, 0, 1);
    bool done = false;
    while (!done && s.n_gen < max_new_tokens) {
        if (p_llama_vocab_is_eog(e->vocab, cur)) break;
        append_piece(e->vocab, cur, gen);
        hist.push_back(cur);
        s.n_gen++;
        if (s.n_gen >= max_new_tokens) break;
        std::vector<llama_token> d = dtoks.empty() ? std::vector<llama_token>{} : lookup_draft(dtoks, hist, draft_max);
        int room = max_new_tokens - s.n_gen;
        if ((int) d.size() > room) d.resize((size_t) room);
        // 批：[cur, d1..dk]，全部要 logits。
        batch.n_tokens = 0;
        for (size_t i = 0; i <= d.size(); i++) {
            int b = batch.n_tokens++;
            batch.token[b] = i == 0 ? cur : d[i - 1];
            batch.pos[b] = n_past + (llama_pos) i;
            batch.n_seq_id[b] = 1;
            batch.seq_id[b][0] = 0;
            batch.logits[b] = 1;
        }
        if (p_llama_decode(e->ctx, batch) != 0) { p_llama_batch_free(batch); set_err(err, err_len, "decode failed"); return 3; }
        s.n_draft_proposed += (int32_t) d.size();
        // 逐位验证：logits[i] 的 argmax 应等于 d[i]（即位置 i+1 的 token）。
        size_t acc = 0;
        llama_token next = (llama_token) argmax(p_llama_get_logits_ith(e->ctx, 0), e->n_vocab);
        while (acc < d.size() && next == d[acc]) {
            if (p_llama_vocab_is_eog(e->vocab, next)) break;
            append_piece(e->vocab, next, gen);
            hist.push_back(next);
            s.n_gen++;
            acc++;
            if (s.n_gen >= max_new_tokens) { done = true; break; }
            next = (llama_token) argmax(p_llama_get_logits_ith(e->ctx, (int32_t) acc), e->n_vocab);
        }
        s.n_draft_accepted += (int32_t) acc;
        n_past += (llama_pos) (1 + acc);
        // 丢弃未被接受的草稿位置的 KV。
        if (acc < d.size()) p_llama_memory_seq_rm(mem, 0, n_past, -1);
        cur = next;
        if (on_partial) on_partial(user, gen.c_str());
    }
    p_llama_batch_free(batch);
    // DRAFT-LANG-PREV-458：记下本次模型自写的语种标记（到 <asr_text> 为止，≤8 个 token），供下一窗草稿用。
    if (pre.empty() && e->asr_text_tok >= 0) {
        for (size_t i = 0; i < hist.size() && i < 8; i++) {
            if (hist[i] == e->asr_text_tok) {
                e->lang_draft.assign(hist.begin(), hist.begin() + (std::ptrdiff_t) i + 1);
                break;
            }
        }
    }
    auto t2 = clk::now();
    s.prefill_ms = std::chrono::duration<double, std::milli>(t1 - t0).count();
    s.gen_ms = std::chrono::duration<double, std::milli>(t2 - t1).count();
    if (st) *st = s;
    if ((int) gen.size() + 1 > out_len) { set_err(err, err_len, "output buffer too small"); return 4; }
    memcpy(out, gen.c_str(), gen.size() + 1);
    return 0;
}



// ============================================================================================
// FORCED-ALIGN-456：Qwen3-ForcedAligner-0.6B 强制对齐（音频 + 已知文字 ⇒ 每个单元的起止秒）。
//
// 模型 = 42ailab 三件套：主干 GGUF（qwen3，embeddings / pooling NONE 取隐状态）+ 音频编码器 mmproj
// （qwen3a）+ 时间戳头 aligner-head.bin（魔数 "AH01"、u32 档数 5000、u32 维数 1024，其后按档逐行 f32）。
// 提示词：<音频> + 每个单元后接两个 <timestamp>（起、止）；一次前向，取每个槽位隐状态 × 时间戳头，
// argmax 档 × 80ms；再按参考实现做「最长非降子序列」修复。做法同 llama.cpp-omni PR #115（公开 API）。
// 时间戳头只算「音频时长内」的档（窗口 ≤ 数十秒 ⇒ 只需前几百档，结果与全算相同、快一个数量级）。
// ============================================================================================

namespace {
constexpr int LAS_ALIGN_SEGMENT_MS = 80;
// embeddings 模式下 llama.cpp 把批内**所有** token 都当输出（b11207 `output_all = cparams.embeddings`），
// 超过 n_outputs_max 即断言崩溃 ⇒ 文字每批 token 数 = 输出上限，结构上不会超。音频段送入时关 embeddings（不输出）。
constexpr int LAS_ALIGN_OUTPUTS_MAX = 128; // Gavin 09-29「1+2」：256→128，计算缓冲 150→75MiB、logits 输出缓冲减半
constexpr int LAS_ALIGN_TEXT_BATCH = LAS_ALIGN_OUTPUTS_MAX;

// 参考实现的时间戳修复：LNDS 标出正常值；≤2 个异常取较近一侧，更长的段两侧插值，只有一侧取该侧。
std::vector<int> las_fix_timestamps(const std::vector<double> & data) {
    const int n = (int) data.size();
    if (n == 0) return {};
    std::vector<int> dp(n, 1), parent(n, -1);
    for (int i = 1; i < n; i++)
        for (int j = 0; j < i; j++)
            if (data[j] <= data[i] && dp[j] + 1 > dp[i]) { dp[i] = dp[j] + 1; parent[i] = j; }
    int max_idx = 0;
    for (int i = 1; i < n; i++) if (dp[i] > dp[max_idx]) max_idx = i;
    std::vector<bool> ok(n, false);
    for (int k = max_idx; k != -1; k = parent[k]) ok[k] = true;
    std::vector<double> r = data;
    int i = 0;
    while (i < n) {
        if (ok[i]) { i++; continue; }
        int j = i;
        while (j < n && !ok[j]) j++;
        const int cnt = j - i;
        bool hl = false, hr = false;
        double lv = 0, rv = 0;
        for (int k = i - 1; k >= 0; k--) if (ok[k]) { lv = r[k]; hl = true; break; }
        for (int k = j; k < n; k++) if (ok[k]) { rv = r[k]; hr = true; break; }
        if (cnt <= 2) {
            for (int k = i; k < j; k++) {
                if (!hl) r[k] = rv;
                else if (!hr) r[k] = lv;
                else r[k] = (k - (i - 1)) <= (j - k) ? lv : rv;
            }
        } else if (hl && hr) {
            const double step = (rv - lv) / (cnt + 1);
            for (int k = i; k < j; k++) r[k] = lv + step * (k - i + 1);
        } else if (hl) {
            for (int k = i; k < j; k++) r[k] = lv;
        } else if (hr) {
            for (int k = i; k < j; k++) r[k] = rv;
        }
        i = j;
    }
    std::vector<int> out(n);
    for (int k = 0; k < n; k++) out[k] = (int) r[k];
    return out;
}

ggml_backend_dev_t las_pick_gpu(std::string & desc) {
    desc = "CPU";
    for (size_t i = 0; i < p_ggml_backend_dev_count(); i++) {
        ggml_backend_dev_t dv = p_ggml_backend_dev_get(i);
        enum ggml_backend_dev_type t = p_ggml_backend_dev_type(dv);
        if (t == GGML_BACKEND_DEVICE_TYPE_GPU || t == GGML_BACKEND_DEVICE_TYPE_IGPU) {
            desc = p_ggml_backend_dev_description(dv);
            return dv;
        }
    }
    return nullptr;
}
} // namespace

struct las_aligner {
    llama_model * model = nullptr;
    llama_context * ctx = nullptr;
    mtmd_context * mctx = nullptr;
    std::vector<float> head; // n_buckets 行 × n_embd
    int n_buckets = 0, n_embd = 0, n_embd_inp = 0, n_ctx = 0, n_threads = 4;
    llama_token ts_token = 151705;
};

extern "C" void las_align_free(las_aligner * a) {
    if (!a) return;
    if (a->mctx) p_mtmd_free(a->mctx);
    if (a->ctx) p_llama_free(a->ctx);
    if (a->model) p_llama_model_free(a->model);
    delete a;
}

// 创建对齐器。gpu=1：主干与音频编码器放 GPU（有设备时）。返回 nullptr = 失败（err 写原因）。
extern "C" las_aligner * las_align_create(const char * backbone_path, const char * mmproj_path,
                                          const char * head_path, int gpu, int n_threads, int n_ctx,
                                          char * info, int info_len, char * err, int err_len) {
    auto * a = new las_aligner();
    // ---- 时间戳头 ----
    FILE * f = fopen(head_path, "rb");
    if (!f) { set_err(err, err_len, std::string("open head failed: ") + head_path); delete a; return nullptr; }
    uint32_t hdr[3] = {0, 0, 0};
    bool ok = fread(hdr, sizeof(uint32_t), 3, f) == 3 && memcmp(hdr, "AH01", 4) == 0 && hdr[1] > 0 && hdr[2] > 0;
    if (ok) {
        a->n_buckets = (int) hdr[1];
        a->n_embd = (int) hdr[2];
        a->head.resize((size_t) a->n_buckets * a->n_embd);
        ok = fread(a->head.data(), sizeof(float), a->head.size(), f) == a->head.size();
    }
    fclose(f);
    if (!ok) { set_err(err, err_len, std::string("bad head file: ") + head_path); delete a; return nullptr; }
    // ---- 主干 ----
    std::string dev_desc;
    ggml_backend_dev_t gpu_dev = gpu ? las_pick_gpu(dev_desc) : nullptr;
    if (!gpu_dev) dev_desc = "CPU";
    ggml_backend_dev_t devs[2] = {gpu_dev, nullptr};
    llama_model_params mp = p_llama_model_default_params();
    mp.n_gpu_layers = gpu_dev ? 999 : 0;
    mp.devices = gpu_dev ? devs : nullptr;
    a->model = p_llama_model_load_from_file(backbone_path, mp);
    if (!a->model) { set_err(err, err_len, std::string("load aligner backbone failed: ") + backbone_path); las_align_free(a); return nullptr; }
    if (p_llama_model_n_embd(a->model) != a->n_embd) { set_err(err, err_len, "backbone hidden size != head"); las_align_free(a); return nullptr; }
    a->n_embd_inp = p_llama_model_n_embd_inp(a->model);
    a->n_ctx = n_ctx;
    a->n_threads = n_threads;
    llama_context_params cp = p_llama_context_default_params();
    cp.n_ctx = (uint32_t) n_ctx;
    cp.n_batch = (uint32_t) n_ctx; // 文字段（每单元 + 两槽）一次送入
    cp.n_ubatch = (uint32_t) las_env_int("LAS_ALIGN_UBATCH", 512);
    // 输出上限 = LAS_ALIGN_OUTPUTS_MAX（计算缓冲按上限预留 15 万维 logits；文字按同样大小分批，结构上不超）。
    cp.n_outputs_max = LAS_ALIGN_OUTPUTS_MAX;
    cp.n_seq_max = 1;
    cp.n_threads = n_threads;
    cp.n_threads_batch = n_threads;
    cp.embeddings = true;
    cp.pooling_type = LLAMA_POOLING_TYPE_NONE;
    cp.flash_attn_type = LLAMA_FLASH_ATTN_TYPE_ENABLED;
    cp.type_k = GGML_TYPE_Q8_0;
    cp.type_v = GGML_TYPE_Q8_0;
    cp.no_perf = true;
    a->ctx = p_llama_init_from_model(a->model, cp);
    if (!a->ctx) { set_err(err, err_len, "init aligner context failed"); las_align_free(a); return nullptr; }
    // <timestamp> token id：按词表解析（缺省 151705）。
    {
        const llama_vocab * v = p_llama_model_get_vocab(a->model);
        llama_token t[4];
        const char * s = "<timestamp>";
        if (p_llama_tokenize(v, s, (int32_t) strlen(s), t, 4, false, true) == 1) a->ts_token = t[0];
    }
    // ---- 音频编码器 ----
    mtmd_context_params mcp = p_mtmd_context_params_default();
    mcp.use_gpu = gpu_dev != nullptr;
    mcp.device = gpu_dev;
    mcp.n_threads = n_threads;
    mcp.print_timings = false;
    mcp.flash_attn_type = LLAMA_FLASH_ATTN_TYPE_ENABLED;
    // 不按最长音频预留编码缓冲（Gavin 09-29 选「1+2」省显存）：省 ~0.4G，换每窗编码慢 12~15%（FORCED-ALIGN-456 实测）。
    mcp.warmup = false;
    a->mctx = p_mtmd_init_from_file(mmproj_path, a->model, mcp);
    if (!a->mctx || !p_mtmd_support_audio(a->mctx)) {
        set_err(err, err_len, std::string("load aligner audio encoder failed: ") + mmproj_path);
        las_align_free(a);
        return nullptr;
    }
    set_err(info, info_len, dev_desc);
    return a;
}

// 对齐：samples（16kHz 单声道）+ n_units 个单元（UTF-8，以 '\x1f' 分隔）⇒ out_start / out_end（秒）。
// ms_out（可空）：double[4] = 总 / 音频编码 / 主干前向 / 时间戳头 毫秒。返回 0 = 成功。
extern "C" int las_align(las_aligner * a, const float * samples, int n_samples, const char * units_joined,
                         int n_units, float * out_start, float * out_end, double * ms_out, char * err,
                         int err_len) {
    using clk = std::chrono::steady_clock;
    auto t0 = clk::now();
    if (!a || !samples || n_samples <= 0 || n_units <= 0) { set_err(err, err_len, "bad args"); return 1; }
    std::string text = p_mtmd_default_marker();
    {
        const char * p = units_joined;
        int got = 0;
        while (got < n_units) {
            const char * q = strchr(p, '\x1f');
            text.append(p, q ? (size_t) (q - p) : strlen(p));
            text += "<timestamp><timestamp>";
            got++;
            if (!q) break;
            p = q + 1;
        }
        if (got != n_units) { set_err(err, err_len, "unit count mismatch"); return 1; }
    }
    mtmd_bitmap * bmp = p_mtmd_bitmap_init_from_audio((size_t) n_samples, samples);
    mtmd_input_chunks * chunks = p_mtmd_input_chunks_init();
    mtmd_input_text in{};
    in.text = text.c_str();
    in.text_len = text.size();
    in.add_special = false;
    in.parse_special = true;
    const mtmd_bitmap * bmps[1] = {bmp};
    int rc = p_mtmd_tokenize(a->mctx, chunks, &in, bmps, 1);
    p_mtmd_bitmap_free(bmp);
    if (rc != 0) { p_mtmd_input_chunks_free(chunks); set_err(err, err_len, "tokenize failed rc=" + std::to_string(rc)); return 2; }
    const size_t n_chunks = p_mtmd_input_chunks_size(chunks);
    size_t n_total = 0;
    for (size_t i = 0; i < n_chunks; i++) n_total += p_mtmd_input_chunk_get_n_tokens(p_mtmd_input_chunks_get(chunks, i));
    if (n_chunks == 0 || (int) n_total > a->n_ctx) {
        p_mtmd_input_chunks_free(chunks);
        set_err(err, err_len, "input exceeds aligner context: " + std::to_string(n_total));
        return 3;
    }
    p_llama_memory_clear(p_llama_get_memory(a->ctx), true);
    llama_pos n_past = 0;
    const int E = a->n_embd;
    std::vector<float> hid;          // 各 <timestamp> 槽位的隐状态（按出现顺序，每个 E 维）
    size_t n_slots = 0;
    double enc_ms = 0;
    auto tf = clk::now();
    for (size_t i = 0; i < n_chunks && rc == 0; i++) {
        const mtmd_input_chunk * ch = p_mtmd_input_chunks_get(chunks, i);
        const bool last = i + 1 == n_chunks;
        const bool non_causal = p_mtmd_decode_use_non_causal(a->mctx, ch);
        if (non_causal) p_llama_set_causal_attn(a->ctx, false);
        if (p_mtmd_input_chunk_get_type(ch) == MTMD_INPUT_CHUNK_TYPE_TEXT) {
            size_t n_tok = 0;
            const llama_token * toks = p_mtmd_input_chunk_get_tokens_text(ch, &n_tok);
            // 分批送入，每批 ≤ LAS_ALIGN_TEXT_BATCH（= 输出上限）个 token；每批解完立即拷出本批槽位隐状态（下一批覆盖输出缓冲）。
            for (size_t b0 = 0; b0 < n_tok && rc == 0; b0 += LAS_ALIGN_TEXT_BATCH) {
                const size_t nb = (std::min)((size_t) LAS_ALIGN_TEXT_BATCH, n_tok - b0);
                llama_batch b = p_llama_batch_init((int32_t) nb, 0, 1);
                std::vector<int> idx;
                for (size_t k = 0; k < nb; k++) {
                    b.token[k] = toks[b0 + k];
                    b.pos[k] = n_past + (llama_pos) (b0 + k);
                    b.n_seq_id[k] = 1;
                    b.seq_id[k][0] = 0;
                    b.logits[k] = last && toks[b0 + k] == a->ts_token;
                    if (b.logits[k]) idx.push_back((int) k);
                }
                b.n_tokens = (int32_t) nb;
                rc = p_llama_decode(a->ctx, b);
                p_llama_batch_free(b);
                for (int k : idx) {
                    const float * h = rc == 0 ? p_llama_get_embeddings_ith(a->ctx, k) : nullptr;
                    if (!h) { rc = rc ? rc : -2; break; }
                    hid.insert(hid.end(), h, h + E);
                    n_slots++;
                }
            }
        } else {
            // 音频段交给 mtmd 辅助函数：42ailab 主干为 qwen3vl（M-RoPE），嵌入输入须按多维位置送入，
            // 手工一维位置会错位（实测所有时间戳塌到第 0 档）。文字段 llama.cpp 会自动展开位置。
            // 音频位置不需要输出：临时关 embeddings，避免 llama.cpp 把全部音频位置强制设为输出（会撑爆输出上限）。
            auto te = clk::now();
            if (p_mtmd_encode_chunk(a->mctx, ch) != 0) { rc = -1; break; }
            enc_ms += std::chrono::duration<double, std::milli>(clk::now() - te).count();
            llama_pos np = n_past;
            p_llama_set_embeddings(a->ctx, false);
            rc = p_mtmd_helper_decode_image_chunk(a->mctx, a->ctx, ch, p_mtmd_get_output_embd(a->mctx), n_past, 0,
                                                  a->n_ctx, &np, nullptr, nullptr);
            p_llama_set_embeddings(a->ctx, true);
            if (non_causal) p_llama_set_causal_attn(a->ctx, true);
            n_past = np;
            continue;
        }
        if (non_causal) p_llama_set_causal_attn(a->ctx, true);
        n_past += p_mtmd_input_chunk_get_n_pos(ch);
    }
    p_mtmd_input_chunks_free(chunks);
    if (rc != 0) { set_err(err, err_len, "aligner decode failed rc=" + std::to_string(rc)); return 4; }
    if ((int) n_slots != n_units * 2) {
        set_err(err, err_len, "timestamp slots " + std::to_string(n_slots) + " != 2*units " + std::to_string(n_units));
        return 5;
    }

    // 时间戳头：只算音频时长内的档（+2 余量）。
    const int max_b = (std::min)(a->n_buckets, (int) ((int64_t) n_samples * 1000 / 16000 / LAS_ALIGN_SEGMENT_MS) + 2);
    std::vector<const float *> hs(n_slots);
    for (size_t s = 0; s < n_slots; s++) hs[s] = hid.data() + s * (size_t) E;
    // 取隐状态会等 GPU 前向完成 ⇒ 主干计时截到这里才准。
    const double fwd_ms = std::chrono::duration<double, std::milli>(clk::now() - tf).count() - enc_ms;
    auto th = clk::now();
    std::vector<double> ts_ms(n_slots, 0.0);
    auto work = [&](size_t s0, size_t s1) {
        for (size_t s = s0; s < s1; s++) {
            const float * h = hs[s];
            int best = 0;
            float bv = -INFINITY;
            for (int bk = 0; bk < max_b; bk++) {
                const float * w = a->head.data() + (size_t) bk * E;
                // 8 路独立累加：打破串行依赖，编译器可向量化（不依赖 /fp:fast）。
                float c0 = 0, c1 = 0, c2 = 0, c3 = 0, c4 = 0, c5 = 0, c6 = 0, c7 = 0;
                int k = 0;
                for (; k + 8 <= E; k += 8) {
                    c0 += w[k] * h[k]; c1 += w[k + 1] * h[k + 1]; c2 += w[k + 2] * h[k + 2]; c3 += w[k + 3] * h[k + 3];
                    c4 += w[k + 4] * h[k + 4]; c5 += w[k + 5] * h[k + 5]; c6 += w[k + 6] * h[k + 6]; c7 += w[k + 7] * h[k + 7];
                }
                float acc = ((c0 + c1) + (c2 + c3)) + ((c4 + c5) + (c6 + c7));
                for (; k < E; k++) acc += w[k] * h[k];
                if (acc > bv) { bv = acc; best = bk; }
            }
            ts_ms[s] = (double) best * LAS_ALIGN_SEGMENT_MS;
        }
    };
    {
        const size_t n = n_slots;
        const size_t nt = (std::min)((size_t) (std::max)(1, a->n_threads), (n + 31) / 32);
        std::vector<std::thread> th_pool;
        const size_t per = (n + nt - 1) / (nt ? nt : 1);
        for (size_t t = 1; t < nt; t++) th_pool.emplace_back(work, (std::min)(n, t * per), (std::min)(n, (t + 1) * per));
        work(0, (std::min)(n, per));
        for (auto & t : th_pool) t.join();
    }
    const std::vector<int> fixed = las_fix_timestamps(ts_ms);
    for (int u = 0; u < n_units; u++) {
        out_start[u] = (float) (fixed[(size_t) u * 2] / 1000.0);
        out_end[u] = (float) (fixed[(size_t) u * 2 + 1] / 1000.0);
    }
    if (ms_out) {
        const auto now = clk::now();
        ms_out[0] = std::chrono::duration<double, std::milli>(now - t0).count();
        ms_out[1] = enc_ms;
        ms_out[2] = fwd_ms;
        ms_out[3] = std::chrono::duration<double, std::milli>(now - th).count();
    }
    return 0;
}

// MEM-TRIM-457 ③：用已加载引擎的词表数 token（替代单独加载的 tokenizer.json，同一套 Qwen3 词表）。
// add_special=false / parse_special=false，与原 tokenizers `encode(word, false)` 同口径。返回 < 0 = 失败。
extern "C" int las_count_tokens(las_engine * e, const char * text) {
    if (!e || !text) return -1;
    const int len = (int) strlen(text);
    if (len == 0) return 0;
    const int n = p_llama_tokenize(e->vocab, text, len, nullptr, 0, false, false);
    return n < 0 ? -n : n;
}
