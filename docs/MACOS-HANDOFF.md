# macOS 团队交接文档 · Feiyin Voice Input

> 编写：Windows Agent 团队 ｜ 日期：2026-07-30 ｜ 依据：DEC-033 及其两条附则
> 读者：macOS 侧 Agent 开发团队（全新接手，未参与过本仓库开发）
> 目的：让你第一天 clone 仓库就能避开我们踩过的坑，并清楚跨平台协作的硬约定

---

## 0 · 先读这份，再读那两份

### 0.2 · ASR-074 音频上行背压修复（2026-09-03）

- **文件域**：`src/audio/mod.rs` + `src/transcription/qwen_inference.rs`
- **macOS 影响**：`src/audio/mod.rs` 是平台中立模块（cpal 回调），macOS 侧编译同一份代码。
  本次改动加 `AtomicU64` 丢帧计数器 + `log::warn!` 可见化，**平台无关**，macOS 上同样生效。
  `qwen_inference.rs` 是在线 ASR 客户端，平台无关（WebSocket + tungstenite）。
- **Step 1 埋点**：`[ASR-LOOP]` / `[ASR-BACKLOG]` / `[ASR-DROP]` 日志行，macOS 上同样输出（`log::debug!` / `log::warn!`）。
- **Step 2 修复（待坐实后实施）**：
  - A（排空 chunk_rx）：平台无关，消费速率提升对 macOS 同样有效。
  - B（drain warm.rx）：平台无关，`warm.rx` 是 crossbeam channel。
  - C（永久 warn 丢帧）：已在 Step 1 落地。
- **结论**：**零编译影响、零行为回归风险**，macOS 侧无需额外改动。

### 0.1 · OVERLAY-061/068/064 跨平台评估（2026-08-30）

- **OVERLAY-068 位置/尺寸同步逻辑**：全部位于 `src/main.rs` 的 `#[cfg(target_os = "windows")]` 消息循环内，macOS 侧目前没有等价 overlay 宿主窗口，因此本次修改对 macOS 侧**零编译影响、零行为影响**。
- **OVERLAY-061 左上角闪屏**：根因为 Windows Win32 `CreateWindowExW` + `CW_USEDEFAULT` + `WS_POPUP` 的组合行为；macOS 侧使用 `NSPanel` 创建窗口，不存在同一机制，**不适用**。
- **OVERLAY-064 灰色边框消失**：根因为 Windows GDI `RoundRect` 右下开区间 + DWM 圆角遮罩的组合；macOS 侧 overlay 绘制将走 AppKit/CoreGraphics，**不适用**。本次不改 GDI 绘制，统一交给下一轮 Direct2D + DirectWrite 迁移（DEC-055）。


本文档是**入职材料**，不是工作总结。读完它你应该知道：
1. 平台层契约长什么样、为什么这样设计（§1）
2. 跨平台协作的硬约定与当前防线缺口（§2）—— **这节最重要，不看会破坏对方平台**
3. 构建环境的两个陷阱（§3）
4. checkout 后立刻会撞上的既有缺口与交接请求（§4）
5. 你们要实现的 TODO 清单（§5）
6. 仓库工程约定（§6）

**深入信息看这三份**（本仓库内，不必外求）：
- `docs/MACOS-PORT-ASSESSMENT.md` — 移植可行性评估，含 P0-P3 缺口全景与工作量估算
- `docs/BUILD-MACOS.md` — macOS 构建环境依赖清单与当前可编译状态
- `docs/MACOS-BRANCH-AUDIT.md` — **macOS cfg 分支静态审计（雷区清单）**。逐块核对了所有
  `#[cfg(target_os = "macos")]` 分支的 API 签名与锁定 crate 版本是否匹配，按
  P0/P1/P2/P3 分级并标注置信度。**结论：唯一会阻塞编译的是
  `src/crash/reporter.rs:369`**（`egui::FontData::from_bytes` 在 egui 0.29.1 中不存在，
  应为 `from_owned`）。你们第一次跑 `cargo check` 前先读它，可少走一轮弯路

---

## 1 · 平台契约

### 1.1 两侧必须各自提供的 15 个符号

平台抽象层在 `src/platform/mod.rs`。本批次（MACOS-COMPAT-001）已把 glob 导出（`pub use windows::*`）改为**显式清单**，两侧导出面集中在同一文件内可肉眼比对。漏列会立即编译失败（响亮失败优于静默漂移）。

两侧（`windows` / `macos`）**必须**各自提供以下 15 个公开符号（依据 `src/platform/mod.rs:61` 起的 `pub use windows::{...}` 显式清单 + 各子模块 `^pub ` 项核对）：

| 模块 | 符号 |
|---|---|
| autolaunch | `enable`, `disable`, `is_enabled` |
| event_loop | `create_controller_window`, `destroy_controller_window`, `run_message_loop` |
| hotkey | `notify_config_changed`, `HotkeyEvent`, `HotkeyListener` |
| injection | `capture_focused_text_snapshot`, `copy_text_to_clipboard`, `inject_text`, `read_text_from_hwnd`, `FocusedTextSnapshot` |
| scene | `capture_scene_signals` |

> 本批次已通过编译器验证：主控后台 `cargo check` 0 errors（4m43s），显式清单无遗漏（依据 `collab/handoffs.md` 2026-07-29 MACOS-COMPAT-001-CORE 条目）。

> 🍎 **【macOS 侧追加·2026-08-04】符号数已从 15 增至 17**，新增 `foreground_window_id`、`capture_scene_signals_by_id` 两个契约符号（两侧均已实现，`platform/mod.rs` 两份清单同步更新）。**本节表格未改写你们的原文**，增补详情与动因见 **§2.10-A**。

### 1.2 平台相关类型差异（刻意保留，不是遗漏）

以下 API 在两侧签名不同，**调用方必须在 `#[cfg]` 分支内使用**（依据 `src/platform/mod.rs` 契约注释块）：

| API | Windows | macOS | 不统一的原因 |
|---|---|---|---|
| `create_controller_window()` | `Result<HWND>` | `Result<()>` | 统一需改 Windows 已交付路径（v0.7.2），违反 DEC-033 第 4 条硬约束「代码重构不得影响任何 Windows 代码功能」 |
| `destroy_controller_window(hwnd)` | 入参 `HWND` | 无入参 | 同上（历史遗留，见 §1.3） |
| `FocusedTextSnapshot.hwnd` | `HWND` | `usize` | 同上 |
| `read_text_from_hwnd(hwnd)` | 入参 `HWND` | 入参 `usize` | 同上 |
| `capture_scene_signals(hwnd)` | 入参 `HWND` | 入参 `usize`（本批次 stub） | 同上 |

### 1.3 stub 设计原则（硬约定）

新增 macOS stub 一律优先保证 **名称 + arity 与 Windows 侧相同，仅参数类型平台化**（如 `HWND` → `usize`）。这样两侧函数形状一致，调用点未来有机会去掉 `#[cfg]` 变成共享代码。

- arity 差异是**最后手段**
- 既有 `destroy_controller_window`（Windows 收 `HWND` / macOS 无参）属**历史遗留**，按红线不动它，但**不要以它为范本**

`capture_scene_signals` 的 macOS stub 入参是 `_hwnd: usize`，该 `usize` 承载什么由你们决定（`CGWindowID` / `AXUIElement` 指针 / 或忽略不用），我们只保证接缝形状（依据 `src/platform/macos/mod.rs:78` TODO 注释）。

### 1.4 为什么不用 trait 抽象

trait 只约束当前编译目标上的实现；`#[cfg(...)]` 切掉的另一侧实现编译器从 AST 移除、不做类型检查（✅官方 Rust Reference）。trait 防不住 cfg 掉的那一侧漂移。真正的防线是显式清单 + 双平台 CI（见 §2 现状说明）。

---

## 2 · 跨平台协作硬约定【最高权重】

### 2.1 必须理解的机制：cfg 不做类型检查

Rust 的 `#[cfg(...)]` 切掉的代码，编译器从 AST 移除、**完全不做类型检查**（✅官方 Rust Reference，`collab/decisions.md` DEC-033 原因段引用）。

后果：
- **我们改共享代码破坏了 macOS，Windows 上 `cargo test` 照样全绿；反之亦然**
- 本项目在纯 Windows 阶段、macOS 一行没跑过时，平台层**已经漂移了 6 处**（`FocusedTextSnapshot.hwnd` 一侧 `HWND` 一侧 `usize`、`notify_config_changed` / `capture_scene_signals` macOS 侧不存在等），**一次编译错误都没触发过**（依据 `docs/MACOS-PORT-ASSESSMENT.md` §7 实证表）
- **trait 抽象也防不住**——trait 只约束当前编译目标上的实现，被 cfg 切掉的实现编译器根本看不见

### 2.2 硬约定条款

- **任何一侧改动 `platform/` 层导出面，必须同步更新 `src/platform/mod.rs` 中两份清单**（Windows 与 macOS），并在提交信息/PR 描述里声明
- 改动共享代码（`src/config/`、`src/llm/`、`src/transcription/` 等非平台目录）时，**必须自查是否触及平台层调用点**（改 `AppConfig` 字段名是最高风险——两侧平台层都消费它，Windows 照样编译通过，macOS 炸）
- **两侧各自本地 `cargo check` 只能验证本平台，防不住对侧**——这是当前状态下的固有缺口

### 2.3 如实陈述：当前无 CI 防线

Gavin 2026-07-29 决定**暂不启用 GitHub CI/CD**，维持本地平台构建发布（DEC-033 附则二，`collab/decisions.md:445`）。

**后果**：无 CI 状态下，「Windows 侧改动破坏 macOS」与「macOS 侧改动破坏 Windows」**都不会在提交时暴露**，回到「破坏发生在提交那一刻、暴露在数周后切换机器那一刻」的状态。项目在纯 Windows 阶段已因此漂移 6 处。

本仓库为公开仓库，标准 GitHub-hosted runner 含 macOS **免费且不限量**，该防线的成本障碍并不存在——若未来改变主意，可零成本启用。但**当前这道防线不存在**，上述人工约定是**唯一可执行的保障**。请重视。

> 研究结论（`collab/research/macos-dualplatform-refactor-001.md`）：双平台 CI 是防止签名漂移的唯一可靠防线。此处平实陈述事实，供你判断风险。

### 2.4 提交署名规范（Gavin 硬要求）

**禁止在 commit message 中添加任何 AI 署名**，包括 `Co-Authored-By: Claude ...`、`Generated with ...` 等。提交作者统一为：

```
Gavin.S <cdexs@hotmail.com>
```

> **为什么之前你们不知道这条**：该规则此前只存在于 Gavin 用户级的 `~/.claude/CLAUDE.md`（不在仓库内），你们无从得知。提交 `a38a315` 带了 `Co-Authored-By: Claude Opus 5` —— **不要求返工**（改历史比留着更危险，见 2.5），后续提交遵守即可。这条现在写进仓库，就是为了终结「规则只在一侧可见」这类问题。

### 2.5 git 破坏性命令禁令【致命，两侧同等适用】

**唯一允许的 git 命令**：`status` / `diff` / `log` / `show`（只读排查）。

**绝对禁止**（无一例外）：

| 命令 | |
|---|---|
| `git reset`（含 `--hard`/`--mixed`/裸用） | `git checkout -- <path>` / `git restore <path>` |
| `git clean`（含 `-f`/`-fd`） | `git stash`（含 `pop`/`drop`/`clear`） |
| `git commit --amend` | 任何强制覆盖历史的操作 |

**背景**：2026-07-24 Windows 侧一个 Agent 为排查一个测试失败，误判他人未提交的改动为「自己误触」，执行批量 `git checkout --` + `git stash`/`pop`，**导致工作区回退两周、11 个已验收批次的改动全部丢失**（`collab/troubleshooting.md [GIT-RESET-INCIDENT-001]`）。

**遇到「这个改动是不是我误触的」这类疑惑时的正确做法**：先用 `git diff` / `git status` 摸清范围 → **报给人类决策**，绝不自己清理。双平台单仓库下这条风险更高：你看到的「非预期改动」很可能是**对侧团队的正常提交**。

### 2.6 共享业务逻辑必须放平台中立模块【本节 2026-07-30 新增】

**规则**：任何**不含平台 API 调用**的业务逻辑，一律放进平台中立模块，**不得**放进带 `#[cfg(target_os = "...")]` 的区域。

**主控 2026-07-30 实测的平台中立模块清单**（`grep -c target_os` 结果为 **0**，可直接双侧复用）：

| 模块 | 内容 |
|---|---|
| `src/itn.rs` | 数字/单位规整规则 |
| `src/translation/mod.rs` | 翻译引擎与方向逻辑 |
| `src/text_normalizer.rs` | 简繁/脚本/语种探针 |
| `src/config/mod.rs` | AppConfig 与持久化 |
| `src/llm/mod.rs` | LLM 请求与解析 |

**反面实例（本方自查发现并已纠正）**：Windows 侧 2026-07-30 实现翻译双向化时，把 `ensure_translation_direction()`（比较缓存方向与目标方向，不一致则重建引擎）和 `persist_translation_direction()`（写 AppConfig 字段并 save）两个函数放进了 `src/main.rs` 且带 `#[cfg(target_os = "windows")]`。**这两个函数体内没有任何一行是平台相关的** —— 若不纠正，你们要用就得各自重写一份，正是本文档 2.1 所述的漂移温床。已挪入平台中立模块，`main.rs` 内**只保留调用点**。

**请两侧共同遵守此模式**：平台特有的部分留在 `platform/` 与 cfg 区，业务逻辑上移到共享模块。

**PUNCT-GOVERNANCE-030-D 更新（2026-08-08）**：`src/llm/mod.rs` 翻译路径的 `system_content` 装配已抽为模块级私有纯函数 `build_translate_system_content(target: TranslationLanguage, punctuation_enabled, wordbook_block: Option<String>, extra_instruction: Option<&str>) -> String`（`impl LlmClient` 之后的模块级区域，**无任何 `#[cfg]`**，两端同为 cfg 自由区编译）。Windows 侧 `optimize_and_translate` 改调用此纯函数；macOS 侧复用同文件自动生效，**无需任何移植**。该函数零行为变更是硬验收项：抽取前后 24 组合逐字节相同（临时断言验证后已删）。若 macOS 侧将来要加翻译路径 prompt 测试（TEST-SYNC-030-B），直接单测此纯函数即可，无需 mock HTTP。

### 2.7 共享文档共编约定与引用纪律【2026-07-30 新增】

`collab/` 与 `logs/` 已于 2026-07-30 移出 `.gitignore` 并入库（Gavin 决定），两侧从此共享同一份治理文档。随之而来的约定：

**共编约定**：

- **只追加自己那侧的条目，不重排、不改写他方段落**（你们提交 `a38a315` 做到了「DEC-033 正文逐行未改动，仅追加附则三」，这正是标准做法）
- 冲突时**保留双方内容**，不要择一丢弃（本方合并 `CHANGELOG.md` 冲突即按此处理）
- 高冲突文件：`collab/todo.md`、`collab/handoffs.md`、`CHANGELOG.md`、`logs/YYYYMMDD.md`

**引用纪律（两次踩坑后立此条）**：

1. **跨团队引用只许引仓库内实际存在的文件**。历史事故：你们文档曾引用 `collab/troubleshooting.md [NPM-CI-LOCK-DESYNC-001]` 与 `[NPM-LOCK-CROSSPLAT-001]`，而当时 `collab/` 尚未入库，本方**根本读不到**；反之本方的 DEC 编号你们也看不到
2. **引用决策编号前先确认它在仓库里存在**。历史事故：`DEC-034` 被新立时，本方 `decisions.md` 只到 DEC-033，双方各自记录同一约束（已由 Gavin 拍板合并，DEC-034 编号作废留作墓碑）
3. 不确定某编号是否存在时，`grep -n "DEC-0xx" collab/decisions.md` 一秒可查

**🔴 跨端影响评估强制条款【2026-08-04 Gavin 指令，两侧同等适用】**：

> **任何一端在实施开发和构建后，都应评估对另一端的影响，并写入本跨端交接文档。**

- **本文档是双向的**：不是「Windows 写给 macOS 看」的单向交接，而是**两侧共编的跨端契约文档**。macOS 侧的改动同样要评估对 Windows 的影响并记入此处
- **时机**：开发**和构建**完成后即评估，与各自的文档收口同步，不批量补
- **判定标准**：

  | 情形 | 处理 |
  | --- | --- |
  | 改动落在**平台中立模块**（非 `src/platform/**`，如 `src/itn.rs` / `src/llm/mod.rs` / `src/scene/mod.rs` / `src/text_normalizer.rs` / `src/config/`） | 另一端编译同一份代码 → **必须记** |
  | **行为变更** | 比 bug 修复更要记 —— 另一端可能已按旧行为写了测试、文档或产品说明 |
  | **平台契约变化**（`src/platform/` 下符号、stub 签名增删改） | 必须记，见第 1 节 15 个符号 |
  | **构建产物/依赖/工具链变化**（`Cargo.toml`、`package.json`、lock 文件、vendor 补丁） | 必须记 —— 另一端的构建可能因此失败 |
  | 纯本端平台代码且无契约变化 | 可不记，但**结论要明确写出来**（「已评估，对另一端无影响」），不得沉默 |

