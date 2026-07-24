import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import LlmPage from "./Llm";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function makeConfig(overrides: Record<string, any> = {}) {
  return {
    ui_language: "Chinese",
    llm: {
      enabled: false,
      api_url: "https://api.openai.com/v1",
      api_key: "sk-test-key-123456",
      model: "gpt-4",
      connectivity_verified: true,
      ...overrides.llm,
    },
    scene: {
      enabled: true,
      send_window_title: false,
      ...overrides.scene,
    },
  };
}

describe("LlmPage — gate validation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("GATE-001: connectivity_verified=false 时点击开关不启用，显示 gate hint", async () => {
    const updateConfig = vi.fn();
    const config = makeConfig({ llm: { connectivity_verified: false } });
    render(<LlmPage config={config} updateConfig={updateConfig} />);

    const checkbox = screen.getByRole("checkbox", { name: /启用格式化输出|Enable Format Output/ });
    fireEvent.click(checkbox);

    expect(
      screen.getByText(
        "请先完整填写接口地址、接口密钥、模型名称，并通过连接测试。"
      )
    ).toBeInTheDocument();
  });

  it("GATE-002: api_url 为空且 verified=true 仍拦截", async () => {
    const updateConfig = vi.fn();
    const config = makeConfig({ llm: { api_url: "" } });
    render(<LlmPage config={config} updateConfig={updateConfig} />);

    const checkbox = screen.getByRole("checkbox", { name: /启用格式化输出|Enable Format Output/ });
    fireEvent.click(checkbox);

    expect(
      screen.getByText(
        "请先完整填写接口地址、接口密钥、模型名称，并通过连接测试。"
      )
    ).toBeInTheDocument();
  });

  it("GATE-003: 三项齐全 + verified=true 可开启，gate hint 不显示", async () => {
    const config = makeConfig();
    const updateConfig = vi.fn();
    render(<LlmPage config={config} updateConfig={updateConfig} />);

    const checkbox = screen.getByRole("checkbox", { name: /启用格式化输出|Enable Format Output/ });
    fireEvent.click(checkbox);

    expect(updateConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        llm: expect.objectContaining({ enabled: true }),
      })
    );
    expect(
      screen.queryByText(
        "请先完整填写接口地址、接口密钥、模型名称，并通过连接测试。"
      )
    ).not.toBeInTheDocument();
  });

  it("GATE-004: 已开启状态下关闭不受限", async () => {
    const config = makeConfig({ llm: { enabled: true } });
    const updateConfig = vi.fn();
    render(<LlmPage config={config} updateConfig={updateConfig} />);

    const checkbox = screen.getByRole("checkbox", { name: /启用格式化输出|Enable Format Output/ });
    expect(checkbox).toBeChecked();
    fireEvent.click(checkbox);

    expect(updateConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        llm: expect.objectContaining({ enabled: false }),
      })
    );
  });

  it("GATE-005: 修改 api_url 重置 connectivity_verified 为 false", async () => {
    const config = makeConfig();
    const updateConfig = vi.fn();
    render(<LlmPage config={config} updateConfig={updateConfig} />);

    const urlInput = screen.getByPlaceholderText("https://api.openai.com/v1");
    fireEvent.change(urlInput, { target: { value: "https://new-url.com/v1" } });

    expect(updateConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        llm: expect.objectContaining({ connectivity_verified: false }),
      })
    );
  });

  it("GATE-006: 修改 api_key 重置 connectivity_verified 为 false", async () => {
    const config = makeConfig();
    const updateConfig = vi.fn();
    render(<LlmPage config={config} updateConfig={updateConfig} />);

    const keyInput = screen.getByPlaceholderText("sk-...");
    fireEvent.change(keyInput, { target: { value: "new-key" } });

    expect(updateConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        llm: expect.objectContaining({ connectivity_verified: false }),
      })
    );
  });

  it("GATE-007: 修改 model 重置 connectivity_verified 为 false", async () => {
    const config = makeConfig();
    const updateConfig = vi.fn();
    render(<LlmPage config={config} updateConfig={updateConfig} />);

    const modelInput = screen.getByDisplayValue("gpt-4");
    fireEvent.change(modelInput, { target: { value: "gpt-4-turbo" } });

    expect(updateConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        llm: expect.objectContaining({ connectivity_verified: false }),
      })
    );
  });
});
