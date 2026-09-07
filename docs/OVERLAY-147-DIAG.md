# OVERLAY-147-DIAG · BUILD-145 端测两问诊断（coder-2，2026-09-07，🔴 只查不修）

负责人：coder-2 ｜ 诊断单，生产代码零改动（`git diff --numstat -- src/` 空）
证据类型标注：【数学】= 独立 rustc 探针程序实测（已删除）／【静态】= 代码取证（`文件:行号`）／【运行时】= 待实测

---

## 0. 结论先行

| 问 | 结论 |
| --- | --- |
| A-1「SDF 对不对」 | **对**。【数学】SDF 覆盖率与 64×64 超采样精确面积覆盖率全位比对：240×36 r=16 平均偏差 **0.0002**、最差 **0.041**；r=10 平均 **0.0001**、最差 **0.032**。四角 cov 剖面为干净单调坡（见 §A3 数据）。**「掩码算错」整片候选被砍掉。** |
| A-2「H1a（fill/stroke 圆心错位）」 | **推翻作为分界解释**。【数学】错位导致的描边带深度偏差 **r 无关**（r=16 实测 [-0.008, +1.207]px，r=10 [-0.012, +1.207]px——逐值相同），角部 45° 处描边外沿**仍在轮廓内**（深 0.21px），不存在「外沿探出轮廓」被 SDF 切出的机制。主控「stroke radius 应为 r-0.5 才同心」的**修法本身是对的**（修后描边外沿恰在轮廓上），但它修的是 ~0.2px 的外观瑕疵，**解释不了 r16/r10 分界**。 |
| A-3「H2（持续重绘）」 | **被静态证据推翻（待 Gavin 一句话确认）**。`RecordingStreamingIdle` = r=16 + **静态**（`:1680` 仅 `needs_repaint` 时重绘；进入后仅 Show/尺寸插值置脏，音频数据不置脏）+ 端测报仍坏。若 Gavin 确认看到的是占位文本变体（无波形动画），H2 死，分界只剩 {半径值, 底色} 两个变量。【静态】`:1678-1686` |
| A-4「r=16 为什么仍坏」 | **静态+数学已穷尽，机制未定**。已证：SDF 正确（A-1）、H1a 不是分界原因（A-2）、H2 不是（A-3）、窗口高度混淆死（`STATUS_OVERLAY_SIZE == RECORDING_OVERLAY_SIZE == 240×36`，`:1086/:1088`）、轮廓区内容结构两组相同（只有 chrome fill+1px stroke；mic/波形/文本/按钮全部内缩；Processing 光晕已裁剪到圆角矩形 `:3656-3668`）。**剩余候选 = 半径值本身经某条未测的运行时路径起作用，或底色差异（0x110F0D vs 0x211D1A）。需要 §A5 的 E1-E3 运行时实验定案。** |
| B「录音窗显示光标」 | **主假设 = 线程级 caret 对象在 EDIT 销毁后泄漏**（EDIT 本体不可能存活到录音帧，静态证死，见 §B2）；EDIT 生命周期另钉死一个**可达的卡死缺陷**（热键停止臂缺 OVERLAY_EDITING 守卫，§B3-②）与一个**双重压制卡窗缺陷**（§B3-③）。FLICKER-130 与 EDIT 泄漏**无关**（被丢弃的 Show 本来就不是清理路径，反而是历史上销毁 EDIT 的肇事路径，§B4）。 |

---

## Part A · r=16 三态圆角仍坏

### A1. 端测事实与本单要解释的分界

（沿用任务书 A1 表：r=16 五态仍坏 / r=10 三态好，零反例。）

### A2. H1/H2 混淆变量 —— 静态证据已把 H2 砍到只剩一个 runtime 确认

重绘触发器全映射【静态】（`src/main.rs`，行号为 09-07 快照，以符号锚点为准）：

