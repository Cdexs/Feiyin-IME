## 2026-08-04 — coder-1 — ITN-FIX-BIGNUM-027-E (DEC-042 补完) 数量级锚定最小单位全面落地

- **来源**：Gavin 否定027-D二分，DEC-042补完全面适用。基线 f489e5b
- **改动**：新增format_dec042_magnitude+隐式分支乘数修正(亿×1e7/万×1e3)+孤立判定统一；12条旧断言更新；10条新测试。is_unit/format_currency_chain/format_weight_chain本体零改动
- **验证**：基线 itn:: 186/0/0 → 196/0/0（+10新增，12条预期内绿转红，174条预期外全绿）；cargo check + --tests 双0；UTF-8 OK；调用点安全6条全过；第八节4组零回归8条全绿
- **边界**：仅 src/itn.rs；DEC-043合规(三套逻辑并存不统一)；未构建/出包；未改版本号；UTF-8用edit工具
- **详情**：result.md（10353 B 非空）+ logs/20260804.md

## 2026-08-04 — coder-1 — ITN-FIX-BIGNUM-027-D (DEC-042) 隐式补全保留锚定单位（行为变更）

- **来源**：Gavin DEC-042 拍板。基线 967cd8d（027-C 已提交）
- **方案评估**：盘点 14 调用点，UnitChain 的 .parse() 若返回带单位串会崩，加孤立判定规避
- **改动**：big_unit_anchor 记录 + 隐式分支孤立判定 + 4 条旧断言更新 + 5 条新测试；is_unit 本体零改动
- **验证**：基线 itn:: 181/0/0 → 186/0/0（+5，4 条预期内绿转红，177 条预期外全绿）；cargo check + --tests 双 0；UTF-8 OK；调用点安全 6 条全过
- **边界**：仅 src/itn.rs；is_unit 本体零改动；未构建/出包；未改版本号；UTF-8 用 edit 工具
- **详情**：result.md（11499 B 非空）+ logs/20260804.md

## 2026-08-04 — coder-1 — ITN-FIX-BIGNUM-027-C + 027-C-2 ✅ 「两」误判致亿级金额蒸发修复

- **来源**：接续 027-A/B（已提交 `136f70f`）。027-A/B 修完后 `一亿两千...` 仍错，027-C 是最后一块。基线 `136f70f`（ahead 42）
- **方案协商**：coder-1 实测发现两个阻断点（主控原分析只定位第一处 large_amount_keep_wan_yi）。修第一处后亿能结算但 :677 two_is_unit 在「两」break。主控拍板扩范围 027-C-2 一并修
- **027-C 改动**：large_amount_keep_wan_yi 万/亿分支加 `big_starts_new_number` 消歧——after_big 首字是数字+第二字进位单位→不break
- **027-C-2 改动**：two_is_unit 加「两后跟 is_cn_unit_char→返回false」，补齐注释原意「进位单位由进位组合路径自行继续此处不误判」。is_unit 本体零改动（git diff 自证）
- **第三处注释-实现不符**：two_is_unit 注释说不误判进位单位但实现只查 all_units。连同 4.6 亿级、geometric_order_hazard 共 3 处，主控要求汇总判断系统性
- **第六节#4 实测**：不是真缺陷——丙型链 parse_cn_number 内部已折叠万/亿到 result，不丢弃
- **验证**：基线 `itn::` 168/0/0 → **181/0/0**（+13 新增零绿转红）；cargo check + --tests 双 0；UTF-8 U+FFFD=0；is_unit git diff 空；017 全套 14 条逐条零回归（先实测现状再写断言）；4.2 六条 + 4.3 四条 + 4.4 六条逐条通过
- **边界**：仅 `src/itn.rs`（两处消歧 + 13 测试）；is_unit 本体零改动；未构建/出包/启动 exe；未改版本号（0.7.3）；未用 git 破坏命令；UTF-8 用 edit 工具
- **详情**：`/d/Workspace/CodeLab/collab/outbox/coder-1/result.md`（12757 B 非空）+ `logs/20260804.md`

## 2026-08-04 — coder-1 — ITN-FIX-BIGNUM-027-A + 027-B ✅ 大额数字进位缺陷修复（027-C 发现另开单）

