import { test as base, expect, type Page } from "@playwright/test";

import type { RecordedCall } from "../fake/control";

/**
 * Playwright's `test`, failing any test in which the page logs a warning or
 * an error or throws: those are bugs even when the UI looks right.
 */
export const test = base.extend({
  page: async ({ page }, use) => {
    // The Chromium project stands in for WebView2 on Windows with a Windows
    // user agent, and the WebKit project for WKWebView on macOS with a macOS
    // one; make `navigator.platform` agree whatever the host is (xterm.js
    // reads it, e.g. to recognise AltGr and Option).
    await page.addInitScript(() => {
      if (navigator.userAgent.includes("Windows")) {
        Object.defineProperty(navigator, "platform", { get: () => "Win32" });
      } else if (navigator.userAgent.includes("Macintosh")) {
        Object.defineProperty(navigator, "platform", { get: () => "MacIntel" });
      }
    });
    const problems: string[] = [];
    page.on("console", (message) => {
      // Headless Chromium's software GL driver reports its own performance
      // notes for WebGL canvases (the terminal); they say nothing about the app.
      if (message.text().includes("GL Driver Message")) return;
      if (message.type() === "warning" || message.type() === "error") {
        problems.push(`${message.type()}: ${message.text()}`);
      }
    });
    page.on("pageerror", (error) => problems.push(`uncaught: ${error.message}`));
    await use(page);
    expect(problems, "console warnings or errors").toEqual([]);
  },
});

export { expect };

/** Opens the app with the given fake backend scenario and waits until it is ready. */
export async function openApp(page: Page, scenario = "default"): Promise<void> {
  await page.goto(`/?scenario=${scenario}`);
  await expect(page.getByText(/^git \d+\.\d+\.\d+$/)).toBeVisible();
}

export function calls(page: Page, cmd?: string): Promise<RecordedCall[]> {
  return page.evaluate((name) => {
    const all = window.__fake.calls();
    return name ? all.filter((call) => call.cmd === name) : all;
  }, cmd);
}

export async function expectCall(page: Page, cmd: string, args: Record<string, unknown>) {
  await expect
    .poll(async () => (await calls(page, cmd)).map((call) => call.args))
    .toContainEqual(args);
}

export function menu(page: Page): Promise<string[]> {
  return page.evaluate(() => window.__fake.menu());
}

export function clickMenu(page: Page, text: string): Promise<void> {
  return page.evaluate((item) => {
    window.__fake.clickMenu(item);
  }, text);
}

export function sidebar(page: Page) {
  return page.getByRole("navigation", { name: "Repositories" });
}

export function changesList(page: Page) {
  return page.getByRole("listbox", { name: "Changed files" });
}

export function diffPane(page: Page) {
  return page.getByRole("region", { name: "Diff" });
}

/**
 * The app's command modifier for the emulated platform: Cmd in WebKit
 * (macOS user agent), Ctrl in Chromium (Windows user agent).
 */
export async function mod(page: Page): Promise<"Meta" | "Control"> {
  const mac = await page.evaluate(() => navigator.userAgent.includes("Mac"));
  return mac ? "Meta" : "Control";
}
