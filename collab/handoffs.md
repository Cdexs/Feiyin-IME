# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-23 归档：2026-09-22 共 31 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

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
