# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-08-30 — coder-1 — VERSION-059 ✅ 版本号升级 v0.8.1 → v0.9.0（零功能改动，待主控验收）

- **来源**：Gavin 2026-08-30 明确指示「升级版本到 v0.9.0」，属「版本号禁止擅改」规则例外放行
- **改动**：3 处手改 + 2 处 cargo 自动写回
  - `Cargo.toml:3` `version = "0.8.1"` → `"0.9.0"`
  - `src-tauri/Cargo.toml:3` `version = "0.8.1"` → `"0.9.0"`
  - `src-tauri/tauri.conf.json:9` `"version": "0.8.1",` → `"0.9.0",`
  - `Cargo.lock:5893` + `src-tauri/Cargo.lock:934` 由 `cargo check` 自动同步
- **方案评估**：同意主控方案。全仓 grep `0.8.1`（排除 target/node_modules/Publish/Backup/.venv）仅命中主控列的 3 处，无第 4 处产品版本号漏列。macOS `scripts/Info.plist:20` 的 `0.7.3` 由 `build-macos.sh:92-147` PlistBuddy 动态覆盖，非手改项（已记 docs/MACOS-HANDOFF.md §VERSION-059）。`ui/package.json:4` 的 `0.1.0` 按红线不动。
- **验证**：cargo fmt clean / cargo check --all-targets 0 error（99 既有 warnings）/ cargo check src-tauri --all-targets 0 error（13/11 既有 warnings）/ git diff -w 五文件仅版本号行变更 / 旧版本号 0.8.1 grep 0 命中 / 新版本号 0.9.0 恰好 5 处
- **红线合规**：只改版本号未碰功能代码 / 未跑 test/build（阶段一）/ 未 commit/push / 禁用 git 破坏性命令全程未用 / 未动 ui/package.json / UTF-8 编写（bash heredoc + Edit 工具）
- **跨平台**：版本号平台中立，macOS 侧无需同步改动，详见 docs/MACOS-HANDOFF.md §VERSION-059
- **详情**：outbox/coder-1/result.md + logs/20260830.md + CHANGELOG.md + collab/progress.md（新建 v0.9.0 段落）

## 2026-08-30 — coder-1 — ASR-067/070 取证 ✅ v0.9.0 两条 P0 根因取证（零生产改动，待主控验收）

- **来源**：主控派单（基线 HEAD `9c9ff73`，v0.9.0）。两条 P0：ASR-067 停顿 1 秒后麦克风不接受输入 / ASR-070 松键后尾部文字丢失
- **ASR-067 结论**：主控 800ms 假设大概率不成立。官方文档确认 sentence_silence 断句不断连（新句 sentence_begin 自动开始），客户端 on_result 状态机（qwen_inference.rs:472-492）正确处理新句，主循环 sentence_end=true 后无 break/return。真根因需 Gavin -debug 实测，三方向：A 服务端静默期 / B channel 积压 / C heartbeat 未过滤。🔴 需确认 Gavin 端测用哪个模型（Publish/config.toml 是 performance 本地模型，若本地则 800ms 假设完全不适用）
- **ASR-070 结论**：根因锁定 `final_text()`（qwen_inference.rs:520-522）只返回 confirmed_sentences 不含 current_sentence。松键时若最后一段未收到 sentence_end=true 则尾部丢。候选 c（揭示进度截断）排除——最终提交取 final_text 不取 displayed_chars（reveal 只管动画）。修复点落 qwen_inference.rs，归 coder-1 与 coder-2 零冲突。建议修法：final_text 改返回 display_text
- **文件域**：只读 qwen_inference.rs / main.rs / audio/mod.rs + 官方文档 + research；**未写入 main.rs**（coder-2 独占）
- **红线合规**：零生产改动 / 未写入 main.rs / 未跑 test/build / 未 commit/push / 禁用 git 破坏性命令 / Publish 运行时数据只读 / 版本号 0.9.0 未动 / UTF-8
- **需主控确认**：① Gavin 端测用哪个 ASR 模型 ② ASR-070 修法 A 是否可立项 ③ ASR-067 是否等 -debug 日志再定方案
- **详情**：outbox/coder-1/result.md + logs/20260830.md + CHANGELOG.md

