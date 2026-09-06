/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { playwright } from "@vitest/browser-playwright";

// UITEST-138: 真实浏览器层（Chromium）测试独立配置。
// 用法：`npm run test:browser`（= vitest run -c vitest.browser.config.ts）。
// 与 `npm run test`（vite.config.ts, happy-dom）双环境并存，互不干扰。
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: process.env.TAURI_PLATFORM == "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
  test: {
    globals: true,
    include: ["src/test/browser/**/*.{test,spec}.{ts,tsx}"],
    setupFiles: "./src/test/setup.ts",
    browser: {
      enabled: true,
      provider: playwright(),
      headless: true,
      instances: [{ browser: "chromium" }],
    },
  },
});