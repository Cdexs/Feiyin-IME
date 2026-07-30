# RESEARCH-ACC-CRASH-001 · accuracy 长音频静默崩溃根因审计

> **任务类型**：纯研究，生产代码零改动
> **作者**：coder-1
> **日期**：2026-07-07
> **背景**：Gavin 端测 accuracy 模型输入一大长段语音后程序崩溃。崩溃特征为**静默死亡**——panic hook 未触发（exe 同级 `crash.json` 未生成）、Windows 事件日志无 WER/Application Error 记录、无 dump、debug.log 未开启。该特征指向 **Rust alloc abort（内存耗尽不走 panic hook）** 或 **native 层（onnxruntime/sherpa-onnx）异常终止**。
> **注意**：Gavin 正在并行做 `--debug` 复现，本审计结论将与复现日志交叉验证。

---

## 一、症状特征分析：为什么不是普通 Rust panic？

| 观测 | 普通 Rust panic | 本案症状 | 匹配？ |
|------|------------------|----------|--------|
| panic hook 触发 → 写 `crash.json` | ✅ 会写 | ❌ 未生成 | ❌ |
| Windows 事件日志有 Application Error | ✅ 通常有 | ❌ 无 WER 记录 | ❌ |
| 有 dump 文件 | ✅ 通常有 | ❌ 无 | ❌ |
| debug.log 有 stacktrace | ✅ 有 | ❌ 未开启 | N/A |

**结论**：静默死亡排除普通 Rust `panic!()`（项目 `Cargo.toml` 未设 `panic = "abort"`，profile.release 只有 `lto=true/codegen-units=1/strip=true`，Rust panic 会 unwind → 触发 `main.rs:2608` 注册的 panic hook → 写 `crash.json`）。

静默死亡的两大候选：

1. **Rust OOM → `alloc::handle_alloc_error` → abort**：Rust 标准库分配失败时调用 `handle_alloc_error`，它直接 `abort()` 进程，**不走 panic 通路**，因此 panic hook 不触发、无 `crash.json`。Windows 对 abort 进程不一定产生 WER（取决于系统配置）。
2. **native 层（onnxruntime / sherpa-onnx C++）崩溃**：C++ 异常、访问违规（0xC0000005）、或 native 内部 `std::abort()`。这些也不触发 Rust panic hook。sherpa-onnx issue #2172 已记录特定输入 buffer 导致 0xC0000005 访问违规崩溃。

---

## 二、代码路径审计

### 2.1 accuracy 长音频 VAD 分段路径（DEC-026）

入口：`src/transcription/mod.rs:166` `transcribe_offline_detailed`。

关键代码位置（`src/transcription/mod.rs`）：

- **L172**：`if self.asr_model == AsrModel::Accuracy && vad::should_segment(samples)` — 触发条件是音频 > 24s（`SEGMENT_TRIGGER_SECS=24.0`，`vad.rs:17`）。
- **L173-176**：取 `self.vad_segmenter`（`Option<Mutex<VadSegmenter>>`），加锁，调 `vad.segment(samples)`。
- **L175-200**：逐段调 `transcribe_segment_detailed`，每段独立创建/销毁 `OfflineStream`（L235 `create_stream()`，stream 在函数末尾 drop）。
- **L211-217**：**分段降级路径**——若 VAD 分段全部空文本，**降级为单次转录**（L220 `transcribe_segment_detailed(samples, script)`），此时**整段长音频直接喂给 native 模型**。

### 2.2 兜底链（fallback）

`transcribe_segment_detailed`（`src/transcription/mod.rs:230-301`）：

- L235-239：native stream `create_stream()` + `accept_waveform(16000, samples)` + `decode` + `get_result`。
- L243-247：`need_fallback` 判定（空/hallucination/repetitive/language_anomaly）。
- L264-281：**fallback 创建独立 `fb_stream`**，`accept_waveform` + `decode` + `get_result`。

**关键内存观察**：L235 的 native `stream` 在函数末尾才 drop。L265 的 `fb_stream` 在 native stream 仍存活期间创建。**短暂时间内 native stream + fallback stream 同时存活**，两份音频样本各持一份引用（`accept_waveform` 在 C++ 侧通常 copy 一份）。

