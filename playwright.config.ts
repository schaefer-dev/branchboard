import { defineConfig, devices } from "@playwright/test";

const PORT = 1430;
const ci = Boolean(process.env.CI);

/**
 * UI end-to-end tests: the real frontend in WebKit (≈ WKWebView on macOS) and
 * Chromium (≈ WebView2 on Windows) against the fake backend in `ui/e2e/fake`.
 */
export default defineConfig({
  testDir: "ui/e2e/specs",
  outputDir: "e2e-results/artifacts",
  fullyParallel: true,
  forbidOnly: ci,
  retries: ci ? 1 : 0,
  reporter: ci
    ? [["list"], ["html", { open: "never", outputFolder: "e2e-results/report" }]]
    : "list",
  use: {
    baseURL: `http://localhost:${String(PORT)}`,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      name: "webkit",
      use: { ...devices["Desktop Safari"], viewport: { width: 1280, height: 800 } },
    },
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } },
    },
  ],
  webServer: {
    command: `pnpm exec vite --mode e2e --port ${String(PORT)} --strictPort`,
    url: `http://localhost:${String(PORT)}`,
    reuseExistingServer: !ci,
  },
});
