import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

// ============================================
// UI-025: 禁用 Web 特性（桌面应用质感）
// ============================================

// 禁用右键菜单
document.addEventListener("contextmenu", (e) => {
  e.preventDefault();
});

// 禁用开发者快捷键
document.addEventListener("keydown", (e) => {
  // F5
  if (e.key === "F5") {
    e.preventDefault();
    return false;
  }
  // F12
  if (e.key === "F12") {
    e.preventDefault();
    return false;
  }
  // Ctrl+Shift+I/J/C
  if (e.ctrlKey && e.shiftKey && ["I", "J", "C", "i", "j", "c"].includes(e.key)) {
    e.preventDefault();
    return false;
  }
  // Ctrl+U (查看源码)
  if (e.ctrlKey && (e.key === "u" || e.key === "U")) {
    e.preventDefault();
    return false;
  }
});

// 禁用拖拽选择文本（可选，保持桌面应用质感）
// 注意：input/textarea 内仍允许选择
document.addEventListener("selectstart", (e) => {
  const target = e.target as HTMLElement;
  if (
    target.tagName === "INPUT" ||
    target.tagName === "TEXTAREA" ||
    target.isContentEditable
  ) {
    return;
  }
  // 不阻止，但可以通过 CSS user-select: none 实现
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
