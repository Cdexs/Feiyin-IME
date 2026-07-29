# macOS cfg 分支静态审计 · AUDIT-MACOS-BRANCH-001

> 日期：2026-07-30  
> 范围：`src/` 与 `src-tauri/src/` 内所有 `#[cfg(target_os = "macos")]` / `#[cfg(not(target_os = "windows"))]` / `#[cfg(unix)]` / `#[cfg(target_family = "unix")]` 分支  
> 方法：仅静态阅读 + docs.rs（crates.io 版本）交叉核对；**未在 macOS 上运行 `cargo check`，所有结论为静态推测，需 macOS 侧编译验证**  
> 任务类型：纯审计，**零代码改动**  

---

## 1 · 执行摘要

共审计 **15 处 cfg 分支**（排除 `patches/`、`vendor/`、`src/main - 副本.rs`）。
- **P0（极可能编译失败）：1 处**
- **P1（运行时空壳/降级）：8 处**
- **P2（签名可疑/语义需确认）：4 处**
- **P3（已知占位/风险透明）：2 处**

未发现会污染 Windows 路径的问题；所有风险均集中在 `#[cfg(target_os = "macos")]` 或 `#[cfg(not(target_os = "windows"))]` 分支内，符合 Rust Reference 的 cfg 移除规则。

---

## 2 · 关键发现

### P0 · `src/crash/reporter.rs:369` — `egui::FontData::from_bytes()` 不存在

```rust
// src/crash/reporter.rs:361-372
#[cfg(target_os = "macos")]
{
    let font_path = "/System/Library/Fonts/PingFang.ttc";
    if std::path::Path::new(font_path).exists() {
        if let Ok(font_data) = std::fs::read(font_path) {
            fonts.font_data.insert(
                "PingFangSC".to_owned(),
                egui::FontData::from_bytes(font_data)   // <-- 不存在
                    .ok()
                    .unwrap_or_default(),
            );
            ...
        }
    }
}
```

- **依据**：`Cargo.lock` 中 `egui = 0.29.1`，`FontData` 的构造函数只有 `from_static` 与 `from_owned`（docs.rs 已确认）。
- **影响**：macOS 分支下 `cargo check` / 编译必然报错。
- **修复方向**：改为 `egui::FontData::from_owned(font_data)`。
- **置信度**：高（docs.rs 已精确核对）。

---

### P1 · 运行时空壳/降级（macOS 路径已声明未实现）

| 位置 | 现象 | 后果 | 修复责任 |
|---|---|---|---|
| `src/main.rs:2765-2768` | `#[cfg(not(target_os = "windows"))]` 分支仅 `log::warn!` 不调用 `run_controller` | 非 Windows 主进程入口无任何功能 | macOS 团队 |
| `src/main.rs:3419-3475` | `mod macos_stubs` 提供空 `OverlayThreadHandle` / `spawn_worker_thread` 等 | overlay、worker、pipeline 完全空转 | macOS 团队 |
| `src/platform/macos/mod.rs:30-58` | `enable/disable/is_enabled/create_controller_window/run_message_loop` 仅返回 Err/Ok/warn | 开机自启、控制器窗口、消息循环均未实现 | macOS 团队 |
| `src/platform/macos/mod.rs:79-82` | `capture_scene_signals(_hwnd: usize)` 永远返回 `None` | 场景信号降级为 Unknown，场景词表/格式化失效 | macOS 团队 |
| `src/platform/macos/mod.rs:66-68` | `notify_config_changed()` 仅 log | config 热更新无法真正唤醒 macOS 侧 listener | macOS 团队 |
| `src/platform/macos/accessibility.rs:43-48` | `request_accessibility_permission()` 空实现，未调用 AX API | 无法申请 Accessibility 权限，CGEventTap 可能无法工作 | macOS 团队 |
| `src/platform/macos/injection.rs:41-47` | `capture_focused_text_snapshot/read_text_from_hwnd` 永远 `None` | 取词、聚焦应用读回不可用 | macOS 团队 |
| `src/audio/mod.rs:864-867` | `is_mic_muted()` 非 Windows 永远 `false` | 麦克风静音检测不可用 | macOS 团队 |

这些属于 DEC-033 明确的 Phase 3 占位，**已在注释中标记为 stub**，不是编译错误，但构成 macOS 产品化缺口。

---

### P2 · 签名可疑/语义需 macOS 侧验证

#### P2-1 · `src/platform/macos/hotkey.rs:115` — `get_integer_value_field` 返回值截断

