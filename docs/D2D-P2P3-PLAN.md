# D2D-P2P3-PLAN-108 · 剩余五态迁移方案设计（只出文档，零代码改动）

**作者**：coder-2 ｜ **日期**：2026-09-05 ｜ **状态**：待主控评审
**基线**：`b072383`（HEAD），`src/main.rs` 9362 行；全部行号已按该基线实测核对
（任务书里写的部分行号与本基线有漂移，**以本文为准**，主控可 `sed -n` 抽查）。

**范围**：`Recording` / `FallingToProcessing` / `StreamingEditing` / `FocusLost` / `Error`
五态从 GDI 迁到 D2D（DEC-057 ②，P2+P3 合并一批）。**不碰**：per-pixel alpha（DEC-057 ③）、
合成方式、`mod d2d` 已迁路径、窗口尺寸/居中体系。

---

## 0. 执行摘要

- 五态全部走 P1 已定型的 `with_d2d` 帧层（`:3099`），**零帧管理层新代码**。
- 新 D2D 原语实际是 **4 个**，不是任务书点名的 3 个：`waveform`、`submit_button`、
  `preview`（标题栏+正文+双按钮的复合体，其中「复制/关闭键」是任务书第 3 项）、
  `error_badge`（红点，很小，与 error 文本合成一个状态级复合体）。
  `chrome`/`mic_indicator`/`stop_button`/`streaming_text_format` 全部复用。
- **关键简化**：`FallingToProcessing` 与 `Recording`（波形变体）的 GDI 像素输出**本来就逐位相同**
  （`draw_overlay_to_dc:2127-2131` vs `draw_recording_overlay:2509-2518`，同一组绘制函数），
  所以两者**共用同一个 D2D 复合体** `draw_recording_waveform_overlay`，只写一次。
- 新增 `D2dResources` 字段 2 个（`centered_text_format`、`wrap_text_format`），
  **不加任何新 thread_local** → D2D-HANG-095 的 `release_resources()`（`:3079`）
  整槽 take/drop 天然覆盖，护栏测试（`main.rs:9277/:9310`）无需改动。
- 五态**没有一个**需要 `measure_text_width`/任何 DirectWrite 度量 → D2D-P1 裁定③
  「度量单一源」零新增风险（见 §3.3-B）。
- StreamingEditing 的 EDIT 子控件共存结论（§3.3-A）：**D2D 迁移不改变父子绘制时序，
  无新增覆盖/闪烁风险**；论证见该节。

---

## 3.1 逐态盘点表（五态 × 每态一节）

### 3.1-A `Recording`（录音波形态）—— 唯一高频态

**1) 当前 GDI 绘制调用链**

```
draw_overlay_to_dc :2070-2079（Recording 分支）
  ├─ apply_overlay_window_region(hwnd, rect, None, true)      :2071   矩形 region
  └─ draw_recording_overlay(hdc, rect, state, false, lang)    :2495
       ├─ draw_overlay_chrome(hdc, rect)                      :2509 → :2198  底 #110F0D + 1px 边 r10
       ├─ draw_recording_indicator_and_waveform               :2516 → :2229
       │    ├─ 三态麦克风图标（18px，4x HALFTONE 超采样）     :2239-2314
       │    ├─ 左分隔线（x=left+30, 高20, 2px）               :2316-2327
       │    ├─ 波形 32 条（bw=3 bgap=2, maxh=48/static 12/min 8, gain 2.5, decay 0.02）:2330-2434
       │    └─ 右分隔线（x=right-36, 高20, 2px）              :2435-2443
       └─ draw_stop_button(hdc, rect) → cancel RECT           :2518 → :2447
```

**2) 绘制元素清单**

| 元素 | 处置 |
| --- | --- |
| chrome | ✅ 复用 `d2d::chrome`（`:3347`，已逐位对齐 GDI :2198） |
| 三态麦克风图标 + 左分隔线 | ✅ 复用 `d2d::mic_indicator`（`:3390`，P1 已迁，三态色逻辑 `:3396-3409` 与 GDI `:2248-2265` 同构；D2D 原生 AA 取代 HALFTONE 超采样是 P1 已验收的既定差异） |
| **波形 32 条** | 🔴 **新写** `d2d::waveform`，见 §3.2-A |
| 右分隔线 | ✅ 复用 `draw_streaming_text_overlay:3663-3684` 同款内联画法（x=w-36、高20、2px、OVERLAY_BORDER_GRAY）——提为 `right_separator(res,w,h)` 小原语供两处调用，消除第二份内联 |
| stop button | ✅ 复用 `d2d::stop_button`（`:3494`） |

**3) 命中矩形**：仅 `cancel_btn_rect`（stop 键）。
GDI 路径返回 `draw_stop_button:2447` 的 `cr`（`right/bottom +1`，GDI Rectangle 排他约定，
`STOP-BUTTON-CENTER-FIX-001`）。D2D 成功路径沿用 P1 既有模式：返回
`draw_stop_button_hit_rect_only(rect)`（`:2524`）——该 helper 与 `stop_button` D2D 原语
内部返回的 `cr` 是同一套公式（`bs=16, bl=right-25, +1`），几何口径单一源已成立。
**保证手段**：D2D 复合体不自行计算 hit rect，一律由调用侧 helper 出 rect（P1 先例 `:2506-2508`）。

**4) 窗口 region / 圆角**：`apply_overlay_window_region(hwnd, rect, None, true)`（`:2071`）= 矩形 region。
迁移**不变**——该调用在 dispatch 层、D2D/GDI 分流之前，本方案不动 dispatch 的 region 调用。
注意点：D2D `chrome` 是圆角填充，矩形 region 下四角会露出 WM_PAINT 预填充底色
（`:1963-1978`，OVERLAY_BG_DARK `#110F0D` = chrome 底色同色）→ 不可见，且 P1 两个已迁态
（idle/text，region 同为 None）已是此形态，线上已验收，非新风险。

### 3.1-B `FallingToProcessing`（落处理中过渡态）

**1) 当前 GDI 绘制调用链**

