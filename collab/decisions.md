# 架构决策 · voice-ime

---

## ⚠️ DEC-000 · 基础约束：Windows 系统兼容性【最高优先级】

- **目标系统**：Windows 11、Windows 10（**已移除 Win7，2026-04-17 Gavin 决策**）
- **约束级别**：所有技术选型、API 调用、代码实现必须满足此兼容性要求
- **落地检查点**：
  - Win32 API：可使用 Win10+ 特性（如 `SetProcessDpiAwarenessContext`）
  - DWM API：`DwmSetWindowAttribute` 圆角/Acrylic 等 Win10/11 特性均可用
  - 热键：`RegisterHotKey` ✅ | 托盘：`tray-icon` ✅ | Overlay：`SetWindowRgn` ✅
- **解锁事项**（原 Win7 禁区，现可使用）：
  - Win10 1607+ 专属 API（如 `SetProcessDpiAwarenessContext`）
  - Win11 视觉特性（Mica 材质、系统圆角边框）
  - WebView2（Win10+，为后续 UI 框架升级解锁路径）

---

## DEC-001 · tray-first 主程序采用 Win32 controller

- 主线程维护隐藏 Win32 controller 窗口和消息循环
- tray、hotkey、worker、overlay 事件统一由 controller 分发
- **原因**：`eframe/winit` 不适合作为 tray-first Windows 工具的主控消息泵（已验证两条失败路线，见 troubleshooting.md [ARCH-001]）

## DEC-002 · 设置窗口独立 `--settings-ui` 入口

- 设置窗口保留 `eframe`，但从主程序主循环中解耦
- 主程序通过子进程拉起配置窗口，不共享主事件循环

## DEC-003 · 录音悬浮层统一为原生 Win32 overlay

- 音频波形条、处理中提示、失焦预览全部改为原生 Win32 overlay 窗口
- **原因**：Win32 对短生命周期悬浮层更直接，避免 viewport 可见性问题

## DEC-004 · 热键从低层 hook 切换为 RegisterHotKey

- 全局热键使用 `RegisterHotKey`；PTT 模式通过独立线程轮询检测释放（`Arc<AtomicBool>` + crossbeam channel）
- **原因**：更贴合场景，与 controller 消息循环整合更自然；Windows Timer 在 PTT 场景不可靠

## DEC-005 · 退出由 controller 统一收口

- 顺序：停止 hotkey → 停止 worker/录音 → 关闭 overlay → 关闭 settings 子进程 → 销毁 tray → 结束主进程

## DEC-006 · 配置窗口左侧 Tab 导航布局

- 左侧 SidePanel Tab 导航（140px）+ 右侧 CentralPanel 内容区 + ScrollArea
- 5 个标签页：General / Voice / Llm / Wordbook / About
- 卡片统一宽度 560px

## DEC-007 · 音频窗口方形+小圆角+绿色主题

- 从胶囊形改为方形+8px圆角，颜色统一为 `rgb(34, 197, 94)` 绿色
- **原因**：胶囊形 `SetWindowRgn` 边缘锯齿明显，方形+小圆角视觉更柔和

## DEC-008 · LLM 推理模式关闭

- 所有请求设置 `enable_thinking: false`，`max_tokens` 统一 512
- **原因**：SiliconFlow 推理模型默认开启 chain-of-thought 导致数秒延迟；语音优化场景不需要长推理

## DEC-009 · 热键线程非阻塞轮询

- 热键线程从 `GetMessageW` 改为 `PeekMessageW` + sleep(10ms)
- **原因**：`GetMessageW` 阻塞导致启动后约 15s 不响应热键

## DEC-010 · LLM 连续失败自动禁用

- 连续失败 3 次后自动标记不可用并持久化；用户重新测试连通性或修改配置时重置
- 落地：`src/config/mod.rs` 新增 `consecutive_failures` + `marked_unavailable`

## DEC-011 · 模型目录统一使用 exe 同级路径

- `model_dir()` 返回 `exe所在目录/models/`，不再依赖运行时工作目录
- **原因**：从非项目目录启动时旧路径会触发网络下载（huggingface.co），导致初始化失败

## DEC-012 · 崩溃信息收集机制架构（待实施）

- 独立 crash-reporter 子进程（与 settings-ui 同架构模式）
- 本地存储：`%LOCALAPPDATA%\voice-ime\crash.json`（只保留一份）
- 崩溃时自动弹出 UI，用户决定是否通过内置 SMTP 发送到开发者邮箱
- 影响：新增 `src/crash/` 模块、`--crash-reporter` 入口

## DEC-013 · UI 框架升级至 Tauri（渐进式路径）

- **决策**：采用渐进式升级，阶段一仅替换 Settings UI，保留 Win32 Controller/Overlay
- **原因**：规避 ARCH-001（后台事件循环问题），降低风险
- **前端框架**：React
- **保留不变**：
  - Win32 Controller 主控（DEC-001）
  - 原生 Win32 Overlay（录音窗口、信息条形窗口，DEC-003）
  - RegisterHotKey 热键（DEC-004）
  - tray-icon 托盘
- **修改范围**：
  - 移除 `eframe/egui` 依赖
  - 新增 `tauri` + React 前端
  - Settings 改为 Tauri 子进程启动
- **产物变化**：
  - 内存增加约 50-100MB（WebView2）
  - exe 体积增加约 3-5MB
- **决策时间**：2026-04-17 Gavin 确认

## DEC-014 · WebView2 运行时自动安装机制

- **背景**：Tauri UI 依赖 WebView2 运行时，Win10 用户可能缺失，需确保安装体验无中断
- **决策**：主程序启动时强制检测 + 全自动下载安装 + 自动重启
- **流程**：
  1. 主程序启动 → 检测 WebView2（注册表查询）
  2. 若缺失 → 弹出 Win32 原生提示窗口（带进度条）
  3. 自动下载官方 Bootstrapper：`https://go.microsoft.com/fwlink/p/?LinkId=2124703`（约 100KB）
  4. 自动启动安装程序（静默模式）
  5. 每 5s 检测安装进程是否退出 + 注册表验证是否安装成功
  6. 安装完成 → 提示窗口关闭 → 主程序自动重启
- **原则**：WebView2 缺失 = 主程序无法运行，不允许残缺体验
- **决策时间**：2026-04-17 Gavin 确认

## DEC-016 · macOS 自动化测试框架技术选型

- **背景**：Windows 端已采用 pyautogui + ctypes SendInput 实现全局热键模拟；macOS 需要对标方案
- **决策**：推荐组合方案如下
  - **全局热键模拟**：pyautogui（跨平台，支持 macOS 键盘事件）
  - **GUI 控件识别**：pyobjc + AppKit（原生 macOS API，比 pywinauto 更贴合）
  - **辅助工具**：
    - `atomacos`（pyobjc 封装，简化 accessibility API）
    - `PyAppleScripts`（AppleScript 调用，处理特殊场景如 Dock/Tray）
- **对比 Windows**：
  | 功能 | Windows | macOS |
  |------|---------|-------|
  | 全局热键 | ctypes SendInput | pyautogui |
  | GUI 控件 | pywinauto | pyobjc/atomacos |
  | 特殊区域 | Win32 API | AppleScript |
- **决策时间**：2026-04-20 Gavin 确认记录

---

## DEC-015 · macOS 事件循环采用 Tauri 作为主机

- **背景**：MAC-005 方案讨论，coder-1 提出直接抽象 Win32 消息循环风险高（DEC-001 核心架构）
- **决策**：macOS 用 Tauri 作为事件主机，不引入 objc2/NSRunLoop 依赖
- **原因**：Tauri 已集成，省去 objc/cocoa bindings 复杂度；两平台保持「Controller 拥有事件循环」架构意图，允许实现差异
- **影响**：MAC-005 目标收窄为 Win32 消息循环迁入 platform/windows/event_loop.rs + macOS stub；完整 macOS 事件循环在 Phase 4 以 Tauri 为宿主实现
- **决策时间**：2026-04-19 orchestrator + coder-1 协商

## [2026-04-20] [coder-1] SENDINPUT-001 测试热键模拟落在 tests/

- 背景：需要自动化验证全局热键，但 `pyautogui/pywinauto` 只能覆盖前台窗口交互，无法可靠驱动 `RegisterHotKey`
- 决策：在 `tests/sendinput_hotkey.py` 中实现 Python `ctypes + SendInput` 测试模块，直接供 pytest 调用；不新增独立测试 exe
- 原因：现有自动化框架已基于 pytest/Python，直接复用最小、调试成本最低，也不会把测试基础设施带入产线构建
- 影响：`tests/test_cases/test_hotkey.py` 改为真实热键 E2E；验证过程中同时暴露并推动修复了标准键位 PTT 释放检测缺口
## DEC-017 路 MAC-011 改用 `CGEventTap(Session)` + `CFRunLoop`

- 背景：`src/platform/macos/mod.rs` 的原占位实现仍把 `kCGHIDEventTap` 留作 TODO，但 `MAC-011` 需要真正落地热键监听。
- 决策：
  - 在 `src/platform/macos/hotkey.rs` 中实现独立监听线程
  - 使用 `CGEventTapLocation::Session`
  - 用 `CFRunLoop` 驱动 tap source，并周期性同步磁盘/内存中的热键配置
- 原因：
  - 对普通桌面应用，更适合依赖 Accessibility 权限下的 session-level 事件监听，而不是把实现推向更偏底层、权限边界更敏感的 HID tap 路径
  - `core-graphics 0.25` / `core-foundation 0.10` 已提供足够的 tap 和 run loop 封装，能直接匹配现有跨平台架构
  - 可以与现有 controller 合同保持一致，继续使用 `Start/Stop/CancelStop` 事件而不改上层录音状态机
- 影响：
  - `MAC-011` 从 stub 变为真实实现
  - `tests/test_cases/test_hotkey_macos.py` 拥有明确的后端落点
  - 后续 `MAC-012` / `MAC-013` 可以继续沿用同一类 Darwin 线程 + platform module 分层方式
## DEC-018 - MAC-012 uses clipboard-first injection with `enigo` fallback

- Background: `src/main.rs` already routes text insertion through `platform::inject_text`, and the synced macOS tests focus on TextEdit injection and clipboard preservation rather than accessibility readback.
- Decision:
  - implement clipboard mode with `pbcopy` / `pbpaste` plus `Cmd+V`
  - implement non-clipboard mode with `enigo.text()`
  - keep `capture_focused_text_snapshot()` and `read_text_from_hwnd()` as explicit stubs for now
- Reason:
  - this is the narrowest implementation that satisfies the current platform contract and test direction without prematurely designing a macOS Accessibility readback layer
  - `pbcopy` / `pbpaste` preserve parity with the existing clipboard-centric flow, while `enigo` covers direct text typing and key events with one crate
- Impact:
  - `MAC-012` is no longer a placeholder
  - clipboard restoration behavior is handled in the backend
  - readback remains open work for a later macOS accessibility task

## DEC-019 路 Windows 自定义标题栏暂不继续押注 `decorations: false`

- **背景**：`CUSTOM-TITLEBAR-001` 与 `WINDOW-TITLEBAR-002` 的真实运行验证均显示，Windows 上即使设置 `decorations: false` 并显式调用 `set_decorations(false)`，主窗口仍可能保留原生标题栏；`TASK-RESEARCH-TITLEBAR-WINDOWS` 进一步确认 Tauri 官方尚无 Windows overlay titlebar 能力。
- **决策**：
  - Windows 短期保留原生标题栏，不再把 `decorations: false` 当作当前版本的主修复方向。
  - 若后续产品强制要求“自定义标题栏替换原生标题栏但保留系统 caption buttons”，需单独立项开发 Windows-only 原生插件/PoC。
  - 原生 PoC 的首选方向是 Windows App SDK `AppWindowTitleBar` + WebView2 `WindowControlsOverlay`，并对 Windows 10 做降级。
- **原因**：
  - Tauri 配置层的 `titleBarStyle` 为 macOS-only。
  - Windows 侧同类 issue 仍在上游持续出现，社区插件也仍依赖 `set_decorations(false)`。
  - 继续在应用层重复 Win32 style 修改，收益低、风险高、难以形成稳定交付。
- **影响**：
  - 当前项目不再以 `decorations: false` 作为 Windows 标题栏改造主线。
  - 后续如需推进，将转入独立 Windows 原生能力评估与实现任务。

## DEC-020 · 第一阶段 exe 体积优化先做 profile 与 feature 收窄

- **背景**：`EXE-SIZE-OPTIMIZATION-001` 研究确认，当前体积主因在 Rust/Tauri 侧；`src-tauri` 缺少独立 release profile，且两个包都显式使用 `tokio/full` 与保留默认 `reqwest` 特性。
- **决策**：
  - 第一阶段只做低风险优化：
    - 为 `src-tauri` 增加独立 `[profile.release]`
    - 收窄主程序与 UI 子进程的 `tokio` / `reqwest` features
    - 删除主程序未使用的直接 `ureq` 依赖
  - crash reporter 拆分、UI 子进程瘦身等架构级优化仅保留评估，不在本轮直接实施
- **原因**：
  - 这一阶段不改业务逻辑，验证成本最低
  - 可以先量化“配置层优化”带来的真实收益，再决定是否继续做更重的架构调整
- **影响**：
  - 后续 tester-1 需要补 release 构建与体积对比
  - `ureq` 不会完全消失，因为仍被 `sherpa-onnx-sys` 传递引入

## DEC-021 · 第二阶段 / 第三阶段体积优化优先拆 crash reporter 与可选化 ASR

- **背景**：`EXE-SIZE-OPT-FULL-002` 分析确认，第一阶段配置级优化后收益有限；当前剩余大头分别来自 crash reporter 依赖链与 ASR 运行时 DLL。
- **决策**：
  - Phase 2 的主线优化优先项定为：crash reporter feature-gate 或独立 bin，而不是继续纠结 `reqwest` / `lettre` 的“网络栈统一”。
  - Phase 3 的主线优化优先项定为：ASR 可选化（`lite/full` 分发或插件化），而不是优先切回 `sherpa-onnx` 静态链接。
  - `shared -> static` 仅保留为低优先级备选，不作为当前版本默认推进路线。
- **原因**：
  - `src/crash/*` 独占 `lettre` / `eframe` / `egui` / `image` / `backtrace` / `chrono`，隔离度高，适合从主产物剥离。
  - `src/transcription/mod.rs` 独占 `sherpa-onnx`，当前发布目录中的 ASR DLL 合计约 18.61 MB，可选化收益远大于链接方式切换。
  - `reqwest` 与 `lettre` 分别承载 HTTP 与 SMTP，不属于协议层可直接替换关系；若要统一网络层，必须引入新的 HTTPS 崩溃上报方案。
- **影响**：
  - 后续如需继续减小默认分发体积，应优先立项 `CRASH-REPORTER-FEATURE-GATE-001` 与 `ASR-OPTIONAL-BUILD-001`
  - ONNX Runtime custom/minimal build 仅建议作为后续 PoC，不进入当前主线交付

## DEC-022 · crash reporter 采用独立 bin，而不是默认开启的 feature-gate

- **背景**：`CRASH-REPORTER-FEATURE-GATE-001` 需要在方案 A（feature-gate）与方案 B（独立 bin）之间二选一，以减少主程序体积。
- **决策**：
  - 采用方案 B：新增独立 `voice-ime-crash-reporter` 可执行文件
  - 主程序 panic 后先写本地 `crash.json`，再尝试拉起独立 reporter
  - 若独立 reporter 缺失，则降级为仅本地落盘
- **原因**：
  - 方案 A 若默认开启，默认主 exe 体积不会变小
  - 方案 A 若默认关闭，本质上是删功能而不是剥离
  - 方案 B 可以保留 reporter 功能，同时把 `eframe` / `egui` / `image` / `lettre` 从主 exe 链接面中移除
- **影响**：
  - 主 exe 预估可继续减小约 4-8 MB
  - 发布目录会新增 `voice-ime-crash-reporter(.exe)`，因此整包体积不一定按相同比例下降
  - 后续验收需补充“主程序 panic 是否能拉起 reporter / reporter 缺失时是否仅保留 crash.json”

## DEC-024 · 翻译功能架构设计（待实施）

- **背景**：用户希望在语音识别后可选翻译输出，支持中文↔英文互译，内置离线模型保证零配置可用。
- **决策**：
  - 翻译只做中文↔英文（简体/繁体是 ASR 输出的字符变体，不属于翻译范畴）
  - 优先级：LLM（已配置时）→ 内置离线翻译模型（自动降级）
  - 对用户完全透明，无需额外 API 配置
  - 内置模型选型：**NLLB-200-distilled-600M**（Meta，INT8 量化，约 350-400MB）
    - 中英互译 BLEU ~40-42，接近 Google 翻译水平
    - 日常口语短句场景可达 80%+ 实际准确率
    - 首次使用时自动下载至 `models/` 目录（同 ASR 模型机制）
  - 实现路径：`ort` crate（ONNX Runtime Rust 封装）+ `tokenizers` crate（HuggingFace Rust）
  - 新增模块：`src/translation/mod.rs`（tokenize + encode + decode + detokenize）
- **触发方式**：
  - 用户在配置界面设置「翻译热键」（单键，如左 Ctrl）
  - 翻译热键 + 录音热键同时按下 → 录音结束后执行翻译
  - 仅按录音热键（不含翻译热键）→ 正常语音输入，不翻译
  - 检测机制：录音热键触发瞬间用 `GetAsyncKeyState` 检测翻译键是否按住
- **处理链路**：录音 → ASR 转录 → LLM 优化（可选）→ 翻译（有翻译热键时）→ 注入
- **UI 改动**：
  - 左侧导航新增「热键设置」标签
  - 页面顶部子 Tab：「语音热键」「翻译热键」
  - 通用页面保留：开机自启、界面语言
- **影响文件**（待实施时参考）：
  - `src/config/mod.rs`：新增 `TranslationConfig`
  - `src/platform/windows/hotkey.rs`：`HotkeyEvent::Start` 携带 `translate: bool`
  - `src/llm/mod.rs`：新增 `translate()` 方法
  - `src/main.rs`：pipeline 加翻译步骤
  - `src/translation/mod.rs`：新建，NLLB 推理引擎
  - `ui/src/App.tsx`：新增热键设置导航项
  - `ui/src/pages/HotkeySettings.tsx`：新建
  - `ui/src/pages/General.tsx`：移除热键设置区块
  - `src-tauri/src/config.rs`：同步 TranslationConfig
- **决策时间**：2026-04-28 Gavin 确认

## DEC-023 · 独立 reporter 名称统一收敛为 `crash-reporter`

- **背景**：`CRASH-REPORTER-RENAME-001` 任务要求把独立 reporter 的程序名从 `voice-ime-crash-reporter` 简化为 `crash-reporter`。
- **决策**：
  - Cargo bin 名统一为 `crash-reporter`
  - 主程序与 `src-tauri` 的崩溃上报路径均改为直接定位 `crash-reporter(.exe)`
- **原因**：
  - 名称更短，和“独立 sidecar 工具”定位更一致
  - 同时可以消除 `src-tauri` 仍走旧启动方式的兼容问题
- **影响**：
  - 后续 tester-1 的发布与运行态验证应改看 `crash-reporter.exe`
  - 旧测试/历史记录里引用的 `voice-ime-crash-reporter` 需要按后续测试同步任务逐步更新

## DEC-025 · ASR 双模型架构：179MB CTC 直换默认 + Native hotwords 可选高准确率模型

- **背景**：POC-QWEN3ASR-002B 验收数据——179MB CTC 版首字 75%（零风险）、Native+hotwords 80%（+10pp 达标但有 hallucination 风险 + 972MB 超分发红线 + 推理延迟 14x）。
- **决策**（2026-07-06 Gavin 拍板）：
  1. 路线 A：当前生产 SenseVoice（237MB，首字 70%）直换为 FunASR Nano CTC 兼容版（179MB，首字 75%），作为**默认性能最优模型**
  2. 路线 B：配置增加 ASR 模型选项，Native FunASR Nano + hotwords（972MB）作为**可选准确率更高模型**；不随包分发——配置界面提示首次使用须先下载模型，显示下载链接与目标存放目录，用户也可自行下载
- **原因**：A 零风险纯收益立即落地；B 收益最高但体积/延迟/hallucination 三风险交由用户自选承担，可选下载规避 600MB 分发红线
- **影响**：
  - `src/transcription/mod.rs` 双模型加载分支；`src/config/mod.rs` 新增模型选择字段
  - UI Voice 页新增模型选择 + 下载引导；i18n 三语新增字符串
  - Native 路线需 hallucination 兜底（输出异常检测降级）
  - 实施状态：2026-07-06 Gavin 指令派发（coder-1：A-001+B-001+B-003；coder-2：B-002），附加兼容性红线——不得破坏 ASR 下游翻译/自动标点/词库链路
  - 前后端接口契约：Tauri command `check_accuracy_model_ready` → `{ ready, model_dir, download_url }`

## DEC-026 · ASR 长音频 VAD 分段转录（仅 accuracy 分支）

- **背景**：ASR-NATIVE-LONG-001 确证 native 模型 max_total_len=512 KV cache 硬限制（模型导出固化，无参数可调），~28s 以上音频截断为空输出/临界乱码；CTC 无此限制（实测 90s 正常）
- **决策**（2026-07-06 Gavin 拍板立项 ASR-LONG-AUDIO-001）：
  1. 采用调研报告路径 A：silero VAD（~2MB）分段 + 每段独立转录 + 文本拼接
  2. **作用域仅 accuracy 分支**：performance 无此问题且工作正常，代码路径不碰（最小回归面）
  3. 短音频（安全阈值内）保持现有单次转录路径，行为与延迟不变；仅超阈值才走分段
  4. VAD 模型缺失时降级现有行为（单次转录 + 三重兜底），不硬失败
- **原因**：业界标准做法（Whisper 同为 30s 窗口分段）；分段同时规避 LLM decoder 长序列 hallucination；每段保持 hotwords 增益
- **影响**：src/transcription/mod.rs 新增分段路径；models/ 新增 silero VAD 模型（随包 +~2MB）；Publish 同步

