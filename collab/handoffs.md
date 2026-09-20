# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行，超 200 行上限）。

## 2026-09-20 — coder-2 — FIX-OVERLAY-SCROLL-255 + LOCALRT-PREVIEW-PUNCT-256 ✅ 交付（待主控验收 → tester-1 回归）

- **255**：`main.rs` GDI `:3677` / D2D `:5200` 排版矩形 `right: text_right - scroll_x` → `right: text_right`。修「流式上屏溢出后右侧留白」；布局宽度 = `max(text_width, visible_w)`，最新文字贴右沿；裁剪区不动。`streaming_scroll_offset` 与两条护栏（`:11903` 契约 / `:11927` 几何）**未动**。🔴 该绘制函数所有流式档共用 ⇒ **在线 FunASR 右侧空白同样消失（修复，非回归）**。
- **256**：`local_stream.rs::transcribe_streaming_local` 加 `punctuation_engine: Option<&mut PunctuationEngine>`；**仅 `endpoint=true` 已确认句**打点，中间句保持裸文本；`None` 跳过。引擎由新分支调用方传入（录音前就绪 `cached_punctuation.as_mut()`），**不在 local_stream 内新建**；只影响 overlay 预览，不影响最终文本（accuracy 2pass 自带标点）。
- **文件**：`src/main.rs` +21/-4、`src/transcription/local_stream.rs` +20/-1。
- **验证**：cargo check --all-targets 0 error、warnings **110/101** = 基线；rustfmt 两文件 clean；`--numstat`==`-w`（21/4、20/1）。🔴 未跑 cargo test（护栏归 tester-1）；按 Gavin 指令不写新测试。
- **红线**：未碰 src-tauri / ui / transcription/mod.rs / 未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-20 — coder-2 — LOCAL-RT-ENGINE-239-B ✅ 交付（新管线主控接线，含 243/247/248；待主控验收 → tester-1 全量回归）

- **新分支**：在线流式分支之后新增 `LocalRealtime` 编排——`send StreamingIdle` → `std::thread::scope`（ASR 线程跑 `transcribe_streaming_local` 发 `StreamingText` + worker 跑 `record_streaming`）→ join 拿 `(preview_text, pcm)` → **丢弃预览文本**、`run_pipeline_core(Ok(pcm), initial_text=None)` 走 accuracy 2pass（主通道 ITN 启用）。
- **单状态（DEC-066 附则一，主控裁 A）**：`6617` 按 `AsrModel::from_config` **枚举**切文案（local_realtime→`overlay_processing`，其余→`overlay_transcribing` 零变）；`run_pipeline_core` 加 `transcribing_status_text: &'static str` 参数，现有两调用点传原值。
- **切档 Info（248）**：LocalRealtime 重载触发时发 `PipelineEvent::Info(loading_hint)`；控制器 Info 臂 auto_close 2500ms。
- **缺失报错（247）**：`asr_reload_in_flight`→提示加载中并跳过（不降级）；否则 `ModelUnavailable(「所选模型不可用：<缺哪个>」)`（**不走 convert_to_friendly_error**）；`last_reload_error` 记录失败原因。
- **243**：新管线发 StreamingText ⇒ `last_streaming_text` 自动填充，编辑态学习零改动；修正 `main.rs:6850` 过时注释。
- **Send 包装**：`local_stream.rs` 新增 `SendOnlineRecognizerRef` + `into_inner(self)`（破 Rust2021 disjoint capture）；移除 `transcribe_streaming_local` 的 dead_code allow。
- **新增 locale key**：`local_realtime_loading_hint` / `local_realtime_unavailable`（三份）。
- **验证**：`cargo check --all-targets` 0 error、warnings **110/101** = 现行基线（任务书 111/102 为 coder-1 mod.rs 改动前旧值；差异 = 接线使 `online_recognizer()` getter 由 dead 变 used，−1 warning）；rustfmt 三文件 clean；`--numstat`==`-w`（334/7）。🔴 未跑 cargo test（归 tester-1）。
- **红线**：未碰 src-tauri / ui / transcription/mod.rs / 未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-20 — coder-2 — DOC-LOCAL-RT-249-250 ✅ 交付（端测清单 + macOS 跨端交接，纯文档零代码）

- **249**：新建 `collab/e2e-checklist-local-realtime.md` —— 本地 realtime 10 项端测（Ctrl+M 只开不关 / 切档 Info ~6s 自动关 / 预览不回退 / 松键单状态 + 三档对照 / accuracy 最终文本 / 热词 / 编辑态 / 词库 MINLEN-242 / 1.6GB 内存释放 / 🔴 现有三档零回归），逐项步骤 + 预期 + FAIL 判据，附前置状态表与通过判据。
- **250**：`docs/MACOS-HANDOFF.md` 追加「LOCAL-RT-249 / MACOS-HANDOFF-250」段 —— 6 项跨端结论（`local_stream.rs` / `online_recognizer` 槽位 / `MIN_CANDIDATE_CHARS=2` 标**行为变更** / N5·N6·N9 extract / `LocalRealtime` 变体需补 match arm / UI Ctrl+M + locale），全部平台中立。
- **红线**：纯文档，未碰任何 `.rs`/`.ts`/`.tsx` / 未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-20 — coder-2 — PIPELINE-ORCH-238-B ✅ 交付（N6 LLM 格式化/翻译提取，纯结构零行为变更；待主控验收 → tester-1 全量回归）

