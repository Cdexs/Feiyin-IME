# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行，超 200 行上限）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

## 2026-09-21 — coder-1 — UI-LRMODEL-HINT-330 ✅ 交付（删除本地流式档「模型文件」提示块）

- **根因**：`src-tauri/src/main.rs:144 check_local_realtime_models_ready` 未随 DEC-076 迁移，仍在找**已不存在**的 FunASR nano 目录 ⇒ 恒报「缺失 · 972MB」，而实际在跑的 Qwen3 是好的 ⇒ **界面说假话**。Gavin 裁定整块删。
- **改动（纯删除 -176，`numstat`==`-w`）**：`ui/src/pages/Voice.tsx` -103；`src-tauri/src/main.rs` -52（命令 + 结构 + 注册行，删干净不留无人调用者）；i18n 三份各 -7（删 6 key，保留 `voice_asr_model_local_realtime(_desc)`）。
- **验证**：`cargo check --all-targets` 0 error / warnings **110/101** 基线 ｜ src-tauri 0 error / 17w ｜ `npm run build` 0 error ｜ Vitest **100P/11S/0F** ｜ 三份 locale **逐键一致（各 121 键）**、被删 6 key 全仓 grep **0 命中**。
- **只报不改**：① accuracy 提示块**未过期**（已迁 Qwen3）② `src/transcription/mod.rs:1508` **不找 nano** 且**无调用者**（死代码但判据正确）。
- **已知代价**：被删块「流式预览 228MB」检测原本准，删后缺模型时界面不再提示（Gavin 已知悉）。
- 🔴 **UI 视觉待 Gavin 目视确认，未声称已验证**。红线：未动版本 / 未 commit / 未出包 / 未碰 `src/main.rs` / 零凭证。

## 2026-09-21 — coder-2 — ACC-REFLOW-PERSIST-329 ✅ 交付（回灌持久化 + 镜像基准）

- **缺陷A**：回灌被流式包冲掉 ⇒ 新增 per-gen `ACC_REFLOW_STATE` + `compose_with_acc_for_gen`；`StreamingText` 渲染分支走 `compose(raw)`（权威前缀 + 流式尾巴），不再被冲掉。
- **缺陷B**：镜像改为早写 `compose(raw)`（**不再写 raw**）并保持 043 门闩前（053-B 契约不变，契约测试仍绿）；`PreviewReflow`/`StreamingFinalPreview` 渲染同步写镜像 = 所显 ⇒ 未编辑提交 `original==edited`。
- **选 (a)**：保留早写、只换内容；未改/删任何护栏。
- **残留 gap**（停止后迟到包镜像领先所见，窄窗口+零修改+2 次才可能出候选）本单不处理，断言钉住，另立 331 编辑入口快照收口。
- **边界**：编辑态不冲输入 / 收尾用合成 / per-gen 清空 / 闩锁后保留前缀 / 非本地 raw passthrough。
- **验证**：fmt clean（skip_children）/ check 0 error / warnings 110/101 / numstat==-w（169/4）/ 329 6P + 325 6P + 053-B 1P。
- 🔴 实机交 tester-1/Gavin，未声称已验证。未动版本 / 未 commit / 未出包 / 未碰 audio 与 transcription / 零凭证。

## 2026-09-21 — coder-2 — AUTOLEARN-CANDIDATE-327 ✅ 交付（候选抽取二次收窄）

- **根因**：字符 diff 后公共后缀为空 ⇒ 候选吞到句尾（`指导灵的信息吗？`）⇒ 被句末标点校验拒、阈值够不着。
- **改法**：`extract_correction_word` 后追加 `narrow_candidate`（`wordbook/mod.rs:276`）：句末标点截断 → 虚词切前导块 → 切过且 <2 字/仍含标点 ⇒ None；未切分原样返回。
- **不动**：threshold、is_valid_candidate 一字未改；「是/在」保留（主控拍板，注释列真实碰撞项）。
- **外置**：新增根 `wordbook-rules.toml`（include_str! 内置 + exe 同级覆盖 + 降级）。
- **验证**：fmt clean（skip_children）/ check 0 error / warnings 110/101 基线 / numstat==-w（177/6）/ `autolearn327` 6P + `wordbook::` 61P 全绿。
- 🔴 实机交 Gavin（判据 `[AUTOLEARN] promoted after threshold` + 词库可见），未声称已验证；出包同步 toml 三副本。
- 未动版本 / 未 commit / 未出包 / 未碰 `src/audio/mod.rs`、`src/main.rs` / 零凭证。

## 2026-09-21 — coder-2 — AUTOLEARN-LOCALRT-326 🔴 盘点完成 / 前提被推翻 / 停手等裁定（零代码）

- **盘点**：学习镜像 `Arc<Mutex<Option<String>>>`（`main.rs:8527`；唯一写 `:6802` / 唯一读 `:7104`）vs 浮层渲染字段（`:1396`/`:2992`，不参与学习）。
- **逐档**：在线流式 `:7816` ✅；本地实时 `:8167` ✅；本地 Accuracy/Performance/批处理 不发 StreamingText + **无编辑入口**（`text_hit_rect` 仅 `:2988`）⇒ N/A。
- **🔴 反证**（`debug-build321.log`，LocalRealtime 会话）：`learn_correction` 被调用（`Auto-learn candidate rejected … "指导灵的信息吗？"`、`[AUTOLEARN] candidate observed: '指导灵' (1/2)`）⇒ 镜像非空、路可达；真因在**判定层**（候选含句末 `？` 被拒 + 阈值 2）。
- **结论**：按「不碰判定逻辑」约束本单无对象可改 ⇒ 请主控裁定（撤单 / 转判定层 / 加编辑入口）。**未改任何代码**。

## 2026-09-21 — coder-1 — PREROLL-DEAD-322 ✅ 交付（pre-roll「全零」定根因：链路无 bug + 诊断口径修正）

- **根因（实测证伪两个候选 + 主控候选 C）**：空闲期采集流**是活的**（10ms 节拍、时间戳 0~500ms）、payload **100% 非零**（`nz_ratio=1.000`/`exact_zero=0`）、重建流/积压/连续消费三对照一致、端点采集音量 0.84 正常。「纯零」= `{:.4}` + i16 落盘把 ~1e-7(≈-140 dBFS) **显示/量化没了**。候选 C 由**同一缓冲内**「前 1000ms=0 → 热键后 3510」证伪（恒定缩放不能只作用一段）⇒ 变的是声学输入；候选 B 由「持续消费不抬幅度」证伪。
- **定案**：环形缓冲装的是麦克风真实输出，**pre-roll 链路无 bug**；主控撤销「修不了就整块摘除」。
- **改动（仅 `src/audio/mod.rs`）**：A = 诊断精度：`pre_roll_diag`/`dump_log` 打 `peak`(dBFS)+`nz_ratio`（新增 `DIAG_DBFS_FLOOR`/`peak_to_dbfs`/`nonzero_ratio`/`diag_peak_nz`，Debug 守卫⇒Warn 零开销）；顺带修 292 **文件② 复用文件① offset** 的误导。保留 1 支 `#[ignore]` 探针 `pr322_idle_probe_manual` + 3 条纯函数回归。
- **验证**：fmt clean ｜ check --all-targets 0 error ｜ warnings **110/101** 基线 ｜ `cargo test` **0 failed**（+3P/+1 ignored）。证据 `collab/evidence/20260921-preroll-dead-322/`。
- **下游线索（🔴 不在本单）**：C920 空闲 ≈-120~-140 dBFS vs 说话 ≈-16 dBFS ⇒ 电平触发的降噪/AGC，其 **attack 疑吃首字爆破音**，与 Gavin「你/按」首字不准吻合；判据折进下次端测。
- **红线**：未动版本 / 未 commit / 未出包 / 未碰 `src/main.rs` 与 `src/transcription/**` / 零凭证。

## 2026-09-21 — coder-2 — ACC-PREVIEW-REFLOW-325 ✅ 交付（accuracy 分片回灌预览）

- **方向**：296/307 证伪「流式补尾字」（gained 恒 0）⇒ 用 298 accuracy 已完成片权威文本回灌浮层。
- **边界**：`committed_len = last_display.chars().count()`（已上屏含标点串，与镜像同源）；新事件 `PreviewReflow{gen,seg_index,committed_len,has_hole,acc_text}`；合成 `acc + mirror[committed_len..]`。
- **编辑态**：per-gen 闩锁（EditRequested 置位/RecordingStarted 复位）——进过一次编辑本次录音回灌全停。
- **主控三订正**：① 单调键=seg_index ② 编辑闩锁（非当前标志）③ 失败片留洞→`SkippedHole`。
- **只本地档**：唯一发送点 `main.rs:8140`（LocalRealtime acc worker）；在线/批处理结构上永不发。
- **验证**：fmt clean（main 用 skip_children）/ check 0 error / warnings 110/101 基线 / numstat==-w（275/5、12/4）/ `preview_reflow_325` 6P + 298 3P + local_stream 10P。
- 中间态曾致整仓短时不编译，已按要求补齐后立即 check 恢复并通知；主动上报 rustfmt 递归子模块风险（主控核 audio 无夹带）。
- 🔴 实机未声称已验证，交 tester-1/Gavin。未动版本 / 未 commit / 未出包 / 未碰 audio 与 transcription/mod.rs / 零凭证。

## 2026-09-21 — coder-2 — ITN-SHIFEN-323 ✅ 交付（「十分」程度副词误转修复）

- **根因**：`十分` 未在保护表 ⇒ `十`→10、`分` 保留 =「10分」；百般/万般 同族。
- **改法**：`[protect.degree_adverbs]`（十分/万分/百般/万般）+ `check_protection:2806` 右邻条件（词放 toml、条件写代码，DEC-038）。
- 🔴 数词读法三判据**判定串写死**：① 词尾以 ≥2 字单位起首（分钟/分贝，`is_unit_multichar` 排除裸 `分`）② 整词右邻单位（十分米）③ 整词右邻「之」（十分之一）。主控拦截：③ 初版误判在词尾，已更正。
- **取向**：句末裸「十分」保持汉字（知情权衡 + 断言锁）；「千万」不碰。
- **验证**：fmt clean、0 error、warnings **110/101** 基线、`numstat`==`-w`(87/0)、itn **259P/0F**（含 4 闸门）。
- 🔴 **交 tester-1**：出包时同步 `itn-rules.toml` 三副本（否则 exe 同级旧副本覆盖内置默认，修复失效）。
- 未动版本 / 未 commit / 未出包 / 未碰 `src/transcription/**` 与 `src/audio/mod.rs` / 零凭证。

## 2026-09-21 — coder-1 — LOCALRT-CTX-INJECT-320 追加 ✅ 交付（删回滚开关 + 全 env 删除 + 词库单引擎化）

- **原则（Gavin）**：不允许开发端/用户端行为不一致——用户机无 env，env 覆盖是假路径，且残留 env 会污染端测结论。
- **改动**（`src/transcription/mod.rs` + `src/main.rs` + `src-tauri/src/main.rs`）：删 `AccuracyEngine`/`accuracy_engine(_from)`/`VOICE_IME_ACCURACY_ENGINE`；`build_recognizer` Accuracy 直接 `create_qwen3_recognizer`；删 `create_funasr_nano_recognizer`/`ensure_funasr_nano_model`/`check_funasr_nano_model_ready`；src-tauri 就位检测单路径 Qwen3；LocalRealtime offline（2pass）切 Qwen3。词库预算单引擎：删 FunASR 支（356/1.85）与双份预算；`HOTWORDS_MAX_ENTRIES=200`/`MAX_TOTAL_TOKENS=3000`/`SAFETY_FACTOR=2.0（未标定保守）`。删三个 env（`ACC_NUM_THREADS`/`LOCAL_RT_CTX_ENABLED`/`LOCAL_RT_CTX_CHARS`）及解析函数/相关单测 → 常量：线程 `min(cores,8)` 兜底 4、注入**恒开**、上限 **500**。
- **验证**：fmt clean；`cargo check --all-targets` 0 error、warnings **110/101** = 基线；root `cargo test` **1367P/0F/16I**；src-tauri **85P/0F**；numstat `main.rs` 35/4 与 `src-tauri` 6/19 == `-w`（`mod.rs` 435/287 vs 432/284 = 3/3 空白，大块删除括号重排，主控认可）。🔴 实机效果交 tester-1/Gavin。
- **红线**：未动版本 / 未 commit / 未出包 / 未删磁盘模型文件 / 未碰流式与 performance 构造 / 零凭证。

