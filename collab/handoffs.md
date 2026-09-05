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

## 2026-09-05 — tester-1 — TEST-FIX-091 + TEST-EXEC-092 ✅（4处测试修正 + 2条ignore跳过挂死项，回归全绿，待主控验收提交）

- **修正**：分隔线同坐标系比对 / reveal 反例同字数 begin变小三角窗口（old=anchored=2,drifted=1）/ advance_width 收窄 660 / 删重复 #[test]；两处二次修正（reveal 词表口径、advance_width 收敛预算算术）实测定位后改对
- **#[ignore] ×2（D2D-HANG-001，Gavin 授权）**：075 模块 P0 + 086 模块流式两态入口，均保留 #[test] 只加 ignore + 完整注释（主控定位卡在 CreateDCRenderTarget 之后，P0 待办非已解决）
- **回归**：root **1061P/0F/11I**（1054+9=1063，2 ignore→1061+11 对账）、src-tauri 76P、vitest 89P；两 REFACTOR 等价性判据全绿；消融 A/B 按 Gavin 拍板跳过
- **红线**：生产零改动（diff 仅测试模块）、v0.9.0 未动、未 commit、无残留进程、临时文件已删
- **详情**：outbox/tester-1/result.md + logs/20260904.md

## 2026-09-05 — tester-1 — BUILD-093 v0.9.0 出包 ✅（七项核验全过 + E2E 64P/1F 间歇 toggle_stop，主控裁定出包）

- **构建**：Step 1-4 顺序完整（npm 654ms → Tauri UI 1m43s → 主程序 2m12s → cp+toml 三副本）
- **核验**：①时间戳 09-05 ②sha256 三对相等且全异于 085（e6f55e0a/7651fd4e/0131ff55）③toml 三副本一致 ④ProductVersion 0.9.0.0 ⑤探针 D2D-P1×2+D2DERR_RECREATE_TARGET×1+index-CJ1JUYoT.js ⑥大小同量级 ⑦冒烟 PID 21680 Responding 无 panic
- **冒烟退出响应性**：CloseMainWindow=False 是 tray 无主窗口正常现象，非挂死；D2D-HANG-001 未复现
- **E2E**：64P/1F/33S。FAIL=test_hotkey_toggle_stop（**间歇性**：全量 FAIL、单跑 PASS/FAIL/PASS 交替）——第二次 F9 后 5s 内未离 recording。归因方向：OVERLAY-086 状态迁移 / D2D 绘制阻塞（D2D-HANG-001 关联）/ cold-start 时序残余。**主控裁定不阻塞出包，列为 Gavin 端测重点（toggle 连按两次）**
- **红线**：生产/测试零改动、v0.9.0 未动、运行时数据零覆盖、config.toml sha 3186ec8c 字节级无污染、临时文件已删
- **详情**：outbox/tester-1/result.md + logs/20260904.md

## 2026-09-05 — coder-2 — D2D-HANG-095 ✅ 根因修复落地（src/main.rs +23/-0，待主控验收）

- **改动 1**：`mod d2d` 新增 `release_resources()`（thread_local `D2D` 块后）：`take()` 就地 drop，仍在线程体内——COM Release 全部发生在 DLL_THREAD_DETACH 之前（探针 b2/b5 实证安全区）
- **改动 2**：`spawn_overlay_thread`（windows-only）闭包尾部调用 `d2d::release_resources()`；放闭包而非 `run_overlay_thread` 尾部=唯一覆盖 `?` 早返回路径（:1096/:1552）的位置
- **产出源盘点**（硬判据）：overlay 线程=唯一生产写入者已处理；`#[ignore]` 测试 :8842/:8883/:8889 解除后测试线程同样触达 thread_local → **tester-1 阶段三解除 ignore 时须在用例末尾调用 `d2d::release_resources()`**（这正是 D2D-HANG-001 挂死机制）；其余全部 thread::spawn 零触达（with_d2d 全仓唯一调用链已核实）
- **验证**：fmt clean / check --all-targets 0 error / 141 条 warning 无一条指向新增（grep 零命中）；diff 仅两 hunk +23/-0，测试模块零触碰
- **🔴 上报主控**：工作区 45 文件纯 CRLF 换行符改动为基线即脏（--ignore-cr-at-eol 后仅剩 4 文件实质改动），非本单引入，按 git 禁令未清理待定性
- **红线合规**：troubleshooting.md 未碰（tester-1 所有）/ 未动 D2dResources 字段序 / 未加 CoInitialize / 未动 with_d2d 与绘制原语 / 未加 join 超时 / 未 commit / v0.9.0 未动 / 零凭证 / 临时文件零生成
- **详情**：outbox/coder-2/result.md + logs/20260905.md + CHANGELOG.md + MACOS-HANDOFF §D2D-HANG-095
## 2026-09-05 — tester-1 — REPRO-094 ✅ 主控验收通过（D2D-HANG-001 根因定位，技术结论 + 本单补账）

