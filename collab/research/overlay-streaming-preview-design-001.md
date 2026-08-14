# DESIGN-OVERLAY-037 · 流式预览 overlay 交互设计

> 任务类型：纯设计文档，零代码改动。
> 设计者：coder-2（前端/UI 负责人）
> 日期：2026-08-14
> 依据：Gavin 2026-08-14 定稿需求 + 主控现状取证 + 既有 DEC 约束。

---

## 〇、设计结论摘要

1. **技术路线**：复用现有 Win32 GDI overlay（DEC-003），不引入新 UI 框架。在录音 overlay 内**叠加文本预览区**，而不是新建独立窗口。
2. **核心冲突的解法**：采用「**双阶段窗口样式**」。
   - **录音/流式显示阶段**：保持 `WS_EX_NOACTIVATE`，overlay 不抢焦点，目标应用 `target_hwnd` 稳定。
   - **用户接管编辑阶段**：去掉 `WS_EX_NOACTIVATE` 并重新 `SetWindowPos(..., SWP_FRAMECHANGED)`，让 overlay 可获焦；编辑完成提交后恢复 `WS_EX_NOACTIVATE` 并归还焦点给 `target_hwnd`。
3. **编辑实现**：推荐 `CreateWindowExW` 内嵌 `EDIT` 控件，理由中文 IME 支持零成本；视觉贴合通过 `WS_CHILD`、`ES_AUTOHSCROLL`、自定义字体/颜色、无边框子类化实现。
4. **PTT 编辑时机**：按 Gavin 拍板采用「乙」——录音中用户点击 overlay 文本区即进入编辑态，不等松开热键。松开热键默认触发 pipeline 后处理，逃生通道必须在录音进行中就能拉。
5. **滚动策略**：65% 上限后，文本区内部左滚（GDI 文本裁剪 + 偏移量），不是窗口整体滚动。
6. **颜色语义**：语音录入文本专用 `白色（#FFFFFF）`；中间未确认结果**不引入下划线**（Gavin 拍板：下划线会丑/奇怪），整段流式文本统一白色；提交按钮用品牌橙，属系统 UI 语义层。

---

## 一、目标与需求逐条映射

Gavin 2026-08-14 定稿目标：换用流式 ASR 后，实现「边录边显示，最终格式优化输出」。TSF 组合文本方案已判死（`collab/research/tsf-composition-feasibility-001.md`），改走 overlay 浮层预览，并**复用现有录音 overlay 窗口**。

| Gavin 原始 UX 描述 | 本设计落点 |
| --- | --- |
| 1. 按下热键开始录入，文字实时显示在 overlay 窗口，从中间开始显示 | 在现有 `Recording` overlay 内新增文本区；文本区水平居中，初始宽度最小，随文字量向两侧扩展 |
| 2. 根据文本量动态扩展窗口宽度，最大宽度 = 当前屏幕显示宽度的 65% | 每次 ASR 增量到达后重新测量文本宽度并调整窗口宽度；上限以 **主屏工作区宽度 × 0.65** 为准 |
| 3. 超过 65% 仍在录入 → 窗口内文本向左滚动，保证最新录入的文字显示在窗口最右端 | 文本区内部滚动：维护一个水平偏移量 `scroll_x`，新文本从右端进入，旧文本被裁切在左端 |
| 4. 用户在窗口中键入光标/移动光标 → 进入「输入后手动编辑阶段」 | 动态去掉 `WS_EX_NOACTIVATE`；overlay 获焦；内嵌 `EDIT` 控件接管输入；显示提交按钮；停止录音与后续 pipeline |
| 4a. 窗口最右端显示提交按钮（回车提交图案） | 在 overlay 右侧内嵌一个自绘按钮（回车箭头图标），支持鼠标点击与 Enter 键 |
| 4b. 立刻停止当前录入及后续 pipeline 处理 | 停止音频采集线程、关闭 ASR WebSocket/取消本地 ASR 任务、取消待发送的 LLM 请求 |
| 4c. 待用户自行编辑后注入到光标处 | 提交时把 `EDIT` 控件内容原样注入 `target_hwnd`，不走 LLM；若 `target_hwnd` 因 UIPI 无法取回焦点，降级为复制到剪贴板 + 预览浮层提示 |

