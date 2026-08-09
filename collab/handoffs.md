# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-08-09 20:5x — tester-1 — TEST-EXEC-030 ✅ 全量回归零红条（⚠️ 本条 handoffs 与 progress 为**主控代记**）

> ⚠️ **[DOC-STATE-DRIFT-001] 又一次复现**：tester-1 完成后只更新了 `CHANGELOG.md` 与 `logs/20260809.md`
> 两份，**`handoffs.md` 与 `progress.md` 零条目**。本条及 progress 增补三的收尾由主控代记，保留问责链。

- **任务**：030 全批（A/A-2/B/B-2/C/D/E）+ 031 阶段四全量回归，只跑不改。基线 HEAD `d2ee6b3`
- **A0 起点自证**：HEAD 对 ✅ ｜ `cargo fmt --check` clean ｜ `cargo check --all-targets` 0 error（51.79s）
  ｜ 曾报 `src/` 14 文件 M → 主控独立取证判为 `[CRLF-CROSSPLAT-001]` 行尾噪声 + git stat 缓存瞬时态，**放行未处理**
- **A1–A7 全绿零 FAIL**：`itn::` **225** ｜ `punctuation::` **43** ｜ `transcription::` **105 passed + 4 ignored**
  ｜ `llm::` **140** ｜ 主 crate 全量 **958 + 8 ignored** ｜ src-tauri **53** ｜ `--list` 自洽 **966 == 966**
- **四族零回归**：① 017 重量链 ② 026 货币链 ③ 027 大额 DEC-042 ④ 031 万一守卫（13 固定词保汉字 + 12 放行组 + 5 跨模块）**全绿**
- **`itn::` 用例数裁定**：实跑双口径 **225**。handoffs 旧记录 212 失效；coder-1 报的 219→221 为**中间态**
  （TEST-SYNC-030-B 之前）。**主控用源码 `#[test]` 计数独立复算 = 225，与实跑一致，裁定成立**
