# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-24 归档：2026-09-23 共 35 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-23 归档：2026-09-22 共 31 条已移入 `handoffs-archive.md`（本文件曾达 405 行，超 200 行上限）。
> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

## 2026-09-24 — coder-1 — TEST-SYNC-406-408B ✅ 交付（阶段三·非作者护栏 16 条；生产零改动）

- **范围**：只在 `mod.rs` / `speaker.rs` 的 `#[cfg(test)]` 加两个模块 —— `testsync406_408b_tests`（10）+ `testsync408b_guard_tests`（6）；未碰生产 / `local_stream.rs` / `main.rs`。
- **406 契约**：门限常量/边界(0.5·0.6)/归一(全半角·大小写·标点)/同音错字放行·零重合幻觉拒/精解更短 0.6~0.8/after_drop 逐位一致+下限 0.2/未闭合标签剥+4 负例。
- **408B 契约**：多档最高分/无就绪·无 emb·<2s 保留/`partition_ranges` 缺位保守/新语种 <0.3 不收/`new_slice_from` 源码锚点/v1→v2 迁移·模型不符重建·坏档不 panic。
- **契约 7**（有剔除 ∧ ja|未知|未就绪 ⇒ 原 ranges 重解）内联需模型 ⇒ **源码锚点护栏**锁死形状。
- **白名单**：`rustfmt` 两文件 **EXIT 0 CLEAN**；`cargo check --all-targets` **0 error**、warnings **92/87 = 基线**；**未跑 cargo test**。**未发现生产缺陷**。
- ⚠️ 过程如实上报：一度对 `MismatchVerdict`（无 Debug）用 `{:?}` 致 13 处错误阻塞 coder-2 ⇒ 改只打字段 + 修模块重名后 check 恢复 0 error（**未给生产加 derive(Debug)**）。
- **返修（主控退回一处，只改测试）**：`ts408b_partition_ranges_conservative` 原按「32000 样本=1s」（应 =2.0s@16k）⇒ 期望修正为 kept=4.0/dropped=2.0，并改用 `SAMPLE_RATE` 换算、不硬编码样本数。`rustfmt` EXIT 0、`cargo check --all-targets` 0 error/warnings 92/87=基线。
- 红线：未改生产 / 未 commit / 未 push / 版本未动 / 零凭证。

## 2026-09-24 — coder-1 — LOCALRT-TAIL-WINDOW-407 ✅ 阶段一交付（待主控验收）

- **触发（`local_stream.rs`）**：`LONG_SILENCE_TAIL_MS=1900` + 纯函数 `should_signal_long_silence` + 新回调 `on_long_silence(pcm_pos)`；读 `acc_silent_ms`，每段静默只发一次、恢复说话复位（L133/137/972/1070/1188/1558）。
- **处理（`main.rs`）**：`enum AccInput{Slice,LongSilence(usize)}`；有 pending ⇒ 末尾窗（前片回溯 2s 找字缝 + pending，无字缝回落 2s 标 no_gap）→ 投解码 → 清 pending；无 pending 不动作。新增 `dispatch_tail_window!` + 纯函数 `tail_window_span`。
- **对齐**：末尾窗交 `ordered.push_window`（span `(p-1,p+1)`），测试证明不重不漏；`ALIGN_MIN_OVERLAP_CHARS=8`（后缀 <8 字落宁重复兜底）。
- **`vad.rs`（主控许可）**：`find_gap_cut` 提 `pub(crate)` + 拆内核；新增 `find_gap_cut_gap_only`；判据只一处。
- **日志**：`[LocalRT-DBG-407] long silence 1900ms: pending=yes|no` / `tail window: … gap=found|no_gap …`（Debug 守卫）。
- **测试**：`ts407_*` 2 + `tw407_*` 2 + 真模型 `poc_tailwindow_407`；更新 `counters_are_pushed_together`（2 处 push，强度不放宽）。`fmt` EXIT 0；`check --all-targets` 0 error/warnings 92/87=基线；全量 `cargo test` **1601P/0F/49I**。证据 `collab/evidence/407/`。
- **R1（主控退回，只改 main.rs）**：固定回溯 2s 慢速下 <对齐门 8 ⇒ 接缝重复；改**按字速自适应** `max(2.0,(8+4)/rate)`（rate=本次产出率均值，冷启动 3.0 字/秒，封顶整前片）；日志加 `backtrack_secs`/`rate_cps`；补测试 `tw407_backtrack_...`（2/3/6 字/秒 ⇒ 12 字）。验证 `tail_window_407` 3P/0F、`check --all-targets` 0 error/warnings 92/87、`fmt --check` EXIT 0。
- 红线：未改 1200ms 常规派发/常规滑窗/406 守卫/近场门；未 commit/未 build release/版本未动/零凭证。

