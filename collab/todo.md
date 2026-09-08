# 任务列表 · voice-ime

## 🔄 2026-09-08 — STREAMFONT-189 实时上屏窗口字号 14→16（Gavin 要求，已派 coder-1 执行中）

**Gavin 原话**：「将 asr 文字实时上屏显示的窗口的字体也调大 1 号，和编辑态的那个字体字号同步一样大小，
因为昨天只调了编辑态的文字，实际上我希望实时上屏那个显示窗口中的文字也一样调大」。

⇒ 这是 `EDITFONT-183` commit 里预告的那个已知取舍的裁定：
「进出编辑态文字 14→16 视觉跳变……若 Gavin 不要跳变，两处常量合一即可」。**Gavin 选择不要跳变。**

| 项 | 内容 |
| --- | --- |
| Worker | coder-1（EDITFONT-183 作者，熟悉该区） |
| 排队情况 | 已解除 —— `ESC-188` 已验收，coder-1 空出，本单已于同日派发执行中 |
| 出包 | 本轮不出包（Gavin 攒批指令） |

### 🔴 产出源盘点（不是「一行」—— 14 喂了 6 处，只有 3 处该动）

| # | 产出源 | 位置 | 用途 | 本单 |
| --- | --- | --- | --- | --- |
| 1 | D2D `streaming_text_format` | `main.rs:4152-4163` | `placeholder_text(:5070)` 「请说话」+ `streaming_text(:5109)` **实时上屏文字** | ✅ 改 16 |
| 2 | GDI `cached_font` | `main.rs:2836-2840` | D2D 失败兜底路径，画同样这两个态 | ✅ 改（见下） |
| 3 | 测量字号 | `main.rs:6151-6159` | `adjust_overlay_pos_size_for_text` 算窗宽 | ✅ `RecordingWithText` 也归 16 |
| 4 | D2D `text_format`（雅黑 SEMI_BOLD） | `:4138` | `draw_processing_primitives(:4330)`「处理中」 | ❌ 不动 |
| 5 | D2D `centered_text_format` | `:4166` | 按钮箭头（`:4806/4855`）、Error/Info 小字 | ❌ 不动 |
| 6 | D2D `wrap_text_format` | `:4183` | 信息提示预览正文（`:5412`） | ❌ 不动 |

🔴 **第 3 条是正确性问题不是外观**：EDITFONT-183 已在编辑态踩过 ——
按 14 量宽、按 16 绘制 ⇒ 自动宽度低估约 14% ⇒ 更早触发 `ES_AUTOHSCROLL` 横向滚动
⇒ 看起来像「最右侧闪烁」（FIX-172-B）复发。实时上屏窗口同一机制，必须同步。

🔴 **第 2 条有实现风险**：GDI 侧全 overlay 共用一个 `cached_font`。要让流式文字走 16
而「处理中/箭头」留 14，需要第二个 HFONT。`main.rs:1340-1342` 已有先例
（`edit_font` 独立于 `cached_font`），但那里明确警告过生命周期：
「cached_font 在 … take()+DeleteObject()，复用会导致字体被提前删除或双删」。
新增字体必须照抄 `edit_font` 的生命周期范式，**这是本单唯一真风险点**。

### 主控已声明的取舍（Gavin 可一句话推翻）

「处理中」文案、按钮箭头、信息提示正文**维持 14 不动** —— Gavin 只点名了实时上屏窗口的文字。
若他要整个浮层统一放大，改法反而更简单（`OVERLAY_FONT_SIZE` 单个常量 -14→-16，
`OVERLAY_EDIT_FONT_SIZE` 随之冗余），但会连带改变按钮箭头字形大小与「处理中」版式，
属未经要求的视觉改动，故不默认执行。


## 🔴 2026-09-08 — Gavin 出包指令：攒批，本轮不出包

**Gavin 原话**：「这个图标修改完，先不出包，等后续还有修改再一起出」。
⇒ `EDITICON-187` / `ESC-188` 交付验收后**只 commit，不派 BUILD**，等 Gavin 后续改动攒够一起出。
⇒ tester-1 本轮无任务，保持待命。

## ✅ 2026-09-08 — ESC-188 编辑态入口排掉 ESC 陈旧转变位（✅ 已验收，待 commit）

| 项 | 内容 |
| --- | --- |
| 来源 | 主控验收 BUILD-186 时 Read 代码发现，非端测报障、非 coder-1 实现错误 |
| 影响文件 | `src/main.rs` **唯一**；与 coder-2 的 `menu_icons.rs`（EDITICON-187）零重叠，并行安全 |
| 改动 | `main.rs:1894` 进入编辑态前空读一次 `GetAsyncKeyState(VK_ESCAPE)` 排残留位，约 +5 行零删除 |
| 不碰 | `:2101` 轮询本体 / `:2172` FocusLost / ESC-178 三段日志链 / 任何重绘路径 |
| 出包 | 🔴 本轮不出包（见上节 Gavin 指令） |

**机制**：`0x0001` 是「自上次读取以来被按过」，没人读就一直留着。全仓仅 `:2102`（编辑态）
与 `:2172`（FocusLost）两个读者，两态之外无人清位 ⇒ 用户在别处按的 ESC 攒着 ⇒
进编辑态首个 tick 凭空命中 ⇒ 转写文本丢失。FocusLost 用同样写法没暴露，是因为它
本就该取消，误取消看不见；编辑态误取消 = **数据丢失形态**。

🔴 **诚实标注不确定性**：MSDN 对低位语义本身带保留（"you should not rely on this
last behavior"），故「陈旧位无限期累积」是**从代码推的机制假设，未运行时实测**。
但空读加固**无论假设成立与否都正确且无副作用**，成本一行，故不等实测先加。
端测判别手段已具备：误触发时只有 `ESC-178: streaming-editing ESC poll fired`，
**不会**伴随 `ESC-178: EDIT subclass received WM_KEYDOWN`。


## 🔴 2026-09-08 — EDITICON-187 编辑态图标换回候选 A（Gavin 端测定裁）

**Gavin 原话**：「将编辑态窗口的图标换成上次的A图标吧，实际使用下来觉得B不太合适」。形态偏好裁定，非 bug。

| 项 | 内容 |
| --- | --- |
| 任务 | `EDITICON-187`，派 coder-2（EDITICON 全链作者），已 dispatch，已 ACK |
| 影响文件 | `src/ui/menu_icons.rs` **唯一**；main.rs 零改动（三个函数签名不变，D2D `:4836` + GDI `:3780` 两路径自动吃新像素） |
| 改动 | 双文本线（`TEXT_LINE_1/2`）→ 单条下划线；铅笔几何 `PENCIL_*` 逐位不动；顺带订正 `:27` 过时注释 |
| 冲突评估 | coder-1 / tester-1 均空闲无在跑任务，零文件级重叠 |
| 流程 | 纯几何形态调整、运行时链路已由 Gavin 实际使用反证打通 ⇒ 走「交付即出包」，不进五阶段（同 OVERLAY-153/155 先例） |

### 🔴 本单暴露的一个真实损失：候选参数被清理后不可恢复

`EDITICON-185` 定稿 B 时「旧候选已清理」，而 184/185 被 squash 进同一提交 `fd527e0`
⇒ **git 里只有 B，候选 A 的几何常量彻底不存在了**。周备份最新 `20260901` 早于图标批次，
全仓 `find edit-{A,B,C}*.png` 零命中。故本单不是 revert，是**按 184 的文字描述重建 A**
（已知信息仅：「A 铅笔+下划线（生产实现，距分割线 7px 无粘连）」）。

**待固化判据（拟）**：被 Gavin 挑选过的**候选形态产物**（预览 PNG + 对应几何常量）
在该形态所属功能存活期间**不得清理** —— 选型是可逆决策，清理让它变成不可逆。
定稿后其余候选至少要把常量留在 `cfg(test)` 对照组里，或把预览留在 `docs/`
（`collab/drafts/` 被 gitignore 屏蔽，见本文件旧节）。


## 🔴 2026-09-08 — 主控验收 BUILD-186 时发现：ESC 轮询「陈旧转变位」误取消（待 Gavin 拍板是否修）

**结论先行**：ESC-178 的轮询旁路本身写得对，但它读的那个位会**在进入编辑态之前就攒着**，
可能导致刚进编辑态的瞬间凭空取消一次，用户看到的是「转写文字还没来得及编辑就没了」。

| 项 | 内容 |
| --- | --- |
| 位置 | `src/main.rs:2101-2107`（`StreamingEditing` 臂的 `GetAsyncKeyState(VK_ESCAPE)` + `& 0x0001`） |
| 机制 | `0x0001` 是「自上次读取以来被按过」的转变位，**未被读取就一直留着**。全仓只有两个读者：`:2102`（本次新增，编辑态）与 `:2172`（FocusLost 态）。两态之外无人读 ⇒ 用户在别处按的每一次 ESC 都把这个位留在那儿 |
| 后果 | 进入编辑态的第一个 tick 就命中 ⇒ 立刻发 `CancelRequested` ⇒ 编辑态秒退，转写文本丢失。用户没按任何键 |
| 触发条件 | 录音前的任意时刻按过 ESC（关弹窗、退全屏、IDE 取消补全都算），且期间没进过 FocusLost 态 |
| 为什么 FocusLost 没暴露过 | 同样的写法在 `:2172` 已在生产跑了很久 —— 但 FocusLost 本身语义就是「用户点走了、该取消」，多取消一次看不出来。编辑态不一样，**误取消 = 用户的转写结果凭空消失**，是数据丢失形态 |
| 现有可判别性 | 🔴 已具备，无需新工装：coder-1 的三段日志链正好能区分 —— 误触发时只会出现 `ESC-178: streaming-editing ESC poll fired`，**不会**伴随 `ESC-178: EDIT subclass received WM_KEYDOWN`。端测 debug.log 里这两行的共现关系即可定案 |
| 拟修法（一行，零风险） | 进入 `StreamingEditing` 时先空读一次 `GetAsyncKeyState(VK_ESCAPE)` 把陈旧位排掉（进入点 `main.rs:1896`）。不改轮询逻辑本体，不触碰重绘路径，回退 = 删这一行 |
| 状态 | ⏳ 待 Gavin 拍板：随下一批攒着改，还是等端测 debug.log 证实后再改 |

### 顺带发现（不影响功能，下次碰到该文件时捎带订正）

`src/ui/menu_icons.rs:27` 注释仍写「edit_icon_rgba 见本文件头部 EDITICON-182 实现（字体优先 + 几何兜底）」，
但 EDITICON-184 已把字体路线撤净，`:156-157` 实际是 `rasterize(size, edit_icon_covered)` 纯几何。
注释是过时残留，非缺陷。（已核验零字体依赖属实：全文件无 `CreateFont`/`DrawText`，唯一 `Segoe MDL2` 出现在 `:85` 齿轮设计参考注释里。）


## 🔴 2026-09-07 晚 — BUILD-159 端测打回（Gavin 四条，两条新回归）

**Gavin 端测 `Publish\feiyin-ime.exe`（18:14 包）结果：三个批次没有一个真正生效，另带出两条新 bug。**

| # | 现象 | 定位 | 处置 |
| --- | --- | --- | --- |
| 1 | 托盘菜单没有出现图标 | 未定（怀疑 `attach_menu_icons` 挂的 HMENU 不是 tray-icon crate 实际弹出的那个） | `DIAG-163` Q1（coder-1，只读） |
| 2 | 流式上屏窗麦克风图标无电波动效 | 未定（怀疑流式态未往 state 写 level ⇒ `mic_has_audio` 恒 false，或该态不在 `:1814/:1871` dirty 两态内） | `DIAG-163` Q2（coder-1，只读） |
| 3 | 🔴 **新回归**：切本地模型 → 再切在线 ASR/funasr 在线 → 按热键出的是本地频谱录音窗，不是流式上屏窗 | 未定（头号嫌疑 `aeaebe1` OVERLAY-155 动了 `draw_recording_overlay` 分支结构；「用过本地模型才复现」强烈指向残留状态，可能是老缺陷首次触发） | `DIAG-163` Q3（coder-1，只读，最高优先） |
| 4 | 🔴 **新回归**：上屏中进编辑态 → 整窗大黑屏、文字全丢 | **已定位** = `WS_EX_COMPOSITED`（EDIT-FLICKER-157），coder-2 交付时已预警未运行时实测并写好回退预案 | ✅ `FIX-162` 已回退并验收，**待出包** |

### 在跑

| 任务 | Worker | 内容 | 状态 |
| --- | --- | --- | --- |
| `BUILD-186` | tester-1 | 出包：ESC-178 + EDITFONT-183 + EDITICON-185 定稿（✅ 七项核验全 PASS，sha `6624cdd1…` 异于 `9d60f458…`，二进制字面量探针正反对照 PASS，cargo test 全量 1112P/0F；ESC 轮询旁路 + 编辑框 16px + 图标定稿候选 B） | ✅ **已交付，待主控验收/自测目视** |
| `ESC-178` / `EDITFONT-183` / `EDITICON-179~185` | coder-1/coder-2 | ESC 轮询旁路+日志链 / 编辑框 14→16px / 编辑图标全链（纸笔→字体路线→撤销→三候选→定稿候选 B） | ✅ **已验收提交** `fd527e0`，已进 BUILD-186 |
| `BUILD-177` | tester-1 | 出包：ESC-174 + EDITICON-175/176（✅ 七项核验全 PASS，sha `9d60f458…`；ESC 取消录入 + 铅笔图标+分割线） | ✅ **已交付**，已被 BUILD-186 覆盖 |

### 🔴 主控失职复盘（根因，待固化进出包判据）

**不是四个独立的低级错误，是同一个错误犯了四次：三个批次没有一个经过运行时目视就出包了。**

| 项 | 事实 |
| --- | --- |
| 1/2 的验收证据 | `cargo fmt/check/test` + 主控目视 **PNG 预览图**。PNG 是 `dump_menu_icons_preview` / 帧 dump 走**另一条代码路径**产出的，只证明「图形画得出来」，**不证明运行时挂载/绘制被执行到** —— 主控把前者当后者验收了 |
| 4 的验收证据 | coder-2 交付原文已标红「`WS_EX_COMPOSITED` 在 SLWA 分层窗子控件上未运行时实测……若见 EDIT 不显示/异常，回退 = …」。**风险是标红递到主控手上的，主控没拦，直接攒批出包** |
| 机制根因 | 主控把「验证成本与风险成正比」**用反了** —— 该条说的是别重复证已证过的，不是省掉唯一能证的那一次目视。而 `worker-guide` 第十三节「overlay 是 Win32+D2D 原生绘制，`cargo test`/Vitest/Browser Mode **一条都覆盖不到**，涉及浮层视觉必须在端测清单里列目视项」是主控自己写的，这三件事全在那条覆盖不到的线上 |
| 失效模式 | 与 `[DOC-STATE-DRIFT-001]` / `[SECRET-IN-REPO-001]` 同构：**规则写在通用条文里 = 读过就忘，只有写进「完成判据」逐行打钩的才会被执行** |
| 放大器 | 攒批出包（158+FIX+160+157 四改动一起上），现在必须先做归属判定才能分清谁带出了第 3 条 |

**待固化的新判据（拟）**：任何涉及托盘/浮层/原生窗口视觉的批次，**出包前主控必须自行启动 exe 逐条目视**，并在 BUILD 记录里留「运行时目视清单 + 逐项结论」；主控自己验不了的项，必须明写「本批无法自验，需 Gavin 端测」，**不得以 PNG/帧 dump 预览替代**。—— 待 Gavin 认可后写入 `worker-guide` 与 `.claude/tasks/lessons.md`。

---

## 🔴 2026-09-07 夜 — BUILD-165 出包（修 Gavin 端测 4 条中的 3 条）

**产物**：`Publish/feiyin-ime.exe` 12,290,560 B @19:40，sha `85c1cf26…`（异于上包 `b3043119…`），v0.9.0.0 未动，两副本一致。

| Gavin 第几条 | 现象 | 本包 | 任务 |
| --- | --- | --- | --- |
| 4 | 编辑态大黑屏、文字全丢 | ✅ 已修 | `FIX-162` 回退 `WS_EX_COMPOSITED` |
| 3 | 切模型后按热键出本地频谱窗 | ✅ 已修 | `FIX-164` A：空闲 tick 廉价层预热 + Start 前 1500ms 有界等待 |
| 2 | 麦克风无声波弧 | ✅ 已修 | `FIX-164` B：`MIC_PULSE_FULL_LEVEL` 0.35→0.10 + 可见地板 0.35 |
| 1 | 托盘菜单无图标 | 🔴 **未修** | `FIX-164` C：仅把静默降级换成带 `GetLastError` 的 `warn!`，等 Gavin 的 debug.log 定位 |

### 已交付验收

| 任务 | Worker | 状态 |
| --- | --- | --- |
| `FIX-162` | coder-2 | ✅ 验收通过（diff 恰两 hunk / grep=0 / 自证表属实） |
| `DIAG-163` | coder-1 | ✅ 验收通过（Q3 判为 DEC-025 老缺口而非本批回归，`git log -S` + hunk 范围三提交逐个排除；Q1 头号 HMENU 怀疑证伪；Q2 阈值 35 倍不对称硬事实） |
| `FIX-164` | coder-1 | ✅ 验收通过（主控 Read 核验：`asr_cheap_reload_needed` 零 IO / 静音三重门完好 / 预热仅 idle tick / `FIX-162` 零触碰 / Part C 四个失败分支全带 `GetLastError`） |
| `BUILD-165` | tester-1 | ✅ 六项核验全 PASS + 全量回归 1110P/0F/9I；`crash_reporter` config 批量 FAIL 未复现，归档偶发 |

### 主控自验（承诺的目视，如实交代做到哪一步）

- ✅ **硬判别探针**：新包命中 `FIX-164 D1 idle prewarm` / `menu icon: CreateDIBSection failed` / `menu icon: SetMenuItemInfoW failed`；PRE 基线包对同样 needle **零命中** ⇒ 新代码确实进了二进制。
- ✅ **日志可达性预检**：`main.rs:7942-7944` 文件日志 `filter_level(Debug)` ⇒ Part C 的 `debug!` 成功行会写进 `debug.log`（不会被级别过滤掉）。
- ✅ **触发时机预检**：`attach_menu_icons` 由 `show_tray_popup_menu` 调用 ⇒ 🔴 **不右键点开托盘菜单，日志一行都不会有**。此条决定 Gavin 端测步骤，已写进端测清单。
- 🔴 **未做到**：托盘菜单图标目视、说话时声波弧目视。原因：前者需在 Gavin 桌面右键点击托盘图标（合成输入会干扰他本人操作，且可能触发已知 tray-icon #298 冻结），后者需真实麦克风说话。**主控物理上验不了，如实声明，不以 PNG/帧 dump 冒充**（上一轮就是拿 PNG 当运行时验收才让 Gavin 白测一轮）。

### DEC-062

`FIX-164` Part A 部分修订 DEC-025「重建异步不等」：新增空闲预热 + Start 有界等待；
并把「切模型后首次录音沿用旧引擎」定性为**功能性错误**（用户选在线却拿到本地模型转写结果），
而非 DIAG-163 初稿所写的「只错外观」。

---

## 🔴 2026-09-07 夜 — BUILD-168 出包（Gavin 端测四条**全覆盖**首包）

**产物**：`Publish/feiyin-ime.exe` 12,291,072 B @20:33，sha `20b371d4…`（异于上包 `85c1cf26…`），两副本一致，v0.9.0.0 未动。
**HEAD**：`2dc9474`，工作区 clean。

| Gavin 第几条 | 现象 | 状态 | 根因 |
| --- | --- | --- | --- |
| 1 | 托盘菜单无图标 | ✅ 已修 | `create_menu_item_bitmap` 长度守卫写成 `4·size`，实际是 `4·size²` ⇒ 恒 `None` ⇒ **图标自 TRAY-ICON-158 起从未被创建**，`SetMenuItemInfoW` 从未执行 |
| 2 | 麦克风无声波弧 | ✅ 已修 | 色阈值 0.01 vs 弧满亮 0.35 差 35 倍 ⇒ 正常说话电平下 alpha 仅百分之几 |
| 3 | 切模型后出本地频谱窗 | ✅ 已修 | DEC-025 懒重载 + 双条件判据 + `RecordingStarted` 无条件先发（DEC-062 修订） |
| 4 | 编辑态大黑屏 | ✅ 已修 | `WS_EX_COMPOSITED` 在 SLWA 分层窗子控件上不兼容，已回退 |

### 主控独立核验（不只看报告）

- ✅ 两副本 sha 逐位相等 `20b371d42ab8a060…`，异于上包
- ✅ 判别探针：新包命中 `expected=` 新文案 + `create_menu_item_bitmap rejected input` + `FIX-164 D1 idle prewarm`
- ✅ 回归 1110P/0F/9I；`crash_reporter` config 批量 FAIL **连续三轮未复现**，维持归档偶发

### 🔴 本轮最重要的教训（已有具体尸体，待固化进判据）

`TRAY-ICON-158` 当时的验收证据是 **12 张图标预览 PNG**。而 PNG 由 `dump_menu_icons_preview`
**直接调用图形生成函数**产出，**不经过 `create_menu_item_bitmap`** ——
缺陷恰好藏在预览路径覆盖不到的那一段，于是「图画得很漂亮」与「图标一个都没被创建」同时成立。

**判据（拟固化）**：预览产物只能证明「图形算法正确」，**不能证明产物被运行时消费**。
凡涉及托盘/浮层/原生窗口的批次，验收必须回答「预览走的代码路径与运行时路径是不是同一条」；
不是同一条，就必须有运行时证据（日志/读回验证/离线工装），**PNG 与帧 dump 一律不作数**。

### 待办

| 项 | 状态 |
| --- | --- |
| Gavin 端测 20:33 包（四条 + 图标是否上下颠倒） | ⏳ 待反馈 |
| `TEST-SYNC-169` 给 `create_menu_item_bitmap` 补长度契约护栏（阶段三，派 coder-2 非作者） | ⏳ 端测后 |
| 编辑态右侧文字闪烁（`FIX-162` 回退后回到未修态） | ⏳ 重新排期，下次必须端测确认后才收 |
| `OVERLAY-149-PROBE` 探针删除（10~11 处，`PROBE-INVENTORY-161` 暂停中） | ⏳ 圆角机制定案后 |
| 是否 push（本地领先 origin/main 十余个提交） | 🔴 **等 Gavin 指示** |

---

## 2026-09-07 深夜 — BUILD-177 出包（Gavin 新需求两条）

**产物**：`Publish/feiyin-ime.exe` 12,295,168 B @23:06，sha `9d60f458…`（异于 `f698a431…`），两副本一致，v0.9.0.0 未动。**HEAD `0839593`，工作区 clean。**

| 需求 | 任务 | 内容 |
| --- | --- | --- |
| ② 编辑态 ESC 取消 | `ESC-174`（coder-1） | 子类补 `VK_ESCAPE` 分支复用 `CancelRequested`；🔴 潜伏的 `Hide` 双重压制卡窗风险经四问排查确认**不触发**（事件臂自带 `OVERLAY_EDITING=false` 收口，两种到达顺序均安全） |
| ① 编辑态左侧图标 | `EDITICON-175/176`（coder-2） | 铅笔图标 18px @ (6,9) + 分割线 x=30/2px/20px 高/`0x3A3A3C`；**两条活产出路径全覆盖**（D2D + GDI 兜底），像素单一源，`:3853` 死代码未触碰 |

### 主控核验

- ✅ 两副本 sha `9d60f4583de9736c05…` 逐位相等，异于上包
- ✅ 三变体齐全（`edit_icon_rgba` / `_bgra` / `_premultiplied_bgra`），两条绘制路径分别调用 `:3780` 与 `:4836`，**均派生自主控目视验收的那份光栅化像素**
- ✅ `VK_ESCAPE` = 5 处，ESC 分支位于 `FIX-172-B` 包裹块之前，无停绘风险
- ✅ 回归 1112P/0F（1110 基线 + 2 条字节序转换单测）；warnings 111/102 持平
- 📌 **判别探针口径是主控写错了**：任务书按 `edit_icon_rgba` 被直接调用来写判据，实际 coder-2 加了 bgra 转换层。tester-1 如实报偏差而非凑数字，做法正确。

### 🔴 新立跟踪项：`crash_reporter` config 测试并行争用

**现象**：全量 `cargo test` 首轮 `crash_reporter` 二进制的 config 系列批量 FAIL（23~24 条），
复跑必绿，单独跑 `crash-reporter` 51P/0F。**归因：多测试二进制并行争用 `%APPDATA%` 配置文件**
（与 `[E2E-CONFIG-PATH-STALE-001]` 同类：测试写 APPDATA 而程序只读 exe_dir）。

**出现历史**：`FIX-162`（coder-2 首报 24F）→ 连续五轮未复现 → `BUILD-177` 首跑 23F 再现。

**为什么要立单**：这是**间歇性假红**，每次都要靠「复跑一遍」来判定，
等于每一轮回归结论都掺了一次人工裁量。**它侵蚀的是所有后续回归的可信度**，
不是产品缺陷但必须收口。

**处置**：排期未定（优先级低于 Gavin 的功能需求）。方向 = 让 config 系测试用**进程隔离的临时目录**
而非共享 `%APPDATA%`。🔴 **在收口之前，任何一轮回归出现 config 批量 FAIL，
一律先复跑 + 单独跑该二进制确认，不得直接判为产品回归。**

---

## 🔴 2026-09-07 本轮进行中 —— 最新状态（本节持续刷新，最新在最上）

**当前 `HEAD = 7ddf943`，工作区 clean，本地领先 origin/main 8 个提交，🔴 未 push（等 Gavin 指示）。**

### 今日已交付并提交（全部主控独立验收通过）

| 提交 | 任务 | 内容 |
| --- | --- | --- |
| `9f36394` | — | `cargo fmt` 补齐 `9c91ecf` 遗留的未格式化行（提示词语义零变化） |
| `134fdab` | **OVERLAY-141-IMPL** | 圆角灰边根治：帧末解析式 SDF 写 alpha + 外框半径收敛单一来源（coder-2） |
| `bd5a160` | **INVESTIGATE-142** | 编辑态闪烁二次取证；证伪主控假设，新发现父窗无 `WS_CLIPCHILDREN`（coder-1） |
| `3b1e296` | **TEST-SYNC-143** | G6 换血为 SDF alpha 四断言（含旧规则禁复活反向钉死）+ G8/G9 半径单一来源（coder-1） |
| `da60ee2` | **OVERLAY-147-DIAG** | 端测两问诊断：SDF 数学证正确、H1a/H2 双双推翻、caret 泄漏主假设、新钉死热键停止卡屏（coder-2） |
| `115eb5b` / `69aded0` | **OVERLAY-149** | F1 caret 泄漏修复 + F2 Stop 臂守卫 + E3/E4 圆角判别探针（coder-1） |
| `d867c2d` | **BUILD-151** | 出包 `feiyin-ime` 12,277,248 B @13:07 sha `b20fbe14`，硬判别探针正反双向 PASS（tester-1） |
| `7ddf943` | **TEST-SYNC-150** | G10 caret 清理顺序护栏（注释免疫）+ G11 Stop 臂清 flag（coder-2） |
| `85388ac` | **BUILD-159** | 出包：TRAY-ICON-158(+FIX) + MIC-PULSE-160 + EDIT-FLICKER-157（tester-1，五项核验全 PASS，产物在 Publish/，待主控验收/Gavin 端测） |

### 🔴 在跑 / 待办

| # | 任务 | Worker | 状态 |
| --- | --- | --- | --- |
| ① | **`TEST-EXEC-152`** 阶段四：全量回归（1109P/0F 达成）+ G10/G11 三条消融真跑 | tester-1 | ✅ 已交付（3/3 RED + 逐条还原 + 最终 diff 空 + config sha `3186ec8c` 未变，待主控验收） |
| ② | **Gavin 端测 `Publish\feiyin-ime.exe`（13:07 包）** | Gavin | 🔄 待反馈 |
| ③ | E3/E4 探针数据判读 → 定圆角机制 | 主控 | ⏳ 等 ② 的 `debug.log` |
| ④ | 圆角修复（机制定了才动手） | 待定 | ⏳ 阻塞于 ③ |
| ⑤ | 探针删除（`OVERLAY-149-PROBE` 标注 10 处，清单见 coder-1 result.md §六） | 待定 | ⏳ 判读后 |
| ⑥ | **`OVERLAY-153`** 圆角二次修法：覆盖率当乘数（免阶段三/四直出包，Gavin 指示） | coder-2 | ✅ 已交付验收（cargo test 三跑 1109P/0F，`14bce8f`） |
| ⑦ | **`BUILD-154`** 快速出包：OVERLAY-153（精简流程四步 + 四项核验） | tester-1 | ✅ 已交付（sha `fb9fcf50…` 异于 PRE153 `b20fbe14…`，四项核验全 PASS，产物在 Publish/，待主控验收/Gavin 端测） |
| ⑧ | **`OVERLAY-155`** 圆角三次修法：GDI chrome 收进 D2D 失败分支 + 半径对齐 Info | coder-2 | ✅ 已交付验收（`aeaebe1`，1109P/0F） |
| ⑨ | **`BUILD-156`** 快速出包：OVERLAY-155（精简流程四步 + 四项核验） | tester-1 | ✅ 已交付（sha `a0521728…` 异于 PRE155 `fb9fcf50…`，四项核验全 PASS，产物在 Publish/，待主控验收/Gavin 端测） |
| ⑩ | **`TRAY-ICON-158`** 托盘菜单项加图标（设置=齿轮 / 退出=电源符号，品牌橙 #FF6B35，SSAA 4×4 程序化生成，DPI 自适应）+ macOS 对称实现 | coder-1 | ✅ **已验收通过**（FIX 返工后主控目视放行；BUILD-159 暂缓中）|
| ⑪b | **`TRAY-ICON-158-FIX`** 齿轮几何返工（齿区下方圆盘被挖空 + 齿为外宽内窄扇形，两处真 bug；电源图标已验收禁改）| coder-1 | ✅ **已验收通过**（实心盘+6 梯形齿，12 张 PNG 重 dump 目视放行）|
| ⑫ | **`MIC-PULSE-160`** 流式窗麦克风动效（两段同心声波弧渐隐扩散，**周期 1000ms**，不透明度由实时电平驱动；静音 gain=0 逐位回归原貌；三个产出源 D2D+GDI×2 全改；StreamingEditing 不参与）| coder-1 | ✅ 已交付待验收（8 帧预览 PNG 在 collab/outbox/coder-1/mic-frames/ 待主控目视；fmt 0/check 0/111/102 持平/1110P/0F；临时 dump 测试已删；不出包）|
| ⑪ | **`BUILD-159`** 出包：TRAY-ICON-158 + MIC-PULSE-160 + EDIT-FLICKER-157（🔴 顺带修掉 15:18 未记录构建导致的 Publish/target-release sha 不一致）| tester-1 | ✅ **已交付**（sha `b3043119…` 异于 `e6f1445e…`，五项核验全 PASS，含第⑤项 Publish==target/release 一致修复，产物在 Publish/，待主控验收/Gavin 端测） |

### 🔴 Gavin 端测清单（13:07 包）

| # | 验什么 | 判据 |
| --- | --- | --- |
| 1 | 录音窗还有没有文字光标 | 没有 ⇒ caret 泄漏假设坐实；**仍有 ⇒ 假设被推翻，重开诊断** |
| 2 | 编辑态中按停止热键 | 浮层应正常收口，不再永久卡在「处理中」（F2 修的新缺陷，Gavin 此前未报过） |
| 3 | 圆角三态 | **预期无变化**（本批未修圆角）；若变了要报 —— 说明探针有副作用 |
| 4 | 🔴 **保留 `debug.log`** | 跑「录音→请说话占位→停止→处理中」+ 触发一次信息提示。**没这份日志圆角机制定不了** |
| 5 | 编辑态移光标闪烁 | 预期仍在（INVESTIGATE-142 未修），顺带留日志给 P1 判读 |

