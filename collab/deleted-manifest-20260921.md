# 删除清单 · 2026-09-21 · accuracy KV 512 回滚备份

> 不可逆操作纪律：删除前先落盘清单。**这两个文件删除后不可恢复**（未入 git，无其他副本）。
> 授权：Gavin 2026-09-21 直接指示「.512.bak 1.12 GB 删掉吧」。

## 删除对象（恰好 2 个文件，均为 KV 512 时代的 `llm.int8.onnx` 备份）

| 路径 | 大小 | mtime |
| --- | --- | --- |
| `models/sherpa-onnx-funasr-nano-int8-2025-12-30/llm.int8.onnx.512.bak` | 600,339,316 B | 2026-04-12 13:42 |
| `Publish/models/sherpa-onnx-funasr-nano-int8-2025-12-30/llm.int8.onnx.512.bak` | 600,339,316 B | 2026-09-20 20:47 |

**合计 1,200,678,632 B ≈ 1.12 GB**

## 删除前核实（删除即时状态）

- 两处**现行** `llm.int8.onnx` 均在位，各 **600,025,528 B**（mtime 2026-09-20 22:50）= `llm_int8_max_token_1024` 版本（DEC-072）。
- `models/` 与 `Publish/models/` 经 `ls -ld` 确认是**真实目录，不是符号链接** ⇒ 无 `[DISK-CLEANUP-001]` 的 symlink 穿透风险。
- 未使用 `cargo clean`（`project_voiceime_no_cargo_clean` 禁令），逐文件 `rm`。
- 删除范围**只有上表两个 `.512.bak`**，同目录其余模型文件一律不动。

## 影响

- 失去 KV 512 的本地回滚路径。若日后需回退，须重新从上游取 `llm_int8_max_token_512` 权重。
- DEC-068/072 已判定 1024 无副作用（20s 段预算 149→661，long 段 512 溢出出空、1024 正常），端测多轮未见 KV 相关问题。
