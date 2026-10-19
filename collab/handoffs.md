# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-29 归档：2026-09-26 / 09-27 共 20 条已移入 `handoffs-archive.md`（本文件曾达 209 行，超 200 行上限）。
> 2026-09-26 归档：2026-09-25 共 52 条已移入 `handoffs-archive.md`（本文件曾达 465 行，超 200 行上限）。
> 2026-09-25 归档：2026-09-24 共 22 条已移入 `handoffs-archive.md`（本文件曾达 303 行，超 200 行上限）。
> 2026-09-24 归档：2026-09-23 共 35 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-23 归档：2026-09-22 共 31 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

## 2026-10-04 — 主控 — BUILD-473 验收通过

- tester-1：回归全绿（bin 1925P/0F/82I）、九项全 PASS，main `4e3357aa…`，0.9.4。
- 待 Gavin 重启后目视：预览字号 / 清晰度（473）、本地实时开录直接「请说话...」与加载中不弹录音窗（472）、加载提示完整（471）。
- 主控更正：任务书 `fix164*` 测试名前缀写错（0 命中），下次按实际测试名列。

## 2026-10-04 — tester-1 — TEST-EXEC-474 + BUILD-474 ✅（回归全绿 ⇒ 出包）

- **性质**：阶段四全量回归 + 阶段五出包（UI-FONT-474，DEC-106）。源码基线 `5220a2b`（构建期间主控 CLEAN-468 提交 `3e0c265` 仅动 `collab/todo.md`，无 `src/`、`ui/src/`、`src-tauri/src/` 变更）。版本 **0.9.4 未动**。
- **回归（全绿）**：bin **1924P/0F/82I**（＝预期）/ root 2012P/0F/84I / `src-tauri` 92P/0F/0I / Vitest 7 files·100P·11skip / `cargo fmt --check` EXIT 0。NEW 通过 1（`ui474_preview_text_drawn_like_edit_box`）+ 忽略 1（`ui474_preview_text_real_dc`）；GONE 通过 2（`omc415_gdi_and_dwrite_are_independent`、`omc415_dwrite_put_hit_and_invalidate`）+ 忽略 1（`ui473_d2d_streaming_text_real_dc`）；恒等式逐位吻合。重点失效模式（`ui47*`/`overlay*`/`d2d*`/`omc415*`/`scroll*`/gdi-fallback/`g3_alpha_fixup*`）全绿、0 FAILED。
- **专项**：`ui474_preview_text_real_dc`（--ignored）1P/0F，`[474] GDI 宽 190px；ClearType 彩边像素 1097；分界 x=156 左侧差异像素 0；尾段亮度 正常 653956 / 浅色 534400`。
- **出包九项全 PASS**：①main 12:28:17 / ui 12:28:26 / crash 23:57:24 ②两副本全等，main `c7bfb2f7…`/ui `727b470d…` 异于 473，crash `9340ab31…` 同（本批零 crash 改动，属预期）③0.9.4.0 / 0.9.4 / 0.9.4.0 ④Publish PID 18836 + target/release PID 16300 均 Responding、无 panic、无 crash.json、零残留 ⑤`Publish/config.toml` `da2be5da…` 前后不变 ⑥90/9 ⑦正 `LAS_ENC_WARMUP`1 / `[ALIGN-456] forced aligner loaded`1 / `[STREAM-SV-463] preview model`1，反 `[LocalRT-DBG-277] gdi_width`0 / `[PHANTOM-459] stripped silence-born text`0 / `streaming-paraformer-trilingual`0 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **模型（如实）**：本批无新增 / 无改动；`Publish/models/sherpa-onnx-sense-voice-…/test_wavs/` 已删未回填（`find … -name test_wavs` = 0）；1.7B / aligner 照常刷新；其余目录未删。
- **红线**：未改生产代码 / 版本未动 / 未 push / 未删用户数据与模型文件 / 零凭证。报告 `collab/outbox/tester-1/result.md`。**Step1 已结束输入法进程 ⇒ 请主控转告 Gavin 重启。**

## 2026-10-04 — 主控 — UI-FONT-474 开发完成（DEC-106），派 BUILD-474

