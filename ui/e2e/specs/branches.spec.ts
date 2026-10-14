import type { Page } from "@playwright/test";

import { clickMenu, expect, expectCall, menu, mod, openApp, test, sidebar } from "./helpers";

function board(page: Page) {
  return page.getByRole("listbox", { name: "Branches" });
}

function branch(page: Page, name: string) {
  return board(page).locator(`[data-branch="${name}"]`);
}

async function openBranches(page: Page) {
  await openApp(page);
  await expect(board(page)).toBeVisible();
}

test.describe("branches", () => {
  test.beforeEach(async ({ page }) => {
    await openBranches(page);
  });

  test("lists branches by recent activity with ahead/behind counts", async ({ page }) => {
    await expect(board(page).locator(".name")).toHaveText([
      "feature/login-rate-limit",
      "spike/local-only",
      "development",
      "chore/old-cleanup",
      "fix/header-overflow",
    ]);
    await expect(branch(page, "feature/login-rate-limit")).toHaveAttribute("aria-current", "true");
    await expect(
      branch(page, "feature/login-rate-limit").getByLabel(
        "3 ahead, 0 behind than origin/development",
      ),
    ).toBeVisible();
    await expect(
      branch(page, "feature/login-rate-limit").getByLabel(
        "1 ahead, 0 behind than origin/feature/login-rate-limit",
      ),
    ).toBeVisible();
    await expect(
      branch(page, "fix/header-overflow").getByLabel("1 ahead, 2 behind than origin/development"),
    ).toBeVisible();
    await expect(branch(page, "chore/old-cleanup")).toContainText("gone");
    await expect(branch(page, "spike/local-only")).toContainText("local");
    await expect(page.getByText("Compared with", { exact: true })).toBeVisible();
    await expect(page.getByText("Fetched 3 min ago")).toBeVisible();
  });

  test("shows the commits a branch is ahead and behind", async ({ page }) => {
    await branch(page, "feature/login-rate-limit").click();
    const pane = page.getByRole("region", { name: "Branch commits" });
    await expect(pane.getByText("feature/login-rate-limit", { exact: true })).toBeVisible();
    await expect(
      pane.getByRole("region", { name: "Ahead of origin/development" }).getByRole("listitem"),
    ).toHaveCount(3);

    await branch(page, "fix/header-overflow").click();
    const ahead = pane.getByRole("region", { name: "Ahead of origin/development" });
    await expect(ahead.getByRole("listitem")).toHaveCount(1);
    await expect(ahead).toContainText("update header.css");
    await expect(
      pane.getByRole("region", { name: "Behind origin/development" }).getByRole("listitem"),
    ).toHaveCount(2);

    await branch(page, "development").click();
    await expect(pane.getByText("Up to date with origin/development")).toBeVisible();
    await expectCall(page, "get_branch_commits", { repoId: "api", branch: "development" });
  });

  test("sorts alphabetically and remembers the choice", async ({ page }) => {
    await page.getByRole("button", { name: "A–Z" }).click();
    await expect(board(page).locator(".name").first()).toHaveText("chore/old-cleanup");
    await page.reload();
    await expect(page.getByRole("button", { name: "A–Z" })).toHaveAttribute("aria-pressed", "true");
    await expect(board(page).locator(".name").first()).toHaveText("chore/old-cleanup");
  });

  test("switches branches with a double click and with Enter", async ({ page }) => {
    await branch(page, "development").dblclick();
    await expectCall(page, "switch_branch", { repoId: "api", branch: "development" });
    await expect(branch(page, "development")).toHaveAttribute("aria-current", "true");

    await board(page).focus();
    await page.keyboard.press("Home");
    await page.keyboard.press("Enter");
    await expectCall(page, "switch_branch", { repoId: "api", branch: "feature/login-rate-limit" });
    await expect(branch(page, "feature/login-rate-limit")).toHaveAttribute("aria-current", "true");
  });

  test("explains why a switch failed", async ({ page }) => {
    await page.evaluate(() => {
      window.__fake.failNext(
        "switch_branch",
        "error: Your local changes to the following files would be overwritten by checkout:\n\tconfig.toml",
      );
    });
    await branch(page, "development").dblclick();
    await expect(page.getByText("Could not switch to development.")).toBeVisible();
    await expect(page.getByText("would be overwritten by checkout")).toBeVisible();
    await expect(branch(page, "feature/login-rate-limit")).toHaveAttribute("aria-current", "true");
  });

  test("fetches and reports when it happened", async ({ page }) => {
    await page.getByRole("button", { name: "Fetch", exact: true }).click();
    await expect(page.getByText("Fetching…")).toBeVisible();
    await expect(page.getByText("Fetched just now")).toBeVisible();
    await expectCall(page, "fetch_repo", { repoId: "api" });
  });

  test("shows fetch failures", async ({ page }) => {
    await page.evaluate(() => {
      window.__fake.failNext("fetch_repo", "fatal: Could not read from remote repository.");
    });
    await page.keyboard.press(`${await mod(page)}+Shift+f`);
    await expect(page.getByText("Fetching failed.")).toBeVisible();
    await expect(page.getByText("Last fetch failed")).toBeVisible();
  });

  test("compares with origin/master where origin/development and origin/main do not exist", async ({
    page,
  }) => {
    await sidebar(page).locator('[data-repo-id="web"]').click();
    await expect(page.getByRole("button", { name: /Compared with/ })).toHaveText(
      /Compared with\s*origin\/master/,
    );
    await page.getByRole("button", { name: /Compared with/ }).click();
    const dialog = page.getByRole("dialog", { name: "Compare branches with" });
    await expect(dialog.getByRole("textbox")).toHaveValue("");
    await expect(
      dialog.getByText(
        /Leave empty for origin\/development, then origin\/main, then origin\/master/,
      ),
    ).toBeVisible();
  });

  test("compares with another branch", async ({ page }) => {
    await page.getByRole("button", { name: /Compared with/ }).click();
    const dialog = page.getByRole("dialog", { name: "Compare branches with" });
    await dialog.getByRole("textbox").fill("origin/main");
    await page.keyboard.press("Enter");
    await expectCall(page, "set_mainline", { repoId: "api", mainline: "origin/main" });
    await expect(
      page.getByText("doesn't exist. Fetch, or compare with another branch."),
    ).toBeVisible();
  });

  test("configures background fetching", async ({ page }) => {
    await page.getByRole("button", { name: "Branch options" }).click();
    await expect.poll(() => menu(page)).toContain("Fetch in the background/Every 10 minutes");
    await clickMenu(page, "Fetch in the background/Off");
    await expectCall(page, "set_fetch_interval", { minutes: 0 });
  });

  test("focuses the lists with the keyboard", async ({ page }) => {
    const key = await mod(page);
    await page.keyboard.press(`${key}+2`);
    await expect(board(page)).toBeFocused();
    await expect(page.getByRole("region", { name: "Branch commits" })).toBeVisible();
    await page.keyboard.press(`${key}+1`);
    await expect(page.getByRole("listbox", { name: "Changed files" })).toBeFocused();
    await expect(page.getByRole("region", { name: "Diff" })).toBeVisible();
  });
});
