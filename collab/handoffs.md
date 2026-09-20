# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行，超 200 行上限）。

## 2026-09-20 — coder-2 — TEST-SYNC-229 ✅ 交付（在线 ASR 两轴阶段三交叉护栏，待主控验收 → 阶段四实跑）

- **改动**：三文件 `mod tests` 纯追加 **+79 / −0**，生产零触碰。G1 `qwen_inference.rs:1778-1806`；G2 `src/config/mod.rs:1806-1823`；G3 `src/transcription/mod.rs:964-995`。
- **G1**：`build_run_task_message` 轴二 `true` 分支 + `false` 对照（原只断言 false 单向，挡不住「丢参数重硬编码」）。
- **G2**：轴二隐藏字段**主配置侧** save→load 往返（`TestEnv` + 置 true）；镜像侧 coder-1 已写，主配置侧是缺口。
- **G3**：`Transcriber::new` 两轴入参 → getter 透传，用非默认值（1234/true）区分「写死默认」。
- **G4**：默认 2000/false 五处核对全在场、未发现漏改；按指令未重复写默认值测试。
- **G5 撤单**：理由经主控纠正——非「与 mirror 字面值测试同义反复」（那条是 value 级，G5 是 schema 级）；真理由是两 crate 独立编译单元，自快照是「提醒」非真闸门且加字段要付维护成本（假闸门比没闸门更糟）。残余风险转 `[CONFIG-MIRROR-DRIFT-001]` 归主控。
- **观测上报**：主配置侧无轴二默认 false 的独立断言（仅镜像侧间接覆盖），按 G4 未补，交主控裁定。
- **约束/验证**：未触碰 src-tauri ⇒ 规避 FMT-COLLATERAL-001；`rustfmt --check` 三文件 clean；`cargo check --all-targets` 0 error、warnings 111/102 持平；numstat 与 numstat -w 逐行相同（无空白噪声）。🔴 未跑 `cargo test`（DEC-048），三条断言阶段四必须实跑。
- **红线**：未 commit / 版本未动 / 零凭证。

## 2026-09-20 — coder-1 — ASR-SEG-229 + VER-BUMP-230 ✅ 交付（在线 ASR 两轴参数化 + 版本 0.9.2，待主控验收 → 阶段四回归）

- **轴一（阈值）**：`asr_online_max_sentence_silence` 默认 **800 → 2000**。`src/config/mod.rs:205` 默认 fn、`src-tauri/src/config.rs:151` 镜像同步、`qwen_inference.rs:80` `DEFAULT_MAX_SENTENCE_SILENCE`（现仅测试夹具）800→2000。理由：官方默认 1300，800 过激进致服务端 VAD 碎句 + 满屏句号；端点预览走中间结果、`final_text=confirmed+current` 不丢尾字 ⇒ 提阈值零代价。
- **轴二（开关）**：删除 `qwen_inference.rs` 的 `const SEMANTIC_PUNCTUATION_ENABLED`，新增隐藏字段 `asr_online_semantic_punctuation_enabled: bool`（`#[serde(default)]`，默认 false）。主配置 `src/config/mod.rs:167-175` + 镜像 `src-tauri/src/config.rs:123-127` 双写。逐层透传：`build_run_task_message`(+1 参) → `transcribe_streaming`/`transcribe_streaming_realtime`(+1 参) → `Transcriber::new`(+1 参 + getter) → `main.rs` 两处 `Transcriber::new`（`:7121` 热重载 / `:7232` 启动）+ `:7521` realtime 调用。
- **VER-BUMP-230**：`Cargo.toml:3` / `src-tauri/Cargo.toml:3` / `src-tauri/tauri.conf.json:9` 三处 `0.9.1→0.9.2`；`ui/package.json`(0.1.0) 未动；两份 `Cargo.lock` 自动重写版本行。
- **三条红线**：① Tauri 镜像字段已同改（含 `make_minimal_cfg_with_asr_model` struct literal）② **未对 src-tauri 跑 `cargo fmt`**（其 fmt 漂移为既有，`:4` 空行，非本单引入）③ 新增测试仅镜像往返 1 条（`mirror_asr_online_semantic_punctuation_roundtrip`），另在既有 mirror 字面值用例补一条默认 false 断言。
- **既有断言机械同步（3 处，非新增覆盖）**：主 config 默认 800→2000、镜像字面值 800→2000、`build_run_task_has_max_sentence_silence_800`→`_2000`。
- **🔴 端测前置（主控已确认写入清单）**：两轴值在 `Transcriber::new` 时熔入，热重载触发键不含二者 ⇒ **A/B 必须重启进程**（与既有 silence 字段行为一致）。Gavin 的 `Publish/config.toml` 若写死 `asr_online_max_sentence_silence = 800`，改默认对其零作用，须手改该行；新字段缺失则走 default(false)。
- **验证**：`cargo check --all-targets` 0 error / test warnings **102**（基线）；`cargo check` 0 error / **111**；`cargo check --manifest-path src-tauri/Cargo.toml --all-targets` 0 error / **17**；`rustfmt --check` 四根文件 clean；定向测试 `build_run_task` 7P、`config::tests::asr_056` 14P、`transcriber_qwen_audio_online` 2P、`..._silence_2000` 1P、镜像 4P（含新断言）——**全 0F**。🔴 未跑全量 `cargo test`（阶段四 tester-1）、未 `cargo build --release`。
- **边界**：`native_punctuated` / 本地 CT-Transformer 跳过逻辑 / 标点后处理**零改动**；`src-tauri/src/qwen3.rs`（连接自检）仍硬编码 false，属独立路径不含 silence，本单不动。
- **红线**：未 commit / 未出包 / 零凭证；临时文件（HEAD 版本 rustfmt 对照）已清理。
