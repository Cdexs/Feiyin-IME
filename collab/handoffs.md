# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-25 归档：2026-09-24 共 22 条已移入 `handoffs-archive.md`（本文件曾达 303 行，超 200 行上限）。
> 2026-09-24 归档：2026-09-23 共 35 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-23 归档：2026-09-22 共 31 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

## 2026-09-25 — coder-1 — FILLER-LONG-REPEAT-419 ✅ 交付（后处理节点规则 C，只改 text_normalizer.rs）

- **范围**：只改 `src/text_normalizer.rs` 的 `strip_fillers_conservative`（A/B 后加规则 C）+ `rule_c_*` 4 个纯函数 + 测试 10 条。未碰 `main.rs`（唯一调用方 `apply_filler_strip` 签名不变）。
- **规则 C**：紧挨着、一字不差重复的**整段**只留第一份。忽略标点/空白比较；长度门 中/日/韩 ≥6 字、英文等空白分词 ≥3 词；排除周期性单元与纯数字/数字+符号；差一字不动；三连一次折到位；与 B 迭代到不动点。例：`我们明天去公园，我们明天去公园。`→`我们明天去公园。`；`I think we should I think we should go`→`I think we should go`。
- **测试**：`rule_c_*` 10 条（中/英/日/韩正例、三连、短单元排除、数字排除、差一字不折、ABAB 不折、幂等）。fmt EXIT 0；check 0 error、warnings 91/87≤基线；全量 test 0 failed（bin 1775P/0F/54I，含 R1）。平台中立，MACOS-HANDOFF 已记。
- 红线：未 commit / 未 build release / 版本 0.9.3 未动 / 零凭证。

## 2026-09-25 — coder-1 — FIX-ALIGN-GATE-416 · R1 ✅ 交付（宽松层排序改绝对编辑距离，只改 mod.rs）

- **起因**：coder-2 TEST-SYNC-416 发现中段插入 1 字丢 1 字（宽松层按最低编辑率，分母偏置偏大 k）。
- **改法**：宽松层按**编辑距离绝对值最小**选、**打平取较小 k**（升序遍历、仅严格更小时替换）；编辑率仅作入选门槛 ≤0.35。`AlignResult` 加 `edit_dist`。
- **去 ignore**：`ts416g_finding_insertion_mid_loses_one_char` 去掉 `#[ignore]`（断言未动）⇒ 转正；`testsync416_tests` 10P/0I。
- **严格层 ③ 自查**：取第一个过门者、先小后大 ⇒ 无同类偏置。
- **验证**：fmt EXIT 0；check 0 error、warnings 91/87≤基线；repro416 9/9、testsync416 10/10；全量 test 0 failed（bin 1775P/0F/54I）。未 commit / 未 build release / 版本未动 / 零凭证。

## 2026-09-25 — coder-2 — TEST-SYNC-416 ✅ 交付（阶段三·非作者护栏 10 条；生产零改动）

