# 验收清单 · FIX-GATE-TIMING-ONLY-392（coder-2）

| # | 设计要求 | 结论 |
| --- | --- | --- |
| 1 | 静默计时清零/累加/补偿用 has_speech（时序） | ✅ `if !has_speech {…} else {清零}` |
| 2 | 337 边界 b 用 has_speech | ✅ 未改 |
| 3 | `speech_since_last_reset`、`acc_pending_has_speech` 用 vad_speech（内容） | ✅ `if vad_speech {…}` |
| 4 | `shadow_done_for_pause` / `acc_done_for_pause` 复位 | ⚠️→✅ **主控任务书写错、已由主控修复**：任务书把 done 复位划入「内容 ← vad_speech」，coder-2 照做 ⇒ 背景人声被门拒时 acc_silent_ms 持续 ≥1200ms、done 每 chunk 被清 ⇒ 每 ~10ms 派发一次、串行解码队列被碎片窗淹没。done 复位属**时序**，改回过门有声（has_speech）分支；补源码护栏 + 「背景人声 5s 只派发一次」单测 `gate392_review_done_reset_is_timing_not_content` |
| 5 | VAD 不可用：vad_speech = 音量兜底，整条判定与 384 逐位相同 | ✅ `ChunkJudgment` 兜底分支 |
| 6 | estimate 偶数取下中位 | ✅ `v[(len-1)/2]` |
| 7 | 每次录音首个已确认段不学习 | ✅ `learn_segment` / `first_learn_skipped` |
| 8 | 埋点 `learned=`、`vad_only_speech_chunks=` | ✅ |
| B | 只改 local_stream.rs；不改 pub 签名；8 条旧用例改写注明「392 契约变更」 | ✅ |

**结论（主控 2026-09-23）**：第 4 条由主控修复后全部符合 ⇒ 验收通过。
