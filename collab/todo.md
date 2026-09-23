# 任务列表 · voice-ime

> **本文件只放「还没做的事」。** 已完成的功能 → `progress.md`；任务完成记录 → `CHANGELOG.md`；
> 过程记录、取证细节、历史批次 → `todo-archive.md`。
> 每条待办的「详情」列指向 `todo-archive.md` 里的原始小节，细节一条没丢，别在这里展开。
> 维护规则见文末。

---

## 当前状态（2026-09-22）

| 项 | 状态 |
| --- | --- |
| 版本 | 🆕 **已升 0.9.3**（Gavin 2026-09-22 指示，主控改 5 处：`Cargo.toml:3`/`Cargo.lock:5893`/`src-tauri/Cargo.toml:3`/`src-tauri/Cargo.lock:934`/`tauri.conf.json:9`，全仓复扫无残留）。上一个包仍是 0.9.2 时代的：🆕 **批次十一 BUILD-347 已出包**（HEAD `ae166e6`，含 354/356/359/363-364/365/367，八项 + 三特殊点 PASS）；🆕 **批次十 BUILD-346 已出包**（HEAD `f335bd8`，含 346/349/350/352/351/e829c67，八项 PASS）；v0.9.3 批次九最新包 **BUILD-345**（HEAD `d87b8b4`，含 **P0 中文切片崩溃修复** / **DEC-077 回滚 340+307 三机制** / 失败片流式填补 G / 342 D/OR 等）。**已替换作废的 8758ca66**。✅ **已 push**：`origin/main` = 本地 HEAD `1ec4143`（2026-09-22 主控 `git ls-remote` 核实，工作区 clean）；**未打 tag**（远端最新 tag 仍 `v0.9.1`），打不打由 Gavin 定 |
| 本轮出包史 | BUILD-258 → 267 → 274 → 280（诊断包）→ 285（🛑 作废）→ 287 → 290 → 296 → 302 → 306 → 321 → 328 → 333 → 338 → 341 → 342（带 P0 已作废）→ 345 → 346 → 347（🛑 吃字作废）→ 348（🛑 丢字作废）→ 349（🛑 前缀漏出+重复句吃字，作废）→ 373（🛑 词条回显+预览刷不进+出字延迟，作废）→ 379 → 380 → 385 → **387（当前）** |
| 端测待办 | ⓪ 🆕 **BUILD-387**（386/387 合包；须 Gavin 真人录音，带 `-debug`）：🔴 **Step1 强杀了输入法进程，请先重启后端测**；重点：**1** 中途末片延后组窗 + 松键短尾合并重解（386，防吃字/尾巴丢）；**2** 预览不再闪回更短（386，保留流式尾巴）；**3** 念词表/标签/空输出判无效后不带注入重解（387，防回显/坍塌）；**4** 近场门 300ms 平滑音量（387，防误挡）；**5** 吃字/重复字；**6** 长录音 300s；**7** 不新增 `crash.json` ① 🆕 **BUILD-385**（381/382/384/385 合包；须 Gavin 真人录音，带 `-debug`）：🔴 **Step1 强杀了输入法进程，请先重启后端测**；重点：**1** 静默判定改 VAD（384，停顿切片更贴语义）；**2** 近场门挡背景人声（385，录音人优先）；**3** 松键立即「识别处理中」不闪回 + 回灌立即刷新（382）；**4** 吃字/重复字；**5** 长录音 300s；**6** 不新增 `crash.json` ① 🆕 **BUILD-380**（替换 379；须 Gavin 真人录音，带 `-debug`）：🔴 **Step1 强杀了输入法进程，请先重启后端测**；重点：**1** 预览停顿即刷（380-A）；**2** 出字延迟，看 `[LocalRT-DBG-380] hook_to_controller_ms`/`stop_to_inject_ms`（380-B）；**3** 长录音 300s；**4** 不新增 `crash.json`；**5** 吃字/重复字；其余沿用 379 重点 ① 🆕 **BUILD-379**（替换作废的 373；须 Gavin 真人录音，带 `-debug`）：🔴 **Step1 强杀了输入法进程，请先重启后端测**；重点：**1** 词条回显不再漏进正文（374）；**2** 预览能刷进、解码坍塌改善（375）；**3** 英文指令句/Context 散文不再被模型续写（377）；**4** 出字延迟；**5** 吃字/重复字；**6** 长录音 300s；**7** 不新增 `target/release/crash.json` ① 🆕 **BUILD-373**（替换作废的 349；须 Gavin 真人录音，带 `-debug`）：🔴 **Step1 强杀了输入法进程，请先重启后端测**；重点沿用 349 十一条 —— **1** 吃字/重复字复测（>4 句、夹一句 8~10s 长句造零重叠）；**2** 连录两次互不污染；**3** 长录音 2~3 分钟（验 300s 上限）；**4** 语种前缀不再漏进正文；其余（松手等待/前文改对/接缝重复/标点语义/1.7B 质量/梅开二度·一度/不新增 crash.json）① 🆕 **BUILD-349 十一条**（重出包；须 Gavin 真人录音，带 `-debug`）：**1** 🔴🔴 **吃字复测（本次重出唯一原因）**=说 **>4 句**让窗口滑动多次、其中**故意夹一句 8~10s 不停顿**造零重叠，检查最终文本有无**整段消失**；**2** 🔴 **连录两次互不污染**（Gavin 点名：第二次不得含第一次内容、预览从空白起）；**3-11** 松手等待时间／前文改对／接缝重复／标点语义／1.7B 质量／梅开二度·一度／长录音 2~3 分钟（验 300s）／不新增 crash.json／重启输入法 ① 🆕 **BUILD-348 十条**（重出包；须 Gavin 真人录音，带 `-debug`）：**0** 🔴🔴 **置顶·唯一重出原因=吃字/重复字重点复测**（说 >4 句让窗口滑动多次，检查最终文本**有没有整段消失**）；**1-9** 沿用 BUILD-347 九条（松手等待时间／预览修正质量／接缝重复／标点语义／1.7B 质量／梅开二度·一度／长录音 180s／不新增 crash.json／重启输入法）① 🆕 **BUILD-347 九条**（须 Gavin 真人录音，带 `-debug`；本批行为大改）：**1** 🔴 松手后等待时间（核心收益，30s/60s 应立即出最终文本，旧版等 ~12s/~25s）；**2** 🔴 预览修正质量（后文能否改对前文）；**3** 🔴 **吃字/重复字（最大风险，重点观察）**；**4** 接缝重复消失；**5** 标点按语义停顿；**6** 1.7B 转写质量；**7** 「梅开二度」「他一度以为」不再误转；**8** 长录音近 180s + 不新增 crash.json；**9** ⚠️ 出包强杀输入法，完成后重启 ① 🆕 **BUILD-346 六条**（须 Gavin 真人录音，带 `-debug`）：**1** 🔴 核心=**预览标点不再打在句中**；**2** ⚠️ 已知代价：一口气长不停顿说话 ⇒ 预览持续裸文本无标点，**到期 1200ms 停顿才打**，请明确表态能否接受；**3** 350 最终输出标点更贴语义、**无 `。。` 重复**；**4** 🔴 在线 realtime / 本地 performance **两档标点须与上版完全一致**；**5** 346 `[LocalRT-DBG-298] seg dispatch` 的 `silence=` **恒 ≥1200ms**，不再按时长触发；**6** 不得新增 `target/release/crash.json` ② 🆕 **BUILD-345 五条**（须 Gavin 真人录音，带 `-debug`）：**1** 🔴 **长句一口气说到底**（中间尽量不停顿）：预览**后半部分**应与最终输出一致；`[LocalRT-DBG-325] action=skipped-hole` **应基本消失**（上一版一次录音 7 片里 6 片是它），请贴 action 分布；**2** 🔴 **不得崩溃**：`target/release/crash.json` 不应新增（P0 修复重点）；**3** 停手不松手 ⇒ 尾字应在 1~3s 内被 accuracy 补上；**4** D 复核：连续说话应有 `silence<800ms` 的派发（改前 47 次最小 800ms）；**5** 其余（重复标点／编辑不被冲／自学习两次+反向／配置界面／首字「你→按」）有观察记一句 ② v0.9.1 四项遗留：overlay 编辑态剥尾标点／Key 输入框 placeholder／繁中 ITN（`三點半→3:30`）／「API 配置」新文案布局 |
| Worker | ✅ 09-23 新 session 三 Worker 全部就绪（OpenCode `deepseek-v4.1-flash`）：coder-1 / coder-2 主动 ACK；tester-1 上下文注入停在输入框未提交，主控补 Enter 后 ACK。三方 inbox 均空，待派发 |
| 文档 | 09-23 归档 handoffs 09-22 共 31 条（404 → 48 行）；todo 移出 371 + 批次十两节至 `todo-archive.md`【归档五】（352 → 247 行）。DEC-064 两层结构：**新增条目必须 archive 与索引两边都写** |
| 下一步 | BUILD-379 已出包（HEAD `e164ec0`），等 Gavin 真人录音端测（见上「端测待办」⓪）；Worker 空闲待派发 |

