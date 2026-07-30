# handoffs · voice-ime

## 2026-07-30 — coder-1 — AUDIT-MACOS-BRANCH-001 ✅ 完成（纯审计，零代码改动）

- **范围**：`src/` 与 `src-tauri/src/` 内所有 `#[cfg(target_os = "macos")]` / `#[cfg(not(target_os = "windows"))]` / `#[cfg(unix)]` / `#[cfg(target_family = "unix")]` 分支的静态 API/签名审计
- **产出**：
  - `collab/research/macos-branch-audit-001.md`（15 处分支 / 1 P0 / 8 P1 / 4 P2 / 2 P3）
  - `collab/outbox/coder-2/result.md`（tmux 完成通知用 outbox 摘要）
- **关键发现**：`src/crash/reporter.rs:369` 在 `egui 0.29.1` 下调用不存在的 `egui::FontData::from_bytes()`，macOS 编译必然失败；修复方向为 `egui::FontData::from_owned(font_data)`
- **新发现**：`src/main.rs:3419-3475` 的 `mod macos_stubs` 为主控此前未知的 macOS 空壳实现，overlay/worker/pipeline 在 macOS 路径完全空转
- **已确认正确**：`core-graphics 0.25.0` + `core-foundation 0.10.1` + `enigo 0.2.1` 主要 API 签名与代码一致；`src-tauri/src/main.rs:193` `set_shadow(false)` 在 tauri 2.10.3 存在
- **未覆盖**：`src/llm/**`（任务边界外）、`patches/`/`vendor/` 内的 cfg、测试文件
- **红线**：未修改任何 Rust/TS/TOML/CI 源文件；未 `cargo build --release` / 出包 / 启动 exe；未使用 git 破坏性命令

## 2026-07-29 — coder-2 — MACOS-COMPAT-001-TAURI-CI ✅ 完成（范围变更后）

- **范围**：Tauri 侧 cfg 隔离 + Windows 新开发者一键获取 sherpa-onnx 脚本
- **改动**：
  - `src-tauri/Cargo.toml`：`windows` 依赖移到 `[target.'cfg(target_os = "windows")'.dependencies]`
  - `src-tauri/src/main.rs`：`check_hotkey_available` 加 cfg 隔离 + 非 Windows 占位实现
  - `src-tauri/src/overlay.rs`：`.transparent(true)` cfg 拆链，Windows 路径逐字节等价
  - `scripts/fetch-sherpa-onnx.ps1`（新增）：给 Windows 新开发者用的一键下载 sherpa-onnx 预编译包脚本
- **按 Gavin 指令取消的改动**：
  - 不解除 `.gitignore` 对 `.github/` 的排除
  - 不新建/修改 `.github/workflows/` 任何 workflow
- **验证**：`cargo check --manifest-path src-tauri/Cargo.toml` ✅ 0 errors；`cd ui && npm run build` ✅；`git diff` 仅保留 cfg 改动 + 新脚本
- **红线**：未碰 `src/**`、`.cargo/config.toml`、`tauri.conf.json`、版本号；未修改 `.gitignore`；未 `cargo build --release`/出包/启动 exe；未写 `scripts/setup-macos.sh`/`env-macos.sh`；未使用 git 破坏性命令
- **下游需知**：`scripts/fetch-sherpa-onnx.ps1` 用于解决 `.gitignore:22` 排除导致全新 checkout 无法构建的问题，macOS 团队即将 checkout 接手

## 2026-07-29 — coder-2 — RESEARCH-MACOS-DUALPLATFORM-001 ✅ 完成（纯研究，零代码改动）

- **范围**：双平台单仓库重构可行性评估；仅产出研究报告，未修改任何 Rust/TS/TOML/CI 源文件
- **产出**：`collab/research/macos-dualplatform-refactor-001.md`（逐条回答 Q1-Q5，带置信度标注） + `collab/outbox/coder-2/result.md`
- **核心结论**：GO（有条件通过）；A 阶段可让 macOS 侧 checkout 后 `cargo check` 通过而不影响 Windows；完整 release/运行需 B/C 阶段 + Apple Developer 账号
- **关键复核**：P0 5 项阻断/签名漂移 6 行/代码结构量化均属实；`#[cfg]` 切掉代码不做类型检查、trait 不能防漂移、双平台 CI 是唯一可靠防线
- **建议最小批次**：约 140 行（仓库内 ~40 行代码 cfg 隔离 + ~100 行脚本/CI），需 macOS 侧先提交 setup/env 脚本并验证 sherpa-onnx dylib / ctranslate2 features
- **Windows 零回归验证命令**：`cargo check` / `cargo check --manifest-path src-tauri/Cargo.toml` / `cd ui && npm run build` / `cargo test` / 人工启动 exe 30s
- **红线**：未修改 `.gitignore` / `.cargo/config.toml` / `Cargo.toml` / `tauri.conf.json` / 任何源文件；未 `cargo build --release` / 出包 / 启动 exe；未使用任何 git 破坏性命令

