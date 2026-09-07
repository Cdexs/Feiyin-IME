# DIAG-163 · BUILD-159 端测三问诊断报告

- **Worker**: coder-1　**日期**: 2026-09-07　**性质**: 🔴 只查不修，`src/` 零改动
- **红线自证**: 本任务对 `src/` 零字节改动。当前工作区 `git diff --numstat -- src/` 显示 `7 8 src/main.rs`，**全部归属 coder-2 的 FIX-162 并行改动**（hunk 仅 `:103 import 行` + `:758 create_edit_control`，与任务书预告一致）。本报告所有结论用符号名锚定（行号会因并行改动漂移）。
- **取证手段**: 只读 git（`show`/`log -S`/`diff`）+ 工作区静态阅读 + WebSearch（MIIM_BITMAP 已知坑核对）。**runtime 证据（BUILD-159 端测的 debug.log）尚未产生**——`Publish/debug.log` mtime 15:57 早于 18:14 出包，与主控待办 ③ 状态一致。凡未定处均如实标注。

---

## Q3 · 切本地→用一次→切回在线→按热键，出的是本地频谱录音窗（🔴 机制已定，代码级）

### 结论（一句话）

**不是三个嫌疑提交带出来的，是一直存在的 DEC-025 异步热重载设计缺口，被 Gavin 这套操作序列首次触发**：模型切换后的**第一次录音永远用旧 transcriber 实例**（重建异步、不等），而 worker 侧判据是「config **且** 实例」双条件 → 该窗口期内走了本地管线 → 无条件发出的 `RecordingStarted` 把控制器先画的流式占位窗**覆盖**成本地频谱窗。

### 代码级证据链（五步，全部符号锚定）

1. **控制器热键路径只看 config**（`show_overlay_streaming_idle` 的调用方，HOTKEY-LATENCY-FIX-001 区块）：
   `is_streaming_asr = AsrModel::from_config(&cfg.audio.asr_model).is_online_streaming()`——切回在线后 config 已是新值 ⇒ 热键按下**先正确显示** `RecordingStreamingIdle` 占位窗。

2. **worker Start 处理器内、先无条件发事件**：`send_event(&event_tx, PipelineEvent::RecordingStarted)` 在 Start 处理的**最顶部**（6727 附近），早于任何流式/本地分支判定。控制器收到后 `PipelineEvent::RecordingStarted => show_overlay(…, OverlayStatus::Recording)`（6192 附近）→ **频谱窗覆盖占位窗**。该行自初始提交 `680d78f` 就存在（`git log -S` 取证）。

3. **worker 判据是双条件**（ASR-056 收敛判据，6807 附近 `[Latency] worker received Start command` 之后）：
   ```rust
   let is_streaming_asr = desired_asr_model_check.is_online_streaming()
       && transcriber.as_ref().is_some_and(|t| t.asr_model().is_online_streaming());
   ```
   **config 与 transcriber 实例不一致的窗口期内为 false** → 走 `record()` 本地管线。

4. **transcriber 热重载是异步 spawn、不等**（DEC-025，81304f7 v0.6.1 引入）：Start 处理器内部检查 `needs_reload` 后只 `asr_reload_in_flight = true` + `std::thread::spawn(Transcriber::new…)`，注释原文「重建异步进行，期间继续用旧实例不阻塞录音」。即：**切模型后的第一次 Start 必然用旧实例**——这是设计使然，非偶发竞态。

5. **双条件判据的引入历史**（`git log -S`）：
   - 双条件 `is_streaming_asr`：`4f3b41b`（ASR-038-B 真流式管线）→ `9c1806d`（OVERLAY-051）细化
   - 重载触发：`81304f7`（v0.6.1 双模型架构）
   - 三个嫌疑提交 hunk 范围核对（`git show <c> -- src/main.rs | grep '^@@'`）：
     - `aeaebe1` OVERLAY-155：仅 `:1122`（半径常量）+ `:3004`（draw_recording_overlay chrome）——**未触碰** show_placeholder 判定与任何状态选择
     - `62800bc` EDIT-FLICKER-157：仅 `:102`（import）+ `:678`（create_edit_control）——够不着
     - `85388ac` TRAY+MIC：hunk 全部落在 tray 菜单（282-396）、重绘门（1729/1784）、GDI/D2D 绘制（2773-4360）——**未触碰** worker Start 区域（6600-6900）

### 「用过本地模型才复现」的解释

切到本地后用一次 → 这次使用期间重载完成、本地实例转正（`active_asr_model` 更新）。再切回在线 → transcriber 又落后 config 一拍 → 第一次 Start 复现。「本地实例已转正」是该时序的必要条件，与残留学说自洽。

### ⚠️ 表现范围（重要）

- **只错窗口外观，不错输出**：本地 record() 照常录音、照常转写（旧实例是本地模型，功能正常）。
- **自愈性**：第一次 Start 已触发重载；重载完成后（秒级）第二次按热键 → 双条件成立 → 正常占位窗。即**每次切模型后仅第一次录音受影响**。
- 🔴 若 Gavin 复测发现**第二次、第三次仍出频谱窗**，本机制被证伪——那意味着重载失败循环（debug.log 会有 `ASR transcriber hot-reload failed`）或其它残留学，需再查。

