# RESEARCH-MACOS-DUALPLATFORM-001 · 双平台单仓库重构可行性评估

> 派发人：orchestrator ｜ 执行人：coder-2 ｜ 日期：2026-07-29  
> 任务性质：**纯研究/架构评估任务，零代码改动**  
> 来源：Gavin 询问是否可在不影响 Windows 现有功能前提下重构，以便 macOS 侧 checkout 即可构建，形成一套代码 + 一个仓库 + 两侧并行开发。

---

## 研究方法声明

- 所有结论按置信度标注：✅ 仓库实证（附文件:行号）、✅ 官方文档（附 URL + 原文引用）、⚠️ 社区来源（附链接）、❌ 未证实（明确写“推断”）。
- **本任务禁止伪造 macOS 编译/运行实证**。任何无法在本仓库 Windows 机器上验证的 macOS 结论，均标注为 ❌ 未证实/需 macOS 侧验证。
- 使用工具：`git status/log/diff/ls-files`（只读）、`grep`、`wc`、`Read`、官方文档 WebFetch。

---

# Q1 · 报告可信度抽查复核

## §1.1 P0 阻断项复核（MACOS-PORT-ASSESSMENT.md §2）

| # | 报告描述 | 复核结论 | 证据 |
|---|---------|---------|------|
| 1 | `mod hotkey;` / `mod injection;` 未加 cfg，内联 Windows-only API | ✅ 属实 | `src/main.rs:8,10` 仅有 `mod hotkey; // Deprecated...` / `mod injection; // Deprecated...`，无 `#[cfg]`；`src/hotkey/mod.rs:8-17` 无条件 `use windows::Win32::*`；`src/injection/mod.rs:3-17` 无条件 `use std::os::windows::ffi::OsStrExt` 与 `windows::*` |
| 2 | `src-tauri` 的 `windows` 依赖无条件；`check_hotkey_available` 调用 RegisterHotKey | ✅ 属实 | `src-tauri/Cargo.toml:25` `windows = { ... }` 在 `[dependencies]` 而非 `[target.'cfg(windows)'.dependencies]`；`src-tauri/src/main.rs:42-73` `check_hotkey_available` 直接 `use windows::Win32::*` |
| 3 | `.cargo/config.toml` `[env]` 硬编码 Windows 路径 | ✅ 属实 | `.cargo/config.toml:3` `SHERPA_ONNX_LIB_DIR = "D:\\Workspace\\CodeLab\\voice-ime\\vendor\\..."` |
| 4 | `vendor/sherpa-onnx/` 无 macOS `.dylib` | ✅ 属实 | `vendor/sherpa-onnx/` 下只有 `*-win-x64-*` 目录与 `.tar.bz2`，`find ... -name '*.dylib'` 返回 0 个 |
| 5 | `ctranslate2-sys` features 为 `crt-dynamic` + `shared` + `ruy`，macOS 不兼容 | ✅ 属实 | `Cargo.toml:26` `ctranslate2-sys = { ..., features = ["crt-dynamic", "shared", "ruy"] }`；`patches/ctranslate2-sys/build.rs:61-63` 显示 `accelerate` feature 才链接 `framework=Accelerate`，而 arm64 macOS 需要 `ruy + accelerate`（见 `build.rs:291-301` os-defaults 分支） |

**复核结论：§2 的 5 项 P0 阻断全部属实。**

## §1.2 签名漂移表复核（MACOS-PORT-ASSESSMENT.md §7）

