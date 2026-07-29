# macOS 移植评估报告

> 评估日期：2026-07-29
> 评估环境：macOS 15.7.8 · arm64（Apple Silicon）· Xcode clang 17.0.0 · Node v24.18.0 / npm 11.16.0
> 评估基线：`695e50e`（v0.7.2，2026-07-28）
> 对应决策：`collab/decisions.md` DEC-033

---

## 0. 本报告的证据等级（重要）

**评估机器上没有安装 Rust 工具链**（`rustc` / `cargo` / `rustup` / `cmake` 全部缺失），因此：

- ✅ **已核实**：代码结构、依赖声明、cfg 分布、平台 API 签名差异、文件行数统计、git 状态 —— 均为直接读取源码/配置得出
- ⚠️ **未核实**：编译错误的**完整**清单。第 2 节列出的阻断项是静态分析得出的，**真实 `cargo check` 大概率会暴露更多**（尤其是 `sherpa-onnx` / `ctranslate2` / `tray-icon` / `eframe` 这几个带原生依赖的 crate 在 darwin 下的行为）

**任何基于本报告的排期，都应在跑通一次真实 `cargo check` 之后重新校准。**

---

## 1. 核心结论

> **项目当前是 Windows-only。macOS 不是"有几个 bug 要修"，而是 Phase 4 尚未开工。**

关键证据在 `src/main.rs:2761`：

```rust
#[cfg(target_os = "windows")]
run_controller(runtime_config)?;
#[cfg(not(target_os = "windows"))]
{
    log::warn!("Non-Windows platform is not supported in controller mode yet");
}
```

整个主控（tray + overlay + 热键分发 + 录音→ASR→LLM→注入 pipeline）都封装在 `run_controller` 内。**即使把全部编译错误修完，macOS 上启动也只会打一行 warn 然后退出。**

`src/main.rs` 共 4416 行，其中 74 处 `cfg(target_os = "windows")`、仅 1 处 macOS。

---

## 2. P0 · 编译 / 链接阻断

| # | 问题 | 位置 |
|---|---|---|
| 1 | **无条件引入 Windows-only 模块**。`mod hotkey;` / `mod injection;` 未加 cfg，而两模块无条件 `use windows::...` + `use std::os::windows::ffi::OsStrExt`。根 crate 的 `windows` 依赖挂在 `[target.'cfg(target_os="windows")'.dependencies]` 下，macOS 上该 crate 不存在 → 成片 E0432。（两模块注释均已标 "Deprecated: use platform::* instead"，只是 `mod` 声明没跟着加 cfg） | `src/main.rs:8,10`<br>`src/hotkey/mod.rs:8`<br>`src/injection/mod.rs:3-5` |
| 2 | **src-tauri 的 `windows` 依赖是无条件的**（不像根 crate 放在 target cfg 下）；`check_hotkey_available` 也无条件调用 `RegisterHotKey`/`UnregisterHotKey` | `src-tauri/Cargo.toml:23`<br>`src-tauri/src/main.rs:42-70` |
| 3 | **`[env]` 段硬编码 Windows 路径**：`SHERPA_ONNX_LIB_DIR = "D:\\Workspace\\..."`。`[env]` 对所有 target 生效 | `.cargo/config.toml` |
| 4 | **无 macOS 版 sherpa-onnx**。`vendor/sherpa-onnx/` 只有 `v1.12.38-win-x64-shared-MD-Release`（.dll/.lib），`sherpa-onnx-lib/` 只有 4 个 .dll，**零 .dylib**。需重新取 osx-arm64 预编译包 | `vendor/` `sherpa-onnx-lib/` |
| 5 | **ctranslate2 需源码编译**（`[patch.crates-io]` 指向 `patches/ctranslate2-sys`），依赖 cmake。当前 features 为 `crt-dynamic`（MSVC 专用）+ `shared` + `ruy`；macOS arm64 应去掉 `crt-dynamic`、启用 `accelerate`（build.rs 已有 `framework=Accelerate` 分支）。**P0 中耗时最长的一项** | `Cargo.toml`<br>`patches/ctranslate2-sys/build.rs:62` |

---

## 3. P1 · 编译通过也跑不起来（架构缺口）

| 缺口 | 现状 |
|---|---|
| 主控入口 | `run_controller` 全 Win32，macOS 无对等实现（见 §1） |
| Overlay | Win32 GDI 手绘（`AlphaBlend` / `SetWindowRgn` / `CreateRoundRectRgn`）。DEC-015 定的 macOS 方案是 Tauri 作事件宿主，`tauri.conf.json` 的 overlay 窗口与 `src-tauri/src/main.rs:181` 的 MAC-013 `set_shadow(false)` 都已就位，**但主程序 → Tauri 进程的 overlay 状态推送通道完全没有** |
| 事件循环 | `src/platform/macos/mod.rs:52` `run_message_loop()` 为 stub，直接返回 `Ok` |
| 托盘 | `tray-icon 0.19` 在 macOS 要求主线程 + NSApplication run loop，当前无处建立 |
| 开机自启 | `src/platform/macos/mod.rs:29-35` 返回 `Err("not implemented")` |
| 设置界面拉起 | `src/main.rs:443` 硬编码 `feiyin-ime-ui.exe` |
| 麦克风静音检测 | `src/audio/mod.rs:842` 非 Windows 恒返回 false（已有 cfg 兜底，属可接受降级） |

