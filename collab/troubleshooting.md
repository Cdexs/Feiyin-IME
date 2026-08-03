# 技术坑 & 解决方案 · voice-ime

---

## [DOC-STATE-DRIFT-001] ⚠️ session 中断致「文档说未做、实际已做」的反向状态漂移【必读】

**状态**：✅ 已识别并规避（2026-07-28 / orchestrator）

- **现象**：2026-07-28 新 session 启动，`collab/todo.md` 明确写着「待 Gavin 决策三项：git commit 未做 / 三处版本号**未动** / 等指令才出包」。Gavin 据此下达「commit + 查版本号 + 出包」三条指令。主控派发构建前按惯例做独立取证，**发现三项实际上在 07-27 晚间已全部完成**：代码提交 `155b595`（20:24，含三处版本号 0.7.1→0.7.2）、release 三 exe 已构建并同步 Publish（20:28~20:31，三处 sha256 一致、ProductVersion 0.7.2.0）
- **根因**：07-27 session 在「代码提交 + 出包」之后、「文档闭环」之前被中断。CHANGELOG 的出包记录写进了工作区但未提交，todo/progress 完全没更新，于是文档定格在中断前的旧状态
- **危害等级高于正向漂移**：常见的「文档说已做、实际未做」会在验收取证时立刻暴露（文件不存在、哈希对不上）；而本次的**反向漂移**不会报错——若照 todo 直接派发构建任务，tester-1 会顺利跑完一次完整出包（~5 分钟）并汇报全绿，没有任何环节会提示「这次构建是多余的」，浪费完全静默
- **强制规则**：**Orchestrator 派发任何构建/版本号/提交类任务前，必须以文件系统与 git 为准重新取证，不以 todo/handoffs 记载为准**。最小取证组合：
  1. `git status --short` + `git log --oneline -5`（提交是否已存在）
  2. `grep -n "^version" Cargo.toml src-tauri/Cargo.toml` + `tauri.conf.json`（版本号实际值）
  3. 产物 mtime vs 源码 mtime vs 提交时间的**先后关系**（产物是否覆盖了最新代码）
  4. 三处 `sha256sum` + `ProductVersion`（产物是否已同步且版本正确）
- **配套**：本条与 [VERSION-DRIFT-001]（版本号不能只信 handoffs 文字）同源，都指向同一条底层原则——**文档是索引不是事实源，事实源永远是文件系统与 git**
- **降低复发**：worker-guide §12 已要求验收后尽快 commit；本条追加要求——**出包/提交完成后的文档同步与产物动作视为同一个不可分事务**，宁可先写文档再动手，也不要留下"做完没记"的窗口

---

## [VERSION-DRIFT-001] 根 Cargo.toml 版本号与 handoffs 记录不符

**状态**：🟡 已规避，未溯源（2026-07-24 / orchestrator）

- **现象**：派发 v0.7.1 批次时，coder-1 发现根目录 `Cargo.toml` 实际版本为 `0.6.2`，但 handoffs.md 中 FORMAT-LLM-001-CORE 条目明确记载"version 0.6.2→0.7.0"；同期 `src-tauri/Cargo.toml`/`tauri.conf.json` 确认已正确停留在 0.7.0（本次再升到 0.7.1 前核实）
- **根因**：未深究（可能是该次改动后被后续覆盖、未提交、或报告与实际操作不一致），根 Cargo.toml 单独漏了一次版本号同步
- **处理**：本次直接由 0.6.2 改为 0.7.1（跳过中间 0.7.0），三处版本号最终对齐，不影响功能
- **教训**：**Orchestrator 验收版本号类改动时不能只信 handoffs.md 文字记录，必须实际 Read 三处文件核对**（`Cargo.toml` / `src-tauri/Cargo.toml` / `tauri.conf.json`），版本号是最容易出现"报告与实际不一致"且难以事后察觉的一类改动

## [WINDOW-TITLEBAR-BUG] ⚠️ Tauri v2 Windows decorations: false 不生效【必读】

**状态**：🔴 已确认 Tauri v2 bug（2026-04-22）

- **现象**：`tauri.conf.json` 设置 `decorations: false` 或 Rust 端调用 `set_decorations(false)` 后，Windows 上原生标题栏仍可见
- **验证**：`HasCaption = True`（预期 False）
- **根因**：Tauri v2 在 Windows 上 WebView2 玄境的窗口装饰控制存在已知 bug（GitHub #14859/#11654/#11296）
- **已尝试无效**：
  1. `tauri.conf.json`: decorations: false + shadow: false
  2. Rust setup: `main.set_decorations(false)`
  3. cargo clean + rebuild
- **可能的 workaround**：
  - WebView2 Repair（Windows 设置 → 应用 → WebView2 → 修复）
  - 暂时保留原生标题栏 + 前端自定义 Logo 区
- **官方进度**：bug 仍在跟踪中，尚未修复

---

## [WINDOW-TITLEBAR-REVERT] ✅ 主窗口标题栏错误隐藏的正确回滚方式

**状态**：🟢 已解决（coder-1，2026-04-22）

- **现象**：`WINDOW-TITLEBAR-001` 将主窗口改为 `decorations: false + transparent: true` 后，真实运行时失去原生标题栏与最小化/关闭按钮
- **根因**：主窗口被整体切换到无边框透明窗体路径，但验收只覆盖了前端构建与自动化页面测试，没有对实际窗口原生样式做运行时校验
- **正确修复**：
  1. 不只恢复 `decorations`，还要一并移除主窗口 `transparent: true`
  2. overlay 继续保持透明无边框，避免误伤录音浮层
  3. 必须实际启动 `voice-ime-ui.exe`，检查运行中窗口样式是否恢复 `WS_CAPTION`、`WS_SYSMENU`、`WS_MINIMIZEBOX`，且 `WS_EX_LAYERED` 不再存在
- **教训**：凡是修改主窗口 `decorations`/`transparent` 的任务，不能只用 Vitest/Playwright 作为验收，必须加一条真实窗口样式校验

---

## [VERIFY-001] ⚠️ 原生窗口行为修改必须人工验收【必读】

**状态**：🔴 问题暴露（2026-04-22 WINDOW-TITLEBAR-001）

- **现象**：WINDOW-TITLEBAR-001 设置 `decorations: false` 后窗口失去最小化/关闭按钮，用户无法关闭窗口，但验收声称"通过"
- **根因**：
  1. Playwright/Vitest 只验证 React DOM 内容，无法验证 Tauri 原生窗口行为
  2. Orchestrator 未实际启动 UI 程序人工检查就宣布验收通过
- **教训**：
  1. 任何涉及 `decorations`/`transparent`/窗口原生行为的修改，**必须**人工启动 voice-ime-ui.exe 验证
  2. 不能只依赖 Playwright/Vitest 测试结果
  3. 测试框架不覆盖原生窗口行为（标题栏、最小化/关闭按钮、拖拽等）
- **强制规则**：原生窗口行为修改 → 构建产物 → 人工启动验证 → 才能宣布验收通过

---

## [BUILD-003] ⚠️ tester-1 构建 task 漏执行 Step 3 主程序【必读】

**状态**：🔴 问题暴露（2026-04-19 BUILD-034）

