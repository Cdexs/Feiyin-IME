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

## 2026-09-29 — 主控 — MEM-453 + VOICEPRINT-EMB-CACHE-454 ✅ 开发完成（⏸ Gavin「等下出包」）

- **453**：KV q8_0 + ubatch 128（`shim.cpp`）；音频编码器 f16→Q8_0，无回退（本地 f16 仅留作对照）；NLLB 改本地翻译路径懒加载（CT2 析构死锁 ⇒ 不做闲置释放）。显存约省 0.72G；回放 Q8 vs f16 编码器生产路径 CER 4.80% 持平，较旧基线差 1 字（KV/ubatch），Gavin「可以接受」。
- **454**：声纹向量按单元音频指纹缓存，判定仍按当前档案现算（逐位不变）。
- **验证**：全量 bin 1979P/0F/74I；fmt clean；warnings 90；src-tauri check 通过。
- **待办**：整机显存复测（工具检查报错中断）；出包等 Gavin 指令；端测看 `emb_cached=true` 命中数与显存。
- **红线**：未 push；版本 0.9.3 未动；零凭证。

## 2026-09-29 — tester-1 — TEST-EXEC-454 ✅ 阶段四全量回归（全绿）· 阶段五待令

- **性质**：阶段四回归（MEM-453 + VOICEPRINT-EMB-CACHE-454），HEAD `5ed28de`，工作区无生产代码改动。阶段五 BUILD-454 🔴 挂起等主控「现在可以出包」。
- **回归（全绿 0F）**：root `cargo test --no-fail-fast` **2067P/0F/76I**（bin `feiyin-ime` **1979P/0F/74I**）；`src-tauri` **92P/0F/0I**；Vitest **7 files / 100P / 11 skip**；`cargo fmt --check` **EXIT 0**。
- **NEW/GONE 对账**：基线 BUILD-452b root 2069P/76I → **2067P/76I**、bin 1981P/74I → **1979P/74I**，逐位吻合。NEW 4 = `emb_cache_454_tests::{t454_same_audio_reuses_embedding,t454_changed_audio_recomputes,t454_filter_uses_cache_and_judges_fresh}` + `mem453_translation_engine_lazy_loaded`；GONE 6 = 旧 `translation_needs_reload_*` 全 0 命中。`migrate320_qwen3_readiness_checks_four_paths` 照常绿。
- **warnings**：`cargo check` EXIT 0，`generated` main **90**（基线 91，−1）/ crash **9**；poc_halluc 1 + 依赖 ct2-sys 18 不计基线（全文件 `^warning` 125 为二者之和 + build 脚本提示，非本批新增）。
- **失效模式**：精解引擎加载 / 翻译懒加载 / 声纹判定三条重点全部绿，无命中。
- **阶段五 BUILD-454 出包（主控放行「现在可以出包」）**：Step2 npm 1.40s + Tauri custom-protocol 1m50s(17w) + cp；Step3 3m12s（main 90w / crash 9w）；Step4 三 exe + 四张规则 toml 同步；`fetch-llama-runtime.ps1 -Targets target\release,Publish`（20 DLL + Q8 编码器，无下载）。
- **九项全 PASS**：①main 15:34:47/ui 15:31:26/crash 15:32:44 ②两副本全等且三件异于 452b（main `3147909227…`/ui `596e1bda…`/crash `557d6b76…`；⚠️ crash 源未改，198 B 元数据抖动 = CodeView GUID + winres 项序，已如实说明）③0.9.3.0 ④冒烟 PID 111168 Responding/无 crash ⑤config `da2be5da…` 三时点不变 ⑥90/9 ⑦正 Q8 探针 main1/ui1 + `emb_cached=` main1，反 f16 main0/ui0 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。Publish 无 f16 mmproj；模型 sha Q8_0 `58e22d05…`/mmproj Q8_0 `46c1d533…`/tokenizer `aeb13307…`；llama 20 DLL 三处一致；冒烟 engine device=AMD Radeon 780M Graphics、启动无 NLLB 加载。
- **红线**：未改生产代码、版本 0.9.3 未动、未 push、零凭证。报告 `collab/outbox/tester-1/result.md`。

## 2026-09-29 — tester-1 — TEST-EXEC + BUILD-454 ✅（453 + 454）

- 全量 2067P/0F/76I；九项全 PASS；产物 main `31479092…` / ui `596e1bda…` / crash `557d6b76…`；Publish 编码器仅 Q8_0；冒烟正常、启动无 NLLB 加载。结果 `collab/outbox/tester-1/result.md`。主控抽查通过。待 Gavin 端测。

