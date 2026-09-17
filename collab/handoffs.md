# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-09-17 — tester-1 — BUILD-228 v0.9.1 出包 ✅ 八项核验逐项全 PASS（首次实战 §一·五 清单）

- **产物**：`Publish/feiyin-ime.exe` **12,348,928B** sha `cd2ad3e4…` @15:43 ｜ `Publish/feiyin-ime-ui.exe` **10,046,976B** sha `33ab3c67…` @15:38（本批 **UI 重建**）｜ `Publish/crash-reporter.exe` **24,878,080B** sha `ce3fd534…` @15:42。三产物 sha **全异于上一包**（`ecfe325a…`/`6b12ddbb…`/`80adbc88…`），target↔Publish 两副本逐一相等。
- **构建**：Step1 清进程残 0 → Step2 `npm build` 677ms + Tauri `1m53s`（custom-protocol）+ cp → Step3 主程序 `2m29s` → Step4 同步 3 exe + toml 三副本。🔴 `src-tauri/Cargo.lock:933-934` `feiyin-ime-ui` **自动 0.9.0→0.9.1** ⇒ Tauri 侧确实真正重建（任务书要求确认项）。
- **八项核验（逐项 PASS，未写「N 项全 PASS」了事）**：①时间戳=本次构建 ✅ ②sha 两副本相等+异于上一包 ✅ ③**ProductVersion `0.9.1.0`/`0.9.1`/`0.9.1.0`** ✅（唯一「变红再变绿」例外）④冒烟 PID 24360 `Responding=True`、`crash.json` 不存在；`-debug` 811 行启动链路完整（starting→Online ASR→hotkey hook→tray→config watcher）、panic/ERROR **0**、清理残留 **0** ✅ ⑤`config.toml` `8b453259…` 与 `wordbook.sqlite` 前后零变化 ✅ ⑥warnings `feiyin-ime` **111**=基线 / `crash-reporter` 9 / `feiyin-ime-ui` 17 ✅ ⑦判别探针正反全过 ✅ ⑧toml 三副本 hash 全等 ✅。
- **🔴 ⑦ 项方法偏离与依据（重要）**：主控建议用前端字面量 `asr-key-input` 作探针，实测 **exe 内 0 命中**。三层证据定性：`ui/src` **5 处** → `ui/dist/assets/*.js` **1** / `*.css` **2** → exe **0** ⇒ **Tauri 资源压缩**（`[BUILD-016]` 既定结论「前端裸串 grep 0 系 Tauri 压缩，改文件名字符探针」+ 我在 §一·五 ⑦ 自设的注意条款）。故改用**文件名探针**（判别力更强）：新 `index-DmsT9phP.js`=1 / 旧 `index-SzdQI43J.js`=**0**、新 `index-y2eVZ7cR.css`=1 / 旧 `index-B0klpvKt.css`=**0** —— **同时堵死「走免重建捷径」**。后端侧正向 `[AUTOLEARN]`=1、`strip_trailing`=1，反向 `voice_strip_trailing_punct`=0。
- **⑧ 项价值实证**：`scene-rules.toml` `8ea93bb1…`×3 与 `itn-rules.toml` `311cbb96…`×3 全等，且**三副本大小完全相同**（51,011 / 37,875 B）⇒ 正是 `[TOML-ALL-NUL-001]`「坏文件与好文件大小完全相等、只比大小检不出来」的现场写照（本项必须以 hash 判）。`itn-rules.toml` hash 与历史记载一致 ⇒ 本批大改 ITN 但规则表**确实零改动**。
- **回归依据**：`TEST-EXEC-224` cargo test `1262P/0F/15I`（10 target 逐个 0F）+ `TEST-EXEC-227` vitest `100P/11S` + browser `5P`。**Vitest/E2E 本单不复跑**（出包单不改码，已有同代码状态结论）。
- **🔴 端测 4 项（交 Gavin 目视）**：a overlay 编辑态提交后剥尾标点（Win32+D2D 原生绘制，自动化零覆盖）b 真实设置页 Key 输入框斜体淡灰 + 点击即消失（Browser Mode 既有 5 条 `placeholder` 命中 0）c 繁体输出 ITN（`三點半→3:30`/`五號→5號`/`十歲→10歲`）d「API 配置（建议使用 deepseek-flash 模型…）」新文案换行/布局。
- **附加发现（已上报，未自行处置）**：① `todo.md` RELEASE-210 第 3 条「`Publish/models` 缺 performance 模型」**疑失效** —— 实测 `Publish/models/sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17/` 已存在且完整（8 文件 / 265,424,253 B，目录 mtime 09-10 21:40），请主控凭文件系统复核销项 ② `progress.md` **缺 `BUILD-209`（09-10 出包）产物表行**（我只补 `v0.9.1` 本包实测行，未代补他人条目）③ `target/release/debug.log` 为追加模式且无轮转（已 1.2MB，跨 09-08~09-17）；④ 本次 `-debug` 会话 783 条 WARN **全部**为 `[ASR-DROP] queue full`（非 ASR-DROP = 0），09-08 历史会话同现象 ⇒ 既有行为、非本批引入。
- **自查自纠两处方法论错误（如实记录）**：① 首次探针把 `[AUTOLEARN]` 交给 grep，**BRE 把方括号当字符类** ⇒ 误得 `18912`；改 `grep -F` 后 = 1 ⇒ **二进制探针必须用固定串匹配** ② 首次 ProductVersion 用 PowerShell `foreach` 内联脚本，`$n` 被双层转义吃掉 ⇒ 三行空值；改逐文件调用后正确（当场发现，未据此下结论）。
- **证据**：`collab/outbox/tester-1/build228/`（12 份：pre/post 基线快照、step 日志、八项核验原始输出、`debug.log.before/after` —— 原日志已按 `[EVIDENCE-LOG-VOLATILE-001]` 还原并经 sha256 校验一致）。
- **红线**：未 commit / 未 push / 未改任何源码 / 版本 0.9.1 系 Gavin 明确授权（VER-BUMP-217）/ 零凭证 / 临时区无残留。

## 2026-09-17 — coder-1 — VER-BUMP-217 ✅ 交付（版本 0.9.0 → 0.9.1，待验收 → 随即出包）

- **授权**：Gavin 2026-09-17「版本升级到 v0.9.1」。前置=本批全部单验收通过 + 阶段四两轮全量回归全绿。
- **改动**（只三处版本串）：`Cargo.toml:3`、`src-tauri/Cargo.toml:3`、`src-tauri/tauri.conf.json:9` 皆 `0.9.0`→`0.9.1`（改前已 grep 核对各只有一处）。`ui/package.json`（0.1.0）**未动**。
- **Cargo.lock**：根 lock 自动重写 1 行（`voice-ime` 版本），未回滚；`src-tauri/Cargo.lock` 未变（根 check 不碰它），🔴 tester-1 的 src-tauri 构建会自动更新为 0.9.1。
- **验证**：`cargo check` 0 error、warnings 111 持平；未跑 release / 未 test / 未 commit。
- **红线**：只动三个版本文件（+lock 自动行）/ 零凭证。

## 2026-09-17 — tester-1 — TEST-EXEC-227 UI 补跑收口 ✅ 两项全绿（只跑不改）