- **记录内容至少含**：改动了什么、行为前后对比、对另一端的具体影响、是否需要对方同步改动
- **为什么这条重要**：另一端接手时会**第一时间读本文档**据此评估如何实施。漏记 = 对方拿到过时契约，要么重复踩坑，要么做出与本端不一致的实现。代码里读不到的东西，只能靠这份文档传递（2.8 就是典型例子）

**排除在入库范围外的三类**（请遵循同一边界，勿提交）：

| 排除项 | 理由 |
|---|---|
| `collab/research/audio-*/` | PoC 语音语料 62MB / 829 个 wav，含 Gavin 本人真实录音；本仓库为**公开仓库** |
| `collab/inbox|outbox|acks|drafts/` | 任务派发/结果/ACK 属瞬时 IPC，两侧各自本地，入库必冲突（outbox 另含 2.8MB 截图） |
| `desktop.ini` | Windows 资源管理器产物 |

### 2.8 管线顺序契约（DEC-035）【代码层面无法共享，必须靠本条传递】

**这是本文档中唯一「代码里读不到、只能靠文档传递」的行为契约，请特别重视。**

Windows 侧 `run_pipeline`（`src/main.rs`，整个函数在 `#[cfg(target_os = "windows")]` 内）的处理顺序为：

```
ASR raw_text
  → is_effective_text 门控（吃 raw_text）
  → 场景采集
  → 【主通道】itn::normalize_numbers(raw_text)        ← 2026-07-31 新增位置
  → 三分支：(a) LLM 成功 / (b) LLM 失败兜底 / (c) LLM 关闭
  → 【补丁通道】itn::normalize_unit_symbols_only(final_text)  ← 仅单位符号段
  → 本地标点引擎（仅 !llm_handled 时运行）
  → 注入
```

> ⚠️ **本节已于 2026-07-31 按 DEC-036 重写。若你们此前按旧版（「ITN 必须在 LLM 之后」单点位置）实现或排期，请以本节为准。**
> 旧版正文见 git history；作废原因见下方「为什么从单点改双通道」。

**四条硬约束**：

1. **ITN 拆为双通道，主通道在 LLM 之前、补丁通道在 LLM 之后**（DEC-036）。这是因为两个方向的缺陷同时存在，单点位置无法兼顾：
   - **放 LLM 后会挨的坑**：ASR 把「摄氏」误听成「摄息/摄斯/摄四」（Windows 侧实测 11 次温度类听写只有 2 次听对），ITN 的 ℃ 规则字面匹配「摄氏」，只有等 LLM 纠正错字后才可能触发 → 这是补丁通道存在的理由
   - **放 LLM 前会挨的坑**：LLM 会曲解原始表达并**销毁信息**（Windows 侧实测：`四点三刻`(=4:45) 被 LLM 输出为 `4:30`；`明天下午` 被输出为 `今天下午`），ITN 放在后面无论多聪明都救不回来 → 这是主通道存在的理由
   - **拆分依据**：`normalize_numbers`（`src/itn.rs`）本来就是两段式——`normalize_with_rules`（中文数字）+ `normalize_unit_symbols`（阿拉伯数字→单位符号）。主通道调完整版，补丁通道**只调第二段**
   - **幂等性是前提，已实测**：`cargo test --bin feiyin-ime unit_symbol` 11/11 passed（含 `unit_symbol_idempotent`）。`40℃` 经补丁通道逐字节不变（`℃` 非 ASCII 数字开头，不匹配）

2. **两个通道都必须在标点引擎之前**。原因：标点模型（CT-Transformer）在 ASR 转写风格文本上训练，若先加 ℃/阿拉伯数字再喂给它，输入即分布外。**模型的分布外退化无法用规则修复，只能观测**；而 ITN 是确定性规则引擎，输入域变化可用规则+单测补齐。**把不确定性留给可控的一侧**（此条自 DEC-035 起未变）

3. **三条路径都必须经过 ITN**。特别是 (b) LLM 运行时失败兜底 —— 漏了这条，用户会看到纯汉字数字（「四十摄氏度」），**比不修更差**。主通道置于 `raw_text` 之后、补丁通道置于三分支之后，两处均天然覆盖三条路径，**不要在各分支里重复调用**（此条自 DEC-035 起未变）

4. **prompt 侧有配套条款，不可只搬管线**。`src/llm/mod.rs` 的 `UNIT_SYMBOL_PROTECTION` 常量假定「输入已含规整后的数字」——该前提**只有在主通道位于 LLM 之前时才成立**。DEC-035 时期该指令前提为假、形同虚设（这个缺口 Windows 侧到 2026-07-31 才发现）。同批次已追加**事实保全**条款（禁止对数值/时间/日期做重算、取整、重新表述或替换）。**macOS 侧实现管线时若只搬顺序不搬 prompt 条款，`4:45→4:30` 类缺陷会照常发生。**

**当前 macOS 侧状态（Windows 侧 2026-07-31 核实）**：`run_pipeline` 整个函数在 `#[cfg(target_os = "windows")]` 内，macOS 侧仍是 `src/main.rs` 的 `mod macos_stubs`（仅占位类型，无管线实现）。**故本节四条约束在代码层无法共享，是纯文档契约** —— 编译器不会帮你们兜底。

完整决策记录见 `collab/decisions.md` **DEC-036**（部分推翻 DEC-035：第 1/2 条单点位置结论作废，第 3/4 条继续有效）、**DEC-037**（输出形态按单位族分治）、**DEC-038**（保护词表不得承载规则性语法族）。

> 🍎 **【macOS 侧追加·2026-08-04，本节末段结论已变更】**
>
> 上一段「**故本节四条约束在代码层无法共享，是纯文档契约 —— 编译器不会帮你们兜底**」**自 macOS 侧 `MACOS-P4-NEUTRAL-001` 起不再成立**（本段不改写你们原文，仅在此追加更正）。
>
> `run_pipeline` 已改为 `#[cfg(target_os = "windows")]` 薄封装，函数体整体搬入**无 cfg 的平台中立 `run_pipeline_core`**。你们 2026-08-04 推送的双通道 ITN 代码（`pre_llm_text = itn::normalize_numbers(...)` 主通道 + `itn::normalize_unit_symbols_only(...)` 补丁通道）合并后**已落在 `run_pipeline_core` 内**，macOS 侧编译的是同一份代码。
>
> **含义：本节四条硬约束中的第 1/2/3 条（双通道位置、两通道均在标点前、三路径全覆盖）现已由代码共享强制保证，不再依赖文档纪律。** 第 4 条（prompt 侧 `UNIT_SYMBOL_PROTECTION` 配套条款）本就在平台中立的 `src/llm/mod.rs` 内，同样共享。
>
> 详见 **§2.10-A**。

---

### 2.9 ITN 数字输出形态契约（DEC-041/042/043）【2026-08-04 新增，行为已变更】

**与 2.8 不同，本节的代码是共享的** —— `src/itn.rs` 是平台中立模块，macOS 侧编译同一份代码，改动自动继承。**但输出形态在 2026-08-03~04 发生了实质变更**，若你们此前按旧行为写过测试、文档或产品说明，**需要同步更新**。

#### 2.9.1 数量级数字：锚定最小单位（DEC-042，🔴 行为变更）

Gavin 2026-08-04 拍板：**判定语言中明确提到的最小单位，输出展开到该层，更小的成分用小数位表示。**

| 输入 | 旧输出 | **新输出** |
| --- | --- | --- |
| 三亿 | `300000000` | **`3亿`** |
| 三亿五 | `300005000` | **`3.5亿`** |
| 三亿五千万 | `350000000` | **`3.5亿`** |
| 两千三百四十五万 | `23450000` | **`2345万`** |
| 十亿 | `1000000000` | **`10亿`** |
| 一亿两千三百四十五万六千七百八十九 | `123456789` | `123456789`（最小单位到个位，不变） |

**四条规则**：① 判定最小明确单位 ② **量级分界在万** —— 落万/亿带后缀，落千/百/十/个输出普通阿拉伯数字 ③ **升亿阈值** —— 万层 ≥1 亿且升亿后小数 ≤1 位才升（`3.5亿` 升，`1.2345亿` 不升、退回 `12345万`）④ 更小成分用小数位。

**Gavin 的原则**：保留用户口述的单位，不替他改写表达。与 026-B（`五块一斤`→`5块一斤` 不归一到元）、单价用 `元一斤` 而非 `元/斤` 同源，均与 L0-1 FIDELITY 不变式同向。

> ✅ **状态：027-E 已落地（2026-08-04，提交 `499d56e`）**，本表已生效。
> ⚠️ 若你们在此之前拉取过代码，拿到的可能是 **027-D 的中间态**（只有「隐式补全」场景保留锚定单位，`两万` 仍展开为 `20000`）—— **该判据已被推翻**，请重新拉取。

**完整行为对照（027-E 落地后的实际输出，共 12 条变更）**：

| 输入 | 旧输出 | 新输出 | 升亿判定 |
| --- | --- | --- | --- |
| 三亿 | `300000000` | **`3亿`** | — |
| 一亿 | `100000000` | **`1亿`** | — |
| 十亿 | `1000000000` | **`10亿`** | — |
| 三千万 / 三百万 / 三十万 / 三万 | 展开 | **`3000万` / `300万` / `30万` / `3万`** | 不足 1 亿 |
| 两千三百四十五万 | `23450000` | **`2345万`** | 不足 1 亿 |
| 三亿五千万 | `350000000` | **`3.5亿`** | ✅ 升（小数 1 位） |
| 一亿两千万 | `120000000` | **`1.2亿`** | ✅ 升 |
| 两亿三千万 | `230000000` | **`2.3亿`** | ✅ 升 |
| 一千二百三十四亿五千万 | `123450000000` | **`1234.5亿`** | ✅ 升 |
| 一亿两千三百四十五万 | `123450000` | **`12345万`** | ❌ 不升（1.2345亿，4 位小数） |
| 三亿零五万 | `300050000` | **`30005万`** | ❌ 不升（1.0003亿，4 位小数） |
| 一万亿 | `1000000000000` | **`10000亿`** | ⏸ 规则直接推论，中文习惯说法为「1万亿」，形态待 Gavin 拍板 |

**千层及以下不变**（最小单位落千/百/十/个 → 普通阿拉伯数字）：
`三千`=`3000` / `三百`=`300` / `三十`=`30` / `两万五千`=`25000` / `三万五千`=`35000` /
`一千零四十六万八千七百四十一`=`10468741` / `一亿两千三百四十五万六千七百八十九`=`123456789`

**实现要点**（供你们理解，代码已共享无需重写）：新增 `format_dec042_magnitude(result, unit_char)` 统一格式化与升亿判定；沿用 027-D 的**孤立判定**（末尾数字后若跟单位则维持展开）规避 `UnitChain` 的 `.parse::<f64>()` 静默归零风险 —— **这点特别注意**：`parse_cn_number` 的返回值不再恒为纯数字，若你们有额外调用点做 `.parse()`，须同样做孤立判定。

**测试基线**：`cargo test --bin feiyin-ime itn::` **196 / 0**（027 全批累计 153→196）。

#### 2.9.2 🔴 三套「数字+单位」逻辑并存，禁止统一（DEC-043）

| 类别 | 规则 | 判据 | 实例 |
| --- | --- | --- | --- |
| 数量级（万/亿） | 锚定**最小**单位 | 最小单位 | `三亿五`→`3.5亿` |
| 货币（元/块/角/毛） | 单段保原单位，多段**归一到元** | **段数** | `五块`→`5块`／`一块两毛二`→`1.22元` |
| 重量（斤/两） | **全部单位逐段保留** | 全保留 | `一斤二两`→`1斤2两` |

**三者方向互不相同**（取最小 / 归一到最大 / 全保留），用任一套去套另外两套都会得到相反结果。

🔴 **这不是缺陷，是三次独立的、经 Gavin 逐条拍板的设计选择。任何人不得以「三者不一致」为由自行统一。** 将来若要统一须单开设计任务先出方案，**不得在 bug 修复或功能任务中顺手统一**。

#### 2.9.3 格式输出必须依赖 LLM，禁止程序化后处理（DEC-041）

排版与格式（是否分行、是否用列表、何种列表、标题、表格）的判定权**全部归 LLM**，**禁止**在 Rust 侧对 LLM 输出做格式重排类后处理来「补救」模型没做到的排版。

**边界**：既有的**安全与正确性**类后处理不受影响 —— `flatten_multiline`（`multiline_safe=false` 场景压平，防一条消息被拆成多条发出）、空结果护栏、标签解析等，它们管的是注入安全与故障兜底，不是替模型做排版决定。

#### 2.9.4 本批次涉及的缺陷（macOS 侧同样存在，代码已共享修复）

| 编号 | 缺陷 | 严重度 |
| --- | --- | --- |
| 026 | 货币链遇未知后继词撕裂（`五块一斤`→`5块`+`一斤`）+ 尾零未去 | 输出损坏 |
| 027-A | 末尾单字越界套隐式千位（`一千零四十六万八千七百四十一` 差 999） | **金额静默改错** |
| 027-B | 万级进位把已结算的亿也乘 1e4（亿+万嵌套差 4 个数量级） | **金额静默改错** |
| 027-C | `is_unit` 的 `starts_with` 使「亿两千…」被判为「亿后跟单位」而 break，**整串金额蒸发成 `1`** | **金额蒸发** |
| 027-C-2 | `two_is_unit` 只查 `all_units`，进位单位不在表内 → 「两千」仍被误判 | 同上 |
| 027-D | DEC-042 隐式补全保留锚定单位（`三亿五`→`3.5亿`），027-D 仅孤立场景 | 行为变更 |
| 027-E | DEC-042 补完全面落地（`三亿`→`3亿`，最小单位落万/亿带后缀） | **行为变更** |

---

### 2.10 Overlay 流式预览 + 编辑态（ASR-038-C）【2026-08-15 新增，Windows 侧实现】

**影响范围**：`src/ui/overlay.rs`（新增状态枚举）、`src/main.rs`（Windows 侧 overlay 线程与主控事件处理）。**macOS 侧 `src/platform/macos/overlay.rs` 未改动，但产品交互契约已变更，必须同步评估。**

#### 2.10.1 新增产品状态

Windows 侧新增两个 `OverlayStatus` 变体：

- `RecordingWithText { text }`：录音过程中显示流式 ASR 文本。
- `StreamingEditing { text }`：用户在录音过程中点击 overlay 文本区，接管编辑；停止录音与后续 pipeline，进入可编辑状态。
- `EditSubmitting { text }`：编辑提交中过渡态，用于 overlay 复位动画。

#### 2.10.2 核心交互契约

1. **按下热键**：overlay 以 `Recording` 态出现（尺寸 240×36，居中，不抢焦点）。
2. **流式文本到达**：overlay 切换到 `RecordingWithText`，文本在客户区中央显示；窗口宽度随文本量扩展，上限为当前显示器工作区宽度的 65%；超过上限后文本在可视区内向左滚动，最新文字紧贴右边缘。
3. **用户点击文本区**：overlay 进入 `StreamingEditing`；同时：
   - 移除 `WS_EX_NOACTIVATE`，使 overlay 可获焦；
   - 创建 Win32 `EDIT` 子控件接管输入/IME；
   - 停止音频采集并取消 ASR/pipeline（不进入 `Processing`）；
   - 托盘仍显示 `Recording`，表示本次语音输入尚未结束。
4. **编辑提交（Enter 或点击提交按钮）**：把 EDIT 内容原样注入录音开始时的目标窗口；若无法取回焦点（UIPI），降级为复制到剪贴板并显示 `FocusLost` 预览。
5. **编辑取消（ESC 或点击停止按钮）**：隐藏 overlay，托盘恢复 Idle。
6. **重新按下热键**：overlay 必须重置为初始 `Recording` 态（尺寸、样式、NOACTIVATE 全部复位）。

#### 2.10.3 macOS 侧待实现项

- 在 `OverlayStatus` 中新增同样变体（已落在平台中立 `src/ui/overlay.rs`，macOS 侧编译同一份代码，无需重定义）。
- `src/platform/macos/overlay.rs` 需要实现对应行为：
  - 在 `Recording` overlay 内叠加只读流式文本；
  - 点击文本区切换到可编辑控件（`NSTextField` / `NSTextView`）；
  - 窗口尺寸动态扩展 + 65% 上限 + 内部滚动；
  - 提交按钮 / Enter 提交 / ESC 取消；
  - 编辑提交后调用 `platform::inject_text` 或 `copy_text_to_clipboard` + `FocusLost` 预览降级。
- 与 Windows 侧保持一致的状态流转：录音中点击即进入编辑，不等松开热键。

#### 2.10.4 2026-08-15 返工修复补充

主控验收 ASR-038-C 时发现 3 项问题，已在 Windows 侧修复并同步影响 macOS 交互契约：

