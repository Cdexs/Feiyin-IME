import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import "../../styles.css";
import VoicePage from "../../pages/Voice";
import LlmPage from "../../pages/Llm";

// SC-BOLD-488 / SC-ALIGN-490（Gavin 10-14「快捷键显示文字和前面的按钮标签文字要中心对齐，字号要一样大，
// 其他快捷键显示文字也是一样要求」「都要加粗」）：真实 Chromium 布局里量——
//   ① 快捷方式文字与同一行标签的字号相同 ② 快捷方式整段 700 粗体
//   ③ 两段文字的竖直中线相差 ≤ 1px（量文字本身的 Range 框，不量容器盒子）。

const config = {
  ui_language: "Chinese",
  hotkey: { mode: "Toggle", vk_code: 0x78, modifiers: 0 },
  translation: { enabled: false, vk_code: 0 },
  audio: {
    device: "default",
    asr_model: "performance",
    chinese_script: "Simplified",
    overlay_opacity: 1.0,
    asr_online_api_key: "",
  },
  llm: { enabled: false, api_url: "", api_key: "", model: "", connectivity_verified: false },
  punctuation: { enabled: true, strip_trailing: false },
};

function textCenterY(el: Element): number {
  const range = document.createRange();
  range.selectNodeContents(el);
  const rects = Array.from(range.getClientRects()).filter((r) => r.height > 0);
  expect(rects.length).toBeGreaterThan(0);
  const r = rects[0];
  return r.top + r.height / 2;
}

function checkRows(expectedRows: number) {
  const rows = Array.from(document.querySelectorAll(".toggle-row"));
  expect(rows.length).toBe(expectedRows);
  for (const row of rows) {
    const label = row.querySelector(".toggle-label, .form-label") as HTMLElement;
    const shortcut = row.querySelector(".shortcut-inline") as HTMLElement;
    expect(label, "row has a label").toBeTruthy();
    expect(shortcut, "row has a shortcut").toBeTruthy();
    const ls = getComputedStyle(label);
    const ss = getComputedStyle(shortcut);
    expect(ss.fontSize, `font-size: ${label.textContent}`).toBe(ls.fontSize);
    expect(ss.fontWeight, `bold: ${label.textContent}`).toBe("700");
    const diff = Math.abs(textCenterY(label) - textCenterY(shortcut));
    expect(diff, `vertical center diff for ${label.textContent}: ${diff}px`).toBeLessThanOrEqual(1);
  }
}

describe("browser: shortcut text aligns with its label (SC-ALIGN-490)", () => {
  it("Voice page: 句尾不显示标点符号 / 中文输出 两行", () => {
    render(<VoicePage config={config} updateConfig={() => {}} />);
    checkRows(2);
  });

  it("Llm page: 启用输出优化 一行", () => {
    render(<LlmPage config={config} updateConfig={() => {}} />);
    checkRows(1);
  });
});