- **范围**：DEC-066 提取阶段最后一块。`run_pipeline_core` 内联 N6（HEAD `src/main.rs:8752-8910`）→ `fn run_llm_stage(...) -> LlmStageOutput`。
- **新符号**：`struct LlmStageOutput { text, llm_handled, format_failed }` + `fn run_llm_stage`（14 参显式传入，零外部可变捕获）；调用点 `let llm_out = run_llm_stage(...)` 后解构三绑定。
- **原位保留（风险点）**：`Processing` 事件 #1 翻译 `:8959`、#2 optimize `:9036`，均在各 LLM 调用前；`learn_llm_suggestions` `:8988` / `:9046`，均在 `Ok(result)` 内。时序零变。
- **方向判据**：`translate_requested` / `derived_target` / `Translation direction derived` 日志保留调用点（实现中修正过一次自造重复日志）。
- **验证**：`cargo check --all-targets` 0 error、warnings **111/102**=基线；`rustfmt --check src/main.rs` clean；token 多重集对照无 old>new；`git diff` 仅 3 hunk 在目标区。🔴 未跑 cargo test（归 tester-1）；按 Gavin 指令不写新测试/护栏/消融。
- **已知差值**：`--numstat` 180/145 vs `-w` 174/139（差 6/6）= 新调用点参数行在 -w 下巧合匹配被删原块行，extract method 固有，非 fmt 连带。
- **红线**：只改 `src/main.rs` / 未 commit / 未出包 / 版本未动 / 零凭证。

## 2026-09-20 — coder-2 — PIPELINE-ORCH-238（N9+N5）✅ 交付（后处理链节点提取，纯结构零行为变更；待主控验收 → tester-1 全量回归）

- **范围**：DEC-066 第一步，按主控调整只做 N9 + N5，N6（LLM 格式化/翻译）拆到 238-B 本单不做。调用点原地替换 = 标准 extract method。
- **N9**：HEAD `src/main.rs:8974-9008` 内联标点决策 → 新 `fn apply_local_punctuation(final_text: String, enabled, llm_handled, translate_requested, native_punctuated, engine: Option<&mut punctuation::PunctuationEngine>) -> String`；调用点传 `punctuation_engine.as_deref_mut()`，内部 `if let Some(ref mut engine)` 等价改 `if let Some(engine)`。
- **N5**：HEAD `:8748-8777` 内联场景采集 + SCENE-OBS-001 观测日志 → 新 `fn capture_pipeline_scene(config: &AppConfig, target_hwnd: platform::WindowId) -> scene::SceneContext`；日志随迁，`f4_injected` 函数内派生。
- **零改**：`from_online_streaming`/`native_punctuated` 取值方式、节点顺序、log 文案与顺序、i18n 字符串；未碰 src-tauri。
- **验证**：`cargo check --all-targets` 0 error、warnings **111/102** = 基线；`rustfmt --edition 2021 --check src/main.rs` clean。🔴 未跑 cargo test（按 DEC-048/分工归 tester-1）；按 Gavin 指令不写新测试/护栏/消融。
- **已知差值**：`--numstat` 90/57 vs `--numstat -w` 88/55（差 2/2）= 新调用点 `final_text,`/`);` 在 -w 下巧合匹配被删 log 行，extract method 固有，非 fmt 连带；已上报主控。
- **红线**：只改 `src/main.rs` 一文件 / 未 commit / 未出包 / 版本未动 / 零凭证。

## 2026-09-20 — coder-2 — TEST-SYNC-229 ✅ 交付（在线 ASR 两轴阶段三交叉护栏，待主控验收 → 阶段四实跑）

- **改动**：三文件 `mod tests` 纯追加 **+79 / −0**，生产零触碰。G1 `qwen_inference.rs:1778-1806`；G2 `src/config/mod.rs:1806-1823`；G3 `src/transcription/mod.rs:964-995`。
- **G1**：`build_run_task_message` 轴二 `true` 分支 + `false` 对照（原只断言 false 单向，挡不住「丢参数重硬编码」）。
- **G2**：轴二隐藏字段**主配置侧** save→load 往返（`TestEnv` + 置 true）；镜像侧 coder-1 已写，主配置侧是缺口。
- **G3**：`Transcriber::new` 两轴入参 → getter 透传，用非默认值（1234/true）区分「写死默认」。
- **G4**：默认 2000/false 五处核对全在场、未发现漏改；按指令未重复写默认值测试。
- **G5 撤单**：理由经主控纠正——非「与 mirror 字面值测试同义反复」（那条是 value 级，G5 是 schema 级）；真理由是两 crate 独立编译单元，自快照是「提醒」非真闸门且加字段要付维护成本（假闸门比没闸门更糟）。残余风险转 `[CONFIG-MIRROR-DRIFT-001]` 归主控。
- **观测上报**：主配置侧无轴二默认 false 的独立断言（仅镜像侧间接覆盖），按 G4 未补，交主控裁定。
- **约束/验证**：未触碰 src-tauri ⇒ 规避 FMT-COLLATERAL-001；`rustfmt --check` 三文件 clean；`cargo check --all-targets` 0 error、warnings 111/102 持平；numstat 与 numstat -w 逐行相同（无空白噪声）。🔴 未跑 `cargo test`（DEC-048），三条断言阶段四必须实跑。
- **红线**：未 commit / 版本未动 / 零凭证。

## 2026-09-20 — coder-1 — ASR-SEG-229 + VER-BUMP-230 ✅ 交付（在线 ASR 两轴参数化 + 版本 0.9.2，待主控验收 → 阶段四回归）