---

## 2026-08-30 — coder-1 — ASR-070-FIX ✅ 修复松键后尾部文字丢失（qwen_inference.rs，待主控验收）

- **来源**：主控派单（基线 HEAD `9c9ff73`）。ASR-070 取证已验收，根因 `final_text()` 丢弃 current_sentence 确认
- **改动**：仅 `src/transcription/qwen_inference.rs` +27/-4
  - `final_text()`（:520-528）：`confirmed_sentences.join("")` → `display_text()`（confirmed+current），注释改写说明根因
  - 断言 :1793（:1795-1797）：`"第一句"` → `"第一句第二"`，加注释说明契约变更
  - 新增护栏（:1816-1830）：`asr_070_final_text_includes_current_sentence`，消融改回旧实现必红
- **重复计数核查**：on_result（:472-491）end=true 时 push confirmed 同时 clear current（:480/:482 原子），其余分支覆盖非追加 → 无重复风险
- **5 条断言推演**：逐条手工推演，仅 :1793 需改（confirmed+current 场景），其余 4 条 current 为空旧新一致
- **fallback 保留**：:1404-1413 未动，:1411-1413 改后成死代码但 bail 路径 :1409 仍必要
- **验证**：cargo fmt clean / cargo check --all-targets 0 error / cargo check src-tauri 0 error / git diff -w 仅 qwen_inference.rs +27/-4
- **红线合规**：只改 qwen_inference.rs 未写入 main.rs / 未跑 test/build / 未 commit/push / 禁 git 破坏性命令 / 版本号 0.9.0 未动 / 未碰 target/release/config.toml / UTF-8
- **详情**：outbox/coder-1/result.md + logs/20260830.md + CHANGELOG.md + collab/progress.md + docs/MACOS-HANDOFF.md §ASR-070-FIX

## 2026-08-30 — coder-1 — ITN-071/FMT-072 ✅ 三五成群补词+护栏 / 一点半点挂起 / FMT-072取证挂起 / 死代码化简（待主控验收）

- **来源**：主控派单（基线 HEAD `958cadb`）。ITN-071 两条 ITN 错误 + FMT-072 有序列举失败 + 顺带清理 qwen_inference 死代码
- **① 三五成群（已修复）**：不在任何保护集，补进 `itn-rules.toml [protect.idioms]`（:163），三副本同步 `be2ef5c7...`。护栏 2 条，消融删词变红
- **② 一点半点（挂起）**：代码层面 unit_collision_map 一桶 + check_protection 第五步应匹配保护，主控独立复核属实。卡点不在代码而在不知道 Gavin 实际方向，等 Gavin 用例
- **③ FMT-072（取证挂起）**：时间线排查三结论存档，静态分析未找到碰坏有序路径的改动，需 API 验证，主控已向 Gavin 索要用例
- **④ 死代码化简**：qwen_inference.rs:1404-1413 不可达 fallback 化简，保留 bail，+4/-6 行为不变
- **改动**：itn-rules.toml +1、src/itn.rs +17、qwen_inference.rs +4/-6。src/llm/mod.rs 零改动
- **验证**：cargo fmt clean / cargo check --all-targets 0 error / cargo check src-tauri 0 error
- **红线合规**：未写入 main.rs/ui / 未跑 test/build / FMT-072 未自行烧 API / 未 commit/push / 禁 git 破坏性命令 / 版本号 0.9.0 未动 / 未碰 config.toml 等运行时数据 / UTF-8
- **详情**：outbox/coder-1/result.md + logs/20260830.md + CHANGELOG.md + collab/progress.md + docs/MACOS-HANDOFF.md

