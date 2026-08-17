# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-08-17 — tester-1 — BUILD-018 ✅ 阶段五出包：OVERLAY-043 全批进 exe（零生产改动，待主控验收）

- **来源**：主控派单（DEC-053 直接出包，基线 HEAD `b499cc3`）。前置四提交 OVERLAY-043 `a588509` / OVERLAY-043-B `5940e73` / TEST-SYNC-043 `497131f` / TEST-EXEC-043 `b499cc3` 已全验收
- **四步构建全执行**：Step1 击杀 Gavin 端测实例 `feiyin-ime` PID 21948（-debug / 12:27 / BUILD-017 旧包 08-16 23:25）——**先报主控获 Gavin 授权**属预期击杀，`feiyin-ime-nor.exe` 备份未删 → Step2 npm build 1.60s + Tauri UI 1m56s → Step3 主程序 2m21s → Step4 同步 Publish/+toml 三副本，`Publish/config.toml` 运行时数据未覆盖
- **七项核验全 PASS**：① 六 exe 时间戳 13:57-14:00 本次构建；② sha256 两副本逐一相等（`24cad6be…`/`1857c0de…`/`17fe10af…`）；③ toml 三副本 hash 全等（与 BUILD-017 相同未变）；④ ProductVersion 0.8.0/0.8.0.0 未动；⑤ 正向探针 `Streaming resampler active`=1；OVERLAY-043 本批如实报探不到（GDI 逻辑无新文案），间接证据三件套（mtime>最新提交+源码引用×24/×9+反向 `delta.abs()` 无残留）；⑥ 大小 feiyin-ime +3,072 B 略增（吻合 +337/-128）/ ui 0 / crash 0；⑦ 冒烟 PID 28300 Responding=True 无 panic 测后清理
- **红线合规**：生产零改动（`git diff -w` src/src-tauri/ui 全空，工作区 M 纯 CRLF 噪声）｜版本号未动｜运行时数据未覆盖｜未 reset/checkout/commit｜**未 push**｜Gavin 端测进程先报后杀（PID 4696 先例同族）
- **Gavin 端测重点四项**（result.md 末尾原样列出）：流畅度目视 / 本地模型录一次（波形动画）/ 波形隐藏+单按钮 / 松开热键立即切处理中
- **验收**：版本号 0.8.0 未动；未 commit（主控统一提交）；产物 `Publish/` 三 exe 12,115,456 / 10,026,496 / 24,859,648 B
- **详情**：outbox/tester-1/result.md + logs/20260817.md + CHANGELOG.md

---

## 2026-08-17 — tester-1 — TEST-EXEC-043 ✅ 阶段四全量回归 + 消融实测（OVERLAY-043 批，生产零改动，待主控验收）

- **来源**：主控派单（基线 HEAD `497131f`）。前置三提交 OVERLAY-043 `a588509` / OVERLAY-043-B `5940e73` / TEST-SYNC-043 `497131f` 已全验收
- **四步回归全过**：Step1 `cargo test` = **1040 passed / 0 failed / 11 ignored**（主 crate 967 含 +10 overlay_043 + crash-reporter 37/2ign + 集成 36；上批 1030 +10 精确命中零残差）→ Step2 Vitest **54 passed** → Step3 src-tauri **55 passed** → Step4 pytest **SKIP**（`Publish/` 为 BUILD-017 旧包 08-16 23:25 早于本批 HEAD 08-17 13:40）
- **消融 A（delta.abs()→delta）**：实测变红 **4 条**（任务书预期 3 条）——预期 3 条全中（shrink_exact_values / sign_symmetry / converges_800_240），**多出 step_never_exceeds_quarter_of_delta**。主控裁决：`interpolate_step` 的 `abs` 变量两处使用（`abs*0.25` + `.min(abs)`），其推演只改比例基数一处、min 仍用绝对值故模型偏轻少算一条；**实测 4 条为准**，该用例捕获同根因更强表现（负 delta 步长 +|delta| 方向暴跳），属护栏更严非失效
- **消融 B（门闩→false）**：实测仅 `ignore_streaming_text_truth_table` 1 条变红（第 3 格失守），与预期完全吻合
- **还原自证**：两次消融编辑器还原 → `git diff -w src/main.rs` 输出空（0 行）→ 还原后复跑 **1040/0/11** 与消融前逐数一致；红条 ③=0/①=0/②=0
- **纪律遵守**：消融 A 实测与任务书预期冲突时先上报主控再继续（主控确认"先报不改"正确），未自行改测试迁就
- **验收**：版本号 0.8.0 未动；未 commit（主控统一提交）；未出包（BUILD-018 须主控明确下令）
- **详情**：outbox/tester-1/result.md + logs/20260817.md + CHANGELOG.md

