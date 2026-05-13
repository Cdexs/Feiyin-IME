# Voice IME — 技术设计文档 (TDD)

**版本**：v0.1  
**对应代码版本**：v0.1.0  
**对应 PRD 版本**：PRD v0.1  
**状态**：草稿  
**日期**：2026-04-13  

---

## 1. 技术栈

| 层次 | 选型 | 版本 | 理由 |
|------|------|------|------|
| 语言 | Rust | stable 1.94+ | 内存安全、无 GC、原生 Windows 集成 |
| 异步运行时 | Tokio | 1.x | LLM HTTP 请求异步化 |
| 音频采集 | cpal | 0.15 | WASAPI 后端，跨 Rust 音频标准 |
| Whisper 推理 | whisper-rs | 0.14 | whisper.cpp FFI，CPU 推理最快 |
| LLM 客户端 | reqwest | 0.12 | 成熟的异步 HTTP client |
| 文本注入 | windows crate | 0.58 | 官方 Rust Windows API 绑定 |
| 词库存储 | rusqlite (bundled) | 0.32 | 嵌入式 SQLite，零额外依赖 |
| GUI | egui/eframe | 0.29 | 纯 Rust，小体积，立即模式 |
| 系统托盘 | tray-icon | 0.19 | Windows/macOS 托盘支持 |
| 配置序列化 | toml + serde | 0.8 / 1.x | 人类可读配置文件 |
| 下载工具 | ureq | 2.x | 同步 HTTP，用于模型下载 |

---

## 2. 系统架构

### 2.1 进程模型

```
┌─────────────────────────────────────────────────────┐
│                   voice-ime.exe                      │
│                                                      │
│  ┌──────────────┐   ┌──────────────────────────────┐ │
│  │  Main Thread │   │       Pipeline Thread        │ │
│  │  (egui loop) │   │  Idle → Record → Transcribe  │ │
│  │              │   │  → Optimize → Inject → Idle  │ │
│  └──────────────┘   └──────────────────────────────┘ │
│         │                        ▲                   │
│         │           ┌────────────┘                   │
│         │           │                                │
│  ┌──────────────┐   │                                │
│  │  Hotkey      │───┘ crossbeam-channel              │
│  │  Thread      │     (hotkey_tx/rx)                 │
│  │ (msg loop)   │                                    │
│  └──────────────┘                                    │
│                                                      │
│  ┌──────────────────────────────────────────────┐    │
│  │           Tokio async runtime                │    │
│  │  (LLM HTTP requests — single-thread block)   │    │
│  └──────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

### 2.2 数据流

```
麦克风 PCM (f32, 16kHz, mono)
    │
    ▼ [audio::AudioCapture]
    │  · WASAPI 采集
    │  · 自动降采样到 16kHz
    │  · RMS 能量 VAD
    │
    ▼ [transcription::Transcriber]
    │  · whisper-rs → whisper.cpp
    │  · FullParams { language: "zh", greedy }
    │
    ▼ [wordbook::Wordbook::apply()]
    │  · SQLite 词对替换（O(n) scan）
    │
    ▼ [llm::LlmClient::optimize()]  ──超时/错误──► 原始文本
    │  · POST /v1/chat/completions
    │  · temperature=0.3, max_tokens=1024
    │
    ▼ [injection::inject_text()]
       · 优先路径：set_clipboard + SendInput(Ctrl+V)
       · 降级路径：SendInput Unicode chars
       · 注入后等待 150ms 恢复剪贴板
