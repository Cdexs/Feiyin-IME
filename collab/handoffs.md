# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-23 归档：2026-09-22 共 31 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

## 2026-09-23 — tester-1 — TEST-SYNC-393 ✅ 交付（阶段三 · 非作者护栏 10 条；生产零改动）

- **性质**：阶段三 TEST-SYNC，给 `VAD-V6-AND-TIMELINE-REUSE-393`（返工 R1~R5）按**设计契约**补独立护栏（不读实现反推）。只改 `#[cfg(test)]` 区；**未碰** coder-2 在飞的 `src/translation/mod.rs`；**未跑 `cargo test`/`build`**（白名单）。
- **新增 10 条**（9 条要求，`#6` 拆两条）：`local_stream.rs` 3（跨两片截断+平移 / 首尾相接不产空区间 / 两调用点源码护栏）；`mod.rs` 2（越界 clamp 不 panic / `WholeWindow` 返回整窗源码护栏）；`main.rs` 4（缺长度 0 偏移 + 三片累计 / 三数组 remove 相邻源码护栏 / 路B `speech_ranges: None` 源码护栏）；`vad.rs` 1（相邻段不 panic + 两函数音频逐位相等 + `pad_before==0`）。
- **验证（白名单）**：我的 4 文件 `rustfmt --config skip_children=true` 后 `--check` **4/4 CLEAN**；`cargo check --all-targets` **EXIT 0**、0 error，warnings **bin 92 / test 87 ≤ 97/88**；`numstat == -w`（74/0、69/0、38/1、28/0）。🔴 **未跑 `cargo test`**（首跑阶段四）。
- ⚠️ **如实上报**：全仓 `cargo fmt --check` 当前 **EXIT 1**，失败点 = coder-2 在飞的 `src/translation/mod.rs:1639`（未提交 WIP 格式），**非本单任何文件**；待 coder-2 落定后需复跑不带参数的 `cargo fmt --check`。
- **红线**：未改生产代码 / 未 commit / 未 push / 版本号未动 / 零凭证。

## 2026-09-23 — coder-1 — VAD-V6-AND-TIMELINE-REUSE-393 ✅ 交付（阶段一；待主控验收）

- **C 换模型**：`models/silero-vad/silero_vad.onnx` **同路径替换为 silero v6.2.3**（`2,327,524B`，sha256 `1a153a22…`）；v4（`643,854B`，`9e2449e1…`）备份至 `collab/evidence/vad-v4-backup/`（**运行时不引用**）。🔴 **无 v4 回退机制**（Gavin 裁定）。v6 下原 2 条以 440Hz 正弦冒充语音的夹具改用 `full.wav` 真人声（断言/参数不动）+ 新增 `v6_pure_sine_not_detected_as_speech`。
- **B**：新增 `LOCALRT_VAD_MAX_SPEECH_SECS=60.0`，仅本地实时两构造使用；离线 `try_new` / 在线 `try_new_for_streaming` 仍 `VAD_MAX_SPEECH_DURATION`（20s）。
- **A 时间线复用**（不再每窗重跑 VAD）：A1 `vad.rs` `feed_speech`/`flush_speech` + `build_sliding_segments_with_spans`；A2 `local_stream.rs` `slice_ranges_from_timeline` + `on_segment` 第 5 参 + `chunk_has_speech` 收集时间线；A3 `main.rs` `shift_and_concat_ranges` + `AccSliceMsg`/`AccTaskMsg`/`decode_window` 接线；A4 `mod.rs` `CtxInject` 增 `speech_ranges`+`streaming_nonempty`、`plan_timeline_trim` 三态分派（空区间 + 流式非空 ⇒ 整窗解码不吞字；`None` ⇒ 回退 391 自跑 VAD，仍 v6），日志 `source=timeline|vad|none`。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings **97/88** = 基线；新增单测 **11 条** + 真模型 E2E `timeline393`（**60.15s → 53.79s**）；`--ignored vad` **11/11 通过**；全量 `cargo test --bin feiyin-ime` **1580P/0F/38I**。
- **同步改动**：`ts391_entry_points_feed_by_window_only` 按 A1 新结构更新（`feed_speech` 走 `feed_in_vad_windows`；`feed_is_speech` 断言委托 `feed_speech`）；`poc_slice_cut_381.rs` / `audio/mod.rs` 仅补 `CtxInject` 新字段。`docs/MACOS-HANDOFF.md` 新增本次节。
- 红线：未 commit / 未 push / 未出包 / 版本号未动 / 零凭证。**待 tester-1 阶段四回归 + 端测**。

## 2026-09-23 — tester-1 — TEST-EXEC + BUILD-392 ✅ 出包（388/389/390/391/392 合包；八项 + 三特殊点全 PASS；guard346 修复后重派）

- **交付源码**：HEAD `ad08251`，工作区 clean，版本 0.9.3。核心单 388（VAD 剪静音+重解把关）/ 389（整句近场门+跨录音沿用音量+预览不回退）/ 390（解码串行+`max_new_tokens` 限流）/ 391（VAD 按 512 逐块喂入，修剪静音吞字）/ 392（近场门只管时序、内容去留只看 VAD；音量估计下中位、首段不学）。**上一轮 HEAD `bfe584b` 全量回归捕捉 `guard346_acc_counter_wiring` FAILED，停手上报；主控 `ad08251` 修复后重派**。
- **阶段四（全量未过滤；fmt `--check` 不带 `skip_children`）**：root `cargo test --no-fail-fast` **1657P/0F/38I**（EXIT 0；`feiyin-ime` bin **1569P/36I**）；`src-tauri` **92P/0F/0I**；Vitest **SKIP**（`ui/` 无 diff）；warnings **97/88/17** = 基线；fmt **EXIT 0**；**`guard346_acc_counter_wiring` 由红转绿**。
- **NEW/GONE（精确集合差）**：基线 1645P/35I ⇒ 净 **+12P/+3I**。NEW **15**（12P+3I）= `gate392_*` 5 + `ts392n_*` 4 + `ts391_*` 3（2 ignored）+ `vad391_*` 3（3 ignored）；**GONE 0**（8 条 `seg389_*` 改写属期望值变更，同名保留）。
- **重点失效模式**：其他管线（`build_padded`/`naive_chunk`/20s 快照/`segment`）、`vad391`/`testsync391`/`feed_in_vad_windows`、`gate392`/`testsync392`、`seg389`/`fix389`/`testsync389`、`fix388`/`testsync388`/`trim388`、`fix390`/`testsync390`、`guard346`、`342` 共 **99 条 ok、0 失败**，无停手。
- **BUILD-392**：Step1 残 0 → Step2 npm 674ms + Tauri 1m37s（17w）→ Step2c UI cp（21:01 / `b8b8e644…`）→ Step3 主程序 2m52s（97w + crash 9w）→ Step4 三 exe→Publish。产物 main `4746f7bff754…`（14,870,528B/21:04:27）/ ui `b8b8e6444663…`（10,050,048B/21:01:30）/ crash `efdd988056f9…`（24,879,104B/21:02:37）；两副本全等、三者均异于 BUILD-390。
- **八项逐项 PASS**：①时间戳 21:01–21:04 ②两副本 sha 相等且异于上包 ③ProductVersion **0.9.3** ④冒烟 PID **164 Responding=True** / 两处无新 crash.json / panic 0 / 残 0 ⑤config `da2be5da…` 三时点不变 ⑥warnings 97/88/17 ⑦正探针 `[LocalRT-DBG-388]`=2 / `vad_only_speech_chunks`=1 / `learned=`=1，反探针 `nearfield gate: vad=on`=**0** ⑧scene/itn 两 toml 三副本全等。
- **三特殊点（仅核验）**：① dll 四张三副本（`sherpa-onnx-lib`/`Publish`/`target-release`）全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本 `60b227de…` 全等；③ `Publish/models/` 1.7B 七文件与源 sha256 全等、0.6B 保留。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启 + 带 `-debug` 端测**。红线：未 push（需 Gavin 指示）/ 版本号未动 / 未改生产代码 / 未 `cargo clean` / 零凭证。

## 2026-09-23 — tester-1 — TEST-EXEC + BUILD-390 ✅ 出包（388/389/390 合包；八项 + 三特殊点全 PASS）

- **交付源码**：HEAD `02c57c8`（`af3a0ad`/`1af43a0`/`e91cccb`/`1efa987`/`9ae53d2`/`02c57c8`），工作区 clean，版本 0.9.3。核心单 388（解码前 VAD 剪静音 + 重解统一质量把关 + 冷启动坍塌下限）/ 389（近场门按 VAD 整句判定 + 跨录音沿用录音人音量 + 部分窗预览不回退）/ 390（窗口解码并发 2→1 + 按剪后语音时长限制 `max_new_tokens`）。DEC-081：任务书已下达「现在可以出包」。
- **阶段四（全量未过滤；fmt `--check` 不带 `skip_children`）**：root `cargo test --no-fail-fast` **1645P/0F/35I**（EXIT 0；`feiyin-ime` bin **1557P/33I**）；`src-tauri` **92P/0F/0I**；Vitest **SKIP**（`ui/` 无 diff）；warnings **97/88/17** = 基线；fmt **EXIT 0**。
- **NEW/GONE（精确集合差）**：基线（1efa987 实测）1639P/35I ⇒ 净 **+6P/+0I**。NEW **6** = `transcription::fix390_tests` 3 + `transcription::testsync390_tests` 3；**GONE 0**。6 条全部 `ok`，+6P 逐位吻合。
- **重点失效模式**：其他管线（`build_padded`/`naive_chunk`/`ts381_padded_20s_snapshot`）、`fix388`/`testsync388`/`trim388`、`seg389`/`fix389`/`testsync389`、`fix390`/`testsync390`、`shared_queue`/`drive_acc_windows`（并发 2→1）、`plan_windows`/`testsync386`/`testsync371`、`localrt384`/`ts384385` 共 **96 条 ok、0 失败**，无停手。
- **BUILD-390**：Step1 残 0 → Step2 npm 677ms + Tauri 1m34s（17w，**完整重做未复用**）→ Step2c UI cp（19:35 / `3a531013…`）→ Step3 主程序 2m47s（97w + crash 9w）→ Step4 三 exe→Publish。产物 main `52bd009437a1…`（14,870,016B/19:38:48）/ ui `3a531013225f…`（10,050,048B/19:35:55）/ crash `d62b82cc3bf0…`（24,879,104B/19:37:02）；两副本全等、三者均异于 BUILD-387。
- **八项逐项 PASS**：①时间戳 19:35–19:38 ②两副本 sha 相等且异于上包 ③ProductVersion **0.9.3** ④冒烟 PID **21560 Responding=True** / 两处无新 crash.json / panic 0 / 残 0 ⑤config `da2be5da…` 三时点不变 ⑥warnings 97/88/17 ⑦正探针 `[LocalRT-DBG-388]`=2 / `[LocalRT-DBG-389]`=4 / `max_new_tokens=`=1，反探针 `nearfield gate: vad=on`=**0** ⑧scene/itn 两 toml 三副本全等。
- **三特殊点（仅核验）**：① dll 四张三副本（`sherpa-onnx-lib`/`Publish`/`target-release`）全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本 `60b227de…` 全等；③ `Publish/models/` 1.7B 七文件与源 sha256 全等、0.6B 保留。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启 + 带 `-debug` 端测**。红线：未 push（需 Gavin 指示）/ 版本号未动 / 未改生产代码 / 未 `cargo clean` / 零凭证。

