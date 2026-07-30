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
  → 三分支：(a) LLM 成功 / (b) LLM 失败兜底 / (c) LLM 关闭
  → itn::normalize_numbers(final_text)      ← 必须在此位置
  → 本地标点引擎（仅 !llm_handled 时运行）
  → 注入
```

**三条硬约束**：

1. **ITN 必须在 LLM 之后**。原因：ASR 会把「摄氏」误听成「摄息/摄斯/摄四」（Windows 侧实测 11 次温度类听写只有 2 次听对），ITN 的 ℃ 规则字面匹配「摄氏」，**只有等 LLM 纠正错字之后**才可能触发
2. **ITN 必须在标点引擎之前**。原因：标点模型（CT-Transformer）在 ASR 转写风格文本上训练，若先加 ℃/阿拉伯数字再喂给它，输入即分布外。**模型的分布外退化无法用规则修复，只能观测**；而 ITN 是确定性规则引擎，输入域变化可用规则+单测补齐。**把不确定性留给可控的一侧**
3. **三条路径都必须经过 ITN**。特别是 (b) LLM 运行时失败兜底 —— 漏了这条，用户会看到纯汉字数字（「四十摄氏度」），**比不修更差**

**若你们实现 macOS 管线时把 ITN 放回 LLM 之前，℃ 缺失的 bug 会在 macOS 上完整复现一遍。** 完整决策记录见 `collab/decisions.md` **DEC-035**（该条同时声明 DEC-030「转录后/LLM 前」的原文已作废）。

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