- 预览文字改 GDI（= 编辑框字体与渲染），窗框仍 D2D；浅色尾巴两段裁剪；删 D2D 预览文字与 DWrite 量宽缓存。
- 预期 bin 1924P/0F/82I（NEW 1 + 1I；GONE 2 + 1I）。

## 2026-10-04 — 主控 — BUILD-474 验收通过（主控 collect）

- 复核 tester-1 报告：bin 1924P/0F/82I、九项全 PASS，main `c7bfb2f7…`，0.9.4；验收通过，已请 Gavin 重启目视 473 / 474。
- 观察项：设置界面 release 构建 warnings 17 → 18（本批未改 src-tauri），STATS-475 改 src-tauri 时一并复核。
- CLEAN-468 盘点完成（`collab/evidence/468/audit.md`），待 Gavin 定 A / B 组。

## 2026-10-04 — 主控 — STATS-475 开发完成（0.9.5，DEC-107），派 BUILD-475

- 统计模块 `src/usage_stats.rs` + 设置界面「我的」页；版本 0.9.5（6 处）。
- 预期：bin 1927P/0F/82I（+3）、src-tauri 95P（+3，同一模块测试）、Vitest 105P（+5）。
- Gavin 追加：分项为 0 不显示（已改，派单前暂停 tester-1 重派）。

## 2026-10-04 — 主控 — BUILD-475 重派（并入 476 / 477 + 475 色值修复）

- tester-1 首轮回归拦下 `Mine.tsx` 裸色值（G1），已改设计令牌；主控自测改为跑全量 Vitest（105P/11skip）。
- Gavin 追加 476（「请说话...」同预览样式）、477（三处字号小一号 18 → 17），两度暂停 tester-1 后一并重派。
- 预期：bin 1929P/0F/82I（stats475 3 + ui476 1 + ui477 1）、src-tauri 95P、Vitest 105P/11skip。

## 2026-10-04 — tester-1 — TEST-EXEC-475 + BUILD-475 ✅（回归全绿 ⇒ 出包 0.9.5）

- **性质**：阶段四全量回归 + 阶段五出包（STATS-475 + UI-FONT-476/477 + 色值修复）。HEAD `89e4a09`；版本 **0.9.5**。中间两次被主控紧急暂停（`260d7e8`/`891c027` 结果作废），报告以最终 HEAD 为准。
- **回归（全绿）**：bin **1929P/0F/82I**（＝预期）/ root 2017P/0F/84I / `src-tauri` 95P/0F/0I / Vitest 8 files·105P·11skip / `cargo fmt --check` EXIT 0。NEW 5（`stats475_words_cjk_by_char_english_by_word`、`stats475_week_is_monday_to_sunday`、`stats475_week_usage_sums_by_category_and_window`、`ui476_listening_placeholder_drawn_like_preview`、`ui477_overlay_text_one_size_smaller`）/ GONE 0；重点失效模式（含 `design-tokens`）全绿、0 FAILED。
- **专项**：`ui474_preview_text_real_dc`（--ignored）1P/0F，`[474] GDI 宽 170px；ClearType 彩边 979；分界 x=144 左侧差异 0`；`[476] 「请说话...」ClearType 彩边 364`。
- **出包九项全 PASS**：①main 12:59:34 / ui 12:59:43 / crash 12:58:01 ②两副本全等，三产物均异于 474（main `28803b92…`/ui `fc958071…`/crash `2ee6423f…`）③0.9.5.0 / 0.9.5 / 0.9.5.0 ④Publish + target/release(本地实时) + 设置界面 三冒烟均 Responding、无 panic、无 crash.json、零残留 ⑤`Publish/config.toml` `da2be5da…` 不变 ⑥main 90 / crash 9 / UI 17 ⑦正 main `usage-stats.sqlite`1 / `STATS-475: usage record failed`1 / `LAS_ENC_WARMUP`1，正 ui `get_usage_week`1，反 `[LocalRT-DBG-277] gdi_width`0 / `streaming-paraformer-trilingual`0 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **D1**：三处冒烟后 `Publish/`、`target/release/` **均无** `usage-stats.sqlite`（启动不建，符合预期）。
- **如实偏差**：`target/release/config.toml` 12:48 被外部改为在线档；tester 临时切 `local_realtime` 完成本地实时冒烟后字节级还原（sha `d86e8651…` 一致，备份已删）。设置界面 standalone 启动即 `exit(1)`（`src-tauri/src/main.rs:194` 要求主进程在跑），故先起主进程再拉 UI 验证。
- **待主控知悉**：`Publish/voice-ime.iss` 仍 `0.9.4`（guide 无 `.iss` 同步步骤，未动）；`Publish/version_check.json` 由程序冒烟自写为 `0.9.5`。
- **红线**：未改生产代码 / 版本未动（主控改）/ 未 push / 未删用户数据与模型文件 / 零凭证。报告 `collab/outbox/tester-1/result.md`。**Step1 已结束输入法进程 ⇒ 请主控转告 Gavin 重启。**

