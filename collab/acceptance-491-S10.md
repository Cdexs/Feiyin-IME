# 验收清单 · ASR-SERVER-491 S10（服务端文字处理）

> 依据：`docs/ASR-SERVER-DESIGN.md` §13.1 后「Gavin 10-17 修正」块（权威）+ §13.4（实现记录 / 偏离留档）；任务书 `collab/spec-491-S10.md`。
> 判定：✅ 符合 / ⚠️ 偏离（附理由，已写进设计 §13.4）/ ❌ 缺失。行号以 2026-10-17 验收时为准。

## A. Gavin 修正逐条对照

| # | 要求（原话 / 选定） | 代码位置 | 判定 |
| --- | --- | --- | --- |
| 1 | 「i t n 和自动补标点……可以通过传参告诉服务端是否启用」 | 网关 `ws.rs:568` / `:571` 读 `inverse_text_normalization_enabled` / `punctuation_prediction_enabled`（缺省 true）⇒ `frame.rs:144` / `:147` `OpenParams.itn / punctuation`（serde 缺省 true，旧网关不带字段照常）⇒ `text_stage.rs:58` / `:84`（ITN）、`:94`（补标点）、`:98`（L2 收尾跟参数）；精解兜底打点 `session.rs:100` 改跟参数 | ✅ |
| 2 | 「翻译功能，也是通过参数传给服务端」 | `ws.rs:574` `translation_enabled`（缺省 false）⇒ `frame.rs:150` ⇒ `session.rs:294` 只在终稿、只在要了时调 `text_stage::translate` ⇒ `frame.rs:194` `ResultPayload.translation` ⇒ 网关 `output.translations`（子 Agent，见 D 节） | ✅ |
| 3 | 「同音纠错……去口水……默认的内置的，不应该交给客户端……开关」 | 同音纠错 `text_stage.rs:56` / `:82` 恒 `true`，无参数；去口水：回灌前已过（441 `DoneUpstream`，`session.rs` 收尾 `apply_authoritative_filler_dedup`），无参数；无效文本 `:74`、繁简 `:90` 固定 | ✅ |
| 4 | 翻译引擎「跟随本机设置」：配了大模型用大模型，否则离线 | `text_stage.rs:127` `should_try_llm_translate(enabled, connectivity_verified)`（与本机同一函数）⇒ `llm_translate`；没配 / 失败 ⇒ `nllb_translate`（进程级服务线程 `:203`–`:250`，复用本机 `try_nllb_translate` + `ensure_translation_direction`） | ✅ |
| 5 | 终端只传「要不要翻译」，不选引擎；方向自动（DEC-082） | `translate` 内 `derive_translation_target(text)`；无引擎 / 方向参数 | ✅ |

## B. 任务书逐条对照

| 任务书条目 | 代码 | 判定 |
| --- | --- | --- |
| §2 顺序：无效文本 → 同音 → ITN → 翻译（另出）→ 补标点 → 收尾 | `apply_final` `:73`–`:99`：无效 → 同音 → ITN → 繁简 → 补标点 → 收尾；翻译在 `session.rs:294` 对**处理后**文字另做 | ⚠️ 两处微调：① 多了繁简一步——本机「不走大模型」分支在 ITN 后正是 `normalize_text_for_language`（`run_llm_stage` else 分支），照搬才算同一管线；② 翻译吃处理后文字而不是 ITN 后文字——本机翻译分支吃的也是标点前文字，但本机此时文字已有 1.7B 原生标点，远程同样已有；多吃一步补标点让 NLLB 按句切分更稳。均已写进 §13.4 |
| §2 补标点「文字无自带标点时」 | `punctuate` `:105`：`has_effective_punctuation` ⇒ 跳过（同本机 `pretranscribed_native_punctuated`，DEC-047 口径） | ✅ |
| §2 补标点用进程级服务，不碰本机录音线程实例 | `:112` `punctuate_via_service`；日韩 `:109` 走 jako（进程级 `OnceLock<Mutex>`，本机 `apply_local_punctuation` 同款分流） | ✅（任务书漏写日韩分流，按本机补齐） |
| §2 预览只做「关 ⇒ 全剥」 | `apply_preview` `:62`–`:68` | ✅ |
| §2 `strip_trailing_always` 远程恒 false | `:67` / `:98` / `:138` 第三参 `false` | ✅ |
| §2 每个节点套 `transfer` | `step` `:39`：文字变了才搬，没变原样 | ✅ |
| §3 `text` / `words` = 处理后文字，不另给 `processed_text` | `session.rs:223`（预览）、`:292`（终稿）后 `result_words(&text, &times, …)` | ✅ |
| §3 译文失败 ⇒ 不给，日志记原因 | `translate` 返回 `None` 前 `log::warn!` | ✅ |
| §4 `OpenParams` 三字段 + serde 缺省 | `frame.rs:143`–`:150` + 手写 `Default`（true/true/false） | ✅ |
| §4 新模块 `text_stage.rs` | `apply(text, times, &TextOpts, final)` 拆成 `apply_preview` / `apply_final` 两个函数 | ✅（同义拆分，调用点各自明确） |
| §4 NLLB 进程级服务线程、首次用到才加载 | `nllb_translate` 懒起线程；线程内首个请求才 `ensure_translation_direction` 加载；60s 上限；send 失败 / 断开 ⇒ 置空下次重建 | ✅ |
| §4 大模型：`LlmClient::new` + 单线程 tokio、场景未知、`multiline_safe = true` | `llm_translate` `:153`：`new_current_thread().enable_all()`、`SceneContext::unknown()`、`send_window_title = false`、`multiline_safe = true`；标点参数用**终端**的 `o.punctuation` | ✅ |
| §4 S8 客户端删 `postprocess`、改发三字段 | 子 Agent，见 D 节 | 见 D |

