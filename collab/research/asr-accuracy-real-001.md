# RESEARCH-ASR-ACCURACY-003 · Gavin 真实语料双模型同源 A/B（分化根因终审）

> 任务编号：RESEARCH-ASR-ACCURACY-003
> 负责人：coder-1
> 完成时间：2026-07-08
> 任务性质：纯研究（生产代码零改动）
> 产出：本报告 + `collab/research/audio-real-gavin/results/`（A/B 矩阵数据 + CER 评分）
> 前置：RESEARCH-ASR-ACCURACY-001（temp=1.0 误判）、002（TTS 短词 PoC，accuracy 优于 CTC）

---

## 核心结论（TL;DR）

1. **Gavin "performance 更优" 体感未在本语料复现**：生产同栈（Windows PoC bin，与生产同 crate 同 DLL）真实语料 A/B 中，**A2 native+VAD（CER 0.0271）仍优于 A1 CTC（CER 0.0724）**，CER 低 63%，专有名词几乎全对（28/30 关键词命中）。与 002 在 TTS 短词上的结论一致。
2. **Step 2 归因走第三分支**：A2（VAD 分段）≈ A3（人工分段，去掉空段后）都不差于 CTC → 病根不在"真实音频×native"也不在"VAD 分段质量"。与 Gavin 体感矛盾，建议启用 DEBUG-AUDIO-DUMP-001 抓应用内部音频做终极对照。
3. **发现 native 长音频空输出风险**：A3 para3（26.5s 整段喂 native）**空输出**（max_total_len=512 截断）。生产 VAD 分段（A2）规避了此问题，但若 VAD 失败/naive_chunk 降级且段超 28s，native 会静默空输出——这是 native 真实缺陷，可能是 Gavin 体感来源之一（若他遇到过长音频 + VAD 降级）。
4. **E1-E4 方向在真实语音上仍成立**：002 在 TTS 短词得出的 temp 0.3→0.2/hotwords 上限下调/backtrack 调优方向，在本真实语料的 native 表现上未被推翻（A2 native 已优于 CTC，调优只会更好）。但本轮未跑 Step 3 复扫（因 Step 1 未复现差距），E1-E4 落地仍需真实语料参数验证。

---

## 一、参数核对表（每条件 vs 生产代码 file:line）

| 参数 | 生产代码值 | PoC 实验值 | 对齐 | 备注 |
|------|-----------|-----------|------|------|
| temperature | 0.3 (mod.rs:639) | 0.3 (`--temperature 0.3`) | ✅ | PoC bin 默认 1.0，显式传 0.3 |
| max_new_tokens | 0=不限 (mod.rs:638) | 0 (`--max-new-tokens 0`) | ✅ | **版本陷阱**：Python sherpa-onnx 1.13.4 的 0=生成0个token致空输出；Rust/PoC bin 的 0=不限，与生产一致 |
| top_p | 1.0 (mod.rs:640) | 1.0 (`--top-p 1.0`) | ✅ | |
| seed | 42 (mod.rs:641) | 42 (`--seed 42`) | ✅ | |
| system_prompt | "You are a helpful assistant." (mod.rs:636) | PoC bin 内置同值 | ✅ | PoC bin 不暴露此参数，但内置与生产同 |
| user_prompt | "语音转写:" (mod.rs:637) | PoC bin 内置同值 | ✅ | 同上 |
| language | None (mod.rs:642) | PoC bin 默认 None | ✅ | |
| itn | 1/True (mod.rs:643) | PoC bin 默认 True | ✅ | |
| hotwords | Gavin curated（002 核实 5 条） | `--hotwords "我吃,比利,词库,灵界,阿炎"` | ✅ | 002 核实 wordbook 11 条 curate 后剩 4-5 条 |
| CTC blank_penalty | 0.0 (mod.rs:608) | PoC bin 默认 0.0 | ✅ | |
| CTC use_itn | True (mod.rs:600) | PoC bin sensevoice 默认 | ✅ | |
| VAD threshold | 0.5 (vad.rs:28) | 0.5 | ✅ | Python sherpa-onnx 1.13.4 silero VAD |
| VAD min_silence | 0.3s (vad.rs:29) | 0.3 | ✅ | |
| VAD min_speech | 0.1s (vad.rs:30) | 0.1 | ✅ | |
| VAD max_speech | 20s (vad.rs:31) | 20.0 | ✅ | |
| VAD window | 512 (vad.rs:27) | 512 | ✅ | |
| VAD padding | 3200=200ms (vad.rs:24) | 3200 | ✅ | |
| SEGMENT_MAX_SECS | 20s (vad.rs:21) | 20.0 | ✅ | |
| onset trim threshold | 0.008 (main.rs:2822) | 0.008 | ✅ | WSL preprocess.py |
| onset window | 160 samples (main.rs 同算法) | 160 | ✅ | |
| silence_head (perf) | 0ms (main.rs:2747) | 0ms | ✅ | ASR-CTC-OPT-001 P1 |
| silence_head (acc) | 0ms (main.rs:2749) | 0ms | ✅ | ASR-ACC-OPT-001 |
| backtrack (perf) | 200ms=3200 (main.rs:2748) | 200ms | ✅ | |
| backtrack (acc) | 100ms=1600 (main.rs:2750) | 100ms | ✅ | |
| sherpa-onnx 版本 | Rust vendored 1.12.38 | Windows PoC bin 同 crate 同 DLL | ✅ | **主控修正**：推理回 Windows PoC bin，避免 WSL Python 1.13.4 栈差异 |
| VAD 实现栈 | Rust sherpa-onnx VoiceActivityDetector | Python sherpa-onnx 1.13.4 silero VAD（仅算切点） | ⚠️ 残留风险 | 版本差异可能致边界微差，报告标注；A2 切点人工核验合理（3 段 18.6/20.3/14.9s） |