### 2.3 run_pipeline 录音缓冲（`src/main.rs:2730` + `src/audio/mod.rs`）

- `config::MAX_RECORD_SECONDS = 300`（`src/config/mod.rs:10`）—— **最长 5 分钟**。
- `collect_recording`（`src/audio/mod.rs:410`）：`state.all_samples: Vec<f32>` 累积全部 native-rate 样本。
- L599-605：若 `sample_rate != 16000`，调 `resample_anti_alias(&state.all_samples, sample_rate, 16000)` 返回新 `Vec<f32>`。

**内存峰值在 resample 瞬间**：`state.all_samples`（native-rate 全量）+ 输出 `Vec<f32>`（16kHz）同时存活。

### 2.4 unwrap/expect/panic 排查

在 transcription + vad 生产路径中**无生产 unwrap/expect/panic**（仅测试代码 `vad.rs:349/387/389` 有，生产安全）。

`src/audio/mod.rs` 中有 `pre_roll.lock().unwrap()`（L229/261/293/364），在 WASAPI 回调线程。Mutex poison 会 panic → unwind → 触发 panic hook → 应写 `crash.json`。**与静默死亡症状不符**，排除。

---

## 三、内存峰值量化估算

### 3.1 常驻内存（模型加载，`Transcriber::new` 期间常驻）

| 组件 | 大小 | 来源 |
|------|------|------|
| FunASR Nano native 模型文件（int8） | 994 MB | embedding(155MB)+encoder(238MB)+llm(600MB)，`models/sherpa-onnx-funasr-nano-int8-2025-12-30/` |
| ONNX Runtime 加载后内存占用（int8 量化） | ~1.2-1.6 GB | int8 量化模型加载后通常会膨胀（权重解压 + ORT 内部 arena） |
| CTC fallback 模型（SenseVoice 179MB int8） | ~250-350 MB | `models/sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17/model.int8.onnx` |
| silero VAD 模型 | <1 MB | `silero_vad.onnx` 644KB |
| **常驻小计** | **~1.5-2.0 GB** | native + fallback 同时加载（DEC-025 设计：accuracy 预创建 fallback） |

### 3.2 录音缓冲峰值（run_pipeline 期间）

| 录音时长 | native-rate (48kHz mono f32) | resample 后 (16kHz) | 峰值（两份同时） |
|----------|------------------------------|---------------------|------------------|
| 30s | 5.76 MB | 1.92 MB | 7.68 MB |
| 60s | 11.52 MB | 3.84 MB | 15.36 MB |
| 120s | 23.04 MB | 7.68 MB | 30.72 MB |
| **300s (上限)** | **57.6 MB** | **19.2 MB** | **76.8 MB** |

### 3.3 VAD 分段瞬时内存

`VadSegmenter::segment`（`vad.rs:66`）：
- 喂全部 samples 给 detector（C++ 侧内部 ring buffer，`buffer_size_in_seconds=300.0` → 预分配 ~19.2MB @ 16kHz）。
- `build_padded_segments` 返回 `Vec<Vec<f32>>`，所有段样本总和 ≈ 原音频长度 + padding（每段 6400 samples × N 段）。

| 录音时长 | 段数（估） | 段样本总量 | padding 总量 | 峰值 |
|----------|-----------|------------|--------------|------|
| 60s | ~3-5 | 3.84 MB | ~30-50 KB | ~4 MB |
| 300s | ~15-20 | 19.2 MB | ~120-200 KB | ~20 MB |

### 3.4 单段转录瞬时内存（native + fallback 同时）

每段 ≤20s（`SEGMENT_MAX_SECS=20.0`）：
- native OfflineStream: 音频样本 1.28MB + KV cache（max_total_len=512，LFR ~333 tokens × 模型维度）+ ONNX 激活内存。Qwen3-0.6B int8 的 KV cache 估算：512 × 1024 dim × 28 layers × 2(K/V) × 1 byte(int8) ≈ 29 MB；激活内存（batch=1, seq~400）估算 50-150 MB。
- fallback CTC OfflineStream: 音频 1.28MB + CTC 激活内存 ~30-80 MB。

