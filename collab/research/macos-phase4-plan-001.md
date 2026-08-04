# macOS 侧 Phase 4 管线实现 · 任务规划（MACOS-P4-PLAN-001）

> 出具方：macOS 侧 Orchestrator ｜ 日期：2026-08-04 ｜ 基线：`0adb819`（v0.7.3，与 `origin/main` 同步）
> 依据：Gavin 2026-08-04 指令「把 macOS 侧管线 Phase 4 的任务规划出来」
> 治理约束：DEC-033（含附则一/二/三）｜ DEC-015（事件宿主，**本文提出复议**）｜ DEC-017 / DEC-018（已落地）
> **状态：阶段 A 已于 2026-08-04 派发 tester-1；阶段 B 及之后仍待 §4 拍板。**

## 【总纲】Gavin 2026-08-04 三条口径（凌驾于本文其余内容）

1. **不得破坏 Windows 侧任何已有代码的功能。** —— Gavin 当日**第三次重申**（前两次见 DEC-033 第 4 条）。Windows 侧功能较领先，**是基准，不是改造对象**。
2. **macOS 侧的功能实现要参考 Windows 侧，做到尽量一致。** —— 行为对齐、口径对齐。新写 macOS 实现前，**先读 Windows 侧对应实现**，把它当规格说明书；偏离必须有理由并记入 decisions。
3. **后段能复用既有代码的就复用**，缺的是 **macOS 前端的录音处理链路**。

**这三条把 Phase 4 的范围收敛为**：

| 层 | 归属 | 做法 |
| --- | --- | --- |
| **后段**：ASR / LLM / ITN / 翻译 / 标点 / 词库 / 场景分类 | 已平台中立（13 个模块 `cfg` 计数为 0） | **复用，不重写**。阶段 B 只做「让它在 macOS 上可达」 |
| **前端**：热键 / 录音 / 事件宿主 / 托盘 / 浮层 / 注入 / 场景采集 | macOS 侧本轮交付物 | **参照 Windows 侧行为实现**（阶段 C / D / E） |
| **Windows 侧任何已交付路径** | 只读 | **不动**。阶段 B 的委托封装设计即为此服务 |

---

## §0 本规划的事实基线（全部为主控本次实测，非引用文档）

遵守 `[DOC-STATE-DRIFT-001]`：以下每一条都由本次 `grep` / `Read` 取得，不采信既有文档结论。

| 项 | 实测值 | 取证方式 |
| --- | --- | --- |
| `src/` 总行数 | 25,592 | `wc -l` |
| `src/main.rs` | 4,437 行，`cfg(target_os="windows")` **77 处**、`macos` **1 处**、`not(windows)` **1 处** | `grep -c` |
| `fn main()` 非 Windows 分支 | `main.rs:2738-2741`，只 `log::warn!` 后返回 `Ok(())` | Read |
| `run_pipeline` | `main.rs:2812-3214`（**402 行**），整函数 `#[cfg(target_os="windows")]` | Read |
| `spawn_worker_thread` | `main.rs:2161-2459`（**299 行**），整函数 `#[cfg(windows)]` | Read |
| `run_controller` | `main.rs:2460-2641`（**182 行**），Win32 消息循环 | Read |
| Win32 overlay 绘制 | `main.rs` 的 `draw_text:302` / `spawn_overlay_thread:555` / `draw_overlay_to_dc:973` / `draw_recording_overlay:1049` / `draw_processing_overlay:1338` / `draw_preview_overlay:1465` / `draw_error_overlay:1656`，合计约 **1,400 行 GDI** | grep |
| `mod macos_stubs` | `main.rs:3431-3486`，**全仓无任何引用**（`grep macos_stubs` 只命中声明行本身）→ 当前是死代码脚手架 | grep |

### 平台中立性实测（`grep -c target_os`，0 = 完全中立）

| 模块 | 命中 | 判定 |
| --- | --- | --- |
| `transcription/mod.rs` / `llm/mod.rs` / `itn.rs` / `config/mod.rs` / `translation/mod.rs` / `punctuation/mod.rs` / `scene/mod.rs` / `wordbook/db.rs` / `version_check/mod.rs` / `ui/overlay.rs` / `ui/tray.rs` / `injection/mod.rs` / `hotkey/mod.rs` | **全部 0** | ✅ 双侧可直接复用 |
| `audio/mod.rs` | 7 | ⚠️ 但见下——实为 cpal 跨平台 + 单点降级 |
| `crash/mod.rs` | 3 | 已有非 Windows 占位 |

