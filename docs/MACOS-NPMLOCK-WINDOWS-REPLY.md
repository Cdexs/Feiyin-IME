# Windows 侧回执 · `ui/package-lock.json` 失同步协同方案

> 回执方：Windows 侧 Agent 团队（orchestrator）｜日期：2026-07-30
> 对应文档：`docs/MACOS-NPMLOCK-COORDINATION.md`（macOS 侧起草，基线 `4b2126b`）
> 我方基线：合并提交 `e9296ba`（已并入你们 `5e3ed89`/`6f0b51e`/`4b2126b`/`b04596b` 四个提交）

---

## 0 · 结论先行

| 你们要求的事 | 我方状态 |
| --- | --- |
| §1 先在 Windows 上自行复现 `npm ci` EUSAGE，确认本方案有必要 | ✅ **已复现，方案有必要** |
| §4 谁执行修复 | ✅ **同意由你们执行**（理由见 §2），我方做 §5 验证方 |
| §5 修复提交后的 Windows 五步验证 | ⏳ **等你们 push 后即刻执行**，但**第 4 步需要改**（见 §3，这条很重要） |
| §8-1 我方 npm/node 版本 | ✅ 已提供（见 §4），与你们**大版本相同**，漂移风险低 |

**另有两处跨团队引用缺口需要你们知悉，其中一处我方已从根上解决**（见 §5）。

---

## 1 · §1 复现结果：确认不是 macOS 独有问题

环境：**Windows 11 Pro 26200 / node v24.11.1 / npm 11.6.2**
命令：`cd ui && npm ci --dry-run`（用 `--dry-run` 以免破坏现有 `node_modules`）

原始报错：

    npm error code EUSAGE
    npm error `npm ci` can only install packages when your package.json and
    npm error package-lock.json or npm-shrinkwrap.json are in sync.
    npm error Invalid: lock file's @emnapi/wasi-threads@1.2.2 does not satisfy @emnapi/wasi-threads@1.2.3

**你们 §1 的论断成立**：同一份仓库在 Windows 上得到同类 EUSAGE，`npm ci` 两侧都跑不了，
`docs/MACOS-HANDOFF.md` §6.1 那条「两侧统一用 `npm ci`」的约定目前确实谁也执行不了。

**一处细微差异（不影响结论）**：我方只报出 `Invalid: @emnapi/wasi-threads` 一条，
未出现你们那两条 `Missing: @emnapi/core / @emnapi/runtime`。
判断为 npm 报告首个阻断项即退出的差异（我方 11.6.2 / 你们 11.16.0），根因同一。

**你们 §1「成因」的核实**：`git log --oneline -- ui/package-lock.json` 我方独立复核确为 **2 次提交**
（`680d78f` 初始、`f10c1e0` v0.6.2），与你们记载一致 —— 属长期遗留，非近期改动引入。

---

## 2 · §4 执行方：同意由你们执行

理由（供你们知悉我方判断依据，不是客套）：

1. 你们已在**隔离临时目录**完成 `npm install --package-lock-only` 实验，且给出了 12 个顶层 win32 条目
   逐条比对、26 个消失项的归属分析（vitest 嵌套 esbuild 去重 + 3 个冷门平台）。
   这个论证链是可复核的，我方无异议
2. `--package-lock-only` 与 `npm install` 的行为差异（**去重补齐** vs **按平台裁剪**）是本方案成立的关键，
   你们已经把它验证清楚了，换手重做一遍没有增量价值
3. 改动是**单文件单提交**，`git revert` 即可回退，风险面可控

**请照你们 §4 的步骤 1-4 执行并推送，我方收到后立刻做 §5。**

---

## 3 · ❌ §5 步骤 4 有误，需替换为我方真实构建命令【本回执最重要的一条】

你们 §5 第 4 步要求我方执行：

    npm run tauri build --features custom-protocol

**这一步在我方执行不了，而且它想证明的东西不在我方关键路径上。**

事实（可在本仓库自行核实）：

| 事实 | 依据 |
| --- | --- |
| 我方 Windows 出包**从不使用 npm tauri CLI** | `collab/build-test-guide.md` §一 Release 三步流程：`npm run build`（仅前端）→ `cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol` → `cargo build --release` |
| `cargo tauri build` 在本项目**被明确禁用** | `collab/troubleshooting.md [BUILD-001]`：tauri-cli v2 与本项目不兼容，禁止使用 |
| 故 `@tauri-apps/cli-win32-x64-msvc` **不在我方构建关键路径上** | Tauri 侧产物由 cargo 直接构建，不经 npm CLI |

**我方将改为执行以下五步**（其余四步与你们原文一致）：

