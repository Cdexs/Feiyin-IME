import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import VoicePage from "./Voice";
import { invoke } from "@tauri-apps/api/core";

// Override setup.ts mock for Voice page tests
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockUpdateConfig = vi.fn();
const mockClipboard = { writeText: vi.fn().mockResolvedValue(undefined) };
const mockExecCommand = vi.fn();



const mockedInvoke = vi.mocked(invoke);

beforeAll(() => {
  Object.defineProperty(navigator, "clipboard", {
    value: mockClipboard,
    writable: true,
    configurable: true,
  });
  Object.defineProperty(document, "execCommand", {
    value: mockExecCommand,
    writable: true,
    configurable: true,
  });
});

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
    mockedInvoke.mockReset();
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

describe("VoicePage — ASR Model", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedInvoke.mockReset();
  });

  it("ASR-UI-001: defaults to performance when asr_model is missing", () => {
    renderVoice();
    const performanceRadio = screen.getByRole("radio", { name: /性能最优/i });
    expect(performanceRadio).toBeChecked();
  });

  it("ASR-UI-002: renders accuracy option", () => {
    renderVoice();
    const accuracyRadio = screen.getByRole("radio", { name: /准确率更高/i });
    expect(accuracyRadio).toBeInTheDocument();
  });

  it("ASR-UI-003: switching to accuracy calls updateConfig with asr_model=accuracy", () => {
    renderVoice();
    const accuracyRadio = screen.getByRole("radio", { name: /准确率更高/i });

    fireEvent.click(accuracyRadio);

    expect(mockUpdateConfig).toHaveBeenCalledTimes(1);
    const callArg = mockUpdateConfig.mock.calls[0][0];
    expect(callArg.audio.asr_model).toBe("accuracy");
  });

  it("ASR-UI-004: shows download alert when accuracy is selected and model is not ready", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_audio_devices") return [];
      if (cmd === "check_accuracy_model_ready") {
        return { ready: false, model_dir: "C:\\models\\accuracy", download_url: "https://example.com/model" };
      }
      return null;
    });

    renderVoice({ audio: { asr_model: "accuracy" } });

    await waitFor(() => {
      expect(screen.getByText(/需先下载模型/i)).toBeInTheDocument();
    });

    expect(screen.getByText("C:\\models\\accuracy")).toBeInTheDocument();
  });

  it("ASR-UI-005: hides download alert when model is ready", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_audio_devices") return [];
      if (cmd === "check_accuracy_model_ready") {
        return { ready: true, model_dir: "C:\\models\\accuracy", download_url: "https://example.com/model" };
      }
      return null;
    });

    renderVoice({ audio: { asr_model: "accuracy" } });

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("check_accuracy_model_ready");
    });

    expect(screen.queryByText(/需先下载模型/i)).not.toBeInTheDocument();
  });

  it("ASR-UI-006: gracefully handles invoke failure", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_audio_devices") return [];
      return Promise.reject(new Error("backend not ready"));
    });

    renderVoice({ audio: { asr_model: "accuracy" } });

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("check_accuracy_model_ready");
    });

    expect(screen.queryByText(/需先下载模型/i)).not.toBeInTheDocument();
  });

  it("ASR-UI-008: switching asr_model triggers re-check of model readiness", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_audio_devices") return [];
      if (cmd === "check_accuracy_model_ready") {
        return { ready: false, model_dir: "C:\\models\\accuracy", download_url: "https://example.com/model" };
      }
      return null;
    });

    const { rerender } = render(
      <VoicePage
        config={{ ui_language: "Chinese", audio: { asr_model: "performance" }, punctuation: { enabled: true } }}
        updateConfig={mockUpdateConfig}
      />
    );

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("check_accuracy_model_ready");
    });

    mockedInvoke.mockClear();

    rerender(
      <VoicePage
        config={{ ui_language: "Chinese", audio: { asr_model: "accuracy" }, punctuation: { enabled: true } }}
        updateConfig={mockUpdateConfig}
      />
    );

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("check_accuracy_model_ready");
    });
  });

  it("ASR-UI-007: copies model directory path to clipboard", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_audio_devices") return [];
      if (cmd === "check_accuracy_model_ready") {
        return { ready: false, model_dir: "C:\\models\\accuracy", download_url: "https://example.com/model" };
      }
      return null;
    });

    renderVoice({ audio: { asr_model: "accuracy" } });

    await waitFor(() => {
      expect(screen.getAllByText('复制', { exact: true })).toHaveLength(2);
    });

    const copyBtns = screen.getAllByText('复制', { exact: true });
    fireEvent.click(copyBtns[1]);

    await waitFor(() => {
      expect(mockClipboard.writeText).toHaveBeenCalledWith("C:\\models\\accuracy");
    });
  });

  it("ASR-UI-009: download button calls open_url_in_browser", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_audio_devices") return [];
      if (cmd === "check_accuracy_model_ready") {
        return { ready: false, model_dir: "C:\\models\\accuracy", download_url: "https://example.com/model" };
      }
      return null;
    });

    renderVoice({ audio: { asr_model: "accuracy" } });

    await waitFor(() => {
      expect(screen.getByText('打开下载页')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('打开下载页'));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('open_url_in_browser', { url: "https://example.com/model" });
    });
  });

  it("ASR-UI-010: displays download URL as visible code element", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_audio_devices") return [];
      if (cmd === "check_accuracy_model_ready") {
        return { ready: false, model_dir: "C:\\models\\accuracy", download_url: "https://example.com/model" };
      }
      return null;
    });

    renderVoice({ audio: { asr_model: "accuracy" } });

    await waitFor(() => {
      const codeElem = screen.getByText("https://example.com/model");
      expect(codeElem).toBeInTheDocument();
      expect(codeElem.tagName).toBe("CODE");
    });
  });

  it("ASR-UI-011: URL and dir copy states are independent", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_audio_devices") return [];
      if (cmd === "check_accuracy_model_ready") {
        return { ready: false, model_dir: "C:\\models\\accuracy", download_url: "https://example.com/model" };
      }
      return null;
    });

    renderVoice({ audio: { asr_model: "accuracy" } });

    await waitFor(() => {
      expect(screen.getAllByText('复制', { exact: true })).toHaveLength(2);
    });

    const copyBtns = screen.getAllByText('复制', { exact: true });
    fireEvent.click(copyBtns[0]);

    await waitFor(() => {
      expect(mockClipboard.writeText).toHaveBeenCalledWith("https://example.com/model");
      expect(screen.getByText('已复制')).toBeInTheDocument();
      expect(screen.getAllByText('复制', { exact: true })).toHaveLength(1);
    });

    mockClipboard.writeText.mockClear();

    const remaining = screen.getAllByText('复制', { exact: true });
    fireEvent.click(remaining[0]);

    await waitFor(() => {
      expect(mockClipboard.writeText).toHaveBeenCalledWith("C:\\models\\accuracy");
    });
  });
});
