# FlashVoice Input

**English | [中文](README.md)**

> Just talk. The words appear as fast as you speak them — and they know which app you're typing into.

[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-blue)](https://github.com/Cdexs/Feiyin-IME)
[![Version](https://img.shields.io/badge/version-v0.9.0-green)](https://github.com/Cdexs/Feiyin-IME/releases)
[![License](https://img.shields.io/badge/license-MIT-orange)](LICENSE)

---

## Why another voice input tool

Most speech-to-text stops at turning sound into characters. But there's still a gap between how people
talk and what belongs on the screen — you say "um" and "you know", you start a sentence over halfway
through, you say "half past three" instead of `3:30`. And more importantly:
**what you want to send in a chat app and what you want to type into a terminal are not the same thing.**

FlashVoice Input is about that second half: **going from *heard* to *usable*.**

---

## What it's like to use

### 🎤 Words appear while you're still talking

Press the hotkey and text flows into a floating overlay at the pace you're speaking — not a whole
paragraph dumped after you finish, and not a mechanical fixed-rate typewriter either.
**Pause and it pauses; speed up and it keeps up.**

Misspoke? No need to start over. **Click the text in the overlay while still recording** and edit it
in place, then let it go to the app.

### 🎯 It knows what app you're in

The same sentence should look different depending on where it lands. FlashVoice detects the active
window and switches its output style automatically:

| Where you are | What it does |
|---------------|--------------|
| Chat apps (WeChat, QQ, Slack…) | Keeps it conversational — **no compression, no restructuring** |
| Terminals and IDEs | Technical tone, no pleasantries, and **never injects newlines** (so half a sentence can't get executed as a command) |
| Email, Word, documents | Proper written style, paragraphs and lists allowed, spoken-language repetition condensed away |
| AI assistants (Claude, ChatGPT…) | Treated as instructions to an agent — formal and structured |
| Browsers | Concise, web-friendly formatting |

These rules live in an external `scene-rules.toml`. **Add your own apps or change any style, restart,
done — no recompiling.**

### 🔢 Numbers the way people actually say them

Spoken numbers and written numbers are different things. FlashVoice converts them for you:

```
half past four, meeting          →  4:30 meeting
an hour and a half               →  1.5 hours
ten million four hundred sixty-eight thousand seven hundred forty-one  →  10468741
```

Just as importantly, **it knows when *not* to convert**. In Chinese, idioms and proper nouns that
happen to contain number characters stay as characters. Those rules are external too
(`itn-rules.toml`), so if you hit an edge case you can fix it yourself.

### 🏠 Local first, fully offline capable

Speech recognition, punctuation restoration and Chinese ↔ English translation **all run on your
machine**. No network required, and nothing you say has to leave your computer.
Want stronger polishing? Plug in any OpenAI-compatible model — **that's an option, not a requirement.**
An online recognition engine is also available if you want the fastest possible first character.

### 📖 It learns your vocabulary

Jargon, names and product terms getting mangled? Add them to your wordbook and they'll stick.
With formatted output enabled, it also **learns from your corrections automatically**, so you don't
have to add every one by hand.

### 🌍 Multilingual

Recognition covers Chinese, English, Japanese, Korean and Cantonese. Hold the translation hotkey while
speaking to get Chinese ↔ English translation directly. The interface itself ships in
Simplified Chinese, Traditional Chinese and English.

---

## Quick start

### You'll need

- Windows 10 / 11 (64-bit)
- A microphone
- WebView2 Runtime (installed automatically if missing)

### Three steps

1. Download the [latest release](https://github.com/Cdexs/Feiyin-IME/releases) and extract it anywhere
2. Run `feiyin-ime.exe` — a tray icon appears → right-click → **Settings**
3. Pick a recording hotkey (**Right Ctrl or Right Alt work well** — reachable one-handed, and they
   don't collide with common shortcuts), then hold it and speak

For better output quality, add a model API key on the **Formatted Output** page (see below).
It works fine without one.

### Hotkeys

| Key | Action |
|-----|--------|
| `F9` (default, configurable) | Start / stop recording (Toggle mode) |
| Hold the key | Push-to-talk — speak while held, release to finish |
| Translation hotkey while recording | Translate as you speak |
| `Esc` | Cancel the current recording |

Configure under **General → Trigger** in Settings. **Left and right modifiers are separate keys**
(Left Ctrl ≠ Right Ctrl), any key combination works, and the app checks that your recording and
translation hotkeys don't conflict.

---

## Connecting a model (optional)

Any OpenAI-compatible endpoint works. Set it in the UI, or edit `config.toml` directly:

```toml
[llm]
api_url = "https://api.deepseek.com/v1"
api_key = "sk-..."
model   = "deepseek-chat"
enabled = true
```

> **It works without this.** With no API key the app falls back to pure local transcription —
> punctuation, translation and number conversion all still work; you just don't get semantic
> polishing and layout.

With a model connected, formatted output will: strip filler words, straighten out sentences you
restarted mid-way, fix obvious homophone typos, and decide whether the result should be paragraphs or
a list based on the app you're in. **It won't change your meaning** — numbers, units, negations,
proper nouns, file paths and code identifiers are all explicitly protected.

---

## Translation

Hold the translation hotkey while recording and the translated text comes out directly.

- **The target language** is set by `translation.target_language` in `config.toml`
  (`Chinese` / `English`). Speech that is already in the target language passes through untranslated —
  so with `English`, Chinese speech becomes English and English speech stays as-is
- **Uses your configured model when available**; otherwise the local opus-mt model, **fully offline**
- Long passages are segmented automatically so nothing gets truncated

```toml
[translation]
enabled = true
target_language = "English"   # flip to "Chinese" for the other direction
```

> There's no UI control for this yet — config file only. Fully automatic two-way translation is on the
> backlog.

---

## What's in the release package

```
Feiyin-IME/
├── feiyin-ime.exe          # Main program (lives in the tray)
├── feiyin-ime-ui.exe       # Settings UI (Tauri + React)
├── crash-reporter.exe      # Crash reporter
├── *.dll                   # Runtime dependencies
├── config.toml             # Your configuration (created on first launch)
├── wordbook.sqlite         # Your wordbook
├── scene-rules.toml        # Scene rules (yours to edit)
├── itn-rules.toml          # Number / idiom rules (yours to edit)
└── models/
    ├── sherpa-onnx-sense-voice-funasr-nano-int8-*/  # Speech recognition (required, ~254MB)
    ├── punct-ct-transformer-zh/                     # Punctuation (optional, ~79MB)
    ├── opus-mt-zh-en/ and opus-mt-en-zh/            # Offline translation (optional, ~153MB each)
    └── silero-vad/                                  # Voice activity detection (~1MB)
```

**Everything loads relative to the executable's own directory**, so the folder is portable — move it,
copy it to a USB drive, or keep several independent copies side by side.

---

## Building from source

### Requirements

- Rust stable (1.75+)
- Node.js 18+
- Windows SDK + VS Build Tools (Desktop development with C++)

### Build

```bash
# Set up the dev environment (links models / DLLs)
PowerShell -File scripts/init-publish.ps1

# Type-check
cargo check

# Release build
build.bat
```

### 🔴 Run once per fresh clone

The repo ships secret/privacy scanning hooks (pre-commit / pre-push), but `core.hooksPath`
**is not inherited by a clone** — you must set it manually for the hooks to run:

```bash
git config core.hooksPath scripts/git-hooks
```

---

## Changelog

Full history in [CHANGELOG](CHANGELOG.md); latest release notes at
[v0.9.0](https://github.com/Cdexs/Feiyin-IME/releases/tag/v0.9.0).

## License

MIT License — see [LICENSE](LICENSE)