## 2026-09-21 — coder-1 — LOCALRT-CTX-INJECT-320 ✅ 交付（上下文注入 + 长跨回显护栏）

- **实现**：accuracy worker 每片经 Qwen3 per-stream 注入 system 段（英文说明句 + `Context:` 时间线 + `Terms:` 词库，各占一行加标签）。**跨录音轮换 A**（`prev2=prev1; prev1=final`，仅成功时）+ **录音内增量 B**（`acc_text` 每片追加）+ **截断 C**（从头部砍保留尾部，CAP=500）+ **护栏 D**（ctx 定稿后 LCS，命中无上下文重解）+ 门控（`LOCAL_RT_CTX_ENABLED=0` 退回；🔴 FunASR 绝不注入防 EXIT(-1)）。
- **推翻 315**：词库改回**注入**（Gavin 判 316 B 组构造性假阴性）；**KV 2048→4096**；词条 **120→200** + 引擎分派预算（FunASR 356/1.85 回滚不变；Qwen3 **3000/2.0 未标定保守值**，不沿用 1.85）。
- **🔴 关键发现**：298 accuracy worker 是**单线程按序处理批次**（非段间并行）⇒ 因果上下文**零并行代价**。
- **验证**：fmt clean、check 0 error、numstat==-w（mod.rs 454/21、main.rs 34/3）；6 ctx320 + 18 curate 单测全绿。🔴 实机交 tester-1/Gavin。
- **上报**：① warnings 111/102（+1 非本单）；② 既有红测试 `tune317_...default_4` 系并发提交 `184633a`（默认 4→8 未同步测试；`local_stream.rs` 非本单未擅改；`:310` 另有可疑残留 `let blank_penalty = c.model_config.debug = false;`）。
- **红线**：未动版本 / 未 commit / 未出包 / 未删 FunASR / 零凭证。

## 2026-09-21 — coder-2 — HOMOPHONE-NODE-318 ✅ 交付（同音纠错可挂载节点）

- **交付**：`src/homophone/mod.rs`（新，规则外置 + 纯函数 `apply_homophone_fix`）+ 根 `homophone-rules.toml`（**129 条**）+ `main.rs` 挂载（`:9384`）+ MACOS-HANDOFF + `evidence/20260921-homophone-318/`。
- **挂载**：`run_pipeline_core` 内、`is_effective_text` 后、ITN 主通道前；恒 `true`；三处调用共用 ⇒ 四档全覆盖。
- **词表**：自查语料同音错 ≈0 ⇒ 主要来自 pycorrector 筛选池，六道过滤 →129（详见 evidence `rules-318.md`）。
- **坑**：子串越界（jingba 可嵌入性剔除）、幂等（改单趟 + `t8` 不变量）、繁体安全（剔 `锺→钟`）。
- **验证**：fmt clean、check 0 error、warnings **110/101** 基线、`numstat`==`-w`、`homophone` **8P/0F**。
- ⚠️ `src/bin/poc_local_stream.rs` 被改非本单（coder-1 在途）。未动版本 / 未 commit / 未出包 / 未碰 `src/transcription/**` / 零凭证。

## 2026-09-21 — coder-1 — MIGRATE-QWEN3-315 ✅ 交付（迁移收尾：线程 env + 词库不注入 + 硬编码排查）

- **2.1 `ACC_NUM_THREADS`**：`default_acc_num_threads()`(=min(逻辑核数,8) 兜底 4)+`acc_num_threads_from()`（非法/缺失/<1 回落）+`acc_num_threads()`；FunASR/Qwen3 两处接线 + `[MIGRATE-QWEN3-315]` 日志。**默认=现状，无 env 逐位无变**（配 317 扫流式N+accuracyM 组合）。
- **2.2 词库不注入**：`create_qwen3_recognizer` hotwords=None 注释改为决策记录（316 B 组零贡献、F 组抵消前文 ⇒ 白占 DEC-068 预算；勿误以为漏接）；未接 per-stream。
- **2.3 用户可见硬编码**：src-tauri `check_accuracy_model_ready` 写死 FunASR（独立 crate，314 未覆盖）⇒ 已按 `VOICE_IME_ACCURACY_ENGINE` 分派；UI accuracy 卡仅动态显示 `modelInfo.model_dir`、**无 name/size 硬编码**（无 locale 变更）；local_realtime offline 卡与在线 FunASR 文案有意保留（LocalRealtime offline 仍 FunASR）。
- **2.4 自检**：两引擎构造均 Ok（`migrate314_both_engine_recognizers_construct --ignored`）、就位判据两套 true；`native_punctuated` 与引擎无关；🔴 未跑实机录音。
- **验证**：root fmt clean / check 0 error / warnings 110/101；src-tauri check 0 error / warnings 17/19；numstat==-w（mod.rs 57/7、src-tauri main.rs 21/6）；1 新单测通过。未碰根 `src/main.rs`；未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-1 — TUNE-STREAM-317 ✅ 交付（流式三调优点）

- **①② env（默认不变）**：`LOCAL_STREAM_NUM_THREADS`（默认 4）+ `LOCAL_STREAM_BLANK_PENALTY`（默认 0.0，负值合法；NaN/inf/非法回落）；接线 `create_local_stream_recognizer` + `[LocalRT-DBG-317]` 打印三实际值。**无 env 逐位无变**；注释写明 blank_penalty 双向风险。
- **③ HomophoneReplacer**：`rule_fsts` **需预编译二进制 .fst**（源码 `TextNormalizer` `binary=true` / `ReadFstKaldi`，无文本分支）；`lexicon.txt` 才是纯文本。**可离线编译一次随包发**（作者侧 WSL/Colab pynini，用户零工具链）；最小形态：lexicon 用 jieba+pypinyin 生成（纯 Python，Windows 可跑，235 已验）+ 每词 1 条 cross 规则（含前后鼻音变体）+ `hr{lexicon,rule_fsts}` 接线。风险：同音误伤（P2 前科）/ 仅中文 / 词库变更须离线重产。**235 实证**：3/3 修对、0/5 越界、2/2 碰撞改写，lexicon-only=no-op。**未实现**。
- **上报**：accuracy 侧 `num_threads`（mod.rs `:1124`/`:1208`）**无 env**；建议主控协调加 `ACC_NUM_THREADS`（本单未跨文件改）。
- **验证**：仅改 `local_stream.rs`（`+81/2`）；fmt clean / check 0 error / warnings 110/101 / numstat==-w；2 单测（env 非法回落）通过。🔴 实机交 tester-1/Gavin。未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-2 — POC-QWEN3-CTX-VALUE-316 ✅ 交付（上下文价值 PoC，只变一变量）

- **方法**：Qwen3-ASR 固定，full.wav 298 切 3 片，只变 system 段：A 空/B 词库/C ctx150/D ctx300/E 不截断/F 词库+ctx/G 无关菜谱；补扫 cap 0/40/80/120/全文。`t10_ctx_value` 164.6s EXIT0。
- **结论**：① 收益微弱且不稳定——CER A 0.0444→C/D/E 0.0356、G 0.0489 更差；专名「约根·斯特兰德」仅 context 组转正，但**前文不含该词 ⇒ 非术语搬运**；F 回到 A。② 拐点未探到（素材前文 ≤~170 字，C/D/E 等价；cap≥40 即饱和，不外推）。③ 词库与前文不可互替（B=A；F 未增强）。④ G 证明 system 段被使用。
- 🔴 **失败**：cap=80 回显片0 的 79 字，CER 0.3467；`has_repeat_loop` 检不出长跨回显 ⇒ 315 须补护栏。
- **建议 315**：勿把 7.9% 代价当已证收益；若做则 cap ≤120 + 长跨护栏 + 可关闭；先更长素材复测。
- **证据**：`collab/evidence/20260921-qwen3-poc/ctx-value-316-raw.log` + harness 快照。红线：未碰 `src/**`、harness 已移出 `tests/`、未动版本/未 commit/未出包/零凭证。

## 2026-09-21 — coder-1 — MIGRATE-QWEN3-314 ✅ 交付（accuracy 档换 Qwen3，识别器层）

- **设计（DEC-076）**：**不新增 `AsrModel` 变体**，只切换 `Accuracy` 背后模型（避开 `[ENUM-EQ-CHECK-MISSES-NEW-VARIANT-001]`）；`uses_accuracy_engine()`/VAD/`native_punctuated` 判据全沿用。**枚举零改动**有 grep 正面证据（空输出）。
- **改动**（仅 `src/transcription/mod.rs`，`+219/10`）：`AccuracyEngine` + `accuracy_engine_from()` + env `VOICE_IME_ACCURACY_ENGINE=funasr|qwen3`（默认 qwen3）；`create_qwen3_recognizer()`（四路径；`max_total_len=2048`、`max_new_tokens=256`、`temp=1e-6/top_p=0.8/seed=42`、**hotwords=None**）；`ensure_qwen3_model()`；`build_recognizer` Accuracy arm 按引擎分派；就位判据拆 funasr/qwen3 两版 + 按引擎分派；`check_local_realtime_models_ready` offline 用 FunASR 判据。
- **验证**：`rustfmt --check` clean、`cargo check --all-targets` 0 error、warnings **110/101**=基线、`numstat`==`-w`（219/10）；3 单测（引擎选择/就位文件集/两引擎构造，后者 `--ignored` 本机跑通：构造均成功）。
- **`native_punctuated`**：判据与引擎无关；313 实测 Qwen3 输出自带「，」「。」⇒ 不回归（原始输出见 result §四）。
- **回滚**：`VOICE_IME_ACCURACY_ENGINE=funasr` 逐位退回 FunASR（模型与代码保留）。
- **边界**：LocalRealtime offline 本单仍 FunASR（换引擎+前文注入下一单）；未碰 `main.rs`/`local_stream.rs`。未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-1 — POC-QWEN3-FITNESS-313 ✅ 交付（Qwen3-ASR 端到端胜任性）

- **结论：有条件胜任** —— 精度/语言/内存达标，**速度比基线慢 ~7.9%**（context+2048KV 代价）。
- **四条判据**（同 298 切片，逐位相同）：① 中文 CER 候选 **0.0444** ≤ 基线 0.0533 ✓；② 速度 12.029s vs **11.151s** ✗(+7.9%)；③ 语言：韩文候选 4/4 正确 Hangul vs 基线全乱码 ✓（日文基线略优、英文候选略优）；④ 内存：20s 片峰值 **2010MB** vs 52s 裸 4298MB（压 53%）✓。
- **Gavin 两点**：自定义上下文指令——候选专名跨片一致（弱证据）；**去口水词英文指令无效**（仅标点抖动，呃/那个/我我我全留，与 299 T2 一致）。
- **311 中止**：主控叫停（不应在 FunASR 上做 PoC），t8 数据原样留 evidence、不补完不写 result。
- **清理（完成判据）**：harness 移出 `tests/` 至 evidence；删 `outbox/poc306`/`poc310`/`models/_ko_tmp`/`models/_ko2.tar.bz2`(418MB)；`models/` 无 tar.bz2/临时目录；保留 `models/korean-testwavs/`(~0.5MB) 与 Qwen3 解压目录（可重下）；`git status` 干净。原始数据全进 `collab/evidence/20260921-qwen3-poc/`。
- **红线**：未碰 `src/**` / 未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-1 — POC-QWEN3-KV-310 ✅ 交付（Qwen3 max_total_len 可上调实测 + 韩文证据补采）