## 2026-09-24 — coder-1 — SPEAKER-VERIFY-408A ✅ 阶段一交付（待主控验收）

- **新增 `src/transcription/speaker.rs`**（平台中立，**未接入管线**）：`SpeakerVerifier::load/embed`、`cosine`、`Voiceprint`（自动注册 ≥12s/≥3 段 + 离群剔除 `cos<0.5`；漂移 EMA `α=min(secs/(total+secs),0.25)` 仅 `score≥0.75` 段）、`judge`/`SegVerdict`（`<2s`/`ja·未知`/未就绪 保留；`score<0.45` 才剔除）。
- **不改 `mod.rs`**：`src/bin/poc_speaker_408.rs`（`#[path="../transcription/speaker.rs"]`）临时宿主；**408B 接入后删本文件 + 模块 `#![allow(dead_code)]`**；mod 行待主控在 406 后合。
- **模型**：`models/speaker-campplus-zh-en/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx`（28,281,164B，sha256 `aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2`，gitignore）。
- **验证**：模块 **10P/1I**；真模型 本人 cos 0.966 vs 他人 ≤0.093；`fmt --check` EXIT 0；`check --all-targets` 0 error、warnings 92/87=基线；全量 `cargo test` 0 failed。证据 `collab/evidence/408a/`。
- 红线：未改 `main.rs`/`mod.rs`/`local_stream.rs` / 未 commit / 未 build release / 版本未动 / 零凭证。

## 2026-09-24 — coder-1 — LOCALRT-PERF-405 ✅ 阶段一交付（待主控验收）

- **性质**：只动 `local_stream.rs` / `patches/ctranslate2-sys/build.rs` / `scripts/init-publish.ps1`（+ 主控许可的 `qwen_inference.rs` 删 1 死方法）；**未动** `main.rs`/`transcription/mod.rs`（coder-2 的 406）。
- **① 影子收尾移除（DEC-086）**：删常量/`endpoint_confirm_text`/影子状态/**整段影子重解分支**/DBG-284·289 shadow 日志/2 测试；显示改主解；新增护栏 `guard405_no_shadow_in_production`。未动切句/342/派发/sentence_id。
- **② F-C-01**：`DisplayCache` 增量缓存 + 5000 随机序列逐字等价测试。**③ F-A-02**：热循环只为日志的计时/计数收进 `log_enabled!(Debug)`。
- **④ CT2 每次重编根因**：build.rs `rerun-if-changed=src/sys`/`CTranslate2` 缺路径 ⇒ `MissingFile` 每次重跑；改仅存在路径登记，实测第二次 build **0.89s 不编 CT2**。**⑤ ps1 Step5** 产物名改 `feiyin-ime.exe` 等 + 缺件 exit1（保持 BOM）。
- **验证**：`fmt --check` EXIT 0；`check --all-targets` 0 error、warnings 92/87=基线；全量 `cargo test` **1597P/0F/48I**；真模型流式 E2E in=60.15s→out=53.79s。证据 `collab/evidence/405/`。
- ⚠️ CT2 首建遇一次 MSBuild `MSB4175`（masm temp DLL）重试即过（与改动无关）；bash `TMP=/tmp` 会加剧，构建用 Windows TEMP。红线：未 commit/未 push/未 build release/版本未动/零凭证。

## 2026-09-24 — coder-1 — POC-SPEAKER-VERIFY-404B ✅ 交付（PoC，待主控验收）