| 状态 | 重绘触发 | 证据 | 端测结果 |
| --- | --- | --- | --- |
| `Recording` | **每循环 InvalidateRect**（波形动画） | `:1670-1677` | 坏 |
| `RecordingStreamingIdle` | **仅 needs_repaint**（Show/尺寸插值置脏；音频更新不置脏）→ **静态** | `:1678-1686` | **坏** |
| `RecordingWithText` | 文字推进时脏 | `:1687-1742` | 坏 |
| `FallingToProcessing` | 重力衰减每帧直至 settle | `:1753-1794` | 坏 |
| `Processing` | **每循环**（shimmer） | `:1795-1802` | 坏 |
| `FocusLost` | 从不重绘（注释明言防闪烁） | `:1803-1810` | 好 |
| `Error` / `Info` | 从不重绘 | `:1811-1816` | 好 |

⇒ `RecordingStreamingIdle` 是**天然的 H2 反例**：r=16 + 静态 + 端测报坏。
**判读注意（给 Gavin 的确认问题，零成本）**：Idle 与 Recording 同为 240×36，区别是 Idle 画
「正在聆听」占位文本、无波形动画。请确认端测看到坏圆角的那一屏是占位文本变体。
- 若确认 ⇒ H2 死，分界变量收缩为 **{半径值, 底色}**（内容结构同构见 §A4）。
- 若 Idle 其实没单独看过（与 Recording 混看）⇒ H2 未被排除，需 E1/E2 判别。

### A3. 纯数学探针结果（任务书强制交付项）

独立 rustc 程序复刻 `apply_alpha_fixup`（`main.rs:2243`）的 SDF，与 64×64 超采样精确
圆角矩形面积覆盖率逐像素比对（240×36，r=16 与 r=10 各跑全幅 8640 像素）：

```
r=16: mean|Δ| = 0.0002   worst 6 处全在四角弧上，最大 |Δ|=0.041（sdf=0.237 vs 精确 0.196）
r=10: mean|Δ| = 0.0001   worst |Δ|=0.032
sdf=0 但面积>0 的像素：r16 8 个 / r10 12 个（均为弧上极端角点，Δ<0.04）
```

四角 cov 剖面（左上角 24×24，数字 = cov×9，0=全透明 9=全不透明）——干净单调坡，无平台/断崖：

```
r=16 左上角（y00 行）：000000000003578999999999 → 角点全透明，x≈12 起进入 AA 坡，x≥16 全覆盖
r=10 左上角（y00 行）：000000368999999999999999 → 同构，AA 坡更窄
左缘中点（y=18）：x=0..5 全部 cov=1.000（两组一致）
```

**结论：SDF 与「理想圆角矩形 1px 解析抗锯齿」逐位等价（偏差 ≤0.04 = 亚像素锯齿不可见级）。**
掩码几何不是病灶。

### A4. H1a 复核（主控欢迎推翻，如实推翻）+ 内容同构性

**H1a 几何复核**【静态+数学】：fill 圆弧圆心 (r, r)（rect 0,0,w,h）；stroke 圆弧圆心
(r+0.5, r+0.5)（rect 0.5,0.5,w-0.5,h-0.5，线宽 1.0）。二者圆心差 (0.5,0.5)（对角 0.707px）属实，
主控对几何的观察**正确**；但推算偏差量级：

```
描边带（圆心距 r±0.5）相对 fill 轮廓的深度范围（沿圆弧采样 90°×16 细分）：
  r=16: [-0.008, +1.207] px
  r=10: [-0.012, +1.207] px
  （正值=轮廓内侧；0=轮廓线上）
```

- 描边带**从不探出轮廓**（最深 -0.012px = 采样舍入噪声）——不存在「描边超出 SDF 覆盖区被切出硬边」的机制；
- 角部 45° 处描边相对直边（深度 [0,1]）**内缩最多 ~0.2px**，该偏差 **r 无关**；
- ⇒ **H1a 的量级（亚 0.5px、均匀两组相同）解释不了 r16/r10 零反例分界，判为不成立（作为分界解释）**。
- 修正建议依然成立（不改代码，供后续批次）：`chrome_with:3746` 与
  `draw_processing_primitives` 的 stroke `radiusX/Y` 用 `r - 0.5`（圆心即与 fill 同心 (r,r)，
  外沿恰落轮廓线）——这是同心化的正确修法，主控方向正确。