- **来源**：Gavin 2026-08-04 端测「一千零四十六万八千七百四十一」→`10469740`（应为 `10468741`），第四种失败模式（数值静默改错）。Gavin 新指令「数字要支持从亿到个位的量级跨度」倒逼主控复查出 027-B（万级公式，差 4 个数量级）。基线 `ad8bbd0`（ahead 41）
- **方案协商**：coder-1 实测发现 027-C（公告8用例中2条因「两」∈units.weight 被 large_amount_keep_wan_yi 的 is_unit starts_with 命中在亿分支 break，027-B 公式修复不可触及）。主控拍板本轮仅修 A+B，027-C 另开单（is_unit 是 ITN 最核心公共函数，动它风险最高）
- **027-A 改动**：新增 `unit_since_big` 状态位；十/百/千分支在 `big_unit_seen` 时置 true；万/亿结算重置 false；末尾判据 `big_unit_seen && !zero_since_big && !unit_since_big`。隐式千位补全适用边界显式声明：万/亿后 + 该段内无零 + 该段内无进位单位
- **027-B 改动**：万分支 `result = (result + section) * 10000` → `result += section * 10000`；亿分支保持 `(result+section)*1e8` 不变（亿是最大单位，反例「一万亿」验证）
- **4.6 确认**：「三亿五」代码乘 1000 得 `300005000` ≠ 注释 `350000000`（五→五千万），注释-实现不符，只报不改
- **验证**：基线 `itn::` 153/0/0 → 改后 **168/0/0**（+15 新增，零绿转红）；cargo check + --tests 双 0 errors；UTF-8 U+FFFD=0；4.2/4.3/4.4 共 11 条逐条通过；027-B 8 用例 4 条生产规则可达全过 + 2 条 027-C 阻断标注 + rules=None 隔离验证 8/8 全过
- **同模式盘点**：4 条（亿级隐式单位 4.6 / 027-C large_amount_keep_wan_yi / 十分支无前导默认1 边界 / 丙型 UnitChain 大单位 result 丢弃），只列不改
- **边界**：仅 `src/itn.rs`（生产 +191/-5 含注释，测试 +15）；`itn-rules.toml`/`src/llm/mod.rs`/`src/main.rs`/`src-tauri/**`/`ui/**`/`scene-rules.toml` 零触碰；未构建/出包/启动 exe；未改版本号（0.7.3）；未用 git 破坏命令；UTF-8 用 edit 工具
- **详情**：`/d/Workspace/CodeLab/collab/outbox/coder-1/result.md`（14497 B 非空）+ `logs/20260804.md`

# handoffs · voice-ime

## 2026-08-04 — tester-1 — TEST-SYNC-026 ✅ ITN-FIX-CHAIN-TEAR-026 测试同步（阶段三）

- **来源**：基线 `9edc839`（coder-1 026+026-B 代码落地但 TEST-SYNC 缺失）。改动 B 允许单段 currency 链，打开误转风险面。
- **A T6-T10 复核**：T6-T9 充分，T10 原 6 条补齐至 12 个 DEC-038 货币族保护词条（+二块钱/六块钱/八块钱/一毛钱/一角钱/五角钱）
- **B B1-B5 反向护栏**：23 条断言覆盖块/角/分/元/毛五组歧义，每条标注「现状锁定」。**疑似误转 2 条**：`三毛`→`3毛`（人名）、`九牛一毛`→`九牛1毛`（成语尾段），已标注 `TODO-026-REGRESSION`
- **C C1-C4 交叉回归**：21 条断言覆盖 017 六条端测/尾零边界/weight 族/016 班级简写
- **边界**：`src/itn.rs` `mod tests` 块内 +114 行，生产代码零改动；版本号未改（0.7.3）；未触碰 `src/llm/mod.rs` / `itn-rules.toml` / `src/main.rs` / `src-tauri/**` / `ui/**`；UTF-8 U+FFFD=0；未执行任何命令（阶段三）
- **详情**：`/d/Workspace/CodeLab/collab/outbox/tester-1/result.md`（非空）+ `logs/20260804.md`

## 2026-08-04 — tester-1 — TEST-EXEC-026 + BUILD-013 ✅ 全量回归 + 出包（阶段四+五）

- **来源**：基线 `9edc839` + TEST-SYNC-026。Gavin 已授权出包。
- **Step A 全量回归**：A1 766/0/6（基线 762→766，+4 新增）| A2 itn:: 153/0 | A3 llm:: 131/0 | A4 src-tauri 53/0 | A5 --list 772 自洽
- **Step B 红条分类**：4 红全 ① 断言写错（走查推断值与实测不符），无 ③ 真回归。修正 6 处断言值（一元二次→1元二次、三元钱→3元钱、五块零→5元、三块钱→3块钱、五块钱→5块钱、九牛一毛→九牛一毛）
- **Step C BUILD-013**：三步构建 + Publish/ 同步。产物 `feiyin-ime.exe` `626bc2e2bc4f` / `feiyin-ime-ui.exe` `7f5b0ce6a6f4` / `crash-reporter.exe` `afd7f48d4f1b`
- **Step D 验收 7 项全过**：mtime 链通过（src/itn.rs 00:43 < exe 00:46~00:48）| 两 toml 三副本一致（`ed77a912`/`7c1f0620`）| 二进制变化确认（≠ BUILD-012 `db07cefd8d51`）| 反向探针 3/3=0 | 正向探针 4/4≥1 | 冒烟 PID 21604 Responding=True 零 panic
- **边界**：仅改 `mod tests` 断言（6 处修正）；版本号未改；未用 git 破坏命令；UTF-8 U+FFFD=0
- **详情**：`/d/Workspace/CodeLab/collab/outbox/tester-1/result.md`（非空）+ `logs/20260804.md`

