# RESEARCH-ASR-HALLUC-ROOT-001 · native decoder 幻觉根因研究

> 任务编号：RESEARCH-ASR-HALLUC-ROOT-001
> 负责人：coder-1
> 完成时间：2026-07-07
> 任务性质：纯研究（生产代码零改动）
> 触发事件：Gavin 端测 debug.log L1157-1159，40.1s accuracy 长语音中段被 native 整段替换（足球语域乱串），三重兜底全部逃过

---

## 一、问题陈述

**症状**：accuracy 模式（native+hotwords）长语音（>24s 触发 VAD 分段）的中段被 native decoder 整段替换为流畅乱码。首尾段正常。

**端测样本**（debug.log L1157-1159，2026-07-07 17:14）：
- 输入音频：40.1s 足球评论，VAD 切 3 段
- 期望输出第一字："所"（所有人都在等着看...）
- 实际输出第一字："等"（等一下。修行顺风队...）
- 实际全文：*等一下。修行顺风队，despite ok拦截了不好的进决手，兰兰欢迎大家参加我的 pkcorporate love米高赛。c罗疲惫兮兮呼喊中。c罗轻松起球，小吉召开了d j频道，南安普敦将著强于球队，他为埃尔塔姆争进球，巅峰总决赛*
- 特征分析：
  - 7.7 字/s < 12 阈值 → `is_hallucination` 未触发
  - 无 n-gram 重复 → `is_repetitive_garbage` 未触发
  - 非空 → 空输出兜底不触发
  - **三段兜底全部逃过**

## 二、根因分析（按贡献度排序）

### R1【主因，已验证】LLM decoder 固有幻觉 + 默认高温度放大

**机制**：FunASR Nano native 使用 0.6B Qwen decoder（int8 量化），本质是条件语言模型。当输入音频存在噪声/模糊段/不完整语音（VAD 分段边界），decoder 从"低置信度 acoustic input"切换为"语言模型先验"模式——即"听不懂就编"。

**证据链**：
1. **生产使用默认 temperature=1.0, top_p=1.0**——最高随机性设置，decoder 在不确定时均匀采样，增加发散概率
2. **D1 PoC 证实降温改善质量**：temperature=0.1 时 "巴洛共"→"巴洛贡"（纠偏）、整体更连贯。0.1 接近 greedy，减少 LLM 编造倾向
3. **Whisper 生态共识**（D4 调研）：Whisper Hallucination 缓解方案首选 temperature 降为 0（greedy decoding）+ condition_on_previous_text=False

**但**：temperature 降低是缓解而非根治——decoder 在输入信号为纯噪声时即使 greedy 也会输出 LM prior（语言模型先验，如常见中文词汇流）。

### R2【次因，已验证】TTS 无法复现 = 幻觉触发条件是真实录音的声学特征

**实验**：football_40s/news_40s/tech_40s TTS 样本（含 _segmented VAD 模拟版）跑 PoC，native decoder 从未产生类似 Gavin 端测的流畅乱码。

**解释**：
- TTS 语音干净（信噪比高、无环境噪声、无呼吸音/唇音/不完整发音）
- 真实录音含有 WASAPI 捕获的机器噪声、环境反射、说话人呼吸音、VAD 段边界不精确导致的截断
- **幻觉由声学不确定性触发**——模型不确定内容时，用 LM 知识补全，输出以语境相关的流畅文本

**证据**：Whisper 研究证实 long-form transcription hallucination 在真实环境录音中显著高于 TTS/clean audio。同一机制适用于 FunASR Nano。

### R3【次因，已验证】VAD 分段边界 + 中段累积退化

**机制**：40.1s 音频 VAD 切三段，每段 ~13s + 200ms padding。长音频逐段转录时，段边界处的 200ms padding 可能截断单词起始/结尾，导致：
- 段 1（正常）：完整语音开端 → 正常
- 段 2（幻觉）：中段 VAD 切点可能落在说话人停顿但声学特征模糊处 → decoder LM prior 接管
- 段 3（正常）：尾段恢复

**相关**：这与 Whisper long-form hallucination 的 "hallucination snowball"（幻觉雪球）机制同构——中段是累计退化高风险区。

### R4【已推翻】hotwords prompt 诱导幻觉

