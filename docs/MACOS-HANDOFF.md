# macOS 团队交接文档 · Feiyin Voice Input

> 编写：Windows Agent 团队 ｜ 日期：2026-07-30 ｜ 依据：DEC-033 及其两条附则
> 读者：macOS 侧 Agent 开发团队（全新接手，未参与过本仓库开发）
> 目的：让你第一天 clone 仓库就能避开我们踩过的坑，并清楚跨平台协作的硬约定

---

## 0 · 先读这份，再读那两份

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
| 027-F | 027-E 引入的静默归零：隐式尾数吸收未做纯数字校验，带单位串进 `format_currency_chain` 的 `.parse()` 静默归零（`五块三亿`→`5元`） | **金额静默改错** |

**027-F 跨端提示**：`parse_cn_number` 契约变化后（027-D/E 返回带单位串），macOS 侧若复用同一份 `src/itn.rs`，隐式尾数吸收处（`:1130`）的纯数字校验已共享修复。但若 macOS 侧有**独立的数字消费逻辑**（如 AX 无障碍读屏提取数字），需同样校验 `parse_cn_number` 返回值是否纯数字后再做数值运算。

**BUILD-014 产物状态（tester-1 补记，2026-08-04）**：027 全批（A/B/C/C-2/D/E/F）已在 Windows 侧完成全量回归（itn:: 212/0）并首次进入 release 产物（feiyin-ime.exe `0e13cff5…`），平台中立模块 `src/itn.rs` 行为与上表一致；macOS 侧沿用共享修复即可，无新增平台差异。

**共同模式**（见 `collab/troubleshooting.md` 的 `[ITN-LOCAL-RULE-OVERREACH-001]`）：

> **局部特例规则没有约束自己的适用范围，在更长的上下文里越界生效。**

四个月内至少四次（016/026/027-A/027-C），**全部由 Gavin 端测撞出，没有一次是测试先抓到** —— 因为单测按设计场景写，缺陷只在设计场景之外显形。027-F 则是 tester-1 走查先发现（首次非端测），主控复核后判定为真风险。

**确立的防御规则**：任何「隐式/省略成分补全」类规则，必须显式声明适用边界，并配反向护栏测试证明它在边界外不生效。**只测生效侧 = 没测。** macOS 侧若新增此类规则，请同样遵守。

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