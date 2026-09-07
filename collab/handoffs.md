# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。


## 2026-09-07 — coder-2 — OVERLAY-141-IMPL ✅ 圆角灰边根治：帧末解析式 SDF 写 alpha + 外框半径单一来源（待主控验收）

- **改动 A（半径单一来源，先做）**：常量 `OVERLAY_FRAME_RADIUS_LG=16.0`/`SM=10.0`（`main.rs:1081/1083`）+ 映射 `overlay_frame_radius(status)`（`:2216`）。**16 处外框站点全收敛**：D2D 8 处（processing_primitives/editing/waveform/idle/streaming_text/error/info/preview）+ GDI 调用点 5 处（`OVERLAY_FRAME_RADIUS_* as i32`）+ GDI 本地 `const CORNER_RADIUS` 4 处（Processing/Preview/Error/Info）+ **死代码独立 hunk** `draw_editing_overlay_chrome`（零调用实证，Gavin 拍板删则整函数带走零残留）。`draw_submit_button:2977` r10 = 提交按钮内部元素，主控裁定**故意不动**。
- **改动 B（SDF alpha）**：`apply_alpha_fixup` 重写（`:2243`，签名 `(bits,width,height,opacity,radius)`）：每像素圆角矩形 SDF `cov=clamp(0.5-d)`；cov==0 四通道归零；否则反预乘还原原色后按 `cov·op` 预乘回写。顺带消除「形状内纯黑像素变透明洞」。落地前自查：`chrome_with`/`draw_processing_primitives` 填充均 `(0,0,w,h)`，SDF 与填充逐位同框 ✅。
- **改动 C（调用点）**：WM_PAINT 锁内与 opacity 同处取 `frame_radius`（`:2136`，request None 兜底 LG），`:2153` 传入；**G3 结构保持**（fixup 与 ULW 提交同 `if use_ulw` 块）。
- **验证**：fmt 过；`check --all-targets` 0 error，warnings **111/102 基线逐位持平**；`cargo test` **1104P/1F/9I**，唯一失败 G6 = **预期红**（旧三分支标记消失，交 TEST-SYNC-143 换血，本单未动护栏）；G2/G3/G5/G7/H7 全绿。
- **自证**：外框半径字面量 grep 0 残留（内部元素清单见 logs/20260907.md，含 `:2977` 有意留存）；SLWA 运行路径零代码改动（StreamingEditing 相关 diff 仅新映射分支与注释 + 同值替换）；macOS 全部 cfg(windows) 内不适用，MACOS-HANDOFF 已同步。
- **红线**：只动 `src/main.rs` 生产区（+187/-44）；未 commit / v0.9.0 未动 / 未出包；仅白名单命令；零凭证；无临时文件。


## 2026-09-07 — coder-1 — INVESTIGATE-142 ✅ 编辑态移光标仍闪二次取证（🔴 只查不修，src 零改动，待主控验收）

- **结论**：编辑稳态应用层零重绘触发器——主控怀疑「图层模式切换」（switch 早退 `:599`+调用点恰 2+编辑期模式恒 Slwa）与旧根因「迟到流式包」（Show 唯一编辑分支 `:5686` 被 FLICKER-130 门控挡死，`editing⇒stopped` 不变量 F3 护栏钉死）**双静态证伪**。**根因未定**：头号在案候选=EDIT 子控件自身重绘（caret 局部/ES_AUTOHSCROLL 滚动整块）+DWM 对 SLWA 分层窗重合成——发生性静态实证（箭头键不经过任何应用逻辑：子类只拦 Enter/NCPAINT、父窗无 WM_COMMAND 臂、per-frame 编辑臂恒零 InvalidateRect），可见性需实测。
- **新发现结构性放大器**：SLWA 父窗 WM_PAINT 全窗 BitBlt 直打屏幕（`:2172`）+父窗无 WS_CLIPCHILDREN（`:1172`/`:1222`）——父窗一旦重绘，EDIT 文字被整块盖掉且不知情，等 caret 闪烁周期才回来。应用级触发器静态为零，系统级触发器=实测 P2 定性。
- **FLICKER-130 定性**：多源叠加，没有判错——修掉的 Show 风暴源真实有效；光标闪来自一直存在、先前被掩盖的另一源（新旧症状触发器不同佐证）。
- **产出**：`collab/drafts/overlay-142-editing-flicker.md`（六候选三态判定表 / M0-M3 修法建议不实施 / 五探针最小实测方案 P1-P5 + 240fps 录屏，判读方式预先声明防事后任意解释 / WebSearch：SLWA+子控件闪烁无文档定论）。
- **红线**：src/** 零写入（diff 中 M 属 coder-2 在途 OVERLAY-141-IMPL）；未 commit/未动版本/未出包/零凭证/无临时文件。⚠️ 行号系 09-07 快照（main.rs 正被 coder-2 并行修改），验收以符号名锚点为准。
