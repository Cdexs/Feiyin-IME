# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-08-15 — coder-2 — TRANS-HOTKEY-039 ✅ 翻译热键全链失效修复（P0）

- **来源**：Gavin 端测报翻译热键 100% 不生效，主控全链取证后派发。基线 HEAD `4b3c63f` + coder-1 ASR-038-B 在途改动
- **根因 A**：`ui/src/pages/HotkeySettings.tsx:43` 的 `VK_TO_LABEL` 把 `0xA0` 标成 Right Shift、`0xA1` 标成 Left Shift，与 Windows 事实（VK_LSHIFT=0xA0 / VK_RSHIFT=0xA1）相反。UI 把 Gavin 的物理左 Shift 配置显示为 "Right Shift"，导致他按物理右 Shift 永远不触发
- **根因 A-补充**：`CODE_TO_VK` 捕获表本身正确（`ShiftRight:0xA1 / ShiftLeft:0xA0`），Gavin 配置中存的 160 来自他当时按了物理左 Shift，被错标签误导记忆
- **根因 B**：`src/platform/windows/hotkey.rs` 原翻译键轮询只有 500ms 窗口，Gavin 「录音中途才按」必然错过。改为跟随录音生命周期，新增 `TRANSLATE_POLL_STOP` 静态 AtomicBool，在 PTT 松开 / Toggle 二次按下 / hook 卸载 / poll_ptt_release_thread 结束等路径置位；再加 `MAX_RECORD_SECONDS + 5` 秒硬上限兜底防泄漏
- **观测性**：`translate_flag.store(true)` 处补 `log::info!`，输出按下时距录音开始的毫秒数
- **跨文件接线**：`src/main.rs` 6 处终止路径调用 `platform::notify_translate_poll_stop()`（:1978 CancelStop / :1987 ESC / :2028 Done|Cancelled / :2032 FocusLost / :2045 Error / :2063 FormatFailed）。这 6 处与 coder-1 ASR-038-B 改动区（:2231-2413）文本零重叠
- **根因 C（macOS）**：`docs/MACOS-HANDOFF.md` 新增 §TRANS-HOTKEY-039，说明 macOS 侧硬编码 false、从未实现，给出若要对齐需做的 5 项工作；本单不改 macOS 代码
- **改动文件**：`ui/src/pages/HotkeySettings.tsx`、`src/platform/windows/hotkey.rs`、`src/platform/windows/mod.rs`、`src/platform/mod.rs`、`src/main.rs`（6 处单函数调用）、`docs/MACOS-HANDOFF.md`
- **验证**：`cargo fmt` clean / `cargo check --all-targets` 0 error / `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / `npm run build` OK
- **版本号未动**（已是 0.8.0）
- **详情**：`outbox/coder-2/result.md` + `logs/20260815.md` + CHANGELOG.md

# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-08-15 — coder-1 — ASR-038-B-streaming ✅ 真流式核心实施（C-1~C-4 + 040-A）

- **来源**：Gavin 拍板真流式 + 两条硬指令（边说边上屏是验收判据；必须用真 VAD 防「键盘声/风扇声/音乐」无效上传，RMS 被否决）。基线 HEAD `4b3c63f`
- **主控取证纠正**：我说「sherpa VoiceActivityDetector 是批处理式」——不对，批处理是当前 wrapper 写法造成的，`accept_waveform`/`detected()`/`front()`/`pop()` 都是独立方法，滚动判定只需逐块 `accept_waveform` 不调 `flush` 查 `detected()`
- **C-1 VadSegmenter 滚动方法**（`src/transcription/vad.rs` +202 行）：
  - `try_new_for_streaming(model_dir)` 工厂，阈值 0.3（低于分段 0.5）
  - `accept_and_check(&[f32]) -> bool`：喂 `accept_waveform` → 查 `detected()`，不调 flush
  - `reset_for_new_session()` + `vad_window_size()` 导出
  - **不改 `segment()`**（FIX-VAD-STATE-RESET-001 在里面）
  - +6 测试（2 非ORT：阈值常量+模型缺失None；4 ORT-dependent 标 `#[ignore]`）
- **C-2 record_streaming**（`src/audio/mod.rs` +155 行）：
  - 热键按下即采集（WASAPI prewarm stream 已在跑，从 channel 读 chunk 推回调）
  - pre-roll 从 VecDeque drain 后作为首批 chunk 推给回调（建连后补发）
  - RMS 静音检测管停录（speech_detected 行为不变，与 Silero VAD 并存）
  - **不改 record()**：平行方法，record 的 collect_recording 零动