- **Q1 确切阻塞 API**：`ID2D1SolidColorBrush::Release()`（thread_local 批量析构最后一步）在线程退出路径自锁死锁。
  证据：探针 B 逐对象 Drop 插桩停在 `[Drop 6/6]`（`repro094/probeB.log`）；bs4 挂死线程 `Rip=ntdll+0x161914`（等待原语），
  **`PEB+0x110 LoaderLock: LockCount=-2 RecursionCount=1 OwningThread=0x3764(=14180)=挂死线程 tid`**。
- **Q2 是否只在 cargo test 挂**：否。独立 exe 子线程同样挂（探针 b/b3/b6）；主线程显式 drop 不挂（探针 A，11ms）。
  探针 C：`--test-threads=1` 通过（0.11s），并行挂死在同一 `[Drop 6/6]`。
- **Q3 线程退出 COM Release 会不会阻塞**：会。探针 D 真实 exe（BUILD-093 feiyin-ime.exe，隔离 APPDATA，
  WM_QUIT 与托盘退出共用 shutdown 链）**5 次 4 挂**；挂死日志停在 `hook uninstalled` 后无 shutdown 后续
  （卡 `shutdown_and_join()` 的 `join()`）；run4 未挂 = 当次未画 D2D 迁移状态、槽为空（间歇性来源）。
- **H1-H4 判定**：H1 成立（自锁死锁）；H2 `CreateTextFormat`/字体缓存 不成立；H3 cargo test 并行 harness 竞争 不成立
  （独立 exe 单子线程零并行同样挂）；H4 缺 `CoInitializeEx` 不成立（b3 加 STA 照样挂）；**与释放顺序无关**
  （b6 反向字段序照样挂，b2/b5 线程体内任意顺序都不挂）。
- **建议修法**（供 coder-2 落地，已实现为 D2D-HANG-095）：overlay 线程收尾在**线程体内**显式清空 D2D 槽
  （`release_resources()`）；测试侧解除 `#[ignore]` 时须在用例尾部调用同一函数。
- **红线合规**：未改 `src/**`、`ui/**`；未 release 构建；未 commit；v0.9.0 未动；零凭证落盘；
  Publish/config.toml sha=3186ec8c 保持基线；Publish/debug.log 探针前备份（`repro094/debug.log.before`）后已恢复原样。
- **详情**：outbox/tester-1/result.md + logs/20260905.md + CHANGELOG.md + troubleshooting.md `[D2D-HANG-001]`

## 2026-09-05 — coder-2 — D2D-HANG-095-B ✅ Drop 守卫收尾（spawn_overlay_thread 单 hunk，待主控验收）

- **改动**：闭包尾部直调 `d2d::release_resources()` → `D2dReleaseGuard`（Drop impl）守卫，声明在 `run_overlay_thread` 之前；panic 展开路径照样释放，堵住 095-B 任务书指出的漏网路径
- **双重借用推理**：复核**成立**——`with_d2d` 的 RefMut 是同帧局部，panic 展开同帧局部逆序析构，借用归还先于守卫 drop；含 :3115 expect 本体情形结论不变；守卫声明最早析构最晚，不存在先于内层借用释放运行的场景 → 用 borrow_mut，不走 try_borrow_mut（不拿 abort 换 hang）
- **注释行号**：panic 点实测 :1475/:3115（首写偏 2 行已校正）
- **验证**：fmt exit 0 / check --all-targets 0 error / diff 相对 095 基线仅 spawn_overlay_thread 一个 hunk（vs HEAD 两 hunk 因 095 未 commit，已说明）
- **红线**：release_resources 本体未动 / 无 catch_unwind / with_d2d·原语·测试模块·troubleshooting.md 未碰 / 未 commit / v0.9.0 未动 / 零凭证
- **详情**：outbox/coder-2/result.md + logs/20260905.md + CHANGELOG.md + MACOS-HANDOFF §D2D-HANG-095 追加句
