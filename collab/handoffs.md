# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-24 归档：2026-09-23 共 35 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-23 归档：2026-09-22 共 31 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

## 2026-09-24 — coder-1 — TRANS-CT2-DNNL-THREADS-397 ✅ 阶段一交付（待主控验收）

- **改动文件**：`Cargo.toml`（Windows 目标依赖追加 `ctranslate2-sys` 的 `dnnl`+`openmp-runtime-comp`）、`patches/ctranslate2-sys/build.rs`（补丁 2 处）、`src/translation/mod.rs`（`nllb_num_threads()` + 接线 + bench）。
- **build.rs 补丁**：① `link()` 编完 oneDNN 后把 `OUT_DIR` 前插 `CMAKE_PREFIX_PATH` 再 `build_native()`（CT2 的 `WITH_DNNL` 只在 `${INTEL_ROOT}/oneapi/dnnl/latest/cpu_gomp` 找，与 build-support 装到 `OUT_DIR` 不通；仅 dnnl 特性生效）；② shared 分支把 DLL 同时拷 `<target>/deps/`（修 `[CT2-DLL-SHADOW-397]`）。
- **线程**：`nllb_num_threads()` = `available_parallelism().min(8)`、回落 4（与识别侧 `default_acc_num_threads()` 同口径，Gavin 定 8）；`NllbModel::new` 传 `num_threads_per_replica`（原 0⇒4）；加载日志打 `(threads=N)`；生产不读 env/config。
- **实测（debug bench 中位数 ms）**：基线（旧 DLL 4 线程）S1 6114 / S2 13916 / S3 17650 / S4 30470；新 DLL 8 线程（生产）S1 392 / S2 1189 / S3 1322 / S4 2824 ⇒ **15.6× / 11.7× / 13.3× / 10.8×**（≥3× 目标）。4/8/16 全表 `collab/evidence/397/threads_new.txt`；`CT2_VERBOSE=1` 实证 `GEMM_S8 backend: DNNL` / `int8_float32` / `ISA AVX2`；导入表只多 `VCOMP140.DLL`、无 dnnl.dll。重译 0 次，无漏句/截断。
- **回归**：`cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **92/87** = 基线；全量 `cargo test --no-fail-fast` **1685P/0F/46I**（+1 ignored = `bench397_real_model`）；macOS `cargo tree` 无 dnnl/openmp。
- **新增 troubleshooting**：`[CT2-DLL-SHADOW-397]`（archive 全文 + 索引一行）。
- 🔴 **交班 tester-1 注意**：① DLL 依赖新增 `VCOMP140.DLL`（VC++ 2015-2022 Redist 自带，与 MSVCP140/VCRUNTIME140 同包）；CT2 DLL 体积 8.2MB → 27.4MB；② 出 release 时 oneDNN 会按 Release 重编（dev 档 bench 是 Debug oneDNN，release 只会更快/相近）；③ 首次 release 构建 ctranslate2-sys 会多编 oneDNN（约 +2~3 分钟，本机 debug 实测 3m41s）。
- 红线：未 commit / 未 push / 未出包 / 版本号未动 / 未 `cargo clean` / 零凭证。

## 2026-09-24 — tester-1 — BUILD-396 ✅ 出包（小改动短路径；八项 + 三特殊点全 PASS）

- **交付源码**：HEAD `5218c92`（= 任务书 `660378a` + docs-only），工作区 clean，版本 0.9.3。单 `LOCALRT-NO-HOTWORDS-396`（DEC-083）：本地实时 B 路径 1.7B 滑窗精解**不再注入词库**（`load_hotwords_for_accuracy` 对 `AsrModel::LocalRealtime` 早退 `None`；Accuracy/在线/LLM 用词库不变）。**Gavin 指示：小改动精确范围 ⇒ 不跑全量回归**。
- **简单验证**：`cargo fmt --check` **EXIT 0**；`cargo test --bin feiyin-ime -- fix396` **1P/0F**（`fix396_local_realtime_returns_none_before_wordbook_read`）。
- **BUILD-396**：Step1 残 0 → Step2 npm 635ms + Tauri 1m35s（17w）→ Step2c UI cp（00:46 / `34bb0e0c…`）→ Step3 主程序 2m53s（92w + crash 9w）→ Step4 三 exe→Publish。产物 main `1fa081c4bde4…`（14,920,192B/00:49:59）/ ui `34bb0e0c7b9c…`（10,050,048B/00:46:59）/ crash `5579119aa19d…`（24,879,104B/00:48:08）；两副本全等、三者均异于 BUILD-395。
- **八项逐项 PASS**：①时间戳 00:46–00:49 ②两副本 sha 相等且异于上包 ③**0.9.3** ④冒烟 PID **14220 Responding=True** / 无新 crash.json / 残 0 ⑤config `da2be5da…` 不变 ⑥warnings 92/87/17/9 ⑦**探针 N/A（不可构造）→ 三证**（源码引用+时间戳+sha 异于上包）⑧两 toml 三副本全等。
- **模型（无变化，只核）**：`target/release/models` 仍 Junction；`Publish/models` silero `1a153a22…` + NLLB 四文件 sha 与 BUILD-395 一致；`opus-mt-*` 未删。
- **三特殊点（仅核验）**：① dll 四张三副本全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本全等；③ 1.7B 七文件与源全等、0.6B 保留。
- 🔴 **冒烟正常退出**：对 `voice-ime-controller-window` 投 `WM_CLOSE`（非强杀）⇒ **1s 内消失**、无 crash.json。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启 + 带 `-debug` 端测**。红线：未 push / 版本号未动 / 未改生产代码 / 零凭证。

## 2026-09-24 — tester-1 — TEST-EXEC + BUILD-395 ✅ 出包（393/394/395 合包；十项 + 三特殊点全 PASS；含模型同步与「退出不卡死」实证）

- **交付源码**：HEAD `1a8f00d`（= 任务书 `240042b` + docs-only），工作区 clean，版本 0.9.3。核心单 393（VAD v6.2 全管线 + 时间线复用）/ 394（NLLB-200-distilled-600M int8 + 逐句批量 + 漏译重译 + 模型进程级常驻永不析构）/ 395（翻译热键页文案三 locale）。DEC-081：任务书已下达「现在可以出包」。
- **阶段四（全量未过滤；fmt `--check` 不带 `skip_children`）**：root `cargo test --no-fail-fast` **1682P/0F/45I**（EXIT 0；`feiyin-ime` bin **1594P/43I**）；`src-tauri` **92P/0F/0I**；**Vitest 7 files/100P/11S/0F**；warnings **92/87/17/9** ≤ 基线；fmt **EXIT 0**。
- **NEW/GONE（精确集合差，基线 `ad08251`）**：净 **+25P/+7I**。NEW **57**（50P+7I）；GONE **25**（全为 opus-mt 专有：`segment_text_*` 7 / 旧解码参数 7 / metaspace 2 / 双目录文件清单 6 / 旧分段行为 3，属预期）。🔴 **`derive_target_*` 五条仍在且全绿**（DEC-082）。
- **重点失效模式**：其他管线（`build_padded`/`naive_chunk`/20s 快照/`segment`）、`vad391`/`ts391`/`ts393`/`ts393c`/`timeline393`、`gate392`/`seg389`/`fix388`/`trim388`/`fix390`/`guard346`、`translation::tests`（含 `s394_*`、`derive_target_*`）共 **107 条 ok、0 失败**，无停手。
- **BUILD-395**：Step1 残 0 → Step2 npm 667ms + Tauri 1m17s（17w）→ Step2c UI cp（00:13 / `61cc642c…`）→ Step3 主程序 2m43s（92w + crash 9w）→ Step4 三 exe→Publish。产物 main `42e16d64889b…`（14,920,192B/00:16:10）/ ui `61cc642c0881…`（10,050,048B/00:13:21）/ crash `3b54e58f9d2a…`（24,879,104B/00:14:26）；两副本全等、三者均异于 BUILD-392。
- **十项逐项 PASS**：①时间戳 00:13–00:16 ②两副本 sha 相等且异于上包 ③**0.9.3** ④冒烟 PID **10112 Responding=True** / 无新 crash.json ⑤config `da2be5da…` 不变 ⑥warnings 92/87/17/9 ⑦正探针 `[LocalRT-DBG-388] trim:`=2/`source=`=2/`timeline fallback`=1/`NLLB model loaded once`=1，反探针 `opus-mt-zh-en`=**0** ⑧两 toml 三副本全等 **⑨两处模型 sha 全等** **⑩`target/release/models` 仍为 Junction**。
- **模型同步**：`Publish/models/silero-vad` v4→v6.2 覆盖（`1a153a22…`）+ 新建 NLLB 四文件（sha 与源逐一相等）；`target/release/models` 只核联接与 sha（未复制，防写穿源）；`opus-mt-*` **未删**。
- **三特殊点（仅核验）**：① dll 四张三副本全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本全等；③ `Publish/models/` 1.7B 七文件与源 sha256 全等、0.6B 保留。
- **UI 探针**：`说中文译为英文` 在 `ui/dist/assets/index-DGCpO3OF.js` =1。
- 🔴 **冒烟「退出不卡死」实证**：`Publish/feiyin-ime.exe` 正常退出（对 controller 窗口投 `WM_CLOSE`，**非强杀**）⇒ 进程 **1s 内消失**、无 crash.json ⇒ **394 通过**。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启 + 带 `-debug` 端测**。红线：未 push / 版本号未动 / 未改生产代码 / 未 `cargo clean` / 零凭证。

## 2026-09-24 — tester-1 — TEST-SYNC-394 ✅ 交付（阶段三 · 非作者护栏 8 条；生产零改动）

- **性质**：阶段三 TEST-SYNC，给 `TRANS-NLLB-AND-SENTENCE-BATCH-394`（+返工 R1~R3）按**设计契约**补独立护栏。只改 `src/translation/mod.rs` 的 `#[cfg(test)]` 区；**未碰** `ui/src/i18n/*`（coder-2 TRANS-COPY-395）；**未跑 `cargo test`/`build`**（白名单）。**每条期望值手算写进注释**（上一单 `ts393c_lens_shorter` 期望值算错的教训）。
- **新增 8 条**：不丢字不变式（4 组，手算句数 6/8/1/2）/ 中文单 `…` vs `……` + `Node.js 3.14` 不因 `.` 断 / 英文 `no.` 数字后置条件（`ends_with_abbreviation` 直测）/ `split_and_merge` 全短合成 1 句==原文 / `finalize_sentence` 三态（calls 1/1/0）/ `strip_target_prefix` 非目标不丢首词 / `join_parts` 空串无多余空格 + 英→中直连 / 源码护栏（无 `Arc<NllbModel>`/`mem::forget`；字段 `&'static NllbModel`；`NllbModel::new(...)?` 在 `*slot = Some(` 之前）。
- **验证（白名单）**：本文件 `rustfmt --config skip_children=true --check` **CLEAN**；**全仓** `cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **EXIT 0**、0 error，warnings **bin 92 / test 87 ≤ 97/88**；`numstat == -w`（232/0）。🔴 **未跑 `cargo test`**（首跑阶段四）。
- **红线**：未改生产代码 / 未 commit / 未 push / 版本号未动 / 零凭证。
