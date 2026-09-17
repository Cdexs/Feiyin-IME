# 任务列表 · voice-ime

> **本文件只放「还没做的事」。** 已完成的功能 → `progress.md`；任务完成记录 → `CHANGELOG.md`；
> 过程记录、取证细节、历史批次 → `todo-archive.md`。
> 每条待办的「详情」列指向 `todo-archive.md` 里的原始小节，细节一条没丢，别在这里展开。
> 维护规则见文末。

---

## 当前状态（2026-09-17）

| 项 | 状态 |
| --- | --- |
| 版本 | **v0.9.0 已发布** —— tag + GitHub Release 均已上线，main 已推至 `5714f63`（含三条 README 文档提交），工作区 clean |
| Worker | ✅ **09-17 已恢复可用**：OpenCode Go 额度确认未恢复（提示约 10-04 重置），Gavin 指令三个 Worker 统一切 `commandgo/deepseek/deepseek-v4.1-flash`，已重启并逐个核验就绪 |
| 文档 | 2026-09-10 已按 DEC-064 改「索引 + 归档」两层，启动必读 227,865 → 23,022 token。**新增条目必须 archive 与索引两边都写** |
| 下一步 | 优先级最高的是 `PROMPT-OPT-204`（需 A/B 授权）与 `TEST-SYNC-194` |

---

## 🔴 待做

### 🆕 v0.9.1 批次（Gavin 2026-09-17 下单，含版本升级授权）

**VER-BUMP-217** · 版本号 0.9.0 → 0.9.1
`Cargo.toml:3` + `src-tauri/Cargo.toml:3` + `src-tauri/tauri.conf.json:9` 三处。
`ui/package.json` 是 0.1.0，历史就没跟版本，**不动**。Gavin 明确授权，见本条。

**V091-PUNCT-TAIL-214** · 「句尾不显示标点符号」新开关（默认关）

| 项 | 内容 |
| --- | --- |
| 影响文件 | `src/config/mod.rs`(PunctuationConfig) / `src/punctuation/mod.rs`(apply_l2_postprocess) / `src/main.rs` / `ui/src/pages/Voice.tsx` / `ui/src/i18n/{en,zh-Hans,zh-Hant}.ts` |
| 复用 | 🔴 **不新建剥离逻辑** —— `L2Action::StripTrailing` 已存在（短文本已在剥尾标点），本需求 = 把「短文本才剥」扩成「开关开则恒剥」 |
| 产出源 | 主 pipeline(`main.rs:9022`)／FocusLost 预览(`6734`)／macOS FocusLost(`8292`) 三源都继承 L2；🔴 **overlay 编辑态提交(`6870→6895/6920`)完全绕过 pipeline，须单独处理** |
| 验收 | 四条产出源逐条实测 + 翻译模式 + 开关关闭时行为与现状逐字一致 |

**V091-ITN-SKIP-ONLINE-215** · 线上 ASR 跳过 ITN

| 项 | 内容 |
| --- | --- |
| 影响文件 | `src/main.rs`（🔴 **与 214 同文件，必须同一 Worker 或串行**）|
| 两个调用点 | DEC-036 双通道：`main.rs:8704` `normalize_numbers`（主通道，回 LLM 前）／`main.rs:8934` `normalize_unit_symbols_only`（补丁通道，标点前）|
| 主控建议 | **只跳主通道**。补丁通道服务的是 LLM 输出而非 ASR 输出，不属于「二次重复处理」，跳了会让 LLM 写回的中文单位失去转换 |
| 门控 | `AsrModel::from_config(&cfg.audio.asr_model).is_online_streaming()`（`transcription/mod.rs:81`）|

**V091-ITN-FIX-YIKE-216** · `那一刻` → `那1刻`