- **结论**：**512 是配置默认值，可上调到至少 4096**（512/1024/2048/4096 全部加载/解码正常，无上限报错）⇒ 选型结论需重写。
- **数据**：加载内存四档一致（~1035MB，与配置无关）；上下文容量 512→≤390 / 1024→≤1040 / 2048→≤2080 / 4096→≤4420 字（rag_chemistry 6.84s）；准确率零退化（Gavin para3+rag 六条 mean CER **四档全 0.0155**，逐字一致）；耗时无实质变化。
- **🔴 追加子问题（单独成条）**：① **KV 非预分配**（同配置 3.5s→+154MB vs 52s→+1999MB，按实际长度增长）；② **per-stream 调小不省内存**（opt 512/2048/4096 均 +154MB）⇒ 只是预算上限/越界防，不是分配开关；clamp 确认。
- **原始数据全进 `collab/evidence/20260921-qwen3-poc/`**（`kv310-raw.md` + `kv310-logs/*.log` + `korean-raw-outputs.md` + `poc_qwen3_compare.rs.txt`）；outbox 仅摘要。含 **306 韩文证据补采**（四条韩文双模型原始输出：Qwen3 4/4 Hangul 正确、FunASR 4/4 崩坏）。
- **范围**：未碰 `src/**`；harness 已 fmt 后移出 `tests/`；`git status` 干净；未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-1 — FIX-PREROLL-RESIDUAL-308 ✅ 交付（窗口开头旧语音判残留）

- **根因**：`select_pre_roll_for_asr` 只看「几段语音」不看「末段在哪结束」⇒ 292 落盘的真实形态（语音贴窗口开头 + 后 500ms 静音）被整段喂入 ⇒ 未说话已有文字（首果 `很好` 是旧话尾巴）。
- **修法（一条判据）**：记录**最后一段结束点**（最后非静音样本）；其到窗口末尾静音 ≥ `PRE_ROLL_RESIDUAL_SILENCE_MS`(200ms) ⇒ 判残留 ⇒ 同场景 B 只留末尾 200ms。日志加 `mode=residual` + `trailing_silence_ms=<n>`。段切分未重写；未动 4 个 PRE_ROLL 常量 / T1/T2 词表 / `is_effective_text`。
- **有意取舍（写入注释）**：说完末字停顿一下才按键也会判残留丢弃 pre-roll —— 用户已说完，丢旧不污染新；真·先开口者按键时仍在说 ⇒ 末尾有能量 ⇒ 不触发。
- **验证**：`rustfmt --check` clean；`cargo check --all-targets` 0 error、warnings **110/101**=基线；`numstat`==`-w`（**150/8**）；新增 7 单测 + 幂等 → `audio::tests` **77P/0F**（原 7 条零回退 + `guard_293_i5` 绿）。
- **在线三档零改动**：唯一生产调用点 `audio/mod.rs:689`（在 `trim_pre_roll_residual` 分支内）；`main.rs:7716` 在线传 `false`、`main.rs:8034` 仅 LocalRealtime 传 `true`。
- 🔴 **实机效果交 Gavin**（未声称已验证）。红线：仅改 `src/audio/mod.rs`；未碰 `src/transcription/**`；未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-2 — FIX-LOCALRT-TAILCHAR-307 ✅ 交付（尾字改整句全量重解码）

- **根因**：291 `input_finished()` 方案被实测证伪（`gained` 七次恒 0）；起作用的是**另起 stream 重喂整句重解码**。Gavin「最后一个字不显示」同因。
- **两处**：endpoint 分支 `:536-561` + 录音结束收尾 `:826-861`，各对最后一句全量重解码，取更长者。
- **选择器**：`endpoint_confirm_text` 两方→三方 `(main, full, shadow)`，严格更多才切换、等长不抖动。
- **保留**：291 flush/换流、284 影子、rule1/2/3、298 常量逻辑（`ACC_MIN_SEGMENT_MS_DEFAULT=5000` 未碰）。**294 G3** endpoint create_stream 计数 1→2。
- **验证**：rustfmt clean、check 0 error、warnings **110/101** 基线、`local_stream::tests` **9P/0F**。⚠️ `numstat` vs `-w` 差 13/13（收尾块去嵌套缩进，非格式化夹带）。
- 🔴 实机 `gained` 交 tester-1/Gavin，未声称已验证。未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-1 — POC-QWEN3ASR-306 ✅ 交付（Qwen3-ASR vs FunASR Nano 选型对比，纯 PoC）

- **结论：各有优劣**（不替 Gavin 决定）。耗时有效音频**打平**（分段 53.84s：11.20 vs 11.19s）；准确率**互有胜负**（Qwen3 `para3` CER 0.0、间隔号更全；FunASR `rag_chemistry` 酯 与 `飞音` 更准）；**语言 Qwen3 决定性胜**（韩文 4/4 Hangul vs FunASR 乱码）；**上下文容量 FunASR 胜**（6.84s 音频 ≤~520 字 vs Qwen3 ≤~390 字）。
- **KV/token 必测**：`max_total_len` FunASR **1024** / Qwen3 **512**（metadata 上限）。Qwen3 **per-stream 逐句可调且实测生效**（512/256 全量、160 截断、128/64 `language`，日志报 `audio_token_len=123 -> keep_audio=113`）；音频速率 Qwen3 **13.0 tok/s** / FunASR ~16.5 tok/s；20s 音频 ≈260 vs ≈330 token；裸长音频上限 Qwen3 ~38s / FunASR ~60s（52s 实测两者都失败）。
- **🔴 撞顶（单独成条）**：输入溢出两模型都有日志、Qwen3 更精确（`context_len (692) exceeds max_total_len (512). Truncating audio placeholders…` + suggestions）；**但「生成饿死」二级静默两者相同**（Qwen3 ctx=520 截断 11 字无日志）。⇒ **不是「强一个量级」**。
- **per-stream hotwords 实测生效**（`只在`→`指在`）⇒ 逐片上下文可行，FunASR 无此能力。
- **范围**：新增临时 `tests/poc_qwen3_compare.rs`（`#[ignore]`，未跟踪，去留听主控通知）；生产零改动、未动 `Cargo.toml`。
- **清理**：删 poc306 日志 + 韩文模型包（418MB）；保留 Qwen3 模型与 `models/korean-testwavs/`；`target/debug/deps/` 补 7 DLL（System32 onnxruntime 1.17.1 遮蔽 1.24.4 致崩溃，纯构建产物）。
- **红线**：未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-2 — WIRE-FF303-305 ✅ 交付（口水词过滤接进管线，Gavin 要求进本包）

- **改动三文件**：`main.rs`（节点 `apply_filler_strip` `:9849` + 唯一调用点 `:9499`）、`text_normalizer.rs`（删 1 行 `#[allow(dead_code)]`）、`docs/MACOS-HANDOFF.md`（+8）。
- **位置**：`apply_local_punctuation`（`:9486-9493`）**之后**、`platform::inject_text`（`:9555`）**之前**，实参 `!llm_handled`。
- **DEC-041 不触**：`enabled=false` 首行原样返回，开 LLM 时完全不执行；四档共用 `run_pipeline_core`，无新判据/开关。
- **验证**：`rustfmt --check` clean、check 0 error、warnings **110/101** 基线、`numstat`==`-w`、新增 `filler_strip_303_tests` **3P/0F** + `ff303` **35P/0F**。
- **边界**：未动 298 常量/逻辑（`ACC_MIN_SEGMENT_MS_DEFAULT=5000` 未碰）；未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-1 — FIX-FF303-B ✅ 交付（规则 B 白名单门控 + 韩语空白分词）

- **规则 B 白名单门控**：只有重复单元本身是已知话语标记才折叠（ZH `然后/就是/那个/这个/所以/反正/其实`；EN `i/the/a/and/so/but/like/you/we/it`；JA `その/あの/えー/まあ`；KO `그/저/음`）。`然后然后`→折叠、`研究研究`→原样、`I I`→折叠、`had had`/`that that`→原样。
- **两张表用途不同不合并**：A 表＝「单独出现即可摘」（那些有实义的 `然后/就是/那个` 不进 A）；B 表＝「只有紧邻重复才折叠」（它们进 B）。
- **韩语改空白分词**（orchestrator 补充）：新增 `is_hangul_char` + Hangul token 分支（口径同英文），`is_cjk_char` 移除韩文范围 —— 修复此前「KO 表全单字 + p≥2 ⇒ 韩语规则 B 静默缺失」。`그 그 사람`→`그 사람`；`그 그림` 不动。
- **验证**：`rustfmt --check` clean；`cargo check --all-targets` 0 error、warnings **110/101**=基线；`numstat`==`-w`（**655/0**）；`cargo test --bin feiyin-ime text_normalizer::` **104P/0F**（原 27 零回退 + 新增 28~34 + 韩语 8 条）。
- **残余风险**：B 表为人工枚举 ⇒ 新口吃词漏摘（有意代价）；`呃逆` 生僻词句首被摘；逗号后语气词保持不摘。
- **红线**：仅改 `src/text_normalizer.rs`；未碰 `src/main.rs`/`src/llm/**`；未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-1 — FIX-FF303-A ✅ 交付（修 303 两条误摘，必修）

- **必修一（规则 B）**：CJK 折叠单元下限 1→2 —— `看看`/`想想`/`试试`（动词重叠）、`哈哈哈`（情绪叠词）不再折叠；`然后然后`（2 字）仍折叠；英文按词不变。🔴 句首叠写 `嗯嗯/呃呃` 由**规则 A** 独立摘除（用例 24 钉死，不依赖规则 B）。
- **必修二（规则 A 分档）**：`LEADING_FILLERS_ZH` 拆 **T1 无边界**（`呃`/`嗯`）与 **T2 须边界**（`啊`/`哦`/`噢`/`唉`/`诶`/`欸`/`呐`）—— `唉声叹气`/`呐喊`/`哦豁` 不再被啃词头；`唉，今天真累`（后接逗号）仍正确摘除。
- **改判**：`唉，今天真累` 句首叹词摘除是**预期行为**（非风险），用例 26 覆盖。
- **验证**：`rustfmt --check` clean；`cargo check --all-targets` 0 error、warnings **110/101**=基线；`numstat`==`-w`（**507/0**）；`cargo test --bin feiyin-ime text_normalizer::` **96P/0F**（原 17 + 新增 18~27，原 17 一条未退）。
- **残余风险**（改后）：① 规则 B 仍折叠 4 连及以上 `哈哈哈哈`→`哈哈`；② T1 `呃逆` 句首会被摘（生僻医学词）；③ 逗号后语气词不摘（符合规格）。
- **红线**：仅改 `src/text_normalizer.rs`；未碰 `src/main.rs`/`src/llm/**`；未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-1 — FORMAT-FALLBACK-303 ✅ 交付（本地免费语气词去除纯函数，第一步）

- **接口**：`text_normalizer::strip_fillers_conservative(text: &str) -> String` —— 管线无关（不接管线类型/language）、不做启停判断、无日志/IO/配置、零新依赖（无正则）；暂带 `#[allow(dead_code)]`（挂载点下一单，warnings 保持 110/101）。
- **规则 A（句首犹豫词）**：中 9 单字 / 英 8 / 日 7 / 韩 4，最长命中，摘后吃 `，,、`/空白。🔴 中文单字**不要求后续边界**（`呃我觉得` 也摘，第 17 条）；英/日/韩**要求边界**（防 `Uhura`/`어디`/`まあまあ`）。
- **规则 B（紧邻重复折叠）**：≥2 次字面相同（中间可选逗号/空白），英按词、中日韩 1–3 字 CJK 单元；迭代到不动点 ⇒ 幂等。
- **主动排除**：那个/就是/然后/所以/但是、裸 あの、그/저기/그러니까（均有实义，理由见 result.md §三）。
- **验证**：`rustfmt --check` clean；`cargo check --all-targets` 0 error、warnings **110/101**=基线；`numstat`==`-w`（**411/0**）；`cargo test --bin feiyin-ime text_normalizer::` **86P/0F**（含 17 条 ff303：必需 7 + 边界外 8 + 形态 2 + 幂等）。`docs/MACOS-HANDOFF.md` 补一节（平台中立）。
- **残余风险**（已声明）：规则 B 会折叠 `看看`→`看`/`哈哈哈`→`哈`；中文无边界 ⇒ `唉声叹气` 句首被摘。按规格实现，未擅改。
- **红线**：仅改 `src/text_normalizer.rs`（+ docs）；未碰 `src/main.rs` / `src/llm/**`；未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-1 — POC-ACC-CONTEXT-299 ✅ 交付（FunASR Nano 上下文注入路径验证，纯 PoC）