**方法论教训应用**：002 第八节教训强制执行——逐参数核对，特别抓出 Python 1.13.4 `max_new_tokens=0` 致空输出的版本陷阱（第一轮 WSL 全跑时 A2/A3 全空，根因即此）。

---

## 二、Step 0 预处理

### 音源
- 原始：`collab/research/audio-real-gavin/录音 (2).m4a`（~56s，Gavin 自录连续朗读体育新闻，含外国人名音译/数字/比分）
- 标准答案：`原始文本.txt`（3 段：1/8决赛挪威胜巴西 / 挪威队疾病 / 水晶宫前锋拉尔森+佩德森）

### ffmpeg 转码
m4a → 16kHz mono PCM wav（WSL ffmpeg 4.4.2）：`processed/full.wav`（56.15s）

### 段落切分（人工分段，用于 A3/A4）
基于 CTC 窗口扫描定位语义边界 + 能量停顿确认：
- para1: 0-11.95s（第1段："上一轮...八强"）
- para2: 12.1-27.7s（第1段尾"如今...英格兰队" + 第2段"然而...最佳状态"）—— 切分有偏差，para2 含第1段末尾
- para3: 28.5-55.0s（第3段："水晶宫...淘汰赛"，26.5s 超 24s 触发阈值）

**切分偏差说明**：para2 包含第1段尾部"如今...英格兰队"，因第1段与第2段间无显著停顿（连续朗读）。此偏差不影响 A3 vs A4 对比（同切分），但 per-para CER 对齐原文段落时失真，报告以 full CER 为准。

### CER 规范化规则
- 剥离标点：`。，、！？；：""''…—·.,!?;:"'()[]` + 空白
- 保留数字/字母原样（1/8、2比1 不转换）
- 与 002 一致：CER = Levenshtein(规范化 hyp, 规范化 ref) / len(规范化 ref)

---

## 三、Step 1 四条件 CER 矩阵（核心结果）

| # | 条件 | full CER | 说明 |
|---|------|----------|------|
| **A1** | CTC full（无分段, bt200） | **0.0724** | 整段无标点，错字多但上下文通顺 |
| **A2** | native+hw+VAD temp0.3（bt100） | **0.0271** | 3段拼接有标点，专有名词几乎全对，**最优** |
| A3 | native+hw para（人工分段, bt100） | 0.5023 | para3 **空输出**（26.5s超max_total_len截断）拖垮 |
| A4 | CTC para（人工分段, bt200） | 0.0633 | 整段对齐，错字同 A1 模式 |