```
draw_overlay_to_dc :2127-2132
  ├─ apply_overlay_window_region(hwnd, rect, None, true)          :2128  矩形 region
  ├─ draw_overlay_chrome(hdc, rect)                               :2129 → :2198
  ├─ draw_recording_indicator_and_waveform(hdc, rect, state)      :2130 → :2229
  └─ draw_stop_button(hdc, rect) → cancel RECT                    :2131 → :2447
```

**2) 与 `Recording` 的关系（本方案的核心简化）**：
`FallingToProcessing` 的三个 GDI 调用与 `Recording`（`show_placeholder=false` 变体）经
`draw_recording_overlay:2509-2518` 展开后**完全同构、同序**（chrome → indicator_and_waveform →
stop_button，参数仅 `state`/`rect`）。像素输出逐位相同。
→ **D2D 复合体共用** `d2d::draw_recording_waveform_overlay(hdc, rect, state)`
（= `with_d2d` + chrome + mic_indicator + waveform + right_separator + stop_button），
`Recording` 与 `FallingToProcessing` 两个 dispatch 分支都调它。
零新原语、零重复代码，且两态切换时画面无缝（本来就该无缝——GDI 时代就是同一段绘制）。

**3) 命中矩形**：仅 `cancel_btn_rect`，同 §3.1-A（`draw_stop_button_hit_rect_only`）。

**4) region**：`None, true`（`:2128`），迁移不变，理由同 §3.1-A。

**附注**：该态是短瞬过渡（`run_overlay_thread:1660-1700` 定时落 `Processing`），
60fps 波形动画在它身上同样跑（`has_active_overlay` 判定 `:1799` 含全部在显状态），
所以 §3.2-A 的性能论证覆盖它，无需单独论证。

### 3.1-C `StreamingEditing`（编辑态，含 Win32 EDIT 子控件）

**1) 当前 GDI 绘制调用链**

```
draw_overlay_to_dc :2159-2165
  ├─ apply_overlay_window_region(hwnd, rect, None, true)      :2160  矩形 region
  ├─ draw_overlay_chrome(hdc, rect)                           :2163 → :2198
  └─ draw_submit_button(hdc, rect) → submit RECT              :2164 → :2646
（正文文字由 EDIT 子控件自绘，父窗口不画文字）
```

EDIT 子控件生命周期（不在绘制路径上，但迁移验收要盯）：
`create_edit_control :593`（EditRequested 时 `:1470` 创建）、`destroy_edit_control :783`
（`:1293/:1529/:1561` 三处销毁）。子控件自带 ClearType 字体（`WM_SETFONT :698-705`，
OVERLAY-054-D）、背景刷 OVERLAY_BG_DARK（`:668`，`WM_CTLCOLOREDIT :1932` 消费）、
子类化 proc（`:720`，WM_NCPAINT 抑制 + Enter 转发）。

**2) 绘制元素清单**

| 元素 | 处置 |
| --- | --- |
| chrome | ✅ 复用 `d2d::chrome`（`:3347`） |
| **提交键（橙圆角 ⏎）** | 🔴 **新写** `d2d::submit_button`，见 §3.2-B |
| 正文文字 | **不迁移** —— EDIT 子控件自绘，D2D 不碰；窗口宽度仍由 `adjust_overlay_pos_size_for_text:4241` 用 GDI `measure_text_width`（`:4265`）决定，本批零改动 |

**3) 命中矩形**：仅 `submit_btn_rect`。
🔴 **几何口径必须逐位保留的不对称点**：`draw_submit_button:2653-2658` 返回的 RECT
是 `right = bl+bs, bottom = bt+bs` —— **没有** `+1`（与 `draw_stop_button:2453-2458` 的
`+1` 排他约定**不同**）。这是两套按钮各自的历史行为，点击判定 `rect_contains` 消费的
就是这个值。D2D 路径必须新增 `draw_submit_button_hit_rect_only(rect)` helper
（镜像 `:2524` 模式，公式 `bs=16, bl=right-25, 无 +1`），GDI/D2D 两路径都用它出 rect，
D2D `submit_button` 原语内部返回值与之同公式。**不许**在迁移时"顺手统一" `+1`——
那是行为变更，会让点击热区漂 1px，且与 GDI 兜底不一致。

**4) region**：`None, true`（`:2160`），迁移不变。四角露出问题同 §3.1-A（同色不可见）。

### 3.1-D `FocusLost`（失焦预览态）

**1) 当前 GDI 绘制调用链**

```
draw_overlay_to_dc :2166-2173
  ├─ apply_overlay_window_region(hwnd, rect, Some(10), false)     :2167  圆角 region r10
  └─ draw_preview_overlay(hdc, rect, text, lang) :3832 → (copy, close, title_close)
       ├─ FillRect 全矩形底 #211D1A                              :3844-3848
       ├─ RoundRect 1px 边 r10（OVERLAY_BORDER_GRAY）            :3850-3867
       ├─ 标题栏文字（橙，DT_CENTER，28px 高带）                 :3868-3886
       ├─ 标题关闭键 18x18（r6 圆角边 OVERLAY_BTN_BORDER + ✕ U+2715 橙）:3888-3924
       ├─ 标题分隔线（y=top+28, 1px, left+8 → right-8）          :3925-3933
       ├─ 正文（#F2F2F2，DT_LEFT|DT_WORDBREAK|DT_END_ELLIPSIS）  :3934-3949
       └─ 底部居中双键 45x18 gap10（r8 边 OVERLAY_BTN_BORDER；复制橙字/关闭灰字）:3950-4020
```

**2) 绘制元素清单** —— 整体新写为**一个状态级复合体** `d2d::preview_overlay`
（内含 5 个小步骤，均为一次性直线/矩形/文本，不单独成可复用原语）：

| 元素 | 处置 |
| --- | --- |
| 底 + 边 r10 | 🟡 变体复用：`chrome` 的底色是 `OVERLAY_BG_DARK #110F0D`，本态是 `#211D1A` → chrome 需参数化（§0/H1：抽 `chrome_with(res,w,h,bg,radius)`，`chrome` 变一行包装，已迁路径零行为变化） |
| 标题文字（橙，居中） | 🔴 新画，用新 `centered_text_format`（§0/H2） |
| 标题关闭键 + ✕ | 🔴 新画（`DrawRoundedRectangle` r6 + centered format 画 `\u{2715}`） |
| 标题分隔线 | 🔴 新画（`DrawLine` 1px） |
| 正文 DT_WORDBREAK | 🔴 新画，用新 `wrap_text_format`（`WORD_WRAPPING_WRAP`）—— 全 mod d2d 唯一的换行文本 |
| 底部复制/关闭键 | 🔴 **新写**（任务书 3.2 第三项），见 §3.2-C |

