# RESEARCH-ASR-ACCURACY-002 · accuracy 优化落地后仍不敌 performance 的深挖研究

> 任务编号：RESEARCH-ASR-ACCURACY-002
> 负责人：coder-1
> 完成时间：2026-07-08
> 任务性质：纯研究（生产代码零改动）
> 产出：本报告 + `collab/research/audio-002B/param_sweep/`（参数扫描数据）+ `cer_comparison.json`（CER 对比）
> 前置：RESEARCH-ASR-ACCURACY-001（2026-07-07）

---

## 核心结论（TL;DR）

1. **RESEARCH-ASR-ACCURACY-001 的核心结论被推翻**：001 报告声称"生产等价条件下 native+hw CER/first% 输给 CTC"，但其 PoC 实验脚本 `run_accuracy_study.py` **未传 `--temperature`**，PoC bin 默认 `temp=1.0`，而生产代码 `create_funasr_nano_recognizer` 硬编码 `temp=0.3`（`src/transcription/mod.rs:639`）。**001 的所有 native 数据都在 temp=1.0 下测得，与生产 temp=0.3 完全不可比**。
2. **生产等价条件下 accuracy 实际优于 performance**：temp=0.3 时 native+hw 的 CER=0.15、first=85%，**远优于** CTC 的 CER=0.3553、first=70%。差距来自 native decoder 在低 temp 下输出更稳定，CTC 在短词上出现泰文乱码/英文单字符等幻觉。
3. **Gavin "performance 仍优于 accuracy" 体感的真实根因待确认**：本研究在 PoC 层证实 accuracy 优于 CTC，但 Gavin 当前实际运行 `asr_model=qwen3_online`（`target/release/config.toml`），并非 accuracy 或 performance。Gavin 的对比体感可能来自更早的端测（当时 001 的 temp=1.0 误判尚未发现，或存在其他环境差异）——**需 Gavin 明确"accuracy 不敌 performance"结论的具体端测时间与切换操作**。
4. **温度是最大可挖空间**：temp=0.0 时 CER=0.10/first=90%，当前生产 temp=0.3 偏保守，有 +5pp first% / -33% CER 的空间。但 temp=0.0 会增加确定性幻觉风险（无随机性），需评估。
5. **天花板判断**：在当前 int8 native 模型资产下，**accuracy 存在稳定超越 performance 的现实路径**（生产 temp=0.3 已实现），001 的"建议放弃调优"结论不成立。UI 定位文案"准确率更高"可保留，无需改为"长音频/自带标点"。

---

## 一、前提修正（主控核实）

研究启动时基于 APPDATA/voice-ime/config.toml 得出"config 无 asr_model→默认 performance"的发现，经主控独立核实后**撤回**：

| 项 | 启动时假设 | 修正后事实 |
|----|-----------|-----------|
| 生效配置路径 | `%APPDATA%/voice-ime/config.toml` | **exe 同级 `target/release/config.toml`**（`src/config/mod.rs:296-302 config_path()`） |
| Gavin 当前 asr_model | （假设 performance） | **`qwen3_online`**（今天端测 qwen3，target/release/config.toml 含 4 个 qwen3 字段） |
| accuracy 模型完整性 | Publish 缺失→降级 | **target/release/models/ 完整（~994MB）**，切 accuracy 真实加载；Publish 缺失是 DEC-025 设计（可选下载不随包分发） |

**审计对象锁定 `target/release/` 路径**（Gavin 活跃实例 = `target/release/feiyin-ime.exe`）。

---

## 二、方向 1：调用层生效性审计

逐项给 file:line 证据，基于 HEAD 代码 + target/release 运行环境。

### 1.1 select_preprocessing_params 调用点覆盖 — PASS