**已落地且质量可用的 macOS 代码**：`src/platform/macos/hotkey.rs`（CGEventTap + CFRunLoop，448 行，VK→macOS keycode 映射表完整）、`injection.rs`（pbcopy/pbpaste + enigo 兜底，134 行）。录音走 cpal 跨平台，风险最低。

---

## 4. P2 · 权限与打包（最易低估）

**macOS 的 TCC 权限绑定在「已签名的 bundle identifier」上，不是绑在可执行文件路径上。** 语音输入法需要麦克风、Accessibility（CGEventTap 监听全局热键），可能还需 Input Monitoring。因此：

- `cargo build --release` 产出的裸二进制直接在 `target/release/` 下运行，权限会归属到**终端 App**，行为不可预期，且重编译/移动路径可能反复弹窗
- 必须打包为 `.app` bundle + `Info.plist`（`NSMicrophoneUsageDescription` 等）+ 至少 ad-hoc 签名，权限才稳定
- **全仓库无任何 `Info.plist`、无 `.entitlements`**，仅有 `src-tauri/icons/icon.icns`
- `src/platform/macos/accessibility.rs:44` 的 `ax_is_process_trusted_with_prompt()` 仍是 stub，只打日志、不调 `AXIsProcessTrustedWithOptions` → **用户永远看不到授权弹窗，热键静默失效**
- `tauri.conf.json` 的 `bundle.targets` 仅 `["msi"]`，无 `dmg`/`app`，无 `bundle.macOS` 配置段（`minimumSystemVersion` / `entitlements` / `signingIdentity` 全缺）
- 签名公证：`collab/todo.md:224` 的 MAC-009 标注依赖 "Apple Developer"，**账号尚未具备**（$99/年）。无 Developer ID 只能 ad-hoc 签名本地自用，分发必被 Gatekeeper 拦截
- 目标机为 arm64；universal binary（`lipo`）目前无任何规划

---

## 5. P3 · 工程杂项

- **`.github/` 被 `.gitignore` 整个排除**（注释写着 "macOS CI not yet ready"）。因此 `.github/workflows/build-macos.yml` **从未进入 git，GitHub 上不存在，一次都没被触发过**。它同时还引用了已废弃的产物名 `target/release/voice-ime`（v0.5.4 已改名 `feiyin-ime`），且未安装 cmake、未设置 `SHERPA_ONNX_LIB_DIR`
- `scripts/build-macos.sh` 同样引用旧名 `voice-ime`，无原生库路径处理，属占位
- `build.bat` 硬编码 `D:\Workspace\CodeLab\voice-ime`
- `src/main.rs` 有 8 行 GBK 乱码注释（文件本身为 UTF-8，乱码已固化进内容，不影响编译）；`src/main - 副本.rs` 为 4400 行未被任何 `mod` 引用的备份副本、含 38 行乱码，建议删除（已在 `.gitignore` 内）
- `CLAUDE.md` 通篇以 Windows 为前提；备份脚本为 PowerShell + Windows 计划任务；git 凭证路径 `C:\Users\Aaron-GMK\...` 在 macOS 上不存在。整套协作规程需要重写

---

## 6. 代码结构量化（决定"双平台维护"是否可行的关键数据）

`src/` 共 **24,504 行**：

| 类别 | 行数 | 占比 |
|---|---:|---:|
| 平台无关核心（transcription 2801 / llm 2686 / wordbook 1704 / itn 1523 / scene 1012 / translation 962 / config 1159 / crash 872 / text_normalizer 754 / i18n 611 / version_check 196 / punctuation 161） | ~14,440 | **59%** |
| `main.rs` 内的平台无关 pipeline 逻辑 | ~1,590 | 6% |
| `audio/`（cpal 跨平台，仅 2 处 Win 门） | 1,709 | 7% |
| **`main.rs` 内的 Win32 代码** | **~2,830**（占该文件 **64%**） | 12% |
| `platform/windows/` | 1,543 | 6% |
| `platform/macos/` | 687 | 3% |

**约 70% 的代码天然跨平台。** 另有 **633 个 `#[test]`**，绝大部分位于共享核心，可直接在 macOS 上运行；前端 React 另有 5 个测试文件，零平台风险。

**结论：问题不在"共享得太少"，而在那 30% 的平台代码缺少接缝，且高度集中于 `main.rs` 单文件内的 74 个内联 cfg。**

---

## 7. 平台层已发生的签名漂移（实证）

`src/platform/mod.rs` 使用 glob 导出：

```rust
#[cfg(target_os = "windows")] pub use windows::*;
#[cfg(target_os = "macos")]   pub use macos::*;
```

**这不是抽象层，是两个各自实现、恰好共用一个名字的模块。** 无 trait、无签名契约，编译器每次只看得到其中一半。结果是在项目仍为纯 Windows 的阶段，两侧就已经对不上：