- **轴一（阈值）**：`asr_online_max_sentence_silence` 默认 **800 → 2000**。`src/config/mod.rs:205` 默认 fn、`src-tauri/src/config.rs:151` 镜像同步、`qwen_inference.rs:80` `DEFAULT_MAX_SENTENCE_SILENCE`（现仅测试夹具）800→2000。理由：官方默认 1300，800 过激进致服务端 VAD 碎句 + 满屏句号；端点预览走中间结果、`final_text=confirmed+current` 不丢尾字 ⇒ 提阈值零代价。
- **轴二（开关）**：删除 `qwen_inference.rs` 的 `const SEMANTIC_PUNCTUATION_ENABLED`，新增隐藏字段 `asr_online_semantic_punctuation_enabled: bool`（`#[serde(default)]`，默认 false）。主配置 `src/config/mod.rs:167-175` + 镜像 `src-tauri/src/config.rs:123-127` 双写。逐层透传：`build_run_task_message`(+1 参) → `transcribe_streaming`/`transcribe_streaming_realtime`(+1 参) → `Transcriber::new`(+1 参 + getter) → `main.rs` 两处 `Transcriber::new`（`:7121` 热重载 / `:7232` 启动）+ `:7521` realtime 调用。
- **VER-BUMP-230**：`Cargo.toml:3` / `src-tauri/Cargo.toml:3` / `src-tauri/tauri.conf.json:9` 三处 `0.9.1→0.9.2`；`ui/package.json`(0.1.0) 未动；两份 `Cargo.lock` 自动重写版本行。
- **三条红线**：① Tauri 镜像字段已同改（含 `make_minimal_cfg_with_asr_model` struct literal）② **未对 src-tauri 跑 `cargo fmt`**（其 fmt 漂移为既有，`:4` 空行，非本单引入）③ 新增测试仅镜像往返 1 条（`mirror_asr_online_semantic_punctuation_roundtrip`），另在既有 mirror 字面值用例补一条默认 false 断言。
- **既有断言机械同步（3 处，非新增覆盖）**：主 config 默认 800→2000、镜像字面值 800→2000、`build_run_task_has_max_sentence_silence_800`→`_2000`。
- **🔴 端测前置（主控已确认写入清单）**：两轴值在 `Transcriber::new` 时熔入，热重载触发键不含二者 ⇒ **A/B 必须重启进程**（与既有 silence 字段行为一致）。Gavin 的 `Publish/config.toml` 若写死 `asr_online_max_sentence_silence = 800`，改默认对其零作用，须手改该行；新字段缺失则走 default(false)。
- **验证**：`cargo check --all-targets` 0 error / test warnings **102**（基线）；`cargo check` 0 error / **111**；`cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / **17**；`rustfmt --check` 四根文件 clean；定向测试 `build_run_task` 7P、`config::tests::asr_056` 14P、`transcriber_qwen_audio_online` 2P、`..._silence_2000` 1P、镜像 4P（含新断言）——**全 0F**。🔴 未跑全量 `cargo test`（阶段四 tester-1）、未 `cargo build --release`。
- **边界**：`native_punctuated` / 本地 CT-Transformer 跳过逻辑 / 标点后处理**零改动**；`src-tauri/src/qwen3.rs`（连接自检）仍硬编码 false，属独立路径不含 silence，本单不动。
- **红线**：未 commit / 未出包 / 零凭证；临时文件（HEAD 版本 rustfmt 对照）已清理。


## 2026-09-20 — tester-1 — TEST-EXEC-229 阶段四全量回归 ✅ 执行完毕（🔴 1 类 FAIL：测试侧 harness 隔离缺陷、生产缺陷 0，只跑不改）

- **结果**：`cargo test`（root 10 target）**1264P / 2F / 15I**；`src-tauri` **77P / 0F / 0I**。**2 F 同一条** `config::tests::asr_229_config_hidden_field_semantic_punctuation_roundtrip_persists`（新增护栏 G2），在 `feiyin-ime` + `crash-reporter` 两 target 各 1F，panic `os error 3`（`src/config/mod.rs:1817`）。
- **定性（证据链）**：① 单独跑 G2 → 1P/0F；② `config::tests::` 加 `--test-threads=1` → 49P/0F；③ 默认并行 → 1F 且失败者换成**既有** `asr_056_config_hidden_field_roundtrip_persists`（谁抢输谁红）。根因 `TestEnv`（`src/config/mod.rs:657-678`）单目录 `voice-ime-test-{pid}` + Drop `remove_dir_all`，G2 与 056 均无锁并行 → 一个删目录另一个正 save。**生产缺陷 0**。
- **处置**：按令**未自行改测试/期望值**；主控另开 `FIX-TESTENV-231`（TestEnv 每实例唯一目录）派 coder-1，排本单之后（同文件串行）。
- **新增护栏**：G1 PASS、G3 PASS；G2 见上。**3 处 800→2000 机械同步全 PASS**（`asr_056_..._silence_2000` / `mirror_asr_fields_match_main_config_literals` / `build_run_task_has_max_sentence_silence_2000`）⇒ 无同步漏改。
- **消融（3 条，过滤跑）**：G1 `qwen_inference.rs:263` 硬编码 false → RED；G2 `config/mod.rs:174` `#[serde(skip)]` → RED；G3 `transcription/mod.rs:228` getter 返回 false → RED。还原后三文件 sha256 回基线、`git status` 无源码 diff、复跑各 1P/0F。
- **SKIP**：npm / test:browser（本批 `ui/` 零 diff，`git show --numstat -- ui/` 空）；E2E（按令，无 release 产物）。
- **warnings**：test profile **102**、bin **111** = 基线；src-tauri test **19**（与 coder-1 的 `check --all-targets` 17 非同一命令，不作持平判据）。
- **证据**：`collab/outbox/tester-1/testexec229/`（cargo_test_root_nff.log / cargo_test_tauri.log / ablate_g1|g2|g3.log）。
- **红线**：未改生产代码与测试期望值 / 未出包 / 版本未动 / 未 commit / 零凭证。

