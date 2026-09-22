# 任务列表 · voice-ime

> **本文件只放「还没做的事」。** 已完成的功能 → `progress.md`；任务完成记录 → `CHANGELOG.md`；
> 过程记录、取证细节、历史批次 → `todo-archive.md`。
> 每条待办的「详情」列指向 `todo-archive.md` 里的原始小节，细节一条没丢，别在这里展开。
> 维护规则见文末。

---

## 当前状态（2026-09-22）

| 项 | 状态 |
| --- | --- |
| 版本 | 🆕 **已升 0.9.3**（Gavin 2026-09-22 指示，主控改 5 处：`Cargo.toml:3`/`Cargo.lock:5893`/`src-tauri/Cargo.toml:3`/`src-tauri/Cargo.lock:934`/`tauri.conf.json:9`，全仓复扫无残留）。上一个包仍是 0.9.2 时代的：v0.9.3 批次九已出包：最新包 **BUILD-345**（HEAD `d87b8b4`，含 **P0 中文切片崩溃修复** / **DEC-077 回滚 340+307 三机制** / 失败片流式填补 G / 342 D/OR 等）。**已替换作废的 8758ca66**。✅ **已 push**：`origin/main` = 本地 HEAD `1ec4143`（2026-09-22 主控 `git ls-remote` 核实，工作区 clean）；**未打 tag**（远端最新 tag 仍 `v0.9.1`），打不打由 Gavin 定 |
| 本轮出包史 | BUILD-258 → 267 → 274 → 280（诊断包）→ 285（🛑 作废）→ 287 → 290 → 296 → 302 → 306 → 321 → 328 → 333 → 338 → 341 → 342（带 P0 已作废）→ **345（当前）** |
| 端测待办 | ① 🆕 **BUILD-345 五条**（须 Gavin 真人录音，带 `-debug`）：**1** 🔴 **长句一口气说到底**（中间尽量不停顿）：预览**后半部分**应与最终输出一致；`[LocalRT-DBG-325] action=skipped-hole` **应基本消失**（上一版一次录音 7 片里 6 片是它），请贴 action 分布；**2** 🔴 **不得崩溃**：`target/release/crash.json` 不应新增（P0 修复重点）；**3** 停手不松手 ⇒ 尾字应在 1~3s 内被 accuracy 补上；**4** D 复核：连续说话应有 `silence<800ms` 的派发（改前 47 次最小 800ms）；**5** 其余（重复标点／编辑不被冲／自学习两次+反向／配置界面／首字「你→按」）有观察记一句 ② v0.9.1 四项遗留：overlay 编辑态剥尾标点／Key 输入框 placeholder／繁中 ITN（`三點半→3:30`）／「API 配置」新文案布局 |
| Worker | ✅ 09-22 新 session 三 Worker 全部就绪（OpenCode `deepseek-v4.1-flash`）：coder-1 / tester-1 主动 ACK，coder-2 经 capture-pane 核实存活并已 ACK。三方 inbox 均空，待派发 |
| 文档 | 09-22 归档 handoffs 09-21 共 59 条（626 → 42 行）。DEC-064 两层结构：**新增条目必须 archive 与索引两边都写** |
| 下一步 | ✅ `346` 已交付验收（`701c4d8`）+ ✅ `349` 已交付（coder-2，预览打点只认静默 1200ms）；✅ `347` 已验收；🔄 `PUNCT-FINAL-REDO-350`（coder-1，`main.rs`+`punctuation/mod.rs`）在飞。并行等 Gavin 端测 BUILD-345 五条 |

---

## 🔴 待做

### 🆕 v0.9.3 批次十 · Gavin 2026-09-22 三项（版本已升 0.9.3）

**版本号**：`0.9.2 → 0.9.3`，Gavin 2026-09-22 明确指示。主控已改 5 处：
`Cargo.toml:3` / `Cargo.lock:5893` / `src-tauri/Cargo.toml:3` / `src-tauri/Cargo.lock:934` /
`src-tauri/tauri.conf.json:9`。全仓复扫无 `0.9.2` 残留（`ui/package.json` 是 `0.1.0`，历来独立，未动）。