### macOS 平台层现状（`src/platform/macos/`，共 712 行）

| 符号 | 状态 | 位置 |
| --- | --- | --- |
| `HotkeyListener` / `HotkeyEvent` | ✅ **真实现**（CGEventTap Session + CFRunLoop，DEC-017） | `hotkey.rs`（448 行） |
| `inject_text` / `copy_text_to_clipboard` | ✅ **真实现**（pbcopy/pbpaste + enigo Cmd+V，DEC-018） | `injection.rs:19/37` |
| `capture_focused_text_snapshot` | ❌ 恒返 `None` | `injection.rs:41` |
| `read_text_from_hwnd` | ❌ 恒返 `None` | `injection.rs:45` |
| `capture_scene_signals` | ❌ 恒返 `None` + warn | `mod.rs:79` |
| `enable` / `disable` / `is_enabled`（自启动） | ❌ 恒返 `Err` / `false` | `mod.rs:30-40` |
| `create_controller_window` / `run_message_loop` / `destroy_controller_window` | ❌ 只 warn 后 `Ok(())` | `mod.rs:44-58` |
| `notify_config_changed` | ❌ 只 warn | `mod.rs:66` |
| `is_accessibility_granted` | ✅ 真实现（`AXIsProcessTrusted` FFI） | `accessibility.rs` |
| `request_accessibility_permission` | ❌ **假实现**——内部 `ax_is_process_trusted_with_prompt()` 只打一行 log，`AXIsProcessTrustedWithOptions` 从未调用 | `accessibility.rs` 末尾 |

---

## §1 核心发现：管线 95% 是平台中立的，只是被整体 cfg 门控掉了

**这是本规划最重要的一条，直接决定任务分解方式。**

`run_pipeline`（402 行）承载了 ASR 前处理 → 转录 → 场景采集 → 翻译/优化三分支 → ITN → 标点 → 注入 → 词库学习的**全部业务逻辑**。逐行核查后，其中**只有 4 个平台接触点**：

| # | 位置 | 内容 | macOS 侧现状 |
| --- | --- | --- | --- |
| 1 | `:2823` | 函数签名 `target_hwnd: HWND` | 类型不存在 |
| 2 | `:2939` | `platform::capture_scene_signals(target_hwnd)` | stub 已在，签名 `usize`，形状一致 |
| 3 | `:3178-3179` | `GetForegroundWindow()` + `focus_lost` 判定 | **无对应实现**（平台契约里没有这个符号） |
| 4 | `:3192-3193` | `capture_focused_text_snapshot()` + `inject_text()` | inject 已真实现；snapshot 恒 None |

`spawn_worker_thread`（299 行）更极端——**只有 1 个平台接触点**：`:2450` 的 `HWND(start.target_hwnd.0 as *mut c_void)`，纯粹是为了把值传给 `run_pipeline`。

**推论**：只要把 `HWND` 换成不透明的窗口标识（`usize`），并给 `GetForegroundWindow` 补一个平台契约符号，**约 700 行核心管线可以一次性从 Windows 专属变为双侧共享**，macOS 侧不需要重写任何业务逻辑。

**同时这也修正了一处长期以来的表述偏差**：`docs/MACOS-HANDOFF.md` §2.8 与 DEC-035 都写「`run_pipeline` 整个函数在 `#[cfg(windows)]` 内，代码层无法共享，DEC-035 的顺序约束是纯文档契约」。前半句是事实，但**「无法共享」是当前状态而非固有属性**——它可共享，只是还没做。DEC-035 顺序契约在本阶段 B 完成后即可从「文档契约」升级为「代码层强制」，`[ITN 顺序反转]` 这类缺陷不会在 macOS 重演。

**`audio/mod.rs` 的 7 处 cfg 不构成阻塞**：录音走 `cpal`（跨平台），7 处 cfg 中 4 处是 WASAPI import、1 处是 `is_mic_muted()` 的 Windows 实现，`:864` 已有 `#[cfg(not(target_os="windows"))] { false }` 降级分支。**macOS 上录音链路在代码层已经通了**，未验证的是运行时的 TCC 麦克风授权（见阶段 A）。

---

## §2 任务分解

### 阶段 A · 前置取证（**必须最先，串行，不可与任何编码任务并行**）

| 编号 | 负责人 | 性质 |
| --- | --- | --- |
| **MACOS-P4-PROBE-001** | tester-1 | 可行性探针，**允许创建临时文件但必须删除、不得入库** |