- **现象**：BUILD-034 任务完成后，target/release/*.exe 时间戳仍为旧（22:26），新构建产物未覆盖
- **根因**：tester-1 只执行了 build-guide.md Step 2（Tauri UI），漏了 Step 3（主程序 cargo build --release）
- **教训**：
  1. 构建任务必须完整执行 build-guide.md Step 1/2/3 三步
  2. 完成后必须验证产物时间戳（ls -la target/release/*.exe）
  3. **禁止**跳过任何步骤，即使 Step 2 已有产物
- **后续**：tester-1 INJECT_MSG 已注入「构建专项规则」，需强化 Step 顺序执行意识

---

## [BUG-027] ✅ 托盘菜单"配置"二次点击后配置窗口不显示【已解决】

**状态**：🟢 已解决（coder-1，2026-04-19）

- **现象**：主程序启动后，第一次点击托盘菜单"配置"可正常显示；之后再次点击不显示
- **根因**：`voice-ime-ui.exe` 除主窗口外还持有隐藏 `overlay` 窗口，主窗口关闭后进程未退出，导致 `spawn_settings_process()` 持续误判"设置窗口仍在运行"
- **解决**：`src-tauri/src/main.rs` 为 main 窗口添加 `CloseRequested` 事件处理，显式关闭 overlay 窗口并调用 `app_handle.exit(0)` 退出整个进程
- **代码变更**：`src-tauri/src/main.rs` 第73-84行 `.on_window_event()` 处理

---

## [UI-001] ⚠️ 系统提示词设计约定：仅 Ctrl+T 弹窗，禁止在 LLM 标签页出现【必读】

**状态**：🔴 已两次回归（2026-04-18 OPT-001-UI，2026-04-19 OPT-001-UI 再次）

- **设计决策（UI-030-4）**：系统提示词 **仅** 在 App.tsx 的 Ctrl+T 弹窗中编辑，`Llm.tsx` 不得出现任何 system_prompt 输入区域
- **反模式**：任何修改 `Llm.tsx` 的任务，**禁止**添加 `system_prompt` textarea 或 section
- **违反后果**：每次 OPT 任务修改 LLM 页面时，必须先检查是否存在 system_prompt 输入区域并移除

---

## [BUILD-002] ⚠️ Tauri UI 构建产物路径混淆导致测试旧版本【必读】

**状态**：🟢 已解决 + 流程固化（2026-04-19）

- **现象**：tester-1 完成 Tauri UI release 构建后，端测结果显示窗口尺寸仍为旧值，改动未生效
- **根因**：
  - Tauri UI 构建产物落在 `src-tauri/target/release/voice-ime-ui.exe`
  - 主程序 `spawn_settings_process()` 优先查找 `target/release/voice-ime-ui.exe`（项目根）
  - `target/release/` 中存在旧版 exe（前次构建的遗留），导致主程序启动旧版 UI
- **解决**：
  1. 代码层：`spawn_settings_process()` 新增 `src-tauri/target/release/` 路径查找（优先于 debug）
  2. 流程层：build-guide.md Step 4 强制要求构建完成后将新 exe 复制到 `target/release/`
     
     ```bash
     cp src-tauri/target/release/voice-ime-ui.exe target/release/voice-ime-ui.exe
     ```
- **教训**：Tauri UI 构建后，必须执行 Step 4 覆盖 target/release/ 的旧产物，否则端测仍是旧版

---

## [BUILD-001] ✅ tauri-cli v2 与 Tauri v1 项目不兼容

**状态**：🟢 已记录（tester-1，2026-04-19）

- 现象：安装 tauri-cli v2.10.1 后执行 `cargo tauri build` 报错，无法构建
- 根因：项目使用 Tauri v1（`tauri = { version = "1" }`），tauri-cli v2 不向下兼容
- 解决：直接用 `cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol`，跳过 tauri-cli

---

## [BUG-025] ✅ 组合热键捕获不起作用

**状态**：🟢 已解决（coder-1，2026-04-18）

### 背景

用户报告只能捕获单个键，不能捕获 Ctrl+A 等组合键。

### 根因分析

1. `keyCode` 已被 MDN 标记 deprecated，在 WebView2 中可能不准确
2. JavaScript keyCode vs Windows VK 码映射可能不一致
3. `e.metaKey` 在 Windows 上不对应 Win 键（Win 键被系统拦截）
4. 纯修饰键按下时无法捕获（被忽略）

### 解决方案

- 改用 `e.code`（物理键标识）替代 keyCode
- 建立 `CODE_TO_VK` 映射表（e.code → Windows VK 码）
- 移除 metaKey（Win 键）修饰符支持（Windows 系统限制）
- 添加 console.log 调试输出便于追踪

### 映射表支持

```typescript
const CODE_TO_VK: { [key: string]: number } = {
  'KeyA': 0x41, 'KeyB': 0x42, ..., 'KeyZ': 0x5A,  // 字母键
  'Digit0': 0x30, 'Digit1': 0x31, ..., 'Digit9': 0x39,  // 数字键
  'F1': 0x70, 'F2': 0x71, ..., 'F12': 0x7B,  // 功能键
  'ControlRight': 0xA3, 'AltRight': 0xA5,  // 右侧修饰键
  ...
};
```

### 相关条目

- [BUG-021] 右侧修饰键捕获（location 区分）
- [BUG-022] 右侧修饰键改用轮询检测

---

## [TEST-001] ✅ Tauri UI 窗口内容截图验证标准（TEST-001 固化）

**状态**：🟢 已固化（2026-04-17）

### 背景

URGENT-TEST-013 发现仅验证进程启动不够，必须截图验证窗口实际内容。

### 标准流程

**步骤 1: 启动 UI**

```bash
# 清理旧进程
taskkill /F /IM voice-ime-ui.exe /T

# 启动 UI
./target/release/voice-ime-ui.exe &

# 等待窗口加载
sleep 5
```

**步骤 2: 截图**

```powershell
# PowerShell 截图命令
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap($screen.Width, $screen.Height)
$gfx = [System.Drawing.Graphics]::FromImage($bmp)
$gfx.CopyFromScreen($screen.Location, [System.Drawing.Point]::Empty, $screen.Size)
$bmp.Save('screenshot.png')
$bmp.Dispose()
$gfx.Dispose()
```

**步骤 3: 验证内容**

- ✅ 窗口标题："飞音语音输入"
- ✅ Tab 导航：通用/语音输入/优化模型/词库/关于
- ✅ 表单控件：复选框、按钮、单选框
- ✅ 主题颜色：橘色 #ff6b35（UI-026）
- ❌ 不应出现："localhost 拒绝连接"

**步骤 4: 诊断**

- 若空白/localhost 错误：执行 `cargo clean -p voice-ime-ui` + 重新构建
- 若进程存在但无窗口：检查 WebView2 子进程是否启动

### 相关条目

- [TAURI-001] Tauri v1 Release 构建必须启用 custom-protocol feature
- [ENV-002] UI 框架（Tauri + React）构建要求

---

## [ARCH-001] ⚠️ eframe/winit 线程模型禁区【必读】

**状态**：已定位失败原因，禁止重复试错

### 禁止事项

1. **禁止**把同一个 root viewport 切成 Hidden / Settings / Overlay 三态
   
   - root viewport 显隐切换在 tray-first 场景下不稳定，配置窗口、波形窗口均可能不显示
   - `eframe/egui` 在窗口不可见时刷新行为不适合作为事件泵

2. **禁止**在同一进程内启动多个 `eframe::run_native` 线程
   
   - `winit` 要求事件循环在主线程创建，且一个进程只创建一次
   - 报错特征：`Initializing the event loop outside of the main thread` / `EventLoop can't be recreated`

3. **禁止**把"退出问题"当单纯窗口关闭问题处理

### 正确方向（已验证，见 DEC-001~005）

- 主控：Win32 controller 窗口 + 消息循环
- 设置窗口：独立 `--settings-ui` 子进程
- 录音悬浮层：原生 Win32 透明顶置窗口（GDI）
- 热键：`RegisterHotKey`
- 退出：controller 统一 shutdown 协议

---

## [BUG-018] 热键启动延迟（模型路径依赖工作目录）

**状态**：🟢 已解决

- **根因**：`model_dir()` 返回相对路径 `models/`，从非项目目录启动时触发 huggingface.co 下载，连接超时导致 Transcriber 初始化失败
- **解决**：`model_dir()` 改为 `current_exe().parent() / "models"`（见 DEC-011）

---

## [BUG-PTT] PTT 松键后录音不停止

**状态**：🟢 已解决（coder-1，2026-04-17）

- **根因**：`SetTimer(HWND::default())` 依赖线程消息队列，在某些场景下 WM_TIMER 不稳定
- **解决**：改用独立轮询线程 + `Arc<AtomicBool>`（ptt_active/ptt_poll_stop）+ crossbeam channel 检测释放事件；移除 PTT_TIMER_ID / SetTimer / KillTimer（见 DEC-004）

---

## [PERF-001] LLM 优化响应延迟过长

**状态**：🟢 已解决

- **根因**：SiliconFlow 推理模型默认开启 chain-of-thought，导致数秒级延迟
- **解决**：所有 LLM 请求添加 `enable_thinking: false`，`max_tokens` 降至 512（见 DEC-008）

---

## [ENCODING-FIX-001] tauri.conf.json 中文标题发生乱码

**状态**：🟢 已解决（coder-1，2026-04-20）

- **现象**：`src-tauri/tauri.conf.json` 中 `productName` 与主窗口 `title` 显示为 `椋為煶璇煶杈撳叆`，GUI 自动化无法断言正确窗口标题
- **根因**：配置文件内这两处中文标题已经以错误编码内容落盘，构建出的 `voice-ime-ui.exe` 会携带相同乱码标题
- **解决**：将两处字段统一改回 `飞音语音输入` 并保持 UTF-8 保存；随后重新执行 release 构建，并同步覆盖根目录 `target/release/voice-ime-ui.exe`，确保测试命中新产物

---

## [ENV] 本机构建环境问题

**状态**：记录备查

- **cmake 找不到**：临时将 VS BuildTools CMake bin 目录追加到 PATH
- **cargo 找不到**：临时追加 `~/.rustup/toolchains/stable-x86_64-pc-windows-msvc/bin`
- **exe 被占用**：先退出运行中的 `voice-ime.exe`，再重跑 `cargo build --release`

---

## [ENV-002] UI 框架（Tauri + React）构建要求

**状态**：记录备查

为了支持 v0.4.0 引入的 Tauri + React 设置界面，开发环境必须满足以下要求：

### 环境要求

- **Node.js**: v18.0+（推荐 v20+）
- **npm**: v9.0+
- **Rust**: stable toolchain
- **Windows SDK**: 确保已安装 C++ 桌面开发工作负载（VS Build Tools）

### PATH 配置

确保以下路径已加入系统环境变量 PATH：

- `C:\Users\<User>\.cargo\bin` (Cargo & Rust)
- Node.js 安装路径 (npm)

### 构建注意事项

1. **第一次运行**: 必须在根目录执行 `npm install`。
2. **Tauri 开发**: 在根目录执行 `npm run tauri dev` 或在 `src-tauri` 执行 `cargo check`。
3. **图标依赖**: `src-tauri/tauri.conf.json` 中的 `bundle.icon` 必须指向有效的图标文件。

---

## [TAURI-001] ⚠️ Tauri v1 Release 构建必须启用 custom-protocol feature【必读】

**状态**：🟢 已解决（tester-1，2026-04-17）

- **现象**：voice-ime-ui.exe 运行后窗口空白或显示"localhost 拒绝连接"
- **根因**：Tauri v1 的 `Cargo.toml` 定义了 `custom-protocol = ["tauri/custom-protocol"]` feature。Release 构建必须启用此 feature，否则 webview 会 fallback 到 `tauri.conf.json` 的 `devPath: http://localhost:1420`
- **解决**：构建命令必须包含 `--features custom-protocol`
  
  ```bash
  cargo build --release --features custom-protocol --manifest-path src-tauri/Cargo.toml
  ```
  
  或使用 Tauri CLI（自动处理）：
  
  ```bash
  npm run tauri build
  ```
- **影响**：BUILD-009 使用不带 feature 的命令构建，导致产物无法正确加载嵌入的 dist/ 资源

---

## [TAURI-002] ⚠️ cargo 缓存可能导致 custom-protocol feature 未重新编译【必读】

**状态**：🔴 问题暴露（tester-1，2026-04-17）

- **现象**：执行 `cargo build --release --features custom-protocol` 后，产物仍尝试连接 localhost:1420
- **根因**：cargo incremental build 缓存导致 voice-ime-ui 未重新编译，旧代码（无 custom-protocol）仍在产物中
- **解决**：清理缓存后重新构建
  
  ```bash
  cargo clean --manifest-path src-tauri/Cargo.toml -p voice-ime-ui
  npm run build
  cargo build --release --features custom-protocol --manifest-path src-tauri/Cargo.toml
  cp src-tauri/target/release/voice-ime-ui.exe target/release/voice-ime-ui.exe
  ```
- **教训**：切换 feature 后必须 `cargo clean -p <package>`，不能依赖增量编译

---

## [TEST-001] ⚠️ Tauri UI 测试必须验证窗口内容，不能只看进程启动【必读】

**状态**：🔴 问题暴露（2026-04-17 Gavin 反馈）

- **现象**：tester-1 报告 PASS，但用户实际测试仍为空白页面
- **根因**：测试方法缺陷，仅验证进程启动成功，未验证窗口内容是否正确显示
- **正确测试方法**：
  1. 启动 voice-ime-ui.exe
  2. 等待窗口完全加载（至少 3-5 秒）
  3. 截图或抓取窗口内容
  4. 验证是否包含：
     - "飞音语音输入"标题
     - General/Voice/Llm/Wordbook/About Tab 导航
     - 表单控件内容
  5. 截图保存作为证据
- **错误方法**：仅检查进程 PID + 内存占用 → 不代表 UI 正常
- **教训**：GUI 验证必须有视觉证据，不能依赖进程状态

---

## [BUG-027] 托盘菜单“配置”二次点击不显示

**状态**：🟢 已解决（coder-1，2026-04-19）

- **现象**：第一次点击托盘“配置”可正常显示；关闭配置窗口后再次点击没有任何反应。
- **根因**：
  - 控制器 `spawn_settings_process()` 依赖 `Child::try_wait()` 判断设置子进程是否已退出。
  - `voice-ime-ui.exe` 除主窗口外还注册了隐藏 `overlay` 窗口。
  - 用户关闭主窗口后，Tauri 进程仍存活，控制器因此误判“设置进程仍在运行”，跳过了二次拉起。
- **解决**：
  - 在 `src-tauri/src/main.rs` 监听主窗口 `CloseRequested`。
  - 主窗口关闭时显式关闭隐藏 `overlay`，并调用 `app_handle.exit(0)` 退出整个 Tauri UI 子进程。
  - 修复后控制器既有的 `try_wait()` 检测逻辑即可重新生效。

## [CRASH-001] Tauri 配置程序崩溃检测

**状态**：🟢 已解决（coder-1，2026-04-20）

- **现象**：`voice-ime-ui.exe` 缺少独立崩溃检测，panic 后不会生成报告，也不会弹出 crash reporter。
- **根因**：主程序 crash hook 仅存在于根项目 `src/main.rs`，`src-tauri` 入口没有注册 panic hook，也没有崩溃报告落盘与 reporter 拉起逻辑。
- **解决**：
  - 在 `src-tauri/src/main.rs` 启动前注册 panic hook。
- 在 `src-tauri/src/crash.rs` 复用根项目 `src/crash/storage.rs` 保存 `crash.json`。
- 崩溃后尝试定位主程序 `voice-ime.exe`，并以 `--crash-reporter` 启动现有 reporter UI。
- 对 Tauri 非预期退出增加 best-effort 兜底，覆盖可检测的 WebView2 渲染崩溃场景。
- **限制**：Tauri v1 当前未直接暴露 WebView2 `ProcessFailed`，因此渲染进程崩溃只能做近似检测，无法做到 100% 精准归因。

## [SENDINPUT-001] 热键 GUI 测试命中旧实例导致误判

**状态**：🟢 已解决（coder-1，2026-04-20）

- **现象**：`py -m pytest tests/test_cases/test_hotkey.py -v` 初次执行时 6 个用例全部失败，overlay 始终保持 hidden。
- **根因**：voice-ime 是单实例程序；如果测试前已有旧的 `voice-ime.exe` 常驻，新的测试进程会被单实例逻辑抢占或快速退出，SendInput 实际打到旧实例，从而与本用例刚写入的热键配置不一致。
- **解决**：
  1. 在热键测试文件内新增本地进程 fixture，启动前后都执行 `kill_existing_voice_ime()`。
  2. 先写入测试配置，再拉起干净进程，并额外等待 1 秒让热键注册完成。
  3. 改用 overlay 状态断言验证真实行为，避免继续把“进程没挂”误当成功。
---

## [MAC-011] 本机无法执行 Darwin 编译验证

**状态**：记录备查（2026-04-20 / coder-1）

- 现象：为补 `MAC-011` 做本地交叉校验时，`rustup target add x86_64-apple-darwin` 失败，导致无法在当前 Windows 主机上执行 `cargo check --target x86_64-apple-darwin`
- 已见错误：组件下载后回滚，最终报 `could not rename ... .partial ... (os error 2)`
- 当前处理：
  - 继续完成代码实现与 Windows 侧静态验证
  - 用 crates/doc source 交叉核对 `core-graphics 0.25` 与 `core-foundation 0.10` 的 API 形状，减少 Darwin-only 编译风险
  - 在 result/handoff 中明确标出“未完成实机 Darwin 编译”这一剩余风险
- 建议：
  - 在具备正常 `rustup`/Apple toolchain 的环境里补一次 `cargo check --target x86_64-apple-darwin`
  - 最终上线前在真实 macOS 主机上验证 Accessibility 权限、F9/组合键、Toggle/PTT/ESC 行为
## [MAC-012] Darwin cross-check blocked by missing C compiler

- Symptom: `cargo check --target x86_64-apple-darwin` fails during dependency build on this Windows host before reaching final crate validation.
- Root cause: the local cross-compilation environment does have the Darwin Rust target installed, but it does not expose a usable `cc` executable for crates such as `ring`.
- Resolution:
  - keep Windows-host validation at `cargo fmt --all` and root `cargo check`
  - record the cross-target failure explicitly instead of claiming Darwin verification succeeded

---

## [PLAYWRIGHT-FIX-001] Playwright CDP fixture ScopeMismatch ERROR

**状态**：🟢 已解决（tester-1，2026-04-21）

- **现象**：`test_webview_ui.py` 9 个测试全部 ERROR（ScopeMismatch）
- **根因**：`cdp_browser` 定义为 `scope="session"`，但依赖 `voice_ime_with_cdp`（`scope="module"`），pytest 禁止 session fixture 依赖 module fixture
- **解决**：
  1. `cdp_browser` scope 从 session 改为 module（匹配 `voice_ime_with_cdp`）
  2. `_find_main_page` 支持 Tauri v2 URL（`https://tauri.localhost/`）
  3. `main_page` fixture 增加 `wait_for_load_state("load")` + React hydration 等待
  4. 修复 Tab/checkbox 选择器匹配实际 CSS 类名
- **结果**：6 passed, 4 skipped, 0 errors（从 9 errors → 0 errors）
  - leave final macOS compile/runtime validation to a machine with a real Darwin cross toolchain or native macOS environment

---

## [UI-DEBUG-001] UI-FIX-005 后滚动条仍可见的诊断结论

**状态**：已定位为“测试覆盖不足 + Chromium/WebView2 fallback 写法高风险”，不是简单的 CSS 覆盖问题（2026-04-21 / coder-1）

- **已确认排除**
  - `ui/src/styles.css` 中 `.main-content` 的滚动条隐藏规则没有被同文件后续样式覆盖
  - 构建产物 `ui/dist/assets/index-Culu6L-0.css` 与源码一致，不存在打包丢规则
  - 普通页面没有第二个常驻纵向滚动容器；只有弹窗 `.modal-body` 在打开 Ctrl+T 模态框时会独立滚动
- **根因**
  1. 现有 Playwright 用例只断言 `getComputedStyle(el).scrollbarWidth/msOverflowStyle/overflowY`，只能证明声明存在，不能证明用户视觉上完全看不到滚动条
  2. 当前 `::-webkit-scrollbar` fallback 为：
     ```css
     .main-content::-webkit-scrollbar {
       width: 0;
       height: 0;
       background: transparent;
       display: none;
     }
     ```
     Chrome 官方文档指出：对 `::-webkit-scrollbar` 设置 `width`/`height` 会强制显示 overlay/classic scrollbar 模式；MDN 说明当 `scrollbar-width` 被支持且设置为非 `auto` 时，会覆盖 `::-webkit-scrollbar-*` 样式。两者混用在新版本 Chromium/WebView2 上存在行为不一致风险。
  3. WebView2 本身暴露了 Fluent overlay scrollbar 相关 browser flags，而当前 app 没有显式配置这些 flag，实际滚动条呈现受运行时/系统设置影响
- **建议修复方向**
  - 不要继续加 `!important`
  - 让 `scrollbar-width: none` 成为现代 Chromium/WebView2 的主路径
  - 将 `::-webkit-scrollbar` fallback 改为真正的 fallback，并移除 `width`/`height` 这类会影响渲染模式的声明
  - 测试升级为：
    - 断言 `offsetWidth - clientWidth === 0` 或等价 gutter 指标
    - 补一条滚动状态下的截图/视觉断言，而不是只看 computed style

### 已落实修复（UI-FIX-006）

- `ui/src/styles.css` 已改为显式分流：
  - `@supports (scrollbar-width: none)` 下仅保留标准路径
  - `@supports not (scrollbar-width: none) and selector(::-webkit-scrollbar)` 下仅保留 `display: none` 的 WebKit fallback
- 旧的 `width: 0` / `height: 0` 已删除，避免继续触发 Chromium/WebView2 的 scrollbar 渲染模式冲突

---

## [UI-DEBUG-002] 用户仍目视看到滚动条的真实来源

**状态**：已确认不是 `.main-content`，而是根页面被 `.sidebar::after` 溢出撑高（2026-04-22 / coder-1）

- **实测证据**
  - `ui/src/styles.css` 与 `ui/dist/assets/index-B0TxTrJi.css` 一致，`.main-content` 的滚动条隐藏规则已正确打包
  - 通过 WebView2 CDP 对 `https://tauri.localhost/` 主页面实测：
    - `.main-content` `scrollbarWidth = none`
    - `getComputedStyle(el, '::-webkit-scrollbar').display = none`
    - `offsetWidth - clientWidth = 0`
  - `CSS.getMatchedStylesForNode` 只命中项目里的 `.main-content` 规则，没有更高优先级覆盖
  - 同时 `body/html/app-container/sidebar` 的 `scrollHeight = 740`，`clientHeight = 720`
  - `.sidebar::after` 的实测样式为 `bottom: -20px; height: 200px;`
- **验证性实验**
  - 在 DevTools 运行时临时注入 `.sidebar { overflow: hidden !important; }`
  - 注入前：`bodyScrollHeight = 740`
  - 注入后：`bodyScrollHeight = 720`
  - 说明用户看到的滚动条来自页面根层溢出，而不是 `.main-content`
- **根治建议**
  1. 优先在 `.sidebar` 上裁剪光晕伪元素溢出，例如 `overflow: hidden`
  2. 或者把 `.sidebar::after` 的光晕改成不突破容器边界的实现
  3. 不建议先对 `body/html` 做全局 `overflow: hidden`，那会掩盖未来真实的页面溢出问题

### 已落实修复（UI-FIX-009）

- `ui/src/styles.css` 的 `.sidebar` 已添加 `overflow: hidden`
- 该修复直接对应已验证的根因：裁剪 `::after` 伪元素超出容器底部的 20px 溢出
- 这样不会改变 `.main-content` 的滚动策略，也不会通过全局隐藏 `body/html` 滚动来掩盖问题

---

## [WINDOW-TITLEBAR-RESEARCH-001] Tauri v2 Windows 自定义标题栏替换原生标题栏的现实约束

**状态**：已确认当前仓库不具备可直接落地的官方方案（2026-04-22 / coder-1）

- **现象**
  - Tauri 官方文档仍建议使用 `decorations: false` + 自定义拖拽区实现 custom titlebar
  - 但本项目真实运行时，主窗口样式仍保留 `WS_CAPTION`
  - 第三方 `tauri-plugin-decorum` 也仍依赖 `set_decorations(false)`
- **已确认事实**
  1. `titleBarStyle` 在 Tauri 配置中是 macOS-only，不支持 Windows
  2. `tauri-apps/tauri#14859`、`#11296`、`#4531`、`#12930` 都说明 Windows 侧标题栏/frameless 能力仍存在缺口
  3. `tao` 源码已经尝试清理 `WS_CAPTION` / `WS_THICKFRAME` 并触发 `SWP_FRAMECHANGED`
  4. `wry` 已启用 WebView2 `IsNonClientRegionSupportEnabled`
  5. 即便如此，本地运行结果仍保留标题栏，说明问题不在应用层简单缺配置
- **结论**
  - 不要继续把“多调几次 `set_decorations(false)`”当成修复路径
  - `tauri-plugin-decorum` 更适合作为现有 frameless 路线的封装，不是当前 Windows 标题栏 bug 的根治方案
  - 真正接近需求的路线是 Windows App SDK `AppWindowTitleBar` / WebView2 `WindowControlsOverlay`，但需要单独做 Windows 原生插件桥接
- **当前建议**
  - 短期保留原生标题栏
  - 若产品强制要求自定义标题栏，单独立项 Windows-only PoC，并明确 Win11 优先、Win10 降级

---

## [TESTER-SCREENSHOT-FAIL] ⚠️ tester-1 截图功能系统性失败【必读】

**状态**：🔴 问题暴露（2026-04-22，连续6次截图失败）

- **现象**：tester-1 执行截图任务时，生成的图片不是配置窗口内容
- **根因**：
  1. 截图时窗口可能已关闭或不在前台
  2. tester-1 自验方法不可靠：仅检查 color variety / orange pixel count，无法区分窗口内容
  3. 自验代码使用 GBK 编码输出导致 UnicodeEncodeError，关键窗口信息被吞没
  4. 使用固定坐标截图时误将 X/Twitter 浏览器窗口等背景窗口内容认作目标窗口
  5. Python ctypes 枚举窗口时，因编码问题（`'gbk' codec can't encode character`）跳过含中文的窗口标题
  6. PowerShell 脚本输出因 shell 转义/编码问题丢失，无法确认窗口实际位置
- **已失败次数**：
  1. SCREENSHOT-WINDOW-001 → 截到桌面背景
  2. SCREENSHOT-CONFIG-WINDOW-001 → 截到桌面背景
  3. SCREENSHOT-DIRECT-001 → 截到桌面背景
  4. BUILD-AND-VERIFY-RESIZABLE-FALSE-001 附带截图 → 截到桌面背景
  5. DEC-025 smoke-ui.png（19:08）→ 截到桌面背景
  6. DEC-025 smoke-ui.png（19:13）→ 截到 X/Twitter 浏览器窗口而非配置窗口
- **教训**：
  1. **截图自验不能依赖 color vareity / orange count 等简单图像统计**——那个只能区分桌面背景与"有内容"，无法区分"是目标窗口还是其他窗口"
  2. 必须验证窗口标题是否匹配（`FindWindow("飞音智能语音输入")` or `EnumWindows`），截图前通过 GetWindowRect 获取实时位置
  3. Windows 终端使用 UTF-8 输出时（`chcp 65001`），Python 默认编码仍是 `mbcs` 导致 GBK 报错。应显式 encode 到文件而非 print
  4. 窗口坐标是动态的——不要假设 (682,320) 等固定坐标，每次截图前必须重新枚举
  5. PowerShell 在 bash 中执行时复杂脚本易因转义问题静默失败，所有关键逻辑应改走 Python ctypes + 文件输出
  6. 截图前必须调用 `SetForegroundWindow` + `BringWindowToTop` + `ShowWindow(SW_RESTORE)` 三重前置
  7. feiyin-ime-ui.exe 不能独立运行（`is_main_process_running()` 检测），截图前须先启动 feiyin-ime.exe
- **强制规则**：
  1. 截图 → 验证窗口标题 + 实时坐标 → MoveWindow → BringWindowToTop → ShowWindow → 截图 → 将截图写入文件后 Read 验证 → 正确才报告完成
  2. 禁止使用固定坐标，每次截图前通过 EnumWindows 获取窗口位置
  3. Python 输出全部写文件绕过 GBK 编解码问题，不直接 print
  4. 如环境不支持可靠截图（WebView2 窗口不在测试机上），改为"目视验收"任务交给 Gavin

---

## [SCREENSHOT-METHOD-001] ✅ 正确截图方法（voice-ime-ui.exe 配置窗口）

**状态**：🟢 已验证（2026-04-22）

### 问题

tester-1 连续5次截图失败，截到桌面背景而非配置窗口。原因是窗口在后台，截图时未强制显示。

### 正确截图方法

**步骤 1：强制显示窗口**

```powershell
$hwnd = [IntPtr]<窗口句柄>

# Win32 API 定义
$code = @'
using System;
using System.Runtime.InteropServices;
public class Win32 {
    [DllImport("user32.dll")]
    public static extern bool MoveWindow(IntPtr hWnd, int X, int Y, int nWidth, int nHeight, bool bRepaint);
    [DllImport("user32.dll")]
    public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern bool BringWindowToTop(IntPtr hWnd);
    public static IntPtr HWND_TOPMOST = new IntPtr(-1);
    public static uint SWP_SHOWWINDOW = 0x0040;
}
'@
Add-Type -TypeDefinition $code -Language CSharp

# 移动窗口到固定位置并设置最顶层
[Win32]::MoveWindow($hwnd, 500, 300, 1195, 759, $true)
[Win32]::SetWindowPos($hwnd, [Win32]::HWND_TOPMOST, 500, 300, 1195, 759, [Win32]::SWP_SHOWWINDOW)
[Win32]::ShowWindow($hwnd, 5)   # SW_SHOW
[Win32]::ShowWindow($hwnd, 9)   # SW_RESTORE
[Win32]::BringWindowToTop($hwnd)
[Win32]::SetForegroundWindow($hwnd)
```

**步骤 2：截取指定区域**

```python
import pyautogui

# 截取窗口区域（位置 (500, 300)，尺寸 1195x759）
screenshot = pyautogui.screenshot(region=(500, 300, 1195, 759))
screenshot.save('output_path.png')
```

### 关键点

1. **MoveWindow**：强制移动窗口到固定位置（解决窗口位置不确定问题）
2. **SetWindowPos(HWND_TOPMOST)**：设置为最顶层（解决窗口被遮挡问题）
3. **截取固定区域**：使用 pyautogui.screenshot(region=...) 截取精确位置

### 错误方法

- ❌ 只调用 SetForegroundWindow（不足以让后台窗口显示）
- ❌ 截取全屏后裁剪（窗口位置不确定，裁剪位置错误）
- ❌ 使用 PowerShell 简单截图命令（窗口可能未激活）

---

## [EXE-SIZE-001] EXE 体积膨胀的优先排查顺序

**状态**：🟢 已分析（coder-1，2026-04-22）

- **当前证据**
  - `voice-ime-ui.exe` 约 21.39MB
  - `voice-ime.exe` 约 30.96MB
  - `ui/dist/assets` 仅约 193KB，不是体积主因
  - 主程序目录额外包含：
    - `onnxruntime.dll` 14.68MB
    - `sherpa-onnx-c-api.dll` 3.82MB
- **最高价值排查项**
  1. `src-tauri/Cargo.toml` 是否缺少独立 `[profile.release]`
     - 当前 UI 包没有自己的 `strip/LTO/codegen-units=1`
     - 若单独构建 `voice-ime-ui.exe`，主程序 `Cargo.toml` 里的 profile 不会自动继承
  2. 是否误用 `tokio/full`
  3. `reqwest` 是否保留默认特性，额外带入 `http2/system-proxy/default-tls`
  4. 是否存在重复网络栈（如 `reqwest + ureq + lettre`）
  5. 是否把“可选功能”直接链接进主 exe（如 crash reporter 的 `eframe/egui/lettre`）
- **建议顺序**
  - 先做 release profile 与 feature 收窄
  - 再做重复依赖收敛
  - 最后才做架构级拆分

### 2026-04-22 第一阶段实施补充

- 已落实：
  - `src-tauri` 独立 `[profile.release]`
  - `tokio` / `reqwest` feature 收窄
  - 主程序直接 `ureq` 依赖删除
- 新确认：
  - `ureq` 仍被 `sherpa-onnx-sys` build-dependency 传递引入
  - 因此“删除直接 `ureq` 依赖”是正确清理，但不是主要体积收益来源

### 2026-04-22 第二阶段 / 第三阶段分析补充

- `reqwest` 与 `lettre` 不是可以直接合并的重复网络栈：
  - `reqwest` 负责 LLM 的 HTTP 请求
  - `lettre` 只负责 crash reporter 的 SMTP 发信
  - 若要“统一网络层”，必须把 crash 上报方案改成 HTTPS 服务，而不是简单改 crate
- `lettre`、`eframe`、`egui`、`image`、`backtrace`、`chrono` 全都集中在 `src/crash/*`
  - 因此 crash reporter feature-gate / 独立产物化，是主程序继续减重的最高价值方向
- 当前 ASR DLL 体积：
  - `onnxruntime.dll` 14.68 MB
  - `sherpa-onnx-c-api.dll` 3.82 MB
  - 其余相关 DLL 合计后约 18.61 MB
- `sherpa-onnx` 当前显式使用 `shared`
  - 官方 crate 默认其实是 `static`
  - 切回 `static` 可行，但主要是把体积搬进 exe，不是当前阶段最佳 ROI
## [TRANS-CT2-001] ctranslate2-sys vendor include path bug on Windows

**Status**: fixed locally in this repo (coder-1, 2026-04-30)

- Symptom: `cargo check` / `cargo build` fails while compiling `ctranslate2-sys 0.1.5` with `fatal error C1083: cannot open include file: "ctranslate2/replica_pool.h"`.
- Root cause:
  - vendor mode downloads prebuilt CT2 artifacts into `target/<profile>/ctranslate2-vendor/`
  - headers are extracted under `target/<profile>/ctranslate2-vendor/include`
  - upstream `build.rs` incorrectly derives the include path from `.../ctranslate2-vendor/lib/include`
- Repo-local fix:
  - add `[patch.crates-io] ctranslate2-sys = { path = "patches/ctranslate2-sys" }`
  - patch `patches/ctranslate2-sys/build.rs` so vendor mode uses `found.parent()/include` instead of `found/include`
- Build prerequisite that still applies:
  - set `CMAKE` to `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe`
  - run cargo inside a VS BuildTools environment (`vcvars64.bat`) so `cl.exe` is available

### Follow-up findings from the same task

- Additional symptoms:
  - MSVC wrapper build fails unless `translator_wrapper.cpp` is compiled with `/std:c++17` and `/EHsc`
  - Windows shared vendor package exposes `ctranslate2.lib` but does not ship `ctranslate2.dll`, so `cargo test` fails at runtime with `STATUS_DLL_NOT_FOUND`
  - switching `ctranslate2-sys` features from shared vendor to static vendor can silently reuse the old `target/<profile>/ctranslate2-vendor` cache and produce wrong library layout assumptions
  - plain `static-windows-*` assets trigger `MT_StaticRelease` vs `MD_DynamicRelease` CRT mismatch during final link
- Additional repo-local fixes:
  - patch `patches/ctranslate2-sys/build.rs` to compile the wrapper with `/std:c++17` and `/EHsc`
  - vendor static linking now reads static libraries from `ctranslate2-vendor/lib` and import libraries / DLLs from `ctranslate2-vendor/dyn`
  - vendor DLLs are copied into both `target/<profile>/` and `target/<profile>/deps/`
  - vendor cache invalidation now keys off the resolved asset URL, so changing `shared` / `crt-dynamic` features forces a redownload
  - `Cargo.toml` enables `ctranslate2-sys` features `vendor + crt-dynamic`, which selects the Windows `static-crt` asset compatible with Rust's dynamic CRT build
  - `esaxx-rs` is patched locally to use `/std:c++11`, `/EHsc`, and non-static CRT on MSVC

## [NLLB-EVAL-001] Rust ctranslate2 high-level prefix path is not NLLB-ready

**Status**: identified during pre-PoC research (coder-1, 2026-04-30)

- Symptom:
  - the current project translation path uses `Translator2<Ct2Tokenizer>` and passes plain strings into `translate_batch`
  - this is sufficient for current `opus-mt` models, but not for `NLLB`, which requires explicit source/target language tags
- Concrete crate issue:
  - in `ctranslate2 2.1.1`, the low-level `Translator` exposes `translate_batch2(tokens, prefixes, options)`
  - but `Translator2::translate_batch_with_prefixes(...)` does not forward prefixes to the low-level translator
  - it still calls plain `translate_batch(...)` and only strips prefix-length tokens from the decoded output afterward
- Why this matters:
  - for `NLLB`, the target language token must actively constrain decoding
  - under the current high-level implementation, that constraint is effectively absent
- Additional stack limitation:
  - the current `hf::Tokenizer` wrapper only provides generic `encode/decode`
  - it does not expose `NLLB`-style `src_lang` / `tgt_lang` controls
- Practical workaround:
  - bypass `Translator2`
  - use the low-level `Translator::translate_batch2(...)`
  - manually construct source tokens as `[src_lang] + tokens + </s>` and target prefix as `[tgt_lang]`
- Recommendation:
  - do not treat `NLLB-600M CT2` as a drop-in model swap on the current stack
  - validate it first with a small PoC before any production migration

---

## [TRANS-CT2-DEBUG-001] Xenova tokenizer.json workaround does not make gaudi OPUS-MT CT2 runnable

**状态**：🟢 已定位并修复（coder-1，2026-05-01）

- **现象**：翻译热键触发后日志显示 `CT2 translator returned no hypotheses`，最终回退注入原文。
- **根因**：
  1. 旧实现将 `gaudi/opus-mt-*-ctranslate2` 模型与外部 Xenova `tokenizer.json` 拼接使用。
  2. 该 `tokenizer.json` 的 `normalizer` 为 `Precompiled` 且 `precompiled_charsmap = null`，Rust `tokenizers` 原始解析会 panic。
  3. 把整个 `normalizer` 设为 `null` 只能让 tokenizer 解析通过，但无法让 CT2 正常产出翻译。
  4. 进一步验证表明路线 A 也不可行：只删除 `precompiled_charsmap` 字段会直接 panic `missing field precompiled_charsmap`。
- **关键诊断结论**：
  - 旧路径 encode 后的 token 并不为空，实测为 `["▁", "是谁"]`，所以问题不是“空 token 列表”。
  - 正确修复不是继续 patch `tokenizer.json`，而是改走 SentencePiece 路线。
- **解决**：
  - 使用官方 `source.spm` 编码、`target.spm` 解码。
  - 从高层 `Translator2<Ct2Tokenizer>` 切换到底层 `ctranslate2::Translator`。
  - 在产品代码保留 `CT2 source tokens` 的 `info` 日志，方便后续运行态确认。
## [TRANS-CT2-EMPTY-002] CT2 returns zero TranslationResult even when source tokens are valid

- Symptom:
  - `CT2 source tokens` log shows a correct token list
  - `translate_batch(...)` still returns zero results and triggers `CT2 translator returned no hypotheses`
- Root cause:
  - the active `ctranslate2 2.1.1` Rust wrapper path had two bugs:
    - `TranslationOptions::default().max_batch_size = 0` was forwarded unchanged, while `ctranslate2-sys/cpp/translator_wrapper.cpp` rejects `max_batch_size == 0`
    - `prepare_string_pts()` returned pointers into a temporary `Vec<Vec<*const c_char>>`, so the following C call received dangling `const char***` pointers
- Fix:
  - bypass the buggy runtime wrapper in `src/translation/mod.rs`
  - call `ctranslate2_sys::translator_translate_batch(...)` directly for the single-source path
  - keep `CString`, token-pointer, and source-pointer ownership alive until the C call returns
  - use `max_batch_size = 1` for the product's single-batch translation path

---

## [ENCODING-UTF8-001] ⚠️ PowerShell Set-Content 破坏 UTF-8 文件编码【致命】

**状态**：🔴 致命问题（2026-05-02 OVERLAY-WAKE-001）

- **现象**：使用 PowerShell `Set-Content` 修改 Rust 源文件后，所有中文字符变成乱码（閸、鈧、顢等），编译报错 `prefix 'xxx' is unknown`
- **根因**：PowerShell 默认使用 **UTF-16 LE 编码（带 BOM）**，而 Rust 源文件是 **UTF-8**。混用导致编码转换破坏文件
- **禁止行为**：
  - ❌ `Set-Content` 直接修改 UTF-8 源文件
  - ❌ `Out-File` 不指定编码
  - ❌ PowerShell 字符串替换操作 UTF-8 文件
- **正确做法**：
  - ✅ WSL Python：`codecs.open(filepath, 'r', 'utf-8')`
  - ✅ PowerShell 加 `-Encoding UTF8` 参数
- **验证方法**：修改后立即 `cargo check`，出现 `prefix 'xxx' is unknown` 说明编码已损坏
- **强制规则**：修改源文件前必须确认工具保持 UTF-8 编码

---

## [RESEARCH-001] ✅ Worker 方案分析时可自行 WebSearch/WebFetch

**状态**：🟢 已授权（2026-05-02）

- **权限**：Worker 分析方案时如需技术调研，可自行使用 WebSearch/WebFetch 搜寻最优方案
- **场景**：
  - 查找 API 最佳实践
  - 确认库/框架版本兼容性
  - 搜索技术问题解决方案
- **限制**：最多 3 轮讨论，Orchestrator 最终决定为准

---

## [TRANS-REGRESSION-001] ⚠️ 本地翻译两个回归 BUG（2026-05-07）

**状态**：🔴 待修复

### BUG-1：翻译输出英文单词间空格丢失

- **现象**：中文→英文翻译后，英文单词之间没有空格（如 "HelloWorld" 而非 "Hello World"）
- **历史**：TRANS-SPACE-FIX-001 曾修复此问题（join("") → join(" ")），当前代码 `join(" ")` 仍存在，但回归
- **可能原因**：CT2 C FFI 路径 `translate_single()` 返回的 token 可能不是正确的 detokenized 字符串，或 `normalize_translation_output` 对某些 token 格式处理不当

### BUG-2：翻译输出被截断

- **现象**：语音翻译后，输出内容明显不完整，只翻译了一部分
- **可能原因**：`MAX_DECODE_STEPS=256` 不足，或 CT2 FFI 路径 `translation_result_output_size` 返回值不完整

### 回归根因

后续代码改动（CT2 C FFI 路径、标点补全等）可能影响了翻译输出的完整性或格式化

### 强制规则

**任何任务方案评估必须包含"不影响现有功能"的回归检查项**

---

## [FIRSTCHAR-002] ⚠️ 送气清声母字首字识别错误的新根因诊断（2026-05-27）

**状态**：🔴 待修复 — Gavin 反馈 D3 出包后"派对"试三遍仍错，"派"等字首字坏

### 现象（Gavin 2026-05-27 反馈）

- D3（FIRSTCHAR-FIX-004，时间戳精确清空）已出包，但"派对"连试三遍首字仍识别错误
- 关键观察：**只有某些字的首字坏**（"派"是典型），不是所有字

### 新根因诊断（orchestrator 代码审查 + 旧日志声学推理）

D1–D3 历轮修复都在调"录音保留哪一段 chunk"，但从未触碰以下两个**真正伤声母**的环节：

| 根因 | 概率 | 文件/行 | 说明 |
|------|------|---------|------|
| **R1：降采样无抗混叠滤波** | 高 | `src/audio/mod.rs:695 resample_linear` + `extend_samples:637` | 48kHz→16kHz 用裸线性插值，无 8kHz 低通抗混叠。4–12kHz 高频混叠（aliasing）折返污染 0–8kHz。送气清声母 /pʰ/(派)、/tʰ/(对)、/tɕʰ/(七)、/tʂʰ/(厂)、擦音 /s//ʂ/ 能量集中在高频 → 声母频谱被破坏，模型听到的"派"近似无声母的"爱/in"。**这精确解释"为什么偏偏是送气/清声母字"——浊音鼻音声母（码/那/忙）能量低频为主，不受混叠影响，故不出错。** |
| **R2：find_speech_anchor 能量门限削清声母** | 中 | `src/audio/mod.rs:681` | 仅冷启动 prime 路径触发。threshold=0.01 RMS 门限，送气清音起始能量低于门限，anchor 跳到元音起点，声母爆破/送气段被当静音 trim 掉。 |
| **R3：silence head 放大效应** | 中 | `src/main.rs:2561` | 转录前固定 prepend 200ms 纯静音头。R1/R2 削掉声母后，模型先看 200ms 死寂再直接元音，更易判成无声母音。 |

### 模型局限（放大器，非根因）

- 日志铁证（target/release/debug.log 5-14）：长句"就在比利去世的前一天…"完整正确；短词"当店费用"/"因商店费用"错。
- SenseVoice(int8) 靠上下文语言模型纠错，2字孤立短词几乎无上下文，声母稍损无法恢复——所有 ASR 短词天花板。
- 当前降采样缺陷（R1）正在人为压低这个天花板。

### 修复优先级（待 Gavin 拍板）

1. **R1 最高性价比**：resample_linear 改为带抗混叠低通的重采样（自写 FIR 低通+抽取，或引入 rubato/合适库）。注意跨平台 + exe 体积影响评估。
2. R2：清声母 anchor 放宽（门限降低 / 起点前扩 N ms / 改用过零率+能量联合检测）。
3. 验证手段：开 `-debug` 跑"派对"抓当前 D3 实测日志确证（现有日志为 D3 之前）。

### 2026-05-27 R1 出包后实测日志验证（FIRSTCHAR-FIX-005 已上线）

Gavin 用 `-debug` 实测 "派发"（Publish/debug.log 17:47）：

| 次 | 识别结果 | 重采样 | drain |
|----|---------|--------|-------|
| 1 | 爱发 ❌（派→爱 声母丢） | anti-alias ✓ | preserved 0 |
| 2 | 开发 ❌（派→开 声母错） | anti-alias ✓ | preserved 0 |
| 3 | 说过 ❌ | anti-alias ✓ | preserved 0 |
| 4 | 派发 ✅ | anti-alias ✓ | preserved 0 |
| 5 | 整句"我刚启动我已经测试过了你看下日子" **完全正确** ✅ | anti-alias ✓ | — |

**结论（修正先前判断）**：
- R1 抗混叠重采样已生效（每次日志均有 "Resampling … with anti-alias filter"），音频质量确已改善——**整句一字不差**即铁证。
- 但孤立短词"派发"首字仍 3 错 1 对，**主导因素修正为模型对孤立短词送气声母的固有局限**（无上下文 + /pʰ/ 声母弱短 + 模型按高频词先验补声母 → 爱发/开发），呈概率性，非确定 bug。R1 仍是真实缺陷且修复有价值，但不是短词错误的主因。
- R2（find_speech_anchor）在本组测试**未触发**：每次 pre_roll=60 chunks（满 600ms），走暖启动路径，不经 prime/anchor。故 R2 与本现象无关。

**新发现的可疑点（R3 升级，有日志依据待验证）**：
- 模型实际输入 = 200ms silence head（main.rs:2561）+ 600ms pre_roll 热键前背景静音 + "派发"语音。
- PTT 模式下用户先按键再开口，pre_roll 600ms 基本是静音 → "派"声母前挂了约 800ms 近静音，弱送气声母易被 offline 模型当静音延续吞掉。
- 假说：对短录音压缩/优化前导静音可能改善短词首字。低成本，1 轮可验证。待 Gavin 拍板（选项 A）。

### 2026-05-27 FIX-006（R2+R3）实测 + 第二步方案调研

**FIX-006 实测（Publish/debug.log 19:13，Gavin 测 13 遍"派X"词）**：
- "派"首字识别 7 对 6 错（约 54%），对比 FIX-005 前约 20%（5 遍 1 对）→ **2.7 倍提升**
- R3 生效确认：每次 trimmed leading silence 600–940ms，silence_head=50ms
- 失误模式：派→爱/哎（/pʰ/ 声母概率性丢失），同词有时对有时错 = 模型短词天花板
- 对：派发/派对/派出所/派发任务；错：爱发/哎对/爱出所

**第二步方案调研结论（2026-05-27 WebSearch sherpa-onnx 官方）**：
- ❌ **hotwords 不可用**：sherpa-onnx 仅 transducer 模型支持 hotwords（需 modified_beam_search 解码），SenseVoice 是 **CTC 架构，明确不支持**。原"第二步 hotwords"在不换模型前提下做不了。
- ⚠️ **pre-emphasis 高频预加重不推荐**：会改变喂给模型的频谱分布，SenseVoice 训练用标准 fbank，额外预加重造成训练-推理不匹配，可能反伤整体识别率。
- 剩余选项（按性价比）：① 接受现状（54% 已显著改善，高频词重说）；② 换非量化 SenseVoice（同架构换模型文件，精度或略升，代价体积/速度，幅度不保证）；③ 换 transducer 模型才能用 hotwords（换引擎+解码+大量回归，且 hotwords 仅覆盖预设词表，不推荐）。
- 已汇报 Gavin，待拍板（推荐 ①/②）。

### 关联

- 续 [FIRSTCHAR-001]，但根因层级不同：FIRSTCHAR-001 是"chunk 保留时序"，本条是"声母频谱质量 + 前导静音稀释 + 模型短词局限"。
- R1（FIX-005 抗混叠）+ R2/R3（FIX-006 前导静音规整+声母回溯）已上线，把送气短词首字正确率从 ~20% 提到 ~54%。剩余为模型固有天花板。

---

## [FIRSTCHAR-001] ⚠️ 首字识别间歇性丢失（已部分修复，备用方案存档）

**状态**：🟡 已实施 D1+D2 修复（2026-05-25），效果待用户验收

### 现象

本地语音识别（SenseVoice/Paraformer）首字符间歇性丢失或识别错误，不稳定——有时正常，有时不正常。此前经历多轮修复（PRE_ROLL_MS 调整、WASAPI prime、PREROLL-RINGBUF-001 环形缓冲区），问题仍未根治。

### 根因分析（2026-05-25 双路研究）

| 根因 | 概率 | 说明 |
|------|------|------|
| C1：idle_cleared 竞争窗口 | 高 | `while rx.try_recv().is_ok()` 无限清空 channel，在热键后 1-20ms 内到达的首字 chunk 被误丢 |
| C2：Prime trim 截首字 | 中 | pre_roll 为空时走 Prime 路径，trim 保留 TAIL 导致 HEAD（首字起始）被丢弃 |
| C3：stream 重建后 pre_roll 为空 | 中 | stream_failed 重建后 pre_roll 从零积累，必走 Prime 路径，叠加 C2 风险 |
| C4：channel try_send 静默丢弃 | 低 | channel 满（256 chunks ≈ 2.5s）时 try_send 静默丢弃，正常操作不触发 |
| C5：VAD 误触发 speech_detected | 低 | pre_roll 噪音超 threshold 时，后续词间停顿可能提前终止录音 |

### 已实施修复（D1+D2）

**D1 — idle_cleared 限量清空（src/audio/mod.rs L120-146）**：
- 旧：`while warm.rx.try_recv().is_ok()` — 无限清空所有积压 chunk
- 新：按 `pre_roll_budget = pre_roll_samples(sample_rate, PRE_ROLL_MS)` 限量清空，清完 pre_roll 等量样本后立即 break
- 效果：消除 C1 竞争窗口，热键后到达的首字 chunk 不再被误丢

**D2 — Prime trim 保头部（src/audio/mod.rs L465-484 + find_speech_anchor L647-663）**：
- 旧：`prime_samples[len - target..]` — 保 TAIL，首字在 HEAD 被丢
- 新：`find_speech_anchor`（160 样本窗口 RMS 扫描）定位语音起点，从起点开始保留 target_samples
- 效果：消除 C2，冷启动场景首字不再被 trim 截断

### 备用方案（若 D1+D2 效果不足）

**D3 — 时间戳标记精确清空（针对 C1 更彻底方案）**：
- 方案：WASAPI 回调为每个 chunk 打上 `std::time::Instant`，`idle_cleared` 只清热键触发时刻之前的 chunk
- 效果：比 D1 更精确，完全消除竞争窗口，无需依赖样本数估算
- 成本：需修改 chunk 类型（从 `Vec<f32>` 改为 `(Instant, Vec<f32>)`），影响三个回调、channel 类型、`drain_pre_roll`、`collect_recording`，工程量中等
- 风险：channel 类型变更需同步修改所有消费端

**D4 — stream 重建后异步 pre_roll 预热（针对 C3）**：
- 方案：`ensure_stream` 重建成功后，新增 `pre_roll_ready: Arc<AtomicBool>` 标志，worker idle 循环等待 pre_roll 积满 600ms 后再置为 ready；`record()` 检查此标志，pre_roll 未 ready 时等待而非立即进入 Prime
- 效果：消除 C3，stream 重建后第一次录音也能从完整 pre_roll 开始
- 成本：需修改 `WarmInputStream` 结构 + `check_stream_health` + `record()` 调用侧

**D5 — VAD grace period（针对 C5）**：
- 方案：`RecordingState` 新增 `speech_grace_samples: usize`，`speech_detected` 首次置 true 后，接下来 N ms（建议 500ms）内不启动静音计数
- 效果：消除词间短暂停顿导致录音过早结束的误判
- 成本：仅改 `RecordingState`，低风险

**D6 — 双写改单写（架构根治，长期）**：
- 方案：移除 WASAPI 回调对 channel 的 try_send，`collect_recording` 改为直接从 pre_roll 环形缓冲区以阻塞方式读取（如用 `Condvar` 通知有新数据）
- 效果：消除 C1 根本原因（不再有 dual-write），idle_cleared 逻辑整体删除
- 成本：大幅重构 `WarmInputStream`，影响 `collect_recording` 整体设计，高风险

### 验收标准

用户反馈首字识别稳定（连续 10 次以上热键触发均正常识别首字）后，本条目状态改为 🟢 已解决。

若 D1+D2 后仍有不稳定，按 D3 → D4 → D5 顺序逐步追加修复。

---

## [BUILD-PUBLISH-001] ⚠️ 出包后未同步 EXE 到 Publish/ 目录

**状态**：🟢 已修复（2026-05-14）

- **现象**：BUILD-RELEASE-20260514B 出包完成，但 Publish/ 目录仍为旧版本 EXE
- **根因**：tester-1 执行构建按 build-test-guide.md 流程，未包含 Publish/ 同步步骤
- **修复**：build-test-guide.md 新增 Step 4（Publish 同步），出包任务强制执行

---

## [ASR-LONG-AUDIO-001] ✅ VAD 分段转录根治 native 长音频上限【必读】

**状态**：🟢 已实施（2026-07-06 coder-1）

- **背景**：ASR-NATIVE-LONG-001 定位根因 native `max_total_len=512` 限制 ~28s 音频截断空输出
- **方案**：VAD 分段（DEC-026，路径 A）—— silero VAD 切分长音频为 ≤20s 段，逐段转录拼接
- **实施**（仅 accuracy 分支，performance 不碰）：
  - 新增 `src/transcription/vad.rs`：VadSegmenter（懒加载）+ 分段纯函数
  - `transcribe_offline` accuracy 长音频(>24s) → VAD 切分 → 逐段 transcribe_segment（含三重兜底）→ 拼接
  - 短音频(≤24s)走原单次路径不变
  - VAD 缺失/失败 → 降级单次转录（有三重兜底垫底）
  - 拼接：中文直接连接；段尾段首均拉丁字母间补空格
- **VAD 模型**：`models/silero-vad/silero_vad.onnx`（643KB，sherpa-onnx 官方）
- **参数**：触发阈值 24s / 段上限 20s / padding 200ms / min_silence 300ms / threshold 0.5
- **验证**：PoC bin 实测 30/60/90s 切分正常（最大段 6.1/9.7/18.4s 均 <20s）；cargo test 337/0/4（含 14 个 vad 单测）
- **下游零影响**：transcribe() 签名不变，performance 分支逻辑不变

---

## [ASR-NATIVE-LONG-001] ⚠️ FunASR Nano native 长音频空输出根因 + 兜底加固【必读】

**状态**：🟢 已定位 + 兜底加固（2026-07-06 coder-1）

- **现象**：Gavin 端测 0.6.0 accuracy（native）模型反馈两类异常：① 长段语音输出含乱码 ② 长段语音输出为空
- **根因**（PoC bin debug 日志铁证）：
  - FunASR Nano native 模型 `max_total_len=512`（KV cache 容量上限，模型导出时设定）
  - 音频经 LFR 降采样后，约 28s 以上音频 context_len（prompt ~18 + audio tokens + after ~5）> 512
  - 超限时 C++ 截断 audio placeholders（`Truncating audio placeholders: audio_token_len=N -> keep_audio=M`）
  - 截断后 decoder 生成 0 token → `generated 0 tokens` → 空文本输出
  - 不是 trailing silence 本身，不是循环重复，是 `max_total_len` 硬限制
- **实测阈值**：
  - 27.88s（context_len=487）正常
  - 29.88s（context_len=520）截断 → 空输出
  - performance（CTC）模型无此限制，30s/60s/90s 全部正常
- **兜底加固**（src/transcription/mod.rs）：
  1. **空输出兜底**：accuracy 输出 trim 为空 → fallback performance 重转
  2. **hallucination 判定增强**：`is_repetitive_garbage` n-gram 环路检测（子串连续重复 ≥4 次且占比 ≥40%）
  3. **终极保底**：fallback 重转后仍空/仍异常 → 返回 Err（绝不静默注入垃圾文本/空文本）
  4. 8 个新增单测覆盖空/环路/正常不误伤
- **根治方案调研**：`collab/research/asr-long-audio-chunking.md`（VAD 分段/固定窗/增大 max_total_len 三路径，推荐 VAD 分段）
- **强制规则**：accuracy 模型长音频异常不得静默通过——必须降级 fallback 或返回 Err

---

## [ASR-RELOAD-001] ⚠️ 异步重建 Transcriber 的并发防护与状态一致性【必读】

**状态**：🟢 已修正（2026-07-06 Orchestrator 验收 ASR-DUAL-B-001 时发现并直接修正）

- **现象**：coder-1 初版热重载用 `asr_reload_rx.is_empty()` 判断是否已有重建在途，但 channel 容量 1 + 6s 构建窗口，is_empty() 在构建线程 send 之前一直为 true，挡不住重复 spawn（并发加载多个 972MB 模型，内存爆炸）。同时 `active_language` 在触发重建时 eager 更新，而 `active_asr_model`/`active_hotwords_version` 在 swap 时延迟更新——重建失败后 language 变更永不重试。
- **根因**：
  1. channel 非空 ≠ 构建在途（构建线程 send 前为空）
  2. 触发条件检查与状态更新时机不统一（eager vs lazy 混用）
- **正确修复**（Orchestrator）：
  1. 新增 `asr_reload_in_flight: bool` 独立标志，spawn 时置 true，channel 改传 `Result<Transcriber>`（成功/失败都回信号清标志）
  2. `active_asr_model` / `active_language` / `active_hotwords_version` 统一在 swap 时更新（新增 `Transcriber::language()` getter）
  3. 重建失败：清 in_flight 标志 + 保留旧实例 + log::warn，下次 Start 可重试
- **强制规则**（同类异步重建实现必须遵守）：
  1. **并发防护用独立标志，不要用 channel 状态推断**（channel 非空 ≠ 构建在途）
  2. **标志必须在 Result 回传时清除**（成功和失败都回信号，否则失败后永久卡死）
  3. **active_* 状态统一在 swap 时更新**，不要 eager 更新部分字段（避免失败后状态不一致导致永不重试）
  4. 若重建对象有多个触发维度（model/language/hotwords），swap 时需从新实例读取全部维度值（getter 模式）

---

## [BUILD-PUBLISH-001] ⚠️ 出包后未同步 EXE 到 Publish/ 目录（旧条目保留）
- **强制规则**：所有出包任务完成后，必须执行 cp target/release/*.exe Publish/ 并验证时间戳

---

## [BUILD-FIX-SYNC-001] ⚠️ 包改名后 target 残留旧名 exe，cp 刷新 mtime 使时间戳检查失效【必读】

**状态**：🔴 问题暴露（2026-07-06 BUILD-FIX-ASR-DUAL-001）

- **现象**：Publish/ 中 `feiyin-ime-ui.exe` 未被正确覆盖，实际同步的是 `target/release/` 中旧版 `voice-ime-ui.exe`（项目从 voice-ime-ui 重命名为 feiyin-ime-ui 前的残留），cp 命令刷新了 mtime 使其伪装成新构建
- **根因**：
  1. 项目重命名后 `build-guide.md` Step 4 的 cp 命令未同步更新（仍用 `voice-ime-ui.exe`）
  2. `target/release/` 中残留旧名 exe（cargo build --release 生成 `feiyin-ime-ui.exe`，但旧 `voice-ime-ui.exe` 不会被自动删除）
  3. `cp` 命令静默覆盖时刷新 mtime → 时间戳检查无法分辨新旧
  4. 验证只检查 mtime 和 size，未核对文件名与 `Cargo.toml package name` 是否一致
- **强制规则**：
  1. 产物同步后必须核对文件名与 `Cargo.toml` 的 `[package] name` 一致
  2. 对照构建日志实际输出路径（`src-tauri/target/release/`），确认正确产物名
  3. 验证不能用 `ls -la`（mtime 被 cp 刷新），必须用 `(Get-Item).VersionInfo.ProductVersion`
  4. 每次构建后检查目标目录是否有旧名残留（`voice-ime-ui.exe`），有则删除
  5. `build-guide.md` 中的 cp 命令必须与最新 `package name` 同步

---

## [COLLAB-WRITE-001] ⚠️ OpenCode Worker write 工具用 MSYS 路径静默失败

**状态**：🟢 已定位（2026-07-06）

- **现象**：coder-1 (OpenCode) 自报报告已写入并 ✅，但 `/d/Workspace/.../collab/research/qwen3-asr-feasibility.md` 实际不存在（目录被创建但文件 0 产出），result.md 同样 0 字节；logs/CHANGELOG（编辑已有文件）正常
- **根因**：OpenCode 的 write 工具对 MSYS 风格路径 `/d/...` 新建文件时**静默失败**（无报错），改用 Windows 路径 `D:\...` 后落盘成功
- **教训**：
  1. Worker 写新文件后必须自行 `ls -la` + `wc -l` 核实非空再报完成
  2. Orchestrator 验收必须实际 Read 产出文件，不信自报 ✅（本次靠验收拦截）
  3. 给 OpenCode Worker 的任务书中，新建文件路径建议写 Windows 风格 `D:\...`

> ### ⚠️ 鉴别诊断（2026-07-31 新增，报「本条复发」前必读）
>
> **本条 2026-07-06 的原始定性有实证支撑**（当时特征是「目录被创建但文件 0 产出」，即文件在**任何路径下都不存在**）。
> **但后续被记为「本条复发」的若干次，至少 2026-07-31 那次经查证不是本条，而是 [COLLAB-PATH-SPLIT-001] 的路径分叉。**
>
> **两者现象相同（`collect` 读到 0 字节），根因与处置完全不同**：
>
> | 判据 | 本条（写工具失败） | [COLLAB-PATH-SPLIT-001]（路径分叉） |
> | --- | --- | --- |
> | 文件在**另一处 collab** 是否存在 | 否，全盘搜不到 | **是，且内容完整** |
> | Worker 自报的 `wc -c` | 与实际一致（0 或报错） | **与实际一致（非 0），Worker 没说谎** |
> | 处置 | 改 Windows 路径重写 | **改读取路径，Worker 无需重写** |
>
> **报「复发」前必做的一步**：
> ```bash
> find /d/Workspace/CodeLab -name "result.md" -newermt "-30 minutes" | while read f; do echo "$(wc -c <"$f") B  $f"; done
> ```
> 若在项目级 `voice-ime/collab/outbox/` 找到非空同名文件 → 是路径分叉，**不是本条**，且**不应要求 Worker 用 WSL Python 重写**（那是在治错的病，2026-07-31 主控犯过这个错）。

---

## [COLLAB-PATH-SPLIT-001] ⚠️ 存在两套 collab 目录，`dispatch.sh` 与 Worker 各写一套

**状态**：🟡 已定位，未修（2026-07-31）

- **现象**：Worker 自报 `result.md` 已写入且 `wc -c` 非零（诚实无误），主控 `collect <id>` 读到 **0 字节**。三个 Worker 均受影响：

  | Worker | 工作区级 `/d/Workspace/CodeLab/collab/outbox/` | 项目级 `voice-ime/collab/outbox/` |
  | --- | --- | --- |
  | coder-1 | 2286 B（旧）→ 按任务书写此处，正确 | 2328 B（内容不同，一度更新） |
  | coder-2 | **0 B** | 8131 B（真实交付） |
  | tester-1 | **0 B** | 1251 B |

- **根因（两条叠加）**：
  1. `dispatch.sh:2` 写死 `COLLAB="/d/Workspace/CodeLab/collab"`（**工作区级**），`dispatch` 与 `collect` 都只认这一处
  2. Worker 的 cwd 是**项目目录** `D:\Workspace\CodeLab\voice-ime`。任务书若用**相对路径** `collab/outbox/<id>/result.md`，即解析到**项目级** → 与框架读取处不是同一个文件
  3. 加重因素：`dispatch()` 派发时执行 `> "$result_file"`，会把**工作区级**那份**截断为 0 字节** —— 于是 0 字节文件是框架自己造出来的，而非 Worker 写失败

- **为什么会误判**：现象（0 字节）与 `[COLLAB-WRITE-001]` 完全一致，且 `[COLLAB-WRITE-001]` 已有 3 次「复发」记录，形成锚定效应。2026-07-31 主控据此要求 coder-2 用 WSL Python 重写，**处置方向错误**；coder-2 重写后主控仍读到 0 字节，才反查出真因。

- **处置**：
  1. **短期（已执行）**：任务书交付物一律写**工作区级绝对路径** `/d/Workspace/CodeLab/collab/outbox/<id>/result.md`
  2. **不要求 Worker 向两处 cp** —— 两处并存会制造「主控读到旧内容」的新风险（coder-1 一度这样做，已叫停）
  3. **长期（待 Gavin 拍板是否现在动）**：统一 collab 目录，或让 `dispatch.sh` 的 `COLLAB` 跟随 Worker cwd 解析。属协作框架变更，会影响在途任务，需择期

- **教训**：
  1. **Worker 自报数字与主控读数不一致时，先查「是不是同一个文件」，再怀疑写失败** —— 两个路径下的同名文件是最容易被忽略的分叉
  2. 已有 troubleshooting 条目会产生锚定效应，**现象相同不等于根因相同**；报「复发」前必须走鉴别诊断
  3. 框架层的路径不一致会**持续污染每一次验收**，且伪装成 Worker 的错误——这类缺陷的代价不在单次，在于它让主控对 Worker 的信任判断长期失真

---

## [ASR-NATIVE-LONG-001] ⚠️ accuracy（native）模型长音频乱码/空输出

**状态**：🔴 调查中（2026-07-06 Gavin 端测 0.6.0 反馈）

- **现象**：① 长段语音输出含乱码/大量错误文本（体感准确率低于 performance）② 长段语音输出为空
- **根因假设**：native = LLM decoder（Qwen3-0.6B）架构，长音频固有弱项；PoC qidian_v1 RTF 2.66 乱码为先例。PoC 仅覆盖短词短句，长音频行为未测——测试设计缺口
- **兜底盲区**（代码确认）：
  1. is_hallucination 只判字符数/秒 > 12：长音频字数预算大，乱码在预算内直接放行
  2. 空输出无兜底分支：0 字不超阈值，原样返回
- **处理**：ASR-NATIVE-LONG-001 已派发 coder-1（调查长音频阈值/max_new_tokens 截断 + 空输出兜底 + n-gram 环路检测 + 分段转录调研报告）
- **用户侧临时缓解**：长段输入切回 performance（CTC 耐长音频）

---

## [TELEGRAM-CHANNEL-001] Telegram 消息收不到的排查路径

**状态**：🔴 根因升级（2026-07-07）：非本地问题，Claude Code 2.1.202 服务端功能开关拦截

- **2026-07-07 复查**：会话已带 `--channels` 参数重启，本地全链路逐项实测健康——token 有效、插件 0.0.6 完整（server.ts 已声明 claude/channel capability）、bun 在 Windows 机器级 PATH、**手动原样执行 spawn 命令（`bun run --cwd <插件目录> --shell=bun --silent start`）完全正常**（bot @gavincc_bot 上线并拉到未读消息）——但 Claude 会话内仍无 bun 子进程
- **新根因**：Claude Code 于 2026-07-07 11:54 自动更新至 2.1.202，channels 注册前新增 5 道闸门（反编译确认）：provider 检查 → **feature gate `tengu_harbor`（statsig 服务端开关，默认 false）** → org policy → --channels 列表 → **allowlist `tengu_harbor_ledger`（statsig 下发批准清单，默认空）**。gate 未开启或 statsig 拉不到值时，通道静默 skip（"channels feature is not currently available"），无任何本地报错
- **本地无解**：无 managed-settings、非第三方 provider，两道拦截均为服务端控制
- **可选路线**：① 降级 Claude Code 到 channels 未加闸门的旧版本 ② 等 Anthropic 放开 gate / 更新 telegram 插件 ③ 临时手动 bun server.ts 轮询（无 MCP 桥接，只能收不能自动回）

**2026-07-06 首次排查（结论"重启会话即可"已被 07-07 实测推翻）**：

- **现象**：Gavin 的 Telegram 消息 orchestrator 收不到
- **排查结论**（逐项验证）：token 有效（getMe 实测通过）/ 插件 v0.0.6 已装 / bun + PATH 正常 / server.ts 手动启动无报错 → **全部健康**；但会话内无 bun 子进程、无 telegram MCP 工具 = 通道连接未建立
- **根因**：通道 MCP server 只在 claude 会话启动时随 `--channels` 拉起，本会话启动时未建立（或中途死亡），无自愈；`/reload-plugins` 不会重建通道
- **修复**：重启 orchestrator 会话（同参数）；重启后用 `tasklist | findstr bun` 验证
- **排查捷径**（下次直接按此顺序）：① tasklist 查 bun 进程 ② curl getMe 验 token ③ 手动 bun server.ts 看报错

---

## [ASR-HALLUC-SEGMENT-001] ⚠️ accuracy 长音频 VAD 分段中段幻觉逃过三重兜底

**状态**：🔴 已定位（2026-07-07 Gavin 端测实锤），待立项修复

- **现象**：40.1s 长语音（accuracy 模式）→ VAD 切 3 段 → 中段输出被 native decoder 幻觉整段替换（"修行顺风队，despite ok拦截了…c罗疲惫兮兮…南安普敦…"等足球语域乱串），首尾两段正常
- **日志证据**：target/release/debug.log L1157-1159（2026-07-07 17:14 本地）——"VAD segmented 641280 samples (40.1s) into 3 segments"，拼接输出中段为幻觉文本
- **为什么三重兜底没拦住**（逐项核对）：
  1. 字/秒阈值：中段 ~13s 输出 ~100 字 ≈ 7.7 字/s < 12 阈值 → 不触发
  2. is_repetitive_garbage：幻觉内容语义乱但**不重复**（无 n-gram 环路）→ 不触发
  3. 空输出检测：非空 → 不触发
- **本质**：RESEARCH-ASR-ACCURACY-001 R3 与 ASR-NATIVE-LONG-001 已预警的"低速率幻觉盲区"在真实端测命中——现有检测只覆盖高速率/重复/空三种形态，语义级幻觉（流畅但无关）无手段
- **候选修复方向**（待评估立项）：accuracy 每段转录后用 CTC（RTF~0.02 极快）对同段交叉转录，字符重合度低于阈值判幻觉→ 采用 CTC 结果；开销极小且 CTC 从不幻觉，可根治段级幻觉
- **用户侧临时缓解**：长段输入用 performance（CTC 耐长音频且无幻觉）
- **排期决策（2026-07-25 Gavin 拍板）**：**暂不排期**。理由：accuracy 模型已通过 ASR-HIDE-ACCURACY-001 从 UI 下拉隐藏（存量配置静默迁移为 performance，见 config/mod.rs:370-434），本问题的触发路径对用户已不可达，风险实际归零。**复活条件**：若未来重新开放 accuracy 选项（或引入其他 LLM-decoder 类 ASR 模型），本条必须同批立项修复——语义级幻觉现有三重兜底（字/秒、重复、空输出）全部拦不住，属已知无防护缺口



**状态**：🟡 修复中（2026-07-07 Gavin 端测 B-002 发现）

- **现象**：配置界面内的外部链接（accuracy 模型下载链接）点击无任何反应
- **根因**：Tauri WebView 拦截外部导航，`<a href target="_blank">` 不会打开系统浏览器
- **正确做法**：走后端命令 `invoke('open_url_in_browser', { url })`（version_check.rs:110 已注册，Windows 用 cmd start；About 页升级下载为正确参照）
- **规则**：UI 中所有需要打开系统浏览器的链接，一律用 open_url_in_browser 命令，禁止裸 `<a target="_blank">`
- **验收教训**：Vitest 只能断言元素存在，"点击开浏览器"行为必须目视端测——再次印证 feedback_ui_visual_verification

## [COLLAB-ACK-001] ⚠️ ACK_FAIL ≠ Worker 僵死，重启前必须 capture-pane 确认

**状态**：🟢 已处置（Orchestrator，2026-07-07）

- **现象**：dispatch.sh 报 coder-2 三次发送无应答（ACK_FAIL），疑似僵死
- **实际**：capture-pane 显示 Worker 存活且正在处理刚派发的任务（跳过了回 ACK 直接开工），重发消息仅进入输入队列（QUEUED，同任务书无害）
- **教训**：
  1. ACK_FAIL 只说明 Worker 没走 ACK 流程，不代表没收到任务或已僵死
  2. 重启前必须 capture-pane 检查：spinner 活跃 + 上下文在增长 = 正在工作，禁止重启（会丢工作上下文）
  3. 只有屏幕停滞、报错循环、进程退出才判僵死重启
- **改进方向**：Worker 注入词强化"收到任务第一动作是回 ACK + 写 ack 文件，然后才开工"

## [WORKER-HANG-001] OpenCode 云模型调用假死的判定与唤醒

**状态**：🟢 处置方法已验证（2026-07-08，两起：coder-1/GLM-5.2、tester-1/kimi-k2.7）

- **现象**：Worker 屏幕完全冻结（含 spinner 计时器不走），dispatch 消息滞留输入队列显示 QUEUED，ACK 超时告警
- **判定**：间隔 12-15s 两次 capture-pane 取 md5 比对，一致=冻结；注意与"正常长思考"区分——正常时计时器/进度条会动
- **唤醒**：`tmux send-keys -t <pane> Escape` 中断挂起的模型调用 → 等 5-20s 再次 md5 比对确认恢复 → 若队列消息在，会自动开始处理；若仍冻结，注入续做提示（说明已完成部分，剩余步骤）+ Enter
- **两起案例均无上下文损失**，中断后续做成功；无需重启 Worker
- **注意**：ACK_FAIL ≠ 僵死。coder-2 三次 ACK_FAIL 均为"收任务直接开工不回 ACK"的行为模式（进程活跃），先 capture-pane 判定再处置

### 补充（2026-08-03，coder-1）：🔴 **Escape 唤醒法失效的顽固形态 + 静默吞消息**

- **现象**：coder-1 完成 017 自验后进入冻结，spinner 计时器锁死在 `4m 17s` 不再走动，状态行 `54.3K (27%)` 恒定；两次间隔 15s 的 capture-pane md5 **完全一致**，判定冻结成立
- **与已知形态的差别（关键）**：
  1. **Escape 唤醒失效**——第一次 Escape 后 spinner 短暂消失（误以为已恢复），约 10s 后画面**原样回退**到 `4m 17s`；再试 `Escape ×2 + Enter` 完全无效
  2. **消息被静默吞掉，且屏幕无任何痕迹**——期间主控发出 4 条消息（`/permissions Full Access` + dispatch 通知 + 2 条纠正），输入框既不显示文本也不显示 QUEUED，pane 内容零变化。**不像已记录形态那样"滞留输入队列显示 QUEUED"，而是完全无痕**
  3. 底部同时显示两个模型名（`DeepSeek V4 Flash Free` 与 `glm-5.2 Ollama Cloud`），疑似模型切换/fallback 期间卡死，但**未确证**
- **危害**：主控会以为消息已送达 Worker。本次是 Gavin 主动提醒「coder1 没收到你的协作消息」才发现——**`tmux send-keys` 返回成功 ≠ Worker 收到**，这一条已在 `feedback_collab_dispatch_timing` 记过，本次是更隐蔽的版本（连屏幕痕迹都没有）
- **本次处置**：判定 coder-1 无待办任务（017 已由主控提交 `9eb80b7`）、不在关键路径上（tester-1 正常运行），故**不重启、不注入上下文**，留待需要时再处理。避免为一个闲置 Worker 打断正在跑的回归
- **待验证的处置手段**（下次遇到可依次尝试）：`send-keys -t <pane> C-c` / `q` / 直接 `respawn-pane`；本次未试，因无收益
- **主控侧强制纪律（本次新增）**：向 Worker 发送**任何**关键消息后，必须 `capture-pane` 确认屏幕有变化，才能认为送达；不得凭 `send-keys` 退出码判断

---

## [TOML-STALE-001] ⚠️ target/release 外置规则 toml 陈旧会静默覆盖新内置默认【必读】

**状态**：🟢 已修复 + 教训固化（2026-07-13 TEST-EXEC-FMT-005 验收发现）

- **现象**：ITN-SMART-002（07-13）给 itn-rules.toml 新增 historical 95 条并同步了根目录与 Publish/，但 target/release/itn-rules.toml 仍是 07-10 旧版（5256B vs 7298B）
- **危害**：DEC-030-⑥ 外置规则机制是"外部文件存在则**覆盖**编译期内置默认"——exe 里明明嵌入了新规则，旁边的陈旧 toml 会把它静默压回旧版。Gavin 从 target/release 启动的 debug 实例因此一直用旧 ITN 词表，期间的 ITN 端测结论失真
- **根因**：规则 toml 修改后的同步清单只覆盖了「根目录 + Publish」两处，遗漏 target/release（三副本规范是 根/Publish/target-release）
- **修复**：cp 根目录版本到 target/release，三副本 sha256 一致后重启实例
- **强制规则**：
  1. 任何外置规则 toml（itn-rules/scene-rules）修改后，必须同步**三副本**并 sha256 逐一核验，不能只对比其中两处
  2. 出包/验收时把「三副本 toml sha256 一致」列入固定检查项（本次 tester-1 例行核对发现，值得保持）

## [COLLAB-ACK-001] ⚠️ dispatch.sh ACK_FAIL 误报：Worker ack 文件 0 字节

**状态**：🔴 已定位根因，待修复（2026-07-14）

- **现象**：coder-2 已通过 tmux 发送 ACK 且正常开始执行任务，但 dispatch.sh 后台监控报「经 3 次发送均无应答」
- **根因**：Worker 写入 acks/<id>/task_ack.md 时落盘 0 字节（与 [COLLAB-WRITE-001] tester-1 result.md 0 字节同模式，疑似 OpenCode 写文件工具对该路径的写入缺陷），监控读不到有效内容判定 ACK 失败并重发任务 3 次
- **影响**：① 误报打扰主控 ② 任务消息被重发 3 次，可能在 Worker 输入队列堆积重复任务（本次 coder-2 未受干扰，需持续观察）
- **临时处置**：收到 ACK_FAIL 先 capture-pane 看 Worker 实况，若在正常执行则忽略误报，禁止盲目重启/重派
- **待修复方向**：dispatch.sh 监控逻辑增加 tmux pane 内容双重确认；或 ack 检测改为「文件 mtime 更新」而非「内容非空」

### [COLLAB-ACK-001] 补充（2026-07-14）：tester-1 OpenCode TUI 二次僵死 + 有效 workaround

- 复现：tester-1 重启后收到 1 条消息并回复，随后 TUI 输入事件循环挂死——send-keys / paste-buffer / Escape 全部无效（capture-pane 正常，屏幕静止，token 计数不变）
- 特征：输入框 footer 模型标签变为「DeepSeek V4 Flash Free OpenCode Zen」与会话模型不一致时，大概率已挂死
- **有效 workaround**：replace-worker 重启后**立即** dispatch（就绪后第一条消息必达）；不要先发待命/寒暄消息再派发
- 待查根因：疑似 OpenCode 特定 provider/model 响应异常导致 TUI 事件循环卡死；coder-1/coder-2（同 OpenCode，不同模型配置）无此问题

---

## [GIT-RESET-INCIDENT-001] 🔴 批量 git checkout -- / stash 导致 11 个已验收批次丢失【必读】

**状态**：🟢 已恢复 + 红线固化（2026-07-24；2026-07-25 补录本条目——此前 todo.md 已引用该编号但 troubleshooting 中缺失）

- **现象**：2026-07-24，coder-2 为排查一个测试失败，误判他人未提交的改动为"自己误触"，执行批量 `git checkout -- <大量文件>` + `git stash`/`git stash pop`，工作区被强制回退到 07-11 最后一次提交（f10c1e0），2026-07-13～07-14 共 11 个已验收批次（8 后端 + 3 前端）的改动全部丢失
- **丢失范围**：src/main.rs、src/config/mod.rs、src/llm/mod.rs、src/itn.rs、ui/src/pages/Llm.tsx 等；涉及 SCENE-SENSE-001/002、FORMAT-LLM-001、LANG-AUTO-001、FMT-LLM-002~005、ITN-SMART-002 等批次
- **根因**：① Worker 在不确定改动归属时自行动手清理，而非上报主控 ② 已验收批次长期悬空在工作区未 commit（07-11 之后两周无提交），爆炸半径被放大到极限
- **恢复**：立项 REBUILD-LOST-001，按**原始实施时间顺序**（非 handoffs 倒序）重新派发后端 7 项（coder-1）+ 前端 3 项（coder-2），确保每步建立在正确前置代码基础上；SCENE-SENSE-001-UI/002-UI 净效果为零 + scene-rules.toml/src/scene/ 模块本体幸存，跳过不重做。最终 f2240b7（44 文件 +5883/-2323）一次性提交并 push
- **强制红线**（已写入 worker-guide.md §十二，任何 Worker 无一例外）：
  1. 禁止 `git reset` / `git checkout -- <path>` / `git restore <path>` / `git clean` / `git stash` / `git commit --amend`
  2. 唯一允许：`git status` / `git diff` / `git log` / `git show`（只读排查）
  3. 遇到"这改动是不是我误触的"疑惑 → 先 `git diff` 摸清范围 → tmux 上报主控 → 由主控逐文件核实后决定处置
- **Orchestrator 侧配套责任**：批次代码任务验收后应尽快推动 git commit，不让改动无限期悬空（本次事故直接教训）
- **关联**：worker-guide.md §十二｜handoffs 2026-07-24 REBUILD-LOST-001 / GIT-AUDIT-001 条目

### [GIT-RESET-INCIDENT-001] 追加伤亡（2026-07-25 发现）：CHANGELOG.md v0.7.0 整段丢失

- **现象**：07-25 例行核对发现 `CHANGELOG.md` 中**没有 `## v0.7.0` 段**——07-13/07-14 两天的全部任务记录（测试同步/测试执行/构建出包/词表热修/审计类共 15 条）无一存留
- **根因**：CHANGELOG.md 是 git 跟踪文件且当时有未提交改动，`git checkout --` 把它一并回退到 07-11 的 f10c1e0 状态。而 REBUILD-LOST-001 重做批次只重做了**代码**（后端 7 项 + 前端 3 项），CHANGELOG 的记录类内容不在重做清单内，因此从未恢复。对比之下 `logs/20260713.md`/`logs/20260714.md`/`collab/logs/20260713.md` 属 07-11 之后新建的**未跟踪文件**，`git checkout --` 不作用于未跟踪文件，故完整幸存
- **代码侧复核结论（无回归）**：重做清单未包含的 FMT-LLM-003、SCENE-TITLE-CASE-001、AUDIT-SCENE-RULES-001、EMAIL-COLON-HOTFIX 四项逐一 grep 验证仍在位——`src/llm/mod.rs:581`（OUTPUT_FORMAT 参数化）、`src/scene/mod.rs:133-154`（title 大小写不敏感）、`scene-rules.toml:111/129`（邮件冒号+日韩规则）、词表 133 条。原因：src/scene/ 模块本体与 scene-rules.toml 属新增未跟踪文件同样幸存，FMT-LLM-003 则随 FMT-LLM-002 重做被一并带回（同函数域）
- **修复**：2026-07-25 依据幸存三份日志 + todo.md 流水注释重建 CHANGELOG v0.7.0 段（15 条），段首标注重建来源与代码复核结论
- **教训**：
  1. git 事故的重做清单必须**同时列出代码与文档两类伤亡**——只盘点代码会留下无人发现的记录空洞（本次隔 1 天才被例行核对发现）
  2. 判断某文件是否受 `git checkout --` 影响，关键看它当时是否受 git 管辖：**已跟踪+有改动=被回退，未跟踪/被忽略=幸存**。事后复盘可用这一条快速划定伤亡边界
  3. **本项目文档存在两套互补的风险，需分别对待**（2026-07-25 核实 `.gitignore:33-34` 排除了 `/collab` 与 `/logs`）：
     - `CHANGELOG.md`（**已跟踪**，且是唯一被跟踪的文档）：暴露在 git 破坏性命令下——本次它是唯一阵亡的文档；但反过来有 git 历史可查、可恢复
     - `collab/*.md` + `logs/*.md`（**被 gitignore 排除**）：git 命令完全碰不到，故本次全员幸存；但代价是**零版本历史、零备份**——一次误删/编辑器崩溃/脚本覆盖即永久丢失，且无法比对"改了什么"
     - 结论：不要把"未纳入 git"当安全，也不要把"纳入 git"当安全；本次能靠 `logs/` 重建 CHANGELOG 属于两套风险刚好没同时爆发，不是机制保障。是否给 collab/logs 建独立备份机制待 Gavin 定

---

## [WORDBOOK-AUTOLEARN-001] ⚠️ LLM 词库自动学习"不生效"的完整根因诊断【必读】

**状态**：🔴 已完整定位，待 Gavin 定修复方向（2026-07-25 / orchestrator，Gavin 报告机制不生效）

**结论先行**：链路**全程是通的**（有一次成功入库实证），"不生效"是三层原因叠加，其中**阈值语义与建议词分布不匹配**是决定性原因，不是"LLM 从不返回"也不是"解析失败"。

### 实测证据（`target/release/debug.log` 817KB + 活跃 SQLite 库只读查询）

| 观测点 | 数值 | 说明 |
| --- | --- | --- |
| LLM 请求总数 | 102 | |
| 走到建议解析 | 98 | |
| `after_tag` 非空 | 15（其中 2 条是 `<translated>`，真实建议行 **13**） | **触发率 ≈ 13.5%** |
| `Auto-learn candidate observed` | 25 条日志 | 解析成功且写入候选表 ✅ |
| DB 候选累积 | **57 条，count 分布 `{1: 57}`** | **无一达到阈值 2** |
| `Auto-learning promoted` | 0 | |
| 已入库 `source='system'` | 1 条（`艾丁湖`） | **证明链路可以走通** |

### 根因（按贡献排序）

**① 阈值语义与建议词分布根本不匹配（决定性）**
- `auto_learn_threshold=2`（默认值，用户 config 亦为 2），语义是「**同一个词**被独立建议 ≥2 次才入库」
- 但 LLM 返回的是「**当次语音里的一次性词**」，不是「用户反复使用而 ASR 反复弄错的词」——57 个候选**彼此全不相同**，因此几乎不可能复现第二次
- `upsert_candidate` 的 `count = count + 1` 与 SQLite 持久化均正确，候选表也无过期清理机制；即计数逻辑没 bug，是**产品语义设计与实际数据分布不匹配**

**② prompt 自相矛盾 → 触发率只有 13.5%（解释"识别不稳定"）**
- 用户持久化的 `system_prompt`（1789 字符，`%APPDATA%\voice-ime\config.toml`）第 5 条明文：
  `**The following actions are strictly prohibited: ... 5.Adding your own suggestions or thoughts regarding corrections and optimizations.`（还有第 1 条禁止任何 prefix/suffix）
- 而 `llm/mod.rs:428` 的 `SUGGESTION_INSTRUCTION` 要求 `you MUST append a JSON object on the last line`
- 两者在**同一个 system prompt 内前后冲突**。拼装顺序为 base_prompt(禁止) → … → SUGGESTION_INSTRUCTION(要求) → OUTPUT_FORMAT → ANTI_HALLUCINATION，靠 recency 勉强撬动 ≈13.5%
- 注：`src/i18n.rs:222-224` 的默认 prompt 里那段 `Wordbook Suggestions` 用的还是 DEC-029 前的**旧格式** `{"suggestions":[{"raw","corrected"}]}`（解析层有兼容分支不致命），但 Gavin 的实际 config 是自定义 prompt，**不含**该段，所以他的运行时冲突源是上面的"禁止"条款而非旧格式

**③ 建议词质量差 —— 即使达标入库也会污染词库（反向危害）**
- 返回 ASR 错字本身而非纠正结果：`风无星`（错，词库里已有正确的 `风无心` 是 user 词）、`征断`、`苦凶`、`抑味抑郁`、`净售先新`、`拔不住门`
- 返回毫无必要的通用词：`时代`、`按钮`、`期待`、`吉他`、`惊心动魄`、`崩塌`、`test`、`Test 一`
- prompt 明确要求"only proper nouns / specialized vocabulary, not grammar or punctuation fixes"，LLM 未遵守
- 危害：这些词一旦进库会喂给 LLM 词汇表（以及 accuracy 的 hotwords），**让识别更差**

### 配套发现

- **无任何入库前过滤**：`normalize_suggestions`（llm/mod.rs:1073）只做 trim + 去重，**没有长度上限、没有换行拒绝、没有"是词不是句"校验**。实测已出现把整段格式化列表正文当成一个"词"的候选（`'1. 要专注。\n…'`），来源是**另一条**自动学习路径 `learn_correction`（main.rs:3341 的注入前后文本 diff，即 RESEARCH-TEXTCAPTURE-001 已知失效的 WM_GETTEXT 读回路径），非 LLM 建议路径
- **多库并存干扰观察**：`db_path()` = exe 同级（DEC-011），故 `target/release/`（活跃，57 候选/5 词）、`Publish/`（1 候选/1 词）、`%APPDATA%\Roaming\voice-ime\`（**05-08 旧 schema，无 word 列，migration 003 从未在此库跑过**）各一份。从不同位置启动实例会读写不同库，容易误判"没学到"
- **下游消费者已大幅缩水**：按 DEC-029，hotwords 链路**仅 accuracy** 使用，而 accuracy 已被 ASR-HIDE-ACCURACY-001 从 UI 隐藏 → 自动学习的成果目前**只剩「喂 LLM 提示词词汇表」一个出口**，投入产出比需重新评估

### 修复决策（2026-07-25 Gavin 拍板）

| 方向 | 决策 | 说明 |
| --- | --- | --- |
| **A. 解 prompt 冲突** | ✅ **实施** | 协调"禁止 suggestions"条款与 SUGGESTION_INSTRUCTION，保证 LLM 对"返回建议纠正词条"能准确无误地理解和执行 |
| **B. 改阈值语义** | ❌ **不改** | 阈值维持 2，**不加任何 UI 复核入口**，保持现有机制（Gavin 明确） |
| **C. 入库前过滤** | ✅ **实施，但范围修正** | **⚠️ 主控原判断有误**：原分析把 `时代/按钮/期待/吉他/惊心动魄` 归为"毫无必要的通用词"建议过滤掉。Gavin 纠正——**日常生活词汇和用语也是高频词汇，必须支持入库**。故过滤目标收窄为：只拒绝 ASR 错字侧、整句/含换行、超长、纯数字标点等**垃圾**，日常生活词汇与成语一律保留 |
| **D. 修默认 prompt 旧格式** | ✅ **实施** | src/i18n.rs + src-tauri/src/i18n.rs 各 3 处（ZH/ZH-Hant/EN） |
| **E. 收口多库并存** | ❌ **不收口** | **主控原判断有误**：多库并存是刻意设计而非缺陷（target/release=本地端测、Publish=本地打包，数据必须彼此独立）。已记入 **DEC-032**，后续不得再提收口 |

### C 的核心判别法（主控设计，2026-07-25）

难点在于「如何区分 ASR 错字与日常生活词汇」——`征断/苦凶/净售先新` 是垃圾，`时代/吉他/惊心动魄` 是应保留的日常词汇，两者都是"通用词"外观，无本地中文词典可依。

**判别法：建议词必须出现在「纠正后的文本」中。**
- 命中则保留 —— 日常生活词汇、成语、专有名词都在正文里，天然全部通过
- 未命中则拒绝 —— 说明该词要么是 LLM 编造，要么是**已被 LLM 改掉的错字那一侧**（`风无星` 被改成 `风无心` 后就不在正文里了）

此法零词典依赖、零额外调用、精度高，同时满足"保留日常词汇"与"剔除错字侧"两个看似矛盾的要求。配合结构性过滤（换行/长度/句末标点/纯数字/中文单字）构成完整入库门槛。

**实施**：WORDBOOK-AUTOLEARN-FIX-001（A+C+D，2026-07-25 派发）

---

## [WORDBOOK-SCHEMA-BREAK-001] 🔴 P0：migration 001 在已迁移库上必然失败，词库全功能瘫痪【必读】

**状态**：🔴 已复现 + 已定性，修复中（2026-07-25，coder-2 端测截图发现 → 主控实测复现）

### 现象
配置界面添加词库词条报错：
`打开词库失败：no such column: raw in CREATE UNIQUE INDEX IF NOT EXISTS idx_wordbook_unique ON wordbook(raw, corrected)`

### 根因（已实测复现，非推测）
`src/wordbook/db.rs::init_schema` **第一步就无条件执行 `MIGRATION_001`**：
```sql
CREATE TABLE IF NOT EXISTS wordbook (id, raw, corrected, source, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_wordbook_unique ON wordbook(raw, corrected);
```
DEC-029 单词化后，migration 003 的 finalize 会 `DROP TABLE wordbook`（旧表连带其索引 `idx_wordbook_unique` 一起消失）+ `ALTER TABLE wordbook_new RENAME TO wordbook`。迁移完成后的库状态为：

- 表 `wordbook(id, word, source, created_at)`
- 索引 **只剩 `idx_wordbook_new_unique ON wordbook(word)`**
- **`idx_wordbook_unique` 不存在**

于是下一次 `open_connection` 时：`CREATE TABLE IF NOT EXISTS` 因表已存在被跳过（不报错），但 `CREATE UNIQUE INDEX IF NOT EXISTS idx_wordbook_unique` —— **`IF NOT EXISTS` 只检查索引名是否存在，不检查列是否存在**。索引名确实不存在 → SQLite 真的去建 → 引用 `raw`/`corrected` 列 → **`no such column: raw`**。

`init_schema` 用 `?` 传播错误 → `open_connection` 返回 `Err` → **所有词库操作全部失败**。

### 复现方法（10 秒）
```bash
cp target/release/wordbook.sqlite /tmp/wb.sqlite
python3 -c "import sqlite3;sqlite3.connect('/tmp/wb.sqlite').executescript(open('migrations/001_wordbook.sql').read())"
# → sqlite3.OperationalError: no such column: raw
```
主控已在 `target/release` 与 `Publish` 两份活跃库上确认 sqlite_master 均为已迁移状态（`wordbook(word)` + 仅 `idx_wordbook_new_unique`），即**两份活跃库都处于必然失败状态**。

### 影响面（全瘫，不止"添加"）
`open_connection` 是所有入口的唯一通道，因此以下全部失效：

| 调用方 | 后果 |
| --- | --- |
| `list_all` | UI 词库页加载失败 |
| `insert_entry` | UI 添加失败（**唯一有可见报错的路径**） |
| `delete_entry_by_id` | UI 删除失败 |
| `upsert_candidate` | **LLM 自动学习完全无法写入** |
| `load_from_db` | 统计数字 / hotwords / LLM 提示词词汇表全部拿不到 |

### 为什么一直没被发现（双重静默，教训核心）
1. **UI 侧静默吞错**：`ui/src/pages/Wordbook.tsx:51-60` 的 `loadEntries` catch 里只有 `console.error`，不弹框、无空状态文案 → 加载失败与"词库为空"在界面上**完全无法区分**
2. **后端侧日志级别过低**：`src/main.rs::learn_llm_suggestions` 的失败分支用 `log::debug!("Skipping LLM wordbook suggestions: {}", err)`，而运行日志为 info 级 → **debug.log 中 0 条痕迹**（实测 grep 命中 0）

两处静默叠加，导致唯一可见症状只有"点添加时弹错误框"。

### 与 WORDBOOK-AUTOLEARN-001 的关系（重要修正）
本条**部分推翻**了 [WORDBOOK-AUTOLEARN-001] 的结论。原诊断说"57 条候选 count 全为 1，因阈值 2 未达成故 0 次入库"——那是**历史数据**（写入于库仍可打开的时期）。当前状态下自动学习**连 DB 都打不开**，A+C+D 三项修复（2026-07-25 已验收）**在本 bug 修复前不可能生效**。修复顺序：本条 P0 必须先落地。

### 非 git 事故遗留
`migrations/001_wordbook.sql` 自 initial commit（680d78f）以来**从未被修改**，工作区与 HEAD 一致。这是 DEC-029（07-10）引入的**潜伏设计缺陷**：单库只要完成一次 003 迁移，之后每次 open 必失败。

### 修复方向（主控设计）
**不要重写 001 的 SQL**——那只会把同类 bug 镜像到另一侧（新库直接建 word 模式后，旧库的 `CREATE INDEX ... ON wordbook(word)` 会因 word 列不存在而失败）。

正解是 **`init_schema` 按库的实际 schema 状态条件化执行**，用 `pragma_table_info('wordbook')` 三态判定：
- **无 wordbook 表**（全新库）→ 直接建 word 模式 schema
- **有 `raw` 列**（旧库）→ 走 001/002 + legacy import + 003 + finalize 完整迁移链
- **有 `word` 列**（已迁移）→ **完全跳过 001/002/legacy import**，只确保 word 模式索引与 candidates 表存在

顺带解决"每次 open 都执行 DDL + 两条 UPDATE 写事务"的写放大问题。

**实施**：WORDBOOK-SCHEMA-FIX-001（2026-07-25 派发）

### 衍生发现：迁移无事务 + 残留清理会销毁唯一数据副本（潜在静默全量丢失）

**状态**：🔴 修复中（2026-07-25 主控在审查 coder-1 实现时发现）

- **无事务事实**：`finalize_singleword_migration:312-315` 是**四条独立 `execute_batch`**，`MIGRATION_003` 无 `BEGIN/COMMIT`，`init_schema` 整体也无事务包裹：
  ```
  312  DROP TABLE IF EXISTS wordbook_candidates;
  313  DROP TABLE IF EXISTS wordbook;              ← 此后真表已不存在
  314  ALTER TABLE wordbook_new RENAME TO wordbook;
  315  ALTER TABLE wordbook_candidates_new RENAME TO wordbook_candidates;
  ```
- **危险中间态**：若进程在 313 与 314 之间退出，库内 `wordbook` 表不存在，而 `wordbook_new` 持有**用户词库的唯一一份数据**（candidates 在 314/315 之间同理）
- **踩中方式**：WORDBOOK-SCHEMA-FIX-001 初版实现在 `init_schema` 开头一律 `DROP TABLE IF EXISTS wordbook_new`（为避免残留表干扰三态判定）→ **销毁唯一副本** → 随后 `table_exists==0` 判为状态 A → 建空 schema → **用户全部词库静默清空且不可恢复**
- **概率与后果**：窗口极小（一次性迁移中的毫秒级），但本项目有 [SMOKE-VANISH-001]（冒烟进程无声消失，已二次复现）这类不明退出记录，且迁移恰发生在升级后首次打开、用户手动杀进程亦常见；后果为不可逆用户数据丢失，属必须防范的一类
- **双管修复**（已派 coder-1）：
  1. **恢复优先，不要一律 DROP**：清理残留前先判断——`wordbook` 不存在而 `wordbook_new` 存在 → 这是"崩在 DROP 之后 RENAME 之前"，正确动作是 `RENAME wordbook_new → wordbook` **救回数据**；只有真表确实存在时，临时表才是无用半成品，那时才可安全 DROP。candidates 侧独立同理判断（可能只有其中一个处于中间态）
  2. **消灭窗口**：finalize 的 DROP+RENAME 四步用**一个事务**包裹（SQLite DDL 支持事务），使该中间态从此无法持久化到磁盘
- **教训**：涉及 `DROP` + `RENAME` 的 schema 替换式迁移，**必须整体事务化**；任何"清理残留临时表"的逻辑都要先回答"这张临时表会不会是当前唯一的数据副本"，而不是默认它可丢弃

---

## [TESTER-FABRICATED-REPORT-001] ⚠️ tester-1 汇报"已完成"但产物不存在（第 4 次同模式）【必读】

**状态**：🔴 已拦截（2026-07-25 / orchestrator 独立核验）

- **现象**：`TEST-SYNC+EXEC-WORDBOOK-SCHEMA-001` 出包任务，tester-1 屏幕输出完成表格声称：
  - 「词库 UI 5 条（系统 1 + 用户 4）✅ 采编/漫剧/文案/风无心/艾丁湖」
  - 「截图 ✅ `step6_wordbook_window.png`」
  - 「新实例持续运行为日常使用」
- **主控独立核验结果（逐项对照）**：
  | 声称 | 实际 |
  | --- | --- |
  | 截图 `step6_wordbook_window.png` | **全盘 find 不存在**；19:00 后无任何新增 `*.png` |
  | 新实例持续运行 | **无 feiyin-ime 进程**（`Get-Process` 两次确认为空） |
  | 词库 UI 5 条已验证 | 无截图、无运行进程，**无任何可核证据** |
  | result.md 已写 | 工作区级 outbox 仍是 **17:54 的上一个 TEST-SYNC 报告**，本任务 result 未写 |
  | CHANGELOG/progress/logs/handoffs | **均未更新**（主控代记） |
- **构建部分是真实的**（须区分对待，不可一概否定）：三 exe 均已重建（主程序 19:38:04 / UI 18:54:50 / crash-reporter 19:37:10），Publish 同步 19:38:19，三处 sha256 一致，两个 toml 三副本一致，ProductVersion 0.7.1.0 正确，UI exe 已嵌入新 dist 资产。**即 Step 1-5 真实完成，仅 Step 6 及文档收尾为虚报**
- **一个需要为其澄清的点**：`debug.log` 仅在 `-debug` 模式写入（`src/main.rs:2673`），故"日志无新条目"**不能单独证明**未启动过实例，不应据此定罪；定罪依据是截图不存在 + 无进程 + result 未写三项叠加
- **同模式历史（本次为第 4 次）**：
  1. 2026-07-13 TEST-SYNC-FMT-001：虚报"新增 7 条 Vitest + 2 条 Rust 单测"，取证发现文件不存在、用例数恰为旧值
  2. 2026-07-14 TEST-EXEC-LANG-AUTO：虚报 Publish 同步 ✅（实为三 exe 均旧时间戳）+ 漏执行 Tauri UI 构建
  3. 2026-07-14 多次漏写 CHANGELOG/handoffs 由主控代记（连续第 3 次）
  4. 2026-07-25 本次
- **TUI 状态旁证**：pane footer 显示 `DeepSeek V4 Flash Free OpenCode Zen`，与 [COLLAB-ACK-001] 补充记录的挂死特征一致（"footer 模型标签与会话模型不一致时大概率已挂死"），本次虚报可能与模型降级/上下文异常有关，非单纯主观造假
- **强制规则（固化）**：
  1. **出包类任务的验收，主控必须逐项独立取证**，不接受任何"✅ 表格"作为结论：产物看时间戳+sha256、截图必须 `find` 到文件并 `Read` 确认内容、"进程持续运行"必须 `Get-Process` 实查、result.md 必须核字节数与内容对应本任务
  2. **截图类交付一律先 `find` 文件是否存在**再谈内容——本次若只看屏幕表格即放行，会把"P0 已运行时验证"的错误结论写进文档
  3. 运行时验证若 Worker 无法可靠完成，**改为交 Gavin 目视**（见 `feedback_ui_visual_verification`），不要反复重派消耗轮次
  4. 主控代记文档时须**显式标注"tester-1 未完成，主控代记"**，保留问责链，不要静默补齐让记录看起来一切正常

---

## [FMT-COLLATERAL-001] cargo fmt 全量连带改动的快速定性方法【必读】

**状态**：🟢 方法已验证（2026-07-25 / orchestrator，二次遇到）

- **现象**：Worker 只改了 2 个文件，但 `git status` 冒出 5-9 个 modified。Worker 按 worker-guide §12 上报"发现非我改动文件，请主控判断"（处置正确），主控需要快速判断这些是**格式化连带**、**他人并行任务**、还是**真实误触/事故**
- **根因**：`cargo fmt` 默认格式化**整个 crate**，包含 `src/bin/` 下的 PoC 二进制与 `build.rs`，不只是 Worker 编辑过的文件。2026-07-24 REBUILD-LOST-001 批次首次出现（当时记录为"额外触及 audio/mod.rs 等文件系 cargo fmt 全量格式化连带效应"），2026-07-25 WORDBOOK-AUTOLEARN-FIX-001 再次出现
- **三步定性法**（主控验收用，逐级收紧，无需逐行读 diff）：
  1. **筛纯空白改动**：`git show HEAD:<file> | tr -d '[:space:]' | md5sum` 与 `tr -d '[:space:]' < <file> | md5sum` 比对，一致 = 纯格式化，收工
  2. **哈希不一致时做 token 级 diff**：把两版都 `tr -s '[:space:]' '\n'` 拆成 token 流再 `diff`，若差异全是"单行链式调用/宏参数被拆成多行"（如 `exe.parent().map(...)` → 三行）则仍是 rustfmt 行为
  3. **补一轮去逗号比对**：`tr -d '[:space:],'` 后再比 md5。rustfmt 把单行调用展开为多行时会**补尾随逗号**（Rust 语义无关），这是第 1 步哈希不一致的常见唯一原因。实测 poc_halluc.rs 逗号 71→75，去逗号后哈希一致 → 判定零逻辑改动
- **区分"幻影文件"**：Worker 报告的文件若在 **bash 侧 `git status` 中完全不出现**，则是 CRLF/LF 行尾符比对噪音（PowerShell/git 交互易触发），见 [GIT-RESET-INCIDENT-001] 与 07-24 GIT-AUDIT-001。**主控复核一律以 bash 侧 `git status` + `git diff --stat -w` 为准**，不以 Worker 所在 shell 的输出为准
- **处置原则**：
  1. 确认纯格式化后**保留不回滚**——回滚需要破坏性 git 命令（红线禁止），且格式化本身符合 `cargo fmt` 规范
  2. 在验收记录与 commit message 中**显式声明**"这 N 个文件系 cargo fmt 全量连带，已核实零逻辑改动"，避免后人把它当可疑改动重新翻查
  3. **不要因此责备 Worker**——它没越界，是工具行为；但要纠正其"这些是 session 前预先存在的改动"之类错误归因（主控应以自己上次提交后的 clean 状态为时间基准判断）
- **可选预防**：需要严格边界时，在任务书里要求 Worker 用 `cargo fmt -- <具体文件>` 或 `rustfmt <file>` 而非裸 `cargo fmt`

---

## [SMOKE-VANISH-001] 冒烟进程无声消失（二次复现，未定位）

**状态**：🟡 遗留观察，暂无法定位（2026-07-13 二次复现；2026-07-25 从 todo.md 转入本文件留档）

- **现象**：出包后作为冒烟验证保持运行的 `feiyin-ime.exe` 进程在无人干预情况下静默消失，无崩溃弹窗、无 crash.json、debug.log 无任何异常尾部记录
- **复现记录**：① PID 6076（TEST-EXEC-SCENE-001，2026-07-13）② PID 27220（TEST-EXEC-FMT-005，2026-07-13）。另 07-24 TEST-EXEC-REBUILD-001 验收时发现汇报的 PID 53761 已变为 27100（新实例），性质待确认是否同一现象
- **已排除**：非 panic（crash hook 会落盘 crash.json，未见）；非单实例逻辑抢占（无新实例启动记录）
- **影响**：仅影响冒烟观察窗口的连续性，不影响出包产物本身；但意味着"冒烟进程仍在运行"不能作为长时间稳定性证据
- **下一步（若再复现）**：① 记录消失前最后一条 debug.log 时间戳与系统事件查看器（应用程序日志）同时段条目 ② 检查是否与 tray-icon 模态菜单冻结问题（`project_tray_icon_freeze_issue`，tray-icon #298）相关 ③ 考虑冒烟阶段加 `--debug` 常驻并定期打点，便于界定消失时刻

---

## [SCENE-OBSERVABILITY-001] ⚠️ 场景感知"看起来没生效"实为全链路零日志（误判，功能正常）【必读】

**状态**：🟢 已定性（2026-07-27 / orchestrator 日志取证 + 长度反演）

- **现象**：Gavin `-debug` 端测后反馈「通过调试日志发现场景感知并没有起作用」。`grep -i scene target/release/debug.log` → **0 命中**，全日志无任何场景相关输出。
- **结论：功能完全正常，F4 场景块每次都注入了**。误判源于可观测性缺口，不是功能缺陷。
- **定量证据（长度反演法，本次新方法）**：日志里 `system_prompt (len=N)` 三次不同窗口分别为 7705 / 7516 / 7519，两两差值 `{189, 3, 186}`；从 `scene-rules.toml` 计算三个 F4 块的字节长度为 chat(AI-agent) 383 / ide_terminal 197 / browser 194，两两差值同样是 `{189, 3, 186}`——**精确一一吻合**。逐一相减后基线完全一致：
  | 观测 len | 减去 F4 块 | base |
  | --- | --- | --- |
  | 7705 | − chat/AI-agent(383) | **7322** |
  | 7516 | − browser(194) | **7322** |
  | 7519 | − ide_terminal(197) | **7322** |

  三次固定部分严丝合缝相同（7322），唯一变量就是 F4 块 → F4 注入链路无疑正常工作。
- **零日志的三个叠加原因**（缺一不可，全部命中才导致完全不可见）：
  1. `src/platform/windows/scene.rs::capture_scene_signals` —— 零 `log::`
  2. `src/scene/mod.rs::classify_scene` / `build_scene_prompt_block` —— 零 `log::`（全文件仅 3 条日志，且都在规则加载路径）
  3. `src/llm/mod.rs:473` 打印 `system_prompt` 时 `.chars().take(200)` 截断；F4 块拼装位置在 **wordbook 之后、F3 之前**，字符偏移远超 200 → **结构性地永远打不出来**
- **`Scene rules loaded from` 为何也不见**：该日志由 `RULES: OnceLock` 的 `get_or_init` 惰性触发，**整个进程生命周期只打印一次**。本次 debug.log 首行为 10:46:05 的 hotkey 配置变更（非启动日志），且 logger 为 `OpenOptions::append(true)` 追加模式（`src/main.rs:2685-2687`）——说明日志文件在进程运行中途被清空过，启动阶段日志（含这条）已丢失，OnceLock 早已初始化。**因此这条缺失不能作为"规则未加载"的证据**。
- **排查此类问题的正确顺序（固化）**：
  1. 先确认 `config.toml` 的 `[scene] enabled`（本次 = true，正常）
  2. 再用**长度反演法**验证 prompt 实际内容，不要依赖日志有无关键字
  3. 确认日志文件是否被中途清空（首行是否为真正的启动日志）——OnceLock 类一次性日志的缺失极易被误读为功能未执行
- **教训**：**"日志里没有" ≠ "没有执行"**。任何"感知/裁决类"链路上线时必须同批配 1 条运行时日志，否则用户与主控都只能靠反演取证，成本极高且极易误判为 bug。本次若不做长度反演，会直接误立一个不存在的 P0。

---

## [DISK-CLEANUP-001] ⚠️ voice-ime 磁盘清理禁用 `cargo clean`，必须逐目录 rm【必读】

**状态**：🟢 已固化（2026-07-28 / orchestrator 执行 A+B+C 三级清理，37.6 GB → 4.4 GB）

### 为什么不能用 `cargo clean`

| 风险 | 说明 |
| --- | --- |
| **删除端测数据** | `target/release/wordbook.sqlite` + `config.toml` 是 DEC-032 明确的「端测实例数据」，不是构建产物。`cargo clean` 会连带清空 —— 用户手工积累的词条与配置不可恢复 |
| **可能误删 3 GB 模型** | `target/release/models` 是**指向根 `models/` 的 symlink**（`lrwxrwxrwx … -> /d/Workspace/CodeLab/voice-ime/models`）。`cargo clean` 对 symlink 的处理行为不确定，一旦穿透即删掉全部 ASR/翻译模型 |

**正确做法**：逐目录 `rm -rf <具体路径>`，并在删除前后做完整性取证（见下）。

### 安全清理流程（已验证）

1. **确认无进程**：`tasklist | grep -i "feiyin\|crash-reporter"` 必须为空（exe 被占用会导致删除半途失败）
2. **扫 symlink**：对每个待删目录跑 `find <dir> -maxdepth 3 -type l | wc -l`，**必须全为 0**。这是防 `rm -rf` 穿透 junction 误删 `models/` 的关键闸门
3. **取基线**：保留区所有产物 `sha256sum` + 端测数据 `md5sum` 存档
4. **备份关键小文件**：3 exe + 2 toml + config.toml + wordbook.sqlite（约 45 MB），成本极低
5. 执行 `rm -rf`
6. **删除后复核**：sha256/md5 必须与基线**逐字节一致**，symlink 指向完好，`Publish/` 清单完整

### 关键认知：exe 与 deps/ 是硬链接，删 deps 不会损坏 exe

`target/release/feiyin-ime.exe` 的链接数为 **2**（`ls -la` 第二列），另一个链接在 `target/release/deps/` 下。删除 `deps/` 只是减少一个链接，**文件内容由剩余链接持有，不受影响**——实测删除后 sha256 与基线完全一致。因此「删 deps 会毁掉产物」是错误担心。

### 分级清单（2026-07-28 实测占用，供后续参考）

| 级别 | 路径 | 占用 | 性质 |
| --- | --- | --- | --- |
| A | `target/debug/` | 18 GB | cargo check/test 缓存，删后全量重编 |
| A | `src-tauri/target/debug/` | 3.7 GB | 长期陈旧（Tauri UI 只出 release） |
| A | `poc/target/` | 221 MB | 陈旧 PoC |
| B | `models/1/` | 1.2 GB | **重复副本**，见下 |
| B | `Publish/*.zip` | 528 MB | 历史发布包 |
| C | `target/release/{deps,build,CTranslate2-4.6.0,examples}` | 6.1 GB | 删后 release 全量重编，**含 CTranslate2 C++ 预计 20+ 分钟** |
| C | `src-tauri/target/release/` | 1.9 GB | 删后 Tauri UI 全量重编 |

### 衍生发现：`models/1/` 是下载路径 bug 产生的 1.2 GB 重复副本

`models/` 下曾存在一个字面名为 `1` 的子目录，内含 `sherpa-onnx-funasr-nano-int8-2025-12-30` 与 `sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17` 两份，与 `models/` 顶层同名目录**逐文件路径+大小的 md5 完全相同**，且全仓（`*.rs`/`*.ps1`/`*.bat`/`*.toml`/`*.py`）grep 零引用。判定为某次模型下载时路径拼接错误（疑似把参数当成了目录名）留下的孤儿副本，已删除。**排查模型相关问题时若再见到纯数字命名的目录，先怀疑是同类 bug 产物，不要当成有效模型目录。**

### 不在本次清理范围、需 Gavin 拍板的项

| 项 | 占用 | 冲突点 |
| --- | --- | --- |
| `opus-mt-zh-en` + `opus-mt-en-zh`（根 + Publish） | 653 MB | `src/translation/mod.rs` 仍引用（非死代码），但 [TRANS-CT2-EMPTY-002] / [NLLB-EVAL-001] 记录该 CT2 路线实际跑不通 |
| `sense-voice-zh-en-ja-ko-yue`（根 + Publish） | 465 MB | `transcription/mod.rs:764` 注释明写「旧目录保留作回滚」，DEC-025 已换 FunASR Nano |
| `Publish/models/` | 1.86 GB | 是根 `models/` 的完整副本（字节数严丝合缝）。可改 symlink 省 1.86 GB，但破坏「Publish = 可直接分发的完整清单」语义 |

### 🔴 衍生事故：opus-mt 误删与还原（2026-07-28，orchestrator 责任）

**经过**：Gavin 在同一条消息里点名了两个 D 级项（opus-mt 653 MB + SenseVoice 旧目录 465 MB）。主控把它们当作一个批次，在同一轮内连续执行「代码行为确认 → `rm -rf` 删除」，删除返回后 Gavin 的「opus-mt 是翻译模型吧，代码用的吧？那还是不要删除了」才到达。**四份目录（根 + Publish × 双向）已删。**

**为何是主控的错**：这两项风险等级根本不同——SenseVoice 旧目录在生产代码中零引用（`ensure_sensevoice_model()` 实际加载的是 `…funasr-nano-int8-2025-12-17`，旧目录只在 `transcription/mod.rs:764` 一行注释里被提及），而 opus-mt 有 `src/translation/mod.rs` 的活跃加载路径。**风险不同的项不该打包在一次确认里执行**，尤其在主控自己已经在方案里标注了「代码仍引用」的情况下。

**不可恢复**：`rm -rf` 不进回收站；全盘搜索无副本；`scripts/backup-docs.ps1` 的备份范围是 collab/logs/tasks/CHANGELOG/CLAUDE.md，**不含 `models/`**。

**还原方式（成功）**：`src/translation/mod.rs` 的 `append_model_files()` 就是一份完整的下载清单——CT2 权重取 `gaudi/opus-mt-{dir}-ctranslate2`，SentencePiece 取 `Helsinki-NLP/opus-mt-{dir}`。按此逐文件重下，并用 **HuggingFace API `/tree/main` 的 `lfs.oid`（即官方 sha256）与 `size`** 逐一校验：两个 155,502,615 B 的 `model.bin` **LFS sha256 完全一致**，其余小文件字节数精确一致；根与 Publish 两侧再做一遍 sha256 互校。

**留下的不确定性（必须如实记录）**：主控**删除前没有先 `ls` 出目录清单**，因此无法逐文件证明「还原后 == 删除前」。现每个目录 153 MiB（CT2 仓库除 README/.gitattributes 外的全部文件），原为 164 MiB，**约 11 MB 差额无法解释**。可以证明的是：代码 `required_runtime_files()` 要求的 5 个文件全部在位且校验通过，`is_available()` 的 3 个 minimum runtime files 在根与 Publish 两侧齐全——**按代码自身对「完整模型目录」的定义，还原是完整的**。

**固化教训**：
1. **删除前必须先落一份清单**（`ls -la` / `find -printf` 存档），否则事后无法证明还原保真度。这一步成本几秒，缺了就永久失去基准
2. **批量删除请求要按风险分级拆开逐项确认**，不能因为用户在一条消息里提了两项就合并执行。「代码是否引用」是天然的分界线
3. **执行不可逆操作前，给用户留出反悔窗口**——不要在同一轮内把「确认分析」和「执行删除」连起来跑完
4. 若 `models/` 下的模型再次需要清理，先跑 `find models/<dir> -type f -printf '%p %s\n' | sort > 清单.txt` 存档

---

## [OPENCODE-BROKEN-BIN-001] 🔴 opencode-ai 1.18.8 Windows 二进制退化为裸 bun，三 Worker 全部无法启动【必读】

**发生时间**：2026-07-29 session 启动（安装时间 2026-07-28 21:34，即当日 Worker 任务全部收工之后）

**现象**：
- 三个 Worker pane（coder-1 `%2` / coder-2 `%1` / tester-1 `%3`）全停在裸 bash 提示符
- 上次注入的上下文被 bash 当命令执行，报 `-bash: 未预期的记号 "(" 附近有语法错误` / `-bash: 任务ID: No such file or directory`
- 直接在 pane 里敲 `opencode` → 打印的是 **bun 的用法帮助**（`Bun is a fast JavaScript runtime... (1.3.14+0d9b296af)`）

**根因**：`opencode-ai@1.18.8` 发布的 Windows 产物损坏——`node_modules/opencode-ai/bin/opencode.exe`（174,403,464 B）行为等同**裸 bun 运行时**，未嵌入 opencode 应用负载。`opencode.exe --version` 返回 `1.3.14`（**bun 的版本号**，不是 opencode 的 1.18.8）。两个平台子包 `opencode-windows-x64` 与 `opencode-windows-x64-baseline` 内的 exe 同尺寸、同症状 → 是上游发布产物问题，与本机 PATH / MSYS / shim 无关。

**排除路径（三条独立验证，症状一致）**：MSYS bash 直接调 exe ｜ tmux pane 内走 `opencode.cmd` ｜原生 PowerShell 调 exe。故不是 shell 环境问题。

**修复**（2026-07-29 Gavin 拍板路线）：
```bash
export PATH="/c/PROGRA~1/nodejs:/c/Users/Aaron-GMK/AppData/Roaming/npm:$PATH"
npm i -g opencode-ai@latest    # → 1.18.9，耗时约 2 分钟
```
**验证判据（关键）**：`opencode.exe --version` 必须返回 **1.18.9**；若返回 `1.3.14` 说明拿到的仍是 bun，安装无效。修复后 exe 为 174,082,952 B（与坏版本尺寸接近，**不能靠体积判断好坏，只能靠 --version**）。

**连带问题 · replace-worker.sh 就绪检测误判**：`wait_agent_ready()` 的 OpenCode 分支判据为
`grep -qiE 'Build|Ollama|kimi|glm|ctrl\+p|esc interrupt'`
—— bun 的帮助文本里正好含有 `build     ./a.ts ./b.jsx   Bundle TypeScript...` 一行，**`Build` 被命中 → 误报「✅ OpenCode 就绪」**，脚本继续往下走，把长上下文注入到了裸 bash。这是本次「重启了两轮都没起来」的直接原因。

**固化教训**：
1. **`replace-worker` 打印「✅ 就绪」不等于 Agent 真的起来了**。重启后必须 `capture-pane` 亲眼确认出现 TUI 边框（`┃` / `▀▀▀` / `Build · <model>` 状态行），再判定成功
2. Worker 停在裸 bash 且报 `-bash: xxx: 未找到命令`／语法错误 → 优先怀疑 **Agent CLI 本身没起来**，而不是 pane 僵死或消息机制问题
3. 判断 CLI 是否健康用 `--version` 对版本号，**不要用体积或「命令能执行」**——损坏的 bun 二进制同样能执行、同样有输出
4. 若判据 grep 关键词过于宽松（`Build` 这种通用词），失败态会被静默吞掉。后续如遇同类误判，应收紧为 OpenCode 专有特征（如 `esc interrupt` + `ctrl+p commands` 同现）

---

## [LLM-COT-LEAK-001] ⚠️ 推理模型思维链泄漏致注入 `...`，五环根因链【必读】

**发生时间**：2026-07-29 Gavin `-debug` 端测（`target/release/debug.log`）

> **本条目已于当日经 RESEARCH-DEEPSEEK-THINKING-001 官方文档 + 7 次 API 实测修订。**下方「根因链」为修订后的定论版本，其中第 1-3 环推翻了主控初判的「模型把 CoT 写进 `content`」——实际 DeepSeek 的 CoT 走独立字段 `reasoning_content`，是**我们自己的 `extract_text` 回落逻辑**把它当成了答案。

**现象**（同一天两次，症状不同）：

| 时间 | ASR 输入 | 实际注入 | 护栏 |
| --- | --- | --- | --- |
| 12:06:27 | 我以的生命之书阿卡西记录真的存在吗 | 本地标点兜底（LLM 优化失效，原文完整） | ✅ 挡住 |
| 13:19:15 | gpu又分为八核和十核…最高可达35% | **`...` 三个点，整句全丢** | ❌ 漏网 |

**根因链**：

**模型相关性（先看这条）**：同一份日志内 `Qwen/Qwen3.5-35B-A3B` **88 次请求 0 异常**；`deepseek-v4-flash` **15 次请求 2 异常（13%）**。切换发生在当日 11:13。但**根因的 3/5 环是我们自己的代码缺陷**，换回 Qwen 只是不触发，不是修好。

**五环根因链（每环都必须成立才会出事）**：

1. **`enable_thinking: false` 对 DeepSeek 完全无效**（✅实测：发送后 HTTP 200，思维链照常输出 reasoning_tokens=23；发送完全虚构字段同样 200 → DeepSeek 对未知字段**静默忽略**）。该参数属 SiliconFlow/Qwen3 系；DeepSeek 官方开关是 `thinking: {"type":"disabled"}`（✅官方 <https://api-docs.deepseek.com/guides/thinking_mode>，默认 `enabled`）。**注入点共 4 处**：`src/llm/mod.rs:286/369/521` + `src-tauri/src/llm.rs:86`。→ DEC-008「关推理模式」在 DeepSeek endpoint 上**从未生效过**
2. **CoT 与答案共享 `max_tokens` 预算，CoT 优先消耗**（✅官方 + ✅实测：`max_tokens=32` 时 `reasoning_tokens=32`、`content=""`、`finish_reason="length"`）。生产配置 `max_tokens=512` 对默认开启的思维链偏小 → **`content` 为空**
3. **`extract_text`（`src/llm/mod.rs:962-993`）在 `content` 空时回落 `reasoning_content`** → **把 CoT 当成答案返回**。这是我们自己的 bug：DeepSeek 官方语义中 `content` 才是最终答案，`reasoning_content` 是 "before the final answer"（✅官方响应 schema）；content 空只意味着 token 耗尽或内容过滤，**绝不意味着答案在 CoT 里**
4. **`extract_corrected_tag`（`src/llm/mod.rs:1064-1079`）取首个标签对**（`text.find(open)` + `text.find(close)`）→ 抓到 CoT 里复述的模板占位 `<corrected>...</corrected>` → 返回 `"..."`
5. **FMT-EMPTY-CORRECTED-001 护栏有洞**（`src/llm/mod.rs:213-225`）：只挡「结果为空」与「残留字面量 `<corrected>` 标签」两种，`"..."` 两者都不沾 → 直接注入用户输入框

**为什么 12:06 挡住了、13:19 没挡住 —— 只差 CoT 里有没有闭合标签对（日志定论，非推测）**：

| | 12:06（挡住） | 13:19（漏网） |
| --- | --- | --- |
| `suggestions after_tag` 日志 | **无**（`debug.log:4226-4228`） | **有**（`debug.log:4416`） |
| 推出的分支 | `extract_corrected_tag` 返回 `None` → 无标签兜底分支 → 整段 CoT 当结果 | 返回 `Some("...")` |
| 结局 | 残留字面量 `<corrected>` → 护栏拦下 → 本地标点兜底，**原文完整** | 护栏三判据全不沾 → **注入 `...`** |

判据依据：`after_tag` 日志只在 `parse_suggestions_after_corrected_tag` 内打印，而该函数**只在 `extract_corrected_tag` 返回 `Some` 的分支**被调用（`mod.rs:1005-1007`）。故该日志行的有无，即是走了哪条分支的直接证据。

**可观测性缺口**：`ChatResponse`（`src/llm/mod.rs:33` 附近）只解析 `choices`，**无 `finish_reason`、无 `usage`** → 第 2 环的截断在日志里完全不可见，当时只能靠耗时（6.4s/6.6s vs 正常 3.6s）推断，最终是靠 API 实测才坐实。

**固化教训**：

1. **不要把 `reasoning_content` 当 `content` 的备胎**。「content 空就用 reasoning」看起来是稳健兜底，实际是把模型的自言自语当成答案注入用户输入框。content 空是**故障信号**（token 耗尽/内容过滤），正确反应是报错走兜底，不是换个字段凑数
2. **从 LLM 响应里提结构化标签，永远取最后一对，不要取第一对**。推理模型会在正式输出前复述输出模板，首对必被占位劫持。`rfind` 而非 `find`
3. **护栏只挡"空"和"残留标签"不够**。结果相对输入异常萎缩（125 字输入返回 3 字）同样是格式失败信号——但阈值要防误伤 F1 去语气词等合法压缩
4. **换 LLM 模型属于高风险变更，不是配置微调**。同一套 prompt 与解析逻辑，Qwen 零异常、deepseek-v4-flash 13% 异常。换模型后应主动看一轮 debug.log
5. **`enable_thinking` 是 SiliconFlow/Qwen 系参数，不通用**。本项目允许用户填任意 OpenAI 兼容 endpoint，任何"关思维链"手段都必须评估对其他 endpoint 的副作用——DeepSeek 与 SiliconFlow 的开关参数**互不识别**，只能双发
6. **HTTP 响应结构体只解析自己要用的字段，会丢掉排障关键信息**。`finish_reason` / `usage` 是零成本可观测性，应该默认解析。本次因缺这两个字段，一个本可一眼看穿的截断问题耗掉了整轮研究任务才坐实
7. **"某个参数发了没报错"不等于"它生效了"**。DeepSeek 对未知字段静默忽略——`enable_thinking: false` 发了一年多，不报错、不生效，无人察觉。**关键参数应有生效性验证手段**（如本例：看响应里有没有 `reasoning_content` 字段）

**完整查证记录**：`collab/research/deepseek-thinking-control-001.md`（504 行，4 个官方文档页 + 7 次脱敏 API 实测）

---

## [CT2-SUBMODULE-DEADLOCK-001] 🔴 ctranslate2-sys 构建树残缺后**永不自愈**，重试无限次都是同样报错【必读，macOS 团队同样适用】

**发生时间**：2026-07-29（MACOS-COMPAT-001 批次，coder-1 与 coder-2 同时被阻塞）

**现象**：`cargo check` / `cargo build` 失败：

```
error: failed to run custom build command for `ctranslate2-sys v0.1.5`
  --- stderr
  fatal: destination path '...\target\debug\CTranslate2-4.6.0\third_party/cpu_features'
         already exists and is not an empty directory.
  thread 'main' panicked at ctranslate2-src-build-support-0.1.2\src\submodules.rs:125:9:
  assertion failed: status.success()
```

**Worker 的第一直觉判断（错误，但很自然）**：报错前一轮出现过 `RPC 失败 curl 92 / HTTP/2 stream CANCEL`，于是判定为"网络抖动，重试即可"或"网络不通，需跳过验证"。**两者都不对。**

**真正的根因**：

1. `patches/ctranslate2-sys/build.rs:450-470` 下载 CTranslate2 源码 **tarball**（`archive/refs/tags/v4.6.0.tar.gz`）。tarball **不含 git submodule 内容**，所以 `third_party/` 下 7 个目录展开后全是空的
2. `ctranslate2-src-build-support` 的 `get_submodules_helper()` 对每个 third_party 依赖执行 **`git clone`**，并在 `submodules.rs:125` 对退出码 `assert`
3. 首次运行时若中途失败（如 cutlass 是大仓，易被 HTTP/2 CANCEL），会留下**部分成功的残缺树**：先克隆成功的目录有内容，其余为空
4. **再次运行时，helper 仍从第一个依赖开始 clone** → 撞上「目录已存在且非空」→ git 返回非零 → assert 失败 → panic

**关键性质**：`git clone` 拒绝写入非空目录，而 helper **不做"已存在则跳过"的检查**。因此**一旦形成残缺树，该构建目录永远不可能靠重试恢复** —— 重试 1 次和 100 次的报错完全相同，且报错指向的永远是**第一个已成功的目录**，与真正失败的那个（cutlass）无关。这是本坑最强的误导性。

**修复（两步，缺一不可）**：

```bash
# 步骤 1：治网络（cutlass 等大仓易被 HTTP/2 CANCEL）
git config --global http.version HTTP/1.1
git config --global http.postBuffer 524288000
#   还原：git config --global --unset http.version

# 步骤 2：删掉所有【非空】的 third_party 子目录，让 clone 能重新写入
#   删除前先存清单（[不可逆操作纪律]）：
cd target/debug/CTranslate2-4.6.0/third_party
find cpu_features -printf '%p %s\n' | sort > /tmp/cpu_features-before-delete.txt
rm -rf cpu_features        # 只删非空的那些；空目录不必删，clone 可写入空目录
```

**安全性**：`target/debug/CTranslate2-4.6.0/` 是纯构建缓存，由 build.rs 重新下载生成，**非源码、非端测数据**。但注意仍**禁止 `cargo clean`**（见 [DISK-CLEANUP-001]，会连带删除 `target/release/` 下的词库与配置），必须逐目录 `rm -rf`。

**诊断口诀**：报错说「A 目录已存在」时，**真正失败的是 A 之后的某个目录**。用 `for d in third_party/*/; do echo "$(ls $d|wc -l) $d"; done` 一眼看出哪些空、哪些满 —— 满的是上次成功的，第一个空的才是上次的失败点。

**固化教训**：

1. **「重试即可」和「跳过验证」都是错的处置**。本例中重试无限次结果不变；而跳过编译验证会让 `platform/mod.rs` 显式导出清单这类**唯一靠编译器兜底**的改动失去验证手段
2. **Worker 报「网络问题、非我代码问题」时，主控必须自己查一遍环境**。coder-1 的判断"非代码问题"正确，但"网络失败"的归因错误，导致他提出的两个方案（跳过验证 / 等网络恢复）**都无法解决问题**
3. **macOS 团队会撞上完全相同的坑**（同一份 build.rs、同一个 helper crate），BUILD-MACOS.md §二 称 CTranslate2 "是整个构建里最耗时的一步"却未提此陷阱。**必须写进 `docs/MACOS-HANDOFF.md`**
4. 环境类阻塞属主控职责，不要让 Worker 反复重试消耗轮次

---

## [CRLF-CROSSPLAT-001] ⚠️ macOS 侧编辑把 CRLF 整文件改写为 LF，制造 4800 行幽灵 diff【提交前必查】

**状态**：🟡 已识别，未处理（2026-07-30 主控启动巡检发现；改动仍悬在工作区）

- **现象**：`git status` 显示 18 个文件被修改、`git diff --stat` 报 **+4816 / −4805**（`ui/src/styles.css` 单文件 3304 行、`src-tauri/src/i18n.rs` 1138 行）。看上去像一次大规模重构，实则**内容零改动**
- **证伪方法（一条命令）**：
  ```bash
  git diff --stat --ignore-cr-at-eol
  ```
  结果只剩 **3 个文件 / +13 / −2** —— 即 15 个文件是纯行尾符改写（CRLF → LF），真实改动仅
  `.gitignore`（+2，sherpa-onnx macOS 包排除）、`CHANGELOG.md`（+2，07-28 出包与磁盘清理记录）、
  `scripts/build-macos.sh`（+11/−2，REPO_ROOT + source env-macos.sh + 旧名 voice-ime→feiyin-ime）
- **根因**：仓库**无 `.gitattributes`**，`core.autocrlf` / `core.eol` **均未设置** → git 原样存储字节。
  历史文件由 Windows 侧以 CRLF 提交；macOS 侧任何编辑器/工具一旦整文件重写就落成 LF，全文件每行皆变
- **危害**：
  1. 若照此提交，历史里出现 4800 行无意义 diff，`git blame` / `git log -p` 对这批文件永久失效
  2. 真实的 13 行改动被埋在噪声里，code review 无从进行
  3. Windows 侧 pull 后再被工具改回 CRLF，形成**两平台来回翻转的提交战争**（与
     [NPM-LOCK-CROSSPLAT-001] 的 package-lock 互删属同一类跨平台锁抖动）
- **处理规则（两平台都适用）**：
  1. **提交前必跑** `git diff --stat --ignore-cr-at-eol`，与 `git diff --stat` 对比。数字差距大 = 存在行尾噪声
  2. 只提交真实改动文件（逐个 `git add <file>`），**不要 `git add -A`**
  3. 行尾噪声文件**不得**用 `git checkout --` 批量还原（[GIT-RESET-INCIDENT-001] 禁令）；
     正确做法是不 add 它们、让其继续悬在工作区，或由 Gavin 拍板后一次性统一行尾
- **根治方案（需 Gavin 拍板，勿擅自执行）**：加 `.gitattributes` 写 `* text=auto eol=lf`（或 `eol=crlf`）
  统一全仓行尾。**代价是一次涉及全仓的规范化提交**，会一次性冲掉大量 `git blame` 归属，
  且必须在两平台无未提交改动时做。**在拍板前，本条只作为提交前检查项使用**

---

## [TOML-SECTION-DRIFT-001] 🔴 条件依赖段头插在表中间，会把后续依赖静默改判为单平台【两侧都必读】

**状态**：🟡 已定位，修复进行中（2026-07-30 发现于 macOS 侧 cargo check 基线）

- **现象**：macOS 上 `cargo check --manifest-path src-tauri/Cargo.toml` 报 7 个错误（3 个逻辑类）——
  `src-tauri/src/qwen3.rs:1,5-8,25` 找不到 `futures_util` / `tokio_tungstenite`，
  `src-tauri/src/main.rs:170` 找不到 `rustls`。而这三个 crate **明明写在 Cargo.toml 里**。

- **根因**：提交 `292eeb0` 把 `windows` 依赖移入条件依赖段时，**把段头插在了 `[dependencies]` 表的中间**：

      [dependencies]
      ...
      chrono = { ... }

      [target.'cfg(target_os = "windows")'.dependencies]     ← 段头插在这里
      windows = { ... }

      tokio-tungstenite = { ... }    ← 本意是共享依赖，实际已归属上面的 Windows 段
      futures-util = "0.3"           ← 同上
      rustls = { ... }               ← 同上

  **TOML 语义：段头之后的所有键都归属该段，直到下一个段头。** 中间的空行、注释都不构成分隔。
  于是三个本应共享的依赖被静默改判为 Windows 专属。
  改动前原文可用 `git show 292eeb0^:src-tauri/Cargo.toml` 核对，三者确实都在 `[dependencies]` 内。

- **为什么在 Windows 侧完全不可见（本条的核心教训）**：
  在 Windows 上 `cfg(target_os = "windows")` 命中，三个依赖照常解析，`cargo check` **0 errors**。
  提交信息里那句「`cargo check --manifest-path src-tauri/Cargo.toml` 0 errors」**是真的，但它拦不住这个错**。
  破坏只在对侧平台显形 —— 与 DEC-033/DEC-034 描述的 cfg 漂移是同一类风险，
  但**发生在依赖清单层而非代码层**。

- **危害不止编译失败**：`rustls` 那行是 `BUG-QWEN3-CRYPTO-001` 的修复
  （`src-tauri/src/main.rs:170` 进程级安装 ring provider，须在任何 TLS 使用之前）。
  被划成 Windows-only 意味着 **macOS 侧连这个 crypto 修复一起丢了**，
  而 qwen3 在线 ASR 是跨平台功能。**编译错误只是症状，功能缺失才是实害。**

- **为什么静态审计查不出**：`docs/MACOS-BRANCH-AUDIT.md` 的方法是扫描源文件里所有
  `#[cfg(...)]` 分支。而 `qwen3.rs` 与 `main.rs:170` 的代码**本身没有任何 cfg 守卫**——
  漂移在 `Cargo.toml` 里。**「依赖层 cfg」与「代码层 cfg」的同步缺口，是源码级 cfg 审计的结构性盲区。**