## 2026-09-20 — coder-1 — FIX-TESTENV-231 ✅ 交付（TestEnv 每实例唯一目录，待验收 → 阶段四复跑）

- **根因**：`src/config/mod.rs:661` `TestEnv::new()` 对所有用例返回同一 `voice-ime-test-{pid}`，`Drop` 对整个目录 `remove_dir_all` ⇒ 并发用例的目录被另一用例的 Drop 删掉。23/25 用例靠 `TEST_MUTEX` 串行掩盖；无锁用例从 **1 个**（`asr_056_config_hidden_field_roundtrip_persists`，TEST-SYNC-056 `cb0d82b` 引入，**潜伏一个多月**）增至 **2 个**（本批 `asr_229_...`，`c5cff01`）即必现。
- **改动（只一个函数）**：`TestEnv::new()` 目录名加进程内原子计数器 → `voice-ime-test-{pid}-{seq}`，每实例唯一；`Drop` 只删自己这份。**25 个用例一行未动**。
- **🔴 不补锁（主控判断，我独立复核同意）**：`TEST_MUTEX` 保护的是 `AppConfig::config_path` 全局态；那 2 例走显式 `save_to(&path)`、不碰全局态，不取锁是对的。补锁只掩盖、且把「共享目录」真根因留在原地（再加第三个无锁用例照样炸）。
- **边界核验**：实测 25 个 TestEnv 用例中无任何一例创建 >1 个 `TestEnv` ⇒ 无用例依赖共享路径；src-tauri 无同款 harness。
- **验证**：`cargo check --all-targets` 0 error、warnings **111/102**（=基线）；`rustfmt --edition 2021 --check src/config/mod.rs` clean（首版多行 `format!` 被判 DIFF，已改单行）。🔴 未跑 `cargo test`（并行竞态一次绿证明不了，归阶段四 tester-1 复跑；重点：默认并行下 `asr_229_...` / `asr_056_...` 两例需稳定绿）。
- **证据链**：`TEST-EXEC-229` 报告「单独跑绿 / `--test-threads=1` 49P0F / 并行失败者随调度漂移」= 共享资源竞态铁证；troubleshooting 条目 `[TESTENV-SHARED-DIR-RACE-001]` 由主控落档。
- **红线**：只改 `TestEnv::new()` / 25 用例零改动 / 未跑 cargo test / 未 commit / 未出包 / 版本未动 / 零凭证。


## 2026-09-20 — tester-1 — TEST-EXEC-231 复跑 + BUILD-232 v0.9.2 出包 ✅（八项核验逐项 PASS）

- **TEST-EXEC-231**：`config::tests` 默认并行**连跑 5 次全绿**（各 49P/0F/2I，证 `FIX-TESTENV-231` 有效）；root 全量 **1266P/0F/15I**（10 target，EXIT 0，与期望逐位吻合）；`src-tauri` **77P/0F**。npm/browser/E2E SKIP（`ui/` 零 diff + 按令）。消融按令不重复。
- **BUILD-232**：Step 1→4 全走（npm 1.51s / Tauri UI 1m40s / 主程序 2m20s），cp UI 入 `target/release/`、三 exe + 两 toml 入 `Publish/`。
- **八项核验**：①时间戳 14:17–14:18 ✅ ②两副本 sha 相等且三者异于上包 ✅ ③**ProductVersion 0.9.2（唯一变红再变绿）** ✅ ④冒烟 Responding=True + 启动链路完整 + 无 crash.json + panic/ERROR 0 + 残留 0 ✅ ⑤`config.toml`/`wordbook.sqlite` 零变化 ✅ ⑥warnings 111/9/17 = BUILD-228 基线 ✅ ⑦正向 `asr_online_semantic_punctuation_enabled`=1 / 反向 `SEMANTIC_PUNCTUATION_ENABLED`=0 + UI 文件名探针 ✅ ⑧toml 三副本 hash 全等 ✅。
- **端测项（交 Gavin）**：`Publish/config.toml:29` 写死 `asr_online_max_sentence_silence = 800`，新默认对其零作用 ⇒ A/B 须手改该行 + **重启**；2×2 四组合与目视重点见 result.md Part C。
- **已知非本批**：冒烟 stdout 的 `[ASR-DROP]` WARN（`src/audio/mod.rs:492-495`），主控确认下批修，不拦出包。
- **证据**：`collab/outbox/tester-1/build232/` 与 `testexec229/`。
- **红线**：未改生产代码 / 未 commit / 零凭证。

## 2026-09-20 — coder-1 — POC-LOCAL-STREAM-235 ✅ 交付（本地流式 ASR 可行性，待主控验收）

