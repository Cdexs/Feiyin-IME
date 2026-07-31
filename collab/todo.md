# 任务列表 · voice-ime

> 当前版本：**v0.7.3**（三处版本号均 0.7.3，2026-07-30 主控复核于 `a07a089` 后重新取证：`Cargo.toml` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json`）
> ✅ **产物已是最新**（2026-07-30 23:00，BUILD-RELEASE-20260730-002）：三 exe 均为 v0.7.3，`target/release` 与 `Publish` 两副本 sha256 逐一一致，`itn-rules.toml` / `scene-rules.toml` 三副本均同步。含 ITN 顺序反转 + ℃ + 翻译双向化 + 几何术语修复 + Type A 碰撞词表 1386 条。冒烟实例 PID 23276 运行中。
> ✅ **已提交并 push**（2026-07-31 主控 session 启动取证，修正原「工作区尚未提交」的过时记载）：`0adb819 feat(itn): Type A 单位碰撞保护词表落地 + 五角大楼白名单`（10 文件，`itn-rules.toml` +227 / `src/itn.rs` +35），`git status -sb` = `## main...origin/main` 无 ahead/behind，工作区仅剩 `?? logs/20260731.md` 一个未跟踪新文件。CRLF 幽灵 diff 已不存在。`collab/` 现已入库受 git 管辖（`git check-ignore` 实测未被忽略）
> 端测方式（2026-07-25 Gavin 指示）：Gavin 已在**实际日常使用中自行测试**，端测项不再列入本文档；发现 bug 或优化点由 Gavin 邀请重新开单。

## 文档更新规则

1. **只保留新产生、进行中、验证失败、待排期或其他待决的任务**
2. **已完成的功能任务立即归档到 progress.md**，不在 todo 保留历史
3. **测试同步/构建/出包任务归入 CHANGELOG**，不在此文档详列
4. **新任务产生时立即写入**，不批量补
5. **不列端测跟踪项**（Gavin 自行使用中测试，有问题会重新开单）

---

## 🔄 进行中 · RESEARCH-ITN-V2-001 ITN 二代设计研究（Gavin 2026-07-31 四项需求）

> 派发方式：**双轨并行研究**——coder-1（`collab/research/itn-v2-design-001.md`）+ 主控独立稿（`collab/research/itn-v2-orchestrator-001.md` ✅ 已完成），交付后合并取优交 Gavin 拍板
> 阶段：研究，**零代码改动**。tester-1 不介入（依三阶段规则，TEST-SYNC 待代码任务产生后才派发）

| 编号 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| R1 | ITN 调用位置回移到 LLM 之前（**DEC-035 反转**）。核心矛盾：回移会复发 ℃ 缺陷。主控方案=双通道拆分（主通道 `normalize_with_rules`+符号在 LLM 前 / 补丁通道仅 `normalize_unit_symbols` 在 LLM 后）+ 新增 **F0 事实保全硬约束** | coder-1 + 主控 | 🔄 研究中 |
| R2 | 转换不彻底三缺陷：**A 撕裂**（`十一块九毛二`→`十一块9毛2`，根因=`itn-rules.toml:124` 的 `"十一"` 遮蔽前半段，主控已闭合根因链）／**B** `半`/`刻` 只识别不转换（`src/itn.rs:573-597` 两处均 `break`）／**C** 余数后缀文法族总体设计 | coder-1 + 主控 | 🔄 研究中 |
| R3 | 含数字地名白名单联网扩充（十三陵类）。**强制排在 R2-A 之后**，否则放大撕裂 | coder-1 | 🔄 研究中 |
| R4 | 格式化输出列表智能（有序/无序）。主控实读发现：**有序列表已实现**（`src/llm/mod.rs:814-819`），**无序列表完全缺失**；且 `multiline_safe=false` 场景（微信/浏览器/IDE/Unknown）**列表根本不生效** | coder-1 + 主控 | ✅ 研究完成 |

### ✅ 研究阶段已闭环（2026-07-31）→ 合并终稿 `collab/research/itn-v2-merged-final.md`

**coder-1 交付**（`itn-v2-design-001.md` 24795 B）已验收：主控 6 条取证逐条复核确认、`git status` 零源文件改动、`result.md` 2286 B 非空（`[COLLAB-WRITE-001]` 未复发）。

**主控独立取证查出三项**（未采信汇报表格）：

1. ✅ **采纳 coder-1 对主控的纠正**：`UNIT_SYMBOL_PROTECTION` 指令已存在（`src/llm/mod.rs:29`，无条件注入 `:560`），主控「新增 F0」提法收回，改为「强化已有指令」。**但主控补一条两人都没说透的**：该指令正文「input text **already contains normalized numbers**」在 DEC-035 反转顺序后**前提为假**（LLM 拿到的是汉字数字），它带 `ITN-CELSIUS-002-PROMPT` 标签写于 ITN 还在 LLM 前的年代，反转时未同步修订 → **这是支持 Gavin 回移决定的独立论据**（回移恢复该指令前提）。
2. ✅ **采纳 coder-1 路径异议**（兜底 ②→①），且主控在独立稿中已自行收敛到同一结论；**同时推翻主控派发时给的错误反例**——`十一月`→`11月` 是正确 ITN 输出，不是「误撤销」，coder-1 据此提的追加白名单要求取消。
3. 🔴 **主控查出 coder-1 一处事实错误 + 一项两人共同的错误假设**：
   - `两` 在 **Rust `src/itn.rs:397` `DIGIT_MAP`**，不在 toml（coder-1 §2.4 记为 toml）→ 影响：改数字表需重新出包，非纯数据热更
   - **保护词表对同一语法族的覆盖是随机的**：`X点半` 中 **一/六/八/九点半在保护表内，二/三/四/五/七/十点半不在** → 用户看到 `八点半`（全汉字）与 `四点半`（`4点半` 撕裂）行为完全相反；`一吨半` 在表内而 `两吨半` 不在，同理。根因是 1386 条机器词频派生词表把**规则性语法族切成随机子集**。**直接结论：修复方向不是补词表，而是把 `N点半`/`N<单位>半` 族整体移出保护表交甲型文法处理**——此项在 coder-1 方案中缺失，为合并新增的关键实施约束。

**实施批次建议**：P1（R1 双通道+指令强化 ｜ R4 列表）可立即并行启动，文件级零重叠；P2（缺陷A 修复）→ P3（甲型文法 + **成对移除保护词条**）→ P4（乙/丙型+单位层级表）→ P5（R3 地名扩表，≥3 字）。

**Gavin 已拍板（2026-07-31）**：G1 = **分治·货币也规范化**（`11.92元`/`5.8元`）→ DEC-037 ｜ G2 = **仅 D 扩大 doc 识别面**（未选 C，`multiline_safe=false` 保持现有内联行为）｜ G3 = **全量 P1-P5，一次性出包** ｜ 追加：**全或无**（`三年二班` 整段不转）→ DEC-037 附则

---

### 📋 TEST-SYNC 待办清单（跨 P1-P4 累积，派发 tester-1 时一次性交付）

> **派发时机**：P5 完成、全部代码收口后（三阶段规则，禁止与代码任务并行）
> **本清单持续累积，勿在派发前删减**

#### A · 既有断言需更新（不是回归，是断言过时）

| 项 | 现状 | 应改为 | 来源 |
| --- | --- | --- | --- |
| `time_half` | 断言 `八点半`→`8点半` | **`8:30`** | P3 移除保护词条 + 甲型文法。该断言本身是随机词表覆盖的产物（DEC-038） |
| `geometric_order_hazard_documented`（`src/itn.rs:1877`）注释 | 写「当前 `itn-rules.toml` **无互相为前缀的条目冲突**」 | **事实错误，实测 4 组**（五一⊂五一广场、十一⊂{十一国庆,十一月,十一边形}）。该「隐患护栏」测试断言的前提本身不成立，需重写为真实断言 | P2 主控取证 |

#### B · 需复核的 Worker 自行新增测试

| 项 | 说明 |
| --- | --- |
| coder-2 在 `src/llm/mod.rs` 新增的 5 条 `flatten_multiline` 单测 | 边界偏差（测试归 tester-1），但**主控裁定保留不回滚**——任务书原文只禁「改既有断言」未禁「新增」，属主控规格漏洞。请 tester-1 复核覆盖面是否充分 |

#### C · 新增功能需补覆盖

| 批次 | 需覆盖内容 |
| --- | --- |
| P1 | ITN 双通道（主通道 LLM 前 / 补丁通道 LLM 后）三路径覆盖；`UNIT_SYMBOL_PROTECTION` 事实保全条款 |
| P1 | F3 列表**四象限**：有序×多行 / 有序×单行 / 无序×多行(`• `) / 无序×单行(`、`「；」) |
| P2 | 撕裂修复（`十一块九毛二`）；确定性最长匹配（`十一月` 多次运行恒定）；`flatten` 分隔符守卫 |
| P3 | 甲型 9 实例 + 反例 4 条（`一刻钟`/`三点五`/`半小时`/`半个小时`） |
| P3 | **⚠️ 范围扩张需专项覆盖**：新增 `[units.time]`（小时/分钟）导致 **`三小时`→`3小时`、`五分钟`→`5分钟`**，此为未经请求的行为变化，须确认无副作用 |
| P4 | 乙/丙型；单位层级表；**`分` 族属消歧 3 用例**；**全或无**（`三年二班` 等 5 用例 + 连续性边界） |
| P5 | 地名白名单 ≥3 字条目；反向护栏（新增词不得让 `<前缀>+单位` 正常表达失效） |

#### D · 已知预期红 / 已知遗留

| 项 | 状态 |
| --- | --- |
| `三年二班` 类撕裂 | Gavin 2026-07-31 拍板「全或无」，P4 处理中 |
| ③ `try_parse_composite_block` | P4 已裁定**删除**（职责被丙型 + 全或无覆盖），相关测试需一并清理 |

**⚠️ 主控自我修正（需回写 troubleshooting `[ITN-PREFIX-SHADOW-001]`）**：2026-07-30 写入的「误保护 = 优雅降级」结论**仅在语义单元内不含其他可转数字时成立**。`十一块九毛二` 是反例——`check_protection` 命中后只前移游标不锁定后续（`src/itn.rs:697-703`），后半段照转 → 产出一半汉字一半数字的**撕裂**。撕裂是第三种失败模式，比误保护严重。

**待 Gavin 拍板三项（研究交付时一并呈报）**：① R2-C 货币目标形态（`11块9毛2` 保口语 vs `11.92元` 规范化）② R4 列表场景覆盖策略（维持现状 / 放开聊天类【危险】/ 单行降级保留序号 / 扩大 doc 识别面）③ R1 是否同时实施 F0 事实保全硬约束

---

## ✅ 工作冻结令已解除（2026-07-30）

> 原冻结令（Gavin 2026-07-29「目前是做好跨平台开发的代码重构和准备，ok 前先不做任何新代码开发」）已随跨平台重构批次闭环解除——Gavin 于 2026-07-30 解冻并指令实施 FIX-COT-LEAK-001-P0（依据 `logs/20260730.md`）。DEC-033 第 2 条「平台兼容为首要约束、不得再产出仅 Windows 可编译的新代码」**继续长期有效**。

---

## ✅ 已出包待端测 · ITN-COLLISION-TYPEA-002 Type A 碰撞保护词表（2026-07-30 23:00 出包 v0.7.3）

> **当前状态**：1386 条已随 v0.7.3 出包并经主控 9 项独立验收，PID 23276 运行中，**等 Gavin 端测反馈**
> 前置：ITN-COLLISION-TYPEA-001 阶段一（coder-2 交付，报告 `collab/research/itn-lexicon-collision-001.md`）
> **Gavin 拍板三项**：① 落地形式 = **新增 `[protect.unit_collisions]` 独立分组**（非并入 proper_nouns → 需改 Rust + 出包）② 规模 = **≤6 字档** ③ **明知前缀遮蔽缺陷仍照常落地**（2026-07-30，见下）

### 🔑 Gavin 的取舍判断（推翻主控首轮结论，已写入 troubleshooting [ITN-PREFIX-SHADOW-001]）

主控首轮结论是「整批否决、路线结构性不可行」，**Gavin 推翻并纠正了主控的严重性定性**：

`check_protection` 命中后只做「原文逐字抄出 + 游标前移」，**物理上不可能产出畸形文本**。故两种失败模式不对称：

| 失败模式 | 输出 | 性质 |
| --- | --- | --- |
| **漏**保护 | `三角形` → `3角形` | 文本被改坏，用户须手动修 |
| **误**保护 | `二分钟` 保持汉字 | 优化未生效，退回 ITN 之前状态，文本仍正确可读 |

**误保护 = 优雅降级；漏保护 = 输出损坏。** 主控原写的「误保护比不保护更糟」**错误，已收回**。且触发频率也不对称——遮蔽需「词条恰为文本前缀」（窄），碰撞只需「数字后跟单位首字」（宽，单位表 68 词）。**故用大词表换覆盖率是合理取舍。**

### 实测收敛数据（好于预期）

**1386 条只打破 1 条既有断言** —— 96 个 itn 测试中仅 `time_half`（`八点半`→`8点半`）变红。既有测试覆盖面与该词表重叠极小。

### ⚠️ 未收口项

| 项 | 状态 |
| --- | --- |
| `time_half` 单测红 | **未修**。Gavin 明确「不以测试通过为出包前提」。待端测后决定：改断言 / 从词表剔 `八点半` / 保持现状 |
| git 提交 | **1386 条 + `src/itn.rs` +34 仍未提交**，等 Gavin 指令 |
| 端测反馈 | 观察点见下 |

**端测观察点**：① 该转没转（新引入）：`二分钟` / `三元钱` / `九度电` / `一点半左右` ② 本该修好：`三角形` 不再变 `3角形`。若前者几乎遇不到 → 词表立住；若频繁硌手 → 按 troubleshooting 优先剔 ≤3 字那 603 条（遮蔽面最大）。

### 出包验收记录（2026-07-30 23:00 产物 / 23:12 主控验收，BUILD-RELEASE-20260730-002）

主控 9 项独立取证全过，**未采信汇报表格**：

- **探针 8/8 命中**：`八里庄北里` / `一个十七八岁` / `三角剖分` / `一个九十度` × 两副本全为 1。方法有效性已预验证——同串在旧 exe 中为 **0**，对照串 `八达岭`/`一心一意` 为 1
- **两副本 sha256 逐一一致**：`74e4b56a…` / `16acff20…` / `cc2ee873…`；主程序 11,798,016 B
- **`itn-rules.toml` 三副本 sha256 `9f36efcb…` 一致**（33,252 B）——这一步是本次最关键的，漏了则外置旧 toml（9,689 B）会赢过新内置默认，1386 条完全不生效且日志表现正常（[TOML-STALE-001]）
- **版本撕裂已消除**：主程序 0.7.3.0 / UI **0.7.3**（此前 UI 停在 0.7.2，故本次 Tauri UI 必须重建）
- **运行时实证**：debug.log 23:07 `ITN rules loaded from ...target\release\itn-rules.toml`，晚于 23:00 同步
- **因果交叉验证**（未只看数字）：`八点半` 在出包 toml 中确实存在 1 次，`src/itn.rs:1295` 断言 `8点半` → time_half 必挂，96/1 汇报自洽
- **边界合规**：源文件零越权改动；`scene-rules.toml` 三副本仍 `7b01b33c…`
- **⚠️ [COLLAB-WRITE-001] 第三次复发**：`result.md` 交付时 0 字节，经主控要求补写为 3644 B（tester-1 改用 WSL Python 绕过）