```rust
let event_keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
```

- `CGEvent::get_integer_value_field` 返回 `i64`（core-graphics 0.25.0 source line 816），截断为 `u16` 对 keycode 是安全的，但 `HotkeyBinding.mac_keycode` 已定义为 `u16`。
- **需确认**：当 keycode 为 `0` 时是否会产生误判（`vk_to_mac_keycode` 不返回 `0`，但理论上 `CGKeyCode = u16`）。
- **置信度**：中（类型安全，但需真机验证逻辑）。

#### P2-2 · `src/platform/macos/hotkey.rs:121-123` — modifier 匹配语义

```rust
let modifiers_ok = modifiers_match(flags, binding.modifiers, binding.primary_modifier_flag);
```

`modifiers_match` 函数在比较时把 `primary_modifier_flag` 从 `required` 和 `actual` 中双双 `remove`，意味着如果热键本身是 `Shift`/`Ctrl`/`Alt`/`Win`，不会要求该修饰键被额外按下，且不会把它当作普通修饰键处理。这与 Windows 侧逻辑是否一致需人工核对；若用户把 `Shift` 设为主键，`FlagsChanged` 触发逻辑会依赖 `primary_key_pressed`。
- **置信度**：中。

#### P2-3 · `src/platform/macos/hotkey.rs:125-126` — autorepeat 字段用于 KeyDown

```rust
let is_repeat = event_type == CGEventType::KeyDown
    && event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) != 0;
```

`KEYBOARD_EVENT_AUTOREPEAT` 文档说明“non-zero when this is an autorepeat of a key-down”，对 `FlagsChanged` 不适用；代码只在 `KeyDown` 分支使用，符合文档。但 `FlagsChanged` 重复触发可能会让 `primary_key_pressed` 反复变化——需真机验证。
- **置信度**：中。

#### P2-4 · `src/platform/macos/hotkey.rs:244-251` — `CGEventTap::new_unchecked` 生命周期

```rust
let tap = unsafe {
    CGEventTap::new_unchecked(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        KEYBOARD_EVENTS.to_vec(),
        move |_proxy, event_type, event| callback_context.borrow_mut().handle_event(event_type, event),
    )
};
```

core-graphics 0.25.0 的 `new_unchecked` 要求调用者保证对象在 `'tap_life` 之前 drop，且回调捕获的状态要么可跨线程 Send，要么 tap 只安装在当前线程 run loop。代码将 tap 与回调都限制在单线程 listener 内，且 `run_loop` 与 `TapContext` 都是单线程的，**理论上满足 unsafe 契约**。但 `new_unchecked` 是 unsafe，且 `CGEventTap<'tap_life>` 的 lifetime 由调用者推断，需 macOS 编译确认。
- **置信度**：中（unsafe 边界正确性无法静态 100% 确认）。

---

### P3 · 低风险/已知占位

| 位置 | 说明 |
|---|---|
| `src/crash/mod.rs:177-183` | `get_windows_version()` 非 Windows 返回 `OS (version detection not implemented)`，已在注释中声明为占位。 |
| `src/crash/reporter.rs:302-303` | macOS 图标路径使用 `.png`（`assets/icons/app.png`），该文件是否存在需 macOS 团队确认；若不存在则 icon 为 `None`，不影响编译。 |

---

## 3 · 已确认正确的 API 使用

以下调用与 `Cargo.lock` / docs.rs 核对后，**签名匹配**（仍需 macOS 编译最终确认）：

