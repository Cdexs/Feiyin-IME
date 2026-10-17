# 飞音识别服务 · 终端接入协议 v1

> 给**另起项目的终端**（安卓 / iOS / 第三方程序）照着对接的契约。只描述线上能看到的行为，不依赖本仓库任何源码。
> 本文与网关实现（`src/bin/feiyin-asr-gateway/`）逐条对应；两者不一致时以网关行为为准，并回来改本文。
> 参考实现：`tools/asr-client/feiyin_asr_client.py`（Python，可直接跑通全流程，见 §9）。
> 版本：v1（2026-10-17，ASR-SERVER-491 一期）。

## 0. 一句话

**照阿里云百炼实时识别的 `run-task` WebSocket 协议发**（同名事件、同名字段），差别只有四处：
① 地址、key、证书指纹来自**配对**（§1）；② 走 `wss://` 且**只认配对时的证书指纹**（§2）；
③ `run-task` 里必须带本机身份 `parameters.feiyin.device_id / device_name`（§4.1）；
④ 一期整段一句：`sentence_id` 恒为 1，`text` 每次给**至今全文**（§5.2）。

---

## 1. 配对：拿到地址、key、指纹

服务端（飞音输入法所在电脑）在设置页「添加终端」后给出三种等价形式：二维码、连接码（二维码的文字版）、分开显示的地址 + key。

### 1.1 连接码 / 二维码内容

```
feiyin-asr://pair?v=1&lan=192.168.1.5:7860&wan=asr.example.com:7860&key=ABCDEFGHIJKLMNOPQRSTUVWXYZ&fp=3f9a…（64 位十六进制）
```

| 字段 | 必有 | 说明 |
| --- | --- | --- |
| `v` | ✅ | 协议版本，当前只认 `1`；不是 `1` ⇒ 拒绝并提示升级 |
| `lan` | 二选一 | 内网地址 `主机:端口` |
| `wan` | 二选一 | 外网地址 `主机:端口`（服务端没配外网时不出现） |
| `key` | ✅ | 终端凭证：RFC 4648 base32（大写、无填充），16 字节随机数 ⇒ 26 字符 |
| `fp` | ✅ | 服务端证书指纹：证书 DER 的 SHA-256，**小写**十六进制 64 字符 |

- 值可能被百分号编码，解析时**先解码、再按 §1.2 规整 key**。**不认识的字段忽略**（以后加字段不破坏老终端）；同名字段重复出现时取最后一个。
- 指纹比对时宽容处理：去掉空白和冒号、转小写后再比（`3F:9A:…` 与 `3f9a…` 视为相同）。
- 地址没写端口 ⇒ 补 `7860`；IPv6 主机须带方括号（`[fe80::1]:7860`）。

### 1.2 手输

用户分别输入内网 / 外网地址（至少一个）与 key；指纹可空。地址规则同 §1.1（缺端口补 7860、IPv6 带方括号）。
- key 规整：**去掉所有空白和 `-`、转大写**后再用（服务端也按同样规则比对）。
- 指纹为空 ⇒ 先只做 TLS 握手取回服务端证书指纹（不发 key），显示给用户核对（服务端设置页有同一串指纹），用户确认后才保存。**未经确认的指纹不得保存、不得带 key 连接。**

### 1.3 key 的生命周期（终端需要知道的部分）

- 新 key **10 分钟内**必须完成首次识别请求（§4.1），否则过期（`KeyExpired`）⇒ 让用户重新添加终端。
- 首次识别请求时 key **绑定本机 `device_id`**；之后同一 key 只能由这台设备使用（换设备 ⇒ `DeviceMismatch`）。
- 服务端可随时撤销（`KeyRevoked`），撤销时在线连接立即断开。
- 以上三种错误终端都应提示「需要重新配对」，**不要自动重试**。

### 1.4 本机身份

| 字段 | 规则 |
| --- | --- |
| `device_id` | 终端第一次配对时随机生成（建议 16 位字母数字），**之后永久不变**（重新配对也沿用），存本地 |
| `device_name` | 给服务端用户看的名字（如「Gavin 的 iPhone」），可随时改；改了下次请求自动更新 |

两者都不能为空（去首尾空白后）。

---

## 2. 连接

### 2.1 选地址（内网 / 外网）