---

## 二、核心冲突：NOACTIVATE vs 可编辑

### 2.1 现状

现有 overlay 创建时带有 `WS_EX_NOACTIVATE`（`src/main.rs:632`）：

```rust
CreateWindowExW(
    WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_NOACTIVATE,
    ...
)
```

`WS_EX_NOACTIVATE` 的官方语义：窗口被点击时**不会成为前台窗口**，系统不会通过键盘导航激活它，**必须显式调用 `SetActiveWindow` / `SetForegroundWindow` 才能激活**（出处：[Extended Window Styles - WS_EX_NOACTIVATE](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles)）。

这是刻意设计——overlay 若抢焦点，`GetForegroundWindow()` 抓到的 `target_hwnd` 会失效，后续注入就没了着落。

### 2.2 设计解法：双阶段样式

不要创建两个窗口，而是**在同一个 overlay hwnd 上动态切换扩展样式**。

| 阶段 | 扩展样式 | 行为 |
| --- | --- | --- |
| 录音 / 流式显示 / Processing | `WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_NOACTIVATE` | 不抢焦点，用户可继续在其他窗口操作 |
| 用户接管编辑 | 移除 `WS_EX_NOACTIVATE` | overlay 可获焦，`EDIT` 子控件可接收键盘/IME |
| 提交 / 取消后 | 恢复 `WS_EX_NOACTIVATE` | 回到被动显示模式 |

切换方式：

```rust
let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
let new_ex_style = ex_style & !(WS_EX_NOACTIVATE.0 as isize);
SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex_style);
SetWindowPos(hwnd, None, 0, 0, 0, 0,
    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE);
```

要点：
- 必须用 `SWP_FRAMECHANGED` 让系统重新计算非客户区（出处：[SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos)）。
- `SWP_NOACTIVATE` 在切换样式时仍然加上，避免样式变更本身导致意外激活；真正激活交给后续 `SetActiveWindow(hwnd)` / `SetFocus(edit_hwnd)`。

### 2.3 焦点归还与 UIPI 降级

提交时必须把焦点还给 `target_hwnd`：

```rust
// 1. 先让目标窗口尝试取回焦点
let result = SetForegroundWindow(target_hwnd);
// 2. 再隐藏/重置 overlay 样式
restore_noactivate(hwnd);
overlay_handle.send(OverlayCommand::Hide);
```

但 `SetForegroundWindow` 受 UIPI 限制：当 `target_hwnd` 所在进程以管理员权限运行、而飞音未提权时，调用会失败（返回 0）。现有代码 `:2716` / `:2908` 的 `focus_lost` 降级逻辑已经处理了这一场景。

**建议的降级链**：

1. 提交时先调用 `SetForegroundWindow(target_hwnd)`。
2. 若返回 0 或目标窗口已非前台（`GetForegroundWindow() != target_hwnd`）：
   - 不直接注入；
   - 把文本复制到剪贴板；
   - overlay 切到 `FocusLost` 预览态（现有 `:2010`），提示用户「目标窗口无法取回焦点，内容已复制，请手动粘贴」。
3. 若返回非 0：走 `platform::inject_text(target_hwnd, text)` 原样注入。

---

## 三、问题 1 · 窗口几何

### 3.1 「从中间开始显示」的准确含义

建议解释为：**窗口水平居中于主屏，文本内容在窗口客户区内水平居中显示**。

- 窗口初始尺寸沿用 `RECORDING_OVERLAY_SIZE = [240, 36]`（`src/main.rs:560`）。
- 文本区占客户区中间一段，左右保留指示灯、分隔条、停止按钮的固定宽度。
- 当文本变长，窗口以**中心为锚点向两侧扩展**，保持窗口整体仍水平居中于屏幕。

### 3.2 动态扩宽的锚点与动画

扩宽策略：

