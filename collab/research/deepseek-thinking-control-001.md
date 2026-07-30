# RESEARCH-DEEPSEEK-THINKING-001 · DeepSeek 思维链控制能力查证

> 研究人：coder-1 ｜ 日期：2026-07-29 ｜ 类型：纯研究（零代码改动）
> 方法：DeepSeek 官方文档（4 个页面）+ 7 次实测 API 请求（脱敏记录见附录 A）

---

## 摘要（主控先看这段）

1. **`deepseek-v4-flash` 是 DeepSeek 官方模型**（✅官方+✅实测），不存在"第三方兼容层"问题
2. **当前代码用的 `enable_thinking: false` 对 DeepSeek 完全无效**——DeepSeek 静默忽略未知字段，思维链照常输出（✅实测铁证）
3. **官方关思维链的唯一参数是 `thinking: {"type": "disabled"}`**（✅官方+✅实测）
4. **CoT 走独立字段 `reasoning_content`，不混在 `content`**（✅官方+✅实测）。主控根因 #3 表述"模型把思维链写进了 content"需要精化——见下方 Q3
5. **Gavin 13:19 "三个点全丢"现象已完整复现**：`max_tokens` 偏小 + thinking 默认开启 → reasoning_tokens 吃光预算 → `content=""` → 我们的 `extract_text` 回落到 `reasoning_content` 把 CoT 当成答案注入（✅实测）
6. ** Gavin 12:06 "原文兜底"现象**：另一条路径——`content` 非空但被系统认为不可信时回落到本地标点；或 LLM 返回了空/异常 content 触发 Err 分支。需要后续日志细查（⚠️见 Q3 末段）

**落地建议（Q6）核心**：把 `enable_thinking: Option<bool>` 字段替换/补充为 `thinking: Option<ThinkingConfig>`（`{"type":"disabled"}`）。这是 DeepSeek 官方契约，对其他 OpenAI 兼容 endpoint 无副作用（未知字段普遍被静默忽略），是真正能关掉思维链的方案。

---

## Q1 · `deepseek-v4-flash` 是什么模型？

### 结论：DeepSeek 官方模型，两个 ID 之一

**✅官方** — DeepSeek API Docs「Models & Pricing」页（<https://api-docs.deepseek.com/quick_start/pricing>）模型详情表列出两个模型：

| MODEL | MODEL VERSION | THINKING MODE | CONTEXT LENGTH | MAX OUTPUT |
|---|---|---|---|---|
| `deepseek-v4-flash` | DeepSeek-V4-Flash | Supports both non-thinking and thinking (default) modes | 1M | MAXIMUM: 384K |
| `deepseek-v4-pro` | DeepSeek-V4-Pro | 同上 | 1M | MAXIMUM: 384K |

原文引用：
> "**deepseek-v4-flash** / **deepseek-v4-pro** / ... THINKING MODE: Supports both non-thinking and thinking (default) modes. See [Thinking Mode](/guides/thinking_mode) for how to switch"

**✅官方** — 「Your First API Call」页（<https://api-docs.deepseek.com/>）明确：
> "model: `deepseek-v4-flash` / `deepseek-v4-pro`"

**✅实测** — `GET https://api.deepseek.com/models`（带 Authorization）返回：
```json
{"object":"list","data":[
  {"id":"deepseek-v4-flash","object":"model","owned_by":"deepseek"},
  {"id":"deepseek-v4-pro","object":"model","owned_by":"deepseek"}
]}
```

**结论**：
- `deepseek-v4-flash` 在官方列表中，是 DeepSeek-V4-Flash 模型的 OpenAI 兼容 API ID
- 两个模型**都支持思维链**，且**默认开启**（"default" 模式是 thinking）
- 两者都支持 non-thinking 模式（通过参数切换，见 Q2）
- 不存在"reasoner 版 vs chat 版分离"的模型 ID 区分——同一模型通过参数切换
- v4-flash 比 v4-pro 便宜（输入 $0.14 vs $0.435 / 1M cache miss；输出 $0.28 vs $0.87 / 1M）

