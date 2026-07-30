# PoC 报告 · POC-QWEN3ASR-002A · FunASR Nano PoC：模型下载 + PoC bin + RTF/内存基准

> 任务：POC-QWEN3ASR-002A
> 负责人：coder-1
> 日期：2026-07-06
> 前置：RESEARCH-QWEN3ASR-001（报告 `collab/research/qwen3-asr-feasibility.md`）
> 主控裁决：选 C（两个模型都测）

---

## 0. 执行摘要

| 维度 | SenseVoice CTC 兼容版 (179MB) | Native FunASR Nano (802.7MB) | 当前 SenseVoice (237MB) |
|------|------------------------------|----------------------------|----------------------|
| 模型体积（解压后） | 254MB | 972MB | 237MB |
| 加载耗时 | 1.6s | 5.9s | ~1-2s |
| CPU RTF (4线程, 10s音频) | **0.011** (0.12s) | **0.185** (1.95s) | 极快 (~0.01) |
| CPU RTF (2线程, 10s音频) | 0.017 (0.18s) | 0.223 (2.35s) | — |
| 稳态内存 | 290MB | 1338MB | ~500MB |
| 峰值内存 | 362MB | 1619MB | — |
| Hotwords | ❌ 不支持（CTC） | ✅ config 层生效（实证纠正"紫菜"→"酯"） | ❌ 不支持 |
| V1 通过标准 (10s≤2s) | ✅ 通过 | ✅ 通过（4线程；2线程 2.35s 接近红线） | — |
| V3 内存 (≤2GB) | ✅ 通过 | ✅ 通过（1.6GB < 2GB，远低于研究预估 4-5GB） | — |

**关键发现**：
1. **Native FunASR Nano 原生版 hotwords 通路验证成功** — config 层 `OfflineFunASRNanoModelConfig.hotwords` 实证纠正识别错误（"紫菜"→"酯"），这是相对当前 SenseVoice 的结构性增益点确认
2. **`create_stream_with_hotwords` API 对非 transducer 模型报错** — sherpa-onnx runtime 层仅 transducer 支持 per-stream contextual biasing；FunASR Nano 必须通过 config 层全局 hotwords 注入
3. **Native 内存 1.6GB 远低于研究预估 4-5GB** — 研究报告基于 Qwen3-ASR torch 推理数据，sherpa-onnx onnxruntime int8 优化后内存显著更低
4. **SenseVoice CTC 兼容版（179MB）是零风险直换候选** — 体积更小、速度更快、内存更低，但无 hotwords（与当前 SenseVoice 同架构）
5. **Native RTF 0.185（4线程）通过 V1 标准，但 2线程 0.223 接近红线** — 桌面级低核 CPU 体验有风险

---

## 1. Step 1：模型下载与文件清单

### 1.1 SenseVoice CTC 兼容版（179MB 压缩）

- 下载源：`sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17.tar.bz2`（178.1MB）
- 解压目录：`models/sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17/`
- 解压后体积：**254MB**

| 文件 | 体积 | 说明 |
|------|------|------|
| model.int8.onnx | 251MB | 单一 ONNX 模型（CTC 架构） |
| tokens.txt | 917KB | 词表 |
| README.md | 1.9KB | 从 Fun-ASR-Nano-2512 转换 |
| test_wavs/ | — | 5 个 wav（zh/en/ja/ko/yue） |

**加载方式**：`OfflineSenseVoiceModelConfig`（与当前 SenseVoice 相同），**不支持 hotwords**

### 1.2 Native FunASR Nano 原生版（802.7MB 压缩）

- 下载源：`sherpa-onnx-funasr-nano-int8-2025-12-30.tar.bz2`（802.7MB）
- 解压目录：`models/sherpa-onnx-funasr-nano-int8-2025-12-30/`
- 解压后体积：**972MB**

| 文件 | 体积 | 说明 |
|------|------|------|
| encoder_adaptor.int8.onnx | 227MB | 音频编码器+适配器 |
| llm.int8.onnx | 572MB | Qwen3-0.6B LLM decoder（int8） |
| embedding.int8.onnx | 148MB | 词嵌入 |
| Qwen3-0.6B/ | — | tokenizer（merges.txt + tokenizer.json + vocab.json） |
| README.md | 253B | 来自 zengshuishui/FunASR-nano-onnx |
| test_wavs/ | — | 25 个 wav（方言/远场/歌词/噪声/RAG/中英混合/越南语） |

