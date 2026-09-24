# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-24 归档：2026-09-23 共 35 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-23 归档：2026-09-22 共 31 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

## 2026-09-24 — coder-1 — RELEASE-ISS-FROM-PUBLISH-400 ✅ 阶段一交付（待主控验收）

- **性质**：只改安装脚本与文档，**未改 Rust/Cargo/tauri.conf 版本，未构建、未编安装包**。
- **`.iss` 改动**：`[Files]` 全部改从 `..\Publish\`（17 条 Publish + 2 条 assets，显式白名单无通配；含三 exe、sherpa×2、onnxruntime×2、`ctranslate2.dll`、五运行库、四规则表）；删弃用 paraformer；排除 `models\` + 用户/运行时数据 + 开发脚本 + `cudnn64_9.dll`/`libiomp5md.dll`（dumpbin 证无导入）；`MyAppVersion`→0.9.3、`MyAppExeName`→`feiyin-ime.exe`、`MyAppId` 未动。两份 sha256 全等 `4448d136…`。
- **验证**：Source 实存检查 **19/19 OK**（`collab/evidence/400/source_check.txt`）；规则表 exe 同级读取行号已给；两份 `.iss` 逐字相同。
- 🔴 **模型下载能力核查（只查不改，缺口单列）**：程序**无任何自动下载器**；七模型/组件均不能自动下载、仅 Accuracy 有 UI 引导（`src-tauri/src/main.rs:104-129`→`Voice.tsx:270`）；paraformer/VAD/punct 连 URL 都没有。安装包不带模型 ⇒ 干净安装后离线档/翻译在用户手动补模型前不可用。详见 result.md §七。
- **偏差**：`.iss` UTF-8 无 BOM（未改编码）；`MinVersion=6.1`（Win7）与 DEC-000（Win10+）矛盾、`[UninstallRun]` 未杀 `feiyin-ime-ui.exe`——均未改，已报。
- 红线：未 commit / 未 push / 未构建 / 版本号（Rust/Cargo/tauri.conf）未动 / 零凭证。

## 2026-09-24 — coder-1 — RELEASE-VCRT-APPLOCAL-399 ✅ 阶段一交付（待主控验收）

- **性质**：纯脚本 / 安装脚本 / 文档，**不改 Rust 源码 / Cargo / 版本号 / 不构建**。
- **最小清单**（`dumpbin /dependents` 全量复核）：`MSVCP140.dll` / `MSVCP140_1.dll` / `VCRUNTIME140.dll` / `VCRUNTIME140_1.dll` / `VCOMP140.DLL`；`api-ms-win-crt-*` 属 UCRT 不带。证据 `collab/evidence/399/dumpbin_all.txt`。
- **`scripts/init-publish.ps1`**：新增 Step 2（`Copy-VcRuntime`，原 Step2/3/4 顺延），vswhere 动态定位 VS Redist、不写死版本号、缺件 `exit 1`；拷到 `Publish/` + `target/release/`；新增 `-RuntimeOnly` 开关；🔴 补写 UTF-8 BOM（原文件实测无 BOM）。
- **`.iss` 两份**：`[Files]` 补 `ctranslate2.dll` + 五个运行库；两份 sha256 全等 `ce37f315…`。
- **`build-test-guide.md`**：Step 4 增小节 + 出包核验 **八项→九项**（⑨）。**`decisions.md`/`decisions-archive.md`**：DEC-085 索引 + 全文。**`docs/MACOS-HANDOFF.md`** §0.7（纯 Windows 打包，macOS 无影响）。
- **实证**：`-RuntimeOnly` 实跑，源 / `Publish` / `target/release` 三处五个 DLL sha256 逐一 MATCH（`collab/evidence/399/{init-publish-runtimeonly.txt,runtime_sha.txt}`）；PowerShell `SYNTAX OK`、首三字节 `efbbbf`。
- ⚠️ **任务书偏差（如实上报）**：任务书称 `init-publish.ps1` 是 UTF-8 **with BOM**，实测**无 BOM**（`23 20 69`）；已补 BOM。另发现既有问题（未修）：`.iss` 主程序 `Source` 指向 `voice-ime.exe`，实际产物名 `feiyin-ime.exe`（RELEASE-210 1b，等 Gavin）。
- 红线：未 commit / 未 push / 未构建 / 版本号未动 / 零凭证。

## 2026-09-24 — coder-1 — TRANS-CT2-DNNL-THREADS-397 ✅ 阶段一交付（待主控验收）

- **改动文件**：`Cargo.toml`（Windows 目标依赖追加 `ctranslate2-sys` 的 `dnnl`+`openmp-runtime-comp`）、`patches/ctranslate2-sys/build.rs`（补丁 2 处）、`src/translation/mod.rs`（`nllb_num_threads()` + 接线 + bench）。
- **build.rs 补丁**：① `link()` 编完 oneDNN 后把 `OUT_DIR` 前插 `CMAKE_PREFIX_PATH` 再 `build_native()`（CT2 的 `WITH_DNNL` 只在 `${INTEL_ROOT}/oneapi/dnnl/latest/cpu_gomp` 找，与 build-support 装到 `OUT_DIR` 不通；仅 dnnl 特性生效）；② shared 分支把 DLL 同时拷 `<target>/deps/`（修 `[CT2-DLL-SHADOW-397]`）。
- **线程**：`nllb_num_threads()` = `available_parallelism().min(8)`、回落 4（与识别侧 `default_acc_num_threads()` 同口径，Gavin 定 8）；`NllbModel::new` 传 `num_threads_per_replica`（原 0⇒4）；加载日志打 `(threads=N)`；生产不读 env/config。
- **实测（debug bench 中位数 ms）**：基线（旧 DLL 4 线程）S1 6114 / S2 13916 / S3 17650 / S4 30470；新 DLL 8 线程（生产）S1 392 / S2 1189 / S3 1322 / S4 2824 ⇒ **15.6× / 11.7× / 13.3× / 10.8×**（≥3× 目标）。4/8/16 全表 `collab/evidence/397/threads_new.txt`；`CT2_VERBOSE=1` 实证 `GEMM_S8 backend: DNNL` / `int8_float32` / `ISA AVX2`；导入表只多 `VCOMP140.DLL`、无 dnnl.dll。重译 0 次，无漏句/截断。
- **回归**：`cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **92/87** = 基线；全量 `cargo test --no-fail-fast` **1685P/0F/46I**（+1 ignored = `bench397_real_model`）；macOS `cargo tree` 无 dnnl/openmp。
- **新增 troubleshooting**：`[CT2-DLL-SHADOW-397]`（archive 全文 + 索引一行）。
- 🔴 **交班 tester-1 注意**：① DLL 依赖新增 `VCOMP140.DLL`（VC++ 2015-2022 Redist 自带，与 MSVCP140/VCRUNTIME140 同包）；CT2 DLL 体积 8.2MB → 27.4MB；② 出 release 时 oneDNN 会按 Release 重编（dev 档 bench 是 Debug oneDNN，release 只会更快/相近）；③ 首次 release 构建 ctranslate2-sys 会多编 oneDNN（约 +2~3 分钟，本机 debug 实测 3m41s）。
- 红线：未 commit / 未 push / 未出包 / 版本号未动 / 未 `cargo clean` / 零凭证。