---

## （历史）派发时的方案记录

| 项 | 决定 | 依据 |
| --- | --- | --- |
| **词库来源** | jieba (MIT) + THUOCL (MIT)，**弃用 CC-CEDICT** | CC BY-SA 4.0 的 share-alike 对派生数据有传染性解释空间 |
| **过滤方案** | **F1 语义规则**（非 F2 长度阈值） | F2 误删「一个三十多岁」等年龄表达 287 条，又留下「一千万」等该剔的金额串 269 条 |
| **数量链** | 2107(Type A) → F1 净化 1741 → 剔 CC-CEDICT **1521** → ≤6 字 1390 → 去重 **1386** | 主控独立实算；词长分布 2字5/3字598/4字488/5字200/6字95 |
| **影响文件** | `itn-rules.toml` + `src/itn.rs`（数据源只读） | 边界零重叠，单 Worker |

**主控核心设计约束（任务书 §四，本方案成立与否的关键）**：新字段必须是 `HashMap<char, Vec<String>>` **分桶 Map，不是第六个 HashSet**。两条理由——① `check_protection`（`:1063`）对既有五个 `*_set` 做的是**遍历 + `starts_with`**（不是哈希查表），O(n) 复杂度，且 `:678` 每遇一个数字位置就调一次；分桶后最大桶 387 条（首字「一」），worst-case 降到 **28%**，实测只落 11 个桶 ② **`HashSet` 遍历顺序不确定 → 最长匹配无保证**（`一个三十` vs `一个三十多岁` 同时存在必然踩到）；桶内按字符数降序排序才是确定性最长匹配。分桶安全性依据：`rest` 起点恒为数字位置，故所有保护词首字必为中文数字。

**❌ 主控查出阶段一报告一处勘误**：§四.4 称「三角洲：jieba + THUOCL」并据此论证「弃用 CC-CEDICT 不影响 Type A 目标覆盖」。实测 `三角洲` **不在** 那 1521 条里（只在含 CC-CEDICT 的 v4 全集）。**结论不受影响**——`三角洲` 已由 `a07a089` 几何白名单硬编码进 `itn-rules.toml`。同理 `三角形`/`三角函数` 落在去重剔除的 4 条内，仍由既有分组保护。

**下游（严格串行，禁止并行派发）**：coder-2 完成验收 → TEST-SYNC-TYPEA-002（tester-1 补单测）→ TEST-EXEC（全量回归）→ **必须出包**（本次改 Rust，非纯数据免构建）。

---

## 📋 待排期 · ITN-COLLISION-TYPEB-001（Gavin 2026-07-30：先生成待办，以后再动手）

> 来源：RESEARCH-ITN-LEXICON-001（`collab/research/itn-lexicon-collision-001.md`）
> 前置：Type A 净化版落地（进行中）｜ **本项需改 Rust 代码，不是纯数据**

**问题**：`is_unit` 用 `s.starts_with(u)`（`src/itn.rs:259`）+ 中文无词边界 → 任何以单字单位词开头的词都会让前面的单字数字被误转。已实证一例（`三角形`→`3角形`，已由几何白名单挡住）。潜在碰撞词 **29,774 条**（Type B：单位字开头，如 `批发`/`元素`/`度假`/`节目`/`升级`/`克服`）。

**⚠️ 主控核实的关键结论（研究报告未发现，实施前必读）**：

1. **Type B 词放进 `protect.proper_nouns` 是 no-op** —— `check_protection`（`:1063`）的匹配起点是**数字位置**（`rest = chars[start..]`，`start` 为数字下标，调用点 `:678`），既有条目全是数字开头（`三亚`/`五一`/`八达岭`）。文本「三度假」的 `rest` 是「三度假」，**不以「度假」开头**，永不命中。研究报告建议的落地方式对这 93% 的词无效
2. **正确改法在 `is_unit` / `decide_conversion` 侧**：判断「数字之后的文本是否以某碰撞词开头、且该词比单位匹配更长」→ 是则不算单位语境
3. **性能必须一并处理**：`check_protection` 是对 **Vec 线性遍历** `starts_with`（不是 HashSet 查表，报告此处亦有误）。29,774 条 × 每个数字位置 = 每次听写十万量级字符串比较 → 需改**按首字分桶或 Trie**，并给实测数据

**实施前必须完成**：许可证原文证据（见 Type A 任务）｜ 完整影响面评估 ｜ 性能实测 ｜ 反向护栏测试（must-convert 表达不被误保护）

---

## ✅ ITN 顺序反转 + ℃ 独立通道 + 翻译双向化 批次闭环（2026-07-30 19:40）

> 来源：Gavin 端测两项反馈 —— ①「说摄氏度输出没转成 ℃」②「开了翻译按热键输出没译成英文」
> 决策：**DEC-035**（ITN 位置反转，DEC-030-① 原文作废）

| 编号 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| ITN-REORDER-001 | ITN 从「LLM 前」移到「三分支后、标点前」，一处覆盖三条路径 | coder-1 | ✅ 已验收（**主控修 1 处**：重建失败返回方向不符旧引擎 → 改为返回 None，防注入幻觉译文） |
| ITN-CELSIUS-003 | ITN 新增**独立于中文数字路径**的单位符号通道（`40摄氏度`/`40°C`→`40℃`），`itn-rules.toml` 新增 `[[unit_symbols.rules]]` 三条；`44度` 绝不转 | coder-1 | ✅ 已验收 |
| TRANS-BIDIR-001 | 方向按内容自动判定，删除 `should_translate_for_language` 门控；单槽位引擎换向；`target_language` 语义改「上次方向缓存」 | coder-1 | ✅ 已验收 |
| REFACTOR-DERIVE-TARGET-001 / REFACTOR-SHARE-TRANSDIR-001 | 三个函数移入平台中立模块（`translation/mod.rs` / `config/mod.rs`）去 cfg 门控，macOS 可直接复用 | coder-1 | ✅ 已验收 |
| TEST-SYNC ×2 轮 | 两条失效断言修正 + ITN 新域 7 条 + 方向矩阵 5 条 + 3 条测试随函数搬入中立模块 | tester-1 | ✅ 已验收 |
| TEST-EXEC ×2 轮 | **717 passed / 0 failed / 8 ignored**（分项 28+653+3+12+10+2+9 自洽 = `--list` 725）+ Tauri 53/0 + Vitest 54/0 + toml 三副本一致 | tester-1 | ✅ 已验收 |
| NPMLOCK-UNIFY-001 | corepack 钉 `packageManager: npm@11.16.0`；**零改动假设成立，lock 一字节未改** | coder-1 | ✅ 已验收（**主控修 1 处表述**：默认 npm 仍 11.6.2） |

**⚠️ 首轮 TEST-EXEC 6 个失败的根因（重要教训，已写入 troubleshooting 备查）**：`normalize_test` 测试助手只调 `normalize_with_rules`，**绕过公共入口 `normalize_numbers` 的第二阶段** —— **96 条断言长期在测一条非公共路径**。生产代码无误。主控改助手直调公共入口后全绿。与 `builtin_rules_parse_ok` 那个「内联 fixture 覆盖不到真实 toml」的黑洞属同一类缺陷。

**npm 环境收口（2026-07-30 19:41）**：`corepack enable npm` 需写 `C:\Program Files\nodejs` 会 EPERM（Gavin 两次尝试均未生效，主控实证）。改用免提权方案 —— shim 装到 `%USERPROFILE%\bin` + 前置用户级 PATH，全新进程实测 `npm --version` = **11.16.0**。

---

## ✅ 已并入 macOS 团队首批交付并通过 Windows 零回归验收（2026-07-30 14:40）

**合并**：`e9296ba`（并入 `5e3ed89`/`6f0b51e`/`4b2126b`/`b04596b`），本地 **ahead 2 未 push**（等 Gavin 指令）。

**Windows 零回归结论成立**：`cargo check --tests` + `cargo check src-tauri` 双 0 errors、**两个 `Cargo.lock` 零改动**、`cargo test` **695/0/8** + src-tauri **53/0/0** + Vitest **54/54**（主控 `--list` 独立计数 703 = 695+8 自洽，5 条 cfg 门控测试确在运行）。详见 `logs/20260730.md`。

**🔴 附带暴露我方缺陷（已记录，无需修复——对方已修）**：coder-2 的 windows target 段插在 `[dependencies]` 中间，静默吞掉 `tokio-tungstenite`/`futures-util`/`rustls`；Windows 上按构造不可见，主控 07-29/30 验收漏检。**审 `Cargo.toml` target 段调整时必须看段落边界之后还剩什么**。

### ⏳ 待办：npm lock 协同方案回执（`docs/MACOS-NPMLOCK-COORDINATION.md`）

| 项 | 状态 |
| --- | --- |
| 其 §1 论断（两平台都跑不了 `npm ci`） | ✅ **主控已在 Windows 复现 EUSAGE**（npm 11.6.2 / node v24.11.1） |
| 其 §8-1 问我方 npm 版本 | ✅ 已取得，与对方**大版本相同**（npm 11 / node 24），漂移风险低 |
| 执行方 | **建议由 macOS 侧执行**（其隔离实验已验证 12 个顶层 win32 条目无损，单文件单提交可 revert），我方做 §5 验证方 |
| ❌ 其 §5 步骤 4 有误 | 要求跑 `npm run tauri build`，但**我方出包从不走 npm tauri CLI**（[BUILD-001] 禁用 `cargo tauri build`），该包不在关键路径 → 回执时改为 `cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol` |
| ⚠️ 跨团队引用缺口 | ① 其引用 **DEC-034**，我方 decisions.md 只到 DEC-033 ② 其引用 `collab/` 内条目，而 `collab/` 在 `.gitignore` 内两侧不共享 → **建议约定「跨团队引用只许引 `docs/`」写入 `MACOS-HANDOFF.md`**（待 Gavin 点头派发） |

---

## ✅ BUILD-RELEASE-20260730-001 出包已完成并验收（2026-07-30 13:13 产物 / 14:12 主控验收）

**产物**：三 exe 于 **13:02~13:13** 重建并同步 Publish/，`target/release` 与 `Publish` 两副本 sha256 **逐一一致**：

| 产物 | 新 sha256（前 8） | 旧值（前 8） | 大小 |
| --- | --- | --- | --- |
| feiyin-ime.exe | `8da29081` | `e35679bd` | 11,615,744 B（+12,288） |
| feiyin-ime-ui.exe | `d9db29e3` | `0d76eca1` | 10,026,496 B（−512） |
| crash-reporter.exe | `950e1474` | `8bfabfb5` | 24,858,624 B |

**主控独立验收（未采信汇报表格）**：

- **V1 决定性探针**：`grep -ac "LLM response meta" feiyin-ime.exe` = **1**（旧 exe 实证为 0）→ `ff492ef` 确已进 exe，两副本均命中
- **✅ 主控补证 tester-1 降级后的缺口（本次验收最有价值的一项）**：它把 V2 标 N/A 并说明「`src-tauri/src/llm.rs` 无 `LLM response meta`」——**属实**（主控核读源码确认 P0-5 镜像只在主程序侧），但它**未换探针证明 Tauri 侧新代码就位**。主控改用 `ff492ef` 在 Tauri 侧引入的独有串复测 UI exe：`enable_thinking`=1 / `reasoning_content`=1 / `disabled`=8 → **P0-1/P0-2 的 Tauri 侧镜像确已就位**
- **V4 ProductVersion 0.7.2.0**（UI 侧 0.7.2）→ 未升版，红线遵守；三处版本号文件复核均为 0.7.2
- **V5** 产物 mtime 13:02~13:13 **晚于**最新源码 07-30 00:57
- **V6** `index-BNQZfcUG.css` / `index-CTgGziQm.js` 各命中 1（[BUILD-002] 防旧构建）
- **toml 三副本** sha256 一致：scene `7b01b33c…`、itn `209ac1e7…`
- **边界合规**：`git status` 仅 `CHANGELOG.md`（+2/−0，任务书允许），源文件与版本号零改动
- **耗时**：主程序 10m02s + Tauri UI 2m51s + npm 1.55s；**CT2 陷阱未触发**（主控预置的 HTTP/1.1 + postBuffer 生效，7 个 third_party 全新 clone 一次成功）

**两处遗留（已通知 tester-1）**：

1. **`result.md` 仍是 0 字节** —— [COLLAB-WRITE-001] 再次复发，汇报只落在 CHANGELOG 与 tmux，已要求补写
2. **冒烟实例 PID 4920 于 14:08:59 退出**（日志为正常 `initiating shutdown`，非崩溃、非 [SMOKE-VANISH-001]）→ 主控已重新启动 **PID 5336**（14:12:45，Responding=True，模型加载正常零 panic）供 Gavin 端测

**⏭ 待 Gavin 端测补证运行时闭环**：说一句话，日志出现 `LLM response meta: finish_reason=stop`、且注入内容不再是「我们分析用户输入…」即 [LLM-COT-LEAK-001] 真正闭环。

---

## （历史）出包派发记录（2026-07-30 12:58 派发 tester-1）

| 项 | 主控决定 | 依据 |
| --- | --- | --- |
| **版本号** | **维持 0.7.2 不动**（任务书列为红线） | Gavin 只说「出包」，未授权升版；「版本号禁止擅改」 |
| **范围** | **全量三步**：npm build + Tauri UI(`--features custom-protocol`) + 主程序 + Publish 同步 | 本批 `src-tauri/src/llm.rs` 有改动，不可如 07-28 跳过 Tauri UI |
| **耗时** | **20+ 分钟全量重编** | 主控取证：`target/release/build`、`target/release/CTranslate2-4.6.0`、`src-tauri/target/release` 三者均不存在 |
| **CT2 陷阱前置** | 已就位（`http.version=HTTP/1.1` + `postBuffer` 全局生效），诊断口诀与处置写入任务书 | [CT2-SUBMODULE-DEADLOCK-001] |
| **决定性验证探针** | `grep -ac "LLM response meta" feiyin-ime.exe` ≥ 1 —— 主控实证**旧 exe 该值为 0**（对照串 `Injecting text`=1 证明方法有效） | 比 mtime 可靠，直接证明 `ff492ef` 进了 exe |

**副作用提醒**：Step 1 会终止 Gavin 正在端测的实例 PID 8592，构建期间语音输入不可用。

**遗留（本次未做）**：升版决策仍悬空 —— 维持 0.7.2 后将存在**第三个**同版本号不同内容的构建（07-27 144词表版 / 07-28 165词表版 / 本次含跨平台+LLM修复版），仅靠 sha256 区分。若 Gavin 更看重可追溯性，可另下指令升 0.7.3 重出包（需再等一次全量重编）。

---