- **C-3 transcribe_streaming_realtime**（`src/transcription/qwen_inference.rs` +447 行）：
  - VAD 入口门控：逐 chunk 喂 Silero，命中即建连。VAD 缺失→立即建连。2s 保底无条件建连
  - pre-roll 补发：VAD 命中前+握手期间缓冲的 chunk 建连后一次性发
  - 边发边收：录音期间 1ms read_timeout 模拟非阻塞读 ws_socket，chunk_rx try_recv 读音频，交替发送+收 result-generated→on_result 回调推 StreamingText
  - finish-task：chunk_rx 断开→恢复 10s timeout→发 finish-task→阻塞收最终结果
  - cancel_signal 各循环检查点检测
  - 040-A 分段 [Latency] 埋点（DNS/TCP/TLS/WS 四段）
- **C-4 worker 接线**（`src/main.rs` +311 行）：
  - QwenAudioOnline 分支：chunk channel → spawn ASR 线程 → record_streaming 推 chunk → drop tx → join ASR → run_pipeline_core(initial_text=Some)
  - 非流式分支：record()+run_pipeline_core(None) 零行为变更
  - run_pipeline_core 新增 `initial_text: Option<String>` 参数
- **FIRSTCHAR 专项自证**：非流式前处理链逐字节等价
  - SPEECH_ENERGY_THRESHOLD=0.008 / select_preprocessing_params / find_speech_onset_with_backtrack / padded 构造全不变
  - is_accuracy 内联为 if 表达式（log 输出值相同）
  - Err 路径：原版 match 内 send_event → 搬迁后 .map_err + 外层 match send_event（行为等价）
  - 6 条 FIRSTCHAR 单测全绿
- **040-A 埋点**：`transcribe_streaming` + `transcribe_streaming_realtime` 都加 DNS/TCP/TLS/WS 分段 [Latency]
- **验收**：cargo fmt clean / cargo check --all-targets 0 error / cargo test 949+32 全绿 0 failed（10 ignored）/ 文件域 6 文件零越界
- **版本号未动**（已是 0.8.0）
- **详情**：outbox/coder-1/result.md + logs/20260815.md + CHANGELOG.md

## 2026-08-15 — coder-1 — ASR-038-B-partial ✅ 038-B 收尾（部分实施，架构项挂起）

- **来源**：ASR-038-B 任务书（主控派发）。基线 HEAD `4b3c63f`（含 WIP `003ba65`：A 批 + B 半成品）
- **主控方案裁决**：coder-1 提伪流式方案，主控反驳（037 编辑态交互在伪流式下物理不成立——录音期间无文本可点），裁决为「立即开工端点缺陷+cancel_signal+preprocessing 结论；伪流式vs真流式挂起等 Gavin 拍板」
- **Step 1：修 E0425**：`load_wordbook_vocabulary` 从 `mod tests` 内移到模块顶层（`src/transcription/mod.rs`），`cargo check --all-targets` 0 error
- **B-1：修三条端点缺陷**（主控独立验证 + 追加两条同族）：
  - ① `mod.rs:302` 用 `self.qwen3_url`（Realtime 端点）调 `transcribe_streaming`（Inference 协议）→ 连错端点必失败
  - ② `qwen_asr_url`/`qwen_asr_model` config 字段全库零消费（grep 证实）
  - ③ `default_qwen_asr_url` 主机名 `dashscope.aliyuncs.com` 是 Realtime API 的，035 文档 A6 明确须为 `{WorkspaceId}.cn-beijing.maas.aliyuncs.com`
  - **修法**：Transcriber 新增 `qwen_asr_url`/`qwen_asr_model` 独立字段 + `new` 签名加两参数 + 所有调用点更新 + `default_qwen_asr_url` 改为 `wss://llm-kudx4dj2bfqn4gr2.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference`
  - **配套**：热重载触发条件加 `qwen_asr_changed` + `active_qwen_asr_*` 跟踪 + reload 块加 `reload_qwen_asr_*`
- **B-2：transcribe_streaming 加 cancel_signal**：签名加 `cancel_signal: Option<&AtomicBool>`，上传/接收循环检测取消 → `close(None)` + `bail!("转录已取消")`。EditRequested/ESC 中断支持
- **B-3：select_preprocessing_params 结论化**：`silence_head`/`onset_backtrack` 伪流式下完全适用，真流式下不适用。注释扩充写明，无逻辑变更
- **新增 +10 测试**：4 config 端点回归防护 + 6 transcription（QwenAudioOnline 配置验证 + asr_model 解析扩展）
- **验收**：cargo fmt clean / cargo check --all-targets 0 error / cargo test 主 crate 947+6ign / config 32 / **0 failed** / src-tauri check 0 error / 文件域 4 文件零越界
- **挂起（等 Gavin 拍板）**：伪流式 vs 真流式 + VAD 入口门控 + record_streaming + 流式管线接线
- **版本号未动**（已是 0.8.0）
- **详情**：outbox/coder-1/result.md + logs/20260815.md + CHANGELOG.md