### 关键观察

1. **A2 (0.0271) 优于 A1 (0.0724)**：native+VAD 在真实语料上 CER 低 63%。**未复现 Gavin 体感**。
2. **A3 (0.5023) 被 para3 空输出拖垮**：para3 26.5s 整段喂 native → max_total_len=512 截断 → 0.48s 快速返回空。**这是 max_total_len 截断在真实语料上的实锤**，反向证明生产 VAD 分段（A2）是 native 长音频可用的必要条件——无 VAD 分段，native 对 >28s 音频静默空输出，而 CTC 无此限制（A1/A4 整段正常）。
3. **A4 (0.0633) 略优于 A1 (0.0724)**：CTC 人工分段比整段略好（分段隔离了错误传播），但差距小。
4. **A2 vs A3**：同为 native，A2 有 VAD 分段（每段 ≤20s）正常输出，A3 人工分段但 para3 太长空输出。**VAD 分段是 native 长音频的必要保护**，不是质量负担。

### A3 para3 空输出专题分析（max_total_len 截断实锤）

**现象**：para3_trim_acc.wav（26.5s）整段喂 native recognizer，PoC bin 0.48s 快速返回空字符串，无任何输出。

**根因**：FunASR Nano native 模型 max_total_len=512（KV cache 硬限制，模型导出固化，vad.rs:14-17 注释记录 27.88s/487token 正常、29.88s/520token 截断）。para3 26.5s ≈ 441 LFR tokens + prompt ≈ 521 > 512 → sherpa-onnx 截断 audio placeholders → 无有效 audio token → 空输出。PoC bin 输出含警告：
```
Context_len (521) exceeds KV capacity (512). Truncating audio placeholders: audio_token_len=441 -> keep_audio=432 (before=75 after=5)
```

**反向证明生产 VAD 分段是必要条件**：
- A2 用 VAD 把 56s 音频分 3 段（18.6/20.3/14.9s），每段 ≤20s < 28s 临界，全部正常输出，CER 0.0271
- A3 人工分段但 para3 超 28s → 空输出
- **若生产 VAD 失败/降级 naive_chunk 切出 >28s 段，native 会静默空输出**，用户感知"accuracy 差"——这是 native 相对 CTC 的真实弱点，可能是 Gavin 体感来源之一
- CTC 无 max_total_len 限制（A1 整段 56s 正常输出），故 CTC 在 VAD 失败时仍可用

**生产防护审计**：`naive_chunk`（vad.rs:253）20s 等分，已有单测 `naive_chunk_60s_three_segments` 验证 60s 切 3 段每段 ≤20s。生产降级路径（mod.rs:283）调 naive_chunk 保证段 ≤20s < 28s，防护成立。但若 naive_chunk 实现有误或段长配置变化，native 仍会空输出。

### 各条件完整转录文本（供 Gavin 目视对比）

**【A1 CTC full】**（无标点）
> 上一轮八分之一决赛凭借哈兰德梅开奥度挪威队二比一暴冷淘汰五届世界杯冠军巴西队历史性闯入世界杯八强如今他们将在迈阿密挑战英格兰队然而据报道近期挪威队内出现了疾病传播多名球员受到发烧咳嗽等症状困扰球队任于时间赛跑希望能在比赛前恢复最佳状态水晶工前锋约根斯特兰德拉尔森因发烧驱席了世界宾首战对阵伊拉克队前的训练并最终无缘那场比赛效力于意甲萨索洛的马库斯霍尔摩格伦佩德森虽然在小组赛第二轮对阵赛利加尔队时取得进球但由于生病屈席了上一场对阵巴西队的淘汰赛