**加载方式**：`OfflineFunASRNanoModelConfig`（encoder+LLM decoder 架构），**支持 config 层 hotwords**

---

## 2. Step 2：PoC bin

### 2.1 源码

- 文件：`src/bin/poc_funasr_nano.rs`（240 行，8430 字节）
- 编译：`cargo build --release --bin poc_funasr_nano` → 0 errors，3 warnings（unused imports，无害）
- 产物：`target/release/poc_funasr_nano.exe`（361KB）

### 2.2 命令行接口

```
poc_funasr_nano [wav...] [--model-type sensevoice|funasr-nano] [--hotwords "w1,w2"] [--threads N] [--repeat N]
```

- `--model-type sensevoice`：加载 179MB CTC 兼容版（OfflineSenseVoiceModelConfig）
- `--model-type funasr-nano`：加载 802.7MB 原生版（OfflineFunASRNanoModelConfig，含 hotwords）
- `--hotwords`：注入 config 层 hotwords（仅 funasr-nano 生效；sensevoice 无此字段）
- `--threads`：默认 4
- `--repeat`：重复次数取均值，默认 1

### 2.3 输出

每个 wav 输出：模型加载耗时 / 每次运行的音频时长、转录耗时、RTF、识别文本 / 平均 RTF。

### 2.4 关键实现决策

- **hotwords 注入方式**：config 层 `OfflineFunASRNanoModelConfig.hotwords`，**不是** `create_stream_with_hotwords`
  - 原因：`create_stream_with_hotwords` 在 runtime 层报错 `"Only transducer models support contextual biasing"`（FunASR Nano 是 encoder+LLM decoder，非 transducer）
  - config 层 hotwords 作为 LLM prompt 前缀注入，全局生效
- **模型路径解析**：`exe.parent().parent().parent().join("models")`（从 target/release/ 向上 3 级到项目根）
- **funasr-nano tokens 字段**：留空字符串（tokenizer 目录已含 vocab，避免冲突）

---

## 3. Step 3：V1 CPU RTF 基准

### 3.1 测试环境

- **CPU**：AMD Ryzen 7 7840HS w/ Radeon 780M Graphics
- **逻辑核数**：16
- **测试音频**：短 ~5.5s (rag_math.wav)、中 ~10.6s (noise_en.wav)、长 ~21.8s (far_3.wav)
- **重复次数**：每个音频 repeat 3 取均值

### 3.2 Native FunASR Nano 原生版 RTF

| 音频 | 时长 | 线程 | 平均转录耗时 | RTF | 加载耗时 |
|------|------|------|------------|-----|---------|
| rag_math.wav | 5.52s | 4 | 0.840s | 0.1523 | 5.96s |
| noise_en.wav | 10.57s | 4 | 1.953s | **0.1848** | 5.82s |
| far_3.wav | 21.80s | 4 | 3.728s | 0.1710 | 5.79s |
| rag_math.wav | 5.52s | 2 | 0.997s | 0.1805 | 5.92s |
| noise_en.wav | 10.57s | 2 | 2.353s | **0.2227** | 5.93s |
| far_3.wav | 21.80s | 2 | 4.859s | 0.2229 | 5.65s |

### 3.3 SenseVoice CTC 兼容版 RTF

| 音频 | 时长 | 线程 | 平均转录耗时 | RTF | 加载耗时 |
|------|------|------|------------|-----|---------|
| rag_math.wav | 5.52s | 4 | 0.070s | 0.0128 | 1.57s |
| noise_en.wav | 10.57s | 4 | 0.120s | **0.0114** | 1.55s |
| far_3.wav | 21.80s | 4 | 0.250s | 0.0115 | 1.54s |
| rag_math.wav | 5.52s | 2 | 0.104s | 0.0189 | 1.63s |
| noise_en.wav | 10.57s | 2 | 0.184s | **0.0174** | 1.61s |
| far_3.wav | 21.80s | 2 | 0.415s | 0.0190 | 1.63s |

