# Qwen3-ASR-0.6B 替换可行性研究报告

> 任务：RESEARCH-QWEN3ASR-001
> 负责人：coder-1
> 日期：2026-07-06
> 类型：纯研究，零代码改动
> 结论：**观望（有条件 go）** — 主推先 PoC 验证 FunASR Nano int8（更优候选），Qwen3-ASR-0.6B 因体积超红线暂不主推

---

## ⚠️ 勘误注记（2026-07-06 POC-QWEN3ASR-002A 实测订正）

POC-QWEN3ASR-002A 下载模型实测后发现，本报告关于 FunASR Nano 的以下说法**有误，需订正**：

| 章节 | 原说法（错误） | 修正（基于 PoC 实测） |
|------|--------------|---------------------|
| 1.2 / 2.2 / 7 | "FunASR Nano int8（179MB）支持 hotwords" | **179MB 是 SenseVoice CTC 兼容版（`sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17`），用 `OfflineSenseVoiceModelConfig` 加载，CTC 架构，不支持 hotwords** |
| 1.2 / 2.2 / 7 | （未区分两版本） | **802.7MB 是原生版（`sherpa-onnx-funasr-nano-int8-2025-12-30`），用 `OfflineFunASRNanoModelConfig` 加载，encoder+LLM decoder 架构，支持 config 层 hotwords；解压后 972MB** |
| 4.1 | "Qwen3-ASR 内存 4-5GB" | PoC 实测 native funasr-nano 峰值内存仅 **1.6GB**（onnxruntime int8 优化远低于 torch float32 预估） |
| 9.2 V2 | "create_stream_with_hotwords 支持 per-stream 注入" | **`create_stream_with_hotwords` 对非 transducer 模型报错 "Only transducer models support contextual biasing"**；FunASR Nano 必须用 config 层全局 hotwords（无法 per-stream 动态切换） |

**PoC 实测数据详见**：`collab/research/poc-funasr-nano-A.md`

**核心结论修正**：hotwords 通路在 native 原生版 config 层**实证可用**（纠正"紫菜"→"酯"），但纠偏效果不稳定（部分词有效部分无效），且无法 per-stream 动态切换。"观望（有条件 go）"结论不变，但 PoC 数据已大幅充实。

---

## 0. 执行摘要

| 维度 | Qwen3-ASR-0.6B-int8 | FunASR Nano int8 (sense-voice) | 当前 SenseVoice int8 |
|------|---------------------|-------------------------------|---------------------|
| 引擎兼容性 | ✅ sherpa-onnx 1.12.38 原生 | ✅ sherpa-onnx 1.12.38 原生 | ✅ 当前方案 |
| 模型体积（解压后） | 🔴 938MB（42+174+722） | 🟢 ~180MB | 🟢 237MB |
| Hotwords | ✅ 支持（Rust API 已暴露） | ✅ 支持（Rust API 已暴露） | ❌ 不支持（CTC） |
| 中文 WER | 优（AISHELL2 3.15%, Fleurs-zh 2.88%） | 优（AIShell1 1.80%, Fleurs-zh 2.56%） | 中（无直接对比数据） |
| CPU RTF | 🟡 0.05-0.17（M4 Pro，内存 4-5GB） | 🟡 同量级（基于 Qwen3-0.6B，预期略优） | ✅ 极快（CTC 一次前向） |
| 语言覆盖 | 30 语 + 22 中文方言 | 🟡 Fun-ASR-Nano: 中/英/日 + 方言；MLT-Nano: 31 语（不含日） | zh/en/ja/ko/yue |
| 许可证 | Apache-2.0 | Apache-2.0 | Apache-2.0 |

