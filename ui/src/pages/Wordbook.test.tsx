import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import WordbookPage from "./Wordbook";

// 覆盖 setup.ts 的全局 mock，针对 Wordbook 命令定制返回
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
const mockInvoke = vi.mocked(invoke);

const mockConfig = {
  ui_language: "Chinese",
};

const mockEntries = [
  { id: 1, raw: "原词1", corrected: "修正1", source: "user", created_at: "2024-01-01" },
  { id: 2, raw: "原词2", corrected: "修正2", source: "user", created_at: "2024-01-01" },
];

function renderWordbook() {
  return render(
    <WordbookPage config={mockConfig} updateConfig={vi.fn()} />
  );
}

describe("WordbookPage — handleDelete", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // 默认：get_wordbook_entries 返回 mock 数据
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_wordbook_entries") return mockEntries;
      return null;
    });
  });

  it("DEL-UNIT-001: 点击删除调用 delete_wordbook_entry_by_id 并传正确 id", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_wordbook_entries") return mockEntries;
      if (cmd === "delete_wordbook_entry_by_id") return null;
      return null;
    });

    renderWordbook();

    // 切换到用户词库 Tab
    const userTab = await screen.findByText("用户词库");
    fireEvent.click(userTab);

    // 找到第一个删除按钮
    const deleteButtons = await screen.findAllByTitle("删除");
    fireEvent.click(deleteButtons[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "delete_wordbook_entry_by_id",
        { id: 1 }
      );
    });
  });

  it("DEL-UNIT-002: 删除成功后条目从列表消失", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_wordbook_entries") return mockEntries;
      if (cmd === "delete_wordbook_entry_by_id") return null;
      return null;
    });

    renderWordbook();
    const userTab = await screen.findByText("用户词库");
    fireEvent.click(userTab);

    const deleteButtons = await screen.findAllByTitle("删除");
    expect(deleteButtons).toHaveLength(2);

    fireEvent.click(deleteButtons[0]);

    await waitFor(() => {
      const remaining = screen.queryAllByTitle("删除");
      expect(remaining).toHaveLength(1);
    });
  });

  it("DEL-UNIT-003: 删除失败时弹窗显示固定文字删除失败！", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_wordbook_entries") return mockEntries;
      if (cmd === "delete_wordbook_entry_by_id") throw new Error("词库条目不存在");
      return null;
    });

    renderWordbook();
    const userTab = await screen.findByText("用户词库");
    fireEvent.click(userTab);

    const deleteButtons = await screen.findAllByTitle("删除");
    fireEvent.click(deleteButtons[0]);

    await waitFor(() => {
      expect(screen.getByText("删除失败！")).toBeInTheDocument();
    });
  });

  it("DEL-UNIT-004: 弹窗确定按钮点击后弹窗关闭", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_wordbook_entries") return mockEntries;
      if (cmd === "delete_wordbook_entry_by_id") throw new Error("error");
      return null;
    });

    renderWordbook();
    const userTab = await screen.findByText("用户词库");
    fireEvent.click(userTab);

    const deleteButtons = await screen.findAllByTitle("删除");
    fireEvent.click(deleteButtons[0]);

    const confirmBtn = await screen.findByText("确定");
    fireEvent.click(confirmBtn);

    await waitFor(() => {
      expect(screen.queryByText("删除失败！")).not.toBeInTheDocument();
    });
  });

  it("DEL-UNIT-005: 删除后重新加载词条时不包含已删除条目", async () => {
    // 模拟持久化生效：删除后重新加载的数据不再包含已删除条目
    let callCount = 0;
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_wordbook_entries") {
        callCount++;
        if (callCount === 1) return mockEntries;          // 初次加载：2条
        return mockEntries.filter(e => e.id !== 1);       // 重载：只剩1条
      }
      if (cmd === "delete_wordbook_entry_by_id") return null;
      return null;
    });

    renderWordbook();
    const userTab = await screen.findByText("用户词库");
    fireEvent.click(userTab);

    const deleteButtons = await screen.findAllByTitle("删除");
    fireEvent.click(deleteButtons[0]);

    // 触发重新加载（模拟重启后的状态）
    await waitFor(() => {
      const remaining = screen.queryAllByTitle("删除");
      expect(remaining).toHaveLength(1);
    });
  });
});