- **范围**：主控裁量**只跑前端** —— `src/` 与 `src-tauri/` 本轮零改动 ⇒ `cargo test` / E2E **SKIP**（`TEST-EXEC-224` 的 `1262P/0F/15I`（10 target 逐个 0F）仍是本批 Rust 侧有效结论）。
- **①vitest**：`7 files passed` / **100 passed + 11 skipped (111)**，EXIT=0（上轮 92P/11S/103 ⇒ **净增 8**，与新增 8 条吻合）。`Voice.test.tsx` **35P/11S/46**（上轮 27P/11S/38）；**`GUARD-225-226` 8 条全 PASS**（`UI-ASRKEY-225-001~006` = 6 + `UI-LLMHINT-226-001~002` = 2）；**既有 `PUNCT-UI-001~007` 7 条全 PASS 未被带塌**（含上轮 `toBe(2)` 那条）。
- **②browser**：**5 passed**，EXIT=0。🔴 **覆盖边界独立核查**：`grep -ci placeholder browser.log` = **0**，5 条全属 `src/test/browser/visual-style.test.tsx` 的 `UITEST-138` sidebar/nav 设计令牌 ⇒ **本轮新增 `.asr-key-input` placeholder 规则一条未碰** ⇒ ②绿只证「未带塌既有视觉令牌」，**不等于本轮 CSS 被验证**。
- **FAIL**：**0**（`grep -cE '^ × ' vitest_voice.log` = 0），无需贴输出。
- **🔴 端测项（转述任务书，我未独立验证其真实设置页成立性）**：① placeholder「斜体 + 淡灰」② `:focus::placeholder{color:transparent}` ③ 真实设置页**父级样式是否覆盖**新规则（coder-2 像素实测在**独立对照工装页**、非真实 VoicePage 上下文）④ LLMHINT-226 文案视觉布局。前两项 happy-dom **无 CSS 级联/布局引擎**原理上测不了。
- **计数纪律**：全部「N 条」由命令取数并粘贴命令。含一次如实记录：`grep -cE '> GUARD-225-226 >'` 误得 **0**、与逐条清单 **8 行**自相矛盾，改用 `^ ✓ .*GUARD-225-226` 后 = **8** 对齐（上轮 9/10 自纠规律的实际触发）。
- **构建环境预热**：测后清 `feiyin-ime / feiyin-ime-nor / voice-ime-ui / feiyin-ime-ui / crash-reporter` ⇒ **残留 0**，等出包指令可直接开构建。
- **证据**：`collab/outbox/tester-1/testexec227/`（vitest / vitest_voice / browser 三份）；224 报告归档 `result-TEST-EXEC-224.md`。
- **红线**：未改生产代码与测试 / 未出包 / 版本 0.9.0 未动 / 未 commit / 零凭证。

## 2026-09-17 — coder-1 — GUARD-225-226 ✅ 交付（UI 交叉护栏，待验收）

- **只改** `ui/src/pages/Voice.test.tsx`（+275/−2，新增 `describe('GUARD-225-226')` 8 条）；`Voice.tsx` / `styles.css` / 三份 i18n 一行未动（coder-2 生产交付）。
- **核心不变量（Gavin 一票否决）**：空值时 `value===''` + 提示文案在 **placeholder 属性** + value 不含文案 + **`updateConfig` 零调用**；输入后写回载荷只含输入值。
- **行为式反硬编码**：英文/繁中界面 placeholder/label 随 locale 变（硬编码必红）+ `Voice.tsx?raw` 源码断言（走 `t.voice_asr_online_api_key_placeholder`、无字面量）；226 三份 `llm_api_config` 均含 `deepseek-flash`，`Llm.tsx` 仍读 key、无字面量。
- **自证中发现并修**：初版把 locale 目录名当 `ui_language` ⇒ 两语言都渲染英文、护栏形同虚设；改用枚举值 + `cleanup()`。
- **覆盖缺口声明**：happy-dom 测不了 `::placeholder` 视觉；真实页父级样式覆盖待目视；落盘属父进程；`Llm.tsx 未改动`无基线（以行为等价表达）。
- **验证**：`npx vitest run src/pages/Voice.test.tsx` **35P/0F**（11 skipped）；`npm run build` PASS。🔴 未跑全量 `npm run test` / 未跑任何 cargo。
- **红线**：单测试文件 / 未 commit / 版本未动 / 零凭证。

## 2026-09-17 — tester-1 — TEST-EXEC-224 阶段四全量复跑（收口轮）✅ 全绿收口（只跑不改）

- **结论**：首轮 5 red **全部转绿**，本轮 **0 FAIL / 0 ERROR**；coder-2 报的 **12 个 `--nocapture` 数字独立复现，全部吻合**。
- **结果**：`cargo test` 全量 **1262P / 0F / 15I**（**10 个 test target 逐 target 0F**（`grep -cE "^ *Running "` = 10 = 5 unittests bin + 5 `tests/*.rs`）; feiyin-ime bin **1175P/0F/13I**，首轮 1166P/5F）／vitest **92P/11S**（7 files、EXIT 0）／test:browser **5P**（EXIT 0）／E2E 按令 SKIP／test profile warnings **102** = 基线。
- **分组**：新增 `guard_222_deadrule*` **4/4**；`itn_v091_216_*` 15/15；`itn_v091_220_*` **6/6**（首轮 4ok/2F）；`guard_214_215::*` **12/12**（首轮 11ok/1F）；`itn_hant_shadow_1to1_guard::*` **8/8**（首轮 6ok/2F）。
- **首轮 5 red 归零**：220_f 期望值（221 现状锚）／220_d 期望值（`這一萬塊` 为**正确输出**）／guard_215_g10 括块窗口（`} else {` 行 `brace_delta` 净差 0）／shadow_identity 谓词改无跳过式／豁免表 6→5 —— 均 coder-2 **测试侧**修正。
- **`FIX-222-DEADRULE` 覆盖确认**：4 条不变量护栏通过（含 `..._fish_entries_reachable` 直接钉 `itn-rules.toml:333`/`:405` 两条鱼名）。🔴 死规则**成因与修法为转述**（任务书 + coder-1 交付说明），我只独立佐证「4 条护栏全过」+ 下行数字。
- **🔴 12 个数字独立复现（`--nocapture` 原样）**：`[DEADRULE] 已编译规则表 key 总数 = 1844`；`toml 引号字面量去重 1767 条 → 归一后 1767 条；被改写 2 条；碰撞 0 条`；`itn-rules.toml 抽取汉字 1175 个（逐字判，无跳过）：ZhHans 会改写 2 个 / ZhHant 会改写 370 个 / 双向变体 0 个`；`全扫描 28096 字（逐字判、无跳过）：ZhHans 会改写 3744 个 / ZhHant 会改写 2670 个 / 双向变体 5 个（已知 5，新增 0）`。另区域自洽 `20992+6592+512=28096` ✅；`370`/`2670` 与首轮独立实测一致。
- **证据**：`collab/outbox/tester-1/testexec224/`（cargo_test 134,955 B / 2,497 行、nocapture_shadow、nocapture_deadrule、vitest、browser）；首轮报告归档 `result-TEST-EXEC-222.md`。
- **红线**：未改生产代码与测试期望值 / 未出包 / 版本 0.9.0 未动 / 未 commit / 零凭证。

## 2026-09-17 — coder-1 — FIX-222-DEADRULE ✅ 交付（规则表 key 加载期归一）

- **根因**：221 判定空间统一到影子串，但表 key 仍为 toml 原文 ⇒ `:333 三角捷鰕虎鱼`（鰕→𫚥）、`:405 三角聚鯻`（鯻→𬶟）两条保护成**不可达死规则**，`check_protection` 恒不命中 ⇒ `三角`→`3角`。221 前能命中 ⇒ 真回归。
- **修法**：新增 `normalize_rule_key` + `KeyNormalizeReport`(norm/set/finish_and_log) + `push_hierarchy_key`；`from_rules` 全部表加载期过 ZhHans 归一 + 碰撞 `log::warn`。`itn-rules.toml` **零改动**（[D]）。
- **①②③ 实测**：① 碰撞 **0**；② 条目数不变（all_units=67/idiom=47/proper=130/historical=94/collision_words=1364 等，碰撞=0 ⇒ 前==后）；③ 改写 **2 条**（正好那两条鱼名），无第 3 条。④ 可达性探针 `check_protection=Some(6)/Some(4)`（修前恒 None）。
- **验证**：`cargo check --all-targets` 0 error、warnings 111/102；`rustfmt --check src/itn.rs` exit 0；特批定向 `itn_v091_216_` 15/15、`itn_v091_220_` 6/6；临时探针已删（grep=0）。
- **边界**：只动 `src/itn.rs`，未碰 coder-2 三文件、未写断言。
- **红线**：未动 itn-rules.toml / 未 commit / 版本未动 / 零凭证。