## 2026-08-30 — coder-1 — ITN-071-B ✅ 时间语境路径绕过成语保护修复（待主控验收）

- **来源**：Gavin 实测用例 `一点半点→1点半点`、`一点点→1点点`
- **产出源全表**：主循环10条路径均受 :1870 check_protection 前置门控，问题在 check_protection 返回 None 而非路径绕过
- **根因**：一点点不在任何保护集→decide_conversion :2275 is_date_suffix("点")=true 误转；一点半点在 unit_collisions(第5步)应保护但防御性挪到 idioms(第1步)
- **修复**：一点半点 unit_collisions→idioms；一点点 新增到 function_words。一点半绝不加保护表
- **回归**：下午一点半/一点十五分/两点半/三点一刻 全部仍正确转换
- **护栏**：4条（Gavin原句2+回归2）各附消融推演
- **改动**：itn-rules.toml +3/-2、src/itn.rs +40。三副本 sha256 311cbb96 一致
- **验证**：fmt clean / check 0 error / src-tauri check 0 error / UTF-8 OK
- **红线合规**：一点半未加保护表 / check_protection 语义未改 / 未写入 main.rs/ui / 未跑 test/build / 未 commit/push / 禁 git 破坏性命令 / 版本号 0.9.0 未动 / config.toml 等未碰

## 2026-08-30 夜 — tester-1 — REPRO-073 ✅ 四项端测现象实机取证完成（零生产/零用例/零出包，待主控验收）

- **基线**：HEAD `9c9ff73`，取证 exe=target/release/feiyin-ime.exe（08-18 23:48，Gavin 端测同款）以 -debug 运行
- **REP-061 闪左上角**：未复现。1044 帧全程 2ms rect 采样，overlay 从可枚举起就是终态 (1160,1292,240,36)，无中间帧。建议 Gavin 多屏/DPI 变更下复测（coder-2 054-B-FIX 或已根治此现象）
- **REP-068 长文本跳动**：复现。宽度阶梯式 12 档增长 240→1664、x 以水平中心锚反向同步；2 次「长文本→Recording 240宽」回缩；y=1292 全程恒定（无上下跳）。**「交替闪烁」主体按数据是「长文本动态宽 vs Recording 240」拉锯（stop 后 late-StreamingText 4-5s 内 6+ 次），不是 Processing 200×36** —— coder-2 查的方向请按此修正
- **REP-067 停顿失聪 (P0)**：复现（4 轮 3 中）。服务端在 >1.4s 静默后的句子丢尾（词流冻结在 14 words）或整句丢（新 id 全 words=0）。**vad_hit_ms 全 8 run = -1**，客户端无 end-of-speech 信号源。另外静默后新 sentence 的 display 继承前句全文再追加——服务端 sentence 上下文管理问题，不是客户端上行断
- **REP-070 尾部丢字 (P0)**：复现（7 组 5 丢）。**服务端 word 流就停在结尾词之前（「门口集合」等从未到达客户端），LLM/注入层零丢失**。直接回答任务书：是服务端没返回，不是返回没注入。规律：≥4s 音频或含降调收尾时丢，3.3s 短句不丢
- **REP-067/070 疑似同根**：客户端 StopSignal→finish-task 与服务端收尾间无 end-of-speech 握手，final_ms 全部远小于音频时长
- **新疑点 3 项（建议立项）**：①ASR-SUMMARY outcome=failed 但 words_total>0（判定脱节，Gavin A/B 会被误导）②vad_hit_ms=-1 从未命中（服务端 VAD 回执没走通）③stop 后迟发 6+次/4-5s
- **对 coder-1**：ASR-070-FIX 建议复核——本次数据显示丢失在「服务端 word 流未发送」层，final_text 改 display_text() 只能救「已到达但未断句」的场景，救不了「从未到达」的（067 同源问题）
- **产物**：outbox/tester-1/result.md（四节+逐帧数据+7组对照表）；取证数据 240 张截图+3 份 rect CSV+debug.log 副本（3442 行）在 /c/msys64/tmp/opencode/repro073/
- **红线合规**：tests/src/src-tauri/ui 零触碰；Publish md5 前后一致；target/release/config.toml md5 前后一致、API key 未泄漏；feiyin-ime-nor.exe 未动；系统音量 28%/C920 mic 93% 已还原；未出包未 commit；UTF-8