| # | 问题 | Windows 侧修法 | macOS 侧需对齐 |
| --- | --- | --- | --- |
| 1 | 编辑态一进即被销毁 | 新增 `OVERLAY_EDITING` 主控侧标志 + overlay 侧 `StreamingEditing` 守卫，使 `EditRequested` 触发的 `PipelineEvent::Cancelled` 不再 `Hide` | macOS `handle_pipeline_event` 在编辑态接管时也需忽略 `Done/Cancelled` 的 Hide 请求，直到用户提交/取消 |
| 2 | 托盘编辑态被复位 Idle | `Done\|Cancelled` 分支编辑态时保持 `Recording` | 编辑态期间托盘保持 `Recording` |
| 3 | 100ms 尺寸节流缺失 | `OverlayWindowState` 新增 `last_resize_time`，`RecordingWithText` 每次刷新至少间隔 100ms 才重算宽度 | 流式文本刷新加 100ms 尺寸防抖，避免窗口宽度抖动 |

> 注：037 设计中的 `pipeline_cancelled` 拦截全库仅存在于注释，未实际实现；返工后以 `OVERLAY_EDITING` 标志 + overlay 侧守卫双保险替代。

#### 2.10.5 对 Windows 侧的反向约束

- `OverlayStatus` 枚举是共享类型，macOS 侧编译同一份定义，**禁止从 Windows 侧删除或重命名新变体**。
- 新增 `OverlayCommand::EnterEditMode` 是 Windows-only 内部命令，不影响 macOS 平台契约。
- 新增 `OverlayUiEvent::SubmitRequested(String, platform::WindowId)` 仅 Windows 侧使用；macOS 侧若采用不同事件模型，不强制对齐签名，但**语义**（提交文本 + 目标窗口）必须一致。
| 027-F | 027-E 引入的静默归零：隐式尾数吸收未做纯数字校验，带单位串进 `format_currency_chain` 的 `.parse()` 静默归零（`五块三亿`→`5元`） | **金额静默改错** |
| 027-G-1 | DEC-042 补充二：万亿层升级链（`一万亿`→`1万亿`，亿层 ≥1e12 且 ≤1位小数升万亿） | **行为变更** |
| 027-G-2 | DEC-044：`十万个为什么`加专名白名单（书名兼俗语，`itn-rules.toml` proper_nouns +1 条） | 数据变更 |

**027-F 跨端提示**：`parse_cn_number` 契约变化后（027-D/E 返回带单位串），macOS 侧若复用同一份 `src/itn.rs`，隐式尾数吸收处（`:1130`）的纯数字校验已共享修复。但若 macOS 侧有**独立的数字消费逻辑**（如 AX 无障碍读屏提取数字），需同样校验 `parse_cn_number` 返回值是否纯数字后再做数值运算。

**027-G 跨端提示**：万亿层是 `format_dec042_magnitude` 内的行为变更（macOS 侧共享 `src/itn.rs` 自动同步）。`十万个为什么` 词表新增在 `itn-rules.toml`（平台中立资产，macOS 侧共享 `include_str!` 内置默认 + 运行时 exe 同级 toml 覆盖）。若 macOS 侧有独立 toml 副本需同步该词条。

**BUILD-014 产物状态（tester-1 补记，2026-08-04）**：027 全批（A/B/C/C-2/D/E/F）已在 Windows 侧完成全量回归（itn:: 212/0）并首次进入 release 产物（feiyin-ime.exe `0e13cff5…`），平台中立模块 `src/itn.rs` 行为与上表一致；macOS 侧沿用共享修复即可，无新增平台差异。

**BUILD-015 产物状态（tester-1 补记，2026-08-04）**：027-G（万亿层 + 十万个为什么白名单）已完成全量回归（itn:: 219/0）并进入 release 产物（feiyin-ime.exe `d29b8325…`），itn-rules.toml 三副本 `b208271b…` 一致（新词条进包）；macOS 侧沿用共享 `src/itn.rs` 与 `itn-rules.toml` 即可，无新增平台差异。

**ITN-FIX-WANYI-031 跨端提示（coder-1 补记，2026-08-08）**：`parse_cn_number` 万/億 分支加了前置守卫
`if !has_digit && section == 0 && result == 0 { return None; }`（:778/:817）——「万一/亿万/百万/千万」这类
固定词不再被数字化（此前输出 `0.1万`/`0万`）。macOS 侧编译同一份 `src/itn.rs` 自动继承，无平台差异。
对照组（一万→10000、十万→100000、三亿五→3.5亿、一万亿→1万亿、一亿两千三百四十五万六千七百八十九→123456789）
全部零回归，itn:: 221/0。`itn-rules.toml` 零改动。

**PUNCT-GOVERNANCE-030-E 跨端提示（coder-2 补记，2026-08-08）**：L2 后处理判定已从 `run_pipeline_core`（平台共享代码）抽为纯函数 `punctuation::apply_l2_postprocess(text, punctuation_enabled) -> (String, L2Action)`（`src/punctuation/mod.rs`）。判定矩阵：(a) 开关关 → `strip_punctuation`；(b) 开且 ≤5 单位 → `strip_trailing`；(c) 其余 → 原样。`main.rs` L2 块只留打日志。macOS 侧编译同一份代码自动继承；`L2Action` 枚举（StripAll/StripTrailing/NoOp）是新增公开类型，若你们在平台代码调用 L2 逻辑请直接用该函数，勿再内联判定（保持零回归护栏可测）。

**PUNCT-GOVERNANCE-030-A/A-2 跨端提示（coder-2 补记，2026-08-08）**：`src/punctuation/mod.rs` + `src/transcription/mod.rs` 是平台中立模块（`run_pipeline_core` 已去平台化，L2 后处理块位于平台共享代码），macOS 侧编译同一份代码，无需镜像。两件事需知晓：
- **030-A**：L2 后处理补位块（`src/main.rs` 共享段）对 6 个产出源一视同仁，删除了来源判据；`count_units`（CJK/假名按字、其余按空格分词）与 `strip_trailing_punctuation`（短句 ≤5 剥末尾标点，直接删除不留空格）为新增平台中立函数。
- **030-A-2**：`strip_trailing_punctuation` 改用 `TRAILING_PUNCT_CHARS`（不含成对符号右半，保护 `"好"`/`）` 不剥）；`native_punctuated` 由假设改为实测 `has_effective_punctuation`（对 `PUNCT_CHARS` 全集合做「词内嵌豁免」：`3.14`/`don't`/`3:30`/`example.com` 内的标点因两侧 ASCII 字母数字夹持不计为有效标点）。**若你们改 prompt 或调用标点判据，请与新函数保持同一口径**，别为单字符特例打补丁（防 ITN-LOCAL-RULE-OVERREACH-001，详见 troubleshooting）。

**PUNCT-GOVERNANCE-030-B/C 跨端提示（coder-1 补记，2026-08-08）**：`src/llm/mod.rs` 是平台中立模块。
030-B：标点开关的槽位条件替换（false 时注入 `NO_PUNCT` 禁句，翻译路径同步改 `step1_correct`）。
030-C：`f3_rules_text` 新增 `punctuation_enabled` 参数，false 时枚举分隔符切换到
`INLINE_SEPARATOR_RULES_NO_PUNCT`（半角空格连接、禁 、；，；两个全角 + 两个半角逗分号），
**且多行分支的 F3c 买菜示例输出侧同步条件化**（punct=false 为空格连接版，输入侧 ASR 原话保持不动），
L1/L2 口径统一无双改写。macOS 侧编译同一份代码，无需镜像；`_NO_PUNCT` 与新示例变量是新产物，
若你们改了 prompt 文本需重新对齐两侧。

**共同模式**（见 `collab/troubleshooting.md` 的 `[ITN-LOCAL-RULE-OVERREACH-001]`）：

> **局部特例规则没有约束自己的适用范围，在更长的上下文里越界生效。**

四个月内至少四次（016/026/027-A/027-C），**全部由 Gavin 端测撞出，没有一次是测试先抓到** —— 因为单测按设计场景写，缺陷只在设计场景之外显形。027-F 则是 tester-1 走查先发现（首次非端测），主控复核后判定为真风险。

**确立的防御规则**：任何「隐式/省略成分补全」类规则，必须显式声明适用边界，并配反向护栏测试证明它在边界外不生效。**只测生效侧 = 没测。** macOS 侧若新增此类规则，请同样遵守。

#### 2.9.5 LLM 连接池僵尸连接（LLM-CONN-POOL-028，2026-08-08，非 ITN）

> 本小节非 ITN，属独立批次，记录在共享模块 `src/llm/mod.rs` 的缺陷与修复，供 macOS 侧知晓。

**缺陷**：Gavin 端测发现 LLM 优化间歇性 0ms 失败（请求未上网络即挂）。根因：`reqwest::Client::builder()` 只设 `connect_timeout`，吃全局默认 `pool_idle_timeout=90s`；DeepSeek 服务端 keep-alive 约 60s 关连接 → 60-90s 窗口内池中残留「服务端已关、客户端以为活着」的死连接，复用即失败。

**修复（macOS 侧编译同一份 `src/llm/mod.rs`，自动同步）**：

1. `POOL_IDLE_TIMEOUT=30s`（新增具名常量，必须 < 服务端 ~60s 留足余量）+ builder `.pool_idle_timeout(POOL_IDLE_TIMEOUT)`
2. 重试判据放宽：`e.is_connect() || e.is_timeout()` → `+ e.is_request()`（`is_request()` = reqwest `Kind::Request`，覆盖连接复用失败类错误；body→`Kind::Body`、decode→`Kind::Decode`、builder→`Kind::Builder`、status→`Kind::Status` 各自独立桶，不会被误吞）
3. 错误日志打印完整 source chain（`fmt_error_chain`，逐层 `Error::source()` 展开），解决 Display 吞链路导致的排障盲区

**src-tauri/src/llm.rs 同步镜像**：同一 `pool_idle_timeout(30s)` + `is_retryable_error` 加 `is_request()`。macOS 设置界面若复用该文件（Tauri UI 默认全平台共享）会自动同步。

**行为前后对比**：空闲间隔 60-90s 的 LLM 请求从此类 0ms 失败 → 正常请求；重试覆盖连接复用失败；debug.log 错误行可看到完整 error chain。

**不需要 macOS 侧独立改动**：本修复全部在平台中立模块内，无平台 API，无契约变化。仅提示：若 macOS 侧 LLM 服务商 keep-alive 比 60s 更短，可自行下调 `POOL_IDLE_TIMEOUT`（需小于服务端 keep-alive 的余量原则不变）。

---

### 2.10 · 🍎 macOS 侧跨端影响评估【本节由 macOS 侧维护，倒序追加】

> 依据 §2.7「跨端影响评估强制条款」（Gavin 2026-08-04 指令，两侧同等适用）。
> 本节是 macOS → Windows 方向的影响回执，**每批开发/构建完成后即追加，不批量补**。
> 格式：改动了什么 · 行为前后对比 · 对 Windows 侧的具体影响 · 是否需要你们同步改动。

---

#### 2.10-A · Phase 4 阶段 A/B 批次（2026-08-04，合并提交 `cc12381`）

**本批次结论一句话**：Windows 侧**无需任何同步改动**，但**平台契约面扩大了 2 个符号**，且 **§1.1 / §2.8 两处原文结论需按上方追加块更新认知**。

##### ① 🔴 平台契约变化（必须知晓）· 15 → 17 个符号

| 新增符号 | Windows 侧实现 | macOS 侧实现 | 位置 |
| --- | --- | --- | --- |
| `foreground_window_id() -> WindowId` | `GetForegroundWindow().0 as usize` | 第一版返 `0`（表「无法判定焦点」） | `platform/windows/event_loop.rs` / `platform/macos/mod.rs` |
| `capture_scene_signals_by_id(id: WindowId)` | 转 `HWND` 后委托既有 `capture_scene_signals` | 委托既有 `capture_scene_signals(usize)` | `platform/windows/scene.rs` / `platform/macos/mod.rs` |

同时 `platform/mod.rs` 新增 `pub type WindowId = usize;`（不透明窗口标识，两侧语义各自定义）。

**对 Windows 侧的影响**：**纯新增**。`git diff --numstat -- src/platform/windows/` 三个文件删除列均为 **0**（`12/0`、`3/0`、`7/0`），既有 Windows 函数的签名、行为、调用点**一行未改**。`capture_scene_signals(HWND)` 与 `create_controller_window()` 等既有 15 个符号完全保留。

**需要你们做什么**：**不需要改代码**。但按 §2.7 引用纪律，若你们后续再动 `platform/` 导出面，两份清单现在是 **17 项**，请以 `src/platform/mod.rs` 实际内容为准。

##### ② 🔴 §2.8 的「纯文档契约」结论已作废（本批次最重要的一条）

`run_pipeline` 改为薄封装 + 平台中立 `run_pipeline_core`，**402 行业务逻辑（ASR 前处理 → 转录 → 场景采集 → 翻译/优化三分支 → ITN 双通道 → 标点 → 注入 → 词库学习）现为双侧共享**。

| 项 | 变更前 | 变更后 |
| --- | --- | --- |
| `run_pipeline` | 整函数 `#[cfg(target_os="windows")]` | **保持** `cfg(windows)` + `target_hwnd: HWND` 签名不变，函数体缩为一行委托 |
| 业务逻辑主体 | 在 `run_pipeline` 内，macOS 不可见 | 移入 `run_pipeline_core`（无 cfg），**macOS 编译同一份** |
| `spawn_worker_thread:2450` 调用点 | — | **一字未改**（薄封装设计即为此） |
| DEC-036 双通道顺序约束 | 纯文档契约 | **代码层共享强制** |

**对 Windows 侧的影响**：**行为零改动**。自证方式——函数体为整段搬运，`git diff` 中该函数体内部仅有 2 个平台接触点 hunk（`capture_scene_signals` → `_by_id`、`GetForegroundWindow` → `foreground_window_id`），其余逐字节保留；`focus_lost` 判据 `!target_hwnd.0.is_null() && current!=target` 等价改写为 `target_hwnd != 0 && current_id != target_hwnd`。

**macOS 侧已用反证法验证 `run_pipeline_core` 确实在 macOS 上参与编译**：往其签名注入一个不存在的类型后 `cargo check` 报 `E0425 cannot find type`，证明并非被 cfg 静默跳过（验证后已还原）。

**需要你们做什么**：**不需要改代码**。但请注意：**今后你们改 `run_pipeline_core` 内的任何逻辑，会直接作用于 macOS 侧**，不再是「只影响自己」。这正是 §2.7 判定标准第一行「改动落在平台中立模块 → 必须记」的情形。

##### ③ 平台中立辅助函数去 cfg（3 个）

`select_preprocessing_params` / `should_try_llm_translate` / `try_nllb_translate` 三个函数上方的 `#[cfg(target_os = "windows")]` 已删除（它们是纯 Rust 逻辑、零平台 API，被 `run_pipeline_core` 调用）。

**对 Windows 侧的影响：可证明的 no-op。** 为 Windows 编译时该 cfg 恒为真、item 本就被无条件包含；删除属性后仍被包含，**两种情况下 Windows 侧 AST 逐字节相同**。

**连带收益**：`select_preprocessing_params` 的 5 个 `#[test]`（你们在 `6f0b51e` 为其加的 cfg 门控）已解封，**macOS 侧测试盲区由 22 条降至 17 条**。

##### ④ macOS 专属缺陷修复（对 Windows 无影响，仅备查）

`src/platform/macos/hotkey.rs` 的 `KEYBOARD_EVENTS` 移除了 `CGEventType::TapDisabledByTimeout`(0xFFFFFFFE) 与 `TapDisabledByUserInput`(0xFFFFFFFF)。这两个是 core-graphics 标注的 "out of band" 事件，本不该进 `eventMask`；其判别值远超 u64 位宽，进入 `CGEventMaskBit!(1 << etype)` 后**debug 下 panic、release 下按 `&63` 掩码静默设错第 62/63 位**。实机探针实测：修复前热键监听线程启动即 panic，一个事件都收不到。

**这是 DEC-017 实现自 2026-04 以来首次实机验证暴露的真 bug。**

##### ⑤ ⚠️ 构建依赖变化（`Cargo.toml` / `Cargo.lock` 是共享文件，按 §2.7 必须记）

| 依赖 | 位置 | 对 Windows 的影响 |
| --- | --- | --- |
| `objc2` / `objc2-app-kit` / `objc2-foundation` / `core-foundation-sys` / `libc` | `[target.'cfg(target_os = "macos")'.dependencies]` 段内（`Cargo.toml:115-119`） | **无**。条件依赖，Windows 不编译。**段落边界已自查**，其后即空行 + `[target.'cfg(target_os="windows")'.build-dependencies]`，未触发 `[TOML-SECTION-DRIFT-001]` |
| ⚠️ `ctrlc = "3.4"` | **被误加进 `[dependencies]` 主表** | **有影响，正在修**。这是为 macOS controller 的 SIGINT 处理引入的跨平台依赖，Windows 侧会一并编译进二进制。macOS 侧已定位，将移入 macOS 专属段或直接移除（Windows 侧本就无 SIGINT 处理器，tray-first 应用靠托盘菜单退出）。**在此如实记录，不隐去。** |

`Cargo.lock` 因上述依赖新增而变更 —— 这是双平台共享文件，你们下次 pull 后 `cargo build` 会重新解析，属预期。

##### ⑥ 📌 DEC 编号撞车（已由 macOS 侧让号，请知悉）

两侧在 2026-08-04 同日并行新立决策，撞了 **DEC-036 / DEC-037** 两个号：