**3) 命中矩形**：三个（这是五态中唯一的"多命中"态）。
dispatch `:2168-2172` 把 `copy_rect → cancel_btn_rect`、`close_rect → close_btn_rect`、
`title_close_rect → title_close_btn_rect`；点击判定 `WM_LBUTTONUP:1870-1891`
（关闭=底部关闭 ∨ 标题 ✕；复制=底部复制）。
🔴 **保证手段（单一源）**：把三个 rect 的计算从 `draw_preview_overlay:3869-3968`
抽成纯函数 `preview_hit_rects(rect) -> (RECT, RECT, RECT)`
（title_close `:3888-3893` / copy `:3957-3962` / close `:3963-3968`，全部只依赖 `rect` 整数运算），
GDI 绘制函数与 D2D 包装**都**调它出返回值——几何口径物理上不可能分叉。
这是 `centered_x`（OVERLAY-101）同款哲学：返回值不允许两份算术。

**4) region**：`Some(10)`（`:2167`）。`CreateRoundRectRgn(0,0,w,h,20,20)`（`:372-374`）
与绘制边 r10 严格同径 → D2D 用 `chrome_with(bg=#211D1A, r=10)` 的圆角填充+圆角描边
（OVERLAY-086 Bug 1 的「fill/stroke 共用一个半径绑定」教训，`:3178-3200` 已示范），
region 把角外像素全部裁掉，无外露。迁移**不变**。

### 3.1-E `Error`（错误态）

**1) 当前 GDI 绘制调用链**

```
draw_overlay_to_dc :2174-2177
  ├─ apply_overlay_window_region(hwnd, rect, Some(10), false)     :2175  圆角 region r10
  └─ draw_error_overlay(hdc, rect, message, lang) :4024（无返回值，五元组全 None）
       ├─ FillRect 全矩形底 #211D1A                               :4036-4040
       ├─ RoundRect 1px 边 r10                                    :4042-4059
       ├─ 红色实心圆 d=8（center x=left+12+4）                    :4060-4080
       └─ 错误文本（橙 BRAND_ORANGE，DT_LEFT|VCENTER|SINGLELINE|END_ELLIPSIS,
          left = circ_x + circ_d/2 + 8, right-14）                :4081-4096
```

**2) 绘制元素清单** —— 新写状态级复合体 `d2d::error_overlay`：

| 元素 | 处置 |
| --- | --- |
| 底 + 边 r10 | 🟡 复用参数化后的 `chrome_with(#211D1A, 10)`（同 §3.1-D） |
| 红点 | 🔴 新写，但只是一句 `FillEllipse`（`Ellipse:4069-4075` 的 D2D 直译，圆心/直径同公式） |
| 错误文本 | 🟡 **复用现有 `streaming_text_format`**（`:3035`，Segoe UI Normal 14, LEADING + NO_WRAP 垂直居中）——与 GDI `DT_LEFT|VCENTER|SINGLELINE` 语义逐项对应，**零新 format**。唯一语义差是 `DT_END_ELLIPSIS`（见 §3.5-U1） |

**3) 命中矩形**：**无**（五元组全 `None`，`draw_error_overlay` 无返回值）。迁移后不变。
无点击交互（错误态只能等超时 `WM_TIMER:2017-2030` 或热键取消），无点击错位风险。

**4) region**：`Some(10)`（`:2175`），同 §3.1-D 论证。迁移不变。

---

## 3.2 三个（实为四个）真正新写的原语

### 3.2-A `waveform`（`Recording` / `FallingToProcessing`，`draw_recording_indicator_and_waveform` :2229 内 ：2330-2434 段）

**D2D 下怎么画**：
- 32 条 3px 圆角竖条（GDI `RoundRect(…, bw*2=6, 6)` = r3 全圆角）→ D2D
  `FillRoundedRectangle` × 32，`radiusX=radiusY=3.0`，单支 `brush` `SetColor(BRAND_ORANGE)` 一次。
- 几何（逐位照抄 GDI）：`bc=32, bw=3, bgap=2, half=16`；`ww = (w-36) - 30 - 24`；
  `wl = 30+12+(ww-108)/2`（rect 相对坐标，D2D 原点=BindDC 子矩形左上，`:3322-3330` 既有约定）；
  `by = h/2`；`maxh=48, static_h=12, minh=8, gain=2.5`。
- 逐条 bar 的 `x` 与 GDI 完全同式：左半 `wl+(half-1-i)*(bw+bgap)`、右半 `wl+(half+i)*(bw+bgap)`。
  GDI bar 高是 `i32`（`minh + v_gain*(maxh-minh)` 截断），D2D 侧**保持 i32 运算后转 f32**
  （整数像素对齐，避免 GDI/D2D 两条路径因 f32 直算出现 ±1px 视觉差）。
- 权重公式 `0.4 + 0.6*cos²(π/2·i/(half-1))` 逐字照抄（`:2372-2376` / `:2405-2409`）。

**逐帧几何从哪来（数据链路）**：
`state.audio_buf`（`Mutex<Vec<AudioLevel>>`）→ 快照。🔴 **必须保持 OVERLAY-LOCK-SCOPE-001
语义**（`:2347` 注释 + `:2348-2366` 现行代码）：**锁内**完成 decay（`level.update(current, 0.02)`）
+ 收集 16 个 `display_value()`，**锁外**绘制。D2D 版逐字复用该段逻辑。
为杜绝 GDI/D2D 两份实现漂移，把两段纯逻辑抽成共享纯函数（IMPL 单实施，本方案裁定）：
- `waveform_snapshot(state, half) -> Vec<f32>`（锁+decay+快照，`:2348-2366`）；
- `waveform_bar_height(v, i, half) -> i32`（权重+gain+高度公式，`:2370-2382`）。
GDI 循环与 D2D 循环都改调这两个纯函数（同值替换，REFACTOR-088/089 先例模式），
消融表随 IMPL 单给出。