## 2026-09-23 — tester-1 — TEST-EXEC + BUILD-387 ✅ 出包（386/387 合包；八项 + 三特殊点全 PASS）

- **交付源码**：HEAD `3646b4d`（`4168960`/`2d972ac`/`69d046d`/`5cd14aa`/`a65ef9b`/`3646b4d`），工作区 clean，版本 0.9.3。核心单 386（中途末片延后组窗 / 松键短尾合并重解 / 预览回灌保留流式尾巴 / 失败窗流式兜底）/ 387（念词表·`<标签>`·空输出判无效后不带注入重解 + 近场门 300ms 平滑音量）。DEC-081：任务书已下达「现在可以出包」。
- **阶段四（全量未过滤；fmt `--check` 不带 `skip_children`）**：root `cargo test --no-fail-fast` **1620P/0F/34I**（EXIT 0；`feiyin-ime` bin **1532P/32I**）；`src-tauri` **92P/0F/0I**；Vitest **SKIP**（`ui/` 无 diff）；warnings **97/88/17** = 基线；fmt **EXIT 0**。
- **NEW/GONE（精确集合差）**：基线 1598P/34I ⇒ 净 **+22P/+0I**。NEW **29**（`plan_windows_386_tests` 8 / `fix386_tests` 2 / `slice_streaming_386_review_tests` 1 / `testsync386_tests` 5 / `fix374_terms_echo_tests` 1 / `fix387_output_guard_tests` 5 / `guard387_review_tests` 1 / `local_stream::tests` 3 / `testsync387_tests` 3）；GONE **7**（旧 `reflow_preview_367_tests` 2 + 旧 `plan_windows_382_tests` 4 + `ladder_uses_remaining_and_never_redecodes`）。⚠️ 任务书预估 28/2，实测 **29/7**（差异全为模块重写，净 +22P 逐位吻合）。
- **重点失效模式**：其他管线（`build_padded`/`naive_chunk`/`ts381_padded_20s_snapshot`）、`plan_windows`/`testsync386`/`testsync371`/`testsync382`、`fix387`/`testsync387`/`guard387`、`localrt384`/`nearfield385`/`ts384385`、368~382 共 **211 条 ok、0 失败**，无停手。
- **BUILD-387**：Step1 残 0 → Step2 npm 666ms + Tauri 1m31s（17w）→ Step2c UI cp（17:07 / `1a8650c0…`）→ Step3 主程序 2m50s（97w + crash 9w）→ Step4 三 exe→Publish。产物 main `a592182ca12f…`（14,839,808B/17:10:09）/ ui `1a8650c06fcb…`（10,050,048B/17:07:14）/ crash `99096bbcc139…`（24,879,104B/17:08:21）；两副本全等、三者均异于 BUILD-385。
- **八项逐项 PASS**：①时间戳 17:07–17:10 ②两副本 sha 相等且异于上包 ③ProductVersion **0.9.3** ④冒烟 PID **1856 Responding=True** / 两处无新 crash.json / panic 0 / 残 0 ⑤config `da2be5da…` 三时点不变 ⑥warnings 97/88/17 ⑦二进制正探针 `[LocalRT-DBG-386]`=1 / `[LocalRT-DBG-387]`=2 / `[LocalRT-DBG-385]`=2 / `[LocalRT-DBG-382]`=3，反探针无 ⑧scene/itn 两 toml 三副本全等。
- **三特殊点（仅核验）**：① dll 四张三副本（`sherpa-onnx-lib`/`Publish`/`target-release`）全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本 `60b227de…` 全等；③ `Publish/models/` 1.7B 七文件与源 sha256 全等、0.6B 保留。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启后端测**。红线：未 push（需 Gavin 指示）/ 版本号未动 / 未改生产代码 / 未 `cargo clean` / 零凭证。

## 2026-09-23 — tester-1 — TEST-EXEC + BUILD-385 ✅ 出包（381/382/384/385 合包；八项 + 三特殊点全 PASS）

- **交付源码**：HEAD `0c443a5`（`1af7212`/`f37b767`/`5f19eca`/`e8e7b6c`/`f8367a6`/`0c443a5`），工作区 clean，版本 0.9.3。核心单 381（滑窗字缝切 + `WINDOW_MAX_SECS` 12→10）/ 382（逐片组窗 + 早显处理态 + 回灌提速）/ 384（静默判定改 silero VAD + 音量兜底）/ 385（近场音量门）。DEC-081：任务书已下达「现在可以出包」。
- **阶段四（全量未过滤；fmt `--check` 不带 `skip_children`）**：root `cargo test --no-fail-fast` **1598P/0F/34I**（EXIT 0；`feiyin-ime` bin **1510P/32I**）；`src-tauri` **92P/0F/0I**；Vitest **SKIP**（`ui/` 无 diff）；warnings **97/88/17** = 基线（384 起主程序 98→97）；fmt **EXIT 0**。
- **NEW/GONE**：基线 1584P/33I ⇒ 净 **+14P/+1I**。NEW 15（14P+1I）= 384 **5**（`guard384_*` 1 + `localrt384_*` 3 + ignored `localrt_vad_feed_drains_queue_bounded`）+ 385 **5**（`nearfield385_*`）+ TEST-SYNC-384-385 **5**（`ts384385_*`）；**GONE 0**。
- **重点失效模式**：其他管线（`build_padded`/`naive_chunk`/`should_segment`/`ts381_padded_20s_snapshot`）、滑窗（`plan_windows`/`plan_gap_cuts`/`ordered_reflow`/`align`）、静默判定（`localrt384`/`guard384`/`nearfield385`）、回灌（`reflow_fast`/`testsync382`/`problem2_order`/`shared_queue`/`drive_acc_windows`）、368/369/371/374/375/377/380 共 **92 条 ok、0 失败**，无停手。
- **BUILD-385**：Step1 残 0 → Step2 npm 656ms + Tauri 1m52s（17w）→ Step2c UI cp（14:52 / `d51a599c…`）→ Step3 主程序 2m59s（97w + crash 9w）→ Step4 三 exe→Publish。产物 main `dc88612f4aaf…`（14,818,816B/14:55:58）/ ui `d51a599cafc4…`（10,050,048B/14:52:53）/ crash `ea5beaf8c0f1…`（24,879,104B/14:54:05）；两副本全等、三者均异于 BUILD-380。
- **八项逐项 PASS**：①时间戳 14:52–14:55 ②两副本 sha 相等且异于上包 ③ProductVersion **0.9.3** ④冒烟 PID **29592 Responding=True** / 两处无新 crash.json / panic 0 / 残 0 ⑤config `da2be5da…` 三时点不变 ⑥warnings 97/88/17 ⑦二进制正探针 `[LocalRT-DBG-382]`=3 / `stop_to_processing_ms`=1 / `reflow suppressed after processing`=1 / `[LocalRT-DBG-384]`=4 / `[LocalRT-DBG-385]`=2，反探针无 ⑧scene/itn 两 toml 三副本全等。
- **三特殊点（仅核验）**：① dll 四张三副本（`sherpa-onnx-lib`/`Publish`/`target-release`）全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本 `60b227de…` 全等；③ `Publish/models/` 1.7B 七文件与源 sha256 全等、0.6B 保留。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启后端测**。红线：未 push（需 Gavin 指示）/ 版本号未动 / 未改生产代码 / 未 `cargo clean` / 零凭证。

## 2026-09-23 — tester-1 — TEST-EXEC-380 + BUILD-380 ✅ 出包（阶段四全绿 → 出包；八项 + 三特殊点全 PASS）