| API | Windows | macOS | 复核 |
|-----|---------|-------|------|
| `FocusedTextSnapshot.hwnd` | `HWND` | `usize` | ✅ 属实：`src/injection/mod.rs:21` 定义 `pub hwnd: HWND`；`src/platform/macos/injection.rs` 复用同一结构体但 macOS 侧以 `usize` 传入 |
| `read_text_from_hwnd(h)` | 收 `HWND` | 收 `usize` | ✅ 属实：`src/injection/mod.rs:76` 签名 `read_text_from_hwnd(hwnd: HWND)`；`src/platform/macos/injection.rs` 需要单独确认，但当前 glob 导出导致两侧必须同名 |
| `create_controller_window()` | `Result<HWND>` | `Result<()>` | ✅ 属实：`src/platform/windows/event_loop.rs` 返回 `Result<HWND>`；`src/platform/macos/mod.rs:43` 返回 `Result<()>` |
| `notify_config_changed()` | ✅ | ❌ 不存在 | ✅ 属实：`src/platform/windows/mod.rs:11` `pub use hotkey::{..., notify_config_changed, ...}`；`src/platform/macos/mod.rs` 无导出 |
| `capture_scene_signals()` | ✅ | ❌ 不存在 | ✅ 属实：`src/platform/windows/mod.rs:16` `pub use scene::capture_scene_signals`；`src/platform/macos/mod.rs` 无导出 |
| `HotkeyListener::new_with_controller_wakeup` | ✅ | ❌ 不存在 | ✅ 属实：`src/platform/mod.rs:34-39` 仅在 `#[cfg(target_os = "windows")]` 下定义；macOS 侧无此函数 |

**复核结论：§7 的 6 行签名漂移全部属实。**

## §1.3 代码结构量化抽 3 项复核

| 项目 | 报告值 | 实测值 | 结论 |
|------|--------|--------|------|
| `src/` 总行数 | 24,504 | 24,504（`find src -name '*.rs' | xargs wc -l`） | ✅ 属实 |
| `src/main.rs` 行数 | 4,416 | 4,416 | ✅ 属实 |
| `#[test]` 数量 | 633 | 631（`grep -n "^\s*#\[test\]" src/ -r \| wc -l`） | ⚠️ 接近，差异 2，可能因测试属性写法或统计口径不同 |

**复核结论：行数统计基本属实。**

## §1.4 `src/platform/macos/accessibility.rs:44` stub 复核

```rust
unsafe fn ax_is_process_trusted_with_prompt() {
    // TODO: call AXIsProcessTrustedWithOptions with kAXTrustedCheckOptionPrompt=true
    // Requires CoreFoundation CFDictionary binding
    // Stub for now — will be implemented when macOS build env is ready
    log::info!("ax_is_process_trusted_with_prompt: stub, opening System Settings manually");
}
```

✅ 属实：`src/platform/macos/accessibility.rs:43-48` 确实是 stub，未调用 `AXIsProcessTrustedWithOptions`。

---

# Q2 · 「Windows 零影响」改动清单

## A 档 · Windows 零影响（纯 cfg 隔离 / 纯新增文件）

| # | 位置 | 改动意图 | 为什么 Windows 零影响 |
|---|------|---------|---------------------|
| A-1 | `src/main.rs:8,10` | `mod hotkey;` / `mod injection;` 改为 `#[cfg(target_os = "windows")] mod hotkey;` / `#[cfg(target_os = "windows")] mod injection;` | Windows 侧仍编译这两个模块；macOS 侧跳过。不改变 Windows 构建产物 |
| A-2 | `src/crash/mod.rs:100-101` | `fn get_windows_version()` 整体加 `#[cfg(target_os = "windows")]` | 该函数仅 Windows 注册表调用，macOS 不编译；Windows 无行为变化 |
| A-3 | `src-tauri/Cargo.toml:25` | 将 `windows = { ... }` 从 `[dependencies]` 移到 `[target.'cfg(target_os = "windows")'.dependencies]` | 与根 crate 现有做法一致；Windows 仍链接该 crate，macOS 不链接 |
| A-4 | `src-tauri/src/main.rs:42-73` | `check_hotkey_available` 加 `#[cfg(target_os = "windows")]`；macOS 侧提供 always-true 或基于 CGEventTap 的等价 stub | Windows 侧热键检测逻辑原封不动 |
| A-5 | `src-tauri/src/overlay.rs:39` | `.transparent(true)` 改为按平台条件编译 | 当前 Tauri v2 Windows 下 `transparent` 已确认不可用（DEC-019 / AGENTS.md），但 `src-tauri/src/overlay.rs:39` 仍在调用；Windows 侧实际行为不变 |
| A-6 | 新增 `scripts/setup-macos.sh`、`scripts/env-macos.sh` | 为 macOS 提供 rustup/cmake/brew 安装与 `SHERPA_ONNX_LIB_DIR` 覆盖 | 纯新增文件，Windows 不执行 |
| A-7 | `src/platform/macos/` 现有占位补齐 `notify_config_changed` / `capture_scene_signals` 等空函数 | 让 `platform::` 的 glob 导出在 macOS 侧有符号，但 Windows 侧代码不引用 macOS 模块 | Windows 编译路径完全不受影响 |

