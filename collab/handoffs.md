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

## 2026-08-16 — coder-1 — ASR-042 在线流式 ASR 采样率修复（StreamingResampler，src/audio/mod.rs +476/-5）

- **来源**：Gavin v0.8.0 端测，新在线 ASR 识别全错（48kHz 音频按 16kHz 解）。主控派单 ASR-042，基线 HEAD `efb101f`
- **根因**：`record_streaming` 边录边发绕过了 `record()` 录音结束时的一次性 `resample_anti_alias`（FIRSTCHAR-FIX-005 引入），48kHz 直推服务端
- **改动**：仅 `src/audio/mod.rs`（+476/-5）
  - 新增 `StreamingResampler`（`pub(crate)`）：带状态流式抗混叠重采样器，数学等价于 `resample_anti_alias`（windowed-sinc FIR, TAPS=32, Hann 窗），跨块保留历史+全局 emitted 计数，接缝不截断卷积核
  - `record_streaming` 接线：pre-roll → post-hotkey → 主循环 依次喂**同一实例** + break 后 `finish()` flush 尾部 + 新日志 `Streaming resampler active: {N}Hz -> 16000Hz`
  - `total_samples`/`max_frames`/RMS/`level_buf` 继续用原始 chunk（未重采样）
  - 新增 `RESAMPLE_TAPS` 常量（`resample_anti_alias` 与 `StreamingResampler` 共用防漂移）
  - +6 测试：等价性（逐点差 ≤1e-5）/ 非对齐块长(441) / 恒等(16k→16k) / 长度(误差 ≤1) / 顺序护栏 / finish 尾部
- **"改坏会红"自证**：`push` 入口注入 `emitted=0`（每块重算）→ 等价性测试 FAILED（stream=807010 vs batch=16000），移除后 GREEN
- **验收**：cargo fmt clean / cargo check --all-targets 0 error / cargo test audio 56 passed 0 failed 1 ignored（既有用例零改动）
- **禁区核验**：`resample_anti_alias` 函数体 / `record()` / `vad.rs` / `main.rs` / `qwen_inference.rs` 零改动
- **跨平台**：`docs/MACOS-HANDOFF.md` 新增 §ASR-042 节（平台中立模块，macOS 编译同份代码；运行时影响取决于 macOS 侧 record_streaming 是否已接线）
- **版本号**：未动
- **详情**：outbox/coder-1/result.md + logs/20260816.md + CHANGELOG.md

---