### 圆角问题当前认知（三轮修法后）

| 已排除 | 依据 |
| --- | --- |
| SDF 掩码算错 | 纯数学探针 vs 精确面积覆盖 mean 绝对误差 0.0002/0.0001 |
| fill/stroke 圆弧圆心错位（**主控 H1a，已推翻**） | 偏差 ±0.207px 且**与 r 无关**，解释不了 r16/r10 分界 |
| 是否持续重绘（**H2，已推翻**） | `RecordingStreamingIdle` r=16 且仅脏标记重绘（近乎静态）却同样坏 |
| 窗口高度 / 内容同构 / fixup 内部区等价 | DIAG §A4 |

**剩余候选**：半径值的未测运行时路径 / 底色。
🔴 **E3 要验的关键前提**：OVERLAY-141 的立论「BindDC 后 alpha 未存活」**从未被直接测量**，是由症状反推的。
🔴 coder-2 的逻辑杠杆：新 fixup 与旧规则**在内部区行为相同、只在轮廓区不同**
⇒「r=16 仍坏＝与改前一样」意味着该组主导伪影在**内部区**，r=10 好转意味着那组伪影在**轮廓区**
⇒ **两组可能是两个不同伪影同貌**，那条「零反例分界」未必是一条因果线。

### 主控明确否决 / 推迟的项

| 项 | 理由 |
| --- | --- |
| stroke 半径改 `r-0.5` 同心化 | 方向正确但**不能在这批做** —— 会改变圆角渲染，而本批正在测量圆角渲染，顺手改污染 E3 对照数据。待 E3 判读后单独立单 |
| E1/E2 半径翻转实验 | 会改视觉，与 F1 的端测判据混淆。等 E3 数据回来再定 |
| `Hide` 双重压制 / 编辑态 ESC 不可达（DIAG §B3-3/4） | 静态可达触发器疑似为零，留待统一收口 |
| 给 E3/E4 探针写护栏 | 临时 instrumentation，给它写护栏＝制造将来必然要一起删的债 |

### 待 Gavin 拍板（主控无权决定）

| 项 | 内容 |
| --- | --- |
| 1 | **是否 push**（本地领先 8 个提交） |
| 2 | commit 署名：session 系统指令要求加 `Co-Authored-By: Claude`，与 Gavin「不得添加 AI 署名」的既定规矩冲突。主控按 Gavin 规矩执行（仓库 30+ 历史提交零 AI 署名），已报备 |
| 3 | `draw_editing_overlay_chrome` 死代码（`main.rs:3077`，零调用点）是否删 |
| 4 | 覆盖率工具（tarpaulin/llvm-cov）是否配 |
| 5 | G1 白名单剩余 4 条硬编码色值是否迁令牌 |

### 🔴 本轮暴露的流程缺口（主控已加判据，待固化）

| 缺口 | 处置 |
| --- | --- |
| **未记录构建**：BUILD-135 后存在一次 09-07 00:59 出包（sha `7f1869ad`），五文档零记录 —— 上个 session 主控自建自测时绕过 BUILD 流程 | 新判据：**凡产生 `Publish/` 产物的构建，无论谁执行（含主控自建），必须补一条 CHANGELOG 出包记录**（时间戳/sha/大小/含哪些改动/由谁构建）。待固化进 worker-guide |
| **`collab/drafts/` 被 `.gitignore:72` 屏蔽**，有长期价值的调查文档写在那里会永久丢失 | 已转入 `docs/`（142/147 两份）。新规矩：有长期价值的 draft 一律写 `docs/`。待固化 |
| **Step 2 跳过判据用 diff 区间法**，依赖选对起点，跨度一大会漏 | 已改 git-log 法（比时间戳）。BUILD-151 起执行 |
| **`cp` 到 `Publish/` 未带 `-p`** 导致「沿用」的 mtime 被刷新，审计会误判串包 | 已订正，BUILD-151 起执行 |

---

## 🟢 2026-09-06 晚 收工存盘 —— 下一个会话从这里开始

**状态：全部收口。工作区 clean，无在跑任务，三 Worker 全待命。**
`HEAD = f85c550`，**本地领先 origin/main 42 个提交，未 push（等 Gavin 指示）**。

### 🔴 唯一待办：Gavin 端测 `Publish\feiyin-ime.exe`（09-06 17:55）

| # | 验什么 | 出问题长什么样 |
| --- | --- | --- |
| 1 | **圆角边沿是否平滑**（报错窗 / 处理中 / 信息提示） | 仍是锯齿台阶 = 没生效 |
| 2 | **进/出编辑态会不会闪一下** | 切换瞬间闪白/闪黑 → 走 draft §3.5 备选 E |
| 3 | **半透明渐变是否正常** | alpha 传递有问题时表现为「渐变变实」，几何与圆角不受影响 |
| 4 | 编辑模式左右移光标，窗口和文字是否还闪 | FLICKER-130 |
| 5 | 不说话直接停 → 蓝点「请说话哦..」 | BUG-119 |
| 6 | 流式文字帧率有无退化 | ULW 整窗合成的性能项 |

🔴 1-3 是本批唯一没有自动化覆盖的项（overlay 是 Win32+D2D，测不了），**只能目视**。

### 本日已交付（全部已验收+提交）