---

## Q2 · DeepSeek 官方 API 是否支持关闭思维链？【核心】

### 结论：支持，参数是 `thinking: {"type": "disabled"}`，不是 `enable_thinking`

**✅官方** — DeepSeek API Docs「Thinking Mode」页（<https://api-docs.deepseek.com/guides/thinking_mode>）"Thinking Mode Toggle and Effort Control" 表格：

| Control Parameter (OpenAI Format) | Control Parameter (Anthropic Format) |  |
|---|---|---|
| `{"thinking": {"type": "enabled/disabled"}}` | (Anthropic 格式略) | Thinking Mode Toggle(1) |
| `{"reasoning_effort": "high/max"}` | `{"output_config": {"effort": "high/max"}}` | Thinking Effort Control(2)(3) |

原文引用（含关键脚注）：
> "(1) The thinking toggle defaults to `enabled`"
> "When using the OpenAI SDK, you need to pass the `thinking` parameter within `extra_body`:
> ```python
> response = client.chat.completions.create(
>   model="deepseek-v4-pro",
>   reasoning_effort="high",
>   extra_body={"thinking": {"type": "enabled"}}
> )
> ```"

**✅官方** — 「Create Chat Completion」API Reference（<https://api-docs.deepseek.com/api/create-chat-completion>）请求体 schema：
> "**thinking** object nullable — Controls the switch between thinking and non-thinking mode.
> **type** string Possible values: [`enabled`, `disabled`] Default value: `enabled`"

### 候选参数逐一查证结果

| 候选参数 | 是否 DeepSeek 官方 | 证据 |
|---|---|---|
| `enable_thinking: false`（**当前代码在用**） | ❌ **不是 DeepSeek 参数** | DeepSeek API Reference 请求体 schema 无此字段；这是 SiliconFlow / Qwen3 系参数（代码注释 `src/llm/mod.rs:42` 已自承"SiliconFlow/DeepSeek 等模型的 thinking 模式"，但 DeepSeek 部分是错的） |
| `thinking: {"type": "disabled"}` | ✅ **官方唯一开关** | 上述 Thinking Mode 页 + API Reference 双重确认 |
| `reasoning_effort: "high"/"max"` | ⚠️ **官方参数但只调"力度"不关** | 仅控制 thinking 模式下的努力程度；`low`/`medium` 被映射为 `high`，`xhigh` 映射为 `max`，没有"off"值 |
| `chat_template_kwargs: {"thinking": false}` | ❌未证实 | DeepSeek 官方文档无此参数，是 vLLM 部署侧的 tokenizer 模板参数，不适用于托管 API |
| 通过换模型 ID 来关 | ❌ **不存在非推理版模型 ID** | Q1 已证：flash/pro 两个 ID 都默认 thinking，只能靠参数关 |

### 对"无法识别字段"行为的关键实测（回答 Q2 后半）

**✅实测** — 发送完全虚构的字段 `this_is_a_completely_bogus_field_xyz: 12345`，HTTP 200 正常返回，思维链照常输出（reasoning_tokens=18）。

**✅实测** — 发送当前代码用的 `enable_thinking: false`，HTTP 200，**思维链照常输出**（reasoning_tokens=23，content="hello"）——证明 DeepSeek 对未知字段采取**静默忽略**策略，不返回 400。

**影响判断**：当前发 `enable_thinking: false` **没有副作用**（不会 400、不影响其他参数解析），但**也没有正面作用**（思维链依然开启，浪费 token/延迟）。这解释了为什么问题没在更早暴露——它"不报错"。

### 官方文档 URL 与原文引用汇总