- **范围**：只在 `src/transcription/mod.rs` 新增 `#[cfg(test)] mod testsync416_tests`（`+275/0`，10 条）；**未碰生产区**、未碰 `main.rs`/`local_stream.rs`/`audio/mod.rs`。
- **契约 1~8（独立推导）**：① 长句不重复 ② 各层不丢字（错字 1~4×start/mid/end + 删除 + 末尾插入）③ 同编辑率取小 k（嵌套精确 k=8/16 ⇒ 层④升序取 8）④ e<8 不放宽但精确接缝去重 ⑤ 接缝去重不误伤（口语短重复/仅 3 字相同）⑥ 全不同 ⇒ concat ⑦ 旧整片形状逐字 == S1+S2 ⑧ 源码锚点（`resolve_overlap` 调用 + `[DBG-416] seam` 在 Debug 门内）+ 阈值常量。自建字表（0x6C00/0x7000）与场景，不复用作者夹具/快照；期望值经 sandbox 复刻 416 纯逻辑逐条复算一致。
- 🔴 **FINDING（主控裁决「必须修，派 coder-1」）**：重叠区**中段插入 1 个额外字**时宽松层取 `k=13`（编辑率 2/13 < k=12 的 2/12）⇒ `committed=prev[..47]`，丢 S1 第 48 字（1 字）。已作 `#[ignore]` xfail 用例 `ts416g_finding_insertion_mid_loses_one_char`，注释写明**「416-R1 修复后去掉 ignore（由 coder-1 在 R1 执行）」**；R1 规则 = 宽松层按「编辑距离绝对值最小、打平取较小 k」选。
- **验证（白名单）**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` 0 error、warnings bin **91** / test **87** ≤ 基线、新增模块零 warning；🔴 **未跑 `cargo test`**（阶段三禁止；tester-1 执行）。除上述 FINDING 外未发现生产缺陷。
- 红线：未改生产代码 / 未 commit / 未 push / 版本未动 / 零凭证。

## 2026-09-25 — coder-1 — REPRO-413-ALIGN-GATE-416 + FIX-ALIGN-GATE-416 ✅ 交付（复现 + 修复，只改 mod.rs）

- **范围**：只改 `src/transcription/mod.rs` —— `align_overlap_with_prior` / `OrderedReflow::push_inner`（+ `AlignLayer`/`AlignResult` 字段、`align_try_k_ratio`、`seam_dedupe`、`resolve_overlap`）+ `repro416_tests`（9 条）。**未碰** `main.rs` / `local_stream.rs`。
- **复现**：413 形状两窗 ⇒ 原长度门 `max(8,0.30×新窗)` 挡真实重叠 ⇒ align_fail ⇒ 拼接 ⇒ 后缀重复；后缀 8/12/16 字在 S2 ≥19/29/41 字起重复。
- **修复分层**：① 有先验且 `e≥8` 时长度门只要求 `≥8`（`e<8` 不进放宽路径，防切多丢字）；② 宽松对齐 `[0.5e,1.5e]` 取编辑率最低、≤0.35 采用；③ 接缝去重（≥4 有效字精确片段只留一份）；④ 比例门兜底/无先验不变，宁重复不丢字。Debug `[DBG-416] seam`。
- **验证**：快照翻转（0/1/2 错字全 `=`，0 错字精确）；真实 debug.log 15 窗回放 修前 dup=2/丢=0 ⇒ 修后 dup=0/丢=0；反例 3 条。`fmt --check` EXIT 0；`check --all-targets` 0 error、warnings 91/87≤基线；全量 `cargo test` **0 failed**（bin 1755P/0F/54I）。平台中立，MACOS-HANDOFF 已记。
- 红线：未 commit / 未 build release / 版本 0.9.3 未动 / 零凭证。

## 2026-09-25 — coder-2 — LOCALRT-STREAM-THREADS-417 ✅ 阶段一交付（待主控验收，只改 local_stream.rs）

- **改动**：`local_stream_num_threads()` 由 `available_parallelism().min(8)` 改 **`.min(4)`**（取不到回落 4）。doc 注释写明 Gavin 2026-09-25 指示与理由（F-F-01：流式/精解各 min(核,8) 抢 CPU，预览小模型让核）。`create_local_stream_recognizer` 处注释同步。**只改预览侧**，精解 `default_acc_num_threads()`（mod.rs，min 8）不动。
- **测试**：`stream_num_threads_follows_machine_cores_and_caps_at_8` 更名 `..._caps_at_4`，封顶断言 8→4，新增「≥8 核时与精解侧刻意不同口径（4 vs 8）」断言、删「须完全一致」断言。
- **验证**：fmt EXIT 0；check 0 error、warnings 92/87=基线；全量 test 排除 coder-1 在途 RED `repro416_tests` 后 0 failed。平台中立（MACOS-HANDOFF 已记）。小改动短路径，不单派阶段三。
- 红线：未 commit / 未 build release / 版本未动 / 零凭证。

## 2026-09-25 — coder-2 — DEBUG-SESSION-WAV-418 ✅ 阶段一交付（待主控验收，只改 audio/mod.rs）

- **改动**：新增 `SessionDump`（`-debug` 时累积**送入本地实时管线的 16k 单声道音频**，录音结束 `Drop` 后台线程写 `debug-audio/session-<ts>.wav` + `[DBG-418]` 日志）；`record_streaming` 内 `emit` 闭包包裹全部 `on_chunk` 出口（pre-roll/post-hotkey/主循环/松手 drain/尾部 flush）；`enforce_dump_limit_where` + `is_session_dump_file` ⇒ session 上限 **10** 与 preroll **20 分开计数**。
- **硬约束**：① Warn 级 `new` 返回 None（零动作）；② 录音线程只 `extend_from_slice`、写盘后台线程、失败只 `warn`；③ 只借 `&[f32]`，送识别音频逐 bit 不变。
- **测试**：`diag418_quiet_at_warn_and_writes_full_session_at_debug`（Warn 零动作 / Debug WAV 头 rate=16000·单声道·4800×2 / sherpa 回读 4800 样本 / 输入不变）、`diag418_session_limit_10_separate_from_preroll`（12→10 删最旧 / preroll+notes 不动 / 反向闸门不动 session）**2P/0F**。
- **覆盖**：`record_streaming`（本地实时+在线流式）生效；批处理 `record()`（本地精确档）不在本链，未覆盖（result 说明）。
- **验证**：fmt EXIT 0；check 0 error、warnings 92/87=基线；全量 test 排除 coder-1 在途 RED `repro416_tests` 后 0 failed。平台中立（MACOS-HANDOFF 已记）。
- 红线：未 commit / 未 build release / 版本未动 / 零凭证。

## 2026-09-25 — coder-2 — OVERLAY-MEASURE-CACHE-415 ✅ 阶段一交付（待主控验收，只改 main.rs 浮层区）

- **改动**（`src/main.rs` 浮层区）：新增 `MeasureKey{text,font_size,dpi}`；GDI `LastMeasure<i32>` + `get_or_insert_with`（命中即返回、**未命中才 `encode_wide`+`GetTextExtentPoint32W`**）；DWrite `DwriteMeasure`（**额外缓存 `encode_utf16` 结果** ⇒ 命中不建 `CreateTextLayout`、不重编码，`DrawText` 复用缓存编码）；两套 `thread_local`（无锁，仅绘制线程）。`measure_text_width(hdc, text, font_size)` 加参（`draw_recording_overlay_with_text` 传 `OVERLAY_TEXT_FONT_SIZE`、`adjust_overlay_pos_size_for_text` 传 `font_size`）；`d2d::streaming_text` 先查缓存、未命中才 encode+量宽（绘制块移入缓存闭包）。
- **约束达成**：① 存原值 ⇒ 命中与不缓存逐位一致；② 键含文本/字号/DPI，GDI 与 DWrite 分开缓存（281：同串差 143~155px 不互串）；③ 单条「上一次」、无锁 thread_local；④ 命中跳过编码；⑤ `log_draw_geo_277` 调用点/500ms 节流/参数不变；⑥ 平台中立性见 `docs/MACOS-HANDOFF.md`（**已评估，对 macOS 无影响**）。
- **测试**：`omc415_hit_skips_measure_and_returns_same` / `omc415_key_covers_text_font_dpi` / `omc415_gdi_and_dwrite_are_independent` / `omc415_dwrite_put_hit_and_invalidate` **4P/0F**（纯逻辑 `LastMeasure`/`DwriteMeasure` 直测，无需 HDC）。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` 0 error、warnings **92/87** = 基线；全量 `cargo test --no-fail-fast` **0 failed**（bin 1744P/54I）；`git diff --name-only` 仅 `src/main.rs`。
- 🔴 **端测须 Gavin 目视**（浮层 Win32+D2D 原生绘制，自动化无覆盖）：文字滚动贴右、窗口随文字变宽、改字号后显示、多屏/DPI 切换。
- 红线：未 commit / 未 build release / 版本未动 / 零凭证。