## 2026-08-03 — tester-1 — TEST-SYNC-024 + TEST-EXEC + BUILD-012-VERIFY ✅ 023 续做收口（session 崩溃中断后）

- **来源**：上一 session 崩溃中断，TEST-SYNC-024（`src/llm/mod.rs` +90/−8，全在 `mod tests`）与 BUILD-012（17:42 产物已存在）未收口。Gavin 18:3x 指令续做。
- **Step A TEST-EXEC**：`cargo test --bin feiyin-ime` 752/0/6（+2 来自 TEST-SYNC-024）/ `llm::` 131/0/0（vs BUILD-011 基线 129/0/0）/ `itn::` 139/0/0 / `src-tauri` 53/0/0 / `--list` 758=752+6 自洽。零红条，无需换锚。
- **Step B BUILD-012-VERIFY（默认不重建）**：独立复核 7 项全过——三 exe sha256 两副本一致（`DB07CEFD8D51`/`46D0F31E149D`/`699ED9656958`）/ 两 toml 三副本一致（`7C1F0620`/`ED77A912`）/ ProductVersion 0.7.3.0/0.7.3 / mtime 链通过 / 正向探针 8/8 ≥1 / 反向探针 4/4 =0 / 冒烟 PID 11088 Responding=True 零 panic。
- **Step C 文档收口**：`logs/20260803.md` + `handoffs.md` + `CHANGELOG.md` + `todo.md` + `troubleshooting.md` 五处已更新。
- **边界**：生产代码零改动；版本号未改；未用 git 破坏性命令；UTF-8 用 Python `codecs.open` 写入。

## 2026-08-03 — coder-2 — FORMAT-F3-SEMANTIC-021 + PROMPT-ARCH-020 + FORMAT-F3-MARKERS-023 ✅

### 021 + 020（F3 判据语义化 + 翻译路径假前提修复）
**文件**：`src/llm/mod.rs` + `scene-rules.toml`（3 处 F4）
**改动**：F3 DECISION RULE 从「标记字面重复」改为「语义并列」（`TWO OR MORE spans stand in a PARALLEL relation`）+ 新增 F3-semantic fallback 兜底授权（DEC-039 四语义齐全）+ F4 三处补无序族与 ILLUSTRATIVE 措辞 + 2 条负向 few-shot + 翻译路径常量补完整 SUSPECT 语义（不悬空引用 L0）+ T4 阈值 15000→16000。
**验证**：cargo check/check --tests 双 0 errors；llm:: 127/2/0（两条红归 tester-1 断言锚旧字面）；UTF-8 U+FFFD=0；T4=15469<16000。
**主控验收**：6 项全过，长度净增 +2198 接受。已提交。

### 023（恢复并扩充四语枚举标记清单，Gavin 推翻 021 精简）
**文件**：`src/llm/mod.rs`
**改动**：恢复 `9eb80b7` 完整清单 132 标记短语 0 遗漏 + 四语扩充（中 `其次是/接下来/像是/好比` 等 / 英 `to start with/among them` 等 / 日 `はじめに/例を挙げると` 等 / 韩 `첫 번째로/가령` 等 / 结构性句式 4 新增）+ per-language contrast 恢复四语改用标记不同形态演示语义并列 + F3c 四语 unordered 改标记不同形态（修正韩语错别字 `쓰하는`→`쓰는`）+ T4 阈值 16000→40000 定位变更为探测异常暴涨。
**验证**：cargo check/check --tests 双 0 errors；llm:: 127/2/0（两条红归 tester-1：①`!contains("for instance")` 反向断言与恢复冲突 ②`contains("比如说有些学生头发过长")` 旧字面，F3c 改为标记不同形态）；UTF-8 U+FFFD=0；T4=17672<40000。
**移交说明**：tester-1 需改 2 条断言（删除 `for instance` 反向断言 + 换锚 `比如说` 为新字面）；兜底授权与保守默认双向原样保留。