## 2026-09-17 — tester-1 — TEST-EXEC-222 阶段四全量回归 ✅ 执行完毕（🔴 5 red 全为测试侧、生产缺陷 0，只跑不改）

- **结果**：`cargo test` **1166P / 5F / 13I**（feiyin-ime bin）+ `51P/2I/0F`（crash-reporter bin）／vitest **92P/11S**（7 files、EXIT 0）／test:browser **5P**（EXIT 0）／E2E 按令 SKIP／test profile warnings **102** = 基线。
- **5 red 分流**：🔴 **全部为测试侧，【生产缺陷 0】**（主控 2026-09-17 定案；确定性 **2/2** 复现，已停手未修）：
  1. `itn_v091_220_f_hant_221_boundary_current_state`（`itn.rs:5314`）实测 `3:30` = 221 现状锚，期望值更新归 coder-2（非回归）。
  2. `itn_v091_220_d_overreach_still_converts`（`itn.rs:5293`）**期望值写错**：实测 `這一萬塊` 才是**正确输出**（与简体基线 `216_d` 一致，正是 221 目标行为）；主控先前据本报告**读反 left/right** 派的生产修复单**已被 coder-1 日志证伪并撤销**。
  3. `guard_215_g10_main_channel_both_branches`（`punctuation/mod.rs:1045`）**括块假红**：`brace_delta` 在 `} else {` 行净差为 0 ⇒ 窗口吞掉 `else` 分支，**生产代码正确**。
  4. `shadow_identity_negative_control_traditional_diverges`（`text_normalizer.rs:1318`）+ 5. `shadow_identity_global_simplified_scan`（`:1264`）**探针前提错**（非「表版本不符」）：`zhconv build.rs:571-575` 对「多值且含自身」的 OpenCC 条目**整条丢弃**（`万→萬 万`、`麽→么 麽`）⇒ 运行时 `ZhHant(万)=万`、`ZhHans(麽)=麽`。
- **🔴 实跑推翻 coder-2 的模拟值**：`itn-rules.toml` 汉字 **1175 吻合**、简体形 **370（准数）**（coder-2 模拟 397）；全域 total **28096 吻合**、简体形 **2670（准数）**（模拟 2742）；非 identity **5**（模拟 6）；**新增 0，无第 7 个未知字**（未超范围）⇒ 复核**认 370，不认 397**。
- **分组**：`itn_v091_216_*` 15/15 全绿；`itn_v091_220_*` 4ok/2F；`guard_214_215::*` 11ok/1F；`itn_hant_shadow_1to1_guard::*` 6ok/2F；UI 新 6 条全 PASS。
- **取证方法**：PASS 用例 stdout 默认不打印 ⇒ 用 `cargo test --bin feiyin-ime <filter> -- --nocapture` 定向取「1175 / 370」原始行。
- **coder-2 索要 6 条真实值一事**：`assert_eq!` 首条 panic 即终止测试函数，`--nocapture` 无效；且 grep 确认这 6 个输入无任何其它用例覆盖、无现成 ignore 探针 ⇒ **只跑不改下无解**。已上报，主控裁 **A**（由 coder-2 在自己要改的函数内临时 `println!` 取真值），我未动手（避免在 coder-1/coder-2 在途的 `itn.rs` 上插入+删除探针）。
- **证据**：`collab/outbox/tester-1/testexec222/`（`cargo_test.log` 133,829 B / 2461 行、`nocapture_1175.log`、`vitest.log`、`vitest_voice.log`、`browser.log`）。
- **红线**：未改生产代码与测试期望值 / 未出包 / 版本 0.9.0 未动 / 未 commit / 零凭证。

## 2026-09-17 — coder-1 — V091-ITN-HANT-SYSTEMIC-221 步骤2（后处理 pass）✅ 交付（待验收）

- **改动**：`src/itn.rs` `normalize_unit_symbols` 构造 `orig`+`shadow`（逐字 ZhHans，1:1+`debug_assert`），未命中推 `orig[i]`；`try_match_arabic_symbol(shadow, orig, pos, r)`；neg-prefix 判定取 shadow、text 风格「零下」前缀取 orig；plain 判定取 shadow（输出仅 ASCII+`℃`）。`itn-rules.toml` 零改动；`normalize_unit_symbols_only` 补丁通道自动继承。
- **③ 影响**：仅含 trigger（摄氏度/摄氏/°C）时才改变输出；**21 条断言均不含这些词 ⇒ 零影响**（216 a~o + 220 a~e 绿，仅 220_f 预期红）。
- **收益**：繁中 `40攝氏度` → `40℃`。
- **验证**：`cargo check --all-targets` 0 error、warnings 111/102 持平；`rustfmt --check src/itn.rs` exit 0；未跑 cargo test。
- **边界**：只动 `src/itn.rs`，未碰 coder-2 的 `src/text_normalizer.rs` / 220 断言。
- **红线**：未 commit / 版本未动 / 零凭证。

## 2026-09-17 — coder-1 — V091-ITN-HANT-SYSTEMIC-221 步骤1（主 pass）✅ 交付（待验收 → 步骤2）

- **设计**（主控裁定「按 pass 拆」）：`orig` + `shadow`（逐字 `Variant::ZhHans`，1:1 + `debug_assert`）；**判定全查 shadow、输出全取 orig**；结构体双词化（[B] 铁律）。
- **改动**：`src/itn.rs` 新增 `hant_to_hans_char`；`normalize_with_rules` 主循环判定/输出分离；`UnitChain` 增 `out_units`；`ImplicitDecimal` 增 `out_unit`；`RemainderSuffix` 增 `out_unit`+`out_real_unit`；`try_parse_*` / `capture_price_per_unit` 线程化 `shadow`+`orig`；`format_*` 上屏取原串字段。`itn-rules.toml` 零改动。
- **③ 静态 trace**：216 a~o（简体）+ 220 a~e（繁中/混排）**预判全绿**；220_f **预期变红**（`三點半→3:30` 等 5 条变化 + `這一點/這一號` 同值），属 221 兑现主用例的预期变化。
- **🔴 风险 R1**：`shadow==orig`（简体）依赖 ZhHans 对已简体字**恒等**，221A 只证长度 1:1、未证 identity → 建议 coder-2 在 `itn_hant_shadow_1to1_guard` 补 identity 断言。
- **验证**：`cargo check --all-targets` 0 error、warnings 111/102 持平；`rustfmt --check src/itn.rs` exit 0；🔴 未跑 cargo test（阶段一）。
- **未做**：步骤2 `normalize_unit_symbols`（禁两步一起交）。
- **红线**：单文件 / 未动 coder-2 断言 / 未 commit / 版本未动 / 零凭证。

## 2026-09-17 — coder-1 — ITN-HANT-POC-221A ✅ 交付（只读取证，生产零改动）

- **结论**：路线②′「影子串」前提**成立**，但方案须修正为 `Variant::ZhHans` + 建议逐字构造影子串。
- **Q1**：`Variant::ZhHans` = 只字形不地区词（源码 `zhconv-0.4.1/src/variant.rs:36-66`、`converters.rs:28`、`variant.rs:130`）。⚠️ 非纯逐字，含 OpenCC 词组规则；产品现状用 `ZhCN/ZhTW`（地区变体）**必须替换**。
- **Q2**：`cargo test --bin feiyin-ime poc_221a -- --nocapture` **5 PASS / 0 FAIL** —— 指定 20 样本 20/20 等长；逐字 1:1 PASS；**全局 28,096 字非 1:1 的 0 个**；词组 60 条 0 不等长。反面对照 `zhconv("後面",ZhTW)=="後麵"`。
- **Q3**：长度守卫够用但不最优（不保对齐 + 静默退化）；**建议逐字构造影子串**（`chars().map(|c| zhconv(c,ZhHans))`）—— 1:1 由构造保证。
- **Q4**：改造面 **约 33 组函数 / 40+ 表查找点** + 全部 `consumed`/索引点（甲/乙/丙型、链扫描、主循环）+ `format_*` 输出须取原串字符。
- **产物**：`src/text_normalizer.rs` 末尾 `#[cfg(test)] mod poc_221a_hant_shadow`（5 用例，一次性，待主控指示清理）。
- **红线**：只读取证 / 生产零改动 / 未 commit / 版本未动 / 零凭证。

