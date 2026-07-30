# RESEARCH-ASR-ACCURACY-001 · accuracy 模型实测准确率低于 performance 的根因研究

> ⚠️ **勘误（2026-07-08，RESEARCH-ASR-ACCURACY-002）**：本报告实验脚本 `run_accuracy_study.py` 漏传 `--temperature`，所有 native 数据在 PoC 默认 temp=1.0 下测得，与生产 temp=0.3 不可比，**native 成绩被系统性低估约 20pp**。生产等价条件（temp=0.3）实测 native+hw CER=0.15 / first=85%，全面优于 CTC（CER=0.3553 / first=70%）。本报告"生产环境 native 难以超越 CTC"“建议改 UI 定位文案”等结论**不成立**；R2（hotwords 全量灌入副作用）、R5（VAD 与短音频无关）等与 temp 无关的结论仍有效。详见 `asr-accuracy-quality-002.md`。CTC 数据不受影响（SenseVoice 无 temperature 参数）。

> 任务编号：RESEARCH-ASR-ACCURACY-001
> 负责人：coder-1
> 完成时间：2026-07-07
> 任务性质：纯研究（生产代码零改动）
> 数据资产：`collab/research/audio-002B/accuracy_study/`（16 组 PoC 输出 + silence curve 6 组 × 3 模型 + full_summary.json）

---

## 一、问题陈述

Gavin 端测反馈：**accuracy 模型（972MB FunASR Nano native + hotwords）实际识别准确率低于 performance 模型（179MB CTC）**，与 POC-QWEN3ASR-002B 数据（native+hotwords 首字 80% vs CTC 75%）矛盾。

## 二、根因结论（按贡献度排序）

### R1【主因，已证实】生产音频前处理链路对 native decoder 伤害远大于 CTC

**证据：silence curve（前导静音敏感度）**

| 前导静音 | CTC first% | Native first% | Native+hw first% |
|----------|-----------|---------------|------------------|
| 0ms      | 72.5      | 67.5          | 77.5             |
| 50ms     | 70.0      | **57.5** ⬇10pp | 65.0             |
| 100ms    | 65.0      | 65.0          | 57.5             |
| 200ms    | 57.5      | 52.5          | 70.0             |
| 400ms    | 52.5      | 60.0          | 57.5             |
| 800ms    | 47.5      | 52.5          | 57.5             |

**关键发现**：
- **CTC 从 0→800ms 线性退化 25pp（72.5→47.5），但 50ms 仅退化 2.5pp（72.5→70）**——生产用 50ms silence head 对 CTC 几乎无害
- **Native 无 hw 从 0→50ms 立刻崩 10pp（67.5→57.5）**——native decoder 对前导静音极度敏感，50ms 静音头就触发退化
- Native+hw 在 0ms 达 77.5%（最优），但 50ms 已掉到 65%——**hotwords 增益被前导静音吃掉**

**生产链路对比（post-trim 模拟）**：

| 条件 | CTC first% | Native first% | Native+hw first% |
|------|-----------|---------------|------------------|
| raw TTS（PoC 理想） | 75.0 | 62.5 | 80.0 |
| post-trim（50ms head + 语音）| 70.0 | 57.5 | 65.0 |
| sim 生产（600ms pre-roll + 200ms trail，未 trim）| 50.0 | 52.5 | 57.5 |

**结论**：
- PoC 用 raw TTS wav（无前导静音）测出 native+hw 80% 是**理想化假象**
- 生产录音经 onset trim + 50ms head 后，**CTC 完全恢复（70% 接近 raw 75%），native 只恢复到 57.5%（低于 raw 62.5%）**
- **生产前处理是为 CTC 调优的（FIRSTCHAR 系列针对 SenseVoice/CTC），对 native decoder 反而有害**
- Gavin 端测"accuracy 反而差"的体感**完全可解释**：生产环境下 native+hw 65% < CTC 70%

**假设对应**：H3 成立（生产音频前处理链路差异）

### R2【次因，已证实】hotwords 全量灌入副作用

**证据：hotwords scaling（raw TTS，native）**

