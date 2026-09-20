# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行，超 200 行上限）。

## 2026-09-20 — coder-2 — LOCALRT-FIRSTCHAR-272 ✅ 交付（首字延迟埋点，不改解码参数；待主控定向 tester-1 冷/热实测）

- **埋点**（`local_stream.rs`，永久观测、仅首次触发）：`first_chunk` / `first_accept_waveform` / `first_is_ready` / `first_nonempty_result` / `first_on_result`（=用户看到首字）+ `FIRST-CHAR breakdown` 汇总行（chunk/accept/ready/result/callback + wait_audio / wait_infer / wait_callback）。t0 = 函数入口（ASR 线程起跑）。
- **不改任何解码参数**（先量后调）。**按 Worker 边界，release 构建 + 麦克风运行时验证由 tester-1/Gavin 执行**，我据 `debug.log` 出拆解表。
- **可调项候选（只读证据）**：① 等音频 → `PRE_ROLL_MS=600`（`audio/mod.rs:23`）、idle drain `cleared` = 热键前 stale（丢弃正确）、chunk ~10ms；② 等推理 → `num_threads=4`（RTF 最优非首字最优，可试 1/2/8）、provider=cpu、encoder chunk 烘进模型不可调；③ 等回调 → 事件 unbounded + `PostMessageW`、overlay 100ms 节流对增长宽度绕过（`main.rs:1644`）。
- **验证**：cargo check --all-targets 0 error、warnings **110/101** = 基线；rustfmt clean；`RULE1/2/3` 一字未动。
- **红线**：只改 `local_stream.rs` / 未碰 src-tauri / ui / 未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-20 — coder-2 — LOCALRT-PUNCT-TIMER-269-B ✅ 交付（预览打点补静默 800ms 触发；待主控验收 → tester-1 回归）

- **两条件「或」**：① 距上次 ≥4s（269 保持）② 静默 ≥800ms（本单）⇒ 全量重打。
- **🔴 独立静默计数**：`local_stream.rs` 内每 chunk `RMS ≤ silence_threshold` 累计、有声清零；**与 sherpa endpoint 完全独立**，静默触发只刷新显示，**不 reset / 不切句 / 不动 sentence_id**（`rule2=2.0` 切句路径零改，说话中间换气不被切）。`RULE1/2/3` 三常量一字未动。
- **有新内容才打**：`preview_display` 加 `has_new` 门控；持续静默（停 5s）只打一次。
- **签名**：`transcribe_streaming_local` 加 `silence_threshold: f32`；`main.rs` 新分支传 `config.audio.silence_threshold`。
- **验证**：cargo check --all-targets 0 error、warnings **110/101** = 基线；rustfmt 两文件 clean；`--numstat`==`-w`。🔴 未跑 cargo test（回归归 tester-1）。
- **红线**：只改 `local_stream.rs` + `main.rs` 新分支一行调用 / 未碰 src-tauri / ui / 未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-20 — coder-2 — LOCALRT-PUNCT-TIMER-269 ✅ 交付（预览标点 ~4s 全量重打；待主控验收 → tester-1 回归）

- **问题**：预览标点只在 `endpoint=true`（静音≥2s / 满 20s）出现，连续说话等不到。
- **实测**（临时 bench 后删，未入库）：`add_punctuation`（CT-Transformer 单线程 CPU）中位 20/50/100/200/400 字 = **3.6/8.2/15.6/30.0/58.4ms**（≈0.15ms/字）⇒ `PUNCT_REFRESH_INTERVAL=4000ms`，400 字仅 ~58ms/4s，RTF 影响 <1.5%。
- **改法**：`preview_display(raw_full, engine, cache, interval, force)`——到点对**原始全文整体重打**；两次之间「上次标点 + raw 新增后缀」。🔴 状态机始终喂**裸文本**、标点只作用于显示 ⇒ 不重复打点（FIX-252 不复发）。引擎 None 直接返回 raw（零开销）；引擎仍由调用方传入；收尾强制打点一次。不影响最终文本。
- **验证**：cargo check --all-targets 0 error、warnings **110/101** = 基线；rustfmt clean；`--numstat`==`-w`（96/17）。🔴 未跑 cargo test；不写新测试；临时 bench 已删。
- **红线**：只改 `src/transcription/local_stream.rs` / 未碰 src-tauri / ui / main.rs / 未动版本 / 未 commit / 未出包 / 零凭证。

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

## 2026-09-20 — coder-1 — RESEARCH-ACC-KV-BUDGET-259 ✅ 交付（KV 512 取证，只取证）