## 2026-09-29 — 主控 — FORCED-ALIGN-456 ✅ 开发 + 回放完成（DEC-094）

- 0.6B 强制对齐接入本地实时，窗口接缝按逐字时间拼接；删 416/431/433/435/436 估算链与旧测试、pinyin 依赖。
- 验证：全量 bin 1904P/0F/71I；对齐器 112 窗与 CrispASR 逐字一致；回放接缝重复 5→0、CER 4.80%、闪烁 5.2%、回缩 0；显存 1.60G / 每窗 248ms。
- 下一步：派 tester-1 TEST-EXEC + BUILD-456（Publish 须经 fetch 脚本同步对齐模型三件套）。

## 2026-09-29 — tester-1 — TEST-EXEC-456 ✅ 回归全绿 + BUILD-456 ✅ 出包（版本 0.9.4）

- **回归**：bin **1904P/0F/71I**（＝预期）/ root 1992P/0F/73I / src-tauri 92P/0F/0I / Vitest 100P/11skip / fmt EXIT 0；NEW 20 全绿、GONE 9 模块 0 命中。⚠️ `interior436_tests` 实际未删（4 条仍绿，`window_overlap_split` 仍被生产调用），与任务书 GONE 清单不符，如实上报，不影响 1904 计数。
- **出包九项全 PASS**：①21:09:17 / 21:06:10 / 21:07:14 ②两副本全等、三件异于 454：main `5d60c1d8…` / ui `0dc5bd41…` / crash `24952e98…` ③0.9.4.0（ui 0.9.4）④冒烟 PID 69408 Responding / 无 crash ⑤config `da2be5da…` 三时点不变 ⑥90/9 ⑦正 `aligner-backbone-q8_0.gguf` main1/ui1 + `[ALIGN-456] aligner attached` main1，反 `[DBG-416] seam:`/`[DBG-433] win:` main0/ui0 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **模型**：对齐器三件 sha `1b5ea4c2…`/`7117f45d…`/`8a3f5ed7…` 全对；1.7B 照旧；Publish 无 f16 mmproj；20 DLL 三处一致。冒烟 engine `device=AMD Radeon 780M Graphics` + `[ALIGN-456] aligner attached`、启动无 NLLB 加载；`window align` 日志因无语音输入未取到（如实说明）。
- **红线**：未改生产代码 / 版本未动 / 未 push / 零凭证。报告 `collab/outbox/tester-1/result.md`。

## 2026-09-29 — tester-1 — TEST-EXEC + BUILD-456 ✅（FORCED-ALIGN-456，v0.9.4）

- 回归 1904P/0F/71I；九项全 PASS；产物 main `5d60c1d8…` / ui `0dc5bd41…` / crash `24952e98…`；主控抽查通过。待 Gavin 端测。

## 2026-09-29 — 主控 — MEM-TRIM-457 + PIPE-SPEED-457 ✅ 开发 + 回放完成（DEC-095）

- 省内存：1.7B 输出上限 16、上下文 1024 起按需扩（≤4096）、词库计数改用引擎词表（删 tokenizer.json 依赖与 `tokenizers` 直依赖）、本地实时 VAD 分段器按需建。
- 提速：精解结果先回灌、对齐晚到再按时间重拼；松键最后一窗不对齐；系统提示 / 词库前缀 KV 复用。
- 回放 17 段 120 窗：解码 −7.4%；对齐（282ms/窗）移出关键路径；终稿与 456 逐位相同 17/17、CER 4.80% 不变；预览 7/112 窗约 0.28s 后微调接缝。
- 自测：bin 1909P/0F/74I；fmt clean；warnings 90；src-tauri check 通过。
- 下一步：QUANT-AB-457（等 bf16 下完）→ 定是否换 Q6_K + f16 编码器 → 派 tester-1 回归 + 出包（显存 / 内存对比 BUILD-456）。

## 2026-09-29 — 主控 — QUANT-AB-457 ✅ 测完：不采纳（DEC-096）

- 对 bf16：现行 Q8 + Q8 字差 0.37%、CER 4.80%（= bf16）；Q6_K + f16 字差 0.93%、CER 5.00%、慢 9%、省 ~0.24G ⇒ 保持现行，生产零改动。
- 本批（457）功能全部完成 ⇒ 任务书 TEST-EXEC + BUILD-457 已写好，待 tmux 会话起来后派 tester-1。

