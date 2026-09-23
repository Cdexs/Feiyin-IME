# 验收清单 · TRANS-NLLB-AND-SENTENCE-BATCH-394（coder-2）

> 规则（Gavin 2026-09-23）：派单前确认的方案，验收时逐条核对落地代码。任一条「偏离 / 缺失」⇒ 不提交。

## 1 模型

| # | 设计要求 | 结论 |
| --- | --- | --- |
| M1 | NLLB-200-distilled-600M CT2 int8 落 `models/nllb-200-distilled-600M-ct2-int8/`；result.md 记来源、大小、sha256 | ✅ mijuanlo 仓库，4 文件（model.bin 622,596,105B），sha 与 HF lfs.oid 吻合 |
| M2 | `model_files` / `is_available` / URL 改 NLLB；代码不出现 opus-mt 加载 | ✅ `mod.rs:28-31/860-871`；源码护栏 `source_guard_no_opus_mt_and_no_forced_min_decoding` |
| M3 | pub 签名不变；一模型双向共享，写明内存取舍 | ✅ `Arc<NllbModel>` + `with_direction`（`:122-128/789`）；签名未变 |
| M4 | opus-mt 目录不删 | ✅ |

## 2 调用规格

| # | 设计要求 | 结论 |
| --- | --- | --- |
| S1 | source = `[src_lang] + sp + ["</s>"]`，prefix = `[tgt_lang]`，batch 调 `translate_batch_with_target_prefix`，输出去首 token；依据官方文档 | ✅ `build_source_tokens :408`、`strip_target_prefix :417`、`translate_batch_with_prefix :243`；注释引 CT2 `guides/transformers` §NLLB |

## 3 分句 + 批量

| # | 设计要求 | 结论 |
| --- | --- | --- |
| P1 | 中文按 `。！？；……` 切，保留句尾标点 | ✅ `split_zh_sentences :542`（`……` 在第二个 `…` 处断） |
| P2 | 英文 `. ? !` 后接空白/结尾；避 Mr./3.14/U.S./e.g. | ✅ `split_en_sentences :565` + `ends_with_abbreviation :587`。⚠️ 缩写表含 `no` / `am` / `pm`：「The answer is no. We left.」「I am.」这类句末不会断句（两句并一句送翻，不丢字但变长）⇒ 返工 R3 |
| P3 | 过长（中 >80 字 / 英 >60 词）按 `，、` / `,;` 切子句，过短并邻，空丢弃 | ✅ `split_and_merge :614` |
| P4 | 一次 batch，分期上限 | ✅ `BATCH_MAX_SENTENCES=64`，`:725` |
| P5 | 拼接：中→英空格、英→中直连；保留换行 | ✅ `join_parts :478`；`translate` 逐行 `:808` |

## 4 解码参数

| # | 设计要求 | 结论 |
| --- | --- | --- |
| D1 | beam 4 / length_penalty 1.0 / no_repeat 3 / repetition 1.1 | ✅ `:46-52` |
| D2 | 删除 `min_decoding_length = 源/2` | ✅ `:743` 为 0 |
| D3 | max_decoding_length ≈ 源×2+16，≤256 | ✅ `:734`（按批内最长句） |

## 5 完整性（Gavin 追加硬要求）

| # | 设计要求 | 结论 |
| --- | --- | --- |
| F1 | 逐句一一对应，不 filter_map 丢句 | ✅ `:826-853` 每句必 push；结果数≠句数直接报错 `:758` |
| F2 | `looks_truncated` 阈值 0.35 / 0.6，空 ⇒ 漏译 | ✅ `:441` |
| F3 | 疑似漏译单句重译一次（beam 6 / lp 1.2），取更长，仍短 warn | ✅ `finalize_sentence :457`，`debug_assert!(retries<=1)` |
| F4 | 数字 / 专名保留检查只记日志 | ✅ `log_token_carry :654`（`log_enabled!` 守卫） |
| F5 | 真模型实跑：原句数==译句数、数字保留、逐句对照表 | ✅ 1P/0F/84.27s，5 段逐句对照。⚠️ 质量观察：「纹饰→embroidery」「讲解员说→According to historians」属词义错译（不属漏译），如实报 Gavin |

## 6 单测 / 边界

| # | 要求 | 结论 |
| --- | --- | --- |
| T1 | split / 拼接 / token 构造 / looks_truncated / 重译至多一次 / 不丢句 / 源码护栏 | ✅ 新增用例覆盖以上各项（split 5、finalize 3、looks_truncated、join、token、源码护栏等） |
| T2 | 删除的旧用例须属 opus-mt 专有 | ⚠️ 删除的旧用例除 1 条外均为 opus 专有（segment_text / 旧参数 / metaspace 归一化 / 双目录文件清单）。❌ **误删** `derive_target_japanese_kanji_returns_english_known_boundary`：它守的是翻译方向判定（DEC-082 刚确认保留自动判定），与换模型无关 ⇒ 返工 R2 |
| B1 | 只改 `translation/mod.rs`；main / transcription / vad / local_stream 未动 | ✅ |
| B2 | fmt / check / warnings / 全量 | ✅ 自证 fmt EXIT0、check 0 error、warnings 92/87（低于基线）、bin 1572P/40I |