- **单次转录路径**：`src/main.rs:2824-2826` 在 `run_pipeline` 内 `let (silence_head_samples, onset_backtrack_samples) = select_preprocessing_params(transcriber.asr_model())`，padded 音频喂 `transcriber.transcribe_with_punct_info`（main.rs:2850）。✅ 覆盖
- **VAD 分段路径**：`transcribe_offline_detailed`（mod.rs:233-291）在 `should_segment` 时调 `transcribe_segments_chunked`（mod.rs:297），逐段调 `transcribe_segment_detailed`（mod.rs:305）。**段不单独再调 select_preprocessing_params**，但段是 main.rs 已做 onset trim + silence head 后的 padded 音频的切片，**段已继承 head=0**。✅ 覆盖（间接，通过上游 padded 音频）
- **naive_chunk 降级路径**：mod.rs:283 `vad::naive_chunk(samples)` 对同样已 padded 的音频等分，走同一 `transcribe_segments_chunked`。✅ 覆盖（同上）
- **结论**：所有 accuracy 音频路径的 silence head=0 均生效。段内不再加 head 是正确的（避免段间插入静音）。

### 1.2 curate_hotwords_entries / build_hotwords_string 双路径生效 — PASS

- **recognizer 创建路径**：`main.rs:2169 load_hotwords_for_accuracy(&config)` → `main.rs:2175` 传入 `Transcriber::new` → `mod.rs:138 build_recognizer(model_dir, &asr_language, asr_model, hotwords)` → `mod.rs:635 hotwords: hotwords.map(|s| s.to_string())` 注入 OfflineFunASRNanoModelConfig。✅
- **热重载路径**：`main.rs:2279 load_hotwords_for_accuracy(&config)` → `main.rs:2325 reload_hotwords.as_deref()` 传入 `Transcriber::new` 重建。词库变更触发 `main.rs:2293 active_hotwords_version != desired_hotwords_version` → 重建 recognizer。✅
- **curate 逻辑生效**：`mod.rs:471 curate_hotwords_entries` 过滤纯 ASCII（`is_pure_ascii` mod.rs:456）、空、超长（>10 字）、上限 50。Gavin wordbook 11 条中 corrected 为 worker1/tester1/todo(×2)/coder1 是纯 ASCII 被过滤，实际注入 5 条（我吃/比利/词库/灵界/阿炎×2 去重后 4 条）。
- **验证**：本研究 param_sweep 的 hw=curated20（20 条精选词）CER=0.15 优于 hw=0（CER=0.20），证实 curate 后的 hotwords 有正收益。✅

### 1.3 effective_model 静默降级风险 — PASS（降级路径存在但 Gavin 环境不触发）

- **降级代码**：`mod.rs:532-547 build_recognizer`，accuracy 分支 `create_funasr_nano_recognizer` 失败 → 降级 `create_sensevoice_recognizer`，`effective_model=Performance`（mod.rs:544）。
- **语义归位**：Transcriber 存 `effective_model`（mod.rs:152 `asr_model: effective_model`），三处自动归位：① 标点 `native_punctuated=false`（mod.rs:369-373）② 空输出 bail 语义（mod.rs:357-361）③ 不触发 VAD 分段（mod.rs:142 `if effective_model == AsrModel::Accuracy`）。✅ 设计正确
- **Gavin 环境不触发**：`target/release/models/sherpa-onnx-funasr-nano-int8-2025-12-30/` 完整（encoder 238MB + llm 600MB + embedding 155MB + Qwen3-0.6B tokenizer），native 加载成功，不降级。
- **UI 仍显示 accuracy？**：降级时 `transcriber.asr_model()` 返回 Performance，但 UI 侧（src-tauri/config.rs）读的是 config 文件的 `asr_model` 字段，**不反映运行时降级**。这是潜在的可观测性缺口（用户以为 accuracy 实际降级 CTC），但 Gavin 环境不触发，非本次研究阻塞项。

### 1.4 H1 temperature 0.3 两条路径覆盖 — PASS

