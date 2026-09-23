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