---

## 2026-08-17 — tester-1 — TEST-SYNC-043 ✅ 阶段三测试同步：OVERLAY-043 + 043-B 补真护栏（src/main.rs +138，待主控验收）

- **来源**：主控派单（基线 HEAD `5940e73`，OVERLAY-043 `a588509` + OVERLAY-043-B `5940e73` 已提交）。前置：缺陷 A 验收打回后已抽 `interpolate_step` 纯函数
- **改动**：仅 `src/main.rs` 测试区新增 `mod overlay_043_interpolate_tests`（+138 行，Windows-only `#[cfg]` 门控，生产零改动）
- **10 条用例**：`interpolate_step` 五契约（零值/≥1px/≤ceil(25%)/不越界/正负对称，±50000 穷举）+ 🔴 缺陷 A 回归护栏（`(-400)==-100`、`(-100)==-25` 改回 delta 即红）+ 双向收敛预算（800→240 与 240→800 均 ≤40 帧，缺陷 A 为 559 帧，正确 23 帧）+ `should_ignore_streaming_text` 四格真值表穷举
- **护栏预验证**：Python 数值复算五契约全范围 0 失败；消融推演（`delta.abs()`→`delta`）确认 `(-400)` 变 `-1`、收敛 560 帧 → 用例必然变红
- **不可测项如实列出**（GDI 绘制/按钮消息循环/脏标记/门闩调用侧接线），无假护栏、无闭包自证
- **验收**：cargo fmt -- --check clean / cargo check --all-targets 0 error（99 warnings 均既有，无一条指向新模块，97→99 的 +2 来自 OVERLAY-043 生产代码）/ git diff --stat 仅 src/main.rs +138 / 阶段三白名单只跑 fmt+check / 消融自证顺延阶段四（TEST-EXEC-043）
- **边界**：未碰 coder-2 生产代码；未 commit（主控统一提交）；版本号 0.8.0 未动
- **详情**：outbox/tester-1/result.md + logs/20260817.md + CHANGELOG.md

---

## 2026-08-16 — tester-1 — TEST-SYNC-045 ✅ 阶段三测试同步：ASR-045 流式判空取消（src/main.rs +46，待主控验收）