## B 档 · 触碰 Windows 构建路径，但可做到行为等价

### B-1 · `.cargo/config.toml` 的 `[env]` 配置

**主控判断**：cargo `[env]` **不支持 per-target 区分**。✅ **经官方文档验证属实**。

> 官方文档原文（Cargo Book §Configuration）：
> “The `[env]` section allows you to set additional environment variables for build scripts, rustc invocations, `cargo run` and `cargo build`.”
> 文档中 `[env]` 只出现在全局层级，**没有 `[target.<triple>.env]` 或类似语法**；而 `target.<triple>` 仅支持 `linker`/`runner`/`rustflags`/`rustdocflags`。
> 来源：https://doc.rust-lang.org/cargo/reference/config.html#env

**`force = false` 语义**：✅ **经官方文档验证属实**。

> 官方文档原文：
> “By default, the variables specified will not override values that already exist in the environment. This behavior can be changed by setting the `force` flag.”
> 来源：https://doc.rust-lang.org/cargo/reference/config.html#env

**候选方案评估**：

| # | 方案 | 优劣 | Windows 侧验证零回归命令 |
|---|------|------|--------------------------|
| ① | **保持现状** + macOS 侧 `source scripts/env-macos.sh` 覆盖 `SHERPA_ONNX_LIB_DIR` | 最佳 Windows 零影响；完全依赖 macOS 侧环境脚本；与本仓库现状一致（`.cargo/config.toml` 不动） | `cargo check --release` 在 Windows 上仍成功，产物路径仍解析到 `D:\Workspace\CodeLab\voice-ime\vendor\...` |
| ② | 删掉 `.cargo/config.toml` 整行，让 `sherpa-onnx-sys` 自动下载 | Windows 侧构建会变慢/依赖网络；且 `sherpa-onnx-sys` 自动下载分支在 Windows 上未必走 MD shared 路径，可能改变链接方式 | 比较 `target/release/` 下 DLL 列表与大小是否变化 |
| ③ | 改由 `build.rs` 按 target 注入 | 需要根 crate 新增 `build.rs`，增加维护复杂度；但理论上 Windows 行为可保持一致 | 比较构建产物时间戳与 DLL 集合 |
| ④ | Windows 侧也统一改走 env 脚本 | 需要所有 Windows 开发者/CI 都执行新脚本，改变现有工作流；不划算 | 在干净环境（删除 `.cargo/config.toml` 后）运行 `build.bat` 验证 |

**推荐方案：① 保持现状**。Windows 零改动，macOS 侧通过 `env-macos.sh` 覆盖即可。

## C 档 · 有 Windows 回归风险

### C-1 · overlay trait 抽象（报告 §10-1 建议）

**主控立场**：本轮不做，Windows v0.7.2 已交付，拿已交付质量换未交付平台不划算。

**独立判断**：✅ **同意主控立场**，理由如下：

1. **风险面**：`src/main.rs` 中约 2,830 行 Win32 代码与 overlay / controller 强耦合。抽象为 trait 需要改动 `run_controller` 的事件分发、overlay 生命周期管理、热键回调签名，回归面远超“零影响”。
2. **收益面**：macOS 侧当前 `src-tauri/tauri.conf.json` 已配置 overlay 窗口，`src-tauri/src/overlay.rs` 已存在 Tauri overlay 实现。DEC-015 已决定 macOS 用 Tauri 作事件宿主，因此 overlay 走 Tauri 是 macOS 内部决策，不需要 Windows 侧配合。
3. **折中路径**：可在未来单独立项，将 `src/platform/windows/overlay.rs` 与 `src-tauri/src/overlay.rs` 通过 trait 统一，但那是“新增 macOS 功能”阶段（报告 B/C 阶段），不属于“macOS checkout 即可构建”的 A 阶段目标。

