import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import App from "../../App";

// UITEST-138: 真实浏览器（Chromium）里验证「计算后样式」与「真实布局」。
// 这些在 happy-dom 里测不了（无 CSS 级联/布局引擎）—— happy-dom 下
// getComputedStyle 解析不了样式表规则，元素没有真实尺寸。

// 颜色归一化：CSS 写 #ff6b35，getComputedStyle 返回 rgb(255, 107, 53)。
// 不做逐字符串比对（最常见假红来源），归一化后比较。
function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const h = hex.replace("#", "");
  const full = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  return {
    r: parseInt(full.slice(0, 2), 16),
    g: parseInt(full.slice(2, 4), 16),
    b: parseInt(full.slice(4, 6), 16),
  };
}

function rgbStrToRgb(rgb: string): { r: number; g: number; b: number } {
  const m = rgb.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*[\d.]+)?\)/);
  if (!m) throw new Error(`cannot parse rgb: ${rgb}`);
  return { r: +m[1], g: +m[2], b: +m[3] };
}

// 每条用例单独 render —— @testing-library/react 在 globals:true 下自动注册
// afterEach(cleanup)，若在 beforeAll 渲染，第一条用例跑完 DOM 就被卸载。
// （主控定位：第 1 条过、后 4 条全查不到元素即此因）
beforeEach(async () => {
  render(<App />);
  await waitFor(() => {
    expect(screen.getByText("通用")).toBeInTheDocument();
  });
});

describe("browser: computed styles match design tokens (UITEST-138)", () => {
  it("sidebar logo icon stroke resolves from --brand-primary", () => {
    const svg = document.querySelector(".sidebar-logo-icon");
    expect(svg).toBeTruthy();
    // App.tsx:135 用 stroke="var(--brand-primary)" → 计算后应等于 #ff6b35
    const stroke = rgbStrToRgb(getComputedStyle(svg as Element).stroke);
    const token = hexToRgb("#ff6b35");
    expect(stroke.r).toBe(token.r);
    expect(stroke.g).toBe(token.g);
    expect(stroke.b).toBe(token.b);
  });

  it("active sidebar nav item color comes from --brand-primary", () => {
    const item = document.querySelector(".sidebar-nav-item.active");
    expect(item).toBeTruthy();
    // styles.css .sidebar-nav-item.active { color: var(--brand-primary); }
    const color = rgbStrToRgb(getComputedStyle(item as Element).color);
    const token = hexToRgb("#ff6b35");
    expect(color.r).toBe(token.r);
    expect(color.g).toBe(token.g);
    expect(color.b).toBe(token.b);
  });

  it("active nav item has real non-zero layout size (happy-dom can't)", () => {
    const item = document.querySelector(".sidebar-nav-item.active");
    expect(item).toBeTruthy();
    const rect = (item as Element).getBoundingClientRect();
    // 真实浏览器里侧边栏项有实际尺寸；happy-dom 下恒为 0
    expect(rect.width).toBeGreaterThan(0);
    expect(rect.height).toBeGreaterThan(0);
  });

  it("active nav item indicator bar is 3px wide (styles.css:215)", () => {
    const item = document.querySelector(".sidebar-nav-item.active");
    expect(item).toBeTruthy();
    // styles.css .sidebar-nav-item.active::before { width: 3px; }
    // 伪元素只能通过 getComputedStyle(el, "::before") 读取
    const pseudo = getComputedStyle(item as Element, "::before");
    expect(pseudo.width).toBe("3px");
    // 且指示条背景也是 brand-primary
    const bg = rgbStrToRgb(pseudo.backgroundColor);
    const token = hexToRgb("#ff6b35");
    expect(bg.r).toBe(token.r);
    expect(bg.g).toBe(token.g);
    expect(bg.b).toBe(token.b);
  });

  it("all sidebar nav items share the same left x (aligned column)", () => {
    const items = document.querySelectorAll(".sidebar-nav-item");
    expect(items.length).toBeGreaterThanOrEqual(4);
    const xs = Array.from(items).map((el) =>
      Math.round((el as Element).getBoundingClientRect().left),
    );
    // 侧边栏导航项左边缘应对齐（同一列）
    expect(new Set(xs).size).toBe(1);
  });
});