- **结论（通了）**：不改 C++，靠「不传 hotwords + 自拼完整 user_prompt」（`has_override=false` ⇒ C++ 原样使用）即可注入任意上下文；`rag_chemistry` 基线 `只在/脂` → 注入「化学术语参考：酯…」后**纠正为 `酯`**。写法：`前文行` 是有效成分，已有前文时热词行无额外增益（P2==P3，P3 省 ~20 token）。
- **per-stream 通道不通（致命）**：FunASR Nano 未 override 带 hotwords 的 `CreateStream`；基类默认 `SHERPA_ONNX_LOGE("Only transducer models support contextual biasing.") + SHERPA_ONNX_EXIT(-1)` ⇒ **桌面进程直接终止**，禁止调用（PoC 未调）。
- **harness 自检**：baseline（传 hotwords 走官方 prefix）vs P1（自拼同串）**18/18 逐字相同** + `user_prompt_dyn` 相等。格式五条（半角 `, ` / 每处 3 `\n` / 全角标点 / prefix-task 直拼 / `system_prompt` 用官方默认 `You are a helpful assistant.`，非生产空串）逐条对齐。
- **四组**：baseline==P1；P2/P3 chemistry 纠错、history 改标点；其余不变。KV `max_total_len=1024`，context_len baseline 185~295 / P2 +17~21 / P3 −20~24，**无 Truncating/Falling back**。
- **字符上限**（6.84s 音频）：context_len ≤469（上下文≤390 字）正确；569 起**静默退化**；769 空；1169 `Falling back`。二级静默（DEC-073）无日志。
- **T1**（itn.wav）：`不进行文本规整` 有效于**标点**、**不改数字**（DEC-030 延续）。**T2**（colloq.wav）：`去除口头禅与重复`/`去除语气词`/`口语顺滑` **全无效**，口语垃圾全留。
- **每片不同上下文**：`user_prompt` 是 recognizer 级 config ⇒ 不改 C++ 只能每片重建 recognizer（972MB/~6s）；stream 级路径须改 C++（本单只出依据）。
- **清理**：删 PoC 源 + exe + `collab/outbox/coder-1/poc299/`；未动 `Cargo.toml`；生产零改动；未动版本 / 未 commit / 零凭证。

## 2026-09-21 — coder-2 — FIX-GUARD-301 ✅ 交付（生产区扫描边界整族修复，第三次复发）

- **根因**：护栏用「首个 `#[cfg(test)]`」当生产区边界 —— 298 在 `main.rs:9165` 插 test mod 致切点塌缩 ⇒ 5 条锚点出扫描区 ⇒ `TEST-EXEC-300` 假红（生产无缺陷）。
- **改法**（仅 `src/main.rs` test 区）：新增 `#[cfg(test)] mod guard_prod_lines`（`:13070`），语义改「**剔除全部 test-gated 项**」；4 处调用点全改用它（旧 `starts_with` 命中 0）。
- **test 判据扩严（主控批准）**：`#[cfg(` ∧ 含 `test` ∧ 不含 `not(test)` —— 同时剔除 8 处 `#[cfg(all(test, target_os="windows"))]`，防其被当生产区扫。`#[cfg_attr(test,…)]` 不命中（不误剔）。
- **剔除清单**：恰 17 条属性行（9 plain + 8 all-test），逐条行号见 result.md §三；与主控预期 25 的差异已说明（6 条为注释行，非属性行）。
- **静态复算**：main.rs 扫描 9164→10109 行；三组护栏 needle 计数旧→新无一变化 ⇒ 预测无护栏变红。
- **验证**：0 error、warnings **110/101** 基线、`rustfmt --check` clean、`numstat`==`-w`（106/40）。🔴 **未跑 `cargo test`**（DEC-048），复跑在 tester-1。
- **红线**：未动版本 / 未 commit / 未碰 `src/transcription/**` 与 `poc_acc_context.rs` / 零凭证。

## 2026-09-21 — coder-2 — LOCALRT-PARALLEL-ACC-298 ✅ 交付（800ms 静音切片 + accuracy 并行转写）

- **派发**：`local_stream.rs` `AccDispatchConfig`（env 三开关）+ 纯函数 `should_dispatch_acc`/`should_dispatch_tail`；静默 ≥800ms ∧ ≥3s ∧ 本轮未派 ⇒ `build_padded_segments` 派发（复用 200ms padding + >20s 硬切）；循环末派尾片（有语音才派）。不 reset/不切句。
- **转写**：`main.rs` 同 scope 新增 accuracy worker（`SendOfflineRecognizerRef`，对象独占），`cancel_signal` 即 break；join 后 `assemble_parallel_accuracy` 有序拼接 + `join_segment_texts` + normalize。
- **下游**：`run_pipeline_core` 加显式参数 `pretranscribed: Option<(String,bool)>`，新分支跳过重转录、照发 `Processing`；`from_online_streaming` 一字未改 ⇒ 本地档 ITN/标点路由逐位一致；`native_punctuated`=各段 `all_native` 与（非文本反推）。
- **🔴 SAFETY 更正（采纳主控意见）**：不再套 `SendOnlineRecognizerRef` 的时间论（298 真并发），改对象独占论证，并给出两条代码证据（reload 仅 loop 顶/D2、scope 前；唯一生产调用点在 scope 后）。详见 result.md §二。
- **边界**：在线三档零改动；`LOCAL_RT_ACC_PARALLEL=0` 逐位退回串行。
- **验证**：0 error、warnings **110/101** = 基线；三文件 `rustfmt --check` clean；`numstat`==`-w`；新增单测 **12P/0F**。🔴 实机 `tail_wait` 交 tester-1/Gavin。
- **红线**：未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-2 — TEST-SYNC-295 ✅ 交付（阶段三：给 coder-1 的 293-B 补 I5 调用点结构护栏）

- **先查覆盖（不重复造）**：293-B 的 I1~I4/I6/I7 已被现有用例覆盖（详见 result.md §一）；主控猜的缺口②「样本未被改动」已在 `:2168` 钉住、缺口③「150/200 判别力」已被精确值断言覆盖 ⇒ **均不补**。
- **唯一真缺口 I5**：新增 `mod tests::guard_293_i5`（`src/audio/mod.rs:2000`，纯 **+81/−0**，单 hunk 全在 `mod tests`）。`include_str!` 自读生产源码：`select_pre_roll_for_asr(` 生产区**恰 2**（1 定义 `:358` + 1 调用 `:638`），且调用落在 `let pre_roll_chunks = if trim_pre_roll_residual {` 块内。
- **消融**：移出 trim 分支 / 加第二调用点 / 删调用 ⇒ 红。
- 🔴 **实现坑**：生产区切点用 `mod tests {` 而非「首个 `#[cfg(test)]`」——`:269` 有 `#[cfg(test)] fn PreRollDump::new_in` 夹在生产段中间，误切会漏真实调用点致护栏恒绿。
- **验证**：`rustfmt --check` clean、`cargo check --all-targets` 0 error、warnings **110/101** 基线、`numstat`==`-w`（81/0）。🔴 **未跑 `cargo test`**（DEC-048 阶段三只许 fmt/check），首跑在阶段四。
- **红线**：只改 `audio/mod.rs` test 区 / 未碰 `local_stream.rs` / 未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-1 — FIX-GUARD-297 ✅ 交付（修 294 护栏自身区域定界缺陷 / G3 假红）

- **根因（护栏缺陷，非生产缺陷）**：`endpoint_guard_regions` 用「首个 `} else`」截断 endpoint 块，误命中块内嵌套的 `let confirm_text = if use_shadow {…} else {…}`（`:400`）⇒ 扫描区截到 `:399`，真正的 `recognizer.create_stream()`（`:450`）被排除 ⇒ G3 实测 0 假红；G1 两次 `get_result`（371/380）恰在错误区间内而偶然成立，本轮绿不作数。
- **修法**：新增 `endpoint_branch_bounds`（单遍花括号游标）——从 `if endpoint {` 起维护相对 depth，`depth == 1 && 行首 == '}'` 即真分支闭合花括号（Rust `} else if … {` 的第一个 `}`）；嵌套 `}` 出现时 depth ≥ 2 天然不命中 ⇒ **对任意嵌套成立**。另加两条自证断言（区间首行 == `if endpoint {`、末行下一行 `starts_with('}')`）。
- **新区间**：行 **362…455**（首行 `if endpoint {`，末行 `shadow_done_for_pause = false;`，下一行 456 为 `} else if …`）。四断言应得：G1 get_result{371,380} 夹 flush{375}／G2 函数体 reset=0／G3 create_stream={450}=1（**修复点**）／G4 on_result={435,482,604}=3 且 `, true,`={435}=1。**期望值无一改动**。
- **验证**：`rustfmt --check` clean；`cargo check --all-targets` 0 error、warnings **110/101** = 基线；numstat==`-w`（38/7）；diff hunk 全在 `mod tests`，生产区零行。
- 🔴 **未跑 `cargo test`**（DEC-048 白名单）——**未执行，首跑仍在 tester-1**。未动版本 / 未 commit / 未出包 / 未碰 `src/audio/mod.rs` / 零凭证。

## 2026-09-21 — coder-1 — TEST-SYNC-294 ✅ 交付（阶段三：给 coder-2 的 291 写源码级结构护栏）

- **范围**：只改 `src/transcription/local_stream.rs` 的 `#[cfg(test)] mod tests` 区，`+168/0`，生产区零行。`OnlineRecognizer` 需真模型 ⇒ 行为级做不了，照 `src/main.rs::overlay_121_guard_tests` 读生产区源码 + needle 计数 + 花括号定界取块。
- **四条护栏**：G1（I1）`stream.input_finished()` 行号夹在 endpoint 分支两次 `.get_result(&stream)` 之间；G2（I2）函数体 `recognizer.reset(` == 0；G3（I3）endpoint 分支 `recognizer.create_stream()` == 1；G4（I4）函数体 `state.on_result(` == 3（`:435` true / `:482` false / `:604` 收尾 false）且 `, true,` 确认恰 1。每条在 doc 注释写明「改错怎么红」。
- **关键坑**：`if endpoint {…} else if …` 是同一 if 表达式、花括号定界会并成一块 ⇒ `endpoint_guard_regions` 块尾剔除 `} else` 分支（否则 G1 的 get_result 数到 3 而非 2）。needle 全带完整前缀并逐条 grep 核实（`[FMT-COLLATERAL-001]` 教训）。
- **验证**：`rustfmt --check` clean；`cargo check --all-targets` 0 error、warnings **110/101** = 基线；numstat==`-w`（168/0）；hunk `@@ -669,0 +670,168 @@ mod tests {` 全在 test 区。
- 🔴 **未跑 `cargo test`**（DEC-048 阶段三白名单只许 `cargo fmt` / `cargo check`）——护栏**未执行过，首跑在阶段四 tester-1**，如实声明，不写「已验证通过」。未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — coder-1 — FIX-LOCALRT-FIRSTCHAR-293（293-B 修订版）✅ 交付（pre-roll 窗口 600→1000ms + 锚定语音起点裁剪）