- **性质**：延续 404，只改 `#[cfg(test)]` PoC + 文档，未改生产。语言按主控确认「中英韩日」。
- **数据**（~1.3GB scratch，已清理）：zh=AISHELL-1 10 人；en=LibriSpeech test-clean 12 人；ko=Zeroth-Korean 12 人；**ja=缺口**（逐源排除）。
- **模型**：加 CAM++ zh_en / CAM++ en-vox / WeSpeaker ResNet34 en-vox / ERes2Net en-vox（+404 两个）。
- **结果**：**推荐修正为 `CAM++ zh_en`**（中 0%、英 2-5s 0.10%、韩 1.33%）；备选 ERes2NetV2 / ERes2Net en-vox；**CAM++ en-vox 异常差不推荐**；1-2s 偏弱、≥2s 才可靠、韩语最难；高置信剔除率好模型 94~100%。
- **缺口**：ja 未测 + 无「同一人多语」数据 ⇒ 补录清单（≥3~5 人、每人每语 ≥10 句 ≥2s、安静房+旁人说、16k WAV）。
- 结论 `collab/research/speaker-verify-404.md`（404B 章节）；证据 `collab/evidence/404/corpus_404b.txt`。pyarrow 走 scratch 独立 venv（系统 Python 已还原）；删 scratch ~4.3GB。未 commit/未构建/版本未动/零凭证。

## 2026-09-24 — coder-1 — POC-SPEAKER-VERIFY-404 ✅ 交付（PoC，待主控验收）

- **性质**：只新增 `#[cfg(test)]` PoC 模块 + 调研文档，**未改任何生产逻辑**。
- **模型**（6 个，sha256 校验通过）：CAM++ zh-cn 27MB / ERes2NetV2 68MB / ERes2Net-large 110.7MB / WeSpeaker ResNet34-cn 25.3MB / ResNet293 109MB / NeMo TitaNet-large 96.7MB。
- **协议**（满足「不是只为我定制」）：8 名说话人（Gavin+外部 7）**每人轮流当使用人**、其余为背景；池化全局阈值 + 逐人 FAR/FRR + 最差。
- **结果**：≥2s 池化 EER **CAM++ 0.26%（最优）**、其余中文 0.53%、ResNet293 2.51%（整位使用者被拒）；1-2s ERes2NetV2 3.07%；**<1s 全不可用**。CAM++ 最快（3s ~40ms）。**高置信剔除阈值 0.55~0.65 → 背景剔除 ~96~99%**、使用者误拒≈0；叠加 score 0.82→0.42（不剔除）。
- **接入设计**：只做高置信剔除 + 延迟到最终注入前（不撤回预览）；最短音频 ≥2s。
- 结论 `collab/research/speaker-verify-404.md`；证据 `collab/evidence/404/`。🔴 缺口：无「本人+旁边人同时说」实录，建议 Gavin 补录。清理 scratch 模型。
- 红线：未 commit / 未构建 / 版本号未动 / 零凭证。

## 2026-09-24 — coder-1 — RELEASE-ISS-FIX-401 ✅ 阶段一交付（待主控验收）

- **性质**：只改两份 `.iss`（逐字相同），未改 Rust/Cargo/tauri.conf，未构建、未编安装包。
- ① 两份 `.iss` → **UTF-8 with BOM**（首三字节 `efbbbf`）；加 BOM 前后去 BOM 的内容 sha256 均 `55ba0349…` ⇒ 中文逐字未变（`collab/evidence/401/encoding_evidence.txt`）。
- ② `MinVersion=6.1` → **`10.0`**（DEC-000：Win7 已移除，注释同步）。
- ③ `[UninstallRun]` 增 `feiyin-ime-ui.exe`（`KillVoiceIMEUI`）/ `crash-reporter.exe`（`KillCrashReporter`）两条 `taskkill`，均 `runhidden`。
- 全文扫描 `[Icons]`/`[Registry]`/`[Run]`/`[Code]`：**无** 写死 `voice-ime.exe` 残留。
- 两份 sha256 全等 `faed9765…`。红线：未 commit / 未 push / 未构建 / 版本号未动 / 零凭证。

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

## 2026-09-24 — tester-1 — BUILD-399 ✅ 出包（撤回 398 词库前缀 · 直接出包；九项 + 专项 + 三特殊点全 PASS）