1. 收到新的流式文本后，用 `GetTextExtentPoint32W` 测量完整文本宽度（含当前字体）。
2. 计算所需窗口宽度 = 文本宽度 + 左侧固定区（指示灯 30px + 分隔条 2px + 间距 12px）+ 右侧固定区（停止按钮 25px + 间距 12px + 预留提交按钮 24px）。
3. 上限 = `monitor_work_width * 0.65`。
4. 若新宽度 ≤ 上限：以屏幕中心为锚点重新计算 `x = (screen_w - new_w) / 2`，调用 `SetWindowPos(hwnd, None, x, y, new_w, h, SWP_NOZORDER | SWP_NOACTIVATE)`。
5. 若新宽度 > 上限：窗口宽度锁定在 65%，进入内部滚动。

动画：不建议做补间动画。窗口位置/尺寸调整本身由 DWM 处理，追求低延迟；每 100-200ms 的 ASR 增量触发一次重绘已足够平滑。

### 3.3 65% 上限的计算基准

| 问题 | 建议 |
| --- | --- |
| 哪个显示器 | 以 overlay 线程创建时 `MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)` 所在显示器为准 |
| 多屏 | 使用 `GetMonitorInfoW` 取该显示器的 `rcWork`（工作区，扣除任务栏），而非 `GetSystemMetrics` 的全屏虚拟尺寸 |
| DPI 缩放 | 所有坐标/尺寸使用 **物理像素**。`GetTextExtentPoint32W` 在设备上下文使用 `CreateClearTypeFont` 时返回的已是物理像素；`GetMonitorInfoW` 返回的也是物理像素 |

> 现有 `overlay_geometry`（`src/main.rs:1742`）使用 `GetSystemMetrics(SM_CXSCREEN)` 取的是主屏宽度，且未考虑 DPI。建议新实现改为 `MonitorFromWindow` + `GetMonitorInfoW`。

---

## 四、问题 2 · 文本滚动实现

### 4.1 滚动方式

窗口达到 65% 上限后，文本区内部左滚。不是滚动窗口，而是**在内存 DC 中绘制时设置裁剪区并偏移文本起点**。

```rust
// 伪代码
let text_width = measure_text(hdc, text);
let visible_w = client_w - left_fixed - right_fixed;
let scroll_x = if text_width > visible_w {
    text_width - visible_w
} else {
    0
};
// 设置裁剪区为文本区
SelectClipRgn(hdc, text_region);
// 文本起点在客户区左边缘 + left_fixed - scroll_x
let text_x = rect.left + left_fixed - scroll_x;
draw_text(hdc, text, text_x, text_baseline);
```

### 4.2 滚动步进与平滑

- 滚动是**像素级**的，不是字符级。`scroll_x = max(0, text_width - visible_w)`，保证最新文字紧贴右边缘。
- 不需要额外平滑：每次 ASR 增量更新时重绘一次，文本自然平滑推进。
- 若 ASR 一次修正导致文本宽度大幅缩短（如把「北京今天天气」改成「北京今天」），`scroll_x` 会同步回退，最新文字仍贴右边缘。

---

## 五、问题 3 · 流式增量渲染与反抖动

### 5.1 数据来源

流式 ASR 会周期性推送 `result-generated` 事件，文本会修正。新增一个 overlay 状态字段：

```rust
struct OverlayWindowState {
    request: Option<OverlayRequest>,
    streaming_text: String,      // 当前流式文本
    streaming_confirmed_len: usize, // 已确认部分字节长度（可选）
    cancel_btn_rect: Option<RECT>,
    ...
}
```

### 5.2 重绘频率控制

- 不要每次 ASR 事件都立即 `SetWindowPos` / `InvalidateRect`。
- 在音频线程/事件线程把文本写入共享状态；overlay 线程已有的 16ms timer（60fps）自然会在下一帧读取并绘制。
- 窗口尺寸调整（扩宽）可以**每 100ms 节流一次**，避免 ASR 高频修正导致窗口抖动。

### 5.3 避免闪烁