| 编号 | Windows 侧（保留，主线先推送） | macOS 侧（已改号） |
| --- | --- | --- |
| DEC-036 | ITN 改双通道 | → 改为 **DEC-045** · macOS 浮层必须是独立窗口 |
| DEC-037 | ITN 输出形态按单位族分治 | → 改为 **DEC-046** · macOS 事件宿主用 objc2 + NSApplication + CFRunLoop |

**处置原则**：以先推送到 `origin/main` 的一侧为准，后合并方让号。macOS 侧已同步更新 `decisions.md` 与全部引用文档。

**这是 §2.7 引用纪律第 2 条「引用决策编号前先确认它在仓库里存在」的一个新变种** —— 不只是"编号是否存在"，并行开发时还要防"同一编号被两侧同时占用"。**建议约定：新立 DEC 前先 `git fetch && grep -n "^## DEC-" origin/main:collab/decisions.md` 取最大号再 +1**，或按平台分段（如 macOS 侧从 DEC-100 起）。请你们拍板取哪种，本侧配合。

##### ⑦ 当前 macOS 侧真实状态（供你们评估，勿按 §5.2 旧表判断）

| 项 | §5.2 旧记载 | 实测现状（2026-08-04） |
| --- | --- | --- |
| 主控入口 | 非 Windows 分支仅 warn 返回 | **正在实现中**（`MACOS-P4-HOST-001`，NSApplication + CFRunLoop，未验收） |
| 事件循环 | `run_message_loop()` 为 stub | 同上，`src/platform/macos/event_loop.rs` 已创建 |

---

#### 2.10-B · TEST-EXEC-MERGE-001 全量回归结论（2026-08-04，tester-1）

**本批次结论一句话**：Windows 侧 68 提交合并（`cc12381`）叠加我方 `run_pipeline_core` 重构后，**macOS 侧全绿**，包括此前一直红的 `itn::tests::time_half`。

##### ① 全量回归数字（`cargo test --no-fail-fast`）

| 层级 | passed | failed | ignored |
| --- | --- | --- | --- |
| 主程序 bin | 816 | **0** | 6 |
| crash-reporter bin | 28 | 0 | 2 |
| 集成测试 ×5 | 36 | 0 | 0 |
| **合计** | **880** | **0** | **8** |
| `--list` 总数 | — | — | **888** |

分项之和 = 880 + 8 = **888 = `--list` 实测值**，数字链自洽。07-31 基线为 701/1/8（`time_half` 失败），本次新增 **179 条**（主要来自 ITN v2 重构新增测试 + 解封 5 条 `select_preprocessing_params` 测试）+ `time_half` 修复。

##### ② `itn::tests::time_half` 状态变更

- **07-31 基线**: FAIL（`八点半` → `8点半`，`[ITN-PREFIX-SHADOW-001]`）
- **本次**: PASS（`八点半` → `8:30`，ITN v2 `027-B/D/E` 系列重构已修复前缀遮蔽）
- **归类**: (c) 既有遗留，**但已由上游修复，不再是失败**
- **对 Windows 侧**: 该修复来自 `origin/main` 的 ITN v2 批次，macOS 侧共享同一份 `src/itn.rs`，自动继承

##### ③ 测试盲区复算（22 → 17）

| 来源 | 条数 | 门控方式 |
| --- | --- | --- |
| `platform/windows/hotkey.rs` | 15 | 模块级 cfg(windows) |
| `platform/windows/scene.rs` | 2 | 同上 |
| `main.rs pipeline_logic_tests` | **0**（原为 7，TEST-SYNC-P4-NEUTRAL-001 已解封） | — |
| **合计** | **17** | — |

`pipeline_logic_tests` 的 7 条已去除 `#[cfg(all(test, target_os = "windows"))]`，现 macOS 可见。

##### ④ A4 热键实机复测（首次成功）

`MACOS-P4-PROBE-001` 的 A4 项当时卡在 `CGEventMaskBit!` `1 << etype` overflow panic（debug 必崩），`MACOS-P4-FIXHOTKEY-001` 修复根因后，本次 **首次有条件真正验证** CGEventTap 全局按键捕获：

- 辅助功能权限已授予（`ensure_accessibility_at_startup()` 无 "not granted" 警告）
- 自动注入 F9 keydown/500ms/keyup → 收到 `Start` + `Stop`（hold > 300ms）
- 自动注入短 tap → 收到 `Start` + `CancelStop`（hold < 300ms）
- **无 panic**：`KEYBOARD_EVENTS [CGEventType; 3]`（剔除 `TapDisabledByTimeout`/`TapDisabledByUserInput`）已生效
- 这是 DEC-017 实现自 2026-04 以来**首次实机验证成功**（此前因 panic 从未走到事件接收）

##### ⑤ 对 Windows 侧的影响

- **68 提交（itn.rs +4411 / llm/mod.rs +1683 / scene/mod.rs +464）全部通过**，零 (a) 跨平台缺陷、零 (b) 测试平台假设问题
- 我方三批改动（FIXHOTKEY-001 / NEUTRAL-001 / TEST-SYNC-P4-NEUTRAL-001）全部验证通过
- **不需要 Windows 侧做任何同步改动**

##### ⑥ ⚠️ 瞬时编译中断说明

`cargo test -- --list` 复跑时遭遇 coder-2 在途改动（`MACOS-P4-NEUTRAL-002`：`spawn_worker_thread` 去 cfg 过程中，`load_hotwords_for_accuracy`/`compute_hotwords_version` 仍为 cfg(windows)，导致 3 个编译错误）。**该中断为瞬时状态**，主控已裁定按此前干净运行（880/0/8）出报告，不归入回归结论。
| 管线 | `mod macos_stubs` 占位，无实现 | **后段已可复用**（`run_pipeline_core`）；**前段接线未完成**（`spawn_worker_thread` 仍 `cfg(windows)`，待 `MACOS-P4-NEUTRAL-002`） |
| Accessibility 弹窗 | stub，只打日志 | **仍是 stub，实机探针已确认属实**，待 `MACOS-P4-PERM-001` |
| Overlay | 无对等实现 | **仍无**。已按 Gavin 拍板（DEC-045）定为**必须实现独立窗口，不得用托盘图标替代**，规格对齐你们的 240×36 / 320×140 + 水平居中底部上移 64px + 三态指示灯 |
| 录音 / ASR 链路 | 未验证 | ✅ **实机已验证可用**：cpal 实录 RMS 0.108 非零；SenseVoice 转录正确（RTF 0.0458），无 dyld 错误 |
| DEC-015（Tauri 作事件宿主） | §5.2 仍引用 | **已复议作废**，改用 objc2 + NSApplication + CFRunLoop 直驱（DEC-046）。理由：主程序至今不依赖 tauri，为借一个 run loop 拖进 WebView 不划算；且 DEC-045 要求真实 AppKit 窗口，objc2 无论如何都要引 |

---

#### 2.10-B · MACOS-P4-PERM-001 辅助功能授权弹窗真实现（2026-08-04，coder-1）

**本批次结论一句话**：**纯 macOS 平台代码，对 Windows 零影响**——无需 Windows 侧任何同步改动。

##### 改动了什么

- 文件：`src/platform/macos/accessibility.rs`（仅此一个文件，`+42 -11`）
- 内容：`ax_is_process_trusted_with_prompt` 原 stub（仅 log，从未调 API）补全为真实现——新增 FFI 声明 `AXIsProcessTrustedWithOptions` + `kAXTrustedCheckOptionPrompt`，用 `core_foundation`（CFString/CFBoolean/CFDictionary）构造 `{kAXTrustedCheckOptionPrompt: true}` 调用之触发系统授权弹窗。`ensure_accessibility_at_startup` 增强 `log::warn` 引导 + 不阻断启动。

##### 行为前后对比

| 状态 | 前（stub） | 后（真实现） |
| --- | --- | --- |
| 未授权时 | 仅一行日志，无弹窗，热键静默失效 | 系统弹窗「辅助功能」授权请求出现，用户可直达系统设置 |
| 启动阻断 | 否 | 否（不变，与 Windows 对齐） |

##### 对 Windows 侧的具体影响

**零影响**。该文件仅调用 macOS 专属 framework（ApplicationServices + CoreFoundation），在 Windows 编译时被 `#[cfg(target_os="macos")]` 门控不参与编译。Windows 侧无 accessibility 概念、无对应文件、无调用点。

##### 是否需要 Windows 侧同步改动

**否**。平台契约面未变（未新增/修改 `platform/mod.rs` 导出符号，未改共享类型）。本任务不涉及任何共享代码。

##### 验证

`cargo check --all-targets` → 0 errors（macOS 侧）。未实机验证（本机已授权，无法安全复现未授权状态，如实降级——代码层完成）。

---

#### 2.10-C · MACOS-P4-NEUTRAL-002 打通语音输入闭环（2026-08-04，coder-1）

**本批次结论一句话**：改了一处 Windows 既有调用点、删了一个 Windows 可见函数、改了两个共享类型定义——但 **Windows 行为零改动**，无需 Windows 侧任何同步改动。

##### 改动了什么

- 文件：`src/main.rs`（仅此一个）
- Windows 既有调用点修改：main.rs:1929 `target_hwnd: SendHwnd(hwnd.0 as isize)` → `target_hwnd: hwnd.0 as usize`（语义等价：原本存 isize 再 :2440 转回 HWND，现在存 usize，转换在 `run_pipeline_core` 内部由既有契约处理）
- Windows 可见函数删除：`run_pipeline` 薄封装（:2897-2929，零调用者，删除是行为中性，不产生新 dead_code）
- 共享类型定义修改：`StartCmd`/`WorkerCommand` 去 `#[cfg(windows)]`（对 Windows 构建为 no-op）；`StartCmd.target_hwnd` 类型 `SendHwnd → platform::WindowId`
- macOS 侧新增：`run_controller_macos` 重写 + `handle_hotkey_event`/`handle_pipeline_event` 辅助函数；`macos_stubs` 删重复定义（SendHwnd/StartCmd/WorkerCommand/spawn_worker_thread）
- 第 7/8 接触点：`load_hotwords_for_accuracy`/`compute_hotwords_version` 去 cfg（对 Windows 为 no-op）

##### 行为前后对比

| 路径 | 前 | 后 |
| --- | --- | --- |
| Windows run_controller | SendHwnd(hwnd.0 as isize) → spawn_worker_thread → run_pipeline(HWND(...)) → run_pipeline_core | hwnd.0 as usize → spawn_worker_thread → run_pipeline_core(target_hwnd)（少一次 HWND 包装转换，语义等价） |
| macOS run_controller_macos | 注释「worker thread remains cfg-windows only」，热键事件只 log 不接 | worker 线程起来，hotkey→worker→run_pipeline_core→注入闭环可达（代码层） |

##### 对 Windows 侧的具体影响

**行为零改动**。三处「修改」均为中性：
- :1929 类型转换等价（isize↔usize 同宽度，转换位置变但结果不变）
- `run_pipeline` 薄封装零调用者删除（`run_pipeline_core` 是原 body 逐字搬入，NEUTRAL-001 已自证）
- 三类型 cfg 删除对 Windows 为可证明 no-op（cfg 恒为真，item 本就被无条件包含）

##### 是否需要 Windows 侧同步改动

**否**。平台契约面未变（`platform/mod.rs` 导出符号未新增/修改，`WindowId` 已由 NEUTRAL-001 定义）。

##### 已知限制（macOS 侧）

1. `foreground_window_id()` 返回 0 → `focus_lost` 恒 false（NEUTRAL-001 降级继承），本轮接受，不实现真实版本。
2. 退出路径：原 ctrlc 处理器已被 coder-2 HOST-001 问题A修复移除（预存在删除，非本任务），SIGINT 不触发 `request_stop()`。建议补退出机制（属 HOST-001/OVERLAY-001 范畴）。

##### 验证

`cargo check --all-targets` → 0 errors。`cargo run --bin feiyin-ime` 启动成功（模型加载 + hotkey listener + CFRunLoop 宿主日志确认）。热键闭环未实机（osascript 模拟 F6 不经 CGEventTap 捕获层，需物理键）。spawn_worker_thread body 去空白 md5 自证逐字节保留（仅 :2439-2440 两接触点改动，0 mismatch）。

---

#### 2.10-D · MACOS-P4-EXIT-001 修复 macOS 无干净退出路径（2026-08-04，coder-1）

**本批次结论一句话**：**纯 macOS 平台代码，对 Windows 零影响**——无需 Windows 侧任何同步改动。

##### 改动了什么

- 文件：`src/main.rs`（仅此一个）
- 内容：补回 SIGINT/SIGTERM 信号处理（HOST-001 问题 A 移除 ctrlc 的补救）。B1 方案：不引 crate，用 `libc::signal`（`libc = "0.2"` 已在 macOS 段）。新增 `MACOS_SIGINT_RECEIVED` static + `macos_signal_handler` extern fn + `install_macos_signal_handler` fn，全部 `#[cfg(target_os="macos")]`。`run_controller_macos` 开头调用。
- 语义：handler 只置 AtomicBool（async-signal-safe），轮询线程在普通上下文调 `platform::request_stop()`（CFRunLoop API 非 async-signal-safe，不在 handler 内调）。

##### 行为前后对比

| 状态 | 前（无信号处理） | 后（B1） |
| --- | --- | --- |
| SIGINT | 无反应，只能 `kill -9`，worker 不 join、资源不释放 | handler 置 flag → 轮询线程调 request_stop → CFRunLoopStop → run_controller_macos 收尾（worker Shutdown+join + hotkey shutdown+join）→ 进程退出 + flock 释放 |

##### 对 Windows 侧的具体影响

**零影响**。所有新增代码均在 `#[cfg(target_os = "macos")]` 内，Windows 编译时不参与。`libc::signal` / `SIGINT` / `SIGTERM` 仅 macOS 可见。未改 `Cargo.toml`（选 B1 零依赖）。

##### 是否需要 Windows 侧同步改动

**否**。平台契约面未变（未新增/修改 `platform/mod.rs` 导出符号，仅调用既有 `platform::request_stop()`）。

##### 实机验证

① `cargo run` → `kill -INT $PID` → 完整退出链：`SIGINT/SIGTERM received, requesting controller stop` → `macOS logic thread exiting` → `macOS controller loop exited cleanly` → 进程 EXITED，无残留。② 退出后立即再启动：正常运行（flock 已释放，无 "Application already running"）。

---

#### 2.10-E · MACOS-P4-OVERLAY-001 macOS 录音浮层 1:1 复刻 Windows（2026-08-05，coder-1）

**本批次结论一句话**：**新增纯 macOS 新文件，对 Windows 零影响**——无需 Windows 侧任何同步改动。

##### 改动了什么

- 文件：`src/platform/macos/overlay.rs`（新增）+ `src/bin/probe_overlay.rs`（临时验证 bin）
- 内容：macOS 录音浮层（Recording 状态 P0）。`NSPanel`（`NSBorderlessWindowMask | NSNonactivatingPanelMask`，≈ Windows `WS_POPUP|WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE`）+ `setLevel(NSStatusWindowLevel)`（≈ TOPMOST）+ 透明背景（≈ LAYERED）。自定义 `OverlayView`（`declare_class!` 子类化 NSView，`isFlipped`→true 左上原点 + `drawRect:`）经 `NSGraphicsContext::graphicsPort()` → `CGContext` 用 core-graphics 绘制。60fps 由 16ms `CFRunLoopTimer` 驱动。
- 视觉 1:1：240×36 / 圆角 10 / 底部上方 64px / `#0D0F11` 背景 + `#070606` 边框（COLORREF 0x060607=0x00BBGGRR→RGB #070606，返工修正 R/B）/ 18px 三态指示灯（红>橙>灰）/ 32 柱中心对称波形（3px 宽 2px 间隔，maxh 48 / static 12 / min 8，gain 2.5，衰减 0.02）/ 左右分隔条 + 停止按钮（对应 Windows `main.rs:1304-1344`）。
- API：`RecordingOverlay::new(levels)` / `show()` / `hide()` / `is_visible()` / `stats()`（性能快照）/ `destroy()`，Drop 自动收尾。

##### 行为前后对比

| 状态 | 前（无 macOS overlay） | 后（OVERLAY-001） |
| --- | --- | --- |
| 录音浮层 | macOS 无浮层（main.rs macos_stubs 空实现） | 240×36 透明浮层显示主屏底部上方 64px，三态指示灯 + 60fps 波形动画，视觉与 Windows 一致 |

##### 对 Windows 侧的具体影响

**零影响**。新增文件为 `src/platform/macos/overlay.rs`（纯 macOS 代码，cfg 门控），未触碰 `src/main.rs`、`platform/mod.rs`、`windows/**`。Cargo.toml 的 overlay feature（`NSPanel`/`NSWindow`/`NSView`/`NSResponder`/`NSScreen`/`NSColor`/`NSGraphics`）由主控添加，本任务未改该文件。

##### 是否需要 Windows 侧同步改动

**否**。未新增/修改 `platform/mod.rs` 导出符号。接线（替换 main.rs macos_stubs 的 OverlayCommand/OverlayRequest/OverlayThreadHandle 为真实调用）留给后续任务。

##### 实机验证（2026-08-05 返工后修订）