**派发动因**：阶段 B/C 的全部工作量估算都建立在「录音能录、模型能跑、热键能收」这三个假设上，而这三条**在 macOS 实机上从未验证过**。07-31 的 `MACOS-TESTEXEC-V073-001` 证明的是「编译过 + 单测过」，**单测不碰麦克风、不加载模型、不注册事件 tap**。若 A1 或 A2 失败，阶段 B/C 的排期全部作废。

| 探针 | 内容 | 手段 | 失败的含义 |
| --- | --- | --- | --- |
| A1 | cpal 能否枚举并实录 macOS 麦克风 | 临时探针 bin（用完即删），调 `audio::` 既有 API 录 3 秒并打印 RMS | 录音链路需重写 → 阶段 C 工作量翻倍 |
| A2 | sherpa-onnx Paraformer 能否在 arm64 加载并转录 | **零改动**——直接跑既有 `poc_funasr_nano` bin 喂一个 wav | ASR 不可用 → 整个 Phase 4 阻塞，需先解 sherpa-onnx dylib 问题 |
| A3 | 终端启动 vs `.app` 启动的 TCC 差异 | 同一探针分别从 Terminal 和最小 `.app` bundle 启动 | 决定 `.app` 打包是否是**前置**而非收尾任务 |
| A4 | CGEventTap 是否真能收到全局按键 | 临时探针 bin 起 `HotkeyListener` 并打印事件 | DEC-017 的实现从未实机验证过；失败则热键需换路线 |
| A5 | `AXIsProcessTrustedWithOptions` 系统弹窗 | 手工触发，观察是否出现系统「辅助功能」授权对话框 | 确认 `accessibility.rs` 的假实现影响面 |

**验收标准**：① 五项各给 PASS/FAIL + 原始输出（不接受只给结论）② A2 必须贴出转录文本与耗时 ③ 明确回答「从终端跑和从 .app 跑，麦克风与辅助功能权限行为是否不同」④ `git status --porcelain` 与开工时一致，临时探针文件已删除 ⑤ **不得修改任何源文件**。

---

### 阶段 B · 管线去平台化（**最高杠杆，但归属待 Gavin 拍板——见 §4-1**）

| 编号 | 负责人 | 影响文件 |
| --- | --- | --- |
| **MACOS-P4-NEUTRAL-001** | coder-1（若归本侧） | `src/main.rs` + `src/platform/mod.rs` + `src/platform/{windows,macos}/mod.rs` |

**具体改动（主控方案 v2，2026-08-04 按 Gavin「不能破坏 Windows 任何代码功能」重申后改为最小触碰版）**：

> **v1 方案已废弃**。v1 直接把 `run_pipeline` 的签名从 `HWND` 改为 `WindowId` 并去掉 cfg，这会**同时改动 Windows 侧的函数签名与调用点**。v2 改用**委托薄封装**，使 **Windows 侧可见的签名与调用点一字不改**。

1. **`platform/mod.rs` 定义 `pub type WindowId = usize;`**（不是新 struct，避免动 `injection/mod.rs` 既有类型）。
2. **`run_pipeline` 保持 `#[cfg(target_os="windows")]` 与 `target_hwnd: HWND` 签名不变**，函数体缩为一行委托：
   ```rust
   #[cfg(target_os = "windows")]
   fn run_pipeline(/* 参数列表逐字不变，含 target_hwnd: HWND */) {
       run_pipeline_core(/* … */, target_hwnd.0 as usize, /* … */)
   }
   ```
   → **`spawn_worker_thread:2450` 的调用点零改动**，Windows 侧类型链完全不动。
3. **新增平台中立的 `run_pipeline_core(…, target_hwnd: WindowId, …)`**，**函数体从原 `run_pipeline` 整段搬入、逐字不改**，仅调整 4 个平台接触点：
   - `:2939` `platform::capture_scene_signals(target_hwnd)` —— 两侧签名本已是 `(usize)` 形状，Windows 侧补一层 `HWND(x as *mut c_void)` 薄转换
   - `:3178` `GetForegroundWindow()` 抽为**新平台契约符号** `platform::foreground_window_id() -> WindowId`。Windows 侧实现 = `GetForegroundWindow().0 as usize`（**行为逐字等价**）；macOS 侧第一版可返 0 表示「无法判定焦点」→ `focus_lost` 恒 false，语义与 Windows 侧 `target_hwnd` 为空时一致
   - `:3192-3193` 无需改动（两侧签名本已一致）