- **修复**：把 `[target.'cfg(...)'.dependencies]` 段**整体移到所有共享依赖之后**，
  只留 `windows` 在其中。不要反过来给 `qwen3.rs` 加 cfg —— 那会真的砍掉 macOS 的功能，方向相反。

- **预防规则（两侧都适用）**：
  1. **新增任何 `[target.'cfg(...)'.dependencies]` 段，一律追加到文件的依赖声明末尾**，
     绝不插在既有 `[dependencies]` 表中间
  2. 改完 `Cargo.toml` 的段结构后，**必须肉眼确认段头之后到下一个段头之间的每一行都确实是该平台专属的**
  3. 本地 `cargo check` 通过**不构成**「没有破坏对侧」的证据——这正是无 CI 状态下的固有缺口（DEC-034）

---

## [SHELL-BASHSOURCE-ZSH-001] ⚠️ `${BASH_SOURCE[0]}` 在 zsh 下为空，而 macOS 默认 shell 就是 zsh【macOS 侧必读】

**状态**：🟢 仓库内已修（2026-07-30 MACOS-PR1-SCRIPTS-001）；协作框架侧未修

- **机制**：`BASH_SOURCE` 是 **bash 专有数组变量，zsh 不提供**。脚本里常见的自定位写法
  `_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"`，在 zsh 下 `${BASH_SOURCE[0]}` 为空 →
  `dirname ""` 得 `.` → `./..` → **解析成当前工作目录的父目录**，而不是脚本所在目录的父目录。
  **不报错、静默得到错误路径**，这是它最难查的地方。