- **来源**：主控派单（基线 `05eb5f0`，代码基线 `54cde62` ASR-045 已提交）。前置：ASR-045 P0 修复（流式文字从未上屏）
- **改动**：仅 `src/main.rs` `mod streaming_empty_samples_tests`（+46 行，生产零改动）
  - ① `nonempty_samples_with_text_truth_table_cell`：真值表第 4 格（非空+Some），如实标注弱护栏
  - ② ③ `empty_string_text_not_cancelled_at_first_layer` / `whitespace_only_text_not_cancelled_at_first_layer`：🔴 本单核心，钉死两层职责划分（`:4288` 只管「有没有东西」「:4334` `trim().is_empty()` 管「文本是否有效」→ 空/纯空白用户可见 Error 提示 2000ms，非静默消失）
- **调用侧不可测**：`:4318` 判空臂需 6 种资源无法单测；全部 6 条纯函数测试保护不了调用侧，guard 改回 `s.is_empty()` P0 即复发；禁闭包伪造护栏；抽 `decide_pipeline_entry`/`EntryDecision` 纯函数建议交主控排期（未动生产代码）
- **验收**：`cargo fmt -- --check` clean / `cargo check --all-targets` 0 error（97 既有 warning）/ `git diff --stat` 仅 src/main.rs +46 / 阶段三白名单只跑 fmt+check / 消融自证顺延阶段四
- **边界**：未碰 `src/audio/mod.rs`；未 commit（主控统一提交）；版本号未动（0.8.0）
- **详情**：outbox/tester-1/result.md + logs/20260816.md + CHANGELOG.md

---

## 2026-08-16 — coder-1 — ASR-045 ✅ 流式识别结果被管线丢弃修复（P0，src/main.rs +44，待主控验收）

- **来源**：主控定位（P0，功能 100% 不可用）：在线流式 ASR 悬浮层实时出字 → 松开热键 → 文字从未上屏。基线 HEAD `427cd50`
- **根因**：调用侧 `run_pipeline_core(Ok(Vec::new()), …, Some(streaming_text))`（`main.rs:3409`）传空 samples + 流式文本；接收侧判空臂 `Ok(s) if s.is_empty()`（原 `:4307`）**无条件**取消，`initial_text` 从未被读取 → 流式文本在函数入口即被丢弃，ITN→LLM→注入后半段从未执行
- **改动**（仅 `src/main.rs`）
  - 新增纯函数 `should_cancel_on_empty(samples, initial_text)`（`:4288`）= `samples.is_empty() && initial_text.is_none()`
  - 判空臂改调该函数（`:4316-4320`）：只有「无样本 且 无流式文本」才真取消；流式（empty+Some）落入 `Ok(samples)` 走既有 `initial_text` 路径
  - 护栏测试 `mod streaming_empty_samples_tests`（`:5982`，3 条）：流式空+文本 proceed / 真空录仍 cancel / 非空+None proceed
- **三问核证**：Q1 `samples` 仅在正常转录 else 分支消费（`:4347-4357`），流式路径零引用，无 panic/除零；Q2 `cancel_signal` 每次 Start 重置 false（`:3189`），`:4323` 只拦 join 期真实取消，无误伤；Q3 全链 27 步核完（热键→录音→流式识别→松开→ITN→LLM→注入）唯一断点即原判空臂
- **验收**：cargo fmt --check clean（仅本人区域）/ cargo check --all-targets 0 error；白名单遵守未跑 test/build；未 commit（主控统一提交）；版本号未动
- **边界**：与 coder-2 OVERLAY-043（`:908-940`、`:1780` 起）无文本重叠；`src/audio/mod.rs`（tester-1 在途）零触碰
- **详情**：outbox/coder-1/result.md + logs/20260816.md + CHANGELOG.md

---

## 2026-08-16 — tester-1 — BUILD-016 ✅ v0.8.0 首包出包（全构建 + 七项核验 + 双探针，生产零改动）

- **来源**：Gavin 已明确下达出包指令，主控派单 BUILD-016（阶段五）。基线 HEAD `be76fc1`
- **构建**：Step 1 清进程（feiyin-ime PID 23888）→ Step 2 npm build（新 `index-DkzLqu_f.js`）+ Tauri UI release 2m14s（cp 到 target/release/）→ Step 3 主程序 2m40s → Step 4 同步 Publish/（三 exe + scene/itn 两 toml；config.toml 等运行时数据未覆盖，保持 07-28 原样）
- **七项核验全 PASS**（详见 outbox/tester-1/result.md，机器实测）：① 六 exe 时间戳 00:45-00:48；② 三 exe 两副本 sha256 相等；③ 两 toml 三副本一致；④ ProductVersion 0.8.0.0/0.8.0/0.8.0.0；⑤ `index-DkzLqu_f.js` 嵌 ui.exe / 旧名 0（i18n 裸串 grep 0 系 Tauri 压缩已知行为，改文件名字符探针）；⑥ 冒烟 Responding=True 已清理；⑦ 大小对照已解释
- **🔴 双探针**：正向 `qwen-audio-3.0-asr-flash-streaming` / `api-ws/v1/inference` 各 1 命中；反向 `api-ws/v1/realtime` / `qwen3-asr-flash-realtime` 0 命中 —— 新 ASR 进包 + 旧引擎清干净
- **产物**：`Publish/feiyin-ime.exe` 12106752B（sha256 `86176283…`）/ `feiyin-ime-ui.exe` 10026496B（`db095564…`）/ `crash-reporter.exe` 24859648B（`a9c30e4e…`）
- **验收**：cargo fmt 未跑（本单纯构建，无代码改动）；版本号三处 0.8.0 未动；生产代码零改动
- **详情**：outbox/tester-1/result.md + logs/20260816.md + CHANGELOG.md

---

## 2026-08-16 — coder-1 — ASR-042 在线流式 ASR 采样率修复（StreamingResampler，src/audio/mod.rs +476/-5）

- **来源**：Gavin v0.8.0 端测，新在线 ASR 识别全错（48kHz 音频按 16kHz 解）。主控派单 ASR-042，基线 HEAD `efb101f`
- **根因**：`record_streaming` 边录边发绕过了 `record()` 录音结束时的一次性 `resample_anti_alias`（FIRSTCHAR-FIX-005 引入），48kHz 直推服务端
- **改动**：仅 `src/audio/mod.rs`（+476/-5）
  - 新增 `StreamingResampler`（`pub(crate)`）：带状态流式抗混叠重采样器，数学等价于 `resample_anti_alias`（windowed-sinc FIR, TAPS=32, Hann 窗），跨块保留历史+全局 emitted 计数，接缝不截断卷积核
  - `record_streaming` 接线：pre-roll → post-hotkey → 主循环 依次喂**同一实例** + break 后 `finish()` flush 尾部 + 新日志 `Streaming resampler active: {N}Hz -> 16000Hz`
  - `total_samples`/`max_frames`/RMS/`level_buf` 继续用原始 chunk（未重采样）
  - 新增 `RESAMPLE_TAPS` 常量（`resample_anti_alias` 与 `StreamingResampler` 共用防漂移）
  - +6 测试：等价性（逐点差 ≤1e-5）/ 非对齐块长(441) / 恒等(16k→16k) / 长度(误差 ≤1) / 顺序护栏 / finish 尾部
- **"改坏会红"自证**：`push` 入口注入 `emitted=0`（每块重算）→ 等价性测试 FAILED（stream=807010 vs batch=16000），移除后 GREEN
- **验收**：cargo fmt clean / cargo check --all-targets 0 error / cargo test audio 56 passed 0 failed 1 ignored（既有用例零改动）
- **禁区核验**：`resample_anti_alias` 函数体 / `record()` / `vad.rs` / `main.rs` / `qwen_inference.rs` 零改动
- **跨平台**：`docs/MACOS-HANDOFF.md` 新增 §ASR-042 节（平台中立模块，macOS 编译同份代码；运行时影响取决于 macOS 侧 record_streaming 是否已接线）
- **版本号**：未动
- **详情**：outbox/coder-1/result.md + logs/20260816.md + CHANGELOG.md

---

## 2026-08-16 — tester-1 — TEST-EXEC-042/045 ✅ 阶段四全量回归 + 消融自证（v0.8.0 出包前最后一道闸，生产零改动）

- **来源**：主控派单 TEST-EXEC-042/045（阶段四，基线 HEAD `fd994a5`）；前置四提交 ASR-042/TEST-SYNC-042/ASR-045/TEST-SYNC-045 已全验收
- **四步回归全过**：Step1 `cargo test` = **1030 passed / 0 failed / 11 ignored**（主 crate 957 + crash-reporter 37 + integration 36；预期 ≈1030 精确命中零残差）→ Step2 Vitest **54 passed / 5 files** → Step3 src-tauri **55 passed / 0 failed** → Step4 pytest **SKIP**（`Publish/` 是 BUILD-016 旧包早于本批，E2E 正确时机 BUILD-017 出包后）
- **消融 A（调用侧缺口实证）**：guard `:4318` 改回 `s.is_empty()` → 6 条 streaming_empty 测试**全绿**（P0 静默复发但零报警）→ 还原。结论：6 条纯函数测试保护不了调用侧，`TEST-045-REFACTOR` 排期维持
- **消融 B（分层契约护栏实证）**：`should_cancel_on_empty` 合并 `trim().is_empty()` → **4 passed / 2 failed**，变红恰为空串/纯空白两条（coder-1 3 条 + 真值表第 4 格仍绿）→ 还原。与主控推演真值表完全一致，护栏有效
- **收尾自证**：两消融全还原 → `git diff src/main.rs` 空 + `git diff -w` 0 行（77 文件仅 CRLF 噪声 [CRLF-CROSSPLAT-001]）→ 还原后复跑 **1030 全绿**逐数一致；红条 ③=0/①=0/②=0
- **验收**：无需 fmt/check（本单纯回归执行 + 临时消融已还原）；版本号 0.8.0 未动；未 commit（主控统一提交）；未出包（BUILD-017 须 Gavin 明确下令）
- **结论**：v0.8.0 全量回归闸门通过，无阻塞项，可进入阶段五 BUILD-017
- **详情**：outbox/tester-1/result.md + logs/20260816.md + CHANGELOG.md

## 2026-08-16 — tester-1 — BUILD-017 ✅ v0.8.0 第二包出包（ASR-042 + ASR-045 进 exe，生产零改动）

- **来源**：Gavin 已下令出包（并确立新规则：测试验收通过后主控直接派发出包，不再逐次请示）。基线 HEAD `3c075e8`
- **构建**：四步全执行。Step1 杀进程（无 `feiyin-ime` 主进程运行）→ Step2 npm build（`index-DkzLqu_f.js` 与 BUILD-016 同名=零前端改动）+ Tauri UI release 1m45s（cp 至 target/release/）→ Step3 主程序 2m04s → Step4 同步 Publish/（三 exe + scene/itn 两 toml；Gavin 运行时数据 config.toml 等四文件未覆盖，mtime 全为历史时间）
- **七项核验全 PASS**（详见 outbox/tester-1/result.md）：① 六 exe 时间戳 23:23-23:25；② 三 exe 两副本 sha256 相等（feiyin `7562b943…` / ui `62ae24df…` / crash `84fdfddd…`）；③ toml 三副本 hash 全等；④ ProductVersion 0.8.0；⑤ 正向探针 `Streaming resampler active`=1 → **ASR-042 进包**；**ASR-045 如实报「探不到」**（无新字符串+可能内联），用间接证据（mtime>54cde62 + 源码 ×12 引用）证明，未编探针；⑥ 大小对照：feiyin +5,632 B（略增吻合代码量）、ui/crash 完全不变；⑦ 冒烟 PID 23956 Responding=True 无 panic 已清理
- **⚠️ 杀进程实测**：Step1 时无 `feiyin-ime` 主进程运行（无 PID 被杀）；但发现并清杀**名单外遗留 `feiyin-ime-nor` PID 23232**（8-9 旧构建，持单实例 mutex，冒烟首次启动被挡）。若 Gavin 在用输入法需重新启动
- **验收**：生产代码零改动（`git diff src/ src-tauri/ ui/`=0）；版本号 0.8.0 未动；未 commit（主控统一提交）
- **Gavin 端测四项**（须 `-debug`，ASR-PERF-040-B/C 唯一数据源）：新端点连通性 / `usage.duration` 计费口径 / 040-A 四段连接耗时 / VAD 门控实际行为
- **详情**：outbox/tester-1/result.md + logs/20260816.md + CHANGELOG.md

## 2026-08-17 — coder-2 — OVERLAY-043 录音悬浮层五项显示与流畅度修复（src/main.rs +349/-150，阶段一完成）

- **来源**：Gavin 2026-08-17 端测截图 + 主控逐条 Read 代码取证；基线 HEAD `56bfa37`
- **根因**：此前任务从未派发，代码未动。端测已确认 ASR-042 生效、流式文字能上 overlay，问题 purely 在 overlay 显示与流畅度
- **改动**（仅 `src/main.rs`）：
  1. 拆 `draw_recording_overlay` 为 chrome/indicator+waveform/stop-button 三段；`RecordingWithText` 路径不再画波形，文字区不被挤压
  2. 右侧单按钮复用：录音态=停止方块，编辑态=同位置橙色 ⏎ 提交；删除独立 submit 绘制
  3. 100ms 尺寸节流改为**延迟合并** + 25% lerp 插值，避免回退 240px 突闪
  4. `InvalidateRect` 改 `bErase=false`；加 `needs_repaint` 脏标记；WM_PAINT 已双缓冲，内存 DC 先 FillRect 背景防垃圾像素
  5. 新增 `STREAMING_STOPPED` 门闩：Stop/ESC/取消/提交/编辑置位，`RecordingStarted` 复位；晚到 `StreamingText` 不再把 `FallingToProcessing` 顶回录音态；编辑态仍同步文字
- **流畅度额外手段**：尺寸插值过渡、状态变化才重置命中区、目标尺寸与当前尺寸差异阈值驱动 `SetWindowPos`
- **验收**：`cargo fmt --all -- --check` clean / `cargo check --all-targets` 0 error（99 warnings 均为既有/未使用变量）/ `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / `git diff --stat` 仅 `src/main.rs`
- **边界**：未碰 `src/audio/mod.rs`、`src/vad.rs`、`src/transcription/**`、版本号（0.8.0）
- **跨平台**：`docs/MACOS-HANDOFF.md` 新增 §OVERLAY-043；改动全在 Windows-only `#[cfg]` 内，macOS 零编译影响，行为契约已记录
- **后续**：阶段三 TEST-SYNC 待 coder-2 验收后派发；最终目视顺滑度由 Gavin 端测拍板
- **详情**：outbox/coder-2/result.md + logs/20260817.md + CHANGELOG.md

