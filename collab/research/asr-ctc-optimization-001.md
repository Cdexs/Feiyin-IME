# RESEARCH-ASR-CTC-OPT-001 · performance（CTC）模型优化空间研究

> 任务编号：RESEARCH-ASR-CTC-OPT-001
> 负责人：coder-1
> 完成时间：2026-07-07
> 任务性质：纯研究（生产代码零改动）
> 数据资产：`collab/research/audio-002B/ctc_study/`

---

## 一、研究背景

CTC（performance，179MB FunASR Nano CTC int8，2025-12-17 导出）是默认模型 + accuracy 的 fallback 垫底。基线（RESEARCH-ASR-ACCURACY-001）：raw TTS 75% / post-trim 70% first%。每 1pp 提升全体用户受益。

## 二、各方向验证结论

### C1 · 前处理 silence head 0ms vs 50ms【可落地，+2.5pp】

**实验**：post-trim 模拟（onset trim 200ms backtrack + head 变量），CTC 模型，40 wav × 3 head 值。

| head_ms | first% | word% | delta_vs_50 |
|---------|--------|-------|-------------|
| 0       | 72.5   | 70.0  | +2.5        |
| 10      | 70.0   | 65.0  | +0.0        |
| 50      | 70.0   | 67.5  | 0（基线）    |

**结论**：
- **0ms head 比 50ms 高 2.5pp（72.5% vs 70%）**——证实 50ms 是旧 SenseVoice（237MB，FIRSTCHAR-FIX-006 2026-05-27 调优）遗产，对 FunASR Nano CTC（2025-12-17 新模型）非最优
- 10ms 与 50ms 持平——CTC 对 head 不如 native 敏感（对比 native 50ms 掉 10pp），小幅 head 不影响
- 注释 L2767-2768 原文"SenseVoice is an offline model that does not require long leading silence for frame alignment; 50ms is just a minimal acoustic padding"——frame alignment 论据对 CTC 不成立，0ms 纯收益

**数据**：`ctc_study/c1_silence_head.json`

### C2 · blank_penalty 扫描【不值得做，无影响】

**实验**：CTC 模型，raw TTS wav，blank_penalty 0/0.25/0.5/0.75/1.0 五档。

| bp   | raw_first% | raw_word% |
|------|-----------|-----------|
| 0.00 | 75.0      | 70.0      |
| 0.25 | 75.0      | 70.0      |
| 0.50 | 75.0      | 70.0      |
| 0.75 | 75.0      | 70.0      |
| 1.00 | 75.0      | 70.0      |

**结论**：**blank_penalty 对 FunASR Nano CTC 完全无影响**（五档全 75%/70%）。0.5 是 SenseVoice 时代遗产值（A-001 直换时保留"PoC 对照无副作用"），对新 CTC 无作用。保留无害但可设 0。**不值得做参数扫描**——CTC greedy 解码无概率调整空间。

**数据**：`ctc_study/c2_blank_penalty.json`

### C3 · CTC hotwords/biasing 支持【不支持，确认旧结论】

**上游调研**：c-api.h `SherpaOnnxOfflineSenseVoiceModelConfig` 仅 model/language/use_itn 三字段，**无 hotwords 字段**。PR #3122 "Add hotword support for FunASR-Nano" 只给 `OfflineFunASRNanoModelConfig`（native）加了 hotwords，CTC 用 `OfflineSenseVoiceModelConfig` 不支持。

**结论**：**CTC 不支持 hotwords**，词库纠偏能力仍是 accuracy 独占。troubleshooting [FIRSTCHAR-002] 旧结论"hotwords 仅 transducer 支持"修正为"hotwords 仅 FunASR Nano native 支持，CTC/SenseVoice 均不支持"。

### C4 · ITN rule-fsts【可落地，需下载资产，体验收益】

**上游调研**：
- `OfflineRecognizerConfig` 有顶层 `rule_fsts` 字段（punctuation/text-processing），独立于 `sense_voice.use_itn`
- 生产 `create_sensevoice_recognizer` 设 `use_itn: true` 但**未设 `rule_fsts`**——ITN 规整化需要 fst 文件，光开 use_itn 不够
- PR #3122 评论确认：ITN 默认不生效，需 `--rule-fsts=./itn_zh_number.fst`
- `itn_zh_number.fst` 需从 sherpa-onnx releases 单独下载（不在模型包内）