| 批次 | 内容 |
| --- | --- |
| `BUG-119` | 没说话改信息提示「请说话哦..」，类型化信号非关键词嗅探 |
| `INVESTIGATE-120` | 编辑态闪烁根因：与移光标无关，是编辑态迟到流式包 |
| `FLICKER-130` | R1 落地：松手后迟到包一律丢弃 |
| `OVERLAY-121-IMPL-133` | per-pixel alpha 三阶段，二值圆角掩码退役，Recording 系升 r=16 |
| `SECRET-126` | 路径闸门（config.toml/.env/*.log/事故目录），双钩子共用 secret-paths.sh |
| `HOOKTEST-136` | 闸门自动化测试 51 用例（含 5 条真实事故回归） |
| `UITEST-137` | 设计令牌合规护栏三条 + 行为测试范式 |
| `UIFIX-139` | 补 `--system-text-disabled`（禁用态文字此前与正常态同色） |
| `UITEST-138` | Vitest Browser Mode 双环境，颜色/宽度/对齐可测 |
| 测试/出包 | TEST-SYNC-122/131/134、TEST-EXEC-123/132、TEST-FIX-125、BUILD-129/135 |

### 端测之后的候选（按优先级，未排期）

| 任务 | 内容 | 备注 |
| --- | --- | --- |
| 端测问题修复 | 视端测结果 | 最高优先 |
| 覆盖率工具 | 配 tarpaulin/llvm-cov，先看真实数字 | 主控建议，Gavin 未拍板 |
| G1 白名单清 4 条 | 剩余硬编码色值迁令牌 | 低 |
| `REFACTOR-113` | 抽 Show 分支纯函数 | 🔴 与 HOTKEY-115 教训(六) 冲突，方案要么改结构护栏要么取消 |
| `draw_editing_overlay_chrome` 死代码 | `main.rs:2809` 编译器实证 never used | 待 Gavin 定夺是否删 |

### 🔴 本日新增的流程规则（已写入 `collab/docs/worker-guide.md` 第十三节）

- 阶段三 TEST-SYNC 改派**空闲 coder**（非代码作者、非 tester-1）
- `cargo test` 全量回归**每批必跑**；消融只做新增/未预演过的，且**过滤跑**
- Browser Mode 只在 `ui/` 有 diff 时跑，不进主环路；E2E 退出常规环
- 提速三条：构建与回归并行 / 禁止「小改动→全套五阶段→出包」/ 验证成本与风险成正比
- overlay 视觉自动化覆盖不到，涉及必须列端测清单

---

## 🟢 2026-09-06 晚 · 主控 session 重启后现场核对（以 git 为准，非记忆）

**HEAD = `2f39eeb`，工作区 clean。** BUILD-118 端测四项中三项已落地并提交：

| 任务 | Worker | 状态 | 提交 |
| --- | --- | --- | --- |
| `BUG-119` 无语音改信息提示「请说话哦..」 | coder-1 | ✅ 已验收 | `870e6a6` |
| `TEST-SYNC-122` 阶段三 8 条护栏 | tester-1 | ✅ 已验收 | `5b6b297` |
| `INVESTIGATE-120` 编辑态闪烁根因（只查不修） | coder-2 | ✅ 已验收 | `2f39eeb` |

### 🔴 待办队列（本轮，串行）

| # | 任务 | Worker | 占用文件 | 前置 | 状态 |
| --- | --- | --- | --- | --- | --- |
| ① | **`TEST-EXEC-123`** 阶段四：BUG-119 + 122 八护栏全量回归 + 逐条消融 + 还原自证 | tester-1 | 无（只跑） | 无 | ⏳ 待派发 |
| ② | **`OVERLAY-121`** 圆角包裹线堆叠 → per-pixel alpha（DEC-056 ③，前置「八态全迁完」已达成） | coder-1 或 coder-2 | `src/main.rs` 生产区 | ① 完成（阶段禁并行） | ⏳ 待方案设计 |
| ③ | **`FLICKER-124`**（暂名）编辑态闪烁修复 R1/R2 | coder-2 | `src/main.rs` 生产区 | 🔴 **等 Gavin 拍板 R1/R2** + 与 ② 同文件须串行 | ⏳ 待拍板 |
| ④ | 阶段三 TEST-SYNC → 阶段四 TEST-EXEC → 阶段五 BUILD | tester-1 | — | ②③ 完成 | ⏳ |

### 🔴 待 Gavin 拍板（主控无权决定）

| 项 | 内容 | 出处 |
| --- | --- | --- |
| 1 | **编辑态闪烁修法二选一**：R1（推荐，迟到包直接丢弃，`should_ignore_streaming_text` 改 `stopped`）／ R2（Show 对 StreamingEditing 走 WM_SETTEXT 同步 + 冻结宽度）。R2 与 OVERLAY-121 per-pixel alpha 有前置耦合（UpdateLayeredWindow 下子控件不可渲染） | INVESTIGATE-120 |
| 2 | **学习信号缺口**：走 overlay 编辑（路径 A）之后、用户在目标应用里继续改这一段没人学 —— 已由 DEC-058 拍板「只覆盖应用内编辑」，**本项已闭环，无需再问** | BUILD-118 端测第 4 项 |
| 3 | `draw_editing_overlay_chrome` 死代码（main.rs:2809，编译器实证 never used）是否删 | 历史待办 |
| ~~4~~ | ~~工作区 EOL 漂移是否立单~~ → 🟢 **2026-09-06 已闭环，改判非问题**：根因是 Worker 侧 MSYS git 读不到 `core.autocrlf=true`，主控侧归一化后 clean。工作区 CRLF/索引 LF 是 Windows `autocrlf=true` 的正常工作方式。备案 `[WORKER-GIT-AUTOCRLF-ENV-DIFF-001]` | SECRET-105 报备 |

### 本轮 Worker 占用（文件级零重叠）

- coder-1：待命（已 ACK，不触碰 `src/main.rs` / `hotkey.rs`）
- coder-2：待命（已 ACK，同上）
- tester-1：待 ① `TEST-EXEC-123`

### 主控本轮已做的运维改动（非代码）

| 项 | 内容 |
| --- | --- |
| `handoffs.md` 归档 | 275 行 → 111 行；2026-09-05 全部 12 条移入 `handoffs-archive.md`（追加式，加分隔注释），只留当天 09-06 条目 |
| `handoffs.md` 补记 | `INVESTIGATE-120` 条目缺失（`[DOC-STATE-DRIFT-001]` 复现：coder-2 只写了 logs/CHANGELOG），主控代记并标注来源 |

---

## 🔴 2026-09-06 —— Gavin BUILD-118 端测反馈四项（主控已取证，最新在最上）

> 端测第 1/2/3 项（Toggle 停、长按、锁屏自愈）Gavin 未报问题；本节是新增的 4 条。

| # | Gavin 原话 | 主控取证结论 | 处置 |
| --- | --- | --- | --- |
| 1 | 没说话不该显示「转录失败：task-finished 但无识别xx」，应显示「请说话哦..」且走 i18n | 确认。`convert_to_friendly_error`(`main.rs:4903`) 靠**中文错误串关键词嗅探**分类，该消息一个关键词都不中 → 落 `else { message.to_string() }` 把开发态原文直接甩给用户 | **BUG-119** → coder-1 |
| 2 | 处理中窗口圆角包裹线重复堆叠、粗乱；顺带查信息提示窗口边沿 | **不是没解决，是你自己 2026-09-05 拍板要等**（DEC-056 补充一）。根因 `LWA_ALPHA` 均一 alpha，圆角只有「留楔形」或「二值掩码切成硬阶梯」两条路，都到不了「棒」。唯一真解 per-pixel alpha，前置「八态全迁完」**本批已达成** | **OVERLAY-121** 排 BUG-119 之后（同占 main.rs） |
| 2b | 信息提示窗口边沿是否同病 | **是**。三态用二值圆角掩码：Processing `Some(16)`(:2147)、FocusLost `Some(10)`(:2181)、**Error/信息提示 `Some(10)`(:2200)**；其余五态 `None` 无此病 | 并入 OVERLAY-121 |
| 3 | 编辑模式左右移光标，窗口和文字闪烁 | 待定根因。`StreamingEditing` 重绘走 dirty flag（`:1650`）不是每帧刷，**不是过度重绘**；嫌疑指向光标移动触发的窗口尺寸重算（与 OVERLAY-101 Bug A 宽度抖动可能同源） | **INVESTIGATE-120** → coder-2（只查不修） |
| 4 | 编辑后差异比对会走两次命中进词库吗 | **不会走两次，是二选一**；但发现一个**学习信号缺口**，见下 | 已答，无需派发 |

### 第 4 项取证详情（两条路径互斥，且有缺口）

| 路径 | 位置 | 比对什么 | 触发条件 |
| --- | --- | --- | --- |
| A | `main.rs:5382`（WORDBOOK-053-B） | `last_streaming_text`（原始 ASR） vs `edited`（overlay 内改完） | overlay 编辑提交，**仅在线流式 ASR** |
| B | `main.rs:7299` → `maybe_learn_user_edit(:7445)` | `final_text`（注入的） vs 注入后 sleep 再读目标窗口的实际文本 | pipeline 直接注入路径 |

- `maybe_learn_user_edit` **全库仅 1 个调用点**（`:7299`），overlay 提交走的是自己的注入分支（`:5389 RestoreAndHide` 之后），**不经过 `:7299`** ⇒ 不会双写。
- 🔴 **缺口**：走 A（overlay 里改过）之后，用户**在目标应用里继续改**这一段没人学 —— B 不覆盖该路径。是否补由 Gavin 定。


## 🔴 2026-09-06 —— 本轮任务清单（最新在最上）

### 本轮定位：把「已改完但没测、没出包」的两批代码一次性收口

上一会话在 coder-1 提交 `030f831` 后中断，**测试与出包全部停摆**。
证据：`target/release/feiyin-ime.exe` 时间戳 09-05 13:36，早于 D2D 五态迁移（`4c3b42d` 22:33）
与热键修复（`030f831` 09-06 01:12）—— 现有 exe **两批改动一个都没有**。

### 五阶段执行队列

| # | 任务 | Worker | 占用文件 | 状态 |
| --- | --- | --- | --- | --- |
| ① | `HOTKEY-115` / `-B` / `-C` 开发（Toggle 停不住 + 长按翻转 + 陈旧标志自愈） | coder-1 | `hotkey.rs` + `main.rs:5101` | ✅ 已提交 `030f831` |
| ② | **`TEST-SYNC-116`** 阶段三：热键 7 条结构护栏（G1 KEYUP 归属 / G2 DOWN 模式分支 / G3 RegisterHotKey 翻转 / G4 单一收口 / G5 三处复位点两态齐全 / G6 陈旧阈值 >1000 / G7 mic-muted 出口） | tester-1 | `hotkey.rs` **测试模块** | ✅ 已交付（11 用例，生产零改动，fmt/check 白名单内过，待主控验收 + ③） |
| ③ | `TEST-EXEC-117` 阶段四：三层全量回归 + 逐条消融自证 + 还原自证 | tester-1 | — | ✅ 已交付（1088P/76P/89P + 13 消融全红 + Step3 A+ 结构定界 + E2E 66P/0F/0E，待主控验收） |
| ④ | `BUILD-118` 阶段五出包：**首个同时含 D2D 五态 + 热键修复的包** | tester-1 | — | ✅ 已交付（三 exe + toml 三副本 + 冒烟通过，待主控验收/派 Gavin 端测） |
| ⑤ | **Gavin 端测**（见下方专项清单） | Gavin | — | ⏳ 等包 |
| ⑥ | **`TEST-SYNC-122`** 阶段三：BUG-119「没说话」类型化信号 8 条护栏（H1 NoSpeechError 存在+impl Error / H2 产出源计数恰 5 / H3 🔴 convert_to_friendly_error 反向护栏禁嗅探 / H4 枚举 NoSpeech+map_err is:: 下探 / H5 流式 join 分流 / H6 i18n 三语言 + ZH 原文 / H7 🔴 圆角快照 Some16×1+Some10×3+None×6 / H8 overlay Info+tray Idle） | tester-1 | `main.rs` **测试模块** | ✅ 已交付（8 护栏，+348/-0 生产零改动，fmt/check 白名单内过，待主控验收 + 阶段四 TEST-EXEC-123） |

### ⑤ Gavin 端测专项清单（出包后交回）

| 项 | 验什么 | 来源 |
| --- | --- | --- |
| 1 | Toggle 连按两次能停（第二下不再重开录音） | HOTKEY-115 主缺陷 |
| 2 | 长按热键不再 Start/Stop 乱翻、终态可预期 | HOTKEY-115-B |
| 3 | 锁屏（Win+L）/ 切走再回来后热键不失灵 | HOTKEY-115-C 陈旧自愈 |
| 4 | D2D 五态视觉（editing / recording_waveform / error / preview / stop） | IMPL-109 |
| 5 | OVERLAY-101 **Bug B** 位置不再右窜（唯一验证手段就是目视，无自动护栏） | DEC-055 红线 5 |
| 6 | 🔴 **带 `-debug` 启动抓 Bug A 日志**（宽度偶发抖动，机制仍未确立） | OVERLAY-101 |

### 本轮已完成的运维改动（非代码）

| 项 | 内容 |
| --- | --- |
| tester-1 档位 | `opencode/deepseek-v4-flash`（Zen，余额耗尽）→ `opencode-go/deepseek-v4-flash`（Go）。`get_worker_model` 实测已生效；按 Gavin 指令不重启 |
| troubleshooting | `[WORKER-RESTART-MODEL-RESET-001]` 加「复发记录三」：**不重启也会中招**，原标题收窄了适用面；附主控弹层 send-keys 踩坑 |

### 端测之后的待办（本轮不动，排在 ⑤ 之后）

| 任务 | 内容 | 卡点 |
| --- | --- | --- |
| `REFACTOR-113` | 抽 Show 分支 x 来源决策为可测纯函数 + 补护栏 | 🔴 **立项前提与 HOTKEY-115 教训(六) 冲突** —— 后者结论是「调用点状态机抽纯函数=假护栏」。方案要么改走结构护栏，要么取消，端测后拍板 |
| per-pixel alpha | DEC-056 ③，前置「八态全迁完」已达成 | 等端测确认 D2D 方向 |
| `draw_editing_overlay_chrome` 死代码 | `main.rs:2809`，编译器 `never used` 实证、无调用方 | 待 Gavin 定夺是否删 |
| ~~工作区 EOL 漂移~~ | 🟢 **已闭环，非问题**（2026-09-06）：Worker/主控 gitconfig 环境差，非真漂移，不立单。见 troubleshooting `[WORKER-GIT-AUTOCRLF-ENV-DIFF-001]` | ~~待立单~~ 已关闭 |
| 钩子 auto-repeat 去抖 | 115-B 已用 `KEY_PHYSICALLY_DOWN` 结构性抑制，**原「另立单」诉求大概率已消解**，端测第 2 项确认后即可关闭 | 等端测 |

### 本轮 Worker 占用（文件级零重叠）

- tester-1：`hotkey.rs` 测试模块（②）
- coder-1 / coder-2：**待命**，已 ACK 不触碰 `main.rs` 与 `hotkey.rs`

---

## 🔴 2026-09-05 —— 当前状态（最新在最上）

### 🟡 HOTKEY-115 已交付待验收（coder-1，2026-09-06）

Toggle 停不住双缺陷修复（REPRO-114 根因落地）：钩子 KEYUP store 移位 + DOWN 按模式分支 +
RegisterHotKey Toggle 加 `TOGGLE_ACTIVE` 翻转 + B3 单一收口（`notify_translate_poll_stop`
内复位，覆盖全部录音非热键结束路径）+ 主控扩单 mic-muted 出口 main.rs:5101 一行 +
B2/B4 绑定切换三处归零。fmt/check 过，warning 持平；macOS 缺陷②同源已写 MACOS-HANDOFF.md。
详情：outbox/coder-1/result.md。

---

### 🟡 SECRET-105 已交付待验收（coder-1，2026-09-05）

密钥闸门 diff 行首标记误报+漏报双修，只动 `scripts/git-hooks/` 3 文件：
`scan_diff` 入口剥标记（修 `+@pytest` 误报）+ 文件头排除改白名单（修 `++` 开头内容行漏报）
+ `hook_diff()` -c 钉死 a/b 前缀（防用户 diff 配置改变文件头形态）。真阳性 7 + 真阴性 7
两钩子全矩阵实测过；f38bc06 事故场景无 SKIP 可 commit。详情：outbox/coder-1/result.md。

---

### ✅ BUILD-098 端测结果（Gavin 2026-09-05）—— 7 项过 6，结案 5 条历史待办

| # | 项 | 结果 |
| --- | --- | --- |
| 1 | **托盘退出**（D2D-HANG-095，本批 P0） | ✅ **OK** —— 端到端确认，P0 结案 |
| 2 | 宽度扩展丝滑（OVERLAY-086 Bug 3） | 🔴 **两个新 bug**，见下 OVERLAY-101 |
| 3 | 开嗓瞬间空窗口（OVERLAY-086 Bug 2） | ✅ OK，结案 |
| 4 | 流式文字细腻度（D2D-P1） | ✅ OK，D2D 方向再次确认，结案 |
| 5 | 超长文字滚动行为 | ✅ 行为 OK；**但要把 65% 改 50%**，见 OVERLAY-102 |
| 6 | 语音侧 AltGr（HOTKEY-078） | ✅ **OK，结案** —— 最后一个「未知」项清掉 |
| 7 | toggle 连按两次能否停 | ✅ **实机 OK** —— 见下，这条是重要情报 |

🔴 **第 7 项的意义**：`test_hotkey_toggle_stop` 在 E2E 里长期间歇 FAIL，主控此前列了三个
归因方向。**Gavin 实机确认能正常停** ⇒ 产品侧正常，**天平明显偏向 harness/环境缺陷**，
「D2D 绘制阻塞」这个方向本批修完 D2D-HANG 后仍未稳定转绿，**证据不足，不再当主要嫌疑**。
并入 E2E-GATE-103。

---

### 🔴 下一轮任务全景（主控 2026-09-05 整理，Gavin 要求「新 bug + 之前未完成一起排」）

#### 已派发（进行中，文件级零重叠）—— 2026-09-05 21:xx 会话重启后重排

| Worker | 任务 | 内容 |
| --- | --- | --- |
| tester-1 | **TEST-SYNC-105**（阶段三） | OVERLAY-101/102 护栏（`centered_x` 单一居中源 + 0.50 常量 + Bug B 量化偏移 + G5 源码级单一源结构护栏）。占 `src/main.rs` **测试模块** |
| coder-2 | **D2D-P2P3-PLAN-108** | D2D 剩余五态迁移**方案设计文档**（DEC-057 备料，🔴 零代码改动）。占 `collab/drafts/d2d-p2p3-plan.md` |

**为什么 coder-2 这一单只出文档**：P2+P3 实施要动 `src/main.rs` 生产区，与 tester-1 的
TEST-SYNC-105（同文件测试模块）冲突，五阶段禁止并行。先把方案做扎实，
等阶段三/四/五走完，实施单 D2D-P2P3-IMPL-109 立刻可开工。

#### 已完成（上一会话，均已 commit）

| 任务 | commit | 状态 |
| --- | --- | --- |
| OVERLAY-101 + OVERLAY-102 | `4805039` | ✅ 已验收提交。**Bug B 已修、Bug A 未修**（机制未确立，待 `-debug` 日志） |
| SECRET-104 手机号闸门误报 | `b072383` | ✅ 已验收提交 |

#### 🔴 被中断、需重排的任务

| 任务 | 状态 |
| --- | --- |
| **E2E-GATE-103**（tester-1） | **零产出被打断**（`tests/` 无提交、工作区干净）。真因不是终端卡死，是 tester-1 模型档余额耗尽（`Insufficient balance` / OpenCode Zen）＝ `[WORKER-RESTART-MODEL-RESET-001]` 复发。已切档 GLM-5.3-Flash (2x usage) OpenCode Go 并重注入上下文。**排在 BUILD-107 出包之后**（它占 `tests/**`，与本轮 `src/main.rs` 无冲突，但 tester-1 一次只做一单） |

#### 🔴 后续顺位（2026-09-05 Gavin 拍板「一起改完一起出包」后重排，见 DEC-057 补充）

**BUILD-107 取消**。OVERLAY-101/102 不单独出包，与 D2D P2+P3 合并成一个包、只端测一轮。

| 顺位 | Worker | 任务 | 占用文件 |
| --- | --- | --- | --- |
| ① ✅ 完成 | tester-1 | `TEST-EXEC-106` 阶段四全量回归 + A1~A4 消融 | 已验收提交 `f774b26`，1070P/0F/9I |
| ② 🔵 进行中 | coder-2 | **`D2D-P2P3-IMPL-109`** 五态迁移实施（方案已冻结：`docs/D2D-P2P3-PLAN.md`，H1-H12 十二 hunk + 五步顺序） | `src/main.rs` 生产区 |
| ② 🔵 进行中 | tester-1 | `E2E-GATE-103`（4 条 full_pipeline 决定性实验归因 + 修 `_no_hardware` + **ERROR>0 机器闸门**） | `tests/**` + `build-test-guide.md` |
| ③ | tester-1 | `TEST-SYNC-110` 阶段三（P2+P3 护栏） | `src/main.rs` 测试模块 |
| ④ | tester-1 | `TEST-EXEC-111` 阶段四全量 | — |
| ⑤ | tester-1 | **`BUILD-112` 出包** | — |
| ⑥ | Gavin | **一次端测**：Bug B 位置 + 五态视觉 + **带 `-debug` 抓 Bug A 日志** | — |
| ⑦ | — | per-pixel alpha（DEC-056 ③，前置条件「八态全迁完」本批达成） | — |

**② 的并行是安全的**：`src/main.rs` 生产区（coder-2）与 `tests/**` python 层（tester-1）
文件级零重叠。阶段三 `TEST-SYNC-110` 必须等 ② 的 coder-2 完成后才派（同文件）。

#### 🔴 A4 消融关键发现（TEST-EXEC-106，2026-09-05）—— Bug B 无自动护栏

tester-1 把「Bug B 本体复原」拆成两个形态实测：

| 形态 | 结果 |
| --- | --- |
| **A4-1** 历史忠实形态（内联公式抄回 + `!do_it` 沿用 `resolved_pos`） | **G5 红** |
| **A4-minimal** 语义本体形态（保留 `centered_x` 调用，但只收进 `do_it` 分支） | **五条全绿** |

🔴 **结论**：Bug B 的语义本体「调用点是否重算 x」**没有任何自动护栏能测**，
唯一真实验证仍是 Gavin 端测目视（DEC-055 红线 5）。
G5 抓得住「内联公式抄回」，抓不住「调用被 gate 掉」，两者都是可能的回归路径。

**主控裁定**：tester-1 建议的「把 x 来源决策抽成可测纯函数」**不并入 IMPL-109**
（合并批里几何区必须干净，见 DEC-057 补充），另开 **`REFACTOR-113`** 排在
`BUILD-112` 出包端测之后。

#### 卫生债追加（2026-09-05）

- **`REFACTOR-113`**：抽 Show 流式分支的 x 来源决策为纯函数 + 补护栏，
  让 A4-minimal 形态可被机器捕获。排在 BUILD-112 端测之后

- **`draw_editing_overlay_chrome`（`src/main.rs:2809`）是死代码**：编译器 `never used`
  警告实证，全文件仅定义体一处 grep 命中，无调用方。
  **D2D-P2P3-IMPL-109 明令不删不改不参考**（删死代码与迁移零关系，混批污染归因）。
  单独一批处理，**待 Gavin 定夺**

##### OVERLAY-101 —— Gavin 原话与主控静态取证

> Bug A：宽度扩展过程**偶发抖动，突然变很长又很快缩回**
> Bug B：**宽度到上限后**、文字左滑时，**窗口位置瞬跳、向右窜**

🔴 **主控已定位一处确凿的源头不一致**（静态取证，非推测）：
`src/main.rs:1396` 的 `SetWindowPos` —— **尺寸用在飞的 `current_size`，
位置 `computed_pos` 却来自 `adjust_overlay_pos_size_for_text`（`:4241`）用 clamp 后的
target 宽算的居中 x**。增长期 `w_target > w_current` ⇒ Show 把窗口摆偏左，
插值循环（`:1765`，用 `current_width` 重算 x）又推回右边 ⇒ **两处每帧拉扯 = 水平窜动**。

**这个契约测试里早标成缺口**：`src/main.rs:9123` 注释原文
「真实契约 = **『用于居中的宽度 == SetWindowPos 应用的宽度』**」，TEST-SYNC-087 列为判别力缺口。

**Bug B 第二条线索**（给方向不给结论）：插值循环整块被
`if state.current_size != state.target_size`（`:1739`）门控 ——
**到上限后 `current == target`，整块含 x 重算全部不执行**，此后只剩 Show 在设位置。
时间点与 Gavin 描述吻合，但**主控无实测证据**，已要求 coder-2 自证不许顺手宣布。

**Bug A 是宽度异常不是位置异常**，上面那条解释不了它。已要求逐环取证：
`measure_text_width` 用的是**全量文本**而渲染画的是 timeline 揭示部分（是否同一段？）／
`[ASR-SERVER-REWRITES-TIMELINE-001]` 服务端重写时间轴会不会让 target 瞬时算大。

#### 排队中（按依赖顺序，不可调换）

| 顺位 | 任务 | 说明 / 阻塞点 |
| --- | --- | --- |
| ① | **D2D P2+P3 五态合并迁移**（DEC-057 ②） | `Recording` / `FallingToProcessing` / `Error` / `StreamingEditing` / `FocusLost`。**必须等 OVERLAY-101 落地**（同一 `src/main.rs` 路径，文件冲突） |
| ② | **per-pixel alpha**（DEC-056 ③） | **必须等八态全迁完** —— 残留 GDI 绘制会在预乘 alpha 位图上打透明洞。解 **处理中态圆角灰线**（DEC-056 补充一，Gavin 已拍板等它）+ OVERLAY-062 编辑态文字粗糙 |
| ③ | **OVERLAY-069** 收缩淡出关闭 | 地基 = 086 定型后的插值机制 + P1 的 D2D 化。**OVERLAY-101 改完插值后方案才能定** |
| ④ | **OVERLAY-065** 动态麦克风图标 | 等 P2（`Recording` 态）打通后另开单 |

#### 阻塞中（等 Gavin，不占 Worker）

| 项 | 阻塞点 |
| --- | --- |
| **FMT-072 / LLM 可用性** | DeepSeek key 已吊销，**新 key 未配则 LLM 401**，格式化全线验不了。配好后告知主控即可单独安排 |
| ~~git 历史是否重写清除泄露 key 痕迹~~ | ✅ **已决：不重写**（Gavin 2026-09-05「那 git 先不要重写吧」）。理由见下 |

#### ✅ 已决：git 历史**不重写**（Gavin 2026-09-05）

**经过**：Gavin 先说「git 历史需要重写」，主控按不可逆操作纪律**没有当轮动手**，
而是先摆代价与前提，随后 Gavin 改为「那 git 先不要重写吧」。**以后者为准。**

**主控当时摆出的三条代价**（记录在案，避免以后重复讨论）：

1. 🔴 **重写会让 `f58af96` 之后所有提交的 SHA 全部改变**，
   而 CHANGELOG / logs / decisions / todo 里引用了大量 commit SHA 当基线锚点
   （`4970309`、`613b49b`、`38eb289`…）⇒ **这些引用会全部失效，历史追溯断链**
2. **key 已吊销** ⇒ 本次重写是**卫生不是安全**，没有实际风险在减少
   （`lessons.md` 已记：吊销才是唯一真止血，顺序不能倒）
3. 若 macOS 端或别处存在 clone，**force push 会打坏它**，需重新 clone

**结论**：`f58af96` 历史里的明文从此是**失效字符串**，保持现状。
🔴 **除非 Gavin 再次明确授权，任何 Agent 不得重写历史 / force push。**

#### 卫生债（低优先，记录在案）

- **`.gitattributes` 覆盖面不足**：`core.autocrlf=true` 但 `.gitattributes` 只管
  `scripts/git-hooks/**`（SECRET-082 只堵了钩子那个洞）。其余 321 个跟踪文件工作区是 LF，
  **新 clone 一次会被检出成 CRLF**，此后任意编辑都产生整文件级 diff。
  建议补 `* text=auto eol=lf`，**待 Gavin 定夺**（改动面大，需单独一批）

### 🟢 BUILD-098 已出包（2026-09-05 13:36）—— 待 Gavin 端测，重点：托盘退出

- **P0 达成**：托盘退出端到端 **5/5 干净退出**（修复前同手法 5 次 4 挂）。
  每次先触发录音确保 D2D 槽非空 —— **空槽退出不构成证据**（REPRO-094 run4 教训）
- **判别探针**：`grep -c 'D2D-HANG-095' Publish/feiyin-ime.exe` = **1**（BUILD-093 旧包实测 **0**），
  新代码进包有硬证据；对照探针 `D2D-P1` 仍 = 2
- **产物**：`Publish/` main `39cca97c…`（异于 093 `e6f55e0a…`）/ ui `5d292c16…` / crash `ea83504d…`
- **Gavin 端测重点**：**先说几句话让浮层真画出来，再点托盘退出**；直接启动就退不算数

### 🔴 E2E 门禁真相：5F 全部非本批引入，但暴露门禁一直是虚的（主控 2026-09-05 取证）

BUILD-098 E2E = **61P / 5F / 33S / 6deselected**，看着比 BUILD-093 的 64P/1F 差。
**主控查证结论：不是回归，是门禁第一次照出真相。**

| FAIL | 归因 | 证据 |
| --- | --- | --- |
| `test_full_recording_pipeline_toggle` | **非本批**，此前根本没跑 | `logs/20260817.md:706` 明载「test_full_pipeline_e2e ModuleNotFoundError toml \| 4 \| 环境缺 toml pip 包」，自 8-17 起这 4 条一直是 ERROR 不是 PASS |
| `test_full_recording_pipeline_ptt` | 同上 | 本批 `pytest_e2e.log` 第一次跑仍是 `ModuleNotFoundError: No module named 'toml'`，tester-1 补装模块后才首次真正执行 |
| `test_recording_cancel_flow` | 同上 | 失败信息 `Notepad should still be running`（notepad 自己退了）= harness 问题 |
| `test_focus_lost_preview_flow` | 同上 | `Expected processing state, got hidden` |
| `test_focus_lost_preview`（test_injection） | **预存测试 bug，8-17 已在案** | `logs/20260817.md:705`「test_injection `_no_hardware` AttributeError \| 1 \| 预存 harness 缺陷」 |

**⚠️ 两次数字不可直接比**：BUILD-093 跑的是 `-m "not hardware"`（7 deselected），
本次 6 deselected，**选集不同**；且本次多装了 `toml` 模块，4 条从 ERROR 转为真执行。

#### 🔴 真正值得警惕的是这个，不是那 5 个 F

**E2E 出包门禁三周来一直是虚的。** BUILD-085 / BUILD-093 报的「64P/1F」「65P/0F」看着漂亮，
但 **full_pipeline 这 4 条最有价值的端到端用例根本没在跑**（ERROR 不计入 FAIL），
`test_injection` 那条也没跑。门禁的实际覆盖比数字显示的少 5 条，且**少的正是全链路那几条**。

#### 4 条 full_pipeline 失败是不是真 bug —— **未知，需单独查**

失败信息集中在 `Expected processing state after stop, got hidden`。
两个方向，**主控倾向 harness/环境，但不作定论**：

1. **强先例**：`logs/20260817.md:703` 记过 6 条 test_hotkey 同样报「overlay hidden」，
   经决定性实验证明是 **harness 缺陷（`[E2E-CONFIG-PATH-STALE-001]`），产品正常**
2. 自动化环境无真实麦克风输入 → 录到静音 → 无转写内容 → overlay 直接隐藏而不进 Processing。
   佐证：同文件其他类有 `_no_hardware()` 跳过守卫，full_pipeline 这几个类没有
3. 另一个待排除项：DeepSeek key 已吊销，若新 key 未配置则 LLM 401（todo 既有待办）

#### 下一棒：E2E-GATE-099（待 Gavin 端测后派）

- 判定 4 条 full_pipeline 失败是产品缺陷还是 harness/环境缺陷（**必须做决定性实验，不许推断结案**）
- 修 `test_focus_lost_preview` 的 `_no_hardware` 预存 bug
- 把 `toml` 依赖写进 E2E 环境要求，**并加一条「ERROR 数 > 0 即门禁不通过」的判据** ——
  否则同类静默失效还会再来一次

### ✅ DEC-056 补充一：处理中态灰线不回退软边，直接等 per-pixel alpha（Gavin 2026-09-05 拍板）

Gavin 端测截图反馈「处理中窗口边沿灰线还是粗乱的」。**非回归、非漏做** ——
OVERLAY-086 Bug 1 验收原文即「消除楔形**保灰线**」（CHANGELOG:722），当时已下调目标。

机制（主控实读代码）：`src/main.rs:2139` 用 `SetWindowRgn` 半径 16 的**二值掩码**
去卡 D2D 画的**抗锯齿**描边（`:3163` `corner_radius=16.0`），
掩码无中间态而抗锯齿恰好活在被切掉那圈 → 断续粗灰点。
窗口仅 36px 高、半径 16 → 上下弧几乎相接近胶囊形，**阶梯在近垂直弧段最扎眼**。

回退 `None` → 软边回来但楔形也回来。`LWA_COLORKEY`、单态临时切 `UpdateLayeredWindow`
两条绕法均已否决。**唯一真解 per-pixel alpha，必须等八态全迁完。**

**Gavin 决定**：不为此单独出包二选一（原话「先不试了，按照你建议来」）。
→ **下一批直接冲 P2+P3 五态合并迁 D2D，迁完立刻接 per-pixel alpha**，DEC-057 排期提到最优先。

### 🔴 D2D-HANG-095 · 根因修复（coder-2，2026-09-05 派发，进行中）

**根因已由 REPRO-094 实证锁死，不是推测**：Rust `thread_local!` 析构器在 Windows 上
运行于 `DLL_THREAD_DETACH`，**加载器锁已被本线程持有**；此时做 D2D/DWrite 最后一次
COM Release → **自持加载器锁死锁**（BS4 抓到 `LoaderLock.OwningThread` = 挂死线程自身 tid）。

- 探针 A 独立 exe 主线程 **正常**（11ms）／ B 子线程退出 **挂死**在 `[Drop 6/6] brush Release`
  ／ b2·b5 线程体内释放 **正常** ／ b6 反向序 **照样挂**（与顺序无关）／ b3 `CoInitializeEx` **不救**
- **端侧「托盘退出无响应」同源路径**：`shutdown_and_join()` `:937-947` 主线程 `join.join()`
  死等 overlay 线程结束 → 它结束前必跑那批析构 → 主线程永不返回
- **修法**：`mod d2d` 加 `release_resources()`（`take()` 就地 drop）+
  `spawn_overlay_thread` 线程闭包尾部调用（闭包是唯一覆盖全部 `?` 早返回路径的位置）
- **硬判据**：产出源盘点（还有哪些线程会写这个 thread_local），答不出不许交付
- 任务书：`collab/inbox/coder-2/task.md`

### 在途任务（2026-09-05）

| Worker | 任务 | 阶段 | 状态 |
| --- | --- | --- | --- |
| tester-1 | **REPRO-094** D2D-HANG-001 根因定位 | 定位 | 🔄 探针 A/B/b2/b3/b5/b6/BS4 已出结论，探针 D 端侧复测进行中 |
| coder-2 | **D2D-HANG-095** 根因修复 | 阶段一 | 🔄 方案已同意，执行中 |
| coder-1 | — | — | 待命 |

**边界评估**：coder-2 占 `src/main.rs` + `docs/MACOS-HANDOFF.md`；
tester-1 占 `collab/outbox/tester-1/repro094/`（独立 cargo 工程，不入库）。**文件级零重叠。**
`collab/troubleshooting.md` 本批归 **tester-1**（证据在它手里），coder-2 禁写，防 `[WORKER-DOC-OVERWRITE-001]`。

**下一棒**：coder-2 验收通过 → 阶段三 `TEST-SYNC-096`（tester-1：去掉两条 `#[ignore]` +
在测试里显式调 `release_resources()` + 加护栏）→ 阶段四全量回归 → 阶段五出包。

### 🔴 REPRO-094 · D2D-HANG-001 根因定位（tester-1，2026-09-05 11:2x 派发，进行中）

**Gavin 2026-09-05 指示「派吧」**。P0：NULL HDC 调 D2D 在 `cargo test` 内挂死，
两条用例现靠 `#[ignore]` 绕过；疑与端侧「点托盘退出无响应只能 kill」同源。

- **本单是定位不是修复**：产出证据 + 根因判定，`src/**` `ui/**` 零改动
- **四探针**：A 主线程复刻 `create_resources` ／ B 子线程 + `join()`（验 thread_local COM Release 阻塞）
  ／ C `cargo test` harness 内对照（验环境差异）／ D 端侧托盘退出 ×5 复测
- **四假设**：H1 线程退出 COM Release 阻塞 ／ H2 `CreateTextFormat` 字体集合阻塞
  ／ H3 test harness 并行 + DWrite SHARED 单例锁 ／ H4 全进程无 `CoInitialize`
- **主控实读取证**：`create_resources` 在 `src/main.rs:2973-3045`，
  「CreateDCRenderTarget 之后」= 阻塞落在 `CreateTextFormat` 或其后；
  `thread_local` D2D（:3047）在线程尾部 drop，而 `shutdown_and_join()`（:937-947）
  主线程 `join.join()` 阻塞等它 —— 这是「托盘退出无响应」的直连路径；
  全仓 `src/` 下 `CoInitialize` 命中数 **0**
- **下一棒**：定位结论出来后派 `D2D-HANG-001-FIX` 给 coder-2（届时 tester-1 转阶段三 TEST-SYNC）
- 任务书：`collab/inbox/tester-1/task.md`

### ⚠️ 2026-09-05 主控 session 启动发现

- **tester-1 模型余额耗尽**：OpenCode Zen（DeepSeek V4 Flash）`Insufficient balance`，
  启动上下文注入失败。主控已切至 **GLM-5.3-Flash (2x usage) / OpenCode Go**（与两个 coder 同源）后恢复。
  → 复发即查此条，不要误判为 Worker 僵死（对照 `[WORKER-RESTART-MODEL-RESET-001]`）
- **handoffs.md 已归档**：299 → 153 行，09-03 的 9 条移入 `handoffs-archive.md`
- **本地领先 origin/main 7 个提交未 push**，等 Gavin 明确指示

### ✅ 阶段五 BUILD-093 已出包（tester-1，2026-09-05 00:37）—— 待 Gavin 端测

- **BUILD-093**：BUILD-085 后五批（OVERLAY-086/D2D-P1/REFACTOR-088+089/测试护栏）进 exe，七项核验全 PASS（sha 全异于 085、ProductVersion 0.9.0.0、探针 D2D-P1×2+D2DERR_RECREATE_TARGET×1）；E2E 64P/**1F**（test_hotkey_toggle_stop 间歇性 FAIL，主控裁定不阻塞出包）；冒烟无 panic；产物在 `Publish/`
- **Gavin 端测清单**（主控已列）：①宽度扩展丝滑度 ②流式空窗口（开嗓瞬间）③处理中圆角灰线 ④D2D 文字/边沿细腻度 ⑤语音热键录 AltGr ⑥文字超屏宽 65% 省略号行为变更 ⑦**toggle 连按两次能否正常停**（E2E 间歇 FAIL 的实机验证点）
- **D2D-HANG-001**：NULL HDC 调 D2D 挂死（CreateDCRenderTarget 之后），Gavin 授权跳过出包阻塞；P0 待办，疑与端侧托盘退出无响应同源

## 🔴 2026-09-04 —— 当前状态（最新在最上）

### ✅ 阶段四 TEST-EXEC-081 收口（tester-1）—— 全量回归通过 + TEST-FIX-084 注释改正完成，等主控出包指令

- **TEST-EXEC-081**（验收通过，主控裁定）：root 1054P/0F（4 条修红转绿）、src-tauri 76P、vitest 89P/0F（S12 红转绿）；必查 A/B/C 全过；消融 B 预期命中，消融 A 停手报告 → 主控裁定归因成立、判红基准改 S12（078 护栏判别力由 S12 实测证实）；pytest 按任务书 SKIP（Publish/ 旧包，E2E 门禁挪阶段五出包后）
- **TEST-FIX-084**：T4b 消融注释改正（+5/-3 仅注释，it() 零触碰）；另 4 条（T1/T2/T3a/T3b）消融推演逐条复核全部成立零改动；tsc 0 error
- 详情见 CHANGELOG.md TEST-EXEC-081 / TEST-FIX-084 ／ handoffs.md 2026-09-04 ／ logs/20260904.md
- ~~下一棒：**阶段五 BUILD v0.9.0**~~ → **BUILD-085 已完成**（2026-09-04 14:23，七项核验全过、ProductVersion 0.9.0.0、E2E 65P/0F 与 BUILD-022 逐位一致，产物在 `Publish/`）

### 🔴 BUILD-085 已出包 —— 待 Gavin 端测三重点（主控 2026-09-04 列）

1. 🔴 处理中态 overlay 文字与圆角边沿是否变细腻（DEC-055 红线 5，D2D-073-P0 验收判据，**不接受静态论证结案**）—— 达标才继续迁剩余五态
2. AltGr 当热键：语音侧/翻译侧各录一次，应显示 `Right Alt` 而非 `Left Ctrl`
3. LLM 401 若仍在 = DeepSeek key 吊销后新 key 未配置，**不是格式化逻辑坏了**，别误判 FMT-072

### 阶段三 TEST-FIX-080 ✅ 主控验收通过（tester-1）

- 四条错测试修正 + 5 条 AltGr 护栏，`+190/-42` 全在测试模块内，**生产代码零改动**（逐 hunk 行号核对）
- 主控独立验证：`cargo fmt --check` exit 0 ／ `cargo check --all-targets` **0 error** ／ `npx tsc --noEmit` **0 error**
- 详情见 CHANGELOG.md TEST-FIX-080 ／ handoffs.md 2026-09-04 ／ logs/20260904.md
- 🔴 tester-1 漏更 CHANGELOG／handoffs／todo 三处（`[DOC-STATE-DRIFT-001]` 第四次复发），**已由主控回填**；
  `progress.md` 按其规则 7「测试同步任务不记录」属 N/A，不算漏。
  阶段四任务书已把「五文档逐条打钩」写进**完成判据**，不再只靠通用规则。

### ✅ Gavin 已处理：DeepSeek key 已吊销（2026-09-04）

**SECRET-077 止血完成。** 泄露的 key 已在服务商后台吊销 —— 这是唯一的真止血，
`f58af96` 历史里的明文从此是**失效字符串**，不再是安全问题。

剩余两项**降级为卫生/可用性问题，不再阻塞**：

1. 是否重写 git 历史 + force push 清除痕迹 —— 破坏性操作，**未获授权不执行**，Gavin 说了算。
2. **LLM 401 是否已解决** —— 新 key 配上后需确认 LLM 可用，否则 FMT-072「有序列举格式化失败」
   出包端测时仍验不了（该项真因就是 LLM 根本没调用成功，不是格式化逻辑坏了）。
3. 每个新 clone 需执行一次：`git config core.hooksPath scripts/git-hooks`（钩子本体已入库不会丢）。

### 在途任务（2026-09-04）

| Worker | 任务 | 阶段 | 状态 |
| --- | --- | --- | --- |
| tester-1 | **TEST-EXEC-081** 全量回归 + TEST-FIX-084 注释改正 | 阶段四 | ✅ 完成（回归逐位吻合 + 消融偏差裁定收口 + 注释改正） |
| coder-1 | **SECRET-082 + FIX** pre-push 密钥/隐私闸门 + `.gitattributes` CRLF 修复 | 独立基建 | ✅ 主控验收通过（13/13 用例独立复验） |
| coder-2 | — | — | 待命（回归期间不得动 src/ 与 ui/） |

**边界评估**：tester-1 占 `src/**`、`ui/**`；coder-1 只碰 `scripts/git-hooks/**` + `.gitattributes`
+ troubleshooting/worker-guide 文档。**文件级零重叠**，可并行。

#### SECRET-082 的由来（Gavin 2026-09-04 问「push 会扫吗」）

主控查证结论：**不会。闸门在 commit，不在 push。**
`core.hooksPath=scripts/git-hooks` 已生效，但目录里只有 `pre-commit` 一个文件，
`.git/hooks` 无自定义钩子 —— **push 这一步是裸奔的**。

两个缺口：① 只认密钥形态（`sk-`/`ghp_`/`AKIA`/`AIza` 等），**邮箱/手机号/身份证一概不拦**；
② 只在 voice-ime 仓库生效，CodeLab 下其他仓库没配。

**附带发现的真隐患（一并修）**：`core.autocrlf=true` + 无 `.gitattributes`
→ 新 clone 会把钩子脚本检出成 CRLF → `bad interpreter: ^M` → **钩子静默失效**。
当前机器工作区副本恰好还是 LF 所以没暴露，但任何人新 clone 一次闸门就是坏的。

**硬性前提（已取证）**：`origin/main..HEAD` 25 个待推提交，密钥/邮箱/手机号/身份证命中 **全 0**；
`f58af96`（泄露那个提交）**已在 origin/main 内**，不落待推范围 —— 这是设计如此，
否则它会让此后每一次 push 被永久拦死。装完钩子 `git push --dry-run` 必须放行。

### ✅ v0.9.0 端测第一轮 —— Gavin 已完成（2026-09-04）

🔴 **主控状态跟踪更正**：此前 todo 把「Gavin 端测」列为待办，**错了**。
Gavin 2026-09-04 明确：「0.9.0 我已进行了一轮端侧，刚提交的三个问题就是测试结果」。
**端测已完成，OVERLAY-086 的三个缺陷就是这一轮的产出。**

#### 本轮结论

| 项 | 结论 |
| --- | --- |
| **D2D-073-P0 处理中态**（DEC-055 红线 5） | ✅ **通过**。Gavin 原话「整体效果满意」，D2D 方向确认，剩余状态迁移已排 P1/P2/P3 |
| 处理中态圆角灰线 | ⚠️ 收尾瑕疵 → OVERLAY-086 Bug 1（**不阻塞迁移**，他明说整体满意） |
| 流式文字中断/空窗 | 🔴 → OVERLAY-086 Bug 2 |
| 宽度扩展抖动/左移 | 🔴 → OVERLAY-086 Bug 3（**他要两边同时扩展 + 丝滑**，居中锚点保持不变） |

**能推出的事实**：Bug 2/3 都在流式文字上屏路径上，说明本轮**真实使用了流式 ASR 听写**，
且用到了「文字长度超出初始窗口宽度」的场景。这条路径是被实际跑过的。

#### 本轮未被提及、状态仍未知的两项（**不是失败，是没有信息**）

| 项 | 状态 | 说明 |
| --- | --- | --- |
| HOTKEY-079 翻译侧 AltGr | ✅ **通过** | Gavin 2026-09-04 补充确认：「热键设置界面我测试了翻译热键，目前正常了」。079 结案 |
| HOTKEY-078 语音侧 AltGr | ❓ 未知 | 他只点名了翻译热键。语音侧是另一条代码路径（组合键语义 + `altGrSynthCtrlActiveRef` 旗生命周期），**不能由翻译侧通过推断语音侧也通过**。下一轮端测顺手录一次语音热键即可 |
| FMT-072 / LLM 可用性 | ❓ 未知 | DeepSeek key 今日已吊销，新 key 若未配置则 LLM 401，格式化全线不生效。**这一项在新 key 配好前无法验证**，不是本轮遗漏 |

**主控不再把「端测」整体列为待办**；上述两项按单项跟踪，不重复催。

### 📦 下一次出包的范围（Gavin 2026-09-04 指示）

Gavin 原话：「**等这次所有任务完成出包，我再进行下一轮端侧**」——
即他不想为零碎改动反复端测，要攒成一个有分量的包再测。

**主控建议范围：OVERLAY-086 + D2D P1，合成一个包。**

理由：086 的 Bug 2/3 与 D2D P1 **落在同一条代码路径**（`RecordingWithText`）。
拆成两包意味着同一段代码改两遍、端测两轮，第二遍还可能把第一遍刚写的推翻。

🔴 **但必须防住 DEC-055 红线 4 警告的「归因不可能」**，做法是**串行 + 中间 commit 隔开**：

```
① coder-2 交付 OVERLAY-086 → 主控验收 → 单独 commit（这是二分锚点）
② 在该 commit 之上派 D2D P1 → 验收 → 单独 commit
③ 一次出包，两批都在里面
```

这样端测若发现异常，可用 `git bisect` / 逐 commit 回退定位到底是 086 的运动逻辑
还是 P1 的 D2D 绘制。**两批视觉特征本就不同**（086 = 位移/空窗；P1 = 文字锐度/边沿），
现场大概率能直接分辨。

**若 Gavin 更想早点拿到 086 的修复**，也可以 086 单独出一包先端测 —— 代价是
P1 之后要再出一包、再端测一轮。**这个取舍归 Gavin 定，主控默认走合成方案。**

### 🎬 OVERLAY-069 · 上屏文字窗口收缩淡出关闭（Gavin 2026-09-04 追问后排入）

**现状取证（主控实读代码，非凭记忆）**：**未实现，零代码**。
`OverlayCommand::Hide` 分支（`src/main.rs:1449-1495`）清完状态直接
`ShowWindow(hwnd, SW_HIDE)` —— 一句硬隐藏、瞬间消失，全文件无任何 fade/淡出/收缩逻辑。
文档侧一致：`CHANGELOG.md` 与 `progress.md` **零条 069 记录**（= 从未完成），
git log 里唯一提到 069 的提交是 DEC-055 的文档更正，不是实现。

**为什么拖到现在**：069 是 Gavin 2026-08-30 端测 13 项之一，与 OVERLAY-062（字体粗糙）、
OVERLAY-065（动态麦克风图标）被判**同源** —— 都卡在 GDI 管线上限（CPU 栅格化 + 位图搬运，
逐帧重绘成本高，做不了高频低成本动画）。DEC-055 当时决定**不在 GDI 上打第三次补丁**，
先迁 D2D 再做动画。**是刻意排序不是遗漏**，只是迁移本身被 ASR-074 / OVERLAY-075 /
HOTKEY 系列插队拖了一个月。

#### 排期位置

```
OVERLAY-086（三缺陷）
  → D2D P1（RecordingWithText 迁 D2D）   ← 069 的地基
  → OVERLAY-069（收缩淡出关闭）           ← 本条
```

#### 🔴 派发前必须先定的技术前提（**不许现在写死方案**）

窗口现为 `WS_EX_LAYERED` + `LWA_ALPHA`（**整窗统一透明度**，`src/main.rs:1340` 附近）。

| 效果 | 依赖 | 现在能不能定 |
| --- | --- | --- |
| 纯淡出（只降 alpha） | 直接调 `SetLayeredWindowAttributes`，D2D 之前就能做 | 能 |
| **收缩 + 淡出**（Gavin 要的） | 尺寸与 alpha 同时变 → **必须接上宽度插值那套机制** | ❌ **不能** |

🔴 **而那套插值机制 OVERLAY-086 Bug 3 正在重写**（`:1666` 变宽 snap 改插值 + `:1334`
流式分支 snap 的处置）。**069 的实现方案取决于 086 最终把插值改成什么样。**

**所以本条的派发条件是：086 交付并验收后，主控依据其最终形态出方案再派。**
现在写死方案 = 大概率白写，且会导致**两套动画逻辑并存迟早打架**（复用同一套插值是硬要求）。

#### 复用要求（写进将来的任务书）

- 复用 086 定型后的插值机制，**禁止另起一套动画循环**
- 关闭动画期间的 `SetWindowPos` 落点必须唯一（OVERLAY-068-B 的老教训：两处争抢 = 闪烁）
- 不得动 `InvalidateRect` 的 `bErase=false`（OVERLAY-043 红线）
- `Hide` 分支现有的两条保护不许破坏：ASR-038-C（编辑态不许拆 EDIT 控件）、
  OVERLAY-054-A（`RestoreAndHide` 显式允许拆）

### 🎨 per-pixel alpha 改造（DEC-056，Gavin 2026-09-04 拍板）

Gavin：「效果一定好，视觉体验一定要棒，这关乎到用户体验，现在硬件性能很强大」。
**视觉是产品目标，不是性能预算的余数。** DEC-055 补充二「帧率 < 30fps 才换路线」判据**已作废**。

🔴 **主控原先把这条路线定性错了**：它不是性能优化，是**视觉能力的天花板**。
`LWA_ALPHA` 整窗统一透明度下，圆角要么留背景楔形（= OVERLAY-086 Bug 1 本体），
要么被 `SetWindowRgn` 二值掩码切成硬阶梯（= DEC-055 要消灭的「粗糙」）。
**两条路都到不了「棒」，与帧率无关，再快的机器也解不开。**

#### 顺序是硬的，不可调换

```
OVERLAY-086      → Bug 1 只能做到「消除楔形」，圆角仍非真抗锯齿（已通知 coder-2 下调目标）
D2D P1/P2/P3     → 八状态全迁完，GDI 绘制清零
per-pixel alpha  → UpdateLayeredWindow 改造        ← Bug 1 的彻底解
OVERLAY-065/069  → 动态图标 / 收缩淡出
```

🔴 **为什么必须等 D2D 全迁完**：GDI 绘制函数（含 `DrawTextW`）不维护 alpha 通道，
会把写过的像素 alpha 置 0，在 `UpdateLayeredWindow` 合成时**该区域整块透明消失**。
残留一处 GDI 绘制，整个 per-pixel alpha 改造就是坏的。

**这给「尽快完成 D2D 迁移」加了一条新理由**：它不再只是为了字更清楚，
而是 per-pixel alpha 的**前置条件**。迁移不完成，视觉天花板打不破。

#### 派发前待实证（勿凭推断直接派）

主控倾向判断**可能不需要新增 Cargo feature**（32bpp DIB section +
`ID2D1DCRenderTarget` 以 `ALPHA_MODE_PREMULTIPLIED` 绑其 DC + 全程 D2D 绘制 +
`UpdateLayeredWindow`，现有 Direct2D/DirectWrite 已足够）。
**但这是推断不是实证**，派发前必须实测；若确需新增 feature 属依赖变更，
按 worker-guide 第十节必须派 BUILD 给 tester-1。

#### 验收判据（DEC-055 红线 5 沿用，且更明确）

**圆角边缘在浅色背景上不得看出阶梯或色块** —— 必须 Gavin 目视确认，不接受静态论证。

### 🔴 D2D 迁移排期（Gavin 2026-09-04 指示：「尽快完成迁移到 D2D」）

**前置已达成**：Gavin 端测处理中态后原话「**整体效果满意**」= DEC-055 红线 5
（视觉必须目视确认）**已过**，D2D-073-P0 试点验收通过，方向确认可继续。
遗留的圆角灰线瑕疵走 OVERLAY-086 Bug 1，**不阻塞后续迁移**。

#### 剩余待迁状态（`draw_overlay_to_dc` 共 8 个分支，已迁 1）

| # | 状态 | 现状 | 备注 |
| --- | --- | --- | --- |
| — | `Processing` | ✅ **已迁**（D2D-073-P0） | 保留 GDI 兜底，Gavin 已目视确认 |
| 1 | `RecordingWithText` | GDI | 🔴 **优先级最高**：流式文字上屏，OVERLAY-062「字体粗糙」的正主；且 OVERLAY-086 Bug 2/3 都在这条路径上 |
| 2 | `Recording` | GDI | 录音态 + 波形；OVERLAY-065 动态麦克风图标的地基 |
| 3 | `RecordingStreamingIdle` | GDI | 与 1 同族，建议同批 |
| 4 | `FallingToProcessing` | GDI | 过渡态，与 Processing 相邻，建议紧跟 Processing 之后 |
| 5 | `StreamingEditing` | GDI | ⚠️ 只迁 chrome + 提交按钮；**EDIT 子控件本体不许动**（DEC-055 红线 2） |
| 6 | `FocusLost` | GDI | 含复制/关闭按钮 |
| 7 | `Error` | GDI | 最简单，可与其他批搭车 |

#### 🔴 排期已改为两轮（DEC-057，Gavin 2026-09-04 拍板「按两轮走」）

原三批 P1/P2/P3 各出一包各端测一轮 + per-pixel alpha 一轮 = 这一包之后还有三轮。
Gavin 问「还要再走三轮端侧？」，主控评估后建议压到两轮，他拍板采纳。

| | 新排期 |
| --- | --- |
| ① | **086 三缺陷 + D2D-P1** → 出包 → 端测（**马上，不压这一包**） |
| ② | **P2 + P3 合并**（Recording / FallingToProcessing / Error / StreamingEditing / FocusLost 五态一次迁完）→ 出包 → 端测 |
| ③ | **per-pixel alpha**（DEC-056）→ 出包 → 端测 |

**为什么能压**：P1 已把共用层 + 五原语（chrome / mic / stop / placeholder / streaming_text）
抽出来，P2/P3 剩余五态主要是复用，真正新写的只有波形、提交键、复制/关闭键三个。
归因难度反而降低——出问题大概率在共用原语层，一坏坏一片，特征明显。

🔴 **per-pixel alpha 仍必须单独一批**，不许并进 P2+P3：它是换合成方式不是迁状态，
且必须等八态全迁完（残留 GDI 绘制会在预乘 alpha 位图上打 alpha 洞）。

🔴 **DEC-055 红线 5 不受影响**：每批出包后视觉仍必须 Gavin 目视确认，
本次放宽的是分批粒度不是验收标准。

#### 🔴 排期约束（DEC-055 红线 4：禁止一次全改）

**必须分批灰度，每批出包后 Gavin 目视确认再开下一批**，理由：一次全改则回归归因不可能。

建议分三批：

| 批次 | 内容 | 理由 |
| --- | --- | --- |
| **P1** | `RecordingWithText` + `RecordingStreamingIdle` | 用得最多、Gavin 感知最强；与 OVERLAY-086 同路径，**必须等 086 落地后再动，否则文件冲突** |
| **P2** | `Recording` + `FallingToProcessing` + `Error` | 录音态打通后 OVERLAY-065 动态图标才有地基 |
| **P3** | `StreamingEditing` + `FocusLost` | 按钮/交互最多，放最后 |

#### 🔴 派发前必须先做的两件事（否则 P1 开不了工）

1. ~~`Cargo.toml` 补 feature~~ ✅ **已核查完毕（主控 2026-09-04 实读，非照抄附录）**
   —— DEC-055 实施附录写的「Direct2D / DirectWrite 一个都没有」是 **08-30 快照，已过期**。
   D2D-073-P0 那批已补齐四个：`Win32_Graphics_Direct2D` / `_Direct2D_Common` /
   `_Dxgi_Common` / `_DirectWrite`，均在 `[target.'cfg(target_os = "windows")'.dependencies]`
   段内（天然不影响 macOS 构建）。**P1/P2/P3 走 BindDC 路线无需再加 feature，不构成依赖变更。**
   ⚠️ 唯一例外：若将来改走 `UpdateLayeredWindow` + DXGI 表面（per-pixel alpha），
   需补 `Win32_Graphics_Dxgi` + `Win32_Graphics_Direct3D11` —— 那才是依赖变更，
   按 worker-guide 第十节**必须派 BUILD 给 tester-1**，coder 不得自行构建。

2. **抽出 P0 的可复用地基**
   —— 当前 `d2d` 模块只有 `draw_processing_overlay` 一个函数 + 一套 `D2dResources`。
   迁 7 个状态前应先评估：`create_resources` / `BindDC` / 画刷 / 文本格式
   要不要抽成共用层。**这是架构决策，派发前主控给方案，不许 Worker 边写边攒。**

#### 顺带一并解决的既有缺口

| ID | Gavin 原话 | 依赖哪一批 |
| --- | --- | --- |
| OVERLAY-062 | 编辑态文字粗糙、提交按钮边沿粗糙 | P3（StreamingEditing） |
| OVERLAY-065 | 替换左侧麦克风图标为动态图标 | P2（Recording）打通后另开单 |
| OVERLAY-069 | 窗口关闭要流畅丝滑，不要生硬突然关闭 | 🔴 **已单列排期，见上方专节** —— 位置在 P1 之后（不是 P2/P3 之后），因为它的地基是 RecordingWithText 的 D2D 化 + 086 定型后的插值机制 |

#### 当前阻塞

**P1 必须等 OVERLAY-086 完成**：086 改的 `RecordingWithText` 绘制路径与宽度插值，
与 P1 是同一批代码。两个任务同时开 = 文件级冲突，违反边界评估规则。

### 下一棒顺序（五阶段串行，禁止并行）

```
阶段三 TEST-FIX-080   ✅ 已验收
阶段四 TEST-EXEC-081  ← 当前，全量回归（tester-1）
阶段五 BUILD v0.9.0      测试无明显问题后主控下达「现在可以出包」
       → Gavin 端测
```

**coder-1 / coder-2 阶段四期间保持待命**：工作区躺着未提交的测试改动，
任何人动 `src/main.rs`／`src/audio/mod.rs`／`src/itn.rs` 都会污染回归结果。

🔴 **出包后必须 Gavin 目视确认**：处理中态 overlay 的**文字与圆角边沿是否变细腻**
（DEC-055 红线 5，不接受静态论证结案）。他说达标才继续迁剩余五个状态；不达标则 D2D 方向需重议。

### TEST-EXEC-076 五条 FAIL —— 处置全部闭环

| # | 用例 | 处置 | 状态 |
| --- | --- | --- | --- |
| ① | `itn_071b_legal_time_yidianban_still_converts` | 改测试 → `下午1:30` | ✅ TEST-FIX-080 |
| ② | `itn_071b_legal_time_yidianshiwufen_still_converts` | 改测试 → `1:15` | ✅ TEST-FIX-080 |
| ③ | `asr_074_stop_drain_...abandoned_count` | 改测试（整条重写，慢消费建模） | ✅ TEST-FIX-080 |
| ④ | `stale_generation_must_not_touch_mirror...` | 改测试（改名 + 末条期望修正） | ✅ TEST-FIX-080 |
| ⑤ | `HOTKEY-047-S12` | **真生产缺陷，改生产** | ✅ HOTKEY-078（+ 079 翻译侧同型） |

**①② 的关键事实（别再翻）**：生产零 bug，ITN-071-B 的两条保护用例（一点半点／一点点）都是 PASS 的。
红的只是顺手写的两条回归护栏，期望值凭直觉写成「1点半」，与既有十余条 `X点半→X:30` 护栏冲突。

**④ 的语义裁定（重要，别再翻）**：代际闸门 = 数据+渲染双拦；043 闸门 = **仅渲染**。
同 session 松手后的迟来包是同一句话的更完整版本，**必须**进 `last_streaming_text` 镜像，
否则 WORDBOOK-053-B 会拿截断的 raw 文本去 diff 用户编辑，学出用户从未做过的伪修正。
**渲染抑制 ≠ 数据抑制。**

### 已结案（不要重查）

- **行尾幻影**：coder-2 上报的「69 文件 18K 行 CRLF 漂移」= racy-git 的 index stat 缓存过期，
  一次 `git status` 刷新即自愈。裁定 `core.autocrlf=true` **不动**。
- **OVERLAY-061 冷启动假设**：❌ **证伪**（COLD 3199 帧 + WARM 9831 帧 = 13030 帧，可见态位置异常
  零命中）。061 不是 overlay 窗口的问题，下一步是 REPRO-061-ALLWIN 全窗口清扫（任务书未写）。
- **068 宽度阶梯归属**：`y≈1292 ∧ h=36` 的宽度阶梯族**属 068 不属 061**，两 bug 不得互相污染。
- **068-B 覆盖面**：**不返工**。拉锯真源是跨 session 渗漏，已由 OVERLAY-075 结构性根治。

---

## ✅ 2026-09-03 收工状态 —— 下一棒从这里开始

### 已完成并提交（工作区干净，**本地领先 origin/main 16 个提交，未 push**）

| 提交 | 内容 | 验收 |
| --- | --- | --- |
| `8fbb913` | **ASR-074** 音频上行背压丢帧根治（coder-1）：2-A 排空 chunk_rx / 2-B 松手 drain 带 500ms 上限 / 2-C 丢帧计数 + 三个埋点缺陷 | 主控逐项 Read + 独立 cargo check |
| `8023cc6` | **OVERLAY-075** 跨 session 代际隔离 + **D2D-073-P0** 处理中态迁移 + **ASR-074-GUARD** send_timeout + **HOTKEY-060** 补账 + 主控代修 macOS 元数（coder-2） | 四条红线 diff 逐个 grep 核对 |

### 在途

| Worker | 任务 | 状态 |
| --- | --- | --- |
| tester-1 | **TEST-SYNC-074/075/D2D**（阶段三，只写用例不跑） | 执行中 |
| coder-1 | — | 待命 |
| coder-2 | — | 待命（main.rs 已交给 tester-1，未经协调不得动） |

### 下一棒顺序

阶段三交付 → **阶段四全量回归** → **阶段五 BUILD v0.9.0** → Gavin 端测。

🔴 **出包后必须 Gavin 目视确认的**：处理中态 overlay 的**文字与圆角边沿是否变细腻**（DEC-055 红线 5，
不接受静态论证结案）。他说达标才继续迁剩余五个状态；不达标则 D2D 方向需重议。

### 🔴 两条待 Gavin 处理（主控无权限 / 需授权）

1. **DeepSeek API key 泄露**（详见 troubleshooting `[SECRET-IN-REPO-001]`）：
   `collab/research/ab033-components.json:17` 明文 key，`f58af96`（08-14）已 push 到 GitHub，
   暴露 20 天，Gavin 已确认遭第三方盗刷、有金钱损失。
   **主控已完成全仓扫描（仅此一处）并落盘经验，但吊销 key 只能 Gavin 在服务商后台做。**
   待 Gavin 决定的两项：① 是否把仓库内该 key 替换为占位符并提交；
   ② 是否重写 git 历史 + force push（**破坏性操作，未获授权前主控不执行**）。
2. **LLM 401 导致格式化全线未生效**：Gavin 提到「暂时关闭格式化输出」，语义待确认。
   **FMT-072「有序列举格式化失败」的真因已查明 = LLM 根本没调用成功**，
   出包端测前需确认 LLM 可用，否则该项无法验证。

### 今日已证伪 / 已结案（不要重查）

| 项 | 结论 |
| --- | --- |
| **OVERLAY-061 冷启动假设** | ❌ **证伪**。COLD 3199 帧 + WARM 9831 帧 = **13030 帧，可见态位置异常零命中**；单 HWND 无重建、GetWindowRect 零失败。两处 `ShowWindow` 均在 `SetWindowPos` 之后（主控 Read 核对 + 7 帧实测佐证）。**061 不是 overlay 窗口的问题**，下一步是 REPRO-061-ALLWIN 全窗口清扫（任务书未写） |
| **068 宽度阶梯归属** | 主控裁定：`y≈1292 ∧ h=36` 的宽度阶梯族**属 068 不属 061**，两 bug 不得互相污染 |
| **068-B 覆盖面** | coder-2 核查后主控采纳：**不返工**。拉锯真源是跨 session 渗漏，已由 OVERLAY-075 结构性根治 |

---



## 🛑 2026-08-30 深夜收工存盘 —— 三 Worker 额度全部超限，下次从这里开始

> **恢复条件**：Gavin 的 Ollama 账号额度重置（coder-1 / coder-2 / tester-1 撞的是**同一个账号墙**，非各自故障）。
> **恢复动作**：~~三个 Worker 的 `inbox/*/task.md` 任务书都已写好并派发过，重启后直接 `dispatch <id>`（不带任务内容，仅通知重读）即可，不要重写任务书。~~
>
> 🔴 **2026-09-03 实测更正：上面这条作废。** 三个 `inbox/*/task.md` 在本次重启时**全部被清空为 0 字节**
> （mtime 2026-09-03 17:14），属 troubleshooting.md `[REPLACE-WORKER-TASKFILE-WIPED-001]` 的已知行为。
> **三份任务书必须重写**（ASR-074 / HOTKEY-060 收尾+D2D-073 / OVERLAY-061 冷启动复测），
> 直接 dispatch 只会让 Worker 读到空文件或凭记忆臆造任务。

### 现场（以 git 为准，非记忆）

| 项 | 值 |
| --- | --- |
| HEAD | `d979c95` |
| 工作区 | **完全 clean**，零悬空改动，零未跟踪文件 |
| 版本号 | **v0.9.0**（VERSION-059 已落地） |
| 本 session 提交数 | 9（`9c9ff73` → `d979c95`） |
| 未 push | 🔴 **本地 ahead，Gavin 未授权 push，不得自动执行** |

### 本 session 已完成并提交

| ID | 内容 | 提交 |
| --- | --- | --- |
| VERSION-059 | 版本号 0.8.1 → 0.9.0（Gavin 明确指示） | `9c9ff73` |
| OVERLAY-068 | 位置跳动（两处）+ 交替闪烁；061 屏幕外坐标安全网；064 根因报告 | `920e1d2` |
| ASR-070 | `final_text()` 改返回 confirmed + current | `958cadb` |
| ITN-071 | `三五成群` 补入 idioms + 护栏 2 条；qwen_inference 死代码化简 | `9506eac` |
| ITN-071-B | `一点半点` 挪 idioms、`一点点` 补 function_words + 护栏 4 条 | `e7a6a29` |
| **HOTKEY-060** | 🔴 **见下方「需注意」第 1 条** | 混在 `9506eac` + `e7a6a29` 内 |
| 文档 | ASR-067 澄清与实测分析 / DEC-055 + 实施附录 / REPRO-073 验收 + ASR-074 合并 / 经验条目 | `1628e27` `a53fffb` `a0752e6` `d979c95` |

### 🔴 需注意（下次接手前必读）

**1. HOTKEY-060 已实现并已提交，但从未走完验收流程。**
主控 `git add -A` 时把 coder-2 的改动**误扫进了 ITN 的两个提交**（`9506eac` / `e7a6a29`），
提交信息里**只字未提**，且当时既没等它的完成通知、也没做 review。**这是主控的流程失误。**

事后补审结论（2026-08-30 深夜）：
- 设计**符合主控裁定**：`voiceFinalizedRef` / `translationFinalizedRef` **两侧各自独立**，
  共用 helper `applyHotkeyIfNoDupConflict(finalizedRef, side, ...)`，状态不串、流程只有一份 ✅
- 删除 `TRANSLATION_SINGLE_KEYS` **不是回归**：原代码 `if (!TRANSLATION_SINGLE_KEYS.includes(vkCode)) { applyTranslationHotkey(vkCode); return; } applyTranslationHotkey(vkCode);`
  —— **两个分支完全相同，本就是死代码**，删除行为等价 ✅
  （主控一度误判为回归，查原文后自行更正，记录在此以免下次重复怀疑）
- `npx tsc --noEmit` **0 error** ✅
- 🔴 **仍缺**：coder-2 的 result.md、五文档条目、阶段三 TEST-SYNC。**恢复后补齐，不要当它已验收过。**

**2. 出包尚未进行。** 五阶段停在阶段一。恢复后顺序：
剩余阶段一 → 阶段三 TEST-SYNC → 阶段四全量回归 → 阶段五 BUILD v0.9.0。

**3. `git add -A` 的教训**：本 session 因此把未验收改动混入无关提交。
恢复后提交前先 `git status` 看清改动归属，跨 Worker 的改动分开提交。

### 三个 Worker 的在途任务（任务书已在 inbox，直接通知重读即可）

| Worker | 任务 | 状态 |
| --- | --- | --- |
| **coder-1** | **ASR-074**（ASR-067 + ASR-070 合并单）：在线 ASR 会话中途停止产出 + finalize 拖尾 4-5s | 已 ACK 同意方案，额度中断，**零交付** |
| **coder-2** | ~~HOTKEY-060 收尾~~（✅ 2026-09-03 补账完成）→ ~~OVERLAY-075~~（✅ 2026-09-03 代码交付待验收）→ **D2D-073** 九阶段（P0 依赖+骨架 → P1 处理中态 → P2 错误态 → P3 录音态+波形+065 动态图标 → P4 流式文字态 → P5 编辑态边框 → P6 069 淡出 → P7 063 字号 → P8 066 高度+3px） | Part A+B 已交付，等主控 ACK 后开 D2D P0 |
| **tester-1** | **OVERLAY-061 冷启动定向复测**（无预热 × 5 次冷启动，抓前 300ms，判据=是否出现在 (0,0) 附近；用 `target/release` 旧包，它不含屏幕外兜底） | 已 ACK，额度中断，**零交付** |

**另需 coder-2 顺带确认**（已发消息，未答）：tester-1 实测显示交替闪烁主体是
「长文本 vs Recording 240 宽」拉锯，而 068-B 修的是「流式 → Processing 200」路径，
请核对覆盖面并给行号证据。

### Gavin 已拍板 / 已答复的事项（不要再问）

| 事项 | Gavin 结论 |
| --- | --- |
| 版本号 | 升 **v0.9.0** |
| overlay 渲染方案 | 选 **B：Direct2D + DirectWrite 重写**，否决 GDI+ 局部补丁（DEC-055） |
| D2D 排期 | **放进本次出包**（主控原建议延后，Gavin 否决） |
| FMT-072 有序列举格式化 | **暂时跳过，下次开单**，他要在端侧判定是否 LLM 造成 |
| ASR-067 | 原意等出包后取 debug.log；**但现行 `target/release` exe 已含 ASR-058 埋点，无需等出包即可查** |
| ITN `一点半点` 用例 | 输入「差的不是一点半点」→ 输出「差的不是1点半点」；「我只是有一点点不信」→「我只是有1点点不信」 |
| 显示器 | **没有多显示器**（故 061 未复现不能归因多屏） |
| 067 与 070 是否同时发生 | **是** → 已合并为 ASR-074 |

### 待 Gavin 端测验证的两条（出包后）

1. **ITN-071-B 判读方式已预先写死**：复测「差的不是一点半点」——
   **仍失败** = ITN 已在第 1 步保护却仍被改 → 根因在 ITN 之外（大概率 LLM 回改），ITN 洗清；
   **通过** = 症状消除，第 5 步疑点降为低优先级。
2. **OVERLAY-062/065/069/063/066** 视觉五项，D2D 迁移后逐状态目视确认。


### 🔴🔴 收工前最后一刻的重大更正：ASR-074 的证据链大半被污染

coder-1 断电前交出了 15:09:33 那次 run 的完整时间线（实测，价值很高），
主控顺着它复核，**发现三条证据里有两条不成立**。

**污染源：tester-1 的 REPRO 脚手架按住热键的时间远短于它播放的 TTS 句子。**

| 录音来源 | 时长样本 |
| --- | --- |
| Gavin 真实使用（04:xx / 09:xx） | 8.8s / 29.9s / 11.5s / 27.3s / 13.3s |
| **tester-1 测试（15:xx）** | **2.6s × 8 次、3.9s、4.1s** |

15:09:33 那次「尾部丢字」：口播「今天天气很好我们下午三点在公司门口集合」
（正常语速 5-6 秒），而 **`recording complete, 4.1s`** —— **尾部那半句从未被录进去**。
服务端返回不了它，不是因为服务端截断，而是**麦克风就没收到**。

**逐条重估：**

| 原证据 | 结论 |
| --- | --- |
| ① 主控：「词时间轴只覆盖录音 51.4%」 | 🔴 **不成立**。主控拿词表跨度比 `final_ms`，但 `final_ms` **含松手后等 finalize 的时间**。对 04:21 那次（真实录音 29.9s）：词表 24.5s vs 录音 29.9s，属正常。**分母用错了** |
| ② tester-1：「7 组尾部丢失 5 组，全部服务端未返回」 | 🔴 **大半不成立**。短按导致尾部未录入，服务端「未返回」是必然结果 |
| ③ `ignoring late StreamingText after stop` 持续 4-5s | ✅ **成立**，但 coder-1 已归因为**服务端处理慢**，非客户端缺陷 |

**幸存且更有价值的新发现（coder-1 实测）**：

> 15:09 那次：录音仅 4.1s，而 **`first_text_ms=7144`** ——
> **首个非空文本在音频开始后 7.1 秒才到，即松手之后 0.8 秒**。
> 录音全程（4.1s）服务端只回 `display=''`、`words=0`。
> stop(28.766) → finish-task(32.700) = **3.9s**，客户端一直在等服务端回吐。

**这直接解释了 ASR-067**：录音期间服务端根本还没产出任何文本，
**屏幕上当然什么都没有**——用户感知为「上屏显示中断」。
根因指向**服务端响应延迟极高**，而非客户端停止接收。

🔴 **但仍不能定案**，因为这一样是从被污染的短录音 run 里取的。
恢复后**第一件事**：请 Gavin 用 `-debug` 做一次**受控复现**（正常按住、说完再松手），
取 `[ASR-SUMMARY]` 的 `first_text_ms` / `final_ms` 与 `recording complete` 时长三者对照。

**教训**：本条正是 `[PLAUSIBLE-FIX-NOT-ACTUAL-CAUSE-001]` 的第四个实例
（当天第四次）——主控与 tester-1 都基于「讲得通」的数据建立了因果链，
**没有先校验数据本身是否可信**。已在该经验条目下补记。

### 🔴 遗留未解（不要当已解决）

| # | 问题 | 状态 |
| --- | --- | --- |
| 1 | `一点半点` 在生产中为何未被 check_protection 第 5 步保护住 | **无解**。词 2026-07-30 即入表、`unit_collision_map` 构建与查表逻辑经复核均正确。已改用「把修复当实验」的方式分流 |
| 2 | ITN 保护是**白名单模型**，漏一词出一 bug | Gavin 已撞四个（三五成群 / 一点半点 / 一点点 / **哪一款**——最后一个是主控在 debug.log 里发现、Gavin 尚未报过）。补词只能逐个堵，结构性泄漏未解决 |
| 3 | ASR-074 根因 | 未定。三份证据指向「会话中途停止产出 + finalize 拖尾」 |
| 4 | OVERLAY-061 真根因 | 未定。1044 帧未复现；冷启动假设待验 |
| 5 | `vad_hit_ms` 早期 run 正常(637/644)、tester-1 26 个 run 全 -1 | 🟡 已降级为待验证假设，环境可能有责（26 run 中 14 run 零识别） |

---


## 🎯 v0.9.0 批次 · Gavin 2026-08-30 端测清单（13 项）+ 版本升级 —— 本批汇报以此表为准

> **Gavin 明确要求：「接下来任务完成度，需要按照这些列表来汇报」。**
> 本表是 v0.9.0 唯一的完成度基准，每项状态变化立即回写，不批量补。
>
> **端测包取证**：Gavin 端测的是 `Publish/` 下 **2026-08-18 23:48** 构建的包
> （feiyin-ime.exe 12,188,160 B），晚于 TEST-EXEC-058 过闸提交 `6009aa3`（23:43）
> → **ASR-058 首字提速已在该包内**，该次出包未入账（编号补记为 BUILD-023-未入账）。
> 因此下列 13 项**全部是针对最新代码的真问题**，无一条属于「修了没进包」。
>
> 🔴 **Gavin 2026-08-30 原话确认**：「我重复提交的问题就是之前没有修复好」——
> 061 / 062 / 064 三项是 OVERLAY-054-B-FIX / 054-D / 054-C **修过但没修对**，
> 不是回归，是当初验收放过了。本批必须做到目视确认，不接受静态论证结案。

### 总表

> 🔄 **2026-09-03 21:30 主控按 git 逐项核对更新**（上一版停留在 08-30 深夜，已过期）。
> **全局前提：`Publish/` 仍是 2026-08-18 23:48 的旧包，自那以来 14 个提交一次都没出包
> → 下表任何 ✅ 都只代表「代码已进本地仓库」，Gavin 端没有一项验证过，无一项能标 🟢。**

| ID | Gavin 原话 | 类型 | 优先级 | 文件域 | 负责人 | 状态（2026-09-03 核对） |
| --- | --- | --- | --- | --- | --- | --- |
| **VERSION-059** | 升级版本到 v0.9.0 | 版本 | 前置 | `Cargo.toml` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` | coder-1 | ✅ 已提交 `9c9ff73` |
| **ASR-067** | 输入中间有停顿（约 1 秒），接着输入就无法继续，麦克风不接受输入 | BUG | 🔴 **P0** | `src/audio/mod.rs` + `qwen_inference.rs` | coder-1 | 🟡 **改判两次后已动手**：原「800ms 断句」假设证伪 → REPRO-073 实测定位到**音频上行背压丢帧**，并入 **ASR-074** 已提交 `8fbb913`。**是否真解决必须靠出包端测**，不接受静态结案 |
| **ASR-070** | 说完松开热键后，识别的文本不完整，被丢掉尾部文字 | BUG | 🔴 **P0** | `qwen_inference.rs` `final_text()` | coder-1 | 🟡 **修了一层，不确定够不够**：`958cadb` 修「已到达未断句」的尾部丢弃；但 REPRO-073 实测显示还有一层「服务端 word 流根本没送达」，那一层归 ASR-074 `8fbb913`。同样必须端测判定 |
| **ASR-074** | （067+070 合并单，主控立项）音频上行背压丢帧 + finalize 拖尾 | BUG | 🔴 **P0** | `src/audio/mod.rs` / `qwen_inference.rs` | coder-1 | ✅ 已提交 `8fbb913`（2-A 排空 / 2-B 松手 drain 带 500ms 上限 / 2-C 丢帧计数 + 三个埋点缺陷 + 永久 debug 埋点） |
| **ASR-074-GUARD** | （主控验收 ASR-074 时发现的残留风险）chunk_tx 阻塞 send 会导致松手卡死 | BUG | 🟠 P1 | `src/main.rs` | coder-2 | ✅ 已提交 `8023cc6`（send_timeout 200ms + warn 计数） |
| **OVERLAY-064** | 原来 overlay 窗口的灰色线条边框消失了，必须显示回来，否则窗口缺少质感 | BUG | 🟠 P1 | `src/main.rs` | coder-2 | 🟠 **只出了根因报告，代码没改**（`920e1d2` 内只有分析）。修复动作归 D2D 迁移批，**未做** |
| **OVERLAY-061** | 按下热键，录音窗口会先显示在屏幕左上角边沿，然后跳到屏幕下方正确位置 | BUG | 🟠 P1 | 待定（已不在 overlay 窗口） | 待派 | ❌ **假设证伪，未定位未修**。COLD+WARM 共 13030 帧采样，可见态位置异常**零命中**；`920e1d2` 只加了屏幕外坐标安全网（治标）。下一步 REPRO-061-ALLWIN 全窗口清扫，**任务书未写** |
| **OVERLAY-068** | 长文本时窗口位置跳动；切到「识别处理中」时长短两个窗口交替闪烁 | BUG | 🟠 P1 | `src/main.rs` | coder-2 | ✅ 已提交 `920e1d2`；「交替闪烁」的真源（跨 session 渗漏）由 OVERLAY-075 结构性根治 |
| **OVERLAY-075** | （主控立项）跨 session 流式文本渗漏：A 拖尾期间开 B → 宽窄拉锯 + A 旧文字渲染进 B 窗口 | BUG | 🟠 P1 | `src/main.rs` | coder-2 | ✅ 已提交 `8023cc6`（会话代际 AtomicU64 闸门，+44/-5） |
| **HOTKEY-060** | 录音、翻译热键设置时，重复拦截提示 | BUG | 🟠 P1 | `ui/src/pages/HotkeySettings.tsx` | coder-2 | ✅ 代码已在 `9506eac`+`e7a6a29`，补账已完成。🔴 **这份改动至今一次 Vitest 都没跑过**，正由 TEST-EXEC-076 补跑 |
| **ITN-071** | ITN 错误：`三五成群` / `一点半点` | BUG | 🟠 P1 | `src/itn.rs` + `itn-rules.toml` | coder-1 | ✅ 已提交 `9506eac`（三五成群）+ `e7a6a29`（一点半点挪 idioms、一点点补 function_words，护栏 4 条）。🔴 结构性问题未解：保护词表是白名单模型，漏一词出一 bug，Gavin 已撞四个 |
| **D2D-073-P0** | （DEC-055 落地）overlay 绘制层迁 Direct2D + DirectWrite，处理中态先行 | 优化 | 🟠 P1 | `src/main.rs` | coder-2 | ✅ 已提交 `8023cc6`（骨架 + 处理中态 + GDI 回落）。🔴 **出包后必须 Gavin 目视确认文字与圆角边沿是否变细腻**，他说达标才继续迁剩余五态 |
| **OVERLAY-062** | 点击进入编辑态，文字字体显示很粗糙，提交按钮的边沿也很粗糙 | 优化 | 🟠 P1 | `src/main.rs` | coder-2 | ⬜ **未做**，排在 D2D P5（编辑态边框） |
| **FMT-072** | 明显有序列举内容，格式化输出失败 | BUG | 🟠 P1 | `src/llm/mod.rs` | — | ⏸ **未做**，Gavin 拍板挂起。🔴 真因已查明 = LLM 401 根本没调用成功，**LLM 不修好这项没法验证** |
| **OVERLAY-065** | 替换左侧麦克风图标为动态图标 | 优化 | 🟡 P2 | `src/main.rs` | coder-2 | ⬜ **未做**，排在 D2D P3 |
| **OVERLAY-069** | 松开热键 / 编辑态提交或回车后，窗口关闭要流畅丝滑，不要生硬突然关闭 | 优化 | 🟡 P2 | `src/main.rs` | coder-2 | ⬜ **未做**，排在 D2D P6 |
| **OVERLAY-063** | 笔记框内字体再大一号 | 优化 | 🟡 P2 | `src/main.rs` | coder-2 | ⬜ **未做**，排在 D2D P7 |
| **OVERLAY-066** | Overlay 窗口的高度再增加 3 个像素 | 优化 | 🟡 P2 | `src/main.rs` | coder-2 | ⬜ **未做**，排在 D2D P8 |

### 一句话总账（2026-09-03）

- **代码已落地（本地已提交，未出包未端测）：9 项** —— VERSION-059 / ASR-070 / ASR-074 / ASR-074-GUARD / OVERLAY-068 / OVERLAY-075 / HOTKEY-060 / ITN-071 / D2D-073-P0
- **动了但没做完：2 项** —— ASR-067（并入 ASR-074，等端测判定）、OVERLAY-064（只有根因报告，无代码）
- **完全没做：6 项** —— OVERLAY-061（假设证伪，重新定位中）/ OVERLAY-062 / 063 / 065 / 066 / 069（全排在 D2D 后续阶段）/ FMT-072（挂起，卡在 LLM 401）
- **Gavin 端测通过（🟢）：0 项** —— 因为一次包都还没出

**状态图例**：⬜ 待派发 / 🔵 已派发执行中 / 🟣 已交付待验收 / ✅ 已验收已提交 / 🟢 Gavin 端测通过 / ❌ 打回

### 🔴 ASR-067 症状澄清（Gavin 2026-08-30 补充）——**排查方向整体改向，原判作废**

**Gavin 补充原话**：

> 我在录音时，停顿了 1 秒，然后继续说话，但是**屏幕窗口中语音识别上屏显示就中断了**

**与最初描述的差别（决定性）**：

| 版本 | 描述 | 指向 |
| --- | --- | --- |
| 初报 | 「接着输入就无法继续，**麦克风不接受输入**」 | 音频采集 / ASR 链路 |
| 澄清后 | 「**屏幕窗口中语音识别上屏显示**就中断了」 | **overlay 显示路径** |

**这解释了 coder-1 静态排查为什么查不出问题**：它查的是音频链路
（`qwen_inference.rs`），结论「录音循环不退出、音频不停发、服务端断句不断连」
**全部正确且与新症状不矛盾** —— 音频确实没断，**断的是屏幕上的字**。

🔴 **原判「ASR-067 属 ASR 问题、归 coder-1、文件域 `qwen_inference.rs`」作废。**
新方向指向 `src/main.rs` 的流式上屏揭示逻辑，**文件域改归 coder-2**。

#### 主控的新假设（**未验证，需 debug.log 实证，不许当判据直接改**）

嫌疑落在 `OVERLAY-051-G-FIN` 的时间戳驱动回放
`reveal_chars_by_timeline`（`src/main.rs:3232-3258`）：

```
let origin_begin = words[0].begin_time;
for w in words {
    if w.begin_time - origin_begin <= elapsed_ms { revealed += ...; }
    else { broke_early = true; break; }   // 假设 words 按 begin_time 升序
}
```

两个关键假设写在代码里但**未被验证**：

1. **`words` 全局按 `begin_time` 升序**（`:3248` 注释自陈）。
   停顿 800ms 触发服务端断句 → `sentence_end=true` → 新句以新 id 开始。
   **若服务端的 `begin_time` 是按「句内相对」而非「整条音频流相对」计时**，
   新句的 `begin_time` 会重新从小值开始 → 合并后的 `confirmed_words + current_words`
   列表**不再全局升序** → 循环在句边界 `break`，后续新句的字**永远轮不到揭示**
   → 表现正是「屏幕显示中断，但音频还在流」。
2. **`origin_begin` 取 `words[0]`**（第一句第一个词）且 `tween_audio_origin`
   只在首次收到非空 words 时设定、此后不重置 —— 跨句是否仍成立未验证。

叠加 `displayed_chars` 的 `.max(displayed)` 单调约束（051-G 契约 4），
一旦算出的 `revealed` 低于已显示值，显示就**冻结不动**，与 Gavin 描述吻合。

#### 处置（Gavin 已拍板顺序）

> Gavin 原话：「等下一波开单，我需要等这次出包后端侧输出 debug log」

- 🔴 **本批不动 ASR-067 代码**。等 v0.9.0 出包 → Gavin 端测 → 取 `debug.log`
- **要看的探针**（ASR-058 已埋，本包内可用）：
  - `[ASR-WORDS]` 词时间轴 —— **直接验证假设 1**：把停顿前后两句的
    `text[begin-end]` 序列拉出来，看 `begin_time` 在句边界是否回落
  - `[ASR-SUMMARY]` 12 字段 —— 看 `first_text_ms` / `final_ms` / `words_total` / `chars_total`
- **出包因此进入 ASR-067 的关键路径**：不出包就拿不到日志，拿不到日志就定不了根因

## 🔴 ASR-074 · 在线 ASR 会话中途停止产出 + finalize 拖尾（**ASR-067 与 ASR-070 合并为同一根因**）

**建单人**：主控，2026-08-30，依据 tester-1 REPRO-073 实测 + 主控独立复核。

### 为什么合并

ASR-067（录音中途上屏显示中断）与 ASR-070（松键后尾部文字丢失）
三份独立证据指向**同一处机制**：

| # | 证据 | 来源 |
| --- | --- | --- |
| 1 | 末次长录音 `final_ms=55474`，词时间轴跨度远小于录音时长；多条 run 出现 `words_total=0 / chars_total=0` 而 `final_ms` 正常 → **录音在跑、服务端不产出** | 主控分析 `target/release/debug.log` 28 条 `[ASR-SUMMARY]` |
| 2 | 7 组尾部对照中 5 组丢失，**全部为服务端未返回**（非客户端丢弃） | tester-1 REPRO-070 |
| 3 | `OVERLAY-043: ignoring late StreamingText after stop` 单 run 6+ 次、**持续 4-5 秒** → 松手后服务端仍在缓慢回吐 | tester-1 REPRO-073 新疑点 3 |

**统一解释**：在线 ASR 会话运行中途停止产出识别结果，且 stop 后 finalize 拖尾 4-5 秒。
中途表现为「上屏显示中断」（=ASR-067），末尾表现为「尾部文字丢失」（=ASR-070）。

### 🔴 主控自陈：已提交的 ASR-070 修复**不解决本症状**

提交 `958cadb` 把 `final_text()` 从「只返回 confirmed」改为「confirmed + current」。

**那是一个真实的潜在缺陷，修法也正确，予以保留**
（若服务端确实返回了尾部但未标 `sentence_end`，旧实现会丢；新实现不会）。

**但它不是 Gavin 所报现象的原因。** 决定性反证（主控独立复核 `debug.log` 15:09:33）：

| 环节 | 内容 |
| --- | --- |
| 口播 | 今天天气很好我们下午**三点在公司门口集合** |
| `Transcribed:`（服务端返回） | 今天天气很好，我们下午。 |
| `Injecting text:`（实际注入） | 今天天气很好，我们下午。**与上一行逐字相同** |

注入层零丢失，尾部字**从未从服务端到达**。

🔴 **主控 2026-08-30 曾向 Gavin 报「尾部丢字已修复」，该结论下得过早，此处更正。**
教训：**修了一个能解释症状的缺陷 ≠ 修了造成症状的缺陷。**
定案必须有「修复前后同场景实测对照」，静态因果链再顺也只是假设。
与本批 `[STATIC-PROOF-MISSED-CALLGATE-001]`、ITN-071-B 的「防御性修复」属同类。

### 排查方向（下一棒，**现行 exe 即可查，不必等出包**）

ASR-058 埋点已在 `target/release/feiyin-ime.exe`（08-18 23:48 构建）内。

1. **会话为何中途停止产出**：`qwen_inference.rs` 主循环在长录音下是否仍持续收发；
   服务端是否发过 `task-failed` / 错误帧而被静默吞掉；
   是否存在未处理的 WebSocket 关闭/超时
2. **finalize 为何拖尾 4-5 秒**：stop 后到 `task-finished` 之间发生了什么
3. **`vad_hit_ms` 异常**（🟡 tester-1 已自行降级为待验证假设，不作判据）：
   28 条 `[ASR-SUMMARY]` 中，早期 2 条（04:19/04:21）为 637/644 正常值，
   tester-1 的 26 条（15:xx）**全为 -1**。可能是环境差异（TTS 放音链路）而非缺陷，
   但值得顺带确认 VAD 命中回执路径

### 🟡 数据可信度限制（tester-1 已在 result.md 附录 A 如实声明）

tester-1 用 TTS 放音经扬声器再由麦克风拾音，信号质量偏低：
**26 次 run 中 14 次 `words_total=0 / chars_total=0`（完全无识别）**。
故其统计口径（如「067 四轮三中」）可能掺入环境因素。
**不受影响的是**：061/068 的几何数据（不依赖识别质量）、
以及 070 的 15:09 那组干净对照（服务端返回与注入逐字比对）。

### 关联的其他 REPRO-073 结论

- **OVERLAY-061 未复现**：1044 帧零命中。tester-1 如实报告未复现并建议
  **多屏 / 不同 DPI 环境复测**，主控采纳。已向 Gavin 确认其显示器配置。
- **OVERLAY-068 覆盖面存疑**：实测交替闪烁主体为「长文本 vs Recording 240 宽」拉锯，
  而 coder-2 的 068-B 修的是「流式 → Processing 200」路径。已要求 coder-2
  核对覆盖面并给行号证据（不返工整体方案）。

---

#### 🔬 ASR-067 实测日志分析（主控 2026-08-30 23:0x，数据源 `target/release/debug.log`）

**数据源合法性**：`debug.log` mtime 2026-08-30 22:24，size 159,779 —— 今晚 tester-1
跑 REPRO-073 时由 `target/release/feiyin-ime.exe`（08-18 23:48 构建，**晚于 ASR-058
埋点提交 `710cec9` 23:27**，故探针在包内）产出。含 `[ASR-WORDS]` 48 行 / `[ASR-SUMMARY]` 4 行。

##### 结论一：🔴 主控原假设**被证伪**，不要再查这个方向

原假设：停顿触发服务端断句 → 新句 `begin_time` 按句内相对计时回落 →
合并词表不再全局升序 → `reveal_chars_by_timeline`（`src/main.rs:3244-3251`）
在句边界 `break` → 后续字永不揭示。

**实测：48 条 `[ASR-WORDS]` 逐条检查，`begin_time` 序列全部全局升序，无一例外。**
服务端时间戳是**整条音频流相对**，不是句内相对。
→ `:3248` 那句「words 按 begin_time 升序排列」的注释**是成立的**，
→ 「乱序导致提前 break」这条路**排除**。

##### 结论二：🟡 发现更强的新线索 —— **词时间轴只覆盖录音的一半**

以最后一次录音（`[ASR-SUMMARY]` 第 4 条）为例：

| 指标 | 值 |
| --- | --- |
| `final_ms`（整条录音时长） | **47,569 ms** |
| 词时间轴跨度（首词 begin → 末词 end） | **24,470 ms** |
| 覆盖率 | **51.4%** |
| `words_total` / `chars_total` | 87 词 / 122 字 |
| 词间静默 ≥700ms 的间隙 | **0 个** |

**即：录音进行到约 24.5 秒之后，服务端不再产出任何词时间轴，而录音又持续了 23 秒。**

词间无一处 ≥700ms 静默，说明**已产出的那 24.5 秒内说话是连续的**；
之后的 23 秒要么没说话，要么**说了但没有任何识别结果回来**。

**后者若成立，就是 ASR-067 的直接机制** —— 与 Gavin 描述
「录音时停顿 1 秒，然后继续说话，但屏幕上语音识别上屏显示就中断了」完全吻合：
音频还在发（coder-1 静态排查已证实客户端不停发，该结论仍然有效且不矛盾），
**但服务端停止回结果**，屏幕自然就不再更新。

##### 🔴 尚不能定案的原因（诚实说明，不要拿本节当判据改代码）

本次日志是 **tester-1 用 TTS 放音装置跑的**，**不是 Gavin 的受控复现**。
无法确定后 23 秒里是否真的有人在说话。
若那 23 秒本来就是静音，则「覆盖率 51.4%」是正常现象，不构成缺陷。

##### 下一步（需 Gavin 一次受控复现即可定案）

Gavin 用 `-debug` 跑一次**明确的**：说话 → 停顿约 1.5 秒 → **继续说明显能听清的一段话** → 松手。
然后看该次录音的：

| 检查项 | 判据 |
| --- | --- |
| `[ASR-WORDS]` 末词 `end_time` | 是否停在停顿处附近（≈ 停顿前的音频位置） |
| `[ASR-SUMMARY]` 的 `final_ms` | 是否远大于上面的末词 end_time |
| 两者差值 | ≈ 停顿之后那段话的时长 → **坐实服务端停止回结果** |

**若坐实**：根因在 ASR 会话侧（服务端断句后未恢复产出 / 客户端未重建 task），
文件域回到 `qwen_inference.rs`，归 coder-1；
**若证伪**（末词 end 接近 final_ms）：根因才在 overlay 揭示侧，归 coder-2。

**这个判据一次复现就能分流，不必两边同时开工。**

### 🔴 文件级冲突评估（派发前必读，主控已做）

**`src/main.rs` 被 8 项任务同时命中**（061/062/063/064/065/066/068/069）——
这是本批最大的冲突源。**必须整包给同一个 Worker（coder-2），禁止拆给两人并行。**

**唯一的跨 Worker 风险点是 ASR-070**：「尾部文字丢失」的修复点可能落在
`src/main.rs` 的 051-G 时间戳回放 / 松键 flush 路径，与 coder-2 的 overlay 批**同文件**。
处理办法：**先只让 coder-1 做只读根因取证**，确认修复点究竟在
`qwen_inference.rs`（服务端 flush 时序）还是 `src/main.rs`（reveal 揭示进度）：
- 落在 `qwen_inference.rs` → coder-1 独立做，与 coder-2 零冲突
- 落在 `src/main.rs` → 并入 coder-2 的批次串行做，**不允许两人同时开 main.rs**

`HOTKEY-060`（ui/）、`ITN-071`（itn.rs）、`FMT-072`（llm/mod.rs）三项文件域互不重叠，
且与 `src/main.rs` 零重叠，可由 coder-1 串行承接。

### 主控初判（供 Worker 取证时参考，**不是结论，不许当判据直接改**）

| ID | 主控假设 | 依据 |
| --- | --- | --- |
| ASR-067 | 疑似 `asr_online_max_sentence_silence`（隐藏字段，默认 800ms）判定句子结束后**会话未重建**，导致后续音频无人接收 | ASR-056 引入该字段；Gavin 描述的「停顿约 1 秒」与 800ms 阈值高度吻合 |
| ASR-062 视觉粗糙 | 疑似 **GDI 对图形不做抗锯齿**，且 `WS_EX_LAYERED` 分层窗口下 ClearType 次像素渲染被禁用 → 文字发虚、圆角/按钮边沿锯齿。054-D 的 `WM_SETFONT`（`src/main.rs:681-690`）确已生效，所以**根因不在字体句柄，而在渲染管线** | 需 coder-2 验证；若成立则属架构级问题，要立 DEC |
| OVERLAY-064 | 054-C 把边框色从 `0x060607`（近黑）改成 `0x3A3A3C`（中灰）后 Gavin 反而说「消失」——需查是否被 `SetWindowRgn` 圆角区域裁掉，或被 `SetLayeredWindowAttributes` 色键吃掉 | `OVERLAY_BORDER_GRAY` 定义在 `src/main.rs:997`，13 处用点已列 |
| OVERLAY-061 | 054-B-FIX 的 `pos: Option` 改造代码确在（`:1163-1172` / `:1651` 两处 `unwrap_or_else`），但仍闪 → 疑似**窗口创建时**就落在 (0,0)，第一次 `SetWindowPos` 之前已被绘制 | 需查 overlay HWND 的 `CreateWindowExW` 初始坐标 |

### 排期建议（主控提，等 Gavin 拍板）

1. **VERSION-059**（已派发，coder-1，约 10 分钟）
2. **ASR-067 + ASR-070 取证**（coder-1，只读，不改代码）—— 两条 P0，且决定 ASR-070 归谁做
3. **OVERLAY 批**（coder-2 独占 `src/main.rs`）：064 → 061 → 068 → 062 → 063 → 066 → 069 → 065
4. **HOTKEY-060 / ITN-071 / FMT-072**（coder-1，与 coder-2 并行，文件域零重叠）
5. 阶段三 TEST-SYNC → 阶段四 全量回归 → 阶段五 出包 v0.9.0

---

## 🔄 2026-08-18 晚 · 会话重启后现场核对（主控，控制台崩溃后重建上下文）

| 项 | 实况（以 `git status` / `git log` 为准，非文档记忆） |
| --- | --- |
| HEAD | `7b8423c`，本地 **ahead 7**（未 push，Gavin 未授权） |
| `src/` 工作区 | **clean** —— 昨日 handoffs 里「051-G WIP +270 未提交」**已过期**，该批已随 `663f1d5` 提交 |
| 未提交残留 | 仅 `tests/utils/state_detector.py` (+70) 与未跟踪 `tests/test_cases/test_overlay_position.py` —— tester-1 域的 E2E 位置断言，**待确认归属后一并提交** |
| 三 Worker | coder-1 / coder-2 / tester-1 **全部 ACK 就绪**（框架重启已清空 inbox/outbox，旧任务书被抹，[REPLACE-WORKER-TASKFILE-WIPED-001] 再现） |
| 流水线阶段 | 051-G-FIN + 054-B-FIX 的**阶段三已完成并提交** `ce690cf`；**阶段四（TEST-EXEC）尚未跑** |

### 本轮派发

| 时间 | 动作 |
| --- | --- |
| 08-18 晚 | **OVERLAY-054-C/D/E → coder-2**（任务书 9.2KB 重写，原 6.0KB 版被框架重启抹掉）。独占 `src/main.rs` |

### 排期决策：054-C/D/E 先做，阶段四合并一次跑

Gavin 已明令「三个问题一起做了再出包，避免无效重复出包」。
故**不为 051-G/054-B 单独跑一轮阶段四**，等 054-C/D/E 完成后
走一次 TEST-SYNC-054CDE（阶段三）+ 一次全量回归（阶段四），
最后 **BUILD-021 单包**带上：054-A 焦点恢复 / ASR-055 测试按钮 / WORDBOOK-053-C/D /
054-B-FIX 窗口位置 / 051-G-FIN 上屏节奏 / E2E-HARNESS-050 门禁 / 054-C/D/E 视觉三项。

### 🔴 出包门禁（主控自加，持续有效）

出包前 **E2E 门禁必须真跑通过**，不以「读代码觉得没问题」代替运行验证。

---

## 🛑 2026-08-17 当前状态 —— 下一步等 Gavin 端测 BUILD-018

> 三个 Worker 全部空闲待命。**下一个动作是等 Gavin 端测结果，不是派新任务。**

| 项 | 状态 |
| --- | --- |
| HEAD | `664b7bd`，工作区 **clean**，零悬空改动 |
| 本批 OVERLAY-043 四提交 | OVERLAY-043 `a588509` / 043-B `5940e73` / TEST-SYNC-043 `497131f` / TEST-EXEC-043 `b499cc3` **全部已验收提交** |
| 阶段四全量回归 | ✅ **1040/0/11**（上批 1030 +10 精确命中零残差）+ Vitest 54 + src-tauri 55；pytest 按书 SKIP（`Publish/` 当时为 BUILD-017 旧包） |
| 双消融实测 | ✅ A（`delta.abs()`→`delta`）变红 **4** 条（推演预期 3，主控裁决实测为准，见 `[ABLATION-MODEL-TOO-LIGHT-001]`）；B（门闩→false）变红 1 条与预期吻合；两次均已还原并 `git diff -w` 自证为空 |
| 阶段五出包 | ✅ BUILD-018 `664b7bd` —— 产物 08-17 13:57-14:00 在 `Publish/`，七项核验全 PASS（三 exe 12,115,456 / 10,026,496 / 24,859,648 B） |
| 未 push | 本地 ahead **6**，**Gavin 未授权 push，不得自动执行** |

### Gavin 端测四项重点（BUILD-018）

1. **流畅度目视** —— 文字进编辑框是否丝滑（`bErase=false` + `needs_repaint` 脏标记 + 25% lerp 插值）
2. **本地模型录一次** —— 确认波形动画未被本批改坏（静态论证四条已过，需实测兜底）
3. **波形隐藏 + 单按钮** —— `RecordingWithText` 态不画波形；右侧只有一个按钮（录音=停止方块 / 编辑=橙色 ⏎ 提交）
4. **松开热键立即切「处理优化中」** —— 晚到的 `StreamingText` 不应再把状态顶回录音态

### 端测通过后的下一棒（按序）

1. **TEST-045-REFACTOR** —— 抽 `decide_pipeline_entry` 纯函数补调用侧护栏缺口（见下方专节；🔴 红线：**不得合并** `:4334` 第二层 `text.trim().is_empty()` 分支）
2. **ASR-PERF-040-B/C** —— 连接健壮性与性能（040-A 埋点已落地，B/C 的唯一数据源是端测 `debug.log`，见下方门禁专节）

### 端测若失败，`debug.log` 优先看这三条

| 目标 | 探针 | 期望 |
| --- | --- | --- |
| ASR-042 采样率修复 | 搜 `Streaming resampler active` | **有**这行 = 重采样器接上了 |
| ASR-045 结果上屏 | 搜 `No audio samples recorded` | 流式录音后**不应**再出现 |
| OVERLAY-043 门闩 | 松开热键后的状态流转 | 不应再出现 `RecordingWithText` 把 `FallingToProcessing` 顶回去 |

### 🔴 两个不许动的东西（Gavin 明确指示，持续有效）

- `target/release/feiyin-ime-nor.exe`（08-09，11.9MB）—— Gavin 的**备份 exe，不得删除**。（其**进程**可在出包 Step1 清杀，名单已于 08-16 补入 `build-test-guide.md`）
- **Gavin 自启的端测进程** —— 杀之前必须先报主控、由 Gavin 授权（PID 4696 / PID 21948 两次先例）

---

## 🔴🔴 P0 最高优先 · OVERLAY-046 录音 overlay 从未被定位/定尺寸（2026-08-17 Gavin 端测 BUILD-018）

> **Gavin 原话**：「热键完全被改坏，无法正常使用输入法……按下设置好的热键，录音窗口不出现，完全无法录音」
>
> **性质：OVERLAY-043 引入的真回归，输入法 100% 不可用。已派发 coder-2。**

| 项 | 内容 |
| --- | --- |
| 编号 | **OVERLAY-046** |
| 文件域 | `src/main.rs`（仅此一个，coder-1/tester-1 本轮无任务，零冲突） |
| 负责人 | coder-2 |
| 状态 | ✅ **已交付，等主控验收** |

### 修复摘要

- **根因**：OVERLAY-043 重构中 Show 分支丢失无条件 `SetWindowPos` / `InvalidateRect`；非流式态恒等赋值 `current_size == target_size` 使插值门控 `size_interpolation_done` 永不置位，`:1221` 的 `SetWindowPos` 永不执行 → 窗口不定位/不定尺寸。
- **修法**：在 `src/main.rs:997` Show 分支 `ShowWindow` 之前恢复无条件 `SetWindowPos(request.pos, computed_size, SWP_NOACTIVATE | SWP_NOZORDER)`；`ShowWindow` 之后补 `InvalidateRect(hwnd, None, false)`（`bErase=false` 红线保持）。
- **覆盖变体**：`Recording` / `FallingToProcessing` / `Processing` / `Error` / `FocusLost` 全部非流式态；流式态 `RecordingWithText` / `StreamingEditing` 不受负影响，插值动画继续。
- **验证**：`cargo fmt --all -- --check` clean / `cargo check --all-targets` 0 error / `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / `git diff -w -- src/main.rs` 仅 `:997-1005` 新增 14 行。
- **阶段三**：不跑 `cargo test` / `cargo build`，测试由 tester-1 负责。

### 根因（主控 git 对照取证，四步）

| # | 证据 |
| --- | --- |
| 1 | `git show 56bfa37:src/main.rs`（043 之前）Show 分支含三件事：**无条件 `SetWindowPos`** + `ShowWindow(SW_SHOWNA)` + `InvalidateRect(hwnd, None, true)` |
| 2 | 现 HEAD `664b7bd` 的 Show 分支（`:997-1005`）只剩 `SetLayeredWindowAttributes` + `ShowWindow` + 条件 `SetTimer` —— **`SetWindowPos` 与 `InvalidateRect` 双双消失** |
| 3 | 全文件唯一定位 overlay 的 `SetWindowPos` 在 `:1221`，被 `size_interpolation_done` 门控；该标志仅在 `:1198` `current_size != target_size` 成立时于 `:1212` 置位 |
| 4 | 但 Show 分支 `:991-993` 对**非流式态**执行 `target_size = current_size = computed_size` → 两者恒等 → `:1198` 恒 false → **`SetWindowPos` 永不执行** |

`:476` `remove_noactivate` / `:494` `restore_noactivate` 的两处 SetWindowPos 都带
`SWP_NOMOVE | SWP_NOSIZE`，不构成兜底。

### 🔴 影响范围比 Gavin 报的更广

`Recording` / `FallingToProcessing` / `Processing` / `Error` **全是非流式态**，全走 `:991-993`
→ **在线模型与本地模型的录音窗口一律受影响**。

🔴 **主控自陈作废一条结论**：本文件下方「本地模型回归核查四条全过 → OVERLAY-043 对本地模型
录音窗口零影响」**是错的**。那四条只机器比对了 GDI 绘制原语序列，**没有查 `SetWindowPos`
的调用门控**，因此漏掉了本缺陷。教训已记 `troubleshooting.md`。

### 红线遵守

① `InvalidateRect` 的 `bErase` 保持 `false` ✅  ② 未动 `:1198` 插值分支与 `interpolate_step` ✅  ③ 未改 `STREAMING_STOPPED` 门闩语义 ✅  ④ 版本号 0.8.0 未动 ✅

---

## ℹ️ 非代码问题（已核实，无需开单）

| 现象 | 结论 |
| --- | --- |
| `LLM optimization error: HTTP 402 Payment Required`（api.deepseek.com） | **DeepSeek 账号余额/账单问题**，非代码 bug。后果：LLM 优化失败 → 走 raw text fallback。需 Gavin 侧充值 |
| `转录失败：task-finished 但无识别结果` ×20 / ASR 结果 254 条中 246 条 `display=''` | **端测手法所致，非 bug**。Gavin 在反复短按热键测「窗口出不出来」，多数按压 <1.5s 且未说话 → VAD 正确判定无语音（`Audio channel closed during VAD gate` / `2s deadline without detection`）→ 服务端正确返回空。同一会话中真说话的两次（06:16:35 / 06:16:36）识别出「你好。」，06:17:26 识别出「Hello.」→ **ASR 链路本身是通的** |

## 🔴 P0 进行中 · v0.8.0 端测两大问题（Gavin 2026-08-16 提交）

> 出包后 Gavin 端测发现两个问题，主控已独立取证定位根因。**执行顺序已与 Gavin 商定：先修识别，再修窗口。**
> 理由：识别输出乱码时，无法目视判断窗口显示是否正确。

| 编号 | 内容 | 文件域 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| **ASR-042** | 在线流式 ASR 采样率修复（48000→16000 带状态流式重采样器） | `src/audio/mod.rs` | coder-1 | ✅ **已验收已提交 `34906a1`**（StreamingResampler + record_streaming 接线 + 6 测试 + 改坏会红自证） |
| TEST-SYNC-042 | 阶段三测试同步（补 4 条流式重采样缺口用例） | `src/audio/mod.rs` `mod tests` | tester-1 | ✅ **已提交 `427cd50`** |
| **ASR-045** | 流式识别结果被管线丢弃（文字从未上屏，P0） | `src/main.rs` | coder-1 | ✅ **已验收已提交 `54cde62`**（`should_cancel_on_empty` 纯函数 + 判空臂改调 + 3 条护栏测试） |
| TEST-SYNC-045 | 阶段三测试同步（ASR-045 缺口 3 条，`src/main.rs` +46 仅测试模块） | `src/main.rs` `mod streaming_empty_samples_tests` | tester-1 | ✅ **已验收**（主控逐行 Read diff + 独立复算 `cargo fmt --check` clean / `cargo check --all-targets` 0 error）。真值表第 4 格（如实标弱护栏）+ 空串/纯空白两层分层契约 ×2（核心）+ 调用侧不可测诚实判定 |
| TEST-EXEC-042/045 | 阶段四全量回归 + 消融 A/B 自证 | — | tester-1 | ✅ **已验收已提交 `2a173f0`**。1030/0/11（941→957 恰 +16，零残差）+ Vitest 54 + src-tauri 55；pytest 按书 SKIP（Publish/ 为 BUILD-016 旧包）；③ 类真回归 = 0 |
| BUILD-017 | 阶段五出包（ASR-042/045 进 exe） | — | tester-1 | ✅ **已验收**（主控独立复算全部七项）。产物 08-16 23:25，`feiyin-ime.exe` 12,112,384B（+5,632）/ ui 10,026,496B 不变 / crash 24,859,648B 不变；两副本 sha256 全等；toml 三副本全等；ProductVersion 0.8.0.0/0.8.0/0.8.0.0。⏭ **待 Gavin 端测** |
| **OVERLAY-043** | 录音窗口**五项**显示错乱 + 流畅度 | `src/main.rs` | coder-2 | ✅ **已验收已提交 `a588509`**（打回一轮，见下方打回记录）。⏭ 流畅度**待 Gavin 端测目视拍板** |
| **OVERLAY-043-B** | 抽 `interpolate_step` / `should_ignore_streaming_text` 补护栏 | `src/main.rs` | coder-2 | ✅ **已验收已提交 `5940e73`**（纯重构零行为变更，无 `#[test]`） |
| **TEST-SYNC-043** | 阶段三测试同步（两纯函数护栏 + 不可测项如实说明） | `src/main.rs` `mod tests` | tester-1 | ✅ **2026-08-17 已完成待验收**（`mod overlay_043_interpolate_tests` +138 行仅测试区：10 条用例——五契约 ±50000 穷举 + 缺陷 A 数值回归护栏 + 双向收敛预算 + 门闩四格真值表；cargo fmt clean / cargo check --all-targets 0 error；Python 复算契约成立、消融推演护栏有效。消融实测顺延阶段四 TEST-EXEC-043） |

### 🔴 OVERLAY-043 验收打回记录（主控独立复算查出，非采信报告）

| 缺陷 | 位置 | 问题 | 数值证据 |
| --- | --- | --- | --- |
| **A** | `main.rs:1195` 插值步长 | `(dx as f32 * 0.25).max(1.0)` 在 `dx` 为负时**负数被 `max(1.0)` 吃掉比例**，恒为 `-1px` | 800px→240px 需 **559 帧 ≈ 8.9 秒**爬行；修正后放大/缩小对称均 **22 帧 ≈ 0.35 秒** |
| **B** | `main.rs:1114` 脏标记 | 纯 `Recording` 态被纳入脏标记，但**全文件无一处在音频电平变化时置位** → 波形动画冻结 | 违反红线「`Recording` 观感零变化」；对照 `:1159` `FallingToProcessing` 有 `!all_settled` 兜底，`Recording` 分支缺同类兜底 |

**教训**：缺陷 A 只能靠手工数值复算发现 —— 算式内联在消息循环内，测试够不着。
故**当场派 OVERLAY-043-B 抽纯函数补护栏**，不排期延后
（与 `TEST-045-REFACTOR` 同类，但那是没塌过的预防，本处是刚塌过一次）。

### ✅ 本地模型回归核查（Gavin 2026-08-17 提出，主控独立取证，四条全过）

> **Gavin 原话**：「本地模型的录音还是会使用原先的录音窗口，所以这次新集成 asr 实时上屏
> 不能影响本地模型录音的 overlay 窗口效果」

| # | 核查项 | 结论与证据 |
| --- | --- | --- |
| 1 | 绘制是否等价 | ✅ **机器比对**：拆分前单体与拆分后 `chrome + indicator_and_waveform + stop_button` 串接，**18 个 GDI 原语序列完全一致**（`diff` 为空） |
| 2 | 是否被尺寸插值波及 | ✅ 不会。`is_streaming_text` 仅 `matches!(RecordingWithText)`；非流式态 `:991-994` 令 `target_size == current_size` → `:1191` 插值条件不成立 |
| 3 | 波形是否会冻 | ✅ 不会。返工后 `Recording` 独立为**每帧重绘**分支，脏标记只用于 `RecordingWithText`/`StreamingEditing` |
| 4 | 门闩是否误伤 | ✅ 不会。`StreamingText` 仅在 `is_streaming_asr`（`AsrModel::QwenAudioOnline`）下产生（`:3512-3543`）；本地模型走 `record()` + `run_pipeline_core` 老路**永不进入该分支**，注释 `:3509` 亦自陈「零行为变更」 |

**结论**：本地模型 overlay 只经历 `Recording → FallingToProcessing → Processing`，
全是本批未改语义的状态。**OVERLAY-043 对本地模型录音窗口零影响。**
🔴 **但这是静态代码论证，Gavin 端测时请顺手用本地模型录一次确认。**

### 🟡 待排期 · TEST-045-REFACTOR 抽 `decide_pipeline_entry` 纯函数（tester-1 建议，主控采纳但延后）

**问题**：ASR-045 全批 6 条测试（coder-1 3 + tester-1 3）**全是纯函数级，保护不了调用侧**。
若有人把 `main.rs:4318` 的 guard 改回 `Ok(s) if s.is_empty()`，P0（流式文字从未上屏）立刻复发，
**6 条测试依然全绿**。这是本批已知且已如实记录的护栏缺口，不是遗漏。

**建议方案**（tester-1 提出，主控核后认可）：把 `run_pipeline_core` 入口三臂判据抽成
`decide_pipeline_entry(&samples_result, &initial_text) -> EntryDecision`
（`CancelNoInput` / `Err` 透传 / `Proceed`），让「空+文本 / 空+无文本 / 非空+无文本 / Err」四种入口组合可测。

**主控裁决：采纳，但排到 OVERLAY-043 之后。** 理由：① 这是可测性改进不是 bug，不该插队 P0；
② OVERLAY-043 马上要动 `main.rs`，两个改动叠一起会让回归归因变难。
🔴 **实施时的红线**：不得合并 `:4334` 第二层的 `text.trim().is_empty()` 分支 ——
分层是刻意设计，合并即复现「空文本静默消失」的 P0 类回归。

### ASR-045 根因（一句话）

调用侧 `run_pipeline_core(Ok(Vec::new()), …, Some(streaming_text))`（`main.rs:3409`）传**空 samples +
流式文本**；接收侧判空臂 `Ok(s) if s.is_empty()`（原 `:4307`）**无条件取消**，`initial_text` 从未被读取
→ 流式文本在函数入口即被丢弃，ITN→LLM→注入后半段从未执行（日志 `:193` 已产出 5 confirmed sentences，
`:194` 即被 WARN 吞掉）。

**取证与修法** → outbox/coder-1/result.md（含三问全链 27 步、Q1 samples 消费点排查、Q2 cancel_signal
时序、护栏测试）。

### ASR-042 根因（一句话）

麦克风 48000Hz 采集，发给服务端的参数写死 `sample_rate: 16000`，中间零重采样 →
服务端按慢 3 倍解码 → 识别输出完全不相干的句子（「你好，花呗还不上」等客服话术）。

**成因**：旧 `record()` 在录音结束那一刻一次性重采样（`audio/mod.rs:757`）；改成边录边发后
「录音结束」这个时刻不存在了，该步骤静默失效。**不是写错，是改架构漏接线。**

**完整取证与修法** → `collab/troubleshooting.md` `[ASR-SAMPLERATE-STREAM-001]`
（含 8 次录音「6 有重采样全对 / 2 无重采样全错」对照表与日志自证行）。

**关键约束**：不能逐块直接调 `resample_anti_alias`（整段 windowed-sinc，TAPS=32），
逐块调用每 10ms 截断一次卷积核，会让 FIRSTCHAR-FIX-005 修过的送气清声母失真复发。
必须做带状态流式重采样器 + 全局输出计数。护栏：批处理 vs 流式逐点差 ≤ 1e-5。

**次生现象**：VAD 2s 漏检（`debug.log:36`）同根因，预期自动恢复，**须实测确认，禁改 `vad.rs`**。

### OVERLAY-043 六项（主控已逐条核对代码，非采信 coder-2 汇报）

> 🔴 **2026-08-17 Gavin 端测截图复核，本节已从四项扩为六项。**
>
> **首先澄清一个误解**：Gavin 反馈「上次发现的问题根本没改过来」——
> **本任务自始至终从未派发过**（本节状态一直是「🔜 等 BUILD-017 + Gavin 目视确认识别正常后再派发」），
> 所以不是「改了没生效」，是**还没动过一行代码**。责任在主控排期，不在 coder-2。
>
> **端测截图的意外收获（重要正面结论）**：截图中 overlay 显示「今天的天气如何 可以说」——
> **语义完全正常**，不再是修复前「你好，花呗还不上」那类 48kHz 按 16kHz 解出来的客服话术。
> **ASR-042 采样率修复已由端测实证生效**，且流式文字确实上了 overlay。

| # | 现象 | 核对结论（主控 Read 代码取证） |
| --- | --- | --- |
| 1 | 波形动效未隐藏 | 属实。`main.rs:1786` 首行 `draw_recording_overlay` 整个重画，注释自陈 reuse「background/border/**waveform**/stop button」，波形被原样带进 |
| 2 | 文字非白色 | ⚠️ **本次端测截图看文字是白的**，Gavin 未再提。降级为待观察，不列入本次修复范围 |
| 3 | 右侧两个按钮（提交+停止） | 属实。`:1786` 返回 stop 按钮，`:1834-1871` 又叠画一个橙色圆角 ⏎ submit 按钮 → **两个并存**。Gavin 明确要求：**提交按钮应复用停止按钮的位置**，而非新增 |
| 4 | 窗口随文字变宽时卡顿、突闪 | 属实且**比原描述严重**：`:908-925` 判 `should_resize`，`:940-949` 为 false 时走 else → **回退到 `request.size`（默认 240px）而非保持上次宽度** → `:956` SetWindowPos 无条件执行 → 窗口在算出宽度与 240px 之间**来回跳** = 突闪 |
| 5 | 🆕 文字进编辑框卡顿，不丝滑 | **主控定位**：`:1078` `InvalidateRect(hwnd, None, **true**)` —— 第三参 `bErase=TRUE` **每帧擦背景再重画**，是 GDI 闪烁/卡顿的经典根因。三个态（Recording/RecordingWithText/StreamingEditing）全走这一行 |
| 6 | 🆕 录音结束未进编辑态时，应立即切「处理优化中」窗口 | **主控定位到竞态**：`:2737-2752` 松开热键的 `HotkeyEvent::Stop` **确实**发了 `FallingToProcessing`，但 `:2788-2796` 晚到的 `PipelineEvent::StreamingText` 又推 `RecordingWithText` **把它覆盖回录音态**。即状态切了、又被流式尾包顶回去 |