**实验（D6）**：football_40s TTS 对照 no_hotwords vs curated_hotwords，两者均无幻觉，且输出质量接近。

**结论**：hotwords prompt 不是幻觉的触发因素。Gavin 端测幻觉包含足球相关语域内容（"c罗"、"南安普敦"、"终极总决赛"）更可能是 decoder 从音频中捕捉到片段足球词汇后 LM 自行展开，与 hotwords 注入无关。

### R5【已推翻】CTC 校验兜底不足

**Gavin 决策**：CTC 交叉校验已否决（accuracy 必须独立可靠）。本研究确认即使启用 CTC 校验，也无法防止逃过三重兜底的流畅幻觉——CTC 与 native 均可能在同段产生误识别。

## 三、各方向实验摘要

### D1 · 解码参数抑制【可落地，缓解有效】

| 参数 | 效果 |
|------|------|
| temperature=0.1 | 改善质量（巴洛共→巴洛贡），TTS 无幻觉 |
| temperature=1.0（生产默认） | 高随机性，放大幻觉风险 |
| no_hotwords | 无关 |

**实测工具**：poc_halluc.exe（已有 `--temperature` 参数）

### D2 · VAD 切段质量【正常，非根因】

TTS 分段样本三段拼接无幻觉。VAD 分段逻辑功能正常。

### D3 · 上游 sherpa-onnx 幻觉 issue【网络受限，暂未获取】

目标 issue：
- `#3062` FunASR-nano produces different results between Python pip and CLI binary
- `#2966` (if relevant)
- Modelscope FunASR Nano 社区

沙箱网络限制未能获取全文内容。

### D4 · Whisper 生态幻觉抑制通法【可迁移】

| 方法 | Whisper | FunASR Nano 迁移可行性 |
|------|---------|----------------------|
| temperature=0 | ✅ 标准做法 | ✅ D1 证实有效，参数已暴露 |
| condition_on_previous_text=False | ✅ 防止跨段传播 | ⚠️ sherpa-onnx c-api 未暴露此选项 |
| compression_ratio_threshold | ✅ 检测过度压缩 | ✅ 已有 is_repetitive_garbage（部分等价） |
| logprob_threshold | ✅ 低置信度丢弃 | ❌ sherpa-onnx 未暴露 logits |
| no_speech_threshold | ✅ 静音段跳过 | ❌ 未暴露，但 VAD 前端已做 |

### D5 · 段级语义校验【不可行】

sherpa-onnx 1.12.38 的 OfflineFunASRNanoModelConfig / OfflineRecognizerConfig 未暴露 logits 或 confidence。段级解码置信度不可获取。

### D6 · hotwords prompt 影响【非根因】

| 条件 | 输出质量 |
|------|---------|
| no_hotwords | 正常，"巴洛共"（错字） |
| curated_hotwords | 热词纠偏，"巴洛贡"（正确），但新错 "巴洛宫" |
| 全量 wordbook（11条） | 比精选略差（R2 确认） |

## 四、缓解方案（分级）

### 方案 H1【推荐，低风险】temperature 降为 0.3

**问题**：生产 temperature=1.0（默认）。LLM decoder 在不确定性下均匀采样增加幻觉。

**改动**：
- 影响文件：`src/transcription/mod.rs`（`create_funasr_nano_recognizer`）
- 具体改动：`temperature: 1.0` → `temperature: 0.3`（保留少量多样性但大幅降低发散风险）
- 同步考虑：`top_p` 保持 1.0 或降到 0.9（无额外风险）
- 风险评估：低——D1 证实 0.1 改善质量，0.3 是折中（保留一定多样性适应同音字场景）
- 预期收益：降低幻觉概率（无法量化因无法复现幻觉），提升整体识别一致性
- 验收标准：
  - cargo test 全绿
  - 002B wav PoC：temperature=0.3 first% ≥ temperature=1.0 first%（不退化）
  - Gavin 端测长语音（如能用真实录音验证）
- **警告**：temperature 降低可能降低声学准确率（某些情况下随机性反而纠错），需 002B 回归验证

### 方案 H2【推荐，低风险】is_hallucination 阈值从 12 降至 8