---

## 🔴 待做

### 🔄 BUILD-387 端测修复（2026-09-23 派发，Gavin 授权开发→测试→出包不再请示）

| 单号 | 内容 | 负责 |
| --- | --- | --- |
| ✅ `FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388` | 解码前 VAD 剪静音（根因：静音多+热词触发 sherpa #3509 类幻觉）；重解统一查有内容+产出率；冷启动坍塌绝对下限。**2026-09-23 阶段一交付 coder-1**：`vad.rs` 只新增 `try_new_for_local_trim`/`speech_ranges` + `trim_to_speech` + 线程级 VAD 缓存 + `has_content`/`output_rate_ok` 下限；新增单测 7+`#[ignore]`1，**契约变更更新 6 条既有期望（不变量未动）**；`fmt --check` EXIT0、`check --all-targets` 0 error、warnings **97/88**=基线、全量 **1621P/0F/35I**。**待 tester-1 阶段四回归 + 出包 + 端测** | coder-1（`transcription/mod.rs`、`vad.rs` 只新增） |
| `FIX-NEARFIELD-BY-SEGMENT-AND-PREVIEW-389` | 近场门改按 VAD 整句判定（段峰值 ≥0.3×level 即整句确认）；多片派发部分窗按时长比例估流式字数，防预览回退 | coder-2（`local_stream.rs`、`main.rs`） |