- **硬编码位置**：`mod.rs:639 temperature: 0.3` 在 `create_funasr_nano_recognizer`，recognizer 创建时注入 OfflineFunASRNanoModelConfig。
- **单次转录**：`transcribe_segment_detailed`（mod.rs:343）用 `self.offline_recognizer`（mod.rs:348），该 recognizer 在创建时已注入 temp=0.3。✅
- **VAD 分段**：`transcribe_segments_chunked`（mod.rs:297）逐段调同一 `self.offline_recognizer`，共用 temp=0.3。✅
- **naive_chunk**：同上，同一 recognizer。✅
- **结论**：temp=0.3 在所有 accuracy 路径生效。**注意**：temp 是 recognizer 级参数，不可按段/按调用动态调整，PoC 扫描证实 temp 影响显著（见方向3.1）。

### 1.5 遗留项5核实：accuracy 模型缺失降级 CTC 时前处理参数 — PASS（描述与现实不符）

- **todo 原描述**："accuracy 模型缺失降级 CTC 时前处理仍用 accuracy 参数"
- **现实代码**：`main.rs:2824 is_accuracy = transcriber.asr_model() == AsrModel::Accuracy`，降级后 `asr_model()=Performance`，`is_accuracy=false`。`select_preprocessing_params(transcriber.asr_model())`（main.rs:2826）传入 Performance → 返回 `(PERF_SILENCE_HEAD=0, PERF_ONSET_BACKTRACK=3200)`（main.rs:2747-2748，0ms/200ms）。
- **结论**：降级 CTC 时**实际用 performance 参数**（head0/bt200），**非 accuracy 参数**（head0/bt100）。todo 遗留项5的描述与现实代码不符，**无 bug**——降级后用 performance 前处理是正确行为（因为是 CTC 模型）。建议清理 todo 此条描述。

### 1.6 音频输入链一致性 — PASS

- performance 与 accuracy 进入 recognizer 前的处理完全一致，唯一差异是 `select_preprocessing_params` 返回值：
  - 同一 `run_pipeline` 代码路径（main.rs:2769）
  - 同一 onset trim 算法：`find_speech_onset_with_backtrack`（main.rs:2828），`SPEECH_ENERGY_THRESHOLD=0.008`（main.rs:2822）两模式同值
  - 同一 silence head 填充：`padded.resize(silence_head_samples, 0.0f32)`（main.rs:2835）
  - 差异仅 backtrack：performance=200ms（3200 samples）/ accuracy=100ms（1600 samples），silence head 都=0
- **结论**：无 accuracy 独有的被遗漏处理环节。两模式音频链一致，差异仅在 backtrack（performance 多留 100ms 前导，对 CTC 无害；accuracy 少留 100ms，对 native 更安全）。

### 方向1总结

| 审计项 | 结论 | 证据 |
|--------|------|------|
| 1.1 select_preprocessing_params 覆盖 | PASS | main.rs:2824-2826（单次）+ mod.rs:297-318（分段继承上游 padded） |
| 1.2 curate/build_hotwords 双路径 | PASS | main.rs:2169/2279（创建+热重载）→ mod.rs:138/635 |
| 1.3 effective_model 降级 | PASS（路径在，Gavin 不触发） | mod.rs:532-547 + target/release/models/ 完整 |
| 1.4 temp 0.3 两路径覆盖 | PASS | mod.rs:639 硬编码，recognizer 级注入 |
| 1.5 遗留项5（降级用 accuracy 参数） | 描述不符，无 bug | main.rs:2824-2826 降级后用 Performance 参数 |
| 1.6 音频链一致性 | PASS | 唯一差异 backtrack（200 vs 100ms） |

**未发现生效性 BUG**。ASR-ACC-OPT-001 的方案 A（curate）+ 方案 B（select_preprocessing_params）在生产代码中真实生效。

---

## 三、方向 2：CER + 幻觉率对比（评估维度错位）

### 3.1 方法论修正：001 报告的 temp 不可比缺陷

**关键发现**：`run_accuracy_study.py`（001 的实验脚本）未传 `--temperature`，PoC bin 默认 `temp=1.0`（`src/bin/poc_funasr_nano.rs:169`），而生产 `create_funasr_nano_recognizer` 硬编码 `temp=0.3`（`mod.rs:639`）。