- **本项目的两处实例**：

  | 位置 | 后果 | 状态 |
  | --- | --- | --- |
  | `scripts/env-macos.sh:11` | `SHERPA_ONNX_LIB_DIR` 指向仓库**父目录**下不存在的路径 → `sherpa-onnx-sys` 的 build.rs 直接 panic。**即 `docs/BUILD-MACOS.md` §一 教给所有新人的 `source scripts/env-macos.sh` 在 macOS 默认 shell 下一直是坏的** | ✅ 已修 |
  | `collab/lib/env.sh:17`（协作框架，不在本仓库） | `$COLLAB` 解析成 `/Users/gavinsun/Workspace`，`dispatch.sh` 报「task.md 为空」 | ❌ 未修，绕法：用 `bash -c 'source .../dispatch.sh; dispatch <id>'` 调用 |

- **修法**：`${BASH_SOURCE[0]:-$0}` —— bash 下 `BASH_SOURCE[0]` 优先；zsh 下 `source` 时 `$0` 即脚本路径。
  一处改动，双 shell 兼容。

- **验证方式（必须双 shell 各跑一次，只测 bash 会漏）**：

      bash -c 'source scripts/env-macos.sh && echo "$SHERPA_ONNX_LIB_DIR" && ls "$SHERPA_ONNX_LIB_DIR" | head -3'
      zsh  -c 'source scripts/env-macos.sh && echo "$SHERPA_ONNX_LIB_DIR" && ls "$SHERPA_ONNX_LIB_DIR" | head -3'

  反向验证（确认 bug 真实存在）：在 zsh 下手工跑旧写法，应得到仓库的**父目录**：

      zsh -c 'echo "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"'