## 📌 出包决策原始记录（2026-07-30 派发前）

**背景**：MACOS-COMPAT-001 + AUDIT-MACOS-BRANCH-001 + FIX-COT-LEAK-001-P0 三批次**代码、测试、文档、git 全部闭环并已 push**（`292eeb0` / `ff492ef` / `2c98976`，`git status -sb` 无 ahead/behind），唯独**未出包，且被 Gavin 明确叫停**。

| 待拍板 | 说明 | 主控建议 |
| --- | --- | --- |
| ① **版本号** | 维持 0.7.2 将产生**第三个**同版本号不同内容的构建，只能靠 sha256 区分 | 建议升 **0.7.3**（版本号是 Gavin 决策权，主控不擅改） |
| ② **构建范围** | 本批 `src-tauri/src/llm.rs` 有改动 → **Tauri UI 必须一并重建**，不可如 07-28 那次跳过 | 三步全量：npm build + Tauri UI(`--features custom-protocol`) + 主程序 |

**⚠️ 耗时提醒**：07-28 磁盘清理已删 release 中间产物，本次为**全量重编**（含 CTranslate2 C++ CMake），预计 20+ 分钟；release 侧可能需再走一遍 [CT2-SUBMODULE-DEADLOCK-001] 处置。

**出包后的端测观察点**：新增的 `LLM response meta:`（finish_reason + usage）日志、DeepSeek 思维链泄漏是否消失（原约每 7 次 1 次注入 `...`）。

---

## 进行中

### 2026-07-30 · macOS 侧交接接管（已完成 checkout + 独立核验，**任务尚未派发**）

> 来源：Windows 侧团队完成 MACOS-COMPAT-001 双平台接缝适配 + 四份交接文档 → Gavin 指令「checkout 最新代码，先做好交接、理清项目，暂不派发任务」
> 治理约束：**DEC-034**（跨平台兼容为首要约束 + 单仓库两端并行）
> 交接文档（均在仓库内、受 git 管辖）：`docs/MACOS-HANDOFF.md`（入职材料，250 行）/ `docs/MACOS-PORT-ASSESSMENT.md` / `docs/BUILD-MACOS.md` / `docs/MACOS-BRANCH-AUDIT.md`（cfg 分支静态审计）

**checkout 结果**：`695e50e` → `2c98976`（fast-forward，4 个提交），`main...origin/main` 已同步。工作区保留 macOS 侧两处真实改动（`.gitignore` +2 / `scripts/build-macos.sh` +11−2）+ 三个未跟踪文件。

**主控独立核验（未采信交接文档结论，逐项查证源码）**：

- ✅ **平台契约属实**：`src/platform/mod.rs:61/71` 两份显式清单各 15 个符号，glob 导出已废除；macOS 侧 15 个符号**逐一 grep 全部存在**，无遗漏
- ✅ **8 项 07-29 实测阻塞项中 5 项已由 Windows 侧修复**：`mod hotkey` / `mod injection` 加 `#[cfg(target_os="windows")]`（消 10 个 E0432）、`src/crash/mod.rs` 补 `get_windows_version()` 非 Windows 占位、`src-tauri/Cargo.toml` 的 `windows` 依赖移入 `[target.'cfg(target_os = "windows")'.dependencies]`、`src-tauri/src/main.rs` 补 `check_hotkey_available` 非 Windows 分支、`src-tauri/src/overlay.rs` 的 `.transparent(true)` 改 `#[cfg(not(target_os="macos"))]`
- ✅ **审计的 P0 属实**：`src/crash/reporter.rs:369` 确为 `egui::FontData::from_bytes(...)` 而 `Cargo.lock` 中 egui = **0.29.1**；同文件 `:347` 的 `include_bytes!("C:/Windows/Fonts/msyh.ttc")` 在 `#[cfg(target_os="windows")]` 块内，macOS 不展开，**不构成第二个 P0**（审计未误报）
- ✅ **审计的依赖版本基线正确**：`Cargo.lock` 同时存在 core-graphics 0.23.2 与 0.25.0、core-foundation 0.9.4 与 0.10.1，但根 crate `Cargo.toml:112-114` 为 macOS 声明的是 `core-graphics = "0.25"` / `core-foundation = "0.10"` / `enigo = "0.2.1"`，与审计核对基线一致（旧版本属其他 crate 的传递依赖）
- ❌ **主控修正 · 审计头条结论低估 2 项**：`docs/MACOS-BRANCH-AUDIT.md` §1/§2/§5 称「**唯一**会阻塞编译的是 `crash/reporter.rs:369`」。**实际剩 3 项**——另两项是 `src/platform/macos/hotkey.rs:124`（`CGEventType` 不支持 `==`）与 `:257`（`create_runloop_source` 返回 `Result` 却调 `.ok_or_else`）。依据：① 这两项是 macOS 侧 07-29 **真实 `cargo check` 实测所得**，已记录在同一仓库的 `docs/BUILD-MACOS.md` §四（审计的参考资料清单里就有这份文档）；② `git log -- src/platform/macos/hotkey.rs` 显示该文件**自初始提交 680d78f 以来从未被改动**，Windows 侧本批次未碰它，故错误必然仍在；③ 审计 §3 表格自己写明 `create_runloop_source` 返回 `Result<CFRunLoopSource, ()>`，却把该调用点列为「签名匹配」——**与调用处的 `.ok_or_else` 自相矛盾**。根因是审计自述的「仅静态阅读 + docs.rs，未在 macOS 上运行 cargo check」，属方法固有局限而非疏忽，但**头条结论的措辞会误导下游按「只剩 1 个错误」排期**
- ⚠️ **工具链未就绪**：`cargo` / `rustc` 不在 PATH（rustup 装在用户目录，需 `source scripts/env-macos.sh`），故本次核验全为静态查证，**未实跑 cargo check**
- ✅ **CT2 构建树完好（好消息）**：`target/debug` 1.6G 存活，`CTranslate2-4.6.0/third_party/` 下 7 个子目录**全部非空**（cpu_features 14 / cutlass 22 / cxxopts 12 / googletest 12 / ruy 11 / spdlog 13 / thrust 17），**未处于 [CT2-SUBMODULE-DEADLOCK-001] 的残缺态**；sherpa-onnx 四个 dylib 已在 `target/debug/`。下次 `cargo check` 不需要重编 CT2

**待派发任务（已理清，等 Gavin 下令）**：

| 编号 | 内容 | 影响文件 | 建议负责人 | 状态 |
| --- | --- | --- | --- | --- |
| MACOS-PR1-SCRIPTS-001 | **Windows 侧明确交接请求**（`docs/MACOS-HANDOFF.md` §4.2）：`setup-macos.sh` + `env-macos.sh` 入库 + `build-macos.sh` 占位替换；派发后追加 C 项（修 `env-macos.sh` 的 zsh 路径失效）与 A 项（`npm install`→`npm ci`） | `scripts/{setup,env,build}-macos.sh` | coder-1 | ✅ **已验收** |
| MACOS-FIX-TAURI-DEPS-001 | 修 `292eeb0` 的依赖段位回归（`tokio-tungstenite`/`futures-util`/`rustls` 被误划 Windows 专属）+ 收掉 `src/main.rs:4349` 既有 test import 错误 | `src-tauri/Cargo.toml` + `src/main.rs` | coder-2 | 🔄 **进行中**（Gavin 2026-07-30 拍板「自己修」，不退回 Windows 侧） |

**PR1 验收记录（2026-07-30 主控独立取证）**：

- **C 项（本任务最有价值的改动）**：`env-macos.sh:11` 改为 `${BASH_SOURCE[0]:-$0}`。**主控独立双 shell 复跑通过**——bash 与 zsh 下 `source` 后 `$SHERPA_ONNX_LIB_DIR` 均指向仓库内实际存在的 lib 目录并能列出 dylib。**另做反向验证**：在 zsh 下手工执行修复前的写法，得到 `/Users/gavinsun/Workspace/CodeLab`（**仓库的父目录**），与诊断完全吻合 → 证明该 bug 真实存在且已被修复。意义：`docs/BUILD-MACOS.md` §一 教给所有新人的 `source scripts/env-macos.sh` 此前在 macOS 默认 shell 下是失效的
- **A 项**：`npm install` → `npm ci`，且按主控裁定实现为**失败时响亮报错 + exit 1 + 指向 MACOS-FIX-NPMLOCK-001，绝不 fallback 到 npm install**，注释中援引 `[NPM-LOCK-CROSSPLAT-001]` 与 DEC-034。同时按 `npm ci` 自带清空 node_modules 的语义简化了原第 61-64 行的手工清理逻辑
- **B 项**：`build-macos.sh` 工作区既有改动（REPO_ROOT / source env / `voice-ime`→`feiyin-ime`）完好保留，未回退
- **三脚本 `bash -n` 语法检查全过；UTF-8 无 BOM**（首三字节均 `23 21 2f`）
- **`ui/package-lock.json` 零改动**（主控独立 `git diff --stat` 确认为空）——跨平台红线守住
- **边界合规**：`git status` 中 `scripts/` 外无本任务改动
- **过程亮点**：coder-1 在撞上 `npm ci` EUSAGE 后**没有自行降级为 `npm install` 交差，而是停下来上报三个选项请主控裁定**，符合 worker-guide §七「主动沟通比默默出错代价更低」

**衍生立项 · MACOS-FIX-NPMLOCK-001（待 Gavin 决策）**：`ui/package-lock.json` 与 `package.json` 长期失同步（`npm ci` 报 EUSAGE：`@emnapi/core@1.11.3`、`@emnapi/runtime@1.11.3` 缺失，`@emnapi/wasi-threads` 锁定 1.2.2 不满足 1.2.3），均为传递依赖；`git log -- ui/package-lock.json` 仅两次提交（`680d78f` 初始 + `f10c1e0` v0.6.2），即**该缺陷长期存在于两个平台**。**后果**：`docs/MACOS-HANDOFF.md` §6.1 提出的「两侧统一用 npm ci」约定当前在**任何平台都无法执行**，Windows 侧同样会 EUSAGE。**限度**：只卡全新 clone，现有 `node_modules` 完好、`npm run build` 正常。**难点**：修 lock 需在某一平台跑 `npm install`，而这正是 `[NPM-LOCK-CROSSPLAT-001]` 警告的动作，需决定由哪侧执行、是否用 `--package-lock-only`、以及如何验证两平台都能 `npm ci`。**建议与 Windows 侧协同处理**（属共享文件，单侧修完对侧仍可能失败）。
| MACOS-CARGOCHECK-BASELINE-001 | 拿真实错误清单：`source scripts/env-macos.sh` 后跑三处 `cargo check`（主程序 / `src-tauri` / crash-reporter bin），产出完整错误清单。**这是 DEC-033 §执行前提与评估报告 §11 共同指定的第一步** | — | tester-1 | ✅ **已验收** |
| MACOS-FIX-COMPILE-001 | 修 3 项编译阻塞：`crash/reporter.rs:369` → `FontData::from_owned`（连带删 `.ok().unwrap_or_default()`）；`hotkey.rs:124` → `matches!`；`:257` → `.map_err` | `src/crash/reporter.rs` + `src/platform/macos/hotkey.rs` | coder-2 | ✅ **已验收** |

**BASELINE 验收记录（2026-07-30 主控独立取证）**：

- **质量高，逐条给了 rustc 原始输出**（含 note/help），未压缩成结论。三处预测错误**全部实测复现且行号一字不差**：E0599 `reporter.rs:369`（rustc 直接提示只有 `from_static`/`from_owned`）、E0369 `hotkey.rs:124`（`CGEventType does not implement PartialEq`）、E0599 `hotkey.rs:257`
- **✅ 校准了审计头条结论**：`docs/MACOS-BRANCH-AUDIT.md` 称「唯一阻塞是 `reporter.rs:369`」，实测主程序 **4 个独特错误**、src-tauri **7 个**。主控派发前的预判（低估 2 项）得到实证
- **🔴 最大发现：查出 Windows 侧本次改动引入的真实回归**（主控已独立复核 `git show 292eeb0 -- src-tauri/Cargo.toml` 确认）。Windows 侧把 `[target.'cfg(target_os = "windows")'.dependencies]` 段头**插在了 `[dependencies]` 表中间**，按 TOML 语义，排在其后的 `tokio-tungstenite` / `futures-util` / `rustls` **三个依赖被静默划为 Windows 专属** → macOS 上 `qwen3.rs` + `main.rs:170` 共 7 个错误。**在 Windows 上 cfg 命中、三者照常解析、`cargo check` 0 errors —— 该回归在 Windows 侧完全不可见**，其提交信息里「src-tauri 0 errors」为真却拦不住它。**且不止编译问题**：`rustls` 那行是 BUG-QWEN3-CRYPTO-001 的 ring provider 修复，被划成 Windows-only 等于 macOS 侧丢了这个 TLS 修复
- **tester-1 的根因诊断正确且指出了审计的方法盲区**：审计方法是扫源码 `#[cfg]` 分支，而本次漂移发生在**依赖清单层**，源码里没有 cfg 可扫。这是 DEC-034 机制的新变种（「依赖层 cfg 与代码层 cfg 同步缺口」），已记入 troubleshooting
- **另查出既有问题（非本次引入，主控查 git log 确认）**：`src/main.rs:4349` 的 `#[cfg(test)]` 块无条件 import 了被 `cfg(windows)` 门控的 `select_preprocessing_params`（E0432），只影响 macOS 上 `cargo test`，不阻塞 release 构建
- **红线遵守确认**：零文件改动（`git status --porcelain` 与开工时一致）、未出包、未 `npm install`（`package-lock.json` 零改动）、未 `cargo clean`、未用 git 破坏性命令
- **诚信记录（正面）**：主动披露了 `env-macos.sh` 的 `BASH_SOURCE` bug 与自己的手工绕过方式，没有把「手工设了变量」隐去当成脚本可用

**FIX-COMPILE 验收记录（2026-07-30 主控独立取证）**：

- **三处改动 Read 逐一核对，全部正确**：`from_owned(font_data)` 且**连带的 `.ok().unwrap_or_default()` 已删除**（这是最易漏的一处，Gavin 特别确认过）；`matches!(event_type, CGEventType::KeyDown)`；`.map_err(|_| anyhow!(...))`
- **主控独立复跑取证（未采信汇报）**：`cargo check` **0 errors** 属实；`cargo check --all-targets` 只剩 `main.rs:4349` 那一个既有 E0432，**未引入任何新错误**
- **Windows 侧零影响已验证**：`reporter.rs:347` 的 `include_bytes!("C:/Windows/Fonts/msyh.ttc")` 仍在原处，`git diff` 中 `msyh.ttc` 命中数 **0** —— DEC-034 与 DEC-033 第 4 条硬约束遵守
- **⚠️ 一处越界（判定为可接受，不回滚）**：`src/llm/mod.rs` 出现 +3/−2 改动，位于 `mod tests`（起于 `:1473`）内，系 `build_optimize_request` 调用由两行压成一行。按 `[FMT-COLLATERAL-001]` 三步法定性：**去空白后 md5 逐字节相同**（`1e3970f6…`），零逻辑改动，属 `cargo fmt` 全量连带，保留不回滚
- **⚠️ 但任务书要求的是 `cargo fmt -- <两个文件>`，仍出现全量连带** —— 这是同一指令第二次未被遵守（上一次见 07-27 TEST-SYNC 验收记录）。后续派发需改为更强的措辞或改由主控自行 fmt
- **CHANGELOG.md +2**：两项任务的完成记录，属 worker-guide 规定的完成联动流程，非越界