- **交付源码**：HEAD `cc83917`，工作区 clean，版本 0.9.3。核心单 `FIX-PREVIEW-HARVEST-380`（路 B 窗口解完即 `push_window` 合并刷新预览 + `[LocalRT-DBG-380]` 松键时延埋点）。DEC-081：任务书已下达「现在可以出包」。
- **阶段四（全量未过滤；fmt `--check` 不带 `skip_children`）**：root `cargo test --no-fail-fast` **1547P/0F/31I**（EXIT 0；`feiyin-ime` bin **1459P/29I**）；`src-tauri` **92P/0F/0I**；Vitest **SKIP**（`ui/` 无 diff）；warnings **98/88/17** = 基线；fmt **EXIT 0**。
- **NEW/GONE**：基线 1546P/31I ⇒ 净 **+1P** = 新增 `preview_harvest_380_tests::drive_acc_windows_harvests_result_while_acc_open`（bin 1458→1459P 逐位吻合）；GONE 无。
- **重点失效模式**：`ordered_reflow`/`align`/`strip_terms_echo`/`apply_acc_disposition`/`output_rate_ok`/`reflow_monotonic_key`/`preview_harvest_380` 命中 **38 条 ok、0 失败**，无停手。
- **BUILD-380**：Step1 残 0 → Step2 npm 1.55s + Tauri 1m48s（17w）→ Step2c UI cp（两处 12:32 / sha `21bf38fd7f3a…` 一致）→ Step3 主程序 3m00s（98w + crash 9w）→ Step4 三 exe→Publish。产物 main `266cd61bc23e…`（14,781,440B/12:35:59）/ ui `21bf38fd7f3a…`（10,050,048B/12:32:54）/ crash `6011c8cb2e68…`（24,879,104B/12:34:17）；两副本全等、三者均异于 BUILD-379。
- **八项逐项 PASS**：①时间戳 12:32–12:35 ②两副本 sha 相等且异于上包 ③ProductVersion **0.9.3** ④冒烟 PID **23452 Responding=True** / 两处无新 crash.json / panic 0 / 残 0 ⑤config `da2be5da…` 三时点不变 ⑥warnings 98/88/17 ⑦二进制正探针 `[LocalRT-DBG-380]`=3 / `hook_to_controller_ms`=2 / `stop_to_inject_ms`=1，反探针无（本单未删字面量）⑧scene/itn 两 toml 三副本全等。
- **三特殊点（仅核验）**：① dll 四张三副本（`sherpa-onnx-lib`/`Publish`/`target-release`）全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本全等；③ `Publish/models/` 1.7B 七文件与源 sha256 全等、0.6B 保留。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启后端测**。红线：未 push（需 Gavin 指示）/ 版本号未动 / 未改生产代码 / 未 `cargo clean` / 零凭证。

## 2026-09-23 — tester-1 — TEST-EXEC-377 + BUILD-379 ✅ 出包（阶段四全绿 → 出包，替换作废的 BUILD-373；八项 + 三特殊点全 PASS）

- **交付源码**：HEAD `243dcc4` + 工作区阶段三 18 条护栏（未提交），版本 0.9.3。🔴 **BUILD-373 作废**（词条回显 + 预览刷不进 + 出字延迟）。DEC-081：Gavin 已预授权「全绿即直接出包」。
- **阶段四（全量未过滤；fmt `--check` 不带 `skip_children` EXIT 0）**：root `cargo test --no-fail-fast` **1546P/0F/31I**（EXIT 0；`feiyin-ime` bin **1458P/29I**）；`src-tauri` **92P/0F/0I**；Vitest **7 files/100P/11S/0F**；warnings **98/88/17** = 基线。
- **NEW/GONE（基线 1423P/28I ⇒ 净 +35P/+1I = 1458P/29I 逐位吻合）**：374/375 **+20P** + 377 **净 -3P/+1I**（删 3 条 `ctx320_*` ctx 注入单测 + 1 PoC `#[ignore]`）+ 阶段三 **+18P**。GONE 3 条（随 377 删 `Context:`/`Terms:`/`last_n_chars`/`merge_ctx_timeline` 特性移除）。⚠️ 任务书按 377「删 4」记，实测净 -3P/+1I，**总数一致**（差异仅在拆法，如实报）。
- 🔴 **重点失效模式**：`strip_terms_echo`/`apply_acc_disposition`/`output_rate_ok`/`reflow_monotonic_key`/`build_ctx_system`/`align`/`ordered_reflow`/`strip_qwen3` 相关**全通过 0 失败**，无停手条件。
- **BUILD-379**：Step1 残 0 → Step2 npm 667ms + Tauri 1m43s（17w）→ Step3 主程序 2m58s（**98w** + crash 9w）→ Step4 UI 同步 + 三 exe→Publish。产物 main `7e14a0fee986…`（14,765,568B/01:13:03）/ ui `e4f1c53298f8…`（10,050,048B/01:10:03）/ crash `926ed04bd72f…`（24,879,104B/01:11:08）；两副本全等、三者均异于作废的 BUILD-373。
- **八项逐项 PASS**：①时间戳 01:10–01:13 ②sha ③**0.9.3** ④冒烟 PID **12852 Responding=True** / 两处无新 crash.json / panic 0 / 残 0 ⑤config `da2be5da…` 三时点不变 ⑥warnings 98/88/17 ⑦探针 ⑧四表三副本全等。
- **探针**：源码级正 `build_ctx_system`(2)/`strip_terms_echo`/`apply_acc_disposition`/`output_rate_ok`/`reflow_monotonic_key`/`QWEN3_PREFIX_MAX_BYTES`/`align_overlap_with_prior`/`AlignPrior` 均 ≥1；二进制正 `SLIDING-WINDOW-367`=4/`[LocalRT-DBG-298]`=3；🔴 反 `CLEANUP_INSTR_EN`/`CTX_INSTR_EN`/`merge_ctx_timeline`/`CTX_DEFAULT_CHARS`/`ctx_prev1`/`ctx_prev2`/`is_qwen3_language_label` **二进制全 0**（源码命中全为注释/PoC 测试常量/测试护栏 ⇒ 无生产符号）。
- **三特殊点（仅核验）**：① dll 四张三副本全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本全等；③ `Publish/models/` 1.7B 七文件与源 sha256 全等、0.6B 保留。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启后端测**。
- **红线**：未 push（需 Gavin 指示）/ 版本号未动 / 未改生产代码 / 未 `cargo clean` / 零凭证。

## 2026-09-23 — tester-1 — TEST-SYNC-377 ✅ 交付（阶段三 · 非作者视角补 18 条护栏；生产代码零改动）

- **性质**：阶段三 TEST-SYNC，**按设计契约写用例**（不读实现反推），只改 `#[cfg(test)]` 区。目标 = `FIX-INJECT-TO-SPEC-377`（注入按 sherpa 规格砍成纯逗号词表）+ 374/375/371/368/369 不回归。
- **新增 18 条**（`src/transcription/mod.rs` +211、`src/main.rs` +38）：
  - **A `build_ctx_system` 9 条**：纯词表逐字返回 / 仅 trim 首尾 / None·空·纯空白·纯换行⇒None / 逗号结构原样（连续·尾随·只有逗号·重复）/ 产出不含指令句·`Context:`·`Terms:` / 不组装多行 / 含换行不崩（原样保留）/ 注入门 / 空⇒token 估算 0。
  - **B 374/375 不回归 3 条**（独立夹具）：新格式裸词表回显仍命中 ≥4 并剥空 / 句中 1~2 词不剥 / `output_rate_ok` 冷启动三路径（无均值·无音频·期望产出少）⇒ 判正常。
  - **C 371/368/369 不回归 4 条**：64B 位置护栏+只截首个+中段不剥 / 周期不丢 / 零重叠拼接 / 对齐失败不丢滑出文本。
  - **D 源码级护栏 2 条**（`testsync377_source_guard_tests`，`include_str!` + `guard_prod_lines`）：`transcription/mod.rs` 生产区（注释剔除）无 `CLEANUP_INSTR_EN`/`CTX_INSTR_EN`/`CTX_DEFAULT_CHARS`；`main.rs` 生产区 `ctx_prev1`/`ctx_prev2` 计数 0。
- **验证（白名单）**：`cargo fmt` clean / `--check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **98/88** = 基线未升；`numstat`==`-w`（38/0、211/0）。🔴 **未跑 `cargo test`**（首跑在阶段四）。
- **发现**：**未发现生产缺陷**。初查 `CLEANUP_INSTR_EN` 等「1 文件命中」经逐行核实为**注释 + PoC 测试常量**（`_377` 后缀，`#[cfg(test)]` 内），非生产残留。
- ⚠️ **设计-实现张力（待主控裁）**：设计括注「词表含换行**不得带进** system 段」与主契约「trim 后**原样返回**」在**内部换行**上冲突；实现按「原样保留」，本单按主契约写断言并上报，**未擅自改绿**。若需硬清换行 ⇒ 属生产改动，请派 coder。
- **红线**：未改生产代码 / 未 commit / 未 push / 版本号未动 / 零凭证。

## FIX-TERMS-ECHO-374 + FIX-PREVIEW-STALE-AND-COLLAPSE-375 + POC-376（coder-1，2026-09-23）

- **374**：结构性判据（连续 ≥4 注入词条、顺序一致）剥词条回显；**未**把 Terms 放回 LCS（43% 误判来源）；
  判据在 ctx 护栏外恒执行 ⇒ 堵住「重启后首次录音 `ctx_raw_len=0` 跳过护栏」。
- **375-A**：`reflow_action` 的 `seg_index` 单调闸对滑窗不适用（切片下标会重复）⇒ 新增 `reflow_seq`。
  🔴 **自 367 起存在（BUILD-347），非 371 引入**。
- **375-B**：新增与回显无关的**产出率**判据识别解码坍塌；冷启动宁漏勿误杀。
  两条合并一套阶梯 `apply_acc_disposition`，至多重解一次、必须不带注入。
- **POC-376**：**主控的 language 推理被证伪** —— 设 `language` 不消回显、只换回显内容。
  官方一手资料：`language=None` 是主用法（评测全程不设）、前缀是官方输出格式（有 `parse_asr_output`）、
  官方 `transcribe()` 无上下文参数、sherpa 的 hotwords 期望「ASCII 逗号分隔词表」却被我们塞了
  指令句+散文。🔴 **换 Qwen3 后词库偏置是否仍有效，至今无证据**（PoC 音频无词表专名发音点）。
- 主控逐条 Read 验收通过；fmt/check 复跑干净；全量 test 0 failed（1443P/28I）；warnings 88 = 基线。

