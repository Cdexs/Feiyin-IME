# Feiyin Smart Voice Input

**English | [中文](README.md)**

> A Windows system-tray voice input tool. Hotkey-triggered, local ASR + LLM optimization, ready to use out of the box.

[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-blue)](https://github.com/Cdexs/Feiyin-IME)
[![Version](https://img.shields.io/badge/version-v0.5.4-green)](https://github.com/Cdexs/Feiyin-IME/releases)
[![License](https://img.shields.io/badge/license-MIT-orange)](LICENSE)

---

## Features

| Feature | Description |
|---------|-------------|
| 🎙️ **Global Hotkey Recording** | Toggle / PTT modes, fully customizable key bindings |
| 🧠 **Local Speech Recognition** | SenseVoice multi-language model (Chinese / English / Japanese / Korean / Cantonese), INT8 quantized |
| ✨ **LLM Text Optimization** | OpenAI-compatible API for error correction, punctuation and formatting |
| 🔤 **Offline Translation** | opus-mt CT2 model, bidirectional Chinese ↔ English, auto-segmented for long text |
| 🔡 **Punctuation Restoration** | CT-Transformer ONNX model, automatically adds punctuation after transcription |
| 📖 **User Wordbook** | Custom term mappings + automatic learning of high-frequency corrections, SQLite persistence |
| 🔇 **Microphone Mute Detection** | Detects mute before hotkey press and during recording, immediate notification |
| 🌐 **Multi-language UI** | Simplified Chinese / Traditional Chinese / English |
| 💥 **Crash Reporting** | Standalone crash-reporter process, local storage + email notification |

---

## Quick Start

### Requirements

- Windows 10 / Windows 11 (64-bit)
- WebView2 Runtime (auto-installed on first launch if missing on Windows 10)
- Microphone device

### Installation

1. Download and extract the release package to any directory
2. Double-click `voice-ime.exe` — the Feiyin icon appears in the system tray
3. Press the default hotkey **F9** to start recording, press again to stop and inject text
4. Right-click the tray icon → **Settings** to configure hotkeys, LLM, translation, etc.

### Release Package Structure

```
Feiyin-IME/
├── voice-ime.exe           # Main application
├── voice-ime-ui.exe        # Settings UI (Tauri + React)
├── crash-reporter.exe      # Crash report utility
├── *.dll                   # Runtime dependencies
├── config.toml             # User configuration (auto-created on first launch)
├── wordbook.sqlite         # User wordbook database
└── models/
    ├── sherpa-onnx-sense-voice-*/   # ASR model (required, ~233MB)
    ├── opus-mt-zh-en/               # Chinese→English translation (optional, ~164MB)
    ├── opus-mt-en-zh/               # English→Chinese translation (optional, ~164MB)
    └── punct-ct-transformer-zh/     # Punctuation model (optional, ~79MB)
```

---

## Hotkeys

| Hotkey | Action |
|--------|--------|
| `F9` (default) | Start / stop recording (Toggle mode) |
| Hold `F9` | Push-to-talk: hold to record, release to stop |
| `Right Ctrl + F9` | Record with translation (requires translation hotkey config) |
| `Esc` | Cancel current recording |

Hotkeys can be customized in Settings → **General → Trigger Mode**.

---

## LLM Configuration

Supports any OpenAI-compatible API endpoint:

```toml
[llm]
api_url = "https://api.openai.com/v1"
api_key = "sk-..."
model   = "gpt-4o-mini"
enabled = true
```

> When LLM is not configured, the app gracefully falls back to local transcription-only mode — no internet required.

---

## Translation

- **Trigger**: Hold the translation hotkey (default: Right Ctrl) while recording
- **Priority**: Uses LLM translation when configured; otherwise automatically uses local opus-mt model
- **Long text**: Automatically segments text >120 characters to prevent truncation

---

## Architecture

```
voice-ime.exe (Win32 Controller)
├── Win32 message loop + RegisterHotKey global hotkeys
├── System tray (tray-icon)
├── Win32 GDI Recording Overlay
│   ├── Recording: waveform + microphone icon
│   ├── Processing: shimmer sweep animation
│   └── Error: red indicator + message
├── WASAPI audio capture (cpal)
├── SenseVoice ASR (sherpa-onnx)
├── LLM text optimization (reqwest / OpenAI API)
├── CT-Transformer punctuation (sherpa-onnx ONNX)
├── opus-mt translation engine (CTranslate2)
└── SQLite wordbook (rusqlite)

voice-ime-ui.exe (Tauri + React)
└── Settings UI (launched as subprocess by main app)

crash-reporter.exe
└── Standalone crash report utility
```

---

## Building from Source

### Prerequisites

- Rust stable (1.75+, `x86_64-pc-windows-msvc` toolchain)
- Visual Studio 2022 Build Tools with "Desktop development with C++"
- Node.js 18+

See [docs/BUILD-DEPS.md](docs/BUILD-DEPS.md) for the complete step-by-step setup guide.

### Quick Build

```powershell
# Initialize dev environment (first time only)
npm install
PowerShell -File scripts\init-publish.ps1

# Build
build.bat
```

---

## Changelog

| Version | Highlights |
|---------|-----------|
| v0.5.4 | exe rename (feiyin-ime) / new orange icon / GitHub version check / About UI redesign / ESC fix / overlay no focus-steal |
| v0.5.3 | Long-text segmented translation / mic mute detection / exe-relative paths / punctuation / Traditional Chinese UI |
| v0.5.2 | SQLite wordbook / LLM auto-learning / multi-language UI |
| v0.5.1 | Tauri v2 upgrade |
| v0.5.0 | macOS cross-platform architecture |
| v0.4.0 | UI framework: eframe → Tauri + React |
| v0.3.x | Win32 architecture / Paraformer ASR / crash reporting |

---

## License

MIT License — see [LICENSE](LICENSE)