- **由来/修订**：Gavin 拍板先修「首字爆破音被 600ms 边界削掉」（`输入你`→`输入按`）；但又补「先按键后开口、旧 buffer 干扰首字」——**只加长窗口会让后者更差**。故 293-B：窗口容量拉 1000ms（不错过早到语音），但喂 ASR 的只从语音起点前 150ms 起。
- **改动（`src/audio/mod.rs` 单文件）**：`PRE_ROLL_CAPACITY_MS=1000`（环形缓冲容量一处）；`PRE_ROLL_LOCAL_RT_MS=1000` + `pre_roll_ms_for_streaming(trim_pre_roll_residual)` 分流（本地 1000 / 在线 600，复用现成第 8 参，不新增穿透）；`trim_pre_roll_last_speech_segment` **泛化**为 `select_pre_roll_for_asr`（≥1 段→最后一段起点−150ms；0 段→末尾 200ms 不为空；无 ≥200ms 静音游程→原样返回兜底）；`PRE_ROLL_PAD_MS` 100→150；埋点 `[LocalRT-DBG-293] window/kept/mode/onset_at`（debug!+守卫）。
- **在线零改变**：`retain_recent_samples` 只留最近 N ms + 在线 `trim_pre_roll_residual=false` **不进裁剪函数**；`record()`（批处理）仍取 600。
- **验证**：`cargo check --all-targets` 0 error、warnings **110/101**=基线；`rustfmt --check` clean；numstat==`-w`（352/67）；新增/替换 8 条单测（A/B/C/兜底/冷启动/分流）+ 292 两条 → `audio::tests` **69P/0F**；root 全量 `cargo test` **1293P/0F/15I**。🔴 实机 `dur=1000ms` 观测交 tester-1/Gavin（我不能跑）。`urgent286_...` 用例加 `LOG_LEVEL_MUTEX` 串行化防并行翻转 max_level。
- **红线**：未碰 `src/transcription/`（coder-2 在途）/ 未动版本 / 未自行 commit / 未出包 / 未跑 release / 零凭证。

## 2026-09-21 — coder-1 — DIAG-LOCALRT-FIRSTCHAR-292 ✅ 交付（debug-only pre-roll WAV 落盘取证）

- **目的**：Gavin 报流式档首字不准（`我自翻` vs `端`）。日志只能证明「先开口后按键」那次语音贴 600ms 窗口末尾，**无法判定声学起点是否被窗口削掉**，两个处置方向相反 ⇒ 先取音频。
- **改动（`src/audio/mod.rs` 单文件 + `.gitignore`）**：`PreRollDump`（`new` 首行 `log_enabled!(Debug)` 守卫 ⇒ 默认 Warn 零文件零计算；写 ① 原始 pre-roll ② pre-roll+其后 2s 实时，Drop 补写）；`write_wav_pcm16` 手写零依赖 PCM16（采样率写真实值，`f32` 先 `clamp` 再 `*32767`）；`enforce_dump_limit(dir,20)` 删最旧防撑爆盘；`debug_audio_dir` exe 同级（DEC-011）；埋点 `[LocalRT-DBG-292]` 含 `head_clipped`（首个 |s|>0.01 落在窗前 40ms 内）。**只读旁路，喂 ASR 音频逐 bit 不变**（单测断言）。
- **验证**：0 error、warnings 110/101=基线、`rustfmt --check` clean、numstat==`-w`；落盘 WAV 经 **Python `wave`** 独立解码 `ch=1/16bit/48000Hz/0.250s` + sherpa `Wave::read` 回读通过。Gavin 产 WAV 步骤见 `result.md` §一。
- **红线**：WAV 属用户语音，`debug-audio/` 已入 `.gitignore`，未提交 / 未动版本 / 零凭证。

## 2026-09-21 — coder-2 — FIX-LOCALRT-TAILCHAR-291 ✅ 交付（中间句丢尾字：endpoint reset 前先 flush）

- **根因**：`transcribe_streaming_local` 的 `input_finished()` 全函数只在 loop 结束后调一次（只救最后一句）；中间句走 `endpoint → recognizer.reset()`，**reset 前从未 flush** ⇒ 解码器压着的最后 token 被丢，每句结构性少尾字。
- **修法（方案 A）**：endpoint 时 `get_result`(flush 前，回落用) → `stream.input_finished()` → drain `decode` → `get_result`(flush 后完整) → 确认本句。新增私有纯函数 `endpoint_confirm_text`（取更长者，flush 后空/更短回落 flush 前）⇒ 绝不整句消失/回退。`input_finished()` 后 stream 不可复用 ⇒ 换 `recognizer.create_stream()`（`let stream`→`let mut stream`），不再 `reset()`。
- **不重复确认**：endpoint 一次 + 非 endpoint 一次（原样）⇒ FIX-252 不复发。
- **诊断**：`[LocalRT-DBG-291] endpoint flush: before_len/after_len/gained`（`debug!`+`log_enabled!` 守卫）。预期 `gained` 多为 1；**若实测恒 0 立刻停手报主控**。
- **未动**：284/289 影子代码（`use_shadow` 预期恒假，删减下一单）；`RULE1/2/3`／`PRE_ROLL_MS`／`SHADOW_MAX_AUDIO_SECS`／269/269-B 标点触发；在线三档（唯一调用点 `main.rs:7948` LocalRealtime）。
- **验证**：`cargo check --all-targets` 0 error、warnings **110/101** = 基线；`rustfmt --check` clean；新增单测 `local_stream::tests` **2P/0F**（连跑 3 次）。🔴 实机（`gained` 实测）交 tester-1/Gavin。
- **⚠️ 协作事件**：主控文档 commit `99799c5`（10:26:26）在本题进行中执行，**把我未完成的 `local_stream.rs` 与 coder-1 的 `src/audio/mod.rs` 一并扫入**（commit message 未反映代码改动）。本单改动已随之落盘、worktree 无额外 diff；`git status` 另见 `.gitignore` + `src/audio/mod.rs`（非本单）。请主控知悉该 commit 语义与文件归属。
- **红线**：只改 `local_stream.rs` / 未动版本 / 未自行 commit / 未出包 / 未跑 `cargo build --release` / 零凭证。

## 2026-09-21 — tester-1 — TEST-EXEC + BUILD-328 ✅ 四单全量回归 + 出包（八项 PASS，含 4 张规则表三副本复核）

- **基线**：HEAD `08fe5c6`，clean、`ui/` 零 diff、版本 0.9.2。含 323/324/322+325/327 + 77313e5 nano 解依赖。
- **回归**：root **1385P/0F/17I**（NEW 19/GONE 0：`itn_shifen_323_*` 3 + `preview_reflow_325_tests` 6 + `wordbook::tests::autolearn327_*` 6 + `audio::tests::pr322_*` 4；`pr322_idle_probe_manual` `#[ignore]` 手工硬件探针 ⇒ **+18 运行/+1 ignored**）；`src-tauri` **91P/0F/0I**（85→91，**NEW 6 = `wordbook::wordbook_core::tests::autolearn327_*` 镜像**，任务书未预告已如实列出）；Vitest **SKIP**（`ui/` 零 diff）。
- **BUILD-328**：Step1–4 全走；源码 mtime 前后 md5 一致（`bff96396…`）。产物 main `e39ac3a0…`（14,662,656B/18:24，较 321 **+50,176B**）/ ui `8294a624…`（10,060,288B）/ crash `4fcc90ee…`（24,879,104B/18:22）；两副本相等；main 异于 BUILD-321（`ea426a0c…`）。
- **八项逐项 PASS**：①时间戳 18:19–18:24 ②sha ③0.9.2（Cargo.toml + tauri.conf.json 未动）④冒烟 PID 18008 Responding=True/无新 crash.json/残 0 ⑤**config/wordbook 零变化**（Publish/config `da2be5da…`、target/release/config `2e0e60a5…`、两 wordbook 不变；⚠️ 根目录无 config/wordbook，任务书「三处」实测两处）⑥warnings 110/9/17 ⑦**四张规则表三副本全等**（itn `ab950ba4…`/scene `8ea93bb1…`/homophone `a5fd4a61…`/wordbook `ac9a72ee…`）⑧探针。
- **探针**：`[LocalRT-DBG-325]`=1 / `[AUTOLEARN]`=1 / `degree_adverbs`=2 / `nz_ratio`=2 / `[LocalRT-DBG-320]`=1 / `[MIGRATE-QWEN3-320]`=1 / `307`=2 / `293`=1 / `298`=4 / 同音 `满头大汉`=1。
- **Gavin 端测六条**：① 十分的重要保汉字 + 十分钟→`10分钟` ② 尾字回灌 `[LocalRT-DBG-325] action=applied` ③ 🔴 **预览窗编辑不被回灌冲掉**（本批最高风险）④ 自学习 `[AUTOLEARN] promoted after threshold` ⑤ 先说半句 pre-roll `[293] mode=onset` + `[278] nz_ratio/dBFS` ⑥ `[320] action=redecode` 频次应远低于 43%。
- **证据**：`collab/outbox/tester-1/testexec328/`。
- **红线**：版本 0.9.2 未动 / 未改生产代码 / 未 push / 零凭证。

## 2026-09-21 — tester-1 — TEST-EXEC + BUILD-321 ✅ 全量回归 + 出包（八项 PASS，Qwen3 迁移批次）

- **基线**：HEAD `51df565`（320 收尾），clean、`ui/` 零 diff。包含 307/308/303+305/318/314+315+320/317+320。
- **回归**：root **1367P/0F/16I**（EXIT 0，**与 coder-1 独立复现一致**）；`src-tauri` **85P/0F/0I**；Vitest **SKIP**（`ui/` 零 diff）。
- **增量**（对比 TEST-EXEC-309 1349P/15I）：**NEW 19 / GONE 0** = 8 `homophone::tests` + 10 `transcription::tests`（320）+ 1 `local_stream`（317+320）；`migrate320_qwen3_recognizer_constructs` `#[ignore]` ⇒ **+18 运行/+1 ignored**（1349→1367、15→16 ✓）。
- **🔴 逐条点名护栏 38/38 ok**：`guard291_g1..g4`、`guard_293_i5`、`nospeech_122` 8/8、`guard_214_215` 12/12、`overlay_121` 10/10、`flicker_130` 3/3 ⇒ 三处源码改动未致扫描区塌缩；318 同音边界 `homophone::tests t1~t8` 8/8。
- **附带核实**：`src/transcription/` 可调 env 读取点**归零**（仅剩 `current_exe`/`temp_dir`）；`homophone-rules.toml` 为 `include_str!` 内嵌（exe 同级覆盖），Publish 侧无该 toml 也走内置 = 正确规则，**非缺陷**。
- **BUILD-321**：Step1–4 全走；源码 mtime 前后 md5 一致（`7ace5bba…`）。产物 main `ea426a0c…`（14,612,480B/15:29，较 306 **+120,832B**）/ ui `8a0a891f…`（10,060,288B）/ crash `c9d7f17c…`（24,879,104B/15:28）；两副本相等；main 异于 BUILD-306（`bc043e25…`）。八项逐项 PASS（①时间戳 ②sha ③0.9.2 ④冒烟 PID 26352 Responding=True/无新 crash.json/残 0 ⑤config `da2be5da…`+wordbook `b6ab43ac…` 零变化 ⑥110/9/17 ⑦探针 ⑧toml 三副本全等）。
- **探针**：`[LocalRT-DBG-320]`=1 / `[MIGRATE-QWEN3-320]`=1 / `[LocalRT-DBG-307]`=2 / `293`=1 / `298`=4 / `291`=1 / `292`=2 / `284`=4 / `289`=1 / `276`=3 / `277`=1 / `278`=2；同音规则 `满头大汉`=1 / `满头大汗`=1（318 进包）。
- **🔴 两条模型附加核验**：① **Qwen3 进 `Publish/models/`** ✅（构建前缺失已上报；内容级 sha 三主文件 + tokenizer 三文件两侧全等）；② **FunASR Nano ⚠️ 仍有一处引用**——`mod.rs:1019` `hotwords_tokenizer()` 仍从 nano 目录取 `Qwen3-0.6B/tokenizer.json`（**只计 token，非 ASR**；accuracy ASR 已是 Qwen3 单引擎）⇒「exe 不再引用 nano」不成立，如实报出；建议改指 Qwen3 ASR 模型自带 tokenizer，等主控定。sense-voice=2 属 performance 档/报错文案，预期内。
- **Gavin 端测四组**：A 浮层预览（307 `gained` 多数应为 1／308 `mode=residual`）／B 最终文字（Qwen3 CER、**韩文应正确**、口水词）／C `tail_wait` + `[LocalRT-DBG-320] action=redecode` 频次／D 本地流式+翻译。🔴 **本包无任何 env 可调**（切片5s/静默800ms/上下文500字/词条200/线程按核数）。
- **证据**：`collab/outbox/tester-1/testexec321/`。
- **红线**：版本 0.9.2 未动 / 未 commit / 未 `cargo clean` / 未破坏性 git / 零凭证。