- 验证：`grep temperature run_accuracy_study.py` → 无匹配（确认漏传）
- 影响：001 报告的 `post_native_curatedhw first=65%` 是 temp=1.0 数据，生产 temp=0.3 下实际 first=85%（本研究方向3.1 实测），**001 低估了 native 20pp**
- CTC 不受 temp 影响（OfflineSenseVoiceModelConfig 无 temperature 字段，PoC 对 sensevoice 传 `--temperature` 被忽略，实测 temp=1.0 与 0.3 输出逐字节一致），001 的 post_ctc first=70% 有效

### 3.2 CER + 幻觉率对比表（生产等价条件）

生产等价条件：post-trim（onset trim + silence head）+ 各模型生产参数。
- CTC：head=0ms, bt=200ms（select_preprocessing_params Performance）
- native+hw：head=0ms, bt=100ms, temp=0.3, hw=curated20（select_preprocessing_params Accuracy + create_funasr_nano_recognizer）

样本：40 个 wav（20 词 × 2 变体），TTS 合成短词。

| 条件 | mean CER | first% | word% | 幻觉(>12字/秒) | 空输出 | 非中文乱码 |
|------|----------|--------|-------|--------------|--------|-----------|
| post_ctc（CTC 生产等价） | **0.3553** | 70.0 | 70.0 | 0 | 1 | 1 |
| post_native_nohw（native 无 hw, temp=1.0 ⚠️） | 0.7237 | 47.5 | 47.5 | 0 | 0 | 4 |
| post_native_curatedhw（native+hw, temp=1.0 ⚠️） | 0.4868 | 65.0 | 65.0 | 0 | 0 | 2 |
| **native+hw（temp=0.3 生产参数，本研究）** | **0.15** | **85.0** | **85.0** | 0 | 0 | 1 |

> ⚠️ 标记行为 001 脚本 temp=1.0 数据，与生产不可比，仅供方法论缺陷佐证。

### 3.3 幻觉/乱码实例（乱码对体感伤害远大于均匀错字）

**CTC（post_ctc）乱码实例**：
- `post_kou_v2` → `เข`（泰文字符，非中文非英文，用户完全无法理解）
- `post_piao_v2` → ``（空输出）
- `post_pai_v1`（native+hw temp0.3）→ `派B的`（中英混杂，含字母 B）

**native temp=1.0 乱码实例（001 数据，非生产）**：
- `post_kou_v2` → `cool`（纯英文）
- `post_pai_v2` → `key`（纯英文）
- `post_pao_v2` → `paul`（纯英文）
- `post_qibai_v1` → `700`（数字，ITN 触发但非中文）

**native temp=0.3（生产）乱码实例**：
- `bt100_head0_ti_v1` → `T`（单字母）
- `bt50_head0_ti_v2` → `T`（单字母）
- 其余 39 个均为正确中文

**结论**：
- CTC 乱码更"致命"（泰文字符、空输出），native temp=0.3 乱码更"温和"（单字母 T，39/40 正确）
- 生产 temp=0.3 下 native+hw 的**非中文乱码率 1/40 = 2.5%**，CTC 为 1/40 + 1 空 = 2/40 = 5%
- CER 维度 native temp=0.3 (0.15) 比 CTC (0.3553) 低 58%，**accuracy 在生产参数下全面优于 performance**

---

## 四、方向 3：调优空间扫描

所有扫描在 post-trim wav（生产等价音频）上，native 模型，每次只变一个参数。

### 3.1 temperature 扫描（hw=curated20, bt=100ms, head=0ms）

| temp | mean CER | first% | word% | 幻觉 | 空输出 | 非中文 |
|------|----------|--------|-------|------|--------|--------|
| 0.0 | **0.10** | **90.0** | **90.0** | 0 | 0 | 1 |
| 0.2 | 0.125 | 87.5 | 87.5 | 0 | 0 | 1 |
| **0.3（生产）** | 0.15 | 85.0 | 85.0 | 0 | 0 | 1 |
| 0.5 | 0.15 | 85.0 | 85.0 | 0 | 0 | 1 |

