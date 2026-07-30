# voice-ime 测试与构建规范

> 本文件是 tester-1 执行构建/验证任务的权威参考。
> 上一个版本：`collab/build-guide.md`（已整合进本文档）

---

## 一、构建流程

### 构建原则

**不要频繁出包。** 构建耗时约 47s，必须由 Orchestrator 明确指令后才出包。

| 场景        | 操作                    |
| --------- | --------------------- |
| 单个代码改动完成  | `cargo check`，**不出包** |
| 多个任务全部完成  | 合并一次 release 构建       |
| 需要端测/视觉验收 | 才出 release 包          |

### Release 构建流程（3 步，严格按顺序）

#### Step 1 — 退出运行中进程

```bash
powershell -Command "Get-Process feiyin-ime,voice-ime-ui -ErrorAction SilentlyContinue | Stop-Process -Force"
```

#### Step 2 — 前端 + Tauri UI

```bash
# 前端构建（~600ms）
cd /d/Workspace/CodeLab/voice-ime/ui && npm run build

# Tauri UI release（含 custom-protocol 内嵌前端）
cd /d/Workspace/CodeLab/voice-ime
cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol
```

#### Step 3 — 主程序

```bash
cd /d/Workspace/CodeLab/voice-ime
cargo build --release
```

### 产物位置【强制规则】

| 产物   | 最终路径                              |
| ---- | --------------------------------- |
| 主程序  | `target/release/feiyin-ime.exe`    |
| 配置界面 | `target/release/feiyin-ime-ui.exe` |

> ⚠️ Tauri UI 构建产物实际可能落在 `src-tauri/target/release/`，需手动复制到 `target/release/`。

### Step 4 — 同步到 Publish/【强制，出包必须执行，不可跳过】

**bash / WSL：**

```bash
cp target/release/feiyin-ime.exe Publish/
cp target/release/feiyin-ime-ui.exe Publish/
cp target/release/crash-reporter.exe Publish/
ls -la Publish/*.exe
```

**PowerShell：**

```powershell
Copy-Item -Path "target\release\feiyin-ime.exe" -Destination "Publish\" -Force
Copy-Item -Path "target\release\feiyin-ime-ui.exe" -Destination "Publish\" -Force
Copy-Item -Path "target\release\crash-reporter.exe" -Destination "Publish\" -Force
Get-ChildItem -Path "Publish\*.exe" | Select-Object Name, LastWriteTime, Length
```

确认三个文件时间戳均为本次构建时间，Publish/ 为发布暂存目录，必须与 target/release/ 保持同步。出包任务必须执行此步骤，不可跳过。

### 预期构建时间 & 产物大小

| 步骤               | 耗时       | 产物大小       |
| ---------------- | -------- | ---------- |
| npm build        | ~600ms   | ~181 KB    |
| Tauri UI release | ~20s     | ~22 MB     |
| 主程序 release      | ~27s     | ~31 MB     |
| **合计**           | **~47s** | **~53 MB** |

### ⚠️ 已知注意事项

| 问题                            | 说明                                   |
| ----------------------------- | ------------------------------------ |
| `cargo tauri build` 报错        | Tauri v1 与 tauri-cli v2 不兼容，**禁止使用** |
| `拒绝访问 (os error 5)`           | 进程占用文件锁，先执行 Step 1 再重试               |
| Tauri UI 空白页 / localhost 拒绝连接 | 缺少 custom-protocol feature           |

---

## 二、测试框架总览（三层框架）

| 层级             | 框架                       | 命令                             | 适用场景                   |
| -------------- | ------------------------ | ------------------------------ | ---------------------- |
| **前端单元**       | Vitest + Testing Library | `cd ui && npm run test`        | React 组件逻辑、状态、Mock API |
| **WebView UI** | Playwright + CDP         | `pytest test_webview_ui.py`    | Tauri UI DOM/CSS/交互    |
| **Win32 E2E**  | pywinauto + pytest       | `pytest tests/test_cases/*.py` | 托盘/热键/Overlay/全流程      |

### Rust 单元测试

| 层级      | 框架         | 命令                                                               |
| ------- | ---------- | ---------------------------------------------------------------- |
| Rust 单元 | cargo test | `cargo test` / `cargo test --manifest-path src-tauri/Cargo.toml` |

### 场景匹配表