## 2026-09-23 — coder-1 — FIX-PREVIEW-HARVEST-380 ✅ 交付（阶段一·只改代码）

- **缺陷**（Gavin BUILD-379 端测）：① 预览中途/尾部已改对却不刷新，停顿数秒也不刷；② 松键后约 4s 才注入。
- **A（问题①根因）**：`main.rs` 路 B 滑窗线程 `for … in acc_rx` 阻塞等下一片，解码结果只在「下次派发」时才 `try_recv` ⇒ 停顿无新片 ⇒ 结果不回灌（6 次录音解出→上屏滞后 2~17.6s，最后一窗每次等松键）。
- **A 修法**：模块级 `drive_acc_windows(acc_rx,res_rx,cancelled,&mut step)` 用 `crossbeam_channel::select!` 同时等两通道，**任一先到即处理**；滑窗线程改单个 `step` 闭包；两份收取合并 `harvest_acc_window!` 宏 ⇒ 该线程 `push_window(` 仅一处；`done` 计入 select 已收，收尾只等未收。**未动**切分/对齐/`push_window` 合并规则/`replace_all` 语义/派发（1200ms）/并发/坍塌判据/词表注入/取消语义。
- **B（问题②定位埋点）**：hotkey.rs 对目标键 **DOWN/UP 都记** `KBDLLHOOKSTRUCT.time`；`take_last_hook_event_tick()` take 语义（读即清，防陈旧）。controller Start/Stop 打 `hook_to_controller_ms`；非钩子路径（RegisterHotKey/`poll_ptt_release_thread`）打 `n/a`（不用 0/陈旧 tick 冒充）。worker `Injection completed` 打 `stop_to_inject_ms`。全部 `log_enabled!(Debug)` 守卫（DEC-077）。B 仅 Windows（macOS 结论：**不适用**，见 `docs/MACOS-HANDOFF.md`）。
- **单测**：+1 `preview_harvest_380_tests::drive_acc_windows_harvests_result_while_acc_open` —— 用**通道先后**制造「`acc_rx` 未关、结果已到」，断言 step 在 acc 关闭前被调；**非 sleep 定时序**（`recv_timeout` 仅作挂死兜底）；退回旧「只在收切片时收结果」语义必超时。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings **98/88** = 基线；全量 `cargo test --no-fail-fast` **1547P/0F/31I**（EXIT 0）；368/369/371/374/375/377 既有单测全绿。numstat：main 257/118、hotkey 20/0、platform/mod 1/1、windows/mod 4/1（-w main 254/115，3 行 whitespace 落改动块内）。
- **未改版本号 / 未 push / 未 build release / 零凭证**。🔴 实机时延读数交 tester-1/Gavin，未声称已验证。

## 2026-09-23 — coder-1 — FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382 ✅ 交付（阶段一·只改代码）

- **问题1（P0 吃前文）**：Gavin 一口气 18.37s ⇒ 只出末尾一句。根因：一次 `Slice` 带 2 片（13s+5.37s），旧实现 2 片一起 push 后只组**一个**窗口，`group_window_start_secs` 丢最远片 ⇒ 13s 那片从不进任何窗口、永不解码。
- **问题1 修法**：Slice 分支改**逐片组窗**（N 片 ⇒ N 窗，每窗以当前片收尾）；新增纯函数 `plan_windows(prev_durs,new_durs,prev_base)`（主控指定放 `main.rs`），生产调用 + 单测断言**覆盖不变量**（每个新片都被某窗覆盖；每窗 end-1 = 对应新片）。**未动** `group_window_start_secs` / `WINDOW_MAX_SECS` / `WINDOW_MAX_SLICES` / `push_window` / `OrderedReflow` / 对齐规则。
- **问题2（松键后处理态出得晚）**：`StreamingFinalPreview`（原 `:8789`）+ `Processing`（run_pipeline_core）都排在 `asr_join`+`acc_join` 之后 ⇒ B 尾窗解码期界面停在录音态。修法：`asr_handle.join()` 后、`acc_handle.join()` 前立即发 预览→处理态（282 顺序、文案同源 `i18n::get(..).overlay_processing`）；删 join 后那次预览发送（防闪回）；埋点 `[LocalRT-DBG-380] stop_to_processing_ms`（复用 `STOP_RECEIVED_TICK`，仅 Windows）。
- **问题2 防闪回（主控裁定②）**：controller 处理到本代 `Processing` 事件时置 `ACC_REFLOW_SUPPRESS` ⇒ 其后 `replace_all` 回灌**只更新权威状态、不重画浮层**（排前面的照常渲染）；`RecordingStarted` 复位；只影响本地实时档。单测 `suppressed_after_processing_skips_render_only`。
- **问题3A（回灌不等边界配对）**：`replace_all` 回灌 `Applied` **立即渲染**（`ReflowFastState` 纯状态机）：边界已知⇒准确的；未知⇒派发当刻 `committed_len`（Slice `_committed_len` → `window_committed_lens` → `PreviewReflow.committed_len`）；边界按 `(gen,seg)` 小 map 记录，后到且该 seg 仍最新已渲染 ⇒ 准确值再渲一次；`boundary=b` 不再扣下。老非 replace_all 路径保留、逐位不变（`try_resolve_reflow` 保留，`ACC_REFLOW_ACC` 恒 None ⇒ no-op）。
- **问题3B（共享队列）**：`rr % concurrency` 固定派发 ⇒ 改**单一共享任务通道**（`task_rx.clone()` 多消费者），`WINDOW_DECODE_CONCURRENCY` 不变。
- **问题3C（埋点）**：`PreviewReflow` 新增 `decode_done_at: Option<Instant>`（worker 解完 `Instant::now()`，经 harvest 带入）；渲染时打 `[LocalRT-DBG-382] reflow latency: seq decode_done→render_ms boundary=known|fallback`；worker 打 `window queue: seq queued_ms decode_ms`。全 Debug 守卫。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings **98/88** = 基线；全量 `cargo test --no-fail-fast` **1563P/0F/33I**（新增 12 条 / GONE 0；368/369/371/374/375/377/380 全绿）。numstat main 776/144（-w 723/91）。
- 🔴 **踩坑（重要）**：`prod_lines_excluding_cfg_test` 跳过 `#[cfg(test)]` 项时用**朴素花括号计数、会把字符串里的 `{`/`}` 算进去** —— 我在新测试模块里写了 `contains("for _ in 0..concurrency {")`，那个 `{` 令扫描**越过模块边界、吞掉其后 `run_pipeline_core` 全部生产行**，导致 `nospeech_122`/`guard_214_215`/`testsync371` 共 5 条跨文件护栏假红。改为不含 `{` 的字串即愈。已记 `collab/troubleshooting.md`。**写测试字符串时禁含 `{`/`}`**（或与现有写法一样保证成对）。
- **未改版本号 / 未 push / 未 build release / 零凭证**。🔴 实机读数（回灌准实时 / 处理态时延 / 吃字）交 tester-1/Gavin，未声称已验证。

## 2026-09-23 — FIX-SLICE-CUT-AT-GAP-381（coder-2，✅ 阶段一交付：切片字缝化 + 窗口统一 10s）

- **Gavin 口径**：硬切可能切碎字 ⇒ 原「12s 窗 / 13s 硬切点」统一改 **10s**；超 10s 在 10s 后找能量低谷的**字缝**切。
- **改动文件**：`src/transcription/vad.rs`（删 `SLIDING_SLICE_MAX_SECS`；新增 4 常量 + `GAP_FRAME_SAMPLES`；`plan_gap_cuts`/`find_gap_cut`/`frame_rms`；`build_sliding_segments`/`plan_sliding_cuts`；`build_padded_segments_capped` 拆 `plan_hard_cuts`+`pad_and_extract` 逐位不变）、`src/transcription/local_stream.rs`（`build_dispatch_segment` 改调 `build_sliding_segments`；注释/单测改名）、`src/transcription/mod.rs`（`WINDOW_MAX_SECS` 12→10；re-export；`#[cfg(test)] mod poc_slice_cut_381;`）、**新文件** `src/transcription/poc_slice_cut_381.rs`。🔴 **未碰 `src/main.rs`**。
- **单测**：vad 9 条（低谷/兜底/尾巴/不丢不重/20s 逐位不变/增益不变/边界护栏）+ local_stream 3 条（改名 `sliding_slice_381_*`）+ `sliding_cut_search_start_equals_window_max`。2 条既有 group_window 断言随上限合法变化（[3,3,3,3] 起点 0→1；[4,4,4,4] 1→2，已注明）。
- **§4 实测**（`cargo test --bin feiyin-ime -- --ignored --nocapture poc_slice_cut_381_cer`）：字缝 CER **0.0356** vs 固定 10s 硬切 **0.0311/0.0400/0.1111/0.0800**（off 0/1.3/2.7/4.1），均值 0.0656；硬切边界出现重复「多」/丢「烧」/幻觉插入/乱码，字缝切无。🔴 **唯一反例 off=0（+0.0044=1 字）**待主控裁量。
- **§5 实测**（`... poc_window_10s_381`）：cap12 与 cap10 窗口完全相同（7 窗），ΔCER=**0.0000** ≤ 0.01 ⇒ 照 10s 交付。⚠️ 本音频无 ≥1200ms 静默 ⇒ 子片全 ≥10s，两档 cap 不可区分（真实带停顿录音才显现差异）。
- **验证**：`cargo fmt --check` EXIT 0 ｜ `cargo check --all-targets` **0 error**、warnings **88**=基线 ｜ 全量 `cargo test --no-fail-fast` **0 failed**（bin 1475P/31I）｜ `--numstat`==`-w`。
- **未验证**：实机端测（字缝切体感 / 长句 >10s）交 tester-1/Gavin，本单未声称已验证。
- 🔴 **跨文件待改（越界）**：`src/config/mod.rs:11`、`src/main.rs:8579`（及历史注释 `:8539/:11060`）仍写 13s，已列 result.md，请主控路由给 coder-1。
- **未改版本号 / 未 push / 未 build release / 零凭证**。

