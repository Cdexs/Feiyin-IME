// ACC-ENGINE-LLAMACPP-452：Qwen3-ASR 1.7B 精解引擎的 llama.cpp 适配层（C++，由 build.rs 经 cc 编译进主程序）。
//
// 设计要点：
// - **运行时动态加载**官方预编译库（Windows: LoadLibrary / macOS: dlopen），函数指针类型取自同版本（b11207）
//   官方头文件（decltype），结构体布局由编译器保证与 DLL 一致；库缺失时主程序照常启动，las_load 返回错误。
// - 只暴露 5 个扁平 C 接口给 Rust（las_load / las_create / las_decode / las_free / las_set_log）。
// - 提示词按模型内置 ChatML 模板：[system 上下文] + user(音频标记) + assistant 预填(prefix)。
// - 贪心逐字生成；可选「草稿推测解码」：草稿 token 由调用方给的文字（如预览）n-gram 查表得出，
//   目标模型一次批量验证，只接受与贪心 argmax 一致的前缀 ⇒ 输出与逐字贪心一致（仅省时间）。

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
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
    X(mtmd_tokenize) X(mtmd_helper_eval_chunks) X(mtmd_helper_log_set)
LAS_FNS(LAS_FN)

typedef void (*las_log_fn)(int level, const char * text);
// 逐字回调（ACC-452 ⑤ 流式回灌）：每轮生成后回调「预填 + 截至目前的生成内容」。
typedef void (*las_partial_fn)(void * user, const char * text);
static las_log_fn g_log = nullptr;
static void log_cb(enum ggml_log_level level, const char * text, void *) {
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

struct las_engine {
    llama_model * model = nullptr;
    llama_context * ctx = nullptr;
    mtmd_context * mctx = nullptr;
    const llama_vocab * vocab = nullptr;
    int n_vocab = 0;
    int n_batch = 512;
};

struct las_stats {
    double prefill_ms;
    double gen_ms;
    int32_t n_prompt;
    int32_t n_gen;
    int32_t n_draft_accepted;
    int32_t n_draft_proposed;
};

// 创建引擎。gpu=1：模型与音频编码器全部放 GPU（有可用 GPU 设备时）；gpu=0：纯 CPU。
// info 写入实际使用的设备描述。返回 nullptr = 失败（err 写原因）。
extern "C" las_engine * las_create(const char * model_path, const char * mmproj_path, int gpu, int n_threads,
                        int n_threads_batch, int n_ctx, char * info, int info_len, char * err, int err_len) {
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
    cp.n_ubatch = 512;
    cp.n_seq_max = 1;
    cp.n_threads = n_threads;
    cp.n_threads_batch = n_threads_batch;
    cp.flash_attn_type = LLAMA_FLASH_ATTN_TYPE_ENABLED;
    cp.no_perf = true;
    llama_context * ctx = p_llama_init_from_model(model, cp);
    if (!ctx) { p_llama_model_free(model); set_err(err, err_len, "init context failed"); return nullptr; }

    mtmd_context_params mcp = p_mtmd_context_params_default();
    mcp.use_gpu = gpu_dev != nullptr;
    mcp.device = gpu_dev;
    mcp.n_threads = n_threads_batch;
    mcp.print_timings = false;
    mcp.flash_attn_type = LLAMA_FLASH_ATTN_TYPE_ENABLED;
    mcp.warmup = true;
    mtmd_context * mctx = p_mtmd_init_from_file(mmproj_path, model, mcp);
    if (!mctx) {
        p_llama_free(ctx); p_llama_model_free(model);
        set_err(err, err_len, std::string("load mmproj failed: ") + mmproj_path);
        return nullptr;
    }
    las_engine * e = new las_engine();
    e->model = model; e->ctx = ctx; e->mctx = mctx;
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
    llama_memory_t mem = p_llama_get_memory(e->ctx);
    p_llama_memory_clear(mem, true);
    llama_pos n_past = 0;
    rc = p_mtmd_helper_eval_chunks(e->mctx, e->ctx, chunks, 0, 0, e->n_batch, true, &n_past);
    p_mtmd_input_chunks_free(chunks);
    p_mtmd_bitmap_free(bmp);
    if (rc != 0) { set_err(err, err_len, "eval prompt failed: " + std::to_string(rc)); return 2; }
    s.n_prompt = (int32_t) n_past;
    auto t1 = clk::now();

    std::vector<llama_token> dtoks = (draft && *draft && draft_max > 0) ? tokenize(e->vocab, draft) : std::vector<llama_token>{};
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
    auto t2 = clk::now();
    s.prefill_ms = std::chrono::duration<double, std::milli>(t1 - t0).count();
    s.gen_ms = std::chrono::duration<double, std::milli>(t2 - t1).count();
    if (st) *st = s;
    if ((int) gen.size() + 1 > out_len) { set_err(err, err_len, "output buffer too small"); return 4; }
    memcpy(out, gen.c_str(), gen.size() + 1);
    return 0;
}