🔴 **主控自陈**：第 4 条的 100ms 节流是**上一轮主控自己要求 coder-2 加的**（防每帧改宽度抖动），
现成为文字截断的直接原因。**不能简单删除**（删了会抖，属「修一个带出一个」）。
修法：改为**延迟合并** —— 100ms 内多次变化攒起来，到点按最新文字算一次宽度。

### ~~待查~~ ✅ 已定性并修复（2026-08-16）

~~`debug.log:194`/`:429` 每次在线流式录完都报 `WARN No audio samples recorded`。~~

**结论：不是日志噪音，就是 ASR-045 本身。** 该 WARN 正是判空臂 `Ok(s) if s.is_empty()`
无条件取消时打的那一行 —— 流式文本在此被丢弃，整条 ITN→LLM→注入后半段从未执行。
ASR-045（`54cde62`）修复后该 WARN 在流式路径不再触发。**本条核销，无残留副作用待查。**

---


## 🔴 P0 门禁 · ASR-PERF-040 新 ASR 连接性能与可靠性（2026-08-15 Gavin 指令）

> **Gavin 原话**：「这次继承新 asr 模型一定要汲取之前的教训，要优化好连接池、
> 要优化好连接的速度和性能。」
>
> **定位：这是 ASR-038 全批的验收门禁，不是可选优化项。** 038-B/C 完成后若本节未落实，不予出包。

