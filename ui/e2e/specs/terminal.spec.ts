import type { Page } from "@playwright/test";

import { calls, changesList, expect, expectCall, mod, openApp, sidebar, test } from "./helpers";

/** The text on the embedded terminal's screen. */
function screen(page: Page): Promise<string> {
  return page.evaluate(() => {
    const buffer = window.__terminal?.buffer.active;
    if (!buffer) return "";
    const lines: string[] = [];
    for (let row = 0; row < buffer.length; row++) {
      lines.push(buffer.getLine(row)?.translateToString(true) ?? "");
    }
    return lines.join("\n");
  });
}

function typed(page: Page): Promise<string> {
  return page.evaluate(() => window.__fake.terminalInput());
}

function output(page: Page, text: string): Promise<void> {
  return page.evaluate((value) => {
    window.__fake.terminalOutput(value);
  }, text);
}

function rightSwitch(page: Page) {
  return page.getByRole("group", { name: "Right pane" }).first();
}

async function showTerminal(page: Page): Promise<void> {
  await rightSwitch(page).getByRole("button", { name: "Terminal" }).click();
  await expect.poll(() => screen(page)).toContain("herdr (fake)");
}

/**
 * Types a character the way a keyboard layout composes it: keydown with the
 * modifiers held, then keypress with the character (as WebKit and Chromium do).
 */
function compose(
  page: Page,
  init: { key: string; code: string; altKey?: boolean; ctrlKey?: boolean },
): Promise<void> {
  return page.evaluate((event) => {
    const target = document.querySelector(".xterm-helper-textarea");
    if (!target) return;
    const code = event.key.charCodeAt(0);
    const down = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...event });
    if (target.dispatchEvent(down)) {
      target.dispatchEvent(
        new KeyboardEvent("keypress", {
          bubbles: true,
          cancelable: true,
          ...event,
          charCode: code,
          keyCode: code,
        }),
      );
    }
  }, init);
}