- **改动**：唯一新增 `src/bin/poc_local_stream.rs`（独立 bin，生产代码零触碰）；模型解压至 `models/`（gitignored）。
- **四数**：① 首字延迟即时起音 **~625–652ms**（略超 600ms；20ms 分帧复测不变 ⇒ 模型前瞻决定）② RTF 1/2/4 线程 **0.065/0.053/0.042**（全 ≤0.3）③ 文字跳动 **0 rewrite / 0 shrink**（前缀单调）④ 2pass performance **0.11–0.75s**、accuracy **0.70–2.60s 且 >~28s 单段空输出**（KV 512）。
- **热词实测（超撤销项）**：SenseVoice + hotwords + greedy → create 失败（"Please use modified_beam_search"）；+ beam → **EXIT 127**（"Only greedy_search is supported"）。online paraformer 同构。悖论互斥 ⇒ 物理封闭。
- **追加5-新 HomophoneReplacer**：**跑通**。SenseVoice baseline「玄界芯片福南人工投安装」→ hr「玄戒芯片湖南人弓头安装」3/3 修对；安全边界用例 0/5 越界；同音碰撞用例 2/2 改写（属既定行为）。lexicon-only = no-op（证明 rule_fsts 必需）。
- **SQLite→replace.fst**：`lexicon.txt` 可由 jieba+pypinyin（纯 Python/Windows）生成；`replace.fst` 需 pynini `cdrewrite`，**pynini 无 Windows 包**（实测 `No matching distribution`）；kaldifst 有 Windows wheel 但无 cdrewrite/cross。路径 A(WSL/Colab 离线生成)/B(kaldifst 原语手搓)/C(Rust 自实现拼音替换)。
- **验证**：`cargo check --bin poc_local_stream` 0 error；`rustfmt --check` clean；🔴 未跑 cargo test / release。
- **证据**：`collab/outbox/coder-1/poc235/`（runA/B/C + 7 probe + 15 snapshot）。
- **红线**：未碰生产代码 / 未 commit / 未出包 / 版本未动 / 零凭证。

## 2026-09-20 — coder-1 — POC-LOCAL-STREAM-236 ✅ 交付（zipformer 热词验证，待验收）

- **改动**：`src/bin/poc_local_stream.rs` 增量扩 `--model zipformer`（transducer 三件套 + cjkchar+bpe + bpe.vocab）、`--hotwords-stream`、`--max-active-paths`；生产零改动。模型 `sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20`（int8 189 MiB）。
- **三组分离变量**：A greedy 无热词 / B beam(4) 无热词 / C beam+热词。
- **热词（唯一必答项）**：**生效但弱**。默认 map=4 下 5/5 零修正；map=10~20 + score≈3.0 才 2/5 修正（飞音输入法、紫菜包饭）；玄戒芯片始终不行，通义千问仅部分，过冲(map50/score5)致重复字。两通道（config file / `create_stream_with_hotwords`）等价；beam 引入改写（rewrites 2–8，破坏 235 的 0/0）。
- **首字延迟**：3/4 素材 ≥700ms（hw5 最高 1031ms），仅 zh_30s ~420ms；**比 paraformer 差**。RTF 全 ≤0.3，beam 1T 约 +5~15%。
- **英文**：全大写确认（`GPT`/`A P I`/`TY`）。中文质量远逊 paraformer（重复结巴极重）。
- **纠 235 错误档案**：accuracy >28s 空输出系 PoC 绕过生产封装；生产 `transcription/mod.rs:374-423` 有 VAD(20s)/naive_chunk 分段三重保障（`vad.rs:17` 触发 24s）。生产 API 运行时复验**未跑**（根 Cargo.toml 无 `[lib]`，bin 无法 use 私有模块，撞红线）。
- **复现坑**：`--hotwords-stream "a/b/c"` 在 MSYS 被路径转换并触发 C++ abort，须 `MSYS_NO_PATHCONV=1`。
- **验证**：`cargo check --bin poc_local_stream` 0 error；rustfmt clean；🔴 未跑 cargo test / release。
- **证据**：`collab/outbox/coder-1/poc236/`。235 报告已留档 `result-235.md`（原被 harness 清 0，按上下文重建，主控核验 20837B）。
- **红线**：未碰生产代码 / 未改 Cargo.toml / 未 commit / 未出包 / 版本未动 / 零凭证。

## 2026-09-20 — coder-1 — LOCAL-RT-UI-240 ✅ 交付（本地 realtime 档位 UI 解锁 + 配置字段，待验收）

- **改动 6 文件**：`src/config/mod.rs`(+10) / `src-tauri/src/config.rs`(+9)（新增 `asr_local_realtime_unlocked: bool`，`#[serde(default)]` false，主+镜像含穷举字面量同步）/ `ui/src/pages/Voice.tsx`(+98/-2) / 3×i18n(+5)。
- **Ctrl+M**：window keydown，仅 Voice 页挂载 + 窗口焦点；**只开不关**（不 toggle）；复用 updateConfig→save_config 落盘。不注册全局热键（未碰 DEC-004）。
- **选项**：`localRealtimeUnlocked` 为 true 才渲染 `<option value="local_realtime">`；`getAsrDesc` 补 case。
- **卡片**：复用 accuracy 结构，列 paraformer-trilingual(228MB)+funasr-nano(972MB)；目标目录根由 `check_accuracy_model_ready.model_dir` 去末段推导（DEC-011），无新增 Tauri 命令。
- **文案**：zh-Hans「本地流式实时模型」/ zh-Hant「本地串流即時模型」/ en「Local Streaming Realtime」。zh-Hant:29 既有漂移未动。
- **验证**：`npm run build` 通过；`cargo check --all-targets` 0 error（111/102 基线）；src-tauri 0 error（17 基线）；main config rustfmt clean（src-tauri config 既有漂移 :4/:11/:512 非本单）；三 locale grep 齐全。
- **红线**：未碰 src/main.rs / ASR 引擎 / transcription；未动版本；未 commit；零凭证。

## 2026-09-20 — coder-1 — LOCAL-RT-ENGINE-239-A（阶段一）✅ 交付（本地流式引擎层，待验收）