## DEC-027 · ASR 单模型加载：accuracy 模式去除 CTC 兜底与异常检测链

- **背景**：accuracy 模式按 DEC-025 同时常驻 native（~994MB）+ CTC fallback（~264MB）双模型，内存峰值 ~2.5GB（RESEARCH-ACC-CRASH-001 量化）；Gavin 要求降内存。
- **决策**（2026-07-07 Gavin 拍板）：
  1. **一次只加载一个 ASR 模型**：accuracy 模式不再预创建 CTC fallback recognizer
  2. **去除回滚 CTC 机制**：need_fallback 兜底链整体移除
  3. **异常检测逻辑一并删除**：is_hallucination / is_language_anomaly / is_repetitive_garbage 从 accuracy 链路移除（若无其他引用则删除函数与对应测试），输出即所得
  4. **保留 H1**（temperature 0.3）作为唯一幻觉源头缓解；空输出仍报错（转录失败提示）
  5. **确保 accuracy 可用**：VAD 分段路径（DEC-026）保留；原"分段全空/VAD 不可用→单次转录整段"的降级路径须重设计（不允许把 >28s 音频喂给 native，那是 max_total_len 未定义行为区）
- **原因**：省 ~250-350MB 常驻内存；H2' 误拦截问题（英文词密集误触）随检测删除自然消解；H1 已大幅降低真幻觉概率，残余幻觉由用户目视删除，代价可接受
- **影响**：
  - `src/transcription/mod.rs`：Transcriber 去 fallback_recognizer 字段与创建；transcribe_segment_detailed 兜底链删除；VAD 降级路径重设计
  - 部分推翻 DEC-025"Native 路线需 hallucination 兜底"与 DEC-026"降级现有行为（单次转录+三重兜底）"条款
  - 撤销今日 ASR-HALLUC-FIX-001 的 H2'（is_language_anomaly + 11 测试用例），H1 保留
  - 实施任务 ASR-SINGLE-MODEL-001，与 FIX-VAD-STATE-RESET-001 串行（同文件域）

## DEC-028 · 接入 Qwen3 在线 ASR（qwen3-asr-flash-realtime）

- **背景**：Gavin 指令接入阿里百炼 qwen3-asr-flash-realtime 在线模型，作为第三个 ASR 选项；同时配置 UI 模型选择从单选框改为下拉列表。
- **协议**（官方文档研究结论）：OpenAI Realtime 风格 WebSocket——`wss://dashscope.aliyuncs.com/api-ws/v1/realtime?model=qwen3-asr-flash-realtime`；`Authorization: Bearer <API_KEY>` + `OpenAI-Beta: realtime=v1`；流程 session.update（pcm/16kHz，turn_detection=null 手动模式）→ input_audio_buffer.append（base64 分块）→ commit → 收 conversation.item.input_audio_transcription.completed 取最终文本。
- **决策**（2026-07-07 Gavin 拍板）：
  1. **转录时机 v1**：录音结束后整段上传（手动 commit 模式），交互与现有一致；真流式边录边上屏留作后续演进
  2. **失败行为**：断网/超时/key 无效 → 报错提示转录失败，不自动降级本地（与 DEC-027 单模型内存目标一致）
  3. **Key 校验**：API Key 输入框旁加"测试连接"按钮（复用 LLM 页测连通性交互模式）
  4. **服务 URL 仅存配置文件**（默认北京 region），不在 UI 显示；用户只需配 API Key
  5. **UI 三选项下拉**：本地模型-快速（performance）/ 本地模型-长音频（accuracy，顺带消化 todo 遗留④文案问题）/ Qwen3在线语音识别服务（qwen3_online）；说明文字品牌色显示在下拉框下方
  6. **选 Qwen3 未填 key 即离开 → 自动回退到之前的模型选项**
- **配置契约**：audio.asr_model 新值 "qwen3_online"；audio.qwen3_api_key（UI 可编辑）；audio.qwen3_asr_url（默认 wss://dashscope.aliyuncs.com/api-ws/v1/realtime，仅配置文件）
- **影响**：src/config/mod.rs + src/transcription/（新 qwen3_online 模块 + AsrModel 三值）+ Cargo.toml（WS 依赖）｜ui/src/pages/Voice.tsx + i18n + src-tauri（config 同步 + test_qwen3_asr_connection command）
- **边界**：qwen3_online 模式不加载任何本地模型（零本地 ASR 内存）；输出自带标点（native_punctuated=true 语义）；词库 hotwords 纠偏 v1 不接入在线模型（后续评估）

## DEC-029 · 词库单词化：词对（raw→corrected）改为单词（word）模式

- **背景**：词对模式是早期为 LLM 映射替换设计的；现词库主要价值是 accuracy 模型 hotwords 偏置与 LLM 词汇表纠偏，词对映射已不匹配。
- **决策**（2026-07-10 Gavin 拍板）：
  1. wordbook 数据模型改单词（word 单列），migration 003 存量词对取 corrected 侧去重导入
  2. 移除转录后 `wb.apply()` 词对文本替换（能力取舍 Gavin 已确认：不开 LLM 时词库不参与 performance/qwen3 纠偏）
  3. LLM prompt 从 XML 映射表改为「用户词汇表」语义（发音相近误写→修正为词汇表写法）
  4. LLM suggestions 自动学习保留，回传格式改单词数组，频次阈值机制不变
  5. hotwords 链路（仅 accuracy）改读单词列表，精选规则（上限20/≤10字/ASCII短词过滤）不变