**内容同构性**【静态】：两组轮廓区都只有 chrome 的 fill+1px stroke；
mic 药丸 x≥11.5、波形条居中带、停止/提交键距右缘 ≥9px、文本居中——全部内缩不触角区
（`mic_indicator`/`waveform`/`stop_button`/`placeholder_text` 几何）；Processing 光晕用
`FillRoundedRectangle(radius=corner_radius, inset 1.0)` 裁剪（`:3656-3668`），不溢出角区。
⇒ 「r16 组画了额外内容到角区」候选不成立。

**fixup 新旧等价性**【数学】：cov=1 内部区（r16: 8356px / r10: 8516px）新规则与旧规则输出
逐位相同；仅轮廓区（r16: 284px / r10: 124px）行为不同（SDF 修复所在）。
⇒ 若 r16 组的「坏」与改前**肉眼相同**，其主导伪影不在轮廓 AA 区（fixup 对内部区是 no-op）；
但端测只能证明「仍不满意」，不能证明「逐位同前」，此推论保留为弱证据。

### A5. 剩余候选与判别实验（按信息量排序，供主控派发）

⚠️ **E3 优先级最高**：OVERLAY-141 的立论前提「BindDC 后 alpha 未存活」**从未被直接测量过**
（当时由症状反推）。E3 一次探针同时测量前提 + 内容 + fixup 三层。

| # | 实验 | 手段 | 能回答什么 |
| --- | --- | --- | --- |
| E1 | Recording 强制 r=10（`overlay_frame_radius` 临时翻转或加 debug 开关） | 探针包 + Gavin 目视 | 坏→好 ⇒ 半径值是原因；仍坏 ⇒ 半径无关，看底色/内容 |
| E2 | Info 强制 r=16（与 E1 同一探针包） | 同上 | 坏 ⇒ 半径因果双向钉死；好 ⇒ 半径无关 |
| E3 | **角区像素 dump**：Recording 与 Info 各帧，WM_PAINT 内 fixup **前/后** 采样四角 8×8 块 + 左缘列 BGRA 打到 debug.log | 探针包，零 GUI | ① BindDC 后 alpha 实际是 0/255/保留（**验证前提**）；② fixup 输出是否符合 SDF 预期（排除 fixup 未生效/半径传错）；③ DPI 缩放异常（DIB 尺寸 vs 绘制坐标） |
| E4 | `GetDpiForWindow(overlay)` vs D2D RT `GetDpi` 对比 | 随 E3 顺带 | 排除 DPI 不一致导致 D2D 坐标系与位图像素错位（若存在会两组同坏，非分界解释，但要排除） |

若 E1/E2 钉死半径因果而 E3 显示 fixup 输出正确，则机制必然在「D2D 对 r=16/h=36 圆弧的
实际光栅化与 SDF 理想覆盖的分歧」或「合成器对大半径小窗的边缘处理」——届时把 E3 dump 的
r16/r10 前后对照数据交给主控定夺下一步（可能需要 D2D 侧 PerPrimitive→双采样或手工软化描边）。

---

## Part B · 录音窗口显示文字光标

### B1. 关键静态事实：EDIT 本体不可能存活到录音帧【静态】

`Show` 处理臂**无条件** `destroy_edit_control(&mut state)`（`:1366`，注释 ASR-038-C）。
全部 EDIT 销毁/存活路径：

| # | 路径 | EDIT 结局 | 证据 |
| --- | --- | --- | --- |
| 1 | Enter 提交（EDIT 子类 `:761-792` / ⏎ 按钮 `:1970-1985` / 父 WM_KEYDOWN `:1943-1956`）→ `SubmitRequested` → `RestoreAndHide` | **销毁** ✓（restore 绕过压制） | `:5834-5840` → `:1622` |
| 2 | `FocusLost` 事件 → `Show(FocusLost)` | 销毁 ✓（Show 臂） | `:5732-5746` → `:1366` |
| 3 | 新录音 `RecordingStarted` → `Show(Recording)` | 销毁 ✓（Show 臂） | `:5638-5650` → `:1366` |
| 4 | `Shutdown` | 销毁 ✓ | `:1654` |
| 5 | `Hide`（任意来源）**在 StreamingEditing 态被压制** | **存活 + 窗口不隐藏** ⚠️ | `:1610-1616` |
| 6 | 🔴 **热键 Stop**（`HotkeyEvent::Stop :5598-5614`）**缺 OVERLAY_EDITING 守卫**：编辑中按热键停止 → 直接发 `Show(FallingToProcessing)` | 销毁 ✓（Show 臂）但 **OVERLAY_EDITING 未清** → 后续 Done/Cancelled 全被 `:5718-5725` 压制 → **Processing 浮层永久卡屏**（每帧 shimmer 重绘，无 auto-close），直到下次录音 | `:5598-5614` + `:5718-5725` |

