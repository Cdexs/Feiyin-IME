# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

## 2026-09-22 — coder-1 — POC-QWEN3-1.7B-351 ✅ 交付（1.7B 实测：不换；发现输出前缀硬不兼容）

- **任务**：填 347 留的两个空白（本机 CPU RTF + 1.7B 中文 CER），纯 PoC 零生产逻辑。
- **前置重构**（主控批准）：`create_qwen3_recognizer_at` **零行为变更提取**，独立 commit **`e829c67`**（+12/-0，函数体原样搬入、原函数缩为委托、签名/调用点不变、不动硬编码目录名）。
- **PoC**：commit **`f335bd8`**（`src/transcription/mod.rs` `#[cfg(test)] mod poc_qwen3_17b_351`，+271/-0）；复用生产构造链 + `transcribe_acc_ctx`，不手搓配置；机器 16 核 / acc_threads=8 / full.wav 56.15s / 切片与 313 逐字相同。
- **读数**（原始日志 `collab/evidence/20260922-poc-qwen3-17b-351/poc-351-raw.log`）：0.6B `peak_priv=2020MB rtf=0.218 cer=0.0356`；1.7B `peak_priv=4967MB rtf=0.324 cer=0.1556(含前缀)`；**剥前缀后 1.7B cer=0.0444**。
- 🔴 **控制前缀（推翻 347 结论）**：1.7B 输出含 `language chinese<asr_text>`，生产 `strip_asr_special_tokens` 只剥 `<|…|>` ⇒ 漏进用户文本；两模型 tokenizer 逐字节相同 ⇒ 是 ONNX 图/元数据不兼容。**1.7B 非 drop-in。**
- **结论：不换**——①CER 未优反劣（0.0444>0.0356）②RTF 1.36–1.49× ③**分片态**峰值 4.97GB 越界（> DEC-076 否决的 4.3GB）。
- **方法教训**：按 decoder 权重比 2.70× 外推耗时 → 实测 1.36–1.49×，高估 1.8–2.0×；选型须实测。
- **边界**：CER n=1（单条 56s），非统计结论；`andrewleech/...-onnx` 未用（主控已证不兼容）。
- **红线**：未改版本 / 未 push / PoC 不夹带产品决策 / 未碰 local_stream.rs / 零凭证。

## 2026-09-22 — coder-1 — PUNCT-FINAL-REDO-350 ✅ 交付（标点剥离独立节点，只挂本地 realtime）

- **需求（Gavin 2026-09-22）**：最终 acc 转写完成后剥光整段已有标点、再交标点模型整段重打；**只针对本地 realtime，其他管线不能动**（在线 ASR 标点可能更准）。设计细化：剥离做成**独立节点**、谁要谁挂，不绑死管线。主控前两稿 v1（四档一视同仁）/v2（`run_pipeline_core` 加 Fill/RedoWhole 模式参数）均作废，**v3 为准**。
- **实现**：`src/punctuation/mod.rs` 新增共享谓词 `is_effective_punctuation`（`has_effective_punctuation` 改调它、行为不变）+ 严格对偶纯函数 `strip_effective_punctuation`；`src/main.rs` 新增自由函数节点 `strip_punctuation_node`（形态照 `apply_filler_strip`），接线在本地 realtime 自有编排块（normalize 之后、`native_punctuated` 之前）⇒ 剥光后 `native_punctuated` 恒 false ⇒ 下游既有门自动放行整段重打。
- 🔴 **DEC-066 证据（共享代码零 diff）**：`run_pipeline_core` 签名**不在 diff**；`apply_local_punctuation` 与 HEAD **md5 逐字节相同**（`8094f722…`）；v1/v2 期间动过的共享代码（`native_punctuated` 参数、`pretranscribed_native_punctuated`、334 测试名、`PunctuationNodeMode`/`PunctuationRunner`）**全部恢复原状**。其余两档结构上不可能受影响。
- **门 = `punctuation.enabled && !start.translate.load(...)`**：翻译路径下游 `apply_local_punctuation` 被挡不重打 ⇒ 整段跳过；`llm_handled` 接线点读不到，核实结论 = LLM 路径下剥光文本只是 LLM 输入（LLM 自带标点输出）、LLM 失败回落 CT 重打 ⇒ 无害。
- **单测 7 条**：穷举性质测试（`has(strip(t))==false`）、词内嵌边界外、334 重复标点回归、「剥光了必定打得回来」不变式。
- **验证**：fmt 全仓 clean / `check --all-targets` 0 error、warnings 99/90 / `--numstat`==`-w`（main 138/2、punctuation 126/10）/ 全量 `cargo test --no-fail-fast` **0 failed**。
- **未决边界（只报不改）**：并行 acc 不可用回落内部转录时不经本节点（原行为）。
- **红线**：未动版本号 / 未 commit / 未 push / 未碰 `src/transcription/local_stream.rs` / 零凭证。

