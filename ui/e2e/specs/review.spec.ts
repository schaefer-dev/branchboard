import type { Page } from "@playwright/test";

import { expect, expectCall, openApp, sidebar, test } from "./helpers";

const pane = (page: Page) => page.getByRole("region", { name: "Branch review" });
const commits = (page: Page) => pane(page).getByRole("listbox", { name: "Commits" });
const files = (page: Page) => pane(page).getByRole("listbox", { name: "Files in this review" });
const file = (page: Page) => pane(page).getByRole("region", { name: "Selected file" });
const branch = (page: Page, name: string) =>
  page.getByRole("listbox", { name: "Branches" }).locator(`[data-branch="${name}"]`);

test.describe("branch review", () => {
  test.beforeEach(async ({ page }) => {
    await openApp(page);
  });

  test("shows everything a branch adds against the mainline", async ({ page }) => {
    await branch(page, "feature/login-rate-limit").click();
    const whole = commits(page).getByRole("option", { name: /All changes/ });
    await expect(whole).toHaveAttribute("aria-selected", "true");
    await expect(whole).toContainText("3 commits since origin/development");
    await expectCall(page, "get_review_files", {
      repoId: "api",
      target: { kind: "branch", name: "feature/login-rate-limit" },
    });

    await expect(files(page).getByRole("option")).toHaveCount(2);
    await expect(files(page).getByRole("option").first()).toHaveAttribute("aria-selected", "true");
    await expect(file(page)).toContainText("since origin/development");

    await files(page)
      .getByRole("option", { name: /rate-limit\.ts/ })
      .click();
    await expect(file(page).locator(".line.added")).toHaveCount(10);
    await expect(file(page).getByText("export const LIMIT = 60;")).toBeVisible();
    await expect(file(page).locator(".stats")).toHaveText("+10 −0");
  });

  test("reviews single commits, ahead and behind", async ({ page }) => {
    await branch(page, "feature/login-rate-limit").click();
    const ahead = commits(page).getByRole("group", { name: "Ahead of origin/development" });
    await ahead.getByRole("option").nth(1).click();
    await expect(files(page).getByRole("option")).toHaveCount(2);
    await files(page)
      .getByRole("option", { name: /rate-limit\.ts/ })
      .click();
    await expect(file(page).getByText("export const LIMIT = 10;")).toBeVisible();
    await expect(file(page).getByText("export const LIMIT = 60;")).toBeVisible();
    await expect(file(page).locator(".scope")).toHaveText(/in [0-9a-f]{7}/);

    await branch(page, "fix/header-overflow").click();
    const behind = commits(page).getByRole("group", { name: "Behind origin/development" });
    await behind.getByRole("option").first().click();
    await expect(files(page).getByRole("option")).toHaveText([/app\.txt/]);
    await expect(file(page).locator(".line.added")).toHaveText([/v3/]);
  });

  test("moves through commits and files with the keyboard", async ({ page }) => {
    await branch(page, "feature/login-rate-limit").click();
    await commits(page).focus();
    await page.keyboard.press("ArrowDown");
    const newest = commits(page)
      .getByRole("group", { name: "Ahead of origin/development" })
      .getByRole("option")
      .first();
    await expect(newest).toHaveAttribute("aria-selected", "true");
    await expect(files(page).getByRole("option")).toHaveText([/login\.txt/]);

    await page.keyboard.press("ArrowUp");
    await expect(files(page).getByRole("option")).toHaveCount(2);
    await files(page).focus();
    await page.keyboard.press("ArrowDown");
    await expect(files(page).getByRole("option").nth(1)).toHaveAttribute("aria-selected", "true");
    await expect(file(page)).toContainText("rate-limit.ts");
  });

  test("keeps the selection when the branch list reloads", async ({ page }) => {
    await branch(page, "feature/login-rate-limit").click();
    const second = commits(page)
      .getByRole("group", { name: "Ahead of origin/development" })
      .getByRole("option")
      .nth(1);
    await second.click();
    await expect(second).toHaveAttribute("aria-selected", "true");
    await page.getByRole("button", { name: "Fetch" }).click();
    await expect(page.getByText(/^Fetched/)).toBeVisible();
    await expect(second).toHaveAttribute("aria-selected", "true");
  });

  test("lists the latest commits when the mainline is missing", async ({ page }) => {
    await sidebar(page).locator('[data-repo-id="web"]').click();
    await branch(page, "development").click();
    await expect(commits(page).getByRole("option", { name: /All changes/ })).toHaveCount(0);
    await expect(
      commits(page)
        .getByRole("group", { name: "Latest commits" })
        .getByText("This branch has no commits."),
    ).toBeVisible();
  });
});