## 2026-08-03 — tester-1 — TEST-SYNC-019 + TEST-EXEC + BUILD-010 ✅ 三段串行收口

- **来源**：两提交 `790e316`（018，提示词分层契约重构）+ `9eb80b7`（017，货币/度量链数值静默改错修复）。基线 `9eb80b7`（ahead 38）
- **P1 红条换锚**：`suggestions_instruction_always_appended` 第2条 assert 从已删除的 `This directive OVERRIDES...` 声明换锚为 `SUGGESTION_INSTRUCTION` 的 `(1) Return the CORRECTED form only`。设计变更致断言过时，非回归，生产代码零改动
- **P2 017 复核**：coder-1 5 组测试（T1-T5）充分性确认——六条端测/死数据锁/虚指护栏/反向护栏全覆盖。016 班级简写 5 条在既有测试中已覆盖，未新增
- **P3 018 复核**：守恒夹具 4 条非空壳（双向比对+白名单显式化）/ L0 置顶 / UNIT_SYMBOL_PROTECTION 假前提已修 / i18n 三处 §2/§4/§5/§7 已裁
- **P4 B 批契约测试**：新增 T1（Topic 跨层唯一归属，同层重复允许）/ T2（矛盾对层号小的赢）/ T3（层序+层内插入序）/ T4（长度预算，实测 ~11500 字符，阈值 15000）。编译时修正 2 处：Topic derive Hash + PromptRule/Topic import
- **阶段四 TEST-EXEC**：`cargo test --bin feiyin-ime` 749/0/6 ✅ | `itn::` 139/0/0 ✅（与 coder-1 自报一致）| `src-tauri` 53/0/0 ✅ | `--list` 755 自洽
- **阶段五 BUILD-010**：三步全量构建 + Publish/ 同步。三 exe sha256 两副本一致；两 toml 三副本一致（itn-rules.toml 37,291B 含 017 改动）；ProductVersion 0.7.3.0/0.7.3；mtime 链通过；探针有效（新增串命中 ≥1，旧措辞命中 0）；冒烟 PID 728 零 panic
- **边界**：`src/llm/mod.rs` +1 derive (Hash) / +2 import / +1 assert 换锚 / +4 测试；生产代码零改动；版本号未改；未用 git 破坏性命令；UTF-8 用 edit 工具
- **详情**：`/d/Workspace/CodeLab/collab/outbox/tester-1/result.md`（非空）+ `logs/20260803.md`

## 2026-08-02 — tester-1 — TEST-SYNC-016 ✅ 015 + 016 测试同步（阶段三，零命令执行）

- **来源**：两提交 `ae452fb`（015，F3b/Output format 对称补 LONG 限定）+ `81cf51a`（016，年级班级简写守卫）。基线 `81cf51a`（ahead 35）
- **A 过时断言**：`build_format_instruction_block_f3_exemplification_enumeration` d 项换锚（`may be FULL SENTENCES` → `List items here are FULL SENTENCES or longer clauses` + `SHORT noun phrases MUST NOT be bulleted` + 负向护栏）。意图=长句 AND 短项禁止 bullet 两侧缺一即退化
- **B 015 覆盖 6 条**：item_form_short_long_split / f3a_f3b_long_symmetry / output_contract_short_inline_exception / f3c_short_inline_example / ⭐item_form_structural_guard（SHORT 内联与 LONG 限定同时存在，写 recency 软化教训）/ false_unchanged_drift_guard
- **C 016 覆盖 6 条**：T1 正向 4 条（一三班/五一班/初二三班/高一四班 全汉字）｜T2 句子形态｜T3 反向护栏 8 条（含 **十三班→13班** code 走查 + coder-1 实测双重确认）｜T4 proper_nouns 保护｜T5 班非数字后｜⭐T6 降级（缺 serial_suffixes 旧 toml → 一三班→13班，锁 [TOML-STALE-001]）
- **自验**：锚点与生产文本字节比对全过；负向锚点生产代码确认缺席；括号平衡；UTF-8 U+FFFD=0；`git diff -w` 真实 diff `+265/−3` 全在测试块，生产零改动
- **边界**：未跑任何命令（阶段三禁执行）；未构建/出包/启动 exe；未改版本号（0.7.3）；未用 git 破坏命令；UTF-8 用 edit 工具
- **详情**：`/d/Workspace/CodeLab/collab/outbox/tester-1/result.md`（非空）+ `logs/20260802.md`

## 2026-08-02 — coder-1 — ITN-FIX-GRADECLASS-016 ✅ 年级班级简写被逐位串误合并