4. **`spawn_worker_thread` 同法**：Windows 侧保留原函数不动，抽出中立 `spawn_worker_thread_core`。
5. **`platform/mod.rs` 两份导出清单同步新增 `foreground_window_id`**（DEC-033 附则三第 4 条硬性要求）。
6. **`mod macos_stubs` 本轮不动**（当前零引用，删它没有收益却增加 diff 面）。

**v2 相对 v1 的 Windows 风险差异**：

| | v1（已废弃） | v2（采纳） |
| --- | --- | --- |
| Windows 侧 `run_pipeline` 签名 | 改 | **不改** |
| Windows 侧调用点 `:2450` | 改 | **不改** |
| `StartCmd` / `SendHwnd` 类型 | 改 | **不改** |
| Windows 编译面新增风险 | 签名 + 调用点 + 类型三处 | **仅 `foreground_window_id` 一个新符号** |

**🔴 本任务的最高红线**：**Windows 侧零行为改动**（DEC-033 第 4 条，Gavin 2026-08-04 已第三次重申）。这是一次**纯搬运重构**，不得夹带任何逻辑修改。

**为什么这个风险是可控的**：本任务的所有改动都是**类型与函数边界层面**的，Rust 会把任何错误变成**响亮的编译失败**而非静默行为漂移。唯一需要人工确认语义的只有 `foreground_window_id()` 在 Windows 上是否与原 `GetForegroundWindow()` 逐字等价——**这是 2 行代码，可 review 到底**。

**验收标准**：
- ① Windows 侧行为等价性**必须以 token 级证据自证**：`run_pipeline` 函数体去空白后除签名行与 4 个接触点外**逐字节不变**（参照 `[FMT-COLLATERAL-001]` 三步法）
- ② macOS `cargo check --all-targets` 0 errors
- ③ macOS `cargo test --no-fail-fast` 相对 07-31 基线（701/1/8）**passed 数只增不减**，failed 数仍为 1（`time_half`）
- ④ `platform/mod.rs` 两份清单符号数相等且逐一对应
- ⑤ **Windows 侧零回归验证由谁做，取决于 §4-1 的拍板结果**

| 编号 | 负责人 | 说明 |
| --- | --- | --- |
| **TEST-SYNC-P4-NEUTRAL-001** | tester-1 | 阶段三（NEUTRAL 完成后串行）：为新符号 `foreground_window_id` 补两侧契约单测；为 `run_pipeline` 新增的 macOS 可达性补最小烟测 |

---

### 阶段 C · macOS 事件宿主（核心新代码，**依赖阶段 A 结论 + §4-2 选型拍板**）

| 编号 | 负责人 | 影响文件 |
| --- | --- | --- |
| **MACOS-P4-HOST-001** | coder-2 | `src/platform/macos/event_loop.rs`（**新建**）+ `src/platform/macos/mod.rs` + `src/main.rs` 的 `fn main()` |
| **MACOS-P4-TRAY-001** | coder-1 | `src/platform/macos/tray.rs`（**新建**）+ `src/ui/tray.rs` |

**HOST-001 具体改动**：
1. 新建 `event_loop.rs`，实现 macOS 版 `create_controller_window` / `run_message_loop` / `destroy_controller_window`，**保持既有平台契约的 arity**（`create` 无参返 `Result<()>`）
2. 事件宿主承担 Win32 侧 `run_controller` 的等价职责：驱动 `process_controller_events` 的定时轮询（Win32 是 15ms `SetTimer`）、接收热键线程唤醒、分发 `PipelineEvent`
3. `fn main()` 的 `#[cfg(not(target_os="windows"))]` 分支从「打 warn 后返回」改为调用 macOS 版 controller
4. **单实例检查**：Windows 用命名 Mutex（`main.rs:2706-2735`），macOS 需换实现（建议 `flock` 锁文件于 `~/Library/Application Support/`，不用 `NSRunningApplication` 枚举——后者在未签名场景不可靠）

**⚠️ HOST-001 的固有难点（任务书须明示，不要让 Worker 自己撞）**：macOS 上 **NSApplication 必须跑在主线程**，且 `tray-icon` 与任何 AppKit UI 都依赖它。而当前 `run_controller` 在主线程跑消息循环、worker/overlay 在子线程——**这个结构 macOS 可以沿用**，但 `run_message_loop` 内部必须是 AppKit 的 run loop，不能是自旋。

