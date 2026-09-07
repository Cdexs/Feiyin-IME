# INVESTIGATE-142 · 编辑态移光标仍闪 · 二次取证（🔴 只查不修）

负责人：coder-1 ｜ 2026-09-07 ｜ 方法论约束：`[PLAUSIBLE-FIX-NOT-ACTUAL-CAUSE-001]` + `[STATIC-PROOF-MISSED-CALLGATE-001]`

> ⚠️ 行号口径：本单取证期间 `src/main.rs` 正被 coder-2 并行修改（OVERLAY-141-IMPL，文件在读取期间从 11669 行增长到 11727 行）。
> 下文行号是 **2026-09-07 读取时刻**的快照，验收时请以**符号名锚点**为准，行号允许 ±漂移。
> 本人全程对 `src/**` 零写入；工作区既有 `M src/main.rs` / `M src/llm/mod.rs` 属在途任务产物，与本单无关（任务书第五节预告）。

---

## 0. 结论先行

1. **编辑稳态下（即光标移动发生的时刻），应用层不存在任何可达的重绘触发器。**
   主控怀疑的「图层模式切换」与 INVESTIGATE-120 的旧根因「迟到流式包」两条路径，均被
   调用门控静态证伪（§2 候选 1、候选 2）——编辑期间**不可能**有 Show/模式切换/SetWindowPos 风暴到达。
2. **唯一与光标移动相关联的绘制活动**：EDIT 子控件自身的重绘（caret 移动的局部重绘；
   文本超宽时 `ES_AUTOHSCROLL` 的整块滚动重绘）+ 系统 caret 绘制 + DWM 对 SLWA 分层窗的重合成。
   这是头号在案候选，但按方法论纪律，**静态只能证明它是唯一发生的事，不能证明它是造成可见闪烁的事**
   —— 定性必须靠 §5 实测。**本单结论：根因未定，需实测。**
3. **附带发现一个结构性放大器（新，前两次取证均未记录）**：父窗 WM_PAINT 在 SLWA 模式下是
   **全窗 BitBlt 直打屏幕**，且父窗**无 WS_CLIPCHILDREN**（类样式 0 `:1172`、窗口 `WS_POPUP` `:1221`）。
   一旦父窗因任何原因重绘，EDIT 子控件的文字会被背景色**整块盖掉**，且 EDIT 不知情、不会重绘，
   文字要等下次 caret 闪烁（~500ms）或下次输入才回来 —— 这正是「窗口和文字闪」的机制载体。
   静态审计：编辑稳态下没有应用级触发器会引发父窗重绘（§1-E），但它是否被系统侧路径触发，
   恰好是实测 P2 要回答的问题。
4. 对任务书主控怀疑的直接回答：**P2 图层模式切换「能解释进入编辑瞬间的一次性闪烁」，
   不能解释「光标移动持续闪」**。FLICKER-130 **没有判错**（它修掉的是真实存在的整窗 Show 风暴源），
   症状残留符合**多源叠加**模型：光标移动闪烁来自另一个一直存在、此前被 Show 风暴的更剧烈症状掩盖的源。

---

## 1. 证据链（静态实证，含调用门控）

### A. 图层模式切换的可达性（候选 1 判据）

- `switch_overlay_layered_mode`（`:594`）首行早退：`if state.layered_mode == target { return; }`（`:599-601`）。
- 全库调用点恰 2 处，且**两处都必须模式实际不同才动作**：
  - `EnterEditMode` 臂 `:1549`（Ulw→Slwa，编辑入口一次性，且已按 P2 设计藏进 SW_HIDE 隐藏区间 `:1528-1530`）；
  - `Show` 处理臂 `:1359-1364`（`state.layered_mode != target_mode` 才先 SW_HIDE 再切换）。
- 编辑稳态期间窗口模式恒为 Slwa，任何后续事件想切也是切回 Slwa → 早退 → **零动作**。

### B. 编辑期间 Show(StreamingEditing) 的可达性（候选 2 判据）

