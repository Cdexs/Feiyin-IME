# 任务列表 · voice-ime

## 🛑 2026-08-17 当前状态 —— 下一步等 Gavin 端测 BUILD-018

> 三个 Worker 全部空闲待命。**下一个动作是等 Gavin 端测结果，不是派新任务。**

| 项 | 状态 |
| --- | --- |
| HEAD | `664b7bd`，工作区 **clean**，零悬空改动 |
| 本批 OVERLAY-043 四提交 | OVERLAY-043 `a588509` / 043-B `5940e73` / TEST-SYNC-043 `497131f` / TEST-EXEC-043 `b499cc3` **全部已验收提交** |
| 阶段四全量回归 | ✅ **1040/0/11**（上批 1030 +10 精确命中零残差）+ Vitest 54 + src-tauri 55；pytest 按书 SKIP（`Publish/` 当时为 BUILD-017 旧包） |
| 双消融实测 | ✅ A（`delta.abs()`→`delta`）变红 **4** 条（推演预期 3，主控裁决实测为准，见 `[ABLATION-MODEL-TOO-LIGHT-001]`）；B（门闩→false）变红 1 条与预期吻合；两次均已还原并 `git diff -w` 自证为空 |
| 阶段五出包 | ✅ BUILD-018 `664b7bd` —— 产物 08-17 13:57-14:00 在 `Publish/`，七项核验全 PASS（三 exe 12,115,456 / 10,026,496 / 24,859,648 B） |
| 未 push | 本地 ahead **6**，**Gavin 未授权 push，不得自动执行** |

### Gavin 端测四项重点（BUILD-018）

1. **流畅度目视** —— 文字进编辑框是否丝滑（`bErase=false` + `needs_repaint` 脏标记 + 25% lerp 插值）
2. **本地模型录一次** —— 确认波形动画未被本批改坏（静态论证四条已过，需实测兜底）
3. **波形隐藏 + 单按钮** —— `RecordingWithText` 态不画波形；右侧只有一个按钮（录音=停止方块 / 编辑=橙色 ⏎ 提交）
4. **松开热键立即切「处理优化中」** —— 晚到的 `StreamingText` 不应再把状态顶回录音态

### 端测通过后的下一棒（按序）

1. **TEST-045-REFACTOR** —— 抽 `decide_pipeline_entry` 纯函数补调用侧护栏缺口（见下方专节；🔴 红线：**不得合并** `:4334` 第二层 `text.trim().is_empty()` 分支）
2. **ASR-PERF-040-B/C** —— 连接健壮性与性能（040-A 埋点已落地，B/C 的唯一数据源是端测 `debug.log`，见下方门禁专节）

### 端测若失败，`debug.log` 优先看这三条

| 目标 | 探针 | 期望 |
| --- | --- | --- |
| ASR-042 采样率修复 | 搜 `Streaming resampler active` | **有**这行 = 重采样器接上了 |
| ASR-045 结果上屏 | 搜 `No audio samples recorded` | 流式录音后**不应**再出现 |
| OVERLAY-043 门闩 | 松开热键后的状态流转 | 不应再出现 `RecordingWithText` 把 `FallingToProcessing` 顶回去 |

### 🔴 两个不许动的东西（Gavin 明确指示，持续有效）

- `target/release/feiyin-ime-nor.exe`（08-09，11.9MB）—— Gavin 的**备份 exe，不得删除**。（其**进程**可在出包 Step1 清杀，名单已于 08-16 补入 `build-test-guide.md`）
- **Gavin 自启的端测进程** —— 杀之前必须先报主控、由 Gavin 授权（PID 4696 / PID 21948 两次先例）

---

## 🔴🔴 P0 最高优先 · OVERLAY-046 录音 overlay 从未被定位/定尺寸（2026-08-17 Gavin 端测 BUILD-018）

> **Gavin 原话**：「热键完全被改坏，无法正常使用输入法……按下设置好的热键，录音窗口不出现，完全无法录音」
>
> **性质：OVERLAY-043 引入的真回归，输入法 100% 不可用。已派发 coder-2。**

| 项 | 内容 |
| --- | --- |
| 编号 | **OVERLAY-046** |
| 文件域 | `src/main.rs`（仅此一个，coder-1/tester-1 本轮无任务，零冲突） |
| 负责人 | coder-2 |
| 状态 | ✅ **已交付，等主控验收** |

### 修复摘要

- **根因**：OVERLAY-043 重构中 Show 分支丢失无条件 `SetWindowPos` / `InvalidateRect`；非流式态恒等赋值 `current_size == target_size` 使插值门控 `size_interpolation_done` 永不置位，`:1221` 的 `SetWindowPos` 永不执行 → 窗口不定位/不定尺寸。
- **修法**：在 `src/main.rs:997` Show 分支 `ShowWindow` 之前恢复无条件 `SetWindowPos(request.pos, computed_size, SWP_NOACTIVATE | SWP_NOZORDER)`；`ShowWindow` 之后补 `InvalidateRect(hwnd, None, false)`（`bErase=false` 红线保持）。
- **覆盖变体**：`Recording` / `FallingToProcessing` / `Processing` / `Error` / `FocusLost` 全部非流式态；流式态 `RecordingWithText` / `StreamingEditing` 不受负影响，插值动画继续。
- **验证**：`cargo fmt --all -- --check` clean / `cargo check --all-targets` 0 error / `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / `git diff -w -- src/main.rs` 仅 `:997-1005` 新增 14 行。
- **阶段三**：不跑 `cargo test` / `cargo build`，测试由 tester-1 负责。

### 根因（主控 git 对照取证，四步）

| # | 证据 |
| --- | --- |
| 1 | `git show 56bfa37:src/main.rs`（043 之前）Show 分支含三件事：**无条件 `SetWindowPos`** + `ShowWindow(SW_SHOWNA)` + `InvalidateRect(hwnd, None, true)` |
| 2 | 现 HEAD `664b7bd` 的 Show 分支（`:997-1005`）只剩 `SetLayeredWindowAttributes` + `ShowWindow` + 条件 `SetTimer` —— **`SetWindowPos` 与 `InvalidateRect` 双双消失** |
| 3 | 全文件唯一定位 overlay 的 `SetWindowPos` 在 `:1221`，被 `size_interpolation_done` 门控；该标志仅在 `:1198` `current_size != target_size` 成立时于 `:1212` 置位 |
| 4 | 但 Show 分支 `:991-993` 对**非流式态**执行 `target_size = current_size = computed_size` → 两者恒等 → `:1198` 恒 false → **`SetWindowPos` 永不执行** |

`:476` `remove_noactivate` / `:494` `restore_noactivate` 的两处 SetWindowPos 都带
`SWP_NOMOVE | SWP_NOSIZE`，不构成兜底。

### 🔴 影响范围比 Gavin 报的更广

`Recording` / `FallingToProcessing` / `Processing` / `Error` **全是非流式态**，全走 `:991-993`
→ **在线模型与本地模型的录音窗口一律受影响**。

🔴 **主控自陈作废一条结论**：本文件下方「本地模型回归核查四条全过 → OVERLAY-043 对本地模型
录音窗口零影响」**是错的**。那四条只机器比对了 GDI 绘制原语序列，**没有查 `SetWindowPos`
的调用门控**，因此漏掉了本缺陷。教训已记 `troubleshooting.md`。

### 红线遵守

① `InvalidateRect` 的 `bErase` 保持 `false` ✅  ② 未动 `:1198` 插值分支与 `interpolate_step` ✅  ③ 未改 `STREAMING_STOPPED` 门闩语义 ✅  ④ 版本号 0.8.0 未动 ✅

---

## ℹ️ 非代码问题（已核实，无需开单）

| 现象 | 结论 |
| --- | --- |
| `LLM optimization error: HTTP 402 Payment Required`（api.deepseek.com） | **DeepSeek 账号余额/账单问题**，非代码 bug。后果：LLM 优化失败 → 走 raw text fallback。需 Gavin 侧充值 |
| `转录失败：task-finished 但无识别结果` ×20 / ASR 结果 254 条中 246 条 `display=''` | **端测手法所致，非 bug**。Gavin 在反复短按热键测「窗口出不出来」，多数按压 <1.5s 且未说话 → VAD 正确判定无语音（`Audio channel closed during VAD gate` / `2s deadline without detection`）→ 服务端正确返回空。同一会话中真说话的两次（06:16:35 / 06:16:36）识别出「你好。」，06:17:26 识别出「Hello.」→ **ASR 链路本身是通的** |

## 🔴 P0 进行中 · v0.8.0 端测两大问题（Gavin 2026-08-16 提交）

> 出包后 Gavin 端测发现两个问题，主控已独立取证定位根因。**执行顺序已与 Gavin 商定：先修识别，再修窗口。**
> 理由：识别输出乱码时，无法目视判断窗口显示是否正确。

| 编号 | 内容 | 文件域 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| **ASR-042** | 在线流式 ASR 采样率修复（48000→16000 带状态流式重采样器） | `src/audio/mod.rs` | coder-1 | ✅ **已验收已提交 `34906a1`**（StreamingResampler + record_streaming 接线 + 6 测试 + 改坏会红自证） |
| TEST-SYNC-042 | 阶段三测试同步（补 4 条流式重采样缺口用例） | `src/audio/mod.rs` `mod tests` | tester-1 | ✅ **已提交 `427cd50`** |
| **ASR-045** | 流式识别结果被管线丢弃（文字从未上屏，P0） | `src/main.rs` | coder-1 | ✅ **已验收已提交 `54cde62`**（`should_cancel_on_empty` 纯函数 + 判空臂改调 + 3 条护栏测试） |
| TEST-SYNC-045 | 阶段三测试同步（ASR-045 缺口 3 条，`src/main.rs` +46 仅测试模块） | `src/main.rs` `mod streaming_empty_samples_tests` | tester-1 | ✅ **已验收**（主控逐行 Read diff + 独立复算 `cargo fmt --check` clean / `cargo check --all-targets` 0 error）。真值表第 4 格（如实标弱护栏）+ 空串/纯空白两层分层契约 ×2（核心）+ 调用侧不可测诚实判定 |
| TEST-EXEC-042/045 | 阶段四全量回归 + 消融 A/B 自证 | — | tester-1 | ✅ **已验收已提交 `2a173f0`**。1030/0/11（941→957 恰 +16，零残差）+ Vitest 54 + src-tauri 55；pytest 按书 SKIP（Publish/ 为 BUILD-016 旧包）；③ 类真回归 = 0 |
| BUILD-017 | 阶段五出包（ASR-042/045 进 exe） | — | tester-1 | ✅ **已验收**（主控独立复算全部七项）。产物 08-16 23:25，`feiyin-ime.exe` 12,112,384B（+5,632）/ ui 10,026,496B 不变 / crash 24,859,648B 不变；两副本 sha256 全等；toml 三副本全等；ProductVersion 0.8.0.0/0.8.0/0.8.0.0。⏭ **待 Gavin 端测** |
| **OVERLAY-043** | 录音窗口**五项**显示错乱 + 流畅度 | `src/main.rs` | coder-2 | ✅ **已验收已提交 `a588509`**（打回一轮，见下方打回记录）。⏭ 流畅度**待 Gavin 端测目视拍板** |
| **OVERLAY-043-B** | 抽 `interpolate_step` / `should_ignore_streaming_text` 补护栏 | `src/main.rs` | coder-2 | ✅ **已验收已提交 `5940e73`**（纯重构零行为变更，无 `#[test]`） |
| **TEST-SYNC-043** | 阶段三测试同步（两纯函数护栏 + 不可测项如实说明） | `src/main.rs` `mod tests` | tester-1 | ✅ **2026-08-17 已完成待验收**（`mod overlay_043_interpolate_tests` +138 行仅测试区：10 条用例——五契约 ±50000 穷举 + 缺陷 A 数值回归护栏 + 双向收敛预算 + 门闩四格真值表；cargo fmt clean / cargo check --all-targets 0 error；Python 复算契约成立、消融推演护栏有效。消融实测顺延阶段四 TEST-EXEC-043） |

### 🔴 OVERLAY-043 验收打回记录（主控独立复算查出，非采信报告）

| 缺陷 | 位置 | 问题 | 数值证据 |
| --- | --- | --- | --- |
| **A** | `main.rs:1195` 插值步长 | `(dx as f32 * 0.25).max(1.0)` 在 `dx` 为负时**负数被 `max(1.0)` 吃掉比例**，恒为 `-1px` | 800px→240px 需 **559 帧 ≈ 8.9 秒**爬行；修正后放大/缩小对称均 **22 帧 ≈ 0.35 秒** |
| **B** | `main.rs:1114` 脏标记 | 纯 `Recording` 态被纳入脏标记，但**全文件无一处在音频电平变化时置位** → 波形动画冻结 | 违反红线「`Recording` 观感零变化」；对照 `:1159` `FallingToProcessing` 有 `!all_settled` 兜底，`Recording` 分支缺同类兜底 |

**教训**：缺陷 A 只能靠手工数值复算发现 —— 算式内联在消息循环内，测试够不着。
故**当场派 OVERLAY-043-B 抽纯函数补护栏**，不排期延后
（与 `TEST-045-REFACTOR` 同类，但那是没塌过的预防，本处是刚塌过一次）。

### ✅ 本地模型回归核查（Gavin 2026-08-17 提出，主控独立取证，四条全过）

> **Gavin 原话**：「本地模型的录音还是会使用原先的录音窗口，所以这次新集成 asr 实时上屏
> 不能影响本地模型录音的 overlay 窗口效果」