## 2026-09-21 — tester-1 — TEST-SWEEP-319 🔴 blank_penalty 曲线：对流式 paraformer 完全无效（只测量）

- **归属**：HEAD `2d814a7`；工作区非 clean（`M src/transcription/mod.rs`，coder-1 315 在途）；只测量、不出包。
- **前置核查（已上报，主控裁定 A）**：`poc_local_stream.rs` **原本无 blank_penalty 入口**（从不给 `c.blank_penalty` 赋值）；`poc_funasr_nano` 的是离线 accuracy 不适用；素材实际在 `collab/research/audio-real-gavin/processed/`（非任务书写的 `collab/evidence/`）。
- **方法**：临时给 PoC bin 加只读 env `POC_BLANK_PENALTY`（默认 0.0）→ **已备份 cp 原样还原，`git diff src/bin/poc_local_stream.rs` 空**；未碰 `src/transcription/**` / Publish / 未跑 `cargo build --release`。
- **赋值自证**：临时 `eprintln!("[SWEEP319] applied blank_penalty={}")` 实测 `-100 ⇒ applied -100`、`100 ⇒ applied 100` ⇒ env 确已写入 config。
- **曲线**（full.wav 56s，threads 4，chunk 100ms；打分复用现成 `score_cer.py` 的 normalize/edit/cer）：bp = 0.0 / −0.5 / −1.0 / −1.5 / +0.5 → **五档全 CER 0.1448、ins 1 / del 7 / sub 24、hyp_len 215、尾字后缀 7，`[RTF-final]` 逐字节相同**；极端值 **±100（para1.wav）仍逐字节相同** ⇒ `blank_penalty` 在流式 paraformer **解码路径未被消费**。
- **结论**：按约定「正负两向无变化 ⇒ 无效，一句话收工」⇒ 第一轮结束，不再深入；未改任何默认值。
- **🔴 附带发现（上报主控）**：317 的生产 env `LOCAL_STREAM_BLANK_PENALTY` 是 **no-op**（设置的正是不被消费的同一 config 字段），与 `[PROVIDER-SILENT-FALLBACK-001]` 同族；建议注释标注无效或下批撤掉，等主控定。
- **第二轮（线程组合）** 待 315 `ACC_NUM_THREADS` 落地后做（4+8=12 线程 vs 8 物理核的超订问题仍待答）。
- **原始数据**：`collab/evidence/20260921-sweep319-blankpenalty/`（5 份 log + score.py + score_result.txt，UTF-8）。
- **红线**：未出包 / 版本未动 / 未改生产代码（PoC 已还原）/ 零凭证。

## 2026-09-21 — tester-1 — TEST-EXEC-309 ✅ 307/308 回归全绿（🛑 出包按 Gavin 令暂停，与 Qwen3-ASR 选型整合后一起出）

- **基线**：HEAD `5ca48b2`（308），clean、`ui/` 零 diff。**未开始 Step1-4、Publish/ 未动**（仍 BUILD-306）。
- **回归**：root **1349P/0F/15I**（EXIT 0；上轮 1342P → **+7**）；`src-tauri` **85P/0F/0I**；Vitest **SKIP**（`ui/` 零 diff）。
- **+7 对账**（NEW 9 / GONE 2）：308 新增 7 条 `audio::tests::pre_roll_residual_308_*`；307 新增 2 条 `endpoint_confirm_text_takes_longest_of_three` / `endpoint_confirm_text_len_not_below_main`；旧 291 两参用例 `endpoint_confirm_text_{never_regresses,len_not_below_before}` **−2**（扩为「三选一取最长」的有意替换）。
- **🔴 护栏逐条点名**：`guard291_g1..g4` 全 ok（**G3 计数已由 307 同步 1→2** = 全量重解码 + 换流）、`audio::tests::guard_293_i5` ok、`nospeech_122` 8/8、`guard_214_215` 12/12、`overlay_121` 10/10、`flicker_130` 3/3（main.rs 四组 **33/33**）⇒ 307/308 **未致扫描区塌缩/锚点位移**。
- **Gavin 端测（带 `-debug`）**：**A 尾字** = `[LocalRT-DBG-307] endpoint/final full-decode: … gained=<n>`（🔴 多数应为 1；**恒 0 则立即报主控**）；**B 残留** = `[LocalRT-DBG-293] … mode=residual trailing_silence_ms=<n>`。其余照旧（首字三场景分开测 / `tail_wait` / 本地流式+翻译 / 切片 A/B/C / 口水词过滤）。
- **🔴 已知未修**：模型对纯静音也可能凭空吐字（BUILD-306 实证 pre-roll `peak=0`，+444ms 就出「我是」）——**308 修不到**，属模型幻觉，需另立单；用 `[LocalRT-DBG-292]` 的 `first_speech_at` 区分。
- **证据**：`collab/outbox/tester-1/testexec309/{root_test.log,tauri_test.log}`。
- **红线**：未出包 / Publish 未动 / 版本 0.9.2 未动 / 未改生产代码 / 零凭证。

## 2026-09-21 — tester-1 — TEST-EXEC + BUILD-306 ✅ 全量回归 + 出包（八项 PASS，303+305 口水词过滤接线）

- **基线**：HEAD `60c0457`（305 接线），clean、`ui/` 零 diff。（本单首次「放行」口头消息因含 shell 元字符被误当命令执行、未送达，主控改走任务书重发。）
- **回归**：root **1342P/0F/15I**（EXIT 0；上轮 77ef6aa 1339P → **+3**）；`src-tauri` **85P/0F/0I**；Vitest **SKIP**（`ui/` 零 diff）。
- **+3 增量**（全在 `main.rs::filler_strip_303_tests`）：`filler_strip_disabled_is_identity` / `filler_strip_enabled_removes_leading_filler` / `filler_strip_enabled_noop_when_nothing_to_strip`。
- **🔴 逐条点名扫描 main.rs 生产区的护栏**（305 动 main.rs）：`nospeech_122_guard_tests` 8/8 ok、`guard_214_215` 12/12 ok（含上两轮修好的 g7/g9/g10/g11）、`overlay_121_guard_tests` 10/10、`flicker_130_guard_tests` 3/3、`overlay_075/086/109` 全 ok ⇒ **新节点未造成扫描区塌缩/锚点位移**。
- **额外核对**：`ACC_MIN_SEGMENT_MS_DEFAULT` 源码恰 1 处 = **5000**（`local_stream.rs:107`）；测试里的 `3000` 是显式入参（非默认写死）。⚠️ `local_stream.rs:115/166` doc 注释仍写 3000/3s（注释滞后，非缺陷，未改，可下批校）。调用点 `main.rs:9499`：`apply_filler_strip(final_text, !llm_handled)`，在 `apply_local_punctuation` 后、inject 前；`enabled=!llm_handled` ⇒ LLM 接手时 no-op。
- **BUILD-306**：Step1–4 全走；源码 mtime 前后 md5 一致（`2ed2b891…`）。产物 main `bc043e25…`（14,491,648B/13:04，较 302 **+13,824B**）/ ui `467232cd…`（10,060,288B/13:04）/ crash `2d05df22…`（24,879,104B/13:02）；两副本相等；main 异于 BUILD-302（`5a318429…`）。八项逐项 PASS（①时间戳 ②sha ③0.9.2 ④冒烟 PID 27244 Responding=True/无新 crash.json/残 0 ⑤config `da2be5da…`+wordbook `b6ab43ac…` 零变化 ⑥110/9/17 ⑦探针 ⑧toml 三副本全等）。
- **探针**：`[LocalRT-DBG-298]`=4 / `291`=1 / `293`=1 / `292`=2 / `284`=3 / `289`=1 / `276`=1 / `277`=1 / `278`=2（与 302 一致）；`283`=0 既定预期；🆕 **`然后`=6、`えー`=1**（303 口水词字面量进包）；反例符号名 `apply_filler_strip`/`strip_fillers_conservative`=0（release 内联，不可作探针）。
- **端测六类交 Gavin**：① 中间句尾字 ② 首字三场景**分开测** ③ 长语音看 `[LocalRT-DBG-298] join:` 的 `tail_wait` ④ 本地流式+翻译 ⑤ 切片粒度 A/B/C（默认 / `LOCAL_RT_ACC_MIN_SEG_MS=15000` / `LOCAL_RT_ACC_PARALLEL=0`）⑥ **口水词过滤**（关 LLM 说「呃，然后然后我们开始吧」→ 期望「我们开始吧」；开 LLM 行为不变）。
- **证据**：`collab/outbox/tester-1/testexec304/`。
- **红线**：版本 0.9.2 未动 / 未 commit / 未 `cargo clean` / 未破坏性 git / 零凭证。

## 2026-09-21 — tester-1 — TEST-EXEC + BUILD-302 ✅ 复跑全绿 + 出包（八项 PASS，301 首跑 4 红已收口）

- **基线**：HEAD `90acbf1`（301-B），clean、`ui/` 零 diff。
- **首跑 `b08c376`（301 原版）仍 4 FAIL**：301 只修好 `nospeech_122_guard_tests::h4`；`guard_214_215::g7/g9/g10/g11` 仍红——**301 漏了 `src/punctuation/mod.rs:718` 自带的 `prod_lines`**（扫 `main.rs`，仍被 `main.rs:9165` 的 `mod parallel_acc_298_tests` 截断，锚点 `:9503/9265/9391/9478` 落区外）。如实上报 → 主控修 `301-B`。
- **复跑（`90acbf1`）全绿**：root **1304P/0F/15I**（crash 52P/2I + bin 1216P/0F/13I + integration 36P）；`src-tauri` **85P/0F/0I**；Vitest **SKIP**（`ui/` 零 diff）。
- **🔴 逐条点名**（不只看总数）：上轮 5 条假红 **5/5 恢复 ok**（`h4` + `g7/g9/g10/g11`）；298 六条新测全绿；`1304−1299=+5` = 假红恢复，无新增测试。
- **在线 / PARALLEL=0**（沿用 300 取证）：在线 `main.rs:7845` `initial_text=Some`+`pretranscribed=None` ⇒ 新分支 `:9279` 结构不可达；`LOCAL_RT_ACC_PARALLEL=0` ⇒ 两派发口 false ⇒ `pretranscribed=None` ⇒ 原 accuracy 2pass，代码级等价串行。
- **BUILD-302**：Step1–4 全走；源码 mtime 前后 md5 一致（`e0656ba4…`）。产物 main `5a318429…`（14,477,824B/12:02，较 296 **+58,368B**）/ ui `1cb71954…`（10,060,288B）/ crash `4844bd21…`（24,879,104B）；两副本相等；main 异于 BUILD-296。八项逐项 PASS（①时间戳 ②sha ③0.9.2 ④冒烟 PID 22620 Responding=True/无 crash.json/残 0 ⑤config `da2be5da…`+wordbook 零变化 ⑥110/9/17 ⑦探针 `[LocalRT-DBG-298]`=4 等 ⑧toml 三副本全等）。
- **探针**：`[LocalRT-DBG-298]`=4（进包）/ `291`=1 / `293`=1 / `292`=2 / `284`=3 / `289`=1 / `276`=1 / `277`=1 / `278`=2；`283`=0 既定预期（293-B 改名）。
- **端测四类交 Gavin**：291 中间句尾字／293-B 三场景**分开测**首字／298 长语音看 `[LocalRT-DBG-298] join:` 的 **`tail_wait`**／本地流式+翻译。应急开关 `LOCAL_RT_ACC_PARALLEL=0`（+ `_SILENCE_MS` 800 / `_MIN_SEG_MS` 3000）。
- **证据**：`collab/outbox/tester-1/testexec302/`。
- **红线**：版本 0.9.2 未动 / 未 commit / 未 `cargo clean` / 未破坏性 git / 零凭证。

