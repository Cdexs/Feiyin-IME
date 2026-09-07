# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。



## 2026-09-07 — tester-1 — BUILD-173 ✅ 出包：FIX-172 声波弧动效修复 + 最右侧闪烁本体（精简流程，待主控验收）

- **基线**：HEAD `b0b5c66` clean（FIX-172 修复已由主控提交）。
- **精简流程**：Step1 清进程（无残留）→ Step2 git-log 法 UI 免重建（f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序（2m07s，feiyin-ime 111 / crash-reporter 5）→ Step4 cp -p 同步 Publish/。
- **七项核验全 PASS**：① 主程序 22:07 本次构建；② 两副本 sha `f698a431…` 一致异于 `3a55ad20…`；③ ProductVersion 0.9.0.0；④ 冒烟 PID 15508 Responding=True 无 panic 已清理；⑤ config.toml sha `3186ec8c` 不变；⑥ warnings 111/102；⑦ 🔴 判别探针三证 PASS（源码 grep+时间戳+sha；WM_SETREDRAW grep=4 含 2 注释代码级=2，amplitude grep=4 含 1 注释代码级=3）。
- **回归（并行）**：cargo test 全量 **1110P/0F/9I** + hotkey 51P + ui_guard 2P；Vitest/E2E Skip。🔴 crash_reporter config 批量 FAIL **本轮未出现**（连续五轮未复现，维持归档偶发）。
- **出包语义**：声波弧「一起亮一起灭」（A 地板截断改抬高振幅，FIX-164 回归修复）+ 编辑态最右侧文字闪烁（B 滚动位块 WM_SETREDRAW 包裹）；🔴 已知取舍如实写入（WM_PRINTCLIENT 下选区反白不渲染，Gavin 已接受）。不出端测清单给 Gavin，主控自出。
- **红线**：未 commit / v0.9.0 未动 / 未 cargo clean / 未 cargo tauri build / 零凭证 / 无临时文件。



## 2026-09-07 — tester-1 — BUILD-171 ✅ 出包：FLICKER-170 编辑态移光标闪烁（精简流程，待主控验收）

- **基线**：HEAD `8efbf89` clean（FLICKER-170 修复已由主控提交）。
- **精简流程**：Step1 清进程（无残留）→ Step2 git-log 法 UI 免重建（f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序（2m06s，feiyin-ime 111 / crash-reporter 5）→ Step4 cp -p 同步 Publish/。
- **七项核验全 PASS**：① 主程序 21:20 本次构建；② 两副本 sha `3a55ad20…` 一致异于 `20b371d4…`；③ ProductVersion 0.9.0.0；④ 冒烟 PID 25444 Responding=True 无 panic 已清理；⑤ config.toml sha `3186ec8c` 不变；⑥ warnings 111/102；⑦ 🔴 判别探针三证 PASS（源码 grep+时间戳+sha；WS_CLIPCHILDREN grep=4 含 2 注释代码级=2，WM_PRINTCLIENT grep=3 含 1 注释代码级=2）。
- **回归（并行）**：cargo test 全量 **1110P/0F/9I** + hotkey 51P + ui_guard 2P；Vitest/E2E Skip。🔴 crash_reporter config 批量 FAIL **本轮未出现**（连续四轮未复现，维持归档偶发）。
- **出包语义**：编辑态移光标闪烁（B' WM_PAINT 单次合成）+ 进出编辑态/文字更新整块被盖（A WS_CLIPCHILDREN）；🔴 已知取舍如实写入（WM_PRINTCLIENT 下选区反白不渲染，Gavin 已接受）。不出端测清单给 Gavin，主控自出。
- **红线**：未 commit / v0.9.0 未动 / 未 cargo clean / 未 cargo tauri build / 零凭证 / 无临时文件。



## 2026-09-07 — tester-1 — BUILD-168 ✅ 出包：DIAG-166 托盘图标根因修复（精简流程，四条全覆盖首包，待主控验收）

- **基线**：HEAD `2dc9474` clean（DIAG-166 修复已由主控提交）。
- **精简流程**：Step1 清进程（无残留）→ Step2 git-log 法 UI 免重建（f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序（2m08s，feiyin-ime 111 / crash-reporter 5）→ Step4 cp -p 同步 Publish/。
- **七项核验全 PASS**：① 主程序 20:33 本次构建；② 两副本 sha `20b371d4…` 一致异于 `85c1cf26…`；③ ProductVersion 0.9.0.0；④ 冒烟 PID 26760 Responding=True 无 panic 已清理；⑤ config.toml sha `3186ec8c` 不变；⑥ warnings 111/102；⑦ 🔴 判别探针字节级 PASS（新文案含 expected= 进包、旧文案 rgba_len=…) 形态 0 命中）。
- **回归（并行）**：cargo test 全量 **1110P/0F/9I** + hotkey 51P + ui_guard 2P；Vitest/E2E Skip。🔴 crash_reporter config 批量 FAIL **本轮未出现**（连续三轮未复现，维持归档偶发）。
- **出包语义**：托盘图标（第 1 条）✅ 已修 = 四条全覆盖首包。不出端测清单给 Gavin，主控自出。
- **顺带修正**：logs/20260907.md BUILD-165 条目乱码 `0X0P+0PPDATA`→`%APPDATA%`。
- **红线**：未 commit / v0.9.0 未动 / 未 cargo clean / 未 cargo tauri build / 零凭证 / 无临时文件。



## 2026-09-07 — tester-1 — BUILD-165 ✅ 出包：FIX-162 + FIX-164（精简流程，Gavin 端测打回四条的修复包，待主控验收）