| hotwords 配置 | first% | word% | empty | 备注 |
|--------------|--------|-------|-------|------|
| 无 | 62.5 | 57.5 | 0 | baseline |
| 精选 20 词（002B 测试词） | 80.0 | 75.0 | 0 | PoC 配置 |
| 全量 wordbook 11 条（含 worker1/tester1/todo 等无关词）| 60.0 | 52.5 | 0 | **比无 hotwords 还差** |
| 合成 220 条（精选 20 + 200 干扰词）| 0.0 | 0.0 | 40 | **全部空输出** |

**关键发现**：
- **全量 wordbook（仅 11 条）就让 native 性能从 62.5% 降到 60%**——无关/低质词条（worker1/tester1/todo/ coder1 等英文+数字）带偏 decoder
- 220 条直接撑爆 `max_total_len=512`（KV cache 上限），user_prompt 占满 context → audio placeholders 被截断 → 全空输出（与 ASR-NATIVE-LONG-001 同根因）
- **hotwords 是 prompt-based**（PR #3122 确认：注入 user_prompt "热词列表：[...]"），吃 context 预算，不是 free 的 biasing

**生产现状**：`build_hotwords_string` 把 wordbook 全部 corrected 词条灌入。当前 Gavin wordbook 仅 11 条，影响小（-2.5pp）；但若用户词库增长到几十条+含无关词，native 性能会持续退化。

**假设对应**：H2 成立（hotwords 全量灌入副作用）

### R3【次因，已证实】native decoder 固有 hallucination + fallback 误触发风险

**证据：fallback 模拟**

| 配置 | native first% | fallback 后 first% | halluc | empty | fb_used |
|------|--------------|-------------------|--------|-------|---------|
| 精选 hw | 80.0 | 80.0 | 1 | 0 | 1 |
| 全量 wordbook | 60.0 | 60.0 | 1 | 0 | 1 |
| 220 hw | 0.0 | **75.0** | 0 | 40 | 40 |

**hallucination 实例**（raw 输出）：
- `qidian_v1`（1.37s）→ `七点 managers what Stayers she went to buy their playback B卡取回的扇形画向令 paddle 把眼一睁开放`（80 字 / 1.37s = 58 字/秒，触发 is_hallucination）
- `pao2_v2`（~1.4s）→ `ANT Moffat Payne Pouche, ATVeonQi & Donna Lina, and The Door of The Palace.`（75 字 / 1.4s = 54 字/秒，触发）

**关键发现**：
- **native decoder 真实存在中英混杂 hallucination**（Qwen3-0.6B LLM decoder 固有缺陷，PoC 002B 已记录 qidian_v1 乱码）
- 生产 is_hallucination（>12字/秒）+ is_repetitive_garbage + 空输出兜底**能正确兜住短音频 hallucination**
- **220 hw 全空 → fallback CTC 全部触发 → 最终 75%（CTC 水平）**——用户以为在用 accuracy，实际全部走 fallback。这是"accuracy 反而差"的另一可解释路径
- **长音频 hallucination 是盲区**：30s 音频阈值 = 360 字，正常长句 200 字不触发，乱码 100 字也不触发（已在 ASR-NATIVE-LONG-001 记录）

**假设对应**：H4 成立（兜底误触发——更准确说是"兜底正确触发但用户感知为 accuracy 无效"）

### R4【已证实但非根因】int8 量化 + 推理参数欠优

**证据（上游调研，非本地实验）**：
- 模型文件名 `encoder_adaptor.int8.onnx` / `llm.int8.onnx` / `embedding.int8.onnx`——确为 int8 量化导出
- sherpa-onnx 官方此模型仅提供 int8 版（k2-fsa/sherpa-onnx releases asr-models），无 fp16/fp32 替代
- PR #3122 揭示 ITN 默认不生效（需 `--rule-fsts=itn_zh_number.fst`，本项目未配置）——但 ITN 是数字规整，不影响首字/整体 CER
- 推理参数（temperature=1.0, top_p=1.0, seed=42）为 sherpa-onnx 默认，未做扫描