**TRAY-001 具体改动**：`tray-icon 0.19` 官方支持 macOS，但**菜单交互路径与 Windows 完全不同**——Windows 侧走的是 `TrackPopupMenu`（`main.rs:224`，为绕开 BUG-027 托盘菜单冻结而刻意用的 Win32 直调），macOS 侧应使用 `tray-icon` 自带的 `Menu`。**不要试图把 Windows 的 TrackPopupMenu 路径抽象化**——那是 Windows 专属 workaround，抽象它会违反红线。

---

### 阶段 D · 平台能力补全（**四项文件级零重叠，可并行**）

| 编号 | 负责人建议 | 影响文件 | 内容 |
| --- | --- | --- | --- |
| **MACOS-P4-SCENE-001** | coder-1 | `src/platform/macos/scene.rs`（新建）+ `macos/mod.rs` 的 re-export | `capture_scene_signals` 真实现：`NSWorkspace.sharedWorkspace.frontmostApplication` 取 bundle 可执行名 + AXUIElement 取窗口标题。**产出的 exe 名必须与 `scene-rules.toml` 词表口径对齐**——现有 165 条全是 Windows `.exe` 名，macOS 侧需新增 macOS 应用名段（这是一条**独立的数据任务**，见下方"未纳入本轮"） |
| **MACOS-P4-PERM-001** | coder-2 | `src/platform/macos/accessibility.rs` | 补全 `AXIsProcessTrustedWithOptions` 真调用（需 CFDictionary 绑定，`core-foundation 0.10` 已在依赖内）；启动时的授权引导对话框 |
| **MACOS-P4-AUTOLAUNCH-001** | 待定 | `src/platform/macos/autolaunch.rs`（新建）+ `macos/mod.rs` | `enable`/`disable`/`is_enabled` 真实现。建议 `SMAppService`（macOS 13+）而非 LaunchAgent plist；需确认最低支持版本 |
| **MACOS-P4-READBACK-001** | 待定 | `src/platform/macos/injection.rs` | `capture_focused_text_snapshot` / `read_text_from_hwnd` 真实现（AXUIElement `kAXValueAttribute`）。**这一项 macOS 反而比 Windows 有优势**——Windows 侧 `WM_GETTEXT` 在现代应用已失效（`RESEARCH-TEXTCAPTURE-001`），词库自动学习路径 A 实际是瘫的；macOS 的 AX API 可以真正读回，见 §4-5 |

---

### 阶段 E · 交付形态（依赖阶段 A3 结论 + §4-4 拍板）

| 编号 | 影响文件 | 内容 |
| --- | --- | --- |
| **MACOS-P4-OVERLAY-001** | `src/platform/macos/overlay.rs`（新建）+ `main.rs` 的 macos_stubs 收尾 | 录音浮层。`Cargo.toml:115-116` 已预留注释掉的 `objc2` / `cocoa` 依赖并标注 "Phase 4 NSWindow transparent overlay"，与本任务对应。**建议不进第一版**，见 §4-3 |
| **MACOS-P4-BUNDLE-001** | `scripts/build-macos.sh` + 新建 `Info.plist` | `.app` bundle 打包 + `NSMicrophoneUsageDescription` 等 TCC 声明 + 签名 / 公证。**范围取决于 §4-4** |

---

## §3 边界评估（派发前必做，DEC-033 + worker-guide §二）

**同批次可并行的组合（文件级零重叠，已逐一核对）**：

| 批次 | 可并行任务 | 重叠检查 |
| --- | --- | --- |
| 阶段 A | 单任务 | — |
| 阶段 B | **单任务独占 `src/main.rs`** | ⚠️ `main.rs` 是全仓最大冲突面，**阶段 B 期间禁止任何其他任务碰它** |
| 阶段 C | HOST-001 与 TRAY-001 **不可并行** | ❌ 两者都要改 `main.rs` / `macos/mod.rs`，且 TRAY 依赖 HOST 的 run loop 就位 → **串行** |
| 阶段 D | SCENE-001 ∥ PERM-001 ∥ AUTOLAUNCH-001 **可三路并行** | ✅ 分别只碰 `macos/scene.rs`(新) / `macos/accessibility.rs` / `macos/autolaunch.rs`(新)。**唯一交汇点是 `macos/mod.rs` 的 re-export 行** → 由主控在三者完成后**统一改一次**，Worker 一律不碰该文件 |
| 阶段 D | READBACK-001 独立 | ✅ 只碰 `macos/injection.rs` |