- **任务一 KV 边界**：DLL 1.12.38/SHA aacfe96f。热词 20/30/40/60/100/200 × 短 9.45s + 长 52s（20s 硬切三段）。**短音频 N≤40 OK；20s 段 N=30 溢出**（`Context_len (536)>512. Truncating audio placeholders`→空）；**N≥60 短音频崩；N≥100 → `Falling back to keep last 512 tokens`（跳过音频，最坏静默失效）**。短 audio_token_len≈158 / 20s 段≈333 ⇒ 分段后预算更小，「分段提上限」不成立。
- **任务二 max_new_tokens**：源码默认 512 / Validate 要求>0；实测 1→「飞」2→「飞音」**0→完整（=不限）**128/256 同 0。建议显式 **256**。
- **任务三 筛词**：scene 有分类无映射；wordbook **无频次列**（candidates 有 count/last_seen）；优先级 频次>场景>上一句。
- **任务四 system_prompt A/B**：短音频三组逐字相同，长段无一致增益 ⇒ **不改**（DEC-059 需更大样本）。
- **任务五 KV 提升**：ModelScope `zengshuishui/FunASR-nano-onnx` 有 `llm_int8_max_token_768/1024`（各 1 个 ~600MB llm.int8.onnx）**零开发可换**；KV 每 512=112MiB(28层×2×8×128×4B)，1024→+112MiB（1.6GB 约+7%）。**未下载验证**（红线）。
- **建议汇总**：HOTWORDS_MAX_ENTRIES 保持 **20**；max_new_tokens 显式 **256**；system_prompt 暂不改；KV 提升候选换 1024 变体。
- **改动**：仅 PoC `src/bin/poc_funasr_nano.rs` 加 `--system-prompt`；生产零改动。验证 `cargo check` 0 error（110/101 基线）。
- **红线**：未改生产常量/生产代码 / 未动版本 / 未 commit / 零凭证。

## 2026-09-20 — coder-1 — RESEARCH-ACC-KV-BUDGET-259-B ✅ 交付（补单两缺口，只取证）

- **缺口①（成立）**：N=20 全 10 字热词（219 字符）→ 20.4s 段 `context_len=553`、`Truncating` → 空；对照 20 短词（101 字符）不溢出。⇒「N=20 安全」**只在短热词下成立**，真实约束是**热词总字符(token)数**。
- **缺口②（成立）**：源码 `valid_len=context_len`（impl:614）、每生成 token `valid_len+=1`（:766）、`>=max_seq_len` break（:628）⇒ **生成与输入共享 512**。实测同段 k=8/10/11/12/13/14 输出 80/74/58/46/32/18 字符、k=20 溢出空；斜率≈−10 字符/条（≈1:1），表现为**长句截断（无句末标点）非空输出**。
- **修正建议**：热词按**总字符**限（保守 ≤100）而非仅条数；热词与生成零和；`max_new_tokens` 显式 256；KV 提升候选换 `llm_int8_max_token_1024`。
- **路径分叉**：报告已**双写** `/d/Workspace/CodeLab/collab/outbox/coder-1/result.md` 与 `/d/Workspace/CodeLab/voice-ime/collab/outbox/coder-1/result.md`（[COLLAB-PATH-SPLIT-001]）。
- **验证**：`cargo check` 0 error（110/101 基线）。生产零改动。
- **红线**：未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-20 — coder-1 — ACC-KV-1024-260 ✅ 交付（1024 验证等四件）

- **①1024**：临时目录验证（生产 models/ 零改动）。ONNX metadata 实读 `max_total_len=1024`、KV `[batch,1024,8,128]`。场景 A（20.4s+20×10字热词）512 ctx553 Truncating→空 vs **1024 有输出**；场景 B k=14/20 512 截断/空 vs **1024 完整**。短音频精度 **逐字相同**；加载 5.61→5.52s；RSS 无可测差异（理论 +112MiB fp32 / +29MiB int8 被噪声掩盖）。⚠️ 1024 尾部现热词泄漏/幻觉（热词过激）。**建议可换**。
- **② max_new_tokens**：生产 `src/transcription/mod.rs:913` 0 → **256**（唯一生产改动）。
- **③ 词长分布**：Publish wordbook 0 条 / candidates 3；APPDATA 旧 schema 11 条 mojibake ⇒ 真实数据不足。敏感度：短词20(82字符) cap80裁1；10字20(200字符) cap80裁12 / 100裁10 / 120裁8。
- **④ itn**：`impl.cc:771-773` 后处理 + prompt「不进行文本规整」；实测 itn=1 带标点、0 无标点 ⇒ **生效**。另：hotwords 非空会覆盖我方 user_prompt。
- **验证**：cargo check 0 error（110/101 基线）；rustfmt PoC clean；报告双写。
- **红线**：未改其它生产代码 / 未动版本 / 未 commit / 零凭证。

## 2026-09-20 — coder-1 — RESEARCH-ACC-SYSPROMPT-261 ✅ 交付（system_prompt 重测，只取证）