- **阻塞裁定**：`AsrModel::LocalRealtime` 命中 `src/main.rs:8536` 穷举 match（禁碰 + coder-2 独占）⇒ 主控裁定阶段一不加变体，只做不依赖枚举的三块。
- **① 新建 `src/transcription/local_stream.rs`**（120 行）：`transcribe_streaming_local(chunk_rx, recognizer, cancel_signal, on_result, pcm_out) -> Result<String>`，与 qwen 真流式平行；`accept_waveform(16000)`→`while is_ready decode`→`get_result`→`on_result`；`is_endpoint()`→`sentence_end=true`+`reset()`；**复用 `StreamingAsrState` 无新状态机**；`pcm_out` 累积 PCM 供 2pass。
- **② `online_recognizer: Option<OnlineRecognizer>` 字段**（mod.rs）+ getter + 两处构造置 None；!Send 沿用既有 `unsafe impl Send` SAFETY 论证（两 recognizer 共用），未发明新模式。阶段一 `#[allow(dead_code)]` 桥接，二阶段移除。
- **③ 注释修正**：VAD「懒加载」过时描述改为「`Transcriber::new` 立即构建」；补 DEC-067 双模型并存文档。
- **🔶 签名偏差待确认**：任务书写 `-> Result<String>`，2pass 需 PCM ⇒ 加可选出参 `pcm_out`；若 239-B 要 `Result<(String, Vec<f32>)>` 一行可改。
- **验证**：`cargo check --all-targets` 0 error（warnings 111/102 基线）；rustfmt 两文件 clean；未写任何新测试/harness（Gavin 指令）。
- **阶段二（待放行）**：枚举变体 + `build_recognizer` 双模型构建 + `main.rs:8536` arm（前处理跟 **Accuracy**）+ `mod.rs:1707` 测试穷举。
- **红线**：未碰 src/main.rs / src-tauri；未动版本；未 commit；零凭证。

## 2026-09-20 — tester-1 — TEST-EXEC-241 238+240 合并回归 ✅（只跑现有用例，FAIL 0）

- **结果**：`cargo test` root **1266P/0F/15I**（11 target，EXIT 0）；`src-tauri` **77P/0F/0I**；`npm run test` **7 files / 100P/0F/11S**（111，EXIT 0）。三项与基线**逐位吻合** ⇒ 238「行为逐位不变」经现有用例坐实、240（config 双写 + ui）无回归。
- **target 计数**：11 vs 任务书基线「10」= `89fe69a`（18:12）新增 `poc_local_stream` bin（0 用例），P/F/I 不受影响。
- **已知遗留未复现**：`TEST-FIX-002/003`（App mock / Wordbook dialog）本批 0 failed——照实报，未改测试期望值。
- **按令不做**：新增用例 / harness / 消融 / 脚本；browser/E2E 与出包 SKIP（无视觉布局改动 / 未收出包令）。
- **证据**：`collab/outbox/tester-1/testexec241/`（cargo_test_root.log / cargo_test_tauri.log / npm_test.log）。
- **红线**：未改生产代码与测试期望值 / 未 commit / 未出包 / 版本未动 / 零凭证。

## 2026-09-20 — tester-1 — TEST-EXEC-245 238-B+239-A 阶段一 回归 ✅（只跑现有用例，FAIL 0）

- **结果**：root **1267P/0F/15I**（EXIT 0，与期望逐位吻合）；`src-tauri` **78P/0F/0I**；`npm run test` **7 files / 100P/0F/11S**（EXIT 0）。
- **src-tauri 78≠77 归因（非回归）**：唯一新增 `minlen242_single_char_candidate_rejected`（`src/wordbook/mod.rs:356`）经 `src-tauri/src/wordbook.rs:1` 的 `#[path="../../src/wordbook/mod.rs"]` 同时编进两 crate ⇒ root 1178→1179 与 tauri 77→78 同源，各计一次。
- **本批 ui/ 零 diff**（任务书所称 i18n 一行属上一批 c6828e9）；npm 仍跑作兜底，与上轮一致。
- **按令不做**：新增用例 / harness / 消融 / 脚本；browser/E2E 与出包 SKIP。
- **证据**：`collab/outbox/tester-1/testexec245/`（cargo_test_root.log / cargo_test_tauri.log / npm_test.log / tauri_list.txt）。
- **红线**：未改生产代码与测试期望值 / 未 commit / 未出包 / 版本未动 / 零凭证。

## 2026-09-20 — coder-1 — LOCAL-RT-ENGINE-239-A（阶段二）✅ 交付（枚举变体 + 双模型构建，待验收）

- **① 枚举**：`AsrModel::LocalRealtime` + `from_config("local_realtime")`。
- **② 双模型**：新建 `build_local_realtime_recognizers`（online=streaming paraformer greedy / offline=accuracy `create_funasr_nano_recognizer` 带 hotwords，返回三元组）；`Transcriber::new` 加 LocalRealtime 专用分支并存常驻 + 按 accuracy 建 VAD；**任一缺失/失败即 Err 不降级**（DEC-067 附则一），错误信息区分 online/offline 供 239-B 浮层。`build_recognizer` 的 LocalRealtime arm 走防御性 `bail!`（3-tuple 契约塞不下 online，主控认可）。
- **③ `main.rs:8545`** 前处理 arm 跟 **Accuracy**（ACC 常量）。
- **④ `mod.rs:1824`** 测试穷举 match 补 arm。
- **端点**：`enable_endpoint=true`；rule1=2.4 / rule2=**2.0**（对齐 ASR-SEG-229 在线档 2000ms，注释锁定勿改回 1.2）/ rule3=20.0；线程 4。
- **签名**：`transcribe_streaming_local -> Result<(String, Vec<f32>)>`；字段/getter allow 已移除，仅函数保留一行（239-B 接线前无调用者，守 warnings 基线）。
- **验证**：`cargo check --all-targets` 0 error（warnings 111/102 = 基线）；rustfmt mod.rs/local_stream.rs/main.rs clean；未写测试/harness（Gavin 指令）。现有三档零影响已自证（build_recognizer 三 arm 逐字未动 / new() 新分支对其余档恒假 / select_preprocessing_params 仅加 arm / is_online_streaming 未改）。
- **待 239-B**：`transcribe_offline_detailed` 的 VAD 分支仅认 Accuracy，LocalRealtime 2pass 复用需纳入；main.rs 若干 `==Accuracy`/`is_online_streaming()` 判定点接线；移除函数 allow。
- **红线**：未碰 src-tauri；未动版本；未 commit；零凭证。