test.describe("embedded terminal", () => {
  test("is not offered without herdr", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("group", { name: "Right pane" })).toHaveCount(0);
    await expect(page.getByRole("region", { name: "Terminal" })).toHaveCount(0);
  });

  test("runs herdr in the right column, next to the diff", async ({ page }) => {
    await openApp(page, "agents");
    await expect(calls(page, "terminal_open")).resolves.toEqual([]);

    await showTerminal(page);
    const opened = await calls(page, "terminal_open");
    expect(opened).toHaveLength(1);
    expect(opened[0]?.args.cols).toBeGreaterThan(40);
    expect(opened[0]?.args.rows).toBeGreaterThan(10);
    await expect(page.getByRole("region", { name: "Terminal" })).toBeVisible();
    await expect(page.getByRole("region", { name: "Diff" })).toBeHidden();

    // Picking a file shows its diff; the terminal keeps running.
    await changesList(page).locator('[data-key="unstaged:src/handlers/login.ts"]').click();
    await expect(page.getByRole("region", { name: "Diff" })).toBeVisible();
    await expect(page.getByRole("region", { name: "Terminal" })).toBeHidden();
    await rightSwitch(page).getByRole("button", { name: "Terminal" }).click();
    await expect(page.getByRole("region", { name: "Terminal" })).toBeVisible();
    expect(await calls(page, "terminal_open")).toHaveLength(1);
  });

  test("sends what is typed to herdr", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);
    await page.keyboard.type("ls");
    await page.keyboard.press("Enter");
    await expect.poll(() => typed(page)).toBe("ls\r");

    await output(page, "\r\nREADME.md\r\n");
    await expect.poll(() => screen(page)).toContain("README.md");
  });

  test("reports modified keys the way herdr asks for", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);

    // Without the kitty protocol Shift+Enter is indistinguishable.
    await page.keyboard.press("Shift+Enter");
    await expect.poll(() => typed(page)).toBe("\r");

    // herdr enables the kitty keyboard protocol (disambiguate escape codes).
    await output(page, "\x1b[>1u");
    await page.keyboard.press("Shift+Enter");
    await page.keyboard.press("Escape");
    await page.keyboard.press("Control+b");
    await expect.poll(() => typed(page)).toBe("\r\x1b[13;2u\x1b[27u\x1b[98;5u");
  });

  test("sends word jumps and pastes like a terminal", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);
    await page.keyboard.press("Alt+ArrowLeft");
    await expect.poll(() => typed(page)).toBe("\x1b[1;3D");

    // herdr turns on bracketed paste, so pasted text is not run line by line.
    await output(page, "\x1b[?2004h");
    await page.evaluate(() => {
      const data = new DataTransfer();
      data.setData("text/plain", "echo one\necho two");
      document
        .querySelector(".xterm-helper-textarea")
        ?.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true }));
    });
    await expect.poll(() => typed(page)).toBe("\x1b[1;3D\x1b[200~echo one\recho two\x1b[201~");
  });

  test("passes characters composed with Option or AltGr", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);
    const mac = (await mod(page)) === "Meta";
    // German layout: @ is Option+L on macOS and AltGr+Q (Ctrl+Alt) on Windows.
    const at = mac
      ? { key: "@", code: "KeyL", altKey: true }
      : { key: "@", code: "KeyQ", altKey: true, ctrlKey: true };
    const brace = mac
      ? { key: "{", code: "Digit8", altKey: true }
      : { key: "{", code: "Digit7", altKey: true, ctrlKey: true };
    for (const kitty of [false, true]) {
      if (kitty) await output(page, "\x1b[>1u");
      await page.evaluate(() => {
        document.querySelector<HTMLElement>(".xterm-helper-textarea")?.focus();
      });
      const before = (await typed(page)).length;
      await compose(page, at);
      await compose(page, brace);
      await expect.poll(async () => (await typed(page)).slice(before)).toBe("@{");
    }
  });

  test("leaves the app's shortcuts to the app", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);
    const key = await mod(page);

    if (key === "Control") {
      // Ctrl+K belongs to the terminal outside macOS.
      await page.keyboard.press("Control+k");
      await expect.poll(() => typed(page)).toBe("\x0b");
      await expect(page.getByRole("dialog")).toHaveCount(0);
    } else {
      await page.keyboard.press("Meta+k");
      await expect(page.getByRole("dialog", { name: "Command palette" })).toBeVisible();
      await page.keyboard.press("Escape");
      expect(await typed(page)).toBe("");
      await page.locator(".xterm").click();
    }

    await page.keyboard.press(`${key}+1`);
    await expect(page.getByRole("region", { name: "Diff" })).toBeVisible();
    await expect(page.getByRole("region", { name: "Terminal" })).toBeHidden();

    await page.keyboard.press(`${key}+Shift+t`);
    await expect(page.getByRole("region", { name: "Terminal" })).toBeVisible();
    await expectCall(page, "open_in_herdr", { repoId: "api" });
    await page.keyboard.type("x");
    await expect.poll(async () => (await typed(page)).endsWith("x")).toBe(true);
  });

  test("follows the pane size", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);
    const before = await page.evaluate(() => window.__fake.terminalSize());
    await page.setViewportSize({ width: 1500, height: 900 });
    await expect
      .poll(() => page.evaluate(() => window.__fake.terminalSize()?.cols))
      .toBeGreaterThan(before?.cols ?? 0);
    await expectCall(page, "terminal_resize", {
      cols: await page.evaluate(() => window.__fake.terminalSize()?.cols),
      rows: await page.evaluate(() => window.__fake.terminalSize()?.rows),
    });
  });

  test("offers to reattach after herdr closed", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);
    await page.evaluate(() => {
      window.__fake.terminalExit(0);
    });
    const region = page.getByRole("region", { name: "Terminal" });
    await expect(region.getByText("herdr closed")).toBeVisible();
    await region.getByRole("button", { name: "Reattach" }).click();
    await expect.poll(async () => (await calls(page, "terminal_open")).length).toBe(2);
    await expect(region.getByText("herdr closed")).toBeHidden();
  });

  test("shows an agent in the terminal from the agents list", async ({ page }) => {
    await openApp(page, "agents");
    await page.getByRole("region", { name: "Agents" }).locator('[data-pane-id="w2:p1"]').click();
    await expectCall(page, "focus_agent", { paneId: "w2:p1" });
    await expect(page.getByRole("region", { name: "Terminal" })).toBeVisible();
    await expect(sidebar(page).locator('[data-repo-id="web"]')).toHaveAttribute(
      "aria-current",
      "true",
    );
  });

  test("shows the workspace of the selected repository", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);
    await sidebar(page).locator('[data-repo-id="web"]').click();
    await expectCall(page, "focus_repo_in_herdr", { repoId: "web" });
    await expect(page.getByRole("region", { name: "Terminal" }).getByRole("status")).toHaveCount(0);
    expect(await calls(page, "open_in_herdr")).toEqual([]);
  });

  test("offers to create a workspace for a repository without one", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);
    await sidebar(page).locator('[data-repo-id="blog"]').click();
    const banner = page.getByRole("region", { name: "Terminal" }).getByRole("status");
    await expect(banner).toHaveText(/blog has no herdr workspace yet/);
    expect(await calls(page, "open_in_herdr")).toEqual([]);

    await banner.getByRole("button", { name: "Create workspace" }).click();
    await expectCall(page, "open_in_herdr", { repoId: "blog" });
    await sidebar(page).locator('[data-repo-id="web"]').click();
    await sidebar(page).locator('[data-repo-id="blog"]').click();
    await expect(banner).toHaveCount(0);
  });

  test("selects the repository herdr switches to", async ({ page }) => {
    await openApp(page, "agents");
    const infra = sidebar(page).locator('[data-repo-id="infra"]');
    const web = sidebar(page).locator('[data-repo-id="web"]');

    // With the diff on screen, navigating in herdr does not move the selection.
    await page.evaluate(() => window.__fake.herdrFocus("web"));
    await expect(sidebar(page).locator('[data-repo-id="api"]')).toHaveAttribute(
      "aria-current",
      "true",
    );

    await showTerminal(page);
    await page.evaluate(() => window.__fake.herdrFocus("infra"));
    await expect(infra).toHaveAttribute("aria-current", "true");
    await expectCall(page, "set_active_repo", { repoId: "infra" });
    await expect(web).not.toHaveAttribute("aria-current", "true");
  });
});