| 项 | 内容 |
| --- | --- |
| 根因 | `itn-rules.toml:119` `[date_time.triggers] suffix` 含「刻」→「一刻」被当数字+时间单位 |
| 🔴 红线 | DEC-038 禁止词表打补丁 —— **不许往 protected 表加「那一刻」**（表里已有「一刻不停/一刻千金/一刻钟」等，正是打补丁的历史遗留）。要做文法级前置护栏，先例见 `src/itn.rs:2310/2322` `is_unit_preceded` |
| 🔴 取证先行 | 动代码前必须先测同族：`那一天/这一年/每一次/哪一天/这一刻` 是否同样中招，按 `[ITN-LOCAL-RULE-OVERREACH-001]` 配边界外护栏测试 |
| 影响文件 | `src/itn.rs`（+ 可能 `itn-rules.toml`）—— 与 214/215 无文件重叠，可并行 |

**V091-ITN-YIKE-HANT-220** · 216 闸门对繁体中文半失效（coder-1 阶段三独立发现）

| 项 | 内容 |
| --- | --- |
| 现象 | `is_demonstrative_yi`（`itn.rs:2423-2424`）字符集合只写了简体字面量：`这`、`么`。繁体 `這`、`麼` 不命中 |
| 🔴 可达性已证 | **不是不可达路径**：`main.rs:8666` 转录时即按 `config.audio.chinese_script` 产出繁体，ITN 在 `:8729` 拿到的就是繁体文本。设置界面「语音输入 → 中文输出 → 繁体」是用户可选项 |
| 为什么必须本批修 | `那/每/哪/某` 繁简同形，所以繁中用户会拿到**一半修好一半没修**：`那一年` 修了、`這一年` 没修。**不一致的半修状态比统一不修更糟**，用户无法形成稳定预期 |
| 改动 | `:2423` 加 `'這'`、`:2424` 的 `'么'` 扩为 `'么' \| '麼'` 且前两字集合加 `'這'`。约 3 处字面量 |
| 边界 | 只补繁体同义字，**不扩语义范围**；不碰条件③的 `is_date_suffix` |
| 护栏归属 | 🔴 交叉规则照旧：谁改生产代码谁不写断言 |

### 🆕 UI-ASRKEY-225 · 在线 ASR 的 Key 输入框标签与占位提示（Gavin 2026-09-17）

| 项 | 内容 |
| --- | --- |
| 需求 | 选「在线语音识别 - FunASR」时：① 标签 `ASR API Key` → `API Key（阿里云百炼Key）` ② 输入框内淡灰色**斜体**占位提示「输入你在阿里云百炼平台的API Key...」，输入时消失、清空后自动回来 |
| 影响文件 | `ui/src/pages/Voice.tsx`（现有渲染条件 `:280`）+ `ui/src/i18n/{en,zh-Hans,zh-Hant}.ts` 三份；现标签 key 为 `voice_asr_online_api_key` |
| 🔴 实现方式收窄 | Gavin 描述的是「监听 focus/blur 事件判断是否填入提示文字」。**不要这么做** —— HTML 原生 `placeholder` 属性的行为与需求逐条一致（空则显示、输入即消失、清空即回来），配 `::placeholder` 设斜体与灰度即可。手写 focus/blur 会把提示文字变成真实 value，用户不改就会被当成 Key 提交 |
| 注意 | 🔴 三份 locale 必须同时补（`I18N-HANT-GAP-001`）；后端零改动 |
| 排期 | **等 TEST-EXEC-224 复跑结束再派** —— 本单动 `ui/`，tester-1 正在跑 vitest/browser，中途改会让它测到移动靶 |

### 🆕 UI-LLMHINT-226 · 格式化输出「API 配置」标签补建议文案（Gavin 2026-09-17）

| 项 | 内容 |
| --- | --- |
| 需求 | 配置界面/格式化输出，`API 配置` → `API 配置（建议使用deepseek-flash模型，参数格式参考提供商文档）` |
| 影响文件 | `ui/src/i18n/{en,zh-Hans,zh-Hant}.ts` 的 `llm_api_config` 三份（现值 `'API 配置'`）。`Llm.tsx` 已引用该 key，**组件零改动** |
| 注意 | 🔴 三份 locale 同时改，英文版语义等价即可不必逐字直译 |
| 归并 | 与 `UI-ASRKEY-225` 同属 `ui/i18n`，**合并给 coder-2 一次做完**，避免同文件并发 |