## 2026-09-23 — TEST-SYNC-382（coder-2，✅ 阶段三交付：非作者护栏 10 条）

- **性质**：只写测试、零生产改动；仅 `src/main.rs` `#[cfg(test)] mod testsync382_tests`。被测 `FIX-WINDOW-COVER-AND-EARLY-PROCESSING-382`（HEAD `1af7212`）。🔴 **未跑 `cargo test`**（阶段四 tester-1 首跑）。
- **新增 10 条**：`plan_windows` 性质（LCG 200 组覆盖/收尾/形状 + 200 组单片等价旧算法 + 空 new）；`ReflowFastState` 退化（跨代 / None→Some / 交错乱序 / suppressed 边界 / `clear()` 复位）；源码级（滑窗派发邻域无 `task_txs`·`% concurrency`·`rr %` + 正向 `task_rx.clone()`；`ACC_REFLOW_SUPPRESS.store(true` 恰一处且在 `Processing` 臂内）。
- **验证**：`rustfmt --config skip_children=true --check src/main.rs` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **88**=基线；numstat==-w（266/0）。独立 Python 端口 `prod_lines_excluding_cfg_test` 核实护栏前提 + 40 万组 `plan_windows` 模拟 0 反例（弥补不能跑单测）。
- **未发现生产缺陷**（无停手项）。**未改版本 / 未 push / 零凭证**。
- 🔴 **交阶段四注意**：全仓 `cargo fmt --check` 目前**仅在 `src/transcription/vad.rs:1572`**（另一 Worker 在飞的 TEST-SYNC-381 `ts381_*`，+298 行未提交）非 0；本单未触碰该文件，请主控确认其作者处理后再出包。

## 2026-09-23 — coder-1 — TEST-SYNC-381 ✅ 交付（阶段三·非作者护栏，只改 `vad.rs` 的 `#[cfg(test)]` 区）

- **被测**：`FIX-SLICE-CUT-AT-GAP-381`（coder-2，已验收 HEAD `1af7212`）。按**设计契约**写、不照实现反推；不重复作者 `gap_cut_*` / `sliding_segments_*` / `legacy_wrapper_*`。
- **范围**：`src/transcription/vad.rs` **仅** `mod tests`（+407/−0，生产代码零改动）；11 条 `#[test]`（`ts381_` 前缀）。
- **覆盖**：
  1. **性质**：确定性 xorshift64 + 正弦/噪声/静音混合（幅度 0.001~1.0）100 段（0.5~60s）⇒ 断言严格相接且并集==[start,end)、非末片∈[10,12]s、末片<11s（或整段<11s）、不 panic。
  2. **退化**：全零（阈值 0 ⇒ **取最早帧中心 = 10s+半帧**，精确断言）／NaN·±inf·全 NaN·全 +inf（不 panic、不死循环）／`end` 越界（`plan_gap_cuts` 不 clamp —— 记明 clamp 由调用方 `plan_sliding_cuts` 负责，只断言安全+结构自洽）／长度恰 10s·11s·11s+1 样本·12s（11s 系无帧中心落点 ⇒ 精确切 `lower`）。
  3. **字缝优先**：10.5s 浅静音 + 11.5s 更深静音 ⇒ 精确切 **10.5s（最早达标帧中心 168160）**，不得跳更深。
  4. **20s 路径逐位不变快照**（主控先作废后**恢复并强化**，最终按恢复版）：`build_padded_segments` 固定输入写死每段 `(前置 padding, 主段区间, 后置 padding)`，断言段数/长度 + 样本与原音频切片 **`to_bits` 逐位相等**；`naive_chunk` 同做切片逐位快照；基线 = **`1af7212^`**（已 `git show` 读 pre-381 源码确认 `build_padded_segments_capped`/`naive_chunk` 逻辑一致）。
  5. **其它边界**：非零且**非帧对齐** `start`／空·倒置区间／滑窗合并阈值 = **10s**（对比 20s 路径 20s）。
- **验证（白名单）**：`cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **88** = 基线；numstat == -w（407/0）。🔴 **未跑 `cargo test`**（任务书禁止，首跑在阶段四）。
- **结论**：**未发现生产缺陷**（NaN/全零无 panic；每轮切点 ≥ 起搜点 ⇒ 严格推进、无死循环）⇒ 无停手项。
- **未改生产代码 / 未改版本 / 未 push / 零凭证**。

## 2026-09-23 — LOCALRT-VAD-SILENCE-384（coder-2，✅ 阶段一交付：本地 realtime 静默判定改 VAD）

- **需求**（Gavin）：环境背景有声音时音量阈值判静默失效 ⇒ 本地 realtime 改 silero VAD 判「有没有人声」，音量阈值兜底（方案 A）；调用方式须最优化。
- **改动**：`vad.rs`（只新增：3 常量 + `try_new_for_local_silence` + `feed_is_speech`）；`local_stream.rs`（进程级缓存 `LOCALRT_VAD_CACHE` + 唯一判定 `chunk_has_speech` + 计时补偿 `localrt_vad_seed_ms` + 埋点 + 边界 b）；**`main.rs` 未改**（模型目录内部取 `transcription::model_dir()`）。
- **单测**：`localrt_vad_feed_drains_queue_bounded`（#[ignore] 真模型 300s，队列恒空 + 对照不排空会累积，PASS）/ `localrt384_fallback_matches_energy_threshold` / `localrt384_seed_is_min_silence_ms` / `localrt384_silence_timing_seeds_then_accumulates` / `guard384_single_speech_judgment_via_chunk_has_speech`。
- **验证**：fmt EXIT 0 ｜ check 0 error、warnings 88=基线 ｜ 全量 test **0 failed**（bin 1500P/32I）｜ numstat==-w。
- **未验证**：实机噪声环境端测（`silence detector=vad` / `vad cost` / 1200ms 触发一致性）交 tester-1/Gavin。
- **未改版本 / 未 push / 未 build release / 零凭证**。

## 2026-09-23 — LOCALRT-NEARFIELD-GATE-385（coder-2，✅ 阶段一交付：近场音量门）

- **需求**（Gavin）：本地 realtime 区分「录音人说话」与「背景人声」。方案 A：录音人离麦近音量大 ⇒ 在线学录音人音量做门。
- **改动（仅 `local_stream.rs`）**：`NearFieldLevel`（30s 窗口 / 80 分位 / 1s 热身）+ `nearfield_gate` / `vad_branch_decision` + `ChunkJudgment`；`chunk_has_speech` 加门（VAD 分支；兜底分支不加、同 384）；补偿只认 VAD 翻转；埋点。
- 🔴 关键决策：**先判门、后更新**（防背景人声自我放行）；已知局限：背景人一样近一样大分不开。
- **单测**：新增 4 条（level 窗口/分位、门就绪/热身、补偿只在 VAD 翻转、录音人→背景场景）全 PASS；384 用例不破。
- **验证**：fmt EXIT 0 ｜ check 0 error、warnings 88=基线 ｜ 全量 test **0 failed**（bin 1504P/32I）｜ numstat==-w。
- **未验证**：实机噪声/背景人声端测交 tester-1/Gavin（盯 `nearfield gate`/`summary` / 1200ms 一致性）。
- **未改版本 / 未 push / 未 build release / 零凭证**。只改本地 realtime，其它管线未动。

## 2026-09-23 — LOCALRT-NEARFIELD-GATE-385 第 1 轮退回修复（coder-2，✅ 已修）

- **退回问题**：`NearFieldLevel` 窗口按「已入样本累计时长」滑动 ⇒ 录音人中途降音量到 <0.25×level 后永不过门 ⇒ 窗口不滑、level 永久锁死 ⇒ 剩余录音全判静默。
- **修法**：样本记入样时刻 `now_ms`（会话音频 ms）；判门前 `prune(now)` 剔除早于 `now−30s` 的样本（按会话时间过期）；有效样本 <1s ⇒ 未就绪（门不生效、重新热身）；最坏锁定 ≤30s；「先判门、后更新」保留。
- **补测**：`nearfield385_window_expires_by_session_time_not_sample_duration`（PASS）；384 回退单测按新签名适配、断言不变。
- **验证**：fmt EXIT 0 ｜ check 0 error、warnings 88=基线 ｜ 全量 test **0 failed**（bin 1505P/32I）｜ numstat==-w（390/18）。仅改 `local_stream.rs`。

## 2026-09-23 — coder-1 — TEST-SYNC-384-385 ✅ 交付（阶段三·非作者护栏，只改 `local_stream.rs` 的 `#[cfg(test)]` 区）

- **被测**：`LOCALRT-VAD-SILENCE-384` + `LOCALRT-NEARFIELD-GATE-385`（coder-2，已交付）。按契约写、不与作者 `localrt384_*`/`nearfield385_*` 重复。
- **范围**：`src/transcription/local_stream.rs` **仅** `mod tests`（+238/−0，生产零改动）；5 条 `#[test]`（`ts384385_`）。
- **覆盖**：
  1. **兜底逐位**：VAD 不可用 ⇒ 随机 **500 组** (rms,thr) 判定恒等 `rms > thr`（含大量贴边/相等，**相等必 false**）；同一对值在「未就绪」与「就绪且 level=1000」下结论一致 ⇒ **兜底不受近场门影响**（`chunk_has_speech(None,..)` 生产真函数）。
  2. **近场门开关**：未就绪一律放行（含 rms=0）；就绪 `level=1.0` 下 **0.26 放行 / 0.24 挡住**；边界 `=level×ratio` 恰通过（`>=`）；VAD 非人声恒 false；附 `vad_branch_decision` 集成旁证。
  3. **锁死恢复**：学到 1.0 → 降到 0.2 ⇒ 28.5s 全挡、≤31.5s 门重开（最坏锁死 30s）、放行后 1.2s 重学 ≈0.2（`NearFieldLevel` + `vad_branch_decision` 生产真函数，无复刻）。
  4. **背景人声**：录音人 1.0 说 5s → 背景 0.1（VAD 真）3s ⇒ 背景段全判静默、**无 300ms 补记**、于背景开始后**恰 1200ms** 达 `should_dispatch_acc` 派发条件、level 不被污染。
  5. **补偿只认 VAD 翻转**：连续会话中门挡（VAD 仍真）⇒ 纯累加不补；VAD 真翻转 ⇒ 计时**被覆盖**为 300ms（非 +300）；VAD 不可用即便 prev_vad=true 也不补。