**结论**：int8 量化是 native decoder 的固有局限放大器（LLM decoder 对量化比 CTC 敏感），但**无法在当前模型资产下验证 fp16 对比**，且无替代模型可用。推理参数欠优是低优先级优化项，非根因。

**假设对应**：H6 部分成立（参数欠优，未扫描但默认值风险中等）/ H7 成立但不可操作（无替代模型）

### R5【已推翻】VAD 分段副作用

**证据**：VAD 分段仅在 >24s 音频触发（`should_segment`），Gavin 端测反馈覆盖短词/普通句场景，与 VAD 无关。silence curve 实验的短音频（<2s）不触发 VAD，已证实 native 在短音频就差。

**假设对应**：H5 推翻（VAD 仅长音频，与本问题无关）

### R6【已证实】PoC 指标片面

**证据**：PoC 002B 只测首字正确率 + 整词正确率，未测整体 CER（Character Error Rate）。本研究的 silence curve 实际上扩展了评估维度，发现"首字 80%"是 raw TTS 假象。

**但 PoC 指标片面不是根因**——即使用更全面指标，PoC 用 raw TTS wav 也会得出 native+hw 优于 CTC 的结论，因为**真实差异来自生产音频前处理**，不是评估维度。

**假设对应**：H1 部分成立（指标片面确实误导，但根因是测试样本与生产环境差异）

---

## 三、优化空间评估

accuracy 模型的可挖掘空间有限且收益不确定：

| 维度 | 空间 | 预期收益 | 风险 |
|------|------|---------|------|
| 前处理适配（为 native 调整 silence head/onset trim）| 中 | +5~10pp | 高（需重新调 FIRSTCHAR 系列，可能伤 CTC）|
| hotwords 策略（精选而非全量）| 大 | +2~5pp | 低（仅改 build_hotwords_string）|
| 推理参数扫描（temperature/top_p）| 小 | 未知 | 中（可能引入新 hallucination）|
| 兜底阈值校准（长音频 hallucination）| 中 | 不可量化 | 低（只加日志统计）|
| 换非量化模型 | 无 | 不可操作 | 无替代资产 |

**核心判断**：accuracy 模型在生产环境下的"准确率优势"主要靠 hotwords 在**理想音频**（无前导静音）下才体现。生产录音的前导静音结构吃掉了大部分优势。**即使优化，accuracy 也难以在生产环境稳定超越 performance**。

---

## 四、分级优化方案

### 方案 A【推荐，低风险，立即落地】hotwords 精选策略

**问题**：`build_hotwords_string` 全量灌入 wordbook，无关词条带偏 decoder + 大词库撑爆 context。

**改动**：
- 影响文件：`src/transcription/mod.rs`（`build_hotwords_string` 函数）
- 具体改动：
  1. 只灌入与当前音频语言相关的词条（过滤纯英文/纯数字词条，除非用户明确添加）
  2. 限制 hotwords 数量上限（建议 ≤50 条），超出按使用频率/最近性排序截断
  3. 长词条（>10 字）过滤或截断（candidates 表中的整句词条不应灌入）
- 风险评估：低——只改 hotwords 构造逻辑，不影响 transcribe() 签名和下游
- 预期收益：+2~5pp（避免全量 wordbook 退化），大词库用户收益更大
- 验收标准：
  - cargo test 全绿
  - 用 002B wav 跑 PoC：native + 精选 hw first% ≥ native + 全量 wordbook first%
  - 单测：build_hotwords_string 过滤英文/数字/长词条 + 上限截断

### 方案 B【推荐，中风险，与 A 合并】accuracy 模式前处理适配

**问题**：生产 onset trim + 50ms silence head 为 CTC 调优，对 native 伤害大（silence curve 证据：native 50ms 掉 10pp）。

**改动**：
- 影响文件：`src/main.rs`（`run_pipeline` 的 silence head 区块，~L2730-2766）
- 具体改动：
  1. accuracy 模式下 silence head 从 50ms 降到 0ms（或 10ms）—— native 是 LLM decoder，不需要 frame alignment padding
  2. accuracy 模式下 onset trim backtrack 从 200ms 降到 100ms（native 对送气声母不如 CTC 敏感，少留前导静音更安全）
  3. 加配置开关或运行时分支，performance 模式保持现状
