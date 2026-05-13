# 飞音语音输入 · Feiyin Voice Input

**[English](README.en.md) | 中文**

> Windows 托盘驻留式语音输入工具，热键触发，本地 ASR + LLM 优化，开箱即用。

[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-blue)](https://github.com/Cdexs/Feiyin-IME)
[![Version](https://img.shields.io/badge/version-v0.5.3-green)](https://github.com/Cdexs/Feiyin-IME/releases)
[![License](https://img.shields.io/badge/license-MIT-orange)](LICENSE)

---

## 核心功能

| 功能 | 说明 |
|------|------|
| 🎙️ **全局热键录音** | Toggle / PTT 双模式，可自定义热键组合 |
| 🧠 **本地语音识别** | SenseVoice 多语言模型（中 / 英 / 日 / 韩 / 粤），INT8 量化 |
| ✨ **LLM 文本优化** | OpenAI 兼容接口，纠错 + 标点 + 格式化 |
| 🔤 **本地离线翻译** | opus-mt CT2 模型，中 ↔ 英双向互译，长文本自动分段 |
| 🔡 **标点自动补全** | CT-Transformer ONNX，识别后自动添加标点 |
| 📖 **用户词库** | 自定义词条映射 + 自动学习高频纠错对，SQLite 持久化 |
| 🔇 **麦克风静音检测** | 热键前 + 录音中双场景检测，静音时立即提示 |
| 🌐 **多语言 UI** | 简体中文 / 繁体中文 / English |
| 💥 **崩溃报告** | 独立 crash-reporter 进程，本地存储 + 邮件上报 |

---

## 快速开始

### 系统要求

- Windows 10 / Windows 11（64位）
- WebView2 运行时（Win10 用户若缺失，程序启动时自动引导安装）
- 麦克风设备

### 安装运行

1. 下载并解压发布包到任意目录
2. 双击 `voice-ime.exe` 启动，托盘出现飞音图标
3. 按默认热键 **F9** 开始录音，再按 F9 结束并输入文字
4. 右键托盘图标 → **设置** 配置热键 / LLM / 翻译等选项

### 目录结构（发布包）

```
Feiyin-IME/
├── voice-ime.exe           # 主程序
├── voice-ime-ui.exe        # 设置界面（Tauri + React）
├── crash-reporter.exe      # 崩溃报告程序
├── *.dll                   # 运行时依赖库
├── config.toml             # 用户配置（首次启动自动生成）
├── wordbook.sqlite         # 用户词库数据库
└── models/
    ├── sherpa-onnx-sense-voice-*/   # 语音识别模型（必须，~233MB）
    ├── opus-mt-zh-en/               # 中→英翻译模型（可选，~164MB）
    ├── opus-mt-en-zh/               # 英→中翻译模型（可选，~164MB）
    └── punct-ct-transformer-zh/     # 标点补全模型（可选，~79MB）
```

---

## 热键说明

| 热键 | 功能 |
|------|------|
| `F9`（默认）| 开始 / 停止录音（Toggle 模式）|
| 按住 `F9` | 按住录音，松开结束（PTT 模式）|
| `右 Ctrl + F9` | 录音同时翻译（需配置翻译热键）|
| `Esc` | 取消当前录音 |

热键可在设置界面 **通用 → 触发方式** 中自定义。

---

## LLM 配置

支持任何 OpenAI 兼容接口：

```toml
[llm]
api_url = "https://api.openai.com/v1"
api_key = "sk-..."
model   = "gpt-4o-mini"
enabled = true
```

推荐国内服务：[SiliconFlow](https://siliconflow.cn)、[DeepSeek](https://deepseek.com)

> 未配置 LLM 时，程序自动降级为纯本地转录模式，无需网络。

---

## 翻译功能

- **触发方式**：录音时同时按住翻译热键（默认右 Ctrl）
- **模型优先级**：已配置 LLM 时优先使用 LLM 翻译；否则自动使用本地 opus-mt 模型
- **分段翻译**：长文本（>120字符）自动分段翻译，避免截断

---

## 技术架构

```
voice-ime.exe (Win32 Controller)
├── Win32 消息循环 + RegisterHotKey 全局热键
├── 系统托盘 (tray-icon)
├── Win32 GDI 录音悬浮窗 (Overlay)
│   ├── 录音状态：波形 + 麦克风图标
│   ├── 处理状态：Shimmer 扫光动效
│   └── 错误状态：红圈 + 提示文字
├── WASAPI 音频采集 (cpal)
├── SenseVoice ASR (sherpa-onnx)
├── LLM 文本优化 (reqwest / OpenAI API)
├── CT-Transformer 标点补全 (sherpa-onnx ONNX)
├── opus-mt 翻译引擎 (CTranslate2)
└── SQLite 用户词库 (rusqlite)

voice-ime-ui.exe (Tauri + React)
└── 配置界面（由主程序子进程拉起）

crash-reporter.exe
└── 独立崩溃上报程序
```

---

## 构建

### 环境要求

- Rust stable（1.75+）
- Node.js 18+
- Windows SDK + VS Build Tools（C++ 桌面开发工作负载）

### 开发构建

```bash
# 初始化开发环境（建立模型/DLL链接）
PowerShell -File scripts/init-publish.ps1

# 检查编译
cargo check

# Release 构建
build.bat
```

### 运行时依赖设置

所有外部资源从 **exe 所在目录**加载：

| 文件 | 路径 |
|------|------|
| 用户配置 | `{exe目录}/config.toml` |
| 词库数据库 | `{exe目录}/wordbook.sqlite` |
| AI 模型 | `{exe目录}/models/` |

---

## 版本历史

| 版本 | 主要特性 |
|------|---------|
| v0.5.3 | 长文本分段翻译 / 麦克风静音检测 / 全路径 exe 目录化 / 标点补全 / 繁体中文 UI |
| v0.5.2 | SQLite 词库 / LLM 词条学习 / 多语言 UI |
| v0.5.1 | Tauri v2 升级 |
| v0.5.0 | macOS 跨平台架构抽象 |
| v0.4.0 | UI 框架升级：eframe → Tauri + React |
| v0.3.x | Win32 架构重构 / Paraformer ASR / 崩溃报告 |

---

## License

MIT License — 详见 [LICENSE](LICENSE)