## 2026-10-04 — 主控 — BUILD-475（0.9.5）验收通过

- 复核 tester-1：bin 1929P/0F/82I、src-tauri 95P、Vitest 105P/11skip、九项全 PASS；main `28803b92…` / ui `fc958071…` / crash `2ee6423f…`，0.9.5；启动不生成 `usage-stats.sqlite` ✅。
- 待定点①：`Publish/voice-ime.iss` 主控已按 `installer/voice-ime.iss` 同步为 0.9.5（两文件除版本号外逐字相同；旧版备份在会话 scratchpad）。`Publish/version_check.json` 是设置界面运行时写的检查缓存（冒烟后已自动为 0.9.5），无需处理。
- 待定点②：`target/release/config.toml` 12:48 被切到在线档系 Gavin 端操作；tester 冒烟临时切本地实时后已字节级还原，正确做法。
- 已请 Gavin 重启并目视：「请说话...」/ 预览 / 编辑态三处字体一致与字号、「我的」页。

## 2026-10-04 — 主控 — PUNCT-478 + UI-MINE-479 + STATS-480 开发完成，派 BUILD-478

- 478 根因与方案见 DEC-108 / `collab/evidence/478/root-cause.md`；Gavin 确认「可行、语义断句不关」。
- 预期：bin 1936P/0F/83I（p478 +7、p480 +1I）、src-tauri 95P、Vitest 105P/11skip。

## 2026-10-04 — tester-1 — TEST-EXEC-478 + BUILD-478 ✅（回归全绿 ⇒ 出包 0.9.5）

- **性质**：阶段四全量回归 + 阶段五出包（PUNCT-478/DEC-108 + STATS-480 + UI-MINE-479/481）。HEAD `a34215d`；版本 **0.9.5 不变**。（主控误推的旧 TEST-EXEC-475 已作废。）
- **回归（全绿）**：bin **1936P/0F/83I**（＝预期）/ root 2024P/0F/85I / `src-tauri` 95P/0F/0I / Vitest 8 files·105P·11skip / `cargo fmt --check` EXIT 0。NEW 7（`p478_seam_punct_next_window_decides_when_it_covers_seam`、`p478_seam_punct_uncovered_keeps_old_dedupe`、`p478_timed_stitch_takes_next_window_comma_at_pause_seam`、`p478_judge_seam_reads_mark_at_seam_position`、`p478_sentence_seams_mark_server_sentence_ends`、`p478_online_seam_period_replaced_by_local_judgement`、`p478_online_seam_real_sentence_end_and_english_space`）+ NEW ignored 1（`p480_effective_speech_excludes_silence`）/ GONE 0；重点失效模式全绿、0 FAILED。
- **专项**：`p480`（--ignored）1P，`[480] 整段 56149ms → 有效语音 51584ms（92%）`；`sv463_preview_freeze_e2e`（--ignored）1P，`[463] 语音 56.1s 处理 9.1s ｜ 刷新 94 ｜ 派发 6`。
- **出包九项全 PASS**：①main 14:24:33 / ui 14:21:57 / crash 12:58:01（未重编）②两副本全等，main `76487e28…`/ui `8b0c02d8…` 异于 475，crash `2ee6423f…` 同属预期 ③0.9.5.0 / 0.9.5 / 0.9.5.0 ④Publish + target/release(本地实时) + 设置界面 三冒烟均 Responding、无 panic、无 crash.json、零残留 ⑤`Publish/config.toml` `da2be5da…` 不变 ⑥main 90 / crash 9 / UI 17 ⑦正/反探针全对（正 main `PUNCT-478: online seam`1/`usage-stats.sqlite`1/`STATS-475: usage record failed`1、正 ui `get_usage_week`1）⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **D1（如实）**：主程序启动（`-debug` 12s / 单独 30s）与 target/release 本地实时档启动**均不生成** `usage-stats.sqlite`；既有 `target/release/usage-stats.sqlite`（13:49）mtime 未被启动改写。⚠️ Settings UI 冒烟窗口内曾两次观察到 `Publish/usage-stats.sqlite` 空库出现（14:27:52 / 14:31:02），后续隔离复跑（Publish 45s / 临时目录 25s / CDP 活动页=通用）**均未复现**；代码层 `usage_stats::open()` 任何调用都建库、读路径 `week_usage()`（← `get_usage_week` ← `Mine.tsx`）故打开「我的」页会落空库，提请主控决定是否将读路径改只读。核心判据 PASS。
- **红线**：未改生产代码 / 版本未动 / 未 push / 未删用户数据与模型文件 / 零凭证。报告 `collab/outbox/tester-1/result.md`。**Step1 已结束输入法进程 ⇒ 请主控转告 Gavin 重启。**

