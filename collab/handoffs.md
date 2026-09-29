# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-26 归档：2026-09-25 共 52 条已移入 `handoffs-archive.md`（本文件曾达 465 行，超 200 行上限）。
> 2026-09-25 归档：2026-09-24 共 22 条已移入 `handoffs-archive.md`（本文件曾达 303 行，超 200 行上限）。
> 2026-09-24 归档：2026-09-23 共 35 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-23 归档：2026-09-22 共 31 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

## 2026-09-26 — tester-1 — TEST-EXEC + BUILD-435（430/433/434/435 合包）✅ 回归 + 出包（首轮红 → 修 → 重跑绿）

- **性质**：阶段四回归 + 阶段五出包（430 焦点丢失回显窗 / 433 预览原文裁判 + forced 长度比 / 434 切点只落 ≥120ms 真停顿 / 435 读音加权 + `pinyin 0.11.0`）。HEAD `b3aa853`，版本 0.9.3 未动，基线 Publish=BUILD-432。
- **首轮红**：`ts433r_source_anchor`（`window_streaming_texts.get(seq)` 被 rustfmt 拆三行，单行 contains 假红）→ 主控派 coder-2 修（`e64b5f2` 去空白比对，反证有效）→ 重跑绿。废包期间未动 Publish。
- **回归**：bin **1904P/0F/65I**（对 BUILD-432 1844P/64I）⇒ NEW **61** / GONE **0**；root 1992P/0F/67I；`src-tauri` **92P/0F/0I**；Vitest/Browser SKIP。
- **消融**：433a 裁判恒取 A ⇒ 6 红；433b forced 恒 None ⇒ 2 红；434 `TAIL_CUT_MIN_PAUSE_MS`→20ms ⇒ 3 红；435 同音 0.2→1.0 ⇒ 3 红；430 宽度 1.7→1.0 ⇒ 4 红。均还原，`git diff -- src/`=0。
- **构建**：Step1 清进程；Step2 npm 641ms + Tauri 1m36s(17w) + cp；Step3 3m03s（main 91w / crash 9w）；Step4 三 exe + 两 toml 同步 Publish。
- **九项**：全项 PASS。①01:03–01:06 ②两副本 sha 全等且异于 BUILD-432（main `48142a62…` / ui `b4171802…` / crash `24d99504…`）③0.9.3 ④冒烟（见限） ⑤config `da2be5da…` ⑥91/9/17 ⑦正探针见上，反 N-A ⑧两 toml 三副本全等 ⑨VC 五件全等。
- **专项**：A CT2 `efa16d81…` 未重编；B 声纹 `aa3cfc16…` 在包、junction；D 用户数据未动、无 voiceprint.bin；**E 主程序 +662,528B**（15,882,240 vs 15,219,712，`pinyin 0.11.0` 新依赖）。
- **冒烟限制（环境，如实）**：`-debug` 能起/录制（≥55s）/无 crash，无 panic；但麦克风持续未拾取扬声器回放（`kv_long.wav` + TTS 多次重试均 `speech_detected=false`；音频端点疑切到断开蓝牙 EDIFIER，未改系统音频），`[DBG-416] seam … arb=` / `[DBG-433] win` / `[DBG-434] tail cut` **未取到**；433/434/430 行为由回归+消融+探针保证，运行期日志请 Gavin 端测确认。
- **debug-audio**：点名删除本人 6 个（2 session + 4 preroll，见 result.md），移除 0；Gavin 数据未动；测后计数回 30。
- **红线**：未改生产代码 / 版本未动 / 未 push / 零凭证。证据 `collab/evidence/435/`。

## 2026-09-26 — coder-1 — SEAM-INTERIOR-ONLY-436 ✅ 交付（阶段二锚点拼接 + 5 接缝选型实验 · 建议 B）