**测试五阶段纪律（worker-guide §二，禁止并行）**：每个编码任务完成 → 主控验收 → 才 dispatch TEST-SYNC → 完成后才 dispatch 测试执行。**阶段 B 尤其不得跳过**。

---

## §4 待 Gavin 拍板（5 项，全部阻塞派发）

### 4-1 · 阶段 B 的归属与 Windows 零回归验证方（**最关键**）

阶段 B 改的是 `src/main.rs` 中**Windows 已交付、已出包的核心路径**。按 DEC-033 第 3 条，「接缝（平台契约）」归 Windows 侧；按第 4 条，Windows 零行为改动是硬红线。而**本侧没有 Windows 机器，无法验证零回归**。三个选项：

| 选项 | 做法 | 代价 |
| --- | --- | --- |
| **A（推荐）** | macOS 侧实施 + token 级等价自证，**推送前交 Windows 侧跑一遍全量回归再合并** | 需一次跨团队协同往返 |
| B | 整体移交 Windows 侧实施 | 本侧进度受对方排期牵制；且改动动因完全来自 macOS |
| C | macOS 侧另写一份 macOS 专属管线，不动 `main.rs` | ❌ **不建议**——制造 400 行永久重复代码，正是 DEC-033 要防的漂移，DEC-035 的顺序契约会立刻失效 |

### 4-2 · 事件宿主选型：DEC-015 是否复议

DEC-015（2026-04-19）定的是「macOS 用 **Tauri** 作事件宿主，不引入 objc2/NSRunLoop」。**当时的前提已变化**：那时 Settings UI 才刚要迁 Tauri，主程序 `feiyin-ime` 至今**不依赖 tauri**（实测 `Cargo.toml` 无 tauri 依赖）。照 DEC-015 执行 = 把整个 WebView 运行时拖进主程序，只为借它的 run loop。

**⚠️ 先厘清一个概念（Gavin 2026-08-04 追问「winit 是 macOS 一侧的事件循环处理机制吗」）**：

**不是。** winit 是 **Rust 的跨平台窗口/事件循环库**，它把各平台原生机制包一层。macOS 的原生机制是 **`NSApplication`（AppKit）持有主线程 run loop + `NSRunLoop`/`CFRunLoop` + `NSEvent`**；winit 在 macOS 上正是通过 `objc2` 去驱动 NSApplication。所以 winit 是**抽象层**，不是 macOS 自己的机制。

**本仓库已经在直接用 macOS 原生机制**：`src/platform/macos/hotkey.rs`（DEC-017）在自己的线程上建 `CFRunLoop` 驱动 CGEventTap（`:1` 注释、`:259` `CFRunLoop::get_current()`、`:220` `.stop()`）。即 CFRunLoop 这条路本项目已有可工作的先例。

**`cargo tree -i winit` 实测更正**：winit 0.30.13 确在依赖树内，但来源是 **`eframe 0.29.1` → `voice-ime`**，而 `eframe`/`egui` 全仓**唯一使用者是 `src/crash/reporter.rs`**（crash-reporter 的 GUI）。**它不是因为 tray-icon 或任何管线代码才存在的。** 主控上一版把它当作"顺手可用"的论据，口径不够精确，此处更正。

| 候选 | 评价 |
| --- | --- |
| **裸 `objc2` + NSApplication + CFRunLoop（改为推荐）** | ① 与 Gavin「参考 Windows 做到尽量一致」最贴——Windows 是**隐藏 controller 窗口 + `GetMessageW` + 15ms `WM_TIMER` + 自定义 `WM_APP_*`**，macOS 对应物就是 **NSApplication + CFRunLoopTimer + CFRunLoopSource**，结构一一对得上 ② 仓库已有 CFRunLoop 工作先例（hotkey.rs） ③ `core-foundation 0.10` / `core-graphics 0.25` 已是既有 macOS 依赖 ④ `Cargo.toml:115-116` 预留的 `objc2` / `cocoa` 注释标的就是 "Phase 4"，与本任务对应 |
| `winit` | 能正确托管 NSApplication 且省掉 unsafe；但**我们只会在 macOS 用它**（Windows 侧红线不动），跨平台性在此毫无收益，等于为单平台引入一层抽象 |
| `tao` | 同 winit，且需新引入 |
| 完整 Tauri（DEC-015 原文） | 为一个 run loop 拖进 WebView，体积与启动开销都不划算 |

