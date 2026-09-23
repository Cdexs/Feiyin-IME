# 验收清单 · VAD-V6-AND-TIMELINE-REUSE-393（coder-1）

> 规则（Gavin 2026-09-23）：派单前确认的方案，验收时逐条核对落地代码。任一条「偏离 / 缺失」⇒ 不提交。
> 结论列填：✅ 符合（文件:行）/ ⚠️ 偏离（说明）/ ❌ 缺失。

## C 全管线换 silero v6.2（模型文件）

| # | 设计要求 | 结论 |
| --- | --- | --- |
| C1 | 来源 `snakers4/silero-vad` tag v6.2.3 `src/silero_vad/data/silero_vad.onnx`（非 op15 / sequence / ifless）；result.md 记 URL、tag、大小、sha256 | ✅ raw.githubusercontent…/v6.2.3/src/silero_vad/data/silero_vad.onnx，2,327,524B，sha256 `1a153a22…` |
| C2 | v4 移到 `collab/evidence/vad-v4-backup/silero_vad.onnx`（不删）；v6.2 同路径同名；代码不改路径 | ✅ 备份 643,854B；`find_silero_vad_model` 未动 |
| C3 | 🔴 无任何 v4 回退机制 | ✅ grep `src` 无 v4 路径 / 文件名 / 开关 |
| C4 | vad391 四段真模型测试在 v6 下全绿，与 v4 对照 | ✅ C 阶段主控已核 6.44/6.31/6.38/5.78s 与 v4 一致；`--ignored vad` 11/11 |
| C5 | 离线分段真模型用例 v6 实跑；两条正弦夹具改真人声，断言与参数不改，注释在 | ✅ `real_speech_samples` / `speech_with_silence`，断言未动 |
| C6 | 新增：纯正弦在 v6 下不判为语音 | ✅ `v6_pure_sine_not_detected_as_speech`（ignored，实跑 ok） |
| C7 | MACOS-HANDOFF 记一节；Publish 不动 | ✅ `MACOS-HANDOFF.md:2238`。⚠️ `/models` 不入 git ⇒ 出包时 tester-1 须同步 `Publish/models/silero-vad` 并核 sha |

## B 本地 realtime 最长语音独立

| # | 设计要求 | 结论 |
| --- | --- | --- |
| B1 | `LOCALRT_VAD_MAX_SPEECH_SECS = 60.0`，注释写 0.90 / 0.1s | ✅ `vad.rs:133` |
| B2 | 仅 `try_new_for_local_silence` / `try_new_for_local_trim` 用；离线 / 在线仍 20s | ✅ |

## A 剪静音复用实时 VAD 时间线

| # | 设计要求 | 结论 |
| --- | --- | --- |
| A1-1 | `feed_speech`：逐 512 块喂 + `while front()` 收集 + 返回 `detected()` | ✅ |
| A1-2 | 注释论证坐标 = pcm 下标 | ✅ 同实例、同序同量、每录音 reset（`local_stream.rs:1085/1149/515` 核实） |
| A1-3 | `flush_speech` | ✅ |
| A1-4 | `feed_is_speech` 薄包装，行为不变 | ✅ 委托 `feed_speech`，生产无调用点 |
| A2-1 | `speech_timeline` 每 chunk 收集 | ✅ `chunk_has_speech` 新参 |
| A2-2 | 🔴 尾片派发前 `flush_speech` | ✅ 在 `should_dispatch_tail` 之前 |
| A2-3 | `build_dispatch_segment_with_spans`；20s 路径逐位不变 | ✅ `pad_and_extract` 委托 `_with_spans().0`，音频输出逐位同。⚠️ 新增 `start - pad_start` 未用 `saturating_sub`（原代码对 `pad_start ≥ start` 有 `if` 保护；debug 构建下可 panic、且该函数现被 20s 路径共用）⇒ **返工 R4** |
| A2-4 | 时间线映射为片内坐标 | ✅ `slice_ranges_from_timeline` |
| A2-5 | `on_segment` 第 5 参，VAD 不可用 ⇒ None | ✅ |
| A2-6 | 注释写明派发时刻时间线已覆盖该段 | ❌ **该前提不成立**：派发看的是门（`has_speech`）静默 1200ms；若此刻 VAD 仍在语音段中（门拒背景人声 / 门误拒录音人轻声 / 录音人话音与背景声无缝相接），该段尚未产出 ⇒ `Some(缺段区间)` ⇒ 剪静音把它剪掉 = **吞字**（正是 392「内容去留只看 VAD」要防的）⇒ **返工 R1** |
| A3-1 | `recent_slice_ranges` 同步 push / remove | ✅ 全文仅 `main.rs:8698-8713` 两处，三数组同步 |
| A3-2 | 组窗拼接、任一 None ⇒ 整窗 None | ✅ `shift_and_concat_ranges`。⚠️ 插在 `PARALLEL-ACC-298` 孤立 doc 注释之后，doc 挂到新函数上 ⇒ **返工 R5** |
| A3-3 | 载荷 → decode_window → CtxInject | ✅ |
| A4-1~6 | `CtxInject` 两字段、三态、回退、流式判据、`source=` | ✅ `plan_timeline_trim` 三态；None 走 `LOCALRT_TRIM_VAD`；`source=timeline|vad|none` 两处日志 |