## 2026-08-14 — coder-1 — ASR-038-A ✅ qwen_inference.rs 流式 ASR 模块实施（第一次动生产代码）

- **来源**：RESEARCH-ASR-038 设计定稿 + 交叉复核通过。基线 HEAD `35a2a74`
- **改动 4 文件**：
  - `src/transcription/qwen_inference.rs` 新建 1076 行（Inference API 流式实现 + 45 单测）
  - `src/transcription/mod.rs` +10/-5（AsrModel 枚举 + from_config + build_recognizer 或模式）
  - `src/config/mod.rs` +19（qwen_asr_url/qwen_asr_model + serde default）
  - `src/main.rs:3210` +3/-3（select_preprocessing_params 或模式，主控授权）
- **验收**：cargo fmt clean / cargo check 0 error / cargo check --all-targets 0 error / 45 单测全绿
- **热词**：user=5 / system=4，wordbook_candidates 表禁止注入，超限过滤防御保留
- **language_hints**：固定 ["zh","en","ja","ko"]，不读 transcription_language（已废弃）
- **遗留待批次 B**：select_preprocessing_params 的 silence_head/onset_backtrack 是批处理概念，流式下是否适用存疑
- **跨端**：MACOS-HANDOFF.md §ASR-038-A 已追加，平台中立模块 macOS 无需同步改动
- **版本号未动**（已是 0.8.0）
- **详情**：outbox/coder-1/result.md + logs/20260814.md

## 2026-08-14 — coder-2 — DESIGN-OVERLAY-037 ✅ 流式预览 overlay 交互设计（纯设计零代码改动）

- **来源**：Gavin 新交互架构（流式输出 + 组合文本预输入 + 松键后 LLM 格式化）。
  TSF 路径被判死（RESEARCH-TSF-036）后，主控要求出 overlay 回退方案。基线 HEAD `35a2a74`
- **任务性质**：纯设计文档，禁止改任何代码
- **核心结论**：
  - **复用现有 Win32 GDI overlay（DEC-003），不新建窗口**
  - **双阶段窗口样式**：录音/流式显示阶段保持 `WS_EX_NOACTIVATE`；用户接管编辑阶段动态移除该样式并内嵌 `EDIT` 子控件
  - **编辑实现推荐 Win32 `EDIT` 控件 + 子类化去边框**：中文 IME 支持是决定性因素
  - **PTT/Toggle 编辑时机（Gavin 拍板）**：录音中用户点击 overlay 文本区即进入编辑态，不等松开热键；编辑态内再次松开热键为空操作（`pipeline_cancelled` 拦截）
  - **窗口几何**：文本从客户区中央起排，随文本量以屏幕中心为锚点扩宽，上限 = 当前显示器工作区宽度 × 65%；超上限后文本区内部左滚
  - **焦点归还**：提交时先 `SetForegroundWindow(target_hwnd)`；UIPI 失败则降级为复制到剪贴板 + `FocusLost` 预览提示
  - **颜色语义（Gavin 拍板）**：语音文本专用白色 `COLORREF(0xFFFFFF)`，**不引入下划线**，整段流式文本统一白色；提交按钮用品牌橙
  - **EDIT 控件精确规格**：尺寸 (42, 10)-(191, 26)、高 16px、背景 `BG_DARK`、文本白、无边框、无滚动条、选中高亮 `BRAND_ORANGE`、字体 `create_clear_type_font(-12)`
  - **托盘状态**：编辑态期间托盘仍为 `Recording`，不随 2500ms 自动复位为 Idle
- **交付物**：`voice-ime/collab/research/overlay-streaming-preview-design-001.md` + `/d/Workspace/CodeLab/collab/outbox/coder-2/result.md`
- **零生产代码改动**：`git diff --ignore-cr-at-eol --numstat -- src/` == 0，版本号未动
- **下游**：设计已按 Gavin 拍板修订完成，待主控三轮验收
- **详情**：本条目 + `outbox/coder-2/result.md` + `logs/20260814.md`

## 2026-08-14 — coder-1 — RESEARCH-ASR-038 ✅ 035 流式化重做 + VAD 计费门控 + 热词注入链路（纯设计零代码改动）