| 单号 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| `ACC-DISPATCH-SILENCE-ONLY-346` | 切片派发规则改「只判静默 1200ms」，删长度支（原 800ms OR 累计 5s）。🔴 **必须同批解决共享计数器**：`silent_ms`（`local_stream.rs:491`）是唯一计数器，标点路径 `:997` 在 800ms 打点后把它清零 ⇒ 阈值抬到 1200 后静默支**结构上不可达**，不解决则 accuracy 录音中永不派发 | coder-2 | ✅ 已交付（2026-09-22：只判静默 1200ms + acc 专用计数器 `acc_silent_ms` 修共享坑；check 0err / 全量 **1420P/0F**；待 tester-1 出包）|
| `RESEARCH-QWEN3-1.7B-347` | 评估 1.7B 替代现役 0.6B。🔴 门禁已答 | coder-1 | 🔄 ✅ **已交付并验收**：`collab/research/qwen3-asr-1.7b-eval-347.md`（15KB）。结论**有条件可行** —— 官方 k2-fsa 无 1.7B 导出（499 asset 全枚举），但 ModelScope `zengshuishui/Qwen3-ASR-onnx` 的 `model_1.7B/` 与现役 0.6B **同源同脚本**（本地三文件 sha256 与该库逐字节相同），四件套同构、`OfflineQwen3ASRModelConfig` **代码零改动**。代价：+1.32GiB、decoder 权重 2.70× ⇒ CPU 解码约 ×2.7；KV 每 token 与 0.6B **完全相同**（同 28 层/8 KV head/head_dim 128）⇒ 4096 仍成立。🔴 CPU RTF 与中文 CER **均未实测** |
| `PUNCT-PREVIEW-SEMANTIC-349` | **问题①**：预览窗标点打在句中。主控定位（线索非结论，须先取证）：触发是「4s 定时 **或** 静默 800ms」，两条都与语义无关 —— 4s 一到就把半截句子喂 CT-Transformer，模型必在半句末尾补终止符。方向 A（只认 1200ms 静默、删 4s 定时，口径对齐 346，主控倾向）／ B（保留节奏但剥掉落在末尾的标点） | coder-2 | ✅ 已交付（2026-09-22：先证后改，只认静默 1200ms、删 4s 定时与 shadow 400ms 强制；check 0err / 全量 0F；待 tester-1 出包）|
| `PUNCT-FINAL-REDO-350` | **Gavin 新增**：最终 acc 转写完 ⇒ **剥光全段标点 + 整段重打**。推翻 `apply_local_punctuation` 的 `!native_punctuated` 门（`main.rs:10802`）。🔴 剥离必须与 `has_effective_punctuation` **共用同一谓词**，词内嵌豁免照搬（`3.14`/`3:30`/`example.com`/`3.5亿` 不许剥）。🔴 **四档全覆盖**（`run_pipeline_core` 共用，DEC-066 禁管线判据），在线 Qwen3 档标点可能变差 ⇒ 必须端测 | coder-1 | 🔄 已派发 |
| `QUERY-PUNCT-MECHANISM-348` | 查前端流式模型调标点模型的机制 | 主控 | ✅ 已查清并答复 Gavin（结论见下） |

#### 346 的取舍（Gavin 拍板，照做，端测须盯）

删长度支 ⇒ **一口气连说不停顿时录音中不派发**，全压到松手后的尾片。
- ✅ **无 OOM/无界音频风险**：`build_padded_segments`（`vad.rs:233-241`）对 ≥ `SEGMENT_MAX_SECS=20.0` 的片
  硬切成子段，单次喂 accuracy 的音频恒 ≤20s，这条安全网与派发规则无关、独立成立。
- ⚠️ **代价是延迟**：BUILD-321 实测「后端并行派发」是该版最大收益
  （后台共解码 20.0s，用户实际等待均值 **0.63s** / 中位 0.32s）。连说 40s 不停顿 ⇒ 这 40s 全部
  在松手后才开始解码。端测请盯 `[LocalRT-DBG-298] join: tail_wait`。

#### 🎯 本版真实目标（Gavin 2026-09-22 补充，原样引用 —— 所有单据以此对齐）

> 这个版本主要是为了解决：
> 1. 就是流式预览窗口当中打标点不精准的问题，经常会出现标点打在这个句子的中间，掐断了整个句意。
> 2. 就是说我们切片传送 acc模型的时候，因为有一个时长达到 5 秒就切片送，也会导致 acc 模型打标点的时候，标点会打在句子的中间，破坏了语义，句子被打碎了。所以要改成探测到静默达到 1200 毫秒才切片传送后端的 acc 模型，这样就是尽量保证标点，打标点是根据语义的停顿来转写和打标点。

**一句话**：标点要**按语义停顿**打，不许打在句子中间。

- 问题 ② 由 `346` 覆盖（已派发）。🔴 因此 346 的「延迟变长」不是缺点而是**有意换来的代价**，
  验收时**不得**以「等待时间变长」为由打回。
- 问题 ① **346 不覆盖**，另立 `349`（见下），且与 346 **同文件必须串行**。

#### `PUNCT-PREVIEW-SEMANTIC-349` · 预览标点打在句中（问题 ①）