- **范围**：`src/transcription/mod.rs`（阶段二 + `interior436_tests` 5 条）、`src/main.rs`（阶段一分界记账 + `:9258` 传 `win_split`）、`src/transcription/local_stream.rs`（`interior_split_frac`）。🔴 未 commit、未 build、未出包；版本 0.9.3 未动；零凭证。
- **实现**：`semiglobal_dp_path` 纯加法追加对角线路径；`interior_stitch` 按 `i_t=round(f·m)`、`j_t=round(f·cont_eff)` ±3 内写法一致字、曼哈顿最小/打平取小 i；`push_window_streaming` + 末参 `split_frac: Option<f32>`（`push`/`push_window` 恒 `None` ⇒ 其余管线零改动）；**只在 `committed_prefix=Some` 与 estimate 成功两分支触发，`else` 的 433 裁判链原样保留兜底**；seam 日志行尾追加 `interior=…`。
- **实验（split=0.5）计分：433 胜 1（行0 `某一世`）· 436 胜 1（行4 `市或市长` 保住「或」）· 平 3（行1/2/3 输出逐字相同）；5 行均无 ≥8 字重复、均未兜底。** design §6「5/5 与 ref 一致」两方案均未达成（残余差异全是 GIGO/标点）。
- **根因**：行0 前窗重叠末字 `世` 在后窗为 `时` ⇒ 锚点必须写法一致 ⇒ 可选锚点全在分歧点之前 ⇒ **436 = 分歧点后信后窗 / 433 = 分歧点后信前窗**，前窗对 433 赢、后窗对 436 赢。
- **建议**：**B（保持 433 不上 436）**；C 备选（436 默认关 + 末端分歧守卫）需更多接缝数据；**选型由 Gavin 拍板**。报告：`collab/outbox/coder-1/result.md`。
- **验证**：`interior436` 9P/0F；`fix43*` 39/39；含 416/431/433/435 名字 66/66；fmt EXIT 0；check --all-targets 0 error、warnings test 87 / bin 91 ≤ 基线；全量 1919P/1F/66I（唯一失败 = coder-2 437 在途 `ts381_degenerate_all_zero…`，vad.rs:1965，与本单无关，已 tmux 通知）。
- **⚠️ 跨单副作用**：跑 `cargo fmt` 时 rustfmt **递归格式化了 `src/transcription/vad.rs`**（coder-2 437 在途文件），**仅注释缩进、零逻辑改动**，已 tmux 告知 coder-2。

## 2026-09-26 — coder-2 — SLICE-CUT-REAL-PAUSE-437 ✅ 交付（强制切片优先落 ≥120ms 真停顿 · 方案修订 R1）

- **范围**：**只改 `src/transcription/vad.rs`**（`plan_gap_cuts` R1 三级 + 新常量 + `fix437_*`/`ts381_*` 测试 + `diag437_*` 统计），与 coder-1 的 436（main/local_stream/mod）零文件重叠。统计/报告在 `collab/evidence/437/stats.md`，结果 `collab/outbox/coder-2/result.md`。🔴 未跑全量 test（红线）、未 build、未 commit、无破坏性 git、版本 0.9.3 未动、零凭证。
- **第一步统计（先统计后实现）**：10 段去重真实录音、生产 `dispatch` 口径 20 刀 → **[10,12]s 60.0% ≥50% ⇒ 进第二步**；[10,14]s 70.0%（**不放宽**）；**[8,10]s 75.0%**；silero `vad` 口径 0 刀（min_silence 0.3s 不产 ≥10s 段）⇒ 以 dispatch 为准。
- **R1 实现**：① `pause_in` —— `[起点+8s, 起点+10s]` 取**从 10s 往前最靠近 10s** 的真停顿中点（片 ≤10s、切点不后移）；② `pause_out` —— `[10s, min(+12s, end-1s)]` 最早真停顿；③ 两级都无 ⇒ `find_gap_cut_gap_only`→`find_gap_cut` **逐位不变**（`kind=gap|lowest`）。新常量 `SLICE_CUT_MIN_PAUSE_MS=120`（与 `local_stream.rs::TAIL_CUT_MIN_PAUSE_MS` 同值互相指认）、`SLICE_CUT_INNER_BACK_SECS=2.0`；`min_frames=6`；片内相对坐标 `audio.get(pos..)`。**未改** `find_gap_cut*` / `real_pause_cut_candidates`；尾巴保护 <11s、12s 硬上限、严格相接不变；非末片契约 `[10,12]s`→**`[8,12]s`**。日志 `[DBG-437] slice cut: … kind=pause_in|pause_out|gap|lowest`（`log_enabled!(Debug)`，DEC-077）。
- **实测**：三级 `pause_in` **15** / `pause_out` **1** / 兜底 **4**，并集 `[8,12)` **80.0%**；提前 15 刀 / 持平 4 / 后移 1（唯一后移 `211641` 片长 10.13→10.28s 仍 <12s，已报）；平均提前 **1.369s**、最长 **2.650s**。**8s 下限被数据支持（75%>60%）维持不改**。证据含 20 行「现行切点 vs R1 切点」逐刀对照。
- **既有断言变更（逐条原因见 result §3）**：全零用例改名 `…_picks_inner_window_midpoint…` 且 160160→**144000**（R1-1 正确输出；**保留 `find_gap_cut==10s+半帧`** 证 R1-3 未动）；`ts381_assert_contract` 非末片 `[10,12]`→**`[8,12]`**（其余四项未动）；原验收 ① 改挂 R1-2、④ 改写为 `fix437_pause_below_8s_is_ignored`（7.5s 低于下限不算）；新增 R1-1 专项（9.2s+9.8s ⇒ 必取 9.870s）。
- **验证**：`vad::` **60P/0F/12I**；分项 `fix437` 8、`381` 14、`434` 15、`407` 14、`gap_cut` 10、`sliding_slice` 3、`dispatch` 14、`local_stream` 82、`tail` 59 全 0 failed；`fmt --check` EXIT 0；`check --all-targets` **0 error**、warnings **91/87 = 基线**、新增代码 0 warning。
- **文档**：result.md（含 9 项收尾自证表）、`CHANGELOG.md` 表首行、`logs/20260926.md`、`docs/MACOS-HANDOFF.md` 437 小节（**平台中立，macOS 无同步改动**）、本条。全量回归与出包待 tester-1 合包（基线 1904P/0F/65I，本单 +8 新测 + 1 改名改期望）。