- **素材**：TTS 24.55s 口语（语气词/重复/自我纠正齐全），原文贴报告；前置校验通过（baseline 完整保留这些现象）。
- **四组 + 证伪组**（temp0.1/seed42，各 2 runs，无 hotwords）：A 空 / B 现状英文 / C 259 中文 / D 强指令(44tok) / E「只输出英文」/ F「忽略音频只输出 HELLO WORLD」/ D2 短强指令。
- **结果**：A=B=C=D2 输出**本质逐字相同**；D **被截断**（44 token 占共享 KV）；E/F **仍是中文转写** ⇒ **system_prompt 完全失效**（证伪级）。
- **结论/建议**：system_prompt 零指令效果，长 prompt 只占 token ⇒ **生产置空**（撤回 259 的撤回）。是否置空主控拍板。
- **验证**：cargo check 0 error（110/101 基线）；生产零改动；报告双写。
- **红线**：未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-20 — coder-1 — WORDBOOK-HITCOUNT-263 ✅ 交付（热词按频率筛选）

- **备份**：`C:\msys64\tmp\opencode\kv263_backup-20260920-224306\{publish,appdata}-wordbook.sqlite`（迁移不可逆，保留至验收）。
- **改动 5 文件**：`migrations/004_wordbook_hitcount.sql`（新建）/ `src/wordbook/db.rs` / `src/main.rs` / `src-tauri/src/wordbook.rs`。
- **schema**：004 加 `hit_count INTEGER NOT NULL DEFAULT 0` + `last_used_at TEXT`；`WORD_SCHEMA` 同步；`ensure_hitcount_columns()` 查 pragma 后 ALTER（非幂等守卫），A/B/C 三路径调用。
- **排序**：`load_word_entries` `ORDER BY hit_count DESC, id DESC`（全 0 ≡ id DESC，零回归；tiebreak 防 hotwords_version 抖动）。
- **计数**：`db.rs::record_hits(final_text)` 单事务批量；`main.rs` 注入成功后调用，gate `uses_accuracy_engine()`；用 final_text。
- **UI**：Tauri `get_wordbook_entries` 按 id DESC 重排，显示顺序不变。
- **影响四条**：① Performance/在线三档零影响（不用 hotwords）② 全 0 版本哈希逐位不变、无抖动 ③ UI 顺序无变化 ④ 旧库 001→003→finalize→004 链路 SQL 实测可执行，列与全新库一致。
- **验证**：列 parity IDENTICAL、0/5/2→5,2,0、全 0 == 旧顺序、record_hits 语义；cargo check root 110/101 / src-tauri 17。
- **红线**：未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-20 — coder-1 — 263 增补 + ACC-KV-1024-SWAP-262 ✅ 交付

- **263 增补**：`main.rs` record_hits → `std::thread::spawn` fire-and-forget（消除 BUSY 3s 挡 worker）。实测读库（同 SQL）冷 1.48/1.67/1.62ms、热 0.05/0.18/0.41ms（0/100/500 条）；写事务 5 条 ~8ms。读路径在录音前、不在识别链路。缓存层评估：收益 <2ms 暂不做（若做用 mtime 失效）。
- **262 换 1024**：全工作区 **2 副本**（`models/`、`Publish/models/` 的 `sherpa-onnx-funasr-nano-int8-2025-12-30/llm.int8.onnx`）；原文件改名 `.512.bak`（600,339,316 B）保留；替换为 600,025,528 B（metadata max_total_len=1024）。**一处一验**：加载 6.40/5.77s 无 panic + 短音频出字且两副本**逐字相同**。`.gitignore` `/models`+`/Publish/*` 覆盖。场景 A 不再溢出 / B k=14 完整。
- **system_prompt 置空**：`transcription/mod.rs` `Some("You are a helpful assistant.")` → `Some(String::new())`（261 证伪组依据，零行为变更）。
- **验证**：cargo check 0 error（110/101 基线）；rustfmt clean。
- **待主控**：`.512.bak` 是否删；端到端 app 启动/热键链路归 tester-1。
- **红线**：未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-20 — coder-1 — 259~263 判据固化草稿 ✅ 交付（纯文档）

- **产出**：`collab/decisions-draft-localrt-259.md`，5 条判据（建议 DEC-068~072）+ 建议索引行，全文待主控定号后并入 `decisions-archive.md` 与 `decisions.md`。
- **内容**：①KV512 四者共享零和 ②撞顶静默 Truncating/Falling back ③system_prompt 完全不被遵循（261 E/F）④热词约束=总字符非条数 ⑤换1024 后 20s 段预算 149→661。
- **红线**：纯文档，未碰生产代码/版本；零凭证。

## 2026-09-20 — coder-1 — DEC-068~072 并入 ✅ 交付（纯文档）

- **两层并入**：全文入 `collab/decisions-archive.md`（5 条），索引入 `collab/decisions.md` 现行表。
- **主控指定改动**：DEC-072 标注「30 为估算，含模板+user_prompt，system_prompt 已置空」；DEC-069 索引行点明「判越界看日志关键字 Truncating/Falling back，不是有无输出」。
- **草稿删除**：`decisions-draft-localrt-259.md` 已删（避免两处真相漂移）。
- **红线**：纯文档；未碰生产代码/版本（遵「TEST-264 前不改生产代码」）；零凭证。