## 2026-07-28 — coder-1 — IMPL-SCENE-COVERAGE-001（场景词表扩展实施，纯 toml 免构建）✅ 代码层完成（待主控验收）

- **来源**：Gavin 指令联网调研场景感知词表扩展（RESEARCH-SCENE-COVERAGE-001 研究报告）→ 主控三处修正 + Gavin 三项决策 → 本实施任务
- **范围**：只改 `scene-rules.toml`（根目录），未碰任何 Rust 源文件 / target/release/ / Publish/ / 版本号
- **改动**：
  1. **A 项**：6 条历史推测项（DouyinIM/FeishuDocs/NewMailEngine/DingTalkLite/WXWorkApp/Obsidian-helper）注释改为 ⚠️ 存疑标注；NewMailEngine「火狐邮件」事实错误改正为 Thunderbird 说明；Obsidian-helper 附本机实测结论。条目保留不删（Gavin 决策：零成本，删除买不到收益反增静默回归风险）
  2. **B 项**：新增 21 条 exe（✅实测 6：Xagent/wezterm/git-bash/MarkText/Koodo Reader/CalendarApp.Gui.Win10；⚠️未证实 15：Doubao/Kimi/Tongyi/Wenxin/ChatGLM/GLM/NanoSearch/Perplexity/Zoom/wemeetapp/Linear/Mailbird/The Bat!/Nu/Figma）。Figma 归 browser（Gavin 决策 3，非我原建议的 ide_terminal）；ChatGLM+GLM 新旧名并存；OneNote.exe 未加（主控修正 2，大小写归一化重复）
  3. **C 项**：doc 块 title_keywords 加 4 条（Jira/TAPD/禅道/Teambition）。主控修正 1：我原建议加进 browser 块是 no-op（src/scene/mod.rs:162-164 细分循环跳过 browser 自身关键词），改放进 doc 块才能生效。browser 块 title_keywords 未改动
  4. **D 项**：Skype 注释补「微软已引导迁移至 Teams」
- **主控三处修正全部采纳**：① title_keywords no-op（加进 doc 块非 browser 块）② OneNote.exe 重复剔除（22→21）③ 15 条 ✅官方降级 ⚠️未证实（证据只支撑产品有 Windows 版，不支撑进程名）
- **自验**：`cargo test --bin feiyin-ime scene::` 48 passed / 0 failed（include_str! BUILTIN_RULES 解析通过，TOML 语法正确，含 The Bat!.exe 含 ! 与 Koodo Reader.exe 含空格）
- **条数差异上报**：原文件实际 141 条 exe（git show HEAD 统计），任务文件 §二基线说 144，差 3 条。我新增 21 条 → 162 条（非 165）。新增数与 §三.B 清单完全一致，请主控核实 144 基线来源
- **边界**：未改 Rust / target/release/ / Publish/ / 版本号；未 cargo build --release；未用 git 破坏性命令；用 edit 工具改 UTF-8

## 2026-07-28 — tester-1 — TEST-EXEC-SCENE-COVERAGE-001 ✅ 全量回归 + 三副本同步 + 运行时验证完成

- **来源**：主控派发 TEST-EXEC 任务（阶段四），对 IMPL-SCENE-COVERAGE-001 + TEST-SYNC-SCENE-COVERAGE-001 执行全量回归、三副本同步、运行时验证（不出包）
- **Step 1**：`cargo test` ✅ 686 passed / 0 failed / 8 ignored（基线 672 + 14 新 scene 单测 = 686，数字链自洽）
- **Step 1b**：`cargo test --manifest-path src-tauri/Cargo.toml` ✅ 53/0/0
- **Step 2/3/4**：SKIP（零前端/零生产 Rust 改动）
- **Step 5**：`scene-rules.toml` 三副本同步（根 → target/release/ → Publish/），sha256 三值一致 `7b01b33ca90b6d782c2cf06430b941c96e79169f2aa2ee2b99e7ed468329cb87`
- **Step 6**：终止旧实例 PID 18548 → 以 `-debug` 启动新实例 PID 23056 Responding=True；debug.log 确认零 `Scene parse error` 与零 `Scene builtin rules parse error`；新实例存活但无录音触发（待 Gavin 自然使用产生 `Scene context:` 行）
- **边界**：未改版本号、未出包（显式禁止）、未修改任何源文件、未用 git 破坏性命令、UTF-8 红线遵守（二进制 cp 拷贝 toml，非文本编辑）