**主控建议（已修正）**：复议 DEC-015，改为 **objc2 + NSApplication + CFRunLoop 直驱**，结论写成 DEC-045。

**唯一需要先验证的点**：`tray-icon 0.19` 在 macOS 上要求有活跃的 NSApplication，其官方示例基于 winit/tao。**我们自建 NSApplication 时它能否正常工作，需要实测**——这一条已并入阶段 A 探针的后续追加项，或在 `MACOS-P4-HOST-001` 任务书内作为第一个验证关卡。若实测不通，退回 winit 作为兜底。

### 4-3 · 四个浮层窗口是否进第一版（Gavin 2026-08-04 追问，**主控已据此修正原建议**）

**结论先行：四个窗口在 macOS 侧全部零实现**，`grep -rn "NSWindow\|NSPanel\|overlay" src/platform/macos/` 命中数为 **0**。

**实测规模（`main.rs`，全部在 `#[cfg(target_os = "windows")]` 下）**：

| 窗口 | 函数 | 行范围 | 行数 |
| --- | --- | --- | --- |
| 录音窗口（波形条） | `draw_recording_overlay` | 1049-1336 | **288** |
| 处理中窗口 | `draw_processing_overlay` | 1338-1463 | **126** |
| 失焦返显窗口 | `draw_preview_overlay` | 1465-1654 | **190** |
| 错误信息窗口 | `draw_error_overlay` | 1656-1729 | **74** |
| — 四窗绘制小计 — | | | **678** |
| 浮层线程 | `spawn_overlay_thread` + `run_overlay_thread` | 555-815 | 261 |
| 窗口过程 | `overlay_wnd_proc` | 816-972 | 157 |
| 位图合成 | `draw_overlay_to_dc` | 973-1046 | 74 |
| 文本绘制助手 | `draw_text` | 302-315 | 14 |
| **合计** | | | **≈ 1,184 行 Win32 GDI** |

**✅ 但状态层是共享的，不必重写**：`src/ui/overlay.rs`（214 行，`grep -c target_os` = **0**）已持有 `OverlayStatus` 五态枚举（`Recording` / `FallingToProcessing` / `Processing` / `FocusLost{text,copied}` / `Error`）、`PeakLevel` 音量峰值与衰减、`AudioLevelBuf` 环形缓冲。`OverlayUiEvent`（`CancelRequested` / `PreviewCopied`）在 `main.rs` 中也**未被 cfg 门控**。**macOS 缺的是「渲染 + 窗口」，不是「状态机」**——这把工作量从"重写浮层"压缩到"给既有状态机接一套 AppKit 视图"。

**🔴 主控修正自己的原建议**：本文档初稿写「overlay 建议整体不进第一版」。追问后逐个复核用途，**该建议对其中两个窗口不成立**：

| 窗口 | 缺失后果 | 可否降级 |
| --- | --- | --- |
| 录音窗口 | 用户不知道正在录音 | ✅ 可降级为**托盘图标状态**（`TrayState` 已是平台中立） |
| 处理中窗口 | 用户不知道在等什么 | ✅ 同上 |
| **失焦返显窗口** | **`PipelineEvent::FocusLost` 携带的转写文本会被静默丢弃 —— 用户说的整段话直接消失** | ❌ **不可裸降级**，必须有替代：自动复制到剪贴板 + 系统通知 |
| **错误信息窗口** | 转录失败/LLM 失败/注入失败全部静默，只在 `debug.log` 可见 | ❌ **不可裸降级**，需系统通知替代 |

### ✅ 已由 Gavin 拍板（2026-08-04）→ **DEC-045，主控的降级建议被推翻**

> Gavin 原话：**「录音需要有单独的窗口，必须和 windows 侧的实现一致，不能用托盘图标代替录音窗口，不符合用户体验」**

**决议**：
1. **录音必须是独立窗口**，不得用托盘图标代替
2. **必须和 Windows 侧实现一致**（与「参考 Windows 做到尽量一致」总纲同源）
3. `MACOS-P4-OVERLAY-001` **从阶段 E 提到阶段 C**，与事件宿主同批，录音窗口为 **P0**
4. 原 `MACOS-P4-FEEDBACK-001` 的「替代浮层」定位**取消**；系统通知若保留只能是补充，不能是替身

**主控已提取的 Windows 侧规格基线**（macOS 实现的对齐依据，全部实测自 `src/main.rs`，完整表见 `decisions.md` DEC-045）：