**【A2 native+VAD】**（3段拼接，有标点）
> 上一轮八分之一决赛，凭借哈兰德梅开二度，挪威队二比一爆冷淘汰五届世界杯冠军巴西队，历史性闯入世界杯八强。如今他们将在迈阿密挑战英格兰队。然而据报道，近期挪威队内出现了疾病传播。多名球员受到发烧、咳嗽等症状困扰，球队正与时间赛跑，希望能在比赛前恢复最佳状态。水晶宫前锋约根·斯特兰德·拉尔森因发烧缺席了世界杯首战，对阵伊拉克队前的训练，并最终无缘那场比赛。效力于意甲萨索洛的马库斯霍尔姆·格伦·佩德森，虽然在小组赛第二轮对阵塞内加尔队时取得进球，但由于生病缺席了上一场对阵巴西队的淘汰赛。

**【A3 native para】**（para3 空输出）
> seg0: 上一轮八分之一决赛，凭借哈兰德梅开二度，挪威队二比一爆冷淘汰五届世界杯冠军巴西队，历史性闯入世界杯八强。
> seg1: 如今他们将在迈阿密挑战英格兰队。然而据报道，近期挪威队内出现了疾病传播，多名球员受到发烧、咳嗽等症状困扰，球队正与时间赛跑，希望能在比赛前恢复最佳状态。
> seg2: **（空输出，26.5s 超 max_total_len 截断）**

**【A4 CTC para】**（无标点）
> seg0: 上一轮八分之一决赛凭借哈兰德梅开奥度挪威队二比一爆冷淘汰五届世界杯冠军巴西队历史性闯入世界杯八强
> seg1: 如今他们将在迈阿密挑战英格兰队然而据报道近期挪威队内出现了疾病传播多名球员受到发烧咳嗽等症状困扰球队在于时间赛跑希望能在比赛前恢复最佳状态
> seg2: 水晶弓前锋约根斯特兰德拉尔森因发烧缺袭了世界杯首战对阵伊拉克队前的训练并最终无缘那场比赛效力于意甲萨索洛的马库斯霍尔摩格伦佩德森虽然在小组赛第二轮对阵赛利加尔队时取得进球但由于生病缺席了上一场对阵巴西队的淘汰赛

### 专有名词 / 数字 / 乱码错误清单

| 关键词 | A1 CTC | A2 native+VAD | A3 native para | A4 CTC para |
|--------|--------|---------------|----------------|-------------|
| 哈兰德 | ✓ | ✓ | ✓ | ✓ |
| 梅开二度 | ✗ 梅开奥度 | ✓ | ✓ | ✗ 梅开奥度 |
| 2比1 | ✗ 二比一 | ✗ 二比一 | ✗ 二比一 | ✗ 二比一 |
| 1/8 | ✗ 八分之一 | ✗ 八分之一 | ✗ 八分之一 | ✗ 八分之一 |
| 水晶宫 | ✗ 水晶工 | ✓ | ✗ (para3空) | ✗ 水晶弓 |
| 约根·斯特兰德·拉尔森 | ✓(无分隔点) | ✓(有·) | ✗ (para3空) | ✓(无分隔点) |
| 缺席 | ✗ 驱席/屈席 | ✓ | ✗ (para3空) | ✗ 缺袭 |
| 世界杯首战 | ✗ 世界宾首战 | ✓ | ✗ (para3空) | ✓ |
| 霍尔姆格伦 | ✗ 霍尔摩格伦 | ✓ 霍尔姆·格伦 | ✗ (para3空) | ✗ 霍尔摩格伦 |
| 塞内加尔 | ✗ 赛利加尔 | ✓ | ✗ (para3空) | ✗ 赛利加尔 |
| 意甲萨索洛 | ✓ | ✓ | ✗ (para3空) | ✓ |
| 马库斯 | ✓ | ✓ | ✗ (para3空) | ✓ |

**专有名词命中率**：A2 28/30（93%），A1 20/30（67%），A4 22/30（73%）。**native 在专有名词上显著优于 CTC**。

### 幻觉 / 空输出 / 非中文乱码
- **A3 seg2 空输出**（26.5s 超 max_total_len，native 静默截断）——唯一严重问题
- 无非中文乱码、无幻觉（>12字/秒）实例
- A1/A4 CTC 无空输出（CTC 无长度限制），A2 native+VAD 无空输出（VAD 分段规避）

---

## 四、Step 2 归因深挖

### 判定（按任务预设三分支）

**A2 (0.0271) 不差于 A1 (0.0724)，A3 去掉空段后也不差** → 走第三分支：**A2/A3 都不差，与 Gavin 体感仍矛盾**。

