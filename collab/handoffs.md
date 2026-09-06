# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-09-06 — coder-2 — HOOKTEST-136-B ✅ 测试文件密钥字面量改运行时拼接（闸门不再命中自己，51 全绿 + 闸门实扫 exit 0，待主控验收）

- **改法**：主控裁定 C（运行时拼接，不开豁免不走 SKIP）—— 9 个假值常量 + git 身份邮箱（chr(64) + noreply 白名单段双保险）+ 路径载荷 f-string 化 + 三处注释自命中形态改述 + tauri 图标载荷拼接。**逐字节比对 PASS**（10 常量运行时打印逐一相等）。
- **决定性证据**：临时仓库 hooksPath 指向真实钩子，对测试文件 add+commit → pre-commit 全量扫描 **exit 0**（改前 6 处命中）。
- **验证**：pytest 51P/0F；反向自证 FAKE_PHONE 拼成 10 位 → phone 用例红（拼接结果=被测形态证实），已还原。
- **红线**：scripts/git-hooks 零触碰 / 未用 SKIP / 未 commit / 未碰 ui/**。

## 2026-09-06 — coder-1 — UIFIX-139 ✅ 补 `--system-text-disabled` 令牌（G2 抓到的缺陷闭环，待主控验收）

- **改动**：`styles.css` +4 行（`--system-text-disabled: rgba(0,0,0,0.3614)` + 取值注释）+ `design-tokens.test.ts` G2 白名单删 1 条（`--value` 保留）。未 commit。
- **缺陷**：styles.css:1356 词库「添加」按钮禁用态文字引用未定义令牌 → 计算值无效回落继承色（禁用态与正常态同色，浏览器不报错）——UITEST-137 G2 首跑抓到的两个真实隐患之一。
- **取值**：WinUI Light TextFillColorDisabled 口径（主控已定）。复核：`.btn-primary:disabled` 现行 tertiary+opacity 不可直接搬（opacity 视觉叠加）、rgba 与 fill/border 族一致，无更优解。
- **验证**：test 97P 不变（零新增用例）；反向自证：删定义 → G2 红在 :1356 → 还原复绿。
- **🔴 build 门禁被外部阻塞**：tester-1 在途 `src/test/browser/visual-style.test.tsx`（UITEST-138）TS6133 使 `npm run build` 红；`npx tsc --noEmit` 证实唯一报错在该文件、本单零错误。红线禁碰其文件，已报备主控，待 tester-1 修复后自然恢复。
- **详情**：outbox/coder-1/result.md + logs/20260906.md + CHANGELOG.md

## 2026-09-06 — coder-1 — UITEST-137 ✅ 第一阶段设计令牌合规护栏（G1/G2/G3 + 3 条行为示范，清理 3 处分叉，待主控验收）

- **产出**：`ui/src/test/design-tokens.test.ts`（新增，8 用例）+ `About.tsx`/`Llm.tsx` 3 处字面量换令牌（精确等值，行号零漂移）。pages 业务逻辑零触碰，未 commit。
- **护栏设计**：Node 侧源码扫描（不依赖 DOM，happy-dom 无 CSS 引擎也工作）。G1 白名单基线制「存量不阻塞、增量被挡」+ 双向自防（白名单空挂即红）；G2 引用完整性 **首跑抓到 2 个真实隐患**（`--value` 死滑杆带 fallback / `--system-text-disabled` 未定义回落继承色，均白名单附理由，不擅自补定义防改变渲染）；G3 值唯一性 + 显式 ALIAS_GROUPS 登记（零重复）。
- **实测量证**：G1 扫描域硬编码实测仅 7 处（任务书估 40 系把 styles.css 定义处 72 处计入，G1 明确排除定义处）。清理 3 / 剩 4（About 105/108 #6b7280 灰阶、114/124 成功态绿与 --status-success 非等值，设计侧未定层级不动）。
- **行为示范**：user-event + aria 断言（checked 翻转 / Escape 状态转换 / role="dialog" 撞键弹窗），替代 class 名断言范式。
- **🔴 环境发现（UITEST-138 前必读）**：happy-dom KeyboardEvent 不映射 F 键 code（`{F9}`→`Unknown`；字母/Enter/Backspace 正常）。依赖 `e.code` 的 user-event 按键流只能用字母键；fireEvent 显式传 code 不受影响（既有测试未踩到的原因）。
- **验证**：`npm run test` 97 passed（89+8）/ `npm run build` 成功；反向自证 G1+G2 双红（临时 fixture 已删）复绿。
- **styles.css 内部遗留**（G1 范围外，记后续）：1349 行 `#e55a2b` == `--brand-hover` 字面量分叉等。
- **详情**：outbox/coder-1/result.md + logs/20260906.md + CHANGELOG.md

## 2026-09-06 — coder-2 — HOOKTEST-136 ✅ 密钥/隐私闸门自动化测试固化（tests/test_secret_hooks.py 51 用例全绿 + 2 次反向自证，待主控验收）

- **产出**：`tests/test_secret_hooks.py`（唯一新增）+ `tests/pytest.ini` 注册 hooks 标记；`scripts/git-hooks/` 零触碰（按新分工规则：闸门是 coder-1 写的，测试由 coder-2 写）。
- **五类齐全**：内容拦 10 / 内容放 12（占位符·白名单邮箱·env 读法）/ 路径拦 10·放 9（全走 add -f 强塞 + CONFIG.TOML 大写）/ 真实事故回归 5（SECRET-104/105 漏报误报 + SECRET-IN-REPO-001 配置 dump + Tauri 图标已知误报如实断言）/ 元行为 5（SKIP 双侧留痕 + push 兜住 commit 绕过 = 纵深防御 + 干净放行 + pre-push fail-closed 构造成功）。
- **验证**：`py -3.11 -m pytest tests/test_secret_hooks.py -v` 51 passed / 0 failed（timeout=30 默认档全过）。反向自证 A（假 key 换进放行用例→红）+ B（.md→.log→红）均证实判别力，已还原。
- **实现红线**：一次性临时仓库 + bare 远端，用完即弃；每次 add 后 ls-files --error-unmatch 复核（MSYS-GIT-ADD-FORCE-NOOP-001 防线）；零真实凭证。pre-commit fail-closed 无法干净构造已在 result.md 如实说明。
- **备注**：工作区 ui/src/* 在途改动非本单产物（其他 Worker），零触碰。

## 2026-09-06 — coder-2 — OVERLAY-121-IMPL-133 ✅ per-pixel alpha P1+P2+P3 一次做完（ULW+DIB+fixup / 双模式切换 / 掩码退役，1098P/1F 唯一红=预期 H7，待主控验收）

- **diff**：仅 src/main.rs +282/-86。P1=渲染骨架（PREMULTIPLIED + 32bpp DIB + apply_alpha_fixup 帧末单点 + ULW 单点提交 + :1376 SLWA 条件化）；P2=switch_overlay_layered_mode 唯一收口（清/置 WS_EX_LAYERED 双向中转，MSDN 逐字）+ EnterEditMode/Show 两处挂钩（隐藏区间切换）；P3=apply_overlay_window_region 函数+10 调用点退役 + chrome 半径参数化（Recording 系/Falling r=16，Error/FocusLost/Info/编辑 r=10，Gavin 口径）。
- **验证**：fmt 0 / check --all-targets 0 error / warnings **111/102 与基线逐位持平** / cargo test **1098P/1F/9I，唯一红=H7 圆角快照（预期，tester-1 阶段三更新）**；wordbook 等其余目标 51P/0F。
- **三阶段 commit 边界 + H7 新快照建议 + 护栏建议**：outbox/coder-2/result.md。
- **PoC 项（端测）**：P2 切换闪烁 / PREMULTIPLIED→DIB alpha 传递（worst case fixup 兜底为不透明，几何仍正确）/ 流式帧率。
- **macOS**：不适用复核成立（全部改动 cfg(windows) 内；NSWindow 原生逐像素透明），已写 docs/MACOS-HANDOFF.md。
- **红线**：未 commit / 未动版本 / 未出包 / 圆角快照护栏未改（H7 红留给 tester-1）/ hotkey.rs 未动 / 零 config 开关。

## 2026-09-06 — coder-2 — FLICKER-130 ✅ 编辑态闪烁根治 R1 落地（should_ignore_streaming_text 收敛单参，三问取证 + 1096P/0F，待主控验收）

- **核心改动**：`should_ignore_streaming_text` 由 `stopped && !editing` 收敛为 `stopped`（单参化），编辑态迟到流式包一律丢弃。仅 src/main.rs +34/-22（含 doc/测试），fmt/check 过，warnings 111/102 与基线逐位持平，cargo test 1096P/0F/9I。
- **三问取证**：① `editing` 豁免系 OVERLAY-043（a588509）有意加的「编辑中同步 EDIT 文字」，但其路径（Show→destroy_edit_control）从未实现该意图，唯一效果是闪烁本身——删除不属 PLAUSIBLE-FIX 形态；② 快照链/词库镜像在门闩前取数，零影响，代价 ≤ 个别字尾巴且现状是「销毁 EDIT」非「同步」——R1 严格改进；③ 非编辑态逐位等价（调用点全库 1 处 + 测试 2 处全盘清）。
- **不变量**：EditRequested 相邻置位 editing⇒stopped（同线程），(false,true) 不可达；门闩仍为「仅渲染」，镜像先行未动（053-B 安全）。
- **macOS**：不同源不适用复核成立（StreamingText 仅 log :6805，无编辑态），已写 MACOS-HANDOFF.md。
- **红线**：未 commit / 未动版本 / 未出包 / 圆角未动 / hotkey.rs 未动 / 零凭证。
- **护栏建议**：result.md 附录（门闩仅渲染顺序护栏 / 调用点唯一性 / editing⇒stopped 不变量护栏）。

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

## 2026-09-06 — tester-1 — TEST-FIX-125 ✅ overlay_109 dispatch 快照 5→6（测试模块 +5/-4，生产区零触碰，待主控验收）

- **背景**：TEST-EXEC-123 唯一 1F = dispatch 护栏断言「恰 5 个 if!d2d::draw_ 分支」，
  BUG-119 新增 draw_info_overlay（main.rs:2221）第 6 分支。主控复核=合法新增快照过期。
- **改动**：assert_eq! 5→6（保持精确相等，禁改 >=）+ 消息分支名清单补 info + docstring 同步。
- **证据**：① 单跑 PASS；② 消融 6→5 红（left:6 right:5）还原绿；③ 定向消融删 :2221 GDI 兜底
  → 护栏精确报 L2221 兜底缺失还原绿（info 兜底断言真实覆盖，现有实现自动纳入）；
  ④ root 全量 **1096P/0F/9I**（1F 转绿，总数不变）。
- **红线**：未 commit / v0.9.0 未动 / 未出包 / 零凭证 / 无临时文件。

## 2026-09-06 — tester-1 — BUILD-129 ✅ 阶段五出包（首个含 BUG-119「请说话哦..」的包，待主控验收 + Gavin 端测）

- **Step 2 跳过前提已复核**（ui/ + src-tauri/ 自 BUILD-118 零 diff），只跑 Step1→Step3（2m13s）→Step4。
- **产物**：feiyin-ime 12,262,400 B@14:56（+7,168B=BUG-119）/ feiyin-ime-ui 10,053,632 B@11:39 沿用 /
  crash-reporter 24,887,808 B@14:55 同批重建（sha 变化=正常，源码零改动+无时间戳嵌入+todo 先例）。
- **七项核验全 PASS**：时间戳 / sha 两副本三对相等 + 主程序异于 BUILD-118（UI 相同）/ toml 三副本 /
  ProductVersion 0.9.0.0 / **判别探针 6 条全命中 + BUILD-118 反向对照 0 命中** / 大小同量级 / 冒烟无 panic。
- **运行时数据零覆盖**：config.toml sha `3186ec8c` 未变；version_check.json 由冒烟启动程序自写（非 Step4 覆盖）。
- **红线**：未 commit / v0.9.0 未动 / 未用 cargo tauri build / 未 cargo clean / 零凭证 /
  热键与浮层端测交回 Gavin（端测清单在 result.md）。

## 2026-09-06 — tester-1 — TEST-SYNC-131 ✅ 阶段三（FLICKER-130 门闩收敛护栏 F1-F3 + F4 唯一权威份安排，待主控验收 + 阶段四）

- **交付**：main.rs +213/-0（hunk1 :9186 +5 doc 注释交叉引用；hunk2 :10988 mod flicker_130_guard_tests +208），生产区零改动。
- **F1** 镜像(L5381)早于门闩(L5389)且同处 process_controller_events 函数体（block_bounds 花括号定界）；clone 锚点重构误红=有意。
- **F2** 调用点恰1 + 单参签名恰1 + 双参0。
- **F3** EditRequested 臂双 store 共现（editing⇒stopped 不变量）；不写不可达分支断言。
- **F4 主控裁定落实**：删 F4 副本、:9190 加唯一权威份交叉引用、flicker 模块留指向注释；阶段四消融验证「函数改回双参→F2 必须红」。
- **验证**：fmt 0 / check --all-targets 0 error / warnings 111/102 持平 / 沙箱预演 F1-F3 PASS + 消融 A-D 全 RED-OK。
- **红线**：未 commit / v0.9.0 未动 / 未出包 / 未跑 cargo test / 零凭证 / 无临时文件。

## 2026-09-06 — tester-1 — TEST-EXEC-132 ✅ 阶段四（三护栏首跑 1099P + 🔴 A5 主控欠账验证闭环，待主控验收）

- **主控缩范围**：只做 Step1a + A5；Step1b/vitest/E2E 挪合并回归（缩范围前已跑全 PASS）。
- **Step1a**：root **1099P/0F/9I**，F1/F2/F3 首跑全 PASS（首跑 24 config FAIL 为并行锁瞬态，二轮全绿）。
- **🔴 A5**：函数改回双参 → **编译失败 4 处 E0061**（生产调用点 :5389/:9477 + 行为测试 :9197/:9199）。
  A5 红成立且比预期更强（编译期硬约束）；**主控删 F4 依据实测闭环，F4 确属冗余，裁定正确**。
- **A1-A4 跳过**（阶段三沙箱已证 RED）。
- **还原**：git diff 空 / config sha 3186ec8c / 无残留进程 / fmt 0。
- **红线**：未 commit / v0.9.0 未动 / 未出包 / 零凭证 / 无临时文件。

## 2026-09-06 — tester-1 — TEST-SYNC-134 ✅ 阶段三（OVERLAY-121 护栏：H7 换血 + G2-G7，待主控验收 + 阶段四）

- **交付**：main.rs +284/-27（H7 换血 -37/+24 + 新模块 overlay_121_guard_tests +273），生产区零改动。
- **H7 换血**：旧快照（Some16×1/Some10×3/None×6）判据基础没了，换 G1 结构护栏（掩码归零），留退役说明。
- **G2-G7**：ULW 单点 / fixup 单点+同块（rposition 反向定位 if use_ulw）/ SLWA 条件化 /
  切换隐藏区间（EnterEditMode+Show 双处，同块+行序）/ fixup 三分支 / DIB 32bpp+负高。
- **预演发现并修正 3 处护栏 bug**：G4/G5 needle 缺 let _ = 前缀（会永久红）；G3 find_line 误锚
  首个 if use_ulw（改 rposition）。已修并通过预演。
- **验证**：fmt 0 / check 0 error / warnings 111/102 持平 / 沙箱 G1-G7 PASS + 消融 7/7 RED。
- **红线**：未 commit / v0.9.0 未动 / 未出包 / 未跑 cargo test / 零凭证 / 无临时文件。

## 2026-09-06 — tester-1 — TEST-EXEC+BUILD-135 ✅ 阶段四+五（回归 1105P + 消融 G1-G7 全 RED + 出包，待主控验收 + Gavin 端测）

- **阶段四**：root 1105P/0F/9I（G1 是 H7 换血不增减，实际 +6，主控确认达标）+ src-tauri 76P + vitest 89P。
- **消融 G1-G7 七条全 RED + 还原**；发现并修 2 处护栏 bug（G1 漏函数定义匹配 / G5 show_hide 缺 let _ 前缀）。
- **出包**：feiyin-ime 12,263,424B@17:55（+1024B OVERLAY-121）sha ac70a990…；UI 沿用 11:39；
  toml 三副本一致；ProductVersion 0.9.0.0；判别探针 UpdateLayeredWindow 新=1旧=0 硬证据；冒烟无 panic；
  config.toml sha 未变。
- **主控紧急加速**：出包后 E2E/src-tauri/vitest 挪下一批；Gavin 端测清单 6 项在 result.md。
- **红线**：未 commit / v0.9.0 未动 / 未出包（已出 BUILD-135）/ 零凭证 / 无临时文件。