终端提供「自动 / 内网 / 外网」三选一，默认自动。**自动的建议做法**：两个地址同时试 TCP 连接（不带 key）；内网通了直接用内网；外网先通 ⇒ 再等内网 300ms，内网仍不通才用外网。每次开始识别前重新挑（进出家门自动切换）。

### 2.2 TLS（必须先验指纹，再发 key）

- 只用 **TLS 1.3**。服务端是自签证书：**不做 CA 校验、不校验主机名**，改为**钉扎指纹**——握手完成后取服务端证书 DER，算 SHA-256，与配对时的 `fp` 比较（忽略大小写），不一致 ⇒ 立即断开，提示「服务端证书变了，需要重新配对」。
- 🔴 **顺序硬要求**：指纹比对通过之后才能发 WebSocket 升级请求。key 在升级请求头里，先发后验等于把 key 交给中间人。

### 2.3 WebSocket 升级

```
GET wss://<主机:端口>/ HTTP/1.1
Authorization: Bearer <key>
```

路径不限（服务端不看路径，建议用 `/`）。key 用 §1.2 规整后的形式。被拒时正文是纯文本错误码（ASCII，很短；有 `Content-Length` 按它读，没有就读到连接关闭）。

| 升级结果 | 含义 | 终端处理 |
| --- | --- | --- |
| `101` | 成功 | 进入 §3 |
| `401`，正文 `InvalidKey` / `KeyExpired` / `KeyRevoked` | key 不对 / 过期 / 撤销 | 提示重新配对，不重试 |
| `503`，正文 `ServicePaused` | 服务端暂停接入 | 提示「对方已暂停」，稍后再试 |
| TCP 直接被断 / 握手无响应 | 同一 IP **连续 5 次**鉴权失败 ⇒ 锁 **10 分钟**（锁定期内连 TLS 都不做）；或网络不通 | 不要循环重连 |

握手（TLS + 升级）须在 **10 秒**内完成。

---

## 3. 连接上的规则

| 项 | 值 |
| --- | --- |
| 同一连接可依次跑多段识别 | ✅，但**同一时刻只能一段**（上一段收到 `task-finished` / `task-failed` 后才能发下一个 `run-task`） |
| 空闲断开 | **60 秒**收不到终端任何消息（含音频、Ping）⇒ 服务端断开。长时间挂着不说话的终端请每 ≤30 秒发一次 WebSocket Ping。识别进行中持续发音频即算活动，不必另发；`finish-task` 之后等终稿通常只需数秒 |
| 单条消息上限 | 1 MB |
| 服务端同时在线连接上限 | 64 |
| 服务端主动断开 | 撤销、鉴权失败、空闲超时、服务端退出 |

---

## 4. 终端 → 服务端

所有文本消息都是 JSON：`{"header": {...}, "payload": {...}}`。

### 4.1 `run-task`（开始一段识别）

```json
{
  "header": { "action": "run-task", "task_id": "8c1f…（每段新生成，建议 32 位十六进制）", "streaming": "duplex" },
  "payload": {
    "task_group": "audio", "task": "asr", "function": "recognition",
    "model": "feiyin-local",
    "input": {},
    "parameters": {
      "format": "pcm",
      "sample_rate": 16000,
      "inverse_text_normalization_enabled": true,
      "punctuation_prediction_enabled": true,
      "translation_enabled": false,
      "vocabulary": ["飞音", "LiveKit"],
      "feiyin": { "device_id": "a1b2c3d4e5f6g7h8", "device_name": "Gavin 的 iPhone" }
    }
  }
}
```

| 字段 | 必填 | 缺省 | 说明 |
| --- | --- | --- | --- |
| `header.task_id` | ✅ | — | 空 ⇒ `InvalidParameter`。后续 `finish-task` 与所有事件都带它 |
| `parameters.format` | | `pcm` | 一期**只收** `pcm`（大小写不限），其它 ⇒ `UnsupportedFormat` |
| `parameters.sample_rate` | | `16000` | 一期**只收** 16000，其它 ⇒ `UnsupportedFormat` |
| `parameters.inverse_text_normalization_enabled` | | `true` | 数字规整（「三点五公斤」⇒「3.5公斤」） |
| `parameters.punctuation_prediction_enabled` | | `true` | 补标点；`false` ⇒ 预览、终稿、译文**全部去标点** |
| `parameters.translation_enabled` | | `false` | 终稿另给一份译文（§5.3）。方向按内容自动：含汉字 ⇒ 译成英文，否则译成中文；`translation_target_languages` 忽略 |
| `parameters.vocabulary` | | 无 | 热词：字符串数组，或百炼的 `{"词": 权重}` 对象（权重忽略）。追加在服务端本机词库之后 |
| `parameters.feiyin.device_id` / `device_name` | ✅ | — | §1.4。缺 ⇒ `MissingDeviceInfo` 并断开 |
| `header.streaming`、`payload.task_group / task / function / model / input` | | — | 服务端忽略，照百炼写即可（方便同一份代码对接百炼） |

