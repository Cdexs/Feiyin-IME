# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

## 2026-09-22 — tester-1 — BUILD-373 ✅ 出包（阶段五 · 替换作废的 BUILD-349；八项 + 三特殊点全 PASS）

- **交付源码**：HEAD `dff4fad`，版本 0.9.3，工作区 clean。含 `7398459` FIX-PREFIX-AND-EAT-371（P0×2）+ `77185aa` TEST-SYNC-371 + `dff4fad` TEST-EXEC-371。阶段四已全绿（root 1511P/0F/30I / src-tauri 92P / Vitest 100P/11S / fmt EXIT 0 / warnings 98/88/17），**本单不重跑回归**。
- **BUILD-373**：Step1 强杀进程残 0 → Step2 npm 718ms + Tauri 1m47s（17w）→ Step3 主程序 3m03s（**98w** + crash 9w）→ Step4 UI 同步 + 三 exe→Publish（dll/itn/models 仅核验）。产物 main `3453c6006186…`（14,744,064B/23:28:17）/ ui `dd7b6e4866d7…`（10,050,048B/23:25:12）/ crash `29a36303bd4f…`（24,879,104B/23:26:19）；两副本全等、三者均异于作废的 BUILD-349。
- **八项逐项 PASS**：①时间戳 23:25–23:28 ②sha 两副本 + 异于上包 ③**0.9.3** ④冒烟 PID **26744 Responding=True** / 两处无新 crash.json / panic 0 / 残 0 ⑤config `da2be5da…` 三时点不变 + wordbook 不变 ⑥warnings **98/88/17** = 基线 ⑦探针 ⑧四表三副本全等。
- **探针**：源码级正 `QWEN3_PREFIX_MAX_BYTES`/`align_overlap_with_prior`/`push_window`/`AlignPrior`/`ALIGN_EXPECTED_K_TOL_DOWN`/`UP` 均 ≥1；二进制级正 `SLIDING-WINDOW-367`=**4**/`[LocalRT-DBG-298]`=**3**；🔴 反 **`is_qwen3_language_label`=0**（371 旧闸已清）/`PUNCT_REFRESH_INTERVAL`=0/`ACC_MIN_SEGMENT_MS_DEFAULT`=0/`min_seg_ms`=0/`sub-seg failed`=0。
- **三特殊点（仅核验）**：① dll 四张三副本全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本 `60b227de…` 全等；③ `Publish/models/` 1.7B 七文件与源逐一 sha256 全等、**0.6B 保留**。
- 🔴 **Step1 强杀了 Gavin 正在使用的输入法进程** ⇒ 已提醒**重启后端测**。
- **红线**：未 push（需 Gavin 指示）/ 版本号未动 / 未改代码 / 未 `cargo clean` / 未动 models 源目录 / 零凭证。

## 2026-09-22 — tester-1 — TEST-EXEC-371 ✅ 阶段四全量回归（不出包；三套全绿，NEW 33/GONE 0）

- **交付源码**：HEAD `77185aa`（`7398459` FIX-PREFIX-AND-EAT-371 + `77185aa` TEST-SYNC-371），工作区 clean。🔴 **未出包**（DEC-079，等 Gavin）、未 build release、未动 `Publish/`。
- **三套测试**：root `cargo test --no-fail-fast`（全量未过滤）**1511P/0F/30I**（EXIT 0；`feiyin-ime` bin **1423P/28I**）；`src-tauri` **92P/0F/0I**；Vitest **7 files/100P/11S/0F**；`cargo fmt --check`（不带 `skip_children`）**EXIT 0**；warnings **98/88/17** = 基线未升。
- **NEW/GONE 对账（基线 1390P/28I ⇒ 净 +33P）**：NEW **33** = `7398459` 实测 **10** 条（8 条 `fix371_repeat_align_tests` + **2 条前缀模块新增** `near_head_non_label_is_stripped`/`mid_body_marker_is_not_stripped`）+ `77185aa` 阶段三 **23** 条；**GONE 0**（`-#[test]`=0；`degenerate_does_not_harm_body` 仅改断言、名未变；删的 `is_qwen3_language_label` 是辅助函数）。
- 🔴 **与任务书预期差异（如实报）**：任务书按 371=8 条算预期 31（1421）；**实测 371=10**（漏计前缀模块 2 条）⇒ `1390+33=1423` 才吻合。
- **重点失效模式（吃字/重复字）核查**：`align` / `ordered_reflow` / `periodic` / `strip_qwen3` / `prefix` 相关用例**全部通过、0 失败**，无停手条件；无失败用例需贴 panic。
- **红线**：未改代码 / 未改版本号 0.9.3 / 未 push / 零凭证。

## 2026-09-22 — tester-1 — TEST-SYNC-371 ✅ 交付（阶段三 · 非作者视角补 23 条护栏；生产代码零改动）

- **性质**：阶段三 TEST-SYNC，**按设计契约写用例**（不读实现反推），只改 `#[cfg(test)]` 区。目标 = `FIX-PREFIX-AND-EAT-371`（A 语种前缀剥离 / B 滑窗对齐 / C 接线）。
- **新增 23 条**（`src/transcription/mod.rs` +381、`src/main.rs` +71）：
  - **A 前缀剥离 11 条**（`testsync371_prefix_contract_tests`）：只取首个 `<asr_text>` / 尾随空白半角全角冒号 / 截完即空 / `QWEN3_PREFIX_MAX_BYTES=64` / 起点 63·64·65 / 跨边界多字节不 panic / 任意前缀形态（换行·`<`·数字标点）/ 语言无关（日韩英俄）/ 空串半空 / 正文不误伤 / 与 `<|…|>` token 叠加顺序。
  - **B 滑窗对齐 10 条**（`testsync371_align_contract_tests`）：近周期差 1/2 字卡质量门 0.15 / 周期 4·5 遍不丢 / 期望比例 0·负·NaN 退化 / 极短窗保守 / `prev_extra_slices=0` 不设硬上界且无重叠不强行提交 / `slice_samples` 不自洽安全退化 / 乱序+中间空窗 / 混合序列全程不丢 / 极长窗不 panic。
  - **C 接线 2 条**（`testsync371_window_counter_guard_tests`）：`include_str!` 扫生产区 —— 两计数器必须 per-recording `let mut Vec::new()`、不得 static；两表同批 push。
- **验证（白名单）**：`cargo fmt` clean / `--check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **98/88** = 基线未升；`numstat`==`-w`（71/0、381/0）。🔴 **未跑 `cargo test`**（护栏首跑在阶段四）。
- **发现**：**未发现真实缺陷**（全部按设计契约静态推演一致）。
- **红线**：未改生产代码 / 未 commit / 未 push / 版本号 0.9.3 未动 / 零凭证。

## 2026-09-22 — tester-1 — TEST-EXEC + BUILD-349 ✅ 重出包（修滑窗丢字 + 13s 安全阀 + 300s 录音；八项 + 三特殊点全 PASS）｜🔴 BUILD-348 作废

- **重出原因**：BUILD-348 含 **P0 丢字**（12s 封顶把窗口切到零重叠 ⇒ 对齐必败 ⇒ 「整窗跳过」⇒ 该窗内容全丢）。由 **369（`24c452a`）+ 370（`919a59e`）** 修复。
- **差异**：369 零重叠⇒**直接拼接**、有重叠对齐失败⇒退回拼接（**无一分支丢整窗**）；370 去 20s 硬切、单片安全阀 **13s**（超出继续切不丢弃）、`MAX_RECORD_SECONDS` 180→**300**。涉及 `transcription/mod.rs`、`local_stream.rs`、`vad.rs`、`main.rs`、`config/mod.rs`、`src-tauri/src/config.rs`。**未动** dll/itn/模型/版本。
- **回归（@ `919a59e`）**：root `cargo test --no-fail-fast` **1478P/0F/30I**（EXIT 0；`feiyin-ime` bin **1390P/28I** = 任务书基线逐位吻合）；`src-tauri` **92P/0F/0I**；Vitest **7 files/100P/11S/0F**；`cargo fmt --check` **EXIT 0**；warnings **98/88/17** = 基线 **98/88**。
- **NEW/GONE**：NEW **11**（369 滑窗 4 + 370 切分 7；`1390−1379=+11` 逐位吻合）；**GONE 0** —— 3 处改名（`max_record_seconds_is_180`→`_is_300`、`no_degrade_within_180s_limit`→`budget_180s_ok_but_300s_degrades`、368 的 `ordered_reflow_consecutive_failures_fallback_no_loss`→`..._falls_back_to_concat`），`+#[test]`/`-#[test]` = 11/0。
- **BUILD-349**：Step1 残 0 → Step2 npm 684ms + Tauri 1m42s（17w）→ Step3 主程序 2m52s（**98w** + crash 9w）→ Step4 UI 同步 + 三 exe→Publish。产物 main `a645825ffec7…`（14,740,992B/21:36:09）/ ui `b78dc71d48f5…`（10,050,048B/21:33:16）/ crash `f7c199e4547a…`（24,879,104B/21:34:21）；两副本全等、均异于作废的 BUILD-348。
- **八项逐项 PASS**：①时间戳 21:33–21:36 ②sha 两副本 + 异于上包 ③**0.9.3** ④冒烟 PID **12380 Responding=True** / 两处无新 crash.json / panic 0 / 残 0 ⑤config `da2be5da…` 三时点不变 + wordbook 不变 ⑥warnings **98/88/17** = 基线 ⑦探针（正 `SLIDING-WINDOW-367`=4/`[LocalRT-DBG-298]`=3/`AUTOLEARN`=11；反 `sub-seg failed`=0/`PUNCT_REFRESH_INTERVAL`=0/`ACC_MIN_SEGMENT_MS_DEFAULT`=0/`min_seg_ms`=0）⑧四表三副本全等。
- **三特殊点（仅核验）**：① dll 四张三副本全等 + onnxruntime **1.28.2**；② itn-rules 三副本 `60b227de…` 全等；③ `Publish/models/` 1.7B 七文件与源逐一 sha256 全等（0.6B 保留）。
- **Gavin 端测十一条**：① 🔴 吃字复测（>4 句，含一句 8~10s 长句造零重叠）② 🔴 连录两次互不污染（Gavin 点名）③ 松手等待时间 ④ 前文被改对 ⑤ 接缝重复 ⑥ 标点语义 ⑦ 1.7B 质量 ⑧ 梅开二度/一度 ⑨ 长录音 2~3 分钟（验 300s）⑩ 不新增 crash.json ⑪ 重启输入法。
- **红线**：版本未动 / 未改生产代码 / 未 push / 未 `cargo clean` / 未动 models 源目录 / 零凭证。