## 2026-09-25 — coder-2 — TEST-SYNC-413 ✅ 交付（阶段三·非作者护栏 10 条；生产零改动）

- **范围**：只在 `src/main.rs` 新增 `#[cfg(test)] mod testsync413_tests`（10 条）；**未碰生产区**、未碰 `transcription/mod.rs`。
- **契约 1~7 逐条**：① 常态单片 `must_start=ge-1` + 前文只取 `ge-2` 后缀；首片/`must_start==gs` 无前文 ② 规则 3 集成（`plan_windows` 4×4s）：无 pending ⇒ 窗 `(2,4)`、pending=1 ⇒ 窗 `(1,4)` 且首窗 `must_start=pending`；非首窗/窗外 pending 忽略 ③ 规则 4 收尾窗 `(1,3)` + 传 `gs` ⇒ `short_context_span=None`（整片）④ 调用点源码锚点（常规窗第 3 参 `must_start`、收尾窗 `gs`）⑤ `trim_and_shift_ranges`（跨 cut 截断 / `b==cut` 丢弃 / `a>=cut` 平移 / `cut=0` 恒等）+ `shift_and_concat_ranges` 组合（窗内坐标、样本表首元素=后缀、None 传播）⑥ 字速 8/2/0（及 NaN/负）⇒ 回溯 2/6/4s；前片短于回溯取整片；多字节按 char 比例基准 ⑦ `context_tail_pending` 只覆 must 片 + 无前文 None 源码锚点 ⑧ BUILD-399「开心开」：末片 2.85s、前片 2.98s ⇒ 5.83s，上界 6.85s < 旧 9.33s。
- **独立**：自建数值/场景、不复用作者夹具；期望值经 sandbox 复刻生产纯函数全部一致。
- **验证（白名单）**：`cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **92/87** = 基线、新增模块零 warning；🔴 未跑 `cargo test`（阶段三禁止；tester-1 执行）。未发现生产缺陷。
- 红线：未改生产代码 / 未 commit / 未 push / 版本未动 / 零凭证。

## 2026-09-25 — coder-1 — LOCALRT-SHORT-CONTEXT-413 ✅ 阶段一交付（待主控验收，只改 main.rs）

- **范围**：只动 `src/main.rs` —— 组窗/派发区（`dispatch_window!` + 两处调用点 + 407 块 ~8689-9141）、纯函数区（`must_start_for_window` / `short_context_span` / `trim_and_shift_ranges` / `take_context_suffix` ~12141）、测试区（`short_context_413_tests` 9 条）。**未碰** `transcription/mod.rs`（coder-2 的 414）。
- **核心**：常规窗音频由「`group_window_start_secs` 带上的整段前文片 + 新片」改为「紧邻前一片后缀（`tail_backtrack_secs` 回溯 ≥2s/≥12 字 + `find_tail_cut` 字缝）+ `[must_start..ge)` 完整解」。`must_start`：常态=窗末片；规则 3 并入 pending 的首窗=pending。对齐 span 收窄 `(must_start-1, ge)`（`window_samples.len()==ge-span_start`，与 407 同口径）。VAD 区间 `trim_and_shift_ranges` 裁剪平移（不新增每窗 VAD）。`new_slice_from` 改按窗内实际样本累计。
- **共函数**：407 与 413 共用 `take_context_suffix`（抽自 407，**非复制**）；407 输出逐位不变。
- **不变（主控确认）**：首片/无前文只解新片；`plan_windows` 规则 4 收尾短尾窗 `(p-1,p+1)` 仍**整片重解**（传 `must_start=gs`，源码护栏 `ts413_rule4_call_site_passes_gs_not_must_start` 锁）；407/410/411/412 行为不变。
- **BUILD-399 09:48:21Z 时长**：旧 9.33s ⇒ 新 ≈ `min(回溯, 紧邻前片)` + 2.85s；冷启动 ≤6.85s，紧邻前片 ~2.98s ⇒ **5.83s**（主控预期 5~6s）。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` 0 error、warnings 92/87 = 基线；全量 `cargo test --no-fail-fast` **0 failed**（`feiyin-ime` bin 1728P/0F/54I）。`docs/MACOS-HANDOFF.md` 已记一笔。
- ⚠️ 过程如实上报：先改宏签名后改调用点，中间态被 coder-2 回归撞见编译失败（已补齐）。红线：未 commit / 未 build release / 版本 0.9.3 未动 / 零凭证。
- **R1 返修（主控首轮退回，只改 main.rs）**：① 带前文常规窗的 406 兜底收窄为**只兜底 must 片**（新增 `context_tail_pending` → `TailPending{span:(must_start,ge),…}`；ctx Some push Some、无前文 push None 不变），防前片后缀（上一窗已精解准确文本）被流式顶掉（410 item2）；兜底 span 与上一窗零重叠 ⇒ 拼接不覆盖不漏字。② 删重复的「389（D3）…」注释一份。测试 11/11（+2 R1）；fmt EXIT0 / check 0 error、warnings 92/87=基线 / 全量 test 0 failed（bin 1730P/0F/54I）。仍未 commit / 未 build release / 版本未动 / 零凭证。