## 2026-09-20 — tester-1 — TEST-SYNC-264 ✅ 交付（263 阶段三护栏 7 条，只写用例未执行，待主控验收 → 阶段四）

- **改动**：`src/wordbook/db.rs` `#[cfg(test)] mod tests` 纯追加 **+7 用例**，生产代码零触碰；`git diff --numstat` == `--numstat -w`（359/3）。
- **四组**：① 🔴 `hitcount263_all_zero_ordering_is_id_desc`（全 hit_count=0 → id DESC，= 263 改动前逐位相同；丢 tiebreak 即红，附 id ASC 反向断言）② `hitcount263_ordering_hits_desc_then_id_desc`（甲0/乙5/丙2/丁2 → [乙,丁,丙,甲]，含 tiebreak）③ `hitcount263_ensure_hitcount_columns_idempotent`（连跑两次 + 补列后数据可写）④ `hitcount263_fresh_vs_migrated_table_info_identical`（PRAGMA 逐字段 name/type/notnull/dflt_value/pk + 顺序，含新列两侧存在性）；record_hits 三条：`..._increments_only_matched_words` / `..._empty_text_and_empty_db_ok` / `..._same_word_twice_counts_once`。
- **🔴 实现要点（须主控知悉）**：`load_word_entries`/`record_hits` **无 `_in_conn` 缝**，复制 SQL 的断言看着像护栏实则不跟生产走；故按 `db_path()` 同源取测试 exe 同级文件库、真实调用 public 函数，`static FILE_DB_LOCK` 串行化。副作用：`target/debug/deps/wordbook.sqlite`（构建产物目录）。**建议 coder 补 `*_in_conn(&Connection)` 变体**（照 `upsert_candidate_in_conn` 先例），后续护栏可纯 in-memory、零文件副作用。
- **record_hits 实际语义**（按令「断言实际行为」）：同词条在同段文本多次出现 = **+1**（每去重词条一条 UPDATE，与 `contains` 次数无关）；子串匹配非分词；空词条过滤。
- **验证（白名单内）**：`rustfmt --check` clean；`cargo check --all-targets` 0 error、warnings **110/101** = 基线；`cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error（bin 17 / test 19）。🔴 未跑 `cargo test`（阶段三禁）。
- **阶段四预期**：root **1274P/0F/15I**（+7）、src-tauri **85P/0F**（+7）；🔴 额外**独立复验**换 1024 后 accuracy 档现有管线行为不变（不复用 coder-1 结论）。
- **红线**：未改生产代码 / 未执行测试 / 未 commit / 版本未动 / 零凭证。证据 `collab/outbox/tester-1/testsync264/`。

## 2026-09-20 — tester-1 — TEST-EXEC-264 ✅ 阶段四回归全绿 + 1024 独立复验通过

- **回归逐 target**：root **1274P/0F/15I**（EXIT 0，=预期；main 1186 / crash-reporter 52 / int 36）；`src-tauri` **85P/0F**（=预期）；UI Vitest **100P/0F/11S**；browser/E2E SKIP。warnings root bin 110 / test 101、src-tauri `cargo check` 17 = 基线。
- **+7 落点吻合**：main 1179→1186、src-tauri 78→85（阶段三 7 条 db 护栏经 `#[path]` 双编入两 crate）。
- **🔴 额外必验 (1) 1024 短音频**：自选 `dia_hunan.wav`(7.01s)→「他总的来讲，孙膑对兵法的理解、文样比庞涓略胜一筹。」、`rag_physics.wav`(12.12s)→「根据碰撞理论，月面样本缺少挥发性物质。」—— 出字 + 标点齐全，RTF 0.16/0.08，加载 5.803s。
- **🔴 额外必验 (2) 长音频 37.25s**：自建 `lyrics_en_3+far_2+rag_math` 拼接；输出覆盖全三段、末句以「。」收尾（= rag_math 原句）**不截断**，RTF 0.2141。口径：poc 无 VAD（生产 ≤20s 分段）⇒ 本测**更严**。
- **262 模型核对**：两副本 `llm.int8.onnx` sha `c326cdeb…` 相等（600,025,528B）；两处 `.512.bak` **保留未删**（600,339,316B）；`max_total_len` 元数据命中。
- **独立边界**：未复用 coder-1 素材/结论；未重跑 512↔1024 逐字差分（需 ~1GB 临时副本）。
- **证据**：`collab/outbox/tester-1/testexec264/`（cargo_test_root / cargo_test_tauri / npm_test / asr_1024_run / model_and_check）。
- **红线**：未改生产代码 / 未删 `.512.bak` 与 wordbook 备份 / 未 commit / 版本未动 / 零凭证。