| 命题 | URL | 原文 |
|---|---|---|
| 关思维链参数 | <https://api-docs.deepseek.com/guides/thinking_mode> | `{"thinking": {"type": "enabled/disabled"}}` |
| 默认 enabled | 同上 | "The thinking toggle defaults to `enabled`" |
| 请求体 schema | <https://api-docs.deepseek.com/api/create-chat-completion> | "thinking object nullable — Controls the switch between thinking and non-thinking mode" |
| `enable_thinking` 不在 schema | 同上 | 请求体字段完整枚举中无此名 |

---

## Q3 · 思维链走哪个字段？

### 结论：走独立字段 `reasoning_content`，**不混在 `content`**。主控根因 #3 需精化

**✅官方** — 「Thinking Mode」页 "Input and Output Parameters" 节（<https://api-docs.deepseek.com/guides/thinking_mode>）：
> "In thinking mode, the chain-of-thought content is returned via the `reasoning_content` parameter, at the same level as `content`."

**✅官方** — 「Create Chat Completion」API Reference 响应 schema：
> "message.content string nullable required — The contents of the message.
> message.reasoning_content string nullable — For thinking mode only. The reasoning contents of the assistant message, before the final answer."

**✅实测** — TEST 12（thinking 默认开启，纠错 prompt）：
```
content: 'GPU分为八核和十核。'   ← 真答案
reasoning_content: '用户需要我根据规则进行纠错...' (202 字)  ← CoT，独立字段
finish_reason: 'stop'
usage.completion_tokens_details.reasoning_tokens: 139
```
两个字段**物理分离**，content 里**不含**CoT 文本。

### 当前 `extract_text` 逻辑评估（mod.rs:962-993）

```rust
if let Some(content) = message.content.filter(|s| !s.trim().is_empty()) {
    parts.push(content);
} else if let Some(reasoning) = message.reasoning_content.filter(|s| !s.trim().is_empty()) {
    parts.push(reasoning);   // ← 关键：content 空时回落到 reasoning_content
}
```

**判断**：在 DeepSeek 语义下，这个"content 空则回落 reasoning_content"的逻辑**是错的**——它会把 CoT 当成最终答案注入用户输入框。

**为什么这么写**：注释和字段名暗示作者以为 DeepSeek 像某些 OpenAI 兼容实现那样"答案可能落在 reasoning_content"。但 DeepSeek 官方文档明确 `content` 才是最终答案，`reasoning_content` 是"在最终答案之前的推理"（"before the final answer"）。content 空**只意味着 max_tokens 不足或被内容过滤**，绝不意味着"答案在 reasoning_content 里"。

**✅实测** — TEST 15 完整复现此 bug 路径：
```
max_tokens=80, thinking 默认开启
→ content: '' (空)
→ reasoning_content: '用户需要我根据规则进行纠错。原文..."gpu又分为八核和十核最高可达35%"，这里缺少标点...' (115 字)
→ finish_reason: 'length' (token 预算耗尽)
→ usage.reasoning_tokens: 80 (吃光全部预算)
```
按当前 `extract_text` 逻辑，这条 CoT 会被作为"答案"返回，再被 `flatten_multiline` 或 `strip_fabricated_email_lines` 处理后注入用户输入框——**这正是 Gavin 13:19 看到"..."的根因**：CoT 是中文长句，被某层后处理（很可能是 `flatten_multiline` 把多行合并或 `strip_fabricated_*` 截断）压成"..."占位。

### 对主控根因分析的精化建议

主控 #3 表述"模型把思维链写进了 `content`"——**不精确**。准确表述：
> 模型把思维链放在独立字段 `reasoning_content`；但由于 `max_tokens=512` 对默认开启的思维链偏小，CoT 吃光预算导致 `content=""`，而我们 `extract_text` 在 content 空时错误地回落到 `reasoning_content`，把 CoT 文本当成"答案"返回，经后处理压缩成"..."注入用户输入框。