## 2026-09-25 — coder-2 — FIX-NOSPEECH-WINDOW-414 ✅ 阶段一交付（待主控验收，只改 mod.rs）

- **改动**（`src/transcription/mod.rs`）：`TimelineTrim::WholeWindow` 更名 `Revad`；`Revad` 与 `None` 回退分支共用新抽 `self_vad_ranges`（线程级 v6 VAD，懒建失败记住）+ 纯函数 `plan_self_vad_trim`（`SelfVadTrim`），**不复制**。时间线无语音 + 流式非空 ⇒ 自跑 VAD 复核：有语音剪静音（含 408B/412 声纹）解码；无语音 ⇒ `trimmed && n_ranges==0` 早退、不进模型；VAD 不可用 ⇒ 原样整窗。Debug `[LocalRT-DBG-414] revad: seg/in/ranges/src`。
- **影响面**：① `Apply`/`Empty` 逐位不变、`None` 输出逐位一致（388 早退/390 cap/406/408B/412 源码锚点护栏全过）；② 调用方仅本地实时滑窗路 B（`main.rs:9272/10466`）+ `audio/mod.rs` PoC + PoC bin，本地精确档/在线识别不调用（grep）；③ 复核路径照常走 408B/412 声纹过滤（与 `None` 同路径）；④ 平台中立无 `cfg`，`docs/MACOS-HANDOFF.md` 已记。
- **测试**：`fix414_*` 3 纯（有区间 ⇒ 剪静音 / 空区间 ⇒ 空样本 / VAD 不可用 ⇒ 原样整窗）+ 1 `#[ignore]` 真模型（`kv_long.wav` 补静音 ⇒ 有区间且剪后更短；纯静音 ⇒ 无区间空样本，**实跑 PASS**）；`ts393_plan_empty_with_streaming_needs_revad` + 源码护栏 `ts393c_revad_branch_uses_recheck_source_guard`（旧 393-A4 按新行为改写）。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` 0 error、`feiyin-ime` **92/87**=基线、mod.rs 0 warning；全量 `cargo test --no-fail-fast` **0 failed**（本单 +3P/+1I）。⚠️ 回归期间 coder-1 在途 `main.rs` 一度 `dispatch_window!` 宏少一参致整 crate 编译失败（已由 coder-1 修复，非本单）。
- 红线：未 commit / 未 build release / 版本未动 / 未碰 `main.rs` / 零凭证。

## 2026-09-25 — coder-1 — TEST-SYNC-411 ✅ 交付（阶段三·非作者护栏 5 条；生产零改动）

- **范围**：只在 `src/main.rs` `#[cfg(test)] mod testsync411_tests`（5 条，`19309` 起）；未碰 `speaker.rs`/`mod.rs`（coder-2 的 412 护栏）、未碰生产区。
- **契约 1~5**：性质（6 分布拼接==整段/边界/多字节）｜常规滑窗组合（真实 `plan_windows`：live `[(0,1),(1,2)]`+pending 2、tail `[(2,3)]`）｜切分+末尾窗 accept（410 基准两部分非空、真实精解 accept）｜切分+末尾窗被拒（只替 pending、前片不动）｜**端到端 BUILD-409 16:44Z 真实三窗无重复**（新 == 三窗精解各一次；旧复现整段重复）。
- **真实数据**：`debug.log:1212+`（整段 86 字流式按 80 截断 = `STREAM80`；ACC0/1/2 = 41/49/28 字；样本 10/10/3.85s）。期望值经沙箱复刻 406（win0 0.882 / 整段 0.412）与 `plan_windows` 验算。
- **白名单**：`rustfmt --config skip_children=true src/main.rs` EXIT 0；`cargo check --all-targets` 0 error、warnings 92/87=基线；**未跑 cargo test**。
- **观察（只报告不修）**：相邻窗不共享切片 ⇒ `OrderedReflow` 不去重 ⇒ 窗间同人短语重复仍在（组窗粒度、非 411；取向宁重复不丢字）。
- 红线：未碰生产代码 / 未 commit / 零凭证。

## 2026-09-25 — coder-1 — SPEAKER-MERGE-SHORT-412 ✅ 阶段一交付（待主控验收）