```

---

## 3. 模块设计

### 3.1 `config` 模块

**职责**：配置读写、默认值、持久化路径  
**存储路径**：`%APPDATA%\voice-ime\config.toml`

```rust
AppConfig {
    llm:       LlmConfig      // api_url, api_key, model, system_prompt, enabled
    hotkey:    HotkeyConfig   // vk_code, modifiers, display_name
    audio:     AudioConfig    // silence_threshold, silence_duration_ms, max_record_seconds
    injection: InjectionConfig // use_clipboard, clipboard_delay_ms
}
```

### 3.2 `audio` 模块

**职责**：麦克风采集、VAD、降采样  
**关键参数**：
- 目标采样率：16000 Hz（Whisper 要求）
- VAD：RMS 能量阈值（可配置，默认 0.01）
- 静音判定：连续 N ms RMS < threshold
- 输出：`Vec<f32>`（mono, 16kHz）

**降采样**：线性插值（适用于 16kHz 降采样场景，低复杂度）

### 3.3 `transcription` 模块

**职责**：Whisper 推理、模型管理  
**模型**：ggml-base.bin（~142MB）  
**存储路径**：`%APPDATA%\voice-ime\models\ggml-base.bin`  
**首次运行**：从 HuggingFace（`ggerganov/whisper.cpp`）下载，校验文件大小 > 100MB  

**推理参数**：
```
SamplingStrategy::Greedy { best_of: 1 }
language: "zh"
print_progress: false
```

### 3.4 `llm` 模块

**职责**：OpenAI 兼容 HTTP 调用  
**协议**：`POST {api_url}/chat/completions`  
**请求格式**：标准 OpenAI Chat Completions JSON  
**超时**：30s  
**错误处理**：HTTP 错误/网络超时均返回原始文本（不崩溃）  

### 3.5 `injection` 模块

**职责**：将文本写入当前焦点窗口  

| 方式 | 实现 | 适用场景 |
|------|------|---------|
| 剪贴板 + Ctrl+V | OpenClipboard + CF_UNICODETEXT + SendInput(VK_CONTROL+VK_V) | 大多数输入框 |
| SendInput Unicode | KEYEVENTF_UNICODE 逐字符 | 不支持粘贴的场景 |

**注意**：注入前保存剪贴板，注入后 150ms 恢复。

### 3.6 `wordbook` 模块

**职责**：自定义词库 CRUD + 自动学习  
**Schema**：
```sql
CREATE TABLE words (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    raw        TEXT NOT NULL,       -- 原文（ASR 输出）
    corrected  TEXT NOT NULL,       -- 替换词
    count      INTEGER DEFAULT 1,   -- 使用次数
    created_at INTEGER DEFAULT (unixepoch()),
    UNIQUE(raw, corrected)
);
```

**自动学习算法**：  
1. 注入后启动 300ms 观察窗口（`SetWinEventHook EVENT_OBJECT_VALUECHANGE`）
2. 获取焦点元素的最终文本
3. 与注入文本做字符级 diff
4. 提取差异片段作为 (raw, corrected) 词对
5. `INSERT OR UPDATE` 到词库

### 3.7 `hotkey` 模块

**职责**：全局热键注册与事件分发  
**实现**：
- `RegisterHotKey(NULL, 1, modifiers, vk_code)` 在专用线程注册
- 线程消息泵：`GetMessageW` 过滤 `WM_HOTKEY`
- 触发时向 pipeline 线程发送 `crossbeam-channel` 信号

**热键 ID**：固定为 1（单热键场景，后续可扩展）

### 3.8 `ui` 模块

**子模块**：
- `ui::settings`：egui 设置窗口（`eframe::App` trait）
- `ui::tray`：托盘状态枚举（`Idle / Recording / Processing / Error`）

**设置窗口布局**：
```
[大模型配置]  ← CollapsingHeader（默认展开）
  · 启用开关 | API 地址 | API Key | 模型名 | System Prompt

[热键配置]    ← CollapsingHeader（默认展开）
  · 当前热键显示 | 预设快捷选择（F9/F10/Ctrl+Space/Alt+`/Ctrl+Alt+V）

[录音配置]    ← CollapsingHeader（默认折叠）
  · 静音阈值滑块 | 停止时长 | 最长时间

[文本注入]    ← CollapsingHeader（默认折叠）
  · 注入方式选择 | 剪贴板延迟

[用户词库]    ← CollapsingHeader（默认展开）
  · 添加词对输入 | 词库列表（可滚动）| 删除按钮

[保存配置]    ← 底部按钮 + 状态提示
```

