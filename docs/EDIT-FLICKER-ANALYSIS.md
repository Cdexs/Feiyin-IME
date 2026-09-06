# INVESTIGATE-120 · 编辑态左右移光标闪烁 —— 根因定位报告

- Worker: coder-2 · 2026-09-06
- 性质: 🔴 只查不修，生产零改动（git diff 仓库内为空；探针工程在 `collab/research/repro120/`，仓库外）
- 行号基准: 调查时刻的 `src/main.rs`（coder-1 正并行改 BUG-119，后续行号可能漂移，已附函数/分支名以便重定位）

---

## 1. 根因（精确到行，附实测证据）

### 1.1 结论一句话

**闪烁不是光标移动本身造成的。** 根因是：进入编辑态后到达的**迟到流式 ASR 包**被控制器转发为
`OverlayCommand::Show(StreamingEditing)`（`:5378-5390`），而 Show 处理器会：

1. **无条件销毁 EDIT 子控件且没有任何重建路径**（`:1298`）→ 文字整体消失；
2. 无条件 `InvalidateRect` 全窗失效（`:1410`）→ 父窗口整幅 BitBlt 重绘（父窗口无 `WS_CLIPCHILDREN`，会盖掉子控件像素）；
3. 流式分支重算 `target_size`（`:1240-1275`，文本变化→宽度变）→ 插值循环（`:1739-1745`）每 tick
   `needs_repaint=true` 并 `SetWindowPos`（`:1776`，且 x 逐帧重居中 → 窗口横移）→ **窗口反复移动/缩放**。

→ 窗口闪烁（几何风暴）+ 文字闪烁（EDIT 被销毁/被盖掉）**同时**出现，与 Gavin 描述「窗口和文字闪烁」逐字吻合。
Gavin 把它与「移动光标」关联，是因为用户进入编辑态的最初几秒恰是 finalize 拖尾包的到达窗口
（ASR-074 已记录的「finalize 拖尾」问题）——两者时间重叠，形成因果错觉。

### 1.2 静态证据链（生产代码）

| # | 事实 | 位置 |
| - | ---- | ---- |
| E1 | 编辑态下 `should_ignore_streaming_text(stopped, editing) = stopped && !editing` —— **编辑态不丢包** | `:4786` |
| E2 | 迟到包走 `else if editing` 分支，被转发为 `Show(StreamingEditing)`（`size: RECORDING_OVERLAY_SIZE=240x36`） | `:5378-5390` |
| E3 | Show 处理器开头无条件 `destroy_edit_control(&mut state)` + `restore_noactivate` | `:1298-1299` |
| E4 | `create_edit_control` 全文件唯一调用点在 `OverlayCommand::EnterEditMode` 处理器 —— **Show 不重建 EDIT** | `:1475` |
| E5 | Show 流式分支（StreamingEditing 属于 `is_streaming_text`）重算 `target_size = adjust_overlay_pos_size_for_text(...)` | `:1232-1236, :1240-1275` |
| E6 | 插值循环对**包括 StreamingEditing 在内的一切活跃态**执行：`current_size != target_size` → 每帧 `needs_repaint=true` | `:1739-1745` |
| E7 | 插值循环每帧 `SetWindowPos` 且 `x = centered_x(...)` 重算（窗口横移） | `:1754-1787` |
| E8 | Show 末尾无条件 `SetWindowPos`（`:1400-1408`）+ `InvalidateRect(hwnd, None, false)`（`:1410`） | 同左 |
| E9 | 父窗口样式 `WS_POPUP`（无 `WS_CLIPCHILDREN`）+ `WS_EX_LAYERED(LWA_ALPHA)`；WM_PAINT 整客户区 BitBlt | `:1160-1166, :1943-2020` |
| E10 | 光标移动路径（EDIT 自绘/滚动）**不触发**任何 `SetWindowPos`/`needs_repaint`；StreamingEditing 定时器分支只读 dirty flag | `:1655-1663` |