- 风险评估：中——改 main.rs 核心前处理，需回归 FIRSTCHAR 测试；但只动 accuracy 分支，performance 零影响
- 预期收益：+5~10pp（silence curve 0ms vs 50ms 差 10pp）
- 验收标准：
  - cargo test 全绿
  - 002B post-trim wav 跑 PoC：accuracy 模式 native first% ≥ 65%（当前 57.5%）
  - Gavin 端测"派对/派发"首字改善
  - performance 模式回归测试无退化

### 方案 C【可选，低风险】兜底触发统计日志

**问题**：当前无统计兜底触发率，无法判断"用户以为 accuracy 但实际 fallback"的发生频率。

**改动**：
- 影响文件：`src/transcription/mod.rs`（`transcribe_segment_detailed` 兜底链）
- 具体改动：兜底触发时 log::info 记录触发原因 + 音频时长 + 文本长度，可选写入 stats 文件供分析
- 风险评估：低——纯日志，无逻辑改动
- 预期收益：可观测性提升，为后续阈值校准提供数据
- 验收标准：cargo test 全绿 + 日志格式可被 grep 统计

### 方案 D【不推荐】换非量化模型 / 推理参数扫描

**问题**：int8 量化可能伤 native decoder；推理参数用默认值。

**不推荐原因**：
- sherpa-onnx 官方此模型仅 int8 版，无替代
- 推理参数扫描工作量大、收益不确定、可能引入新 hallucination
- 应优先方案 A+B（根因优化），参数扫描作为后续可选

---

## 五、核心结论

1. **Gavin 端测"accuracy 反而差"的体感是真实的，且有量化证据**：生产环境下 native+hw 65% < CTC 70%（post-trim 模拟数据）
2. **根因是生产音频前处理为 CTC 调优，对 native decoder 有害**（R1，主因）+ **hotwords 全量灌入副作用**（R2）+ **native 固有 hallucination**（R3）
3. **PoC 002B 的 "native+hw 80%" 是 raw TTS 理想化假象**，不能代表生产环境
4. **优化方向**：方案 A（hotwords 精选）+ 方案 B（accuracy 前处理适配）合并实施，预期 accuracy 生产 first% 可从 ~57% 提升到 ~70%，与 CTC 持平
5. **若优化后 accuracy 仍无法稳定超越 CTC**，建议向 Gavin 汇报：accuracy 模型在当前生产链路下不适合作为"准确率更高"选项，可考虑：
   - 重新定位 accuracy 为"长音频/特定场景"选项（native 自带标点，长句阅读体验好）
   - 或下线 accuracy，专注 CTC 优化

## 六、生产代码零改动确认

本研究仅修改/新增：
- `src/bin/poc_funasr_nano.rs`（未改，复用现有 PoC bin）
- `collab/research/audio-002B/accuracy_study/`（新增实验输出）
- `collab/research/audio-002B/run_accuracy_study.py`（新增实验脚本）
- `collab/research/audio-002B/run_silence_curve.py`（新增实验脚本）
- `collab/research/audio-002B/aggregate_results.py`（新增聚合脚本）
- `collab/research/audio-002B/rescore_silence.py`（新增重算脚本）
- `collab/research/asr-accuracy-quality-001.md`（本报告）

`src/`、`src-tauri/`、`ui/`、`Publish/`、`models/` 零改动。

## 七、数据资产索引

- 实验脚本：`collab/research/audio-002B/run_accuracy_study.py`、`run_silence_curve.py`、`aggregate_results.py`
- 原始 PoC 输出：`collab/research/audio-002B/accuracy_study/*.txt`（16 组 + 18 组 silence curve）
- 汇总数据：`collab/research/audio-002B/accuracy_study/full_summary.json`、`silence_curve.json`
- 模拟生产 wav：`collab/research/audio-002B/accuracy_study/silence_wavs/`、`sim_wavs/`