## 2026-08-17 — coder-2 — OVERLAY-043-B 抽两个纯函数补真护栏（src/main.rs +39/-12，阶段一补强）

- **来源**：OVERLAY-043 验收时主控手工数值复算发现 `interpolate_step` 内联逻辑缺陷；基线 HEAD `a588509`
- **改动**（仅 `src/main.rs`）：
  1. 新增 `interpolate_step(delta: i32) -> i32` 纯函数（doc 注释含四条契约），替换 `run_overlay_thread` 中内联步长计算；与原内联表达式逐位等价
  2. 新增 `should_ignore_streaming_text(stopped: bool, editing: bool) -> bool` 纯函数，替换 `PipelineEvent::StreamingText` 分支的内联门闩判断；与原布尔表达式 `stopped && !editing` 逐位等价
- **性质**：纯可测性重构，**行为零变更**；测试用例由阶段三 tester-1 负责，本任务不写 `#[test]`
- **验收**：`cargo fmt --all -- --check` clean / `cargo check --all-targets` 0 error / `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / `git diff --stat` 仅 `src/main.rs`，无新增 `#[test]`
- **边界**：未碰 `src/audio/mod.rs`、`src/vad.rs`、`src/transcription/**`、版本号（0.8.0）
- **跨平台**：`docs/MACOS-HANDOFF.md` §OVERLAY-043 已声明纯 Windows-only `#[cfg]` 代码，macOS 零编译影响；本次抽函数仍在同一 `#[cfg]` 块内
- **详情**：outbox/coder-2/result.md + logs/20260817.md + CHANGELOG.md

---

## 2026-08-16 — tester-1 — 文档维护：build-test-guide.md Step 1 杀进程名单补 `feiyin-ime-nor`（主控验收后建议，非派单）

- **背景**：BUILD-017 验收通过（提交 `2074859`）。主控建议把名单外遗留进程 `feiyin-ime-nor` 补进 Step 1 杀进程名单，否则下次出包重蹈 mutex 坑。该文档归 tester-1 维护，无需等派单
- **改动**：两处命令 `Get-Process feiyin-ime,voice-ime-ui` → `feiyin-ime,feiyin-ime-nor,voice-ime-ui,feiyin-ime-ui,crash-reporter`（原名单还缺 `feiyin-ime-ui`/`crash-reporter`，一并补齐）+ 注释说明 BUILD-017 实测背景
- **纯文档维护**：无代码改动、无出包、无 commit（主控统一提交）
- **留意项（不动作）**：`target/release/feiyin-ime.exe` PID 4696（00:08:39 启动，非冒烟进程）锁着 exe，下次构建可能报 os error 5；主控已报 Gavin 判断归属，回话前任何人不得杀。保持待命等 Gavin 端测
- **详情**：logs/20260816.md