## 2026-09-26 — coder-2 — REPLAY-436 ✅ 交付（433 vs 436 全量 10 段离线对照 · 建议 A）

- **范围**：**只新增** `src/transcription/replay436_tests.rs`（整文件 `#![cfg(test)]`，用例 `#[ignore]`）+ `src/transcription/mod.rs` **1 行** `mod replay436_tests;`。未改 `main.rs`/`local_stream.rs`/`vad.rs`，未改可见性；未跑全量 test、未 build、未 commit、版本 0.9.3 未动、零凭证。产物 `collab/evidence/436/replay/`（6 件），结果 `collab/outbox/coder-2/result.md`（原 437 结果存档 `result-437.md`）。
- **口径**：切片 = 复刻 `diag437_dispatch_slices` + `plan_gap_cuts`；组窗 = `take_context_suffix(3.0字/s)` 后缀 + 本片（共享 1 片）；每窗 `trim_to_speech` + `decode_accuracy_allow_empty`（1.7B 生产同参）精解 + 流式 paraformer 整窗作 R；`split_frac` = `window_overlap_split`；同一几何同一份解码跑 433（`Some=None`）与 436（`Some(split)`）；区域取 `full[committed.len()..]`，全文取 `finish()`。
- **结果（10 段 / 78 窗 / 59 接缝 / 201.7s）**：平 **49** · 433 胜 **3** · 436 胜 **5** · 待回听 **2**；≥8 字重复 **3:3 且 3 条两方案逐字相同（436 零新增回归）**；436 命中 **39(66.1%)** / 兜底 20（`no_align`10·`short_overlap`5·`na`5）；**5 胜全在 `hit`、兜底仅输 1 条（该条参考含 11×「嗯」）**。
- **全文 CER（5 段）**：175022 `0.1384/0.1384` · 175535 `0.0420/0.0336` · 211203 `0.0492/0.0574` · 211641 `0.1176/0.1176` · 223620 `0.1028/0.0467` ⇒ 均值 **0.0900 → 0.0787（−1.13pp，相对 −12.6%）**。
- **分歧**：相同 23 / 不同 36（其中 26 判平，真分高下 10）；主导方向 **433 信前窗 13:5 / 436 信后窗 15:3** —— 与上轮「436 信后窗、433 信前窗」一致且更明显。436 输的 3 条里 2 条参考不可靠（1 伪参考 + 1 含噪 ref）、1 条为真 ref 2pp 临界。
- **真实 split_frac**：中位/p75 **恰 0.500**，**49/59 = 83.1% 与旧硬编码 0.5 逐字一致**，10 条偏离 >2pp（0.414~0.567），59/59 落 40–60% 搜索带 ⇒ **收益来自锚点判据而非 split 精度**。
- **建议**：**A（采纳 436 + 守卫：`兜底*` 一律保持 433；`hit` 且 `a_kept`/`b_from` 差值超阈才改写）** / B（只上真实 split，收益≈0）/ C（维持 433，净收益为负）。**选型待 Gavin 拍板**。
- **本单口径修正（已复跑验证）**：① 末窗空解码清零全文 ⇒ 改取 `finish()`（首批 CER 1.0000 → 0.1384）；② 局部 CER 滑窗改**等长窗**（原 `+12` slack 把参考多出字符全算必删 ⇒ 0.35 虚高 → 0.09，seam3 由「待回听」恢复可判）；③ 日志 `R` 在 436 hit/concat 路径不回填 ⇒ 回放自算 R 列 + 03 表列 `日志R` 佐证，**只报告未改生产**。
- **验证**：`cargo check --all-targets` **0 error**、warnings **91/87 = 基线**、本文件 0 warning；`cargo fmt -- --check` **EXIT 0**；仅跑自己的 ignored 用例。平台中立（纯测试代码），未动 `docs/MACOS-HANDOFF.md`。