## 2026-09-21 — tester-1 — TEST-EXEC-300 ⚠️ 298 回归 5 FAIL（全为测试侧扫描边界假红，非生产回归）

- **基线**：HEAD `9133c32`（298），`ui/` 零 diff，工作区仅多**未跟踪** `src/bin/poc_acc_context.rs`（coder-1 在途，未碰、未删）。
- **回归**：root `cargo test --no-fail-fast` = **1299P/5F/15I**；`src-tauri` **85P/0F/0I**；Vitest **SKIP**（`ui/` 零 diff）。`poc_acc_context` **0P/0F 编译通过**（不构成阻断）。
- **新测**：`#[test]` 新增 **6**（非任务书 12；「12」= 断言数）：`parallel_acc_298_tests::{assemble_*×2, acc_parallel_result_usable_gate}` + `local_stream::tests::acc_should_dispatch_{requires_all_three_conditions,honors_env_thresholds,tail_requires_speech}`。NEW 6 / GONE 0，净 +1 自洽。
- **🔴 5 FAIL 根因（沙箱复算证明，非推测）**：`nospeech_122_guard_tests::h4` + `guard_214_215::g7/g9/g10/g11` 均从 `include_str!("main.rs")` 取生产区，切点=「**首个 `#[cfg(test)]`**」（`punctuation/mod.rs:720`、`main.rs:13074`）。298 在 **`main.rs:9165`** 插入 `#[cfg(test)] mod parallel_acc_298_tests`（`run_pipeline_core` 定义 `:9206` 之前）⇒ 扫描区截到 **9164 行**；5 条锚点全在 **9265–9503**（`:9503 apply_l2_postprocess` / `:9265 from_online_streaming` / `:9391 pre_llm_text` / `:9478 normalize_unit_symbols_only` / `:9266 transcription_result`）⇒ 全部 panic「锚点不存在」。锚点在全文逐字存在、语义未变（G7 两实参齐、G9 `from_online_streaming` 非注释恰 2、G10/G11 分支结构未动）⇒ **修好边界应全绿**。
- **同族与隐患**：与上轮 `guard291_g3`（294 护栏区域定界）同族；295 的 coder-2 已用 `mod tests {` 规避同坑，main.rs 这几组未规避。⚠️ 附**自发现在内**：main.rs 生产区扫描自 298 起被静默收窄到 9164 行（锚点 <9165 的护栏暂未暴露）。**建议**（交主控裁定）：切点改稳定上界标记或显式 `// ==PRODUCTION-END==`，或作者把新测试模块移到文件尾。
- **Q1 `LOCAL_RT_ACC_PARALLEL=0`**：`from_env`⇒enabled=false；`should_dispatch_acc` 首行短路 / `should_dispatch_tail` 首条件（均有单测）；`main.rs:7946 acc_enabled=false`⇒`:7960 acc_handle=None`⇒`:8063 acc_result=None`⇒`acc_parallel_result_usable(c, "")=false`⇒`pretranscribed=None`⇒`:9279` 新分支不进、走原 accuracy 2pass。**代码级等价串行**；🔴 本机无法触发录音，**非运行时逐样本对照**。
- **Q2 在线三档**：在线调用 `main.rs:7845` 传 `initial_text=Some(streaming_text)` + `pretranscribed=None`（注释「在线路径无并行预转写」）⇒ 新 `else if pretranscribed`（`:9279`）**结构不可达**；批处理 `:8278` 双 None。`transcription/mod.rs` 改动**纯新增**（`SendOfflineRecognizerRef`/`transcribe_accuracy_segment`/`offline_recognizer()`）。**运行时覆盖=0**，不以「没红」当「没影响」。
- **证据**：`collab/outbox/tester-1/testexec300/{root_test.log,tauri_test.log}`。
- **红线**：未出包 / 版本 0.9.2 未动 / 未 `cargo clean` / 未改生产代码 / 未碰 `poc_acc_context.rs` / 零凭证。

## 2026-09-21 — tester-1 — TEST-EXEC-296 + BUILD-296 ✅ 回归全绿 + 出包（八项 PASS + 294 假红定位）

- **基线**：HEAD `a0334b9`（含 coder-1 `FIX-GUARD-297`），工作区 clean、`ui/` 零 diff。
- **回归**：root **1298P/0F/15I**（EXIT 0；基线 1284P，**+17 新测 −3 改名 = 净 +14**）；`src-tauri` **85P/0F/0I**；Vitest **SKIP**（`ui/` 零 diff，酌情原则）。
- **🔴 首跑即红 → 定位护栏缺陷（本单最大产出）**：`099ca7f` 树上 `guard291_g3` FAIL（endpoint 块 `recognizer.create_stream()` 实测 0、期望 1）。用与护栏同构的花括号游标复算：`endpoint_guard_regions()` 的「首个 `} else`」截断切在嵌套 `let confirm_text = if use_shadow {…} else {…}`（`:400`），扫描区截到 `:399`，真 `create_stream()`（`:450`）在区外 ⇒ **假红、生产代码正确**。报主控→裁定 coder-1 修（`FIX-GUARD-297`，改用 `endpoint_branch_bounds` 单遍游标），终版 g1~g4 全绿。
- **消融**：仅 295/audio **I5** 三种改法（主控缩减令取消 291 四条，理由「给测试做测试」）全部 **RED** 且即时还原（`git diff src/audio/mod.rs` 空）：①调用移出 `trim_pre_roll_residual` 分支→`块 L638..646，调用 L637` 红；②`else`（在线）分支加第二调用→命中 3（期望 2）红；③删调用→命中 1（期望 2）红。
- **在线三档零回归（正面取证）**：调用点隔离 `main.rs:7716` 在线=`false` / `main.rs:7965` 本地=`true`；现有测试**不执行** `record_streaming`（需音频设备）⇒ **在线调用路径零覆盖**（如实声明，不以「没红」当「没影响」）；分流纯函数 `pre_roll_ms_for_streaming(false)==600` 由 `pre_roll_streaming_ms_routes_by_existing_switch` 覆盖，唯一调用点+门控由 I5 护栏钉住。
- **BUILD-296（Step1–4 全走）**：npm 1.67s（`index-CipQtFxc.js`）/ Tauri 1m53s（17w）/ 主程序 2m57s（110w + crash 9w）；🔴 源码 mtime 前后 md5 一致（`6593df97…`，无中途改动）。产物 main `117d0411…`（14,419,456B/11:05）/ ui `66b006e3…`（10,060,288B/11:05）/ crash `f013357a…`（24,879,104B/11:03）；两副本逐一相等；main 异于 BUILD-290（`835d402b…`）。
- **八项核验逐项 PASS**：①时间戳 ②两副本 sha+异于上包 ③0.9.2（`0.9.2.0`/`0.9.2`/`0.9.2.0`）④冒烟 PID 20432 Responding=True/无新 crash.json/残留 0 ⑤config `da2be5da…`+wordbook `b6ab43ac…` 零变化 ⑥warnings 110/9/17=基线 ⑦字面量探针 ⑧toml 三副本全等。
- **探针**：`[LocalRT-DBG-291]`=1 / `293`=1 / `292`=2 / `284`=3 / `289`=1 / `276`=1 / `277`=1 / `278`=2；`283`=0 属**预期**（293-B `1b49bb3` 把 283 日志字面量改名为 `293`，并删 `trim_pre_roll_*` 3 测）；`chunks dropped during THIS recording`=2。
- **端测 5 项交 Gavin**（本机 LL 钩子不接受注入合成 Right-Alt，无法真人发声）：①连说 4~5 句每句停 >2s 验中间句尾字（291）②先开口再按键（293-A）③按键后立刻说（293-B）④按键后停 2s 再说（293-B）⑤本地流式+开翻译；**三条 293 必须分开测**；带 `-debug` 跑会落 `debug-audio/` pre-roll WAV。
- **证据**：`collab/outbox/tester-1/testexec296/`（root/tauri 最终日志、build 日志、`abl_i5_1..3.log`）。
- **红线**：版本 0.9.2 未动 / 未 commit / 未 `cargo clean` / 未破坏性 git / 零凭证 / `debug-audio/` 未入 git。

## 2026-09-21 — coder-2 — FIX-SHADOW-DISPLAY-289 ✅ 交付（endpoint 确认改用当前句最完整结果）

- **根因**：影子解码正常（finalize 有结果），但 endpoint 固定用 main 的 `r.text` confirm ⇒ main 缺尾字 ⇒ 影子完整显示被回退（尾字不显示 / 等下一句才出）。
- **修法**：endpoint 确认「当前句最完整结果」——`use_shadow = endpoint && shadow.chars().count() > main.chars().count()`；影子空/更短/非 endpoint 用 main。`shadow_current` 空结果置 None 防陈旧。显示仍取更长者（不闪回）。
- **诊断**：`[LocalRT-DBG-289] endpoint confirm: main_len/shadow_len/used`（debug! + `log_enabled!` 守卫，沿用 284）。
- **验证**：cargo check --all-targets 0 error、warnings **110/101** = 基线；rustfmt clean；`--numstat`==`-w`（29/6）。🔴 实机时序取证（shadow→endpoint→confirm 顺序与文本）交 tester-1/Gavin（我不能跑）。
- **红线**：只改本地流式 / 在线档一行未动 / RULE1/2/3 未动 / 未动版本 / 未 commit / 未出包 / 零凭证。

## 2026-09-21 — tester-1 — TEST-EXEC-274 + BUILD-274 ✅ 回归全绿 + 出包（八项 PASS，⏸ 272 延迟待录音）

- **回归**：root **1279P/0F/15I** + `src-tauri` **85P/0F** + Vitest **100P/0F/11S**；warnings root 110/101、src-tauri check 17 = 基线。
- **重点验 ①**（271 未改坏 accuracy，独立对照）：`poc_funasr_nano --threads 0`(旧=单线程) vs `--threads 8`(新) 同音频 —— `far_3` 文本**逐字相同** 7.308→4.004s；`rag_physics` **逐字相同** 2.134→1.038s。
- **重点验 ②**（269-B 未动切句）：`LOCAL_STREAM_RULE1/2/3 = 2.4/2.0/20.0` 跨 `277b2fc`→`ace0786` **逐字相同**；269-B 对 `reset()`/`sentence_id`/`endpoint` 零新增（仅注释+显示层独立静默计数）。🔴 行为级「句中停 1s 不切句」需真人发声——`transcribe_streaming_local` 无测试缝、`poc_local_stream` 用自己的 rule2=1.2 不代表生产 ⇒ 交 Gavin 端测（未造假）。
- **重点验 ③**（标点不重复）：结构上 `on_result` 恒喂 `r.text` 裸文本、`display_text()` 恒裸、标点仅 `preview_display` 作用于显示不回灌；间接实测流式 paraformer 原始输出无标点。🔴 视觉级留端测。
- **重点验 ④**（生成未饿死）：`far_3` 21.8s +71 条 / +20 条、自建 27.3s +71 条 / 无热词 —— 全部**完整、末句「。」、无 `Truncating/Falling/Reduce`**。
- **BUILD-274**：Step1–4 全走（npm 797ms / Tauri UI 112s / 主程序 165s）；三 exe + 两 toml 同步 `Publish/` 与 `target/release/`。八项逐项 PASS：①时间戳 main 00:07:31 / ui 00:04:40 / crash 00:05:52 ②两副本 sha 相等且三者异于 BUILD-267（main `40ccd5ea…` / ui `c788acf0…` / crash `21cb7d8a…`）③ProductVersion 0.9.2 不变（Gavin 未升版，按 sha 替代判据）④冒烟 Responding=True + 无 crash.json + panic/ERROR 0 + 残留 0 ⑤`config.toml` `da2be5da…` / `wordbook.sqlite` `b6ab43ac…` 零变化 ⑥warnings 110/101/17 ⑦探针 4 串命中 ⑧scene `8ea93bb1…` / itn `311cbb96…` 全等。
- **额外**：`Publish/models/.../llm.int8.onnx` = 600,025,528（1024，`c326cdeb…`）；`.512.bak` 保留未删、无 zip/setup 产物。⚠️ 主程序 +1.9MB（268 引入 `tokenizers` crate，预期）。
- **⏸ 未执行**：**272 首字延迟实测**（需真人发声，无法执行；前置已备：两处新产物 + `target/release/debug.log`，等 Gavin 配合录音）。
- **证据**：`collab/outbox/tester-1/testexec274/` + `build274/`。
- **红线**：版本 0.9.2 未动 / 未 commit / 未改生产代码 / 备份未删 / 零凭证。

