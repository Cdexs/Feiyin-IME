# todo 归档 · voice-ime

> **2026-08-17 主控归档**：`todo.md` 已膨胀到 2448 行（约 112K tokens），
> 严重违反 `worker-guide.md` 第九节防膨胀规则与「每 session 必读文档合计 ≤20K tokens」预算。
> 本文件收纳从 `todo.md` 迁出的**已结案 / 已闭环 / 已出包 / 历史交接**章节，**内容零删改**。
>
> 迁出范围（原 `todo.md` 行号）：2026-08-16 收工交接 ｜ BUILD-016 ｜ 2026-08-15 会话暂停交接 ｜
> TEST-SYNC-038-B + TEST-EXEC-038 ｜ TRANS-HOTKEY-039 全批 ｜ v0.8.0 在线 ASR 引擎更替批次（08-14）｜
> 2026-08-09 会话中断交接 ｜ 无序枚举不出列表（DEC-049）｜ 2026-08-08 批次 ｜ ITN-FIX-BIGNUM-027 实施记录 ｜
> ITN-FIX-CHAIN-TEAR-026 ｜ 2026-08-02 P0 与双线批次 ｜ PROMPT-ARCH-020 ｜ ITN-V2 全批次 ｜
> RESEARCH-ITN-V2-001 ｜ macOS 07-30/07-31 交接接管与历史批次 ｜ 07-30 出包系列 ｜
> TEST-EXEC-043 / BUILD-018 交付明细
>
> **仍在 `todo.md` 的**：未结案 P0、待派发 / 待排期 / 待裁定、待 Gavin 拍板、已知遗留问题、macOS 未派发规划。
>
> 原始完整快照另存 `todo.md.bak-20260817`（不入 git）。

---

## 🛑 2026-08-16 收工交接 —— 明天从这里开始

> Gavin 收工（当晚）。**下一个动作是等他端测结果，不是派新任务。**

| 项 | 状态 |
| --- | --- |
| HEAD | `e182311`，工作区 **clean**，零悬空改动 |
| 本批四个 P0/测试任务 | ASR-042 `34906a1` / TEST-SYNC-042 `427cd50` / ASR-045 `54cde62` / TEST-SYNC-045 `fd994a5` **全部已验收提交** |
| 阶段四全量回归 | ✅ `2a173f0` —— 1030/0/11，③ 类真回归 = 0 |
| 阶段五出包 | ✅ BUILD-017 `2074859` —— 产物 08-16 23:25 在 `Publish/`，七项核验主控独立复算通过 |
| 三个 Worker | coder-1 / coder-2 / tester-1 **全部空闲待命，手上零任务** |
| 未 push | 本地 ahead 若干，**Gavin 未授权 push，不得自动执行** |

### 明天第一件事：问 Gavin 端测结果（两个 P0 一眼可判）

1. **识别内容对不对** —— 修复前输出「你好，花呗还不上」这类完全不相干的客服话术（48kHz 按 16kHz 解）
2. **文字上不上屏** —— 修复前悬浮层实时出字，松开热键后文字凭空消失

### 端测环境（已核，勿重复排查）

- Gavin 跑的是 **`target/release/feiyin-ime.exe`（PID 4696，他自己启的）**，sha256 与 `Publish/` 版**完全相同**，等价于测包
- 其 `debug.log` 落在 **`target/release/debug.log`**，不在 `Publish/`
- `target/release/config.toml` 三项 ASR 配置已核对全对：`qwen_audio_online` / `/api-ws/v1/inference` / `qwen-audio-3.0-asr-flash-streaming`，api_key 非空
- 🔴 **`enable_streaming = false` 是虚惊，不影响真流式** —— `main.rs:3276` 的 `is_streaming_asr` 只看 `asr_model`，与该开关无关。**别再为这行重新排查一遍**

### 端测通过后的下一棒

1. **OVERLAY-043**（coder-2 待命中）—— 录音窗口四项显示错乱，四条现象主控已逐条核过代码，见下方专节
2. **TEST-045-REFACTOR**（排在 OVERLAY-043 之后）—— 抽 `decide_pipeline_entry` 纯函数补调用侧护栏缺口

### 端测若失败

`debug.log` 优先看这两条：
- ASR-042 是否生效：搜 `Streaming resampler active` —— 有这行说明重采样器接上了
- ASR-045 是否生效：搜 `No audio samples recorded` —— 流式录音后**不应**再出现这行

### 🔴 两个不许动的东西（Gavin 2026-08-16 明确指示）

- `target/release/feiyin-ime-nor.exe`（08-09，11.9MB，非当前构建目标）—— **是 Gavin 的备份 exe，不得删除**
- **PID 4696** —— Gavin 端测进程，不得 kill

## ✅ 已结案（主控复算通过） · BUILD-016 v0.8.0 首包出包（tester-1，2026-08-16）

> ✅ **2026-08-16 tester-1 完成**（基线 `be76fc1`，Gavin 已下达出包指令），主控将独立复算后交 Gavin 端测。
>
> - **构建**：Step 1 清进程（feiyin-ime PID 23888）→ Step 2 npm build（新 `index-DkzLqu_f.js`）+ Tauri UI release 2m14s（cp 到 target/release/）→ Step 3 主程序 2m40s → Step 4 同步 Publish/（三 exe + scene/itn 两 toml；config.toml 等运行时数据未覆盖）
> - **七项核验全 PASS**（详见 outbox/tester-1/result.md，机器实测）：① 六 exe 时间戳 00:45-00:48 ② 三 exe 两副本 sha256 逐一相等 ③ 两 toml 三副本一致（`0a3a0b9…`/`b208271…`）④ ProductVersion 0.8.0.0/0.8.0/0.8.0.0 ⑤ 新前端 `index-DkzLqu_f.js` 嵌 ui.exe / 旧名 `CTgGziQm` 0（i18n 裸串 grep 0 系 Tauri 压缩已知行为）⑥ 冒烟 PID 30276 Responding=True 无 panic 已清理 ⑦ 主程序 +149504B / UI 0B / crash +2048B 已解释
> - **🔴 双探针**：正向 `qwen-audio-3.0-asr-flash-streaming` / `api-ws/v1/inference` 各 1 命中；反向 `api-ws/v1/realtime` / `qwen3-asr-flash-realtime` 0 命中——新 ASR 进包 + 旧引擎清干净
> - **产物**：`Publish/feiyin-ime.exe` 12,106,752B / `feiyin-ime-ui.exe` 10,026,496B / `crash-reporter.exe` 24,859,648B
> - **Gavin 端测重点**（`-debug` 跑）：新端点连通性 / usage.duration 计费口径 / 040-A 四段连接耗时 / VAD 门控行为 → ASR-PERF-040-B/C 优化数据源

---

## 🛑 2026-08-15 会话暂停交接 —— 下次从这里开始

> **暂停原因**：coder-2 token 额度用尽（Gavin 指令：暂停开发，额度恢复后继续，完成再通知主控）。

### 断点状态（主控实测取证，非推测）

| 项 | 状态 |
| --- | --- |
| 编译 | ✅ **`cargo check --all-targets` 0 error**（暂停前实测） |
| 工作区 | ✅ 已全部提交，无悬空改动 |
| coder-2 上下文 | ⚠️ 从 82% **掉到 32%** —— session 被压缩或重启，**恢复时必须重新注入任务上下文** |
| coder-1 / tester-1 | ✅ 空闲，本轮任务全部结案 |

### 各任务状态

| 任务 | 负责人 | 状态 |
| --- | --- | --- |
| ASR-038-B 真流式（C-1~C-4 + 040-A 埋点） | coder-1 | ✅ **已验收已提交** `4f3b41b` |
| TRANS-HOTKEY-039 翻译热键 | coder-2 | ✅ **已验收已提交** `4f3b41b` |
| TEST-SYNC-038 测试同步（10 用例） | tester-1 | ✅ **已验收**（返工一轮） |
| TRANS-HOTKEY-039-D 抽纯函数补真护栏 | coder-1 | ✅ **已验收**（更正一处过强结论） |
| **ASR-038-C overlay 边说边上屏** | coder-2 | 🔴 **返工修复已完成，待主控验收** |

### 🔴 ASR-038-C 恢复开发时必须先修的三项（主控代码验收查出）

**① P0：编辑态一进就被销毁，headline 功能失效**

证据链（四步全部主控 Read 取证）：

```
main.rs:2834  EditRequested 处理里 cancel_signal.store(true)
main.rs:3289  流式 worker 检测到 cancel_signal → send_event(PipelineEvent::Cancelled)
main.rs:2758  Done | Cancelled 分支 → overlay_handle.send(OverlayCommand::Hide)
main.rs:993   Hide 处理 → destroy_edit_control() + state.request = None（无任何守卫）
```

→ 用户点 overlay 文本区进编辑态，**EDIT 控件刚建好就被销毁，overlay 消失**。

> 037 设计里本有 `pipeline_cancelled` 拦这个，但**全库只出现在 `main.rs:153` 的文档注释里，从未实现**。
> **修法（双保险，建议都做）**：① 主控侧维护编辑态标志，编辑态时 `Done|Cancelled` 不发 `Hide`；
> ② overlay 侧 `Hide` 加守卫，`status == StreamingEditing` 时忽略。

**② 托盘状态**：037 要求编辑态期间托盘保持 `Recording`，但 `:2758` 的 `Cancelled` 分支
`set_tray_state(Idle)`。与 ① 同根因，一并修。

**③ 100ms 尺寸节流缺失**：`:2741` 注释引 037 §5.2 称「靠 16ms timer 自然节流」——
但 16ms timer 节流的是**重绘频率**，挡不住**窗口宽度每帧都变**。037 §5.2 与风险表第 4 条
明确要求第二道尺寸节流防抖。**请补。**

### ✅ ASR-038-C 已做对的部分（恢复时不要重做）

`StreamingText`/`EditRequested` 双向接线 ｜ 双阶段样式 `remove_noactivate`/`restore_noactivate` ｜
`EDIT` 子类化 ｜ UIPI 降级到剪贴板 ｜ 白色 `0xFFFFFF` + 品牌橙 `0x006BFF` ｜
65% 宽度上限 `STREAMING_OVERLAY_MAX_SCREEN_RATIO` ｜ `cargo check` 0 error

> 📌 **EDIT 几何他做得比设计稿好**：037 写死 `(42,10)-(191,26)`，他改成按边距从 overlay rect 推导
> （`STREAMING_TEXT_LEFT_MARGIN = 42` 与规格吻合）。窗口随文字变宽时硬编码那版会错位，
> 他这版能自适应。**这是正确的偏离，不要改回去。**

### 🔴 另一个独立缺口：新 ASR 引擎【没有任何途径被启用】

| 层 | 状态 |
| --- | --- |
| Rust 后端 | ✅ 认 `qwen_audio_online`（`transcription/mod.rs:48`） |
| **UI 下拉** | ❌ **`qwen_audio_online` 在 `ui/src` 里零命中** |
| Gavin 的 config | ❌ `asr_model = "qwen3_online"`（旧 Realtime 引擎） |

**后果**：即使 038-C 修好、包也出了，端测跑的仍是**旧引擎** ——
新协议 / 真流式 / VAD 计费门控 / 热词注入 / 040-A 埋点**一样都不会触发**。

> 与今天的端点三缺陷同类：**后端做完了，开关没装上。**
> **待办**：① 补 UI 下拉选项（`ui/`，可派 coder-1，与 coder-2 文件域零重叠）；
> ② 端测前可先手改 `config.toml` 的 `asr_model = "qwen_audio_online"` 抢先验证新引擎连通性。

### 模型 ID 核对（Gavin 2026-08-15 特别确认，三处一致）

**新引擎 = `qwen-audio-3.0-asr-flash-streaming`**
（`qwen_inference.rs:47` `DEFAULT_MODEL` / `config/mod.rs:203` / `transcription/mod.rs:948,957` 断言）

| | 旧（现役） | 新（集成中） |
| --- | --- | --- |
| 模型 ID | `qwen3-asr-flash-realtime` | **`qwen-audio-3.0-asr-flash-streaming`** |
| 配置项 | `qwen3_asr_model` | **`qwen_asr_model`** |
| API / 路径 | Realtime `/api-ws/v1/realtime` | **Inference `/api-ws/v1/inference`** |
| model 位置 | URL query | **payload 内** |
| 音频帧 | base64 文本帧 | **二进制帧 3200B/片** |
| `asr_model` 枚举 | `qwen3_online` | **`qwen_audio_online`** |

两套完全并存，WorkspaceId（`llm-kudx4dj2bfqn4gr2`）与 API key 共用，仅路径与协议不同。
**今天的端点三缺陷根因就是两边命名太像，A 批写成了「用新协议连旧端点」。**

### 恢复后的执行顺序（Gavin 已拍板）

```
① coder-2 恢复 → 修 038-C 三项 → 主控验收
② 补 UI 下拉选项（qwen_audio_online）
③ tester-1：TEST-SYNC-038-B（main.rs 用例，规格表已在 outbox/tester-1/result.md）
      ↓ 串行
④ tester-1：TEST-EXEC-038 全量回归（阶段四，本批从未跑过全量）
⑤ BUILD-016 出包（阶段五）🔴 **2026-08-16 DEC-053 已改规则：阶段四通过后主控直接派发，不再请示 Gavin**
```

> ⚠️ **出包后建议 Gavin 首次用 `-debug` 跑**：新端点连通性 / 计费口径（`usage.duration` 是
> 上传音频时长还是墙钟会话时长，官方文档未覆盖）/ 040-A 四段耗时 / VAD 门控实际行为
> —— 这四项**只有 debug.log 能给答案**，且是 040-B/C 连接优化的唯一数据来源。

---

> ✅ **产物已更新**（2026-08-03 **17:42**，BUILD-012）：`Publish/feiyin-ime.exe` `DB07CEFD8D51` / `feiyin-ime-ui.exe` `46D0F31E149D` / `crash-reporter.exe` `699ED9656958`，包含 017/018/020/021/**023**（四语标记清单恢复扩充）。三 exe 两副本 sha256 一致，两 toml 三副本一致（`scene-rules.toml` `7C1F0620` / `itn-rules.toml` `ED77A912`），ProductVersion 0.7.3.0/0.7.3。**⏭ 待 Gavin 端测。**
> 🔴 **2026-08-04 主控核查：上方产物不含 ITN-FIX-CHAIN-TEAR-026** —— 产物 mtime `17:42:42` 早于 `src/itn.rs` `23:47:25` 整 6 小时。026 需 BUILD-013 才进 exe，详见下方 P0 节。
> ⏳ **本地 ahead 2 未 push**（最新 `2447dbb`，Gavin 只授权提交，不授权 push）。
> 🔴 **2026-08-15 主控核查：上方 BUILD-012/015 产物段落均已过期** —— 当前处于 **v0.8.0 在线 ASR 引擎更替**批次，
> HEAD `4b3c63f`（含 WIP `003ba65`）。coder-1 已完成 038-B 真流式核心实施（C-1~C-4+040-A），**当前可编译，949+32 测试全绿**，
> 待主控终验（重点验 FIRSTCHAR 等价 + record 零改动 + 040-A 四段覆盖），详见下方「当前批次」节。
> ✅ **2026-08-08 已合并 origin/main 的 6 个 macOS Phase 4 提交**（merge `7e76465`，+6694/−281，零冲突，`cargo check` 0 error）。mac 端顺带跑了全库 `cargo fmt`，后续改动须保持 rustfmt 风格。
> 端测方式（2026-07-25 Gavin 指示）：Gavin 已在**实际日常使用中自行测试**，端测项不再列入本文档；发现 bug 或优化点由 Gavin 邀请重新开单。

> ✅ **TEST-EXEC-024 + BUILD-012-VERIFY 已闭环并提交 `7a1329e`**（崩溃中断续做，tester-1 2026-08-03 18:3x，主控 18:4x 独立验收）。详见 CHANGELOG / `logs/20260803.md`（规则 3：测试同步与出包不在本文档详列）。

---

## ✅ 已结案（主控标注） · TEST-SYNC-038-B + TEST-EXEC-038（tester-1，2026-08-15）

> ✅ **2026-08-15 tester-1 交付 + 主控独立复算通过**（基线 `ee4d472`），等主控统一提交。
>
> **阶段三（TEST-SYNC-038-B）新增 6 条真测用例**（仅 3 测试文件 +164 行，生产零改动）：
> - `src/config/mod.rs`：`asr_online_url/model` legacy alias 真读 ×2 + `qwen3_online→qwen_audio_online` 迁移 load() 路径 ×1
> - `src/transcription/mod.rs`：`AsrModel` 三变体穷举 match 编译期护栏 ×1
> - `src-tauri/src/config.rs`：镜像与主 config 字面值一致性 ×2
>
> **可测性判定**：038-C overlay 五覆盖点全内联 Win32 消息循环，判不可纯单测（理由 + 抽纯函数建议交主控排期）；main.rs 规格表热键下沿/硬上限/旧路径已由 039-D 纯函数与既有用例覆盖。
>
> **阶段四（TEST-EXEC-038）全绿**：A0-A4 全 PASS；主 crate 1014 passed/0 failed；src-tauri 55；Vitest 54；`--list` 自洽（1025 = 950+39+36）；红条三分类 ③0/①0/②0。
>
> **残差 3 对账**（主控源码 1028 vs 实跑 1025）：crash-reporter bin 经 `#[path]` 重复引入 config(36)+i18n(3)，`--list` 分 target 明细 = feiyin-ime 950 / crash-reporter 39 / 集成 36，实跑合计 1025；源码去重计数把重复引入的 3 处 cfg 门控差异算入即约 1028，不影响结论。

---

## ✅ 已结案（主控标注） · TRANS-HOTKEY-039 翻译热键全链失效（2026-08-15）

> ✅ **2026-08-15 主控终验通过并提交 `4f3b41b`**（打回一轮后）。
> `cargo fmt --check` clean ｜ `cargo check --all-targets` **0 error**（主控独立复算）｜
> 五文档 + `MACOS-HANDOFF` grep 全部命中 ｜ 收尾自证表九行填满 ｜ coder-1 在途改动未被误伤。
>
> ⏭ **待 Gavin 端测**：修好后 UI 上热键会**显示为 "Left Shift"** —— 那不是又坏了，
> 是它第一次说实话（config 存的本来就是 `160` = 左 Shift）。**要用右 Shift 需重新录制一次。**
> 另：根因 B 修好后「录音中途按翻译键」才真正可用（原先只有 500ms 窗口）。

### ✅ TRANS-HOTKEY-039-D 抽判据纯函数 + 清理死常量（coder-1，2026-08-15）

- **来源**：主控验收 TEST-SYNC-038 时发现假护栏（tester-1 闭包自述同义反复）
- **改动**：仅 `src/platform/windows/hotkey.rs`（+90/-6）
  - ① 抽 `should_stop_translate_poll_on_keyup(mode: u32) -> bool` 纯函数
  - ② 抽 `hotkey_mode_to_u32(mode: HotkeyMode) -> u32` 纯函数（消除三处硬编码 1/0 漂移）
  - ③ 删死常量 `TRANSLATE_WINDOW_MS`（全库零使用）
  - +4 真护栏测试（调生产函数，把 store(true) 移出 if 门控或映射改错会变红）
- **验收**：cargo fmt clean / cargo check --all-targets 0 error / cargo test hotkey 23/0
- **行为零变更**：PTT 抬起仍停、Toggle 抬起仍不停

### ✅ ASR-041 在线 ASR 模型换代 UI 选项 + 存量配置静默迁移（coder-1，2026-08-15）

- **来源**：Gavin 指令「用新模型替代旧模型，改选项文本」
- **改动**：6 文件（Voice.tsx / Voice.test.tsx / en.ts / zh-Hans.ts / zh-Hant.ts / config/mod.rs）
  - 下拉 value + desc case `qwen3_online` → `qwen_audio_online`
  - 三份 i18n 去 Qwen3 字样（`在线语音识别模型`/`線上語音辨識模型`/`Online Speech Recognition`）
  - Voice.tsx 4 处逻辑判断同步（切换检查 key/显示 key 输入框/卸载回退）
  - config/mod.rs load+load_from 存量 `qwen3_online` 静默迁移为 `qwen_audio_online`（含 log+落盘）
  - Voice.test.tsx 17 处同步；+2 迁移单测
- **验收**：cargo fmt clean / cargo check --all-targets 0 error / npm run build 通过 / config 43/0 / grep qwen3_online ui/src 零命中
- **旧代码路径保留**（Qwen3Online 枚举/qwen3_online.rs/qwen3_asr_* 不删，回退能力）

### ✅ ASR-041-B 清除旧在线 ASR 代码路径 + 字段改名通用名（coder-1，2026-08-15）

- **来源**：Gavin 指令「连代码一起清掉，不用保留旧的在线 asr 模型作为回退兜底」+「qwen3_api_key 改成通用名」
- **删除**：Qwen3Online 枚举 + qwen3_online.rs(686行) + qwen3_asr_url/model 配置 + Transcriber qwen3 字段/参数 + 热重载 qwen3_changed + 所有 match 臂 + 测试
- **保留**：①asr_online_api_key+serde alias ②f32_to_pcm16_le 搬家到 qwen_inference.rs ③qwen3_online→qwen_audio_online 存量迁移 ④Accuracy
- **字段改名**：qwen3_api_key→asr_online_api_key + qwen_asr_url→asr_online_url + qwen_asr_model→asr_online_model（三字段统一 asr_online_ 前缀 + serde alias）
- **镜像补字段**：src-tauri/config.rs 补 asr_online_url/model（038-A 遗留隐患）
- **验收**：cargo fmt/check 0 error / src-tauri check 0 error / cargo test transcription 136/0/7ign / config 41/0/2ign / npm run build 通过 / grep Qwen3Online src/ 零代码命中

> 🔴 **2026-08-15 主控更正状态**：coder-2 曾把本节标为「✅ 已结案」，但**当时主控已打回**，
> Toggle 回归尚未修复。**状态不实，已改回「验收中」。**
> 教训与 `[DOC-STATE-DRIFT-001]` 同族但方向相反：不是漏写，是**提前宣告完成**。
> **结案状态只能由主控在验收通过后标注，Worker 不得自行标 ✅ 已结案。**

> 已移入 CHANGELOG / `logs/20260815.md` / `handoffs.md`。本节仅留梗概。

> **Gavin 原话**：「无论是否开启格式化输出（注意是走的不同的翻译路径），在录音的时候同时按下翻译热键，
> 输出的结果并没有进行翻译，比如从中文翻译到英文。」
>
> **性质**：功能 100% 不可用，且**两个独立根因各自都足以致死** —— 不是一个 bug，是两个叠在一起。
> 「无论是否开启格式化输出都不翻译」正是这个特征：两条翻译路径共用同一个上游 flag，flag 恒 false，
> 下游走哪条路径都一样。

### 取证一：`translate` flag 恒为 false（日志 109 条命中，无一例外）

`target/release/debug.log`（2026-08-15 12:15:55，362KB）中**每一条**
`Controller received hotkey start (translate=false)`，含今日 04:11–04:15 全部会话。

**但这不是 bug 本身** —— `HotkeyEvent::Start` 携带的是 `Arc<AtomicBool>`，
`spawn_translate_poll_thread`（`hotkey.rs:99`）本就设计成「录音开始后继续轮询、中途按下也能翻转」，
真正消费点在 `main.rs:3430` 的 `translate.load(Ordering::Acquire)`（管线阶段读，时机正确）。
日志打的只是 Start 那一刻的值，**不足以定罪**。

### 🔴 根因 A：UI 标签表 Shift 左右互换（一行，最致命）

`ui/src/pages/HotkeySettings.tsx:43`：

```ts
0xA0: 'Right Shift', 0xA2: 'Left Ctrl', 0xA4: 'Left Alt', 0xA1: 'Left Shift',
```

**Windows 事实**：`VK_LSHIFT = 0xA0`、`VK_RSHIFT = 0xA1`。**这两个标签是反的。**

对照同表其余四条**全部正确**：`0xA3: 'Right Ctrl'`（VK_RCONTROL=0xA3 ✅）/
`0xA5: 'Right Alt'`（VK_RMENU=0xA5 ✅）/ `0xA2: 'Left Ctrl'` ✅ / `0xA4: 'Left Alt'` ✅。
**只有 Shift 这一对是错的**，所以长期没被发现。

**完整失效链**（与 Gavin 的 config 实测吻合）：

```
target/release/config.toml:67-70
  [translation] enabled = true / vk_code = 160 / display_name = "Right Shift"
       ↓ display_name 由 getHotkeyDisplayName(160) 经上述错表算出
  UI 告诉 Gavin：「你的翻译热键是 Right Shift」
       ↓ 运行时 translation_pressed() → GetAsyncKeyState(160) = 物理 Left Shift
  Gavin 按物理 Right Shift（0xA1）→ 永远 false
```

**结论：只要 Gavin 按的是右 Shift，翻译功能 100% 死，与录音时机、格式化开关全都无关。**

> ✅ **2026-08-15 Gavin 已确认**：「我按下的是右 shift，我在配置里配置的就是右 shift」。
> **根因 A 由「疑似」升格为「已确认的元凶」** —— 他在 UI 里选的是标着 "Right Shift" 的项，
> 存进去的是 `160`（物理左 Shift），运行时等的也是物理左 Shift，而他按的是物理右 Shift（`161`）。
> **三方一致地错开，功能自配置之日起就从未生效过。**
> 根因 B（500ms 窗口）是**修好 A 之后立刻会咬人的第二颗雷**，两条必须一起修。

> 📌 附带疑点（需一并查清）：`HotkeySettings.tsx:69` 的
> `TRANSLATION_SINGLE_KEYS = [0xA3, 0xA2, 0xA5, 0xA4]` **不含任何 Shift**，
> 而 `config/mod.rs:128` 的 `translation.vk_code` 默认是 `0`。
> 那么 **160 是怎么进到 config 里的？** 说明除下拉外还有一条按键捕获路径，
> 且该路径可能把物理右 Shift 捕获成 `0xA0`。**根因 A 若只改标签表而不查捕获路径，可能只治了一半。**

### 🔴 根因 B：`TRANSLATE_WINDOW_MS = 500`，轮询窗口只有 0.5 秒

`src/platform/windows/hotkey.rs:29-30`：

```rust
const TRANSLATE_POLL_MS: u64 = 10;
const TRANSLATE_WINDOW_MS: u64 = 500;
```

`spawn_translate_poll_thread` 只在**录音开始后 500ms 内**盯着翻译键，到点线程退出。
`translate_flag.store(true, …)` 全代码库**只有 `hotkey.rs:108` 一处**（另一处是创建时的
`AtomicBool::new(translation_pressed())`）→ **超过 500ms 后 flag 再无任何途径变 true。**

**这正是 Gavin 描述的场景**：「在录音的时候同时按下」= 录音已经跑了若干秒才按 → 必然错过窗口。

**根因 A 与 B 相互独立**：修好 A 之后，B 依然会让「录音中途按下」失效。**两个都必须修。**

### 🟡 根因 C（跨平台缺口）：macOS 侧翻译热键从未接线

`src/platform/macos/hotkey.rs:137` 与 `:153` 两处**硬编码** `translate: Arc::new(AtomicBool::new(false))`，
无 `translation_pressed()`、无 poll 线程。**macOS 端翻译热键完全不存在，不是失效是没做。**
依「跨平台强制规则」必须给出明确结论，不得沉默跳过。

### ✅ 结案摘要

- **039-A**：`VK_TO_LABEL` 中 Shift 左右标签已修正（`0xA0`→Left Shift、`0xA1`→Right Shift）。`CODE_TO_VK` 捕获表经复核本身正确，Gavin 配置中的 160 来自他当时按了物理左 Shift，被错标签误导为 "Right Shift"。
- **039-B**：轮询从固定 500ms 改为跟随录音生命周期（`TRANSLATE_POLL_STOP` 信号 + PTT 松开/Toggle 二次按下/hook 卸载/poll_ptt_release_thread 结束均置位），并加 `MAX_RECORD_SECONDS + 5` 秒硬上限兜底；flag 翻转日志已补。
- **039-C**：macOS 侧未实施，结论写入 `docs/MACOS-HANDOFF.md` §TRANS-HOTKEY-039。
- **跨文件接线**：`src/main.rs` 6 处终止路径调用 `platform::notify_translate_poll_stop()`（:1978 / :1987 / :2028 / :2032 / :2045 / :2063），与 coder-1 ASR-038-B 改动区文本零重叠。

### 🔴 主控验收（逐行 Read + 独立复算，未采信汇总表格）

**通过项**：`VK_TO_LABEL` 修正正确且 `CODE_TO_VK` 未被误动 ✅ ｜硬上限兜底真落地
（`MAX_RECORD_SECONDS + 5`，带 warn 日志）✅ ｜flag 翻转日志已补（含按下时距录音开始毫秒数）✅ ｜
`cargo check --all-targets` **0 error**（主控独立跑）✅

**`TRANSLATE_POLL_STOP` 复位配对已核**：全局 static 若不复位，首次 ESC 后将永久为 true、
翻译热键再次全死。实测复位与 spawn **三处严格配对**（`171→176` / `524→526` / `537→539`），
**该风险不成立**。

#### 🔴 打回项：Toggle 模式下把根因 B 原样复活

`hotkey.rs:197` `WM_KEYUP` 分支把 `TRANSLATE_POLL_STOP.store(true)` 放在**无条件**位置，
而紧随其后的 `send Stop` 却有 `if mode == 1` 门控。**`mode == 0` 是 Toggle。**

> Toggle 语义是「按一下开始 → **松手** → 说话 → 再按一下结束」，**松手 ≠ 录音结束**。
> 故 Toggle 下用户一松开主热键 poll 线程即死，之后按翻译键永不生效。
> **且比改之前更糟**：旧代码固定轮询 500ms、不看 keyup；改后变成松手即死，通常远不到 500ms。
> 触犯最高原则「修改不可引入新问题」。**修法**：`store(true)` 移进 `if mode == 1` 块内。
> Gavin 本人当前为 `PushToTalk`（debug.log `sync_binding` 实证），不受影响；Toggle 用户会中招。

#### 📌 只报不改（既有死逻辑，非 039 引入，本单明令不修）

`hotkey.rs:185` 的 `else if mode == 0`（Toggle 第二次按下结束录音）分支在 **hook 路径下不可达**
—— `:205` 的 `PTT_ACTIVE.store(false)` **无条件执行**，第二次按下时 `!PTT_ACTIVE` 成立，
会重新走 start 分支而非进入该 else。

> hook 路径只用于 `RegisterHotKey` 不支持的键（`requires_polling`：`0xA0..=0xA5` 裸修饰键）。
> Gavin 的 vk=165（VK_RMENU）正走此路径，但他是 PTT 模式故不触发。
> **待开单**：Toggle + 裸修饰键组合下的行为需实测确认。

#### 硬上限兜底的实证价值（保留供后续参考）

主控要求的硬上限在验收中被证明**不是冗余**：Toggle 经 `RegisterHotKey` 路径（`:523`）时，
**第二次按下会重置 `STOP=false` 并 spawn 一个新 poll 线程**，而那次按下本意是结束录音 ——
该线程无任何正常终止路径，**全靠硬上限兜住**。

> **补调用点是穷举，穷举必漏（coder-2 第一版就漏了 ESC）；硬上限是兜底，漏了也不出事。**
> 两者是纵深防御，不是二选一。

---

## 🔄 当前批次 · v0.8.0 在线 ASR 引擎更替 + 流式上屏（2026-08-14 起）

> 🔴 **2026-08-15 主控补记**：本批次自 08-14 开工，`handoffs.md` 与 `logs/20260814.md` 有完整记录，
> 但 **todo.md / progress.md 长期零条目**（`grep "ASR-038"` 命中 0 次）——
> `[DOC-STATE-DRIFT-001]` 又一次复现，且这次漂的是**主控自己职责内的两份文档**。已补齐。

### 🔴 断点：当前代码库不可编译（主控 08-15 `cargo check --all-targets` 实跑取证）

```
src\transcription\mod.rs:301:52: error[E0425]:
  cannot find function `load_wordbook_vocabulary` in module `crate::transcription`
error: could not compile `voice-ime` (bin "feiyin-ime") due to 1 previous error; 10 warnings
error: could not compile `voice-ime` (bin "feiyin-ime" test) due to 1 previous error; 14 warnings
```

**唯一 1 个 error**。14 个 warning 全为既有 unused variable（`itn.rs:2013` / `punctuation/mod.rs:284` /
`main.rs:4305,4559`），非本批引入、非阻塞。

**根因**：`mod tests` 在 `src/transcription/mod.rs:838` 开、`:1570` 闭（一直到文件末尾），
`pub fn load_wordbook_vocabulary()`（`:1314–1354` 含文档注释）**被插在 `mod tests` 内部**。
虽写在第 0 列看着像顶层，词法上仍属测试模块 → 正式构建不可见。
**修法：移到 `:837` 的 `#[cfg(test)]` 之前。一处改动。**

> ⚠️ 上次会话主控靠**缩进目测**判断该函数在顶层 → 判断错误。
> **模块归属以编译器 `help` 输出为准，不要靠缩进目测。**

### 任务分解与状态

| 编号 | 内容 | 文件域 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| RESEARCH-ASR-035 | qwen-audio-3.0 接入研究 | 无（纯研究） | coder-1 | ✅ 闭环（§4 前提已被 038 取代） |
| RESEARCH-TSF-036 | Windows TSF 组合文本可行性 | 无（纯研究） | coder-1 | ✅ 闭环 → **DEC-050 判死** |
| DESIGN-OVERLAY-037 | overlay 流式预览交互设计 | 无（纯设计） | coder-2 | ✅ 闭环 |
| RESEARCH-ASR-038 | 流式管线 + VAD 门控 + 热词链路 | 无（纯设计） | coder-1 | ✅ 闭环 |
| **ASR-038-A** | `qwen_inference.rs` 新建（1076 行 + 45 单测） | `src/transcription/` 新文件 | coder-1 | ✅ **主控独立复现验收通过** |
| **ASR-038-B** | VAD 入口门控 + 管线改造（边录边发 + 增量接收） | `src/audio/`、`src/main.rs`、`src/transcription/`、`qwen_inference.rs`、`src/ui/overlay.rs`（🔴 仅数据字段） | coder-1 | 🟡 **真流式核心已实施**（C-1~C-4+040-A，编译通/测试全绿），待主控终验 |
| ASR-038-C | overlay 流式显示 + 编辑态 + EDIT 控件 | `src/main.rs`、`src/ui/overlay.rs`（绘制与交互） | coder-2 | 🔜 **等 B**（同动 `main.rs`，零并行空间） |
| TEST-SYNC-038 | 阶段三测试同步 | 各 `mod tests` | tester-1 | 🔜 等 B 验收 |
| TEST-EXEC-038 | 阶段四全量回归 | — | tester-1 | 🔜 |
| BUILD-016 | 阶段五 v0.8.0 首包 | — | tester-1 | 🔜 |

### 038-B 任务书要点（正文备份，防 inbox 跨 session 丢失）

**禁止**：`src-tauri/`、`ui/`；**禁止**实现 overlay 渲染 / EDIT 控件 / 提交按钮 / 编辑态交互（属 038-C，coder-2 做）。

**两条硬性自证**：

| # | 项 | 状态 |
| --- | --- | --- |
| ① | 词库调用链只连 `wordbook` 表 | ✅ **主控已代验通过**（`list_all()` → `load_word_entries()` → `SELECT … FROM wordbook`，不碰 `wordbook_candidates`） |
| ② | VAD 门控最坏情况**不吞字** | ⏸ **待 Worker 自证**：pre-roll 从热键按下**之前**就在缓冲，须证「按下瞬间即开口」不丢音频；**VAD 模型缺失时降级为总是建连**（宁可多花钱不可吞字） |

**其余要点**：`record_streaming()` 与现有 `record()` 并存、不改其行为 ｜ `speech_detected`（RMS）管停录、
Silero VAD 管建连，**并存不替换、行为零变更** ｜ 增量文本**推送不设限**，节流靠消费端 16ms timer +
100ms 尺寸节流两道 ｜ 推送 `StreamingAsrState::display_text()` **全量文本**，消费端整体替换 ｜
收 `EditRequested` → 关 WS / 丢弃 state / 置 `pipeline_cancelled` ｜ 松键后整段交现有 LLM 管线
（**F3 跨句能力必须保留**）｜ 复核 `select_preprocessing_params` 在流式下是否适用（A 批遗留：
`silence_head`/`onset_backtrack` 是批处理概念，流式下是否适用存疑）。

**验收**：`cargo fmt` clean ｜ `cargo check --all-targets` 0 error ｜ `cargo test` **全量回归**
（动了既有文件，不能只跑局部）｜ 文件域未越界。

### 待端测顺手确认的低成本项（无需单独开单）

**计费口径**：`usage.duration` 是「上传音频时长」还是「墙钟会话时长」，官方文档未覆盖，
038 已按保守假设（墙钟）设计。`-debug` 跑一次录音对比实际录音时长即可。
若为「上传音频时长」，VAD 第②层会话内静音门控可再省一截。

---

## 🛑 2026-08-09 00:30 会话中断交接 —— 下次会话从这里开始

> **中断原因**：tester-1 额度用尽（Gavin 指令：记下状态，下次会话重新派发）。

### 断点状态（主控已取证，非推测）

| 项 | 状态 |
| --- | --- |
| 工作区 | ✅ **干净**（`git status -- src/ docs/ CHANGELOG.md` 为空），无悬空改动 |
| HEAD | `d2ee6b3`，本批四个 commit 全部落地，**本地 ahead 未 push**（push 需 Gavin 明确指示） |
| tester-1 产出 | ❌ **零**（`outbox/tester-1/result.md` 0 字节）—— TEST-EXEC-030 未开跑或无产出 |
| coder-1 / coder-2 | 空闲，本批任务全部结案 |

### 本批四个 commit

| commit | 内容 |
| --- | --- |
| `94bfb0b` | 030-A/A-2/B/B-2/C + 031（功能主体） |
| `5fc390d` | TEST-SYNC-030（+17 用例） |
| `201bb4f` | 030-D/E（抽两个纯函数，纯重构零行为变更） |
| `d2ee6b3` | TEST-SYNC-030-B（+19 用例） |

### ✅ 2026-08-09 21:13 BUILD-015 已出包 —— ⏭ **待 Gavin 端测**

**030 全批 + 031 首次进 exe。** `Publish/feiyin-ime.exe` `831c254d…` / `feiyin-ime-ui.exe` `14411dee…`
/ `crash-reporter.exe` `9fa58f9b…`，ProductVersion 0.7.3.0/0.7.3，版本号未动。
主控七项独立复算全过（时间戳/两副本 sha256/三副本 toml/版本号/UI 嵌入/冒烟/大小）。

**🔴 出包时拦截到一个静默失效**：`scene-rules.toml` 的 `Publish/` 与 `target/release/` 两副本
停留在 08-03 旧版（41714B），根目录已是 45591B，**差 3877B 实质内容**。来路是 macOS 端 `f96c817`
经 merge `7e76465` 带入、本端从未编辑过该文件 → 无任何「该同步」的触发点。窗口内恰好没出包，
**未流到 Gavin 手上**，但不查就会随本次包发出。已收敛三副本。

**已落地的两项流程修复**：① `build-test-guide.md` Step 4 补入 toml 同步 + 三副本 sha256 验证
+「不得同步」清单；② `[TOML-STALE-001]` 新增第 3 条：跨端 merge 后首次出包必须核对两 toml 三副本。
**顺带修正**：guide 的产物大小基准（~31MB/~22MB）是 DEC-021 体积优化前的旧值，已改实测值。

---

## ✅ 已结案 · 无序枚举不出列表（033 + 034 两轮共 42 次调用，DEC-049 拍板不修）

**机制已定位到代码侧**：F3 两道闸（`src/llm/mod.rs:1219-1233`）之间存在定义空隙 ——
`有没有新鲜的青椒` 这类**无主语疑问谓词短语**，既不符合 SHORT（名词短语、无谓语、≤6 字），
也不符合 LONG（带谓语或内部标点的完整子句），落进空隙后命中闸一保守兜底
「If unsure, DO NOT use a list」→ 保持段落。**模型全程执行正确。**

对照实测（三类形态 → 三种确定行为，完全自洽）：`新鲜的青椒` → 顿号内联 3/3 ｜
`有些同学下课之后徘徊在校园里不走，三五成群` → bullet 列表 3/3 ｜ `有没有新鲜的青椒` → 两者皆无 9/9。

**同时排除**：① 不是 030 的 `ADD_PUNCT` 改动（G1/G2 唯一差异即该句，IN-B 两组均 3/3、
IN-C 新版反而更好、IN-D 两组均 0/3，无「旧能出新不能」模式）；② 不是 macOS 端场景配置变更。

**Gavin 拍板（DEC-049）**：保持现状，吃不准的不走列表化，避免规则过分精细化造成易失误和交叉误判。
**F3 规则不改动，D5 确认实验取消，`[F3-UNORDERED-LIST-001]` 关闭。零生产代码改动。**

**两个独立遗留**（不随本条关闭，待 Gavin 定）：
① **模型漂移** —— 033 G0 同提示词同模型同采样参数，08-03 IN-B 3/3 → 08-14 **0/3**。
   不可控、不预告；若要把「无序枚举出列表」当可依赖能力，需回归看门测试。
② **场景块是正向变量** —— 同日同模型，无场景块 IN-B 0/3、含场景块 **3/3**，机制未明，
   是目前唯一实测有效的正向变量。

---

## 🔄 2026-08-08 当前批次（Gavin 端测 + 需求）

### ✅ 已闭环 · LLM-CONN-POOL-028 连接池僵尸连接致 0ms 请求失败

提交 `480a23c`。根因：`reqwest` 默认 `pool_idle_timeout=90s` ＞ DeepSeek 服务端 keep-alive ~60s，
60–90s 空闲窗口内池中残留死连接，取出即 0ms 失败。判据是空闲间隔：<60s 全成功、
62.5/67.9/72.3/72.8s 四次全失败、>98s（含 4244s）全成功。
修复三层：`POOL_IDLE_TIMEOUT=30s` / 重试判据补 `is_request()` / `fmt_error_chain` 展开 source chain。
**主控无法在验收阶段自证** —— 依赖「服务端 keep-alive ≈60s」的反推，真正判据是端测后 0ms 失败是否绝迹。

### 🔄 PUNCT-GOVERNANCE-030 标点子系统治理（架构稿 `research/punctuation-architecture-001.md`）

> **起因**：Gavin 提「开启自动补标点时字/词数 ≤5 不加末尾标点」，主控给了补丁式方案被 Gavin 打回：
> 「每一个功能都要从架构程度全局层面来设计方案，不能像补丁式的这样来设计方案」。
> 盘点后发现**标点有 6 个产出源，开关只完全控制 1 个**。教训见 `.claude/tasks/lessons.md` 2026-08-08 条。

架构：**L1 源头控制为主，L2 后处理补位**（Gavin 指示：能在源头关的就别产出后再剥）。
源头可控性已核实：在线 Qwen3 ASR（官方文档无标点参数）/ 本地 Accuracy native（模型内建）/
本地 NLLB（seq2seq 无参数）**三条物理上不可控**，L2 不可省。

| 编号 | 内容 | 负责人 | 文件域 | 状态 |
| --- | --- | --- | --- | --- |
| 030-A | L2 后处理：`count_units` + `strip_trailing_punctuation` + `strip_punctuation` 改造（标点位置留空格）+ 收口点删除来源判据 | coder-2 | `main.rs` `transcription/` `punctuation/` | 🟡 **代码在工作区，未验证未提交** |
| 030-B | L1 源头：LLM 提示词双向明确化（true 明确补足+优化，false 明确禁止，**不留空**） | coder-1 | `llm/mod.rs` | 🟡 **代码在工作区，未验证未提交** |
| 030-A-2 | ① `strip_trailing_punctuation` 剥成对符号右半（`（笑）`→`（笑`）→ 拆 `TRAILING_PUNCT_CHARS`；② `native_punctuated` 硬编码 `true` → `has_effective_punctuation` 实测（见下方 DEC 待补） | coder-2 | `punctuation/` `transcription/` | ✅ 已验证（c 方案，验收清单全绿，待 TEST-SYNC-030） |
| 030-B-2 | 修 030-B 引入的「文本守恒」契约红（`NO_PUNCT` 不在 `whitelist_new`，`assert_text_conservation(*,false)` 两条必 FAIL） | coder-1 | `llm/mod.rs` | ✅ 主控复验通过 |
| 030-C | L3 列表分隔符 `、；` 条件化 —— 新增 `INLINE_SEPARATOR_RULES_NO_PUNCT`，`f3_rules_text` 加 `punctuation_enabled` 参数 | coder-1 | `llm/mod.rs` | ✅ 主控复验通过（打回 1 轮） |
| TEST-SYNC-030 | 测试同步 3.1~3.5 + 分隔符切换护栏（修正版）；缺口 1/2/3 改交「用例规格表」不写代码 | tester-1 | 各 `mod tests` | 🔄 阶段三进行中 |
| 030-D | 抽 `build_translate_system_content`（`optimize_and_translate` 内联且 async+HTTP，不可单测） | coder-1 | `llm/mod.rs` | 🔜 等 TEST-SYNC-030 交规格表 |
| 030-E | 抽 `apply_l2_postprocess`（L2 收口块内联在 `run_pipeline_core`，不可单测） | coder-2 | `punctuation/` `main.rs` | ✅ 已验证（日志方案 C，8 条矩阵实测，待 TEST-SYNC-030-B） |
| TEST-SYNC-030-B | 按 tester-1 自己的规格表把缺口 1/2/3 落成真测试 | tester-1 | | 🔜 等 030-D/E |
| TEST-EXEC-030 | 全量回归 | tester-1 | | 🔜 阶段四 |
| BUILD-015 | 出包（030 全批 + 031 一次端测） | tester-1 | | 🔜 阶段五 |

> **🔴 tester-1 2026-08-08 提的两条异议，主控核实后全部采纳（主控方案有错）**：
> ① 原护栏「punct=false 时 `f3_rules_text` 完整输出不得含 `、`/`；`」**物理上不可能通过** ——
> `INLINE_SEPARATOR_RULES_NO_PUNCT` 正文字面就含这两个字符（禁令句 `do NOT use "、" or "；"`
> + CROSS-LANGUAGE 句），加上日文示例输入侧的 `、`。改为**常量级 + 片段级双断言**：
> 断言整常量在场/不在场（锁整体切换）+ 已知 `、` 连接输出片段黑名单（锁具体示例），不碰输入侧 `，`。
> ② 缺口 1/2/3 的目标逻辑确实内联在 `async fn optimize_and_translate`（紧接发 HTTP）与
> `run_pipeline_core` 中，**不可单测**，须抽纯函数。但 tester-1 提的 `#[ignore]` 骨架方案不可行 ——
> `#[ignore]` 的测试仍要编译，引用尚不存在的函数会让整个 crate 编译失败。
> 故改为三段串行：规格表 → 抽函数（030-D/E）→ TEST-SYNC-030-B，保证同一文件同一时刻只有一个写者。

> 🔴 **2026-08-08 18:5x 新 session 主控 `git diff` 取证**：上一 session 中断，030-A/030-B 代码**已落在工作区但从未收口** ——
> `src/punctuation/mod.rs` +145（`PUNCT_CHARS`/`is_punctuation`/`count_units`/`strip_trailing_punctuation` + 10 条单测）、
> `src/transcription/mod.rs` `strip_punctuation` 重写（标点→空格 + 域名点保护，3 条旧断言已改）、
> `src/main.rs` L2 补位块（删来源判据 + ≤5 剥末尾标点）、`src/llm/mod.rs` `NO_PUNCT` 常量 + 槽位条件替换 + 翻译路径 B4/B5。
> **缺口**：无 `cargo check`/`cargo test` 证据、handoffs/CHANGELOG/logs 零条目、未提交。
> 这是 `[DOC-STATE-DRIFT-001]` 第 N 次复现，主控本轮**不采信任何未取证的完成度**，须走：编译验证 → 主控 Read 验收 → TEST-SYNC-030 → TEST-EXEC。
> 另：`src/itn.rs` 也在 `git status` 里，但 `git diff --stat` 为空 = 仅行尾差异，非 031 改动（031 是只测不改）。

### 🔴 030-C 打回教训（主控 2026-08-08 验收查出，值得记）

coder-1 首版只换了**分隔符规则常量**，却没动 `f3_rules_text` 里写死的 **F3c/F3-item 示例**——
开关关闭时同一个 L3 块里一边写「`、` is FORBIDDEN」，一边拿 `、` 连接的示例教模型，
违反 DEC-040「裁决点必须唯一、层内不得存在歧义」。

**这是本批次同一个病的第三次发作**：
① 030-A 只改 `PUNCT_CHARS` 不改 `strip_trailing_punctuation` 的成对符号；
② 030-B 只改常量不改 `whitelist_new` 契约名单（两条断言必红）；
③ 030-C 只改规则不改示例。

> **共性：改了「说什么」，没改「示范什么」/「校验什么」。**
> 派发提示词类任务时，任务书必须显式列出「规则文本 + 示例 + 契约断言」三处同步检查项。

Gavin 拍板口径：阈值 ≤5 ｜ 中日文数字符/英韩文数词、混合相加 ｜ 短句末尾标点全剥含问号感叹号 ｜
开关关闭时任何标点都剥（含引号括号、列表分隔符）且**标点位置留空格** ｜ 翻译两条引擎都算 ｜
≤5 规则只在开关开启时生效、放后处理点。

### 🔄 ITN-PROBE-031 「万一」→「0.1万」类缺陷探测（2026-08-08 Gavin 端测）

**现象**：`万一` 被转成 `0.1万`。「万一」是中文常用词，不该数字化。

**主控代码追踪（待 tester-1 实测确认）**：`parse_cn_number` 中 `万`/`亿` 分支缺
「大单位前必须有数字」的前置守卫。`万一` → `digit=0` 照常结算 → `big_unit_anchor=Some(('万',0))`
+ `big_unit_seen=true` → 末尾「一」命中隐式千位 `+1*1000` → DEC-042 输出 `0.1万`。

对照：`百`/`千` 分支（`:721`/`:729`）同样无守卫，但不设锚点、`result==0` 被拒，**歪打正着没出事**。
`万`/`亿` 是 027-E 引入锚点机制后才把洞暴露出来。

**这是一类不是一个词** —— 疑与 todo 里挂了很久的 `:682` 十分支默认 1（`十分`→`10分`）同族。
> ✅ **2026-08-08 19:1x coder-1 已实现，主控 Read 验收通过（代码层）**：万/亿两分支对称加守卫
> `if !has_digit && section == 0 && result == 0 { return None; }`，位置在 `large_amount_keep_wan_yi`
> 判定之后、`section += digit` 之前。新增 25 条断言（13 条固定词保汉字 + 12 条放行组零回归）。
> ⏳ **未收口**：① coder-1 报的基线 `itn:: 219→221` 与 handoffs 记录的 `212/0` 对不上，已要求说明；
> ② 全量回归归 tester-1（阶段四）；③ 只报不改项：`零万` 类输入 `has_digit=true` 会绕过守卫，
> 仍产零系数锚点，非真实中文表达，优先级低。

**2026-08-08 18:5x 主控 Read 代码复核确认根因成立**（`itn.rs:770/779/784`），Gavin 指示直接修，
不再等 tester-1 五组探测。**已派发 coder-1（Step A）**：万/亿分支入口加守卫
`if !has_digit && section == 0 && result == 0 { return None; }` —— 走机制层，禁止走词表（DEC-038）。
同族词 `亿万`/`百万`/`千万` 一并实测；`:682` 十分支默认 1（`十分`→`10分`）只报不改（避免混改污染零回归判别力）。

---

## 🔴 P0 进行中 · ITN-FIX-BIGNUM-027 大额数字末尾个位被误套隐式千位（2026-08-04 Gavin 端测）

> **性质：第四种失败模式** —— 金额被静默改错，输出流畅自信但数值已错，**用户看不见**。与 017 同类，金额场景下最危险。

### 现象（debug.log 取证）

```
ASR 正确:  总共金额是一千零四十六万八千七百四十一元。
ITN 输出:  总共金额是10469740元。      ← 送进 LLM 时已经错了
正确应为:  10468741
```

`target/release/debug.log:3956/3962`。LLM 未改动该数字，错误 100% 在 ITN。

### 根因（主控算术反推，与实际输出逐位吻合，**待 coder-1 实测确认**）

`src/itn.rs:800-808` 末尾挂起数字处理：

```rust
if big_unit_seen && !zero_since_big {
    section += digit * 1000;   // "两万五"=25000 的隐式千位规则
}
```

| 步 | 状态 |
| --- | --- |
| 「万」结算 | `result = 1046*10000 = 10_460_000`，`big_unit_seen=true` |
| 「八千七百四十」 | `section = 8740`，无零 → `zero_since_big=false` |
| 末尾「一」 | 命中隐式千位 → `section += 1*1000` = **9740** |
| 合计 | **10_469_740** ← 与实际输出一字不差 |

**缺陷本质**：`两万五=25000` 规则本身正确，但守卫只查了「万后有没有零」，**没查万后是否已出现过新的进位单位**。万后走完 `千→百→十` 完整进位链时，末尾单字就是个位，不该套隐式千位。

**影响面**：凡「N万/N亿 + 完整进位链 + 末尾个位数 + 段内无零」全错（`三万五千二百四十一`/`八万七千六百五十三`）。有零的（`三万五千二百零一`）因 `zero_since_big=true` 走 else 分支反而正确 —— **这解释了为何一直没被发现**。

### 任务分解

| 编号 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| 027-A | `unit_since_big` 守卫收窄隐式千位适用范围 | coder-1 | ✅ 提交 `136f70f`（itn:: 153→168） |
| 027-B | 万分支 `result += section*10000`，不再把已结算的亿卷进乘法 | coder-1 | ✅ 同上 |
| 027-C | `large_amount_keep_wan_yi` 加 `big_starts_new_number` 消歧 | coder-1 | ✅ 提交 `967cd8d`（itn:: 168→181） |
| 027-C-2 | `two_is_unit` 补齐注释原意（两后跟进位单位→非单位） | coder-1 | ✅ 同上 |
| 027-D | DEC-042 初版：隐式补全保留锚定单位（**判据已被 E 推翻**） | coder-1 | ✅ 提交 `e79ed05`（itn:: 181→186） |
| 027-E | DEC-042 补完版全面落地：最小明确单位 + 量级分界在万 + 升亿阈值 ≤1 位小数 | coder-1 | ✅ 提交 `499d56e`（itn:: 186→196，12 条行为变更） |
| TEST-SYNC-027 | 43 条复核 + 契约护栏 + 跨模块回归 | tester-1 | ✅ 完成（result.md 18964B），S1-S5 补测待落地 |
| **027-F** | **修 027-E 引入的静默归零**（丙型链隐式尾数吸收旁路） | coder-1 | 🔄 **进行中**（08-04 派发，已 ACK，实测中） |
| S1-S5 补测落地 | tester-1 建议清单，含升亿 2 位边界 + 静默归零钉现状 | tester-1 | 🔜 等 027-F 完成（同文件不并行） |
| TEST-EXEC-027 | 全量回归 | tester-1 | 🔜 阶段四 |
| BUILD-014 | 出包（026 + 027 全批一次端测） | tester-1 | 🔜 阶段五 |

### 🔴 027-F 根因：027-E 契约变化的漏网旁路（tester-1 走查发现，非跑测试发现）

027-E 后 `parse_cn_number` 返回带单位串，`三亿`/`一万` 的 **consumed 恰好等于 2**，穿过丙型链隐式尾数吸收的 `num_consumed <= 2` 门控（`src/itn.rs:1122-1133`，026 遗留），带单位串进 `parts` → `format_currency_chain:1303` 的 `.parse::<f64>().unwrap_or(0.0)` **静默归零**。

| 输入 | 现状 | 丢失 |
| --- | --- | --- |
| 五块一万 | `5元` | 「1万」归零 |
| 五块三亿 | `5元` | 「3亿」归零 |
| 三毛五亿 | `0.3元` | 「5亿」归零 |

**教训**：027-D/E 的孤立判定只挡了**主解析路径**，没挡**函数内部的二次消费**。027-C 那次盘的 14 个调用点是 027-E **之前**的契约口径，不覆盖这条。已要求 coder-1 按新契约**重盘一次**。

**这是 `[ITN-LOCAL-RULE-OVERREACH-001]` 模式的第五个实例** —— 只不过这次是**测试侧走查先抓到**，不是等 Gavin 端测撞出来，说明该条目第三节的防御规则（成对边界断言）开始起作用。


### ⏸ 2026-08-04 全线暂停（Gavin 指令）

> **Gavin 原话**：「先暂停派发任务，因为额度用尽了。要等待额度重置，等我指令」

**Worker 状态**：

| Worker | 模型 | 状态 |
| --- | --- | --- |
| coder-1 | `glm-5.2` Ollama Cloud | 空闲待命（186K/19%） |
| coder-2 | `kimi-k2.7-code` Ollama Cloud | 空闲待命（112K/43%） |
| tester-1 | **`DeepSeek V4 Flash Free (New)` OpenCode Zen** | 空闲待命，**上下文已重建** |

**tester-1 重启记录**：原模型 `ollama-cloud/deepseek-v4-flash` 触发账号 session usage limit（退避拉到 2 分钟，`attempt #7`）。按 Gavin 指令换 `opencode/deepseek-v4-flash-free` 重启。

⚠️ **重启踩坑（值得记）**：`opencode -s <session> -m <model>` 恢复会话时 **`-m` 不生效** —— session 内已存的模型配置优先级更高，底栏仍显示旧模型。**要换模型只能全新启动**（`opencode -m <model>`），代价是丢失会话上下文（本次丢 222.3K/21%，已用注入消息重建）。

### 🔜 额度恢复后的执行顺序

```
① 027-E（coder-1）—— DEC-042 修订版，范围远大于 027-D
② TEST-SYNC-027（tester-1）—— 覆盖 A/B/C/C-2/D/E 全批
③ TEST-EXEC-027（tester-1）
④ BUILD-014 出包 —— 026 + 027 全批，Gavin 一次端测
```

**为什么 TEST-SYNC 必须等 027-E**：027-E 会变更 `一亿`/`十亿`/`两千三百四十五万`/`一千二百三十四亿五千万` 等多条既有断言，现在写测试届时要重写，违反三阶段规则（代码全部完成后才 TEST-SYNC）。

### Gavin 需求「数字要支持从亿到个位数这个量级的跨度」达成情况

| 用例 | 修复前 | 现在 |
| --- | --- | --- |
| 一亿两千三百四十五万六千七百八十九 | **`1`**（整串金额蒸发） | **123456789** ✅ |
| 一亿两千万 | `1` | **120000000** ✅ |
| 一千二百三十四亿五千万 | 差 4 个数量级 | **123450000000** ✅ |
| 一千零四十六万八千七百四十一 | 10469740 | **10468741** ✅ |

**017 零回归**：`一斤二两`=`1斤2两` / `二两半`=`2.5两` / `两斤`=`2斤`，14 条逐条实测（先测现状再写断言）。`is_unit` 本体 `git diff` 自证零改动。

### 🔴 027-D 的真正风险（已写入任务书）

DEC-042 要求 `parse_cn_number` 返回 `"3.5亿"` 这类**带单位字符串**，而其契约原先**恒为纯数字**。026 货币链 / 017 重量链 / 016 班级守卫全部依赖它。**风险不在格式化本身，在下游能否消费带单位的返回值。**

已要求 coder-1 **动手前先做调用点盘点**，并明示：若判断会污染契约、影响面过大，这是有分量的异议，可直接提异议，备选方向是**在更外层格式化、`parse_cn_number` 契约不变**。

### ⏸ 待 Gavin 拍板的边界（027-D 实测后汇总）

- `三亿` 无隐式补全 → 展开 `300000000`，而 `三亿五` 保留 `3.5亿`。**同一开头两种形态**，是否接受？
- `两万五元` / `三亿五元`：保留锚定单位后再跟货币单位，与 026-B 单段货币链如何交互？
- `两万五百` / `三亿五万`：走哪条路（实测确认）

### 📋 同模式规则盘点结果（coder-1 2026-08-04 交付，见 `[ITN-LOCAL-RULE-OVERREACH-001]`）

| # | 位置 | 状态 |
| --- | --- | --- |
| 1 | `:805` 亿级隐式单位注释-实现不符 | ✅ 已升级为 DEC-042 → 027-D |
| 2 | `:699/716` `large_amount_keep_wan_yi` | ✅ 027-C 已修 |
| 3 | `:682` 十分支默认 1（`零十万` 类边界未声明） | ⏸ 待实测确认是否真缺陷 |
| 4 | `:900` 丙型链 `result` 被丢弃 | ✅ **实测确认不是真缺陷**（coder-1 027-C 附加验证） |

### 🔴 Gavin 两条指令（任务书已写为门禁）

1. 「要分析此类问题的根源，**从机制上解决**，不能再犯类似问题」→ 不接受打补丁
2. 「**要保证不能改坏现有正常规则和代码逻辑**」→ **一票否决项**，优先级高于修好 bug。改动前存基线、改后逐条比对、绿转红即停

### 附带产出（比修 bug 更重要）

已要求 coder-1 盘点 `src/itn.rs` 中**同模式规则**。026 的 `after_is_boundary` 耦合、016 的逐位串判据、本次的隐式千位**同属一个病**：

> **局部特例规则没有约束自己的适用范围，在更长的上下文里越界生效。**

三次都是 Gavin 端测撞出来的 → 说明还有没撞到的。盘点结果出来后主控逐个开单补边界守卫。

### 🔍 顺带查出、已明令「只报不改」

`src/itn.rs:803` 注释写「三亿五 = 350000000（五→五千万）」，但代码乘的是 **1000**（`3亿 + 5*1000 = 300005000` ≠ 350000000）。**注释与实现疑似不符**，要求 coder-1 实测确认后单独报，不在 027 里顺手改（混改会让零回归验证失去判别力）。

---

## ✅ 已闭环 · ITN-FIX-CHAIN-TEAR-026 收口（2026-08-04，崩溃中断遗留）

> **发现方式**：Gavin 2026-08-04 问「上次会话是否有测试或出包任务未完成」，主控独立取证查出。**非 Worker 自报** —— coder-1 写了 CHANGELOG/decisions/troubleshooting/logs，唯独 todo.md 与 handoffs.md 无条目，且改动全部悬空未提交，是 `[DOC-STATE-DRIFT-001]` 的又一次复现。

### 三件套判定（`[SESSION-CRASH-RECOVERY-001]`）

| 项 | 数据 | 判定 |
| --- | --- | --- |
| sha256 两副本 | `db07cefd8d51` target == Publish | ✅ 一致，但那是 BUILD-012（017/018/020/021/023） |
| **mtime 链** | 产物 **17:42:42** ＜ `src/itn.rs` **23:47:25** | 🔴 **产物早于源文件 6 小时 → 026 未进 exe** |

**结论**：与 08-03 那次「做了没记」相反，本次是**代码做了、测试与出包都没做**，必须走完整流程。

### 026 生产改动（主控 Read 取证）

| 编号 | 位置 | 改动 |
| --- | --- | --- |
| A | `try_parse_unit_chain` 无单位分支 | 去 `after_is_boundary` 耦合；隐式尾数单位按前级层级动态决定（块/元→毛，毛/角→分） |
| B | `try_parse_unit_chain` 返回判据 | 允许单段 currency 链（原 `parts.len()>=2`）🔴 **行为大幅放开，主要风险面** |
| C | `format_currency_chain` | 单段保原单位不归一到元（Gavin 方案 C，判据是段数非 `per_unit`）+ 多段去尾零 |

### 任务分解

| 编号 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| ITN-FIX-CHAIN-TEAR-026 / 026-B | 货币链撕裂 + 尾零 + 单段保原单位 | coder-1 | ✅ 已完成，主控 08-04 补提交 `9edc839` |
| TEST-SYNC-026 | T6-T10 复核 + T10 补至 12 词条 + B1-B5 五组歧义反向护栏 + C1-C4 交叉回归 | tester-1 | ✅ 已完成（+114 行，生产零改动） |
| TEST-EXEC-026 | 全量回归 | tester-1 | ✅ **766/0/6** ｜ itn:: 153/0 ｜ llm:: 131/0 ｜ src-tauri 53/0 ｜ --list 772 自洽 |
| BUILD-013 | 出包（026 首次进 exe） | tester-1 | ✅ **08-04 00:48 出包**，主控 7 项独立验收全过 |

**本节已闭环，提交 `f1024e8`。**

### 🔴 TEST-EXEC 查出的一件事（记入 P6 前置，非本批回归）

TEST-EXEC 的 4 条红**全部是 coder-1 在 026 里自写的 T10 断言写错**，不是回归：

| 词条 | coder-1 断言 | 实测 | toml 保护表 |
| --- | --- | --- | --- |
| 三毛钱 / 三块钱 / 三元钱 / 五块钱 | 保持汉字 | **`3毛钱` / `3块钱` / `3元钱` / `5块钱`** | ❌ 不在表 |
| 五毛钱 / 一块钱 | 保持汉字 | 保持汉字 ✅ | ✅ 在表（`itn-rules.toml:440/490`） |

**这正是 Gavin 已实感的 DEC-038 原型**：`五毛钱` 在表保汉字、`三毛钱` 不在表变数字，同一表达因数值不同行为完全相反。

⚠️ **连带结论（P6 派发前必看）**：CHANGELOG 026-B 声称「12 个 DEC-038 词条实测确认与放开单段链前一致」，**该记录已被证伪 4 条** —— 因此**无法确证 026 前后误转面是否扩大**。P6 批次实施前须先做 026 前后行为差分，不得直接采信该条记录。

### 端测观察点（出包后给 Gavin）

`五块一斤`→`5块一斤` ｜ `八角`→`8角` ｜ `二十五块`→`25块` ｜ `五块一`→`5.1元`（不再 `5.10元`）｜ `一块八一斤`→`1.8元一斤` ｜ `三块四毛八一斤`→`3.48元一斤`

---

## 🔴 P0 · 2026-08-02 Gavin 端测：ITN 数值被静默改错 + 提示词架构重构指令

> **性质判定（主控）**：数值错误是**第四种失败模式**，比撕裂更严重 —— 前三种（漏保护 / 误保护 / 撕裂）用户都看得见，这一种**用户看不见**（输出流畅自信但事实已错）。
> 设计稿：`collab/research/prompt-architecture-001.md`（主控 2026-08-02 交付，含 7 条架构缺陷诊断 + 四层契约设计 + Q1-Q4 待拍板）

### Gavin 端测原始 6 条

| # | 输入 | 实际输出 | 应为 |
| --- | --- | --- | --- |
| 1 | 一斤二两 | **1.22斤** | 1.2斤 / 保持汉字 |
| 2 | 一块两毛二**一斤**（🔴 Gavin 2026-08-03 亲口确认真实输入；原报告作「一块两毛二已经」，「已经」是 ASR 把「一斤」听错） | **22.20元** | 1.22元一斤 |
| 3 | 三块四毛八一斤 | **84.40元**（「一斤」被删） | 3.48元/斤 |
| 4 | 一块八毛一斤 | **2.80元**（「一斤」被删） | 1.8元/斤 |
| 5 | 一块八一斤 | **82元**（「一斤」被删） | 1.8元/斤 |
| 6 | 三斤六两五 | **3.625斤** | 3斤6两5钱 |

### 根因（主控代码取证 + 算术反推，**待 Worker 实测确认**）

- **RC-A · `两` 兼任数字与单位**：`src/itn.rs:511` `DIGIT_MAP` 含 `("两",'2')`。「二两」读成 2、2 → `.22`；「六两五」→ `625`。解释 #1 #2 #6
- **RC-B · 余数链不在语义边界终止**：「一块八**一斤**」后续量词短语首字「一」被吸入货币余数链 → `八一`=81 → `1块+81`=82。解释 **#2 #3 #4 #5**（#2：`1 + 0.2 + 「二一」21 = 22.2`，四条同根因）
  - ⚠️ **主控推翻 coder-1 首轮结论**：coder-1 按报告原文测「一块两毛二」得 `1.22元` 正确，汇报「#2 已正确不在范围」。主控从错误值反推算术锁定真实输入含「一斤」，Gavin 确认。**若采信则六条只修五条**，详见 troubleshooting `[BUGREPORT-SELFCORRUPT-001]`
- **RC-C · 提示词把 ITN 错误洗成不可见**：`UNIT_SYMBOL_PROTECTION:29` 用 MUST/never 级措辞禁止 LLM 改数字，而「不得删除语义」（`F3d:887`）强度最低且埋在格式块内 → 面对 `2.80元一斤` 这种不自洽句子，LLM 只能删「一斤」求通顺。**架构把吵闹的错误变成了安静的错误**

### Gavin 2026-08-02 拍板（Q1-Q4）

| # | 决策 | 备注 |
| --- | --- | --- |
| Q1 | ✅ 采纳 L0-3「宁可别扭不可静默改错」 | L0 随 C 批落地 |
| Q2 | ✅ 裁剪默认基座 §2/§4/§5/§7；**Gavin 从未改过提示词 → 不需要迁移逻辑** | 方案简化 |
| Q3 | ✅ **ABC 一次做完**（主控原建议分三批，Gavin 拍板合并） | byte-identical 检查点降级为任务内强制步骤，不减轮次 |
| Q4 | ✅ ITN 与提示词**并行** | Gavin：「前端算错了后面输出肯定违背原意」——两线都得修 |
| Q5 | ✅ **单价限定词用 `元一斤`，不用 `元/斤`**（2026-08-03 Gavin 亲口拍板） | Gavin 原话：「用元一斤把」「比较贴合原输入」。理由：`元一斤` 保留用户口述原始措辞，`元/斤` 是价签书写体、属系统替用户改写表达，与 018 刚落地的 L0-1 FIDELITY 不变式同向。**注意**：Gavin 最初 bug 报告里 #3/#4/#5 写的是 `3.48元/斤`，本条为后续修订，以本条为准 |

### 任务分解（2026-08-02 23:2x 已派发）

| 编号 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| ITN-FIX-CURRENCY-017 | RC-A（`两`消歧）/ RC-B（余数链边界终止）修复 | coder-1 | ✅ **已完成，未提交**（工作区 `src/itn.rs` + `itn-rules.toml`） |
| PROMPT-ARCH-018 | A+C 合并：分层结构 + 元规则句 + L0 四条 + 基座裁剪（i18n **三处**）+ 删两处 OVERRIDE 补丁 | coder-2 | ✅ **已提交 `790e316`**（08-03 01:00），主控 5 项独立验收全过，详见 `logs/20260803.md` |
| TEST-SYNC-019 | 017/018 断言同步 + **B 批契约测试**（T1 Topic 唯一归属 / T2 矛盾对 / T3 层序 / T4 长度预算） | tester-1 | ✅ **已完成**（红条换锚 + 017/018 复核 + T1-T4 新增 4 条） |
| TEST-EXEC | 全量回归验证 | tester-1 | ✅ **749/0/6** + itn::139/0/0 + src-tauri 53/0/0 |
| BUILD-010 | 出包（017+018 一次端测完） | tester-1 | ✅ **已完成**（2026-08-03 13:06，三 exe + 两 toml 三副本一致，ProductVersion 0.7.3.0/0.7.3） |

---

## ✅ 已闭环 · PROMPT-ARCH-020 翻译路径假前提未修（018 只修了一半）

> **状态（主控 2026-08-03 18:3x 复核）**：✅ **已完成并出包**。修复随 `4c8f830`（021+020 合批，coder-2）落地；`TEST-SYNC-022`（tester-1）补齐 5 条翻译路径对称断言；`BUILD-011` 17:07 出包，反向探针 2/2 = 0。本节保留作根因存档，不再是待办。

> **来源**：主控 2026-08-03 BUILD-010 出包验收时用**反向探针**查出（`already contains normalized numbers` 在新 exe 里仍 =1，而 018 声称已修）。**非 Worker 自报，非测试发现** —— 749 条测试全绿也没抓到，因为没有任何断言覆盖翻译路径的这条常量。

### 事实

| 常量 | 位置 | 状态 |
| --- | --- | --- |
| `UNIT_SYMBOL_PROTECTION`（主路径） | `src/llm/mod.rs:32` | ✅ 018 已修：改为 `may have been pre-processed by an automatic number-normalizer that is NOT infallible (see L0-3)`，并追加 `This rule NEVER justifies deleting a unit or measure phrase — see L0-1 and L0-3.` |
| `UNIT_SYMBOL_PROTECTION_TRANSLATE`（**翻译路径**） | `src/llm/mod.rs:33` | ❌ **原样未动**：仍为 `The input text already contains normalized numbers and unit symbols`，且**缺 L0-1/L0-3 对齐句** |

### 为什么这是真缺陷（主控代码取证）

`src/main.rs:2941` `let pre_llm_text = itn::normalize_numbers(&raw_text);` 位于翻译分支判定（`:2984` `translate_requested`）**之前**；`:3021` `optimize_and_translate(...)` 与 `:3038`/`:3049` 的 `try_nllb_translate(&pre_llm_text, ...)` 用的都是这份已 ITN 处理的文本。

**结论**：翻译路径与主路径吃同一份 ITN 输出，假前提同样为假。**Gavin 若用翻译功能说「一块八一斤」，2026-08-02 那个「LLM 删掉『斤』输出 2.80元。」的静默改错会原样复现** —— 而且翻译路径连「本条款绝不授权删除单位/量词短语」这句兜底都没有。

### 修复方案（派发时用）

1. `UNIT_SYMBOL_PROTECTION_TRANSLATE` 开头同样改为「输入可能经过不可靠的自动数字规范化，见 L0-3」
2. 末尾同样追加「本条款绝不授权删除单位或量词短语，见 L0-1/L0-3」
3. **保留翻译路径特有的 `In the <corrected> line` 限定**（两常量的差异是刻意设计，见 `:28` 注释，不得抹平）
4. 确认翻译路径是否也注入 L0 四条 —— 若未注入，则 `see L0-3` 是悬空引用，需一并处理（**这是本项真正的架构问题，优先查清**）

### 测试缺口（一并补）

现有断言只覆盖 `UNIT_SYMBOL_PROTECTION`（`:1918` 有 `assert!(!...contains("already contains normalized numbers"))`），**翻译路径常量无任何等价断言** —— 这正是它逃过 749 条测试的原因。补对称断言。

---

## 🔄 进行中 · 2026-08-02 格式与 ITN 双线批次

| 编号 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| FORMAT-F3-SHORTITEM-014 | 多行分支补「短项内联 / 长句列表」分流 + 四语措辞补充 | coder-2 | ✅ 已提交 `3b4b622` |
| FORMAT-F3-SHORTITEM-015 | F3b 与输出契约对称补 LONG 限定，收口 014 的 recency 冲突 | coder-2 | ✅ 已提交 `ae452fb` |
| ITN-FIX-GRADECLASS-016 | 年级班级简写（一三班/初二三班/高一四班）被逐位串误合并 | coder-1 | ✅ 已提交 `81cf51a` |
| TEST-SYNC-016 | 015 断言换锚（`may be FULL SENTENCES` 过时）+ 016 年级班级 6 条 + 反向护栏 8 条 | tester-1 | ✅ 已交付，**改动未提交**（`src/itn.rs` / `src/llm/mod.rs` 测试块 +265/−3） |
| TEST-EXEC + BUILD | 全量回归 → 出包（含 014/015/016 三项，Gavin 一次端测完） | tester-1 | 🔜 待派发（阶段四） |

> ⚠️ **提交哈希勘误（主控 2026-08-02 22:5x 核实）**：handoffs 记录的 `a03b89c`(015) / `2d7703d`(016) 与实际 `git log` 不符，实际为 `ae452fb` / `81cf51a`（本表已按实际订正）。当前 HEAD = `81cf51a`，工作区尚有 TEST-SYNC-016 的测试改动 + 文档改动未提交。

### 015 的由来（主控验收查出，非 Worker 自报）

014 引入 `F3-item form`（SHORT 内联 / LONG 列表）后**只给 F3a 补了 LONG 限定**，F3b 与末段 Output format 仍无条件要求 bullet/多行，且位置更靠后 —— 即 `src/llm/mod.rs:813-817` 注释自陈的「后段软化前段」失败模式。Gavin 买菜用例（无序 + 短名词短语）恰好命中，若不修则端测照旧被拆 4 行。

### 016 根因（主控 Read 取证，coder-1 已纠正主控机制方案）

- `src/itn.rs:582` 逐位串判据 `serial_len >= 2 && !next_is_unit`：「一三」后跟「班」（非进位单位）→ 合并为 `13`
- `:1831` `consumed >= 2` 直接判转；「班」不在 `[protect.classifiers]`；全或无因「班」非单位在此终止链，拦不住
- **修法走规则层不走词表**（DEC-038）：2 位逐位串后紧跟班级后缀 → `parse_cn_number` **直接 return None**
- ⚠️ **主控原方案「跳过 early return 落进位组合路径」已作废** —— coder-1 指出会因 `digit` 被覆盖产出 (3,2) → 「3班」撕裂
- 🔍 **DEC-038 活样本**：`五一` 在 `[protect.proper_nouns]:159` 而「一三」不在 → 同构表达一对一错，正是不该用加词表方式修的证据

---

## ✅ ITN-V2 全批次闭环并已出包（2026-08-01 12:51 产物，主控 12:55 独立验收）

> **本节取代原「🛑 会话中断交接」节** —— 该次中断（coder-1 额度超限）已于 2026-08-01 完全恢复并收口。

### 一、提交链（`main` ahead 5，**未 push**，Gavin 只授权提交）

```
5799c02  test(itn): TEST-SYNC-ITN-V2-006 补 6 条单测          ← 测试基线
05de1bc  fix(itn): 移除 unit_collisions 中 5 条 2 字遮蔽词条   ← 红1 闭合
b462f83  feat(itn): ENGINE-006 双隶属量词守卫 + 语法族全量扫描
f6700ea  docs(itn): ITN 二代决策记录、研究稿与协作文档同步
6fdba85  feat(itn): ITN 二代重构 P1-P5 + 测试同步
```

### 二、本轮闭合的两条红

| 红 | 内容 | 结果 |
| --- | --- | --- |
| 红2 | `五间半` 类双隶属量词在逐字路径与甲型路径行为不一致 | ✅ `decide_conversion` 判据 `is_unit`→`is_real_unit`。**多位数不受影响**（`consumed>=2` 守卫在其后），已由 T3 四条单测锁死 |
| 红1 | `N分钟` 家族覆盖随机 | ✅ 移除 7 条 + **`二分` 2 字遮蔽词**（真根因，非首轮所说的「保护词条已移除」）。`二分钟`→`2分钟` |

### 三、任务C 语法族全量扫描结论（`collab/research/itn-v2-grammar-family-scan-006.md`）

反向分组算法：1651 条保护词 → 1324 个尾串家族 → **130 个随机子集**（首轮从单位表正扫只找到 7 个，形状错）。逐族分类：**🔴 能产语法族 75 / ⚪ 专名固定表达 55**。

**本批只处置了 5 条 2 字遮蔽词**（`三元`/`九度`/`二分`/`五类`/`四大`），Gavin 2026-08-01 拍板。**75 个能产族 → P6 批次待排期**（见下）。

### 四、出包产物（BUILD-RELEASE-20260801-001）

| 产物 | sha256(前12) | 大小 | 说明 |
| --- | --- | --- | --- |
| feiyin-ime.exe | `8092cf385d51` | 11,878,912 B（+80,896） | **首次含完整 P1-P5 + ENGINE-006**（P1-P5 此前从未进过 exe，故增量主要来自二代文法引擎） |
| crash-reporter.exe | `b02ca32ca376` | 24,858,624 B | |
| feiyin-ime-ui.exe | `16acff200bb3` | 10,026,496 B | **未重建**（`ui/`+`src-tauri/` 自 `0adb819` 零改动，主控 diff 取证） |

**主控独立验收 6 项全过**（未采信汇报表格）：

1. **决定性探针（反向设计）** —— 本批无新增代码字符串，改用「被删的词应消失」：`一分钟`/`五分钟`/`八分钟` 旧 exe 均 =1 → **新 exe 两副本均 =0**；对照探针 `一刻钟`=1、`二分查找`=3（保留词条）证明 grep 方法有效
2. **两副本 sha256 逐一一致**（三个 exe 全部 target/release == Publish）
3. **`itn-rules.toml` 三副本一致** `93ab3972`（**本次最关键** —— 出包前根目录与另两副本故意不同步，漏同步则外置旧 toml 静默赢过新内置默认，`[TOML-STALE-001]`）；`scene-rules.toml` 三副本仍 `7b01b33c`
4. **ProductVersion 0.7.3.0**，三处版本号文件均 `0.7.3`，红线遵守
5. **mtime 链**：产物 12:51:47 / 12:52:07 晚于 `src/itn.rs` 12:34:03 与 `itn-rules.toml` 12:18:38（`[BUILD-002]`）
6. **冒烟实例 PID 20000** Responding=True，零 panic，`git status` 源文件零改动

**一处如实标注的缺口**：`ITN rules loaded from` 日志行因 ITN 懒加载**尚未出现**（需 Gavin 首次语音后才写入）。toml 三副本 sha256 一致是更强的保证，不阻塞。

### 五、测试基线

`cargo test` **767 / 0 / 8**（`--list` 775 = 767+8 自洽）｜ `itn::` **124 / 0**（主控 `--list` 独立计数一致）｜ src-tauri **53 / 0 / 0**。零红条。

### 六、⏭ 待 Gavin 端测

**端测观察点**：

| 类别 | 观察项 |
| --- | --- |
| 本轮修复 | `二分钟`→`2分钟`、`三元钱`→`3元钱`、`九度电`→`9度电`（07-30 预测的三条casualty 已全部恢复） |
| 双隶属量词 | `三条`/`五台`/`两辆`/`三次` 保持汉字；`三十五台`→`35台`（多位数仍转） |
| **P1-P5 首次上线** | `四点半`→`4:30`、`五点三刻`→`5:45`、`十一块九毛二`→`11.92元`、`五块八`→`5.8元`、`一米二`→`1.2米`、`一个半小时`→`1.5小时`、`三年二班` 整段不转（全或无） |
| 仍随机的 75 族 | `五毛钱` 保汉字 / `三毛钱`→`3毛钱`、`三点钟` 保汉字 / `四点钟`→`4点钟` 等 —— **P6 未做，遇到不算新 bug** |

### 七、Worker 状态

| Worker | 状态 |
| --- | --- |
| coder-1 | ✅ 空闲待命（ENGINE-006-B / R2 / LEXICON-006-C 三轮均已验收） |
| coder-2 | ✅ 空闲待命 |
| tester-1 | ✅ 空闲待命（TEST-SYNC / TEST-EXEC / BUILD 三轮均已验收） |

---

## ✅ RESEARCH-ITN-V2-001 ITN 二代设计研究（Gavin 2026-07-31 四项需求）

> 派发方式：**双轨并行研究**——coder-1（`collab/research/itn-v2-design-001.md`）+ 主控独立稿（`collab/research/itn-v2-orchestrator-001.md` ✅ 已完成），交付后合并取优交 Gavin 拍板
> 阶段：研究，**零代码改动**。tester-1 不介入（依三阶段规则，TEST-SYNC 待代码任务产生后才派发）

| 编号 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| R1 | ITN 调用位置回移到 LLM 之前（**DEC-035 反转**）。核心矛盾：回移会复发 ℃ 缺陷。主控方案=双通道拆分（主通道 `normalize_with_rules`+符号在 LLM 前 / 补丁通道仅 `normalize_unit_symbols` 在 LLM 后）+ 新增 **F0 事实保全硬约束** | coder-1 + 主控 | 🔄 研究中 |
| R2 | 转换不彻底三缺陷：**A 撕裂**（`十一块九毛二`→`十一块9毛2`，根因=`itn-rules.toml:124` 的 `"十一"` 遮蔽前半段，主控已闭合根因链）／**B** `半`/`刻` 只识别不转换（`src/itn.rs:573-597` 两处均 `break`）／**C** 余数后缀文法族总体设计 | coder-1 + 主控 | 🔄 研究中 |
| R3 | 含数字地名白名单联网扩充（十三陵类）。**强制排在 R2-A 之后**，否则放大撕裂 | coder-1 | 🔄 研究中 |
| R4 | 格式化输出列表智能（有序/无序）。主控实读发现：**有序列表已实现**（`src/llm/mod.rs:814-819`），**无序列表完全缺失**；且 `multiline_safe=false` 场景（微信/浏览器/IDE/Unknown）**列表根本不生效** | coder-1 + 主控 | ✅ 研究完成 |

### ✅ 研究阶段已闭环（2026-07-31）→ 合并终稿 `collab/research/itn-v2-merged-final.md`

**coder-1 交付**（`itn-v2-design-001.md` 24795 B）已验收：主控 6 条取证逐条复核确认、`git status` 零源文件改动、`result.md` 2286 B 非空（`[COLLAB-WRITE-001]` 未复发）。

**主控独立取证查出三项**（未采信汇报表格）：

1. ✅ **采纳 coder-1 对主控的纠正**：`UNIT_SYMBOL_PROTECTION` 指令已存在（`src/llm/mod.rs:29`，无条件注入 `:560`），主控「新增 F0」提法收回，改为「强化已有指令」。**但主控补一条两人都没说透的**：该指令正文「input text **already contains normalized numbers**」在 DEC-035 反转顺序后**前提为假**（LLM 拿到的是汉字数字），它带 `ITN-CELSIUS-002-PROMPT` 标签写于 ITN 还在 LLM 前的年代，反转时未同步修订 → **这是支持 Gavin 回移决定的独立论据**（回移恢复该指令前提）。
2. ✅ **采纳 coder-1 路径异议**（兜底 ②→①），且主控在独立稿中已自行收敛到同一结论；**同时推翻主控派发时给的错误反例**——`十一月`→`11月` 是正确 ITN 输出，不是「误撤销」，coder-1 据此提的追加白名单要求取消。
3. 🔴 **主控查出 coder-1 一处事实错误 + 一项两人共同的错误假设**：
   - `两` 在 **Rust `src/itn.rs:397` `DIGIT_MAP`**，不在 toml（coder-1 §2.4 记为 toml）→ 影响：改数字表需重新出包，非纯数据热更
   - **保护词表对同一语法族的覆盖是随机的**：`X点半` 中 **一/六/八/九点半在保护表内，二/三/四/五/七/十点半不在** → 用户看到 `八点半`（全汉字）与 `四点半`（`4点半` 撕裂）行为完全相反；`一吨半` 在表内而 `两吨半` 不在，同理。根因是 1386 条机器词频派生词表把**规则性语法族切成随机子集**。**直接结论：修复方向不是补词表，而是把 `N点半`/`N<单位>半` 族整体移出保护表交甲型文法处理**——此项在 coder-1 方案中缺失，为合并新增的关键实施约束。

**实施批次建议**：P1（R1 双通道+指令强化 ｜ R4 列表）可立即并行启动，文件级零重叠；P2（缺陷A 修复）→ P3（甲型文法 + **成对移除保护词条**）→ P4（乙/丙型+单位层级表）→ P5（R3 地名扩表，≥3 字）。

**Gavin 已拍板（2026-07-31）**：G1 = **分治·货币也规范化**（`11.92元`/`5.8元`）→ DEC-037 ｜ G2 = **仅 D 扩大 doc 识别面**（未选 C，`multiline_safe=false` 保持现有内联行为）｜ G3 = **全量 P1-P5，一次性出包** ｜ 追加：**全或无**（`三年二班` 整段不转）→ DEC-037 附则

---

### 📋 TEST-SYNC 待办清单（跨 P1-P4 累积，派发 tester-1 时一次性交付）

> **派发时机**：P5 完成、全部代码收口后（三阶段规则，禁止与代码任务并行）
> **本清单持续累积，勿在派发前删减**

#### A · 既有断言需更新（不是回归，是断言过时）

| 项 | 现状 | 应改为 | 来源 |
| --- | --- | --- | --- |
| `time_half` | 断言 `八点半`→`8点半` | **`8:30`** | P3 移除保护词条 + 甲型文法。该断言本身是随机词表覆盖的产物（DEC-038） |
| `geometric_order_hazard_documented`（`src/itn.rs:1877`）注释 | 写「当前 `itn-rules.toml` **无互相为前缀的条目冲突**」 | **事实错误，实测 4 组**（五一⊂五一广场、十一⊂{十一国庆,十一月,十一边形}）。该「隐患护栏」测试断言的前提本身不成立，需重写为真实断言 | P2 主控取证 |

#### B · 需复核的 Worker 自行新增测试

| 项 | 说明 |
| --- | --- |
| coder-2 在 `src/llm/mod.rs` 新增的 5 条 `flatten_multiline` 单测 | 边界偏差（测试归 tester-1），但**主控裁定保留不回滚**——任务书原文只禁「改既有断言」未禁「新增」，属主控规格漏洞。请 tester-1 复核覆盖面是否充分 |

#### C · 新增功能需补覆盖

| 批次 | 需覆盖内容 |
| --- | --- |
| P1 | ITN 双通道（主通道 LLM 前 / 补丁通道 LLM 后）三路径覆盖；`UNIT_SYMBOL_PROTECTION` 事实保全条款 |
| P1 | F3 列表**四象限**：有序×多行 / 有序×单行 / 无序×多行(`• `) / 无序×单行(`、`「；」) |
| P2 | 撕裂修复（`十一块九毛二`）；确定性最长匹配（`十一月` 多次运行恒定）；`flatten` 分隔符守卫 |
| P3 | 甲型 9 实例 + 反例 4 条（`一刻钟`/`三点五`/`半小时`/`半个小时`） |
| P3 | **⚠️ 范围扩张需专项覆盖**：新增 `[units.time]`（小时/分钟）导致 **`三小时`→`3小时`、`五分钟`→`5分钟`**，此为未经请求的行为变化，须确认无副作用 |
| P4 | 乙/丙型；单位层级表；**`分` 族属消歧 3 用例**；**全或无**（`三年二班` 等 5 用例 + 连续性边界） |
| P5 | 地名白名单 ≥3 字条目；反向护栏（新增词不得让 `<前缀>+单位` 正常表达失效） |

#### D · 已知预期红 / 已知遗留

| 项 | 状态 |
| --- | --- |
| `三年二班` 类撕裂 | Gavin 2026-07-31 拍板「全或无」，P4 处理中 |
| ③ `try_parse_composite_block` | P4 已裁定**删除**（职责被丙型 + 全或无覆盖），相关测试需一并清理 |

**⚠️ 主控自我修正（需回写 troubleshooting `[ITN-PREFIX-SHADOW-001]`）**：2026-07-30 写入的「误保护 = 优雅降级」结论**仅在语义单元内不含其他可转数字时成立**。`十一块九毛二` 是反例——`check_protection` 命中后只前移游标不锁定后续（`src/itn.rs:697-703`），后半段照转 → 产出一半汉字一半数字的**撕裂**。撕裂是第三种失败模式，比误保护严重。

**待 Gavin 拍板三项（研究交付时一并呈报）**：① R2-C 货币目标形态（`11块9毛2` 保口语 vs `11.92元` 规范化）② R4 列表场景覆盖策略（维持现状 / 放开聊天类【危险】/ 单行降级保留序号 / 扩大 doc 识别面）③ R1 是否同时实施 F0 事实保全硬约束
## 🍎 [macOS 侧] 2026-07-31 · 接收 v0.7.3 并启动编译基线取证

> 本节由 **macOS 侧主控**追加（遵守 `docs/MACOS-HANDOFF.md` §2.7 共编约定：只追加己方条目，不改写对侧段落）。
> 已 pull `a38a315` → `0adb819`（5 提交，21 文件 +1804/−89），版本号三处实测同步为 **0.7.3**。

### ✅ 已闭环

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| MACOS-CARGOCHECK-BASELINE-002 | **纯取证零改动**：`source scripts/env-macos.sh` 后跑三处 `cargo check`（主程序 `--all-targets` / `src-tauri` / crash-reporter bin）+ 前端只读构建，产出 v0.7.3 在 macOS 上的完整错误清单 | — | tester-1 | ✅ **已验收**（2026-07-31，含主控一处计数修正） |

**🎉 核心结论：v0.7.3 在 macOS 上编译零错误——主程序 `--all-targets` 0 / `src-tauri` 0 / crash-reporter 0 / 前端 build 成功。Windows 侧本批 5 个提交未引入任何跨平台破坏，无 (a) 类平台中立模块错误，无 (b) 类 cfg 门控漂移。**

**主控独立取证（未采信汇报表格，详见 `logs/20260731.md`）**：

- **主控亲自复跑 `cargo check --all-targets` → 0 errors**（`Finished dev profile in 8.32s`，exit=0），**亲自复跑 `src-tauri` → 0 errors / 13 warnings** 与汇报逐字吻合。这是本次验收的决定性证据
- **`AppConfig` 零字段改动经字段级双向 grep 核实**：新增 `pub xxx:` 字段 **0**、删除 **0**；+54 行的实体是平台中立方法 `remember_translation_direction()` + 2 条单测。**DEC-033 附则三点名的最高风险面本批次未被触碰**
- **既有 E0432 消失的归因正确**：主控 `git show 6f0b51e -- src/main.rs` 核实，该提交给 `use super::select_preprocessing_params` 及其下 5 个 `#[test]` 各加了 `#[cfg(target_os = "windows")]`，且刻意未把同 use 语句里的 `transcription` 一起 cfg 掉（避免误伤其他测试）
- **诚信记录（正面）**：tester-1 **未顺着任务书预设走**。主控在任务书里写的是「既有 E0432 会换行号出现，属既有」，它实测后给出「该错误已整体消失」并追查到 `6f0b51e` 这个正确出处，而非套用主控框架交差
- **边界合规**：`git status --porcelain` 仅主控自己的 `M collab/todo.md` + `?? logs/20260731.md`；源文件、版本号、`ui/package-lock.json` 零改动
- **❌ 主控修正一处**：汇报称 `src/main.rs:2962` 有乱码注释残留，属实但**计数不全，实际 5 处**（`2483 / 2673 / 2689 / 2722 / 2962`）。已据此更新下方 MOJIBAKE-COMMENT-001

**⚠️ 验证陷阱（供后续验收避坑）**：存在**两个同名 outbox**——`CodeLab/collab/outbox/tester-1/result.md` 是 dispatch.sh 实际写入路径（本次 13095 字节，正确）；`voice-ime/collab/outbox/tester-1/result.md` 是项目内同名目录，仍停留在 07-30 的旧任务文件。主控首次误取后者，一度怀疑 [COLLAB-WRITE-001] 复发，**实际未复发**。后续一律以前者为准，项目内陈旧副本建议清理。

**下游**：原预设的 `MACOS-FIX-COMPILE-002` **无需派发**——零错误，无可修。

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| MACOS-TESTEXEC-V073-001 | **测试执行零改动**：主程序 `cargo test` + `src-tauri` + Vitest + pytest `--collect-only`；**并量化跨平台测试盲区**；定向复核 `time_half` 与 ITN 1386 条词表 | — | tester-1 | ✅ **已验收**（2026-07-31，含主控两处实质补正） |

**🎉 核心结论：v0.7.3 在 macOS 上唯一失败是既有的 `itn::tests::time_half`，且属 Gavin 明知并接受的取舍。** 完整数字（主控 `--no-fail-fast` 取得）：**701 passed / 1 failed / 8 ignored = 710**，与 `--list` 实测 710 完全自洽。`src-tauri` **53/0/0**、Vitest **54/54**、pytest 收集 147/156 无回归——三项主控均亲自复跑确认。

**❌ 主控补正一（本次验收最有价值的一项）· 汇报的「全量」实际不全**：`cargo test` **默认在首个失败 target 处中止**，后续测试二进制不执行。汇报的 `637/1/6` **只是主程序 bin 一个 target**，其后 crash-reporter bin（28/0/2）与 5 个集成测试（3+12+10+2+9）**在其会话中从未运行**，共 **66 条**。汇报中其实已有未被察觉的内部矛盾：`637+1+6=644` 而同报告 `--list` 为 710，差 66 无人解释。**结论方向未变（确实只 1 个失败），但此前被验证的范围不足。**

> **⚠️ 派发纪律（即刻生效）**：**后续所有测试执行类任务的任务书必须强制要求 `cargo test --no-fail-fast`**。否则在存在任一失败时，「全量回归通过」是不成立的断言。

**🔍 `time_half` 根因主控已定位到确切行号**：断言在 `src/itn.rs:1295`（`normalize_test("八点半")` 期望 `"8点半"`）；根因是 `itn-rules.toml:432` 的 **`[protect.unit_collisions]`** 分组内含 **`"八点半"`**——正是 `0adb819` 本批次新增的 1386 条 Type A 碰撞保护词表，保护该词导致「八」不再转「8」。**这不是缺陷，是 Gavin 2026-07-30 明知并接受的取舍**（[ITN-PREFIX-SHADOW-001]，「明知前缀遮蔽缺陷仍照常落地」「不以测试通过为出包前提」）。断言与词表均为平台中立文件，**Windows 侧同样红，与平台无关的判断成立**。三个待选处置（改断言 / 从词表剔 `八点半` / 保持现状）仍待 Gavin 端测后决定。

**❌ 主控补正二 · 盲区数应为 22 非 23，占比应为 ~3.0% 非 3.27%**：

| 来源 | 条数 | 门控方式 |
| --- | --- | --- |
| `src/main.rs` | **5**（汇报记 6） | 逐条 `#[cfg(target_os = "windows")]`，与 `6f0b51e` 实际只给 5 个 `#[test]` 加 cfg 吻合 |
| `src/platform/windows/hotkey.rs` | 15 | **模块级**——`src/platform/mod.rs:58-59` 的 `#[cfg(target_os = "windows")] mod windows;` |
| `src/platform/windows/scene.rs` | 2 | 同上（汇报记作「scene 2」，实为同一模块级门控，非独立 scene 模块） |
| **合计** | **22** | 另核实被门控的 `mod hotkey`/`mod injection`（`main.rs:9/12`）内 `#[test]` 均为 0 |

占比口径：汇报的 3.27% 用了分母 703（**Windows v0.7.2 的 `--list`**），与 v0.7.3 macOS 数字混用。正确为 `22/(710+22)` ≈ **3.0%**。

**💡 盲区的实质分布比占比更重要**：22 条中 **17 条（77%）集中在 `platform/windows/`**，即**热键与场景采集**。这两块 macOS 有各自独立实现（`platform/macos/hotkey.rs` 等），**其行为在本侧零测试覆盖** —— 补 macOS 管线时这里是首要补测目标。

**📌 附带核实：TEST-FIX-002/003 应从下方遗留表移除**——主控核实 `ui/src/test/setup.ts:18` 的 `vi.mock("@tauri-apps/api/core", ...)` 早在 **`f10c1e0`（v0.6.2）** 即存在，非近期修复，**这两条遗留记录长期过时**。本次 macOS 侧 54/54 全绿可作移除依据（建议 Windows 侧复核一致后删除）。

**边界合规**：`git status --porcelain` 仅主控自己的 `M collab/todo.md` + `?? logs/20260731.md`；源文件/测试文件/版本号/`ui/package-lock.json` 全部零改动；未 npm install、未出包、未 cargo clean、未执行 pytest 用例、未用 git 破坏性命令。

**派发动因（Gavin 2026-07-31 选定）**：编译过 ≠ 行为对。本批次带进 1386 条 ITN 碰撞保护词表 + ITN 顺序反转（DEC-035）+ 翻译双向化，均未在 macOS 上跑过测试。

**任务书的核心设计 —— 目标 B「量化跨平台测试盲区」**：当前无 CI 防线，`#[cfg(target_os = "windows")]` 门控的测试在 macOS 上**根本不会编译进测试二进制**，故 macOS 上「全绿」覆盖不到的那部分行为**永远不会在本侧暴露**。**我们至今不知道这个盲区有多大**。要求产出：macOS `--list` 实际条数 / ignored 数 / 被 cfg 门控掉的条数 / **盲区占比**，并列出**哪些模块在 macOS 上完全无测试覆盖**。这是本任务相对常规回归的独特价值。

**主控派发时已明确的判据**：**failed 数是唯一判据，passed 数低于 Windows 基线属正常**（cfg 门控必然导致 macOS 跑到的条数更少），不得当成回归上报。Windows v0.7.2 对照基线：主程序 695/0/8、src-tauri 53/0/0、Vitest 54/54。参考量级：全仓 `src/` 共 676 处 `#[test]`（`llm/mod.rs` 103 / `itn.rs` 97 / `translation/mod.rs` 32 / `config/mod.rs` 27）。

**要求按三类归因每个失败**：**(a)** 真实跨平台缺陷 ｜ **(b)** 测试自身的平台假设（写死 Windows 路径/分隔符/换行符）｜ **(c)** 既有遗留（TEST-FIX-002/003、`time_half`）。三类处置方式完全不同。

**已明示为能力缺口而非跳过**：`collab/build-test-guide.md` 的 Step 3/4（pytest smoke / E2E）全文以 Windows 为前提（pywinauto / `.exe` / PowerShell 截图 / `RegisterHotKey` 模拟），**macOS 侧目前没有可用的 GUI/E2E 测试实现**。本轮 pytest 只做 `--collect-only`（确认 `2051993` 解锁的 147 条仍能正常收集，防收集期回归）。补 macOS 章节仍是 DEC-033 附则三列出的待排期文档缺口。

**派发动因**：本次 pull 大改的正是**双侧共享的平台中立模块**——`src/itn.rs` **+451**、`src/main.rs` **151 行变动**、`src/translation/mod.rs` **+196**、`src/config/mod.rs` **+54**——而**这 5 个提交在 macOS 上从未编译过**。DEC-033 附则二警告的「破坏在提交那刻发生、切机器那刻才暴露」窗口正在此处，当前无 CI 防线。

**对比基线**（`2c98976` + MACOS-FIX-COMPILE-001 后实测）：`cargo check` **0 errors**；`--all-targets` 仅剩 1 个既有 test-only E0432（`select_preprocessing_params`，原 `main.rs:4349`，本次行号必然已漂移）。

**要求 Worker 按两类定性每个新错误**：**(a)** 平台中立模块自身的编译错误 ｜ **(b)** cfg 门控漂移（Windows 侧新增符号只在 `cfg(windows)` 下存在）。两类修复方式不同，(b) 属 DEC-033 接缝问题需回报对侧或补 stub。特别核查 `AppConfig` +54 行是否新增/改名字段（DEC-033 附则三第 2 条点名的最高风险动作）。

**下游**：错误清单 → `MACOS-FIX-COMPILE-002`（尚未派发）。

### 主控已核实的 Windows 侧交接要点

| 项 | 主控独立取证结论 |
| --- | --- |
| **DEC-035 管线顺序契约**（`MACOS-HANDOFF.md` §2.8） | ✅ 确为**纯文档契约**——`src/main.rs:3431` 的 `mod macos_stubs` 仍在，`run_pipeline` 整个函数在 `#[cfg(target_os = "windows")]` 内，**代码层无法共享**。本方将来实现管线时若把 ITN 放回 LLM 之前，℃ 缺失缺陷会在 macOS 完整复现 |
| **§2.6 平台中立模块清单** | ✅ `grep -c target_os` 实测为 **0** 属实：`itn.rs` / `translation/mod.rs` / `text_normalizer.rs` / `config/mod.rs` / `llm/mod.rs` 五个模块可双侧直接复用 |
| **§2.4 禁止 AI 署名** | 已知悉。规则原只在 Gavin 用户级 `~/.claude/CLAUDE.md`，本方此前无从得知；`a38a315` 已带 `Co-Authored-By`，对方明确**不要求返工**，后续遵守 |
| **DEC-030-① 已作废** | 已知悉：ITN「转录后 / LLM 前挂入管线」的原文自 DEC-035 起作废，任何 Agent 不得依 DEC-030 原文把 ITN 挪回 LLM 之前 |

### ⏳ 阻塞在 Gavin 决策上

| 项 | 说明 |
| --- | --- |
| **npm lock 双平台失同步** | Windows 侧 §6.1 已依实测**推翻**原「两侧统一 `npm ci`」约定，并更正了自己「大版本相同、漂移风险低」的原判断。实测：任何一侧单独生成的 lock 都无法同时满足对方 `npm ci`，差异纯粹是 npm 11.6.2 与 11.16.0 记录 lock 完整性的方式不同（**包版本逐键比对完全一致，无依赖漂移**）。真正解法是钉死两侧 node/npm 版本；对方已用免提权 shim 把 Windows 侧收口到 11.16.0。**待 Gavin 拍板是否升级 Windows 侧 node，本项阻塞中。**本方在此期间**禁止任何 Worker 执行 `npm install` / `npm ci`** |

### 📌 附带发现（协作框架缺陷，未修复）

`collab/lib/env.sh:17` 用 `${BASH_SOURCE[0]}` 推导 `COLLAB` 路径，**zsh 下失效**（zsh 不设 `BASH_SOURCE`）→ 变量为空 → `COLLAB` 被推导成当前目录的父目录，`dispatch` 报路径错误。**与 2026-07-30 coder-1 在 `scripts/env-macos.sh:11` 修掉的是同一个 bug**（修法为 `${BASH_SOURCE[0]:-$0}`）。macOS 默认 shell 即 zsh，凡从 zsh 直接 `source dispatch.sh` 均会踩到。主控当前用 `bash -c` 绕过，**未改动该文件**（属 `collab/` 框架，在本仓库之外）。

---

## ✅ 工作冻结令已解除（2026-07-30）

> 原冻结令（Gavin 2026-07-29「目前是做好跨平台开发的代码重构和准备，ok 前先不做任何新代码开发」）已随跨平台重构批次闭环解除——Gavin 于 2026-07-30 解冻并指令实施 FIX-COT-LEAK-001-P0（依据 `logs/20260730.md`）。DEC-033 第 2 条「平台兼容为首要约束、不得再产出仅 Windows 可编译的新代码」**继续长期有效**。

---

## ✅ 已出包待端测 · ITN-COLLISION-TYPEA-002 Type A 碰撞保护词表（2026-07-30 23:00 出包 v0.7.3）

> **当前状态**：1386 条已随 v0.7.3 出包并经主控 9 项独立验收，PID 23276 运行中，**等 Gavin 端测反馈**
> 前置：ITN-COLLISION-TYPEA-001 阶段一（coder-2 交付，报告 `collab/research/itn-lexicon-collision-001.md`）
> **Gavin 拍板三项**：① 落地形式 = **新增 `[protect.unit_collisions]` 独立分组**（非并入 proper_nouns → 需改 Rust + 出包）② 规模 = **≤6 字档** ③ **明知前缀遮蔽缺陷仍照常落地**（2026-07-30，见下）

### 🔑 Gavin 的取舍判断（推翻主控首轮结论，已写入 troubleshooting [ITN-PREFIX-SHADOW-001]）

主控首轮结论是「整批否决、路线结构性不可行」，**Gavin 推翻并纠正了主控的严重性定性**：

`check_protection` 命中后只做「原文逐字抄出 + 游标前移」，**物理上不可能产出畸形文本**。故两种失败模式不对称：

| 失败模式 | 输出 | 性质 |
| --- | --- | --- |
| **漏**保护 | `三角形` → `3角形` | 文本被改坏，用户须手动修 |
| **误**保护 | `二分钟` 保持汉字 | 优化未生效，退回 ITN 之前状态，文本仍正确可读 |

**误保护 = 优雅降级；漏保护 = 输出损坏。** 主控原写的「误保护比不保护更糟」**错误，已收回**。且触发频率也不对称——遮蔽需「词条恰为文本前缀」（窄），碰撞只需「数字后跟单位首字」（宽，单位表 68 词）。**故用大词表换覆盖率是合理取舍。**

### 实测收敛数据（好于预期）

**1386 条只打破 1 条既有断言** —— 96 个 itn 测试中仅 `time_half`（`八点半`→`8点半`）变红。既有测试覆盖面与该词表重叠极小。

### ⚠️ 未收口项

| 项 | 状态 |
| --- | --- |
| `time_half` 单测红 | **未修**。Gavin 明确「不以测试通过为出包前提」。待端测后决定：改断言 / 从词表剔 `八点半` / 保持现状 |
| git 提交 | **1386 条 + `src/itn.rs` +34 仍未提交**，等 Gavin 指令 |
| 端测反馈 | 观察点见下 |

**端测观察点**：① 该转没转（新引入）：`二分钟` / `三元钱` / `九度电` / `一点半左右` ② 本该修好：`三角形` 不再变 `3角形`。若前者几乎遇不到 → 词表立住；若频繁硌手 → 按 troubleshooting 优先剔 ≤3 字那 603 条（遮蔽面最大）。

### 出包验收记录（2026-07-30 23:00 产物 / 23:12 主控验收，BUILD-RELEASE-20260730-002）

主控 9 项独立取证全过，**未采信汇报表格**：

- **探针 8/8 命中**：`八里庄北里` / `一个十七八岁` / `三角剖分` / `一个九十度` × 两副本全为 1。方法有效性已预验证——同串在旧 exe 中为 **0**，对照串 `八达岭`/`一心一意` 为 1
- **两副本 sha256 逐一一致**：`74e4b56a…` / `16acff20…` / `cc2ee873…`；主程序 11,798,016 B
- **`itn-rules.toml` 三副本 sha256 `9f36efcb…` 一致**（33,252 B）——这一步是本次最关键的，漏了则外置旧 toml（9,689 B）会赢过新内置默认，1386 条完全不生效且日志表现正常（[TOML-STALE-001]）
- **版本撕裂已消除**：主程序 0.7.3.0 / UI **0.7.3**（此前 UI 停在 0.7.2，故本次 Tauri UI 必须重建）
- **运行时实证**：debug.log 23:07 `ITN rules loaded from ...target\release\itn-rules.toml`，晚于 23:00 同步
- **因果交叉验证**（未只看数字）：`八点半` 在出包 toml 中确实存在 1 次，`src/itn.rs:1295` 断言 `8点半` → time_half 必挂，96/1 汇报自洽
- **边界合规**：源文件零越权改动；`scene-rules.toml` 三副本仍 `7b01b33c…`
- **⚠️ [COLLAB-WRITE-001] 第三次复发**：`result.md` 交付时 0 字节，经主控要求补写为 3644 B（tester-1 改用 WSL Python 绕过）

---

## （历史）派发时的方案记录

| 项 | 决定 | 依据 |
| --- | --- | --- |
| **词库来源** | jieba (MIT) + THUOCL (MIT)，**弃用 CC-CEDICT** | CC BY-SA 4.0 的 share-alike 对派生数据有传染性解释空间 |
| **过滤方案** | **F1 语义规则**（非 F2 长度阈值） | F2 误删「一个三十多岁」等年龄表达 287 条，又留下「一千万」等该剔的金额串 269 条 |
| **数量链** | 2107(Type A) → F1 净化 1741 → 剔 CC-CEDICT **1521** → ≤6 字 1390 → 去重 **1386** | 主控独立实算；词长分布 2字5/3字598/4字488/5字200/6字95 |
| **影响文件** | `itn-rules.toml` + `src/itn.rs`（数据源只读） | 边界零重叠，单 Worker |

**主控核心设计约束（任务书 §四，本方案成立与否的关键）**：新字段必须是 `HashMap<char, Vec<String>>` **分桶 Map，不是第六个 HashSet**。两条理由——① `check_protection`（`:1063`）对既有五个 `*_set` 做的是**遍历 + `starts_with`**（不是哈希查表），O(n) 复杂度，且 `:678` 每遇一个数字位置就调一次；分桶后最大桶 387 条（首字「一」），worst-case 降到 **28%**，实测只落 11 个桶 ② **`HashSet` 遍历顺序不确定 → 最长匹配无保证**（`一个三十` vs `一个三十多岁` 同时存在必然踩到）；桶内按字符数降序排序才是确定性最长匹配。分桶安全性依据：`rest` 起点恒为数字位置，故所有保护词首字必为中文数字。

**❌ 主控查出阶段一报告一处勘误**：§四.4 称「三角洲：jieba + THUOCL」并据此论证「弃用 CC-CEDICT 不影响 Type A 目标覆盖」。实测 `三角洲` **不在** 那 1521 条里（只在含 CC-CEDICT 的 v4 全集）。**结论不受影响**——`三角洲` 已由 `a07a089` 几何白名单硬编码进 `itn-rules.toml`。同理 `三角形`/`三角函数` 落在去重剔除的 4 条内，仍由既有分组保护。

**下游（严格串行，禁止并行派发）**：coder-2 完成验收 → TEST-SYNC-TYPEA-002（tester-1 补单测）→ TEST-EXEC（全量回归）→ **必须出包**（本次改 Rust，非纯数据免构建）。

---

## ✅ ITN 顺序反转 + ℃ 独立通道 + 翻译双向化 批次闭环（2026-07-30 19:40）

> 来源：Gavin 端测两项反馈 —— ①「说摄氏度输出没转成 ℃」②「开了翻译按热键输出没译成英文」
> 决策：**DEC-035**（ITN 位置反转，DEC-030-① 原文作废）

| 编号 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| ITN-REORDER-001 | ITN 从「LLM 前」移到「三分支后、标点前」，一处覆盖三条路径 | coder-1 | ✅ 已验收（**主控修 1 处**：重建失败返回方向不符旧引擎 → 改为返回 None，防注入幻觉译文） |
| ITN-CELSIUS-003 | ITN 新增**独立于中文数字路径**的单位符号通道（`40摄氏度`/`40°C`→`40℃`），`itn-rules.toml` 新增 `[[unit_symbols.rules]]` 三条；`44度` 绝不转 | coder-1 | ✅ 已验收 |
| TRANS-BIDIR-001 | 方向按内容自动判定，删除 `should_translate_for_language` 门控；单槽位引擎换向；`target_language` 语义改「上次方向缓存」 | coder-1 | ✅ 已验收 |
| REFACTOR-DERIVE-TARGET-001 / REFACTOR-SHARE-TRANSDIR-001 | 三个函数移入平台中立模块（`translation/mod.rs` / `config/mod.rs`）去 cfg 门控，macOS 可直接复用 | coder-1 | ✅ 已验收 |
| TEST-SYNC ×2 轮 | 两条失效断言修正 + ITN 新域 7 条 + 方向矩阵 5 条 + 3 条测试随函数搬入中立模块 | tester-1 | ✅ 已验收 |
| TEST-EXEC ×2 轮 | **717 passed / 0 failed / 8 ignored**（分项 28+653+3+12+10+2+9 自洽 = `--list` 725）+ Tauri 53/0 + Vitest 54/0 + toml 三副本一致 | tester-1 | ✅ 已验收 |
| NPMLOCK-UNIFY-001 | corepack 钉 `packageManager: npm@11.16.0`；**零改动假设成立，lock 一字节未改** | coder-1 | ✅ 已验收（**主控修 1 处表述**：默认 npm 仍 11.6.2） |

**⚠️ 首轮 TEST-EXEC 6 个失败的根因（重要教训，已写入 troubleshooting 备查）**：`normalize_test` 测试助手只调 `normalize_with_rules`，**绕过公共入口 `normalize_numbers` 的第二阶段** —— **96 条断言长期在测一条非公共路径**。生产代码无误。主控改助手直调公共入口后全绿。与 `builtin_rules_parse_ok` 那个「内联 fixture 覆盖不到真实 toml」的黑洞属同一类缺陷。

**npm 环境收口（2026-07-30 19:41）**：`corepack enable npm` 需写 `C:\Program Files\nodejs` 会 EPERM（Gavin 两次尝试均未生效，主控实证）。改用免提权方案 —— shim 装到 `%USERPROFILE%\bin` + 前置用户级 PATH，全新进程实测 `npm --version` = **11.16.0**。

---

## ✅ 已并入 macOS 团队首批交付并通过 Windows 零回归验收（2026-07-30 14:40）

**合并**：`e9296ba`（并入 `5e3ed89`/`6f0b51e`/`4b2126b`/`b04596b`），本地 **ahead 2 未 push**（等 Gavin 指令）。

**Windows 零回归结论成立**：`cargo check --tests` + `cargo check src-tauri` 双 0 errors、**两个 `Cargo.lock` 零改动**、`cargo test` **695/0/8** + src-tauri **53/0/0** + Vitest **54/54**（主控 `--list` 独立计数 703 = 695+8 自洽，5 条 cfg 门控测试确在运行）。详见 `logs/20260730.md`。

**🔴 附带暴露我方缺陷（已记录，无需修复——对方已修）**：coder-2 的 windows target 段插在 `[dependencies]` 中间，静默吞掉 `tokio-tungstenite`/`futures-util`/`rustls`；Windows 上按构造不可见，主控 07-29/30 验收漏检。**审 `Cargo.toml` target 段调整时必须看段落边界之后还剩什么**。

### ⏳ 待办：npm lock 协同方案回执（`docs/MACOS-NPMLOCK-COORDINATION.md`）

| 项 | 状态 |
| --- | --- |
| 其 §1 论断（两平台都跑不了 `npm ci`） | ✅ **主控已在 Windows 复现 EUSAGE**（npm 11.6.2 / node v24.11.1） |
| 其 §8-1 问我方 npm 版本 | ✅ 已取得，与对方**大版本相同**（npm 11 / node 24），漂移风险低 |
| 执行方 | **建议由 macOS 侧执行**（其隔离实验已验证 12 个顶层 win32 条目无损，单文件单提交可 revert），我方做 §5 验证方 |
| ❌ 其 §5 步骤 4 有误 | 要求跑 `npm run tauri build`，但**我方出包从不走 npm tauri CLI**（[BUILD-001] 禁用 `cargo tauri build`），该包不在关键路径 → 回执时改为 `cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol` |
| ⚠️ 跨团队引用缺口 | ① 其引用 **DEC-034**，我方 decisions.md 只到 DEC-033 ② 其引用 `collab/` 内条目，而 `collab/` 在 `.gitignore` 内两侧不共享 → **建议约定「跨团队引用只许引 `docs/`」写入 `MACOS-HANDOFF.md`**（待 Gavin 点头派发） |

---

## ✅ BUILD-RELEASE-20260730-001 出包已完成并验收（2026-07-30 13:13 产物 / 14:12 主控验收）

**产物**：三 exe 于 **13:02~13:13** 重建并同步 Publish/，`target/release` 与 `Publish` 两副本 sha256 **逐一一致**：

| 产物 | 新 sha256（前 8） | 旧值（前 8） | 大小 |
| --- | --- | --- | --- |
| feiyin-ime.exe | `8da29081` | `e35679bd` | 11,615,744 B（+12,288） |
| feiyin-ime-ui.exe | `d9db29e3` | `0d76eca1` | 10,026,496 B（−512） |
| crash-reporter.exe | `950e1474` | `8bfabfb5` | 24,858,624 B |

**主控独立验收（未采信汇报表格）**：

- **V1 决定性探针**：`grep -ac "LLM response meta" feiyin-ime.exe` = **1**（旧 exe 实证为 0）→ `ff492ef` 确已进 exe，两副本均命中
- **✅ 主控补证 tester-1 降级后的缺口（本次验收最有价值的一项）**：它把 V2 标 N/A 并说明「`src-tauri/src/llm.rs` 无 `LLM response meta`」——**属实**（主控核读源码确认 P0-5 镜像只在主程序侧），但它**未换探针证明 Tauri 侧新代码就位**。主控改用 `ff492ef` 在 Tauri 侧引入的独有串复测 UI exe：`enable_thinking`=1 / `reasoning_content`=1 / `disabled`=8 → **P0-1/P0-2 的 Tauri 侧镜像确已就位**
- **V4 ProductVersion 0.7.2.0**（UI 侧 0.7.2）→ 未升版，红线遵守；三处版本号文件复核均为 0.7.2
- **V5** 产物 mtime 13:02~13:13 **晚于**最新源码 07-30 00:57
- **V6** `index-BNQZfcUG.css` / `index-CTgGziQm.js` 各命中 1（[BUILD-002] 防旧构建）
- **toml 三副本** sha256 一致：scene `7b01b33c…`、itn `209ac1e7…`
- **边界合规**：`git status` 仅 `CHANGELOG.md`（+2/−0，任务书允许），源文件与版本号零改动
- **耗时**：主程序 10m02s + Tauri UI 2m51s + npm 1.55s；**CT2 陷阱未触发**（主控预置的 HTTP/1.1 + postBuffer 生效，7 个 third_party 全新 clone 一次成功）

**两处遗留（已通知 tester-1）**：

1. **`result.md` 仍是 0 字节** —— [COLLAB-WRITE-001] 再次复发，汇报只落在 CHANGELOG 与 tmux，已要求补写
2. **冒烟实例 PID 4920 于 14:08:59 退出**（日志为正常 `initiating shutdown`，非崩溃、非 [SMOKE-VANISH-001]）→ 主控已重新启动 **PID 5336**（14:12:45，Responding=True，模型加载正常零 panic）供 Gavin 端测

**⏭ 待 Gavin 端测补证运行时闭环**：说一句话，日志出现 `LLM response meta: finish_reason=stop`、且注入内容不再是「我们分析用户输入…」即 [LLM-COT-LEAK-001] 真正闭环。

---

## （历史）出包派发记录（2026-07-30 12:58 派发 tester-1）

| 项 | 主控决定 | 依据 |
| --- | --- | --- |
| **版本号** | **维持 0.7.2 不动**（任务书列为红线） | Gavin 只说「出包」，未授权升版；「版本号禁止擅改」 |
| **范围** | **全量三步**：npm build + Tauri UI(`--features custom-protocol`) + 主程序 + Publish 同步 | 本批 `src-tauri/src/llm.rs` 有改动，不可如 07-28 跳过 Tauri UI |
| **耗时** | **20+ 分钟全量重编** | 主控取证：`target/release/build`、`target/release/CTranslate2-4.6.0`、`src-tauri/target/release` 三者均不存在 |
| **CT2 陷阱前置** | 已就位（`http.version=HTTP/1.1` + `postBuffer` 全局生效），诊断口诀与处置写入任务书 | [CT2-SUBMODULE-DEADLOCK-001] |
| **决定性验证探针** | `grep -ac "LLM response meta" feiyin-ime.exe` ≥ 1 —— 主控实证**旧 exe 该值为 0**（对照串 `Injecting text`=1 证明方法有效） | 比 mtime 可靠，直接证明 `ff492ef` 进了 exe |

**副作用提醒**：Step 1 会终止 Gavin 正在端测的实例 PID 8592，构建期间语音输入不可用。

**遗留（本次未做）**：升版决策仍悬空 —— 维持 0.7.2 后将存在**第三个**同版本号不同内容的构建（07-27 144词表版 / 07-28 165词表版 / 本次含跨平台+LLM修复版），仅靠 sha256 区分。若 Gavin 更看重可追溯性，可另下指令升 0.7.3 重出包（需再等一次全量重编）。

---

## 📌 出包决策原始记录（2026-07-30 派发前）

**背景**：MACOS-COMPAT-001 + AUDIT-MACOS-BRANCH-001 + FIX-COT-LEAK-001-P0 三批次**代码、测试、文档、git 全部闭环并已 push**（`292eeb0` / `ff492ef` / `2c98976`，`git status -sb` 无 ahead/behind），唯独**未出包，且被 Gavin 明确叫停**。

| 待拍板 | 说明 | 主控建议 |
| --- | --- | --- |
| ① **版本号** | 维持 0.7.2 将产生**第三个**同版本号不同内容的构建，只能靠 sha256 区分 | 建议升 **0.7.3**（版本号是 Gavin 决策权，主控不擅改） |
| ② **构建范围** | 本批 `src-tauri/src/llm.rs` 有改动 → **Tauri UI 必须一并重建**，不可如 07-28 那次跳过 | 三步全量：npm build + Tauri UI(`--features custom-protocol`) + 主程序 |

**⚠️ 耗时提醒**：07-28 磁盘清理已删 release 中间产物，本次为**全量重编**（含 CTranslate2 C++ CMake），预计 20+ 分钟；release 侧可能需再走一遍 [CT2-SUBMODULE-DEADLOCK-001] 处置。

**出包后的端测观察点**：新增的 `LLM response meta:`（finish_reason + usage）日志、DeepSeek 思维链泄漏是否消失（原约每 7 次 1 次注入 `...`）。

---

## 进行中

### 2026-07-30 · macOS 侧交接接管（已完成 checkout + 独立核验，**任务尚未派发**）

> 来源：Windows 侧团队完成 MACOS-COMPAT-001 双平台接缝适配 + 四份交接文档 → Gavin 指令「checkout 最新代码，先做好交接、理清项目，暂不派发任务」
> 治理约束：**DEC-034**（跨平台兼容为首要约束 + 单仓库两端并行）
> 交接文档（均在仓库内、受 git 管辖）：`docs/MACOS-HANDOFF.md`（入职材料，250 行）/ `docs/MACOS-PORT-ASSESSMENT.md` / `docs/BUILD-MACOS.md` / `docs/MACOS-BRANCH-AUDIT.md`（cfg 分支静态审计）

**checkout 结果**：`695e50e` → `2c98976`（fast-forward，4 个提交），`main...origin/main` 已同步。工作区保留 macOS 侧两处真实改动（`.gitignore` +2 / `scripts/build-macos.sh` +11−2）+ 三个未跟踪文件。

**主控独立核验（未采信交接文档结论，逐项查证源码）**：

- ✅ **平台契约属实**：`src/platform/mod.rs:61/71` 两份显式清单各 15 个符号，glob 导出已废除；macOS 侧 15 个符号**逐一 grep 全部存在**，无遗漏
- ✅ **8 项 07-29 实测阻塞项中 5 项已由 Windows 侧修复**：`mod hotkey` / `mod injection` 加 `#[cfg(target_os="windows")]`（消 10 个 E0432）、`src/crash/mod.rs` 补 `get_windows_version()` 非 Windows 占位、`src-tauri/Cargo.toml` 的 `windows` 依赖移入 `[target.'cfg(target_os = "windows")'.dependencies]`、`src-tauri/src/main.rs` 补 `check_hotkey_available` 非 Windows 分支、`src-tauri/src/overlay.rs` 的 `.transparent(true)` 改 `#[cfg(not(target_os="macos"))]`
- ✅ **审计的 P0 属实**：`src/crash/reporter.rs:369` 确为 `egui::FontData::from_bytes(...)` 而 `Cargo.lock` 中 egui = **0.29.1**；同文件 `:347` 的 `include_bytes!("C:/Windows/Fonts/msyh.ttc")` 在 `#[cfg(target_os="windows")]` 块内，macOS 不展开，**不构成第二个 P0**（审计未误报）
- ✅ **审计的依赖版本基线正确**：`Cargo.lock` 同时存在 core-graphics 0.23.2 与 0.25.0、core-foundation 0.9.4 与 0.10.1，但根 crate `Cargo.toml:112-114` 为 macOS 声明的是 `core-graphics = "0.25"` / `core-foundation = "0.10"` / `enigo = "0.2.1"`，与审计核对基线一致（旧版本属其他 crate 的传递依赖）
- ❌ **主控修正 · 审计头条结论低估 2 项**：`docs/MACOS-BRANCH-AUDIT.md` §1/§2/§5 称「**唯一**会阻塞编译的是 `crash/reporter.rs:369`」。**实际剩 3 项**——另两项是 `src/platform/macos/hotkey.rs:124`（`CGEventType` 不支持 `==`）与 `:257`（`create_runloop_source` 返回 `Result` 却调 `.ok_or_else`）。依据：① 这两项是 macOS 侧 07-29 **真实 `cargo check` 实测所得**，已记录在同一仓库的 `docs/BUILD-MACOS.md` §四（审计的参考资料清单里就有这份文档）；② `git log -- src/platform/macos/hotkey.rs` 显示该文件**自初始提交 680d78f 以来从未被改动**，Windows 侧本批次未碰它，故错误必然仍在；③ 审计 §3 表格自己写明 `create_runloop_source` 返回 `Result<CFRunLoopSource, ()>`，却把该调用点列为「签名匹配」——**与调用处的 `.ok_or_else` 自相矛盾**。根因是审计自述的「仅静态阅读 + docs.rs，未在 macOS 上运行 cargo check」，属方法固有局限而非疏忽，但**头条结论的措辞会误导下游按「只剩 1 个错误」排期**
- ⚠️ **工具链未就绪**：`cargo` / `rustc` 不在 PATH（rustup 装在用户目录，需 `source scripts/env-macos.sh`），故本次核验全为静态查证，**未实跑 cargo check**
- ✅ **CT2 构建树完好（好消息）**：`target/debug` 1.6G 存活，`CTranslate2-4.6.0/third_party/` 下 7 个子目录**全部非空**（cpu_features 14 / cutlass 22 / cxxopts 12 / googletest 12 / ruy 11 / spdlog 13 / thrust 17），**未处于 [CT2-SUBMODULE-DEADLOCK-001] 的残缺态**；sherpa-onnx 四个 dylib 已在 `target/debug/`。下次 `cargo check` 不需要重编 CT2

**待派发任务（已理清，等 Gavin 下令）**：

| 编号 | 内容 | 影响文件 | 建议负责人 | 状态 |
| --- | --- | --- | --- | --- |
| MACOS-PR1-SCRIPTS-001 | **Windows 侧明确交接请求**（`docs/MACOS-HANDOFF.md` §4.2）：`setup-macos.sh` + `env-macos.sh` 入库 + `build-macos.sh` 占位替换；派发后追加 C 项（修 `env-macos.sh` 的 zsh 路径失效）与 A 项（`npm install`→`npm ci`） | `scripts/{setup,env,build}-macos.sh` | coder-1 | ✅ **已验收** |
| MACOS-FIX-TAURI-DEPS-001 | 修 `292eeb0` 的依赖段位回归（`tokio-tungstenite`/`futures-util`/`rustls` 被误划 Windows 专属）+ 收掉 `src/main.rs:4349` 既有 test import 错误 | `src-tauri/Cargo.toml` + `src/main.rs` | coder-2 | 🔄 **进行中**（Gavin 2026-07-30 拍板「自己修」，不退回 Windows 侧） |

**PR1 验收记录（2026-07-30 主控独立取证）**：

- **C 项（本任务最有价值的改动）**：`env-macos.sh:11` 改为 `${BASH_SOURCE[0]:-$0}`。**主控独立双 shell 复跑通过**——bash 与 zsh 下 `source` 后 `$SHERPA_ONNX_LIB_DIR` 均指向仓库内实际存在的 lib 目录并能列出 dylib。**另做反向验证**：在 zsh 下手工执行修复前的写法，得到 `/Users/gavinsun/Workspace/CodeLab`（**仓库的父目录**），与诊断完全吻合 → 证明该 bug 真实存在且已被修复。意义：`docs/BUILD-MACOS.md` §一 教给所有新人的 `source scripts/env-macos.sh` 此前在 macOS 默认 shell 下是失效的
- **A 项**：`npm install` → `npm ci`，且按主控裁定实现为**失败时响亮报错 + exit 1 + 指向 MACOS-FIX-NPMLOCK-001，绝不 fallback 到 npm install**，注释中援引 `[NPM-LOCK-CROSSPLAT-001]` 与 DEC-034。同时按 `npm ci` 自带清空 node_modules 的语义简化了原第 61-64 行的手工清理逻辑
- **B 项**：`build-macos.sh` 工作区既有改动（REPO_ROOT / source env / `voice-ime`→`feiyin-ime`）完好保留，未回退
- **三脚本 `bash -n` 语法检查全过；UTF-8 无 BOM**（首三字节均 `23 21 2f`）
- **`ui/package-lock.json` 零改动**（主控独立 `git diff --stat` 确认为空）——跨平台红线守住
- **边界合规**：`git status` 中 `scripts/` 外无本任务改动
- **过程亮点**：coder-1 在撞上 `npm ci` EUSAGE 后**没有自行降级为 `npm install` 交差，而是停下来上报三个选项请主控裁定**，符合 worker-guide §七「主动沟通比默默出错代价更低」

**衍生立项 · MACOS-FIX-NPMLOCK-001（待 Gavin 决策）**：`ui/package-lock.json` 与 `package.json` 长期失同步（`npm ci` 报 EUSAGE：`@emnapi/core@1.11.3`、`@emnapi/runtime@1.11.3` 缺失，`@emnapi/wasi-threads` 锁定 1.2.2 不满足 1.2.3），均为传递依赖；`git log -- ui/package-lock.json` 仅两次提交（`680d78f` 初始 + `f10c1e0` v0.6.2），即**该缺陷长期存在于两个平台**。**后果**：`docs/MACOS-HANDOFF.md` §6.1 提出的「两侧统一用 npm ci」约定当前在**任何平台都无法执行**，Windows 侧同样会 EUSAGE。**限度**：只卡全新 clone，现有 `node_modules` 完好、`npm run build` 正常。**难点**：修 lock 需在某一平台跑 `npm install`，而这正是 `[NPM-LOCK-CROSSPLAT-001]` 警告的动作，需决定由哪侧执行、是否用 `--package-lock-only`、以及如何验证两平台都能 `npm ci`。**建议与 Windows 侧协同处理**（属共享文件，单侧修完对侧仍可能失败）。
| MACOS-CARGOCHECK-BASELINE-001 | 拿真实错误清单：`source scripts/env-macos.sh` 后跑三处 `cargo check`（主程序 / `src-tauri` / crash-reporter bin），产出完整错误清单。**这是 DEC-033 §执行前提与评估报告 §11 共同指定的第一步** | — | tester-1 | ✅ **已验收** |
| MACOS-FIX-COMPILE-001 | 修 3 项编译阻塞：`crash/reporter.rs:369` → `FontData::from_owned`（连带删 `.ok().unwrap_or_default()`）；`hotkey.rs:124` → `matches!`；`:257` → `.map_err` | `src/crash/reporter.rs` + `src/platform/macos/hotkey.rs` | coder-2 | ✅ **已验收** |

**BASELINE 验收记录（2026-07-30 主控独立取证）**：

- **质量高，逐条给了 rustc 原始输出**（含 note/help），未压缩成结论。三处预测错误**全部实测复现且行号一字不差**：E0599 `reporter.rs:369`（rustc 直接提示只有 `from_static`/`from_owned`）、E0369 `hotkey.rs:124`（`CGEventType does not implement PartialEq`）、E0599 `hotkey.rs:257`
- **✅ 校准了审计头条结论**：`docs/MACOS-BRANCH-AUDIT.md` 称「唯一阻塞是 `reporter.rs:369`」，实测主程序 **4 个独特错误**、src-tauri **7 个**。主控派发前的预判（低估 2 项）得到实证
- **🔴 最大发现：查出 Windows 侧本次改动引入的真实回归**（主控已独立复核 `git show 292eeb0 -- src-tauri/Cargo.toml` 确认）。Windows 侧把 `[target.'cfg(target_os = "windows")'.dependencies]` 段头**插在了 `[dependencies]` 表中间**，按 TOML 语义，排在其后的 `tokio-tungstenite` / `futures-util` / `rustls` **三个依赖被静默划为 Windows 专属** → macOS 上 `qwen3.rs` + `main.rs:170` 共 7 个错误。**在 Windows 上 cfg 命中、三者照常解析、`cargo check` 0 errors —— 该回归在 Windows 侧完全不可见**，其提交信息里「src-tauri 0 errors」为真却拦不住它。**且不止编译问题**：`rustls` 那行是 BUG-QWEN3-CRYPTO-001 的 ring provider 修复，被划成 Windows-only 等于 macOS 侧丢了这个 TLS 修复
- **tester-1 的根因诊断正确且指出了审计的方法盲区**：审计方法是扫源码 `#[cfg]` 分支，而本次漂移发生在**依赖清单层**，源码里没有 cfg 可扫。这是 DEC-034 机制的新变种（「依赖层 cfg 与代码层 cfg 同步缺口」），已记入 troubleshooting
- **另查出既有问题（非本次引入，主控查 git log 确认）**：`src/main.rs:4349` 的 `#[cfg(test)]` 块无条件 import 了被 `cfg(windows)` 门控的 `select_preprocessing_params`（E0432），只影响 macOS 上 `cargo test`，不阻塞 release 构建
- **红线遵守确认**：零文件改动（`git status --porcelain` 与开工时一致）、未出包、未 `npm install`（`package-lock.json` 零改动）、未 `cargo clean`、未用 git 破坏性命令
- **诚信记录（正面）**：主动披露了 `env-macos.sh` 的 `BASH_SOURCE` bug 与自己的手工绕过方式，没有把「手工设了变量」隐去当成脚本可用

**FIX-COMPILE 验收记录（2026-07-30 主控独立取证）**：

- **三处改动 Read 逐一核对，全部正确**：`from_owned(font_data)` 且**连带的 `.ok().unwrap_or_default()` 已删除**（这是最易漏的一处，Gavin 特别确认过）；`matches!(event_type, CGEventType::KeyDown)`；`.map_err(|_| anyhow!(...))`
- **主控独立复跑取证（未采信汇报）**：`cargo check` **0 errors** 属实；`cargo check --all-targets` 只剩 `main.rs:4349` 那一个既有 E0432，**未引入任何新错误**
- **Windows 侧零影响已验证**：`reporter.rs:347` 的 `include_bytes!("C:/Windows/Fonts/msyh.ttc")` 仍在原处，`git diff` 中 `msyh.ttc` 命中数 **0** —— DEC-034 与 DEC-033 第 4 条硬约束遵守
- **⚠️ 一处越界（判定为可接受，不回滚）**：`src/llm/mod.rs` 出现 +3/−2 改动，位于 `mod tests`（起于 `:1473`）内，系 `build_optimize_request` 调用由两行压成一行。按 `[FMT-COLLATERAL-001]` 三步法定性：**去空白后 md5 逐字节相同**（`1e3970f6…`），零逻辑改动，属 `cargo fmt` 全量连带，保留不回滚
- **⚠️ 但任务书要求的是 `cargo fmt -- <两个文件>`，仍出现全量连带** —— 这是同一指令第二次未被遵守（上一次见 07-27 TEST-SYNC 验收记录）。后续派发需改为更强的措辞或改由主控自行 fmt
- **CHANGELOG.md +2**：两项任务的完成记录，属 worker-guide 规定的完成联动流程，非越界

**边界评估（已做，供派发时直接用）**：三项任务文件级零重叠——PR1 只碰 `scripts/`，BASELINE 零改动，FIX 只碰 `src/crash/reporter.rs` + `src/platform/macos/hotkey.rs`。PR1 与 BASELINE 可并行，唯一注意点是 PR1 不得在 BASELINE 运行期间改动 `env-macos.sh` 内容（tester-1 要 source 它）。

**已知非阻塞遗留**：`src-tauri/gen/schemas/macOS-schema.json` 为 Tauri 生成物、当前未跟踪且未被 `.gitignore` 覆盖，需判断是入库还是加 ignore。


> **2026-07-30 主控取证结论：本节原列的三个批次已全部闭环，无进行中的代码任务。** 三个 Worker（coder-1 / coder-2 / tester-1）当前空闲待派发。历史验收记录保留在下方备查。

### 2026-07-29 · MACOS-COMPAT-001 双平台兼容适配（A 阶段实施，Gavin 指令派发）

> 依据：**DEC-033**（本日新增）｜ 前置研究：RESEARCH-MACOS-DUALPLATFORM-001（已验收）
> **最高红线（Gavin 两次重申）**：「代码重构，不得影响任何 windows 的代码功能」——列为两份任务书的第一验收标准
> 分工边界（DEC-033）：本侧负责共享代码 + Windows 专用代码 + **接缝（平台契约）**；macOS 专用功能由 macOS 侧 Agent 团队后续开发

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| MACOS-COMPAT-001-CORE | 主程序侧接缝适配四项：① `mod hotkey`/`mod injection` 加 `#[cfg(windows)]`（**全仓零引用，实证**）② `crash/mod.rs:99` `get_windows_version` cfg 隔离 + 非 Win 等签名版本（**调用点 `:74` 一行不改**）③ **`platform/mod.rs` glob 导出改显式清单 + 契约注释块**（核心）④ macOS 侧补 `notify_config_changed` / `capture_scene_signals` stub（带 `TODO(macOS team)`） | `src/main.rs` + `src/crash/mod.rs` + `src/platform/**` | coder-1 | 🔄 已派发（已 ACK，核读源码中） |
| MACOS-COMPAT-001-TAURI | Tauri 侧 cfg 隔离**三项**：`src-tauri/Cargo.toml` windows 依赖挪 target 段 ｜ `check_hotkey_available` cfg 隔离 ｜ `overlay.rs:39` `.transparent(true)` **cfg 拆链**（不启用 `macos-private-api`，避免动共享 `tauri.conf.json`）。**+ sherpa-onnx 获取脚本**（Windows 侧，见下方重新定位） | `src-tauri/**` + `scripts/**` | coder-2 | 🔄 进行中（**范围已变更，见下**） |

**⚠️ 范围变更（2026-07-29 Gavin 指令「目前暂未考虑使用 github 的 CI/CD，还是用本地平台构建发布」→ DEC-033 附则二）**：

- **取消 B-1**（解除 `.gitignore` 的 `.github/` 排除）→ `.gitignore` 不动
- **取消 B-2**（重写双平台 CI workflow）→ `.github/` 一字不改，现有陈旧 workflow 保持未入库
- **B-3 保留但重新定位**：sherpa-onnx 获取脚本继续做，目的从「喂 CI」改为「**解决全新 checkout 构建不了的既有问题**」——这是 macOS 团队能否起步的前置条件，与 CI 无关。按「给新开发者用的一键获取脚本」设计（PowerShell，UTF-8 **with BOM**）
- 已通过 tmux 实时通知 coder-2，要求若已动 `.gitignore`/`.github/` 则用 edit 工具还原（**禁止 git checkout--/restore**）

**主控已向 Gavin 声明的风险（决策以 Gavin 为准，此处仅存档）**：无 CI 状态下「Windows 改动破坏 macOS」与「macOS 改动破坏 Windows」**都不会在提交时暴露**（`#[cfg]` 切掉的代码编译器不做类型检查，trait 亦无法约束）。替代防线只剩：显式导出清单 → 契约注释 → **交接纪律** → 两侧各自本地 `cargo check`（防不住对侧，属固有缺口）。因此 `docs/MACOS-HANDOFF.md` 的「改导出面必须同步更新两份清单」条款，从"建议"升级为**唯一可执行的防线**，权重显著提高。

**边界评估**：两任务文件级**零重叠**（`src/**` vs `src-tauri/**`+CI），可并行。与已冻结的 FIX-COT-LEAK-001-P0（`src/llm/mod.rs` + `src-tauri/src/llm.rs`）亦零重叠。

### 交付路径（Gavin 2026-07-29：「等你这边代码重构完毕，提交后，我让 macOS 那边团队 checkout 开始接收开发」）

**「重构完毕」的验收口径 = 对方 checkout 后能真正接手，不是我们本机编译过就算。** 五步串行：

| 步 | 内容 | 负责人 | 状态 |
| --- | --- | --- | --- |
| 1 | MACOS-COMPAT-001-CORE + TAURI 代码完成 | coder-1 / coder-2 | ✅ **已交付** |
| 2 | 主控验收：Read 实际改动逐条核 + **编译实证** | orchestrator | ✅ **已通过**（详见下方验收记录） |
| 3 | **TEST-SYNC + TEST-EXEC 全量回归**——本批次红线是「Windows 零行为改动」 | tester-1 | ✅ **已验收**（TEST-SYNC 评估无需新增；主程序 686/0/8 + Tauri 53/0/0；主控 `cargo test -- --list` 独立计数 694 自洽） |
| 4 | **交接文档** `docs/MACOS-HANDOFF.md`（242 行，六块内容） | coder-1 | ✅ **已验收**（主控修正两处引用错误：导出清单实际在 `:61`；`build-macos.sh` 已入库） |
| 5 | git commit + push | orchestrator | ✅ **已完成**（`292eeb0`，14 文件 +730/−26 → `3882236..292eeb0`；零 token 残留） |

**后续增补（2026-07-30）**：AUDIT-MACOS-BRANCH-001（coder-1，纯审计零改动）→ `docs/MACOS-BRANCH-AUDIT.md` + `collab/research/macos-branch-audit-001.md`，15 处 cfg 分支 / **P0×1**（`src/crash/reporter.rs:369` 调用 `egui 0.29.1` 不存在的 `FontData::from_bytes`，macOS 必然编译失败，修复方向 `from_owned`）/ P1×8（含主控此前未知的 `main.rs:3419-3475 mod macos_stubs` 空实现）/ P2×4 / P3×2。已提交 `2c98976`，交由 macOS 团队据此做 Phase 3 实现。

**主控验收记录（2026-07-29~30，步 2）**：

- **编译实证（本批次最关键，Worker 侧均未能自证）**：主控独立跑通
  - `cargo check` → **0 errors**，4m43s，86 warnings 且**无一条指向 `src/platform/**` 或 `src/crash/**`**（main.rs 的 warning 集中在 1191 行 Win32 GDI 既有问题，远离改动的第 5-12 行）
  - `cargo check --manifest-path src-tauri/Cargo.toml` → **0 errors**，11 个既有 warning（全在 wordbook 未使用函数）
  - 意义：`platform/mod.rs` 的 15 符号显式导出清单**无遗漏**——这是本批次唯一靠编译器兜底的改动，漏一个符号即编译失败
- **coder-1 四项 Read 核验通过**：`mod hotkey`/`mod injection` 仅加 cfg 未删除（符合要求）；`get_windows_version` 拆两版**调用点 `:74` 确未改动**；契约注释块五要素齐全且含主控给的「名称+arity 相同、类型平台化」stub 原则；两个 stub 均带 `TODO(macOS team)` 且注明 `usize` 语义留给 macOS 团队定
- **coder-2 A 组三项通过**：`.gitignore` 与 `.github/` 经 `git diff` 确认**一字未动**（附则二合规）；`overlay.rs` 采用 cfg 拆链，Windows 路径调用序列不变
- **⚠️ 一处越界（已核为无害，保留不回滚）**：coder-2 对 `src-tauri` 整个 crate 跑了 `cargo fmt`（任务书要求只格式化改动文件），连带改动 `config.rs`(+1/−4) / `qwen3.rs`(+5/−4) / `version_check.rs`(+3/−10)。**主控逐 hunk 读完全部 diff，numstat 与 hunk 数完全对应、无遗漏**，全部为 rustfmt 重排（闭包体加花括号、去尾逗号、长行合并）。注：去空白 md5 比对不一致是因 rustfmt 增删了 `{}` 与 `,` 这类 token，非逻辑改动；按 [FMT-COLLATERAL-001] 惯例保留
- **环境阻塞已排除**：两个 Worker 均被 [CT2-SUBMODULE-DEADLOCK-001] 阻塞，主控定位根因并修复（详见 troubleshooting）。**coder-1 的"网络问题"归因错误**，其提出的两个方案（跳过验证 / 等网络恢复）均无法解决问题——该构建树属永不自愈状态

**⚠️ 必须先让 Gavin 知悉的交接缺口**：即便本批次全部完成，**macOS 团队 checkout 后仍然构建不了**，原因是本仓库既有的两个基础设施问题（非本批次引入）：

1. **sherpa-onnx 预编译库未入库**（`.gitignore:22` 排除，`git ls-files` 实证为 0）
2. **`.cargo/config.toml` 把 `SHERPA_ONNX_LIB_DIR` 硬编码为本机 Windows 绝对路径**，且按最高红线不得修改

**解法（不构成死锁）**：BUILD-MACOS.md §一 表明 macOS 侧同事本机**已写好** `scripts/setup-macos.sh` + `scripts/env-macos.sh`（前者自动拉取 osx-arm64 预编译包，后者用 `[env]` 的 `force=false` 语义 export 覆盖）。**这两个脚本应作为 macOS 团队的第一个 PR 提交** —— 需在 `MACOS-HANDOFF.md` 中显式写明这一交接约定，否则对方会以为环境已就绪。我方对应提供 Windows 侧获取脚本（coder-2 任务 B-3）。

**主控核心设计决策（写入任务书）**：

- **平台相关类型差异刻意不统一**（`create_controller_window` 返回 `Result<HWND>` vs `Result<()>`、`FocusedTextSnapshot.hwnd` 为 `HWND` vs `usize`）。统一需改 `src/injection/mod.rs` 与 `platform/windows/*` 等 Windows 已交付路径，违反最高红线。改为在契约注释中显式标注 + 调用方必须在 cfg 分支内使用 + CI 兜底
- **不用 trait 做抽象**：trait 只约束当前编译目标上的实现，防不住被 cfg 切掉那一侧的漂移（✅官方 Rust Reference：cfg 为假的项从 AST 移除，不做类型检查）。**glob 改显式清单**的价值在于——漏列会**立即响亮编译失败**，而非静默行为漂移
- **Windows CI job 是保护我们自己的门**：DEC-033 下 macOS 团队会改共享代码，没有 Windows job 就没有反向防护。故它必需且必须能真绿

**⚠️ 派发前主控新发现的基础设施缺口（决定 CI 能否绿）**：

**sherpa-onnx 预编译库根本没入库** —— `git check-ignore -v` 实证 `.gitignore:22` 的 `/vendor/sherpa-onnx/*-Release/` 排除了它，`git ls-files sherpa-onnx-lib/` 为 **0**，`git ls-files vendor/` 只有 16 个文件且全属 `vendor/cmake/` crate 源码。**含义：任何平台的全新 checkout 都构建不了**，不只是 macOS——本机能构建纯粹因为磁盘上有那份未入库目录。叠加 `.cargo/config.toml` 把 `SHERPA_ONNX_LIB_DIR` 硬编码到 `D:\Workspace\...`，CI runner 上 `sherpa-onnx-sys` build.rs 必然 panic（BUILD-MACOS.md §三 记录了 macOS 侧遇到的同一 panic）。

**这条同时修正了 coder-2 报告 §4.3「Windows .dll/.lib 已入库」的错误结论**（验收记录第四处修正）。可用杠杆：cargo `[env]` 默认 `force=false`，CI 里 export 真实路径即可覆盖，**无需改 `.cargo/config.toml`**。已授权 coder-2 在无法可靠完成时**如实降级**（两 job 均先 `continue-on-error` + 拆出独立后续任务），明确"如实降级 > 硬凑跑不通的 CI"。

### 2026-07-29 · RESEARCH-MACOS-DUALPLATFORM-001 双平台单仓库可行性评估（Gavin 指令）

> 来源：Gavin 请 macOS 侧开发人员出具两份评估报告 → 要求评估「不影响 Windows 任何功能的前提下重构，实现一套代码 + 一个 GitHub 仓库 + 双平台并行开发」
> 输入文档：`docs/MACOS-PORT-ASSESSMENT.md`（195 行）、`docs/BUILD-MACOS.md`（120 行），均为 untracked

**主控已核实的仓库事实（派发时写入任务书）**：

1. **macOS 侧环境改造全部滞留对方本机**：`scripts/setup-macos.sh` / `scripts/env-macos.sh` 本仓库**不存在**（BUILD-MACOS.md §一 却要求执行它们）；`scripts/build-macos.sh` 是 2026-04-19 的 394 字节占位；§六 声称已追加的 `.gitignore` 项 `vendor/sherpa-onnx/*-shared-lib` 本仓库**没有**
2. 本地 == `origin/main` == `3882236`，远端仅 main 一个分支，对方零提交
3. 报告头部引用「对应决策 `collab/decisions.md` DEC-033」，但本仓库 **decisions.md 只到 DEC-032，DEC-033 不存在**
4. 主控复核属实的三条：`.gitignore:80` 确有 `.github/`（CI 从未进仓库）｜`.cargo/config.toml` 仅 3 行且 `[env]` 硬编码 Windows 绝对路径｜`src/platform/mod.rs` 确为 glob 导出无 trait，`create_hotkey_listener` 两侧签名不同
5. `src/main.rs:8,10` 的 `mod hotkey;` / `mod injection;` 确无 cfg 隔离

**报告核心数据**（待 coder-2 抽查复核）：构建环境已打通（324 crate 含 CTranslate2/sherpa-onnx 全在 arm64 编过）；剩 20 个源码错误（主程序 16 / Tauri 3 / crash-reporter 1）；`src/` 24,504 行中约 70% 天然跨平台；633 个 `#[test]` 绝大部分可直接在 macOS 跑；**平台层签名漂移已实际发生 6 处**（从未触发编译错误，因两侧从未同时编译）

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| RESEARCH-MACOS-DUALPLATFORM-001 | **纯研究零改动**：报告可信度抽查复核 ｜ 改动按「Windows 零影响 / 行为等价 / 有回归风险」三档分级 ｜ 签名漂移的真正防线（trait 能否解决 vs 双平台 CI）｜ 单仓库工程约定（package-lock 冲突 / vendor 二进制 / 分支策略）｜ 最小可提交批次与 GO-NO-GO 结论 | 无（产出 `collab/research/macos-dualplatform-refactor-001.md`，370 行） | coder-2 | ✅ **已验收（含主控三处修正，其中一处推翻核心成本结论）** |

**RESEARCH-MACOS-DUALPLATFORM-001 验收记录（2026-07-29 主控独立取证）**：

- **边界合格**：`git status` 仅两份 macOS 报告 untracked，源文件/`.gitignore`/`.cargo/config.toml` 零改动；`result.md` 1774 字节非空
- **✅ 主控复核属实的部分**：§2 五项 P0 阻断、§7 六行签名漂移、`src/` 24504 行 / `main.rs` 4416 行、`accessibility.rs:43-48` 确为 stub、`.github/workflows/build-macos.yml` 确实存在（816 B，2026-04-19）但被 `.gitignore:80` 排除从未进 git
- **❌ 主控修正一 · CI 成本结论基于错误前提（最重要）**：报告 §3.4 全篇按**私有仓库**计费——「macOS $0.062/分钟、约 Linux 10.3 倍、GitHub Free 2000 分钟/月按倍率折算」，并据此设计了「若 CI 成本不可接受的退而求其次方案」四选项。**但 `Cdexs/Feiyin-IME` 是公开仓库**——主控实测 `api.github.com/repos/Cdexs/Feiyin-IME` 返回 `"private": false` / `"visibility": "public"`（HTTP 200 免鉴权可读即为佐证）。**标准 GitHub-hosted runner（含 macOS）对公开仓库免费且不限量**（✅官方：billing 文档「The use of standard GitHub-hosted runners is free: In public repositories」；仅 `-large`/`-xlarge` 大型 runner 收费）。**结论反转**：成本不构成约束，"退而求其次"整节不适用，可直接上 push + PR 双触发，甚至跑完整 `cargo test`。真实约束只剩**单 job 6 小时上限**与 **cache 10GB 上限**
- **❌ 主控修正二 · 根 `node_modules` 的 gitignore 判断错误**：报告 §4.2 称「`.gitignore` 未显式忽略根 `node_modules/`」。实际 `.gitignore:12` 有**裸 `node_modules` 模式**，git 在任意层级匹配同名目录。主控实证 `git check-ignore -v node_modules` → `.gitignore:12:node_modules	node_modules`。故建议里「追加 `/node_modules/` 防误提交」是**冗余项**（"删除根 node_modules 历史遗留"的建议本身仍成立）
- **❌ 主控修正三 · `#[test]` 计数「差异 2」是自身 grep 口径问题**：报告用 `^\s*#\[test\]` 得 631 并标 ⚠️ 质疑原报告的 633。主控用 `grep -rn "#\[test\]" src/` 得 **633**，与原报告**完全一致**。原报告此项无误，不应标 ⚠️
- **一处核验不完整（不阻塞）**：§1.2 第 2 行 `read_text_from_hwnd` 报告自承「`src/platform/macos/injection.rs` 需要单独确认」，但仍标了 ✅属实。属证据略超前于结论，结论方向不影响
- **Q3 两个核心判断经官方文档验证成立**：cargo `[env]` 不支持 per-target（✅官方 Cargo Book）；`force=false` 时 shell 已有变量优先（✅官方）；`#[cfg]` 切掉的代码从 AST 移除、不做类型检查（✅官方 Rust Reference）→ **trait 不能防签名漂移，双平台 CI 是唯一可靠防线**，主控判断被确认

**主控预判的三个难点**（写入任务书供 Worker 验证或推翻）：

- **`.cargo/config.toml` 的 `[env]` 是唯一必须触碰 Windows 构建路径的改动**。cargo 的 `[env]` 不支持 per-target 区分，需在「移除改走自动下载 / build.rs / Windows 侧也改 env 脚本」间取舍，每个候选都要给 Windows 侧验证方法
- **trait 抽象不能防签名漂移**：cfg 切掉的代码编译器不做类型检查，trait 只约束被编译的那一侧。若此判断成立，**双平台 CI 是唯一可靠防线** → `.github/` 必须先解除 gitignore。需量化 macOS runner 上 CTranslate2 源码编译耗时与 Actions 额度
- **overlay trait 抽象（报告 §10-1）属高风险档**，为未交付平台重写已交付的 Windows GDI 关键路径，建议本轮不做

### 2026-07-29 · LLM-COT-LEAK-001 思维链泄漏致输出异常（Gavin 端测发现）

> 来源：Gavin 2026-07-29 `-debug` 端测反馈「语音输入后输出异常，疑似 LLM 优化后有问题」→ 主控日志取证分析
> 诊断记录：`troubleshooting.md [LLM-COT-LEAK-001]`、`logs/20260729.md`
> 证据：`target/release/debug.log` 12:06:27 与 13:19:15 两次异常

**根因链（主控独立取证）**：

1. **上游触发**：Gavin 于 11:13 把模型从 `Qwen/Qwen3.5-35B-A3B` 换为 `deepseek-v4-flash`（endpoint `api.deepseek.com`）。该模型把思维链写进 `content`。**Qwen 88 次请求 0 异常，v4-flash 15 次请求 2 异常（13%）**
2. **`enable_thinking:false` 对 DeepSeek 端点无效**（`llm/mod.rs:286/369/521`）：该参数属 SiliconFlow/Qwen 系，DeepSeek 官方 API 不识别，静默忽略 → DEC-008「关推理模式」在此 endpoint 上从未生效
3. **疑似 `max_tokens=512` 被思维链吃光**，真答案无预算输出（两次异常耗时 6.4s/6.6s vs 正常 3.6s）。**此条为强推断，尚未证实**——见第 5 条
4. **`extract_corrected_tag`（`llm/mod.rs:1064-1079`）取首个标签对**（`text.find`），思维链里的模板占位 `<corrected>...</corrected>` 劫持了真答案 → 注入 `"..."`（13:19 案例，整句全丢）
5. **FMT-EMPTY-CORRECTED-001 护栏漏网**（`llm/mod.rs:213-225`）：只挡「结果为空」与「残留字面量标签」两种，`"..."` 两者都不沾。12:06 之所以被挡住纯属偶然——那段思维链无闭合标签对，走兜底分支残留了字面量标签才触发护栏
6. **可观测性缺口**：`ChatResponse`（`llm/mod.rs:33` 附近）只解析 `choices`，**无 `finish_reason` / `usage`** → 截断与否无法从日志判定

**两次异常的实际表现**：

| 时间 | 输入 | 注入结果 | 护栏 |
| --- | --- | --- | --- |
| 12:06:27 | 我以的生命之书阿卡西记录真的存在吗 | 本地标点兜底（LLM 优化失效但原文完整） | ✅ 挡住 |
| 13:19:15 | gpu又分为八核和十核…最高可达35% | **`...`（整句全丢）** | ❌ 漏网 |

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| RESEARCH-DEEPSEEK-THINKING-001 | **纯研究零改动**：查证 deepseek-v4-flash 身份、DeepSeek 官方是否支持关闭思维链及参数名、CoT 走 `content` 还是 `reasoning_content`、max_tokens 是否含 CoT token、finish_reason/usage 字段契约、未知参数处理策略；产出落地建议 | 无（产出 `collab/research/deepseek-thinking-control-001.md`，504 行） | coder-1 | ✅ **已验收（推翻主控一处判断 + 主控回补一处）** |

**RESEARCH-DEEPSEEK-THINKING-001 验收记录（2026-07-29 主控独立取证）**：

- **边界合格**：`git status` 仅两份 macOS 报告 untracked，源文件零改动；`result.md` 3006 字节非空（[COLLAB-WRITE-001] 未复发）
- **API key 零泄漏**：主控取 config.toml 中真实 key 反向 grep `collab/research/` `collab/outbox/` `handoffs.md` `logs/`，零命中
- **✅ coder-1 推翻主控判断一处（正确，主控采纳）**：主控原判「模型把思维链写进 `content`」**不成立**。DeepSeek CoT 走**独立字段 `reasoning_content`**（官方文档 + 实测双证）。真实链路是——`max_tokens=512` 被 CoT 吃光 → `content=""` → **我们自己的 `extract_text`（`src/llm/mod.rs:962-993`）在 content 空时错误回落到 `reasoning_content`**，把 CoT 当答案返回。**这是我们自己的 bug，不是模型的**
- **✅ coder-1 补出主控漏掉的第 4 处注入点**：`src-tauri/src/llm.rs:23/86` 同样发 `enable_thinking: Some(false)`，主控任务书只列了主程序三处。主控复核属实
- **❌ 主控回补 coder-1 一处推测错误**：报告 Q3（第 152 行）推测 CoT「很可能被 `flatten_multiline` 或 `strip_fabricated_*` 压成 `...`」——**不成立**。主控用日志定论：13:19 那次 `suggestions after_tag` 日志**存在**（`debug.log:4416`），而该日志只在 `parse_suggestions_after_corrected_tag` 内打印，该函数只在 `extract_corrected_tag` 返回 `Some` 的分支被调用（`mod.rs:1005-1007`）→ **证明 `extract_corrected_tag` 成功返回了 `"..."`**。真正的第 4 环是它用 `text.find` 取**首个**标签对，抓走了 CoT 里的模板占位 `<corrected>...</corrected>`，与后处理层无关
- **两次异常的分支差异已由日志定论**：12:06 **无** after_tag 日志（`debug.log:4226-4228`）→ `extract_corrected_tag` 返回 `None` → 走无标签兜底分支 → 整段 CoT 含字面量标签 → FMT-EMPTY-CORRECTED-001 护栏拦下；13:19 **有** after_tag 日志 → 取到占位 `"..."` → 护栏三项判据全不沾 → 注入。**同一根因链，只因 CoT 里有没有闭合标签对而结局不同**
- **✅官方标注质量合格**（上一轮的返工点未复发）：Q1/Q2/Q3/Q4/Q5 的 ✅官方 条目均附 URL + 原文引用；7 次实测均附脱敏命令与响应关键字段；`content_filter` 实测未触发、Anthropic 兼容层冲突两项**主动降级为 ⚠️/❌未证实** 并列入附录 C

**合并后的完整根因链（主控定论，五环）**：

1. `enable_thinking: false` 对 DeepSeek 无效（✅实测静默忽略）→ 思维链默认开启
2. CoT 与答案共享 `max_tokens=512` 预算，CoT 优先消耗（✅实测 TEST 4/13）→ `content=""`，`finish_reason="length"`
3. `extract_text`（`mod.rs:962-993`）content 空时回落 `reasoning_content` → **CoT 被当成答案**
4. `extract_corrected_tag`（`mod.rs:1064-1079`）用 `text.find` 取**首对**标签 → 抓到 CoT 里的模板占位 → `"..."`
5. FMT-EMPTY-CORRECTED-001 护栏（`mod.rs:213-225`）只挡「空」与「残留字面量标签」→ `"..."` 漏网 → 注入用户输入框
| FIX-COT-LEAK-001-P0 | 代码加固**五项**（研究结论落地，对应根因链五环）：**P0-1** 请求体双发字段——保留 `enable_thinking`（SiliconFlow/Qwen3 用）+ 新增 `thinking:{"type":"disabled"}`（DeepSeek 官方参数），**4 处**注入点 `src/llm/mod.rs:286/369/521` + `src-tauri/src/llm.rs:86` ｜ **P0-2** `extract_text` 移除「content 空回落 `reasoning_content`」逻辑（字段保留解析，仅供日志）｜ **P0-3** `extract_corrected_tag` 改取**最后一对**标签（`rfind`）｜ **P0-4** 护栏补合理性校验（结果相对输入异常萎缩判格式失败，**须防误伤 F1 去语气词等合法压缩**）｜ **P0-5** `ChatResponse` 补 `finish_reason` + `usage`（含 `completion_tokens_details.reasoning_tokens`）并打日志 | `src/llm/mod.rs` + `src-tauri/src/llm.rs` | coder-1 | ✅ **已完成并已提交 `ff492ef`**（2026-07-30 解冻后实施；含主控两处修正 + 判据 B 设计反转为「只观测不拒绝」；695/0/8 + Tauri 53/0/0）｜**⚠️ 未出包 → 旧 exe 仍 100% 复现，见下** |

**✅ 已实施（2026-07-30，Gavin 解冻后指令实施）**：五项全部落地，详见 `logs/20260730.md`，提交 `ff492ef`（2 文件 +326/−29）。

**🔴 但缺陷在端测中仍 100% 复现 —— 因为未出包（2026-07-30 12:40/12:41 主控日志实证）**：

- 运行实例 PID 8592 用的是 **07-28 18:42 的旧 exe**，不含 `ff492ef`
- 两次听写（12:40:23 / 12:41:22）的注入内容均为 DeepSeek 思维链整段（1946 / 1927 字符，「我们分析用户输入：…」），与 [LLM-COT-LEAK-001] 根因链完全一致
- **本次两例走的是「有闭合标签对」以外的第三种形态**：`extract_text` 回落 `reasoning_content` 后 CoT 直接成为 optimize 结果，护栏三项判据全不沾（非空、无字面量标签、非纯标点）
- **结论：这不是新 bug，是已修复但未交付。唯一解法是出包**（版本号与范围两项待 Gavin 拍板，见文档顶部）

**⚠️ 冻结期间的现实影响（Gavin 已知悉）**：当前 LLM 配置为 `deepseek-v4-flash`，实测约每 7 次请求有 1 次触发该缺陷——白等约 6.4s 后回退本地标点（LLM 优化失效但原文完整），其中少数情况会向输入框注入 `...` 致整句丢失。**规避方式**：若冻结期较长，可在设置里把模型换回 `Qwen/Qwen3.5-35B-A3B`（同日志内 88 次请求零异常），属纯配置改动、零代码风险。

**解冻后的实施要点**：五项对应根因链五环，缺一不可：

- **P0-1 治本**（关掉思维链，DeepSeek 延迟 6.4s → 3.6s，token 费用同降）。**双发是为零副作用**——若只发 `thinking` 而删掉 `enable_thinking`，SiliconFlow/Qwen3 用户的思维链会重新开启，属回归；实测证明 DeepSeek 对未知字段静默忽略（不 400），双发安全
- **P0-2/P0-3/P0-4 是防御纵深**，与模型选择无关。即便换回 Qwen，这三处缺陷仍潜伏——任何模型只要 `content` 为空、或在输出前复述一遍带占位的模板，就会复现
- **P0-5 补可观测性**，12:06 那类「LLM 优化失效」今后可在日志直接看出是 `length` 截断 / 超时 / `content_filter`

**⚠️ 报告附录 C 的未证实项（实施时必须评估）**：用户若填 **Anthropic 兼容 endpoint**，双发的 `thinking` 字段可能与 Anthropic 自有 `thinking` 参数语义冲突（❌未证实）。实施前需决定是否加 guard。

### 2026-07-27 批次 · Gavin 端测四项（v0.7.1 后）

> 来源：Gavin 2026-07-27 `-debug` 端测反馈 4 项 + 主控日志取证分析
> 诊断记录：`troubleshooting.md [SCENE-OBSERVABILITY-001]`、`logs/20260727.md`
> 边界：文件级零重叠；版本号不动；③ 拆两侧协同（coder-2 改符号，coder-1 改 prompt 保护）

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| SCENE-OBS-001 | 场景感知运行时日志（方案 C）：采集后打 exe+kind+multiline_safe+f4_injected（不打窗口标题）；F4 块整段单独打印绕开 200 字符截断 | `src/main.rs` + `src/llm/mod.rs` | coder-1 | ✅ **已验收** |
| LANG-MIXED-001 | 中日/中韩夹杂被强行译成中文：新增假名/谚文探针；`script_instruction` 措辞收紧为「只统一中文字形、不翻译非中文」；混合文本跳过 zhconv | `src/text_normalizer.rs` + `src/llm/mod.rs` | coder-1 | ✅ **已验收** |
| ITN-CELSIUS-002-PROMPT | LLM 改写回汉字单位的保护条款（**必须运行时追加，不能只改 i18n 默认 prompt**——存量 config 已持久化） | `src/llm/mod.rs` | coder-1 | ✅ **已验收** |
| ASR-NOSPEECH-FILTER-001 | 空语音返回 `<\|nospeech\|>` 被注入：转录出口剥离 `<\|...\|>` 全族 token，剥离后为空走空转录路径 | `src/transcription/mod.rs` | coder-2 | ✅ **已验收（含主控一处修正）** |
| ITN-CELSIUS-002-SYMBOL | 摄氏度符号 `°C`(U+00B0+C) → `℃`(U+2103) 单字符；**单说「度」不转**（Gavin 2026-07-27 拍板：角度/温度同形，强转会把「转九十度」变「转90℃」） | `src/itn.rs` | coder-2 | ✅ **已验收** |
| TEST-SYNC-20260727 | 测试同步：P0 补 6 条边界 + P1 补 2 条日文汉字零改变断言 + P2 无旧措辞绑定 + P3 前端无需同步 | 各 `#[cfg(test)]` | tester-1 | ✅ **已验收** |
| TEST-EXEC-20260727 | 全量回归：Step 1 672/0/8 PASS、Step 1b 53/0/0 PASS、Step 2/3/4 SKIP；**未出包** | — | tester-1 | ✅ **已验收** |

**Gavin 确认要点（2026-07-27）**：
- ③ 实际现象为「返回汉字摄氏度」，判定主因是 LLM 优化阶段把 ITN 已规整的符号改写回中文 → 主修在 prompt 保护，符号统一为 ℃ 为辅
- ② 端测环境为 Claude Desktop；`Claude.exe` 已在 `scene-rules.toml:86` chat/AI-agent 词表内，另两次分类为 browser/ide_terminal 是否误判**待 SCENE-OBS-001 日志落地后复核**

**主控验收记录（2026-07-27）**：
- **coder-1 三项全部 Read 核验通过**：翻译路径回归护栏落实到位——`main.rs:3011` 确实改调 `script_instruction_for_translate`、`:3063` optimize 路径保持原函数，两者分流正确；6 条护栏单测用反向断言 `!contains("不要翻译")` 锁死回归；探针码区正确（假名 U+3040-309F/U+30A0-30FF、谚文三区）；两处 zhconv 均加跳过；ITN 保护条款为模块级 const 且在 `llm/mod.rs:336`(翻译)+`:451`(optimize) 两条运行时路径追加，**未碰 `src/i18n.rs`**（符合"存量 config 已持久化、改默认无效"的关键约束）；场景日志字段不含 `window_title`，隐私红线遵守
- **coder-2 两项通过，主控修正一处缺陷**：`strip_asr_special_tokens` 的 `pending_space` 对 CJK 也补空格，`你<|NEUTRAL|>好` → `你 好`，中文凭空多出空格且会直接注入用户输入框；**该缺陷还被写进断言当作预期值**（`mixed_with_text` 期望 `"你 好"`），属于把 bug 固化成设计。已收紧为「两侧均 ASCII 字母数字才补空格」（英文 `hello<|X|>world` → `hello world` 防粘连行为保留），断言改为 `你好`，新增 2 条护栏单测；主控独立 `cargo check --tests` 0 errors
- **未跑 cargo test**（遵守 orchestrator 不亲自执行测试的边界），全量回归归入 TEST-EXEC

**衍生发现（未立项，待 Gavin 拍板）**：`should_translate_for_language`（`main.rs:3202`）判据为 `contains_han`，而日文汉字与中文汉字同码区 → 日文文本被判为"已是中文"，`target=Chinese` 时**翻译热键对日文会静默跳过**。属 LANG-AUTO-001 既有语义遗留（非本批次引入），但中日混合成为明确需求后该缺口更突出，与 todo「等 Gavin 拍板 · 翻译方向是否改双向全自动」同源。

**TEST-SYNC 验收记录（2026-07-27 主控）**：
- **主控修正三要素逐一核验完好**（本次派发时特别提醒不得覆盖）：补空格条件的 `is_ascii_alphanumeric` 双侧判断在位、`mixed_with_text` 断言仍为 `你好`、两条主控护栏单测均存在
- **P1 断言方向正确**（最关键）：断言 `contains("龍")` / `contains("亞")` 保持不变，而非断言转换结果——且选字精准，龍/亞 都是 zhconv 必转字，一旦跳过逻辑失效测试立即变红
- **P0 覆盖到位**：`strip_asr_special_tokens` 系列累计 15 条（coder-2 原 7 + 主控 2 + tester-1 补 6），新增覆盖 token 串首/串尾/已有空格/全角标点相邻/连续 token 夹 ASCII/空输入
- **生产代码零改动确认**：关键生产函数逐一复核完好；主控独立 `cargo check --tests` 0 errors
- **一处 cargo fmt 连带**：`src/wordbook/db.rs` 出现 1 处改动（`mod tests` 内 `assert_eq!` 多行压单行，token 逐字相同），按 [FMT-COLLATERAL-001] 定性为**零逻辑改动的格式化连带**，保留不回滚。提醒：任务书要求 `cargo fmt -- <file>`，本次仍出现全量连带

**TEST-EXEC 验收记录（2026-07-27 主控独立取证）**：
- **数字链自洽**：672 = coder-1 交付基线 662 + tester-1 TEST-SYNC 补 8 + 主控修正补 2，三方独立来源相加严丝合缝
- **主控针对性抽查 4 组共 30 条全绿**（未只信汇报表格，按 [TESTER-FABRICATED-REPORT-001] 强制取证）：`strip_asr_special_tokens` 15/15（数量正好等于逐条核对的 7+2+6）、`translate_path` 8/8、`keeps_japanese_kanji` 2/2、`temp_celsius` 5/5
- **result.md 2213 字节非空**且内容对应本任务（[COLLAB-WRITE-001] 0 字节问题未复发）
- **边界遵守确认**：未出包、未同步 Publish、未改生产代码、版本号文件未动

**批次状态**：**五项代码 + TEST-SYNC + TEST-EXEC + 版本号 + 出包 + git 提交全部闭环**（2026-07-28 收口）。

**收口记录（2026-07-28 Gavin 指令「commit / 查版本号 / 出包」→ 主控独立取证）**：
1. **git commit** —— 代码提交 `155b595`（07-27 20:24，11 文件 +835/-38，含三处版本号 bump）已在上次 session 完成；本次补提交 `fb230f9`（CHANGELOG 出包记录 + `src-tauri/Cargo.lock` 0.7.1→0.7.2 构建期传播）。工作区 clean；**两个提交已按 Gavin 指令 push 到 GitHub**（`9540335..fb230f9`），push 后已恢复 clean remote URL，`.git/config` 零 token 残留，`git status -sb` 确认 `main...origin/main` 完全同步
2. **版本号** —— 已是 **v0.7.2**：`Cargo.toml` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` 三处均 0.7.2，产物 ProductVersion 0.7.2.0（UI 侧 0.7.2）
3. **出包** —— 已于 07-27 20:31 完成，**本次未重复构建**（源码自 20:02 后零改动，重建产物无功能差异，遵守「不要频繁出包」）。主控取证：三处 sha256 全一致（feiyin-ime `7fbb1e4b` / feiyin-ime-ui `0d76eca1` / crash-reporter `559c7506`）、itn-rules + scene-rules 三副本一致、产物 mtime 20:28~20:31 晚于全部源码 mtime 与代码提交时间、UI exe 内嵌当前 dist 资产（`index-BNQZfcUG.css` / `index-CTgGziQm.js` 各命中 1）、运行实例 PID 18548 指向 `target/release/feiyin-ime.exe` 且 Responding=True

**遗留小项**：~~`Publish/voice-ime-ui.exe` 旧包名死文件~~ → **已按 Gavin 指令删除（2026-07-28，10,013,184B）**。删除前核实生产路径 `src/main.rs:436~478` 七处全部使用 `feiyin-ime-ui.exe`，旧名仅存在于非生产文件中（见下表），运行时零影响。

**删除时发现的陈旧引用（未处理，待排期）**：

| 文件 | 问题 | 影响 |
| --- | --- | --- |
| `build.bat:26` / `scripts/init-publish.ps1:112` | 仍按旧名 `voice-ime.exe` / `voice-ime-ui.exe` 清理与拷贝产物 | 这两个脚本若被执行会产出错误的 Publish 清单（现出包走 build-guide 三步流程，未走这两个脚本，故未暴露） |
| `src/main - 副本.rs` | 早期 `main.rs` 的手工副本，含 mojibake 注释，451~498 行全是旧名 | 未被 cargo 编译（非模块），纯仓库垃圾文件，建议删除 |
| `tests/ui_guard_tests.rs` | GUARD-001/002 骨架测试注释与路径拼接用旧名 | 均为未实现的骨架（无真实断言），不影响测试结果 |

**端测重点（v0.7.2 已可端测）**：① 中日/中韩混合语句看日韩文是否原样保留 ② 说「今天三十摄氏度」看是否输出 `30℃`（LLM 改写主因判定目前仍是推断，只有真实 LLM 调用能证实）③ 空语音看是否不再注入 `<\|nospeech\|>` ④ 看 debug.log 的 `Scene context:` 行确认场景分类是否误判

> **2026-07-25 批次已全部闭环并出包**（详见 CHANGELOG / progress）：
> - **WORDBOOK-SCHEMA-FIX-001**（P0 词库全瘫）：CORE + UI 均已验收，**Gavin 端测目视确认词库页列出 5 条，P0 闭环**
> - **WORDBOOK-AUTOLEARN-FIX-001**（A+C+D）：CORE + TAURI + TEST-SYNC 均已验收
> - 出包：`b0c70b3` 提交 → 三 exe 19:38 重建同步 Publish，621/0 + 53/0 + 54/54，ProductVersion 0.7.1.0 不升版
>
> **⚠️ 自动学习效果仍待观察（非 bug，属机制固有）**：阈值维持 2（Gavin 决定不改），即**同一个词需被独立建议 2 次才入库**。故短期端测多半只见候选累积、不见入库，属预期行为。观察方法：`target/release/debug.log` 中 `suggestions after_tag` 非空率（修复前 13/96 ≈ 13.5%，A 生效应显著上升）与 `WORDBOOK-AUTOLEARN-FIX-001-C: rejected ...` 的原因分布（大量 `not_in_corrected_text` = LLM 仍返回错字侧；大量 `too_long_cjk` = 8 字上限偏严，改一行 const 即可调）。想快速验证闭环可对同一个 ASR 老出错的词连说两次。

### ~~WORDBOOK-AUTOLEARN-FIX-001~~（已闭环，保留实施记录备查）

> 诊断依据：`troubleshooting.md` [WORDBOOK-AUTOLEARN-001]（实测 102 次 LLM 请求：触发率仅 13.5%、57 候选 count 全为 1、0 次入库；链路本身通，有 `艾丁湖` 成功入库实证）
> Gavin 决策：A+C+D 实施 ｜ **B 阈值不改、不加 UI 复核** ｜ **E 多库并存不收口（DEC-032）**

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| WORDBOOK-AUTOLEARN-FIX-001-CORE | A 解 prompt 冲突（SUGGESTION_INSTRUCTION 加 OVERRIDES 覆盖声明，破解用户 config 中"strictly prohibited: adding suggestions"条款）+ C 入库前过滤（**核心判别法：建议词必须出现在纠正后正文中**，零词典剔除 ASR 错字侧同时保留日常生活词汇；配结构性过滤换行/长度/句读/纯数字/中文单字 + 拒绝原因日志）+ D 主程序侧默认 prompt 旧格式修正（3 处 ZH/ZH-Hant/EN） | `src/llm/mod.rs` + `src/i18n.rs` | coder-1 | ✅ **已验收（含主控一处修正）** |

**CORE 验收记录（2026-07-25 主控）**：
- Read 实际改动确认：A 的 OVERRIDES 声明点名三类在先禁令（`adding your own suggestions` / `thoughts regarding corrections` / `prefix/suffix output`）且限定"仅作用于这一行 JSON、其余禁令继续有效"，含 machine-readable protocol line 定性 + 日常高频词显式在范围内 + `风无星→风无心` 真实正反例；C 的七条过滤规则齐备、`MAX_CJK_CHARS=8`/`MAX_TOTAL_CHARS=24` 具名 const 双限同查、**铁律遵守**（入库存 `word.to_string()` 原形，归一化仅用于比较且注释写明）、每条拒绝均有带原因的 `log::info!`；两个返回分支 + translate 路径均正确传入 `<corrected>` 正文
- **主控修正一处缺陷**：`has_sentence_punct` 黑名单误含 ASCII 撇号 `'`(U+0027)，与该函数自身注释（声称撇号放行、`O'Brien` 不应误杀）自相矛盾——实测 `O'Brien`/`don't`/`it's` 被拒，而弯撇号 `’`(U+2019) 反而放行，行为不一致。英文所有格与缩写属 Gavin 明确要求收录的日常用语，已移除撇号条目并去掉 `"`/`'` 重复项，注释补修正说明
- 独立复核：`cargo check` 0 errors；`cargo test` **606 passed / 0 failed**（含主控修正后重跑）；14 条 `fix001_*` 单测全绿；D 两侧段落 **md5 逐字节一致**（`cc7c1e58...`），旧格式与 correction pair 措辞双侧零残留
- **cargo fmt 连带**：`build.rs` / `src/bin/poc_{halluc,funasr_nano,vad}.rs` / `src/config/mod.rs` 5 文件系 `cargo fmt` 全量格式化连带，已用三步法（去空白 md5 → token diff → 去逗号 md5）逐一证明**零逻辑改动**，保留不回滚，详见 troubleshooting [FMT-COLLATERAL-001]
| WORDBOOK-AUTOLEARN-FIX-001-TAURI | D Tauri 侧默认 prompt 旧格式镜像修正（3 处），措辞与 coder-1 侧对齐 | `src-tauri/src/i18n.rs` | coder-2 | ✅ **已验收**（2026-07-25：主控 Read 三处实际改动确认单词格式+收录范围含日常词汇+verbatim 约束就位，旧格式与 correction pair 措辞零残留，独立 `cargo check --manifest-path src-tauri/Cargo.toml` 0 errors，边界未越界；另纠正其日志中「旧格式导致解析失败」的错误因果表述——解析层两种格式皆兼容，D 的价值是统一口径而非修 bug） |

**边界**：两任务文件级零重叠，并行派发。**C 的范围修正**：主控原分析误把 `时代/吉他/惊心动魄` 归为"应过滤的通用词"，Gavin 纠正——日常生活词汇是高频词汇必须支持入库，过滤只针对垃圾（错字侧/整句/超长）。
| TEST-SYNC-WORDBOOK-AUTOLEARN-001 | 测试同步：P0 补主控撇号修正的测试护栏（`fix001_keeps_apostrophe_words` / `keeps_curly_apostrophe` / `rejects_ending_punct_variants` 新增 3 条 + `keeps_intra_word_connector` 扩展）+ P1 全测试面签名变更审计（零偏差）+ P2/P3 评估无需补 | `src/llm/mod.rs` `#[cfg(test)]` 块 | tester-1 | ✅ **已验收** |

**TEST-SYNC 验收记录（2026-07-25 主控）**：Read 三条新单测确认断言方向正确（`O'Brien`/`don't`/`it's` 断言**保留**且校验 len==3、弯撇号 `it’s` 单独覆盖、句末标点四变体循环断言**拒绝**——反向护栏防黑名单被删空）；4 条相关单测逐条实跑全 ok；**主控独立跑全量 `cargo test` 609 passed / 0 failed**（606+3 新增，数字链自洽）；`cargo check --tests` 0 errors；生产要素五项逐一复核确认 tester-1 未动生产代码（双 const / 黑名单无 U+0027 / 入库存原形 / OVERRIDES 声明 / 交叉校验分支全部在位，**主控的撇号修正与说明注释完好未被覆盖**）

**批次状态**：**A+C+D 代码 + 测试同步全部闭环**。下游：TEST-EXEC（全量回归 + 三步构建 + Publish 同步）等 Gavin 下「现在可以出包」指令后派发。
**注意**：本修复的实际效果**必须端测验证**——A 的 prompt 覆盖能否把触发率从 13.5% 拉起来，只有真实 LLM 调用能证明，单测无法覆盖。建议出包后观察 `debug.log` 中 `suggestions after_tag` 非空率与 `WORDBOOK-AUTOLEARN-FIX-001-C: rejected` 日志分布。

---

## TEST-EXEC-043（2026-08-17，tester-1）— 阶段四回归 + 消融实测
- [x] Step1 cargo test 全量 1040/0/11（主967+crash37+集成36，精确命中）
- [x] Step2 Vitest 54 passed
- [x] Step3 src-tauri 55 passed
- [x] Step4 pytest SKIP（Publish/ 为 BUILD-017 旧包早于本批）
- [x] 消融前基线 overlay_043 专项 10/10
- [x] 消融 A（delta.abs()→delta）：4 条变红（含预期外 step_never_exceeds_quarter_of_delta，主控裁决实测为准）
- [x] 消融 B（门闩→false）：仅真值表 1 条变红
- [x] 还原自证：git diff -w 空 + 还原后复跑 1040/0/11
- [x] 文档同步（logs/CHANGELOG/handoffs/MACOS-HANDOFF/本文件）
- [ ] 主控验收

## BUILD-018（2026-08-17，tester-1）— 阶段五出包 OVERLAY-043 全批进 exe
- [x] Step1 杀进程（含 Gavin 端测实例 PID 21948，先报主控获授权）
- [x] Step2 npm build + Tauri UI（custom-protocol + cp）
- [x] Step3 主程序 release
- [x] Step4 同步 Publish/ + toml 三副本（config.toml 未覆盖）
- [x] ① 六 exe 时间戳 ② sha256 两副本 ③ toml 三副本 ④ ProductVersion 0.8.0
- [x] ⑤ 探针（正向 1 命中 + 间接证据三件套）
- [x] ⑥ 大小对照（feiyin-ime +3,072 / ui 0 / crash 0）
- [x] ⑦ 冒烟 PID 28300 Responding=True 无 panic 测后清理
- [x] result.md + 五文档同步
- [ ] 主控验收 + Gavin 端测（流畅度/本地模型/波形/切态 四项）


---

# 📦 2026-09-10 归档批次 · todo.md 全量归档（RELEASE-210 后精简）

> **归档原因**：todo.md 长到 3026 行 ≈ 69K token，每次 session 启动全量加载，
> 与 CodeLab CLAUDE.md「启动必读文档合计 ≤ ~20K token」的约束严重冲突（实测启动必读合计 ≈228K）。
> **本次把归档前的 todo.md 全文原样搬入本文件（零删改）**，todo.md 重写为只含待办的精简版。
> 精简版里每条待办都指回本批次的对应小节，细节与取证一条未丢。

以下为归档前 todo.md 全文（3026 行，截至 2026-09-10 RELEASE-210）：

---

# 任务列表 · voice-ime

## ✅ 2026-09-10 — v0.9.0 已发布（RELEASE-210）

tag `v0.9.0` + GitHub Release 已上线：https://github.com/Cdexs/Feiyin-IME/releases/tag/v0.9.0
main 已推至 `9520a58`，本地与远端同步。

### 本次发布的遗留待办

| # | 项 | 说明 |
| --- | --- | --- |
| 1 | **README 正文更新** | `README.md` / `README.en.md` 核心功能表仍是 v0.6 时代（写着 SenseVoice 本地识别，无流式实时上屏 / 场景感知 / 在线 FunASR / 录音中编辑）。本次只改了版本徽章。建议随下一批派 coder 重写功能表 + 截图 |
| 2 | **Release 挂安装包** | 本次 Release 未挂二进制。`docs/../Publish/voice-ime.iss` 是 Inno Setup 脚本但未构建安装包。需要对外分发时：tester-1 出 release 包 → 构建 setup.exe → upload asset |
| 3 | `Publish/models` 缺 performance 模型 | 主控 21:38 查得 `Publish/models/sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17` 不在（目录 mtime 09-10 21:27），而 `config.toml` 是 `asr_model="performance"` ⇒ 跑 Publish 版本本地识别会直接报错。根目录与 `target/release/` 下都还在。**等 Gavin 确认是否手动删除**，确认后从 `target/release/models/` 拷回 |

## 🔴 2026-09-10 — 提示词优化三单（Gavin 拍板「开单」，DEC-059 管辖）

**Gavin 指令**：① 等提示词优化后再出包 ② 开单 ③ 等优化开发完成再 push。
⇒ 已攒 commit（`141fd8c` F3 注入、`979ffd7` 多行授权）**暂不出包不 push**，与本三单一起走。

### 实测基线（临时探针渲染真实 prompt，测完已删）

| 场景 | 字符 | ≈token |
| --- | --- | --- |
| agent / 终端（`multiline_safe=false`） | 14,628 | ~3,600 |
| 微信（最小） | 13,684 | ~3,367 |
| **Word / notepad / doc（`ml=true`）** | **25,123** | **~6,140** |

**来源占比**：`config.toml` 用户基座 1,184（8%）+ `scene-rules.toml` 场景 1,151（8%）
+ **Rust `const` 硬编码 12,293（84%）**。

**前缀缓存实测**：同 `ml` 不同场景命中 9,427 B（64%）；跨 `ml` 仅 4,050 B（28%）。

**分段体量（agent, ml=F）**：`f3_lists` 4,094（ml=T 时 **13,789**）/ `L0_1_FIDELITY` 2,421
/ `suggestion` 1,511 / `user_base` 1,184 / `scene_f4` 1,151 / `number_symbol` 810 / 其余 < 600。

---

### PROMPT-OPT-202 · L3 内调序（f3_lists 提到 scene_f4 之前）

| 项 | 内容 |
| --- | --- |
| 影响文件 | `src/llm/mod.rs` `build_prompt_layers` L3 装配段**唯一** |
| 改动 | 两个 `l3.push` 交换顺序；**零字符内容变更** |
| 收益 | 同 `ml` 不同场景的公共前缀 9,427 → **13,521 B（64%→92%）** |
| 风险 | 极低 —— 同层内并列，且 `META_RULE_PRECEDENCE` 明写优先级 `ABSOLUTE and OVERRIDES position` |
| 依据 | 按「稳定性递减」排：`f3_lists` 仅 4 种取值（ml × punct），`scene_f4` 有 9 种场景 |
| 验收 | 🔴 **量化护栏**：断言两场景 prompt 公共前缀 ≥ 13,000 B（直接断言要的属性，不是结构锚）+ 全量绿 |
| 备注 | 翻译路径已在 TRANS-F3-201 排好（F3 在 scene 前），本单是主路径补齐 |

### ~~PROMPT-OPT-203 · `suggestion` 词库学习指令条件注入~~ 🔴 **已撤单（2026-09-10 取证）**

> **撤单理由**：① 全 config 无「关闭自动学习」开关（只有 `auto_learn_threshold: u32`，
> 且 `==0` 被两处代码重置为默认值 2 ⇒ 0 是非法值兜底非关闭语义），找不到可用条件；
> ② 🔴 **按「有无词库」条件化会自锁** —— 无词库⇒不注入指令⇒LLM 不建议新词⇒永远学不到
> 第一个词⇒永远无词库，**新用户永久失去自动学习能力**（开单时未想到的致命缺陷）；
> ③ 对 Gavin 零收益 —— 实测 `Publish/wordbook.sqlite` 有 1 条词，条件恒成立照样全量注入。
> **替代**：唯一可行方向是精简 `SUGGESTION_INSTRUCTION` 内容本身，属内容改动，
> 风险与 204 同类、同受 DEC-059 管辖，**应并入 204 一起走，不单独立单**。

<details><summary>原方案（存档）</summary>


| 项 | 内容 |
| --- | --- |
| 影响文件 | `src/llm/mod.rs` `build_prompt_layers` L2 装配段 + `build_translate_system_content` |
| 现状 | `suggestion`（1,511 chars）**无条件 `l2.push`**；而旁边 `wordbook_block` 是条件的（无词库返回 `None`） ⇒ **没词库时「怎么学词库」的指令照发**，纯浪费 ~10% |
| 改动 | 与 `wordbook_block` 同源判定：无词库/未开自动学习时不注入 |
| 🔴 风险 | 条件判错会让**词库自动学习静默失效**（WORDBOOK-AUTOLEARN 系列历史坑）。必须先查清「自动学习开关」的真实判据，不得只看 wordbook 是否为空 |
| 验收 | 有词库 → 指令在场；无词库 → 不在场且 `suggestions` 解析链路不报错；全量绿 + 词库学习端测 |

</details>

### PROMPT-OPT-204 · `f3_lists` 精简（最大单块）

| 项 | 内容 |
| --- | --- |
| 影响文件 | `src/llm/mod.rs` `f3_rules_text` / `INLINE_SEPARATOR_RULES(_NO_PUNCT)` |
| 现状 | ml=T 时 **13,789 chars = doc 场景总量 55%**；ml=F 时 4,094 |
| 🔴 风险 | **最高** —— F3 + DEC-060 核心规则，直接决定列表化行为。刚被 Gavin 端测打过（TRANS-F3-201/B），此刻动它风险叠加 |
| 🔴 DEC-059 硬要求 | **真实 API A/B 实证**（固定样本、改前改后对照、贴原始输出），不接受静态论证 |
| 建议 | **排最后做**，且 202/203 出包端测稳定后再启动。A/B 方式待 Gavin 定：主控用 config 里的 key 跑（花钱）/ 或 Gavin 端测充当 A/B |

### 三单执行顺序（串行，同文件不并行）

`202`（最安全，收益明确）→ `203`（中风险）→ `204`（最高风险，需 A/B 授权）。
三单均改 `src/llm/mod.rs` **同一文件**，禁止并行。

## ✅ 2026-09-10 — VERBOSE-195 正式写作场景冗余压缩（Gavin 拍板并扩容，**已执行，待端测**）

> 🔴 **2026-09-10 Gavin 拍板扩容，推翻本单原「doc 不动」取舍**：
> 「除了 chat 类，其他都算正式写作场景，在去除结巴字词、重复字句、纠正字词后都要进行冗余内容的优化简化」。
> ⇒ 范围从 agent+terminal **扩到 email / doc / browser 全部**，只留社交 chat 两组不动。
>
> **已执行**（纯词表，免构建，重启即生效）：7 组 style 追加 CONDENSE 条款 + agent 组 `+ZCode.exe`。
> 三副本 md5 `fb5a54ca0a85186a14ae6f98ed59df65`，TOML 解析 PASS，社交组 CONDENSE 零命中。
> 详见 `logs/20260910.md` 与 CHANGELOG `VERBOSE-195`。
>
> ### 🔴 本单遗留（全需 Rust 改动，等 Worker 额度恢复）
>
> | # | 项 | 说明 |
> | --- | --- | --- |
> | 1 | ~~pi desktop exe 名~~ | ✅ **已解决**（VERBOSE-195-B，2026-09-10 联网取证）：两个上游项目都自称 pi desktop，四条全录 —— `Pi Desktop.exe`/`pi-desktop.exe`（FaqFirebase/pi-desktop，Electron productName）+ `Pi Agent.exe`/`pi-agent-desktop.exe`（abcwyc/pi-agent-desktop，Tauri productName）。🔴 非本机核实，待端测看 `Scene context: app_exe=` 日志坐实 |
> | 2 | ✅ ~~翻译路径补 scene + 用户基座~~ **已完成 TRANS-SCENE-197**（2026-09-10，主控代做，1119P/0F，消融双证，待出包端测） | `optimize_and_translate`(`main.rs:8783`) 只传 4 参，`scene_context`/`multiline_safe` 就在 `:8717` 同作用域没传；`build_translate_system_content`(`llm/mod.rs:997`) 签名无 scene 位。⇒ **开翻译时 VERBOSE-195 全部失效** |
> | 3 | ✅ ~~翻译路径补 multiline_safe~~ **已完成 TRANS-SAFE-196**（2026-09-10，主控代做，1115P/0F，消融双证，待出包端测） | `llm/mod.rs:898` 自陈绕过 `try_once`⇒ 不走 `flatten_multiline`(`:903`)。终端/vim(`multiline_safe=false`)开翻译可能吃到换行被当命令执行。**推自代码未实测**，需一次端测坐实 |
> | 4 | ✅ ~~agent 从 chat kind 拆出~~ **已完成 AGENT-KIND-198**（2026-09-10，主控代做，1122P/0F，消融实证）。🔴 因新增 kind，**必须随包出**不得热更 toml（旧 exe 读新 kind 会落 Unknown 静默失效） | `scene/mod.rs:350-353` F4 首句拼 `typing into a {kind} application` ⇒ agent 场景实际写「typing into a **chat** application」，与 style 自相矛盾且反向拉口语。本次 style 内 `NOTE` 治标，根治需 `SceneKind::Agent` + 全量回归 |
> | 5 | **翻译版 L0-1(A) 保真底线** | 既有裁定(`llm/mod.rs:53`)「L0 不注入翻译路径」本身对（L0-1 要求语义单元原样出现，与翻译相悖），但副作用=翻译路径**完全无保真底线**：`UNIT_SYMBOL_PROTECTION_TRANSLATE` 只护数字单位，否定/情态/限定词/主命题全裸奔。照该常量先例做翻译版 L0-1(A) |
> | 6 | CONDENSE 条款重复 7 份 | 运行时只注入 1 份（一次只匹配一个场景）⇒ **token 成本不变**，代价是**维护性**：改一处要改 7 处（ide_terminal 两处同串已是既有坑）。根治=代码侧按 kind 统一注入或 toml 加共享段 |
> | 7 | browser 组已知局限 | 场景按 exe 匹配，浏览器 exe 就是 chrome.exe，**无法区分网页内是微博/评论（社交）还是文档编辑**。本次按 Gavin「其他都算正式」一并加了压缩。此为既有架构局限非本次引入 |


**Gavin 原话**：「除了聊天、社交类软件之外，比如 agent、terminal 等正式应用软件中，
格式化输出没有优化掉输入文本中的冗余、重复的字句（词语），这需要通过系统提示词来优化」。

### 取证结论：不是「规则缺失」，是许可被三重锁死

`src/llm/mod.rs:283-286` 的 L0-1(B) **早就写了**允许删：
`YOU MAY REMOVE OR CONDENSE … verbatim repetition and redundant restatement of the same point`。
没生效的原因是三条叠加：

| # | 位置 | 原文 | 锁死机制 |
| --- | --- | --- | --- |
| 1 | `scene-rules.toml:95`（agent） | `Keep the intent and wording intact.` | L3 明令保持措辞。与 L0-1(B) **形式上不冲突**（一个「可以删」一个「保持原样」）⇒ 层级优先级 `META_RULE_PRECEDENCE:266` 判不了，模型自然听具体指令 = 不删 |
| 2 | `scene-rules.toml:200` **和 `:265`**（ide_terminal，两处同串） | `Technical style. … No pleasantries.` | 一个字没提冗余。「No pleasantries」= 不要客套，≠ 删重复。L0-1(B) 是 **MAY** 不是 MUST，无 L3 驱动 ⇒ 默认不行使 |
| 3 | `src/llm/mod.rs:299-301` | `TIE-BREAKER: if unsure … KEEP IT` | 口语冗余天然是灰色地带（强调？澄清？啰嗦？），一犹豫就保留 |

### 落点必须是 L3 不是 L0（硬理由）

- **爆炸半径**：L0 全场景吃。把 MAY 升 MUST 会连微信/QQ 一起压缩，正是 Gavin 排除的场景。
- **架构自证**：`src/llm/mod.rs:294-296` 红字明写
  `🔴 WHETHER to polish at all is decided ONLY by the scene style rule in layer L3, never here`
  ⇒ 本方案不是发明新机制，走的是它自己指定的那条路。
- L0-1 的 (A) 与 TIE-BREAKER **一字不动**（DEC-060 保真底线）。

### 改动清单（三处，全在 scene-rules.toml，零 Rust 改动）

🔴 **ide_terminal 是两处同串（`:200` + `:265`）—— 只改一处 = 半失效**（`feedback_no_patch_design` 的坑）。

每条新增措辞自带「刹车」子句 `brevity NEVER outranks completeness`，
钉死不得越界到 L0-1(A)（数字/单位/否定/情态/限定词/路径/标识符）——
这是「修 A 且不带出 B」，不是把风险二选一递给 Gavin。

🔴 措辞**不写 `L0-1(B)` 跨层引用**，自带完整语义 —— 同 `src/llm/mod.rs:52-53`
对 `UNIT_SYMBOL_PROTECTION_TRANSLATE` 的判例（悬空引用有 PROMPT-ARCH-020 同因风险，
且 toml 是数据文件、可被用户副本覆盖，写死层号更脆）。

### 交付形态：免构建、免出包 ✅（已核实，非记忆）

`src/scene/mod.rs:284-305` = 运行时**优先读 exe 同级 `scene-rules.toml`**，
失败才回落 `include_str!` 内置（`:14`）。⇒ 改三副本 + 重启即可端测。
三副本现 md5 一致 `d3356ef8ba8f7683a5447012b4807cba`（根 / Publish/ / target/release/），
无 `[TOML-ALL-NUL-001]` 污染。

🔴 **三个 Worker 全额度耗尽期间，这是少数能推进的任务形态**（改数据文件，非 Rust 生产代码）。

### 卡点：DEC-059 要求真实 API A/B 实证

提示词改动完成判据 = 固定样本改前改后对照 + 贴原始输出，不接受静态论证。
Worker 全挂 ⇒ 合成 A/B 做不了。**建议改由 Gavin 端测充当 A/B**：
同一句话在改前包/改后各口述一遍，对照上屏结果 —— 比合成样本更有说服力。

### 主控声明的取舍（Gavin 可一句话推翻）

- `doc` 场景（`:323` 正式书面语，Word 等）**本单不动** —— Gavin 只点名 agent + terminal。
  书面语场景压缩收益可能更大，但属未经要求的扩大，不默认执行。
- `chat`（`:26`/`:79`）、`email`、`browser` 一律不动 —— Gavin 明确排除聊天社交类。

## 🔴 2026-09-10 会话状态：**三个 Worker 全部额度耗尽，本轮无法派发任何任务**

coder-1 / coder-2（`GLM-5.3-Flash · OpenCode Go`）与 tester-1（`deepseek-v4-flash · OpenCode Go`）
末行均为 `monthly usage limit reached. It will reset in 24 days 10 hours`。
**是 provider 级（OpenCode Go）月度限额，不是单档耗尽** —— 换模型名无效（tester-1 已实证）。
Gavin 确认：「额度已用完，需要等重置」。

⇒ 主控本轮**不重启、不切档、不派发**，下方所有待办**原样保留**，等额度重置后接着做。
⇒ 完整判据与处置见 `troubleshooting.md [WORKER-RESTART-MODEL-RESET-001]` 复发记录四。
⇒ 原「遗留待办 #4 tester-1 故障处置（`Bad Request: deepseek-v4-flash`）」**已定性**：
   同一条账户额度问题，非模型 API 故障，无需单独排查。

## ✅ 2026-09-08 本批已收官 —— BUILD-193 Gavin 端测全过

**产物** `Publish/feiyin-ime.exe` @13:44:56 sha `d081c742…`，HEAD `8280927`。
Gavin 原话「端侧全部通过！」（含一票否决项「无新增闪烁」）。

| 任务 | 内容 | 状态 |
| --- | --- | --- |
| `EDITICON-187/187-ALT/190` | 编辑态图标 B→A2 | ✅ 已提交 `fa4416a` |
| `ESC-188` + `STREAMFONT-189` | ESC 陈旧位加固 + 上屏字号 16 | ✅ 已提交 `9933a40` |
| `DIAG-191` + `FIX-192` | 右侧空白根因 H5 + 结构修复 | ✅ 已提交 `8280927` |
| `BUILD-193` | 出包 + 端测 | ✅ 通过 |

### 🔴 本批遗留待办（按优先级）

| # | 项 | 说明 |
| --- | --- | --- |
| 1 | **是否 push** | 本地领先 origin/main 十余个提交，🔴 **等 Gavin 明确指示**，不得自动 |
| 2 | `TEST-SYNC-194` FIX-192 顺序护栏 | 钉死「先扩窗再建 EDIT」，防以后改回去。派空闲 coder（非作者 coder-1） |
| 3 | 出包核验加第八项 | 随包数据文件**内容级**校验（与根目录 hash 比对）。`[TOML-ALL-NUL-001]` 证明只核 exe 不够，且**只比大小检不出来** |
| 4 | tester-1 故障处置 | 模型 API `Bad Request: deepseek-v4-flash`，出包后未交报告。重启前必读 `[WORKER-RESTART-MODEL-RESET-001]` |
| 5 | 重审受污染的端测结论 | `[TOML-ALL-NUL-001]` 期间场景感知/ITN 静默失效，此前基于那些包的结论需重看 |
| 6 | `OVERLAY-149-PROBE` 探针删除 | 10~11 处，圆角机制定案后 |
| 7 | ESC-178 H1 机制定案 | 消息路由层为何不响应仍未查明，轮询旁路是绕行不是根治。需带 debug.log 的端测 |

### 判据沉淀（本批新增，已写入 logs）

1. **字面量判别探针只对带日志文案的改动有效**。行为型改动（换序/改常量/加空读）不产生字符串，
   命中 0 属正常，须改用「源码 mtime < 产物 mtime」等新鲜度判据 —— 不得因命中 0 误判为未进包。
2. **查历史参数必须把 `logs/YYYYMMDD.md` 列入检索面**（本批主控漏查，误判「候选 A 参数彻底丢失」）。
3. **候选形态产物不得清理**：被挑选过的候选，其几何常量必须留在 `cfg(test)`，
   且文档中逐个数字列出 —— 只留一句文字描述等于丢失。
4. **风险不是请示的理由**：发现「修 A 可能带出 B」时，主控职责是设计出「修 A 且不带出 B」的方案
   并给出验证手段，不是把二选一递给 Gavin（详见 `.claude/tasks/lessons.md` 2026-09-08 条）。


## 🔄 2026-09-08 — STREAMFONT-189 实时上屏窗口字号 14→16（Gavin 要求，已派 coder-1 执行中）

**Gavin 原话**：「将 asr 文字实时上屏显示的窗口的字体也调大 1 号，和编辑态的那个字体字号同步一样大小，
因为昨天只调了编辑态的文字，实际上我希望实时上屏那个显示窗口中的文字也一样调大」。

⇒ 这是 `EDITFONT-183` commit 里预告的那个已知取舍的裁定：
「进出编辑态文字 14→16 视觉跳变……若 Gavin 不要跳变，两处常量合一即可」。**Gavin 选择不要跳变。**

| 项 | 内容 |
| --- | --- |
| Worker | coder-1（EDITFONT-183 作者，熟悉该区） |
| 排队情况 | 已解除 —— `ESC-188` 已验收，coder-1 空出，本单已于同日派发执行中 |
| 出包 | 本轮不出包（Gavin 攒批指令） |

### 🔴 产出源盘点（不是「一行」—— 14 喂了 6 处，只有 3 处该动）

| # | 产出源 | 位置 | 用途 | 本单 |
| --- | --- | --- | --- | --- |
| 1 | D2D `streaming_text_format` | `main.rs:4152-4163` | `placeholder_text(:5070)` 「请说话」+ `streaming_text(:5109)` **实时上屏文字** | ✅ 改 16 |
| 2 | GDI `cached_font` | `main.rs:2836-2840` | D2D 失败兜底路径，画同样这两个态 | ✅ 改（见下） |
| 3 | 测量字号 | `main.rs:6151-6159` | `adjust_overlay_pos_size_for_text` 算窗宽 | ✅ `RecordingWithText` 也归 16 |
| 4 | D2D `text_format`（雅黑 SEMI_BOLD） | `:4138` | `draw_processing_primitives(:4330)`「处理中」 | ❌ 不动 |
| 5 | D2D `centered_text_format` | `:4166` | 按钮箭头（`:4806/4855`）、Error/Info 小字 | ❌ 不动 |
| 6 | D2D `wrap_text_format` | `:4183` | 信息提示预览正文（`:5412`） | ❌ 不动 |

🔴 **第 3 条是正确性问题不是外观**：EDITFONT-183 已在编辑态踩过 ——
按 14 量宽、按 16 绘制 ⇒ 自动宽度低估约 14% ⇒ 更早触发 `ES_AUTOHSCROLL` 横向滚动
⇒ 看起来像「最右侧闪烁」（FIX-172-B）复发。实时上屏窗口同一机制，必须同步。

🔴 **第 2 条有实现风险**：GDI 侧全 overlay 共用一个 `cached_font`。要让流式文字走 16
而「处理中/箭头」留 14，需要第二个 HFONT。`main.rs:1340-1342` 已有先例
（`edit_font` 独立于 `cached_font`），但那里明确警告过生命周期：
「cached_font 在 … take()+DeleteObject()，复用会导致字体被提前删除或双删」。
新增字体必须照抄 `edit_font` 的生命周期范式，**这是本单唯一真风险点**。

### 主控已声明的取舍（Gavin 可一句话推翻）

「处理中」文案、按钮箭头、信息提示正文**维持 14 不动** —— Gavin 只点名了实时上屏窗口的文字。
若他要整个浮层统一放大，改法反而更简单（`OVERLAY_FONT_SIZE` 单个常量 -14→-16，
`OVERLAY_EDIT_FONT_SIZE` 随之冗余），但会连带改变按钮箭头字形大小与「处理中」版式，
属未经要求的视觉改动，故不默认执行。


## 🔴 2026-09-08 — Gavin 出包指令：攒批，本轮不出包

**Gavin 原话**：「这个图标修改完，先不出包，等后续还有修改再一起出」。
⇒ `EDITICON-187` / `ESC-188` 交付验收后**只 commit，不派 BUILD**，等 Gavin 后续改动攒够一起出。
⇒ tester-1 本轮无任务，保持待命。

## ✅ 2026-09-08 — ESC-188 编辑态入口排掉 ESC 陈旧转变位（✅ 已验收，待 commit）

| 项 | 内容 |
| --- | --- |
| 来源 | 主控验收 BUILD-186 时 Read 代码发现，非端测报障、非 coder-1 实现错误 |
| 影响文件 | `src/main.rs` **唯一**；与 coder-2 的 `menu_icons.rs`（EDITICON-187）零重叠，并行安全 |
| 改动 | `main.rs:1894` 进入编辑态前空读一次 `GetAsyncKeyState(VK_ESCAPE)` 排残留位，约 +5 行零删除 |
| 不碰 | `:2101` 轮询本体 / `:2172` FocusLost / ESC-178 三段日志链 / 任何重绘路径 |
| 出包 | 🔴 本轮不出包（见上节 Gavin 指令） |

**机制**：`0x0001` 是「自上次读取以来被按过」，没人读就一直留着。全仓仅 `:2102`（编辑态）
与 `:2172`（FocusLost）两个读者，两态之外无人清位 ⇒ 用户在别处按的 ESC 攒着 ⇒
进编辑态首个 tick 凭空命中 ⇒ 转写文本丢失。FocusLost 用同样写法没暴露，是因为它
本就该取消，误取消看不见；编辑态误取消 = **数据丢失形态**。

🔴 **诚实标注不确定性**：MSDN 对低位语义本身带保留（"you should not rely on this
last behavior"），故「陈旧位无限期累积」是**从代码推的机制假设，未运行时实测**。
但空读加固**无论假设成立与否都正确且无副作用**，成本一行，故不等实测先加。
端测判别手段已具备：误触发时只有 `ESC-178: streaming-editing ESC poll fired`，
**不会**伴随 `ESC-178: EDIT subclass received WM_KEYDOWN`。


## 🔴 2026-09-08 — EDITICON-187 编辑态图标换回候选 A（Gavin 端测定裁）

**Gavin 原话**：「将编辑态窗口的图标换成上次的A图标吧，实际使用下来觉得B不太合适」。形态偏好裁定，非 bug。

| 项 | 内容 |
| --- | --- |
| 任务 | `EDITICON-187`，派 coder-2（EDITICON 全链作者），已 dispatch，已 ACK |
| 影响文件 | `src/ui/menu_icons.rs` **唯一**；main.rs 零改动（三个函数签名不变，D2D `:4836` + GDI `:3780` 两路径自动吃新像素） |
| 改动 | 双文本线（`TEXT_LINE_1/2`）→ 单条下划线；铅笔几何 `PENCIL_*` 逐位不动；顺带订正 `:27` 过时注释 |
| 冲突评估 | coder-1 / tester-1 均空闲无在跑任务，零文件级重叠 |
| 流程 | 纯几何形态调整、运行时链路已由 Gavin 实际使用反证打通 ⇒ 走「交付即出包」，不进五阶段（同 OVERLAY-153/155 先例） |

### 🔴 本单暴露的一个真实损失：候选参数被清理后不可恢复

`EDITICON-185` 定稿 B 时「旧候选已清理」，而 184/185 被 squash 进同一提交 `fd527e0`
⇒ **git 里只有 B，候选 A 的几何常量彻底不存在了**。周备份最新 `20260901` 早于图标批次，
全仓 `find edit-{A,B,C}*.png` 零命中。故本单不是 revert，是**按 184 的文字描述重建 A**
（已知信息仅：「A 铅笔+下划线（生产实现，距分割线 7px 无粘连）」）。

**待固化判据（拟）**：被 Gavin 挑选过的**候选形态产物**（预览 PNG + 对应几何常量）
在该形态所属功能存活期间**不得清理** —— 选型是可逆决策，清理让它变成不可逆。
定稿后其余候选至少要把常量留在 `cfg(test)` 对照组里，或把预览留在 `docs/`
（`collab/drafts/` 被 gitignore 屏蔽，见本文件旧节）。


## 🔴 2026-09-08 — 主控验收 BUILD-186 时发现：ESC 轮询「陈旧转变位」误取消（待 Gavin 拍板是否修）

**结论先行**：ESC-178 的轮询旁路本身写得对，但它读的那个位会**在进入编辑态之前就攒着**，
可能导致刚进编辑态的瞬间凭空取消一次，用户看到的是「转写文字还没来得及编辑就没了」。

| 项 | 内容 |
| --- | --- |
| 位置 | `src/main.rs:2101-2107`（`StreamingEditing` 臂的 `GetAsyncKeyState(VK_ESCAPE)` + `& 0x0001`） |
| 机制 | `0x0001` 是「自上次读取以来被按过」的转变位，**未被读取就一直留着**。全仓只有两个读者：`:2102`（本次新增，编辑态）与 `:2172`（FocusLost 态）。两态之外无人读 ⇒ 用户在别处按的每一次 ESC 都把这个位留在那儿 |
| 后果 | 进入编辑态的第一个 tick 就命中 ⇒ 立刻发 `CancelRequested` ⇒ 编辑态秒退，转写文本丢失。用户没按任何键 |
| 触发条件 | 录音前的任意时刻按过 ESC（关弹窗、退全屏、IDE 取消补全都算），且期间没进过 FocusLost 态 |
| 为什么 FocusLost 没暴露过 | 同样的写法在 `:2172` 已在生产跑了很久 —— 但 FocusLost 本身语义就是「用户点走了、该取消」，多取消一次看不出来。编辑态不一样，**误取消 = 用户的转写结果凭空消失**，是数据丢失形态 |
| 现有可判别性 | 🔴 已具备，无需新工装：coder-1 的三段日志链正好能区分 —— 误触发时只会出现 `ESC-178: streaming-editing ESC poll fired`，**不会**伴随 `ESC-178: EDIT subclass received WM_KEYDOWN`。端测 debug.log 里这两行的共现关系即可定案 |
| 拟修法（一行，零风险） | 进入 `StreamingEditing` 时先空读一次 `GetAsyncKeyState(VK_ESCAPE)` 把陈旧位排掉（进入点 `main.rs:1896`）。不改轮询逻辑本体，不触碰重绘路径，回退 = 删这一行 |
| 状态 | ⏳ 待 Gavin 拍板：随下一批攒着改，还是等端测 debug.log 证实后再改 |

### 顺带发现（不影响功能，下次碰到该文件时捎带订正）

`src/ui/menu_icons.rs:27` 注释仍写「edit_icon_rgba 见本文件头部 EDITICON-182 实现（字体优先 + 几何兜底）」，
但 EDITICON-184 已把字体路线撤净，`:156-157` 实际是 `rasterize(size, edit_icon_covered)` 纯几何。
注释是过时残留，非缺陷。（已核验零字体依赖属实：全文件无 `CreateFont`/`DrawText`，唯一 `Segoe MDL2` 出现在 `:85` 齿轮设计参考注释里。）


## 🔴 2026-09-07 晚 — BUILD-159 端测打回（Gavin 四条，两条新回归）

**Gavin 端测 `Publish\feiyin-ime.exe`（18:14 包）结果：三个批次没有一个真正生效，另带出两条新 bug。**

| # | 现象 | 定位 | 处置 |
| --- | --- | --- | --- |
| 1 | 托盘菜单没有出现图标 | 未定（怀疑 `attach_menu_icons` 挂的 HMENU 不是 tray-icon crate 实际弹出的那个） | `DIAG-163` Q1（coder-1，只读） |
| 2 | 流式上屏窗麦克风图标无电波动效 | 未定（怀疑流式态未往 state 写 level ⇒ `mic_has_audio` 恒 false，或该态不在 `:1814/:1871` dirty 两态内） | `DIAG-163` Q2（coder-1，只读） |
| 3 | 🔴 **新回归**：切本地模型 → 再切在线 ASR/funasr 在线 → 按热键出的是本地频谱录音窗，不是流式上屏窗 | 未定（头号嫌疑 `aeaebe1` OVERLAY-155 动了 `draw_recording_overlay` 分支结构；「用过本地模型才复现」强烈指向残留状态，可能是老缺陷首次触发） | `DIAG-163` Q3（coder-1，只读，最高优先） |
| 4 | 🔴 **新回归**：上屏中进编辑态 → 整窗大黑屏、文字全丢 | **已定位** = `WS_EX_COMPOSITED`（EDIT-FLICKER-157），coder-2 交付时已预警未运行时实测并写好回退预案 | ✅ `FIX-162` 已回退并验收，**待出包** |

### 在跑

| 任务 | Worker | 内容 | 状态 |
| --- | --- | --- | --- |
| `BUILD-186` | tester-1 | 出包：ESC-178 + EDITFONT-183 + EDITICON-185 定稿（✅ 七项核验全 PASS，sha `6624cdd1…` 异于 `9d60f458…`，二进制字面量探针正反对照 PASS，cargo test 全量 1112P/0F；ESC 轮询旁路 + 编辑框 16px + 图标定稿候选 B） | ✅ **已交付，待主控验收/自测目视** |
| `ESC-178` / `EDITFONT-183` / `EDITICON-179~185` | coder-1/coder-2 | ESC 轮询旁路+日志链 / 编辑框 14→16px / 编辑图标全链（纸笔→字体路线→撤销→三候选→定稿候选 B） | ✅ **已验收提交** `fd527e0`，已进 BUILD-186 |
| `BUILD-177` | tester-1 | 出包：ESC-174 + EDITICON-175/176（✅ 七项核验全 PASS，sha `9d60f458…`；ESC 取消录入 + 铅笔图标+分割线） | ✅ **已交付**，已被 BUILD-186 覆盖 |

### 🔴 主控失职复盘（根因，待固化进出包判据）

**不是四个独立的低级错误，是同一个错误犯了四次：三个批次没有一个经过运行时目视就出包了。**

| 项 | 事实 |
| --- | --- |
| 1/2 的验收证据 | `cargo fmt/check/test` + 主控目视 **PNG 预览图**。PNG 是 `dump_menu_icons_preview` / 帧 dump 走**另一条代码路径**产出的，只证明「图形画得出来」，**不证明运行时挂载/绘制被执行到** —— 主控把前者当后者验收了 |
| 4 的验收证据 | coder-2 交付原文已标红「`WS_EX_COMPOSITED` 在 SLWA 分层窗子控件上未运行时实测……若见 EDIT 不显示/异常，回退 = …」。**风险是标红递到主控手上的，主控没拦，直接攒批出包** |
| 机制根因 | 主控把「验证成本与风险成正比」**用反了** —— 该条说的是别重复证已证过的，不是省掉唯一能证的那一次目视。而 `worker-guide` 第十三节「overlay 是 Win32+D2D 原生绘制，`cargo test`/Vitest/Browser Mode **一条都覆盖不到**，涉及浮层视觉必须在端测清单里列目视项」是主控自己写的，这三件事全在那条覆盖不到的线上 |
| 失效模式 | 与 `[DOC-STATE-DRIFT-001]` / `[SECRET-IN-REPO-001]` 同构：**规则写在通用条文里 = 读过就忘，只有写进「完成判据」逐行打钩的才会被执行** |
| 放大器 | 攒批出包（158+FIX+160+157 四改动一起上），现在必须先做归属判定才能分清谁带出了第 3 条 |

**待固化的新判据（拟）**：任何涉及托盘/浮层/原生窗口视觉的批次，**出包前主控必须自行启动 exe 逐条目视**，并在 BUILD 记录里留「运行时目视清单 + 逐项结论」；主控自己验不了的项，必须明写「本批无法自验，需 Gavin 端测」，**不得以 PNG/帧 dump 预览替代**。—— 待 Gavin 认可后写入 `worker-guide` 与 `.claude/tasks/lessons.md`。

---

## 🔴 2026-09-07 夜 — BUILD-165 出包（修 Gavin 端测 4 条中的 3 条）

**产物**：`Publish/feiyin-ime.exe` 12,290,560 B @19:40，sha `85c1cf26…`（异于上包 `b3043119…`），v0.9.0.0 未动，两副本一致。

| Gavin 第几条 | 现象 | 本包 | 任务 |
| --- | --- | --- | --- |
| 4 | 编辑态大黑屏、文字全丢 | ✅ 已修 | `FIX-162` 回退 `WS_EX_COMPOSITED` |
| 3 | 切模型后按热键出本地频谱窗 | ✅ 已修 | `FIX-164` A：空闲 tick 廉价层预热 + Start 前 1500ms 有界等待 |
| 2 | 麦克风无声波弧 | ✅ 已修 | `FIX-164` B：`MIC_PULSE_FULL_LEVEL` 0.35→0.10 + 可见地板 0.35 |
| 1 | 托盘菜单无图标 | 🔴 **未修** | `FIX-164` C：仅把静默降级换成带 `GetLastError` 的 `warn!`，等 Gavin 的 debug.log 定位 |

### 已交付验收

| 任务 | Worker | 状态 |
| --- | --- | --- |
| `FIX-162` | coder-2 | ✅ 验收通过（diff 恰两 hunk / grep=0 / 自证表属实） |
| `DIAG-163` | coder-1 | ✅ 验收通过（Q3 判为 DEC-025 老缺口而非本批回归，`git log -S` + hunk 范围三提交逐个排除；Q1 头号 HMENU 怀疑证伪；Q2 阈值 35 倍不对称硬事实） |
| `FIX-164` | coder-1 | ✅ 验收通过（主控 Read 核验：`asr_cheap_reload_needed` 零 IO / 静音三重门完好 / 预热仅 idle tick / `FIX-162` 零触碰 / Part C 四个失败分支全带 `GetLastError`） |
| `BUILD-165` | tester-1 | ✅ 六项核验全 PASS + 全量回归 1110P/0F/9I；`crash_reporter` config 批量 FAIL 未复现，归档偶发 |

### 主控自验（承诺的目视，如实交代做到哪一步）

- ✅ **硬判别探针**：新包命中 `FIX-164 D1 idle prewarm` / `menu icon: CreateDIBSection failed` / `menu icon: SetMenuItemInfoW failed`；PRE 基线包对同样 needle **零命中** ⇒ 新代码确实进了二进制。
- ✅ **日志可达性预检**：`main.rs:7942-7944` 文件日志 `filter_level(Debug)` ⇒ Part C 的 `debug!` 成功行会写进 `debug.log`（不会被级别过滤掉）。
- ✅ **触发时机预检**：`attach_menu_icons` 由 `show_tray_popup_menu` 调用 ⇒ 🔴 **不右键点开托盘菜单，日志一行都不会有**。此条决定 Gavin 端测步骤，已写进端测清单。
- 🔴 **未做到**：托盘菜单图标目视、说话时声波弧目视。原因：前者需在 Gavin 桌面右键点击托盘图标（合成输入会干扰他本人操作，且可能触发已知 tray-icon #298 冻结），后者需真实麦克风说话。**主控物理上验不了，如实声明，不以 PNG/帧 dump 冒充**（上一轮就是拿 PNG 当运行时验收才让 Gavin 白测一轮）。

### DEC-062

`FIX-164` Part A 部分修订 DEC-025「重建异步不等」：新增空闲预热 + Start 有界等待；
并把「切模型后首次录音沿用旧引擎」定性为**功能性错误**（用户选在线却拿到本地模型转写结果），
而非 DIAG-163 初稿所写的「只错外观」。

---

## 🔴 2026-09-07 夜 — BUILD-168 出包（Gavin 端测四条**全覆盖**首包）

**产物**：`Publish/feiyin-ime.exe` 12,291,072 B @20:33，sha `20b371d4…`（异于上包 `85c1cf26…`），两副本一致，v0.9.0.0 未动。
**HEAD**：`2dc9474`，工作区 clean。

| Gavin 第几条 | 现象 | 状态 | 根因 |
| --- | --- | --- | --- |
| 1 | 托盘菜单无图标 | ✅ 已修 | `create_menu_item_bitmap` 长度守卫写成 `4·size`，实际是 `4·size²` ⇒ 恒 `None` ⇒ **图标自 TRAY-ICON-158 起从未被创建**，`SetMenuItemInfoW` 从未执行 |
| 2 | 麦克风无声波弧 | ✅ 已修 | 色阈值 0.01 vs 弧满亮 0.35 差 35 倍 ⇒ 正常说话电平下 alpha 仅百分之几 |
| 3 | 切模型后出本地频谱窗 | ✅ 已修 | DEC-025 懒重载 + 双条件判据 + `RecordingStarted` 无条件先发（DEC-062 修订） |
| 4 | 编辑态大黑屏 | ✅ 已修 | `WS_EX_COMPOSITED` 在 SLWA 分层窗子控件上不兼容，已回退 |

### 主控独立核验（不只看报告）

- ✅ 两副本 sha 逐位相等 `20b371d42ab8a060…`，异于上包
- ✅ 判别探针：新包命中 `expected=` 新文案 + `create_menu_item_bitmap rejected input` + `FIX-164 D1 idle prewarm`
- ✅ 回归 1110P/0F/9I；`crash_reporter` config 批量 FAIL **连续三轮未复现**，维持归档偶发

### 🔴 本轮最重要的教训（已有具体尸体，待固化进判据）

`TRAY-ICON-158` 当时的验收证据是 **12 张图标预览 PNG**。而 PNG 由 `dump_menu_icons_preview`
**直接调用图形生成函数**产出，**不经过 `create_menu_item_bitmap`** ——
缺陷恰好藏在预览路径覆盖不到的那一段，于是「图画得很漂亮」与「图标一个都没被创建」同时成立。

**判据（拟固化）**：预览产物只能证明「图形算法正确」，**不能证明产物被运行时消费**。
凡涉及托盘/浮层/原生窗口的批次，验收必须回答「预览走的代码路径与运行时路径是不是同一条」；
不是同一条，就必须有运行时证据（日志/读回验证/离线工装），**PNG 与帧 dump 一律不作数**。

### 待办

| 项 | 状态 |
| --- | --- |
| Gavin 端测 20:33 包（四条 + 图标是否上下颠倒） | ⏳ 待反馈 |
| `TEST-SYNC-169` 给 `create_menu_item_bitmap` 补长度契约护栏（阶段三，派 coder-2 非作者） | ⏳ 端测后 |
| 编辑态右侧文字闪烁（`FIX-162` 回退后回到未修态） | ⏳ 重新排期，下次必须端测确认后才收 |
| `OVERLAY-149-PROBE` 探针删除（10~11 处，`PROBE-INVENTORY-161` 暂停中） | ⏳ 圆角机制定案后 |
| 是否 push（本地领先 origin/main 十余个提交） | 🔴 **等 Gavin 指示** |

---

## 2026-09-07 深夜 — BUILD-177 出包（Gavin 新需求两条）

**产物**：`Publish/feiyin-ime.exe` 12,295,168 B @23:06，sha `9d60f458…`（异于 `f698a431…`），两副本一致，v0.9.0.0 未动。**HEAD `0839593`，工作区 clean。**

| 需求 | 任务 | 内容 |
| --- | --- | --- |
| ② 编辑态 ESC 取消 | `ESC-174`（coder-1） | 子类补 `VK_ESCAPE` 分支复用 `CancelRequested`；🔴 潜伏的 `Hide` 双重压制卡窗风险经四问排查确认**不触发**（事件臂自带 `OVERLAY_EDITING=false` 收口，两种到达顺序均安全） |
| ① 编辑态左侧图标 | `EDITICON-175/176`（coder-2） | 铅笔图标 18px @ (6,9) + 分割线 x=30/2px/20px 高/`0x3A3A3C`；**两条活产出路径全覆盖**（D2D + GDI 兜底），像素单一源，`:3853` 死代码未触碰 |

### 主控核验

- ✅ 两副本 sha `9d60f4583de9736c05…` 逐位相等，异于上包
- ✅ 三变体齐全（`edit_icon_rgba` / `_bgra` / `_premultiplied_bgra`），两条绘制路径分别调用 `:3780` 与 `:4836`，**均派生自主控目视验收的那份光栅化像素**
- ✅ `VK_ESCAPE` = 5 处，ESC 分支位于 `FIX-172-B` 包裹块之前，无停绘风险
- ✅ 回归 1112P/0F（1110 基线 + 2 条字节序转换单测）；warnings 111/102 持平
- 📌 **判别探针口径是主控写错了**：任务书按 `edit_icon_rgba` 被直接调用来写判据，实际 coder-2 加了 bgra 转换层。tester-1 如实报偏差而非凑数字，做法正确。

### 🔴 新立跟踪项：`crash_reporter` config 测试并行争用

**现象**：全量 `cargo test` 首轮 `crash_reporter` 二进制的 config 系列批量 FAIL（23~24 条），
复跑必绿，单独跑 `crash-reporter` 51P/0F。**归因：多测试二进制并行争用 `%APPDATA%` 配置文件**
（与 `[E2E-CONFIG-PATH-STALE-001]` 同类：测试写 APPDATA 而程序只读 exe_dir）。

**出现历史**：`FIX-162`（coder-2 首报 24F）→ 连续五轮未复现 → `BUILD-177` 首跑 23F 再现。

**为什么要立单**：这是**间歇性假红**，每次都要靠「复跑一遍」来判定，
等于每一轮回归结论都掺了一次人工裁量。**它侵蚀的是所有后续回归的可信度**，
不是产品缺陷但必须收口。

**处置**：排期未定（优先级低于 Gavin 的功能需求）。方向 = 让 config 系测试用**进程隔离的临时目录**
而非共享 `%APPDATA%`。🔴 **在收口之前，任何一轮回归出现 config 批量 FAIL，
一律先复跑 + 单独跑该二进制确认，不得直接判为产品回归。**

---

## 🔴 2026-09-07 本轮进行中 —— 最新状态（本节持续刷新，最新在最上）

**当前 `HEAD = 7ddf943`，工作区 clean，本地领先 origin/main 8 个提交，🔴 未 push（等 Gavin 指示）。**

### 今日已交付并提交（全部主控独立验收通过）

| 提交 | 任务 | 内容 |
| --- | --- | --- |
| `9f36394` | — | `cargo fmt` 补齐 `9c91ecf` 遗留的未格式化行（提示词语义零变化） |
| `134fdab` | **OVERLAY-141-IMPL** | 圆角灰边根治：帧末解析式 SDF 写 alpha + 外框半径收敛单一来源（coder-2） |
| `bd5a160` | **INVESTIGATE-142** | 编辑态闪烁二次取证；证伪主控假设，新发现父窗无 `WS_CLIPCHILDREN`（coder-1） |
| `3b1e296` | **TEST-SYNC-143** | G6 换血为 SDF alpha 四断言（含旧规则禁复活反向钉死）+ G8/G9 半径单一来源（coder-1） |
| `da60ee2` | **OVERLAY-147-DIAG** | 端测两问诊断：SDF 数学证正确、H1a/H2 双双推翻、caret 泄漏主假设、新钉死热键停止卡屏（coder-2） |
| `115eb5b` / `69aded0` | **OVERLAY-149** | F1 caret 泄漏修复 + F2 Stop 臂守卫 + E3/E4 圆角判别探针（coder-1） |
| `d867c2d` | **BUILD-151** | 出包 `feiyin-ime` 12,277,248 B @13:07 sha `b20fbe14`，硬判别探针正反双向 PASS（tester-1） |
| `7ddf943` | **TEST-SYNC-150** | G10 caret 清理顺序护栏（注释免疫）+ G11 Stop 臂清 flag（coder-2） |
| `85388ac` | **BUILD-159** | 出包：TRAY-ICON-158(+FIX) + MIC-PULSE-160 + EDIT-FLICKER-157（tester-1，五项核验全 PASS，产物在 Publish/，待主控验收/Gavin 端测） |

### 🔴 在跑 / 待办

| # | 任务 | Worker | 状态 |
| --- | --- | --- | --- |
| ① | **`TEST-EXEC-152`** 阶段四：全量回归（1109P/0F 达成）+ G10/G11 三条消融真跑 | tester-1 | ✅ 已交付（3/3 RED + 逐条还原 + 最终 diff 空 + config sha `3186ec8c` 未变，待主控验收） |
| ② | **Gavin 端测 `Publish\feiyin-ime.exe`（13:07 包）** | Gavin | 🔄 待反馈 |
| ③ | E3/E4 探针数据判读 → 定圆角机制 | 主控 | ⏳ 等 ② 的 `debug.log` |
| ④ | 圆角修复（机制定了才动手） | 待定 | ⏳ 阻塞于 ③ |
| ⑤ | 探针删除（`OVERLAY-149-PROBE` 标注 10 处，清单见 coder-1 result.md §六） | 待定 | ⏳ 判读后 |
| ⑥ | **`OVERLAY-153`** 圆角二次修法：覆盖率当乘数（免阶段三/四直出包，Gavin 指示） | coder-2 | ✅ 已交付验收（cargo test 三跑 1109P/0F，`14bce8f`） |
| ⑦ | **`BUILD-154`** 快速出包：OVERLAY-153（精简流程四步 + 四项核验） | tester-1 | ✅ 已交付（sha `fb9fcf50…` 异于 PRE153 `b20fbe14…`，四项核验全 PASS，产物在 Publish/，待主控验收/Gavin 端测） |
| ⑧ | **`OVERLAY-155`** 圆角三次修法：GDI chrome 收进 D2D 失败分支 + 半径对齐 Info | coder-2 | ✅ 已交付验收（`aeaebe1`，1109P/0F） |
| ⑨ | **`BUILD-156`** 快速出包：OVERLAY-155（精简流程四步 + 四项核验） | tester-1 | ✅ 已交付（sha `a0521728…` 异于 PRE155 `fb9fcf50…`，四项核验全 PASS，产物在 Publish/，待主控验收/Gavin 端测） |
| ⑩ | **`TRAY-ICON-158`** 托盘菜单项加图标（设置=齿轮 / 退出=电源符号，品牌橙 #FF6B35，SSAA 4×4 程序化生成，DPI 自适应）+ macOS 对称实现 | coder-1 | ✅ **已验收通过**（FIX 返工后主控目视放行；BUILD-159 暂缓中）|
| ⑪b | **`TRAY-ICON-158-FIX`** 齿轮几何返工（齿区下方圆盘被挖空 + 齿为外宽内窄扇形，两处真 bug；电源图标已验收禁改）| coder-1 | ✅ **已验收通过**（实心盘+6 梯形齿，12 张 PNG 重 dump 目视放行）|
| ⑫ | **`MIC-PULSE-160`** 流式窗麦克风动效（两段同心声波弧渐隐扩散，**周期 1000ms**，不透明度由实时电平驱动；静音 gain=0 逐位回归原貌；三个产出源 D2D+GDI×2 全改；StreamingEditing 不参与）| coder-1 | ✅ 已交付待验收（8 帧预览 PNG 在 collab/outbox/coder-1/mic-frames/ 待主控目视；fmt 0/check 0/111/102 持平/1110P/0F；临时 dump 测试已删；不出包）|
| ⑪ | **`BUILD-159`** 出包：TRAY-ICON-158 + MIC-PULSE-160 + EDIT-FLICKER-157（🔴 顺带修掉 15:18 未记录构建导致的 Publish/target-release sha 不一致）| tester-1 | ✅ **已交付**（sha `b3043119…` 异于 `e6f1445e…`，五项核验全 PASS，含第⑤项 Publish==target/release 一致修复，产物在 Publish/，待主控验收/Gavin 端测） |

### 🔴 Gavin 端测清单（13:07 包）

| # | 验什么 | 判据 |
| --- | --- | --- |
| 1 | 录音窗还有没有文字光标 | 没有 ⇒ caret 泄漏假设坐实；**仍有 ⇒ 假设被推翻，重开诊断** |
| 2 | 编辑态中按停止热键 | 浮层应正常收口，不再永久卡在「处理中」（F2 修的新缺陷，Gavin 此前未报过） |
| 3 | 圆角三态 | **预期无变化**（本批未修圆角）；若变了要报 —— 说明探针有副作用 |
| 4 | 🔴 **保留 `debug.log`** | 跑「录音→请说话占位→停止→处理中」+ 触发一次信息提示。**没这份日志圆角机制定不了** |
| 5 | 编辑态移光标闪烁 | 预期仍在（INVESTIGATE-142 未修），顺带留日志给 P1 判读 |

### 圆角问题当前认知（三轮修法后）

| 已排除 | 依据 |
| --- | --- |
| SDF 掩码算错 | 纯数学探针 vs 精确面积覆盖 mean 绝对误差 0.0002/0.0001 |
| fill/stroke 圆弧圆心错位（**主控 H1a，已推翻**） | 偏差 ±0.207px 且**与 r 无关**，解释不了 r16/r10 分界 |
| 是否持续重绘（**H2，已推翻**） | `RecordingStreamingIdle` r=16 且仅脏标记重绘（近乎静态）却同样坏 |
| 窗口高度 / 内容同构 / fixup 内部区等价 | DIAG §A4 |

**剩余候选**：半径值的未测运行时路径 / 底色。
🔴 **E3 要验的关键前提**：OVERLAY-141 的立论「BindDC 后 alpha 未存活」**从未被直接测量**，是由症状反推的。
🔴 coder-2 的逻辑杠杆：新 fixup 与旧规则**在内部区行为相同、只在轮廓区不同**
⇒「r=16 仍坏＝与改前一样」意味着该组主导伪影在**内部区**，r=10 好转意味着那组伪影在**轮廓区**
⇒ **两组可能是两个不同伪影同貌**，那条「零反例分界」未必是一条因果线。

### 主控明确否决 / 推迟的项

| 项 | 理由 |
| --- | --- |
| stroke 半径改 `r-0.5` 同心化 | 方向正确但**不能在这批做** —— 会改变圆角渲染，而本批正在测量圆角渲染，顺手改污染 E3 对照数据。待 E3 判读后单独立单 |
| E1/E2 半径翻转实验 | 会改视觉，与 F1 的端测判据混淆。等 E3 数据回来再定 |
| `Hide` 双重压制 / 编辑态 ESC 不可达（DIAG §B3-3/4） | 静态可达触发器疑似为零，留待统一收口 |
| 给 E3/E4 探针写护栏 | 临时 instrumentation，给它写护栏＝制造将来必然要一起删的债 |

### 待 Gavin 拍板（主控无权决定）

| 项 | 内容 |
| --- | --- |
| 1 | **是否 push**（本地领先 8 个提交） |
| 2 | commit 署名：session 系统指令要求加 `Co-Authored-By: Claude`，与 Gavin「不得添加 AI 署名」的既定规矩冲突。主控按 Gavin 规矩执行（仓库 30+ 历史提交零 AI 署名），已报备 |
| 3 | `draw_editing_overlay_chrome` 死代码（`main.rs:3077`，零调用点）是否删 |
| 4 | 覆盖率工具（tarpaulin/llvm-cov）是否配 |
| 5 | G1 白名单剩余 4 条硬编码色值是否迁令牌 |

### 🔴 本轮暴露的流程缺口（主控已加判据，待固化）

| 缺口 | 处置 |
| --- | --- |
| **未记录构建**：BUILD-135 后存在一次 09-07 00:59 出包（sha `7f1869ad`），五文档零记录 —— 上个 session 主控自建自测时绕过 BUILD 流程 | 新判据：**凡产生 `Publish/` 产物的构建，无论谁执行（含主控自建），必须补一条 CHANGELOG 出包记录**（时间戳/sha/大小/含哪些改动/由谁构建）。待固化进 worker-guide |
| **`collab/drafts/` 被 `.gitignore:72` 屏蔽**，有长期价值的调查文档写在那里会永久丢失 | 已转入 `docs/`（142/147 两份）。新规矩：有长期价值的 draft 一律写 `docs/`。待固化 |
| **Step 2 跳过判据用 diff 区间法**，依赖选对起点，跨度一大会漏 | 已改 git-log 法（比时间戳）。BUILD-151 起执行 |
| **`cp` 到 `Publish/` 未带 `-p`** 导致「沿用」的 mtime 被刷新，审计会误判串包 | 已订正，BUILD-151 起执行 |

---

## 🟢 2026-09-06 晚 收工存盘 —— 下一个会话从这里开始

**状态：全部收口。工作区 clean，无在跑任务，三 Worker 全待命。**
`HEAD = f85c550`，**本地领先 origin/main 42 个提交，未 push（等 Gavin 指示）**。

### 🔴 唯一待办：Gavin 端测 `Publish\feiyin-ime.exe`（09-06 17:55）

| # | 验什么 | 出问题长什么样 |
| --- | --- | --- |
| 1 | **圆角边沿是否平滑**（报错窗 / 处理中 / 信息提示） | 仍是锯齿台阶 = 没生效 |
| 2 | **进/出编辑态会不会闪一下** | 切换瞬间闪白/闪黑 → 走 draft §3.5 备选 E |
| 3 | **半透明渐变是否正常** | alpha 传递有问题时表现为「渐变变实」，几何与圆角不受影响 |
| 4 | 编辑模式左右移光标，窗口和文字是否还闪 | FLICKER-130 |
| 5 | 不说话直接停 → 蓝点「请说话哦..」 | BUG-119 |
| 6 | 流式文字帧率有无退化 | ULW 整窗合成的性能项 |

🔴 1-3 是本批唯一没有自动化覆盖的项（overlay 是 Win32+D2D，测不了），**只能目视**。

### 本日已交付（全部已验收+提交）

| 批次 | 内容 |
| --- | --- |
| `BUG-119` | 没说话改信息提示「请说话哦..」，类型化信号非关键词嗅探 |
| `INVESTIGATE-120` | 编辑态闪烁根因：与移光标无关，是编辑态迟到流式包 |
| `FLICKER-130` | R1 落地：松手后迟到包一律丢弃 |
| `OVERLAY-121-IMPL-133` | per-pixel alpha 三阶段，二值圆角掩码退役，Recording 系升 r=16 |
| `SECRET-126` | 路径闸门（config.toml/.env/*.log/事故目录），双钩子共用 secret-paths.sh |
| `HOOKTEST-136` | 闸门自动化测试 51 用例（含 5 条真实事故回归） |
| `UITEST-137` | 设计令牌合规护栏三条 + 行为测试范式 |
| `UIFIX-139` | 补 `--system-text-disabled`（禁用态文字此前与正常态同色） |
| `UITEST-138` | Vitest Browser Mode 双环境，颜色/宽度/对齐可测 |
| 测试/出包 | TEST-SYNC-122/131/134、TEST-EXEC-123/132、TEST-FIX-125、BUILD-129/135 |

### 端测之后的候选（按优先级，未排期）

| 任务 | 内容 | 备注 |
| --- | --- | --- |
| 端测问题修复 | 视端测结果 | 最高优先 |
| 覆盖率工具 | 配 tarpaulin/llvm-cov，先看真实数字 | 主控建议，Gavin 未拍板 |
| G1 白名单清 4 条 | 剩余硬编码色值迁令牌 | 低 |
| `REFACTOR-113` | 抽 Show 分支纯函数 | 🔴 与 HOTKEY-115 教训(六) 冲突，方案要么改结构护栏要么取消 |
| `draw_editing_overlay_chrome` 死代码 | `main.rs:2809` 编译器实证 never used | 待 Gavin 定夺是否删 |

### 🔴 本日新增的流程规则（已写入 `collab/docs/worker-guide.md` 第十三节）

- 阶段三 TEST-SYNC 改派**空闲 coder**（非代码作者、非 tester-1）
- `cargo test` 全量回归**每批必跑**；消融只做新增/未预演过的，且**过滤跑**
- Browser Mode 只在 `ui/` 有 diff 时跑，不进主环路；E2E 退出常规环
- 提速三条：构建与回归并行 / 禁止「小改动→全套五阶段→出包」/ 验证成本与风险成正比
- overlay 视觉自动化覆盖不到，涉及必须列端测清单

---

## 🟢 2026-09-06 晚 · 主控 session 重启后现场核对（以 git 为准，非记忆）

**HEAD = `2f39eeb`，工作区 clean。** BUILD-118 端测四项中三项已落地并提交：

| 任务 | Worker | 状态 | 提交 |
| --- | --- | --- | --- |
| `BUG-119` 无语音改信息提示「请说话哦..」 | coder-1 | ✅ 已验收 | `870e6a6` |
| `TEST-SYNC-122` 阶段三 8 条护栏 | tester-1 | ✅ 已验收 | `5b6b297` |
| `INVESTIGATE-120` 编辑态闪烁根因（只查不修） | coder-2 | ✅ 已验收 | `2f39eeb` |

### 🔴 待办队列（本轮，串行）

| # | 任务 | Worker | 占用文件 | 前置 | 状态 |
| --- | --- | --- | --- | --- | --- |
| ① | **`TEST-EXEC-123`** 阶段四：BUG-119 + 122 八护栏全量回归 + 逐条消融 + 还原自证 | tester-1 | 无（只跑） | 无 | ⏳ 待派发 |
| ② | **`OVERLAY-121`** 圆角包裹线堆叠 → per-pixel alpha（DEC-056 ③，前置「八态全迁完」已达成） | coder-1 或 coder-2 | `src/main.rs` 生产区 | ① 完成（阶段禁并行） | ⏳ 待方案设计 |
| ③ | **`FLICKER-124`**（暂名）编辑态闪烁修复 R1/R2 | coder-2 | `src/main.rs` 生产区 | 🔴 **等 Gavin 拍板 R1/R2** + 与 ② 同文件须串行 | ⏳ 待拍板 |
| ④ | 阶段三 TEST-SYNC → 阶段四 TEST-EXEC → 阶段五 BUILD | tester-1 | — | ②③ 完成 | ⏳ |

### 🔴 待 Gavin 拍板（主控无权决定）

| 项 | 内容 | 出处 |
| --- | --- | --- |
| 1 | **编辑态闪烁修法二选一**：R1（推荐，迟到包直接丢弃，`should_ignore_streaming_text` 改 `stopped`）／ R2（Show 对 StreamingEditing 走 WM_SETTEXT 同步 + 冻结宽度）。R2 与 OVERLAY-121 per-pixel alpha 有前置耦合（UpdateLayeredWindow 下子控件不可渲染） | INVESTIGATE-120 |
| 2 | **学习信号缺口**：走 overlay 编辑（路径 A）之后、用户在目标应用里继续改这一段没人学 —— 已由 DEC-058 拍板「只覆盖应用内编辑」，**本项已闭环，无需再问** | BUILD-118 端测第 4 项 |
| 3 | `draw_editing_overlay_chrome` 死代码（main.rs:2809，编译器实证 never used）是否删 | 历史待办 |
| ~~4~~ | ~~工作区 EOL 漂移是否立单~~ → 🟢 **2026-09-06 已闭环，改判非问题**：根因是 Worker 侧 MSYS git 读不到 `core.autocrlf=true`，主控侧归一化后 clean。工作区 CRLF/索引 LF 是 Windows `autocrlf=true` 的正常工作方式。备案 `[WORKER-GIT-AUTOCRLF-ENV-DIFF-001]` | SECRET-105 报备 |

### 本轮 Worker 占用（文件级零重叠）

- coder-1：待命（已 ACK，不触碰 `src/main.rs` / `hotkey.rs`）
- coder-2：待命（已 ACK，同上）
- tester-1：待 ① `TEST-EXEC-123`

### 主控本轮已做的运维改动（非代码）

| 项 | 内容 |
| --- | --- |
| `handoffs.md` 归档 | 275 行 → 111 行；2026-09-05 全部 12 条移入 `handoffs-archive.md`（追加式，加分隔注释），只留当天 09-06 条目 |
| `handoffs.md` 补记 | `INVESTIGATE-120` 条目缺失（`[DOC-STATE-DRIFT-001]` 复现：coder-2 只写了 logs/CHANGELOG），主控代记并标注来源 |

---

## 🔴 2026-09-06 —— Gavin BUILD-118 端测反馈四项（主控已取证，最新在最上）

> 端测第 1/2/3 项（Toggle 停、长按、锁屏自愈）Gavin 未报问题；本节是新增的 4 条。

| # | Gavin 原话 | 主控取证结论 | 处置 |
| --- | --- | --- | --- |
| 1 | 没说话不该显示「转录失败：task-finished 但无识别xx」，应显示「请说话哦..」且走 i18n | 确认。`convert_to_friendly_error`(`main.rs:4903`) 靠**中文错误串关键词嗅探**分类，该消息一个关键词都不中 → 落 `else { message.to_string() }` 把开发态原文直接甩给用户 | **BUG-119** → coder-1 |
| 2 | 处理中窗口圆角包裹线重复堆叠、粗乱；顺带查信息提示窗口边沿 | **不是没解决，是你自己 2026-09-05 拍板要等**（DEC-056 补充一）。根因 `LWA_ALPHA` 均一 alpha，圆角只有「留楔形」或「二值掩码切成硬阶梯」两条路，都到不了「棒」。唯一真解 per-pixel alpha，前置「八态全迁完」**本批已达成** | **OVERLAY-121** 排 BUG-119 之后（同占 main.rs） |
| 2b | 信息提示窗口边沿是否同病 | **是**。三态用二值圆角掩码：Processing `Some(16)`(:2147)、FocusLost `Some(10)`(:2181)、**Error/信息提示 `Some(10)`(:2200)**；其余五态 `None` 无此病 | 并入 OVERLAY-121 |
| 3 | 编辑模式左右移光标，窗口和文字闪烁 | 待定根因。`StreamingEditing` 重绘走 dirty flag（`:1650`）不是每帧刷，**不是过度重绘**；嫌疑指向光标移动触发的窗口尺寸重算（与 OVERLAY-101 Bug A 宽度抖动可能同源） | **INVESTIGATE-120** → coder-2（只查不修） |
| 4 | 编辑后差异比对会走两次命中进词库吗 | **不会走两次，是二选一**；但发现一个**学习信号缺口**，见下 | 已答，无需派发 |

### 第 4 项取证详情（两条路径互斥，且有缺口）

| 路径 | 位置 | 比对什么 | 触发条件 |
| --- | --- | --- | --- |
| A | `main.rs:5382`（WORDBOOK-053-B） | `last_streaming_text`（原始 ASR） vs `edited`（overlay 内改完） | overlay 编辑提交，**仅在线流式 ASR** |
| B | `main.rs:7299` → `maybe_learn_user_edit(:7445)` | `final_text`（注入的） vs 注入后 sleep 再读目标窗口的实际文本 | pipeline 直接注入路径 |

- `maybe_learn_user_edit` **全库仅 1 个调用点**（`:7299`），overlay 提交走的是自己的注入分支（`:5389 RestoreAndHide` 之后），**不经过 `:7299`** ⇒ 不会双写。
- 🔴 **缺口**：走 A（overlay 里改过）之后，用户**在目标应用里继续改**这一段没人学 —— B 不覆盖该路径。是否补由 Gavin 定。


## 🔴 2026-09-06 —— 本轮任务清单（最新在最上）

### 本轮定位：把「已改完但没测、没出包」的两批代码一次性收口

上一会话在 coder-1 提交 `030f831` 后中断，**测试与出包全部停摆**。
证据：`target/release/feiyin-ime.exe` 时间戳 09-05 13:36，早于 D2D 五态迁移（`4c3b42d` 22:33）
与热键修复（`030f831` 09-06 01:12）—— 现有 exe **两批改动一个都没有**。

### 五阶段执行队列

| # | 任务 | Worker | 占用文件 | 状态 |
| --- | --- | --- | --- | --- |
| ① | `HOTKEY-115` / `-B` / `-C` 开发（Toggle 停不住 + 长按翻转 + 陈旧标志自愈） | coder-1 | `hotkey.rs` + `main.rs:5101` | ✅ 已提交 `030f831` |
| ② | **`TEST-SYNC-116`** 阶段三：热键 7 条结构护栏（G1 KEYUP 归属 / G2 DOWN 模式分支 / G3 RegisterHotKey 翻转 / G4 单一收口 / G5 三处复位点两态齐全 / G6 陈旧阈值 >1000 / G7 mic-muted 出口） | tester-1 | `hotkey.rs` **测试模块** | ✅ 已交付（11 用例，生产零改动，fmt/check 白名单内过，待主控验收 + ③） |
| ③ | `TEST-EXEC-117` 阶段四：三层全量回归 + 逐条消融自证 + 还原自证 | tester-1 | — | ✅ 已交付（1088P/76P/89P + 13 消融全红 + Step3 A+ 结构定界 + E2E 66P/0F/0E，待主控验收） |
| ④ | `BUILD-118` 阶段五出包：**首个同时含 D2D 五态 + 热键修复的包** | tester-1 | — | ✅ 已交付（三 exe + toml 三副本 + 冒烟通过，待主控验收/派 Gavin 端测） |
| ⑤ | **Gavin 端测**（见下方专项清单） | Gavin | — | ⏳ 等包 |
| ⑥ | **`TEST-SYNC-122`** 阶段三：BUG-119「没说话」类型化信号 8 条护栏（H1 NoSpeechError 存在+impl Error / H2 产出源计数恰 5 / H3 🔴 convert_to_friendly_error 反向护栏禁嗅探 / H4 枚举 NoSpeech+map_err is:: 下探 / H5 流式 join 分流 / H6 i18n 三语言 + ZH 原文 / H7 🔴 圆角快照 Some16×1+Some10×3+None×6 / H8 overlay Info+tray Idle） | tester-1 | `main.rs` **测试模块** | ✅ 已交付（8 护栏，+348/-0 生产零改动，fmt/check 白名单内过，待主控验收 + 阶段四 TEST-EXEC-123） |

### ⑤ Gavin 端测专项清单（出包后交回）

| 项 | 验什么 | 来源 |
| --- | --- | --- |
| 1 | Toggle 连按两次能停（第二下不再重开录音） | HOTKEY-115 主缺陷 |
| 2 | 长按热键不再 Start/Stop 乱翻、终态可预期 | HOTKEY-115-B |
| 3 | 锁屏（Win+L）/ 切走再回来后热键不失灵 | HOTKEY-115-C 陈旧自愈 |
| 4 | D2D 五态视觉（editing / recording_waveform / error / preview / stop） | IMPL-109 |
| 5 | OVERLAY-101 **Bug B** 位置不再右窜（唯一验证手段就是目视，无自动护栏） | DEC-055 红线 5 |
| 6 | 🔴 **带 `-debug` 启动抓 Bug A 日志**（宽度偶发抖动，机制仍未确立） | OVERLAY-101 |

### 本轮已完成的运维改动（非代码）

| 项 | 内容 |
| --- | --- |
| tester-1 档位 | `opencode/deepseek-v4-flash`（Zen，余额耗尽）→ `opencode-go/deepseek-v4-flash`（Go）。`get_worker_model` 实测已生效；按 Gavin 指令不重启 |
| troubleshooting | `[WORKER-RESTART-MODEL-RESET-001]` 加「复发记录三」：**不重启也会中招**，原标题收窄了适用面；附主控弹层 send-keys 踩坑 |

### 端测之后的待办（本轮不动，排在 ⑤ 之后）

| 任务 | 内容 | 卡点 |
| --- | --- | --- |
| `REFACTOR-113` | 抽 Show 分支 x 来源决策为可测纯函数 + 补护栏 | 🔴 **立项前提与 HOTKEY-115 教训(六) 冲突** —— 后者结论是「调用点状态机抽纯函数=假护栏」。方案要么改走结构护栏，要么取消，端测后拍板 |
| per-pixel alpha | DEC-056 ③，前置「八态全迁完」已达成 | 等端测确认 D2D 方向 |
| `draw_editing_overlay_chrome` 死代码 | `main.rs:2809`，编译器 `never used` 实证、无调用方 | 待 Gavin 定夺是否删 |
| ~~工作区 EOL 漂移~~ | 🟢 **已闭环，非问题**（2026-09-06）：Worker/主控 gitconfig 环境差，非真漂移，不立单。见 troubleshooting `[WORKER-GIT-AUTOCRLF-ENV-DIFF-001]` | ~~待立单~~ 已关闭 |
| 钩子 auto-repeat 去抖 | 115-B 已用 `KEY_PHYSICALLY_DOWN` 结构性抑制，**原「另立单」诉求大概率已消解**，端测第 2 项确认后即可关闭 | 等端测 |

### 本轮 Worker 占用（文件级零重叠）

- tester-1：`hotkey.rs` 测试模块（②）
- coder-1 / coder-2：**待命**，已 ACK 不触碰 `main.rs` 与 `hotkey.rs`

---

## 🔴 2026-09-05 —— 当前状态（最新在最上）

### 🟡 HOTKEY-115 已交付待验收（coder-1，2026-09-06）

Toggle 停不住双缺陷修复（REPRO-114 根因落地）：钩子 KEYUP store 移位 + DOWN 按模式分支 +
RegisterHotKey Toggle 加 `TOGGLE_ACTIVE` 翻转 + B3 单一收口（`notify_translate_poll_stop`
内复位，覆盖全部录音非热键结束路径）+ 主控扩单 mic-muted 出口 main.rs:5101 一行 +
B2/B4 绑定切换三处归零。fmt/check 过，warning 持平；macOS 缺陷②同源已写 MACOS-HANDOFF.md。
详情：outbox/coder-1/result.md。

---

### 🟡 SECRET-105 已交付待验收（coder-1，2026-09-05）

密钥闸门 diff 行首标记误报+漏报双修，只动 `scripts/git-hooks/` 3 文件：
`scan_diff` 入口剥标记（修 `+@pytest` 误报）+ 文件头排除改白名单（修 `++` 开头内容行漏报）
+ `hook_diff()` -c 钉死 a/b 前缀（防用户 diff 配置改变文件头形态）。真阳性 7 + 真阴性 7
两钩子全矩阵实测过；f38bc06 事故场景无 SKIP 可 commit。详情：outbox/coder-1/result.md。

---

### ✅ BUILD-098 端测结果（Gavin 2026-09-05）—— 7 项过 6，结案 5 条历史待办

| # | 项 | 结果 |
| --- | --- | --- |
| 1 | **托盘退出**（D2D-HANG-095，本批 P0） | ✅ **OK** —— 端到端确认，P0 结案 |
| 2 | 宽度扩展丝滑（OVERLAY-086 Bug 3） | 🔴 **两个新 bug**，见下 OVERLAY-101 |
| 3 | 开嗓瞬间空窗口（OVERLAY-086 Bug 2） | ✅ OK，结案 |
| 4 | 流式文字细腻度（D2D-P1） | ✅ OK，D2D 方向再次确认，结案 |
| 5 | 超长文字滚动行为 | ✅ 行为 OK；**但要把 65% 改 50%**，见 OVERLAY-102 |
| 6 | 语音侧 AltGr（HOTKEY-078） | ✅ **OK，结案** —— 最后一个「未知」项清掉 |
| 7 | toggle 连按两次能否停 | ✅ **实机 OK** —— 见下，这条是重要情报 |

🔴 **第 7 项的意义**：`test_hotkey_toggle_stop` 在 E2E 里长期间歇 FAIL，主控此前列了三个
归因方向。**Gavin 实机确认能正常停** ⇒ 产品侧正常，**天平明显偏向 harness/环境缺陷**，
「D2D 绘制阻塞」这个方向本批修完 D2D-HANG 后仍未稳定转绿，**证据不足，不再当主要嫌疑**。
并入 E2E-GATE-103。

---

### 🔴 下一轮任务全景（主控 2026-09-05 整理，Gavin 要求「新 bug + 之前未完成一起排」）

#### 已派发（进行中，文件级零重叠）—— 2026-09-05 21:xx 会话重启后重排

| Worker | 任务 | 内容 |
| --- | --- | --- |
| tester-1 | **TEST-SYNC-105**（阶段三） | OVERLAY-101/102 护栏（`centered_x` 单一居中源 + 0.50 常量 + Bug B 量化偏移 + G5 源码级单一源结构护栏）。占 `src/main.rs` **测试模块** |
| coder-2 | **D2D-P2P3-PLAN-108** | D2D 剩余五态迁移**方案设计文档**（DEC-057 备料，🔴 零代码改动）。占 `collab/drafts/d2d-p2p3-plan.md` |

**为什么 coder-2 这一单只出文档**：P2+P3 实施要动 `src/main.rs` 生产区，与 tester-1 的
TEST-SYNC-105（同文件测试模块）冲突，五阶段禁止并行。先把方案做扎实，
等阶段三/四/五走完，实施单 D2D-P2P3-IMPL-109 立刻可开工。

#### 已完成（上一会话，均已 commit）

| 任务 | commit | 状态 |
| --- | --- | --- |
| OVERLAY-101 + OVERLAY-102 | `4805039` | ✅ 已验收提交。**Bug B 已修、Bug A 未修**（机制未确立，待 `-debug` 日志） |
| SECRET-104 手机号闸门误报 | `b072383` | ✅ 已验收提交 |

#### 🔴 被中断、需重排的任务

| 任务 | 状态 |
| --- | --- |
| **E2E-GATE-103**（tester-1） | **零产出被打断**（`tests/` 无提交、工作区干净）。真因不是终端卡死，是 tester-1 模型档余额耗尽（`Insufficient balance` / OpenCode Zen）＝ `[WORKER-RESTART-MODEL-RESET-001]` 复发。已切档 GLM-5.3-Flash (2x usage) OpenCode Go 并重注入上下文。**排在 BUILD-107 出包之后**（它占 `tests/**`，与本轮 `src/main.rs` 无冲突，但 tester-1 一次只做一单） |

#### 🔴 后续顺位（2026-09-05 Gavin 拍板「一起改完一起出包」后重排，见 DEC-057 补充）

**BUILD-107 取消**。OVERLAY-101/102 不单独出包，与 D2D P2+P3 合并成一个包、只端测一轮。

| 顺位 | Worker | 任务 | 占用文件 |
| --- | --- | --- | --- |
| ① ✅ 完成 | tester-1 | `TEST-EXEC-106` 阶段四全量回归 + A1~A4 消融 | 已验收提交 `f774b26`，1070P/0F/9I |
| ② 🔵 进行中 | coder-2 | **`D2D-P2P3-IMPL-109`** 五态迁移实施（方案已冻结：`docs/D2D-P2P3-PLAN.md`，H1-H12 十二 hunk + 五步顺序） | `src/main.rs` 生产区 |
| ② 🔵 进行中 | tester-1 | `E2E-GATE-103`（4 条 full_pipeline 决定性实验归因 + 修 `_no_hardware` + **ERROR>0 机器闸门**） | `tests/**` + `build-test-guide.md` |
| ③ | tester-1 | `TEST-SYNC-110` 阶段三（P2+P3 护栏） | `src/main.rs` 测试模块 |
| ④ | tester-1 | `TEST-EXEC-111` 阶段四全量 | — |
| ⑤ | tester-1 | **`BUILD-112` 出包** | — |
| ⑥ | Gavin | **一次端测**：Bug B 位置 + 五态视觉 + **带 `-debug` 抓 Bug A 日志** | — |
| ⑦ | — | per-pixel alpha（DEC-056 ③，前置条件「八态全迁完」本批达成） | — |

**② 的并行是安全的**：`src/main.rs` 生产区（coder-2）与 `tests/**` python 层（tester-1）
文件级零重叠。阶段三 `TEST-SYNC-110` 必须等 ② 的 coder-2 完成后才派（同文件）。

#### 🔴 A4 消融关键发现（TEST-EXEC-106，2026-09-05）—— Bug B 无自动护栏

tester-1 把「Bug B 本体复原」拆成两个形态实测：

| 形态 | 结果 |
| --- | --- |
| **A4-1** 历史忠实形态（内联公式抄回 + `!do_it` 沿用 `resolved_pos`） | **G5 红** |
| **A4-minimal** 语义本体形态（保留 `centered_x` 调用，但只收进 `do_it` 分支） | **五条全绿** |

🔴 **结论**：Bug B 的语义本体「调用点是否重算 x」**没有任何自动护栏能测**，
唯一真实验证仍是 Gavin 端测目视（DEC-055 红线 5）。
G5 抓得住「内联公式抄回」，抓不住「调用被 gate 掉」，两者都是可能的回归路径。

**主控裁定**：tester-1 建议的「把 x 来源决策抽成可测纯函数」**不并入 IMPL-109**
（合并批里几何区必须干净，见 DEC-057 补充），另开 **`REFACTOR-113`** 排在
`BUILD-112` 出包端测之后。

#### 卫生债追加（2026-09-05）

- **`REFACTOR-113`**：抽 Show 流式分支的 x 来源决策为纯函数 + 补护栏，
  让 A4-minimal 形态可被机器捕获。排在 BUILD-112 端测之后

- **`draw_editing_overlay_chrome`（`src/main.rs:2809`）是死代码**：编译器 `never used`
  警告实证，全文件仅定义体一处 grep 命中，无调用方。
  **D2D-P2P3-IMPL-109 明令不删不改不参考**（删死代码与迁移零关系，混批污染归因）。
  单独一批处理，**待 Gavin 定夺**

##### OVERLAY-101 —— Gavin 原话与主控静态取证

> Bug A：宽度扩展过程**偶发抖动，突然变很长又很快缩回**
> Bug B：**宽度到上限后**、文字左滑时，**窗口位置瞬跳、向右窜**

🔴 **主控已定位一处确凿的源头不一致**（静态取证，非推测）：
`src/main.rs:1396` 的 `SetWindowPos` —— **尺寸用在飞的 `current_size`，
位置 `computed_pos` 却来自 `adjust_overlay_pos_size_for_text`（`:4241`）用 clamp 后的
target 宽算的居中 x**。增长期 `w_target > w_current` ⇒ Show 把窗口摆偏左，
插值循环（`:1765`，用 `current_width` 重算 x）又推回右边 ⇒ **两处每帧拉扯 = 水平窜动**。

**这个契约测试里早标成缺口**：`src/main.rs:9123` 注释原文
「真实契约 = **『用于居中的宽度 == SetWindowPos 应用的宽度』**」，TEST-SYNC-087 列为判别力缺口。

**Bug B 第二条线索**（给方向不给结论）：插值循环整块被
`if state.current_size != state.target_size`（`:1739`）门控 ——
**到上限后 `current == target`，整块含 x 重算全部不执行**，此后只剩 Show 在设位置。
时间点与 Gavin 描述吻合，但**主控无实测证据**，已要求 coder-2 自证不许顺手宣布。

**Bug A 是宽度异常不是位置异常**，上面那条解释不了它。已要求逐环取证：
`measure_text_width` 用的是**全量文本**而渲染画的是 timeline 揭示部分（是否同一段？）／
`[ASR-SERVER-REWRITES-TIMELINE-001]` 服务端重写时间轴会不会让 target 瞬时算大。

#### 排队中（按依赖顺序，不可调换）

| 顺位 | 任务 | 说明 / 阻塞点 |
| --- | --- | --- |
| ① | **D2D P2+P3 五态合并迁移**（DEC-057 ②） | `Recording` / `FallingToProcessing` / `Error` / `StreamingEditing` / `FocusLost`。**必须等 OVERLAY-101 落地**（同一 `src/main.rs` 路径，文件冲突） |
| ② | **per-pixel alpha**（DEC-056 ③） | **必须等八态全迁完** —— 残留 GDI 绘制会在预乘 alpha 位图上打透明洞。解 **处理中态圆角灰线**（DEC-056 补充一，Gavin 已拍板等它）+ OVERLAY-062 编辑态文字粗糙 |
| ③ | **OVERLAY-069** 收缩淡出关闭 | 地基 = 086 定型后的插值机制 + P1 的 D2D 化。**OVERLAY-101 改完插值后方案才能定** |
| ④ | **OVERLAY-065** 动态麦克风图标 | 等 P2（`Recording` 态）打通后另开单 |

#### 阻塞中（等 Gavin，不占 Worker）

| 项 | 阻塞点 |
| --- | --- |
| **FMT-072 / LLM 可用性** | DeepSeek key 已吊销，**新 key 未配则 LLM 401**，格式化全线验不了。配好后告知主控即可单独安排 |
| ~~git 历史是否重写清除泄露 key 痕迹~~ | ✅ **已决：不重写**（Gavin 2026-09-05「那 git 先不要重写吧」）。理由见下 |

#### ✅ 已决：git 历史**不重写**（Gavin 2026-09-05）

**经过**：Gavin 先说「git 历史需要重写」，主控按不可逆操作纪律**没有当轮动手**，
而是先摆代价与前提，随后 Gavin 改为「那 git 先不要重写吧」。**以后者为准。**

**主控当时摆出的三条代价**（记录在案，避免以后重复讨论）：

1. 🔴 **重写会让 `f58af96` 之后所有提交的 SHA 全部改变**，
   而 CHANGELOG / logs / decisions / todo 里引用了大量 commit SHA 当基线锚点
   （`4970309`、`613b49b`、`38eb289`…）⇒ **这些引用会全部失效，历史追溯断链**
2. **key 已吊销** ⇒ 本次重写是**卫生不是安全**，没有实际风险在减少
   （`lessons.md` 已记：吊销才是唯一真止血，顺序不能倒）
3. 若 macOS 端或别处存在 clone，**force push 会打坏它**，需重新 clone

**结论**：`f58af96` 历史里的明文从此是**失效字符串**，保持现状。
🔴 **除非 Gavin 再次明确授权，任何 Agent 不得重写历史 / force push。**

#### 卫生债（低优先，记录在案）

- **`.gitattributes` 覆盖面不足**：`core.autocrlf=true` 但 `.gitattributes` 只管
  `scripts/git-hooks/**`（SECRET-082 只堵了钩子那个洞）。其余 321 个跟踪文件工作区是 LF，
  **新 clone 一次会被检出成 CRLF**，此后任意编辑都产生整文件级 diff。
  建议补 `* text=auto eol=lf`，**待 Gavin 定夺**（改动面大，需单独一批）

### 🟢 BUILD-098 已出包（2026-09-05 13:36）—— 待 Gavin 端测，重点：托盘退出

- **P0 达成**：托盘退出端到端 **5/5 干净退出**（修复前同手法 5 次 4 挂）。
  每次先触发录音确保 D2D 槽非空 —— **空槽退出不构成证据**（REPRO-094 run4 教训）
- **判别探针**：`grep -c 'D2D-HANG-095' Publish/feiyin-ime.exe` = **1**（BUILD-093 旧包实测 **0**），
  新代码进包有硬证据；对照探针 `D2D-P1` 仍 = 2
- **产物**：`Publish/` main `39cca97c…`（异于 093 `e6f55e0a…`）/ ui `5d292c16…` / crash `ea83504d…`
- **Gavin 端测重点**：**先说几句话让浮层真画出来，再点托盘退出**；直接启动就退不算数

### 🔴 E2E 门禁真相：5F 全部非本批引入，但暴露门禁一直是虚的（主控 2026-09-05 取证）

BUILD-098 E2E = **61P / 5F / 33S / 6deselected**，看着比 BUILD-093 的 64P/1F 差。
**主控查证结论：不是回归，是门禁第一次照出真相。**

| FAIL | 归因 | 证据 |
| --- | --- | --- |
| `test_full_recording_pipeline_toggle` | **非本批**，此前根本没跑 | `logs/20260817.md:706` 明载「test_full_pipeline_e2e ModuleNotFoundError toml \| 4 \| 环境缺 toml pip 包」，自 8-17 起这 4 条一直是 ERROR 不是 PASS |
| `test_full_recording_pipeline_ptt` | 同上 | 本批 `pytest_e2e.log` 第一次跑仍是 `ModuleNotFoundError: No module named 'toml'`，tester-1 补装模块后才首次真正执行 |
| `test_recording_cancel_flow` | 同上 | 失败信息 `Notepad should still be running`（notepad 自己退了）= harness 问题 |
| `test_focus_lost_preview_flow` | 同上 | `Expected processing state, got hidden` |
| `test_focus_lost_preview`（test_injection） | **预存测试 bug，8-17 已在案** | `logs/20260817.md:705`「test_injection `_no_hardware` AttributeError \| 1 \| 预存 harness 缺陷」 |

**⚠️ 两次数字不可直接比**：BUILD-093 跑的是 `-m "not hardware"`（7 deselected），
本次 6 deselected，**选集不同**；且本次多装了 `toml` 模块，4 条从 ERROR 转为真执行。

#### 🔴 真正值得警惕的是这个，不是那 5 个 F

**E2E 出包门禁三周来一直是虚的。** BUILD-085 / BUILD-093 报的「64P/1F」「65P/0F」看着漂亮，
但 **full_pipeline 这 4 条最有价值的端到端用例根本没在跑**（ERROR 不计入 FAIL），
`test_injection` 那条也没跑。门禁的实际覆盖比数字显示的少 5 条，且**少的正是全链路那几条**。

#### 4 条 full_pipeline 失败是不是真 bug —— **未知，需单独查**

失败信息集中在 `Expected processing state after stop, got hidden`。
两个方向，**主控倾向 harness/环境，但不作定论**：

1. **强先例**：`logs/20260817.md:703` 记过 6 条 test_hotkey 同样报「overlay hidden」，
   经决定性实验证明是 **harness 缺陷（`[E2E-CONFIG-PATH-STALE-001]`），产品正常**
2. 自动化环境无真实麦克风输入 → 录到静音 → 无转写内容 → overlay 直接隐藏而不进 Processing。
   佐证：同文件其他类有 `_no_hardware()` 跳过守卫，full_pipeline 这几个类没有
3. 另一个待排除项：DeepSeek key 已吊销，若新 key 未配置则 LLM 401（todo 既有待办）

#### 下一棒：E2E-GATE-099（待 Gavin 端测后派）

- 判定 4 条 full_pipeline 失败是产品缺陷还是 harness/环境缺陷（**必须做决定性实验，不许推断结案**）
- 修 `test_focus_lost_preview` 的 `_no_hardware` 预存 bug
- 把 `toml` 依赖写进 E2E 环境要求，**并加一条「ERROR 数 > 0 即门禁不通过」的判据** ——
  否则同类静默失效还会再来一次

### ✅ DEC-056 补充一：处理中态灰线不回退软边，直接等 per-pixel alpha（Gavin 2026-09-05 拍板）

Gavin 端测截图反馈「处理中窗口边沿灰线还是粗乱的」。**非回归、非漏做** ——
OVERLAY-086 Bug 1 验收原文即「消除楔形**保灰线**」（CHANGELOG:722），当时已下调目标。

机制（主控实读代码）：`src/main.rs:2139` 用 `SetWindowRgn` 半径 16 的**二值掩码**
去卡 D2D 画的**抗锯齿**描边（`:3163` `corner_radius=16.0`），
掩码无中间态而抗锯齿恰好活在被切掉那圈 → 断续粗灰点。
窗口仅 36px 高、半径 16 → 上下弧几乎相接近胶囊形，**阶梯在近垂直弧段最扎眼**。

回退 `None` → 软边回来但楔形也回来。`LWA_COLORKEY`、单态临时切 `UpdateLayeredWindow`
两条绕法均已否决。**唯一真解 per-pixel alpha，必须等八态全迁完。**

**Gavin 决定**：不为此单独出包二选一（原话「先不试了，按照你建议来」）。
→ **下一批直接冲 P2+P3 五态合并迁 D2D，迁完立刻接 per-pixel alpha**，DEC-057 排期提到最优先。

### 🔴 D2D-HANG-095 · 根因修复（coder-2，2026-09-05 派发，进行中）

**根因已由 REPRO-094 实证锁死，不是推测**：Rust `thread_local!` 析构器在 Windows 上
运行于 `DLL_THREAD_DETACH`，**加载器锁已被本线程持有**；此时做 D2D/DWrite 最后一次
COM Release → **自持加载器锁死锁**（BS4 抓到 `LoaderLock.OwningThread` = 挂死线程自身 tid）。

- 探针 A 独立 exe 主线程 **正常**（11ms）／ B 子线程退出 **挂死**在 `[Drop 6/6] brush Release`
  ／ b2·b5 线程体内释放 **正常** ／ b6 反向序 **照样挂**（与顺序无关）／ b3 `CoInitializeEx` **不救**
- **端侧「托盘退出无响应」同源路径**：`shutdown_and_join()` `:937-947` 主线程 `join.join()`
  死等 overlay 线程结束 → 它结束前必跑那批析构 → 主线程永不返回
- **修法**：`mod d2d` 加 `release_resources()`（`take()` 就地 drop）+
  `spawn_overlay_thread` 线程闭包尾部调用（闭包是唯一覆盖全部 `?` 早返回路径的位置）
- **硬判据**：产出源盘点（还有哪些线程会写这个 thread_local），答不出不许交付
- 任务书：`collab/inbox/coder-2/task.md`

### 在途任务（2026-09-05）

| Worker | 任务 | 阶段 | 状态 |
| --- | --- | --- | --- |
| tester-1 | **REPRO-094** D2D-HANG-001 根因定位 | 定位 | 🔄 探针 A/B/b2/b3/b5/b6/BS4 已出结论，探针 D 端侧复测进行中 |
| coder-2 | **D2D-HANG-095** 根因修复 | 阶段一 | 🔄 方案已同意，执行中 |
| coder-1 | — | — | 待命 |

**边界评估**：coder-2 占 `src/main.rs` + `docs/MACOS-HANDOFF.md`；
tester-1 占 `collab/outbox/tester-1/repro094/`（独立 cargo 工程，不入库）。**文件级零重叠。**
`collab/troubleshooting.md` 本批归 **tester-1**（证据在它手里），coder-2 禁写，防 `[WORKER-DOC-OVERWRITE-001]`。

**下一棒**：coder-2 验收通过 → 阶段三 `TEST-SYNC-096`（tester-1：去掉两条 `#[ignore]` +
在测试里显式调 `release_resources()` + 加护栏）→ 阶段四全量回归 → 阶段五出包。

### 🔴 REPRO-094 · D2D-HANG-001 根因定位（tester-1，2026-09-05 11:2x 派发，进行中）

**Gavin 2026-09-05 指示「派吧」**。P0：NULL HDC 调 D2D 在 `cargo test` 内挂死，
两条用例现靠 `#[ignore]` 绕过；疑与端侧「点托盘退出无响应只能 kill」同源。

- **本单是定位不是修复**：产出证据 + 根因判定，`src/**` `ui/**` 零改动
- **四探针**：A 主线程复刻 `create_resources` ／ B 子线程 + `join()`（验 thread_local COM Release 阻塞）
  ／ C `cargo test` harness 内对照（验环境差异）／ D 端侧托盘退出 ×5 复测
- **四假设**：H1 线程退出 COM Release 阻塞 ／ H2 `CreateTextFormat` 字体集合阻塞
  ／ H3 test harness 并行 + DWrite SHARED 单例锁 ／ H4 全进程无 `CoInitialize`
- **主控实读取证**：`create_resources` 在 `src/main.rs:2973-3045`，
  「CreateDCRenderTarget 之后」= 阻塞落在 `CreateTextFormat` 或其后；
  `thread_local` D2D（:3047）在线程尾部 drop，而 `shutdown_and_join()`（:937-947）
  主线程 `join.join()` 阻塞等它 —— 这是「托盘退出无响应」的直连路径；
  全仓 `src/` 下 `CoInitialize` 命中数 **0**
- **下一棒**：定位结论出来后派 `D2D-HANG-001-FIX` 给 coder-2（届时 tester-1 转阶段三 TEST-SYNC）
- 任务书：`collab/inbox/tester-1/task.md`

### ⚠️ 2026-09-05 主控 session 启动发现

- **tester-1 模型余额耗尽**：OpenCode Zen（DeepSeek V4 Flash）`Insufficient balance`，
  启动上下文注入失败。主控已切至 **GLM-5.3-Flash (2x usage) / OpenCode Go**（与两个 coder 同源）后恢复。
  → 复发即查此条，不要误判为 Worker 僵死（对照 `[WORKER-RESTART-MODEL-RESET-001]`）
- **handoffs.md 已归档**：299 → 153 行，09-03 的 9 条移入 `handoffs-archive.md`
- **本地领先 origin/main 7 个提交未 push**，等 Gavin 明确指示

### ✅ 阶段五 BUILD-093 已出包（tester-1，2026-09-05 00:37）—— 待 Gavin 端测

- **BUILD-093**：BUILD-085 后五批（OVERLAY-086/D2D-P1/REFACTOR-088+089/测试护栏）进 exe，七项核验全 PASS（sha 全异于 085、ProductVersion 0.9.0.0、探针 D2D-P1×2+D2DERR_RECREATE_TARGET×1）；E2E 64P/**1F**（test_hotkey_toggle_stop 间歇性 FAIL，主控裁定不阻塞出包）；冒烟无 panic；产物在 `Publish/`
- **Gavin 端测清单**（主控已列）：①宽度扩展丝滑度 ②流式空窗口（开嗓瞬间）③处理中圆角灰线 ④D2D 文字/边沿细腻度 ⑤语音热键录 AltGr ⑥文字超屏宽 65% 省略号行为变更 ⑦**toggle 连按两次能否正常停**（E2E 间歇 FAIL 的实机验证点）
- **D2D-HANG-001**：NULL HDC 调 D2D 挂死（CreateDCRenderTarget 之后），Gavin 授权跳过出包阻塞；P0 待办，疑与端侧托盘退出无响应同源

## 🔴 2026-09-04 —— 当前状态（最新在最上）

### ✅ 阶段四 TEST-EXEC-081 收口（tester-1）—— 全量回归通过 + TEST-FIX-084 注释改正完成，等主控出包指令

- **TEST-EXEC-081**（验收通过，主控裁定）：root 1054P/0F（4 条修红转绿）、src-tauri 76P、vitest 89P/0F（S12 红转绿）；必查 A/B/C 全过；消融 B 预期命中，消融 A 停手报告 → 主控裁定归因成立、判红基准改 S12（078 护栏判别力由 S12 实测证实）；pytest 按任务书 SKIP（Publish/ 旧包，E2E 门禁挪阶段五出包后）
- **TEST-FIX-084**：T4b 消融注释改正（+5/-3 仅注释，it() 零触碰）；另 4 条（T1/T2/T3a/T3b）消融推演逐条复核全部成立零改动；tsc 0 error
- 详情见 CHANGELOG.md TEST-EXEC-081 / TEST-FIX-084 ／ handoffs.md 2026-09-04 ／ logs/20260904.md
- ~~下一棒：**阶段五 BUILD v0.9.0**~~ → **BUILD-085 已完成**（2026-09-04 14:23，七项核验全过、ProductVersion 0.9.0.0、E2E 65P/0F 与 BUILD-022 逐位一致，产物在 `Publish/`）

### 🔴 BUILD-085 已出包 —— 待 Gavin 端测三重点（主控 2026-09-04 列）

1. 🔴 处理中态 overlay 文字与圆角边沿是否变细腻（DEC-055 红线 5，D2D-073-P0 验收判据，**不接受静态论证结案**）—— 达标才继续迁剩余五态
2. AltGr 当热键：语音侧/翻译侧各录一次，应显示 `Right Alt` 而非 `Left Ctrl`
3. LLM 401 若仍在 = DeepSeek key 吊销后新 key 未配置，**不是格式化逻辑坏了**，别误判 FMT-072

### 阶段三 TEST-FIX-080 ✅ 主控验收通过（tester-1）

- 四条错测试修正 + 5 条 AltGr 护栏，`+190/-42` 全在测试模块内，**生产代码零改动**（逐 hunk 行号核对）
- 主控独立验证：`cargo fmt --check` exit 0 ／ `cargo check --all-targets` **0 error** ／ `npx tsc --noEmit` **0 error**
- 详情见 CHANGELOG.md TEST-FIX-080 ／ handoffs.md 2026-09-04 ／ logs/20260904.md
- 🔴 tester-1 漏更 CHANGELOG／handoffs／todo 三处（`[DOC-STATE-DRIFT-001]` 第四次复发），**已由主控回填**；
  `progress.md` 按其规则 7「测试同步任务不记录」属 N/A，不算漏。
  阶段四任务书已把「五文档逐条打钩」写进**完成判据**，不再只靠通用规则。

### ✅ Gavin 已处理：DeepSeek key 已吊销（2026-09-04）

**SECRET-077 止血完成。** 泄露的 key 已在服务商后台吊销 —— 这是唯一的真止血，
`f58af96` 历史里的明文从此是**失效字符串**，不再是安全问题。

剩余两项**降级为卫生/可用性问题，不再阻塞**：

1. 是否重写 git 历史 + force push 清除痕迹 —— 破坏性操作，**未获授权不执行**，Gavin 说了算。
2. **LLM 401 是否已解决** —— 新 key 配上后需确认 LLM 可用，否则 FMT-072「有序列举格式化失败」
   出包端测时仍验不了（该项真因就是 LLM 根本没调用成功，不是格式化逻辑坏了）。
3. 每个新 clone 需执行一次：`git config core.hooksPath scripts/git-hooks`（钩子本体已入库不会丢）。

### 在途任务（2026-09-04）

| Worker | 任务 | 阶段 | 状态 |
| --- | --- | --- | --- |
| tester-1 | **TEST-EXEC-081** 全量回归 + TEST-FIX-084 注释改正 | 阶段四 | ✅ 完成（回归逐位吻合 + 消融偏差裁定收口 + 注释改正） |
| coder-1 | **SECRET-082 + FIX** pre-push 密钥/隐私闸门 + `.gitattributes` CRLF 修复 | 独立基建 | ✅ 主控验收通过（13/13 用例独立复验） |
| coder-2 | — | — | 待命（回归期间不得动 src/ 与 ui/） |

**边界评估**：tester-1 占 `src/**`、`ui/**`；coder-1 只碰 `scripts/git-hooks/**` + `.gitattributes`
+ troubleshooting/worker-guide 文档。**文件级零重叠**，可并行。

#### SECRET-082 的由来（Gavin 2026-09-04 问「push 会扫吗」）

主控查证结论：**不会。闸门在 commit，不在 push。**
`core.hooksPath=scripts/git-hooks` 已生效，但目录里只有 `pre-commit` 一个文件，
`.git/hooks` 无自定义钩子 —— **push 这一步是裸奔的**。

两个缺口：① 只认密钥形态（`sk-`/`ghp_`/`AKIA`/`AIza` 等），**邮箱/手机号/身份证一概不拦**；
② 只在 voice-ime 仓库生效，CodeLab 下其他仓库没配。

**附带发现的真隐患（一并修）**：`core.autocrlf=true` + 无 `.gitattributes`
→ 新 clone 会把钩子脚本检出成 CRLF → `bad interpreter: ^M` → **钩子静默失效**。
当前机器工作区副本恰好还是 LF 所以没暴露，但任何人新 clone 一次闸门就是坏的。

**硬性前提（已取证）**：`origin/main..HEAD` 25 个待推提交，密钥/邮箱/手机号/身份证命中 **全 0**；
`f58af96`（泄露那个提交）**已在 origin/main 内**，不落待推范围 —— 这是设计如此，
否则它会让此后每一次 push 被永久拦死。装完钩子 `git push --dry-run` 必须放行。

### ✅ v0.9.0 端测第一轮 —— Gavin 已完成（2026-09-04）

🔴 **主控状态跟踪更正**：此前 todo 把「Gavin 端测」列为待办，**错了**。
Gavin 2026-09-04 明确：「0.9.0 我已进行了一轮端侧，刚提交的三个问题就是测试结果」。
**端测已完成，OVERLAY-086 的三个缺陷就是这一轮的产出。**

#### 本轮结论

| 项 | 结论 |
| --- | --- |
| **D2D-073-P0 处理中态**（DEC-055 红线 5） | ✅ **通过**。Gavin 原话「整体效果满意」，D2D 方向确认，剩余状态迁移已排 P1/P2/P3 |
| 处理中态圆角灰线 | ⚠️ 收尾瑕疵 → OVERLAY-086 Bug 1（**不阻塞迁移**，他明说整体满意） |
| 流式文字中断/空窗 | 🔴 → OVERLAY-086 Bug 2 |
| 宽度扩展抖动/左移 | 🔴 → OVERLAY-086 Bug 3（**他要两边同时扩展 + 丝滑**，居中锚点保持不变） |

**能推出的事实**：Bug 2/3 都在流式文字上屏路径上，说明本轮**真实使用了流式 ASR 听写**，
且用到了「文字长度超出初始窗口宽度」的场景。这条路径是被实际跑过的。

#### 本轮未被提及、状态仍未知的两项（**不是失败，是没有信息**）

| 项 | 状态 | 说明 |
| --- | --- | --- |
| HOTKEY-079 翻译侧 AltGr | ✅ **通过** | Gavin 2026-09-04 补充确认：「热键设置界面我测试了翻译热键，目前正常了」。079 结案 |
| HOTKEY-078 语音侧 AltGr | ❓ 未知 | 他只点名了翻译热键。语音侧是另一条代码路径（组合键语义 + `altGrSynthCtrlActiveRef` 旗生命周期），**不能由翻译侧通过推断语音侧也通过**。下一轮端测顺手录一次语音热键即可 |
| FMT-072 / LLM 可用性 | ❓ 未知 | DeepSeek key 今日已吊销，新 key 若未配置则 LLM 401，格式化全线不生效。**这一项在新 key 配好前无法验证**，不是本轮遗漏 |

**主控不再把「端测」整体列为待办**；上述两项按单项跟踪，不重复催。

### 📦 下一次出包的范围（Gavin 2026-09-04 指示）

Gavin 原话：「**等这次所有任务完成出包，我再进行下一轮端侧**」——
即他不想为零碎改动反复端测，要攒成一个有分量的包再测。

**主控建议范围：OVERLAY-086 + D2D P1，合成一个包。**

理由：086 的 Bug 2/3 与 D2D P1 **落在同一条代码路径**（`RecordingWithText`）。
拆成两包意味着同一段代码改两遍、端测两轮，第二遍还可能把第一遍刚写的推翻。

🔴 **但必须防住 DEC-055 红线 4 警告的「归因不可能」**，做法是**串行 + 中间 commit 隔开**：

```
① coder-2 交付 OVERLAY-086 → 主控验收 → 单独 commit（这是二分锚点）
② 在该 commit 之上派 D2D P1 → 验收 → 单独 commit
③ 一次出包，两批都在里面
```

这样端测若发现异常，可用 `git bisect` / 逐 commit 回退定位到底是 086 的运动逻辑
还是 P1 的 D2D 绘制。**两批视觉特征本就不同**（086 = 位移/空窗；P1 = 文字锐度/边沿），
现场大概率能直接分辨。

**若 Gavin 更想早点拿到 086 的修复**，也可以 086 单独出一包先端测 —— 代价是
P1 之后要再出一包、再端测一轮。**这个取舍归 Gavin 定，主控默认走合成方案。**

### 🎬 OVERLAY-069 · 上屏文字窗口收缩淡出关闭（Gavin 2026-09-04 追问后排入）

**现状取证（主控实读代码，非凭记忆）**：**未实现，零代码**。
`OverlayCommand::Hide` 分支（`src/main.rs:1449-1495`）清完状态直接
`ShowWindow(hwnd, SW_HIDE)` —— 一句硬隐藏、瞬间消失，全文件无任何 fade/淡出/收缩逻辑。
文档侧一致：`CHANGELOG.md` 与 `progress.md` **零条 069 记录**（= 从未完成），
git log 里唯一提到 069 的提交是 DEC-055 的文档更正，不是实现。

**为什么拖到现在**：069 是 Gavin 2026-08-30 端测 13 项之一，与 OVERLAY-062（字体粗糙）、
OVERLAY-065（动态麦克风图标）被判**同源** —— 都卡在 GDI 管线上限（CPU 栅格化 + 位图搬运，
逐帧重绘成本高，做不了高频低成本动画）。DEC-055 当时决定**不在 GDI 上打第三次补丁**，
先迁 D2D 再做动画。**是刻意排序不是遗漏**，只是迁移本身被 ASR-074 / OVERLAY-075 /
HOTKEY 系列插队拖了一个月。

#### 排期位置

```
OVERLAY-086（三缺陷）
  → D2D P1（RecordingWithText 迁 D2D）   ← 069 的地基
  → OVERLAY-069（收缩淡出关闭）           ← 本条
```

#### 🔴 派发前必须先定的技术前提（**不许现在写死方案**）

窗口现为 `WS_EX_LAYERED` + `LWA_ALPHA`（**整窗统一透明度**，`src/main.rs:1340` 附近）。

| 效果 | 依赖 | 现在能不能定 |
| --- | --- | --- |
| 纯淡出（只降 alpha） | 直接调 `SetLayeredWindowAttributes`，D2D 之前就能做 | 能 |
| **收缩 + 淡出**（Gavin 要的） | 尺寸与 alpha 同时变 → **必须接上宽度插值那套机制** | ❌ **不能** |

🔴 **而那套插值机制 OVERLAY-086 Bug 3 正在重写**（`:1666` 变宽 snap 改插值 + `:1334`
流式分支 snap 的处置）。**069 的实现方案取决于 086 最终把插值改成什么样。**

**所以本条的派发条件是：086 交付并验收后，主控依据其最终形态出方案再派。**
现在写死方案 = 大概率白写，且会导致**两套动画逻辑并存迟早打架**（复用同一套插值是硬要求）。

#### 复用要求（写进将来的任务书）

- 复用 086 定型后的插值机制，**禁止另起一套动画循环**
- 关闭动画期间的 `SetWindowPos` 落点必须唯一（OVERLAY-068-B 的老教训：两处争抢 = 闪烁）
- 不得动 `InvalidateRect` 的 `bErase=false`（OVERLAY-043 红线）
- `Hide` 分支现有的两条保护不许破坏：ASR-038-C（编辑态不许拆 EDIT 控件）、
  OVERLAY-054-A（`RestoreAndHide` 显式允许拆）

### 🎨 per-pixel alpha 改造（DEC-056，Gavin 2026-09-04 拍板）

Gavin：「效果一定好，视觉体验一定要棒，这关乎到用户体验，现在硬件性能很强大」。
**视觉是产品目标，不是性能预算的余数。** DEC-055 补充二「帧率 < 30fps 才换路线」判据**已作废**。

🔴 **主控原先把这条路线定性错了**：它不是性能优化，是**视觉能力的天花板**。
`LWA_ALPHA` 整窗统一透明度下，圆角要么留背景楔形（= OVERLAY-086 Bug 1 本体），
要么被 `SetWindowRgn` 二值掩码切成硬阶梯（= DEC-055 要消灭的「粗糙」）。
**两条路都到不了「棒」，与帧率无关，再快的机器也解不开。**

#### 顺序是硬的，不可调换

```
OVERLAY-086      → Bug 1 只能做到「消除楔形」，圆角仍非真抗锯齿（已通知 coder-2 下调目标）
D2D P1/P2/P3     → 八状态全迁完，GDI 绘制清零
per-pixel alpha  → UpdateLayeredWindow 改造        ← Bug 1 的彻底解
OVERLAY-065/069  → 动态图标 / 收缩淡出
```

🔴 **为什么必须等 D2D 全迁完**：GDI 绘制函数（含 `DrawTextW`）不维护 alpha 通道，
会把写过的像素 alpha 置 0，在 `UpdateLayeredWindow` 合成时**该区域整块透明消失**。
残留一处 GDI 绘制，整个 per-pixel alpha 改造就是坏的。

**这给「尽快完成 D2D 迁移」加了一条新理由**：它不再只是为了字更清楚，
而是 per-pixel alpha 的**前置条件**。迁移不完成，视觉天花板打不破。

#### 派发前待实证（勿凭推断直接派）

主控倾向判断**可能不需要新增 Cargo feature**（32bpp DIB section +
`ID2D1DCRenderTarget` 以 `ALPHA_MODE_PREMULTIPLIED` 绑其 DC + 全程 D2D 绘制 +
`UpdateLayeredWindow`，现有 Direct2D/DirectWrite 已足够）。
**但这是推断不是实证**，派发前必须实测；若确需新增 feature 属依赖变更，
按 worker-guide 第十节必须派 BUILD 给 tester-1。

#### 验收判据（DEC-055 红线 5 沿用，且更明确）

**圆角边缘在浅色背景上不得看出阶梯或色块** —— 必须 Gavin 目视确认，不接受静态论证。

### 🔴 D2D 迁移排期（Gavin 2026-09-04 指示：「尽快完成迁移到 D2D」）

**前置已达成**：Gavin 端测处理中态后原话「**整体效果满意**」= DEC-055 红线 5
（视觉必须目视确认）**已过**，D2D-073-P0 试点验收通过，方向确认可继续。
遗留的圆角灰线瑕疵走 OVERLAY-086 Bug 1，**不阻塞后续迁移**。

#### 剩余待迁状态（`draw_overlay_to_dc` 共 8 个分支，已迁 1）

| # | 状态 | 现状 | 备注 |
| --- | --- | --- | --- |
| — | `Processing` | ✅ **已迁**（D2D-073-P0） | 保留 GDI 兜底，Gavin 已目视确认 |
| 1 | `RecordingWithText` | GDI | 🔴 **优先级最高**：流式文字上屏，OVERLAY-062「字体粗糙」的正主；且 OVERLAY-086 Bug 2/3 都在这条路径上 |
| 2 | `Recording` | GDI | 录音态 + 波形；OVERLAY-065 动态麦克风图标的地基 |
| 3 | `RecordingStreamingIdle` | GDI | 与 1 同族，建议同批 |
| 4 | `FallingToProcessing` | GDI | 过渡态，与 Processing 相邻，建议紧跟 Processing 之后 |
| 5 | `StreamingEditing` | GDI | ⚠️ 只迁 chrome + 提交按钮；**EDIT 子控件本体不许动**（DEC-055 红线 2） |
| 6 | `FocusLost` | GDI | 含复制/关闭按钮 |
| 7 | `Error` | GDI | 最简单，可与其他批搭车 |

#### 🔴 排期已改为两轮（DEC-057，Gavin 2026-09-04 拍板「按两轮走」）

原三批 P1/P2/P3 各出一包各端测一轮 + per-pixel alpha 一轮 = 这一包之后还有三轮。
Gavin 问「还要再走三轮端侧？」，主控评估后建议压到两轮，他拍板采纳。

| | 新排期 |
| --- | --- |
| ① | **086 三缺陷 + D2D-P1** → 出包 → 端测（**马上，不压这一包**） |
| ② | **P2 + P3 合并**（Recording / FallingToProcessing / Error / StreamingEditing / FocusLost 五态一次迁完）→ 出包 → 端测 |
| ③ | **per-pixel alpha**（DEC-056）→ 出包 → 端测 |

**为什么能压**：P1 已把共用层 + 五原语（chrome / mic / stop / placeholder / streaming_text）
抽出来，P2/P3 剩余五态主要是复用，真正新写的只有波形、提交键、复制/关闭键三个。
归因难度反而降低——出问题大概率在共用原语层，一坏坏一片，特征明显。

🔴 **per-pixel alpha 仍必须单独一批**，不许并进 P2+P3：它是换合成方式不是迁状态，
且必须等八态全迁完（残留 GDI 绘制会在预乘 alpha 位图上打 alpha 洞）。

🔴 **DEC-055 红线 5 不受影响**：每批出包后视觉仍必须 Gavin 目视确认，
本次放宽的是分批粒度不是验收标准。

#### 🔴 排期约束（DEC-055 红线 4：禁止一次全改）

**必须分批灰度，每批出包后 Gavin 目视确认再开下一批**，理由：一次全改则回归归因不可能。

建议分三批：

| 批次 | 内容 | 理由 |
| --- | --- | --- |
| **P1** | `RecordingWithText` + `RecordingStreamingIdle` | 用得最多、Gavin 感知最强；与 OVERLAY-086 同路径，**必须等 086 落地后再动，否则文件冲突** |
| **P2** | `Recording` + `FallingToProcessing` + `Error` | 录音态打通后 OVERLAY-065 动态图标才有地基 |
| **P3** | `StreamingEditing` + `FocusLost` | 按钮/交互最多，放最后 |

#### 🔴 派发前必须先做的两件事（否则 P1 开不了工）

1. ~~`Cargo.toml` 补 feature~~ ✅ **已核查完毕（主控 2026-09-04 实读，非照抄附录）**
   —— DEC-055 实施附录写的「Direct2D / DirectWrite 一个都没有」是 **08-30 快照，已过期**。
   D2D-073-P0 那批已补齐四个：`Win32_Graphics_Direct2D` / `_Direct2D_Common` /
   `_Dxgi_Common` / `_DirectWrite`，均在 `[target.'cfg(target_os = "windows")'.dependencies]`
   段内（天然不影响 macOS 构建）。**P1/P2/P3 走 BindDC 路线无需再加 feature，不构成依赖变更。**
   ⚠️ 唯一例外：若将来改走 `UpdateLayeredWindow` + DXGI 表面（per-pixel alpha），
   需补 `Win32_Graphics_Dxgi` + `Win32_Graphics_Direct3D11` —— 那才是依赖变更，
   按 worker-guide 第十节**必须派 BUILD 给 tester-1**，coder 不得自行构建。

2. **抽出 P0 的可复用地基**
   —— 当前 `d2d` 模块只有 `draw_processing_overlay` 一个函数 + 一套 `D2dResources`。
   迁 7 个状态前应先评估：`create_resources` / `BindDC` / 画刷 / 文本格式
   要不要抽成共用层。**这是架构决策，派发前主控给方案，不许 Worker 边写边攒。**

#### 顺带一并解决的既有缺口

| ID | Gavin 原话 | 依赖哪一批 |
| --- | --- | --- |
| OVERLAY-062 | 编辑态文字粗糙、提交按钮边沿粗糙 | P3（StreamingEditing） |
| OVERLAY-065 | 替换左侧麦克风图标为动态图标 | P2（Recording）打通后另开单 |
| OVERLAY-069 | 窗口关闭要流畅丝滑，不要生硬突然关闭 | 🔴 **已单列排期，见上方专节** —— 位置在 P1 之后（不是 P2/P3 之后），因为它的地基是 RecordingWithText 的 D2D 化 + 086 定型后的插值机制 |

#### 当前阻塞

**P1 必须等 OVERLAY-086 完成**：086 改的 `RecordingWithText` 绘制路径与宽度插值，
与 P1 是同一批代码。两个任务同时开 = 文件级冲突，违反边界评估规则。

### 下一棒顺序（五阶段串行，禁止并行）

```
阶段三 TEST-FIX-080   ✅ 已验收
阶段四 TEST-EXEC-081  ← 当前，全量回归（tester-1）
阶段五 BUILD v0.9.0      测试无明显问题后主控下达「现在可以出包」
       → Gavin 端测
```

**coder-1 / coder-2 阶段四期间保持待命**：工作区躺着未提交的测试改动，
任何人动 `src/main.rs`／`src/audio/mod.rs`／`src/itn.rs` 都会污染回归结果。

🔴 **出包后必须 Gavin 目视确认**：处理中态 overlay 的**文字与圆角边沿是否变细腻**
（DEC-055 红线 5，不接受静态论证结案）。他说达标才继续迁剩余五个状态；不达标则 D2D 方向需重议。

### TEST-EXEC-076 五条 FAIL —— 处置全部闭环

| # | 用例 | 处置 | 状态 |
| --- | --- | --- | --- |
| ① | `itn_071b_legal_time_yidianban_still_converts` | 改测试 → `下午1:30` | ✅ TEST-FIX-080 |
| ② | `itn_071b_legal_time_yidianshiwufen_still_converts` | 改测试 → `1:15` | ✅ TEST-FIX-080 |
| ③ | `asr_074_stop_drain_...abandoned_count` | 改测试（整条重写，慢消费建模） | ✅ TEST-FIX-080 |
| ④ | `stale_generation_must_not_touch_mirror...` | 改测试（改名 + 末条期望修正） | ✅ TEST-FIX-080 |
| ⑤ | `HOTKEY-047-S12` | **真生产缺陷，改生产** | ✅ HOTKEY-078（+ 079 翻译侧同型） |

**①② 的关键事实（别再翻）**：生产零 bug，ITN-071-B 的两条保护用例（一点半点／一点点）都是 PASS 的。
红的只是顺手写的两条回归护栏，期望值凭直觉写成「1点半」，与既有十余条 `X点半→X:30` 护栏冲突。

**④ 的语义裁定（重要，别再翻）**：代际闸门 = 数据+渲染双拦；043 闸门 = **仅渲染**。
同 session 松手后的迟来包是同一句话的更完整版本，**必须**进 `last_streaming_text` 镜像，
否则 WORDBOOK-053-B 会拿截断的 raw 文本去 diff 用户编辑，学出用户从未做过的伪修正。
**渲染抑制 ≠ 数据抑制。**

### 已结案（不要重查）

- **行尾幻影**：coder-2 上报的「69 文件 18K 行 CRLF 漂移」= racy-git 的 index stat 缓存过期，
  一次 `git status` 刷新即自愈。裁定 `core.autocrlf=true` **不动**。
- **OVERLAY-061 冷启动假设**：❌ **证伪**（COLD 3199 帧 + WARM 9831 帧 = 13030 帧，可见态位置异常
  零命中）。061 不是 overlay 窗口的问题，下一步是 REPRO-061-ALLWIN 全窗口清扫（任务书未写）。
- **068 宽度阶梯归属**：`y≈1292 ∧ h=36` 的宽度阶梯族**属 068 不属 061**，两 bug 不得互相污染。
- **068-B 覆盖面**：**不返工**。拉锯真源是跨 session 渗漏，已由 OVERLAY-075 结构性根治。

---

## ✅ 2026-09-03 收工状态 —— 下一棒从这里开始

### 已完成并提交（工作区干净，**本地领先 origin/main 16 个提交，未 push**）

| 提交 | 内容 | 验收 |
| --- | --- | --- |
| `8fbb913` | **ASR-074** 音频上行背压丢帧根治（coder-1）：2-A 排空 chunk_rx / 2-B 松手 drain 带 500ms 上限 / 2-C 丢帧计数 + 三个埋点缺陷 | 主控逐项 Read + 独立 cargo check |
| `8023cc6` | **OVERLAY-075** 跨 session 代际隔离 + **D2D-073-P0** 处理中态迁移 + **ASR-074-GUARD** send_timeout + **HOTKEY-060** 补账 + 主控代修 macOS 元数（coder-2） | 四条红线 diff 逐个 grep 核对 |

### 在途

| Worker | 任务 | 状态 |
| --- | --- | --- |
| tester-1 | **TEST-SYNC-074/075/D2D**（阶段三，只写用例不跑） | 执行中 |
| coder-1 | — | 待命 |
| coder-2 | — | 待命（main.rs 已交给 tester-1，未经协调不得动） |

### 下一棒顺序

阶段三交付 → **阶段四全量回归** → **阶段五 BUILD v0.9.0** → Gavin 端测。

🔴 **出包后必须 Gavin 目视确认的**：处理中态 overlay 的**文字与圆角边沿是否变细腻**（DEC-055 红线 5，
不接受静态论证结案）。他说达标才继续迁剩余五个状态；不达标则 D2D 方向需重议。

### 🔴 两条待 Gavin 处理（主控无权限 / 需授权）

1. **DeepSeek API key 泄露**（详见 troubleshooting `[SECRET-IN-REPO-001]`）：
   `collab/research/ab033-components.json:17` 明文 key，`f58af96`（08-14）已 push 到 GitHub，
   暴露 20 天，Gavin 已确认遭第三方盗刷、有金钱损失。
   **主控已完成全仓扫描（仅此一处）并落盘经验，但吊销 key 只能 Gavin 在服务商后台做。**
   待 Gavin 决定的两项：① 是否把仓库内该 key 替换为占位符并提交；
   ② 是否重写 git 历史 + force push（**破坏性操作，未获授权前主控不执行**）。
2. **LLM 401 导致格式化全线未生效**：Gavin 提到「暂时关闭格式化输出」，语义待确认。
   **FMT-072「有序列举格式化失败」的真因已查明 = LLM 根本没调用成功**，
   出包端测前需确认 LLM 可用，否则该项无法验证。

### 今日已证伪 / 已结案（不要重查）

| 项 | 结论 |
| --- | --- |
| **OVERLAY-061 冷启动假设** | ❌ **证伪**。COLD 3199 帧 + WARM 9831 帧 = **13030 帧，可见态位置异常零命中**；单 HWND 无重建、GetWindowRect 零失败。两处 `ShowWindow` 均在 `SetWindowPos` 之后（主控 Read 核对 + 7 帧实测佐证）。**061 不是 overlay 窗口的问题**，下一步是 REPRO-061-ALLWIN 全窗口清扫（任务书未写） |
| **068 宽度阶梯归属** | 主控裁定：`y≈1292 ∧ h=36` 的宽度阶梯族**属 068 不属 061**，两 bug 不得互相污染 |
| **068-B 覆盖面** | coder-2 核查后主控采纳：**不返工**。拉锯真源是跨 session 渗漏，已由 OVERLAY-075 结构性根治 |

---



## 🛑 2026-08-30 深夜收工存盘 —— 三 Worker 额度全部超限，下次从这里开始

> **恢复条件**：Gavin 的 Ollama 账号额度重置（coder-1 / coder-2 / tester-1 撞的是**同一个账号墙**，非各自故障）。
> **恢复动作**：~~三个 Worker 的 `inbox/*/task.md` 任务书都已写好并派发过，重启后直接 `dispatch <id>`（不带任务内容，仅通知重读）即可，不要重写任务书。~~
>
> 🔴 **2026-09-03 实测更正：上面这条作废。** 三个 `inbox/*/task.md` 在本次重启时**全部被清空为 0 字节**
> （mtime 2026-09-03 17:14），属 troubleshooting.md `[REPLACE-WORKER-TASKFILE-WIPED-001]` 的已知行为。
> **三份任务书必须重写**（ASR-074 / HOTKEY-060 收尾+D2D-073 / OVERLAY-061 冷启动复测），
> 直接 dispatch 只会让 Worker 读到空文件或凭记忆臆造任务。

### 现场（以 git 为准，非记忆）

| 项 | 值 |
| --- | --- |
| HEAD | `d979c95` |
| 工作区 | **完全 clean**，零悬空改动，零未跟踪文件 |
| 版本号 | **v0.9.0**（VERSION-059 已落地） |
| 本 session 提交数 | 9（`9c9ff73` → `d979c95`） |
| 未 push | 🔴 **本地 ahead，Gavin 未授权 push，不得自动执行** |

### 本 session 已完成并提交

| ID | 内容 | 提交 |
| --- | --- | --- |
| VERSION-059 | 版本号 0.8.1 → 0.9.0（Gavin 明确指示） | `9c9ff73` |
| OVERLAY-068 | 位置跳动（两处）+ 交替闪烁；061 屏幕外坐标安全网；064 根因报告 | `920e1d2` |
| ASR-070 | `final_text()` 改返回 confirmed + current | `958cadb` |
| ITN-071 | `三五成群` 补入 idioms + 护栏 2 条；qwen_inference 死代码化简 | `9506eac` |
| ITN-071-B | `一点半点` 挪 idioms、`一点点` 补 function_words + 护栏 4 条 | `e7a6a29` |
| **HOTKEY-060** | 🔴 **见下方「需注意」第 1 条** | 混在 `9506eac` + `e7a6a29` 内 |
| 文档 | ASR-067 澄清与实测分析 / DEC-055 + 实施附录 / REPRO-073 验收 + ASR-074 合并 / 经验条目 | `1628e27` `a53fffb` `a0752e6` `d979c95` |

### 🔴 需注意（下次接手前必读）

**1. HOTKEY-060 已实现并已提交，但从未走完验收流程。**
主控 `git add -A` 时把 coder-2 的改动**误扫进了 ITN 的两个提交**（`9506eac` / `e7a6a29`），
提交信息里**只字未提**，且当时既没等它的完成通知、也没做 review。**这是主控的流程失误。**

事后补审结论（2026-08-30 深夜）：
- 设计**符合主控裁定**：`voiceFinalizedRef` / `translationFinalizedRef` **两侧各自独立**，
  共用 helper `applyHotkeyIfNoDupConflict(finalizedRef, side, ...)`，状态不串、流程只有一份 ✅
- 删除 `TRANSLATION_SINGLE_KEYS` **不是回归**：原代码 `if (!TRANSLATION_SINGLE_KEYS.includes(vkCode)) { applyTranslationHotkey(vkCode); return; } applyTranslationHotkey(vkCode);`
  —— **两个分支完全相同，本就是死代码**，删除行为等价 ✅
  （主控一度误判为回归，查原文后自行更正，记录在此以免下次重复怀疑）
- `npx tsc --noEmit` **0 error** ✅
- 🔴 **仍缺**：coder-2 的 result.md、五文档条目、阶段三 TEST-SYNC。**恢复后补齐，不要当它已验收过。**

**2. 出包尚未进行。** 五阶段停在阶段一。恢复后顺序：
剩余阶段一 → 阶段三 TEST-SYNC → 阶段四全量回归 → 阶段五 BUILD v0.9.0。

**3. `git add -A` 的教训**：本 session 因此把未验收改动混入无关提交。
恢复后提交前先 `git status` 看清改动归属，跨 Worker 的改动分开提交。

### 三个 Worker 的在途任务（任务书已在 inbox，直接通知重读即可）

| Worker | 任务 | 状态 |
| --- | --- | --- |
| **coder-1** | **ASR-074**（ASR-067 + ASR-070 合并单）：在线 ASR 会话中途停止产出 + finalize 拖尾 4-5s | 已 ACK 同意方案，额度中断，**零交付** |
| **coder-2** | ~~HOTKEY-060 收尾~~（✅ 2026-09-03 补账完成）→ ~~OVERLAY-075~~（✅ 2026-09-03 代码交付待验收）→ **D2D-073** 九阶段（P0 依赖+骨架 → P1 处理中态 → P2 错误态 → P3 录音态+波形+065 动态图标 → P4 流式文字态 → P5 编辑态边框 → P6 069 淡出 → P7 063 字号 → P8 066 高度+3px） | Part A+B 已交付，等主控 ACK 后开 D2D P0 |
| **tester-1** | **OVERLAY-061 冷启动定向复测**（无预热 × 5 次冷启动，抓前 300ms，判据=是否出现在 (0,0) 附近；用 `target/release` 旧包，它不含屏幕外兜底） | 已 ACK，额度中断，**零交付** |

**另需 coder-2 顺带确认**（已发消息，未答）：tester-1 实测显示交替闪烁主体是
「长文本 vs Recording 240 宽」拉锯，而 068-B 修的是「流式 → Processing 200」路径，
请核对覆盖面并给行号证据。

### Gavin 已拍板 / 已答复的事项（不要再问）

| 事项 | Gavin 结论 |
| --- | --- |
| 版本号 | 升 **v0.9.0** |
| overlay 渲染方案 | 选 **B：Direct2D + DirectWrite 重写**，否决 GDI+ 局部补丁（DEC-055） |
| D2D 排期 | **放进本次出包**（主控原建议延后，Gavin 否决） |
| FMT-072 有序列举格式化 | **暂时跳过，下次开单**，他要在端侧判定是否 LLM 造成 |
| ASR-067 | 原意等出包后取 debug.log；**但现行 `target/release` exe 已含 ASR-058 埋点，无需等出包即可查** |
| ITN `一点半点` 用例 | 输入「差的不是一点半点」→ 输出「差的不是1点半点」；「我只是有一点点不信」→「我只是有1点点不信」 |
| 显示器 | **没有多显示器**（故 061 未复现不能归因多屏） |
| 067 与 070 是否同时发生 | **是** → 已合并为 ASR-074 |

### 待 Gavin 端测验证的两条（出包后）

1. **ITN-071-B 判读方式已预先写死**：复测「差的不是一点半点」——
   **仍失败** = ITN 已在第 1 步保护却仍被改 → 根因在 ITN 之外（大概率 LLM 回改），ITN 洗清；
   **通过** = 症状消除，第 5 步疑点降为低优先级。
2. **OVERLAY-062/065/069/063/066** 视觉五项，D2D 迁移后逐状态目视确认。


### 🔴🔴 收工前最后一刻的重大更正：ASR-074 的证据链大半被污染

coder-1 断电前交出了 15:09:33 那次 run 的完整时间线（实测，价值很高），
主控顺着它复核，**发现三条证据里有两条不成立**。

**污染源：tester-1 的 REPRO 脚手架按住热键的时间远短于它播放的 TTS 句子。**

| 录音来源 | 时长样本 |
| --- | --- |
| Gavin 真实使用（04:xx / 09:xx） | 8.8s / 29.9s / 11.5s / 27.3s / 13.3s |
| **tester-1 测试（15:xx）** | **2.6s × 8 次、3.9s、4.1s** |

15:09:33 那次「尾部丢字」：口播「今天天气很好我们下午三点在公司门口集合」
（正常语速 5-6 秒），而 **`recording complete, 4.1s`** —— **尾部那半句从未被录进去**。
服务端返回不了它，不是因为服务端截断，而是**麦克风就没收到**。

**逐条重估：**

| 原证据 | 结论 |
| --- | --- |
| ① 主控：「词时间轴只覆盖录音 51.4%」 | 🔴 **不成立**。主控拿词表跨度比 `final_ms`，但 `final_ms` **含松手后等 finalize 的时间**。对 04:21 那次（真实录音 29.9s）：词表 24.5s vs 录音 29.9s，属正常。**分母用错了** |
| ② tester-1：「7 组尾部丢失 5 组，全部服务端未返回」 | 🔴 **大半不成立**。短按导致尾部未录入，服务端「未返回」是必然结果 |
| ③ `ignoring late StreamingText after stop` 持续 4-5s | ✅ **成立**，但 coder-1 已归因为**服务端处理慢**，非客户端缺陷 |

**幸存且更有价值的新发现（coder-1 实测）**：

> 15:09 那次：录音仅 4.1s，而 **`first_text_ms=7144`** ——
> **首个非空文本在音频开始后 7.1 秒才到，即松手之后 0.8 秒**。
> 录音全程（4.1s）服务端只回 `display=''`、`words=0`。
> stop(28.766) → finish-task(32.700) = **3.9s**，客户端一直在等服务端回吐。

**这直接解释了 ASR-067**：录音期间服务端根本还没产出任何文本，
**屏幕上当然什么都没有**——用户感知为「上屏显示中断」。
根因指向**服务端响应延迟极高**，而非客户端停止接收。

🔴 **但仍不能定案**，因为这一样是从被污染的短录音 run 里取的。
恢复后**第一件事**：请 Gavin 用 `-debug` 做一次**受控复现**（正常按住、说完再松手），
取 `[ASR-SUMMARY]` 的 `first_text_ms` / `final_ms` 与 `recording complete` 时长三者对照。

**教训**：本条正是 `[PLAUSIBLE-FIX-NOT-ACTUAL-CAUSE-001]` 的第四个实例
（当天第四次）——主控与 tester-1 都基于「讲得通」的数据建立了因果链，
**没有先校验数据本身是否可信**。已在该经验条目下补记。

### 🔴 遗留未解（不要当已解决）

| # | 问题 | 状态 |
| --- | --- | --- |
| 1 | `一点半点` 在生产中为何未被 check_protection 第 5 步保护住 | **无解**。词 2026-07-30 即入表、`unit_collision_map` 构建与查表逻辑经复核均正确。已改用「把修复当实验」的方式分流 |
| 2 | ITN 保护是**白名单模型**，漏一词出一 bug | Gavin 已撞四个（三五成群 / 一点半点 / 一点点 / **哪一款**——最后一个是主控在 debug.log 里发现、Gavin 尚未报过）。补词只能逐个堵，结构性泄漏未解决 |
| 3 | ASR-074 根因 | 未定。三份证据指向「会话中途停止产出 + finalize 拖尾」 |
| 4 | OVERLAY-061 真根因 | 未定。1044 帧未复现；冷启动假设待验 |
| 5 | `vad_hit_ms` 早期 run 正常(637/644)、tester-1 26 个 run 全 -1 | 🟡 已降级为待验证假设，环境可能有责（26 run 中 14 run 零识别） |

---


## 🎯 v0.9.0 批次 · Gavin 2026-08-30 端测清单（13 项）+ 版本升级 —— 本批汇报以此表为准

> **Gavin 明确要求：「接下来任务完成度，需要按照这些列表来汇报」。**
> 本表是 v0.9.0 唯一的完成度基准，每项状态变化立即回写，不批量补。
>
> **端测包取证**：Gavin 端测的是 `Publish/` 下 **2026-08-18 23:48** 构建的包
> （feiyin-ime.exe 12,188,160 B），晚于 TEST-EXEC-058 过闸提交 `6009aa3`（23:43）
> → **ASR-058 首字提速已在该包内**，该次出包未入账（编号补记为 BUILD-023-未入账）。
> 因此下列 13 项**全部是针对最新代码的真问题**，无一条属于「修了没进包」。
>
> 🔴 **Gavin 2026-08-30 原话确认**：「我重复提交的问题就是之前没有修复好」——
> 061 / 062 / 064 三项是 OVERLAY-054-B-FIX / 054-D / 054-C **修过但没修对**，
> 不是回归，是当初验收放过了。本批必须做到目视确认，不接受静态论证结案。

### 总表

> 🔄 **2026-09-03 21:30 主控按 git 逐项核对更新**（上一版停留在 08-30 深夜，已过期）。
> **全局前提：`Publish/` 仍是 2026-08-18 23:48 的旧包，自那以来 14 个提交一次都没出包
> → 下表任何 ✅ 都只代表「代码已进本地仓库」，Gavin 端没有一项验证过，无一项能标 🟢。**

| ID | Gavin 原话 | 类型 | 优先级 | 文件域 | 负责人 | 状态（2026-09-03 核对） |
| --- | --- | --- | --- | --- | --- | --- |
| **VERSION-059** | 升级版本到 v0.9.0 | 版本 | 前置 | `Cargo.toml` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` | coder-1 | ✅ 已提交 `9c9ff73` |
| **ASR-067** | 输入中间有停顿（约 1 秒），接着输入就无法继续，麦克风不接受输入 | BUG | 🔴 **P0** | `src/audio/mod.rs` + `qwen_inference.rs` | coder-1 | 🟡 **改判两次后已动手**：原「800ms 断句」假设证伪 → REPRO-073 实测定位到**音频上行背压丢帧**，并入 **ASR-074** 已提交 `8fbb913`。**是否真解决必须靠出包端测**，不接受静态结案 |
| **ASR-070** | 说完松开热键后，识别的文本不完整，被丢掉尾部文字 | BUG | 🔴 **P0** | `qwen_inference.rs` `final_text()` | coder-1 | 🟡 **修了一层，不确定够不够**：`958cadb` 修「已到达未断句」的尾部丢弃；但 REPRO-073 实测显示还有一层「服务端 word 流根本没送达」，那一层归 ASR-074 `8fbb913`。同样必须端测判定 |
| **ASR-074** | （067+070 合并单，主控立项）音频上行背压丢帧 + finalize 拖尾 | BUG | 🔴 **P0** | `src/audio/mod.rs` / `qwen_inference.rs` | coder-1 | ✅ 已提交 `8fbb913`（2-A 排空 / 2-B 松手 drain 带 500ms 上限 / 2-C 丢帧计数 + 三个埋点缺陷 + 永久 debug 埋点） |
| **ASR-074-GUARD** | （主控验收 ASR-074 时发现的残留风险）chunk_tx 阻塞 send 会导致松手卡死 | BUG | 🟠 P1 | `src/main.rs` | coder-2 | ✅ 已提交 `8023cc6`（send_timeout 200ms + warn 计数） |
| **OVERLAY-064** | 原来 overlay 窗口的灰色线条边框消失了，必须显示回来，否则窗口缺少质感 | BUG | 🟠 P1 | `src/main.rs` | coder-2 | 🟠 **只出了根因报告，代码没改**（`920e1d2` 内只有分析）。修复动作归 D2D 迁移批，**未做** |
| **OVERLAY-061** | 按下热键，录音窗口会先显示在屏幕左上角边沿，然后跳到屏幕下方正确位置 | BUG | 🟠 P1 | 待定（已不在 overlay 窗口） | 待派 | ❌ **假设证伪，未定位未修**。COLD+WARM 共 13030 帧采样，可见态位置异常**零命中**；`920e1d2` 只加了屏幕外坐标安全网（治标）。下一步 REPRO-061-ALLWIN 全窗口清扫，**任务书未写** |
| **OVERLAY-068** | 长文本时窗口位置跳动；切到「识别处理中」时长短两个窗口交替闪烁 | BUG | 🟠 P1 | `src/main.rs` | coder-2 | ✅ 已提交 `920e1d2`；「交替闪烁」的真源（跨 session 渗漏）由 OVERLAY-075 结构性根治 |
| **OVERLAY-075** | （主控立项）跨 session 流式文本渗漏：A 拖尾期间开 B → 宽窄拉锯 + A 旧文字渲染进 B 窗口 | BUG | 🟠 P1 | `src/main.rs` | coder-2 | ✅ 已提交 `8023cc6`（会话代际 AtomicU64 闸门，+44/-5） |
| **HOTKEY-060** | 录音、翻译热键设置时，重复拦截提示 | BUG | 🟠 P1 | `ui/src/pages/HotkeySettings.tsx` | coder-2 | ✅ 代码已在 `9506eac`+`e7a6a29`，补账已完成。🔴 **这份改动至今一次 Vitest 都没跑过**，正由 TEST-EXEC-076 补跑 |
| **ITN-071** | ITN 错误：`三五成群` / `一点半点` | BUG | 🟠 P1 | `src/itn.rs` + `itn-rules.toml` | coder-1 | ✅ 已提交 `9506eac`（三五成群）+ `e7a6a29`（一点半点挪 idioms、一点点补 function_words，护栏 4 条）。🔴 结构性问题未解：保护词表是白名单模型，漏一词出一 bug，Gavin 已撞四个 |
| **D2D-073-P0** | （DEC-055 落地）overlay 绘制层迁 Direct2D + DirectWrite，处理中态先行 | 优化 | 🟠 P1 | `src/main.rs` | coder-2 | ✅ 已提交 `8023cc6`（骨架 + 处理中态 + GDI 回落）。🔴 **出包后必须 Gavin 目视确认文字与圆角边沿是否变细腻**，他说达标才继续迁剩余五态 |
| **OVERLAY-062** | 点击进入编辑态，文字字体显示很粗糙，提交按钮的边沿也很粗糙 | 优化 | 🟠 P1 | `src/main.rs` | coder-2 | ⬜ **未做**，排在 D2D P5（编辑态边框） |
| **FMT-072** | 明显有序列举内容，格式化输出失败 | BUG | 🟠 P1 | `src/llm/mod.rs` | — | ⏸ **未做**，Gavin 拍板挂起。🔴 真因已查明 = LLM 401 根本没调用成功，**LLM 不修好这项没法验证** |
| **OVERLAY-065** | 替换左侧麦克风图标为动态图标 | 优化 | 🟡 P2 | `src/main.rs` | coder-2 | ⬜ **未做**，排在 D2D P3 |
| **OVERLAY-069** | 松开热键 / 编辑态提交或回车后，窗口关闭要流畅丝滑，不要生硬突然关闭 | 优化 | 🟡 P2 | `src/main.rs` | coder-2 | ⬜ **未做**，排在 D2D P6 |
| **OVERLAY-063** | 笔记框内字体再大一号 | 优化 | 🟡 P2 | `src/main.rs` | coder-2 | ⬜ **未做**，排在 D2D P7 |
| **OVERLAY-066** | Overlay 窗口的高度再增加 3 个像素 | 优化 | 🟡 P2 | `src/main.rs` | coder-2 | ⬜ **未做**，排在 D2D P8 |

### 一句话总账（2026-09-03）

- **代码已落地（本地已提交，未出包未端测）：9 项** —— VERSION-059 / ASR-070 / ASR-074 / ASR-074-GUARD / OVERLAY-068 / OVERLAY-075 / HOTKEY-060 / ITN-071 / D2D-073-P0
- **动了但没做完：2 项** —— ASR-067（并入 ASR-074，等端测判定）、OVERLAY-064（只有根因报告，无代码）
- **完全没做：6 项** —— OVERLAY-061（假设证伪，重新定位中）/ OVERLAY-062 / 063 / 065 / 066 / 069（全排在 D2D 后续阶段）/ FMT-072（挂起，卡在 LLM 401）
- **Gavin 端测通过（🟢）：0 项** —— 因为一次包都还没出

**状态图例**：⬜ 待派发 / 🔵 已派发执行中 / 🟣 已交付待验收 / ✅ 已验收已提交 / 🟢 Gavin 端测通过 / ❌ 打回

### 🔴 ASR-067 症状澄清（Gavin 2026-08-30 补充）——**排查方向整体改向，原判作废**

**Gavin 补充原话**：

> 我在录音时，停顿了 1 秒，然后继续说话，但是**屏幕窗口中语音识别上屏显示就中断了**

**与最初描述的差别（决定性）**：

| 版本 | 描述 | 指向 |
| --- | --- | --- |
| 初报 | 「接着输入就无法继续，**麦克风不接受输入**」 | 音频采集 / ASR 链路 |
| 澄清后 | 「**屏幕窗口中语音识别上屏显示**就中断了」 | **overlay 显示路径** |

**这解释了 coder-1 静态排查为什么查不出问题**：它查的是音频链路
（`qwen_inference.rs`），结论「录音循环不退出、音频不停发、服务端断句不断连」
**全部正确且与新症状不矛盾** —— 音频确实没断，**断的是屏幕上的字**。

🔴 **原判「ASR-067 属 ASR 问题、归 coder-1、文件域 `qwen_inference.rs`」作废。**
新方向指向 `src/main.rs` 的流式上屏揭示逻辑，**文件域改归 coder-2**。

#### 主控的新假设（**未验证，需 debug.log 实证，不许当判据直接改**）

嫌疑落在 `OVERLAY-051-G-FIN` 的时间戳驱动回放
`reveal_chars_by_timeline`（`src/main.rs:3232-3258`）：

```
let origin_begin = words[0].begin_time;
for w in words {
    if w.begin_time - origin_begin <= elapsed_ms { revealed += ...; }
    else { broke_early = true; break; }   // 假设 words 按 begin_time 升序
}
```

两个关键假设写在代码里但**未被验证**：

1. **`words` 全局按 `begin_time` 升序**（`:3248` 注释自陈）。
   停顿 800ms 触发服务端断句 → `sentence_end=true` → 新句以新 id 开始。
   **若服务端的 `begin_time` 是按「句内相对」而非「整条音频流相对」计时**，
   新句的 `begin_time` 会重新从小值开始 → 合并后的 `confirmed_words + current_words`
   列表**不再全局升序** → 循环在句边界 `break`，后续新句的字**永远轮不到揭示**
   → 表现正是「屏幕显示中断，但音频还在流」。
2. **`origin_begin` 取 `words[0]`**（第一句第一个词）且 `tween_audio_origin`
   只在首次收到非空 words 时设定、此后不重置 —— 跨句是否仍成立未验证。

叠加 `displayed_chars` 的 `.max(displayed)` 单调约束（051-G 契约 4），
一旦算出的 `revealed` 低于已显示值，显示就**冻结不动**，与 Gavin 描述吻合。

#### 处置（Gavin 已拍板顺序）

> Gavin 原话：「等下一波开单，我需要等这次出包后端侧输出 debug log」

- 🔴 **本批不动 ASR-067 代码**。等 v0.9.0 出包 → Gavin 端测 → 取 `debug.log`
- **要看的探针**（ASR-058 已埋，本包内可用）：
  - `[ASR-WORDS]` 词时间轴 —— **直接验证假设 1**：把停顿前后两句的
    `text[begin-end]` 序列拉出来，看 `begin_time` 在句边界是否回落
  - `[ASR-SUMMARY]` 12 字段 —— 看 `first_text_ms` / `final_ms` / `words_total` / `chars_total`
- **出包因此进入 ASR-067 的关键路径**：不出包就拿不到日志，拿不到日志就定不了根因

## 🔴 ASR-074 · 在线 ASR 会话中途停止产出 + finalize 拖尾（**ASR-067 与 ASR-070 合并为同一根因**）

**建单人**：主控，2026-08-30，依据 tester-1 REPRO-073 实测 + 主控独立复核。

### 为什么合并

ASR-067（录音中途上屏显示中断）与 ASR-070（松键后尾部文字丢失）
三份独立证据指向**同一处机制**：

| # | 证据 | 来源 |
| --- | --- | --- |
| 1 | 末次长录音 `final_ms=55474`，词时间轴跨度远小于录音时长；多条 run 出现 `words_total=0 / chars_total=0` 而 `final_ms` 正常 → **录音在跑、服务端不产出** | 主控分析 `target/release/debug.log` 28 条 `[ASR-SUMMARY]` |
| 2 | 7 组尾部对照中 5 组丢失，**全部为服务端未返回**（非客户端丢弃） | tester-1 REPRO-070 |
| 3 | `OVERLAY-043: ignoring late StreamingText after stop` 单 run 6+ 次、**持续 4-5 秒** → 松手后服务端仍在缓慢回吐 | tester-1 REPRO-073 新疑点 3 |

**统一解释**：在线 ASR 会话运行中途停止产出识别结果，且 stop 后 finalize 拖尾 4-5 秒。
中途表现为「上屏显示中断」（=ASR-067），末尾表现为「尾部文字丢失」（=ASR-070）。

### 🔴 主控自陈：已提交的 ASR-070 修复**不解决本症状**

提交 `958cadb` 把 `final_text()` 从「只返回 confirmed」改为「confirmed + current」。

**那是一个真实的潜在缺陷，修法也正确，予以保留**
（若服务端确实返回了尾部但未标 `sentence_end`，旧实现会丢；新实现不会）。

**但它不是 Gavin 所报现象的原因。** 决定性反证（主控独立复核 `debug.log` 15:09:33）：

| 环节 | 内容 |
| --- | --- |
| 口播 | 今天天气很好我们下午**三点在公司门口集合** |
| `Transcribed:`（服务端返回） | 今天天气很好，我们下午。 |
| `Injecting text:`（实际注入） | 今天天气很好，我们下午。**与上一行逐字相同** |

注入层零丢失，尾部字**从未从服务端到达**。

🔴 **主控 2026-08-30 曾向 Gavin 报「尾部丢字已修复」，该结论下得过早，此处更正。**
教训：**修了一个能解释症状的缺陷 ≠ 修了造成症状的缺陷。**
定案必须有「修复前后同场景实测对照」，静态因果链再顺也只是假设。
与本批 `[STATIC-PROOF-MISSED-CALLGATE-001]`、ITN-071-B 的「防御性修复」属同类。

### 排查方向（下一棒，**现行 exe 即可查，不必等出包**）

ASR-058 埋点已在 `target/release/feiyin-ime.exe`（08-18 23:48 构建）内。

1. **会话为何中途停止产出**：`qwen_inference.rs` 主循环在长录音下是否仍持续收发；
   服务端是否发过 `task-failed` / 错误帧而被静默吞掉；
   是否存在未处理的 WebSocket 关闭/超时
2. **finalize 为何拖尾 4-5 秒**：stop 后到 `task-finished` 之间发生了什么
3. **`vad_hit_ms` 异常**（🟡 tester-1 已自行降级为待验证假设，不作判据）：
   28 条 `[ASR-SUMMARY]` 中，早期 2 条（04:19/04:21）为 637/644 正常值，
   tester-1 的 26 条（15:xx）**全为 -1**。可能是环境差异（TTS 放音链路）而非缺陷，
   但值得顺带确认 VAD 命中回执路径

### 🟡 数据可信度限制（tester-1 已在 result.md 附录 A 如实声明）

tester-1 用 TTS 放音经扬声器再由麦克风拾音，信号质量偏低：
**26 次 run 中 14 次 `words_total=0 / chars_total=0`（完全无识别）**。
故其统计口径（如「067 四轮三中」）可能掺入环境因素。
**不受影响的是**：061/068 的几何数据（不依赖识别质量）、
以及 070 的 15:09 那组干净对照（服务端返回与注入逐字比对）。

### 关联的其他 REPRO-073 结论

- **OVERLAY-061 未复现**：1044 帧零命中。tester-1 如实报告未复现并建议
  **多屏 / 不同 DPI 环境复测**，主控采纳。已向 Gavin 确认其显示器配置。
- **OVERLAY-068 覆盖面存疑**：实测交替闪烁主体为「长文本 vs Recording 240 宽」拉锯，
  而 coder-2 的 068-B 修的是「流式 → Processing 200」路径。已要求 coder-2
  核对覆盖面并给行号证据（不返工整体方案）。

---

#### 🔬 ASR-067 实测日志分析（主控 2026-08-30 23:0x，数据源 `target/release/debug.log`）

**数据源合法性**：`debug.log` mtime 2026-08-30 22:24，size 159,779 —— 今晚 tester-1
跑 REPRO-073 时由 `target/release/feiyin-ime.exe`（08-18 23:48 构建，**晚于 ASR-058
埋点提交 `710cec9` 23:27**，故探针在包内）产出。含 `[ASR-WORDS]` 48 行 / `[ASR-SUMMARY]` 4 行。

##### 结论一：🔴 主控原假设**被证伪**，不要再查这个方向

原假设：停顿触发服务端断句 → 新句 `begin_time` 按句内相对计时回落 →
合并词表不再全局升序 → `reveal_chars_by_timeline`（`src/main.rs:3244-3251`）
在句边界 `break` → 后续字永不揭示。

**实测：48 条 `[ASR-WORDS]` 逐条检查，`begin_time` 序列全部全局升序，无一例外。**
服务端时间戳是**整条音频流相对**，不是句内相对。
→ `:3248` 那句「words 按 begin_time 升序排列」的注释**是成立的**，
→ 「乱序导致提前 break」这条路**排除**。

##### 结论二：🟡 发现更强的新线索 —— **词时间轴只覆盖录音的一半**

以最后一次录音（`[ASR-SUMMARY]` 第 4 条）为例：

| 指标 | 值 |
| --- | --- |
| `final_ms`（整条录音时长） | **47,569 ms** |
| 词时间轴跨度（首词 begin → 末词 end） | **24,470 ms** |
| 覆盖率 | **51.4%** |
| `words_total` / `chars_total` | 87 词 / 122 字 |
| 词间静默 ≥700ms 的间隙 | **0 个** |

**即：录音进行到约 24.5 秒之后，服务端不再产出任何词时间轴，而录音又持续了 23 秒。**

词间无一处 ≥700ms 静默，说明**已产出的那 24.5 秒内说话是连续的**；
之后的 23 秒要么没说话，要么**说了但没有任何识别结果回来**。

**后者若成立，就是 ASR-067 的直接机制** —— 与 Gavin 描述
「录音时停顿 1 秒，然后继续说话，但屏幕上语音识别上屏显示就中断了」完全吻合：
音频还在发（coder-1 静态排查已证实客户端不停发，该结论仍然有效且不矛盾），
**但服务端停止回结果**，屏幕自然就不再更新。

##### 🔴 尚不能定案的原因（诚实说明，不要拿本节当判据改代码）

本次日志是 **tester-1 用 TTS 放音装置跑的**，**不是 Gavin 的受控复现**。
无法确定后 23 秒里是否真的有人在说话。
若那 23 秒本来就是静音，则「覆盖率 51.4%」是正常现象，不构成缺陷。

##### 下一步（需 Gavin 一次受控复现即可定案）

Gavin 用 `-debug` 跑一次**明确的**：说话 → 停顿约 1.5 秒 → **继续说明显能听清的一段话** → 松手。
然后看该次录音的：

| 检查项 | 判据 |
| --- | --- |
| `[ASR-WORDS]` 末词 `end_time` | 是否停在停顿处附近（≈ 停顿前的音频位置） |
| `[ASR-SUMMARY]` 的 `final_ms` | 是否远大于上面的末词 end_time |
| 两者差值 | ≈ 停顿之后那段话的时长 → **坐实服务端停止回结果** |

**若坐实**：根因在 ASR 会话侧（服务端断句后未恢复产出 / 客户端未重建 task），
文件域回到 `qwen_inference.rs`，归 coder-1；
**若证伪**（末词 end 接近 final_ms）：根因才在 overlay 揭示侧，归 coder-2。

**这个判据一次复现就能分流，不必两边同时开工。**

### 🔴 文件级冲突评估（派发前必读，主控已做）

**`src/main.rs` 被 8 项任务同时命中**（061/062/063/064/065/066/068/069）——
这是本批最大的冲突源。**必须整包给同一个 Worker（coder-2），禁止拆给两人并行。**

**唯一的跨 Worker 风险点是 ASR-070**：「尾部文字丢失」的修复点可能落在
`src/main.rs` 的 051-G 时间戳回放 / 松键 flush 路径，与 coder-2 的 overlay 批**同文件**。
处理办法：**先只让 coder-1 做只读根因取证**，确认修复点究竟在
`qwen_inference.rs`（服务端 flush 时序）还是 `src/main.rs`（reveal 揭示进度）：
- 落在 `qwen_inference.rs` → coder-1 独立做，与 coder-2 零冲突
- 落在 `src/main.rs` → 并入 coder-2 的批次串行做，**不允许两人同时开 main.rs**

`HOTKEY-060`（ui/）、`ITN-071`（itn.rs）、`FMT-072`（llm/mod.rs）三项文件域互不重叠，
且与 `src/main.rs` 零重叠，可由 coder-1 串行承接。

### 主控初判（供 Worker 取证时参考，**不是结论，不许当判据直接改**）

| ID | 主控假设 | 依据 |
| --- | --- | --- |
| ASR-067 | 疑似 `asr_online_max_sentence_silence`（隐藏字段，默认 800ms）判定句子结束后**会话未重建**，导致后续音频无人接收 | ASR-056 引入该字段；Gavin 描述的「停顿约 1 秒」与 800ms 阈值高度吻合 |
| ASR-062 视觉粗糙 | 疑似 **GDI 对图形不做抗锯齿**，且 `WS_EX_LAYERED` 分层窗口下 ClearType 次像素渲染被禁用 → 文字发虚、圆角/按钮边沿锯齿。054-D 的 `WM_SETFONT`（`src/main.rs:681-690`）确已生效，所以**根因不在字体句柄，而在渲染管线** | 需 coder-2 验证；若成立则属架构级问题，要立 DEC |
| OVERLAY-064 | 054-C 把边框色从 `0x060607`（近黑）改成 `0x3A3A3C`（中灰）后 Gavin 反而说「消失」——需查是否被 `SetWindowRgn` 圆角区域裁掉，或被 `SetLayeredWindowAttributes` 色键吃掉 | `OVERLAY_BORDER_GRAY` 定义在 `src/main.rs:997`，13 处用点已列 |
| OVERLAY-061 | 054-B-FIX 的 `pos: Option` 改造代码确在（`:1163-1172` / `:1651` 两处 `unwrap_or_else`），但仍闪 → 疑似**窗口创建时**就落在 (0,0)，第一次 `SetWindowPos` 之前已被绘制 | 需查 overlay HWND 的 `CreateWindowExW` 初始坐标 |

### 排期建议（主控提，等 Gavin 拍板）

1. **VERSION-059**（已派发，coder-1，约 10 分钟）
2. **ASR-067 + ASR-070 取证**（coder-1，只读，不改代码）—— 两条 P0，且决定 ASR-070 归谁做
3. **OVERLAY 批**（coder-2 独占 `src/main.rs`）：064 → 061 → 068 → 062 → 063 → 066 → 069 → 065
4. **HOTKEY-060 / ITN-071 / FMT-072**（coder-1，与 coder-2 并行，文件域零重叠）
5. 阶段三 TEST-SYNC → 阶段四 全量回归 → 阶段五 出包 v0.9.0

---

## 🔄 2026-08-18 晚 · 会话重启后现场核对（主控，控制台崩溃后重建上下文）

| 项 | 实况（以 `git status` / `git log` 为准，非文档记忆） |
| --- | --- |
| HEAD | `7b8423c`，本地 **ahead 7**（未 push，Gavin 未授权） |
| `src/` 工作区 | **clean** —— 昨日 handoffs 里「051-G WIP +270 未提交」**已过期**，该批已随 `663f1d5` 提交 |
| 未提交残留 | 仅 `tests/utils/state_detector.py` (+70) 与未跟踪 `tests/test_cases/test_overlay_position.py` —— tester-1 域的 E2E 位置断言，**待确认归属后一并提交** |
| 三 Worker | coder-1 / coder-2 / tester-1 **全部 ACK 就绪**（框架重启已清空 inbox/outbox，旧任务书被抹，[REPLACE-WORKER-TASKFILE-WIPED-001] 再现） |
| 流水线阶段 | 051-G-FIN + 054-B-FIX 的**阶段三已完成并提交** `ce690cf`；**阶段四（TEST-EXEC）尚未跑** |

### 本轮派发

| 时间 | 动作 |
| --- | --- |
| 08-18 晚 | **OVERLAY-054-C/D/E → coder-2**（任务书 9.2KB 重写，原 6.0KB 版被框架重启抹掉）。独占 `src/main.rs` |

### 排期决策：054-C/D/E 先做，阶段四合并一次跑

Gavin 已明令「三个问题一起做了再出包，避免无效重复出包」。
故**不为 051-G/054-B 单独跑一轮阶段四**，等 054-C/D/E 完成后
走一次 TEST-SYNC-054CDE（阶段三）+ 一次全量回归（阶段四），
最后 **BUILD-021 单包**带上：054-A 焦点恢复 / ASR-055 测试按钮 / WORDBOOK-053-C/D /
054-B-FIX 窗口位置 / 051-G-FIN 上屏节奏 / E2E-HARNESS-050 门禁 / 054-C/D/E 视觉三项。

### 🔴 出包门禁（主控自加，持续有效）

出包前 **E2E 门禁必须真跑通过**，不以「读代码觉得没问题」代替运行验证。

---

## 🛑 2026-08-17 当前状态 —— 下一步等 Gavin 端测 BUILD-018

> 三个 Worker 全部空闲待命。**下一个动作是等 Gavin 端测结果，不是派新任务。**

| 项 | 状态 |
| --- | --- |
| HEAD | `664b7bd`，工作区 **clean**，零悬空改动 |
| 本批 OVERLAY-043 四提交 | OVERLAY-043 `a588509` / 043-B `5940e73` / TEST-SYNC-043 `497131f` / TEST-EXEC-043 `b499cc3` **全部已验收提交** |
| 阶段四全量回归 | ✅ **1040/0/11**（上批 1030 +10 精确命中零残差）+ Vitest 54 + src-tauri 55；pytest 按书 SKIP（`Publish/` 当时为 BUILD-017 旧包） |
| 双消融实测 | ✅ A（`delta.abs()`→`delta`）变红 **4** 条（推演预期 3，主控裁决实测为准，见 `[ABLATION-MODEL-TOO-LIGHT-001]`）；B（门闩→false）变红 1 条与预期吻合；两次均已还原并 `git diff -w` 自证为空 |
| 阶段五出包 | ✅ BUILD-018 `664b7bd` —— 产物 08-17 13:57-14:00 在 `Publish/`，七项核验全 PASS（三 exe 12,115,456 / 10,026,496 / 24,859,648 B） |
| 未 push | 本地 ahead **6**，**Gavin 未授权 push，不得自动执行** |

### Gavin 端测四项重点（BUILD-018）

1. **流畅度目视** —— 文字进编辑框是否丝滑（`bErase=false` + `needs_repaint` 脏标记 + 25% lerp 插值）
2. **本地模型录一次** —— 确认波形动画未被本批改坏（静态论证四条已过，需实测兜底）
3. **波形隐藏 + 单按钮** —— `RecordingWithText` 态不画波形；右侧只有一个按钮（录音=停止方块 / 编辑=橙色 ⏎ 提交）
4. **松开热键立即切「处理优化中」** —— 晚到的 `StreamingText` 不应再把状态顶回录音态

### 端测通过后的下一棒（按序）

1. **TEST-045-REFACTOR** —— 抽 `decide_pipeline_entry` 纯函数补调用侧护栏缺口（见下方专节；🔴 红线：**不得合并** `:4334` 第二层 `text.trim().is_empty()` 分支）
2. **ASR-PERF-040-B/C** —— 连接健壮性与性能（040-A 埋点已落地，B/C 的唯一数据源是端测 `debug.log`，见下方门禁专节）

### 端测若失败，`debug.log` 优先看这三条

| 目标 | 探针 | 期望 |
| --- | --- | --- |
| ASR-042 采样率修复 | 搜 `Streaming resampler active` | **有**这行 = 重采样器接上了 |
| ASR-045 结果上屏 | 搜 `No audio samples recorded` | 流式录音后**不应**再出现 |
| OVERLAY-043 门闩 | 松开热键后的状态流转 | 不应再出现 `RecordingWithText` 把 `FallingToProcessing` 顶回去 |

### 🔴 两个不许动的东西（Gavin 明确指示，持续有效）

- `target/release/feiyin-ime-nor.exe`（08-09，11.9MB）—— Gavin 的**备份 exe，不得删除**。（其**进程**可在出包 Step1 清杀，名单已于 08-16 补入 `build-test-guide.md`）
- **Gavin 自启的端测进程** —— 杀之前必须先报主控、由 Gavin 授权（PID 4696 / PID 21948 两次先例）

---

## 🔴🔴 P0 最高优先 · OVERLAY-046 录音 overlay 从未被定位/定尺寸（2026-08-17 Gavin 端测 BUILD-018）

> **Gavin 原话**：「热键完全被改坏，无法正常使用输入法……按下设置好的热键，录音窗口不出现，完全无法录音」
>
> **性质：OVERLAY-043 引入的真回归，输入法 100% 不可用。已派发 coder-2。**

| 项 | 内容 |
| --- | --- |
| 编号 | **OVERLAY-046** |
| 文件域 | `src/main.rs`（仅此一个，coder-1/tester-1 本轮无任务，零冲突） |
| 负责人 | coder-2 |
| 状态 | ✅ **已交付，等主控验收** |

### 修复摘要

- **根因**：OVERLAY-043 重构中 Show 分支丢失无条件 `SetWindowPos` / `InvalidateRect`；非流式态恒等赋值 `current_size == target_size` 使插值门控 `size_interpolation_done` 永不置位，`:1221` 的 `SetWindowPos` 永不执行 → 窗口不定位/不定尺寸。
- **修法**：在 `src/main.rs:997` Show 分支 `ShowWindow` 之前恢复无条件 `SetWindowPos(request.pos, computed_size, SWP_NOACTIVATE | SWP_NOZORDER)`；`ShowWindow` 之后补 `InvalidateRect(hwnd, None, false)`（`bErase=false` 红线保持）。
- **覆盖变体**：`Recording` / `FallingToProcessing` / `Processing` / `Error` / `FocusLost` 全部非流式态；流式态 `RecordingWithText` / `StreamingEditing` 不受负影响，插值动画继续。
- **验证**：`cargo fmt --all -- --check` clean / `cargo check --all-targets` 0 error / `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / `git diff -w -- src/main.rs` 仅 `:997-1005` 新增 14 行。
- **阶段三**：不跑 `cargo test` / `cargo build`，测试由 tester-1 负责。

### 根因（主控 git 对照取证，四步）

| # | 证据 |
| --- | --- |
| 1 | `git show 56bfa37:src/main.rs`（043 之前）Show 分支含三件事：**无条件 `SetWindowPos`** + `ShowWindow(SW_SHOWNA)` + `InvalidateRect(hwnd, None, true)` |
| 2 | 现 HEAD `664b7bd` 的 Show 分支（`:997-1005`）只剩 `SetLayeredWindowAttributes` + `ShowWindow` + 条件 `SetTimer` —— **`SetWindowPos` 与 `InvalidateRect` 双双消失** |
| 3 | 全文件唯一定位 overlay 的 `SetWindowPos` 在 `:1221`，被 `size_interpolation_done` 门控；该标志仅在 `:1198` `current_size != target_size` 成立时于 `:1212` 置位 |
| 4 | 但 Show 分支 `:991-993` 对**非流式态**执行 `target_size = current_size = computed_size` → 两者恒等 → `:1198` 恒 false → **`SetWindowPos` 永不执行** |

`:476` `remove_noactivate` / `:494` `restore_noactivate` 的两处 SetWindowPos 都带
`SWP_NOMOVE | SWP_NOSIZE`，不构成兜底。

### 🔴 影响范围比 Gavin 报的更广

`Recording` / `FallingToProcessing` / `Processing` / `Error` **全是非流式态**，全走 `:991-993`
→ **在线模型与本地模型的录音窗口一律受影响**。

🔴 **主控自陈作废一条结论**：本文件下方「本地模型回归核查四条全过 → OVERLAY-043 对本地模型
录音窗口零影响」**是错的**。那四条只机器比对了 GDI 绘制原语序列，**没有查 `SetWindowPos`
的调用门控**，因此漏掉了本缺陷。教训已记 `troubleshooting.md`。

### 红线遵守

① `InvalidateRect` 的 `bErase` 保持 `false` ✅  ② 未动 `:1198` 插值分支与 `interpolate_step` ✅  ③ 未改 `STREAMING_STOPPED` 门闩语义 ✅  ④ 版本号 0.8.0 未动 ✅

---

## ℹ️ 非代码问题（已核实，无需开单）

| 现象 | 结论 |
| --- | --- |
| `LLM optimization error: HTTP 402 Payment Required`（api.deepseek.com） | **DeepSeek 账号余额/账单问题**，非代码 bug。后果：LLM 优化失败 → 走 raw text fallback。需 Gavin 侧充值 |
| `转录失败：task-finished 但无识别结果` ×20 / ASR 结果 254 条中 246 条 `display=''` | **端测手法所致，非 bug**。Gavin 在反复短按热键测「窗口出不出来」，多数按压 <1.5s 且未说话 → VAD 正确判定无语音（`Audio channel closed during VAD gate` / `2s deadline without detection`）→ 服务端正确返回空。同一会话中真说话的两次（06:16:35 / 06:16:36）识别出「你好。」，06:17:26 识别出「Hello.」→ **ASR 链路本身是通的** |

## 🔴 P0 进行中 · v0.8.0 端测两大问题（Gavin 2026-08-16 提交）

> 出包后 Gavin 端测发现两个问题，主控已独立取证定位根因。**执行顺序已与 Gavin 商定：先修识别，再修窗口。**
> 理由：识别输出乱码时，无法目视判断窗口显示是否正确。

| 编号 | 内容 | 文件域 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| **ASR-042** | 在线流式 ASR 采样率修复（48000→16000 带状态流式重采样器） | `src/audio/mod.rs` | coder-1 | ✅ **已验收已提交 `34906a1`**（StreamingResampler + record_streaming 接线 + 6 测试 + 改坏会红自证） |
| TEST-SYNC-042 | 阶段三测试同步（补 4 条流式重采样缺口用例） | `src/audio/mod.rs` `mod tests` | tester-1 | ✅ **已提交 `427cd50`** |
| **ASR-045** | 流式识别结果被管线丢弃（文字从未上屏，P0） | `src/main.rs` | coder-1 | ✅ **已验收已提交 `54cde62`**（`should_cancel_on_empty` 纯函数 + 判空臂改调 + 3 条护栏测试） |
| TEST-SYNC-045 | 阶段三测试同步（ASR-045 缺口 3 条，`src/main.rs` +46 仅测试模块） | `src/main.rs` `mod streaming_empty_samples_tests` | tester-1 | ✅ **已验收**（主控逐行 Read diff + 独立复算 `cargo fmt --check` clean / `cargo check --all-targets` 0 error）。真值表第 4 格（如实标弱护栏）+ 空串/纯空白两层分层契约 ×2（核心）+ 调用侧不可测诚实判定 |
| TEST-EXEC-042/045 | 阶段四全量回归 + 消融 A/B 自证 | — | tester-1 | ✅ **已验收已提交 `2a173f0`**。1030/0/11（941→957 恰 +16，零残差）+ Vitest 54 + src-tauri 55；pytest 按书 SKIP（Publish/ 为 BUILD-016 旧包）；③ 类真回归 = 0 |
| BUILD-017 | 阶段五出包（ASR-042/045 进 exe） | — | tester-1 | ✅ **已验收**（主控独立复算全部七项）。产物 08-16 23:25，`feiyin-ime.exe` 12,112,384B（+5,632）/ ui 10,026,496B 不变 / crash 24,859,648B 不变；两副本 sha256 全等；toml 三副本全等；ProductVersion 0.8.0.0/0.8.0/0.8.0.0。⏭ **待 Gavin 端测** |
| **OVERLAY-043** | 录音窗口**五项**显示错乱 + 流畅度 | `src/main.rs` | coder-2 | ✅ **已验收已提交 `a588509`**（打回一轮，见下方打回记录）。⏭ 流畅度**待 Gavin 端测目视拍板** |
| **OVERLAY-043-B** | 抽 `interpolate_step` / `should_ignore_streaming_text` 补护栏 | `src/main.rs` | coder-2 | ✅ **已验收已提交 `5940e73`**（纯重构零行为变更，无 `#[test]`） |
| **TEST-SYNC-043** | 阶段三测试同步（两纯函数护栏 + 不可测项如实说明） | `src/main.rs` `mod tests` | tester-1 | ✅ **2026-08-17 已完成待验收**（`mod overlay_043_interpolate_tests` +138 行仅测试区：10 条用例——五契约 ±50000 穷举 + 缺陷 A 数值回归护栏 + 双向收敛预算 + 门闩四格真值表；cargo fmt clean / cargo check --all-targets 0 error；Python 复算契约成立、消融推演护栏有效。消融实测顺延阶段四 TEST-EXEC-043） |

### 🔴 OVERLAY-043 验收打回记录（主控独立复算查出，非采信报告）

| 缺陷 | 位置 | 问题 | 数值证据 |
| --- | --- | --- | --- |
| **A** | `main.rs:1195` 插值步长 | `(dx as f32 * 0.25).max(1.0)` 在 `dx` 为负时**负数被 `max(1.0)` 吃掉比例**，恒为 `-1px` | 800px→240px 需 **559 帧 ≈ 8.9 秒**爬行；修正后放大/缩小对称均 **22 帧 ≈ 0.35 秒** |
| **B** | `main.rs:1114` 脏标记 | 纯 `Recording` 态被纳入脏标记，但**全文件无一处在音频电平变化时置位** → 波形动画冻结 | 违反红线「`Recording` 观感零变化」；对照 `:1159` `FallingToProcessing` 有 `!all_settled` 兜底，`Recording` 分支缺同类兜底 |

**教训**：缺陷 A 只能靠手工数值复算发现 —— 算式内联在消息循环内，测试够不着。
故**当场派 OVERLAY-043-B 抽纯函数补护栏**，不排期延后
（与 `TEST-045-REFACTOR` 同类，但那是没塌过的预防，本处是刚塌过一次）。

### ✅ 本地模型回归核查（Gavin 2026-08-17 提出，主控独立取证，四条全过）

> **Gavin 原话**：「本地模型的录音还是会使用原先的录音窗口，所以这次新集成 asr 实时上屏
> 不能影响本地模型录音的 overlay 窗口效果」

| # | 核查项 | 结论与证据 |
| --- | --- | --- |
| 1 | 绘制是否等价 | ✅ **机器比对**：拆分前单体与拆分后 `chrome + indicator_and_waveform + stop_button` 串接，**18 个 GDI 原语序列完全一致**（`diff` 为空） |
| 2 | 是否被尺寸插值波及 | ✅ 不会。`is_streaming_text` 仅 `matches!(RecordingWithText)`；非流式态 `:991-994` 令 `target_size == current_size` → `:1191` 插值条件不成立 |
| 3 | 波形是否会冻 | ✅ 不会。返工后 `Recording` 独立为**每帧重绘**分支，脏标记只用于 `RecordingWithText`/`StreamingEditing` |
| 4 | 门闩是否误伤 | ✅ 不会。`StreamingText` 仅在 `is_streaming_asr`（`AsrModel::QwenAudioOnline`）下产生（`:3512-3543`）；本地模型走 `record()` + `run_pipeline_core` 老路**永不进入该分支**，注释 `:3509` 亦自陈「零行为变更」 |

**结论**：本地模型 overlay 只经历 `Recording → FallingToProcessing → Processing`，
全是本批未改语义的状态。**OVERLAY-043 对本地模型录音窗口零影响。**
🔴 **但这是静态代码论证，Gavin 端测时请顺手用本地模型录一次确认。**

### 🟡 待排期 · TEST-045-REFACTOR 抽 `decide_pipeline_entry` 纯函数（tester-1 建议，主控采纳但延后）

**问题**：ASR-045 全批 6 条测试（coder-1 3 + tester-1 3）**全是纯函数级，保护不了调用侧**。
若有人把 `main.rs:4318` 的 guard 改回 `Ok(s) if s.is_empty()`，P0（流式文字从未上屏）立刻复发，
**6 条测试依然全绿**。这是本批已知且已如实记录的护栏缺口，不是遗漏。

**建议方案**（tester-1 提出，主控核后认可）：把 `run_pipeline_core` 入口三臂判据抽成
`decide_pipeline_entry(&samples_result, &initial_text) -> EntryDecision`
（`CancelNoInput` / `Err` 透传 / `Proceed`），让「空+文本 / 空+无文本 / 非空+无文本 / Err」四种入口组合可测。

**主控裁决：采纳，但排到 OVERLAY-043 之后。** 理由：① 这是可测性改进不是 bug，不该插队 P0；
② OVERLAY-043 马上要动 `main.rs`，两个改动叠一起会让回归归因变难。
🔴 **实施时的红线**：不得合并 `:4334` 第二层的 `text.trim().is_empty()` 分支 ——
分层是刻意设计，合并即复现「空文本静默消失」的 P0 类回归。

### ASR-045 根因（一句话）

调用侧 `run_pipeline_core(Ok(Vec::new()), …, Some(streaming_text))`（`main.rs:3409`）传**空 samples +
流式文本**；接收侧判空臂 `Ok(s) if s.is_empty()`（原 `:4307`）**无条件取消**，`initial_text` 从未被读取
→ 流式文本在函数入口即被丢弃，ITN→LLM→注入后半段从未执行（日志 `:193` 已产出 5 confirmed sentences，
`:194` 即被 WARN 吞掉）。

**取证与修法** → outbox/coder-1/result.md（含三问全链 27 步、Q1 samples 消费点排查、Q2 cancel_signal
时序、护栏测试）。

### ASR-042 根因（一句话）

麦克风 48000Hz 采集，发给服务端的参数写死 `sample_rate: 16000`，中间零重采样 →
服务端按慢 3 倍解码 → 识别输出完全不相干的句子（「你好，花呗还不上」等客服话术）。

**成因**：旧 `record()` 在录音结束那一刻一次性重采样（`audio/mod.rs:757`）；改成边录边发后
「录音结束」这个时刻不存在了，该步骤静默失效。**不是写错，是改架构漏接线。**

**完整取证与修法** → `collab/troubleshooting.md` `[ASR-SAMPLERATE-STREAM-001]`
（含 8 次录音「6 有重采样全对 / 2 无重采样全错」对照表与日志自证行）。

**关键约束**：不能逐块直接调 `resample_anti_alias`（整段 windowed-sinc，TAPS=32），
逐块调用每 10ms 截断一次卷积核，会让 FIRSTCHAR-FIX-005 修过的送气清声母失真复发。
必须做带状态流式重采样器 + 全局输出计数。护栏：批处理 vs 流式逐点差 ≤ 1e-5。

**次生现象**：VAD 2s 漏检（`debug.log:36`）同根因，预期自动恢复，**须实测确认，禁改 `vad.rs`**。

### OVERLAY-043 六项（主控已逐条核对代码，非采信 coder-2 汇报）

> 🔴 **2026-08-17 Gavin 端测截图复核，本节已从四项扩为六项。**
>
> **首先澄清一个误解**：Gavin 反馈「上次发现的问题根本没改过来」——
> **本任务自始至终从未派发过**（本节状态一直是「🔜 等 BUILD-017 + Gavin 目视确认识别正常后再派发」），
> 所以不是「改了没生效」，是**还没动过一行代码**。责任在主控排期，不在 coder-2。
>
> **端测截图的意外收获（重要正面结论）**：截图中 overlay 显示「今天的天气如何 可以说」——
> **语义完全正常**，不再是修复前「你好，花呗还不上」那类 48kHz 按 16kHz 解出来的客服话术。
> **ASR-042 采样率修复已由端测实证生效**，且流式文字确实上了 overlay。

| # | 现象 | 核对结论（主控 Read 代码取证） |
| --- | --- | --- |
| 1 | 波形动效未隐藏 | 属实。`main.rs:1786` 首行 `draw_recording_overlay` 整个重画，注释自陈 reuse「background/border/**waveform**/stop button」，波形被原样带进 |
| 2 | 文字非白色 | ⚠️ **本次端测截图看文字是白的**，Gavin 未再提。降级为待观察，不列入本次修复范围 |
| 3 | 右侧两个按钮（提交+停止） | 属实。`:1786` 返回 stop 按钮，`:1834-1871` 又叠画一个橙色圆角 ⏎ submit 按钮 → **两个并存**。Gavin 明确要求：**提交按钮应复用停止按钮的位置**，而非新增 |
| 4 | 窗口随文字变宽时卡顿、突闪 | 属实且**比原描述严重**：`:908-925` 判 `should_resize`，`:940-949` 为 false 时走 else → **回退到 `request.size`（默认 240px）而非保持上次宽度** → `:956` SetWindowPos 无条件执行 → 窗口在算出宽度与 240px 之间**来回跳** = 突闪 |
| 5 | 🆕 文字进编辑框卡顿，不丝滑 | **主控定位**：`:1078` `InvalidateRect(hwnd, None, **true**)` —— 第三参 `bErase=TRUE` **每帧擦背景再重画**，是 GDI 闪烁/卡顿的经典根因。三个态（Recording/RecordingWithText/StreamingEditing）全走这一行 |
| 6 | 🆕 录音结束未进编辑态时，应立即切「处理优化中」窗口 | **主控定位到竞态**：`:2737-2752` 松开热键的 `HotkeyEvent::Stop` **确实**发了 `FallingToProcessing`，但 `:2788-2796` 晚到的 `PipelineEvent::StreamingText` 又推 `RecordingWithText` **把它覆盖回录音态**。即状态切了、又被流式尾包顶回去 |

🔴 **主控自陈**：第 4 条的 100ms 节流是**上一轮主控自己要求 coder-2 加的**（防每帧改宽度抖动），
现成为文字截断的直接原因。**不能简单删除**（删了会抖，属「修一个带出一个」）。
修法：改为**延迟合并** —— 100ms 内多次变化攒起来，到点按最新文字算一次宽度。

### ~~待查~~ ✅ 已定性并修复（2026-08-16）

~~`debug.log:194`/`:429` 每次在线流式录完都报 `WARN No audio samples recorded`。~~

**结论：不是日志噪音，就是 ASR-045 本身。** 该 WARN 正是判空臂 `Ok(s) if s.is_empty()`
无条件取消时打的那一行 —— 流式文本在此被丢弃，整条 ITN→LLM→注入后半段从未执行。
ASR-045（`54cde62`）修复后该 WARN 在流式路径不再触发。**本条核销，无残留副作用待查。**

---


## 🔴 P0 门禁 · ASR-PERF-040 新 ASR 连接性能与可靠性（2026-08-15 Gavin 指令）

> **Gavin 原话**：「这次继承新 asr 模型一定要汲取之前的教训，要优化好连接池、
> 要优化好连接的速度和性能。」
>
> **定位：这是 ASR-038 全批的验收门禁，不是可选优化项。** 038-B/C 完成后若本节未落实，不予出包。

### 🔴 第一件事：028 的教训不能照搬，因为 ASR 根本没有连接池

| 项 | LLM（028 现场） | ASR（本批） |
| --- | --- | --- |
| 传输 | `reqwest` HTTP，**有连接池** | `tungstenite` WebSocket，**无池，每次全新建连** |
| 028 根因 | 池中空闲连接被服务端 keep-alive(~60s) 杀掉，取出即 0ms 失败 | **不适用** —— 没有池就没有僵尸连接 |
| 修复 | `POOL_IDLE_TIMEOUT=30s` + `is_request()` 重试 + `fmt_error_chain` | **照搬无效** |

**🔴 但真正的陷阱在这里**：为了提速而引入「**预热 / 复用长连接**」，
会**精确复现 028 的 bug 类** —— 预热好的 WS 空闲期间被服务端 idle timeout 静默杀掉，
下次录音取用即失败，且表现为「0ms 失败」，与 028 一模一样。

**因此复用设计必须自带三件套，缺一不可**：
① 本端 idle 上限**必须短于**服务端 idle timeout（服务端值未知 → 须实测，不可假设）；
② 取用前**健康检查**（WS Ping/Pong 或轻量探针）；
③ 检查失败**立即回退新建连**，绝不把失败抛给用户。

> **这正是 028 的真正教训**：不是「把超时调小」，而是
> **「任何跨请求复用的连接，都必须假设它在空闲期间已经死了」。**

### 取证：当前连接路径的实际开销（`qwen_inference.rs:445-465`）

每次录音**从零走完整条链**，零预热、零复用、零重试：

```
DNS 解析 (to_socket_addrs，阻塞系统调用，无缓存)
  → TCP connect (CONNECT_TIMEOUT = 5s)
  → TLS 握手 (client_tls_with_config)
  → WS 升级 (HTTP 101)
  → 才开始发第一个音频字节
```

`grep -i "prewarm|reuse|retry|keepalive|pool" src/transcription/qwen_inference.rs` → **零命中**。

### 已定位的四个问题

| # | 问题 | 位置 | 性质 |
| --- | --- | --- | --- |
| **40-1** | **`socket_addrs.first()` 只试第一个地址** —— DNS 返回 IPv6 在前但本机无 IPv6 连通性时，**直接失败**，不会回退第二个地址 | `qwen_inference.rs:461` | 🔴 **健壮性缺陷**。标准做法是遍历所有 addr（`TcpStream::connect` 本身就是这么做的） |
| **40-2** | **零重试** —— 建连瞬时抖动直接变「转录失败」 | 同上 | 🔴 与 028 修复精神相悖（028 明确补了重试判据） |
| **40-3** | **零 `[Latency]` 埋点** —— DNS/TCP/TLS/WS 各段耗时**完全不可观测** | `qwen_inference.rs` 全文 | 🔴 **Gavin 要求「优化速度」，但现在连测都测不了**。全库已有 12 处 `[Latency]` 约定可循 |
| **40-4** | `CONNECT_TIMEOUT=5s` 被同时用作 **connect / read / write** 三处超时 | `:32`、`:456-459` | 🟡 语义混用。建连 5s 合理，但流式收包期间的 read timeout 应独立定义 |

### 🔴 真流式在这里是**性能杠杆**，不只是交互需求

| 方案 | 建连成本落在哪 | 用户感知 |
| --- | --- | --- |
| **真流式**（边录边发） | 建连与**说话并行** → DNS+TCP+TLS 被说话时间吸收 | **零感知** |
| **伪流式**（录完再发） | 建连在**录音结束之后** → 全部落在关键路径 | **干等一个完整 RTT 链** |

**这条独立于 037 交互论证，直接服务于 Gavin 的性能指令。**
伪流式方案下，「优化连接速度」这个目标先天就少了最大的一块杠杆。

### 建议实施顺序（待派发）

| 编号 | 内容 | 前置 |
| --- | --- | --- |
| 040-A | **先补 `[Latency]` 埋点**（DNS/TCP/TLS/WS 升级/首帧/首结果分段计时） | 无 —— **必须最先做，否则后续优化全是盲调** |
| 040-B | 修 40-1 地址遍历 + 40-2 建连重试（判据参照 028 的 `is_request()` 精神）+ 40-4 超时语义拆分 | 040-A |
| 040-C | 实测服务端 idle timeout（**不可假设**），据此决定是否上预热/复用及其 idle 上限 | 040-A/B + 端测数据 |

**🔴 040-C 是 028 同款雷区，未拿到服务端 idle timeout 实测值之前，禁止上长连接复用。**

### 文件域

`src/transcription/qwen_inference.rs`（与 ASR-038-B 同域，**必须串行，归同一个 Worker**）。

---


## 📋 待派发 · ITN-IDIOM-COVER-032 量级单位字成语零覆盖（2026-08-09 主控取证发现）

> **来路**：`examples/probe_031.rs`（031 临时探测程序）删除前，主控核查其覆盖词的测试情况时发现。
> **性质**：与 031「万一→`0.1万`」**同一类** —— 量级单位字（十/百/千/万/亿）开头的固定表达。

### 取证：六个成语在 `src/itn.rs` 里**零出现**

| 词 | `src/itn.rs` 出现次数 |
| --- | --- |
| 十全十美 | **0** |
| 十有八九 | **0** |
| 百般 / 百般刁难 | **0** |
| 千方百计 | **0** |
| 百里挑一 | **0** |
| 成千上万 | **0** |

对照：`十分` 出现 8 次（有覆盖）、`千千万万` 出现 1 次。

### 为什么这不是「补几个词」那么简单

031 只给**万/亿**两个分支加了守卫（`!has_digit && section == 0 && result == 0 → return None`）。
`十/百/千` 三个分支**至今无守卫**，按 todo 既有记录：百/千分支不设锚点、`result==0` 被拒，
**歪打正着没出事**；而 `:682` 十分支默认 1（`十分`→`10分`）挂了很久，一直是「只报不改」。

**所以本单要先回答一个问题**：这六个词现在的实际输出是什么？
—— 是已经正确（靠 `result==0` 侥幸拦下），还是已经错了但没人测过。
**取证之前不要动代码**，也不要走保护词表（DEC-038：词表不承载规则性语法族）。

### 建议分解（待派发）

| 编号 | 内容 | 负责人 |
| --- | --- | --- |
| 032-A | 六词 + `十分`族实测现状，产出「现状表」（只测不改） | tester-1 |
| 032-B | 依 032-A 结果判定：真缺陷则走机制层给十/百/千分支补对称守卫；已正确则只补断言钉住 | coder-1 |
| 032-C | `:682` 十分支默认 1 一并结论化，不再挂「只报不改」 | coder-1 |

**文件域**：`src/itn.rs`（与任何 ITN 任务互斥，不可并行派发）。

---

### 🧪 给 Gavin 的端测观察点（本批改了什么、该看什么）

| 编号 | 改了什么 | 端测怎么看 |
| --- | --- | --- |
| 030 标点全源治理 | 关掉自动标点开关后，**6 个产出源一视同仁**（原来只控 1 个） | 关开关说一段话，看是否**任何标点都不出现**（含引号、括号、`、；`列表分隔符），且标点位置留空格 |
| 030 短句 ≤5 | 开着开关时，字/词数 ≤5 的短句不加末尾标点 | 说「好的」「今天天气不错」，看末尾有无句号；≥6 字应正常加 |
| 030-A-2 成对符号 | `（笑）` 不再被剥成 `（笑` | 说带括号/引号的短句，看有无孤儿左半 |
| DEC-047 Qwen3 标点 | 在线 ASR 由「假设必有标点」改为**实测**，短句无标点时不再跳过标点引擎 | 用在线 ASR 说短句，看该补的标点有没有补上 |
| 031 万一守卫 | `万一` 不再变 `0.1万`；`亿万`/`百万`/`千万` 同族一并覆盖 | 说「万一下雨怎么办」「百万富翁」，看是否保持汉字 |
| 027 零回归 | 大额数字跨亿到个位 | 说「一千零四十六万八千七百四十一」应得 `10468741` |

---

### 📜 2026-08-09 20:5x TEST-EXEC-030 验收记录（已完成，保留供追溯）

**结果：A0–A7 零 FAIL，四族零回归全绿。** `itn::` 225 ｜ `punctuation::` 43 ｜ `transcription::` 105+4ign
｜ `llm::` 140 ｜ 主 crate 958+8ign ｜ src-tauri 53 ｜ `--list` 自洽 966==966。零生产代码改动。

**主控独立复算（未采信汇总表格）**：用源码 `#[test]` 计数逐项验证，六个数字全部吻合 ——
`itn.rs`=225／`punctuation/mod.rs`=43／`llm/mod.rs`=140／transcription 三文件 54+28+27=109／
src-tauri 22+5+26=53（`#[path]` 引入 `wordbook/mod.rs`+`wordbook/db.rs`）／总数 900+30+36=966。

**`itn::` 用例数争议已裁定为 225**：旧记录 212 失效，coder-1 报的 219→221 为 TEST-SYNC-030-B 之前的中间态。

⚠️ **[DOC-STATE-DRIFT-001] 复现**：tester-1 只更 CHANGELOG + logs 两份，handoffs 与 progress 零条目，主控代记（已标注）。

📌 **只报不改遗留**：`examples/probe_031.rs`（08-08 031 探测残留，未清理未入库）。因 cargo 会自动发现
`examples/`，它会被 `cargo check --all-targets` / `cargo test` 连带编译。**处置待 Gavin 拍板**（删除／入库／保留），
主控不擅自删（依据「不可逆操作必须单独成轮」）。

---

### 📜 2026-08-09 19:38 派发记录（已完成，保留供追溯）

> 🔴 **交接记录有一处不符，已更正**：handoffs 写「任务书原样可用」，但 `inbox/tester-1/task.md`
> 在 19:33 新 session 启动时**已被清空为 0 字节**。主控已按原设计**重写**任务书（7030B），
> 含 A0–A7 步骤、红条三分类纪律（①测试写错可改／②预期内行为变更须给来源依据／③真回归一律上报不动）、
> 四族零回归专项（017 重量链／026 货币链／027 大额数字+DEC-042／031 万一守卫）、
> 红线（禁生产代码改动、禁出包、禁 git 破坏性命令、禁 PowerShell 改 UTF-8、禁虚报须贴原始摘要行）。
> 另要求 tester-1 给出 `itn::` **实跑用例数**，裁定 coder-1 报的 219→221 与 handoffs 记录不符一事。

**三 Worker 启动状态（19:3x 实测）**：coder-1 `%2` / coder-2 `%1` / tester-1 `%3` 全部存活并已 ACK 就绪，
均为 `DeepSeek V4 Flash Free · OpenCode Zen`（与本文件旧记录的 glm-5.2 / kimi-k2.7 已不同）。
coder-1 为 **OpenCode 类型非 Codex**，故未发 `/permissions Full Access`。
本轮 tester-1 独占 `src/`，coder-1/coder-2 已通知空闲待命勿动 `src/` → 零文件级重叠。

**基线数字**（供对照）：`itn::` 221 ｜ `llm::` 134+9 ｜ `punctuation::` 38+10 ｜
`transcription::` 105 ｜ src-tauri 53。Vitest/pytest 本批 SKIP（零前端零 UI 改动）。

**回归风险最高的三处**：① `strip_punctuation` 标点由「删除」改为「换空格」（行为变更，
既有断言大概率绿转红，属②类不是③类）；② `main.rs` L2 收口块删来源判据 + 抽纯函数；
③ `itn.rs` 万/亿分支新增守卫。

**之后**：TEST-EXEC 通过 → BUILD-015 出包（**主控须明确下达「现在可以出包」**）→ Gavin 端测。

### 🔴 待 Gavin 拍板：阶段三规则开例外

**同一根因本批发作三次**，第三次已从「diff 变脏」升级为「主干编译失败」：

| # | 任务 | 后果 |
| --- | --- | --- |
| ① | TEST-SYNC-030 | 代码非 rustfmt-clean，被后续 coder 的 `cargo fmt` 连带归一，污染两个 commit 的 diff |
| ② | 同上 | 同上 |
| ③ | TEST-SYNC-030-B | `src/llm/mod.rs:4776` 多一个 `}`，**整个 crate 编译失败**，主控提交前 `cargo check` 才发现并修掉 |

**根因**：三阶段规则明令阶段三「禁止执行任何命令」，`cargo check` 也在禁止之列 →
tester-1 连括号配没配对都无从知道，**交付前不可能自查**。

**主控建议**：阶段三开白名单例外，只允许 `cargo fmt` + `cargo check`。
理由：规则要防的是「测到半成品、拿到假结果」，而 `fmt` 只格式化、`check` 只做类型检查，
**都不执行测试、不产出二进制**，与该风险无关；禁止它们反而直接导致交付物编译不过。
⏸ **Gavin 未拍板前规则不变**，继续由主控在提交前兜底。

---


## ⏸ 待 Gavin 拍板 · ITN-FIX-BIGNUM-027 遗留三条（2026-08-04 挂账，实施已全部结案）

> 027 全批 A–E 已提交结案（`136f70f` / `967cd8d` / `e79ed05` / `499d56e`），
> 完整实施记录与根因取证已迁入 `todo-archive.md`。本节只留**仍未拍板的三条断言**。
> 🔴 其中「十万个为什么」一条**已由 DEC-044 解决**（专名加入保护白名单），保留行文备查即可，无需再拍。

### ⏸ 待 Gavin 拍板三条（tester-1 已按现状锁定断言，标注「待拍板」）

| 用例 | 现状 | 问题 |
| --- | --- | --- |
| 一万亿 | `10000亿` | DEC-042 规则直接推论，但中文习惯说「1万亿」。三选项：`10000亿`（守规则）/ `1万亿`（万亿当一个量级）/ `1000000000000`（特例展开） |
| **十万个为什么** | **`10万个为什么`** | 🔴 **专名被转，书名被改写**。属 DEC-038 词表覆盖问题（与 `五毛钱`/`三毛钱` 同源），**非 027 引入**，不阻塞出包，是否开单待定 |
| 两万五百 | `20500` | 最小单位百 → 普通数字，主控判断符合规则，列此备查 |

---

## 📋 待派发 · ITN 成语漏保护：`三五成群` → `35成群`（2026-08-04 Gavin 端测顺带发现）

> **性质：漏保护 = 输出损坏**（比误保护严重）。本次侥幸被 LLM 救回并进了 suggestions，若 LLM 超时走本地兜底，用户直接看到 `35成群`。

**取证**：`target/release/debug.log:2797`
```
ASR:  再比如说有同学三五成群打小赌
ITN:  再比如说有同学35成群打小赌
LLM:  三五成群（改回来了，suggestions:["三五成群"]）
```

**派发时机**：⏸ **等 027 完成后再派** —— 与 027 同属 `src/itn.rs` / `itn-rules.toml` 文件域，并行会冲突（文件级重叠检查）。

**修法待定**：`三五成群` 是成语（不可推导的固定表达），按 DEC-038 属于「保护词表**应该**承载」的那一类，可走词表。但需先确认 `[protect.idioms]` 为何未覆盖 —— 若成语表本就该有它，那是词表缺漏；若是逐位串路径绕过了成语保护，则是规则层缺陷，须走规则层修。**开单前主控先做这项取证。**

---


## 文档更新规则

1. **只保留新产生、进行中、验证失败、待排期或其他待决的任务**
2. **已完成的功能任务立即归档到 progress.md**，不在 todo 保留历史
3. **测试同步/构建/出包任务归入 CHANGELOG**，不在此文档详列
4. **新任务产生时立即写入**，不批量补
5. **不列端测跟踪项**（Gavin 自行使用中测试，有问题会重新开单）

---


## 📋 待排期 · LLM-USAGE-OBSERVABILITY-001 解析 usage 缓存字段（Gavin 2026-08-04：先建待办，下次再做）

> 性质：**可观测性补齐**，改动极小，不碰提示词本身。
> 前置：无，可随时启动。

### 背景：97% 的 prompt token 花在固定前缀上

主控 2026-08-04 从 `target/release/debug.log` 实测：

| 场景 | system_prompt | prompt_tokens |
| --- | --- | --- |
| 多行（Notepad/文档类，`multiline_safe=true`） | **21,868 字符** | **~5,300** |
| 单行（IDE/终端/微信，`multiline_safe=false`） | **13,174 字符** | **~3,269** |

差额 8,694 字符是多行分支独有的 F3 格式契约 + 四语枚举标记清单（023 恢复的 132 个标记短语）。

构成：主体（L0-L3 分层 + F3 + 四语清单 + i18n 基座）~21,400 ｜ 场景 F4 块 226 ｜ `extra_instruction` ~210 ｜ 词库 12 条。

**用户实际语音内容仅 62-232 字符（~50-150 tokens），completion 仅 17-64 tokens** —— 即每次请求 **97% 以上的 prompt token 是完全相同的固定前缀**。

### 问题：我们看不到缓存有没有命中

DeepSeek 支持上下文缓存（相同前缀命中后计费更低、延迟更低），而我们的系统提示词**每次完全相同**，是理想的缓存对象。

但 `ChatResponse`（`src/llm/mod.rs`）**没有解析 usage 里的缓存相关字段** —— 日志只有 `prompt_tokens` / `completion_tokens` / `reasoning_tokens`，**无法判断缓存是否命中、命中率多少**。

**在补上这个字段之前，任何关于提示词成本的讨论都只是估算。**

### 与既有教训同源

`[LLM-COT-LEAK-001]` 教训 6 原文：

> HTTP 响应结构体只解析自己要用的字段，会丢掉排障关键信息。`finish_reason` / `usage` 是零成本可观测性，应该默认解析。本次因缺这两个字段，一个本可一眼看穿的截断问题耗掉了整轮研究任务才坐实。

**同一个毛病第二次出现**。上次缺 `finish_reason`/`usage` 害得排查绕了一整轮，这次缺缓存字段导致成本无法量化。

### 实施范围（预估：一个 coder 小任务）

1. `ChatResponse` 的 usage 结构体补解析缓存相关字段（**实施前先查证 DeepSeek 官方响应 schema 的确切字段名**，不要照猜）
2. `LLM response meta` 日志行追加缓存字段输出
3. 跨 endpoint 兼容：本项目允许用户填任意 OpenAI 兼容 endpoint，其他厂商字段名可能不同或缺失，**用 `Option` 容错，缺失不得影响主流程**
4. 不碰提示词本身，不改任何现有行为

### 后续可能的衍生

拿到命中率数据后才谈得上判断：是否需要为缓存友好而调整提示词结构（如把最易变的 F4 场景块与词库注入移到末尾，保证前缀稳定）。**数据出来之前不做任何提示词改动**（DEC-041 与 `[F3-UNORDERED-LIST-001]` 的教训：提示词改动必须由实测驱动）。

---

## 📋 待排期 · SCENE-FORMAT-DIMS-001 拆分 `multiline_safe` 为格式能力多维度（Gavin 2026-08-01：先建 todo，后续排期）

> 性质：**内部架构重构**，无用户可见配置项。需一次构建。
> 前置：无。可随时启动。**风险点是它动的正是刚稳定下来的 prompt 构建链。**

### 问题

`multiline_safe`（单 bool）在 `src/main.rs:3073` 传入 `build_optimize_request` 后，**一个人决定了三件事**：

| 它控制 | 位置 |
| --- | --- |
| `<corrected>` 能否跨行 | `build_output_format(multiline_safe)`（`src/llm/mod.rs:795`） |
| F3 用多行列表还是单行内联 | `build_format_instruction_block(multiline_safe)`（`:818`） |
| 是否 `flatten_multiline` 压成一行 | `src/llm/mod.rs:660` |

**这几个问题的答案并不总是一致。已实际撞上一次**：代码编辑器要换行、不要 `- ` 项目符号 —— 当时只能用「方案 A：接受列表」绕过（Gavin 2026-08-01 拍板）。

### 需求矩阵（主控已梳理）

| 场景 | 换行 | 列表 | `#` 标题 | 表格 |
| --- | --- | --- | --- | --- |
| Markdown 编辑器 | ✅ | ✅ `- ` | ✅ 想要 | ✅ 想要 |
| 代码编辑器 | ✅ | ❌ 不想要 | ❌ | ❌ |
| Word / WPS | ✅ | ✅（会自动转项目符号） | ❌ | 视情况 |
| 记事本 | ✅ | `- ` 仅字面字符 | ❌ | ❌ |
| 邮件 | ✅ | ✅ | ❌ | ❌ |

### 设计草案

```toml
newline    = true          # 能否多行注入
lists      = true          # 能否用列表
list_style = "markdown"    # markdown(- / 1.) | plain(• / 1.) | none
headings   = false         # 能否用 # 标题
tables     = false
```

### 影响面

`scene-rules.toml` schema ｜ `SceneRule`（`src/scene/mod.rs:94`）｜ `SceneContext`（`:55`）｜ 解析与传递 ｜ `build_output_format` / `build_format_instruction_block` 从 bool 参数改结构体 ｜ 相关测试。

### 实施前必查

1. **回查 DEC-031 单开关原则** —— 主控初判不触碰（这是内部规则数据，非用户可见开关），但须正式确认
2. 跨平台：`src/scene/mod.rs` 与 `src/llm/mod.rs` 均为平台中立模块，对 macOS 透明，不违反 DEC-033
3. **迁移成本随词表增长而上升** —— 现已 9 个 scene 块，越晚做越贵

### 附带清理（本项一并处理）

`scene-rules.toml:15` 注释称 `multiline_safe` 还控制「强制剪贴板」，**主控 grep 注入侧代码零引用，该句为过时描述**，一并订正。

---

## 📋 待排期 · SCENE-FOCUS-PROBE-001 焦点控件类型探测（UIA / AX）（Gavin 2026-08-01：先建 todo，后续排期）

> 性质：**架构级新能力**，跨平台，需完整评估。
> 🔴 **前置硬门槛：必须先做 PoC，PoC 不通过则不立项。**

### 问题：只有窗口级信号，要判断的却是控件级行为

场景识别拿到的是 `(进程名, 窗口标题)`，**两者都是窗口级**。而「Enter 是换行还是提交」取决于**当前焦点控件**：

| 应用 | 同一窗口内的分歧 |
| --- | --- |
| VS Code / Cursor | 编辑器（Enter=换行）／集成终端（Enter=执行）／AI 面板（Enter=发送） |
| Todo 软件 | 快速添加框（Enter=建任务）／详情备注（Enter=换行） |
| 设计软件 | 文本图层（Enter=换行）／评论框（Enter=发送） |
| 浏览器 | 文档编辑区／评论框／搜索框 |

**当前所有残留误判风险的总根源。** 现在是「按应用整体赌一个答案」。

### 技术路径

| 平台 | API | 信号 |
| --- | --- | --- |
| Windows | `IUIAutomation::GetFocusedElement()` | `ControlType`：`UIA_EditControlTypeId` ≈ 单行；`UIA_DocumentControlTypeId` ≈ 多行。辅以控件 ClassName（如 Windows Terminal 的 `TermControl`） |
| macOS | `AXUIElement` → `AXFocusedUIElement` | `AXRole`：**`AXTextField` = 单行、`AXTextArea` = 多行**。信号比 Windows 更干净 |

接缝已存在：`capture_scene_signals` 两平台签名同形（Windows `src/platform/windows/scene.rs:22`，macOS `src/platform/macos/mod.rs:79` 仍为 stub）。

### 🔴 PoC 必须先回答的三个问题

1. **Chrome / Electron 能否拿到有意义的焦点控件类型？**
   Chrome 默认不开放完整无障碍树（只在检测到读屏器时启用），Electron 应用同理。
   **而 `chrome.exe` 占真实听写量的 36%（151/414，主控 2026-08-01 从 debug.log 实测）。拿不到 = 方案价值砍半。这是头号风险。**
2. **耗时**：跨进程 COM 查询通常 5–50ms，但**对无响应应用可能挂住** → 超时与降级策略是否可行
3. **VS Code 编辑器 vs 集成终端**能否区分 —— 这是最想解决的那个用例

**PoC 规模：半天量级，只验上述三点，不写生产代码。**

### 其他约束

- 跨平台：Windows COM + macOS AX 两套实现，DEC-033 要求不得只产出仅 Windows 可编译的新代码
- 隐私：**严格只取控件类型，绝不读取控件内容**
- 失败降级：拿不到信号时必须退回现有 `(exe, title)` 判定，不得阻塞管线

### 与 SCENE-FORMAT-DIMS-001 的关系

两者正交但互补：本项解决「**当前焦点适合什么**」，前者解决「**适合的东西怎么表达**」。建议**先做拆维度**（收益确定、成本有界），本项走 PoC 门禁。

---

## 📋 待裁定 · 领域级泛化关键词第二批（coder-1 建议，主控未采纳未否决）

coder-1 在 DATA-SCENE-GENERIC-008 中评估后建议的候选：**`思维导图` / `白板` / `表格`**。

已评估并**否决**的：`笔记`（小红书网页标题大量含之，社交类判 doc 方向反了；「笔记本电脑」购物页同样命中）、`文档`（帮助/API/产品文档等阅读页覆盖过宽，失去判别意义）。

**裁定判据（主控 2026-08-01 定）**：看误伤会落到哪个方向 ——
- 误伤 → **doc**：只多给多行与列表，文本仍正确，属优雅降级，**可放宽**
- 误伤 → **把 chat 类应用判成 doc**：多行注入发帖/发消息框，可能把一条拆成多条发出，**必须卡严**

待 Gavin 拍板。

---
## 📋 待排期 · ITN-V2-P6 能产语法族批量收口（Gavin 2026-08-01：先出包，P6 另立）

> 依据：`collab/research/itn-v2-grammar-family-scan-006.md` 的 130 族分类结果
> 前置：本轮出包端测反馈

**范围**：75 个 🔴 能产语法族（`N点钟`/`N秒钟`/`N块钱`/`N毛钱`/`N角钱`/`N位数`/`N年级`/`N节课`/`N件套`/`N句话`/`N日游` 等）从 `[protect.unit_collisions]` 移出，交甲/乙/丙型文法处理。

**核心问题**（Gavin 已实感）：`五毛钱` 在表保持汉字、`三毛钱` 不在表变 `3毛钱` —— 同一表达因数值不同行为完全相反（DEC-038）。

**实施前必须**：① 逐族确认已有文法覆盖，避免移出后变成「漏保护」（输出改坏，比未优化严重）② 反向护栏：55 个 ⚪ 专名族不得误伤 ③ 预判并同步既有断言 ④ 涉及数百词条，需完整回归 + 出包

### 📌 2026-08-04 补充：分类扫描已完成，Gavin 拍板暂缓

> **Gavin 2026-08-04 决定**：「先保持原样，后面我端测了有问题再开单」。**P6 不启动，等端测反馈。**

主控已完成当前词表复扫，成果存 **`collab/research/itn-v2-p6-family-classification-001.md`**，含：

- 129 个多数字族的**四组分类**（A 数量表达 37 移出 / B 地名编号 49 保留 / C 术语 41 保留 / D Gavin 指定保留 2：**N年级 / N节课**）
- 🔴 **新发现**：`一个*` 前缀 **134 条占全表 10% 是挖掘噪声**（`一个三十多岁` 会冻结「三十」不转），删除零专名风险，是 P6 成本最低的第一刀
- 🔴 **新发现**：979 个单例族占 72%，**按族裁决方法在此失效**，需改按尾字聚类
- 三处待 Gavin 裁定：`N分熟` / `N米板` / `N岁时·N月底`

⚠️ **四组分类是主控单方初判，Gavin 未逐组确认**，P6 启动时必须先过一遍，不得直接采信。

**启动前另一道门**：CHANGELOG 026-B 的「12 词条实测确认」已被 TEST-EXEC-026 证伪 4 条，须先做 026 前后行为差分。

### 八、待 Gavin 决定的两项（非阻塞）

- `dispatch.sh` 路径分叉修复时机（`[COLLAB-PATH-SPLIT-001]`）
- 是否加 `.gitattributes` 收口 CRLF 幽灵 diff（`core.autocrlf=true` 且无 `.gitattributes`，已两次干扰验收判断；属仓库级改动、影响 macOS 侧，需协调）

---


## 🍎 [macOS 侧] 2026-08-04 · Phase 4 管线实现规划（**规划稿，尚未派发**）

> 来源：Gavin 2026-08-04 指令「把 macOS 侧管线 Phase 4 的任务规划出来」
> **完整方案见 `collab/research/macos-phase4-plan-001.md`**（含逐任务的影响文件 / 具体改动 / 验收标准 / 边界矩阵）
> 基线：`0adb819`（v0.7.3），主控本次逐文件实测，未采信既有文档结论

**🔑 核心发现（改变了任务分解方式）**：`run_pipeline`（`main.rs:2812-3214`，402 行）与 `spawn_worker_thread`（`:2161-2459`，299 行）虽整体被 `#[cfg(target_os="windows")]` 门控，但**平台接触点分别只有 4 个和 1 个**，其余全是平台中立业务逻辑。**约 700 行核心管线可一次性转为双侧共享**，macOS 侧无需重写任何业务逻辑。这同时修正了 `docs/MACOS-HANDOFF.md` §2.8「管线代码层无法共享」的表述——那是当前状态，不是固有属性。

| 阶段 | 编号 | 内容 | 建议负责人 | 状态 |
| --- | --- | --- | --- | --- |
| A | MACOS-P4-PROBE-001 | 实机可行性探针：cpal 实录 / sherpa-onnx 实跑 / TCC 权限 / CGEventTap 实收 / AX 授权弹窗 | tester-1 | ✅ **已验收**（A1/A2/A5 PASS，**A4 FAIL 查出真 bug**，A3 强度不足待复验） |
| A+ | MACOS-P4-FIXHOTKEY-001 | 修 `KEYBOARD_EVENTS` 事件掩码越界（A4 的下游修复） | coder-2 | ✅ **已验收**（2026-08-04，主控独立复跑 `cargo check --all-targets` 0 errors） |
| B | MACOS-P4-NEUTRAL-001 | 管线去平台化：`HWND`→`WindowId(usize)`、新增契约符号 `foreground_window_id`、去 cfg 门控 | coder-1 | ⏸ **归属待拍板** |
| B | TEST-SYNC-P4-NEUTRAL-001 | 契约单测 + macOS 可达性烟测 | tester-1 | ⏸ |
| C | MACOS-P4-HOST-001 | macOS 事件宿主 + 主循环 + 单实例锁 | coder-2 | ⏸ **选型待拍板** |
| C | MACOS-P4-TRAY-001 | macOS 托盘（**不可与 HOST 并行**） | coder-1 | ⏸ |
| C | **MACOS-P4-OVERLAY-001** | 录音浮层（Recording，P0）NSPanel 1:1 复刻 Win32 | coder-1 | ✅ **已验收**（2026-08-05，返工后主控独立取证：BORDER_GRAY `#070606` 已修正、性能四项已补数、SIGBUS 悬垂指针真缺陷已重构、probe 已删、工作区干净，提交 `d7b9f39`） |
| C | **MACOS-P4-OVERLAY-WIRE-001** | **浮层接线**：接进模块树 + 跨线程请求通道 + 15ms timer 主线程应用 + PipelineEvent 七分支映射 | coder-1 | ✅ **接线部分已验收**（主控独立取证：`mod overlay;` 已加、overlay 5 条单测**首次真进测试二进制**（主控自跑 `--list` 确认）、Windows 导出块零改动、七分支齐全）。⚠️ **实机闭环仍降级**，转入 CFGGATE-001 C 项 |
| **P0** | **MACOS-P4-CFGGATE-001** | 🔴 **修 macOS 专用函数缺 cfg 门控致 Windows 编译必炸**：`main.rs:2880/:2930` 无 `#[cfg]` 却引用 `request_tray_state`（TRAY-001 既有）+ `request_overlay`/`OverlayRequest`（WIRE-001 新增 7 处）。**Windows 侧已编不过一天多无人察觉**。含 11 个 macOS 专属符号全量扫描 + 实机重试 | coder-1 | ✅ **已验收**（主控**自写脚本独立重扫**，非采信 Worker 表：11 符号全部落 cfg(macos) item 内、未门控引用点 **0**；`cargo check --all-targets` CARGO_EXIT=0 / error 0；diff 恰好 +2 行；**主控追加全仓扫描**确认 macos 目录与 main.rs 之外零引用。🟢 **Windows 编译阻塞解除**） |
| C | TEST-SYNC-P4-OVERLAY-WIRE-001 | 阶段三**预研**（只读零改动，产出写 outbox）：5 条单测走查 + 七分支真值表 + 线程契约 + 盲区预估 | tester-1 | 🔄 **进行中**（与 WIRE-001 并行安全，因零文件落地） |
| C | MACOS-P4-OVERLAY-002 | 处理中 / 失焦返显 / 错误三态浮层 + **指示灯形状裁定**（Windows 实为麦克风图标，现实现为实心圆，主控裁定本轮不动） | 待定 | ⏸ |
| ~~C~~ | ~~MACOS-P4-FEEDBACK-001~~ | ~~托盘状态替代浮层~~ → **已按 DEC-036 取消替代定位**（Gavin：不能用托盘图标代替录音窗口，不符合用户体验） | — | ❌ 作废 |
| D | MACOS-P4-SCENE-001 | `capture_scene_signals` 真实现（NSWorkspace + AXUIElement） | coder-1 | ⏸ |
| D | MACOS-P4-PERM-001 | `AXIsProcessTrustedWithOptions` 真实现（现为只打 log 的假实现） | coder-2 | ⏸ |
| D | MACOS-P4-AUTOLAUNCH-001 | 自启动（建议 SMAppService） | 待定 | ⏸ |
| **P1** | **MACOS-P4-AXINJECT-001** | **辅助功能 API 直写替代剪贴板注入**（解决剪贴板被污染/图片被永久覆盖）—— Gavin 2026-08-05 拍板独立 P1；**同日 12:1x 派发 coder-2**。三条主控裁定：①必须 `kAXSelectedTextAttribute`，**严禁** `kAXValueAttribute`（会抹掉用户已输入内容）②必须 `AXUIElementSetMessagingTimeout(0.5)`（否则无响应 App 会挂死 pipeline worker 线程）③必须检查 `AXError` 返回码 | coder-2 | ✅ **已验收**（三条裁定逐条 Read 核对全部落实；CARGO_EXIT=0/error 0；仅 `injection.rs` +122/−1）。🔴 **但主控 review 查出静默丢词风险** → 见下行 002 |
| **P1** | **MACOS-P4-AXINJECT-002** | 🔴 **堵 AX 静默丢词**：部分实现 AX 的 App（Electron/Java Swing）接受 `SetAttributeValue` 并返回成功却不真插入 → `inject_text` 拿 Ok 即 return、**永不走剪贴板兜底** → 用户整段话消失。**推翻了 001 立项前提「最坏等同现状」**。修法：`AXUIElementIsAttributeSettable` 预检 + AX 成功日志带 `AXRole`。已否定两条歧路（写后读回：选区塌缩读回空串不可作判据；role 白名单：误伤自定义控件） | coder-2 | 🔄 **进行中** |
| C | **MACOS-P4-OVERLAY-WIRE-002** | 抽纯函数 `overlay_request_for_event` 使七分支真值表可单测（**强制穷举 match，禁 `_ =>` 通配符**——通配符会让将来新增 PipelineEvent 变体静默落进 Hide）。由来：tester-1 预研指出映射内联在 handler 里夹三种副作用、单测调不动 | coder-1 | 🔄 **进行中** |
| C | TEST-SYNC-P4-OVERLAY-WIRE-001 预研 | ✅ **已交付**（13683 B）。**核心发现（主控复核认同）：overlay 现有 5 条单测 4 条凑数 1 条半有效** —— `decay_rate_matches_windows` 是 `assert_eq!(常量, 它自己的字面量)`；三条波形测试独立重算公式不调生产路径，且 `lx<rx`／`8<=16` 恒真。**本次接线目前零真护栏** | tester-1 | ✅ 预研完成，阶段三待 WIRE-002 交付 |
| D | MACOS-P4-READBACK-001 | AX 回读（学习路径）—— ⚠️ **主控已收回「相对 Windows 的能力优势」表述**：300ms 观察窗 + Word/Google Docs 读不到 + 全文 diff 昂贵，三重打折后产出存疑；建议优先考虑 `WORDBOOK-CORRECTION-UI-001` 显式纠错路线 | 待定 | ⏸ **降级** |
| ~~E~~ | ~~MACOS-P4-OVERLAY-001~~ | 已上移至阶段 C（DEC-045） | — | ↑ |
| E | MACOS-P4-BUNDLE-001/002 | `.app` 打包 + Info.plist TCC 声明 + 自签名证书（BUNDLE-002 已改自签名，禁 ad-hoc） | coder-2 | ✅ **已完成**（BUNDLE-002 2026-08-05：`build-macos.sh` 自签名 Feiyin Dev，实机 Build completed + Authority=Feiyin Dev；详见 outbox/coder-2/result.md）。⚠️ 公证仍不可做（无 Apple Developer 账号） |

**边界评估结论**：阶段 B 独占 `src/main.rs`，期间禁止任何其他任务碰它；阶段 C 的 HOST 与 TRAY **必须串行**；阶段 D 的 SCENE / PERM / AUTOLAUNCH **可三路并行**，唯一交汇点 `macos/mod.rs` 的 re-export 行由主控统一改一次。

**⏳ 阻塞在 Gavin 决策上的 5 项**（详见方案 §4）：① 阶段 B 归属与 Windows 零回归验证方 ② 事件宿主选型（**建议复议 DEC-015 的 Tauri，改用 winit**）③ 四个浮层是否进第一版 ④ Apple Developer 账号 ⑤ AX 回读是否提前。

---

## 🍎 [macOS 侧·待排期] MACOS-P4-AXINJECT-001 · 辅助功能 API 直写替代剪贴板注入

> **⚠️ 本条仅适用于 macOS 侧，Windows 侧无需处理**（Windows 走 SendInput / 剪贴板双通道，机制不同、无此缺陷）。
> 来源：Gavin 2026-08-04 端测关切「现在的回写方式确实会污染剪贴板，干扰到用户剪贴板输入」。
> 参照：竞品 **Typeless** 官方文档明示其用 Accessibility 权限「paste text into any text field」，**不经剪贴板**（[installation-and-setup](https://www.typeless.com/help/installation-and-setup)）。

### 现状与缺陷（主控实测 `src/platform/macos/injection.rs:49-64`）

当前 `inject_via_clipboard` 流程：`pbpaste` 存旧内容 → `pbcopy` 写新文本 → sleep 50ms → enigo 发 Cmd+V → sleep `delay_ms` → `pbcopy` 恢复旧内容。

**已有恢复逻辑，但存在四个真实缺陷（按严重度排序）**：

| # | 缺陷 | 后果 |
| --- | --- | --- |
| **1** | **`get_clipboard_text()` 只用 `pbpaste`，仅支持纯文本**（`:119-134`） | 用户剪贴板里若是**图片 / 富文本 / 文件 / 多格式数据**，`old_content` 取不到 → `None` → **恢复分支根本不执行** → **用户原剪贴板内容被永久覆盖丢失**。这不是"污染"，是**数据丢失** |
| 2 | 恢复是尽力而为（`let _ = set_clipboard_text(&old)`，忽略错误） | 恢复失败无感知、无日志 |
| 3 | 约 **100ms+ 时间窗**内剪贴板是我们的内容 | 用户此刻手动粘贴会拿到错误内容 |
| 4 | **剪贴板历史工具**（Paste / Maccy / Raycast 等）会记录每一次写入 | 每次听写都往用户剪贴板历史里塞一条垃圾，长期使用体验很差 |

### 目标方案

用 **AXUIElement 直写**替代剪贴板通道：取焦点元素（`kAXFocusedUIElementAttribute`）→ `AXUIElementSetAttributeValue` 写 `kAXValueAttribute`，或用 `kAXSelectedTextAttribute` 做插入。**全程不碰剪贴板。**

### 实施要点

- **保留剪贴板通道作为兜底**：AX 对某些 App（Electron / 部分 Java App / 未实现 AX 的自绘控件）不可用，失败时回退现有 `inject_via_clipboard`，与 DEC-018「clipboard-first + enigo fallback」的分层思路一致，只是把优先级改为 **AX → 剪贴板 → enigo.text()**
- **权限已具备**：辅助功能权限本就是热键（CGEventTap）的前置条件，`MACOS-P4-PERM-001` 已补全授权弹窗，**不需要新增任何权限申请**
- **保底路径不受 App 差异影响**：AX 写不进去就回退现有剪贴板通道，**最坏情况等同现状，不会更差**

### 🔴 与 `MACOS-P4-READBACK-001` **不得捆绑**（Gavin 2026-08-05 拍板：AXINJECT-001 独立 P1）

> **主控此前建议"两者合并为一个批次"，该建议已作废并收回。** 它们只是碰巧用同一套 `AXUIElement` API，但**价值与风险完全不同**，捆绑会让高价值的直写被低价值的回读拖累。

| 项 | 价值 | 依赖 |
| --- | --- | --- |
| **AXINJECT-001（本条，注入）** | **高且确定**。解决真实数据丢失 + 剪贴板历史污染。**只需"写得进去"，有剪贴板兜底** | 无 |
| `READBACK-001`（回读，学习） | **存疑**，见下 | 受三重打折 |

**主控收回的一处过乐观表述**：此前写「macOS 的 AX 能真正读回，是本平台相对 Windows 的能力优势」。**该表述不准确**，实测复核后三点打折：

1. **观察窗只有 300ms**（`main.rs:161` `AUTO_LEARN_OBSERVE_MS = 300`，`:3662` sleep 后即读一次就结束）。用户不可能在 300ms 内看完注入文本、发现错字并改掉——**真实纠错发生在数秒后，这条路径设计上就抓不到几乎任何真实纠错**。此缺陷与平台无关，Windows 侧同样存在
2. **AX 覆盖面确实比 `WM_GETTEXT` 广**（原生 NSTextView / 备忘录 / Safari 输入框可读，而 `WM_GETTEXT` 在现代应用基本全废），**但读不到 Microsoft Word（非原生 AX 文本控件）与 Google Docs（编辑面是 canvas）** —— 而这恰是长文写作主场
3. **全文 diff 代价高**：`extract_changed_text`（`main.rs`）走公共前缀+后缀夹逼，**数学上对整篇文档成立、不需要知道光标位置**，但要把全文 `chars().collect::<Vec<char>>()` **两次**（before/after）。50 页文档每次听写数十 MB 临时分配

**→ 真要修好「注入后纠错学习」，正确路线不是 AX 回读，而是既有待排期项 `WORDBOOK-CORRECTION-UI-001`**（注入后浮层显示「纠错」小按钮，用户点了才进编辑）：**显式交互，不猜、不轮询、不受 App 差异影响，且没有时间窗问题**。

### 影响文件（预估）

`src/platform/macos/injection.rs`（主体）+ 可能新增 `src/platform/macos/ax.rs`。**不碰 `src/platform/windows/**`，不碰平台中立模块，对 Windows 侧零影响。**

### 优先级：**P1 独立排期**（Gavin 2026-08-05 拍板）

**不阻塞当前闭环** —— pbcopy+Cmd+V 现在能工作。但 Gavin 日常使用会持续被缺陷 1（复制的图片被永久覆盖）与缺陷 4（剪贴板历史被塞垃圾）硌到，**Phase 4 主体交付后立即排期，不等 READBACK**。

---


## 📋 待排期 · ITN-COLLISION-TYPEB-001（Gavin 2026-07-30：先生成待办，以后再动手）

> 来源：RESEARCH-ITN-LEXICON-001（`collab/research/itn-lexicon-collision-001.md`）
> 前置：Type A 净化版落地（进行中）｜ **本项需改 Rust 代码，不是纯数据**

**问题**：`is_unit` 用 `s.starts_with(u)`（`src/itn.rs:259`）+ 中文无词边界 → 任何以单字单位词开头的词都会让前面的单字数字被误转。已实证一例（`三角形`→`3角形`，已由几何白名单挡住）。潜在碰撞词 **29,774 条**（Type B：单位字开头，如 `批发`/`元素`/`度假`/`节目`/`升级`/`克服`）。

**⚠️ 主控核实的关键结论（研究报告未发现，实施前必读）**：

1. **Type B 词放进 `protect.proper_nouns` 是 no-op** —— `check_protection`（`:1063`）的匹配起点是**数字位置**（`rest = chars[start..]`，`start` 为数字下标，调用点 `:678`），既有条目全是数字开头（`三亚`/`五一`/`八达岭`）。文本「三度假」的 `rest` 是「三度假」，**不以「度假」开头**，永不命中。研究报告建议的落地方式对这 93% 的词无效
2. **正确改法在 `is_unit` / `decide_conversion` 侧**：判断「数字之后的文本是否以某碰撞词开头、且该词比单位匹配更长」→ 是则不算单位语境
3. **性能必须一并处理**：`check_protection` 是对 **Vec 线性遍历** `starts_with`（不是 HashSet 查表，报告此处亦有误）。29,774 条 × 每个数字位置 = 每次听写十万量级字符串比较 → 需改**按首字分桶或 Trie**，并给实测数据

**实施前必须完成**：许可证原文证据（见 Type A 任务）｜ 完整影响面评估 ｜ 性能实测 ｜ 反向护栏测试（must-convert 表达不被误保护）

---


## ⏸ 待 Gavin 拍板 · RESEARCH-SCENE-COVERAGE-001 场景词表扩展研究（2026-07-28 方案已回）

> 从原 `todo.md`「进行中」大节中抽出——该大节其余内容（macOS 07-30 交接接管等）已迁入 `todo-archive.md`，
> 只有本项仍**待 Gavin 拍板三项**，故保留在 todo。

### 2026-07-28 · RESEARCH-SCENE-COVERAGE-001 场景词表扩展研究（方案已回，**待 Gavin 拍板三项**）

| 编号 | 内容 | 产出 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| RESEARCH-SCENE-COVERAGE-001 | 场景感知软件词表与分类体系扩展研究（纯研究，零文件改动） | `collab/research/scene-rules-expansion-001.md`（397 行） | coder-1 | ✅ 已验收（**含主控三处实质修正**） |

**主控独立核验结果**：

- **边界合格**：`git status` 零改动，`scene-rules.toml` / Rust 源文件全未碰；result.md 4300 字节非空（[COLLAB-WRITE-001] 未复发）
- **✅实测层可信**：抽查 6/6 全部属实——Obsidian 目录确无 `Obsidian-helper.exe`、`D:\xshell\Xagent.exe` 存在、`MarkText.exe` / `Koodo Reader.exe` 路径正确、`wezterm.exe` 确在 `%LocalAppData%\Programs\WezTerm\`（主控首轮搜索因未覆盖该根目录误判为不存在，二次广域搜索证实 coder-1 无误）、四个 AppxManifest 的 Executable 字段逐一复读一致（OneCalendar→`CalendarApp.Gui.Win10.exe`、OutlookForWindows→`olk.exe`、MSTeams→`ms-teams.exe`、Claude→`app\Claude.exe`）
- **❌ 主控修正一：10 条 title_keywords 是 no-op**。报告建议把「微博/小红书/知乎/Jira/TAPD/禅道/Linear/Teambition/Salesforce/Zendesk」加进 **browser 块**的 title_keywords，目的是「浏览器场景细分时命中」。但 `src/scene/mod.rs:162-164` 浏览器细分循环里有 `if other_rule.kind == SceneKind::Browser { continue; }` —— **显式跳过浏览器自身的 title_keywords**。浏览器 exe 在优先级 1 命中后直接返回 Browser，这 10 条永远不会生效（只有在 exe 未命中任何规则的优先级 2 路径才会被查，那不是目标场景）。报告第 6.2 节还引用 `:159-177` 称该机制「已正确处理」，属误读 `continue` 分支。**结论：10 条全部作废**；若确要生效，必须放进**非 browser 的 kind 块**（如 chat）
- **❌ 主控修正二：`OneNote.exe` 是零效果重复条目**。exe 匹配大小写不敏感（`:133` 编译期 `to_lowercase` + `:152` 输入 `to_lowercase`），`OneNote.exe` 与表内既有 `ONENOTE.EXE` 归一化后完全相同。报告自己在第 127/313 行两次援引「toml 不区分大小写」，此处却当作新增项，自相矛盾。**22 条新增实际净 21 条**
- **⚠️ 主控修正三：✅官方 置信度标注失真（最重要）**。报告附录第 397 行自述「WebFetch 核实国内站点被 JS 占位，**主要依靠本机实证 + 通用软件知识**」——「通用软件知识」即凭记忆。故 15 条标 ✅官方 的条目（豆包/Kimi/通义/文心/ChatGLM/GLM/纳米/Perplexity/Zoom/腾讯会议/Linear/Figma/Mailbird/The Bat!/Nu），其证据实际只支撑「该产品有 Windows 版」，**不支撑「进程名就叫 X」**。这恰好触碰本次任务设定的核心红线。**须降级为 ⚠️ 未证实**
- **争议项（主控不同意见）**：报告建议删除 6 条历史推测项。主控评估**成本收益不对称**——不存在的 exe 名在运行时永不命中、开销为零（HashSet 查表），删除买不到任何收益；而万一其中某条在特定版本真实存在，删除即制造静默回归。**建议改为保留 + 注释标注「未证实」**，与 DEC-031-⑤「词表尽可能详细」一致
- **分类结论认同**：D5 六个候选场景全部归入现有 5 类、不新增 kind——主控同意（新增 kind 要改 Rust 枚举 + 解析 + 单测 + 重新出包，收益不足）。但 **Figma 归 ide_terminal 存疑**：该 style 是「保留技术术语/代码标识符、无客套」，而 Figma 文本框内容多为设计文案而非代码，`browser` 的「web-friendly 简洁单行」更贴切

**Gavin 三项拍板（2026-07-28，全部按主控建议）**：
1. **6 条历史推测项** → **保留 + 注释标 ⚠️未证实**（不删除）
2. **15 条未证实 exe** → **落地 + 注释标 ⚠️未证实**（错条目零成本；待 SCENE-OBS-001 日志实证）
3. **Figma** → 归 **browser**（非 ide_terminal）

| 编号 | 内容 | 影响文件 | 负责人 | 状态 |
| --- | --- | --- | --- | --- |
| IMPL-SCENE-COVERAGE-001 | 词表扩展实施：6 条存疑项保留改注释 + 新增 21 条 exe（✅实测 6 / ⚠️未证实 15）+ doc 块补 4 条工单 title_keywords + Figma 归 browser + Skype 注释 | `scene-rules.toml`（仅根目录） | coder-1 | ✅ **已验收（含主控两处修正）** |
| TEST-SYNC-SCENE-COVERAGE-001 | 测试同步：P0-1 `BUILTIN_RULES` 解析单测（堵住「toml 语法错→静默全 Unknown」黑洞）+ P0-2 特殊字符条目（`The Bat!.exe` / `Koodo Reader.exe`）+ P0-3 doc 块 title_keywords 浏览器细分生效 + P0-4 反向护栏锁死 `:162-164` continue + P0-5 归类决策断言 + P1 大小写不敏感 | `src/scene/mod.rs` `#[cfg(test)]` | tester-1 | ✅ **已验收** |
| TEST-EXEC-SCENE-COVERAGE-001 | 全量回归 686/0/8 + Tauri 53/0 + Step2/3/4 SKIP + 三副本 sha256 一致 + 新实例 PID 23056 零 parse error | — | tester-1 | ✅ **已验收（运行时实证已由 Gavin 补证闭环）** |
| BUILD-RELEASE-SCENE-COVERAGE-001 | 出包：**只重建主程序**（`cargo build --release`），把 165 条新词表经 `include_str!` 嵌入内置默认；**跳过 Tauri UI 构建**（`src-tauri/**` 与 `ui/**` 零改动，重建只产出功能等价二进制）；关键验证=新 exe 内检索 `NanoSearch.exe` / `Koodo Reader.exe` 两个探针字符串确认缓存未复用旧产物；版本号维持 0.7.2 不动 | — | tester-1 | ✅ **已验收（含主控一处事实修正）** |

**BUILD-RELEASE 验收记录（2026-07-28 主控独立取证）**：

- **产物 sha256 两副本一致**：`e35679bd…`（target/release + Publish）；crash-reporter `8bfabfb5…` 亦一致——tester-1 报告写「继承，未查」，实际它 18:41 已随 `cargo build --release` 一并重建并同步，主控补查确认无误
- **词表嵌入验证主控独立换探针**：不复用 tester-1 那 5 个，改用 6 个字符串复查，含**最易触发 toml 解析问题的 `The Bat!.exe`（含 `!`）与 `Koodo Reader.exe`（含空格）**、以及本批次归类决策项 `Figma.exe` 与 `Teambition`，全部命中 → 缓存确未复用旧产物，165 条已 `include_str!` 嵌入
- **ProductVersion 0.7.2.0 未变**；`scene-rules.toml` 三副本 sha256 `7b01b33c…` 一致
- **边界合规**：`git status` 仅 `CHANGELOG.md` 一处改动，**源文件零改动**、未碰 Tauri UI / npm build / 版本号；项目级 result.md 1251 字节非空且对应本任务
- **❌ 主控修正 · 「已知缺口」不成立**：报告称「`Publish/voice-ime-ui.exe` 沿用 07-24 旧版 10,013,184 B」。**Publish/ 根本没有该文件**——已于 07-28 按 Gavin 指令删除。Publish 内是正确的 `feiyin-ime-ui.exe`（10,027,008 B，07-27 20:31，sha256 `0d76eca1…`，即 v0.7.2 那版）。那个 07-24 文件只在 `target/release/`（本次已随磁盘清理删除），且生产代码 `src/main.rs:436~478` 七处全走 `feiyin-ime-ui.exe`。**本次出包不存在 UI 缺口**
- **⚠️ 观察（不阻塞）**：冒烟实例 PID 18928 在主控 21:40 复核时已不存在，符合 [SMOKE-VANISH-001] 既有模式，产物结论不依赖该进程

**git 收口（2026-07-28）**：提交 `695e50e`（3 文件 +195/−7：`scene-rules.toml` / `src/scene/mod.rs` / `CHANGELOG.md`），已按 Gavin 指令 push → `fb230f9..695e50e`，push 后恢复 clean remote URL，`.git/config` token 残留数 0。

**⚠️ 已知副作用（出包后确认，待 Gavin 定夺）**：本次维持 **0.7.2 不升版**（Gavin 未指示升版，遵守「版本号禁止擅改」）。故现存在**两个内容不同、但 ProductVersion 均为 0.7.2.0 的主程序构建**：07-27 20:31 那版内置 144 条词表（sha256 `7fbb1e4b…`，已被覆盖）/ 07-28 18:42 本版内置 165 条（sha256 `e35679bd…`，当前 Publish 中的）。可追溯性下降，仅靠 sha256 区分。若更看重可追溯性可升 0.7.3 重出包——但注意**本次磁盘清理已删除 release 中间产物，重出包需全量重编（含 CTranslate2 C++，预计 20+ 分钟）**，不再是 2 分钟。

**TEST-EXEC 验收记录（2026-07-28 主控独立取证）**：

- **数字链自洽**：686 = TEST-EXEC-20260727 基线 672 + 本轮 TEST-SYNC 新增 14，三方来源相加严丝合缝
- **主控定向抽跑 `builtin_rules_parse_ok` 单条 → ok**（未只信汇报表格）。这条是本批次最大风险的唯一守门人，**至此真实 165 条 toml 可被 `toml` crate 正确解析已获权威验证**。注：`BUILTIN_RULES` 是根目录 `scene-rules.toml` 的 `include_str!`，与运行时加载的 `target/release/scene-rules.toml` **sha256 逐字节相同**（`7b01b33c…`），解析保证可传递
- **三副本 sha256 主控亲自复核一致**：`7b01b33ca90b6d78…`（根 / target/release / Publish 三处），同步时间 14:03:26~27
- **实例复核属实**：PID 23056、`Responding=True`、路径 `target/release/feiyin-ime.exe`、启动 14:03:59（紧接三副本同步之后）
- **✅ 遗留项已由 Gavin 补证闭环（2026-07-28 16:38，决定性证据）**：Gavin `-debug` 重启（PID **5968**，启动 16:37:56 = `08:37:56Z`）并实际录音后，日志给出完整证据链：
  1. `08:38:04.415Z INFO feiyin_ime::scene] Scene rules loaded from "...\target\release\scene-rules.toml"` —— 主控查证 `src/scene/mod.rs:243` 该 `log::info!` **只在 `toml::from_str` 的 `Ok(r)` 分支打印**（`Err` 分支走 `:246` 的 `log::warn!` 并回落内置默认）。**故这一行本身即是「外置新 toml 运行时解析成功」的直接证明**，而该文件 sha256 `7b01b33c…` 正是同步后的 165 条版本
  2. 同一毫秒 `08:38:04.415Z Scene context: app_exe="WindowsTerminal.exe", kind=IDE/terminal, multiline_safe=false, f4_injected=true` —— 分类与 F4 注入全链路正常（惰性初始化的印证：首次 classify 与 rules 加载同刻发生）
  3. `08:38:25.629Z` 第二条 Scene context 同样正常；全日志 `Scene rules parse error` / `Scene builtin rules parse error` 命中数 **0**
  4. 时序自洽：`08:37:36Z` 那条 load 属被重启掉的上一实例（早于 PID 5968 的 `08:37:56Z`），`08:38:04Z` 属当前实例 —— **每进程恰好一次加载**，与 `OnceLock` 语义吻合
  - **结论：144→165 条新词表已在运行时真实生效，本批次运行时验证闭环，无任何遗留。**

- **（历史记录，问题已解决）**验收当时的证据空洞：tester-1 那轮的新实例自 `06:03:59Z` 启动后**从未产生任何 `Scene context:` 行**。主控查证根因：`classify_scene` 全仓唯一调用点在 `main.rs:2962` 的录音流程内，`RULES` 为 `OnceLock` 惰性初始化 → **无录音 = toml 从未被读取**。因此「新实例零 parse error」是**空洞的消极证据**——未尝试解析，何来错误。日志中最后一条 `Scene rules loaded from ...target\release\scene-rules.toml` 停在 `04:47:47Z`（旧实例 PID 18548），新实例无此行。**决定性证据只需 Gavin 触发一次听写**：正常应打出 `app_exe=..., kind=..., f4_injected=true`；若 toml 解析失败会退化为 `kind=unknown, f4_injected=false`（空规则集），一次即可分辨
- **tester-1 诚信记录（正面）**：其 result.md §72 **主动如实披露**了 `Scene context:` 未取得，并自行正确诊断出「lazy OnceLock 且需录音触发」的机制原因，未虚报。这是 [TESTER-FABRICATED-REPORT-001] 四次同模式事故后首次在同类情形下如实交底，予以记录
- **边界合规**：`git status` 无新增源文件改动（仅 IMPL/TEST-SYNC 既有改动）；未出包、未改版本号、未重建 exe；result.md 4566 字节非空
- ~~**已知可接受状态**：exe 内置默认词表仍是旧 144 条、外置 toml 为新 165 条，外置若被删除会静默退回 144 条~~ → **✅ 已于 2026-07-28 18:42 的 BUILD-RELEASE-SCENE-COVERAGE-001 消除**：内置与外置现均为 165 条，外置文件缺失也不再退化

**TEST-SYNC 验收记录（2026-07-28 主控）**：

- **边界完好**：`git diff --numstat src/scene/mod.rs` = **+154 / −0**（纯追加零删除），唯一 hunk 在 `@@ +858,154 @@`，深在 `#[cfg(test)] mod tests`（起于 `:319`）内部；**`scene-rules.toml` 的 numstat 仍是 37/7 与 IMPL 交付时逐字一致，coder-1 的成果未被覆盖**
- **断言方向逐条核对正确**（写反了测试就变成锁死 bug）：P0-3 四条断言的是 `SceneKind::Doc` 而**非** `Browser`——这是验证主控替换 no-op 方案的设计是否成立的关键，方向对了才有意义；P0-1 确实直接用 `toml::from_str::<Rules>(BUILTIN_RULES)` 并把 `parsed.err()` 带进 panic 消息，**没有**退化成吞掉 `Err` 的 `compile_rules_from_content`
- **P0-4 的诚实处理值得记一笔**：任务书要求找一个「只在 browser 块出现」的独有关键词，tester-1 核对后发现 browser 块的 title_keywords 与 email/doc 块**完全重叠、不存在唯一词**，遂按任务书给的备选路径改用内联 fixture 构造（browser 块含独有词 `UniqueBrowserOnlyKeyword`），并在注释里写明为何改用 fixture。**没有硬凑一个假的唯一词来交差**
- **实际交付 14 条 `#[test]`**（汇报写「P0×5+P1×1=6 条」是按**类别**计数，函数数为 14），全文件 scene 单测 46 → **60**
- **主控独立 `cargo check --tests` 0 errors**（81 warnings 全为既有），未采信 tester-1 自验结论
- **一处遗留观察（不阻塞）**：P0-3/P0-5/P1 用的是既有 helper `classify_builtin`（`:329`），它走全局 `rules()`，而 `rules()` 会**优先读 exe 同级的外置 toml**。cargo test 下测试二进制在 `target/debug/deps/`，该目录无 toml 故回落内置默认，结论正确；但若该目录未来出现 toml 副本，这批测试会静默测错文件。属既有 helper 的固有性质（既有 SCENE-AI-AGENT-005~008 同样使用），非本次引入，记录备查。**P0-1 不受影响**（直接解析 `BUILTIN_RULES`，环境无关）

**IMPL 验收记录（2026-07-28 主控独立取证）**：

- **逐条断言核查全过**：`OneNote.exe` 未误加(0) / `ONENOTE.EXE` 仍在(1) / `Figma.exe` 在 browser 块(`:297`) / `ChatGLM.exe`+`GLM.exe` 并存(2) / **browser 块 title_keywords 零改动**（确认没走回 no-op 老路）/ 6 条存疑项全部保留(6) / 新增条目无一条裸写置信度（唯一无标注行是 Skype 注释修改，属 D 项非新增）/ UTF-8 无 BOM（首三字节 `23 20 73`）/ Rust 源文件零改动 / 未碰三副本
- **注释质量超出要求**：`NewMailEngine.exe` 的「火狐邮件」事实错误已改正并写明理由；`wezterm.exe` / `git-bash.exe` 注释如实标注「实际前台窗口通常是已在表内的 `mintty.exe`/`wezterm-gui.exe`，此条为补漏」——没有虚报价值
- **❌ 主控修正一 · coder-1 的基线质疑不成立**：他上报「原文件实际 141 条、任务书 144 有 3 条误差、终值 162」。主控用两种独立方法复核——① `git show HEAD:scene-rules.toml` 取改动前原始文件计数 = **144**；② 严格「首个非空白字符为双引号」行计数，原始 144 / 当前 **165**。git diff 数字自洽：+29 引号行 = 21 新增 + 7 注释重写 + 1 关键词行，−7 = 被重写的 7 行。**141 与 162 两个数字均错，新增数 21 本身无误，正确表述是 144→165。**
- **❌ 主控修正二 · 「cargo test 48 passed ⇒ TOML 语法通过」推理不成立**：现有 scene 单测**全部**用 `compile_rules_from_content` 内联 fixture，无一条读 `BUILTIN_RULES`。测试通过只能证明文件是合法 UTF-8（`include_str!` 编译期校验编码），证明不了 toml 可解析——而解析失败正是本批次最大风险。这与上一轮研究任务的 ✅官方 标注失真属**同一类错误：把不构成证明的东西当成证明**
- **主控过渡验证**（WSL Python 3.10 无 tomllib/toml/tomli，未做真解析）：结构化校验全通过——数组内每行均符合「条目 + 可选尾注释」或纯注释模式（零可疑行）、全文件双引号成对、方括号 11:11 配对、`[[scene]]` 块数仍为 8。**权威解析验证交 TEST-SYNC 的 P0-1 单测 + TEST-EXEC 的运行时 debug.log 核查**

**主控对 title_keywords 的替代设计（派发时补入，非 coder-1 原方案）**：原 10 条作废后，改为只在 **doc 块**加 `Jira / TAPD / 禅道 / Teambition` 4 条——doc 的 kind 非 Browser，浏览器细分循环（`scene/mod.rs:161-177`）会查到它，浏览器开 Jira → 重分类 doc → `multiline_safe=true`，工单描述可得多行结构化输出，与表内既有「腾讯文档 / Google Docs」同一条路径。**排除 `Linear` 关键词**（英文常用词，"linear regression" 会误命中；`Linear.exe` 走 exe 精确匹配即可）、排除微博/小红书/知乎（browser 默认已够）、排除 Salesforce/Zendesk（低频）。

**⚠️ 本批次最大风险（已写入任务书）**：`scene/mod.rs:258-267` 运行时 toml 解析失败只打 `log::warn!` 后**降级空规则集 → 所有场景全变 Unknown**，无用户可见报错。`The Bat!.exe`（含 `!`）与 `Koodo Reader.exe`（含空格）是最易出错的两条。现有 46 条 scene 单测用内联 fixture，**覆盖不到真实 toml** —— 故 TEST-SYNC 必须补一条解析 `BUILTIN_RULES` 的单测把这个洞堵上。

**主控增补建议（省一轮研究）**：SCENE-OBS-001 刚落地的 `Scene context:` 运行时日志会打印 `app_exe`——Gavin 日常使用中打开豆包/Kimi 等桌面版时，**真实进程名会自动出现在 debug.log 里**。与其再派一轮网络查证，不如先落地带 ⚠️ 标注的条目，用实际使用日志做零成本实证收口。


---

## 等 Gavin 拍板

| 事项 | 说明 |
| --- | --- |
| 翻译方向是否改双向全自动 | 现 v1 语义：翻译热键方向由隐藏字段 `translation.target_language` 决定（默认 Chinese），反方向语音自动跳过不翻译；`contains_han` 只做同语种跳过门控。若期望双向全自动需另立项——LLM 路径易改，NLLB 离线路径需按方向换模型（LANG-AUTO-001-CORE 验收时提出，2026-07-14） |
| FORMAT 保底层（A 方案）是否追加 | 不开 LLM 的用户是否需要规则层语气词去除保底（仅"嗯/啊/额"必删项，不碰口头禅，可复用 itn-rules.toml 外置模式）；现方案立场：不开 LLM 不做语义格式化 |
| DEC-028 Qwen3 收尾一项 | `qwen3_asr_url` 默认值维持 dashscope 还是留空强制用户配置（Gavin 实际用工作空间 endpoint，默认值对 MaaS key 用户无效） |
| TELEGRAM-RESTART-001 修复路线 | 见「未排期任务」表 |
| accuracy 体感根因收口（已降级，非阻塞） | 三选一：F1 DEBUG-AUDIO-DUMP-001（`--debug` 存喂模型前音频，小改动）｜F2 Gavin 录 5-10 条日常短指令语料 ｜ 提供当时 accuracy 出错的具体实例（定向复现最快）。**2026-07-25 起进一步降优先级**：accuracy 模型已从 UI 隐藏（ASR-HIDE-ACCURACY-001），用户侧不可达（研究报告 RESEARCH-ASR-ACCURACY-002/003 见 collab/research/） |

---

## 已知遗留问题（待修复）

| 编号 | 问题 | 文件 | 优先级 |
| --- | --- | --- | --- |
| ASR-HALLUC-SEGMENT-001 | accuracy 长音频 VAD 分段中段语义级幻觉（40.1s 语音切 3 段，中段被无关内容整段替换），现有三重兜底（字/秒、重复、空输出）全部拦不住；候选方案：每段用 CTC 交叉转录比对字符重合度 | `src/transcription/mod.rs` | **挂起**（2026-07-25 Gavin 拍板暂不排期：accuracy 已从 UI 隐藏，触发路径不可达。**若未来重开 accuracy 或引入其他 LLM-decoder 类 ASR 模型，必须同批立项**，详见 troubleshooting [ASR-HALLUC-SEGMENT-001]） |
| TEST-FIX-002 | App.test.tsx 缺少 `@tauri-apps/api/core` mock，2 个用例 FAIL | `ui/src/App.test.tsx` | 低 |
| TEST-FIX-003 | Wordbook.test.tsx `getByRole("dialog")` 无 role 属性时报错 | `ui/src/pages/Wordbook.test.tsx` | 低 |
| TECH-DEBT-001 | parse_version 实现不一致：主程序先 split('-') 再 split('.')，Tauri 侧先 split('.') 再 split('-')，prerelease 处理结果有差异 | `src/version_check/mod.rs` + `src-tauri/src/version_check.rs` | 低 |
| ACC-DEGRADE-UI-001 | accuracy 静默降级 performance 时 UI 仍显示 accuracy（可观测性缺口，Gavin 环境不触发；accuracy 已隐藏后影响进一步收窄） | `src/transcription/mod.rs:532-547` | 低 |
| MOJIBAKE-COMMENT-001 | `main.rs` 既有 mojibake 注释（历史编码损伤，非功能影响），归入待清理小项（LANG-AUTO-001-CORE 验收时发现，2026-07-14）。**2026-07-31 macOS 侧主控全文件扫描更正计数：实际 5 处** —— `2483 / 2673 / 2689 / 2722 / 2962`（原记「~L2996 一处」偏少）。已核实为既有（基线 `a38a315` 即存在），且 `0adb819` 批次已顺手清掉另一处同源乱码。清理时须以 UTF-8 无 BOM 读写 | `src/main.rs` | 低 |

---

## 未排期任务

### PLATFORM-001 · macOS 跨平台（Phase 4-6）

| 编号 | 任务 | 前提条件 |
| --- | --- | --- |
| MAC-008 | macOS 构建环境 + CI 配置 | Phase 3 ✅ |
| MAC-009 | 代码签名 + Notarization | Apple Developer |
| MAC-010~013 | E2E/热键/注入/Overlay | MAC-008 |

### 其他待排期

| 编号 | 任务 | 备注 |
| --- | --- | --- |
| PROMPT-CLEANUP-001 | 默认 system_prompt（src/i18n.rs default_system_prompt_en）Rule 4/5（Markdown/List Formatting）与 F1~F4 指令体系职责重叠清理：格式化策略应统一归 F 段管辖，默认 prompt 只留纠错/语义职责。注意 system_prompt 持久化在用户 config，改默认仅对新建配置生效，存量靠 FMT-LLM-002 的 F3 Override 兜底 | 待 Gavin 排期（FMT-LLM-002 审计发现，2026-07-13） |
| WORDBOOK-CORRECTION-UI-001 | 注入后 overlay 纠错入口（Gavin 2026-07-06 选定方案 2）：识别注入完成后 overlay 短暂显示「纠错」小按钮，点击弹出编辑框改文本，确认后调 wordbook.learn_correction 入库（喂 hotwords）。背景：WM_GETTEXT 读回在现代应用无效（RESEARCH-TEXTCAPTURE-001），自动学习路径 A 实际失效，需自建纠错闭环。涉及：src/main.rs（overlay 生命周期）+ Win32 overlay 绘制 + wordbook API（已有）。注意：DEC-029 词库已单词化，此任务未来改为学单词 | 待 Gavin 排期 |
| UI-I18N-COMPLETE-001 | UI 硬编码文字补全 i18n（coder-2 2026-07-07 建议）：App.tsx Loading/Error、Llm.tsx Success/Failed 等 5 处状态文字补 key + 替换；热键显示名（VK_TO_LABEL 的 Ctrl/F9）属通用键名评估后大概率不译 | 待 Gavin 排期（归入"小优化"批次） |
| LLM-KEY-REVEAL-001 | 格式化输出页 API Key 明文查看（显示/隐藏小按钮）：现为 password 掩码，Gavin 端测反馈框太小看不清已由 FORMAT-UI-POLISH-001 解决，明文查看为当时保留的备选追加项 | 待 Gavin 排期（归入"小优化"批次，2026-07-14 提出） |
| TELEGRAM-RESTART-001 | Telegram 通道恢复（troubleshooting [TELEGRAM-CHANNEL-001]）：2026-07-07 实测重启无效，根因为 Claude Code 2.1.202 服务端功能开关（tengu_harbor）拦截，本地无解；候选路线：降级 CLI / 等官方放开 / 临时手动轮询 | 待 Gavin 拍板路线 |
| MAC-P2-001 | macOS 热键 dispatch 延迟优化（P2） | Windows P2 完成后评估 |
| RESEARCH-TEXTCAPTURE-001 | 现代应用文本捕获方案研究 | WM_GETTEXT 在现代应用无效 |
| DEC-014 | WebView2 自动安装 | Win10 用户 |
| CRASH-EMAIL-001 | crash reporter 邮箱设置 | 需 SMTP 配置 |
| QWEN3-CORPUS-BIAS-001 | qwen3 在线模型接入词库偏置：官方 input_audio_transcription.corpus.text（max 10K tokens）可作 hotwords 等价通道；注意两份官方文档记载矛盾，实施前需实测验证。注意：DEC-029 词库已单词化，corpus 直接用单词列表 | 待 Gavin 排期（研究报告 collab/research/qwen3-protocol-alignment-001.md）|
| QWEN3-STREAM-V2 | qwen3 真流式边录边上屏（DEC-028 v1 为整段上传，流式留作演进） | 待 Gavin 排期 |

---

## Phase 3 · 演进评估（不排期，仅记录）

场景感知后续演进方向：UIA 控件信号、浏览器细分词表迭代、内容压缩独立开关、语气适配 LLM 兜底开关（未命中时参考应用名，默认关）、个性化风格学习。

来源：DESIGN-FORMAT-SCENE-001 技术方案（collab/research/typeless-format-design-001.md）+ DEC-031。


---

## 📋 待排期 · I18N-HANT-GAP-001 繁体中文缺 `voice_asr_model_accuracy` 一个 key（2026-08-17 主控验收 HOTKEY-047 时机器发现）

**发现方式**：验收 HOTKEY-047 时主控用 `comm` 比对三份 locale 的 key 集合（不是采信 Worker 自证）。

| 文件 | key 数（`^  [a-z_0-9]*:` 计） |
| --- | --- |
| `ui/src/i18n/en.ts` | 111 |
| `ui/src/i18n/zh-Hans.ts` | 111 |
| `ui/src/i18n/zh-Hant.ts` | **110** ← 缺 `voice_asr_model_accuracy` |

**已确认是历史遗留**：`git show HEAD:ui/src/i18n/zh-Hant.ts` 计得 110，`en.ts` 计得 111
→ **不是 HOTKEY-047 引入**，coder-1 无责。

**影响**：繁体中文用户在语音设置页看到 ASR 模型「准确度」相关文案时会拿到 `undefined`
或回退（取决于 `getTranslations` 的兜底策略，**未核，实施前需先确认**）。

**修法**：给 `zh-Hant.ts` 补该 key 的繁体译文。改动极小，但**必须顺带核一件事** ——
`ui/src/i18n/index.ts` 的 `getTranslations` 对缺失 key 是否有兜底；
若无兜底则属于「繁中用户看到 undefined」的用户可见缺陷，优先级要上调。

**顺带的方法教训**（已记 `[WORKER-WRITE-SILENT-FAIL-001]` 同族）：
coder-1 自证里报「三份 locale = 111:111:111 一致」，等式不成立 ——
**报数字前要真的量一次**。主控验收一律独立复算，不采信自证里的数字。

---

## 🛑 2026-08-17 停派指令（Gavin）—— 恢复时从这里开始

> **Gavin 原话**：「你的额度快尽了，先别派发任务，等我指令再派」

| 项 | 状态 |
| --- | --- |
| HEAD | `2e70875`，**已 push**（`56bfa37..2e70875`，10 个提交），token 无残留 |
| 工作区 | clean（仅 `collab/todo.md.bak-20260817` 未跟踪备份，可留） |
| BUILD-019 产物 | `Publish/` 08-17 16:18，三 exe sha 相对 BUILD-018 全变，⏭ **待 Gavin 端测** |
| coder-1 / coder-2 / tester-1 | **全部空闲待命，手上零任务** |
| 🔴 恢复后第一个动作 | 派 **HOTKEY-049**（Gavin 已定顺序「先 049」），方案与判据已固化在上方专节，**可直接派，无需再问** |
| 其后 | 阶段三 TEST-SYNC-049 → 阶段四 → BUILD-020 → push（push 授权已给，出包后执行） |
| 再其后 | `E2E-HARNESS-050`（修两道 harness 错位，裁决已定：改 harness 不改生产、尺寸与 `main.rs` 常量同源） |

### 恢复时不必重新调研的（已定论，直接用）

- HOTKEY-049 判据、键集合推导、7 条实例对照表 → 见上方 049 专节
- 049 的 UI 红线：**不得复用带「仍然使用」的现有冲突弹窗**，须另做只有「重新设置」出口的提示
- 049 双向生效：设翻译键查语音键、设语音键查翻译键，两侧都要做
- 049 范围外：后端不做强制（手改 config.toml 不受保护），已如实记录
- E2E 门禁结论：产品正常，两道 harness 错位自初始提交即存在 → `[E2E-CONFIG-PATH-STALE-001]`

### Gavin 端测五项（BUILD-019）

1. 按热键录音窗口出不出来（本包核心）
2. 点一次「设置热键」就能录上（不用点两次）
3. 左 Alt / 左 Ctrl / 左 Shift 可单独设，且按下即回显键名
4. 按住右 Alt 再按 M → 应得 `Alt+M` 而非 `Ctrl+Alt+M`
5. 🔴 **只有端测能验**：真按左 Ctrl+左 Alt+字母 → 应得 `Ctrl+Alt+字母`，不得被吞成 `Alt+字母`
   （happy-dom 把 `AltGraph` 与 `alt` 同映射，单测证明不了，见
   `[HAPPYDOM-ALTGR-INDISTINGUISHABLE-001]`）

---

## 🟡 2026-08-18 上午 · OVERLAY-051-G-FIN 已交付待验收 —— coder-1 完成

| 项 | 值 |
| --- | --- |
| HEAD | `12f0915`；工作区**非 clean**（051-G 在途，`main.rs` +270 / `qwen_inference.rs` +45） |
| Worker | coder-1 **已交付**（051-G 收尾完成，待主控验收）／ coder-2 **待命**（054-B/C/D/E 全在 `main.rs`，文件冲突，本轮不派）／ tester-1 **待命**（等阶段三 TEST-SYNC） |
| 主控实测 | `cargo check --all-targets` ✅ 0 error（coder-1 修完编译 + 方向纠正完成） |
| 方向纠偏 | ✅ 已纠正：删 `compute_tween_advance` + 5 常量 + 9 测试，新增 `reveal_chars_by_timeline`（时间戳驱动，不压缩停顿，words 空退回立即显示） |
| 决策补记 | `decisions.md` **DEC-054**（时间戳驱动回放 + 三个坑处理口径） |
| 未 push | 本地 ahead **3**（`0049c33` / `8fd9767` / `12f0915`），Gavin 未授权，不得自动 push |

**下一棒**：coder-1 交付 → 主控验收（Read + `cargo check`）→ 阶段三 TEST-SYNC → 阶段四回归 →
E2E 门禁 → 出诊断包 → **Gavin 跑一次 `-debug` 看 `words=N`**（回放参数校准的唯一数据源）。

---

### 进展更新（当日滚动）

| 时间 | 事件 |
| --- | --- |
| 11:14 | `OVERLAY-051-G-FIN` 派发 coder-1（含方向纠偏：删固定速率打字机，改时间戳驱动） |
| ~12:0x | **Gavin 端测再报**：录音窗口先闪屏幕左上角再跳回底部。主控 Read 取证 → `OVERLAY-054-B-FIX`，根因见 troubleshooting `[OVERLAY-POS-HARDCODED-ZERO-001]`，**追加进 coder-1 同批**（同文件串行，零冲突） |
| ~12:1x | coder-1 交付 051-G：删 `compute_tween_advance`+5 常量+9 测试，新增 `reveal_chars_by_timeline`，words 累积下沉 `StreamingAsrState`，+264/-58 两文件 |
| 同上 | 主控 Read 验收查出**尾部字符不显示**缺口（词表覆盖不到 `total_chars` 时无路径推进，端测表现＝最后一两个字迟迟不上屏），已反馈并入本批：循环无 break ⇒ `return total_chars` |

**OVERLAY-054-B 根因一句话**：`show_overlay_streaming_idle`（`main.rs:3215`）绕过 `overlay_geometry`
硬写 `pos:[0,0]`，而 OVERLAY-046 恢复了无条件 `SetWindowPos` → 硬编码从"无害"变"真生效"。
治本修法：`OverlayRequest.pos` 改 `Option<[i32;2]>`，Show 端 `None` 兜底算几何，
验收判据加**全文件 `grep "pos: [0, 0]"` 零命中**（上一轮漏修一半就是因为没这条判据）。
附带查出 `:3693` FocusLost 提示框同样错位（Gavin 还没撞上），同批修。

---

## 🛑 2026-08-18 01:4x 收工交接 —— 明天从这里开始

### 状态快照

| 项 | 值 |
| --- | --- |
| HEAD | `8fd9767`（E2E-HARNESS-050） |
| 已 push | 到 `2e70875`；之后 **未 push 2 个**（`0049c33` / `8fd9767`）—— Gavin 未授权，勿自动 push |
| 🔴 工作区**非 clean** | coder-1 的 **051-G 在途改动未提交**：`src/main.rs` +270 / `src/transcription/qwen_inference.rs` +45 |
| 在途备份 | `scratchpad/051G-wip-0818-0141.patch`（22KB，已存，丢不了） |
| coder-1 / coder-2 | ⏸ **token 额度耗尽，Gavin 已令暂停派发**，等重置 |
| tester-1 | ✅ 空闲，可用（另一额度池） |
| 产物 | `Publish/` 仍是 **BUILD-020**（08-17 19:37），**不含**今天 054-A/055/053-C/D/E2E 的任何改动 |

### 🔴 明天第一件事：确认 coder-1 的 051-G 在途代码状态

它被额度打断在半途。**先 `cargo check --all-targets` 确认能否编译**
（今天两条 `test_platform_cargo_test_*` E2E 用例失败就是因为它编译不过）。
- 能编译 → 让 coder-1 继续收尾
- 不能编译 → 让 coder-1 先修复编译再继续，**不要 revert**（备份在，但优先让它自己收尾）

---

## 卡顿问题：调查已完结，方案已定，等一次验证

### 已排除（全部有实测/文档依据，**不要重查**）

| 假设 | 结论 |
| --- | --- |
| 建连慢 | ❌ DNS 10ms + TCP 0ms + TLS 91ms = **103ms** |
| 首字慢 | ❌ 开口→首字 **338ms**，「反应快」已达标 |
| 我们攒着音频不发 | ❌ 实时逐帧发，100ms/帧，**与官方推荐完全一致**，无节流 |
| debug 日志 IO 拖累 | ❌ 正式模式同样卡；且正式模式 `LevelFilter::Warn` 无文件目标，零磁盘 IO |
| 本地 VAD 门控导致 | ❌ 仅入口门控，建连后**不再过滤**（主循环 VAD 调用数 = 0） |
| API 有参数可调推送频率 | ❌ **官方文档确认无此参数**（全部 9 个参数均不控制此项） |
| Manual 模式高频 commit | ❌ 它控的是**断句**不是中间结果频率；每 300ms 断句会**丢上下文、毁准确率** |
| `words[]` 带来更高频推送 | 🟡 大概率不成立（words 是同一条消息内对同一 text 的分解），**待一次 -debug 实证** |

### 确认的根因

**服务端每约 1000ms 推一批、每批 3-4 字**（实测：786/1036/1105/1246ms 间隔）。
这是它的固有行为，客户端调不动。

### 已定方案（Gavin 亲自拍板，主控曾三次判断错误后被纠正）

**时间戳驱动回放** —— 用 `words[].begin_time` 按用户**真实语速**回放。

🔴 **Gavin 的三条明确指示（不得再动摇）**：
1. 「**时间戳驱动才是应该选方案**」→ 不用固定速率抖动缓冲
2. 「**停顿和用户讲话的节奏一致才对**」→ **不得压缩停顿**，删掉主控原提的间隔上限
3. 几百毫秒固定延迟**可以接受**

`words` 不可用时 → **退回现在的立即显示**，不要换另一套平滑算法。

### 🔴 实施必须注意的三个坑（主控今天查出，写进任务书了）

1. **时间戳基准**：我们是攒 1.5s pre-roll 才一次性补发的，
   `begin_time` 相对「发出去的音频流起点」，**不是**「按下热键的时刻」。搞混会整体偏移 1.5 秒
2. **断句会重置**：服务端 `max_sentence_silence = 800ms`，
   **停顿超 0.8 秒即断句、开新 `sentence_id`** → 回放游标必须跟着重置
3. 官方文档新增可用信息：`words[]` 还有 **`end_time`**（每词该显示多久）与
   **`punctuation`**（标点归属），都该用上

### 待验证（一次 -debug 即可，不需额外开发）

coder-1 已把 `words={}` 加进日志。**新包出来后 Gavin 跑一次 `-debug` 说两三句话**：
- 空 text 消息的 `words=N` 是 0 还是有值
- 每批实际几个词、时间戳跨度多大 → **这是回放偏移量取值的依据**

---

## 今天完成并已提交

| 提交 | 内容 |
| --- | --- |
| `0049c33` | OVERLAY-054-A 焦点恢复（RestoreAndHide + SetForegroundWindow + 200ms 轮询确认）／ ASR-055 测试连接重写（原传空串 → Inference 协议，model 入 payload）／ WORDBOOK-053-C 候选词七条校验 ／ 053-D 污染排查 |
| `8fd9767` | **E2E-HARNESS-050**：9 FAIL → **2 FAIL / 61 PASS**；配置路径单一来源；state_detector **正则解析 main.rs 常量**（零硬编码，今后改尺寸免疫）；新发现 `[E2E-COLD-START-RACE-001]` |

### 🔴 053-D 结果待 Gavin 授权（主控与 Worker 均未删任何数据）

`db_path()` = exe 同级 `wordbook.sqlite`
- `wordbook` 表 18 条 **全部正常**（污染未进正式词库）
- `candidates` 表 164 条中 **12 条脏数据**（多行编号列表，24–65 字符）

**清不清由 Gavin 定。**

---

## 待办队列（按序）

| 序 | 项 | 归属 | 前置 |
| --- | --- | --- | --- |
| 1 | **051-G 收尾**（时间戳回放 + 三个坑） | coder-1 | 额度 |
| 2 | 出诊断包 → Gavin `-debug` 一次 → 定回放参数 | tester-1 + Gavin | 1 |
| 3 | **054-B 打回**：`:3117` `RecordingStreamingIdle` 仍传 `pos:[0,0]`，**在线路径仍闪左上角**（coder-2 上轮只改了本地模型路径） | coder-2 | 额度 |
| ~~4~~ | ~~054-C/D/E~~ ✅ 已完成并提交（`9c0b1b1`） | coder-2 | — |
| 5 | ASR-055 端测（需有效 api_key，Worker 无法真连） | Gavin | 出包 |
| 6 | `ASR-PERF-052` `multi_threshold_mode_enabled` 实验 | 待定 | 低优先 |

### 🔴 主控给自己加的新硬约束

**出包前 E2E 门禁必须通过，不过不出包。** 不再以「读代码觉得没问题」代替实际运行验证。
—— 起因：Gavin 批评「几轮拿不到主流程走通的版本」「验收潦草」，
根因是主控验收上限止于 `cargo check` + 读代码，从不真正跑程序。

### 其他挂账

- **api_key 明文存 `config.toml`**（`Publish/` 与 `target/release/` 均有），Gavin 未决定是否处理
- `main.rs:4723`、`hotkey.rs:300` 等处中文注释**乱码**（`[ENCODING-UTF8-001]` 历史遗留，范围不止一个文件）
- `src/main - 副本.rs` 99KB 未入 git 的旧副本残留，**删除需 Gavin 单独确认**

### 🔍 REPRO-073 取证新疑点（tester-1 2026-08-30 夜，待主控定性/派发）

1. **ASR-SUMMARY outcome 判定脱节**：所有成功识别 run 均标 `outcome=failed`（words_total>0 的 run 也是）——Gavin 端测 A/B 若引用该字段会被误导。查 `AsrSummary::format_summary()`（qwen_inference.rs:209-218）outcome 赋值链
2. **vad_hit_ms=-1 从未命中**（8 run）：服务端 VAD 回执疑似从未到达客户端，客户端无 end-of-speech 信号源——与 067/070 同根
3. **stop 后迟发窗口**：`OVERLAY-043: ignoring late StreamingText after stop` 单 run 6+ 次、持续 4-5s——REPRO-068 交替闪烁与 070 尾部丢失疑似同源 finalize 拖尾

---

## 🟢 2026-09-06 晚 · SECRET-126 已验收（Gavin 指令：确保配置与隐私永不入库）

**Gavin 原话**：「确保本地配置文件和代码不会提交任何 key、和其他隐私敏感信息」。

### 主控审计现状（先说结论：当前无正在泄露的东西）

| 审计项 | 实测 |
| --- | --- |
| 磁盘上真正含非空 `api_key` 的文件 | 仅 `Publish/config.toml` 与 `target/release/config.toml`，**两者已 ignore** ✅ |
| 未跟踪且未被忽略（`git add -A` 会抓的） | **0 个** ✅ |
| 326 个已跟踪文件全量内容扫描 | 命中 8 处**全为良性**：上游 vendor 作者邮箱（cmake/esaxx-rs）、`@example.com` 占位、Gutenberg 语料公开地址、Tauri 图标清单里的 `128x128 @ 2x.png` 文件名（@ 前后加空格以免自触发闸门）被邮箱正则误命中。**零真实凭证** ✅ |
| `assets/default-config.toml` | `api_key = ""` 空占位 ✅ |
| 两道钩子 | 实机验过：pre-commit 真阳性 5 拦/真阴性 3 放；pre-push 能兜住被 `SECRET_SCAN_SKIP=1` 绕过的 commit ✅ |

### 补的 latent 洞（SECRET-126，coder-1）

`.gitignore` 四类路径规则 + **新增 `scripts/git-hooks/secret-paths.sh` 路径闸门**
（pre-commit + pre-push 双侧 source 同一份，防 SECRET-082 式漂移）。
拦截判据是**路径不是内容** —— `[GATE-MATCHES-SHAPE-NOT-CONTENT-001]` 的教训：
口述转写日志、实验 dump 这类「文件类别本身危险」的东西是内容正则的永久盲区。

**主控独立复跑（未采信报告）**：路径闸门 **17/17** 全对（含 `CONFIG.TOML` 大小写绕过、
`collab/research/` 强加 `-f`、`logs/*.md` 与 `.cargo/config.toml` 未误伤）；
内容闸门 **7/7** 零回归；`ls-files` 326 未变；三钩子 LF+UTF-8+可执行。

### 遗留

| 项 | 处置 |
| --- | --- |
| coder-1 的 result.md 缺「收尾自证表」 | 已退回补写（`[DOC-STATE-DRIFT-001]` 又复现，且这次漏的正是防它的那张表） |
| MSYS git `add -f <ignored>` 偶发静默 no-op（coder-1 报） | 主控立 troubleshooting 条目 |