## 2026-09-22 — tester-1 — TEST-EXEC + BUILD-348 ✅ 重出包（修 P0 吃字；八项 + 三特殊点全 PASS）｜🔴 BUILD-347 作废

- **重出原因**：BUILD-347 含 **P0 吃字**（`OrderedReflow::push` 对齐失败时无条件替换 `last_window_text` ⇒ 滑出文本永久丢失）。由 **`FIX-ORDERED-REFLOW-DROP-368`（HEAD `4201d39`）** 修复：新增常量 `REFLOW_FALLBACK_FAILS=3`，连续失败达阈值把旧 `last_window_text` 整体并入 committed（兜底不去重）。
- **差异**：368 仅改 `src/transcription/mod.rs`（+133/−18）+文档；**未动** `align_overlap`/`group_window_start_secs`/管线接线/dll/模型/itn ⇒ dll 与 itn 三副本、`Publish/models/` 1.7B **仅核验不重拷**（结果均通过）。
- **回归（@ `4201d39`）**：root `cargo test --no-fail-fast` **1467P/0F/30I**（EXIT 0；`feiyin-ime` bin **1379P/28I** = 任务书基线逐位吻合）；`src-tauri` **92P/0F/0I**；Vitest **7 files/100P/11S/0F**（`ui/` 无 diff 仍执行）；`cargo fmt --check`（不带 `skip_children`）**EXIT 0**。
- **NEW/GONE**：NEW **3** = 368 三条 `ordered_reflow_*`；**GONE 0**（3 条旧乱序测试原地换长文本样本、名字仍在）。⚠️ 任务书写「4 条新单测」，实测 `git diff` 增 `#[test]`=3；`1376+3=1379` 与基线自洽 ⇒ 以实测 3 为准。
- **BUILD-348**：Step1 残 0 → Step2 npm 610ms + Tauri 1m51s（17w）→ Step3 主程序 2m43s（**98w** + crash 9w）→ Step4 UI 同步 + 三 exe→Publish。产物 main `f9ba2822c731…`（14,739,456B/20:02:04）/ ui `7d72bced2fc6…`（10,050,048B/19:59:10）/ crash `61e3ab00869b…`（24,879,104B/20:00:19）；两副本全等、均异于作废的 BUILD-347（`edf7d088…`/`af045bc8…`/`6d71c620…`）。
- **八项逐项 PASS**：①时间戳 19:59–20:02 ②sha 两副本 + 异于上包 ③**0.9.3** ④冒烟 PID **8112 Responding=True** / 两处无新 crash.json / panic 0 / 残 0 ⑤config `da2be5da…` 三时点不变 + wordbook 不变 ⑥warnings **98/88/17**（test 88 vs 基线 89，−1 如实报）⑦探针（正 `SLIDING-WINDOW-367`=**4**/`[LocalRT-DBG-298]`=**3**/`AUTOLEARN`=**11**；反 `sub-seg failed`=0/`PUNCT_REFRESH_INTERVAL`=0/`ACC_MIN_SEGMENT_MS_DEFAULT`=0/`min_seg_ms`=0）⑧四表三副本全等。
- **三特殊点（仅核验）**：① dll 四张 `sherpa-onnx-lib`=`Publish`=`target-release` 三副本全等，`onnxruntime.dll`=`422d776a…`、**ProductVersion=1.28.2**；② itn-rules 三副本 `60b227de…` 全等；③ `Publish/models/` 1.7B 齐全且与源逐一 sha256 全等（0.6B 保留）。
- **Gavin 端测**：🔴 **置顶新增=吃字/重复字重点复测**（说 4 句以上让滑窗多次，看最终文本有无整段消失，368 修的就是这个）+ 原九条（松手等待时间/预览修正/接缝重复/标点语义/1.7B 质量/梅开二度·一度/长录音 180s/不新增 crash.json/重启输入法）。
- **红线**：版本未动 / 未改生产代码 / 未 push / 未 `cargo clean` / 未动 models 源目录 / 零凭证。

## 2026-09-22 — tester-1 — TEST-EXEC + BUILD-347 ✅ 出包（v0.9.3 批次十一；八项 + 三特殊点全 PASS）🔴 后因 P0 吃字作废

- **交付源码**：HEAD **`ae166e6`**（含主控 fmt 修复），版本 0.9.3。范围：354/356 ITN、359（sherpa 1.13.8/ORT 1.28.2 + Qwen3 **1.7B** + 剥前缀）、363/364（双路 + 上限 300→180s）、DEC-080/365（摘剥光标点节点）、367（滑动窗口四阶段）。
- **🔴 fmt 卡点（过程）**：首轮 `cargo fmt --check` EXIT=1（唯一 `src/transcription/mod.rs:4210`，367 测试 `assert!` 超宽）。判断「必须出包前修、否则重出」→ 上报主控；主控以 `ae166e6` 纯 rustfmt 折行修复。**tester 未改 src**，在 `ae166e6` **重跑全量**绑定交付。
- **回归（全量未过滤）**：root `cargo test --no-fail-fast` **1464P/0F/30I**（EXIT 0，11 二进制；`feiyin-ime` bin **1376P/28I** = 任务书基线逐位吻合）；`src-tauri` **92P/0F/0I**；Vitest **7 files/100P/11S/0F**（`ui/` 无 diff，仍执行）；`cargo fmt --check` **EXIT 0**。
- **NEW/GONE（对 BUILD-346，逐位对账 1376−1347=+29P / 28−21=+7I）**：NEW 30 runnable（356 ITN 8 / 367 滑窗+对齐+有序定稿 12 / 359 剥前缀 4 / 363-364 预算 4 / 365-DEC080 2 / 重命名 1）+ 7 ignored PoC；**GONE 1 = `max_record_seconds_is_300`→`_is_180`**（上限调整，机制随单）。367 重写**未删任何测试函数**（`-#[test]`=0），核心有序定稿判据 `ordered_reflow_*` 3 条为新增。
- **BUILD-347**：Step1 残 0 → Step2 npm 640ms + Tauri 1m47s（17w）→ Step3 主程序 2m53s（**98w** + crash 9w）→ Step4 三 exe→Publish + dll 三副本 + 四表三副本 + 补 1.7B。产物 main `edf7d088b74e…`（14,737,920B/19:28:12）/ ui `af045bc855c7…`（10,050,048B/19:25:18）/ crash `6d71c620bbe9…`（24,879,104B/19:26:21）；两副本全等、均异于 BUILD-346。
- **八项逐项 PASS**：①时间戳 19:25–19:28 ②sha 两副本 + 异于上包 ③**0.9.3**（main/crash `0.9.3.0`、ui `0.9.3`）④冒烟 PID **28280 Responding=True** / 两处无新 crash.json / panic 0 / 残 0 ⑤config `da2be5da…` 三时点不变 + wordbook 40960B/Sep10 不变 ⑥warnings **98 / 88 / 17**（基线 98/89/17 ⇒ **test 88 −1 如实报**，方向为减少）⑦探针 ⑧四表三副本全等（itn `60b227de…`/scene `8ea93bb1…`/homophone `a5fd4a61…`/wordbook `ac9a72ee…`）。
- **探针（`grep -a -o -F` 字面量）**：正 `SLIDING-WINDOW-367`=**3** / `[LocalRT-DBG-298]`=**3** / `AUTOLEARN`=**11**；🆕 反 `sub-seg failed`=**0**（路A 每片解码字面量，随 367 废弃）/ `PUNCT_REFRESH_INTERVAL`=0 / `ACC_MIN_SEGMENT_MS_DEFAULT`=0 / `min_seg_ms`=0。⚠️ `PARALLEL-ACC-298` **不可作反向探针**（该机制仍活，`main.rs:10583` 有生产字面量），故取其被删的 `sub-seg failed` 分支。
- **三特殊点**：① **dll 三副本**：出包前 `target/release` 已是新 1.28.2，但 `Publish/` 与 `sherpa-onnx-lib/` 仍旧 1.24.4 ⇒ 已同步，四张 dll 三副本 sha256 **全等**；`onnxruntime.dll`=`422d776a…`、**ProductVersion=1.28.2**；旧 1.12.38 目录**保留未动**。② **itn-rules.toml**：出包前 root `60b227de` vs Publish/target-release `ab950ba4` 分叉 ⇒ 已同步，三行全等 `60b227de`。③ **Publish/models 1.7B**：出包前**缺失** ⇒ 已补拷（3 onnx + tokenizer，与源逐一 sha256 全等）；0.6B 保留。
- **Gavin 端测九条**：🔴 松手后等待时间（核心收益）／预览修正质量（二比零→二比一）／🔴 **吃字重字（最大风险，重点观察）**／接缝重复消失／标点按语义／1.7B 转写质量／「梅开二度」「他一度以为」不再误转／长录音近 180s + 不新增 crash.json／⚠️ 出包强杀输入法，完成后重启。
- **红线**：版本未动 / 未改生产代码 / 未 push / 未 `cargo clean` / 未动 models 源目录 / 零凭证。⚠️ 模型拷贝首次 robocopy 挂死，改分步 `cp` 完成。

## 2026-09-22 — tester-1 — TEST-EXEC + BUILD-346 ✅ 出包（v0.9.3 批次十；八项 PASS，正反向探针 3 归零）