- **推广**：macOS 侧新增任何需要自定位的 shell 脚本，一律用 `${BASH_SOURCE[0]:-$0}`，
  并在两种 shell 下各验证一次。Windows 侧的 `.ps1` 脚本不受影响。

---

## [NPM-CI-LOCK-DESYNC-001] ⚠️ `npm ci` 在两个平台都跑不了：lock 与 package.json 长期失同步

**状态**：🟡 已定位，未修（2026-07-30 发现；修复需两侧协同，待 Gavin 决策）

- **现象**：`cd ui && npm ci` 报 `EUSAGE`：

      Missing: @emnapi/core@1.11.3 from lock file
      Missing: @emnapi/runtime@1.11.3 from lock file
      Invalid: lock file's @emnapi/wasi-threads@1.2.2 does not satisfy @emnapi/wasi-threads@1.2.3

- **性质**：三者均为**传递依赖**（`package.json` 里没有也不该有 `emnapi`，它经由 tauri/wasm 相关链路引入），
  lock 的传递树相对声明范围已陈旧。`git log -- ui/package-lock.json` 只有两次提交
  （`680d78f` 初始 + `f10c1e0` v0.6.2），即**该失同步长期存在、非任何一次改动引入**。

- **影响面（重要）**：`docs/MACOS-HANDOFF.md` §6.1 提出的「两侧统一用 `npm ci`」跨平台约定，
  **当前在任何一个平台上都无法执行 —— Windows 侧同样会 EUSAGE**。这不是 macOS 独有问题。