## 2026-09-30 — tester-1 — TEST-EXEC-457 ✅ 回归全绿 + BUILD-457 ✅ 出包（版本 0.9.4；重跑落档）

- **背景**：上一会话回归汇总时模型接口 invalid request 中断、结果未落盘；本会话按要求从头重跑并落档，final artifacts 替换上一会话未落盘的临时值（原 `89eeb3c6…` / `a35395c5…` / `eb88a45c…`）。
- **回归**：bin **1909P/0F/74I**（＝预期）/ root 1997P/0F/76I / src-tauri 92P/0F/0I / Vitest 100P/11skip / fmt EXIT 0；NEW 5（`refine_reflow_457_tests`）+ NEW ignored 3（`replay457_*` ×2 + `poc457_prefix_determinism`）、GONE 0；`migrate320_*` / `curate_*` 绿；warnings 90/9。
- **出包九项全 PASS**：①main 00:30:54 / ui 00:18:08 / crash 00:29:38（Publish 00:31:06）②两副本全等、三件异于 456：main `ba491a57…` / ui `50b150b4…` / crash `7a176aa1…` ③0.9.4.0（ui 0.9.4）④冒烟 PID 36680 Responding / 无 crash / zero residual ⑤config `da2be5da…` 三时点不变 ⑥90/9 ⑦正 `[PIPE-457] last window: skip align` 1 / `[MEM-TRIM-457] llama context grown` 1 / `LAS_NO_PREFIX_CACHE` 1，反 `HOTWORDS-TOKEN-268: tokenizer loaded` 0 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **模型**：不变，1.7B + 对齐三件 sha 全对；Publish 无 f16 mmproj、tokenizer 目录保留；20 DLL 三处一致。冒烟 `engine loaded … ctx=1024(max 4096)` + `[ALIGN-456] aligner attached`、无 tokenizer/NLLB 加载。
- **🟢 窗口路径实测取到**（麦克风 C920 拾环境音，非人工朗读）：`[ALIGN-456] window align` seq=1/2/3 + `[PIPE-457] last window: skip align seq=4` + `[DBG-457] seam refine`；新 exe 复现 `window align seq=0` + `skip align seq=1`。
- **显存/内存**：引擎加载后 WS **1305.2 MB** / Dedicated **3739.7 MB** / Shared **665.8 MB**；录一段后（运行态）WS **1663.2 MB** / Dedicated **3780.2 MB** / Shared **841.2 MB**（参考 456 运行态 ~1.6G / ~3.75G / ~0.905G）。
- **红线**：未改生产代码 / 版本未动 / 未 push / 零凭证。报告 `collab/outbox/tester-1/result.md`。

## 2026-09-30 — 主控 — LLAMA-TUNE-458 评估 + DRAFT-LANG-PREV-458 + VAD-REUSE-458 ✅ 开发完成（DEC-097）

- 评估：`collab/research/llama-tune-458.md`；Gavin「一起做」建议①②。
- ① 草稿加语种标记 + 上一窗精解文字：回放 −13%、CER 4.80% 不变、终稿差异仅标点 ② 中途片按开始说话判定位置补齐、末尾窗拼区间：真实录音 0 吞字。
- 自测：bin 1910P/0F/77I；fmt clean；warnings 90；E2E `t458_mid_speech_dispatch_e2e` / `timeline393_r3` 通过。
- 下一步：派 tester-1 TEST-EXEC-458 + BUILD-458。

## 2026-09-30 — tester-1 — TEST-EXEC-458 + BUILD-458 ✅ 回归全绿 + 出包（版本 0.9.4）