### 🔴 第一件事：028 的教训不能照搬，因为 ASR 根本没有连接池

| 项 | LLM（028 现场） | ASR（本批） |
| --- | --- | --- |
| 传输 | `reqwest` HTTP，**有连接池** | `tungstenite` WebSocket，**无池，每次全新建连** |
| 028 根因 | 池中空闲连接被服务端 keep-alive(~60s) 杀掉，取出即 0ms 失败 | **不适用** —— 没有池就没有僵尸连接 |
| 修复 | `POOL_IDLE_TIMEOUT=30s` + `is_request()` 重试 + `fmt_error_chain` | **照搬无效** |

**🔴 但真正的陷阱在这里**：为了提速而引入「**预热 / 复用长连接**」，
会**精确复现 028 的 bug 类** —— 预热好的 WS 空闲期间被服务端 idle timeout 静默杀掉，
下次录音取用即失败，且表现为「0ms 失败」，与 028 一模一样。

**因此复用设计必须自带三件套，缺一不可**：
① 本端 idle 上限**必须短于**服务端 idle timeout（服务端值未知 → 须实测，不可假设）；
② 取用前**健康检查**（WS Ping/Pong 或轻量探针）；
③ 检查失败**立即回退新建连**，绝不把失败抛给用户。

> **这正是 028 的真正教训**：不是「把超时调小」，而是
> **「任何跨请求复用的连接，都必须假设它在空闲期间已经死了」。**

