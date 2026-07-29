# macOS 构建环境 · Feiyin Voice Input

> 对应 Windows 版 `BUILD-DEPS.md`。本文档记录 macOS（Apple Silicon）上的依赖安装、环境变量与当前可编译状态。
> 验证环境：macOS 15.7.8 / arm64 / 2026-07-29。

---

## 一、一键初始化

```bash
bash scripts/setup-macos.sh      # 幂等，可重复执行
source scripts/env-macos.sh      # 每个新 shell 都要执行
```

之后：

```bash
cargo check                                       # 主程序
cargo check --manifest-path src-tauri/Cargo.toml  # Tauri 后端
cd ui && npm run build                            # 前端
bash scripts/build-macos.sh                       # 完整 release 构建
```

---

## 二、依赖清单

| 依赖 | 要求 | 本机实测版本 | 安装方式 |
|------|------|--------------|----------|
| Xcode Command Line Tools | 提供 clang / ld / 系统头文件 | Apple clang 17.0.0 | `xcode-select --install`（或完整 Xcode）|
| Rust | 1.75+，target `aarch64-apple-darwin` | 1.97.1 stable | `curl -sSf https://sh.rustup.rs \| sh` |
| CMake | 3.20+ | 4.4.1 | `brew install cmake` |
| pkg-config | — | pkgconf 3.0.4 | `brew install pkg-config` |
| Node.js | 18+ | v24.18.0 / npm 11.16.0 | `brew install node` |
| sherpa-onnx 预编译动态库 | v1.12.38 **osx-arm64-shared** | 已下载到 vendor/ | setup 脚本自动拉取 |

**没有 MSVC 对应物**：Windows 侧的 VS Build Tools 在 macOS 上由 Xcode CLT 的 clang 替代，无需额外安装。

### 自动获取、无需手工处理的依赖

- **CTranslate2 4.6.0**：`patches/ctranslate2-sys` 的 build.rs 首次构建时把源码拉到
  `target/<profile>/CTranslate2-4.6.0/`，再用 CMake 从源码编译（arm64 走 ruy + NEON kernels）。
  **这是整个构建里最耗时的一步，也是必须装 CMake 的唯一原因**；首次构建需联网。
- **sentencepiece / esaxx-rs / rusqlite(bundled)**：由 cc/cmake 从源码本地编译，已验证在 arm64 上通过。
- **onnxruntime**：随 sherpa-onnx 预编译包一起提供（`libonnxruntime.1.24.4.dylib`）。

---

## 三、环境变量：为什么必须 `source scripts/env-macos.sh`

`.cargo/config.toml` 的 `[env]` 段把 `SHERPA_ONNX_LIB_DIR` 写死为 Windows 路径：

```toml
SHERPA_ONNX_LIB_DIR = "D:\\Workspace\\CodeLab\\voice-ime\\vendor\\sherpa-onnx\\...\\lib"
```

在 macOS 上直接 `cargo check` 会硬失败：

```
thread 'main' panicked at sherpa-onnx-sys-1.12.38/build.rs:40:9:
SHERPA_ONNX_LIB_DIR does not exist or is not a directory: D:\Workspace\...
```

cargo 的 `[env]` 默认 `force = false` —— **shell 中已导出的同名变量优先**，所以 `env-macos.sh`
只需 export 覆盖，不必修改 `config.toml`（改了会破坏 Windows 侧构建）。

> 备注：`sherpa-onnx-sys` 本身有按 target 自动下载预编译包的能力（不设该变量时生效），
> 但只要 config.toml 里那行还在，就永远走不到自动下载分支。

---

## 四、当前可编译状态（2026-07-29 实测）

| 层 | 状态 | 说明 |
|----|------|------|
| 依赖编译（324 个 crate，含全部 C/C++ 依赖）| ✅ 通过 | CTranslate2（CMake 源码编译）/ sherpa-onnx / sentencepiece / rusqlite / eframe / tray-icon 均在 arm64 上编译成功 |
| 前端 `ui`（tsc + vite）| ✅ 通过 | 48 modules，产物 ~202 KB |
| Tauri 后端 `src-tauri` | ❌ 3 个源码错误 | 见下表 |
| 主程序 `feiyin-ime` | ❌ 16 个源码错误 | 见下表 |
| crash-reporter | ❌ 1 个源码错误 | 见下表 |

**构建环境本身已不是瓶颈**，剩余全部是源码未完成 macOS 适配（Phase 4）。

### 剩余阻塞项

| 文件 | 错误 | 性质 |
|------|------|------|
| `src/hotkey/mod.rs:8-14` | 4× `unresolved crate windows` | Windows 专用模块未做 `#[cfg(windows)]` 隔离 |
| `src/injection/mod.rs:3-14,113` | 6× `unresolved crate windows` + `encode_wide` | 同上 |
| `src/crash/mod.rs:100-101` | 2× `unresolved crate windows` | 同上 |
| `src/platform/macos/hotkey.rs:124` | `CGEventType` 不支持 `==` | core-graphics 0.25 API 变更，需改用 `matches!` / 转 u32 比较 |
| `src/platform/macos/hotkey.rs:257` | `Result` 无 `ok_or_else` | 应为 `map_err` 或先 `.ok()` |
| `src/crash/reporter.rs:369` | `FontData::from_bytes` 不存在 | egui 0.29 API，需改 `FontData::from_owned/from_static` |
| `src-tauri/src/main.rs:47-48` | `windows::Win32` 找不到 | `windows` 依赖在 src-tauri/Cargo.toml 里**未按 target 隔离**，crate 在 macOS 上编成空壳，用到 Win32 就报错 |
| `src-tauri/src/overlay.rs:39` | `WebviewWindowBuilder::transparent` 不存在 | 需要 tauri 的 `macos-private-api` feature |

> 注意：即便这些错误全部修完，`src/main.rs` 的 `fn main()` 在非 Windows 分支仍然只打一行 warn 就返回
> —— 能编译 ≠ 能运行，参见 Phase 4 移植计划。

---

## 五、已知运行时风险（尚未验证）

- **CTranslate2 动态库路径**：`libctranslate2.dylib` 留在 `target/*/build/ctranslate2-sys-*/out/lib/`，
  未被复制到产物目录。sherpa-onnx 的 build.rs 会自动把 `libsherpa-onnx-c-api.dylib` /
  `libonnxruntime*.dylib` 拷到 `target/<profile>/` 并注入 `@loader_path` rpath，CTranslate2 没有同等处理。
  真正出包（.app / dmg）时需要补一步 `install_name_tool` 或手工复制。
- **权限**：热键走 CGEventTap、注入走 enigo + pbcopy，运行时需要「辅助功能」与「输入监控」授权，
  未签名二进制每次重新编译后授权会失效。

---

## 六、迁移期遗留物

- 仓库根目录的 `node_modules/`（39 个包，全是 win32 二进制）没有对应的 `package.json`，
  疑似历史遗留，macOS 侧不使用；前端依赖只在 `ui/` 下。
- `npm install` 会重写 `ui/package-lock.json`，删掉 win32 可选依赖条目。
  **在 macOS 上装完依赖后请勿提交该文件**，否则 Windows 侧 `npm ci` 会失败。
- `.gitignore` 已追加 `vendor/sherpa-onnx/*-shared-lib{,.tar.bz2}`，避免 15 MB 的 macOS 预编译包进库。