- **范围**：`701c4d8`(ACC-DISPATCH-SILENCE-ONLY-346) / `e2f259f`(PUNCT-PREVIEW-SEMANTIC-349) / `c9b59b3`(PUNCT-FINAL-REDO-350) / `e829c67`(零行为提取) / `1269ddc`(TEST-SYNC-352) / `f335bd8`(351 PoC) + 版本升 **0.9.3**。绿灯前置：允许 351 停写并单独 commit 后才开跑（脏树风险已上报主控确认）。
- **回归（全量未过滤；`cargo fmt --check` 不带 `skip_children` EXIT 0）**：root `cargo test --no-fail-fast` **1435P/0F/23I**（EXIT 0，11 二进制）；`src-tauri` **92P/0F/0I** = 基线；Vitest **7 files/100P/11S/0F** = 基线。
- **NEW/GONE 对账（逐位）**：1420→1435 = **+15P**，22→23 = **+1I**。NEW = 349 2 条 + 350 7 条（`strip_node_*`/`test_350_*`）+ 352 6 条（`sync352_*`）+ 351 `poc_17b_351_fitness`（`#[ignore]`）。**GONE = 0**：346 两条（`acc_should_dispatch_or_semantics`/`acc_should_dispatch_honors_env_thresholds`）**已被 coder-2 的 1420 基线吸收**；349 实测 `+2/-0`、无删除用例（任务书预期的「349 删 4s 定时用例」与实测不符）。相对 BUILD-345（`d87b8b4` 1419P/22I）则 GONE=2、NEW=18+1I（1435=1419+18−2 自洽）。
- **e829c67 零行为提取鉴定**：`+12/-0`，函数体原样搬入 `create_qwen3_recognizer_at` + 原函数委托，签名/调用点/硬编码目录名不变；0 failed / 无 GONE / 无行为差异 ⇒ **判据成立**。
- **BUILD-346**：Step1 清残 0 → Step2 npm 1.45s + Tauri 1m25s（17w）→ Step3 主程序 3m11s（**99w** + crash 9w）→ Step4 同步 `Publish/` 三 exe + 四表三副本；UI 两处时间戳 `cp -p` 后逐位一致（11:33:49.338）。产物 main `98df432cccca…`（14,677,504B/11:37:09）/ ui `ea68c4023e52…`（10,050,048B/11:33:49）/ crash `887957b195f7…`（24,879,104B/11:35:11）；两副本全等、三者均异于 BUILD-345。
- **八项逐项 PASS**：①时间戳 11:33–11:37 ②sha 两副本 + 异于上包 ③**0.9.3**（main/crash `0.9.3.0`、ui `0.9.3`）④冒烟 PID **18876 Responding=True** / `%APPDATA%` 与 `target/release` 两处**均无新 crash.json** / panic 扫描 0 / 残 0 ⑤config `da2be5da…` 三时点不变 + wordbook 40960B/Sep10 不变 ⑥**warnings 99/90/17** = 基线 ⑦探针 ⑧四表三副本全等（itn `ab950ba4…`/scene `8ea93bb1…`/homophone `a5fd4a61…`/wordbook `ac9a72ee…`）。
- **探针（`grep -a -o -F` 字面量）**：正向 `[LocalRT-DBG-298]`=**4** / `[LocalRT-DBG-325]`=**2** / `AUTOLEARN`=**11** / `Punctuation strip node`=**1**（`main.rs:10965` 生产日志，非 test）；🆕 **反向 `PUNCT_REFRESH_INTERVAL`=0 / `ACC_MIN_SEGMENT_MS_DEFAULT`=0 / `min_seg_ms`=0** ⇒ 349 的 4s 定时与 346 的长度支彻底移除。三符号在 src 仅残留注释，与二进制 0 自洽。
- 🔴 **时间线边界（待主控知悉）**：回归/构建均基于 7-commit 范围（`f335bd8`）；`4112354`(353 RETEST, +73) 于 **11:38:07** 提交（晚于主程序产物 11:37:09），`mod.rs` 于 11:39:46 再次 dirty（+10）—— 两者均纯 `#[cfg(test)] mod poc_qwen3_17b_351`、不进 release、不影响产物，但 **353 测试代码未纳入本次回归**（超范围，若需覆盖须补跑 `cargo test`）。绿灯时树为 clean（仅两份 `.bak` untracked）。
- **Gavin 端测六条**：①核心=预览标点不打句中 ②⚠️已知代价：长不停顿说话预览持续无标点（到期 1200ms 才打）——请明确能否接受 ③350 最终标点无 `。。` ④在线 realtime / 本地 performance 两档标点须与上版完全一致 ⑤346 `[LocalRT-DBG-298] seg dispatch` 的 `silence=` 恒 ≥1200ms ⑥不得新增 `target/release/crash.json`。
- **红线**：版本由主控升未动 / 未改生产代码 / 未 push / 未 `cargo clean` / 未动 `models/…-1.7B-…`（2.2GiB）/ 零凭证。

## 2026-09-22 — coder-1 — UNWIRE-STRIP-NODE-365（365+366 收口）✅ 交付

- **365 条件挂载**：350「剥光标点重打」节点**路B 主路径摘除、降级分支仍挂**（保留代码）。`main.rs`：`b_strip_enabled = path_b_text.is_none() && config.punctuation.enabled && !start.translate`，调用 `strip_punctuation_node(normalized, b_strip_enabled)`；注释记录 何时/为何摘、为何留降级分支、何时整体回挂、334 由 `!native_punctuated` 门兜住。源码级顺序护栏 `sync352-4` 仅同步锚点、语义不变。
- **366 三件全不做**：🥇 满核**实测负收益**（16 vs 8：20s 慢 1.73×、56s 慢 1.79×）⇒ 取消；② 取消路A 在飞会丢降级 fallback ⇒ 不做；③ 路B 无中断 API ⇒ 不做。
- **366 PoC 保留为证据**：`make_qwen3_threads` + `#[ignore] poc_366_threads`；`default_acc_num_threads` 注释补「min(cores,8)=实测最优、别调高」。
- **验证**：`cargo build --release` EXIT 0（构建前 kill 占用 exe 的残留 `feiyin-ime` PID 12220）；全量 `cargo test --no-fail-fast` 0 failed（1363P/28I）；350 七单测 + sync352 护栏全绿；warnings 98/89 基线；numstat==-w。未碰 `local_stream.rs`。
- **未出包**（DEC-079）；未改版本；未 push；零凭证。

## 2026-09-22 — coder-1 — DUAL-PATH-REFINE-364 ✅ 交付（录音上限 180s + 预算精确计算）

- **Gavin**：批准「估算→精确计算、缩小余量」；录音上限 300→180s。
- **Part1**：`config/mod.rs` `MAX_RECORD_SECONDS=180`；连带断言 `max_record_seconds_is_180`、`hotkey` 文案 305→185（`+5` 算式未动）、`qwen_inference` 注释 300→180；全仓 grep 无其它硬编码。
- **Part2**：`expected_audio_tokens`（移植 C++ `FeatToAudioTokensLen`，chunk=100；20s→260 吻合实测、180s→2340）+ `estimate_inject_tokens`（对实际注入 system 串 tokenizer 实数 ×2.0 差异系数，含跨录音上下文+词库）；`SAFETY_MARGIN` **512→64**；删旧估算常量。判据 `audio+inject+256+64 ≤ 4096` ⇒ 阈值 ≈`(3776−inject)/13`s（典型≈272s、带满上下文≈213s）；**180s 不降级**；闸门保留防未来大词库/改上限。
- **CONTEXT_RESERVE 含义不变**（跨录音语意背景，必须扣），数值来源改实数。
- **验证**：`cargo build --release` EXIT 0；全量 `cargo test --no-fail-fast` 0 failed（1363P/27I）；warnings 98/89 基线；numstat==-w；单测全绿。路A 未碰 `local_stream.rs`。
- **未出包**（DEC-079）；未改版本；未 push；零凭证。
- **端测**：180s 满长仍不降级（若日志见 `路B 降级` 回报）；满长松手等待（~74s）实感。

## 2026-09-22 — coder-1 — DUAL-PATH-ACC-363 ✅ 交付（acc 双路：路A 切片刷预览 + 路B 累积全量出终文）

- **Gavin 指示**：acc 后台分两路 —— 一路实时预览的最新切片回灌刷新预览；一路累积喂入形成最终输出文本。
- **路A 零改动**：1200ms 静默探测（346）/ 派发 / 回灌（325/329）判定体一行未动，**不碰 local_stream.rs**。
- **路B 新增**：松手后在 `acc_handle.join()` 之后跑**一次全量解**。全量音频 = ASR 线程返回的 `local_pcm`（`local_stream.rs:462`）⇒ **无额外累积、主路径零新增开销**；**不中途预解**（361 证无增量复用）。调用 `transcribe_acc_ctx`，**注入跨录音上下文（ctx_prev2/ctx_prev1）+ 词库**，`current=None`。成功 ⇒ 全文**整体替换**切片拼装；失败/降级 ⇒ 退回拼装。复用 `pretranscribed` 通路、最小改动。
- **预算闸门** `path_b_budget_ok(audio_secs, terms_tokens)`：`音频×13 + 词库 + 生成256 + 提示词45 + 上下文385 + 余量512 ≤ 4096`；不够 ⇒ 路B 降级 + log::info!（原因+秒数）。常量注释：CONTEXT_RESERVE=跨录音背景（真实需要必须扣）/ SAFETY_MARGIN=防估算偏差撞顶（DEC-069，故意不用满）/ 200s=典型词库等价参考值非实测边界。
- **验证**：check 0 error；**`cargo build --release` EXIT 0**；**全量 `cargo test --no-fail-fast` 0 failed**（root 1363P/27I）；warnings **98/89 = 基线**；`--numstat`==`-w`（main 94/2、transcription 100/0）；闸门单测 4 条全绿。
- **阶段一方案**主控确认（并纠回第③点：CONTEXT_RESERVE 保留 + 路B 必须注入跨录音上下文）。
- **未出包**（DEC-079）；未改版本 / 未 push / 零凭证。
- **端测清单（交 Gavin）**：① 接缝重复（多名多名）是否消失 ② 预览刷新是否仍然快（路A 未动）③ 松手后等待（一次全量解，RTF≈0.41）实感是否可接受 ④ 超长录音（>~200s）是否正确降级并打日志 ⑤ 全量解是否自我纠错前文。