### runtime 复核签名（debug.log，一条命令定案）

BUILD-159 端测日志里找这三行的**时序**：
```
Triggering ASR transcriber hot-reload: model Performance|Accuracy -> QwenAudioOnline|FunAsrRealtime
[Latency] worker received Start command          ← 必须在上一行之后、重建完成之前
[Latency] ensure_stream completed                ← record()（本地路径）的签名；流式路径是 "record_streaming ensure_stream"
```
且该次会话中 `ASR transcriber hot-reload completed` 出现在 `worker received Start` **之后** = 本机制实锤。E4 探针 Paint 状态名（status 1 Recording vs 2 RecordingStreamingIdle）可作为窗口层旁证。

### 修法建议（标注：不实施，供主控决策）

- **建议 A（最小）**：worker 在 Start 时若 `needs_reload && !in_flight`，同步等待本次重建完成再走分支（仅 Start 路径等，不动 idle 期异步性）——消除「第一次用旧实例」，代价是切模型后首次按热键多等一个重建时长。
- **建议 B（UI 侧缓解）**：控制器侧对「config 在线但 worker 实际走本地」时补发状态修正事件。复杂度高，不建议。
- **建议 C（现状声明）**：接受该行为并在文档标注「切模型后首次录音沿用旧引擎」。若采纳，本单可关。
- 三者都需 Gavin/主控拍板，本单不动手。

---

## Q1 · 托盘菜单图标为什么没挂上（🔴 头号怀疑已证伪；静态全链路无缺陷；机制未定，需一次插桩定位）

### 主控头号怀疑的证伪

「`:386` 的 HMENU 不是被弹出的那个」——**证伪**。调用链：
`show_tray_popup_menu`（:357）→ `CreatePopupMenu()`（:367）自建菜单 → `AppendMenuW` ×3（设置 1001 / 分隔线 / 退出 1002）→ `attach_menu_icons(menu)`（:386）→ `TrackPopupMenu(menu, …)`（:388）→ `DestroyMenu(menu)`（:398）。
**挂图标、弹出、销毁全程同一个 HMENU**。tray-icon crate 侧：`build_tray`（:272）只有 `with_tooltip`/`with_icon`，**无 `with_menu`**——crate 自建菜单不存在，右键菜单 100% 是我们自建的这个。`tray-icon = "0.19"`。

### 其余候选逐条结论（按任务书顺序，全部给出判定）

| 候选 | 结论 | 证据 |
| --- | --- | --- |
| wID 与 MENU_CMD_* 匹配 | ✅ 匹配 | `MENU_CMD_SETTINGS=1001`、`MENU_CMD_EXIT=1002`（:255/:257），AppendMenuW 同值；非 0、合法 |
| MIIM_BITMAP 是否需配 MIIM_STATE 等 | ✅ 无需 | fMask=MIIM_BITMAP 单独合法；微软官方「Using Menu-Item Bitmaps」示例即此模式；SO 78577359 社区确认 32bpp 预乘 DIB + MIIM_BITMAP 可用 |
| SM_CXSMICON 尺寸 | ✅ 正常 | 取 min(CX,CY) clamp(16,64)；尺寸只影响显示大小，不影响有无 |
| 32bpp top-down 预乘 BGRA 符号 | ✅ 正确 | `biHeight: -size`（负 = top-down，:注释明示）；预乘公式 BGRA 全对 |
| DeleteObject 时机 | ✅ 正确 | `menu_bitmaps` 在 `DestroyMenu(menu)` **之后**才逐个删除；TrackPopupMenu 模态期间句柄存活 |
| SetMenuItemInfoW 返回值 | ❓ **无法静态判定** | 代码 `is_ok()` 成功才推入 attached，失败则删位图——**但两条路都无日志**（静默降级），失败与否运行时不可见 |

### 结论

**代码全链路（RGBA 光栅化 → DIB → SetMenuItemInfoW → TrackPopupMenu）静态审查零缺陷**；`rasterize` 返回恰好 `size*size*4` 字节，长度契约通过；12 张预览 PNG 证明图形层正确。**「没生效」的根因藏在唯一无观测的两点之一**：
1. `SetMenuItemInfoW` 运行时失败（原因未知，静默降级吞掉）；
2. `CreateDIBSection` 运行时失败（同样静默）。
**机制未定**——静默降级是本单查不到直接原因的直接原因（任务书已预判）。按 `[PLAUSIBLE-FIX-NOT-ACTUAL-CAUSE-001]` 纪律，本报告不把任何单一假设写成结论。

### 修法建议（标注：不实施）

给 `attach_menu_icons`/`create_menu_item_bitmap` 加**临时插桩**（log::info：size、CreateDIBSection 成败、SetMenuItemInfoW 返回值、attached 数量），出一次性诊断包，右键一次即定案。这不是修法是探针，与 E3/E4 同范式；定位后探针可留可删。

---