| # | 核查项 | 结论与证据 |
| --- | --- | --- |
| 1 | 绘制是否等价 | ✅ **机器比对**：拆分前单体与拆分后 `chrome + indicator_and_waveform + stop_button` 串接，**18 个 GDI 原语序列完全一致**（`diff` 为空） |
| 2 | 是否被尺寸插值波及 | ✅ 不会。`is_streaming_text` 仅 `matches!(RecordingWithText)`；非流式态 `:991-994` 令 `target_size == current_size` → `:1191` 插值条件不成立 |
| 3 | 波形是否会冻 | ✅ 不会。返工后 `Recording` 独立为**每帧重绘**分支，脏标记只用于 `RecordingWithText`/`StreamingEditing` |
| 4 | 门闩是否误伤 | ✅ 不会。`StreamingText` 仅在 `is_streaming_asr`（`AsrModel::QwenAudioOnline`）下产生（`:3512-3543`）；本地模型走 `record()` + `run_pipeline_core` 老路**永不进入该分支**，注释 `:3509` 亦自陈「零行为变更」 |

**结论**：本地模型 overlay 只经历 `Recording → FallingToProcessing → Processing`，
全是本批未改语义的状态。**OVERLAY-043 对本地模型录音窗口零影响。**
🔴 **但这是静态代码论证，Gavin 端测时请顺手用本地模型录一次确认。**

### 🟡 待排期 · TEST-045-REFACTOR 抽 `decide_pipeline_entry` 纯函数（tester-1 建议，主控采纳但延后）

**问题**：ASR-045 全批 6 条测试（coder-1 3 + tester-1 3）**全是纯函数级，保护不了调用侧**。
若有人把 `main.rs:4318` 的 guard 改回 `Ok(s) if s.is_empty()`，P0（流式文字从未上屏）立刻复发，
**6 条测试依然全绿**。这是本批已知且已如实记录的护栏缺口，不是遗漏。

**建议方案**（tester-1 提出，主控核后认可）：把 `run_pipeline_core` 入口三臂判据抽成
`decide_pipeline_entry(&samples_result, &initial_text) -> EntryDecision`
（`CancelNoInput` / `Err` 透传 / `Proceed`），让「空+文本 / 空+无文本 / 非空+无文本 / Err」四种入口组合可测。

**主控裁决：采纳，但排到 OVERLAY-043 之后。** 理由：① 这是可测性改进不是 bug，不该插队 P0；
② OVERLAY-043 马上要动 `main.rs`，两个改动叠一起会让回归归因变难。
🔴 **实施时的红线**：不得合并 `:4334` 第二层的 `text.trim().is_empty()` 分支 ——
分层是刻意设计，合并即复现「空文本静默消失」的 P0 类回归。

### ASR-045 根因（一句话）

调用侧 `run_pipeline_core(Ok(Vec::new()), …, Some(streaming_text))`（`main.rs:3409`）传**空 samples +
流式文本**；接收侧判空臂 `Ok(s) if s.is_empty()`（原 `:4307`）**无条件取消**，`initial_text` 从未被读取
→ 流式文本在函数入口即被丢弃，ITN→LLM→注入后半段从未执行（日志 `:193` 已产出 5 confirmed sentences，
`:194` 即被 WARN 吞掉）。

**取证与修法** → outbox/coder-1/result.md（含三问全链 27 步、Q1 samples 消费点排查、Q2 cancel_signal
时序、护栏测试）。

### ASR-042 根因（一句话）

麦克风 48000Hz 采集，发给服务端的参数写死 `sample_rate: 16000`，中间零重采样 →
服务端按慢 3 倍解码 → 识别输出完全不相干的句子（「你好，花呗还不上」等客服话术）。

**成因**：旧 `record()` 在录音结束那一刻一次性重采样（`audio/mod.rs:757`）；改成边录边发后
「录音结束」这个时刻不存在了，该步骤静默失效。**不是写错，是改架构漏接线。**

**完整取证与修法** → `collab/troubleshooting.md` `[ASR-SAMPLERATE-STREAM-001]`
（含 8 次录音「6 有重采样全对 / 2 无重采样全错」对照表与日志自证行）。

**关键约束**：不能逐块直接调 `resample_anti_alias`（整段 windowed-sinc，TAPS=32），
逐块调用每 10ms 截断一次卷积核，会让 FIRSTCHAR-FIX-005 修过的送气清声母失真复发。
必须做带状态流式重采样器 + 全局输出计数。护栏：批处理 vs 流式逐点差 ≤ 1e-5。

**次生现象**：VAD 2s 漏检（`debug.log:36`）同根因，预期自动恢复，**须实测确认，禁改 `vad.rs`**。

### OVERLAY-043 六项（主控已逐条核对代码，非采信 coder-2 汇报）

> 🔴 **2026-08-17 Gavin 端测截图复核，本节已从四项扩为六项。**
>
> **首先澄清一个误解**：Gavin 反馈「上次发现的问题根本没改过来」——
> **本任务自始至终从未派发过**（本节状态一直是「🔜 等 BUILD-017 + Gavin 目视确认识别正常后再派发」），
> 所以不是「改了没生效」，是**还没动过一行代码**。责任在主控排期，不在 coder-2。
>
> **端测截图的意外收获（重要正面结论）**：截图中 overlay 显示「今天的天气如何 可以说」——
> **语义完全正常**，不再是修复前「你好，花呗还不上」那类 48kHz 按 16kHz 解出来的客服话术。
> **ASR-042 采样率修复已由端测实证生效**，且流式文字确实上了 overlay。

| # | 现象 | 核对结论（主控 Read 代码取证） |
| --- | --- | --- |
| 1 | 波形动效未隐藏 | 属实。`main.rs:1786` 首行 `draw_recording_overlay` 整个重画，注释自陈 reuse「background/border/**waveform**/stop button」，波形被原样带进 |
| 2 | 文字非白色 | ⚠️ **本次端测截图看文字是白的**，Gavin 未再提。降级为待观察，不列入本次修复范围 |
| 3 | 右侧两个按钮（提交+停止） | 属实。`:1786` 返回 stop 按钮，`:1834-1871` 又叠画一个橙色圆角 ⏎ submit 按钮 → **两个并存**。Gavin 明确要求：**提交按钮应复用停止按钮的位置**，而非新增 |
| 4 | 窗口随文字变宽时卡顿、突闪 | 属实且**比原描述严重**：`:908-925` 判 `should_resize`，`:940-949` 为 false 时走 else → **回退到 `request.size`（默认 240px）而非保持上次宽度** → `:956` SetWindowPos 无条件执行 → 窗口在算出宽度与 240px 之间**来回跳** = 突闪 |
| 5 | 🆕 文字进编辑框卡顿，不丝滑 | **主控定位**：`:1078` `InvalidateRect(hwnd, None, **true**)` —— 第三参 `bErase=TRUE` **每帧擦背景再重画**，是 GDI 闪烁/卡顿的经典根因。三个态（Recording/RecordingWithText/StreamingEditing）全走这一行 |
| 6 | 🆕 录音结束未进编辑态时，应立即切「处理优化中」窗口 | **主控定位到竞态**：`:2737-2752` 松开热键的 `HotkeyEvent::Stop` **确实**发了 `FallingToProcessing`，但 `:2788-2796` 晚到的 `PipelineEvent::StreamingText` 又推 `RecordingWithText` **把它覆盖回录音态**。即状态切了、又被流式尾包顶回去 |

🔴 **主控自陈**：第 4 条的 100ms 节流是**上一轮主控自己要求 coder-2 加的**（防每帧改宽度抖动），
现成为文字截断的直接原因。**不能简单删除**（删了会抖，属「修一个带出一个」）。
修法：改为**延迟合并** —— 100ms 内多次变化攒起来，到点按最新文字算一次宽度。

### ~~待查~~ ✅ 已定性并修复（2026-08-16）

~~`debug.log:194`/`:429` 每次在线流式录完都报 `WARN No audio samples recorded`。~~

**结论：不是日志噪音，就是 ASR-045 本身。** 该 WARN 正是判空臂 `Ok(s) if s.is_empty()`
无条件取消时打的那一行 —— 流式文本在此被丢弃，整条 ITN→LLM→注入后半段从未执行。
ASR-045（`54cde62`）修复后该 WARN 在流式路径不再触发。**本条核销，无残留副作用待查。**

---


## 🔴 P0 门禁 · ASR-PERF-040 新 ASR 连接性能与可靠性（2026-08-15 Gavin 指令）

> **Gavin 原话**：「这次继承新 asr 模型一定要汲取之前的教训，要优化好连接池、
> 要优化好连接的速度和性能。」
>
> **定位：这是 ASR-038 全批的验收门禁，不是可选优化项。** 038-B/C 完成后若本节未落实，不予出包。

### 🔴 第一件事：028 的教训不能照搬，因为 ASR 根本没有连接池

| 项 | LLM（028 现场） | ASR（本批） |
| --- | --- | --- |
| 传输 | `reqwest` HTTP，**有连接池** | `tungstenite` WebSocket，**无池，每次全新建连** |
| 028 根因 | 池中空闲连接被服务端 keep-alive(~60s) 杀掉，取出即 0ms 失败 | **不适用** —— 没有池就没有僵尸连接 |
| 修复 | `POOL_IDLE_TIMEOUT=30s` + `is_request()` 重试 + `fmt_error_chain` | **照搬无效** |

**🔴 但真正的陷阱在这里**：为了提速而引入「**预热 / 复用长连接**」，
会**精确复现 028 的 bug 类** —— 预热好的 WS 空闲期间被服务端 idle timeout 静默杀掉，
下次录音取用即失败，且表现为「0ms 失败」，与 028 一模一样。

**因此复用设计必须自带三件套，缺一不可**：
① 本端 idle 上限**必须短于**服务端 idle timeout（服务端值未知 → 须实测，不可假设）；
② 取用前**健康检查**（WS Ping/Pong 或轻量探针）；
③ 检查失败**立即回退新建连**，绝不把失败抛给用户。

> **这正是 028 的真正教训**：不是「把超时调小」，而是
> **「任何跨请求复用的连接，都必须假设它在空闲期间已经死了」。**

### 取证：当前连接路径的实际开销（`qwen_inference.rs:445-465`）

每次录音**从零走完整条链**，零预热、零复用、零重试：

```
DNS 解析 (to_socket_addrs，阻塞系统调用，无缓存)
  → TCP connect (CONNECT_TIMEOUT = 5s)
  → TLS 握手 (client_tls_with_config)
  → WS 升级 (HTTP 101)
  → 才开始发第一个音频字节
```

`grep -i "prewarm|reuse|retry|keepalive|pool" src/transcription/qwen_inference.rs` → **零命中**。

### 已定位的四个问题

| # | 问题 | 位置 | 性质 |
| --- | --- | --- | --- |
| **40-1** | **`socket_addrs.first()` 只试第一个地址** —— DNS 返回 IPv6 在前但本机无 IPv6 连通性时，**直接失败**，不会回退第二个地址 | `qwen_inference.rs:461` | 🔴 **健壮性缺陷**。标准做法是遍历所有 addr（`TcpStream::connect` 本身就是这么做的） |
| **40-2** | **零重试** —— 建连瞬时抖动直接变「转录失败」 | 同上 | 🔴 与 028 修复精神相悖（028 明确补了重试判据） |
| **40-3** | **零 `[Latency]` 埋点** —— DNS/TCP/TLS/WS 各段耗时**完全不可观测** | `qwen_inference.rs` 全文 | 🔴 **Gavin 要求「优化速度」，但现在连测都测不了**。全库已有 12 处 `[Latency]` 约定可循 |
| **40-4** | `CONNECT_TIMEOUT=5s` 被同时用作 **connect / read / write** 三处超时 | `:32`、`:456-459` | 🟡 语义混用。建连 5s 合理，但流式收包期间的 read timeout 应独立定义 |

### 🔴 真流式在这里是**性能杠杆**，不只是交互需求

| 方案 | 建连成本落在哪 | 用户感知 |
| --- | --- | --- |
| **真流式**（边录边发） | 建连与**说话并行** → DNS+TCP+TLS 被说话时间吸收 | **零感知** |
| **伪流式**（录完再发） | 建连在**录音结束之后** → 全部落在关键路径 | **干等一个完整 RTT 链** |

**这条独立于 037 交互论证，直接服务于 Gavin 的性能指令。**
伪流式方案下，「优化连接速度」这个目标先天就少了最大的一块杠杆。

### 建议实施顺序（待派发）

| 编号 | 内容 | 前置 |
| --- | --- | --- |
| 040-A | **先补 `[Latency]` 埋点**（DNS/TCP/TLS/WS 升级/首帧/首结果分段计时） | 无 —— **必须最先做，否则后续优化全是盲调** |
| 040-B | 修 40-1 地址遍历 + 40-2 建连重试（判据参照 028 的 `is_request()` 精神）+ 40-4 超时语义拆分 | 040-A |
| 040-C | 实测服务端 idle timeout（**不可假设**），据此决定是否上预热/复用及其 idle 上限 | 040-A/B + 端测数据 |

**🔴 040-C 是 028 同款雷区，未拿到服务端 idle timeout 实测值之前，禁止上长连接复用。**

### 文件域

`src/transcription/qwen_inference.rs`（与 ASR-038-B 同域，**必须串行，归同一个 Worker**）。

---


## 📋 待派发 · ITN-IDIOM-COVER-032 量级单位字成语零覆盖（2026-08-09 主控取证发现）

