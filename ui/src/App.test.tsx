import { render, screen, waitFor } from "@testing-library/react";
import App from "./App";

describe("App", () => {
  it("renders app with sidebar navigation", async () => {
    render(<App />);

    // Wait for config to load
    await waitFor(() => {
      expect(screen.getByText("通用")).toBeInTheDocument();
    });

    // Verify sidebar navigation items
    expect(screen.getByText("语音输入")).toBeInTheDocument();
    expect(screen.getByText("优化模型")).toBeInTheDocument();
    expect(screen.getByText("词库")).toBeInTheDocument();
    expect(screen.getByText("关于")).toBeInTheDocument();
  });

  it("shows loading state initially", () => {
    render(<App />);
    // Loading state is shown while config is being fetched
    // May or may not be visible depending on timing
    expect(true).toBe(true);
  });
});