## 2026-09-20 — coder-1 — HOTWORDS-BUDGET-265 ✅ 交付（热词上限 120 + 字符预算 500）

- **改动 1 文件** `src/transcription/mod.rs`：`HOTWORDS_MAX_ENTRIES` 20→120；新增 `HOTWORDS_MAX_TOTAL_CHARS=500`（算式注释：1024−333−24=667，留200给生成≈560字符取500）；`curate_hotwords_entries` 累加含逗号字符、超预算 **break**。
- **单测 18/18 PASS**（120 短词全进 359；120 长词截 45/494；边界 500 保留・501 丢弃）。
- **实测**：20s 音频上 120 短词与预算后长词均**无 Truncating/Falling back** 且输出完整；未预算 120 长词（1319）Truncating 空（对照）。
- **⚠️ 待裁定**：500 下 2–3 字可容 120 条，4 字×120=599 → 截到 100。若要 4 字全进需预算 ≥600。
- **验证**：cargo check 0 error（110/101 基线）；rustfmt clean；未跑全量（归 tester-1）。
- **红线**：未动版本；备份未动；零凭证。

## 2026-09-20 — coder-1 — WORDBOOK-UI-SORT-266 ✅ 交付（并入 265 同批）

- **改动 1 文件** `src-tauri/src/wordbook.rs`：删 `entries.sort_by(|a,b| b.id.cmp(&a.id))`（263 的 UI 稳定排序），改注释为依据 Gavin 2026-09-20 指示按频率排序、底层 SQL 已 `hit_count DESC, id DESC` 直接透传；`mut entries`→`entries`。
- **检查**：src-tauri 无 id DESC 断言；db.rs 排序断言属 DB 层未动；前端 filter 保序 ⇒ 两 tab 自动正确。
- **验证**：src-tauri cargo check 0 error（17 基线）；rustfmt clean；`npm run test -- Wordbook` 13/13 PASS。
- **红线**：未动版本；备份未动；零凭证。

## 2026-09-20 — coder-1 — 265 600 裁定 ✅ 交付

- **变更**：`HOTWORDS_MAX_TOTAL_CHARS` 500→**600**（主控裁定；算式 1024−340−24−500=160 给生成；让 4 字词装满 120 条）。
- **单测**：新增 4字×120 全进(599)；10字×120 截 54(593)；边界正好600保留/601丢弃；`-- curate` 全 PASS。
- **实测**：常用字 4×120(599) 无 Truncating/Falling back；**生僻字** 4×120(599) → context 1289 Truncating 空 ⇒ **char≠token 边界**（生僻 CJK byte 回退 ≈3tok/字），字符预算为近似闸门非硬上界。
- **验证**：cargo check 0 error（110/101）；rustfmt clean；未跑全量（归测试阶段）。
- **红线**：未动版本；备份未动；零凭证。

## 2026-09-20 — coder-1 — HOTWORDS-TOKEN-268 评估 ✅ 交付（方案建议，未改代码）

- **结论：选 B**（`tokenizers` 真实编码）。`tokenizers = "0.22.1"` 已在 Cargo.toml:27 ⇒ **无新依赖**（主控头号顾虑不成立）。
- **A 否决**：vocab.json 是 **byte-level BPE**（'北' in vocab=False）→ CJK 全部查表失败按字节 → common4×120 估 1559 vs 真值 360（4.3×）→ 只 38 条。
- **中间档否决**：字节词表贪心对 CJK 失效 → rare4×120 估 599 vs 真值 783 低估 → 实际 623>500 溢出（违保守判据）。
- **B 数据**：common4×120=360 全进 / rare4×120=783 截 79 / mixed1/3=476 全进；加载 375.8ms（OnceLock 懒加载）。
- **校准**：C++ 与 Rust tokenizer 可能差 ~1.22× → B 计数加安全边际，实现阶段用实测系数定。
- **待出包后实现**（BUILD-267 期间不改生产代码）；本单零生产改动。

## 2026-09-20 — tester-1 — BUILD-267 出包 ✅（263/262/260/265/266，八项 + 三条额外全 PASS）