> **来路**：`examples/probe_031.rs`（031 临时探测程序）删除前，主控核查其覆盖词的测试情况时发现。
> **性质**：与 031「万一→`0.1万`」**同一类** —— 量级单位字（十/百/千/万/亿）开头的固定表达。

### 取证：六个成语在 `src/itn.rs` 里**零出现**

| 词 | `src/itn.rs` 出现次数 |
| --- | --- |
| 十全十美 | **0** |
| 十有八九 | **0** |
| 百般 / 百般刁难 | **0** |
| 千方百计 | **0** |
| 百里挑一 | **0** |
| 成千上万 | **0** |

对照：`十分` 出现 8 次（有覆盖）、`千千万万` 出现 1 次。

### 为什么这不是「补几个词」那么简单

031 只给**万/亿**两个分支加了守卫（`!has_digit && section == 0 && result == 0 → return None`）。
`十/百/千` 三个分支**至今无守卫**，按 todo 既有记录：百/千分支不设锚点、`result==0` 被拒，
**歪打正着没出事**；而 `:682` 十分支默认 1（`十分`→`10分`）挂了很久，一直是「只报不改」。

**所以本单要先回答一个问题**：这六个词现在的实际输出是什么？
—— 是已经正确（靠 `result==0` 侥幸拦下），还是已经错了但没人测过。
**取证之前不要动代码**，也不要走保护词表（DEC-038：词表不承载规则性语法族）。

### 建议分解（待派发）

| 编号 | 内容 | 负责人 |
| --- | --- | --- |
| 032-A | 六词 + `十分`族实测现状，产出「现状表」（只测不改） | tester-1 |
| 032-B | 依 032-A 结果判定：真缺陷则走机制层给十/百/千分支补对称守卫；已正确则只补断言钉住 | coder-1 |
| 032-C | `:682` 十分支默认 1 一并结论化，不再挂「只报不改」 | coder-1 |

**文件域**：`src/itn.rs`（与任何 ITN 任务互斥，不可并行派发）。

---

### 🧪 给 Gavin 的端测观察点（本批改了什么、该看什么）

| 编号 | 改了什么 | 端测怎么看 |
| --- | --- | --- |
| 030 标点全源治理 | 关掉自动标点开关后，**6 个产出源一视同仁**（原来只控 1 个） | 关开关说一段话，看是否**任何标点都不出现**（含引号、括号、`、；`列表分隔符），且标点位置留空格 |
| 030 短句 ≤5 | 开着开关时，字/词数 ≤5 的短句不加末尾标点 | 说「好的」「今天天气不错」，看末尾有无句号；≥6 字应正常加 |
| 030-A-2 成对符号 | `（笑）` 不再被剥成 `（笑` | 说带括号/引号的短句，看有无孤儿左半 |
| DEC-047 Qwen3 标点 | 在线 ASR 由「假设必有标点」改为**实测**，短句无标点时不再跳过标点引擎 | 用在线 ASR 说短句，看该补的标点有没有补上 |
| 031 万一守卫 | `万一` 不再变 `0.1万`；`亿万`/`百万`/`千万` 同族一并覆盖 | 说「万一下雨怎么办」「百万富翁」，看是否保持汉字 |
| 027 零回归 | 大额数字跨亿到个位 | 说「一千零四十六万八千七百四十一」应得 `10468741` |

---

### 📜 2026-08-09 20:5x TEST-EXEC-030 验收记录（已完成，保留供追溯）

**结果：A0–A7 零 FAIL，四族零回归全绿。** `itn::` 225 ｜ `punctuation::` 43 ｜ `transcription::` 105+4ign
｜ `llm::` 140 ｜ 主 crate 958+8ign ｜ src-tauri 53 ｜ `--list` 自洽 966==966。零生产代码改动。

**主控独立复算（未采信汇总表格）**：用源码 `#[test]` 计数逐项验证，六个数字全部吻合 ——
`itn.rs`=225／`punctuation/mod.rs`=43／`llm/mod.rs`=140／transcription 三文件 54+28+27=109／
src-tauri 22+5+26=53（`#[path]` 引入 `wordbook/mod.rs`+`wordbook/db.rs`）／总数 900+30+36=966。

**`itn::` 用例数争议已裁定为 225**：旧记录 212 失效，coder-1 报的 219→221 为 TEST-SYNC-030-B 之前的中间态。

⚠️ **[DOC-STATE-DRIFT-001] 复现**：tester-1 只更 CHANGELOG + logs 两份，handoffs 与 progress 零条目，主控代记（已标注）。

📌 **只报不改遗留**：`examples/probe_031.rs`（08-08 031 探测残留，未清理未入库）。因 cargo 会自动发现
`examples/`，它会被 `cargo check --all-targets` / `cargo test` 连带编译。**处置待 Gavin 拍板**（删除／入库／保留），
主控不擅自删（依据「不可逆操作必须单独成轮」）。

---

### 📜 2026-08-09 19:38 派发记录（已完成，保留供追溯）

> 🔴 **交接记录有一处不符，已更正**：handoffs 写「任务书原样可用」，但 `inbox/tester-1/task.md`
> 在 19:33 新 session 启动时**已被清空为 0 字节**。主控已按原设计**重写**任务书（7030B），
> 含 A0–A7 步骤、红条三分类纪律（①测试写错可改／②预期内行为变更须给来源依据／③真回归一律上报不动）、
> 四族零回归专项（017 重量链／026 货币链／027 大额数字+DEC-042／031 万一守卫）、
> 红线（禁生产代码改动、禁出包、禁 git 破坏性命令、禁 PowerShell 改 UTF-8、禁虚报须贴原始摘要行）。
> 另要求 tester-1 给出 `itn::` **实跑用例数**，裁定 coder-1 报的 219→221 与 handoffs 记录不符一事。

**三 Worker 启动状态（19:3x 实测）**：coder-1 `%2` / coder-2 `%1` / tester-1 `%3` 全部存活并已 ACK 就绪，
均为 `DeepSeek V4 Flash Free · OpenCode Zen`（与本文件旧记录的 glm-5.2 / kimi-k2.7 已不同）。
coder-1 为 **OpenCode 类型非 Codex**，故未发 `/permissions Full Access`。
本轮 tester-1 独占 `src/`，coder-1/coder-2 已通知空闲待命勿动 `src/` → 零文件级重叠。

**基线数字**（供对照）：`itn::` 221 ｜ `llm::` 134+9 ｜ `punctuation::` 38+10 ｜
`transcription::` 105 ｜ src-tauri 53。Vitest/pytest 本批 SKIP（零前端零 UI 改动）。

**回归风险最高的三处**：① `strip_punctuation` 标点由「删除」改为「换空格」（行为变更，
既有断言大概率绿转红，属②类不是③类）；② `main.rs` L2 收口块删来源判据 + 抽纯函数；
③ `itn.rs` 万/亿分支新增守卫。

**之后**：TEST-EXEC 通过 → BUILD-015 出包（**主控须明确下达「现在可以出包」**）→ Gavin 端测。

### 🔴 待 Gavin 拍板：阶段三规则开例外

**同一根因本批发作三次**，第三次已从「diff 变脏」升级为「主干编译失败」：

| # | 任务 | 后果 |
| --- | --- | --- |
| ① | TEST-SYNC-030 | 代码非 rustfmt-clean，被后续 coder 的 `cargo fmt` 连带归一，污染两个 commit 的 diff |
| ② | 同上 | 同上 |
| ③ | TEST-SYNC-030-B | `src/llm/mod.rs:4776` 多一个 `}`，**整个 crate 编译失败**，主控提交前 `cargo check` 才发现并修掉 |

**根因**：三阶段规则明令阶段三「禁止执行任何命令」，`cargo check` 也在禁止之列 →
tester-1 连括号配没配对都无从知道，**交付前不可能自查**。

**主控建议**：阶段三开白名单例外，只允许 `cargo fmt` + `cargo check`。
理由：规则要防的是「测到半成品、拿到假结果」，而 `fmt` 只格式化、`check` 只做类型检查，
**都不执行测试、不产出二进制**，与该风险无关；禁止它们反而直接导致交付物编译不过。
⏸ **Gavin 未拍板前规则不变**，继续由主控在提交前兜底。

---


## ⏸ 待 Gavin 拍板 · ITN-FIX-BIGNUM-027 遗留三条（2026-08-04 挂账，实施已全部结案）

> 027 全批 A–E 已提交结案（`136f70f` / `967cd8d` / `e79ed05` / `499d56e`），
> 完整实施记录与根因取证已迁入 `todo-archive.md`。本节只留**仍未拍板的三条断言**。
> 🔴 其中「十万个为什么」一条**已由 DEC-044 解决**（专名加入保护白名单），保留行文备查即可，无需再拍。

### ⏸ 待 Gavin 拍板三条（tester-1 已按现状锁定断言，标注「待拍板」）

| 用例 | 现状 | 问题 |
| --- | --- | --- |
| 一万亿 | `10000亿` | DEC-042 规则直接推论，但中文习惯说「1万亿」。三选项：`10000亿`（守规则）/ `1万亿`（万亿当一个量级）/ `1000000000000`（特例展开） |
| **十万个为什么** | **`10万个为什么`** | 🔴 **专名被转，书名被改写**。属 DEC-038 词表覆盖问题（与 `五毛钱`/`三毛钱` 同源），**非 027 引入**，不阻塞出包，是否开单待定 |
| 两万五百 | `20500` | 最小单位百 → 普通数字，主控判断符合规则，列此备查 |

---

## 📋 待派发 · ITN 成语漏保护：`三五成群` → `35成群`（2026-08-04 Gavin 端测顺带发现）

> **性质：漏保护 = 输出损坏**（比误保护严重）。本次侥幸被 LLM 救回并进了 suggestions，若 LLM 超时走本地兜底，用户直接看到 `35成群`。

**取证**：`target/release/debug.log:2797`
```
ASR:  再比如说有同学三五成群打小赌
ITN:  再比如说有同学35成群打小赌
LLM:  三五成群（改回来了，suggestions:["三五成群"]）
```

**派发时机**：⏸ **等 027 完成后再派** —— 与 027 同属 `src/itn.rs` / `itn-rules.toml` 文件域，并行会冲突（文件级重叠检查）。

**修法待定**：`三五成群` 是成语（不可推导的固定表达），按 DEC-038 属于「保护词表**应该**承载」的那一类，可走词表。但需先确认 `[protect.idioms]` 为何未覆盖 —— 若成语表本就该有它，那是词表缺漏；若是逐位串路径绕过了成语保护，则是规则层缺陷，须走规则层修。**开单前主控先做这项取证。**

---


## 文档更新规则

1. **只保留新产生、进行中、验证失败、待排期或其他待决的任务**
2. **已完成的功能任务立即归档到 progress.md**，不在 todo 保留历史
3. **测试同步/构建/出包任务归入 CHANGELOG**，不在此文档详列
4. **新任务产生时立即写入**，不批量补
5. **不列端测跟踪项**（Gavin 自行使用中测试，有问题会重新开单）

---


## 📋 待排期 · LLM-USAGE-OBSERVABILITY-001 解析 usage 缓存字段（Gavin 2026-08-04：先建待办，下次再做）

> 性质：**可观测性补齐**，改动极小，不碰提示词本身。
> 前置：无，可随时启动。

### 背景：97% 的 prompt token 花在固定前缀上

主控 2026-08-04 从 `target/release/debug.log` 实测：

| 场景 | system_prompt | prompt_tokens |
| --- | --- | --- |
| 多行（Notepad/文档类，`multiline_safe=true`） | **21,868 字符** | **~5,300** |
| 单行（IDE/终端/微信，`multiline_safe=false`） | **13,174 字符** | **~3,269** |

差额 8,694 字符是多行分支独有的 F3 格式契约 + 四语枚举标记清单（023 恢复的 132 个标记短语）。

构成：主体（L0-L3 分层 + F3 + 四语清单 + i18n 基座）~21,400 ｜ 场景 F4 块 226 ｜ `extra_instruction` ~210 ｜ 词库 12 条。

**用户实际语音内容仅 62-232 字符（~50-150 tokens），completion 仅 17-64 tokens** —— 即每次请求 **97% 以上的 prompt token 是完全相同的固定前缀**。

### 问题：我们看不到缓存有没有命中

DeepSeek 支持上下文缓存（相同前缀命中后计费更低、延迟更低），而我们的系统提示词**每次完全相同**，是理想的缓存对象。

但 `ChatResponse`（`src/llm/mod.rs`）**没有解析 usage 里的缓存相关字段** —— 日志只有 `prompt_tokens` / `completion_tokens` / `reasoning_tokens`，**无法判断缓存是否命中、命中率多少**。

**在补上这个字段之前，任何关于提示词成本的讨论都只是估算。**

### 与既有教训同源

`[LLM-COT-LEAK-001]` 教训 6 原文：

> HTTP 响应结构体只解析自己要用的字段，会丢掉排障关键信息。`finish_reason` / `usage` 是零成本可观测性，应该默认解析。本次因缺这两个字段，一个本可一眼看穿的截断问题耗掉了整轮研究任务才坐实。

**同一个毛病第二次出现**。上次缺 `finish_reason`/`usage` 害得排查绕了一整轮，这次缺缓存字段导致成本无法量化。

### 实施范围（预估：一个 coder 小任务）

1. `ChatResponse` 的 usage 结构体补解析缓存相关字段（**实施前先查证 DeepSeek 官方响应 schema 的确切字段名**，不要照猜）
2. `LLM response meta` 日志行追加缓存字段输出
3. 跨 endpoint 兼容：本项目允许用户填任意 OpenAI 兼容 endpoint，其他厂商字段名可能不同或缺失，**用 `Option` 容错，缺失不得影响主流程**
4. 不碰提示词本身，不改任何现有行为