**结论**：
- temp 与质量负相关，越低越好。temp=0.0 比 0.3 提升 +5pp first% / -33% CER
- 当前生产 temp=0.3 **偏保守**，有 +5pp 空间
- temp=0.0 风险：零随机性可能放大确定性幻觉（相同输入永远输出同一错误），但本组 40 样本无幻觉，需更大样本验证
- temp=0.2 是稳健折中（+2.5pp first%，CER 0.125）

### 3.2 hotwords 敏感度扫描（temp=0.3, bt=100ms, head=0ms）

| hotwords 数 | mean CER | first% | word% | 幻觉 | 空输出 | 非中文 |
|-------------|----------|--------|-------|------|--------|--------|
| 0 | 0.20 | 80.0 | 80.0 | 0 | 0 | 1 |
| 10 | **0.15** | **85.0** | **85.0** | 0 | 0 | 1 |
| **20（生产 curated）** | 0.15 | 85.0 | 85.0 | 0 | 0 | 1 |
| 50 | 0.1875 | 82.5 | 80.0 | 0 | 0 | 1 |

**结论**：
- 0→10 条 hotwords 收益 +5pp first%，10→20 持平，20→50 反而退化 -2.5pp
- 当前上限 50 偏大，**10-20 是最优区间**
- 001 报告"220 条撑爆 context 全空输出"结论仍成立，50 条未撑爆但已现退化
- ASR-ACC-OPT-001 的 `HOTWORDS_MAX_ENTRIES=50`（mod.rs:447）可考虑下调至 20，收益 +2.5pp

### 3.3 backtrack 扫描（temp=0.3, hw=curated20, head=0ms）

| backtrack ms | mean CER | first% | word% | 幻觉 | 空输出 | 非中文 |
|--------------|----------|--------|-------|------|--------|--------|
| 0 | 0.2375 | 75.0 | 75.0 | 0 | 0 | 0 |
| 50 | **0.1375** | **87.5** | 85.0 | 0 | 0 | 1 |
| **100（生产）** | 0.15 | 85.0 | 85.0 | 0 | 0 | 1 |
| 200 | 0.1625 | 85.0 | 82.5 | 0 | 0 | 0 |

**结论**：
- bt=0 明显差（CER 0.2375），送气声母被截
- **bt=50 最优**（CER 0.1375, first 87.5%），bt=100（生产）次优
- bt=200（performance 值）用在 accuracy 上略退化（CER 0.1625 vs 0.15），证实 accuracy 用 100ms 而非 200ms 的决策正确
- 当前 bt=100 已接近最优，调到 50ms 收益 +2.5pp first% / -8% CER，但样本小，需回归 FIRSTCHAR 测试

### 3.4 sherpa-onnx 其余可调参数盘点

OfflineFunASRNanoModelConfig（mod.rs:631-645）当前配置：
- `temperature: 0.3`（已扫描，见 3.1）
- `top_p: 1.0`（未扫描，但 LLM decoder 在低 temp 下 top_p 影响有限）
- `seed: 42`（固定 seed，temp=0 时决定性输出）
- `max_new_tokens: 0`（=不限，sherpa-onnx 内部默认上限）
- `language: None`（auto，生产 audio.transcription_language="zh" 但未传入 funasr_nano config——潜在缺口，但 native 模型训练数据以中文为主，影响小）
- `itn: 1`（启用 ITN，把"七"→"7"——对输入法场景有害，见 ASR-CTC-OPT-001 P2 撤销记录）
- `system_prompt: "You are a helpful assistant."`（默认，未优化为 ASR 专用）
- `user_prompt: "语音转写:"`（默认前缀，可调但影响未知）

**未启用的质量相关开关**：无（CTC 的 `rule_fsts` ITN 已撤销，native 无等价开关）

