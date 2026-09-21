# 删除清单 · 2026-09-21（二）· 历史端测 debug 日志

> 不可逆操作纪律：删除前落盘清单。**这些文件不进 git，删除后不可恢复。**
> 授权：Gavin 2026-09-21 明确指示删除。

## 删除对象

| 文件 | 大小 | 内容 |
| --- | --- | --- |
| `collab/evidence/debug-gavin-endtest-20260904.log` | 98,348,661 B（94 MB） | 2026-09-04 Gavin 端测原始 debug 日志 |
| `collab/evidence/debug-gavin-endtest-20260905.log` | 115,513,967 B（110 MB） | 2026-09-05 Gavin 端测原始 debug 日志 |
| `collab/evidence/debug-gavin-endtest-BUILD135-20260907-0136.log` | 23,970,807 B（23 MB） | 2026-09-07 BUILD-135 端测原始 debug 日志 |

**合计 237,833,435 B ≈ 227 MB**

## 判断依据

- 三份日志对应的问题**均已关闭**（09-04/05 批次与 BUILD-135 批次的缺陷都已修复并出包）
- 结论性内容已沉淀进 `CHANGELOG.md` / `logs/YYYYMMDD.md` / `decisions-archive.md` /
  `troubleshooting-archive.md`，**日志本身只是原始取证材料**
- 当前活跃的取证材料**不在此列**，保留不动：
  `20260921-gavin-e2e/`（BUILD-290 端测）、`20260921-build306-e2e/`（BUILD-306 端测）、
  `20260921-qwen3-poc/`（选型 PoC 原始数据）

## 影响

若日后需要复查 09-04/05/07 那三批的原始现场，**无法恢复**，只能依据上述已沉淀文档。
判断：那三批问题已关闭且结论已成文，风险可接受。