⇒ **「EDIT 存活 + 录音帧可见」被静态证死**（任何到达录音帧的路径都先过 :1366）。
Gavin 截图里的光标 **不是活的 EDIT 控件**。

### B2. 主假设：线程级 caret 对象泄漏【静态证据成立，可见性需运行时实测】

- caret 由 EDIT 内部 WM_SETFOCUS 创建（`SetFocus(edit_hwnd)` `:1590`）；全库 **零** `DestroyCaret`/`HideCaret`/`CreateCaret` 调用（grep 实证）——caret 清理完全依赖 EDIT 内部行为。
- `destroy_edit_control`（`:814-848`）只做：还原 wndproc → RemoveProp → `DestroyWindow(edit_hwnd)` → 删 brush/font。**无任何 caret/focus 处置**。
- Win32 语义要点（文档未定论处，如实标注）：焦点窗口被 `DestroyWindow` 时收到的是 `WM_DESTROY`，**不是** `WM_KILLFOCUS`；EDIT 的 caret 生命周期与**线程输入队列**绑定而非窗口。若 EDIT 内部只在 WM_KILLFOCUS 销毁 caret，则「带焦销毁」路径下 caret 对象存留，焦点回落同线程父窗后 caret 保持可见闪烁，位置停在最后 `SetCaretPos`（EDIT 文本区，恰在浮层窗口内）。
- 系统 caret **直接打屏幕、不走分层窗合成**（ULW 整面被合成器替换、子控件不参与；caret 是 GDI 屏幕级绘制）⇒ 录音窗（ULW）上出现文字光标，与截图症状**精确吻合**。
- **判别探针（交 tester/Gavin，一行日志级）**：复现「录音→编辑→提交」后，在 overlay 线程调
  `GetGUIThreadInfo(GetCurrentThreadId())`，看 `hCaret != 0` 与 `flags`；或退出编辑后立即
  `DestroyCaret()` 的探针包看症状是否消失（消失 = 钉死）。

### B3. EDIT 生命周期次生缺陷清单（静态可达性已证，均不在本单修）

1. **caret 泄漏**（B2，主假设）——建议修法：`destroy_edit_control` 内 `DestroyWindow` 前后
   显式 `HideCaret+DestroyCaret`（或先 `SetFocus(NULL)`）。代价一行级；风险：无（caret 本就是我们线程的）。
2. **热键停止臂缺守卫**（§B1-⑥）——修法：`HotkeyEvent::Stop` 臂在发 `Show(FallingToProcessing)`
   前检查 `OVERLAY_EDITING`，为真则改发 `RestoreAndHide`（或先清 OVERLAY_EDITING）。
   代价一行级；风险：注意与 Done/Cancelled 压制逻辑（`:5718`）联动，别造新死锁。
3. **Hide 双重压制卡窗**——`CancelRequested`→`Hide`（`:5810-5816`）被 `:1610-1616` 压制
   （status 仍 StreamingEditing）；`Cancelled`→再次 `Hide`（`:5718-5725`）再被压制。
   ⚠️ 可达性存疑：编辑态 ESC 到不了父窗 WM_KEYDOWN（焦点在 EDIT，子类只转发 Enter，`:757-810`），
   编辑态也点不到取消钮（`:1995-1997` 有意移除）⇒ 当前静态可达触发器疑似为零，
   但守卫语义矛盾（controller 注释声称「controller will also suppress Hide while OVERLAY_EDITING」，
   而 CancelRequested 先清 flag 再发 Hide，两层压制互相打架）——建议与 ② 一起在下一批统一收口。