### 取证：当前连接路径的实际开销（`qwen_inference.rs:445-465`）

每次录音**从零走完整条链**，零预热、零复用、零重试：

```
DNS 解析 (to_socket_addrs，阻塞系统调用，无缓存)
  → TCP connect (CONNECT_TIMEOUT = 5s)
  → TLS 握手 (client_tls_with_config)
  → WS 升级 (HTTP 101)
  → 才开始发第一个音频字节
```

`grep -i "prewarm|reuse|retry|keepalive|pool" src/transcription/qwen_inference.rs` → **零命中**。

### 已定位的四个问题

| # | 问题 | 位置 | 性质 |
| --- | --- | --- | --- |
| **40-1** | **`socket_addrs.first()` 只试第一个地址** —— DNS 返回 IPv6 在前但本机无 IPv6 连通性时，**直接失败**，不会回退第二个地址 | `qwen_inference.rs:461` | 🔴 **健壮性缺陷**。标准做法是遍历所有 addr（`TcpStream::connect` 本身就是这么做的） |
| **40-2** | **零重试** —— 建连瞬时抖动直接变「转录失败」 | 同上 | 🔴 与 028 修复精神相悖（028 明确补了重试判据） |
| **40-3** | **零 `[Latency]` 埋点** —— DNS/TCP/TLS/WS 各段耗时**完全不可观测** | `qwen_inference.rs` 全文 | 🔴 **Gavin 要求「优化速度」，但现在连测都测不了**。全库已有 12 处 `[Latency]` 约定可循 |
| **40-4** | `CONNECT_TIMEOUT=5s` 被同时用作 **connect / read / write** 三处超时 | `:32`、`:456-459` | 🟡 语义混用。建连 5s 合理，但流式收包期间的 read timeout 应独立定义 |

### 🔴 真流式在这里是**性能杠杆**，不只是交互需求

| 方案 | 建连成本落在哪 | 用户感知 |
| --- | --- | --- |
| **真流式**（边录边发） | 建连与**说话并行** → DNS+TCP+TLS 被说话时间吸收 | **零感知** |
| **伪流式**（录完再发） | 建连在**录音结束之后** → 全部落在关键路径 | **干等一个完整 RTT 链** |

**这条独立于 037 交互论证，直接服务于 Gavin 的性能指令。**
伪流式方案下，「优化连接速度」这个目标先天就少了最大的一块杠杆。

### 建议实施顺序（待派发）

| 编号 | 内容 | 前置 |
| --- | --- | --- |
| 040-A | **先补 `[Latency]` 埋点**（DNS/TCP/TLS/WS 升级/首帧/首结果分段计时） | 无 —— **必须最先做，否则后续优化全是盲调** |
| 040-B | 修 40-1 地址遍历 + 40-2 建连重试（判据参照 028 的 `is_request()` 精神）+ 40-4 超时语义拆分 | 040-A |
| 040-C | 实测服务端 idle timeout（**不可假设**），据此决定是否上预热/复用及其 idle 上限 | 040-A/B + 端测数据 |

**🔴 040-C 是 028 同款雷区，未拿到服务端 idle timeout 实测值之前，禁止上长连接复用。**

### 文件域

`src/transcription/qwen_inference.rs`（与 ASR-038-B 同域，**必须串行，归同一个 Worker**）。

---


## 📋 待派发 · ITN-IDIOM-COVER-032 量级单位字成语零覆盖（2026-08-09 主控取证发现）

> **来路**：`examples/probe_031.rs`（031 临时探测程序）删除前，主控核查其覆盖词的测试情况时发现。
> **性质**：与 031「万一→`0.1万`」**同一类** —— 量级单位字（十/百/千/万/亿）开头的固定表达。

### 取证：六个成语在 `src/itn.rs` 里**零出现**

| 词 | `src/itn.rs` 出现次数 |
| --- | --- |
| 十全十美 | **0** |
| 十有八九 | **0** |
| 百般 / 百般刁难 | **0** |
| 千方百计 | **0** |
| 百里挑一 | **0** |
| 成千上万 | **0** |

对照：`十分` 出现 8 次（有覆盖）、`千千万万` 出现 1 次。

### 为什么这不是「补几个词」那么简单

031 只给**万/亿**两个分支加了守卫（`!has_digit && section == 0 && result == 0 → return None`）。
`十/百/千` 三个分支**至今无守卫**，按 todo 既有记录：百/千分支不设锚点、`result==0` 被拒，
**歪打正着没出事**；而 `:682` 十分支默认 1（`十分`→`10分`）挂了很久，一直是「只报不改」。

**所以本单要先回答一个问题**：这六个词现在的实际输出是什么？
—— 是已经正确（靠 `result==0` 侥幸拦下），还是已经错了但没人测过。
**取证之前不要动代码**，也不要走保护词表（DEC-038：词表不承载规则性语法族）。

### 建议分解（待派发）

| 编号 | 内容 | 负责人 |
| --- | --- | --- |
| 032-A | 六词 + `十分`族实测现状，产出「现状表」（只测不改） | tester-1 |
| 032-B | 依 032-A 结果判定：真缺陷则走机制层给十/百/千分支补对称守卫；已正确则只补断言钉住 | coder-1 |
| 032-C | `:682` 十分支默认 1 一并结论化，不再挂「只报不改」 | coder-1 |

**文件域**：`src/itn.rs`（与任何 ITN 任务互斥，不可并行派发）。

---

### 🧪 给 Gavin 的端测观察点（本批改了什么、该看什么）

| 编号 | 改了什么 | 端测怎么看 |
| --- | --- | --- |
| 030 标点全源治理 | 关掉自动标点开关后，**6 个产出源一视同仁**（原来只控 1 个） | 关开关说一段话，看是否**任何标点都不出现**（含引号、括号、`、；`列表分隔符），且标点位置留空格 |
| 030 短句 ≤5 | 开着开关时，字/词数 ≤5 的短句不加末尾标点 | 说「好的」「今天天气不错」，看末尾有无句号；≥6 字应正常加 |
| 030-A-2 成对符号 | `（笑）` 不再被剥成 `（笑` | 说带括号/引号的短句，看有无孤儿左半 |
| DEC-047 Qwen3 标点 | 在线 ASR 由「假设必有标点」改为**实测**，短句无标点时不再跳过标点引擎 | 用在线 ASR 说短句，看该补的标点有没有补上 |
| 031 万一守卫 | `万一` 不再变 `0.1万`；`亿万`/`百万`/`千万` 同族一并覆盖 | 说「万一下雨怎么办」「百万富翁」，看是否保持汉字 |
| 027 零回归 | 大额数字跨亿到个位 | 说「一千零四十六万八千七百四十一」应得 `10468741` |

---

### 📜 2026-08-09 20:5x TEST-EXEC-030 验收记录（已完成，保留供追溯）

**结果：A0–A7 零 FAIL，四族零回归全绿。** `itn::` 225 ｜ `punctuation::` 43 ｜ `transcription::` 105+4ign
｜ `llm::` 140 ｜ 主 crate 958+8ign ｜ src-tauri 53 ｜ `--list` 自洽 966==966。零生产代码改动。

**主控独立复算（未采信汇总表格）**：用源码 `#[test]` 计数逐项验证，六个数字全部吻合 ——
`itn.rs`=225／`punctuation/mod.rs`=43／`llm/mod.rs`=140／transcription 三文件 54+28+27=109／
src-tauri 22+5+26=53（`#[path]` 引入 `wordbook/mod.rs`+`wordbook/db.rs`）／总数 900+30+36=966。

**`itn::` 用例数争议已裁定为 225**：旧记录 212 失效，coder-1 报的 219→221 为 TEST-SYNC-030-B 之前的中间态。

⚠️ **[DOC-STATE-DRIFT-001] 复现**：tester-1 只更 CHANGELOG + logs 两份，handoffs 与 progress 零条目，主控代记（已标注）。

📌 **只报不改遗留**：`examples/probe_031.rs`（08-08 031 探测残留，未清理未入库）。因 cargo 会自动发现
`examples/`，它会被 `cargo check --all-targets` / `cargo test` 连带编译。**处置待 Gavin 拍板**（删除／入库／保留），
主控不擅自删（依据「不可逆操作必须单独成轮」）。

---

### 📜 2026-08-09 19:38 派发记录（已完成，保留供追溯）

> 🔴 **交接记录有一处不符，已更正**：handoffs 写「任务书原样可用」，但 `inbox/tester-1/task.md`
> 在 19:33 新 session 启动时**已被清空为 0 字节**。主控已按原设计**重写**任务书（7030B），
> 含 A0–A7 步骤、红条三分类纪律（①测试写错可改／②预期内行为变更须给来源依据／③真回归一律上报不动）、
> 四族零回归专项（017 重量链／026 货币链／027 大额数字+DEC-042／031 万一守卫）、
> 红线（禁生产代码改动、禁出包、禁 git 破坏性命令、禁 PowerShell 改 UTF-8、禁虚报须贴原始摘要行）。
> 另要求 tester-1 给出 `itn::` **实跑用例数**，裁定 coder-1 报的 219→221 与 handoffs 记录不符一事。

**三 Worker 启动状态（19:3x 实测）**：coder-1 `%2` / coder-2 `%1` / tester-1 `%3` 全部存活并已 ACK 就绪，
均为 `DeepSeek V4 Flash Free · OpenCode Zen`（与本文件旧记录的 glm-5.2 / kimi-k2.7 已不同）。
coder-1 为 **OpenCode 类型非 Codex**，故未发 `/permissions Full Access`。
本轮 tester-1 独占 `src/`，coder-1/coder-2 已通知空闲待命勿动 `src/` → 零文件级重叠。

**基线数字**（供对照）：`itn::` 221 ｜ `llm::` 134+9 ｜ `punctuation::` 38+10 ｜
`transcription::` 105 ｜ src-tauri 53。Vitest/pytest 本批 SKIP（零前端零 UI 改动）。

**回归风险最高的三处**：① `strip_punctuation` 标点由「删除」改为「换空格」（行为变更，
既有断言大概率绿转红，属②类不是③类）；② `main.rs` L2 收口块删来源判据 + 抽纯函数；
③ `itn.rs` 万/亿分支新增守卫。

**之后**：TEST-EXEC 通过 → BUILD-015 出包（**主控须明确下达「现在可以出包」**）→ Gavin 端测。

### 🔴 待 Gavin 拍板：阶段三规则开例外

**同一根因本批发作三次**，第三次已从「diff 变脏」升级为「主干编译失败」：

| # | 任务 | 后果 |
| --- | --- | --- |
| ① | TEST-SYNC-030 | 代码非 rustfmt-clean，被后续 coder 的 `cargo fmt` 连带归一，污染两个 commit 的 diff |
| ② | 同上 | 同上 |
| ③ | TEST-SYNC-030-B | `src/llm/mod.rs:4776` 多一个 `}`，**整个 crate 编译失败**，主控提交前 `cargo check` 才发现并修掉 |

**根因**：三阶段规则明令阶段三「禁止执行任何命令」，`cargo check` 也在禁止之列 →
tester-1 连括号配没配对都无从知道，**交付前不可能自查**。

**主控建议**：阶段三开白名单例外，只允许 `cargo fmt` + `cargo check`。
理由：规则要防的是「测到半成品、拿到假结果」，而 `fmt` 只格式化、`check` 只做类型检查，
**都不执行测试、不产出二进制**，与该风险无关；禁止它们反而直接导致交付物编译不过。
⏸ **Gavin 未拍板前规则不变**，继续由主控在提交前兜底。

---


## ⏸ 待 Gavin 拍板 · ITN-FIX-BIGNUM-027 遗留三条（2026-08-04 挂账，实施已全部结案）

> 027 全批 A–E 已提交结案（`136f70f` / `967cd8d` / `e79ed05` / `499d56e`），
> 完整实施记录与根因取证已迁入 `todo-archive.md`。本节只留**仍未拍板的三条断言**。
> 🔴 其中「十万个为什么」一条**已由 DEC-044 解决**（专名加入保护白名单），保留行文备查即可，无需再拍。

### ⏸ 待 Gavin 拍板三条（tester-1 已按现状锁定断言，标注「待拍板」）

| 用例 | 现状 | 问题 |
| --- | --- | --- |
| 一万亿 | `10000亿` | DEC-042 规则直接推论，但中文习惯说「1万亿」。三选项：`10000亿`（守规则）/ `1万亿`（万亿当一个量级）/ `1000000000000`（特例展开） |
| **十万个为什么** | **`10万个为什么`** | 🔴 **专名被转，书名被改写**。属 DEC-038 词表覆盖问题（与 `五毛钱`/`三毛钱` 同源），**非 027 引入**，不阻塞出包，是否开单待定 |
| 两万五百 | `20500` | 最小单位百 → 普通数字，主控判断符合规则，列此备查 |

---

## 📋 待派发 · ITN 成语漏保护：`三五成群` → `35成群`（2026-08-04 Gavin 端测顺带发现）

> **性质：漏保护 = 输出损坏**（比误保护严重）。本次侥幸被 LLM 救回并进了 suggestions，若 LLM 超时走本地兜底，用户直接看到 `35成群`。

**取证**：`target/release/debug.log:2797`
```
ASR:  再比如说有同学三五成群打小赌
ITN:  再比如说有同学35成群打小赌
LLM:  三五成群（改回来了，suggestions:["三五成群"]）
```

**派发时机**：⏸ **等 027 完成后再派** —— 与 027 同属 `src/itn.rs` / `itn-rules.toml` 文件域，并行会冲突（文件级重叠检查）。

**修法待定**：`三五成群` 是成语（不可推导的固定表达），按 DEC-038 属于「保护词表**应该**承载」的那一类，可走词表。但需先确认 `[protect.idioms]` 为何未覆盖 —— 若成语表本就该有它，那是词表缺漏；若是逐位串路径绕过了成语保护，则是规则层缺陷，须走规则层修。**开单前主控先做这项取证。**

---


## 文档更新规则

1. **只保留新产生、进行中、验证失败、待排期或其他待决的任务**
2. **已完成的功能任务立即归档到 progress.md**，不在 todo 保留历史
3. **测试同步/构建/出包任务归入 CHANGELOG**，不在此文档详列
4. **新任务产生时立即写入**，不批量补
5. **不列端测跟踪项**（Gavin 自行使用中测试，有问题会重新开单）

---


## 📋 待排期 · LLM-USAGE-OBSERVABILITY-001 解析 usage 缓存字段（Gavin 2026-08-04：先建待办，下次再做）

> 性质：**可观测性补齐**，改动极小，不碰提示词本身。
> 前置：无，可随时启动。

### 背景：97% 的 prompt token 花在固定前缀上

主控 2026-08-04 从 `target/release/debug.log` 实测：

| 场景 | system_prompt | prompt_tokens |
| --- | --- | --- |
| 多行（Notepad/文档类，`multiline_safe=true`） | **21,868 字符** | **~5,300** |
| 单行（IDE/终端/微信，`multiline_safe=false`） | **13,174 字符** | **~3,269** |

差额 8,694 字符是多行分支独有的 F3 格式契约 + 四语枚举标记清单（023 恢复的 132 个标记短语）。

构成：主体（L0-L3 分层 + F3 + 四语清单 + i18n 基座）~21,400 ｜ 场景 F4 块 226 ｜ `extra_instruction` ~210 ｜ 词库 12 条。

**用户实际语音内容仅 62-232 字符（~50-150 tokens），completion 仅 17-64 tokens** —— 即每次请求 **97% 以上的 prompt token 是完全相同的固定前缀**。

### 问题：我们看不到缓存有没有命中

DeepSeek 支持上下文缓存（相同前缀命中后计费更低、延迟更低），而我们的系统提示词**每次完全相同**，是理想的缓存对象。

但 `ChatResponse`（`src/llm/mod.rs`）**没有解析 usage 里的缓存相关字段** —— 日志只有 `prompt_tokens` / `completion_tokens` / `reasoning_tokens`，**无法判断缓存是否命中、命中率多少**。

**在补上这个字段之前，任何关于提示词成本的讨论都只是估算。**

### 与既有教训同源

`[LLM-COT-LEAK-001]` 教训 6 原文：

> HTTP 响应结构体只解析自己要用的字段，会丢掉排障关键信息。`finish_reason` / `usage` 是零成本可观测性，应该默认解析。本次因缺这两个字段，一个本可一眼看穿的截断问题耗掉了整轮研究任务才坐实。

**同一个毛病第二次出现**。上次缺 `finish_reason`/`usage` 害得排查绕了一整轮，这次缺缓存字段导致成本无法量化。

### 实施范围（预估：一个 coder 小任务）

1. `ChatResponse` 的 usage 结构体补解析缓存相关字段（**实施前先查证 DeepSeek 官方响应 schema 的确切字段名**，不要照猜）
2. `LLM response meta` 日志行追加缓存字段输出
3. 跨 endpoint 兼容：本项目允许用户填任意 OpenAI 兼容 endpoint，其他厂商字段名可能不同或缺失，**用 `Option` 容错，缺失不得影响主流程**
4. 不碰提示词本身，不改任何现有行为

### 后续可能的衍生

拿到命中率数据后才谈得上判断：是否需要为缓存友好而调整提示词结构（如把最易变的 F4 场景块与词库注入移到末尾，保证前缀稳定）。**数据出来之前不做任何提示词改动**（DEC-041 与 `[F3-UNORDERED-LIST-001]` 的教训：提示词改动必须由实测驱动）。

---

## 📋 待排期 · SCENE-FORMAT-DIMS-001 拆分 `multiline_safe` 为格式能力多维度（Gavin 2026-08-01：先建 todo，后续排期）

> 性质：**内部架构重构**，无用户可见配置项。需一次构建。
> 前置：无。可随时启动。**风险点是它动的正是刚稳定下来的 prompt 构建链。**

### 问题

`multiline_safe`（单 bool）在 `src/main.rs:3073` 传入 `build_optimize_request` 后，**一个人决定了三件事**：

| 它控制 | 位置 |
| --- | --- |
| `<corrected>` 能否跨行 | `build_output_format(multiline_safe)`（`src/llm/mod.rs:795`） |
| F3 用多行列表还是单行内联 | `build_format_instruction_block(multiline_safe)`（`:818`） |
| 是否 `flatten_multiline` 压成一行 | `src/llm/mod.rs:660` |

**这几个问题的答案并不总是一致。已实际撞上一次**：代码编辑器要换行、不要 `- ` 项目符号 —— 当时只能用「方案 A：接受列表」绕过（Gavin 2026-08-01 拍板）。

### 需求矩阵（主控已梳理）

| 场景 | 换行 | 列表 | `#` 标题 | 表格 |
| --- | --- | --- | --- | --- |
| Markdown 编辑器 | ✅ | ✅ `- ` | ✅ 想要 | ✅ 想要 |
| 代码编辑器 | ✅ | ❌ 不想要 | ❌ | ❌ |
| Word / WPS | ✅ | ✅（会自动转项目符号） | ❌ | 视情况 |
| 记事本 | ✅ | `- ` 仅字面字符 | ❌ | ❌ |
| 邮件 | ✅ | ✅ | ❌ | ❌ |

### 设计草案

```toml
newline    = true          # 能否多行注入
lists      = true          # 能否用列表
list_style = "markdown"    # markdown(- / 1.) | plain(• / 1.) | none
headings   = false         # 能否用 # 标题
tables     = false
```

### 影响面

`scene-rules.toml` schema ｜ `SceneRule`（`src/scene/mod.rs:94`）｜ `SceneContext`（`:55`）｜ 解析与传递 ｜ `build_output_format` / `build_format_instruction_block` 从 bool 参数改结构体 ｜ 相关测试。

### 实施前必查

1. **回查 DEC-031 单开关原则** —— 主控初判不触碰（这是内部规则数据，非用户可见开关），但须正式确认
2. 跨平台：`src/scene/mod.rs` 与 `src/llm/mod.rs` 均为平台中立模块，对 macOS 透明，不违反 DEC-033
3. **迁移成本随词表增长而上升** —— 现已 9 个 scene 块，越晚做越贵

### 附带清理（本项一并处理）

`scene-rules.toml:15` 注释称 `multiline_safe` 还控制「强制剪贴板」，**主控 grep 注入侧代码零引用，该句为过时描述**，一并订正。

---

## 📋 待排期 · SCENE-FOCUS-PROBE-001 焦点控件类型探测（UIA / AX）（Gavin 2026-08-01：先建 todo，后续排期）

> 性质：**架构级新能力**，跨平台，需完整评估。
> 🔴 **前置硬门槛：必须先做 PoC，PoC 不通过则不立项。**

### 问题：只有窗口级信号，要判断的却是控件级行为

场景识别拿到的是 `(进程名, 窗口标题)`，**两者都是窗口级**。而「Enter 是换行还是提交」取决于**当前焦点控件**：

| 应用 | 同一窗口内的分歧 |
| --- | --- |
| VS Code / Cursor | 编辑器（Enter=换行）／集成终端（Enter=执行）／AI 面板（Enter=发送） |
| Todo 软件 | 快速添加框（Enter=建任务）／详情备注（Enter=换行） |
| 设计软件 | 文本图层（Enter=换行）／评论框（Enter=发送） |
| 浏览器 | 文档编辑区／评论框／搜索框 |

**当前所有残留误判风险的总根源。** 现在是「按应用整体赌一个答案」。

### 技术路径

| 平台 | API | 信号 |
| --- | --- | --- |
| Windows | `IUIAutomation::GetFocusedElement()` | `ControlType`：`UIA_EditControlTypeId` ≈ 单行；`UIA_DocumentControlTypeId` ≈ 多行。辅以控件 ClassName（如 Windows Terminal 的 `TermControl`） |
| macOS | `AXUIElement` → `AXFocusedUIElement` | `AXRole`：**`AXTextField` = 单行、`AXTextArea` = 多行**。信号比 Windows 更干净 |

接缝已存在：`capture_scene_signals` 两平台签名同形（Windows `src/platform/windows/scene.rs:22`，macOS `src/platform/macos/mod.rs:79` 仍为 stub）。

### 🔴 PoC 必须先回答的三个问题

1. **Chrome / Electron 能否拿到有意义的焦点控件类型？**
   Chrome 默认不开放完整无障碍树（只在检测到读屏器时启用），Electron 应用同理。
   **而 `chrome.exe` 占真实听写量的 36%（151/414，主控 2026-08-01 从 debug.log 实测）。拿不到 = 方案价值砍半。这是头号风险。**
2. **耗时**：跨进程 COM 查询通常 5–50ms，但**对无响应应用可能挂住** → 超时与降级策略是否可行
3. **VS Code 编辑器 vs 集成终端**能否区分 —— 这是最想解决的那个用例

**PoC 规模：半天量级，只验上述三点，不写生产代码。**

### 其他约束

- 跨平台：Windows COM + macOS AX 两套实现，DEC-033 要求不得只产出仅 Windows 可编译的新代码
- 隐私：**严格只取控件类型，绝不读取控件内容**
- 失败降级：拿不到信号时必须退回现有 `(exe, title)` 判定，不得阻塞管线

### 与 SCENE-FORMAT-DIMS-001 的关系

两者正交但互补：本项解决「**当前焦点适合什么**」，前者解决「**适合的东西怎么表达**」。建议**先做拆维度**（收益确定、成本有界），本项走 PoC 门禁。

---

## 📋 待裁定 · 领域级泛化关键词第二批（coder-1 建议，主控未采纳未否决）

coder-1 在 DATA-SCENE-GENERIC-008 中评估后建议的候选：**`思维导图` / `白板` / `表格`**。

已评估并**否决**的：`笔记`（小红书网页标题大量含之，社交类判 doc 方向反了；「笔记本电脑」购物页同样命中）、`文档`（帮助/API/产品文档等阅读页覆盖过宽，失去判别意义）。

**裁定判据（主控 2026-08-01 定）**：看误伤会落到哪个方向 ——
- 误伤 → **doc**：只多给多行与列表，文本仍正确，属优雅降级，**可放宽**
- 误伤 → **把 chat 类应用判成 doc**：多行注入发帖/发消息框，可能把一条拆成多条发出，**必须卡严**

待 Gavin 拍板。

---
## 📋 待排期 · ITN-V2-P6 能产语法族批量收口（Gavin 2026-08-01：先出包，P6 另立）

> 依据：`collab/research/itn-v2-grammar-family-scan-006.md` 的 130 族分类结果
> 前置：本轮出包端测反馈

**范围**：75 个 🔴 能产语法族（`N点钟`/`N秒钟`/`N块钱`/`N毛钱`/`N角钱`/`N位数`/`N年级`/`N节课`/`N件套`/`N句话`/`N日游` 等）从 `[protect.unit_collisions]` 移出，交甲/乙/丙型文法处理。

**核心问题**（Gavin 已实感）：`五毛钱` 在表保持汉字、`三毛钱` 不在表变 `3毛钱` —— 同一表达因数值不同行为完全相反（DEC-038）。

**实施前必须**：① 逐族确认已有文法覆盖，避免移出后变成「漏保护」（输出改坏，比未优化严重）② 反向护栏：55 个 ⚪ 专名族不得误伤 ③ 预判并同步既有断言 ④ 涉及数百词条，需完整回归 + 出包

### 📌 2026-08-04 补充：分类扫描已完成，Gavin 拍板暂缓

> **Gavin 2026-08-04 决定**：「先保持原样，后面我端测了有问题再开单」。**P6 不启动，等端测反馈。**

主控已完成当前词表复扫，成果存 **`collab/research/itn-v2-p6-family-classification-001.md`**，含：

- 129 个多数字族的**四组分类**（A 数量表达 37 移出 / B 地名编号 49 保留 / C 术语 41 保留 / D Gavin 指定保留 2：**N年级 / N节课**）
- 🔴 **新发现**：`一个*` 前缀 **134 条占全表 10% 是挖掘噪声**（`一个三十多岁` 会冻结「三十」不转），删除零专名风险，是 P6 成本最低的第一刀
- 🔴 **新发现**：979 个单例族占 72%，**按族裁决方法在此失效**，需改按尾字聚类
- 三处待 Gavin 裁定：`N分熟` / `N米板` / `N岁时·N月底`

⚠️ **四组分类是主控单方初判，Gavin 未逐组确认**，P6 启动时必须先过一遍，不得直接采信。