- **验证（白名单）**：`cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **97/88** ≤ 基线 98/88；numstat == -w（238/0）。🔴 **未跑 `cargo test`**（任务书禁止，首跑在阶段四）。
- **结论**：**未发现生产缺陷** ⇒ 无停手项。
- ⚠️ **说明**：三个静默计时分支内联在 `transcribe_streaming_local` 内（受 `guard346` 源码护栏约束不可抽函数）⇒ #4/#5 的计时推进按**契约三分支**复刻（同作者注释所承认）；判定类（兜底/门/门-未就绪）全部走**生产纯函数**。
- **未改生产代码 / 未改版本 / 未 push / 零凭证**。

## 2026-09-23 — coder-1 — FIX-TAIL-WINDOW-AND-FALLBACK-386 ✅ 交付（阶段一·只改 `main.rs`）

- **背景**（Gavin BUILD-385）：① 常念「维生素b12」；② 结尾缺字+`<location>`；③ 预览中途闪回更短；④ 短尾应与前/后窗重组。
- **A**：重写纯函数 `plan_windows(prev,new,base,pending,is_tail) -> WindowPlan{windows,pending}`：单片立刻组窗（含强制纳入 pending）；**≥2 片末片延后**；下次派发首个窗口起点强制 ≤ pending（可超 `WINDOW_MAX_SECS`）；松键收尾 pending <3s 且有前片 ⇒ `[p-1,p+1)` 重解前片、否则单独。滑窗线程新增 `pending_slice`/`recent_streaming`/`window_streaming_texts`/`last_dispatch_idx`/`last_committed_len`，`dispatch_window!` 宏统一派发（批次与收尾单一定义）；收尾在 `drop(task_tx)` 前。
- **B**：`render_authoritative_reflow` 改用 `compose_reflow_preview`（= `compose_with_acc_for_gen`：acc 全文 + `streaming[committed_len..]`）；删 `reflow_preview_367`（replace_all 丢尾 ⇒ 截短闪回）与其 2 条旧单测。
- **C**：窗口解码 Err/空 ⇒ `window_text_with_fallback` 用该窗流式文本兜底 + `[LocalRT-DBG-386]` warn；流式也空才空。
- **单测**：新增 `plan_windows_386_tests` 8 + `fix386_tests` 2；**更新**（非放宽）`testsync382_tests` 的 3 条 plan_windows 用例与 `testsync371_window_counter_guard_tests::counters_are_pushed_together`（386 走宏后 `window_spans.push`/`window_samples.push` 仍**恰 1 处且相邻**，锚点改 `contains`+相邻断言）。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings **97/88** = 基线；全量 `cargo test --no-fail-fast` **1608P/0F/34I**（EXIT 0）。numstat main 410/182（-w 404/176）。
- **未改版本 / 未 push / 未 build release / 零凭证**。🔴 同日 `local_stream.rs`/`transcription/mod.rs` 属 coder-2 的 387，非本单。

## 2026-09-23 — FIX-ACC-OUTPUT-GUARD-AND-GATE-SMOOTH-387（coder-2，✅ 阶段一交付）

- **需求**（Gavin）：词条 `维生素b12` 漏进正文 / `<location>` 当正文 / 查其它 bug。
- **改动**：`transcription/mod.rs`（D 输出守卫：回显一律重解不保留残余 / 残余全词表视同回显 / 标签守卫 / 空守卫 / 重解无效 ⇒ 空 + warn + `[DBG-387]` 埋点）；`local_stream.rs`（E 近场门改 300ms 平滑音量，O(1) `EnergySmoother`，门比较与 level 学习都用平滑值；兜底逐位同 384）。**未改 main.rs**。
- **单测**：`fix387_output_guard_tests` 5 条 + `ts387_smoothed_volume_not_gated_for_syllabic_speech` PASS；384/385 兜底测试适配签名、断言不变。
- **验证**：fmt EXIT 0 ｜ check 0 error、warnings 88=基线 ｜ 全量 test **0 failed**（bin 1520P/32I）｜ numstat==-w。🔴 过程：初期 386 在飞致树不可编译，按主控批示待命未动 main.rs；386 落地后复跑全绿。
- **未验证**：实机端测（端测盯 `[DBG-387] guard`、`[DBG-385] nearfield summary` gated 比例回落）交 tester-1/Gavin。
- **未改版本 / 未 push / 未 build release / 零凭证**。

## 2026-09-23 — coder-1 — TEST-SYNC-387 ✅ 交付（阶段三·非作者护栏，只改 `mod.rs` + `local_stream.rs` 的 `#[cfg(test)]` 区）

- **被测**：`FIX-ACC-OUTPUT-GUARD-AND-GATE-SMOOTH-387`（coder-2，HEAD `5cd14aa`）。按契约、不与作者 `fix387_*`/`ts387_smoothed_volume_*` 重复。
- **范围**：`src/transcription/mod.rs` **仅** `mod testsync387_tests`（+130/0）、`local_stream.rs` **仅** `mod tests` 追加（+115/0）；生产代码零改动。
- **5 条**：
  1. **标签边界**（`strip_angle_tags`）：剥 `<location>`/`</x>`/`<_a>`/正文夹标签（保留正文）；不剥 `<3岁`/`a<b`/`< 空格>`/`<你好>`/内容 31 字符/未闭合超 31；恰 30 字符剥。
  2. **回显残余**：末条残留（`维生素b12`）⇒ `Echo` 重解一次；句中恰含 1 词条 ⇒ 不触发、原样返回。
  3. **重解次数**：300 组随机输入 ⇒ 闭包至多 1 次、`redecoded==(calls==1)`、`invalid⇒空串`。
  4. **平滑音量**：音节 150ms 高能 + 50ms 近零 ⇒ 平滑波动 < 逐块波动；30 万块长跑有限、非负。
  5. **近场门抗误挡**：平滑门误挡 <5%、**原始逐块门误挡 >20%**（反证平滑收益）；背景（平滑 0.1×level）全部被挡。
- **验证（白名单）**：`cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **97/88** = 基线；numstat==-w。🔴 **未跑 `cargo test`**（禁止，首跑阶段四）。
- **未发现新生产缺陷**。⚠️ 已知「待修 2（387：重解只出标点被收下）」由主控后续修复；本单测试按**契约**写、未断言未修行为 ⇒ 修复后仍绿。「待修 1（386：兜底文本重复）」在 `main.rs`（coder-2 的 TEST-SYNC-386 在飞），非本单。
- **未改生产代码 / 未改版本 / 未 push / 零凭证**。

## 2026-09-23 — TEST-SYNC-386（coder-2，✅ 阶段三交付：非作者护栏 5 条）

- **性质**：只写测试、零生产改动；仅 `src/main.rs` `#[cfg(test)] mod testsync386_tests`。被测 `FIX-TAIL-WINDOW-AND-FALLBACK-386`（HEAD `4168960`）。🔴 **未跑 `cargo test`**（阶段四 tester-1 首跑）。
- **新增 5 条**：会话级性质（200 次 × 3~8 派发 × 1~3 片 × 0.3~12s、末次收尾：全覆盖 / 无 pending / `s<e≤总片数`）；中途一大一小含 pending；结尾一大一小（短尾合并 vs 长尾单独）；预览随 streaming 增长不回退且与同参数 StreamingText 渲染逐字相等；兜底（空/非空/双空/纯空白）。
- **验证**：`rustfmt` + `cargo check --all-targets` **0 error**、warnings **88**=基线；`cargo fmt --check` EXIT 0；numstat==-w（main 139/0）。独立 Python 复刻 `plan_windows` 校验用例 2/3 逐条吻合 + 5000 会话属性模拟 bad=0（弥补不能跑单测）。
- **未发现生产缺陷**。**未改版本 / 未 push / 零凭证**。
- 🔴 注意：同工作区 `local_stream.rs` 有 coder-1 在飞的 TEST-SYNC-387（+112 行未提交），非本单、未触碰。

## 2026-09-23 — coder-1 — FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388 ✅ 交付（阶段一，只改 `transcription/mod.rs` + `vad.rs`）