- **基线**：HEAD `e012108` + 未 commit 的 FIX-162（WS_EX_COMPOSITED 回退）+ FIX-164（Part A 预热+1500ms 有界等待 / Part B 声波弧阈值 0.10+地板 0.35 / Part C GetLastError 日志），主控已验收。
- **精简流程**：Step1 清进程（PID 23692）→ Step2 git-log 法 UI 免重建（f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序（2m00s，feiyin-ime 111 / crash-reporter 5 warnings）→ Step4 cp -p 同步 Publish/。
- **六项核验全 PASS**：① 主程序 19:40 本次构建；② 两副本 sha `85c1cf26…` 一致异于 `b3043119…`；③ ProductVersion 0.9.0.0 未动；④ 冒烟 PID 29352 Responding=True 无 panic 已清理；⑤ config.toml sha `3186ec8c` 不变；⑥ warnings 111/102。
- **回归（并行）**：cargo test 全量 **1110P/0F/9I** + hotkey 51P + ui_guard 2P + 其余 0F；Vitest/E2E Skip。🔴 crash_reporter config 批量 FAIL **未复现**（归档偶发）。
- **出包语义**：修 Gavin 端测第 4/3/2 条；🔴 第 1 条（托盘图标）未修，仅加 GetLastError 日志待 debug.log 定位。不出端测清单给 Gavin，主控先自测目视。
- **红线**：未 commit / v0.9.0 未动 / 未 cargo clean / 未 cargo tauri build / 零凭证 / 无临时文件。



## 2026-09-07 — tester-1 — BUILD-159 ✅ 出包：TRAY-ICON-158(+FIX) + MIC-PULSE-160 + EDIT-FLICKER-157（精简流程，Gavin 在等，待主控验收/端测）

- **基线**：HEAD `85388ac` clean。
- **🔴 对照基线抢存**：`Publish/feiyin-ime.exe` 旧二进制（sha `e6f1445e…`，mtime 15:18，五文档零记录构建）已备份 `collab/evidence/binaries/feiyin-ime-PRE159-UNRECORDED-1518.exe`。
- **精简流程**：Step1 清进程 → Step2 git-log 判 UI 免重建（f85c550@09-06 18:47 早于 exe 01:15）→ Step3 主程序（2m14s，111 warnings 持平）→ Step4 cp -p 同步 Publish/。
- **五项核验全 PASS**：① 主程序 18:14 本次构建；② 两副本 sha `b3043119…` **异于** e6f1445e…；③ ProductVersion 0.9.0.0 未动；④ 冒烟 PID 28976 Responding=True 无 panic 已清理；⑤ **Publish==target/release sha 完全一致**（历史不一致问题顺带修复）。
- **跳过**：toml 三副本/判别探针/大小对照/cargo test/vitest/E2E/消融。config.toml sha `3186ec8c` 未变。
- **红线**：未 commit / v0.9.0 未动 / 未用 cargo tauri build / 未 cargo clean / 零凭证 / 无临时文件。
- **Gavin 端测清单 6 项**见 `outbox/tester-1/result.md`（🔴 托盘图标目视 / 🔴 流式麦克风声波弧动效 / 🔴 静音时无弧 / 🔴 流式上屏卡顿闪烁 / 编辑态文字闪烁消失 / 编辑态移光标无新增闪烁）。
## 2026-09-07 — coder-2 — EDIT-FLICKER-157 ✅ 编辑态右侧文字闪烁：EDIT 开双缓冲（待主控验收 → 直出包）

- **机制复核成立**（三条定位逐条核对）→ 修法 = `CreateWindowExW` 扩展样式加 `WS_EX_COMPOSITED`（一行 + import，共 +8/-3 双 hunk :102/:678）。
- **风险留痕**：`WS_EX_COMPOSITED` 在 SLWA 分层窗子控件上未运行时实测——Gavin 端测若见 EDIT 不显示/异常，回退 = 子类拦 `WM_ERASEBKGND` 自绘背景（父窗同色刷+内存 DC BitBlt）或创建后 `SetWindowLongPtrW`，已注释标注。
- **验证**：fmt 0 / check --all-targets 0 error / warnings **111/102 持平** / cargo test **1109P/0F**。红线：只动 create_edit_control+import / 未 commit / 零凭证。


## 2026-09-07 — tester-1 — BUILD-156 ✅ 快速出包：OVERLAY-155 圆角三次修法（精简流程，Gavin 在等，待主控验收/端测）

- **基线**：HEAD `aeaebe1` clean。
- **精简流程**（免阶段四/消融）：Step1 清进程 → Step3 主程序（1m45s，111 warnings 持平）→ Step4 cp -p 同步 Publish/。
- **Step2 Skip（git-log 法）**：`git log -1 -- ui/ src-tauri/` ⇒ `f85c550 @09-06 18:47` 早于 UI exe 01:15。
- **四项核验全 PASS**：① 主程序 14:55:13 本次构建；② sha 两副本一致 `a0521728…` **异于** PRE155 `fb9fcf50…`；③ ProductVersion 0.9.0.0 未动；④ 冒烟 PID 8956 Responding=True 无 panic 已清理。
- **跳过**：toml 三副本/判别探针/大小对照/cargo test/vitest/E2E/消融。config.toml sha `3186ec8c` 未变。
- **红线**：未 commit / v0.9.0 未动 / 未用 cargo tauri build / 未 cargo clean / 零凭证。
- **Gavin 端测清单 5 项**见 `outbox/tester-1/result.md`（🔴 录音+处理中窗圆角灰边恢复 1px / 信息·错误窗不回退 / 录音窗光标复验 / 编辑态停止热键复验 / 🔴 保留 debug.log 给 E3）。


## 2026-09-07 — tester-1 — BUILD-154 ✅ 快速出包：OVERLAY-153 圆角二次修法（精简流程，Gavin 在等，待主控验收/端测）

- **基线**：HEAD `14bce8f` clean。
- **精简流程**（Gavin 指示，免阶段四/消融）：Step1 清进程 → Step3 主程序（2m09s，111 warnings 持平）→ Step4 cp -p 同步 Publish/。
- **Step2 Skip（git-log 法）**：`git log -1 -- ui/ src-tauri/` ⇒ `f85c550 @09-06 18:47` 早于 UI exe 01:15。
- **四项核验全 PASS**：① 主程序 13:55:38 本次构建；② sha 两副本一致 `fb9fcf50…` **异于** PRE153 `b20fbe14…`；③ ProductVersion 0.9.0.0 未动；④ 冒烟 PID 27380 Responding=True 无 panic 已清理。
- **跳过**：toml 三副本/判别探针/大小对照/cargo test/vitest/E2E/消融（本批只改 apply_alpha_fixup 单函数）。config.toml sha `3186ec8c` 未变。
- **红线**：未 commit / v0.9.0 未动 / 未用 cargo tauri build / 未 cargo clean / 零凭证。
- **Gavin 端测清单 5 项**见 `outbox/tester-1/result.md`（🔴 圆角灰边恢复 1px / 信息·错误窗不回退 / 录音窗光标复验 / 编辑态停止热键复验 / 🔴 保留 debug.log 给 E3）。