### 3.4 V1 结论

| 模型 | 10s 音频 4线程 | 10s 音频 2线程 | 判定 |
|------|--------------|--------------|------|
| Native FunASR Nano | 1.95s (RTF 0.185) | 2.35s (RTF 0.223) | ✅ 通过（4线程 ≤2s）；⚠️ 2线程接近红线 |
| SenseVoice CTC 兼容版 | 0.12s (RTF 0.011) | 0.18s (RTF 0.017) | ✅ 远超通过标准 |

**Native FunASR Nano V1 通过**：4 线程 10s 音频 1.95s ≤ 2s 标准。2 线程 2.35s 接近 3s 红线，低核 CPU 体验有风险。

---

## 4. Step 4：V3 内存基准

### 4.1 测试方法

PowerShell `Get-Process` 采样 `PeakWorkingSet64`，转录 far_3.wav (21.8s) 过程中采样。

### 4.2 内存数据

| 模型 | 稳态 WorkingSet | 峰值 WorkingSet | 加载耗时 | 对比当前 SenseVoice ~500MB |
|------|----------------|----------------|---------|---------------------------|
| Native FunASR Nano | 1338MB | **1619MB** | 5.9s | ~3.2x（但仍 <2GB 标准） |
| SenseVoice CTC 兼容版 | 290MB | **362MB** | 1.6s | ~0.7x（比当前更省） |

### 4.3 V3 结论

| 模型 | 峰值内存 | 判定 |
|------|---------|------|
| Native FunASR Nano | 1.6GB | ✅ 通过（<2GB 标准，远低于研究预估 4-5GB） |
| SenseVoice CTC 兼容版 | 362MB | ✅ 通过（远低于当前 SenseVoice） |

**关键修正**：研究报告（基于 Qwen3-ASR torch 推理）预估内存 4-5GB，实测 sherpa-onnx onnxruntime int8 优化后仅 1.6GB，**远低于预估**。这是因为 onnxruntime int8 量化 + 内存优化比 torch float32 推理节省 ~60%。

---

## 5. Step 5：Hotwords 通路验证

### 5.1 验证方法

对比同一音频带/不带 `--hotwords` 的识别输出，确认 config 层 hotwords 生效。

### 5.2 验证结果

| 音频 | 无 hotwords | 有 hotwords | 纠偏效果 |
|------|------------|------------|---------|
| rag_chemistry.wav | "比如说**紫菜**，当时被认为是一种含氧酸盐。" | "比如说**酯**，在当时被认为是一种含氧酸盐。" | ✅ **有效纠正**（紫菜→酯） |
| far_2.wav | "然后被灌停了渣男线的城防...首都公路站...八号线" | "然后被灌停了渣男线的车...省公路站...8号线" | ⚠️ 部分有效（"8号线"纠正，"沈杜公路"未纠正） |

### 5.3 Hotwords 通路结论

- ✅ **config 层 hotwords 通路可用** — `OfflineFunASRNanoModelConfig.hotwords` 实证影响 LLM decoder 输出
- ⚠️ **纠偏效果不稳定** — rag_chemistry 完全纠正，far_2 仅部分纠正（"沈杜公路"未生效）
- ⚠️ **hotwords 引入额外延迟** — rag_chemistry 0.65s→0.87s（+0.2s），far_2 1.58s→1.65s
- ❌ **`create_stream_with_hotwords` 不可用** — runtime 报错 "Only transducer models support contextual biasing"，FunASR Nano 必须用 config 层全局 hotwords（无法 per-stream 动态切换）
- **002B 需验证**：送气短词（派/七/厂）+ hotwords 的纠偏实效，这是 Gavin 核心诉求

### 5.4 关键限制：per-stream hotwords 不可用

sherpa-onnx runtime 的 `create_stream_with_hotwords` 仅支持 transducer 模型（如 zipformer transducer）。FunASR Nano（encoder+LLM decoder）必须通过 config 层全局 hotwords，意味着：
- 每次切换 hotwords 词表需重建 recognizer（~5.9s 加载）
- 无法在一次 recognizer 会话中为不同录音注入不同 hotwords
- **对项目影响**：若采用 hotwords 路线，需在启动时根据用户词库固定 hotwords，或接受重建延迟