- **背景**（Gavin BUILD-387）：松键后处理久、模型频繁低级错误。根因：带 hotwords 的 Qwen3-ASR 遇静音吐热词（sherpa #3509），派发片把长停顿带进窗口。
- **A**：`vad.rs` 只新增 `LOCALRT_TRIM_PAD_SECS(0.2)` / `try_new_for_local_trim` / `speech_ranges`（`reset→accept 整段→flush→front/pop→clear+reset`）；`mod.rs` 新增 `trim_to_speech` 纯函数 + 线程级 `thread_local!` VAD 缓存；`transcribe_acc_ctx` 开头剪静音，**整窗无语音 ⇒ 早退 `Ok(("", true))`**（交 386 流式兜底，不进模型），首解/重解/产出率时长全用剪后 `samples`。
- **D1**：新增 `has_content`（≥1 `is_alphanumeric`）；首解无内容 ⇒ `Tag`/`Empty`（取代 `is_only_punct`，覆盖 `**`）；重解后 `acceptable` **所有 kind 统一** `has_content && !still_echo && output_rate_ok`。
- **D2**：`output_rate_ok` 冷启动（`None`/`NaN`/`0.0`/`±inf` 均按无均值）且 `audio_secs ≥ COLD_MIN_AUDIO_SECS(3.0)` 且 `< COLD_MIN_CHARS_PER_SEC(1.0)` ⇒ 坍塌；`<3s` 不判；不 panic/不除零。
- **单测**：新增 `fix388_trim_and_floor_tests` 7 条 + `#[ignore]` 真模型 `speech_ranges` 1 条。**契约变更**（主控已裁定同意）：更新 6 条既有用例期望 —— D1：「重解」四条从「收下短文本」改「invalid 空串」；D2：两条冷启动 `None/NaN/0.0/±inf @ ≥3s` 从 `true` 改「坍塌」。**「重解至多一次」等不变量断言一条未动**；每条变更处注释 `【388 契约变更】` + old→new。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings **97/88** = 基线；全量 `cargo test --no-fail-fast` **1621P/0F/35I**（EXIT 0）。numstat：mod.rs 377/24、vad.rs 70/0（== -w）。
- **未改版本 / 未 push / 未 build release / 零凭证**。🔴 同工作区 `main.rs`/`local_stream.rs` 属 coder-2 的 389（非本单）；期间曾因其在飞不可编译/单测红，待其转绿后复跑全量得 0F。

## 2026-09-23 — FIX-NEARFIELD-BY-SEGMENT-AND-PREVIEW-389（coder-2，✅ 阶段一交付）

- **需求**（Gavin BUILD-387）：近场门仍误挡 ~40%（不能直接关，优化算法）；预览回灌仍「缩短后又恢复」。
- **改动**：`local_stream.rs`（C 整句段门 `SegmentGate`/`SegmentPeakLevel`；C2 跨录音沿用 `LOCALRT_CARRY_LEVEL` + `with_seed`/拒段丢弃/写回 + 新增 `vad_device` 入参）；`main.rs`（D3 `PreviewReflow.boundary_usable` + `partial_win_committed` + `ReflowFastState` 部分窗不截短；调用点传 device）。🔴 未改 `mod.rs`/`vad.rs`（coder-1 的 388）。
- **单测 +14**（旧逐块门单测改写为新算法版本）。
- **验证**：fmt EXIT 0 ｜ check 0 error、warnings 97/88=基线 ｜ 全量 test **0 failed**（bin 1540P/33I）｜ numstat main 180/28、ls 495/685（-w 178/26、473/663，差额为替换块内缩进重排）。
- **未验证**：实机端测（`[LocalRT-DBG-389] nearfield summary` rejected 比例、`carry level`、预览不缩短）交 tester-1/Gavin。
- **未改版本 / 未 push / 零凭证**；`docs/MACOS-HANDOFF.md` 未改（结论交主控合入）。

## 2026-09-23 — coder-1 — TEST-SYNC-389 ✅ 交付（阶段三·非作者护栏，只改 `local_stream.rs` + `main.rs` 的 `#[cfg(test)]` 区）

- **被测**：`FIX-NEARFIELD-BY-SEGMENT-AND-PREVIEW-389`（coder-2，HEAD `af3a0ad`）。按契约、不与作者 `ts389_*`/`seg389c2_*`/`fix389_*` 重复。
- **范围**：`src/transcription/local_stream.rs` **仅** `mod tests` 追加（+134/0）、`src/main.rs` **仅** `fix389_partial_window_tests` 追加（+47/0）；生产代码零改动。
- **5 条**：
  1. **整句不切**：level=1.0，段内 `[0.8, 0.05×10, 0.7, 0.02×20]`（VAD 全真）⇒ 第一个高值后到段末全部有声。
  2. **背景整句挡住**：峰值恒 0.2 < 0.3×1.0 ⇒ 全 false，且峰值不进学习样本、`rejected` +1。
  3. **防锁死**：学到 1.0 后整句 0.25（<0.3）连续 35s ⇒ 30s 内全被拒；样本按会话时间过期后回未就绪 ⇒ 整句确认并重学 ≈0.25。
  4. **seed 流程**：seed=1.0 首句 0.5（≥0.3×seed）⇒ 确认；连续两句 0.1 ⇒ `seed_dropped`、第三句未就绪直接确认；`seed_usable` 同设备 600s true / 601s·换设备 false。
  5. **部分窗折算性质**：随机 500 组 `(prev ≤ cur、cum ≤ total)` ⇒ 恒 ∈ `[prev,cur]`、随 cum 单调不减、`cum=total⇒cur`、`total=0⇒cur`。
- **验证（白名单）**：`cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **97/88** = 基线；numstat==-w。🔴 **未跑 `cargo test`**（禁止，首跑阶段四）。
- **未发现生产缺陷**；**未改生产代码 / 未改版本 / 未 push / 零凭证**。

## 2026-09-23 — TEST-SYNC-388（coder-2，✅ 阶段三交付：非作者护栏 5 条）

- **性质**：只写测试、零生产改动；仅 `src/transcription/mod.rs` `#[cfg(test)] mod testsync388_tests`。被测 `FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388`（HEAD `af3a0ad`）。🔴 **未跑 `cargo test`**（阶段四 tester-1 首跑）。
- **新增 5 条**：`trim_to_speech` 性质（300 组随机：不增长/递增序/不丢语音）；首解空进 Empty 重解恰 1 次；`has_content` 全角/假名/emoji 边界；冷启动 2.99/3.0/3.01 + avg 非有限等价；重解至多一次性质（`invalid⇒空串`）。
- **验证**：`rustfmt` + `cargo check --all-targets` **0 error**、warnings **97/88**=基线；`cargo fmt --check` EXIT 0；numstat==-w（mod 149/0）。独立 Python 复刻 `trim_to_speech` 500 组性质 bad=0。
- **未发现生产缺陷**。**未改版本 / 未 push / 零凭证**。

## 2026-09-23 — coder-1 — TUNE-DECODE-SERIAL-AND-TOKEN-CAP-390 ✅ 交付（阶段一）

- **需求**（Gavin）：研究 B 路径窗口解码能否用框架并行接口优化性能。**依据**（主控查证）：sherpa 1.13.8 `DecodeStreams` 对 Qwen3 **无并行收益**（逐 `Decode`）；两解码线程共用同一 recognizer（ORT 8 线程池）**互相争抢** ⇒ 实测单独 **274** vs 并发 **476** ms/音频秒（每窗慢 74%、总吞吐仅 +15%）；预览按窗序回灌 ⇒ 单窗变慢直接推迟刷新。
- **改动1**：`WINDOW_DECODE_CONCURRENCY` **2 → 1**（注释写实测 + 「改回须附新实测」）；共享队列 / `drive_acc_windows` / 收尾逻辑不变（=1 自然退顺序）。
- **改动2**：新增 `TOKEN_CAP_PER_SEC=12`/`BASE=24`/`MIN=48`/`MAX=256` + 纯函数 `max_new_tokens_for(speech_secs)`（非有限/负 ⇒ 256；饱和防溢出）；`decode_accuracy_allow_empty` 增 `max_new_tokens: Option<i32>`；`decode_accuracy_once` 传 `None`（其它调用方逐位不变）；`transcribe_acc_ctx` 用**剪静音后**时长算 cap、首解与重解都带 `Some(cap)`（重解改走 `allow_empty`，空输出 `Ok("")` 与原 `unwrap_or_default` 语义一致）；`[DBG-388] trim` 追加 `max_new_tokens=`。不改任何 `pub` 签名。
- **main.rs**：`reflow_monotonic_key` 注释去具体并发值（中性表述，仅注释）。
- **单测**：新增 `fix390_tests` 3 条（并发度=1 / `max_new_tokens_for` 取值·界·非有限·负·单调·极大值不 panic / 源码护栏：首解重解都带 `Some(token_cap)` 且 `decode_accuracy_once` 传 `None`）。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings **97/88** = 基线；全量 `cargo test --no-fail-fast` **1642P/0F/35I**（EXIT 0）。numstat main 2/2、mod 138/10（== -w）。
- **未改版本 / 未 push / 未 build release / 零凭证**。

## 2026-09-23 — TEST-SYNC-390（coder-2，✅ 阶段三交付：非作者护栏 3 条）

- **性质**：只写测试、零生产改动；仅 `src/transcription/mod.rs` `#[cfg(test)] mod testsync390_tests`。被测 `TUNE-DECODE-SERIAL-AND-TOKEN-CAP-390`（HEAD `9ae53d2`）。🔴 **未跑 `cargo test`**（阶段四首跑）。
- **新增 3 条**：快语速不截断（s∈{0.5..25}，cap≥ceil(7s)+5 或 256，单调）；源码护栏（cap 在剪后遮蔽 samples 之后 + `decode_accuracy_once` 无 `Some(`）；并发度 ==1 且注释含实测依据 274/476。
- **验证**：`rustfmt` + `cargo check --all-targets` **0 error**、warnings **97/88**=基线；`cargo fmt --check` EXIT 0；numstat==-w（mod 81/0）。独立 Python 复刻 `max_new_tokens_for` 逐点 bad=0 + 源码锚点实测成立。
- **未发现生产缺陷**。**未改版本 / 未 push / 零凭证**。

## 2026-09-23 — coder-1 — TEST-SYNC-392 ✅ 交付（阶段三·非作者护栏，只改 `local_stream.rs` 的 `#[cfg(test)]` 区）

