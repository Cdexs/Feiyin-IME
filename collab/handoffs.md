# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-09-04 — tester-1 — TEST-FIX-080 阶段三 ✅ 主控验收通过（代码 PASS，文档同步由主控回填）

- **交付**：`+190/-42` 四文件，全部落在测试模块内（主控逐 hunk 核对行号：audio `mod tests` :1365 起／
  itn :2533 起／main `overlay_075_d2d_guard_tests` :7970 起／`.test.tsx` 纯追加）。**生产代码零改动属实**。
- **四条修正主控逐条对生产取证，全部成立**：①② ITN 时间族 —— `format_remainder_suffix` :1711-1729
  与 `format_time_chain` :1381-1398 实证 `H:MM`，同族既有护栏 :3460／:2630／:3549 三处佐证；
  ③ audio drain 重写 —— 旧用例 `Err(Empty)=>continue` 与生产 :322 `break` 不同构，`abandoned`
  结构性不可达；新用例慢消费建模对齐生产「deadline 靠 on_chunk 阻塞触发」；④ main.rs 消费序 ——
  生产 :3996-4013 实证「代际闸门 → 写镜像 → 043 仅裁渲染」，旧断言与自身闭包矛盾。
- **5 条 tsx 护栏**：所引生产行号 :160/:353/:369/:373/:387/:408 主控 `sed` 逐处核对全部对上。
  T3 拆 T3a（泄漏探针，删清 pending 行会红）+ T3b（正向回归）的判断主控复核认可，断言粒度不降反升。
- **主控独立验证**（验收上限内，未跑 test）：`cargo fmt --check` exit 0 ／
  `cargo check --all-targets` **0 error** ／ `npx tsc --noEmit` **0 error**。
- 🔴 **文档同步 3/5 缺，主控回填**：只写了 `logs/20260904.md`；CHANGELOG／handoffs／todo
  `grep` 零命中（`progress.md` 按其规则 7「测试同步任务不记录」判 **N/A**，不算漏）。
  `[DOC-STATE-DRIFT-001]` 第四次复发。叠加 Worker 重启把 `outbox/tester-1/result.md` 清成 0 字节
  （`[REPLACE-WORKER-TASKFILE-WIPED-001]`），**收尾自证表无从核对** —— 已把「五文档逐条打钩」
  写进阶段四任务书的完成判据，不再只靠通用规则。
- **结论复述**：ITN-071-B 生产修复是好的，`一点半点`／`一点点` 两条保护用例都绿，
  **不存在「071-B 没修彻底」**。红的只是两条护栏的期望值笔误。

---


## 2026-09-03 — coder-2 — HOTKEY-060 收尾补账 ✅（代码已在 9506eac/e7a6a29，本次零代码，待主控验收）

- **来源**：主控派单 Part A。08-30 主控 `git add -A` 误扫入 ITN 提交，未走验收流程，本次补齐账目
- **代码盘点（git 取证）**：`ui/src/pages/HotkeySettings.tsx` 9506eac +93/-65（finalizedRef 两侧拆分
  + reset 函数拆分 + 冲突检测收敛为共用 helper `applyHotkeyIfNoDupConflict` :154-195 +
  删除死常量 TRANSLATION_SINGLE_KEYS）；e7a6a29 -3 行（checkAndApplyVoiceHotkey 手写 finalized
  检查收敛进 helper）。调用点：语音 :204/:219/:497，翻译 :350
- **主控补审结论（转录）**：两侧独立 ✅ / 死代码删除等价非回归 ✅ / tsc 0 error ✅
- **红线合规**：纯文档零代码 / 未跑 test/build / 未 commit / 版本号 v0.9.0 未动 / UTF-8
- **详情**：outbox/coder-2/result.md + logs/20260903.md + CHANGELOG.md + progress.md + todo.md

## 2026-09-03 — coder-2 — OVERLAY-075 跨 session 渗漏隔离 ✅（src/main.rs +44/-5，待主控验收+阶段三）

- **根因**：STREAMING_STOPPED 布尔门闩表达不了会话身份。A 拖尾期间开 B → :3679 重置门闩 →
  A 迟到包畅通 → 宽窄拉锯 + A 旧文字渲染进 B 窗口（内容错误）
- **修法**：会话代际身份判断。StreamingText(u64, String, Vec<WordTiming>) +
  STREAMING_GENERATION AtomicU64：Start bump（:4180）→ 闭包捕获（:4286）→ 每包加戳（:4313）→
  消费闸门（:3694-3709，先于词库镜像）。STREAMING_STOPPED 未动（两闸正交）
