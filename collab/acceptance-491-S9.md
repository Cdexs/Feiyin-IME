# 验收清单 · ASR-SERVER-491 S9（逐字时间戳 T1–T6）

> 依据：`docs/ASR-SERVER-DESIGN.md` §12.1–12.4（原设计）+ §12.5（实现记录 / 偏离留档）；任务书 `collab/spec-491-S9.md`。
> 判定：✅ 符合 / ⚠️ 偏离（附理由，已写进设计 §12.5）/ ❌ 缺失。行号以 2026-10-17 验收时为准。

## A. 设计逐条对照

| # | 设计要求（原文要点） | 代码位置 | 判定 |
| --- | --- | --- | --- |
| 0 | §12.1「拼接器再加上窗口起点，换算成整段录音的绝对秒，坐标系是对的」 | 核实不成立：虚拟时间轴（片首尾相接 + 每片前后补零）。修法：片映射 `char_times::SliceMap`；派发前登记 `local_stream.rs:73` `record_slices`（三处派发点 `:2263` / `:2406` / `:2516`，均在 `on_segment(` 之前）；会话换算 `session.rs:207`（回灌）/ `:269`（终稿） | ⚠️ 设计原文有误，已在 §12.1 加更正条 |
| 1 | T1「`OrderedReflow` 加与 `committed` 逐字对应的 `committed_times`，各拼接分支同步追加」「文字逐位不变」 | `transcription/mod.rs:2988` 字段；`:3035` `commit_times`；四个 `push_str` 分支 `:3128` / `:3144` / `:3165` / `:3183` 均先并时间再并文字；`refine_times` 回退同步截断 `:3209`；`full_times` `:3278`、`timed_snapshot` `:3289`、`finish_timed` `:3297`。不参与任何拼接判定 | ✅ |
| 2 | T2「只对远程会话照常对齐最后一窗；本机不变」 | `localrt_session.rs:190` 条件加 `!recognizer.is_remote()`；收尾循环本来就收到通道关闭为止（`Aligned` 消息一并处理），无需另等 | ✅ |
| 3 | T3「远程会话在该句定稿前对这段音频补对齐一次；补不出来的字标成估算」 | 不补对齐，直接插值标 `estimated`（`char_times::fill_missing`，终稿 `session.rs:57` `result_words`） | ⚠️ 偏离，理由见设计 §12.5 T3 行：无时间的三类情形都不是「再对一次能对上」 |
| 4 | T4「整段兜底：远程会话对整段补对齐一次」 | `session.rs:300` `whole_fallback_times`：≤30s（`:54`）整段对齐；更长 / 失败 ⇒ 预览粗略时间搬运 ⇒ 估算 | ✅（加了 30s 上限，超限退粗略时间：防长录音对齐占通道过久） |
| 5 | T5「远程会话把 SenseVoice 的 token 时间带出去，标成粗略；本机不影响」 | `local_stream.rs:1789` 重解时折成每字时间（仅 `is_remote`）；`:1815` 确认并入；`:2385` 442 切分同步；`:1925` / `:2573` 远程给 `rough_preview_words`，本机仍 `state.display_words()`（空） | ✅ |
| 6 | T6「节点一个不改，外面套一层前后对比搬时间」：没变的原样、替换串占旧串起止、插入标点取前字终点、删掉的消失 | `transcription/char_times.rs` `transfer`；套用点：去口水重复（`localrt_session.rs:1320` `acc_times_for`，回灌三处 + 终稿 `session.rs:269` 后一行）、繁简规整（`session.rs:279`）、预览收尾（`compose.rs` `final_preview`）。S10 节点接入时同样套 | ✅ |
| 7 | §12.4「`words` 按阿里格式：`begin_time` / `end_time` 毫秒、相对会话第一帧音频、`text`、`punctuation`」 | `frame.rs:156` `ResultPayload.words`；网关 `ws.rs:668` `result_json`；成词 `char_times::group_words`（ASCII 连写一词、标点挂前词、拼接不变量） | ✅ |
| 8 | §12.4「来源标记放扩展字段 `feiyin.ts_source`」 | `ws.rs:668` 按 `words` 顺序给来源数组 | ✅ |
| 9 | §12.4「定稿句里的每个字都保证有时间，补不出的才标 estimated」 | 终稿 `result_words(&text, &times, pcm_secs)`：`fill_missing` 补满（尾部缺失补到录音末尾） | ✅ |

## B. 不改坏现有功能（本机路径逐项）

