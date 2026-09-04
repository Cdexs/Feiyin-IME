# voice-ime 端侧反馈三个 overlay 渲染 bug 的分析报告

日期：2026-09-04（分析完成，未动代码——用户指示：重启会话后派发修复任务）

## 用户反馈原文（端侧实测，含 debug log）

1. 处理中窗口（"识别处理中..."）边沿圆角有多余的灰线（原设计是包裹窗口边沿的灰线），但整体效果满意
2. ASR 文字上屏显示窗口 bug：说话过程中文字连续显示时会出现**中断显示，窗口中文字全部消失，只剩空窗口**（疑似扩展窗口宽度带来的问题）
3. 显示窗口会出现**位置向左移动**的情况，而不是窗口宽度扩展；文字宽度超过原窗口宽度进行动态宽度扩展时会**抖动**（本质上和第 2 个问题一样？）——期望在原位置上动态扩展窗口宽度，这点对用户体验非常差

附图：SyncClipboard\file\20260904\Image_13895E6ABE30F30523D01D542B050F561EA94D7CCBCC28A6C6C0BC0CD80736CF\Image_2026-09-04_17-01-46_1gy1w1w4.uvy.png（"识别处理中..." 窗口，边沿灰线异常）

代码库：D:\Workspace\CodeLab\voice-ime（Rust + Win32，main.rs 8183 行）
版本基线：commit b6484c2（v0.9.0 阶段五出包完成）

## 关键代码位置（行号基于 b6484c2）

| 区域 | 位置 |
|------|------|
| 悬浮窗命令循环（Show/resize/插值/路由） | main.rs 1044-1755 |
| Show 命令处理（coalescing+插值+居中） | main.rs 1140-1260 |
| 状态切换清理（tween 重置） | main.rs 1285-1330 |
| OVERLAY-068-B snap（current_size=target） | main.rs 1330-1345 |
| 无条件 SetWindowPos（Show 应用几何） | main.rs 1350-1360 |
| 插值循环（current→target 每帧逼近） | main.rs 1666-1687 |
| OVERLAY-068-A R1 插值期 x 重算 + SetWindowPos | main.rs 1689-1717 |
| overlay_wnd_proc（WM_PAINT 双缓冲） | main.rs 1746-1990 |
| WM_PAINT 主体 | main.rs 1882-1946 |
| draw_overlay_to_dc（各状态分派绘制） | main.rs 1963 起 |
| RecordingWithText 绘制分支 | main.rs ~2015-2030 |
| Processing 状态（D2D 优先，GDI 兜底） | main.rs ~2032-2045 |
| draw_overlay_chrome（背景+灰线圆角边框） | main.rs 2113-2137 |
| draw_recording_indicator_and_waveform | main.rs 2140+ |
| 窗口圆角 region | main.rs 374（CreateRoundRectRgn） |
| DWM 圆角 + OVERLAY-064 H1 注释 | main.rs 1156-1170 |
| apply_overlay_window_region | main.rs 374 附近调用（各状态绘制分支均调用） |
| overlay_max_width / adjust_overlay_pos_size_for_text | main.rs 3614 / 3637 附近 |
| 状态机常量（边框灰 0x3A3A3C、背景 0x110F0D、边距 42/49/10/10） | main.rs 1005-1040 |

## Bug 1：处理中窗口圆角边沿多余灰线

### 渲染链
WM_PAINT → draw_overlay_to_dc → Processing 分支 → `apply_overlay_window_region(hwnd, rect, None, true)` + D2D `d2d::draw_processing_overlay`（失败时 GDI `draw_processing_overlay` 兜底）。

### 根因分析
三层叠加导致角部灰线：