## 2026-09-22 — coder-1 — POC-TIMESTAMP-DECODE-CURVE-361 ✅ 交付（RP-1 时间戳不填值；decode 线性、无增量复用）

- **任务**：RP-1 时间戳可用性 + RP-3① decode 开销曲线（纯 PoC，零生产代码；走生产 `create_qwen3_recognizer_at` 路径）。
- **① RP-1 失败**：`timestamps`/`durations` 均 `Some(len=0)`（`tokens` 有 61 个）⇒ **不填值**，非「有值不准」⇒ **RP-2 时间戳拼接路断**（除非引独立 aligner）。已避开「字段非空=通过」陷阱。
- **② RP-3① decode 开销近似线性增长、无增量复用**：1s=691ms→10s=3087→20s=6633→30s=9456→56s=**22885ms**，斜率≈0.41 s/s，严格单调 ⇒ 每次从头解 ⇒ **累积重识别整段线性膨胀，须窗口封顶**；Gavin「应有内部优化」不成立、主控判断成立。A 选项（累积喂同一 stream）也省不了模型侧计算。
- **③ 同前缀重复解逐字稳定；随音频增长前文被改写**（6s「二比零领先」→8s「二比一爆冷」）⇒ 豆包式回改文字层存在，采纳即交互模型变更（DEC-054/051 需重评）。
- **结果入** `collab/research/qwen3-1.7b-capability-roadmap.md`「RP-1/RP-3① 实测结果」节；PoC `d7650b3`（+119/-0 纯测试）；日志 `collab/evidence/20260922-qwen3-361/`。
- **边界**：只测 B 选项（新建 stream 全量喂），A 未测；单样本/中文；准度对比需 Gavin 语料。
- **红线**：未改任何生产代码 / 未碰 `src/itn.rs` / 未改版本 / 未出包 / 未 push / 零凭证。

## 2026-09-22 — coder-1 — MIGRATE-1.13.8-1.7B-359 ✅ 交付（升库+换 1.7B+剥前缀，一批到位）

- **Gavin 决策**：不分两批，直接升 1.13.8 + 上 1.7b + 前缀 B（不锁语种）；**回滚整批一起回**。
- **① 升库**：`Cargo.toml` sherpa-onnx → `1.13.8`；lock 已 `sherpa-onnx(-sys) 1.13.8`。
- **② 换 dll**：官方 1.13.8 `shared-MD-Release` 归档 → `vendor/sherpa-onnx/sherpa-onnx-v1.13.8-win-x64-shared-MD-Release/`（ORT **1.28.2**）；`SHERPA_ONNX_LIB_DIR` 指其 `lib/`。🔴 **旧 1.12.38 目录原样保留＝整批回滚备份**；新目录去 `-lib` 后缀复用 `.gitignore`（不入 git，同 1.12.38）。
- **③ 换模型目录**：常量 `QWEN3_MODEL_SUBDIR`（唯一来源）接入 5 处：`check_qwen3_model_ready` / `hotwords_tokenizer` 生产 tokenizer.json 路径 / `test_tokenizer()` 与 readiness 测试夹具 / `src-tauri/src/main.rs`（独立 crate 镜像）/ `audio` gate335 helper。生产路径无 0.6B 残留；旧 0.6B 目录保留（回滚+test_wavs）。
- **④ 剥前缀（语言无关）**：`strip_qwen3_language_prefix` + `is_qwen3_language_label` 并入 `strip_asr_special_tokens`；规则=开头 `<asr_text>` 且前缀「标签样」（官方 `language …` 或 ≤8 字纯字母语种名单词、且 `<asr_text>` 紧贴）；无标记⇒原样（**0.6B no-op**）。单测 4 条（多语种/无前缀/退化不误伤/与 `<|…|>` 叠加）。
- **验证**：`cargo build`(debug) + **`cargo build --release`** 均 EXIT 0；src-tauri check EXIT 0；**全量 `cargo test --no-fail-fast` 0 failed**（root 1359P/0F/26I）；warnings bin release **98**<基线 99、debug test 89<90；`--numstat`==`-w`。
- **⚠️ 瞬时 FAIL 排除**：首轮一次 `itn::tests::itn356_*` FAIL（单跑 ok、二轮全绿）；`src/itn.rs` 正被 coder-2 356 编辑 ⇒ 他人 WIP，非本批，未改任何 itn 文件。
- **未出包**（DEC-079）；dll 分发副本同步（Publish/target）归 tester-1 出包时做。回滚三步：Cargo.toml→1.12 / cargo update / 指回旧 lib。
- **红线**：未碰 `src/itn.rs`·`itn-rules.toml` / 未改主程序版本 0.9.3 / 未设 `language` 选项 / 未 push / 零凭证。

## 2026-09-22 — coder-1 — UPGRADE-SHERPA-1.13.8-358 ✅ 交付（接口 diff：接口零风险、行为有真风险）

- **任务**：Gavin「升级 1.13.8 适配 1.7b，注意接口参数有无变化」；**第一步只做接口 diff，不真升级**。纯读源，零文件改动。
- **方法**：下载 crates.io `sherpa-onnx(-sys) 1.13.8` .crate 解压到临时目录，与本地 1.12.38 源逐项 diff + 读 release notes/PR diff。
- **① 接口兼容**：`offline_asr.rs` diff **仅 +8 行**（新 `unsafe impl Send/Sync` for OfflineRecognizer/OfflineStream）；`lib.rs` 仅 doc；其它模块仅 +Send/Sync；**10 字段/所有签名零变化 ⇒ 预期零改动编译通过**（A 级；B 级编译待真升级单）。✅ 无冲突（仅对自有类型 `SendHwnd`/`Transcriber`/`SendOfflineRecognizerRef` impl Send）。features 不变。
- **② 行为风险**：🔴 **#3873（1.13.7）centered-STFT 改特征 ⇒ 同一音频输出可能变、**含 0.6B** ⇒ 升级后必须全量回归 + 0.6B/1.7B 端测**。**#3907（1.13.7）只覆盖「整段全静音」**（不修 ja1「有语音却幻觉」）。**#3912（1.13.8）PRNG 竞态现状不触发**（accuracy worker 单线程串行 `transcribe_acc_ctx`，main.rs:8210；num_threads=8 是 ORT 内并行）。onnxruntime 1.24.4→1.28.2。
- **③ dll**：`SHERPA_ONNX_LIB_DIR` 短路由 1.13.8 sys build.rs **保留** ⇒ 换目录即可；选 **`shared-MD-Release`** 档（与现 1.12.38 同 CRT），归档 `sherpa-onnx-v1.13.8-win-x64-shared-MD-Release-lib.tar.bz2`（已确认存在）；含 `sherpa-onnx-c-api.dll`/`onnxruntime.dll(1.28.2)` 等，随 exe 分发。
- **④ 步骤草案**（未执行）：下归档→换 `SHERPA_ONNX_LIB_DIR`→改 Cargo.toml 版本→build（更新 lock，属下一单）→`cp` dll→全量回归→0.6B&1.7B 端测；回滚=指回 1.12.38。版本/出包按 DEC-079 须 Gavin 同意。
- **产出** `collab/research/sherpa-1.13.8-upgrade-358.md`。
- **红线**：未真升级 / 未改 Cargo.toml·lock / 未编译 / 未碰 `src/itn.rs`·`itn-rules.toml` / 未改版本 / 未 push / 零凭证。

## 2026-09-22 — coder-1 — RESEARCH-QWEN3-CALL-OPTIMIZE-357 ✅ 交付（提示词能传但模型不照做）

- **任务**：Gavin「摸清 1.7B 特性，看提示词传入/文本优化/ITN 有没有能起作用的」；前提=换 1.7B 已拍板。纯调研+只读实测，零生产代码。
- 🔴 **命门结论**：**`hotwords` 通道 = system prompt 通道**（C++ `offline-recognizer-qwen3-asr-impl.cc:49-52` 把 `hotwords` 包进 `<|im_start|>system\n…<|im_end|>`；`:56-59` 逗号→空格）⇒ **提示词能传、且我们一直在传**（生产 `decode_accuracy_once` 的 `set_option("hotwords", s)`）。**A/B 答案 = B（能，入口叫 hotwords），不是「不吃」也不是「没接线」。**
- 🔴 **但「能传」≠「照做」**：ITN 指令 **零效果**（`itn.wav` 3 条指令逐字不变）；去口水词**几乎不变**（英文指令→**空输出**）；格式指令**反噬**（「用简体中文输出」→输出大幅截断）。**与 DEC-070 同型。** ⇒ ITN/文本清理**继续自研，不甩给模型**。
- **通道细节**：逗号被换成空格（`Terms: a,b,c`→`a b c`）；hotwords token 计入 `before_len` 挤音频 KV（C++ `:849-871` 告警）。
- **版本侧**：`sherpa-onnx 1.12.38` 与最新 `1.13.8` 的 `OfflineQwen3ASRModelConfig` **字段相同（10 个、无 prompt/itn）**；上游 C++ master 的 `BuildSourceIds` 只有 `hotwords`+`language` ⇒ **升级 crate 拿不到 prompt 字段**。
- **参数建议**：7 个写死值对 1.7B **保持**（同 355）。
- **PoC** `fab61cc`（+97/-0，纯 `#[cfg(test)]`）；产出 `collab/research/qwen3-call-optimize-357.md` + `collab/evidence/20260922-qwen3-357/`。
- **红线**：未改生产代码 / 未碰 `src/itn.rs`、`itn-rules.toml` / 未改版本 / 未 push / 零凭证。

## 2026-09-22 — coder-1 — RESEARCH-QWEN3-1.7B-CAPABILITY-355 ✅ 交付（能力增量：无 B 级增量；调用层：无可调优点）