**边界评估（已做，供派发时直接用）**：三项任务文件级零重叠——PR1 只碰 `scripts/`，BASELINE 零改动，FIX 只碰 `src/crash/reporter.rs` + `src/platform/macos/hotkey.rs`。PR1 与 BASELINE 可并行，唯一注意点是 PR1 不得在 BASELINE 运行期间改动 `env-macos.sh` 内容（tester-1 要 source 它）。

**已知非阻塞遗留**：`src-tauri/gen/schemas/macOS-schema.json` 为 Tauri 生成物、当前未跟踪且未被 `.gitignore` 覆盖，需判断是入库还是加 ignore。


> **2026-07-30 主控取证结论：本节原列的三个批次已全部闭环，无进行中的代码任务。** 三个 Worker（coder-1 / coder-2 / tester-1）当前空闲待派发。历史验收记录保留在下方备查。

### 2026-07-29 · MACOS-COMPAT-001 双平台兼容适配（A 阶段实施，Gavin 指令派发）

> 依据：**DEC-033**（本日新增）｜ 前置研究：RESEARCH-MACOS-DUALPLATFORM-001（已验收）
> **最高红线（Gavin 两次重申）**：「代码重构，不得影响任何 windows 的代码功能」——列为两份任务书的第一验收标准
> 分工边界（DEC-033）：本侧负责共享代码 + Windows 专用代码 + **接缝（平台契约）**；macOS 专用功能由 macOS 侧 Agent 团队后续开发

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| MACOS-COMPAT-001-CORE | 主程序侧接缝适配四项：① `mod hotkey`/`mod injection` 加 `#[cfg(windows)]`（**全仓零引用，实证**）② `crash/mod.rs:99` `get_windows_version` cfg 隔离 + 非 Win 等签名版本（**调用点 `:74` 一行不改**）③ **`platform/mod.rs` glob 导出改显式清单 + 契约注释块**（核心）④ macOS 侧补 `notify_config_changed` / `capture_scene_signals` stub（带 `TODO(macOS team)`） | `src/main.rs` + `src/crash/mod.rs` + `src/platform/**` | coder-1 | 🔄 已派发（已 ACK，核读源码中） |
| MACOS-COMPAT-001-TAURI | Tauri 侧 cfg 隔离**三项**：`src-tauri/Cargo.toml` windows 依赖挪 target 段 ｜ `check_hotkey_available` cfg 隔离 ｜ `overlay.rs:39` `.transparent(true)` **cfg 拆链**（不启用 `macos-private-api`，避免动共享 `tauri.conf.json`）。**+ sherpa-onnx 获取脚本**（Windows 侧，见下方重新定位） | `src-tauri/**` + `scripts/**` | coder-2 | 🔄 进行中（**范围已变更，见下**） |

**⚠️ 范围变更（2026-07-29 Gavin 指令「目前暂未考虑使用 github 的 CI/CD，还是用本地平台构建发布」→ DEC-033 附则二）**：

- **取消 B-1**（解除 `.gitignore` 的 `.github/` 排除）→ `.gitignore` 不动
- **取消 B-2**（重写双平台 CI workflow）→ `.github/` 一字不改，现有陈旧 workflow 保持未入库
- **B-3 保留但重新定位**：sherpa-onnx 获取脚本继续做，目的从「喂 CI」改为「**解决全新 checkout 构建不了的既有问题**」——这是 macOS 团队能否起步的前置条件，与 CI 无关。按「给新开发者用的一键获取脚本」设计（PowerShell，UTF-8 **with BOM**）
- 已通过 tmux 实时通知 coder-2，要求若已动 `.gitignore`/`.github/` 则用 edit 工具还原（**禁止 git checkout--/restore**）

**主控已向 Gavin 声明的风险（决策以 Gavin 为准，此处仅存档）**：无 CI 状态下「Windows 改动破坏 macOS」与「macOS 改动破坏 Windows」**都不会在提交时暴露**（`#[cfg]` 切掉的代码编译器不做类型检查，trait 亦无法约束）。替代防线只剩：显式导出清单 → 契约注释 → **交接纪律** → 两侧各自本地 `cargo check`（防不住对侧，属固有缺口）。因此 `docs/MACOS-HANDOFF.md` 的「改导出面必须同步更新两份清单」条款，从"建议"升级为**唯一可执行的防线**，权重显著提高。

**边界评估**：两任务文件级**零重叠**（`src/**` vs `src-tauri/**`+CI），可并行。与已冻结的 FIX-COT-LEAK-001-P0（`src/llm/mod.rs` + `src-tauri/src/llm.rs`）亦零重叠。

### 交付路径（Gavin 2026-07-29：「等你这边代码重构完毕，提交后，我让 macOS 那边团队 checkout 开始接收开发」）

**「重构完毕」的验收口径 = 对方 checkout 后能真正接手，不是我们本机编译过就算。** 五步串行：

| 步 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| 1 | MACOS-COMPAT-001-CORE + TAURI 代码完成 | coder-1 / coder-2 | ✅ **已交付** |
| 2 | 主控验收：Read 实际改动逐条核 + **编译实证** | orchestrator | ✅ **已通过**（详见下方验收记录） |
| 3 | **TEST-SYNC + TEST-EXEC 全量回归**——本批次红线是「Windows 零行为改动」 | tester-1 | ✅ **已验收**（TEST-SYNC 评估无需新增；主程序 686/0/8 + Tauri 53/0/0；主控 `cargo test -- --list` 独立计数 694 自洽） |
| 4 | **交接文档** `docs/MACOS-HANDOFF.md`（242 行，六块内容） | coder-1 | ✅ **已验收**（主控修正两处引用错误：导出清单实际在 `:61`；`build-macos.sh` 已入库） |
| 5 | git commit + push | orchestrator | ✅ **已完成**（`292eeb0`，14 文件 +730/−26 → `3882236..292eeb0`；零 token 残留） |

**后续增补（2026-07-30）**：AUDIT-MACOS-BRANCH-001（coder-1，纯审计零改动）→ `docs/MACOS-BRANCH-AUDIT.md` + `collab/research/macos-branch-audit-001.md`，15 处 cfg 分支 / **P0×1**（`src/crash/reporter.rs:369` 调用 `egui 0.29.1` 不存在的 `FontData::from_bytes`，macOS 必然编译失败，修复方向 `from_owned`）/ P1×8（含主控此前未知的 `main.rs:3419-3475 mod macos_stubs` 空实现）/ P2×4 / P3×2。已提交 `2c98976`，交由 macOS 团队据此做 Phase 3 实现。

**主控验收记录（2026-07-29~30，步 2）**：

- **编译实证（本批次最关键，Worker 侧均未能自证）**：主控独立跑通
  - `cargo check` → **0 errors**，4m43s，86 warnings 且**无一条指向 `src/platform/**` 或 `src/crash/**`**（main.rs 的 warning 集中在 1191 行 Win32 GDI 既有问题，远离改动的第 5-12 行）
  - `cargo check --manifest-path src-tauri/Cargo.toml` → **0 errors**，11 个既有 warning（全在 wordbook 未使用函数）
  - 意义：`platform/mod.rs` 的 15 符号显式导出清单**无遗漏**——这是本批次唯一靠编译器兜底的改动，漏一个符号即编译失败
- **coder-1 四项 Read 核验通过**：`mod hotkey`/`mod injection` 仅加 cfg 未删除（符合要求）；`get_windows_version` 拆两版**调用点 `:74` 确未改动**；契约注释块五要素齐全且含主控给的「名称+arity 相同、类型平台化」stub 原则；两个 stub 均带 `TODO(macOS team)` 且注明 `usize` 语义留给 macOS 团队定
- **coder-2 A 组三项通过**：`.gitignore` 与 `.github/` 经 `git diff` 确认**一字未动**（附则二合规）；`overlay.rs` 采用 cfg 拆链，Windows 路径调用序列不变
- **⚠️ 一处越界（已核为无害，保留不回滚）**：coder-2 对 `src-tauri` 整个 crate 跑了 `cargo fmt`（任务书要求只格式化改动文件），连带改动 `config.rs`(+1/−4) / `qwen3.rs`(+5/−4) / `version_check.rs`(+3/−10)。**主控逐 hunk 读完全部 diff，numstat 与 hunk 数完全对应、无遗漏**，全部为 rustfmt 重排（闭包体加花括号、去尾逗号、长行合并）。注：去空白 md5 比对不一致是因 rustfmt 增删了 `{}` 与 `,` 这类 token，非逻辑改动；按 [FMT-COLLATERAL-001] 惯例保留
- **环境阻塞已排除**：两个 Worker 均被 [CT2-SUBMODULE-DEADLOCK-001] 阻塞，主控定位根因并修复（详见 troubleshooting）。**coder-1 的"网络问题"归因错误**，其提出的两个方案（跳过验证 / 等网络恢复）均无法解决问题——该构建树属永不自愈状态

**⚠️ 必须先让 Gavin 知悉的交接缺口**：即便本批次全部完成，**macOS 团队 checkout 后仍然构建不了**，原因是本仓库既有的两个基础设施问题（非本批次引入）：

1. **sherpa-onnx 预编译库未入库**（`.gitignore:22` 排除，`git ls-files` 实证为 0）
2. **`.cargo/config.toml` 把 `SHERPA_ONNX_LIB_DIR` 硬编码为本机 Windows 绝对路径**，且按最高红线不得修改

**解法（不构成死锁）**：BUILD-MACOS.md §一 表明 macOS 侧同事本机**已写好** `scripts/setup-macos.sh` + `scripts/env-macos.sh`（前者自动拉取 osx-arm64 预编译包，后者用 `[env]` 的 `force=false` 语义 export 覆盖）。**这两个脚本应作为 macOS 团队的第一个 PR 提交** —— 需在 `MACOS-HANDOFF.md` 中显式写明这一交接约定，否则对方会以为环境已就绪。我方对应提供 Windows 侧获取脚本（coder-2 任务 B-3）。

**主控核心设计决策（写入任务书）**：

- **平台相关类型差异刻意不统一**（`create_controller_window` 返回 `Result<HWND>` vs `Result<()>`、`FocusedTextSnapshot.hwnd` 为 `HWND` vs `usize`）。统一需改 `src/injection/mod.rs` 与 `platform/windows/*` 等 Windows 已交付路径，违反最高红线。改为在契约注释中显式标注 + 调用方必须在 cfg 分支内使用 + CI 兜底
- **不用 trait 做抽象**：trait 只约束当前编译目标上的实现，防不住被 cfg 切掉那一侧的漂移（✅官方 Rust Reference：cfg 为假的项从 AST 移除，不做类型检查）。**glob 改显式清单**的价值在于——漏列会**立即响亮编译失败**，而非静默行为漂移
- **Windows CI job 是保护我们自己的门**：DEC-033 下 macOS 团队会改共享代码，没有 Windows job 就没有反向防护。故它必需且必须能真绿

**⚠️ 派发前主控新发现的基础设施缺口（决定 CI 能否绿）**：

**sherpa-onnx 预编译库根本没入库** —— `git check-ignore -v` 实证 `.gitignore:22` 的 `/vendor/sherpa-onnx/*-Release/` 排除了它，`git ls-files sherpa-onnx-lib/` 为 **0**，`git ls-files vendor/` 只有 16 个文件且全属 `vendor/cmake/` crate 源码。**含义：任何平台的全新 checkout 都构建不了**，不只是 macOS——本机能构建纯粹因为磁盘上有那份未入库目录。叠加 `.cargo/config.toml` 把 `SHERPA_ONNX_LIB_DIR` 硬编码到 `D:\Workspace\...`，CI runner 上 `sherpa-onnx-sys` build.rs 必然 panic（BUILD-MACOS.md §三 记录了 macOS 侧遇到的同一 panic）。

**这条同时修正了 coder-2 报告 §4.3「Windows .dll/.lib 已入库」的错误结论**（验收记录第四处修正）。可用杠杆：cargo `[env]` 默认 `force=false`，CI 里 export 真实路径即可覆盖，**无需改 `.cargo/config.toml`**。已授权 coder-2 在无法可靠完成时**如实降级**（两 job 均先 `continue-on-error` + 拆出独立后续任务），明确"如实降级 > 硬凑跑不通的 CI"。

### 2026-07-29 · RESEARCH-MACOS-DUALPLATFORM-001 双平台单仓库可行性评估（Gavin 指令）

> 来源：Gavin 请 macOS 侧开发人员出具两份评估报告 → 要求评估「不影响 Windows 任何功能的前提下重构，实现一套代码 + 一个 GitHub 仓库 + 双平台并行开发」
> 输入文档：`docs/MACOS-PORT-ASSESSMENT.md`（195 行）、`docs/BUILD-MACOS.md`（120 行），均为 untracked

**主控已核实的仓库事实（派发时写入任务书）**：

1. **macOS 侧环境改造全部滞留对方本机**：`scripts/setup-macos.sh` / `scripts/env-macos.sh` 本仓库**不存在**（BUILD-MACOS.md §一 却要求执行它们）；`scripts/build-macos.sh` 是 2026-04-19 的 394 字节占位；§六 声称已追加的 `.gitignore` 项 `vendor/sherpa-onnx/*-shared-lib` 本仓库**没有**
2. 本地 == `origin/main` == `3882236`，远端仅 main 一个分支，对方零提交
3. 报告头部引用「对应决策 `collab/decisions.md` DEC-033」，但本仓库 **decisions.md 只到 DEC-032，DEC-033 不存在**
4. 主控复核属实的三条：`.gitignore:80` 确有 `.github/`（CI 从未进仓库）｜`.cargo/config.toml` 仅 3 行且 `[env]` 硬编码 Windows 绝对路径｜`src/platform/mod.rs` 确为 glob 导出无 trait，`create_hotkey_listener` 两侧签名不同
5. `src/main.rs:8,10` 的 `mod hotkey;` / `mod injection;` 确无 cfg 隔离

**报告核心数据**（待 coder-2 抽查复核）：构建环境已打通（324 crate 含 CTranslate2/sherpa-onnx 全在 arm64 编过）；剩 20 个源码错误（主程序 16 / Tauri 3 / crash-reporter 1）；`src/` 24,504 行中约 70% 天然跨平台；633 个 `#[test]` 绝大部分可直接在 macOS 跑；**平台层签名漂移已实际发生 6 处**（从未触发编译错误，因两侧从未同时编译）

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| RESEARCH-MACOS-DUALPLATFORM-001 | **纯研究零改动**：报告可信度抽查复核 ｜ 改动按「Windows 零影响 / 行为等价 / 有回归风险」三档分级 ｜ 签名漂移的真正防线（trait 能否解决 vs 双平台 CI）｜ 单仓库工程约定（package-lock 冲突 / vendor 二进制 / 分支策略）｜ 最小可提交批次与 GO-NO-GO 结论 | 无（产出 `collab/research/macos-dualplatform-refactor-001.md`，370 行） | coder-2 | ✅ **已验收（含主控三处修正，其中一处推翻核心成本结论）** |