- 继续使用现有双缓冲（内存 DC → `BitBlt`）。
- 文本区背景用 `BG_DARK` 先填充，再绘制文本，避免透明叠加残留。
- 不要每次重绘都重新创建字体/画笔，把它们缓存在状态里。

---

## 六、问题 4 · 状态机

```
Idle
  │ 用户按热键
  ▼
Recording ──OverlayStatus::Recording + streaming_text 实时更新
  │ 用户点击 overlay 文本区 / 移动光标
  ├──────────────────────────────────────────────┐
  │                                                ▼
  │                                        StreamingEditing
  │                                          ──移除 NOACTIVATE
  │                                          显示 EDIT 控件 + 提交按钮
  │                                          停止录音 + ASR（路径 A，无 LLM）
  │
  │ 松开热键（PTT 默认行为）                     │
  │  或 收到 Stop（Toggle）                       │
  │  但 pipeline_cancelled=true → 拦截           │
  ▼                                                │
FallingToProcessing ──音量条衰减动画                │
  │ 音频停止                                       │ 用户提交（Enter / 点击提交按钮）
  ▼                                                ▼
Processing ──LLM 格式化 / 标点 / ITN 补丁通道      InjectRaw
  │ LLM 成功                                       ──原样注入 EDIT 文本
  ▼                                                恢复 NOACTIVATE，隐藏 overlay
Done ──注入 / 隐藏 overlay                         │
                                                   │ 用户取消（ESC / 点击停止按钮）
                                                   ▼
                                              Cancelled
                                                   ──隐藏 overlay

StreamingEditing 期间再次松开热键（PTT）
  │
  ▼
NO-OP（空操作）
  ──已在进入 StreamingEditing 时设置 pipeline_cancelled=true
  ──松开热键的处理函数检查到 pipeline_cancelled，直接返回，不触发 FallingToProcessing
```

说明：
- `StreamingEditing` 是一个新增 overlay 状态，不是现有 `OverlayStatus` 枚举的替代。
- 进入 `StreamingEditing` 的同时发送 `OverlayUiEvent::EditRequested` 到主控，主控负责：
  - 设置 `stop_recording_signal = true`；
  - 取消当前 ASR WebSocket / 本地 ASR future；
  - 设置 `pipeline_cancelled = true`，用于拦截后续松开热键触发的 FallingToProcessing；
  - 取消待发送的 LLM 请求（若已发送则忽略其后续结果）。
- **编辑态下松开热键空操作**：PTT 的 `HotkeyEvent::Stop` 到达时，主控先检查 `pipeline_cancelled`；若为 true，则不进入 FallingToProcessing/Processing，只把 `stop_recording_signal` 置 true 保证音频线程干净退出。

---

## 七、问题 5 · 提交按钮

### 7.1 位置与图案

- 位置：overlay 客户区最右端，停止按钮左侧或右侧（建议放在停止按钮**左侧**，避免误触停止）。
- 图案：回车箭头（⏎，Unicode `U+23CE`）。尺寸 16×16。
- 颜色：品牌橙 `BRAND_ORANGE`（#FF6B00），属系统 UI 语义层，不是用户内容。

### 7.2 点击与 Enter 等价

- 鼠标：在 `WM_LBUTTONUP` 中判断命中提交按钮矩形 → 触发 `OverlayUiEvent::SubmitRequested`。
- 键盘：EDIT 控件内监听 `WM_KEYDOWN`，若 `VK_RETURN` 且无 Ctrl/Shift → 触发提交。
- ESC：取消编辑，恢复录音停止后的正常流程或隐藏 overlay。

### 7.3 与 `apply_overlay_window_region` 的关系

`apply_overlay_window_region`（`src/main.rs:291`）用 `SetWindowRgn` 裁剪圆角。编辑态下窗口形状不变，只是内部多一个 EDIT 子控件。子控件区域必须完全落在圆角矩形内部（留足 10px 圆角边距），否则会被 `SetWindowRgn` 裁掉。

---

## 七A · EDIT 控件精确视觉规格（Gavin 拍板要求）

### 7A.1 总体约束