**性能与 GDI 版对比**（这是唯一每帧变化的高频绘制；60fps 帧驱动 `:1799`）：
- GDI 现状每帧：32×2 半 = 32 次循环，每次 `CreateSolidBrush` + `CreatePen(PS_NULL)` +
  `SelectObject`×2 + `RoundRect` + `SelectObject`×2 + `DeleteObject`×2 ≈ **224 次 GDI 对象
  创建/销毁**，外加三态图标 72×72 HALFTONE 超采样 `StretchBlt` 一次（`:2266-2314`）。
- D2D 每帧：1 次 `SetColor` + 32 次 `FillRoundedRectangle`（栈上结构体）+ 图标走 P1
  `mic_indicator`（无超采样）。GDI 对象操作 **224 → 0**，GPU 光栅化替代 CPU GDI。
- 结论：D2D 版本每帧成本**严格低于** GDI 版，无新增性能风险；60fps 余量变大而非变小。
  唯一新增每帧分配是快照 `Vec<f32>`（16 元素，与 GDI 现状相同，非新增）。

### 3.2-B `submit_button`（`StreamingEditing`，`draw_submit_button` :2646）

- 几何（rect 相对）：`bs=16, bl=w-25, bt=(h-16)/2`；外框 `DrawRoundedRectangle`
  `radius=5.0`（GDI `RoundRect(…, CORNER_RADIUS=10, 10)` 的 GDI 语义：椭圆直径 10 → 半径 5；
  与 `chrome` 的 `RoundRect(…,10*2,10*2)`→r10 不同，**别抄混**），1px 描边内缩 0.5px
  （`stop_button:3505-3524` 同款像素对齐论证）。
- 填充：`FillRoundedRectangle` OVERLAY_BRAND_ORANGE。
- ⏎（U+23CE）箭头：GDI 用 `draw_text`（DT_CENTER|DT_VCENTER|SINGLELINE）+ dispatch 级
  缓存字体（Segoe UI -14，`:2052-2056`）→ D2D 用新 `centered_text_format`
  （Segoe UI Normal 14, CENTER + PARAGRAPH_CENTER + NO_WRAP）`DrawText` 到按钮 rect。
  文字色 BG_DARK `#110F0D`（`:2677`）。
- **返回 RECT 无 +1**（§3.1-C-3 红线，`bl..bl+bs` 原样）。
- 不需要任何度量（DT_CENTER/DWRITE_CENTER 自居中）→ 无度量源问题。

### 3.2-C 复制/关闭键（`FocusLost` 底部双键，`draw_preview_overlay` :3950-4020）

- 几何：`btn_w=45, btn_h=18, gap=10`，`btn_left = left + (W-100)/2`（水平居中于全宽，
  `:3955`），`btn_top = bottom-28`（`:3956`）。两键 `DrawRoundedRectangle` r=4
  （GDI `RoundRect(…,8,8)` → 半径 4），1px OVERLAY_BTN_BORDER `#707070`。
- 标签：复制=`preview_copy_btn` 橙、关闭=`preview_close` 灰 `#808080`
  （`:4002-4019`），皆 DT_CENTER → `centered_text_format`。
- 命中 rect 由共享纯函数 `preview_hit_rects` 出（§3.1-D-3），原语自身不算几何。
- 同批顺带画成的前后文：标题 ✕ 键（18x18，r=3：GDI `RoundRect(…,6,6)`）与标题栏/
  正文，一起装进 `preview_overlay` 复合体（§3.1-D-2），不单独暴露。

---

## 3.3 风险项（五节，每节独立结论）

### 3.3-A 🔴 `StreamingEditing` 的真实 Win32 EDIT 子控件与 D2D 共存

**结论（明确，不留"实施时再看"）：D2D 迁移不改变父子绘制时序，无新增覆盖/闪烁风险；
EDIT 子控件的渲染路径与本批改动零交集。**

论证链（全部来自现行代码，非推断）：

1. **D2D 的输出终点与 GDI 相同**。`with_d2d:3099` 把 DC render target `BindDC` 到
   **父窗口 WM_PAINT 的内存 DC**（`mem_dc`，`:1959-1961`），EndDraw 后该帧仍是走既有的
   `BitBlt(mem_dc → 屏幕)`（`:2000-2003`）上屏。D2D 只替换「`mem_dc` 上怎么画」，
   **不替换「`mem_dc` 怎么上屏」**。合成方式零改动（这正是 DEC-057 三的边界）。
2. **EDIT 子控件是独立 HWND、独立表面**。它不在父窗口的 `mem_dc` 里，D2D 帧里
   根本不存在它的像素；它的文字/光标/选区由 `EDIT` 类自己的 wndproc 绘制
   （子类化 `:720-780` 只拦 WM_NCPAINT/Enter，其余全转发原 proc `:776`）。
   D2D 帧不可能"画到子控件上"。
3. **父帧覆盖子控件区域的既有行为不因 D2D 改变**。父窗口无 `WS_CLIPCHILDREN`
   （`:1160-1164`，`WS_POPUP` 裸样式），`BitBlt` 全客户区上屏时会覆盖子控件屏幕像素，
   随后依赖子控件自身重绘恢复——**这是 GDI 时代就存在的行为**。D2D 版帧的 `mem_dc`
   内容与 GDI 版逐位同构（chrome 覆盖同区域、同色），覆盖与恢复的时序、观感不变。
4. **颜色闭环**：即使发生瞬时覆盖，父帧在该区域画的是 chrome 底色 `#110F0D`
   （D2D `chrome:3350` = GDI `draw_overlay_chrome:2199` = EDIT 背景刷 `:668`
   `WM_CTLCOLOREDIT:1937` 的 `SetBkColor` 同值）→ 覆盖瞬间颜色与 EDIT 空白区背景一致，
   视觉上不可辨。
5. **D2D 失败回落不影响子控件**：`with_d2d` 返回 false 时走 GDI 兜底，帧内容同构
   （§3.3-C），子控件无感知。

**端测观察点（交给 Gavin，不是实施障碍）**：进入编辑态打字/移动光标时目视确认无新增
闪烁；若出现（理论上不应出现），回退动作 = StreamingEditing 单态退回 GDI
（一个 dispatch 分支回退，单 hunk，见 §3.4 顺序）。
另注：`[EDIT-CONTROL-NO-FONT-001]` 的字体问题已由 OVERLAY-054-D 的 `WM_SETFONT`
（`:698-705`）解决，与本批无关；该条目「region 圆角硬裁剪未处理」部分本批同样不碰
（per-pixel alpha 批次的既定议题）。