### 🆕 ITN-FIX-LIANGDIAN-223 · `这两点一个都不能少` → `这2.1个都不能少`（Gavin 2026-09-17 端测报）

| 项 | 内容 |
| --- | --- |
| 复现 | 「这**两点一**个都不能少」→「这**2.1**个都不能少」；「这两点**一点**都不能少」同病 |
| 语义 | 「两点」= 两个要点（量词），「一个都不能少」是独立短语。ITN 把 `两点一` 当成了小数 2.1 |
| 🔴 取证先行 | 主控初查**未定位**：乙型 `try_parse_implicit_decimal`（`itn.rs:1607`）第 1624 行已有护栏 —— `date_suffixes` 命中且非「度」即 `return None`，而「点」在 `date_time.triggers.suffix` 里，**理论上乙型不该接手**。所以真凶可能是甲型/丙型/`parse_cn_number` 的小数点处理，**必须实测定位，禁止照「乙型越界」这个假设动手** |
| 同族 | `[ITN-LOCAL-RULE-OVERREACH-001]`（局部规则在更长上下文越界）。护栏必须配边界外用例 |
| 🔴 边界外红线 | 真小数不得被误挡：`两点一五`→2.15、`三点五`、`两点一度`（度是温度单位兼 date_suffix）、`下午两点一刻` |
| 影响文件 | `src/itn.rs`（与 221/222 同文件，**必须串行**） |

### 🔴 ITN-HANT-SYSTEMIC-221 · ITN 对「繁体输出」用户大面积降级（待 Gavin 拍板，**不进 v0.9.1**）

**由来**：coder-1 做 220 时上报「繁中『那一號』不受保护」，主控顺藤取证，发现问题比那大得多。

| 判据 | 取证 |
| --- | --- |
| ITN 确实拿到繁体文本 | `transcription/mod.rs` 的 `transcribe_with_punct_info` 内部就调 `text_normalizer::normalize_text_for_language(trimmed, script)`，**ASR 出口即繁体**；ITN 主通道在 `main.rs:8729` 之后 |
| 规则表全简体 | `itn-rules.toml` 繁体异形字出现次数 **全为 0**；对应简体字出现 200+ 次（`号`29 `点`38 `个`37 `万`19 `时`18 `岁`17 `钱`16 `块`12 `亿`3） |
| ITN 不做繁简归一 | `src/itn.rs` 内无任何 `Traditional` / 归一化逻辑，自身源码繁体字亦为 0 |

**后果**（繁体输出用户）：`年月日分秒刻` 繁简同形故仍可用；但
日期 `五號`、金额 `五萬塊` / `三塊錢`、时间 `三點半`、年龄 `十歲`、量词 `三個` **全部不转**。
**金额与时间恰是 ITN 的核心用例。**

**三条候选路线（需 Gavin 定，勿自行开工）**：
① 规则表补全繁体异形字（200+ 处，易漏，维护面翻倍）
② ITN 前繁→简归一、处理完再转回（架构改动，但一处收口）
③ 把简繁转换整体挪到 ITN 之后（改 pipeline 顺序，影响面最大）

🔴 **不进 v0.9.1**：与本批四单无关，属独立系统性缺口，需单独评估 + 端测。
220 修的 `這/麼` 仍然有效且必要 —— `年/刻` 繁简同形，`這一年`/`這一刻` 是真实可达的 bug。

**WORDBOOK-AUTOLEARN-OBS-218** · 自动学习可观测性（Gavin 端测「没看到效果」的根因）