## 2026-09-22 — coder-1 — RESEARCH-QWEN3-1.7B-347 ✅ 交付（纯调研，零生产代码）

- **任务**：Gavin 要求「上 HF 找下载率最高的 qwen3-asr 1.7b，评估替代 0.6B 可行性；1.7b 也找 onnx 版，最大化向现役 0.6B 推理框架兼容」。
- **结论**：**有条件可行**。产出 `collab/research/qwen3-asr-1.7b-eval-347.md`。
- **头号门禁**：官方（k2-fsa / csukuangfj）**无** 1.7B sherpa-onnx 四件套 —— GitHub release `asr-models` **全量 499 asset 仅 1 个 qwen = 0.6B**；k2-fsa 文档只文档化 0.6B；`csukuangfj` HF 767 repo 含 qwen 者 0；`csukuangfj2` 仅 0.6B 镜像。⚠️ `releases/expanded_assets/...?page=N` 不响应分页，枚举须走 assets API。
- **关键发现（本单价值）**：现役 0.6B 的**同一来源** ModelScope `zengshuishui/Qwen3-ASR-onnx` 同时含 **`model_1.7B/`**；本机 0.6B 三文件 sha256 与该仓库 `model_0.6B/` **逐字节相同** ⇒ 1.7B 与现役 0.6B 同作者/同导出脚本（`Wasser1462/Qwen3-ASR-onnx`）/同四件套布局，**`OfflineQwen3ASRModelConfig` 代码零改动**。HF `thieunv-asilla` / `ilmina` 为逐字节相同镜像；`solavr` 为 mixed INT8/FP32（Q/K/V/O 保 FP32 防复读）独立导出。
- **数据**：1.7B int8 四件套 = 2,399,761,248 B（decoder 2,037,458,645 / encoder 314,222,162 / conv 48,080,441），+1.32 GiB；KV 每 token 与 0.6B 相同（28 层/8 KV 头/128，4096 → 896 MiB/流，cache 为动态轴无需重导）；官方 Offline 平均 WER 2.69 vs 0.6B 3.48；Apache-2.0。
- **🔴 诚实边界**：本机 CPU RTF **未查到**（唯一公开点 solavr RTF 0.743@2线程，CPU 型号缺失且为更重 mixed 导出）；1.7B 中文 CER 未查到；内存为以 0.6B 实测 4298MB 外推的**估算**。
- **建议**：不直接换默认，先做同音频 A/B PoC（判据：CER 显著优于 0.0444 + 延迟可接受 + 峰值内存不越界）。
- **红线**：未下载模型 / 未改 `src/` / 未改版本号 / 未 commit / 未 push / 未碰 `local_stream.rs` / 零凭证。

## 2026-09-22 — tester-1 — TEST-EXEC + BUILD-345 ✅ 出包（P0 崩溃修复 + DEC-077 回滚 + 344-G；八项 PASS，三机制探针归零）