---

## 2026-09-26 — coder-1 — SEAM-INTERIOR-ONLY-436 定稿小修（FIX-416-SEAM-R）✅ 交付（seam 日志 `R=` 全路径回填）

- **范围**：**只改 `src/transcription/mod.rs` 的 `push_inner`**（删 1 行 `r_dbg` 初始化 + 增 1 行声明即回填，含 5 行说明注释）+ **新增 1 条单测** `interior436_tests::interior436_seam_log_r_filled_on_all_paths`。🔴 未 commit、未 build；版本 0.9.3 未动；**未碰 `vad.rs` / `replay436_tests.rs`**（`git status` 复核）；零凭证。
- **根因**：`r_dbg` 唯一赋值点是 `arbitrate_or_longer` 返回的 `r30 = r.chars().take(30).collect()`，而 **436 锚点命中（`committed_prefix=Some` 与 `estimate_overlap=Some` 两处提前 `Hit` 走人）** 与 **concat 兜底（`estimate_overlap=None`）** 三条路径不经裁判 ⇒ `[DBG-416] seam` 的 `R=\"\"` 恒空，端测看不到裁判用预览原文。
- **改法**：在 `let r = streaming_overlap_region(...)` 之后、`match &res.committed_prefix` **之前**的直线路径上 `let mut r_dbg = r.chars().take(30).collect::<String>();` —— **同来源**（`r` 即 433 裁判用的 `streaming_overlap_region` 结果）、**同口径**（与 `r30` 逐字符相同）；433 两条裁判的 `r_dbg = r30;` 原样保留作同值覆盖。原 `String::new()` 初始化删除，否则产生 `value assigned to r_dbg is never read`（实测超基线，已自纠回到 91/87）。
- **输出文字不变**：`r_dbg` 全文件仅 4 处 —— 声明 / 2 处裁判覆盖 / `log::debug!` 的 `R=\"{}\"`，不进任何拼接或文本状态 ⇒ 拼接结果逐字不变；仅 Debug 日志 `R=` 由空变非空。
- **单测口径**：走 `guard_prod_lines::prod_lines_excluding_cfg_test`（剔除 `#[cfg(test)]`，避免测试自身字符串自匹配——首版用 `include_str!` 数 `r_dbg = r30;` 数到 3，已自纠为 2），断言行序 `r_src < fill < branch < log`（⇒ hit/concat 三条必继承非空 `R=`，即任务书允许的等价断言）+ `Hit` 分支与 `[DBG-433] concat fallback` 分支仍在 + `r_dbg = r30;` 恰 2 处。
- **验证**：`interior436` **10P/0F**（原 9 + 新 1）、`fix43` **40P**、`416` **19P**、`431` **12P**；`cargo fmt --check` **EXIT 0**（**只用单文件 `rustfmt --edition 2021 src/transcription/mod.rs`**，未跑 `cargo fmt` 以免递归改到 coder-2 的 `vad.rs`）；`cargo check --all-targets` **0 error**、warnings **bin 91 / test 87 = 基线**。全量 test 按派发**未跑**（交 tester-1）。
- **文档**：`collab/outbox/coder-1/result.md`（原 436 阶段二报告归档为 `result-436-phase2.md`）、本条、`logs/20260926.md`。

---

## 2026-09-26 — coder-2 — PREVIEW-PUNCT-LIVE-438 ✅ 预览打点改「3.5s 间隔 + 冻结前缀尾巴重打」单一机制（代码验收通过，已提交 2252ce0）