**固定内置、不可关**：同音字纠错、去口水词与口吃重复、无效文本判定（只有「嗯啊」之类 ⇒ 终稿为空）、繁简（跟服务端本机设置）。

成功 ⇒ 服务端回 `task-started`（§5.1），**之后**才能发音频。失败 ⇒ `task-failed`（§5.4）；其中 `KeyRevoked` / `KeyExpired` / `InvalidKey` / `DeviceMismatch` / `MissingDeviceInfo` 之后服务端**断开连接**。

### 4.2 音频：二进制帧

- **16 kHz、单声道、16 位有符号小端 PCM**，裸数据（无 WAV 头）。
- 建议每帧 **100 ms**（3200 字节），实时节奏发送。帧长不必固定，但单帧 ≤ 1 MB。
- `task-started` 之前或没有进行中的识别时发的音频**被丢弃**。

### 4.3 `finish-task`（说完了）

```json
{ "header": { "action": "finish-task", "task_id": "<同 run-task>", "streaming": "duplex" }, "payload": { "input": {} } }
```

`task_id` 对不上 / 没有进行中的识别 ⇒ `task-failed`（`InvalidParameter`，`no such task`），连接保持。发完后继续收事件，直到 `task-finished` / `task-failed`。

### 4.4 取消

一期没有取消消息：**直接关闭连接**即取消（服务端丢弃该段，不出终稿）。

---

## 5. 服务端 → 终端

事件统一形如 `{"header": {"task_id", "event", "attributes": {}}, "payload": {...}}`。

### 5.1 `task-started`

`payload` 为空对象。收到后开始发音频。

### 5.2 `result-generated`

```json
{
  "header": { "task_id": "8c1f…", "event": "result-generated", "attributes": {} },
  "payload": {
    "output": {
      "sentence": {
        "sentence_id": 1,
        "begin_time": 120,
        "end_time": 2380,
        "text": "今天买了3.5公斤苹果。",
        "sentence_end": false,
        "words": [
          { "begin_time": 120, "end_time": 400, "text": "今", "punctuation": "" },
          { "begin_time": 2100, "end_time": 2380, "text": "果", "punctuation": "。" }
        ]
      },
      "feiyin": { "tentative": 3, "ts_source": ["aligned", "rough"] }
    },
    "usage": null
  }
}
```

**怎么用**：

| 字段 | 语义 |
| --- | --- |
| `sentence_id` | 一期恒为 1（整段一句） |
| `text` | **本段识别至今的全文**。每条预览都**整体替换**上一条，不要拼接 |
| `sentence_end` | `false` = 预览，后面还会变；`true` = **终稿**，本段只出现一次，之后不再变 |
| `feiyin.tentative` | 预览里 `text` **末尾**还没稳定的字数（建议浅色显示）；终稿恒 0 |
| `words` | 逐词时间：`begin_time` / `end_time` 为毫秒，**相对本段第一帧音频**。中文一字一词，连写的英文 / 数字合成一词；标点挂在前一个词的 `punctuation` 上。保证：所有词 `text + punctuation` 依次拼接 == `sentence.text`（空格也挂在词上，逐字符相等）。预览里的时间是粗略的，可能为空数组 |
| `begin_time` / `end_time`（句） | 首词起点 / 末词终点；没有词时 `begin_time` 为 0、`end_time` 为 `null` |
| `feiyin.ts_source` | 与 `words` 一一对应的时间来源：`aligned`（强制对齐，准）/ `rough`（识别模型自带，粗）/ `estimated`（插值估算）。终稿里每个词都有时间，补不出的才是 `estimated` |

> 阿里客户端只认 `output.sentence` 的终端可直接用；`output.feiyin` 是扩展字段，可忽略。
> 终稿可能是**空文本**（没听到有效语音 / 只有口水词）。