### （补 2026-08-30 深夜）REPRO-073 验收反馈回填 —— 数据局限性声明

主控验收通过并采纳 070 对照结论（已向 Gavin 更正「服务端未返回」定性）。回填一条局限性：主控复核 28 条 [ASR-SUMMARY] 出更早 2 run vad_hit_ms=637/644 正常 → 「vad_hit_ms=-1 从未命中=回执没走通」降级为待验证假设，真因更可能是 TTS 合成音频不触发 VAD；14/26 run 零识别 → 067/070 的比例型统计掺环境因素，**15:09 干净对照与窗口几何数据不受影响**。详见 result.md 附录A 局限性声明。

## 2026-09-03 — coder-2 — HOTKEY-060 收尾补账 ✅（代码已在 9506eac/e7a6a29，本次零代码，待主控验收）

- **来源**：主控派单 Part A。08-30 主控 `git add -A` 误扫入 ITN 提交，未走验收流程，本次补齐账目
- **代码盘点（git 取证）**：`ui/src/pages/HotkeySettings.tsx` 9506eac +93/-65（finalizedRef 两侧拆分
  + reset 函数拆分 + 冲突检测收敛为共用 helper `applyHotkeyIfNoDupConflict` :154-195 +
  删除死常量 TRANSLATION_SINGLE_KEYS）；e7a6a29 -3 行（checkAndApplyVoiceHotkey 手写 finalized
  检查收敛进 helper）。调用点：语音 :204/:219/:497，翻译 :350
- **主控补审结论（转录）**：两侧独立 ✅ / 死代码删除等价非回归 ✅ / tsc 0 error ✅
- **红线合规**：纯文档零代码 / 未跑 test/build / 未 commit / 版本号 v0.9.0 未动 / UTF-8
- **详情**：outbox/coder-2/result.md + logs/20260903.md + CHANGELOG.md + progress.md + todo.md

## 2026-09-03 — coder-2 — OVERLAY-075 跨 session 渗漏隔离 ✅（src/main.rs +44/-5，待主控验收+阶段三）

- **根因**：STREAMING_STOPPED 布尔门闩表达不了会话身份。A 拖尾期间开 B → :3679 重置门闩 →
  A 迟到包畅通 → 宽窄拉锯 + A 旧文字渲染进 B 窗口（内容错误）
- **修法**：会话代际身份判断。StreamingText(u64, String, Vec<WordTiming>) +
  STREAMING_GENERATION AtomicU64：Start bump（:4180）→ 闭包捕获（:4286）→ 每包加戳（:4313）→
  消费闸门（:3694-3709，先于词库镜像）。STREAMING_STOPPED 未动（两闸正交）
- **产出源清单**：Windows 唯一产出 :4279（改前）；macOS 三处 1 字段签名（:4976/:5038/:7597 改前）
  = 051-G 遗留存量破损（macOS 此前必编不过），本批同步 3 字段修复，见 MACOS-HANDOFF §OVERLAY-075
- **实施自查纠错**：闸门第一版放在词库镜像后，会污染 WORDBOOK-053-B 学习数据，已移前
- **消融**：删代际闸门 → A 包经四格真值表「不忽略」→ 拉锯+内容错+镜像污染全复发
- **时序安全**：bump 先于本 session 任何事件发送；A 包先于 B bump 入队=合法尾包照常消费，两种交错均正确
- **验证**：fmt clean / check --all-targets 0 error（99 warnings 基线一致）/ git diff -w 仅 main.rs
- **红线合规**：043 语义未动 / bErase 未动 / 未改尺寸 / 未跑 test/build / 未 commit / 版本号未动 / UTF-8
- **详情**：outbox/coder-2/result.md + logs/20260903.md + CHANGELOG.md + progress.md + todo.md