**RESEARCH-MACOS-DUALPLATFORM-001 验收记录（2026-07-29 主控独立取证）**：

- **边界合格**：`git status` 仅两份 macOS 报告 untracked，源文件/`.gitignore`/`.cargo/config.toml` 零改动；`result.md` 1774 字节非空
- **✅ 主控复核属实的部分**：§2 五项 P0 阻断、§7 六行签名漂移、`src/` 24504 行 / `main.rs` 4416 行、`accessibility.rs:43-48` 确为 stub、`.github/workflows/build-macos.yml` 确实存在（816 B，2026-04-19）但被 `.gitignore:80` 排除从未进 git
- **❌ 主控修正一 · CI 成本结论基于错误前提（最重要）**：报告 §3.4 全篇按**私有仓库**计费——「macOS $0.062/分钟、约 Linux 10.3 倍、GitHub Free 2000 分钟/月按倍率折算」，并据此设计了「若 CI 成本不可接受的退而求其次方案」四选项。**但 `Cdexs/Feiyin-IME` 是公开仓库**——主控实测 `api.github.com/repos/Cdexs/Feiyin-IME` 返回 `"private": false` / `"visibility": "public"`（HTTP 200 免鉴权可读即为佐证）。**标准 GitHub-hosted runner（含 macOS）对公开仓库免费且不限量**（✅官方：billing 文档「The use of standard GitHub-hosted runners is free: In public repositories」；仅 `-large`/`-xlarge` 大型 runner 收费）。**结论反转**：成本不构成约束，"退而求其次"整节不适用，可直接上 push + PR 双触发，甚至跑完整 `cargo test`。真实约束只剩**单 job 6 小时上限**与 **cache 10GB 上限**
- **❌ 主控修正二 · 根 `node_modules` 的 gitignore 判断错误**：报告 §4.2 称「`.gitignore` 未显式忽略根 `node_modules/`」。实际 `.gitignore:12` 有**裸 `node_modules` 模式**，git 在任意层级匹配同名目录。主控实证 `git check-ignore -v node_modules` → `.gitignore:12:node_modules	node_modules`。故建议里「追加 `/node_modules/` 防误提交」是**冗余项**（"删除根 node_modules 历史遗留"的建议本身仍成立）
- **❌ 主控修正三 · `#[test]` 计数「差异 2」是自身 grep 口径问题**：报告用 `^\s*#\[test\]` 得 631 并标 ⚠️ 质疑原报告的 633。主控用 `grep -rn "#\[test\]" src/` 得 **633**，与原报告**完全一致**。原报告此项无误，不应标 ⚠️
- **一处核验不完整（不阻塞）**：§1.2 第 2 行 `read_text_from_hwnd` 报告自承「`src/platform/macos/injection.rs` 需要单独确认」，但仍标了 ✅属实。属证据略超前于结论，结论方向不影响
- **Q3 两个核心判断经官方文档验证成立**：cargo `[env]` 不支持 per-target（✅官方 Cargo Book）；`force=false` 时 shell 已有变量优先（✅官方）；`#[cfg]` 切掉的代码从 AST 移除、不做类型检查（✅官方 Rust Reference）→ **trait 不能防签名漂移，双平台 CI 是唯一可靠防线**，主控判断被确认

**主控预判的三个难点**（写入任务书供 Worker 验证或推翻）：

- **`.cargo/config.toml` 的 `[env]` 是唯一必须触碰 Windows 构建路径的改动**。cargo 的 `[env]` 不支持 per-target 区分，需在「移除改走自动下载 / build.rs / Windows 侧也改 env 脚本」间取舍，每个候选都要给 Windows 侧验证方法
- **trait 抽象不能防签名漂移**：cfg 切掉的代码编译器不做类型检查，trait 只约束被编译的那一侧。若此判断成立，**双平台 CI 是唯一可靠防线** → `.github/` 必须先解除 gitignore。需量化 macOS runner 上 CTranslate2 源码编译耗时与 Actions 额度
- **overlay trait 抽象（报告 §10-1）属高风险档**，为未交付平台重写已交付的 Windows GDI 关键路径，建议本轮不做

### 2026-07-29 · LLM-COT-LEAK-001 思维链泄漏致输出异常（Gavin 端测发现）

> 来源：Gavin 2026-07-29 `-debug` 端测反馈「语音输入后输出异常，疑似 LLM 优化后有问题」→ 主控日志取证分析
> 诊断记录：`troubleshooting.md [LLM-COT-LEAK-001]`、`logs/20260729.md`
> 证据：`target/release/debug.log` 12:06:27 与 13:19:15 两次异常

**根因链（主控独立取证）**：

1. **上游触发**：Gavin 于 11:13 把模型从 `Qwen/Qwen3.5-35B-A3B` 换为 `deepseek-v4-flash`（endpoint `api.deepseek.com`）。该模型把思维链写进 `content`。**Qwen 88 次请求 0 异常，v4-flash 15 次请求 2 异常（13%）**
2. **`enable_thinking:false` 对 DeepSeek 端点无效**（`llm/mod.rs:286/369/521`）：该参数属 SiliconFlow/Qwen 系，DeepSeek 官方 API 不识别，静默忽略 → DEC-008「关推理模式」在此 endpoint 上从未生效
3. **疑似 `max_tokens=512` 被思维链吃光**，真答案无预算输出（两次异常耗时 6.4s/6.6s vs 正常 3.6s）。**此条为强推断，尚未证实**——见第 5 条
4. **`extract_corrected_tag`（`llm/mod.rs:1064-1079`）取首个标签对**（`text.find`），思维链里的模板占位 `<corrected>...</corrected>` 劫持了真答案 → 注入 `"..."`（13:19 案例，整句全丢）
5. **FMT-EMPTY-CORRECTED-001 护栏漏网**（`llm/mod.rs:213-225`）：只挡「结果为空」与「残留字面量标签」两种，`"..."` 两者都不沾。12:06 之所以被挡住纯属偶然——那段思维链无闭合标签对，走兜底分支残留了字面量标签才触发护栏
6. **可观测性缺口**：`ChatResponse`（`llm/mod.rs:33` 附近）只解析 `choices`，**无 `finish_reason` / `usage`** → 截断与否无法从日志判定

**两次异常的实际表现**：

| 时间 | 输入 | 注入结果 | 护栏 |
| --- | --- | --- | --- |
| 12:06:27 | 我以的生命之书阿卡西记录真的存在吗 | 本地标点兜底（LLM 优化失效但原文完整） | ✅ 挡住 |
| 13:19:15 | gpu又分为八核和十核…最高可达35% | **`...`（整句全丢）** | ❌ 漏网 |

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| RESEARCH-DEEPSEEK-THINKING-001 | **纯研究零改动**：查证 deepseek-v4-flash 身份、DeepSeek 官方是否支持关闭思维链及参数名、CoT 走 `content` 还是 `reasoning_content`、max_tokens 是否含 CoT token、finish_reason/usage 字段契约、未知参数处理策略；产出落地建议 | 无（产出 `collab/research/deepseek-thinking-control-001.md`，504 行） | coder-1 | ✅ **已验收（推翻主控一处判断 + 主控回补一处）** |

**RESEARCH-DEEPSEEK-THINKING-001 验收记录（2026-07-29 主控独立取证）**：

- **边界合格**：`git status` 仅两份 macOS 报告 untracked，源文件零改动；`result.md` 3006 字节非空（[COLLAB-WRITE-001] 未复发）
- **API key 零泄漏**：主控取 config.toml 中真实 key 反向 grep `collab/research/` `collab/outbox/` `handoffs.md` `logs/`，零命中
- **✅ coder-1 推翻主控判断一处（正确，主控采纳）**：主控原判「模型把思维链写进 `content`」**不成立**。DeepSeek CoT 走**独立字段 `reasoning_content`**（官方文档 + 实测双证）。真实链路是——`max_tokens=512` 被 CoT 吃光 → `content=""` → **我们自己的 `extract_text`（`src/llm/mod.rs:962-993`）在 content 空时错误回落到 `reasoning_content`**，把 CoT 当答案返回。**这是我们自己的 bug，不是模型的**
- **✅ coder-1 补出主控漏掉的第 4 处注入点**：`src-tauri/src/llm.rs:23/86` 同样发 `enable_thinking: Some(false)`，主控任务书只列了主程序三处。主控复核属实
- **❌ 主控回补 coder-1 一处推测错误**：报告 Q3（第 152 行）推测 CoT「很可能被 `flatten_multiline` 或 `strip_fabricated_*` 压成 `...`」——**不成立**。主控用日志定论：13:19 那次 `suggestions after_tag` 日志**存在**（`debug.log:4416`），而该日志只在 `parse_suggestions_after_corrected_tag` 内打印，该函数只在 `extract_corrected_tag` 返回 `Some` 的分支被调用（`mod.rs:1005-1007`）→ **证明 `extract_corrected_tag` 成功返回了 `"..."`**。真正的第 4 环是它用 `text.find` 取**首个**标签对，抓走了 CoT 里的模板占位 `<corrected>...</corrected>`，与后处理层无关
- **两次异常的分支差异已由日志定论**：12:06 **无** after_tag 日志（`debug.log:4226-4228`）→ `extract_corrected_tag` 返回 `None` → 走无标签兜底分支 → 整段 CoT 含字面量标签 → FMT-EMPTY-CORRECTED-001 护栏拦下；13:19 **有** after_tag 日志 → 取到占位 `"..."` → 护栏三项判据全不沾 → 注入。**同一根因链，只因 CoT 里有没有闭合标签对而结局不同**
- **✅官方标注质量合格**（上一轮的返工点未复发）：Q1/Q2/Q3/Q4/Q5 的 ✅官方 条目均附 URL + 原文引用；7 次实测均附脱敏命令与响应关键字段；`content_filter` 实测未触发、Anthropic 兼容层冲突两项**主动降级为 ⚠️/❌未证实** 并列入附录 C

**合并后的完整根因链（主控定论，五环）**：

1. `enable_thinking: false` 对 DeepSeek 无效（✅实测静默忽略）→ 思维链默认开启
2. CoT 与答案共享 `max_tokens=512` 预算，CoT 优先消耗（✅实测 TEST 4/13）→ `content=""`，`finish_reason="length"`
3. `extract_text`（`mod.rs:962-993`）content 空时回落 `reasoning_content` → **CoT 被当成答案**
4. `extract_corrected_tag`（`mod.rs:1064-1079`）用 `text.find` 取**首对**标签 → 抓到 CoT 里的模板占位 → `"..."`
5. FMT-EMPTY-CORRECTED-001 护栏（`mod.rs:213-225`）只挡「空」与「残留字面量标签」→ `"..."` 漏网 → 注入用户输入框
| FIX-COT-LEAK-001-P0 | 代码加固**五项**（研究结论落地，对应根因链五环）：**P0-1** 请求体双发字段——保留 `enable_thinking`（SiliconFlow/Qwen3 用）+ 新增 `thinking:{"type":"disabled"}`（DeepSeek 官方参数），**4 处**注入点 `src/llm/mod.rs:286/369/521` + `src-tauri/src/llm.rs:86` ｜ **P0-2** `extract_text` 移除「content 空回落 `reasoning_content`」逻辑（字段保留解析，仅供日志）｜ **P0-3** `extract_corrected_tag` 改取**最后一对**标签（`rfind`）｜ **P0-4** 护栏补合理性校验（结果相对输入异常萎缩判格式失败，**须防误伤 F1 去语气词等合法压缩**）｜ **P0-5** `ChatResponse` 补 `finish_reason` + `usage`（含 `completion_tokens_details.reasoning_tokens`）并打日志 | `src/llm/mod.rs` + `src-tauri/src/llm.rs` | coder-1 | ✅ **已完成并已提交 `ff492ef`**（2026-07-30 解冻后实施；含主控两处修正 + 判据 B 设计反转为「只观测不拒绝」；695/0/8 + Tauri 53/0/0）｜**⚠️ 未出包 → 旧 exe 仍 100% 复现，见下** |

**✅ 已实施（2026-07-30，Gavin 解冻后指令实施）**：五项全部落地，详见 `logs/20260730.md`，提交 `ff492ef`（2 文件 +326/−29）。

**🔴 但缺陷在端测中仍 100% 复现 —— 因为未出包（2026-07-30 12:40/12:41 主控日志实证）**：

- 运行实例 PID 8592 用的是 **07-28 18:42 的旧 exe**，不含 `ff492ef`
- 两次听写（12:40:23 / 12:41:22）的注入内容均为 DeepSeek 思维链整段（1946 / 1927 字符，「我们分析用户输入：…」），与 [LLM-COT-LEAK-001] 根因链完全一致
- **本次两例走的是「有闭合标签对」以外的第三种形态**：`extract_text` 回落 `reasoning_content` 后 CoT 直接成为 optimize 结果，护栏三项判据全不沾（非空、无字面量标签、非纯标点）
- **结论：这不是新 bug，是已修复但未交付。唯一解法是出包**（版本号与范围两项待 Gavin 拍板，见文档顶部）

**⚠️ 冻结期间的现实影响（Gavin 已知悉）**：当前 LLM 配置为 `deepseek-v4-flash`，实测约每 7 次请求有 1 次触发该缺陷——白等约 6.4s 后回退本地标点（LLM 优化失效但原文完整），其中少数情况会向输入框注入 `...` 致整句丢失。**规避方式**：若冻结期较长，可在设置里把模型换回 `Qwen/Qwen3.5-35B-A3B`（同日志内 88 次请求零异常），属纯配置改动、零代码风险。

**解冻后的实施要点**：五项对应根因链五环，缺一不可：

- **P0-1 治本**（关掉思维链，DeepSeek 延迟 6.4s → 3.6s，token 费用同降）。**双发是为零副作用**——若只发 `thinking` 而删掉 `enable_thinking`，SiliconFlow/Qwen3 用户的思维链会重新开启，属回归；实测证明 DeepSeek 对未知字段静默忽略（不 400），双发安全
- **P0-2/P0-3/P0-4 是防御纵深**，与模型选择无关。即便换回 Qwen，这三处缺陷仍潜伏——任何模型只要 `content` 为空、或在输出前复述一遍带占位的模板，就会复现
- **P0-5 补可观测性**，12:06 那类「LLM 优化失效」今后可在日志直接看出是 `length` 截断 / 超时 / `content_filter`

**⚠️ 报告附录 C 的未证实项（实施时必须评估）**：用户若填 **Anthropic 兼容 endpoint**，双发的 `thinking` 字段可能与 Anthropic 自有 `thinking` 参数语义冲突（❌未证实）。实施前需决定是否加 guard。

### 2026-07-28 · RESEARCH-SCENE-COVERAGE-001 场景词表扩展研究（方案已回，**待 Gavin 拍板三项**）

