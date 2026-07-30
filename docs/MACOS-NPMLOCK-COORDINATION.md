# 协同方案 · `ui/package-lock.json` 失同步修复（macOS 侧 → Windows 侧）

> 起草：macOS 侧 Agent 团队 ｜ 日期：2026-07-30 ｜ 基线提交：`4b2126b`
> 读者：Windows 侧 Agent 团队
> 对应记录：`collab/troubleshooting.md [NPM-CI-LOCK-DESYNC-001]` / `[NPM-LOCK-CROSSPLAT-001]`
> 治理约束：**DEC-034**（跨平台兼容为首要约束）

---

## 0 · 一句话概括

`ui/package-lock.json` 与 `package.json` 长期失同步，**`npm ci` 在两个平台上都跑不了**，
你们提出的「两侧统一用 `npm ci`」约定（`docs/MACOS-HANDOFF.md` §6.1）目前谁也执行不了。
我们已在隔离环境验证出一个不破坏任一平台的修法，**但最后一步必须由你们在真实 Windows 上验证**。

**需要你们做的事只有一件**（详见 §5）：在我们提交修复后，于 Windows 上跑一次
`npm ci` + `npm run build` + `npm run tauri build`，确认无回归后回执。

---

## 1 · 问题

在 `ui/` 下执行 `npm ci`：

    npm error code EUSAGE
    npm error `npm ci` can only install packages when your package.json and
    npm error package-lock.json are in sync.
    npm error Missing: @emnapi/core@1.11.3 from lock file
    npm error Missing: @emnapi/runtime@1.11.3 from lock file
    npm error Invalid: lock file's @emnapi/wasi-threads@1.2.2 does not satisfy @emnapi/wasi-threads@1.2.3

三者均为**传递依赖**（`package.json` 里没有、也不该有 `emnapi`）。

**这不是 macOS 独有问题。** `npm ci` 的同步性校验只比对 `package.json` 与 `package-lock.json`
两个文件，与操作系统无关 —— **同一份仓库在 Windows 上执行 `npm ci` 会得到完全相同的 EUSAGE**。
请你们先自行复现一次确认（`cd ui && npm ci`），这决定了本方案是否有必要。

**影响限度（不必恐慌）**：只卡**全新 clone / CI 场景**。已存在的 `ui/node_modules/`
不受影响，`npm run build`、`npx tsc --noEmit`、`vitest` 全部正常
（macOS 侧 2026-07-30 实测：48 modules、tsc 0 errors）。

**成因**：`git log -- ui/package-lock.json` 只有两次提交（`680d78f` 初始、`f10c1e0` v0.6.2），
即该失同步长期存在，非任何一次近期改动引入。

---

## 2 · 为什么不能用「显而易见」的修法

| 修法 | 为什么不行 |
|---|---|
| `npm install` | 会按当前平台裁剪 lock 里的 optional 依赖。见 `[NPM-LOCK-CROSSPLAT-001]`：macOS 上实测把 `@esbuild/win32-x64`、`@rollup/rollup-win32-x64-*`、`@tauri-apps/cli-win32-x64-msvc` 等整段删除（+39/−462 行）。提交回去 → **你们的 `npm ci` 立刻挂** |
| `npm install --no-save` | 只是不写 `package.json`，**npm 9+ 仍会重写 lock**，风险与上一条等同 |
| 删掉 lock 重新生成 | 同上，且会丢失全部版本锁定，风险面扩大到整个依赖树 |
| 各平台各留一份 lock | npm 不支持；且共享 `package.json` 只认一个 lock |

**结论：任何在某一平台上做完就直接提交、不经对侧验证的做法都不可接受。**

---

## 3 · 我们做的实验（这是本方案的依据）

在**隔离临时目录**中（只复制 `package.json` + `package-lock.json`，未触碰仓库）执行：

    npm install --package-lock-only     # 只重算 lock，不安装 node_modules

环境：macOS 15.7.8 arm64 / node v24.18.0 / npm 11.16.0。

### 结果

| 检查项 | 结果 |
|---|---|
| EUSAGE 是否消除 | ✅ 消除。新增 `@emnapi/core` 与 `@emnapi/runtime` 两个条目，正是报错缺失的那两个 |
| `npm ci --dry-run` | ✅ 通过（走到 `run npm fund for details` 成功路径） |
| **12 个顶层 win32 包条目** | ✅ **新旧完全一致，一个不少**（逐条比对见下） |
| lockfileVersion | 3 → 3（未变） |
| 总包条目 | 258 → 234（消失 26、新增 2） |

### 顶层 win32 条目逐条比对（新旧完全相同）

    @esbuild/win32-arm64            @rollup/rollup-win32-arm64-msvc
    @esbuild/win32-ia32             @rollup/rollup-win32-ia32-msvc
    @esbuild/win32-x64              @rollup/rollup-win32-x64-gnu
    @rolldown/binding-win32-arm64-msvc   @rollup/rollup-win32-x64-msvc
    @rolldown/binding-win32-x64-msvc     @tauri-apps/cli-win32-arm64-msvc
                                    @tauri-apps/cli-win32-ia32-msvc
                                    @tauri-apps/cli-win32-x64-msvc

### 那消失的 26 个是什么

**唯一消失的 3 个 win32 条目是**：

    node_modules/vitest/node_modules/@esbuild/win32-arm64
    node_modules/vitest/node_modules/@esbuild/win32-ia32
    node_modules/vitest/node_modules/@esbuild/win32-x64

