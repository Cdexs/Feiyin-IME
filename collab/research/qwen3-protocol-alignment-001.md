# Qwen3 在线 ASR 协议对齐研究（RESEARCH-QWEN3-PROTOCOL-001）

> 2026-07-07 Orchestrator 亲自研究。Gavin 指令：仔细研究官方示例，确认接口调用最优，避免影响识别效果。
> 资料来源：阿里云 Model Studio 官方文档（qwen-asr-realtime-interaction-process / qwen-asr-realtime-client-events / real-time-speech-recognition-user-guide）

## 逐项比对：官方 schema vs 当前实现（qwen3_online.rs）

| # | 项 | 官方 | 当前实现 | 判定 |
|---|---|---|---|---|
| 1 | URL + model 查询参数 | `wss://.../api-ws/v1/realtime?model=qwen3-asr-flash-realtime` | 一致 | ✅ |
| 2 | Headers | `Authorization: Bearer` + `OpenAI-Beta: realtime=v1` | 一致 | ✅ |
| 3 | 音频格式声明 | `"input_audio_format": "pcm"` + **独立** `"sample_rate": 16000` 两个字段 | `"input_audio_format": "pcm/16000"` 合并串 | ❌ **P0 必修**：非法值。目前碰巧能用是因为默认值恰为 pcm/16000，服务端忽略非法字段回落默认——不可依赖 |
| 4 | 语言指定 | `"input_audio_transcription": {"language": "zh"}` | `{"model": model_id}`（无 language） | ❌ **P0 必修**：缺 language 直接影响识别准确率（中文场景必须传 "zh"）；`model` 不是 session 文档字段（model 已在 URL query），应移除 |
| 5 | turn_detection 手动模式 | `"turn_detection": null` | 一致 | ✅ |
| 6 | modalities | 官方示例带 `"modalities": ["text"]` | 未带 | 🟡 P2 对齐官方示例，低风险加上 |
| 7 | append/commit 事件 | `{"type":"input_audio_buffer.append","audio":b64}` / `{"type":"input_audio_buffer.commit"}`，单事件上限 15 MiB | 一致（200ms/块，远小于上限） | ✅ |
| 8 | 分块节奏 | 官方示例 3200 bytes(100ms)+0.1s sleep——那是模拟实时麦克风流 | 6400 bytes(200ms) 全速发 | ✅ 批量手动模式无需模拟实时节奏；端测若现限流再议 |
| 9 | 转录结果事件 | 中间 `...transcription.text` → 最终 `...transcription.completed`，字段 `transcript` | 只取 completed.transcript，中间事件忽略 | ✅ 且中间事件天然刷新 10s read 超时 |
| 10 | 时间戳 | Qwen-ASR 不返回时间戳 | 未依赖 | ✅ |

## 准确率相关的额外发现

- **corpus.text 上下文偏置**（client-events 页记载：`input_audio_transcription.corpus.text`，max 10,000 tokens）——这是 qwen3 的"hotwords 等价物"。DEC-028 拍板 v1 不接词库，维持边界；但这是后续把 wordbook 热词接入在线模型的现成通道，记入未排期演进项 QWEN3-CORPUS-BIAS-001。注意：user-guide 页称无 corpus 选项，两页矛盾，实施前需以实测为准。
- language 省略行为：官方未明确说明缺省时是否自动检测；稳妥策略——config 为 zh/en 等明确值时必传；auto 时省略字段（交服务端检测）。

## 修正任务（R2，派 coder-1）

1. session.update 改为：`input_audio_format: "pcm"` + `sample_rate: 16000` + `modalities: ["text"]` + `input_audio_transcription: {language}`（asr_language 明确时传，auto 省略）+ `turn_detection: null`
2. 移除 session 内非文档 `model` 字段（model_id 仍用于 URL query）
3. build_session_update_message 签名改为接收 language 参数；单测同步
