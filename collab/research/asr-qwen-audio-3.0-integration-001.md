# RESEARCH-ASR-035 · qwen-audio-3.0-asr-flash-streaming 接入研究与整合方案

> **任务**：RESEARCH-ASR-035（纯研究，零代码改动）
> **来源**：Gavin 2026-08-14 原话要点（见任务书 §一）
> **交付**：本文件 + `outbox/coder-1/result.md`
> **红线**：`git diff --ignore-cr-at-eol --numstat -- src/` == 0

---

## 〇、方案评估与异议

### 对主控方案的评估

主控任务书的前提判断（「这是换协议，不是换模型名」「`qwen3_online.rs` 基本要重写」）**完全成立**。我在独立抓取阿里云三页官方文档（WebSocket API / 客户端事件 / 服务端事件）+ 用户指南 + 提升准确率 + 选型 + 定价页后，逐条核实，无异议。两个协议在事件名、消息结构、音频发送方式（base64 文本帧 → 二进制帧）、鉴权 header 上全部不同，不存在「改几个常量」的可能。

### 一处补充建议（非异议）

主控任务书 §2.3 说「上下文需要你评估从哪来」。我在 B2 调研中发现官方提供了 `continue-task` 事件用于**任务运行中更新上下文**。这意味着上下文不一定要在 `run-task` 时一次性塞满——**可以在一次录音会话中动态追加**。这对「上一句的识别结果作为下一句的上下文」这类场景更自然。本方案在 4.4 给出了具体链路设计。

---

## 一、协议层问答（A1–A6）

### A1 · `run-task` 完整 JSON（逐字段说明）