- **来源**：Gavin 2026-08-02 端测 `我是一三班的学生`（=一年级三班）被转 `13班`；同类 五一班/初二三班/高一四班。基线 `ae452fb`（ahead 34）
- **根因**：`parse_cn_number` 逐位串 `serial_len>=2 && !next_is_unit` 把「一三」当两位数 + `decide_conversion` `consumed>=2` 无条件转；「班」不在 classifiers。`五一` 在 proper_nouns:`一三` 不在 = DEC-038 随机覆盖病症
- **方案协商（重要）**：主控原方案「跳 :582 early return 落进位组合路径」经分析会产出 `("3",2)`（进位路径末位 digit 覆盖）→`3班`撕裂。协商采纳正确落点——**守卫命中时 `parse_cn_number` 直接 `return None`**（主循环 :1600 `if let Some` 短路，字符走单字路径，班非单位/量词→全汉字）。主控 ACK 采纳原方案作废
- **改动**：`itn-rules.toml` +7 新增 `[protect.serial_suffixes]`(words=["班"])；`src/itn.rs` +38/−1（Protect/CompiledRules 加字段 + from_rules 填充 + parse_cn_number 守卫 `serial_len==2 && 后继命中 serial_suffixes`→None + 订正过时注释 ≥3→≥2 不改实现）
- **主控复核点已实读确认**：① 链扫描 :830/:1290/:1348 三处 None 均 `break` 推进无死循环，且守卫 None 短路使 :1622 永不到达（chain_end==i 空转不可能）② 甲乙丙型 :988/:1120/:1228 `?` 传播期望行为，既有断言零误伤
- **验证**：目标 4 条全汉字（一三班/五一班/初二三班/高一四班）；反向护栏 10 条全过（九八年→98年、三零二房间→302、二零二六→2026、幺三八零零→13800、三年二班保持、**十三班→13班 现行为实测未改**、一班/三班保持、五一/五一广场保护）；`cargo test --bin feiyin-ime itn::` **128 passed / 0 failed**；双 cargo check 0 errors；UTF-8 U+FFFD=0
- **🔴 全量唯一红条（非本任务引入）**：`llm::tests::build_format_instruction_block_f3_exemplification_enumeration`（src/llm/mod.rs:1856 断言旧措辞 `may be FULL SENTENCES`，现文案 :875 为 `List items here are FULL SENTENCES or longer clauses`）——coder-2 015 改动措辞未同步断言。`git status --short src/llm/mod.rs` 空输出证明工作区零改动，红条在 HEAD 即存在，**归 tester-1 TEST-SYNC 换锚点**
- **边界**：仅 `src/itn.rs` + `itn-rules.toml`（45 insertions/1 deletion）；`src/llm/mod.rs`/`scene*`/`main.rs`/`src-tauri/**`/`ui/**` 零触碰；未构建/出包/启动 exe；未改版本号（0.7.3）；未用 git 破坏命令；UTF-8 用 edit 工具
- **下游需知**：本次改 `itn-rules.toml`，tester-1 出包时需三副本同步（`[TOML-STALE-001]` 纪律）；端测观察点 `一三班` 类保持汉字、`十三班` 仍 `13班`
- **详情**：`/d/Workspace/CodeLab/collab/outbox/coder-1/result.md`（非空）+ `logs/20260802.md`

## 2026-08-02 — coder-2 — FORMAT-F3-SHORTITEM-015 ✅ F3b 与输出契约对称补 LONG 限定（收口 014 的 recency 冲突）