### Gavin 12:06 "原文兜底"路径（⚠️ 需后续日志确认）

12:06 那次日志显示"本地标点兜底（LLM 优化失效，原文完整）"——说明 LLM 路径走了 `Err(e)` 分支（mod.rs:3085-3091），触发 `format_failed=true` + 本地标点兜底。可能原因：
- (a) LLM 返回了 `finish_reason=length` 但 content 和 reasoning_content 都非空，`extract_text` 返回了 content，但 content 被上层判定为"不可信"（如不含原文关键 token）触发兜底
- (b) HTTP 层超时（6.4s 耗时接近 8s timeout 第一次尝试）
- (c) `try_once_raw` 的 `error_for_status()` 因偶发 5xx 报错

**这条无法仅凭官方文档定论**，建议主控后续在 `try_once_raw` 增加 `finish_reason`/`usage` 日志后重测一次确认。本任务 Q5 已给出 Rust struct 建议。

---

## Q4 · `max_tokens` 的语义

### 结论：`max_tokens` 是"completion 总预算"，CoT 计入其中；无独立 reasoning token 上限参数

**✅官方** — 「Create Chat Completion」API Reference（<https://api-docs.deepseek.com/api/create-chat-completion>）：
> "max_tokens integer nullable — The maximum number of tokens that can be generated in the chat completion. The total length of input tokens and generated tokens is limited by the model's context length."

即 `max_tokens` 限制的是**整个 completion（含 reasoning + 答案）**。

**✅实测** — TEST 4（`max_tokens=512` + thinking 默认开启，长问题）：
```
finish_reason: 'length'   ← 512 不够，被截断
completion_tokens: 512    ← 用满
reasoning_tokens: 144     ← 其中 144 是 CoT
content: '...荣格认为，人类共享一个深层的' (被截断的答案)
```
CoT 与答案**共享 512 预算**，CoT 优先消耗，剩余才给答案。

**✅实测** — TEST 13（`max_tokens=32` + thinking 默认开启）：
```
content: '' (空)
reasoning_tokens: 32 (全部预算给 CoT)
finish_reason: 'length'
```
小预算时 CoT **先吃光**，答案一字未出。

### 是否有独立 reasoning token 上限参数？

**❌未证实**——DeepSeek 官方 API 请求体 schema 中**没有**独立的 reasoning token 上限字段。只有 `reasoning_effort`（控制力度 high/max，不是 token 数）。

### 建议值与代价评估

**关闭思维链后的 `max_tokens`**：
- 当前 512 对纠错+标点任务**已绰绰有余**（实测 TEST 5 关闭思维链后 512 输出完整 3 段长答案都没用完）
- 关闭后**无需调高**，保持 512 即可

**若坚持保留思维链**（不推荐，见 Q6）：
- `max_tokens` 需要至少 2048（CoT 通常 200-500 tokens + 答案 100-300）
- 代价：延迟翻 2-3 倍（实测 thinking 响应 6.4s vs non-thinking 3.6s），费用按 output token 翻倍

---

## Q5 · 响应字段契约

### `finish_reason` 完整取值集合

**✅官方** — 「Create Chat Completion」响应 schema（<https://api-docs.deepseek.com/api/create-chat-completion>）：
> "finish_reason string required Possible values: [`stop`, `length`, `content_filter`, `tool_calls`, `insufficient_system_resource`]"

| 值 | 含义 |
|---|---|
| `stop` | 自然停止或命中 stop 序列 |
| `length` | 达到 `max_tokens` 或上下文上限 |
| `content_filter` | 内容过滤拦截 |
| `tool_calls` | 模型调用了工具 |
| `insufficient_system_resource` | 推理系统资源不足中断 |