### 后续可能的衍生

拿到命中率数据后才谈得上判断：是否需要为缓存友好而调整提示词结构（如把最易变的 F4 场景块与词库注入移到末尾，保证前缀稳定）。**数据出来之前不做任何提示词改动**（DEC-041 与 `[F3-UNORDERED-LIST-001]` 的教训：提示词改动必须由实测驱动）。

---

## 📋 待排期 · SCENE-FORMAT-DIMS-001 拆分 `multiline_safe` 为格式能力多维度（Gavin 2026-08-01：先建 todo，后续排期）

> 性质：**内部架构重构**，无用户可见配置项。需一次构建。
> 前置：无。可随时启动。**风险点是它动的正是刚稳定下来的 prompt 构建链。**

### 问题

`multiline_safe`（单 bool）在 `src/main.rs:3073` 传入 `build_optimize_request` 后，**一个人决定了三件事**：

| 它控制 | 位置 |
| --- | --- |
| `<corrected>` 能否跨行 | `build_output_format(multiline_safe)`（`src/llm/mod.rs:795`） |
| F3 用多行列表还是单行内联 | `build_format_instruction_block(multiline_safe)`（`:818`） |
| 是否 `flatten_multiline` 压成一行 | `src/llm/mod.rs:660` |

**这几个问题的答案并不总是一致。已实际撞上一次**：代码编辑器要换行、不要 `- ` 项目符号 —— 当时只能用「方案 A：接受列表」绕过（Gavin 2026-08-01 拍板）。

### 需求矩阵（主控已梳理）

| 场景 | 换行 | 列表 | `#` 标题 | 表格 |
| --- | --- | --- | --- | --- |
| Markdown 编辑器 | ✅ | ✅ `- ` | ✅ 想要 | ✅ 想要 |
| 代码编辑器 | ✅ | ❌ 不想要 | ❌ | ❌ |
| Word / WPS | ✅ | ✅（会自动转项目符号） | ❌ | 视情况 |
| 记事本 | ✅ | `- ` 仅字面字符 | ❌ | ❌ |
| 邮件 | ✅ | ✅ | ❌ | ❌ |

### 设计草案

```toml
newline    = true          # 能否多行注入
lists      = true          # 能否用列表
list_style = "markdown"    # markdown(- / 1.) | plain(• / 1.) | none
headings   = false         # 能否用 # 标题
tables     = false
```

### 影响面

`scene-rules.toml` schema ｜ `SceneRule`（`src/scene/mod.rs:94`）｜ `SceneContext`（`:55`）｜ 解析与传递 ｜ `build_output_format` / `build_format_instruction_block` 从 bool 参数改结构体 ｜ 相关测试。

### 实施前必查

1. **回查 DEC-031 单开关原则** —— 主控初判不触碰（这是内部规则数据，非用户可见开关），但须正式确认
2. 跨平台：`src/scene/mod.rs` 与 `src/llm/mod.rs` 均为平台中立模块，对 macOS 透明，不违反 DEC-033
3. **迁移成本随词表增长而上升** —— 现已 9 个 scene 块，越晚做越贵

### 附带清理（本项一并处理）

`scene-rules.toml:15` 注释称 `multiline_safe` 还控制「强制剪贴板」，**主控 grep 注入侧代码零引用，该句为过时描述**，一并订正。

---

## 📋 待排期 · SCENE-FOCUS-PROBE-001 焦点控件类型探测（UIA / AX）（Gavin 2026-08-01：先建 todo，后续排期）

> 性质：**架构级新能力**，跨平台，需完整评估。
> 🔴 **前置硬门槛：必须先做 PoC，PoC 不通过则不立项。**

### 问题：只有窗口级信号，要判断的却是控件级行为

场景识别拿到的是 `(进程名, 窗口标题)`，**两者都是窗口级**。而「Enter 是换行还是提交」取决于**当前焦点控件**：

| 应用 | 同一窗口内的分歧 |
| --- | --- |
| VS Code / Cursor | 编辑器（Enter=换行）／集成终端（Enter=执行）／AI 面板（Enter=发送） |
| Todo 软件 | 快速添加框（Enter=建任务）／详情备注（Enter=换行） |
| 设计软件 | 文本图层（Enter=换行）／评论框（Enter=发送） |
| 浏览器 | 文档编辑区／评论框／搜索框 |

**当前所有残留误判风险的总根源。** 现在是「按应用整体赌一个答案」。

### 技术路径

| 平台 | API | 信号 |
| --- | --- | --- |
| Windows | `IUIAutomation::GetFocusedElement()` | `ControlType`：`UIA_EditControlTypeId` ≈ 单行；`UIA_DocumentControlTypeId` ≈ 多行。辅以控件 ClassName（如 Windows Terminal 的 `TermControl`） |
| macOS | `AXUIElement` → `AXFocusedUIElement` | `AXRole`：**`AXTextField` = 单行、`AXTextArea` = 多行**。信号比 Windows 更干净 |

接缝已存在：`capture_scene_signals` 两平台签名同形（Windows `src/platform/windows/scene.rs:22`，macOS `src/platform/macos/mod.rs:79` 仍为 stub）。

### 🔴 PoC 必须先回答的三个问题

1. **Chrome / Electron 能否拿到有意义的焦点控件类型？**
   Chrome 默认不开放完整无障碍树（只在检测到读屏器时启用），Electron 应用同理。
   **而 `chrome.exe` 占真实听写量的 36%（151/414，主控 2026-08-01 从 debug.log 实测）。拿不到 = 方案价值砍半。这是头号风险。**
2. **耗时**：跨进程 COM 查询通常 5–50ms，但**对无响应应用可能挂住** → 超时与降级策略是否可行
3. **VS Code 编辑器 vs 集成终端**能否区分 —— 这是最想解决的那个用例

**PoC 规模：半天量级，只验上述三点，不写生产代码。**

### 其他约束

- 跨平台：Windows COM + macOS AX 两套实现，DEC-033 要求不得只产出仅 Windows 可编译的新代码
- 隐私：**严格只取控件类型，绝不读取控件内容**
- 失败降级：拿不到信号时必须退回现有 `(exe, title)` 判定，不得阻塞管线

### 与 SCENE-FORMAT-DIMS-001 的关系

两者正交但互补：本项解决「**当前焦点适合什么**」，前者解决「**适合的东西怎么表达**」。建议**先做拆维度**（收益确定、成本有界），本项走 PoC 门禁。

---

## 📋 待裁定 · 领域级泛化关键词第二批（coder-1 建议，主控未采纳未否决）

coder-1 在 DATA-SCENE-GENERIC-008 中评估后建议的候选：**`思维导图` / `白板` / `表格`**。

已评估并**否决**的：`笔记`（小红书网页标题大量含之，社交类判 doc 方向反了；「笔记本电脑」购物页同样命中）、`文档`（帮助/API/产品文档等阅读页覆盖过宽，失去判别意义）。

**裁定判据（主控 2026-08-01 定）**：看误伤会落到哪个方向 ——
- 误伤 → **doc**：只多给多行与列表，文本仍正确，属优雅降级，**可放宽**
- 误伤 → **把 chat 类应用判成 doc**：多行注入发帖/发消息框，可能把一条拆成多条发出，**必须卡严**

待 Gavin 拍板。

---
## 📋 待排期 · ITN-V2-P6 能产语法族批量收口（Gavin 2026-08-01：先出包，P6 另立）

> 依据：`collab/research/itn-v2-grammar-family-scan-006.md` 的 130 族分类结果
> 前置：本轮出包端测反馈

**范围**：75 个 🔴 能产语法族（`N点钟`/`N秒钟`/`N块钱`/`N毛钱`/`N角钱`/`N位数`/`N年级`/`N节课`/`N件套`/`N句话`/`N日游` 等）从 `[protect.unit_collisions]` 移出，交甲/乙/丙型文法处理。

**核心问题**（Gavin 已实感）：`五毛钱` 在表保持汉字、`三毛钱` 不在表变 `3毛钱` —— 同一表达因数值不同行为完全相反（DEC-038）。

**实施前必须**：① 逐族确认已有文法覆盖，避免移出后变成「漏保护」（输出改坏，比未优化严重）② 反向护栏：55 个 ⚪ 专名族不得误伤 ③ 预判并同步既有断言 ④ 涉及数百词条，需完整回归 + 出包

### 📌 2026-08-04 补充：分类扫描已完成，Gavin 拍板暂缓

> **Gavin 2026-08-04 决定**：「先保持原样，后面我端测了有问题再开单」。**P6 不启动，等端测反馈。**

主控已完成当前词表复扫，成果存 **`collab/research/itn-v2-p6-family-classification-001.md`**，含：

- 129 个多数字族的**四组分类**（A 数量表达 37 移出 / B 地名编号 49 保留 / C 术语 41 保留 / D Gavin 指定保留 2：**N年级 / N节课**）
- 🔴 **新发现**：`一个*` 前缀 **134 条占全表 10% 是挖掘噪声**（`一个三十多岁` 会冻结「三十」不转），删除零专名风险，是 P6 成本最低的第一刀
- 🔴 **新发现**：979 个单例族占 72%，**按族裁决方法在此失效**，需改按尾字聚类
- 三处待 Gavin 裁定：`N分熟` / `N米板` / `N岁时·N月底`

⚠️ **四组分类是主控单方初判，Gavin 未逐组确认**，P6 启动时必须先过一遍，不得直接采信。

**启动前另一道门**：CHANGELOG 026-B 的「12 词条实测确认」已被 TEST-EXEC-026 证伪 4 条，须先做 026 前后行为差分。

### 八、待 Gavin 决定的两项（非阻塞）

- `dispatch.sh` 路径分叉修复时机（`[COLLAB-PATH-SPLIT-001]`）
- 是否加 `.gitattributes` 收口 CRLF 幽灵 diff（`core.autocrlf=true` 且无 `.gitattributes`，已两次干扰验收判断；属仓库级改动、影响 macOS 侧，需协调）

---


## 🍎 [macOS 侧] 2026-08-04 · Phase 4 管线实现规划（**规划稿，尚未派发**）

> 来源：Gavin 2026-08-04 指令「把 macOS 侧管线 Phase 4 的任务规划出来」
> **完整方案见 `collab/research/macos-phase4-plan-001.md`**（含逐任务的影响文件 / 具体改动 / 验收标准 / 边界矩阵）
> 基线：`0adb819`（v0.7.3），主控本次逐文件实测，未采信既有文档结论

**🔑 核心发现（改变了任务分解方式）**：`run_pipeline`（`main.rs:2812-3214`，402 行）与 `spawn_worker_thread`（`:2161-2459`，299 行）虽整体被 `#[cfg(target_os="windows")]` 门控，但**平台接触点分别只有 4 个和 1 个**，其余全是平台中立业务逻辑。**约 700 行核心管线可一次性转为双侧共享**，macOS 侧无需重写任何业务逻辑。这同时修正了 `docs/MACOS-HANDOFF.md` §2.8「管线代码层无法共享」的表述——那是当前状态，不是固有属性。

