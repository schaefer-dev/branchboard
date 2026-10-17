import type { Page } from "@playwright/test";

import { expect, expectCall, mod, openApp, sidebar, test } from "./helpers";

/** ⌘J on macOS; Ctrl+Shift+J elsewhere, since Ctrl+J belongs to the terminal. */
async function next(page: Page) {
  const key = await mod(page);
  await page.keyboard.press(key === "Meta" ? "Meta+j" : "Control+Shift+j");
}

function terminal(page: Page) {
  return page.getByRole("region", { name: "Terminal" });
}

test.describe("agents that need the user", () => {
  test("steps through them: the terminal for input, the work for a finished one", async ({
    page,
  }) => {
    await openApp(page, "agents");
    const agents = page.getByRole("region", { name: "Agents" });
    await expect(agents.getByRole("heading")).toHaveText("Agents 3 need you");

    // Codex waits for input in api.
    await next(page);
    await expectCall(page, "focus_agent", { paneId: "w1:p2" });
    await expect(terminal(page)).toBeVisible();

    // Claude finished in web, which has no changes: its branch is shown.
    await next(page);
    await expectCall(page, "focus_agent", { paneId: "w2:p1" });
    await expect(sidebar(page).locator('[data-repo-id="web"]')).toHaveAttribute(
      "aria-current",
      "true",
    );
    await expect(page.getByRole("region", { name: "Branch review" })).toBeVisible();
    await expect(terminal(page)).toBeHidden();

    // An agent outside every repository is shown in the terminal; then it starts over.
    await agents.getByRole("button", { name: "Next" }).click();
    await expectCall(page, "focus_agent", { paneId: "w6:p1" });
    await expect(terminal(page)).toBeVisible();
    await next(page);
    await expect
      .poll(async () =>
        (await page.evaluate(() => window.__fake.calls()))
          .filter((call) => call.cmd === "focus_agent")
          .map((call) => call.args.paneId),
      )
      .toEqual(["w1:p2", "w2:p1", "w6:p1", "w1:p2"]);
  });

  test("shows the changes of a finished agent's repository when it has any", async ({ page }) => {
    await openApp(page, "agents");
    await page.evaluate(async () => {
      await window.__fake.setAgents({
        available: true,
        repos: [
          {
            repoId: "api",
            state: "done",
            agents: [{ paneId: "w1:p1", agent: "claude", state: "done", title: null }],
          },
        ],
        agents: [
          {
            paneId: "w1:p1",
            agent: "claude",
            state: "done",
            title: null,
            workspace: "api",
            tab: null,
            repoId: "api",
          },
        ],
        focusedRepoId: null,
      });
    });
    await page.keyboard.press(`${await mod(page)}+2`);
    await next(page);
    await expectCall(page, "focus_agent", { paneId: "w1:p1" });
    await expect(page.getByRole("region", { name: "Diff" })).toBeVisible();
  });

  test("opens the agent of a clicked notification", async ({ page }) => {
    await openApp(page, "agents");
    await page.evaluate(() => window.__fake.openAlert("w2:p1"));
    await expectCall(page, "focus_agent", { paneId: "w2:p1" });
    await expect(sidebar(page).locator('[data-repo-id="web"]')).toHaveAttribute(
      "aria-current",
      "true",
    );
  });

  test("tells the backend whether the terminal is on screen", async ({ page }) => {
    await openApp(page, "agents");
    await page.keyboard.press(`${await mod(page)}+Shift+t`);
    await expectCall(page, "set_terminal_visible", { visible: true });
    await page.keyboard.press(`${await mod(page)}+1`);
    await expectCall(page, "set_terminal_visible", { visible: false });
  });

  test("mutes the sounds from the status bar", async ({ page }) => {
    await openApp(page);
    const sounds = page.getByRole("group", { name: "Agent sounds" });
    const mute = sounds.getByRole("button", { name: "Mute agent sounds" });
    const watching = sounds.getByRole("button", { name: "Mute the agent on screen" });
    await expect(mute).toHaveAttribute("aria-pressed", "false");
    await expect(watching).toHaveAttribute("aria-pressed", "true");

    await watching.click();
    await expectCall(page, "set_mute_when_watching", { mute: false });
    await expect(watching).toHaveAttribute("aria-pressed", "false");

    await mute.click();
    await expectCall(page, "set_sound_muted", { muted: true });
    await expect(mute).toHaveAttribute("aria-pressed", "true");
    await expect(watching).toBeDisabled();
  });
});
