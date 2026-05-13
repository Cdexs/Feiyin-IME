# 构建依赖清单 · Feiyin Voice Input

> 目标：按照本文档安装所有依赖后，执行 `build.bat` 即可完成完整构建。
> 平台：Windows 10 / Windows 11（64位）

---

## 一、必须安装的软件

### 1. Rust 工具链

| 项目 | 要求 |
|------|------|
| 工具链 | `stable-x86_64-pc-windows-msvc`（**必须是 MSVC 版本**，不能用 GNU）|
| 最低版本 | Rust 1.75+ |

**安装步骤：**
```powershell
# 下载并运行 rustup-init.exe（官网或国内镜像）
# https://www.rust-lang.org/tools/install
# 安装时选择：x86_64-pc-windows-msvc（默认）
rustup-init.exe

# 验证安装
rustc --version       # 应显示 rustc 1.75.0+
cargo --version       # 应显示 cargo 1.75.0+
```

**国内镜像（加速下载）：**
```powershell
$env:RUSTUP_DIST_SERVER = "https://mirrors.ustc.edu.cn/rust-static"
$env:RUSTUP_UPDATE_ROOT = "https://mirrors.ustc.edu.cn/rust-static/rustup"
```

---

### 2. Visual Studio Build Tools 2022

提供 C++ 编译器（cl.exe）、链接器和 Windows SDK。**必须安装，缺少将无法编译 C++ 依赖。**

| 项目 | 要求 |
|------|------|
| 安装包 | Visual Studio 2022 Build Tools |
| 工作负载 | ✅ **使用 C++ 的桌面开发** |
| 包含组件（自动勾选）| MSVC v143 编译器、Windows 11 SDK、CMake |