| API | Windows | macOS |
|---|---|---|
| `FocusedTextSnapshot.hwnd` | `HWND` | `usize` |
| `read_text_from_hwnd(h)` | 收 `HWND` | 收 `usize` |
| `create_controller_window()` | `Result<HWND>` | `Result<()>` |
| `notify_config_changed()` | ✅ | ❌ 不存在 |
| `capture_scene_signals()` | ✅ | ❌ 不存在 |
| `HotkeyListener::new_with_controller_wakeup` | ✅ | ❌ 不存在 |

**这些漂移从未触发过任何一次编译错误**，因为两侧从未被同时编译。这是"双平台会不会互相破坏"这一问题的实证答案：**会，而且已经发生了。**

---

## 8. 双平台维护的风险机制

Rust 的一条性质决定了一切：

> **`#[cfg(...)]` 切掉的代码，编译器不做类型检查。**

在 Windows 上 `cargo build` 时，macOS 分支只被当作词法记号扫过（语法不能烂），但**不解析类型、不校验签名、不验证字段存在性**。反之亦然。

| 在 Windows 上的改动 | Windows 编译 | macOS 真实状态 |
|---|---|---|
| `AppConfig` 改字段名 | ✅ 通过 | 💥 已炸，无人知晓 |
| `HotkeyEvent` 加枚举变体 | ✅ 通过 | 💥 match 不穷尽 |
| 改 `inject_text` 参数 | ✅ 通过 | 💥 签名不匹配 |

**破坏发生在提交那一刻，暴露在数周后切换机器那一刻。** 修复成本与破坏存活时长成正比 —— 这正是"反复破坏 + 无谓工作量"的来源。

### 改动风险地图

| 改动位置 | 破坏另一平台的风险 |
|---|---|
| React 前端 `ui/` | 🟢 零 |
| 共享核心（itn / wordbook / llm / transcription / scene） | 🟢 低，633 个单测接得住 |
| `config/mod.rs` 的 `AppConfig` 结构 | 🔴 **最高** —— 两侧平台层都消费它，改字段名 Windows 照样编译通过 |
| `platform/` 任一侧 | 🔴 高，当前无契约 |
| `main.rs` | 🟠 中高，64% 为 Win32 |

---

## 9. 工作量估算

| 阶段 | 内容 | 估时 |
|---|---|---|
| **A. 打通编译** | 装 rustup/cmake → 修 §2 的 5 项阻断 → 取 sherpa-onnx osx-arm64 → ctranslate2 源码编译调 features | **1–2 天**（ct2 编译踩坑可能占大半） |
| **B. 主控可运行** | macOS 事件宿主 + tray 主线程 run loop + `run_controller` 的 macOS 对等实现，跑通「热键→录音→ASR→注入」闭环，overlay 先降级不显示 | **1–2 周** |
| **C. 权限 + 打包** | Accessibility 弹窗补完 + `.app` bundle + Info.plist + entitlements + ad-hoc 签名 + Tauri overlay IPC 通道 | **1–2 周** |
| **D. 分发** | Developer ID 签名 + notarization + dmg | 卡 Apple 账号；账号到位后 2–3 天 |

**单人全职约 4–6 周**至可分发状态；至"本机能自用"约 2–3 周。

---

## 10. 待 Gavin 拍板的开放问题

1. **overlay 路线取舍**（影响 B/C 阶段做法）
   - `main.rs` 那 2830 行 Win32 绝大部分是手绘 GDI overlay + Win32 消息循环 controller。而 `tauri.conf.json` 中**已有配好的 overlay 窗口**，Tauri 2 也自带跨平台 tray
   - 若 overlay/tray 统一走 Tauri，平台特定代码可从 ~5000 行压到 ~800 行（只剩热键与注入，且 macOS 侧均已写好）
   - **代价**：Windows v0.7.2 已交付用户，原生 GDI overlay 大概率是为延迟与置顶行为而选（DEC-003）。为 macOS 重写已稳定的 Windows 关键路径，是拿已交付质量换未交付平台
   - **建议折中**：不重写 Windows overlay；将 overlay 抽为 trait，Windows 保留 GDI 实现，**macOS 直接复用已配好的 Tauri overlay 窗口** —— Windows 零回归，macOS 无需新写原生代码，trait 保证契约不漂
2. **DEC-000 的措辞**是否需要更新。DEC-000 现为「最高优先级：Windows 系统兼容性」，与"正式支持 macOS"存在字面冲突。本报告不擅自改动 DEC-000，需 Gavin 决定
3. **Apple Developer 账号**（$99/年）是否投入 —— 决定 D 阶段能否启动

---

## 11. 建议的第一步

**先只做 A 阶段，且顺序是：装工具链 → 跑一次真实 `cargo check` → 拿到完整错误清单 → 再决定修法。**

理由见 §0：本报告的阻断清单是静态分析结果，原生依赖在 darwin 下的真实行为必须实测才能确定。在拿到真实错误清单前，不要细化 B/C 阶段的排期。