`cargo run --bin probe_overlay 30`（音频模拟线程 16ms 推流 + 主线程 `NSRunLoop runUntilDate:`，与真实主线程一致）→ 30s 无 panic，真实渲染 **1876 帧 ≈ 62.5 fps**。**性能四项**：① avg 帧间隔 **16.00 ms**（max 18.03ms）② avg 单帧绘制 **0.022 ms**（max 0.083ms，要求 <5ms）③ 录音期间 CPU **1.3–1.5%** ④ 音频 push 间隔 avg 16.003ms/max 23.989ms **无丢帧**。`cargo check --all-targets` 0 errors；`cargo test --bin probe_overlay` 13 passed；`cargo fmt --check` clean。

> **初版测量口径勘误**：初版报告的"59.1/59.0 fps"是 probe 自身 push 计数（当时 `sleep` 泵不出 CFRunLoopTimer，overlay 从未真正渲染）。本次 runUntilDate 实测为真实帧率。同时修复初版隐藏缺陷：`ACTIVE_OVERLAY` 存 `new()` 栈帧指针，move 后悬垂 → run loop 下 drawRect SIGBUS，重构为 `OverlayIvars` 直接持有 `Arc<AudioLevelBuf>` 消除裸指针。
>
> **⚠️ 指示灯形状待主控裁定**：Windows `main.rs:1099-1167` 实际是麦克风图标（pill 28×49 全圆角 + stem + base，4x 超采样缩 18px）非纯圆；规格表写"直径 18px"。当前实现为实心圆。基准以规格表为准还是 Windows 代码为准，待主控裁定。

---

## 3 · 构建环境陷阱【必读，否则会白折腾】

### 3.1 [CT2-SUBMODULE-DEADLOCK-001] ctranslate2-sys 构建树残缺后永不自愈

`ctranslate2-sys` 的 build.rs（`patches/ctranslate2-sys/build.rs:450-470`）下载 CTranslate2 源码 **tarball**（不含 git submodule 内容），再对 `third_party/` 下 7 个依赖逐个 `git clone`，并在 `submodules.rs:125` 对退出码 `assert`。

**陷阱机制**：
1. 首次运行若中途失败（cutlass 是大仓，易被 HTTP/2 CANCEL），留下**部分成功的残缺树**：先克隆成功的目录有内容，其余为空
2. 再次运行时，helper 仍从第一个依赖开始 clone → 撞上「目录已存在且非空」→ git 返回非零 → assert 失败 → panic
3. **重试 1 次和 100 次的报错完全相同，且报错指向的永远是第一个已成功的目录，与真正失败的那个无关** ← 这是本坑最强的误导性

> **真实案例（我们自己的教训）**：本批次 coder-1 被 `cargo check` 阻塞，报错前一轮出现过 `RPC 失败 curl 92 / HTTP/2 stream CANCEL`，于是判定为"网络抖动，重试即可"。**两者都不对**——真正根因是残缺树，重试无限次结果不变。这个误判让我们浪费了多轮。你们用的是同一份 build.rs + 同一个 helper crate，**必然会撞上**，别再踩。

**修复（两步，缺一不可）**：

```bash
# 步骤 1：治网络（cutlass 等大仓易被 HTTP/2 CANCEL）
git config --global http.version HTTP/1.1
git config --global http.postBuffer 524288000
#   还原：git config --global --unset http.version

# 步骤 2：删掉所有【非空】的 third_party 子目录，让 clone 能重新写入
#   删除前先存清单（不可逆操作纪律）：
cd target/debug/CTranslate2-4.6.0/third_party
find cpu_features -printf '%p %s\n' | sort > /tmp/cpu_features-before-delete.txt
rm -rf cpu_features   # 只删非空的那些；空目录不必删，clone 可写入空目录
```

**诊断口诀**：报错说「A 目录已存在」时，**真正失败的是 A 之后的某个目录**。用 `for d in third_party/*/; do echo "$(ls $d|wc -l) $d"; done` 一眼看出哪些空、哪些满 —— 满的是上次成功的，**第一个空的才是上次的失败点**。

依据：`collab/troubleshooting.md:1666` [CT2-SUBMODULE-DEADLOCK-001]。

### 3.2 [DISK-CLEANUP-001] 禁用 cargo clean

`cargo clean` 会连带删除 `target/release/` 下的词库与配置。磁盘清理必须逐目录 `rm -rf`，不要一键 clean。

依据：`collab/troubleshooting.md:1513` [DISK-CLEANUP-001]。

---

## 4 · checkout 后无法直接构建的既有缺口与交接请求

### 4.1 sherpa-onnx 预编译库未入库

`sherpa-onnx` 预编译库**未入库**（`.gitignore:22` 排除 `/vendor/sherpa-onnx/*-Release/`，`git ls-files sherpa-onnx-lib/` 为 0）→ **任何平台的全新 checkout 都构建不了**，不只是 macOS（依据 `docs/MACOS-PORT-ASSESSMENT.md` §2 第 4 项）。

`.cargo/config.toml:3` 的 `[env]` 把 `SHERPA_ONNX_LIB_DIR` 硬编码为 Windows 本机绝对路径 `D:\Workspace\...\sherpa-onnx-v1.12.38-win-x64-shared-MD-Release\lib`，且按 DEC-033 红线**不得修改**。

**绕法**：cargo 的 `[env]` 默认 `force = false`，**shell 中已导出的同名变量优先**（✅官方 Cargo Book，`docs/BUILD-MACOS.md` §三）。故 macOS 侧 `export SHERPA_ONNX_LIB_DIR=...` 覆盖即可，无需改配置。

### 4.2 交接请求：请把 setup-macos.sh / env-macos.sh 作为第一个 PR 提交

`docs/BUILD-MACOS.md` §一 的「一键初始化」让人执行 `scripts/setup-macos.sh` + `scripts/env-macos.sh`，但**这两个脚本从未提交过本仓库** —— `git ls-files scripts/` 当前只有 `backup-docs.ps1` / `build-macos.sh` / `init-publish.ps1` 三个。

⚠️ 注意其中的 `scripts/build-macos.sh` **不是可用脚本**：它是 2026-04-19 的 394 字节占位，引用已废弃的产物名 `voice-ime`（v0.5.4 已改名 `feiyin-ime`），且无原生库路径处理。**不要以它为起点**，请以你们本机那份 `setup-macos.sh` 为准，并顺带更新或替换这个占位。

**明确请求**：请把这两个脚本作为你们的**第一个 PR** 提交。它们是 macOS 侧任何人能起步构建的前置条件。

我们已提供 Windows 侧对应脚本 `scripts/fetch-sherpa-onnx.ps1`（本批次新增，负责拉取 Windows 预编译包），可作为 macOS 侧脚本的参照。

---

## 5 · macOS 侧待实现清单（TODO 索引）

### 5.1 本批次新增的 `// TODO(macOS team):` 标记

| 位置 | 含义 |
|---|---|
| `src/platform/macos/mod.rs:65` | `notify_config_changed()` 占位 stub，需实现 CFRunLoop wake / Tauri event 真实通知 |
| `src/platform/macos/mod.rs:78` | `capture_scene_signals(_hwnd: usize)` 占位 stub，需实现 NSWorkspace frontmostApp + AXUIElement 场景信号采集 |

### 5.2 既有 P1 缺口（编译通过也跑不起来）

提炼自 `docs/MACOS-PORT-ASSESSMENT.md` §3：

| 缺口 | 现状 |
|---|---|
| 主控入口 `run_controller` | `src/main.rs:2761` 非 Windows 分支仅 `log::warn!` 返回，整个主控未实现 |
| Overlay | Win32 GDI 手绘，macOS 无对等实现；DEC-015 定 Tauri 作事件宿主，但主程序→Tauri 的 overlay 状态推送通道完全没有 |
| 事件循环 | `src/platform/macos/mod.rs:52` `run_message_loop()` 为 stub 直接返回 `Ok` |
| 托盘 | `tray-icon 0.19` 在 macOS 要求主线程 + NSApplication run loop，当前无处建立 |
| 开机自启 | `src/platform/macos/mod.rs:29-35` 返回 `Err("not implemented")` |
| 设置界面拉起 | `src/main.rs:443` 硬编码 `feiyin-ime-ui.exe` |
| Accessibility 弹窗 | `src/platform/macos/accessibility.rs:44` `ax_is_process_trusted_with_prompt()` 仍是 stub，只打日志、不调 `AXIsProcessTrustedWithOptions` → 用户永远看不到授权弹窗，热键静默失效 |

### 5.3 P2 权限与打包缺口

提炼自 `docs/MACOS-PORT-ASSESSMENT.md` §4：

- 全仓库无任何 `Info.plist`、无 `.entitlements`，仅有 `src-tauri/icons/icon.icns`
- `tauri.conf.json` 的 `bundle.targets` 仅 `["msi"]`，无 `dmg`/`app`，无 `bundle.macOS` 配置段
- 签名公证依赖 Apple Developer 账号（$99/年），账号尚未具备 → 只能 ad-hoc 签名本地自用，分发必被 Gatekeeper 拦截
- CTranslate2 动态库 `libctranslate2.dylib` 未被复制到产物目录，出包时需补 `install_name_tool` 或手工复制（依据 `docs/BUILD-MACOS.md` §五）

### 5.4 阶段边界

本批次只做 **A 阶段**（**可编译 + 接缝就位**）。B/C/D 阶段归你们：
- B · 主控可运行（事件宿主 + tray + run_controller 对等实现，跑通「热键→录音→ASR→注入」闭环）
- C · 权限 + 打包（Accessibility 弹窗 + `.app` bundle + Info.plist + entitlements + ad-hoc 签名）
- D · 分发（Developer ID 签名 + notarization + dmg）

依据：`collab/decisions.md:425` DEC-033 第 5 条；`docs/MACOS-PORT-ASSESSMENT.md` §9 工作量估算。

### 5.5 已落地且质量可用的 macOS 代码

`src/platform/macos/hotkey.rs`（CGEventTap + CFRunLoop，448 行，VK→macOS keycode 映射表完整）、`injection.rs`（pbcopy/pbpaste + enigo 兜底，134 行）。录音走 cpal 跨平台，风险最低。可在此基础上继续，无需重写。

> ⚠️ `docs/BUILD-MACOS.md` §四 记录了 2 个 macOS 编译错误（`hotkey.rs:124` CGEventType 不支持 `==`、`hotkey.rs:257` Result 无 `ok_or_else`），属 core-graphics 0.25 API 变更，需改 `matches!` / 转 u32 比较 / `.map_err`。请在跑通 `cargo check` 时一并处理。

### 5.6 场景词表已预置 macOS 自带应用（DATA-SCENE-COVERAGE-004）【2026-08-01 新增】

`scene-rules.toml` 已预置 macOS 自带应用，**双形式并存**（每个应用两条）：

| 形式 A（localizedName） | 形式 B（bundleIdentifier） | 归属 |
|---|---|---|
| `Notes` | `com.apple.Notes` | doc |
| `Reminders` | `com.apple.reminders` | doc |
| `Stickies` | `com.apple.Stickies` | doc |
| `TextEdit` | `com.apple.TextEdit` | doc |
| `Pages` | `com.apple.iWork.Pages` | doc |
| `Mail` | `com.apple.mail` | email |
| `Xcode` | `com.apple.dt.Xcode` | ide_terminal（`multiline_safe=true`） |
| `Terminal` | `com.apple.Terminal` | ide_terminal（`multiline_safe=false`） |
| `iTerm2` | `com.googlecode.iterm2` | ide_terminal（`multiline_safe=false`） |

实现 `capture_scene_signals`（`src/platform/macos/mod.rs:79` 现为 stub 直接 `return None`）时，**第一槽位返回任一形式均可命中**——双形式并存正是为了化解「第一槽位放 `localizedName` 还是 `bundleIdentifier`」的未定项。当前 macOS 侧永不命中（exe 是精确集合匹配），对 Windows 零影响。

**两条硬提醒**：

1. **新增 macOS 应用时，终端类必须进 `multiline_safe=false` 块**（多行 = 逐行执行命令，危险）—— Terminal / iTerm2 已按此归入 false 块，Xcode 是编辑器非终端归 true 块。
2. **不要往 `title_keywords` 加 macOS 应用名**——那是子串匹配，`Notes` / `Mail` / `Pages` 这类通用词进标题匹配会大面积误伤（如任何含 "Notes" 的网页标题都会被重分类为 doc）。macOS 应用只走 `exe` 精确匹配。

---

## 6 · 仓库工程约定

### 6.1 npm 双平台冲突【2026-07-30 依实测大幅修订，原表述已过时】

**原表述**（「macOS 上 `npm install` 会删掉 win32 条目，约定两侧统一用 `npm ci`」）**不准确**，实测修订如下：

| 实测事实 | 证据 |
|---|---|
| `npm ci` 在**两个平台都跑不了**，不是 Windows 独有 | Windows 侧复现 EUSAGE：`Invalid: lock file's @emnapi/wasi-threads@1.2.2 does not satisfy @1.2.3`（node v24.11.1 / npm 11.6.2） |
| macOS 侧 `npm install --package-lock-only` 修复后，**Windows 侧 `npm ci` 依然失败**，报缺 25 条 `@esbuild` 平台条目 | Windows 侧 2026-07-30 实测 |
| **顶层 12 条 win32 条目其实一直都在**，丢的是 `vitest/node_modules/@esbuild/*` 嵌套副本 | 两份 lock 逐条比对 |
| **两份 lock 的包版本完全一致**（vite 5.4.21 / vitest 4.1.5 / esbuild 0.21.5 / 嵌套 vite 8.0.9），无任何依赖漂移 | 逐键比对 |
| 差别纯粹是**两个 npm 小版本对 lock 完整性的记录方式不同** | macOS npm 11.16.0 生成 235 条、含 emnapi 2 条但**无**嵌套 esbuild；Windows npm 11.6.2 生成 259 条、含 23 条嵌套 esbuild 但**无** emnapi 2 条 |

**结论：任何一侧单独生成的 lock 都无法同时满足对方的 `npm ci`。** 「让另一侧重算再提交」只会来回摆动。

**真正的解法**：统一两侧 node/npm 版本（`packageManager` 字段 / `.nvmrc` 钉死），由统一版本的一侧重算 lock，另一侧只做验证。**Windows 侧当初判断「大版本相同、漂移风险低」是错的，你们 §8-1 的顾虑成立** —— 此结论已由实测推翻并在此更正。

当前状态与后续步骤见 `docs/MACOS-NPMLOCK-COORDINATION.md`（你们起草）与 `docs/MACOS-NPMLOCK-WINDOWS-REPLY.md`（本方回执，含五步验证清单与对其步骤 4 的更正）。**Gavin 尚未拍板是否升级 Windows 侧 node，本项阻塞中。**

### 6.2 构建发布走本地流程

Gavin 决定暂不启用 GitHub CI/CD（DEC-033 附则二）。Windows 侧沿用 `collab/build-test-guide.md` 三步流程 + `Publish/` 同步；macOS 侧由你们在本机构建。

### 6.3 版本号铁律

只有 Gavin 能决定改版本号，任何 Agent 不得擅改 `Cargo.toml` / `src-tauri/Cargo.toml` / `tauri.conf.json` 的版本字段。

### 6.4 协作文档体系索引

`collab/` 下（注意：实际路径在仓库内 `voice-ime/collab/`）：

| 文档 | 职责 |
|---|---|
| `todo.md` | 未排期任务列表 |
| `decisions.md` | 技术决策记录（DEC-033 在此） |
| `troubleshooting.md` | 问题与解决方案（CT2 陷阱在此） |
| `handoffs.md` | Worker 任务交接 |
| `progress.md` | 版本里程碑进度 |

---

## 附 · 仓库工程现状速览（2026-07-30）

- 版本：v0.7.2（已交付 Windows 用户）
- `src/` 共 24,504 行，约 70% 天然跨平台，633 个 `#[test]` 绝大部分位于共享核心可直接在 macOS 运行
- `src/main.rs` 4416 行，其中 74 处 `cfg(target_os = "windows")`、仅 1 处 macOS，64% 为 Win32 代码（主控入口 `run_controller` 全 Win32）
- 本批次（MACOS-COMPAT-001）已落地接缝：主程序侧 4 文件 + Tauri 侧（coder-2 并行任务）。编译验证 `cargo check` 0 errors（主控后台跑通，4m43s，86 warnings 无一条指向 platform/ 或 crash/）

依据：`docs/MACOS-PORT-ASSESSMENT.md` §6 代码结构量化；`collab/handoffs.md` 2026-07-29 MACOS-COMPAT-001-CORE 条目。

---

## §ASR-038-A · Qwen-Audio-3.0 流式 ASR 模块新增（2026-08-14）

### 改动范围

| 文件 | 动作 | 平台中立？ |
|---|---|---|
| `src/transcription/qwen_inference.rs` | **新建** 1076 行 | ✅ 是（纯 Rust + tungstenite WS，无 Win32 API） |
| `src/transcription/mod.rs` | `AsrModel` 枚举新增 `QwenAudioOnline` 变体 + `from_config` 映射 + `build_recognizer` match 分支 | ✅ 是 |
| `src/config/mod.rs` | `AudioConfig` 新增 `qwen_asr_url` / `qwen_asr_model` 字段 + 默认值 | ✅ 是 |
| `src/main.rs:3210` | `select_preprocessing_params` match 新增 `QwenAudioOnline` 或模式分支 | ⚠️ 在 `#[cfg(target_os="windows")]` 内，但改动是纯枚举 match 逻辑 |