### 3.3-B 度量单一源（D2D-P1 裁定③ 的延续）

**结论：五态零度量需求，D2D 侧不新增任何 DirectWrite 度量调用，`measure_text_width`
（`:4289`）的全部现有调用方保持不变。**

逐态核对：

| 态 | 文字 | 度量需求 | 说明 |
| --- | --- | --- | --- |
| Recording / FallingToProcessing | 无文字 | 无 | 波形几何来自 audio_buf，与字体无关 |
| StreamingEditing | 正文=EDIT 自绘（GDI 字体） | 无（D2D 只画 chrome+submit） | 窗口宽度仍由 `adjust_overlay_pos_size_for_text:4241` 用 GDI 度量（`:4265`），该函数本批零改动；⏎ 是 DT_CENTER 自居中 |
| FocusLost | 标题/正文/标签 | 无 | 窗口尺寸固定 `PREVIEW_OVERLAY_SIZE [320,140]`（`:1034`），正文 DT_WORDBREAK 在固定矩形内由 DrawTextW/DWrite 各自断行，**不反馈到窗口尺寸** → 断行差异不产生几何漂移，只剩视觉差异（§3.5-U2） |
| Error | 单行错误文本 | 无 | 同上，固定 `STATUS_OVERLAY_SIZE [240,36]`（`:1032`） |

`streaming_text_format`/新 format 一律只用于 `DrawText`（画），从不用于测量。
已迁两态的 `streaming_text`（`:3584`）模式（宽度由调用方 GDI 度量传入）保持权威。

### 3.3-C GDI 兜底契约（每态必留，组织方式）

**组织方式（P1 定型模式的推广）**：

1. **调用点形状**：每个迁移态在 dispatch/GDI 函数内保持「d2d 优先、false 则 GDI 本体」
   的单一分流点（先例 `:2144-2157` Processing、`:2506-2508` idle）。
   GDI 绘制函数**本体一行不改逻辑**（仅 Recording/Falling 的波形纯逻辑抽共享纯函数，
   同值替换）。
2. **返回值分流**：D2D 成功路径不跑 GDI 绘制，hit rect 一律由「rect-only helper」出
   —— 既有 `draw_stop_button_hit_rect_only:2524` 复用；新增
   `draw_submit_button_hit_rect_only` 与 `preview_hit_rects`（§3.1-C/D）。
   helper 是唯一几何源，GDI 本体也改调它（同值替换），杜绝 P1「常态与回落各算各的」。
3. **元素级对齐清单（P1 右分隔线教训的制度化）**：P1 曾出现「D2D 常态缺右分隔线、
   GDI 回落才有」（后补 `:3663-3684`）。本批每个新复合体合并前必须过一张
   「GDI 元素 → D2D 元素」逐一对照表（本文 §3.1 各表的"处置"列即雏形，
   IMPL 单要求每态附最终版 + 行号），缺一项即打回。
4. **回落触发面不变**：init 失败 / BindDC 失败 / EndDraw 失败 / RECREATE_TARGET
   （`:3091-3137`）四条路径都归 `with_d2d` 返回 false，五个新包装不新增回落分支形态。
   兜底帧必须与 D2D 帧逐元素同构 → 观感上回落不可察觉（除已验收的 AA 差异）。

### 3.3-D per-pixel alpha 边界（DEC-057 三：本批绝对不碰）

**点名本方案不会动的东西**：

- 像素格式与 alpha 模式：`D2D1_PIXEL_FORMAT { DXGI_FORMAT_B8G8R8A8_UNORM,
  D2D1_ALPHA_MODE_IGNORE }`（`:3000-3003`）；
- 合成管线：`WS_EX_LAYERED`（`:1161`）、`UpdateLayeredWindow`/`SetLayeredWindowAttributes`
  相关一切调用、WM_PAINT 的 `mem_dc → BitBlt`（`:2000-2003`）；
- 窗口 region 调用（dispatch 内五处 `apply_overlay_window_region` 的参数原样）；
- `opacity` 字段与 `OverlayRequest` 的透明度链路；
- `D2D1_ALPHA_MODE_IGNORE` 下「不透明帧」的隐含契约（新原语一律画满帧或由
  WM_PAINT 预填充 `:1963-1978` 兜底，不引入半透明像素）。

per-pixel alpha 批次（DEC-056 ③，等八态全迁完）届时才把 `ALPHA_MODE_IGNORE` 换
premultiplied、重审每一态的角部像素。本批所有新原语**按不透明假设画**，不预埋半透明。

### 3.3-E `FallingToProcessing` / `Error` 与已迁 `Processing` 的关系

**结论：三个态视觉上并不相近到可以共用状态级复合体；共用会抹掉真实差异。
共用止步于元素级，且 `FallingToProcessing` 的正确共用对象是 `Recording`，不是 `Processing`。**

三态实际差异矩阵（逐项实测）：

| 维度 | Processing（已迁） | FallingToProcessing | Error |
| --- | --- | --- | --- |
| 底色 | `#181A18`（`:3171-3177`） | `#110F0D`（chrome） | `#211D1A`（`:4032`） |
| 圆角半径 | 16（`:3182`，region Some(16) `:2140`） | 10 | 10 |
| 边框 | 1px OVERLAY_BORDER_GRAY | 同左 | 同左 |
| 内容 | 渐变 shimmer 光带 + 居中橙字（YaHei SemiBold centered format） | 麦克风三态图标 + 32 条波形 + stop 键 | 红点 + 左对齐橙字 |
| 命中矩形 | 无 | cancel（stop 键） | 无 |
| 字体 format | `text_format`（YaHei，居中） | 无文字 | `streaming_text_format`（Segoe，左对齐）—— **与 Processing 不同源** |

- 若把 FallingToProcessing 并进 Processing 复合体：要么丢掉波形/图标/stop（行为变更），
  要么塞进条件分支（复合体变三态怪物，正是 DEC-055 红线 4 禁的一次全改式耦合）。
  正确做法是 §3.1-B：**FallingToProcessing = Recording 的 D2D 复合体原样复用**
  （两者 GDI 输出本就逐位相同），Processing 不参与。