| 项 | 内容 |
| --- | --- |
| 结论 | **链路没坏**（`main.rs:6854-6866` → `wordbook::learn_correction`），是四道闸门叠加导致看不见 |
| 四道闸门 | ① 落库 source=`"system"`（`wordbook/mod.rs:176`），要在**系统词库**页看，不是用户词库 ② 阈值默认 2（`config/mod.rs:12`），同词改够 2 次才提升 ③ 🔴 **两条关键 info 日志在非 debug 模式被 `LevelFilter::Warn` 全过滤**（`main.rs:8357`）④ 只有在线流式路径有 `last_streaming_text`，本地模型不可达 |
| Gavin 拍板 | 自动学的词**维持进 system 不改**（2026-09-17）|
| 主控拍板 | 阈值**不往界面露**（DEC-031 零配置原则，`feedback_single_switch_principle`）|
| 本单只做 | 闸门③：`wordbook/mod.rs:161` 与 `:170` 两条 `log::info!` 提到 `log::warn!` 并加 `[AUTOLEARN]` 前缀，让正常运行时可见可 grep。**禁止**把 release 全局日志级别提到 Info（Gavin 明确要求过压低实时日志 IO）|
| 影响文件 | `src/wordbook/mod.rs` —— 与 214/215/216 零重叠 |
| 验收 | release 模式跑一次编辑提交，日志能 grep 到 `[AUTOLEARN]`；候选未达阈值和达阈值两种情况各出一条 |

### PROMPT-OPT-204 · `f3_lists` 精简（DEC-059 管辖，最高风险）

| 项 | 内容 |
| --- | --- |
| 影响文件 | `src/llm/mod.rs` 的 `f3_rules_text` / `INLINE_SEPARATOR_RULES(_NO_PUNCT)` |
| 现状 | 多行场景 13,789 字符 = doc 场景提示词总量 55%；单行场景 4,094 |
| 风险 | **最高** —— F3 + DEC-060 核心规则，直接决定列表化行为，刚被端测打过 |
| 硬要求 | 🔴 DEC-059：**真实 API A/B 实证**，不接受静态论证。已有 `PROMPT-LAB` 的 `lab_ab` 可直接跑（比开单时条件好） |
| 待定 | A/B 谁来跑：主控用生产 key 跑（花钱）／ Gavin 端测充当 A/B |
| 详情 | `todo-archive.md` §「提示词优化三单」（202 已完成、203 已撤单） |

### TRANS-LANG-UI-213 · 翻译目标语言补设置界面开关

**由来**：Gavin 定原则「程序内部加载处理的逻辑不暴露给用户，防止误操作导致处理异常」，
README 里教用户改 `config.toml` 的段落已全部删除。但 `translation.target_language`
**没有界面控件**（`grep ui/src` 确认，仅 `HotkeySettings.tsx:145` 作默认值透传），
删掉说明后用户就无从切换方向 —— **README 已写「切换选项正在补进设置界面」，这是对用户的承诺，必须兑现**。

| 项 | 内容 |
| --- | --- |
| 影响文件 | `ui/src/pages/HotkeySettings.tsx`（翻译热键区加下拉）+ `ui/src/i18n/{en,zh-Hans,zh-Hant}.ts` 三份补 key |
| 改动 | 中文 / 英文 二选一下拉，写回 `translation.target_language`；Rust 侧字段已存在，**后端零改动** |
| 注意 | 🔴 三份 locale 必须同时补，参照 `I18N-HANT-GAP-001`（繁中曾漏一个 key）|
| 验收 | 切换后 `config.toml` 落盘正确 + 重启保持 + 三语言界面文案齐全 |

### TEST-SYNC-194 · FIX-192 顺序护栏

钉死「先扩窗再建 EDIT」的顺序，防以后被改回去。按分工派**非作者的空闲 coder**（不是 coder-1）。

### BUILD-VERIFY-219 · 出包核验清单成文 + 加第八项（🔄 已派 tester-1，2026-09-17）

现在只核 exe。`[TOML-ALL-NUL-001]` 证明数据文件会整文件变 NUL 且**只比大小检不出来**，
必须与根目录副本做 hash 比对。落点：`build-test-guide.md` 出包核验清单。