4. **ESC 编辑态不可达**（上面 ③ 的根因之一）——用户在编辑态按 ESC 大概率无效果（静态推断，
   需运行时确认）；若产品语义要求 ESC 退出编辑，需子类proc 加 VK_ESC 转发。

### B4. FLICKER-130 耦合判定（任务书 §B3-2 的怀疑，**证伪**）

任务书怀疑「FLICKER-130 丢弃编辑态迟到包 ⇒ Show 臂的 `:1366` destroy 不再执行 ⇒ 清理链断裂」。
**静态证伪**：

- 被丢弃的 Show 是 `Show(StreamingEditing{text})`（`should_ignore_streaming_text(stopped)`
  门控位于 StreamingText 事件臂 `:5682`；编辑中 stopped=true（F3 不变量）⇒ 一律丢弃）。
- 该 Show 若**不**被丢弃会怎样：进 Show 臂 → `:1366` **销毁 EDIT** 且无重建（create 只在
  EnterEditMode `:1563`）——正是 `:5679-5681` 注释自述的历史缺陷（"the old editing exemption
  destroyed the EDIT control instead of syncing it"）。
- ⇒ 丢弃这些 Show 不是破坏清理，而是**消灭了一个销毁 EDIT 的肇事路径**；EDIT 的清理从不依赖
  它们（清理在命令路径：RestoreAndHide/Show/Shutdown，全部完好）。
- **FLICKER-130 与 B 组症状无因果**。caret 泄漏假设与 INVESTIGATE-142 的候选 3/4（编辑态内
  父窗 BitBlt/EDIT 自绘）**不冲突也不互相解释**——142 研究的是编辑稳态内的闪烁（EDIT 按设计存活），
  本条是跨态残留（EDIT 按设计死亡后）。142 的候选 6（caret 在分层窗上的系统级行为）获得第二个
  数据点：caret 伪影是队列级、可跨合成模式存活，建议 142 的实测探针顺带测 `GetGUIThreadInfo().hCaret`。

---

## 修法建议汇总（**本单一律不实施**）

| # | 缺陷 | 建议 | 代价 | 风险 |
| --- | --- | --- | --- | --- |
| F1 | Part A 机制未定 | 先派 E3 探针包（+E1/E2 半径翻转），拿到 dump 数据再定 | 一次探针构建 + Gavin 一轮目视 | 低 |
| F2 | H1a 同心度 0.2px（非分界原因，顺手项） | stroke radius 改 `r-0.5`（`chrome_with` + `draw_processing_primitives`） | 极低 | 低（视觉口径微移，需 Gavin 过目） |
| F3 | caret 泄漏 | `destroy_edit_control` 显式 caret 清理 | 极低 | 低 |
| F4 | 热键停止臂缺编辑守卫 | `HotkeyEvent::Stop` 检查 `OVERLAY_EDITING` | 极低 | 中（联动压制逻辑，需护栏） |
| F5 | Hide 压制语义矛盾 + ESC 编辑态不可达 | 统一收口退出编辑路径 | 低 | 中（涉及退出语义产品决策） |

## macOS 适用性

本文全部取证锚点在 Windows overlay 线程（`overlay_wnd_proc`/EDIT 子类/ULW 提交/caret 队列语义），
全部位于 `#[cfg(target_os = "windows")]`；macOS 侧无 ULW/EDIT/caret 对应物，**不适用**（macOS
overlay 若将来实现，注意 caret 等价物（NSText insertion point）归焦点系统管，无此泄漏形态）。

## 红线自证

- `src/**` 零改动：`git diff --numstat -- src/` 空 + `git status --short -- src/` 空（本单执行时实测）
- 数学探针为独立 rustc 单文件（`C:\msys64\tmp\opencode\probe_sdf.rs`），**未进 crate、未占 main.rs 测试区**，交付前已删除
- 未 commit / 版本号未动 / 未出包 / 未跑 cargo clean；仅读命令 + rustc 探针编译（独立目录，不触碰 crate 的 target/）
- 零 git 破坏性命令；零凭证落盘；无残留临时文件