E10 是主控「不是刷太勤」判断的补充确认：**纯光标移动在代码里没有任何窗口级重绘/几何路径**。

### 1.3 实测证据（探针决定性实验）

探针工程 `collab/research/repro120/probe-win/`（独立复刻 overlay 窗口结构，见 §5），
外部监测：`poller.py`（~1.2MHz 轮询窗口几何/可见性/子控件）+ `capture.py`（~75fps 帧差分类）。

| 实验（各 8s） | 窗口几何变化次数（poller） | LARGE_AREA 内容事件（capture） | 结论 |
| --- | --- | --- | --- |
| baseline（复刻编辑态+左右键 26 次，文本超宽触发滚动） | **1**（仅初始化） | 7~15（均值约 8） | 光标滚动只引起 EDIT 自身重绘，**无几何变化、无擦白帧** |
| baseline，关闭 ES_AUTOHSCROLL（不滚动） | 0 | 0~12（中位数 2） | 滚动重绘是 EDIT 内大额重绘的主要来源，但非闪烁根因 |
| baseline，去掉每次 paint 的 `SetWindowRgn` | 0 | 5~10 | 与 baseline 无系统差异 → `SetWindowRgn` 同区域不引发重绘风暴 |
| baseline，去掉 `WS_EX_LAYERED` | 0 | 7~13 | 分层窗口不是主因 |
| **packets_prod（复刻迟到包：每 ~900ms destroy EDIT + target_size 重算 + InvalidateRect + 插值 SetWindowPos）** | **26**（~300ms 一波） | **21~31（全部 LARGE）** | **复现「窗口和文字同时闪」的完整签名** |
| packets_sync（同上但重建 EDIT——修复候选 A 的行为） | 26 | 42 LARGE + 17 SMALL | 每包重建 = 每包文字闪现，同样闪烁 |

补充测量（75fps 帧差）：baseline / no_setrgn / no_layered 全部变体均**未捕获到任何「文字全消失」帧**
（bright 像素计数无近零谷）——即 EDIT 自绘路径不存在可见的擦白闪烁；而 packets_prod 的内容大变更
波次与 poller 几何波次一一对齐。

> 注：LARGE_AREA 绝对计数在轮次间波动较大（滚动时机随机），故结论取「有/无几何变化」「有/无擦白帧」
> 这两个鲁棒判据，不依赖绝对计数。

### 1.4 真机端到端验证的环境限制（如实声明）

真机全链路复现需要在线流式 ASR 产出文本 → 进入 RecordingWithText → 点击进编辑态。实测本机：

- 默认输入（HD Pro Webcam C920 麦克风）：5s 采样 RMS=0.000015 / peak=0.000031 —— 数字静音（物理遮蔽）；
- 主板麦克风阵列（Senary Audio）：RMS=0.000015 / peak=0.000183 —— 同样无拾音；
- 两个蓝牙耳机（EDIFIER TWS1、天猫精灵 B2）endpoint 状态 = **Unplugged**（未连接）；
- 扬声器回路正常（WASAPI loopback 实测 TTS 信号 RMS=0.082 / peak=0.94）—— 声音出得去，但无麦克风能听到。

在线 ASR 三次实测均 `outcome=failed, 0 samples`（VAD 判静音），与 Gavin 今天 12:07-12:09 的会话日志一致。
**因此「真实 exe 端到端复现」不可行**，根因判定基于：生产代码静态链（E1-E10，逐条可复核）+ 结构复刻探针的机制复现（§1.3）。
若需端到端确认，建议 Gavin 正常使用时挂 `-debug` 并留意编辑态开头 2 秒内 `debug.log` 是否出现
`OVERLAY-043: ignoring late StreamingText` **之外**的迟到包转发记录（当前该路径无日志，见 §4 修法建议第 0 条）。

---

## 2. 三嫌疑裁决

### 嫌疑 1：光标移动触发尺寸重算 → `SetWindowPos` 在飞 —— **证实（但触发源修正）**

