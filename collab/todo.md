# 任务列表 · voice-ime

> **本文件只放「还没做的事」。** 已完成的功能 → `progress.md`；任务完成记录 → `CHANGELOG.md`；
> 过程记录、取证细节、历史批次 → `todo-archive.md`。
> 每条待办的「详情」列指向 `todo-archive.md` 里的原始小节，细节一条没丢，别在这里展开。
> 维护规则见文末。

---

## 当前状态（2026-09-21）

| 项 | 状态 |
| --- | --- |
| 版本 | 仍 **0.9.2**。v0.9.3 批次九已出包：最新包 **BUILD-345**（HEAD `d87b8b4`，含 **P0 中文切片崩溃修复** / **DEC-077 回滚 340+307 三机制** / 失败片流式填补 G / 342 D/OR 等）。**已替换作废的 8758ca66**。🔴 未 push、未打 tag，等 Gavin 明示 |
| 本轮出包史 | BUILD-258 → 267 → 274 → 280（诊断包）→ 285（🛑 作废）→ 287 → 290 → 296 → 302 → 306 → 321 → 328 → 333 → 338 → 341 → 342（带 P0 已作废）→ **345（当前）** |
| 端测待办 | ① 🆕 **BUILD-345 五条**（须 Gavin 真人录音，带 `-debug`）：**1** 🔴 **长句一口气说到底**（中间尽量不停顿）：预览**后半部分**应与最终输出一致；`[LocalRT-DBG-325] action=skipped-hole` **应基本消失**（上一版一次录音 7 片里 6 片是它），请贴 action 分布；**2** 🔴 **不得崩溃**：`target/release/crash.json` 不应新增（P0 修复重点）；**3** 停手不松手 ⇒ 尾字应在 1~3s 内被 accuracy 补上；**4** D 复核：连续说话应有 `silence<800ms` 的派发（改前 47 次最小 800ms）；**5** 其余（重复标点／编辑不被冲／自学习两次+反向／配置界面／首字「你→按」）有观察记一句 ② v0.9.1 四项遗留：overlay 编辑态剥尾标点／Key 输入框 placeholder／繁中 ITN（`三點半→3:30`）／「API 配置」新文案布局 |
| Worker | ✅ 09-21 新 session 三 Worker 就绪（OpenCode `commandgo/deepseek-v4.1-flash`），coder-1 / coder-2 已 ACK |
| 文档 | 09-21 归档 handoffs 57 条（610 → 89 行）、todo 批次明细入 archive。DEC-064 两层结构：**新增条目必须 archive 与索引两边都写** |
| 下一步 | **等 Gavin 端测 BUILD-345 反馈**（重点：长句 `skipped-hole` 是否基本消失 + 不得新增 `crash.json`）。✅ P0 char-boundary 崩溃已由 `8282203` 修复（单测 + 冒烟双证），原现场待 Gavin 长口述复核。⏸ 335 电平闸挂起 |

---

## 🔴 待做

### 🆕 v0.9.4 批次一 · Gavin 2026-09-21 端测 BUILD-290 反馈（证据 `collab/evidence/20260921-gavin-e2e/debug-1018.log`）

| 单号 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| `FIX-LOCALRT-TAILCHAR-291` | **中间句必丢尾字**。根因已定位：`endpoint` 分支直接 `recognizer.reset()`，**reset 前从未 `input_finished()`** ⇒ 解码器里压着的最后一个 token 被丢弃；全函数唯一一次 flush 在 loop 之后，只救最后一句。日志三句全中（电[影]／空[气]／问[题]）。方案：endpoint 时先 flush 再取结果，下一句改 `create_stream()` 而非 `reset()` | coder-2 | ✅ 已交付（`99799c5`，仅 `local_stream.rs`；实机 `gained` 待 tester-1/Gavin）|
| `FIX-LOCALRT-FIRSTCHAR-293` | **首字爆破音被窗口削掉**（Gavin 2026-09-21 拍板，不等取证先修；新证据「输入**你**」→「输入**按**」）。方案：环形缓冲容量 600→**1000ms**（`audio/mod.rs:873` 一处），本地流式 `drain_pre_roll` 取 1000ms、**在线三档仍取 600ms**（`retain_recent_samples` 保留最近 N ms ⇒ 在线零改变）；分流用现成的 `record_streaming` 第 8 参 `trim_pre_roll_residual`；283 裁剪逻辑一行不改，窗口变长后正好由它兜住混进来的上句尾音 | coder-1 | ✅ 已交付（293-B 修订：容量 1000ms 保留，新增「锚定语音起点」统一裁剪 A/B/C+兜底；实机 `dur=1000ms` 待 tester-1/Gavin）|
| `DIAG-LOCALRT-FIRSTCHAR-292` | **首字取证（293 的验证手段，不撤）**。已排除 283（四次全 `28800→28800`，按设计只在 ≥2 语音段时才裁，本场景 1 段）。出错那次 pre-roll 有语音贴在窗口末尾（`ratio=0.07`）= 「先开口后按键」。**「端」的声学起点在不在这 600ms 里」无法从日志判定** ⇒ 先做 debug-only WAV dump 拿音频 | coder-1 | ✅ 已交付（WAV dump + `[LocalRT-DBG-292]`；待 tester-1/Gavin 实机产出音频）|

