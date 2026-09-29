# 删除清单 2026-09-29

| 路径 | 大小 | sha256 | 性质 | 授权 |
| --- | --- | --- | --- | --- |
| `Publish/models/qwen3-asr-1.7b-gguf/mmproj-Qwen3-ASR-1.7B-f16.gguf` | 641774112 | `5bc361e19bfdf3617c85247f9b706f7186ce0d156d9ed3c5d8bca8900b8fc3b7` | 发布目录副本（MEM-453 起引擎只用 Q8_0 编码器）；原件 `models/qwen3-asr-1.7b-gguf/` 同 sha，**保留**（Gavin：本地留 f16 作对照） | Gavin 2026-09-29「先删除吧」 |

删前核验：`Publish/models` 与其 `qwen3-asr-1.7b-gguf` 均为实体目录（LinkType 空，非 junction / symlink），删除不穿透到原件。