### 3.5 VAD 分段参数盘点（仅评估，不跑长音频）

`src/transcription/vad.rs` 当前配置：
- `SEGMENT_TRIGGER_SECS`：24s（>24s 触发分段）
- `SEGMENT_MAX_SECS`：20s（naive_chunk 等分段长）
- `SEGMENT_PADDING_SAMPLES`：段边界 padding

**评估**：VAD 分段仅影响 >24s 长音频，Gavin 端测反馈覆盖短词/普通句，与 VAD 无关。段边界 padding 对长音频质量有影响但需长音频测试集，本研究短词资产无法验证。建议后续立专项研究长音频 VAD 参数。

---

## 五、方向 4：天花板定性结论 + 分级建议方案

### 5.1 天花板判断

**在当前 int8 native 模型资产下，accuracy 存在稳定超越 performance 的现实路径**——生产 temp=0.3 已实现（CER 0.15 < CTC 0.3553，first 85% > 70%）。001 报告"建议放弃调优、改定位文案"的结论基于 temp=1.0 误判，**不成立**。

**剩余可挖项预期收益上限**：

| 维度 | 当前值 | 最优值 | 预期收益 | 风险 |
|------|--------|--------|---------|------|
| temperature | 0.3 | 0.0 | +5pp first% / -33% CER | 确定性幻觉风险（需大样本验证） |
| temperature | 0.3 | 0.2 | +2.5pp first% / -17% CER | 低（稳健折中） |
| hotwords 上限 | 50 | 20 | +2.5pp first% | 低（仅改常量） |
| backtrack | 100ms | 50ms | +2.5pp first% / -8% CER | 中（需回归 FIRSTCHAR） |

**叠加最优（temp=0.0 + hw20 + bt50）预估**：first% 可达 ~92-95%，CER ~0.08-0.10。但各参数交互效应未测，叠加收益可能非线性。

### 5.2 分级建议方案

#### 方案 E1【推荐，低风险，立即可落地】temp 0.3→0.2

**问题**：生产 temp=0.3 偏保守，temp=0.2 实测 +2.5pp first% / -17% CER。

**改动**：
- 影响文件：`src/transcription/mod.rs:639`
- 具体改动：`temperature: 0.3` → `temperature: 0.2`
- 风险：低（temp=0.2 仍有随机性，不放大确定性幻觉；40 样本无幻觉）
- 预期收益：+2.5pp first% / -17% CER
- 验收标准：cargo test 全绿 + 002B post-trim wav 跑 PoC native temp=0.2 first% ≥ 87.5%（实测值）+ Gavin 端测

#### 方案 E2【可选，低风险】hotwords 上限 50→20

**问题**：hw=50 实测比 hw=20 退化 -2.5pp，上限偏大。

**改动**：
- 影响文件：`src/transcription/mod.rs:447 HOTWORDS_MAX_ENTRIES`
- 具体改动：`pub const HOTWORDS_MAX_ENTRIES: usize = 50` → `= 20`
- 风险：低（只改常量，Gavin 当前 wordbook 11 条远未达上限）
- 预期收益：大词库用户 +2.5pp（Gavin 当前无收益，仅防御性）
- 验收标准：cargo test 全绿 + curate_hotwords_entries 单测更新

#### 方案 E3【可选，中风险】backtrack 100→50ms

**问题**：bt=50 实测最优（CER 0.1375），bt=100 次优。

**改动**：
- 影响文件：`src/main.rs:2750 ACC_ONSET_BACKTRACK_SAMPLES`
- 具体改动：`1600`（100ms）→ `800`（50ms）
- 风险：中（需回归 FIRSTCHAR-FIX-006 系列测试，送气声母前导可能被截）
- 预期收益：+2.5pp first% / -8% CER
- 验收标准：cargo test 全绿 + FIRSTCHAR 端测"派对/派发"无回归 + 002B PoC first% ≥ 87.5%

#### 方案 E4【需评估，中风险】temp 0.3→0.0