- Error 与 Processing 只共享「底+边」骨架，但底色、半径、文本 format、内容全不同；
  共用的正确层次是 §0/H1 的 `chrome_with(bg, radius)` 参数化底+边原语，
  状态内容各自独立成复合体。
- 三态差异不会被抹掉的验证手段：§3.4 护栏 G1（各态独立包装函数 = 分流点独立），
  不存在三态共用入口的可能形态。

---

## 3.4 实施计划

### 3.4.1 hunk 级拆解（IMPL-109 验收单元）

| Hunk | 位置 | 内容 | 备注 |
| --- | --- | --- | --- |
| H1 | `mod d2d` | 抽 `chrome_with(res,w,h,bg:COLORREF,radius:f32)`；`chrome` 变一行包装（默认 `OVERLAY_BG_DARK`+10） | 已迁两态调用点零改动、行为零变化 |
| H2 | `mod d2d::D2dResources` + `create_resources` | 新增字段 `centered_text_format`（Segoe UI Normal 14, CENTER+PARA_CENTER+NO_WRAP）、`wrap_text_format`（同 face, LEADING+PARA_CENTER+**WORD_WRAPPING_WRAP**） | **零新 thread_local**；`release_resources:3079` 整槽 take/drop 自动覆盖 → D2D-HANG-095 关已过（护栏 `:9277/:9310` 回归即验） |
| H3 | `mod d2d` | 新原语 `waveform(res,w,h,state)`；根模块抽共享纯函数 `waveform_snapshot` + `waveform_bar_height`，GDI `:2348-2382/:2403-2416` 改调（同值替换） | OVERLAY-LOCK-SCOPE-001 语义固化在纯函数里 |
| H4 | `mod d2d` | 新原语 `submit_button(res,w,h) -> RECT`（无+1）+ `right_separator(res,w,h)` 小原语 | `draw_streaming_text_overlay:3663-3684` 内联右分隔线改调 `right_separator`（同值） |
| H5 | `mod d2d` | 复合体 `draw_recording_waveform_overlay(hdc,rect,state) -> bool`（chrome+mic+waveform+right_sep+stop） | 供 Recording 与 FallingToProcessing 共用（§3.1-B） |
| H6 | 根模块 | `draw_recording_overlay:2515-2518` else 分支：先试 `d2d::draw_recording_waveform_overlay`，成功返回 `draw_stop_button_hit_rect_only`；GDI 本体保持兜底 | Recording 态接线 |
| H7 | 根模块 dispatch `:2127-2132` | FallingToProcessing 分支：先试同一复合体，成功返回 hit-rect-only；GDI 三连调保持兜底 | FallingToProcessing 接线 |
| H8 | 根模块 | 新 helper `draw_submit_button_hit_rect_only(rect)`（无+1）；`draw_submit_button:2653-2658` 改调（同值） | StreamingEditing 命中几何单一源 |
| H9 | `mod d2d` + dispatch `:2159-2165` | 复合体 `draw_editing_overlay`（chrome_with 默认底 + submit_button）；dispatch 分支 d2d 优先，成功返回 helper rect | StreamingEditing 接线 |
| H10 | 根模块 + `mod d2d` | 抽纯函数 `preview_hit_rects(rect) -> (RECT,RECT,RECT)`；`draw_preview_overlay:3869-3968` 改调出返回值 | 命中矩形单一源先行（纯重构，可独立验收） |
| H11 | `mod d2d` | 复合体 `draw_preview_overlay`（chrome_with #211D1A + 标题/✕/分隔线/正文 wrap/双键）；dispatch `:2166-2173` d2d 优先，成功返回 `preview_hit_rects` | FocusLost 接线 |
| H12 | `mod d2d` + dispatch `:2174-2177` | 复合体 `draw_error_overlay`（chrome_with #211D1A + FillEllipse + streaming_text_format 文本）；dispatch d2d 优先 | Error 接线 |

每个 hunk 独立可 `cargo check`、独立可回退。测试模块零触碰（阶段三 tester-1 加护栏）。

### 3.4.2 分步顺序（批内落地顺序，与 hunk 对应）

**建议顺序：Error → StreamingEditing → FocusLost → Recording 波形 → FallingToProcessing。**

| 步 | hunk | 理由 |
| --- | --- | --- |
| ① | H1+H2+H12 | Error 最简单（无命中矩形、无交互、独立窗口态），一次打通 `chrome_with` 参数化与两个新 format 的基建，且立即验证「变体底色+region r10」组合 |
| ② | H8+H9 | StreamingEditing 第二：验证 hit-rect helper 模式与 submit 原语；EDIT 共存（§3.3-A，结论=无新风险）在此步的端测中专项确认，出问题回退面最小 |
| ③ | H10+H11 | FocusLost 第三：`preview_hit_rects` 纯重构先独立落地（可单独回归点击五元组），再接复合体；它是新绘制元素最多的态 |
| ④ | H3+H4+H5+H6 | Recording 波形第四：唯一高频态，放基建稳定之后；纯函数抽取+原语+接线一步到位 |
| ⑤ | H7 | FallingToProcessing 最后：零新代码（纯接线），且依赖④的复合体存在 |

理由归纳：**独立静态窗口态先于高频动画态；每个可端测的观感边界单独成步；纯接线放最后**。
任一步出问题只需回退该步 hunk（GDI 兜底全程在位，回退=删 d2d 分流一个点），
满足「出问题好回退」。

### 3.4.3 非回归点名清单（本批必须逐项确认不回退）