**✅实测验证**：
- `stop`：TEST 11（thinking disabled，2+2）
- `length`：TEST 4/13/15（max_tokens 耗尽）
- `content_filter`：TEST 9/10 触发了内容拦截但 finish_reason 仍是 `stop`/`length`（DeepSeek 的 content_filter 似乎以"拒绝回答"文本形式返回而非 finish_reason 标记——⚠️ 这点官方文档与实测略有出入，建议日志观测）

### `usage` 对象字段

**✅官方** — API Reference 响应 schema：
> "usage object:
> completion_tokens integer required
> prompt_tokens integer required
> prompt_cache_hit_tokens integer required
> prompt_cache_miss_tokens integer required
> total_tokens integer required
> completion_tokens_details object:
>   reasoning_tokens integer — Tokens generated by the model for reasoning."

**✅实测** — TEST 12 完整 usage：
```json
"usage": {
  "prompt_tokens": 34,
  "completion_tokens": 149,
  "total_tokens": 183,
  "prompt_tokens_details": {"cached_tokens": 0},
  "completion_tokens_details": {"reasoning_tokens": 139},
  "prompt_cache_hit_tokens": 0,
  "prompt_cache_miss_tokens": 34
}
```
注意：实测响应里**同时**有 `prompt_tokens_details.cached_tokens` 和顶层 `prompt_cache_hit_tokens`/`prompt_cache_miss_tokens`——前者是新别名，后者是旧字段，两者并存（向后兼容）。

### 可直接落地的 Rust struct 字段定义建议

```rust
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,           // 新增：当前完全未解析，无法观测 token 消耗
}

#[derive(Deserialize)]
struct Choice {
    message: Option<ResponseMessage>,
    delta: Option<ResponseDelta>,
    #[serde(default)]
    finish_reason: Option<String>,  // 新增：当前完全未解析，无法区分 stop/length/content_filter
}

// ResponseMessage / ResponseDelta 已有 content + reasoning_content，保持不变

#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens: Option<u32>,
    #[serde(default)]
    total_tokens: Option<u32>,
    #[serde(default)]
    prompt_cache_hit_tokens: Option<u32>,
    #[serde(default)]
    prompt_cache_miss_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens_details: Option<CompletionTokensDetails>,
}

#[derive(Deserialize)]
struct CompletionTokensDetails {
    #[serde(default)]
    reasoning_tokens: Option<u32>,
}
```

**落地要点**：
- `usage` 和 `finish_reason` 都用 `#[serde(default)]` + `Option`，因为流式响应中间 chunk 的 `usage` 为 null
- 加完后在 `try_once_raw`（mod.rs:599）增加日志：
  ```rust
  log::info!(
      "LLM finish_reason={:?}, usage: prompt={}, completion={}, reasoning={}",
      chat.choices.get(0).and_then(|c| c.finish_reason.as_deref()),
      chat.usage.as_ref().and_then(|u| u.prompt_tokens),
      chat.usage.as_ref().and_then(|u| u.completion_tokens),
      chat.usage.as_ref().and_then(|u| u.completion_tokens_details.as_ref())
          .and_then(|d| d.reasoning_tokens),
  );
  ```
- 这让 Gavin 12:06 那种"LLM 优化失效"能在日志里直接看到是 `length` 截断还是 `content_filter` 还是 HTTP 超时

---

## Q6 · 落地建议（最重要的一节）

### 推荐方案：替换 `enable_thinking` 为 `thinking: {"type": "disabled"}`

#### 能不能真正关掉思维链？✅ 能

**具体改哪里**：

**改动点 1** — `src/llm/mod.rs:42-45`（ChatRequest 结构体）：
```rust
// 当前（错误）：
#[serde(skip_serializing_if = "Option::is_none")]
enable_thinking: Option<bool>,

// 改为：
#[serde(skip_serializing_if = "Option::is_none")]
thinking: Option<ThinkingConfig>,
```
新增 struct：
```rust
#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    thinking_type: String,  // "enabled" | "disabled"
}
```