**单段峰值（native + fallback 同时存活）**：~150-250 MB。

### 3.5 总内存峰值估算表

| 场景 | 常驻模型 | 录音缓冲 | VAD 段缓冲 | 转录瞬时 | **总峰值** |
|------|----------|----------|------------|----------|------------|
| 30s accuracy 长音频 | ~1.5-2.0 GB | 7.7 MB | ~2 MB | ~150-250 MB | **~1.7-2.3 GB** |
| 60s | ~1.5-2.0 GB | 15.4 MB | ~4 MB | ~150-250 MB | **~1.7-2.3 GB** |
| 120s | ~1.5-2.0 GB | 30.7 MB | ~8 MB | ~150-250 MB | **~1.8-2.4 GB** |
| **300s（上限）** | ~1.5-2.0 GB | **76.8 MB** | ~20 MB | ~150-250 MB | **~1.9-2.5 GB** |

**关键结论**：accuracy 模式常驻内存 1.5-2.0 GB，长音频场景总峰值可达 **~2.5 GB**。在 4GB 物理内存机器上（或可用内存 <2.5GB 时）极易触发 OOM。

---

## 四、Top 3 崩溃候选根因（按概率排序）

### 🥇 候选 1（概率 50%）：native 模型单次转录超 max_total_len 导致 native 层崩溃/abort

**代码位置**：
- `src/transcription/mod.rs:172-218`（VAD 降级路径）
- `src/transcription/mod.rs:220`（降级单次转录）
- `src/transcription/mod.rs:216`（VAD segmenter 未就位降级）

**机制**：
1. accuracy 模式下，若 **VAD 分段器未初始化**（`vad_segmenter = None`，即 `VadSegmenter::try_new` 返回 None——silero_vad.onnx 缺失或加载失败）→ `transcribe_offline_detailed` 走 L216 warn 然后 L220 直接调 `transcribe_segment_detailed(samples, script)` 喂**整段长音频**给 native 模型。
2. 或 VAD 分段成功但**所有段返回空文本**（L211）→ 降级单次转录同样喂整段。
3. native 模型 `max_total_len=512` 是导出时固化的 KV cache 硬上限。当输入 LFR tokens 超过 ~489（≈28s）时，C++ 层行为未定义——可能：
   - 空输出（已知，RESEARCH-ASR-ACCURACY-001 记录）
   - **native 内部 assert/abort**（C++ 越界访问或 ORT 推理时 tensor 形状异常）→ 进程 abort，不走 Rust panic hook → 静默死亡。

**为何符合症状**：
- C++ abort / 访问违规 不触发 Rust panic hook → 无 `crash.json`。
- WER 不一定记录 native DLL 内部的 abort（取决于 DLL 是否注册了 SEH/VEH）。
- sherpa-onnx issue #2172 已证实特定输入 buffer 可导致 `0xC0000005` 访问违规崩溃。

**验证方法**（等 Gavin `--debug` 复现日志）：
- 查日志是否有 `"VAD segmenter unavailable, falling back to single-pass"`（L216）或 `"All VAD segments produced empty text, falling back to single-pass"`（L212）。
- 查崩溃前最后一条日志：若是 `"Transcribing N samples"` 且 N/16000 > 28，则确认超长输入走单次转录。
- 确认 `models/silero-vad/silero_vad.onnx` 是否存在于 Gavin 机器（缺失 → VAD 永不初始化）。

**若成立的修复方向**：
- **硬上限保护**：在 `transcribe_offline_detailed` 降级路径加 guard——若 `samples.len() > SEGMENT_TRIGGER_SECS * 16000` 且 VAD 不可用，直接 `bail!` 返回错误而非喂给 native 模型（让上层走 CTC fallback 或报错）。
- **VAD 缺失检测**：accuracy 模式启动时若 VAD 模型缺失，日志 WARN 提示用户下载（当前 `try_new` 失败静默返回 None）。
- **单次转录分段兜底**：即使 VAD 不可用，对超长音频用朴素等分（按 20s 切）替代 VAD，避免 native 模型越界。