### 对 macOS 的影响

1. **`AsrModel` 枚举新增变体**：macOS 侧消费 `AsrModel` 的 match 分支需同步——`build_recognizer`（`transcription/mod.rs:639`）已用或模式覆盖 `Qwen3Online | QwenAudioOnline`，macOS 编译同一份代码无需额外改动。
2. **`AudioConfig` 新增字段**：`qwen_asr_url` / `qwen_asr_model` 带 `#[serde(default)]`，macOS 侧 `config.rs` 镜像（`src-tauri/src/config.rs`）需在批次 C 同步新增，本批未碰 `src-tauri/`。
3. **`qwen_inference.rs` 平台中立**：模块内零 Win32 API 调用，依赖 `tungstenite`（跨平台 WS 客户端）+ `serde_json`，macOS 编译同一份代码可直接复用。
4. **`main.rs:3210`**：`select_preprocessing_params` 在 `#[cfg(target_os="windows")]` 内，macOS 侧不编译此函数。但 `AsrModel` 枚举本身是平台中立的——macOS 侧若有 match `AsrModel` 的代码，编译器会要求覆盖新变体。本批已确认 `build_recognizer` 是 macOS 可达的唯一 match 点，已覆盖。

### 结论

**macOS 侧无需同步改动即可编译通过**（`AsrModel` 枚举的 match 已覆盖、config 字段带 serde default）。`src-tauri/src/config.rs` 的字段镜像属批次 C 任务，与本批解耦。

---

## §ASR-038-B-partial · 038-B 收尾（端点缺陷修复 + cancel_signal + preprocessing 结论，2026-08-15）

### 改动范围

| 文件 | 动作 | 平台中立？ |
|---|---|---|
| `src/config/mod.rs` | `default_qwen_asr_url` 改为含 WorkspaceId 的 Inference 端点 + 4 测试 | ✅ 是 |
| `src/transcription/mod.rs` | Transcriber 新增 `qwen_asr_url`/`qwen_asr_model` 字段 + `new` 签名加两参数 + getter + 非流式回退分支用独立字段 + 6 测试 | ✅ 是 |
| `src/transcription/qwen_inference.rs` | `transcribe_streaming` 加 `cancel_signal` 参数 + 上传/接收循环 cancel 检查 + 测试调用更新 | ✅ 是 |
| `src/main.rs` | `spawn_worker_thread` 两处 `Transcriber::new` 调用更新 + 热重载加 `qwen_asr_changed` + `active_qwen_asr_*` 跟踪 + `select_preprocessing_params` 注释扩充 | ⚠️ 在 `#[cfg(target_os="windows")]` 内的调用点，但 `Transcriber::new` 签名变更是平台中立的 |

### 三条端点缺陷（主控独立验证 + 追加两条同族）

| # | 缺陷 | 修法 |
|---|---|---|
| ① | `mod.rs:302` 用 `self.qwen3_url`（Realtime 端点）调 `transcribe_streaming`（Inference 协议） | Transcriber 新增 `qwen_asr_url`/`qwen_asr_model` 独立字段 |
| ② | `qwen_asr_url`/`qwen_asr_model` config 字段全库零消费 | `Transcriber::new` 签名加两参数 + 所有调用点更新 |
| ③ | `default_qwen_asr_url` 主机名 `dashscope.aliyuncs.com` 是 Realtime API 的 | 改为 `wss://llm-kudx4dj2bfqn4gr2.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference` |

### 行为前后对比

- **修前**：QwenAudioOnline 模式复用 Qwen3Online 的 Realtime API 端点 + model，Inference 协议去连 Realtime 端点 → **必然连不上**
- **修后**：QwenAudioOnline 用独立的 Inference API 端点（含 WorkspaceId）+ 独立 model 名 → 协议与端点匹配
- **cancel_signal**：`transcribe_streaming` 新增可选取消信号，EditRequested/ESC 可中断进行中的 WebSocket 转录（修前无法中断）

### 对 macOS 的影响

1. **`Transcriber::new` 签名变更**：加了两参数 `qwen_asr_url`/`qwen_asr_model`。macOS 侧若有 `Transcriber::new` 调用点需同步更新——`spawn_worker_thread` 已去 `cfg(windows)`（NEUTRAL-002），macOS 侧调用同一函数，本批已更新两处调用点，macOS 编译同一份代码无需额外改动。
2. **`AsrModel` 枚举**：A 批已加 `QwenAudioOnline`，本批未改枚举本身。
3. **`config/mod.rs`**：`default_qwen_asr_url` 默认值变更 + 新增测试，平台中立，macOS 编译同一份代码。
4. **`qwen_inference.rs`**：`transcribe_streaming` 签名加 `cancel_signal`，平台中立。
5. **`select_preprocessing_params`**：在 `#[cfg(target_os="windows")]` 内，macOS 侧不编译，无影响。

### 结论

**已评估，对 macOS 侧编译无影响**——`Transcriber::new` 签名变更已被 macOS 可达的 `spawn_worker_thread` 两处调用点同步更新，其余改动平台中立或 macOS 不可达。`src-tauri/src/config.rs` 的 `qwen_asr_url`/`qwen_asr_model` 字段镜像属批次 C，与本批解耦。

### 挂起项（等 Gavin 拍板，macOS 侧暂无需动作）

伪流式 vs 真流式 + VAD 入口门控 + `record_streaming()` + 流式管线接线 —— 这些属架构层，待 Gavin 拍板后可能影响 `src/audio/mod.rs`（`record_streaming()` 可能需平台分支：WASAPI vs cpal），届时 macOS 侧需同步评估。

---

## §TRANS-HOTKEY-039 · 翻译热键全链失效修复（Windows 侧，2026-08-15）

### 改动范围

| 文件 | 动作 | 平台中立？ |
|---|---|---|
| `ui/src/pages/HotkeySettings.tsx` | 修正 `VK_TO_LABEL` 中 Shift 左右标签（0xA0/0xA1 对调） | ✅ 纯前端 |
| `src/platform/windows/hotkey.rs` | 翻译键轮询跟随录音生命周期 + 硬上限兜底 + flag 翻转日志 | ❌ Windows 专用 |
| `src/platform/windows/mod.rs` | re-export `notify_translate_poll_stop` | ❌ Windows 专用 |
| `src/platform/mod.rs` | Windows 侧导出 `notify_translate_poll_stop` | ❌ Windows 专用 |
| `src/main.rs` | 6 处录音终止路径调用 `platform::notify_translate_poll_stop()` | ❌ 在 `#[cfg(target_os="windows")]` 内 |

### macOS 侧现状

- `src/platform/macos/hotkey.rs:137` 与 `:153` 两处硬编码 `translate: Arc::new(AtomicBool::new(false))`，无 `translation_pressed()`、无 poll 线程。
- **结论：macOS 翻译热键从未实现，不是失效，是功能不存在。**

### 若 macOS 侧要对齐 Windows，需做的工作

1. **配置消费**：在 `macos/hotkey.rs` 读取 `config.translation.enabled` 与 `config.translation.vk_code`（当前被忽略）。
2. **按键检测**：为翻译键增加按键检测（CGEventTap 层或轮询 `GetAsyncKeyState` 的 macOS 等价物）。
3. **poll 线程**：按录音生命周期启动/停止翻译键轮询，并将结果写入 `HotkeyEvent::Start { translate }` 的 `Arc<AtomicBool>`。
4. **终止路径**：与 `main.rs` 中的 ESC / 录音结束 / overlay 取消等路径对齐；macOS 侧 `process_controller_events` 镜像当前只在 `:2843` 之后存在 stub，需先完整实现 Windows 行为镜像。
5. **跨平台导出**：在 `src/platform/macos/mod.rs` 提供 `pub fn notify_translate_poll_stop()` stub（当前不存在），使平台层符号对称。

### 本单决策

- **不实施 macOS 侧代码改动**（任务书明确要求只给结论）。
- Windows 侧修复对 macOS 编译无影响：所有新增符号均位于 `#[cfg(target_os = "windows")]` 路径，`process_controller_events` 本身在 macOS 侧不编译。

---

## §ASR-038-B-streaming · 真流式核心实施（C-1~C-4 + 040-A，2026-08-15）

### 改动范围

| 文件 | 动作 | 平台中立？ |
|---|---|---|
| `src/transcription/vad.rs` | +202 行：VadSegmenter 新增 `try_new_for_streaming`/`accept_and_check`/`reset_for_new_session`/`vad_window_size` + 6 测试 | ✅ 是（sherpa-onnx 跨平台） |
| `src/audio/mod.rs` | +155 行：`record_streaming` 平行方法 | ⚠️ WASAPI/cpal 采集，macOS 需评估 cpal 分支 |
| `src/transcription/qwen_inference.rs` | +447 行：`transcribe_streaming_realtime` 真流式 + 040-A 埋点 + 旧 `transcribe_streaming` 也加 040-A 埋点 | ✅ 是（tungstenite 跨平台） |
| `src/main.rs` | +311 行：worker QwenAudioOnline 分支 + `run_pipeline_core` `initial_text` 参数 | ⚠️ `spawn_worker_thread` 已中立（NEUTRAL-002），macOS 可达 |
| `src/config/mod.rs` | 无新改动（B-1 已在 partial 批完成） | ✅ |
| `src/transcription/mod.rs` | 无新改动（B-1/B-2 已在 partial 批完成） | ✅ |

### 行为前后对比

- **修前**：QwenAudioOnline 走 `record()` 录完整段→`transcribe_streaming` 一次性发→回调推增量（伪流式，录音期间无文本上屏）
- **修后**：QwenAudioOnline 走 `record_streaming` 边录边推 chunk→ASR 线程 VAD 门控建连→边发边收→`on_result` 回调推 `StreamingText` 到 overlay（真流式，录音期间实时上屏）
- **非流式路径零改动**：record() + run_pipeline_core(None) 行为不变（FIRSTCHAR 前处理链逐字节等价已自证）

### 对 macOS 的影响

1. **`vad.rs`**：平台中立，macOS 编译同一份代码。`try_new_for_streaming`/`accept_and_check` 可直接复用
2. **`audio/mod.rs` `record_streaming`**：WASAPI/cpal 采集层。macOS 用 cpal（非 WASAPI），`ensure_stream` 已是 cpal 跨平台代码。`record_streaming` 复用 `ensure_stream` + `warm.rx`，macOS 应可直接调用。但需端测验证 cpal 在 macOS 上的 chunk 时序行为
3. **`qwen_inference.rs`**：平台中立（tungstenite WS）
4. **`main.rs` worker 分支**：`spawn_worker_thread` 已中立（NEUTRAL-002），macOS 可达。QwenAudioOnline 分支判断 + 流式管线在 macOS 编译同一份代码。但 `record_streaming` 的设备名传递 + overlay `StreamingText` 事件在 macOS 侧的渲染（038-C）需评估
5. **`run_pipeline_core` `initial_text` 参数**：签名变更影响所有调用点。worker 线程非流式调用传 `None`（macOS 可达，已更新）

### 结论

**macOS 侧编译应无影响**（vad/qwen_inference 平台中立，main.rs worker 分支已中立）。`record_streaming` 的 cpal 分支需 macOS 端测验证。`StreamingText` 事件的 macOS overlay 渲染属 038-C 任务。

### 🔴 行为变更：真流式绕过前导静音裁剪（trade-off 记录）

**变更性质**：QwenAudioOnline 模式从非流式回退路径切到真流式主路径后，`select_preprocessing_params` 返回的 `(0, 3200)`（0ms silence_head + 200ms onset_backtrack）**不再被应用**。

| 项 | 非流式回退路径（旧） | 真流式主路径（新） |
|---|---|---|
| 前导静音裁剪 | `find_speech_onset_with_backtrack` 裁掉语音起始前的静音 | 不裁剪（边录边发，无完整 samples 可 trim） |
| 上传音频量 | 裁剪后仅语音段 | 含前导静音（VAD 命中前缓冲的 pre-roll） |
| 费用影响 | 少 | 多（上传音频变多） |
| 送气声母风险 | 裁剪可能截断 /pʰ/ /tʰ/ 等送气清声母（FIRSTCHAR-FIX 001~006 的起因） | 不裁剪，无截断风险 |

**与 Gavin 省钱指令的关系**：Gavin 今日硬指令「防止无效语音输入上传浪费 token」由 VAD 入口门控解决（纯静音不建连）。前导静音裁剪省的是「有语音但前面有静音」的那段，VAD 门控挡不住这类。真流式多上传的音频量 = pre-roll 缓冲（~600ms）+ VAD 判定窗口（~32ms）+ 握手期间音频（~200-500ms）≈ 0.8-1.1s，相对于一次录音的总时长占比小。

**不裁剪反而更安全的理由**：FIRSTCHAR-FIX 001~006 六轮才把首字率从 ~20% 压到 ~54%，根因就是裁剪截断了送气声母。真流式不裁剪，从机制上消除了这个风险。VAD 入口门控 + pre-roll 补发保证首字不丢。

**端测观察点**：真流式模式下，若发现费用异常增加（相对非流式回退），可考虑在 transcribe_streaming_realtime 的 pre-roll 补发阶段加能量门控（低于阈值的 pre-roll chunk 不发），但这会重新引入送气声母风险，需端测权衡。

---

## §TRANS-HOTKEY-039-D · 抽判据纯函数 + 清理死常量（Windows 侧，2026-08-15）

### 改动范围

仅 `src/platform/windows/hotkey.rs`（+90/-6）：
- 新增 `hotkey_mode_to_u32(mode: HotkeyMode) -> u32` 纯函数（PTT=1/Toggle=0）
- 新增 `should_stop_translate_poll_on_keyup(mode: u32) -> bool` 纯函数
- `keyboard_hook_proc` 的 `if mode == 1` → 调 `should_stop_translate_poll_on_keyup(mode)`
- `install_keyboard_hook` 的 `if mode == PushToTalk { 1 } else { 0 }` → 调 `hotkey_mode_to_u32(mode)`
- 删死常量 `TRANSLATE_WINDOW_MS`（全库零使用）
- +4 真护栏测试（调生产函数）

### 对 macOS 的影响

**已评估，对 macOS 无影响**。`hotkey.rs` 全部位于 `#[cfg(target_os = "windows")]` 门控内，macOS 侧不编译此文件。`HotkeyMode` 枚举本身是平台中立的（config 层定义），但两个新纯函数只在 Windows 的 hotkey 模块内使用，macOS 侧无对应调用点。`TRANSLATE_WINDOW_MS` 原本也只在 Windows 侧使用，删除不影响 macOS。

---

## §ASR-041 · 在线 ASR 模型换代 UI 选项 + 存量配置静默迁移（2026-08-15）

### 改动范围

| 文件 | 动作 | 平台中立？ |
|---|---|---|
| `ui/src/pages/Voice.tsx` | 下拉 value + desc case + 4 处逻辑判断 `qwen3_online` → `qwen_audio_online` | N/A（前端，平台无关） |
| `ui/src/i18n/en.ts` / `zh-Hans.ts` / `zh-Hant.ts` | 去 Qwen3 字样 | N/A |
| `ui/src/pages/Voice.test.tsx` | 17 处 `qwen3_online` → `qwen_audio_online` | N/A |
| `src/config/mod.rs` | load+load_from 存量 `qwen3_online` 静默迁移为 `qwen_audio_online` + 文档注释 | ✅ 是（config 平台中立模块） |

### 行为变更：存量配置静默迁移

**变更**：用户 `config.toml` 中 `asr_model = "qwen3_online"` 在 load 时自动迁移为 `qwen_audio_online` 并落盘保存。

| 项 | 修前 | 修后 |
|---|---|---|
| UI 下拉 | `qwen3_online` 选项 | `qwen_audio_online` 选项（新引擎） |
| 存量 config | `qwen3_online` 可匹配下拉 | `qwen3_online` 不匹配任何 option → 静默迁移为 `qwen_audio_online` |
| 后端 | 已认 `qwen_audio_online`（038-A/B 已完成） | 无变化 |

### 对 macOS 的影响

**`config/mod.rs` 是平台中立模块**，macOS 编译同一份代码。静默迁移逻辑对 macOS 同样生效——macOS 用户的 `qwen3_online` 配置也会自动迁移为 `qwen_audio_online`。这是预期行为（新旧引擎替代是全平台的，不是 Windows 专属）。

`ui/` 前端改动平台无关。macOS 侧 Tauri UI 编译同一份前端代码。

---

## §ASR-041-B · 清除旧在线 ASR 代码路径 + 字段改名通用名（2026-08-15）

### 改动范围