🔴 **284/289 影子机制为何没兜住**（别再往影子上打补丁）：① `shadow_done_for_pause` 在静默第一个 400ms 就上锁，之后 main 还在长 ⇒ endpoint 时恒 `shadow < main`，5 次 confirm **全 `used=main`**；② 超 `SHADOW_MAX_AUDIO_SECS=12s` 的句子影子被整个跳过。
✅ **反向价值**：`shadow finalize #6` 给 15 字而同刻 main 只有 14 —— 这正是 `input_finished()` 能多吐一个字的硬证据，291 方案据此成立。


### ⏸ v0.9.3 收尾三件 · 等 Gavin 拍板

| 事项 | 说明 |
| --- | --- |
| `.512.bak` 删不删 | 两处共 **1.12 GB**，是 KV 1024 的回滚路径；端测确认 1024 无问题后可删。属不可逆操作，须单独成轮确认 |
| `git push` | 已积 **56 个未推 commit**（最新 `ad74b64`），一直等 Gavin 明示 |
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

## 2026-09-21 BUILD-321 端测六条 + 日志调优复盘 — 处置台账

| # | 问题 | 结论 / 处置 | 状态 |
| --- | --- | --- | --- |
| ① | ITN：「十分的重要」→「10分的重要」 | 🔴 **不是加保护词条就行**（主控原判已推翻）：`check_protection` 前缀匹配，加裸「十分」会打红 `十分钟→10分钟`(itn.rs:3997)、`三点二十分→3:20`(3896)、`三小时二十分→3:20`(3882)。且 DEC-038 禁保护表承载语法族 ⇒ **必须落成规则** | ✅ **已交付** `5164a10`（itn 259P/0F） |
| ② | tokenizer 报错 | 旧 exe（BUILD-321 构建于修复 `77313e5` 之前）指向已不存在的 nano 目录。**修复已入库，重建即消失** | ⏳ 待出包 |
| ③ | 流式预览尾字丢失 | 🔴 **291（flush 信号）与 307（整句重解码）双双证伪**：`[LocalRT-DBG-307]` 21 条全 `gained=0`。同音频完整重解一字不差 ⇒ **流式 paraformer 本身不输出该字，再重解码无用**。新方向：拿 accuracy 分片结果**回灌预览**（298 并行下分片结果录音中即到，`seg dispatch` 带 `pcm_pos` 可作边界） | ✅ **已交付** `4c78ef3`(325) + `4d49252`(329 持久化) |
| ④ | 编辑态自学习不触发（「指导灵」未入库） | 🔴 **主控原判（`AUTOLEARN-REACH-001`）已实测证伪并作废**：自学习**每次都触发了**（BUILD-321 日志 4 条）。真因 = 候选抽取按字符 diff，ASR 尾部也听错时公共后缀为空、跨度吞到句尾被句末标点校验整体拒 ⇒ 4 次只落 1 次计数，门槛 2 够不着 | ✅ **已交付** `291063c`(327 收窄) + `7d3a3a5`(331 基准) |
| ⑤ | 上下文/词条是否真注入 | ✅ **已证实注入**：`terms_len=73` 每片都有；`acc_len` 录音内 0→12→29→57→79 增量；跨录音轮换 `prev1_len=97` → `prev2_len=97 prev1_len=14`；`cut=0` 从未截断。🔴 但护栏误触 **43%（6/14）** | ✅ **已修（ORCH-CTX-GUARD-FIX-324，主控直改）** |
| ⑥ | Qwen3 的 ITN 是否启用 | **该模型没有这个开关**：`OfflineQwen3ASRModelConfig` 无 `itn` 字段（FunASR 有）。数字规整一直由自研 `src/itn.rs` 承担（DEC-030），换模型前后不变 | ✅ 已答，无需动作 |