## 2026-10-04 — 主控 — BUILD-478 验收通过

- 复核：bin 1936P/0F/83I、root 2024P、src-tauri 95P、Vitest 105P/11skip、九项全 PASS；main `76487e28…` / ui `8b0c02d8…`，0.9.5；设置界面 warnings 17（474 的 18 为偶发，已回基线）。
- D1 主控拍板：`usage_stats` 读路径改为「库不存在 ⇒ 返回全 0、不建库」（读不产生副作用，与「首次记录才建库」一致），随 CLEAN-468 第二批一并改、下次出包回归。
- 已请 Gavin 重启并端测：本地实时 / 在线各说一句带停顿的话看接缝标点；「我的」页分组框。

## 2026-10-04 — 主控 — CLEAN-468 源码清理 + D1 + UI-MINE-482，派 BUILD-482

- 源码：删 4 个 `src/bin/poc_*.rs`、12 个失效 #[ignore] 测试（bin 忽略 83 → 71）；`usage_stats::week_usage` 库不存在不建库；「我的」页统一组件样式。
- 预期：bin 1936P/0F/71I、src-tauri 95P、Vitest 106P/11skip。

## 2026-10-04 — tester-1 — TEST-EXEC-482 + BUILD-482 ✅（回归全绿 ⇒ 出包 0.9.5）

- **性质**：阶段四全量回归 + 阶段五出包（CLEAN-468 + D1 + UI-MINE-482）。HEAD `9a5c0c2`；版本 **0.9.5 不变**。
- **回归（全绿）**：bin **1936P/0F/71I**（＝预期，通过数不变、ignored 83→71）/ root 2024P/0F/73I / `src-tauri` 95P / Vitest 8 files·106P·11skip（NEW 1 STATS-UI-006）/ `cargo fmt --check` EXIT 0。GONE 忽略 12 逐一 0 命中（`poc_speaker_404_*` ×4、`poc_17b_351_fitness`、`poc_353_itn_cer`、`poc_355_capability`、`poc_376_language_separator`、`poc_355_language_option`、`poc_355_lang_none_multilingual`、`poc_357_prompt`、`gate335_asr_ab_manual`）；已删 4 个 `src/bin/poc_*`，root log 中其测试目标 = 0；0 FAILED。
- **出包九项全 PASS**：①main 14:53:36 / ui 14:50:58 / crash 12:58:01（未重编）②两副本全等，main `ca84731d…`/ui `932916ed…` 异于 478，crash `2ee6423f…` 同属预期 ③0.9.5.0 / 0.9.5 / 0.9.5.0 ④Publish + target/release(本地实时) + 设置界面 三冒烟均 Responding、无 panic、无 crash.json、零残留 ⑤`Publish/config.toml` `da2be5da…` 不变 ⑥main 90 / crash 9 / UI 17 ⑦正/反探针全对 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **D1 修复已验证（PASS）**：`Publish/usage-stats.sqlite` 冒烟前不存在；打开设置界面后 t=12/18/24/30s **始终不存在**（读路径不再建库）。
- **临时切档**：`target/release/config.toml` 原 `performance`，临时切 `local_realtime` 完成本地实时冒烟后字节级还原（sha `d7f34b62…` 改前=还原后）。
- **红线**：未改生产代码 / 版本未动 / 未 push / 未删用户数据与模型文件 / 零凭证。报告 `collab/outbox/tester-1/result.md`。**Step1 已结束输入法进程 ⇒ 请主控转告 Gavin 重启。**

