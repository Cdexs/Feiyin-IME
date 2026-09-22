# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

> 2026-09-22 归档：2026-09-21 共 59 条已移入 `handoffs-archive.md`（本文件曾达 626 行，超 200 行上限）。
> 2026-09-21 归档：2026-09-20 共 57 条已移入 `handoffs-archive.md`（本文件曾达 610 行）。
> 2026-09-20 归档：2026-09-08 / 09-17 共 26 条已移入 `handoffs-archive.md`（本文件曾达 288 行）。

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