- **产出源清单**：Windows 唯一产出 :4279（改前）；macOS 三处 1 字段签名（:4976/:5038/:7597 改前）
  = 051-G 遗留存量破损（macOS 此前必编不过），本批同步 3 字段修复，见 MACOS-HANDOFF §OVERLAY-075
- **实施自查纠错**：闸门第一版放在词库镜像后，会污染 WORDBOOK-053-B 学习数据，已移前
- **消融**：删代际闸门 → A 包经四格真值表「不忽略」→ 拉锯+内容错+镜像污染全复发
- **时序安全**：bump 先于本 session 任何事件发送；A 包先于 B bump 入队=合法尾包照常消费，两种交错均正确
- **验证**：fmt clean / check --all-targets 0 error（99 warnings 基线一致）/ git diff -w 仅 main.rs
- **红线合规**：043 语义未动 / bErase 未动 / 未改尺寸 / 未跑 test/build / 未 commit / 版本号未动 / UTF-8
- **详情**：outbox/coder-2/result.md + logs/20260903.md + CHANGELOG.md + progress.md + todo.md

---

## 2026-09-03 — tester-1 — REPRO-061-COLD ✅ 冷启动定向复测完成：判定 B 态（不算复现，强线索），待主控验收

- **判定**：主控收紧三态判据（测前锁定）下落 **B 态**——可见态 (0,0) 帧 5 次冷启动共 3199 帧 **0 命中**；隐藏态 (0,0) 帧 1870 帧（HWND 可枚举首帧即 (0,0) 1x1，隐藏期 ~600ms 全停 (0,0)）；1323 个可见帧 rect **全部**为终态 (1160,1292,240,36)
- **关键帧证据**：7 帧「rect 已在终态、IsWindowVisible 仍 FALSE」（先挪后显的直接实证），用户可见的永远是 SetWindowPos 之后的帧
- **窗口创建时机（主控要求单列）**：overlay HWND 进程启动后 **+190~+320ms** 已创建（6 次观测一致，含 sanity），热键时窗口已存在 ~570-620ms，**非懒创建**
- **方法**：C# 采样器（EnumWindows 类名 `voice-ime-overlay-window` + PID，src/main.rs:219）+ timeBeginPeriod(1) 校准（间隔 med ~1.5ms，首版 15ms 未达标数据已废弃重跑）；SendInput RightAlt(165) PushToTalk hold 400ms；每冷启动完整 taskkill→重启
- **数据**：outbox/tester-1/repro061-cold/run{1..5}.csv（逐帧 9 列）+ OverlaySampler.cs；md5 前后 19/19 一致；debug.log 仅被测程序自身追加，未用于判定
- **给主控的定向事实**：「(0,0) 创建」成立、「显示前被绘制」不成立——Gavin 看到的机制在 Win32 API 层采样覆盖不到的层（本单不延伸假设）

## 2026-09-03 — coder-1 — ASR-074-FIX ✅ 音频上行背压修复（audio/mod.rs + qwen_inference.rs + MACOS-HANDOFF.md，待主控验收）

**基线**：HEAD `e389289`，v0.9.0。证据源：`collab/evidence/debug-gavin-controlled-repro-20260903-0927.log`（Gavin 09:27 受控复现只读副本）。

**旧日志间接证据**：循环转速 65.2 Hz（903 chunks / 13.86s）< 100 Hz，丢失 351 chunks = 3.51s，finalize 拖尾 4.05s——三数字与主控独立推算逐位吻合。🔴 65.2 Hz 是倒推非证明，瓶颈在 read 还是 send 是最后一个未知数。

**永久埋点**（主控条件三，转永久 debug 级）：
- `[ASR-LOOP]` 每 1000 轮：循环转速 + read/send **分别** min/avg/p95/max（主控条件一）+ chunk_rx 积压
- `[ASR-BACKLOG]` 每 500ms：chunk_rx 积压深度
- `[ASR-DROP]` `log::warn!` 级别：队列满丢帧计数
- `percentile()` 辅助函数 + read/send 各自 samples 数组（cap 4096）

**Step 2-B**（`audio/mod.rs:299-340`）：stop_signal break 前 drain warm.rx，重采样后推 on_chunk。🔴 **时间上限 500ms（非数量上限）**：on_chunk 落到 chunk_tx.send() 是阻塞 send，ASR 线程卡住时 drain 无限等 = 把 ASR 故障传导到录音线程 = 松手后卡死（主控验收时发现的回归风险）。500ms deadline，超时放弃并 `[ASR-DROP]` warn 记录。