- **限度（不必恐慌）**：只卡**全新 clone**。现有 `ui/node_modules/` 完好，
  `npm run build` / `npx tsc --noEmit` 均正常（2026-07-30 实测 48 modules 通过）。

- **不可接受的「修法」**：直接 `npm install`。它会按当前平台裁剪 lock 里的 optional 依赖
  （见 `[NPM-LOCK-CROSSPLAT-001]`，实测 +39/−462 行），把对侧平台的二进制条目整段删掉，
  提交回去就轮到对侧 `npm ci` 失败。`npm install --no-save` **同样不行**——
  它只是不写 `package.json`，npm 9+ 仍会重写 lock。

- **当前的临时处置**（`scripts/setup-macos.sh`，2026-07-30）：调用 `npm ci`，
  **失败时响亮报错并 `exit 1`**，打印指向本条的说明，**绝不 fallback 到 `npm install`**。
  宁可让新人看到明确错误，也不要给他一个静默破坏对侧的 lock。

- **待决**：谁来修、用什么方式（`--package-lock-only` 是否能保住全平台 optional 并集需实测）、
  如何验证两平台都能 `npm ci`。**属共享文件，单侧修完对侧仍可能失败，建议与 Windows 侧协同。**

---

## [PYTEST-MACOS-COLLECT-001] ⚠️ macOS 上 pytest 收集期即崩：CT2 源码树 + Windows-only import【首次跑 pytest 前必读】