- **任务**：Gavin「看换 1.7B 有哪些收益（0.6B 没有的）」+「注意调用上与 0.6B 的不同点、可着手调优的」。纯调研+PoC，零生产代码。
- **结论**：**无 B 级能力增量**。官方卡对 0.6B/1.7B 同一段功能描述 ⇒ 功能集相同；实测 ITN/文本清理**无增量**，专名 `·` 1.7B **更差**，语种识别两者都有（1.7B 前缀**判错**：韩语→`汉语`）。
- **调用层**：①7 个写死字段对 1.7B **语义相同、无需改**；唯一行为差异=1.7B 吐 `X<asr_text>` 前缀。②建议全部保持（`max_new_tokens=256` 对 30s 单段 135 字未截断，256/512 逐字相同）。③**未发现只对 1.7B 划算的调用方式**（30s 大分片两者都可行）。④前缀**无法从配置关闭**（`SherpaOnnxOfflineQwen3ASRModelConfig` 仅 10 字段、无 language/prompt/itn，`c-api.h:999-1021`），只能应用层剥离。
- **附带**：生产恒发注入是**承重墙**（`system=None` 时 ja1/codeswitch 两模型都幻觉，带注入才正常）；**1.7B 对注入更敏感**（同 `CLEANUP_INSTR_EN` 出「传播不名」）。
- 🔴 **崩溃未定性**：4 次运行 1 成功 / 3 次 `0xffffffff`，崩点不定（1.7B 重建/首建/S1 1.7B 解码）；无 `crash.json`；空闲内存 30GB ⇒ **非系统 OOM**；**不宣称根因**，建议空闲机专项复现。
- **PoC**：`5651ccc`（+196/-3，纯 `#[cfg(test)]`）；产出 `collab/research/qwen3-1.7b-capability-355.md` + `collab/evidence/20260922-poc-qwen3-17b-355/`。
- **待 Gavin 真实语料**：中英混/日韩差异、`·` 是否普遍、注入敏感是否稳定、精度实际幅度。
- **追加（前缀可抑制）**：运行时支持 **per-stream `language` 选项**（`offline-recognizer-qwen3-asr-impl.cc:823-826`）；设对⇒前缀消失、部分语种更准；设错⇒改坏输出；`language=None`（现生产）下正文对错并存（ja→中文幻觉、中英混→中文乱写、英语带噪→空输出）⇒「不设+语言无关剥离」非安全方案。运行时已内置 1.7B 复读坍缩兜底（upstream #3535）。**所有结论 n=1，须扩充真实语料。** 报告已加「最终收尾」；1.7B 模型目录保留不动。
- **追加（崩溃）**：`0xffffffff` = `SHERPA_ONNX_EXIT(-1)`（`_Exit(-1)`，`macros.h:54-59`），用于 ONNX metadata 读取失败等；**但崩溃前无 LOGE 文案** ⇒ 仍未定性。探针 `06d54a6`。
- **红线**：未改生产代码 / 未碰 `src/itn.rs`、`itn-rules.toml` / 未改版本 / 未 push / 未测 CER、未报速度内存 / 零凭证。

## 2026-09-22 — coder-1 — POC-QWEN3-1.7B-RETEST-353 ✅ 交付（生产口径重测：**无统计显著差异**；口径修正是重点）

- **背景**：Gavin 新约束「本地 realtime 只有极客用户、内存不是问题、要精度」⇒ 347 §五内存/速度判据作废、**唯一判据=生产口径精度**。主控发现 351 在 **ITN 之前**测 CER。
- **生产链复现**（只读调用、零生产改动）：`itn::normalize_numbers → normalize_text_for_language → itn::normalize_unit_symbols_only`（`main.rs:10501/10582`）；其后标点/口水词/L2 不影响 CER。日志 `collab/evidence/20260922-poc-qwen3-17b-351/poc-353-itn-raw.log`。
- **两套数字**：0.6B 裸 0.0356(8) → **生产 0.0267(6)**；1.7B 裸 0.0444(10) → **生产 0.0356(8)**（剥前缀）；含泄漏前缀 0.1467(33)。
- **逐条差异**：0.6B 裸 8 = 八分之一(4,ITN可修)+二比一(2,ITN不修)+多名重复(2,切片伪影)；生产 6 含 **ITN 反引入 2 处过度转换**(梅开2度/第2轮)。1.7B 与 0.6B 差距 = 仅 2 个缺 `·`。
- 🔴 **「比」不修**（`二比一`→`二比一`；`八分之一`→`1/8` 命中分数通道）。**只查不改**。附带发现 ITN 过度转换 2 处，亦只报不改。
- 🔴 **前缀代价**：锚定剥 `^language \w+<asr_text>`，~5–10 行+单测、无接口改动、误剥 0.6B 风险低、须端测。**只查不改**。
- **结论**：n=1（差 2/225，集中在专名中点号）**无统计显著差异，需扩充语料**；「为精度换 1.7B」当前不成立。
- **PoC**：`4112354`（+73/-0，纯 `#[cfg(test)]`）。
- **353-H 追加（主控立单用）**：两处 ITN 过度转换均走**主通道** `normalize_with_rules`——①`梅开二度→梅开2度` 经温度单位 `[units.temperature]`（`itn-rules.toml:40-42`），对照 `一年一度` 在保护表（`:393`）实测不变而 `梅开二度` 不在（`grep 梅开` 零命中）⇒ 根因=成语未入保护表；②`第二轮→第2轮` 经序数 `[ordinal] prefix="第"`（`:129-130` + `itn.rs:2199-2205`），**DEC-030-③ 设计行为**，是否算缺陷待 Gavin 定口径。探针 `0bf60f1`。
- **红线**：未改任何生产代码 / 未改版本 / 未 push / 无速度数字 / 零凭证。

## 2026-09-22 — coder-1 — POC-QWEN3-1.7B-351 ✅ 交付（1.7B 实测：不换；发现输出前缀硬不兼容）

- **任务**：填 347 留的两个空白（本机 CPU RTF + 1.7B 中文 CER），纯 PoC 零生产逻辑。
- **前置重构**（主控批准）：`create_qwen3_recognizer_at` **零行为变更提取**，独立 commit **`e829c67`**（+12/-0，函数体原样搬入、原函数缩为委托、签名/调用点不变、不动硬编码目录名）。
- **PoC**：commit **`f335bd8`**（`src/transcription/mod.rs` `#[cfg(test)] mod poc_qwen3_17b_351`，+271/-0）；复用生产构造链 + `transcribe_acc_ctx`，不手搓配置；机器 16 核 / acc_threads=8 / full.wav 56.15s / 切片与 313 逐字相同。
- **读数**（原始日志 `collab/evidence/20260922-poc-qwen3-17b-351/poc-351-raw.log`）：0.6B `peak_priv=2020MB rtf=0.218 cer=0.0356`；1.7B `peak_priv=4967MB rtf=0.324 cer=0.1556(含前缀)`；**剥前缀后 1.7B cer=0.0444**。
- 🔴 **控制前缀（推翻 347 结论）**：1.7B 输出含 `language chinese<asr_text>`，生产 `strip_asr_special_tokens` 只剥 `<|…|>` ⇒ 漏进用户文本；两模型 tokenizer 逐字节相同 ⇒ 是 ONNX 图/元数据不兼容。**1.7B 非 drop-in。**
- **结论：不换**——①CER 未优反劣（0.0444>0.0356）②RTF 1.36–1.49× ③**分片态**峰值 4.97GB 越界（> DEC-076 否决的 4.3GB）。
- **方法教训**：按 decoder 权重比 2.70× 外推耗时 → 实测 1.36–1.49×，高估 1.8–2.0×；选型须实测。
- **边界**：CER n=1（单条 56s），非统计结论；`andrewleech/...-onnx` 未用（主控已证不兼容）。
- **红线**：未改版本 / 未 push / PoC 不夹带产品决策 / 未碰 local_stream.rs / 零凭证。

## 2026-09-22 — coder-1 — PUNCT-FINAL-REDO-350 ✅ 交付（标点剥离独立节点，只挂本地 realtime）

- **需求（Gavin 2026-09-22）**：最终 acc 转写完成后剥光整段已有标点、再交标点模型整段重打；**只针对本地 realtime，其他管线不能动**（在线 ASR 标点可能更准）。设计细化：剥离做成**独立节点**、谁要谁挂，不绑死管线。主控前两稿 v1（四档一视同仁）/v2（`run_pipeline_core` 加 Fill/RedoWhole 模式参数）均作废，**v3 为准**。
- **实现**：`src/punctuation/mod.rs` 新增共享谓词 `is_effective_punctuation`（`has_effective_punctuation` 改调它、行为不变）+ 严格对偶纯函数 `strip_effective_punctuation`；`src/main.rs` 新增自由函数节点 `strip_punctuation_node`（形态照 `apply_filler_strip`），接线在本地 realtime 自有编排块（normalize 之后、`native_punctuated` 之前）⇒ 剥光后 `native_punctuated` 恒 false ⇒ 下游既有门自动放行整段重打。
- 🔴 **DEC-066 证据（共享代码零 diff）**：`run_pipeline_core` 签名**不在 diff**；`apply_local_punctuation` 与 HEAD **md5 逐字节相同**（`8094f722…`）；v1/v2 期间动过的共享代码（`native_punctuated` 参数、`pretranscribed_native_punctuated`、334 测试名、`PunctuationNodeMode`/`PunctuationRunner`）**全部恢复原状**。其余两档结构上不可能受影响。
- **门 = `punctuation.enabled && !start.translate.load(...)`**：翻译路径下游 `apply_local_punctuation` 被挡不重打 ⇒ 整段跳过；`llm_handled` 接线点读不到，核实结论 = LLM 路径下剥光文本只是 LLM 输入（LLM 自带标点输出）、LLM 失败回落 CT 重打 ⇒ 无害。
- **单测 7 条**：穷举性质测试（`has(strip(t))==false`）、词内嵌边界外、334 重复标点回归、「剥光了必定打得回来」不变式。
- **验证**：fmt 全仓 clean / `check --all-targets` 0 error、warnings 99/90 / `--numstat`==`-w`（main 138/2、punctuation 126/10）/ 全量 `cargo test --no-fail-fast` **0 failed**。
- **未决边界（只报不改）**：并行 acc 不可用回落内部转录时不经本节点（原行为）。
- **红线**：未动版本号 / 未 commit / 未 push / 未碰 `src/transcription/local_stream.rs` / 零凭证。