---

## 4. 关键技术决策记录

### ADR-001：Whisper 推理库选型

**决策**：使用 `whisper-rs`（whisper.cpp FFI）  
**备选**：candle（纯 Rust）、onnxruntime（ONNX）  
**理由**：whisper.cpp 是该任务最成熟的 C++ 实现，CPU 推理速度在同类方案中最快；whisper-rs 提供直接的 Rust 封装。  
**代价**：编译时依赖 libclang（bindgen 需要）、CMake、MSVC。  

### ADR-002：文本注入策略

**决策**：剪贴板注入优先，SendInput 降级  
**理由**：SendInput Unicode 在高 DPI 场景和部分 IME 开启时有兼容问题；剪贴板方式对 Unicode 最可靠。  
**风险**：会短暂覆盖用户剪贴板内容；通过 150ms 后恢复缓解。  

### ADR-003：GUI 框架选型

**决策**：egui/eframe  
**理由**：纯 Rust、立即模式、二进制增量 ~5MB、内存占用约 20MB，符合"低资源"要求。原生 Win32 UI 需要大量 boilerplate；Tauri 依赖 WebView 较重。  

### ADR-004：配置格式

**决策**：TOML  
**理由**：人类可读可编辑，用户可直接修改配置文件；相比 JSON 更适合配置文件场景。  

---

## 5. 编译与打包

### 5.1 编译前置条件

| 工具 | 要求 |
|------|------|
| Rust | stable 1.74+，target: x86_64-pc-windows-msvc |
| MSVC | VS 2022 BuildTools，MSVC 14.x |
| CMake | 3.15+（VS BuildTools 内置） |
| LLVM/libclang | 需要 libclang.dll（bindgen 生成 whisper-rs 绑定用） |

### 5.2 构建命令

```bash
# Debug
cargo build

# Release（启用 LTO + 优化）
cargo build --release
```

### 5.3 Release Profile

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
```

### 5.4 安装包

使用 Inno Setup 6.x 打包：
- 主程序：`target/release/voice-ime.exe`
- 图标：`assets/icons/`
- 开机自启（可选，用户选择）
- 卸载支持

---

## 6. 性能目标与测量方法

| 指标 | 目标 | 测量方法 |
|------|------|---------|
| 推理延迟（10s 语音，CPU） | ≤ 2s | `std::time::Instant` 计时 |
| 后台内存（待机） | ≤ 80MB | Process Monitor |
| 后台 CPU（待机） | < 1% | Task Manager |
| LLM 调用延迟 | ≤ 3s（P90） | 日志时间戳 |
| 文本注入到位时间 | < 200ms | 肉眼 / 日志 |

---

## 7. 错误处理策略

| 场景 | 处理方式 |
|------|---------|
| 无麦克风设备 | 弹窗提示，返回待机状态 |
| Whisper 推理失败 | 日志记录，返回待机状态 |
| LLM API 超时/错误 | 降级使用原始转录文本，继续注入 |
| 文本注入失败 | 日志记录，不影响其他流程 |
| 配置文件损坏 | 使用默认配置，覆盖写入 |
| 模型下载失败 | 提示用户重试，程序仍可启动（跳过转录） |

---

## 8. 版本管理规范

- `Cargo.toml` `version` 字段与文档版本号保持一致
- 文档命名格式：`{类型}-v{主}.{次}.md`（如 `PRD-v0.1.md`、`TDD-v0.1.md`）
- 每次发布前同步更新文档的"对应代码版本"字段
- 重大架构变更需同时更新 TDD 并在 ADR 中记录决策