- **性质**：阶段四全量回归 + 阶段五出包（回归全绿即出包）。HEAD `6df6e0f`，工作区无生产代码改动（`git status --porcelain` 空）。
- **回归（全绿 0F）**：bin **1910P/0F/77I**（＝预期）/ root 1998P/0F/79I / src-tauri 92P/0F/0I / Vitest 7 files·100P·11skip / fmt EXIT 0。
- **NEW/GONE 对账**：NEW ok 1 `t458_mid_speech_ranges_cover_ongoing_tail`；NEW ignored 3 `replay458_tune`/`poc458_quality`/`t458_mid_speech_dispatch_e2e`；改写 2（`ts393_dispatch_slice_ranges_fallback_when_mid_speech`、`ts393c_..._source_guard`）绿；**GONE 0**（1909+1=1910、74+3=77）。重点失效模式全绿；全库 FAILED=0。
- **出包九项全 PASS**：①main 22:26:53 / ui 22:26:54 / crash 22:25:15（Publish 22:27:10）②两副本全等、三件异于 457：main `d2b5adcf…` / ui `8d4dc3e9…` / crash `21ba6f9a…` ③0.9.4.0（ui 0.9.4）④冒烟 PID 27884 Responding / 无 crash / zero residual ⑤config `da2be5da…` 三时点不变 ⑥main 90 / crash 9 / Tauri 17 ⑦正 `LAS_NO_DRAFT_LANG` 1 / `[PIPE-457] last window: skip align` 1 / `[MEM-TRIM-457] llama context grown` 1，反 `LAS_DRAFT_LANG` 0 / `HOTWORDS-TOKEN-268: tokenizer loaded` 0 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **模型**：1.7B Q8_0 `58e22d05…` / mmproj `46c1d533…`；对齐三件 `1b5ea4c2…` / `7117f45d…` / `8a3f5ed7…` 全对（不变）；Publish 无 f16 mmproj、tokenizer 保留；32 DLL 两处一致。冒烟 `ctx=1024(max 4096)` + `[ALIGN-456] aligner attached`、无 panic。
- 🔴 **窗口路径未取到新 decode 行**：录音尝试 7.7s（C920）但 `speech_detected=false`（环境静默），如实记录；日志中 `draft=a/b` / `source=vad|timeline` 系本构建前 04:11Z 会话，非本包证据。
- **红线**：未改生产代码 / 版本未动 / 未 push / 零凭证；debug-audio 点名删本人 3 个、Gavin 24 个未动。报告 `collab/outbox/tester-1/result.md`。出包 Step 1 已结束输入法进程，**请主控转告 Gavin 重启**。

## 2026-09-30 — 主控 — PHANTOM-406-459 ✅ 开发完成（DEC-098）

- 根因（Gavin BUILD-458 端测录音 1）：静音幻字经 406 兜底进原始转写 + 兜底窗无时间致接缝比例估算多切。
- ① 派发片开头剥静音幻字（不换流、不动预览；换流方案实测漂移已否）② 兜底窗强制对齐、按时间拼接。
- 自测：bin 1912P/0F/80I；fmt clean；warnings 90；真录音 17 段对照 + 兜底窗回放通过。
- 下一步：派 tester-1 TEST-EXEC-459 + BUILD-459。


## 2026-09-30 — tester-1 — TEST-EXEC-459 + BUILD-459 ✅（回归全绿 ⇒ 出包）

- 阶段四全绿：bin **1912P/0F/80I**（预期一致）、root 2000P/0F/82I、src-tauri 92P/0F/0I、Vitest 100P/11skip、fmt EXIT 0；NEW 2 + ignored 3、GONE 0；warnings 90/9；重点失效模式全绿。
- 阶段五 BUILD-459（0.9.4）：九项全 PASS（①~⑨见 `collab/outbox/tester-1/result.md`）。main `db8c5e74…` / ui `2377357b…` 异于 458；crash `21ba6f9a…` 同 458（本批零改动）。
- 冒烟：`-debug` PID 23180 Responding；`ctx=1024(max 4096)` + `aligner attached`；无 panic / 无 crash.json / 零残留。
- 出包 Step 1 已结束输入法进程 ⇒ Gavin 需重启（提醒转告）。

## 2026-10-03 — tester-1 — TEST-EXEC-460 + BUILD-460 ✅（回归全绿 ⇒ 出包）