| 阶段 | 编号 | 内容 | 建议负责人 | 状态 |
| --- | --- | --- | --- | --- |
| A | MACOS-P4-PROBE-001 | 实机可行性探针：cpal 实录 / sherpa-onnx 实跑 / TCC 权限 / CGEventTap 实收 / AX 授权弹窗 | tester-1 | ✅ **已验收**（A1/A2/A5 PASS，**A4 FAIL 查出真 bug**，A3 强度不足待复验） |
| A+ | MACOS-P4-FIXHOTKEY-001 | 修 `KEYBOARD_EVENTS` 事件掩码越界（A4 的下游修复） | coder-2 | ✅ **已验收**（2026-08-04，主控独立复跑 `cargo check --all-targets` 0 errors） |
| B | MACOS-P4-NEUTRAL-001 | 管线去平台化：`HWND`→`WindowId(usize)`、新增契约符号 `foreground_window_id`、去 cfg 门控 | coder-1 | ⏸ **归属待拍板** |
| B | TEST-SYNC-P4-NEUTRAL-001 | 契约单测 + macOS 可达性烟测 | tester-1 | ⏸ |
| C | MACOS-P4-HOST-001 | macOS 事件宿主 + 主循环 + 单实例锁 | coder-2 | ⏸ **选型待拍板** |
| C | MACOS-P4-TRAY-001 | macOS 托盘（**不可与 HOST 并行**） | coder-1 | ⏸ |
| C | **MACOS-P4-OVERLAY-001** | 录音浮层（Recording，P0）NSPanel 1:1 复刻 Win32 | coder-1 | ✅ **已验收**（2026-08-05，返工后主控独立取证：BORDER_GRAY `#070606` 已修正、性能四项已补数、SIGBUS 悬垂指针真缺陷已重构、probe 已删、工作区干净，提交 `d7b9f39`） |
| C | **MACOS-P4-OVERLAY-WIRE-001** | **浮层接线**：接进模块树 + 跨线程请求通道 + 15ms timer 主线程应用 + PipelineEvent 七分支映射 | coder-1 | ✅ **接线部分已验收**（主控独立取证：`mod overlay;` 已加、overlay 5 条单测**首次真进测试二进制**（主控自跑 `--list` 确认）、Windows 导出块零改动、七分支齐全）。⚠️ **实机闭环仍降级**，转入 CFGGATE-001 C 项 |
| **P0** | **MACOS-P4-CFGGATE-001** | 🔴 **修 macOS 专用函数缺 cfg 门控致 Windows 编译必炸**：`main.rs:2880/:2930` 无 `#[cfg]` 却引用 `request_tray_state`（TRAY-001 既有）+ `request_overlay`/`OverlayRequest`（WIRE-001 新增 7 处）。**Windows 侧已编不过一天多无人察觉**。含 11 个 macOS 专属符号全量扫描 + 实机重试 | coder-1 | ✅ **已验收**（主控**自写脚本独立重扫**，非采信 Worker 表：11 符号全部落 cfg(macos) item 内、未门控引用点 **0**；`cargo check --all-targets` CARGO_EXIT=0 / error 0；diff 恰好 +2 行；**主控追加全仓扫描**确认 macos 目录与 main.rs 之外零引用。🟢 **Windows 编译阻塞解除**） |
| C | TEST-SYNC-P4-OVERLAY-WIRE-001 | 阶段三**预研**（只读零改动，产出写 outbox）：5 条单测走查 + 七分支真值表 + 线程契约 + 盲区预估 | tester-1 | 🔄 **进行中**（与 WIRE-001 并行安全，因零文件落地） |
| C | MACOS-P4-OVERLAY-002 | 处理中 / 失焦返显 / 错误三态浮层 + **指示灯形状裁定**（Windows 实为麦克风图标，现实现为实心圆，主控裁定本轮不动） | 待定 | ⏸ |
| ~~C~~ | ~~MACOS-P4-FEEDBACK-001~~ | ~~托盘状态替代浮层~~ → **已按 DEC-036 取消替代定位**（Gavin：不能用托盘图标代替录音窗口，不符合用户体验） | — | ❌ 作废 |
| D | MACOS-P4-SCENE-001 | `capture_scene_signals` 真实现（NSWorkspace + AXUIElement） | coder-1 | ⏸ |
| D | MACOS-P4-PERM-001 | `AXIsProcessTrustedWithOptions` 真实现（现为只打 log 的假实现） | coder-2 | ⏸ |
| D | MACOS-P4-AUTOLAUNCH-001 | 自启动（建议 SMAppService） | 待定 | ⏸ |
| **P1** | **MACOS-P4-AXINJECT-001** | **辅助功能 API 直写替代剪贴板注入**（解决剪贴板被污染/图片被永久覆盖）—— Gavin 2026-08-05 拍板独立 P1；**同日 12:1x 派发 coder-2**。三条主控裁定：①必须 `kAXSelectedTextAttribute`，**严禁** `kAXValueAttribute`（会抹掉用户已输入内容）②必须 `AXUIElementSetMessagingTimeout(0.5)`（否则无响应 App 会挂死 pipeline worker 线程）③必须检查 `AXError` 返回码 | coder-2 | ✅ **已验收**（三条裁定逐条 Read 核对全部落实；CARGO_EXIT=0/error 0；仅 `injection.rs` +122/−1）。🔴 **但主控 review 查出静默丢词风险** → 见下行 002 |
| **P1** | **MACOS-P4-AXINJECT-002** | 🔴 **堵 AX 静默丢词**：部分实现 AX 的 App（Electron/Java Swing）接受 `SetAttributeValue` 并返回成功却不真插入 → `inject_text` 拿 Ok 即 return、**永不走剪贴板兜底** → 用户整段话消失。**推翻了 001 立项前提「最坏等同现状」**。修法：`AXUIElementIsAttributeSettable` 预检 + AX 成功日志带 `AXRole`。已否定两条歧路（写后读回：选区塌缩读回空串不可作判据；role 白名单：误伤自定义控件） | coder-2 | 🔄 **进行中** |
| C | **MACOS-P4-OVERLAY-WIRE-002** | 抽纯函数 `overlay_request_for_event` 使七分支真值表可单测（**强制穷举 match，禁 `_ =>` 通配符**——通配符会让将来新增 PipelineEvent 变体静默落进 Hide）。由来：tester-1 预研指出映射内联在 handler 里夹三种副作用、单测调不动 | coder-1 | 🔄 **进行中** |
| C | TEST-SYNC-P4-OVERLAY-WIRE-001 预研 | ✅ **已交付**（13683 B）。**核心发现（主控复核认同）：overlay 现有 5 条单测 4 条凑数 1 条半有效** —— `decay_rate_matches_windows` 是 `assert_eq!(常量, 它自己的字面量)`；三条波形测试独立重算公式不调生产路径，且 `lx<rx`／`8<=16` 恒真。**本次接线目前零真护栏** | tester-1 | ✅ 预研完成，阶段三待 WIRE-002 交付 |
| D | MACOS-P4-READBACK-001 | AX 回读（学习路径）—— ⚠️ **主控已收回「相对 Windows 的能力优势」表述**：300ms 观察窗 + Word/Google Docs 读不到 + 全文 diff 昂贵，三重打折后产出存疑；建议优先考虑 `WORDBOOK-CORRECTION-UI-001` 显式纠错路线 | 待定 | ⏸ **降级** |
| ~~E~~ | ~~MACOS-P4-OVERLAY-001~~ | 已上移至阶段 C（DEC-045） | — | ↑ |
| E | MACOS-P4-BUNDLE-001/002 | `.app` 打包 + Info.plist TCC 声明 + 自签名证书（BUNDLE-002 已改自签名，禁 ad-hoc） | coder-2 | ✅ **已完成**（BUNDLE-002 2026-08-05：`build-macos.sh` 自签名 Feiyin Dev，实机 Build completed + Authority=Feiyin Dev；详见 outbox/coder-2/result.md）。⚠️ 公证仍不可做（无 Apple Developer 账号） |

**边界评估结论**：阶段 B 独占 `src/main.rs`，期间禁止任何其他任务碰它；阶段 C 的 HOST 与 TRAY **必须串行**；阶段 D 的 SCENE / PERM / AUTOLAUNCH **可三路并行**，唯一交汇点 `macos/mod.rs` 的 re-export 行由主控统一改一次。

**⏳ 阻塞在 Gavin 决策上的 5 项**（详见方案 §4）：① 阶段 B 归属与 Windows 零回归验证方 ② 事件宿主选型（**建议复议 DEC-015 的 Tauri，改用 winit**）③ 四个浮层是否进第一版 ④ Apple Developer 账号 ⑤ AX 回读是否提前。

---

## 🍎 [macOS 侧·待排期] MACOS-P4-AXINJECT-001 · 辅助功能 API 直写替代剪贴板注入