**Step 2-C**（`audio/mod.rs`）：三处 `let _ = tx_audio.try_send` → 计数 + `log::warn!`。`WarmInputStream` 新增 `dropped_chunks: Arc<AtomicU64>` 永久字段。完成日志追加 `dropped_chunks=N`。

**Step 2-A**（`qwen_inference.rs:1388-1480`）：每轮排空 chunk_rx 最多 N=16 chunks，合并成一次 send。N=16 理由见 result.md。🔴 若瓶颈在 send：排空治标不治本，下一步拆 send 到独立线程（tungstenite WebSocket 非 Send，需架构改动）。

**三个小缺陷**：① first_audio_byte_ms 主循环首帧 send 处补赋值 ② outcome 赋值挪到 format_summary 之前（7 处退出路径） ③ format_summary 去重。

**验证**：cargo fmt clean / cargo check --all-targets **0 error**（101 warnings，93 duplicates，既有 99 + coder-2 D2D 新增 2，均非本任务）。验证时间：coder-2 恢复 main.rs 编译后一次性跑完。

**红线合规**：版本号 v0.9.0 未动 / 未写入 main.rs / 未 commit / 未碰运行时数据 / UTF-8（bash heredoc + Edit）/ 禁用 git 破坏性命令。macOS 影响：MACOS-HANDOFF §0.2，零编译影响零行为回归。

## 2026-09-03 — coder-2 — ASR-074-GUARD ✅ chunk_tx 超时防卡死（单点，待主控验收）

- **背景**：主控验收 ASR-074 发现跨文件域残留风险——drain 循环 500ms deadline 在循环头判断，
  单次 on_chunk→chunk_tx.send()（bounded 无超时阻塞）卡住即回不去；ASR 停死→队满→录音线程
  不返回→松手卡死
- **改动**：send_timeout(200ms)；Timeout→[ASR-DROP] warn + ASR_CHUNK_DROPS 计数；
  Disconnected 静默。200ms=健康消费 20 倍余量
- **边界**：仅 src/main.rs（on_chunk 闭包 + 新 static）；audio/qwen_inference 未碰；容量 256 未动
- **验证**：fmt clean / check --all-targets 0 error（99 warnings 基线持平）
- **详情**：outbox/coder-2/result.md + logs/20260903.md + CHANGELOG.md

---

## 2026-09-03 — tester-1 — TEST-SYNC-074/075/D2D ✅ 阶段三 14 用例交付（纯追加负增量0，fmt/check 过，待主控验收）

- **交付**：src/main.rs +211（新模块 overlay_075_d2d_guard_tests 5 用例：075 领号协议/镜像污染/首字段三元 + GUARD 三分支/计数器 + D2D 回落触发器）、src/audio/mod.rs +154（Step 2-B drain 四用例）、src/transcription/qwen_inference.rs +233（批量边界 ×2 + 缺陷1/2/3 ×3）
- **消融**：每条用例附「改回旧实现会不会红」推演（result.md 逐条表）
- **覆盖缺口 5 项如实声明**（真实 WS 帧序/完整 match arm/真实 DC 成功路径/WASAPI 回调内计数/真实慢消费 abandoned 精确值），阶段五端测建议已附
- **自查**：fmt --check clean；check --all-targets 0 error；git diff -w 纯增量；既有 228 用例零触碰
- **注意**：阶段三未跑用例（红线）；阶段四执行时新用例随 `cargo test` 生效，其中 main.rs 新模块带 `#[cfg(all(test, target_os="windows"))]`（Windows-only statics 依赖）

## 2026-09-03 夜 — tester-1 — TEST-EXEC-076 阶段四全量回归 ✅（源码层纯执行，5 FAIL 原样上报，待主控验收+裁定）

- **范围**：Step 1 两 crate cargo test + Step 2 vitest + 必查A/B/C；pytest 系列按任务书 SKIP（08-18 旧包）
- **净结果**：root 1050P/4F/9I ｜ src-tauri 76P 全绿 ｜ vitest 83P/1F（HotkeySettings 23/24）
- **5 FAIL 全部完整取证**：①②itn_071b×2（一点半→「下午1:30」非「1点半」，疑生产 071-B 不彻底）③asr_074 abandoned 5/5 稳定红（测试自建循环语义≠生产 Empty=>break，疑似测试设计缺陷）④stale_generation（断言 vs 生产 :4006 镜像先写冲突，053-B/075 语义裁定）⑤S12（AltGr 弹窗键名 Left Ctrl≠Right Alt，拦截行为正确）
- **必查A**：guard 门控模块 --list 实证 6 条非 0 ｜ **必查B**：228+15（git 物理实提交 15 个 #[test]，文档「14」是语义组口径，差额=1 已说明）｜ **必查C**：挂钟用例连跑 5 次全红 0.90-0.91s
- **红线合规**：零生产/零用例改动、零出包、零 commit、版本号未动、纯 bash 追加文档
- **详情**：outbox/tester-1/result.md（含裁定请求表：③建议改测试循环对齐生产；④⑤①②需主控裁定改哪侧）

