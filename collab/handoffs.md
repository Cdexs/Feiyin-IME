# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-23 归档：2026-09-22 共 31 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

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