- **不能覆盖/遮挡原 overlay 的指示灯、分隔条、停止按钮**。
- **不能露白底**：EDIT 控件背景色必须与 `BG_DARK` 完全一致。
- **不能出现系统默认滚动条**：水平滚动由 `ES_AUTOHSCROLL` 内部处理，不显示滚动条。
- **选中/高亮色必须贴合品牌色系**，不能跳出 Windows 默认蓝紫高亮。

### 7A.2 精确尺寸

| 项 | 值 | 来源/说明 |
| --- | --- | --- |
| overlay 客户区 | 240×36（初始） | `RECORDING_OVERLAY_SIZE`（`src/main.rs:560`） |
| 圆角半径 | 10px | `CORNER_RADIUS`（`src/main.rs:1073`） |
| 左侧固定区 | 42px | 指示灯区 30px（含 6px 左间距 + 18px 图标 + 6px 右间距） |
| 右侧固定区 | 49px | 停止按钮区 25px + 提交按钮区 24px |
| 文本区左侧 | 42px | 左固定区右边缘 |
| 文本区右侧 | 191px | 客户区宽 240 − 右侧固定区 49 |
| 文本区宽度 | 149px | 240 − 42 − 49 |
| 文本区高度 | 22px | 客户区高 36 − 上边距 7 − 下边距 7 |
| 文本区上边距 | 7px | 使文本垂直居中于 36px 高度 |
| 文本区下边距 | 7px | 同上 |
| 文本区与圆角边界最小距离 | 10px | 防止圆角裁掉 EDIT 内容 |
| 提交按钮尺寸 | 16×16 | 与现有停止按钮同高 |
| 提交按钮右间距 | 9px | 客户区右边缘 240 − (25 停止按钮 + 9 间隔 + 16 提交按钮 + 9 右间距) = 191 文本区右边缘 |

### 7A.3 样式与颜色

| 项 | 值 |
| --- | --- |
| EDIT 控件背景色 | `BG_DARK` `COLORREF(0x110F0D)` |
| EDIT 文本色 | `COLORREF(0xFFFFFF)` 白色 |
| EDIT 边框 | 无（子类化 `WM_NCPAINT` 返回 0） |
| EDIT 字体 | `create_clear_type_font(-12)`，Segoe UI，与现有绘制一致 |
| 选中高亮色 | `BRAND_ORANGE` `COLORREF(0x006BFF)`（#FF6B00） |
| 选中高亮文本色 | 白色 |
| 滚动条 | 不显示（无 `WS_HSCROLL`/`WS_VSCROLL`，用 `ES_AUTOHSCROLL` 内部滚动） |

### 7A.4 子类化去边框

```rust
// 父窗口 overlay_wnd_proc
WM_NCPAINT => {
    if hwnd == edit_hwnd {
        return LRESULT(0); // 不绘制 EDIT 默认边框
    }
}
WM_CTLCOLOREDIT => {
    SetBkColor(hdc, BG_DARK);
    SetTextColor(hdc, COLORREF(0xFFFFFF));
    return bg_brush as LRESULT; // 返回 BG_DARK 画刷
}
```

### 7A.5 与 `apply_overlay_window_region` 的精确关系

- `SetWindowRgn` 裁剪后的可见区域是 `(0,0)-(w,h)` 内距离边缘 ≥10px 的圆角矩形内部。
- EDIT 控件矩形 `(42, 7)-(191, 29)` 完全落在该区域内：
  - 左侧距圆角边界 42px ≥ 10px ✅
  - 右侧距圆角边界 49px ≥ 10px ✅
  - 上侧距圆角边界 7px < 10px ❌ → 需要把上边距调整为 **10px**，高度改为 **16px**（36 − 10 − 10）。

**修正后规格**：

| 项 | 修正后值 |
| --- | --- |
| 文本区上边距 | 10px |
| 文本区下边距 | 10px |
| 文本区高度 | 16px |
| 文本区矩形 | (42, 10)-(191, 26) |

这样 EDIT 控件四边均距圆角边界 ≥10px，不会被 `SetWindowRgn` 裁掉。