- **被测**：`FIX-GATE-TIMING-ONLY-392`（coder-2，HEAD `a752455`，含主控补 done 复位）。按契约、不与作者 `gate392_tests` 重复。
- **范围**：`src/transcription/local_stream.rs` **仅** `mod gate392_tests` 追加（+189/−1，生产零改动；那 1 行为补测模块 `use` 的 `should_dispatch_acc`）。
- **4 条**：① 门误判（has_speech 恒 false、vad_speech 真）一整句 ⇒ 内容标志置位，其后静默满 1200ms ⇒ `should_dispatch_acc` 恰一次 ② 背景 10s + 录音人 2s + 停顿 1.5s ⇒ 恰 2 次派发（背景仅一次，done latch）③ 首段不学（0.09 不进样本）+ 下中位（[0.02,0.02,0.025]⇒0.02；[0.3,0.1]⇒0.1）④ VAD 不可用 ⇒ `chunk_has_speech(None)` 两标志 == rms>thr，随机 300 组标志更新逐位一致。
- **方法**：测试内契约状态机 `Flags392`（复刻 `:1046-1090` + `:1432-1472`，循环内联不可抽函数）；判定走生产真函数。
- **验证（白名单）**：`cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **97/88** = 基线；numstat==-w。🔴 **未跑 `cargo test`**（禁止，首跑阶段四）。
- **未发现生产缺陷**；**未改生产代码 / 未改版本 / 未 push / 零凭证**。

## 2026-09-23 — coder-1 — FIX-VAD-FEED-BY-WINDOW-391 ✅ 交付（阶段一·只改 `vad.rs`）

- **缺陷**（Gavin BUILD-390，P0）：说话到一半卡住、预览与最终输出只出前半段、结果出错。根因（主控查 sherpa 源码）：388 的 `speech_ranges`/`feed_is_speech` 把整段一次性 `accept_waveform`；sherpa `voice-activity-detector.cc`「一次调用内全窗 OR、起点定在输入末尾前 ~0.164s」⇒ 5s 语音只剩 ~0.37s（日志 `in=6.31s out=0.37s`）。🔴 该错误用法出自主控 **384 任务书补充第 2 条**「整块一次喂、不要切 512」——本单纠正（原 `segment()` 逐 512 块才是正确用法）。
- **改动**（只 `vad.rs`）：新增 `feed_in_vad_windows(audio, accept)`（按 `VAD_WINDOW_SIZE`=512 逐块喂、末尾不足一块照常、返回 `ceil(len/512)`）；`speech_ranges` 与 `feed_is_speech` 改走它（`feed_is_speech` 每块后不查询、整块喂完只调一次 `detected()`）；注释写 sherpa 依据 + 注明 384 指示错误。未改常量/旧构造/`segment()`/pub 签名。
- **必须实跑的真模型验证**（`--ignored --nocapture vad391`，输出原样见 result.md）：① 4 段各 6s 语音 + 前后各 3s 静音 ⇒ 剪后 **6.44 / 6.31 / 6.38 / 5.78s**；② 对照旧整块写法 ⇒ **0.36s**（复现根因）；③ `feed_is_speech` 块大小 160/512/1600/16000 ⇒ 人声开始/结束差异均 ≤ 喂入块。
- **单测**：新增 `vad391_feed_in_vad_windows_counts_ceil`（纯） + 2 条 `#[ignore]` 真模型。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` **0 error**、warnings test **88**（`vad.rs` **0 新增**；bin 98 的 +1 属 coder-2 在飞的 392 `local_stream.rs:657`）；全量 `cargo test --no-fail-fast` **1646P/0F/37I**（EXIT 0）。numstat `vad.rs` 200/13。
- **未改版本 / 未 push / 未 build release / 零凭证**。

## 2026-09-23 — FIX-GATE-TIMING-ONLY-392（coder-2，✅ 阶段一交付）

- **需求**（Gavin BUILD-390）：「说一半卡住、只出前半段」根因修复。
- **根因**：门结果 `has_speech` 同时驱动时序与**内容去留** ⇒ 门误判（偶数上中位锁死 level）就丢录音人内容（342 丢流式 + 298 不派发）。
- **改动（仅 `local_stream.rs`）**：时序用 `has_speech`、内容去留改用 `vad_speech`；`estimate` 偶数取下中位；首段不学习；埋点 `learned=`/`vad_only_speech_chunks`；VAD 不可用兜底逐位同 384。
- **单测**：新增 `gate392_*` 4 条 + 改写 8 条（392 契约变更：首段不学习⇒就绪需 3 段 / 下中位 / vad_speech 取值）。
- **验证**：fmt EXIT 0 ｜ check 0 error、warnings 97/88=基线 ｜ 全量 test **0 failed**（bin 1562P/35I）｜ numstat==-w。
- **未验证**：实机端测（不再中途卡住 / 预览与最终完整 / `segment end learned=` / `vad_only_speech_chunks`）交 tester-1/Gavin。
- **未改版本 / 未 push / 零凭证**；未改 `docs/MACOS-HANDOFF.md`（结论交主控合入）。

## 2026-09-23 — TEST-SYNC-391（coder-2，✅ 阶段三交付：非作者护栏 3 条）

- **性质**：只写测试、零生产改动；仅 `src/transcription/vad.rs` `#[cfg(test)] mod testsync391_tests`。被测 `FIX-VAD-FEED-BY-WINDOW-391`（HEAD `a752455`）。🔴 **未跑 `cargo test`**（阶段四首跑）。
- **新增 3 条**：逐块覆盖性质（300 组逐样本拼接校验）、源码护栏（无直接整段 accept、必经 `feed_in_vad_windows`）、`#[ignore]` 旧写法反例（整块只落末尾 vs 逐块覆盖语音主体）。
- **验证**：`rustfmt` + `cargo check --all-targets` **0 error**、warnings **97/88**=基线；`cargo fmt --check` EXIT 0；numstat==-w（vad 160/0）。独立 Python 复刻 `feed_in_vad_windows` 5000 组 bad=0 + 源码护栏实跑成立。
- **未发现生产缺陷**。**未改版本 / 未 push / 零凭证**。

## 2026-09-23 — TRANS-NLLB-AND-SENTENCE-BATCH-394（coder-2，✅ 阶段一交付）

- **需求**（Gavin）：长文本翻译「被精简/偏离/离散」⇒ 换 NLLB + 优化调用 + 完整性硬要求（逐句一一对应 / 漏译检测重译 / 数字专名保留 / 真模型验收）。
- **模型**：`mijuanlo/nllb-200-distilled-600M-ct2-int8` → `models/nllb-200-distilled-600M-ct2-int8/`（4 文件 sha256 见 result.md；opus-mt 目录保留、代码不再加载）。
- **改动（仅 `src/translation/mod.rs`）**：NLLB 官方调用规格（src/tgt lang + target_prefix + 去首 token）；`split_sentences` 分句/子句；逐句批量；解码参数（beam4/lenpen1.0/norepeat3/rep1.1、删 min_decoding 强制、max=源tok×2+16≤256）；`looks_truncated` 漏译守卫 + 单句重译一次；数字/专名日志；`Arc<NllbModel>` 双向共享。pub 签名不变。
- **真模型实跑**：`--ignored trans394_real_model` **1P/0F/84.27s**（中→英 3 + 英→中 2；原句数==译句数、无漏译、数字保留）。
- **验证**：fmt EXIT 0 ｜ check 0 error、warnings 92/87 ｜ 全量 test **0 failed**（bin 1572P/40I）｜ numstat 1008/611（-w 987/590）。
- 🔴 运行时观察：CT2 `translator_destroy` 测试 teardown 挂死（生产 `process::exit` 规避）。
- **未改版本 / 未 push / 零凭证**；未改 `docs/MACOS-HANDOFF.md`（结论交主控合入）。

## 2026-09-23 — TRANS-394-REWORK（coder-2，✅ 阶段一返工交付）

- **退回项**：R1 `translator_destroy` 死锁（运行期/退出期都会调；原「生产走 `process::exit` 规避」说法经主控核实**不成立** —— `main.rs` 无 `process::exit`，退出走 `worker_join.join()`）；R2 误删方向判定用例；R3 `no`/`am`/`pm` 缩写误伤句末。
- **R1-a 取证**（真模型 `#[ignore]`，普通线程 drop + 进程 CPU 采样）：
  - A `load→translate→drop`：🔴 30s 不返回，CPU 16.48s→**22.11s 后停涨**（= 死锁非自旋），`timeout 75` 退出码 **124**（进程自身无法退出）；
  - B `load→不翻译→drop`：✅ 1.28~1.64s 返回，exit 0（复现 3 次）⇒ 死锁**只在推理过的 translator 上**发生（create→destroy 最小复现测不出来）。
- **R1-b 修法**：模型改**进程级、加载一次、永不析构** —— `thread_local! { static NLLB_MODEL }` + `shared_model()`（命中同路径复用，否则 `Box::leak`），`TranslationEngine` 持 `&'static NllbModel`；`Drop for Ct2Translator` 保留但生产路径永不触发。代价：模型常驻 ≈600MB 直到退出。方案选 `thread_local` 而非 `static Mutex<Arc>`（免 `unsafe impl Send/Sync`；生产单 worker 线程串行）。修复后 A 组 8s 返回、exit 0。
- **R1-c**：两次 `new` `std::ptr::eq` 相同、第二次 **0.005ms**、干净 drop（无 `mem::forget`）；源码护栏：生产区 `NllbModel::new(` 恰 1 次 + `Box::leak` + `thread_local!`。
- **R2**：从 `HEAD` 原样恢复 `derive_target_japanese_kanji_returns_english_known_boundary`。**R3**：`am`/`pm` 删除、`no` 加「下一非空白字符是数字」后置条件；补 2 条单测。
- **验证**：`cargo fmt --check` EXIT 0 ｜ `check --all-targets` 0 error、warnings **92/87** ≤ 97/88 ｜ translation **26P/0F** ｜ `--ignored …translation::tests::trans394` **4P/0F/95.79s, exit 0** ｜ 全量 bin **1585P/1F**（唯一失败为 TEST-SYNC-393 期望值错，`c0baf8c` 已修）。
- **未改 `main.rs` / transcription / vad / local_stream**（+`troubleshooting.md` 一条）；未改版本 / 未 commit / 未 push / 零凭证。