## 2026-10-04 — 主控 — BUILD-482 验收通过

- 复核：bin 1936P/0F/71I（忽略 83 → 71，12 个 GONE 全符）、root 2024P/0F/73I、src-tauri 95P、Vitest 106P/11skip、九项全 PASS；main `ca84731d…` / ui `932916ed…`，0.9.5；warnings 90 / 9 / 17。
- D1 实测：打开设置界面 30s 内不生成 `usage-stats.sqlite` ✅。tester 冒烟临时切档后已字节级还原（Gavin 当前配置为 performance 档）。
- 已请 Gavin 重启并目视「我的」页统一样式 + 端测接缝标点。

## 2026-10-14 — 主控 — 0.9.6 开发 + 回归 + BUILD-096 出包（Worker 额度用尽，主控包办）

- 内容：WB-ECHO-483（DEC-109）、UI-RENAME-484、SC-LLM-485 / SC-PUNCT-486（DEC-110）、I18N-ERR-485、VER-096。
- 回归：root 全量 0 失败（bin 1961P/0F/71I）、src-tauri 98P、Vitest 127P/11skip、tsc 干净、fmt --check 干净。
- 出包九项全过：main `91a2022c…` / ui `46be1b5d…` / crash-reporter `d63d9421…`，0.9.6；warnings 90 / 17 / 6。
- 待 Gavin：重启输入法 → 按 todo 0.9.6 表逐项验收（快捷方式对话框目视、右Ctrl+1/2 提示窗、只说词条端测）。本地已 commit，未 push。

## 2026-10-14 — 主控 — 0.9.6 追加 487~490 + BUILD-096b 出包

- 内容：输出优化改名（UI-RENAME-487）、快捷方式文字加粗（SC-BOLD-488）、「中文输出」快捷方式简繁切换 + 非字母数字键红字（SC-SCRIPT-489，DEC-110 附则一；Shift 支持 Gavin 撤回）、快捷方式文字与标签同字号中线对齐（SC-ALIGN-490）。
- 回归：bin 1967P/0F/71I、src-tauri 98P、Vitest 133P/11skip、真实浏览器 7P、tsc / fmt 干净。
- 出包九项全过：main `5ce6be19…` / ui `7f21b8ab…` / crash-reporter `1c40e3e1…`。待 Gavin 重启输入法后目视 + 端测 右Ctrl+3。本地已 commit，未 push。

## 2026-10-17 — 主控 — ASR-SERVER-491 S9 逐字时间戳验收通过（提交 `0f00283`，未出包）

- 内容：T1~T6 + 时间轴更正（虚拟时间轴 ⇒ 片映射换算）；子 Agent 做 `char_times.rs`（37 单测）与测试补充，主控做接线与验收。任务书 `collab/spec-491-S9.md`，验收清单 `collab/acceptance-491-S9.md`（逐条对照设计，含偏离留档）。
- 偏离：T3 不补对齐，改插值标 `estimated`（设计 §12.5）。远程行为变化：终稿多等约 0.25s，最后一道接缝按时间拼。本机路径逐位不变。
- 回归：首轮 2044 过 / 4 失败 ⇒ 1 个本批引起（恢复生产写法）、3 个 S1 遗留护栏漂移（改读法，断言不变）；复跑全量 **2048 过 / 0 失败 / 48 忽略**、网关 **26 过**、fmt 干净。未交 tester-1（S10、S11 完成后统一回归出包）。
- 新方向：Gavin 10-17 修正 S10（客户端参数只有 ITN / 自动补标点 / 翻译，其余纯文字节点服务端固定开；翻译引擎跟随服务端本机设置）。S10 以设计 §13.1 前修正块为准。本地已 commit，未 push。遗留：长文本时间搬运改 Myers 差分。


## 2026-10-17 — 主控 — ASR-SERVER-491 S10 服务端文字处理验收通过（未出包）