**排期**：⏸ 等 346 交付验收后再派，**同派 coder-2**。
🔴 与 346 同文件（`local_stream.rs`），**绝不可并行**（`feedback_collab_task_file_conflict`）。

**主控初步定位（读码所得，待 debug.log 坐实，不得当已证结论动手）**：
预览打点触发是「① 距上次 ≥ `PUNCT_REFRESH_INTERVAL=4s` **或** ② 静默 ≥ `PUNCT_SILENCE_TRIGGER_MS=800ms` 且有新字」
（`local_stream.rs:987` / `preview_display():254-292`）。两条都与语义无关：
- **①4s 定时**：用户正说到一半，4s 一到就把**半截句子**喂给 CT-Transformer，模型必然在这半句末尾补终止符 ⇒ 句中出现「。」
- **②800ms**：代码自己的注释就写着「800ms 停顿在口语里很密」—— 换气即触发，同样落在句中

**候选方向（派发前须先确认①，再二选一或组合）**：
- **A**：预览打点也改成只认「静默 ≥1200ms」，删 4s 定时 —— 与 346 同一哲学，口径统一。
  代价：长时间不停顿说话时预览长时间是裸文本。
- **B**：保留定时，但打点后**剥掉落在文本末尾的标点**（未说完的句子不给终止符），句内已确定的标点保留。
  代价：要判「末尾标点」，多一层处理。

**已知残留（非 349 引入，记录在案）**：`build_padded_segments` 对 ≥ `SEGMENT_MAX_SECS=20.0` 的片硬切，
若用户 20s 内无 1200ms 停顿，仍会在 20s 处切在句中 ⇒ 问题 ② 低频复现。**本批不修，先观察端测频次**。

#### 348 结论 · 本地流式档的标点机制（主控已读实际运行代码取证）

**一句话**：预览与最终输出是**两个独立阶段**，共用同一个 CT-Transformer 引擎，但触发判据不同 ——
预览按「静默 ≥1200ms + 有新内容」打（`local_stream.rs` / `preview_display()`；🔴 **349 起删除 4s 定时与 shadow 400ms 强制**，见 `PUNCT-PREVIEW-SEMANTIC-349`）；
最终输出**先验文本里实际有没有标点**（`has_effective_punctuation`，DEC-047 口径），
已有就**跳过引擎**，没有才跑（`main.rs:9551` / `:8421`）。
🔴 全文（含性能实测 0.15ms/字、增量缓存与 344 P0 崩溃的关系、三层阈值为何不能混用）
见 `todo-archive.md` §「【归档四】QUERY-PUNCT-MECHANISM-348」。

### ⏸ v0.9.3 收尾三件 · 等 Gavin 拍板

| 事项 | 说明 |
| --- | --- |
| `.512.bak` 删不删 | 两处共 **1.12 GB**，是 KV 1024 的回滚路径；端测确认 1024 无问题后可删。属不可逆操作，须单独成轮确认 |
| `git push` | 🔴 **本条已作废（2026-09-22 主控取证订正，属 `[DOC-STATE-DRIFT-001]`）**：`git ls-remote origin refs/heads/main` = `1ec4143` = 本地 HEAD ⇒ **早已全部推送，无未推 commit**。原记「56 个未推、最新 `ad74b64`」为过期记录。剩余待 Gavin 定的只有**打不打 tag** |
| 版本号 | 仍 0.9.2，但本轮动了数据库 schema + 模型文件 + 新增本地流式档位，与最初的 0.9.2 已非同物，是否给新号由 Gavin 定 |

### 🔄 v0.9.3 本地流式实时模型 · 剩余风险项

| 项 | 内容 |
| --- | --- |
| 交叉场景未验 | **本地流式 + 开启翻译**：代码上通（复用同一 `run_pipeline_core`，无 `LocalRealtime` 特判绕过翻译），状态文案也统一为「识别处理中...」，但**从未实际测过**，已列入端测清单 |
| 贯穿约束 | 🔴 Gavin 2026-09-20 强调 **千万不能改坏现有管线**。238/238-B 是零行为变更提取，唯一硬证据是全量回归与基线逐位吻合，**每轮必跑** |
| 端测清单 | `collab/e2e-checklist-local-realtime.md`（10 项） |

**259~290 全部已交付并出包** → 单号明细见 `CHANGELOG.md`，过程取证见 `todo-archive.md` §「v0.9.3 批次（2026-09-20/21）」。
`ASR-DROP-234` 已由 `FIX-ASR-DROP-288` 修复并随 BUILD-290 出包（`-debug` 静置 150s，`[ASR-DROP]` 0 条）。

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