**核心判断**：
- Qwen3-ASR-0.6B 解压后 938MB，远超历史 600MB 红线（`logs/20260507.md` RESEARCH-CS-001），**分发体积不可接受**
- FunASR Nano int8（179MB 压缩）是同源 Qwen3-0.6B 的 ASR 微调，体积比当前 SenseVoice 还小，且支持 hotwords，**是更优候选**
- 两者 CPU RTF 均在可接受范围（0.05-0.17），但内存占用 4-5GB 是新约束（当前 SenseVoice ~500MB）
- 短词首字纠偏（本项目核心痛点）的 hotwords 实效**尚无第三方实测数据**，必须 PoC 验证

**结论**：**观望（有条件 go）** — 建议阶段二 PoC 优先验证 FunASR Nano int8 的 hotwords 对"派/七/厂"类送气短词的纠偏实效与桌面级 CPU RTF；Qwen3-ASR-0.6B 因体积暂不主推，待体积优化（GGUF 量化/llama.cpp 路线）后再评估。

---

## 1. 维度 3：推理引擎兼容性（gating 项）

### 1.1 Qwen3-ASR-0.6B — sherpa-onnx 官方支持

**结论：✅ 已原生支持，无需升级依赖**

- 当前项目 `Cargo.lock` 锁定 `sherpa-onnx 1.12.38`（2026-04-13 发布），**已包含** Qwen3-ASR Rust API
- Rust 高层 API：`sherpa_onnx::OfflineQwen3ASRModelConfig`（`sherpa-onnx/rust/sherpa-onnx/src/offline_asr.rs:325`）
  - 字段：`conv_frontend`, `encoder`, `decoder`, `tokenizer`, `hotwords: Option<String>`
  - 与现有 `OfflineSenseVoiceModelConfig` 同级，挂载于 `OfflineModelConfig.qwen3_asr`
- 官方 Rust 示例：`rust-api-examples/examples/qwen3_asr.rs`
- 模型资产：`sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2`（asr-models release）
- 支持来源：PR #3399（2026-04 合并）、PR #3476（CI 启用 Qwen3 测试）

**来源**：
- https://github.com/k2-fsa/sherpa-onnx/blob/master/sherpa-onnx/rust/sherpa-onnx/src/offline_asr.rs#L325
- https://github.com/k2-fsa/sherpa-onnx/blob/master/rust-api-examples/examples/qwen3_asr.rs
- https://github.com/k2-fsa/sherpa-onnx/pull/3399
- https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2

### 1.2 FunASR Nano — sherpa-onnx 官方支持

**结论：✅ 已原生支持，无需升级依赖**

- 同样在 sherpa-onnx 1.12.38 中可用：`OfflineFunASRNanoModelConfig`
  - 字段：`encoder_adaptor`, `llm`, `embedding`, `tokenizer`, `hotwords: Option<String>`
- 官方 Python 示例：`python-api-examples/offline-funasr-nano-decode-files.py`
- 模型资产（两个版本）：
  - `sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17.tar.bz2`（**179MB**，标注 sense-voice）
  - `sherpa-onnx-funasr-nano-int8-2025-12-30.tar.bz2`（802.7MB，纯 funasr-nano）

**来源**：
- https://github.com/k2-fsa/sherpa-onnx/blob/master/sherpa-onnx/rust/sherpa-onnx/src/offline_asr.rs#L392
- https://github.com/k2-fsa/sherpa-onnx/blob/master/python-api-examples/offline-funasr-nano-decode-files.py
- https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17.tar.bz2

### 1.3 gating 判定

**两项均通过 gating** — sherpa-onnx 1.12.38 原生支持，集成成本量级低（替换 `create_sensevoice_recognizer` 函数约 30 行 + 模型目录名）。无需引入新 runtime（onnxruntime 已随 sherpa-onnx-sys 传递引入）。

---

## 2. 维度 1：模型基本面

### 2.1 Qwen3-ASR-0.6B