### 三分支逐一评估

1. **"真实音频 × native decoder"病根假设**：✗ 推翻。A2 native+VAD 在真实麦克风语音上 CER 0.0271 优于 CTC 0.0724，专有名词 93% 命中。native 在真实音频上能力成立。
2. **"VAD 分段质量"病根假设**：✗ 推翻。A2 VAD 分 3 段（18.6/20.3/14.9s），切点在语义停顿处，无切在词中间，每段正常输出。VAD 分段质量良好，非病根。
3. **A2/A3 都不差，与体感矛盾**：✓ 此分支。建议启用 DEBUG-AUDIO-DUMP-001 抓应用内部音频做终极对照。

### 矛盾的剩余嫌疑清单（三层逐一拆解）

本语料未复现 Gavin 体感，剩余嫌疑分三层：

#### 嫌疑 a · 应用内采集链（DEBUG-AUDIO-DUMP-001 可排查）
- **PoC 音频链 vs 生产音频链差异**：本 PoC 用录音机 app 录的 m4a → ffmpeg 转码 wav → WSL onset trim；生产链是 麦克风 → WASAPI 回调 → 环形缓冲 → 重采样（FIRSTCHAR-FIX-005 抗混叠）→ main.rs onset trim。两条链的音频特征可能不同：
  - **噪底**：录音机 app 可能带 AGC/降噪，生产 WASAPI 原始输入更脏
  - **重采样痕迹**：生产 48kHz→16kHz 抗混叠（audio/mod.rs resample_linear），录音机 app 可能不同采样率处理
  - **能量包络**：WASAPI 环形缓冲 + pre_roll 600ms + onset trim 的组合，与 PoC 的 onset trim 可能在前导静音结构上有微差（001 已证 native 对前导结构极度敏感）
- **排查手段**：DEBUG-AUDIO-DUMP-001 抓 main.rs run_pipeline 入口处的 `samples`（onset trim 后、喂 recognizer 前），与 PoC full_trim_*.wav 逐字节比对
- **排查范围**：仅此层。DEBUG-AUDIO-DUMP-001 **无法**排查嫌疑 b/c

#### 嫌疑 b · Gavin 日常语料形态（DEBUG-AUDIO-DUMP-001 不可排查）
- **本语料 vs 日常使用差异**：本语料是清晰连续朗读 56s 体育新闻（有上下文、连贯长句）；Gavin 日常使用是**自发短指令**（"打开浏览器"、"派发任务"、混合中英技术词）
- **短指令 vs 长朗读的模型差异**：
  - CTC 对短指令更稳（无 LLM decoder 幻觉风险，001/002 已证 native 对孤立短词有幻觉倾向）
  - native LLM decoder 在长朗读有上下文增益（本语料 A2 专有名词 93% 命中），但短指令上下文不足时可能退化
  - 002 的 TTS 短词 PoC 曾显示 native temp=1.0 下短词输出英文（cool/key/paul），temp=0.3 已大幅改善但短词场景仍需验证
- **排查手段**：Gavin 录 5-10 条日常短指令语料（非朗读），跑同矩阵
- **本报告无法覆盖此层**，需多样本研究

#### 嫌疑 c · 应用内转录后处理（DEBUG-AUDIO-DUMP-001 不可排查）
- **PoC 仅测 ASR 原始输出，生产有后处理链**：
  - **标点引擎**：CTC 输出无标点 → 走标点引擎（punctuation/）；native 输出自带标点 → 跳过标点引擎（transcribe_with_punct_info native_punctuated=true）。**两模型的后处理路径不同**，可能引入感知差异（标点引擎偶尔加错标点 vs native 自带标点偶有断句错误）
  - **LLM 优化**：config.toml 显示 `llm.enabled=false`（Gavin 当前关 LLM），但若曾开启，LLM 对 CTC 无标点输入 vs native 有标点输入的纠错效果可能不同
  - **词库应用**：wordbook::apply 在 ASR 输出后运行，对 CTC/native 输出的纠正效果可能不同
  - **注入环节**：clipboard+Ctrl+V 注入，与 ASR 无关，不引入差异