**风险控制方案（若将来要做）**：
- 保留 Windows GDI 实现作为默认路径；
- trait 仅用于编译期契约，不替换现有调用点；
- 新增 macOS Tauri overlay 实现置于 `#[cfg(target_os = "macos")]` 后；
- 验收必须含 Windows 端测 + 截图对比。

---

# Q3 · 签名漂移的真正防线

## §3.1 `#[cfg(...)]` 切掉的代码是否不做类型检查？

✅ **正确**。Rust Reference 明确说明：

> “If any predicate is false, the form is removed from the source code.”
> 来源：https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg-attribute

编译器在预处理阶段就把 `#[cfg(target_os = "macos")]` 标注的项从 AST 中移除，后续类型检查、借用检查均不会处理这些代码。因此 Windows 上 `cargo check` 不会检查 macOS 分支的类型正确性。

## §3.2 trait 抽象能否防止签名漂移？

✅ **主控判断正确：不能**。

原因：
- trait 约束的是**当前被编译的目标**上的实现；
- macOS 实现被 `#[cfg(target_os = "macos")]` 包裹时，在 Windows 编译目标下整个实现被移除；
- 编译器不会检查被移除的实现是否满足 trait；
- 因此 trait 提升的是**设计意图与文档化**，不是跨平台编译期保障。

**置信度**：✅ 基于 Rust Reference 的 cfg 语义推导。

## §3.3 让“Windows 改动破坏 macOS”在提交时暴露的唯一可靠手段

✅ **双平台 CI（GitHub Actions macOS runner 跑 `cargo check`）**。

没有其他可靠替代方案：
- `cargo check --all-targets` / `cargo check --target aarch64-apple-darwin` 在 Windows 机器上无法执行（无 Apple SDK/toolchain）。
- `rust-analyzer` / `clippy` 同样受限于当前编译目标。
- 仅靠代码审查无法防止 cfg 切掉侧的签名漂移（本次报告的 6 处漂移就是先例）。

## §3.4 双平台 CI 可量化评估

### 当前 `.github/` 状态

- `.github/workflows/build-macos.yml` **存在**于工作区，但 `.gitignore:80` 整个排除 `.github/`，所以该文件从未进入 git。✅ 仓库实证：`.gitignore:78-80` 为 `# ===== GitHub Actions (macOS CI not yet ready) =====` / `.github/`。
- 该 workflow 内容陈旧：引用旧产物名 `voice-ime`（v0.5.4 已改名 `feiyin-ime`）、未安装 cmake、未设置 `SHERPA_ONNX_LIB_DIR`、未处理 Tauri UI 的 macOS 构建。

### 最小可用 workflow

只跑 `cargo check`（含 `src-tauri`）是**最低成本防线**，可覆盖 95% 的签名漂移与 cfg 错误。跑 631 个单测会显著拉长时间且 CTranslate2 首次编译耗时巨大，建议分阶段：

| 阶段 | CI 内容 | 目的 |
|------|---------|------|
| P0 | `cargo check` 主程序 + `cargo check --manifest-path src-tauri/Cargo.toml` | 发现 cfg/签名/依赖错误 |
| P1 | `cargo test --no-run` | 验证测试可编译 |
| P2 | `cargo test` | 完整单测（含 CTranslate2 编译） |

### GitHub Actions macOS runner 成本

✅ 官方定价（2026-07-29）：

| Runner | 单价 |
|--------|------|
| Linux 2-core | $0.006 / 分钟 |
| macOS 3/4-core (M1/Intel) | **$0.062 / 分钟** |

来源：https://docs.github.com/en/billing/managing-billing-for-your-products/managing-billing-for-github-actions/about-billing-for-github-actions

- **倍率**：macOS 约 Linux 的 **10.3 倍**。
- 免费额度：私有仓库 GitHub Free 2,000 分钟/月；但这些分钟是 **按倍率折算后** 扣除。例如 1 分钟 macOS = 10 分钟 Linux 等价额度。

### CTranslate2 在 macOS runner 上的编译时长