**结论**：**ITN 是可落地方案**，数字规整化（"一百二十三"→"123"、"百分之二十五"→"25%"、"十二月二十五日"→"12月25日"）对输入法体验是直接收益。需下载 itn_zh_number.fst + 改 create_sensevoice_recognizer 设置 rule_fsts。本研究未下载资产实测（纯研究边界：下载资产需另立项），但上游 PR 评论有用户实证。

### C5 · 错误样本分类分析【同音字是天花板，70% 错误】

**实验**：CTC raw TTS 输出（75% first%，10 个错误），错误分类。

| 错误类型 | 数量 | 占比 | 示例 |
|---------|------|------|------|
| homophone_single（同音字）| 7 | 70% | 厂→唱、开→嗨、口→扣、气→系、踢→t |
| aspirated_confusion（送气混淆）| 2 | 20% | 跑→泡 |
| extra_chars（多字）| 1 | 10% | 口→เข |

**结论**：
- **70% 错误是同音字**——CTC 无语言模型上下文，同音字是固有盲区（厂/唱、口/扣、气/系 都是同音不同调或近音字）
- **20% 送气声母混淆**（跑/泡）——送气/不送气声母区分，FIRSTCHAR 系列已部分改善
- **踢→t** 两次——TTS 的"踢"被识别为英文 t，可能是 TTS 音色或模型中英混淆
- **同音字错误是 CTC 天花板**——无 LM rescoring 无法解决，只能靠词库后处理（wordbook.apply 已有）或换 LM 模型（accuracy native）

**数据**：`ctc_study/c5_error_analysis.json`

### C6 · 解码方法【不支持，CTC offline 仅 greedy】

**上游调研**：
- `SherpaOnnxOnlineCtcFstDecoderConfig`（graph/max_active）是 **online（streaming）** 用的
- `decoding_method`（greedy_search/modified_beam_search）也是 online recognizer
- **offline CTC 只有 greedy 解码**，无 beam/LM rescoring 选项

**结论**：**offline CTC 不支持 beam/LM rescoring**，无法通过解码方法提升。同音字错误（C5 主因）只能靠后处理或换模型解决。

### C7 · 模型资产更新【无更新版本】

**上游调研**：
- sherpa-onnx 文档 SenseVoice 页面仅列 2024-07-17 和 2025-09-09 两版（旧 SenseVoice，非 FunASR Nano CTC）
- FunASR Nano 官方页面仅 `sherpa-onnx-funasr-nano-int8-2025-12-30`（native，972MB，accuracy 用的）
- 当前 CTC（179MB，2025-12-17）不在官方 FunASR Nano 页面，来源 Wasser1462/FunASR-nano-onnx 导出
- **无更新 CTC 版本可替代**，int8 是唯一版本（无 fp16/fp32）

**结论**：**无新模型资产可用**，int8 量化是当前唯一选项。

---

## 三、优化方案（分级）

### 方案 P1【强烈推荐，低风险，+2.5pp】CTC silence head 50→0ms

**问题**：生产 50ms silence head 是旧 SenseVoice 遗产，对 FunASR Nano CTC 非最优（C1 证实 0ms 高 2.5pp）。

**改动**：
- 影响文件：`src/main.rs`（`select_preprocessing_params` 函数，ASR-ACC-OPT-001 已提取的纯函数）
- 具体改动：`Performance` 分支 `PERF_SILENCE_HEAD_SAMPLES` 从 800（50ms）改为 0（0ms），backtrack 保持 3200（200ms）
- **注意**：ASR-ACC-OPT-001 已把 performance 分支设为字面零改动红线。本方案需解除该红线——但 C1 数据证实 0ms 对 CTC 更优，且有 ASR-ACC-OPT-001 的 accuracy 分支已用 0ms 验证过 LLM decoder 不需 padding。CTC 同样是 offline 模型，0ms 安全
- 风险评估：低——onset trim 200ms backtrack 已保留送气声母，0ms head 只是去掉无用的前导静音；现有测试 `asr_silence_head_is_50ms_at_16khz`（L3452）需更新为 0ms
- 预期收益：+2.5pp first%（70%→72.5%）
- 验收标准：
  - cargo test 全绿（更新 silence head 相关测试）
  - 002B post-trim wav CTC 0ms first% ≥ 72.5%（C1 基线）
  - Gavin 端测短词首字体感