## Q2 · 流式窗麦克风声波弧为什么没画出来（🔴 链路代码级已查通；发现唯一静态自洽解释 + 一处待 runtime 定夺的矛盾）

### Gavin 截图窗口走哪条路径

流式占位窗（`RecordingStreamingIdle`）WM_PAINT → `draw_recording_overlay(show_placeholder=true)` → **D2D 在先**：`d2d::draw_streaming_idle_overlay`（:3243）→ 内部调 `mic_indicator`（D2D，:4774 调用点）→ 弧代码在内。若 D2D 失败 → GDI 兜底 `draw_recording_indicator`（GDI 流式分支）→ 弧代码同样在内。**三个 D2D 变体（text :4667 / idle :4774 / waveform :4796）+ 两个 GDI 分支全部含弧代码**——交付时「三个产出源全改」的声明与代码实况一致。

### 弧的绘制链路逐环判定

| 环节 | 结论 | 证据（符号锚定） |
| --- | --- | --- |
| 电平来源 | ✅ 流式路径**有喂** | worker `record_streaming(..., Some(Arc::clone(&audio_buf)), …)`；`audio/mod.rs` 逐 chunk `if let Some(ref buf) = level_buf` 写入（:359，RecordingState::push :923）。与本地 record() 同一链路 |
| 同一个 buf 吗 | ✅ 同一 Arc | `:7096` 创建 → `:7107` 交 worker → `:7125` 交 overlay 线程，三方共享 |
| 重绘门 | ✅ 开启 | `RecordingStreamingIdle`/`RecordingWithText` 两分支 `dirty = needs_repaint \|\| mic_has_audio(&state)`（MIC-PULSE-160 加的）；overlay 循环活跃态 ~25fps（OVERLAY-WAKE-001） |
| 绘制判据 | ✅ 有 | `if has_audio { gain = (mic_level / MIC_PULSE_FULL_LEVEL).clamp(0,1); if gain > 0 { mic_pulse_arcs(...) } }`，D2D/GDI 三处同构 |
| 墙钟相位 | ✅ 正常 | `mic_pulse_tri(p, offset)`：0→1→0 三角波宽 0.55、周期 1000ms、外弧滞后 0.3、乘 gain |

### 🔴 代码级矛盾（这是 Q2 的核心发现）

`mic_audio_snapshot` 在**同一次加锁内**同时产出麦克风颜色与弧的 gain：
- 麦克风**橙色** ⇔ `has_audio`（任一 level > **0.01**）
- 弧的 alpha ⇔ `gain = max_level / 0.35`，再乘 0→1→0 三角波

**Gavin 截图里麦克风是橙色 = 截帧那一帧 `has_audio=true` = 同帧 `gain>0` = 弧被绘制了**（除非 alpha 低到不可见）。所以「代码没执行到」不成立——弧**画了，但可能看不见**。

### 唯一静态自洽解释（🔴 标注：假设，机制未定，待 runtime 复核）

**非对称阈值冻结帧**：色彩阈值 0.01 vs 弧满亮 0.35 差 35 倍。说话间隙电平衰减落入 0.01~0.04 区间时，最后一帧 = 橙色麦克风 + gain≈3%~11% 的弧（视觉不可见）；随后 `mic_has_audio=false` → 重绘门关闭 → **画面永远冻结在这帧**。静默间隙截图即「橙色麦克风、周围无弧」。
**该假设的证伪条件**：Gavin 若在**正在说话时**观察仍无弧 → 假设被证伪（说话时 level 高、gain 大、25fps 重绘中，弧应明显）→ 需要新增弧级插桩（log mic_level/gain/alphas，或 E5 探针）再查。

### runtime 复核手段（二选一）

1. **零成本**：请 Gavin 复测时**持续说话**盯住流式窗 3 秒，回话「说话中有没有弧」——一次观测定案。
2. **插桩**：弧绘制处加 debug 日志（level/gain/alpha），诊断包跑一次。

---

## 总结

| 问题 | 定性 | 置信度 |
| --- | --- | --- |
| Q3 切模型出错窗 | **机制已定**：DEC-025 异步重载设计缺口（切模型后首次录音沿用旧 transcriber），非本批回归；debug.log 时序可一测定案 | 高（代码级五步证据链闭环；待日志复核实锤） |
| Q1 菜单图标 | **机制未定**：头号 HMENU 怀疑证伪；静态零缺陷；根因在静默降级黑箱内（SetMenuItemInfoW/CreateDIBSection 运行时成败不可见），需一次插桩 | 中（排除法完成，无法再靠静态收敛） |
| Q2 声波弧 | **链路已查通**：电平喂入/重绘门/绘制判据全部接通；截图橙色 ⇔ 弧已画但可能不可见；「阈值冻结帧」为唯一静态自洽假设，待 Gavin 说话中观察定案 | 中高 |

**给主控的三个决策点**：① Q3 修法 A/C 二选一（或先拿 debug.log 实锤再定）；② Q1 是否批一个插桩诊断包；③ Q2 是否用「说话中观察」零成本复核，不行再插桩。