| 修改类型        | Step 1       | Step 2   | Step 3  | Step 4                   | 说明      |
| ----------- | ------------ | -------- | ------- | ------------------------ | ------- |
| 只改 React/UI | ❌ SKIP       | ✅ Vitest | ❌ SKIP  | ✅ Playwright             | 前端双测    |
| 只改 Rust 后端  | ✅ cargo test | ❌ SKIP   | ❌ SKIP  | ✅ pywinauto              | 后端+E2E  |
| 改前后端        | ✅ cargo test | ✅ Vitest | ❌ SKIP  | ✅ pytest 全量              | 全覆盖     |
| 改配置/文档      | ❌ SKIP       | ❌ SKIP   | ✅ smoke | ✅ pytest                 | 快速验证    |
| 改 Tauri 配置  | ❌ SKIP       | ❌ SKIP   | ❌ SKIP  | ✅ Playwright + pywinauto | UI + 进程 |

### 各框架能力边界

**Vitest（前端单元）**

- ✅ React 组件渲染、状态逻辑、Mock Tauri invoke
- ❌ 无法测试真实 WebView2 DOM / CSS 实际渲染

**Playwright + CDP（WebView UI）**

- ✅ 连接真实 WebView2、DOM 元素定位、CSS 验证、Tab/Toggle 交互
- ❌ 需要 feiyin-ime.exe 运行、无法测试托盘/热键

**pywinauto + pytest（Win32 E2E）**

- ✅ 托盘交互、Overlay 窗口、全局热键（SendInput）、进程生命周期
- ❌ 无法深入 WebView2 DOM

---

## 二·五、TEST-SYNC 任务规范【tester-1 必读】

TEST-SYNC 任务在代码任务派发的同时下达，与 coder 并行执行。

### TEST-SYNC = 评估 + **编写测试用例**（不只出分析报告）

| 步骤 | 操作 | 说明 |
|------|------|------|
| 1 | 分析改动范围 | 读 task.md，了解代码改动内容 |
| 2 | 评估可测性 | 哪些逻辑可自动化，哪些只能目视 |
| 3 | **编写测试用例** | 在测试文件中直接写好，不只写分析报告 |
| 4 | 说明覆盖结论 | 已写用例 + 不可自动化项的目视验收建议 |

### 编写测试用例的范围

| 类型 | 测试文件位置 | 说明 |
|------|------------|------|
| Rust 逻辑函数 | `src/main.rs`（`#[cfg(test)]` 块内） | 只改测试区，不动生产代码 |
| 前端组件 | `ui/src/*.test.tsx` | Vitest 单测 |
| E2E / 系统行为 | `tests/test_cases/*.py` | pytest 用例 |

**⛔ 禁止**：TEST-SYNC 阶段执行任何测试命令（cargo test / pytest / npm test）

### 完成标准
- result.md 中列出：已编写的测试用例清单 + 不可自动化项的目视验收建议
- 不要只写"无法自动化"，能写的测试必须写好

---

## 三、测试执行流程

### Python 环境【强制】

**必须使用 Python 3.11**（Python 3.14 不兼容 pyautogui）。

```bash
py -3.11 --version    # 应输出 Python 3.11.9
```

### 依赖安装

```bash
# pytest + GUI 自动化
py -3.11 -m pip install pytest pyautogui pywinauto pytest-timeout

# Playwright（首次使用需安装浏览器）
py -3.11 -m pip install playwright
playwright install chromium
```

### 测试执行 Step

| Step    | 命令                                                                       | 耗时   | 用途                            | 何时执行                                           |
| ------- | ------------------------------------------------------------------------ | ---- | ----------------------------- | ---------------------------------------------- |
| Step 1  | `cargo test`                                                             | ~10s | Rust 单元测试                     | 改了 `src/` 或 `src-tauri/src/` Rust 代码           |
| Step 2  | `cd ui && npm run test`                                                  | ~2s  | Vitest 前端单元                   | 改了 `ui/src/` React 组件（.tsx/.ts）                |
| Step 3  | `py -3.11 -m pytest tests/test_cases/ -m smoke -v`                       | ~15s | pytest 冒烟（需构建产物）              | release 构建后快速验证进程启动/退出                         |
| Step 4a | `py -3.11 -m pytest tests/test_cases/test_webview_ui.py -v --timeout=60` | ~15s | Playwright WebView UI         | 改了 CSS/布局/Tauri 配置/前端交互（需 feiyin-ime-ui.exe 运行） |
| Step 4b | `py -3.11 -m pytest tests/test_cases/ -m "not webview" -v`               | ~60s | pywinauto Win32 E2E           | 改了热键/托盘/Overlay/文字注入/进程管理                      |
| Step 4c | `py -3.11 -m pytest tests/test_cases/ -v`                                | ~90s | pytest 全量（含 Playwright + E2E） | 大版本发布/回归测试/前后端都改了                              |