- **交付**：唯一改动 `src/transcription/local_stream.rs`（+567/−103）。删 1200ms 静默触发 + 显示层 `silent_ms` 计数器（DEC-077 无死代码）；新常量 `PUNCT_PREVIEW_INTERVAL_MS=3500.0` + 独立 `punct_interval_ms` 每 chunk 累加、实际打点后归零；`should_repunctuate_preview` 改纯间隔判定、无新字不打。
- **R1 冻结前缀**：双坐标 `punct_head_chars=committed_len` + `punct_tail_start=last_display_raw_len` 在中途/尾片**两处派发同刻同源**捕获 ⇒ `punct_head` 拼出冻结前缀逐字节不变、`committed_len` 坐标不漂；引擎只见 `raw[tail_start..]` 裸尾巴。R1-2 结论：坐标系 = **带标点显示文本字符数**，改前整段重打有多字/丢字隐患（既有），冻结后不再可达。
- **349 防线升级**：`build_punct_prefix`（存缓存唯一入口）= 冻结前缀 + `strip_trailing_punctuation(尾巴打点)` ⇒ 句终标点根本不入缓存，句中「，。」照旧；新字仍 `punct_cache_reuse` 直接接缓存后。收尾强制打点同只打尾巴（尾片已派发 ⇒ 尾巴空由回灌补；acc 关 ⇒ 坐标 0 整段打）。
- **silent_ms 影响面（任务书必查）**：生产唯一读者即打点触发本身；`should_dispatch_acc`/`should_signal_long_silence` 恒读 `acc_silent_ms`、`endpoint_action` 无时间参、shadow 已随 DEC-086 删 ⇒ **acc 派发逐位不变**（双跑对照测试钉死）；无任何读者依赖「标点清零」。
- **测试**：新增 `fix438_*` **10/10**（验收 ①–⑨ + 源码级接线护栏）；既有 349 测试改名换口径（严格度原样）、349 缓存测试断言一字不动、`guard346` 锚点贴 438 形态、`acc346` 只改 doc、`Flags392`/测试本地仿真零改动。过滤全绿 `fix438` 10P / `punct` 140P/0F/2I / `local_stream` 92P/0F/7I。
- **验证**：`cargo fmt --check` EXIT 0；`cargo check --all-targets` 0 error、warnings **91/87 = 基线**；🔴 未跑全量 test、未 build、未 commit（红线）。**文档**：`collab/outbox/coder-2/result.md`（436 报告备份 `result-436-replay.md`）、`CHANGELOG.md`、`logs/20260926.md`、`docs/MACOS-HANDOFF.md`（平台中立一行）、本条。

---

## 2026-09-26 — coder-2 — FIX-EDIT-STUCK-PROCESSING-439 ✅ 交付（编辑态卡「识别处理中」P0 · 两层修复 + A/B/C/D/E 五路走读）

- **范围**：**只改 `src/main.rs`**（+256/−19，恰 3 块：controller `Processing` 臂守卫 :7749 / worker 取消门 :9944 / 新测试 mod :15016）。`destroy_edit_control` 清理逻辑零改动（改法 3）。🔴 未 commit、未 build、未跑全量 test（红线）；版本 0.9.3 未动；零凭证。结果 `collab/outbox/coder-2/result.md`（438 报告备份 `result-438.md`）。
- **改法（两层）**：① controller `PipelineEvent::Processing` 臂加 `OVERLAY_EDITING` 守卫（:7756，与 282 预览守卫 :7524 / 038-C 压制臂 :7775 同款写法），编辑态整臂跳过 `ACC_REFLOW_SUPPRESS` 置位/托盘/`show_overlay(FallingToProcessing)`，打 info `ASR-038-C: Processing suppressed while editing`（:7757）；② worker `asr_handle.join()`（:9932）后两次早期发送（收尾预览 :9952 + 识别处理中 :9957）包 `if cancel_signal.load` 门（:9944），取消只打 info（:9946）不发。**源头不发 + controller 兜底**，竞态两向均覆盖（走读见 result §三 A 步骤 4）。
- **E 查清（任务书 🔴 分岔不触发）**：停止按钮 = **取消**——`stop button cancels`（:2552）→ `cancel_btn_rect`（:2553）→ `CancelRequested`（:2558）→ 控制器臂 :7906–7918（cancel + stop + editing=false + Hide + 托盘 Idle，**无 Submit/注入**）；「取消 = 不出字」全仓一致（:8728/:10147/:10260）⇒ `cancel_signal` 门成立。正常松键 `HotkeyEvent::Stop`（:7330）只置 `stop_recording_signal`（:7349）**不置 cancel** ⇒ else 分支出字逐位不变（全文件 `cancel_signal.store(true` 恰 4 处 = CancelStop :7382 / Esc :7392 / 停止钮 :7912 / 编辑点击 :7931）。
- **走读（全带行号，result §三/§四）**：**A** 录音点击 :2564 → EditRequested :7922（editing=true :7925 先于 cancel :7931）→ 门 :9944 拦 + 臂 :7756 兜底 ⇒ EDIT 不被 :1854 销毁；初值回退链 :2026–2035 + 快照 :2038；提交 Enter :1140 / 按钮 :2540 → SubmitRequested :7938 → `inject_text` :8007 **注入编辑框文字** ⇒ Idle+Hide。**B** 松键不置 cancel ⇒ :9944 发送逐位不变；点击/编辑/提交零 diff。**C** Esc 子类 :1111 / 收口 :7906 / FocusLost :7783 均不在 diff 内，与改前一致。**D** Esc 轮询 :7389–7397 走 cancel ✓；改前与点击编辑共用同一发送点会闪「识别处理中」，改后直接取消、不闪、不注入。**E** 改前闪处理态→改后不闪，仍无输出（原设计）。
- **测试**：`edit_stuck_439_tests` **4/4**（:15030 守卫先于置位/show、:15075 门先于两发送且 282 顺序不变、:15121 E 语义 cancel→Hide 无 Submit、:15180 恰一处 ESC 轮询置 cancel + `is_recording` 守卫 + Idle；`prod_lines_excluding_cfg_test` + `concat!` 拆字面量防自匹配）。过滤全绿：`439` **4P**、`382` **18P**、`038` **2P**、`edit` **28P**、`suppress` **5P**（既有 `early_preview_then_processing_before_acc_join`、`suppress_flag_only_set_inside_processing_arm` 均不受影响）；`fmt --check` EXIT 0；`check --all-targets` 0 error、warnings **91/87 = 基线**。
- **文档**：result.md（A–E 走读 + 12 项自证表）、**本条（438 漏写教训，本单已写）**、`docs/MACOS-HANDOFF.md` 439 小节（macOS 无编辑态 ⇒ 无同类 bug；worker 门在平台中立 `spawn_worker_thread` :8295（Windows :10342 / macOS :10607 共用）⇒ 零同步）、`CHANGELOG.md` 表首行、`logs/20260926.md`。