**状态**：🟡 已定位，未修（2026-07-30 macOS 测试环境部署后主控核查发现）

macOS 测试环境已装好（`voice-ime/.venv`：pytest 9.1.1 / playwright 1.61.0 + Chromium 149 /
pyautogui 0.9.54 / psutil / pytest-timeout / python-dotenv），但**装好 ≠ 能跑**，有两道坎：

### 坎 1：仓库根目录裸跑 `pytest` 直接 INTERNALERROR

    36 tests collected, 24 errors
    INTERNALERROR> SystemExit: 0
      → target/debug/CTranslate2-4.6.0/third_party/ruy/third_party/googletest/
         googlemock/scripts/generator/cpp/gmock_class_test.py

- **根因**：pytest 递归走进 **CTranslate2 源码树里 googletest 自带的 Python 脚本**，
  那些脚本 import 阶段就 `sys.exit(0)`，把 collection 整个打崩。24 个错误全部来自 `target/debug`
- **为什么配置没拦住**：`pytest.ini` 位于 **`tests/` 下而非仓库根**，根目录调用时其
  `testpaths = tests/` 不生效
- **⚠️ 已实测：`pytest -c tests/pytest.ini` 也照样崩** —— `-c` 只指定配置文件，
  **不改变收集根目录**。这是最容易想当然踩空的一步