### ⚠️ 强制规则：任何代码修改后必须先构建对应产物

> **原则**：任何代码修改（前端 CSS/React 或后端 Rust）后执行自动化测试前，**必须先构建对应产物**。否则测试连接的是旧产物，修改不会生效。
> 构建后必须验证产物时间戳为当前构建时间（`ls -la target/release/*.exe`）。

### ⚠️ 强制规则：tauri.conf.json 修改必须完整构建

> **教训来源**：2026-04-22 WINDOW-TITLEBAR 系列任务暴露，coder-2 只做 npm build（前端），不做 cargo build --release（Tauri 后端），导致 tauri.conf.json 配置修改未打包进 exe，tester-1 测试的是旧产物。

**tauri.conf.json 改动后的完整构建步骤**：

```bash
# 1. 清理旧进程
taskkill /F /IM feiyin-ime-ui.exe /T

# 2. 前端构建（~600ms）
cd ui && npm run build

# 3. Tauri UI 构建（必须带 --features custom-protocol）
cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol

# 4. 同步产物（强制）
cp src-tauri/target/release/feiyin-ime-ui.exe target/release/feiyin-ime-ui.exe

# 5. 确认产物时间戳（强制）
ls -la target/release/feiyin-ime-ui.exe
# 输出时间必须为当前时间，否则测试仍命中旧产物
```

**Orchestrator 责任**：
- tauri.conf.json 改动后，必须派发 BUILD 任务给 tester-1
- 不能只让 coder 做 npm build，必须完整构建链

| 修改类型 | 必须构建的产物 | 构建命令 |
|----------|---------------|----------|
| **tauri.conf.json 窗口配置** | `feiyin-ime-ui.exe` | **完整链**：npm build + cargo build（custom-protocol）+ 同步产物 + 确认时间戳 |
| 前端 CSS 修改 | `feiyin-ime-ui.exe` | `npm run build` + `cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol` |
| 前端 React 修改 | `feiyin-ime-ui.exe` | `npm run build` + `cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol` |
| 后端 Rust 修改 | `feiyin-ime.exe` | `cargo build --release` |
| 前后端同时修改 | `feiyin-ime.exe` + `feiyin-ime-ui.exe` | 完整执行 Step 1/2/3（见「一、构建流程」） |

### Playwright + CDP 执行前置条件

> Playwright 测试连接的是 `feiyin-ime-ui.exe` 的 WebView2 实例，执行前必须确保产物包含最新 CSS/React 修改。

```bash
# 0. 【必须先构建】确保 feiyin-ime-ui.exe 包含最新前端修改
powershell -Command "Get-Process feiyin-ime,voice-ime-ui -ErrorAction SilentlyContinue | Stop-Process -Force"
cd /d/Workspace/CodeLab/voice-ime/ui && npm run build
cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol
cp src-tauri/target/release/feiyin-ime-ui.exe target/release/  # 同步产物
ls -la target/release/feiyin-ime-ui.exe  # 验证时间戳为当前

# 1. 确保 feiyin-ime-ui.exe 运行且 CDP 端口 9222 暴露
#    方法 A: 启动主程序时设置环境变量
WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"
./target/release/feiyin-ime.exe

#    方法 B: 直接启动配置窗口（测试脚本会自动拉起）
./target/release/feiyin-ime-ui.exe

# 2. 验证 CDP 端口就绪
curl http://localhost:9222/json/version  # 应返回版本信息

# 3. 执行 Playwright 测试
py -3.11 -m pytest tests/test_cases/test_webview_ui.py -v --timeout=60
```

### 常用测试命令

```bash
# 单文件测试
py -3.11 -m pytest tests/test_cases/test_hotkey.py -v

# Playwright 专项
py -3.11 -m pytest tests/test_cases/test_webview_ui.py -v --timeout=60

# 冒烟测试
py -3.11 -m pytest tests/test_cases/ -m smoke -v

# 跳过 WebView 测试（CDP 未就绪时）
py -3.11 -m pytest tests/test_cases/ -m "not webview" -v

# 跳过硬件测试（无需麦克风）
py -3.11 -m pytest tests/test_cases/ -m "not hardware" -v

# 全量测试
py -3.11 -m pytest tests/test_cases/ -v
```