| 编号 | 内容 | 产出 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| RESEARCH-SCENE-COVERAGE-001 | 场景感知软件词表与分类体系扩展研究（纯研究，零文件改动） | `collab/research/scene-rules-expansion-001.md`（397 行） | coder-1 | ✅ 已验收（**含主控三处实质修正**） |

**主控独立核验结果**：

- **边界合格**：`git status` 零改动，`scene-rules.toml` / Rust 源文件全未碰；result.md 4300 字节非空（[COLLAB-WRITE-001] 未复发）
- **✅实测层可信**：抽查 6/6 全部属实——Obsidian 目录确无 `Obsidian-helper.exe`、`D:\xshell\Xagent.exe` 存在、`MarkText.exe` / `Koodo Reader.exe` 路径正确、`wezterm.exe` 确在 `%LocalAppData%\Programs\WezTerm\`（主控首轮搜索因未覆盖该根目录误判为不存在，二次广域搜索证实 coder-1 无误）、四个 AppxManifest 的 Executable 字段逐一复读一致（OneCalendar→`CalendarApp.Gui.Win10.exe`、OutlookForWindows→`olk.exe`、MSTeams→`ms-teams.exe`、Claude→`app\Claude.exe`）
- **❌ 主控修正一：10 条 title_keywords 是 no-op**。报告建议把「微博/小红书/知乎/Jira/TAPD/禅道/Linear/Teambition/Salesforce/Zendesk」加进 **browser 块**的 title_keywords，目的是「浏览器场景细分时命中」。但 `src/scene/mod.rs:162-164` 浏览器细分循环里有 `if other_rule.kind == SceneKind::Browser { continue; }` —— **显式跳过浏览器自身的 title_keywords**。浏览器 exe 在优先级 1 命中后直接返回 Browser，这 10 条永远不会生效（只有在 exe 未命中任何规则的优先级 2 路径才会被查，那不是目标场景）。报告第 6.2 节还引用 `:159-177` 称该机制「已正确处理」，属误读 `continue` 分支。**结论：10 条全部作废**；若确要生效，必须放进**非 browser 的 kind 块**（如 chat）
- **❌ 主控修正二：`OneNote.exe` 是零效果重复条目**。exe 匹配大小写不敏感（`:133` 编译期 `to_lowercase` + `:152` 输入 `to_lowercase`），`OneNote.exe` 与表内既有 `ONENOTE.EXE` 归一化后完全相同。报告自己在第 127/313 行两次援引「toml 不区分大小写」，此处却当作新增项，自相矛盾。**22 条新增实际净 21 条**
- **⚠️ 主控修正三：✅官方 置信度标注失真（最重要）**。报告附录第 397 行自述「WebFetch 核实国内站点被 JS 占位，**主要依靠本机实证 + 通用软件知识**」——「通用软件知识」即凭记忆。故 15 条标 ✅官方 的条目（豆包/Kimi/通义/文心/ChatGLM/GLM/纳米/Perplexity/Zoom/腾讯会议/Linear/Figma/Mailbird/The Bat!/Nu），其证据实际只支撑「该产品有 Windows 版」，**不支撑「进程名就叫 X」**。这恰好触碰本次任务设定的核心红线。**须降级为 ⚠️ 未证实**
- **争议项（主控不同意见）**：报告建议删除 6 条历史推测项。主控评估**成本收益不对称**——不存在的 exe 名在运行时永不命中、开销为零（HashSet 查表），删除买不到任何收益；而万一其中某条在特定版本真实存在，删除即制造静默回归。**建议改为保留 + 注释标注「未证实」**，与 DEC-031-⑤「词表尽可能详细」一致
- **分类结论认同**：D5 六个候选场景全部归入现有 5 类、不新增 kind——主控同意（新增 kind 要改 Rust 枚举 + 解析 + 单测 + 重新出包，收益不足）。但 **Figma 归 ide_terminal 存疑**：该 style 是「保留技术术语/代码标识符、无客套」，而 Figma 文本框内容多为设计文案而非代码，`browser` 的「web-friendly 简洁单行」更贴切

**Gavin 三项拍板（2026-07-28，全部按主控建议）**：
1. **6 条历史推测项** → **保留 + 注释标 ⚠️未证实**（不删除）
2. **15 条未证实 exe** → **落地 + 注释标 ⚠️未证实**（错条目零成本；待 SCENE-OBS-001 日志实证）
3. **Figma** → 归 **browser**（非 ide_terminal）

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| IMPL-SCENE-COVERAGE-001 | 词表扩展实施：6 条存疑项保留改注释 + 新增 21 条 exe（✅实测 6 / ⚠️未证实 15）+ doc 块补 4 条工单 title_keywords + Figma 归 browser + Skype 注释 | `scene-rules.toml`（仅根目录） | coder-1 | ✅ **已验收（含主控两处修正）** |
| TEST-SYNC-SCENE-COVERAGE-001 | 测试同步：P0-1 `BUILTIN_RULES` 解析单测（堵住「toml 语法错→静默全 Unknown」黑洞）+ P0-2 特殊字符条目（`The Bat!.exe` / `Koodo Reader.exe`）+ P0-3 doc 块 title_keywords 浏览器细分生效 + P0-4 反向护栏锁死 `:162-164` continue + P0-5 归类决策断言 + P1 大小写不敏感 | `src/scene/mod.rs` `#[cfg(test)]` | tester-1 | ✅ **已验收** |
| TEST-EXEC-SCENE-COVERAGE-001 | 全量回归 686/0/8 + Tauri 53/0 + Step2/3/4 SKIP + 三副本 sha256 一致 + 新实例 PID 23056 零 parse error | — | tester-1 | ✅ **已验收（运行时实证已由 Gavin 补证闭环）** |
| BUILD-RELEASE-SCENE-COVERAGE-001 | 出包：**只重建主程序**（`cargo build --release`），把 165 条新词表经 `include_str!` 嵌入内置默认；**跳过 Tauri UI 构建**（`src-tauri/**` 与 `ui/**` 零改动，重建只产出功能等价二进制）；关键验证=新 exe 内检索 `NanoSearch.exe` / `Koodo Reader.exe` 两个探针字符串确认缓存未复用旧产物；版本号维持 0.7.2 不动 | — | tester-1 | ✅ **已验收（含主控一处事实修正）** |

**BUILD-RELEASE 验收记录（2026-07-28 主控独立取证）**：

- **产物 sha256 两副本一致**：`e35679bd…`（target/release + Publish）；crash-reporter `8bfabfb5…` 亦一致——tester-1 报告写「继承，未查」，实际它 18:41 已随 `cargo build --release` 一并重建并同步，主控补查确认无误
- **词表嵌入验证主控独立换探针**：不复用 tester-1 那 5 个，改用 6 个字符串复查，含**最易触发 toml 解析问题的 `The Bat!.exe`（含 `!`）与 `Koodo Reader.exe`（含空格）**、以及本批次归类决策项 `Figma.exe` 与 `Teambition`，全部命中 → 缓存确未复用旧产物，165 条已 `include_str!` 嵌入
- **ProductVersion 0.7.2.0 未变**；`scene-rules.toml` 三副本 sha256 `7b01b33c…` 一致
- **边界合规**：`git status` 仅 `CHANGELOG.md` 一处改动，**源文件零改动**、未碰 Tauri UI / npm build / 版本号；项目级 result.md 1251 字节非空且对应本任务
- **❌ 主控修正 · 「已知缺口」不成立**：报告称「`Publish/voice-ime-ui.exe` 沿用 07-24 旧版 10,013,184 B」。**Publish/ 根本没有该文件**——已于 07-28 按 Gavin 指令删除。Publish 内是正确的 `feiyin-ime-ui.exe`（10,027,008 B，07-27 20:31，sha256 `0d76eca1…`，即 v0.7.2 那版）。那个 07-24 文件只在 `target/release/`（本次已随磁盘清理删除），且生产代码 `src/main.rs:436~478` 七处全走 `feiyin-ime-ui.exe`。**本次出包不存在 UI 缺口**
- **⚠️ 观察（不阻塞）**：冒烟实例 PID 18928 在主控 21:40 复核时已不存在，符合 [SMOKE-VANISH-001] 既有模式，产物结论不依赖该进程

**git 收口（2026-07-28）**：提交 `695e50e`（3 文件 +195/−7：`scene-rules.toml` / `src/scene/mod.rs` / `CHANGELOG.md`），已按 Gavin 指令 push → `fb230f9..695e50e`，push 后恢复 clean remote URL，`.git/config` token 残留数 0。

**⚠️ 已知副作用（出包后确认，待 Gavin 定夺）**：本次维持 **0.7.2 不升版**（Gavin 未指示升版，遵守「版本号禁止擅改」）。故现存在**两个内容不同、但 ProductVersion 均为 0.7.2.0 的主程序构建**：07-27 20:31 那版内置 144 条词表（sha256 `7fbb1e4b…`，已被覆盖）/ 07-28 18:42 本版内置 165 条（sha256 `e35679bd…`，当前 Publish 中的）。可追溯性下降，仅靠 sha256 区分。若更看重可追溯性可升 0.7.3 重出包——但注意**本次磁盘清理已删除 release 中间产物，重出包需全量重编（含 CTranslate2 C++，预计 20+ 分钟）**，不再是 2 分钟。

**TEST-EXEC 验收记录（2026-07-28 主控独立取证）**：

- **数字链自洽**：686 = TEST-EXEC-20260727 基线 672 + 本轮 TEST-SYNC 新增 14，三方来源相加严丝合缝
- **主控定向抽跑 `builtin_rules_parse_ok` 单条 → ok**（未只信汇报表格）。这条是本批次最大风险的唯一守门人，**至此真实 165 条 toml 可被 `toml` crate 正确解析已获权威验证**。注：`BUILTIN_RULES` 是根目录 `scene-rules.toml` 的 `include_str!`，与运行时加载的 `target/release/scene-rules.toml` **sha256 逐字节相同**（`7b01b33c…`），解析保证可传递
- **三副本 sha256 主控亲自复核一致**：`7b01b33ca90b6d78…`（根 / target/release / Publish 三处），同步时间 14:03:26~27
- **实例复核属实**：PID 23056、`Responding=True`、路径 `target/release/feiyin-ime.exe`、启动 14:03:59（紧接三副本同步之后）
- **✅ 遗留项已由 Gavin 补证闭环（2026-07-28 16:38，决定性证据）**：Gavin `-debug` 重启（PID **5968**，启动 16:37:56 = `08:37:56Z`）并实际录音后，日志给出完整证据链：
  1. `08:38:04.415Z INFO feiyin_ime::scene] Scene rules loaded from "...\target\release\scene-rules.toml"` —— 主控查证 `src/scene/mod.rs:243` 该 `log::info!` **只在 `toml::from_str` 的 `Ok(r)` 分支打印**（`Err` 分支走 `:246` 的 `log::warn!` 并回落内置默认）。**故这一行本身即是「外置新 toml 运行时解析成功」的直接证明**，而该文件 sha256 `7b01b33c…` 正是同步后的 165 条版本
  2. 同一毫秒 `08:38:04.415Z Scene context: app_exe="WindowsTerminal.exe", kind=IDE/terminal, multiline_safe=false, f4_injected=true` —— 分类与 F4 注入全链路正常（惰性初始化的印证：首次 classify 与 rules 加载同刻发生）
  3. `08:38:25.629Z` 第二条 Scene context 同样正常；全日志 `Scene rules parse error` / `Scene builtin rules parse error` 命中数 **0**
  4. 时序自洽：`08:37:36Z` 那条 load 属被重启掉的上一实例（早于 PID 5968 的 `08:37:56Z`），`08:38:04Z` 属当前实例 —— **每进程恰好一次加载**，与 `OnceLock` 语义吻合
  - **结论：144→165 条新词表已在运行时真实生效，本批次运行时验证闭环，无任何遗留。**

- **（历史记录，问题已解决）**验收当时的证据空洞：tester-1 那轮的新实例自 `06:03:59Z` 启动后**从未产生任何 `Scene context:` 行**。主控查证根因：`classify_scene` 全仓唯一调用点在 `main.rs:2962` 的录音流程内，`RULES` 为 `OnceLock` 惰性初始化 → **无录音 = toml 从未被读取**。因此「新实例零 parse error」是**空洞的消极证据**——未尝试解析，何来错误。日志中最后一条 `Scene rules loaded from ...target\release\scene-rules.toml` 停在 `04:47:47Z`（旧实例 PID 18548），新实例无此行。**决定性证据只需 Gavin 触发一次听写**：正常应打出 `app_exe=..., kind=..., f4_injected=true`；若 toml 解析失败会退化为 `kind=unknown, f4_injected=false`（空规则集），一次即可分辨
- **tester-1 诚信记录（正面）**：其 result.md §72 **主动如实披露**了 `Scene context:` 未取得，并自行正确诊断出「lazy OnceLock 且需录音触发」的机制原因，未虚报。这是 [TESTER-FABRICATED-REPORT-001] 四次同模式事故后首次在同类情形下如实交底，予以记录
- **边界合规**：`git status` 无新增源文件改动（仅 IMPL/TEST-SYNC 既有改动）；未出包、未改版本号、未重建 exe；result.md 4566 字节非空
- ~~**已知可接受状态**：exe 内置默认词表仍是旧 144 条、外置 toml 为新 165 条，外置若被删除会静默退回 144 条~~ → **✅ 已于 2026-07-28 18:42 的 BUILD-RELEASE-SCENE-COVERAGE-001 消除**：内置与外置现均为 165 条，外置文件缺失也不再退化

**TEST-SYNC 验收记录（2026-07-28 主控）**：

- **边界完好**：`git diff --numstat src/scene/mod.rs` = **+154 / −0**（纯追加零删除），唯一 hunk 在 `@@ +858,154 @@`，深在 `#[cfg(test)] mod tests`（起于 `:319`）内部；**`scene-rules.toml` 的 numstat 仍是 37/7 与 IMPL 交付时逐字一致，coder-1 的成果未被覆盖**
- **断言方向逐条核对正确**（写反了测试就变成锁死 bug）：P0-3 四条断言的是 `SceneKind::Doc` 而**非** `Browser`——这是验证主控替换 no-op 方案的设计是否成立的关键，方向对了才有意义；P0-1 确实直接用 `toml::from_str::<Rules>(BUILTIN_RULES)` 并把 `parsed.err()` 带进 panic 消息，**没有**退化成吞掉 `Err` 的 `compile_rules_from_content`
- **P0-4 的诚实处理值得记一笔**：任务书要求找一个「只在 browser 块出现」的独有关键词，tester-1 核对后发现 browser 块的 title_keywords 与 email/doc 块**完全重叠、不存在唯一词**，遂按任务书给的备选路径改用内联 fixture 构造（browser 块含独有词 `UniqueBrowserOnlyKeyword`），并在注释里写明为何改用 fixture。**没有硬凑一个假的唯一词来交差**
- **实际交付 14 条 `#[test]`**（汇报写「P0×5+P1×1=6 条」是按**类别**计数，函数数为 14），全文件 scene 单测 46 → **60**
- **主控独立 `cargo check --tests` 0 errors**（81 warnings 全为既有），未采信 tester-1 自验结论
- **一处遗留观察（不阻塞）**：P0-3/P0-5/P1 用的是既有 helper `classify_builtin`（`:329`），它走全局 `rules()`，而 `rules()` 会**优先读 exe 同级的外置 toml**。cargo test 下测试二进制在 `target/debug/deps/`，该目录无 toml 故回落内置默认，结论正确；但若该目录未来出现 toml 副本，这批测试会静默测错文件。属既有 helper 的固有性质（既有 SCENE-AI-AGENT-005~008 同样使用），非本次引入，记录备查。**P0-1 不受影响**（直接解析 `BUILTIN_RULES`，环境无关）