## 2026-09-26 — 主控 — 440 + 441 接手开发 + BUILD-441 出包（Worker 套餐超限）

- **接手原因**：coder-1 / coder-2 / tester-1 套餐超限停止，Gavin「你直接接手他们的工作吧」「把代码开发完、补充测试用例做测试、然后出包、对齐文档」。
- **接手时进度**：440 未开工（text_normalizer.rs 零改动）；441 半成品（回灌处调用 + 纯函数 + 临时 scratch 测试，最终节点与管线声明未做）。
- **440**：规则 D `collapse_restarts` + `rule_d_*` 9 条。**441**：最终 `final_src` 同源去重、`FinalFillerNode` 显式声明（本地实时 DoneUpstream / 退回 2pass 与在线批处理 AtFinal）、删 scratch、`reflow_filler_once_441_tests` 6 条。
- **测试**：全量 2039P/0F/69I；消融两处均红并逐字节还原。**出包**：Step2 跳过（ui 无改动），主程序 `c8fc64a1…`，九项要点全过，冒烟正常。
- **待 Gavin 端测**：①录音中点浮层进编辑→改→提交上屏 ②录音中 Esc / 停止按钮直接收起不闪 ③「你们的知，你们的知识」预览与上屏都去掉前段。

## 2026-09-26 — 主控 — DISPATCH-LONG-SPEECH-442 开发 + BUILD-442 出包（Worker 不可用）

- **起因**：BUILD-441 端测背景人声连说 17s 挂在预览；日志坐实 346 只按静默派发、437 切点只作用于派发后。
- **改动**：派发触发 = 静默 1200ms 或满 10s（回看切点，`vad::slice_cut_at` 与兜底切片同一函数，上限 11s、满 12s 兜底）；预览分界按切点那刻流式文字；兜底切片保留。DEC-088。
- **测试**：全量 2048P/0F/69I（新增 9）；2 条护栏按派发点口径更新计数；消融 2 处均红、还原。**出包**：主程序 `b9154b38…` 22:27，九项要点全过，冒烟正常。
- **待 Gavin 端测**：连续说话 / 背景人声不停 ≥10s 时，预览是否在录音中途被精解刷新、背景人声是否中途剔除；日志看 `seg dispatch … reason=lookback kind=…`。

## 2026-09-26 — 主控 — FIX-ACC-GUARD-POLLUTED-STREAM-443 + BUILD-443