- **改动**（`src/transcription/speaker.rs`；`mod.rs` 仅 +1 源码锚点测试）：新增纯函数 `merge_speech_units` + `MERGE_GAP_SECS=0.5s`（**暂定，待端测校准**）+ `SpeechUnit`（语音时长不含间隔；退化零长区间保留为零长成员保一一对应）；`filter_ranges_by_voiceprint` 改**按单元**判定（单元 ≥2s 拼接全部成员语音算一次声纹、判定作用全成员；<2s `KeepShort`）；注册/漂移按单元 offer（**起点** ≥ `new_slice_from`，跨界单元保守不 offer）。
- **7 条影响面逐条结论+测试**：① `kept` 仍原各段（剪静音喂 `f.kept`，388 不变）② kept/dropped 口径同改前 ③ 解码后重解用原 ranges+整窗音频 ④ 漂移上限仍 0.25（8s/100s 单元同位移）⑤ 跨界单元不 offer ⑥ 无就绪档逐位一致 ⑦ 缺模型合并未执行。
- **测试**：`ts412_*` 9 + `mod.rs` 锚点 1 = **10P/1I**；`#[ignore]` 真模型单元 vs 2.4s 长段 **0.915**。同步更新 408B `ts408b_new_slice_from_contract_anchor` 锚点（**契约不变**）。
- **验证**：`fmt --check` EXIT 0；`check --all-targets` 0 error、warnings 92/87=基线；全量 `cargo test` **1701P/0F/53I**。证据 `collab/evidence/412/`。
- **R1 返修（数据校准，主控退回）**：`MERGE_GAP_SECS` **0.5→0.8s**（BUILD-409 同窗相邻间隔一半以上 0.5~0.8s，原阈值下窗#4 一段拼不上）；规则改 **只为凑够 2s 才拼**（单元 ≥2s 即封口）+ **尾段并入前单元** + **≥2s 单段独立**；安全性（拼错只令混合单元得分居中 ⇒ 不误删）入注释；补 4 条测试（窗#4 真实区间/凑 2s 封口/长段独立/尾段并入）⇒ `ts412` **14P/1I**；7 条影响面复核结论不变。
- 红线：未碰 `main.rs` / 未 commit / 未 build release / 版本未动 / 零凭证。

## 2026-09-25 — coder-1 — FIX-TAIL-GUARD-410 ✅ 阶段一交付（待主控验收，只改 main.rs）

- **① 比对基准只取前片后缀**：纯函数 `tail_streaming_baseline`（前片流式按样本占比取末尾相应**字符数**、char 切 + pending 流式）替代「整前片 + pending」⇒ 不再误拒正确精解。
- **② 末尾窗被拒/空只兜底 pending**：`TailPending` + `window_tail_pending`；harvest 用 span `(p,p+1)` + pending 文本 ⇒ 前片不动。406 逻辑未改。
- **③ 兜底先打标点**：进程级常驻标点服务线程（首次兜底才 spawn、引擎线程内加载、全进程复用）；500ms 超时、失败/超时原样 + warn；**Gavin 追加**：异常退出 ⇒ 重启重试最多 2 次、全失败 ⇒ 无标点 + 30s 冷却、超时不重启。
- **测试**：`f410_*` **8 条**（真实数据误拒→accept、前片不变+pending、char 切 emoji、关闭/已含标点、异常退出后重启成功、两次失败走无标点、超时不重启、冷却窗口）+ `#[ignore]` 真模型（`今天天气不错我们出去玩吧`→`今天天气不错，我们出去玩吧。`）。
- **验证**：`fmt --check` EXIT 0；`check --all-targets` 0 error、warnings 92/87=基线；全量 `cargo test` **1674P/0F/52I**。证据 `collab/evidence/410/`。
- 红线：只改 `src/main.rs` / 未 commit / 未 build release / 版本未动 / 零凭证。

## 2026-09-25 — tester-1 — TEST-EXEC-412（411+412 合包）✅ 回归通过 · 🔴 出包暂停

- **交付源码**：HEAD `94ab214`（`35a1e8e` 411+412+阶段三 + `94ab214` 返修），版本 0.9.3。单：`FIX-SPLIT-SLICE-STREAMING-411`（多片切分时流式文本按样本占比分配）+ `SPEAKER-MERGE-SHORT-412`（声纹判定前短段拼 ≥2s 单元）+ 阶段三 15 条。
- **回归**：首轮 bin **1715P/1F/53I**（1 条红 `ts412b_unit_scope_three_members` 系手算常量 2.8 写错、应为 0.8+0.9+0.9=2.6，阶段三测试自身错）→ 返修（期望改由区间数据推导）后复跑 bin **1716P/0F/53I**、总 **1804P/0F/55I**；`src-tauri` **92P/0F/0I**；`fmt --check` EXIT 0；Vitest SKIP。
- **NEW/GONE**（对 BUILD-410 bin 1680P/52I）：NEW **+36P**（`ts411_*` 7 + `ts411guard_*` 5 + `ts412_merge_confined_to_predecode_trim_anchor` + `ts412_*` 14 + `ts412b_*` 10）**+1I**（`ts412_real_merge_short_segments`）；**GONE 0**（`main.rs::slice_streaming_text` 为同名重构的生产 fn，非用例）。
- **构建**：Step1 残 0 → Step2 SKIP（ui/src-tauri 无 diff）→ Step3 主程序 **2m36s**（**CT2 编译计数 0**）。产物仅在 `target/release/`（main 01:54:56 / crash 01:53:17）。
- 🔴 **出包暂停（主控指令：Gavin「别总频繁重复出包」）**：**未执行 Step4 同步 Publish、未冒烟/未强杀输入法**；`Publish/` 仍为 BUILD-410（main `01:05:59`/crash `01:04:01`/ui `13:45:21`/ct2 `efa16d81…`），config `da2be5da…` 未变。待主控统一号令后补 Step4 + 冒烟 + 九项。
- **只读专项**：A CT2 不重编（sha `efa16d81…` 与上包逐位相同）；B 声纹模型 `aa3cfc16…` 仍在包内；D 用户数据只列不删（`voiceprint.bin` 不存在）；C 冒烟 N/A。三特殊点只读核验全 PASS。
- 未 push / 版本号未动 / 未改生产代码 / 未 `cargo clean` / 未动 Publish / 零凭证。证据 `collab/evidence/412/`。

