# 任务列表 · voice-ime

> **本文件只放「还没做的事」。** 已完成的功能 → `progress.md`；任务完成记录 → `CHANGELOG.md`；
> 过程记录、取证细节、历史批次 → `todo-archive.md`。
> 每条待办的「详情」列指向 `todo-archive.md` 里的原始小节，细节一条没丢，别在这里展开。
> 维护规则见文末。

---

## 当前状态（2026-09-20）

| 项 | 状态 |
| --- | --- |
| 版本 | **v0.9.2 已出包**（`BUILD-232`，八项核验逐项全 PASS），本地已 commit 至 `e557892`。🔴 **未 push、未打 tag、未发 Release**，等 Gavin 明示 |
| 端测待办 | ① **v0.9.2 两轴 2×2**（`asr_online_max_sentence_silence` 800/2000 × `asr_online_semantic_punctuation_enabled` false/true）🔴 须手改 `Publish/config.toml:29` 的 800 那一行 + **重启进程**（热重载不含二者）② v0.9.1 四项：overlay 编辑态剥尾标点 ／ Key 输入框 placeholder ／ 繁中 ITN（`三點半→3:30`）／「API 配置」新文案布局 |
| Worker | ✅ 09-20 新 session 三 Worker 全部就绪（`commandgo/deepseek-v4.1-flash`），coder-1 / coder-2 / tester-1 均已 ACK |
| 文档 | 09-20 已归档 handoffs 26 条（288 → 7 行）。DEC-064 两层结构；**新增条目必须 archive 与索引两边都写** |
| 下一步 | 候选（三 Worker 空闲，等 Gavin 点单）：`ITN-FIX-LIANGDIAN-223`（待取证定位）、`ASR-DROP-234`（待取证定性）、`TRANS-LANG-UI-213`（纯前端，与前两者零重叠）、`TEST-SYNC-194`；`PROMPT-OPT-204` 仍缺 A/B 授权 |

---

## 🔴 待做

### ✅ v0.9.1 批次已全部完成并出包（2026-09-17，销项于 09-20）

`214` 句尾标点开关 ／ `215` 在线 ASR 跳主通道 ITN ／ `216`「那一刻」文法闸门 ／ `218` `[AUTOLEARN]` 日志
／ `220` 闸门补繁中异形字 ／ `221` ITN 繁体影子串（**路线②已实施，非「待拍板」**）／ `222` 规则表 key 加载期归一
／ `225` Key 标签+placeholder ／ `226`「API 配置」文案 ／ `217` 版本号三处 ／ `219` 出包核验八项清单成文。
逐条取证见 `CHANGELOG.md` 的 `BUILD-228` 条目与 `handoffs-archive.md` 的 09-17 段（26 条）。
**未销项的只剩 `ITN-FIX-LIANGDIAN-223`**（见下），它当时未进批。

### ✅ v0.9.2 批次已全部完成并出包（2026-09-20）

`ASR-SEG-229` 在线 ASR 两轴参数化（`asr_online_max_sentence_silence` 800→2000 ＋ 隐藏字段
`asr_online_semantic_punctuation_enabled`）／ `VER-BUMP-230` 版本 0.9.1→0.9.2 ／ `TEST-SYNC-229` 三条交叉护栏
／ `FIX-TESTENV-231` TestEnv 每实例唯一目录（治 `config::tests` 并行竞态）／ `TEST-EXEC-231` 回归全绿
／ `BUILD-232` 出包八项核验逐项 PASS。取证见 `CHANGELOG.md` 与 `handoffs.md` 09-20 段。

🔴 **只剩 Plan B 悬而未决**：若 Gavin 2×2 端测证明 L1（提阈值 ＋ 语义断句）治不住碎句，才启用
L2「剥服务端句尾标点、整段交本地 CT-Transformer 重打」（PUNCT-GOVERNANCE-030 定源头优先）。
另一未知风险只能靠端测暴露：语义模式下服务端若憋着批量发中间结果 → overlay 预览变顿。

### 🆕 ASR-DROP-234 · 冒烟日志逐帧 `[ASR-DROP]` WARN（`BUILD-232` 发现，主控定下批修）

| 项 | 内容 |
| --- | --- |
| 现象 | 冒烟 stdout 出现 `[ASR-DROP]` WARN 刷屏 |
| 位置 | `src/audio/mod.rs:492-495` |
| 定性 | 非本批引入、已按「非本批缺陷不拦出包」放行；**尚未定性是真丢帧还是日志级别用错**，取证先行不动代码 |
| 影响文件 | `src/audio/mod.rs`（与 v0.9.2 批次零重叠，可独立派发） |

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
| 1 | ~~README 正文更新~~ | ✅ **已完成 README-212**（2026-09-10）：中英 README 重写为叙事式产品页，英文名定为 **FlashVoice Input** |
| 1b | 🔴 **命名不一致待定** | 产品英文名已改 FlashVoice Input，但这些**还是旧名**：仓库名 `Feiyin-IME`、产物名 `feiyin-ime.exe` / `feiyin-ime-ui.exe`、GitHub Release 标题「飞音智能语音输入 v0.9.0」、安装包脚本 `voice-ime.iss`。**改产物名会动构建链和用户升级路径，等 Gavin 决定改到哪一层** |
| 2 | Release 挂安装包 | 本次 Release 未挂二进制。`voice-ime.iss` 是 Inno Setup 脚本但未构建。需对外分发时：出 release 包 → 构建 setup.exe → upload asset |
| 3 | ~~`Publish/models` 缺 performance 模型~~ | ✅ **已销项（09-20 主控文件系统复核）**：`Publish/models/` 下五个模型目录齐全，含 `sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17`。与 tester-1 在 `BUILD-228` 的上报一致，原条目系当时取证时点问题 |

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
| AUTOLEARN-REACH-001 | `218` 只修了闸门③（日志可见）。剩余三道按设计保留：落库 source=`system`（Gavin 09-17 拍板不改）／阈值 2 不上界面（DEC-031）／🔴 **只有在线流式路径有 `last_streaming_text`，本地模型路径自动学习不可达** —— 第三条是真缺口，若 Gavin 端测再报「没效果」先查是不是用的本地模型 | `src/wordbook/mod.rs` + `src/main.rs` |

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