- `OverlayCommand::Show` 全库发送点穷举（10 处）：`:5369/:5389`（录音开始/停止类）、`:5528`（托盘/设置）、
  `:5571`（streaming idle 占位）、`:5686`（StreamingText 的 editing 分支）、`:5751/:5773/:5795`（FocusLost/Error/Info，
  均先 `OVERLAY_EDITING.store(false)` 即退出编辑态的转移）、`:5908`（SubmitRequested 提交后的 fallback，提交已完成）。
- **编辑稳态期间唯一理论可达的是 `:5686`**，但它被 FLICKER-130 门控挡死：
  `should_ignore_streaming_text(stopped)`（`:5680-5682`）在 `stopped=true` 时丢弃。
- `editing ⇒ stopped` 不变量：`EditRequested` 臂内 `OVERLAY_EDITING.store(true)` 与
  `STREAMING_STOPPED.store(true)` 同线程相邻 store（`:5824/:5829`），且已被 F3 结构护栏
  机器钉死（`:11450-11472`，共现断言）。⇒ `(editing=true, stopped=false)` 组合不可达，
  `:5653` 的 `else if editing` 分支在编辑稳态**永不执行**。
- 同理，Show 处理臂的无条件 `destroy_edit_control`（`:1366`）在编辑稳态同样不可达。

### C. 尺寸插值循环的可达性（候选 5 判据）

- 循环条件 `state.current_size != state.target_size`（`:1835`）。
- `EnterEditMode` 臂内**同值赋值**：`state.target_size = new_size; state.current_size = new_size;`（`:1577-1578`）
  ⇒ 进入编辑后循环条件恒假，循环休眠，**零 SetWindowPos / 零 InvalidateRect**。

### D. 光标移动本身不经过任何应用逻辑（候选 4 的发生性证明）

- EDIT 获得焦点：`SetFocus(edit_hwnd)`（`:1590`）—— 箭头键直达 EDIT 原生窗口过程。
- EDIT 子类过程只拦截两种消息：`WM_NCPAINT`（去边框）与 `WM_KEYDOWN VK_RETURN`（提交），
  其余全部 `CallWindowProcW` 透传原配（`:757-807`）—— 箭头键、caret 移动、滚动全部在
  EDIT 控件内部消化，**应用层无任何代码感知光标移动**。
- 父窗 `overlay_wnd_proc` 无 `WM_COMMAND` 臂（`WM_COMMAND` 仅出现在 import 表），EDIT 的
  EN_CHANGE/EN_UPDATE 通知不被应用处理。
- per-frame 主循环（`MsgWaitForMultipleObjects` 16ms 节拍 `:1887-1897`）中 StreamingEditing 臂
  （`:1743-1752`）仅在 `needs_repaint` 时 InvalidateRect；`needs_repaint` 全部 setter 穷举
  （`:1199/:1440/:1521/:1573/:1700/:1706/:1714/:1730/:1784/:1839/:2370/:2146`）——编辑稳态下：
  Show 不可达（B）、UpdateWordTimings 不可达（B 同门控）、插值循环休眠（C）、
  `:2370` 的 text_changed 检测属 RecordingWithText 渲染路径不经过编辑态
  ⇒ **编辑稳态 needs_repaint 恒 false ⇒ 恒零 InvalidateRect**。

### E. 结构性放大器的本体（候选 3 判据）

- SLWA 模式（编辑态）的父窗 WM_PAINT（`:2039-2182`）：`BeginPaint` → 内存 DC 全窗重绘
  （含背景 FillRect `:2106-2119`）→ `BitBlt(hdc, ... SRCCOPY)` **一次性覆盖整个客户区**（`:2172-2174`）。
- 父窗无 `WS_CLIPCHILDREN`（`:1172` 类样式 0；`:1221` `WS_POPUP`）⇒ BitBlt 会把 EDIT 子控件
  区域连同文字一起盖成背景色；子控件对父窗的直接屏幕写**不知情**，不触发自身重绘。
- EDIT 背景色 == 父窗背景色（`OVERLAY_BG_DARK`，`WM_CTLCOLOREDIT` 返回同色刷 `:2028-2037`）
  ⇒ 盖掉后视觉上「底板无缝」，**唯独文字消失**，直到 EDIT 下次重绘（caret 闪烁周期或下次输入）。
- ULW 模式（其余七态）经 `UpdateLayeredWindow` 整面替换、无子控件，不存在此问题
  —— 这解释了为什么全库只有编辑态暴露此结构。