## 2026-09-17 — coder-1 — V091-ITN-YIKE-HANT-220 ✅ 交付（生产改动，待主控验收 → 繁体护栏由 coder-2 交叉另派）

- **改动**：`src/itn.rs` `is_demonstrative_yi` 条件② 两个集合补繁中异形字 —— `prev_is_demonstrative` 加 `這`；`prev_is_me_form` 前字扩为 `么`|`麼`、其前两字加 `這`（`:2437-2443`）。条件①/③、主循环、其它函数**零触碰**，不扩语义范围。
- **由来**：TEST-SYNC-216 我的独立发现②（集合仅简体 → 繁中「這一年」不命中）；主控核可达性后判真缺口。根因形状=那/每/哪/某 繁简同形 ⇒ 繁中「一半修好一半没修」。
- **刻族注释补记**：按主控裁定判为已知边界不修，在 doc「已知边界」加小节，写成 **刻意接受 + 三条理由**（明确否掉「暂不支持/待优化」措辞），并指回现状断言 `itn_v091_216_l_...`。
- **交叉规则**：本单改生产代码 → 我**不写断言**，繁体护栏由 coder-2 另派。
- **边界外风险上报**：后缀表「號」仍为简体（`那一號` 不受保护），属条件③表内容问题，本单红线不碰故未动。
- **验证**：`rustfmt --edition 2021 --check src/itn.rs` exit 0（🔴 未跑全量 fmt）/ `cargo check --all-targets` 0 error、warnings 111/102 持平；🔴 未跑 `cargo test`。
- **红线**：单文件 / 未 commit / 版本未动 / 未出包 / 零凭证。

## 2026-09-17 — tester-1 — BUILD-VERIFY-219 ✅ 出包核验清单成文（七项取证还原 + 新增第八项，纯文档单，零构建零测试）

- **落点**：`collab/build-test-guide.md` 新增 `一·五、出包核验清单【八项，出包任务必须逐项报 PASS/FAIL】`（+113 行），并在 §一 Step 4 末尾加一行交叉引用（**动作 ≠ 核验**）。**未动任何其它文件的生产内容**。
- **取证方法**：全量扫 `handoffs-archive.md`，按条目抽取 50 条含核验描述的 BUILD 记录，机器分组比对条目集合 —— **未采信主控给的 BUILD-186 记忆**（主控明示以 archive 为准）。
- **还原结论**：清单非固定七项，而是从「四项（BUILD-154/156）→五项（159）→六项（165）→七项（168/171/173/177/186）」演进；v0.8.x 时代（016/017/019/020/022）七项内容与现行**不同**（含 toml 三副本、大小对照、UI 前端嵌入探针）。
- **如实记录的三个偏差**：① 「七项」名号与实际条数长期不符（BUILD-145/151 报「七项」实际列 **8 条**；BUILD-098 报「七项」正文只列 4 类）② 🔴 **toml 三副本 hash 在 v0.8.x 是第③项，BUILD-159 起被丢进「跳过」清单，165/173/177/186 只 cp 不核** —— 第八项正是补回这个洞 ③ 大小对照已移出清单（BUILD-020 证「大小非唯一判据」）。另记 BUILD-193 为**唯一一次核验项不完整的出包**（tester-1 模型故障、主控代做、未逐项枚举）。
- **第八项全文**：判据=`scene-rules.toml` / `itn-rules.toml` 各自三副本（仓库根 / `Publish/` / `target/release/`）sha256 两两全等；命令 `sha256sum <三路径>`；🔴 必须 hash 非大小 —— `[TOML-ALL-NUL-001]` 坏文件与好文件大小完全相等；失败=用户跑的不是代码里的规则（`[TOML-STALE-001]`）或文件已损坏。
- **自查**：`git diff --numstat -- collab/build-test-guide.md` = **`+113 / -0`**（纯新增，删除列 0）；全文 UTF-8 无 BOM，`grep -c 核验` 从 0 → 有命中（原文档零成文判据，主控判断属实）。
- **红线**：未跑 `cargo build`/`cargo test`/`cargo clean`/未出包；未碰 `src/`、`ui/`、`src-tauri/`；未 commit；版本 0.9.0 未动；零凭证；无临时文件。

## 2026-09-17 — coder-1 — TEST-SYNC-216 阶段三交叉护栏 ✅ 交付（待主控验收 → 阶段四实跑）

- **改动**：`src/itn.rs` `mod tests` 末尾追加 **9 fn / 22 assert**（`itn_v091_216_{g,h,i,j,k,l,m,n,o}_*`）；**生产区 0 字节**，coder-2 既有 a~f 组一行未动。
- **流程合规**：先只读生产代码独立推边界并写入 result.md ①，之后才读 coder-2 的 41 条做差集（差集表见 result.md ②）。
- **补的缺口**：月后缀+指示代词 / 这么那么+年 / 裸么族（要么·什么）/ `么一年` i==1 / i==0 `一年·一年半` / 句尾 `那一·某一` / 非一字数字 `那十年·那三年·那三个` / 甲型半模式非日期单位 `这一吨半·这一块半` / 🔴 源码顺序机器闸门 / 保护词优先级 `十一·十一月·十一块` / 组合句+幂等。
- **🔴 两条独立发现**：① `N点M刻` 刻族未声明边界（现状 `这一点1刻/3刻`），已断言钉现状，请主控裁定「并入已知边界」还是「缺口」；② 繁体「這一年」不命中（集合仅简体），旧 bug 繁中或未修 —— 只上报不写断言。
- **验证**：`rustfmt --edition 2021 --check src/itn.rs` exit 0（🔴 未跑全量 fmt）/ `cargo check --all-targets` 0 error、warnings 111/102 持平；🔴 未跑 `cargo test`（阶段三禁，归阶段四）。
- **红线**：只动 mod tests / 未 commit / 版本未动 / 零凭证。

## 2026-09-17 — coder-1 — V091-PUNCT-TAIL-214 + V091-ITN-SKIP-ONLINE-215 ✅ 交付（待主控验收 → 阶段三 TEST-SYNC → 阶段四实测）

- **214 交付**：`apply_l2_postprocess` 加第三参 `strip_trailing_always`（`punctuation/mod.rs:171`，分支全剥优先），`punctuation/mod.rs` 8 处既有调用补 `false`；`src/config/mod.rs:360` + **Tauri 镜像 `src-tauri/src/config.rs:253`** 各增 `strip_trailing`（serde default false）；`main.rs:9008` 传参、`main.rs:6870` 给 #4/#5 单独剥尾；`Voice.tsx` 加开关 + i18n 三份；`Voice.test.tsx` PUNCT-UI-002 计数 1→2。
- **215 交付**：`main.rs:8619` 捕获 `from_online_streaming = initial_text.is_some()`，`:8720` 主通道 ITN 据此跳过；补丁通道保留。
- **🔴 任务书两处修正（均经主控批准）**：① Tauri 镜像结构漏项（不同步会静默丢配置，TECH-DEBT-001 同族）；② 215 门控由 `is_online_streaming()` 改 `initial_text.is_some()`（前者在 config/引擎热重载竞态缝里误跳本地输出 ITN）。验收判据 c 同步改写。
- **验证**：`cargo fmt` / `cargo check --all-targets` **0 error**、warnings **111/102** 持平；`cargo check --manifest-path src-tauri/Cargo.toml` 0 error；`ui && npm run build` 通过。🔴 未跑 `cargo test` / `cargo build --release`（阶段一白名单）。
- **🔴 FMT-COLLATERAL-001 复发**：对 src-tauri 跑 `cargo fmt` 连带重排 4 个零改动文件，已逐文件 `git show HEAD:<path>` 恢复（src-tauri 侧 diff 只剩 config.rs 9+/1-），coder-2 在途文件核过未被波及。主控认可处置、纠正流程（应先报后备）。🔴 后续不再对 src-tauri 跑全量 fmt。
- **红线**：未 commit / 版本未动 / 未出包 / 零凭证。

