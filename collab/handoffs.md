# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。

## 2026-08-16 — tester-1 — BUILD-016 ✅ v0.8.0 首包出包（全构建 + 七项核验 + 双探针，生产零改动）

- **来源**：Gavin 已明确下达出包指令，主控派单 BUILD-016（阶段五）。基线 HEAD `be76fc1`
- **构建**：Step 1 清进程（feiyin-ime PID 23888）→ Step 2 npm build（新 `index-DkzLqu_f.js`）+ Tauri UI release 2m14s（cp 到 target/release/）→ Step 3 主程序 2m40s → Step 4 同步 Publish/（三 exe + scene/itn 两 toml；config.toml 等运行时数据未覆盖，保持 07-28 原样）
- **七项核验全 PASS**（详见 outbox/tester-1/result.md，机器实测）：① 六 exe 时间戳 00:45-00:48；② 三 exe 两副本 sha256 相等；③ 两 toml 三副本一致；④ ProductVersion 0.8.0.0/0.8.0/0.8.0.0；⑤ `index-DkzLqu_f.js` 嵌 ui.exe / 旧名 0（i18n 裸串 grep 0 系 Tauri 压缩已知行为，改文件名字符探针）；⑥ 冒烟 Responding=True 已清理；⑦ 大小对照已解释
- **🔴 双探针**：正向 `qwen-audio-3.0-asr-flash-streaming` / `api-ws/v1/inference` 各 1 命中；反向 `api-ws/v1/realtime` / `qwen3-asr-flash-realtime` 0 命中 —— 新 ASR 进包 + 旧引擎清干净
- **产物**：`Publish/feiyin-ime.exe` 12106752B（sha256 `86176283…`）/ `feiyin-ime-ui.exe` 10026496B（`db095564…`）/ `crash-reporter.exe` 24859648B（`a9c30e4e…`）
- **验收**：cargo fmt 未跑（本单纯构建，无代码改动）；版本号三处 0.8.0 未动；生产代码零改动
- **详情**：outbox/tester-1/result.md + logs/20260816.md + CHANGELOG.md

---