## 2026-09-03 — coder-2 — HOTKEY-078 ✅ AltGr 尾随 keyup 串键修复（HotkeySettings.tsx -3+5，待主控验收）

- **根因（独立复核与主控推演一致）**：`handleVoiceHotkeyKeyUp` :287-289 在 AltRight keyUp
  提前清 `altGrSynthCtrlActiveRef`，尾随合成 ControlLeft keyUp 逃过 :279（现 :284）抑制 →
  第二次 `checkAndApplyVoiceHotkey(0xA2,0)`；async 闸门（await invoke 后才置 finalizedRef）
  挡不住同步连续 keyUp；available=false else 分支 :213-216 不置 finalized 反而 reset →
  `setPendingHotkey({vk:0xA2})` 覆盖 → S12 冲突弹窗错显 Left Ctrl
- **修法**：删 3 行提前清旗，旗唯一清零点回归 `resetVoiceRecordingState()`（:141，会话级生命周期）；
  +5 行注释。非回归五场景独立复核全过（wasActive 捕获序不变/AltRight 先抬修好/ControlLeft 先抬
  行为不变/非 AltGr 无变化/同会话双保险）
- **验证**：npx tsc --noEmit 0 error；git diff -w 精确 -3+5 注释仅此一文件；S12 测试零触碰；
  未跑 cargo build/npm run test（tester-1 职责）；未 commit；v0.9.0 未动
- **Part B 取证（等主控裁定）**：①翻译侧推演成立——handleTranslationHotkeyKeyDown(:335-361)
  无修饰键过滤/无 AltGr 旗/无 keyUp 处理器（:466-467），AltGr 首事件 keyDown ControlLeft
  :344 查表 0xA2 → :350 全同步直调 finalize → 翻译热键被录成 Left Ctrl，连 async 重入窗口
  都不存在，无用例覆盖故未红；②修法=镜像语音侧键序生命周期（结构性改动需配套用例），
  建议另开单，不碰 HOTKEY-060 helper 契约；③同步闸门同意不做，补充反论：入口置 finalized
  会破坏 catch 回退路径（:218-228 invoke 异常时热键永远写不进去）
- **附加发现备案**：Alt 先抬 Ctrl 后抬后单按 Left Ctrl 被 :284 误抑制至 Escape 重进；
  修前错录 Left Ctrl、修后静默忽略，属模糊歧义键序更安全取舍
- **红线合规**：仅 HotkeySettings.tsx / 测试零触碰 / 未 commit / 版本号未动 / UTF-8（Edit 工具）/
  零临时文件 / MACOS-HANDOFF §HOTKEY-078 已记
- **详情**：outbox/coder-2/result.md + logs/20260903.md + CHANGELOG.md

## 2026-09-03 — coder-2 — HOTKEY-079 ✅ 翻译侧 AltGr 串键修复（HotkeySettings.tsx +60，待主控验收）

- **方案评估**：同意 Plan A（只对 ControlLeft 延后裁决），无反对。翻译侧单键语义
  （translationKeySet 单 vk 无 modifiers），Plan B 搬语音侧组合键机制会引入用不上的
  pressedMods 状态且 078 刚证明该机制有生命周期陷阱
- **实施**：①translationPendingCtrlRef 翻译侧独占（HOTKEY-060 红线）②keyDown 插两分支：
  ControlLeft→pending=true+return 不 finalize；AltRight 且 pending→清+录 0xA5（AltGr 接管）
  ③新 handleTranslationHotkeyKeyUp 挂 onKeyUp：ControlLeft 且 pending 仍 true→录 0xA2
  （真单按 Ctrl），其余 return ④resetTranslationRecordingState 清 pending（唯一清零点，
  078 同型陷阱预防）⑤除 ControlLeft 外任何键 finalize 时机与产出 vk 一字不变