## 2026-09-07 — coder-2 — OVERLAY-153 ✅ 圆角二次修法：覆盖率当乘数（内部区不再强制不透明，待主控验收 → 直出包）

- **推论验证：成立**。`*p.add(3) = premul(255, k)` 无条件覆写 ⇒ 内部区（cov=1）a 强制 255·op；描边直线段与栅格对齐无 AA、**圆角弧段 AA 带铺 2-3px 全在 cov=1** ⇒ 角区实心灰团 = 「包边灰线粗乱」伪影本体；141 相对 121 在内部区是退步属实。
- **修法**：三分支 cov≤0 归零（不变）/ a>0 等比 ×(cov·op)（**保留原 alpha**，反预乘整步删）/ a==0 GDI 提亮（未存活世界与 141 逐位同）。§三两世界安全论证复核成立；唯一 nuance = 存活世界轮廓 1px 坡 cov² 略陡（如实报告，量级远小于收益，不构成停手）。
- **G6**：四条 needle 改后全绿**零改动**，仅 ③ 语义注释/消息更新（流程变更授权项，:11946/:11988）。
- **验证**：fmt 0 / check --all-targets 0 error / warnings **111/102 持平** / cargo test **三跑稳定 1109P/0F**（首跑 config 瞬态红未复现，如实记录）；护栏模块 10P/0F。
- **红线**：只动 fixup（生产区 1 hunk）+ G6 注释；未顺手改 r-0.5（F2 仍否决）；E3/E4 探针照留；未 commit/未动版本；零凭证。diff：生产 1 hunk + 测试 2 hunk。


## 2026-09-07 — tester-1 — TEST-EXEC-152 ✅ 阶段四：OVERLAY-149 + TEST-SYNC-150 全量回归 + G10/G11 消融真跑（生产零改动，待主控验收）

- **基线**：HEAD `7ddf943` clean。
- **Step1a 三层**：root **1109P/0F/9I**（预期 1107+G10/G11 逐位命中）+ hotkey 51P + src-tauri **76P/0F**。
- **Step1b Skip（git-log 法）**：`git log -1 -- ui/` ⇒ f85c550@09-06 18:47、`-- src-tauri/` ⇒ 9c9ff73@08-30 均早于上次跑测 ⇒ ui/ 零改动。
- **Step2 消融 3/3 RED 真跑 + 逐条还原**：A1 删 DestroyCaret 行留注释 → G10 红（注释不喂绿）；A2 caret 三步挪 DestroyWindow 后 → G10 红（L869/875/876 早于 L862 顺序守卫命中）；A3 删 Stop 臂 store(false) → G11 红。每条 Edit 还原。
- **还原自证**：git diff src/main.rs = 0 行；复跑 1109P/0F/9I 0 红；无残留进程；Publish/config.toml sha `3186ec8c` 未变。
- **红线**：零生产改动 / 未 commit / v0.9.0 未动 / 未出包 / 零凭证。


## 2026-09-07 — coder-2 — TEST-SYNC-150 ✅ 阶段三：OVERLAY-149 双修复护栏 G10/G11（生产区零字节，待主控验收 → TEST-EXEC）

- **G10（caret 清理顺序）**：`destroy_edit_control` 内 `let _ = SetFocus(` / `let _ = HideCaret(` / `let _ = DestroyCaret(` 三行必须全部早于 `let _ = DestroyWindow(`——顺序是护栏主体（销毁后清 caret=空操作）。needle 全代码行首形态注释免疫；锚 `fn destroy_edit_control(` raw 计数=1 唯一；块定界起点断言防漂移；PROBE 探针块删除不影响。
- **G11（Stop 臂清 OVERLAY_EDITING）**：结构锚 `HotkeyEvent::Stop => {` raw 计数=1（macOS 臂 `platform::` 前缀不可命中，不绑日志文案——主控裁定采纳）；sanity=`if is_recording.load(` 结构特征，误锚红显式暴露。
- **沙箱预演+消融 4 条全验红**（独立 rust 复刻 helper 语义，验后整目录删除）：A1 删 DestroyCaret 代码行**留注释**→G10 红（主控追加的注释喂绿验证）；A2 三步挪 DestroyWindow 后→顺序红；A3 删 store(false)→G11 红；A4 裸重复臂注入→sanity 红。baseline 双 PASS。验后还原：`git diff --numstat src/main.rs` = **+82/-0 单 hunk @ mod overlay_121_guard_tests（:12102，测试区）**。
- **macOS 定稿**：OVERLAY_EDITING 唯一 store(true) 在 cfg(windows) 内 ⇒ macOS 恒 false ⇒ F2 缺陷 macOS 不可达，macOS Stop 臂无需同款守卫。
- **验证**：fmt 0 / check --all-targets 0 error / warnings **111/102 基线逐位持平**。红线：未跑 cargo test/build / 未 commit / 未动版本 / 零凭证 / 沙箱已清理。


## 2026-09-07 — tester-1 — BUILD-151 ✅ 阶段五出包：首个含 OVERLAY-149 caret 修复 + 热键停止卡屏修复 + 圆角判别探针 E3/E4 的包（待主控验收/Gavin 端测）