### ✅ BUILD-387 已出包（2026-09-23 17:10，HEAD `3646b4d`）· 待 Gavin 端测

合包 386（中途末片延后组窗 / 松键短尾 <3s 合窗重解 / 预览回灌保留流式尾巴 / 失败窗流式兜底）+ 387（念词表·`<标签>`·空输出判无效后不带注入重解 / 近场门 300ms 平滑音量）+ 主控验收补 2 处。回归 1620P/0F/34I。
**端测重点**：①不再出现「维生素b12」等词条 ②结尾不截断、无 `<location>` ③说话中预览不回退变短 ④近场门不误挡自己（`[LocalRT-DBG-385] nearfield summary` 看 gated 比例）。明细见 `todo-archive.md`【归档八】。

### 🧊 待观察 · FORCED-ALIGN-372（设想，**未立项**，等 371 端测结果）

Gavin 2026-09-22 原话：
> 能不能借助第三方工具，或者说再借助一个小模型来干这个事呢？目的就是能够针对 acc 模型的片段和输出文本，建立很精确的位置对应关系、映射关系。
> 你先把讨论的用单独小模型做精确对齐的设想方案记下来，我先用目前的机制端侧看效果，如果确实有问题，再进一步扩展实施设想方案

**要解决的**：滑窗合并时「切片 → 文本位置」目前是**估算**（层② 时长比率），不准 ⇒ 落兜底 ⇒ **某句重复**。
（丢字已由层① 硬约束堵死，不是本条要解决的。）