### 测试标记（Markers）

| 标记         | 说明                     |
| ---------- | ---------------------- |
| `smoke`    | 冒烟测试（构建后快速验证）          |
| `hardware` | 需要硬件设备（麦克风）            |
| `gui`      | 需要 GUI 窗口              |
| `webview`  | Playwright WebView2 测试 |
| `cdp`      | 需要远程调试端口               |
| `tauri_v2` | Tauri v2 升级验证          |
| `e2e`      | 端到端全流程测试               |

---

## 四、pytest/pywinauto 技术栈

### 技术栈

- **pytest**: 测试框架核心
- **pytest-timeout**: 测试超时控制
- **pyautogui**: 键盘/鼠标模拟
- **pywinauto**: Win32 窗口操作
- **python-dotenv**: 环境变量管理

### 目录结构

```
tests/
├── conftest.py               # pytest 配置和 fixtures
├── test_cases/               # 具体测试用例
│   ├── test_hotkey.py        # 热键响应（SendInput）
│   ├── test_tray.py          # 系统托盘
│   ├── test_crash.py         # 崩溃处理
│   ├── test_injection.py     # 文字注入
│   ├── test_platform.py      # 跨平台抽象层
│   └── test_webview_ui.py    # Playwright WebView2
└── utils/                    # 测试工具
```

### 状态检测机制

基于 Win32 API 检测 overlay 窗口尺寸：

| 状态         | 窗口尺寸    |
| ---------- | ------- |
| Recording  | 240x36  |
| Processing | 200x36  |
| FocusLost  | 320x110 |
| Hidden     | 无窗口     |

### 注意事项

- 测试前确保 feiyin-ime.exe 未在运行
- 测试期间不要手动操作键盘/鼠标
- GUI 测试需构建完成 release 产物后再运行
- 测试完成后确保无残留进程

---

## 五、Vitest 前端单元测试

### 配置

- **配置文件**: `ui/vite.config.ts`（`test` 配置块）
- **Setup 文件**: `ui/src/test/setup.ts`（mock Tauri invoke + jest-dom）
- **环境**: happy-dom（轻量级 DOM 模拟）

### 运行命令

```bash
cd ui
npm run test         # 单次运行
npm run test:watch   # 监听模式
npm run test:ui      # UI 模式
```

### Tauri API Mock

```typescript
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    // 返回 mock 配置
  }),
}));
```

### 示例测试

```typescript
import { render, screen } from "@testing-library/react";
import AboutPage from "../pages/About";

describe("AboutPage", () => {
  it("renders page title", () => {
    render(<AboutPage config={mockConfig} updateConfig={vi.fn()} />);
    expect(screen.getByText("飞音语音输入")).toBeInTheDocument();
  });
});
```

---

## 六、Playwright WebView UI 测试

### 前置条件

```bash
py -3.11 -m pip install playwright
playwright install chromium
```

### CDP 连接原理

WebView2 启动时通过 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"` 暴露 CDP 端口，Playwright 连接后操控 DOM。

### 运行命令

```bash
py -3.11 -m pytest tests/test_cases/test_webview_ui.py -v --timeout=60
```

### 已知问题

- **Fixture 作用域**：`cdp_browser` 必须与 `voice_ime_with_cdp` 同为 `scope="module"`
- **页面加载等待**：必须 `wait_for_load_state("load")` + React hydration 等待
- **URL 匹配**：支持 Tauri v2 URL（`https://tauri.localhost/`）

---

## 七、tester-1 执行汇报模板

```markdown
测试执行报告：

| Step | 框架 | 执行情况 | 结果 | 说明 |
|------|------|---------|------|------|
| Step 1 | cargo test | ❌ SKIP | N/A | 本次仅改前端 |
| Step 2 | Vitest | ✅ 执行 | 8 PASS | React 组件逻辑 |
| Step 3 | pytest smoke | ❌ SKIP | N/A | 无需快速验证 |
| Step 4 | Playwright | ✅ 执行 | 6 PASS | UI 交互 |

覆盖范围：<说明本次覆盖的测试范围>
缺口：<无/列出缺口>
```

### 构建汇报模板