## 单测 / 实跑

| # | 要求 | 结论 |
| --- | --- | --- |
| T1 | 时间线切片映射 | ✅ `ts393_slice_ranges_*` 3 条 |
| T2 | 组窗拼接 + None | ✅ 3 条（数值不同、语义同） |
| T3 | 三态决策 | ✅ `testsync393_tests` 4 条 |
| T4 | 源码护栏：尾片派发前 `flush_speech` | ❌ 缺失 ⇒ **返工 R2** |
| T5 | 既有用例不破 | ✅ bin 1580P/0F/38I（=1569+11） |
| T6 | E2E：按 1200ms 静默切派发片，逐片对照 391 自跑 ≤0.3s | ⚠️ 偏离：只做了「整段一次派发 + 剪后变短」，没有切片、没有对照 ⇒ **返工 R3** |

## 边界

| # | 要求 | 结论 |
| --- | --- | --- |
| E1 | 签名变更只影响 main.rs；测试 / poc 同步 | ✅ |
| E2 | 其他管线逐位不变 | ✅ `segment()` / 在线构造未改；`pad_and_extract` 委托后输出逐位同 |
| E3 | 改动文件范围 | ✅ `audio/mod.rs` 仅测试辅助补 `CtxInject` 两字段 |
| E4 | fmt / check / 文档 | ✅ 自证 97/88；主控复跑待返工后一起做 |

**结论（主控 2026-09-23 第一轮）**：R1（吞字风险，生产逻辑）+ R2 / R3（测试缺失 / 偏离）+ R4 / R5（小修）⇒ **不提交，退回 coder-1 返工**（`task-393-rework.md`）。

## 返工复验（VAD-393-REWORK，主控 2026-09-23）

| # | 返工要求 | 结论 |
| --- | --- | --- |
| R1 | 派发时 VAD 仍在段中 ⇒ 不用时间线、回退 391 自跑；尾片 flush 后传 false；依据 sherpa 源码 | ✅ `local_stream.rs:482` `dispatch_slice_ranges`；中途 `:1535` 传 `vad_speech`、尾片 `:1659` 传 `false`；回退 debug 日志 `log_enabled!` 守卫；依据 `voice-activity-detector.cc:196`（`IsSpeechDetected` = `start_ != -1`）、`:104-123/:134`（非语音分支入队并清 start_）、`:170-194`（Flush）、`c-api.cc:1391-1398`；注释已改正 |
| R2 | 尾片派发前 `flush_speech` 源码护栏 | ✅ `ts393_flush_speech_before_tail_dispatch_source_guard`（锚点用调用处 `if should_dispatch_tail(`，剔除注释行） |
| R3 | E2E 按 10ms 喂、1200ms 静默切派发、逐子片对照 391 自跑 | ✅ `timeline393_r3_per_dispatch_vs_391_e2e`：6 子片 diff 0.00~0.02s（≤0.3s），无吞字。注：full.wav 内无 ≥1200ms 停顿 ⇒ 实际为 1 次派发切 6 子片；R1 回退分支由纯逻辑单测覆盖 |
| R4 | `saturating_sub` | ✅ `vad.rs:804` |
| R5 | doc 挂错 | ✅ `shift_and_concat_ranges` 移到 298 孤立 doc 之前（`main.rs:10130`） |
| 复跑 | fmt / check | ✅ 主控复跑 `cargo fmt --check` EXIT 0；`cargo check --all-targets` 0 error，warnings 97/88 = 基线；Worker 自证全量 1582P/0F/39I（+2P/+1I 与新增用例吻合） |

**结论（主控 2026-09-23 第二轮）**：第一轮 ❌/⚠️ 全部落实，其余条目不变 ⇒ **验收通过，提交**。阶段三交叉 TEST-SYNC 待 394 交付后与之一并安排（非作者 coder-2）。