- **基线**：开工 HEAD `115eb5b` + 工作区仅 `docs/MACOS-HANDOFF.md`（coder-2 macOS 复核同步，主控确认漏 add）；构建期主控补提交 `69aded0`，`git diff 115eb5b 69aded0` 仅该文档 +6 行零代码差异 ⇒ **报告基线 69aded0，二进制零影响**。
- **Step 2 跳过（判据升级）**：git-log 法 `git log -1 --format='%h %ad' -- ui/ src-tauri/` ⇒ `f85c550 @09-06 18:47` 早于 UI exe 01:15 ⇒ 已含全部 ui 改动（不依赖 diff 区间起点）；补充 `git diff bd5a160..HEAD -- ui/ src-tauri/` 为空。
- **构建**：Step1 清进程 → Step3 主程序（1m58s，111 warnings 持平）→ Step4 **cp -p** 同步 Publish/（BUILD-145 订正项，UI mtime 保留 01:15）+ toml 三副本。
- **七项核验全 PASS**：① 主程序 13:07:47 本次构建；② sha 两副本三对相等，主 `b20fbe14…` 异于 PRE149 `59d64c4d…`；③ toml 三副本一致；④ ProductVersion 0.9.0.0 未动；⑤ 🔴 **硬判别探针正反双向**（OVERLAY-149-PROBE E3×2/E4×2/F1×1/pre-fixup×1/post-fixup×1 新包全命中，PRE149 6 串全 0）；⑥ 大小 12,277,248 B（+9,216 B 含探针代码合理）；⑦ 冒烟 Responding=True 无 panic 已清理；⑧ config.toml sha 前后不变。
- **红线**：未 commit / v0.9.0 未动 / 未用 cargo tauri build / 未 cargo clean / 零凭证 / 无临时文件。
- **Gavin 端测清单 5 项**见 `outbox/tester-1/result.md`（caret 是否消失 / 编辑态停止热键收口 / 圆角三态预期无变化 / 🔴 保留 debug.log 给 E3/E4 / 编辑态闪烁仍在）。


## 2026-09-07 — coder-2 — OVERLAY-147-DIAG ✅ BUILD-145 端测两问诊断（🔴 只查不修，src 零改动，待主控验收）

- **Part A**：SDF 数学实证正确（探针 vs 精确面积覆盖 mean|Δ|=0.0002/0.0001）⇒ 掩码候选整片砍掉；**H1a 如实推翻**（描边深度偏差 r 无关 ~0.2px，r16 [-0.008,+1.207] vs r10 [-0.012,+1.207]，描边从不探出轮廓；主控 r-0.5 同心化修法方向正确列 F2 顺手项）；**H2 被静态推翻**（RecordingStreamingIdle = r16+静态+坏，`:1678-1686`；待 Gavin 确认看的是占位变体）；窗口高度/内容同构混淆全部排除 ⇒ 剩余候选 {半径值未测路径, 底色}，**机制未定**，判别实验 E1/E2（半径翻转）+ E3（角区像素 dump，顺带验证「BindDC alpha 未存活」从未直接测过的立论前提）已写进 `docs/OVERLAY-147-DIAG.md §A5`。
- **Part B**：EDIT 不可能存活到录音帧（Show 臂 `:1366` 无条件销毁）⇒ 截图光标 = **线程级 caret 泄漏**主假设（全库零 DestroyCaret、destroy 无 caret 处置、带焦 DestroyWindow 无 WM_KILLFOCUS、caret 直打屏幕不走 ULW 合成），探针=退出编辑后 `GetGUIThreadInfo().hCaret`；**新钉死可达缺陷**：`HotkeyEvent::Stop :5598` 缺 OVERLAY_EDITING 守卫 → 编辑中热键停止 → **Processing 浮层永久卡屏**（Done/Cancelled 全被 `:5718` 压制）；Hide 双重压制卡窗静态存在、可达触发器疑似为零（编辑态 ESC 不可达）；**FLICKER-130 耦合证伪**（被丢 Show 本是销毁 EDIT 的肇事路径，清理不依赖它，`:5679-5681` 注释自述）。
- **产出**：`docs/OVERLAY-147-DIAG.md`（直接写 docs/）；修法建议 F1-F5 全部标注不实施。
- **红线自证**：`git diff --numstat -- src/` 空；探针文件已删；未 commit/未动版本/未出包；零凭证；无临时文件。


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

## 2026-09-07 — coder-1 — OVERLAY-149 ✅ F1 caret 泄漏修复 + F2 热键停止臂守卫 + E3/E4 判别探针（cargo test 1107P/0F 零预期红，待主控验收 + tester-1 阶段四 + Gavin 端测）