## 2026-09-25 — tester-1 — TEST-EXEC + BUILD-410 ✅ 出包（末尾窗 × 406 配合修正；九项 + 专项 A~D + 三特殊点全 PASS）

- **交付源码**：HEAD `1814baf`（`e4cae44` FIX-TAIL-GUARD-410 + `1814baf` TEST-SYNC-410），工作区 clean，版本 0.9.3。单：末尾窗比对基准只取前片对应末尾 + pending、被拒只兜底 pending 前片不动、流式兜底经进程级标点服务线程打标点（懒启动/异常重启≤2/冷却 30s/超时不重启）+ 6 条非作者护栏。
- **回归**：root bin **1680P/0F/52I**、总 **1768P/0F/54I**（EXIT 0）；`src-tauri` **92P/0F/0I**；`cargo fmt --check` **EXIT 0**；Vitest **SKIP**（`ui/` 无 diff）。**NEW/GONE**（对 BUILD-409 bin 1666P/51I）：**+14P**（`f410_*` 8 + `ts410_*` 6）**+1I**（`f410_real_punctuate`）；**GONE 0**⇒ 1680P/52I。预期逐位命中。
- **构建**：Step1 残 0 → Step2 **SKIP**（`ui/`+`src-tauri/` 无 diff）→ Step3 #1 **2m57s**（**CT2 编译计数 0**）/#2 **0.94s** 全增量（0 Compiling）→ Step4 `scripts/init-publish.ps1`（Step5 三 exe 全 OK）+ 声纹模型 + 四张 rules toml。产物 main `a3365d7925e7…`(15,099,904B/01:05:59) / ui `ee7f2571f1d4…`(未变) / crash `b675b6f481a4…`(24,879,104B/01:04:01) / ct2 `efa16d8110bd…`(未重编)；两副本全等。
- **九项逐项 PASS**：①时间戳 01:03–01:06 ②两副本 sha 相等（main/crash 异于上包；ct2/ui 未变属预期）③**0.9.3** ④冒烟 PID **3364 Responding=True**/无新 crash.json/无 panic/`WM_CLOSE` **380ms**/残 0 ⑤config `da2be5da…` 不变 ⑥warnings 92/9（+`ctranslate2-sys` 18 基线）⑦正 `[LocalRT-DBG-410]`=4（反探针 N/A：本批无删除字面量）⑧scene/itn 三副本全等 ⑨**VC++ 五运行库 sha 与 Redist `14.44.35112` 全等**。
- **专项 A~D**：A **CT2 不重编**（#1=0、#2=0.94s，sha `efa16d81…` 与上包逐位相同）；B 声纹模型 `aa3cfc16963a…` 28,281,164B 仍在包内；C 冒烟（未录音：DBG-406/407/410 新窗=0，标点服务未 spawn 属正常，无 shadow finalize/speaker warn/panic）；D 用户数据**只列不删**（`voiceprint.bin` 不存在；config.toml 2026-09-20 / wordbook.sqlite 2026-09-10）。
- **三特殊点（仅核验）**：① sherpa 四 DLL 三副本全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本 `60b227de…`；③ `Publish/models/` 1.7B 七文件与源全等、0.6B 保留。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启 + 带 `-debug` 端测**。未 push / 版本号未动 / 未改生产代码 / 未 `cargo clean` / 未删用户数据 / 零凭证。证据 `collab/evidence/410/`。

## 2026-09-25 — tester-1 — TEST-EXEC + BUILD-409 ✅ 出包（405~408B 合包；九项 + 专项 A~E + 三特殊点全 PASS）

