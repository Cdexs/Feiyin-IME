import { render, screen } from "@testing-library/react";
import AboutPage from "../pages/About";

describe("AboutPage", () => {
  const mockConfig = {
    ui_language: "Chinese",
    auto_start: false,
    hotkey: { mode: "toggle", vk_code: 0x78, modifiers: 0 },
    audio: {},
    llm: {},
    wordbook: [],
  };

  const mockUpdateConfig = vi.fn();

  it("renders page title in Chinese", () => {
    render(<AboutPage config={mockConfig} updateConfig={mockUpdateConfig} />);
    const title = screen.getByText("飞音语音输入");
    expect(title).toBeInTheDocument();
  });

  it("renders page title in English", () => {
    const enConfig = { ...mockConfig, ui_language: "English" };
    render(<AboutPage config={enConfig} updateConfig={mockUpdateConfig} />);
    const title = screen.getByText("Feiyin Voice Input");
    expect(title).toBeInTheDocument();
  });

  it("renders version information", async () => {
    render(<AboutPage config={mockConfig} updateConfig={mockUpdateConfig} />);
    const versionLabel = await screen.findByText("版本");
    expect(versionLabel).toBeInTheDocument();
    const versionValue = await screen.findByText("v0.5.3");
    expect(versionValue).toBeInTheDocument();
  });

  it("renders check updates button", () => {
    render(<AboutPage config={mockConfig} updateConfig={mockUpdateConfig} />);
    const btn = screen.getByText("检查更新");
    expect(btn).toBeInTheDocument();
    expect(btn.tagName).toBe("BUTTON");
  });

  it("renders check updates button in English", () => {
    const enConfig = { ...mockConfig, ui_language: "English" };
    render(<AboutPage config={enConfig} updateConfig={mockUpdateConfig} />);
    const btn = screen.getByText("Check for updates");
    expect(btn).toBeInTheDocument();
    expect(btn.tagName).toBe("BUTTON");
  });

  it("does not render engine info or copyright", () => {
    render(<AboutPage config={mockConfig} updateConfig={mockUpdateConfig} />);
    expect(screen.queryByText("SenseVoice / Paraformer")).not.toBeInTheDocument();
    expect(screen.queryByText(/© 2026 CodeLab/)).not.toBeInTheDocument();
    expect(screen.queryByText("构建日期")).not.toBeInTheDocument();
  });
});
