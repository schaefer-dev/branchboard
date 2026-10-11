import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";

// Tauri expects a fixed port and must not have its output cleared.
// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  root: "ui",
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    // WebView2 (Chromium) on Windows, WKWebView on macOS 13+.
    target: ["chrome120", "safari16"],
    sourcemap: process.env.TAURI_ENV_DEBUG === "true",
  },
  test: {
    include: ["src/**/*.test.ts"],
  },
});