### 方案 P2【推荐，中风险，体验收益】ITN rule-fsts 启用

**问题**：生产 `use_itn: true` 但未设 `rule_fsts`，ITN 数字规整化不生效（C4 确认）。

**改动**：
- 影响文件：`src/transcription/mod.rs`（`create_sensevoice_recognizer`）+ `models/` 新增 itn_zh_number.fst 资产
- 具体改动：
  1. 下载 `itn_zh_number.fst`（sherpa-onnx releases）至 `models/itn/itn_zh_number.fst`
  2. `OfflineRecognizerConfig.rule_fsts` 设为该路径
- 风险评估：中——需下载资产 + 测试数字规整化效果；fst 文件可能影响非数字文本（需回归测试）
- 预期收益：数字输出规整化（"一百二十三"→"123"），输入法体验直接收益，不可量化但高价值
- 验收标准：
  - 数字类样本 PoC 实测：ITN 开 vs 关，数字规整化生效
  - 非数字文本无回归（002B wav first%/word% 不降）
  - cargo test 全绿

### 方案 P3【可选，低风险】blank_penalty 0.5→0

**问题**：blank_penalty 0.5 对 FunASR Nano CTC 无影响（C2 证实五档全 75%），是 SenseVoice 遗产值。

**改动**：
- 影响文件：`src/transcription/mod.rs`（`create_sensevoice_recognizer`）
- 具体改动：`blank_penalty: 0.5` → `blank_penalty: 0.0`（或删除该行用默认 0）
- 风险评估：低——C2 证实无影响，改动无害
- 预期收益：0（无性能变化，仅清理遗产值）
- 验收标准：cargo test 全绿 + 002B wav first%/word% 不变

### 不推荐方向

- **C3 CTC hotwords**：不支持，c-api 无字段，需上游改 sherpa-onnx
- **C6 解码方法**：offline CTC 仅 greedy，无 beam/LM rescoring
- **C7 模型更新**：无新版本资产
- **同音字错误（C5 主因）**：CTC 天花板，只能靠词库后处理（已有 wordbook.apply）或换 accuracy native 模型

---

## 四、核心结论

1. **CTC 最大优化空间是 silence head 50→0ms（方案 P1，+2.5pp）**——50ms 是旧 SenseVoice 遗产，FIRSTCHAR-FIX-006 是 2026-05-27 对旧模型调的，新 FunASR Nano CTC 用 0ms 更优
2. **blank_penalty 无影响（C2）**——0.5 是遗产值，可清理但无收益
3. **ITN 是体验收益（方案 P2）**——数字规整化对输入法有价值，需下载资产
4. **同音字错误是 CTC 天花板（C5，70% 错误）**——无 LM rescoring 无法解决，CTC 固有盲区
5. **CTC 不支持 hotwords（C3）**——词库纠偏仍依赖 wordbook.apply 后处理 + accuracy native

## 五、推荐路线

**优先级**：方案 P1（+2.5pp，低风险）> 方案 P2（ITN 体验，中风险）> 方案 P3（清理遗产值，零收益）

**战略判断**：CTC 优化空间有限（+2.5pp 是天花板），同音字错误无法在 CTC 框架内解决。若 Gavin 期望更大提升，需考虑：
- 启用 accuracy native + 优化后的方案 A+B（RESEARCH-ASR-ACCURACY-001 已落地，native+hw 77.5%）
- 或等 sherpa-onnx 上游为 CTC 加 hotwords 支持

## 六、生产代码零改动确认

本研究仅修改/新增：
- `collab/research/audio-002B/ctc_study/`（新增实验输出）
- `collab/research/audio-002B/run_ctc_c1.py`、`run_ctc_c2.py`、`run_ctc_c5.py`（新增实验脚本）
- `collab/research/asr-ctc-optimization-001.md`（本报告）

`src/`、`src-tauri/`、`ui/`、`Publish/`、`models/` 零改动。

## 七、数据资产索引

- C1 silence head：`ctc_study/c1_silence_head.json` + `h0_*.wav`/`h10_*.wav`/`h50_*.wav`
- C2 blank_penalty：`ctc_study/c2_blank_penalty.json` + 10 组 raw txt
- C5 错误分类：`ctc_study/c5_error_analysis.json`
- 实验脚本：`run_ctc_c1.py`、`run_ctc_c2.py`、`run_ctc_c5.py`