### 5.3 译文（只在终稿、且 `translation_enabled = true` 时）

终稿的 `payload.output` 里多一个（阿里 Gummy 同形）：

```json
"translations": [ { "sentence_id": 1, "lang": "en", "text": "I bought 3.5 kg of apples today.", "sentence_end": true } ]
```

- `lang`：`en` 或 `zh`。译文不带逐词时间。
- 引擎由服务端决定（服务端配了大模型用大模型，否则用离线翻译模型；首次离线翻译要加载模型，可能多等几秒）。
- 翻译失败 ⇒ **没有** `translations` 键，原文照常。

### 5.4 `task-finished` / `task-failed`

```json
{ "header": { "task_id": "8c1f…", "event": "task-finished", "attributes": {} },
  "payload": { "output": {}, "usage": { "duration": 5 } } }
```

`usage.duration` = 本段音频秒数（向上取整）。之后可在同一连接发下一个 `run-task`。

```json
{ "header": { "task_id": "8c1f…", "event": "task-failed", "error_code": "ServiceBusy", "error_message": "capacity", "attributes": {} },
  "payload": {} }
```

| `error_code` | 何时 | 连接 | 终端提示建议 |
| --- | --- | --- | --- |
| `InvalidParameter` | JSON 坏、未知 `action`、缺 `task_id`、已有进行中的识别、`finish-task` 对不上 | 保持 | 程序错误，记日志 |
| `UnsupportedFormat` | 非 pcm / 非 16000Hz | 保持 | 程序错误 |
| `MissingDeviceInfo` | 缺 `device_id` / `device_name` | **断开** | 程序错误 |
| `InvalidKey` / `KeyExpired` / `KeyRevoked` / `DeviceMismatch` | 见 §1.3 | **断开** | 需要重新配对 |
| `ServicePaused` | 服务端暂停接入 | 保持 | 对方已暂停，稍后再试 |
| `ServiceBusy` | 服务端推理通道已满 | 保持 | 对方正忙，稍后再试 |
| `ModelNotReady` | 服务端电脑没切到「本地流式实时」识别模型 | 保持 | 对方识别模型未就绪 |
| `InternalError` | 其它 | 保持 | 识别失败，可重试 |

进行中的识别遇到撤销：先收到 `task-failed`（`KeyRevoked`）再断开。

---

## 6. 一段识别的完整时序

```
终端                                   服务端
 │ TCP + TLS1.3 ───────────────────────▶│
 │ （本地比对证书指纹，通过才继续）       │
 │ GET / + Authorization: Bearer key ──▶│ 401/503 ⇒ 结束
 │◀──────────────────────────── 101     │
 │ run-task ───────────────────────────▶│
 │◀──────────────────────── task-started│
 │ 音频 100ms ─────────────────────────▶│
 │ 音频 100ms … ───────────────────────▶│
 │◀── result-generated（预览，反复多条） │
 │ finish-task ────────────────────────▶│
 │◀── result-generated（sentence_end=true，终稿；可含 translations）
 │◀─────────────────────── task-finished│
 │ （可再发 run-task，或关闭连接）        │
```

---

## 7. 与阿里百炼的差异速查

| 项 | 百炼 | 飞音 v1 |
| --- | --- | --- |
| 鉴权 | `Authorization: bearer <API Key>` | 同写法，key 来自配对；另须钉扎证书指纹 |
| 终端身份 | 无 | `parameters.feiyin.device_id / device_name` 必填 |
| 音频格式 | pcm / opus / wav… 多采样率 | 一期只 pcm 16k |
| 句子 | 多句，每句一个 `sentence_id` | 一期整段一句，`text` 为全文 |
| 心跳 | `heartbeat` 参数 | 一期没有；用 WebSocket Ping 保活（§3） |
| 扩展 | — | `output.feiyin.tentative / ts_source` |

## 8. 二期预告（未实现，终端不要依赖）

Opus 音频、断线续传、`heartbeat` 保活包、手机走 LiveKit（WebRTC）音频。届时协议版本升级并保持 v1 兼容。

## 9. 参考客户端

`tools/asr-client/feiyin_asr_client.py`：解析连接码 → 选地址 → TLS 钉扎 → 升级 → `run-task` → 按实时节奏发 WAV / PCM → 打印预览与终稿 → `task-finished`。用法见脚本头部说明。它同时是服务端的端到端冒烟测试。