**IMPL 验收记录（2026-07-28 主控独立取证）**：

- **逐条断言核查全过**：`OneNote.exe` 未误加(0) / `ONENOTE.EXE` 仍在(1) / `Figma.exe` 在 browser 块(`:297`) / `ChatGLM.exe`+`GLM.exe` 并存(2) / **browser 块 title_keywords 零改动**（确认没走回 no-op 老路）/ 6 条存疑项全部保留(6) / 新增条目无一条裸写置信度（唯一无标注行是 Skype 注释修改，属 D 项非新增）/ UTF-8 无 BOM（首三字节 `23 20 73`）/ Rust 源文件零改动 / 未碰三副本
- **注释质量超出要求**：`NewMailEngine.exe` 的「火狐邮件」事实错误已改正并写明理由；`wezterm.exe` / `git-bash.exe` 注释如实标注「实际前台窗口通常是已在表内的 `mintty.exe`/`wezterm-gui.exe`，此条为补漏」——没有虚报价值
- **❌ 主控修正一 · coder-1 的基线质疑不成立**：他上报「原文件实际 141 条、任务书 144 有 3 条误差、终值 162」。主控用两种独立方法复核——① `git show HEAD:scene-rules.toml` 取改动前原始文件计数 = **144**；② 严格「首个非空白字符为双引号」行计数，原始 144 / 当前 **165**。git diff 数字自洽：+29 引号行 = 21 新增 + 7 注释重写 + 1 关键词行，−7 = 被重写的 7 行。**141 与 162 两个数字均错，新增数 21 本身无误，正确表述是 144→165。**
- **❌ 主控修正二 · 「cargo test 48 passed ⇒ TOML 语法通过」推理不成立**：现有 scene 单测**全部**用 `compile_rules_from_content` 内联 fixture，无一条读 `BUILTIN_RULES`。测试通过只能证明文件是合法 UTF-8（`include_str!` 编译期校验编码），证明不了 toml 可解析——而解析失败正是本批次最大风险。这与上一轮研究任务的 ✅官方 标注失真属**同一类错误：把不构成证明的东西当成证明**
- **主控过渡验证**（WSL Python 3.10 无 tomllib/toml/tomli，未做真解析）：结构化校验全通过——数组内每行均符合「条目 + 可选尾注释」或纯注释模式（零可疑行）、全文件双引号成对、方括号 11:11 配对、`[[scene]]` 块数仍为 8。**权威解析验证交 TEST-SYNC 的 P0-1 单测 + TEST-EXEC 的运行时 debug.log 核查**

**主控对 title_keywords 的替代设计（派发时补入，非 coder-1 原方案）**：原 10 条作废后，改为只在 **doc 块**加 `Jira / TAPD / 禅道 / Teambition` 4 条——doc 的 kind 非 Browser，浏览器细分循环（`scene/mod.rs:161-177`）会查到它，浏览器开 Jira → 重分类 doc → `multiline_safe=true`，工单描述可得多行结构化输出，与表内既有「腾讯文档 / Google Docs」同一条路径。**排除 `Linear` 关键词**（英文常用词，"linear regression" 会误命中；`Linear.exe` 走 exe 精确匹配即可）、排除微博/小红书/知乎（browser 默认已够）、排除 Salesforce/Zendesk（低频）。

**⚠️ 本批次最大风险（已写入任务书）**：`scene/mod.rs:258-267` 运行时 toml 解析失败只打 `log::warn!` 后**降级空规则集 → 所有场景全变 Unknown**，无用户可见报错。`The Bat!.exe`（含 `!`）与 `Koodo Reader.exe`（含空格）是最易出错的两条。现有 46 条 scene 单测用内联 fixture，**覆盖不到真实 toml** —— 故 TEST-SYNC 必须补一条解析 `BUILTIN_RULES` 的单测把这个洞堵上。

**主控增补建议（省一轮研究）**：SCENE-OBS-001 刚落地的 `Scene context:` 运行时日志会打印 `app_exe`——Gavin 日常使用中打开豆包/Kimi 等桌面版时，**真实进程名会自动出现在 debug.log 里**。与其再派一轮网络查证，不如先落地带 ⚠️ 标注的条目，用实际使用日志做零成本实证收口。

### 2026-07-27 批次 · Gavin 端测四项（v0.7.1 后）

> 来源：Gavin 2026-07-27 `-debug` 端测反馈 4 项 + 主控日志取证分析
> 诊断记录：`troubleshooting.md [SCENE-OBSERVABILITY-001]`、`logs/20260727.md`
> 边界：文件级零重叠；版本号不动；③ 拆两侧协同（coder-2 改符号，coder-1 改 prompt 保护）

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| SCENE-OBS-001 | 场景感知运行时日志（方案 C）：采集后打 exe+kind+multiline_safe+f4_injected（不打窗口标题）；F4 块整段单独打印绕开 200 字符截断 | `src/main.rs` + `src/llm/mod.rs` | coder-1 | ✅ **已验收** |
| LANG-MIXED-001 | 中日/中韩夹杂被强行译成中文：新增假名/谚文探针；`script_instruction` 措辞收紧为「只统一中文字形、不翻译非中文」；混合文本跳过 zhconv | `src/text_normalizer.rs` + `src/llm/mod.rs` | coder-1 | ✅ **已验收** |
| ITN-CELSIUS-002-PROMPT | LLM 改写回汉字单位的保护条款（**必须运行时追加，不能只改 i18n 默认 prompt**——存量 config 已持久化） | `src/llm/mod.rs` | coder-1 | ✅ **已验收** |
| ASR-NOSPEECH-FILTER-001 | 空语音返回 `<\|nospeech\|>` 被注入：转录出口剥离 `<\|...\|>` 全族 token，剥离后为空走空转录路径 | `src/transcription/mod.rs` | coder-2 | ✅ **已验收（含主控一处修正）** |
| ITN-CELSIUS-002-SYMBOL | 摄氏度符号 `°C`(U+00B0+C) → `℃`(U+2103) 单字符；**单说「度」不转**（Gavin 2026-07-27 拍板：角度/温度同形，强转会把「转九十度」变「转90℃」） | `src/itn.rs` | coder-2 | ✅ **已验收** |
| TEST-SYNC-20260727 | 测试同步：P0 补 6 条边界 + P1 补 2 条日文汉字零改变断言 + P2 无旧措辞绑定 + P3 前端无需同步 | 各 `#[cfg(test)]` | tester-1 | ✅ **已验收** |
| TEST-EXEC-20260727 | 全量回归：Step 1 672/0/8 PASS、Step 1b 53/0/0 PASS、Step 2/3/4 SKIP；**未出包** | — | tester-1 | ✅ **已验收** |

**Gavin 确认要点（2026-07-27）**：
- ③ 实际现象为「返回汉字摄氏度」，判定主因是 LLM 优化阶段把 ITN 已规整的符号改写回中文 → 主修在 prompt 保护，符号统一为 ℃ 为辅
- ② 端测环境为 Claude Desktop；`Claude.exe` 已在 `scene-rules.toml:86` chat/AI-agent 词表内，另两次分类为 browser/ide_terminal 是否误判**待 SCENE-OBS-001 日志落地后复核**

**主控验收记录（2026-07-27）**：
- **coder-1 三项全部 Read 核验通过**：翻译路径回归护栏落实到位——`main.rs:3011` 确实改调 `script_instruction_for_translate`、`:3063` optimize 路径保持原函数，两者分流正确；6 条护栏单测用反向断言 `!contains("不要翻译")` 锁死回归；探针码区正确（假名 U+3040-309F/U+30A0-30FF、谚文三区）；两处 zhconv 均加跳过；ITN 保护条款为模块级 const 且在 `llm/mod.rs:336`(翻译)+`:451`(optimize) 两条运行时路径追加，**未碰 `src/i18n.rs`**（符合"存量 config 已持久化、改默认无效"的关键约束）；场景日志字段不含 `window_title`，隐私红线遵守
- **coder-2 两项通过，主控修正一处缺陷**：`strip_asr_special_tokens` 的 `pending_space` 对 CJK 也补空格，`你<|NEUTRAL|>好` → `你 好`，中文凭空多出空格且会直接注入用户输入框；**该缺陷还被写进断言当作预期值**（`mixed_with_text` 期望 `"你 好"`），属于把 bug 固化成设计。已收紧为「两侧均 ASCII 字母数字才补空格」（英文 `hello<|X|>world` → `hello world` 防粘连行为保留），断言改为 `你好`，新增 2 条护栏单测；主控独立 `cargo check --tests` 0 errors
- **未跑 cargo test**（遵守 orchestrator 不亲自执行测试的边界），全量回归归入 TEST-EXEC

**衍生发现（未立项，待 Gavin 拍板）**：`should_translate_for_language`（`main.rs:3202`）判据为 `contains_han`，而日文汉字与中文汉字同码区 → 日文文本被判为"已是中文"，`target=Chinese` 时**翻译热键对日文会静默跳过**。属 LANG-AUTO-001 既有语义遗留（非本批次引入），但中日混合成为明确需求后该缺口更突出，与 todo「等 Gavin 拍板 · 翻译方向是否改双向全自动」同源。

**TEST-SYNC 验收记录（2026-07-27 主控）**：
- **主控修正三要素逐一核验完好**（本次派发时特别提醒不得覆盖）：补空格条件的 `is_ascii_alphanumeric` 双侧判断在位、`mixed_with_text` 断言仍为 `你好`、两条主控护栏单测均存在
- **P1 断言方向正确**（最关键）：断言 `contains("龍")` / `contains("亞")` 保持不变，而非断言转换结果——且选字精准，龍/亞 都是 zhconv 必转字，一旦跳过逻辑失效测试立即变红
- **P0 覆盖到位**：`strip_asr_special_tokens` 系列累计 15 条（coder-2 原 7 + 主控 2 + tester-1 补 6），新增覆盖 token 串首/串尾/已有空格/全角标点相邻/连续 token 夹 ASCII/空输入
- **生产代码零改动确认**：关键生产函数逐一复核完好；主控独立 `cargo check --tests` 0 errors
- **一处 cargo fmt 连带**：`src/wordbook/db.rs` 出现 1 处改动（`mod tests` 内 `assert_eq!` 多行压单行，token 逐字相同），按 [FMT-COLLATERAL-001] 定性为**零逻辑改动的格式化连带**，保留不回滚。提醒：任务书要求 `cargo fmt -- <file>`，本次仍出现全量连带

**TEST-EXEC 验收记录（2026-07-27 主控独立取证）**：
- **数字链自洽**：672 = coder-1 交付基线 662 + tester-1 TEST-SYNC 补 8 + 主控修正补 2，三方独立来源相加严丝合缝
- **主控针对性抽查 4 组共 30 条全绿**（未只信汇报表格，按 [TESTER-FABRICATED-REPORT-001] 强制取证）：`strip_asr_special_tokens` 15/15（数量正好等于逐条核对的 7+2+6）、`translate_path` 8/8、`keeps_japanese_kanji` 2/2、`temp_celsius` 5/5
- **result.md 2213 字节非空**且内容对应本任务（[COLLAB-WRITE-001] 0 字节问题未复发）
- **边界遵守确认**：未出包、未同步 Publish、未改生产代码、版本号文件未动

**批次状态**：**五项代码 + TEST-SYNC + TEST-EXEC + 版本号 + 出包 + git 提交全部闭环**（2026-07-28 收口）。