🔴 **派发时新发现**：`build-test-guide.md` 全文 657 行里**根本没有出包核验清单这一节** ——
历次「七项核验全 PASS」只活在 `handoffs` 条目里，是口头实践，无成文判据。
故本单范围扩为两步：**A. 从 `handoffs-archive.md` 取证还原七项并落成正式清单**（不许凭印象写）；
**B. 新增第八项**。第八项 ≠ 重复 `build-test-guide.md:72` 的「toml 三副本同步」——
那是**构建步骤**（我做了这个动作），第八项是**出包核验判据**（我验证了结果对），两者不能互顶。

### ESC-178 · H1 机制定案

消息路由层为何不响应仍未查明，现在的轮询旁路是绕行不是根治。需一次带 debug.log 的端测。

### OVERLAY-149-PROBE · 探针删除

10~11 处临时探针，等圆角机制定案后清理。

### VERBOSE-195 遗留（需 Rust 改动，等 Worker 额度）

| # | 项 | 说明 |
| --- | --- | --- |
| 1 | **翻译版 L0-1(A) 保真底线** | 翻译路径目前只有 `UNIT_SYMBOL_PROTECTION_TRANSLATE` 护数字单位，否定/情态/限定词/主命题全裸奔。照该常量先例做翻译版 L0-1(A) |
| 2 | CONDENSE 条款重复 7 份 | 运行时只注入 1 份，token 成本不变，代价是维护性（改一处要改 7 处）。根治 = 代码侧按 kind 统一注入，或 toml 加共享段 |
| 3 | pi desktop exe 名待坐实 | 四条已按联网取证录入，非本机核实，端测时看 `Scene context: app_exe=` 日志坐实 |

### 发布遗留（RELEASE-210）

| # | 项 | 说明 |
| --- | --- | --- |
| 1 | ~~README 正文更新~~ | ✅ **已完成 README-212**（2026-09-10）：中英 README 重写为叙事式产品页，英文名定为 **FlashVoice Input** |
| 1b | 🔴 **命名不一致待定** | 产品英文名已改 FlashVoice Input，但这些**还是旧名**：仓库名 `Feiyin-IME`、产物名 `feiyin-ime.exe` / `feiyin-ime-ui.exe`、GitHub Release 标题「飞音智能语音输入 v0.9.0」、安装包脚本 `voice-ime.iss`。**改产物名会动构建链和用户升级路径，等 Gavin 决定改到哪一层** |
| 2 | Release 挂安装包 | 本次 Release 未挂二进制。`voice-ime.iss` 是 Inno Setup 脚本但未构建。需对外分发时：出 release 包 → 构建 setup.exe → upload asset |
| 3 | `Publish/models` 缺 performance 模型 | `sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17` 不在 `Publish/models/`（目录 mtime 09-10 21:27），而 config 是 `asr_model="performance"` ⇒ 跑 Publish 版本本地识别会直接报错。根目录与 `target/release/` 下都还在。**等 Gavin 确认是否手动删除**，确认后从 `target/release/models/` 拷回 |

---

## 📋 待派发 / 待排期

