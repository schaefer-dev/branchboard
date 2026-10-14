import type { Locator, Page } from "@playwright/test";

import { changesList, diffPane, expect, openApp, sidebar, test } from "./helpers";

const ROW = 28;

function branchList(page: Page) {
  return page.getByRole("listbox", { name: "Branches" });
}

function splitter(page: Page) {
  return page.getByRole("separator", { name: "Resize branches" });
}

async function box(locator: Locator) {
  const rect = await locator.boundingBox();
  if (!rect) throw new Error("element is not visible");
  return rect;
}

function listHeight(page: Page): Promise<number> {
  return branchList(page).evaluate((element) => element.clientHeight);
}

test.describe("work pane", () => {
  test.beforeEach(async ({ page }) => {
    await openApp(page);
  });

  test("shows branches above the changes", async ({ page }) => {
    await expect(branchList(page)).toBeVisible();
    await expect(changesList(page)).toBeVisible();
    const branches = await box(branchList(page));
    const changes = await box(changesList(page));
    expect(branches.y + branches.height).toBeLessThanOrEqual(changes.y);
    // Five branches fit without scrolling.
    await expect.poll(() => listHeight(page)).toBe(5 * ROW);
  });

  test("caps the branch list at eight rows and scrolls the rest", async ({ page }) => {
    await sidebar(page).locator('[data-repo-id="infra"]').click();
    await expect(branchList(page).locator(".name").first()).toHaveText("feature/topic-01");
    await expect.poll(() => listHeight(page)).toBe(8 * ROW);
    expect(await branchList(page).evaluate((element) => element.scrollHeight)).toBe(12 * ROW);
  });

  test("the right pane follows the list used last", async ({ page }) => {
    const commits = page.getByRole("region", { name: "Branch review" });
    await changesList(page).locator('[data-key="unstaged:src/handlers/login.ts"]').click();
    await expect(diffPane(page)).toBeVisible();

    await branchList(page).locator('[data-branch="development"]').click();
    await expect(commits).toBeVisible();
    await expect(diffPane(page)).toBeHidden();

    await changesList(page).locator('[data-key="unstaged:src/handlers/login.ts"]').click();
    await expect(diffPane(page)).toBeVisible();
    await expect(commits).toBeHidden();
  });

  test("resizes the branch list for the session", async ({ page }) => {
    await expect.poll(() => listHeight(page)).toBe(5 * ROW);

    const handle = await box(splitter(page));
    const x = handle.x + handle.width / 2;
    const y = handle.y + handle.height / 2;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y + 2 * ROW, { steps: 4 });
    await page.mouse.up();
    await expect.poll(() => listHeight(page)).toBe(7 * ROW);
    await expect(changesList(page)).toBeVisible();

    await splitter(page).focus();
    await page.keyboard.press("ArrowUp");
    await expect.poll(() => listHeight(page)).toBe(6 * ROW);

    await splitter(page).dblclick();
    await expect.poll(() => listHeight(page)).toBe(5 * ROW);

    await splitter(page).focus();
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => listHeight(page)).toBe(6 * ROW);
    await page.reload();
    await expect.poll(() => listHeight(page)).toBe(5 * ROW);
  });

  test("keeps room for the changes when the branch list grows", async ({ page }) => {
    await splitter(page).focus();
    await page.keyboard.press("End");
    await expect.poll(() => listHeight(page)).toBeGreaterThan(8 * ROW);
    const changes = await box(page.getByRole("group", { name: "Changes" }));
    expect(changes.height).toBeGreaterThanOrEqual(120);
  });
});