- **来源**：主控独立 Read 取证——014 的 F3-item form（SHORT→内联）位于 F3a/F3b 之前，下游两处仍无条件要求 bullet/多行且 recency 更高（`src/llm/mod.rs` 注释 :813-817「后段软化前段」失败模式），Gavin 买菜用例（无序+短名词短语）恰好命中。基线 `3b4b622`（ahead 32）
- **范围**：仅 `src/llm/mod.rs` 的 `build_output_format` 真分支（4 处文本：3 语义 + 1 示例）；假分支零改动
- **改动 1**：F3b 标题补 `AND items are LONG`，与 F3a :869 `items are LONG` 对称
- **改动 2**：F3b 正文 `List items may be FULL SENTENCES ... do NOT need to be short noun phrases` → 语义反转 `List items here are FULL SENTENCES ... SHORT noun phrases MUST NOT be bulleted — per F3-item form above they are joined INLINE with the enumeration separator`（`Narrative exemplification` 句保留）
- **改动 3**：Output format 行 `MUST span multiple lines when F3 applies` → `when F3 applies AND F3-item form routes the items to a LIST`，补 `or when F3-item form routes SHORT items INLINE, output a single continuous paragraph`
- **改动 4**：F3c 插入 Chinese SHORT items inline 正向示例（买菜句 → 顿号内联，与 Gavin 端测用例一致）
- **验证**：`cargo check` + `cargo check --tests` 双 0 errors；`cargo test --bin feiyin-ime llm::` **113 passed / 1 failed**——唯一红 `build_format_instruction_block_f3_exemplification_enumeration:1856` 断言 `may be FULL SENTENCES`，**断言过时**（改动2 明确删除该措辞），归 tester-1 TEST-SYNC 换锚点，未改断言；Python 解码核对全 PASS（`AND items are LONG`/`MUST NOT be bulleted`/买菜示例/`Narrative exemplification` 保留/`routes SHORT items INLINE`）；全文件不再含 `they do NOT need to be short noun phrases`；UTF-8 无 mojibake；字符数（解码后）`build_output_format(true)` 5183→5511（+328）
- **设计约束遵守**：F3-item form 本体零改动；段落位置零移动；`INLINE_SEPARATOR_RULES` 一字未改；:891-894 JSON `{{ }}` 转义保留
- **边界**：`src/text_normalizer.rs`/`src/scene/mod.rs`/`scene-rules.toml`/`src/itn.rs`/`itn-rules.toml`/`src/main.rs`/`src-tauri/**`/`ui/**` 零触碰；未构建/出包/启动 exe；未改版本号（0.7.3）；未用 git 破坏命令（仅 `git show` 只读对比）；UTF-8 用 edit 工具
- **详情**：`/d/Workspace/CodeLab/collab/outbox/coder-2/result.md`（非空）

## 2026-08-02 — coder-2 — FORMAT-F3-SHORTITEM-014 ✅ 多行分支补「短项内联 / 长句列表」分流 + 四语措辞补充

- **来源**：Gavin 2026-08-02 端测——`今天出去买菜了，买了3斤土豆，一个西瓜，20斤大米，还有3斤香蕉`（msedge/Memos/multiline_safe=true）被拆成四行 `- ` 列表，但这是短名词短语清单，应顿号内联。主控定位：短项 `、`/长句 `；` 规则只存在于 `multiline_safe=false` 分支，`true` 分支没有「短 vs 长」区分。基线 `bba7f08`（ahead 32）
- **范围**：仅 `src/llm/mod.rs` 的 `build_output_format`（真/假两分支 + 新增共享常量）。`src/text_normalizer.rs` 零触碰（coder-1 并行）
- **改动 A · 短项内联/长项列表分流（重点）**：DECISION RULE 之后新增 **F3-item form**——数量 ≥2 确认枚举后按项目形态分流：SHORT（无谓语、无内部标点、≤6 字/词）→ **内联分隔符，不做列表**（含 Gavin 用例 `买了3斤土豆、一个西瓜、20斤大米、还有3斤香蕉`）；LONG（含谓语或内部标点）→ **列表**（`1. `/`- `）。边界：混合长短按多数项决定、不确定用列表；有序短项保留序号词（`第一个土豆，第二个西瓜` 内联）
- **改动 B · 四语措辞补充**：中文 `再者/最后一点/另外一点/第X条`；English `in addition/plus/and then/namely`；日本語（有序最薄仅 3 组，补 `①②③/最初に/続いて/それに/加えて/ほかにも`）；한국어 `또/이어서/끝으로/아울러`。**单行分支标记词同步补齐**至与多行分支覆盖一致（Python 核对 18 词 count=2 全 OK）
- **分隔符表抽共享常量 `INLINE_SEPARATOR_RULES`**：两分支 `format!(..., INLINE_SEPARATOR_RULES)` 共用，保证分隔符规则**字面一致**（避免两套说法漂移）。为此 `build_output_format` 返回类型 `&'static str` → `String`（真分支需运行时拼接常量）
- **精简意识**：真分支 6371 / 假分支 3345 字符（HEAD 版 4297/2649，增长主要为分流规则 + 四语补充词，均紧凑列举未造句）
- **验证**：`cargo check` + `cargo check --tests` 双 0 errors；`cargo test --bin feiyin-ime llm::` **114 passed / 0 failed** **零红条**（tester-1 已把断言锚到 build_output_format，我的改动保留全部关键措辞：`1. `/`- ` 符号、保守默认双向、DECISION RULE、四语标记、负向示例；新增分流不破坏任何断言）；UTF-8 Python 验证无 mojibake（U+FFFD=0）；18 个补充词两分支覆盖一致
- **⚠️ cargo fmt 连带 3 处**：既有测试块 :1659/:1872/:1934 三条 `assert!` 长行被 rustfmt 重排（零逻辑变化，[FMT-COLLATERAL-001] 保留）
- **边界**：`src/text_normalizer.rs`/`src/scene/mod.rs`/`scene-rules.toml`/`src/itn.rs`/`itn-rules.toml`/`src/main.rs`/`src-tauri/**`/`ui/**` 零触碰；未构建/出包/启动 exe；未改版本号（0.7.3）；未用 git 破坏命令；UTF-8 用 edit 工具
- **详情**：`/d/Workspace/CodeLab/collab/outbox/coder-2/result.md`（非空）