- 证实「SetWindowPos 在飞」：packets_prod 探针 26 次几何变化/8s；生产链 E5→E6→E7→E8。
- **修正**：触发源不是光标移动，而是编辑态迟到包的 Show。纯光标移动（E10 + baseline 探针 0 几何变化）不触发任何 SetWindowPos。
- `:1396`（OVERLAY-101 标记的「在飞 current_size」源头不一致点）正是本链路的执行点之一。

### 嫌疑 2：`WM_ERASEBKGND`（`:1834`）与双缓冲配合 —— **证伪**

- `WM_ERASEBKGND` 恒返回 1（跳过擦除），所有路径一致；
- 75fps 帧差在所有非 packets 变体中**零擦白帧**；
- `:2210`（现 `:2194` 附近 FocusLost/Error 分支）`apply_overlay_window_region(hwnd, rect, Some(10), false)`
  与其他调用点第四参的差异**无语义**——形参名为 `_is_recording`（带下划线，函数体未使用，`:363-380`）；
- `SetWindowRgn` 同区域重复调用不引发重绘风暴（探针 no_setrgn 与 baseline 无系统差异）；
  但注意：**每次 WM_PAINT 都 SetWindowRgn(bRedraw=TRUE)（`:2174`→`:378`）本身是不必要的重复调用**，
  属卫生债（见 §4 附加建议），不是本闪烁根因。

### 嫌疑 3：EDIT 控件与自绘的双重绘制 —— **证伪（作为主因）；降级为「叠加观感因素」**

- 子类化完整转发原 EDIT proc（`:672-682`，`CallWindowProcW`），不存在「绕一半」的打架；
- 探针证实 EDIT 滚动重绘存在（ES_AUTOHSCROLL + 超宽文本，~8 次/8s@26 次按键）但：
  无几何变化、无擦白帧 —— 肉眼是「文字跳位」，不是「窗口和文字闪烁」；
- 长按方向键（~30Hz 键重复）会把滚动重绘放大到接近闪烁观感，建议修根因后一并观察；
- 颜色步进理论（EDIT 背景 vs D2D chrome 背景微差）证伪：两侧均为 `OVERLAY_BG_DARK 0x110F0D`
  （`:1945/:673` vs `:3723` D2D `colorref_to_d2d` 同源）。

---

## 3. 与 OVERLAY-101 Bug A 的关系 —— **同源**

Bug A（宽度偶发抖动、突然变长又缩回）与本闪烁共用同一几何机器：

- 同一条 `Show 流式分支 → target_size 重算 → 插值循环 → SetWindowPos + centered_x` 链
  （`:1240-1275` → `:1739-1745` → `:1754-1787`）；
- OVERLAY-101 静态取证标记的 `:1396`（在飞 current_size 应用点）在链路内；
- 迟到包在编辑态给这条机器加了一个**新的触发场景**（此前只有录制流文本触发）。

推论：修 §4 的根因（编辑态不再走 Show 几何机器）会同时消除编辑态场景下的 Bug A 表现；
但 Bug A 的录制态场景仍需 OVERLAY-101 原案独立收口，**本单不替代 OVERLAY-101**。

---

## 4. 修法建议（供主控/ Gavin 拍板，本单未实施）

### 方案 R1（推荐，最小面）：编辑态迟到包直接丢弃

- 位置：控制器 StreamingText 处理（`:5376-5390`）。
- 改动：`should_ignore_streaming_text` 改为 `stopped`（不再豁免 editing），或直接删掉 `else if editing` 分支。
- 语义：`EditRequested` 已经 cancel + stop（`:5515-5527`），迟到的 finalize 尾包对编辑中的用户只有害处
  （盖写文本/触发几何风暴）；EDIT 里已有文本，丢弃尾包不影响提交内容。
- 影响文件：`src/main.rs` 一处 match 分支；护栏：现有 OVERLAY-043 测试族 + 新增一条
  「editing=true 时 StreamingText 被忽略」断言（可进 tester-1 阶段三）。