- **行为差异复核**：主控差异表漏三和弦场景已复核补全——Ctrl 按住+AltGr 录 Right Alt
  （与单按 AltGr 同路径）。🔴 **另两行 coder-2 写错、主控验收时更正**：Ctrl 按住+F5 与
  Ctrl 按住+Right Ctrl **修前都录 Left Ctrl 不是录第二个键** —— 修前 keyDown 在第一个键
  就 finalize，其 `setRecording(false)` 会把聆听态 div 换成按钮、**监听器随之卸载**，
  第二个键的 keyDown 到不了 handler，不存在「先到先得」。故本次行为变化是**三处**不是一处：
  ①ControlLeft 单键 finalize 时机 ②Ctrl+任意非 Ctrl 键的产出键 ③Ctrl+AltGr 的产出键。
  **主控裁定三处全部接受不返工**（翻译侧单键语义下「第一个键定局」才是反直觉的一侧）。
  主控另补一条真实代价：按下 Ctrl 后抬起前被夺焦 → onBlur 清 pending → 本次零录入
  （修前已录 Left Ctrl），判定可接受，阶段三不必写用例。详见 logs/20260903.md 同节
- **用例需求 5 条已交 result.md**：T1 翻译 AltGr→Right Alt（本单核心）/ T2 单按 Ctrl
  回归护栏 / T3 pending 清零防污染 / T4 语音侧键序矩阵 T4a AltRight 先抬+T4b ControlLeft
  先抬（各附消融）/ T5 旗残留键序**定案降级**——严格推演不存在「会话存活+旗残留+能单按
  Left Ctrl」的可执行键序（旗置位仅 :259、清零 reset :150 被 Escape/onBlur/finalize 全量
  调用；唯一持续窗口是两键都不抬=用户还按着 AltGr 本身），不要求写用例，上单「附加发现」
  备案按此修正，不再模糊流转
- **验证**：tsc 0 error；diff +60/-0 仅此一文件；测试零触碰；未跑 cargo build/npm test；
  未 commit；v0.9.0 未动；UTF-8（Edit 工具）
- **红线合规**：MACOS-HANDOFF §HOTKEY-079 已记（零编译影响/零行为差异/macOS 无 AltGr 键序）
- **详情**：outbox/coder-2/result.md + logs/20260903.md + CHANGELOG.md

## 2026-09-04 — coder-1 — SECRET-082 ✅ pre-push 闸门 + CRLF 隐患修复（4 文件，零生产代码，待主控验收）

**基线**：`ace4a5c`，v0.9.0（未动）

**交付物**：
- `scripts/git-hooks/secret-patterns.sh`（新增，共享模式表 + scan_diff 函数）
- `scripts/git-hooks/pre-push`（新增，覆盖三种 ref 情况）
- `scripts/git-hooks/pre-commit`（改为 source 共享文件，行为不变）
- `.gitattributes`（新增，`scripts/git-hooks/** text eol=lf`）

**协作文档更新**：
- `collab/troubleshooting.md [SECRET-IN-REPO-001]` 追加 pre-push 闸门 + CRLF 隐患记录
- `collab/docs/worker-guide.md` 加「每个新 clone 执行 git config core.hooksPath」提示
- `logs/20260904.md` + `CHANGELOG.md` 已追加

**实测**：A1-A4 拦截 / B1-B3 放行 / C1-C2 / D / E / pre-push 拦截 / F 本仓 dry-run 放行，全绿
**临时仓库**：`/c/msys64/tmp/opencode/secret082/tmprepo/` 测完 `rm -rf` 已删
**红线**：零生产代码改动；未 commit、未 push；版本号未动

## 2026-09-04 — coder-1 — SECRET-082-FIX ✅ pre-push 根提交+fail-open 修复（pre-push/pre-commit，待主控验收）

**基线**：SECRET-082 工作区改动（未提交），v0.9.0

**修复**：
- 缺陷一：根提交无父时 `${FIRST}^` 解析失败 → 空树 base + `git rev-parse --verify -q` fallback
- 缺陷二：fail-open → fail-closed，git diff 失败时拒绝放行并打印报错（pre-push + pre-commit 一并收敛）

**实测**：A1-A4 拦 / B1-B3 放 / C1-C5 / D / E / F 本仓 dry-run 全绿（C3 根提交+密钥被拦，C4 根提交干净放行，C5 无效 sha fail-closed）
**协作文档**：troubleshooting 新开 [HOOK-FAIL-OPEN-001] + logs + CHANGELOG + handoffs
**临时仓库**：测完 rm -rf 已删

## 2026-09-04 — tester-1 — TEST-EXEC-081 阶段四全量回归 ✅（1054/76/89 逐位吻合，消融A偏差裁定后收口）