> **⚠️ 本条仅适用于 macOS 侧，Windows 侧无需处理**（Windows 走 SendInput / 剪贴板双通道，机制不同、无此缺陷）。
> 来源：Gavin 2026-08-04 端测关切「现在的回写方式确实会污染剪贴板，干扰到用户剪贴板输入」。
> 参照：竞品 **Typeless** 官方文档明示其用 Accessibility 权限「paste text into any text field」，**不经剪贴板**（[installation-and-setup](https://www.typeless.com/help/installation-and-setup)）。

### 现状与缺陷（主控实测 `src/platform/macos/injection.rs:49-64`）

当前 `inject_via_clipboard` 流程：`pbpaste` 存旧内容 → `pbcopy` 写新文本 → sleep 50ms → enigo 发 Cmd+V → sleep `delay_ms` → `pbcopy` 恢复旧内容。

**已有恢复逻辑，但存在四个真实缺陷（按严重度排序）**：

| # | 缺陷 | 后果 |
| --- | --- | --- |
| **1** | **`get_clipboard_text()` 只用 `pbpaste`，仅支持纯文本**（`:119-134`） | 用户剪贴板里若是**图片 / 富文本 / 文件 / 多格式数据**，`old_content` 取不到 → `None` → **恢复分支根本不执行** → **用户原剪贴板内容被永久覆盖丢失**。这不是"污染"，是**数据丢失** |
| 2 | 恢复是尽力而为（`let _ = set_clipboard_text(&old)`，忽略错误） | 恢复失败无感知、无日志 |
| 3 | 约 **100ms+ 时间窗**内剪贴板是我们的内容 | 用户此刻手动粘贴会拿到错误内容 |
| 4 | **剪贴板历史工具**（Paste / Maccy / Raycast 等）会记录每一次写入 | 每次听写都往用户剪贴板历史里塞一条垃圾，长期使用体验很差 |

### 目标方案

用 **AXUIElement 直写**替代剪贴板通道：取焦点元素（`kAXFocusedUIElementAttribute`）→ `AXUIElementSetAttributeValue` 写 `kAXValueAttribute`，或用 `kAXSelectedTextAttribute` 做插入。**全程不碰剪贴板。**

### 实施要点

- **保留剪贴板通道作为兜底**：AX 对某些 App（Electron / 部分 Java App / 未实现 AX 的自绘控件）不可用，失败时回退现有 `inject_via_clipboard`，与 DEC-018「clipboard-first + enigo fallback」的分层思路一致，只是把优先级改为 **AX → 剪贴板 → enigo.text()**
- **权限已具备**：辅助功能权限本就是热键（CGEventTap）的前置条件，`MACOS-P4-PERM-001` 已补全授权弹窗，**不需要新增任何权限申请**
- **保底路径不受 App 差异影响**：AX 写不进去就回退现有剪贴板通道，**最坏情况等同现状，不会更差**

### 🔴 与 `MACOS-P4-READBACK-001` **不得捆绑**（Gavin 2026-08-05 拍板：AXINJECT-001 独立 P1）

> **主控此前建议"两者合并为一个批次"，该建议已作废并收回。** 它们只是碰巧用同一套 `AXUIElement` API，但**价值与风险完全不同**，捆绑会让高价值的直写被低价值的回读拖累。

| 项 | 价值 | 依赖 |
| --- | --- | --- |
| **AXINJECT-001（本条，注入）** | **高且确定**。解决真实数据丢失 + 剪贴板历史污染。**只需"写得进去"，有剪贴板兜底** | 无 |
| `READBACK-001`（回读，学习） | **存疑**，见下 | 受三重打折 |

**主控收回的一处过乐观表述**：此前写「macOS 的 AX 能真正读回，是本平台相对 Windows 的能力优势」。**该表述不准确**，实测复核后三点打折：

1. **观察窗只有 300ms**（`main.rs:161` `AUTO_LEARN_OBSERVE_MS = 300`，`:3662` sleep 后即读一次就结束）。用户不可能在 300ms 内看完注入文本、发现错字并改掉——**真实纠错发生在数秒后，这条路径设计上就抓不到几乎任何真实纠错**。此缺陷与平台无关，Windows 侧同样存在
2. **AX 覆盖面确实比 `WM_GETTEXT` 广**（原生 NSTextView / 备忘录 / Safari 输入框可读，而 `WM_GETTEXT` 在现代应用基本全废），**但读不到 Microsoft Word（非原生 AX 文本控件）与 Google Docs（编辑面是 canvas）** —— 而这恰是长文写作主场
3. **全文 diff 代价高**：`extract_changed_text`（`main.rs`）走公共前缀+后缀夹逼，**数学上对整篇文档成立、不需要知道光标位置**，但要把全文 `chars().collect::<Vec<char>>()` **两次**（before/after）。50 页文档每次听写数十 MB 临时分配

**→ 真要修好「注入后纠错学习」，正确路线不是 AX 回读，而是既有待排期项 `WORDBOOK-CORRECTION-UI-001`**（注入后浮层显示「纠错」小按钮，用户点了才进编辑）：**显式交互，不猜、不轮询、不受 App 差异影响，且没有时间窗问题**。

### 影响文件（预估）

`src/platform/macos/injection.rs`（主体）+ 可能新增 `src/platform/macos/ax.rs`。**不碰 `src/platform/windows/**`，不碰平台中立模块，对 Windows 侧零影响。**

### 优先级：**P1 独立排期**（Gavin 2026-08-05 拍板）

**不阻塞当前闭环** —— pbcopy+Cmd+V 现在能工作。但 Gavin 日常使用会持续被缺陷 1（复制的图片被永久覆盖）与缺陷 4（剪贴板历史被塞垃圾）硌到，**Phase 4 主体交付后立即排期，不等 READBACK**。

---


## 📋 待排期 · ITN-COLLISION-TYPEB-001（Gavin 2026-07-30：先生成待办，以后再动手）

> 来源：RESEARCH-ITN-LEXICON-001（`collab/research/itn-lexicon-collision-001.md`）
> 前置：Type A 净化版落地（进行中）｜ **本项需改 Rust 代码，不是纯数据**

**问题**：`is_unit` 用 `s.starts_with(u)`（`src/itn.rs:259`）+ 中文无词边界 → 任何以单字单位词开头的词都会让前面的单字数字被误转。已实证一例（`三角形`→`3角形`，已由几何白名单挡住）。潜在碰撞词 **29,774 条**（Type B：单位字开头，如 `批发`/`元素`/`度假`/`节目`/`升级`/`克服`）。

**⚠️ 主控核实的关键结论（研究报告未发现，实施前必读）**：

1. **Type B 词放进 `protect.proper_nouns` 是 no-op** —— `check_protection`（`:1063`）的匹配起点是**数字位置**（`rest = chars[start..]`，`start` 为数字下标，调用点 `:678`），既有条目全是数字开头（`三亚`/`五一`/`八达岭`）。文本「三度假」的 `rest` 是「三度假」，**不以「度假」开头**，永不命中。研究报告建议的落地方式对这 93% 的词无效
2. **正确改法在 `is_unit` / `decide_conversion` 侧**：判断「数字之后的文本是否以某碰撞词开头、且该词比单位匹配更长」→ 是则不算单位语境
3. **性能必须一并处理**：`check_protection` 是对 **Vec 线性遍历** `starts_with`（不是 HashSet 查表，报告此处亦有误）。29,774 条 × 每个数字位置 = 每次听写十万量级字符串比较 → 需改**按首字分桶或 Trie**，并给实测数据

**实施前必须完成**：许可证原文证据（见 Type A 任务）｜ 完整影响面评估 ｜ 性能实测 ｜ 反向护栏测试（must-convert 表达不被误保护）

---


## ⏸ 待 Gavin 拍板 · RESEARCH-SCENE-COVERAGE-001 场景词表扩展研究（2026-07-28 方案已回）

> 从原 `todo.md`「进行中」大节中抽出——该大节其余内容（macOS 07-30 交接接管等）已迁入 `todo-archive.md`，
> 只有本项仍**待 Gavin 拍板三项**，故保留在 todo。

### 2026-07-28 · RESEARCH-SCENE-COVERAGE-001 场景词表扩展研究（方案已回，**待 Gavin 拍板三项**）

| 编号 | 内容 | 产出 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| RESEARCH-SCENE-COVERAGE-001 | 场景感知软件词表与分类体系扩展研究（纯研究，零文件改动） | `collab/research/scene-rules-expansion-001.md`（397 行） | coder-1 | ✅ 已验收（**含主控三处实质修正**） |

**主控独立核验结果**：

- **边界合格**：`git status` 零改动，`scene-rules.toml` / Rust 源文件全未碰；result.md 4300 字节非空（[COLLAB-WRITE-001] 未复发）
- **✅实测层可信**：抽查 6/6 全部属实——Obsidian 目录确无 `Obsidian-helper.exe`、`D:\xshell\Xagent.exe` 存在、`MarkText.exe` / `Koodo Reader.exe` 路径正确、`wezterm.exe` 确在 `%LocalAppData%\Programs\WezTerm\`（主控首轮搜索因未覆盖该根目录误判为不存在，二次广域搜索证实 coder-1 无误）、四个 AppxManifest 的 Executable 字段逐一复读一致（OneCalendar→`CalendarApp.Gui.Win10.exe`、OutlookForWindows→`olk.exe`、MSTeams→`ms-teams.exe`、Claude→`app\Claude.exe`）
- **❌ 主控修正一：10 条 title_keywords 是 no-op**。报告建议把「微博/小红书/知乎/Jira/TAPD/禅道/Linear/Teambition/Salesforce/Zendesk」加进 **browser 块**的 title_keywords，目的是「浏览器场景细分时命中」。但 `src/scene/mod.rs:162-164` 浏览器细分循环里有 `if other_rule.kind == SceneKind::Browser { continue; }` —— **显式跳过浏览器自身的 title_keywords**。浏览器 exe 在优先级 1 命中后直接返回 Browser，这 10 条永远不会生效（只有在 exe 未命中任何规则的优先级 2 路径才会被查，那不是目标场景）。报告第 6.2 节还引用 `:159-177` 称该机制「已正确处理」，属误读 `continue` 分支。**结论：10 条全部作废**；若确要生效，必须放进**非 browser 的 kind 块**（如 chat）
- **❌ 主控修正二：`OneNote.exe` 是零效果重复条目**。exe 匹配大小写不敏感（`:133` 编译期 `to_lowercase` + `:152` 输入 `to_lowercase`），`OneNote.exe` 与表内既有 `ONENOTE.EXE` 归一化后完全相同。报告自己在第 127/313 行两次援引「toml 不区分大小写」，此处却当作新增项，自相矛盾。**22 条新增实际净 21 条**
- **⚠️ 主控修正三：✅官方 置信度标注失真（最重要）**。报告附录第 397 行自述「WebFetch 核实国内站点被 JS 占位，**主要依靠本机实证 + 通用软件知识**」——「通用软件知识」即凭记忆。故 15 条标 ✅官方 的条目（豆包/Kimi/通义/文心/ChatGLM/GLM/纳米/Perplexity/Zoom/腾讯会议/Linear/Figma/Mailbird/The Bat!/Nu），其证据实际只支撑「该产品有 Windows 版」，**不支撑「进程名就叫 X」**。这恰好触碰本次任务设定的核心红线。**须降级为 ⚠️ 未证实**
- **争议项（主控不同意见）**：报告建议删除 6 条历史推测项。主控评估**成本收益不对称**——不存在的 exe 名在运行时永不命中、开销为零（HashSet 查表），删除买不到任何收益；而万一其中某条在特定版本真实存在，删除即制造静默回归。**建议改为保留 + 注释标注「未证实」**，与 DEC-031-⑤「词表尽可能详细」一致
- **分类结论认同**：D5 六个候选场景全部归入现有 5 类、不新增 kind——主控同意（新增 kind 要改 Rust 枚举 + 解析 + 单测 + 重新出包，收益不足）。但 **Figma 归 ide_terminal 存疑**：该 style 是「保留技术术语/代码标识符、无客套」，而 Figma 文本框内容多为设计文案而非代码，`browser` 的「web-friendly 简洁单行」更贴切

**Gavin 三项拍板（2026-07-28，全部按主控建议）**：
1. **6 条历史推测项** → **保留 + 注释标 ⚠️未证实**（不删除）
2. **15 条未证实 exe** → **落地 + 注释标 ⚠️未证实**（错条目零成本；待 SCENE-OBS-001 日志实证）
3. **Figma** → 归 **browser**（非 ide_terminal）

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| IMPL-SCENE-COVERAGE-001 | 词表扩展实施：6 条存疑项保留改注释 + 新增 21 条 exe（✅实测 6 / ⚠️未证实 15）+ doc 块补 4 条工单 title_keywords + Figma 归 browser + Skype 注释 | `scene-rules.toml`（仅根目录） | coder-1 | ✅ **已验收（含主控两处修正）** |
| TEST-SYNC-SCENE-COVERAGE-001 | 测试同步：P0-1 `BUILTIN_RULES` 解析单测（堵住「toml 语法错→静默全 Unknown」黑洞）+ P0-2 特殊字符条目（`The Bat!.exe` / `Koodo Reader.exe`）+ P0-3 doc 块 title_keywords 浏览器细分生效 + P0-4 反向护栏锁死 `:162-164` continue + P0-5 归类决策断言 + P1 大小写不敏感 | `src/scene/mod.rs` `#[cfg(test)]` | tester-1 | ✅ **已验收** |
| TEST-EXEC-SCENE-COVERAGE-001 | 全量回归 686/0/8 + Tauri 53/0 + Step2/3/4 SKIP + 三副本 sha256 一致 + 新实例 PID 23056 零 parse error | — | tester-1 | ✅ **已验收（运行时实证已由 Gavin 补证闭环）** |
| BUILD-RELEASE-SCENE-COVERAGE-001 | 出包：**只重建主程序**（`cargo build --release`），把 165 条新词表经 `include_str!` 嵌入内置默认；**跳过 Tauri UI 构建**（`src-tauri/**` 与 `ui/**` 零改动，重建只产出功能等价二进制）；关键验证=新 exe 内检索 `NanoSearch.exe` / `Koodo Reader.exe` 两个探针字符串确认缓存未复用旧产物；版本号维持 0.7.2 不动 | — | tester-1 | ✅ **已验收（含主控一处事实修正）** |

**BUILD-RELEASE 验收记录（2026-07-28 主控独立取证）**：

- **产物 sha256 两副本一致**：`e35679bd…`（target/release + Publish）；crash-reporter `8bfabfb5…` 亦一致——tester-1 报告写「继承，未查」，实际它 18:41 已随 `cargo build --release` 一并重建并同步，主控补查确认无误
- **词表嵌入验证主控独立换探针**：不复用 tester-1 那 5 个，改用 6 个字符串复查，含**最易触发 toml 解析问题的 `The Bat!.exe`（含 `!`）与 `Koodo Reader.exe`（含空格）**、以及本批次归类决策项 `Figma.exe` 与 `Teambition`，全部命中 → 缓存确未复用旧产物，165 条已 `include_str!` 嵌入
- **ProductVersion 0.7.2.0 未变**；`scene-rules.toml` 三副本 sha256 `7b01b33c…` 一致
- **边界合规**：`git status` 仅 `CHANGELOG.md` 一处改动，**源文件零改动**、未碰 Tauri UI / npm build / 版本号；项目级 result.md 1251 字节非空且对应本任务
- **❌ 主控修正 · 「已知缺口」不成立**：报告称「`Publish/voice-ime-ui.exe` 沿用 07-24 旧版 10,013,184 B」。**Publish/ 根本没有该文件**——已于 07-28 按 Gavin 指令删除。Publish 内是正确的 `feiyin-ime-ui.exe`（10,027,008 B，07-27 20:31，sha256 `0d76eca1…`，即 v0.7.2 那版）。那个 07-24 文件只在 `target/release/`（本次已随磁盘清理删除），且生产代码 `src/main.rs:436~478` 七处全走 `feiyin-ime-ui.exe`。**本次出包不存在 UI 缺口**
- **⚠️ 观察（不阻塞）**：冒烟实例 PID 18928 在主控 21:40 复核时已不存在，符合 [SMOKE-VANISH-001] 既有模式，产物结论不依赖该进程

**git 收口（2026-07-28）**：提交 `695e50e`（3 文件 +195/−7：`scene-rules.toml` / `src/scene/mod.rs` / `CHANGELOG.md`），已按 Gavin 指令 push → `fb230f9..695e50e`，push 后恢复 clean remote URL，`.git/config` token 残留数 0。

**⚠️ 已知副作用（出包后确认，待 Gavin 定夺）**：本次维持 **0.7.2 不升版**（Gavin 未指示升版，遵守「版本号禁止擅改」）。故现存在**两个内容不同、但 ProductVersion 均为 0.7.2.0 的主程序构建**：07-27 20:31 那版内置 144 条词表（sha256 `7fbb1e4b…`，已被覆盖）/ 07-28 18:42 本版内置 165 条（sha256 `e35679bd…`，当前 Publish 中的）。可追溯性下降，仅靠 sha256 区分。若更看重可追溯性可升 0.7.3 重出包——但注意**本次磁盘清理已删除 release 中间产物，重出包需全量重编（含 CTranslate2 C++，预计 20+ 分钟）**，不再是 2 分钟。

**TEST-EXEC 验收记录（2026-07-28 主控独立取证）**：

- **数字链自洽**：686 = TEST-EXEC-20260727 基线 672 + 本轮 TEST-SYNC 新增 14，三方来源相加严丝合缝
- **主控定向抽跑 `builtin_rules_parse_ok` 单条 → ok**（未只信汇报表格）。这条是本批次最大风险的唯一守门人，**至此真实 165 条 toml 可被 `toml` crate 正确解析已获权威验证**。注：`BUILTIN_RULES` 是根目录 `scene-rules.toml` 的 `include_str!`，与运行时加载的 `target/release/scene-rules.toml` **sha256 逐字节相同**（`7b01b33c…`），解析保证可传递
- **三副本 sha256 主控亲自复核一致**：`7b01b33ca90b6d78…`（根 / target/release / Publish 三处），同步时间 14:03:26~27
- **实例复核属实**：PID 23056、`Responding=True`、路径 `target/release/feiyin-ime.exe`、启动 14:03:59（紧接三副本同步之后）
- **✅ 遗留项已由 Gavin 补证闭环（2026-07-28 16:38，决定性证据）**：Gavin `-debug` 重启（PID **5968**，启动 16:37:56 = `08:37:56Z`）并实际录音后，日志给出完整证据链：
  1. `08:38:04.415Z INFO feiyin_ime::scene] Scene rules loaded from "...\target\release\scene-rules.toml"` —— 主控查证 `src/scene/mod.rs:243` 该 `log::info!` **只在 `toml::from_str` 的 `Ok(r)` 分支打印**（`Err` 分支走 `:246` 的 `log::warn!` 并回落内置默认）。**故这一行本身即是「外置新 toml 运行时解析成功」的直接证明**，而该文件 sha256 `7b01b33c…` 正是同步后的 165 条版本
  2. 同一毫秒 `08:38:04.415Z Scene context: app_exe="WindowsTerminal.exe", kind=IDE/terminal, multiline_safe=false, f4_injected=true` —— 分类与 F4 注入全链路正常（惰性初始化的印证：首次 classify 与 rules 加载同刻发生）
  3. `08:38:25.629Z` 第二条 Scene context 同样正常；全日志 `Scene rules parse error` / `Scene builtin rules parse error` 命中数 **0**
  4. 时序自洽：`08:37:36Z` 那条 load 属被重启掉的上一实例（早于 PID 5968 的 `08:37:56Z`），`08:38:04Z` 属当前实例 —— **每进程恰好一次加载**，与 `OnceLock` 语义吻合
  - **结论：144→165 条新词表已在运行时真实生效，本批次运行时验证闭环，无任何遗留。**

- **（历史记录，问题已解决）**验收当时的证据空洞：tester-1 那轮的新实例自 `06:03:59Z` 启动后**从未产生任何 `Scene context:` 行**。主控查证根因：`classify_scene` 全仓唯一调用点在 `main.rs:2962` 的录音流程内，`RULES` 为 `OnceLock` 惰性初始化 → **无录音 = toml 从未被读取**。因此「新实例零 parse error」是**空洞的消极证据**——未尝试解析，何来错误。日志中最后一条 `Scene rules loaded from ...target\release\scene-rules.toml` 停在 `04:47:47Z`（旧实例 PID 18548），新实例无此行。**决定性证据只需 Gavin 触发一次听写**：正常应打出 `app_exe=..., kind=..., f4_injected=true`；若 toml 解析失败会退化为 `kind=unknown, f4_injected=false`（空规则集），一次即可分辨
- **tester-1 诚信记录（正面）**：其 result.md §72 **主动如实披露**了 `Scene context:` 未取得，并自行正确诊断出「lazy OnceLock 且需录音触发」的机制原因，未虚报。这是 [TESTER-FABRICATED-REPORT-001] 四次同模式事故后首次在同类情形下如实交底，予以记录
- **边界合规**：`git status` 无新增源文件改动（仅 IMPL/TEST-SYNC 既有改动）；未出包、未改版本号、未重建 exe；result.md 4566 字节非空
- ~~**已知可接受状态**：exe 内置默认词表仍是旧 144 条、外置 toml 为新 165 条，外置若被删除会静默退回 144 条~~ → **✅ 已于 2026-07-28 18:42 的 BUILD-RELEASE-SCENE-COVERAGE-001 消除**：内置与外置现均为 165 条，外置文件缺失也不再退化

**TEST-SYNC 验收记录（2026-07-28 主控）**：

- **边界完好**：`git diff --numstat src/scene/mod.rs` = **+154 / −0**（纯追加零删除），唯一 hunk 在 `@@ +858,154 @@`，深在 `#[cfg(test)] mod tests`（起于 `:319`）内部；**`scene-rules.toml` 的 numstat 仍是 37/7 与 IMPL 交付时逐字一致，coder-1 的成果未被覆盖**
- **断言方向逐条核对正确**（写反了测试就变成锁死 bug）：P0-3 四条断言的是 `SceneKind::Doc` 而**非** `Browser`——这是验证主控替换 no-op 方案的设计是否成立的关键，方向对了才有意义；P0-1 确实直接用 `toml::from_str::<Rules>(BUILTIN_RULES)` 并把 `parsed.err()` 带进 panic 消息，**没有**退化成吞掉 `Err` 的 `compile_rules_from_content`
- **P0-4 的诚实处理值得记一笔**：任务书要求找一个「只在 browser 块出现」的独有关键词，tester-1 核对后发现 browser 块的 title_keywords 与 email/doc 块**完全重叠、不存在唯一词**，遂按任务书给的备选路径改用内联 fixture 构造（browser 块含独有词 `UniqueBrowserOnlyKeyword`），并在注释里写明为何改用 fixture。**没有硬凑一个假的唯一词来交差**
- **实际交付 14 条 `#[test]`**（汇报写「P0×5+P1×1=6 条」是按**类别**计数，函数数为 14），全文件 scene 单测 46 → **60**
- **主控独立 `cargo check --tests` 0 errors**（81 warnings 全为既有），未采信 tester-1 自验结论
- **一处遗留观察（不阻塞）**：P0-3/P0-5/P1 用的是既有 helper `classify_builtin`（`:329`），它走全局 `rules()`，而 `rules()` 会**优先读 exe 同级的外置 toml**。cargo test 下测试二进制在 `target/debug/deps/`，该目录无 toml 故回落内置默认，结论正确；但若该目录未来出现 toml 副本，这批测试会静默测错文件。属既有 helper 的固有性质（既有 SCENE-AI-AGENT-005~008 同样使用），非本次引入，记录备查。**P0-1 不受影响**（直接解析 `BUILTIN_RULES`，环境无关）

**IMPL 验收记录（2026-07-28 主控独立取证）**：

- **逐条断言核查全过**：`OneNote.exe` 未误加(0) / `ONENOTE.EXE` 仍在(1) / `Figma.exe` 在 browser 块(`:297`) / `ChatGLM.exe`+`GLM.exe` 并存(2) / **browser 块 title_keywords 零改动**（确认没走回 no-op 老路）/ 6 条存疑项全部保留(6) / 新增条目无一条裸写置信度（唯一无标注行是 Skype 注释修改，属 D 项非新增）/ UTF-8 无 BOM（首三字节 `23 20 73`）/ Rust 源文件零改动 / 未碰三副本
- **注释质量超出要求**：`NewMailEngine.exe` 的「火狐邮件」事实错误已改正并写明理由；`wezterm.exe` / `git-bash.exe` 注释如实标注「实际前台窗口通常是已在表内的 `mintty.exe`/`wezterm-gui.exe`，此条为补漏」——没有虚报价值
- **❌ 主控修正一 · coder-1 的基线质疑不成立**：他上报「原文件实际 141 条、任务书 144 有 3 条误差、终值 162」。主控用两种独立方法复核——① `git show HEAD:scene-rules.toml` 取改动前原始文件计数 = **144**；② 严格「首个非空白字符为双引号」行计数，原始 144 / 当前 **165**。git diff 数字自洽：+29 引号行 = 21 新增 + 7 注释重写 + 1 关键词行，−7 = 被重写的 7 行。**141 与 162 两个数字均错，新增数 21 本身无误，正确表述是 144→165。**
- **❌ 主控修正二 · 「cargo test 48 passed ⇒ TOML 语法通过」推理不成立**：现有 scene 单测**全部**用 `compile_rules_from_content` 内联 fixture，无一条读 `BUILTIN_RULES`。测试通过只能证明文件是合法 UTF-8（`include_str!` 编译期校验编码），证明不了 toml 可解析——而解析失败正是本批次最大风险。这与上一轮研究任务的 ✅官方 标注失真属**同一类错误：把不构成证明的东西当成证明**
- **主控过渡验证**（WSL Python 3.10 无 tomllib/toml/tomli，未做真解析）：结构化校验全通过——数组内每行均符合「条目 + 可选尾注释」或纯注释模式（零可疑行）、全文件双引号成对、方括号 11:11 配对、`[[scene]]` 块数仍为 8。**权威解析验证交 TEST-SYNC 的 P0-1 单测 + TEST-EXEC 的运行时 debug.log 核查**

**主控对 title_keywords 的替代设计（派发时补入，非 coder-1 原方案）**：原 10 条作废后，改为只在 **doc 块**加 `Jira / TAPD / 禅道 / Teambition` 4 条——doc 的 kind 非 Browser，浏览器细分循环（`scene/mod.rs:161-177`）会查到它，浏览器开 Jira → 重分类 doc → `multiline_safe=true`，工单描述可得多行结构化输出，与表内既有「腾讯文档 / Google Docs」同一条路径。**排除 `Linear` 关键词**（英文常用词，"linear regression" 会误命中；`Linear.exe` 走 exe 精确匹配即可）、排除微博/小红书/知乎（browser 默认已够）、排除 Salesforce/Zendesk（低频）。

**⚠️ 本批次最大风险（已写入任务书）**：`scene/mod.rs:258-267` 运行时 toml 解析失败只打 `log::warn!` 后**降级空规则集 → 所有场景全变 Unknown**，无用户可见报错。`The Bat!.exe`（含 `!`）与 `Koodo Reader.exe`（含空格）是最易出错的两条。现有 46 条 scene 单测用内联 fixture，**覆盖不到真实 toml** —— 故 TEST-SYNC 必须补一条解析 `BUILTIN_RULES` 的单测把这个洞堵上。

**主控增补建议（省一轮研究）**：SCENE-OBS-001 刚落地的 `Scene context:` 运行时日志会打印 `app_exe`——Gavin 日常使用中打开豆包/Kimi 等桌面版时，**真实进程名会自动出现在 debug.log 里**。与其再派一轮网络查证，不如先落地带 ⚠️ 标注的条目，用实际使用日志做零成本实证收口。


---

## 等 Gavin 拍板

| 事项 | 说明 |
| --- | --- |
| 翻译方向是否改双向全自动 | 现 v1 语义：翻译热键方向由隐藏字段 `translation.target_language` 决定（默认 Chinese），反方向语音自动跳过不翻译；`contains_han` 只做同语种跳过门控。若期望双向全自动需另立项——LLM 路径易改，NLLB 离线路径需按方向换模型（LANG-AUTO-001-CORE 验收时提出，2026-07-14） |
| FORMAT 保底层（A 方案）是否追加 | 不开 LLM 的用户是否需要规则层语气词去除保底（仅"嗯/啊/额"必删项，不碰口头禅，可复用 itn-rules.toml 外置模式）；现方案立场：不开 LLM 不做语义格式化 |
| DEC-028 Qwen3 收尾一项 | `qwen3_asr_url` 默认值维持 dashscope 还是留空强制用户配置（Gavin 实际用工作空间 endpoint，默认值对 MaaS key 用户无效） |
| TELEGRAM-RESTART-001 修复路线 | 见「未排期任务」表 |
| accuracy 体感根因收口（已降级，非阻塞） | 三选一：F1 DEBUG-AUDIO-DUMP-001（`--debug` 存喂模型前音频，小改动）｜F2 Gavin 录 5-10 条日常短指令语料 ｜ 提供当时 accuracy 出错的具体实例（定向复现最快）。**2026-07-25 起进一步降优先级**：accuracy 模型已从 UI 隐藏（ASR-HIDE-ACCURACY-001），用户侧不可达（研究报告 RESEARCH-ASR-ACCURACY-002/003 见 collab/research/） |

---

## 已知遗留问题（待修复）

| 编号 | 问题 | 文件 | 优先级 |
| --- | --- | --- | --- |
| ASR-HALLUC-SEGMENT-001 | accuracy 长音频 VAD 分段中段语义级幻觉（40.1s 语音切 3 段，中段被无关内容整段替换），现有三重兜底（字/秒、重复、空输出）全部拦不住；候选方案：每段用 CTC 交叉转录比对字符重合度 | `src/transcription/mod.rs` | **挂起**（2026-07-25 Gavin 拍板暂不排期：accuracy 已从 UI 隐藏，触发路径不可达。**若未来重开 accuracy 或引入其他 LLM-decoder 类 ASR 模型，必须同批立项**，详见 troubleshooting [ASR-HALLUC-SEGMENT-001]） |
| TEST-FIX-002 | App.test.tsx 缺少 `@tauri-apps/api/core` mock，2 个用例 FAIL | `ui/src/App.test.tsx` | 低 |
| TEST-FIX-003 | Wordbook.test.tsx `getByRole("dialog")` 无 role 属性时报错 | `ui/src/pages/Wordbook.test.tsx` | 低 |
| TECH-DEBT-001 | parse_version 实现不一致：主程序先 split('-') 再 split('.')，Tauri 侧先 split('.') 再 split('-')，prerelease 处理结果有差异 | `src/version_check/mod.rs` + `src-tauri/src/version_check.rs` | 低 |
| ACC-DEGRADE-UI-001 | accuracy 静默降级 performance 时 UI 仍显示 accuracy（可观测性缺口，Gavin 环境不触发；accuracy 已隐藏后影响进一步收窄） | `src/transcription/mod.rs:532-547` | 低 |
| MOJIBAKE-COMMENT-001 | `main.rs` 既有 mojibake 注释（历史编码损伤，非功能影响），归入待清理小项（LANG-AUTO-001-CORE 验收时发现，2026-07-14）。**2026-07-31 macOS 侧主控全文件扫描更正计数：实际 5 处** —— `2483 / 2673 / 2689 / 2722 / 2962`（原记「~L2996 一处」偏少）。已核实为既有（基线 `a38a315` 即存在），且 `0adb819` 批次已顺手清掉另一处同源乱码。清理时须以 UTF-8 无 BOM 读写 | `src/main.rs` | 低 |

---

## 未排期任务

### PLATFORM-001 · macOS 跨平台（Phase 4-6）

| 编号 | 任务 | 前提条件 |
| --- | --- | --- |
| MAC-008 | macOS 构建环境 + CI 配置 | Phase 3 ✅ |
| MAC-009 | 代码签名 + Notarization | Apple Developer |
| MAC-010~013 | E2E/热键/注入/Overlay | MAC-008 |

### 其他待排期

| 编号 | 任务 | 备注 |
| --- | --- | --- |
| PROMPT-CLEANUP-001 | 默认 system_prompt（src/i18n.rs default_system_prompt_en）Rule 4/5（Markdown/List Formatting）与 F1~F4 指令体系职责重叠清理：格式化策略应统一归 F 段管辖，默认 prompt 只留纠错/语义职责。注意 system_prompt 持久化在用户 config，改默认仅对新建配置生效，存量靠 FMT-LLM-002 的 F3 Override 兜底 | 待 Gavin 排期（FMT-LLM-002 审计发现，2026-07-13） |
| WORDBOOK-CORRECTION-UI-001 | 注入后 overlay 纠错入口（Gavin 2026-07-06 选定方案 2）：识别注入完成后 overlay 短暂显示「纠错」小按钮，点击弹出编辑框改文本，确认后调 wordbook.learn_correction 入库（喂 hotwords）。背景：WM_GETTEXT 读回在现代应用无效（RESEARCH-TEXTCAPTURE-001），自动学习路径 A 实际失效，需自建纠错闭环。涉及：src/main.rs（overlay 生命周期）+ Win32 overlay 绘制 + wordbook API（已有）。注意：DEC-029 词库已单词化，此任务未来改为学单词 | 待 Gavin 排期 |
| UI-I18N-COMPLETE-001 | UI 硬编码文字补全 i18n（coder-2 2026-07-07 建议）：App.tsx Loading/Error、Llm.tsx Success/Failed 等 5 处状态文字补 key + 替换；热键显示名（VK_TO_LABEL 的 Ctrl/F9）属通用键名评估后大概率不译 | 待 Gavin 排期（归入"小优化"批次） |
| LLM-KEY-REVEAL-001 | 格式化输出页 API Key 明文查看（显示/隐藏小按钮）：现为 password 掩码，Gavin 端测反馈框太小看不清已由 FORMAT-UI-POLISH-001 解决，明文查看为当时保留的备选追加项 | 待 Gavin 排期（归入"小优化"批次，2026-07-14 提出） |
| TELEGRAM-RESTART-001 | Telegram 通道恢复（troubleshooting [TELEGRAM-CHANNEL-001]）：2026-07-07 实测重启无效，根因为 Claude Code 2.1.202 服务端功能开关（tengu_harbor）拦截，本地无解；候选路线：降级 CLI / 等官方放开 / 临时手动轮询 | 待 Gavin 拍板路线 |
| MAC-P2-001 | macOS 热键 dispatch 延迟优化（P2） | Windows P2 完成后评估 |
| RESEARCH-TEXTCAPTURE-001 | 现代应用文本捕获方案研究 | WM_GETTEXT 在现代应用无效 |
| DEC-014 | WebView2 自动安装 | Win10 用户 |
| CRASH-EMAIL-001 | crash reporter 邮箱设置 | 需 SMTP 配置 |
| QWEN3-CORPUS-BIAS-001 | qwen3 在线模型接入词库偏置：官方 input_audio_transcription.corpus.text（max 10K tokens）可作 hotwords 等价通道；注意两份官方文档记载矛盾，实施前需实测验证。注意：DEC-029 词库已单词化，corpus 直接用单词列表 | 待 Gavin 排期（研究报告 collab/research/qwen3-protocol-alignment-001.md）|
| QWEN3-STREAM-V2 | qwen3 真流式边录边上屏（DEC-028 v1 为整段上传，流式留作演进） | 待 Gavin 排期 |

---

## Phase 3 · 演进评估（不排期，仅记录）

场景感知后续演进方向：UIA 控件信号、浏览器细分词表迭代、内容压缩独立开关、语气适配 LLM 兜底开关（未命中时参考应用名，默认关）、个性化风格学习。

来源：DESIGN-FORMAT-SCENE-001 技术方案（collab/research/typeless-format-design-001.md）+ DEC-031。


---

## 📋 待排期 · I18N-HANT-GAP-001 繁体中文缺 `voice_asr_model_accuracy` 一个 key（2026-08-17 主控验收 HOTKEY-047 时机器发现）

**发现方式**：验收 HOTKEY-047 时主控用 `comm` 比对三份 locale 的 key 集合（不是采信 Worker 自证）。

| 文件 | key 数（`^  [a-z_0-9]*:` 计） |
| --- | --- |
| `ui/src/i18n/en.ts` | 111 |
| `ui/src/i18n/zh-Hans.ts` | 111 |
| `ui/src/i18n/zh-Hant.ts` | **110** ← 缺 `voice_asr_model_accuracy` |

**已确认是历史遗留**：`git show HEAD:ui/src/i18n/zh-Hant.ts` 计得 110，`en.ts` 计得 111
→ **不是 HOTKEY-047 引入**，coder-1 无责。

**影响**：繁体中文用户在语音设置页看到 ASR 模型「准确度」相关文案时会拿到 `undefined`
或回退（取决于 `getTranslations` 的兜底策略，**未核，实施前需先确认**）。

**修法**：给 `zh-Hant.ts` 补该 key 的繁体译文。改动极小，但**必须顺带核一件事** ——
`ui/src/i18n/index.ts` 的 `getTranslations` 对缺失 key 是否有兜底；
若无兜底则属于「繁中用户看到 undefined」的用户可见缺陷，优先级要上调。

**顺带的方法教训**（已记 `[WORKER-WRITE-SILENT-FAIL-001]` 同族）：
coder-1 自证里报「三份 locale = 111:111:111 一致」，等式不成立 ——
**报数字前要真的量一次**。主控验收一律独立复算，不采信自证里的数字。

---

## 🛑 2026-08-17 停派指令（Gavin）—— 恢复时从这里开始

> **Gavin 原话**：「你的额度快尽了，先别派发任务，等我指令再派」

| 项 | 状态 |
| --- | --- |
| HEAD | `2e70875`，**已 push**（`56bfa37..2e70875`，10 个提交），token 无残留 |
| 工作区 | clean（仅 `collab/todo.md.bak-20260817` 未跟踪备份，可留） |
| BUILD-019 产物 | `Publish/` 08-17 16:18，三 exe sha 相对 BUILD-018 全变，⏭ **待 Gavin 端测** |
| coder-1 / coder-2 / tester-1 | **全部空闲待命，手上零任务** |
| 🔴 恢复后第一个动作 | 派 **HOTKEY-049**（Gavin 已定顺序「先 049」），方案与判据已固化在上方专节，**可直接派，无需再问** |
| 其后 | 阶段三 TEST-SYNC-049 → 阶段四 → BUILD-020 → push（push 授权已给，出包后执行） |
| 再其后 | `E2E-HARNESS-050`（修两道 harness 错位，裁决已定：改 harness 不改生产、尺寸与 `main.rs` 常量同源） |

### 恢复时不必重新调研的（已定论，直接用）

- HOTKEY-049 判据、键集合推导、7 条实例对照表 → 见上方 049 专节
- 049 的 UI 红线：**不得复用带「仍然使用」的现有冲突弹窗**，须另做只有「重新设置」出口的提示
- 049 双向生效：设翻译键查语音键、设语音键查翻译键，两侧都要做
- 049 范围外：后端不做强制（手改 config.toml 不受保护），已如实记录
- E2E 门禁结论：产品正常，两道 harness 错位自初始提交即存在 → `[E2E-CONFIG-PATH-STALE-001]`

### Gavin 端测五项（BUILD-019）

1. 按热键录音窗口出不出来（本包核心）
2. 点一次「设置热键」就能录上（不用点两次）
3. 左 Alt / 左 Ctrl / 左 Shift 可单独设，且按下即回显键名
4. 按住右 Alt 再按 M → 应得 `Alt+M` 而非 `Ctrl+Alt+M`
5. 🔴 **只有端测能验**：真按左 Ctrl+左 Alt+字母 → 应得 `Ctrl+Alt+字母`，不得被吞成 `Alt+字母`
   （happy-dom 把 `AltGraph` 与 `alt` 同映射，单测证明不了，见
   `[HAPPYDOM-ALTGR-INDISTINGUISHABLE-001]`）

---

## 🛑 2026-08-18 01:4x 收工交接 —— 明天从这里开始

### 状态快照

| 项 | 值 |
| --- | --- |
| HEAD | `8fd9767`（E2E-HARNESS-050） |
| 已 push | 到 `2e70875`；之后 **未 push 2 个**（`0049c33` / `8fd9767`）—— Gavin 未授权，勿自动 push |
| 🔴 工作区**非 clean** | coder-1 的 **051-G 在途改动未提交**：`src/main.rs` +270 / `src/transcription/qwen_inference.rs` +45 |
| 在途备份 | `scratchpad/051G-wip-0818-0141.patch`（22KB，已存，丢不了） |
| coder-1 / coder-2 | ⏸ **token 额度耗尽，Gavin 已令暂停派发**，等重置 |
| tester-1 | ✅ 空闲，可用（另一额度池） |
| 产物 | `Publish/` 仍是 **BUILD-020**（08-17 19:37），**不含**今天 054-A/055/053-C/D/E2E 的任何改动 |

### 🔴 明天第一件事：确认 coder-1 的 051-G 在途代码状态

它被额度打断在半途。**先 `cargo check --all-targets` 确认能否编译**
（今天两条 `test_platform_cargo_test_*` E2E 用例失败就是因为它编译不过）。
- 能编译 → 让 coder-1 继续收尾
- 不能编译 → 让 coder-1 先修复编译再继续，**不要 revert**（备份在，但优先让它自己收尾）

---

## 卡顿问题：调查已完结，方案已定，等一次验证

### 已排除（全部有实测/文档依据，**不要重查**）

| 假设 | 结论 |
| --- | --- |
| 建连慢 | ❌ DNS 10ms + TCP 0ms + TLS 91ms = **103ms** |
| 首字慢 | ❌ 开口→首字 **338ms**，「反应快」已达标 |
| 我们攒着音频不发 | ❌ 实时逐帧发，100ms/帧，**与官方推荐完全一致**，无节流 |
| debug 日志 IO 拖累 | ❌ 正式模式同样卡；且正式模式 `LevelFilter::Warn` 无文件目标，零磁盘 IO |
| 本地 VAD 门控导致 | ❌ 仅入口门控，建连后**不再过滤**（主循环 VAD 调用数 = 0） |
| API 有参数可调推送频率 | ❌ **官方文档确认无此参数**（全部 9 个参数均不控制此项） |
| Manual 模式高频 commit | ❌ 它控的是**断句**不是中间结果频率；每 300ms 断句会**丢上下文、毁准确率** |
| `words[]` 带来更高频推送 | 🟡 大概率不成立（words 是同一条消息内对同一 text 的分解），**待一次 -debug 实证** |

### 确认的根因

**服务端每约 1000ms 推一批、每批 3-4 字**（实测：786/1036/1105/1246ms 间隔）。
这是它的固有行为，客户端调不动。

### 已定方案（Gavin 亲自拍板，主控曾三次判断错误后被纠正）

**时间戳驱动回放** —— 用 `words[].begin_time` 按用户**真实语速**回放。

🔴 **Gavin 的三条明确指示（不得再动摇）**：
1. 「**时间戳驱动才是应该选方案**」→ 不用固定速率抖动缓冲
2. 「**停顿和用户讲话的节奏一致才对**」→ **不得压缩停顿**，删掉主控原提的间隔上限
3. 几百毫秒固定延迟**可以接受**

`words` 不可用时 → **退回现在的立即显示**，不要换另一套平滑算法。

### 🔴 实施必须注意的三个坑（主控今天查出，写进任务书了）

1. **时间戳基准**：我们是攒 1.5s pre-roll 才一次性补发的，
   `begin_time` 相对「发出去的音频流起点」，**不是**「按下热键的时刻」。搞混会整体偏移 1.5 秒
2. **断句会重置**：服务端 `max_sentence_silence = 800ms`，
   **停顿超 0.8 秒即断句、开新 `sentence_id`** → 回放游标必须跟着重置
3. 官方文档新增可用信息：`words[]` 还有 **`end_time`**（每词该显示多久）与
   **`punctuation`**（标点归属），都该用上

### 待验证（一次 -debug 即可，不需额外开发）

coder-1 已把 `words={}` 加进日志。**新包出来后 Gavin 跑一次 `-debug` 说两三句话**：
- 空 text 消息的 `words=N` 是 0 还是有值
- 每批实际几个词、时间戳跨度多大 → **这是回放偏移量取值的依据**

---

## 今天完成并已提交

| 提交 | 内容 |
| --- | --- |
| `0049c33` | OVERLAY-054-A 焦点恢复（RestoreAndHide + SetForegroundWindow + 200ms 轮询确认）／ ASR-055 测试连接重写（原传空串 → Inference 协议，model 入 payload）／ WORDBOOK-053-C 候选词七条校验 ／ 053-D 污染排查 |
| `8fd9767` | **E2E-HARNESS-050**：9 FAIL → **2 FAIL / 61 PASS**；配置路径单一来源；state_detector **正则解析 main.rs 常量**（零硬编码，今后改尺寸免疫）；新发现 `[E2E-COLD-START-RACE-001]` |

### 🔴 053-D 结果待 Gavin 授权（主控与 Worker 均未删任何数据）

`db_path()` = exe 同级 `wordbook.sqlite`
- `wordbook` 表 18 条 **全部正常**（污染未进正式词库）
- `candidates` 表 164 条中 **12 条脏数据**（多行编号列表，24–65 字符）

**清不清由 Gavin 定。**

---

## 待办队列（按序）

| 序 | 项 | 归属 | 前置 |
| --- | --- | --- | --- |
| 1 | **051-G 收尾**（时间戳回放 + 三个坑） | coder-1 | 额度 |
| 2 | 出诊断包 → Gavin `-debug` 一次 → 定回放参数 | tester-1 + Gavin | 1 |
| 3 | **054-B 打回**：`:3117` `RecordingStreamingIdle` 仍传 `pos:[0,0]`，**在线路径仍闪左上角**（coder-2 上轮只改了本地模型路径） | coder-2 | 额度 |
| 4 | 054-C 边框（🔴 `BORDER_GRAY` 实测 **7 处**全为近黑 `0x060607`，非主控此前所说 2 处）／ 054-D 抗锯齿（`WM_SETFONT` 全库 0 次 + GDI `RoundRect` 无抗锯齿）／ 054-E 字体大一号 | coder-2 | 额度 |
| 5 | ASR-055 端测（需有效 api_key，Worker 无法真连） | Gavin | 出包 |
| 6 | `ASR-PERF-052` `multi_threshold_mode_enabled` 实验 | 待定 | 低优先 |

### 🔴 主控给自己加的新硬约束

**出包前 E2E 门禁必须通过，不过不出包。** 不再以「读代码觉得没问题」代替实际运行验证。
—— 起因：Gavin 批评「几轮拿不到主流程走通的版本」「验收潦草」，
根因是主控验收上限止于 `cargo check` + 读代码，从不真正跑程序。

### 其他挂账

- **api_key 明文存 `config.toml`**（`Publish/` 与 `target/release/` 均有），Gavin 未决定是否处理
- `main.rs:4723`、`hotkey.rs:300` 等处中文注释**乱码**（`[ENCODING-UTF8-001]` 历史遗留，范围不止一个文件）
- `src/main - 副本.rs` 99KB 未入 git 的旧副本残留，**删除需 Gavin 单独确认**