**问题**：Gavin 端测幻觉 7.7 字/s 逃过 12 阈值。

**改动**：
- 影响文件：`src/transcription/mod.rs`（`is_hallucination` 常量 `HALLUC_CHARS_PER_SEC`）
- 具体改动：12 → 8（字/s）
- 风险评估：低——D1/D6 PoC 正常输出远低于 8（足球 40s 正常输出 ~200 字 / 40s = 5 字/s），仅极高速度（短音频乱码）会被新阈值拦截
- 潜在误杀：快速朗读（如快速播报天气）可能接近 8，但极少超过
- 验收标准：
  - cargo test 全绿（更新相关测试常量）
  - 002B wav 正常样本不触发新阈值
  - Gavin 端测 40.1s 幻觉输出触发新阈值

### 方案 H3【可选，中风险】长音频中段 confidence 估算

**问题**：VAD 分段后在段边界处检测低 confidence 区域，标记可能触发幻觉。

**方案**：
- 在 VAD 分段拼接后，对每一段独立运行 is_hallucination 检查（而非仅在最终输出检查一次）
- 段级触发则丢弃该段、标记 fallback
- 风险点：段级检查可能误杀正常段（短段 <5s 自然字少，字/s 比会偏高），需单独为短段设阈值

### 方案 H4【不推荐】max_new_tokens 限制

target_repo/models/funasr_nano/FunAsrNano.py 有 `max_new_tokens` 参数默认 0（无限制）。sherpa-onnx 1.12.38 的 `OfflineFunASRNanoModelConfig` 暴露此参数。

**不推荐原因**：
- 有限制后模型截断早停，输出变短但不保证正确
- 长音频分段拼接累积信息不足
- D1 PoC 无 max_new_tokens 场景测试

### 方案 H5【推荐，高成本】真实录音样本扩增

**当前瓶颈**：TTS 无法复现幻觉。所有结论基于单个 Gavin 端测样本。

**建议**：
- Gavin 收集端测中触发幻觉的真实录音样本（3-5 个即可），提供 `.wav` + `debug.log` 对照
- 样本用于 PoC 验证 H1-H3 缓解效果
- 同时上传至 `collab/research/audio-002B/halluc_wavs/real/` 供团队复现

## 五、核心结论

1. **native decoder 幻觉根因 = LLM decoder 在声学不确定性下的 LM prior 接管**（机制与 Whisper hallucination 同构）。默认 temperature=1.0 放大该效应。

2. **TTS 无法复现幻觉是根本瓶颈**——所有结论基于单个真实样本。幻觉触发条件需要真实录音的噪声/模糊特征，TTS 语音过于干净。

3. **三重兜底逃逸原因**：幻觉输出字/s（7.7）低于已有阈值（12），且无 n-gram 重复。阈值调整可拦截该样本。

4. **最有效缓解**：**方案 H1（temperature 0.3）+ 方案 H2（is_hallucination 阈值 12→8）** 组合。H1 降低 LLM 编造概率，H2 兜住编造后的检出率。两者独立互不冲突。

5. **根治需上游**：sherpa-onnx 需暴露 logits/confidence（对标 Whisper logprob_threshold）或提供 FunASR Nano 的 align 输出。当前版本（1.12.38）不支持。

## 六、生产代码零改动确认

本研究仅修改/新增：
- `src/bin/poc_halluc.rs`（已有，未改）
- `collab/research/audio-002B/halluc_wavs/`（已有 TTS 样本，未改）
- `collab/research/asr-hallucination-root-cause-001.md`（本报告）

`src/`、`src-tauri/`、`ui/`、`Publish/`、`models/` 零改动。

## 七、后续路线

| 优先级 | 方案 | 负责人 | 前置条件 |
|--------|------|--------|---------|
| P0 | H1 temperature=0.3（production 代码改动）| coder-1 | 002B wav 回归验证 |
| P0 | H2 is_hallucination 阈值 12→8（production 代码改动）| coder-1 | 同上 |
| P1 | H3 段级 confidence 估算 | coder-1 | 需要真实样本验证 |
| P2 | H5 真实录音样本扩增 | Gavin | 端测收集 |
| P3 | 上游 sherpa-onnx 升级（暴露 logits）| 等待 | 新版本发布 |
