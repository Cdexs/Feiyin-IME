# RESEARCH-ASR-038 · 035 方案流式化重做 + VAD 计费门控 + 词库热词注入链路

> **任务**：RESEARCH-ASR-038（纯设计，零代码改动）
> **来源**：Gavin 推翻 035「录完整段再发」前提，要求真流式「边录边显示」
> **依据**：035 协议/能力事实（继续有效）+ DEC-050（overlay 回退）+ DESIGN-OVERLAY-037（coder-2 已验收）
> **红线**：`git diff --ignore-cr-at-eol --numstat -- src/` == 0，不改 035 原文档

---

## 〇、与 035 的关系

035 文档的**协议层（A1–A6）与能力层（B1–B5）事实部分继续有效**，本文件不重复。本文件替代 035 的**整合方案部分（§4.1~4.8）**，按流式前提重做管线设计、VAD 门控、热词链路、参数调优。

---

## 一、前置问题：计费时长口径

### 结论：文档未覆盖精确口径，按保守设计（墙钟会话时长）

**出处**：
- [服务端事件](https://help.aliyun.com/zh/model-studio/fun-asr-server-events)：`usage.duration` 标注「任务计费时长（秒）」，仅在 `sentence_end=true` 时返回
- [用户指南](https://help.aliyun.com/zh/model-studio/real-time-speech-recognition-user-guide)：示例 `print('任务计费时长（秒）：', message['payload']['usage']['duration'])`

**文档未明确区分**：
- 「上传音频时长」= 实际发送的二进制音频帧总时长
- 「墙钟会话时长」= 从 task-started 到 task-finished 的挂起时间

**保守设计**：按**墙钟会话时长**计——即假定会话内静音也计费，第②层门控（会话内静音不上传）对省钱无效。

**理由**：
1. `usage.duration` 在 `sentence_end=true` 时返回，且名为「任务计费时长」——这更像是「该句对应的音频段时长」而非整个会话墙钟。
2. 但文档没有明确说「只计有语音的段」或「只计上传的音频」。
3. **保守设计意味着门控①③的价值成立（晚建连/早断连减少会话时长），门控②暂不做**——即使后续确认按上传音频计费，门控①③也不亏，门控②可以后补。

**端测确认方法**：用 `-debug` 模式跑一次录音，对比 `usage.duration` 与实际录音时长/上传音频时长，即可确认口径。

---

## 二、VAD 计费门控设计

### 2.1 三层门控总览

| 层 | 目标 | 是否做 | 理由 |
|----|------|--------|------|
| **① 入口门控** | 按热键后先不建连，本地滚动 VAD 确认有语音才建连上传 | ✅ 做 | 按墙钟计费时，晚建连直接省会话时长；首字安全靠预缓冲补发保证 |
| **② 会话内静音门控** | 说话中途长停顿不上传 | ❌ 不做 | 计费口径未确认按上传音频计费前不做；且服务端 VAD 已有 `max_sentence_silence` 断句，本地再切静音与它交互复杂 |
| **③ 收尾门控** | 松键前尾部静音不传 | ✅ 做（轻量） | 松键即发 finish-task，服务端自然会处理尾部；本地不需要额外截断，收益小风险大（截断会吞尾字） |

**简化结论**：实际只做**①入口门控**，③是自然行为（finish-task 即结束），②暂不做。

### 2.2 ① 入口门控详细设计

**流程**：
```
用户按热键
  → 音频线程开始采集（含 600ms pre-roll 预缓冲）
  → 滚动 VAD 检测：每收到一个 chunk（~20ms），喂给 Silero VAD
  → VAD 确认有语音（speech_detected_by_vad = true）
  → 立即建连 WebSocket + 发 run-task + 等 task-started
  → task-started 后：先发 pre-roll 预缓冲（600ms）补回首字
  → 然后继续发实时 chunk
```

**关键参数**：

| 参数 | 建议值 | 理由 |
|------|--------|------|
| VAD 判定窗口 | **512 samples = 32ms**（Silero VAD 要求） | 硬件限制，silero_vad.onnx 固定窗口 |
| VAD 阈值 | **0.5**（与现有 `vad.rs:28` 一致） | 已调优值，不另设 |
| 确认有语音的条件 | **VAD 返回非空段（任意长度）** | 一次窗口命中即建连，不等多窗口累积（减少延迟） |
| 首字安全保证 | **600ms pre-roll 补发** | 现有 pre-roll 机制：热键前 600ms 音频已在缓冲里，建连后先发这段，保证 VAD 判定窗口的 32ms 之前的音频不丢 |

**吞字风险评估（红线①）**：

| 场景 | 是否吞字 | 原因 |
|------|----------|------|
| 正常说话（热键后 200ms 开口） | ❌ 不吞 | VAD 在开口后 ~32ms 命中，建连 + 握手 ~200ms，pre-roll 600ms 覆盖热键前 600ms 到热键后 ~200ms 的全部音频 |
| 极快开口（热键后立即说话） | ❌ 不吞 | pre-roll 600ms 从热键前开始缓冲，热键瞬间已有 600ms 历史，VAD 几乎立即命中 |
| 轻声/送气声母开头 | ⚠️ 有风险 | Silero VAD 对送气清声母的检测灵敏度待实测；若 VAD 漏检，首字可能被门控吞掉 |
| 纯静音（用户按了热键没说话） | ✅ 不建连 | 门控生效，省成本 |

**送气声母风险缓解**：
- VAD 阈值 0.5 是现有值，对送气声母可能偏高。但**降低阈值会引入噪声误触发**（环境噪声也建连）。
- **建议**：入口门控 VAD 阈值用 **0.3**（比现有 0.5 低），牺牲一些「纯静音不建连」的效果换取首字安全。0.3 对应「只要有微弱语音特征就建连」。
- **终极保底**：若 VAD 在 2 秒内未命中，**无条件建连**（宁可白花 2 秒会话费也不能让用户按了热键没反应）。

**握手耗时**：
- WebSocket TLS 握手 + run-task + 等 task-started：预计 **200-500ms**（取决于网络）
- 这段时间音频继续采集并缓冲，握手完成后一次性补发
- pre-roll 600ms + 握手期间 ~300ms 音频 = ~900ms 补发量，一个二进制帧 3200 bytes = 100ms，共 ~9 帧补发

### 2.3 ② 会话内静音门控（暂不做）

**明确写「不做」，理由**：
1. 计费口径未确认按上传音频计费——若按墙钟计费，不发也不省钱
2. 服务端已有 `max_sentence_silence`（默认 1300ms）做 VAD 断句，本地再切静音与之交互复杂
3. 切静音意味着要「停发 + 检测到新语音再续发」，但 WebSocket 连接已建立，停发不等于断连，墙钟仍在走
4. 若后续确认按上传音频计费，再设计此层——届时用本地 VAD 检测静音段，静音段不传二进制帧（但连接保持）

### 2.4 ③ 收尾门控（自然行为）

**松键即发 finish-task**，服务端收到后停止处理并返回 task-finished。尾部静音自然包含在最后发的音频帧里，服务端 VAD 会判为静音不产出句子。

**不需要本地截断尾部静音**——截断有吞尾字风险（用户最后一个字可能是轻声），收益极小。

### 2.5 `speech_detected` 回归面评估

**现状**：`speech_detected`（`audio/mod.rs:650`）是逐 chunk RMS 能量阈值（`rms > silence_threshold=0.01`），用于：
1. 决定**何时结束录音**（`silent_count >= silence_frames` → 静音超时停录）
2. 日志记录（`Recording complete: ... speech_detected=true`）

**被使用位置**（grep `speech_detected`）：
- `audio/mod.rs:651`：置位 `self.speech_detected = true`
- `audio/mod.rs:592`：日志输出
- **未被其他模块消费**——它只影响录音停止逻辑和日志

**结论：不换。并存。**

| 机制 | 用途 | 判据 | 保留 |
|------|------|------|------|
| `speech_detected`（RMS） | 决定何时停录 | 能量阈值 0.01 | ✅ 保留，行为不变 |
| 入口门控 VAD（Silero） | 决定何时建连 API | 模型判定 0.3 | ✅ 新增，不替换 |

**理由**：
- `speech_detected` 决定「何时停录」是 Gavin 端测过的行为，替换为 Silero VAD 会改变停录灵敏度（Silero 有 32ms 延迟 + 模型判据与 RMS 不同），回归风险高
- 两者职责正交：RMS 管停录、Silero 管建连
- 不违反红线②——不换 `speech_detected`，无行为变更

---

## 三、流式管线重做

### 3.1 录音与上传并发：线程/任务模型

**现状**：`spawn_worker_thread`（`main.rs:2173`）在 `WorkerCommand::Start` 后调 `audio_capture.record()`（阻塞直到松键），完成后调 `run_pipeline_core`。录音与转录是**串行**的。

**流式改为**：

```
WorkerCommand::Start
  → spawn 两个并发任务：
    ├── 音频采集线程：audio_capture.record_streaming()
    │     产出：crossbeam channel<AudioChunk>（实时流）
    │     停止条件：stop_recording_signal / 静音超时
    │
    └── ASR 流式线程：
          ├── [入口门控] 滚动 VAD 检测 → 确认有语音
          ├── 建连 WebSocket + run-task + 等 task-started
          ├── 从音频 channel 读 chunk → 补发 pre-roll → 持续发二进制帧
          ├── 同时收 result-generated → 推送增量文本到 overlay
          └── 收到 stop 信号 → 发 finish-task → 等 task-finished → 返回最终文本
  → ASR 线程返回最终文本 → 交给 run_pipeline_core 做 LLM 格式化
```

**与现有 `run_pipeline_core` 衔接**：
- `run_pipeline_core` 签名不变（仍接收 `samples_result: Result<Vec<f32>>`）
- 但流式模式下 `samples_result` 由 ASR 线程在结束时构造（从音频 channel drain 剩余 chunk 拼成完整 samples，用于 LLM 管线的 ITN/标点等后处理）
- 或者更简洁：流式 ASR 线程直接返回 `String`（最终文本），跳过 `run_pipeline_core` 的转录步骤，只走 LLM 格式化分支

**建议方案**：新增 `run_streaming_pipeline` 函数，与 `run_pipeline_core` 并列：
- 流式 ASR 在新函数内完成，返回最终文本
- LLM 格式化 / ITN / 标点 / 注入 复用 `run_pipeline_core` 的后半段逻辑（抽取为公共函数）

### 3.2 增量结果接收：数据结构

**服务端事件**：`result-generated` 有两种：
- `sentence_end=false`：中间结果（文本会修正）
- `sentence_end=true`：最终结果（该句定型，`usage` 出现）

**数据结构**：
```rust
struct StreamingAsrState {
    /// 已确认句（sentence_end=true 的句子文本，按顺序）
    confirmed_sentences: Vec<String>,
    /// 当前正在变化的句（最新中间结果的文本）
    current_sentence: String,
    /// 当前句 ID
    current_sentence_id: i32,
}

impl StreamingAsrState {
    /// 合并出完整显示文本（已确认句 + 当前句）
    fn display_text(&self) -> String {
        let mut text = self.confirmed_sentences.join("");
        text.push_str(&self.current_sentence);
        text
    }

    /// 处理 result-generated 事件
    fn on_result(&mut self, sentence_id: i32, text: &str, sentence_end: bool) {
        if sentence_end {
            // 句子确认：加入 confirmed，清空 current
            self.confirmed_sentences.push(text.to_string());
            self.current_sentence.clear();
        } else if sentence_id == self.current_sentence_id {
            // 同一句的中间修正：替换 current
            self.current_sentence = text.to_string();
        } else {
            // 新句开始（sentence_id 递增）
            self.current_sentence_id = sentence_id;
            self.current_sentence = text.to_string();
        }
    }
}
```

**与 overlay 的显示对齐**：
- DESIGN-OVERLAY-037 §12.2 Gavin 拍板：**整段流式文本统一白色，不区分已确认/未确认**
- 因此 overlay 侧只需 `display_text()` 的完整字符串，不需要区分 confirmed/current
- 但内部维护 `confirmed_sentences` 是为了松键后拿到**干净的最终文本**（只含 confirmed 句 + 最后一句的 final 版本）交给 LLM

### 3.3 与 overlay 的数据接口（🔴 写死）

**引用**：`collab/research/overlay-streaming-preview-design-001.md`（DESIGN-OVERLAY-037）

| 接口项 | 设计 |
|--------|------|
| **增量文本推送频率** | 每个 `result-generated` 事件都推。overlay 侧已有 16ms timer（60fps）自然节流，不会每个事件都重绘。窗口尺寸调整节流 100ms（DESIGN-OVERLAY-037 §5.2） |
| **推送方式** | 新增 `PipelineEvent::StreamingText(String)` → `overlay_request_for_event` 映射到 overlay 更新 `streaming_text` 字段 |
| **中间结果修正** | overlay 侧整段覆盖（`streaming_text = display_text()`），不区分哪句在变。DESIGN-OVERLAY-037 §12.2 拍板整段统一白色 |
| **取消信号（overlay → ASR）** | DESIGN-OVERLAY-037 §6 `OverlayUiEvent::EditRequested`：用户点击 overlay 文本区进入编辑态。ASR 侧接收后：① 关闭 WebSocket ② 丢弃 `StreamingAsrState` ③ 设置 `pipeline_cancelled=true` |
| **overlay 状态机对齐** | DESIGN-OVERLAY-037 §6 状态机：`Idle → Recording(流式显示) → StreamingEditing / FallingToProcessing → Processing → Done`。本设计的 ASR 线程在 `Recording` 阶段运行，进入 `StreamingEditing` 或 `FallingToProcessing` 时停止 |

**新增 PipelineEvent 变体**：
```rust
enum PipelineEvent {
    RecordingStarted,
    StreamingText(String),    // 新增：流式 ASR 增量文本
    Processing(String),
    Done,
    Cancelled,
    FocusLost(String),
    Error(String),
    FormatFailed,
}
```

**`overlay_request_for_event` 映射更新**：
```rust
PipelineEvent::StreamingText(text) => platform::OverlayRequest::UpdateStreamingText(text),
```
（`OverlayRequest` 需新增 `UpdateStreamingText` 变体，或复用 `Show` 带新 `OverlayStatus::RecordingWithText`）

### 3.4 松键后：整段交给 LLM 管线

**流程**：
1. ASR 线程收到 stop 信号 → 发 `finish-task` → 等 `task-finished`
2. 从 `StreamingAsrState` 取最终文本：`confirmed_sentences.join("") + 最后一句 final`
3. 发 `PipelineEvent::Processing(transcribing_msg)` → overlay 切 Processing 态
4. 最终文本走现有 LLM 管线：ITN → LLM 格式化 → 标点 → 注入

**F3 跨句能力**：整段文本一次性交给 LLM，F3 的跨句列表/结构化能力完整保留（DEC-050）。

### 3.5 取消与清理：三种中断

| 中断类型 | 触发 | ASR 线程动作 | overlay 状态 |
|----------|------|-------------|-------------|
| 用户进编辑态 | `OverlayUiEvent::EditRequested` | 关 WebSocket、丢弃 state、`pipeline_cancelled=true` | → StreamingEditing |
| ESC 取消 | `OverlayUiEvent::CancelRequested` | 同上 | → Cancelled → Hide |
| 网络失败 | WebSocket 断连 / 超时 | 关连接、发 `PipelineEvent::Error` | → Error |

**清理要点**：
- 关 WebSocket = 发 Close 帧 + drop socket（tungstenite drop 自然关闭）
- 音频采集线程由 `stop_recording_signal` 停止
- `pipeline_cancelled` 拦截后续松键的 `FallingToProcessing`（DESIGN-OVERLAY-037 §6）

### 3.6 回退方案

| 场景 | 回退 |
|------|------|
| 新模型服务不可用/超时 | 报错提示，不自动降级（DEC-028 原则不变） |
| 用户切回 `qwen3_online`（旧模型） | 旧模型仍可用，走旧非流式管线 |
| 用户切回 `performance`/`accuracy` | 本地模型，走旧非流式管线 |
| VAD 模型缺失 | 入口门控降级为「总是建连」（与现状一致） |

**关键**：流式管线**只对 `QwenAudioOnline` 模式生效**。其他模式走现有 `run_pipeline_core`，零行为变更。

### 3.7 影响文件清单

| 文件 | 改动类型 | 行号范围（估） | 说明 |
|------|----------|----------------|------|
| `src/transcription/qwen_inference.rs` | **新建** | ~500行 | 流式 Inference API 实现（run-task/二进制帧/finish-task/result-generated 解析/StreamingAsrState） |
| `src/transcription/mod.rs` | 改 | 30-48（AsrModel）、251-290（路由）、新增流式转录入口 | 新增 QwenAudioOnline + `transcribe_streaming` |
| `src/transcription/vad.rs` | 改（轻量） | 新增 `has_speech(samples) -> bool` 纯函数 | 入口门控用，复用现有 VadSegmenter |
| `src/audio/mod.rs` | 改 | 新增 `record_streaming()` 方法 | 返回 channel<AudioChunk> 而非阻塞 Vec<f32> |
| `src/main.rs` | 改 | 87-97（PipelineEvent 新增 StreamingText）、2173-2420（spawn_worker_thread 流式分支）、3229+（run_streaming_pipeline） | 管线核心改造 |
| `src/config/mod.rs` | 改 | 136-201（AudioConfig） | 新增 qwen_asr_url/qwen_asr_model |
| `src-tauri/src/config.rs` | 改 | 106-149 | 同步新增字段 |
| `src-tauri/src/qwen3.rs` | 改 | 43-166 | 新增 test_qwen_asr_connection |
| `src-tauri/src/main.rs` | 改 | 89-91、206 | 注册新 command |
| `ui/src/pages/Voice.tsx` | 改 | 模型选择下拉 | 新增选项 |
| `ui/src/i18n/*.ts` | 改 | — | 三语新增 |

**跨平台**（DEC-033）：
- `src/transcription/`、`src/config/`、`src-tauri/src/config.rs` 平台中立
- `src/audio/mod.rs` 的 `record_streaming()` 可能需要平台分支（WASAPI vs cpal 跨平台）
- `src/main.rs` 流式管线在 `#[cfg(target_os="windows")]` 内（现有管线也在 cfg 内）
- macOS 侧需同步 `AsrModel` 新变体 + `PipelineEvent::StreamingText` 的 match 分支

### 3.8 分批实施建议

| 批次 | 内容 | 验收标准 | 文件域 |
|------|------|----------|--------|
| **ASR-038-A** | `qwen_inference.rs` 流式模块（纯函数 + StreamingAsrState + transcribe_streaming） + `AsrModel` 枚举 + config 字段 | `cargo check` 0 error + 纯函数单测 | `src/transcription/`、`src/config/` |
| **ASR-038-B** | VAD 入口门控 + `record_streaming()` + `run_streaming_pipeline` + PipelineEvent 新增 + overlay 数据接口 | `cargo test` 回归 + `cargo check --all-targets` 0 error | `src/audio/`、`src/main.rs`、`src/transcription/` |
| **ASR-038-C** | Tauri config 镜像 + 连测 command + UI 下拉 + i18n | `npm run build` + Vitest + src-tauri check | `src-tauri/`、`ui/` |
| **TEST-SYNC-038** | 测试同步 | tester-1 | 各 mod tests |
| **TEST-EXEC-038** | 全量回归 | tester-1 | — |
| **BUILD-038** | 出包 | tester-1 | — |

A→B 串行（同文件域 `src/transcription/`），C 可与 B 并行。

---

## 四、词库热词注入链路

### 4.1 注入设计

**路径**：
```
src/wordbook/mod.rs  Wordbook::list_all() → Vec<WordEntry>
  → 按 source 分层权重：
    source = "user"（手工添加，专名领域词）→ weight = 5
    source = "system"（历史 bug 沉淀保护词，非自动学习）→ weight = 4
  → 长度过滤（见 4.2）
  → build vocabulary JSON: {"词1": 5, "词2": 4, ...}
  → run-task payload.parameters.vocabulary
```

**当前词库仅 ~15 条，远低于 2000 上限 → 全量注入，不需要筛选逻辑**。

**权重分层**（主控建议，采纳）：
- 手工添加 = 5（用户明确意图，高权重）
- 自动学习 = 4（可能有噪声，中等权重）
- 超级热词（50）v1 不用

### 4.2 长度限制过滤策略

**官方限制**：含非ASCII ≤15 字符；纯ASCII 按空格切分 ≤7 片段。

**策略：丢弃超限词条不注入 ASR，但保留给 LLM 提示词**（主控倾向第三种，采纳）。

**理由**：
- ASR 热词超限会被服务端忽略（不报错但无效），注入了也白费
- LLM 提示词没有这个长度限制，词库在 LLM 侧的纠偏能力不应因 ASR 限制而削弱
- 实现：`build_asr_vocabulary()` 做过滤，`build_llm_wordbook_prompt()` 不做过滤（现状不变）

**过滤函数**：
```rust
fn passes_asr_vocab_limit(word: &str) -> bool {
    let has_non_ascii = word.chars().any(|c| !c.is_ascii());
    if has_non_ascii {
        word.chars().count() <= 15
    } else {
        word.split_whitespace().count() <= 7
    }
}
```

### 4.3 热词与 LLM 提示词词库的关系

**两边都注入会不会重复纠正、互相打架？**

**不会**，理由：
- ASR 热词作用于**声学→文本**阶段（解码时偏向输出这些词）
- LLM 词库作用于**文本→文本**阶段（纠错/优化时参考这些词）
- 两者是流水线上下游：ASR 先把音频转成文本（热词帮助它听对），LLM 再优化文本（词库帮助它改对）
- 最坏情况：ASR 听成「风无星」，热词「风无心」把它纠成「风无心」；LLM 拿到「风无心」，词库里也有「风无心」，LLM 确认不改。**一致，不打架**
- 若 ASR 热词纠错了（把不该纠的纠了），LLM 词库无法知道原始音频，只能基于文本判断——这是已有局限，不是新问题

### 4.4 词库变更的热更新

**现有机制**：`hotwords_version`（`transcription/mod.rs:72`）= 词表内容哈希，用于 accuracy 模式感知变更触发重建。

**流式会话进行中词库变了怎么办？**

**结论：会话中不变。** 流式会话开始时取词表快照，整个会话用同一份。会话结束后下次录音自动用新词表。

**理由**：
- 流式会话短（一次录音几秒到几分钟），词库变更概率极低
- `run-task` 的 `vocabulary` 在任务开始时设定，**任务中途无法更新热词**（`continue-task` 只更新 context，不更新 vocabulary）
- 若要中途更新，需结束当前 task 再开新 task——等于断句，体验割裂

**复用 `hotwords_version`**：在 `Transcriber` 新增 `hotwords_snapshot: Vec<(String, i32)>`（词+权重），`hotwords_version` 变更时更新快照。流式会话开始时用快照构建 vocabulary。

### 4.5 本地模型分支

**Performance 不支持热词**——本次只报不改。

**现状**：
- Performance（179MB CTC）：不支持热词（sherpa-onnx CTC 架构限制）
- Accuracy（972MB native）：支持 hotwords（现有 `build_hotwords_string` 逗号分隔串）
- QwenAudioOnline（新）：即时热词 `vocabulary` JSON

**本次不处理 Performance 的热词缺口**——CTC 架构不支持热词是模型限制，非代码缺陷。若 Gavin 要求 Performance 也支持热词，需换模型（不在本任务范围）。

### 4.6 词库范围定稿（Gavin 澄清 + 主控核实实际 DB）

**Gavin 澄清**：词库 = 现有 SQLite `wordbook` 表的 `source='system'` + `source='user'` 两类词条。

**主控核实实际 DB**（`target/release/wordbook.sqlite`）：

| source | 条数 | 内容 | 最长字符 |
|--------|------|------|----------|
| `system` | 8 | 艾丁湖/你好/摄氏度/三五成群/LLM/Cloud/M2/维生素B12 | 6（维生素B12） |
| `user` | 10 | 采编/漫剧/文案/风无心/隋变/派安盈/主控/coder1/coder2/tester1 | 4 |
| **合计** | **18** | 全部通过热词长度校验 | — |

距 2000 上限极远，**全量注入无需筛选逻辑**。超限过滤策略（§4.2）在当前数据下是空转，但**保留作为防御**——未来词库增长后生效。

**🔴 红线新增：`wordbook_candidates` 表绝对不得注入**

`wordbook_candidates` 是自动学习**待确认候选**表（163 条），含：
- 纯数字（如 `3`/`6`）——不是词
- 31 字整句（如「风五心刚刚穿越到人间界…」）——既未确认又超 15 字上限会被服务端拒

**实现要点**：只读 `wordbook` 表（`Wordbook::list_all()`），**不读 `wordbook_candidates` 表**。现有 `list_all()` 只查 `wordbook` 表，天然安全，但方案里显式写明此禁止项以防未来误改。

### 4.7 权重定稿（基于实际数据判断）

**user=5、system=4**。

**理由（基于实际 DB 内容的判断，非拍脑袋）**：
- **user 词条**（采编/漫剧/文案/风无心/隋变/派安盈）多为专名领域词、同音词多最易错——是热词最该发力处，权重 5
- **system 词条**（艾丁湖/你好/摄氏度/三五成群/LLM/Cloud/M2/维生素B12）多为通用词或历史 bug 沉淀的保护词，本身识别率不低——权重过高可能在不该出现的语境强推，权重 4

### 4.8 热词 vs ITN 保护词表：事前引导 vs 事后保护（不可合并）

**system 词库内容暴露的洞察**：`三五成群` 在 `wordbook` 表中是因曾被 ITN 转成 `35成群`（见 `todo.md` ITN 成语漏保护节）。这类词现靠两条路保护：

| 路 | 机制 | 时机 | 语义 |
|----|------|------|------|
| **热词**（ASR vocabulary） | 事前引导 ASR 识别成这个词 | 声学→文本 | 「请优先识别成这个词」 |
| **ITN 保护词表**（itn-rules.toml） | 事后阻止数字化 | 文本→数字规整 | 「别把这个词数字化」 |

热词是事前引导，让 ASR 一开始就识别对比出错再修更根本。但**两条路必须分清不可合并**——热词说「识别成这个词」，ITN 保护词表说「别把这个词数字化」，混用会污染。

**结论**：`itn-rules.toml` 保护词表**仍不注入 ASR 热词**。此判断因看到 system 词库实际内容（`三五成群` 同时存在于 wordbook 和 itn-rules.toml，但两处机制不同）而更确定。

---

## 五、参数调优建议

### 5.1 流式场景参数取值

| 参数 | 建议值 | 理由 |
|------|--------|------|
| `semantic_punctuation_enabled` | **false**（默认） | VAD 断句延迟低，适合「边说边出字」。语义断句更准但延迟高，不适合交互场景 |
| `max_sentence_silence` | **800ms**（低于默认 1300ms） | 「边说边出字」手感：用户停顿 800ms 就应该出句号 + 最终结果。1300ms 太慢，用户觉得卡。但太低（如 200ms）会把正常停顿误断句。800ms 是对话场景的常见推荐值 |
| `heartbeat` | **false**（默认） | 入口门控保证只在有语音时建连，会话不会太长（一次录音），不需要心跳保活。若用户录音超长（>30s）且中途停顿，服务端 VAD 会断句但不断连 |
| `speech_noise_threshold` | **0.0**（默认，不调） | 本地不做降噪（035 §5.9 结论 A），服务端默认阈值已合理。若端测发现噪声误触发，调到 +0.3 降低灵敏度 |
| `language_hints` | **固定 `["zh","en","ja","ko"]`**（Gavin 拍板） | 见下方 §5.3 专项设计 |

### 5.2 DEC-031 核对

**全部走配置文件级，不暴露给用户。**

| 参数 | 是否暴露 | 理由 |
|------|----------|------|
| `semantic_punctuation_enabled` | ❌ | 内部调优，用户不懂语义断句 |
| `max_sentence_silence` | ❌ | 内部调优，用户不懂 VAD 静音阈值 |
| `heartbeat` | ❌ | 内部行为 |
| `speech_noise_threshold` | ❌ | 高级参数，调整可能显著影响效果 |
| `language_hints` | ❌（固定四语，见 §5.3） | Gavin 拍板不让用户选，固定传四语 |

**DEC-031 核对结论**：不新增用户可见开关/阈值，全部配置文件级。**不冲突**。

### 5.3 language_hints 专项设计（Gavin 拍板 + 主控定稿）

**Gavin 决策**：产品默认支持中英韩日四种输入语言，`language_hints = ["zh","en","ja","ko"]` **无条件固定传入**，让 ASR 后端在这四语范围内自动检测。不让用户自己选。

**四码校验**：
- 正好卡在 Qwen-Audio-3.0 的 4 个上限，不会截断
- 四码（zh/en/ja/ko）均在官方 28 语支持列表内
- 出处：[客户端事件 - language_hints](https://help.aliyun.com/zh/model-studio/fun-asr-client-events)

**架构一致性佐证**：项目 LLM 提示词层本来就是按中英日韩四语设计的——F3 规则原文 `it applies to all supported input languages: Chinese, English, Japanese, Korean`，枚举标记清单四语对称。ASR 语言范围与 LLM 侧对齐后，整条链路语言边界统一。

**数组顺序是否影响优先级**：**文档未覆盖**。官方原文只说「最多支持设置 4 个值，即便设置超出 4 个，也仅前 4 个生效」，未提及顺序与优先级的关系。保守按 `["zh","en","ja","ko"]` 顺序（中文优先，符合产品主语言），但不假定顺序影响检测结果。

**`transcription_language` 已废弃（LANG-AUTO-001）**：

主控核实确认：`transcription_language` 已由 LANG-AUTO-001 彻底废弃，比原方案判断更彻底：
- `main.rs:2210`：`Transcriber::new` 的 `asr_language` 参数恒为 `"auto"`
- `main.rs:2335`：热重载已移除语言变更监听
- `text_normalizer.rs:4`：简繁转换改按 `contains_han` 内容门控，不读 `transcription_language`
- `main.rs:3676`：翻译方向亦按内容判定（LANG-AUTO-001），不依赖 `transcription_language`

**`transcription_language` 是死字段**。本方案**不复活它**——不读它、不根据它分支、不新增 UI 选择器。`language_hints` 无条件固定为四语。

~~方案 A/B 选择~~ → **主控定稿：直接固定四语，无需 A/B 选择**。原「保留手动锁定逃生口」判断作废（已更正）。

实现：
```rust
// 无条件固定，不读 transcription_language
const ASR_LANGUAGE_HINTS: &[&str] = &["zh", "en", "ja", "ko"];
```

---

## 六、版本号

建议 `0.8.0`（minor bump，流式 ASR + VAD 门控 + 热词注入属功能增量）。**由主控统一确认，本方案不自行决定**。

---

## 七、未解问题

| # | 问题 | 处置 |
|---|------|------|
| 1 | 计费时长口径（上传音频 vs 墙钟） | 端测时观察 `usage.duration` 确认；按墙钟保守设计 |
| 2 | Silero VAD 对送气声母的检测灵敏度 | 实测；入口门控阈值 0.3 + 2s 无条件建连保底 |
| 3 | ~~「系统词库」含义~~ | **Gavin 已澄清**：现有 system + user 两类，按 §4.6 定稿 |
| 4 | `max_sentence_silence` 最佳值 | 800ms 建议，端测后调优 |
| 5 | 版本号 | Gavin 指定 |
| 6 | ~~`language_hints` 取值~~ | **Gavin 拍板 + 主控定稿**：无条件固定 `["zh","en","ja","ko"]`，transcription_language 已废弃不复活 |
| 7 | ~~`transcription_language` 方案 A/B~~ | **主控定稿**：无需选择，直接固定四语（A/B 收敛为同一结果） |

---

## 八、出处链接汇总

| 内容 | 链接 |
|------|------|
| 035 原方案（协议/能力事实） | `collab/research/asr-qwen-audio-3.0-integration-001.md` |
| DESIGN-OVERLAY-037（overlay 交互设计） | `collab/research/overlay-streaming-preview-design-001.md` |
| 服务端事件（usage.duration） | https://help.aliyun.com/zh/model-studio/fun-asr-server-events |
| 客户端事件（run-task 参数） | https://help.aliyun.com/zh/model-studio/fun-asr-client-events |
| 用户指南（VAD 断句配置） | https://help.aliyun.com/zh/model-studio/real-time-speech-recognition-user-guide |
| 提升识别准确率（即时热词） | https://help.aliyun.com/zh/model-studio/improve-asr-accuracy |

---

> **本文件为零代码改动设计产物**。`git diff --ignore-cr-at-eol --numstat -- src/` == 0。