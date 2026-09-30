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
