# handoffs · voice-ime

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