## 2026-09-17 — coder-2 — UI-ASRKEY-225 + UI-LLMHINT-226 ✅ 交付（纯前端，v0.9.1 出包前最后一单）

- **交付**：三份 locale（`voice_asr_online_api_key` 改值 + **新增** `voice_asr_online_api_key_placeholder` + `llm_api_config` 改值）+ `Voice.tsx`（`placeholder` 由硬编码 `sk-...` 改引用 i18n key、加类名 `asr-key-input`）+ `styles.css`（两条**类名限定**规则：斜体淡灰 + 聚焦 `color:transparent` 隐藏）；`Llm.tsx` 零改动。🔴 **未用 focus/blur 事件改 value**。
- **Gavin 一票否决判据三条实证**（工装渲染真实 `VoicePage` → `evidence.json` → 工装已删）：① 提示显示时 `input.value` 空 / state 字段 `""` / **全部** updateConfig 序列不含提示文案；② 空 Key 提示已渲染 **且** 测试连接按钮 `disabled`（无法绕过；后端 `transcription/mod.rs:154` 另有 `bail!`）；③ 保存 payload 该字段 `""`(string) 不含提示文案 + 全仓 grep 该文案 **0 命中**。
- **焦点行为 + 「占位 vs 不可见」实测**（Chrome 152 + CDP + **真实构建 CSS**）：文字区占位灰像素 未聚焦 **65 → 聚焦 0（完全不可见）→ 失焦 65（与未聚焦逐像素零差异、md5 相同 ⇒ 稳定复现）**；DOM rect 三态恒 `320×38`、全视口差异仅限输入框自身 ⇒ **无布局位移/不占位**；对照 `.input`/`.llm-input` 24/24、51/51 **未被污染**；F 态 732px 残留全在边框/焦点环（`.1s` 过渡中间帧）。证据产物：`collab/outbox/coder-2/asrkey225-evidence/`（5 PNG + evidence.json）。
- **评估（任务书要求）**：存量 `qwen_audio_online` **共用同一文案** —— 同一 `asr_online_api_key` 字段 + 同一 `handleTestQwen3Connection` + `ASR-056` 明写两者同走 `is_online_streaming` + 同 dashscope/百炼体系，且该模型已从下拉不可达（`ASR-UI-208`）⇒ 不设 per-model 分支。
- **i18n 通则（Gavin 新立）**：本次改动只改三份 locale + 组件引用 key；移除原硬编码 placeholder；改动后 `ui/src/pages/*.tsx` 未新增任何中英文字面量。
- **验证**：`npm run build`（`tsc && vite build`）通过（48 modules / js 206.57 kB / css 25.19 kB）；`ui/dist` 未入 git；后端 `src/`、`src-tauri/`、`itn-rules.toml` **零改动**。
- **清理**：临时 vitest 工装已删、Chrome 专用实例 + `chrome225` profile 已删、临时目录内容已清空（files=0）；⚠️ 残留一个**空目录** `/c/msys64/tmp/opencode/asrkey225`（`Device or resource busy`，无内容），如实上报。
- **红线**：未跑 `cargo test` / 未 `cargo build` / 未 commit / 版本未动 / 零真实凭证（取证用 `sk-abc` 仅存在于已删工装与截图）。

## 2026-09-17 — coder-2 — GUARD-222-DEADRULE ✅ 交付（加载期归一不变量护栏，交叉）

- **角色**：非作者交叉（FIX-222-DEADRULE 作者 = coder-1）。
- **定位**：防**影子串契约单边落地**（归一了输入、没归一规则表）。功能用例抓不到（表里 99.8% 条目本就是简体），**价值全在不变量**。
- **交付**：只改 `src/itn.rs` `mod tests`，末尾新增嵌套模块 **`guard_222_deadrule`**（4 个 `#[test]`），生产零触碰；用 `compile_rules_from_content(BUILTIN_RULES)`（避开 exe 同级残留 toml = `[TOML-STALE-001]` 的面）。
- **①🔴核心不变量**：已编译规则表**全部 key** 满足 `normalize_rule_key(k)==k`（覆盖 12 个 HashSet + `unit_collision_map` 全词 + `unit_symbol_rules` trigger + `unit_hierarchy` key + 3 前缀/模式；反空下限 1000，**实测 key 总数 1844**）。判别力=未来任何人往 toml 写非简体字 → 立刻红在**加载期契约**层，不依赖其恰好写了会被用例覆盖的规则。
- **②碰撞 + 计数**：碰撞必须 0（=两条规则被静默合并=无声规则丢失，是表内容的函数，将来可能变 1 必须红）；归一前后计数须相等（交叉验证）。**实测 toml 引号字面量 1767→1767、被改写 2、碰撞 0**。实现说明：`KeyNormalizeReport` 只在加载期打日志、未留在产物里，故从内置 toml 全部引号字面量（key 超集、逐行配对）复刻 `set(:281-300)` 语义自检；超集安全论证已写入注释。
- **③可达性锚**：`check_protection(shadow(「三角捷鰕虎鱼」))==Some(6)`、`「三角聚鯻」==Some(4)`；注释写明**它只是佐证**（删它不变量仍成立，删不变量它形同虚设）。
- **④🔴反向可证伪自证**：`normalize_rule_key("鰕")!="鰕"` + 简体对照恒等 + 归一 1:1 长度 ⇒ 判据确实会为假 ⇒ ①②③ 不是恒真式。变红条件 7 条已写进代码注释。
- **验证（实跑）**：`guard_222_deadrule` **4P/0F** / `itn_v091_216_` **15P/0F** / `itn_v091_220_` **6P/0F**（既有 21 条不塌）/ `rustfmt --check src/itn.rs` 0 diff（未跑全量 `cargo fmt`）/ `cargo check --all-targets` **0 error、warnings 111/102 持平**。
- 🔴 **删除列自查**：`numstat src/itn.rs` = `1152/243`；243 条删除行逐条审计：含我的模块名/用例名 **0**、形如测试代码 **0**（样例全为 coder-1 的 `from_rules` 生产区改写）⇒ **纯追加**。
- **边界（不冒充）**：本组判**内置表**的加载期契约；「外置旧 toml 覆盖内置新默认」属 `[TOML-STALE-001]`，归构建/出包核验。
- **红线**：生产零改动 / 只动 mod tests / 未跑全量 `cargo test`、未 `cargo build` / 未 commit / 版本未动 / 零凭证。

## 2026-09-17 — coder-2 — FIX-222-TESTS ✅ 交付（阶段四首跑测试侧三条 + 两条期望值更新）