**出处**：[客户端事件 - run-task](https://help.aliyun.com/zh/model-studio/fun-asr-client-events)

```json
{
  "header": {
    "action": "run-task",
    "task_id": "<32位UUID>",
    "streaming": "duplex"
  },
  "payload": {
    "task_group": "audio",
    "task": "asr",
    "function": "recognition",
    "model": "qwen-audio-3.0-asr-flash-streaming",
    "parameters": {
      "format": "pcm",
      "sample_rate": 16000,
      "vocabulary_id": "<id>",
      "vocabulary": {"张三": 5},
      "language_hints": ["zh"],
      "semantic_punctuation_enabled": false,
      "max_sentence_silence": 1300,
      "multi_threshold_mode_enabled": false,
      "heartbeat": false,
      "speech_noise_threshold": 0.0,
      "special_word_filter": "..."
    },
    "input": {
      "context": [
        {"role": "user", "content": [{"type": "input_text", "text": "前轮识别结果"}]},
        {"role": "assistant", "content": [{"type": "text", "text": "前轮回复"}]}
      ]
    }
  }
}
```

**关键字段说明**：
- `task_id`：UUID 格式，Python 示例用 `uuid.uuid4().hex[:32]`（32位无连字符）。服务端只要求与后续 `finish-task`/`continue-task` 的 `task_id` 一致。
- `streaming: "duplex"`：双工模式，音频上行 + 文本下行同时进行。
- `model`：**写在 payload 里**（旧 Realtime API 写在 URL query），这是两种协议的关键区别之一。
- `parameters` 全部字段均为可选除 `format`/`sample_rate` 必选外。

### A2 · 音频发送方式：二进制帧，不是 base64

**出处**：[WebSocket API 交互流程](https://help.aliyun.com/zh/model-studio/fun-asr-realtime-websocket-api) + [用户指南原始协议示例](https://help.aliyun.com/zh/model-studio/real-time-speech-recognition-user-guide)

**结论**：音频以 **WebSocket Binary 帧**发送，不是 base64 文本帧。

```python
# 官方 Python 原始协议示例
ws.send(chunk, opcode=websocket.ABNF.OPCODE_BINARY)
time.sleep(0.1)  # 100ms 间隔模拟实时
```

- **没有 `continue-task` 用于音频**。`continue-task` 只用于**更新上下文**（见 B2），不用于音频。
- 音频发送完才发 `finish-task`。
- 分片要求：官方示例统一用 **3200 bytes/片 = 100ms @ 16kHz 16bit mono**。文档未明文规定上限，但所有示例都用 3200。**建议沿用 3200**。

### A3 · 服务端事件完整 JSON

**出处**：[服务端事件](https://help.aliyun.com/zh/model-studio/fun-asr-server-events)

#### task-started（任务启动成功）
```json
{"header": {"task_id":"<UUID>","event":"task-started","attributes":{}}, "payload": {}}
```
收到后才能发音频。

#### result-generated（识别结果，含中间与最终）
```json
{
  "header": {"task_id":"<UUID>","event":"result-generated","attributes":{}},
  "payload": {
    "output": {
      "sentence": {
        "begin_time": 170, "end_time": 920, "text": "好，我知道了",
        "heartbeat": false, "sentence_begin": true, "sentence_end": true,
        "sentence_id": 1,
        "words": [
          {"begin_time":170,"end_time":295,"text":"好","punctuation":"，"},
          {"begin_time":295,"end_time":503,"text":"我","punctuation":""}
        ]
      }
    },
    "usage": {"duration": 3}
  }
}
```
- `usage` 在 `sentence_end=false` 时为 `null`。
- **标点在 `words[].punctuation` 字段**——每个字后跟的标点，无则空串。句级 `text` 也含标点。

#### task-finished（任务正常结束）
```json
{"header": {"task_id":"<UUID>","event":"task-finished","attributes":{}}, "payload": {"output":{}, "usage": null}}
```

#### task-failed（任务失败，连接关闭）
```json
{
  "header": {
    "task_id":"<UUID>","event":"task-failed",
    "error_code": "CLIENT_ERROR", "error_message": "request timeout after 23 seconds.",
    "attributes": {}
  }, "payload": {}
}
```

### A4 · 分片要求

**出处**：[用户指南 - 原始协议示例](https://help.aliyun.com/zh/model-studio/real-time-speech-recognition-user-guide)

- **每片 3200 bytes** = 100ms @ 16kHz/16bit/mono（1600 samples × 2 bytes）。
- **发送间隔**：官方示例用 `time.sleep(0.1)`（100ms），模拟实时流。我们的场景是**录完整段再发**（非真流式），可以不 sleep 或用更短间隔，但**不建议一次性全发**——服务端按流式设计，瞬间灌入大量数据可能触发内部缓冲限制（文档未明文，但 SDK 示例全部 sleep 0.1s）。
- **没有固定间隔的硬性要求**，也没有明文上限。建议：3200 bytes/片，间隔 20–100ms。
- **我们的现有代码** `qwen3_online.rs:92-98` 的 `chunk_pcm_to_base64` 用 3200 samples/片 = 6400 bytes/片（200ms），这是 base64 文本帧的块大小。新协议是二进制帧，建议改回 **3200 bytes/片**（1600 samples = 100ms），与官方示例对齐。

### A5 · 鉴权 header 全集

**出处**：[WebSocket API - 请求头](https://help.aliyun.com/zh/model-studio/fun-asr-realtime-websocket-api)

| Header | 必选 | 说明 |
|--------|------|------|
| `Authorization` | **是** | `Bearer <api_key>`，握手阶段验证，失败返回 401/403 |
| `user-agent` | 否 | 客户端标识 |
| `X-DashScope-WorkSpace` | 否 | 业务空间ID |
| `X-DashScope-DataInspection` | 否 | 数据合规检测，默认不传 |

**关键差异**：旧 Realtime API 需要 `OpenAI-Beta: realtime=v1` header。**新 Inference API 不需要这个 header**。鉴权只需要 `Authorization: Bearer <key>`。

### A6 · URL 里 `{WorkspaceId}` 的取值

**出处**：[WebSocket API - 接口地址](https://help.aliyun.com/zh/model-studio/fun-asr-realtime-websocket-api)

新端点：`wss://{WorkspaceId}.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference`

**与现用 URL 的关系**：
- 现用（生产 debug.log）：`wss://llm-kudx4dj2bfqn4gr2.cn-beijing.maas.aliyuncs.com/api-ws/v1/realtime?model=qwen3-asr-flash-realtime`
- 新端点：`wss://{WorkspaceId}.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference`

**`llm-kudx4dj2bfqn4gr2` 就是 WorkspaceId**。两协议的主机名格式完全一致（`{WorkspaceId}.{region}.maas.aliyuncs.com`），只是路径从 `/api-ws/v1/realtime` 变为 `/api-ws/v1/inference`，且 model 从 URL query 移到 payload。

**迁移方式**：现有 `qwen3_asr_url` 配置项存的是 `wss://...maas.aliyuncs.com/api-ws/v1/realtime`。新协议需要改为 `wss://...maas.aliyuncs.com/api-ws/v1/inference`。**建议新增独立配置项 `qwen_asr_url`**（不破坏旧配置）。

**⚠️ WorkspaceId 获取**：[获取 Workspace ID](https://help.aliyun.com/zh/model-studio/obtain-the-app-id-and-workspace-id) —— 在百炼控制台获取。现有 URL 已含 WorkspaceId（`llm-kudx4dj2bfqn4gr2`），用户只需把 `/realtime` 改成 `/inference`。

---

## 二、能力层问答（B1–B5）

### B1 · 热词注入：即时热词（无需预建词表），这是决定性优势

**出处**：[客户端事件 - run-task](https://help.aliyun.com/zh/model-studio/fun-asr-client-events) + [提升识别准确率](https://help.aliyun.com/zh/model-studio/improve-asr-accuracy)

**两种热词方式**：

| 方式 | 字段 | 格式 | 是否需预建词表 | 适用 |
|------|------|------|----------------|------|
| **预编译热词** | `parameters.vocabulary_id` | 字符串ID | **是**，需先调 HTTP API 创建词表拿ID | 词汇稳定、跨请求复用 |
| **即时热词** | `parameters.vocabulary` | `{"词": 权重}` | **否**，随请求内联 | 临时、会话级 |

**决定性发现**：`qwen-audio-3.0-asr-flash-streaming` **是唯一支持即时热词的模型**（官方明确：「仅 qwen-audio-3.0-asr-flash-streaming 支持即时热词」）。

**即时热词格式**：
```json
"vocabulary": {"张三": 5, "李四": 5, "语音实验室": 50}
```
- 键 = 热词文本（string），值 = 权重（integer）
- 权重范围：`[1, 5]` 或 `50`（50=超级热词，最多50个）
- 普通+超级同时存在时，超级最多50个，合并后总数上限 **2000**

**热词文本长度限制**：
- 含非ASCII字符：总字符数 ≤ 15（如 `"厄洛替尼盐酸盐"` 7字符 OK）
- 纯ASCII字符：按空格切分片段数 ≤ 7（如 `"Exothermic reaction"` 2片段 OK）

**对本项目的意义**：**无需新增词表同步机制**。现有 `src/wordbook/` 的用户词库可以直接作为即时热词 `vocabulary` 内联到 `run-task` 请求中。这是比旧模型（`qwen3-asr-flash-realtime` 不支持热词）巨大的提升。

**权重建议**：从 `weight=4` 起测（官方推荐），词库里的词统一用 4。超级热词（50）暂不用。

### B2 · 上下文 context：字段名、长度上限、语义

**出处**：[客户端事件 - run-task/continue-task](https://help.aliyun.com/zh/model-studio/fun-asr-client-events) + [提升识别准确率 - 上下文增强](https://help.aliyun.com/zh/model-studio/improve-asr-accuracy#ctx-enhance-h2)

**字段名**：`payload.input.context`，数组，每元素 `{role, content: [{type, text}]}`。

**两种角色**：
| role | type | 语义 |
|------|------|------|
| `user` | `input_text` | 前几轮用户语音的**识别结果**或**领域词表** |
| `assistant` | `text` | 前几轮大语言模型的**回复内容** |

**约束**：
- 消息条数：`input_text` 和 `text` 各最多 **5 条**，超出保留最近 5 条
- 每轮文本总长度（user+assistant 的 text 字段长度之和）≤ **400 字符**（按字符数计，每个字符计1）
- 顺序：按对话轮次排列，每轮 user 在 assistant 之前

**语义**：上下文主要用于**词表匹配**修正专有词汇。`text` 字段需包含音频里待识别的原词。仅传语义相关但不包含原词的描述，纠正效果有限。

**运行中更新**：`continue-task` 事件可在任务运行中更新上下文：
```json
{
  "header": {"action":"continue-task","task_id":"<UUID>","streaming":"duplex"},
  "payload": {"input": {"context": [/* 新的上下文消息 */]}}
}
```

**对本项目的意义**：
1. **最简方案**：不传上下文（`input: {}`），仅靠即时热词。适合 v1。
2. **进阶方案**：传上一句的识别结果（user）+ LLM 回复（assistant）作为上下文，提升连续对话的专有词识别。
3. **词表方案**：把用户词库也作为 `input_text` 传入上下文（与即时热词互补）。

### B3 · language 指定方式

**出处**：[客户端事件 - language_hints](https://help.aliyun.com/zh/model-studio/fun-asr-client-events)

**字段名**：`parameters.language_hints`，**数组**（不是旧 API 的单个字符串）。

- Qwen-Audio-3.0 系列：最多 **4 个**值（超出仅前4个生效）
- Fun-ASR-Realtime 系列：仅 1 个值
- **不设置时模型自动检测**

**支持的语言代码**（Qwen-Audio-3.0-asr-flash-streaming）：zh / en / ja / ko / vi / th / id / ms / tl / hi / ar / fr / de / es / pt / ru / it / nl / sv / da / fi / no / el / pl / cs / hu / ro / bg / hr / sk（共28种）

**与旧 API 的差异**：
- 旧 Realtime API：`session.input_audio_transcription.language`（单个字符串）
- 新 Inference API：`parameters.language_hints`（数组）

**本项目映射**：现有 `transcription_language` 配置值 `zh`/`en`/`ja`/`ko`/`auto`。
- `auto` → 不设 `language_hints`（自动检测）
- 其他 → `language_hints: [config值]`（单个元素数组）

### B4 · 标点 / ITN 开关字段

**出处**：[客户端事件 - semantic_punctuation_enabled](https://help.aliyun.com/zh/model-studio/fun-asr-client-events)

**关键发现**：新协议**没有独立的「标点开关」字段**，也没有独立的 ITN 开关。

相关参数：
| 字段 | 默认 | 说明 |
|------|------|------|
| `semantic_punctuation_enabled` | `false` | true=语义断句（更准但延迟高），false=VAD断句（延迟低） |
| `max_sentence_silence` | 1300ms | VAD断句静音阈值，[200,6000] |

**标点行为**：`result-generated` 的 `sentence.text` **自带标点**，`words[].punctuation` 字段携带字级标点。模型默认输出带标点的文本。

**与 DEC-047 的关系**：
- DEC-047「是否含标点的判据全子系统统一」核心是：**不再假定在线 ASR 必然带标点**，改用 `has_effective_punctuation` 实测。
- 新模型 `qwen-audio-3.0-asr-flash-streaming` 的 `result-generated.sentence.text` **带标点**（从官方示例可见 `"好，我知道了"`）。
- **结论**：现有 `has_effective_punctuation` 判据**继续适用**，无需修改。新模型的标点产出更可靠（有 `words[].punctuation` 字段可佐证），但短句仍可能无尾标点——保持实测判据不变是最安全的。

### B5 · 音频格式与采样率

**出处**：[选型 - 音频规格](https://help.aliyun.com/zh/model-studio/asr-model)

| 项 | 支持 |
|----|------|
| 格式 | pcm / wav / mp3 / opus / speex / aac / amr |
| 采样率 | **任意**（8k模型仅8000，其他任意） |
| 声道 | **单声道** |
| 时长 | **不限** |

**我们现状**：PCM 16kHz 单声道（`f32_to_pcm16_le` 输出）。**直接兼容，无需转换**。

注意：opus/speex 必须 Ogg 封装；wav 必须 PCM 编码；amr 仅 AMR-NB。

---

## 三、商务层问答（C1–C3）

### C1 · 计费口径：按音频时长（秒），不是 token

**出处**：[服务端事件 - result-generated usage](https://help.aliyun.com/zh/model-studio/fun-asr-server-events) + [用户指南](https://help.aliyun.com/zh/model-studio/real-time-speech-recognition-user-guide)

**结论**：**按音频时长计费**（秒），不是 token。

证据：
1. `result-generated` 的 `usage.duration` 是「任务计费时长（秒）」——整数秒
2. 用户指南示例：`print('任务计费时长（秒）：', message['payload']['usage']['duration'])`
3. 定价页在「语音识别」分类下按时长计费（具体单价见控制台活动优惠，建议 Gavin 在百炼控制台查看实时单价）。

**对 VAD 门控的影响**：Gavin 要「本地先做 VAD，判断有有效语音输入才调 API，避免无效 token 浪费」。
- 计费口径是**时长**不是 token → 措辞应改为「避免无效**时长**浪费」
- VAD 门控的价值：**过滤纯静音/噪音段**，只把有语音的音频发给服务端，按实际语音时长计费
- **但**：服务端自己也有 VAD（`max_sentence_silence`），如果我们发的全是静音，服务端 VAD 不会产出句子 → 可能不计费或计费极短。需要实测确认。
- **保守结论**：本地 VAD 门控的价值主要在**省请求开销 + 降低延迟**（不发无意义数据 + 不等服务端 VAD 超时），而不是直接省钱。省钱效果取决于服务端是否对「发了但没识别出内容」的音频计费。

### C2 · 限流 / 并发限制

**出处**：文档未在抓取的页面中明确列出实时 ASR 的并发限制。

**文档未覆盖**。建议在百炼控制台查看或咨询阿里云客服。本项目是单用户单录音场景，并发=1，限流风险极低。

### C3 · GA 状态 / region / API key

**出处**：[选型 - 推荐模型](https://help.aliyun.com/zh/model-studio/asr-model) + [提升准确率 - 支持的模型与地域](https://help.aliyun.com/zh/model-studio/improve-asr-accuracy)

- **GA 状态**：已 GA（出现在「推荐模型」表，无 beta/preview 标注）
- **Region**：华北2（北京）+ 新加坡
- **API Key**：**北京与新加坡的 API Key 不同**（文档多次强调）。现有项目用的是北京 region 的 key（`llm-kudx4dj2bfqn4gr2.cn-beijing`），**同一个 API Key 可用**（都是百炼平台 key，inference 端点鉴权方式与 realtime 相同，都是 `Authorization: Bearer <key>`）。

---

## 四、整合方案

### 4.1 新写模块 vs 改造 `qwen3_online.rs`

**结论：新写模块 `qwen_inference.rs`，保留 `qwen3_online.rs` 作为回退选项（不删除）。**

理由：
1. 两个协议的事件名、消息结构、音频发送方式（base64→binary）、鉴权 header 全部不同，无法共用主体逻辑。
2. 可复用的纯函数只有 `f32_to_pcm16_le`（PCM 编码）——可以 `pub use` 导出给新模块复用。
3. **保留旧模块**的理由：DEC-028 确立的「不自动降级本地」原则下，如果新模型服务不可用，用户可以手动切回 `qwen3_online`（旧模型仍可用）。两个协议共存于不同 `asr_model` 值即可。

**模块设计**：
```
src/transcription/
  mod.rs          — AsrModel 新增 QwenAudioOnline 变体
  qwen3_online.rs — 保留不动（旧 Realtime API）
  qwen_inference.rs — 新建（Inference API，二进制帧 + run-task/finish-task）
  vad.rs          — 不动（VAD 门控复用）
```

**`AsrModel` 枚举变化**：
```rust
pub enum AsrModel {
    Performance,
    Accuracy,
    Qwen3Online,       // 旧：qwen3-asr-flash-realtime（Realtime API）
    QwenAudioOnline,   // 新：qwen-audio-3.0-asr-flash-streaming（Inference API）
}
```
`from_config` 新增 `"qwen_audio_online"` → `QwenAudioOnline`。

### 4.2 VAD 门控接在哪一层

**结论：接在 `transcribe_with_punct_info` 的 QwenAudioOnline 分支入口，在调 API 之前。**

```
录音完成 → samples(f32, 16kHz)
  → [VAD 门控] VadSegmenter.has_speech(samples)?
      → false: bail("no effective speech detected")  // 不调 API
      → true:  transcribe_inference(samples)          // 调 API
```

**与现有 `VadSegmenter` 的关系**：
- `VadSegmenter`（`vad.rs`）是**分段器**（把长音频切成 ≤20s 段），不是**有无语音检测器**。
- 需要新增一个**轻量函数** `has_effective_speech(samples) -> bool`，复用 `VadSegmenter` 的 detector：
  - `segment(samples)` 返回空 Vec → 无语音
  - 返回非空 → 有语音
- **或**更简单：直接用 `VadSegmenter::segment`，`is_empty()` 判定。

**与 `speech_detected` 的关系**（`audio/mod.rs:650`）：
- `speech_detected` 是**逐 chunk RMS 能量阈值**（`rms > silence_threshold`），判据很粗（`silence_threshold=0.01`）。
- 它在录音**过程中**置位，用于判断「是否该结束录音」（静音超时）。
- VAD 门控在录音**结束后**做二次确认，两者不重复：
  - `speech_detected` 决定**何时停录**
  - VAD 门控决定**录完的音频要不要发 API**
- **回归面评估**：VAD 门控是新行为，只影响 QwenAudioOnline 分支。如果 VAD 模型缺失（`VadSegmenter::try_new` 返回 None），降级为「总是调 API」（与现状一致，不阻塞）。

**门控逻辑**：
```rust
// 伪代码（QwenAudioOnline 分支）
if let Some(vad) = &self.vad_segmenter {
    let segs = vad.lock().unwrap().segment(samples);
    if segs.is_empty() {
        log::info!("VAD gate: no effective speech, skipping API call");
        anyhow::bail!("ASR transcription skipped: no effective speech detected");
    }
    // 有语音 → 调 API（用完整 samples，不分段——在线模型不限时长）
    let text = qwen_inference::transcribe_inference(..., samples, ...)?;
} else {
    // VAD 模型缺失 → 降级，总是调 API
    let text = qwen_inference::transcribe_inference(..., samples, ...)?;
}
```

**注意**：在线模型**不限音频时长**（选型表标注「无限制」），所以**不需要分段**——VAD 只做「有无语音」的 gate，不做切分。把完整 `samples` 一次性发给服务端。

**VAD segmenter 的初始化**：
- 现有代码只在 `Accuracy` 模式初始化 `vad_segmenter`（`transcription/mod.rs:142`）。
- 新方案：`QwenAudioOnline` 也初始化 VAD（复用同一 `VadSegmenter`，模型文件 `models/silero-vad/silero_vad.onnx` 已存在）。
- 或者在 QwenAudioOnline 分支按需 lazy init——但 Accuracy 已有的初始化逻辑更简单，直接在 `Transcriber::new` 里对 `QwenAudioOnline` 也初始化即可。

### 4.3 热词链路：wordbook → ASR vocabulary

**完整路径**：
```
src/wordbook/mod.rs  Wordbook::list_all()  → Vec<WordEntry>
    ↓
src/transcription/mod.rs  Transcriber 字段存最新词表快照
    ↓
qwen_inference.rs  build_run_task(..., vocabulary: &HashMap<String, i32>)
    ↓
run-task payload.parameters.vocabulary = {"词1": 4, "词2": 4, ...}
```

**设计要点**：

1. **词表快照**：Transcriber 已有 `hotwords_version`（u64，词表内容哈希）用于感知变更。新增 `hotwords_snapshot: Vec<String>` 字段，在 `Transcriber::new` 和 config reload 时更新。

2. **精选规则**（复用现有）：项目已有精选规则——上限 20 词、≤10 字、ASCII 短词过滤（DEC-029 §5）。这些规则原本为 accuracy hotwords 设计，对即时热词同样适用。但**即时热词上限 2000**，远大于 accuracy 的 20——可以考虑放宽上限。**建议 v1 保持 20 词上限**（与现有行为一致），后续按需调整。

3. **权重**：统一 `weight=4`（官方推荐起始值）。

4. **格式转换**：
```rust
fn build_vocabulary(words: &[String]) -> serde_json::Value {
    let vocab: serde_json::Map<_> = words.iter()
        .map(|w| (w.clone(), serde_json::json!(4)))
        .collect();
    serde_json::Value::Object(vocab)
}
```

5. **文本长度过滤**：即时热词文本含非ASCII ≤15字符、纯ASCII ≤7片段。现有精选规则已有 ≤10字 过滤，兼容。超长的词跳过（log warn）。

6. **同步时机**：**每次录音请求都带上当前词表快照**（即时热词随请求内联，无需预建/同步）。词表变更通过 config watcher 更新 `hotwords_snapshot`，下次请求自动用新词表。

7. **失效处理**：即时热词无需失效处理——词表变了下次请求自动用新的，不需要管旧词表的清理。

### 4.4 上下文链路

**v1 建议：不传上下文**（`input: {}`）。

理由：
1. 即时热词已覆盖主要痛点（专有词汇识别）。
2. 上下文需要「上一句识别结果」，但本项目是**单次录音→注入**模式，不是连续对话。用户不会在录音间保持上下文。
3. 上下文 400 字符限制较紧，管理成本高。
4. Gavin 的需求是「用上上下文以提升效果」，但没指定来源。**v1 先不上，待端测确认热词效果不足时再补上下文**。

**若后续需要上下文的链路设计**（备选，不在 v1 范围）：
- 来源：`main.rs` 维护一个 `last_transcription: Option<String>`（上一次注入的文本）
- 更新：每次录音注入后更新
- 传递：`transcribe_inference` 新增 `context: Option<&[(role, text)]>` 参数
- 格式：`[{"role":"user","content":[{"type":"input_text","text": last_transcription}]}]`
- 动态更新：`continue-task` 事件用于长录音中更新上下文（本项目录音短，用不到）
- 隐私：只传用户自己的识别结果，不传窗口标题/剪贴板等敏感信息

### 4.5 配置项设计（DEC-031 门禁核对）

**DEC-031 原则**：「格式化输出：单开关统一配置，不设独立开关」——这条约束的是「格式化输出」相关配置。ASR 模型选择属于「语音输入」配置域，不在 DEC-031 的约束范围内。但 DEC-031 的精神（「能零配置自适应就不暴露给用户」）应遵循。

**新增配置项清单**：

| 配置项 | 是否暴露给用户 | 理由 |
|--------|----------------|------|
| `audio.qwen_asr_url` | **否**（仅配置文件） | 与 `qwen3_asr_url` 同级，服务端 URL 不该让用户管 |
| `audio.qwen_asr_model` | **否**（仅配置文件） | 固定 `qwen-audio-3.0-asr-flash-streaming` |
| `audio.qwen_asr_api_key` | **是**（UI 可编辑） | 与 `qwen3_api_key` 同级，用户需填自己的 key |

**关键决策：复用还是新增 API Key 字段？**

两个方案：
- **方案 A：复用 `qwen3_api_key`**——两个在线模型用同一个 key（都是百炼平台 key，技术上可行）。
  - 优点：用户只需配一次 key
  - 缺点：字段名 `qwen3_api_key` 语义不准确（也用于 qwen-audio）
- **方案 B：新增 `qwen_asr_api_key`**——独立 key 字段。
  - 优点：语义清晰
  - 缺点：用户要填两次 key

**建议方案 A（复用）**：把 `qwen3_api_key` 当作「百炼在线 ASR 通用 key」，UI 上「API Key」输入框的 label 不变，只在模型下拉说明里注明。后续如需重命名，单独做 config migration。

**UI 改动**：
- Voice 页模型下拉新增选项「Qwen-Audio 3.0 在线语音识别服务」
- 下拉说明文字注明：支持热词、上下文、多语种方言
- API Key 输入框复用现有（`qwen3_api_key`）

**DEC-031 核对结论**：
- 不新增用户可见开关 ✅
- 不新增用户可见阈值 ✅
- 模型选择是已有的下拉（只是加一个选项），不是新增开关 ✅
- **不冲突**

### 4.6 影响文件清单

| 文件 | 改动类型 | 行号范围（估） | 说明 |
|------|----------|----------------|------|
| `src/transcription/mod.rs` | 改 | 30-48（AsrModel 枚举）、76-160（Transcriber 字段/构造）、251-290（transcribe_with_punct_info 路由） | 新增 QwenAudioOnline 变体 + 路由 |
| `src/transcription/qwen_inference.rs` | **新建** | ~400行 | Inference API 实现（run-task/二进制帧/finish-task/事件解析） |
| `src/transcription/qwen3_online.rs` | 不动 | — | 保留旧模块 |
| `src/transcription/vad.rs` | 不动 | — | VAD 门控复用现有 VadSegmenter |
| `src/config/mod.rs` | 改 | 136-201（AudioConfig）、字段默认值 | 新增 `qwen_asr_url`/`qwen_asr_model`，复用 `qwen3_api_key` |
| `src/main.rs` | 改 | 2213-2370（Transcriber 构造/reload） | 新增 QwenAudioOnline 路径 |
| `src-tauri/src/config.rs` | 改 | 106-149（AudioConfig 镜像） | 同步新增字段 |
| `src-tauri/src/qwen3.rs` | 改 | 43-166（test_qwen3_asr_connection） | 新增 `test_qwen_asr_connection`（Inference API 连测） |
| `src-tauri/src/main.rs` | 改 | 89-91、206 | 注册新 command |
| `ui/src/pages/Voice.tsx` | 改 | 模型选择下拉 | 新增选项 + 说明文字 |
| `ui/src/i18n/*.ts` | 改 | — | 新增 i18n 字符串（三语） |

**跨平台影响**（DEC-033）：
- `src/transcription/`、`src/config/`、`src-tauri/src/config.rs` 均为**平台中立模块**
- macOS 侧管线（`mod macos_stubs`）目前是空壳，但 `AsrModel` 枚举和 `Transcriber` 结构体的字段变化会通过 `config/mod.rs` 影响两侧
- **风险**：新增 `QwenAudioOnline` 变体是 `AsrModel` 枚举扩展，macOS 侧 `from_config`/match 需同步——但这是 `#[cfg]` 门控不到的纯 Rust 枚举，两侧编译时都会看到。**只要 macOS 侧的 match 加了新变体（或用了 `_ =>` 兜底），就不会编译失败**。
- **结论**：macOS 侧需在消费 `AsrModel` 的 match 分支补 `QwenAudioOnline => ...`，属新增而非破坏，不违反 DEC-033 第4条。

### 4.7 风险与回退方案

| 风险 | 回退方案 |
|------|----------|
| 新模型服务不可用/超时 | 用户手动切回 `qwen3_online`（旧模型仍可用）或 `performance`（本地） |
| 鉴权失败（key 无效） | 报错提示，不自动降级（DEC-028 原则不变） |
| VAD 门控误判（有语音被判为无） | VAD 模型缺失时降级为「总是调 API」；门控只拦「VAD 确认无语音」，有语音段总是放行 |
| 二进制帧发送被 tungstenite 限制 | tungstenite 支持 `Message::Binary(Vec<u8>)`，无限制 |
| 即时热词超 2000 个 | 现有精选规则限 20 词，远低于上限 |
| WorkspaceId 与旧 API 不同 | 实测：同一个 key 同一个 workspace（`llm-kudx4dj2bfqn4gr2`），只是 URL 路径变 |

**超时策略**（沿用 DEC-028）：
- 连接超时 5s
- 静默超时 10s（finish-task 后服务端无响应）
- 硬上限 `max(30s, 音频时长×0.5)`
- fail-fast，不重试

### 4.8 分批实施建议

| 批次 | 内容 | 验收标准 | 文件域 |
|------|------|----------|--------|
| **ASR-035-A** | `qwen_inference.rs` 新建（纯函数 + transcribe_inference 主入口） + `AsrModel` 枚举扩展 + config 字段 | `cargo check` 0 error + 纯函数单测全绿 | `src/transcription/qwen_inference.rs`（新建）、`src/transcription/mod.rs`、`src/config/mod.rs` |
| **ASR-035-B** | VAD 门控接入 + wordbook 热词链路 + main.rs 路由 | `cargo test` 全量回归 + `cargo check --all-targets` 0 error | `src/transcription/mod.rs`、`src/main.rs`、`src/wordbook/`（只读） |
| **ASR-035-C** | Tauri config 镜像 + 连测 command + UI 下拉选项 + i18n | `npm run build` + Vitest + `cargo check --manifest-path src-tauri/Cargo.toml` 0 error | `src-tauri/src/`、`ui/src/` |
| **TEST-SYNC-035** | 测试同步 | tester-1 补单测 | 各 `mod tests` |
| **TEST-EXEC-035** | 全量回归 | tester-1 | — |
| **BUILD-035** | 出包 | tester-1 | — |

**建议 A→B 串行**（同文件域 `src/transcription/mod.rs`），C 可与 B 并行（不同文件域）。但 B 改 `main.rs`，C 不碰 `main.rs`，文件级零重叠。

---

## 五、本地录音降噪可行性研究（§八 追加需求）

### 5.1 现状取证（已由主控完成，直接引用）

| 步骤 | 位置 | 说明 |
|------|------|------|
| 增益归一化 | `audio/mod.rs:563-579` | peak → 0.8，倍数 clamp(1.0, 12.0)，实测增益 4.68x |
| 抗混叠重采样 | `audio/mod.rs:597-609` | 48kHz→16kHz，`resample_anti_alias`（FIRSTCHAR-FIX-005） |
| `speech_detected` | `audio/mod.rs:644-653` | **逐 chunk RMS 能量阈值**，不是 VAD |
| 降噪 | **无** | Cargo.toml 零降噪依赖 |

### 5.2 降噪与增益的先后（D4）

**结论：降噪应在增益前。**

理由：
1. 增益最高放大 12 倍，**噪声同比放大**。安静环境说话时增益拉满，底噪一起上去。
2. 先降噪再增益：降噪去除底噪 → 增益放大干净的语音信号 → 信噪比最优。
3. 先增益再降噪：增益放大噪声 → 降噪需要处理更高能量的噪声 → 效果可能更差（降噪模型对高能噪声的处理不如低能噪声稳定）。

**链路顺序**：`录音 → 降噪 → 增益归一化 → 抗混叠重采样 → VAD 门控 → 调 API`

### 5.3 D1 · Rust 生态实时降噪方案评估

| 方案 | 许可证 | 维护活跃度 | C 依赖 | Windows 构建 | macOS 构建 | 延迟 |
|------|--------|------------|--------|-------------|------------|------|
| **nnnoiseless** | BSD-3 | 低（最后发布 2022，但功能稳定） | **是**（捆绑 RNNoise C 库，通过 build.rs 编译） | ✅（需 C 编译器，项目已有 MSVC） | ✅（需 C 编译器，项目已有 clang） | ~10ms/frame（RNNoise 设计延迟） |
| **webrtc-audio-processing** | BSD-3 | 中（有 Rust binding crate） | **是**（WebRTC C++ 库，体积大） | ⚠️（WebRTC C++ 构建复杂，依赖多） | ⚠️（同上） | 低（实时设计） |
| **speexdsp** | BSD | 中 | 是（C 库） | ✅ | ✅ | 低 |
| **纯 Rust 重写 RNNoise** | — | 无现成 crate | 否 | ✅ | ✅ | — |

**nnnoiseless 评估**：
- `nnnoiseless` 是 RNNoise 的 Rust 封装，内部捆绑了 RNNoise 的 C 代码
- build.rs 自动编译 C 代码，需要 C 编译器（项目已有 MSVC/clang）
- 纯 C 编译，无 C++ 依赖，跨平台构建简单
- 延迟：RNNoise 设计为实时，每帧 ~10ms（512 samples @ 48kHz 或 320 samples @ 16kHz）
- API：`DenoiseState::new()` → `process(frame: &mut [f32])`，in-place 降噪

**webrtc-audio-processing 评估**：
- 包含 NS（降噪）+ AGC（自动增益）+ AEC（回声消除）
- C++ 库体积大，构建依赖多（abseil、protobuf 等）
- Rust binding crate 存在但维护活跃度中等
- **对本项目过重**——我们只需要 NS，不需要 AGC/AEC，引入整个 WebRTC 栈不划算

**推荐**：**nnnoiseless**（轻量、纯 C 编译、跨平台简单、满足实时需求）。

### 5.4 D2 · 延迟与 CPU 开销

- RNNoise 设计为实时低延迟，每帧处理 ~10ms（16kHz 下 320 samples/帧）
- CPU：单核 < 1%（RNNoise 用浅层 RNN，推理极快）
- **对首字延迟的影响**：降噪在录音**过程中**逐帧处理，不增加录音后到首字的延迟。增益/重采样是录音后一次性处理，降噪如放录音后做会增加 ~10ms × 帧数（1秒音频 ~100帧 × 10ms = 1s 额外延迟，**不可接受**）。
- **结论**：降噪**必须在录音过程中逐帧做**（streaming），不能在录音后做。这要求在 `audio/mod.rs` 的 `push_chunk` 里插入降噪步骤。

### 5.5 D3 · 降噪是否会伤 ASR 准确率

**关键问题**：RNNoise 针对**人耳感知**优化，可能削掉 ASR 依赖的高频细节。

**依据**：
1. RNNoise 论文（Jean-Marc Valin, 2018）明确目标是「perceptual quality」，训练数据基于人耳主观评分
2. ASR 模型（尤其是送气清声母 /pʰ//tʰ//s/）依赖 4–12kHz 高频能量（本项目已做抗混叠重采样保护这些频率）
3. 降噪模型可能在去噪时连带削掉高频语音成分 → ASR 准确率下降
4. **无明确论文实测 RNNoise 对 ASR 的影响**——但业界经验是「降噪是双刃剑」，强降噪通常伤 ASR

**本项目特殊风险**：FIRSTCHAR-FIX-005 刚修好送气声母问题（抗混叠重采样），如果降噪削掉高频，可能**复发**。

**结论**：**有风险，需实测**。不能假设降噪对 ASR 无害。

### 5.6 D5 · 降噪与 VAD 门控的协同

**顺序**：`降噪 → VAD → 判有效 → 调 API`

- 降噪在录音过程中逐帧做（streaming）
- VAD 门控在录音结束后做（用降噪后的完整 samples）
- 降噪后的音频信噪比更高，VAD 判定更准（减少「噪音被误判为语音」）
- 如果降噪放 VAD 后：VAD 在含噪音频上判定，可能误判 → 不合理

**但**：如果服务端自带降噪（见 D6），本地降噪后送 VAD 再送 API，可能**重复降噪**（服务端再降噪一次）。这不一定是问题——两次轻降噪比一次强降噪更安全。

### 5.7 D6 · 服务端自带降噪能力

**出处**：[客户端事件 - speech_noise_threshold](https://help.aliyun.com/zh/model-studio/fun-asr-client-events)

**发现**：新 API 有 `speech_noise_threshold` 参数（[-1.0, 1.0]）：
> 「语音与噪音的判定阈值，用于调整语音活动检测（VAD）的灵敏度。取值越接近 +1：语音被误判为噪音的概率增大；越接近 -1：噪音被识别为语音的概率增大」

这是**服务端 VAD 的噪音阈值**，不是独立的降噪模块。但它说明服务端**有噪音处理能力**（至少在 VAD 层面）。

**进一步**：用户指南概述提到「具备应对复杂声学环境的能力，支持自动语种检测与智能非人声过滤」——「智能非人声过滤」可能就是服务端降噪/去非人声。

**结论**：**服务端可能已有降噪**。如果服务端降噪足够好，本地降噪可能**整个不必要**。但文档没有明确说「服务端做了 RNNoise 级降噪」，只说「非人声过滤」，程度不明。

### 5.8 D7 · speech_detected 是否应被 Silero VAD 取代

**现状**：`speech_detected`（`audio/mod.rs:650`）是逐 chunk RMS 能量阈值（`rms > silence_threshold`），用于决定「何时结束录音」（静音超时）。

**替换为 Silero VAD 的影响**：
- Silero VAD 更准（基于模型，不只是能量）——能区分「人声」和「噪音」，RMS 不能
- 但 Silero VAD 有 ~32ms 延迟（512 samples window），RMS 是即时的
- **行为变更**：替换后静音检测的灵敏度/延迟都会变，影响「何时停录」——这是 Gavin 端测过的行为，属回归风险
- **回归面**：所有使用该录音模式的应用（全部场景），因为 `speech_detected` 在 `push_chunk` 里，影响每一条录音

**结论**：**不建议替换**。`speech_detected` 用于「停录」、Silero VAD 用于「API 门控」，两者职责不同，应共存。

### 5.9 降噪方案建议：三选一

**建议：A（不做本地降噪）——但附条件。**

理由：
1. **D3 风险真实**：RNNoise 伤 ASR 高频有业界经验支撑，本项目刚修好送气声母问题，不宜冒复发风险
2. **D6 服务端可能已处理**：新 API 有 `speech_noise_threshold` + 文档称「智能非人声过滤」，服务端降噪能力不明但存在
3. **D2 延迟约束**：降噪必须在录音中逐帧做，改动 `audio/mod.rs` 录音链路，影响面大
4. **收益不确定**：Gavin 的诉求是「省 token（实为时长）」，降噪对此帮助有限——静音段服务端 VAD 不产出句子，可能不计费

**附条件**：
- 端测时用 `-debug` 模式观察新 API 的 `usage.duration` 与音频实际时长的关系，确认服务端是否对静音段计费
- 如果服务端对静音段**计费** → 本地 VAD 门控已足够（过滤纯静音），不需要降噪
- 如果服务端对静音段**不计费** → 降噪更无必要

**若端测发现确实需要降噪（方案 B 的触发条件）**：
- 用 **nnnoiseless**
- 放在 `audio/mod.rs` 的 `push_chunk` 里（逐帧降噪，录音过程中做）
- 顺序：`录音 → 逐帧降噪 → 增益归一化 → 抗混叠重采样 → VAD 门控 → 调 API`
- 降噪开关**不暴露给用户**（DEC-031 精神：零配置自适应），内部默认开启或关闭
- 需先做最小实验：录几段含噪音频，对比「降噪前/后」的 ASR 识别结果（尤其是送气声母词），确认无损再上线

**方案 C（先实验）不是独立建议**——方案 A 的「附条件」已包含「端测确认」这一实验步骤，无需单独开实验批次。

---

## 六、版本号建议

Gavin 已指示升级版本号但未指定目标号。当前版本 `0.7.3`。

建议：`0.8.0`（minor bump，新功能——新 ASR 模型 + 热词 + VAD 门控，属功能增量非破坏性变更）。

**但最终由 Gavin 拍板**，本方案不自行决定。

---

## 七、未解问题（需 Gavin 拍板或实测确认）

| # | 问题 | 处置 |
|---|------|------|
| 1 | 服务端是否对静音段计费？ | 端测时观察 `usage.duration` |
| 2 | 上下文 context v1 是否上线？ | 建议 v1 不上，待热词效果不足时补 |
| 3 | API Key 复用 `qwen3_api_key` 还是新增字段？ | 建议复用，但需 Gavin 确认 |
| 4 | 版本号目标？ | Gavin 指定 |
| 5 | 即时热词上限保持 20 还是放宽到 2000？ | v1 保持 20，后续按需 |
| 6 | 降噪是否需要？ | 方案 A（不做），端测后视情况转 B |
| 7 | `qwen3_asr_url` 里的 WorkspaceId 是否需要用户手动改？ | 不需要——现有 URL 已含 WorkspaceId，只需把 `/realtime` 改成 `/inference` |

---

## 八、出处链接汇总

| 内容 | 链接 |
|------|------|
| WebSocket API（接口地址/请求头/交互流程） | https://help.aliyun.com/zh/model-studio/fun-asr-realtime-websocket-api |
| 客户端事件（run-task/continue-task/finish-task） | https://help.aliyun.com/zh/model-studio/fun-asr-client-events |
| 服务端事件（task-started/result-generated/task-finished/task-failed） | https://help.aliyun.com/zh/model-studio/fun-asr-server-events |
| 用户指南（示例代码/VAD配置/热词/时间戳/敏感词） | https://help.aliyun.com/zh/model-studio/real-time-speech-recognition-user-guide |
| 提升识别准确率（预编译热词/即时热词/上下文增强） | https://help.aliyun.com/zh/model-studio/improve-asr-accuracy |
| 选型（支持语言/音频规格/模型对比） | https://help.aliyun.com/zh/model-studio/asr-model |
| 模型调用价格 | https://help.aliyun.com/zh/model-studio/billing-for-model-studio |
| 获取 Workspace ID | https://help.aliyun.com/zh/model-studio/obtain-the-app-id-and-workspace-id |

---

> **本文件为零代码改动研究产物**。`git diff --ignore-cr-at-eol --numstat -- src/` == 0。