- 风险：低。行为变化 = 编辑期间文本不再被服务器侧更新（这正是用户编辑的预期语义）。

### 方案 R2（备选，若产品确要「编辑中持续同步服务器文本」）：Show 对 StreamingEditing 走同步路径

- 位置：Show 处理器（`:1298` 附近）。
- 改动：`status == StreamingEditing && state.edit_hwnd.is_some()` 时**不销毁 EDIT**，
  改为 `SetWindowText(edit, text)` + EM_SETSEL 保光标；**跳过 target_size 重算与几何应用**（编辑期间冻结宽度）。
- 风险：中。用户已敲入的编辑会被服务器尾包盖写（需要产品确认）；实现涉及光标位置保持，边界多。
- 与 OVERLAY-121（per-pixel alpha）耦合说明：两种方案都不触碰窗口合成方式，无直接耦合；
  但需提示 OVERLAY-121 若走 `UpdateLayeredWindow` 路线，**子控件（EDIT）将无法渲染**——届时整个
  「EDIT 子控件编辑态」设计需重审（这是 OVERLAY-121 的前置技术前提之一，非本单范围）。

### 附加卫生建议（非本单必修）

1. `:2174` StreamingEditing 分支每次 WM_PAINT 都 `SetWindowRgn(bRedraw=TRUE)`：同区域重复调用无意义，
   建议仅在尺寸变化时设置（或缓存上次区域判等）。
2. 迟到包路径当前零日志：无论采纳哪个方案，建议在丢弃/同步处加一条 `log::debug!`，
   让「编辑期间是否有包到达」在真机上可观测（本单因该路径无日志而只能靠机制探针定案）。

---

## 5. 影响范围（若修）

- **会动到**：控制器 StreamingText 事件处理、Show 处理器（仅 R2）；两者都在 `src/main.rs`。
- **不会动**：WM_PAINT/双缓冲、SetWindowRgn、EDIT 子类化、EnterEditMode、隐藏/取消路径。
- **会不会让修过的 bug 复发**：
  - OVERLAY-043（脏标记节流）：不触碰 dirty flag 逻辑 —— 不复发；
  - OVERLAY-051-A/D/G（编辑态、Enter 提交、时间戳回放）：R1 不触碰；R2 需保 EM_SETSEL 光标语义 —— 有回归面，需过 `overlay_*_tests`；
  - OVERLAY-075（跨会话身份门禁）：丢弃发生在门禁**之后**（`:5362-5375` gen 校验在前）—— 顺序不变则不复发；
  - ASR-074（finalize 拖尾）：R1 把「拖尾在编辑态的可见后果」消掉，但拖尾本身（提交前丢字）仍在，
    **不能当作 ASR-074 的修复**；
  - OVERLAY-101 Bug A：编辑态场景随之消失，录制态场景仍在 —— 不算复发也不算修复。

## 6. macOS 同源性

**不同源、不适用。** macOS 侧流水线对 StreamingText 仅打日志不渲染
（`log::debug!("macOS pipeline: StreamingText ({} chars)")`，main.rs `:6641` 附近），
无 StreamingEditing 编辑态、无 Win32 EDIT 子控件、无 Show 几何插值机器 —— 不存在本缺陷的载体。

## 7. 探针工程与临时文件说明

- 保留：`collab/research/repro120/`
  - `probes/`（poller.py / capture.py / matrix.py / driver.py，外部监测与实验编排）
  - `probe-win/`（结构复刻探针源码 + 可执行文件；env 开关见源码头注释）
  - `frames/*.jsonl`（帧差事件记录，小体积证据）
- 已清理：`run/`（含 exe/DLL/含密钥 config 副本）、`frames/*.png`、`rec.m4a`、`show_handler.txt`、
  探针与被测进程均已退出。
- 仓库内 git diff：仅其他 Worker 的在途改动，本单未改任何仓库文件。