**设想**：小模型出「带时间戳的文本」→ 边界时间查到小模型的字位置 → 两份转写做**全局对齐**映射到 acc 文本位置。
关键是**全局对齐两端都锚死、无未知偏移** ⇒ 周期性内容不构成歧义（现算法正是栽在"未知偏移 + 周期性"）。

**立项前必验（15 分钟 PoC）**：本地 `sherpa-onnx-sense-voice-funasr-nano-int8` 在 sherpa 1.13.8 里**填不填 timestamps**。
整个方案押在这一个事实上；不填则退路是离线 paraformer（有 predictor，天然出 token 时长）。

**决策顺序**：371 出包 → Gavin 端测 → 重复不碍眼 ⇒ **不做**；碍眼 ⇒ 先跑 PoC 验时间戳 ⇒ 验通再立项。
**全文（含已排除的 5 条路及理由、替代取舍、实测数据）**：`collab/research/forced-alignment-372.md`

---

### ⏸ v0.9.3 收尾三件 · 等 Gavin 拍板

| 事项 | 说明 |
| --- | --- |
| `.512.bak` 删不删 | 两处共 **1.12 GB**，是 KV 1024 的回滚路径；端测确认 1024 无问题后可删。属不可逆操作，须单独成轮确认 |

### 🔄 v0.9.3 本地流式实时模型 · 剩余风险项

| 项 | 内容 |
| --- | --- |
| 交叉场景未验 | **本地流式 + 开启翻译**：代码上通（复用同一 `run_pipeline_core`，无 `LocalRealtime` 特判绕过翻译），状态文案也统一为「识别处理中...」，但**从未实际测过**，已列入端测清单 |
| 贯穿约束 | 🔴 Gavin 2026-09-20 强调 **千万不能改坏现有管线**。238/238-B 是零行为变更提取，唯一硬证据是全量回归与基线逐位吻合，**每轮必跑** |
| 端测清单 | `collab/e2e-checklist-local-realtime.md`（10 项） |

### ✅ v0.9.1 / v0.9.2 两批已出包并销项 → 全文见 `todo-archive.md` 末尾

### 🔴 ITN-FIX-LIANGDIAN-223 · `这两点一个都不能少` → `这2.1个都不能少`（Gavin 2026-09-17 端测报）

| 项 | 内容 |
| --- | --- |
| 复现 | 「这**两点一**个都不能少」→「这**2.1**个都不能少」；「这两点**一点**都不能少」同病 |
| 语义 | 「两点」= 两个要点（量词），「一个都不能少」是独立短语。ITN 把 `两点一` 当成了小数 2.1 |
| 🔴 取证先行 | 主控初查**未定位**：乙型 `try_parse_implicit_decimal`（`itn.rs:1607`）第 1624 行已有护栏 —— `date_suffixes` 命中且非「度」即 `return None`，而「点」在 `date_time.triggers.suffix` 里，**理论上乙型不该接手**。所以真凶可能是甲型/丙型/`parse_cn_number` 的小数点处理，**必须实测定位，禁止照「乙型越界」这个假设动手** |
| 同族 | `[ITN-LOCAL-RULE-OVERREACH-001]`（局部规则在更长上下文越界）。护栏必须配边界外用例 |
| 🔴 边界外红线 | 真小数不得被误挡：`两点一五`→2.15、`三点五`、`两点一度`（度是温度单位兼 date_suffix）、`下午两点一刻` |
| 影响文件 | `src/itn.rs`（与 221/222 同文件，**必须串行**） |

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
| 1b | 🔴 **命名不一致待定** | 产品英文名已改 FlashVoice Input，但这些**还是旧名**：仓库名 `Feiyin-IME`、产物名 `feiyin-ime.exe` / `feiyin-ime-ui.exe`、GitHub Release 标题「飞音智能语音输入 v0.9.0」、安装包脚本 `voice-ime.iss`。**改产物名会动构建链和用户升级路径，等 Gavin 决定改到哪一层** |
| 2 | Release 挂安装包 | 本次 Release 未挂二进制。`voice-ime.iss` 是 Inno Setup 脚本但未构建。需对外分发时：出 release 包 → 构建 setup.exe → upload asset |

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
| AUTOLEARN-REACH-001 | `218` 只修了闸门③（日志可见）。剩余三道按设计保留：落库 source=`system`（Gavin 09-17 拍板不改）／阈值 2 不上界面（DEC-031）／~~只有在线流式路径有 `last_streaming_text`，本地模型路径自动学习不可达~~ 🔴 **2026-09-21 实测证伪、本条作废**：自学习在本地实时档每次都触发（BUILD-321 日志 4 条）。真因是候选抽取过宽，见 AUTOLEARN-CANDIDATE-327 | `src/wordbook/mod.rs` + `src/main.rs` |

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