## 2026-08-01 — tester-1 — TEST-SYNC + TEST-EXEC + BUILD-RELEASE-20260801-008 ✅ 三轮收口（本轮 18 红最多）

- **来源**：2 提交 b60517a（F3 与输出契约合并到最末 + 四语标记穷举）+ 978afa7（系统提示词全英文）。基线 `978afa7`（ahead 31 未 push）
- **Step 0**：0a llm:: 8 红换锚点函数（build_format_instruction_block 只剩 F1/F2，F3/输出契约在 build_output_format）+锚定串更新；0b text_normalizer:: 10 红+1 隐形假绿改英文断言 + 2 新守卫（翻译路径不得含 Do NOT translate [LANG-MIXED-001]、5 指令串无 CJK [Gavin 纯英文]）；0c 四语标记；0d ⭐结构护栏（走 build_optimize_request 真实组装断言格式契约在 ANTI_HALLUCINATION 后且末段，注释写五次段落顺序教训）；cargo check 0 errors
- **Step A 全绿**：788/0/8（+4）；src-tauri 53/0/0；llm:: 114/0（104/8→全绿）；text_normalizer:: 61/0；--list 796=788+8 自洽；点名 3/3（0d 结构护栏/mastodon/memos）
- **B0-pre 构建前探针预验证**：旧 exe c4cfe76c 四新探针=0 + 旧中文串判别力=1 + Notepad=1
- **Step B**：两处实例无运行；构建 2m05s；Publish 同步 feiyin-ime.exe（fb74146b/11,901,440B）+ crash-reporter（9fb4022a），ui 未动；两 toml 三副本未变（910b2c1f/93ab3972）
- **Step C**：四新探针全≥1 + 旧中文串=0（判别力 1→0）；两副本 sha256 三 exe 全一致；0.7.3.0；mtime 23:48:31 > llm 23:43:14 > normalizer 23:41:59；冒烟 PID 21408 零 panic
- **详情**：`/d/Workspace/CodeLab/collab/outbox/tester-1/result.md`（WSL Python，106 行）+ `logs/20260801.md` §36

## 2026-08-01 — coder-1 — PROMPT-EN-UNIFY-013 ✅ `extra_instruction` 英文化（系统提示词纯英文）

- **来源**：Gavin 指令「系统提示应该用纯英文」。基线 `3490c2a`（ahead 29）
- **范围**：`src/text_normalizer.rs` 指令字符串 5 条英文化（**任务书说 4 条，实际 5 条**——`:197` 假名/谚文纯保护措辞也一并英文化，否则系统提示仍不纯英文）；简繁转换逻辑零改动
- **改动**：主路径 Simp/Trad 保留「不要翻译非中文」子句；翻译路径 Simp/Trad 不含（两组差异保持，翻译路径绝不可注入防翻译语义）
- **验证**：text_normalizer:: 49/10（10 红全断言中文子串过时归 TEST-SYNC）；双 cargo check 0 errors；全量 702/18（另 8 红为 coder-2 llm 既有红条，与本改动无关——src/llm/mod.rs 对指令零引用）
- **效果**：LLM 是否仍正确简繁归一单测无法验证，**需 Gavin 端测确认**（建议：说含繁体字形的话看是否归一简体）
- **详情**：`/d/Workspace/CodeLab/collab/outbox/coder-1/result.md`（非空）+ `logs/20260801.md`

## 2026-08-01 — coder-2 — FORMAT-F3-UNIFY-I18N-012 ✅ F3 与输出契约合并到 prompt 最末 + 枚举标记四语穷举

