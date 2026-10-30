import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import LlmPage, { llmCanEnable } from "./Llm";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function makeConfig(overrides: Record<string, any> = {}) {
  return {
    ui_language: "Chinese",
    // FLASH-P3-C：本机门槛只在直连阿里时生效 ⇒ 门槛用例默认直连阿里（连 flashasr 见末尾用例）。
    audio: { asr_model: "fun_asr_realtime", ...overrides.audio },
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

    const checkbox = screen.getByRole("checkbox", { name: /启用输出优化|Enable Output Optimization/ });
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

    const checkbox = screen.getByRole("checkbox", { name: /启用输出优化|Enable Output Optimization/ });
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

    const checkbox = screen.getByRole("checkbox", { name: /启用输出优化|Enable Output Optimization/ });
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

    const checkbox = screen.getByRole("checkbox", { name: /启用输出优化|Enable Output Optimization/ });
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

// 10-14 Gavin「UI和提示对话框中的文字一定都要走i18n国际化」：原硬编码的前置条件提示 /「未测试」改走 i18n。
describe("LlmPage — i18n", () => {
  it("I18N-LLM-1: 英文界面的前置条件提示与未测试状态为英文", () => {
    const config = { ...makeConfig({ llm: { connectivity_verified: undefined } }), ui_language: "English" };
    render(<LlmPage config={config} updateConfig={vi.fn()} />);
    expect(screen.getByText(/Fill in the API URL, API key and model/)).toBeInTheDocument();
    expect(screen.getByText("Not tested")).toBeInTheDocument();
    expect(screen.queryByText(/请先完整填写/)).not.toBeInTheDocument();
  });
});

// FLASH-P3-C：输出优化页在两种连接方式下的门槛差异 + 服务端大模型状态。
describe("LlmPage — 连 flashasr / 直连阿里", () => {
  const GATE = "请先完整填写接口地址、接口密钥、模型名称，并通过连接测试。";
  const flashConfig = () =>
    makeConfig({
      audio: { asr_model: "feiyin_server" },
      llm: { api_url: "", api_key: "", model: "", connectivity_verified: false },
    });

  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("P3C-LLM-1: 连 flashasr 时本机未填 Key 也能开启，不显示本机门槛提示", async () => {
    vi.mocked(invoke).mockResolvedValue({ llm_configured: true, net: "lan" });
    const updateConfig = vi.fn();
    render(<LlmPage config={flashConfig()} updateConfig={updateConfig} />);
    expect(screen.queryByText(GATE)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox", { name: /启用输出优化/ }));
    expect(updateConfig).toHaveBeenCalledWith(
      expect.objectContaining({ llm: expect.objectContaining({ enabled: true }) })
    );
    expect(await screen.findByText(/识别服务的大模型：已配置/)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("asr_remote_status");
  });

  it("P3C-LLM-2: 连 flashasr 且服务端没配大模型 ⇒ 显示未配置与「开了也会跳过」", async () => {
    vi.mocked(invoke).mockResolvedValue({ llm_configured: false, net: "lan" });
    render(<LlmPage config={flashConfig()} updateConfig={vi.fn()} />);
    expect(await screen.findByText(/识别服务的大模型：未配置/)).toBeInTheDocument();
    expect(screen.getByText("识别服务没有配置大模型，开了也会跳过。")).toBeInTheDocument();
  });

  it("P3C-LLM-3: 连 flashasr 时 API 配置卡收起并说明，点「展开配置」才显示输入框", async () => {
    vi.mocked(invoke).mockRejectedValue("unreachable");
    render(<LlmPage config={flashConfig()} updateConfig={vi.fn()} />);
    expect(screen.getByText(/这里的配置只在直连阿里时使用/)).toBeInTheDocument();
    expect(screen.queryByPlaceholderText("sk-...")).not.toBeInTheDocument();
    expect(await screen.findByText(/暂时取不到识别服务的状态/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "展开配置" }));
    expect(screen.getByPlaceholderText("sk-...")).toBeInTheDocument();
  });

  it("P3C-LLM-4: 直连阿里维持本机门槛，不查服务端状态", () => {
    const config = makeConfig({ llm: { connectivity_verified: false } });
    const updateConfig = vi.fn();
    render(<LlmPage config={config} updateConfig={updateConfig} />);
    fireEvent.click(screen.getByRole("checkbox", { name: /启用输出优化/ }));
    expect(updateConfig).not.toHaveBeenCalled();
    expect(screen.getByText(GATE)).toBeInTheDocument();
    expect(screen.queryByText(/识别服务的大模型/)).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("asr_remote_status");
  });

  it("P3C-LLM-5: llmCanEnable 纯函数", () => {
    expect(llmCanEnable("feiyin_server", {})).toBe(true);
    expect(llmCanEnable("fun_asr_realtime", {})).toBe(false);
    expect(
      llmCanEnable("fun_asr_realtime", { connectivity_verified: true, api_url: "u", api_key: "k", model: "m" })
    ).toBe(true);
  });
});