### 调优复盘结论（BUILD-321 日志，11 次录音）

| 调优项 | 判定 | 证据 |
| --- | --- | --- |
| 后端并行派发（800ms / 5s） | ✅ **本版最大收益，保留** | 十次录音后台共解码 **20.0s**，用户实际等待均值 **0.63s**、中位 **0.32s**（`join: total_decode` vs `tail_wait`） |
| Endpoint 门限 2.4/2.0/20 | ✅ **有效，值别动** | 11 次录音仅 11 次断句，无碎切 |
| 线程数 8（随核心数） | ⚪ 生效但无可观测收益 | `num_threads=8` 两处确认；流式解码本无拥塞 |
| Shadow 预览 | ⚠️ 日志内 **11:0 从未胜出**（`used=main` 11/11，`shadow_len` 恒 ≤ `main_len`），单次最贵 475ms。**但 Gavin 端测观察到「有时起作用」⇒ 保留不动** | `[LocalRT-DBG-289]` |
| Pre-roll 前导缓冲 | 🔴 **全程空转** | `mean_abs=0.0000 peak=0.0000 speech_frames=0/50` **11/11**；`mode=tail` 11/11。缓冲满 100 chunks 但全是精确零 ⇒ 每次录音头部只贴 200ms 纯静音 | 
| ↳ 处置 | ✅ **已交付** `4c78ef3`：**pre-roll 链路无 bug**，「全零」是 `{:.4}` 显示精度 + 16-bit 落盘量化共同造成的读数假象（实测 nz_ratio=1.000、peak 4e-8~1.3e-6）。已改打 dBFS + nz_ratio。原「修不了就整块摘除」的前提不成立，**该要求已撤销** | |

---

## 2026-09-21 本批收口（BUILD-333 已出包）—— 台账对账

🔴 **本节存在的理由**：上方 09-21 台账表在本批开工后**数小时未同步**，其中 ④ 一直挂着
已被证伪作废的 `AUTOLEARN-REACH-001`。而主控当天正是照着那条过期记录写了错误任务书派发，
靠 Worker 停手反证才拦住。**过期台账比没有台账更危险** —— 已就地订正，本节为最终状态。

### 已出包（BUILD-333，产物 main `703788ed…`，版本 0.9.2 未动）

| 单号 | 内容 | 提交 |
| --- | --- | --- |
| ITN-SHIFEN-323 | 「十分的重要」误转 | `5164a10` |
| ORCH-CTX-GUARD-FIX-324 | 回显护栏比对面收窄 + 埋点修正 + 清理指令恒发 | `7cc7cea` |
| PREROLL-DEAD-322 | pre-roll 根因定案（**无 bug**）+ 诊断精度 dBFS/nz_ratio | `4c78ef3` |
| ACC-PREVIEW-REFLOW-325 | accuracy 分片结果回灌预览（尾字换方向） | `4c78ef3` |
| AUTOLEARN-CANDIDATE-327 | 候选二次收窄（真因） | `291063c` |
| ACC-REFLOW-PERSIST-329 | 回灌持久化 + 学习镜像基准对齐 | `4d49252` |
| UI-LRMODEL-HINT-330 | 删除指向已废弃 nano 的假提示块 | `b20f733` |
| AUTOLEARN-EDIT-SNAPSHOT-331 | 编辑入口快照学习基准 + 词条上限 30→12 | `7d3a3a5` |
| AUTOLEARN-DROP-PATHB-332 | 摘路径B（对齐 DEC-058）+ 清三处死代码 | `b0eca48` |

### 待 Gavin 端测验证（判据见 `collab/outbox/tester-1/result.md`）

