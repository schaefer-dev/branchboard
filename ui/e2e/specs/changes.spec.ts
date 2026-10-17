import type { Page } from "@playwright/test";

import {
  changesList,
  clickMenu,
  diffPane,
  expect,
  expectCall,
  menu,
  mod,
  openApp,
  test,
} from "./helpers";

const row = (page: Page, key: string) => changesList(page).locator(`[data-key="${key}"]`);

async function select(page: Page, key: string) {
  await row(page, key).click();
  await expect(row(page, key)).toHaveAttribute("aria-selected", "true");
}

test.describe("changes and diffs", () => {
  test.beforeEach(async ({ page }) => {
    await openApp(page);
  });

  test("lists staged and unstaged changes and shows the first diff", async ({ page }) => {
    const names = changesList(page).locator('[role="option"] .name');
    await expect(names).toHaveText([
      "config.toml",
      "legacy-notes.txt",
      "README.md",
      "config.toml",
      "login.ts",
      "logo.png",
      "big.csv",
      "notes.md",
    ]);
    await expect(row(page, "staged:config.toml")).toHaveAttribute("aria-selected", "true");
    const diff = diffPane(page);
    await expect(diff.getByText("Staged", { exact: true })).toBeVisible();
    await expect(diff.getByText("port = 9090")).toBeVisible();
    await expect(diff.locator(".stats")).toHaveText("+1 −1");
  });

  test("shows hunks with line numbers", async ({ page }) => {
    await select(page, "unstaged:src/handlers/login.ts");
    const diff = diffPane(page);
    await expect(diff.locator(".hunk")).toHaveCount(2);
    const added = diff
      .locator(".line.added")
      .filter({ hasText: 'log.debug("login attempt", { user });' });
    await expect(added.locator(".num").nth(1)).toHaveText("6");
    // Syntax highlighting recorded from the real backend is rendered.
    await expect(diff.locator(".tok-function", { hasText: "loadUsers" }).first()).toBeVisible();
    await expect(
      diff.locator(".tok-comment", { hasText: "/** Handles login requests. */" }),
    ).toBeVisible();
    await expect(diff.getByText("src/handlers/")).toBeVisible();
  });

  test("describes renames, binary files and large diffs", async ({ page }) => {
    await select(page, "staged:legacy-notes.txt");
    await expect(diffPane(page).getByText("rename from legacy.txt")).toBeVisible();
    await expect(diffPane(page).getByText("No content changes")).toBeVisible();

    await select(page, "unstaged:assets/logo.png");
    await expect(diffPane(page).getByText("Binary file")).toBeVisible();

    await select(page, "unstaged:data/big.csv");
    await expect(diffPane(page).getByText("This diff is large")).toBeVisible();
    await diffPane(page).getByRole("button", { name: "Show diff" }).click();
    await expectCall(page, "get_diff", {
      repoId: "api",
      request: {
        path: "data/big.csv",
        origPath: null,
        area: "unstaged",
        kind: "untracked",
        full: true,
      },
    });
    await expect(diffPane(page).getByText("row 1", { exact: true })).toBeVisible();
  });

  test("stages with Space and moves on to the next file", async ({ page }) => {
    await select(page, "unstaged:src/handlers/login.ts");
    await page.keyboard.press("Space");
    await expectCall(page, "stage_paths", { repoId: "api", paths: ["src/handlers/login.ts"] });
    await expect(row(page, "staged:src/handlers/login.ts")).toBeVisible();
    await expect(row(page, "unstaged:assets/logo.png")).toHaveAttribute("aria-selected", "true");
  });

  test("stages a range selected with Shift", async ({ page }) => {
    await select(page, "unstaged:assets/logo.png");
    await page.keyboard.press("Shift+ArrowDown");
    await page.keyboard.press("Shift+ArrowDown");
    await page.keyboard.press("Enter");
    await expectCall(page, "stage_paths", {
      repoId: "api",
      paths: ["assets/logo.png", "data/big.csv", "notes.md"],
    });
    await expect(row(page, "staged:notes.md")).toBeVisible();
  });

  test("unstages from the context menu", async ({ page }) => {
    await row(page, "staged:config.toml").click({ button: "right" });
    await expect.poll(() => menu(page)).toEqual(["Open in VS Code", "Comment on file", "Unstage"]);
    await clickMenu(page, "Unstage");
    await expectCall(page, "unstage_paths", { repoId: "api", paths: ["config.toml"] });
  });

  test("discards after confirmation, and not without", async ({ page }) => {
    await select(page, "unstaged:notes.md");
    await page.evaluate(() => {
      window.__fake.setConfirm(false);
    });
    await page.keyboard.press("Delete");
    await expectCall(
      page,
      "plugin:dialog|message",
      expect.objectContaining({ title: "Discard changes to notes.md?" }),
    );
    await expect(row(page, "unstaged:notes.md")).toBeVisible();

    await page.evaluate(() => {
      window.__fake.setConfirm(true);
    });
    await page.keyboard.press("Delete");
    await expectCall(page, "discard_paths", { repoId: "api", paths: ["notes.md"] });
    await expect(row(page, "unstaged:notes.md")).toHaveCount(0);
  });

  test("stages everything with Stage all", async ({ page }) => {
    await changesList(page).getByRole("button", { name: "Stage all", exact: true }).click();
    await expect(
      changesList(page).getByRole("button", { name: "Stage all", exact: true }),
    ).toHaveCount(0);
    await expect(changesList(page).locator('[data-key^="unstaged:"]')).toHaveCount(0);
  });

  test("opens the selected file in VS Code at the first change", async ({ page }) => {
    await select(page, "unstaged:src/handlers/login.ts");
    await expect(diffPane(page).locator(".hunk")).toHaveCount(2);
    await page.keyboard.press(`${await mod(page)}+Shift+e`);
    await expectCall(page, "open_in_editor", {
      repoId: "api",
      path: "src/handlers/login.ts",
      line: 5,
    });

    await select(page, "unstaged:README.md");
    await diffPane(page).getByRole("button", { name: "Open in VS Code" }).click();
    await expectCall(page, "open_in_editor", { repoId: "api", path: null, line: null });
  });

  test("follows changes made outside the app", async ({ page }) => {
    await page.evaluate(() => window.__fake.replaceStatus("api", ["src/new-feature.rs"]));
    await expect(row(page, "unstaged:src/new-feature.rs")).toBeVisible();
    await expect(row(page, "unstaged:notes.md")).toHaveCount(0);
  });
});