- **基线/归属**：HEAD `d87b8b4`，**源码 clean**，版本 0.9.2。本包替换作废的 `8758ca66`。🔴 开工前 `M src/main.rs`（纯注释 4+/2−）→ 上报主控提交 `d87b8b4` 后放行（脏树不自行 commit）。
- **回归（全量未过滤；`cargo fmt --check` 不带 `skip_children` EXIT 0）**：root `cargo test --no-fail-fast` **1419P/0F/22I**（NEW 6/GONE 3）；`src-tauri` **92P/0F/0I**；Vitest **7 files/100P/11S/0F**。
- **增量**：`charboundary344_{mid_char_raw_len_does_not_panic,degenerate_inputs_fall_back_to_raw}`（**P0 护栏**）/ `reflow_hole_344_{segment_streaming_text_is_char_safe,fill_by_streaming_not_a_hole,assembled_text_stays_continuous}`（G）/ `endpoint_confirm_text_takes_longest_of_two`；GONE `..._of_three`（307 回滚）+ `tailpad340_*`（340 回滚）。
- **BUILD-345**：Step1–4 全走；源码 mtime 前后 md5 一致（`93469b74…`）。产物 main `ce10c4b7…`（14,675,456B/00:59，−7,680B vs 作废 342）/ ui `1e3df6d4…` / crash `01378435…`；两副本相等；异于作废 342（`8758ca66…`）与已发 341（`68e4528b…`）。
- **八项逐项 PASS**：①时间戳 00:55–00:59 ②sha ③0.9.2（Cargo.toml + tauri.conf.json 未动）④🔴 删旧 crash.json 后冒烟 PID 25220 Responding=True、**`target/release/crash.json` 未创建**、残 0 ⑤config/wordbook 本窗口内零变化（`Publish/config` `da2be5da…`、`target/release/config` `2e0e60a5…`、`Publish/wordbook` `b6ab43ac…`、`target/release/wordbook` `7e5755cb…`）⑥**warnings 99/90/17 未下降**（与预期不符；被删代码零 warning，如实报）⑦四表三副本全等（itn `ab950ba4…`/scene `8ea93bb1…`/homophone `a5fd4a61…`/wordbook `ac9a72eee…`）⑧探针。
- **探针**：自检 `feiyin`=19；正向 `337`=4/`336`=1/`325`=1/`AUTOLEARN`=4/`degree_adverbs`=2/`nz_ratio`=2/`342`=2；反向 332 五符号全 0；🆕 **345 反向 `feed_tail_silence`=0 / `SHADOW_TAIL_PAD_MS`=0 / `FLUSH_TAIL_PAD_MS`=0** ⇒ DEC-077 回滚彻底。
- **P0 验证**：`charboundary344_mid_char_raw_len_does_not_panic` 全量回归通过（复现「raw_len 落中文字符中间」不 panic）+ 冒烟不崩；⚠️ 原现场（长口述）本机无麦，交 Gavin。
- **Gavin 端测五条**：① 🔴 长句一口气说到底，`[LocalRT-DBG-325] action=skipped-hole` 应基本消失（上版 7 片里 6 片是它），请贴 action 分布 ② 🔴 不得新增 `target/release/crash.json` ③ 停手尾字 1~3s 被 accuracy 补上 ④ D 复核 `silence<800ms` 派发 ⑤ 其余（重复标点/编辑不被冲/自学习/配置界面/首字「你→按」）有观察记一句。
- **证据**：`collab/outbox/tester-1/testexec345/`（P0 旧 crash 备份在 `testexec342/crash-20260922-0000.json`）。
- **红线**：版本 0.9.2 未动 / 未改生产代码 / 未 push / 零凭证。

## 2026-09-22 — tester-1 — TEST-EXEC + BUILD-342 ✅ 出包（八项 PASS）＋🔴 crash 产物待裁

- **基线**：HEAD `fbec727`，clean，版本 0.9.2。含 342（D OR 根因 / A 松手非取消 / F1+F3 假 endpoint 护栏 / padding / B 结论）。
- **回归（全量未过滤；`cargo fmt --check` 不带 `skip_children` EXIT 0）**：root `cargo test --no-fail-fast` **1416P/0F/22I**（NEW 8/GONE 1）；`src-tauri` **92P/0F/0I**；Vitest **100P/11S/0F**。
- **BUILD-342**：Step1–4 全走；源码 mtime 前后 md5 一致（`b07b91b3…`）。产物 main `8758ca66…`（14,683,136B/00:18，+4,608B）/ ui `94f2a97a…` / crash `a4e2672d…`；两副本相等；三者异于 BUILD-341。
- **八项逐项 PASS**：①时间戳 00:13–00:18 ②sha ③0.9.2 ④冒烟 PID 28292 Responding=True/无新 crash.json/残 0 ⑤config+wordbook 本次窗口内零变化 ⑥warnings 99/90/17 ⑦四表三副本全等 ⑧探针（自检 `feiyin`=19；正向 `[LocalRT-DBG-342]`=3/`337`=4/`336`=1/`325`=1/`AUTOLEARN`=4/`degree_adverbs`=2/`nz_ratio`=2；反向 5 符号全 0）。
- 🔴 **发现 1（待主控裁）**：`target/release/crash.json`（mtime **00:00:34**，早于本 build）报 `Panic: byte index 235 is not a char boundary; it is inside '斯'`（对含中文串按字节下标切片，栈顶 `core::str::slice_error_fail`）。**`fbec727` 未显式修**（`git show fbec727 | grep raw_len` 空）。候选根因：`local_stream.rs:315 raw_full[cache.raw_len..]`（只判长度未判字符边界；`raw_full` 在 shadow/main 间切换时 `raw_len` 可落中文字符中间）。已备份 `testexec342/crash-20260922-0000.json`；**未改生产代码**。
- **发现 2**：`target/release/wordbook.sqlite` 341→342 间被 app 写（`72dc9738…`→`9477887b…`）；本次窗口内三时点恒定，非本次构建所致。
- **量化**：冒烟 idle CPU 5s = **0.04%**（31.2ms/5020ms/16核）、WS 1791MB（🔴 idle ≠ 录音期，测不到 D 真实开销）；历史改前（fbec727 前）47 次非尾派发 **最小 silence=800ms（37×800 + 其余各 1，无一 <800）**；`join: total_decode/tail_wait` 历史与本机日志均未捕到 ⇒ **如实报无数据**。
- **Gavin 端测五条**：① 🔴 D：一口气连说 15s+，`[LocalRT-DBG-298] seg dispatch` 应出现 **silence<800ms** 的派发（改前一次都没有）② 🔴 尾字：说完停住不松手，`[LocalRT-DBG-325/337] reflow applied` 且 action 非 `skipped-cancel` ③ 句尾幻字 ④ 接缝重复（「也可以。」孤立片段）⑤ 其余照旧。
- **证据**：`collab/outbox/tester-1/testexec342/`（含 `crash-20260922-0000.json`）。
- **红线**：版本 0.9.2 未动 / 未改生产代码 / 未 push / 零凭证。