---

## 八、问题 6 · 中断语义

用户进入编辑态时，必须干净停止以下任务：

| 任务 | 停止方式 |
| --- | --- |
| 音频采集 | 设置 `stop_recording_signal = true`；音频线程检测到后停止 `cpal` 流 |
| ASR WebSocket（qwen3_online） | 关闭 WebSocket 连接；丢弃未完成的 `input_audio_buffer` 提交 |
| 本地 ASR（performance/accuracy） | 取消当前 `transcribe_segment` 的 future；若仍在模型推理中，Rust async cancel 会自然终止 |
| LLM 格式化 | 若录音停止后已发送 LLM 请求，设置一个 `llm_cancelled` AtomicBool；LLM 回调中检测到则丢弃结果 |
| 标点引擎 | 若未进入 LLM 分支，本地标点引擎正在运行；取消方式同上 |

**按进入编辑态的时机分两条路径**：

- **路径 A：从 `Recording` 进入 `StreamingEditing`（Gavin 拍板主路径）**
  - 音频：正在采集 → 设置 `stop_recording_signal = true`，音频线程自然停止。
  - ASR：正在流式识别（WebSocket 在线 / 本地实时）→ 关闭 WebSocket 或取消本地 ASR future。
  - LLM：**尚未触发**（因为还没松开热键）→ 无 LLM 请求可取消。
  - 标点：**尚未触发**。
  - 实现：进入编辑态时同步设置 `pipeline_cancelled = true`，拦截后续松开热键的 FallingToProcessing。

- **路径 B：从 `FallingToProcessing` / `Processing` 进入 `StreamingEditing`（松开热键后用户才点击）**
  - 音频：已经停止或正在停止中。
  - ASR：已经返回最终文本。
  - LLM：可能已发送请求 → 设置 `llm_cancelled = true`，LLM 回调忽略结果。
  - 标点：可能正在运行 → 设置 `postprocess_cancelled = true`，标点线程/回调忽略结果。
  - 实现：编辑态下所有 pipeline 事件到达时都先检查取消标志，已取消则直接丢弃。

**关于 `sherpa-onnx` C 端能否安全中断**：
- 当前设计暂不依赖「强制中断 C 端推理」。音频线程停止后，本地 ASR 的输入流会自然结束；`transcribe_segment` 会拿到最终音频片段并完成一次识别，之后退出。
- 若 `sherpa-onnx` 的 `Recognizer` 正在执行 `recognize()`，Rust 侧取消 future 只会停止等待结果，**C 端推理可能继续运行直到结束**。
- **结论**：安全中断能力待实测验证；实现上先采用「停止输入 + 忽略后续结果」的温和策略，不强行 abort C 线程。

**关键点**：停止后不能留下悬挂任务占用 CPU/网络。建议在 `StartCmd` 中携带一个 `CancellationToken`（或用现有 `cancel_signal`），进入编辑态时触发它。

---

## 九、问题 7 · 回归面

| 既有行为 | 是否受影响 | 说明 |
| --- | --- | --- |
| 音量条波形 | 否 | 录音阶段继续绘制，编辑态可隐藏或淡化 |
| 状态提示（Processing/Error/FocusLost） | 否 | 这些状态不显示文本区 |
| 2500ms 自动复位（`:95`） | 否 | 仅 Error 态使用；编辑态不触发 |
| 停止按钮 | 是 | 编辑态下停止按钮改为「取消编辑」或保留停止语义 |
| `apply_overlay_window_region` 圆角 | 是 | 需确保 EDIT 控件在圆角内（见 §7A） |
| `SW_SHOWNA` 显示方式 | 否 | 录音/流式阶段仍用 `SW_SHOWNA`；编辑态改用 `SW_SHOW` |
| ESC 取消 | 是 | 录音阶段 ESC 取消录音；编辑阶段 ESC 退出编辑态 |
| **托盘图标状态（新增）** | **是** | 编辑态期间托盘应显示为 **Recording**，因为用户仍在处理本次语音输入；不可在 2500ms 后自动复位为 Idle，否则用户会误以为任务已结束 |