---

## 6. 识别输出对比示例

### 6.1 far_3.wav（21.8s 中文远场对话）

- **SenseVoice CTC**："周末要不要去露营最近天气超舒服露营我怕虫子咬而且晚上睡帐篷会不会很冷啊放心我借了专业装备还有**暖宝宝**再带点火锅食材边吃边看星星超惬意"（无标点）
- **Native FunASR Nano**："周末要不要去露营？最近天气超舒服。露营我怕虫子咬，而且晚上睡帐篷会不会很冷啊。放心，我借了专业装备，还有**男宝宝**，再带点火锅食材，边吃边看星星超惬意。"（有标点，但"暖宝宝"误为"男宝宝"）
- **参考转录**："周末要不要去露营，最近天气超舒服，露营？我怕虫子咬，而且晚上睡帐篷会不会很冷啊？放心，我借了专业装备还有暖宝宝，再带点火锅食材，边吃边看星星超惬意。"

**观察**：Native 版输出带标点（体验更好），但有同音误识别；CTC 版无标点但"暖宝宝"正确。

---

## 7. 验收标准核对

- [x] 模型落盘且清单核实（两套模型，254MB + 972MB）
- [x] poc_funasr_nano.rs 编译通过（cargo build --release --bin poc_funasr_nano 0 errors，3 warnings 无害）
- [x] RTF 表覆盖 短/中/长 × 2/4 线程，附 CPU 型号（AMD Ryzen 7 7840HS）
- [x] 内存两项数据（稳态+峰值，两模型均测）
- [x] hotwords 通路开/关均能出结果（native 版 config 层；sensevoice 无 hotwords 字段）
- [x] V1 结论明确（native 4线程通过/2线程接近红线；sensevoice 远超通过）

---

## 8. 对研究报告的勘误（主控附加要求 ④）

研究报告 `collab/research/qwen3-asr-feasibility.md` 需修正以下错误：

| 章节 | 原说法（错误） | 修正为 |
|------|--------------|-------|
| 1.2 | "FunASR Nano int8（179MB 压缩）...支持 hotwords" | 179MB 是 SenseVoice CTC 兼容版，**不支持 hotwords**；802.7MB 原生版才支持 |
| 2.2 | "体积（压缩包）179MB...支持 hotwords" | 179MB CTC 版无 hotwords；802.7MB 原生版有 hotwords，解压后 972MB |
| 7 | "FunASR Nano (sense-voice) 179MB ✅ hotwords" | 179MB ❌ 无 hotwords；802.7MB 原生版 ✅ 有 hotwords |

勘误已同步至研究报告文首勘误注记（见 `qwen3-asr-feasibility.md` 顶部）。

---

## 9. 阶段二 002B 建议

基于本次 PoC 数据，对 002B（hotwords 短词纠偏实效验证）的建议：

1. **用 native funasr-nano 原生版**（802.7MB），不要用 179MB CTC 版（无 hotwords）
2. **hotwords 通过 config 层注入**，不要用 `create_stream_with_hotwords`（对非 transducer 报错）
3. **测试组设计**：
   - (a) 当前 SenseVoice（baseline，54% 正确率）
   - (b) Native FunASR Nano 无 hotwords
   - (c) Native FunASR Nano + hotwords=[送气短词词表]
   - (d) SenseVoice CTC 兼容版 179MB（作为零风险直换候选，无 hotwords）
4. **注意 hotwords 重建延迟**：每次切换 hotwords 需重建 recognizer（~5.9s），002B 测试脚本应预加载固定 hotwords 词表
5. **关注 far_2 部分纠正现象**：hotwords 纠偏不稳定，需多词多轮测试统计显著性

---

## 10. 边界遵守

- ✅ 未修改主线代码（仅新增 `src/bin/poc_funasr_nano.rs`）
- ✅ 未改 Cargo.toml / src-tauri / ui / config
- ✅ 未出包，未碰 Publish/
- ✅ 未新增 crate 依赖（复用 sherpa-onnx 1.12.38）
- ✅ UTF-8 编码（write 工具用 Windows 路径 D:\...）