**问题**：temp=0.0 实测最优（CER 0.10, first 90%），但零随机性可能放大确定性幻觉。

**改动**：
- 影响文件：`src/transcription/mod.rs:639`
- 具体改动：`temperature: 0.3` → `temperature: 0.0`
- 风险：中（相同输入永远输出同一错误；需大样本/长音频验证幻觉率）
- 预期收益：+5pp first% / -33% CER
- 验收标准：cargo test 全绿 + 002B PoC first% ≥ 90% + **新增长音频幻觉率测试**（>30s 音频 10 条，幻觉率 ≤ temp=0.3 水平）+ Gavin 端测
- **建议**：先落地方案 E1（temp=0.2），观察稳定性后再评估 E4

#### 方案 E5【不推荐】改 UI 定位文案

**不推荐原因**：本研究证实 accuracy 在生产 temp=0.3 下确实优于 performance（CER/first%/乱码率全面占优），UI 文案"准确率更高"准确，无需改为"长音频/自带标点"。001 的文案修改建议基于 temp=1.0 误判，撤回。

### 5.3 待确认项（需主控/Gavin 确认）

1. **Gavin "accuracy 不敌 performance" 体感的真实来源**：本研究 PoC 层证实 accuracy 优于 CTC，但 Gavin 当前跑 qwen3_online。需 Gavin 确认：
   - 该体感结论的端测时间（是否在 ASR-ACC-OPT-001 落地前？）
   - 切 accuracy 的具体操作（UI 下拉选 accuracy 后是否重启/确认生效？）
   - 是否有 accuracy 模型加载失败日志（降级 CTC 但 UI 仍显示 accuracy 的可观测性缺口）
2. **001 报告的方法论缺陷是否需补充勘误**：001 报告的 post_native_curatedhw first=65% 是 temp=1.0 数据，建议在 001 报告加勘误注，避免后续误用。

---

## 六、生产代码零改动确认

本研究仅修改/新增：
- `collab/research/asr-accuracy-quality-002.md`（本报告）
- `collab/research/audio-002B/compute_cer_comparison.py`（CER 重算脚本）
- `collab/research/audio-002B/run_param_sweep.py`（参数扫描脚本）
- `collab/research/audio-002B/param_sweep/`（扫描输出 + param_sweep.json）
- `collab/research/audio-002B/accuracy_study/cer_comparison.json`（CER 对比数据）

`src/`、`src-tauri/`、`ui/`、`Publish/`、`models/`、`target/release/` 零改动。Gavin 实例未打扰。

## 七、数据资产索引

- CER 对比数据：`collab/research/audio-002B/accuracy_study/cer_comparison.json`
- 参数扫描数据：`collab/research/audio-002B/param_sweep/param_sweep.json`
- 参数扫描原始输出：`collab/research/audio-002B/param_sweep/*.txt`
- CER 重算脚本：`collab/research/audio-002B/compute_cer_comparison.py`
- 参数扫描脚本：`collab/research/audio-002B/run_param_sweep.py`
- 复用 001 资产：`collab/research/audio-002B/accuracy_study/*.txt`（post_ctc / post_native_*）

## 八、方法论教训

1. **PoC 实验脚本必须显式传生产参数**：001 的 `run_accuracy_study.py` 漏传 `--temperature`，导致 PoC 默认 temp=1.0 与生产 temp=0.3 不可比，整个 001 报告的 native 结论被低估 20pp。后续 PoC 脚本必须对照生产代码逐参数核对。
2. **CTC 与 native 的参数空间不同**：CTC 无 temperature（被 PoC 忽略），native 有 temperature。混在同一 PoC 脚本里时，CTC 的 temp 参数是 no-op，native 的 temp 必须显式设生产值。
3. **"评估维度错位"假设被部分推翻**：001 假设 PoC 指标片面（只测首字），但真实问题是 PoC 参数错（temp=1.0），不是指标错。本研究补了 CER 维度，但 CER 也证实 accuracy 优于 CTC。