- **影响**：src/wordbook/* + migration 003 + main.rs + llm/mod.rs + transcription/mod.rs + src-tauri/wordbook.rs + ui/Wordbook.tsx + 双侧 i18n + 全层测试；WORDBOOK-CORRECTION-UI-001（未排期）未来改学单词
- **实施**：WORDBOOK-SINGLEWORD-001（Phase1 CORE=coder-1 → Phase2 UI+Tauri=coder-2）

## DEC-030 · 智能 ITN 数字规整：自研规则模块（方案 A），单字保护

- **背景**：两个本地模型的模型级 ITN 开关（use_itn/itn:1）实际无效，中文数字原样输出；ASR-CTC-OPT-001 P2 的 rule_fsts 方案因「七→7」单字副作用撤销。
- **决策**（2026-07-10 Gavin 拍板立项 ITN-SMART-001）：
  1. 自研 Rust 后处理模块 `src/itn.rs`，纯函数确定性规则，转录后/LLM 前统一挂入管线，三模型（performance/accuracy/qwen3）一致生效（对已是阿拉伯数字的输入幂等）
  2. 核心原则：**多位数字串/带计量语境的转阿拉伯，单字数字无单位语境保留汉字**
  3. 覆盖场景（Gavin 指定+扩展）：金额、电话号码、日期时间、年份、经纬度、温度、压力、百分比、小数、分数、序数（第X）、计量单位（长度/重量/容积/电学/频率等）、门牌房间号、逐位数字串（幺→1）
  4. 保护规则：单字+通用量词保留（三个人）、含数字成语/习语白名单（三心二意等）、专有名词白名单（三亚/五一等）
  5. v1 默认开启无 UI 开关，端测观察后再决定是否加用户开关（方案 C 留作演进）
  6. **规则数据外置（2026-07-10 Gavin 补充拍板）**：算法与规则数据分离——中文数字解析算法（进位/组合/小数/幺）留代码；触发词表（单位分类表/场景前缀词/序数触发词）、保护白名单（成语/专有名词/通用量词）、歧义裁决开关（单字+量词是否转换、零下输出格式等）全部外置到 exe 同级 `itn-rules.toml`（沿 DEC-011 exe 同级路径原则）。同一份规则文件经 include_str! 编译期嵌入作为内置默认，外部文件存在则覆盖、缺失/解析失败降级内置默认（零配置可用，升级规则只需替换 toml 不动 exe）
- **影响**：新增 src/itn.rs + itn-rules.toml（+Publish 同步）+ main.rs 管线挂钩；与 WORDBOOK-SINGLEWORD-001-CORE 同文件域（main.rs），串行派发 coder-1
- **实施**：ITN-SMART-001，等 CORE 完成后派发

## DEC-031 · 格式化输出：单开关统一配置，F1/F2/F3 无独立开关

- **背景**：DESIGN-FORMAT-SCENE-001 原方案为 F1/F2/F3 三独立开关（F3 默认关）；Gavin 2026-07-13 审阅后简化。
- **决策**（2026-07-13 Gavin 拍板）：
  1. **不设三开关**：现有「LLM 优化」配置整体更名为「格式化输出」，`llm.enabled` 即格式化输出总开关（字段名不变，零迁移）；开启即 F1 语气词去除 + F2 改口修正 + F3 结构重组 三段指令全部生效
  2. **开启门槛**：UI 上提示并校验——开启格式化输出必须已配置 LLM 信息且连接测试通过（connectivity_verified）
  3. **运行时失败行为**：LLM 调用失败 → 本次格式化失败，报错提示用户检查配置（overlay），**不自动关闭开关**，下一次照常调用；原文注入兜底保持（用户语音不丢）。DEC-010「连续失败自动禁用」立场废止（经查该机制实际未在代码落地，无需回滚）
  4. **F3 多行安全**：Phase 1 无场景感知，LLM 输出多行一律兜底单行化（换行→"；"）；Phase 2 场景感知按 multiline_safe 放开
  5. **场景感知词表**（Phase 2 要求）：scene-rules.toml 应用列表尽可能详细，各类别不限于头部 5-8 个，覆盖越全感知体验越好
  6. **版本号**：本批次升 v0.7.0（root Cargo.toml + src-tauri/Cargo.toml + tauri.conf.json）
- **影响**：FORMAT-LLM-001-CORE/UI 任务定义按此更新；config 无新增字段
- **实施**：FORMAT-LLM-001（CORE=coder-1 / UI=coder-2 并行）

### DEC-028 实施附注（2026-07-08 全链交付后补记）

- **协议对齐**：session.update 按官方 schema 为 `input_audio_format:"pcm"` + 独立 `sample_rate:16000` + `modalities:["text"]` + `input_audio_transcription:{language}`（asr_language 明确时传，auto 省略）；model 仅在 URL query
- **超时终版**（Gavin 拍板输入法即时反馈原则）：连接 5s / 静默超时 10s（任何服务端消息重置）/ 硬上限 max(30s, 音频×0.5)；不加重试，fail-fast
- **endpoint 事实**：MaaS 工作空间签发的 key 必须配工作空间 URL（`wss://{WorkspaceId}.{region}.maas.aliyuncs.com/api-ws/v1/realtime`，Gavin 实际使用），默认 dashscope URL 仅适用经典百炼 key；默认值策略待 Gavin 拍板
- **四个联网盲区 P0 教训**（编译/单测/冒烟均检不出）：rustls provider 冲突 / WS 握手头缺失 / __rustls-tls 无根证书 / HTTP 101 误判——详见 lessons 2026-07-08 与 CHANGELOG v0.6.1 DEC-028 版块


## DEC-032 · 多执行路径的配置与数据刻意隔离（不收口）

- **背景**：2026-07-25 WORDBOOK-AUTOLEARN-001 诊断中，主控发现 `wordbook.sqlite` 存在三份（`target/release/`、`Publish/`、`%APPDATA%\Roaming\voice-ime\`），一度判定为"多库并存干扰观察"的缺陷，建议收口到唯一位置。
- **决策**（2026-07-25 Gavin 澄清，**驳回收口建议**）：
  1. `db_path()` = exe 同级目录（DEC-011 既有原则）的行为**保持不变，不收口**
  2. 不同 exe 路径下的配置与词库等数据**本就应当彼此独立**，这是刻意设计而非缺陷：
     - `target/release/` —— 本地端侧测试用实例
     - `Publish/` —— 本地打包分发用实例
  3. 两者需要各自独立的配置与词库数据，互不污染
- **原因**：端测实例与打包实例若共用一份数据，端测过程中的脏词库/试验配置会污染待分发产物，反之打包实例的数据也会干扰端测结论。数据隔离是按用途区分运行实例的前提。
- **影响**：
  - 任何 Agent **不得**以"多库并存"为由提出合并/收口 db 或 config 路径
  - 排查"词库/配置没生效"类问题时，**必须先确认目标实例的 exe 路径**，再定位对应目录下的 `wordbook.sqlite` / `config.toml`，不能假设只有一份
  - `%APPDATA%\Roaming\voice-ime\` 下那份为历史遗留（05-08 旧 schema，migration 003 未在其上运行过），非当前活跃实例，不影响本决策
  - 副作用记录：exe 同级路径意味着 `cargo build` 不会清理数据，但 `cargo clean` 会连带删除 `target/release/` 下的词库与配置——端测数据不具持久性，重要词条应在 Publish 侧或手工备份

### DEC-031 实施勘误（2026-07-13 Gavin 端测纠正）

- **违反事实**：SCENE-SENSE-001-UI（主控任务定义失误）在格式化输出页新增「场景感知」区块两开关（scene.enabled / send_window_title），违反 DEC-031-① 单开关统一配置原则——Gavin 端测发现后重申：只用「启用格式化输出」一个开关，不需要其他任何开关配置。
- **修正**（SCENE-SENSE-002-UI）：UI 整块移除（不迁移到其他页），三语 5 key 删除，SCENE-UI 测试删除；场景感知随 llm.enabled 实际生效（F4/裁决仅作用于 LLM 路径，后端零改动）；scene.enabled（默认 true）/send_window_title（默认 false）保留为 config.toml 隐藏字段（不在 UI 暴露），src-tauri SceneConfig 结构保留（serde 往返防丢段）。
- **教训**：派发任何新增用户可见配置项的任务前，必须回查既有 DEC 约束；「单开关」类原则性决策适用于后续所有 Phase，不因新功能而默认豁免。

---

## DEC-033 · 双平台单仓库并行开发：平台兼容为首要约束，分工按「共享 + Windows」/「macOS」切分

- **背景**：2026-07-29 Gavin 请 macOS 侧开发人员完成移植可行性评估（`docs/MACOS-PORT-ASSESSMENT.md` / `docs/BUILD-MACOS.md`），主控派发 RESEARCH-MACOS-DUALPLATFORM-001 复核后结论 GO（有条件）。Gavin 据此确定长期开发模式。
- **决策**（2026-07-29 Gavin 拍板）：
  1. **一套代码 + 一个 GitHub 仓库 + 两侧平台并行开发**。macOS 侧由独立 Agent 团队负责，两侧提交同一仓库
  2. **平台兼容是首要约束**：任何架构设计、方案设计、代码开发，**必须首先考虑跨平台兼容性**，不得再产出仅 Windows 可编译的新代码
  3. **分工边界**：
     - **本侧（Windows Agent 团队）负责**：两侧共享的通用代码 + Windows 专用（Win32）代码的开发与构建，以及**保证接缝（平台契约）存在且不漂移**
     - **macOS 侧团队负责**：macOS 专用代码的开发与构建
  4. **硬约束（Gavin 2026-07-29 两次重申）**：**代码重构不得影响任何 Windows 代码功能**。为 macOS 做的适配一律以「Windows 零行为改动」为验收前提，宁可保留平台相关类型差异，也不为对称性去改已交付的 Windows 路径
  5. **macOS 进度落后可接受**：其功能实现（移植报告的 B/C/D 阶段）后续由 macOS 团队启动；本侧当前只做 A 阶段——**让 macOS 侧 checkout 后能上手继续开发**
- **原因**：
  - Rust 的 `#[cfg]` 切掉的代码**不做类型检查**（✅官方 Rust Reference），故平台层签名漂移不会触发任何编译错误。项目在纯 Windows 阶段就已漂移 6 处（`FocusedTextSnapshot.hwnd` 一侧 `HWND` 一侧 `usize`、`notify_config_changed` / `capture_scene_signals` macOS 侧不存在等），**破坏发生在提交那一刻、暴露在数周后**
  - **trait 抽象不能防漂移**（trait 只约束当前被编译目标上的实现），**双平台 CI 是唯一可靠防线**
  - 本仓库为**公开仓库**（实测 `"visibility": "public"`），标准 GitHub-hosted runner 含 macOS **免费且不限量**，故 CI 防线无成本障碍
- **影响**：
  - `src/platform/mod.rs` 的 glob 导出（`pub use windows::*`）改为**显式清单**，使两侧导出面在同一文件内可肉眼比对；漏列会立即编译失败（响亮失败优于静默漂移）
  - **平台相关类型差异（`HWND` vs `usize` 等）刻意保留不统一**——统一需改 Windows 已交付路径，违反第 4 条硬约束。改为在契约注释中显式标注，由 CI 兜底
  - `.gitignore` 对 `.github/` 的排除需解除，双平台 CI 入库
  - 实施批次：MACOS-COMPAT-001（A 阶段）
- **与 DEC-000 的关系**：DEC-000「Windows 系统兼容性最高优先级」**继续有效且不降级**——它约束的是「Windows 上必须支持 Win10/11」，与本决策「新代码必须跨平台可编译」是**正交**的两件事。二者叠加后的完整含义是：**Windows 行为不可退化（DEC-000 + 本决策第 4 条），同时新代码不得阻断 macOS 编译（本决策第 2 条）**。

### DEC-033 附则 · 工作冻结令（2026-07-29 Gavin 指令）

- **指令原文**：「目前是做好跨平台开发的代码重构和准备，ok 前先不做任何新代码开发」
- **含义**：在跨平台兼容重构完成并经 Gavin 确认 OK 之前，**冻结一切新功能开发、Bug 修复与优化项**；仅跨平台重构/准备类任务、以及文档、研究、测试同步可以推进
- **已受影响的既有排期**：FIX-COT-LEAK-001-P0（LLM 思维链泄漏五项修复，方案已完备）转入冻结，待解冻后重启
- **派发纪律**：主控在解冻前不得派发非跨平台重构类的代码任务；Worker 收到疑似越界任务应主动回问主控
- **原因**：双平台单仓库模式下，若在接缝尚未建立、双平台 CI 尚未生效时并行推进功能开发，新代码会持续产出仅 Windows 可编译的实现，重构面只增不减；先把接缝与防线立起来，后续所有开发才能天然满足 DEC-033 第 2 条「平台兼容为首要约束」

### DEC-033 附则二 · 暂不启用 GitHub CI/CD，维持本地平台构建发布（2026-07-29 Gavin 指令）

- **指令原文**：「目前暂未考虑使用 github 的 CI/CD，还是用本地平台构建发布」
- **决策**：
  1. **不建双平台 CI**：`.gitignore` 对 `.github/` 的排除**保持不变**；现有 `.github/workflows/build-macos.yml`（816 B，2026-04-19，内容已陈旧）保持未入库状态，不修改、不提交
  2. 构建与发布**继续走本地流程**：Windows 侧沿用 `collab/build-guide.md` 三步流程 + `Publish/` 同步；macOS 侧由 macOS 团队在本机构建
  3. MACOS-COMPAT-001-TAURI-CI 任务的 B-1/B-2 两项（解除 gitignore 排除、重写 workflow）**取消**
  4. **B-3 保留但重新定位**：sherpa-onnx 获取脚本继续做，目的从「喂 CI」改为「解决全新 checkout 构建不了的既有问题」——这是 macOS 团队能否起步的前置条件，与 CI 无关
- **⚠️ 主控已向 Gavin 声明的风险（决策仍以 Gavin 为准）**：
  - RESEARCH-MACOS-DUALPLATFORM-001 的核心结论是「**双平台 CI 是防止签名漂移的唯一可靠防线**」——因为 `#[cfg]` 切掉的代码编译器不做类型检查（✅官方 Rust Reference），trait 抽象亦无法约束被切掉的那一侧
  - 无 CI 状态下，「Windows 侧改动破坏 macOS」与「macOS 侧改动破坏 Windows」**都不会在提交时暴露**，回到「破坏发生在提交那一刻、暴露在数周后切换机器那一刻」的状态。项目在纯 Windows 阶段已因此漂移 6 处
  - 本仓库为公开仓库，标准 runner 含 macOS **免费不限量**，故该防线的成本障碍并不存在——若未来改变主意，可零成本启用
- **替代防线（无 CI 状态下的实际手段，逐级减弱）**：
  1. **显式导出清单**（MACOS-COMPAT-001-CORE 3.3 已落地）：两侧导出面集中在 `src/platform/mod.rs` 同一文件内，可肉眼比对；本侧漏列会立即编译失败
  2. **契约注释块**（同上）：平台相关类型差异显式标注，新增 stub 遵循「名称 + arity 相同，类型可平台化」原则
  3. **交接纪律**（写入 `docs/MACOS-HANDOFF.md`）：任何一侧改动 platform 层导出面，**必须同步更新 `platform/mod.rs` 的两份清单**，并在 PR 描述中声明
  4. **两侧各自本地 `cargo check`**：只能验证本平台，防不住对侧——这是无 CI 状态下的固有缺口，需靠纪律弥补

---

### DEC-033 附则三 · macOS 侧执行细则（2026-07-30 Gavin 重申 + macOS 侧补充，原 DEC-034 并入）

> 本附则由 macOS 侧于 2026-07-30 起草。当时 `collab/` 尚未入库、两侧互不可见，
> macOS 侧不知 DEC-033 已存在，遂依 Gavin 当日口头指示新立 DEC-034 记录同一治理约束。
> Gavin 2026-07-30 拍板**两条合并**，故 DEC-034 正文并入此处，编号作废。

- **Gavin 2026-07-30 对 macOS 侧的原话要点**（与 DEC-033 主决策第 1-3 条一致，此处存证）：
  一个代码仓 + 两端并行；核心通用代码两边共享；Windows / macOS 本地专用代码各自开发；
  但**必须遵守跨平台兼容规范——从架构设计、技术选型、到方案、代码设计与开发，都以此为首要约束**。

- **对所有 Agent 的执行要求**（DEC-033 第 2 条的可操作化）：
  1. 任何涉及**共享代码**的任务书，必须包含「对另一平台的影响评估」一节；主控派发前自查，Worker 收到后可就此提异议
  2. **改 `src/config/mod.rs` 的 `AppConfig` 结构是最高风险动作**——两侧平台层都消费它，
     改字段名在 Windows 上照样编译通过而 macOS 直接炸。此类改动必须显式列出两侧平台层调用点
  3. 新增 macOS stub 遵守 `src/platform/mod.rs` 的 stub 设计原则：
     **名称 + arity 与 Windows 侧相同，仅参数类型平台化**；arity 差异是最后手段
  4. 任一侧改动 `platform/` 导出面，必须**同步更新 `src/platform/mod.rs` 中两份清单**并在提交信息声明

- **关于与 DEC-000 的关系（macOS 侧原 DEC-034 的表述已作废，以 DEC-033 正文为准）**：
  原 DEC-034 写「跨平台兼容接管了 DEC-000 的最高优先级定位」。
  **该表述不采纳** —— DEC-033 正文的判读更准确：DEC-000 约束的是「Windows 上必须支持 Win10/11」，
  与「新代码必须跨平台可编译」是**正交**的两件事，DEC-000 继续有效且不降级。

- **本条附带记录的两项文档缺口（待 Gavin 排期）**：
  - `CLAUDE.md` 通篇以 Windows 为前提（构建命令、PowerShell 备份脚本、
    git 凭证路径 `C:\Users\Aaron-GMK\...`），与双平台并行模式冲突，需重写
  - `collab/build-test-guide.md` 全文以 Windows 为前提（pywinauto / `.exe` / PowerShell），
    macOS 侧 tester-1 的 Step 3/4 无对应实现，需补 macOS 章节后才能派发测试执行类任务

---

## DEC-034 · 【已并入 DEC-033，编号作废】

> 2026-07-30 Gavin 拍板：DEC-034 与 DEC-033 主题重叠（同为「双平台单仓库 + 平台兼容为首要约束」），
> **两条合并，以 DEC-033 为准**。原 DEC-034 正文已并入 **DEC-033 附则三**。
>
> **成因存档**：`collab/` 于 2026-07-30 才移出 `.gitignore` 入库，此前两侧各自本地、互不可见。
> macOS 侧不知 DEC-033 已存在，依 Gavin 当日口头指示新立 DEC-034 记录同一约束。
> 这本身即是「两侧文档不互通」代价的一个实例，也是 collab/ 入库的直接动因之一。
>
> 本编号保留为墓碑，不再复用；引用请改指 DEC-033 及其附则三。


---

## DEC-035 · ITN 调用位置反转：从「LLM 前」移到「LLM 后、标点前」

- **背景**：Gavin 2026-07-30 端测报「说摄氏度，输出没转成 ℃ 符号」。主控日志取证的根因链：
  1. ASR 把「摄氏」误听成「摄息/摄斯/摄四」——`target/release/debug.log` 全量温度类听写 **11 次，只有 2 次听对**（摄四度 7 / 摄氏度 2 / 摄息度 1 / 摄斯度 1）
  2. ITN 的 ℃ 规则判据是 `unit_word.contains("摄氏")`（`src/itn.rs:760/801`），字面不匹配 → 不加符号
  3. LLM 随后把「摄息」纠正为「摄氏」，但输出的是**汉字**
  4. ITN 全仓唯一调用点在 LLM **之前**（原 `src/main.rs:2949`）→ **LLM 纠正后再无 ITN 机会**
- **决策**（2026-07-30 Gavin 提出方向、主控细化落点）：
  1. `itn::normalize_numbers` 从「转录后 / LLM 前」移到**三分支产出 `final_text` 之后、本地标点块之前**
  2. **不放在管线最末端**（主控原方案，已否）。理由：今天标点引擎吃的就是 ITN 之后的文本（日志实证 `Local punctuation applied: '...达到40摄斯度' -> '...，达到40摄斯度。'`，输入里已有「40」）。若把 ITN 挪到标点之后，标点模型（CT-Transformer，在 ASR 转写风格文本上训练）的输入会从「汉字数字」变成「阿拉伯数字 + ℃ 符号」= 分布外。**模型的分布外退化无法用规则修复，只能观测；ITN 是确定性规则引擎，输入域变化可用规则 + 单测补齐。把不确定性留在可控的一侧**
  3. **三条路径都必须经过 ITN**：(a) LLM 成功 (b) **LLM 运行时失败兜底** (c) LLM 关闭。放在「三分支之后」这一个位置天然覆盖三条，**不在各分支重复调用**。(b) 尤其关键——漏了它用户会看到纯汉字数字（「四十摄氏度」），**比不修更差**；该路径真实发生过（07-30 12:37 `LLM formatting failed; raw text injected as fallback`，07-29 连续三次 siliconflow 超时）
  4. `is_effective_text` 门控随之改吃 `raw_text`。ITN 只改数字形态、不增删语义字符，语气词（啊/呃/嗯）非数字，故 filler 判定结果不变
- **推翻的既有条文**：**DEC-030-① 的「转录后 / LLM 前统一挂入管线」原文自本条起作废**。任何 Agent 不得依 DEC-030 原文把 ITN 挪回 LLM 之前。
- **配套发现（同批次，必需）**：仅靠移位置**不足以修复**。ITN 的 ℃ 替换只存在于「中文数字→阿拉伯数字」转换分支内，而其契约为「已是阿拉伯数字的输入逐字节不变」，且全仓无任何「已是阿拉伯数字时做单位符号规整」的通道（`grep '°' src/itn.rs` 命中 0）。而 LLM **会自己把中文数字转成阿拉伯数字**（日志实证：`温度是四十四四度` → LLM 输出 `温度是44度。`）→ ITN 拿到的仍是阿拉伯数字 → ℃ 依旧不出现。故必须同批实施 **ITN-CELSIUS-003**：新增独立于中文数字路径的单位符号规整（`40摄氏度`→`40℃`、`40°C`→`40℃`），并保持 **`44度` 绝不转**（DEC 级既有拍板：角度/温度同形，强转会把「转九十度」变「转90℃」）。副作用：`itn.rs` 的幂等契约措辞需从「已是阿拉伯数字逐字节不变」改为「`f(f(x))==f(x)`」
- **跨平台影响（DEC-033 附则三要求）**：`run_pipeline` 整个函数在 `#[cfg(target_os = "windows")]` 内，macOS 侧管线目前仍是 `mod macos_stubs` 空壳。**因此本条的顺序约束在代码层面无法共享，是纯文档契约** —— 已写入 `docs/MACOS-HANDOFF.md` §2.8。若 macOS 侧实现管线时把 ITN 放回 LLM 之前，℃ 缺失的缺陷会在 macOS 上完整复现。
- **实施**：ITN-REORDER-001（已验收，含主控一处修正）+ ITN-CELSIUS-003（进行中）
- **决策时间**：2026-07-30
- **⚠️ 本条已被 DEC-036 部分推翻（2026-07-31）**：第 1/2 条的「单点位置」结论作废，改为双通道。**第 3 条（三条路径都必须经过 ITN）与第 4 条继续有效。**

---

## DEC-036 · ITN 改为双通道：主通道回到 LLM 前，补丁通道留在 LLM 后（部分推翻 DEC-035）

- **背景**：Gavin 2026-07-31 端测反馈「LLM 优化输出时会对某些词曲解误解，导致后端 ITN 转换失败」，实例：**`四点三刻`（=4:45）被 LLM 曲解为 `4:30`**，原始输入信息在 LLM 环节即被销毁，ITN 无论放在多后面都救不回来。Gavin 据此指令把 ITN 移回 LLM 之前。

- **核心矛盾**：DEC-035 把 ITN 从「LLM 前」移到「LLM 后」的根因是相反方向的真实缺陷——ASR 把「摄氏」误听成「摄四/摄息/摄斯」（日志实证：温度类听写 11 次只对 2 次），ITN 字面不匹配 → ℃ 不出现；LLM 纠正为「摄氏」后若 ITN 已跑完则再无机会。**两个缺陷方向相反，任何单点位置都无法同时满足。**

- **决策**（2026-07-31 Gavin 指令方向 + 主控与 coder-1 双稿独立收敛）：

  1. **拆为双通道**，利用 `normalize_numbers`（`src/itn.rs:682-688`）**本来就是两段式**这一既有事实（`normalize_with_rules` 中文数字段 + `normalize_unit_symbols` 阿拉伯数字单位符号段），把两段拆到管线两处：

     | 通道 | 位置 | 调用 | 职责 |
     | --- | --- | --- | --- |
     | 主通道 | `is_effective_text` 之后、LLM 之前 | `itn::normalize_numbers` 完整版 | `四点三刻`→`4:45`，LLM 拿到成形数字 |
     | 补丁通道 | 三分支 `final_text` 之后、本地标点之前（原 DEC-035 位置） | `itn::normalize_unit_symbols_only` | 捞回 LLM 纠正后的 `40摄氏度`→`40℃` |

  2. **幂等性是本方案成立的唯一技术前提，已实测**：`cargo test --bin feiyin-ime unit_symbol` 11/11 passed（含 `unit_symbol_idempotent`）；机制上 `40℃` 的 `℃` 非 ASCII 数字开头，补丁通道不匹配、逐字节抄出，`f(f(x))==f(x)` 成立。

  3. **DEC-035 第 3 条「三条路径都必须经过 ITN」继续有效**：主通道置于 `raw_text` 之后天然覆盖 (a) LLM 成功 (b) LLM 运行时失败兜底 (c) LLM 关闭；补丁通道置于三分支之后同样覆盖三条。**(b) 尤其关键**——漏了它用户会看到纯汉字数字，比不修更差。

  4. **DEC-035 第 2 条「ITN 不放管线最末端」的理由继续有效**：本地标点引擎（CT-Transformer）在 ASR 转写风格文本上训练，把 ITN 挪到标点之后会让标点模型输入分布外。双通道方案的两个位置**都在标点之前**，该约束未被破坏。

- **配套发现（主控独立取证，本条决策的第三个论据）**：`src/llm/mod.rs:29` 的 `UNIT_SYMBOL_PROTECTION` 指令正文写着「The input text **already contains normalized numbers and unit symbols**…」。该指令带 `ITN-CELSIUS-002-PROMPT` 标签，写于 ITN 尚在 LLM 之前的年代；**DEC-035 反转顺序时未同步修订它，导致其前提自 2026-07-30 起即为假**（LLM 实际拿到的是汉字数字）。**主通道回移恰好恢复了该指令的前提，使其从空转变为真正生效。**

- **配套改动（同批次必需）**：该指令覆盖的是**记法**（notation：符号↔文字、记法风格），**不覆盖数值与语义** —— `4:45`→`4:30` 是数值错误、`明天`→`今天` 根本不是数字。故须追加**事实保全**条款：禁止对任何数值/时间/日期做重算、取整、重新表述或替换。落点 `src/llm/mod.rs:29`（+ 评估 `:30` 翻译路径版本）。

- **跨平台影响（DEC-033 附则三要求）**：`run_pipeline` 整个函数在 `#[cfg(target_os="windows")]` 内，macOS 侧管线仍是 `mod macos_stubs` 空壳，**故本条的顺序约束在代码层无法共享，是纯文档契约**。`docs/MACOS-HANDOFF.md` §2.8 此前已按 DEC-035 写入「ITN 在 LLM 之后」的契约，**本次反转必须同步修订该节**，否则 macOS 团队实现管线时会按已作废的顺序落地。

- **实施**：ITN-V2-ENGINE-001（coder-1，`src/itn.rs`+`src/main.rs`）+ ITN-V2-PROMPT-001（coder-2，`src/llm/mod.rs`）
- **决策时间**：2026-07-31

---

## DEC-037 · ITN 输出形态：按单位族分治，货币归一到标准单位

- **背景**：Gavin 2026-07-31 反馈 ITN「转换不彻底、不伦不类」，实例 `十一块九毛二`→`十一块9毛2`、`四点半`→`4点半`，并要求「结合现有各类单位，从总体上、更高的高度设计兼容方案，不能转换得生硬、像机器翻译一样」。

- **决策**（2026-07-31 Gavin 拍板）：**按单位族分治，货币也规范化**

  | 单位族 | 形态 | 实例 |
  | --- | --- | --- |
  | 时间 | 通用书写形式 | `四点半`→`4:30`、`五点三刻`→`5:45` |
  | 度量衡 | 小数合并 | `一米二`→`1.2米`、`三十九度八`→`39.8度`、`一吨半`→`1.5吨` |
  | 货币 | **归一到标准单位** | `十一块九毛二`→`11.92元`、`五块八`→`5.8元` |

- **主控补充的实施细则（Gavin 未示例，主控裁定，可被推翻）**：Gavin 给的货币示例**全部带小数或多级链**。对**裸单位无小数**的情况（`五块钱`/`十一块`）**保留原单位词**（`5块钱`/`11块`），不强转为 `5元钱`——那会改变用户语体。**规则：仅当发生小数合并或多级单位链合并时才归一到标准单位（元/米/吨）；单一单位无小数则保留原单位词。**

- **统一文法族（「余数后缀」，三型）**：

  | 型 | 模式 | 实例 |
  | --- | --- | --- |
  | 甲 分数后缀 | `N<单位>半` / `N点M刻` | 四点半、五点三刻、一吨半、一个半小时 |
  | 乙 隐式小数位 | `N<单位>M`（M 后无单位） | 一米二、三十九度八 |
  | 丙 显式多级单位链 | `N<u1>M<u2>[K<u3>]` | 十一块九毛二、三小时二十分 |

- **乙型边界护栏（主控设计，本形态能否安全落地的关键安全阀）**：隐式小数位**仅当尾数数字后紧邻边界**（字符串结束/标点/空白）才触发。一条规则挡住全部门牌与序列误转，无需额外黑名单：`一米二。`✅转｜`三年二班`❌（`二`后为汉字 `班`）｜`三楼二号`❌｜`五排八座`❌（`排`/`座` 不在单位表）。

- **新增数据需求**：`itn-rules.toml` 的 `[units.*]` 当前只是**平铺集合无层级**，丙型需新增「单位层级表」（元/块 > 角/毛 > 分；米 > 分米 > 厘米；吨 > 千克 > 克；小时 > 分 > 秒）。

- **决策时间**：2026-07-31

### DEC-037 附则 · 全或无：宁可整体不转，不produce撕裂输出（2026-07-31 Gavin 指令）

- **触发**：主控 P3 验收时实测 `三年二班` → **`3年二班`**（`三` 转、`二` 未转）。成因与 `十一块九毛二` 不同——③ 块级匹配要求每段都有单位，而 `班` 不在单位表 → 整块识别失败 → 回退逐字 → `三` 因后跟 date_suffix `年` 而转、`二` 因后跟非单位 `班` 而不转。**该行为早于 ITN-V2 各批，非本次引入。**

- **Gavin 原话**：「`3年二班` 这种情况就直接不转了吧，应该要保护起来，按照原来的输入输出就行。」

- **决策**：**在一段连续的「中文数字 + 跟随字符」链中，若部分数字会转、部分不会 → 整段都不转，原样输出。**

  | 输入 | 试算 | 输出 |
  | --- | --- | --- |
  | `三年二班` | 混合 | **`三年二班`**（全汉字） |
  | `十一块九毛二` | 全转 | `11.92元` |
  | `三楼二号` | 全转 | `3楼2号` |
  | `五排八座` | 全不转 | `五排八座` |

- **实现约束（主控，源自实测）**：**不得以「③ `try_parse_composite_block` 返回 `None`」作为撕裂信号。** 该函数的 `match_unit_word(...)?` 用 `?` 而非 `break`，任何一段单位匹配失败即从整个函数返回 `None`——**该转的 `十一块九毛二`（末尾 `二` 无单位）与不该转的 `三年二班` 都会返回 `None`，两者无法区分**。必须改为「**先试算每段是否会转，再决定整段**」。

  > **附带事实**：`十一块九毛二`→`11块9毛2` 的修复实际由 **①右邻否决 + 逐字路径**产出，③ 在该用例上从未触发。③ 目前只在「每段都有单位且干净收尾」（如 `三楼二号`）时生效。

- **与 DEC-038 的关系**：DEC-038 的「撕裂」定义此前限于「保护词表遮蔽语义单元前半段」。本附则将其扩展为**第二种撕裂来源：单位表覆盖不全导致的部分转换**。两者现象相同（一半汉字一半数字），处置一致（全或无）。

- **决策时间**：2026-07-31

---

## DEC-038 · 保护词表不得承载规则性语法族（机器派生词表的边界）

- **背景**：主控 2026-07-31 独立取证发现，`itn-rules.toml` 的保护词表对**同一语法族的覆盖是随机的**：

  | 在保护表内 | 不在保护表内 |
  | --- | --- |
  | 一点半、六点半、八点半、九点半 | 二、三、**四**、五、七、十点半 |

  `一吨半` 在表内而 `两吨半` 不在，同理。**后果**：用户看到同一表达因数值不同行为完全相反——`八点半`→全汉字，`四点半`→`4点半` 撕裂。Gavin 报的 `四点半` 只是露出水面的那一个。

- **根因**：`[protect.unit_collisions]` 1386 条（ITN-COLLISION-TYPEA-002）是**从词频表机器派生**的——高频组合被收录、低频的没有，于是**把一个规则性语法族切成了随机子集**。

- **决策**（2026-07-31 主控裁定）：

  1. **保护词表只承载「不可推导的专名与习语」**（三亚、一心一意、五代十国），**不得承载可由文法规则推导的表达**（`N点半`、`N<单位>半`、`N<单位>M`）
  2. 规则性语法族一律交由 ITN 文法引擎处理（DEC-037 甲/乙/丙型）
  3. **删除保护词条与文法上线必须成对交付**：先删会让 `八点半` 立刻变成 `8点半` 撕裂；先做文法不删词条则 `八点半` 永远走不到文法。两者必须同批次上线
  4. 后续任何机器派生词表落地前，**必须先做「是否切割了规则性语法族」的检查**

- **与 `[ITN-PREFIX-SHADOW-001]` 的关系**：该条目此前归纳两种失败模式（漏保护=输出损坏、误保护=优雅降级）。**本次新增第三种：撕裂**——`check_protection` 命中后只前移游标不锁定后续（`src/itn.rs:697-703`），语义单元后半段仍被独立转换，产出一半汉字一半数字。**主控 2026-07-30 写入的「误保护 = 优雅降级」结论需补充适用条件：仅在语义单元内不含其他可转数字时成立。**

- **决策时间**：2026-07-31

---

## DEC-039 · 提示词里的模式清单一律是「示例」而非「判据」，必须配语义兜底授权

- **背景**（Gavin 2026-08-03 端测 + 架构指示）：Gavin 说「建议从以下方面入手：**比如**英语学习要多读多背，**再比如**多听一些视频的节目，**还有就是**要多出去和别人交流」，场景 `msedge/Memos/kind=document/multiline_safe=true/f4_injected=true` 全部正常，却输出为单行段落，未走无序列表。

- **取证结论：四语标记词表健在，是被判据架空的**（主控 grep 实证：`比如说` 8 处、`たとえば` 6 处、`예를 들어` 6 处、`一方面…另一方面` 2 处，FORMAT-F3-UNIFY-I18N-012 的成果一字未丢）。

  真正拦住它的是 `src/llm/mod.rs:1040-1042` 的判据：

  ```
  DECISION RULE: a marker appearing ONCE signals a mere example — keep the text as a
  continuous paragraph; the SAME marker appearing in 2 OR MORE parallel items signals an
  enumeration — you MUST use a list.
  ```

  以及紧随其后的示例 `Chinese "比如..." (1) vs "比如说A，比如说B" (2+)`。

  逐条走通 Gavin 用例：`比如`×1 / `再比如`×1 / `还有就是`×1 —— **三个标记各不相同，没有任何一个出现 ≥2 次**，`the SAME marker` 判据不满足 → LLM 严格按规则判为「举例」→ 保持段落。**LLM 没有不听话，是规则把这种说法排除在外了。**

- **该判据的来源与失误**：`1b2697b`「修 F3b 无序列表在『比如说』式举例枚举下不触发」引入，目的是区分「举例」与「枚举」以防过度列表化 —— **目的正当，但选错了判定维度**：用「标记字面是否重复」代替「是否存在语义并列项」。而真实口语（尤其中文）**几乎从不重复同一标记**，自然表达就是递进变化的 `比如…再比如…还有…`。判据因此把最常见的枚举形态排除，同时让 30+ 条无序词表整体失效。

- **Gavin 的架构指示（本决策核心）**：

  > 「我们这种穷举特征词的方式，在实际使用场景中终归是有缺陷或者有覆盖不到的地方……要让大模型在既定的规则之外，也要根据自己的智能去分析输入的语言当中有没有符合有序和无序列表的语言特征，不能让模型只按照给定的死规则来判断，也要让它充分发挥它的智能空间。不然就会变成，只要用户的语言特征在我们的规则之外，我们就给写死了，识别不出来。」

- **决策**：

  1. **提示词中的任何模式清单（标记词、句式、语法族）一律定性为 ILLUSTRATIVE（示例），禁止定性为 EXHAUSTIVE（穷举判据）**。清单的作用是「校准 LLM 对该类模式的理解」，不是「限定可识别集合」
  2. **每一处模式清单后必须配一条语义兜底授权**，明确授权 LLM 用自身语言理解识别清单之外的同类模式
  3. **判据必须建立在语义关系上，不能建立在词汇字面上**。本例中正确判据是「是否存在 2+ 个并列项（同一句法角色、同一语义功能、共同构成一个集合）」，而非「同一标记是否重复」
  4. **兜底授权必须双向对称**：既授权「清单外也可识别为枚举」，也保留「并列不成立时不得列表」的反向护栏，防止从「漏识别」滑向「过度格式化」
  5. 本原则**适用于提示词全域**，不限于 F3 列表：ITN 保护、场景适配、格式规则等凡采用清单式表达处，均需回查是否把示例误用作判据

- **与 DEC-038 的关系**：DEC-038 讲的是**规则引擎侧**「词表不得承载可推导的语法族」；本条讲的是**提示词侧**「清单不得充当封闭判据」。两者是同一认知在确定性代码与概率模型两侧的镜像 —— **穷举法在两边都会把规则性/开放性的语言现象切成随机子集**。

- **决策时间**：2026-08-03（Gavin 指示，主控取证并归纳）

### DEC-039 补充 · Gavin 2026-08-03 对提示词模块设计定位的论断

> 「LLM 提示词模块的设计，需要有很高的架构设计能力，既要有规则约束，但也要充分利用 LLM 的高智能能力，**否则不如自己穷举规则来程序处理了**。」

**这句话给出了判断提示词设计好坏的检验标准**：如果一段提示词的效果，等价于把同样的清单写成 `if/else` 匹配，那这段提示词就是**失败的设计** —— 它花了 LLM 的推理成本，却只买到了正则表达式的能力。

**本项目的实证**：F3 的四语标记清单有 30+ 条无序标记（012 批次成果），但配上 `the SAME marker` 这个字面判据后，其行为**完全等价于一个字符串匹配程序** —— 而且比程序更糟，因为它还要付 4267 个 prompt token 的代价（见 debug.log:506）。Gavin 的用例正是撞在这个「退化为穷举匹配」的边界上。

**由此得出提示词模块的两层职责划分**：

| 层 | 承担什么 | 不该承担什么 |
| --- | --- | --- |
| **规则层**（清单、示例、few-shot） | 校准 LLM 对该类模式的**理解与尺度**（什么算枚举、多保守、输出什么形态） | ❌ 充当可识别集合的**边界** |
| **智能层**（语义兜底授权） | 处理清单覆盖不到的**开放集**，用语言理解做同类判断 | ❌ 无约束自由发挥（必须配反向护栏） |

**判断一条提示词该写进哪层的检验问题**：*「这条规则能不能用 100 行代码等价实现？」* 能 → 它属于规则层，且必须显式声明为示例；不能（需要语义理解/上下文推断）→ 它属于智能层，应当授权而非枚举。

**反向风险同样要防**：只给智能层不给规则层，会退化成「全凭模型发挥」，尺度不可控、跨版本不稳定 —— 这正是 F3 历史上反复出现「时而列表时而段落」的原因。**两层缺一不可，这就是 Gavin 所说「很高的架构设计能力」的具体含义。**

### 🔴 DEC-039 修正（Gavin 2026-08-03 当日纠正主控）—— 「加智能层就该减规则层」这条**作废**

**主控原写法（错误，已作废）**：

> 「**加了智能层，就该减规则层**。只加不减的话，prompt 迟早堆回到指令互相稀释的老问题上。」

据此，主控在 FORMAT-F3-SEMANTIC-021 中要求 coder-2 精简四语标记清单（删「语义自明的单词标记」如 `第一/第二`、`for instance`、`まず/次に`），把净增从 3347 压到 2198。

**Gavin 纠正原话**：

> 「LLM 系统提示词这一块啊，就是要列举常用的一些示例，这些是必不可少的，当然**越充分越好**。不用太在意提示词的大小，目前提示词的窗口、大模型的上下文窗口和返回的 token 长度，应该是能够**远远大于**我们的提示词程度的，所以提示词还是要充分一些好，这样才能够让大模型**学习到我们的诉求要求**，它才能够去做推导，能够发挥它的智能。」

**主控的误判在哪里（自我复盘）**：

1. **误读了 012 批次的教训**。主控引用 `logs/20260801.md`「连续两轮措辞层修复均失败，prompt_tokens +271 证明新文本已加载仍被压制」来支持「控制长度」—— 但那次的结论恰恰相反：**主控当时自己定论「根因是结构不是措辞」**。那条记录证明的是「加文本不解决结构问题」，**不是「文本多有害」**。用它来论证长度控制是断章取义。
2. **结构问题已由 018 分层解决**。优先级现在由层号决定而非 recency，「后段软化前段」的机制已经消失 —— 长度早已不是主要矛盾，主控还在用旧模型思考。
3. **示例的作用被低估了**。示例不只是「判据的枚举」，更是**让模型理解我们要什么的样本**。删掉 `第一/第二` 这类看似「语义自明」的词，等于减少了模型校准尺度的依据 —— 模型认得这个词，不等于知道我们希望它在这个词出现时做什么。

**修正后的原则**：

| 项 | 修正后 |
| --- | --- |
| 清单定位 | 仍是 **ILLUSTRATIVE 非 EXHAUSTIVE**（DEC-039 主条**不变**，兜底授权继续保留） |
| 清单规模 | **越充分越好**，不设长度预算约束。示例是模型学习诉求的样本，不是可精简的冗余 |
| 智能兜底 | 与清单**并存且互补**，不是替代关系 —— 兜底覆盖清单外的开放集，清单校准模型对已知模式的尺度 |
| 长度顾虑 | **不作为设计约束**。当前 prompt ~16600 字符 / ~4300 token，相对现代模型上下文窗口可忽略 |
| T4 长度预算测试 | 定位从「控制膨胀」改为「**探测异常暴涨**」（如意外重复拼接、循环注入），阈值应设得宽松，仅拦截数量级错误 |

**后续动作**：FORMAT-F3-MARKERS-023 —— 恢复 021 精简掉的标记词，并按「越充分越好」**主动扩充**四语有序/无序标记与 few-shot。

**决策时间**：2026-08-03（Gavin 纠正，主控记录并自我复盘）

---

## DEC-040 · 分层提示词的层间不得存在歧义：引用只能向上，裁决点必须唯一

- **Gavin 2026-08-03 原话**：

  > 「系统提示词里面要有结构化的判断，有优先级的这种判断规则，**但是不允许不同的层级和优先级之间存在歧义和判断模糊的情况**。」

- **背景**：DEC-039 落地后（021 判据语义化 + 023 标记清单扩充），Gavin 端测无序枚举仍不出列表。主控取证发现**根因不在措辞** —— Gavin 的原句逐字写在 `src/llm/mod.rs:1086` 的 Contrast 正向例里、并标注 `→ bullet list`，模型照样输出段落。真正的原因是 PROMPT-ARCH-018 分层时制造了一处**层间歧义**：

  | 位置 | 内容 | 层 |
  | --- | --- | --- |
  | `output_contract`（`:1171`） | 「多行**仅当 F3 applies 且 F3-item form 判为 LIST**；否则输出 single continuous paragraph」 | **L1** |
  | `f3_rules_text`（`:1035`） | F3 本体 —— 定义什么叫 applies | **L3** |
  | `META_RULE_PRECEDENCE`（`:242`） | 「组号小的赢，**绝对**，压过位置」 | 顶部 |

  即：**L1 的规则依赖一个定义在 L3 的条件，而元规则已宣布 L3 打不过 L1**。「F3 applies」不是可查表的事实而是需判断的事，加上 F3 自带「拿不准就别做列表」，任何犹豫都倒向 L1 的默认值「段落」。有序枚举（`第一/第二/第三`）零犹豫故能翻盘，无序枚举（`比如`）有判断余地故必败。

  **副作用之严重在于**：021/023 对 F3 内容的全部改进，在架构上不可能生效 —— 往一个被宣布必输的层里加内容，加多少都不改变结果。

- **决策（四条不变式，适用于提示词全域）**：

  1. **引用只能向上**：任一层的规则文本**不得依赖需要更低优先级层才能判定的条件**。引用更高优先级层（层号更小）合法；引用同层合法；**引用更低层非法**。必须引用时，两者归入同一层。
  2. **裁决点唯一**：同一个输出维度（行结构 / 输出标签 / 语气风格 / 数值形态）**只能由一层的一条规则裁决**，禁止两层各说一半。
  3. **默认值与例外条件同处**：不允许 A 层规定默认形态、B 层规定例外条件。默认与例外必须在同一条规则内成对出现。
  4. **禁止虚假自称权威**：规则不得自称「the SINGLE authority on X」，除非它确实是 X 的唯一裁决点。（现状违例：F3 段开头 `:1037` 仍自称是 lists/line structure/output tags 的唯一权威，但 018 已把后两者搬去 L1。）

- **可机械检查（本决策的落地形式，非口号）**：上述第 1 条可写成永久回归夹具 —— 扫描每条规则文本中对其它规则的显式引用（`F3` / `L0-1` / `L0-3` / `F3-item form` 等），断言 **被引用者的层号 ≤ 引用者的层号**。当前 `output_contract`(L1) → `F3`(L3) 会被该断言直接抓出。**同一断言还能顺带抓「悬空引用」**（引用了一条根本没注入的规则，如 020 修复前翻译路径的 `see L0-3`）。

- **与 DEC-039 的关系**：DEC-039 讲**规则内容**（清单是示例不是判据，必须配语义兜底）；本条讲**规则之间的结构关系**（层级不得制造歧义）。**两者是独立的失效维度** —— 本次事故证明：内容写到完美（用户原句逐字在场并标注期望输出），结构有歧义照样全盘失效。**结构问题不会被内容改进掩盖，只会把内容改进浪费掉。**

- **对 PROMPT-ARCH-025 的约束**：消融实验选出的方案，必须同时满足上述四条。若某方案实测有效但违反不变式（例如靠 recency 侥幸取胜而裁决点仍分裂），不得采纳。

- **决策时间**：2026-08-03（Gavin 提出原则，主控归纳为可检查的不变式）

### 🔴 DEC-040 勘误（2026-08-03 19:4x，PROMPT-ARCH-025 消融实验实测推翻主控归因）

**原文里有一处归因是错的，必须订正：本条背景段把「无序枚举不出列表」归因于层间歧义（`output_contract`(L1) 依赖 `F3`(L3)）。实测证明这个归因不成立。**

消融实验结果（coder-2，6 组变体 × 3 条真实失败输入 × 各 3 次 = 54 次真实 API 调用）：

| 变体 | 改动 | 三条输入出无序列表 |
| --- | --- | --- |
| V0 | 现状基线 | **0 / 9** |
| V1 | 删除 L1 的「默认单段落」表述 | **0 / 9** |
| V2 | F3 从 L3 提到 L1 | **0 / 9** |
| V3 | V1 + V2（主控主推方案 S1+S3） | **0 / 9** |
| V4 | 复现 012 结构（L1 契约与 F3 合并放最末） | **0 / 9** |
| V6 | 修正 F3 虚假自称权威 | **0 / 9** |

反向对照全部正常（C1 有序 3/3 出 `1. 2. 3.`；C2 单例保持段落；C3 短项顿号内联）→ 实验方法有效，非 harness 故障。

**结论**：**层间歧义不是本次 bug 的根因**（或至少不是充分原因）。按主控原判断直接改架构，会改动四处提示词结构而 bug 依旧。

**本条决策的四条不变式本身仍然成立** —— 它们是独立的正确性要求（引用只能向上 / 裁决点唯一 / 默认与例外同处 / 禁止虚假自称权威），只是**不该被用来解释这次的失败**。

**新的待验证假设（主控从实验数据反推，尚未验证，禁止当结论用）**：

- 有序列表成功时**标记词被保留**（C1 实际输出 `1. 第一点怎样怎样。`，「第一点」仍在项内）→ 做有序列表**不需要删任何字**
- 而 `src/llm/mod.rs:1119` F3c 期望的无序输出**删掉了「比如」「再比如」** → 做无序列表**必须删标记词**
- `L0_1_FIDELITY`（`:248`）要求「每个语义单元必须出现在 `<corrected>`」并自称「压过下面所有格式规则」；`L0_2` 追加「宁可别扭不可残缺」
- **V1–V6 六个变体没有任何一个动过 L0** —— 这精确解释了六组结果为何完全一致

→ 决定性测试：V7（F3c 无序示例改为**保留**标记词形态，如 `- 比如有些学生头发过长`）/ V8（临时放宽 L0-1 对话语标记的约束）。**根因确认前不得改生产提示词。**

**方法论教训（本次最值钱的部分）**：主控当时的静态取证非常"完整"——代码行号、元规则原文、012 与 018 的因果链、用户原句逐字出现在 few-shot 里——**看起来完全闭环，实测一枪打空**。提示词类问题的因果链只能由真实 API 消融实验确认，**静态阅读再严密也只是假设**。「先证伪再动手」这条纪律本次直接避免了一次无效的架构改动。

### DEC-040 后续（2026-08-03 结案）

三轮消融实验共 11 个假设全部证伪，**根因机制未定位**，Gavin 拍板挂起。完整的「已排除路径清单」与下次继续的接续点见 `troubleshooting.md` 的 **`[F3-UNORDERED-LIST-001]`** —— 再碰此问题前必读，避免重走 210 次 API 调用。

**对本决策的影响**：DEC-040 的四条不变式**继续有效**（它们是独立的正确性要求），但**不得再被用来解释无序枚举不出列表**。第 1 条「引用只能向上」的机器检查夹具仍然值得做 —— 它能抓的是**悬空引用与跨层前向依赖**这类真实缺陷（如 020 修复前翻译路径的 `see L0-3` 悬空引用，749 条测试全绿都没抓到），与本次的 F3 问题无关。

---

## DEC-041 · 格式优化输出必须由 LLM 决定，禁止程序化后处理

- **背景**：`[F3-UNORDERED-LIST-001]` 无序枚举不出列表的问题挂起后，2026-08-04 Gavin 端测再次命中（Notepad / `multiline_safe=true` / 提示词 21868 字符全注入 / `reasoning_tokens=None`，链路全对，LLM 仍返回整段）。主控据此建议第三条路：在 Rust 侧检测枚举标记重复出现后，对 LLM 结果做后处理强制切分为列表。

- **决策（Gavin 2026-08-04 否决主控建议）**：

  > 「格式优化输出必须要依赖 LLM，不能做程序化后处理，这是目前原则」

  1. 排版与格式（是否分行、是否用列表、用何种列表、标题、表格）的判定权**全部归 LLM**
  2. **禁止**在 Rust 侧对 LLM 输出做任何格式重排类后处理来"补救"模型没做到的排版
  3. 该原则适用于本项目全部格式相关需求，后续任何"代码兜底排版"类方案**不予立项**

- **边界（本决策不涉及的）**：既有的**安全与正确性**类后处理不受影响 —— `flatten_multiline`（`multiline_safe=false` 场景压平，防止把一条消息拆成多条发出）、`FMT-EMPTY-CORRECTED-001` 空结果护栏、`extract_corrected_tag` 标签解析等，它们处理的是注入安全与故障兜底，不是"替模型做排版决定"。

- **由此确立的攻坚方向**：既然不能从代码侧绕，剩下的变量只有**提示词**与**模型**。而 2026-08-03 的三轮消融实验（210 次 API 调用）经主控 2026-08-04 复查，**全部跑在 DeepSeek 家族**（`deepseek-v4-pro` 22 / `deepseek-v4-flash` 8 / `deepseek-chat` 8），**跨厂商从未验证过**。故「机制未明」这一结论存在一个未探测的盲区：**它可能是 DeepSeek 系的模型特性而非提示词缺陷**。本项目已有先例——`[LLM-COT-LEAK-001]` 中 `Qwen3.5-35B` 88 次 0 异常 vs `deepseek-v4-flash` 15 次 2 异常。

- **下一步**：Gavin 2026-08-04 决定切换 LLM 模型做端测。换模型属高风险变更（`[LLM-COT-LEAK-001]` 教训 4），换后须主动看一轮 `debug.log`。

---

## DEC-045 · macOS 侧浮层必须是独立窗口，与 Windows 侧实现一致，禁止用托盘图标代替

- **背景**：2026-08-04 macOS 侧 Phase 4 规划中，主控曾建议「录音窗口 / 处理中窗口降级为托盘图标状态，浮层整体不进第一版」，理由是 Win32 侧四个浮层共约 1,184 行 GDI，是 Phase 4 最大单块工作量。
- **决策（2026-08-04 Gavin 拍板，推翻主控建议）**：
  1. **录音必须有独立窗口**，**不能用托盘图标代替**——「不符合用户体验」
  2. **必须和 Windows 侧的实现一致**
- **适用范围**：与 Gavin 同日重申的「macOS 侧功能实现要参考 Windows 侧，做到尽量一致」一并适用于四个浮层（录音 / 处理中 / 失焦返显 / 错误信息）。**录音窗口为 P0**，其余三个按同一原则对齐。
- **影响**：
  - `MACOS-P4-OVERLAY-001` 从阶段 E（收尾）提到 **阶段 C（核心）**，与事件宿主同批
  - 原 `MACOS-P4-FEEDBACK-001`（托盘状态 + 系统通知替代层）**取消其"替代浮层"的定位**；系统通知若保留，只能是浮层之外的补充，不得作为浮层的替身
  - macOS 侧需引入 AppKit 窗口能力（`Cargo.toml:115-116` 预留的 `objc2` / `cocoa` 注释即为此准备）
- **Windows 侧影响**：**零**。本决策只新增 macOS 实现，不触碰任何 Win32 绘制代码（DEC-033 第 4 条红线，Gavin 2026-08-04 第三次重申）。
- **主控提取的 Windows 侧规格基线**（macOS 实现的对齐依据，全部实测自 `src/main.rs`）：

  | 项 | Windows 实测值 | 出处 |
  | --- | --- | --- |
  | 录音 / 落波 / 处理中 / 错误 窗口尺寸 | **240 × 36** | `:549` `:551` |
  | 失焦返显窗口尺寸 | **320 × 140** | `:553` |
  | 窗口定位 | **水平居中；底部上移 64px**（`x=(screen_w-w)/2`, `y=(screen_h-h-64).max(0)`） | `overlay_geometry:1731-1744` |
  | 窗口样式 | `WS_POPUP` + `WS_EX_TOOLWINDOW｜WS_EX_TOPMOST｜WS_EX_LAYERED｜WS_EX_NOACTIVATE` | `run_overlay_thread` 内 `CreateWindowExW` |
  | 圆角 | DWM 圆角，不可用时回落 `SetWindowRgn`；绘制圆角半径 `CORNER_RADIUS=10` | 同上 + `:1062` |
  | 透明度 | `SetLayeredWindowAttributes(LWA_ALPHA)`，alpha 可变 | 同上 |
  | 录音窗配色 | 背景 `#0D0F11`／边框 `#070606`／品牌橙 `#FF6B00`／故障红 `#FF0000`／静音灰 `#808080` | `draw_recording_overlay:1056-1060`（COLORREF 为 `0x00BBGGRR`，此处已转 RGB） |
  | 音频指示灯 | 18px 圆点，左边距 6px，垂直居中，4× 超采样抗锯齿 | `:1092-1094` |
  | 指示灯三态优先级 | **红（缓冲为空＝设备故障）> 橙（level>0.01 有音频）> 灰（设备正常但静音）** | `:1097-1114` |

  **macOS 对应物建议**（待 `MACOS-P4-OVERLAY-001` 任务书细化）：`NSPanel`（`.nonactivatingPanel` + borderless）≈ `WS_EX_NOACTIVATE`＋`WS_POPUP`；`window.level = .statusBar` ≈ `WS_EX_TOPMOST`；`collectionBehavior = [.canJoinAllSpaces, .stationary]`；`isOpaque=false` + `backgroundColor=.clear` ≈ `WS_EX_LAYERED`；不进 Dock / 不进 Mission Control ≈ `WS_EX_TOOLWINDOW`。

- **决策时间**：2026-08-04

---

## DEC-042 · 隐式补全的大单位数字保留锚定单位，不展开（三亿五 → 3.5亿）

- **背景**：`src/itn.rs:805` 的亿级隐式单位补全长期注释-实现不符（注释称「三亿五=350000000（五→五千万）」，实现乘 1000 得 `300005000`）。主控 2026-08-04 请 Gavin 在「按注释改成 350000000」与「按实现保持 300005000」之间拍板。

- **Gavin 2026-08-04 拍板：两个选项都不对**

  > 「三亿五，这个锚定的单位是亿，所以**不应该单位往后推算，直接输出 3.5亿**」

  主控原先在「展开成完整阿拉伯数字」这个框架内出题，方向就错了。用户口述的锚是「亿」，系统应保留该单位、用小数表达系数，而不是替他换一种记法。

- **与 DEC-037/026-B 的关系**：这是**同一条原则的第三次应用** ——

  | 决策 | 表现 | 共同原则 |
  | --- | --- | --- |
  | 026-B（方案 C） | `五块一斤` → `5块一斤`，不归一到元 | 用户明确用了某单位，系统不替他改写表达 |
  | Q5（2026-08-03） | 单价用 `元一斤` 而非 `元/斤` | 保留口述原始措辞 |
  | **DEC-042** | `三亿五` → `3.5亿`，不展开 | 锚定单位不往后推算 |

  三者同源，均与 L0-1 FIDELITY 不变式同向。

- **适用边界（Gavin 2026-08-04 逐项确认）**：

  | 类型 | 判据 | 输出 |
  | --- | --- | --- |
  | **隐式补全** | 万/亿后跟**孤立单字数字**，其后无任何单位 | **保留锚定单位**：`三亿五`→`3.5亿`／`两万五`→`2.5万`／`五万三`→`5.3万` |
  | **完整表达** | 单位说全了 | **照常展开**：`三亿五千万`→`350000000`／`两万五千`→`25000`／`一亿两千三百四十五万六千七百八十九`→`123456789`／`一千零四十六万八千七百四十一`→`10468741` |

  **万级与亿级一视同仁**（Gavin 明确选择一致性优先，接受「两万五」从 `25000` 变为 `2.5万` 的行为变更）。

- **实现落点（主控设计，待 027-D 实施）**：适用范围**恰好等于** 027-A 新增的守卫条件 —— `big_unit_seen && !zero_since_big && !unit_since_big` 命中的就是「隐式补全」场景。027-A 为修 bug 引入的边界判据，正好成为 DEC-042 的适用范围判据，无需新增识别逻辑。原 `section += digit * 1000` 改为按锚定单位格式化输出。

  需补一个「最近的大单位是万还是亿」的记录（函数注释里曾提及 `last_big` 但实现中不存在）。

- **影响**：`两万五`=25000 是 027-A 刚刚作为零回归项验收通过的断言，027-D 将**主动变更**它。届时该断言须同步更新，属设计变更非回归。

- **决策时间**：2026-08-04

---

## DEC-042 修订版（2026-08-04 当日两次修正，**以本节为准**）

> ⚠️ 上一节 DEC-042 的规则表述**已作废**，其「隐式补全 vs 完整表达」的二分是主控自行加的边界，Gavin 两次指出不成立。本节为最终规则。

### 一、修正过程（记录下来是因为主控连错两次，值得复盘）

| 轮次 | 主控给的框架 | Gavin 的纠正 |
| --- | --- | --- |
| 初次 | 在「展开成 350000000」与「保持 300005000」之间二选一 | **两个都不对**：「锚定的单位是亿，不应该单位往后推算，直接输出 3.5亿」 |
| 二次 | 按「有无隐式补全尾数」分界：`三亿五`→`3.5亿` 保留，`三亿`→`300000000` 展开 | **「三亿为什么要展开？不是已经锚定单位亿了吗？再展开不是画蛇添足吗？」** |
| 三次 | 问「三亿五千万」按字面规则应输出 `35000万` | 「**万层如果能被亿整除就升到亿**（→3.5亿）」 |

**主控两次错误的共同点**：都在「展开成完整阿拉伯数字」这个隐含前提下出题，而 Gavin 的原则从一开始就是**保留用户口述的单位**。出题框架错了，选项再多也选不出正确答案。

### 二、最终规则（Gavin 2026-08-04 定稿）

> **判定语言中明确提到的**最小**单位，输出展开到该层；比该层更小的成分用小数位表示。**

补充细则：

1. **单位后缀只用万/亿**（中文仅这两级有习惯后缀）。最小单位落在千/百/十/个时，输出普通阿拉伯数字
2. **万层可升亿**：万层数值若达到 1 亿，升级用亿表达（`三亿五千万` → `3.5亿`，不写 `35000万`）
3. **适用范围：全面适用**（Gavin 明确选 A），不限于「有省略尾数」的场景

### 三、行为对照表（含本次变更的既有行为）

| 输入 | 明确提到的最小单位 | 输出 | 相对 027-A/B/C 是否变更 |
| --- | --- | --- | --- |
| 三亿 | 亿 | **3亿** | 🔄 变更（原 300000000） |
| 三亿五 | 亿 | **3.5亿** | 🔄 变更（原 300005000） |
| 一亿 | 亿 | **1亿** | 🔄 变更（原 100000000） |
| 十亿 | 亿 | **10亿** | 🔄 变更（原 1000000000） |
| 三亿五千万 | 万 → 升亿 | **3.5亿** | 🔄 变更（原 350000000） |
| 一千二百三十四亿五千万 | 万 → 升亿 | **1234.5亿** | 🔄 变更（原 123450000000） |
| 两万 | 万 | **2万** | 🔄 变更 |
| 两万五 | 万 | **2.5万** | ✅ 027-D 已实现 |
| 两千三百四十五万 | 万 | **2345万** | 🔄 变更（原 23450000） |
| 五千万 | 万（不足 1 亿） | **5000万** | 🔄 变更 |
| 两万五千 | 千 | **25000** | 不变 |
| 一千零四十六万八千七百四十一 | 个 | **10468741** | 不变 |
| 一亿两千三百四十五万六千七百八十九 | 个 | **123456789** | 不变 |

### 四、实施影响（027-E，**尚未派发**）

- **范围远大于 027-D**：027-D 只动「隐式补全」分支，027-E 要动**所有含万/亿的解析路径**
- **回归基线要重建**：上表 🔄 行中有多条是 027-B/C 今天刚验收的断言（`一亿`/`十亿`/`两千三百四十五万`/`一千二百三十四亿五千万`），届时须按 DEC-042 修订版更新，属**设计变更非回归**
- 🔴 **契约风险扩大**：`parse_cn_number` 将**大量**返回带单位字符串（`"2345万"`）。027-D 的「孤立判定」（后继有单位则展开）可挡住货币/重量场景（`两万五千元`→`25000元`），但**非货币场景的影响面须重新盘点**
- **待实测边界**：`一万亿`（最小单位亿 → `10000亿`？现状 `1000000000000`）／`两万五百`／`十万个为什么`类专名

### 五、状态

⏸ **027-E 暂停派发** —— Gavin 2026-08-04 指示：账号模型额度用尽，等额度重置后再派，听候指令。

---

## DEC-043 · 三套「数字+单位」转换逻辑并存，暂不统一到全局原则

- **背景**：DEC-042 确立数量级层「锚定最小明确单位」规则后，Gavin 2026-08-04 追问「所以现在数字+单位的转换，是要锚定最小单位来转换是吗？」主控核查后指出：该规则**只覆盖数量级层**，项目里实际并存三套方向不同的逻辑。

- **三套逻辑现状**：

  | 类别 | 规则 | 判据 | 实例 | 出处 |
  | --- | --- | --- | --- | --- |
  | **数量级**（万/亿） | 锚定**最小**明确单位 + 小数位 | 最小单位 | `三亿五`→`3.5亿`／`两千三百四十五万`→`2345万` | DEC-042（2026-08-04） |
  | **货币**（元/块/角/毛） | 单段保原单位，多段**归一到最大单位**（元） | **段数** | `五块`→`5块`／`一块两毛二`→`1.22元` | 026-B 方案 C（2026-08-03 Gavin 拍板） |
  | **重量**（斤/两） | **全部单位逐段保留**，不合成 | 全保留 | `一斤二两`→`1斤2两`／`三斤六两五`→`3斤6两5` | 017 `format_weight_chain`（2026-08-03） |

  **三者方向互不相同**：数量级取最小单位、货币归一到最大单位、重量全保留。用 DEC-042 去套货币会得到 `12.2毛`，套重量会得到 `12两`，均与现行实现相反。

- **决策（Gavin 2026-08-04）**：

  > 「保留三套并存，暂时不同归到全局」

  1. **三套逻辑各管一摊，互不适用**。DEC-042 的「锚定最小单位」**仅适用于数量级层（万/亿）**，不得外推到货币或重量
  2. 🔴 **任何 Agent 不得以「三者不一致」为由自行统一** —— 这不是缺陷，是三次独立的、经 Gavin 逐条拍板的设计选择
  3. 027-E 实施 DEC-042 时**只动数量级层**，`format_currency_chain` / `format_weight_chain` 一行不碰

- **为何暂不统一（主控评估，Gavin 采纳）**：

  - 货币归一到元是 Gavin 2026-08-03 亲自拍板（方案 C），当时判据是**段数**：「单段说明用户明确用了某个货币单位，系统不替他改写表达；多段才有合成数值的必要」。与 DEC-042 的最小单位判据**并不冲突，只是各管一摊**
  - 重量逐段保留是 017 修 P0 时定的（零乘法拼接），改动会碰 Gavin 端测验证过的买菜用例
  - 统一属**架构级改动**，须先出方案再动手，不适合塞进当前批次

- **将来若要统一**：单开设计任务，先产出方案交 Gavin 拍板，**不得在 bug 修复或功能任务中顺手统一**。

- **决策时间**：2026-08-04

---

## DEC-042 补完 · 量级分界与升亿阈值（2026-08-04 定稿，**027-E 实施依据**）

> 本节补完 DEC-042 修订版的两处未定边界，与修订版合并构成完整规则。Gavin 2026-08-04 确认：「认同你现在的这种做法，确实简洁易读」。

### 一、完整规则（四条）

1. **判定语言中明确提到的最小单位**
2. **量级分界在万**：最小单位落在 **万/亿** → 带单位后缀输出；落在 **千/百/十/个** → 输出普通阿拉伯数字
   （理由：中文仅万、亿两级有习惯后缀写法，`3000` 没人写成 `3千`）
3. **升亿阈值**：万层数值 ≥1 亿 **且** 升亿后**小数位 ≤1 位** → 升到亿；否则保持万层
4. 比最小单位更小的成分，用小数位表示

### 二、单一量级对照（从亿到个）

| 输入 | 最小明确单位 | 输出 |
| --- | --- | --- |
| 三亿 | 亿 | `3亿` |
| 三千万 | 万 | `3000万` |
| 三百万 | 万 | `300万` |
| 三十万 | 万 | `30万` |
| 三万 | 万 | `3万` |
| 三千 | 千 | `3000` |
| 三百 | 百 | `300` |
| 三十 | 十 | `30` |
| 三 | — | `3` |

### 三、混合量级与升亿判定

| 输入 | 数值 | 万层表达 | 升亿后 | 小数位 | 最终输出 |
| --- | --- | --- | --- | --- | --- |
| 三亿五千万 | 350000000 | 35000万 | 3.5亿 | 1 | ✅ **`3.5亿`** |
| 一千二百三十四亿五千万 | 123450000000 | 12345000万 | 1234.5亿 | 1 | ✅ **`1234.5亿`** |
| 一亿两千三百四十五万 | 123450000 | 12345万 | 1.2345亿 | 4 | ❌ 不升 → **`12345万`** |
| 一亿零三万 | 100030000 | 10003万 | 1.0003亿 | 4 | ❌ 不升 → **`10003万`** |
| 三千五百万 | 35000000 | 3500万 | 不足 1 亿 | — | **`3500万`** |
| 三十五万 | 350000 | 35万 | 不足 1 亿 | — | **`35万`** |
| 三亿五 | 350000000 | — | 本就在亿层 | 1 | **`3.5亿`** |
| 三万五千 | 35000 | 最小单位是千 | — | — | **`35000`** |
| 一千零四十六万八千七百四十一 | 10468741 | 最小单位是个 | — | — | **`10468741`** |
| 一亿两千三百四十五万六千七百八十九 | 123456789 | 最小单位是个 | — | — | **`123456789`** |

### 四、「整除」一词的澄清

Gavin 原话是「万层如果能**被亿整除**就升到亿」。严格数学意义上 `3.5` 并非整除，故该词取「**能用亿简洁表达**」之意，落地判据为**小数位 ≤1 位**（本节第一条第 3 款）。

### 五、027-E 实施约束（与 DEC-043 合并遵守）

- **只动数量级层**。`format_currency_chain` / `format_weight_chain` 一行不碰（DEC-043）
- 上表中多条是 027-B/C 已验收断言（`一亿`/`十亿`/`两千三百四十五万`/`一千二百三十四亿五千万`），届时按本节更新，属**设计变更非回归**
- 🔴 契约风险：`parse_cn_number` 将大量返回带单位字符串，027-D 的「孤立判定」须覆盖扩大后的范围

---

## DEC-042 补充二 · 万亿层（2026-08-04 Gavin 拍板）

> **Gavin 原话**：「一万亿肯定是要遵循中文的习惯，不能够把它全转成数字。」

### 一、问题

027-E 落地后 `一万亿` 输出 `10000亿`（规则直接推论：最小明确单位是「亿」）。但中文习惯说「一万亿」，写作「1万亿」，`10000亿` 不符合语感。

### 二、决策：量级扩展为三级，逐级升

| 层级 | 数值 | 升级条件 |
| --- | --- | --- |
| 万 | 1e4 | — |
| 亿 | 1e8 | 万层 ≥1 亿 **且** 升亿后小数 ≤1 位 |
| **万亿** | **1e12** | **亿层 ≥1 万亿 且 升万亿后小数 ≤1 位** |

**与既有「万层升亿」完全同构**，只是多一级。升级阈值统一为**小数 ≤1 位**。

### 三、行为对照

| 输入 | 最小明确单位 | 逐级升 | 输出 |
| --- | --- | --- | --- |
| 一万亿 | 亿 | 10000亿 → 1 万亿（0 位小数） | **`1万亿`** |
| 三万亿 | 亿 | 30000亿 → 3 万亿 | **`3万亿`** |
| 一万五千亿 | 亿 | 15000亿 → 1.5 万亿（1 位） | **`1.5万亿`** |
| 十亿 | 亿 | 不足 1 万亿 | **`10亿`**（不变） |
| 一千二百三十四亿五千万 | 万 → 亿 | 1234.5亿，不足 1 万亿 | **`1234.5亿`**（不变） |
| 三亿五千万 | 万 → 亿 | 3.5亿 | **`3.5亿`**（不变） |

**只影响达到 1 万亿的表达**，1 万亿以下行为一律不变。

---

## DEC-044 · 《十万个为什么》类专名加入保护白名单

> **Gavin 原话**：「《十万个为什么》，这应该要加入白名单，这是一个常用的一个书的名字，或者说一个日常的一个俗语，也不应该转成数字。」

### 一、问题

027-E 落地后 `十万个为什么` → `10万个为什么`，**书名被改写**（tester-1 在 TEST-SYNC-027 走查中发现并上报）。

### 二、决策：加入 `[protect.proper_nouns]`

**这不违反 DEC-038**，恰恰是 DEC-038 第 1 条明确规定该词表**应当**承载的那一类：

> 保护词表只承载「**不可推导的专名与习语**」（三亚、一心一意、五代十国），不得承载可由文法规则推导的表达（`N点半`、`N<单位>半`、`N<单位>M`）

《十万个为什么》是**书名（专名）兼日常俗语**，「十万」在此不表数量而是固定表达的一部分，**无法由任何文法规则推导**，因此走词表是正确路径。

### 三、与「加词表打补丁」的区别（重要，防止将来被误引用）

| 情形 | 判定 |
| --- | --- |
| `五毛钱` 在表、`三毛钱` 不在表 | 🔴 **错误用法** —— `N毛钱` 是能产语法族，词表把它切成随机子集（DEC-038 病症），应交文法处理（P6 批次） |
| `十万个为什么` 加入词表 | ✅ **正确用法** —— 专名，不可推导，词表是唯一手段 |

**判据**：该表达能否由文法规则推导？能 → 交文法；不能 → 走词表。

### 四、落点

`itn-rules.toml` 的 `[protect.proper_nouns]`（人工组），**不是** `[protect.unit_collisions]`（机器派生组）。
## DEC-046 · macOS 事件宿主改用 objc2 + NSApplication + CFRunLoop 直驱（**DEC-015 复议，其"以 Tauri 为宿主"作废**）

- **背景**：DEC-015（2026-04-19）定「macOS 用 Tauri 作事件主机，不引入 objc2/NSRunLoop」。**该决策的前提到 2026-08-04 已不成立**：
  1. 主程序 `feiyin-ime` 至今**不依赖 tauri**（实测根 `Cargo.toml` 无 tauri 依赖，tauri 只在 `src-tauri/` 的 Settings UI 子进程内）。照 DEC-015 原文执行 = 为借一个 run loop 把整个 WebView 运行时拖进主程序
  2. **DEC-036 已要求四个浮层必须是独立窗口**，AppKit 窗口能力成为硬需求 → `objc2` 无论如何都要引，再套一层抽象纯属冗余
- **主控对「winit 是不是 macOS 的事件循环机制」的澄清（Gavin 2026-08-04 追问）**：**不是。** winit 是 Rust 的**跨平台**窗口/事件循环库，在 macOS 上通过 `objc2` 驱动 NSApplication。macOS 的原生机制是 **`NSApplication`（AppKit）持有主线程 run loop + `NSRunLoop`/`CFRunLoop` + `NSEvent`**。
- **决策（2026-08-04 Gavin 拍板「事件宿主选型按你建议方案来」）**：**objc2 + NSApplication + CFRunLoop 直驱**，不引入 winit / tao / 完整 Tauri。
- **理由**：
  1. **与 Gavin「参考 Windows 做到尽量一致」总纲最贴**，结构一一对应：

     | Windows 侧 | macOS 对应物 |
     | --- | --- |
     | 隐藏 controller 窗口（DEC-001） | `NSApplication` |
     | `GetMessageW` 主循环（`main.rs:2531`） | `NSApplication.run()` / CFRunLoop |
     | 15ms `WM_TIMER` 轮询（`:2519`） | `CFRunLoopTimer` |
     | `WM_APP_HOTKEY_EVENT` / `WM_APP_PIPELINE_EVENT` 唤醒 | `CFRunLoopSource` signal |

  2. **仓库已有 CFRunLoop 可工作先例**：`src/platform/macos/hotkey.rs`（DEC-017）在独立线程上建 CFRunLoop 驱动 CGEventTap（`:259` `CFRunLoop::get_current()`、`:220` `.stop()`）
  3. `core-foundation 0.10` / `core-graphics 0.25` 已是既有 macOS 依赖，无需新增
  4. `Cargo.toml:115-116` 预留的 `objc2` / `cocoa` 注释原文即标注 "Phase 4 NSWindow transparent overlay"，与本决策同源
  5. winit 的价值是跨平台，但**我们只会在 macOS 用它**（Windows 侧按红线不动），跨平台性在此收益为零
- **⚠️ 待验证的唯一风险点**：`tray-icon 0.19` 在 macOS 上要求有活跃的 NSApplication，其官方示例基于 winit/tao。**自建 NSApplication 时它能否正常工作需实测**，已列为 `MACOS-P4-HOST-001` 的第一道验证关卡。**若实测不通，退回 winit 作为兜底**，届时另立决策记录。
- **`cargo tree -i winit` 实测存档**：winit 0.30.13 确在依赖树内，但来源是 `eframe 0.29.1 → voice-ime`，而 `eframe`/`egui` 全仓**唯一使用者是 `src/crash/reporter.rs`**（crash-reporter 的 GUI），与 tray-icon 及管线代码无关。
- **Windows 侧影响**：**零**。Windows 继续走 Win32 消息循环，本决策只新增 macOS 实现。
- **决策时间**：2026-08-04

---

## DEC-047 · 「是否含标点」的判据全子系统统一：词内嵌豁免，不另立「句子级标点」分类

**背景**（PUNCT-GOVERNANCE-030-A-2，2026-08-08）

`src/transcription/mod.rs:279` 长期把 Qwen3 在线 ASR 的 `native_punctuated` **硬编码为 `true`**。主控核实阿里云官方文档后确认：`qwen3-asr-flash-realtime` 的 `session.update` 确无任何标点参数（只有 `modalities` / `input_audio_format` / `sample_rate` / `input_audio_transcription.{language,corpus}` / `turn_detection`），**源头关不掉成立**；但「模型必定输出标点」是假设而非事实 —— 短句/单词场景常无尾标点。假设为假时 `main.rs:3569` 的 `!native_punctuated` 门控会**跳过标点引擎**，用户开着自动标点开关却拿不到标点。

改为实测后暴露判据问题：直接用 `punctuation::is_punctuation` 扫描，`3.14` 的小数点会被判成标点。

**两个候选方案**

| | 主控初版 | coder-2 反提案（**采纳**） |
| --- | --- | --- |
| 思路 | 新增 `has_sentence_punctuation`：全角标点即判定；半角终结符仅在句末位置（后接空白或结尾）才算；引号括号一律不算 | 新增 `has_effective_punctuation`：对 `PUNCT_CHARS` **全集合**统一施加「词内嵌豁免」—— 标点字符两侧都是 ASCII 字母/数字则视为嵌入字符，不计；任一无夹持的标点出现即 true |
| 同族覆盖 | 分类式，按字符类别列举 | 一条规则同时豁免 `3.14` 的 `.`、`don't` 的 `'`、`3:30` 的 `:` |

**决策：采纳 coder-2 方案。**

**原因**（主控被说服，原方案撤回）：

1. **不引入第二套分类**。初版凭空造出「句子级标点 vs 字符级标点」两个概念，本质是在同一子系统里维护两套「什么算标点」的定义。
2. **检测器与剥离器口径统一**（决定性理由）。`strip_punctuation` 本就有 ASCII 夹持保护，采纳后两个函数对同一字符给出同一答案；初版会让检测器说「不是标点」而剥离器说「是」，子系统内口径分裂 —— 这正是本次 030 治理批次要消除的东西。
3. 同族覆盖等价，实现更小。

**已知接受边界**（记录在案，不得当 bug 重修）：

| 输入 | 本决策口径 | 初版口径 | 裁定 |
| --- | --- | --- | --- |
| `他说“好”`（中文引号，无句号） | `true` → 跳过标点引擎 | `false` → 走引擎补句号 | **接受本决策口径** —— 引号无句号不是用户会报的缺陷，两方案真实撞到的三个同族均已覆盖 |
| `https://…` | `:` 右侧为 `/` 单侧夹持 → 计为标点 | 同 | 接受，与 `strip_punctuation` 对 `:` 的保守口径一致，语音转写出 URL 概率近零 |
| `３.１４`（全角数字） | 两侧非 ASCII → 不豁免 → `true` | — | 只报不改，与半角行为不一致，优先级低 |

**适用范围限制**：本轮 `has_effective_punctuation` **只在 `transcription` 的 Qwen3 分支使用**，不替换其他调用点（扩大改动面会让零回归验证失去判别力）。`accuracy native`（`:439`）与 `performance`（`:446`）两分支的标记各有依据，不动。

**跨平台**：`src/transcription/mod.rs` 与 `src/punctuation/mod.rs` 均为平台中立模块，macOS 编译同一份代码，`run_pipeline_core` 已去平台化 → **两端同时生效**，须记入 `docs/MACOS-HANDOFF.md`。

**决策时间**：2026-08-08

---

## DEC-048 · 阶段三（TEST-SYNC）开命令白名单：只放 `cargo fmt` + `cargo check`

**背景**（2026-08-09 Gavin 拍板）

五阶段测试工作流中，阶段三 TEST-SYNC 原本**禁止执行任何命令**，`cargo check` 也在禁止之列。
后果是 tester-1 **交付前不可能自查** —— 连括号配没配对都无从知道。

**2026-08-08 PUNCT-GOVERNANCE-030 一批之内，同一根因引爆三次，且逐次升级**：

| # | 任务 | 后果 |
| --- | --- | --- |
| ① | TEST-SYNC-030 | 代码非 rustfmt-clean，被后续 coder 的 `cargo fmt` 连带归一，污染两个 commit 的 diff |
| ② | 同上 | 同上 |
| ③ | TEST-SYNC-030-B | `src/llm/mod.rs:4776` 多一个 `}`，**整个 crate 编译失败**，靠主控提交前 `cargo check` 才发现 |

**决策：开白名单例外，只允许 `cargo fmt` 与 `cargo check`（含 `--all-targets`）两条命令。**

**原因**

1. **与规则要防的风险无关**。阶段三禁令要防的是「测到半成品、拿到假结果」与「提前出包」。
   `cargo fmt` 只格式化、`cargo check` 只做类型检查，**都不执行测试、不产出二进制**。
2. **禁止它们直接导致交付物编译不过** —— 规则本身在制造它要防的那类问题。
3. 代价近零：`cargo check` 约 13–50s。

**边界（写成白名单而非「允许只读命令」的理由）**

| 允许 | 禁止 |
| --- | --- |
| `cargo fmt` | `cargo test`（含任何过滤参数） |
| `cargo check` / `cargo check --all-targets` | `cargo build` / `cargo build --release` |
| | 其余一切执行类命令 |

**只有上面两个命令名可用，不做推广解释。** 若写成「允许只读命令」，
「我只是 check 一下」会滑向「我顺手跑个测试」，口子守不住。

**连带修正**：项目级 `.claude/CLAUDE.md` 第 3 节此前写作「三阶段」且**阶段一为并行**
（代码任务与 TEST-SYNC 同时派发），与 `worker-guide.md` 2026-05-05 起执行的
「五阶段禁止并行」**长期冲突**。本次一并改正为五阶段全串行，以该节为准。

**落地位置**：`.claude/CLAUDE.md` §3 ｜ `collab/docs/worker-guide.md` §二-3
｜ `collab/docs/task-book-template.md` 常备红线表

**决策时间**：2026-08-09

---

## DEC-049 · F3 列表路由保持「吃不准就不列表化」，不为边界项目形态增设细分规则

**日期**：2026-08-14 ｜ **决策人**：Gavin ｜ **触发**：`TEST-PROMPT-IND-034` 机制定位完成后

### 背景

Gavin 端测发现「你可以去菜场看一看…比如有没有新鲜的青椒，有没有新鲜的西瓜，
还有没有比较新鲜的香蕉…」不输出为无序列表。经 `TEST-PROMPT-AB-033`（24 次调用）
与 `TEST-PROMPT-IND-034`（18 次调用）两轮实测，机制已定位到**代码侧**：

F3 有两道闸（`src/llm/mod.rs:1219-1233`）：

```
闸一（是不是枚举）：If unsure, DO NOT use a list — keep the text as a continuous paragraph.
                    Over-formatting normal speech into lists is a regression.
闸二（是枚举后按项目形态分流）：
  - SHORT（名词/短语：无谓语、无内部标点、≤6 字）→ 顿号内联，DO NOT make a list
  - LONG （带谓语或内部标点的完整子句）        → 做列表
```

**三类项目形态 → 三种确定行为，实测完全自洽**：

| 项目形态 | 归属 | 模型行为 | 是否符合规则 |
| --- | --- | --- | --- |
| `新鲜的青椒`（5 字名词短语） | 明确 SHORT | 逗号→顿号内联 3/3 | ✅ 正确 |
| `有些同学下课之后徘徊在校园里不走，三五成群`（主谓齐全+内部标点） | 明确 LONG | 拆 bullet 列表 3/3 | ✅ 正确 |
| `有没有新鲜的青椒`（8 字，**无主语疑问谓词短语**） | **两边都不像** | 既不列表也不加顿号 9/9 | ⬅️ 落进定义空隙 |

**模型全程执行正确**，它在边界项目上「什么都不做」，正是闸一的保守兜底在命令它这么做。

### 决策

**保持现状，不修改 F3 规则。** 吃不准的一律不走列表化处理。

Gavin 原话：「**先保持现状：吃不准的不走列表化处理，避免规则过分精细化造成易失误和交叉误判**」。

### 原因

1. **两类错误代价不对称**。规则原文已写明「Over-formatting normal speech into lists is a
   regression」—— 把正常叙述误切成列表，是用户**每句话都可能撞上**的高频伤害；
   而漏切一句边界枚举，只是少一次锦上添花。**保守兜底是刻意设计，不是疏漏。**
2. **规则精细化的边际收益递减、边际风险递增**。要覆盖「无主语疑问谓词短语」这一类，
   就得在 SHORT/LONG 之外再立判据；而自然语言的边界形态无穷无尽，每加一条细分规则，
   就多一处与既有规则**交叉误判**的可能。这与 `DEC-039`（模式清单是示例不是判据）、
   `DEC-040`（裁决点必须唯一、层内不得存在歧义）同向。
3. **闸一与闸二的兜底本就相反**（闸一「不确定→段落」，闸二「mixed 不确定→列表」），
   若不动闸一而只补闸二，会把这处已知矛盾进一步复杂化。
4. **已有可靠规避方式**：改说「第一/第二/第三」，有序枚举跨全部实验、跨两个模型型号
   **从未失败过一次**。用户侧零成本。

### 影响

- `src/llm/mod.rs` 的 F3 规则**不改动**，`TEST-PROMPT-IND-034` 以「机制已定位、决策不修复」结案
- `[F3-UNORDERED-LIST-001]` 从「机制未明」转为 **「机制已明，按 DEC-049 刻意不修」**，条目关闭
- 原计划的 D5 确认实验（把项目改成有主语完整子句）**不再执行** —— 机制已足够支撑决策，
  再测只增加文档完整性，不改变结论
- **后续若有人再提「无序枚举不出列表」，先读本条**：这不是 bug，是既定取舍

### 未纳入本决策的遗留

- **模型漂移**（`TEST-PROMPT-AB-033` G0：同提示词同模型同采样参数，08-03 IN-B 3/3 →
  08-14 0/3）是**独立问题**，本决策不涉及。是否上回归看门测试待定
- **场景块是正向变量**（同日同模型：无场景块 IN-B 0/3、含场景块 3/3）机制未明，
  本决策不涉及

---

## DEC-050 · 流式上屏走 overlay 浮层预览，**否决 TSF 组合文本（TIP）路线**

**日期**：2026-08-14 ｜ **决策人**：Gavin ｜ **依据**：`RESEARCH-TSF-036`

### 背景

换用 `qwen-audio-3.0-asr-flash-streaming` 流式 ASR 后，目标是
「**边录边显示，最终格式优化输出**」。Gavin 初始设想用**「组合文本」（Composition Text）**
模拟系统输入法的预输入效果 —— 在目标应用光标处显示带下划线的未提交文本，
松开热键后整段送 LLM 格式化，再正式注入。

**该设想的架构优点成立**：组合文本是「未提交」态，替换它不算重写已提交内容，
用户心智与系统输入法一致，且 **LLM 仍拿整段优化，F3 跨句能力完整保留**。

### 可行性调研结论（主控已用微软官方文档独立复核）

**跨进程插入 TSF 组合文本，必须注册成活动 TIP（Text Input Processor），
且用户必须把系统输入法切换到飞音。** 官方原文（[TSF Architecture](https://learn.microsoft.com/en-us/windows/win32/tsf/architecture)）：

> "A text service is implemented as a **COM in-proc server** that registers itself with TSF.
> **When registered, the user interacts with the text service using the language bar or keyboard shortcuts.**"
>
> "A text service **never interacts directly with an application**. All communication passes through
> the TSF manager. The TSF manager is implemented by the operating system and **cannot be replaced**."

绕开路径亦全部堵死：`ImmSetCompositionString`（IMM32）同样要求飞音是活动 IME；
UI Automation 的 `TextPattern` / `ValuePattern` 是辅助功能 API，只能读写**最终文本**，无组合态。

**顺带纠错**：Gavin 引用的「VoxType 在 Windows 上用此机制」不成立 ——
[VoxType](https://voxtype.io/) 是 Linux/macOS 项目；Windows 上的近名项目
（[cubhe/VoiceType](https://github.com/cubhe/VoiceType)、微软商店 Voice Type）
均为「松开后粘贴到活动输入框」，非 TSF 组合文本。**无现成 Windows 参考实现。**

### 决策

**走 overlay 浮层预览（方案丙），否决 TIP 路线。** 并**复用现有录音 overlay 窗口**。

### 原因

1. 🔴 **崩溃影响面不可接受**。TIP 是 in-proc COM 组件，会被加载进
   Word / Chrome / VS Code。**飞音一个 panic 可能带走用户正在写的文档**。
   当前形态下飞音崩了只需重启托盘，两者风险量级完全不同。
2. **产品形态被迫改变**。绿色 exe 免安装保不住，须改为安装包 + 注册表注册 + 代码签名，
   且 32/64 位各出一套（老应用加载 32 位 TIP）。
3. **附带工程沉重**。TIP 必须做**全键盘透传**，否则用户切到飞音后无法正常打字。
4. **交互倒退**。用户必须先把系统输入法切到飞音才生效，与现有「托盘常驻 + 全局热键即用」
   的零摩擦体验冲突（DEC-004）。
5. **收益有限**。Gavin 的目标表述是「边录边**显示**」，overlay 完全满足；
   TIP 的增量收益仅为「预览位置从浮层挪到光标处」，与上述代价不成比例。

### 影响

- **不注册 TIP，不引入 TSF 依赖**，`src/platform/windows/` 无新增 COM 组件
- 预览改由**现有 overlay 窗口**承载，交互设计见 `DESIGN-OVERLAY-037`
- **最终注入路径不变**：松开热键 → 整段送 LLM 格式化 → 注入到 `target_hwnd`
  —— 与现有管线一致，`F3` 跨句格式化能力**不受任何削弱**
- **PoC 不做**：核心问题已由官方架构文档直接回答，PoC 只能重复验证已知答案

### 🔴 由本决策引出的新设计约束（`DESIGN-OVERLAY-037` 处理）

overlay 现以 `WS_EX_NOACTIVATE` 创建（`src/main.rs:632`），**刻意不能获焦** ——
因为 `target_hwnd` 在热键按下时用 `GetForegroundWindow()` 抓取（`:1907`/`:2909`），
overlay 抢焦点会使注入失去目标。

而 Gavin 要求「用户可在 overlay 中键入光标并手动编辑」，**这必须能获焦**。
两者对撞，解法与降级（尤其**目标为管理员权限进程时 UIPI 限制导致无法 `SetForegroundWindow`**）
由 037 给出。

### 未来若要重提 TIP 路线

**先读本条**。除非以下前提之一发生变化，否则不重开调研：
① Windows 提供了 out-of-proc 的 TIP 宿主机制；② 产品定位主动改为「真·输入法」并接受宿主崩溃风险。

---

## DEC-051 · 流式 ASR 走真流式（边录边发），伪流式提案被否决

**日期**：2026-08-15　**拍板**：Gavin　**提出异议方**：coder-1（已主动撤回）

### 背景

`ASR-038-A` 交付的 `transcribe_streaming` 实为**伪流式** ——
录完整段 → 一次性发 → 服务端流式返回时回调推增量。
`038-B` 实施前，coder-1 提出异议：建议 B 批**沿用伪流式**，不做真流式。

其理由是工程风险：真流式需重写音频采集核心 + 双线程 + 重验 `FIRSTCHAR-FIX` 时序，
而 `FIRSTCHAR-FIX-001~006` 是六轮迭代才把首字率从 ~20% 压到 ~54% 的硬骨头，回归代价高。

**该异议在时间上晚于 Gavin 于 2026-08-14 亲自推翻「录完整段再发」前提的指示**，
故不属于常规方案协商，须回到 Gavin 拍板。

### 决策

**维持真流式。伪流式方案否决。**

Gavin 2026-08-15 原话：「需要流式的效果就是**边说边上屏**，一定要有这个交互体验。」

### 原因（三条，按分量排序）

**① 伪流式会静默废掉 DEC-050 与 037 的核心交互（决定性理由）**

037 的核心是 Gavin 亲自拍板的「**录音中用户点击 overlay 文本区即进入编辑态，不等松开热键**」。
伪流式下**录音期间屏幕上无任何文本**，没有可点击对象 —— **该交互物理上不成立**。

选伪流式等于同时作废 037 的编辑态设计与 DEC-050 的一半，
但**没有任何任务书会写明「本次放弃编辑态」** —— 它是悄悄消失的。
这正是 `feedback_no_patch_design` 要防的「盘不全产出源」的同构形态：
盘不全的是**下游依赖**。

**② 伪流式先天放弃最大的一块连接延迟杠杆**

同日 Gavin 另有指令：「要优化好连接池、要优化好连接的速度和性能。」

| 方案 | 建连成本落点 | 用户感知 |
| --- | --- | --- |
| 真流式 | 建连与**说话并行**，DNS+TCP+TLS 被说话时间吸收 | 零感知 |
| 伪流式 | 建连在**录音结束之后**，整条 RTT 落在关键路径 | 干等 |

**③ 异议方的回归风险估计偏高**

任务书要求 `record_streaming()` 与现有 `record()` **并存、不改其行为**；
`audio/mod.rs:101` 的 `record()` 签名自成一体，新增平行方法不需要动它。
`FIRSTCHAR-FIX` 六轮的成果在旧路径上，**旧路径不动就不回归**。

coder-1 复核后接受此点并**主动撤回异议**（协商第 2 轮结案，未用满 3 轮）。

### 诚实记录的代价（不掩盖）

- 真流式**确实更贵、更易出问题**：两条路径共用 `ensure_stream` 与设备状态，且新增双线程时序。风险**不是零**，只是量级为「新增一条隔离的平行路径」而非「重写采集核心」。
- 伪流式当时**已可跑通**（端点三缺陷刚修完），选真流式意味着 v0.8.0 首包延后。
- 主控曾提出「先出伪流式包验证底层 + 并行开真流式」的折中，**Gavin 未采纳**。

### 副产品：本次异议的正向价值

coder-1 的异议本身**提得对** —— 顺着这一问翻出了 `ASR-038-A` 的**端点三缺陷**
（`mod.rs:303` 用 Realtime 端点跑 Inference 协议 / `qwen_asr_url` 全库零消费 /
`default_qwen_asr_url` 主机名缺 WorkspaceId），任一条都足以让新 ASR 连不上。
**若无此次架构质疑，三条会一路带进出包。**

> **方法论教训（主控侧）**：A 批验收查了四条硬红线，唯独没查端点接线 ——
> 因为代码注释写着「URL 在 config 层区分」而主控采信了注释。
> **新增配置项必须 `grep` 消费点，光看定义与注释不算数。**

### 关联

`DEC-050`（overlay 否决 TIP）｜`037` overlay 流式预览设计｜`038` 流式管线设计｜
`ASR-PERF-040`（连接性能门禁）｜`DEC-052`（本地 VAD 计费门控）

---

## DEC-053 · 测试验收通过后主控直接派发出包，不再逐次请示 Gavin

**日期**：2026-08-16 ｜ **决策人**：Gavin ｜ **原话**：「测试任务完成验收后，直接派发出包任务」

### 背景

此前主控的实践是：阶段四测试验收通过后，**停下来向 Gavin 请示「现在可以出包吗」**，
等他回一句才派发 BUILD 任务（`todo.md:194` 旧文「阶段五，主控须请示 Gavin 后执行」即此）。

2026-08-16 v0.8.0 第二批（ASR-042 + ASR-045）跑完阶段三/四后，主控又一次停下请示，
Gavin 直接改规则。

### 决策

**阶段四测试验收通过（③ 类真回归 = 0）后，主控直接派发阶段五出包任务，无需请示 Gavin。**

| 环节 | 变更前 | 变更后 |
| --- | --- | --- |
| 阶段四 → 阶段五 | 主控停下请示 Gavin，等回复 | **主控直接派发 BUILD** |
| 主控 → tester-1 | 主控须明确下达「现在可以出包」 | **不变**（tester-1 仍不得自行发起构建） |
| `git push` | 须 Gavin 明确指示 | **不变**（外发操作，仍须明示） |

### 原因

出包不是外发操作 —— 产物只落到本机 `Publish/`，Gavin 端测才是下一步。
请示环节没有拦住任何风险（真正的闸门是阶段四的 ③ 类真回归清点），
只是把 Gavin 变成了流水线上的一个人工 gate，**违背他一贯的「防止我忘了」诉求**
（同 `CLAUDE.md` 自动提交规则的动机：不把流程记忆挂在他脑子里）。

### 边界（不要过度推广）

- **仅适用于「阶段四已通过」这一条路径。** 阶段四未跑或有 ③ 类红条 → 不许出包，也不许请示了事
- **`git push` 不在本决策范围内**，仍须 Gavin 明确指示。2026-08-04 曾有 Worker 幻觉出一条
  「出包后 push」的授权，教训犹在
- **版本号仍不得擅改**（见项目级 `.claude/CLAUDE.md` 约束 2）
- 出包 Step 1 会强杀 Gavin 正在使用的输入法进程，**须在完成通知里提醒他重启**

### 关联

`worker-guide.md` 五阶段工作流 ｜ `DEC-048`（阶段三命令白名单）｜ `CLAUDE.md` 自动提交规则

---

## DEC-054 · 流式文字上屏节奏走「时间戳驱动回放」，否决固定速率抖动缓冲

**日期**：2026-08-17（Gavin 拍板）／2026-08-18 补记 ｜ **决策人**：Gavin
**原话**：「时间戳驱动才是应该选方案」「停顿和用户讲话的节奏一致才对」

### 背景

在线流式 ASR 上屏观感「一顿一顿」。调查已完结（见 `todo.md` 卡顿专节，结论不要重查）：
**服务端每约 1000ms 推一批、每批 3-4 字**（实测间隔 786/1036/1105/1246ms），
客户端调不动 —— 官方 9 个参数无一控制推送频率，Manual 模式控的是断句不是中间结果。
建连 103ms、首字 338ms、逐帧实时发送，均已排除。

主控先后三次提「固定速率抖动缓冲」类方案，三次被 Gavin 纠正。

### 决策

用 `words[].begin_time` 按**用户真实语速**回放：

1. **时间戳驱动**，不用固定速率打字机
2. **不得压缩停顿** —— 不设词间间隔上限/下限，不做 backlog 追赶加速
3. 几百毫秒**固定**延迟可以接受
4. `words` 不可用（缺字段/空数组）→ **退回立即显示**，不许换另一套平滑算法顶上

### 原因

固定速率的本质是把语音的时间信息丢掉再造一个假的：用户停顿 2 秒被压成 350ms，
「读起来的节奏」与「说出来的节奏」脱钩，观感是另一种别扭，并没有解决问题。
`words[]` 里已经带着真实节奏，直接用即可。

### 实施红线（三个坑，2026-08-18 主控查出）

| # | 坑 | 处理 |
| --- | --- | --- |
| ① | `begin_time` 相对「发出去的音频流起点」，我们攒 1.5s pre-roll 一次性补发，**不是**按键时刻 | 只用 `begin_time` **差值**（减首词），偏移自动消掉，**不去算绝对锚点** |
| ② | 服务端 `max_sentence_silence=800ms`，停顿超 0.8s 即断句开新 `sentence_id` | words 与 `display_text` **同源累积**（`StreamingAsrState` 内 `confirmed_words`/`current_words`），每次下发全量对齐词表，overlay 侧整体替换 |
| ③ | 官方 `words[]` 还有 `end_time` 与 `punctuation` | 一并提取；缺字段降级（`end_time`→`begin_time`，`punctuation`→空串），**不整条丢弃** |

### 待实证

`words=N` 的真实取值需 Gavin 跑一次 `-debug` 确认（日志探针 coder-1 已埋）。
在此之前算法参数按上述契约实现，**不预设经验常数**。

### 关联

`DEC-050`（overlay 浮层预览）｜ `DEC-051`（真流式）｜ `OVERLAY-051-G` 任务链

## DEC-055 · overlay 绘制层从 GDI 迁移到 Direct2D + DirectWrite（否决「GDI+ 局部打补丁」）

**拍板人**：Gavin，2026-08-30。主控给出 A/B 两案，Gavin 选 **B**。

### 背景

Gavin 2026-08-30 端测提交 13 项问题，其中三项**同源**，全部卡在 GDI 渲染管线的能力上限：

| ID | Gavin 原话 | 卡在哪 |
| --- | --- | --- |
| OVERLAY-062 | 点击进入编辑态，文字字体显示很粗糙，提交按钮的边沿也很粗糙 | 文字抗锯齿 + 图形抗锯齿 |
| OVERLAY-065 | 替换左侧麦克风图标为动态图标 | 逐帧动画的重绘成本 |
| OVERLAY-069 | 窗口关闭要流畅丝滑，缩短最后消失的过程，不要生硬突然关闭 | 淡出动画需要高频低成本重绘 |

### 取证（主控 2026-08-30 独立查证，非推测）

1. **分层窗口禁用 ClearType**：overlay 窗口以
   `WS_EX_LAYERED`（`src/main.rs:1114`）创建，并在 `:1297` 调用
   `SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA)`。
   Windows 在带 alpha 的分层窗口上**关闭次像素（ClearType）渲染**，文字最多拿到灰度抗锯齿。
   这就是 Gavin 说的「字体显示很粗糙」。

2. **GDI 图形不做抗锯齿**：`src/main.rs` 中 18 处 `RoundRect` 调用
   （`:2066` / `:2143` / `:2248` / `:2474` / `:2638` / `:2715` / `:2774` / `:2897` / `:2943` / `:3018` / `:3027` 等）
   全部走 GDI 的整像素栅格化，**没有任何抗锯齿开关**。圆角与按钮边沿必然是硬像素阶梯。
   这就是 Gavin 说的「提交按钮的边沿也很粗糙」。

3. 🔴 **OVERLAY-054-D 修的不是这个问题**：054-D 给 EDIT 控件补了 `WM_SETFONT`
   （`src/main.rs:681-690`，`state.edit_font` 独立 HFONT）。那**是一个真 bug**
   （控件此前根本没设字体，退回点阵字体），修得对、代码现在也确实在跑。
   但它只解决「压根没设字体」，**解决不了管线层的糙**。
   2026-08-30 Gavin 复报 062 时明确说「我重复提交的问题就是之前没有修复好」，
   根因至此才定位到管线层。**教训：视觉类问题不能只查资源句柄，要一路查到栅格化管线。**

### 被否决的方案 A：GDI+ 局部打补丁

用 GDI+ 的 `Graphics::SetSmoothingMode(SmoothingModeAntiAlias)` 替换若干 `RoundRect` 调用点。

**否决理由**：
- 只能救图形边沿，救不了分层窗口下的文字渲染（文字仍走 GDI/ClearType 禁用路径）
- 救不了 065 / 069 的动画需求（GDI+ 仍是 CPU 栅格化 + 位图搬运，逐帧成本高）
- 等于在 GDI 上打第三次补丁（前两次是 FIX-006-1 边框调暗、054-C 边框提亮），
  最后仍要重来 —— **Gavin 明确不接受这种反复**

### 决策：方案 B —— Direct2D + DirectWrite 重写 overlay 绘制层

| 维度 | 结论 |
| --- | --- |
| 图形 | Direct2D，`D2D1_ANTIALIAS_MODE_PER_PRIMITIVE`，圆角/圆形/按钮边沿全部平滑 |
| 文字 | DirectWrite，`IDWriteTextFormat` + `D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE`，不受分层窗口 ClearType 禁用影响 |
| 合成 | 与现有 `WS_EX_LAYERED` + `LWA_ALPHA` 兼容路径待实施时定；若需 per-pixel alpha 则走 `UpdateLayeredWindow` + D2D DXGI 表面 |
| 动画 | ~~D2D 硬件加速重绘，为 065（动态图标）与 069（淡出）提供逐帧低成本重绘基础~~ 🔴 **此条与 P0 实际实现不符，见「DEC-055 补充二」**：P0 走的是 DC render target（每帧 BindDC 到 GDI 表面），拿不到硬件加速收益。当前不换路线，触发判据见补充二 |
| 平台 | Direct2D 是 Win7+ API，满足 DEC-000（Win10/11），无兼容性风险 |

### 影响范围（🔴 派发前必须逐项评估，不得遗漏）

| 受影响面 | 说明 |
| --- | --- |
| `src/main.rs` 全部 `draw_*` 函数 | 录音态 / 流式文字态 / 编辑态 / 处理中 / 错误态 / 焦点丢失态，**六种状态全部要迁** |
| overlay 消息循环 | `WM_PAINT` 路径、`needs_repaint` 脏标记（OVERLAY-043）、`interpolate_step` 尺寸插值（043-B）都要重新接线 |
| 字体缓存 | `create_clear_type_font` / `cached_font` / `state.edit_font` 三套 HFONT 生命周期管理要重构为 DirectWrite 对象 |
| EDIT 子控件 | 编辑态用的是**真 Win32 EDIT 子窗口**，不是自绘。D2D 迁移**不能顺手把它也重写**，否则 051-A/051-D 的子类化、Enter 转发、横向滚动全部要重做 —— 本批**明确不动 EDIT 控件本身** |
| 现有单测 | 颜色常量断言（`OVERLAY_BORDER_GRAY` / `OVERLAY_BTN_BORDER` 的测试区镜像值）、`compute_edit_box_geometry` 契约、`interpolate_step` 护栏 |
| E2E | `tests/utils/state_detector.py` 按 `src/main.rs` 正则解析尺寸常量；窗口尺寸若变，E2E 判据同步失效 |
| 跨平台 | Direct2D 是 Windows 独有。macOS 侧 overlay 走独立实现（DEC-045 独立窗口），**必须在 `docs/MACOS-HANDOFF.md` 记明本决策不适用于 macOS，且 Windows 侧绘制契约已换代** |

### 红线

1. 🔴 **不得顺手改窗口尺寸**。066（高度 +3px）是独立任务，有它自己的连锁评估（`overlay_geometry` / `state_detector` 正则 / E2E 断言），**不许混进 D2D 迁移批**。
2. 🔴 **不得重写 EDIT 子控件**。见上表。
3. 🔴 **不得动 `InvalidateRect` 的 `bErase=false`**（OVERLAY-043 红线）、
   **不得动 `STREAMING_STOPPED` 门闩语义**（043 红线）、
   **不得合并 `run_pipeline_core` 第二层 `text.trim().is_empty()` 分支**（ASR-045 红线）。
4. 🔴 **迁移必须可分状态灰度**：先迁一个状态跑通并由 Gavin 目视确认，再迁其余，
   **禁止六状态一次性全改**（一次全改则回归归因不可能）。
5. 🔴 视觉结果**必须 Gavin 目视确认**，不接受静态代码论证结案
   （`feedback_ui_visual_verification` + 本次 054-D 的教训）。

### 与既有决策的关系

- **DEC-003**（录音悬浮层原生 Win32 overlay / GDI 绘制）：**窗口宿主不变**，仍是原生 Win32 overlay；
  **仅绘制后端从 GDI 换成 Direct2D**。DEC-003 的「原生 Win32」部分继续有效，「GDI」部分由本决策取代。
- **DEC-045**（macOS 侧浮层独立窗口）：不受影响，macOS 侧不走 Direct2D。
- **DEC-050**（流式上屏走 overlay 浮层预览，否决 TSF）：不受影响，预览载体不变。

---

### DEC-055 实施附录（主控 2026-08-30 补，派发 D2D 批前的地基勘测）

#### 前提一：`windows` crate 当前**没有** Direct2D/DirectWrite feature

`Cargo.toml:91-107` 现有 15 个 feature，与绘制相关的只有
`Win32_Graphics_Gdi` 与 `Win32_Graphics_Dwm`。**Direct2D / DirectWrite / DXGI 一个都没有。**

迁移前必须补：`Win32_Graphics_Direct2D`、`Win32_Graphics_DirectWrite`，
若走 `UpdateLayeredWindow` + DXGI 表面路线还需 `Win32_Graphics_Dxgi`、`Win32_Graphics_Direct3D11`。

🔴 **这是依赖变更，不是纯代码改动**：
- 按 worker-guide 第十节，`Cargo.toml` 依赖变更**必须派发 BUILD 给 tester-1**，coder 不得自行构建
- 首次编译会显著变慢（新增 Windows API 绑定），要给 tester-1 预期
- ~~macOS 侧 Cargo.toml 共用，需核清 feature 在 Windows target 段之内还是之外~~ —— **2026-08-30 主控已核实排除**：`Cargo.toml:90` 即 `[target.'cfg(target_os = "windows")'.dependencies]`，`windows` crate（`:91`）本就在该段内，加 Direct2D feature **天然不影响 macOS 构建**，无需额外评估

#### 前提二：绘制有唯一入口，灰度迁移可行（好消息）

全部 13 个绘制函数都收敛在单一入口 **`draw_overlay_to_dc`（`src/main.rs:1959`）**，
且**全部以裸 `HDC` 为参数**：

| 分类 | 函数 |
| --- | --- |
| 入口 | `draw_overlay_to_dc:1959` |
| 通用件 | `draw_text:366`、`draw_overlay_chrome:2089` |
| 录音态 | `draw_recording_overlay:2386`、`draw_recording_indicator_and_waveform:2120`、`draw_recording_indicator:2543`、`draw_stop_button:2338` |
| 流式文字态 | `draw_recording_overlay_with_text:2406`、`draw_listening_placeholder:2633`、`draw_submit_button:2494` |
| 编辑态 | `draw_editing_overlay_chrome:2657` |
| 处理中 | `draw_processing_overlay:2785` |
| 预览 | `draw_preview_overlay:2912` |
| 错误态 | `draw_error_overlay:3104` |

**意义**：DEC-055 红线 4 要求「分状态灰度迁移，禁止六状态一次性全改」——
单一入口让这件事真正可行：在 `draw_overlay_to_dc` 里按状态分流，
已迁移的状态走 D2D、未迁移的继续走 GDI，两套并存直到全部迁完。
**派发时必须要求 Worker 采用这个结构，而不是原地替换。**

#### 建议的迁移顺序（等 Gavin 逐个目视确认）

| 序 | 状态 | 理由 |
| --- | --- | --- |
| 1 | **处理中 `draw_processing_overlay`** | 最简单（无波形、无文字滚动、无子控件、无插值），是验证 D2D 管线是否接通的最小闭环；出问题影响面最小 |
| 2 | **错误态 `draw_error_overlay`** | 同上，结构近似，可复用第 1 步搭好的资源管理 |
| 3 | **录音态 `draw_recording_overlay` + 波形** | Gavin 的 OVERLAY-065（动态麦克风图标）落在这里；波形是逐帧动画，正是 D2D 的主场 |
| 4 | **流式文字态 `draw_recording_overlay_with_text`** | 最复杂：涉及横向滚动、字宽度量、051-G 时间戳回放。放最后 |
| 5 | **编辑态 `draw_editing_overlay_chrome`** | 🔴 只迁**自绘边框部分**；EDIT 子控件是真 Win32 控件，DEC-055 红线 2 明令不动 |

**OVERLAY-062（字体/边沿粗糙）在第 1 步就能得到部分验证** ——
处理中浮层也有圆角与文字，Gavin 看第一版就能判断 D2D 的观感是否达标，
不必等全部迁完才知道方向对不对。

#### 与另外两项优化的关系

- **OVERLAY-063（字号再大一号）**：D2D 用 DirectWrite 的字号语义与 GDI 的负数字号
  （当前 `OVERLAY_FONT_SIZE = -14`）**不是一回事**。迁移后再定字号，
  否则先在 GDI 上调了、迁完还得重调。**排在迁移之后。**
- **OVERLAY-066（窗口高度 +3px）**：与绘制后端无关，但会连锁
  `overlay_geometry` / `tests/utils/state_detector.py` 正则 / E2E 尺寸断言。
  **可以独立于 D2D 先做，也可以迁完再做**，但**绝不能混在 D2D 批里**（DEC-055 红线 1）。

---

### DEC-055 补充二 · P0 走的是 DC render target，**没有拿到「硬件加速」那份收益**（2026-09-04 主控实读代码补记）

**起因**：Gavin 2026-09-04 端测确认处理中态效果满意后问：「D2D 在效果和性能上是比之前用的 GDI 更强吗」。
主控读代码后发现，**效果与性能是两个不同的结论**，而本决策正文第 4 行写的
「D2D 硬件加速重绘，为 065（动态图标）与 069（淡出）提供逐帧低成本重绘基础」
**与 P0 的实际实现对不上**。故补记，避免后人照着立项理由去找硬件加速却找不到。

#### 效果：确实更强，且这是迁移的真实收益

| 维度 | GDI | D2D/DirectWrite | 差距 |
| --- | --- | --- | --- |
| 图形抗锯齿 | **完全没有**（18 处 `RoundRect` 整像素栅格化，无开关） | `D2D1_ANTIALIAS_MODE_PER_PRIMITIVE` | **巨大**，Gavin 说的「细腻」主要来自这里 |
| 文字抗锯齿 | 灰度 | 灰度 | **无差别** —— 见下方澄清 |
| 字形定位 | 整像素对齐 | **亚像素定位**（字形可落在小数 x） + 更好的 gamma 校正 | 中等，体现为字间距更均匀 |

🔴 **一处必须澄清的常见误解**：D2D **没有把 ClearType 找回来**。
overlay 是 `WS_EX_LAYERED` + 带 alpha，Windows 在这类窗口上禁用次像素渲染，
**GDI 与 DirectWrite 都只能拿到灰度抗锯齿**（代码 `:2883` 显式设的就是
`D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE`）。文字变好看是「更好的灰度栅格化 + 亚像素定位」，
不是「ClearType 回来了」。**别拿这个去承诺文字锐度**。

#### 性能：当前路线**不比 GDI 强，每帧还多一点开销**

`src/main.rs:2867`：

```rust
let rt: ID2D1DCRenderTarget = factory.CreateDCRenderTarget(&rt_props)?;
```

**DC render target**，每帧 `BindDC` 绑到现有的 GDI 内存 DC 上绘制。
代码注释把动机写得很清楚：

> GDI-compatible surface, so the existing double-buffered WM_PAINT (mem_dc → BitBlt)
> keeps working with **zero structural change**

**这是刻意选的低风险试点路线，选择本身是对的** —— 只替换「怎么画」，不替换「画到哪」，
不动 WM_PAINT / OVERLAY-043 脏标记 / 尺寸插值任何结构。P0 能一次跑通、Gavin 一次目视通过，
很大程度上就是因为这个选择。

**但代价要说清**：输出必须落进 GDI 表面，即便 D2D 内部动用了 GPU，
结果也要传回系统内存再 `BitBlt`。**拿不到「GPU 直接合成到屏幕」那份收益**，
反而多了每帧 `BindDC` / `BeginDraw` / `EndDraw` 的开销。
`D2D1_RENDER_TARGET_TYPE_DEFAULT`（`:2855`）虽写着「有硬件就用硬件」，
也改变不了目标表面是 GDI DC 这个事实。

另注 `:2858` `alphaMode: D2D1_ALPHA_MODE_IGNORE` —— **无 per-pixel alpha**。
（对 OVERLAY-069 不构成障碍：窗口用的是 `LWA_ALPHA` 整窗透明度，淡出直接调 alpha 即可。）

#### 对 065 / 069 的实际影响（**这才是本补充存在的意义**）

正文承诺的「逐帧低成本重绘基础」，**当前 DC 路线并不提供**。
若 OVERLAY-065（动态麦克风图标）或 OVERLAY-069（收缩淡出）做出来发现帧率不够，
需要再走一次迁移：`UpdateLayeredWindow` + DXGI 表面，或改用 HWND render target。
那需要补 `Win32_Graphics_Dxgi` + `Win32_Graphics_Direct3D11` 两个 feature —— **属依赖变更**，
按 worker-guide 第十节必须派 BUILD 给 tester-1，coder 不得自行构建。

#### 🔴 主控的判断：现在**不换**，等实测数据

overlay 最宽也就千把像素、高 36。这么小的表面，**软件渲染在绝对值上依然很便宜**，
60fps 压不垮。所谓「性能不如预期」是相对的，实际大概率感知不到。

**决定：先按现有 DC 路线把 P1/P2/P3 迁完、把 065/069 做出来。
真出现帧率不够再换路线** —— 那时有实测数据支撑；现在换等于凭猜测提前上难度，
且会把 P0 已验证的「零结构改动」优势一次性丢掉。

~~**触发换路线的判据（写死，免得将来靠感觉拍）**：
065 或 069 实现后，动画期间实测帧率 < 30fps，或 Gavin 端测明确反馈卡顿/掉帧。~~

🔴 **本判据已于当日作废，见 DEC-056**（Gavin：「效果一定好，视觉体验一定要棒」）。
作废原因不是帧率数字定错了，而是**这条判据把 per-pixel alpha 当成了性能优化 —— 定性就错了**。
它实际是视觉能力的天花板问题：`LWA_ALPHA` 整窗统一透明度下，
圆角要么留背景楔形、要么被 `SetWindowRgn` 二值掩码切成硬阶梯，**两条路都到不了「棒」**。
与帧率无关，再快的机器也解不开。本节其余对 DC render target 性能特征的描述仍然成立。


## DEC-057 · D2D 剩余迁移 P2+P3 合并为一批（部分放宽 DEC-055 红线 4，Gavin 2026-09-04 拍板）

**起因**：Gavin 看到排期后问「还要再走三轮端侧？」——主控评估后认为可压到两轮，他拍板「按两轮走」。

### 一、原排期与新排期

| | 原（DEC-055 红线 4：分批灰度） | 新（本决策） |
| --- | --- | --- |
| ① | 086 三缺陷 + D2D-P1 → 端测 | **不变**（马上出包） |
| ② | P2（Recording / FallingToProcessing / Error）→ 端测 | **P2 + P3 合并**（剩余五态一次迁完）→ 端测 |
| ③ | P3（StreamingEditing / FocusLost）→ 端测 | per-pixel alpha（DEC-056）→ 端测 |
| ④ | per-pixel alpha → 端测 | — |

**这一包之后由三轮端测压到两轮。**

### 二、为什么现在可以放宽（红线 4 防的问题已被 P1 解决大半）

红线 4 的原文理由是「一次全改则回归归因不可能」。**但 D2D-P1 不只迁了两个状态**，
它把共用层与五个绘制原语抽了出来：`chrome` / `mic_indicator` / `stop_button` /
`placeholder_text` / `streaming_text`，外加 `with_d2d` 帧封装与 `D2DERR_RECREATE_TARGET` 处理。

P2/P3 剩余五态**主要是复用这批现成原语**，真正新写的只有三个：波形（Recording）、
提交键（StreamingEditing）、复制/关闭键（FocusLost）。

**归因难度反而降低**：真出问题大概率在共用原语层，而那层是共用的 ——
一个状态坏则多个状态一起坏，特征明显、好定位；不像 P1 那样每个原语都是首次落地。

**P1 是开路的那一批，P2+P3 是铺路的。**

### 三、🔴 不放宽的部分（这条是硬的）

**per-pixel alpha（DEC-056）必须单独一批，不许并进 P2+P3。** 两条理由：

1. 它不是「再迁一个状态」，而是**换掉整个合成方式**（`UpdateLayeredWindow` 取代
   现有 WM_PAINT + `LWA_ALPHA` 模型）——性质与状态迁移完全不同。
2. 它**必须等八态全部迁完**才能做：残留任何 GDI 绘制都会在预乘 alpha 位图上打出
   alpha 洞（GDI 不维护 alpha 通道，写过的像素 alpha 被置 0，合成时整块透明消失）。

把它和五个状态迁移捆在一起，**才是真正会导致「归因不可能」的那种捆法**。

### 四、被否决的更激进方案

主控评估过「这一包也不出、等 P2+P3 做完一起端测」= 压到两轮总数不变但更早合并。**否决**：

- Gavin 报的三个缺陷（尤其他明说「体验非常差」的宽度抖动）已修好，让他多等一批才能验不值当
- 万一 P2/P3 带出新问题，会把这三个修复的验证一起搅浑

**要压就压 P2+P3，不压这一包。**

### 五、DEC-055 红线 5 不受影响

每批出包后**视觉效果仍必须 Gavin 目视确认**，不接受静态代码论证结案。
本决策放宽的是**分批粒度**，不是验收标准。

### 与既有决策的关系

- **DEC-055 红线 4**：本决策部分放宽其分批要求，理由见第二节；红线 1/2/3/5 全部继续有效
- **DEC-056**：不受影响，per-pixel alpha 仍是独立批次，且前置条件（八态全迁完）不变


## DEC-056 · 视觉效果优先于性能顾虑；overlay 走 per-pixel alpha 路线（Gavin 2026-09-04 拍板）

**拍板人**：Gavin，2026-09-04。原话：

> 现在硬件配置已经不用介意这些小的窗口效果带来的负载了。
> **效果一定好，视觉体验一定要棒，这关乎到用户体验**，现在硬件性能很强大，不会卡帧残影的。

### 一、这条决策推翻了什么

`DEC-055 补充二` 定的换路线判据是「动画期间实测帧率 < 30fps 才考虑换」——
那是把 `UpdateLayeredWindow` + per-pixel alpha 当成**性能优化**，所以保守。

**Gavin 明确否决这个优先级排序**：视觉效果是产品目标，不是性能预算的余数。
该判据**作废**，不再以帧率作为是否升级渲染路线的门槛。

### 二、🔴 更要紧的是：主控把这条路线的性质判断错了

它**根本不是性能优化，是视觉能力的天花板问题**。主控实读代码确认：

| 项 | 现状（`src/main.rs`） |
| --- | --- |
| 扩展样式 | `WS_EX_LAYERED`（`:1130`） |
| 合成方式 | `SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA)`（`:1341`）—— **整窗统一透明度** |
| `UpdateLayeredWindow` | **全仓零调用** |
| 绘制 | 双缓冲 `mem_dc` → `BitBlt`（`:1883` / `:1940`） |

`LWA_ALPHA` 意味着**窗口矩形内每个像素的 alpha 完全相同**。所以圆角只有两种可能：

| 做法 | 结果 |
| --- | --- |
| 角上照常绘制 | 圆角描边之外露出背景色楔形 = **OVERLAY-086 Bug 1 的本体** |
| `SetWindowRgn` 圆角裁剪 | region 是**二值掩码、无抗锯齿** → 圆角变成硬像素阶梯 = **正是 DEC-055 要消灭的「粗糙」** |

🔴 **两条路都到不了「棒」。这是当前架构的天花板，不是实现没写好。**
D2D 把曲线画得再平滑，最后一步 `LWA_ALPHA` 合成时也会被打回原形 ——
**边缘像素没有独立 alpha，就不可能与桌面背景做真正的融合。**

### 三、per-pixel alpha 解锁的是效果，不是速度

改用 `UpdateLayeredWindow` + 32bpp 预乘 alpha 位图后：

| 能力 | 现在 | 之后 |
| --- | --- | --- |
| 圆角边缘 | 楔形残留 或 硬阶梯 | **真抗锯齿**，边缘像素按覆盖率与桌面融合 |
| 投影 / 柔和外发光 | 做不了 | 可做 |
| 淡出（OVERLAY-069） | 整窗均匀变淡（能做，但边缘仍硬） | 边缘随之柔化，收缩过程更干净 |
| 与桌面的贴合感 | 像贴上去的一块矩形 | 像悬浮的一个形状 |

### 四、🔴 顺序约束：必须先迁完全部 D2D 状态

**任何残留的 GDI 绘制都会在 per-pixel alpha 位图上打出 alpha 洞** ——
GDI 的绘制函数（含 `DrawTextW`）不维护 alpha 通道，会把写过的像素 alpha 置 0，
在 `UpdateLayeredWindow` 合成时表现为**该区域整块透明消失**。

所以顺序是硬的，不可调换：

```
OVERLAY-086（三缺陷，Bug 1 只能做到「消除楔形」，圆角仍非真抗锯齿）
  → D2D P1 / P2 / P3（八个状态全部迁完，GDI 绘制清零）
  → per-pixel alpha 改造（UpdateLayeredWindow，本决策）   ← Bug 1 的彻底解
  → OVERLAY-065 动态图标 / OVERLAY-069 收缩淡出
```

**这也是 D2D 迁移「尽快做完」的新理由**：它不再只是为了字更清楚，
而是 per-pixel alpha 的前置条件。迁移没完成，视觉天花板就打不破。

### 五、依赖影响（待实施时实测确认，勿凭本节结论直接派发）

主控倾向判断：**可能不需要新增 Cargo feature**。
思路是创建 32bpp DIB section → `ID2D1DCRenderTarget` 以
`D2D1_ALPHA_MODE_PREMULTIPLIED` 绑其 DC → D2D 全程绘制（不掺 GDI）→ `UpdateLayeredWindow` 推送。
现有 `Win32_Graphics_Direct2D` + `_DirectWrite` 已足够，
`Win32_Graphics_Dxgi` / `_Direct3D11` **未必需要**。

🔴 **但这是推断不是实证。** 派发前必须实测验证；若确需新增 feature，
属依赖变更 → 按 worker-guide 第十节**必须派 BUILD 给 tester-1**，coder 不得自行构建。

### 六、验收（沿用 DEC-055 红线 5）

视觉结果**必须 Gavin 目视确认**，不接受静态代码论证结案。
本决策的验收判据尤其明确：**圆角边缘在浅色背景上不得看出阶梯或色块**。

### 与既有决策的关系

- **DEC-055**：不冲突，本决策是它的下一步。DEC-055 换掉「怎么画」，本决策换掉「怎么合成上屏」。
- **DEC-055 补充二**：其「帧率 < 30fps 才换路线」的判据被本决策**作废**。
  补充二对 DC render target 性能特征的技术描述仍然成立、继续有效。
- **DEC-003**（原生 Win32 overlay）：窗口宿主仍不变，仅合成方式改变。
- **DEC-045**（macOS 独立窗口）：不受影响，macOS 侧不走此路线。


---

## DEC-056 补充一 · 处理中态圆角灰线：不回退软边，直接等 per-pixel alpha（Gavin 2026-09-05 拍板）

### 背景

Gavin 2026-09-05 端测 BUILD-093/098 期间截图反馈：**处理中态窗口边沿灰线仍然「粗乱」**。

**这不是回归，也不是漏做。** OVERLAY-086 Bug 1 的验收结论原文即
「消除楔形**保灰线**」（CHANGELOG.md:722）—— 当时已下调目标，灰线是**明确保留项**。

### 机制（主控 2026-09-05 实读代码取证，非凭记忆）

`src/main.rs:2139` 用 `apply_overlay_window_region(hwnd, rect, Some(16), true)`
建一个 **`CreateRoundRectRgn` 半径 16 的二值掩码**（`:363-380`），
去卡 `draw_processing_primitives`（`:3163`）用 D2D `DrawRoundedRectangle`
画的**抗锯齿** 1px 描边（`corner_radius = 16.0`，色 `OVERLAY_BORDER_GRAY = 0x3A3A3C`）。

- `SetWindowRgn` 每个像素非在即不在，**没有中间态**
- 而抗锯齿的柔和过渡**恰好活在被掩码切掉的那一圈**
- ⇒ 幸存的半亮像素成为断续粗灰点，被切处留缺口 = **视觉上的「粗乱」而非「细而硬」**

**加重因素**：overlay 高 36px，圆角半径 16 —— 上下圆弧几乎相接，左右两端近胶囊形，
**二值掩码的阶梯在近乎垂直的弧段上最扎眼**，故左端尤脏。

生产代码注释（`:2133-2138`）当时即写明该取舍，并留话
「if end-testing prefers the soft edge, revert to None」——**本次端测就是那句话等的答复**。

### 为什么不回退 `None`

回退 → 软边恢复，但**角上楔形同时恢复**（= Gavin 上一轮报的 Bug 1 本体）。

主控另评估过两条绕法，均否决：
- `LWA_COLORKEY` 替 `LWA_ALPHA`：仍是二值透明，阶梯照旧，还多一层色键边缘杂色
- 只在 Processing 态临时切 `UpdateLayeredWindow`：同一 HWND 上混用两种合成方式，
  属补丁式设计（违反 `feedback_no_patch_design`），状态切换会闪

⇒ **重申 DEC-056 结论：`LWA_ALPHA` 下两条路都到不了「棒」，与硬件性能无关。**

### 决策

| 项 | 决定 |
| --- | --- |
| 是否为这条灰线单独出包让 Gavin 二选一 | **否**。Gavin 原话「先不试了，按照你建议来」 |
| 是否打断 BUILD-098 | **否**。该包无任何 overlay 视觉改动，装的是 D2D-HANG-095 托盘退出 P0 修复 |
| 下一批做什么 | **P2+P3 五态合并迁 D2D → 紧接 per-pixel alpha**，按 DEC-057 排期，提到最优先 |

### 原因

1. 软边与硬边**两个都难看**，让 Gavin 在两个难看的里选一个是浪费他的端测轮次
   （他 2026-09-04 已明确：不想为零碎改动反复端测）
2. per-pixel alpha 是唯一真解，而它**必须等八态全迁完** ——
   残留任何一处 GDI 绘制都会在预乘 alpha 位图上打透明洞
3. 因此「尽快迁完 D2D」不再只是为了字更清楚，**它是这条灰线的唯一通路**

### 影响

- **不影响** BUILD-098 的出包与验收
- OVERLAY-062 / 065 / 069 的排期依赖关系不变
- DEC-057 两轮端测排期不变，本决策只是确认第 ② 轮内容并提高其优先级

## DEC-057 补充 · OVERLAY-101/102 不单独出包，与 P2+P3 合并端测（Gavin 2026-09-05 拍板，**推翻 DEC-057 四节**）

**Gavin 原话**：「不用先出包，一起改完一起出包端侧，节省时间」。

### 推翻了什么

DEC-057 第四节曾**否决**过「这一包也不出、等 P2+P3 做完一起端测」，理由两条：
1. Gavin 报的三个缺陷（尤其他明说「体验非常差」的宽度抖动）已修好，让他多等一批才能验不值当；
2. 万一 P2/P3 带出新问题，会把这三个修复的验证一起搅浑。

当时的结论是「**要压就压 P2+P3，不压这一包**」。**本决策把这一条也压掉了，以 Gavin 本次指示为准。**

### 为什么现在可以压（与 09-04 的判断差在哪）

09-04 那次否决的前提是「这一包＝086 三缺陷 + D2D-P1」，那是**首批 D2D 落地**，
Gavin 一个月没见到效果，且三个缺陷都是他亲口报的体验问题。

本次要压的这一包**性质不同**：BUILD-098 端测 7 项已过 6，**三个缺陷里两个当场结案**
（开嗓空窗口 ✅、流式细腻度 ✅），只剩 OVERLAY-101 的 Bug B（宽度到上限后右窜）
一个位置缺陷 + OVERLAY-102 的宽度比例调整。**风险敞口比 09-04 小得多**，
且 Bug A 本来就缺证据、修不了，这一包出去也解不了它。

### 代价（记录在案，出问题不要说没提醒）

1. **归因搅浑的风险仍然存在**：合并包里若出现位置/宽度异常，
   要区分「101 没修好」还是「P2+P3 带出的新问题」会更费劲。
   **缓解**：IMPL-109 的 12 hunk 全部限定在五态绘制路径内，
   `centered_x` / `interpolate_step` / `streaming_scroll_offset` 三个几何函数
   **明令不许碰**（见 `docs/D2D-P2P3-PLAN.md` §3.4.3 非回归清单）。
   真出位置问题，先查 P2+P3 的 region 与 hit rect，几何函数是干净的。
2. **Bug A 的 `-debug` 日志跟着延后一批**。Bug A 机制至今未确立
   （coder-2 的假说已被主控全量日志统计证伪，见 `[LOG-FIELD-IS-OUTPUT-NOT-INPUT-001]`），
   在拿到 09-05 之后带 `-debug` 的现场日志之前，**任何人不许再提 Bug A 的修法**。

### 新的批次形态

| 批 | 内容 | 端测轮次 |
| --- | --- | --- |
| 本批（合并后） | OVERLAY-101 Bug B + OVERLAY-102 + **D2D P2+P3 五态迁移** | 一次端测：Bug B 位置 + 五态视觉 + **带 `-debug` 抓 Bug A 日志** |
| 下批 | per-pixel alpha（DEC-056 ③，前置条件八态全迁完，本批之后达成） | 一次端测 |

**DEC-057 二/三/五节全部继续有效**：per-pixel alpha 仍不许并进来（第三节是硬的）；
每批出包后 Gavin 目视确认仍是唯一视觉验收方式（DEC-055 红线 5）。

## DEC-058 · 用户词库自动学习只覆盖「应用内编辑」，放弃「注入后目标窗口再改」（Gavin 2026-09-06 拍板）

### 背景

BUILD-118 端测第 4 项，Gavin 问「上屏文字经用户编辑后会走两次命中进词库吗」。
主控取证后报告存在两条学习路径与一个缺口，Gavin 就该缺口拍板。

### 两条路径（主控 2026-09-06 实读代码取证）

| 路径 | 位置 | 比对 | 触发 |
| --- | --- | --- | --- |
| **A 应用内编辑** | `main.rs:5382`（WORDBOOK-053-B） | 原始 ASR 文本 vs overlay 内提交文本 | `SubmitRequested` |
| **B 注入后观察** | `main.rs:7299` → `maybe_learn_user_edit(:7445)` | 注入文本 vs sleep 后重读目标窗口文本 | pipeline 直接注入 |

`maybe_learn_user_edit` 全库仅 1 个调用点，overlay 提交走自己的注入分支
（`:5389 RestoreAndHide` 之后），**不经过 `:7299`** ⇒ 两路径互斥，不会双写。

### 决策

| 项 | 决定 |
| --- | --- |
| 「已注入到目标窗口后，用户再修改」这条学习路径 | 🔴 **不做** |
| 「应用内（overlay 编辑态）的修改」按规则比对学习进词库 | ✅ **必须保证** |

### 原因（Gavin 原话）

> 已经注入到目标窗口，用户再修改，目前技术无法完美检测用户修改内容，所以这条先不做。
> 我们要保证在自己应用内做的修改，能够按规则比对学习进词库。

路径 B 靠 `sleep(AUTO_LEARN_OBSERVE_MS)` 后重读目标窗口文本做 diff ——
读得到什么完全取决于目标应用的可访问性实现，**观测本身不可靠**，
学进去的可能是用户改的、也可能是应用自己的自动补全/格式化。不可靠的信号进词库是负资产。

### 「保证应用内修改能学」的现状核查（主控 2026-09-06 取证，结论：结构上已成立）

**应用内编辑入口全库唯一**：`RecordingWithText` 态点击文本区 → `EditRequested`(`:1932`)
→ `EnterEditMode`(`:1446`) → `StreamingEditing` → 提交。

- `text_hit_rect`（可点击进编辑的热区）**只在 `RecordingWithText` 分支设置**（`:2126`）；
  `FocusLost` 预览分支只有「复制到剪贴板」（`:1895` 区），**不可编辑**
- ⇒ 进得了编辑态就必然有流式文本 ⇒ `last_streaming_text` 必然有值
- ⇒ 代码注释所说「仅在线流式 ASR 才走这条」**不是缺口，是结构必然**：
  本地模型路径不产生 `StreamingText`，overlay 里根本没有文本可点、进不了编辑态

### 学习仍需通过的四道闸（非缺陷，是设计，记录备查）

| # | 闸 | 位置 | 效果 |
| --- | --- | --- | --- |
| 1 | 路径限定 | 结构必然 | 仅在线流式（见上，非缺口） |
| 2 | `extract_correction_word` | `wordbook/mod.rs:186` | 提取不出差异词则不学 |
| 3 | `is_valid_candidate` | `wordbook/mod.rs:28` | 拒句末标点 / 换行 / 超长 / `1.`·`①` 编号项 |
| 4 | **阈值 = 2** | `config/mod.rs:21` 默认 2，用户 `config.toml` 实测 2 | **同一词第 2 次改到才晋升**，首次只进候选表 |

已在词库中的词直接跳过（`mod.rs:153` `cache.exists` → 删候选返回），不重复计数。

### 影响

- 路径 B 代码**不删**（直接注入场景仍在用），只是不再为其补 overlay 提交后的覆盖
- 后续若要提高应用内学习命中率，可调的是闸 4 阈值（Gavin 决策权）与闸 3 规则，不是加路径
- 🔴 **待办**：闸 2/3/4 目前无端到端验证证据，只有单元测试。
  排入 BUG-119 之后的 TEST-SYNC，补「应用内编辑 → 词库」全链路验证

## DEC-059 · 系统提示词按独立模块对待：改动须有 A/B 实证，快照护栏机器拦截

- **背景**（Gavin 2026-09-06 重申）：
  > 「系统提示词重要性等同于一个单独模块——格式化输出模块。改动提示词一定要仔细慎重，不可改坏原有功能。」

  当日 Gavin 端测报「无序列表输出不出来」，怀疑提示词被改坏。主控取证：
  代码侧 F3 无序规则与四语枚举标记清单**完好、8-14 后零改动**；
  真因是测试污染把 `llm.enabled` 写成 false（另案）。
  **但这已是同一疑问第二次出现**（首次 2026-08-14，结论「模型漂移+样本临界」），
  而当时提出的「引入回归看门测试」建议**一直没落地** —— 所以每次漂移都只能靠 Gavin 端测撞出来。

### 决策

| # | 规则 |
| --- | --- |
| 1 | **提示词是模块，不是文案。** 承载它的三处（`src/llm/mod.rs` 的 F3/F4 构建、`src-tauri/src/i18n.rs` 的默认 system prompt、用户 `config.toml` 的 `llm.system_prompt`）任一改动，一律按「改生产模块」立单，禁止夹带在其它任务里顺手改 |
| 2 | 🔴 **改动完成判据 = 真实 API A/B 实证**，不接受静态论证。照 `TEST-PROMPT-AB-033` 的做法：固定样本集、改前改后各跑、贴原始输出对照。理由见既有教训：提示词的因果链只能由真实调用确认，静态阅读再严密也只是假设 |
| 3 | **快照护栏机器拦截**：对提示词承载区做 include_str 快照断言，任何改动**必然变红** —— 红了必须由改动者主动更新快照，并在 result.md 附 A/B 证据。这一条把「慎重」从口号变成必经动作 |
| 4 | **固定回归样本集**（8-14 遗留建议，本次落实）：维护一组覆盖有序/无序、中英日韩的样本，定期跑并留档。目的不是防我们改坏，而是**把模型漂移变成可观测事件** —— 否则漂移永远只能靠 Gavin 端测撞出来 |
| 5 | 用户在 `config.toml` 里的自定义提示词属**用户资产**，任何路径迁移/默认值回填都不得静默覆盖它 |

### 与既有决策的关系

- DEC-039 管**怎么写**（清单是示例不是判据），DEC-040 管**层间结构**（引用只能向上、裁决点唯一），
  **本条管「改动流程」** —— 三者互补不重叠。

### 遗留

- 规则 4 的样本集尚未建立，需单独立单；在它建成之前，规则 2 的 A/B 只能临时组样本。

## DEC-060 · L0 保真规则区分「内容单元」与「话语标记」：后者允许在格式化时移除（Gavin 2026-09-06 拍板）

### 背景

Gavin 2026-09-06 端测：有序列表完美、无序列表始终不出。主控取证定位到提示词内部硬冲突：

| 规则 | 位置 | 要求 |
| --- | --- | --- |
| `L0_1_FIDELITY` | `src/llm/mod.rs:272` | 「`<speech>` 里**每一个语义单元都必须出现**在 `<corrected>`……**本规则压过下面所有格式、样式规则**」 |
| `L0_2_FIDELITY_OVER_FLUENCY` | `:276` | 「保留语义单元让句子别扭，就保留它、接受别扭。别扭但完整是正确，流畅但残缺是失败」 |
| `F3c` 无序示例 | `:1247` | `比如有些学生头发过长，再比如…` → `- 有些学生头发过长\n- …`（**示例把「比如」「再比如」删掉了**） |

⇒ 模型面临死结：L0 说一个字不许删且自称最高优先级；F3c 说做无序列表得删标记词。
模型选 L0 → 保留「比如」→ 做不成 F3c 演示的形态 → **退回散文**。

**这精确解释了 2026-08-03 PROMPT-ARCH-025 的实验数据**：6 变体 × 3 输入 × 3 次 = 54 次真实调用，
无序**全部 0/9**、有序 3/3 —— 而**六个变体没有一个动过 L0**。
有序之所以没事：`1. 第一点怎样怎样` 里标记词留在项内，**什么都没删**，与 L0 不冲突。

### Gavin 的论断（原话）

> 「L0 规则有问题。如果需要格式化输出，必然要对输入的文本、语义单元做处理。
> 而且无序列表输出从效果上说，去掉原来输入里的『比如、再比如、还比如、还有…』这些词效果更好。
> 所以 L0 规则规定得太死板，应该是可以做适当处理，但不能违背、扭曲原来的语意。」

### 决策

**L0 的保护对象收窄为「内容单元」，话语标记不在其列。**

| 类别 | 举例 | L0 是否保护 |
| --- | --- | --- |
| **内容单元** | 数量、单位、量词（3斤/一斤/per pound/ずつ）、实体、修饰语、谓语 | ✅ **必须保留，规则不变** |
| **话语标记 / 枚举标记** | 比如/比如说/例如/譬如、再比如、还有/另外/此外、for example/also/plus、たとえば/また、예를 들어/또 | 🔴 **允许在格式化时移除** |

**放行条件（三者同时满足）**：
1. 移除发生在**格式化规则要求的转换**中（如无序枚举转 Markdown 列表）；
2. 该标记所承载的**并列/举例关系由输出结构本身承载**（`- ` 列表形式即表达了并列）；
3. **不得改变、扭曲任何内容单元的语义**。

### 🔴 必须避开的回归（L0 的立身之本）

L0 当初是为修真 bug 而加：模型会删掉量词单位求通顺（`2.80元斤`、`3斤土豆` 的「斤」）。
**本次放宽绝不能把这个洞重新打开。** 实施时必须：

- 在规则文本里**显式列出话语标记这一唯一例外**，而非泛泛写「可适当处理」；
- 保留 L0-2「宁可别扭不可残缺」对**内容单元**的效力；
- 回归验证必须包含**当初触发 L0 的那批数量/量词样本**，确认它们仍然完整保留。

### 实施与验证

- 属**提示词模块改动**，按 `DEC-059` 执行：单独立单 + **真实 API A/B 实证** + 快照护栏更新。
- 这一单同时就是 2026-08-03 遗留的**决定性测试 V7**（F3c 无序示例是否需保留标记词）：
  L0 放宽后，F3c 的删标记示例不再与 L0 冲突，**V7 的前提消失** —— 应先验放宽 L0 的效果，再决定 F3c 是否还需改。
- A/B 样本集必须覆盖：无序长项 / 无序短项（买菜句，`:1248` 有反面示例明说短项内联不列表）/ 有序 /
  数量量词保真 / 单例举例不列表。

### 遗留

`:1248` 的买菜反面示例与用户实测样本高度相似，**今晚的失败可能同时受两个机制影响**
（L0 冲突 + 短项内联反面示例）。A/B 时必须用长项样本把两者分开。

## DEC-061 · 提示词不写死规则，把边界判断权交还模型（Gavin 2026-09-07 拍板，推翻自己 2026-07-31 的保守默认）

### Gavin 的论断（原话）

> 「规则不能定死了，需要让 LLM 有足够的思考判断空间，否则我们也没办法穷尽所有规则，
> 反而不停制造更多的漏洞。」

### 这条是怎么被证出来的

2026-09-06/07 端测：无序列表反复不出。**沿「继续补样例」的路走了两轮都没解决** ——
补了动作型、跨句型、事物型示例，端测仍失败。

真因不是样例不够，是 F3c 末尾一句**单向保守兜底**：

```
If unsure, DO NOT use a list — keep the text as a continuous paragraph.   （Gavin 2026-07-31 拍板）
```

前文的语义授权其实写得很足（「必须运用你自己的语言理解，不能只做词汇匹配……
即使标记词不同、不在清单里、甚至完全没有标记词，只要平行就成立」），
**但这一句把它整个抵消了** —— 模型读完一大堵标记清单，对任何非样例输入都处在
「拿不准」状态，于是永远选择不做。

删掉它、换成「边界情形必须自己判断」之后，**同一批端测四项全过**
（跨句枚举 / 事物型枚举 / 普通叙述不被过度列表化 / 短项流水账仍内联）。

### 决策

| # | 规则 |
| --- | --- |
| 1 | **禁止单向兜底默认**。提示词里不得出现「拿不准就 X」这类把判断替换成固定答案的条款。边界情形一律要求模型**按该语言、该语境下一个讲究的写作者会怎么做**来判断 |
| 2 | **清单与示例只作校准，不作闸门**（DEC-039 的执行层落法）。必须**显式声明**「不是白名单、不是闸门」，并**显式禁止**「没有示例匹配所以不做」这条推理路径 —— 只写「ILLUSTRATIVE, NOT EXHAUSTIVE」不够，实测挡不住 |
| 3 | **对称约束代替单向默认**。两个失败方向（做过头 / 漏做）必须同时点名、明确等价，让模型在两侧之间判断，而不是倒向安全的一侧 |
| 4 | **补样例是校准手段，不是修 bug 手段**。连续两轮补样例无效时，应怀疑存在**抵消授权的兜底条款**，而不是继续加样例 |

### 与既有决策的关系

- **DEC-039**（清单是示例非判据）给出原则，**本条给出它的失效形态与执行判据** ——
  原则写了不等于生效，`ILLUSTRATIVE` 四个字挡不住一句「if unsure, don't」。
- **DEC-040**（层间不得歧义、裁决点唯一）管结构，本条管**授权与兜底的力量对比**。
- **DEC-059**（提示词按模块对待，改动须 A/B + 快照护栏）是本条的执行纪律。

### 已落的机器闸门

`src/llm/mod.rs` 快照护栏新增三条断言并逐条消融验红：
`JUDGEMENT, NOT DEFAULT` 在场 / `NOT a whitelist and NOT a gate` 在场 /
`no example matches, therefore no` 反向禁令在场。删任一条即变红。

### 保留的反向防线（防倒向过度格式化）

去掉保守默认**不等于放任**。同时保留：对称警告（两方向一样错）、
单例举例仍是段落、≤6 字裸名词仍内联。端测已验证普通叙述未被过度列表化。
