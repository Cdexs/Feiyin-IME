# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-09-06 — coder-1 — HOTKEY-115 ✅ Toggle 停不住双缺陷修复（hotkey.rs 9 hunks + main.rs 1 行扩单收口，待主控验收）

- **缺陷 1**：钩子 KEYUP 分支 `PTT_ACTIVE.store(false)` 在 `if should_stop…` 外，Toggle 松键被重置 → 第二次 DOWN 恒发 Start（Stop 分支不可达）。修：store 移进 if 内（PTT 行为逐位等价）+ DOWN 按模式分支。
- **缺陷 2**：RegisterHotKey Toggle 分支恒发 Start（无状态，靠控制器兜底生存）。修：`TOGGLE_ACTIVE.swap` 翻转。
- **选型**：另开 `TOGGLE_ACTIVE`（PTT_ACTIVE 被 poll_ptt_release_thread 自旋消费，复用两生命周期打架）；B1 由按模式分支构造性成立。
- **互斥证据（主控要求实证）**：handle_hotkey_trigger:574 uses_hook 早退 / 钩子仅 CURRENT_HOOK 非空时被调 / sync_binding 清理→归零→安装顺序 / 中间态两路径都不产事件。
- **B3 单一收口**：控制器 8 处结束路径汇聚 `notify_translate_poll_stop()` → 该函数内复位 TOGGLE_ACTIVE（零新增分发）；「有时」= 兜底依赖 is_recording 时序窗口 + 外部结束后的反转，真 Stop 无条件置信号后竞态消除。
- **主控扩单③**：main.rs:5101 mic-muted 拒绝出口补 1 行 notify（Start 唯一无 pipeline 事件出口，否则态反转）。
- **B4**：install/uninstall/sync_binding 三处两态归零。
- **教训(六)**：给 tester-1 的护栏建议 = include_str 结构护栏守调用点（store 在 if 内 / 分支 atom 归属 / 收口含复位），不再造测纯函数的假护栏。
- **报备**：钩子路径 auto-repeat 长按 Toggle 翻转新旧持平（无回归），去抖另立单；macOS 缺陷②同源已写 MACOS-HANDOFF.md。
- **验证**：fmt --check 0 / check --all-targets 0 error / warning stash 对照与基线持平（111/102）/ 未 commit / v0.9.0 未动 / 零凭证。
- **详情**：outbox/coder-1/result.md + logs/20260906.md + CHANGELOG.md

## 2026-09-06 — tester-1 — REPRO-114 ✅ 🔴 P0 热键第二下按不停根因定位（只查不修，生产零改动）

- **根因锁死**：`src/platform/windows/hotkey.rs:221` WM_KEYUP 分支 `PTT_ACTIVE.store(false)` **无条件执行**，Toggle 松键重置标志 → 第二下按下又发 `Start` 而非 `Stop`。RegisterHotKey 路径（F9）`:536-547` 恒发 Start 无 toggle 状态。
- **H1 证伪**：probe1/2 实测回调 60-83us（默认 1000ms 超时 1/10000），loader-lock 压力下正常；probe3 逐句镜像生产 PTT_ACTIVE 逻辑决定性复现（两次 DOWN 都 START）。
- **真实 app 实证**：双按（gap 1000ms/20ms）第二下恒 Start、overlay 卡 recording ~20s；单按 5/5 可靠。
- **建议修法（交 coder）**：① `:221` 移入 `if should_stop_translate_poll_on_keyup(mode)` 块内；② `:536` Toggle 分支加 PTT_ACTIVE toggle 状态。
- **红线**：生产零改动 / 未出包 / 未 commit / v0.9.0 未动 / config.toml sha 3186ec8c / 探针工程保留 repro114/ / 零凭证。
- **详情**：outbox/tester-1/result.md + logs/20260906.md + CHANGELOG.md

## 2026-09-05 — tester-1 — TEST-EXEC-111 ✅ 阶段四（三层全绿 + A1-A5 消融 + 🔴 热键环境级失效证据，G4 重设计保留）