- **A `guard_215_g10` 假阳性**：真因 **`} else {` 行 `brace_delta` = +1−1 = 0 ⇒ 深度不归零**（不是「取到闭合花括号」），窗口延伸到整个 `if/else` 的 `};` 把 else 分支算进真分支。修=新增 `if_true_body`（首个以 `}` 开头的行前截断）、删 `block_contains`。**非恒真自证**：把 `normalize_numbers` 挪进 online 分支 → 窗口必含它 → 断言立刻红。
- **B identity 护栏**：根因 **`zhconv-0.4.1/build.rs:571-575`**（多值条目含自身则整条丢弃，*be conservative*）：`万 ↦ 萬 万` 被丢 ⇒ `ZhHant(万)==万`（旧闸门**跳过万**，`0` 曾建立在「跳过=安全」的未验证假设上）；`麽 ↦ 么 麽` 被丢 ⇒ `ZhHans(麽)==麽`（麽 非双向变体）。补规则后与 tester 实测逐位吻合（2670/5）。改**谓词无关式**（`ZhHans(c)≠c` 必须 `ZhHant(c)==c`；两者都变=双向变体须与豁免表精确相等；**无跳过**），豁免表 6→5。**实测准数**：toml 抽取 **1175** 字 → ZhHans 改写 2 / ZhHant 改写 370 / **双向变体 0**；全域 **28096** → 3744 / 2670 / **5**（緼·苧·藴·輼·醖）。397/2742/6 模拟值已清除。
- **C `itn_v091_220_f`**：主控特批定向取证（`--nocapture` 先打印再写回，**零推算**）：三點半→`3:30`（221 兑现主用例）/ 五號→`5號` / 十歲→`10歲` / 三塊錢→`3塊錢` / 五萬塊→`五萬塊`（万后跟单位保守不转，与简体基线一致）/ 這一點·這一號 保汉字。注释写明**「221 预期变化、不是回归、勿回滚 221」**。
- **D `itn_v091_220_d`**：`這一萬塊`（主控撤销 FIX-222-R1；`這一萬` 仍 `這1万` 未改）。
- **🔴 追加报告（只报不改）**：toml 中 ZhHans 会改写的 2 字 —— `鰕`(U+9C15→𫚥) `itn-rules.toml:333`、`鯻`(U+9BFB→𬶟) `:405`，均在 `protect.unit_collisions`（条目「三角捷鰕虎鱼」/「三角聚鯻」）。221 影子匹配下二者**永远匹配不上（潜在死规则）**，待 coder-1/主控判定。
- **验证（实跑）**：`guard_214_215` **12P/0F** ／ `itn_hant_shadow_1to1_guard` **8P/0F** ／ `itn_v091_220_` **6P/0F**；三文件 `rustfmt --check` 全 0 diff（未跑全量 `cargo fmt`，未对 src-tauri 跑）；`cargo check --all-targets` **0 error、warnings 111/102 持平**。
- **红线**：A/B 只动测试区，C/D 只动自己那两个函数体；coder-1 的 itn.rs 生产区未碰；未跑全量 `cargo test`、未 `cargo build`、未 commit、版本未动、零凭证。

## 2026-09-17 — coder-2 — GUARD-221-IDENTITY ✅ 交付（影子串 identity 护栏，待主控验收 → 阶段四实跑）

- **为什么**：221 的「简体路径逐字零回归」全建立在 `shadow == orig`，221A 只证了长度 1:1、**未证 identity**。
- **交付**：只改 `src/text_normalizer.rs`（`itn.rs` 零触碰 —— coder-1 在其中做 221 步骤2，其在途 diff `+699/−162`）；在 `mod itn_hant_shadow_1to1_guard` 新增 3 条 `#[test]` + 2 helper + 1 豁免表（**+529/−0** 纯新增）：① ITN 规则表用字（48 字最小集无条件 + `include_str!` 抽取 itn-rules.toml 全表汉字）② 全域 28,096 字扫描（🔴 先判「是简体形」再要求 identity）③ 🔴 反面对照（`這/麼/點/號/萬` 经 ZhHans 必须**不** identity + 繁体不得进过滤器 + 简体对照五字）。
- **🔴 实测推翻原判据（主控已确认以实测为准）**：按 zhconv-0.4.1 的 OpenCC `{ST,TS}Characters.txt` 同谓词模拟 → 全域简体形 2,742 个中**非 identity 恰 6 个**：緼→縕/缊、苧→薴/苎、藴→蘊/蕴、輼→轀/辒、醖→醞/酝、麽→麼/么（全是 ST/TS 双表同字不同目标的**双向变体字**）。
- **🔴 决定性依据**：`itn-rules.toml` 抽取 1,175 汉字中简体形 397 个、**非 identity = 0** ⇒ 规则表 key 零受影响；ITN 输出恒取原串 ⇒ 这 6 字即使出现在口述里也只影响一次不命中任何规则的匹配判定，**用户可见文字分毫不变** ⇒ R1 实质解除。附带：`麽` 的影子 `么` 是 220 条件② 合法成员（影子路径下 麽/麼/么 归一，行为正确），反证 220 的 `這/麼` 显式分支在影子串下冗余 —— 按主控指示**保留作双保险不删**。
- **处置（主控拍板 Design B）**：不把「0」写死成「6」，改**豁免表 `R1_AMBIGUOUS_VARIANTS` + 漂移检测**（实测集合与豁免表**精确相等**：第 7 个出现→红；6 个任一不再违反→红）。
- **🔴 变红条件 a~e（已写入代码注释）**：a) 第 7 个违反字 → ② novel 非空；b) 豁免集合不再精确相等 → 集合比对；c) identity 判据被改恒真（`!=`→`==` / 换变体）→ ③ `assert_ne!` 首条；d) 最小集任一 ITN 用字非 identity 或计数 <40；e) `total<25_000` 或 `required<300`。另 3 处反空断言硬下限（48 / 1,175+397 / 28,096+2,742）。
- **验证**：`rustfmt --check src/text_normalizer.rs` **0 diff**（只 check，未跑 `cargo fmt`；未对 src-tauri 跑）/ `cargo check --all-targets` **0 error、warnings 111/102 持平** / `numstat` **+529/−0** / 足迹仅 1 文件。
- 🔴 3 条新用例**未实跑**（阶段三禁 `cargo test`），**阶段四必须实跑**（重点：② 是否只报那 6 个已知、①-b 的 1,175/397 计数）。
- **事故**：上报消息一处反引号包裹的 `== 0` 被 bash 当命令替换吃掉（`HEREDOC-EXPANSION-001` 同类）→ 已补发说明；后续 tmux 消息不用反引号。
- **红线**：生产代码未动 / 未碰 `itn.rs` / 未 commit / 版本未动 / 未出包 / 零凭证。

## 2026-09-17 — coder-2 — TEST-SYNC-220 ✅ 交付（繁中异形字交叉护栏，待主控验收 → 阶段四实跑）

- **角色**：非作者交叉（220 作者 = coder-1）。先只读生产代码独立推 10 项边界（result.md ①）→ 之后才读他用例做差集。
- **🔴 差集核心**：测试区 15 条 `itn_v091_216_*`（我 a~f 6 条 + coder-1 g~o 9 条）**全为简体输入**，`這`/`麼` 零出现 ⇒ 220 改动面交付时 **0 覆盖**。
- **交付**：`src/itn.rs` 文件末尾追加 6 条 `itn_v091_220_*`（共 +105 行，纯新增）：a=🔴`這`+繁简同形后缀（這一年/月/日/分/秒/刻、這一分鐘、這一刻鐘 + 简体对照 这一年）／b=🔴`這麼·那麼·這么`（(么,麼)×(这,這,那) 四组合）／c=反向红线裸 `麼` 不算（什麼一年/要麼一年/怎麼一年）／d=越界红线（那100年/這1000米/這10年/這1万/這1萬塊）／e=繁简混排句子逐字不变／f=🔴**221 现状锚**（三點半/五號/十歲/三塊錢 保汉字、五萬塊=`5万塊`）+ `點/號` 双向安全不变量。
- **🔴 顺带发现两条（已上报，均非我改动）**：① coder-1 `itn.rs:5096` `assert_eq!(money, "1.8元")` 期望值疑错 → 正确 `这1.8元`（`这` 走普通字符分支先 push），**阶段四会 FAIL 且非生产回归**，我未改其行；② 纠正任务书口述「五萬塊 全都不转」不成立（`parse_cn_number:737` 有 `'萬'` 分支 ⇒ `五萬`→`5万`，只有 `塊` 保留 ⇒ `5万塊`）。
- **🔴 同文件编辑事故（自披露 + 已修复）**：首次 append 误改 coder-1 `:5096` 一行 → 立即还原，三重佐证逐字节一致（原 oldString 精确匹配成功 / numstat 415-0 零删除 / 源串存在性检查 true）；未重排、未格式化其区段。
- **验证**：`rustfmt --check src/itn.rs` exit 0 零 diff（🔴 只 check，未跑 `cargo fmt`，避免重排 coder-1 区段；未对 src-tauri 跑）/ `cargo check --all-targets` **0 error、warnings 111/102 持平** / `numstat` **+415/−0** 纯新增。
- 🔴 **6 条新断言未实跑**（阶段三禁 `cargo test`），**阶段四必须跑**；另 o 组那条疑错也需在阶段四一并确认。
- **未覆盖缺口**：繁体 `號/點/萬/歲/錢/塊` 转换能力 = ITN-HANT-SYSTEMIC-221（本单只做现状锚）；`chinese_script=Traditional` 端到端需实机端测。
- **红线**：生产代码逐字节未动 / 未重排他区段 / 未 commit / 版本未动 / 未出包 / 零凭证。