- **起因**：Gavin 旁放他人语音端测，预览没被精解刷新。查证：声纹零剔除（干扰 <1.5s 或与本人同单元），最终干净是 LLM 删的；另有正确精解被 406 误拒（流式被带偏）。
- **改动**：406 加放行路径（保留率 ≥0.3 且长度比 ≥0.8）；当日 19 条重算只变两条误拒。**测试** 2051P/0F；消融红；**出包** `35f8f4ce…` 22:53。
- **待办**：`POC-VOICEPRINT-SHORT-444` 离线测短片声纹（不改代码）。**端测**：旁放语音时，精解回灌后预览是否变成正确文字（日志 `DBG-406 … accept=true`）。

## 2026-09-26 — 主控 — FIX-LATE-STREAM-TAIL-445 + BUILD-445

- **起因**：LLM 关闭端测，预览末尾多「算了」、最终没有。337 边界冻结后流式滞后吐字、静默无新派发 ⇒ 永远不被精解覆盖。
- **改动**：冻结后跟踪，VAD 无新语音时迟到字归入上一片、重登记边界；开口 / 新派发停止。**测试** 2056P/0F；消融红；**出包** `b9bd0db3…` 23:14。
- **端测**：说完停顿不松键，看预览末尾是否还会多出字；日志 `[DBG-445] late stream chars absorbed`。声纹：本段零 `DropNonUser`（干扰片 0.8~1.06s KeepShort、混合单元 0.88 KeepUser），`POC-VOICEPRINT-SHORT-444` 进行中。

## 2026-09-26 — 主控 — POC-VOICEPRINT-SHORT-444（离线测试，生产零改动）

- 3 段旁放语音录音离线测：本人短片 1.0~1.5s 最低 0.658（门 0.45，安全）；干扰与本人混在同一段、得分 0.84 判本人，未形成独立段 ⇒ 调门槛无效。建议不调 432。测试 `poc_voiceprint_short_444.rs`（#[ignore]），证据 `collab/evidence/444/`（不入库）。

## 2026-09-26 — 主控 — 446（声纹门槛 1.0s + 兜底剔除远场）+ BUILD-446

- **起因**：15:23 旁放语音端测，干扰短片未判 + 精解失败后流式兜底写入背景误识别。
- **改动**：DEC-089。门槛 1.0s；远场区间从流式线程逐 chunk 记录，经派发回调第 6 参 → 片 / 窗坐标 → 兜底时并入剔除。**测试** 2061P/0F；声纹 12 条按新门槛更新；消融 2 处均红；**出包** `ff1b5056…` 23:43。
- **端测**：旁放语音时最终文字是否不再含背景误识别；日志 `DBG-412` 有无 1.0~1.5s 的 `DropNonUser`、`[DBG-446] fallback far-field excluded`。⚠️ 关注本人 1 秒左右短句是否被误剔。

## 2026-09-27 — 主控 — VOICEPRINT-FRAGMENT-GROUP-447 + BUILD-447

- **起因**：16:11 端测，<1.0s 旁人碎片漏进预览与最终。
- **改动**：DEC-090，碎片拼组再判（`speaker.rs`）。离线 POC-447 复核真录音 6 窗：本人全留、旁人全剔。**测试** 2066P/0F；消融红；**出包** `757288aa…`。
- **端测**：旁放语音时预览与最终是否干净；日志 `[DBG-447] fragment group`。⚠️ 留意本人插话短句（「对」「嗯」）是否被误删。

## 2026-09-27 — 主控 — VOICEPRINT-SLIDING-448 + BUILD-448

- **起因**：16:41 端测，他人语音与本人连成一段整段判本人，干扰经 406 拒收 + 兜底进最终。
- **改动**：DEC-091，本人长段内 1s 滑窗子剔除（`speaker.rs`）。POC-448b 两段录音窗口级复核通过。**测试** 2072P/0F；消融红；**出包** `5a761a68…`。
- **端测**：他人插话与本人连着说时是否被切出；日志 `[DBG-448] sliding sub-drop`。⚠️ 留意本人含糊 / 压低嗓音的句子是否被误删。

## 2026-09-27 — 主控 — REVERT-VOICEPRINT-449 + BUILD-449（Worker 不可用，主控自做）

