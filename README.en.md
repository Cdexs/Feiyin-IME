# FlashVoice Input

**English | [中文](README.md)**

> Just talk. The words appear as fast as you speak them, their structure adapts to whichever app
> you're in, and the wordbook keeps learning your vocabulary — it fits you better the more you use it.

[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-blue)](https://github.com/Cdexs/Feiyin-IME)
[![Version](https://img.shields.io/github/v/release/Cdexs/Feiyin-IME?color=green&label=version)](https://github.com/Cdexs/Feiyin-IME/releases)
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

> Live word-by-word output and mid-recording editing **require the online recognition model**
> (see [Setup guide step 2](#step-2--pick-a-speech-recognition-model)); the default local fast model
> delivers the whole passage once you stop speaking.

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

### 🔢 Numbers the way people actually say them

Spoken numbers and written numbers are different things. FlashVoice converts them for you:

```
half past four, meeting          →  4:30 meeting
an hour and a half               →  1.5 hours
ten million four hundred sixty-eight thousand seven hundred forty-one  →  10468741
```

Just as importantly, **it knows when *not* to convert**. In Chinese, idioms and proper nouns that
happen to contain number characters stay as characters.

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
2. Run `feiyin-ime.exe` — a tray icon appears → **right-click → Settings**
3. Walk through the four steps below — **only the first one is required**, and you're up and running

---

## Setup guide

Settings has six pages down the left side; the four steps below cover four of them.
**Only step 1 is required** — the rest make it better, but it runs without them.

### Step 1 · Set your hotkeys (required)

**Where**: tray icon → right-click → **Settings** → **Hotkey** in the left sidebar

The page has two tabs: **Voice Hotkey** and **Translation Hotkey**. Click the big hotkey button in
the middle — it switches to "Press new hotkey..." — then just press the key you want.

Here's what we recommend:

| Key | Action | Where to set it |
|-----|--------|-----------------|
| **Right Alt** | Record. Tap to start, tap again to stop (**Toggle**) — or **hold to talk, release to finish** (**Push-to-talk**) | Voice Hotkey tab |
| **Right Ctrl** | Translate. Hold it while recording and the translated text comes out directly | Translation Hotkey tab — tick **Enable translation** first |
| `Esc` | Cancel the current recording | Fixed, nothing to set |

**Why these two keys**: both sit under your right hand and are reachable one-handed; almost nothing
else claims them, so they won't collide with existing shortcuts; and since left and right modifiers are
detected separately, your normal Left-Ctrl / Left-Alt shortcuts never trigger them by accident.

**The translation hotkey is optional** — skip it if you don't need live translation. If you do want it,
tick **Enable translation** first or the button stays greyed out. The two hotkeys can't share a key;
the app catches the collision on the spot and asks you to pick another.

> Out of the box the recording key is `F9` (Toggle) and translation is disabled — which is why this
> ten-second change is worth making first.

**Toggle vs. Push-to-talk** is chosen under **Trigger mode** on the same page. **Left and right
modifiers are separate keys** (Left Ctrl ≠ Right Ctrl) and any combination works; if the key you pick
is already claimed by another app, you'll get a prompt asking whether to use it anyway.

### Step 2 · Pick a speech recognition model

**Where**: **Voice** in the left sidebar → **ASR Model**

Two options — pick based on what you care about:

| Option | Speed | Accuracy | Network | Cost |
|--------|-------|----------|---------|------|
| **Local Model - Fast** (default) | Fast | Good enough | ❌ Fully offline | ❌ Free |
| **Online Speech Recognition - FunASR** | Low latency, **words appear as you speak** | **Highest** | ✅ Required | ✅ Metered |

**The local model ships with the release package** — it's already in `models/` after you extract,
works out of the box, and burns no quota. For everyday input it's all you need.

**To use the online model you need your own key**:

1. Create an API key on [Alibaba Cloud Bailian](https://bailian.console.aliyun.com/)
2. Paste it into **ASR API Key** on the **Voice** page
3. 🔴 **Always hit "Test Connection" afterwards** — you want to see "✓ Connected" before relying on it

> Don't switch over without a passing connection test. The first time you press the hotkey it will
> simply error out — the app deliberately does **not** silently fall back to the local model, so you
> never end up thinking you're on the high-accuracy engine when you aren't.

### Step 3 · Turn on smart output polishing (optional, recommended)

**Where**: **Format Output** in the left sidebar → switch on **Enable Format Output**

Any OpenAI-compatible endpoint works. Three fields to fill in:

| Field | What goes in it |
|-------|-----------------|
| **API URL** | The endpoint your model provider gave you |
| **API Key** | Your API key |
| **Model** | The model ID — **`deepseek-flash` recommended** (rationale below) |

Same parameters you'd use to configure a model in any other agent tool. Then **hit "Test Connection"**
and wait for "✓ Connected" before enabling it.

**It works without this**, you just lose the post-recognition polish: filler words and stutters removed,
false starts smoothed out, homophones corrected, layout adapted to whatever app you're typing into.
See [Smart output polishing](#smart-output-polishing) below for what it does and why DeepSeek Flash.

### Step 4 · Fill in your wordbook (optional)

**Where**: **Wordbook** in the left sidebar → **User** tab → **Add Entry**

Two kinds of words are worth adding:

- **Domain terms**: jargon, product codenames, project names, people and brand names — the model has
  never seen them and can't guess them from sound alone
- **Everyday words you use constantly** but that keep coming out wrong

Once added, these words are **injected into the model's context to bias recognition**, and they also
feed the post-processing correction pass. The **System** tab above is built in and read-only;
everything you add lives in **User**.

---

## Smart output polishing

**A raw transcript is what you *said*, not what you can *use*.** Real speech comes with filler words,
false starts and repetition — paste it as-is and you still have to clean it up yourself.
Turn on smart output polishing and that step goes away:

| What it does | Example |
|--------------|---------|
| **Drops filler words** | "um… so… I think maybe" → gone |
| **Fixes false starts** | "this option — no wait, that option works" → one clean sentence |
| **Corrects homophones** | using surrounding context and your own wordbook |
| **Formats for the app** | paragraphs, a list, or a single unbroken line (see the scene table above) |
| **Condenses padding** | in formal writing it merges points you made twice; chat is left alone |

**It won't change your meaning.** Numbers, units, negations, proper nouns, file paths and code
identifiers are all explicitly protected — it changes how you said it, never what you said.

### Turning it on

Any OpenAI-compatible endpoint works — fill in **API URL, API Key and Model**, then flip the switch.
Step-by-step instructions are in [Setup guide step 3](#step-3--turn-on-smart-output-polishing-optional-recommended) above.

### Recommended model: DeepSeek Flash

FlashVoice's prompts are **tuned specifically for DeepSeek Flash (V4 / V4.1 Flash)** — model ID
`deepseek-flash`. Using the [official DeepSeek API](https://deepseek.com) is what we'd suggest.

Other OpenAI-compatible models work too, but the formatting judgement — when to use a list, when to
keep everything on one line — hasn't been tuned to the same degree, so results will vary.

> **It works without this.** With no API key the app falls back to pure local transcription —
> punctuation, translation and number conversion all still work; you just don't get semantic
> polishing and layout.

---

## Translation

Hold the translation hotkey (Right Ctrl, as recommended above) while recording, and the translated
text comes out directly. **It's off by default** — tick **Enable translation** and bind a key under
**Hotkey → Translation Hotkey** first, per [Setup guide step 1](#step-1--set-your-hotkeys-required).

- **Chinese is translated to English by default.** Speech already in the target language passes
  through as-is, with no extra processing
- **Uses your configured model when available**; otherwise the local opus-mt model, **fully offline**
- Long passages are segmented automatically so nothing gets truncated

> A switch for the translation direction is being added to the settings UI.

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
[the latest release](https://github.com/Cdexs/Feiyin-IME/releases/latest).

## License

MIT License — see [LICENSE](LICENSE)