## LOCALRT-344（coder-2，2026-09-22）

- **背景**：BUILD-342 端测 `crash.json` 报 `byte index 235 is not a char boundary`；Gavin 要求主路径不得加拖累机制。
- **改动（仅 `src/transcription/local_stream.rs`）**：①P0 复核采纳 `is_char_boundary`（O(1)），抽 `punct_cache_reuse` + 2 条回归单测；②回滚 340 shadow 补静音 / 340 收尾补静音 / 307 整句全量重解码，删 `feed_tail_silence`+3 调用+2 常量（无死代码），`endpoint_confirm_text` 三方→两方；③护栏：tailpad340 顺序红线随机制移除（原位注释），G3 计数 2→1 同步断言。
- **字节切片清单**：全文件仅 `&raw[raw_len..]` 一处 `&str` 切片（已修），其余 `Vec<f32>`/`Vec<String>` 切片 N/A。
- **验证**：fmt clean / check 0 error / warnings **99/90**（未下降，如实报）/ numstat==-w / 全量 `cargo test` 0 failed（1328+52+…）。
- **红线**：未动版本号 / 未出包 / 未 commit / 零凭证。
- **追加 G（344-G）**：修复「回灌洞永久失效」—— `acc_text` 累积、失败/空片留洞 ⇒ `has_hole` 永不复位 ⇒ 一片失败后续回灌全废。`on_segment` 载荷加 `seg_streaming_text`（`last_display` 自上一片 `committed_len` 起的字符后缀，按 char 切片）；worker 对 `Err` 与「Ok 空」统一 `hole_fill_decision`：非空填空不记洞，真·无法填补才兜底 `has_hole`；`SkippedHole` 保留。单测 3 条。**顺带查**：短片空输出 = 该段音频本身无语音（shadow/streaming/accuracy 三方同时为空），非 padding/静音判据。改动 `src/transcription/local_stream.rs` + `src/main.rs`；全量 `cargo test` 0 failed（1331）。

## 2026-09-22 — coder-2 — ACC-DISPATCH-SILENCE-ONLY-346 ✅ 交付（派发规则只判静默 1200ms + 修共享计数器坑）

- **Gavin 原话**：「改成只判断静默1200ms，不按时长来切片」。
- **改动（仅 `src/transcription/local_stream.rs`，`numstat`==`-w` 206/133）**：①阈值 800→**1200ms**；②删长度支（原 800ms OR 累计 5s）—— 按 DEC-077 连根删 `ACC_MIN_SEGMENT_MS_DEFAULT` / `AccDispatchConfig.min_seg_ms` / `should_dispatch_acc` 的 `min_seg_ms` 参数与分支，无死代码；③🔴 **核心坑**：`silent_ms` 是唯一共享计数器，标点路径 800ms 打点后清零它 ⇒ 阈值抬到 1200 后静默支**结构上不可达**。修法 =**方案 A**：新增 acc 专用 `acc_silent_ms`，只在静音累加/语音归零两处同步，**不被标点清零**；调用点改读它。shadow(400)/标点(800) 行为逐位不变。未选 B（删清零会连带动 shadow 时序）。④订正 5 处过期注释（含 `:970` 错误的「本处独立计数」）。
- **核心判据（测试）**：`acc346_acc_counter_survives_punct_reset` —— 逐 chunk 模拟「标点 800ms 清零」后，**正证** acc 计数器仍能走到 1200 并派发；**反证** 误用 `silent_ms` 则不可达。另 `guard346_acc_counter_wiring` 源码级护栏钉死接线。
- **验证**：`cargo fmt --check`（不带 `skip_children`）EXIT 0 ｜ `cargo check` 0 error ｜ warnings **99** = 基线 ｜ `numstat`==`-w` ｜ 全量 `cargo test --no-fail-fast` **1420P/0F/22I**（NEW 3 / GONE 2）。
- **只报不改**：`src/main.rs:9570` 注释「要攒够 `min_seg_ms` 且静默 800ms 才派片」已过期（越界项，报主控）。
- 🔴 实机 `tail_wait` / 派发分布交 tester-1/Gavin，未声称已验证。未动版本号（0.9.3 主控已升）/ 未 commit / 未出包 / 未碰 `src/main.rs`、`src/audio/**` / 零凭证。