**启动前另一道门**：CHANGELOG 026-B 的「12 词条实测确认」已被 TEST-EXEC-026 证伪 4 条，须先做 026 前后行为差分。

### 八、待 Gavin 决定的两项（非阻塞）

- `dispatch.sh` 路径分叉修复时机（`[COLLAB-PATH-SPLIT-001]`）
- 是否加 `.gitattributes` 收口 CRLF 幽灵 diff（`core.autocrlf=true` 且无 `.gitattributes`，已两次干扰验收判断；属仓库级改动、影响 macOS 侧，需协调）

---


## 🍎 [macOS 侧] 2026-08-04 · Phase 4 管线实现规划（**规划稿，尚未派发**）

> 来源：Gavin 2026-08-04 指令「把 macOS 侧管线 Phase 4 的任务规划出来」
> **完整方案见 `collab/research/macos-phase4-plan-001.md`**（含逐任务的影响文件 / 具体改动 / 验收标准 / 边界矩阵）
> 基线：`0adb819`（v0.7.3），主控本次逐文件实测，未采信既有文档结论

**🔑 核心发现（改变了任务分解方式）**：`run_pipeline`（`main.rs:2812-3214`，402 行）与 `spawn_worker_thread`（`:2161-2459`，299 行）虽整体被 `#[cfg(target_os="windows")]` 门控，但**平台接触点分别只有 4 个和 1 个**，其余全是平台中立业务逻辑。**约 700 行核心管线可一次性转为双侧共享**，macOS 侧无需重写任何业务逻辑。这同时修正了 `docs/MACOS-HANDOFF.md` §2.8「管线代码层无法共享」的表述——那是当前状态，不是固有属性。

| 阶段 | 编号 | 内容 | 建议负责人 | 状态 |
| --- | --- | --- | --- | --- |
| A | MACOS-P4-PROBE-001 | 实机可行性探针：cpal 实录 / sherpa-onnx 实跑 / TCC 权限 / CGEventTap 实收 / AX 授权弹窗 | tester-1 | ✅ **已验收**（A1/A2/A5 PASS，**A4 FAIL 查出真 bug**，A3 强度不足待复验） |
| A+ | MACOS-P4-FIXHOTKEY-001 | 修 `KEYBOARD_EVENTS` 事件掩码越界（A4 的下游修复） | coder-2 | ✅ **已验收**（2026-08-04，主控独立复跑 `cargo check --all-targets` 0 errors） |
| B | MACOS-P4-NEUTRAL-001 | 管线去平台化：`HWND`→`WindowId(usize)`、新增契约符号 `foreground_window_id`、去 cfg 门控 | coder-1 | ⏸ **归属待拍板** |
| B | TEST-SYNC-P4-NEUTRAL-001 | 契约单测 + macOS 可达性烟测 | tester-1 | ⏸ |
| C | MACOS-P4-HOST-001 | macOS 事件宿主 + 主循环 + 单实例锁 | coder-2 | ⏸ **选型待拍板** |
| C | MACOS-P4-TRAY-001 | macOS 托盘（**不可与 HOST 并行**） | coder-1 | ⏸ |
| C | **MACOS-P4-OVERLAY-001** | 录音浮层（Recording，P0）NSPanel 1:1 复刻 Win32 | coder-1 | ✅ **已验收**（2026-08-05，返工后主控独立取证：BORDER_GRAY `#070606` 已修正、性能四项已补数、SIGBUS 悬垂指针真缺陷已重构、probe 已删、工作区干净，提交 `d7b9f39`） |
| C | **MACOS-P4-OVERLAY-WIRE-001** | **浮层接线**：接进模块树 + 跨线程请求通道 + 15ms timer 主线程应用 + PipelineEvent 七分支映射 | coder-1 | ✅ **接线部分已验收**（主控独立取证：`mod overlay;` 已加、overlay 5 条单测**首次真进测试二进制**（主控自跑 `--list` 确认）、Windows 导出块零改动、七分支齐全）。⚠️ **实机闭环仍降级**，转入 CFGGATE-001 C 项 |
| **P0** | **MACOS-P4-CFGGATE-001** | 🔴 **修 macOS 专用函数缺 cfg 门控致 Windows 编译必炸**：`main.rs:2880/:2930` 无 `#[cfg]` 却引用 `request_tray_state`（TRAY-001 既有）+ `request_overlay`/`OverlayRequest`（WIRE-001 新增 7 处）。**Windows 侧已编不过一天多无人察觉**。含 11 个 macOS 专属符号全量扫描 + 实机重试 | coder-1 | ✅ **已验收**（主控**自写脚本独立重扫**，非采信 Worker 表：11 符号全部落 cfg(macos) item 内、未门控引用点 **0**；`cargo check --all-targets` CARGO_EXIT=0 / error 0；diff 恰好 +2 行；**主控追加全仓扫描**确认 macos 目录与 main.rs 之外零引用。🟢 **Windows 编译阻塞解除**） |
| C | TEST-SYNC-P4-OVERLAY-WIRE-001 | 阶段三**预研**（只读零改动，产出写 outbox）：5 条单测走查 + 七分支真值表 + 线程契约 + 盲区预估 | tester-1 | 🔄 **进行中**（与 WIRE-001 并行安全，因零文件落地） |
| C | MACOS-P4-OVERLAY-002 | 处理中 / 失焦返显 / 错误三态浮层 + **指示灯形状裁定**（Windows 实为麦克风图标，现实现为实心圆，主控裁定本轮不动） | 待定 | ⏸ |
| ~~C~~ | ~~MACOS-P4-FEEDBACK-001~~ | ~~托盘状态替代浮层~~ → **已按 DEC-036 取消替代定位**（Gavin：不能用托盘图标代替录音窗口，不符合用户体验） | — | ❌ 作废 |
| D | MACOS-P4-SCENE-001 | `capture_scene_signals` 真实现（NSWorkspace + AXUIElement） | coder-1 | ⏸ |
| D | MACOS-P4-PERM-001 | `AXIsProcessTrustedWithOptions` 真实现（现为只打 log 的假实现） | coder-2 | ⏸ |
| D | MACOS-P4-AUTOLAUNCH-001 | 自启动（建议 SMAppService） | 待定 | ⏸ |
| **P1** | **MACOS-P4-AXINJECT-001** | **辅助功能 API 直写替代剪贴板注入**（解决剪贴板被污染/图片被永久覆盖）—— Gavin 2026-08-05 拍板独立 P1；**同日 12:1x 派发 coder-2**。三条主控裁定：①必须 `kAXSelectedTextAttribute`，**严禁** `kAXValueAttribute`（会抹掉用户已输入内容）②必须 `AXUIElementSetMessagingTimeout(0.5)`（否则无响应 App 会挂死 pipeline worker 线程）③必须检查 `AXError` 返回码 | coder-2 | ✅ **已验收**（三条裁定逐条 Read 核对全部落实；CARGO_EXIT=0/error 0；仅 `injection.rs` +122/−1）。🔴 **但主控 review 查出静默丢词风险** → 见下行 002 |
| **P1** | **MACOS-P4-AXINJECT-002** | 🔴 **堵 AX 静默丢词**：部分实现 AX 的 App（Electron/Java Swing）接受 `SetAttributeValue` 并返回成功却不真插入 → `inject_text` 拿 Ok 即 return、**永不走剪贴板兜底** → 用户整段话消失。**推翻了 001 立项前提「最坏等同现状」**。修法：`AXUIElementIsAttributeSettable` 预检 + AX 成功日志带 `AXRole`。已否定两条歧路（写后读回：选区塌缩读回空串不可作判据；role 白名单：误伤自定义控件） | coder-2 | 🔄 **进行中** |
| C | **MACOS-P4-OVERLAY-WIRE-002** | 抽纯函数 `overlay_request_for_event` 使七分支真值表可单测（**强制穷举 match，禁 `_ =>` 通配符**——通配符会让将来新增 PipelineEvent 变体静默落进 Hide）。由来：tester-1 预研指出映射内联在 handler 里夹三种副作用、单测调不动 | coder-1 | 🔄 **进行中** |
| C | TEST-SYNC-P4-OVERLAY-WIRE-001 预研 | ✅ **已交付**（13683 B）。**核心发现（主控复核认同）：overlay 现有 5 条单测 4 条凑数 1 条半有效** —— `decay_rate_matches_windows` 是 `assert_eq!(常量, 它自己的字面量)`；三条波形测试独立重算公式不调生产路径，且 `lx<rx`／`8<=16` 恒真。**本次接线目前零真护栏** | tester-1 | ✅ 预研完成，阶段三待 WIRE-002 交付 |
| D | MACOS-P4-READBACK-001 | AX 回读（学习路径）—— ⚠️ **主控已收回「相对 Windows 的能力优势」表述**：300ms 观察窗 + Word/Google Docs 读不到 + 全文 diff 昂贵，三重打折后产出存疑；建议优先考虑 `WORDBOOK-CORRECTION-UI-001` 显式纠错路线 | 待定 | ⏸ **降级** |
| ~~E~~ | ~~MACOS-P4-OVERLAY-001~~ | 已上移至阶段 C（DEC-045） | — | ↑ |
| E | MACOS-P4-BUNDLE-001/002 | `.app` 打包 + Info.plist TCC 声明 + 自签名证书（BUNDLE-002 已改自签名，禁 ad-hoc） | coder-2 | ✅ **已完成**（BUNDLE-002 2026-08-05：`build-macos.sh` 自签名 Feiyin Dev，实机 Build completed + Authority=Feiyin Dev；详见 outbox/coder-2/result.md）。⚠️ 公证仍不可做（无 Apple Developer 账号） |

**边界评估结论**：阶段 B 独占 `src/main.rs`，期间禁止任何其他任务碰它；阶段 C 的 HOST 与 TRAY **必须串行**；阶段 D 的 SCENE / PERM / AUTOLAUNCH **可三路并行**，唯一交汇点 `macos/mod.rs` 的 re-export 行由主控统一改一次。

**⏳ 阻塞在 Gavin 决策上的 5 项**（详见方案 §4）：① 阶段 B 归属与 Windows 零回归验证方 ② 事件宿主选型（**建议复议 DEC-015 的 Tauri，改用 winit**）③ 四个浮层是否进第一版 ④ Apple Developer 账号 ⑤ AX 回读是否提前。

---

## 🍎 [macOS 侧·待排期] MACOS-P4-AXINJECT-001 · 辅助功能 API 直写替代剪贴板注入

