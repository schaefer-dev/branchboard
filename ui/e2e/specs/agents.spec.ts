import type { Page } from "@playwright/test";

import { clickMenu, expect, expectCall, menu, mod, openApp, sidebar, test } from "./helpers";

function agentList(page: Page) {
  return page.getByRole("region", { name: "Agents" });
}

function badge(page: Page, repoId: string) {
  return sidebar(page).locator(`[data-repo-id="${repoId}"]`).getByRole("img");
}

test.describe("coding agents in herdr", () => {
  test("shows nothing without herdr", async ({ page }) => {
    await openApp(page);
    await expect(sidebar(page).getByRole("img")).toHaveCount(0);
    await expect(agentList(page)).toHaveCount(0);

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
        agents: [
          {
            paneId: "w2:p1",
            agent: "claude",
            state: "blocked",
            title: null,
            workspace: "web",
            tab: null,
            repoId: "web",
          },
        ],
        focusedRepoId: null,
      }),
    );
    await expect(badge(page, "web")).toHaveAttribute("data-state", "blocked");
    await expect(badge(page, "api")).toHaveCount(0);
    await expect(agentList(page).getByRole("button")).toHaveCount(1);
    await expect(agentList(page).getByRole("heading")).toHaveText("Agents 1 needs input");

    // herdr stopped: no badges, and nothing to open.
    await page.evaluate(() =>
      window.__fake.setAgents({ available: false, repos: [], agents: [], focusedRepoId: null }),
    );
    await expect(sidebar(page).getByRole("img")).toHaveCount(0);
    await expect(agentList(page)).toHaveCount(0);
    await sidebar(page).locator('[data-repo-id="web"]').click({ button: "right" });
    await expect.poll(() => menu(page)).not.toContain("Open in herdr");
  });

  test("shows a repository's workspace in the terminal from the context menu", async ({ page }) => {
    await openApp(page, "agents");
    await sidebar(page).locator('[data-repo-id="web"]').click({ button: "right" });
    await expect.poll(() => menu(page)).toContain("Show in terminal");
    await clickMenu(page, "Show in terminal");
    await expectCall(page, "open_in_herdr", { repoId: "web" });
    await expect(page.getByRole("region", { name: "Terminal" })).toBeVisible();
    await expect(sidebar(page).locator('[data-repo-id="web"]')).toHaveAttribute(
      "aria-current",
      "true",
    );
  });

  test("opens the workspace in herdr's own terminal when herdr is not installed here", async ({
    page,
  }) => {
    // herdr runs (e.g. started from another terminal) but Branchboard cannot
    // find its binary, so there is no embedded terminal.
    await openApp(page);
    await page.evaluate(() =>
      window.__fake.setAgents({ available: true, repos: [], agents: [], focusedRepoId: null }),
    );
    await sidebar(page).locator('[data-repo-id="web"]').click({ button: "right" });
    await expect.poll(() => menu(page)).toContain("Open in herdr");
    await clickMenu(page, "Open in herdr");
    await expectCall(page, "open_in_herdr", { repoId: "web" });
    await expect(page.getByRole("region", { name: "Terminal" })).toHaveCount(0);
  });

  test("opens the active repository with a shortcut", async ({ page }) => {
    await openApp(page, "agents");
    await page.keyboard.press(`${await mod(page)}+Shift+t`);
    await expectCall(page, "open_in_herdr", { repoId: "api" });
  });

  test("lists every agent in every space, most urgent first", async ({ page }) => {
    await openApp(page, "agents");
    const list = agentList(page);
    await expect(list.getByRole("heading")).toHaveText("Agents 1 needs input");
    await expect(list.locator(".place")).toHaveText([
      "api · agents",
      "web",
      "notes",
      "api · agents",
      "infra",
      "dotfiles",
    ]);
    await expect(list.locator(".what")).toHaveText([
      "Codex · Add rate limit headers",
      "Claude · Fix the login form",
      "Claude · Weekly review",
      "Claude · Refactor the login handler",
      "Claude",
      "Claude",
    ]);
    await expect(list.locator('[data-pane-id="w6:p1"]')).toHaveAccessibleName(
      "Claude is done · Weekly review, in notes",
    );
  });

  test("jumps to an agent and its repository", async ({ page }) => {
    await openApp(page, "agents");
    const list = agentList(page);

    await list.locator('[data-pane-id="w2:p1"]').click();
    await expectCall(page, "focus_agent", { paneId: "w2:p1" });
    await expectCall(page, "set_active_repo", { repoId: "web" });
    await expect(sidebar(page).locator('[data-repo-id="web"]')).toHaveAttribute(
      "aria-current",
      "true",
    );

    // An agent outside every repository only switches herdr.
    await list.locator('[data-pane-id="w6:p1"]').click();
    await expectCall(page, "focus_agent", { paneId: "w6:p1" });
    await expect(sidebar(page).locator('[data-repo-id="web"]')).toHaveAttribute(
      "aria-current",
      "true",
    );
  });

  test("navigates the agents with the keyboard", async ({ page }) => {
    await openApp(page, "agents");
    const list = agentList(page);
    await list.locator('[data-pane-id="w1:p2"]').focus();
    await page.keyboard.press("ArrowDown");
    await expect(list.locator('[data-pane-id="w2:p1"]')).toBeFocused();
    await page.keyboard.press("End");
    await expect(list.locator('[data-pane-id="w4:p1"]')).toBeFocused();
    await page.keyboard.press("Enter");
    await expectCall(page, "focus_agent", { paneId: "w4:p1" });
  });
});