## 2026-09-22 — coder-1 — RESEARCH-QWEN3-1.7B-347 ✅ 交付（纯调研，零生产代码）

- **任务**：Gavin 要求「上 HF 找下载率最高的 qwen3-asr 1.7b，评估替代 0.6B 可行性；1.7b 也找 onnx 版，最大化向现役 0.6B 推理框架兼容」。
- **结论**：**有条件可行**。产出 `collab/research/qwen3-asr-1.7b-eval-347.md`。
- **头号门禁**：官方（k2-fsa / csukuangfj）**无** 1.7B sherpa-onnx 四件套 —— GitHub release `asr-models` **全量 499 asset 仅 1 个 qwen = 0.6B**；k2-fsa 文档只文档化 0.6B；`csukuangfj` HF 767 repo 含 qwen 者 0；`csukuangfj2` 仅 0.6B 镜像。⚠️ `releases/expanded_assets/...?page=N` 不响应分页，枚举须走 assets API。
- **关键发现（本单价值）**：现役 0.6B 的**同一来源** ModelScope `zengshuishui/Qwen3-ASR-onnx` 同时含 **`model_1.7B/`**；本机 0.6B 三文件 sha256 与该仓库 `model_0.6B/` **逐字节相同** ⇒ 1.7B 与现役 0.6B 同作者/同导出脚本（`Wasser1462/Qwen3-ASR-onnx`）/同四件套布局，**`OfflineQwen3ASRModelConfig` 代码零改动**。HF `thieunv-asilla` / `ilmina` 为逐字节相同镜像；`solavr` 为 mixed INT8/FP32（Q/K/V/O 保 FP32 防复读）独立导出。
- **数据**：1.7B int8 四件套 = 2,399,761,248 B（decoder 2,037,458,645 / encoder 314,222,162 / conv 48,080,441），+1.32 GiB；KV 每 token 与 0.6B 相同（28 层/8 KV 头/128，4096 → 896 MiB/流，cache 为动态轴无需重导）；官方 Offline 平均 WER 2.69 vs 0.6B 3.48；Apache-2.0。
- **🔴 诚实边界**：本机 CPU RTF **未查到**（唯一公开点 solavr RTF 0.743@2线程，CPU 型号缺失且为更重 mixed 导出）；1.7B 中文 CER 未查到；内存为以 0.6B 实测 4298MB 外推的**估算**。
- **建议**：不直接换默认，先做同音频 A/B PoC（判据：CER 显著优于 0.0444 + 延迟可接受 + 峰值内存不越界）。
- **红线**：未下载模型 / 未改 `src/` / 未改版本号 / 未 commit / 未 push / 未碰 `local_stream.rs` / 零凭证。

## 2026-09-22 — tester-1 — TEST-EXEC + BUILD-345 ✅ 出包（P0 崩溃修复 + DEC-077 回滚 + 344-G；八项 PASS，三机制探针归零）

- **基线/归属**：HEAD `d87b8b4`，**源码 clean**，版本 0.9.2。本包替换作废的 `8758ca66`。🔴 开工前 `M src/main.rs`（纯注释 4+/2−）→ 上报主控提交 `d87b8b4` 后放行（脏树不自行 commit）。
- **回归（全量未过滤；`cargo fmt --check` 不带 `skip_children` EXIT 0）**：root `cargo test --no-fail-fast` **1419P/0F/22I**（NEW 6/GONE 3）；`src-tauri` **92P/0F/0I**；Vitest **7 files/100P/11S/0F**。
- **增量**：`charboundary344_{mid_char_raw_len_does_not_panic,degenerate_inputs_fall_back_to_raw}`（**P0 护栏**）/ `reflow_hole_344_{segment_streaming_text_is_char_safe,fill_by_streaming_not_a_hole,assembled_text_stays_continuous}`（G）/ `endpoint_confirm_text_takes_longest_of_two`；GONE `..._of_three`（307 回滚）+ `tailpad340_*`（340 回滚）。
- **BUILD-345**：Step1–4 全走；源码 mtime 前后 md5 一致（`93469b74…`）。产物 main `ce10c4b7…`（14,675,456B/00:59，−7,680B vs 作废 342）/ ui `1e3df6d4…` / crash `01378435…`；两副本相等；异于作废 342（`8758ca66…`）与已发 341（`68e4528b…`）。
- **八项逐项 PASS**：①时间戳 00:55–00:59 ②sha ③0.9.2（Cargo.toml + tauri.conf.json 未动）④🔴 删旧 crash.json 后冒烟 PID 25220 Responding=True、**`target/release/crash.json` 未创建**、残 0 ⑤config/wordbook 本窗口内零变化（`Publish/config` `da2be5da…`、`target/release/config` `2e0e60a5…`、`Publish/wordbook` `b6ab43ac…`、`target/release/wordbook` `7e5755cb…`）⑥**warnings 99/90/17 未下降**（与预期不符；被删代码零 warning，如实报）⑦四表三副本全等（itn `ab950ba4…`/scene `8ea93bb1…`/homophone `a5fd4a61…`/wordbook `ac9a72eee…`）⑧探针。
- **探针**：自检 `feiyin`=19；正向 `337`=4/`336`=1/`325`=1/`AUTOLEARN`=4/`degree_adverbs`=2/`nz_ratio`=2/`342`=2；反向 332 五符号全 0；🆕 **345 反向 `feed_tail_silence`=0 / `SHADOW_TAIL_PAD_MS`=0 / `FLUSH_TAIL_PAD_MS`=0** ⇒ DEC-077 回滚彻底。
- **P0 验证**：`charboundary344_mid_char_raw_len_does_not_panic` 全量回归通过（复现「raw_len 落中文字符中间」不 panic）+ 冒烟不崩；⚠️ 原现场（长口述）本机无麦，交 Gavin。
- **Gavin 端测五条**：① 🔴 长句一口气说到底，`[LocalRT-DBG-325] action=skipped-hole` 应基本消失（上版 7 片里 6 片是它），请贴 action 分布 ② 🔴 不得新增 `target/release/crash.json` ③ 停手尾字 1~3s 被 accuracy 补上 ④ D 复核 `silence<800ms` 派发 ⑤ 其余（重复标点/编辑不被冲/自学习/配置界面/首字「你→按」）有观察记一句。
- **证据**：`collab/outbox/tester-1/testexec345/`（P0 旧 crash 备份在 `testexec342/crash-20260922-0000.json`）。
- **红线**：版本 0.9.2 未动 / 未改生产代码 / 未 push / 零凭证。

## 2026-09-22 — tester-1 — TEST-EXEC + BUILD-342 ✅ 出包（八项 PASS）＋🔴 crash 产物待裁

- **基线**：HEAD `fbec727`，clean，版本 0.9.2。含 342（D OR 根因 / A 松手非取消 / F1+F3 假 endpoint 护栏 / padding / B 结论）。
- **回归（全量未过滤；`cargo fmt --check` 不带 `skip_children` EXIT 0）**：root `cargo test --no-fail-fast` **1416P/0F/22I**（NEW 8/GONE 1）；`src-tauri` **92P/0F/0I**；Vitest **100P/11S/0F**。
- **BUILD-342**：Step1–4 全走；源码 mtime 前后 md5 一致（`b07b91b3…`）。产物 main `8758ca66…`（14,683,136B/00:18，+4,608B）/ ui `94f2a97a…` / crash `a4e2672d…`；两副本相等；三者异于 BUILD-341。
- **八项逐项 PASS**：①时间戳 00:13–00:18 ②sha ③0.9.2 ④冒烟 PID 28292 Responding=True/无新 crash.json/残 0 ⑤config+wordbook 本次窗口内零变化 ⑥warnings 99/90/17 ⑦四表三副本全等 ⑧探针（自检 `feiyin`=19；正向 `[LocalRT-DBG-342]`=3/`337`=4/`336`=1/`325`=1/`AUTOLEARN`=4/`degree_adverbs`=2/`nz_ratio`=2；反向 5 符号全 0）。
- 🔴 **发现 1（待主控裁）**：`target/release/crash.json`（mtime **00:00:34**，早于本 build）报 `Panic: byte index 235 is not a char boundary; it is inside '斯'`（对含中文串按字节下标切片，栈顶 `core::str::slice_error_fail`）。**`fbec727` 未显式修**（`git show fbec727 | grep raw_len` 空）。候选根因：`local_stream.rs:315 raw_full[cache.raw_len..]`（只判长度未判字符边界；`raw_full` 在 shadow/main 间切换时 `raw_len` 可落中文字符中间）。已备份 `testexec342/crash-20260922-0000.json`；**未改生产代码**。
- **发现 2**：`target/release/wordbook.sqlite` 341→342 间被 app 写（`72dc9738…`→`9477887b…`）；本次窗口内三时点恒定，非本次构建所致。
- **量化**：冒烟 idle CPU 5s = **0.04%**（31.2ms/5020ms/16核）、WS 1791MB（🔴 idle ≠ 录音期，测不到 D 真实开销）；历史改前（fbec727 前）47 次非尾派发 **最小 silence=800ms（37×800 + 其余各 1，无一 <800）**；`join: total_decode/tail_wait` 历史与本机日志均未捕到 ⇒ **如实报无数据**。
- **Gavin 端测五条**：① 🔴 D：一口气连说 15s+，`[LocalRT-DBG-298] seg dispatch` 应出现 **silence<800ms** 的派发（改前一次都没有）② 🔴 尾字：说完停住不松手，`[LocalRT-DBG-325/337] reflow applied` 且 action 非 `skipped-cancel` ③ 句尾幻字 ④ 接缝重复（「也可以。」孤立片段）⑤ 其余照旧。
- **证据**：`collab/outbox/tester-1/testexec342/`（含 `crash-20260922-0000.json`）。
- **红线**：版本 0.9.2 未动 / 未改生产代码 / 未 push / 零凭证。