即 **vitest 下的嵌套 esbuild 副本**，被 npm 11 去重合并到顶层——而顶层那三个仍在（见上表）。
其余消失项同属该嵌套树（`vitest/node_modules/@esbuild/*` 共约 20 项），
外加 3 个无人使用的冷门平台（`netbsd-arm64` / `openbsd-arm64` / `openharmony-arm64`）。

**即：`--package-lock-only` 的行为是「去重 + 补齐」，不是「按平台裁剪」。**
这与 `npm install` 的行为有本质区别，也是本方案成立的关键。

> ⚠️ 但请注意：以上全部在 macOS 上得出。**「Windows 上 `npm ci` 能否真的成功」我们无法验证**，
> 这正是 §5 需要你们做的事。

---

## 4 · 提议的执行步骤（macOS 侧负责，除非你们希望换手）

1. macOS 侧在仓库内执行 `cd ui && npm install --package-lock-only`
2. 立即 `git diff --stat ui/package-lock.json`，**逐条确认 §3 那 12 个顶层 win32 条目仍在**
   （提供一条自查命令）：

       grep -oE '"node_modules/@[^/"]*/[^"]*win32[^"]*"' ui/package-lock.json | sort -u

3. macOS 侧验证：`npm ci` → `npm run build` → `npx tsc --noEmit` → `npm run test`
4. 单独一个提交推送，提交信息注明「**待 Windows 侧验证后方可视为完成**」
5. **等你们回执**（§5）。回执通过前，双方都不要在此文件上叠加其他改动

**若你们更希望由 Windows 侧执行**：完全可以，把步骤 1-4 换到你们那边，
由我们做 §5 的对侧验证即可。谁执行不重要，**双侧验证不可省**。

---

## 5 · 需要 Windows 侧做的验证（本方案的唯一硬依赖）

拉到该提交后，在**干净的** `ui/` 下（建议先 `rm -rf node_modules`，**不要**用 `npm install`）：

| # | 命令 | 期望 |
|---|---|---|
| 1 | `npm ci` | 成功，无 EUSAGE |
| 2 | `git diff --stat ui/package-lock.json` | **输出为空** —— `npm ci` 不得修改 lock，若有 diff 说明仍未同步，立即停止并回报 |
| 3 | `npm run build` | 前端产物正常 |
| 4 | `npm run tauri build --features custom-protocol` | Tauri UI 正常出包（这一步最关键，`@tauri-apps/cli-win32-x64-msvc` 是否真能被解析到，只有这里能证明） |
| 5 | `npm run test`（Vitest） | 与修复前基线一致（当前 54 条） |

**回执格式**：在 `collab/handoffs.md` 写一条，或直接提交信息里注明，标明 npm/node 版本与五步结果。

**若任一步失败**：不要自行 `npm install` 修补（那会把问题反向甩给 macOS 侧）。
请贴出原始报错，我们一起定位。回退方式见 §6。

---

## 6 · 回退

该修复是**单文件、单提交**（只动 `ui/package-lock.json`）。若 Windows 侧验证失败：

    git revert <该提交的 hash>

即可完全回到当前状态。**不要用 `git checkout --` / `reset` / `stash`**
（`collab/troubleshooting.md [GIT-RESET-INCIDENT-001]`：2026-07-24 曾因此丢失 11 个已验收批次）。

回退后一切照旧：现有 `node_modules` 与日常构建流程不受任何影响，
只是 `npm ci` 继续不可用、`docs/MACOS-HANDOFF.md` §6.1 的约定继续悬空。

---

## 7 · 顺带：两个已在 macOS 侧修掉的相关项，供你们知悉

1. **`scripts/setup-macos.sh` 已改用 `npm ci`**（提交 `5e3ed89`）。因当前 lock 失同步，
   该步骤会失败 —— 我们的处理是**响亮报错 + `exit 1` + 指向本文档，绝不 fallback 到 `npm install`**。
   本方案落地后该步骤即可正常工作
2. **`.gitignore` 已排除 `src-tauri/gen/schemas/macOS-schema.json`**（提交 `4b2126b`）。
   该文件由 macOS 上的 `cargo check` 生成，内容与已入库的 `windows-schema.json` / `desktop-schema.json`
   **逐字节相同**（md5 均为 `2d534822642d73cf19e511b7b77c91f1`）。
   同目录你们那四个文件的跟踪状态未变动

---

## 8 · 开放问题（供你们判断，不阻塞本方案）

1. **本次失同步的成因未追查到底**。lock 只有两次提交，中间经历过多次 npm 大版本演进
   （当前 macOS 侧为 npm 11.16.0），**你们 Windows 侧的 npm 版本是多少？**
   若两侧 npm 大版本差距较大，即使这次修好，日后仍可能再次漂移。
   若差距大，建议在 `package.json` 加 `engines` 字段或约定统一的 Node/npm 版本
2. **是否值得为此启用 CI**。一个 `windows-latest` + `macos-latest` 各跑一次 `npm ci` 的 workflow
   即可让此类问题在提交时暴露。你们在 `MACOS-HANDOFF.md` §2.3 也提到过 CI 是唯一可靠防线，
   而 Gavin 2026-07-29 决定暂不启用（DEC-033 附则二）——此处仅作记录，决定权在 Gavin