**收口记录（2026-07-28 Gavin 指令「commit / 查版本号 / 出包」→ 主控独立取证）**：
1. **git commit** —— 代码提交 `155b595`（07-27 20:24，11 文件 +835/-38，含三处版本号 bump）已在上次 session 完成；本次补提交 `fb230f9`（CHANGELOG 出包记录 + `src-tauri/Cargo.lock` 0.7.1→0.7.2 构建期传播）。工作区 clean；**两个提交已按 Gavin 指令 push 到 GitHub**（`9540335..fb230f9`），push 后已恢复 clean remote URL，`.git/config` 零 token 残留，`git status -sb` 确认 `main...origin/main` 完全同步
2. **版本号** —— 已是 **v0.7.2**：`Cargo.toml` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` 三处均 0.7.2，产物 ProductVersion 0.7.2.0（UI 侧 0.7.2）
3. **出包** —— 已于 07-27 20:31 完成，**本次未重复构建**（源码自 20:02 后零改动，重建产物无功能差异，遵守「不要频繁出包」）。主控取证：三处 sha256 全一致（feiyin-ime `7fbb1e4b` / feiyin-ime-ui `0d76eca1` / crash-reporter `559c7506`）、itn-rules + scene-rules 三副本一致、产物 mtime 20:28~20:31 晚于全部源码 mtime 与代码提交时间、UI exe 内嵌当前 dist 资产（`index-BNQZfcUG.css` / `index-CTgGziQm.js` 各命中 1）、运行实例 PID 18548 指向 `target/release/feiyin-ime.exe` 且 Responding=True

**遗留小项**：~~`Publish/voice-ime-ui.exe` 旧包名死文件~~ → **已按 Gavin 指令删除（2026-07-28，10,013,184B）**。删除前核实生产路径 `src/main.rs:436~478` 七处全部使用 `feiyin-ime-ui.exe`，旧名仅存在于非生产文件中（见下表），运行时零影响。

**删除时发现的陈旧引用（未处理，待排期）**：

| 文件 | 问题 | 影响 |
| --- | --- | --- |
| `build.bat:26` / `scripts/init-publish.ps1:112` | 仍按旧名 `voice-ime.exe` / `voice-ime-ui.exe` 清理与拷贝产物 | 这两个脚本若被执行会产出错误的 Publish 清单（现出包走 build-guide 三步流程，未走这两个脚本，故未暴露） |
| `src/main - 副本.rs` | 早期 `main.rs` 的手工副本，含 mojibake 注释，451~498 行全是旧名 | 未被 cargo 编译（非模块），纯仓库垃圾文件，建议删除 |
| `tests/ui_guard_tests.rs` | GUARD-001/002 骨架测试注释与路径拼接用旧名 | 均为未实现的骨架（无真实断言），不影响测试结果 |

**端测重点（v0.7.2 已可端测）**：① 中日/中韩混合语句看日韩文是否原样保留 ② 说「今天三十摄氏度」看是否输出 `30℃`（LLM 改写主因判定目前仍是推断，只有真实 LLM 调用能证实）③ 空语音看是否不再注入 `<\|nospeech\|>` ④ 看 debug.log 的 `Scene context:` 行确认场景分类是否误判

> **2026-07-25 批次已全部闭环并出包**（详见 CHANGELOG / progress）：
> - **WORDBOOK-SCHEMA-FIX-001**（P0 词库全瘫）：CORE + UI 均已验收，**Gavin 端测目视确认词库页列出 5 条，P0 闭环**
> - **WORDBOOK-AUTOLEARN-FIX-001**（A+C+D）：CORE + TAURI + TEST-SYNC 均已验收
> - 出包：`b0c70b3` 提交 → 三 exe 19:38 重建同步 Publish，621/0 + 53/0 + 54/54，ProductVersion 0.7.1.0 不升版
>
> **⚠️ 自动学习效果仍待观察（非 bug，属机制固有）**：阈值维持 2（Gavin 决定不改），即**同一个词需被独立建议 2 次才入库**。故短期端测多半只见候选累积、不见入库，属预期行为。观察方法：`target/release/debug.log` 中 `suggestions after_tag` 非空率（修复前 13/96 ≈ 13.5%，A 生效应显著上升）与 `WORDBOOK-AUTOLEARN-FIX-001-C: rejected ...` 的原因分布（大量 `not_in_corrected_text` = LLM 仍返回错字侧；大量 `too_long_cjk` = 8 字上限偏严，改一行 const 即可调）。想快速验证闭环可对同一个 ASR 老出错的词连说两次。

### ~~WORDBOOK-AUTOLEARN-FIX-001~~（已闭环，保留实施记录备查）

> 诊断依据：`troubleshooting.md` [WORDBOOK-AUTOLEARN-001]（实测 102 次 LLM 请求：触发率仅 13.5%、57 候选 count 全为 1、0 次入库；链路本身通，有 `艾丁湖` 成功入库实证）
> Gavin 决策：A+C+D 实施 ｜ **B 阈值不改、不加 UI 复核** ｜ **E 多库并存不收口（DEC-032）**

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| WORDBOOK-AUTOLEARN-FIX-001-CORE | A 解 prompt 冲突（SUGGESTION_INSTRUCTION 加 OVERRIDES 覆盖声明，破解用户 config 中"strictly prohibited: adding suggestions"条款）+ C 入库前过滤（**核心判别法：建议词必须出现在纠正后正文中**，零词典剔除 ASR 错字侧同时保留日常生活词汇；配结构性过滤换行/长度/句读/纯数字/中文单字 + 拒绝原因日志）+ D 主程序侧默认 prompt 旧格式修正（3 处 ZH/ZH-Hant/EN） | `src/llm/mod.rs` + `src/i18n.rs` | coder-1 | ✅ **已验收（含主控一处修正）** |

**CORE 验收记录（2026-07-25 主控）**：
- Read 实际改动确认：A 的 OVERRIDES 声明点名三类在先禁令（`adding your own suggestions` / `thoughts regarding corrections` / `prefix/suffix output`）且限定"仅作用于这一行 JSON、其余禁令继续有效"，含 machine-readable protocol line 定性 + 日常高频词显式在范围内 + `风无星→风无心` 真实正反例；C 的七条过滤规则齐备、`MAX_CJK_CHARS=8`/`MAX_TOTAL_CHARS=24` 具名 const 双限同查、**铁律遵守**（入库存 `word.to_string()` 原形，归一化仅用于比较且注释写明）、每条拒绝均有带原因的 `log::info!`；两个返回分支 + translate 路径均正确传入 `<corrected>` 正文
- **主控修正一处缺陷**：`has_sentence_punct` 黑名单误含 ASCII 撇号 `'`(U+0027)，与该函数自身注释（声称撇号放行、`O'Brien` 不应误杀）自相矛盾——实测 `O'Brien`/`don't`/`it's` 被拒，而弯撇号 `’`(U+2019) 反而放行，行为不一致。英文所有格与缩写属 Gavin 明确要求收录的日常用语，已移除撇号条目并去掉 `"`/`'` 重复项，注释补修正说明
- 独立复核：`cargo check` 0 errors；`cargo test` **606 passed / 0 failed**（含主控修正后重跑）；14 条 `fix001_*` 单测全绿；D 两侧段落 **md5 逐字节一致**（`cc7c1e58...`），旧格式与 correction pair 措辞双侧零残留
- **cargo fmt 连带**：`build.rs` / `src/bin/poc_{halluc,funasr_nano,vad}.rs` / `src/config/mod.rs` 5 文件系 `cargo fmt` 全量格式化连带，已用三步法（去空白 md5 → token diff → 去逗号 md5）逐一证明**零逻辑改动**，保留不回滚，详见 troubleshooting [FMT-COLLATERAL-001]
| WORDBOOK-AUTOLEARN-FIX-001-TAURI | D Tauri 侧默认 prompt 旧格式镜像修正（3 处），措辞与 coder-1 侧对齐 | `src-tauri/src/i18n.rs` | coder-2 | ✅ **已验收**（2026-07-25：主控 Read 三处实际改动确认单词格式+收录范围含日常词汇+verbatim 约束就位，旧格式与 correction pair 措辞零残留，独立 `cargo check --manifest-path src-tauri/Cargo.toml` 0 errors，边界未越界；另纠正其日志中「旧格式导致解析失败」的错误因果表述——解析层两种格式皆兼容，D 的价值是统一口径而非修 bug） |

**边界**：两任务文件级零重叠，并行派发。**C 的范围修正**：主控原分析误把 `时代/吉他/惊心动魄` 归为"应过滤的通用词"，Gavin 纠正——日常生活词汇是高频词汇必须支持入库，过滤只针对垃圾（错字侧/整句/超长）。
| TEST-SYNC-WORDBOOK-AUTOLEARN-001 | 测试同步：P0 补主控撇号修正的测试护栏（`fix001_keeps_apostrophe_words` / `keeps_curly_apostrophe` / `rejects_ending_punct_variants` 新增 3 条 + `keeps_intra_word_connector` 扩展）+ P1 全测试面签名变更审计（零偏差）+ P2/P3 评估无需补 | `src/llm/mod.rs` `#[cfg(test)]` 块 | tester-1 | ✅ **已验收** |

**TEST-SYNC 验收记录（2026-07-25 主控）**：Read 三条新单测确认断言方向正确（`O'Brien`/`don't`/`it's` 断言**保留**且校验 len==3、弯撇号 `it’s` 单独覆盖、句末标点四变体循环断言**拒绝**——反向护栏防黑名单被删空）；4 条相关单测逐条实跑全 ok；**主控独立跑全量 `cargo test` 609 passed / 0 failed**（606+3 新增，数字链自洽）；`cargo check --tests` 0 errors；生产要素五项逐一复核确认 tester-1 未动生产代码（双 const / 黑名单无 U+0027 / 入库存原形 / OVERRIDES 声明 / 交叉校验分支全部在位，**主控的撇号修正与说明注释完好未被覆盖**）

**批次状态**：**A+C+D 代码 + 测试同步全部闭环**。下游：TEST-EXEC（全量回归 + 三步构建 + Publish 同步）等 Gavin 下「现在可以出包」指令后派发。
**注意**：本修复的实际效果**必须端测验证**——A 的 prompt 覆盖能否把触发率从 13.5% 拉起来，只有真实 LLM 调用能证明，单测无法覆盖。建议出包后观察 `debug.log` 中 `suggestions after_tag` 非空率与 `WORDBOOK-AUTOLEARN-FIX-001-C: rejected` 日志分布。

---

## 等 Gavin 拍板

| 事项 | 说明 |
| --- | --- |
| 翻译方向是否改双向全自动 | 现 v1 语义：翻译热键方向由隐藏字段 `translation.target_language` 决定（默认 Chinese），反方向语音自动跳过不翻译；`contains_han` 只做同语种跳过门控。若期望双向全自动需另立项——LLM 路径易改，NLLB 离线路径需按方向换模型（LANG-AUTO-001-CORE 验收时提出，2026-07-14） |
| FORMAT 保底层（A 方案）是否追加 | 不开 LLM 的用户是否需要规则层语气词去除保底（仅"嗯/啊/额"必删项，不碰口头禅，可复用 itn-rules.toml 外置模式）；现方案立场：不开 LLM 不做语义格式化 |
| DEC-028 Qwen3 收尾一项 | `qwen3_asr_url` 默认值维持 dashscope 还是留空强制用户配置（Gavin 实际用工作空间 endpoint，默认值对 MaaS key 用户无效） |
| TELEGRAM-RESTART-001 修复路线 | 见「未排期任务」表 |
| accuracy 体感根因收口（已降级，非阻塞） | 三选一：F1 DEBUG-AUDIO-DUMP-001（`--debug` 存喂模型前音频，小改动）｜F2 Gavin 录 5-10 条日常短指令语料 ｜ 提供当时 accuracy 出错的具体实例（定向复现最快）。**2026-07-25 起进一步降优先级**：accuracy 模型已从 UI 隐藏（ASR-HIDE-ACCURACY-001），用户侧不可达（研究报告 RESEARCH-ASR-ACCURACY-002/003 见 collab/research/） |

---

## 已知遗留问题（待修复）

| 编号 | 问题 | 文件 | 优先级 |
| --- | --- | --- | --- |
| ASR-HALLUC-SEGMENT-001 | accuracy 长音频 VAD 分段中段语义级幻觉（40.1s 语音切 3 段，中段被无关内容整段替换），现有三重兜底（字/秒、重复、空输出）全部拦不住；候选方案：每段用 CTC 交叉转录比对字符重合度 | `src/transcription/mod.rs` | **挂起**（2026-07-25 Gavin 拍板暂不排期：accuracy 已从 UI 隐藏，触发路径不可达。**若未来重开 accuracy 或引入其他 LLM-decoder 类 ASR 模型，必须同批立项**，详见 troubleshooting [ASR-HALLUC-SEGMENT-001]） |
| TEST-FIX-002 | App.test.tsx 缺少 `@tauri-apps/api/core` mock，2 个用例 FAIL | `ui/src/App.test.tsx` | 低 |
| TEST-FIX-003 | Wordbook.test.tsx `getByRole("dialog")` 无 role 属性时报错 | `ui/src/pages/Wordbook.test.tsx` | 低 |
| TECH-DEBT-001 | parse_version 实现不一致：主程序先 split('-') 再 split('.')，Tauri 侧先 split('.') 再 split('-')，prerelease 处理结果有差异 | `src/version_check/mod.rs` + `src-tauri/src/version_check.rs` | 低 |
| ACC-DEGRADE-UI-001 | accuracy 静默降级 performance 时 UI 仍显示 accuracy（可观测性缺口，Gavin 环境不触发；accuracy 已隐藏后影响进一步收窄） | `src/transcription/mod.rs:532-547` | 低 |
| MOJIBAKE-COMMENT-001 | `main.rs` ~L2996 TRANS-008 段既有 mojibake 注释（历史编码损伤，非功能影响），归入待清理小项（LANG-AUTO-001-CORE 验收时发现，2026-07-14） | `src/main.rs` | 低 |

---

## 未排期任务

### PLATFORM-001 · macOS 跨平台（Phase 4-6）

| 编号 | 任务 | 前提条件 |
| --- | --- | --- |
| MAC-008 | macOS 构建环境 + CI 配置 | Phase 3 ✅ |
| MAC-009 | 代码签名 + Notarization | Apple Developer |
| MAC-010~013 | E2E/热键/注入/Overlay | MAC-008 |

### 其他待排期

| 编号 | 任务 | 备注 |
| --- | --- | --- |
| PROMPT-CLEANUP-001 | 默认 system_prompt（src/i18n.rs default_system_prompt_en）Rule 4/5（Markdown/List Formatting）与 F1~F4 指令体系职责重叠清理：格式化策略应统一归 F 段管辖，默认 prompt 只留纠错/语义职责。注意 system_prompt 持久化在用户 config，改默认仅对新建配置生效，存量靠 FMT-LLM-002 的 F3 Override 兜底 | 待 Gavin 排期（FMT-LLM-002 审计发现，2026-07-13） |
| WORDBOOK-CORRECTION-UI-001 | 注入后 overlay 纠错入口（Gavin 2026-07-06 选定方案 2）：识别注入完成后 overlay 短暂显示「纠错」小按钮，点击弹出编辑框改文本，确认后调 wordbook.learn_correction 入库（喂 hotwords）。背景：WM_GETTEXT 读回在现代应用无效（RESEARCH-TEXTCAPTURE-001），自动学习路径 A 实际失效，需自建纠错闭环。涉及：src/main.rs（overlay 生命周期）+ Win32 overlay 绘制 + wordbook API（已有）。注意：DEC-029 词库已单词化，此任务未来改为学单词 | 待 Gavin 排期 |
| UI-I18N-COMPLETE-001 | UI 硬编码文字补全 i18n（coder-2 2026-07-07 建议）：App.tsx Loading/Error、Llm.tsx Success/Failed 等 5 处状态文字补 key + 替换；热键显示名（VK_TO_LABEL 的 Ctrl/F9）属通用键名评估后大概率不译 | 待 Gavin 排期（归入"小优化"批次） |
| LLM-KEY-REVEAL-001 | 格式化输出页 API Key 明文查看（显示/隐藏小按钮）：现为 password 掩码，Gavin 端测反馈框太小看不清已由 FORMAT-UI-POLISH-001 解决，明文查看为当时保留的备选追加项 | 待 Gavin 排期（归入"小优化"批次，2026-07-14 提出） |
| TELEGRAM-RESTART-001 | Telegram 通道恢复（troubleshooting [TELEGRAM-CHANNEL-001]）：2026-07-07 实测重启无效，根因为 Claude Code 2.1.202 服务端功能开关（tengu_harbor）拦截，本地无解；候选路线：降级 CLI / 等官方放开 / 临时手动轮询 | 待 Gavin 拍板路线 |
| MAC-P2-001 | macOS 热键 dispatch 延迟优化（P2） | Windows P2 完成后评估 |
| RESEARCH-TEXTCAPTURE-001 | 现代应用文本捕获方案研究 | WM_GETTEXT 在现代应用无效 |
| DEC-014 | WebView2 自动安装 | Win10 用户 |
| CRASH-EMAIL-001 | crash reporter 邮箱设置 | 需 SMTP 配置 |
| QWEN3-CORPUS-BIAS-001 | qwen3 在线模型接入词库偏置：官方 input_audio_transcription.corpus.text（max 10K tokens）可作 hotwords 等价通道；注意两份官方文档记载矛盾，实施前需实测验证。注意：DEC-029 词库已单词化，corpus 直接用单词列表 | 待 Gavin 排期（研究报告 collab/research/qwen3-protocol-alignment-001.md）|
| QWEN3-STREAM-V2 | qwen3 真流式边录边上屏（DEC-028 v1 为整段上传，流式留作演进） | 待 Gavin 排期 |

---

## Phase 3 · 演进评估（不排期，仅记录）

场景感知后续演进方向：UIA 控件信号、浏览器细分词表迭代、内容压缩独立开关、语气适配 LLM 兜底开关（未命中时参考应用名，默认关）、个性化风格学习。

来源：DESIGN-FORMAT-SCENE-001 技术方案（collab/research/typeless-format-design-001.md）+ DEC-031。