## LOCALRT-344（coder-2，2026-09-22）

- **背景**：BUILD-342 端测 `crash.json` 报 `byte index 235 is not a char boundary`；Gavin 要求主路径不得加拖累机制。
- **改动（仅 `src/transcription/local_stream.rs`）**：①P0 复核采纳 `is_char_boundary`（O(1)），抽 `punct_cache_reuse` + 2 条回归单测；②回滚 340 shadow 补静音 / 340 收尾补静音 / 307 整句全量重解码，删 `feed_tail_silence`+3 调用+2 常量（无死代码），`endpoint_confirm_text` 三方→两方；③护栏：tailpad340 顺序红线随机制移除（原位注释），G3 计数 2→1 同步断言。
- **字节切片清单**：全文件仅 `&raw[raw_len..]` 一处 `&str` 切片（已修），其余 `Vec<f32>`/`Vec<String>` 切片 N/A。
- **验证**：fmt clean / check 0 error / warnings **99/90**（未下降，如实报）/ numstat==-w / 全量 `cargo test` 0 failed（1328+52+…）。
- **红线**：未动版本号 / 未出包 / 未 commit / 零凭证。
- **追加 G（344-G）**：修复「回灌洞永久失效」—— `acc_text` 累积、失败/空片留洞 ⇒ `has_hole` 永不复位 ⇒ 一片失败后续回灌全废。`on_segment` 载荷加 `seg_streaming_text`（`last_display` 自上一片 `committed_len` 起的字符后缀，按 char 切片）；worker 对 `Err` 与「Ok 空」统一 `hole_fill_decision`：非空填空不记洞，真·无法填补才兜底 `has_hole`；`SkippedHole` 保留。单测 3 条。**顺带查**：短片空输出 = 该段音频本身无语音（shadow/streaming/accuracy 三方同时为空），非 padding/静音判据。改动 `src/transcription/local_stream.rs` + `src/main.rs`；全量 `cargo test` 0 failed（1331）。

## 2026-09-22 — coder-2 — ACC-DISPATCH-SILENCE-ONLY-346 ✅ 交付（派发规则只判静默 1200ms + 修共享计数器坑）

- **Gavin 原话**：「改成只判断静默1200ms，不按时长来切片」。
- **改动（仅 `src/transcription/local_stream.rs`，`numstat`==`-w` 206/133）**：①阈值 800→**1200ms**；②删长度支（原 800ms OR 累计 5s）—— 按 DEC-077 连根删 `ACC_MIN_SEGMENT_MS_DEFAULT` / `AccDispatchConfig.min_seg_ms` / `should_dispatch_acc` 的 `min_seg_ms` 参数与分支，无死代码；③🔴 **核心坑**：`silent_ms` 是唯一共享计数器，标点路径 800ms 打点后清零它 ⇒ 阈值抬到 1200 后静默支**结构上不可达**。修法 =**方案 A**：新增 acc 专用 `acc_silent_ms`，只在静音累加/语音归零两处同步，**不被标点清零**；调用点改读它。shadow(400)/标点(800) 行为逐位不变。未选 B（删清零会连带动 shadow 时序）。④订正 5 处过期注释（含 `:970` 错误的「本处独立计数」）。
- **核心判据（测试）**：`acc346_acc_counter_survives_punct_reset` —— 逐 chunk 模拟「标点 800ms 清零」后，**正证** acc 计数器仍能走到 1200 并派发；**反证** 误用 `silent_ms` 则不可达。另 `guard346_acc_counter_wiring` 源码级护栏钉死接线。
- **验证**：`cargo fmt --check`（不带 `skip_children`）EXIT 0 ｜ `cargo check` 0 error ｜ warnings **99** = 基线 ｜ `numstat`==`-w` ｜ 全量 `cargo test --no-fail-fast` **1420P/0F/22I**（NEW 3 / GONE 2）。
- **只报不改**：`src/main.rs:9570` 注释「要攒够 `min_seg_ms` 且静默 800ms 才派片」已过期（越界项，报主控）。
- 🔴 实机 `tail_wait` / 派发分布交 tester-1/Gavin，未声称已验证。未动版本号（0.9.3 主控已升）/ 未 commit / 未出包 / 未碰 `src/main.rs`、`src/audio/**` / 零凭证。

## 2026-09-22 — coder-2 — PUNCT-PREVIEW-SEMANTIC-349 ✅ 交付（预览标点打在句中 · 只认静默 1200ms）

- **Gavin 原话**：「流式预览...标点打在这个句子的中间，掐断了整个句意」；诉求「按语义停顿打标点」。
- **先证后改（真实引擎探针 + 实机日志）**：① 真实 CT-Transformer 对任意截断的半句**恒在末尾补「。」**（8~32 字七档全中）；② `punct_cache_reuse` 把该终止符作前缀保留、新字追加其后 ⇒ **落到句中**；③ `debug-build321.log` 单会话 53 次重打点，大量落在 10~34 字前缀（4s/800ms/shadow 三条与语义无关）。证据 `collab/evidence/20260922-punct349/`。
- **改动（仅 `local_stream.rs`，`numstat`==`-w` 92/49）**：`PUNCT_SILENCE_TRIGGER_MS` 800→**1200**；删 `PUNCT_REFRESH_INTERVAL`(4s 定时) + `preview_display` 的 `interval/due`；删 `shadow_fired` 对 force 的贡献；新判据纯函数 `should_repunctuate_preview`；删 `PunctPreviewCache.last_punct_at`（`has_new` 改长度比较）；收尾强制打点保留。**shadow/acc/endpoint 状态机零改动**。
- **验证**：`cargo fmt --check`（不带 `skip_children`）EXIT 0 / check 0 error / `local_stream.rs` 零 warning / 全量 `cargo test` 0 failed（feiyin-ime 1337P，NEW 2；探针已删）。
- 🔴 **归属**：全量树含 coder-1 在飞 350 未提交改动，总数 1420→1425（本单 2 + 350 3），**本单隔离 +2**。
- **残余**：1200ms 停顿处若其实没说完仍会出现「。」（语义边界固有代价，符合 Gavin 口径）。
- 🔴 实机交 tester-1/Gavin，未声称已验证。未动版本号 / 未 commit / 未出包 / 未碰 `src/main.rs`、`src/punctuation/mod.rs` / 零凭证。

## 2026-09-22 — coder-2 — TEST-SYNC-352 ✅ 交付（阶段三：给 350 补独立护栏，非作者）

- **性质**：阶段三，**只写测试、零生产代码改动**；命令白名单（DEC-048）仅 `cargo fmt` / `cargo check`，🔴 **未跑 `cargo test`**（首跑在阶段四 tester-1）。
- **产出 6 条（`sync352_` 前缀）**：`punctuation/mod.rs` +131 = ① `has_` 重构等价性（旧语义逐字重写进测试 + 41,371 串穷举比对 + 「strip 恒等 ⟺ has false」不变量）② 退化输入（对偶/幂等/恒等）③ UTF-8 多字节紧邻标点（防 344 P0 同族）；`main.rs` +102 = ④ 节点唯一挂载 + 门含 `!start.translate` ⑤ 剥离早于 `pretranscribed_native_punctuated`（源码级顺序）⑥ 门关逐字返回（同覆盖 translate 分支）。
- **坑（已规避）**：`let stripped = strip_punctuation_node(` 在作者测试里也出现 ⇒ 不能数全文件，改用生产独有首参 `normalized` 过滤；护栏字面量一律 `concat!` 拆开防自命中。
- **验证**：`cargo fmt --check` EXIT 0 ｜ `cargo check --all-targets` 0 error、warnings **99/90** = 基线 ｜ `numstat`==`-w`（102/0、131/0）。
- **静态复核**（保首跑通过）：挂载过滤后恰 1；剥离 `:8433` < native `:8442`，距 785B < 1500；门窗口含两 switch。
- **结论**：**未发现 350 真实缺陷**。未 commit / 未 push / 版本号未动 / 零凭证。

## 2026-09-22 — coder-2 — ITN-IDIOM-DUFAMILY-354 ✅ 交付（「度」字固定语保护，先证后改）

- **由来**：Gavin 实测 `梅开二度→梅开2度`（`度∈[units.temperature]`，单位规则不认成语边界）。
- **先证后改**：临时探针逐条取证后把问题重定义为「**度=量词/维度 vs 度=单位**的系统性歧义」，比原假设准确。full 表见 logs/result。
- **主控裁定后只做 A**：8 条不可推导固定语入 `[protect.idioms]`（DEC-044）：`梅开二度 / 六度万行 / 八度空间 / 八度音程 / 八度音阶 / 八度音 / 高八度 / 低八度`。
- **剔除并入 B 单 `ITN-DU-AMBIGUITY-356`**（DEC-038）：`增四度/减五度`（音程能产族）、`N度空间` 全族（含订正 `三度/四度空间` 不一致）、裸 `一度/二度/三度/八度` 量词义（**核心，`他一度以为` 高频**）、`N度出山`、医学 `N度烧伤`。先例：`itn.rs:2808-2814` 裸「十分」带右邻条件。
- **遮蔽自检**（[ITN-PREFIX-SHADOW-001]）：`八度音`(3) 是 `八度音程/八度音阶`(4) 前缀，`.max()` 最长匹配 ⇒ 三条互不吃（已测）；其余保护后主循环从后续继续，不遮蔽。逐条答案在 result.md。
- **改动**：`src/itn.rs` +69（仅 `#[cfg(test)]` 3 用例）/ `itn-rules.toml` +7（生产算法零改动）。
- **验证**：`cargo fmt --check` EXIT 0 ｜ `cargo check` 0 error、warnings **99/90**=基线 ｜ `numstat`==`-w`（69/0、7/0）｜ 全量 `cargo test` **0 failed**、`itn::` **262P/0F**（NEW 3）。边界外同批钉死。临时探针已删干净。
- 🔴 `itn-rules.toml` 三副本：root 已改，`Publish/` + `target/release/` **待 tester-1 出包同步**。
- **B 单未碰**。未动版本号 / 未 commit / 未 push / 未出包 / 零凭证。

