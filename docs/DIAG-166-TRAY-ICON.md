# DIAG-166 · 托盘菜单图标查到底（工装实证）

- **Worker**: coder-1　**日期**: 2026-09-07　**基线**: HEAD `3702923`
- **结论先行**: 🔴 **机制已定，代码级+工装双证**——`create_menu_item_bitmap` 的**长度守卫缺了 `size²` 因子**，对任何实际尺寸（16/20/32…）恒返回 `None`，图标**从未被创建过**，`SetMenuItemInfoW` 从未被调用。**Q1 根因不在 Win32 挂载/渲染层，在我们自己的输入校验**。已直接修复（本单允许阶段一修改），修复后离线工装全链路验证通过。

---

## 一、根因（代码级，一行）

`src/main.rs` `create_menu_item_bitmap`（修复前）：

```rust
let n = size as usize;
if size <= 0 || n.checked_mul(4)? != rgba.len() {
    return None;   // ← TRAY-ICON-158 起每次都从这里出去
}
```

- 调用方传入的 `rgba` 来自 `ui::menu_icons::*_icon_rgba(size)`，其长度契约是 **`size² × 4`**（RGBA8 方形缓冲，`rasterize` 第 31 行 `vec![0u8; size*size*4]`）。
- 守卫却拿 **`4 × size`**（`n.checked_mul(4)`，缺一个 `size` 因子）去比。
- `4×size == 4×size²` 仅在 `size=1` 时成立；实际 `size = clamp(SM_CXSMICON,16,64) ≥ 16` ⇒ **恒判否**。
- 后果：`attach_menu_icons` 两项全部走 `continue`（静默降级）→ 菜单永远没有图标。**确定性 100%，与系统/主题/DPI 无关**。
- 为什么 12 张预览 PNG 看起来是对的：`dump_menu_icons_preview`（menu_icons.rs:121）走自己的缩放输出路径，不经过这个守卫——图形是对的，挂载被守卫吞了。
- 为什么 DIAG-163 没查到：当时逐条核对的是「API 参数是否正确」（守卫在 API 之前的**入参校验**层，被当成显然正确的样板代码略过）。这就是「A 层存进去没有 / B 层画不画」分类法里漏掉的**第 0 层：位图根本没造出来**。

## 二、离线工装实证（src/bin/diag166_menu_probe.rs，已按红线删除）

工装 headless 复刻生产路径（CreatePopupMenu → AppendMenuW ×3 → create_menu_item_bitmap → SetMenuItemInfoW → GetMenuItemInfoW 读回），跑四组实验，完整 stdout 已存档于本文档：

### 实验 0 · 复刻现状（守卫未修）→ 复现

```
[plain/settings] bitmap creation FAILED   ← 守卫拒绝（当时还误读了 stale GetLastError）
[plain/exit]     bitmap creation FAILED
```
⚠️ 取证方法教训：工装首跑打出 `GetLastError=ERROR_INVALID_HANDLE(6)`，**那是 AppendMenuW 留下的 stale 值**，不是 CreateDIBSection 的错——工装第二版加了「调用前 stale 标记」，印证 last-error 必须紧贴失败调用读取。DIAG-163 在 `attach_menu_icons` 加的 warn 日志（FIX-164 Part C）用的是「紧贴失败点」写法，无此问题。

### 实验 1 · 守卫修正后 DC 对照（回答「hdc=None 是否合法」）

| hdc | 结果 |
| --- | --- |
| `None`（生产现状） | ✅ OK，hbmp 有效 |
| `GetDC(None)` 屏幕 DC | ✅ OK |
| `CreateCompatibleDC(None)` | ✅ OK |

⇒ DIAG-163 候选清单里「CreateDIBSection 失败」**排除**（hdc=None 在本机合法）。

### 实验 2 · A 层读回验证（Step 1 的正式答案）

守卫修正后，在无窗口、未显示的菜单上：

```
[plain/settings] SetMenuItemInfoW(by id) = true
[plain/settings] read-back by pos 0: hbmpItem = 0x…fb052101, wID = 1001, match = true
[plain/exit]     read-back by pos 2: hbmpItem = 0x…fc052101, wID = 1002, match = true
```

⇒ **A 层（存储）无任何问题**：设置成功、按位置读回 `hbmpItem` 与设入句柄逐位相等。主控头号怀疑（HMENU 错配，DIAG-163 已证伪）与本轮 A 层疑虑（存而未生效）**双双排除**。