## C. 不改坏现有功能

| 改动点 | 影响 | 结论 |
| --- | --- | --- |
| `text_stage.rs` 新模块 | 只被远程会话调用；调的全是现有函数（同音 / ITN / 繁简 / 标点服务 / jako / L2 / 翻译），**一行未改** | 本机管线零接触 |
| `crate::punctuate_via_service` 被远程终稿调用 | 本来精解线程（含远程会话）就在用这个进程级服务，串行、带冷却 / 超时 / 重启 | 无新风险；只在精解失败退预览文本时才多一次调用 |
| 远程 NLLB 线程 | 只在「服务端没配大模型 + 终端要翻译」才起；本机 worker 的 `thread_local` 缓存不受影响（另一线程另一份） | 内存 +约 600MB（仅该情形），已写 §13.4 |
| `session.rs:100` 精解兜底打点改跟终端参数 | 只影响远程会话 | 本机 `acc_punct_enabled` 在 `main.rs` 另算，未动 |
| `OpenParams` / `ResultPayload` 加字段 | serde 缺省 + `skip_serializing_if` ⇒ 新旧内核 / 网关互通 | 见 E 节兼容测试 |
| 远程预览多跑同音 + ITN | 纯函数，毫秒级；部分句子 ITN 随字数增加可能前后变化（如「三」→「3」→「3.5」），与阿里预览行为同类 | 接受 |

## D. 子 Agent 交付物验收

| 交付物 | 对照 | 结论 |
| --- | --- | --- |
| 网关 `ws.rs` `result_json` | 有译文 ⇒ `output.translations = [{sentence_id:1, lang, text, sentence_end:true}]`；无 ⇒ 不出键；其它字段未动 | ✅ |
| S8 客户端 `transcription/feiyin_remote.rs` | `run_task_ext` 删 `postprocess`，只剩身份；新增 `text_params()`（true / true / false） | ✅ |
| `transcription/qwen_inference.rs` Feiyin 分支 | `text_params()` 合并进 `payload.parameters` **顶层**；`build_run_task_message` 未动 ⇒ 阿里云路径不变（`build_run_task` 9 个旧测试过） | ✅ |
| 单测 | 见 E 节；断言覆盖开关两态、标点不叠加、口水词终稿空、**时间与字数一一对应**、serde 缺省互通 | ✅ |

子 Agent 自报未覆盖：① 网关 run-task 参数解析内联在 `handle_run_task`，未单测（缺省值靠 `OpenParams` serde 缺省测试间接覆盖）；② Feiyin 分支合并写在大函数里，只测了 `text_params()` 形状。两处逻辑各 3 行，已逐行人工核对（`ws.rs:568`–`:574`、`qwen_inference.rs` Feiyin 分支 `extend`），接受；端到端由 S11 Python 参考客户端冒烟覆盖。

## E. 测试

| 项 | 结论 |
| --- | --- |
| 新增单测 | 12 个：`text_stage.rs` 7（`itn_switch_preview_and_final` / `punctuation_off_strips_all_punctuation` / `preview_with_punctuation_on_keeps_marks` / `final_with_existing_punctuation_not_repunctuated` / `filler_only_final_is_empty` / `times_always_match_char_count` / `translate_empty_text_is_none`）；`frame.rs` 3（缺字段取缺省 / 手写 `Default` == serde 缺省 / `translation: None` 不出键）；网关 `ws.rs` 2 + 旧测补一条断言 |
| 分项 | `asr_service` 25 过；`feiyin_remote` 3 过；网关 31 过；`build_run_task` 9 过 |
| 全量 | `feiyin-ime` **2061 过 / 0 失败** / 48 忽略（S9 验收时 2048） |
| rustfmt / `cargo check --all-targets` | 干净 |
| 正式回归 | S11 完成后统一交 tester-1 |

## F. 跨平台

全部改动在平台中立模块（`asr_service/` / 网关 / `audio/feiyin_remote.rs`），无 `cfg(target_os)` 分支 ⇒ Windows / macOS 同一行为。macOS 侧需知：`OpenParams` 多三字段、`ResultPayload` 多 `translation`、远程翻译线程。写入 `docs/MACOS-HANDOFF.md`。