| 成果 | 不回退的判据 |
| --- | --- |
| OVERLAY-043 | 插值/needs_repaint 机制零触碰；绘制层改动不影响 `last_streaming_text` 门控 `:2119-2124` |
| OVERLAY-046 / 068-A / 068-B | 窗口定位/定尺寸体系零触碰（`overlay_geometry:4211`、`adjust_overlay_pos_size_for_text:4241`、`centered_x:4341`、`advance_width:4365` 均不在 hunk 清单） |
| OVERLAY-051-G | 时间轴揭示 `reveal_chars_by_timeline:4158` 与 `displayed_chars` 消费 `:2108` 零触碰 |
| OVERLAY-075 / 086 | 圆角填充+region 匹配教训在新原语中执行（fill/stroke 共半径绑定）；region 参数原样 |
| D2D-P0 | `draw_processing_primitives:3163` 及其包装 `:3149` 零触碰（H1 只动 `chrome`，不动 P0 本体） |
| D2D-P1 | `chrome/mic_indicator/stop_button/placeholder_text/streaming_text` 几何零变化（H3/H4 对 `streaming_text_overlay` 的唯一触点是右分隔线内联→原语的同值替换）；两态回落路径不变 |
| OVERLAY-101 | `centered_x` 单一居中源零触碰；本批无任何居中计算 |
| OVERLAY-102 | `STREAMING_OVERLAY_MAX_SCREEN_RATIO=0.50:1052` 零触碰 |
| D2D-HANG-095 | **零新 thread_local**（H2 只加结构体字段）；`release_resources`/`D2dReleaseGuard:1080` 零触碰；护栏 `:9277/:9310` 必须仍绿 |
| 命中矩形契约 | 五元组语义逐位不变：Recording/Falling=cancel(stop, +1)；StreamingEditing=submit(**无**+1)；FocusLost=copy→cancel/close/title_close；Error=全 None（§3.1 各节） |
| OVERLAY-LOCK-SCOPE-001 | 波形快照锁内 decay/锁外绘制语义固化进 `waveform_snapshot` |
| WAVEFORM-HEIGHT-FIX-001 / MIC-ICON-ENLARGE-001 / STOP-BUTTON-CENTER-FIX-001 / UI-TUNE-001 | 波形/图标/stop 常量逐位照抄（§3.2-A） |
| OVERLAY-054-C/F/G/H | 颜色统一走 `OVERLAY_BORDER_GRAY` 等 file-level 常量；EDIT 度量与文字 inset 常量零触碰 |
| FIX-006 系列 | 预览窗按钮 45x18/gap10/r8 边、✕ 键 18x18、双色标签逐位照抄（§3.2-C） |
| ASR-038-C / OVERLAY-051-B/D | EDIT 子控件生命周期（创建/销毁/子类化/WM_SETFONT）零触碰 |

### 3.4.4 建议护栏清单（交 tester-1，阶段三）

**可真单测（有判别力）**：

1. **G1 五个回落触发测试**：镜像 `d2d_processing_returns_false_on_invalid_hdc_gdi_fallback_trigger:8863`
   与 `d2d_streaming_two_entries_return_false_on_invalid_hdc_gdi_fallback_trigger:8904` 的模式，
   为 `draw_editing_overlay` / `draw_preview_overlay`(d2d) / `draw_error_overlay`(d2d) /
   `draw_recording_waveform_overlay` 各写 invalid-hdc → false 断言（Falling 与 Recording 共用
   同一包装，无需单列）。判别力：包装函数存在性 + `with_d2d` 接线正确性 + 资源构建路径。
2. **G2 命中矩形等值断言**（镜像 `right_separator_geometry_matches_between_gdi_and_d2d:9220`）：
   - `draw_submit_button_hit_rect_only` 输出 == 旧 `draw_submit_button` 内联公式（含**无+1** 的负向断言：`right != left+bs+1`，防"顺手统一"回归）；
   - `draw_stop_button_hit_rect_only` 与 `d2d::stop_button` 返回值同公式（既有契约加固）；
   - `preview_hit_rects` 对固定输入 rect 的三 rect 真值表（`btn_w=45/gap=10/居中` 逐位）。
3. **G3 波形纯函数**：`waveform_bar_height` 公式表（v=0 → static_h=12；v_gain>0.01 边界；
   weight 用 i=0 与 i=half-1 两个端点值断言）+ `waveform_snapshot` 在空 buf/poisoned lock 下
   返回空 vec 的契约。判别力：把 §3.2-A 的常量与公式钉死，GDI/D2D 共用后任何单侧擅改即红。
4. **G4 include_str! 结构护栏**（比照 G5 先例 `logs/20260905.md`）：断言 dispatch 五分支
   均含 `d2d::draw_` 前缀调用且其下仍保留 GDI 函数调用（回落在位的静态证据）。
   正则易脆，tester-1 评估后可提异议。
5. **G5 既有 D2D-HANG-095 护栏回归**（`main.rs:9277/:9310`）：H2 加字段后必须仍绿——
   这是「新增 D2D 资源过 D2D-HANG 关」的机器判据。不需新测试，列入必跑。

**只能靠 Gavin 端测目视（DEC-055 红线 5，如实标注、不凑数）**：

- 波形观感/流畅度、三态图标颜色切换（红/橙/灰）——像素级渲染对比测试在本管线
  （GDI 内存位图 vs GPU 光栅）不可行，**判别力不足，不设伪单测**；
- StreamingEditing 的 EDIT 共存（打字/光标移动/选区时的闪烁）——§3.3-A 结论为无新风险，
  但仍需端测实证；
- FocusLost 正文断行观感（GDI vs DWrite 断词差异，§3.5-U2）；
- Error 文本溢出时的表现（截断 vs 省略号，§3.5-U1 裁决项）;
- ⏎/✕ 字形与垂直居中的细微差（§3.5-U3）；
- 各态圆角边缘（region 裁剪最外 1px 的硬边，OVERLAY-086 既定取舍的延续观感）。

---

## 3.5 不确定项（待定 + 需要什么信息才能定）

