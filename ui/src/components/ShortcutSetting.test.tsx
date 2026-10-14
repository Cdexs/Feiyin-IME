// SC-LLM-485 / SC-PUNCT-486：功能开关快捷方式 —— 显示、对话框录制、冲突红字、保存。
import { render, screen, fireEvent, within } from "@testing-library/react";
import { vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import ShortcutSetting, {
  shortcutConflict,
  shortcutInactive,
  shortcutDisplay,
  mainKeyVk,
} from "./ShortcutSetting";
import LlmPage from "../pages/Llm";
import { zhHans } from "../i18n/zh-Hans";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function makeConfig(overrides: Record<string, any> = {}) {
  return {
    ui_language: "Chinese",
    hotkey: { mode: "Toggle", vk_code: 0x78, modifiers: 0, ...overrides.hotkey },
    translation: { enabled: false, vk_code: 0, ...overrides.translation },
    llm: {
      enabled: false,
      api_url: "https://api.openai.com/v1",
      api_key: "sk-test",
      model: "gpt-4",
      connectivity_verified: true,
    },
    punctuation: { enabled: true, strip_trailing: false },
    ...(overrides.shortcuts ? { shortcuts: overrides.shortcuts } : {}),
  };
}

function renderSetting(config: any, action: "llm_toggle" | "strip_trailing_toggle" = "llm_toggle") {
  const updateConfig = vi.fn();
  render(<ShortcutSetting config={config} updateConfig={updateConfig} action={action} />);
  return { updateConfig };
}

function openDialog() {
  fireEvent.click(screen.getByRole("button", { name: zhHans.shortcut_link }));
  return screen.getByRole("dialog");
}

function startCapture(dialog: HTMLElement) {
  const keyBtn = within(dialog).getByRole("button", { name: /Ctrl|Alt|未设置/ });
  fireEvent.click(keyBtn);
  return within(dialog).getByRole("textbox");
}

function captureCalls() {
  return (invoke as any).mock.calls.filter((c: any[]) => c[0] === "set_shortcut_capture").map((c: any[]) => c[1].active);
}

const saveButton = (dialog: HTMLElement) => within(dialog).getByRole("button", { name: zhHans.shortcut_save });

describe("ShortcutSetting — 行内显示", () => {
  beforeEach(() => vi.clearAllMocks());

  it("SC-UI-001: 无 shortcuts 段时显示默认「（快捷方式: 右Ctrl+1）」，快捷方式是可点链接", () => {
    renderSetting(makeConfig());
    const link = screen.getByRole("button", { name: "快捷方式" });
    const inline = link.closest(".shortcut-inline") as HTMLElement;
    expect(inline).toHaveTextContent("（快捷方式: 右Ctrl+1）");
  });

  it("SC-UI-002: 句尾开关默认 右Ctrl+2；自定义值按 左/右 Ctrl/Alt 显示", () => {
    renderSetting(makeConfig(), "strip_trailing_toggle");
    expect(screen.getByText("右Ctrl+2")).toBeInTheDocument();
  });

  it("SC-UI-003: 配置里的快捷方式与录音热键冲突 → 行内红字提示不可用", () => {
    renderSetting(makeConfig({ hotkey: { vk_code: 0xa3 } }));
    expect(screen.getByText(zhHans.shortcut_inline_conflict)).toBeInTheDocument();
  });
});

describe("ShortcutSetting — 设置对话框", () => {
  beforeEach(() => vi.clearAllMocks());

  it("SC-UI-010: 点链接弹出模态对话框，大按钮显示当前快捷方式；未改动时保存不可点", () => {
    renderSetting(makeConfig());
    const dialog = openDialog();
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(within(dialog).getByRole("button", { name: "右Ctrl+1" })).toBeInTheDocument();
    expect(within(dialog).getByText(zhHans.llm_enable)).toBeInTheDocument();
    expect(saveButton(dialog)).toBeDisabled();
  });

  it("SC-UI-011: 录制 左Ctrl+5 → 按钮显示新组合，保存写回 shortcuts.llm_toggle", () => {
    const { updateConfig } = renderSetting(makeConfig());
    const dialog = openDialog();
    const box = startCapture(dialog);
    expect(captureCalls()).toEqual([true]);
    fireEvent.keyDown(box, { code: "ControlLeft" });
    expect(box).toHaveTextContent("左Ctrl+");
    fireEvent.keyDown(box, { code: "Digit5" });
    expect(captureCalls()).toEqual([true, false]);
    expect(within(dialog).getByRole("button", { name: "左Ctrl+5" })).toBeInTheDocument();
    fireEvent.click(saveButton(dialog));
    expect(updateConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        shortcuts: {
          llm_toggle: { modifier: 0xa2, key: 0x35 },
          strip_trailing_toggle: { modifier: 0xa3, key: 0x32 },
        },
      })
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("SC-UI-012: 功能键已是录音热键（右Alt）→ 对话框红字提示重新设置，保存不可点", () => {
    renderSetting(makeConfig({ hotkey: { vk_code: 0xa5 } }));
    const dialog = openDialog();
    const box = startCapture(dialog);
    fireEvent.keyDown(box, { code: "AltRight" });
    fireEvent.keyDown(box, { code: "KeyK" });
    const alert = within(dialog).getByRole("alert");
    expect(alert).toHaveTextContent("「右Alt」已设置为录音热键，请重新设置快捷方式");
    expect(saveButton(dialog)).toBeDisabled();
  });

  it("SC-UI-013: 打开时当前快捷方式就冲突 → 直接显示红字", () => {
    renderSetting(makeConfig({ hotkey: { vk_code: 0xa3 } }));
    const dialog = openDialog();
    expect(within(dialog).getByRole("alert")).toHaveTextContent("「右Ctrl」已设置为录音热键");
  });

  it("SC-UI-014: 功能键是已启用的翻译热键 → 红字", () => {
    renderSetting(makeConfig({ translation: { enabled: true, vk_code: 0xa4 } }));
    const dialog = openDialog();
    const box = startCapture(dialog);
    fireEvent.keyDown(box, { code: "AltLeft" });
    fireEvent.keyDown(box, { code: "KeyT" });
    expect(within(dialog).getByRole("alert")).toHaveTextContent("「左Alt」已设置为翻译热键");
  });

  it("SC-UI-015: 整个组合与录音热键（Ctrl+1）相同 → 红字", () => {
    renderSetting(makeConfig({ hotkey: { vk_code: 0x31, modifiers: 0x0002 } }));
    const dialog = openDialog();
    expect(within(dialog).getByRole("alert")).toHaveTextContent("「右Ctrl+1」与录音热键相同");
  });

  it("SC-UI-016: 与另一个开关的快捷方式重复 → 红字点名对方", () => {
    renderSetting(makeConfig(), "strip_trailing_toggle");
    const dialog = openDialog();
    const box = startCapture(dialog);
    fireEvent.keyDown(box, { code: "ControlRight" });
    fireEvent.keyDown(box, { code: "Digit1" });
    expect(within(dialog).getByRole("alert")).toHaveTextContent(`与「${zhHans.llm_enable}」的快捷方式重复`);
    expect(saveButton(dialog)).toBeDisabled();
  });

  it("SC-UI-017: 不按功能键直接按字母 / 夹带 Shift → 提示须按住一个 Ctrl/Alt，继续录制", () => {
    renderSetting(makeConfig());
    const dialog = openDialog();
    const box = startCapture(dialog);
    fireEvent.keyDown(box, { code: "KeyA" });
    expect(within(dialog).getByRole("alert")).toHaveTextContent(zhHans.shortcut_need_modifier);
    fireEvent.keyDown(box, { code: "ShiftLeft" });
    fireEvent.keyDown(box, { code: "ControlRight" });
    fireEvent.keyDown(box, { code: "Digit3" });
    expect(within(dialog).getByRole("alert")).toHaveTextContent(zhHans.shortcut_need_modifier);
    expect(within(dialog).getByRole("textbox")).toBeInTheDocument();
  });

  it("SC-UI-018: AltGr（合成 左Ctrl + 右Alt）记成 右Alt", () => {
    renderSetting(makeConfig());
    const dialog = openDialog();
    const box = startCapture(dialog);
    fireEvent.keyDown(box, { code: "ControlLeft" });
    fireEvent.keyDown(box, { code: "AltRight" });
    fireEvent.keyDown(box, { code: "ControlLeft" }); // 按住连发
    fireEvent.keyDown(box, { code: "AltRight" });
    fireEvent.keyDown(box, { code: "Digit3" });
    expect(within(dialog).getByRole("button", { name: "右Alt+3" })).toBeInTheDocument();
  });

  it("SC-UI-019: 录制中按 Esc 退出录制并释放主程序拦截；取消不写配置", () => {
    const { updateConfig } = renderSetting(makeConfig());
    const dialog = openDialog();
    const box = startCapture(dialog);
    fireEvent.keyDown(box, { code: "Escape", key: "Escape" });
    expect(captureCalls()).toEqual([true, false]);
    expect(within(dialog).getByRole("button", { name: "右Ctrl+1" })).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: zhHans.shortcut_cancel }));
    expect(updateConfig).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("SC-UI-020: 录制中关掉对话框也释放主程序拦截", () => {
    renderSetting(makeConfig());
    const dialog = openDialog();
    startCapture(dialog);
    fireEvent.click(dialog.closest(".modal-overlay") as HTMLElement);
    expect(captureCalls()).toEqual([true, false]);
  });
});

describe("ShortcutSetting — 与主程序同一判据", () => {
  it("SC-UI-030: 冲突判据镜像 Rust conflict_with", () => {
    const cfg = (hotkey: any, translation: any = {}) => ({ hotkey, translation });
    const rctrl1 = { modifier: 0xa3, key: 0x31 };
    expect(shortcutConflict(rctrl1, cfg({ vk_code: 0x78, modifiers: 0 }))).toBeNull();
    expect(shortcutConflict(rctrl1, cfg({ vk_code: 0xa3, modifiers: 0 }))).toBe("recording");
    expect(shortcutConflict(rctrl1, cfg({ vk_code: 0x78 }, { enabled: true, vk_code: 0xa3 }))).toBe("translation");
    expect(shortcutConflict(rctrl1, cfg({ vk_code: 0x78 }, { enabled: false, vk_code: 0xa3 }))).toBeNull();
    expect(shortcutConflict(rctrl1, cfg({ vk_code: 0x31, modifiers: 0x0002 }))).toBe("combo");
    expect(shortcutConflict(rctrl1, cfg({ vk_code: 0x31, modifiers: 0x0006 }))).toBeNull();
    const ralt1 = { modifier: 0xa5, key: 0x31 };
    expect(shortcutConflict(ralt1, cfg({ vk_code: 0x31, modifiers: 0x0001 }))).toBe("combo");
    expect(shortcutConflict(ralt1, cfg({ vk_code: 0x31, modifiers: 0x0002 }))).toBeNull();
  });

  it("SC-UI-031: 两个快捷方式相同时句尾那个不生效（镜像 active_shortcuts 保留前者）", () => {
    const same = { modifier: 0xa3, key: 0x31 };
    const config = { hotkey: { vk_code: 0x78 }, shortcuts: { llm_toggle: same, strip_trailing_toggle: same } };
    expect(shortcutInactive(config, "llm_toggle")).toBe(false);
    expect(shortcutInactive(config, "strip_trailing_toggle")).toBe(true);
  });

  it("SC-UI-032: 只认字母 / 数字主键；未设置显示「未设置」", () => {
    expect(mainKeyVk("KeyZ")).toBe(0x5a);
    expect(mainKeyVk("Digit0")).toBe(0x30);
    expect(mainKeyVk("F5")).toBeUndefined();
    expect(mainKeyVk("Numpad1")).toBeUndefined();
    expect(shortcutDisplay({ modifier: 0, key: 0 }, zhHans as any)).toBe(zhHans.shortcut_not_set);
  });
});

describe("LlmPage — 快捷方式集成", () => {
  beforeEach(() => vi.clearAllMocks());

  it("SC-UI-040: 启用识别输出优化开关后显示快捷方式；点链接只开对话框、不拨动开关", () => {
    const updateConfig = vi.fn();
    render(<LlmPage config={makeConfig()} updateConfig={updateConfig} />);
    const checkbox = screen.getByRole("checkbox", { name: /启用识别输出优化/ });
    expect(checkbox).not.toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "快捷方式" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(updateConfig).not.toHaveBeenCalled();
    expect(checkbox).not.toBeChecked();
  });
});

describe("uiLanguageFromSystem — 配置未读到时的界面语言", () => {
  it("I18N-SYS-1: 按系统语言选简 / 繁 / 英", async () => {
    const { uiLanguageFromSystem } = await import("../i18n");
    expect(uiLanguageFromSystem("zh-CN")).toBe("Chinese");
    expect(uiLanguageFromSystem("zh-TW")).toBe("TraditionalChinese");
    expect(uiLanguageFromSystem("zh-Hant-HK")).toBe("TraditionalChinese");
    expect(uiLanguageFromSystem("en-US")).toBe("English");
    expect(uiLanguageFromSystem("")).toBe("English");
  });
});