- **来源**：Gavin 推翻 035「录完整段再发」前提，要求真流式「边录边显示」。基线 HEAD `35a2a74`
- **设计产出**：`collab/research/asr-streaming-pipeline-design-001.md`（469 行）
  - 计费口径：文档未覆盖，按保守设计（墙钟会话时长）
  - VAD 门控：只做①入口门控（滚动 VAD 确认有语音才建连，2s 无条件建连保底），②暂不做，③自然行为
  - speech_detected 不换（RMS 管停录 / Silero 管建连并存）
  - 流式管线：音频采集+ASR并发，StreamingAsrState 维护已确认句+当前句，松键后整段交 LLM（F3 跨句保留）
  - overlay 接口：PipelineEvent::StreamingText 推增量，EditRequested → 关 WS + 丢弃 + pipeline_cancelled
  - 热词注入：全量注入（~15条），source 分权重（user=5/system=4），超限丢弃 ASR 保留 LLM
  - 参数调优：max_sentence_silence=800ms，全部配置文件级不暴露（DEC-031 通过）
  - 分批 A→B 串行 C 并行
- **零生产代码改动**：`git diff --ignore-cr-at-eol --numstat -- src/` == 0，版本号未动
- **下游**：方案待主控复核 + Gavin 拍板后排期实施批次 ASR-038-A/B/C
- **详情**：`outbox/coder-1/result.md`（工作区级+项目级两处）+ 方案文档 + `logs/20260814.md`

## 2026-08-14 — coder-1 — RESEARCH-TSF-036 ✅ Windows TSF 组合文本可行性调研（纯研究零代码改动）

- **来源**：Gavin 新交互架构（流式输出 + 组合文本预输入 + 松键后 LLM 格式化）。基线 HEAD `35a2a74`
- **核心问题**：跨进程插入 TSF 组合文本，飞音是否必须注册成活动的 TIP？
- **三选一结论：丙（判死 TSF，走 overlay 回退方案）**
  - A1 答案：是，必须注册成活动 TIP（官方架构文档：text service 是 in-proc COM server，被加载进目标进程）
  - TIP 崩溃直接拖垮宿主应用（in-proc COM 无进程隔离）
  - 绿色免安装形态不能保持（需写注册表 + 代码签名 + DLL 注册）
  - 无绕开路径（IMM32 同样要求活动 IME；UI Automation 无组合态）
  - 产品形态不可逆变更（托盘工具→真·输入法，与现有交互根本冲突）
  - 工程量 ~30-80 人天，PoC 不值得做（核心问题已有文档级答案）
  - 回退方案 = overlay 浮层实时显示 + 松键后注入（与当前体验一致，无退化）
- **跨平台抽象**：`CompositionText` trait 已设计（begin/update/commit/cancel），三端映射已给，但结论为丙暂不落地
- **零生产代码改动**：`git diff --ignore-cr-at-eol --numstat -- src/` == 0，版本号未动
- **下游**：方案待主控复核 + Gavin 拍板（是否接受 overlay 回退 / 是否走 TIP / 是否做 PoC）
- **详情**：`outbox/coder-1/result.md` + 方案文档 + `logs/20260814.md`

## 2026-08-14 — coder-1 — RESEARCH-ASR-035 ✅ qwen-audio-3.0-asr-flash-streaming 接入研究（纯研究零代码改动）

- **来源**：Gavin 2026-08-14 指令（换在线 ASR + VAD 门控 + 热词/上下文 + 降噪研究）。基线 HEAD `35a2a74`
- **研究产出**：`collab/research/asr-qwen-audio-3.0-integration-001.md`（~650 行方案文档）
  - A 协议层 6 问全答（run-task/二进制帧/服务端事件/分片/鉴权/WorkspaceId）
  - B 能力层 5 问全答（即时热词无需预建词表 / 上下文 400 字符限制 / language_hints 数组 / 标点自带无独立开关 / PCM 16kHz 直接兼容）
  - C 商务层 3 问（按时长计费非 token / 限流文档未覆盖 / GA + 北京 region + 同一 API key）
  - D 降噪 7 问 + 三选一建议 A（不做，端测后视情况转 B 用 nnnoiseless）
  - 整合方案：新写 `qwen_inference.rs` 保留旧模块 / VAD 门控复用 VadSegmenter / 热词复用 wordbook / 上下文 v1 不上
  - DEC-031 核对通过 / 跨平台结论 / 影响文件清单 / 风险与回退 / 分批建议 A→B→C
  - 7 个未解问题待 Gavin 拍板