---

## 2. 候选源清单（穷举，逐条判据，不许留空）

| # | 候选源 | 判据 | 证据类型 | 结论 |
| --- | --- | --- | --- | --- |
| 1 | 图层模式切换（主控怀疑，WS_EX_LAYERED 清/置重建 redirection 表面） | 调用点恰 2 + 模式相同早退 + 编辑稳态模式恒 Slwa（§1-A）；且设计上切换已藏进隐藏区间 | **静态实证** | **不成立**（对「光标移动闪」）；只能解释进入编辑瞬间的一次性闪，且该瞬间已按设计隐藏 |
| 2 | 迟到流式包 → Show 风暴（INVESTIGATE-120 旧根因） | 唯一发送点 `:5686` 被 stopped 门控 + `editing⇒stopped` 不变量（F3 护栏）双挡（§1-B） | 静态实证（门控唯一性）；门控**运行时生效**需实测复核 | **静态不成立**；保留实测 P1 做运行时复核（若 log 出现丢包记录或 EDIT 重建记录即翻案） |
| 3 | 父窗 WM_PAINT 全窗 BitBlt 盖掉 EDIT（无 WS_CLIPCHILDREN） | 结构本身确凿（§1-E）；编辑稳态应用级触发器为零（§1-D），系统级触发器未知 | 结构=静态实证；触发器=**需运行时实测** | **机制成立、是否发生未定** → 实测 P2 定性 |
| 4 | EDIT 子控件自身重绘（caret 局部重绘 / ES_AUTOHSCROLL 滚动整块重绘）与 DWM 对 SLWA 窗的重合成时序 | 光标移动唯一相关联的绘制活动（§1-D 证明其余全为零）；子控件重绘在 SLWA 分层窗上走 redirection 表面 + DWM 重合成，时序错位即可见闪烁；**平台层无文档定论**（已检索 MSDN/Stack Overflow，SLWA+子控件闪烁无权威条目） | 发生性=静态实证；**可见性=需运行时实测** | **头号在案候选，未定** → 实测 P3 + 录屏帧分析 |
| 5 | 尺寸插值循环 SetWindowPos（INVESTIGATE-120 旧嫌疑 / OVERLAY-101 同源） | EnterEditMode 同值赋值 ⇒ 循环条件恒假（§1-C） | **静态实证** | **不成立**（编辑稳态）；仅进入编辑瞬间理论可达数帧，与「持续闪」不符 |
| 6 | caret 在 SLWA 分层窗上的系统级行为（caret 画在 redirection 表面之外/之内、闪烁重绘波及面） | 无文档定论，无法静态判定 | **需运行时实测** | 与候选 4 同场实测（录屏逐帧 + P3 计数） |

---

## 3. 为什么 FLICKER-130 修完还在（验收标准 3）

**多源叠加，不是判错，也不是修法没覆盖某条路径的 bug：**

- FLICKER-130 修掉的源（迟到包→`Show(StreamingEditing)`→无条件 `destroy_edit_control`+
  重算尺寸→整窗 Show 循环）**真实存在且是当时最剧烈的源**，门控+不变量至今有效（§1-B）。
- 「光标移动闪」来自另一个**一直存在**的源（候选 3/4 的子控件绘制与父窗结构层）。
  在 FLICKER-130 之前，Show 风暴的剧烈闪烁掩盖了它；风暴修掉后，较轻的源显露出来 ——
  这正是 `[PLAUSIBLE-FIX-NOT-ACTUAL-CAUSE-001]` 描述的经典形态：修掉一个能解释症状的源 ≠
  症状只剩一个源。
- 佐证：症状的可感知位置变了 —— 旧症状随 ASR 尾包节奏抖动，新症状精确随光标操作出现，
  两者触发器不同，支持「两个独立源」模型。

## 4. 修法建议（可多选一；🔴 本单不实施）

