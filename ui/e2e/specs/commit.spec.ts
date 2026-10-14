import type { Page } from "@playwright/test";

import { changesList, expect, expectCall, mod, openApp, sidebar, test } from "./helpers";

const form = (page: Page) => page.getByRole("form", { name: "Commit" });
const summary = (page: Page) => page.getByRole("textbox", { name: "Commit summary" });
const description = (page: Page) => page.getByRole("textbox", { name: "Commit description" });
const stagedRows = (page: Page) => changesList(page).locator('[data-key^="staged:"]');
const branchRow = (page: Page, name: string) =>
  page.getByRole("listbox", { name: "Branches" }).getByRole("option", { name });

test.describe("committing", () => {
  test.beforeEach(async ({ page }) => {
    await openApp(page);
  });

  test("commits the staged files with the keyboard", async ({ page }) => {
    await expect(stagedRows(page)).toHaveCount(2);
    const button = form(page).getByRole("button", { name: "Commit 2 files" });
    await expect(button).toBeDisabled();

    await summary(page).fill("Add rate limit headers");
    // Enter moves on to the description instead of committing.
    await summary(page).press("Enter");
    await expect(description(page)).toBeFocused();
    await description(page).fill("Limits are per user.");
    await expect(button).toBeEnabled();
    await description(page).press(`${await mod(page)}+Enter`);

    await expectCall(page, "commit", {
      repoId: "api",
      message: "Add rate limit headers\n\nLimits are per user.",
      amend: false,
    });
    await expect(stagedRows(page)).toHaveCount(0);
    await expect(
      page.getByRole("status").filter({ hasText: /^Committed [0-9a-f]{7}/ }),
    ).toBeVisible();
    await expect(form(page).getByText("Add rate limit headers")).toBeVisible();
    await expect(summary(page)).toHaveValue("");
    await expect(description(page)).toHaveValue("");
    // The branch list follows the new commit.
    await expect(branchRow(page, "feature/login-rate-limit")).toContainText("↑4");
  });

  test("keeps an unfinished message per repository", async ({ page }) => {
    await summary(page).fill("Work in progress");
    await sidebar(page).locator('[data-repo-id="web"]').click();
    await expect(summary(page)).toHaveValue("");
    await sidebar(page).locator('[data-repo-id="api"]').click();
    await expect(summary(page)).toHaveValue("Work in progress");

    await page.reload();
    await expect(summary(page)).toHaveValue("Work in progress");
  });

  test("shows the hook output when the commit fails", async ({ page }) => {
    await page.evaluate(() => {
      window.__fake.failNext("commit", "lint: 2 problems\nsrc/login.ts:4 unused import");
    });
    await summary(page).fill("Add rate limit headers");
    await form(page).getByRole("button", { name: "Commit 2 files" }).click();

    const alert = form(page).getByRole("alert");
    await expect(alert).toContainText("Commit failed.");
    await expect(alert).toContainText("src/login.ts:4 unused import");
    await expect(stagedRows(page)).toHaveCount(2);
    await expect(summary(page)).toHaveValue("Add rate limit headers");

    await summary(page).fill("Add rate limit headers, take two");
    await expect(alert).toBeHidden();
  });

  test("undoes the last commit and brings its message back", async ({ page }) => {
    await summary(page).fill("Add rate limit headers");
    await form(page).getByRole("button", { name: "Commit 2 files" }).click();
    await expect(stagedRows(page)).toHaveCount(0);

    await form(page).getByRole("button", { name: "Undo" }).click();
    await expectCall(page, "undo_last_commit", { repoId: "api" });
    await expect(stagedRows(page)).toHaveCount(2);
    await expect(summary(page)).toHaveValue("Add rate limit headers");
    await expect(form(page).getByText("update login.txt")).toBeVisible();
  });

  test("amends the last commit", async ({ page }) => {
    await summary(page).fill("My draft");
    await form(page).getByRole("checkbox", { name: "Amend last commit" }).check();
    await expect(summary(page)).toHaveValue("update login.txt");
    // The newest commit is not on the remote yet.
    await expect(form(page).getByText(/already on a remote/)).toBeHidden();

    await summary(page).fill("Update the login limit");
    await form(page).getByRole("button", { name: "Amend commit" }).click();
    await expectCall(page, "commit", {
      repoId: "api",
      message: "Update the login limit",
      amend: true,
    });
    await expect(form(page).getByRole("checkbox", { name: "Amend last commit" })).not.toBeChecked();
    // The draft set aside while amending was replaced by the commit.
    await expect(summary(page)).toHaveValue("");
    await expect(form(page).getByText("Update the login limit")).toBeVisible();
  });

  test("warns before amending a pushed commit and offers no undo", async ({ page }) => {
    await sidebar(page).locator('[data-repo-id="web"]').click();
    await expect(form(page).getByText("Initial commit")).toBeVisible();
    await expect(form(page).getByRole("button", { name: "Undo" })).toHaveCount(0);
    await form(page).getByRole("checkbox", { name: "Amend last commit" }).check();
    await expect(form(page).getByText(/already on a remote/)).toBeVisible();
    await expect(form(page).getByRole("button", { name: "Amend commit" })).toBeEnabled();
  });

  test("cannot amend before the first commit", async ({ page }) => {
    await sidebar(page).locator('[data-repo-id="blog"]').click();
    await expect(form(page).getByText("No commits yet")).toBeVisible();
    await expect(form(page).getByRole("checkbox", { name: "Amend last commit" })).toBeDisabled();
  });

  test("focuses the message from the keyboard", async ({ page }) => {
    await changesList(page).focus();
    await page.keyboard.press(`${await mod(page)}+3`);
    await expect(summary(page)).toBeFocused();
  });
});