## 2026-07-28 — tester-1 — TEST-SYNC-SCENE-COVERAGE-001 ✅ 测试编写完成

- **来源**：主控派发 TEST-SYNC 任务（阶段三），配合 coder-1 的 scene-rules.toml 纯词表扩充（144→165 exe + doc title_keywords Jira/TAPD/禅道/Teambition）
- **范围**：仅改 `src/scene/mod.rs` `#[cfg(test)]` 块，生产代码零改动
- **P0×5 + P1×1 = 6 条新增单测**：
  - P0-1：`builtin_rules_parse_ok`——直接用 `toml::from_str::<Rules>(BUILTIN_RULES)` 断言解析成功（非 `compile_rules_from_content`），堵住 toml 静默降级全 Unknown 的测试黑洞
  - P0-2：特殊字符条目 `The Bat!.exe`（!）→ Email / `Koodo Reader.exe`（空格）→ Doc
  - P0-3：浏览器细分——chrome + Jira/TAPD/禅道/Teambition → Doc（4 条，断言方向均为 Doc 非 Browser）
  - P0-4：反向护栏——browser 自身 title_keywords 不参与细分（自定义 fixture，因真实 browser 块 title_keywords 与 email/doc 100% 重叠）
  - P0-5：Figma→Browser / ChatGLM,GLM→Chat / Zoom,wemeetapp→Chat（5 条归类决策断言）
  - P1：`OneNote.exe` / `ONENOTE.EXE` 大小写不敏感均 → Doc（常量相等断言）
- **自验**：`cargo check --tests` 0 errors；`cargo fmt -- src/scene/mod.rs` 0 diff
- **红线**：仅改测试文件；禁止 cargo test/build/pytest/启动 exe——全部遵守；未使用 git 破坏性命令；UTF-8 红线遵守（edit 工具）

## 2026-07-28 — tester-1 — BUILD-RELEASE-SCENE-COVERAGE-001（出包，仅主程序）✅ 已交付

- **来源**：Gavin 指令「出包」。功能与测试代码之前已完成，本次仅 `cargo build --release` 根项目
- **范围**：仅构建主程序（feiyin-ime.exe + crash-reporter.exe），**未碰 Tauri UI / npm build / 版本号 / 源文件**
- **结果摘要**：
  - `cargo build --release` 2m25s 0 errors
  - 产物 11,603,456 B（旧 11,599,360 B），+4096 B 符合 21 条 TOML 增量
  - 新条目 `NanoSearch.exe` / `Koodo Reader.exe` / `CalendarApp.Gui.Win10.exe` / `wezterm.exe` / `git-bash.exe` 均在 exe 中命中
  - sha256 两副本一致：`e35679bd95484c71e74a00b7a94829466416e8b583725571c62ef170d4d17380`
  - ProductVersion 0.7.2.0 未变
  - PID 18928 运行中，0 panic
- **已知缺口**：`voice-ime-ui.exe` 沿用 07-24 旧版（本次未重建）

> 2026-07-27 及更早条目已归档至 handoffs-archive.md（2026-07-28 归档，见 worker-guide.md §九文档防膨胀）。

> 只保留当天条目，>200 行时归档到 handoffs-archive.md。

## 2026-07-30 — coder-1 — MACOS-PR1-SCRIPTS-001（macOS 构建脚本入库，第一个 PR）✅ 代码层完成（待主控验收）

- **来源**：Windows 侧明确交接请求（docs/MACOS-HANDOFF.md §4.2）—— setup-macos.sh / env-macos.sh 本机存在但从未提交，全新 clone 无法在 macOS 起步构建
- **范围**：只碰 `scripts/` 下三文件，未碰 src/src-tauri/ui/docs/.cargo/.gitignore/版本号/ui/package-lock.json
- **改动**：
  1. **C 项（env-macos.sh:11，本任务最重要）**：`${BASH_SOURCE[0]}` → `${BASH_SOURCE[0]:-$0}`，修 zsh 下路径推导失效（zsh 不设 BASH_SOURCE → 回退 $0；macOS 默认 shell 即 zsh）。bash+zsh 双验证通过，均指向仓库内 `vendor/sherpa-onnx/sherpa-onnx-v1.12.38-osx-arm64-shared-lib/lib` 且列出 dylib。修复前 zsh 实测指向仓库父目录（不存在），sherpa-onnx-sys build.rs 会 panic。采用主控推荐写法（非 zsh 专有 ${(%):-%x} 分支），最小改动面。
  2. **A 项（setup-macos.sh:58-67）**：`npm install` → `npm ci`（消灭破坏 Windows 侧 lock 的动作，DEC-034），并简化掉原 Windows-marker 检测 + node_modules 存在性判断（npm ci 自身清空 node_modules，两段皆 no-op；marker 检测只看 @esbuild/win32-x64 覆盖不全）。无条件 npm ci 从语义保证平台正确。
  3. **B 项（build-macos.sh）**：保留既有工作区改动（+10/−3，REPO_ROOT+cd / source env / 产物名 feiyin-ime），零触碰。