- **起因**：Gavin「短于1s的碎片组合做声纹比较……有时长损耗、有明显迟滞感」；取证 447 单次 ≈16ms、448 每段 110~250ms 且剔多留少 ⇒ Gavin「一起去」，并要求「不可影响既有的预览刷新和精确识别功能、还有管线上其他功能」。
- **改动**：DEC-092。`speaker.rs`、`poc_voiceprint_short_444.rs` 还原为 `8598176` ⇒ `src/`、`src-tauri/`、`ui/src` 与 BUILD-446 逐字相同；446 保留。
- **测试**：全量 2061P/0F/70I = BUILD-446 逐项相同。**出包** `b7c6443a…` 15:18，九项全过，冒烟正常，config 未动。
- **端测**：旁放语音时 1 秒以上干扰仍应剔除（`DBG-412` `DropNonUser`）；预览刷新与最终上屏是否不再迟滞；日志不再出现 `DBG-447` / `DBG-448`。

## 2026-09-27 — 主控 — LOCALRT-WARM-CACHE-450 ✅ 已提交（随下一包出）

- 剪静音 VAD + 声纹 CAM++ 改进程级缓存 + 本地实时模型加载时预热；每次录音首窗精解预计提前 ~0.4–0.55s，识别结果逐位不变。全量 2062P/0F/72I。
- 端测：日志 `speaker: CAM++ extractor loaded` 整个运行期只出现 1 次（模型加载时）；首窗 `seg dispatch #0` → `DBG-388 trim` 间隔 ~200ms。

## 2026-09-27 — 主控 — ACC-ENGINE-LLAMACPP-452 ①②③ 已提交（⑤ 流式回灌进行中，未出包）

- 1.7B 精解换 llama.cpp（Q8_0 + f16，Vulkan / CPU 自动选）+ 预览草稿 + 本地实时恢复词库（DEC-093）；回放 CER 7.87%→4.63%、精解 ~2.2×。
- 出包前须 `powershell -ExecutionPolicy Bypass -File scripts\fetch-llama-runtime.ps1`（运行库 20 个 dll + GGUF 模型同步到 target / Publish，sha256 校验）。
- 端测看：日志 `[ACC-452] Qwen3-ASR llama.cpp engine loaded: device=…`（应为显卡名）、`[ACC-452] decode: … prefill=…ms gen=…ms draft=a/p`。

## 2026-09-27 — 主控 — BUILD-452（450 + 452 ①②③⑤）✅ 出包，待 Gavin 端测

- 内容：精解换 llama.cpp（Vulkan，Q8_0 + f16）+ 预览草稿 + 本地实时恢复词库 + 流式回灌 + 首窗预热（450）。回放 CER 7.87%→4.63%、精解 ~2.2×、流式回灌平均提前 199ms（闪烁 0.7%）。
- 端测看：① 精解结果是否更准（专有名词 / 词库词）② 预览是否更快被精解刷新、刷新是否逐步出现 ③ 有没有闪回（中途文字又被改掉）④ 日志 `[ACC-452] decode: … prefill=…ms gen=…ms draft=a/p`、`[ACC-452] partial reflow`。
- ⚠️ 词库变更不再触发模型重载（下一次录音自动生效）；`Publish/config.toml` 当前档位为 performance（快速档，不走新引擎），target/release 为 local_realtime。

## 2026-09-27 — 主控 — BUILD-452b（流式回灌回缩修复）✅ 出包，待 Gavin 端测

- 修 BUILD-452 端测「说话途中预览文字短暂被吞掉又出现」：半截精解改为按「精解写到预览的位置」逐字替换，显示长度不回缩（回放明显回缩 88 → 0）。主程序 `2f902d7e…`。
- 端测看：说话途中预览是否只增不缩、精解文字是否逐步替换预览；日志 `[ACC-452] partial reflow` 与 `[LocalRT-DBG-337] reflow applied … preview_len` 应单调不减（允许 ≤2 字口水词修正）。

## 2026-09-29 — 主控 — MEM-453 + VOICEPRINT-EMB-CACHE-454 ✅ 开发完成（⏸ Gavin「等下出包」）

- **453**：KV q8_0 + ubatch 128（`shim.cpp`）；音频编码器 f16→Q8_0，无回退（本地 f16 仅留作对照）；NLLB 改本地翻译路径懒加载（CT2 析构死锁 ⇒ 不做闲置释放）。显存约省 0.72G；回放 Q8 vs f16 编码器生产路径 CER 4.80% 持平，较旧基线差 1 字（KV/ubatch），Gavin「可以接受」。
- **454**：声纹向量按单元音频指纹缓存，判定仍按当前档案现算（逐位不变）。
- **验证**：全量 bin 1979P/0F/74I；fmt clean；warnings 90；src-tauri check 通过。
- **待办**：整机显存复测（工具检查报错中断）；出包等 Gavin 指令；端测看 `emb_cached=true` 命中数与显存。
- **红线**：未 push；版本 0.9.3 未动；零凭证。
