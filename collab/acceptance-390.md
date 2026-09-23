# 验收清单 · TUNE-DECODE-SERIAL-AND-TOKEN-CAP-390（coder-1）

| # | 设计要求 | 结论 |
| --- | --- | --- |
| 1 | `WINDOW_DECODE_CONCURRENCY` 2→1，注释写实测依据（274 vs 476 ms/音频秒） | ✅ `transcription/mod.rs` |
| 2 | `main.rs:10410` 注释改中性表述，只改注释 | ✅ main.rs 2/2 仅注释 |
| 3 | `max_new_tokens_for`：clamp(ceil(12×s)+24, 48, 256)；非有限/负 ⇒ 256；饱和不溢出 | ✅ |
| 4 | cap 用剪静音后时长（388 `samples_used`） | ✅ `speech_secs` 在 `samples` 遮蔽为剪后音频之后计算 |
| 5 | 首解 / 重解都带 `Some(cap)`；`decode_accuracy_once` 传 `None` | ✅ 重解闭包改 `decode_accuracy_allow_empty(.., Some(cap))`，空输出语义同原 `unwrap_or_default` |
| 6 | `[LocalRT-DBG-388] trim:` 追加 `max_new_tokens=` | ✅ |
| B | 不改 pub 签名；只动本地 realtime；单测 3 条；fmt/check；MACOS-HANDOFF | ✅ |

**结论（主控 2026-09-23）**：全部符合，验收通过。
