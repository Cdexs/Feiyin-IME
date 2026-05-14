# 主程序运行依赖清单 · voice-ime

> 生成日期：2026-05-13
> 说明：列出 feiyin-ime.exe 正常运行所必须的全部外部文件，按部署位置分组。

---

## 一、可执行文件（安装目录 / Publish/）

| 文件 | 大小（参考）| 说明 |
|------|------------|------|
| `feiyin-ime.exe` | ~10.9MB | 主程序（Win32 controller + ASR + 托盘）|
| `feiyin-ime-ui.exe` | ~8.65MB | 设置界面（Tauri + React）|
| `crash-reporter.exe` | ~24.7MB | 崩溃报告独立程序 |

---

## 二、运行时 DLL（安装目录 / Publish/，与 exe 同级）

| 文件 | 大小 | 说明 | 必须性 |
|------|------|------|--------|
| `sherpa-onnx-c-api.dll` | 3.9MB | ASR 推理核心（SenseVoice 语音识别）| **必须** |
| `sherpa-onnx-cxx-api.dll` | 104KB | ASR C++ API 层 | **必须** |
| `onnxruntime.dll` | 15MB | ONNX 推理引擎（ASR 模型加载）| **必须** |
| `onnxruntime_providers_shared.dll` | 12KB | ONNX 执行提供器 | **必须** |
| `ctranslate2.dll` | 7.8MB | CTranslate2 翻译引擎（opus-mt 翻译模型）| 启用翻译时必须 |
| `libiomp5md.dll` | 1.6MB | Intel OpenMP 并行运算库（CT2 依赖）| 启用翻译时必须 |
| `cudnn64_9.dll` | 264KB | cuDNN GPU 加速库 | 可选（GPU 推理时需要，CPU 模式可省略）|

---

## 三、AI 模型文件（安装目录下的 `models/` 子目录）

模型路径（代码实现 `src/transcription/mod.rs`，EXE-DIR-PATHS-001 统一后）：
- 固定为 `{exe所在目录}/models/`，无 fallback

### 3.1 语音识别模型（必须）

**目录**：`models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09/`
**大小**：~233MB

| 文件 | 说明 |
|------|------|
| `model.int8.onnx` | SenseVoice INT8 量化推理模型（主模型文件）|
| `tokens.txt` | 语音识别词表 |

### 3.2 本地翻译模型（可选，启用翻译功能时需要）

**目录 1**：`models/opus-mt-zh-en/`（中→英翻译）
**目录 2**：`models/opus-mt-en-zh/`（英→中翻译）
**大小**：每个约 164MB

每个目录包含：

| 文件 | 说明 |
|------|------|
| `model.bin` | CT2 格式翻译模型权重 |
| `config.json` | 模型配置 |
| `source.spm` | 源语言 SentencePiece 分词器 |
| `target.spm` | 目标语言 SentencePiece 分词器 |
| `shared_vocabulary.json` | 共享词表 |
| `tokenizer.json` | Tokenizer 配置 |
| `tokenizer_2.json` | 备用 Tokenizer 配置 |

### 3.3 标点补全模型（可选，设置中启用标点补全时需要）

**目录**：`models/punct-ct-transformer-zh/`
**大小**：~79MB

| 文件 | 说明 |
|------|------|
| `model.onnx` | 标点恢复 ONNX 模型（CT-Transformer）|
| `tokens.json` | 标点词表 |
| `vocab.txt` | 词汇表 |
| `config.yaml` | 模型配置 |

---

## 四、用户数据文件（与 exe 同目录，EXE-DIR-PATHS-001）

所有数据文件与 exe 同目录存放，便携式部署，无需 AppData。

| 文件路径 | 说明 | 创建时机 |
|----------|------|----------|
| `{exe目录}/config.toml` | 用户配置（LLM Key、热键、语言等）| init-publish.ps1 预置；不存在时自动生成默认值 |
| `{exe目录}/wordbook.sqlite` | 词库数据库（词条 + 候选学习表）| init-publish.ps1 预置空库；不存在时首次启动自动创建 |
| `{exe目录}/crash.json` | 崩溃报告存档（只保留最新一份）| 崩溃时自动写入 |
| `{exe目录}/debug.log` | 调试日志 | 以 -debug 参数启动时创建 |

---

## 五、Publish 目录完整结构（发布打包参考）

```
Publish/
├── feiyin-ime.exe               # 主程序
├── feiyin-ime-ui.exe            # 设置界面
├── crash-reporter.exe          # 崩溃报告
├── sherpa-onnx-c-api.dll       # ASR 核心 DLL
├── sherpa-onnx-cxx-api.dll     # ASR C++ DLL
├── onnxruntime.dll             # ONNX 推理引擎
├── onnxruntime_providers_shared.dll
├── ctranslate2.dll             # 翻译引擎（启用翻译时必须）
├── libiomp5md.dll              # OpenMP（启用翻译时必须）
├── cudnn64_9.dll               # cuDNN（可选，GPU 推理时需要）
├── config.toml                 # 用户配置（init-publish.ps1 预置）
├── wordbook.sqlite             # 词库数据库（init-publish.ps1 预置空库）
└── models/
    ├── sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09/   # 必须 ~233MB
    │   ├── model.int8.onnx
    │   └── tokens.txt
    ├── opus-mt-zh-en/          # 可选：中→英翻译 ~164MB
    │   ├── model.bin
    │   ├── source.spm
    │   ├── target.spm
    │   └── ...
    ├── opus-mt-en-zh/          # 可选：英→中翻译 ~164MB
    │   └── ...
    └── punct-ct-transformer-zh/ # 可选：标点补全 ~79MB
        ├── model.onnx
        ├── tokens.json
        └── ...
```

---

## 六、最小运行包（不含翻译/标点功能）

只需语音识别 + 文本注入的最小部署：

| 类型 | 文件 | 大小 |
|------|------|------|
| EXE | feiyin-ime.exe | 10.9MB |
| EXE | feiyin-ime-ui.exe | 8.65MB |
| EXE | crash-reporter.exe | 24.7MB |
| DLL | sherpa-onnx-c-api.dll | 3.9MB |
| DLL | sherpa-onnx-cxx-api.dll | 104KB |
| DLL | onnxruntime.dll | 15MB |
| DLL | onnxruntime_providers_shared.dll | 12KB |
| 模型 | models/sherpa-onnx-sense-voice-*/model.int8.onnx | ~230MB |
| 模型 | models/sherpa-onnx-sense-voice-*/tokens.txt | <1MB |
| 配置 | config.toml | <1KB |
| 词库 | wordbook.sqlite | 24KB |
| **合计** | | **~303MB** |
