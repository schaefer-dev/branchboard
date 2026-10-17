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
    const switcher = page.getByRole("group", { name: "Right pane" });
    await expect(switcher.getByRole("button", { name: "History", exact: true })).toBeVisible();
    await expect(switcher.getByRole("button", { name: "Terminal", exact: true })).toHaveCount(0);
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

  test("reports modified keys in win32-input-mode when ConPTY asks for it", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);

    // ConPTY drops herdr's kitty request and sends this instead (Windows).
    await output(page, "\x1b[?9001h");
    await page.keyboard.press("Shift+Enter");
    // Vk;Sc;Uc;Kd;Cs;Rc – Shift+Enter down, then up, with the shift state.
    await expect.poll(() => typed(page)).toContain("\x1b[13;28;13;1;16;1_");
    await expect.poll(() => typed(page)).toContain("\x1b[13;28;13;0;16;1_");
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

  for (const kittyFlags of [0, 1, 3]) {
    test(`leaves native paste shortcuts uncancelled with Kitty flags ${String(kittyFlags)}`, async ({
      page,
    }) => {
      await openApp(page, "agents");
      await showTerminal(page);
      const results = await page.evaluate(async (flags) => {
        const term = window.__terminal;
        const input = term?.textarea;
        if (!term || !input) throw new Error("Terminal input is unavailable");
        await new Promise<void>((resolve) => {
          term.write(`\x1b[=${String(flags)}u`, resolve);
        });
        const mac = navigator.userAgent.includes("Mac");
        const shortcuts: KeyboardEventInit[] = mac
          ? [{ key: "v", code: "KeyV", keyCode: 86, metaKey: true }]
          : [
              { key: "v", code: "KeyV", keyCode: 86, ctrlKey: true },
              { key: "V", code: "KeyV", keyCode: 86, ctrlKey: true, shiftKey: true },
              { key: "Insert", code: "Insert", keyCode: 45, shiftKey: true },
              { key: "v", code: "KeyV", keyCode: 86, metaKey: true },
            ];
        input.focus();
        return shortcuts.map((shortcut) => {
          let emitted = "";
          const listener = term.onData((data) => (emitted += data));
          // Synthetic keys let us test Windows shortcuts on a macOS host
          // without invoking the host clipboard or its history UI.
          const allowed = ["keydown", "keypress", "keyup"].map((type) =>
            input.dispatchEvent(
              new KeyboardEvent(type, {
                bubbles: true,
                cancelable: true,
                charCode: type === "keypress" ? (shortcut.key?.charCodeAt(0) ?? 0) : 0,
                ...shortcut,
              }),
            ),
          );
          listener.dispose();
          return { shortcut, allowed, emitted };
        });
      }, kittyFlags);
      for (const result of results) {
        expect(result.allowed, JSON.stringify(result.shortcut)).toEqual([true, true, true]);
        expect(result.emitted, JSON.stringify(result.shortcut)).toBe("");
      }
      expect(await typed(page)).toBe("");
    });

    test(`pastes Unicode and multiline text once with Kitty flags ${String(kittyFlags)}`, async ({
      page,
    }) => {
      await openApp(page, "agents");
      await showTerminal(page);
      let expected = "";
      for (const bracketed of [false, true]) {
        await page.evaluate(
          async ({ flags, bracketed }) => {
            const term = window.__terminal;
            const input = term?.textarea;
            if (!term || !input) throw new Error("Terminal input is unavailable");
            await new Promise<void>((resolve) => {
              term.write(`\x1b[=${String(flags)}u\x1b[?2004${bracketed ? "h" : "l"}`, resolve);
            });
            const data = new DataTransfer();
            data.setData("text/plain", "Grüße\r\n你好\nlast");
            input.dispatchEvent(
              new ClipboardEvent("paste", { clipboardData: data, bubbles: true }),
            );
          },
          { flags: kittyFlags, bracketed },
        );
        const text = "Grüße\r你好\rlast";
        expected += bracketed ? `\x1b[200~${text}\x1b[201~` : text;
        await expect.poll(() => typed(page)).toBe(expected);
      }
    });
  }

  test("keeps Ctrl+C and macOS Ctrl+V as terminal input", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);
    const mac = (await mod(page)) === "Meta";
    await page.keyboard.press("Control+c");
    if (mac) await page.keyboard.press("Control+v");
    await expect.poll(() => typed(page)).toBe(mac ? "\x03\x16" : "\x03");

    await output(page, "\x1b[>1u");
    await page.keyboard.press("Control+c");
    if (mac) await page.keyboard.press("Control+v");
    await expect
      .poll(() => typed(page))
      .toBe(mac ? "\x03\x16\x1b[99;5u\x1b[118;5u" : "\x03\x1b[99;5u");
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

  test("uses Menlo at the remembered size and resizes it on request", async ({ page }) => {
    await openApp(page, "agents");
    await showTerminal(page);
    const font = () =>
      page.evaluate(() => ({
        family: window.__terminal?.options.fontFamily ?? "",
        size: window.__terminal?.options.fontSize ?? 0,
      }));
    expect(await font()).toMatchObject({ size: 17 });
    expect((await font()).family).toMatch(/^Menlo, /);
    // Larger cells fit fewer of them: rows, and columns where widths round.
    const cells = async () => {
      const size = await page.evaluate(() => window.__fake.terminalSize());
      return (size?.cols ?? 0) * (size?.rows ?? 0);
    };
    const before = await cells();

    const key = await mod(page);
    await page.locator(".xterm").click();
    // US layouts: Cmd/Ctrl+= ; German layouts have a "+" key of their own.
    await page.keyboard.press(`${key}+Equal`);
    await expectCall(page, "set_terminal_font_size", { size: 18 });
    await expect.poll(async () => (await font()).size).toBe(18);
    // herdr is told the new size.
    await expect.poll(cells).toBeLessThan(before);

    await page.keyboard.press(`${key}+0`);
    await expectCall(page, "set_terminal_font_size", { size: 17 });
    await expect.poll(async () => (await font()).size).toBe(17);
  });
});