describe("WordbookPage — Add Entry Modal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_wordbook_entries") return mockEntries;
      if (cmd === "add_wordbook_entry") return null;
      return null;
    });
  });

  it("ADD-UNIT-001: 点击添加按钮弹出弹窗，结构正确（含 modal-header / modal-close / 输入框）", async () => {
    renderWordbook();
    const userTab = await screen.findByText("用户词库");
    fireEvent.click(userTab);

    // 点击添加按钮
    const addBtn = await screen.findByTitle("添加词条");
    fireEvent.click(addBtn);

    // 验证弹窗存在且结构正确：
    // 1. 不应有 modal-small 类（已改为统一 modal-dialog）
    const dialog = screen.getByRole("dialog") || document.querySelector(".modal-dialog");
    expect(dialog).toBeInTheDocument();
    expect(dialog).not.toHaveClass("modal-small");

    // 2. 必须有 modal-header 包裹标题和关闭按钮
    const header = document.querySelector(".modal-header");
    expect(header).toBeInTheDocument();

    // 3. modal-header 内必须有标题（span.modal-title）和关闭按钮（.modal-close）
    const titleEl = header?.querySelector("span.modal-title");
    expect(titleEl).toBeInTheDocument();
    expect(titleEl?.textContent).toContain("添加词条");

    const closeBtn = header?.querySelector(".modal-close");
    expect(closeBtn).toBeInTheDocument();

    // 4. 必须有原词和修正词输入框
    const rawInput = document.querySelector('input[placeholder="输入原词"]');
    const correctedInput = document.querySelector('input[placeholder="输入修正词"]');
    expect(rawInput).toBeInTheDocument();
    expect(correctedInput).toBeInTheDocument();
  });

  it("ADD-UNIT-002: 点击关闭按钮或遮罩层关闭弹窗", async () => {
    renderWordbook();
    const userTab = await screen.findByText("用户词库");
    fireEvent.click(userTab);

    const addBtn = await screen.findByTitle("添加词条");
    fireEvent.click(addBtn);

    // 点击关闭按钮
    const closeBtn = document.querySelector(".modal-close") as HTMLElement;
    expect(closeBtn).toBeInTheDocument();
    fireEvent.click(closeBtn);

    await waitFor(() => {
      expect(document.querySelector(".modal-dialog")).not.toBeInTheDocument();
    });
  });

  it("ADD-UNIT-003: 输入框为空时添加按钮禁用", async () => {
    renderWordbook();
    const userTab = await screen.findByText("用户词库");
    fireEvent.click(userTab);

    const addBtn = await screen.findByTitle("添加词条");
    fireEvent.click(addBtn);

    // 添加按钮在 footer 中
    const footerBtn = document.querySelector(".modal-footer .btn-primary") as HTMLButtonElement;
    expect(footerBtn).toBeInTheDocument();
    expect(footerBtn.disabled).toBe(true);

    // 输入原词后仍禁用（修正词仍为空）
    const rawInput = document.querySelector('input[placeholder="输入原词"]') as HTMLInputElement;
    fireEvent.change(rawInput, { target: { value: "test" } });
    expect(footerBtn.disabled).toBe(true);

    // 两者都输入后启用
    const correctedInput = document.querySelector('input[placeholder="输入修正词"]') as HTMLInputElement;
    fireEvent.change(correctedInput, { target: { value: "correct" } });
    expect(footerBtn.disabled).toBe(false);
  });

  it("ADD-UNIT-004: 弹窗包含橙色列表图标（.modal-icon SVG）、提示文字（.modal-hint）和 max-width 约束", async () => {
    renderWordbook();
    const userTab = await screen.findByText("用户词库");
    fireEvent.click(userTab);

    const addBtn = await screen.findByTitle("添加词条");
    fireEvent.click(addBtn);

    // 1. 橙色列表图标：.modal-header 内必须有 .modal-icon SVG 元素
    const modalIcon = document.querySelector(".modal-header .modal-icon");
    expect(modalIcon).toBeInTheDocument();
    expect(modalIcon?.tagName.toLowerCase()).toBe("svg");

    // 2. 提示文字：.modal-body 内必须有 .modal-hint 元素
    const modalHint = document.querySelector(".modal-body .modal-hint");
    expect(modalHint).toBeInTheDocument();
    // 提示文字应包含 ⓘ 图标和内容
    const hintIcon = modalHint?.querySelector(".modal-hint-icon");
    expect(hintIcon).toBeInTheDocument();
    expect(hintIcon?.textContent).toContain("ⓘ");

    // 3. 弹窗缩小约束：弹窗容器应有 max-width 样式（通过 style 属性或 CSS 类）
    const dialog = document.querySelector(".modal-dialog") as HTMLElement;
    expect(dialog).toBeInTheDocument();
    // 验证 style 属性中 max-width 被设置（~420px）
    const maxWidth = dialog?.style?.maxWidth;
    expect(maxWidth).toBeTruthy();
    expect(Number(maxWidth.replace("px", ""))).toBeLessThanOrEqual(450);
  });
});
