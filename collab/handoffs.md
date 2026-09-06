# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-09-06 — coder-2 — OVERLAY-121-PLAN-127 ✅ per-pixel alpha 落地方案设计（零代码，主控验收通过 + 三处反馈已闭环，待 Gavin 拍板：方案 B 批准 + 三态是否顺带升 r=16 圆角）

- **产出**：collab/drafts/overlay-121-plan.md（唯一可写文件，git status 干净，零 src/** 触碰）。
- **推荐方案 B 运行时双模式**：其余七态 ULW + 逐像素 alpha；编辑态（StreamingEditing）切回 SLWA——编辑态现状本就是矩形 None 掩码（main.rs:2175），零视觉倒退；A ❌（EDIT 白送的全套重写不成比例）/ C ❌（双窗常驻状态机不值得）/ D ❌ 不可行（main.rs:1376 每次显示必调 SLWA，ULW 必败）/ 新提第五条 E（WM_PRINTCLIENT 桥接）作 B 闪烁 PoC 不过时的备选。
- **关键查证（MSDN 逐字出处见文档 §12）**：SLWA 调过后 ULW 必败须清/置 style bit 中转；**现有 DCRenderTarget 原生支持 D2D1_ALPHA_MODE_PREMULTIPLIED** → main.rs:3089 一处字段改动 + BindDC 改绑 DIB section，DEC-055 单点收口零变化，不需换 WIC target。
- **主控验收三处反馈已闭环**：① handoffs 补记（本条）；② result.md 补收尾自证表；③ config.toml 开关否决（DEC-031 默认零配置），回滚改为 P1/P2/P3 各自独立 commit、revert 即回滚，draft §8 表已改并附裁定注记。
- **待拍板**：① 方案 B 批准与否；② Recording 系三态+FallingToProcessing 顺带升 r=16 圆角；③ PoC 并入实施单 P2 首项。
- **macOS**：不适用无需迁移（NSWindow 原生支持逐像素透明 + AA 圆角），实施时 MACOS-HANDOFF.md 补一句即可。
- **护栏预告**：TEST-SYNC-122 H7 圆角快照在 P3 后必然变红（预期），建议 tester-1 届时改写为正向护栏（ULW 调用点恰 1 / PREMULTIPLIED 恰 1 / switch_layered_mode 挂钩恰 2）。

## 2026-09-06 — coder-1 — SECRET-126 ✅ 配置/隐私「永不入库」机器闸门（.gitignore 四类路径规则 + pre-commit/pre-push 路径闸门，待主控验收）

- **改动面**：`.gitignore`（+12 行，User app data 节内，含理由注释）+ `scripts/git-hooks/secret-paths.sh`（新建，scan_paths 共享判定）+ `pre-commit`（内容扫描前路径闸门，fail-closed）+ `pre-push`（push_path_gate 函数 + 两分支各 1 调用，情况 2 补 BASE 推导）。未 commit，待主控验收。
- **secret-patterns.sh 现有模式表零触碰**；路径判定单独成文件、两钩子共用防漂移（SECRET-082 同理），两层职责分开。
- **pre-push 侧已加（任务书留判）**：理由 = pre-commit 闸门有 `SECRET_SCAN_SKIP=1` 绕过口 + 未装钩子 clone 不跑；公开仓库 + f58af96 前科，push 是最后一道网。
- **实机矩阵全绿**（一次性临时仓库 + 真 hooks，已 rm -rf）：pre-commit 真阳性 5 拦（根 config.toml 纯路径拦 / .env / debug.log / collab/research 强加 / wordbook.sqlite）、真阴性 4 放（.cargo/config.toml、assets 模板、logs/20260906.md、notes.md）、内容闸门 4 拦 2 放零回归；pre-push 情况 3 + 情况 2 均拦、纯删除放行（--diff-filter=ACMR 排 D）、skip 口径双钩子放行+警告。
- **实现细化**：匹配小写归一（CONFIG.TOML 绕过堵死，Windows FS 实测 + Linux 单元级兜底）/ core.quotePath=false 钉路径形态 / 根 config.toml 仅根锚定。
- **本仓库判据**：git ls-files = 326 不变；untracked 除本单三文件外保持 0。
- **红线**：未 commit / 不动版本号 / 未碰 src/main.rs、hotkey.rs / secret-patterns.sh 模式表零改动 / 零真实密钥（用例全假值）/ 临时仓库已清理。
- **详情**：outbox/coder-1/result.md + logs/20260906.md + CHANGELOG.md

## 2026-09-06 — coder-1 — BUG-119 ✅ 无语音改为信息提示「请说话哦..」（i18n 类型化信号 + Info 态浮层，待主控验收）

- **根因双重**：主控取证的关键词嗅探分类器漏 + 实施侧实证的 `map_err(|e| e.to_string())` 先抹 anyhow 类型（拦截点前移到 map_err 下探）。
- **选型**：`transcription::NoSpeechError`（unit struct，anyhow downcast）。验收判据实证：任务书外的源 4/5（在线回退空 / 全段空拼接）加入时 main.rs 分类代码零改动。
- **产出源 5+2 全表**（含保留 Error 的设备/模型异常 2 条判据）见 logs/20260906.md BUG-119 节。
- **显示**：`PipelineEvent::NoSpeech`（照 FormatFailed 先例）→ `OverlayStatus::Info(String)`（GDI+D2D 双路径，蓝点 #3399FF + 白字，圆角 Some(10) 未动）+ i18n `no_speech_hint`（ZH「请说话哦..」逐字 / ZH_TW「請說話喔..」/ EN "Please say something.."）。tray 复位 Idle。
- **扩单**：src/ui/overlay.rs 加 Info 变体（主控批准，无其它重构）。
- **macOS**：产出源/消费侧同源（transcription 共享 + main.rs cfg(macos) 两分支已加）；ShowInfo 视觉形态留 macOS overlay 批，结论在 docs/MACOS-HANDOFF.md。
- **验证**：fmt --check exit 0 / check --all-targets 0 error / warnings 111/102 与基线逐位持平。
- **红线**：未 commit / v0.9.0 未动 / 圆角 Some(10)/Some(16) 未动 / hotkey.rs 未动 / 零凭证 / 无临时文件。
- **详情**：outbox/coder-1/result.md + logs/20260906.md + CHANGELOG.md + docs/MACOS-HANDOFF.md

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

## 2026-09-06 — tester-1 — TEST-SYNC-116 ✅ 阶段三（HOTKEY-115/B/C 七条结构护栏，待主控验收 + 阶段四 TEST-EXEC-117）

- **交付**：`src/platform/windows/hotkey.rs` 测试模块 +373/-0（单 hunk `@@ -1192,0 +1193,373`），生产零改动（numstat 373/0）；`src/main.rs` 仅 include_str 只读、零触碰。
- **G1**：`PTT_ACTIVE.store(false)` 落在 `should_stop_translate_poll_on_keyup(mode)` if 块内（block_contains 花括号深度）。
- **G2**：钩子 DOWN 路径 Toggle 分支 `TOGGLE_ACTIVE.swap(true)` 翻转（else_branch_contains 定位 `} else {`）。
- **G3**：RegisterHotKey Toggle 分支同翻转 + `:652` 复位（match binding.mode 锚点 + 20 行窗）。
- **G4**：`notify_translate_poll_stop` 函数体内含 `TOGGLE_ACTIVE.store(false)`（B3 单一收口）。
- **G5a/b/c**：install/uninstall/sync_binding 三处清理点各归零 PTT_ACTIVE+TOGGLE_ACTIVE+KEY_PHYSICALLY_DOWN；uninstall 额外 `TRANSLATE_POLL_STOP.store(true)`。
- **G6a/b/c**：`LAST_TARGET_DOWN_TICKS` 存在 + 闸判据形态 + **阈值>1000 语义断言**（从闸行 RHS 解析 token：字面量直读 / const 回查，不断言 ==2000）。
- **G7**：main.rs mic-muted 拒绝出口含 `notify_translate_poll_stop()` 调用（norm_line 剥 `platform::` 前缀）。
- **自扫描规避**：按首个 `#[cfg(test)]` 切分只扫生产区 + 一律 startswith（禁 contains）+ needle 全 `concat!` 拆串（TEST-SYNC-110 教训）。
- **验证**：fmt --check exit 0 / check --all-targets 0 error（102 warnings 与基线持平）/ 未跑 cargo test（白名单设计如此，首跑归阶段四）。
- **逻辑预演**：沙箱逐字复刻护栏匹配逻辑 —— 19 项主断言全 PASS；G1/G2/G3(flip+reset)/G4/G6c(const 删除)/G7 六组消融模拟全 RED-OK；G7 块内注释「notify_translate_poll_stop()」startswith 正确排除无误命中。
- **红线**：未 commit / v0.9.0 未动 / 零凭证 / 无临时文件（预演跑沙箱 temp）。

## 2026-09-06 — tester-1 — TEST-EXEC-117 ✅ 阶段四（三层回归 + 13 消融全红 + Step3 A+ 结构定界，待主控验收）

- **Step1**：root **1088P/0F/9I**（1077+11 逐位对账）+ src-tauri 76P + vitest 89P。
- **Step2 消融 13/13 全红（真代码）**：G1 移 store 出 if / G2+G3flip 改恒发 Start / G3reset 删 store / G4 删收口 store / G5a·b·c 各删三态之一 / G5b-附加 删 POLL_STOP=true / G6a 改名保编译 / G6b 去陈旧闸 / **G6c 2000→1000 边界（"got 1000" 证 `>` 非 `>=`）** / G7 删 mic-muted notify。逐次还原。
- **Step3 行窗余量 → A+ 结构定界（主控批准 + 修正1/2/3 落实）**：G3/G5a/b/c 改 block_contains 花括号定界（零行窗）；修正1 增强支持多行签名锚点（install/sync_binding 锚点行无 `{`，前扫首开括号，干净 PASS 实证）；修正2 G3 锚 `HotkeyMode::Toggle => {` 排除 PTT arm；修正3 每转换护栏 ①干净 PASS + ②消融 RED 双证据；window_has 删死代码。测试模块 +29/-34 生产零触碰。
- **Step4 E2E**：门禁 66P/0F/0E 通过；🔴 热键 6/6 PASS —— **E2E-GATE-103 环境失效已恢复**（任务书预期红未发生，如实报）；33 skip 既有类别无新失败；旧包口径，HOTKEY-115 真 E2E 归 BUILD-118。
- **Step5 还原**：numstat 仅剩 Step3 测试模块 29/34 + main.rs 空；三层重跑与 Step1 一致；消融变量全归零；无进程残留；config sha 3186ec8c；fmt exit 0。
- **红线**：未 commit（含 Step3 A+ 改动）/ v0.9.0 未动 / 零凭证 / 无临时文件 / 未出包。

## 2026-09-06 — tester-1 — BUILD-118 ✅ 阶段五出包（首个含 D2D-109 + HOTKEY-115/B/C 的包，待主控验收 + Gavin 端测）

- **Step1-4 完整执行**：清进程（无在跑）→ npm 1.50s + Tauri UI 1m44s → 主程序 2m12s（111 warnings 持平）→ 同步 Publish/。
- **产物**：feiyin-ime 12,255,232 B@11:42 / feiyin-ime-ui 10,053,632 B@11:39 / crash-reporter 24,887,808 B@11:42；UI 自 src-tauri/target/release `cp -p` 三处一致。
- **toml 三副本**：scene `0a3a0b9a…`×3 / itn `311cbb96…`×3（六行 sha256 原始输出在 result.md）。
- **大小对照**：主程序 +25,600B=两批增量（同量级）；UI/crash 与基线相同（零改动）；~3m58s 同量级。
- **运行时数据零触碰**：config.toml/wordbook.sqlite/debug.log/version_check.json mtime 保持旧值。
- **冒烟**：启动 Responding=True 无 panic；进程清理；**热键端测交回 Gavin**（任务书明确不做）。
- **红线**：未 commit / v0.9.0 未动 / 零凭证 / 无临时文件 / 未用 cargo tauri build。

## 2026-09-06 — tester-1 — TEST-SYNC-122 ✅ 阶段三（BUG-119「没说话」类型化信号 8 条护栏，待主控验收 + 阶段四 TEST-EXEC-123）

- **交付**：main.rs 末尾追加 `#[cfg(test)] mod nospeech_122_guard_tests`，+348/-0（单 hunk `@@ -10624,0 +10625,348`），生产零改动。
- **H1**：NoSpeechError 存在 + impl `std::error::Error`（downcast/is:: 依赖）。
- **H2**：产出源 bail 计数恰 5（qwen:921/1605 + mod:331/373/453），只数代码行（注释 startswith 排除）。
- **H3** 🔴 反向护栏：convert_to_friendly_error 函数体禁「没说话」嗅探（block_contains_any 花括号定界 + 子串例外，守「又回去加 contains」回归路径）。
- **H4**：TranscriptionFailure 枚举含 NoSpeech + run_pipeline_core map_err `is::<transcription::NoSpeechError>()` 下探（拦截在 to_string 前）。
- **H5**：spawn_worker_thread 流式 join is:: 下探 + send PipelineEvent::NoSpeech。
- **H6**：i18n no_speech_hint 三语言非空互异 + ZH 恰为「请说话哦..」（Gavin 原文两 dot）。
- **H7** 🔴 圆角快照：apply_overlay_window_region Some(16)×1 / Some(10)×3 / None×6（OVERLAY-121 有意改动时护栏红属预期）。
- **H8**：Windows 控制器 NoSpeech arm 用 OverlayStatus::Info + tray Idle。
- **写法**：include_str! 按首个 #[cfg(test)] 切分只扫生产区 + 一律 startswith（禁 contains，H3 唯一子串例外）+ needle 全 concat! 拆串 + block_contains 花括号定界（未复活 window_has）。
- **验证**：fmt --check exit 0 / check --all-targets 0 error（102 warnings 与基线持平）/ 未跑 cargo test（首跑归阶段四）。
- **逻辑预演**：沙箱逐字复刻 —— 22 项主断言全 PASS + 消融模拟 RED-OK（H2/H3/H4/H6/H7/H8 各代表消融）。
- **判别力边界**：H2 只扫现有三文件；H5 event 行绑定 event_tx 形参名；H6 按文件首处判 ZH。
- **红线**：未 commit / v0.9.0 未动 / 零凭证 / 无临时文件（append 临时模块文件已 rm）。

## 2026-09-06 — coder-2 — INVESTIGATE-120 ✅ 编辑态光标闪烁根因定位（🔴 只查不修，生产零改动）

> ⚠️ 本条系 **主控 2026-09-06 代记**（`[DOC-STATE-DRIFT-001]` 复现：coder-2 已写 logs/CHANGELOG，
> handoffs.md 零条目）。原始内容以 `logs/20260906.md` 同名节与 `outbox/coder-2/result.md` 为准。

- **根因**：编辑态迟到流式包（`should_ignore_streaming_text = stopped && !editing`，main.rs:4786 不丢包）
  被转发为 `Show(StreamingEditing)`（:5378-5390）→ Show 处理器无条件 `destroy_edit_control`（:1298，
  `create_edit_control` 唯一调用点在 EnterEditMode :1475，**无重建路径**）+ 流式分支重算 `target_size`
  （:1240-1275）→ 插值循环每帧 `SetWindowPos` + x 重居中（:1739-1745/:1754-1787）
  → 窗口几何风暴 + 文字消失 =「窗口和文字闪烁」。**光标移动本身无任何窗口级路径**。
- **三嫌疑裁决**：嫌疑1（SetWindowPos 在飞）证实但触发源修正为迟到包；嫌疑2（WM_ERASEBKGND/双缓冲）
  证伪；嫌疑3（EDIT 双绘制）证伪为主因，降级为长按方向键叠加观感。
- **与 OVERLAY-101 Bug A 同源**：共用 Show→target_size→插值→SetWindowPos+centered_x 几何机器。
- **决定性探针**（collab/research/repro120/）：baseline 光标 26 次/8s → 几何变化 1 次、零擦白帧；
  packets_prod（复刻迟到包链）→ 几何变化 26 次/8s + 全 LARGE 内容波次，完整复现签名。
- **真机端到端受限（如实声明）**：双麦克风数字静音、蓝牙 Unplugged → 在线 ASR 三次 0 samples。
- **修法建议（待 Gavin 拍板）**：R1（推荐）编辑态迟到包直接丢弃（`should_ignore_streaming_text` 改 `stopped`）；
  R2（备选）Show 对 StreamingEditing 走 WM_SETTEXT 同步 + 冻结宽度。R2 与 OVERLAY-121 per-pixel alpha
  存在前置耦合（UpdateLayeredWindow 下子控件不可渲染）。
- **红线**：生产零改动 / v0.9.0 未动 / 探针工程保留 collab/research/repro120/ / run 目录（含密钥 config 副本）已整目录删除 / 零凭证入档。
- **详情**：collab/drafts/edit-flicker-analysis.md + outbox/coder-2/result.md + logs/20260906.md

## 2026-09-06 — tester-1 — TEST-EXEC-123 ✅ 阶段四（三层回归 + 八护栏首跑消融 8/8 全红 + E2E 门禁，待主控验收）

- **Step1 三层**：root **1095P/1F/9I**（总数 1096 对账一致）+ src-tauri 76P + vitest 89P。
  🔴 **1F = 既有护栏快照过时**：`overlay_109 dispatch_five_branches`（断言恰 5 个 d2d::draw_ 分支）
  vs BUG-119 新增 `draw_info_overlay`（main.rs:2221）第 6 分支。非本批八护栏问题、非回归。
  **建议快照 5→6 或改 ≥6 语义，交主控裁定**；本单未修（阶段四零改动红线）。
- **Step2 消融 8/8 全红（真代码，非推演）**：A1 删 impl Error / A2 bail 改字符串（4≠5）/
  **A3 convert 加「没说话」嗅探=回归路径真红** / A4 map_err 改 to_string / A5 删流式 is:: /
  A6 改 ZH 文案 / A7 Some(10)→None（3→2）/ A8 Info→Error。逐条还原后 8/8 绿。
- **Step4 E2E 门禁**：首跑 63P/1F/2E → **逐条单测复跑全绿判定环境瞬态**（notepad 残留实例
  阻塞 `subprocess.run(["notepad"])` fixture + [E2E-COLD-START-RACE-001] prewarm 竞态）；
  二次全量 **66P/0F/0E/33S/6D 通过**，与 TEST-EXEC-117 一致。E2E 跑 BUILD-118 旧包，仅回归确认。
- **Step5 还原**：git diff --numstat 与 HEAD 零差异（仅 CRLF 警告）/ 三层重跑逐位一致 /
  无残留进程 / config sha 3186ec8c / fmt 0。
- **教训**：A8 还原 oldString 过长误伤 show_overlay（cannot find value hint），按 git show HEAD
  手工修复。消融还原必须用行号级短锚点。
- **红线**：未 commit / v0.9.0 未动 / 未出包 / 零凭证 / 无临时文件。