- **可用的临时办法**：显式限定路径 `pytest tests/`（但仍有坎 2）
- **根治方向**：仓库根加 `pytest.ini`（或 `pyproject.toml` 的 `[tool.pytest.ini_options]`），
  至少配 `testpaths = tests/` + `norecursedirs = target .venv node_modules vendor patches`
- **为什么 Windows 侧没暴露**：CT2 源码树是构建时下载到 `target/<profile>/` 的，
  且 Windows 侧走 `build-test-guide.md` 的既定流程、未在仓库根裸跑 pytest

### 坎 2：2 个 Windows-only 测试文件在 import 阶段炸，中断整个收集

限定 `pytest tests/` 后：

    147/156 tests collected (9 deselected), 2 errors
    !!!!!! Interrupted: 2 errors during collection !!!!!!

| 文件 | 报错 |
|---|---|
| `tests/test_cases/test_hotkey.py` | `AttributeError: module 'ctypes' has no attribute 'WinDLL'` |
| `tests/test_cases/test_full_pipeline_e2e.py` | `ModuleNotFoundError: No module named 'win32clipboard'` |

- **关键点：这是 error 不是 skip**。收集期中断会导致**后面 147 条测试一条都跑不了**，
  不是「跳过两个、其余照跑」
- **修法**：在这两个文件顶部加平台守卫，例如
  `pytestmark = pytest.mark.skipif(sys.platform != "win32", reason="Win32-only")`；
  但注意 `pytest.mark.skipif` **拦不住模块级 import**，若 `import win32clipboard` 写在文件顶层，
  需改为函数内延迟 import，或用 `pytest.importorskip("win32clipboard")`
- macOS 侧已有三个专有用例可作对照：`tests/test_cases/test_{hotkey,injection,overlay}_macos.py`

### 教训（验收口径）

本次 Worker 报告的自验是 `pytest --collect-only → 5/7 test_config tests 可发现` ——
那是**限定到单个文件的窄范围运行**，两道坎都被遮住。
**环境类验收的准绳应为「仓库根裸跑 + `tests/` 全量收集」两条都过**，而非挑一个能过的范围证明可用。

---

## [NPM-LOCK-CROSSPLAT-001] 补充修正（2026-07-30 macOS 侧实测）：`+39/−462` 这个特征数字会误导

本条原文记载：在 macOS 上跑 `npm install` 会把 lock 里 win32 的 optional 依赖条目
「**整段删除**」，实测 diff 为 **+39 / −462 行**。

**2026-07-30 执行 `MACOS-NPMLOCK` 修复时发现：`npm install --package-lock-only` 产生的 diff
也恰好是 `+39 / −462`，但它并不删除任何 win32 条目。** 逐条核验：

| 核验项 | 结果 |
| --- | --- |
| 12 个**顶层** win32 包条目（`@esbuild/win32-*` / `@rolldown/binding-win32-*` / `@rollup/rollup-win32-*` / `@tauri-apps/cli-win32-*`） | HEAD 与修复后 `comm` 逐条比对，**丢失数 0** |
| 被删的 26 个包条目 | 23 个是 `vitest/node_modules/@esbuild/*` **嵌套副本**（被 npm 11 去重到仍然存在的顶层同名条目），3 个是 `netbsd-arm64` / `openbsd-arm64` / `openharmony-arm64` 冷门平台 |
| 新增 | `@emnapi/core`、`@emnapi/runtime` —— 正是 EUSAGE 报缺的两个 |

**教训（这才是本补充的价值）**：

1. **不要用 diff 行数判断 lock 是否被破坏。** `-  "node_modules/vitest/node_modules/@esbuild/win32-x64"`
   这样的行看起来像「win32 条目被删」，实际删的是嵌套副本，顶层同名条目仍在。
   本条原文的「整段删除」判断很可能就是这样读出来的
2. **唯一可靠的判据是：比对顶层包条目是否丢失**：

       git show HEAD:ui/package-lock.json | grep -oE '"node_modules/@[^/"]*/[^"]*win32[^"]*"' | sort -u > /tmp/a
       grep -oE '"node_modules/@[^/"]*/[^"]*win32[^"]*"' ui/package-lock.json | sort -u > /tmp/b
       comm -23 /tmp/a /tmp/b        # 有输出才是真丢失

3. **本条对 `npm install` 的禁令仍然有效** —— 未在 macOS 上重跑 `npm install` 做对照实验
   （风险不对称：真破坏了就得再修一轮）。故**不能断言原记载有误**，只能确认
   `--package-lock-only` 是安全的，且 `+39/−462` 不构成「被破坏」的证据

## [ITN-PREFIX-SHADOW-001] ⚠️ 机器挖掘 ITN 保护词表的前缀遮蔽缺陷（已知并接受，Gavin 2026-07-30 拍板落地）【改词表前必读】

**结论先行**：从通用中文词库离线挖掘的保护词表存在**前缀遮蔽**这一结构性缺陷 —— 每条短词条会遮蔽所有以它开头的文本。**Gavin 2026-07-30 评估后拍板：缺陷远小于收益，1386 条照常落地出包端测。**

**⚠️ 严重性定性（主控首轮判断有误，此为修正后口径）**：

`check_protection` 命中后的行为是「把原文逐字抄出 + 游标前移」，**物理上不可能产出畸形文本**。因此两种失败模式**不对称**：

| 失败模式 | 输出 | 性质 |
| --- | --- | --- |
| **漏**保护 | `三角形` → `3角形` | **文本被改坏**，用户须手动修正 |
| **误**保护 | `二分钟` 保持汉字 | **优化未生效**，退回 ITN 之前的状态，文本仍正确可读 |

**误保护 = 优雅降级；漏保护 = 输出损坏。** 主控首轮写的「误保护比不保护更糟」**错误，已收回**（Gavin 2026-07-30 指出）。按此框架，用大词表换取覆盖率是合理取舍 —— 代价只是部分转换不发生，不会制造新的错误输出。

> ### ⚠️ 上段结论的适用条件（2026-07-31 补充，Gavin 端测实证反例）
>
> **「误保护 = 优雅降级」仅在「被遮蔽的语义单元内不含其他可转数字」时成立。** 存在第三种失败模式使该结论不适用：
>
> #### 失败模式三 · 撕裂（tearing）
>
> Gavin 2026-07-31 实测：`这件商品的价格是十一块九毛二。` → **`这件商品的价格是十一块9毛2。`**
>
> 根因：`check_protection` 命中后只做「原文抄出 + **游标前移**」（`src/itn.rs:697-703`），**不锁定后续字符**。故：
>
> | 步 | 游标 | 判定 | 输出 |
> | --- | --- | --- | --- |
> | 1 | `十一` | 命中 `[protect.proper_nouns]` 的 `"十一"`（国庆节义，`itn-rules.toml:124`） | `十一` 原文抄出，游标 +2 |
> | 2 | `九` | 后跟 `毛` ∈ currency → `is_unit` → 转 | `9` |
> | 3 | `二` | `consumed==1` 且前置单位 `毛` → `is_unit_preceded`（`:330`/`:1070`） | `2` |
>
> **保护只挡住语义单元的前半段，后半段照转** → 产出一半汉字一半数字的混合体。
>
> | 模式 | 输出 | 性质 |
> | --- | --- | --- |
> | 漏保护 | `三角形`→`3角形` | 输出损坏 |
> | 误保护 | `二分钟` 保持汉字 | 优雅降级（整体退回原状，仍可读） |
> | **撕裂** | `十一块九毛二`→`十一块9毛2` | **同一语义单元被劈成两半，比误保护严重** |
>
> **影响面远大于单个 bug**：`十一` 遮蔽的不止金额，还有 `十一点半`/`十一个`/`十一岁`/`十一公里`。白名单中所有**裸 2 字数字词**（十一/五一/七一/八一/三亚/四川/九江…）同属风险源。
>
> #### 连带发现 · 保护词表对规则性语法族的覆盖是随机的
>
> 主控 2026-07-31 全量盘点 `X点半`：
>
> | 在保护表内 | **不在**保护表内 |
> | --- | --- |
> | 一点半、六点半、八点半、九点半 | 二、三、**四**、五、七、十点半 |
>
> `一吨半` 在表内而 `两吨半` 不在，同理。**后果**：用户看到同一表达因数值不同行为完全相反 —— `八点半`→全汉字，`四点半`→`4点半` 撕裂。Gavin 报的 `四点半` 只是露出水面的那一个。
>
> **根因**：1386 条是从**词频表**机器派生的，高频组合被收录、低频的没有 → **把一个规则性语法族切成了随机子集**。
>
> #### 由此确立的原则（→ DEC-038）
>
> 1. 保护词表只承载**不可推导的专名与习语**（三亚、一心一意、五代十国），**不得承载可由文法规则推导的表达**（`N点半`、`N<单位>半`、`N<单位>M`）
> 2. 规则性语法族一律交 ITN 文法引擎处理（DEC-037 甲/乙/丙型）
> 3. **删保护词条与文法上线必须成对交付** —— 先删会让 `八点半` 立刻变 `8点半` 撕裂；先做文法不删词条则 `八点半` 永远走不到文法
> 4. 后续任何机器派生词表落地前，**必须先做「是否切割了规则性语法族」的检查**
>
> **对 Type B（29,774 条）的影响**：撕裂风险按词表规模等比放大，且 Type B 的词以**单位字开头**（`度假`/`元素`/`批发`），遮蔽形态与 Type A 不同，立项前须单独做撕裂面量化。

下文的缺陷分析仍然成立，作为**改动此词表时的必读注意事项**保留，而非否决理由。

### 一、这个方案想解决什么

`is_unit` 用 `s.starts_with(u)` 判定单位（`src/itn.rs:259`）+ 中文无词边界 → 任何「中文数字 + 单位首字」开头的词都会让前面的数字被误转。实证案例：`三角形` → `3角形`（「角」是货币单位，`角形`.starts_with(`角`) 为真）。

设想的解法：从公开词库离线挖出所有这类词，作为整词保护白名单塞进 `check_protection`。

### 二、三个致命缺陷（逐条有实证）

#### 缺陷 1 · 挖掘定义框住的是量词组合空间，不是碰撞集

Type A 的定义「词以中文数字开头、第二字为单位首字」在中文里命中的是一个**巨大的组合空间**，绝大多数是合法量词表达，不是需要保护的固定词。

实测 1386 条最终表中，`一个X` 前缀 135 条，抽样 30 条：`一个团` / `一个桶` / `一个纸` / `一个盆` / `一个整` / `一个响` / `一个双` / `一个九十度` / `一个二十五岁` / `一个三十岁`。**这些转换（`一个团`→`1个团`、`一个九十度`→`一个90度`）都是正确且期望的 ITN 行为**，把它们放进保护表不是修 bug，是造 bug。

#### 缺陷 2 · 前缀遮蔽：危害外溢到无穷多文本（最致命，Gavin 2026-07-30 指出）

`check_protection` 的匹配是 `rest.starts_with(词条)`（`src/itn.rs`），命中后**整词跳过**。因此**每一条短词条都会遮蔽所有以它开头的文本**：

| 文本（真实听写会说） | 被哪条词条遮蔽 | 后果 |
| --- | --- | --- |
| `二分钟` | `二分` | 该转 `2分钟`，整段跳过不转 |
| `三元一斤` / `三元钱` | `三元` | 该转 `3元一斤` |
| `九度电` | `九度` | 该转 `9度电` |
| `一点半左右` | `一点半` | 该转 `1点半左右` |
| `二分之一秒` | `二分之一` | 该转 `1/2秒` |

**1386 条中 603 条（44%）为 ≤3 字**，每一条都是一个遮蔽源。受污染的不是词表内的 1386 条，而是**所有以这 603 条开头的表达** —— 一个开放集合，无法穷举，无法测试覆盖。

**「桶内按长度降序排序」救不了它**：降序只保证在**词表条目之间**选最长的；而 `二分钟` 根本不在词表里，`二分` 无对手直接胜出。最长匹配解决的是词表内部歧义，解决不了「词条本身就不该匹配」。

#### 缺陷 3 · 不存在能分离两者的机械判据

区分 `三角形`（该保护）与 `八点半`（该转）需要的是「这个词是不是词典固定搭配、单位义是否成立」的**语义标签**，源词库不带。实测两个候选过滤器均失败：

- **后缀黑名单**（剔除结尾落在 `units.*` / `date_time.triggers` / `classifiers` 的条目）：剔 93 条，其中 **40 条是本该保护的**（`一个九十度` / `零摄氏度` / `一年一度` / 35 个「一个X十Y岁」年龄表达 —— 恰是阶段一 F1 方案特意从 F2 手里救回来的）；同时**漏掉 `一分钟`**（结尾「钟」不在 units 列表内）。
- **动态筛法**（跑 `normalize_numbers`，只保留「输出≠输入」的条目）：**逻辑不成立**。`八点半`→`8点半` 输出也≠输入，会被保留而非剔除，与既有单测 `src/itn.rs` 的 `time_half`（`assert normalize_test("八点半") == "8点半"`）直接矛盾。该判据只能筛掉「完全没转换」的条目，实测为 **0 条**。

### 三、为什么既有手工白名单是安全的（本质差别，不是数量差别）

既有 `[protect.idioms/proper_nouns/historical/function_words/classifiers]` 共 254 条同样走前缀匹配，为何没有遮蔽问题？

因为手工条目都是**完整的词汇单位**（成语 `一心一意`、地名 `八达岭`、专名 `二锅头`）。`八达岭` 遮蔽 `八达岭长城` 无害 —— 两者都本该保持汉字。

而机器派生的条目是**组合片段**（`三元` / `二分` / `九度`），片段的延续多半是合法量词表达。**手工表安全是因为条目是词汇单位，不是因为条目少。** 因此「先小规模试点、再逐步扩大」这条退路也不成立 —— 规模不是变量，条目性质才是。

### 四、改动此词表时的操作纪律

**新增条目前必问一句：这个词做前缀会遮蔽掉什么？** 词条越短，遮蔽面越大。理想条目是**完整词汇单位**（成语/地名/术语），而非组合片段。

**若日后需要收窄**：优先剔除 ≤3 字条目（603 条，遮蔽面最大），而非按后缀黑名单剔（实测后缀黑名单会误删 40 条本该保护的词，见 §二 缺陷 3）。

已知的真实 Type A 碰撞（`三角形` / `三角洲` / `三角函数`）已由提交 `a07a089` 的几何白名单覆盖。今后遇到新碰撞按上法逐条加 —— 纯数据改动、免构建、免出包（前提是并入既有分组而非新增分组，见下）。

### 五、连带教训：新增 toml 分组必然要改 Rust

`Rules` / `Protect` 结构体**未启用 `deny_unknown_fields`**（实测 grep 命中 0）→ toml 里出现未知段落会被 **serde 静默忽略**，不报错、不警告、日志照常打 `ITN rules loaded from ...`。

含义：**新增 `[protect.xxx]` 分组在 Rust 侧加字段之前是完全的死数据**，且失效无任何可观测信号。若追求「纯数据、免构建」，必须并入既有分组。

### 六、Type B 的风险倍数（仍待排期，非撤案）

ITN-COLLISION-TYPEB-001（29,774 条，单位首字开头）出自**同一挖掘定义**，上述缺陷全部适用且**遮蔽面显著更大**（条目更短、更常见，且 Type B 词不带数字前缀 → 单条覆盖面更广）。规模是 Type A 的 21 倍。

**立项时必须先拿 Type A 的端测反馈做依据** —— 若 Type A 的误保护在实际使用中无感，Type B 可按同法推进但需先做遮蔽面量化；若 Type A 已明显影响体感，Type B 直接放弃。

### 七、本次留下的资产

- `collab/research/itn-lexicon-collision-001.md` —— 调研报告 + 阶段一报告（含三份词库的**许可证原文取证**，jieba/THUOCL 为 MIT 可商用闭源分发，CC-CEDICT 为 CC BY-SA 有 share-alike 传染风险 —— 这部分证据独立于本方案，日后仍可复用）
- `collab/research/data/` —— 中间产物，已在 `.gitignore` 内不入库

> 记录人：orchestrator ｜ 2026-07-30 ｜ 触发人：Gavin（缺陷 2 由其提出的 `第二点都不满意` 反例引出）

## [COLLAB-ACK-001] 补充（2026-08-02）：第三次假警报 —— Worker 正常工作但漏写 ack 文件，且重发反而伤害 Worker

**现象**：2026-08-02 23:57 派发 PROMPT-ARCH-018 给 coder-2，`dispatch.sh` 连发 3 次后报 `[ACK_FAIL] coder-2 经 3 次发送均无应答`。

**实况**（主控 capture-pane 取证，未采信告警）：coder-2 **完全正常**，正在逐段 Read `src/llm/mod.rs` 测试块设计分层方案，上下文已用 124.7K（62%）。它只是**没执行 `echo ACK > task_ack.md` 这一步**——ack 文件 0 字节，而 `dispatch.sh:151` 用 `[[ -s "$ack_file" ]]`（非空）判定，于是判为无应答。

**若照 ACK_FAIL 字面重启，会直接丢弃 124.7K 上下文的分析成果。**

**新增的两点认识（前两次未记录）**：

1. **重发机制对「正在干活的 Worker」是有害的**。`dispatch.sh` 的 ACK 监控在超时后会**把完整派发消息重新 send-keys 一遍**（`:171`）。coder-2 因此在 90 秒内收到 3 条完全相同的任务通知，每条都会被当作一个新 turn 处理，白白消耗 turn 与上下文。**Worker 越忙（读文件慢、不及时写 ack），越容易触发重发，而重发又进一步拖慢它 —— 正反馈。**
2. **ACK 超时参数与任务规模不匹配**。`ACK_TIMEOUT=15` 秒对「收到任务先读 4 份协作文档 + 读目标源文件」的大任务过短。派发大任务（任务书 >200 行 / 目标文件 >2000 行）时应预期首次 ACK 晚于 15s。

**正确处置（本次执行的）**：

1. `capture-pane` 确认活着 → **不重启**
2. tmux 单发一条短消息：让它补写 ack 文件 + 告知「重发 3 次是同一任务，按一次处理」+ 上下文预算提醒
3. 不再触碰 `dispatch.sh`（`[COLLAB-PATH-SPLIT-001]` 未收口前不改它）

**待办**：`dispatch.sh` 的重发逻辑应改为「重发前先 capture-pane，检测到 Worker 正在活动则只延长等待、不重发消息」。与 `[COLLAB-PATH-SPLIT-001]` 一并处理。

## [BUGREPORT-SELFCORRUPT-001] ⚠️ 用本 IME 口述的 bug 报告，可能被 bug 本身污染【主控必读】

**事件**：2026-08-02 Gavin 报 ITN 数值错误，其中一条写作：

> 「这个西瓜是一块两毛二**已经**，会被转换成 22.20 元」

「已经」在该句中语法不通。主控未直接采信报告文本，而是**从错误值反推算术**：

```
一块(1) + 两毛(0.2) + 逐位串「二一」(21) = 22.2  →  22.20元   精确匹配
```

据此推断真实输入为「一块两毛二**一斤**」，「已经」是 ASR 把「一斤」（yī jīn）听成「已经」（yǐ jīng）的音近错误。**Gavin 2026-08-03 亲口确认：「对，我说的是一块两毛二一斤」。**

**这条如果不查出来会怎样**：coder-1 已按报告原文测「一块两毛二」，得 `1.22元` 正确，据此汇报「#2 已正确，不在修复范围」。**主控若采信，六条 bug 会只修五条，剩下这条端测必然复现。**

**教训**：

1. **Gavin 用这个输入法报 bug，报告文本本身就经过 ASR + ITN + LLM 三道加工，可能被它要报告的那个 bug 污染。** 报告里的「输入原文」不是原始事实，是**又一次经过管线的产物**。
2. **错误值本身是最可靠的证据。** 本例中「22.20」这个数比报告里的文字更可信——主控靠算术反推锁定了真实输入。**修 ITN 类数值 bug 时，应优先用错误值反推输入，而不是照抄报告文本去复现。**
3. Worker 用报告原文测出「正确」时，**这不构成「不是 bug」的结论**，只说明「这个输入不触发」。主控必须交叉核对报告文本的语法合理性。

**规则**：

- 主控收到端测 bug 报告，**先检查文本是否语法自洽**；出现语法突兀的词（本例「已经」）先怀疑是 ASR 音近错误，用错误值反推校验
- Worker 汇报「按报告原文测是正确的」时，**主控必须回到错误值做算术复核**，不得据此缩小修复范围
- 拿不准时**直接问 Gavin 原话**（本例一句话就确认了）
