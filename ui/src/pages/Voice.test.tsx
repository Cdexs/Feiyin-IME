import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import VoicePage from "./Voice";

// Override setup.ts mock for Voice page tests
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockUpdateConfig = vi.fn();

function renderVoice(configOverrides = {}) {
  const baseConfig = {
    ui_language: "Chinese",
    audio: { input_device: "", chinese_script: "Simplified" },
    punctuation: { enabled: true },
    ...configOverrides,
  };
  return render(
    <VoicePage config={baseConfig} updateConfig={mockUpdateConfig} />
  );
}

describe("VoicePage — Punctuation Toggle", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("PUNCT-UI-001: renders punctuation toggle with Chinese label when ui_language=Chinese", () => {
    renderVoice();
    const toggleLabel = screen.getByText("自动补全标点符号");
    expect(toggleLabel).toBeInTheDocument();
  });

  it("PUNCT-UI-002: renders punctuation toggle with English label when ui_language=English", () => {
    renderVoice({ ui_language: "English" });
    const toggleLabel = screen.getByText("Auto-punctuation");
    expect(toggleLabel).toBeInTheDocument();
  });

  it("PUNCT-UI-003: toggle is checked when punctuation.enabled=true", () => {
    renderVoice({ punctuation: { enabled: true } });
    const checkbox = screen.getByRole("checkbox");
    expect(checkbox).toBeChecked();
  });

  it("PUNCT-UI-004: toggle is unchecked when punctuation.enabled=false", () => {
    renderVoice({ punctuation: { enabled: false } });
    const checkbox = screen.getByRole("checkbox");
    expect(checkbox).not.toBeChecked();
  });

  it("PUNCT-UI-005: toggling off calls updateConfig with enabled=false", () => {
    renderVoice({ punctuation: { enabled: true } });
    const checkbox = screen.getByRole("checkbox");

    fireEvent.click(checkbox);

    expect(mockUpdateConfig).toHaveBeenCalledTimes(1);
    const callArg = mockUpdateConfig.mock.calls[0][0];
    expect(callArg.punctuation.enabled).toBe(false);
  });

  it("PUNCT-UI-006: toggling on calls updateConfig with enabled=true", () => {
    renderVoice({ punctuation: { enabled: false } });
    const checkbox = screen.getByRole("checkbox");

    fireEvent.click(checkbox);

    expect(mockUpdateConfig).toHaveBeenCalledTimes(1);
    const callArg = mockUpdateConfig.mock.calls[0][0];
    expect(callArg.punctuation.enabled).toBe(true);
  });

  it("PUNCT-UI-007: updateConfig preserves other config fields", () => {
    renderVoice({
      ui_language: "Chinese",
      audio: { input_device: "Mic A", chinese_script: "Traditional" },
      punctuation: { enabled: true },
    });
    const checkbox = screen.getByRole("checkbox");

    fireEvent.click(checkbox);

    const callArg = mockUpdateConfig.mock.calls[0][0];
    expect(callArg.ui_language).toBe("Chinese");
    expect(callArg.audio.input_device).toBe("Mic A");
    expect(callArg.audio.chinese_script).toBe("Traditional");
  });
});