- **构建**：Step1–4 全走（npm 749ms / Tauri UI 109s / 主程序 143s）；三 exe + 两 toml 入 `Publish/`；本批 `ui/` 零 diff（266 为后端 `src-tauri/src/wordbook.rs` 去 UI 重排、透传 `hit_count DESC`）。
- **八项**：①时间戳 main 23:17:59 / ui 23:15:26 / crash 23:16:43 ②两副本 sha 相等且三者异于 BUILD-258（main `db7c7878…` / ui `01090dc2…` / crash `85e7c5cf…`）③ProductVersion 0.9.2 与上包相同系 Gavin 未升版，按替代判据「sha 异于上包」④冒烟 Responding=True ×3 + 无 crash.json + panic/ERROR 0 + 残留 0 ⑤`config.toml` `da2be5da…` / `wordbook.sqlite` `b6ab43ac…` 零变化 ⑥warnings 110/101/17 ⑦探针（`grep -a -F`）`hit_count = hit_count + 1`=1 / `ORDER BY hit_count DESC, id DESC`=1 / `local_realtime`=1 / `asr_local_realtime_unlocked`=1 ⑧scene `8ea93bb1…` / itn `311cbb96…` 三副本全等。
- **额外①**：`Publish/models/.../llm.int8.onnx` = 600,025,528 B（1024，sha `c326cdeb…` = 源副本），**非** `.512.bak` 600,339,316。
- **额外②**：两处 `.512.bak` **保留未删**；`Publish/` 无 zip/setup 产物，`voice-ime.iss` [Files] 不含 funasr-nano 目录 ⇒ 现成安装包不带它；⚠️ **整目录打包 `Publish/` 会带上 600MB 备份**，出 zip/安装包时须排除（未擅自移动/删除）。
- **额外③**：真实 exe 冒烟驱动 DB 迁移三态全通 —— 新库列 = `id,word,source,created_at,hit_count,last_used_at`；旧库 raw→word + `auto→system` 归一 + 候选 `count=3` 保留 + 新列补齐；状态 C 补列正常；全程无 `no such column`/`duplicate column`/panic。测试前备份并**原样恢复** `target/release/{config.toml,wordbook.sqlite}`。
- **证据**：`collab/outbox/tester-1/build267/`（prebuild_baseline / verify_all / extra12 / extra3_fresh|legacy|restore / verify_smoke）。
- **红线**：版本 0.9.2 三处未动 / 未 commit / 未改生产代码 / `.512.bak` 与 wordbook 备份未删 / 零凭证。

## 2026-09-20 — coder-1 — 268 追踪：951 构成判别 ✅（情况①）

- **判别**：104s 音频强制溢出取 C++ `before`，4 组差值 168/168/**293**/173 ⇒ 不恒定 ⇒ **情况① tokenizer 不一致**（C++ 手搓 byte-level BPE vs tokenizer.json 管线），热词 token 比 1.15–1.66×，Rust `tokenizers` 低估。
- **连带发现**：4 字常用×120 before=653 → 20s 段 context 991≤1024 无 Truncating 但只剩 33 token 生成 ⇒ **生成饿死无日志**，验证判据须加「生成未饿死」；且 120×4 字与 1024 预算冲突。
- **建议**：首选 Rust 复刻 C++ 分词；否则按 C++ 真计数重定预算/降目标。**待主控裁定**，出包后实施。
- **红线**：零生产改动；未动版本；零凭证。

## 2026-09-20 — coder-1 — HOTWORDS-TOKEN-268 ✅ 交付（token 预算 + 保守系数）

- **改动 1 文件** `src/transcription/mod.rs`：`HOTWORDS_MAX_TOTAL_TOKENS=356`；`HOTWORDS_CPP_SAFETY_FACTOR=1.85`（实测 C++/Rust 比 1.15–1.66，最大×1.1）；`HOTWORDS_MAX_TOTAL_CHARS` 删除；`curate_hotwords_entries_with` 用 tokenizers 真实编码×系数、超 356 break；tokenizer 惰性加载 + 启动预热（Rust 加载 429.1ms）；加载失败退化 UTF-8 字节上界。
- **实装条数**：2字 75 / 3字 54 / 4字常用 71 / 4字生僻 33 / 5字 51（系数保守，2字也未达 120）。
- **验证**：单测（预算不变式/极大/边界/回退/顺序）全 PASS；真实 20s 音频 5 组生产参数下无 Truncating/Falling back、输出完整末句有标点；cargo check 0 error（110/101 基线）；rustfmt clean。
- **待裁定**：若需更多词可改按词长分档系数。
- **红线**：未动版本；备份未动；零凭证。

## 2026-09-20 — coder-1 — DEC-073 / DEC-074 并入 ✅（纯文档）

- **DEC-073**：静默失效两级（一级输入溢出有日志 `Truncating`/`Falling back`；二级输入没溢出但生成空间被挤干、**完全无日志**）⇒ 判据须同时看截断日志与生成剩余空间；**修订 DEC-069**（archive 069 节末加指向行）。证据：4字常用×120 context=991≤1024 无日志但生成仅 33 token。
- **DEC-074**：C++ 手搓 byte-level BPE + 模拟 pre_tokenizer 正则 ≠ tokenizer.json（CJK），比值 1.15–1.66 随内容变 ⇒ Rust 计数乘保守系数 1.85，上游变更重标定。
- **落档**：全文 archive + 索引 decisions.md；logs/CHANGELOG 同步。
- **红线**：纯文档；未碰生产代码；未动版本；零凭证。

## 2026-09-20 — tester-1 — TEST-EXEC-270 ✅ 268+269 全量回归全绿（⚠️ 仅回归，主控令未出包）