| 文件 | 调用 | docs.rs 版本 | 结论 |
|---|---|---|---|
| `src/platform/macos/hotkey.rs:8-11` | `CGEventTap`, `CGEventFlags`, `CGEventTapLocation`, `CGEventTapOptions`, `CGEventTapPlacement`, `CGEventType`, `CallbackResult`, `EventField`, `KeyCode` | core-graphics 0.25.0 | 类型均存在 |
| `src/platform/macos/hotkey.rs:244` | `CGEventTap::new_unchecked(...)` | core-graphics 0.25.0 | 签名匹配（unsafe） |
| `src/platform/macos/hotkey.rs:255-257` | `tap.mach_port().create_runloop_source(0)` | core-graphics 0.25.0 source + core-foundation 0.10.1 `CFMachPort::create_runloop_source` | 签名匹配，返回 `Result<CFRunLoopSource, ()>` |
| `src/platform/macos/hotkey.rs:259-264` | `CFRunLoop::get_current()`, `add_source(&source, unsafe { kCFRunLoopCommonModes })` | core-foundation 0.10.1 | 签名匹配；mode 类型为 `CFRunLoopMode`（`CFStringRef`），unsafe 使用 `kCFRunLoopCommonModes` 是惯例 |
| `src/platform/macos/hotkey.rs:281-285` | `CFRunLoop::run_in_mode(...)` | core-foundation 0.10.1 | 签名匹配 |
| `src/platform/macos/injection.rs:66-72` | `Enigo::new(&Settings::default())`, `Keyboard::text(text)` | enigo 0.2.1 | 签名匹配；注意 `text` 不能含 `\0` |
| `src/platform/macos/injection.rs:75-90` | `enigo.key(Key::Meta, Press)`, `Key::Unicode('v')` | enigo 0.2.1 | `Key::Meta` 存在，`Key::Unicode(char)` 存在 |
| `src-tauri/src/main.rs:193` | `overlay.set_shadow(false)` | tauri 2.10.3 | 方法存在，仅在 macOS 有意义 |
| `src-tauri/src/version_check.rs:111-117` | `Command::new("open").arg(&url)` | 标准库 | 语义正确 |
| `src-tauri/src/overlay.rs:40-41` | `#[cfg(not(target_os = "macos"))] .transparent(true)` | tauri 2.10.3 | 与 MAC-013 注释一致 |

---

## 4 · 潜在但需进一步核对的符号

以下符号 docs.rs 页面或路径返回 404/不可用，**未做精确签名核对**，建议 macOS 团队编译时重点关注：

- `core_foundation::runloop::{kCFRunLoopCommonModes, kCFRunLoopDefaultMode}`：源码中它们是 `pub static` 的 `CFStringRef`，需要 `unsafe` 使用；`add_source` 与 `run_in_mode` 都需要 `CFStringRef` 作为 mode。
- `EventField::KEYBOARD_EVENT_AUTOREPEAT` 在 core-graphics 0.25.0 中为 `CGEventField = 8`，与代码一致（source line 275）。

---

## 5 · 建议 macOS 团队首项修复

1. **先修 P0**：`src/crash/reporter.rs:369` 改为 `egui::FontData::from_owned(font_data)`，这是唯一会阻塞编译的已知问题。
2. **运行 `cargo check --target aarch64-apple-darwin` / `cargo check --target x86_64-apple-darwin`**，捕获 `hotkey.rs` 中 `KeyCode::COMMAND` / `RIGHT_COMMAND` 等常量是否实际存在（docs.rs source line 138-146 已确认存在）。
3. **验证运行时权限流**：`accessibility.rs` 需要真正调用 `AXIsProcessTrustedWithOptions` 才能让 CGEventTap 工作。
4. **overlay/controller 路径**：`main.rs:3419` 的 `macos_stubs` 与 `main.rs:2765` 的非 Windows 入口均未实现，是 Phase 3 最大工作项。

---

## 6 · 未覆盖范围说明

- 未审计 `patches/`、`vendor/` 内的 `#[cfg]`（属于依赖补丁，非项目代码）。
- 未审计 `src/llm/**`（当前任务边界外）。
- 未审计测试文件（tester-1 任务边界）。
- 所有结论基于 crates.io 当前 `Cargo.lock` 版本；若 macOS 团队升级依赖，需重新核对。

---

## 7 · 置信度汇总

| 级别 | 含义 | 数量 |
|---|---|---|
| 高 | docs.rs 精确核对或源码直接可见 | 1 |
| 中 | 静态推断合理，但需 macOS 编译/运行确认 | 4 |
| 低 | 仅声明为 stub，行为已知降级 | 10 |

---

## 8 · 参考资料

- `Cargo.lock`：`egui 0.29.1`、`core-graphics 0.25.0`、`core-foundation 0.10.1`、`enigo 0.2.1`
- docs.rs：
  - https://docs.rs/egui/0.29.1/egui/struct.FontData.html
  - https://docs.rs/core-graphics/0.25.0/core_graphics/event/index.html
  - https://docs.rs/core-foundation/0.10.1/core_foundation/runloop/index.html
  - https://docs.rs/enigo/0.2.1/enigo/index.html
  - https://docs.rs/core-graphics/0.25.0/src/core_graphics/event.rs.html
- `docs/MACOS-HANDOFF.md` §1.1 / §4
- `docs/MACOS-PORT-ASSESSMENT.md`
- `docs/BUILD-MACOS.md`