### 🆕 LOCAL-RT-ENGINE-239 追加要求 · 切档模型加载提示（Gavin 2026-09-20）

| 项 | 内容 |
| --- | --- |
| 需求 | 切到本地 realtime 档位时要预加载两个模型（约 6s），期间用**现有 overlay 信息提示窗口**显示提示，**短暂显示后自动关闭，不要久留** |
| 现成机制 | `OverlayStatus::Info(String)`（蓝点白字，BUG-119 引入）。照抄 `main.rs:6791-6800` 的 `PipelineEvent::NoSpeech` 发送写法即可 |
| 发送方 | 🔴 **后端**（热重载在 `main.rs:7111` `Transcriber::new`），不是 UI 侧 |
| 文案 | 需补 i18n 新 key，三份 locale |

### 🆕 I18N-DRIFT-HANT-001 · 繁中档位文案与简中语义不一致（低优先）

`ui/src/i18n/zh-Hant.ts:29` `voice_asr_model_performance` = 「效能最優」，
而简中是「本地模型 - 快速」、英文是 `Local Model - Fast`。**繁中丢了「本地模型」语义**。
既有漂移，非本批引入；`LOCAL-RT-UI-240` 明令不许顺手改，单独排期。

---

## ⏸ 挂起中（等外部条件，非阻塞出包）

### GATE-ATTACK-PROBE-335 · 电平闸 attack 是否吃掉首字爆破音 —— ⏸ 挂起

**挂起原因**（Gavin 2026-09-21）：「我现在环境有声音，没办法做到静默啊。」⇒ 有底噪 ⇒ 闸一直开着
⇒ 「冷起振」这个被测条件不成立。**等 Gavin 说环境安静了再跑**。
🔴 **恢复流程、采集脚本、为什么这么设计、已就绪的两个 manual 测试与两个 py 量具、
预备读数（attack 31/326ms，首 30ms 亏欠 42.9/35.9dB）—— 全文见 `todo-archive.md`
§「【归档三】GATE-ATTACK-PROBE-335 全文」**。别凭这 8 行开跑，会漏掉「不可拍手做标记」这类硬约束。
**零成本旁证**（已请 Gavin 日常端测留意）：环境吵 ⇒ 闸开 ⇒ 首字应不易错。
反馈「『你』不再被听成『按』」⇒ 强旁证；「照样出错」⇒ 与闸无关，线索划掉。
### 已知遗留（非本批引入）
- KV `max_total_len=4096` 与 `HOTWORDS_MAX_TOTAL_TOKENS=3000` 耦合，改需同批核算（注释已写明）
- `MAX_WHITESPACE_SEGMENTS=4` 等其余候选闸门未复核（本批只改了长度上限 30→12）