| 文件 | 动作 | 平台中立？ |
|---|---|---|
| `src/transcription/mod.rs` | 删 Qwen3Online 枚举 + from_config 分支 + transcribe 分支 + qwen3 字段/参数 + match 臂 + 测试 | ✅ 是 |
| `src/transcription/qwen3_online.rs` | **整个文件删除**（686 行） | ✅ 是（纯 Rust + tungstenite） |
| `src/transcription/qwen_inference.rs` | f32_to_pcm16_le 搬家来此 + 2 单测 | ✅ 是 |
| `src/config/mod.rs` | 删 qwen3_asr_url/model 配置项 + 改名 qwen3_api_key→asr_online_api_key + qwen_asr_url→asr_online_url + qwen_asr_model→asr_online_model（三字段带 serde alias）+ 删旧测试 + 加 alias 单测 | ✅ 是 |
| `src/main.rs` | Transcriber::new 调用更新 + 热重载删 qwen3_changed + match 臂删 Qwen3Online + 测试更新 | ⚠️ spawn_worker_thread 已中立，macOS 可达 |
| `ui/src/**` | qwen3_api_key→asr_online_api_key + i18n key 改名 + 文案去 Qwen3 | N/A（前端） |
| `src-tauri/src/config.rs` | 补 asr_online_url/model 字段 + alias + asr_online_api_key alias + 删 qwen3_asr_* | ✅ 是（Tauri config 镜像） |
| `src-tauri/src/main.rs` | test_qwen3_asr_connection 调用改用空串占位（旧字段已删） | ✅ 是 |

### 行为变更

| 项 | 修前 | 修后 |
|---|---|---|
| AsrModel 变体 | 4 个（Performance/Accuracy/Qwen3Online/QwenAudioOnline） | 3 个（Performance/Accuracy/QwenAudioOnline） |
| 旧 qwen3_online.rs | 存在（Realtime API） | **删除** |
| config 字段名 | qwen3_api_key / qwen3_asr_url / qwen3_asr_model / qwen_asr_url / qwen_asr_model | asr_online_api_key / asr_online_url / asr_online_model（三字段统一通用名 + serde alias 免疫存量） |
| 存量配置迁移 | qwen3_online→qwen_audio_online（保留） | 同（不受影响） |
| src-tauri config | 缺 qwen_asr_url/model 字段 → Gavin 保存设置静默丢弃 | 已补 asr_online_url/model（修复 038-A 遗留隐患） |

### 对 macOS 的影响

1. **`transcription/mod.rs`**：平台中立，macOS 编译同一份代码。`AsrModel` 枚举变体减少一个，macOS 侧若有 match `AsrModel` 的代码需编译器要求覆盖——`build_recognizer` 已更新（只匹配 QwenAudioOnline），macOS 可达的 `spawn_worker_thread` 已更新调用。
2. **`qwen3_online.rs` 删除**：macOS 侧若引用 `qwen3_online::transcribe_online` 或 `f32_to_pcm16_le` 会编译失败。`f32_to_pcm16_le` 已搬到 `qwen_inference.rs`，macOS 侧引用路径需从 `qwen3_online::f32_to_pcm16_le` 改为 `qwen_inference::f32_to_pcm16_le`——但唯一引用方就是 `qwen_inference.rs` 本身（已改），macOS 侧无其他引用。
3. **`config/mod.rs`**：字段改名 + alias，平台中立。macOS 用户的存量 `qwen3_api_key` 配置通过 alias 正确读入 `asr_online_api_key`。
4. **`src-tauri/src/config.rs`**：Tauri config 镜像，macOS 侧 Tauri 编译同一份代码。补字段 + alias 对 macOS Tauri UI 同样生效。

### 结论

**macOS 侧编译应无影响**——所有改动在平台中立模块内，`AsrModel` match 已更新，`f32_to_pcm16_le` 搬家后唯一引用方已适配。macOS 侧若有遗留的 `Qwen3Online` match 分支，编译器会报 non-exhaustive match 强制修复（这是好事，编译时发现而非运行时）。

---

## §ASR-042 · 在线流式 ASR 采样率修复（StreamingResampler）（2026-08-16）

### 改了什么

`src/audio/mod.rs`（平台中立模块，macOS 编译同一份代码）：

1. **新增 `StreamingResampler` 结构体**：带状态的流式抗混叠重采样器，数学等价于既有 `resample_anti_alias`（windowed-sinc FIR, TAPS=32），但支持逐块喂入。跨块保留历史样本 + 全局 `emitted` 计数，接缝处不截断卷积核。
2. **`record_streaming` 接线**：pre-roll → post-hotkey → 主循环 依次喂进同一实例，主循环 break 后调 `finish()` flush 尾部。新增日志 `Streaming resampler active: {N}Hz -> 16000Hz`。
3. **新增 6 条测试**：批处理/流式等价性、非对齐块长（441）、恒等（16k→16k）、长度、顺序护栏、finish 尾部。
4. **`record()` 路径零改动**，`resample_anti_alias` 函数体零改动。

### 行为前后对比

| 项 | 修前 | 修后 |
|---|---|---|
| `record_streaming` 输出采样率 | 原始麦克风采样率（通常 48000Hz）直推 `on_chunk` | 重采样到 16000Hz 后推 `on_chunk` |
| 在线 ASR 识别结果 | 全错（服务器按 16kHz 解 48kHz 音频） | 正确（采样率匹配） |
| VAD 门控 | 漏检（48kHz 喂 16kHz VAD → safety net 强制建连） | 预期自动恢复（VAD 收到 16kHz），需日志实测确认 |
| 本地模型路径（`record()`） | 不受影响 | 不受影响（零改动） |
| FIR 卷积核边界 | N/A（无重采样） | 不截断（跨块保留历史 + lookahead） |

### 对 macOS 的具体影响

1. **`src/audio/mod.rs` 是平台中立模块**，macOS 编译同一份代码。`StreamingResampler` 纯 Rust 数学，无平台依赖。
2. **macOS 麦克风默认采样率**：macOS 的 cpal/AudioUnit 默认输入采样率通常为 **44100Hz 或 48000Hz**（取决于设备）。本修复对 macOS 同样生效——`record_streaming` 会将 44100/48000 重采样到 16000Hz 喂给在线 ASR。
3. **macOS 侧 `record_streaming` 是否已接线**：需 macOS 团队确认。若 macOS 侧尚未实现流式录音（仍在用 `record()` 批处理路径），则本改动对 macOS 无运行时影响，但编译通过（`StreamingResampler` 是 `pub(crate)`，即使未使用也不会告警）。
4. **VAD（`src/transcription/vad.rs`）**：macOS 侧若已接线 VAD，同样受益于采样率修复。VAD 代码零改动。

### 结论

**macOS 侧编译无影响**（平台中立模块，纯 Rust 数学）。**运行时影响取决于 macOS 侧 `record_streaming` 是否已接线**：若已接线则同步受益（采样率匹配 + VAD 恢复），若未接线则无影响。macOS 团队无需同步改动，但实现流式录音时应直接复用 `StreamingResampler`。

## §ASR-045 · 流式判空取消修复（`should_cancel_on_empty`）（2026-08-16）

### 改了什么

`src/main.rs`（平台中立 `run_pipeline_core` 内，macOS 编译同一份代码）：

1. **新增纯函数 `should_cancel_on_empty(samples, initial_text)`**（`:4288`）：`samples.is_empty() && initial_text.is_none()`。
2. **判空臂改调该函数**（`:4316-4320`）：原来 `Ok(s) if s.is_empty()` 无条件取消；现在只有「无样本 **且** 无流式文本（initial_text=None）」才取消。流式模式（`Ok(Vec::new())` + `Some(text)`）落入 `Ok(samples)` 分支走既有 `initial_text` 路径（ITN→LLM→注入）。
3. **新增 3 条护栏测试**（`mod streaming_empty_samples_tests`，`:5982`）。

### 对 macOS 的具体影响

1. **`run_pipeline_core` 是平台中立共享代码**（NEUTRAL-001/002），macOS 侧同一份编译。**若 macOS 将来接入流式录音并复用 `run_pipeline_core(Ok(Vec::new()), …, Some(text))` 调用形态，本次修复将其一并带上**——原本会被 `s.is_empty()` 无条件取消吞掉，现在能正常走完 LLM 后半段。
2. **macOS 非流式路径零行为变更**：`samples 空 + initial_text=None` 仍走 `Cancelled`（原行为由 `normal_empty_samples_without_text_still_cancels` 用例钉住）。
3. **`samples` 消费点核证对 macOS 同样成立**：`Ok(samples)` 分支内 `samples` 仅在 else 正常转录分支（`:4341-4370`）被引用，流式路径零引用，空数组无 panic 风险。
4. **签名未变**：本次不修改 `run_pipeline_core` 签名（`:4293` 起），macOS 既有调用点零改动。

### 结论

**macOS 侧无需任何同步改动**；若 macOS 侧已接线 `run_pipeline_core` 流式调用，则本次修复自动生效。属「改动落在平台中立模块 → 必须记」情形（§2.7）。

## §OVERLAY-043 · 录音悬浮层显示与流畅度修复（2026-08-17）

### 改了什么

`src/main.rs`（**Windows 专用** `#[cfg(target_os = "windows")]` 代码，`run_overlay_thread` / `draw_overlay_to_dc` / `process_controller_events` 区域）：

1. **拆分录音态绘制函数**：`draw_recording_overlay` → `draw_overlay_chrome`（背景+边框）+ `draw_recording_indicator_and_waveform`（指示灯+波形）+ `draw_stop_button`（停止按钮）。`draw_recording_overlay_with_text` 只调 chrome + 指示灯 + 停止按钮，**不再画波形**，避免文字区被挤压。
2. **右侧按钮复用**：`RecordingWithText` 与 `StreamingEditing` 共用同一个右按钮矩形；录音态显示停止方块，编辑态同一位置改为橙色圆角 ⏎ 提交按钮。删除独立的 submit 绘制段。
3. **延迟合并 + 平滑插值**：100ms 内连续 `StreamingText` 不再把窗口宽度跳回默认 240px，而是把最新目标宽度存入 `target_size`；每帧按剩余距离 25%（至少 1px）插值移动 `current_size`，避免 `SetWindowPos` 瞬跳。
4. **减少无效重绘**：`InvalidateRect` 改 `bErase=false`；WM_PAINT 已双缓冲（CreateCompatibleDC/Bitmap/BitBlt）。加 `needs_repaint` 脏标记，仅在文字变化、状态切换、尺寸目标变化时触发重绘。
5. **停止后门闩**：新增 `STREAMING_STOPPED` AtomicBool，`HotkeyEvent::Stop` / `CancelStop` / ESC / `CancelRequested` / `EditRequested` / `SubmitRequested` 时置位；`RecordingStarted` 复位。`StreamingText` 分支若检测到已停止且未编辑，则忽略晚到的流式尾包，避免把刚切过去的 `FallingToProcessing` 又顶回录音态。
6. **内存 DC 预填充背景**：双缓冲内存位图在绘制前先 `FillRect` 全区域为 `OVERLAY_BG_DARK`，避免未初始化像素暴露垃圾。
7. **编辑态文字同步**：`OVERLAY_EDITING=true` 时，晚到的 `StreamingText` 仍以 `StreamingEditing` 状态更新请求（EDIT 控件内容同步），但不切换窗口状态。

### 行为前后对比

| 项 | 修前 | 修后 |
|---|---|---|
| 文字态波形 | 完整波形占据中央，文字被挤成窄条 | 仅保留左侧指示灯，右侧留给文字 |
| 右按钮 | 停止 + 提交 两个按钮并排 | 同一位置按状态切换语义 |
| 宽度变化 | 100ms 内回退 240px，与算出宽度来回跳 = 突闪 | 保持目标宽度、插值过渡 |
| 重绘 | `bErase=true` 每帧擦背景；16ms timer 无脑 Invalidate | `bErase=false` + 脏标记 |
| 停止后状态 | 晚到流式尾包把 `FallingToProcessing` 顶回 `RecordingWithText` | 门闩拦截尾包 |

### 对 macOS 的具体影响

1. **全部改动在 Windows-only `#[cfg]` 区域内**，macOS 编译不接触这些函数，**零编译影响**。
2. **行为契约需同步**：macOS 侧若将来实现同等的「边说边上屏」overlay，应参考本节的交互语义：
   - 录音带文字态不显示波形，避免文字挤压；
   - 停止/提交共用同一按钮位置，按状态切换语义；
   - 窗口宽度变化做平滑过渡而非瞬跳；
   - 录音停止后忽略晚到的流式识别尾包，防止状态回退。
3. **无平台层契约变化**：未新增/修改 `platform/` 导出符号，未改动 `AppConfig` 字段。

### 结论

**macOS 侧无需任何同步改动**。本改动属「纯 Windows 平台代码且无契约变化」情形，已在 §2.7 要求下明确评估并记录为「无影响」。

### OVERLAY-043-B 补充（2026-08-17）

- 仅将 `interpolate_step` 与 `should_ignore_streaming_text` 两个内联布尔/算术表达式抽为纯函数，调用点替换。
- 新增函数仍位于 Windows-only `#[cfg(target_os = "windows")]` 区域内，macOS 编译零接触。
- 行为零变更，macOS 侧无需任何动作。

### TEST-SYNC-043 补充（2026-08-17）

- 阶段三测试同步：仅新增 `mod overlay_043_interpolate_tests`（`src/main.rs` 测试区，+138 行，生产零改动）。
- 模块带 `#[cfg(target_os = "windows")]` 门控（与 `interpolate_step` / `should_ignore_streaming_text` 同平台约束），macOS 编译完全不包含该模块。
- 纯 Windows 平台测试，macOS 侧无任何动作。

### TEST-EXEC-043 补充（2026-08-17）

- 阶段四全量回归 + 消融实测：cargo test 1040/0/11、Vitest 54、src-tauri 55 全绿；pytest 按书 SKIP（Publish/ 为 BUILD-017 旧包）。
- 消融 A（`delta.abs()` 改回 `delta`）实测 4 条变红（含任务书预期外的 `step_never_exceeds_quarter_of_delta`，主控确认以实测为准）；消融 B（门闩改 `false`）实测仅真值表 1 条变红。
- 两次消融均完整还原，还原后全量复跑 1040/0/11 逐数一致。
- 纯 Windows 平台测试执行，macOS 侧无任何动作。

### OVERLAY-046 补充（2026-08-17）

- **根因**：`src/main.rs:997` Show 分支在 OVERLAY-043 重构中丢失了无条件 `SetWindowPos` / `InvalidateRect`，非流式态（`Recording`/`FallingToProcessing`/`Processing`/`Error`）于 `:991-993` 被赋 `current_size == target_size`，导致 `:1198` 插值门控永不置位，`SetWindowPos` 一次都不执行 → 录音窗口不定位/不定尺寸，完全不可见。
- **修复**：在 Show 分支内 `ShowWindow` 之前恢复无条件 `SetWindowPos(hwnd, request.pos, computed_size, SWP_NOACTIVATE | SWP_NOZORDER)`，并在 `ShowWindow` 之后补 `InvalidateRect(hwnd, None, false)`。`bErase` 保持 `false`（红线 1）。
- **不影响插值**：流式 `RecordingWithText` 的 `computed_size` 与 `:1221` 插值路径同源（均来自 `:930-959` 的 `pending_size`/`target_size`）；Show 时一次性定位到最新目标尺寸，后续 16ms timer 仍按 `interpolate_step` 推进 `current_size → target_size`，动画连续。
- **全部改动仍位于 Windows-only `#[cfg(target_os = "windows")]` 区域内**，macOS 编译零接触、零行为契约变化、`platform/` 导出符号无增减、`AppConfig` 字段无变化。**macOS 侧无需任何动作**。

### BUILD-018 补充（2026-08-17）

- 阶段五出包：OVERLAY-043 全批（`a588509`/`5940e73`/`497131f`/`b499cc3`）进 exe，改动全在 `src/main.rs`。
- 四步构建全执行 + 七项核验全 PASS；三 exe 两副本 sha256 逐一相等、toml 三副本一致、ProductVersion 0.8.0 未动、冒烟 Responding=True 无 panic。
- `interpolate_step` / `should_ignore_streaming_text` 均为 Windows overlay 纯函数，本包为 Windows 产物构建，macOS 侧无任何动作。

### HOTKEY-047 补充（2026-08-17）

- 仅改前端 `ui/src/pages/HotkeySettings.tsx`（+117/-13），两端共用此文件，macOS 端同步生效。
- 修三个根因：① 焦点竞态 `setTimeout(50)` → `useLayoutEffect` + `autoFocus` + `onBlur` 兜底；② 左/右修饰键可单设（keydown 定组合 / keyup 定单键）；③ AltGr 合成的左 Ctrl 用 `getModifierState('AltGraph')` 精确过滤。
- 录制期间实时回显已按下的修饰键（`voiceRecordingPreview`），消灭「按了没反应」。
- 翻译热键侧只同步焦点竞态修复（`useLayoutEffect` + `autoFocus` + `onBlur`），录制规则未改。
- 后端能力边界（macOS 侧 `src/platform/macos/hotkey.rs` 未读，本轮未碰）：UI 层改动平台中立，若 macOS 后端不支持修饰键单键 polling，UI 仍会发出对应 vk/modifiers，届时由 macOS 后端决定是否生效；本任务红线明确不动后端。
- 无 `*.test.tsx`（阶段三 TEST-SYNC-047 由 tester-1 串行派发）。

### OVERLAY-051-A/H 补充（2026-08-17）