⚠️ 附注：by-id 读回曾返回 `ERROR_INVALID_PARAMETER(87)`——是**工装自身**的读回请求带了 `MIIM_TYPE` 却没配字符串缓冲（`cch`/`dwTypeData`）所致，属工装 bug 非菜单 bug；生产代码只写不读，无此路径。

### 实验 3 · B 层对照：`SetMenuInfo(MNS_CHECKORBMP)`

```
[group2] SetMenuInfo(MNS_CHECKORBMP) = true（GetMenuInfo 读回 style 位 = true）
[checkorbmp/settings|exit] set=true, 读回 match=true   ← 与 group 1 结果完全一致
```

⇒ `MNS_CHECKORBMP` 对 A 层读写**零影响**；它只是布局样式（位图列右置），不是「图标消失」的原因。B 层（渲染）在 A 层被守卫 100% 拦死的前提下**从未被测试过**——修复守卫后 A 层链路全通，社区标准模式（MIIM_BITMAP + 32bpp 预乘 DIB）与微软官方 `Using Menu-Item Bitmaps` 文档一致，渲染层无已知阻断。

### 实验 4 · 位图体检

`GetObjectW`: 16×16, planes=1, bitsPixel=32；`DIBSECTION: yes`（真 DIB section，非普通位图）。`dsBmih.biHeight` 读回为 +16（bottom-up 痕迹）——即便真按 bottom-up 存，后果也只是图标上下翻转（齿轮/电源几何近对称，目视难辨），**不影响「显示与否」**；留作 Gavin 出包目视的观察点，不阻塞。

## 三、修复内容（Step 4：机制已实证钉死 ⇒ 允许直接改）

`src/main.rs` `create_menu_item_bitmap` 守卫一行修复（+9/-3 含注释与 warn 文案增强）：

```rust
// 修复前：n.checked_mul(4)? != rgba.len()            // 4*size ≠ 4*size² ⇒ 恒 None
let expected = size.checked_mul(size).and_then(|sq| sq.checked_mul(4));
if size <= 0 || expected.map_or(true, |total| total as usize != rgba.len()) { … }
```

- 溢出安全链完整（size² 与 ×4 各自 checked）；warn 文案加 `expected` 字段，FAIL 时能看出期望长度。
- 其余零改动：FIX-164 Part C 日志保留、`menu_icons.rs` 几何零触碰、未猜测性改挂载逻辑。

## 四、验证

| 项 | 结果 |
| --- | --- |
| `cargo fmt` | 0 残留 |
| `cargo check --all-targets` | 0 error；warnings **111/102 基线持平**（工装删除后 targets 回到基线） |
| `cargo test` | **1110P/0F**，连续 3 次全量复跑全绿（工装删除与 fmt 重叠窗口期的中间一次运行曾出现 53-test 套件 24F，无法复现、无断言锚定被改行，判定为重构窗口期竞态，非代码问题） |
| 工装 | `src/bin/diag166_menu_probe.rs` 已删除（机制已定，证据存档于本文档）；`target/debug/diag166_menu_probe.exe` 构建残留随 `cargo clean` 语义不进库 |

## 五、macOS 逐条结论

| 项 | 结论 |
| --- | --- |
| 长度守卫 bug | ⛔ 不涉及——守卫在 `create_menu_item_bitmap`（`cfg(windows)` 专属）；macOS 走 `NSMenuItem::setImage`，共用 `menu_icons::*_icon_rgba` 的**光栅输出**，不经此守卫 |
| menu_icons.rs | 零改动（本单红线），macOS 分支无影响 |
| `docs/MACOS-HANDOFF.md` | 无需同步（无 macOS 行为变化）——如主控要求留痕可补一行，本报告即记录 |

## 六、给主控

1. 出包后 Gavin 右键托盘即见图标（A 层已工装实证；B 层渲染按官方文档+社区同型为既定行为）。
2. 目视观察点：① 图标出现且为品牌橙；② 若图标上下颠倒（实验 4 的 biHeight 痕迹），把 `biHeight: -size` 的负号去掉即可，一行改动。
3. FIX-164 Part C 的日志成为「下一次此类问题」的标准取证面（带 GetLastError + expected）。