- **交付源码**：HEAD `d215a95`（`79240dd` + 返修 `05a6d7d`/`d215a95`），工作区 clean，版本 0.9.3。单：405（移除影子收尾 DEC-086 + 显示缓存 + CT2 不重编 + 脚本 exe 名）/406（精解与同窗流式比对守卫）/407（1900ms 末尾组窗）/408A+B（声纹 CAM++ 接入路B）+ 阶段三护栏。
- **回归**：首轮 bin **1664P/2F/51I**，2 条红均为阶段三测试自身 bug（`ts407_tail_only_when_pending_source_guard` 源码护栏窗口 2600 字符过小；`ts408b_partition_ranges_conservative` 误按 32kHz 算时长），非生产缺陷；主控核实后派作者返修。复跑 bin **1666P/0F/51I**，总 **1754P/0F/53I**（EXIT 0）；`src-tauri` **92P/0F/0I**；`cargo fmt --check` **EXIT 0**；Vitest **SKIP**（`ui/` 无 diff）。
- **NEW/GONE**：对 **408B `73537a2`**（bin 1641P/51I）**+25P/0G** ⇒ 1666P/51I；对**上包 BUILD-399 `49bae07`** NEW **80** 测试、GONE **2**（`endpoint_confirm_text_takes_longest_of_two`/`_len_not_below_main`）；`vad.rs::find_gap_cut` 为同名重构、不计 GONE。
- **构建**：Step1 残 0 → Step2 **SKIP**（`ui/`+`src-tauri/` 无 diff）→ Step3 #1 **6m06s**（CT2 重编一次：405 改了 build.rs，属预期）/#2 **2m18s**（voice-ime 返修后重编；**CT2 编译计数 0、sha 不变**）/#3 **0.90s** 全增量（0 Compiling）→ Step4 `scripts/init-publish.ps1`（Step5 三 exe 全 OK）+ 声纹模型 + 四张 rules toml。产物 main `b37aa194d76f…`(15,031,296B/00:23:06) / ui `ee7f2571f1d4…`(未改) / crash `a555f11c4141…`(24,879,104B/00:21:35) / ct2 `efa16d8110bd…`(27,405,312B)；两副本全等。
- **九项逐项 PASS**：①时间戳 00:12–00:23 ②两副本 sha 相等（main/crash/ct2 异于上包；ui 未改属预期）③**0.9.3** ④冒烟 PID **27040 Responding=True**/无新 crash.json/无 panic/`WM_CLOSE` **340ms**/残 0 ⑤config `da2be5da…` 不变 ⑥warnings 92/9（+`ctranslate2-sys` 18 基线）⑦正 `[LocalRT-DBG-408]`=3、反 `shadow finalize`=0 ⑧scene/itn/homophone/wordbook 四表三副本全等 ⑨**VC++ 五运行库 sha 与 Redist `14.44.35112` 全等**。
- **专项 A~E**：A **CT2 第二次起不重编**（#2=0、#3=0.90s，sha `efa16d81…` 稳定）；B 声纹模型进包 `aa3cfc16963a…` 28,281,164B（与源等）；C `init-publish.ps1` Step5 自动复制 `feiyin-ime.exe`/`feiyin-ime-ui.exe`/`crash-reporter.exe`（**无 SKIP、无手工 cp**）；D 冒烟：未录音（N/A 三证 + exe `[LocalRT-DBG-408]`=3），新窗 `shadow finalize`=0、无 speaker warn；E 用户数据**只列不删**（`Publish/voiceprint.bin` 不存在；config.toml 2026-09-20 / wordbook.sqlite 2026-09-10）。
- **三特殊点（仅核验）**：① sherpa 四 DLL 三副本全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本 `60b227de…`；③ `Publish/models/` 1.7B 七文件与源全等、0.6B 保留。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启 + 带 `-debug` 端测**。未 push / 版本号未动 / 未改生产代码 / 未 `cargo clean` / 未删 Publish 用户数据 / 零凭证。证据 `collab/evidence/409/`。

## 2026-09-25 — coder-2 — FIX-SPLIT-SLICE-STREAMING-411 ✅ 交付（多片切分流式按占比分配）