| 方案 | 内容 | 适用前提（由 §5 实测判定） | 代价与风险 |
| --- | --- | --- | --- |
| M1 | 父窗创建样式加 `WS_CLIPCHILDREN` | P2 实测父窗 WM_PAINT 参与闪烁 | 一行改动；风险：WS_CLIPCHILDREN 与 SLWA 分层窗、D2D BindDC 路径的兼容性需 PoC；且七态 ULW 路径共用同一 wndproc，需确认 ULW 模式零回归（ULW 整面替换本就不受子控件裁剪影响，理论安全） |
| M2 | 编辑态退出 SLWA：退回纯 ULW + EDIT 文字父窗自绘（= OVERLAY-121-PLAN-127 已否决的方案 A 复活）或 WM_PRINTCLIENT 桥接（方案 E 备选） | P3 实测确认闪烁在 SLWA+子控件合成层 | 自绘方案成本高（丢失原生 EDIT 输入法/选择/滚动能力）；方案 E 保留 EDIT 但绘制链复杂，需 PoC |
| M3 | 去掉 `ES_AUTOHSCROLL` 或对 EDIT 做重绘节流 | P3 实测显示闪烁集中在横向滚动重绘 | 丢「长文本随光标滚动」行为，需产品侧确认；节流治标 |
| M0 | 不动代码，先录屏 240fps 逐帧定位闪的是「整窗」还是「文字区」还是「caret 本身」 | 任何情况 | 零风险，建议作为实测第一步（与 §5 合并执行） |

## 5. 最小实测方案（探针位置精确到行；全部交付前撤除，`git diff src/` 必须为空）

**采集动作**：进编辑态（录音→停→进编辑），输入长文本使超出宽度，然后左右光标移动 30s（含按住不放），
再采 30s 静止（对照）。全程 `debug.log` + 240fps 屏幕录制（OBS）。

| 探针 | 位置 | 采什么 | 预期区分度 |
| --- | --- | --- | --- |
| P1 门控复核 | 现有日志即可，不改码：grep `ignoring late StreamingText after stop`（`:5683`）与 `created EDIT control`（`:738`）在编辑时段的出现次数 | 迟到包是否仍在到达 / EDIT 是否被重建 | 两者均 0 ⇒ 门控生效、旧根因排除坐实；任一 >0 ⇒ 翻案回候选 2 |
| P2 父窗 WM_PAINT 计数 | `overlay_wnd_proc` WM_PAINT 臂入口（`:2039`）加 `log::info!("PAINT mode={:?}", layered_mode)` | 编辑稳态父窗重绘次数 | 与按键强相关 ⇒ 根因链含父窗重绘（触发器顺 InvalidateRect/系统路径续追），修 M1；恒 0 ⇒ 闪烁纯在子控件/合成层，排除 M1 |
| P3 EDIT 重绘计数 | `edit_subclass_wnd_proc` 入口（`:751`）对 WM_PAINT / WM_ERASEBKGND 分别计数 | 每次光标移动的重绘次数与配对 | 重绘次数与按键强相关且 ERASE+PAINT 配对 ⇒ 滚动/局部重绘是唯一活动，结合录屏定可见性；≈0 ⇒ 转 caret/系统层 |
| P4 切换计数 | `switch_overlay_layered_mode` 入口（`:594`）加 log | 编辑全程切换次数 | 预期恒 0；>0 直接翻案候选 1 |
| P5 插值循环 | `:1835` 条件真时 log | 编辑态循环激活帧数 | 预期恒 0 |

**判读方式预先声明**（`[PLAUSIBLE-FIX-NOT-ACTUAL-CAUSE-001]` 纪律，防事后任意解释）：

- P2>0 且与按键同步 → 根因链 = 父窗重绘（候选 3），修法 M1，并在修复单中先修触发器再验视觉；
- P2=0 且 P3 与按键强相关 + 录屏显示闪烁限于文字区 → 根因 = 候选 4（SLWA+子控件合成），修法 M2/M3；
- P2=0、P3≈0 但肉眼仍闪 → 根因 = 候选 6（caret/系统层），以录屏逐帧定性后另立方案；
- P1/P4/P5 任一非零 → 对应候选翻案，以实测数据为准重新定界。

## 6. macOS 适用性

不适用 —— 编辑态整链（Win32 EDIT 子控件、SLWA/ULW 双模式、WS_EX_LAYERED、BitBlt 提交）全部
`cfg(windows)` 内；macOS NSWindow 原生支持逐像素透明与子视图合成，不存在该机制对位物。