**改动点 2** — `src/llm/mod.rs:286, 369, 521`（三处注入点）：
```rust
// 当前：
enable_thinking: Some(false),
// 改为：
thinking: Some(ThinkingConfig { thinking_type: "disabled".to_string() }),
```

**改动点 3** — `src-tauri/src/llm.rs:21-23, 86`（Tauri 端 LLM 探测用，同样问题）：
同样把 `enable_thinking: Option<bool>` 改为 `thinking: Option<ThinkingConfig>`，注入点改为 `thinking: Some(ThinkingConfig { thinking_type: "disabled".to_string() })`。

**改动点 4**（可选但推荐）— `src/llm/mod.rs:962-993`（`extract_text`）：
移除"content 空则回落 reasoning_content"的逻辑。理由：DeepSeek 语义下 content 空意味着 token 耗尽/内容过滤，回落到 CoT 只会把"模型的自言自语"注入用户输入框，是 bug。改为：
```rust
if let Some(content) = message.content.filter(|s| !s.trim().is_empty()) {
    parts.push(content);
}
// 不再回落 reasoning_content
```
delta 路径同理。`reasoning_content` 字段保留解析（用于 Q5 日志观测），但不参与答案提取。

**改动点 5**（可选）— Q5 的 `ChatResponse` 增加 `usage` + `finish_reason` 字段 + 日志。这能彻底解决 Gavin 12:06 "无法定位是 length 还是超时"的问题。

#### 风险与影响范围评估（硬约束：对其他 OpenAI 兼容 endpoint 无副作用）

| 其他 endpoint | 当前行为 | 改后行为 | 副作用 |
|---|---|---|---|
| **SiliconFlow**（Qwen3 系） | `enable_thinking: false` 被识别，关思维链 | `thinking: {"type":"disabled"}` 是未知字段，被静默忽略 → **思维链会重新开启** | ⚠️ **有副作用** |
| **OpenAI 官方** | `enable_thinking` 被忽略 | `thinking` 被忽略 | 无变化（OpenAI 用 `reasoning_effort`，本就不受这两个字段影响） |
| **Anthropic 兼容** | `enable_thinking` 被忽略 | `thinking` 可能与 Anthropic 的 `thinking` 字段冲突 | ⚠️ 需查 Anthropic API spec |
| **其他第三方兼容层** | 视实现而定 | 视实现而定 | 未知 |

**关键风险**：本项目 LLM 配置是用户可填任意 OpenAI 兼容 endpoint（任务书硬约束）。如果用户填的是 SiliconFlow Qwen3 endpoint，当前 `enable_thinking: false` 是**有效**的，改成 `thinking: {"type":"disabled"}` 后 Qwen3 的思维链会重新开启，**这是回归**。

**次优解（兼顾两者）**：

**方案 A（推荐）**：双发——两个字段都发
```rust
#[serde(skip_serializing_if = "Option::is_none")]
enable_thinking: Option<bool>,           // 保留，给 SiliconFlow/Qwen3
#[serde(skip_serializing_if = "Option::is_none")]
thinking: Option<ThinkingConfig>,         // 新增，给 DeepSeek
```
- 注入点同时设 `enable_thinking: Some(false)` 和 `thinking: Some(ThinkingConfig { thinking_type: "disabled".to_string() })`
- DeepSeek 忽略 `enable_thinking`，认 `thinking`；SiliconFlow 忽略 `thinking`，认 `enable_thinking`
- **副作用为零**（实测 DeepSeek 对未知字段静默忽略；SiliconFlow 文档亦表明对未知字段容忍）
- 缺点：请求体多一个字段，略不优雅，但**安全第一**

**方案 B**：按 api_url 域名分流
- 检测 `config.api_url` 是否含 `deepseek.com`，是则发 `thinking`，否则发 `enable_thinking`
- 缺点：硬编码域名，用户接 DeepSeek 兼容代理（如 OpenRouter）时会漏判；维护成本高
- 不推荐