---

## 2026-09-03 — tester-1 — REPRO-061-COLD ✅ 冷启动定向复测完成：判定 B 态（不算复现，强线索），待主控验收

- **判定**：主控收紧三态判据（测前锁定）下落 **B 态**——可见态 (0,0) 帧 5 次冷启动共 3199 帧 **0 命中**；隐藏态 (0,0) 帧 1870 帧（HWND 可枚举首帧即 (0,0) 1x1，隐藏期 ~600ms 全停 (0,0)）；1323 个可见帧 rect **全部**为终态 (1160,1292,240,36)
- **关键帧证据**：7 帧「rect 已在终态、IsWindowVisible 仍 FALSE」（先挪后显的直接实证），用户可见的永远是 SetWindowPos 之后的帧
- **窗口创建时机（主控要求单列）**：overlay HWND 进程启动后 **+190~+320ms** 已创建（6 次观测一致，含 sanity），热键时窗口已存在 ~570-620ms，**非懒创建**
- **方法**：C# 采样器（EnumWindows 类名 `voice-ime-overlay-window` + PID，src/main.rs:219）+ timeBeginPeriod(1) 校准（间隔 med ~1.5ms，首版 15ms 未达标数据已废弃重跑）；SendInput RightAlt(165) PushToTalk hold 400ms；每冷启动完整 taskkill→重启
- **数据**：outbox/tester-1/repro061-cold/run{1..5}.csv（逐帧 9 列）+ OverlaySampler.cs；md5 前后 19/19 一致；debug.log 仅被测程序自身追加，未用于判定
- **给主控的定向事实**：「(0,0) 创建」成立、「显示前被绘制」不成立——Gavin 看到的机制在 Win32 API 层采样覆盖不到的层（本单不延伸假设）

## 2026-09-03 — coder-1 — ASR-074-FIX ✅ 音频上行背压修复（audio/mod.rs + qwen_inference.rs + MACOS-HANDOFF.md，待主控验收）

**基线**：HEAD `e389289`，v0.9.0。证据源：`collab/evidence/debug-gavin-controlled-repro-20260903-0927.log`（Gavin 09:27 受控复现只读副本）。

**旧日志间接证据**：循环转速 65.2 Hz（903 chunks / 13.86s）< 100 Hz，丢失 351 chunks = 3.51s，finalize 拖尾 4.05s——三数字与主控独立推算逐位吻合。🔴 65.2 Hz 是倒推非证明，瓶颈在 read 还是 send 是最后一个未知数。

**永久埋点**（主控条件三，转永久 debug 级）：
- `[ASR-LOOP]` 每 1000 轮：循环转速 + read/send **分别** min/avg/p95/max（主控条件一）+ chunk_rx 积压
- `[ASR-BACKLOG]` 每 500ms：chunk_rx 积压深度
- `[ASR-DROP]` `log::warn!` 级别：队列满丢帧计数
- `percentile()` 辅助函数 + read/send 各自 samples 数组（cap 4096）

**Step 2-B**（`audio/mod.rs:299-340`）：stop_signal break 前 drain warm.rx，重采样后推 on_chunk。🔴 **时间上限 500ms（非数量上限）**：on_chunk 落到 chunk_tx.send() 是阻塞 send，ASR 线程卡住时 drain 无限等 = 把 ASR 故障传导到录音线程 = 松手后卡死（主控验收时发现的回归风险）。500ms deadline，超时放弃并 `[ASR-DROP]` warn 记录。