- **F1（🔴 待端测判定，未预先宣布已修）**：destroy_edit_control 在 DestroyWindow 前新增三步——GetFocus==edit ⇒ SetFocus(父窗) / HideCaret / DestroyCaret。语义依据 DIAG §B1/B2（带焦销毁收 WM_DESTROY 而非 WM_KILLFOCUS、caret 绑线程输入队列、系统 caret 屏幕级绘制不走 ULW 合成）。端测判定：光标消失=假设坐实；仍在=假设推翻须回报重开调查。运行时证据探针（OVERLAY-149-PROBE F1）destroy 后查 GetGUIThreadInfo.hwndCaret。
- **F2**：HotkeyEvent::Stop 臂补 OVERLAY_EDITING.store(false)（无条件 store；非编辑态逐位不变，controller 线程单写者）。修复卡屏可达序列：编辑态（worker 未 finalize）按停止热键 → FallingToProcessing → Cancelled 被 :5720 压制 → 永久卡屏；修复后 Done/Cancelled 不再被压制 → Idle+Hide 收口。**方案选择**：无条件 store(false) 而非改发 RestoreAndHide——Case-B（worker 已 finalize，is_recording=false）Stop 臂 is_recording 门关着本就不 Show，此时 RestoreAndHide 反而会多余销毁 EDIT 藏窗；完整 Step1-4+Case-A/B/C 序列在 result.md §二。
- **E3**：ULW 分支 apply_alpha_fixup 前/后采样四角 8×8+左缘整列 BGRA hex（首测 OVERLAY-141 立论前提「BindDC 后 alpha 未存活」）+元数据（w/h/radius/opacity/状态名）；节流=状态切换后仅前 3 帧、仅 RecordingStreamingIdle(r16 坏)/Info(r10 好) 对照态；判读期待预声明（pre-fixup 左缘列 alpha 存活 ⇒ 前提被推翻）。
- **E4（口径替代已报备）**：GetDpiForWindow 需 Cargo.toml Win32_UI_HiDpi feature（红线只许改 src/main.rs）→ GetDeviceCaps(LOGPIXELSX/SY) 对同一 hdc（DIB 源 DC）+ d2d::with_d2d 内 RT GetDpi once；如主控裁定要原文 API 需加 feature（一行，主控/后续批次）。
- **探针删除清单**（result.md §六 四处）：①两 helper fn ②WM_PAINT 两调用 ③with_d2d once 块 ④F1 证据块；caret 清理本体=F1 实修复永久保留。
- **验证**：fmt 0（幂等）/check --all-targets 0 error/warnings 111/102 基线持平/cargo test（本单允许）1107P/0F+51P/0F——**零预期红**，G1-G9 全绿（F2 Stop 臂系独立 match 臂，F3 锚 EditRequested 臂不受影响）。
- **明确不做**（主控裁定）：stroke 同心化/Hide 双压制/编辑态 ESC/E1·E2/ui/**。
- **macOS**：四件事全 Windows 专属，MACOS-HANDOFF.md 已同步三段。
- **红线**：只改 src/main.rs（+230/-14）；未 commit/版本未动/未出包/零凭证/无临时文件。

## 2026-09-07 — coder-1 — OVERLAY-155 ✅ 圆角三次修法（GDI chrome 收进 D2D 失败分支 + LG 半径 16→10，1109P/0F=基线，交付即出包，待 Gavin 端测）

- **差异 A 结论（成立+精确收窄）**：全库逐位排查「GDI chrome 先于 D2D 无条件执行」——唯一肇事点 = `draw_recording_overlay` 波形分支（show_placeholder=false 每帧先画无 AA 的 GDI RoundRect，D2D waveform 在后）：1px 灰描边 + D2D AA 描边错位 ~0.5px 叠画 → 角区灰线粗乱；这些像素全在 SDF cov>0 区，fixup 清不掉 ⇒ 121/141/153 三轮改 fixup 改错地方。**其余七处（placeholder 分支/with_text/FallingToProcessing/Processing/Editing/FocusLost/Error/Info）本来就是 Info 同构，零改动。**
- **改动 1**：chrome 移进 `show_placeholder` 分支（D2D idle 失败兜底）；波形分支 D2D 在先、成功即 return、失败才 chrome。D2D 失败兜底契约完整（失败路径与改前逐位相同；`if !d2d::draw_` 形态护栏绿）。
- **如实声明（防 PLAUSIBLE-FIX）**：Idle（无叠画）与 Processing（无叠画）端测也报坏，差异 A 解释不了这两态——DIAG §A2 给 Gavin 的「Idle 是否单独看过」确认问题仍未答；**改动 2（LG 16→10）是覆盖全部 r=16 组的共同修复**，两改动互补。
- **改动 2**：`OVERLAY_FRAME_RADIUS_LG: 16.0 → 10.0`（单一来源一行全生效；LG/SM 保留未合并，Gavin 想要回 16 改回字面量即可）。
- **验证**：fmt 0（幂等）/check --all-targets 0 error/warnings 111/102 基线持平/**cargo test 1109P/0F（=主控基线）护栏零变红，未改任何 needle**。
- **红线**：apply_alpha_fixup 零字节未动；E3/E4 探针照留；只改 src/main.rs（+16/-5 两 hunk）；未 commit/版本未动/未出包/零凭证/无临时文件；MACOS-HANDOFF 已同步。

## 2026-09-07 — coder-1 — TRAY-ICON-158 ✅ 托盘菜单项加图标（齿轮/电源，品牌橙 SSAA 程序化生成，待主控验收 → BUILD-159 出包）

- **方案**：同意主控并执行。`src/ui/menu_icons.rs` 新建（纯 Rust、零新 crate）：齿轮（外径 0.42S/8 齿 20°/齿根 0.30S/孔 0.15S）+ 电源（环 0.30S/线宽 0.11S/顶 70° 缺口/竖线圆头），SSAA 4×4，品牌橙 #FF6B35。
- **Windows**：`create_menu_item_bitmap`（32bpp top-down 预乘 BGRA DIB）+ `attach_menu_icons`（SM_CXSMICON/CYSMICON min clamp(16,64)，MIIM_BITMAP 按 wID 挂载，失败静默降级）；`DeleteObject` 在 `DestroyMenu` 后、现建现删不缓存。MENU_VISIBLE/SetForegroundWindow/TPM 标志/命令 ID 零触碰。
- **macOS**：`build_tray_menu` 两项 `setImage(nsimage_from_rgba(18,…))` 不设 template；🔴 只写不构建不端测，`docs/MACOS-HANDOFF.md` 已留痕。
- **视觉自证**：常驻 `dump_menu_icons_preview` 产出 12 张 PNG（16/24/32 × 2 × 原图+x8）在 `collab/outbox/coder-1/icons/`，待主控逐张目视。**如实声明**：16px 档 8 齿齿轮欠采样发糊（齿宽 ~1.2px 固有），变体 B（22.5° 齿宽）略好/C（6 齿）更差已删，维持规格等 Gavin 定夺（调整只需动 TOOTH_HALF_ANGLE_RAD）。
- **验证**：fmt 幂等 0／check --all-targets 0 error／warnings 111/102 逐位持平／cargo test **1110P/0F**（1109 基线+1 dump，如实说明）；Cargo.toml 零字节；diff 5 生产/文档文件全在任务书清单内；未 commit/版本未动/未出包/零凭证/探索临时 PNG 已清理。

## 2026-09-07 — coder-1 — TRAY-ICON-158-FIX ✅ 齿轮几何返工（电源已验收零改动，12 张 PNG 重 dump，待主控目视 → BUILD-159）

- **打回根因三条全修**（照主控处方，只动 `gear_covered`+常量）：①实心盘 hole(0.13S)..root(0.32S) 整片实心（原齿下盘被挖空⇒放射刺/雪花）②齿改梯形（齿根半角 15°→齿顶半角 10° 随 r 线性收窄，原恒定角宽=外宽内窄反向）③8 齿→6 齿（16px 密度过载，对齐系统级 UI 齿轮）+外径 0.42→0.44S。
- **自查**：16px 清晰可辨、32px 标准齿轮；12 张预览 PNG 已重新 dump 覆盖 `collab/outbox/coder-1/icons/`。
- **红线**：`power_covered` 及电源 5 常量零字节未动；`rasterize`/Win32 挂载/macOS setImage 零新增改动；fmt 0/check 0 error/warnings 111/102 持平/test 1110P/0F；Cargo.toml 零字节/未 commit/版本未动/未出包/零凭证。

## 2026-09-07 — coder-1 — MIC-PULSE-160 ✅ 流式窗麦克风声波弧动效（待主控目视 8 帧 → 不出包等 Gavin）

- **视觉**：左右各一组对称声波弧（内 R5.5/外 R8.0，右 -40°..+40° / 左 140°..220°，同相位同亮度，线宽 1.2），墙钟 1000ms 周期 tri 波（内弧先行 0.0、外弧滞后 0.30、宽 0.55），电平驱动 gain（≥0.35 满亮），外弧 ×0.85 略淡。静音完全不画（逐位零回归）；红/灰态不画。两轮 Gavin 修订（1000ms、左右对称）已并入。
- **三产出源**：①D2D mic_indicator（main.rs:4252，弧=mic_pulse_arcs:4367，clip+DrawEllipse×4+SetOpacity 恢复）②GDI 波形态（:3071-3081）③GDI 流式态（:3525-3534）——后两者 4x 画布采样折线 20 段/弧+BG 插值近似。共享纯函数 main.rs:2882/2898 三处同源。
- **重绘**：RecordingWithText(:1871)/RecordingStreamingIdle(:1814) dirty 加 `|| mic_has_audio(&state)`；StreamingEditing 零改动；MENU_VISIBLE 门控原位。锁纪律：绘制侧 snapshot 一次锁取 (empty,audio,level)（:2908），判定侧每帧一次锁。
- **预览**：8 帧 PNG（gain=1.0, ×8）在 collab/outbox/coder-1/mic-frames/，自查节奏=内弧先亮→外弧跟进→渐隐；临时 dump 测试已删（保持 1110P）。
- **macOS**：overlay.rs 无流式窗麦克风元素 → 不适用，MACOS-HANDOFF 记待办。
- **验证**：fmt 0／check 0 error／warnings 111/102 持平／test 1110P/0F；TRAY-ICON-158 零触碰；Cargo.toml 零字节；未 commit/版本未动/未出包/零凭证。

## 2026-09-07 — coder-2 — FIX-162 ✅ P0 止血：WS_EX_COMPOSITED 已回退（EDIT 大黑屏/文字全丢，待主控验收 → 直出包）

- **改动**：`src/main.rs` 两 hunk = 62800bc 的逆操作：import 行删 `WS_EX_COMPOSITED`、`create_edit_control` ex-style 回退 `WS_EX_NOACTIVATE`；注释改为回退留痕，备选方案（WM_ERASEBKGND 子类自绘 / SetWindowLongPtrW）原样保留供后续参考。
- **验证**：grep=0／fmt 幂等／check --all-targets 0 error／warnings 111/102 持平／cargo test 1110P/0F + 其余二进制全绿。首轮 crash_reporter 24F 为并行环境偶发（crash-reporter 不含 main.rs），复跑两轮全绿。
- **状态**：编辑态右侧文字闪烁回到「未修」状态（已知问题回到原点）；未 commit/版本未动/未出包/零凭证。可立即出包给 Gavin。

## 2026-09-07 — coder-1 — DIAG-163 ✅ BUILD-159 端测三问诊断（🔴 只查不修，src/ 零改动，待主控验收 + runtime 复核清单）

- **Q3 切模型出错窗（机制已定，非回归）**：DEC-025 异步热重载缺口——切模型后**首次录音必用旧 transcriber**（worker 判据 config 且实例双条件，ASR-038-B `4f3b41b` 引入；重载机制 `81304f7` v0.6.1；无条件 `RecordingStarted` 初始提交即有，三者均先于 aeaebe1/62800bc/85388ac，三提交 hunk 范围取证未触碰状态选择）。窗口期 worker 走本地管线 → RecordingStarted 覆盖控制器按 config 先画的流式占位窗 → 频谱窗。**只错外观不错输出，第二次按热键自愈**。若二次仍坏则本机制证伪（debug.log 找 `hot-reload failed` 循环）。定案签名：日志时序 Triggering hot-reload → worker received Start → ensure_stream（非 record_streaming）→ hot-reload completed 在 Start 之后。
- **Q1 托盘菜单图标（机制未定，头号怀疑证伪）**：`show_tray_popup_menu` 自建 CreatePopupMenu，attach/TrackPopup/Destroy 全程同一 HMENU；tray-icon 0.19 无 with_menu，crate 菜单不存在。wID=1001/1002、MIIM_BITMAP 单掩码合法（MSDN + SO 78577359 同型确认）、biHeight 负 top-down、预乘 BGRA、DeleteObject 在 DestroyMenu 后——**静态全链路零缺陷**。根因只能在静默降级黑箱（SetMenuItemInfoW / CreateDIBSection 运行时成败无日志）。建议：一次性插桩诊断包（log size/DIB 成败/SetMenuItemInfoW 返回/attached 数），右键一次定案——**未实施，等主控批**。
- **Q2 声波弧（链路查通 + 核心矛盾）**：流式路径电平**有喂**（record_streaming 传同一 audio_buf Arc；audio/mod.rs:359/:923 逐 chunk 写；重绘门 `needs_repaint||mic_has_audio` 已开 ~25fps；三 D2D 变体 + 两 GDI 分支全含弧代码）。🔴 截图橙色麦克风 ⇔ 同帧 has_audio=true ⇔ gain>0 ⇔ 弧已画——「代码没执行」不成立。唯一静态自洽假设=**非对称阈值冻结帧**（色阈值 0.01 vs 弧满亮 0.35 差 35 倍；静默间隙末帧=橙麦+gain 3~11% 不可见弧，随后重绘停止画面冻结）。**证伪条件**：说话中仍无弧 → 需弧级插桩（E5 探针）。
- **产出**：主文档 `docs/DIAG-163-BUILD159-ENDTEST.md`（含三问结论/证据链/修法建议 A/C/插桩方案，均标注不实施）；result.md 含收尾自证表。
- **红线**：`git diff --numstat -- src/` 唯一条目 7+/8- src/main.rs 为 coder-2 FIX-162 并行改动（hunk 仅 import + create_edit_control）；只读 git；cargo 未跑（纯静态+git 取证，无符号确认需求）；未出包；零凭证；临时文件已清理（/tmp/main_155.rs）。
- **交主控三个决策点**：① Q3 修法 A（Start 同步等重建）/C（文档声明现状）二选一，或先拿 debug.log 实锤；② Q1 是否批插桩诊断包；③ Q2 先用「说话中观察 3 秒」零成本复核，不行再插桩。

## 2026-09-07 — coder-1 — FIX-164 ✅ 端测三条一次修完（阶段一：src/main.rs 独占，1110P/0F + warnings 111/102 持平，待主控验收 → coder-2 TEST-SYNC → tester-1 回归出包）

- **Part A Q3（主修，方案 D 两段）**：D1 预热——worker 空闲 tick 轮询廉价层判定（模型身份+在线配置，不读词库，主控拍板），配置变更 ≤500ms 后台重载；失败签名防风暴（签名=模型+key/url/model；Start 主动路径不受限）。D2 兜底——Start 决策前廉价层命中且在途，最多等 1500ms，逐 ≤100ms 切片可被 stop/cancel 打断，超时/失败退回旧行为，硬红线不挂死。三处收口（asr_cheap_reload_needed / spawn_asr_reload / apply_reload_result）两路共用零漂移；RecordingStarted 时机零改动（HOTKEY-LATENCY-FIX-001 地盘）。
- **Part B Q2**：FULL_LEVEL 0.35→0.10 + 可见地板 0.35（外弧×0.85）；只改 mic_pulse_alphas 一处；gain<=0 ⇒ (0,0) 静音契约逐位不变。
- **Part C Q1**：菜单图标失败分支全量 log::warn!（带 GetLastError），成功一条 debug!；永久代码非探针；降级行为不变。
- **DEC-062** 已落 collab/decisions.md（部分修订 DEC-025 + 定性修正「功能性错误」）；MACOS-HANDOFF.md 已同步（A 两端同效；B/C Windows 专属不涉及）。
- **验证**：fmt 0 / check --all-targets warnings 111/102 持平 / cargo test 1110P/0F 零预期红；FIX-162 两 hunk 零触碰。
- **红线**：未 commit/未出包/版本未动/零凭证/临时文件无（无截图无脚本）；diff --numstat 328+/81- 全部归属本人改动+coder-2 既有回退。
- **给主控**：出包前目视清单（托盘右键一次看 debug.log 的 menu icon 行=Q1 取证；说话中看弧=Q2）；Q2 若端测仍无弧，弧级插桩（E5）待批。

## 2026-09-07 — coder-1 — DIAG-166 ✅ 托盘菜单图标查到底（工装实证定案 Q1 根因 + 守卫修复 +9/-3，待主控验收）

- **根因（定案，非候选）**：`create_menu_item_bitmap` 长度守卫 `n.checked_mul(4) != rgba.len()`——rgba 是 size²*4 的方形缓冲，比较值只有 4*size ⇒ size≥16 时恒判否 ⇒ 函数恒返 None ⇒ 图标从未创建/挂载。与 HMENU 归属（DIAG-163 已证伪）、MENUINFO/MNS_CHECKORBMP（本轮工装对照排除）、CreateDIBSection 失败（DC 对照排除）、SetMenuItemInfoW 失效（读回逐位相等排除）全无关——第 0 层：位图没造出来。
- **工装**：src/bin/diag166_menu_probe.rs 四组实验（现状复现/DC 对照/A 层读回/CHECKORBMP 对照），全 stdout 存档 docs/DIAG-166-TRAY-ICON.md，**工装文件已按红线删除**。
- **修复**：守卫改 size²*4 溢出安全链（+9/-3）；机制实证钉死才动手（Step 4 达成）；FIX-164 Part C 日志零触碰。
- **验证**：fmt 0 / warnings 111/102 持平 / cargo test 1110P/0F 连续 3 次全绿；中间一次 24F 不可复现（与工装删除+fmt 重叠窗口期可疑，无断言锚定被改行），已在主文档如实记录。
- **出包观察点（交主控/Gavin）**：右键托盘应见双图标品牌橙；若上下颠倒（DIBSECTION 读回 biHeight=+16 痕迹）→ 去掉 biHeight 负号一行即修。
- **红线**：未 commit/版本未动/未出包/零凭证；工装已清理。

## 2026-09-07 — coder-1 — FLICKER-170 ✅ 编辑态移光标闪烁 A+B' 双修（阶段一：80+/9-，1110P/0F、warnings 111/102 持平，待主控验收 → tester-1 出包）

- **A（放大器）**：父窗 WS_POPUP → WS_POPUP|WS_CLIPCHILDREN。修 INVESTIGATE-142 放大器（父窗全窗 BitBlt 盖 EDIT）；非编辑态无子窗口恒惰性、ULW 无 EDIT 惰性、其余三态零变化。回退=删 WS_CLIPCHILDREN。
- **B'（现象本体）**：EDIT 子类拦 WM_PAINT，WM_PRINTCLIENT(PRF_ERASEBKGND|PRF_CLIENT) 进内存 DC 一次画完擦除+文字，单次 BitBlt 提交 ps.rcPaint；caret 成对 Hide/Show；资源失败兜底回默认绘制。修 EDIT 内部两步直打表面的 DWM 合成间隙闪烁。回退=删 WM_PAINT 分支。
- **工装实证**：擦背景 ✓ / 文字 ✓ / 🔴 选区反白 ✗ 不渲染（0px diff ×3 组，Gavin 已接受）；caret/ps.rcPaint/IME 结论见 result.md。
- **预期现象分离（供 Gavin 端测判归因）**：A 修「进出编辑态/文字更新瞬间 EDIT 整块被盖掉后等 caret 周期回来」；B' 修「按住方向键移光标时最右侧文字闪烁」。只好一半时按此归因决定去留。
- **红线**：未 commit/版本未动/未出包/零凭证/工装已删；不碰 COMPOSITED/ULW/apply_alpha_fixup/圆角/RecordingStarted/预热逻辑。

## 2026-09-07 — coder-1 — FIX-172 ✅ 声波弧回归修复 + 编辑态最右侧闪烁根治（阶段一：46+/3-，1110P/0F、warnings 111/102 持平，待主控验收 → tester-1 出包）

- **FIX-172-A（声波弧回归）**：.max() 截断 → 抬高振幅 `amplitude=FLOOR+(1-FLOOR)*gain`；三条契约保住（谷底回 0/相位次序/静音零绘制）；只改 mic_pulse_alphas 一处。回退=恢复 .max() 两行。
- **FIX-172-B（最右侧闪烁）**：工装双实验定案——E1 证明滚动位块绕过 WM_PAINT 直打屏幕（变化 1894px/paints=0），E2 证明 SETREDRAW 包裹可把更新压成单次全客户区合成（变化=覆盖，TRUE 零额外重绘）。实施=子类消息尾包裹按键/字符/EM_SETSEL/点击消息（🔴 在 Enter 提交分支之后）。回退=删包裹块。
- **macOS**：mic_pulse_* 与 EDIT 子类均 cfg(windows) 专属，不涉及，MACOS-HANDOFF 无需同步。
- **预期现象分离（供端测归因）**：A 修「声波弧依次亮/1 秒一轮」；B 修「左右移光标最右侧文字闪烁」。只好一半按此去留。
- **红线**：未 commit/版本未动/未出包/零凭证/工装已删；FLICKER-170-A/B' 主体/选区/圆角/ULW/预热/托盘零触碰。

## 2026-09-07 — coder-1 — ESC-174 ✅ 编辑态 ESC 取消编辑与录入（阶段一：30+/0-，1111P/0F，待主控验收）

- **重点结论（压制链不卡窗，双顺序安全）**：CancelRequested 臂（:6537）自带完整收口——cancel/stop 双信号 + OVERLAY_EDITING=false + STREAMING_STOPPED=true + Hide + 托盘 Idle。worker 后续 Cancelled 到压制臂时 editing 已 false ⇒ else 幂等再 Hide。乱序场景（Cancelled 先到被压制吞掉）编辑窗保持、ESC 仍可收口。**「按 ESC 卡屏」不成立，无需拆单、无需先修压制**。
- **语义双满足**：关窗（F1 caret 清理+EDIT 销毁）+ 作废录入（cancel_signal→ASR 关 WS 丢文本；注入唯一通道 SubmitRequested 不经过 cancel）。
- **实施**：子类 WM_KEYDOWN 补 VK_ESCAPE 分支（同 Enter 分支父通道取 data，request.is_some() 守卫），位于 FIX-172-B 包裹块之前 return 0 无 SETREDRAW 停绘风险。
- **四条边界**：非编辑态零改动（EDIT 仅编辑态存在，父窗 :2226/FocusLost :2097 未动）；父子天然互斥+取消臂幂等；ESC/RETURN 无交叉；包裹块不参与。
- **验证**：fmt 0 / 0 error / cargo test 1111P/0F；warnings 118/109 中新增 7 条全在 menu_icons.rs = coder-2 EDITICON-175 在途改动（与本单无关），本单 main.rs 零新增。
- **Hide 双重压制静态缺陷**：绕开即可（理由见上），不动压制臂。
- **红线**：未 commit/版本未动/未出包/零凭证；macOS 不涉及。

## 2026-09-07 — coder-2 — EDITICON-175 ✅ 编辑小图标（铅笔）图标本体交付（只写 src/ui/menu_icons.rs，待主控验收 → 下一单集成）

- **交付**：`src/ui/menu_icons.rs` +99 行——`edit_icon_rgba(size)` 铅笔图标光栅化（SSAA 4x4、品牌橙、尖-杆缺口识别特征，18px/16px 均可辨）+ `dump_edit_icon_preview` 测试；预览 PNG 在 collab/outbox/coder-2/icons/（16/18/24 + x8，🔴 只证形态不证运行时）。
- **下一单集成依据**：图标盒子 18px@(6,9)（main.rs:4427-4429）；分割线规格=沿用现有左分割线 x=30/2px/20px 高/OVERLAY_BORDER_GRAY 0x3A3A3C（:4514-4529），图标右缘 24→线 30 间距 6px，线右缘 31→文字左缘 42 间距 11px。
- **注意**：main.rs 中 `edit_icon_rgba` 暂以 allow(dead_code) 压 warning（111/102 持平），集成后删 allow；cargo test 1111P/0F（+1 预览测试）；main.rs 零触碰（numstat +30 为 coder-1 ESC-174 在途）。

## 2026-09-07 — coder-2 — EDITICON-176 ✅ 集成完成：编辑态铅笔图标+分割线（D2D+GDI 兜底全覆盖，待主控验收 → 出包端测）

- **两条活产出路径都已覆盖**：D2D `draw_editing_overlay`（main.rs:4827 内 `edit_icon_and_left_separator`）+ GDI 兜底（:2903 调用 `draw_edit_icon_and_separator_gdi`）；`:3853 draw_editing_overlay_chrome` 是死代码未触碰。像素=175 验收的光栅化源（D2D 预乘 BGRA / GDI 直通 BGRA），无第二几何源。
- **验证**：fmt 幂等/check 0 error/warnings 111-102 持平/test 1112P-0F（+1 转换单测）；ESC-174 `@@ -948` hunk 零触碰。
- **待出包端测清单**：见 result.md 第七节（图标位置/大小/颜色、分割线、版式一致性、编辑态闪烁未回归、其余四态无变化）。