- **Step1**：root **1077P/0F/9I**（1070+7）/ src-tauri 76P / vitest 89P，三层全绿。
- **消融**：A1→G1 红；A2→G2a 红；A3→G3a 红；**A4 初版 G4 未红（8 行窗口误扫 else 分支 helper）→ 重设计只扫 if-body 后基线绿/消融红**；A5 注释线程体内释放→**整个 test 进程挂死 timeout exit 124（D2D-HANG-001 本体）**。
- **Step3 🔴 决定性发现**：当前环境**热键模拟整体失效**（全 6 条 hotkey FAIL，非仅预暖竞态）。证据链：非代码回归（旧包同失败）/ SendInput 对 notepad 正常但到不了 app 的 WH_KEYBOARD_LL 钩子 / detached 同失败 / app 完全响应 / 时间线显示 ~23:00 过、~23:50 全失败=环境退化。**根因在 Windows 输入层，tests/** 无 API 可修**。已提选项 A（门禁显式豁免）/选项 B（出包接受门禁红、主控逐次显式裁定，**强烈建议 B**）。
- **还原自证**：A1-A5 全还原（ABLATION-A=0）；root 重跑 1077P/0F/9I；G1-G4 单跑 7P；无残留进程；config.toml sha 3186ec8c；target/release config 恢复 vk=165。唯一 src/main.rs 残留 diff = G4 重设计（28/13 全测试模块，非消融残留）。
- **红线**：git-hooks 未碰 / 未出包 / 未 commit / v0.9.0 未动 / 零凭证。
- **详情**：outbox/tester-1/result.md + logs/20260905.md + CHANGELOG.md

## 2026-09-05 — coder-1 — SECRET-105 ✅ 密钥闸门 diff 标记误报+漏报双修（只动 scripts/git-hooks/ 3 文件，待主控验收）

- **根因**：`scan_diff` 输入为 git diff 原始行，行首 `+` 标记未剥离；邮箱 local-part 类含 `+`，标记被当 local-part → `<+>@pytest.hookimpl` 全中招（SECRET-082/104 同族）。
- **修 1**：`scan_diff` 入口 `sed 's/^[+-]//'` 剥一个标记（一处修全类受益；行数 1:1 行号不变；真阳性实测全保留）。
- **修 2（漏报洞）**：文件头排除裸 `^+++` → 白名单 `^\+\+\+ (b/|/dev/null|")`（pre-commit 1 + pre-push 2）——内容行 `++foo` 旧过滤器整行丢弃=真密钥漏报，修前漏报已复现、修后必拦。
- **修 3**：新增 `hook_diff()`（-c 钉死 noprefix/mnemonicPrefix/srcPrefix/dstPrefix），用户 diff 配置无法再改变文件头形态，白名单匹配成保证；3 处 git diff 全走它。
- **顺序结论**：文件头过滤必须在剥离标记之前对原始行做（先剥则 `+++ b/x`→`++ b/x` 无人能认出）。
- **验证**：真阳性 7 + 真阴性 7 两钩子全矩阵实测（pre-push 用 --no-verify 独立造提交，case2/case3 全覆盖）；三种文件头形态排除无噪音；f38bc06 事故场景无 SKIP 可 commit；行号 `3:` 正确；bash -n 三文件过；LF/UTF-8 未破坏。
- **残余窄口报备**：内容 `++ b/x` 与文件头逐字节同形属统一 diff 固有歧义（状态机式过滤可根治，本单未动）。
- **另报备**：工作区 47 文件整文件 EOL 漂移（worktree CRLF vs index LF），非本单造成，建议另立单。
- **红线**：未 commit / v0.9.0 未动 / 零凭证 / 临时仓库 4 个全 rm -rf。

## 2026-09-05 — tester-1 — TEST-SYNC-110 ✅ 阶段三（D2D-P2P3-IMPL-109 五态迁移护栏，待主控验收 + 阶段四 TEST-EXEC-111 首跑）

- **交付**：`src/main.rs` +357/-0，单 hunk `:10049` 后新增 `#[cfg(all(test, target_os="windows"))] mod overlay_109_d2d_p2p3_guard_tests`，生产零改动（numstat 357/0）。
- **G1** 回落触发：editing / recording_waveform / error / preview 四入口无效 HDC→false + `d2d::release_resources()`。
- **G2** 命中矩形：G2a submit 无 +1 负向断言；G2b stop 与 d2d 侧同公式（含 +1）；G2c preview 三件套真值表。
- **G3** 波形纯函数：G3a `waveform_bar_height` 公式表（🔴 f32 边界实测修正：0.004→8 非 12，改 0.0039→12）；G3b `waveform_snapshot` 空 buf / poisoned lock。
- **G4** include_str 结构护栏：dispatch 五分支行首计数==5 + GDI 兜底在 8 行内（🔴 contains→startswith 修自命中）。
- **G5** 既有 D2D-HANG-095 护栏在位（:9802/:9835）确认。
- **验证**：fmt --check exit 0 / check --all-targets 0 error（102 warnings 持平）/ 未跑 cargo test（白名单设计如此，首跑归阶段四）。
- **红线合规**：未 commit / v0.9.0 未动 / 零凭证 / 无临时文件。

## 2026-09-05 — tester-1 — E2E-GATE-103 ✅ pytest 门禁归因 + ERROR>0 机器闸门（只动 tests/** + build-test-guide.md，src/main.rs 零触碰）

- **4 条 full_pipeline 决定性实验判 B harness 缺陷**：① -debug 日志实证产品全链路正常（Recording→Processing→注入→Hidden，speech_detected=true / ASR-SUMMARY finished / Injection completed）；② PROCESSING 与 RECORDING 同尺寸 240x36，`detect_overlay_state()` 结构性不可观测 PROCESSING（dict 序 recording 先命中）；③ Win11 notepad shim PID 立即退出。
- **修复**：`_wait_for_pipeline_completion`（等 HIDDEN）+ `_notepad_window_exists`（EnumWindows），四条改写后 **4/4 PASS**。
- **事项2**：`TestFocusLostPreview._no_hardware` 静态方法补齐（8-17 在案 AttributeError），test_injection 3/3 PASS。
- **事项3 机器闸门**：conftest GATE 横幅（选集+deselected+各计数+判定）+ errors>0 强制非零退出码；`tests/e2e_gate.py` 门禁脚本；**故意制造 ERROR 正反验证通过**。
- **全量 E2E**（BUILD-098 旧包）：71P/0F/13S/20D/1 error（test_overlay_position 间歇预暖竞态，单跑 PASS，门禁首次照出）。BUILD-098 5F 全消失。
- **build-test-guide.md**：门禁判据节 + 状态检测表修正（Processing 240x36、FocusLost 320x140）。
- **红线**：生产零改动 / 未 commit / config.toml sha 3186ec8c / debug.log 恢复基线 / 零凭证。
- **详情**：outbox/tester-1/result.md + logs/20260905.md + CHANGELOG.md

## 2026-09-05 — tester-1 — TEST-EXEC-106 ✅ 阶段四（三层全绿 + A1~A4 消融 + 还原自证，生产/测试零改动）

- **Step1**：root **1070P/0F/9I**（1065+5 逐位吻合）/ src-tauri 76P / vitest 89P，三层全绿。
- **消融**：A1 centered_x 忽略 applied_w → G1/G2/G3 红（G5 也红 collateral）；A2 ratio 0.65 → G4 红；A3 overlay_geometry 内联旧公式 → G5 红（命中 2）；A4 Bug B 复原 → **A4-1 历史内联式 G5 红 / A4-minimal 保留调用式 5 条全绿** —— 与主控「一条都不红」预判相反，Bug B 语义本体无护栏可测，唯一真实验证仍是 Gavin 端测目视；建议下一轮把 Show 分支 x 来源抽成可测纯函数。
- **还原自证**：`git diff --numstat src/main.rs` 空；全量重跑 root/src-tauri/vitest 全绿；无残留进程；config.toml sha 3186ec8c 未变。
- **红线**：生产/测试零改动（最终 diff 空）/ 未出包（BUILD-107 取消）/ 未 commit / v0.9.0 未动 / 零凭证。
- **详情**：outbox/tester-1/result.md + logs/20260905.md + CHANGELOG.md

## 2026-09-05 — tester-1 — TEST-SYNC-105 ✅ 阶段三（OVERLAY-101/102 五条护栏，待主控验收 + 阶段四 TEST-EXEC-106 首跑）

- **交付**：`src/main.rs` +164/-0，单 hunk `:9360` 后新增 `#[cfg(all(test, target_os="windows"))] mod overlay_101_centering_guard_tests`，生产零改动（numstat 164/0）。
- **G1** centered_x 公式精确值（work_left=0/负坐标屏/奇偶向零截断/超宽负 x 不 clamp）；**G2** Bug B 量化：同 work_w=1920 下 240 与 960 宽居中 x 差 == (960−240)/2 == 360；**G3** 中心不变量 x+w/2 恒定 + 宽+2→x−1 + 奇数宽 ≤1 容差；**G4** `STREAMING_OVERLAY_MAX_SCREEN_RATIO==0.50` + 1920 复算 960（可测性边界如实声明）；**G5** 单一居中源结构护栏：include_str 自读源码 + 去空白 needle「(work_w−」恰 1 行 + applied_w 断言 + `fn centered_x` 存在，全角减号规避自触发，实测命中恰 1 行（:4342）。
- **G5 判别力边界如实声明**：只匹配 work_w 变量名形态，别的变量名内联会漏。
- **验证**：fmt --check exit 0 / check --all-targets 0 error（102 warnings 持平）/ 未跑 cargo test（白名单设计如此，首跑归阶段四）。
- **红线合规**：未 commit / v0.9.0 未动 / 零凭证 / 无临时文件。

## 2026-09-05 — coder-1 — SECRET-104 ✅ 手机号闸门 Cargo.lock 校验和误报修复（secret-patterns.sh 边界收紧 + README 补 hooksPath 说明，待主控验收）

- **改动**：`scripts/git-hooks/secret-patterns.sh` PHONE 边界 `[^0-9]` → `[^0-9A-Za-z]`（+3 行注释）；
  README.md 新增「### Git 提交钩子」小节（worker-guide.md:156 已有，README 补齐）
- **实测**：一次性临时仓库七用例全过 + push 侧两向验证（干净放行/手机号拦截），
  明细见 `outbox/coder-1/result.md`；首轮 push 侧误判系测试脚本 non-fast-forward 未触发钩子，已复盘重建实证
- **已知漏网形态（如实）**：紧贴字母/数字的手机号（`id13812345678`）不再命中
- **附带发现（未动手，按 §12 上报）**：工作区另有 44 个文件纯 CRLF/LF 翻转，
  `git diff -w` 后实质差异仅 secret-patterns.sh，非本 session 造成，请主控裁定处置方式


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


## 2026-09-05 — tester-1 — TEST-SYNC-096 ✅ 阶段三（解除两条 #[ignore] + D2D-HANG-095 护栏，待主控验收 + 阶段四 TEST-EXEC-097 首跑）

- **交付**：`src/main.rs` +59/-6，五处 hunk 全部落在 `#[cfg(all(test, target_os = "windows"))]` 模块内（overlay_075 / overlay_086），生产零改动。
- **1.1**：两条 `#[ignore]`（:8842 / :8881）整体删除，用例末尾加 `d2d::release_resources()` + 说明注释；测试内 3 个会填槽的 `d2d::draw_*` 调用点（:8855/:8896/:8902）全部覆盖。
- **1.2**：新护栏 `d2d_thread_with_populated_slot_exits_after_in_thread_release` —— `is_finished()` 10s 轮询断言线程真实退出；消融=删线程体内释放必红。
- **1.3**：新护栏 `d2d_release_resources_is_idempotent_on_empty_slot` —— 空槽重复调用不 panic。
- **验证**：`cargo fmt --check` exit 0；`cargo check --all-targets` 0 error（102 条 warning 均既有）；未跑 `cargo test`（白名单设计如此，首跑归阶段四）。
- **红线合规**：未 commit / v0.9.0 未动 / 零凭证 / 无临时文件。


## 2026-09-05 — tester-1 — TEST-EXEC-097 ✅ 阶段四全量回归（三层全绿，消融证判别力，待主控验收 → 阶段五 BUILD-098）

- **数字**：root 主 bin **1065P/0F/9I**（与 1061+2+2 / 11-2 对账逐位吻合）+ src-tauri 76P + vitest 89P；E2E/smoke 主控指定 SKIP（旧包不含本批）。
- **消融**：注释 `src/main.rs:9269`（护栏线程体内 release）→ 单跑护栏用例 → **整个 test 进程挂死**（ablation_run.log：running 1 test 后无结果行，EXIT=124）= D2D-HANG-001 本体复现，判别力最强形态；timeout 清理，无残留进程。
- **还原自证**：diff --numstat=59/6 复原；重跑护栏 1P/0F/0I 0.10s 绿；全量在其后跑 0.69s 无卡顿。
- **红线**：未改任何用例（仅消融行注释并还原）；未 commit；v0.9.0 未动；Publish/config.toml sha=3186ec8c 保持。
- **详情**：outbox/tester-1/result.md + 执行日志（cargo_test_root/tauri.log、ablation_run.log、restore_check.log）。


## 2026-09-05 — tester-1 — BUILD-098 ✅ 阶段五出包（七项核验全过 + 托盘退出 5/5 干净退出，E2E 61P/5F 待主控归因，产物在 Publish/）

- **构建**：四步完成；feiyin-ime 12,229,632B / ui 10,053,632B / crash 24,887,808B；toml 三副本 scene 0a3a0b9a / itn 311cbb96。
- **七项核验**：sha target↔Publish 三对相等、main 39cca97c 异于 093(e6f55e0a)；ProductVersion 0.9.0.0；**D2D-HANG-095 探针=1**（新代码入包硬证据）+ D2D-P1=2；冒烟 Responding 无 panic。
- **托盘退出 E2E（核心）**：复用探针 D（隔离 APPDATA + WM_QUIT 同链路 + 先触发录音画 D2D）→ 新包 **5/5 干净退出**（修复前 5次4挂）；debug.log 4 次 "D2D resources released in-thread before thread exit" 佐证。**D2D-HANG-095 端到端修复成立**。
- **E2E 门禁**：补装缺失 `toml`（首轮 4 条 full_pipeline 收集期 ERROR）后全量 **61P/5F/33S/6deselected**。5F=4×full_pipeline（processing state got hidden/recording，可复现）+1×focus_lost_preview（`_no_hardware` AttributeError = 独立类未继承静态方法的**预存在测试代码 bug**，建议主控派 TEST-FIX）。toggle_stop：本批全量转绿一次、standalone 3x=PASS/FAIL/PASS，**仍间歇**，交主控归因（非本批修复可证事项，第8项已独立证明退出修复）。
- **红线**：未改任何代码；未 commit；v0.9.0 未动；config.toml sha 3186ec8c 保持；debug.log 恢复原样；日志全部 mv 至 outbox/tester-1/build098/（无 cp 残留）；无残留进程。
- **详情**：outbox/tester-1/result.md + build098/ 证据目录。


## 2026-09-05 — coder-2 — OVERLAY-101+102 ✅（Bug B 修复 + 宽度上限 50%，待主控验收）

- **Bug A 协商结果**：我的「display 双计」假说被主控驳回（我把 log display 输出字段二次误当 raw 输入反推；52 包 display 单调、最大 61、零回落）；Bug A 转待证据（09-05 端测无 -debug 日志），qwen_inference.rs 零改动
- **Bug B 机制**（主控裁定「你对了」）：!do_it 节流帧不重算 x，沿用 overlay_geometry 默认位（240 居中）+ SetWindowPos 应用 current → 到上限后右窜 (current−240)/2 且插值循环休眠无帧纠正；触发=句界双 Show 同毫秒（日志 25.835 实证）
- **改动**：centered_x 纯函数统一 5 处居中；Show 流式两分支 x 一律 current 现算（!do_it 帧=修复本体）；插值循环/overlay_geometry/adjust 只换调用不换宽度（主控边界遵守，同值证明表在 result.md）；OVERLAY-102 0.65→0.50 分步改
- **验证**：fmt exit 0 / check 0 error / 163 warning 持平；7 hunk 全列行号，测试模块零触碰；消融推演逐项
- **红线**：interpolate_step/grow-snap/streaming_scroll_offset/d2d 未动 / troubleshooting.md 未碰 / 未 commit / v0.9.0 未动 / 零凭证
- **详情**：outbox/coder-2/result.md + logs/20260905.md + CHANGELOG.md + MACOS-HANDOFF §OVERLAY-101

## 2026-09-05 — coder-2 — D2D-P2P3-PLAN-108 ✅ 五态迁移方案设计交付（零代码改动，待主控评审）

- **交付物**：`collab/drafts/d2d-p2p3-plan.md`（基线 b072383 只读取证，行号实测，主控可 sed -n 抽查）
- **三章全齐**：§3.1 五态逐节（调用链/元素清单/命中矩形口径/region）；§3.2 新原语（任务书 3 个实为 4 个，waveform 性能对比 GDI ~224 次/帧对象操作→0）；§3.3 五风险项各成节（EDIT 共存结论=无新增风险，5 步论证链）；§3.4 12 hunk + 五步顺序（Error→StreamingEditing→FocusLost→Recording→FallingToProcessing）+ 15 项非回归 + 5 条护栏；§3.5 六条不确定项（含 U5：draw_editing_overlay_chrome:2809 死代码，待拍板）
- **协商点**：FallingToProcessing 与 Recording 波形变体 GDI 输出逐位相同 → 共用同一 D2D 复合体（与任务书预设不同，属简化，请主控裁决）；D2dResources +2 字段零新 thread_local（D2D-HANG-095 关天然过）
- **红线**：src/**、ui/**、src-tauri/**、tests/** 零写入；零构建；未 commit；v0.9.0 未动；troubleshooting.md 未碰；零凭证
- **详情**：outbox/coder-2/result.md（含收尾自证表）+ logs/20260905.md + CHANGELOG.md

## 2026-09-05 — coder-2 — D2D-P2P3-IMPL-109 ✅ 五态迁移实施完成（仅 src/main.rs，待主控逐 hunk 验收）

- **范围**：src/main.rs +637/-115，22 hunks 全生产区（末 hunk :4549 ≪ 测试区）；基线 f774b26
- **五步顺序未改**：Error（H1+H2+H12）→ StreamingEditing（H8+H9）→ FocusLost（H10+H11）→ Recording 波形（H3+H4+H5+H6）→ FallingToProcessing（H7）
- **验证**：fmt --check 0 / check --all-targets 0 error / warning 163 = stash 基线持平；几何区六锚点零 diff（grep 证）；零新 thread_local；命中矩形三 helper 单一源（stop+1 / submit 无+1 / preview 三件套纯函数）
- **实施修正 2 条**：① DWRITE_PARAGRAPH_ALIGNMENT_TOP 不存在于 windows 0.58 → NEAR（等价）；② 波形奇数高度 i32 整除口径在 D2D 复刻（方案笔误修正，防 ±1px）
- **给 tester-1（TEST-SYNC-110）**：G1-G5 照方案 §3.4.4；新增素材见 result.md §五（submit 无+1 负向断言 / 波形整除口径 / dispatch 形态正则）
- **红线**：未 commit / tests/** 与 troubleshooting.md 未碰 / draw_editing_overlay_chrome 不删不改不参考（U5）/ 零凭证
- **详情**：outbox/coder-2/result.md（逐 hunk 行号表 + 15 项非回归保证手段 + 收尾自证表）
