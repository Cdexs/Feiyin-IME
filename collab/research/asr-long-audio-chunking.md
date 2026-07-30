# ASR 长音频分段转录方案调研

> 调研人：coder-1
> 日期：2026-07-06
> 任务：ASR-NATIVE-LONG-001 第三部分（只调研不实施）

## 一、问题背景

FunASR Nano native（972MB）模型存在 `max_total_len=512` 硬限制（KV cache 容量）。
经实测：音频经 LFR（Low Frame Rate）降采样后，约 **28 秒以上**音频的 context_len（prompt + audio tokens）超过 512，触发截断，
截断后 decoder 生成 0 token，输出空文本。

- LFR window_size=7, shift=6（sherpa-onnx FunASR Nano 默认）
- 10ms 帧间隔 → LFR 后每 ~60ms 一个 token
- 28s ≈ 467 LFR tokens + ~18 prompt tokens + 5 after tokens ≈ 490 < 512（临界）
- 30s ≈ 500 LFR tokens + 23 prompt = 523 > 512 → 截断

performance（CTC）模型无此限制，30s/60s/90s 全部正常输出。

## 二、sherpa-onnx 分段转录可行路径

### 路径 A：VAD 分段（推荐）

sherpa-onnx 提供 `sherpa-onnx-vad-with-offline-asr` 工具链，使用 silero VAD 模型切分长音频为短段，
每段独立送 offline recognizer 转录，最后拼接。

**实现方式**：
1. 新增 `OnlineRecognizer`（VAD streaming）或直接用 `VoiceActivityDetector` 切分
2. VAD 检测语音段边界（start/end timestamps），静音段作为分隔
3. 每段音频 ≤ N 秒（如 20s，留裕量 < 512 max_total_len）独立转录
4. 文本按段序拼接

**优点**：
- VAD 天然在静音处切分，段边界语义完整
- 对 performance（CTC）模型也有益（超长音频一次性转录 RTF 增长，分段可并行）
- sherpa-onnx 官方已提供 VAD 模型 + 集成路径

**缺点**：
- 需额外 VAD 模型（silero VAD ~2MB，体积可忽略）
- 段间拼接无上下文（各段独立转录，跨段语义连贯性可能略降）
- 实时性：VAD 在录音结束后切分（非实时），增加 ~0.1s 后处理延迟

**预计工作量**：
- 新增 `src/vad/` 模块：VoiceActivityDetector 封装（~200 行）
- `transcribe_offline` accuracy 分支改为：VAD 切分 → 分段转录 → 拼接（~50 行）
- VAD 模型下载/打包（~2MB silero 模型）
- 单测：VAD 切分 + 分段拼接（~100 行）
- 总计 ~350 行 + 1 个模型文件，约 2-3 工时

### 路径 B：固定窗 + 重叠

不用 VAD，按固定窗口（如 20s）切分，相邻段重叠 1-2s 保证边界词完整，取重叠区最优拼接近似。

**优点**：无需 VAD 模型，实现简单

**缺点**：
- 固定窗可能在词中间截断（边界词被切两半）
- 重叠区拼接策略复杂（如何选最优拼接点，避免重复/丢失）
- 对静音密集音频浪费（多段纯静音也切分转录）

**预计工作量**：~200 行，约 1-2 工时，但质量不如路径 A

### 路径 C：增大 max_total_len（模型层）

sherpa-onnx 源码提示：可导出 `max_total_len > 512` 的模型
（`https://github.com/Wasser1462/FunASR-nano-onnx`），
或从 ModelScope 下载更大容量的版本。

**优点**：零代码改动，只换模型文件

**缺点**：
- KV cache 容量增大 → 内存增加（max_total_len=2048 约增 ~2x LLM 层内存）
- 模型文件可能需重新导出/下载，依赖第三方维护
- 治标不治本（超长音频如 5 分钟仍会超限）

**预计工作量**：下载新模型 + 测试，约 0.5 工时，但依赖外部模型可用性

## 三、对 performance 分支的益处

performance（CTC）模型当前无 max_total_len 限制问题（CTC 非 LLM decoder，无 KV cache 容量约束），
但超长音频（如 5 分钟）一次性转录 RTF 会增长（encoder 计算量线性），分段可：
- 并行化各段转录（多线程）
- 降低单次推理内存峰值
- 提升用户体感响应速度（首段可先输出）

不过 performance 模型 30s RTF=0.4s、90s RTF=2.2s，当前性能已可接受，分段收益不大。

## 四、推荐方案

**推荐路径 A（VAD 分段）**，理由：
1. 官方工具链成熟，风险低
2. 对 accuracy 模型是刚需（解决空输出根因）
3. 对 performance 模型可选优化（并行化）
4. VAD 模型体积小（~2MB），不增加分发负担
5. 段边界语义完整，拼接质量好

**实施建议**：作为后续独立任务立项（ASR-LONG-AUDIO-001），不与当前兜底任务合并。
当前 ASR-NATIVE-LONG-001 的兜底加固（空输出/hallucination/重复检测 → fallback）已覆盖
accuracy 模型长音频异常的用户可感影响（降级 performance 重转，不静默注入垃圾）。

## 五、参考资料

- sherpa-onnx VAD 文档：https://k2-fsa.github.io/sherpa/onnx/vad/
- silero VAD 模型：https://github.com/snakers4/silero-models
- FunASR Nano onnx 导出脚本：https://github.com/Wasser1462/FunASR-nano-onnx
- sherpa-onnx-vad-with-offline-asr 示例：sherpa-onnx v1.12.38 bin/
- max_total_len 调试输出：本任务调查阶段 PoC bin debug 日志