---

## 九A · 托盘状态补充（回归面漏项）

### 9A.1 编辑态期间托盘图标

| 状态 | 托盘图标 | 原因 |
| --- | --- | --- |
| Recording + 流式显示 | Recording | 用户正在说话 |
| StreamingEditing | **Recording** | 用户仍在处理本次语音输入的编辑；任务未结束，不能显示 Idle |
| InjectRaw / Cancelled | Idle | 编辑完成或取消后恢复 |

### 9A.2 实现方式

- 进入 `StreamingEditing` 时，不调用 `set_tray_state(tray, TrayState::Idle, ...)`。
- 也不启动任何自动复位 timer；现有的 2500ms 自动复位只对 `Error` 态生效（`src/main.rs:95` 相关逻辑），编辑态不参与。
- 提交或取消后，由 `InjectRaw` / `Cancelled` 路径主动设置 `TrayState::Idle`。

---

## 十、问题 8 · 跨平台结论

### 10.1 Windows

- 本设计针对 Windows 实现。
- 需要改动的文件（未来实施时）：
  - `src/main.rs`：overlay 样式切换、EDIT 控件创建/销毁、提交按钮绘制、流式文本字段、状态机扩展。
  - `src/ui/overlay.rs`：可能新增 `OverlayStatus::StreamingEditing` 或相关辅助类型。
  - `src/i18n.rs`：新增「提交」「编辑」相关文案（若需要）。

### 10.2 macOS

- macOS 侧已有 `src/platform/macos/overlay.rs`（`NSPanel` + 自定义 `OverlayView` + `CGContext` 绘制，DEC-045）。
- 本次新增的需求（流式文本预览、可编辑、提交按钮）是**产品级行为**，不是 Windows 专属 API。
- **结论**：macOS 侧未来实现时应**复用同样的产品状态机**，但底层窗口/编辑控件需用 AppKit 实现（`NSTextField` / `NSTextView`）。
- 需要在 `docs/MACOS-HANDOFF.md` 中追加评估：Windows overlay 新增「流式预览 + 编辑态」的交互契约，macOS 侧实现时应保持状态机一致，但具体控件由 AppKit 提供。

---

## 十一、两个待 Gavin 拍板项的建议

### 11.1 ① PTT 模式下，编辑阶段何时可进入？

**Gavin 拍板：乙——录音中用户点击 overlay 文本区即进入编辑态，不等待松开热键。**

理由（Gavin 提出，设计侧接受并修正）：
- 松开热键即触发 pipeline 后处理（LLM/标点/ITN 补丁），此时再进入编辑态等于永远赶不上。
- 逃生通道必须在录音进行中就能拉；点击文本区/移动光标即表示用户要接管控制权，应当立即响应。
- 因此 PTT 与 Toggle 模式在此行为上统一：录音中点击 overlay 文本区即进入编辑态。

| 模式 | 行为 |
| --- | --- |
| PTT | 录音中点击 overlay 文本区 → 立刻进入 `StreamingEditing`，同时停止录音与后续 pipeline |
| Toggle | 同上 |

### 11.2 ② overlay 内的文本编辑如何实现？

**建议：Win32 `EDIT` 子控件 + 子类化去边框 + 父窗口处理 `WM_CTLCOLOREDIT`。**

理由已在 7.1 详述：中文 IME 支持是决定性因素，自绘编辑成本过高且风险大。

---

## 十二、颜色语义设计（Gavin 追加要求）

### 12.1 白色使用范围（Gavin 拍板：不下划线）

Gavin 要求：语音录入的文本颜色显示为**白色**，以便区别于系统提示和错误信息。

| 元素 | 颜色 | 语义 |
| --- | --- | --- |
| 流式识别文本 | `COLORREF(0xFFFFFF)` 白 | 用户说出来的内容 |
| 用户手动编辑后的文本 | 白色 | 仍是「用户内容」，机器未改写 |
| 提交按钮 | `BRAND_ORANGE` #FF6B00 | 系统 UI 动作 |
| 停止/取消按钮 | `BRAND_ORANGE` / 灰 | 系统 UI 动作 |
| 音量条/波形 | `BRAND_ORANGE` | 系统状态反馈 |
| 错误信息 | `RED_STREAM_FAILED` #FF0000 | 系统错误 |