- **Step 1**：root 1054P/0F/9I（4 条修红转绿，与模型一致）；src-tauri 76P 持平
- **必查 A**：四条逐条转绿，两个新名生效（旧名 gives_up…/stale_generation… 零命中）
- **必查 B**：挂钟用例 5/5 绿（0.50/0.50/0.51/0.50/0.51s，deadline 闸门 ~500ms 生效）
- **必查 C**：S12 红转绿 + T1/T2/T3a/T3b/T4b 五条新护栏全绿（vitest 89P/0F）
- **消融 B**：079 撤销 → T1 变红（received 0xA2）预期命中；**消融 A**：078 撤销 → T4b 仍绿、S12 变红（弹窗显示 Left Ctrl = 原始症状精确复现）→ 停手报告 → **主控裁定归因成立**（清旗在 altGrSynthWasActive 捕获之后），判红基准改 S12，**本体验收通过**
- pytest 系列按任务书 SKIP（Publish/ 为 08-18 旧包，E2E 门禁挪阶段五出包后）
- 两处消融均还原，`git diff -w` 0 字节；详情 outbox/tester-1/result.md + logs/20260904.md

## 2026-09-04 — tester-1 — TEST-FIX-084 T4b 消融注释改正 ✅（阶段四收口）

- **改动**：`ui/src/pages/HotkeySettings.test.tsx` T4b 消融注释 +5/-3 仅注释（结构性不红事实 + 078 判别力由 S12 承担 + T4b 保留职责守 ControlLeft 先抬半边矩阵）；it() 断言/键序/用例名零触碰
- **另 4 条复核**：T1/T2/T3a/T3b 消融推演事件顺序 vs 用例 fireEvent 顺序逐条核对全部成立（T3a 的 :160/:408/:529 行号核实、T3b 自陈「删 :160 仍绿」推演正确），零凑数改动
- **验证**：tsc 0 error（白名单内）；未跑 npm/cargo test；未 commit；v0.9.0 未动
- **详情**：outbox/tester-1/result.md + logs/20260904.md

## 2026-09-04 — tester-1 — BUILD-085 v0.9.0 阶段五出包 ✅（七项核验全过 + E2E 65P/0F，待主控验收+Gavin 端测）

- **构建**：Step 1-4 顺序完整执行（npm 1.46s → Tauri UI 2m03s 含 custom-protocol → 主程序 2m42s → cp+toml 三副本同步）
- **核验**：①时间戳 09-04 ②sha256 三对相等 ③toml 三副本 hash 一致（scene `0a3a0b9a…`/itn `311cbb96…`）④**ProductVersion 0.9.0.0** ⑤探针 ASR-DROP×4/ASR-LOOP×1 + index-CJ1JUYoT.js ⑥大小同量级（main +29KB 预期内）⑦冒烟 PID 25288 Responding 无 panic
- **E2E 门禁**：65P/0F/33S（175s）与 BUILD-022 逐位一致，两条已知坑未命中，config.toml 双侧 sha256 字节级相等无污染
- **运行时数据**：config.toml/wordbook.sqlite/debug.log 零覆盖；version_check.json mtime 变化已定性=程序冒烟自写缓存（src/version_check/mod.rs save_cache），非出包覆盖
- 产物：`Publish/` 三 exe 就绪，待 Gavin 端测（DEC-055 红线 5 目视 / AltGr 双侧 / LLM 401 定性提示见 result.md）
- **详情**：outbox/tester-1/result.md + logs/20260904.md

## 2026-09-04 — coder-2 — OVERLAY-086 ✅ 三 overlay 缺陷修复（main.rs +213/-52 + qwen_inference.rs +10/-2，代码验收通过，含打回整改）

