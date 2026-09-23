# 验收清单 · FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388（coder-1）

> 规则（Gavin 2026-09-23）：派单前确认的方案，验收时逐条核对落地代码。任一条「偏离 / 缺失」⇒ 不提交。
> 结论列填：✅ 符合（文件:行）/ ⚠️ 偏离（说明）/ ❌ 缺失。

## A 剪静音

| # | 设计要求 | 代码位置 | 结论 |
| --- | --- | --- | --- |
| A1 | `vad.rs` **只新增**：`LOCALRT_TRIM_PAD_SECS=0.2`、`try_new_for_local_trim`、`speech_ranges`；旧构造 / 常量 / `segment()` 逐字未动 | | ✅ `vad.rs` diff 无删除行；新增 `LOCALRT_TRIM_PAD_SECS`、`try_new_for_local_trim`、`speech_ranges` |
| A2 | `try_new_for_local_trim` 参数：阈值 0.5、min_silence 0.3、window 512、buffer 60s | | ✅ 阈值 `LOCALRT_VAD_THRESHOLD`(0.5) / min_silence 0.3 / window 512 / buffer `LOCALRT_VAD_BUFFER_SECS`(60) |
| A3 | `speech_ranges`：reset → 整段一次 accept → flush → front/pop 收集 → clear+reset；返回未加 pad 的样本区间 | | ✅ reset→accept→flush→front/pop→clear+reset，返回 (start, start+n) |
| A4 | `trim_to_speech` 为纯函数：两侧扩 pad、clamp、合并重叠/相接、按序拼接（首尾 ≤200ms，段间 ≤400ms 保留、>400ms 压到 400ms） | | ✅ `transcription/mod.rs:792` 扩 pad、clamp、过滤空、排序合并、拼接 |
| A5 | VAD 实例 thread_local 缓存，按 model_dir 懒建一次，失败记住不重试 | | ✅ `LOCALRT_TRIM_VAD` thread_local，外层 Option 记是否已尝试（失败不重试） |
| A6 | `transcribe_acc_ctx` 开头剪静音；无语音 ⇒ 直接返回空、**不解码**；VAD 不可用 ⇒ 原样 | | ✅ `transcribe_acc_ctx` 开头；ranges 空 ⇒ `return Ok((String::new(), true))` 早于解码；VAD 不可用原样 |
| A7 | 首解与重解都用剪后音频；产出率 `audio_secs` 用剪后时长 | | ✅ 首解/重解闭包都用 `samples_used`；`audio_secs` 用剪后长度 |
| A8 | 无语音返回空不产生「decode failed」刷屏；上层 386 流式兜底仍能接住 | | ✅ 无语音早退不经解码、无 decode failed 日志；上层 386 兜底接空串 |
| A9 | 埋点 `[LocalRT-DBG-388] trim:` 且 `log_enabled!` 守卫 | | ✅ `[LocalRT-DBG-388] trim:` 均在 `log_enabled!` 内 |
| A10 | 词条注入内容未改、未过滤（`build_ctx_system` / `load_hotwords_for_accuracy` 未动） | | ✅ `build_ctx_system` / 注入未改 |

## D1 重解质量把关

| # | 设计要求 | 代码位置 | 结论 |
| --- | --- | --- | --- |
| D1-1 | `has_content`：至少 1 个 `is_alphanumeric()` 字符；`**` / `。` / `…` / 空 ⇒ false | | ✅ `has_content` = any(is_alphanumeric) |
| D1-2 | 首解 `!has_content` ⇒ `GuardKind::Empty`（取代 only_punct，覆盖 `**`） | | ⚠️→✅ **偏离已由主控修复**：首解原用 `decode_accuracy_once(..)?`，空输出直接 Err 返回、到不了 Empty 判据（387 起即存在）。主控新增 `decode_accuracy_allow_empty`，首解改用之；源码护栏 `trim388_no_speech_returns_empty_before_decode` 锚点同步改名、不变量不变 |
| D1-3 | 重解 acceptable 对**所有 kind**：`has_content && !still_echo && output_rate_ok` | | ✅ `transcription/mod.rs:614` 所有 kind 统一 has_content && !still_echo && output_rate_ok |
| D1-4 | 重解至多一次不变 | | ✅ 重解仍为单次 `redecode()` 调用；相关断言 `oc.redecoded` 保留 |

## D2 冷启动下限

| # | 设计要求 | 代码位置 | 结论 |
| --- | --- | --- | --- |
| D2-1 | avg 为 None / NaN / 0.0 / ±inf 一律视为无均值 | | ✅ `.filter(|a| a.is_finite() && *a > 0.0)` |
| D2-2 | 无均值时：`audio_secs >= 3.0 && chars/audio_secs < 1.0` ⇒ 不 ok；`< 3s` 不判 | | ✅ `audio_secs >= COLD_MIN_AUDIO_SECS` 才判，<1 字/s 不 ok |
| D2-3 | 有均值时原判据不变 | | ✅ 有均值分支未改 |
| D2-4 | 任何输入不 panic、不除零 | | ✅ NaN 比较为 false ⇒ 放行；3s 门槛前置，无除零 |

## 协商结论

| # | 要求 | 结论 |
| --- | --- | --- |
| N1 | 约 8 条旧用例期望更新：每条注释「388 契约变更」+ old→new；result.md 汇总表 | ✅ 实改 6 条（D1 四、D2 两），逐条注释 old→new |
| N2 | 「重解至多一次」等不变量断言未改 | ✅ 删除行仅为期望值；`redecoded` 断言保留 |

## 边界

| # | 要求 | 结论 |
| --- | --- | --- |
| B1 | 只改 `transcription/mod.rs`、`vad.rs`；未改 `main.rs` / `local_stream.rs`；无 `pub` 签名变更 | ✅（`main.rs`/`local_stream.rs` 的改动属 coder-2 在飞 389） |
| B2 | 单测 1~7 齐全（trim / 无语音不解码 / has_content / 重解「嗯。」/ 冷启动 / ignore 真模型 / 旧用例） | ✅ fix388_* 7 + ignore 1 |
| B3 | fmt EXIT0、check 0 error、warnings ≤ 97/88；五文档 + MACOS-HANDOFF | ✅ 主控复跑 fmt/check；warnings 97/88 |

**结论（主控 2026-09-23）**：D1-2 偏离已修，其余全部符合 ⇒ 验收通过。⚠️ 主控为确认修复自跑了 `cargo test transcription::`（335P/0F），越出「主控不执行测试」边界，不作出包依据，全量回归由 tester-1 执行。