## 2026-09-20 — coder-1 — LOCAL-RT-READY-246 ✅ 交付（双模型就位检测，待验收）

- **改动 5 文件**：`src/transcription/mod.rs`（新增 `check_local_realtime_models_ready`，现有 accuracy 检测零改动）/ `src-tauri/src/main.rs`（新 Tauri 命令+结构体+注册）/ `ui/src/pages/Voice.tsx`（卡片改用新命令，分别显示缺失）/ 3×i18n（4 key）。
- **判据一致**：online 文件名（目录 + encoder.int8.onnx + decoder.int8.onnx + tokens.txt）与 `local_stream.rs:47-50` 加载清单**三处逐字一致**；offline 与 accuracy 判据一致。
- **判据来源（主控打回后定稿）**：offline **直接调用** `check_accuracy_model_ready`（判据单一来源，防副本漂移致静默失效）；该处由主控 Edit 落地。warnings 基线自本单起 111/102 → **110/101**（主控批准，属改善）。
- **中途修正**：卡片首版裸 `#d9534f` 被 design-tokens 测试（UITEST-137）判红 → 改令牌 `var(--status-success/error)`。
- **验证**：`npm run build` 通过；`npm run test` 100P/11S；root cargo check 0 error（warnings **110/101** = 新基线）；src-tauri 0 error（17 基线）；transcription/mod.rs rustfmt clean；未新增测试。未碰 coder-2 的 src/main.rs / src/i18n.rs / local_stream.rs。
- **红线**：未动版本；未 commit；未出包；零凭证。

## 2026-09-20 — tester-1 — TEST-EXEC-251 v0.9.3 最终全量回归 ✅（出包前闸门，五项全绿，FAIL 0）

- **结果（五项）**：root **1267P/0F/15I**（EXIT 0，=期望）；`src-tauri` **78P/0F/0I**（=期望）；Vitest happy-dom **7 files/100P/0F/11S**；🆕 **Vitest browser（Chromium）1 file/5P/0F**（本批 ui 有 diff，按 worker-guide §五必跑兜底；UITEST-137 未复现）；E2E SKIP（无 release 产物）。
- **warnings 新基线核对**：bin "feiyin-ime" **110** / test 档 **101** / src-tauri `cargo check` **17** —— 三项逐位吻合；src-tauri test 档 19 与 check 17 非同一命令，不作判据。
- **结论**：三处提取（N5/N6/N9）+ 第四条 ASR 管线 + 词库闸门 + 246 前端，共 **1345** 用例全绿 ⇒ 无本批回归，出包闸门通过。
- **按令不做**：新增用例 / harness / 消融 / 脚本；未出包。
- **证据**：`collab/outbox/tester-1/testexec251/`（cargo_test_root / cargo_test_tauri / cargo_check_tauri / npm_test / npm_test_browser 五日志）。
- **红线**：未改生产代码与测试期望值 / 未 commit / 未出包 / 版本未动 / 零凭证。

## 2026-09-20 — tester-1 — BUILD-244 v0.9.2 二包（本地流式实时模型批次）✅ 八项核验逐项 PASS

- **放行**：TEST-EXEC-251 五项全绿 + 主控「现在可以出包」；HEAD `8354497`。**版本号维持 0.9.2（Gavin 明确「不升，0.9.2 一起出包」）**。
- **构建**：Step1 清进程（PID 25612）→ npm 665ms（新资产 `index-CipQtFxc.js`/`index-y2eVZ7cR.css`）→ Tauri UI 96s（17 warnings）→ 主程序 152s（bin 110 / crash-reporter 9）→ cp UI 入 `target/release/` → Step4 三 exe + 两 toml 入 `Publish/`。
- **八项**：①时间戳 main 20:50:36 / ui 20:48:05 / crash 20:49:21 ✅ ②两副本 sha 相等且全异于 BUILD-232 上包（main `e5807ccd…` / ui `99a15b02…` / crash `964a7163…`）✅ ③**ProductVersion 0.9.2 与上包相同系 Gavin 指示**，按替代判据「sha 异于上包」✅ ④冒烟 Responding=True ×2 + 无 crash.json + panic/ERROR 0（仅已知 `[ASR-DROP]` WARN）+ 残留 0 ✅ ⑤`config.toml` `18fe8608…` / `wordbook.sqlite` `b6ab43ac…` 零变化 ✅ ⑥warnings bin 110 / test 101 / src-tauri 17 = 新基线 ✅ ⑦探针（`grep -a -F`，无异常大命中）主 exe `local_realtime`=1 / `LocalRealtime`=5 / `streaming-paraformer-trilingual`=1，UI `local_realtime`=2 / `LocalRealtime`=1 ✅ ⑧scene `8ea93bb1…` / itn `311cbb96…` 三副本全等 ✅。
- **端测（交 Gavin）**：Ctrl+M 解锁「本地流式实时模型」；双模型已就位 `Publish/models/`（paraformer-trilingual 228MB + funasr-nano-int8 972MB）；清单 `collab/e2e-checklist-local-realtime.md` 十项，🔴 第 10 项现有三档零回归为红线。
- **证据**：`collab/outbox/tester-1/build244/`（prebuild_baseline / build_* / verify_all / verify_smoke / smoke_debug 等）。
- **红线**：版本号三处未动 / 未 commit / 零凭证。