- **零生产代码改动**：`git diff --ignore-cr-at-eol --numstat -- src/` == 0，版本号未动
- **下游**：方案待主控复核 + Gavin 拍板后，由主控排期实施批次 ASR-035-A/B/C
- **详情**：`outbox/coder-1/result.md` + 方案文档 + `logs/20260814.md`

## 2026-08-09 21:2x — tester-1 — BUILD-015 ✅ 030 全批 + 031 首次出包（⚠️ handoffs/progress 主控代记）

> ⚠️ **[DOC-STATE-DRIFT-001] 今日第三次**：tester-1 完成后仍只更 `CHANGELOG.md` + `logs/20260809.md`，
> `handoffs.md` / `progress.md` 零条目。主控代记，保留问责链。

- **基线**：HEAD `faa672d`。构建前产物为 08-04 19:0x（不含 030/031），本次是 030/031 **首次进 exe**
- **四步构建**：Step1 清进程 → Step2 npm build + Tauri UI（`--features custom-protocol`）**2m01s**
  → Step3 主程序 **2m24s** → Step4 同步 Publish
- **七项核验全过（主控已逐项独立复算，非采信表格）**：

  | # | 项 | 主控独立复算结果 |
  | --- | --- | --- |
  | ① | 六 exe 时间戳 | target 21:09/21:11/21:12 ｜ Publish 三份 21:13，**全为今天** ✅ |
  | ② | 三 exe sha256 两副本 | `831c254d…`／`14411dee…`／`9fa58f9b…` 三对全等 ✅ |
  | ③ | 两 toml 三副本 | scene `0a3a0b9a…`×3 ｜ itn `b208271b…`×3 ✅（**修复见下**） |
  | ④ | ProductVersion | feiyin `0.7.3.0` ｜ ui `0.7.3` ｜ crash `0.7.3.0`，版本号未动 ✅ |
  | ⑤ | UI 嵌入新前端 | ui.exe 晚于 `ui/dist/` ✅ |
  | ⑥ | 冒烟启动 | PID 24024 `Responding=True` @21:20:57，**测后已 Stop-Process 清理**（主控复查无进程属预期，非虚报）✅ |
  | ⑦ | 产物大小 | 11957248 / 10026496 / 24857600 —— 主程序较 08-04 基线 **+17408B（+17KB）**，UI 与 crash **与基线完全相同**（本批零前端零 crash 改动）✅ |

- **🔴 本次最有价值的发现（tester-1 例行核对抓到）**：`scene-rules.toml` 的 `Publish/` 与
  `target/release/` 两副本仍是 **08-03 版 41714B**，根目录已是 **45591B**（差 3877B 实质内容）。
  已 cp 根目录版收敛三副本。**这是 `[TOML-STALE-001]` 的第二次发作，且是一条全新来路** ——
  toml 由 macOS 端 `f96c817`（08-05）改动，经 merge `7e76465`（08-08）进入本端，
  **本端从未编辑过该文件**，故不会有任何「该同步了」的触发点。窗口内恰好没出包，未流到 Gavin 手上
- **根因升级**：`build-test-guide.md` Step 4 原文**只 cp 三个 exe，完全没提 toml**，三副本规范只写在
  troubleshooting 里没落到可执行步骤 → 靠人记就一定会漏
- **主控已落地的两项流程修复**：
  1. Step 4 补入 toml 同步命令 + 三副本 sha256 验证 + 「不得同步」清单（`config.toml`/`wordbook.sqlite`/`debug.log`/`version_check.json` 属 Gavin 运行时数据）
  2. `[TOML-STALE-001]` 新增第 3 条强制规则：**跨端 merge 后的首次出包必须显式核对两 toml 三副本**，
     自查命令 `git log --oneline <上次出包commit>..HEAD -- scene-rules.toml itn-rules.toml`
- **顺带修正文档错误（tester-1 提出，主控核实采纳）**：`build-test-guide.md` 的产物大小基准
  ~31MB/~22MB 是 **DEC-021 体积优化之前**的旧值，与实测（11.9/10.0MB）长期不符，已按实测改写，
  并改口径为「与上次出包逐一对照，不作硬阈值」；构建耗时 ~47s 亦改为实测 ~4m30s
- **零生产代码改动**，版本号未动（0.7.3）
- **下游**：⏭ **交 Gavin 端测**（`Publish/feiyin-ime.exe`）
- **详情**：`outbox/tester-1/result.md`（8280B）+ `CHANGELOG.md` + `logs/20260809.md`

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