**Step 2-C**（`audio/mod.rs`）：三处 `let _ = tx_audio.try_send` → 计数 + `log::warn!`。`WarmInputStream` 新增 `dropped_chunks: Arc<AtomicU64>` 永久字段。完成日志追加 `dropped_chunks=N`。

**Step 2-A**（`qwen_inference.rs:1388-1480`）：每轮排空 chunk_rx 最多 N=16 chunks，合并成一次 send。N=16 理由见 result.md。🔴 若瓶颈在 send：排空治标不治本，下一步拆 send 到独立线程（tungstenite WebSocket 非 Send，需架构改动）。

**三个小缺陷**：① first_audio_byte_ms 主循环首帧 send 处补赋值 ② outcome 赋值挪到 format_summary 之前（7 处退出路径） ③ format_summary 去重。

**验证**：cargo fmt clean / cargo check --all-targets **0 error**（101 warnings，93 duplicates，既有 99 + coder-2 D2D 新增 2，均非本任务）。验证时间：coder-2 恢复 main.rs 编译后一次性跑完。

**红线合规**：版本号 v0.9.0 未动 / 未写入 main.rs / 未 commit / 未碰运行时数据 / UTF-8（bash heredoc + Edit）/ 禁用 git 破坏性命令。macOS 影响：MACOS-HANDOFF §0.2，零编译影响零行为回归。

## 2026-09-03 — coder-2 — ASR-074-GUARD ✅ chunk_tx 超时防卡死（单点，待主控验收）

- **背景**：主控验收 ASR-074 发现跨文件域残留风险——drain 循环 500ms deadline 在循环头判断，
  单次 on_chunk→chunk_tx.send()（bounded 无超时阻塞）卡住即回不去；ASR 停死→队满→录音线程
  不返回→松手卡死
- **改动**：send_timeout(200ms)；Timeout→[ASR-DROP] warn + ASR_CHUNK_DROPS 计数；
  Disconnected 静默。200ms=健康消费 20 倍余量
- **边界**：仅 src/main.rs（on_chunk 闭包 + 新 static）；audio/qwen_inference 未碰；容量 256 未动
- **验证**：fmt clean / check --all-targets 0 error（99 warnings 基线持平）
- **详情**：outbox/coder-2/result.md + logs/20260903.md + CHANGELOG.md

---

## 2026-09-03 — tester-1 — TEST-SYNC-074/075/D2D ✅ 阶段三 14 用例交付（纯追加负增量0，fmt/check 过，待主控验收）

- **交付**：src/main.rs +211（新模块 overlay_075_d2d_guard_tests 5 用例：075 领号协议/镜像污染/首字段三元 + GUARD 三分支/计数器 + D2D 回落触发器）、src/audio/mod.rs +154（Step 2-B drain 四用例）、src/transcription/qwen_inference.rs +233（批量边界 ×2 + 缺陷1/2/3 ×3）
- **消融**：每条用例附「改回旧实现会不会红」推演（result.md 逐条表）
- **覆盖缺口 5 项如实声明**（真实 WS 帧序/完整 match arm/真实 DC 成功路径/WASAPI 回调内计数/真实慢消费 abandoned 精确值），阶段五端测建议已附
- **自查**：fmt --check clean；check --all-targets 0 error；git diff -w 纯增量；既有 228 用例零触碰
- **注意**：阶段三未跑用例（红线）；阶段四执行时新用例随 `cargo test` 生效，其中 main.rs 新模块带 `#[cfg(all(test, target_os="windows"))]`（Windows-only statics 依赖）

## 2026-09-03 夜 — tester-1 — TEST-EXEC-076 阶段四全量回归 ✅（源码层纯执行，5 FAIL 原样上报，待主控验收+裁定）