- 仅改 `src/main.rs`（+55/-17），改动全部位于 Windows-only `#[cfg(target_os = "windows")]` 区域内，macOS 编译零接触、零行为契约变化。
- **051-A**：`edit_subclass_wnd_proc` 原转发 `DefWindowProcW`（默认窗口过程）→ EDIT 文本存储/绘制/按键/滚动/选区全被绕过（Gavin 端测：文字消失/没法编辑/光标到不了）。改为 `CallWindowProcW(old_proc)` 转发原 EDIT 过程；old_proc 用 `SetPropW`/`GetPropW`/`RemovePropW` 存取于 EDIT 窗口的命名属性 `fyn_edit_oldpro`（`GWLP_USERDATA` 已被 051-D 占用存父窗口 HWND）。`destroy_edit_control` 加 `RemovePropW` 清理。
- **051-H**：去掉 `ES_MULTILINE` 改真正单行 EDIT（保留 `ES_AUTOHSCROLL`），单行模式 Enter 上报 `VK_RETURN`（051-D 回车提交更可靠），`ES_AUTOHSCROLL` 跟随式自动滚动光标可达全部文字，无 `WS_HSCROLL` 滚动条。
- macOS 侧无对应 EDIT 子类化实现（macOS overlay 用 NSView/NSPanel，不是 Win32 EDIT），本改动平台隔离，macOS 侧无需任何动作。

### ASR-055 + WORDBOOK-053-C/D 补充（2026-08-17）

- **ASR-055**：`src-tauri/src/qwen3.rs` 完全重写（旧 Realtime 协议 → Inference 协议），`src-tauri/src/main.rs` 读真实配置。改动在 `src-tauri/`（Tauri 设置 UI 后端），macOS 侧无 Tauri（macOS 用独立 UI），零编译影响。Inference 协议平台中立，macOS 侧若实现测试连接可用同协议。
- **WORDBOOK-053-C**：`src/wordbook/mod.rs` 新增 `is_valid_candidate` 纯函数 + 7 条校验规则。`src/wordbook` 是平台中立模块（SQLite + 纯 Rust 逻辑），macOS 编译同份代码，校验规则自动生效。`log::debug!` 跨平台一致（release 零磁盘 IO）。
- **WORDBOOK-053-D**：脏数据排查只读报告，无代码改动。`db_path()` 在 macOS 侧由 `src/wordbook/db.rs` 的 `cfg` 决议（DEC-011/DEC-032），与本任务无关。

### OVERLAY-051-G-FIN 补充（2026-08-18）

- **改动范围**：`src/main.rs` + `src/transcription/qwen_inference.rs` 两个文件。
- **平台隔离**：所有 overlay 改动（`OverlayCommand::UpdateWordTimings`、`OverlayWindowState.word_timings/tween_audio_origin`、`RecordingWithText` 分支、`reveal_chars_by_timeline`、四个清理点）全部位于 `#[cfg(target_os = "windows")]` 区域内，macOS 编译零接触、零行为契约变化。
- **平台中立部分**（macOS 编译同份代码，行为自动生效）：
  - `src/transcription/qwen_inference.rs` 的 `WordTiming` 结构（4 字段：begin_time/end_time/text/punctuation）、`extract_words()` 纯函数、`StreamingAsrState` 增 `confirmed_words`/`current_words` 字段 + `on_result` 扩 `words` 参数 + `display_words()` 方法。这些是 ASR 状态机的数据层改动，平台中立。
  - `transcribe_streaming_realtime` 回调签名 `FnMut(&str, &[WordTiming])`（macOS 若实现流式 ASR 接线需同步签名）。`transcribe_streaming`（非 realtime 老路径）传空 `&[]` 保持原语义。
- **macOS 侧契约**（若将来实现流式 overlay 需遵守）：
  1. **时间戳驱动，不压缩停顿**（Gavin 三条指示）：词间间隔多长就等多长，无 interval 上限/下限，无 backlog 加速
  2. **words 为空 → 立即全显**（降级路径）
  3. **words 与 display_text 同源累积**：在 `StreamingAsrState` 里与文本一一对应累积（end=true push 进 confirmed 并清空 current；同 id 整体替换 current；新 id 换 id 并替换 current），`display_words()` 与 `display_text()` 字符级对齐。overlay 侧 `UpdateWordTimings` 整体替换，不做合并。
  4. **单调不回退**：`displayed` 只增不减（用 `.max(prev)`）；服务端回撤（target < displayed）→ snap 到 target
  5. **离开 RecordingWithText（松键）→ flush 游标到全长**
- **不可单测项**：`reveal_chars_by_timeline` 是纯函数可单测（阶段三 tester-1 负责）；overlay 调用侧依赖真实 HWND + GDI 消息循环，不可单测。
- **Gavin -debug 实证前暂定参数**：无新常量引入（删掉了 5 个固定速率常量）。时间戳驱动完全依赖服务端 `words[].begin_time`，无任何人为参数。降级路径在 `words` 缺失时立即全显，无延迟参数。下一棒 tester-1 出诊断包 → Gavin 跑一次 `-debug` 看 `words=N` 实际值，用真实数据校准「词表是否覆盖完整文本」「begin_time 差值是否与墙钟对齐」。

### OVERLAY-054-B-FIX 补充（2026-08-18）

- **改动范围**：仅 `src/main.rs`（与 051-G-FIN 同文件，串行无冲突）。
- **平台隔离**：`OverlayRequest` struct 及所有 Show 处理逻辑都在 `#[cfg(target_os = "windows")]` 区域内，macOS 编译零接触。
- **改动内容**：`OverlayRequest.pos` 从 `[i32; 2]` 改为 `Option<[i32; 2]>`。语义：`None` = 调用方没有位置可给，overlay 线程在 Show 时通过 `overlay_geometry(&status, hwnd)` 解析默认位置。Show 端入口 `unwrap_or_else` 兜底，解析后写回 `Some(resolved)`，后续读取点保证 `Some`。
- **macOS 侧契约**：`platform::OverlayRequest`（平台抽象层，`src/platform/mod.rs`）不受影响——它用的是 `OverlayCommand` enum 的跨平台抽象，不直接暴露 `pos` 字段。macOS 若实现 overlay 需自行决定位置解析策略，本改动的 `Option` 语义是 Windows 实现细节。
- **红线遵守**：未动 OVERLAY-046 的无条件 `SetWindowPos`（`:1175` 区域，只是改读 `resolved_pos` 而非 `request.pos[0]`，调用本身保留）；未动 `InvalidateRect bErase=false`；未动 051-G 揭示逻辑 / `interpolate_step` / `STREAMING_STOPPED` 门闩。

### VERSION-059 补充（2026-08-30）

- **改动范围**：版本号升级 v0.8.1 → v0.9.0。仅 3 处手改：`Cargo.toml:3` / `src-tauri/Cargo.toml:3` / `src-tauri/tauri.conf.json:9`；`Cargo.lock` / `src-tauri/Cargo.lock` 由 cargo 自动同步。
- **平台中立**：版本号是平台中立改动，macOS 侧编译同一份 `Cargo.toml`，无需同步改动。
- **`scripts/Info.plist` 核查**：`scripts/Info.plist:20` 的 `CFBundleShortVersionString` / `CFBundleVersion` 当前为 `0.7.3` 占位值，但 `scripts/build-macos.sh:92-147` 打包时从 `Cargo.toml` 动态读取版本号（`grep '^version' Cargo.toml`）并用 `PlistBuddy -c "Set :CFBundleShortVersionString $VERSION"` 覆盖 → `.app` 打包时 `CFBundleShortVersionString` 会自动跟随 Cargo.toml 变为 0.9.0，**不构成需手改的产品版本号**。
- **macOS 侧无独立版本号声明**：核查 `scripts/`（build-macos.sh / env-macos.sh / setup-macos.sh / Info.plist）后确认，除 `Info.plist` 的占位值（由打包脚本动态覆盖）外，macOS 侧无其他独立产品版本号声明。

### ASR-070-FIX 补充（2026-08-30）

- **改动范围**：仅 `src/transcription/qwen_inference.rs`（+27/-4）。`StreamingAsrState::final_text()` 从 `confirmed_sentences.join("")` 改为 `display_text()`（confirmed + current）。
- **平台中立**：`qwen_inference.rs` 是平台中立模块（无 cfg 门控，无平台特定 API），`StreamingAsrState`/`final_text()`/`on_result()` 纯 Rust 逻辑。macOS 侧编译同份代码，ASR-070 修复自动生效。
- **根因**：旧 `final_text()` 丢弃 `current_sentence`，依赖未验证假设「finish-task 后服务端发最后 sentence_end=true 清空 current」。Gavin 端测实证不成立 → 松键时尾部文字丢失。修法 A 让 final_text 返回 confirmed+current。
- **重复计数核查**：`on_result`（:472-491）end=true 时 push confirmed 同时 clear current（原子），无重复风险。macOS 侧若实现流式 ASR 接线同样受益。

### ITN-071/FMT-072 补充（2026-08-30）

- **改动范围**：`itn-rules.toml`（+1，三五成群补进 [protect.idioms]）、`src/itn.rs`（+17，护栏测试）、`src/transcription/qwen_inference.rs`（+4/-6，死代码化简）。`src/llm/mod.rs` 零改动（FMT-072 只取证）。
- **平台中立**：`src/itn.rs` / `src/llm/mod.rs` / `src/transcription/qwen_inference.rs` 均为平台中立模块（无 cfg 门控），macOS 侧编译同份代码自动生效。`itn-rules.toml` 是平台中立规则数据，三副本同步即可（词表类改动不需构建）。
- **ITN-071**：三五成群补词已修复（idioms 段 + 护栏）。一点半点挂起——代码层面 unit_collision_map 应保护（:2442-2453），等 Gavin 实际输入输出定方向。
- **FMT-072**：取证存档挂起。时间线排查三结论（fe69f23 恢复完整有序清单 / 4c8f830 DECISION RULE 未变 / 94bfb0b 只碰 F3c 不碰 F3a），静态分析未找到碰坏有序路径的改动，需 API 验证。

### ITN-071-B 补充（2026-08-30）

- **改动范围**：`itn-rules.toml`（+3/-2，词表挪动+新增）、`src/itn.rs`（+40，护栏测试）。
- **平台中立**：`src/itn.rs` 纯 Rust ITN 逻辑，`itn-rules.toml` 平台中立规则数据。macOS 侧编译同份代码 + 三副本同步即可生效。
- **修复内容**：`一点半点` 从 [protect.unit_collisions] 挪到 [protect.idioms]（第1步最高优先级）；`一点点` 新增到 [protect.function_words]。根因：`decide_conversion` :2275 `is_date_suffix("点")` 把 `点` 当时间后缀误判，导致不在保护集的 `一` 开头词被转。修法在词表层面，未改 check_protection 代码逻辑。

## §OVERLAY-075 · 跨 session 流式文本渗漏隔离（2026-09-03，Windows 侧会话代际）

**本决策不适用 macOS 侧渲染路径（macOS 流式文本本就不渲染），但 `PipelineEvent` 枚举
签名变更是平台中立的，macOS 编译单元必须同步——本批已同步修复。**

- `PipelineEvent::StreamingText` 增加 `u64` 代际首字段（OVERLAY-075，跨 session 隔离，
  Windows 专属机制 `STREAMING_GENERATION` 在 `src/main.rs` cfg(windows) 区）。
- 🔴 **存量破损顺带修复**：macOS 三处（`overlay_request_for_event` 映射 /
  `handle_pipeline_event` 消费 / `overlay_wire_tests` 单测构造）自 OVERLAY-051-G
  给枚举加 word timings 字段起就是 1 字段签名对 2 字段枚举，**macOS 侧此前编译必失败**。
  本批同步为 3 字段新签名，macOS 编译恢复一致（未在 macOS 机器实际验证，需 macOS 端测确认）。
- Windows 侧绘制契约（GDI → Direct2D，DEC-055 / D2D-073）与本条无交互；代际闸门在
  controller 事件分发层（`process_controller_events`），绘制层不感知代际。

## §HOTKEY-078 · AltGr 尾随 keyup 串键修复（2026-09-03，Windows 侧 UI 修复）

**改动文件**：`ui/src/pages/HotkeySettings.tsx`（Tauri 设置页 React，Windows/macOS 共用同一份前端）。

- **性质**：删除 `handleVoiceHotkeyKeyUp` 中 AltRight keyUp 处的 3 行提前清旗代码
  （`altGrSynthCtrlActiveRef.current = false`），让合成 Ctrl 抑制旗的唯一清零点回归
  `resetVoiceRecordingState()`（录制会话级生命周期）。+5 行注释说明缺陷机理。
- **平台影响**：**零编译影响、零行为差异**——纯前端 TSX 逻辑，无平台分支、无 Rust 改动、
  无 `PipelineEvent` 签名变更，macOS 编译单元零接触。
- **修复场景**：Windows 下 AltGr 物理键合成 ControlLeft+AltRight 双事件，旧代码在
  AltRight keyUp 时提前落旗，导致尾随的 ControlLeft keyUp 逃过抑制被录成第二个热键
  （冲突弹窗键名显示 Left Ctrl 而非 Right Alt）。macOS 无 AltGr 合成键序，此缺陷
  在 macOS 上本就不触发，但修复对 macOS 无副作用（旗只在 AltRight 按下时置位）。
- **验证**：`npx tsc --noEmit` 0 error；测试执行与出包归 tester-1（阶段四/五）。

## §HOTKEY-079 · 翻译侧 AltGr 串键修复（2026-09-03，Windows 侧 UI 修复）

**改动文件**：`ui/src/pages/HotkeySettings.tsx`（Tauri 设置页 React，Windows/macOS 共用同一份前端）。

- **性质**：翻译侧热键录制新增 AltGr 合成键裁决——`translationPendingCtrlRef`（翻译侧独占 ref，
  不与语音侧共享状态，HOTKEY-060 红线）+ `handleTranslationHotkeyKeyUp`（新 handler 挂 onKeyUp）。
  `ControlLeft` keyDown 延后裁决（pending=true），`AltRight` 接管则录 Right Alt(0xA5)，
  其自身 keyUp 到来仍 pending 则录 Left Ctrl(0xA2)。
- **生命周期**：pending 唯一清零点 `resetTranslationRecordingState()`（Escape / 失焦 / finalize
  全路由经过），与 HOTKEY-078 语音侧同型陷阱的预防性处理。
- **平台影响**：**零编译影响、零行为差异**——纯前端 TSX 逻辑，无 Rust 改动。macOS 无 AltGr
  合成键序（ControlLeft+AltRight 成对事件），此缺陷在 macOS 上本就不触发；macOS 用户单按
  Left Ctrl 仍走 keyUp finalize 路径，行为一致。
- **验证**：`npx tsc --noEmit` 0 error；测试执行与出包归 tester-1（阶段三写用例/阶段四跑）。

## OVERLAY-086（2026-09-04，coder-2）

**范围**：Bug1 D2D/GDI 圆角填充 + region、Bug2 空包闸门/渲染护栏/时间轴双端锚定、Bug3 宽度插值。

**零编译影响**：全部改动位于 `#[cfg(target_os = "windows")]` 区——`d2d` mod、`draw_overlay_to_dc`、
`draw_processing_overlay`（GDI 兜底）、`apply_overlay_window_region`、`OverlayWindowState`（含新增
`tween_timeline_origin` 字段）、`reveal_chars_by_timeline` 及其全部调用点、`run_overlay_thread` 的
Show/UpdateWordTimings/插值循环。macOS overlay 走 `src/platform/macos/overlay.rs` 独立路径，
`PipelineEvent::StreamingText` 在 macOS 侧不渲染（:5397 区）。

**零行为差异**：`reveal_chars_by_timeline` 虽加参数但函数本体带 `#[cfg(windows)]`；
qwen_inference.rs 的空 display 闸门在 `transcribe_streaming_realtime`（平台中立），macOS 若
未来接 realtime 流式路径将同样受益（空包不转发是纯数据层改进，无平台耦合）。

**macOS 无 AltGr 键序**结论沿用 HOTKEY-078/079 条目；本单不触及输入/热键域。

## D2D-P1（2026-09-04，coder-2）

**Direct2D 为 Windows 独有**：本单全部改动位于 `#[cfg(target_os = "windows")]` 的 `mod d2d`
与 windows-only 绘制函数（`draw_recording_overlay` / `draw_recording_overlay_with_text` /
`draw_stop_button_hit_rect_only` / `text_hit_rect_for`）。`with_d2d`/原语层/两态入口全在该
cfg 区内，macOS 编译不触及。

**零编译影响 / 零行为差异**：macOS overlay 走 `src/platform/macos/overlay.rs` 独立路径；
流式文字的度量与滚动逻辑（GDI measure_text_width）同为 windows-only。macOS 无对应改动需求。

## REFACTOR-088（2026-09-04，coder-2）

纯函数 `streaming_scroll_offset` 及两处调用点（GDI/D2D 绘制路径）均在
`#[cfg(target_os = "windows")]` 区内。Direct2D/GDI 均为 Windows 独有，macOS 零编译影响、
零行为差异；macOS overlay 独立路径不消费该函数。

## REFACTOR-089（2026-09-04，coder-2）

`advance_width` 纯函数与插值区调用点均在 `#[cfg(target_os = "windows")]` 区内
（`run_overlay_thread` 是 windows-only 线程主循环）。macOS 零编译影响、零行为差异；
macOS overlay 独立路径不消费该函数。