- **交付源码**：HEAD `49bae07`（`src/` 与 `678c7a0` 逐字节一致：本地实时恢复不注入词库、无 `Technical terms:` 前缀；397 翻译提速保留），版本 0.9.3。**Gavin 明示不跑回归**。
- **构建**：Step1 残 0 → Step2 **SKIP**（`ui/`+`src-tauri/` 无 diff）→ Step3 main **6m14s**（92w + crash 9w）→ Step4 `scripts/init-publish.ps1`（DLL + VC++ runtime）+ 手工 cp 三 exe/四 rules toml。产物 main `0dba0c97e553…`(14,922,240B/15:37:19) / ui `ee7f2571f1d4…`(10,050,048B/13:45:21 未改) / crash `a3ee1930d585…`(24,879,104B/15:35:40) / ct2 `5655295e195e…`(27,405,312B/15:34:42)；两副本全等。
- **九项逐项 PASS**：①时间戳 15:31–15:39 ②两副本 sha 相等（main/crash 异于 398；ui 未改属预期）③**0.9.3** ④冒烟 PID **12492 Responding=True**/无新 crash.json/无新 panic/`WM_CLOSE`→controller **328ms**/残 0 ⑤config `da2be5da…` 不变 ⑥warnings 92/9（+`ctranslate2-sys` 18=397 基线）⑦反探针 `Technical terms: `=**0**、正探针 `(threads=`=1 ⑧scene/itn/homophone/wordbook 四表三副本全等 ⑨**VC++ 五个运行库齐全 + sha 与 Redist `14.44.35112` 源一致**。
- **专项**：冒烟 `-debug` 新窗 `[LocalRT-DBG-320] hotwords inject`=**0** + 源码 `main.rs:7574` LocalRealtime 早退存在 + 时间戳 + sha（未真实录音，按 N/A 三证）。
- **三特殊点（仅核验）**：① sherpa 四 DLL 三副本全等 + `onnxruntime` **1.28.2**；② itn-rules 三副本 `60b227de…` 全等；③ `Publish/models/` 1.7B 七文件与源 sha256 全等、0.6B 保留。
- ⚠️ **与预期不符两处**：(1) **CT2 被重编** —— `patches/ctranslate2-sys/build.rs` 的 `cargo:rerun-if-changed=CTranslate2` 使每次 release 重跑 build script；sha `b7a5e2c1`→`5655295e`，**同大小 27,405,312B、不同字节 18.17%**（C++ 并行构建不可复现），导入表与 398 完全一致（含 `VCOMP140.DLL`、无 `dnnl.dll`），冒烟实证 `(threads=8)` ⇒ 功能正常，代价是每次出包多 ~4–5min；建议另单收窄 `rerun-if-changed`。(2) `init-publish.ps1` Step5 exe 名过期（`voice-ime.exe`）→ 两主 exe 被 SKIP，本次手工 `cp` 补齐；建议另单改为 `feiyin-ime.exe`/`feiyin-ime-ui.exe`。
- 🔴 **Step1 强杀输入法进程 ⇒ 已提醒 Gavin 重启 + 带 `-debug` 端测**。未 push / 版本号未动 / 未改生产代码 / 未 `cargo clean` / 零凭证。证据 `collab/evidence/399/`。

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

## 2026-09-24 — coder-2 — RT-PERF-AUDIT-403 ✅ 交付（本地实时管线只读性能审计，无代码改动）

- **性质**：只读审计本地实时全链路（采集 → 流式预览 → 路B 精解 → 预览上屏 → 松键上屏），**未改任何代码/配置/Publish**；产出 `collab/research/rt-perf-audit-403.md`。证据源 `target/release/debug.log`（本会话 525 行）。
- **前提核对**：`log::debug!/info!` 宏自身按级别短路（默认 Warn ⇒ 宏内参数不求值）⇒ 真问题是**宏外累计 + 真实分配/测量**。
- **Top5**：① `local_stream.rs:1447-1494` shadow 收尾在流式线程同步重解（本会话 7 次合计 2903ms、单次峰值 **1267.9ms**；`endpoint confirm` **2/2 used=main** 零净收益，**Gavin 明令保留 ⇒ 只报告**）；② 每 chunk 全文 `display_text`/`confirmed_text`/shadow_view 拼接 + `preview_display` 分配（`:1571-1580`）；③ 每帧 GDI 量宽 + D2D `CreateTextLayout` + 两次 Vec 分配（`main.rs:3755/5293/5356/3063`；浮层 16ms 循环 `main.rs:2419`）；④ hot-loop 只为日志的计时/计数未守卫（`:1099/1113-1118/1156/1196/1215/1229/1354/1474/1683`，违 DEC-077）；⑤ 流式 / 路B 精解各 `min(核,8)` 并发争用（`local_stream.rs:64` + `mod.rs:133`）。
- **六类均结论**（B 类「已查无」）；**确定冗余** F-C-01/F-A-01/F-A-02、**需 Gavin 定** shadow（F-D-01）、**需实测** 线程数（F-F-01）。**已核正确**：audio pre-roll 诊断全守卫、`t_shadow`/DBG-380/382/277 已守卫、VAD 185.2ms/1541 chunks 可忽略、无 sleep 轮询。
- **红线**：未改生产代码 / 未 `cargo build` / 未 commit / 未 push / 零凭证。