- **主控独立验收**（不采信汇总表格，依据 `[TESTER-FABRICATED-REPORT-001]`）：六个数字全部用源码计数复算吻合 ——
  `itn.rs`=225 ｜ `punctuation/mod.rs`=43 ｜ `llm/mod.rs`=140 ｜ transcription 三文件 54+28+27=**109**（=105+4ign）
  ｜ src-tauri 22 + `#[path]` 引入的 `wordbook/mod.rs` 5 + `wordbook/db.rs` 26 = **53**
  ｜ 总数 900(feiyin-ime) + 30(crash-reporter) + 36(tests/*.rs 逐文件 3/12/10/2/9 全对) = **966**
- **零生产代码改动**：`git status -- src/` 空，测试断言亦未改（无 ①② 类红条需处理）
- **遗留（只报不改）**：`examples/probe_031.rs`（08-08 031 探测残留，未清理、未入库）—— 处置待 Gavin 拍板
- **下游**：🔜 **BUILD-015 出包**，主控下达指令后执行
- **详情**：`outbox/tester-1/result.md`（9071B）+ `CHANGELOG.md` + `logs/20260809.md`

## 2026-08-09 00:30 — orchestrator — 🛑 会话中断交接（tester-1 额度用尽）

- **断点**：TEST-EXEC-030（阶段四）已派发但 tester-1 零产出（`outbox/tester-1/result.md` 0 字节）即中断
- **工作区**：✅ 干净，无悬空改动。HEAD `d2ee6b3`，本批四 commit 全部落地，**本地 ahead 未 push**
- **下次第一件事**：清空 `outbox/tester-1/result.md` → `dispatch tester-1`。任务书 `collab/inbox/tester-1/task.md` **已写好可原样复用**（含 A0–A7、红条三分类纪律、四族零回归专项）
  - 🔴 **2026-08-09 19:38 更正**：该任务书在新 session 启动（19:33）时**已被清空为 0 字节，无法复用**，主控已重写（7030B）并派发。教训：跨 session 不可假定 `inbox/*/task.md` 存活，交接时应把任务书正文落到 `collab/drafts/` 或 todo 内，而非只留 inbox 路径引用
- **基线数字**：`itn::` 221 ｜ `llm::` 134+9 ｜ `punctuation::` 38+10 ｜ `transcription::` 105 ｜ src-tauri 53 ｜ Vitest/pytest SKIP
- **⚠️ 本批至中断为止一次 `cargo test` 都没跑过** —— 只有 `cargo check --all-targets`（主控实跑 0 error）+ `cargo fmt --check` 通过 + 各 Worker 局部自验。全量回归有红是正常的（`strip_punctuation` 由「删标点」改「换空格」属行为变更）
- **待 Gavin 拍板**：阶段三是否开白名单例外（只允许 `cargo fmt` + `cargo check`）—— 同一根因本批发作三次，第三次致主干编译失败
- **详情**：`collab/todo.md` 顶部交接节 + `logs/20260808.md` 末节 + `collab/progress.md` v0.7.3 增补三

## 2026-08-08 — coder-1 — PUNCT-GOVERNANCE-030-D ✅ 翻译路径 system_content 抽纯函数（零行为变更）

- **来源**：tester-1 走查发现翻译路径 zero 断言（PROMPT-ARCH-020 复发形态）。基线 `5fc390d`
- **改动 `src/llm/mod.rs`**：`optimize_and_translate` 内联的 `system_content` 装配抽为模块级**私有**自由函数 `fn build_translate_system_content(target: TranslationLanguage, punctuation_enabled: bool, wordbook_block: Option<String>, extra_instruction: Option<&str>) -> String`（impl 块之后）。函数内做 `target→target_desc` match、`step1_correct` 双形态、`punct_instruction` 双形态、wordbook/extra 的 `\n\n` 前缀拼接、六参 format!；**无 await/无请求/无 I/O/无 self**（`build_wordbook_prompt_block` 的 SQLite I/O 留调用侧传入）
- **签名定案**：弃 `text`（实测不参与构造，仅进 `<speech>` 用户消息）；`target` 传 Copy 枚举；`wordbook_block` 传 `Option<String>` 直传（省调用侧临时 let）；`extra_instruction` 函数内 trim+非空过滤+前缀。签名 tmux 发主控确认，批准用私有 fn（与 `f3_rules_text` 一致）
- **逐字节验证**：临时断言按旧内联实现逐字重建参考函数，穷举 2目标×2标点×2wordbook×3extra=24 组合 `old==new` 全 PASS，验证后删除
- **缺口 3**：`step1_correct` 双分支 / target_desc 映射 / punct 双形态全部可在返回值断言（TEST-SYNC-030-B 归 tester-1 补写，本任务不新增测试）
- **验收**：cargo check --all-targets 0err（13s）/ `cargo test llm::` 134/0（temp 测试删后重跑）/ rustfmt 未动 / 仅改 mod.rs / 未 build --release / 未出包 / 未 commit
- **边界**：`translate()`（:616）自身另段内联 system prompt 不在本任务范围；未碰 main.rs / punctuation/mod.rs / coder-2 在途文件
- **详情**：outbox/coder-1/result.md + logs/20260808.md + CHANGELOG.md

## 2026-08-08 — coder-1 — LLM-CONN-POOL-028 ✅ 连接池僵尸连接修复（reqwest pool_idle_timeout + is_request 重试 + 错误链路日志）

- **来源**：Gavin 端测发现 LLM 优化间歇性 0ms 失败（未上网络即挂）。基线 merge `7e76465`
- **根因**：reqwest builder 只设 connect_timeout，吃默认 pool_idle_timeout=90s；DeepSeek 服务端 keep-alive ~60s → 60-90s 窗口内死连接复用即 0ms 失败（实测失败点 62.5/67.9/72.3/72.8s 吻合）
- **改动 `src/llm/mod.rs`**：① 新增 `POOL_IDLE_TIMEOUT=30s`（必须 < keep-alive 余量，CONNECT_TIMEOUT 旁注释写明）+ builder `.pool_idle_timeout` ② 重试判据 `e.is_connect()||e.is_timeout()` → `+ e.is_request()`（`Kind::Request` 桶覆盖连接复用失败；body→Body/decode→Decode/builder→Builder/status→Status 独立桶不误吞，已核 reqwest-0.12.28 error.rs）③ 新增 `fmt_error_chain`（逐层 source() 展开），三处日志改用
- **镜像 `src-tauri/src/llm.rs`**：`POOL_IDLE_TIMEOUT` + `.pool_idle_timeout` + `is_retryable_error` 加 `is_request()`
- **验证**：cargo fmt clean / cargo check 0err（13.5s，pre-existing warnings）/ src-tauri check 0err（33.56s）/ `llm::` 131/0 / src-tauri 53/0
- **边界**：未改 CONNECT_TIMEOUT/ATTEMPT_TIMEOUTS/MAX_ATTEMPTS 现有值；未碰 itn.rs/prompt/无关逻辑；未 build --release；未出包；UTF-8 用 edit 工具
- **下游**：Gavin 端测验证间歇失败消失；docs/MACOS-HANDOFF §2.9.5 已记跨端说明（macOS 复用同文件自动同步）
- **详情**：outbox/coder-1/result.md + logs/20260808.md + CHANGELOG.md