- **性质**：阶段四全量回归 + 阶段五出包（VOICEPRINT-LEAK-460 / DEC-099）。HEAD `58e3720`，工作区 clean。版本 **0.9.4 未动**。
- **回归（全绿 0F）**：bin **1913P/0F/80I**（＝预期）/ root 2001P/0F/82I / src-tauri 92P/0F/0I / Vitest 7 files·100P·11skip / fmt EXIT 0。NEW 1 `t460_voiceprint_dropped_all`、GONE 0。重点失效模式（guard291/342/346、fix438、ts393、t456/457/458/459、partial_reflow_452、tail_window_407）全绿，全库 0 FAILED。
- **出包九项全 PASS**：①main 12:39:22 / ui 12:34:07 / crash 09-30 22:25（本批零改动）②两副本全等、main `1d68bdfc…` / ui `5d988735…` 异于 459，crash `21ba6f9a…` 同 459 ③0.9.4.0（ui 0.9.4）④冒烟 PID 23544 Responding / 无 crash / 零残留 ⑤`Publish/config.toml` `da2be5da…` 三时点不变 ⑥main 90 / crash 9 / Tauri 17 ⑦正 `[VOICEPRINT-LEAK-460] all speech dropped by voiceprint`1 + `[PHANTOM-459] stripped silence-born text`1，反 `[PHANTOM-459] restart stream`0 / `LAS_DRAFT_LANG`0 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **模型**：不变，1.7B Q8_0 `58e22d05…` / mmproj Q8_0 `46c1d533…`；对齐三件 `1b5ea4c2…`/`7117f45d…`/`8a3f5ed7…` 全对；Publish 无 f16 mmproj；32 DLL。冒烟 `ctx=1024(max 4096)` + `[ALIGN-456] aligner attached`、无 panic；无录音故无 decode 行（如实）。
- **故障如实记录**：Step1 首轮残留 `feiyin-ime.exe` PID 23124（`target\release\`）持文件锁，致首次 `cargo build --release` 在删除旧 exe 处 EXIT=101（编译已通过）；补杀后 Step3 成功。
- **红线**：未改生产代码 / 版本未动 / 未 push / 零凭证。报告 `collab/outbox/tester-1/result.md`。**出包 Step 1 已结束输入法进程 ⇒ 请主控转告 Gavin 重启。**

## 2026-10-03 — 主控 — VOICEPRINT-LEAK-460 ✅ 开发完成（DEC-099）

- 声纹全剔 ⇒ 不走整段兜底解码 ⇒ 按「未检测到语音」处理。bin 1913P/0F/80I；fmt clean；warnings 90。
- 下一步：派 tester-1 TEST-EXEC-460 + BUILD-460。

## 2026-10-03 — 主控 — STREAM-SV-463 ✅ 开发完成（DEC-100）

- 预览 = 官方 SenseVoice-Small 2024-07-17 int8 模拟流式（每 0.6s 重解当前句、8 线程 = 物理核）；派发点即冻结；末尾未定字浅色；performance 档同一模型、关 ITN。
- 端到端抓出并修掉「标点缓存默认文字只增不改」（PREVIEW-REWRITE-463）：缓存复用核对原文、重解挪到派发之后、442 按 token 时间切分。
- 定向回归全绿；`sv463_preview_freeze_e2e` 全绿。新模型已放 `models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/`（与 poc-462 下载件逐字节一致）。
- 下一步：派 tester-1 TEST-EXEC-463 + BUILD-463；旧模型目录（funasr-nano 2025-12-17、streaming-paraformer-trilingual）待 Gavin 确认后删除。

## 2026-10-03 — tester-1 — TEST-EXEC-463 + BUILD-463 ✅（回归全绿 ⇒ 出包）

- **性质**：阶段四全量回归 + 阶段五出包（STREAM-SV-463 / DEC-100）。HEAD `90620ca`，工作区 clean。版本 **0.9.4 未动**。
- **回归（全绿 0F）**：bin **1910P/0F/77I**（＝预期）/ root 1998P/0F/79I / src-tauri 92P/0F/0I / Vitest 7 files·100P·11skip / fmt EXIT 0。NEW 通过 8 + 忽略 2、GONE 通过 11 + 忽略 5（全 0 命中）、改写 3 组（ts405b/t445_wiring/fix438×10）全绿。重点失效模式（guard463/fix438/ts393/guard346/guard384/ts405b/t445/t456/457/458/partial_reflow_452/tail_window_407）全绿、0 FAILED。
- **Step7 真模型 E2E**：`sv463_preview_freeze_e2e` **1P/0F/11.56s**（56.1s 录音 / 处理 8.6s / 6 片 6 边界 / ja+ko 预览 / 标点引擎在位）。
- **出包九项全 PASS**：①main 17:34:15 / ui 17:31:01 / crash 09-30 22:25（零改动）②两副本全等、main `2fada242…` / ui `e862c0cd…` 异于 460，crash `21ba6f9a…` 同 460 ③0.9.4.0（ui 0.9.4）④冒烟见下 ⑤`Publish/config.toml` `da2be5da…` 三时点不变 ⑥90/9/17 ⑦正 `[STREAM-SV-463] preview model`1 / `decode win=`1 / `first_decode`1 / VOICEPRINT-460 1，反 PHANTOM-459 strip 0 / `[LocalRT-DBG-291] endpoint flush`0 / `streaming-paraformer-trilingual`0 / `LAS_DRAFT_LANG`0 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **模型**：🔴 新 sense-voice 整目录复制到 `Publish/models/`；`model.int8.onnx` sha16 `c71f0ce00bec95b0`、`tokens.txt` `f449eb28dc567533`（源 = Publish 一致）。1.7B / 对齐三件不变。旧模型目录（funasr-nano / streaming-paraformer）**全保留未删**。
- **④冒烟（如实）**：Publish 包 PID 17860 Responding=True、`SenseVoice model found at "...\Publish\models\sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17"`、无 panic/crash/残留。⚠️ Publish 的 config 为 `asr_model=performance`，不发 local-realtime 专属行 ⇒ 以**字节相同**的 `target/release` 二进制（config=local_realtime）补充实测 `LocalRealtime: dual recognizers loaded (preview SenseVoice + offline accuracy)` + `ctx=1024(max 4096)` + `[ALIGN-456] aligner attached`。`[STREAM-SV-463] decode` 行无麦克风语音未取到。
- **红线**：未改生产代码 / 版本未动 / 未 push / 旧模型未删 / 零凭证。报告 `collab/outbox/tester-1/result.md`。**Step1 已结束输入法进程 ⇒ 请主控转告 Gavin 重启。**

## 2026-10-03 — 主控 — PREVIEW-PUNCT-464 + KOJA-PUNCT-464 ✅ 开发完成（DEC-101）

- 预览标点一变就打、停顿 / 录音结束补句末标点；含假名 / 谚文不送标点模型（预览 + 最终节点）。
- 研究：SenseVoice 标点与 ITN 同一开关（withitn/woitn），不能单开标点。
- 更正：本地实时最终不会剥光 1.7B 标点（DEC-080 已摘该节点），向 Gavin 的误述已更正。
- 定向回归全绿（local_stream 103 / punct 146 / transcription 545 / overlay 145 / guard 175）；端到端全绿。预期 bin 1912P/0F/78I。
- 下一步：派 tester-1 TEST-EXEC-464 + BUILD-464。

## 2026-10-03 — tester-1 — TEST-EXEC-464 + BUILD-464 ✅（回归全绿 ⇒ 出包）

- **性质**：阶段四全量回归 + 阶段五出包（PREVIEW-PUNCT-464 + KOJA-PUNCT-464 / DEC-101）。HEAD `6c88581`，工作区 clean。版本 **0.9.4 未动**。
- **回归（全绿 0F）**：bin **1912P/0F/78I**（＝预期）/ root 2000P/0F/80I / src-tauri 92P/0F/0I / Vitest 7 files·100P·11skip / fmt EXIT 0。NEW 通过 3（`koja464_ct_unsupported_script` / `koja464_local_punct_checks_script_before_engine` / `fix464_punct_on_every_change_independent_of_silence`）+ 忽略 1、GONE 通过 1（`fix438_silence_1200ms_no_longer_triggers` 0 命中）、改写 3（punct438_preview_only_on_interval / fix438_interval_gate_refires_after_reset / fix438_source_guards）全绿。重点失效模式全绿、0 FAILED。
- **Step7 真模型 E2E**：`sv463_preview_freeze_e2e` **1P/0F/11.70s**（含「停顿派发补句末标点，PREVIEW-PUNCT-464」）+ `koja464_local_punct_real_engine` **1P/0F/0.26s**。
- **出包九项全 PASS**：①main 18:45:16 / ui 18:43:07 / crash 09-30 22:25（零改动）②两副本全等、main `289b59db…` / ui `47a17f25…` 异于 463，crash `21ba6f9a…` 同 463 ③0.9.4.0（ui 0.9.4）④冒烟见下 ⑤`Publish/config.toml` `da2be5da…` 三时点不变 ⑥90/9/17 ⑦正 `Local punctuation skipped: ja/ko text not supported by punctuation model`1 / `[STREAM-SV-463] preview model`1 / VOICEPRINT-460 1，反 PHANTOM-459 strip 0 / `streaming-paraformer-trilingual`0 / `LAS_DRAFT_LANG`0 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **模型**：本批无新增。sense-voice `model.int8.onnx` sha16 `c71f0ce00bec95b0` 未变；`Publish/models/sherpa-onnx-funasr-nano-int8-2025-12-30` 已按 Gavin 指示删除（未恢复，属正常）；其余旧目录（sense-voice-funasr-nano-2025-12-17 / streaming-paraformer-trilingual）保留。
- **④冒烟**：Publish 包 PID 27288 Responding=True + `SenseVoice model found at Publish\models\…`；target/release 本地实时档 PID 22760 Responding=True + `dual recognizers loaded (preview SenseVoice + offline accuracy)` + `ctx=1024(max 4096)` + `[ALIGN-456] aligner attached`；无 panic / 无 crash.json / 零残留。
- **红线**：未改生产代码 / 版本未动 / 未 push / 未删用户数据与模型文件 / 零凭证。报告 `collab/outbox/tester-1/result.md`。**Step1 已结束输入法进程 ⇒ 请主控转告 Gavin 重启。**

## 2026-10-03 — 主控 — PUNCT-JAKO-466 ✅ 开发完成（DEC-102）

- 子项 466-1~466-6、466-8 完成（见 todo）；466-7 派 tester-1；466-9 结论入 DEC 后清理临时环境。
- 日韩标点：int8 59MB，首次判出日韩文才加载（最终输出同步 ≈0.5s 一次；预览后台加载不卡）；本地实时 + performance 同一最终节点。
- 新依赖 `ort =2.0.0-rc.13`（load-dynamic，复用 onnxruntime 1.28.2）；新模型目录需随包（Publish/models 复制）。

## 2026-10-03 — tester-1 — TEST-EXEC-466 + BUILD-466 ✅（回归全绿 ⇒ 出包）

- **性质**：阶段四全量回归 + 阶段五出包（PUNCT-JAKO-466 / DEC-102）。HEAD `015450e`。版本 **0.9.4 未动**。
- **回归（全绿 0F）**：bin **1916P/0F/79I**（＝预期）/ root 2004P/0F/81I / src-tauri 92P/0F/0I / Vitest 7 files·100P·11skip / fmt EXIT 0。NEW 通过 4（`punctuation::jako::tests`）+ 忽略 1、GONE 0。重点失效模式（jako466/koja464/fix464/fix438/guard463/ts393/guard346/ts405b/t445/t456/t457/partial_reflow_452）全绿、0 FAILED。
- **Step7 真模型**：`jako466_real_model` **1P/0F/0.56s**（加载 519ms，韩/日打点）+ `koja464_local_punct_real_engine` **1P/0F/0.86s**（ko/ja/zh）+ `sv463_preview_freeze_e2e` **1P/0F/12.17s**。
- **出包九项全 PASS**：①main 19:40:10 / ui 19:37:45 / crash 19:38:51 ②两副本全等、main `ca0fb60b…` / ui `8113b7e8…` / crash `d427ccb3…`（crash 源码零改动，因新增 `ort` 依赖重链接而同 crate bin 变化）③0.9.4.0（ui 0.9.4）④冒烟见下 ⑤`Publish/config.toml` `da2be5da…` 三时点不变 ⑥90/9/17 ⑦正 `[PUNCT-JAKO-466] ja/ko punctuation model loaded on first use`1 + `Local ja/ko punctuation applied`1 + `[STREAM-SV-463] preview model`1，反 PHANTOM-459 strip 0 / `streaming-paraformer-trilingual`0 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **模型 / 运行库**：新 `punct-cap-seg-47lang-int8` 复制到 `Publish/models/`，model.int8.onnx sha16 `fa630cad87cdf398`、spe_unigram `1bc15b6e5fd80dfa`（源=Publish）；旧目录全保留。🔴 onnxruntime 仅原有 2 文件（`onnxruntime.dll` 17,136,128 B + `onnxruntime_providers_shared.dll` 10,752 B，两处 sha 等），无第二份。
- **④冒烟 + 延迟加载核验**：Publish PID 1332（`SenseVoice model found`）+ target/release PID 21132（`dual recognizers loaded (preview SenseVoice + offline accuracy)` + `ctx=1024(max 4096)` + `aligner attached`）；无 panic / 无 crash.json / 零残留。🔴 **两处冒烟日志 grep `[PUNCT-JAKO-466] ja/ko punctuation model loaded` = 0**（启动未加载日韩标点模型）。
- **红线**：未改生产代码 / 版本未动 / 未 push / 未删用户数据与模型文件 / 零凭证。报告 `collab/outbox/tester-1/result.md`。**Step1 已结束输入法进程 ⇒ 请主控转告 Gavin 重启。**

## 2026-10-03 — 主控 — MEM-469 / MEM-ENC-470 / UI-HINT-471 / UI-REC-472 开发完成，UI-PREVIEW-473 取证中

- 469：内存 / 显存分项见 `collab/evidence/469/`；470：编码器不预留（DEC-103），A/B 证据 `collab/evidence/470/`。
- 471：提示窗宽度按文字自适应；472：本地实时直接「请说话...」、未就绪只给加载提示（DEC-104）。
- 473：离屏真绘制两种路径字号字形相同，待 Gavin 截图；四单待合包出包（473 未定前不拆包，等 Gavin 定）。

## 2026-10-03 — 主控 — UI-PREVIEW-473 开发完成（DEC-105）；470~473 合包派 tester-1

- 截图取证：运行时走 D2D、字体相同，差在渲染模式（竖笔画发虚）；修：显式 GDI 经典渲染模式 + 字号 18px。
- 四单（470 / 471 / 472 / 473）全部开发完成 ⇒ 合包派 tester-1 回归 + 出包（BUILD-473）。

## 2026-10-03 — tester-1 — TEST-EXEC-473 + BUILD-473 ✅（回归全绿 ⇒ 出包）

- **性质**：阶段四全量回归 + 阶段五出包（MEM-ENC-470 / UI-HINT-471 / UI-REC-472 / UI-PREVIEW-473）。HEAD `a8ee634`。版本 **0.9.4 未动**。
- **回归（全绿 0F）**：bin **1925P/0F/82I**（＝预期）/ root 2013P/0F/84I / src-tauri 92P/0F/0I / Vitest 7 files·100P·11skip / fmt EXIT 0。NEW 通过 9（`enc470_tests::t470_…` + `ui471_472_tests` 8 条）+ 忽略 3、GONE 0。重点失效模式（ui47/t470/overlay148/d2d25/t450/t456/t457/sv463/guard463/jako466/koja464）全绿、0 FAILED。⚠️ 任务书 `fix164*` 0 命中（无此测试名，如实上报）。
- **Step7 真模型**：`sv463_preview_freeze_e2e` **1P/0F/12.13s** + `ui473_d2d_streaming_text_real_dc` **1P/0F/0.12s**（`[473]` 渲染模式 DWRITE_RENDERING_MODE(0)、gamma 2.2、左半区差异像素=0）。
- **出包九项全 PASS**：①main 23:58:51 / ui 23:56:17 / crash 23:57:24 ②两副本全等、main `4e3357aa…` / ui `8be3f375…` / crash `9340ab31…` 均异于 466 ③0.9.4.0（ui 0.9.4）④冒烟见下 ⑤`Publish/config.toml` `da2be5da…` 三时点不变 ⑥90/9/17 ⑦正 `LAS_ENC_WARMUP`1 / `LAS_LOG_INFO`1 / `[ALIGN-456] forced aligner loaded`1 / `[STREAM-SV-463] preview model`1，反 PHANTOM-459 strip 0 / `streaming-paraformer-trilingual`0 ⑧四 toml 三副本全等 ⑨VC 五件 == Redist 14.44.35112。
- **模型（如实）**：本批无新增 / 无改动。`Publish/models` 现 8 个目录；较 466 减少的 opus-mt-en-zh / opus-mt-zh-en / sense-voice-funasr-nano-2025-12-17 / streaming-paraformer-trilingual 由 **MODEL-CLEAN-467**（Gavin 确认，commit `2f0d354`）删除，**非本单操作**；`test_wavs` 未重新出现；onnxruntime 仅原有 2 文件。
- **④冒烟**：Publish PID 21548 Responding + `SenseVoice model found`；target/release PID 16320 Responding + `[ACC-452] engine loaded … in 1198ms` + `dual recognizers loaded (preview SenseVoice + offline accuracy)` + `aligner attached`；无 panic / 无 crash.json / 零残留。
- **红线**：未改生产代码 / 版本未动 / 未 push / 未删用户数据与模型文件 / 零凭证。报告 `collab/outbox/tester-1/result.md`。**Step1 已结束输入法进程 ⇒ 请主控转告 Gavin 重启。**

## 2026-10-04 — 主控 — BUILD-473 验收通过

- tester-1：回归全绿（bin 1925P/0F/82I）、九项全 PASS，main `4e3357aa…`，0.9.4。
- 待 Gavin 重启后目视：预览字号 / 清晰度（473）、本地实时开录直接「请说话...」与加载中不弹录音窗（472）、加载提示完整（471）。
- 主控更正：任务书 `fix164*` 测试名前缀写错（0 命中），下次按实际测试名列。

## 2026-10-04 — 主控 — UI-FONT-474 开发完成（DEC-106），派 BUILD-474

- 预览文字改 GDI（= 编辑框字体与渲染），窗框仍 D2D；浅色尾巴两段裁剪；删 D2D 预览文字与 DWrite 量宽缓存。
- 预期 bin 1924P/0F/82I（NEW 1 + 1I；GONE 2 + 1I）。