### 12.2 中间结果 vs 已确认结果

**Gavin 拍板：不引入下划线。** 下划线会让流式文本看起来「丑和奇怪」。

**设计侧建议（待 Gavin 确认是否采用）**：
- 方案 A（推荐但非 Gavin 要求）：对未确认部分使用 **白色 70% 透明度** 渲染，已确认部分用纯白色。这样既有视觉区分，又不引入下划线。
- 方案 B：完全不区分，整段流式文本统一纯白色。实现最简单，但用户无法一眼看出哪些文本仍在变化。

**当前按 Gavin 拍板落地方案 B**：整段流式文本统一白色，不区分已确认/未确认。若后续端测发现需要更强反馈，再按方案 A 评估。

### 12.3 现有三色是否受影响

| 颜色 | 既有用途 | 影响 |
| --- | --- | --- |
| `BRAND_ORANGE` #FF6B00 | 品牌橙、正常状态、波形、停止按钮 | 不变 |
| `BRIGHT_ORANGE` #FF8C00 | 更亮的橙，Processing shimmer | 不变 |
| `RED_STREAM_FAILED` #FF0000 | 设备错误 | 不变 |
| `GRAY_SILENT` #808080 | 静音 | 不变 |
| `BG_DARK` #0D0F11 | 深色背景 | 不变，EDIT 控件背景也用它 |

白色是新增语义色，与现有三色无冲突。

---

## 十三、实现阶段建议（未来落地时）

1. **Phase 1**：在现有 `Recording` 状态内绘制流式文本（只读），实现动态扩宽与 65% 滚动。
2. **Phase 2**：添加 EDIT 控件与提交按钮，实现用户接管编辑。
3. **Phase 3**：添加状态机扩展与主控事件联动（停止录音/ASR/LLM）。
4. **Phase 4**：UIPI 降级、DPI 多屏适配、macOS 评估同步。

---

## 十四、风险清单

| # | 风险 | 严重度 | 缓解 |
| --- | --- | --- | --- |
| 1 | `SetWindowLongPtrW` 动态改 `WS_EX_NOACTIVATE` 在某些系统上可能不立即生效 | 中 | 必须配合 `SetWindowPos(..., SWP_FRAMECHANGED)`，并在取回焦点后再改 |
| 2 | 目标窗口管理员权限导致 `SetForegroundWindow` 失败 | 高 | 已有 `focus_lost` 降级链，复制到剪贴板 + 预览提示 |
| 3 | `EDIT` 子控件在 `WS_EX_LAYERED` 父窗口下的 IME 候选框位置异常 | 中 | 子类化后测试中文/日文/韩文 IME；必要时用 `ImmSetCompositionWindow` 调整 |
| 4 | 流式 ASR 高频修正导致窗口宽度抖动 | 中 | 窗口尺寸调整节流 100ms；文本区内部滚动时窗口不再变 |
| 5 | `sherpa-onnx` 本地 ASR 无法安全取消正在运行的推理 | 高 | 需实测 `Recognizer::recognize` 在 cancel 后的行为；必要时只停止音频输入、等待自然结束 |
| 6 | macOS 侧未来实现时状态机分叉 | 中 | 在 `docs/MACOS-HANDOFF.md` 中明确记录交互契约 |
| 7 | 用户误触编辑态 | 中 | PTT 下仅松开热键后进入；点击区域限制在文本区，避开停止/提交按钮 |

---

## 十五、参考链接

- [Extended Window Styles - WS_EX_NOACTIVATE](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles)
- [SetWindowPos function](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos)
- [SetWindowLongPtr function](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowlongptra)
- [SetForegroundWindow function](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow)
- [SetActiveWindow function](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setactivewindow)
- [TSF Composition Feasibility Research](tsf-composition-feasibility-001.md)