| 项 | Windows 实测值 | 出处 |
| --- | --- | --- |
| 录音/落波/处理中/错误 窗口尺寸 | **240 × 36** | `:549` `:551` |
| 失焦返显窗口尺寸 | **320 × 140** | `:553` |
| 定位 | **水平居中，底部上移 64px** | `overlay_geometry:1731-1744` |
| 窗口样式 | `WS_POPUP` + `WS_EX_TOOLWINDOW｜TOPMOST｜LAYERED｜NOACTIVATE` | `CreateWindowExW` |
| 圆角 | DWM 圆角，回落 `SetWindowRgn`；`CORNER_RADIUS=10` | `:1062` |
| 录音窗配色 | 背景 `#0D0F11` / 边框 `#070606` / 品牌橙 `#FF6B00` / 故障红 `#FF0000` / 静音灰 `#808080` | `:1056-1060` |
| 指示灯 | 18px 圆点，左边距 6，垂直居中，4× 超采样抗锯齿 | `:1092-1094` |
| 指示灯三态优先级 | **红（缓冲空＝设备故障）> 橙（level>0.01）> 灰（静音）** | `:1097-1114` |

**macOS 对应物**：`NSPanel`（`.nonactivatingPanel` + borderless）≈ `WS_EX_NOACTIVATE`＋`WS_POPUP`；`level = .statusBar` ≈ `WS_EX_TOPMOST`；`isOpaque=false` + `backgroundColor=.clear` ≈ `WS_EX_LAYERED`；不进 Dock / Mission Control ≈ `WS_EX_TOOLWINDOW`；`collectionBehavior = [.canJoinAllSpaces, .stationary]`。

**⚠️ 由此产生的新依赖**：需启用 `Cargo.toml:115-116` 预留的 `objc2`（/ `cocoa`），**只加在 `[target.'cfg(target_os = "macos")'.dependencies]` 段内**。

> 🔴 **加依赖时必读 `[TOML-SECTION-DRIFT-001]`**：Windows 侧 2026-07-30 就是因为把 `[target.'cfg(target_os="windows")'.dependencies]` 段头**插在 `[dependencies]` 表中间**，静默把 `tokio-tungstenite` / `futures-util` / `rustls` 三个依赖划成了 Windows 专属，在 Windows 上完全不可见。**本次加 macOS 依赖必须确认段落边界之后还剩什么。**

### 4-4 · Apple Developer 账号（$99/年）

决定 `MACOS-P4-BUNDLE-001` 的上限：

| 无账号 | 有账号 |
| --- | --- |
| 只能本地 ad-hoc 签名；**每次重新签名后辅助功能/麦克风授权会失效需重新授权**；分发给他人需手工「右键打开」绕过 Gatekeeper | 可正常公证分发，权限授权稳定 |

**若只是你自己在本机用，无账号可行**，但阶段 A3 需先确认重新签名导致的重复授权是否可忍受。

### 4-5 · `MACOS-P4-READBACK-001` 是否提前

Windows 侧词库自动学习的「注入后回读」路径**实际是瘫的**（`WM_GETTEXT` 在现代应用无效，故有待排期的 `WORDBOOK-CORRECTION-UI-001` 手动纠错方案）。macOS 的 AXUIElement **能真正读回**。这意味着 macOS 侧有机会让自动学习真正跑起来，是**本平台相对 Windows 的能力优势**。要不要在 Phase 4 就吃下这个红利，还是先保交付、放 Phase 5？

---

## §5 明确不纳入本轮（避免范围蔓延）

- **`scene-rules.toml` 的 macOS 应用词表**：现有 165 条全是 Windows `.exe` 名，macOS 侧需要独立的一批 bundle id / 可执行名。属**纯数据任务**，应在 `MACOS-P4-SCENE-001` 落地、能从日志拿到真实应用名之后再开单（与 Windows 侧 `SCENE-OBS-001` 的零成本实证路径同法）。
- **Tauri Settings UI 在 macOS 的适配**：`src-tauri` 侧编译已 0 errors，但从未在 macOS 实际启动过窗口。属独立批次，与主程序管线解耦。
- **`time_half` 单测**：既有取舍，与 Phase 4 无关（`[ITN-PREFIX-SHADOW-001]`）。
- **pytest E2E 的 macOS 章节**：`collab/build-test-guide.md` Step 3/4 全文以 Windows 为前提，补 macOS 章节是 DEC-033 附则三点名的文档缺口，应在阶段 C 有可启动产物之后再做。
- **CI**：DEC-033 附则二明确暂不启用。