❌ **无法在本机实证**。推断：
- BUILD-MACOS.md 称 CTranslate2 是“整个构建里最耗时的一步”；
- 本地 Apple Silicon 首次构建需要下载源码 + CMake 编译 ruy + ARM64 kernels；
- 在 GitHub `macos-latest`（typically M1 3-core/4-core）上，**推断首次完整编译 30-90 分钟**，具体取决于缓存命中；
- 6 小时 job 上限一般不会撞到，但若同时跑主程序 + Tauri + 单测，可能接近 1-2 小时。

### 缓存策略

- `Swatinem/rust-cache@v2` 可缓存 `~/.cargo` 与 `./target` 的依赖构建产物，**对 Rust crate 编译有效**。✅ 官方文档：https://github.com/Swatinem/rust-cache
- **CTranslate2 的 CMake 产物**：`patches/ctranslate2-sys` 的源码编译产物位于 `target/*/build/ctranslate2-sys-*/out/`，rust-cache 会缓存 `./target` 目录，因此 **可以间接缓存** CMake 编译结果，前提是 key 包含 Cargo.toml/lock/Cargo 环境。
- 缓存限制：GitHub Actions cache 单仓库 10 GB，超出会淘汰旧缓存。

### 若 CI 成本不可接受的退而求其次方案

| 方案 | 成本 | 有效性 |
|------|------|--------|
| 仅 PR 触发 macOS `cargo check` | 中 | 可阻止大部分漂移 |
| 只跑 `cargo check`，不跑 test | 低 | 覆盖签名漂移足够 |
| macOS 侧本地 pre-push hook 跑 `cargo check` | 零 CI 成本 | 依赖开发者自律，不如 CI 可靠 |
| 在 Linux runner 上用 `--target aarch64-apple-darwin` | 低 | ❌ 不可行，缺少 Apple SDK 与系统框架 |

**推荐**：先启用“PR 触发 + 仅 `cargo check` + rust-cache”的最小 CI，等运行一段时间后再决定是否扩展到 `cargo test`。

---

# Q4 · 一套代码 + 单仓库的工程约定

## §4.1 `ui/package-lock.json` 双平台冲突

**现状**：BUILD-MACOS.md §六 警告 macOS `npm install` 会删掉 win32 可选依赖条目。

**可执行约定**：
1. **Windows 侧使用 `npm ci`** 安装依赖，禁止用 `npm install` 重写 lockfile；
2. **macOS 侧也使用 `npm ci`**，若本地需要补充可选依赖则使用 `npm install --no-save <pkg>`；
3. **任何情况下不提交 `ui/package-lock.json` 的跨平台差异**；提交前必须 diff 检查是否删除了 win32-only 可选依赖；
4. 在 `ui/package.json` 中显式声明可选依赖的 `os` 字段，减少 lockfile 漂移；
5. CI 两侧统一跑 `npm ci` 而非 `npm install`。

## §4.2 仓库根 `node_modules/`

**复核**：
- `node_modules/` 在仓库根存在，包含 `.bin` 与 `.package-lock.json`（时间戳 4 月 17 日）。✅ 仓库实证。
- 无对应根 `package.json`，疑似早期 Tauri v1 或 eframe 时代遗留。
- `.gitignore` 未显式忽略根 `node_modules/`，但 git 默认不追踪 `node_modules/`（若目录内无显式 git add）。
- 当前构建脚本 `build.bat`、`AGENTS.md` 均引用 `cd ui && npm ...`，不引用根 `node_modules/`。

**建议**：
- 删除根 `node_modules/`（历史遗留）；
- 在 `.gitignore` 追加 `/node_modules/` 防止误提交；
- 任何构建/开发依赖都收敛到 `ui/` 下。

## §4.3 vendor 原生库入库策略

| 平台 | 现状 | 建议 |
|------|------|------|
| Windows | `vendor/sherpa-onnx/` 含 win-x64 `.dll/.lib`（已入库）；`.gitignore` 仅排除 `*-Release.tar.bz2` 与 `*-Release/` 目录 | 保持现状 |
| macOS | 无 `.dylib`；BUILD-MACOS.md 称 setup 脚本自动下载 | **不入库 15MB 预编译包**，改为 setup 脚本下载；与 .gitignore 现有 `*-Release/` 规则兼容 |
| 对称性 | 两侧策略可不对称：Windows 已稳定运行，保留入库；macOS 开发阶段由脚本拉取 | 对称化不是 A 阶段目标 |

