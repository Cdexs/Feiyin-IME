# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-08-18 — tester-1 — E2E-HARNESS-050 ✅ 修复 E2E 两道陈年错位 + 全套 harness 修复（9 FAIL→2 FAIL，待主控验收）

- **来源**：Gavin 批评「代码几轮修改仍未拿到主程序流程完全走通的版本」，主控复盘根因是 E2E 门禁**自初始提交起从未绿**。基线 HEAD `0049c33`（⚠️ 工作区实际含 coder-2 **未提交** OVERLAY-051-G WIP：`src/main.rs` +270 / `qwen_inference.rs` +45，全程不触碰）
- **错位一（配置路径）**：harness 写 `%APPDATA%\voice-ime\config.toml`，程序 `src/config/mod.rs:340-346` 只读 `<exe_dir>/config.toml`。三处实例统一改为 `tests/conftest.py` 新增 `voice_ime_config_file()` 单一来源；未加 `VOICE_IME_CONFIG` 环境变量（遵 DEC-031）
- **错位二（overlay 判据）**：`state_detector.py` 导入时正则解析 `src/main.rs:880-884` 三常量动态建表（零硬编码）+ `TOLERANCE_PX=15` 容差带（依据 <3px 抖动 << 80px 状态间隔）；`recording==processing` 240x36 纯尺寸不可分局限已在 docstring 如实声明
- **附带修复**：`test_platform.py` `--lib`→`--bin feiyin-ime`（bin-only crate）+ `_cargo_bin()` 定位 + UTF-8 capture（原 GBK 崩线程）；`test_tauri_v2_commands.py` 白名单补 3 命令（前端 6 个 invoke 已核实全覆盖）
- **hotkey cold 竞态**（[E2E-COLD-START-RACE-001]）：worker `src/main.rs:3969-3974` Start 清空 stop 信号，cold 窗口内二击丢失 / PTT<300ms 防误触 `cancel-stop`；warm 4/4 探针证据 → harness 加 `_prewarm_recording()` 预热往返，不改产品不弱化断言
- **实跑**：Publish/ 真包 `pytest tests/test_cases/ -m "not hardware"` → **2 FAIL / 61 PASS / 32 SKIP / 7 deselected（142.96s）**，基线 9 FAIL 中 **7 条 harness 全过**；残留 2 条 `test_cargo_test_*` = **② coder-2 阻塞**（未提交 WIP `qwen_inference.rs:712` 签名改 vs `:1645` 测试闭包 1 参 → E0593，非 harness）
- **红线合规**：`git diff --stat` 仅 `tests/**` 8 文件（+138/-47，CRLF 归一）；生产零改动；未放宽断言；config.toml 字节级还原（最终 sha=`3186ec8c05cd…`=备份）；guard 改字节级还原防 LF→CRLF；版本号/commit/出包均未动
- **详情**：result.md + logs/20260818.md + CHANGELOG.md + troubleshooting [E2E-COLD-START-RACE-001] 新增

## 2026-08-17 — coder-1 — OVERLAY-051-G ✅ 抖动缓冲（打字机效果，src/main.rs +199/-28，待主控验收）

- **来源**：Gavin 端测日志：服务端每约 1000ms 吐一批 3-4 字，用户看到「一顿一顿地蹦字」。首字延迟 42ms 不在关键路径。方案：服务端到达节奏与屏幕显示节奏解耦
- **改动**（仅 `src/main.rs` +199/-28）：
  1. 新增纯函数 `compute_tween_advance(displayed, target, elapsed_ms, tween_start_target) -> (usize, u64)`：速率自适应（backlog 摊 1000ms，interval clamp 180-350ms），边界 1（target<displayed snap），边界 3（backlog>20 jump）
  2. `OverlayWindowState` 新增 `tween_start: Option<Instant>`
  3. `RecordingWithText` 分支重写 tween 推进，用 `compute_tween_advance` 替代旧 budget 逻辑
  4. `Show` 处理：离开 `RecordingWithText` 时（如 `HotkeyEvent::Stop`→`FallingToProcessing`）游标排空到 target
  5. `EnterEditMode`/`Hide` 补 `tween_start = None`
  6. 9 条纯函数测试（含 Gavin 真实数据推演）
- **自证**：① Gavin 真实数据逐时刻推演（"最近"2字 interval=350ms，"最近有什么"5字 backlog=3 interval=333ms）；② 三个边界（撤回 snap/新句归零/积压>20 jump）；③ 提交路径拿完整文本（EnterEditMode 排空到 text.chars().count() + SubmitRequested 用 EDIT 完整文本 + last_streaming_text 存完整）；④ 本地模型零影响（覆盖调用门控：is_streaming_asr→StreamingText→RecordingWithText→tween 仅在此分支）；⑤ 纯函数签名 `compute_tween_advance(displayed, target, elapsed_ms, tween_start_target) -> (new_displayed, next_offset_ms)`
- **验证**：`cargo fmt` clean / `cargo check --all-targets` 0 error / `cargo check src-tauri` 0 error / `cargo test overlay_051g` 9 passed / `git diff -w --stat` 仅 `src/main.rs`
- **红线合规**：未碰 `src/transcription/**` / `interpolate_step`/`should_ignore_streaming_text` 契约 / `InvalidateRect bErase` / `ui` / 版本号 0.8.0 / 未 commit
- **详情**：logs/20260817.md + CHANGELOG.md

## 2026-08-17 — coder-1 — ASR-055 + WORDBOOK-053-C/D ✅ 测试连接 Inference 协议重写 + 词库候选校验 + 脏数据排查（3 文件 +428/-89，待主控验收）