---

### 🥈 候选 2（概率 30%）：内存耗尽（OOM）触发 Rust alloc abort

**代码位置**：
- `src/config/mod.rs:10`（`MAX_RECORD_SECONDS = 300`）
- `src/transcription/mod.rs:59-71`（Transcriber 持双 recognizer 常驻）
- `src/audio/mod.rs:599-605`（resample 期间双份音频共存）

**机制**：
1. accuracy 模式启动时同时加载 native(994MB) + CTC fallback(264MB) 两个模型到 ONNX Runtime，常驻 ~1.5-2.0 GB。
2. 用户录制超长语音（接近 300s 上限），`all_samples` 累积 57.6MB（48kHz），resample 期间峰值 76.8MB。
3. VAD 分段后逐段转录，每段 native 推理瞬时分配 ~150-250MB 激活内存。
4. 若 Gavin 机器可用内存 <2.5GB（或被其他程序占用），Rust allocator 在某次 `Vec::reserve` / ORT 内部分配时失败 → `alloc::handle_alloc_error` → **直接 abort，不走 panic hook**。

**为何符合症状**：
- `handle_alloc_error` 调 `std::process::abort()`，不触发 Rust panic hook → 无 `crash.json`。
- OOM abort 在 Windows 上不一定产生 WER（取决于 `ErrorReporting` 配置）。
- 静默死亡 = 进程瞬间消失，无日志（debug.log 未开）。

**验证方法**：
- 等 Gavin `--debug` 复现：查崩溃前最后日志是否在 `"Transcribing N samples"` 之后（转录阶段分配大块 ORT 激活内存时 OOM）。
- 查 Gavin 机器物理内存与崩溃时其他进程占用（任务管理器历史）。
- 可选：在 `run_pipeline` 入口加 `log::info!("Memory: {}MB available", ...)` 辅助诊断。

**若成立的修复方向**：
- **降低常驻**：accuracy 模式改为**懒加载 fallback recognizer**（首次需要兜底才创建），省 250-350MB 常驻。
- **缩短录音上限**：accuracy 模式 `MAX_RECORD_SECONDS` 从 300s 降到 120s（分段后单段 ≤20s，VAD 段数 ≤6，瞬时内存可控）。
- **内存监控**：转录前检查 `GlobalMemoryStatusEx` 可用内存，低于阈值（如 500MB）时拒录并提示用户。

---

### 🥉 候选 3（概率 15%）：native stream + fallback stream 同时存活 + ORT arena 累积导致 native 崩溃

**代码位置**：
- `src/transcription/mod.rs:235-280`（native stream 全程存活，fallback stream L265 创建）
- ONNX Runtime `MemoryArena` 行为（ORT 默认启用 arena，分配后不立即归还 OS）

**机制**：
1. VAD 分段循环中，每段创建 native `OfflineStream` + `accept_waveform` + `decode`。ORT arena 在每段推理时分配激活内存，**arena 默认不归还 OS**（缓存复用）。
2. 段数较多（300s → ~15-20 段）时，arena 可能累积到峰值后稳定，但若某段触发 fallback（L264），**额外创建 fallback stream**，native stream 仍存活 → arena 需同时容纳两份激活 → 可能触发 arena 扩容失败或越界。
3. 长时间运行后 ORT arena 碎片化，某次分配触发 native assert/abort。

**为何符合症状**：native abort 不走 Rust panic hook。

**验证方法**：
- `--debug` 日志查崩溃前是否有多段 `"ASR accuracy model abnormal output, falling back"` 日志（频繁兜底 = arena 双份活跃）。
- 查段数：`"VAD segmented N samples into M segments"`，M > 10 则 arena 累积风险高。

**若成立的修复方向**：
- native stream 在 decode + get_result 后**立即 drop**（L239 后显式 `drop(stream)`），再判定 fallback，避免双 stream 同时存活。
- ORT 配置 `arena_extend_strategy=kSameAsRequested`（减少 arena 缓存膨胀）——需 sherpa-onnx 支持。
- 限制 fallback 触发频率（连续 N 段 fallback 则整体降级 CTC）。