## 2026-09-20 — coder-1 — FIX-LOCALRT-ENGINE-EQ-252 ✅ 交付（== Accuracy 漏认 LocalRealtime，待验收）

- **根因（Gavin 端测两 bug 同源）**：`asr_model=LocalRealtime` 未被三处 `== AsrModel::Accuracy` 认可（相等比较编译器不报错）。① `native_punctuated` 落 performance ⇒ 外部 CT-Transformer + accuracy 自带标点**双重打点**；② 长音频不分段 ⇒ 整段喂 native 撞 KV512 ⇒ **空输出**；③ VAD segmenter 不建。
- **改法**：`impl AsrModel` 新增 `pub fn uses_accuracy_engine(self)`（`matches!(Accuracy | LocalRealtime)`），三处收敛（mod.rs :253 / :455 / :582），:145 注释同步。
- **同族排查**：`main.rs:7071` load_hotwords（真 bug，改为 `!uses_accuracy_engine()`）、`:7469` 热重载条件（改）、`:9002` 日志文案（改）；`mod.rs:1627` 测试断言（不改）、`select_preprocessing_params`（239-A 已加 arm）；全仓无 `== AsrModel::Performance` 比较。
- **设计**：达成「用模型内标点」；「CT-Transformer 可不加载」本单不做（全局预载、涉三档，另行评估）。
- **验证**：cargo check --all-targets 0 error（warnings 110/101 = 基线）；rustfmt mod.rs / main.rs clean；未碰 src-tauri；未写测试（Gavin 指令，回归由 tester-1）。
- **自证三档零影响**：`uses_accuracy_engine()` 对 Performance/QwenAudioOnline/FunAsrRealtime 恒 false、Accuracy 恒 true，六处取值与改前逐位一致（新增命中仅 LocalRealtime）。
- **红线**：未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-20 — tester-1 — TEST-EXEC-253 回归 ✅ + BUILD-254 🛑 中止（产物作废）

- **回归全绿**：root **1267P/0F/15I**（=期望）+ `src-tauri` **78P/0F**；npm/browser/E2E SKIP（`ui/` 零 diff）；warnings 110/101/17 = 基线。
- **出包中止**：主控停令（FIX-252 + FIX-OVERLAY-SCROLL-255 + LOCALRT-PREVIEW-PUNCT-256 合并成一包）；停令时 Step1–4 已跑完，按令**作废、不再推进**。存档 sha main `ee6a1f10…` / ui `3b2e5dcd…` / crash `8a23b4e6…`（两副本相等，三者异于 BUILD-244 `e5807ccd…`/`99a15b02…`/`964a7163…`）。🔴 `Publish/` 当前暂存该作废包，合并包重出时覆盖。
- **探针发现**：任务书要求 `uses_accuracy_engine` 命中，实测 release exe **=0** —— Rust 方法名被内联/剥离，二进制探针不可构造；`git show 7e433ee` 无新增字符串字面量 ⇒ 应按 ⑦ 降级条款报三证。下包建议改测 255/256 的行为字符串。
- **证据**：`collab/outbox/tester-1/testexec253/`（cargo test 两日志）+ `build254/`（构建/核验日志）。
- **红线**：版本 0.9.2 三处未动 / 未 commit / 未新增测试 / 零凭证。

## 2026-09-20 — tester-1 — TEST-EXEC-257 回归 ✅ + BUILD-258 出包 ✅（四项修复合并包，八项 PASS）

- **回归全绿**：root **1267P/0F/15I** + `src-tauri` **78P/0F**；npm/browser/E2E SKIP（ui 零 diff）；两条护栏专项绿：`streaming_scroll_offset_contract`、护栏 9 `right_separator_geometry_matches_between_gdi_and_d2d`；warnings 110/101/17 = 基线。
- **出包**：Step1–4 全走（npm 654ms / Tauri UI 97s / 主程序 126s）；八项核验逐项 PASS：①时间戳 main 21:51:15 / ui 21:48:58 / crash 21:50:08 ②两副本 sha 相等且三者异于 BUILD-254 作废包（main `b8a3fa98…` / ui `8ab68022…` / crash `240fb14a…`）③ProductVersion 0.9.2 与上包相同系 Gavin 指示，按替代判据「sha 异于上包」④冒烟 Responding=True ×2 + 无 crash.json + panic/ERROR 0 + 残留 0 ⑤`config.toml` `18fe8608…` / `wordbook.sqlite` `b6ab43ac…` 零变化 ⑥warnings 110/101/17 ⑦探针（字符串字面量）`sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en`=1 / `local_realtime`=1 / `asr_local_realtime_unlocked`=1 ⑧scene `8ea93bb1…` / itn `311cbb96…` 三副本全等。
- **探针口径**：改字符串字面量；`uses_accuracy_engine` 命中 0 属方法名内联（已立档 `[BINARY-PROBE-SYMBOL-INLINED-001]`）。`Publish/models/` 未触碰。
- **端测（交 Gavin）四复验**：①标点不重复 ②长音频出字 ③流式右侧无留白（⚠️ 在线 FunASR 档同生效，修复非回归）④本地流式预览有标点；完整十项见 `collab/e2e-checklist-local-realtime.md`。
- **证据**：`collab/outbox/tester-1/testexec257/` + `build258/`。
- **红线**：版本 0.9.2 三处未动 / 未 commit / 未新增测试 / 零凭证。