- **Bug1 圆角灰线**：D2D 背景与辉光 `FillRoundedRectangle`（corner_radius 单绑定共享，0.5px 笔偏移保留）+ GDI 兜底 `FillRgn(CreateRoundRectRgn 16*2)` + Processing region `Some(16)`。DEC-056 下取「楔形被裁」优于「楔形外露」，端测不偏好可一行回退 None
- **Bug2 空窗口真根因**：realtime on_result 无空过滤 → VAD 预热 21 空包/170ms → Show(RecordingWithText{text:""}) 空窗口 + WORDBOOK-053-B 镜像被空串抹掉。①源头 `!display.is_empty()` 闸门（qwen_inference.rs:1579）②渲染层空文本回落 placeholder（不变量）③`tween_timeline_origin` 与墙钟零点同锚，`reveal_chars_by_timeline` 加 Option<i64> 第4参（None=旧行为，11 处护栏零语义改动）——词表重写（is→试 重排、origin 1120→1160）冻结/爆发成因消除
- **Bug3 宽度瞬移**：变宽改插值（删 grow-snap）+ 删 068-B snap + Show 流式 SetWindowPos 用 in-flight current_size——两处 SetWindowPos 尺寸同源（current），068-B 防争抢结构性保留，046 非流式零变，068-A R1 居中保留=两边扩展
- **打回整改（主控验收两处，均已处理）**：① `:1249` 068-A applied_size 的 if/else 删除，统一 `state.current_size`（A 方案）——变宽帧原按 desired 算居中、窗口按 current 画，左缘偏 `(desired-current)/2` 一帧；现在 :1249/:1366/R1 三处同源。② 插值循环注释「reveal now follows the interpolated width」是代码不存在的承诺，已改事实表述（scroll_x 在已画宽度内滚动 + target 只走节流 Show 路径 + reveal 纯时间轴驱动，无窗口宽度耦合）
- **验证（主控独立复跑）**：fmt --check exit 0 / check --all-targets 0 error / diff -w 仅 2 生产文件 + 3 文档 / 测试文件零触碰 / v0.9.0 未动
- **非回归结论**：043/046/051-G/068-A/068-B/075/D2D-073/053-B/DEC-056 逐项点名全过（result.md 各节），interpolate_step 零改动、bErase=false 未动
- **红线合规**：未跑 test/build / 未 commit / UTF-8（bash heredoc + Edit + py -3.11 codecs）/ 零凭证 / 建议护栏 4 组交阶段三
- **详情**：outbox/coder-2/result.md（含打回整改记录节）+ logs/20260904.md + CHANGELOG.md + MACOS-HANDOFF §OVERLAY-086

## 2026-09-04 — coder-2 — D2D-P1 ✅ 流式文字两态迁 D2D（main.rs +490/-33，待主控验收）

- **共用层三层化**：资源层（D2dResources + streaming_text_format Segoe UI/normal/14px/LEADING）→ 帧层 `with_d2d`（BindDC/Begin/End/RECREATE 集中）→ 原语层 chrome/mic_indicator/stop_button/placeholder_text/streaming_text + colorref_to_d2d。P2/P3 只写原语调用
- **D2DERR_RECREATE_TARGET**（0x8899000C）：EndDraw 失败判 code → 丢 thread_local 资源 + false → GDI 当帧兜底 → 下帧重建（休眠唤醒/驱动更新/RDP）
- **P0 机械包装**（裁定采纳）：draw_processing_overlay 走 with_d2d，原语改 draw_processing_primitives——与 HEAD 旧 draw_with 核心区 109 非注释行逐行 diff=0（机器比对），RECREATE 修复覆盖处理中态
- **两态迁移**：RecordingStreamingIdle（D2D chrome+mic+placeholder+stop）/ RecordingWithText（D2D chrome+mic+PushAxisAlignedClip 滚动裁剪文字+stop），GDI 兜底保留；Recording 波形态未迁（红线 4）
- **度量单一源**（裁定③）：窗口定宽与 scroll_x 全用 GDI measure_text_width，D2D 侧零 DirectWrite 度量，零双度量漂移
- **逐原语对照表**：result.md §二（含两处有意差异：背景圆角=086 教训前置、占位 LEADING=与流式连续；两处遗漏如实声明：右分隔线缺、超宽无省略号）
- **非回归**：043/046/051-A/051-G/068-A/075/086/D2D-073-P0/GDI 兜底契约逐项点名全过（result.md §三）；P0 原语零 diff；空文本护栏路径复验（空文本→D2D idle 画 placeholder）
- **验收整改**：右分隔线已补（D2D 流式态 DrawLine x=w-36/2px/高20，照抄 GDI :2617；此前「常态缺线、回落才有」回归已消除）；#4 省略号按主控裁定改写为「有意行为变更」（超宽裁剪无省略号：触发罕见 + 右端省略号会盖最新文字，请 Gavin 端测知悉判断）
- **验证**：fmt clean / check --all-targets 0 error（自跑）/ git diff -w 仅 main.rs / interpolate_step 零改动 / 版本号 v0.9.0 未动 / 未跑 test/build / 未 commit / UTF-8（Edit 工具）/ 零凭证 / MACOS-HANDOFF §D2D-P1 已记
- **详情**：outbox/coder-2/result.md + logs/20260904.md + CHANGELOG.md

## 2026-09-04 — coder-2 — REFACTOR-088 ✅ 抽 streaming_scroll_offset 纯函数（main.rs +函数+2调用替换，待主控验收）