- **主控调整令**：269-B 正由 coder-2 改 `local_stream.rs`（Gavin 明确标点触发：静默 800ms / 说满 4s），本轮 269 相关作废，**不出包**；268 部分有效。
- **全量回归**：root **1279P/0F/15I**（EXIT 0；main 1186→1191 = +5，全为 268 的 `curate_*` token 预算用例）；`src-tauri` **85P/0F/0I**；Vitest **100P/0F/11S**；warnings root 110/101、src-tauri check 17 = 基线。
- **268 额外验 ①**（accuracy 未被改坏，独立素材）：短音频 + 生产预算热词（71 条常用 4 字 / est 354 token）→ `dia_hunan`「他总的来讲，孙膑对兵法的理解、文样比庞涓略胜一筹。」、`far_2` 正常出字带标点。
- **268 额外验 ③**（生成未饿死 / DEC-073）：37.25s 无热词 → 完整带「。」；+20 条(~99字) → 完整带「。」；**+71 条(生产预算) → 空输出 + sherpa `Reduce hotwords` 警告**；**21.8s `far_3` +71 条 → 完整带标点**。⇒ 生产 VAD（≤20s）+ naive_chunk 落在安全区；**超 VAD 长度的单发**满预算会饿死生成。⚠️ 已报主控。
- **269（作废待复验）**：`preview_display` 私有无单测；`poc_local_stream` 自实现流式、不加载 `local_stream.rs` ⇒ LocalRealtime 预览标点无自动化入口（已报备，269-B 后可补缝或端测）。加验素材已备（800ms 触发 / 句中停 1s 不被切开）。
- **未执行**：出包 SKIP（按令）；消融未派。
- **证据**：`collab/outbox/tester-1/testexec270/`（cargo_test_root / cargo_test_tauri / npm_test / check_tauri / curate_268_tests / asr_268_* ）。
- **红线**：未改生产代码 / 未 commit / 未出包 / 版本未动 / 零凭证 / 备份未删 / 未干扰 coder-1 的 models/kv259 实验。

## 2026-09-20 — coder-1 — RESEARCH-ACC-LATENCY-271 ✅ 交付（只取证）