- 内容：按 Gavin 10-17 修正，终端只传 ITN / 自动补标点 / 翻译三项参数，其余纯文字节点服务端固定内置。任务书 `collab/spec-491-S10.md`，验收清单 `collab/acceptance-491-S10.md`（含偏离留档），设计 §13.4。
- 改动文件：`src/asr_service/text_stage.rs`（新增）、`src/asr_service/session.rs`、`src/asr_service/frame.rs`、`src/asr_service/mod.rs`、网关 `src/bin/feiyin-asr-gateway/ws.rs`、`src/transcription/feiyin_remote.rs`、`src/transcription/qwen_inference.rs`。
- 要点：翻译引擎跟随服务端本机设置（大模型优先，否则进程级 NLLB 线程）；`OpenParams` 三字段 serde 缺省互通；S8 客户端删 `postprocess` 改发三项阿里同名参数到 `parameters` 顶层；本机管线一行未改。
- 回归：全量 **2061 过 / 0 失败 / 48 忽略**、网关 **31 过**、fmt / `cargo check --all-targets` 干净。未交 tester-1（S11 完成后统一回归出包）。
- 遗留：网关参数解析（`ws.rs:568`–`:574`）与 Feiyin 分支合并无独立单测，已人工逐行核对，由 S11 Python 参考客户端冒烟覆盖。下一步 S11。

## 2026-10-17 — 主控 — ASR-SERVER-491 S11 协议文档 + Python 参考客户端验收通过（未出包）

- 内容：对外协议文档做成独立契约（Gavin 10-17：安卓 / iOS 端另起项目），并配纯标准库 Python 参考客户端。验收清单 `collab/acceptance-491-S11.md`，提交 `b1463de`。另含 S10 追加修正（预览 tentative 换算，`35e928a`，6 单测，网关 31 过）。
- 新增文件：`docs/ASR-PROTOCOL.md`、`tools/asr-client/feiyin_asr_client.py`、`tools/asr-client/test_feiyin_asr_client.py`、`tools/asr-client/.gitignore`。无 Rust 改动。
- 测试：`py -3 -m unittest discover -s tools/asr-client` 45 过 / 0 失败，含本地 TLS 1.3 假网关集成测试 7 个。
- 客户端注意点：音频发送线程与收事件线程共用一个 SSLSocket，**必须走同一把锁**，否则 Windows 偶发 `WinError 10035`。
- 遗留：真实服务端冒烟未跑：`py -3 tools/asr-client/feiyin_asr_client.py --code "<连接码>" --wav <16k 单声道 wav>`。
- 下一步：T，交 tester-1 做 TEST-SYNC → 全量回归 → 出包（五阶段串行，出包须主控明确下达）。待 Gavin：S6/S7/S8 界面目视、两台电脑实测。

## 2026-10-18 — 主控 — BUILD-491 出包核验通过（0.9.7）

- BUILD-491 出包（0.9.7，HEAD `aa2f2bc`）：tester-1 构建并同步 Publish 后额度触顶，主控接手核验；主控复跑告警时误设 `SHERPA_ONNX_LIB_DIR` 触发整体重编（见 [BUILD-ENV-SHERPA-491]），改由 Sonnet 子 Agent 按指南环境重编并复核。最终 sha（target = Publish）：feiyin-ime `5c3f22607ecf7bf1`、feiyin-ime-ui `4075e0f8aa99b1cb`（00:08 tester-1 构建，未重编）、crash-reporter `98a62541b643a4c5`、feiyin-asr-gateway `01c943b1b0e246e9`（3.8MB）。九项全 PASS：版本 0.9.7（Rust 三件显示 0.9.7.0）；告警 feiyin-ime 89 / crash 6（上次 0.9.6 为 90 / 6）；探针 main `[ASR-SERVER-491]` 3、`feiyin-asr-gateway` 1，ui `asr-service.json` 1，网关 `run-task` / `task-started` 各 1；网关 dumpbin 只依赖系统库 + VCRUNTIME140，无模型库；网关无参运行报 missing --core、退出码 2、不驻留；主程序 -debug 冒烟 Responding、无 panic、无 crash.json，已清理；config.toml 三时点不变；toml 四件三副本、VC++ 五件三处全等；iss 两份逐字相同。本机无 Inno Setup，未编安装包。结果 `collab/outbox/tester-1/result.md`。待 Gavin：重启输入法；S6/S7/S8 界面目视；两台电脑端测；Python 客户端对真服务冒烟。

