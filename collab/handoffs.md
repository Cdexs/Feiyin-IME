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


## 2026-09-20 — tester-1 — TEST-EXEC-229 阶段四全量回归 ✅ 执行完毕（🔴 1 类 FAIL：测试侧 harness 隔离缺陷、生产缺陷 0，只跑不改）

- **结果**：`cargo test`（root 10 target）**1264P / 2F / 15I**；`src-tauri` **77P / 0F / 0I**。**2 F 同一条** `config::tests::asr_229_config_hidden_field_semantic_punctuation_roundtrip_persists`（新增护栏 G2），在 `feiyin-ime` + `crash-reporter` 两 target 各 1F，panic `os error 3`（`src/config/mod.rs:1817`）。
- **定性（证据链）**：① 单独跑 G2 → 1P/0F；② `config::tests::` 加 `--test-threads=1` → 49P/0F；③ 默认并行 → 1F 且失败者换成**既有** `asr_056_config_hidden_field_roundtrip_persists`（谁抢输谁红）。根因 `TestEnv`（`src/config/mod.rs:657-678`）单目录 `voice-ime-test-{pid}` + Drop `remove_dir_all`，G2 与 056 均无锁并行 → 一个删目录另一个正 save。**生产缺陷 0**。
- **处置**：按令**未自行改测试/期望值**；主控另开 `FIX-TESTENV-231`（TestEnv 每实例唯一目录）派 coder-1，排本单之后（同文件串行）。
- **新增护栏**：G1 PASS、G3 PASS；G2 见上。**3 处 800→2000 机械同步全 PASS**（`asr_056_..._silence_2000` / `mirror_asr_fields_match_main_config_literals` / `build_run_task_has_max_sentence_silence_2000`）⇒ 无同步漏改。
- **消融（3 条，过滤跑）**：G1 `qwen_inference.rs:263` 硬编码 false → RED；G2 `config/mod.rs:174` `#[serde(skip)]` → RED；G3 `transcription/mod.rs:228` getter 返回 false → RED。还原后三文件 sha256 回基线、`git status` 无源码 diff、复跑各 1P/0F。
- **SKIP**：npm / test:browser（本批 `ui/` 零 diff，`git show --numstat -- ui/` 空）；E2E（按令，无 release 产物）。
- **warnings**：test profile **102**、bin **111** = 基线；src-tauri test **19**（与 coder-1 的 `check --all-targets` 17 非同一命令，不作持平判据）。
- **证据**：`collab/outbox/tester-1/testexec229/`（cargo_test_root_nff.log / cargo_test_tauri.log / ablate_g1|g2|g3.log）。
- **红线**：未改生产代码与测试期望值 / 未出包 / 版本未动 / 未 commit / 零凭证。

## 2026-09-20 — coder-1 — FIX-TESTENV-231 ✅ 交付（TestEnv 每实例唯一目录，待验收 → 阶段四复跑）

- **根因**：`src/config/mod.rs:661` `TestEnv::new()` 对所有用例返回同一 `voice-ime-test-{pid}`，`Drop` 对整个目录 `remove_dir_all` ⇒ 并发用例的目录被另一用例的 Drop 删掉。23/25 用例靠 `TEST_MUTEX` 串行掩盖；无锁用例从 **1 个**（`asr_056_config_hidden_field_roundtrip_persists`，TEST-SYNC-056 `cb0d82b` 引入，**潜伏一个多月**）增至 **2 个**（本批 `asr_229_...`，`c5cff01`）即必现。
- **改动（只一个函数）**：`TestEnv::new()` 目录名加进程内原子计数器 → `voice-ime-test-{pid}-{seq}`，每实例唯一；`Drop` 只删自己这份。**25 个用例一行未动**。
- **🔴 不补锁（主控判断，我独立复核同意）**：`TEST_MUTEX` 保护的是 `AppConfig::config_path` 全局态；那 2 例走显式 `save_to(&path)`、不碰全局态，不取锁是对的。补锁只掩盖、且把「共享目录」真根因留在原地（再加第三个无锁用例照样炸）。
- **边界核验**：实测 25 个 TestEnv 用例中无任何一例创建 >1 个 `TestEnv` ⇒ 无用例依赖共享路径；src-tauri 无同款 harness。
- **验证**：`cargo check --all-targets` 0 error、warnings **111/102**（=基线）；`rustfmt --edition 2021 --check src/config/mod.rs` clean（首版多行 `format!` 被判 DIFF，已改单行）。🔴 未跑 `cargo test`（并行竞态一次绿证明不了，归阶段四 tester-1 复跑；重点：默认并行下 `asr_229_...` / `asr_056_...` 两例需稳定绿）。
- **证据链**：`TEST-EXEC-229` 报告「单独跑绿 / `--test-threads=1` 49P0F / 并行失败者随调度漂移」= 共享资源竞态铁证；troubleshooting 条目 `[TESTENV-SHARED-DIR-RACE-001]` 由主控落档。
- **红线**：只改 `TestEnv::new()` / 25 用例零改动 / 未跑 cargo test / 未 commit / 未出包 / 版本未动 / 零凭证。


## 2026-09-20 — tester-1 — TEST-EXEC-231 复跑 + BUILD-232 v0.9.2 出包 ✅（八项核验逐项 PASS）

- **TEST-EXEC-231**：`config::tests` 默认并行**连跑 5 次全绿**（各 49P/0F/2I，证 `FIX-TESTENV-231` 有效）；root 全量 **1266P/0F/15I**（10 target，EXIT 0，与期望逐位吻合）；`src-tauri` **77P/0F**。npm/browser/E2E SKIP（`ui/` 零 diff + 按令）。消融按令不重复。
- **BUILD-232**：Step 1→4 全走（npm 1.51s / Tauri UI 1m40s / 主程序 2m20s），cp UI 入 `target/release/`、三 exe + 两 toml 入 `Publish/`。
- **八项核验**：①时间戳 14:17–14:18 ✅ ②两副本 sha 相等且三者异于上包 ✅ ③**ProductVersion 0.9.2（唯一变红再变绿）** ✅ ④冒烟 Responding=True + 启动链路完整 + 无 crash.json + panic/ERROR 0 + 残留 0 ✅ ⑤`config.toml`/`wordbook.sqlite` 零变化 ✅ ⑥warnings 111/9/17 = BUILD-228 基线 ✅ ⑦正向 `asr_online_semantic_punctuation_enabled`=1 / 反向 `SEMANTIC_PUNCTUATION_ENABLED`=0 + UI 文件名探针 ✅ ⑧toml 三副本 hash 全等 ✅。
- **端测项（交 Gavin）**：`Publish/config.toml:29` 写死 `asr_online_max_sentence_silence = 800`，新默认对其零作用 ⇒ A/B 须手改该行 + **重启**；2×2 四组合与目视重点见 result.md Part C。
- **已知非本批**：冒烟 stdout 的 `[ASR-DROP]` WARN（`src/audio/mod.rs:492-495`），主控确认下批修，不拦出包。
- **证据**：`collab/outbox/tester-1/build232/` 与 `testexec229/`。
- **红线**：未改生产代码 / 未 commit / 零凭证。