- **根因（实测）**：`create_funasr_nano_recognizer` 的 `num_threads` 默认 0；`session.cc:134-147` 原样透传 ORT，**0 实测≈单线程**（vs 1 线程几乎同耗时）。⇒ accuracy 一直单线程。
- **线程表（中位数）**：short 0/1/2/4/8=2.625/2.704/1.914/**1.610**/1.682；long=30.885/31.118/20.903/22.350/**17.017**（CPU 8核16线程）⇒ 4–8 最优。
- **③ 512 vs 1024**：short 一致（1.618/1.637）；long 512=0.772s **溢出→0 token 空** vs 1024=15.598s（134 token）⇒ **1024 不慢**，排除。
- **④** max_new_tokens=256 未跑满（26/134 token，EOS 停）。**⑤** VAD 分段串行；并行需多实例 ~1.6GB，不建议。
- **建议**：设 `num_threads=8`（1 行、零内存、long ~1.8×）；可选透传 provider "cpu"；不动 1024/max_new_tokens。
- **红线**：只取证；临时 512 对照目录已删；未动生产模型/版本；零凭证。

## 2026-09-20 — coder-1 — 273 正确性 + 271 num_threads 实施 ✅

- **273**：`should_segment > 24.0` ⇒ 生产上界 24s≈400token。268 满预算热词（est354）跑 21.8/23.0/23.9/24.1s 全无 Truncating/Falling/Reduce；23.9s generated 57 token、输出完整有标点 ⇒ **无需降 356→296**。23.0s 无标点=音频自身词中被切。**273-B**：265 字符预算 600（653 C++token）24s → Context 1056>1024 Truncating→空 ⇒ **BUILD-267 已知雷成立**，268 修复有效须进下包。
- **271**：`create_funasr_nano_recognizer` 加 `num_threads=min(available_parallelism,8)` 兜底 4 + `provider="cpu"`；注释含实测表/机型/根因/取舍。预期 long 30.9→17.0s (~1.83×)。
- **未动**：1024 / max_new_tokens / VAD 并行。
- **验证**：cargo check 0 error（110/101）；rustfmt clean。
- **红线**：未动版本；备份未动；零凭证。

## 2026-09-20 — coder-1 — RESEARCH-ACC-GPU-275 ✅ 交付（关卡一未过，按令停工）

- **关卡一 不通过**：sherpa-onnx 官方 releases 无 directml/dml 资产（win-x64 仅 CPU/CUDA）；源码 `session.cc:27/303` 证 DirectML 是编译期开关（`SHERPA_ONNX_ENABLE_DIRECTML`），未编译则 `provider=directml` 静默回退 CPU。⇒ 要 GPU 必须自编 sherpa-onnx+DirectML ORT（Windows 源码编译无底洞先例）→ **停工报告，不编译**。
- **关卡二 通过**：`llm_fp16/llm.fp16.onnx` = **1.19GB**（int8 600MB 的 2×）；`llm_fp32`≈2.38GB。
- **结论**：GPU 不值得走（硬阻塞=无预编译 DirectML；780M 共享内存无带宽优势；fp16 整包 ~2.74GB）；271 num_threads 已给 ~1.83× 零成本替代。重提前置：官方出 DirectML 包 或 独显机。
- **红线**：未碰生产 sherpa-onnx-lib/models；未动版本；未 commit 二进制；零凭证；无大文件下载。

## 2026-09-21 — tester-1 — TEST-EXEC-274 + BUILD-274 ✅ 回归全绿 + 出包（八项 PASS，⏸ 272 延迟待录音）

- **回归**：root **1279P/0F/15I** + `src-tauri` **85P/0F** + Vitest **100P/0F/11S**；warnings root 110/101、src-tauri check 17 = 基线。
- **重点验 ①**（271 未改坏 accuracy，独立对照）：`poc_funasr_nano --threads 0`(旧=单线程) vs `--threads 8`(新) 同音频 —— `far_3` 文本**逐字相同** 7.308→4.004s；`rag_physics` **逐字相同** 2.134→1.038s。
- **重点验 ②**（269-B 未动切句）：`LOCAL_STREAM_RULE1/2/3 = 2.4/2.0/20.0` 跨 `277b2fc`→`ace0786` **逐字相同**；269-B 对 `reset()`/`sentence_id`/`endpoint` 零新增（仅注释+显示层独立静默计数）。🔴 行为级「句中停 1s 不切句」需真人发声——`transcribe_streaming_local` 无测试缝、`poc_local_stream` 用自己的 rule2=1.2 不代表生产 ⇒ 交 Gavin 端测（未造假）。
- **重点验 ③**（标点不重复）：结构上 `on_result` 恒喂 `r.text` 裸文本、`display_text()` 恒裸、标点仅 `preview_display` 作用于显示不回灌；间接实测流式 paraformer 原始输出无标点。🔴 视觉级留端测。
- **重点验 ④**（生成未饿死）：`far_3` 21.8s +71 条 / +20 条、自建 27.3s +71 条 / 无热词 —— 全部**完整、末句「。」、无 `Truncating/Falling/Reduce`**。
- **BUILD-274**：Step1–4 全走（npm 797ms / Tauri UI 112s / 主程序 165s）；三 exe + 两 toml 同步 `Publish/` 与 `target/release/`。八项逐项 PASS：①时间戳 main 00:07:31 / ui 00:04:40 / crash 00:05:52 ②两副本 sha 相等且三者异于 BUILD-267（main `40ccd5ea…` / ui `c788acf0…` / crash `21cb7d8a…`）③ProductVersion 0.9.2 不变（Gavin 未升版，按 sha 替代判据）④冒烟 Responding=True + 无 crash.json + panic/ERROR 0 + 残留 0 ⑤`config.toml` `da2be5da…` / `wordbook.sqlite` `b6ab43ac…` 零变化 ⑥warnings 110/101/17 ⑦探针 4 串命中 ⑧scene `8ea93bb1…` / itn `311cbb96…` 全等。
- **额外**：`Publish/models/.../llm.int8.onnx` = 600,025,528（1024，`c326cdeb…`）；`.512.bak` 保留未删、无 zip/setup 产物。⚠️ 主程序 +1.9MB（268 引入 `tokenizers` crate，预期）。
- **⏸ 未执行**：**272 首字延迟实测**（需真人发声，无法执行；前置已备：两处新产物 + `target/release/debug.log`，等 Gavin 配合录音）。
- **证据**：`collab/outbox/tester-1/testexec274/` + `build274/`。
- **红线**：版本 0.9.2 未动 / 未 commit / 未改生产代码 / 备份未删 / 零凭证。

## 2026-09-20 — coder-1 — DEC-075 + PROVIDER-SILENT-FALLBACK-001 并入 ✅（纯文档）

- **DEC-075**：GPU 暂缓（①官方无 DirectML 预编译须自编 ②780M 共享内存无带宽优势 ③fp16 1.19GB 体积大，而 271 num_threads ~1.83% 零成本）；重启=官方出 DirectML 包/换独显机；fp16/fp32 现成（关卡二通过）。
- **[PROVIDER-SILENT-FALLBACK-001]**：provider 无效值**静默回退 CPU**（session.cc:303 编译期开关）；判据须「验证实际生效」；同族 DEC-069/073。
- **残留清单（先报再删）**：`<repo>/1024`(0B)、`models/kv259/`(8.5MB)、`tmp/opencode/lat_*.txt`、`tok_funasr-nano-tokenizer.h`、`kv263_backup_path.txt` 建议删；`kv263_backup-*` + 两处 `.512.bak` 建议保留待端测。
- **红线**：本次未删文件；未碰生产/版本；零凭证。