| # | 不确定项 | 现状方案倾向 | 定案所需信息 |
| --- | --- | --- | --- |
| U1 | **Error 文本 `DT_END_ELLIPSIS`**：D2D `DrawText` + NO_WRAP 只会硬裁剪，无内建省略号。DWrite trimming 需 `IDWriteTextLayout` + trimming sign，属新增代码面 | 接受 NO_WRAP 裁剪：错误文案全是友好短语（`convert_to_friendly_error:4378` 的固定短句），`STATUS_OVERLAY_SIZE` 240px 宽，实际溢出概率极低；GDI 兜底仍有省略号，回落观感差仅出现在本已异常的帧 | Gavin 裁决「错误文本溢出时接受截断否」；若不接受，IMPL 单为 Error 加 layout trimming（+1 hunk，仅此态） |
| U2 | **FocusLost 正文断行差异**：GDI `DrawTextW(DT_WORDBREAK)` 与 DWrite `WORD_WRAPPING_WRAP` 断词算法不同，长英文段可能差一两个词换行位置 | 接受差异（固定窗口、不反馈几何，纯观感） | Gavin 端测目视；若不可接受，正文改用 GDI 画（该态正文单元素退回 GDI = 局部混合帧，需主控裁定是否开此先例） |
| U3 | **⏎/✕ 字形**：U+23CE/U+2715 在 DWrite/Segoe UI 的字形可用性与基线位置、`DT_VCENTER` vs `DWRITE_PARAGRAPH_ALIGNMENT_CENTER` 的垂直居中细微差 | 同 face（Segoe UI）理论一致；GDI 侧现用同一字体渲染正常 → DWrite 缺字形自动 fallback 到同引擎 | Gavin 端测目视 ⏎ 与 ✕ 的位置/粗细；异常时微调 format（基线偏移）属实施细节 |
| U4 | **D2D 帧时长变化对 EDIT 子控件时序的影响**：§3.3-A 结论是合成管线不变，但 D2D 光栅化耗时与 GDI 不同，父帧 BitBlt 时刻的微小漂移理论上改变覆盖-恢复窗口的观感 | 风险评级极低（覆盖内容同色闭环）；回退预案=单态回 GDI 单 hunk | Gavin 端测编辑态专项观察（§3.4.4 端测清单） |
| U5 | **死代码 `draw_editing_overlay_chrome:2809`**：全仓唯一出现即定义处（grep 计数=1），无任何调用方。它与现行 StreamingEditing 绘制路径（chrome+submit 两键）不同源（它是 cancel+submit 双键旧版） | IMPL-109 顺带删除（死代码留着会误导后续迁移者以为编辑态有两键） | 主控拍板：删 or 留（删除零行为影响，纯卫生债） |
| U6 | **FocusLost/Error 底色 `#211D1A` ≠ WM_PAINT 预填充 `#110F0D`**：region r10 裁掉角部后无外露，但若将来有人改 region 参数（如 Some(10)→None），角部会露出色差 | 记录在案，本批不改；U6 与 per-pixel alpha 批次的角部像素重审合并处理 | 无需现在定，防未来回归的备忘 |

---

## 附：本方案未覆盖 / 明确不做

- per-pixel alpha 与合成方式（§3.3-D 点名清单）；
- `[EDIT-CONTROL-NO-FONT-001]` 遗留的 region 圆角硬裁剪问题（troubleshooting 已标「另行排期」）；
- macOS 侧对应态（DEC-033 分工边界）；
- 任何窗口尺寸/居中/插值逻辑（OVERLAY-043/046/068/086/101/102 体系）。

---

## 附录 · 主控裁决（orchestrator，2026-09-05，方案冻结）

本方案 **验收通过**，两个协商点裁决如下。IMPL-109 以本附录为准。

### 裁决一：`FallingToProcessing` 复用 `Recording` 的 D2D 复合体 —— **采纳 coder-2 方案**

主控独立取证（`sed -n` 逐处核对，非采信报告）：

| 分支 | 元素序列 |
| --- | --- |
| `OverlayStatus::Recording`（`:2070-2079`） | `apply_overlay_window_region(hwnd, rect, None, true)` → `draw_recording_overlay(..., show_placeholder=false, ...)`（`:2495`）＝ `draw_overlay_chrome` → `draw_recording_indicator_and_waveform` → `draw_stop_button` |
| `OverlayStatus::FallingToProcessing`（`:2127-2131`） | `apply_overlay_window_region(hwnd, rect, None, true)` → `draw_overlay_chrome` → `draw_recording_indicator_and_waveform` → `draw_stop_button` |

**两者逐位一致**（含 region 参数）。唯一差异是动画驱动方（消息循环 `:1660` 的 gravity 衰减），
与绘制无关。

🔴 **派发任务书的预设是错的**：主控按「状态名相近」把 `FallingToProcessing` 归到
`Processing` 一组问「三态是否可共用」，coder-2 按**实际调用链**归到 `Recording`，是对的。
**以 coder-2 的方案为准**：Falling 原样复用 Recording 复合体，`Processing` 不参与。

### 裁决二：`draw_editing_overlay_chrome`（`:2809`）死代码 —— **本批不删**

coder-2 的判断成立，主控 `cargo check --all-targets` 复现编译器原话：
`warning: function draw_editing_overlay_chrome is never used`（`src\main.rs:2809:4`），
全文件仅一处 grep 命中＝定义体本身，无调用方。

**但本批不动它**：删死代码是卫生债，与 D2D 迁移零关系；混进迁移批会污染归因
（DEC-055 红线 4 精神）。已记入 `collab/todo.md` 卫生债单独处理。
**IMPL-109 对它的口径：不删、不改、不参考。**

### 行号抽查结果

主控 `sed -n` 逐处核对以下引用，**全部对上**：
`:2809`（死函数定义）／`:2229`（`draw_recording_indicator_and_waveform`）／
`:2447`（`draw_stop_button`）／`:2646`（`draw_submit_button`）／`:3099`（`with_d2d`）／
`:4237`（`overlay_max_width` 算式）／`:2524`（`draw_stop_button_hit_rect_only`）／
`:2001`（`BitBlt` 上屏）／`:1161`（`WS_EX_LAYERED`）／`:3000-3003`（`ALPHA_MODE_IGNORE`）。

### 存放位置变更（主控处置）

原交付路径 `collab/drafts/d2d-p2p3-plan.md` 命中 `.gitignore:72` 的 `/collab/drafts/`，
**不会入库** —— 而本文是 IMPL-109 的实施与验收依据，属协作契约级文档，丢失代价高。
故移至 `docs/D2D-P2P3-PLAN.md`（tracked）并提交。与 `.gitignore` 该段注释的本意
（不入库的是研究语料，「协作契约继续入库不变」）一致。

### 冻结状态

方案冻结。实施单 `D2D-P2P3-IMPL-109` 在 **TEST-EXEC-106 → BUILD-107 出包 → Gavin 端测**
走完之后才派发（`src/main.rs` 当前归 tester-1，五阶段禁止并行）。