## 2026-09-17 — coder-2 — TEST-SYNC-214/215 ✅ 交付（阶段三交叉护栏，待主控验收 → 阶段四实跑）

- **角色**：非作者交叉（214/215 作者是 coder-1，按规矩阶段三不能派给他）。
- **流程**：先只读生产代码独立推 21 项边界清单（result.md ①），**之后**才读 coder-1 用例做差集 —— 顺序纪律已守。
- **差集**：既有面覆盖 `count_units` 口径 / 尾标点字符集 / `apply_l2_postprocess` 旧行为（第三参恒 false）；**0 覆盖** = ① 四格真值表**第 2 格「关闭+开尾 → 全剥优先」**（8 个既有调用点第三参全 false → 该格从未被执行）② 恒剥尾长文本 ③ 英文/翻译路径 ④ 配置 serde 缺字段回落 ⑤ #4/#5 刻意不覆盖 ⑥ #1 传参 ⑦ src-tauri 镜像字段 ⑧ 215 全部判据面。
- **交付**：12 个 Rust 用例（`src/punctuation/mod.rs::tests::guard_214_215`，我新增 398 行全在测试区；文件现态 `+423/-17`，17 行删除全属 coder-1 迁移）+ 6 个 UI 用例（`ui/src/pages/Voice.test.tsx`，`+98/-1`）。其中 **4 条钉「刻意选择」**：G6（#4/#5 不覆盖 `enabled` 全剥，反向断言）、G9（215 判据用 `initial_text` 数据路径而非 config/asr_model，非注释出现恰 2 次）、G11（补丁通道必须在门控块之外）、G8（src-tauri 字段必须镜像 + `serde(default)`）。
- **结构护栏 idiom**：`include_str!` + 截断首个 `#[cfg(test)]` 只扫生产区 + 行首 `startswith` + 花括号定界；新增 `stmt_window` 处理 `let x = if <多行条件> { … }`（条件在 `{` 之前，花括号定界取不到）。
- **验证**：`rustfmt --check src/punctuation/mod.rs` 零 diff（只单文件，未跑全量 `cargo fmt`=避 `[FMT-COLLATERAL-001]`，未对 src-tauri 跑）/ `cargo check --all-targets` **0 error、warnings 111/102 持平** / 删除行审计：全部删除行均属 coder-1 迁移，我生产零改动 / 文件足迹仅 2 个。
- 🔴 **12+6 条用例尚未实跑**（阶段三禁 `cargo test`/`npm run test`，仅过了编译与类型）→ **阶段四必须跑**，UI 尤其需 vitest 兜底。
- **未覆盖缺口如实上报**：`initial_text=Some("")` → `NoSpeech` 早退无单测面（需 `run_pipeline_core` 级 harness）；215 真实数据路径需在线模型端测；UI 渲染细节属 Browser Mode 面。
- **红线**：未 commit / 版本未动 / 未出包 / 未碰生产代码 / 未碰 coder-1 文件（`main.rs`·`itn.rs`·`wordbook`·`src-tauri`）/ 零凭证。

## 2026-09-17 — coder-2 — V091-ITN-FIX-YIKE-216 + WORDBOOK-AUTOLEARN-OBS-218 ✅ 交付（待主控验收 → 阶段四实测复核）

- **216 交付**：`src/itn.rs` 新增 `is_demonstrative_yi`（三判据：当前字「一」／前字 ∈ {这,那,每,哪,某} 或「那么·这么」形式（裸「么」不算）／**后字必须命中 `date_time.triggers.suffix`**）+ 主循环闸门插在 `check_protection`（`:1870`）之后、**甲型（`:1880`）之前**。生产 +37 行 / 测试 +132 行（6 组：a 组 5 例、b 组 16 例、c 组 6 例、越界红线 5 例、洞证据 5 例、刻意边界 4 例）；`itn-rules.toml` **零改动**（红线 DEC-038 零词表打补丁）。
- **216 取证（静态逐行 trace，阶段一禁 cargo test）**：根因 = `decide_conversion:2275` `is_date_suffix(after_str)` 不分「数量词一 / 指示代词构词成分一」；同族 = 年/月/日/号/点/分/秒全中（那一年/每一年/这一点/这一秒/那一分/那一号），`这一点我同意`→`这1点我同意` 是高频实害；`那一天`/`那两天` 本就安全（天不在三表）；`那一年半` 走**甲型抢跑**（这是闸门必须放在甲型之前的原因）。
- **🔴 本单两次纠错（均由主控拦下，均已按判据钉进测试）**：① 闸门位置必须早于甲型；② 判据不能用「后字非数字」排除法 —— 会让 `这一块八`（真数量，现状 1.8元 / 1.2元 / 1.5元）的「一」被拦成孤立字 → 乙型隐式小数与丙型货币链整条跳过 → 退化成 `这一块8`（`[ITN-LOCAL-RULE-OVERREACH-001]` 同形）。改白名单后天然蕴含「一后不接数字/进位字」：`那一百年`→`那100年`、`这一十年`→`这10年`、`那一千米`→`那1000米`、`这一万`→`这1万` 照常转。
- **218 交付**：`src/wordbook/mod.rs:161/170` 两条 `log::info!` → `log::warn!` + `[AUTOLEARN]` 前缀（+11/-4，逻辑零改动）。不改全局日志级别（Gavin 压日志 IO）；否决独立 `target` 方案（filter 配置在 main.rs + release 默认态零收益）。
- **刻意边界（写进注释，非「暂不支持」）**：`这一点五`/`这一点半` 保持汉字；理由三条（`这一点` 压倒性是「这个观点」，为罕见写法放行会打坏高频正确用法／真要讲 1:30 通常不带「这」／`itn.rs:2977-3013` 历史注释证明该带是雷区，加分支回归风险大于收益）。
- **验证**：`rustfmt --check src/itn.rs` 零 diff（🔴 **未跑全量 `cargo fmt`**，避 `[FMT-COLLATERAL-001]` 连带格式化 coder-1 在途文件）/ `cargo check` 与 `cargo check --all-targets` 均 **0 error，warnings 111/102 与基线持平** / `git status` 确认我只动 2 文件（+169 / 11 行）。🔴 测试断言为静态 trace 推得，**待阶段四 tester-1 实跑复核**。
- **红线**：未 commit / 版本号未动（0.9.1 待回归后）/ 未出包 / 未碰 coder-1 文件（main.rs·config/mod.rs·punctuation/mod.rs·ui/）/ 零凭证。



## 2026-09-08 — coder-2 — EDITICON-190 ✅ A2 几何迁入生产（Gavin 定裁「图标选a2」，待主控验收 → 与 188/189 一批 commit）

