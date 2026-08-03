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