## 2026-09-24 — tester-1 — TEST-EXEC + BUILD-398 ✅ 出包（397+398 合包；八项 + 专项 A~D + 三特殊点全 PASS；回归与构建并行）

- **交付源码**：HEAD `9686621`（`678c7a0` 397 + `9686621` 398），工作区 clean，版本 0.9.3。单：`TRANS-CT2-DNNL-THREADS-397`（CT2 换 oneDNN + OpenMP、NLLB 线程 `min(核,8)`）+ `LOCALRT-TERMS-PREFIX-398`（本地实时路B 恢复词库注入 `Technical terms: a, b, c.` + 前缀回显剥离，推翻 DEC-083 立 DEC-084）。
- **回归（与构建并行）**：root `cargo test --no-fail-fast` **1685P/0F/46I**（EXIT 0；11 二进制）；`src-tauri` **92P/0F/0I**；`cargo fmt --check` **EXIT 0**；Vitest **SKIP**（`ui/` 无 diff）。NEW/GONE 基线 **BUILD-395 `1a8f00d`**（1682P/45I）：**+8P**（`fix398_*` 4 + `ctx398_*` 4）**+1I**（`bench397_real_model`）**−5P**（`fix377_build_ctx_system_is_bare_terms_only` + `ctx377_*` 4）⇒ **1685P/46I**。🔴 `collab/evidence/397/full_test.log` 实为 398 状态（1685P/46I、含 `fix398`、无 `fix396`），**非可用基线**（397+398 同树跑后拆 commit）。
- **构建**：Step1 残 0 → npm 1.47s + Tauri 3m16s(17w) → Step2c UI cp(`ee7f2571…`) → main 7m57s(92w + crash 9w，含 CT2 oneDNN release 编译) → Step4 三 exe + **ctranslate2.dll** + 四 rules toml → Publish。产物 main `15c3ec9f853e…`(14,934,528B/13:45:05) / ui `ee7f2571f1d4…`(10,050,048B/13:45:21) / crash `c6a38d5b9d49…`(24,879,104B/13:43:19)；两副本全等、三者均异于 BUILD-396。
- **八项逐项 PASS**：①时间戳 13:37–13:45 ②两副本 sha 相等且异于上包 ③**0.9.3** ④冒烟 PID **24488 Responding=True** / 无新 crash.json / 无新 panic / `WM_CLOSE`→controller **335ms** 退出 / 残 0 ⑤config `da2be5da…` 不变 ⑥warnings 92/87/17/9（+ `ctranslate2-sys` 18=397 基线）⑦正探针 `Technical terms: `=1、`(threads=`=1；反探针 `initialized at {}"`=0、`Terms:`=0 ⑧scene/itn/homophone/wordbook 四表三副本全等。
- **专项 A~D**：A 新 DLL `b7a5e2c1…` 27,405,312B，Publish==target/release，≠旧 `e28b44b8…` 8,168,448B；B pefile 导入 OLD 15→NEW 16，**仅多 `VCOMP140.DLL`、无 `dnnl.dll`**；C `target/release/deps/ctranslate2.dll`==`target/release/ctranslate2.dll`；D 冒烟 `-debug` 真实日志 **`NLLB CT2 model initialized at … (threads=8)`**（16 核 ⇒ 8）。
- **三特殊点（仅核验）**：① sherpa 四 DLL 三副本全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本 `60b227de…` 全等；③ `Publish/models/` 1.7B 七文件与源 sha256 全等、0.6B 保留。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启 + 带 `-debug` 端测**。未 push / 版本号未动 / 未改生产代码 / 未 `cargo clean` / 零凭证。证据 `collab/evidence/398/`。

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