1. **GDI RoundRect 描边的像素归属**（main.rs:2125-2135）：1px pen 沿 rect.left/top/right/bottom 画 `RoundRect`，1px 笔有一半像素落在矩形外侧——与窗口 region 的圆角边缘不完全重合。
2. **窗口 region 与描边圆角矩形不对齐**：窗口 region `CreateRoundRectRgn(0,0,width,height,dia,dia)`（main.rs:374）以**窗口坐标系**（含非客户区？无边框窗口则=客户区）建立，而描边在**客户区 rect** 上绘制——两者若差 1px，边框线会被 region 裁剪得断续，角部弧线处尤甚（截图中的"多余灰线"）。
3. **DWM 圆角与 GDI 边框叠加**（main.rs:1156-1170）：Win11 上 DWMWCP_ROUND 会对窗口做 DWM 级圆角裁剪——**GDI 画的 1px 灰色边框最外圈被 DWM 合成裁掉一部分**，形成"边框断续/角落出现杂散灰线"。main.rs:1167 的 OVERLAY-064 H1 注释已记录此现象（"DWM rounded-corner compositing may clip the outermost 1px border drawn by GDI"），归入 Direct2D 迁移（DEC-055）但未修。

### 建议修复方向（任选其一，按代价排序）
- **方案 A（最小改动）**：描边矩形内缩 1px——`RoundRect(hdc, rect.left+1, rect.top+1, rect.right-1, rect.bottom-1, ...)`，使整条边框完整落在 region 内，避免被 DWM/region 裁剪。同时 FillRect 也内缩 1px，防止角落直角背景外露。
- **方案 B**：去掉 GDI RoundRect 描边，圆角视觉完全交给 DWM 圆角 + 深色背景（边框线由 DWM 系统边框承担）——视觉最干净但改变设计（原设计有包裹边沿的灰线）。
- **方案 C（根治，大工程）**：DEC-055 Direct2D 迁移，绘制与圆角全部走 D2D，与 DWM 合成对齐。
- 验证：Win10（SetWindowRgn 路径）与 Win11（DWM 路径）都要测。

## Bug 2：流式 ASR 文字中断显示、窗口清空

### 相关机制
- 流式文本通过 `OverlayCommand::Show(RecordingWithText{text})` 持续下发
- 显示走 tween 机制：`displayed_chars` 向 `tween_target_chars` 逼近（word_timings 驱动）
- 窗口尺寸：`adjust_overlay_pos_size_for_text` 计算目标尺寸 → OVERLAY-068-B 直接 `current_size = target_size`（snap）→ 无条件 SetWindowPos → `InvalidateRect` → WM_PAINT 重绘

### 根因分析（两个候选，需修复时用 debug log 确认主因）

**候选 1：resize 与重绘的时序竞争导致空帧**
- OVERLAY-068-B（main.rs:1330-1345）：streaming-to-streaming 更新时 `current_size = target_size` **立即 snap**，随后无条件 `SetWindowPos` 改变窗口尺寸（宽度变化 → 窗口 region 由 `apply_overlay_window_region` 在下一次 WM_PAINT 的 draw 分支里重建）
- **窗口尺寸变化的瞬间**：SetWindowPos 使客户区变大 → WM_ERASEBKGND/WM_PAINT 之间若 state.needs_repaint 被提前清掉或绘制区域与新尺寸不同步，可能出现**新区域未被绘制**的空窗帧
- 另外：`apply_overlay_window_region(hwnd, rect, None, true)` 在 draw 分支里每次重建 region——窗口尺寸变化后 **region 与新客户区的同步存在 1 帧延迟**（SetWindowPos 改尺寸 → 旧 region 仍按旧尺寸裁剪 → 新增区域被裁掉 → 看起来"文字消失只剩空窗口"）

**候选 2：tween/状态重置竞态**
- OVERLAY-051-G-FIN 分支（main.rs:1296-1310）：status 判定用 `discriminant` 比较——RecordingWithText{..} → RecordingWithText{..} 算不变 ✓；但 `RecordingWithText` → `RecordingStreamingIdle` → `RecordingWithText` 的往返（流式间隙）会触发"离开 RecordingWithText"分支 → `displayed_chars = tween_target_chars` + `word_timings.clear()`——若随后新文本到达且 tween 状态已清，可能出现一帧空文本（displayed_chars=0 或 text 前缀为空）
- `RecordingStreamingIdle` 状态的绘制分支（draw_overlay_to_dc 中未见专门处理？需确认——若落到无绘制分支则整窗空白）