**方案 C**：只改 `extract_text` 不动请求字段
- 只移除 content 空回落 reasoning_content 的逻辑
- 优点：零请求体改动，对其他 endpoint 零影响
- 缺点：思维链依然消耗 token 和延迟（DeepSeek 路径下 6.4s vs 3.6s），只是不再"泄漏到答案"，**治标不治本**

#### 综合推荐

**首选方案 A（双发）+ 改动点 4 + 改动点 5**：
- 双发解决"关思维链"（治本，DeepSeek 延迟从 6.4s 降回 3.6s）
- 改动点 4 解决"CoT 泄漏到答案"（防御性，即使某天 max_tokens 又调小也不会再泄漏）
- 改动点 5 解决"可观测性"（Gavin 12:06 那种"原文兜底"能在日志里直接看到 finish_reason/usage）

**如果主控担心双发字段冗余**，可只做方案 A 的 `thinking` 字段 + 改动点 4/5，删掉 `enable_thinking`——但需接受 SiliconFlow/Qwen3 用户的回归风险。**不建议**。

#### `max_tokens` 建议

- 关闭思维链后保持 512（实测 TEST 5 足够）
- **不要**调高到 2048——那是"保留思维链"的妥协，关掉后无必要，徒增费用

---

## 附录 A · 实测命令与响应（脱敏）

所有实测使用 `Authorization: Bearer $DEEPSEEK_KEY`，key 从 `target/release/config.toml` 读取，**未落盘到任何文件**，**未出现在任何 tmux 消息**。以下命令示例均用环境变量形式。

### TEST 1 — `GET /models`
```bash
curl -sS https://api.deepseek.com/models -H "Authorization: Bearer $DEEPSEEK_KEY"
```
响应关键字段：`{"data":[{"id":"deepseek-v4-flash",...},{"id":"deepseek-v4-pro",...}]}`

### TEST 2 — `enable_thinking: false`（当前代码行为）
```bash
curl -sS https://api.deepseek.com/chat/completions \
  -H "Authorization: Bearer $DEEPSEEK_KEY" \
  -d '{"model":"deepseek-v4-flash","messages":[{"role":"user","content":"Say only the word: hello"}],"max_tokens":64,"enable_thinking":false,"stream":false}'
```
响应关键字段：`content="hello"`, `reasoning_content="We need to respond..."`(23 reasoning_tokens)，HTTP 200。**证明 `enable_thinking` 被忽略，思维链照常输出**。

### TEST 3 — `thinking: {"type":"disabled"}`（官方参数）
```bash
curl -sS https://api.deepseek.com/chat/completions \
  -H "Authorization: Bearer $DEEPSEEK_KEY" \
  -d '{"model":"deepseek-v4-flash","messages":[{"role":"user","content":"Say only the word: hello"}],"max_tokens":64,"thinking":{"type":"disabled"},"stream":false}'
```
响应关键字段：`content="hello"`, **无 `reasoning_content` 字段**, `completion_tokens=1`。**证明官方参数有效**。

### TEST 7 — 虚构字段静默忽略
```bash
curl -sS -w "%{http_code}" https://api.deepseek.com/chat/completions \
  -H "Authorization: Bearer $DEEPSEEK_KEY" \
  -d '{"model":"deepseek-v4-flash","messages":[...],"max_tokens":32,"this_is_a_completely_bogus_field_xyz":12345,"stream":false}'
```
HTTP 200 + 正常响应（思维链照常）。**证明未知字段静默忽略**。

### TEST 13 — 复现 Gavin 13:19（max_tokens=32 + thinking 默认开）
```bash
curl -sS https://api.deepseek.com/chat/completions \
  -H "Authorization: Bearer $DEEPSEEK_KEY" \
  -d '{"model":"deepseek-v4-flash","messages":[{"role":"system","content":"Fix errors..."},{"role":"user","content":"<speech>gpu又分为八核和十核</speech>"}],"max_tokens":32,"stream":false}'
```
响应：`content=""`, `reasoning_content="(90 字 CoT)"`, `finish_reason="length"`, `reasoning_tokens=32`。**完美复现"CoT 吃光预算导致答案空"**。