| 编号 | 一句话 | 前置 / 风险 | 详情（`todo-archive.md`） |
| --- | --- | --- | --- |
| ITN-IDIOM-COVER-032 | 六个量级单位字成语（十全十美 / 千方百计等）在 `itn.rs` 零覆盖，先测现状再定改不改 | 与任何 ITN 任务互斥（同 `src/itn.rs`）。**取证前不动代码** | §「ITN-IDIOM-COVER-032」 |
| LLM-USAGE-OBSERVABILITY-001 | 补解析 LLM 响应里的缓存 usage 字段，让提示词成本可量化 | 无前置，改动极小。**实施前查证 DeepSeek 官方字段名**，跨 endpoint 用 `Option` 容错 | §「LLM-USAGE-OBSERVABILITY-001」 |
| SCENE-FORMAT-DIMS-001 | 把 `multiline_safe` 单 bool 拆成「换行 / 列表 / 标题 / 表格」多维度 | 动的是刚稳定的 prompt 构建链。**迁移成本随场景块增长，越晚越贵** | §「SCENE-FORMAT-DIMS-001」 |
| SCENE-FOCUS-PROBE-001 | 用 UIA / AX 探测焦点控件类型，解决「同一窗口内 Enter 语义不同」 | 🔴 **PoC 门禁**：Chrome / Electron 拿不拿得到无障碍信息是头号风险（chrome.exe 占真实听写量 36%） | §「SCENE-FOCUS-PROBE-001」 |
| ITN-V2-P6 | 75 个能产语法族从保护表移出交文法处理（治「五毛钱保持、三毛钱变 3毛钱」） | Gavin 2026-08-04 拍板暂缓，等端测反馈。分类扫描已完成 | §「ITN-V2-P6」 |
| I18N-HANT-GAP-001 | 繁体中文缺 `voice_asr_model_accuracy` 一个 key | 改动极小，**须顺带核 `getTranslations` 有无兜底**，无兜底则优先级上调 | §「I18N-HANT-GAP-001」 |
| ITN-COLLISION-TYPEB-001 | ITN 碰撞 B 型 | Gavin 2026-07-30：先建待办，以后再动手 | §「ITN-COLLISION-TYPEB-001」 |

---

## ⏸ 待 Gavin 拍板

| 事项 | 一句话 | 详情 |
| --- | --- | --- |
| ITN-FIX-BIGNUM-027 遗留三条 | `一万亿`→`10000亿` 是否改；`十万个为什么`→`10万个为什么` 专名被改写是否开单；`两万五百`→`20500` 备查 | §「ITN-FIX-BIGNUM-027」 |
| RESEARCH-SCENE-COVERAGE-001 | 场景词表扩展研究，三项已拍板落地，剩余争议项待定 | §「RESEARCH-SCENE-COVERAGE-001」 |
| 领域级泛化关键词第二批 | `思维导图` / `白板` / `表格` 是否收进词表 | §「领域级泛化关键词第二批」 |
| 翻译方向是否改双向全自动 | 现方向由 `translation.target_language` 决定；改双向 LLM 路径易改，离线 NLLB 需按方向换模型 | §「等 Gavin 拍板」 |
| FORMAT 保底层（A 方案） | 不开 LLM 的用户要不要规则层语气词去除保底 | §「等 Gavin 拍板」 |
| `qwen3_asr_url` 默认值 | 维持 dashscope 还是留空强制用户配置 | §「等 Gavin 拍板」 |
| TELEGRAM-RESTART-001 | Telegram 通道恢复路线（降级 CLI / 等官方放开 / 手动轮询） | §「未排期任务」 |
| `src/main - 副本.rs` 99KB 残留 | 未入 git 的旧副本，**删除需单独确认**（不可逆操作纪律） | §「待办队列」 |
| api_key 明文存 `config.toml` | 是否处理 | §「待办队列」 |

---

## 已知遗留问题（低优先，非阻塞）

| 编号 | 问题 | 位置 |
| --- | --- | --- |
| ASR-HALLUC-SEGMENT-001 | accuracy 长音频分段中段语义级幻觉，三重兜底全拦不住。**挂起**（accuracy 已从 UI 隐藏，路径不可达）。🔴 未来若重开 accuracy 或引入其他 LLM-decoder 类 ASR，必须同批立项 | `src/transcription/mod.rs` |
| TEST-FIX-002 / 003 | `App.test.tsx` 缺 `@tauri-apps/api/core` mock（2 例 FAIL）；`Wordbook.test.tsx` `getByRole("dialog")` 无 role 报错 | `ui/src/` |
| TECH-DEBT-001 | `parse_version` 主程序与 Tauri 侧实现不一致，prerelease 处理有差异 | `src/version_check/mod.rs` + `src-tauri/src/version_check.rs` |
| ACC-DEGRADE-UI-001 | accuracy 静默降级时 UI 仍显示 accuracy（可观测性缺口） | `src/transcription/mod.rs` |
| MOJIBAKE-COMMENT-001 | `main.rs` 5 处历史 mojibake 注释（`2483 / 2673 / 2689 / 2722 / 2962`），清理须 UTF-8 无 BOM 读写 | `src/main.rs` |