```
构建报告：
- Step 1: 清理旧进程 ✅
- Step 2: npm build (~600ms) + Tauri UI (~20s, X warnings)
- Step 3: 主程序 (~27s, X warnings)
- Step 4: cp src-tauri/target/release/feiyin-ime-ui.exe target/release/ ✅
- 产物时间戳验证: src-tauri/target/release/feiyin-ime-ui.exe == target/release/feiyin-ime-ui.exe ✅
- 产物验证: feiyin-ime.exe <时间>/<大小> + feiyin-ime-ui.exe <时间>/<大小>
- 冒烟测试: X/X PASS
```

---

## 八、测试质量规范【强制，2026-04-24 Gavin 确立】

> 来源：v0.5.2 验收失败复盘 — 测试全通过但功能全挂的根本原因分析。
> **以下规范适用于所有 tester-1 编写和执行的测试。**

### 规范 1：禁止用静态文件扫描代替功能测试

❌ **禁止**：`read_to_string(file).contains("某函数名")` 类型的断言

这种断言只验证"代码里写了什么字符串"，无法发现运行时 Bug（SQL 不匹配、状态未更新、UI 未响应）。

✅ **要求**：每个功能点必须有至少一个真正执行该功能的测试：
- Rust DB 函数：必须调用函数并断言返回值 / 数据库实际状态
- React 组件：必须渲染组件、触发交互、断言 DOM 变化

### 规范 2：前端必须有 Vitest 单元测试

项目已具备 Vitest + React Testing Library + `invoke` mock 基础设施（`ui/src/test/setup.ts`）。

✅ **要求**：每个有业务逻辑的 React 页面组件（如 Wordbook、Llm）必须有对应的 `.test.tsx` 文件，覆盖：
- 主流程（成功路径）：调用正确命令 + 正确参数
- 失败路径：invoke 抛出时，错误弹窗显示预期文字
- 状态更新：操作成功后 DOM 反映最新状态

### 规范 3：Rust 层必须有真实 DB 操作的单元测试

✅ **要求**：涉及 SQLite 操作的模块（`src/wordbook/db.rs` 等）必须在 `#[cfg(test)]` 块中用 `rusqlite::Connection::open_in_memory()` 写真实单元测试：
- 插入 → 按 id 删除 → 验证记录已消失
- 删除不存在的 id → 验证返回 0 rows affected
- 多条记录时，删除其中一条不影响其他

### 规范 4：静态分析断言绑定行为约定，不绑定实现字符串

❌ **禁止**：
- 断言具体函数名字符串（如 `"delete_wordbook_entry"`）——需求变更时产生误报
- 断言 CSS 像素值（如 `"width: 16px"`）——需求变更时产生误报
- 断言内部变量名（如 `"rawValue"`）

✅ **允许**：
- 断言命令名 pattern（如 `"delete_wordbook_entry"` + `_by_id` 的 or 条件）
- 断言行为语义（如 `"WHERE id ="` 而非具体变量名）
- CSS 断言应覆盖多个合法值范围（如 `<=16px` 而非 `==16px`）

### 规范 5：构建验证必须包含 target/release/ 路径检查

❌ **之前的失误**：tester 验证了 `src-tauri/target/release/feiyin-ime-ui.exe` 时间戳，但漏掉了把新 exe 复制到 `target/release/`，导致 Gavin 运行的是旧 binary。

✅ **强制要求**：每次 BUILD 任务完成时必须验证：
```bash
# Step 4 必须执行（cp 到 target/release/）
cp src-tauri/target/release/feiyin-ime-ui.exe target/release/feiyin-ime-ui.exe

# 强制验证：两处时间戳必须一致
ls -la src-tauri/target/release/feiyin-ime-ui.exe
ls -la target/release/feiyin-ime-ui.exe
# 时间戳不一致 = cp 未执行，必须重做
```

### 规范 6：关键功能必须有端到端验证

删除、添加等修改数据的操作，必须有完整的端到端测试流程：

✅ **最低要求**（Vitest 层）：
- 添加词条 → 在列表中出现（验证数据写入）
- 点击删除 → 列表中消失（验证数据删除 + UI 同步）

✅ **完整要求**（Playwright CDP 层，涉及真实 UI 的场景）：
- 打开 UI → 添加词条 → 刷新/重载 → 词条仍存在（验证持久化）
- 打开 UI → 点击删除 → 确认 → 词条消失（验证 Tauri IPC 层）