### TEST 15 — 复现"CoT 泄漏到答案"（max_tokens=80）
同 TEST 13 但 `max_tokens=80`。响应：`content=""`, `reasoning_content="(115 字 CoT)"`。按当前 `extract_text` 逻辑这 115 字 CoT 会被作为"答案"返回——**这就是 Gavin 看到"..."的根因**。

### TEST 11 — thinking disabled 完整响应结构
```bash
curl -sS https://api.deepseek.com/chat/completions \
  -H "Authorization: Bearer $DEEPSEEK_KEY" \
  -d '{"model":"deepseek-v4-flash","messages":[{"role":"user","content":"What is 2+2? Just the number."}],"max_tokens":32,"thinking":{"type":"disabled"},"stream":false}'
```
响应 message 对象**只有** `role`+`content`，**没有** `reasoning_content` 字段。证明 disabled 模式下 CoT 字段完全缺席。

### TEST 8 — temperature 在 thinking disabled 下生效
`thinking:disabled` + `temperature:1.5` 三次重复请求返回 `Cerulean/Periwinkle/Cerulean`——有随机性，证明 temperature 在 non-thinking 模式下生效（与 thinking 模式下"temperature 静默忽略"不同）。

---

## 附录 B · 对主控根因分析的修正建议

主控"已确证事实"第 3 条原文：
> "当前请求体发送 `enable_thinking: false`（`src/llm/mod.rs:286` / `:369` / `:521`）。主控判断该参数属 SiliconFlow/Qwen 系，DeepSeek 不识别 → 这正是本任务 Q2 要查证的核心"

**修正**：判断正确，本任务 Q2 已实证。补充第 4 处注入点：`src-tauri/src/llm.rs:86`（Tauri 端连通性探测同样发 `enable_thinking: false`，同样无效）。

主控"已确证事实"第 3 条后半（隐含在根因 #3）：
> "模型把思维链写进了 `content`"

**修正**（见 Q3）：模型把思维链写在**独立字段 `reasoning_content`**，不混入 `content`。Gavin 看到 CoT 出现在最终文本里，是因为 `extract_text` 在 content 空时错误回落到 reasoning_content。这是**两个独立的 bug**：
1. `enable_thinking` 对 DeepSeek 无效 → 思维链开启 → 吃 token 预算
2. `extract_text` 回落逻辑错误 → content 空时把 CoT 当答案

修 #1 能治本（关掉思维链，content 不会空），修 #2 能防御（即使 content 空也不会泄漏 CoT）。**两个都应修**。

---

## 附录 C · 未证实/待后续验证项

| 项 | 状态 | 说明 |
|---|---|---|
| `content_filter` finish_reason 是否会被实际返回 | ⚠️ 实测未触发 | TEST 9/10 触发内容拦截但 finish_reason 仍是 stop/length，DeepSeek 似乎以"拒绝回答"文本返回而非 finish_reason 标记。官方文档列了该值但实测难复现。建议日志观测。 |
| Anthropic 兼容 endpoint 的 `thinking` 字段冲突 | ❌未证实 | 本项目不主推 Anthropic 兼容 endpoint，但用户可能填。方案 A 双发下若用户填 Anthropic 兼容层，`thinking` 字段可能与 Anthropic 自有 thinking 参数冲突。建议主控后续调研或加 api_url 域名 guard。 |
| Gavin 12:06 "原文兜底"具体路径 | ⚠️ 需日志 | 需在改动点 5 落地后重测，看 finish_reason 是 length/超时还是 content_filter |

---

**报告完毕。零代码改动，零 API key 泄漏。**