- **来源**：Gavin 2026-08-01 指令——「系统提示应该用纯英文，但在提示词里把各种枚举情况、枚举的用词、措辞都说到，并明确要求考虑中文、英文、日文、韩文的输入场景」。背景：连续两轮措辞层修复（F3 DECISION RULE、MAY→MUST）均失败（prompt_tokens +271 证明新文本已加载仍被压制），主控定论根因是**结构**——prompt 里三处谈格式靠位置争优先级。基线 `3490c2a`（ahead 29）
- **范围**：仅 `src/llm/mod.rs`。`src/text_normalizer.rs` 零触碰（coder-1 并行改 extra_instruction 英文化）
- **改动 A · 结构合并**：
  - `build_format_instruction_block` 只留 F1/F2（filler + self-correction，与格式无关，留原位 :554）；参数改 `_multiline_safe`
  - `build_output_format` 重写为**合并的 F3+输出契约**（全仓唯一谈格式/列表/标签的地方），调用点从 :589 移到 **:596（ANTI_HALLUCINATION 之后，最末，recency 最高）**
  - **放置理由**：ANTI_HALLUCINATION 约束「语音不是问题、只重排返回」，与格式排版正交，放其前面不影响效力；格式段获最高 recency（本批修复目标）
  - `multiline_safe=false` 分支同样合并（单行契约 + i18n 五语分隔符表，**一字未改**）
- **改动 B · 枚举标记四语穷举**（Gavin 指令核心）：指令散文全英文；标记词用目标语言原文——
  - 中文有序：第一/第二/第三、第一点/第二点、一是/二是/三是、首先/其次/再次/最后、然后/接着、一来/二来、其一/其二
  - 中文无序：比如/比如说/例如/譬如/像、有的…有的…、**有些…有些…**、**有一些…还有一些…**、一些…一些…、还有/另外/此外/以及/包括/诸如/等等、一方面…另一方面、一类是…一类是
  - English 有序：first/second/third、firstly/secondly/lastly、step 1/2/3、point one/two、to begin with、next、finally
  - English 无序：for example/for instance/such as/like/including/includes/also/another/additionally/moreover/besides/as well as/e.g./etc./some… some…/one… another…
  - 日本語：第一に/第二に/第三に、まず/次に/それから/最後に、一つ目/二つ目/三つ目、たとえば/例えば、など/とか、また/さらに/そのほか、〜や〜、ある人は…ある人は…、一つは…もう一つは…
  - 한국어：첫째/둘째/셋째、먼저/다음으로/마지막으로、첫 번째/두 번째、우선、그다음、예를 들어/예컨대、등、그리고/또한/게다가、~같은、뿐만 아니라、어떤 사람은…어떤 사람은…
- **DECISION RULE 跨语言**：改为语言无关表述 + 每语一组「1 次 vs ≥2 次」对照（Chinese 比如/English for example/Japanese たとえば/Korean 예를 들어）
- **few-shot**：中文保留既有（2 有序 + 2 无序含 `比如说` 长句 + 单例负向）；**en/ja/ko 各 1 条无序正向**（长句形态）+ **非中文负向例各 1 条**（单 for example/たとえば/예를 들어 不列表）
- **改动 C · 四语覆盖声明**：合并段开头显式声明适用于 Chinese/English/Japanese/Korean 四种输入语言，按输入文本主体语言选用对应标记集
- **验证**：`cargo check` + `cargo check --tests` 双 0 errors；`cargo test --bin feiyin-ime llm::` **104 passed / 8 failed**——8 红全部**断言过时**（F3/i18n 表从 `build_format_instruction_block` 移到 `build_output_format`，测试仍指向旧位置/旧措辞），逐条判定无真回归，归 tester-1 TEST-SYNC 未改断言；UTF-8 Python 验证无 mojibake（U+FFFD=0）；四语标记/few-shot/跨语言 DECISION RULE/覆盖声明全部 Python 核对 PASS；i18n 五语表 6 串核对 PASS 一字未改；fmt 零测试块连带（3 hunk 全在改动区域）
- **⚠️ 8 条红清单**（供 tester-1）：`build_format_instruction_block_single_line_when_not_multiline_safe`(:1560) / `build_output_format_single_line_when_not_multiline_safe`(:1577 断言 Line 1:) / `build_output_format_multi_line_when_multiline_safe`(:1599) / `build_output_format_multi_line_mentions_numbered_and_bullet`(:1617 断言 numbered lists 措辞) / `build_format_instruction_block_multi_line_when_multiline_safe`(:1570) / `build_format_instruction_block_four_quadrants`(:1651) / `build_format_instruction_block_false_i18n_separators`(:1705) / `build_format_instruction_block_f3_exemplification_enumeration`(:1767)——断言全部从 `build_format_instruction_block` 迁到 `build_output_format`
- **边界**：`src/text_normalizer.rs`/`src/scene/mod.rs`/`scene-rules.toml`/`src/itn.rs`/`itn-rules.toml`/`src/main.rs`/`src-tauri/**`/`ui/**` 零触碰；未构建/出包/启动 exe；未改版本号（0.7.3）；未用 git 破坏命令；UTF-8 用 edit 工具
- **详情**：`/d/Workspace/CodeLab/collab/outbox/coder-2/result.md`（非空）