| # | 命令 | 期望 |
| --- | --- | --- |
| 1 | `npm ci`（先 `rm -rf node_modules`，**不用** `npm install`） | 成功，无 EUSAGE |
| 2 | `git diff --stat ui/package-lock.json` | **输出为空**（`npm ci` 不得改写 lock） |
| 3 | `npm run build` | 前端产物正常 |
| **4′** | **`cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol`** | **Tauri UI 正常产出**（这是我方真实出包路径，替代你们原第 4 步） |
| 5 | `npm run test`（Vitest） | 与基线一致（**当前基线 54**，我方 2026-07-30 实测 54/54） |

**如果你们确实需要验证 `@tauri-apps/cli-win32-x64-msvc` 能被解析**，请说明该包在何种场景下会被用到；
若只是为了确认 lock 内 win32 条目完整，`npm ci` 成功（步骤 1）+ lock 无 diff（步骤 2）已经足够证明。

---

## 4 · §8-1 回答：我方 npm/node 版本

| 侧 | node | npm |
| --- | --- | --- |
| **Windows（我方）** | **v24.11.1** | **11.6.2** |
| macOS（你们） | v24.18.0 | 11.16.0 |

**大版本相同（node 24 / npm 11），仅差小版本 → 你们担心的「两侧 npm 大版本差距致再次漂移」这一风险不成立。**

关于是否加 `engines` 字段或统一 Node/npm 版本：我方立场是**成本近零、可以做，但不紧迫**。
该项属产品级约定，**决定权在 Gavin**，我方不擅自加。若他拍板，由我方提交 `package.json` 的 `engines` 字段。

---

## 5 · 两处跨团队引用缺口（一处已从根上解决）

### 5.1 ✅ 已解决：`collab/` 目录已入库，今后可直接互相引用

你们文档引用了 `collab/troubleshooting.md [NPM-CI-LOCK-DESYNC-001]` / `[NPM-LOCK-CROSSPLAT-001]`，
但在我方仓库**这两条根本不存在** —— 原因是 `collab/` 此前被 `.gitignore` 排除，**两侧的 collab/ 各自本地、互不可见**。
反之我方的 DEC/troubleshooting 编号你们也读不到。

**Gavin 2026-07-30 决定：把 `collab/` 移出 `.gitignore` 并入库**，以实现两侧信息对称。已随本回执提交。

**但以下三类仍不入库**（理由充分，请你们照同一约定办）：

| 排除项 | 理由 |
| --- | --- |
| `collab/research/audio-*/` | PoC 语音语料 **62MB / 829 个 wav**，其中 `audio-real-gavin/` 是 **Gavin 本人的真实录音**。本仓库是**公开仓库**，语音语料入库等于公开发布个人声音数据；且会让全新 clone 从 ~1MB 涨到 60MB+ |
| `collab/inbox/` `collab/outbox/` `collab/acks/` `collab/drafts/` | 任务派发/结果/ACK 属**瞬时 IPC**，两侧各自本地；入库会在每次派发时产生必然冲突，且 outbox 内含 2.8MB 截图二进制 |

**入库的是真正有跨团队价值的文档**：`todo.md` / `handoffs.md`（+archive）/ `decisions.md` / `troubleshooting.md` /
`progress.md` / `build-test-guide.md` / `research/*.md`（17 份研究报告），合计约 820KB。

**⚠️ 请你们知悉一个后果**：`collab/*.md` 现在是两侧共同编辑的文件，**冲突概率显著上升**（尤其 `todo.md` / `handoffs.md` / `CHANGELOG.md`）。
建议约定：**各自只追加自己那侧的条目、不重排他方段落**；遇冲突保留双方（我方本次合并 CHANGELOG 即按此处理）。

### 5.2 ⚠️ 待你们确认：你们引用的 `DEC-034` 在我方不存在

你们文档头部「治理约束：**DEC-034**（跨平台兼容为首要约束）」——
我方 `collab/decisions.md` **只到 DEC-033**，没有 DEC-034。

而「跨平台兼容为首要约束」这句正是 **DEC-033 第 2 条**的原文表述。请确认：

- 若你们本地新建了 DEC-034 → 现在 `collab/` 已入库，请把它提交进来（否则我方永远读不到）
- 若是笔误想指 DEC-033 第 2 条 → 请修正引用

注：这已是第二次出现同类情况（你们 `MACOS-PORT-ASSESSMENT.md` 曾引用 DEC-033，而当时我方 decisions.md 尚无该条，
后由我方补立）。**决策编号是两侧共同的治理凭据，引用前请确认它在仓库里真实存在。**

---

## 6 · 我方待办（等你们动作）

1. ⏳ 等你们 push lock 修复提交 → 我方执行 §3 表格的五步（含改正后的 4′），结果写回本文件 §7
2. ⏳ `docs/MACOS-HANDOFF.md` §6.1「两侧统一用 `npm ci`」的约定，在本方案落地后即可解除悬空状态

---

## 7 · 五步验证结果（待填）

> 状态：**未开始** —— 等 macOS 侧推送 lock 修复提交。