| # | 项 | 判据 |
| --- | --- | --- |
| 1 | 尾字（本包核心） | `[LocalRT-DBG-325] streaming render` 的 `has_acc_prefix` / `committed_len` 连续说话期间**不归零** |
| 2 | 预览窗编辑（风险最高） | 录音中编辑不被回灌冲掉；**退出编辑后继续录音仍不被冲**（闩锁） |
| 3 | 自学习 | 编辑改对同一词**两次** ⇒ `(1/2) observed` → `(2/2) promoted` + 词库可见；**反向：不编辑不应出现任何 `[AUTOLEARN]` 行** |
| 4 | 配置界面 | 本地流式档下方**无任何模型文件提示块**（UI 只能目视验） |
| 5 | 先说半句再按热键 | `[LocalRT-DBG-293] mode=onset`（不再恒 tail） |
| 6 | 护栏误触 | `[LocalRT-DBG-320] action=redecode` 频次显著低于改前 **43%（6/14）** |

### 🔴 已知遗留（非本批引入，未动）

| 项 | 说明 |
| --- | --- |
| 电平闸 attack 吃首字爆破音 | coder-1 在 322 发现的下游线索：C920 采集链的电平触发降噪闸有 attack 时间，疑似压掉首字声母，与长期报的「你→按」吻合。**未动手**，待端测第 5 项观察后立单 |
| `maybe_learn_user_edit` 的窄 gap | 已由 331 收口，329 的锚断言已改写语义 |
| KV `max_total_len` 取值 | 代码 4096（与词库预算 3000 耦合，仅在 4096 下成立）。Gavin 早期口径 2048 已在注释中说明耦合关系，**改需同批下调词库预算** |

---

## ⏸ 挂起中（等外部条件，非阻塞出包）

### GATE-ATTACK-PROBE-335 · 电平闸 attack 是否吃掉首字爆破音

**挂起原因**（Gavin 2026-09-21）：「我现在环境有声音，没办法做到静默啊。」
⇒ 环境有底噪 ⇒ 闸一直开着 ⇒ **「冷起振」这个被测条件不成立**，此时采集的数据无效。

**Gavin 口径**：等环境安静了再跑，届时他会告知。

🔴 **恢复流程（别记错，这不是看普通录音日志）**：
1. Gavin 说「可以了」
2. 主控让 coder-1 跑 `cargo test gate335_capture_envelope_manual -- --ignored --nocapture`（45s 采集窗）
3. Gavin 按脚本说约 34 秒（脚本见下，届时由主控贴给他，他不必记）
4. coder-1 依次做：量 attack/亏欠 → 切 A/B 片段 → 跑 `gate335_asr_ab_manual` → 出结论

**采集脚本**：
```
静默2s → 说「你好」→ 停2s → 说「你说」→ 停2s
→ 一口气连读：今天天气不错你好我们开始吧我这边都准备好了你说是不是
→ 停2s → 整段再来一遍
```
⚠️ 段间静默不可缩短（唯一分段依据）；🔴 **不可拍手/敲桌做标记**（会把闸打开，毁掉冷起振条件）。

**为什么要这么设计**：「你好」「你说」各出现两次 —— 一次冷起振（闸未开）、一次藏在连读句中（闸已开）。
同一人、同一字、同一次录音，**唯一差别就是过不过闸** ⇒ A 错 B 对即因果坐实，两组一样即线索证伪。

**已就绪（零生产代码改动，numstat 279/0 全在 test cfg）**：
`gate335_capture_envelope_manual`（45s 采集 → capture.wav + envelope-1ms.txt）、
`gate335_asr_ab_manual`（走生产解码路径 `transcribe_acc_ctx`）、
`analyze_attack.py`、`cut_segments.py`（均带量具自检）。跑法见 `collab/outbox/coder-1/result.md`。

**预备读数（n=2，不是结论）**：attack 31ms / 326ms；首 30ms 相对稳态亏欠 42.9dB / 35.9dB。

**零成本旁证（已请 Gavin 日常端测留意）**：环境吵 ⇒ 闸开着 ⇒ 此时首字应不易出错。
若反馈「现在『你』不再被听成『按』」⇒ 强旁证；若「照样出错」⇒ 与闸无关，线索可划掉。

### 已知遗留（非本批引入）
- KV `max_total_len=4096` 与 `HOTWORDS_MAX_TOTAL_TOKENS=3000` 耦合，改需同批核算（注释已写明）
- `MAX_WHITESPACE_SEGMENTS=4` 等其余候选闸门未复核（本批只改了长度上限 30→12）
