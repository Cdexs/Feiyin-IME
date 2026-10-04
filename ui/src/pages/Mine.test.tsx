import { render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import MinePage, { formatMinutes, WeekUsage } from "./Mine";

// STATS-475：「我的」页本周统计。
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
const mockInvoke = vi.mocked(invoke);

const usage: WeekUsage = {
  week_start: "2026-09-28",
  week_end: "2026-10-04",
  speech_ms: 750_000,
  words: 2345,
  llm_calls: 17,
  local_fast: { speech_ms: 120_000, words: 300 },
  online_asr: { speech_ms: 330_000, words: 1045 },
  local_streaming: { speech_ms: 300_000, words: 1000 },
};

describe("MinePage — STATS-475 本周使用统计", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("STATS-UI-001: 总计行 = 时长（分钟）/ 字数 / LLM 调用次数", async () => {
    mockInvoke.mockImplementation(async (cmd: string) =>
      cmd === "get_usage_week" ? usage : null
    );
    render(<MinePage config={{ ui_language: "Chinese" }} updateConfig={vi.fn()} />);
    const total = await screen.findByTestId("mine-total");
    expect(total.textContent).toBe(
      "本周总共使用时长：12.5 分钟、输入字数：2345 字、优化LLM调用：17 次"
    );
    expect(mockInvoke).toHaveBeenCalledWith("get_usage_week");
    expect(screen.getByText("2026-09-28 ~ 2026-10-04", { exact: false })).toBeInTheDocument();
  });

  it("STATS-UI-002: 三项分项按本地快速 / 在线 ASR / 本地流式顺序", async () => {
    mockInvoke.mockImplementation(async (cmd: string) =>
      cmd === "get_usage_week" ? usage : null
    );
    render(<MinePage config={{ ui_language: "Chinese" }} updateConfig={vi.fn()} />);
    expect((await screen.findByTestId("mine-row-local_fast")).textContent).toBe(
      "本地快速模型识别时长：2 分钟，输入字数：300 字"
    );
    expect(screen.getByTestId("mine-row-online_asr").textContent).toBe(
      "在线ASR模型服务时长：5.5 分钟，输入字数：1045 字"
    );
    expect(screen.getByTestId("mine-row-local_streaming").textContent).toBe(
      "本地流式模型识别时长：5 分钟，输入字数：1000 字"
    );
  });

  it("STATS-UI-003: 读取失败显示错误、不崩", async () => {
    mockInvoke.mockRejectedValue(new Error("db locked"));
    render(<MinePage config={{ ui_language: "Chinese" }} updateConfig={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("读取使用统计失败")).toBeInTheDocument());
    expect(screen.queryByTestId("mine-total")).toBeNull();
  });

  it("STATS-UI-004: 分钟取 1 位小数，整数不带 .0", () => {
    expect(formatMinutes(0)).toBe("0");
    expect(formatMinutes(30_000)).toBe("0.5");
    expect(formatMinutes(60_000)).toBe("1");
    expect(formatMinutes(754_000)).toBe("12.6");
  });
});