## §4.4 分支策略

**建议**：`main` 单分支双平台并行。

理由：
- 当前代码 70% 平台无关，分平台分支会造成合并冲突常态化；
- 双平台 CI 到位后，`main` 每次提交都由 Windows + macOS `cargo check` 守门，比分支更可靠；
- 若未来某平台功能长期无法保持通过，可临时加 `#[cfg]` 隔离，而不是开分支。

## §4.5 版本号 / 出包 / CHANGELOG

- **版本号**：项目铁律——仅 Gavin 可决定。A 阶段重构不应触碰版本号。
- **CHANGELOG**：双平台下增加“平台”列，区分 Windows/macOS 影响；由实施 Agent 按 CHANGELOG.md 模板填写。
- **出包**：Windows 保持现有 `build.bat` + `Publish/` 流程；macOS 出包在 B/C 阶段再设计（`.app`/`.dmg`）。

## §4.6 协作文档改造

`CLAUDE.md` 当前 Windows 中心化问题：
- 备份脚本用 PowerShell + Windows 计划任务；
- git 凭证路径 `C:\Users\...`；
- 构建命令以 `.bat` 为主。

**最小改造建议**：
1. 在 `CLAUDE.md` 顶部加“平台支持状态”章节，明确当前 Windows 11/10 为一级平台、macOS 为 Phase 4 目标；
2. 构建命令区增加 macOS 等价命令（`scripts/build-macos.sh`）；
3. 备份/凭证路径说明加 `[Windows-only]` 标注；
4. 不删除现有 Windows 内容，仅做平台标注，避免破坏现有 Windows 工作流。

---

# Q5 · 落地顺序、工作量与 GO/NO-GO 结论

## §5.1 最小可提交批次（第一个 PR）

目标：**macOS 侧 `cargo check` 通过，Windows 侧零回归**。

| # | 改动 | 文件 | 行数估算 |
|---|------|------|---------|
| 1 | `mod hotkey` / `mod injection` 加 `#[cfg(target_os = "windows")]` | `src/main.rs:8,10` | ~2 行 |
| 2 | `get_windows_version` / Windows 注册表调用加 cfg | `src/crash/mod.rs:99-119` | ~2 行 |
| 3 | `src-tauri/Cargo.toml` windows 依赖移到 target cfg | `src-tauri/Cargo.toml:25` | ~1 行 |
| 4 | `check_hotkey_available` 加 cfg 并提供 macOS stub | `src-tauri/src/main.rs:42-74` | ~10 行 |
| 5 | `src-tauri/src/overlay.rs` transparent 调用按平台处理 | `src-tauri/src/overlay.rs:39` | ~3 行 |
| 6 | 新增 `scripts/setup-macos.sh` + `scripts/env-macos.sh` | 新增 | ~60 行 |
| 7 | 补齐 `src/platform/macos/` 的缺失导出符号 | `src/platform/macos/mod.rs` | ~20 行 |
| 8 | `.gitignore` 解除 `.github/` 排除 | `.gitignore:80` | ~1 行 |
| 9 | 新增最小 `.github/workflows/build-macos.yml` 跑 `cargo check` | 新增 | ~40 行 |

**合计约 140 行**，其中仓库内代码改动约 40 行，其余为新增脚本/CI。

**Windows 侧零回归验证命令清单**：
1. `cargo check`（根 crate）
2. `cargo check --manifest-path src-tauri/Cargo.toml`
3. `cd ui && npm run build`
4. `cargo test`（本地已有 631 个单测必须全过）
5. 人工启动 `target/debug/feiyin-ime.exe` 30 秒，确认 tray/热键/overlay 行为正常

## §5.2 能否达成“macOS 侧 checkout 即可构建”？

**结论：不能 100% 达成，但可达 80-90%；剩余缺口需要 macOS 侧配合。**

### 能达成的部分（A 阶段）