- **改动**：`menu_icons.rs` 只换常量——`PENCIL_TIP→(-0.24,0.26)`、`PENCIL_END→(0.26,-0.24)`、`UNDERLINE→(0.361,-0.42,0.40)`（HALF_W/S_TIP/S_GAP_END 不变）；三个 covered 函数体逐字节未动；三 pub 签名不变 → main.rs D2D/GDI 自动吃新像素，免集成单。
- **🔴 参数留底**：A1 全套（`A1_TIP/A1_END/A1_LINE` + `a1_pencil_covered/a1_covered` + `dump_edit_icon_a1_preview`）留 cfg(test) 对照组，注释标明「勿删」；完整常量表见 outbox/coder-2/result.md。
- **逐字节自证**：生产重 dump 的 `edit-A2-18.png` 与 Gavin 定裁预览 **md5 一致 `187ab98e3311d791ee94acab04f49cd5`**。
- **验证**：fmt 幂等 / check 零 error、warnings **111/102** 持平 / 全量 **1113P/0F**（-a2 测试 +a1 对照测试，用例数不变）/ 零字体 grep=0 / diff 仅 menu_icons.rs（main.rs 98+/36- 为 coder-1 STREAMFONT-189 在途，零触碰）。
- **预览**：icons/ 16 张全保留（edit-A-* / edit-A1-* / edit-A2-* / edit-FINAL-* 各 {18,16}+x8）。
- **红线**：未 commit/版本未动/未出包（Gavin 攒批）/零凭证。



## 2026-09-08 — coder-2 — EDITICON-187-ALT ✅ A2 变体预览（cfg(test)，生产零改动，待 Gavin 对 A1/A2 二选一）

- **A2 几何**：铅笔整体下移——tip y=0.26（推导自档案气隙 1.3px）、tip x=-0.24 / END=(0.26,-0.24)（纯推算：沿 175→187 迁移线插值，result.md 已分列「档案实数/推导/推算」）；下划线 y=0.361、x -0.42..0.40 档案原值逐位照抄。
- **改动**：全在 `mod tests`（A2 常量 + `a2_pencil_covered` 参数化副本 + `a2_covered` + `dump_edit_icon_a2_preview`，+~110 行）；生产 `edit_icon_covered/UNDERLINE/pencil_covered/PENCIL_*` 逐字节未动。
- **A1 vs A2**（18px）：A1 笔尖行 11.5/线行 13/气隙 1.5px/线下留白 4 行、铅笔盒缘裁切原样；A2 笔尖行 13.7/线行 15/气隙 1.3px（档案）/线下留白 2 行、铅笔 s=1.0 角内收尾（右上 ~0.8px 空隙）。
- **验证**：fmt 幂等 / check 零 error、warnings **111/102** 持平 / 全量 **1113P/0F**（+1=A2 测试）/ 零字体 grep=0 / diff 仅 menu_icons.rs（main.rs +7/-0 为 coder-1 ESC-188 在途）。
- **预览**：`outbox/coder-2/icons/` 12 张 = `edit-A-*`(A1) + `edit-A2-*`(A2) + `edit-FINAL-*`(B) 各 {18,16}+x8，全保留供 Gavin 并排比。
- **红线**：未 commit/版本未动/未出包（Gavin 攒批）/零凭证。



## 2026-09-08 — coder-2 — EDITICON-187 ✅ 编辑图标换回候选 A「铅笔+单条下划线」（铅笔逐位不动，待主控验收 → Gavin 目视 → 并入下次 BUILD）

- **取证重建**：A 常量未留底（185 清理 + 184/185 squash），档案 `logs/20260907.md:571` 命中 A 下划线参数（y=0.361、x -0.42..0.40、7px/1.8px 邻距、笔尖-线 1.3px 气隙）。
- **关键取舍**：184 的 A 铅笔是「上移缩距」版（笔位与 185 定稿不同，原始常量丢失）；任务书红线「铅笔逐位不动」优先 → 下划线改 y=0.25 补偿，笔尖-基线气隙 1.5px ≈1.3px 构图；x/-0.42..0.40 按 archive（7px 净距/1.8px 盒缘）。
- **改动**：`menu_icons.rs` 44+/32-（UNDERLINE 单线 + `:27` 注释订正 + 测试断言换 A）；铅笔五常量逐位未动；main.rs 零触碰（+7/-0 为 coder-1 ESC-188 在途）。
- **验证**：fmt 幂等 / check 零 error、warnings **111/102** 持平 / 全量 **1112P/0F** / 零字体 grep=0 / diff 仅 menu_icons.rs 一文件。
- **预览**：`outbox/coder-2/icons/edit-A-{18,16}+x8` 4 张，**保留不清理**（任务书明示）。待主控目视 → 转 Gavin 确认形态 → 通过则并入下次 BUILD（本单零 main.rs 改动，D2D/GDI 兜底自动吃新像素，免集成单）。
- **红线**：未 commit/版本未动/零凭证/未 cargo build --release。



## 2026-09-08 — tester-1 — BUILD-186 ✅ 出包：ESC-178 + EDITFONT-183 + EDITICON-185 定稿（精简流程，跨日构建，待主控验收）

- **基线**：HEAD `fd527e0` clean（ESC-178/EDITFONT-183/EDITICON-179~185 全链已由主控提交）。构建 00:48-00:50 跨日，沿用 20260907.md logs。
- **精简流程**：Step1 清进程（无残留）→ Step2 git-log 法 UI 免重建（f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序（2m10s，feiyin-ime 111 / crash-reporter 5）→ Step4 cp -p 同步 Publish/。
- **七项核验全 PASS**：① 主程序 09-08 00:50 本次构建；② 两副本 sha `6624cdd1…` 一致异于 `9d60f458…`；③ ProductVersion 0.9.0.0；④ 冒烟 PID 25412 Responding=True 无 panic 已清理；⑤ config.toml sha `3186ec8c` 不变；⑥ warnings 111/102 持平；⑦ 🔴 **二进制字面量判别探针正反对照 PASS**（`ESC-178:` debug 日志可构造：PRE186 反向 0 命中 / 新包正向 1 命中含 7 个前缀片段；`OVERLAY_EDIT_FONT_SIZE` 源码 grep=6 ≥5）。
- **回归（并行）**：cargo test 全量 **1112P/0F/9I** + hotkey 51P + ui_guard 2P；Vitest/E2E Skip。🔴 crash_reporter config 批量 FAIL **本轮未出现**（已立跟踪项）；asr_074/asr_056 异域偶发也未出现。
- **出包语义**：ESC-178 轮询旁路（🔴 原消息路由机制未定待 debug 日志端测定案）+ EDITFONT-183 编辑框 14→16px + EDITICON-185 图标定稿候选 B（铅笔+两条短文本线，Gavin 选定）；🔴 已知取舍三条如实写入（选区反白不渲染/他窗 ESC 也取消/文字 14→16 跳变）。不出端测清单给 Gavin，主控自出。
- **红线**：未 commit / v0.9.0 未动 / 未 cargo clean / 未 cargo tauri build / 零凭证 / 无临时文件。






## 2026-09-08 — 主控 — BUILD-193 ✅ 出包 + Gavin 端测全过（tester-1 故障，核验由主控代做）

- **产物**：`Publish/feiyin-ime.exe` @13:44:56 sha `d081c742…`，两副本一致，异于上包 `6624cdd1…`；v0.9.0.0 未动；config.toml sha `3186ec8c` 未变。HEAD `8280927` 代码区 clean。
- **本批四改动**：EDITICON-190（图标 A2）+ STREAMFONT-189（上屏字号 16）+ ESC-188（ESC 陈旧位加固）+ FIX-192（右侧空白结构修复）。
- **tester-1 故障**：出包后模型 API `Bad Request: deepseek-v4-flash`，未交验证报告。产物七项核验由主控独立完成；回归以 coder-1 交付 FIX-192 时的全量 1113P/0F 为准（跑的即最终源码状态，其后零改动）。
- 🔴 **出包顺带发现 `[TOML-ALL-NUL-001]`**：随包两份规则词表整文件全 NUL，已修复，详见 troubleshooting。
- **端测**：Gavin「端侧全部通过！」，一票否决项「无新增闪烁」通过。