## 2026-10-19 — 主控 — 10-19 优化批三项完成（未出包，等 Gavin 指令）

- LAN-ADDR-492（子 Agent 写、主控逐行验收）：`settings.rs` `pick_lan_ip` + Windows `GetAdaptersAddresses` / unix `getifaddrs`；本机实测 `192.168.1.10:7860`。⚠️ 11:25 已构建进 Publish（Gavin「先不要构建」到达前跑完），四件 sha：main `bd56617c` / ui `fff29ea9` / crash `497ce45b` / gw `5992d092`，未冒烟。
- PUNCT-REDO-493 / DEC-112：`B_STRIP_WIRED_367 = true`，门 = 开标点 && 非翻译；护栏 sync352 同步改断言。
- ITN-ORDINAL-494：`enum_point_run` / `enum_point_arabic`，4 单测。
- LOCALTEST-495：`localtest/`（.gitignore），`--features poc` 挂载；CT 语义对照测试与结果在内。
- 回答 Gavin：WB-ECHO-483 的拼音相似度检查只在「整句恰为词条」且已触发重解时才算，正常窗口不算；拼音比对本身微秒级。
- 测试：feiyin-ime 2082 过 / 0 失败 / 49 忽略；网关 37；src-tauri 119；fmt 0。
- ITN-ENUM-496：`itn-rules.toml` `[enumeration]` 规则族（前缀×量词×可选「个」），`itn.rs` `match_enumeration`；494 硬编码删除；`itn496_tests`。出包时 itn-rules.toml 三副本须同步。
- TAIL-WIN-497：`LONG_SILENCE_TAIL_MS` 1900 → 1500，ts407 两条边界测试同步。
- PIPE-REVIEW-498：`main.rs` 路B 词条读库挪进 `if PATH_B_WIRED_367`；其余节点复查结论见 CHANGELOG。
- 测试：feiyin-ime 2083 过 / 0 失败；fmt 0。未出包。
- UI-NOTICE-FONT-499：信息 / 错误提示窗文字改 GDI ClearType（streaming_font），统一入口 `draw_status_overlay`；D2D `draw_info_overlay` / `draw_error_overlay` 去掉 message 参数只画窗框圆点。2084 过 / 0 失败。待 Gavin 出包后目视。
- TRAY-ICON-500：托盘识别服务项图标（启动 = 播放三角 / 停止 = 停止方块），`menu_icons::service_icon_rgba` + `rasterize_coverage`；Windows `attach_menu_icons` 多一参，macOS `set_asr_item_icon`。预览 `collab/outbox/coder-1/icons/service-*.png`。2085 过。
- ICON-AA-501：`menu_icons::rasterize` alpha = hits/16×255（原 /4 溢出回绕）；四个菜单 / 编辑图标边缘平滑，形状不变。
- BUILD-502：10-19 优化批出包，九项全 PASS；feiyin-ime `639a287b8989b93e`，其余三件同 11:25 包。待 Gavin 重启输入法目视。
- TRAY-ASR-503：`supervisor::tray_item` 判据改为配置选了本地流式实时模型（`CONFIG` OnceLock 由 `spawn` 登记），删 `tray_menu_asr_unavailable`；2087 过。未出包。
- ASR-UI-HIDE-FAST-504：`ui/src/pages/Voice.tsx` performance 选项改 grandfathered；Vitest 134 过。
- BUILD-505：出包九项全 PASS（feiyin-ime `f606a11f44ebbda4` / ui `18ead272c637b3de` / crash `a4d3acd29169ebb4` / 网关同上一包），已 push。待 Gavin 重启输入法。
- ASR-DEFAULT-LRT-506 / DEC-113：新装默认 local_realtime（`NEW_INSTALL_ASR_MODEL` 两处 + `assets/default-config.toml`），设置页删 Ctrl+M；serde 缺字段回落仍 performance。全量测试过。
- BUILD-507：出包九项全 PASS（Sonnet 子 agent 核验），feiyin-ime `c7021e27055c5d1f`；未 push，等 Gavin。工作区另有 src-tauri 三文件纯格式化改动未提交（主控 cargo fmt 带出），待 Gavin 定。