- **动机**：scroll_x 公式内联两处（GDI :2576 / D2D :3564），单一实现防静默漂移（GDI 兜底路径平时不可见，分叉等回落才炸）
- **签名**：`streaming_scroll_offset(text_width: i32, visible_w: i32) -> i32`（#[cfg(windows)]）
- **f32/i32 处置**：D2D 调用点 `visible_w as i32` 传参 + 结果 `as f32` 回转——等价性链条（w 整数值 f32 / margin i32 常量 / visible_w=w-91.0 整数值 / 屏宽≪2^24 无舍入 → 截断永不发生）逐环主控复核通过，全文写入 doc-comment；断裂条件（margin 非整数/窗口宽带小数）同文标注
- **归属边界**：工作区 mod overlay_086_d2d_p1_guard_tests ≈380 行 = tester-1 TEST-SYNC-087 交付物非本单；本单 diff 仅新函数+2 调用替换；tester-1 的 streaming_scroll_offset_contract 直测本函数签名逐字匹配
- **验证**：fmt clean / check 0 error（主控独立复跑）/ 版本号未动 / 未跑 test/build / 未 commit / UTF-8 / 零凭证
- **详情**：outbox/coder-2/result.md + logs/20260904.md + CHANGELOG.md + MACOS-HANDOFF §REFACTOR-088

## 2026-09-04 — coder-2 — REFACTOR-089 ✅ 抽 advance_width 纯函数（main.rs 插值区+新函数，待主控验收）

- **动机**：护栏 7（变宽必须插值）消融对象是内联分支，用例无法真实调用 → 判别力 0；OVERLAY-046 先例证明「修复被重构静默回退」的代价，Gavin 痛点最大的 Bug 3 值得真护栏
- **签名**：`advance_width(current: i32, target: i32) -> i32`（#[cfg(windows)]，含吸附 + 完整等价性 doc-comment + 消融参考）
- **等价性**：两轴独立（吸附只读本轴字段）+ d==0 no-op 等值 + |d|=1/2 边界同值，主控逐环复核；外层守卫与 size_interpolation_done 置位时机零改动
- **验证**：fmt clean / check 0 error / interpolate_step 零改动 / 086/D2D-P1/088 成果零回退 / 版本号未动 / 未跑 test/build / 未 commit / UTF-8 / 零凭证
- **hunk 归属**：我=插值区 :1715-1731 + 新函数 :4271-4326；tester-1 TEST-SYNC-087 :8823+378 不在本单
- **详情**：outbox/coder-2/result.md + logs/20260904.md + CHANGELOG.md + MACOS-HANDOFF §REFACTOR-089

## 2026-09-04 — tester-1 — TEST-SYNC-087 阶段三测试同步 ✅（九护栏全覆盖：6 真判别力 + 3 缺口如实声明，fmt/check 过，待主控验收）

- **交付**：src/main.rs `mod overlay_086_d2d_p1_guard_tests` 9 用例 + 2 helper（≈400 行测试区）
- **真护栏 6 条**：流式两态 D2D 入口无效 HDC→false（护栏1）/ RECREATE_TARGET 权威常量（护栏3）/ reveal 第4参数双行为 None 逐位+Some(fixed) 不倒退（护栏5×2）/ **advance_width 直调变宽插值**（护栏7，REFACTOR-089 后消融改回 snap 必红：240→290 而非 440）/ **streaming_scroll_offset 直调**（护栏2，REFACTOR-088 后）/ 分隔线几何两路径同口径（护栏9）
- **判别力缺口 3 条**（单列成节，裁定 B 口径）：护栏6 空文本路由（std 方法+内联分支）/ 护栏8 居中同源（单行赋值无物可抽）/ 源头闸门（耦合 WS）；各自给补法，未造假推演
- **不可测项 7 条**：D2D 视觉效果全归 Gavin 端测目视（DEC-055 红线 5）
- **过程**：三轮协商（护栏2可测性→主控裁B；E0425 并行中间态误报按红线只报不动；**护栏7/8 假护栏交付前自审**——判别力区分=消融对象是否生产真函数，主控裁定 7走A/8·6走B，已按裁定落地）
- **红线合规**：既有用例/11 处 None 直调/interpolate_step 零触碰；生产代码我侧零改动；fmt --check exit 0 / check --all-targets 0 error；cargo test/build 未跑；v0.9.0 未动；零临时文件
- **详情**：outbox/tester-1/result.md（含判别力缺口独立节 + 消融推演与顺序自证表）+ logs/20260904.md