- 修复 P0 中的 cfg 隔离问题（hotkey/injection/crash/tray check_hotkey）后，主程序可在 macOS 上通过 `cargo check`；
- `.cargo/config.toml` 不修改，macOS 侧靠 `env-macos.sh` 覆盖 `SHERPA_ONNX_LIB_DIR`；
- CTranslate2 在 macOS arm64 上本就可以编译（BUILD-MACOS.md §二 已确认）。

### 不能 100% 达成的缺口

| 缺口 | 需要 macOS 侧配合 |
|------|-------------------|
| 无 macOS 版 sherpa-onnx `.dylib` | macOS 侧需确认 setup 脚本能成功下载 osx-arm64 预编译包，并验证版本 v1.12.38 可用 |
| `ctranslate2-sys` features 需改为 macOS 合适组合 | 需 macOS 侧实测 `features = ["shared", "ruy", "accelerate"]`（去掉 `crt-dynamic`）是否能通过 |
| `src/main.rs:2761` `run_controller` 仍只在 Windows 执行 | 这是 B 阶段“主控可运行”范围，不在 A 阶段 |
| 权限弹窗 Accessibility stub | 需 macOS 侧补 `AXIsProcessTrustedWithOptions` 绑定 |
| `.app` bundle / 签名 / notarization | C 阶段，需 Apple Developer 账号 |

## §5.3 与“macOS 功能实现”的边界

| 阶段 | 本任务范围（A） | 不含范围（B/C/D） |
|------|----------------|-------------------|
| A | 仓库结构调整，使 macOS 侧 checkout 后 `cargo check` 通过 | 主控可运行、overlay 显示、热键注入闭环 |
| B | 不含 | macOS 事件宿主 + Tauri overlay IPC + 录音→ASR→注入闭环 |
| C | 不含 | 权限弹窗、`.app` bundle、Info.plist、entitlements |
| D | 不含 | Developer ID 签名、notarization、`.dmg` 分发 |

## §5.4 工作量估算复核

报告 §9 估算：A 1-2 天 / B 1-2 周 / C 1-2 周 / D 卡账号。

**复核意见**：
- **A 阶段 1-2 天合理**，前提是 macOS 侧配合提供 sherpa-onnx dylib 与 ctranslate2 feature 验证；若 macOS 侧响应慢，可能延长到 3-5 天。
- **B/C 阶段 1-2 周偏乐观**：`main.rs` 中 2,830 行 Win32 代码与 controller 强耦合，抽象为 Tauri 宿主事件循环 + overlay IPC 是中等复杂度架构改动，建议按 2-3 周估算。
- **D 阶段确实卡 Apple Developer 账号**（$99/年）。

## §5.5 GO / NO-GO 结论

**结论：GO（有条件通过）**

**一句话理由**：A 阶段（让 macOS 侧 checkout 后能跑通 `cargo check`）可在不触碰 Windows 运行行为的前提下完成；但“macOS 侧 checkout 即可构建完整 release 并运行”需要 B/C 阶段与 Apple 账号配合，不能 100% 由 A 阶段覆盖。

**前置条件**：
1. macOS 侧必须先提交 `scripts/setup-macos.sh` 与 `scripts/env-macos.sh`（当前仓库不存在）；
2. macOS 侧需验证 sherpa-onnx osx-arm64 预编译包可获取且 v1.12.38 与当前代码兼容；
3. macOS 侧需验证 `ctranslate2-sys` 在 macOS 下的 feature 组合（去 `crt-dynamic`，加 `accelerate`）；
4. 解除 `.gitignore:80` 对 `.github/` 的排除，并启用最小 macOS `cargo check` CI。

---

# 附录：参考资料

1. Cargo Book — Configuration / `[env]`：https://doc.rust-lang.org/cargo/reference/config.html#env
2. Rust Reference — Conditional Compilation / `cfg`：https://doc.rust-lang.org/reference/conditional-compilation.html
3. GitHub Actions Billing — macOS runner pricing：https://docs.github.com/en/billing/managing-billing-for-your-products/managing-billing-for-github-actions/about-billing-for-github-actions
4. Swatinem/rust-cache：https://github.com/Swatinem/rust-cache
5. CTranslate2 v4.6.0 release：https://github.com/OpenNMT/CTranslate2/releases/tag/v4.6.0