| 改动点 | 本机路径 | 结论 |
| --- | --- | --- |
| `OrderedReflow` 多维护 `committed_times` | 只增删时间数组，拼接判定 / 文字不读它 | 文字逐位不变；`finish` 仍在（改为仅测试），生产走 `finish_timed` 取前两项 |
| `run_acc_windows` | `timed = recognizer.is_remote()` = false ⇒ 不调 `timed_snapshot` / `transfer`，`acc_times` 恒 `None`，返回第 6 项 `None`；main.rs 解构加 `_` | 回灌事件个数 / 内容 / 节奏不变（对齐晚到且文字没变时**不**另发回灌，远程也一样） |
| T2 条件 | 本机 `!is_remote()` = true ⇒ 与改前同一判定 | 不变 |
| 预览线程 | `slice_log = None` ⇒ `record_slices` 立即返回；`sim_times` 恒空 ⇒ `append` / `split_off(0)` 无操作；`words` 仍 `state.display_words()` | 不变 |
| `PipelineEvent::PreviewReflow` 新字段 | controller 臂解构加 `acc_times: _`；其余匹配用 `..` | 不变 |
| 护栏锚点 | 未动：`let mut probe = ordered.clone();`、`probe.push_window_timed(`、`for authoritative in ordered.push_window_timed(`、`sim_confirm!();`、`sim_start = cut;`、`on_segment(` 次序、`PipelineEvent::PreviewReflow {` 匹配臂 | 待测试确认（E 节） |

**远程会话的预期变化**：最后一窗照常对齐 ⇒ 终稿多等约 0.25s；最后一道接缝按时间拼（与中间各窗同一规则），终稿文字可能比改前更准。已写进设计 §12.5。

## C. 跨平台

全部改动在平台中立模块（`transcription/` / `localrt_session.rs` / `asr_service/` / 网关），无 `cfg(target_os)` 分支 ⇒ Windows / macOS 同一行为。macOS 侧需知：`PreviewReflow` 多一个字段（构造处须补）、`run_acc_windows` 返回 6 元组。写入 `docs/MACOS-HANDOFF.md`。

## D. 子 Agent 交付物验收

| 交付物 | 结论 |
| --- | --- |
| `char_times.rs`（37 单测） | 逐函数对照任务书 §5：`transfer` 的分组 / 均分 / 纯插入 / 删除，`fill_missing` 四种边界，`SliceMap::to_real` 补零区贴边与越界，`rough_from_marks`，`group_words` 成词与不变量 —— 全部符合。**发现一处质量隐患**：长文本（中段 > 400 万格，约 2000 字以上）两头都有改动时整段当一次替换 ⇒ 时间粗化。处理：超限分支改 Myers 差分（见 F 节） |

## E. 测试

| 项 | 结论 |
| --- | --- |
| 新增单测（子 Agent） | 共 58 个：`char_times.rs` 37 个；`transcription/mod.rs` 4 个（字数不变量覆盖零重叠 / 双时间 / 仅前窗时间 / 比例估算 / `refine_times` 五条拼接路径，`finish_timed` 文字 == `finish`，`timed_snapshot` 只认当前全文）；`compose.rs` 6 个（每次事件后 `display_times` 字数 == 显示字数）；`local_stream.rs` 5 个；`session.rs` 4 个（成词拼接不变量、估算来源）；网关 `ws.rs` 2 个；`frame.rs` 2 个（旧内核无 `words` 字段照常解析、空表不序列化） |
| 首轮全量 | 2044 过 / **4 失败**：① `t441_pipeline_declarations` **本批引起**：护栏锚点 `apply_authoritative_filler_dedup(&authoritative)` 被改名打断 ⇒ **改生产代码恢复原写法**（只在远程时另存一份去重前原文），护栏不放宽；②③ `ts423_single_dispatch_window_call_and_tail_macro` / `ts423g_dispatch_call_sites_source_anchor`、④ `g7_mic_muted_reject_exit_resets_poll_stop` 是 **S1 遗留的护栏漂移**（S1 搬迁后 rustfmt 把宏调用排成单行、`let must_start =` 折行；`main.rs` 第 34 行单项 `#[cfg(test)] const MAIN_RS_SRC` 让 G7 读生产区时提前截断）⇒ 修护栏的**读法**（新增 `guard_prod_lines::macro_call_arg` 单行 / 多行同口径取实参；G7 只在测试**模块**处截断），断言内容不变 |
| 复跑 | 4 个原失败全过；全量 `feiyin-ime` **2048 过 / 0 失败** / 48 忽略；网关 **26 过 / 0 失败**；rustfmt 全部干净 |
| 正式回归 | 本批不出包；S10、S11 完成后统一交 tester-1 |

## F. 遗留

- `transfer` 超限分支改 Myers O(ND) 差分（派子 Agent，单独提交）。