## 2026-09-24 — coder-2 — LOCALRT-TERMS-PREFIX-398 ✅ 交付（阶段一·推翻 DEC-083，恢复路B 注入 + `Technical terms:` 固定前缀）

- **需求**（Gavin 2026-09-24）：「我建议我们也在词条前加 Technical terms」「一定要按照你上次在网上找到的那个起作用的方法来传」「修改这么简单，就直接改了吧」（明示豁免 DEC-059 A/B，且不派阶段三）。
- **改动（仅 `src/main.rs` + `src/transcription/mod.rs`，平台中立）**：① `load_hotwords_for_accuracy` 删除 `AsrModel::LocalRealtime` 早退，与 Accuracy 同走 `uses_accuracy_engine()` 门（恢复读词库 / 改词库重新触发 1.7B 重载）；② `build_ctx_system` 输出 `Technical terms: <词条, 连接>.`（按 `,` 切分逐条 `trim`、丢空段；空词表 `None`；`build_hotwords_string` 未动）；③ 抽取 `find_best_terms_run`（374 共用，`strip_terms_echo` 逻辑不变）+ 新增 `strip_technical_terms_echo`（前缀回显剥离；护栏：前缀后须有 ≥1 真词条），接入 `apply_acc_disposition` 3 处（首剥 / 重解再剥 / `still_echo`）。
- **单测**：更新因行为变化变红 8 条（`fix377_build_ctx_system_is_bare_terms_only`→`fix398_build_ctx_system_technical_terms_prefix`、`fix377_terms_echo_still_hits_new_format`、testsync377 5 条 `ctx377_*`→`ctx398_*`/`echo374_new_format_run_is_stripped`、`fix396_*`→`fix398_tests`）；新增 3 条（`fix398_build_ctx_system_spec_example` / `fix398_technical_terms_prefix_echo_stripped` / `echo374_new_format_run_is_stripped` 的 398 前缀断言）。
- **验证**：`cargo fmt --check` **EXIT 0** ｜ `cargo check --all-targets` **0 error**、warnings **92/87** = 基线 ｜ 全量 `cargo test --no-fail-fast` **0 failed**（bin **1597P/44I**）。
- **文档**：`decisions.md`（DEC-083→已推翻 + 新增 DEC-084 索引行）、`decisions-archive.md`（DEC-084 全文）、`docs/MACOS-HANDOFF.md`、`logs/20260924.md`、`CHANGELOG.md`、`progress.md`。
- 🔴 **未验证**：中文场景端测（词库词召回 / 是否仍念词表 / `Technical terms` 是否漏进正文 / 空输出 / 截尾）交 tester-1 / Gavin。
- **未改版本 / 未 commit / 未 push / 未 build release / 零凭证**。
