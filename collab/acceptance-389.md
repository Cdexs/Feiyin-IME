# 验收清单 · FIX-NEARFIELD-BY-SEGMENT-AND-PREVIEW-389（coder-2）

> 规则（Gavin 2026-09-23）：派单前确认的方案，验收时逐条核对落地代码。任一条「偏离 / 缺失」⇒ 不提交。
> 结论列填：✅ 符合（文件:行）/ ⚠️ 偏离（说明）/ ❌ 缺失。

## C 近场门按整句判定

| # | 设计要求 | 代码位置 | 结论 |
| --- | --- | --- | --- |
| C1 | 以 VAD 段为单位：段开始时 `seg_confirmed = !level.ready()`（未就绪整句直接确认） | | ✅ `SegmentGate::update`：段开始 `seg_confirmed = !level.ready()` |
| C2 | 段内 `seg_peak = max(sm)`；`sm >= estimate × 0.3` 即确认，确认后本段剩余全部 has_speech=true，直到 VAD 段结束 | | ✅ 段内取峰值；`smooth_rms >= estimate × NEARFIELD_PEAK_RATIO` 即确认，确认后至段结束恒有声 |
| C3 | `has_speech = vad && seg_confirmed`，5 处判定共用同一个值 | | ✅ `chunk_has_speech` 唯一判定，5 处共用 `judg.has_speech` |
| C4 | 段结束只 push **已确认段**的峰值；`SegmentPeakLevel` estimate=中位数、ready=有效样本≥2、按会话时间 30s 过期 | | ✅ 只 push 已确认段峰值；estimate 中位数；ready = 样本≥2 或 seed；`prune` 按会话 ms 30s |
| C5 | 300ms 补偿只在「已确认段因 VAD 翻转结束」时补；未确认段不补 | | ✅ `vad_on && prev_has_speech && !vad_speech` 才补 300ms |
| C6 | VAD 不可用的音量兜底与 384 逐位相同 | | ✅ VAD 不可用分支 `chunk_rms > silence_threshold` 不变 |
| C7 | 旧 `NearFieldLevel` / 逐块门代码删除，单测改写为新算法版本（不只删不补） | | ✅ `NearFieldLevel` 删除，单测改写为段门版本（段门 5 条） |
| C8 | 埋点 `segment end` 与 `nearfield summary: segments/confirmed/rejected/level`，`log_enabled!` 守卫 | | ✅ `segment end` / `nearfield summary` 均 `log_enabled!` 守卫 |

## C2 跨录音沿用

| # | 设计要求 | 代码位置 | 结论 |
| --- | --- | --- | --- |
| C2-1 | 进程内存 `Mutex<Option<CarryLevel{device, level, updated}>>`，不写配置文件 | | ✅ `static LOCALRT_CARRY_LEVEL: Mutex<Option<CarryLevel>>` |
| C2-2 | device = `config.audio.input_device`（空串原样当 key）；同设备且 ≤10 分钟才作 seed | | ✅ `seed_usable`：同 device 且 ≤600s；device=`config.audio.input_device` |
| C2-3 | 有 seed ⇒ 立即就绪、estimate=seed；本次学到样本就绪后只用本次样本 | | ✅ `estimate`：样本≥2 只用本次样本，否则 seed |
| C2-4 | seed 在用时连续 2 段未确认 ⇒ 丢弃 seed 回到未就绪（打日志）；确认段清零计数 | | ✅ `end_segment`：seed 在用时连续 2 拒丢弃并打日志，确认段清零 |
| C2-5 | 录音结束且本次就绪 ⇒ 写回 carry | | ⚠️→✅ **两处偏离已由主控修复**：①原按 `level.ready()` 写回 ⇒ 仅靠 seed 就绪也写回、旧 seed 被刷新时间戳永不过期；改为 `samples_ready()`（本次学到）才写回 ②seed 被丢弃后未清 carry ⇒ 下次录音重复用同一坏 seed；新增 `seed_dropped`，未学到新值时清 carry。补单测 `seg389c2_review_seed_dropped_and_no_writeback_without_samples` |
| C2-6 | `transcribe_streaming_local` 新增入参仅改 `main.rs` 一处调用；`audio/mod.rs` 未动；其他 pub 签名未动 | | ✅ 仅 `transcribe_streaming_local` 新增 `vad_device`，唯一调用点 `main.rs`；`audio/mod.rs` 未动 |

## D3 预览不回退

| # | 设计要求 | 代码位置 | 结论 |
| --- | --- | --- | --- |
| D3-1 | 记录 prev_committed（上次派发 committed_len，首次 0） | | ✅ `last_committed_len` 每次派发前取作 `prev_committed` |
| D3-2 | 部分窗 `win_committed = prev + round((cur − prev) × 已含片样本数/全部片样本数)` | | ✅ `partial_win_committed`（main.rs:11183）按新片累计样本占比折算 |
| D3-3 | 含本次末片的窗（含松键合并窗、带 pending 的下次首窗）用该片所属派发的 committed_len | | ✅ 含本次末片窗 `usable = k+1 == n_new` 用 committed_len；松键合并窗传 `true` |
| D3-4 | `PreviewReflow.boundary_usable`：部分窗 false ⇒ `on_text` 不用 `bound_of`；`on_bound` 对部分窗不重渲 | | ✅ `on_text` 部分窗不用 `bound_of`；`on_bound` 仅 `usable` 时重渲 |
| D3-5 | 单测：[10.55,5.56]、prev 0、cur 71 ⇒ 首窗 47，边界 71 已到仍用 47；含末片窗用 71 | | ✅ 单测 main.rs:17419 起（首窗 46/47 区间断言、含末片 71） |

## 边界

| # | 要求 | 结论 |
| --- | --- | --- |
| B1 | 只改 `local_stream.rs`、`main.rs`；未改 `transcription/mod.rs` / `vad.rs` | ✅ |
| B2 | 其他管线逐位不变 | ✅ 改动均在本地 realtime 专用函数与滑窗线程 |
| B3 | fmt EXIT0、check 0 error、warnings ≤ 97/88；五文档；macOS 结论在 result.md | ✅ 主控复跑 fmt/check；warnings 97/88 |

**结论（主控 2026-09-23）**：C2-5 两处偏离已修，其余全部符合 ⇒ 验收通过。