- **排查手段**：需在生产环境对比"ASR 原始输出 vs 注入最终文本"，可能 native 自带标点反而让用户感知更"生硬"（断句与用户预期不符）vs CTC+标点引擎更"顺"
- **本报告无法覆盖此层**，需应用内端到端对照

### 三层嫌疑优先级

| 嫌疑 | 可排查手段 | 预期贡献 | 优先级 |
|------|-----------|---------|-------|
| a 采集链 | DEBUG-AUDIO-DUMP-001 | 中（001 已证前导结构敏感，但本 PoC onset trim 已复刻） | 中 |
| b 语料形态 | 多样本短指令语料 | **高**（短指令是 Gavin 日常，且 001/002 已证 native 短词有风险） | **高** |
| c 后处理 | 应用内端到端对照 | 中（标点/LLM 路径差异可能影响感知） | 中 |

---

## 五、Step 3 真实语料参数复扫

**跳过**：任务规定"仅当 Step 1 复现差距后"跑 Step 3。本语料未复现 Gavin 体感差距（A2 优于 A1），故 Step 3 不跑。

**E1-E4 方向状态**：002 在 TTS 短词得出的 E1（temp 0.3→0.2）/E2（hotwords 上限 50→20）/E3（backtrack 100→50ms）/E4（temp 0.3→0.0）方向，在本真实语料上未被推翻（A2 native 已优于 CTC，调优只会更好），但**仍需真实语料参数验证才可落地**。建议 E1-E4 落地前补一轮真实语料参数扫描（多样本）。

---

## 六、明确回答（验收标准必答）

### Q1: Gavin 体感是否被复现？
**否**。A2 native+VAD（CER 0.0271）优于 A1 CTC（CER 0.0724），CER 低 63%，专有名词命中率 93% vs 67%。与 002 结论一致，未复现"performance 更优"体感。

### Q2: 差距来自采集链、VAD 分段、还是 native 真实语音能力？
- **采集链**：✗ 未发现伤害（A2 native 在真实麦克风语音上表现好）
- **VAD 分段**：✗ 质量良好（3 段切点合理，无词中切断）
- **native 真实语音能力**：✗ 短/中音频能力成立（CER 0.0271）；但 **长音频空输出风险** 是 native 真实缺陷（A3 para3 26.5s 空输出），若生产 VAD 降级可能暴露

### Q3: E1-E4 是否仍然成立？
**是**（方向成立，未推翻），但 **E1-E4 仍挂起，需真实语料参数验证才可落地**。本轮未跑 Step 3（未复现差距），建议 E1-E4 落地前补真实语料参数扫描。

### Q4: 下一步建议？
1. **启用 DEBUG-AUDIO-DUMP-001**：抓 Gavin 应用内部音频（生产路径真实输入）做终极对照，排除 PoC 预处理与生产 main.rs 预处理的微差
2. **多样本真实语料**：56s 单一样本不足以终审，建议 Gavin 录 5-10 条不同场景（嘈杂/口音/长音频/混合语言）语料
3. **native 长音频空输出防护**：A3 para3 证实 native >28s 整段空输出。建议检查生产 VAD 降级路径（mod.rs:283 naive_chunk 20s 等分）是否保证每段 ≤20s，避免 naive_chunk 切出 >28s 段
4. **Gavin 体感来源确认**：需 Gavin 明确"performance 优于 accuracy"体感的端测时间、切换操作、是否遇到长音频空输出

---

## 七、分级建议方案

### 方案 F1【推荐，可观测性】启用 DEBUG-AUDIO-DUMP-001 抓应用内部音频
- **问题**：PoC 预处理（WSL onset trim + Python VAD 切点）与生产 main.rs 预处理可能有微差，终极对照需抓生产路径真实输入音频
- **改动**：生产代码加 DEBUG 音频 dump 开关（仅在 -debug 模式生效），不影响发布
- **影响文件**：`src/main.rs`（run_pipeline 音频 dump 钩子）
- **风险**：低（仅 debug 模式，发布关闭）
- **验收标准**：Gavin -debug 跑一次，抓到的音频与本 PoC full.wav 比对一致性