---

### 其他低概率候选（合计 5%）

- **pre_roll mutex poison panic**（`src/audio/mod.rs:229/261/293/364`）：会触发 panic hook → 应有 `crash.json`，与症状不符，排除。
- **WASAPI 设备丢失**：`collect_recording` 返回 Err，走 `run_pipeline` 的 `Err(e)` 分支正常报错，不崩溃。
- **translation/llm 模块崩溃**：崩溃发生在转录阶段（Gavin 输入长语音即崩，未到翻译），排除。
- **Windows 系统级问题**（DWM/驱动）：无法从代码侧判定，等复现日志排除。

---

## 五、与 Gavin `--debug` 复现日志的交叉验证清单

等 Gavin 复现日志后，按以下顺序确认根因：

| 步骤 | 查日志内容 | 指向哪个候选 |
|------|-----------|--------------|
| 1 | 是否有 `crash.json` 生成？ | 有 → 非 OOM/native abort（推翻全部三候选）；无 → 继续 |
| 2 | 崩溃前最后一条日志是什么？ | `"Transcribing N samples"` 且 N/16000 > 28 → 候选 1（VAD 降级单次） |
| 3 | 是否有 `"VAD segmenter unavailable"` 或 `"All VAD segments produced empty text"`？ | 有 → 候选 1 确认 |
| 4 | 是否有 `"VAD segmented N samples into M segments"`？M 值多少？ | M > 10 → 候选 3（arena 累积） |
| 5 | 是否有多条 `"ASR accuracy model abnormal output, falling back"`？ | 有 → 候选 3（双 stream） |
| 6 | `models/silero-vad/silero_vad.onnx` 是否存在于 Gavin 机器？ | 缺失 → 候选 1 确认（VAD 永不初始化） |
| 7 | Gavin 机器物理内存？崩溃时其他进程占用？ | <8GB 且其他程序占多 → 候选 2（OOM） |

---

## 六、产出与下游

- **本报告**：`collab/research/acc-crash-001.md`
- **生产代码改动**：零（纯研究）
- **下游**：等 Gavin `--debug` 复现日志 → 交叉验证 → 确认根因 → 立项修复任务（候选 1 修复成本最低、价值最高）

---

## 七、附录：关键代码位置索引

| 文件 | 行 | 内容 |
|------|----|------|
| `src/transcription/mod.rs` | 166-221 | `transcribe_offline_detailed`（VAD 分段 + 降级） |
| `src/transcription/mod.rs` | 230-301 | `transcribe_segment_detailed`（单段 + 兜底链） |
| `src/transcription/mod.rs` | 235-239 | native stream create + accept + decode + get_result |
| `src/transcription/mod.rs` | 264-281 | fallback stream create + accept + decode + get_result |
| `src/transcription/vad.rs` | 40-96 | `VadSegmenter::try_new` + `segment` |
| `src/transcription/vad.rs` | 17 | `SEGMENT_TRIGGER_SECS=24.0` |
| `src/transcription/vad.rs` | 21 | `SEGMENT_MAX_SECS=20.0` |
| `src/transcription/vad.rs` | 57 | `VoiceActivityDetector::create(&config, 300.0)` |
| `src/main.rs` | 2712-2726 | `select_preprocessing_params` |
| `src/main.rs` | 2730-2810 | `run_pipeline` 录音处理 + 转录入口 |
| `src/main.rs` | 2608-2622 | panic hook 注册 |
| `src/audio/mod.rs` | 410-608 | `collect_recording`（录音累积 + resample） |
| `src/audio/mod.rs` | 599-605 | `resample_anti_alias` 调用（双份音频峰值） |
| `src/audio/mod.rs` | 737-808 | `resample_anti_alias` 实现（FIR f64 累加） |
| `src/config/mod.rs` | 10 | `MAX_RECORD_SECONDS = 300` |
| `Cargo.toml` | 110-114 | `[profile.release]`（无 `panic=abort`） |