---

## 未排期（有想法，没排期）

| 编号 | 任务 |
| --- | --- |
| WORDBOOK-CORRECTION-UI-001 | 注入后 overlay 纠错入口（Gavin 选定方案 2）：注入完成后 overlay 显示「纠错」按钮 → 编辑 → 入词库。背景：WM_GETTEXT 在现代应用读不回，自动学习路径实际失效 |
| UI-I18N-COMPLETE-001 | UI 硬编码文字补 i18n（App.tsx Loading/Error、Llm.tsx Success/Failed 等 5 处） |
| LLM-KEY-REVEAL-001 | 格式化输出页 API Key 明文查看小按钮 |
| QWEN3-CORPUS-BIAS-001 | 在线模型接入词库偏置（官方 `corpus.text` 通道，max 10K tokens）。两份官方文档记载矛盾，实施前需实测 |
| QWEN3-STREAM-V2 | 在线 ASR 真流式演进 |
| RESEARCH-TEXTCAPTURE-001 | 现代应用文本捕获方案研究 |
| DEC-014 | WebView2 自动安装（Win10 用户） |
| CRASH-EMAIL-001 | crash reporter 邮箱设置（需 SMTP） |
| Phase 3 演进评估 | 场景感知后续方向：UIA 控件信号、浏览器细分词表迭代、内容压缩独立开关、语气适配 LLM 兜底、个性化风格学习。仅记录不排期 |

---

## 🍎 macOS 侧

Phase 4 完整规划见 `collab/research/macos-phase4-plan-001.md`，逐任务状态见 `todo-archive.md` §「macOS 侧 Phase 4 管线实现规划」。

| 项 | 状态 |
| --- | --- |
| A 探针 / A+ 修热键 / C-OVERLAY / C-WIRE / CFGGATE / E-BUNDLE | ✅ 已验收 |
| MACOS-P4-AXINJECT-002（堵 AX 静默丢词） | 🔄 进行中 |
| MACOS-P4-OVERLAY-WIRE-002（抽纯函数使七分支可单测） | 🔄 进行中 |
| B-NEUTRAL / C-HOST / C-TRAY / D-SCENE / D-PERM / D-AUTOLAUNCH / MAC-008~013 | ⏸ 待拍板或待排期 |

**⏳ 阻塞在 Gavin 决策上的 5 项**：阶段 B 归属与 Windows 零回归验证方 ／ 事件宿主选型（建议复议 DEC-015 的 Tauri，改用 winit）／ 四个浮层是否进第一版 ／ Apple Developer 账号 ／ AX 回读是否提前。

**边界**：阶段 B 独占 `src/main.rs`；C 的 HOST 与 TRAY 必须串行；D 的 SCENE / PERM / AUTOLAUNCH 可三路并行，交汇点 `macos/mod.rs` 的 re-export 由主控统一改一次。

---

## 文档更新规则

1. **只保留新产生、进行中、验证失败、待排期或其他待决的任务**
2. **已完成的功能任务立即归档到 `progress.md`**，不在此保留历史
3. **测试同步 / 构建 / 出包任务归入 `CHANGELOG.md`**，不在此详列
4. **新任务产生时立即写入**，不批量补
5. **不列端测跟踪项**（Gavin 自行使用中测试，有问题会重新开单）
6. 🔴 **单条待办不超过 8 行**：背景、取证、方案推演一律写进 `todo-archive.md`，这里只留「是什么 + 前置/风险 + 指针」
7. 🔴 **本文件行数上限 250 行** —— 它每次 session 启动都会被完整读进上下文，超了立刻归档