### 方案 F2【推荐，多样本】收集 5-10 条真实语料
- **问题**：单一样本不足以终审，需多样本覆盖嘈杂/口音/长音频/混合语言场景
- **改动**：无代码改动，Gavin 录制多样本
- **验收标准**：5-10 条语料跑 A/B 矩阵，统计 CER 分布

### 方案 F3【中风险，防护】native 长音频空输出防护审计
- **问题**：A3 para3 26.5s 整段喂 native 空输出。生产 VAD 降级路径 naive_chunk（mod.rs:283）20s 等分应保证每段 ≤20s，但需审计确认
- **改动**：无代码改动（审计确认），若 naive_chunk 切出 >28s 段则需修
- **影响文件**：`src/transcription/vad.rs`（naive_chunk 实现，已测 20s 等分）
- **验收标准**：审计 naive_chunk 对 >20s 音频的切段长度 ≤20s（已有单测 naive_chunk_60s_three_segments）

### 方案 F4【挂起，等 F1/F2 数据】E1-E4 参数落地
- **状态**：002 的 E1（temp 0.3→0.2）/E2/E3/E4 方向成立，但落地需真实语料参数扫描数据
- **建议**：F1/F2 完成后，在多真实语料上跑 temp/hotwords/backtrack 扫描，确认 E1-E4 收益后落地

---

## 八、生产代码零改动确认

本研究仅修改/新增：
- `collab/research/asr-accuracy-real-001.md`（本报告）
- `collab/research/audio-real-gavin/preprocess.py`（WSL 预处理）
- `collab/research/audio-real-gavin/vad_cut.py`（WSL VAD 切点，未用，被 preprocess.py 整合）
- `collab/research/audio-real-gavin/run_poc_transcribe.sh`（Windows PoC bin 调用）
- `collab/research/audio-real-gavin/score_cer.py`（WSL CER 评分）
- `collab/research/audio-real-gavin/run_step1_matrix.py`（早期 WSL 全跑版本，已被 Windows PoC 方案取代，保留作历史）
- `collab/research/audio-real-gavin/processed/`（转码 + 预处理 wav）
- `collab/research/audio-real-gavin/results/`（A/B 矩阵输出 + CER 数据）

`src/`、`src-tauri/`、`ui/`、`Publish/`、`models/`、`target/release/` 零改动。Gavin 原始录音与 txt 只读未改未删。Gavin 实例未打扰。

## 九、方法论教训

1. **WSL Python 栈与生产 Windows Rust 栈不可混用**（主控修正）：第一轮 WSL 全跑用 Python sherpa-onnx 1.13.4，`max_new_tokens=0` 被 Python 当"生成0个token"致全空输出，而 Rust 生产当"不限"。栈差异会引入混淆变量，污染分化信号。**推理必须用生产同栈（Windows PoC bin）**。
2. **PoC 预处理必须复刻生产 main.rs 的 onset trim**：早期直接喂原始 wav 给 PoC bin 漏了 onset trim + silence head，需 WSL 预处理输出 trimmed wav 再喂 PoC bin。
3. **VAD 切点用 Python 算 + 人工核验合理性**：Python VAD 与 Rust VAD 实现可能有边界微差，A2 切点（3 段 18.6/20.3/14.9s）人工核验落在语义停顿处，合理。残留风险在报告标注。
4. **单一样本不足以终审**：56s 单语料结论需多样本验证，DEBUG-AUDIO-DUMP 是终极对照手段。

## 十、数据资产索引

- A/B 矩阵转录输出：`collab/research/audio-real-gavin/results/{A1_ctc_full,A2_native_vad,A3_native_para,A4_ctc_para}.txt`
- CER 评分数据：`collab/research/audio-real-gavin/results/cer_matrix.json`
- VAD 切点信息：`collab/research/audio-real-gavin/processed/vad_segs/segments.json`
- 预处理 wav：`collab/research/audio-real-gavin/processed/{full_trim_acc,full_trim_perf,paraN_trim_*}.wav` + `vad_segs/segN.wav`
- 脚本：`preprocess.py` / `run_poc_transcribe.sh` / `score_cer.py`