> **⚠️ 本条仅适用于 macOS 侧，Windows 侧无需处理**（Windows 走 SendInput / 剪贴板双通道，机制不同、无此缺陷）。
> 来源：Gavin 2026-08-04 端测关切「现在的回写方式确实会污染剪贴板，干扰到用户剪贴板输入」。
> 参照：竞品 **Typeless** 官方文档明示其用 Accessibility 权限「paste text into any text field」，**不经剪贴板**（[installation-and-setup](https://www.typeless.com/help/installation-and-setup)）。

### 现状与缺陷（主控实测 `src/platform/macos/injection.rs:49-64`）

当前 `inject_via_clipboard` 流程：`pbpaste` 存旧内容 → `pbcopy` 写新文本 → sleep 50ms → enigo 发 Cmd+V → sleep `delay_ms` → `pbcopy` 恢复旧内容。

**已有恢复逻辑，但存在四个真实缺陷（按严重度排序）**：

| # | 缺陷 | 后果 |
| --- | --- | --- |
| **1** | **`get_clipboard_text()` 只用 `pbpaste`，仅支持纯文本**（`:119-134`） | 用户剪贴板里若是**图片 / 富文本 / 文件 / 多格式数据**，`old_content` 取不到 → `None` → **恢复分支根本不执行** → **用户原剪贴板内容被永久覆盖丢失**。这不是"污染"，是**数据丢失** |
| 2 | 恢复是尽力而为（`let _ = set_clipboard_text(&old)`，忽略错误） | 恢复失败无感知、无日志 |
| 3 | 约 **100ms+ 时间窗**内剪贴板是我们的内容 | 用户此刻手动粘贴会拿到错误内容 |
| 4 | **剪贴板历史工具**（Paste / Maccy / Raycast 等）会记录每一次写入 | 每次听写都往用户剪贴板历史里塞一条垃圾，长期使用体验很差 |

### 目标方案

用 **AXUIElement 直写**替代剪贴板通道：取焦点元素（`kAXFocusedUIElementAttribute`）→ `AXUIElementSetAttributeValue` 写 `kAXValueAttribute`，或用 `kAXSelectedTextAttribute` 做插入。**全程不碰剪贴板。**

### 实施要点

- **保留剪贴板通道作为兜底**：AX 对某些 App（Electron / 部分 Java App / 未实现 AX 的自绘控件）不可用，失败时回退现有 `inject_via_clipboard`，与 DEC-018「clipboard-first + enigo fallback」的分层思路一致，只是把优先级改为 **AX → 剪贴板 → enigo.text()**
- **权限已具备**：辅助功能权限本就是热键（CGEventTap）的前置条件，`MACOS-P4-PERM-001` 已补全授权弹窗，**不需要新增任何权限申请**
- **保底路径不受 App 差异影响**：AX 写不进去就回退现有剪贴板通道，**最坏情况等同现状，不会更差**

### 🔴 与 `MACOS-P4-READBACK-001` **不得捆绑**（Gavin 2026-08-05 拍板：AXINJECT-001 独立 P1）

> **主控此前建议"两者合并为一个批次"，该建议已作废并收回。** 它们只是碰巧用同一套 `AXUIElement` API，但**价值与风险完全不同**，捆绑会让高价值的直写被低价值的回读拖累。

| 项 | 价值 | 依赖 |
| --- | --- | --- |
| **AXINJECT-001（本条，注入）** | **高且确定**。解决真实数据丢失 + 剪贴板历史污染。**只需"写得进去"，有剪贴板兜底** | 无 |
| `READBACK-001`（回读，学习） | **存疑**，见下 | 受三重打折 |

**主控收回的一处过乐观表述**：此前写「macOS 的 AX 能真正读回，是本平台相对 Windows 的能力优势」。**该表述不准确**，实测复核后三点打折：

1. **观察窗只有 300ms**（`main.rs:161` `AUTO_LEARN_OBSERVE_MS = 300`，`:3662` sleep 后即读一次就结束）。用户不可能在 300ms 内看完注入文本、发现错字并改掉——**真实纠错发生在数秒后，这条路径设计上就抓不到几乎任何真实纠错**。此缺陷与平台无关，Windows 侧同样存在
2. **AX 覆盖面确实比 `WM_GETTEXT` 广**（原生 NSTextView / 备忘录 / Safari 输入框可读，而 `WM_GETTEXT` 在现代应用基本全废），**但读不到 Microsoft Word（非原生 AX 文本控件）与 Google Docs（编辑面是 canvas）** —— 而这恰是长文写作主场
3. **全文 diff 代价高**：`extract_changed_text`（`main.rs`）走公共前缀+后缀夹逼，**数学上对整篇文档成立、不需要知道光标位置**，但要把全文 `chars().collect::<Vec<char>>()` **两次**（before/after）。50 页文档每次听写数十 MB 临时分配

**→ 真要修好「注入后纠错学习」，正确路线不是 AX 回读，而是既有待排期项 `WORDBOOK-CORRECTION-UI-001`**（注入后浮层显示「纠错」小按钮，用户点了才进编辑）：**显式交互，不猜、不轮询、不受 App 差异影响，且没有时间窗问题**。

### 影响文件（预估）

`src/platform/macos/injection.rs`（主体）+ 可能新增 `src/platform/macos/ax.rs`。**不碰 `src/platform/windows/**`，不碰平台中立模块，对 Windows 侧零影响。**

### 优先级：**P1 独立排期**（Gavin 2026-08-05 拍板）

**不阻塞当前闭环** —— pbcopy+Cmd+V 现在能工作。但 Gavin 日常使用会持续被缺陷 1（复制的图片被永久覆盖）与缺陷 4（剪贴板历史被塞垃圾）硌到，**Phase 4 主体交付后立即排期，不等 READBACK**。

---


## 📋 待排期 · ITN-COLLISION-TYPEB-001（Gavin 2026-07-30：先生成待办，以后再动手）

> 来源：RESEARCH-ITN-LEXICON-001（`collab/research/itn-lexicon-collision-001.md`）
> 前置：Type A 净化版落地（进行中）｜ **本项需改 Rust 代码，不是纯数据**

**问题**：`is_unit` 用 `s.starts_with(u)`（`src/itn.rs:259`）+ 中文无词边界 → 任何以单字单位词开头的词都会让前面的单字数字被误转。已实证一例（`三角形`→`3角形`，已由几何白名单挡住）。潜在碰撞词 **29,774 条**（Type B：单位字开头，如 `批发`/`元素`/`度假`/`节目`/`升级`/`克服`）。

**⚠️ 主控核实的关键结论（研究报告未发现，实施前必读）**：

1. **Type B 词放进 `protect.proper_nouns` 是 no-op** —— `check_protection`（`:1063`）的匹配起点是**数字位置**（`rest = chars[start..]`，`start` 为数字下标，调用点 `:678`），既有条目全是数字开头（`三亚`/`五一`/`八达岭`）。文本「三度假」的 `rest` 是「三度假」，**不以「度假」开头**，永不命中。研究报告建议的落地方式对这 93% 的词无效
2. **正确改法在 `is_unit` / `decide_conversion` 侧**：判断「数字之后的文本是否以某碰撞词开头、且该词比单位匹配更长」→ 是则不算单位语境
3. **性能必须一并处理**：`check_protection` 是对 **Vec 线性遍历** `starts_with`（不是 HashSet 查表，报告此处亦有误）。29,774 条 × 每个数字位置 = 每次听写十万量级字符串比较 → 需改**按首字分桶或 Trie**，并给实测数据

**实施前必须完成**：许可证原文证据（见 Type A 任务）｜ 完整影响面评估 ｜ 性能实测 ｜ 反向护栏测试（must-convert 表达不被误保护）

---


## ⏸ 待 Gavin 拍板 · RESEARCH-SCENE-COVERAGE-001 场景词表扩展研究（2026-07-28 方案已回）

> 从原 `todo.md`「进行中」大节中抽出——该大节其余内容（macOS 07-30 交接接管等）已迁入 `todo-archive.md`，
> 只有本项仍**待 Gavin 拍板三项**，故保留在 todo。

### 2026-07-28 · RESEARCH-SCENE-COVERAGE-001 场景词表扩展研究（方案已回，**待 Gavin 拍板三项**）

| 编号 | 内容 | 产出 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| RESEARCH-SCENE-COVERAGE-001 | 场景感知软件词表与分类体系扩展研究（纯研究，零文件改动） | `collab/research/scene-rules-expansion-001.md`（397 行） | coder-1 | ✅ 已验收（**含主控三处实质修正**） |

**主控独立核验结果**：

- **边界合格**：`git status` 零改动，`scene-rules.toml` / Rust 源文件全未碰；result.md 4300 字节非空（[COLLAB-WRITE-001] 未复发）
- **✅实测层可信**：抽查 6/6 全部属实——Obsidian 目录确无 `Obsidian-helper.exe`、`D:\xshell\Xagent.exe` 存在、`MarkText.exe` / `Koodo Reader.exe` 路径正确、`wezterm.exe` 确在 `%LocalAppData%\Programs\WezTerm\`（主控首轮搜索因未覆盖该根目录误判为不存在，二次广域搜索证实 coder-1 无误）、四个 AppxManifest 的 Executable 字段逐一复读一致（OneCalendar→`CalendarApp.Gui.Win10.exe`、OutlookForWindows→`olk.exe`、MSTeams→`ms-teams.exe`、Claude→`app\Claude.exe`）
- **❌ 主控修正一：10 条 title_keywords 是 no-op**。报告建议把「微博/小红书/知乎/Jira/TAPD/禅道/Linear/Teambition/Salesforce/Zendesk」加进 **browser 块**的 title_keywords，目的是「浏览器场景细分时命中」。但 `src/scene/mod.rs:162-164` 浏览器细分循环里有 `if other_rule.kind == SceneKind::Browser { continue; }` —— **显式跳过浏览器自身的 title_keywords**。浏览器 exe 在优先级 1 命中后直接返回 Browser，这 10 条永远不会生效（只有在 exe 未命中任何规则的优先级 2 路径才会被查，那不是目标场景）。报告第 6.2 节还引用 `:159-177` 称该机制「已正确处理」，属误读 `continue` 分支。**结论：10 条全部作废**；若确要生效，必须放进**非 browser 的 kind 块**（如 chat）
- **❌ 主控修正二：`OneNote.exe` 是零效果重复条目**。exe 匹配大小写不敏感（`:133` 编译期 `to_lowercase` + `:152` 输入 `to_lowercase`），`OneNote.exe` 与表内既有 `ONENOTE.EXE` 归一化后完全相同。报告自己在第 127/313 行两次援引「toml 不区分大小写」，此处却当作新增项，自相矛盾。**22 条新增实际净 21 条**
- **⚠️ 主控修正三：✅官方 置信度标注失真（最重要）**。报告附录第 397 行自述「WebFetch 核实国内站点被 JS 占位，**主要依靠本机实证 + 通用软件知识**」——「通用软件知识」即凭记忆。故 15 条标 ✅官方 的条目（豆包/Kimi/通义/文心/ChatGLM/GLM/纳米/Perplexity/Zoom/腾讯会议/Linear/Figma/Mailbird/The Bat!/Nu），其证据实际只支撑「该产品有 Windows 版」，**不支撑「进程名就叫 X」**。这恰好触碰本次任务设定的核心红线。**须降级为 ⚠️ 未证实**
- **争议项（主控不同意见）**：报告建议删除 6 条历史推测项。主控评估**成本收益不对称**——不存在的 exe 名在运行时永不命中、开销为零（HashSet 查表），删除买不到任何收益；而万一其中某条在特定版本真实存在，删除即制造静默回归。**建议改为保留 + 注释标注「未证实」**，与 DEC-031-⑤「词表尽可能详细」一致
- **分类结论认同**：D5 六个候选场景全部归入现有 5 类、不新增 kind——主控同意（新增 kind 要改 Rust 枚举 + 解析 + 单测 + 重新出包，收益不足）。但 **Figma 归 ide_terminal 存疑**：该 style 是「保留技术术语/代码标识符、无客套」，而 Figma 文本框内容多为设计文案而非代码，`browser` 的「web-friendly 简洁单行」更贴切

**Gavin 三项拍板（2026-07-28，全部按主控建议）**：
1. **6 条历史推测项** → **保留 + 注释标 ⚠️未证实**（不删除）
2. **15 条未证实 exe** → **落地 + 注释标 ⚠️未证实**（错条目零成本；待 SCENE-OBS-001 日志实证）
3. **Figma** → 归 **browser**（非 ide_terminal）

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| IMPL-SCENE-COVERAGE-001 | 词表扩展实施：6 条存疑项保留改注释 + 新增 21 条 exe（✅实测 6 / ⚠️未证实 15）+ doc 块补 4 条工单 title_keywords + Figma 归 browser + Skype 注释 | `scene-rules.toml`（仅根目录） | coder-1 | ✅ **已验收（含主控两处修正）** |
| TEST-SYNC-SCENE-COVERAGE-001 | 测试同步：P0-1 `BUILTIN_RULES` 解析单测（堵住「toml 语法错→静默全 Unknown」黑洞）+ P0-2 特殊字符条目（`The Bat!.exe` / `Koodo Reader.exe`）+ P0-3 doc 块 title_keywords 浏览器细分生效 + P0-4 反向护栏锁死 `:162-164` continue + P0-5 归类决策断言 + P1 大小写不敏感 | `src/scene/mod.rs` `#[cfg(test)]` | tester-1 | ✅ **已验收** |
| TEST-EXEC-SCENE-COVERAGE-001 | 全量回归 686/0/8 + Tauri 53/0 + Step2/3/4 SKIP + 三副本 sha256 一致 + 新实例 PID 23056 零 parse error | — | tester-1 | ✅ **已验收（运行时实证已由 Gavin 补证闭环）** |
| BUILD-RELEASE-SCENE-COVERAGE-001 | 出包：**只重建主程序**（`cargo build --release`），把 165 条新词表经 `include_str!` 嵌入内置默认；**跳过 Tauri UI 构建**（`src-tauri/**` 与 `ui/**` 零改动，重建只产出功能等价二进制）；关键验证=新 exe 内检索 `NanoSearch.exe` / `Koodo Reader.exe` 两个探针字符串确认缓存未复用旧产物；版本号维持 0.7.2 不动 | — | tester-1 | ✅ **已验收（含主控一处事实修正）** |

**BUILD-RELEASE 验收记录（2026-07-28 主控独立取证）**：

- **产物 sha256 两副本一致**：`e35679bd…`（target/release + Publish）；crash-reporter `8bfabfb5…` 亦一致——tester-1 报告写「继承，未查」，实际它 18:41 已随 `cargo build --release` 一并重建并同步，主控补查确认无误
- **词表嵌入验证主控独立换探针**：不复用 tester-1 那 5 个，改用 6 个字符串复查，含**最易触发 toml 解析问题的 `The Bat!.exe`（含 `!`）与 `Koodo Reader.exe`（含空格）**、以及本批次归类决策项 `Figma.exe` 与 `Teambition`，全部命中 → 缓存确未复用旧产物，165 条已 `include_str!` 嵌入
- **ProductVersion 0.7.2.0 未变**；`scene-rules.toml` 三副本 sha256 `7b01b33c…` 一致
- **边界合规**：`git status` 仅 `CHANGELOG.md` 一处改动，**源文件零改动**、未碰 Tauri UI / npm build / 版本号；项目级 result.md 1251 字节非空且对应本任务
- **❌ 主控修正 · 「已知缺口」不成立**：报告称「`Publish/voice-ime-ui.exe` 沿用 07-24 旧版 10,013,184 B」。**Publish/ 根本没有该文件**——已于 07-28 按 Gavin 指令删除。Publish 内是正确的 `feiyin-ime-ui.exe`（10,027,008 B，07-27 20:31，sha256 `0d76eca1…`，即 v0.7.2 那版）。那个 07-24 文件只在 `target/release/`（本次已随磁盘清理删除），且生产代码 `src/main.rs:436~478` 七处全走 `feiyin-ime-ui.exe`。**本次出包不存在 UI 缺口**
- **⚠️ 观察（不阻塞）**：冒烟实例 PID 18928 在主控 21:40 复核时已不存在，符合 [SMOKE-VANISH-001] 既有模式，产物结论不依赖该进程

**git 收口（2026-07-28）**：提交 `695e50e`（3 文件 +195/−7：`scene-rules.toml` / `src/scene/mod.rs` / `CHANGELOG.md`），已按 Gavin 指令 push → `fb230f9..695e50e`，push 后恢复 clean remote URL，`.git/config` token 残留数 0。

**⚠️ 已知副作用（出包后确认，待 Gavin 定夺）**：本次维持 **0.7.2 不升版**（Gavin 未指示升版，遵守「版本号禁止擅改」）。故现存在**两个内容不同、但 ProductVersion 均为 0.7.2.0 的主程序构建**：07-27 20:31 那版内置 144 条词表（sha256 `7fbb1e4b…`，已被覆盖）/ 07-28 18:42 本版内置 165 条（sha256 `e35679bd…`，当前 Publish 中的）。可追溯性下降，仅靠 sha256 区分。若更看重可追溯性可升 0.7.3 重出包——但注意**本次磁盘清理已删除 release 中间产物，重出包需全量重编（含 CTranslate2 C++，预计 20+ 分钟）**，不再是 2 分钟。

**TEST-EXEC 验收记录（2026-07-28 主控独立取证）**：

- **数字链自洽**：686 = TEST-EXEC-20260727 基线 672 + 本轮 TEST-SYNC 新增 14，三方来源相加严丝合缝
- **主控定向抽跑 `builtin_rules_parse_ok` 单条 → ok**（未只信汇报表格）。这条是本批次最大风险的唯一守门人，**至此真实 165 条 toml 可被 `toml` crate 正确解析已获权威验证**。注：`BUILTIN_RULES` 是根目录 `scene-rules.toml` 的 `include_str!`，与运行时加载的 `target/release/scene-rules.toml` **sha256 逐字节相同**（`7b01b33c…`），解析保证可传递
- **三副本 sha256 主控亲自复核一致**：`7b01b33ca90b6d78…`（根 / target/release / Publish 三处），同步时间 14:03:26~27
- **实例复核属实**：PID 23056、`Responding=True`、路径 `target/release/feiyin-ime.exe`、启动 14:03:59（紧接三副本同步之后）
- **✅ 遗留项已由 Gavin 补证闭环（2026-07-28 16:38，决定性证据）**：Gavin `-debug` 重启（PID **5968**，启动 16:37:56 = `08:37:56Z`）并实际录音后，日志给出完整证据链：
  1. `08:38:04.415Z INFO feiyin_ime::scene] Scene rules loaded from "...\target\release\scene-rules.toml"` —— 主控查证 `src/scene/mod.rs:243` 该 `log::info!` **只在 `toml::from_str` 的 `Ok(r)` 分支打印**（`Err` 分支走 `:246` 的 `log::warn!` 并回落内置默认）。**故这一行本身即是「外置新 toml 运行时解析成功」的直接证明**，而该文件 sha256 `7b01b33c…` 正是同步后的 165 条版本
  2. 同一毫秒 `08:38:04.415Z Scene context: app_exe="WindowsTerminal.exe", kind=IDE/terminal, multiline_safe=false, f4_injected=true` —— 分类与 F4 注入全链路正常（惰性初始化的印证：首次 classify 与 rules 加载同刻发生）
  3. `08:38:25.629Z` 第二条 Scene context 同样正常；全日志 `Scene rules parse error` / `Scene builtin rules parse error` 命中数 **0**
  4. 时序自洽：`08:37:36Z` 那条 load 属被重启掉的上一实例（早于 PID 5968 的 `08:37:56Z`），`08:38:04Z` 属当前实例 —— **每进程恰好一次加载**，与 `OnceLock` 语义吻合
  - **结论：144→165 条新词表已在运行时真实生效，本批次运行时验证闭环，无任何遗留。**

- **（历史记录，问题已解决）**验收当时的证据空洞：tester-1 那轮的新实例自 `06:03:59Z` 启动后**从未产生任何 `Scene context:` 行**。主控查证根因：`classify_scene` 全仓唯一调用点在 `main.rs:2962` 的录音流程内，`RULES` 为 `OnceLock` 惰性初始化 → **无录音 = toml 从未被读取**。因此「新实例零 parse error」是**空洞的消极证据**——未尝试解析，何来错误。日志中最后一条 `Scene rules loaded from ...target\release\scene-rules.toml` 停在 `04:47:47Z`（旧实例 PID 18548），新实例无此行。**决定性证据只需 Gavin 触发一次听写**：正常应打出 `app_exe=..., kind=..., f4_injected=true`；若 toml 解析失败会退化为 `kind=unknown, f4_injected=false`（空规则集），一次即可分辨
- **tester-1 诚信记录（正面）**：其 result.md §72 **主动如实披露**了 `Scene context:` 未取得，并自行正确诊断出「lazy OnceLock 且需录音触发」的机制原因，未虚报。这是 [TESTER-FABRICATED-REPORT-001] 四次同模式事故后首次在同类情形下如实交底，予以记录
- **边界合规**：`git status` 无新增源文件改动（仅 IMPL/TEST-SYNC 既有改动）；未出包、未改版本号、未重建 exe；result.md 4566 字节非空
- ~~**已知可接受状态**：exe 内置默认词表仍是旧 144 条、外置 toml 为新 165 条，外置若被删除会静默退回 144 条~~ → **✅ 已于 2026-07-28 18:42 的 BUILD-RELEASE-SCENE-COVERAGE-001 消除**：内置与外置现均为 165 条，外置文件缺失也不再退化

**TEST-SYNC 验收记录（2026-07-28 主控）**：

- **边界完好**：`git diff --numstat src/scene/mod.rs` = **+154 / −0**（纯追加零删除），唯一 hunk 在 `@@ +858,154 @@`，深在 `#[cfg(test)] mod tests`（起于 `:319`）内部；**`scene-rules.toml` 的 numstat 仍是 37/7 与 IMPL 交付时逐字一致，coder-1 的成果未被覆盖**
- **断言方向逐条核对正确**（写反了测试就变成锁死 bug）：P0-3 四条断言的是 `SceneKind::Doc` 而**非** `Browser`——这是验证主控替换 no-op 方案的设计是否成立的关键，方向对了才有意义；P0-1 确实直接用 `toml::from_str::<Rules>(BUILTIN_RULES)` 并把 `parsed.err()` 带进 panic 消息，**没有**退化成吞掉 `Err` 的 `compile_rules_from_content`
- **P0-4 的诚实处理值得记一笔**：任务书要求找一个「只在 browser 块出现」的独有关键词，tester-1 核对后发现 browser 块的 title_keywords 与 email/doc 块**完全重叠、不存在唯一词**，遂按任务书给的备选路径改用内联 fixture 构造（browser 块含独有词 `UniqueBrowserOnlyKeyword`），并在注释里写明为何改用 fixture。**没有硬凑一个假的唯一词来交差**
- **实际交付 14 条 `#[test]`**（汇报写「P0×5+P1×1=6 条」是按**类别**计数，函数数为 14），全文件 scene 单测 46 → **60**
- **主控独立 `cargo check --tests` 0 errors**（81 warnings 全为既有），未采信 tester-1 自验结论
- **一处遗留观察（不阻塞）**：P0-3/P0-5/P1 用的是既有 helper `classify_builtin`（`:329`），它走全局 `rules()`，而 `rules()` 会**优先读 exe 同级的外置 toml**。cargo test 下测试二进制在 `target/debug/deps/`，该目录无 toml 故回落内置默认，结论正确；但若该目录未来出现 toml 副本，这批测试会静默测错文件。属既有 helper 的固有性质（既有 SCENE-AI-AGENT-005~008 同样使用），非本次引入，记录备查。**P0-1 不受影响**（直接解析 `BUILTIN_RULES`，环境无关）

**IMPL 验收记录（2026-07-28 主控独立取证）**：

- **逐条断言核查全过**：`OneNote.exe` 未误加(0) / `ONENOTE.EXE` 仍在(1) / `Figma.exe` 在 browser 块(`:297`) / `ChatGLM.exe`+`GLM.exe` 并存(2) / **browser 块 title_keywords 零改动**（确认没走回 no-op 老路）/ 6 条存疑项全部保留(6) / 新增条目无一条裸写置信度（唯一无标注行是 Skype 注释修改，属 D 项非新增）/ UTF-8 无 BOM（首三字节 `23 20 73`）/ Rust 源文件零改动 / 未碰三副本
- **注释质量超出要求**：`NewMailEngine.exe` 的「火狐邮件」事实错误已改正并写明理由；`wezterm.exe` / `git-bash.exe` 注释如实标注「实际前台窗口通常是已在表内的 `mintty.exe`/`wezterm-gui.exe`，此条为补漏」——没有虚报价值
- **❌ 主控修正一 · coder-1 的基线质疑不成立**：他上报「原文件实际 141 条、任务书 144 有 3 条误差、终值 162」。主控用两种独立方法复核——① `git show HEAD:scene-rules.toml` 取改动前原始文件计数 = **144**；② 严格「首个非空白字符为双引号」行计数，原始 144 / 当前 **165**。git diff 数字自洽：+29 引号行 = 21 新增 + 7 注释重写 + 1 关键词行，−7 = 被重写的 7 行。**141 与 162 两个数字均错，新增数 21 本身无误，正确表述是 144→165。**
- **❌ 主控修正二 · 「cargo test 48 passed ⇒ TOML 语法通过」推理不成立**：现有 scene 单测**全部**用 `compile_rules_from_content` 内联 fixture，无一条读 `BUILTIN_RULES`。测试通过只能证明文件是合法 UTF-8（`include_str!` 编译期校验编码），证明不了 toml 可解析——而解析失败正是本批次最大风险。这与上一轮研究任务的 ✅官方 标注失真属**同一类错误：把不构成证明的东西当成证明**
- **主控过渡验证**（WSL Python 3.10 无 tomllib/toml/tomli，未做真解析）：结构化校验全通过——数组内每行均符合「条目 + 可选尾注释」或纯注释模式（零可疑行）、全文件双引号成对、方括号 11:11 配对、`[[scene]]` 块数仍为 8。**权威解析验证交 TEST-SYNC 的 P0-1 单测 + TEST-EXEC 的运行时 debug.log 核查**

**主控对 title_keywords 的替代设计（派发时补入，非 coder-1 原方案）**：原 10 条作废后，改为只在 **doc 块**加 `Jira / TAPD / 禅道 / Teambition` 4 条——doc 的 kind 非 Browser，浏览器细分循环（`scene/mod.rs:161-177`）会查到它，浏览器开 Jira → 重分类 doc → `multiline_safe=true`，工单描述可得多行结构化输出，与表内既有「腾讯文档 / Google Docs」同一条路径。**排除 `Linear` 关键词**（英文常用词，"linear regression" 会误命中；`Linear.exe` 走 exe 精确匹配即可）、排除微博/小红书/知乎（browser 默认已够）、排除 Salesforce/Zendesk（低频）。

**⚠️ 本批次最大风险（已写入任务书）**：`scene/mod.rs:258-267` 运行时 toml 解析失败只打 `log::warn!` 后**降级空规则集 → 所有场景全变 Unknown**，无用户可见报错。`The Bat!.exe`（含 `!`）与 `Koodo Reader.exe`（含空格）是最易出错的两条。现有 46 条 scene 单测用内联 fixture，**覆盖不到真实 toml** —— 故 TEST-SYNC 必须补一条解析 `BUILTIN_RULES` 的单测把这个洞堵上。

**主控增补建议（省一轮研究）**：SCENE-OBS-001 刚落地的 `Scene context:` 运行时日志会打印 `app_exe`——Gavin 日常使用中打开豆包/Kimi 等桌面版时，**真实进程名会自动出现在 debug.log 里**。与其再派一轮网络查证，不如先落地带 ⚠️ 标注的条目，用实际使用日志做零成本实证收口。


---

## 等 Gavin 拍板

| 事项 | 说明 |
| --- | --- |
| 翻译方向是否改双向全自动 | 现 v1 语义：翻译热键方向由隐藏字段 `translation.target_language` 决定（默认 Chinese），反方向语音自动跳过不翻译；`contains_han` 只做同语种跳过门控。若期望双向全自动需另立项——LLM 路径易改，NLLB 离线路径需按方向换模型（LANG-AUTO-001-CORE 验收时提出，2026-07-14） |
| FORMAT 保底层（A 方案）是否追加 | 不开 LLM 的用户是否需要规则层语气词去除保底（仅"嗯/啊/额"必删项，不碰口头禅，可复用 itn-rules.toml 外置模式）；现方案立场：不开 LLM 不做语义格式化 |
| DEC-028 Qwen3 收尾一项 | `qwen3_asr_url` 默认值维持 dashscope 还是留空强制用户配置（Gavin 实际用工作空间 endpoint，默认值对 MaaS key 用户无效） |
| TELEGRAM-RESTART-001 修复路线 | 见「未排期任务」表 |
| accuracy 体感根因收口（已降级，非阻塞） | 三选一：F1 DEBUG-AUDIO-DUMP-001（`--debug` 存喂模型前音频，小改动）｜F2 Gavin 录 5-10 条日常短指令语料 ｜ 提供当时 accuracy 出错的具体实例（定向复现最快）。**2026-07-25 起进一步降优先级**：accuracy 模型已从 UI 隐藏（ASR-HIDE-ACCURACY-001），用户侧不可达（研究报告 RESEARCH-ASR-ACCURACY-002/003 见 collab/research/） |

---

## 已知遗留问题（待修复）

| 编号 | 问题 | 文件 | 优先级 |
| --- | --- | --- | --- |
| ASR-HALLUC-SEGMENT-001 | accuracy 长音频 VAD 分段中段语义级幻觉（40.1s 语音切 3 段，中段被无关内容整段替换），现有三重兜底（字/秒、重复、空输出）全部拦不住；候选方案：每段用 CTC 交叉转录比对字符重合度 | `src/transcription/mod.rs` | **挂起**（2026-07-25 Gavin 拍板暂不排期：accuracy 已从 UI 隐藏，触发路径不可达。**若未来重开 accuracy 或引入其他 LLM-decoder 类 ASR 模型，必须同批立项**，详见 troubleshooting [ASR-HALLUC-SEGMENT-001]） |
| TEST-FIX-002 | App.test.tsx 缺少 `@tauri-apps/api/core` mock，2 个用例 FAIL | `ui/src/App.test.tsx` | 低 |
| TEST-FIX-003 | Wordbook.test.tsx `getByRole("dialog")` 无 role 属性时报错 | `ui/src/pages/Wordbook.test.tsx` | 低 |
| TECH-DEBT-001 | parse_version 实现不一致：主程序先 split('-') 再 split('.')，Tauri 侧先 split('.') 再 split('-')，prerelease 处理结果有差异 | `src/version_check/mod.rs` + `src-tauri/src/version_check.rs` | 低 |
| ACC-DEGRADE-UI-001 | accuracy 静默降级 performance 时 UI 仍显示 accuracy（可观测性缺口，Gavin 环境不触发；accuracy 已隐藏后影响进一步收窄） | `src/transcription/mod.rs:532-547` | 低 |
| MOJIBAKE-COMMENT-001 | `main.rs` 既有 mojibake 注释（历史编码损伤，非功能影响），归入待清理小项（LANG-AUTO-001-CORE 验收时发现，2026-07-14）。**2026-07-31 macOS 侧主控全文件扫描更正计数：实际 5 处** —— `2483 / 2673 / 2689 / 2722 / 2962`（原记「~L2996 一处」偏少）。已核实为既有（基线 `a38a315` 即存在），且 `0adb819` 批次已顺手清掉另一处同源乱码。清理时须以 UTF-8 无 BOM 读写 | `src/main.rs` | 低 |

---

## 未排期任务

### PLATFORM-001 · macOS 跨平台（Phase 4-6）

| 编号 | 任务 | 前提条件 |
| --- | --- | --- |
| MAC-008 | macOS 构建环境 + CI 配置 | Phase 3 ✅ |
| MAC-009 | 代码签名 + Notarization | Apple Developer |
| MAC-010~013 | E2E/热键/注入/Overlay | MAC-008 |

### 其他待排期

| 编号 | 任务 | 备注 |
| --- | --- | --- |
| PROMPT-CLEANUP-001 | 默认 system_prompt（src/i18n.rs default_system_prompt_en）Rule 4/5（Markdown/List Formatting）与 F1~F4 指令体系职责重叠清理：格式化策略应统一归 F 段管辖，默认 prompt 只留纠错/语义职责。注意 system_prompt 持久化在用户 config，改默认仅对新建配置生效，存量靠 FMT-LLM-002 的 F3 Override 兜底 | 待 Gavin 排期（FMT-LLM-002 审计发现，2026-07-13） |
| WORDBOOK-CORRECTION-UI-001 | 注入后 overlay 纠错入口（Gavin 2026-07-06 选定方案 2）：识别注入完成后 overlay 短暂显示「纠错」小按钮，点击弹出编辑框改文本，确认后调 wordbook.learn_correction 入库（喂 hotwords）。背景：WM_GETTEXT 读回在现代应用无效（RESEARCH-TEXTCAPTURE-001），自动学习路径 A 实际失效，需自建纠错闭环。涉及：src/main.rs（overlay 生命周期）+ Win32 overlay 绘制 + wordbook API（已有）。注意：DEC-029 词库已单词化，此任务未来改为学单词 | 待 Gavin 排期 |
| UI-I18N-COMPLETE-001 | UI 硬编码文字补全 i18n（coder-2 2026-07-07 建议）：App.tsx Loading/Error、Llm.tsx Success/Failed 等 5 处状态文字补 key + 替换；热键显示名（VK_TO_LABEL 的 Ctrl/F9）属通用键名评估后大概率不译 | 待 Gavin 排期（归入"小优化"批次） |
| LLM-KEY-REVEAL-001 | 格式化输出页 API Key 明文查看（显示/隐藏小按钮）：现为 password 掩码，Gavin 端测反馈框太小看不清已由 FORMAT-UI-POLISH-001 解决，明文查看为当时保留的备选追加项 | 待 Gavin 排期（归入"小优化"批次，2026-07-14 提出） |
| TELEGRAM-RESTART-001 | Telegram 通道恢复（troubleshooting [TELEGRAM-CHANNEL-001]）：2026-07-07 实测重启无效，根因为 Claude Code 2.1.202 服务端功能开关（tengu_harbor）拦截，本地无解；候选路线：降级 CLI / 等官方放开 / 临时手动轮询 | 待 Gavin 拍板路线 |
| MAC-P2-001 | macOS 热键 dispatch 延迟优化（P2） | Windows P2 完成后评估 |
| RESEARCH-TEXTCAPTURE-001 | 现代应用文本捕获方案研究 | WM_GETTEXT 在现代应用无效 |
| DEC-014 | WebView2 自动安装 | Win10 用户 |
| CRASH-EMAIL-001 | crash reporter 邮箱设置 | 需 SMTP 配置 |
| QWEN3-CORPUS-BIAS-001 | qwen3 在线模型接入词库偏置：官方 input_audio_transcription.corpus.text（max 10K tokens）可作 hotwords 等价通道；注意两份官方文档记载矛盾，实施前需实测验证。注意：DEC-029 词库已单词化，corpus 直接用单词列表 | 待 Gavin 排期（研究报告 collab/research/qwen3-protocol-alignment-001.md）|
| QWEN3-STREAM-V2 | qwen3 真流式边录边上屏（DEC-028 v1 为整段上传，流式留作演进） | 待 Gavin 排期 |

---

## Phase 3 · 演进评估（不排期，仅记录）

场景感知后续演进方向：UIA 控件信号、浏览器细分词表迭代、内容压缩独立开关、语气适配 LLM 兜底开关（未命中时参考应用名，默认关）、个性化风格学习。

来源：DESIGN-FORMAT-SCENE-001 技术方案（collab/research/typeless-format-design-001.md）+ DEC-031。


---

## 📋 待排期 · I18N-HANT-GAP-001 繁体中文缺 `voice_asr_model_accuracy` 一个 key（2026-08-17 主控验收 HOTKEY-047 时机器发现）

**发现方式**：验收 HOTKEY-047 时主控用 `comm` 比对三份 locale 的 key 集合（不是采信 Worker 自证）。

| 文件 | key 数（`^  [a-z_0-9]*:` 计） |
| --- | --- |
| `ui/src/i18n/en.ts` | 111 |
| `ui/src/i18n/zh-Hans.ts` | 111 |
| `ui/src/i18n/zh-Hant.ts` | **110** ← 缺 `voice_asr_model_accuracy` |

**已确认是历史遗留**：`git show HEAD:ui/src/i18n/zh-Hant.ts` 计得 110，`en.ts` 计得 111
→ **不是 HOTKEY-047 引入**，coder-1 无责。

**影响**：繁体中文用户在语音设置页看到 ASR 模型「准确度」相关文案时会拿到 `undefined`
或回退（取决于 `getTranslations` 的兜底策略，**未核，实施前需先确认**）。

**修法**：给 `zh-Hant.ts` 补该 key 的繁体译文。改动极小，但**必须顺带核一件事** ——
`ui/src/i18n/index.ts` 的 `getTranslations` 对缺失 key 是否有兜底；
若无兜底则属于「繁中用户看到 undefined」的用户可见缺陷，优先级要上调。

**顺带的方法教训**（已记 `[WORKER-WRITE-SILENT-FAIL-001]` 同族）：
coder-1 自证里报「三份 locale = 111:111:111 一致」，等式不成立 ——
**报数字前要真的量一次**。主控验收一律独立复算，不采信自证里的数字。

---

## 🛑 2026-08-17 停派指令（Gavin）—— 恢复时从这里开始

> **Gavin 原话**：「你的额度快尽了，先别派发任务，等我指令再派」

| 项 | 状态 |
| --- | --- |
| HEAD | `2e70875`，**已 push**（`56bfa37..2e70875`，10 个提交），token 无残留 |
| 工作区 | clean（仅 `collab/todo.md.bak-20260817` 未跟踪备份，可留） |
| BUILD-019 产物 | `Publish/` 08-17 16:18，三 exe sha 相对 BUILD-018 全变，⏭ **待 Gavin 端测** |
| coder-1 / coder-2 / tester-1 | **全部空闲待命，手上零任务** |
| 🔴 恢复后第一个动作 | 派 **HOTKEY-049**（Gavin 已定顺序「先 049」），方案与判据已固化在上方专节，**可直接派，无需再问** |
| 其后 | 阶段三 TEST-SYNC-049 → 阶段四 → BUILD-020 → push（push 授权已给，出包后执行） |
| 再其后 | `E2E-HARNESS-050`（修两道 harness 错位，裁决已定：改 harness 不改生产、尺寸与 `main.rs` 常量同源） |

### 恢复时不必重新调研的（已定论，直接用）

- HOTKEY-049 判据、键集合推导、7 条实例对照表 → 见上方 049 专节
- 049 的 UI 红线：**不得复用带「仍然使用」的现有冲突弹窗**，须另做只有「重新设置」出口的提示
- 049 双向生效：设翻译键查语音键、设语音键查翻译键，两侧都要做
- 049 范围外：后端不做强制（手改 config.toml 不受保护），已如实记录
- E2E 门禁结论：产品正常，两道 harness 错位自初始提交即存在 → `[E2E-CONFIG-PATH-STALE-001]`

### Gavin 端测五项（BUILD-019）

1. 按热键录音窗口出不出来（本包核心）
2. 点一次「设置热键」就能录上（不用点两次）
3. 左 Alt / 左 Ctrl / 左 Shift 可单独设，且按下即回显键名
4. 按住右 Alt 再按 M → 应得 `Alt+M` 而非 `Ctrl+Alt+M`
5. 🔴 **只有端测能验**：真按左 Ctrl+左 Alt+字母 → 应得 `Ctrl+Alt+字母`，不得被吞成 `Alt+字母`
   （happy-dom 把 `AltGraph` 与 `alt` 同映射，单测证明不了，见
   `[HAPPYDOM-ALTGR-INDISTINGUISHABLE-001]`）

---

## 🟡 2026-08-18 上午 · OVERLAY-051-G-FIN 已交付待验收 —— coder-1 完成

| 项 | 值 |
| --- | --- |
| HEAD | `12f0915`；工作区**非 clean**（051-G 在途，`main.rs` +270 / `qwen_inference.rs` +45） |
| Worker | coder-1 **已交付**（051-G 收尾完成，待主控验收）／ coder-2 **待命**（054-B/C/D/E 全在 `main.rs`，文件冲突，本轮不派）／ tester-1 **待命**（等阶段三 TEST-SYNC） |
| 主控实测 | `cargo check --all-targets` ✅ 0 error（coder-1 修完编译 + 方向纠正完成） |
| 方向纠偏 | ✅ 已纠正：删 `compute_tween_advance` + 5 常量 + 9 测试，新增 `reveal_chars_by_timeline`（时间戳驱动，不压缩停顿，words 空退回立即显示） |
| 决策补记 | `decisions.md` **DEC-054**（时间戳驱动回放 + 三个坑处理口径） |
| 未 push | 本地 ahead **3**（`0049c33` / `8fd9767` / `12f0915`），Gavin 未授权，不得自动 push |

**下一棒**：coder-1 交付 → 主控验收（Read + `cargo check`）→ 阶段三 TEST-SYNC → 阶段四回归 →
E2E 门禁 → 出诊断包 → **Gavin 跑一次 `-debug` 看 `words=N`**（回放参数校准的唯一数据源）。

---

### 进展更新（当日滚动）

| 时间 | 事件 |
| --- | --- |
| 11:14 | `OVERLAY-051-G-FIN` 派发 coder-1（含方向纠偏：删固定速率打字机，改时间戳驱动） |
| ~12:0x | **Gavin 端测再报**：录音窗口先闪屏幕左上角再跳回底部。主控 Read 取证 → `OVERLAY-054-B-FIX`，根因见 troubleshooting `[OVERLAY-POS-HARDCODED-ZERO-001]`，**追加进 coder-1 同批**（同文件串行，零冲突） |
| ~12:1x | coder-1 交付 051-G：删 `compute_tween_advance`+5 常量+9 测试，新增 `reveal_chars_by_timeline`，words 累积下沉 `StreamingAsrState`，+264/-58 两文件 |
| 同上 | 主控 Read 验收查出**尾部字符不显示**缺口（词表覆盖不到 `total_chars` 时无路径推进，端测表现＝最后一两个字迟迟不上屏），已反馈并入本批：循环无 break ⇒ `return total_chars` |

**OVERLAY-054-B 根因一句话**：`show_overlay_streaming_idle`（`main.rs:3215`）绕过 `overlay_geometry`
硬写 `pos:[0,0]`，而 OVERLAY-046 恢复了无条件 `SetWindowPos` → 硬编码从"无害"变"真生效"。
治本修法：`OverlayRequest.pos` 改 `Option<[i32;2]>`，Show 端 `None` 兜底算几何，
验收判据加**全文件 `grep "pos: [0, 0]"` 零命中**（上一轮漏修一半就是因为没这条判据）。
附带查出 `:3693` FocusLost 提示框同样错位（Gavin 还没撞上），同批修。

---

## 🛑 2026-08-18 01:4x 收工交接 —— 明天从这里开始

### 状态快照

| 项 | 值 |
| --- | --- |
| HEAD | `8fd9767`（E2E-HARNESS-050） |
| 已 push | 到 `2e70875`；之后 **未 push 2 个**（`0049c33` / `8fd9767`）—— Gavin 未授权，勿自动 push |
| 🔴 工作区**非 clean** | coder-1 的 **051-G 在途改动未提交**：`src/main.rs` +270 / `src/transcription/qwen_inference.rs` +45 |
| 在途备份 | `scratchpad/051G-wip-0818-0141.patch`（22KB，已存，丢不了） |
| coder-1 / coder-2 | ⏸ **token 额度耗尽，Gavin 已令暂停派发**，等重置 |
| tester-1 | ✅ 空闲，可用（另一额度池） |
| 产物 | `Publish/` 仍是 **BUILD-020**（08-17 19:37），**不含**今天 054-A/055/053-C/D/E2E 的任何改动 |

### 🔴 明天第一件事：确认 coder-1 的 051-G 在途代码状态

它被额度打断在半途。**先 `cargo check --all-targets` 确认能否编译**
（今天两条 `test_platform_cargo_test_*` E2E 用例失败就是因为它编译不过）。
- 能编译 → 让 coder-1 继续收尾
- 不能编译 → 让 coder-1 先修复编译再继续，**不要 revert**（备份在，但优先让它自己收尾）

---

## 卡顿问题：调查已完结，方案已定，等一次验证

### 已排除（全部有实测/文档依据，**不要重查**）

| 假设 | 结论 |
| --- | --- |
| 建连慢 | ❌ DNS 10ms + TCP 0ms + TLS 91ms = **103ms** |
| 首字慢 | ❌ 开口→首字 **338ms**，「反应快」已达标 |
| 我们攒着音频不发 | ❌ 实时逐帧发，100ms/帧，**与官方推荐完全一致**，无节流 |
| debug 日志 IO 拖累 | ❌ 正式模式同样卡；且正式模式 `LevelFilter::Warn` 无文件目标，零磁盘 IO |
| 本地 VAD 门控导致 | ❌ 仅入口门控，建连后**不再过滤**（主循环 VAD 调用数 = 0） |
| API 有参数可调推送频率 | ❌ **官方文档确认无此参数**（全部 9 个参数均不控制此项） |
| Manual 模式高频 commit | ❌ 它控的是**断句**不是中间结果频率；每 300ms 断句会**丢上下文、毁准确率** |
| `words[]` 带来更高频推送 | 🟡 大概率不成立（words 是同一条消息内对同一 text 的分解），**待一次 -debug 实证** |

### 确认的根因

**服务端每约 1000ms 推一批、每批 3-4 字**（实测：786/1036/1105/1246ms 间隔）。
这是它的固有行为，客户端调不动。

### 已定方案（Gavin 亲自拍板，主控曾三次判断错误后被纠正）

**时间戳驱动回放** —— 用 `words[].begin_time` 按用户**真实语速**回放。

🔴 **Gavin 的三条明确指示（不得再动摇）**：
1. 「**时间戳驱动才是应该选方案**」→ 不用固定速率抖动缓冲
2. 「**停顿和用户讲话的节奏一致才对**」→ **不得压缩停顿**，删掉主控原提的间隔上限
3. 几百毫秒固定延迟**可以接受**

`words` 不可用时 → **退回现在的立即显示**，不要换另一套平滑算法。

### 🔴 实施必须注意的三个坑（主控今天查出，写进任务书了）

1. **时间戳基准**：我们是攒 1.5s pre-roll 才一次性补发的，
   `begin_time` 相对「发出去的音频流起点」，**不是**「按下热键的时刻」。搞混会整体偏移 1.5 秒
2. **断句会重置**：服务端 `max_sentence_silence = 800ms`，
   **停顿超 0.8 秒即断句、开新 `sentence_id`** → 回放游标必须跟着重置
3. 官方文档新增可用信息：`words[]` 还有 **`end_time`**（每词该显示多久）与
   **`punctuation`**（标点归属），都该用上

### 待验证（一次 -debug 即可，不需额外开发）

coder-1 已把 `words={}` 加进日志。**新包出来后 Gavin 跑一次 `-debug` 说两三句话**：
- 空 text 消息的 `words=N` 是 0 还是有值
- 每批实际几个词、时间戳跨度多大 → **这是回放偏移量取值的依据**

---

## 今天完成并已提交

| 提交 | 内容 |
| --- | --- |
| `0049c33` | OVERLAY-054-A 焦点恢复（RestoreAndHide + SetForegroundWindow + 200ms 轮询确认）／ ASR-055 测试连接重写（原传空串 → Inference 协议，model 入 payload）／ WORDBOOK-053-C 候选词七条校验 ／ 053-D 污染排查 |
| `8fd9767` | **E2E-HARNESS-050**：9 FAIL → **2 FAIL / 61 PASS**；配置路径单一来源；state_detector **正则解析 main.rs 常量**（零硬编码，今后改尺寸免疫）；新发现 `[E2E-COLD-START-RACE-001]` |

### 🔴 053-D 结果待 Gavin 授权（主控与 Worker 均未删任何数据）

`db_path()` = exe 同级 `wordbook.sqlite`
- `wordbook` 表 18 条 **全部正常**（污染未进正式词库）
- `candidates` 表 164 条中 **12 条脏数据**（多行编号列表，24–65 字符）

**清不清由 Gavin 定。**

---

## 待办队列（按序）

| 序 | 项 | 归属 | 前置 |
| --- | --- | --- | --- |
| 1 | **051-G 收尾**（时间戳回放 + 三个坑） | coder-1 | 额度 |
| 2 | 出诊断包 → Gavin `-debug` 一次 → 定回放参数 | tester-1 + Gavin | 1 |
| 3 | **054-B 打回**：`:3117` `RecordingStreamingIdle` 仍传 `pos:[0,0]`，**在线路径仍闪左上角**（coder-2 上轮只改了本地模型路径） | coder-2 | 额度 |
| ~~4~~ | ~~054-C/D/E~~ ✅ 已完成并提交（`9c0b1b1`） | coder-2 | — |
| 5 | ASR-055 端测（需有效 api_key，Worker 无法真连） | Gavin | 出包 |
| 6 | `ASR-PERF-052` `multi_threshold_mode_enabled` 实验 | 待定 | 低优先 |

### 🔴 主控给自己加的新硬约束

**出包前 E2E 门禁必须通过，不过不出包。** 不再以「读代码觉得没问题」代替实际运行验证。
—— 起因：Gavin 批评「几轮拿不到主流程走通的版本」「验收潦草」，
根因是主控验收上限止于 `cargo check` + 读代码，从不真正跑程序。

### 其他挂账

- **api_key 明文存 `config.toml`**（`Publish/` 与 `target/release/` 均有），Gavin 未决定是否处理
- `main.rs:4723`、`hotkey.rs:300` 等处中文注释**乱码**（`[ENCODING-UTF8-001]` 历史遗留，范围不止一个文件）
- `src/main - 副本.rs` 99KB 未入 git 的旧副本残留，**删除需 Gavin 单独确认**

### 🔍 REPRO-073 取证新疑点（tester-1 2026-08-30 夜，待主控定性/派发）

1. **ASR-SUMMARY outcome 判定脱节**：所有成功识别 run 均标 `outcome=failed`（words_total>0 的 run 也是）——Gavin 端测 A/B 若引用该字段会被误导。查 `AsrSummary::format_summary()`（qwen_inference.rs:209-218）outcome 赋值链
2. **vad_hit_ms=-1 从未命中**（8 run）：服务端 VAD 回执疑似从未到达客户端，客户端无 end-of-speech 信号源——与 067/070 同根
3. **stop 后迟发窗口**：`OVERLAY-043: ignoring late StreamingText after stop` 单 run 6+ 次、持续 4-5s——REPRO-068 交替闪烁与 070 尾部丢失疑似同源 finalize 拖尾

---

## 🟢 2026-09-06 晚 · SECRET-126 已验收（Gavin 指令：确保配置与隐私永不入库）

**Gavin 原话**：「确保本地配置文件和代码不会提交任何 key、和其他隐私敏感信息」。

### 主控审计现状（先说结论：当前无正在泄露的东西）

| 审计项 | 实测 |
| --- | --- |
| 磁盘上真正含非空 `api_key` 的文件 | 仅 `Publish/config.toml` 与 `target/release/config.toml`，**两者已 ignore** ✅ |
| 未跟踪且未被忽略（`git add -A` 会抓的） | **0 个** ✅ |
| 326 个已跟踪文件全量内容扫描 | 命中 8 处**全为良性**：上游 vendor 作者邮箱（cmake/esaxx-rs）、`@example.com` 占位、Gutenberg 语料公开地址、Tauri 图标清单里的 `128x128 @ 2x.png` 文件名（@ 前后加空格以免自触发闸门）被邮箱正则误命中。**零真实凭证** ✅ |
| `assets/default-config.toml` | `api_key = ""` 空占位 ✅ |
| 两道钩子 | 实机验过：pre-commit 真阳性 5 拦/真阴性 3 放；pre-push 能兜住被 `SECRET_SCAN_SKIP=1` 绕过的 commit ✅ |

### 补的 latent 洞（SECRET-126，coder-1）

`.gitignore` 四类路径规则 + **新增 `scripts/git-hooks/secret-paths.sh` 路径闸门**
（pre-commit + pre-push 双侧 source 同一份，防 SECRET-082 式漂移）。
拦截判据是**路径不是内容** —— `[GATE-MATCHES-SHAPE-NOT-CONTENT-001]` 的教训：
口述转写日志、实验 dump 这类「文件类别本身危险」的东西是内容正则的永久盲区。

**主控独立复跑（未采信报告）**：路径闸门 **17/17** 全对（含 `CONFIG.TOML` 大小写绕过、
`collab/research/` 强加 `-f`、`logs/*.md` 与 `.cargo/config.toml` 未误伤）；
内容闸门 **7/7** 零回归；`ls-files` 326 未变；三钩子 LF+UTF-8+可执行。

### 遗留

| 项 | 处置 |
| --- | --- |
| coder-1 的 result.md 缺「收尾自证表」 | 已退回补写（`[DOC-STATE-DRIFT-001]` 又复现，且这次漏的正是防它的那张表） |
| MSYS git `add -f <ignored>` 偶发静默 no-op（coder-1 报） | 主控立 troubleshooting 条目 |
