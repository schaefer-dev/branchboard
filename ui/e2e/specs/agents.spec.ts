import type { Page } from "@playwright/test";

import { clickMenu, expect, expectCall, menu, mod, openApp, sidebar, test } from "./helpers";

function badge(page: Page, repoId: string) {
  return sidebar(page).locator(`[data-repo-id="${repoId}"]`).getByRole("img");
}

test.describe("coding agents in herdr", () => {
  test("shows nothing without herdr", async ({ page }) => {
    await openApp(page);
    await expect(sidebar(page).getByRole("img")).toHaveCount(0);

    await sidebar(page).locator('[data-repo-id="web"]').click({ button: "right" });
    await expect.poll(() => menu(page)).not.toContain("Open in herdr");
  });

  test("shows the most urgent agent state per repository", async ({ page }) => {
    await openApp(page, "agents");

    const api = badge(page, "api");
    await expect(api).toHaveAttribute("data-state", "blocked");
    await expect(api).toHaveAccessibleName(
      "Codex needs input · Add rate limit headers; Claude is working · Refactor the login handler",
    );
    await expect(api).toHaveText("2");
    await expect(badge(page, "web")).toHaveAttribute("data-state", "done");
    await expect(badge(page, "web")).toHaveAttribute(
      "title",
      "Claude is done · Fix the login form",
    );
    await expect(badge(page, "infra")).toHaveAttribute("data-state", "working");
    await expect(badge(page, "dotfiles")).toHaveAttribute("data-state", "idle");
    await expect(badge(page, "blog")).toHaveCount(0);
  });

  test("follows herdr as agents change", async ({ page }) => {
    await openApp(page, "agents");
    await expect(badge(page, "web")).toHaveAttribute("data-state", "done");

    await page.evaluate(() =>
      window.__fake.setAgents({
        available: true,
        repos: [
          {
            repoId: "web",
            state: "blocked",
            agents: [{ paneId: "w2:p1", agent: "claude", state: "blocked", title: null }],
          },
        ],
      }),
    );
    await expect(badge(page, "web")).toHaveAttribute("data-state", "blocked");
    await expect(badge(page, "api")).toHaveCount(0);

    // herdr stopped: no badges, and nothing to open.
    await page.evaluate(() => window.__fake.setAgents({ available: false, repos: [] }));
    await expect(sidebar(page).getByRole("img")).toHaveCount(0);
    await sidebar(page).locator('[data-repo-id="web"]').click({ button: "right" });
    await expect.poll(() => menu(page)).not.toContain("Open in herdr");
  });

  test("opens a repository's workspace from the context menu", async ({ page }) => {
    await openApp(page, "agents");
    await sidebar(page).locator('[data-repo-id="web"]').click({ button: "right" });
    await expect.poll(() => menu(page)).toContain("Open in herdr");
    await clickMenu(page, "Open in herdr");
    await expectCall(page, "open_in_herdr", { repoId: "web" });
  });

  test("opens the active repository with a shortcut", async ({ page }) => {
    await openApp(page, "agents");
    await page.keyboard.press(`${await mod(page)}+Shift+t`);
    await expectCall(page, "open_in_herdr", { repoId: "api" });
  });
});
