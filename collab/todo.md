# 任务列表 · voice-ime

> **本文件只放「还没做的事」。** 已完成的功能 → `progress.md`；任务完成记录 → `CHANGELOG.md`；
> 过程记录、取证细节、历史批次 → `todo-archive.md`。

**🚀 下一包**

|  | Gavin 09-26：预览只保留一个打点机制「每隔 3.5 秒用全部预览文本重打一次」，去掉 1200ms 静默打点；主控补「重打结果末尾终止符不显示」防 349 复发；句尾标点靠精解回灌 | coder-2 | local_stream.rs 预览标点区 | 🔄 已派 |
| --- | --- | --- | --- | --- |
| `TEST-GAP-438-TAILSTART` | 护栏缺口（BUILD-438 消融 438b 未红）：`preview_display` 重打时忽略 `tail_start` 改为整段重打，`fix438_*` 全绿——护栏只直调纯函数、唯一进 `preview_display` 的用例 `engine=None`。主控已读码确认接线正确（不影响本包）。补：带真 CT-Transformer 或可注入打点函数的端到端用例 | 待定（非作者 coder） | local_stream.rs 测试区 | ⏳ 攒批补 |
| `RT-STOP-LATENCY-MULTIWIN` | BUILD-398 #9 遗留：长录音切多窗串行解码，停止时队列压窗 ⇒ 松键到上屏 4.5s（16:44Z 23.85s 录音）；后半「无语音整窗送解」已由 414 修 | 待定 | — | ⏳ 待更多端测数据再定 |

**⏸ 挂起**

| 单号 | 内容 | 负责 | 文件 | 状态 |
| --- | --- | --- | --- | --- |
| `POC-QWEN3-PREFIX-424` | Gavin 灵感：已识别精确文本作 Qwen3 **输出前缀**（assistant `<asr_text>` 后）续写，替代系统提示注入；sherpa 加 per-stream `prefix` 补丁，session wav 离线回放，甲（末尾一截）/乙（整片）两种取法 vs 现状 | coder-1/2 | 仅测试 + sherpa 补丁 | ⏸ **暂不上生产**（Gavin 2026-09-25「先不上」）：甲 CER 小幅改善、官方形态真录音失控、需自维护 sherpa 双平台 fork；补丁 `collab/evidence/424/sherpa-prefix.patch` 留存 |

> 每条待办的「详情」列指向 `todo-archive.md` 里的原始小节，细节一条没丢，别在这里展开。
> 维护规则见文末。

---

## 当前状态（2026-09-26）

| 项 | 状态 |
| --- | --- |
| 版本 | 0.9.3（未动）；最新包 **BUILD-438**（HEAD `2252ce0`，含 438；前一包 BUILD-437 含 436/437），待 Gavin 带 `-debug` 端测；未 push |
| Worker | 09-26 三方就绪（OpenCode `MiMo-V2.6-Flash Free`，无需 `/permissions`）；三方空闲，待派 |
| 文档 | 09-26 归档 handoffs 09-25 共 52 条；已完成功能按版本移入 `progress.md`「v0.9.3（BUILD-398 ~ BUILD-435）」；撤销项与过程原文移入 `todo-archive.md`【归档十】 |

---

## 🔴 待做

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

### ⏸ MODEL-DOWNLOADER-402 · 模型下载器（Gavin 2026-09-24 选「做模型下载器」，方案待确认）

背景：400 核查程序**无任何模型自动下载器**，安装包不带模型（Gavin 定）⇒ 本地三档 / VAD / 标点 / 离线翻译装完不可用。
主控方案草案：内置模型清单（子目录 / 文件 / 大小 / sha256 / 主源 + 镜像）→ 设置界面按功能提示缺失模型、一键下载（进度、断点续传 HTTP Range、sha256 校验、临时目录下完再原子改名）→ 平台中立模块（macOS 共用）。🆕 **Gavin 已定**：托管=「直接用原始出处」（HF / GitHub 原始地址 + hf-mirror 国内镜像兜底，不自建托管；代价：文件不受控，靠 sha256 校验兜）；首次引导=「首次启动弹窗引导」（弹设置界面模型页、默认勾当前档位所需模型、一键下载）。下一步：coder-2 做模型清单调研 `MODEL-MANIFEST-RESEARCH-402A`，再出实施方案。⏸ **09-24 Gavin：「安装脚本的事儿往后再说，现在我都是通过压缩包形式发布的」⇒ 402A 暂停（已查内容落盘 research/model-manifest-402.md）**。

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