| 项 | 值 | 来源 |
|---|---|---|
| 权重开源 | ✅ HuggingFace `Qwen/Qwen3-ASR-0.6B` + ModelScope | [HF](https://huggingface.co/Qwen/Qwen3-ASR-0.6B) |
| 许可证 | Apache-2.0 | HF 模型卡 |
| 发布时间 | 2026-01（arXiv 2601.21337） | HF |
| 参数量 | 0.9B（BF16） | HF 模型卡 |
| 量化版本 | int8（sherpa-onnx 导出） | asr-models release |
| **体积（解压后）** | **938MB**（conv_frontend 42M + encoder.int8 174M + decoder.int8 722M + tokenizer ~328B） | issue #3409 官方 ls 输出 |
| 体积（压缩包） | 838MB | asr-models release asset size |

**体积红线判定**：解压后 938MB >> 历史 600MB 红线（`logs/20260507.md` RESEARCH-CS-001 将 600MB+ 判为"过大"）。**分发体积不可接受**。

### 2.2 FunASR Nano

| 项 | 值 | 来源 |
|---|---|---|
| 权重开源 | ✅ ModelScope `FunAudioLLM/Fun-ASR-Nano-2512` + HF | [GitHub](https://github.com/FunAudioLLM/Fun-ASR) |
| 许可证 | Apache-2.0 | Fun-ASR LICENSE |
| 发布时间 | 2025-12（2512 版本） | Fun-ASR README |
| 参数量 | 0.8B | Fun-ASR README |
| 量化版本 | int8（sherpa-onnx 导出） | asr-models release |
| **体积（压缩包）** | **179MB**（sense-voice-funasr-nano-int8-2025-12-17） | asr-models release asset size |
| 架构基础 | 基于 Qwen3-0.6B 的 ASR 微调（encoder adaptor + LLM decoder） | sherpa-onnx issue #3062、Fun-ASR README |
| 替代路线 | llama.cpp + GGUF（2026/06），量化后 ~484MB，单二进制 | Fun-ASR README "What's New" |

**体积判定**：179MB 压缩包 < 当前 SenseVoice 237MB，**体积优势明显**。

---

## 3. 维度 2：准确率指标

### 3.1 Qwen3-ASR-0.6B 官方 benchmark（WER ↓）

来源：[HF 模型卡](https://huggingface.co/Qwen/Qwen3-ASR-0.6B) "Evaluation" 章节，vLLM + bfloat16 + greedy

| 测试集 | Qwen3-ASR-0.6B | Qwen3-ASR-1.7B | Whisper-large-v3 |
|--------|---------------|---------------|------------------|
| AISHELL-2-test | 3.15 | **2.71** | 5.06 |
| WenetSpeech net | 5.97 | **4.97** | 9.86 |
| Fleurs-zh | 2.88 | **2.41** | 4.09 |
| CV-zh | 6.89 | **5.35** | 12.91 |
| Fleurs-en | 4.39 | 3.35 | 4.08 |
| Librispeech clean | 2.11 | 1.63 | 1.51 |
| KeSpeaker（方言） | 7.08 | **5.10** | 28.79 |

**HF open-asr-leaderboard**：Mean WER 6.42，RTFx 166.23（GPU）

### 3.2 Fun-ASR-Nano 官方 benchmark（WER %）

来源：[Fun-ASR README](https://github.com/FunAudioLLM/Fun-ASR) "Performance" 章节

| 测试集 | Fun-ASR-nano (0.8B) | Fun-ASR (7.7B) | Whisper-large-v3 | FireRed-ASR (1.1B) |
|--------|---------------------|---------------|------------------|---------------------|
| AIShell1 | 1.80 | 1.22 | 4.72 | 0.54 |
| AIShell2 | 2.75 | 2.39 | 4.68 | 2.58 |
| Fleurs-zh | 2.56 | 2.53 | 5.18 | 4.81 |
| Fleurs-en | 5.96 | 4.74 | 6.23 | 10.79 |
| Librispeech-clean | 1.76 | 1.51 | 1.86 | 1.84 |
| WenetSpeech Meeting | 6.60 | 6.17 | 18.39 | 4.95 |
| WenetSpeech Net | 6.01 | 5.46 | 11.89 | 4.94 |

**Industry benchmark（WER %）**：Fun-ASR-nano 平均 16.72，优于 Whisper-large-v3 (33.39)、Paraformer v2 (23.49)，略逊于 FireRed-ASR (22.63) 与 Fun-ASR 7.7B (12.70)。

### 3.3 短语音/孤立短词表现（本项目核心痛点）

**关键缺陷：无针对孤立短词（2-3 字）的第三方实测数据**

- Qwen3-ASR / FunASR Nano 的官方 benchmark 均基于完整句段测试集（AISHELL/WenetSpeech 等），**无 2-3 字孤立短词测试**
- 本项目痛点是"派对/七/厂"类送气清声母短词首字识别（`troubleshooting.md` [FIRSTCHAR-002]），FIX-006 后正确率 ~54%，剩余为模型固有局限
- Qwen3-ASR / FunASR Nano 均为 encoder + LLM decoder 架构（非 CTC），理论上 LLM 的上下文理解能力对短词有增益，但**孤立短词无上下文，增益不确定**
- sherpa-onnx issue #3509 报告 Qwen3-ASR hotwords + language 参数在空音频上产生不同输出，说明 hotwords 确实影响 decoder——**纠偏机制存在，但实效需 PoC 验证**

### 3.4 Context biasing / hotwords（最大潜在增益点）

**Qwen3-ASR hotwords**：
- Rust API：`OfflineQwen3ASRModelConfig.hotwords: Option<String>`（comma-separated）
- `OfflineRecognizer::create_stream_with_hotwords(&self, hotwords: &str)` 支持per-stream 注入
- sherpa-onnx issue #3509 确认 hotwords 参数生效（虽在空音频上有副作用 bug，open 状态）
- 注入方式：作为 decoder prompt 前缀，影响 LLM 解码分布

**FunASR Nano hotwords**：
- Rust API：`OfflineFunASRNanoModelConfig.hotwords: Option<String>`
- 官方示例：`hotwords=["开放时间"]`、`hotwords=["张三","北京"]`
- Fun-ASR README 明确列出 "hotwords" 为核心特性
- 注入方式：同 Qwen3-ASR（基于 Qwen3-0.6B）

**对短词纠偏的预期**：
- 机制上：hotwords 提升"派/七/厂"等目标词的解码概率，可能纠正 /pʰ/→/a/ 的声母丢失
- 风险：hotwords 作为 prompt 注入，对 LLM decoder 的纠偏强度取决于模型训练时是否针对 hotwords 做了显式优化；空音频 bug（#3509）提示机制尚不完全稳定
- **必须 PoC 实测**：用"派对/派发/七/厂"等送气短词 + hotwords 列表，对比有无 hotwords 的识别正确率

---

## 4. 维度 4：资源与速度

### 4.1 Qwen3-ASR-0.6B CPU RTF 实测

**关键数据点 1（issue #3110 评论，samshipengs，Apple M4 Pro 12 核，torch.float32）**：

| 音频 | 时长 | 转录耗时 | RTF | 峰值内存 |
|------|------|---------|-----|---------|
| 英文 speech | 15.1s | 2.53s | 0.168 | 4,303 MB |
| 中文 speech | 4.2s | 675ms | 0.161 | 4,402 MB |
| 合成 5s | 5.0s | 422ms | 0.084 | 4,449 MB |
| 合成 10s | 10.0s | 577ms | 0.058 | 4,586 MB |
| 合成 30s | 30.0s | 1.49s | 0.050 | 5,342 MB |

模型加载：首次 59s（含下载），缓存后 ~4s。

**关键数据点 2（issue #3569，AMD EPYC 7642 48 核服务器，2 线程，sherpa-onnx onnxruntime）**：
- 用户实测 2.9s（官方示例 0.7s），差距 4 倍——**桌面级 CPU + 2 线程会显著更慢**

**体验红线评估**：
- 本项目典型语音 3-10s，按 M4 Pro 数据 RTF 0.08-0.16 → 转录 0.4-1.6s，**可接受**
- 但 x86 桌面级 CPU（用户实际环境）预期 RTF 0.3-0.5 → 10s 语音转录 3-5s，**接近红线**
- **内存 4-5GB 是新硬约束**（当前 SenseVoice ~500MB），多任务环境可能压力

### 4.2 FunASR Nano CPU RTF

**无直接实测数据**，但因基于 Qwen3-0.6B + 额外 encoder adaptor，预期：
- RTF 同量级或略高（encoder adaptor 增加少量计算）
- 内存同量级（~4-5GB）
- 179MB int8 模型加载更快（磁盘 I/O 少）

Fun-ASR 官方 llama.cpp/GGUF 路线（2026/06）提到"量化后 ~484MB，单二进制 CPU 运行"，但**该路线尚未集成进 sherpa-onnx Rust API**，属远期选项。

### 4.3 与当前 SenseVoice 对比

| 指标 | SenseVoice int8（当前） | Qwen3-ASR / FunASR Nano |
|------|------------------------|------------------------|
| 架构 | CTC offline，一次前向 | encoder + LLM decoder，自回归 |
| CPU 推理 | 极快（<0.5s for 10s audio） | 较慢（1-3s for 10s audio） |
| 内存 | ~500MB | 4-5GB |
| 模型加载 | ~1-2s | ~4s |

**体验影响**：用户按完热键到出字延迟将从 <1s 增至 1-3s，需评估用户接受度。

---

## 5. 维度 5：功能对齐清单

| 功能 | SenseVoice（当前） | Qwen3-ASR-0.6B | Fun-ASR-Nano-2512 | Fun-ASR-MLT-Nano-2512 |
|------|-------------------|----------------|-------------------|----------------------|
| 中文 | ✅ | ✅ + 22 方言 | ✅ + 7 方言 + 26 口音 | ❌（MLT 不含中文） |
| 英文 | ✅ | ✅ | ✅ + 多国口音 | ✅ |
| 日文 | ✅ | ✅ | ✅ | ❌ |
| 韩文 | ✅ | ✅ | ❌ | ✅ |
| 粤语 | ✅ | ✅（Cantonese） | ✅（7 方言之一） | ❌ |
| language 参数 | ✅ zh/en/ja/ko/yue | ✅ 30 语 | ✅ 中/英/日 | ✅ 28 语 |
| 标点输出 | ✅ | ✅ | ✅ | ✅ |
| 繁简 | ✅（后处理 normalize） | ✅ | ✅ | ✅ |
| ITN（数字/单位归一化） | ✅ use_itn | ✅ | ✅ itn=True | ✅ |
| Hotwords | ❌（CTC 不支持） | ✅ | ✅ | ✅ |

### 关键功能回退标注（主控附加要求 ②）

**Fun-ASR-Nano-2512（主模型，179MB）只支持中/英/日，不含韩文和粤语**：
- 当前 SenseVoice 支持 zh/en/ja/ko/yue 五语（`config.audio.transcription_language` 配置项）
- 切换 Fun-ASR-Nano-2512 将丢失 **韩文 (ko)** 和 **粤语 (yue)** 支持
- 影响：用户在 Voice 设置页选择的"韩文""粤语"选项将失效，属**功能回退**
- 缓解方案：
  - 方案 A：使用 Fun-ASR-MLT-Nano-2512（31 语含韩文，但不含日文/粤语）——仍丢日/粤
  - 方案 B：保留 SenseVoice 作为 ko/yue fallback，按 language 路由到不同模型——工程复杂度增加
  - 方案 C：仅在本项目核心痛点场景（中文短词）启用 FunASR Nano，其余语言沿用 SenseVoice

**Qwen3-ASR-0.6B 语言覆盖完整**（30 语 + 22 方言，含 zh/en/ja/ko/yue），**无功能回退**。

---

## 6. 维度 6：集成成本与风险

### 6.1 影响文件范围

| 文件 | 改动 | 风险 |
|------|------|------|
| `src/transcription/mod.rs` | `create_sensevoice_recognizer` → `create_qwen3_recognizer` 或 `create_funasr_nano_recognizer`（~30 行）；`ensure_sensevoice_model` 模型目录名 | 中（核心集成点） |
| `Cargo.toml` | 无需改动（sherpa-onnx 1.12.38 已含 API） | 低 |
| `src/config/mod.rs` | 新增 hotwords 配置字段（可选，用于预设词表） | 低 |
| `src/main.rs` | Transcriber 初始化传入 hotwords；pipeline 不变 | 低 |
| `ui/src/pages/Voice.tsx` | 可选：新增 hotwords 输入框 | 低 |
| `models/` 目录 | 新增模型子目录（~180MB-938MB） | 分发体积 |

### 6.2 FIRSTCHAR 系列修复保留性

- FIRSTCHAR-FIX-001~006 改动在 `src/audio/mod.rs`（音频前端：抗混叠重采样、find_speech_anchor 回溯、前导静音规整）
- 与 ASR 模型解耦，**可完全保留**
- 注意：prime/anchor/静音参数（PRE_ROLL_MS=600, silence_head=50ms, 回溯 150ms）针对 SenseVoice 调优，换模型后可能需重调：
  - Qwen3-ASR / FunASR Nano 是 LLM decoder，对前导静音的敏感度可能与 SenseVoice 不同
  - PoC 时需对比"派/七/厂"短词在现有音频参数 vs 重调参数下的识别率

### 6.3 回归面评估

- 翻译功能（`src/translation/`）：不受影响（输入是 ASR 文本输出）
- LLM 优化（`src/llm/`）：不受影响
- 注入（`src/injection/`）：不受影响
- 词库（`src/wordbook/`）：不受影响
- 配置持久化（`src/config/`）：新增 hotwords 字段需向后兼容

---

## 7. 备选模型建议

任务要求：若发现更合适候选可附简要建议。

| 模型 | 体积（int8） | hotwords | 语言 | 评估 |
|------|------------|----------|------|------|
| **FunASR Nano (sense-voice)** | 179MB | ✅ | 中/英/日+方言 | 🥇 **主推 PoC**（体积小、hotwords、Apache-2.0） |
| Qwen3-ASR-0.6B | 938MB | ✅ | 30 语+22 方言 | 🥈 体积超红线，待 GGUF 量化后评估 |
| FireRedASR2-CTC | 496MB | ❌（config 无 hotwords） | 中英 | 🥉 CTC 快但无 hotwords，不解决核心痛点 |
| Dolphin small CTC | 182.7MB | ❌ | 多语 | 无 hotwords，无增益 |
| Cohere Transcribe 14-lang | ~? | ❌ | 14 语 | 无 hotwords |
| Fun-ASR-Nano GGUF (llama.cpp) | ~484MB | ✅ | 中/英/日 | 远期：未集成进 sherpa-onnx Rust API |

---

## 8. 结论与理由

### 三选一结论：**观望（有条件 go）**

**理由**：
1. **引擎兼容性已确认**（维度 3 通过）：sherpa-onnx 1.12.38 原生支持 Qwen3-ASR 和 FunASR Nano，无需升级依赖，集成成本量级低（~30 行代码）
2. **hotwords 能力是真实增益点**：当前 SenseVoice CTC 架构不支持 hotwords 是核心痛点根因之一（`troubleshooting.md` [FIRSTCHAR-002]），Qwen3-ASR/FunASR Nano 均支持，机制存在
3. **但短词纠偏实效未经验证**：无第三方孤立短词 + hotwords 实测数据，且 issue #3509 暴露 hotwords 在空音频上有副作用 bug（open），**必须 PoC 验证**
4. **Qwen3-ASR 体积超红线**：938MB >> 600MB 红线，分发不可接受
5. **FunASR Nano 是更优候选**：179MB < 当前 237MB，支持 hotwords，但语言覆盖有回退（丢 ko/yue）
6. **CPU RTF 与内存是新约束**：4-5GB 内存（vs 当前 500MB）和 1-3s 转录延迟（vs 当前 <1s）需评估用户接受度

**不直接 go 的原因**：体积（Qwen3）/语言回退（FunASR Nano）/短词纠偏实效未验证（两者）三项风险均需 PoC 数据决策。

### 不直接 no-go 的原因

- hotwords 是相对当前方案的**唯一结构性增益**，直接对应本项目核心痛点
- FunASR Nano 179MB 体积优秀，PoC 成本低（~30 行代码 + 下载模型）
- sherpa-onnx 集成已完成，技术门槛低

---

## 9. 阶段二 PoC 设计（主控附加要求 ③）

### 9.1 PoC 目标

验证 FunASR Nano int8（主候选）与 Qwen3-ASR-0.6B int8（对照）在以下三项的实效，为 go/no-go 决策提供数据。

### 9.2 PoC 验证项

| 验证项 | 方法 | 通过标准 | 失败标准 |
|--------|------|---------|---------|
| **V1: CPU RTF 实测** | 在目标硬件（桌面级 x86，如 i5/i7 消费级 CPU，4 线程）跑 sherpa-onnx Rust 示例 `qwen3_asr.rs` 与 `funasr_nano` 示例，测 3s/5s/10s/30s 音频 | 10s 音频转录 ≤2s（RTF ≤0.2） | 10s 音频转录 >3s（体验红线） |
| **V2: hotwords 短词纠偏实效** | 准备 20 个送气清声母短词音频（派/派对/派发/七/七百/厂/厂家/对/对方/踢/踢球 等），每组 10 次，对比 (a) SenseVoice 当前 (b) Qwen3-ASR 无 hotwords (c) Qwen3-ASR + hotwords=[词表] (d) FunASR Nano 无 hotwords (e) FunASR Nano + hotwords | (c)/(e) 正确率较 (b)/(d) 提升 ≥10pp，且较 (a) SenseVoice 54% 基线提升 ≥10pp | hotwords 无明显纠偏效果（<5pp 提升） |
| **V3: 内存占用** | 运行时测峰值内存（任务管理器或 `GetProcessMemoryInfo`） | ≤2GB | >4GB（影响多任务） |

### 9.3 PoC 执行步骤

1. **下载模型**（~180MB FunASR Nano + ~938MB Qwen3-ASR 对照）
2. **写 PoC 脚本**（独立 bin，不动主线代码）：基于 `rust-api-examples/examples/qwen3_asr.rs` 改造，加 hotwords 参数 + RTF/内存测量
3. **准备测试音频**：录制或合成 20 个送气短词 + 5 个整句对照
4. **跑 5 组对比**（a/b/c/d/e），每组 10 次，记录正确率
5. **输出 PoC 报告**：含 RTF 表、正确率对比表、内存表、go/no-go 建议

### 9.4 PoC 不做的事

- ❌ 不动主线 `src/transcription/mod.rs`
- ❌ 不改配置结构
- ❌ 不做出包
- ❌ 不评估 macOS（本项目 Windows-only，DEC-000）

### 9.5 PoC 派发建议

建议主控将阶段二拆为两个子任务并行派发：
- **POC-QWEN3ASR-002A**：coder-1 写 PoC bin + 跑 RTF/内存基准
- **POC-QWEN3ASR-002B**：tester-1 准备短词音频 + 跑 hotwords 纠偏对比（需 PoC bin 就绪后）

---

## 10. 关键来源链接汇总

| 数据 | 来源 |
|------|------|
| Qwen3-ASR-0.6B 模型卡 | https://huggingface.co/Qwen/Qwen3-ASR-0.6B |
| Qwen3-ASR 技术报告 | arXiv 2601.21337 |
| sherpa-onnx Rust API（OfflineQwen3ASRModelConfig） | https://github.com/k2-fsa/sherpa-onnx/blob/master/sherpa-onnx/rust/sherpa-onnx/src/offline_asr.rs#L325 |
| sherpa-onnx Rust 示例 | https://github.com/k2-fsa/sherpa-onnx/blob/master/rust-api-examples/examples/qwen3_asr.rs |
| sherpa-onnx Qwen3-ASR 支持 PR | https://github.com/k2-fsa/sherpa-onnx/pull/3399 |
| Qwen3-ASR 模型下载 | https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2 |
| Qwen3-ASR 解压后体积（938MB） | https://github.com/k2-fsa/sherpa-onnx/issues/3409 |
| Qwen3-ASR hotwords 空音频 bug | https://github.com/k2-fsa/sherpa-onnx/issues/3509 |
| Qwen3-ASR CPU RTF（M4 Pro, 0.05-0.17） | https://github.com/k2-fsa/sherpa-onnx/issues/3110#issuecomment |
| Qwen3-ASR CPU RTF（EPYC 7642, 2.9s） | https://github.com/k2-fsa/sherpa-onnx/issues/3569 |
| Fun-ASR 官方仓库 | https://github.com/FunAudioLLM/Fun-ASR |
| Fun-ASR-Nano 模型 | https://www.modelscope.cn/models/FunAudioLLM/Fun-ASR-Nano-2512 |
| Fun-ASR Nano WER benchmark | https://github.com/FunAudioLLM/Fun-ASR#performance- |
| Fun-ASR Nano hotwords 示例 | https://github.com/FunAudioLLM/Fun-ASR（README "Inference" 章节） |
| sherpa-onnx FunASR Nano Rust API | https://github.com/k2-fsa/sherpa-onnx/blob/master/sherpa-onnx/rust/sherpa-onnx/src/offline_asr.rs#L392 |
| FunASR Nano 模型下载（179MB） | https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17.tar.bz2 |
| sherpa-onnx 1.12.38 crates.io | https://crates.io/crates/sherpa-onnx/1.12.38 |
| 本项目 sherpa-onnx 锁定版本 | `voice-ime/Cargo.lock`（1.12.38） |
| 本项目 SenseVoice 集成点 | `voice-ime/src/transcription/mod.rs:87` |
| 体积红线历史判定 | `voice-ime/logs/20260507.md` RESEARCH-CS-001 |
| FIRSTCHAR 痛点根因 | `voice-ime/collab/troubleshooting.md` [FIRSTCHAR-002] |

---

## 11. 验收标准核对

- [x] 维度 3 引擎兼容性有明确结论（支持，无需升级依赖），附官方来源
- [x] 体积、WER、RTF 三项硬数据齐全且附链接
- [x] Context biasing 能力核实（Qwen3-ASR + FunASR Nano 均支持 hotwords，Rust API 已暴露）
- [x] 三选一结论明确（观望/有条件 go）
- [x] 报告写入指定路径 `collab/research/qwen3-asr-feasibility.md`
- [x] FunASR Nano 按 6 维度完整覆盖（维度 1-6 均含）
- [x] FunASR Nano 语言回退明确标注（丢 ko/yue，第 5 章）
- [x] 阶段二 PoC 设计附上（第 9 章，含 V1/V2/V3 验证项 + 执行步骤）
- [x] 更新 `logs/20260706.md` 追加研究摘要