## 2026-09-22 — coder-2 — ITN-DU-AMBIGUITY-356 ✅ 交付（单字数字+「度」义项消歧，354 的 B 单核心）

- **核心缺陷**：`他一度以为 → 他1度以为`（量词/副词义「度」被温度单位规则误转）。
- **先证后改 + 方案经主控批准（Option 1）**。规则：单字数字（**一..九 + 两**）+「度」**默认保护**，
  仅「度」后 ∈ **END（串末/空白/标点）** 或测量续接词 **{电,角,左右,以上,以下,多,有余}** 才转；多位数不受影响。
- **已证**：A 侧（一度以为/中断/夺冠…）全保护；B 侧（一度电/相差一度/零下一度/三十度/九十度角/一度水…）保持现状。
- **条件 3 收口**：移除 13 条被规则覆盖的冗余词条（含 354 的 六度万行/八度空间/八度音…），
  `N度空间` 族**一致保护**；**保留** `五度五关`（规则只护「五度」、后半「五关」被 `unit_preceded` 转 ⇒ 覆盖不到）。
- **条件 2 已知代价（显式断言）**：单字+度+名词（`一度水/二度低温/五度低温`）保护（不转）。
- **音程族建议不做**（能产族 + 频次极低）。
- **验证**：`cargo fmt --check` EXIT 0 ｜ `cargo check` 0 error、warnings **98≤99** ｜ `numstat`==`-w`（246/0、19/8）｜ 全量 `cargo test` **0 failed**、`itn::` **267P/0F**（+5）。临时探针已删。
- 🔴 `itn-rules.toml` 三副本：root 已改，`Publish/` + `target/release/` **待 tester-1 出包同步**。
- 未动版本号 / 未 commit / 未 push / 未出包（DEC-079）/ 未碰甲/乙型 / 零凭证。

## 2026-09-22 — coder-1 — FIX-ORDERED-REFLOW-DROP-368（🔴 P0 吃字）✅ 交付

- **缺陷**：`OrderedReflow::push` 对齐失败时 `committed` 不推进但 `last_window_text` 被无条件覆盖 ⇒ 本该定稿的滑出片永久丢失（吃字）。
- **修法**：失败 ⇒ committed 不动 + last_window_text 不动 + 不产出回灌 + next 仍 +=1；连续失败 ≥ `REFLOW_FALLBACK_FAILS(3)` ⇒ 兜底旧 last 整体并入 committed（不去重）再接新窗；取向「宁可重复不可丢字」写入注释。
- **范围**：仅 `OrderedReflow`（+常量）；未动 `align_overlap`/`group_window_start_secs`；未碰 `itn.rs`。
- **单测**：4 条新增全绿（中途失败不丢字 / 失败不产出 / 连续失败兜底 / next 推进）；3 条旧乱序测试改用长文本同步。
- **验证**：`cargo fmt --check` EXIT 0；全量 cargo test 0 failed（1379P/28I）；`cargo build --release` EXIT 0；warnings 98/89 基线；numstat（transcription 115/18 vs -w 111/14，差 4 行 whitespace-only 落改动区块内）。
- **未改版本 / 未 push / 零凭证**。🔴 BUILD-347 含此缺陷，须重出包。
## 2026-09-22 — coder-1 — FIX-WINDOW-DISJOINT-369 🔴 P0 ✅ 交付

- **缺陷**：12s 封顶把窗口切到「与上一窗零重叠」（单片 9.34s 顶满）⇒ 对齐必败；368 的「失败 ⇒ 整窗跳过」
  在必败场景下变成**整窗内容全丢**（Gavin BUILD-348：6 片 31s ⇒ 最终 50 字，后半段消失）。
- **修法**：`OrderedReflow` 改用**切片区间**判重叠（`push(seq, start_slice, end_slice, text)`）——
  ①零重叠 ⇒ **直接拼接**（无重复可去，绝不跳过）；②有重叠 + 对齐成功 ⇒ 去重；
  ③有重叠 + 对齐失败（长度门误拒）⇒ **退回拼接**（宁可重复不可丢字）。
  `main.rs` 传窗口全局区间 `[total_slices - recent_slices.len() + start, total_slices)`。
- **范围**：`src/transcription/mod.rs`（OrderedReflow + 单测）、`src/main.rs`（切片区间追踪）；
  **未动** `align_overlap`/`group_window_start_secs`/长度门常量；移除死状态 `fail_streak`/`REFLOW_FALLBACK_FAILS`；未碰 `itn.rs`。
- **单测**：新增 4（真实日志序列 6 片全在 / 零重叠拼接 / 有重叠去重 / 有重叠失败退回拼接）；删 1（368 阈值兜底，已被③取代）；改 6（签名 + 语义）。
- **反证**：临时改回 368「跳过整窗」⇒ 真实序列测试 FAILED（片5 整窗丢失，复现 Gavin 现象）。
- **验证**：`cargo fmt --check` EXIT 0；全量 cargo test 0 failed（1382P/28I）；`cargo build --release` EXIT 0；warnings 98/88；
  numstat main 32/2、mod 212/82（-w 205/75，7 行 whitespace-only 落改动区块内）。
- **未改版本 / 未 push / 零凭证**。🔴 **BUILD-348 含此缺陷，须重出包。**
## 2026-09-22 — coder-1 — FIX-REMOVE-HARDSPLIT-370 ✅ 交付（含 300s 上限同批）

- **缺陷**：滑窗派发路径经 `build_padded_segments` 按 `SEGMENT_MAX_SECS`(20s) **硬切** —— 不看语义、
  与「只按 1200ms 静默切片」原则冲突，且硬切出的非停顿子段加重 369 的零重叠。
- **修法**：`vad::build_padded_segments_capped(.., max_seg_secs)`（显式上限，DEC-066）+ `build_padded_segments` 变薄包装；
  滑窗专用 `local_stream::build_dispatch_segment` 上限 `SLIDING_SLICE_MAX_SECS`=**13s**（极端长句兜底，超出继续切不丢弃）。
  200ms padding 与 FIX-VAD-STATE-RESET-001 越界护栏**保留**；`main.rs` 派发点保留 `path_b_budget_ok` 撞顶 warn。
- **13s 依据（Gavin 实测体验，非估算）**：对齐滑窗封顶 `WINDOW_MAX_SECS`=12s 的同一水平 —— 端测确认
  「12s 窗口解码+回灌刷新预览」无明显卡顿；13s 解码 ≈5.3s vs 12s 的 4.9s（+0.4s）⇒ 触发时体验无差别。
  被否：16s(6.6s, Gavin「还是有点长」)/30s(12.3s)/90s(36.9s)。KV 与内存均非瓶颈（13s≈169 tok）。
- **同批**：`MAX_RECORD_SECONDS` 180 → **300**（5 分钟）——滑窗后与 KV 解耦，代价仅 18MB 缓冲 + 松手只解 ≤13s 窗；
  连带 `src-tauri/src/config.rs` 镜像常量、`hotkey` 断言 185→305。
- **单测**：新增 8（vad 5 + local_stream 3，含「旧包装 ≡ capped(SEGMENT_MAX_SECS)」逐位证明）；改名 2。
- **验证**：fmt EXIT 0；全量 cargo test 0 failed（**1390P/28I**）；release EXIT 0；src-tauri check EXIT 0；warnings 98/88。
- 未改版本/未 push/零凭证。369 同批未提交。

## FIX-PREFIX-AND-EAT-371（coder-1，2026-09-22）— P0 两缺陷：前缀漏出 + 重复句吃字

- **触发**：Gavin 端测 BUILD-349，输出 `language chinese<asr_text>明天…吗？明天…吗？来看电影吧。明天…吗？明天…吗`。
- **A（前缀）**：355 定「无条件截断到第一个 `<asr_text>`（含）」，359 私加两闸（标签样 / 紧贴标签）。
  355 观测时 `raw_decode` **无 hotwords**，生产每次注入 ctx+词库 ⇒ 前缀形态变 ⇒ 两闸失效。
  修：删 `is_qwen3_language_label` + 两闸；唯一护栏 `QWEN3_PREFIX_MAX_BYTES=64`；新增 `[LocalRT-DBG-371]` 打被剥原文。
- **B（吃字）**：`align_overlap` 从 `max_k` 往短找 ⇒ 周期性内容下 `S1S2S3` ≡ `S2S3S4` ⇒ 切点归零 ⇒ 整窗丢。
  修：四层判据 ①硬约束（`prev_extra_slices≥1 ⇒ k<prev有效字数`，精确、最先）②软范围（期望 k ± **不对称**
  `DOWN=0.50/UP=0.25`）③质量门未改 ④兜底升序小 k。接线 `push_window` + `AlignPrior`；`main.rs` 加 `window_samples`。
- **不对称容差的理由**（写进 `ALIGN_EXPECTED_K_TOL_DOWN` 注释）：层①只保证 `committed_prefix` **非空**、
  **不保证不吃半句** ⇒ 向小=重复（可容忍）、向大=吃字（P0）⇒「宁宽勿窄」**只适用于安全方向**。
- **单测 +8**，含两条反证：旧算法内联证必命中 `max_k`；去层①同输入即空提交。
- **主控验收**：22 条清单（`collab/acceptance-371.md`）逐条 Read 代码，首轮 20 过 → B3 改判 + C3 补 → 复验全过。
- **验证**：fmt EXIT 0 / check 0 error（主控复跑）/ 全量 test 0 failed（**1400P/28I**）/ warnings 98/88 = 基线。
- `docs/MACOS-HANDOFF.md` 新增 0.3 节。未改版本 / 未 push / 未出包（BUILD-349 已作废）。
