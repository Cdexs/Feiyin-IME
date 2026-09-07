# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。


## 2026-09-07 — tester-1 — BUILD-145 ✅ 阶段五出包：首个含 OVERLAY-141 圆角灰边根治的包（待主控验收/Gavin 端测）

- **基线**：HEAD `bd5a160` clean，主控明确下达「现在可以出包」。
- **Step 2 跳过（主控核实合法）**：ui/ 最后改动 `f85c550`(09-06 18:47) 早于 UI exe 构建(09-07 01:15)，已含全部 ui/ 改动；两副本 sha `05cef408…` 一致。沿用 BUILD-129/135 先例。
- **反向对照基线改绑**：存在一次未记录构建（09-07 00:59，sha `7f1869ad…`，12,267,520 B，主控抢存 `collab/evidence/binaries/feiyin-ime-PRE141-20260907-0059.exe`）⇒ 第 2/5 项与大小基准用 PRE141，不用 BUILD-135 `ac70a990…`（隔两代判别力空）。
- **构建**：Step1 清进程 → Step3 主程序（2m25s，feiyin-ime 111 warnings / crash-reporter 5，均与基线持平）→ Step4 同步 Publish/ + toml 三副本。
- **七项核验全 PASS**：① 主程序时间戳 11:50 本次构建；② sha 两副本三对相等，主 `59d64c4d…` 异于 PRE141 `7f1869ad…`；③ toml 三副本 scene `0a3a0b9a…`/itn `311cbb96…` 一致；④ ProductVersion 0.9.0.0 未动；⑤ 🔴 判别探针三证（如实：OVERLAY-141 纯几何零新增字符串/import，二进制探针不可构造 → 源码 grep `OVERLAY_FRAME_RADIUS_LG`×12/`apply_alpha_fixup`×3/`overlay_frame_radius`×1 + 构建时间戳 11:49-11:50 + sha 差异，字节级 10.9MB 代码字节真实不同）；⑥ 大小 12,268,032 B 同量级（+512 B）；⑦ 冒烟 PID 18772 Responding=True 无 panic；⑧ config.toml sha 前后不变。
- **红线**：未 commit / v0.9.0 未动 / 未用 cargo tauri build / 未 cargo clean / 零凭证 / 无临时文件。
- **Gavin 端测清单 7 项**见 `outbox/tester-1/result.md`（圆角平滑/信息窗/错误窗/处理中 shimmer/圆角外透明/编辑态逐位一致/进编辑态采 debug.log）。


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

## 2026-09-07 — coder-1 — TEST-SYNC-143 ✅ 阶段三（OVERLAY-141 护栏换血 G6 + 半径单一来源 G8/G9，沙箱预演 3 PASS + 消融 10 变异全验红，待主控验收 + 阶段四）

- **交付**：src/main.rs 测试区 +146/-15（四 hunk 全在 overlay_121_guard_tests，生产区 0 字节）。**G6 换血**：旧「三分支完整」命题随按色三分支退役而失效 → `g6_fixup_sdf_alpha_invariants` 四条：①SDF 覆盖率 needle ②`if cov <= 0.0` 块内四通道全零（逐通道）③反预乘 `if a > 0` ④🔴反向断言（contains 级）钉死 `a == 0 && (r|g|b)` 禁复活——「圆角灰边」事故转机器判据。**G8**：`frame_radius = overlay_frame_radius(` 赋值形态恰 1（needle 绑完整形态，`let mut` 兜底行/调用行不误计）+ 调用行实参绑 `frame_radius`。**G9**：三类外框绘制调用（draw_overlay_chrome 5/chrome 4/折行 chrome_with 4）实参必须引用 OVERLAY_FRAME_RADIUS_，逐调用收集到 `);`，总数钉 13；豁免=fn chrome 内透传形参 radius；🔴不扫全库 CORNER_RADIUS 防假红。新增 helper `block_contains_raw`；G2-G5/G7 未动；模块名沿用。
- **验证**：cargo fmt 0（幂等）/ check --all-targets 0 error / warnings **111/102 基线逐位持平**；沙箱预演（独立 rustc harness 于 tmp/opencode/ts143，未触仓库 cargo test）对 fmt 后真实文件 **3 PASS**；消融 10 变异全验红（G6×5 含🔴塞回旧规则⇒红 / G8×3 / G9×2）+ 豁免假红核对 1 条 PASS（result.md 附录 A）——消融在临时副本执行，`git diff src/` 始终为空（规避 TEST-EXEC 还原误伤教训）。
- **macOS**：模块 cfg(all(test, windows)) 整体不编译，不适用。
- **红线**：未 commit / v0.9.0 未动 / 未出包 / 未跑 cargo test / 零凭证；临时目录已整删（check.rs/exe/mutant.rs/ablate*.py/out，result.md 附录 B）。
