# Changelog

## v0.3.3 (2026-04-16) — UI Polish & LLM Fault Tolerance

### Fixed

- BUG-015: Hotkey not responding for ~15 seconds after startup → switched to PeekMessageW + sleep polling
- BUG-016: Input device selection not updating display → fixed ComboBox index calculation
- BUG-017: LLM continuous failures wasting resources → auto-disable after 3 consecutive failures

### UI

- UI-010: Settings window width increased from 780px to 880px
- UI-011: Overlay window corner radius increased from 8px to 12px (smoother edges)
- UI-012: Cancel button size reduced to 75% of original
- UI-013: Section titles: removed bullet dot, increased font size to 18.5, lighter weight
- UI-014: LLM config panel: hide test prompt when LLM disabled, show red warning for unavailable state

### Added

- `-debug` command line flag for debug logging to `%LOCALAPPDATA%\voice-ime\debug.log`
- LLM failure counter (`consecutive_failures`) and unavailable flag (`marked_unavailable`) in config
- Auto-save config when marking LLM unavailable

### Architecture

- Hotkey thread: GetMessageW → PeekMessageW + 10ms sleep (non-blocking)
- LLM pipeline: check `marked_unavailable` before calling, skip if flagged
- User can reset LLM state by modifying config or re-testing connectivity

---

## v0.3.2 (2026-04-15) — ASR Engine Upgrade: Paraformer

### Changed

- **ASR Engine**: Whisper.cpp → sherpa-onnx Paraformer (zh-int8-2025-10-07)
- Model size: ~238MB (auto-download on first run)
- Chinese CER improved from ~18-20% (Whisper) to ~3-5% (Paraformer)

### Architecture

- Added `sherpa-onnx` dependency (v1.12, shared-MD build)
- Required DLLs: `onnxruntime.dll`, `sherpa-onnx-c-api.dll`, etc.
- Binary size reduced: 15.2MB → 14.5MB (removed Whisper)

### Added

- `enable_streaming` config option (placeholder for future 2-pass mode)
- LLM optimization as second pass after ASR transcription

### Fixed

- BUG-014: Silence detection default changed to 8000ms
- Config reload on each recording start (ensures settings applied immediately)

---

## v0.2.0 (2026-04-15) — Bug Fix + UI Polish

### Architecture

- Main lifecycle owner switched from eframe to Win32 controller (DEC-001)
- Settings window decoupled as standalone `--settings-ui` entry (DEC-002)
- Recording/Processing overlay switched to native Win32 layered window (DEC-003)
- Hotkey switched from `WH_KEYBOARD_LL` to `RegisterHotKey` (DEC-004)
- Unified shutdown protocol managed by controller (DEC-005)

### Fixed

- BUG-001: Settings window not showing → decoupled as standalone UI entry
- BUG-002: Tray exit not closing main process → controller unified shutdown
- BUG-003: Recording overlay not showing after hotkey → native Win32 overlay
- BUG-007: Restored capsule-style overlay visual
- BUG-008: Restored cancel button with independent hitbox
- BUG-009: `llm::probe()` now retries 3 times with error classification
- BUG-010: Chinese script drift (Simplified/Traditional) → zhconv deterministic conversion
- BUG-011: LLM test success not immediately saved to config
- BUG-012: LLM timeout too short → 5s connect + 30s×3 request
- BUG-013: Recording cancel button invisible → dark red rounded button with hand-drawn X
- BUG-014: Silence detection default 1200ms too short → changed to 8000ms

### UI

- UI-004: Settings window light theme + layout reorganization
- UI-005: LLM test button moved below model input field
- UI-006: Recording overlay size reduced (272x30) + cancel button fully visible
- UI-007: Overlay size unified + shimmer animation for processing state
- UI-008: FocusLost preview redesigned (320x64 dark panel + copy button)
- UI-009: FocusLost 8px rounded corners + ESC cancel during recording + simplified text

### Added

- `assets/icons/app.ico` for Inno Setup installer
- `src/text_normalizer.rs` for Chinese script normalization (zhconv)
- ESC key to cancel recording during active recording
- Shimmer animation for "Processing..." overlay state

---

## v0.1.1 (2026-04-13) — Tray Icon & Race Fix

### Added

- System tray icon (programmatically generated 32×32 blue microphone icon; no external asset required)
- Tray right-click context menu: "设置 (Settings)" and "退出 (Exit)"
- Tray double-click opens Settings window
- Tray tooltip updates to reflect current state (录音中 / 处理中 / 待机)
- OS close button on settings window minimizes to tray instead of exiting
- "最小化到托盘" button in Settings panel

### Fixed

- Hotkey race condition: second hotkey press during active recording now correctly stops recording
  via dedicated Dispatcher thread that owns hotkey channel and can set stop_signal while pipeline blocks

---

## v0.1.0 (2026-04-13) — Initial Release

### Added

- Local Whisper Base model voice transcription (auto-download on first run)
- OpenAI-compatible LLM text optimization with configurable API URL/Key/Model
- System Prompt: professional voice correction + punctuation + Markdown formatting + list structuring
- Two hotkey trigger modes: Toggle (press once/press again) and Push-to-Talk (hold/release)
- Configurable global hotkey (F9, F10, Ctrl+Space, Alt+`, Ctrl+Alt+V)
- Recording overlay bar at bottom of screen with audio waveform visualization
- Escape key to cancel recording at any time
- Focus-loss detection: when target window loses focus during processing, text is shown in a preview window with Copy button
- Custom word library: manual entry + auto-learn from post-injection corrections
- Multi-language UI: Chinese (Simplified) and English
- Multi-language transcription: zh / en / ja / ko / auto
- Settings window (accessible via --settings flag or on first run)
- Config persisted to %APPDATA%\voice-ime\config.toml
- Word library stored in SQLite at %APPDATA%\voice-ime\wordbook.sqlite
- Text injection: clipboard+Ctrl+V (primary) with SendInput Unicode fallback
- Inno Setup installer script

### Architecture

- Main binary: ~13MB (stripped release build)
- All voice processing done locally (no audio uploaded)
- Three threads: main (egui), hotkey listener, pipeline
- Tokio async runtime for LLM HTTP calls