## 2026-09-22 — coder-2 — PUNCT-PREVIEW-SEMANTIC-349 ✅ 交付（预览标点打在句中 · 只认静默 1200ms）

- **Gavin 原话**：「流式预览...标点打在这个句子的中间，掐断了整个句意」；诉求「按语义停顿打标点」。
- **先证后改（真实引擎探针 + 实机日志）**：① 真实 CT-Transformer 对任意截断的半句**恒在末尾补「。」**（8~32 字七档全中）；② `punct_cache_reuse` 把该终止符作前缀保留、新字追加其后 ⇒ **落到句中**；③ `debug-build321.log` 单会话 53 次重打点，大量落在 10~34 字前缀（4s/800ms/shadow 三条与语义无关）。证据 `collab/evidence/20260922-punct349/`。
- **改动（仅 `local_stream.rs`，`numstat`==`-w` 92/49）**：`PUNCT_SILENCE_TRIGGER_MS` 800→**1200**；删 `PUNCT_REFRESH_INTERVAL`(4s 定时) + `preview_display` 的 `interval/due`；删 `shadow_fired` 对 force 的贡献；新判据纯函数 `should_repunctuate_preview`；删 `PunctPreviewCache.last_punct_at`（`has_new` 改长度比较）；收尾强制打点保留。**shadow/acc/endpoint 状态机零改动**。
- **验证**：`cargo fmt --check`（不带 `skip_children`）EXIT 0 / check 0 error / `local_stream.rs` 零 warning / 全量 `cargo test` 0 failed（feiyin-ime 1337P，NEW 2；探针已删）。
- 🔴 **归属**：全量树含 coder-1 在飞 350 未提交改动，总数 1420→1425（本单 2 + 350 3），**本单隔离 +2**。
- **残余**：1200ms 停顿处若其实没说完仍会出现「。」（语义边界固有代价，符合 Gavin 口径）。
- 🔴 实机交 tester-1/Gavin，未声称已验证。未动版本号 / 未 commit / 未出包 / 未碰 `src/main.rs`、`src/punctuation/mod.rs` / 零凭证。

## 2026-09-22 — coder-2 — TEST-SYNC-352 ✅ 交付（阶段三：给 350 补独立护栏，非作者）

- **性质**：阶段三，**只写测试、零生产代码改动**；命令白名单（DEC-048）仅 `cargo fmt` / `cargo check`，🔴 **未跑 `cargo test`**（首跑在阶段四 tester-1）。
- **产出 6 条（`sync352_` 前缀）**：`punctuation/mod.rs` +131 = ① `has_` 重构等价性（旧语义逐字重写进测试 + 41,371 串穷举比对 + 「strip 恒等 ⟺ has false」不变量）② 退化输入（对偶/幂等/恒等）③ UTF-8 多字节紧邻标点（防 344 P0 同族）；`main.rs` +102 = ④ 节点唯一挂载 + 门含 `!start.translate` ⑤ 剥离早于 `pretranscribed_native_punctuated`（源码级顺序）⑥ 门关逐字返回（同覆盖 translate 分支）。
- **坑（已规避）**：`let stripped = strip_punctuation_node(` 在作者测试里也出现 ⇒ 不能数全文件，改用生产独有首参 `normalized` 过滤；护栏字面量一律 `concat!` 拆开防自命中。
- **验证**：`cargo fmt --check` EXIT 0 ｜ `cargo check --all-targets` 0 error、warnings **99/90** = 基线 ｜ `numstat`==`-w`（102/0、131/0）。
- **静态复核**（保首跑通过）：挂载过滤后恰 1；剥离 `:8433` < native `:8442`，距 785B < 1500；门窗口含两 switch。
- **结论**：**未发现 350 真实缺陷**。未 commit / 未 push / 版本号未动 / 零凭证。