- **阻塞上报与主控裁定**：实跑 npm ci 报 EUSAGE（ui/package-lock.json 与 package.json 既有失同步，@emnapi/core@1.11.3 / @emnapi/runtime@1.11.3 缺失、@emnapi/wasi-threads 1.2.2≠1.2.3，自初始提交 680d78f 即如此）。主控独立复现后裁定 B+C：脚本仍改 npm ci，失败时响亮报错 + exit 1 + 指向 MACOS-FIX-NPMLOCK-001，绝不 fallback npm install；验收标准 3 改为粘 EUSAGE 证据不要求成功；修 lock 另立 MACOS-FIX-NPMLOCK-001 上报 Gavin。A（npm install --no-save）否决（npm 9+ 仍重写 lock）。
- **自验**：验收标准 1-7 全过。git diff --stat ui/package-lock.json 为空（最关键）；git status 改动只在 scripts/ 下（其他 M/?? 项为既有工作区状态非本任务产生）；三脚本 UTF-8 无 BOM；C 项 bash+zsh 双验证均指向实际 lib 目录。
- **遗留（报告未改）**：build-macos.sh 无可执行位（644，既有状态，D 项只报告不修改，建议后续 chmod +x）；npm ci 既有失同步另立 MACOS-FIX-NPMLOCK-001。
- **边界**：未跑 cargo check/build/test；未 git commit/push/add；未用 git 破坏性命令；未运行 npm install；UTF-8 无 BOM 用 edit 工具；未改版本号未出包未碰 Publish/
- **详情**：`collab/outbox/coder-1/result.md`（13109 字节非空）

## 2026-07-30 — tester-1 — MACOS-CARGOCHECK-BASELINE-001 ✅ macOS 编译错误基线取证完成

- **来源**：主控派发基线取证任务（依 DEC-033 §执行前提：装工具链→实跑 cargo check→拿完整错误清单）
- **范围**：零文件改动。macOS 15.7.8 arm64 上执行 `cargo check --all-targets` + `src-tauri` + 前端
- **关键产出**：
  - ✅ 主程序 4 个独特错误（hotkey.rs:124 E0369 / hotkey.rs:257 E0599 / crash/reporter.rs:369 E0599 / main.rs:4349 E0432 test-only）
  - ✅ src-tauri 7 个错误（3 逻辑类：qwen3 依赖链 + rustls，皆因 Cargo.toml 条件依赖与代码 cfg 不匹配）
  - ✅ 前端 0 errors, `package-lock.json` 零改动
  - ✅ **最大发现**：两份既有文档都未预测到的 src-tauri qwen3 依赖链错误（MACOS-BRANCH-AUDIT.md 只查 `#[cfg]` 分支未查 Cargo.toml 条件依赖）
  - CT2 增量编译，未触发全量重编
  - `git status --porcelain` 与开工时完全一致
- **边界**：未修复任何错误、未 cargo build --release、未出包、未 npm install、未 cargo clean、未使用 git 破坏性命令
- **详情**：`collab/outbox/tester-1/result.md`

## 2026-07-30 — coder-2 — MACOS-FIX-COMPILE-001 ✅ 修三处 macOS 编译阻塞

- **来源**：主控基于 MACOS-CARGOCHECK-BASELINE-001 的实测错误清单派发修复任务
- **范围**：仅改 `src/crash/reporter.rs`（macOS 块内 FontData 构造）+ `src/platform/macos/hotkey.rs`（两处 API 误用）
- **改动**：
  1. `reporter.rs:369` — `FontData::from_bytes` 不存在 → `from_owned(font_data)`；删除 `.ok().unwrap_or_default()` 链式调用（`from_owned` 直接返 `FontData` 非 `Result`）
  2. `hotkey.rs:124` — `CGEventType` 无 `PartialEq`，`==` 编译不过 → `matches!(event_type, CGEventType::KeyDown)`
  3. `hotkey.rs:257` — `create_runloop_source` 返回 `Result` 但用 `Option` 的 `.ok_or_else()` → `.map_err(|_| anyhow!(...))?`
- **自验**：`cargo check` 0 errors；Windows cfg 块未碰；UTF-8 无 BOM
- **边界**：未改 config/Cargo.toml/版本号；未改 platform/mod.rs 导出；未 cargo build --release/clean；无 git 破坏命令

> 只保留当天条目，>200 行时归档到 handoffs-archive.md。