## 2026-09-24 — coder-2 — FIX-ACC-MISMATCH-GUARD-406 ✅ 交付（同窗流式比对守卫 + 未闭合标签守卫）

- **需求**（Gavin BUILD-398 端测 #1/#1+/#7）：精解漏整句 / 幻觉短句 / 吐未闭合标签 ⇒ 预览回缩 + 最终丢句。根因：`output_rate_ok` 只比字/秒；`strip_angle_tags` 只剥闭合标签。
- **改动（仅 `src/main.rs` + `src/transcription/mod.rs`，平台中立）**：
  - A `transcription/mod.rs:434-446` 常量 + `MismatchVerdict`；`:452` `normalize_for_mismatch`；`:470` `lcs_subseq_len`（**最长公共子序列**）；`:496` `pub(crate) acc_vs_streaming`。接入 `main.rs:8528-8554` `harvest_acc_window!`（精解非空且不通过 ⇒ 用同窗流式替换 + `[LocalRT-DBG-406]` warn）。
  - B `strip_angle_tags`（`:574-625`）新增**未闭合标签**剥离（`<`+≥3 连续 ASCII 字母、名字 run 内无 `>`）；`a<b`/`3<5`/`<3块钱` 不误剥。
- **校准（步骤 C）**：三例 reject（ret/len = 0.43/0.43、0.04/0.32、0.00/0.48）；正常纠错 accept（0.83/1.00、0.96/1.04、全角/大小写 1.00/1.00）；`reflow applied` **24** 条长度比代理 **21/24 ≥0.8**。🔴 历史 acc 串不落盘 ⇒ 字符串级 retention 不可还原（如实记录）。
- **测试**：新增 `fix406_acc_vs_streaming_verdicts` / `fix406_harvest_acc_window_calls_acc_vs_streaming`；改写 `ts387_tag_boundaries`（未闭合标签契约变更）。
- **验证**：`rustfmt --config skip_children=true`（我两文件）CLEAN；全仓 `cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **bin 92 / test 87** = 基线；全量 `cargo test --no-fail-fast` **0 failed**（bin **1597P/48I**）。
- **未验证**：实机端测（三例是否不再回缩 / `[LocalRT-DBG-406]` 命中与 0 误拒）交 tester-1 / Gavin。
- **红线**：未改 local_stream / patches / scripts（coder-1 405）；未 commit / 未 push / 未 build release / 零凭证。

## 2026-09-24 — coder-2 — TEST-SYNC-408A ✅ 交付（阶段三 · 声纹模块非作者护栏 14 条）

- **被测**：`SPEAKER-VERIFY-408A`（作者 coder-1，HEAD `190b105`）。按**设计契约**写，不照抄实现。
- **范围**：`src/transcription/speaker.rs` **仅**追加 `#[cfg(test)] mod testsync408a_tests`（`:586-908`，+329/0）；生产代码零改动。
- **14 条 / 契约 1~6**：判定顺序与边界；注册防污染 + 已知局限；漂移上限 + 低分逐位不变；存档往返与丢弃；跨用户不吸收；余弦边界。
- **验证（白名单）**：`rustfmt --config skip_children=true src/transcription/speaker.rs` **CLEAN**；`cargo check --all-targets` **EXIT 0**、0 error、warnings **92/87** = 基线、speaker.rs 零 warning。🔴 **未跑 `cargo test`**（阶段三禁止，首跑由 tester-1）。
- **未发现生产缺陷**；**未改生产代码 / 未 commit / 未 push / 零凭证**。

## 2026-09-24 — coder-2 — SPEAKER-VERIFY-408B ✅ 交付（声纹接入路B + 按语种分档存档）