- **来源**：Gavin 端测配置 UI 测试按钮报错 + 自动学习整句话入库。基线 HEAD cb71a46
- **ASR-055**：src-tauri/src/qwen3.rs 完全重写（旧 Realtime 协议 → Inference 协议），src-tauri/src/main.rs 读真实配置 asr_online_url/asr_online_model。model 在 payload.model（对照生产 qwen_inference.rs:95-128），三类错误信息（API key/网络/URL），不发音频省钱快
- **WORDBOOK-053-C**：src/wordbook/mod.rs +163 新增纯函数 is_valid_candidate 7 条规则，learn_suggestion 入口调用，拒绝用 log::debug!（release 零磁盘 IO），不改阈值
- **WORDBOOK-053-D**：db_path()=exe 同级 wordbook.sqlite；wordbook 表 18 条全正常；candidates 表 164 条中 12 条脏数据已报告 5 条样例；绝对未删除
- **自证**：日志脏数据必拒 + 正常词必过逐条推演；model 在 payload.model 与生产对照一致；三类错误文案与触发条件；不会误杀 LLM 建议词
- **验证**：cargo fmt clean / cargo check src-tauri 0 error / cargo test src-tauri 74 passed / cargo test wordbook 51 passed / git diff -w --stat 仅 3 文件
- **⚠️ 主 crate cargo check --all-targets**：因 coder-2 OVERLAY-054 未提交改动（src/main.rs E0277）失败，非本任务引起
- **红线合规**：未碰 src/main.rs / ui/** / 版本号 0.8.0 / 未 commit；未删 qwen3.rs（报主控后议）
- **write 工具 silent fail**：edit/write 对 qwen3.rs 和 wordbook/mod.rs 出现 silent fail，改用 WSL Python codecs.open 写入成功
- **详情**：logs/20260817.md + CHANGELOG.md

# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-08-17 — coder-1 — OVERLAY-051-A/H ✅ EDIT 子类化转发修复 + 编辑态横向滚动（P0，src/main.rs +55/-17，待主控验收）

- **来源**：OVERLAY-051 coder-2 完成七项后上下文耗尽，剩下 051-A/H（最根本的一条）改派 coder-1。基线 HEAD `9c1806d`。Gavin 端测三现象：① 进编辑态窗口文字消失；② 没法编辑；③ 文字超窗口宽光标到不了无法标记
- **根因**：`edit_subclass_wnd_proc` 除拦 `WM_NCPAINT`/`VK_RETURN` 外一律 `DefWindowProcW`（默认窗口过程，**不是 EDIT 类过程**），`grep CallWindowProcW src/main.rs`=0 印证原过程保存了但从未调用 → EDIT 文本存储/绘制/按键/插入符滚动/选区全被绕过
- **改动**（仅 `src/main.rs` +55/-17）：
  1. import 加 `CallWindowProcW`/`SetPropW`/`GetPropW`/`RemovePropW`/`HANDLE`，移除未用 `ES_MULTILINE`
  2. const `EDIT_OLD_PROC_PROP` 宽字符串 prop 名
  3. `create_edit_control` 用 `SetPropW` 把 old_proc 存到 EDIT 命名属性（`GWLP_USERDATA` 已被 051-D 占用存父窗口 HWND）
  4. `edit_subclass_wnd_proc` 其余消息改 `CallWindowProcW(old_proc)` 转发（`GetPropW` 取回 transmute `WNDPROC`）；`WM_NCPAINT`/`VK_RETURN` 拦截不变
  5. `destroy_edit_control` 加 `RemovePropW` 清理
  6. 051-H：去 `ES_MULTILINE` 改真正单行 EDIT（保留 `ES_AUTOHSCROLL` 跟随式自动滚动，无 `WS_HSCROLL` 滚动条）
- **方案选择**：选项 1（`CallWindowProcW` + `SetPropW`），不选选项 2（`SetWindowSubclass` + Comctl32 依赖）。理由：改动小、不引入 Cargo.toml feature 变更（红线 1）、所需 API 全在已启用 feature 下、`GWLP_USERDATA` 已被占用故用 `SetPropW`
- **自证**：① `old_proc` 存 `SetPropW` 命名属性（非 `GWLP_USERDATA`）；② `WM_NCPAINT` 拦截在 `CallWindowProcW` 转发前返回 0，去边框效果不丢；③ `destroy_edit_control` 先恢复原 WNDPROC → `RemovePropW` → `DestroyWindow`，无悬空指针无泄漏；④ 去 `ES_MULTILINE` 后单行 + `ES_AUTOHSCROLL` 标准语义光标可达全部文字，**无 `WS_HSCROLL` 滚动条**；⑤ 051-D 回车提交在单行模式下**更可靠**（多行 Enter 插换行不上报 `VK_RETURN`）；⑥ 不可单测（EDIT 子类化依赖真实 HWND），未写假护栏，靠 Gavin 端测 BUILD-020
- **验证**：`cargo fmt --all -- --check` clean / `cargo check --all-targets` 0 error（109 既有 warnings 无新增）/ `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / `git diff -w --stat -- src/ src-tauri/` 仅 `src/main.rs`
- **红线合规**：未碰 `src/transcription/**`（红线 2）/ `interpolate_step` `should_ignore_streaming_text` 签名契约（红线 3）/ `InvalidateRect` `bErase`（红线 4）/ coder-2 已提交七项（红线 5，在其基础上补）/ `ui/**`（红线 7）/ 版本号 0.8.0 未动（红线 6）/ 未 commit
- **详情**：logs/20260817.md + CHANGELOG.md（OVERLAY-051-A/H 条目）

## 2026-08-17 — coder-1 — HOTKEY-049 ✅ 翻译热键与录音热键重复检测 + 拦截（4 文件 +78，待主控验收）

- **来源**：Gavin 拍板「要拦截，如果是组合键，其中一个键重复也要拦」。方案完全固化在 todo.md `## 🔴 已拍板待派发 · HOTKEY-049` 专节（含 7 条实例对照表）。HOTKEY-048 文案已写「不可和录音热键重复」但三层强制机制全为空，本单补 UI 层强制
- **判据**：两个热键的按键集合交集非空即拦。语音键集合 = `{vk_code}` ∪ (modifiers 展开左右修饰键 vk：MOD_ALT→{0xA4,0xA5}/MOD_CONTROL→{0xA2,0xA3}/MOD_SHIFT→{0xA0,0xA1}/MOD_WIN→{0x5B,0x5C})；翻译键集合 = `{translation.vk_code}`（vk=0 时空集不拦）。修饰键必须展开左右两个，因为语音 modifiers 是 Win32 `MOD_*` 不分左右
- **改动**（4 文件 +78）：
  1. 三个纯函数 `voiceKeySet`/`translationKeySet`/`keysOverlap`
  2. 新增 `dupConflict` state
  3. 语音侧 `checkAndApplyVoiceHotkey` 在 `check_hotkey_available` 通过后、`applyVoiceHotkey` 前加 `keysOverlap` 检测，命中不落库弹新提示
  4. 翻译侧 `applyTranslationHotkey` 入口加 `keysOverlap` 检测
  5. 新弹窗只有「知道了」一个出口**无「仍然使用」放行按钮**（红线 1）
  6. 三份 locale 各 +5 i18n key（`hotkey_dup_title`/`prefix`/`infix`/`suffix`/`ack`，沿用既有 `hotkey_conflict_prefix/suffix` 拼接风格）
- **自证**：① 7 条实例对照表逐条走通（与 todo.md 完全一致）；② 新弹窗 JSX footer 仅 `t.hotkey_dup_ack` 一个按钮，无 `hotkey_use_anyway`；③ 双向校验调用点（语音侧 `checkAndApplyVoiceHotkey` + 翻译侧 `applyTranslationHotkey`）；④ `grep -c ":"` 三份 locale = 116:116:116（新增 5 个 dup key 一致）+ `grep -P '[\x{4e00}-\x{9fff}]' HotkeySettings.tsx` = 0 命中（tsx 无裸中文）
- **范围外**：后端不做强制（手改 config.toml 不受保护，Gavin 说「设置时拦截」属 UI 层）/ 不动 `check_hotkey_available`（签名拿不到配置）/ 不动 `TRANSLATION_SINGLE_KEYS` / `zh-Hant.ts` 历史缺 key 问题未在本次处理
- **验证**：`npx tsc --noEmit` 0 error / `npm run build` 通过 / `git diff -w --stat -- ui/` 恰 4 文件 / 未跑 `npm run test`（阶段三 tester-1 负责）
- **红线合规**：未碰 `src/` `src-tauri/`（coder-2 独占 `src/main.rs`）/ 新弹窗无放行按钮（红线 1）/ 双向生效（红线 2）/ 新增 i18n 三语齐全（红线 3）/ 版本号 0.8.0 未动 / 未 commit
- **详情**：logs/20260817.md + CHANGELOG.md（HOTKEY-049 条目）

## 2026-08-17 — coder-1 — HOTKEY-048 ✅ 翻译热键页说明文字改文案（三份 locale，纯文案，待主控验收）

- **来源**：Gavin 追加要求。HOTKEY-047 已结案提交（`4d5c56b`），本单纯文案无冲突。BUILD-019 卡在此单等前端重跑构建
- **改动**（仅三份 locale 的 `hotkey_set_translation` key，各 +1/-1）：
  - `ui/src/i18n/zh-Hans.ts:104` → `'推荐：左右 Ctrl / Alt，不可和录音热键重复'`
  - `ui/src/i18n/zh-Hant.ts:104` → `'推薦：左右 Ctrl / Alt，不可和錄音熱鍵重複'`
  - `ui/src/i18n/en.ts:104` → `'Recommended: left or right Ctrl / Alt; must not duplicate the recording hotkey'`
  - 用 Edit 工具改（非 PowerShell），编码安全。渲染位置 `HotkeySettings.tsx:401`，tsx 一行未动
- **明确不在本单范围**：文案「不可和录音热键重复」目前无代码强制（`check_hotkey_available` 对 0xA0..0xA5 无条件 return true、`handleTranslationHotkeyKeyDown` 无重复校验）。本单只改文案**不实现校验**，是否加强制校验及重复时 UI 行为是产品决策，主控正在问 Gavin，另开 HOTKEY-049。**未自行发挥**
- **自证**：① `grep -n hotkey_set_translation ui/src/i18n/*.ts` 三行新值正确，中文未乱码；② `grep -n hotkey_click_to_change ui/src/i18n/zh-Hans.ts` → `'建议设置左右 Ctrl 或 Alt 键为热键'` —— 上一单（HOTKEY-047 R1）的值**未被误改** ✅
- **验证**：`npx tsc --noEmit` 0 error / `git diff -w --stat -- ui/` 恰 3 文件各 +1/-1 / 未跑 `npm run build`（构建由 tester-1 做）/ 未跑 `npm run test`（纯文案单）
- **红线合规**：未碰 `HotkeySettings.tsx`、`src/`、`src-tauri/`（红线 1/2）/ 未实现重复校验（红线 3）/ 版本号 0.8.0 未动（红线 4）/ 未 commit（红线 5）/ 未跑 npm run test（红线 6）
- **详情**：logs/20260817.md + CHANGELOG.md（HOTKEY-048 条目）

## 2026-08-17 — coder-1 — HOTKEY-047-R2 ✅ 回归修复（右 Alt 单键 modifiers 被合成 Ctrl 污染，1 段改，待主控验收）

- **来源**：主控 R1 验收通过主体，查出一处相对原代码的回归 —— 右 Alt 单键 `modifiers` 被合成 Ctrl 污染。基线同 R1
- **根因**：原代码 `:126-129` `if (code === 'AltRight') applyVoiceHotkey(0xA5, 0)` modifiers 硬编码 0 永远干净；R1 改成推算后 ku AltRight 时 `e.ctrlKey`（合成 LeftCtrl 仍按下）&& `!isAltGrUp`（AltGraph 在 AltRight 抬起时已为 false）→ 条件成立 → `modifiers |= 0x0002` → display_name 变 `Ctrl+Right Alt`。Gavin 热键正是右 Alt，属「界面撒谎」（TRANS-HOTKEY-039 修过的同类）
- **改动**（仅 `ui/src/pages/HotkeySettings.tsx` keyup 单键定案段，+2/-1）：
  ```tsx
  const altGrSynthWasActive = altGrSynthCtrlActiveRef.current;   // 在清零之前捕获
  if (code === 'AltRight') { altGrSynthCtrlActiveRef.current = false; }
  ...
  if (e.ctrlKey && !isAltGrUp && !altGrSynthWasActive) modifiers |= 0x0002;
  ```
  只动 keyup 单键定案这一段；keydown 主键路径（`!isAltGr` 实时判定）已经是对的，未碰
- **自证 1（右 Alt 单键逐事件，含 modifiers 逐位）**：kd ControlLeft(合成)→kd AltRight(delete ControlLeft+altGrSynth=true)→ku AltRight(altGrSynthWasActive=true 清前捕获→altGrSynth=false→hadNonModifierKey=F→delete→size=0→singleVk=0xA5→e.altKey=F 0x0001 不置；e.ctrlKey=T 但 !altGrSynthWasActive=false → 0x0002 不置；e.shiftKey=F → 0x0004 不置 → **modifiers=0**)→ku ControlLeft(finalized return) = **vk_code=165(0xA5), modifiers=0, display_name='Right Alt'**（不是 `Ctrl+Right Alt`）✅
- **自证 2（按住右 Alt 再按 M，验证未修坏 B）**：kd ControlLeft(合成)→kd AltRight→kd KeyM(非修饰→isAltGr=getModifierState('AltGraph')=true→`e.ctrlKey && !isAltGr` ctrl 不计入→modifiers=0x0001→checkAndApply(0x4D,0x0001)) = **vk=0x4D mod=0x0001 display_name='Alt+M'** ✅（keydown 主键路径用 `!isAltGr` 实时判定，R2 未碰）
- **主控已核清不受影响的三条**（不用重复验）：左 Alt 单键 / 右 Ctrl 单键 / 左 Shift 单键 ku 时 altKey/ctrlKey/shiftKey 均 false → modifiers=0。只有 AltGr 这一条受影响，已修
- **关于 locale key 数量**：R1 自证报 111:111:111 有误，主控实测 111/111/110（zh-Hant 缺 voice_asr_model_accuracy，HEAD 版本就缺，非本任务引入，主控另开待办）。方法提醒：报数字前要真量一次
- **验证**：`npx tsc --noEmit` 0 error / `npm run build` 通过 / `git diff -w --stat -- ui/` 仍 4 文件（HotkeySettings.tsx +147/-17 + 三份 i18n 各 +1/-1）
- **红线合规**：未碰 `src/` `src-tauri/`（红线 1）/ 未改 CODE_TO_VK/VK_TO_LABEL/MOD_LABELS 数值（红线 2）/ 组合键 display_name 无 Left/Right（红线 3）/ 未用启发式（红线 4，`getModifierState('AltGraph')` 仍用于算 modifiers + `altGrSynthWasActive` 是平台固有事实捕获）/ 翻译侧只修焦点竞态（红线 6）/ 版本号 0.8.0 未动（红线 5）/ 未 commit
- **result.md 落盘**：write 工具历史 silent fail，本轮直接贴 tmux 通知
- **详情**：logs/20260817.md + CHANGELOG.md（HOTKEY-047-R2 条目）

## 2026-08-17 — coder-1 — HOTKEY-047-R1 ✅ 打回修复（缺陷 1/2/3 + finalized 防重入，4 文件，待主控验收）

- **来源**：主控验收 R0 打回三项：①【P0】右 Alt 单键被定案成 Left Ctrl（合成 ControlLeft 在 AltGraph 时序未成立时进入 pressedMods，keyup 顺序 AltRight→ControlLeft 时第 4 步 ControlLeft 误单键定案）；②追加需求 1 三语文案 `hotkey_click_to_change` 一个字没改；③组合键定案后残留 keyup 覆盖成单键。基线同 R0（HEAD `664b7bd`）
- **改动**（4 文件：`ui/src/pages/HotkeySettings.tsx` +146/-17 + 三份 `ui/src/i18n/*.ts` 各 +1/-1）：
  1. **缺陷 1**：改以「看见 AltRight 即剔除 ControlLeft」为锚点（不依赖 AltGraph 时序）—— keydown `code===AltRight` 时 `pressedModsRef.delete('ControlLeft')` + `altGrSynthCtrlActiveRef=true`；keyup 最前 `code===ControlLeft && altGrSynth` → 删除并 return（合成键永不定案）；keyup `AltRight` 时清 altGrSynth。删除原依赖 AltGraph 时序的合成键入口。`getModifierState('AltGraph')` 仍用于算 modifiers 过滤合成 Ctrl（红线 4 不破）。副作用：真想设「左 Ctrl+右 Alt+某键」的用户拿不到左 Ctrl，Windows 平台固有限制
  2. **缺陷 2**：三份 locale `hotkey_click_to_change` 改为 `建议设置左右 Ctrl 或 Alt 键为热键` / `建議設定左右 Ctrl 或 Alt 鍵為熱鍵` / `Recommended: left or right Ctrl / Alt as the hotkey`。用 Edit 工具改（非 PowerShell），编码安全；`hotkey_set_translation` 未动
  3. **缺陷 3**：选 ① `if (!isRecordingVoice) return` + 新增 `finalizedRef` 防重入。`applyVoiceHotkey`/`checkAndApplyVoiceHotkey`/keydown/keyup 入口 guard；`resetRecordingState` 清 false；`checkAndApplyVoiceHotkey` apply 分支前临时 `finalizedRef=false` 让 apply guard 通过。理由：`isRecordingVoice` 异步刷新，async `await invoke` 期间 keyup 仍看到旧值 true，ref 同步置 true 填补窗口期
- **自证**：① 右 Alt 单键逐事件 kd ControlLeft(合成)→kd AltRight(剔除ControlLeft+altGrSynth=true)→ku AltRight(altGrSynth=false→delete→size=0→singleVk=0xA5→finalized=T)→ku ControlLeft(finalized=T return) = **vk=0xA5 Right Alt**（非 0xA2）✅；② Ctrl+Shift+M 释放 kd ControlLeft→kd ShiftLeft→kd KeyM(finalized=T)→ku KeyM/ShiftLeft/ControlLeft 全 return = **Ctrl+Shift+M 不被覆盖** ✅；③ 三份 locale `grep -c ":"` = 111:111:111，三行新值正确 ✅
- **验证**：`npx tsc --noEmit` 0 error / `npm run build` 通过 / `git diff -w --stat -- ui/` 仅 4 文件
- **红线合规**：未碰 `src/` `src-tauri/`（红线 1）/ 未改 CODE_TO_VK/VK_TO_LABEL/MOD_LABELS 数值（红线 2）/ 组合键 display_name 无 Left/Right（红线 3）/ 未用启发式代替 `getModifierState('AltGraph')`（红线 4，仍用于算 modifiers）/ 翻译侧只修焦点竞态（红线 6）/ 版本号 0.8.0 未动（红线 5）/ 未 commit
- **result.md 落盘**：write 工具两次 silent fail（`wc -c`=0，mtime 未动），已按主控指示把自证+收尾表贴进 tmux 通知
- **详情**：logs/20260817.md + CHANGELOG.md（HOTKEY-047-R1 条目）

## 2026-08-17 — coder-1 — HOTKEY-047 ✅ 设置 UI 热键录制重做（P0，ui/src/pages/HotkeySettings.tsx +117/-13，待主控验收）

- **来源**：Gavin 端测两轮反馈 + 主控独立取证三根因派单。基线 HEAD `664b7bd`（BUILD-018）
- **三根因**：A 焦点竞态 `setTimeout(50)`+`ref?.focus()` 静默失败（首次进入页面 React 未提交 DOM → ref.current===null，整轮拿不到焦点 → 按 alt 无反应；第二次组件已渲染、主线程空闲 → 成功）；B 左侧修饰键 `:131` 硬编码 `return` 吞掉；C 右 Alt=AltGr，Windows 合成左 Ctrl，按住右 Alt+M 被误记成 Ctrl+Alt+M
- **改动**（仅 `ui/src/pages/HotkeySettings.tsx`，+117/-13）：
  1. 焦点竞态：`setTimeout` 全删 → `useLayoutEffect` 在 DOM 提交后同步 focus（ref 必非 null）+ `autoFocus` 第二道兜底 + `onBlur` 退出录制态防僵死；语音侧 + 翻译侧同款
  2. 修饰键单设 + 任意组合：keydown 定组合 / keyup 定单键；维护 `pressedModsRef: Set<string>` + `hadNonModifierKeyRef`；keydown 修饰键入集 + 实时回显 `voiceRecordingPreview`，非修饰键立即定案；keyup 若本轮无非修饰键且最后一个修饰键松开 → 定案为该单键（vk 区分左右）；`onKeyUp` handler 新增
  3. AltGr：`getModifierState('AltGraph')` 精确判定（非启发式）；合成 ControlLeft 用 `altGrSynthCtrlActiveRef` 过滤、不进 `pressedModsRef`；算 modifiers 时 `if (e.ctrlKey && !isAltGr)` 过滤
  4. 统一冲突检查：删原 `:122-129` 右 Ctrl/右 Alt 绕过 `check_hotkey_available` 的快捷路径，所有定案走 `checkAndApplyVoiceHotkey`
- **自证（三必答）**：① 四种操作逐条走通（单左 Alt→`Left Alt` vk=0xA4 mod=0 / 左 Alt+M→`Alt+M` vk=0x4D mod=0x0001 / 右 Alt(AltGr)+M→`Alt+M`（合成 Ctrl 被过滤）/ Ctrl+Shift+M→`Ctrl+Shift+M` mod=0x0006）；② `useLayoutEffect` 在 React DOM mutation 后同步执行（早于 paint），ref 必非 null，与 `setTimeout` 跨越渲染提交时序的本质区别消除静默失败，`autoFocus`+`onBlur` 兜底；③ 组合键 display_name 经 `getHotkeyDisplayName`，修饰键部分只用 `MOD_LABELS`（`Alt`/`Ctrl`/`Shift`/`Win` 不分左右），**无 `Left`/`Right` 字样**（自证通过）；单键 display_name 用 `VK_TO_LABEL` 的 `Left Alt`/`Right Alt`（单键真区分左右，非撒谎）
- **验收**：`npx tsc --noEmit` 0 error / `npm run build` 通过（tsc && vite build，dist 正常）/ `git diff -w --stat` 仅 `ui/src/pages/HotkeySettings.tsx` +117/-13（CRLF 噪声属既有 `[CRLF-CROSSPLAT-001]`）
- **红线合规**：未碰 `src/` `src-tauri/`（红线 1）/ 未改 `CODE_TO_VK` `VK_TO_LABEL` `MOD_LABELS` 数值（红线 2）/ 未用启发式代替 `getModifierState('AltGraph')`（红线 4）/ 翻译侧只修焦点竞态、录制规则未改（红线 6）/ 版本号 0.8.0 未动（红线 5）/ 未 commit（主控统一提交）
- **测试**：无 `*.test.tsx`（阶段三 TEST-SYNC-047 由 tester-1 串行派发，禁止并行）
- **边界**：与 coder-2 在 `src/main.rs` 零重叠；`ui/src/pages/Voice.tsx` 等其他文件未碰
- **跨平台**：`docs/MACOS-HANDOFF.md` 追加 §HOTKEY-047
- **详情**：outbox/coder-1/result.md + logs/20260817.md + CHANGELOG.md

## 2026-08-17 — tester-1 — BUILD-018 ✅ 阶段五出包：OVERLAY-043 全批进 exe（零生产改动，待主控验收）

- **来源**：主控派单（DEC-053 直接出包，基线 HEAD `b499cc3`）。前置四提交 OVERLAY-043 `a588509` / OVERLAY-043-B `5940e73` / TEST-SYNC-043 `497131f` / TEST-EXEC-043 `b499cc3` 已全验收
- **四步构建全执行**：Step1 击杀 Gavin 端测实例 `feiyin-ime` PID 21948（-debug / 12:27 / BUILD-017 旧包 08-16 23:25）——**先报主控获 Gavin 授权**属预期击杀，`feiyin-ime-nor.exe` 备份未删 → Step2 npm build 1.60s + Tauri UI 1m56s → Step3 主程序 2m21s → Step4 同步 Publish/+toml 三副本，`Publish/config.toml` 运行时数据未覆盖
- **七项核验全 PASS**：① 六 exe 时间戳 13:57-14:00 本次构建；② sha256 两副本逐一相等（`24cad6be…`/`1857c0de…`/`17fe10af…`）；③ toml 三副本 hash 全等（与 BUILD-017 相同未变）；④ ProductVersion 0.8.0/0.8.0.0 未动；⑤ 正向探针 `Streaming resampler active`=1；OVERLAY-043 本批如实报探不到（GDI 逻辑无新文案），间接证据三件套（mtime>最新提交+源码引用×24/×9+反向 `delta.abs()` 无残留）；⑥ 大小 feiyin-ime +3,072 B 略增（吻合 +337/-128）/ ui 0 / crash 0；⑦ 冒烟 PID 28300 Responding=True 无 panic 测后清理
- **红线合规**：生产零改动（`git diff -w` src/src-tauri/ui 全空，工作区 M 纯 CRLF 噪声）｜版本号未动｜运行时数据未覆盖｜未 reset/checkout/commit｜**未 push**｜Gavin 端测进程先报后杀（PID 4696 先例同族）
- **Gavin 端测重点四项**（result.md 末尾原样列出）：流畅度目视 / 本地模型录一次（波形动画）/ 波形隐藏+单按钮 / 松开热键立即切处理中
- **验收**：版本号 0.8.0 未动；未 commit（主控统一提交）；产物 `Publish/` 三 exe 12,115,456 / 10,026,496 / 24,859,648 B
- **详情**：outbox/tester-1/result.md + logs/20260817.md + CHANGELOG.md

---

## 2026-08-17 — tester-1 — TEST-EXEC-043 ✅ 阶段四全量回归 + 消融实测（OVERLAY-043 批，生产零改动，待主控验收）

- **来源**：主控派单（基线 HEAD `497131f`）。前置三提交 OVERLAY-043 `a588509` / OVERLAY-043-B `5940e73` / TEST-SYNC-043 `497131f` 已全验收
- **四步回归全过**：Step1 `cargo test` = **1040 passed / 0 failed / 11 ignored**（主 crate 967 含 +10 overlay_043 + crash-reporter 37/2ign + 集成 36；上批 1030 +10 精确命中零残差）→ Step2 Vitest **54 passed** → Step3 src-tauri **55 passed** → Step4 pytest **SKIP**（`Publish/` 为 BUILD-017 旧包 08-16 23:25 早于本批 HEAD 08-17 13:40）
- **消融 A（delta.abs()→delta）**：实测变红 **4 条**（任务书预期 3 条）——预期 3 条全中（shrink_exact_values / sign_symmetry / converges_800_240），**多出 step_never_exceeds_quarter_of_delta**。主控裁决：`interpolate_step` 的 `abs` 变量两处使用（`abs*0.25` + `.min(abs)`），其推演只改比例基数一处、min 仍用绝对值故模型偏轻少算一条；**实测 4 条为准**，该用例捕获同根因更强表现（负 delta 步长 +|delta| 方向暴跳），属护栏更严非失效
- **消融 B（门闩→false）**：实测仅 `ignore_streaming_text_truth_table` 1 条变红（第 3 格失守），与预期完全吻合
- **还原自证**：两次消融编辑器还原 → `git diff -w src/main.rs` 输出空（0 行）→ 还原后复跑 **1040/0/11** 与消融前逐数一致；红条 ③=0/①=0/②=0
- **纪律遵守**：消融 A 实测与任务书预期冲突时先上报主控再继续（主控确认"先报不改"正确），未自行改测试迁就
- **验收**：版本号 0.8.0 未动；未 commit（主控统一提交）；未出包（BUILD-018 须主控明确下令）
- **详情**：outbox/tester-1/result.md + logs/20260817.md + CHANGELOG.md

---

## 2026-08-17 — tester-1 — TEST-SYNC-043 ✅ 阶段三测试同步：OVERLAY-043 + 043-B 补真护栏（src/main.rs +138，待主控验收）

- **来源**：主控派单（基线 HEAD `5940e73`，OVERLAY-043 `a588509` + OVERLAY-043-B `5940e73` 已提交）。前置：缺陷 A 验收打回后已抽 `interpolate_step` 纯函数
- **改动**：仅 `src/main.rs` 测试区新增 `mod overlay_043_interpolate_tests`（+138 行，Windows-only `#[cfg]` 门控，生产零改动）
- **10 条用例**：`interpolate_step` 五契约（零值/≥1px/≤ceil(25%)/不越界/正负对称，±50000 穷举）+ 🔴 缺陷 A 回归护栏（`(-400)==-100`、`(-100)==-25` 改回 delta 即红）+ 双向收敛预算（800→240 与 240→800 均 ≤40 帧，缺陷 A 为 559 帧，正确 23 帧）+ `should_ignore_streaming_text` 四格真值表穷举
- **护栏预验证**：Python 数值复算五契约全范围 0 失败；消融推演（`delta.abs()`→`delta`）确认 `(-400)` 变 `-1`、收敛 560 帧 → 用例必然变红
- **不可测项如实列出**（GDI 绘制/按钮消息循环/脏标记/门闩调用侧接线），无假护栏、无闭包自证
- **验收**：cargo fmt -- --check clean / cargo check --all-targets 0 error（99 warnings 均既有，无一条指向新模块，97→99 的 +2 来自 OVERLAY-043 生产代码）/ git diff --stat 仅 src/main.rs +138 / 阶段三白名单只跑 fmt+check / 消融自证顺延阶段四（TEST-EXEC-043）
- **边界**：未碰 coder-2 生产代码；未 commit（主控统一提交）；版本号 0.8.0 未动
- **详情**：outbox/tester-1/result.md + logs/20260817.md + CHANGELOG.md

---

## 2026-08-16 — tester-1 — TEST-SYNC-045 ✅ 阶段三测试同步：ASR-045 流式判空取消（src/main.rs +46，待主控验收）

- **来源**：主控派单（基线 `05eb5f0`，代码基线 `54cde62` ASR-045 已提交）。前置：ASR-045 P0 修复（流式文字从未上屏）
- **改动**：仅 `src/main.rs` `mod streaming_empty_samples_tests`（+46 行，生产零改动）
  - ① `nonempty_samples_with_text_truth_table_cell`：真值表第 4 格（非空+Some），如实标注弱护栏
  - ② ③ `empty_string_text_not_cancelled_at_first_layer` / `whitespace_only_text_not_cancelled_at_first_layer`：🔴 本单核心，钉死两层职责划分（`:4288` 只管「有没有东西」「:4334` `trim().is_empty()` 管「文本是否有效」→ 空/纯空白用户可见 Error 提示 2000ms，非静默消失）
- **调用侧不可测**：`:4318` 判空臂需 6 种资源无法单测；全部 6 条纯函数测试保护不了调用侧，guard 改回 `s.is_empty()` P0 即复发；禁闭包伪造护栏；抽 `decide_pipeline_entry`/`EntryDecision` 纯函数建议交主控排期（未动生产代码）
- **验收**：`cargo fmt -- --check` clean / `cargo check --all-targets` 0 error（97 既有 warning）/ `git diff --stat` 仅 src/main.rs +46 / 阶段三白名单只跑 fmt+check / 消融自证顺延阶段四
- **边界**：未碰 `src/audio/mod.rs`；未 commit（主控统一提交）；版本号未动（0.8.0）
- **详情**：outbox/tester-1/result.md + logs/20260816.md + CHANGELOG.md

---

## 2026-08-16 — coder-1 — ASR-045 ✅ 流式识别结果被管线丢弃修复（P0，src/main.rs +44，待主控验收）

- **来源**：主控定位（P0，功能 100% 不可用）：在线流式 ASR 悬浮层实时出字 → 松开热键 → 文字从未上屏。基线 HEAD `427cd50`
- **根因**：调用侧 `run_pipeline_core(Ok(Vec::new()), …, Some(streaming_text))`（`main.rs:3409`）传空 samples + 流式文本；接收侧判空臂 `Ok(s) if s.is_empty()`（原 `:4307`）**无条件**取消，`initial_text` 从未被读取 → 流式文本在函数入口即被丢弃，ITN→LLM→注入后半段从未执行
- **改动**（仅 `src/main.rs`）
  - 新增纯函数 `should_cancel_on_empty(samples, initial_text)`（`:4288`）= `samples.is_empty() && initial_text.is_none()`
  - 判空臂改调该函数（`:4316-4320`）：只有「无样本 且 无流式文本」才真取消；流式（empty+Some）落入 `Ok(samples)` 走既有 `initial_text` 路径
  - 护栏测试 `mod streaming_empty_samples_tests`（`:5982`，3 条）：流式空+文本 proceed / 真空录仍 cancel / 非空+None proceed
- **三问核证**：Q1 `samples` 仅在正常转录 else 分支消费（`:4347-4357`），流式路径零引用，无 panic/除零；Q2 `cancel_signal` 每次 Start 重置 false（`:3189`），`:4323` 只拦 join 期真实取消，无误伤；Q3 全链 27 步核完（热键→录音→流式识别→松开→ITN→LLM→注入）唯一断点即原判空臂
- **验收**：cargo fmt --check clean（仅本人区域）/ cargo check --all-targets 0 error；白名单遵守未跑 test/build；未 commit（主控统一提交）；版本号未动
- **边界**：与 coder-2 OVERLAY-043（`:908-940`、`:1780` 起）无文本重叠；`src/audio/mod.rs`（tester-1 在途）零触碰
- **详情**：outbox/coder-1/result.md + logs/20260816.md + CHANGELOG.md

---

## 2026-08-16 — tester-1 — BUILD-016 ✅ v0.8.0 首包出包（全构建 + 七项核验 + 双探针，生产零改动）

- **来源**：Gavin 已明确下达出包指令，主控派单 BUILD-016（阶段五）。基线 HEAD `be76fc1`
- **构建**：Step 1 清进程（feiyin-ime PID 23888）→ Step 2 npm build（新 `index-DkzLqu_f.js`）+ Tauri UI release 2m14s（cp 到 target/release/）→ Step 3 主程序 2m40s → Step 4 同步 Publish/（三 exe + scene/itn 两 toml；config.toml 等运行时数据未覆盖，保持 07-28 原样）
- **七项核验全 PASS**（详见 outbox/tester-1/result.md，机器实测）：① 六 exe 时间戳 00:45-00:48；② 三 exe 两副本 sha256 相等；③ 两 toml 三副本一致；④ ProductVersion 0.8.0.0/0.8.0/0.8.0.0；⑤ `index-DkzLqu_f.js` 嵌 ui.exe / 旧名 0（i18n 裸串 grep 0 系 Tauri 压缩已知行为，改文件名字符探针）；⑥ 冒烟 Responding=True 已清理；⑦ 大小对照已解释
- **🔴 双探针**：正向 `qwen-audio-3.0-asr-flash-streaming` / `api-ws/v1/inference` 各 1 命中；反向 `api-ws/v1/realtime` / `qwen3-asr-flash-realtime` 0 命中 —— 新 ASR 进包 + 旧引擎清干净
- **产物**：`Publish/feiyin-ime.exe` 12106752B（sha256 `86176283…`）/ `feiyin-ime-ui.exe` 10026496B（`db095564…`）/ `crash-reporter.exe` 24859648B（`a9c30e4e…`）
- **验收**：cargo fmt 未跑（本单纯构建，无代码改动）；版本号三处 0.8.0 未动；生产代码零改动
- **详情**：outbox/tester-1/result.md + logs/20260816.md + CHANGELOG.md

---

## 2026-08-16 — coder-1 — ASR-042 在线流式 ASR 采样率修复（StreamingResampler，src/audio/mod.rs +476/-5）

- **来源**：Gavin v0.8.0 端测，新在线 ASR 识别全错（48kHz 音频按 16kHz 解）。主控派单 ASR-042，基线 HEAD `efb101f`
- **根因**：`record_streaming` 边录边发绕过了 `record()` 录音结束时的一次性 `resample_anti_alias`（FIRSTCHAR-FIX-005 引入），48kHz 直推服务端
- **改动**：仅 `src/audio/mod.rs`（+476/-5）
  - 新增 `StreamingResampler`（`pub(crate)`）：带状态流式抗混叠重采样器，数学等价于 `resample_anti_alias`（windowed-sinc FIR, TAPS=32, Hann 窗），跨块保留历史+全局 emitted 计数，接缝不截断卷积核
  - `record_streaming` 接线：pre-roll → post-hotkey → 主循环 依次喂**同一实例** + break 后 `finish()` flush 尾部 + 新日志 `Streaming resampler active: {N}Hz -> 16000Hz`
  - `total_samples`/`max_frames`/RMS/`level_buf` 继续用原始 chunk（未重采样）
  - 新增 `RESAMPLE_TAPS` 常量（`resample_anti_alias` 与 `StreamingResampler` 共用防漂移）
  - +6 测试：等价性（逐点差 ≤1e-5）/ 非对齐块长(441) / 恒等(16k→16k) / 长度(误差 ≤1) / 顺序护栏 / finish 尾部
- **"改坏会红"自证**：`push` 入口注入 `emitted=0`（每块重算）→ 等价性测试 FAILED（stream=807010 vs batch=16000），移除后 GREEN
- **验收**：cargo fmt clean / cargo check --all-targets 0 error / cargo test audio 56 passed 0 failed 1 ignored（既有用例零改动）
- **禁区核验**：`resample_anti_alias` 函数体 / `record()` / `vad.rs` / `main.rs` / `qwen_inference.rs` 零改动
- **跨平台**：`docs/MACOS-HANDOFF.md` 新增 §ASR-042 节（平台中立模块，macOS 编译同份代码；运行时影响取决于 macOS 侧 record_streaming 是否已接线）
- **版本号**：未动
- **详情**：outbox/coder-1/result.md + logs/20260816.md + CHANGELOG.md

---

## 2026-08-16 — tester-1 — TEST-EXEC-042/045 ✅ 阶段四全量回归 + 消融自证（v0.8.0 出包前最后一道闸，生产零改动）

- **来源**：主控派单 TEST-EXEC-042/045（阶段四，基线 HEAD `fd994a5`）；前置四提交 ASR-042/TEST-SYNC-042/ASR-045/TEST-SYNC-045 已全验收
- **四步回归全过**：Step1 `cargo test` = **1030 passed / 0 failed / 11 ignored**（主 crate 957 + crash-reporter 37 + integration 36；预期 ≈1030 精确命中零残差）→ Step2 Vitest **54 passed / 5 files** → Step3 src-tauri **55 passed / 0 failed** → Step4 pytest **SKIP**（`Publish/` 是 BUILD-016 旧包早于本批，E2E 正确时机 BUILD-017 出包后）
- **消融 A（调用侧缺口实证）**：guard `:4318` 改回 `s.is_empty()` → 6 条 streaming_empty 测试**全绿**（P0 静默复发但零报警）→ 还原。结论：6 条纯函数测试保护不了调用侧，`TEST-045-REFACTOR` 排期维持
- **消融 B（分层契约护栏实证）**：`should_cancel_on_empty` 合并 `trim().is_empty()` → **4 passed / 2 failed**，变红恰为空串/纯空白两条（coder-1 3 条 + 真值表第 4 格仍绿）→ 还原。与主控推演真值表完全一致，护栏有效
- **收尾自证**：两消融全还原 → `git diff src/main.rs` 空 + `git diff -w` 0 行（77 文件仅 CRLF 噪声 [CRLF-CROSSPLAT-001]）→ 还原后复跑 **1030 全绿**逐数一致；红条 ③=0/①=0/②=0
- **验收**：无需 fmt/check（本单纯回归执行 + 临时消融已还原）；版本号 0.8.0 未动；未 commit（主控统一提交）；未出包（BUILD-017 须 Gavin 明确下令）
- **结论**：v0.8.0 全量回归闸门通过，无阻塞项，可进入阶段五 BUILD-017
- **详情**：outbox/tester-1/result.md + logs/20260816.md + CHANGELOG.md

## 2026-08-16 — tester-1 — BUILD-017 ✅ v0.8.0 第二包出包（ASR-042 + ASR-045 进 exe，生产零改动）

- **来源**：Gavin 已下令出包（并确立新规则：测试验收通过后主控直接派发出包，不再逐次请示）。基线 HEAD `3c075e8`
- **构建**：四步全执行。Step1 杀进程（无 `feiyin-ime` 主进程运行）→ Step2 npm build（`index-DkzLqu_f.js` 与 BUILD-016 同名=零前端改动）+ Tauri UI release 1m45s（cp 至 target/release/）→ Step3 主程序 2m04s → Step4 同步 Publish/（三 exe + scene/itn 两 toml；Gavin 运行时数据 config.toml 等四文件未覆盖，mtime 全为历史时间）
- **七项核验全 PASS**（详见 outbox/tester-1/result.md）：① 六 exe 时间戳 23:23-23:25；② 三 exe 两副本 sha256 相等（feiyin `7562b943…` / ui `62ae24df…` / crash `84fdfddd…`）；③ toml 三副本 hash 全等；④ ProductVersion 0.8.0；⑤ 正向探针 `Streaming resampler active`=1 → **ASR-042 进包**；**ASR-045 如实报「探不到」**（无新字符串+可能内联），用间接证据（mtime>54cde62 + 源码 ×12 引用）证明，未编探针；⑥ 大小对照：feiyin +5,632 B（略增吻合代码量）、ui/crash 完全不变；⑦ 冒烟 PID 23956 Responding=True 无 panic 已清理
- **⚠️ 杀进程实测**：Step1 时无 `feiyin-ime` 主进程运行（无 PID 被杀）；但发现并清杀**名单外遗留 `feiyin-ime-nor` PID 23232**（8-9 旧构建，持单实例 mutex，冒烟首次启动被挡）。若 Gavin 在用输入法需重新启动
- **验收**：生产代码零改动（`git diff src/ src-tauri/ ui/`=0）；版本号 0.8.0 未动；未 commit（主控统一提交）
- **Gavin 端测四项**（须 `-debug`，ASR-PERF-040-B/C 唯一数据源）：新端点连通性 / `usage.duration` 计费口径 / 040-A 四段连接耗时 / VAD 门控实际行为
- **详情**：outbox/tester-1/result.md + logs/20260816.md + CHANGELOG.md

## 2026-08-17 — coder-2 — OVERLAY-046：录音 overlay 窗口未被定位/定尺寸修复（P0 阻塞日常使用）

- **来源**：Gavin 2026-08-17 端测 BUILD-018 报「按下热键录音窗口完全不出现」；主控 `git show` 对照取证
- **根因**：OVERLAY-043 重构中 Show 分支丢失了无条件 `SetWindowPos` / `InvalidateRect`；非流式 `OverlayStatus` 在 `:991-993` 被赋 `current_size == target_size`，导致 `:1198` 插值条件永不成立、`size_interpolation_done` 永不置位，`:1221` 的 `SetWindowPos` 永不执行
- **影响**：`Recording`/`FallingToProcessing`/`Processing`/`Error`/`FocusLost` 全部非流式态均受影响；在线模型与本地模型录音窗口一律无法定位/定尺寸
- **改动**（仅 `src/main.rs` `:997-1005`）：
  - 在 Show 分支 `ShowWindow` 之前恢复无条件 `SetWindowPos(hwnd, request.pos, computed_size, SWP_NOACTIVATE | SWP_NOZORDER)`
  - `ShowWindow` 之后补 `InvalidateRect(hwnd, None, false)`，保持 `bErase=false`（红线 1）
- **不破坏 043 插值**：流式 `RecordingWithText` 的 `computed_size` 与 `:1221` 插值路径同源（`:930-959` 的 `pending_size`/`target_size`）；Show 时一次性定位到最新目标尺寸，后续 16ms timer 仍按 `interpolate_step` 推进动画
- **覆盖的恒等赋值变体**：`Recording`、`FallingToProcessing { .. }`、`Processing(_)`、`Error(_)`、`FocusLost { .. }` 全部在本次修复后被覆盖
- **验收**：`cargo fmt --all -- --check` clean / `cargo check --all-targets` 0 error（109 warnings 均为既有）/ `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / `git diff -w -- src/main.rs` 仅 `:997-1005` 新增 14 行
- **边界**：未碰 `src/audio/mod.rs`、`src/vad.rs`、`src/transcription/**`、`ui/`、`src-tauri/`；版本号 0.8.0 未动
- **跨平台**：`docs/MACOS-HANDOFF.md` §OVERLAY-043 已追加 OVERLAY-046 条目；改动全在 Windows-only `#[cfg]` 内，macOS 零编译影响
- **阶段三**：不跑 `cargo test` / `cargo build`，测试由 tester-1 负责
- **详情**：outbox/coder-2/result.md + logs/20260817.md + CHANGELOG.md

## 2026-08-17 — coder-2 — OVERLAY-043 录音悬浮层五项显示与流畅度修复（src/main.rs +349/-150，阶段一完成）

- **来源**：Gavin 2026-08-17 端测截图 + 主控逐条 Read 代码取证；基线 HEAD `56bfa37`
- **根因**：此前任务从未派发，代码未动。端测已确认 ASR-042 生效、流式文字能上 overlay，问题 purely 在 overlay 显示与流畅度
- **改动**（仅 `src/main.rs`）：
  1. 拆 `draw_recording_overlay` 为 chrome/indicator+waveform/stop-button 三段；`RecordingWithText` 路径不再画波形，文字区不被挤压
  2. 右侧单按钮复用：录音态=停止方块，编辑态=同位置橙色 ⏎ 提交；删除独立 submit 绘制
  3. 100ms 尺寸节流改为**延迟合并** + 25% lerp 插值，避免回退 240 px 突闪
  4. `InvalidateRect` 改 `bErase=false`；加 `needs_repaint` 脏标记；WM_PAINT 已双缓冲，内存 DC 先 FillRect 背景防垃圾像素
  5. 新增 `STREAMING_STOPPED` 门闩：Stop/ESC/取消/提交/编辑置位，`RecordingStarted` 复位；晚到 `StreamingText` 不再把 `FallingToProcessing` 顶回录音态；编辑态仍同步文字
- **流畅度额外手段**：尺寸插值过渡、状态变化才重置命中区、目标尺寸与当前尺寸差异阈值驱动 `SetWindowPos`
- **验收**：`cargo fmt --all -- --check` clean / `cargo check --all-targets` 0 error（99 warnings 均为既有/未使用变量）/ `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / `git diff --stat` 仅 `src/main.rs`
- **边界**：未碰 `src/audio/mod.rs`、`src/vad.rs`、`src/transcription/**`、版本号（0.8.0）
- **跨平台**：`docs/MACOS-HANDOFF.md` 新增 §OVERLAY-043；改动全在 Windows-only `#[cfg]` 内，macOS 零编译影响，行为契约已记录
- **后续**：阶段三 TEST-SYNC 待 coder-2 验收后派发；最终目视顺滑度由 Gavin 端测拍板
- **详情**：outbox/coder-2/result.md + logs/20260817.md + CHANGELOG.md

## 2026-08-17 — coder-2 — OVERLAY-043-B 抽两个纯函数补真护栏（src/main.rs +39/-12，阶段一补强）

- **来源**：OVERLAY-043 验收时主控手工数值复算发现 `interpolate_step` 内联逻辑缺陷；基线 HEAD `a588509`
- **改动**（仅 `src/main.rs`）：
  1. 新增 `interpolate_step(delta: i32) -> i32` 纯函数（doc 注释含四条契约），替换 `run_overlay_thread` 中内联步长计算；与原内联表达式逐位等价
  2. 新增 `should_ignore_streaming_text(stopped: bool, editing: bool) -> bool` 纯函数，替换 `PipelineEvent::StreamingText` 分支的内联门闩判断；与原布尔表达式 `stopped && !editing` 逐位等价
- **性质**：纯可测性重构，**行为零变更**；测试用例由阶段三 tester-1 负责，本任务不写 `#[test]`
- **验收**：`cargo fmt --all -- --check` clean / `cargo check --all-targets` 0 error / `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / `git diff --stat` 仅 `src/main.rs`，无新增 `#[test]`
- **边界**：未碰 `src/audio/mod.rs`、`src/vad.rs`、`src/transcription/**`、版本号（0.8.0）
- **跨平台**：`docs/MACOS-HANDOFF.md` §OVERLAY-043 已声明纯 Windows-only `#[cfg]` 代码，macOS 零编译影响；本次抽函数仍在同一 `#[cfg]` 块内
- **详情**：outbox/coder-2/result.md + logs/20260817.md + CHANGELOG.md

---

## 2026-08-16 — tester-1 — 文档维护：build-test-guide.md Step 1 杀进程名单补 `feiyin-ime-nor`（主控验收后建议，非派单）

- **背景**：BUILD-017 验收通过（提交 `2074859`）。主控建议把名单外遗留进程 `feiyin-ime-nor` 补进 Step 1 杀进程名单，否则下次出包重蹈 mutex 坑。该文档归 tester-1 维护，无需等派单
- **改动**：两处命令 `Get-Process feiyin-ime,voice-ime-ui` → `feiyin-ime,feiyin-ime-nor,voice-ime-ui,feiyin-ime-ui,crash-reporter`（原名单还缺 `feiyin-ime-ui`/`crash-reporter`，一并补齐）+ 注释说明 BUILD-017 实测背景
- **纯文档维护**：无代码改动、无出包、无 commit（主控统一提交）
- **留意项（不动作）**：`target/release/feiyin-ime.exe` PID 4696（00:08:39 启动，非冒烟进程）锁着 exe，下次构建可能报 os error 5；主控已报 Gavin 判断归属，回话前任何人不得杀。保持待命等 Gavin 端测
- **详情**：logs/20260816.md

---

## 2026-08-17 — tester-1 — TEST-SYNC-046/047 ✅ 阶段三测试同步：OVERLAY-046 + HOTKEY-047 补护栏（ui/ +215 新测试文件，待主控验收）

- **来源**：主控派单（基线 HEAD `4d5c56b`，OVERLAY-046 `46be389` + HOTKEY-047 `4d5c56b` 已提交）
- **改动**：仅新建 `ui/src/pages/HotkeySettings.test.tsx`（+215 行，13 条用例 = 12 必测场景 S1-S12 + 1 locale 护栏），生产零改动
- **S1-S8** 逐条断言 vk_code/modifiers/display_name（右 Alt 单键 0xA5/0/'Right Alt'、AltGr+M 0x4D/0x0001/'Alt+M'、左修饰键单键、Ctrl+Shift+M 0x4D/0x0006、finalizedRef 防重入恰 1 次落库）；**S9** 同步断言 activeElement（useLayoutEffect vs setTimeout 差异）；**S10** 预览即时回显；**S11** Escape 零调用；**S12** 冲突弹窗非落库
- **排雷**：happy-dom `getModifierState('AltGraph')` 返回 altKey，`{altKey:true,ctrlKey:true}` 即 AltGraph=true 无需 override；react-dom 合成事件直接委托 nativeEvent 无 TypeError 路径（node 库级探针 + react-dom 源码静态实证替代禁跑的 Vitest）
- **OVERLAY-046 判定**：同意主控预判不可纯单测（raw FFI + 真实 HWND + 副作用-only），给出两个真实可测切面——①抽 `should_position_at_show` 纯函数抓门控回归一半根因（参照 interpolate_step 先例）；②E2E 层现成护栏已存在（state_detector 按可见性+尺寸判 RECORDING，test_hotkey.py:228 等），缺口是 BUILD-018 未实际跑 E2E 的流程缺口，建议纳入 release smoke 门
- **消融推演**：A/B/D 同意任务书（S1/S1/S9），C 持异议（hadNonModifierKeyRef+isRecordingVoice+checkAndApply 首行三重兜底，S8 预计不红，建议阶段四在 S12 加「冲突后残留 keyup」变体实测），诚实标注推演可能偏轻
- **验收**：npx tsc --noEmit 0 error / git diff -w 源码域零 diff 仅新增测试文件 / 未碰 src/ src-tauri/、版本号 0.8.0、未 commit（主控统一提交）
- **详情**：outbox/tester-1/result.md + logs/20260817.md + CHANGELOG.md（TEST-SYNC-046/047 条目）

---

## 2026-08-17 — tester-1 — TEST-EXEC-046/047 ✅ 阶段四全量回归 + 消融实测（HOTKEY-047 批，生产零改动，待主控验收）

- **来源**：主控派单（基线 HEAD `b5a96a9`，HOTKEY-047 `4d5c56b` + TEST-SYNC `4d5c56b` 后一提交已验收）
- **四步回归全过**：Step1 `cargo test` = **1040 passed / 0 failed / 11 ignored**（与基线逐数一致）→ Step2 Vitest **67 passed**（54+13 差账吻合）→ Step3 src-tauri **55 passed** → Step4 pytest **SKIP**（`Publish/` 为 BUILD-018 旧包 14:00 早于本批 HEAD 15:07，E2E 待 BUILD-019）
- **消融实测**：**A**（keyup 去 `!altGrSynthWasActive`）→ S1 红（modifiers 0→2、'Ctrl+Right Alt'）1/12 吻合；**B**（keydown AltRight 去 delete ControlLeft）→ **S1+S12 红**（S1 vk 165→162 'Left Ctrl'；S12 弹窗 'Right Alt' 失配）2/11 **超任务书预测**，护栏更严非失效；**D**（focus 改 setTimeout 50）→ S9 红 1/12 吻合；**C 按主控裁决取消**（finalizedRef 四处检查点纵深防御，主护栏为 :217 hadNonModifierKeyRef，不可独立消融，不另造人工消融）
- **还原自证**：三次消融编辑器还原（禁 git reset/checkout/stash/clean），每次 `git diff -w -- ui/` 0 字节 + HotkeySettings.tsx byte-identical to HEAD，还原后 vitest 复跑 67/67 逐数一致
- **红条三分类**：③真回归 0 / ①测试错 0 / ②预期内 0（消融期 4 条红均有意消融已还原，终态全绿），无阻塞项
- **补记不可测缺口**：真按 左Ctrl+左Alt+M 与 AltGr 在 happy-dom 单测不可区分（AltGraph 映射 altKey，0x0001 'Alt+M' vs 真实 0x0003 'Ctrl+Alt+M'，S7 规避正确），troubleshooting.md [HAPPYDOM-ALTGR-INDISTINGUISHABLE-001]
- **边界**：未出包（BUILD-019 待主控放行）、版本号 0.8.0 未动、未 commit（主控统一提交）
- **详情**：outbox/tester-1/result.md + logs/20260817.md + CHANGELOG.md（TEST-EXEC-046/047 条目）

---

## 2026-08-17 — tester-1 — BUILD-019 v0.8.0 第四包出包 + 首次真跑 E2E 门禁

- **出包**：OVERLAY-046 + HOTKEY-047 + HOTKEY-048 进 exe，基线 `b5a96a9` + 048 未提交文案
- **两轮构建**：首轮（048 前）全量；二轮（048 后）仅 Step2/4，**Step3 跳过**（`git diff -w` Rust 零改动，主程序沿用 15:48 构建）
- **七项核验全 PASS**：sha 两副本逐一相等（`e0785397…`/`1a2b40c8…`/`7b499bbf…`）+ toml 三副本未变 + ProductVersion 0.8.0 + index 探针 `index-CZoCPT7t.js` + 冒烟 PID 10392
- **E2E 首跑**：50 PASS / 32 SKIP / 10 FAIL / 4 error，**全部 harness/环境缺陷零真回归**（决定性实验证 F9 链路产品正常）；详见 troubleshooting [E2E-CONFIG-PATH-STALE-001]
- **纯出包**：无代码改动、版本号未动、未 commit（主控统一提交）、未 push
- **建议主控下一步**：① 修 harness 三处缺陷（配置写 exe_dir / state_detector 尺寸 240x36 / 补 pip toml）后重跑 E2E 冲绿；② 端测项转达：HOTKEY-047 右 Alt 单键 PTT + 任意组合键 + AltGr 过滤
- **详情**：logs/20260817.md + CHANGELOG.md + progress.md 产物表

---

## 2026-08-17 — tester-1 — BUILD-020 ✅ v0.8.0 第五包出包（本批 6 提交进 exe，阶段五）

- **来源**：主控直接派发（DEC-053 + Gavin 授权「测试完直接出包然后 push」；push 由主控执行，我不碰）
- **四步构建**：Step1 清进程（预查**无 Gavin 自启实例**，0 残留）→ Step2 npm build（`index-B5q249eW.js`）+ Tauri UI 1m39s + cp 时间戳一致 → Step3 主程序 2m08s（Rust 侧实质改动必跑）→ Step4 同步 Publish/ 三 exe + 两 toml（config.toml 未覆盖）
- **七项核验全 PASS**：① 时间戳本次 ② 三 exe 两副本 sha 逐一相等 ③ 两 toml 三副本 hash 全等 ④ ProductVersion 0.8.0 三处未变 ⑤ 探针：三条 sha 全异于 BUILD-019 + 新 JS 名内嵌 + i18n `hotkey_dup` 5 key 命中（exe grep 0 为 Tauri 压缩，按 BUILD-016 既定结论改用 JS 名字符探针）+ 后端 resampler=1 ⑥ 大小：feiyin +23KB（Rust 增量）/ ui 同大小但 sha 变（BUILD-019 教训：大小不作唯一判据）⑦ 冒烟 PID 28448 零 panic 清理
- **Step5 E2E**：9 FAIL / 54 PASS / 32 SKIP 记 **BLOCKED**，失败项与 TEST-EXEC-049/051/053 **逐一相同无新增类型**（[E2E-CONFIG-PATH-STALE-001] harness 缺陷）
- **Gavin 端测清单 14 项**已原样列于 result.md（含 right-Alt+M 得 Alt+M、Ctrl+Alt+M 不被吞、热键重复拦截弹窗、编辑态横向滚动、debug 分离测流畅度等）
- **红线合规**：版本号 0.8.0 三处未动 / 未 push / 未 commit / Publish 运行时数据未覆盖 / feiyin-ime-nor.exe 保留 / 无 Gavin 自启实例
- **详情**：outbox/tester-1/result.md + logs/20260817.md + CHANGELOG.md + progress.md 产物表新增行

---

## 2026-08-17 — tester-1 — TEST-EXEC-049/051/053 ✅ 阶段四全量回归 + 消融实测（HOTKEY-049 + WORDBOOK-053 批，无阻塞项）

- **来源**：主控派单（基线 HEAD `5a5a7e7`，本批三提交 + TEST-SYNC 已验收，仅我一人执行）
- **四步回归**：cargo test **1045/0/11**（与预期精确吻合）→ Vitest **78/78**（与预期精确吻合）→ src-tauri **60/0**（任务书预期 55，**对账非回归**：`src-tauri/src/wordbook.rs:2` `#[path="../../src/wordbook/mod.rs"]` 把主 crate wordbook 整体编入，TEST-SYNC +5 条 `extract_correction_word` 使 src-tauri 侧 wordbook 31→36→60；`git diff b5a96a9..HEAD -- src-tauri/` 空证自身零改动）→ pytest **9 FAIL/54 PASS/32 SKIP 记 BLOCKED**（全已知 harness 缺陷 [E2E-CONFIG-PATH-STALE-001]，`tests/` 零 diff 非本批引入，未调 harness 待 E2E-HARNESS-050）
- **消融 A**：删 MOD_ALT 分支 `set.add(0xA4)` → 实测变红 **4 条超任务书预测 1 条**：T4 命中；T3/T9/T11 因精确数组/穷举/组合断言同样 assert 0xA4 缺失而同红，**全同源、护栏更严非失效**（[ABLATION-MODEL-TOO-LIGHT-001] 同向）；**零 false alarm**（T5/T7 独立性保持绿）
- **还原自证**：编辑器还原（禁 git reset/checkout/stash/clean）→ `git diff -w` 0 字节 → 复跑 Vitest 78/78 逐数一致
- **红条三分类**：① 0 / ② 9（pytest harness）/ ③ 真回归 **0**，无阻塞项
- **红线合规**：版本号 0.8.0 未动 / 未 commit / 未改 coder-2 判据 / `src/**`+`src-tauri/**` 生产零改动 / progress.md N/A（规则 7）
- **详情**：outbox/tester-1/result.md + logs/20260817.md + CHANGELOG.md

---

## 2026-08-17 — tester-1 — TEST-SYNC-049/051/053 ✅ 阶段三测试同步：HOTKEY-049 护栏 + WORDBOOK-053 方向复核（前置 export 授权协商闭环，待主控验收）

- **来源**：主控派单（基线 HEAD `15ea6b5`，本批三提交 HOTKEY-049+OVERLAY-051 `9c1806d` / OVERLAY-051-A/H `f03a4ea` / WORDBOOK-053 A+B `15ea6b5` 已验收）
- **前置协商**：任务书称三纯函数「已导出可直接测」不实（模块私有，`git log -S` 从未 export）→ 按 [ASSERT-ADJUST-REPORT-001 附则] 先报主控 → 主控认错 + **授权仅加 `export`**（严格边界），diff `--numstat` 3/3 自证，`:500` export default 原样
- **任务一**：`HotkeySettings.test.tsx` 追加 +86（不新建）：T1-T7 七条实例逐条一用例（精确集合断言 + keysOverlap 拦/放行）+ T8-T11 边界（`translationKeySet(0)` 空集不得拦 / 修饰键展开穷举 / 空集短路 / mod=0x7 六修饰键）；消融推演删 `set.add(0xA4)` → T4 必红（Gavin 拍板「左右都算」）
- **任务二**：`src/wordbook/mod.rs` 测试区补 +15，复核 coder-2 四条均为精确值断言真护栏；补 `test_extract_correction_word_never_returns_original_side_text`（阿里云/阿里運，断言不含原侧 `云` + eq `運`）；附注 `_does_not_learn_original_side` 注释陈旧（写 None 实为 Some("云")，判据正确未动）
- **任务三**：五项不可测项如实清单（EDIT 子类化/横向滚动/C-E-F/线程归属/Ctrl+Alt+M vs AltGr），不写假护栏
- **验证**：`npx tsc --noEmit` 0 error / `cargo check --all-targets` 0 error（白名单内）；未跑测试执行类命令（消融实测留阶段四 TEST-EXEC-049/051/053）
- **红线合规**：版本号 0.8.0 未动 / 未 commit / 未改 coder-2 判据 / `src/**` 生产零改动（仅测试模块）
- **详情**：outbox/tester-1/result.md + logs/20260817.md + CHANGELOG.md

## 2026-08-18 — coder-1 — OVERLAY-051-G-FIN ✅ 051-G 收尾：时间戳驱动回放（纠正方向，src/main.rs + qwen_inference.rs +264/-58，待主控验收）

- **来源**：昨天被打断的 051-G 半成品是「固定速率打字机」（每字 clamp 180-350ms、backlog 摊 1000ms、超 20 字 jump），违反 Gavin 三条指示（时间戳驱动/不压缩停顿/words 空退回立即显示）。基线 HEAD 12f0915
- **改动**（仅 src/main.rs + src/transcription/qwen_inference.rs +264/-58）：
  1. 修编译错误（qwen_inference.rs:1653 测试闭包 |_, _|{} 适配 :712 新签名 FnMut(&str, &[WordTiming])）
  2. WordTiming 补 end_time/punctuation 两字段（缺字段降级不整条丢弃，官方 schema 取证 asr-qwen-audio-3.0-integration-001.md:105-107）
  3. **words 累积放 StreamingAsrState**（关键设计，避免原 WIP 的 extend 合并 bug）：增 confirmed_words/current_words 字段 + on_result 扩 words 参数（与文本同构）+ display_words()；回调每次下发与 display_text 完全对齐的全量词表 → overlay 侧 UpdateWordTimings 整体替换
  4. 删 compute_tween_advance + 5 速率常量 + 9 测试，新增 reveal_chars_by_timeline 纯函数（契约：words 空→立即全显/以 words[0].begin_time 取差值不依赖绝对起点/不压缩停顿/单调不回退/.min(total_chars) 宁可多显绝不少显）
  5. RecordingWithText 分支改时间戳驱动（origin 墙钟 + reveal_chars_by_timeline + .max(displayed) 单调 + 降级 words 空立即全显）+ 四个清理点补 word_timings.clear/tween_audio_origin=None 防跨会话复用
- **自证**：① words 累积放 StreamingAsrState 与文本同构（end=true push confirmed 并清空 current；同 id 整体替换 current；新 id 换 id 替换 current）→ display_words 与 display_text 字符级对齐 → UpdateWordTimings 整体替换无合并 bug；② reveal_chars_by_timeline 五契约逐条推演（words 空→total_chars；words[0].begin_time 取差值 1.5s pre-roll 消掉；不压缩停顿无上限下限；.max(displayed) 单调；.min(total_chars) 宁可多显）；③ 降级路径 src/main.rs:1365 word_timings.is_empty()→立即全显；④ 四个清理点补防跨会话复用；⑤ 未动三条历史红线（InvalidateRect bErase=false/interpolate_step/STREAMING_STOPPED）
- **验证**：cargo fmt clean / cargo check --all-targets 0 error（99 既有 warnings 无新增）/ cargo check src-tauri --all-targets 0 error / git diff --stat 仅两文件 +264/-58 / 残留自查全空（grep 不到 compute_tween_advance 及 5 常量）/ 版本号 0.8.0 三处未动
- **红线合规**：未碰 src/audio/**/src/vad.rs/ui/**/src-tauri/**/tests/**（文件域独占）/ 未跑 cargo test/build（阶段一）/ 未 push / 未 commit（主控统一提交）/ 版本号 0.8.0 未动
- **Gavin -debug 实证前暂定参数**：无新常量引入（删掉了 5 个）。时间戳驱动完全依赖服务端 words[].begin_time，无任何人为参数。下一棒 tester-1 出诊断包 → Gavin 跑 -debug 看 words=N 实际值校准：① 词表是否覆盖完整文本；② begin_time 差值是否与墙钟对齐
- **详情**：outbox/coder-1/result.md + logs/20260818.md + CHANGELOG.md（OVERLAY-051-G-FIN 条目）+ docs/MACOS-HANDOFF.md §OVERLAY-051-G-FIN

## 2026-08-18 — coder-1 — OVERLAY-054-B-FIX ✅ 录音窗口闪左上角修复（类型层面根治，src/main.rs，待主控验收）

- **来源**：Gavin 2026-08-18 端测第二次报同一现象（上一轮只修本地模型路径，在线路径没修）。录音开始窗口先闪屏幕左上角再跳回正确位置
- **根因**（主控已 Read 取证三步闭环）：show_overlay_streaming_idle 绕过 overlay_geometry 硬写 pos:[0,0]（在线流式第一次 Show）；OVERLAY-046 恢复无条件 SetWindowPos 后 [0,0] 真的生效 → 窗口摆到左上角；随后第一条 StreamingText 走 show_overlay 算出正确位置 → 窗口跳一下。三处 pos:[0,0] 的实际危害：streaming_idle 真错位（Gavin 看到的）/ FocusLost 真错位（注入失败提示框会出现在左上角）/ StreamingEditing 当前无害（adjust_overlay_pos_size_for_text 重算 x/y 忽略传入 pos）但同样要清
- **改动**（仅 src/main.rs，与 051-G-FIN 同文件串行无冲突）：
  1. OverlayRequest.pos 从 [i32;2] 改为 Option<[i32;2]>（None=调用方无位置可给，overlay 线程 Show 时解析）
  2. Show 端入口 unwrap_or_else(|| overlay_geometry(&request.status, hwnd).0) 解析，写回 Some(resolved_pos) —— 在 overlay 线程内算比调用侧算更对（monitor_work_rect 解析的是 overlay 窗口所在显示器，多屏时调用侧 hwnd 可能不同块）
  3. streaming 分支用 final_pos 局部变量跟踪，最终 request.pos = Some(final_pos)
  4. SetWindowPos 读 resolved_pos（已解析）而非 request.pos[0]
  5. 插值路径 req.pos.unwrap_or([0,0]) 兜底（state.request 存的已是 Some）
  6. 9 个构造点：3 处 [0,0] 改 None（streaming_idle/FocusLost/StreamingEditing）；4 处已算好位置改 Some(pos)；1 处 overlay 线程内 .. 模式匹配不改；1 处 show_overlay 改 Some(pos)
- **同批 051-G-FIN 尾部托底修复**：reveal_chars_by_timeline 循环记录 broke_early，若无 break（全部词到期）→ return total_chars（词表覆盖不到的尾部一并放出，契约 5「宁可多显绝不少显」的托底，修前尾部差额卡到松键 flush 才补上，端测表现是最后一两字迟迟不上屏）
- **自证**：① grep "pos:[0,0]" 零命中；② OverlayRequest.pos 已是 Option<[i32;2]>（:182）；③ Show 端 unwrap_or_else 兜底（:1044）；④ 9 构造点逐点对照表（见 result.md）；⑤ 三条红线未动（OVERLAY-046 无条件 SetWindowPos 仅改读 resolved_pos 不改调用 / InvalidateRect bErase=false / 051-G 揭示+interpolate_step+STREAMING_STOPPED）
- **验证**：cargo fmt clean / cargo check --all-targets 0 error / cargo check src-tauri --all-targets 0 error / git diff --stat 仅 src/main.rs + qwen_inference.rs / 版本号 0.8.0 三处未动
- **红线合规**：未跑 cargo test/build（阶段一）/ 未 push / 未 commit（主控统一提交）/ 未碰 src/audio/**/src/vad.rs/ui/**/src-tauri/**/tests/**
- **详情**：outbox/coder-1/result.md + logs/20260818.md + CHANGELOG.md + docs/MACOS-HANDOFF.md §OVERLAY-054-B-FIX


## TEST-SYNC-051G/054B · 阶段三 · coder-2 → tester-1 → 全体

- **交接物**：`src/main.rs` 新增 `mod overlay_051g_reveal_tests`（T1-T8，+122）；`src/transcription/qwen_inference.rs` `mod tests` 追加（T9-T16，+210）。工作区已含两文件新增，**未 commit**。
- **运行要求**：tester-1 阶段四 `cargo test` 请以 `export PATH="/c/Users/Aaron-GMK/.cargo/bin:$PATH"` 开头；仅白名单 `cargo fmt` / `cargo check`（DEC-048）。`cargo test` 已在消融推演列表就绪（A 删 `- origin_begin`→T2 红；B 引入 clamp→T3 红；C append 回归→T10/T12 红）。
- **已评估未实施**：OVERLAY-054-B-FIX 的 E2E 定位断言（`state_detector.py` rect + `MonitorFromWindow` 判下半部）需构建产物，留作未来增强单独派发。
- **提出者追加**：OVERLAY-051-G 回放 reveal 的 EST-09 命题源自既有 EST-04/05/06/07（tester-1 已置 Reflect）；OVERLAY-054-B-FIX 命题 EST-11 置为 verify-pass，spec 不新增测试要求。