## 2026-09-21 — tester-1 — BUILD-280 ✅ 诊断包（只读埋点 276/277，无新修复）

- **用途**：给 Gavin 真机跑一次收 LocalRealtime 诊断数据（coder-1/coder-2 均无法发声）；HEAD `5ad0670`，版本维持 0.9.2。
- **构建**：Step1–4 全走；`target/release` 与 `Publish/` 三 exe + 两 toml 均更新；warnings bin 110 / crash 9 / tauri 17 = 基线。
- **产物**：main `33b65b3f626da7b71563c180b6e01f96774f9c7ed77de999d26ee1b6352016aa`（14,360,576B / 00:39:54）/ ui `486dc68ccfa4a1ae5848ec3c3fcf08e7e172c424d154f06ff54e32d6c8f34932` / crash `807bfb17568d01cc615d75f2791f6a6565f932db4c2628b4a3e3518051cd5a2c`；两副本逐一相等，三者均异于 BUILD-274。
- **🔴 埋点探针**（`grep -a -F` 于 `target/release/feiyin-ime.exe`）：`[LocalRT-DBG-276]`=**1**、`[LocalRT-DBG-277]`=**1** ⇒ 埋点确已进包。
- **冒烟**：PID 4188 `Responding=True`；`target/release/debug.log` 生成（189,574B / 1562 行）；panic/ERROR=0；无 `crash.json`；残留 0。（本次无麦克风输入，`LocalRT-DBG` 行 0，需 Gavin 录音触发。）
- **278（coder-1 流式首字/pre_roll）未落地**，按令不等，本包只含 276/277。
- **证据**：`collab/outbox/tester-1/build280/`（build280 / build_* / verify_smoke）。
- **红线**：版本 0.9.2 未动 / 未 commit / 未改生产代码 / 模型与备份未碰 / 零凭证。

## 2026-09-21 — tester-1 — BUILD-285 🛑 中止（主控停令），产物作废；回归有效

- **停令**：诊断埋点在 release 下仍求值+格式化（DBG-278 遍历 pre_roll）→ URGENT-286 降 `debug!`+`log_enabled!`；**已开始则中止重来**。停令时 Step1–4 已跑完（01:04–01:08），按令中止、**不交付**。
- **作废**：main `ca2365f6…` / ui `59198f5c…` / crash `e0713a12…`；源码 mtime 01:08:12–26 落构建中途 ⇒ 混合快照。🔴 `Publish/` 与 `target/release/` 暂存该作废包，**Gavin 勿用**（重出会覆盖）。
- **回归（复用）**：root **1282P/0F/15I**（1279+3 条 283 `trim_pre_roll_*`）/ `src-tauri` **85P/0F/0I** / Vitest **100P/0F/11S**；release warnings bin 110 / crash 9 / tauri 17 = 基线。
- **探针教训**：`StreamingFinalPreview` / `trim_pre_roll`（符号名）release `grep -F`=0，不可作探针；用 `.rdata` 日志字面量 `[LocalRT-DBG-276/277/278/283]`（实测 1/1/2/1）。下包改用此口径。
- **四条重点验需真人录音**（②需长文撑窗触发 `[LocalRT-DBG-277]`；④需两语音时序），我无发声能力，未产数据，留 Gavin。
- **证据**：`collab/outbox/tester-1/testexec285/` + `build285/`。
- **红线**：版本 0.9.2 未动 / 未 commit / 未改生产代码 / 备份未删 / 零凭证。

## 2026-09-21 — tester-1 — TEST-EXEC + BUILD-287 ✅ 回归全绿 + 出包（八项 PASS + 日志开关两头验）

- **构建纪律**：开工前后源码 mtime 逐位一致（`01:08:12–01:11:06`）⇒ **无中途改动**，吸取 285 作废教训。
- **回归（286 后重跑）**：root **1283P/0F/15I**（+1 条 286 `urgent286_pre_roll_diag_quiet_at_warn_full_at_debug`）/ `src-tauri` **85P/0F/0I** / Vitest **100P/0F/11S**；warnings **110/101/17** = 基线。
- **产物**：main `e16e3738b2f7f979a49a7209438f5c44f77ebfc3a1db26b5bf32573b4aeb5955`（14,379,008B / 01:17:00）/ ui `ee6d6e73486e2e611da5a8e52805cdce935bf52545cf484438f8319b1776025d` / crash `a4b238aa11c9b89e5ba3957cd8674204ad5765fdb3cf5c40e1c22a88d5007aed`；两副本相等，均异于 BUILD-280。
- **八项逐项 PASS**：①时间戳 01:14–01:17 ②两副本 sha 相等且异于上包 ③ProductVersion 0.9.2 ④冒烟 Responding=True + 无 crash.json + 残留 0 ⑤config/wordbook 零变化 ⑥warnings 110/101/17 ⑦探针 ⑧toml 三副本全等。
- **探针（字面量）**：`[LocalRT-DBG-276/277/278/283/284]` = 1/1/2/1/3；284 功能性字面量 `LOCAL_RT_SHADOW_MS` = 1（非诊断标签，证 284 进包）。
- **🔴 日志开关两头自验**：不带 `-debug` → 无 debug.log、DBG 0 条；带 `-debug` → debug.log 1848 行且 `[LocalRT-DBG-284]` 实写；静态守卫 `pre_roll_diag` 首行 `log_enabled!` 早退。诚实边界：LL 钩子（vk=165）不接受注入合成 Right-Alt，运行时未触发录音路径。
- **四条修复效果**（281 右留白 / 282 收尾预览 / 283 两场景 / 284 影子解码）仍需 Gavin 真人录音。
- **证据**：`collab/outbox/tester-1/testexec287/` + `build287/`。
- **红线**：版本 0.9.2 未动 / 未 commit / 未改生产代码 / 备份未删 / 零凭证。

## 2026-09-21 — tester-1 — BUILD-290 🛑 阻塞未构建（工作区不 clean，288 未提交）

- **任务前提不符**：任务书称「工作区 clean / HEAD `17875b3` / 288 下包 / 构建期冻结」；实测 **` M src/audio/mod.rs`（未提交 +122/−23）= 288 全部实现**（含新测 `asr_drop_288_throttle_and_delta_semantics`，mtime 01:40:45）。
- **处置**：**未构建**（避免越界含 288 / 混合快照），已报主控三选一：A 提交 288 后构建 / B coder-1 撤下 288 恢复 17875b3 / C 主控确认 288 即本包。
- **现状**：未动任何文件；`Publish/` 与 `target/release/` 仍为 BUILD-287 产物（sha main `e16e3738…` / ui `ee6d6e73…` / crash `a4b238aa…`）。
- **只读取证**（脏树 289+288）：root 1284P/0F/15I、warnings 110/101、EXIT 0，仅证可编译、**不作验收**。
- **证据**：`collab/outbox/tester-1/testexec290/`（脏树回归日志）。
- **红线**：未构建 / 未 commit / 未改生产代码 / 未执行破坏性 git / 备份未删 / 零凭证。

## 2026-09-21 — coder-1 — FIX-ASR-DROP-288 完成（解除 tester-1 BUILD-290 阻塞点）

- **背景**：tester-1 报构建期工作区含未提交 `M src/audio/mod.rs`（288 实现，mtime 01:40:45）→ 暂停 BUILD-290 待主控裁定。**288 现已完成**（含单测/实测/文档），可由主控裁定 A（提交）或 C（即本包）。
- **改动**：`src/audio/mod.rs` 单文件——回调 3 处删 `log::warn!`（仅 `fetch_add`）；新增消费端 `report_recording_drops_throttled()`；`collect_recording` 新增 `dropped_chunks: &AtomicU64` 参数（5 调用点）；`record_streaming`/`collect_recording` 汇总改用**录音前后差值**（修掉「全局累计当本次」误导）。
- **验证**：cargo check 0 error / 110 101 / rustfmt clean；`audio::tests` **62/62**（含 `asr_drop_288_throttle_and_delta_semantics`）；**实测空闲 15s `[ASR-DROP]`=0 条**（改前 3659/40s）。
- **红线**：丢弃逻辑/队列容量 256 未动；实时回调未新增锁/分配/格式化；在线档与本地档行为不变；未动版本；零凭证。

## 2026-09-21 — tester-1 — TEST-EXEC + BUILD-290 ✅ 288+289 出包（八项 PASS + 288 空闲零日志 + 日志开关两头验）

- **HEAD**：`ad74b64`（288 已提交）；构建期源码 mtime 前后一致（静止）✅。
- **回归（clean 树）**：root **1284P/0F/15I**（=1283+288 新测）/ `src-tauri` **85P/0F/0I** / Vitest **100P/0F/11S**；warnings **110/101/17** = 基线。
- **产物**：main `835d402b26374c194272c19c647b8f74d2c9786b763c0d3a2641e7e43cc99623`（14,384,128B/01:47:50）/ ui `0454b288697f729279528c45126101b845d98680062f52ec1182780739ae480a` / crash `ae914a088bd45e710b25d17a57a1b4d2342a374eac4cfd859954fce2d8e8bef1`；两副本相等，均异于 BUILD-287。
- **八项逐项 PASS**：①时间戳 01:45–01:47 ②sha ③0.9.2 ④冒烟 Responding+无 crash.json+panic/ERROR 0+残留 0 ⑤config/wordbook 零变化 ⑥warnings 110/101/17 ⑦探针 ⑧toml 三副本全等。
- **探针（字面量）**：`[LocalRT-DBG-289]`=1（必须命中）；`chunks dropped during THIS recording`=2（**288 功能性字面量**）；`[LocalRT-DBG-284]`=3 / 276=1 / 277=1 / 278=2 / 283=1。
- **🔴 288 核心自验**：`-debug` 静置 **150s** 不录音 → `[ASR-DROP]` **0 条**、debug.log 24 行、panic/ERROR 0、残留 0（改前空闲即刷 warn）。
- **日志开关两头验**：不带 `-debug` → 无 debug.log、DBG 0；带 `-debug` → 生成且含 DEBUG 级数据；临时切 local_realtime 演示 `[LocalRT-DBG-284]` 实写后**原样还原** config.toml（sha 一致）。
- **诚实边界**：本机 LL 钩子不接受注入 Right-Alt → 运行时未触发录音路径（276/277/278/283/289 由探针+单测坐实）；尾字与四条修复效果待 Gavin 录音。
- **证据**：`collab/outbox/tester-1/testexec290/` + `build290/`。
- **红线**：版本 0.9.2 未动 / 未 commit / 未改生产代码 / 备份未删 / 零凭证。

## 2026-09-21 — 主控 — ORCH-CTX-GUARD-FIX-324 ✅ 护栏收窄 + 埋点修正 + 清理指令恒发

- **背景**：BUILD-321 端测日志分析（`collab/evidence/20260921-build321-e2e/debug-build321.log`，11 次录音）。
- **改动**：`src/transcription/mod.rs` 单文件，53+/19-。① 护栏 LCS 比对面由整个 system 收窄为仅 `ctx`，并加无前文跳过门；② `out_chars` 改记判决当时长度、新增 `final_chars`；③ 新增 `CLEANUP_INSTR_EN` 恒发，`build_ctx_system` 契约改为恒返回 `Some`，`CTX_INSTR_EN` 拆出仅在有前文时追加。
- **验证**：fmt clean / check 0 error / warnings **101** = 基线 / `ctx320` **6/6**。
- **红线**：未动版本号 / 未出包 / 未碰 coder-1（`src/audio/mod.rs`）与 coder-2（`src/itn.rs`、`itn-rules.toml`）在飞文件 / 零凭证。
- **待端测判据**：`action=redecode` 频次应显著低于改前的 43%（6/14）。