**安装步骤：**
1. 下载 [Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
2. 运行安装程序 → 选择 **"使用 C++ 的桌面开发"** 工作负载
3. 确认包含以下组件：
   - MSVC v143 - VS 2022 C++ x64/x86 生成工具
   - Windows 11 SDK（10.0.22621.0 或更高）
   - 适用于 Windows 的 C++ CMake 工具

**验证安装：**
```powershell
# 在 Developer Command Prompt 中执行
cl.exe /?          # 应显示 Microsoft C/C++ 编译器版本
cmake --version    # 应显示 cmake version 3.x
```

---

### 3. Node.js（前端构建）

| 项目 | 要求 |
|------|------|
| 版本 | Node.js **18.0+**（推荐 LTS 版本）|
| 包管理器 | npm（随 Node.js 自动安装）|

**安装步骤：**
```powershell
# 下载 LTS 版本：https://nodejs.org/
# 安装完成后验证：
node --version    # 应显示 v18.x 或更高
npm --version     # 应显示 9.x 或更高

# 安装前端依赖（首次克隆后执行一次）
cd voice-ime
npm install
```

---

## 二、环境变量配置

### sherpa-onnx 预编译库路径（构建核心 ASR 依赖）

项目 vendor 目录已包含预编译的 sherpa-onnx 库，**无需单独下载**，但需要配置路径。

**方法 A：修改 .cargo/config.toml（推荐，自动生效）**

编辑项目根目录的 `.cargo/config.toml`，将路径改为你的实际项目路径：

```toml
[env]
SHERPA_ONNX_LIB_DIR = "C:\\your\\path\\to\\voice-ime\\vendor\\sherpa-onnx\\sherpa-onnx-v1.12.38-win-x64-shared-MD-Release\\lib"
```

**方法 B：每次构建前手动设置（build.bat 已内置）**

build.bat 会自动设置此变量，若直接用 `cargo build` 需手动执行：
```bat
set SHERPA_ONNX_LIB_DIR=<项目路径>\vendor\sherpa-onnx\sherpa-onnx-v1.12.38-win-x64-shared-MD-Release\lib
```

### CMake 路径（若 cmake 不在 PATH）

```powershell
$env:CMAKE = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
```

---

## 三、初始化开发环境（首次克隆后执行一次）

```powershell
# 1. 安装前端依赖
npm install

# 2. 初始化 Publish/ 和 target/release/ 目录（建立模型链接、复制 DLL 和配置）
PowerShell -ExecutionPolicy Bypass -File scripts\init-publish.ps1

# 3. 验证构建环境
cargo check
```

---

## 四、执行构建

### 主程序完整构建（使用 build.bat）

```bat
build.bat
```

build.bat 自动完成：
1. 设置 SHERPA_ONNX_LIB_DIR 环境变量
2. `cargo build --release`（编译 Rust 主程序）
3. 复制 sherpa-onnx DLL 到 target/release/
4. 同步 EXE 到 Publish/ 目录

### Tauri 设置界面构建（有 UI 改动时需要）

```powershell
# Step 1: 前端构建
cd ui
npm run build
cd ..

# Step 2: Tauri UI Release 构建
cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol

# Step 3: 复制产物（如需要）
copy src-tauri\target\release\voice-ime-ui.exe target\release\voice-ime-ui.exe
```

### 快速验证（不出包）

```powershell
cargo check                                                      # 检查主程序
cargo check --manifest-path src-tauri/Cargo.toml               # 检查 Tauri UI
cargo test                                                       # 运行单元测试
```

---

## 五、构建产物位置

| 产物 | 路径 |
|------|------|
| 主程序 | `target\release\voice-ime.exe` |
| 设置界面 | `target\release\voice-ime-ui.exe` |
| 崩溃报告 | `target\release\crash-reporter.exe` |
| 运行时 DLL | `target\release\*.dll` |
| 发布暂存 | `Publish\`（build.bat 自动同步 EXE）|

---

## 六、常见问题

| 错误 | 原因 | 解决方法 |
|------|------|----------|
| `cannot find -lctranslate2` | sherpa-onnx 库路径未配置 | 检查 `.cargo/config.toml` 中的 `SHERPA_ONNX_LIB_DIR` 路径是否正确 |
| `cl.exe` not found | MSVC 未安装或环境未激活 | 安装 VS Build Tools，或在 Developer Command Prompt 中构建 |
| `cmake` not found | CMake 未在 PATH | 设置 `CMAKE` 环境变量指向 VS BuildTools 中的 cmake.exe |
| `prefix 'xxx' is unknown` | UTF-8 源文件编码损坏 | 不要用 PowerShell Set-Content 修改 .rs 文件，改用 WSL Python |
| `拒绝访问 (os error 5)` | exe 正在运行中 | 先关闭 voice-ime.exe，再重新构建 |
| Tauri UI 显示空白页 | 缺少 `custom-protocol` feature | 确认 Tauri 构建命令包含 `--features custom-protocol` |
| `STATUS_DLL_NOT_FOUND` | 缺少运行时 DLL | 运行 `build.bat` 自动复制 DLL，或手动复制 vendor 中的 DLL |

---

## 七、依赖版本参考

| 依赖 | 版本 | 用途 |
|------|------|------|
| Rust | 1.75+ | 主要编译器 |
| MSVC / VS Build Tools 2022 | v143 | C++ 编译 ctranslate2-sys / esaxx-rs |
| Windows SDK | 10.0.22621+ | Win32 API |
| CMake | 3.20+ | C++ 依赖构建 |
| Node.js | 18.0+ | 前端 React 构建 |
| npm | 9.0+ | 前端包管理 |
| sherpa-onnx（预编译）| 1.12.38 | 语音识别 DLL（vendor 目录已包含）|

---

## 八、CI / 自动化构建

项目包含 `.github/workflows/build-macos.yml`，用于 GitHub Actions 自动化 macOS 构建（需要 `workflow` 权限推送）。

Windows CI 配置待补充（可基于上述步骤编写 `.github/workflows/build-windows.yml`）。