### 建议修复方向
- 修复候选 1：在 `SetWindowPos` 改变尺寸后**立即同步调用** `apply_overlay_window_region`（或把 region 更新移到 SetWindowPos 之前、以新尺寸计算），消除 region 与客户区的 1 帧错位；同时在 OVERLAY-068-B snap 后强制 `InvalidateRect` 全客户区
- 修复候选 2：`RecordingStreamingIdle` 状态保留上一段 `last_streaming_text` 的绘制（不清空），直到新文本到达；`word_timings.clear()` 时不要清 `displayed_chars`
- 修复时加临时 debug log（displayed_chars / text 长度 / 窗口尺寸 / region 重建时机），端侧复现后按 log 确认主因

## Bug 3：宽度扩展时窗口位置向左移动/抖动（期望原位置扩展）

### 根因分析（这个是设计使然，需要改锚点策略）

- 当前所有宽度变化都按**工作区水平居中**重新计算 x（main.rs:1231-1240 与 1689-1707 两处同源）：
  `x = work.left + (work_w - applied_size[0]) / 2`
- 窗口宽度增长 → `x` 减小 → **左边缘向左移动**（右边缘也向右移）——这就是用户看到的"位置向左移动"
- 插值期间（OVERLAY-068-A R1，main.rs:1689-1717）每帧按 `current_size` 重算 x → 窗口逐帧平移 → 抖动
- main.rs:1226-1239 的注释（OVERLAY-068-A）已经意识到 hop 问题并把 x 绑定到 applied_size，但**锚点仍是"水平居中"**，居中锚点天然会左右移动

### 建议修复方向（按用户期望"在原位置上动态扩展窗口宽度"）

用户期望 = **固定一边锚点扩展**。两种锚点策略：
- **方案 A（推荐）**：**左边缘锚定**——首次显示时记录 `anchor_x = pos[0]`，之后宽度变化时 `x = anchor_x` 不变（只改宽度）；仅当宽度增长会超出屏幕右边界时才左移（`x = max(anchor_x, work.right - new_w)`）。符合输入法悬浮窗惯例（跟随输入位置）。
- **方案 B**：右边缘锚定（向左扩展）——如果 mic 图标区在左侧、提交按钮在右侧，右锚会让 mic 固定、向左扩正文区；但初始位置在屏幕中央时左锚更自然。
- 需要用户确认锚点方向（左锚 or 右锚），这属于交互设计决策。

### 修改点（以方案 A 为例）
- `OverlayWindowState` 增加 `anchor_x: i32` 字段（Show 首次显示时记录）
- main.rs:1231-1240 与 1689-1707 两处的居中计算改为 `x = anchor_x`（带右边界钳制）
- EnterEditMode 的 `adjust_overlay_pos_size_for_text` 调用处（main.rs:1410 附近）同样改锚点
- `adjust_overlay_pos_size_for_text` 函数内部的居中逻辑同步检查（main.rs 3637 附近）

## 三个 bug 的依赖关系

- Bug 2 与 Bug 3 相关但不相同：Bug 3 是锚点策略问题（水平居中导致移动），Bug 2 是 resize 与 region/重绘的时序问题。修 Bug 3（锚点固定）后 Bug 2 的"窗口移动感"会减轻，但"空窗口帧"仍需单独修（region 同步）。
- Bug 1 独立。

## 建议修复顺序

1. Bug 3（锚点方案 A，改动集中在 3 处居中计算）——用户痛点最大
2. Bug 2（region 同步 + StreamingIdle 绘制兜底）
3. Bug 1（RoundRect 内缩 1px，最小改动）

## 验证清单

- Win11 DWM 路径 + Win10 SetWindowRgn 路径都要验证圆角/边框
- 流式长文本（>800 字符）连续听写 30s+，观察是否出现空窗口帧
- 宽度从最小扩到 overlay_max_width（overlay_max_width = 屏宽 65%，main.rs:3614）往返多次，观察抖动与位置
- 多显示器（overlay 所在显示器与主屏不同时）水平居中逻辑