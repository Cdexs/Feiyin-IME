# 飞音智能语音输入 · Flash Voice Input

**[English](README.en.md) | 中文**

> 说话就是输入。文字跟着你的语速出现在屏幕上，而且它知道你正在用哪个软件。

[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-blue)](https://github.com/Cdexs/Feiyin-IME)
[![Version](https://img.shields.io/badge/version-v0.9.0-green)](https://github.com/Cdexs/Feiyin-IME/releases)
[![License](https://img.shields.io/badge/license-MIT-orange)](LICENSE)

---

## 为什么又一个语音输入工具

市面上的语音转文字大多止步于「把声音变成字」。但口语和书面语之间还隔着一段距离 ——
你说话时会有「嗯」「那个」，会说半句改口重来，会把「三点半」说成三个字而不是 `3:30`，
更重要的是：**你在微信里想说的话，和你在终端里想输入的命令，根本不是同一种东西。**

飞音想做的是后面这一段：**从「听见」到「可以直接用」**。

---

## 它是什么样的

### 🎤 边说边出字，不用等

按下热键，文字跟着你的语速平滑地出现在悬浮窗里 —— 不是说完等一下才整段蹦出来，
也不是机械的打字机匀速动画，**你停顿它就停，你说快它就快**。

说错了？不用重来。**录音过程中直接点悬浮窗里的文字就能改**，改完再上屏。

### 🎯 它知道你在用什么软件

同一句话，在不同的地方本来就该是不同的样子。飞音会识别当前窗口，自动切换输出风格：

| 你在用 | 它怎么处理 |
|--------|-----------|
| 微信、QQ 等聊天软件 | 保持口语，该怎么说就怎么说，**不做压缩不做排版** |
| 终端、IDE | 技术风格、不寒暄，**绝不注入换行**（防止半句话被当成命令执行） |
| 邮件、Word、文档 | 正式书面语，允许分段和列表，自动压掉口语里的重复啰嗦 |
| Claude、ChatGPT 等 AI 助手 | 按「给 AI 的工作指令」处理，正式、结构化 |
| 浏览器 | 简洁的网页友好排版 |

这套规则放在外置的 `scene-rules.toml` 里，**你可以自己加软件、改风格，改完重启即可，不用重新编译**。

### 🔢 中文数字说人话

口语里的数字和写下来的数字是两回事，飞音替你换算：

```
四点半开会          →  4:30 开会
一个半小时          →  1.5 小时
一千零四十六万八千七百四十一  →  10468741
营收三亿五          →  营收 3.5亿
```

同时它**知道什么时候不该换**：「五代十国」「三五成群」「一点点」这类成语、专名、固定表达
一律保持汉字 —— 这部分规则同样外置在 `itn-rules.toml`，踩到坑可以自己补。

### 🏠 本地优先，可以完全离线

语音识别、标点补全、中英翻译**全都能跑在本机**，不联网也能用，说的话不出电脑。
需要更强的润色能力时，再接任意 OpenAI 兼容的大模型 —— **这是可选项，不是前提**。
在线识别引擎也提供，追求最快首字响应时可以切过去。

### 📖 越用越懂你

专业术语、人名、产品名老是识别错？把它加进用户词库，之后就认得了。
开启格式化输出后，它还会从你的修改里**自动学习**高频纠错，不用手动一个个加。

### 🌍 说中文，也说英文

识别支持中 / 英 / 日 / 韩 / 粤五种语言；按住翻译热键说话，可以直接出中英互译的结果。
界面本身有简体中文 / 繁體中文 / English 三套。

---

## 快速开始

### 你需要

- Windows 10 / 11（64 位）
- 一个麦克风
- WebView2 运行时（没有的话程序会自动装）

### 三步跑起来

1. 下载 [最新发布包](https://github.com/Cdexs/Feiyin-IME/releases) 解压到任意目录
2. 双击 `feiyin-ime.exe`，托盘出现图标 → 右键「配置」
3. 设好录音热键（**推荐右 Ctrl 或右 Alt** —— 单手可按，不和常用快捷键打架），按住就能说

想要更好的输出质量，再去「格式化输出」页填一个大模型的 API Key（见下文）。不填也能用。

### 热键

| 热键 | 功能 |
|------|------|
| `F9`（默认，可改）| 开始 / 停止录音（Toggle 模式）|
| 按住不放 | 按住说话，松开结束（PTT 模式）|
| 录音时按住翻译热键 | 边说边翻译 |
| `Esc` | 取消这次录音 |

热键在设置界面 **通用 → 触发方式** 里改。**左右修饰键可以分开设**（左 Ctrl 和右 Ctrl 是两个键），
也支持任意组合键，设置时还会检查语音热键和翻译热键有没有撞车。

---

## 配置大模型（可选）

支持任何 OpenAI 兼容接口，在设置界面填就行，也可以直接改 `config.toml`：

```toml
[llm]
api_url = "https://api.deepseek.com/v1"
api_key = "sk-..."
model   = "deepseek-chat"
enabled = true
```

国内可用：[DeepSeek](https://deepseek.com)、[SiliconFlow](https://siliconflow.cn)、[通义千问](https://dashscope.aliyun.com)

> **不配也能用。** 没有 API Key 时程序自动降级为纯本地转录，标点、翻译、数字规整照常工作，
> 只是不做语义级的润色和排版。

配好之后，格式化输出会替你做这些事：去掉「嗯 / 那个」这类语气词、把改口重说的半句话理顺、
纠正明显的同音错别字、按场景决定要不要分段和列表。**它不会改你的意思** ——
数字、单位、否定词、专有名词、路径和代码标识符都在保护范围里。

---

## 翻译

录音时按住翻译热键，说完直接出译文。

- **目标语言**由 `config.toml` 的 `translation.target_language` 决定（`Chinese` / `English`）。
  说的话如果已经是目标语言，就原样输出不翻译 —— 所以设成 `English` 时，说中文出英文、说英文照旧
- **优先用你配的大模型**；没配就用本地 opus-mt 模型，**完全离线**
- 长文本自动分段，不会说到一半被截断

```toml
[translation]
enabled = true
target_language = "English"   # 想反过来就改成 "Chinese"
```

> 目前这一项还没有界面开关，只能改配置文件；「说什么语言都自动互译」在待办里。

---

## 发布包里有什么

```
Feiyin-IME/
├── feiyin-ime.exe          # 主程序（托盘常驻）
├── feiyin-ime-ui.exe       # 设置界面（Tauri + React）
├── crash-reporter.exe      # 崩溃报告
├── *.dll                   # 运行时依赖
├── config.toml             # 你的配置（首次启动自动生成）
├── wordbook.sqlite         # 你的词库
├── scene-rules.toml        # 场景规则（可自己改）
├── itn-rules.toml          # 数字/成语规则（可自己改）
└── models/
    ├── sherpa-onnx-sense-voice-funasr-nano-int8-*/  # 语音识别（必需，~254MB）
    ├── punct-ct-transformer-zh/                     # 标点补全（可选，~79MB）
    ├── opus-mt-zh-en/ 与 opus-mt-en-zh/             # 离线翻译（可选，各 ~153MB）
    └── silero-vad/                                  # 语音端点检测（~1MB）
```

**所有资源都从 exe 所在目录读取**，整个文件夹可以随便挪、拷到 U 盘、放多份互不干扰。

---

## 自己构建

### 环境

- Rust stable（1.75+）
- Node.js 18+
- Windows SDK + VS Build Tools（C++ 桌面开发工作负载）

### 构建

```bash
# 初始化开发环境（建立模型 / DLL 链接）
PowerShell -File scripts/init-publish.ps1

# 检查编译
cargo check

# Release 构建
build.bat
```

### 🔴 新 clone 必须执行一次

仓库自带密钥 / 隐私扫描闸门（pre-commit / pre-push），但 `core.hooksPath` **不会被 clone 继承**，
必须手动配一次才生效：

```bash
git config core.hooksPath scripts/git-hooks
```

---

## 更新日志

完整变更见 [CHANGELOG](CHANGELOG.md)，最新版本说明见 [v0.9.0 Release](https://github.com/Cdexs/Feiyin-IME/releases/tag/v0.9.0)。

## License

MIT License — 详见 [LICENSE](LICENSE)