- **范围**：Step 1 两 crate cargo test + Step 2 vitest + 必查A/B/C；pytest 系列按任务书 SKIP（08-18 旧包）
- **净结果**：root 1050P/4F/9I ｜ src-tauri 76P 全绿 ｜ vitest 83P/1F（HotkeySettings 23/24）
- **5 FAIL 全部完整取证**：①②itn_071b×2（一点半→「下午1:30」非「1点半」，疑生产 071-B 不彻底）③asr_074 abandoned 5/5 稳定红（测试自建循环语义≠生产 Empty=>break，疑似测试设计缺陷）④stale_generation（断言 vs 生产 :4006 镜像先写冲突，053-B/075 语义裁定）⑤S12（AltGr 弹窗键名 Left Ctrl≠Right Alt，拦截行为正确）
- **必查A**：guard 门控模块 --list 实证 6 条非 0 ｜ **必查B**：228+15（git 物理实提交 15 个 #[test]，文档「14」是语义组口径，差额=1 已说明）｜ **必查C**：挂钟用例连跑 5 次全红 0.90-0.91s
- **红线合规**：零生产/零用例改动、零出包、零 commit、版本号未动、纯 bash 追加文档
- **详情**：outbox/tester-1/result.md（含裁定请求表：③建议改测试循环对齐生产；④⑤①②需主控裁定改哪侧）

## 2026-09-03 — coder-2 — HOTKEY-078 ✅ AltGr 尾随 keyup 串键修复（HotkeySettings.tsx -3+5，待主控验收）

- **根因（独立复核与主控推演一致）**：`handleVoiceHotkeyKeyUp` :287-289 在 AltRight keyUp
  提前清 `altGrSynthCtrlActiveRef`，尾随合成 ControlLeft keyUp 逃过 :279（现 :284）抑制 →
  第二次 `checkAndApplyVoiceHotkey(0xA2,0)`；async 闸门（await invoke 后才置 finalizedRef）
  挡不住同步连续 keyUp；available=false else 分支 :213-216 不置 finalized 反而 reset →
  `setPendingHotkey({vk:0xA2})` 覆盖 → S12 冲突弹窗错显 Left Ctrl
- **修法**：删 3 行提前清旗，旗唯一清零点回归 `resetVoiceRecordingState()`（:141，会话级生命周期）；
  +5 行注释。非回归五场景独立复核全过（wasActive 捕获序不变/AltRight 先抬修好/ControlLeft 先抬
  行为不变/非 AltGr 无变化/同会话双保险）
- **验证**：npx tsc --noEmit 0 error；git diff -w 精确 -3+5 注释仅此一文件；S12 测试零触碰；
  未跑 cargo build/npm run test（tester-1 职责）；未 commit；v0.9.0 未动
- **Part B 取证（等主控裁定）**：①翻译侧推演成立——handleTranslationHotkeyKeyDown(:335-361)
  无修饰键过滤/无 AltGr 旗/无 keyUp 处理器（:466-467），AltGr 首事件 keyDown ControlLeft
  :344 查表 0xA2 → :350 全同步直调 finalize → 翻译热键被录成 Left Ctrl，连 async 重入窗口
  都不存在，无用例覆盖故未红；②修法=镜像语音侧键序生命周期（结构性改动需配套用例），
  建议另开单，不碰 HOTKEY-060 helper 契约；③同步闸门同意不做，补充反论：入口置 finalized
  会破坏 catch 回退路径（:218-228 invoke 异常时热键永远写不进去）
- **附加发现备案**：Alt 先抬 Ctrl 后抬后单按 Left Ctrl 被 :284 误抑制至 Escape 重进；
  修前错录 Left Ctrl、修后静默忽略，属模糊歧义键序更安全取舍
- **红线合规**：仅 HotkeySettings.tsx / 测试零触碰 / 未 commit / 版本号未动 / UTF-8（Edit 工具）/
  零临时文件 / MACOS-HANDOFF §HOTKEY-078 已记
- **详情**：outbox/coder-2/result.md + logs/20260903.md + CHANGELOG.md