- **缺陷**（BUILD-409 16:44Z）：23.85s 切 3 片 ⇒ `slice_streaming_text` 旧「k==0 整段 / 其余空」使窗#0 基准=整段 86 字（406 retention 0.38 误拒）⇒ 整段兜底回灌 ⇒ 最终文本重复；后片窗基准缺字；末尾窗 pending 兜底为空（410 潜在丢字）。
- **改动（仅 `src/main.rs`）**：`slice_streaming_text(k, seg, &slice_samples)` 按样本占比分配（`round(total_chars×cum/total)` 单一定义 ⇒ 无缝、求和==总字数；char 切）；调用传 `&new_lens`；Debug 埋点 `[LocalRT-DBG-411]`。
- **影响面**：常规窗 concat 基准逐片正确；末尾窗 `tail_streaming_baseline` 前后片非空；🔴 被拒 pending 兜底非空（旧为空⇒丢字）；344-G `hole_fill_decision` 生产无调用点 + `partial_win_committed` 不读流式 ⇒ 不受影响。
- **测试**：新增 `fix411_tests` 7 条（含真实数据、切分+末尾窗被拒兜底非空、源码护栏）；更新 386 用例计数（17⇒9+8）。**验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` 0 error、warnings 92/87=基线；全量 `cargo test --no-fail-fast` **0 failed**（bin **1697P/52I**）。
- **未验证**：实机端测（多片切分不再误拒/重复、`[LocalRT-DBG-411]` 埋点）交 tester-1/Gavin。
- **红线**：未改 406/407/410/声纹；未加 config/env；未 commit / 未 push / 未 build release / 零凭证。

## 2026-09-25 — coder-2 — TEST-SYNC-412 ✅ 交付（阶段三 · 短段拼接非作者护栏 6 条）

- **被测**：`SPEAKER-MERGE-SHORT-412`（相邻短段按 <0.8s 间隔拼「连续说话单元」再判；作者 coder-1，HEAD `25d097b`）。
- **范围**：`src/transcription/speaker.rs` **仅**追加 `mod testsync412_guard_tests`（+173/0，6 条）；生产代码零改动；未碰 main.rs/mod.rs。
- **10 条**（首轮 6 + 主控裁量 A 后补 4）：阈值精确边界（恰 0.8s 不拼）；随机性质 500 组（成员一一对应 / `speech_samples`==成员和 / 单元内间隔 <0.8s / 退化零长段）；BUILD-409 间隔集不 panic；三成员单元作用域（Drop 全剔 / Keep 原段）；无就绪 KeepNotReady + 缺模型生产早退源码护栏；空成员合成单元边界；**契约2 前向封口**；**③ 收尾例外**（`[2.1s,0.3gap,0.5s]`⇒1；`[2.1s,0.3gap,0.5s,0.3gap,2.5s]`⇒3）；**契约6 注册门**（起点边界 + 跨界不 offer + 源码护栏）；**契约3 窗#4 精确**（恰 2 单元均 ≥2s）。
- **契约2 措辞（主控裁量 A）**：③ 收尾「<2s 尾段并入前一单元」为 412 R1 有意例外（非缺陷）；契约2 限定「前向累积阶段」，据此分别钉住。
- **验证（白名单）**：`rustfmt --config skip_children=true src/transcription/speaker.rs` CLEAN；`cargo check --all-targets` **EXIT 0**、0 error、warnings **92/87** = 基线；sandbox 复刻 `merge_speech_units` 500 组性质 bad=0 + 边界 + BUILD-409 ✅。🔴 **未跑 `cargo test`**（阶段三禁止，首跑由 tester-1）。
- **疑似生产缺陷：未发现**；**未改生产代码 / 未 commit / 未 push / 零凭证**。
- **返修（tester-1 回归红，测试自身错误）**：`ts412b_unit_scope_three_members` 三段 0.8+0.9+0.9=2.6s，两处期望手算成 2.8 ⇒ 红；改为由数据导出 `total=Σ(e-s)/SR` 再比。sandbox 未覆盖之因：复刻只移植 `merge_speech_units`、未移植 `partition_ranges` 秒数累计，且阶段三禁 `cargo test`。复跑：`rustfmt --skip_children` CLEAN；`cargo check --all-targets` 0 error、warnings 92/87=基线。

## 2026-09-25 — tester-1 — TEST-EXEC + BUILD-415（411~415 合包）✅ 回归+构建（🔴 Publish 先行同步，主控裁定保持）

- **性质**：阶段四回归 + 阶段五构建（411+412+413+414+415）。源码 HEAD `8c6f1a9`，版本 0.9.3（未动）。
- **回归**：root bin **1744P/0F/54I**（基线上轮 1716P/53I）⇒ NEW **31** / GONE **2**（GONE 均为 414 契约有意重命名：`ts393_plan_empty_with_streaming_decodes_whole_window`→`_needs_revad`、`ts393c_whole_window_branch_returns_whole_samples_source_guard`→`ts393c_revad_branch_uses_recheck_source_guard`）；`src-tauri` **92P/0F/0I**；`cargo test --bin feiyin-ime f414 -- --ignored` **1P/0F**；Vitest/Browser SKIP（ui 无 diff）。
- **消融（过滤跑，全还原）**：413 `short_context_span` 恒 None ⇒ 5 红（`short_context_413_tests` 3 + `testsync413_tests` 2）；414 `Revad` 臂改整窗 ⇒ `ts393c_revad_branch_uses_recheck_source_guard` 1 红，`plan_self_vad_trim` 改整窗 ⇒ `f414_self_vad_empty_yields_empty`/`f414_self_vad_nonempty_trims` 2 红。`git diff -- src/` 为空、`git status` 仅 docs。
- **构建**：Step1 清进程 0 残留；Step2 npm 1.43s（JS 资产 `index-DGCpO3OF.js` 与上包同名）+ Tauri 1m52s(17w) + cp；Step3 3m08s（main 92w / crash 9w）；Step4 三 exe + 两 toml 同步 Publish。
- **九项核验**：全项 PASS，唯 ④冒烟「部分」——`-debug` 起、`Responding=True`、无 crash.json，浮层未验（target/release config 热键 vk=165 与脚本 F9 不符）；①时间戳 12:22–12:26 ②两副本 sha 相等且异于 BUILD-410 ③0.9.3 ⑤config `da2be5da…` 不变 ⑥92/9/17 ⑦正探针 `[LocalRT-DBG-414]`=1/`-413`=2/`-411`=1、反探针不可构造（本批无删除字面量）⑧`scene-rules`/`itn-rules` 三副本全等 ⑨VC 五件 sha 与 Redist 源三处全等。
- **专项**：A CT2 `efa16d81…` 前后未变（未重编）；B 声纹模型 `aa3cfc16…` 源/junction/Publish 三处全等、`target/release/models` 仍 Junction；D 用户数据（config/wordbook/debug.log/version_check.json）未动、无 voiceprint.bin。
- 🔴 **范围变更**：主控「先别打包」指令到达前已完成 Step4 同步 + 一次冒烟；主控裁定 **A：Publish 保持 BUILD-415 不回滚**，本单结束，等下一包号令。
- **红线**：未改生产代码（消融全还原）/ 版本号未动 / 未 push / 未 `cargo clean` / 零凭证。证据 `collab/evidence/415/`。