- **需求**（Gavin BUILD-399）：「背景人声被近场门放行」⇒ 按声纹认人；「识别模型给出的标签是哪个语言，就用对应语言版本的声纹档案比照」。
- **改动（流程定稿）**：① 解码前 `speaker::filter_ranges_by_voiceprint`（每 ≥2s 区间对所有已就绪语种档取最高分，≥一档就绪且 <0.45 ⇒ 剔除）② 解码一次取 L（前缀优先 / 字符集兜底）③ 解码后 L=ja/未知/档未就绪 ⇒ `redecode_with_ranges(原 ranges)` 保护 ④ `commit_voiceprint_offers` 按 L 归档（新语种闸 <0.3 不收）⑤ 按语种分档存 `voiceprint.bin`（v2，v1 迁移不丢弃）⑥ 406 按剔除比例放宽。
- **文件**：`transcription/speaker.rs`（重写 862/340）、`transcription/mod.rs`（418/49）、`main.rs`（45/10）、`audio/mod.rs`（2/1）、`poc_slice_cut_381.rs`（6/3）；**删 `src/bin/poc_speaker_408.rs`**。
- **测试**：`speaker` 单测 + `testsync408a/b` + `mod.rs` `fix408b_tests`；真模型 `#[ignore]` 2 条（self 0.966/他人 ≤0.093；window self 0.992/他人 0.076）。
- **验证**：`rustfmt --config skip_children=true` CLEAN；全仓 `cargo fmt --check` **EXIT 0**；`cargo check --all-targets` **0 error**、warnings **92/87** = 基线；全量 `cargo test --no-fail-fast` **0 failed**（bin **1641P/51I**）。
- **出包提示**：Publish 需新增 `models/speaker-campplus-zh-en/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx`（28,281,164 B，sha256 `aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2`；gitignore）。
- **未验证**：实机端测（背景人声是否被剔、本人不误剔、`[LocalRT-DBG-408]` 埋点）交 tester-1 / Gavin。
- **红线**：未改近场门/1200ms 派发/滑窗规则/407 末尾窗；未加 config/UI/env；未 commit / 未 push / 未 build release / 零凭证。

## 2026-09-24 — coder-2 — TEST-SYNC-405-407 ✅ 交付（阶段三 · 非作者护栏 9 条）

- **被测**：`LOCALRT-PERF-405`（DEC-086 影子移除 / `DisplayCache` / F-A-02 计时计数）与 `LOCALRT-TAIL-WINDOW-407`（1900ms 末尾组窗），作者 coder-1，HEAD `73537a2`。
- **范围**：`src/transcription/local_stream.rs` **仅**追加 `mod testsync405_407_tests`（+168/0，4 条）、`src/main.rs` **仅**追加 `mod testsync407_tests`（+128/0，5 条）；生产代码零改动；未碰 mod.rs/speaker.rs。
- **405**：源码（`create_stream()` 恰 2 处 + 无 `shadow`）；`DisplayCache` 与 `StreamingAsrState::display_text()` 逐字相等（独立夹具）；源码（判定实参不含日志计时/计数）；407 触发时序模拟。
- **407**：源码（长静默以 pending 为条件、用后清空）；`tail_window_span` 边界；`tail_backtrack_secs` 性质；收尾后继续说话仍全覆盖；末尾窗 1 字差异不丢尾。
- **验证（白名单）**：`rustfmt --config skip_children=true`（两文件）CLEAN；`cargo check --all-targets` **EXIT 0**、0 error、warnings **92/87** = 基线、两文件零新增 warning。🔴 **未跑 `cargo test`**（阶段三禁止，首跑由 tester-1）。
- **疑似生产缺陷：未发现**；**未改生产代码 / 未 commit / 未 push / 零凭证**。
- **返修（tester-1 回归红，测试自身错误）**：`ts407_tail_only_when_pending_source_guard` 原用固定 2600 字符窗口截 `AccInput::LongSilence` 臂，而 `pending_slice = None;` 在臂内 ~5173 字符 ⇒ 假红。改为 `arm_body()`：对臂体 `{…}` **括号配平**（跳过字符串/字符/行·块注释内括号，防 `[GUARD-SKIP-BRACE-IN-STRING-382]`）截到**臂闭合 `}`**；sandbox 复刻验证 seg=5173、含三断言串。仅测试自身修正，生产零改动。验证：`rustfmt --skip_children` CLEAN；`cargo check --all-targets` 0 error、warnings 92/87=基线。