## 7 🔴 运行时风险（result.md 结论不成立）

result.md：「`translator_destroy` 在测试进程 teardown 会挂死；**生产走 `process::exit` 跳过析构 ⇒ 不受影响**」。主控核实**不成立**：
- `main.rs` 生产退出**不走** `process::exit`（全文件无此调用；`fn main` 正常 `return Ok(())`）；退出序列 `worker_tx.send(Shutdown)` → `worker_join.join()`（`main.rs:9437-9445`）⇒ worker 线程返回时 **drop `cached_translation` ⇒ `translator_destroy`** ⇒ 若挂死 = **退出程序卡住**
- 运行中也会析构：关闭翻译 ⇒ `cached_translation = None`（`main.rs:8240/9137/9216` 三处）；配置方向与缓存不一致 ⇒ 重新 `load_for_direction` 并替换旧引擎 ⇒ 旧模型析构 ⇒ **录音线程卡死、输入法失灵**
⇒ ❌ 返工 R1（结构性保证运行期与退出期都不调用 `translator_destroy`）

**结论（主控 2026-09-23 第一轮）**：R1（阻断）+ R2 + R3 ⇒ 不提交，退回 coder-2 返工。

## 返工复验（TRANS-394-REWORK，主控 2026-09-24）

| # | 返工要求 | 结论 |
| --- | --- | --- |
| R1-a | 普通线程取证 drop 是否挂死 | ✅ A 组（加载→翻译→drop）30s 不返回、CPU 停涨 = **死锁**，进程退不出（exit 124）；B 组（加载→不翻译→drop）1.3~1.6s 返回 ×3 ⇒ 只在**推理过**的 translator 上死锁。`troubleshooting.md` 新增 `[CT2-DESTROY-DEADLOCK-394]`。⇒ 第一轮判定的「退出卡死 / 关翻译卡死」风险**属实**（opus-mt 时代同一 CT2 同样存在，只是没人在做过离线翻译后退出时注意到） |
| R1-b | 进程级缓存永不析构 | ✅ `thread_local! NLLB_MODEL` + `shared_model`（`mod.rs:142-168`）命中复用，否则 `Box::leak`；失败不缓存；`TranslationEngine.model: &'static NllbModel`；选 thread_local 理由（生产只在 worker 线程加载，免 unsafe Send/Sync）已写注释。已核 `main.rs` 4 处 `load_for_direction` 与 `ensure_translation_direction` 均在 `spawn_worker_thread` 线程内 |
| R1-c | 同指针、第二次 <50ms、无 mem::forget、源码护栏 | ✅ ptr::eq 同指针、第二次 0.005ms；`mem::forget` 全删、测试干净退出（exit 0，改前 124）；护栏：生产区 `NllbModel::new(` 恰 1 次 + `Box::leak` + `thread_local!`；「process::exit 规避」错误说法已删 |
| R2 | 恢复方向判定用例 | ✅ `derive_target_japanese_kanji_returns_english_known_boundary`（`:1119`）自 HEAD 原样恢复 |
| R3 | 缩写表 | ✅ 删 `am`/`pm`；`no` 仅后接数字算缩写（`ends_with_abbreviation :634`，新增 `next_nonspace` 参数）；补 2 条单测 |
| 复跑 | fmt / check | ✅ 主控复跑 `cargo fmt --check` EXIT 0（全仓）；`cargo check --all-targets` 0 error，warnings 92/87（低于 97/88）；Worker：translation 26P/0F，`--ignored trans394` 4P/0F/95.79s exit 0，全量 1585P/1F（唯一失败为 TEST-SYNC-393 期望值错误，主控已 `c0baf8c` 修正） |
| ⚠️ 纪律 | — | coder-2 为复测 HEAD 用了 `git stash`（禁用命令）。主控核：`git stash list` 为空、工作区改动完整，无损失；已记入教训，下次派单重申 |

**主控合入**：`docs/MACOS-HANDOFF.md` 394 节（模型换 NLLB 四文件 sha、永不析构的原因与 macOS 必做项）；README 中英文翻译节与发布包清单（opus-mt → NLLB、VAD ~2MB）。

**结论（主控 2026-09-24 第二轮）**：全部落实 ⇒ **验收通过，提交**。下一步 TEST-SYNC-394（tester-1）+ TRANS-COPY-395（coder-2，界面说明文案）⇒ 合包回归出包。
