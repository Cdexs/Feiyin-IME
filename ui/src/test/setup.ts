/// <reference types="vitest/globals" />
import "@testing-library/jest-dom";

// Mock Tauri window API for unit tests (TEST-FIX-002)
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setMaximizable: vi.fn().mockResolvedValue(undefined),
    metadata: {},
  }),
}));

// Mock Tauri app API for unit tests
vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn().mockResolvedValue("0.5.3"),
}));

// Mock Tauri API for unit tests
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, _args?: any) => {
    // Mock default config
    const mockConfig = {
      auto_start: false,
      hotkey: { mode: "toggle", vk_code: 0x78, modifiers: 0 },
      ui_language: "Chinese",
      audio: { device: "default", silence_threshold: 0.01, silence_duration_ms: 1500 },
      llm: { enabled: false, api_url: "", api_key: "", model: "sensevoice" },
      wordbook: [],
      overlay_opacity: 1.0,
      transcription_language: "zh",
    };

    if (cmd === "get_config") return mockConfig;
    if (cmd === "save_config") return true;
    return null;
  }),
}));
