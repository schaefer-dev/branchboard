import { clickMenu, expect, expectCall, menu, mod, openApp, sidebar, test } from "./helpers";

test.describe("repository sidebar", () => {
  test.beforeEach(async ({ page }) => {
    await openApp(page);
  });

  test("groups repositories with favorites first and shows their state", async ({ page }) => {
    const nav = sidebar(page);
    const headings = nav.locator(".header .title");
    await expect(headings).toHaveText(["Favorites", "Side projects", "Work", "Ungrouped"]);

    const api = nav.locator('[data-repo-id="api"]');
    await expect(api).toHaveAttribute("aria-current", "true");
    await expect(api).toContainText("feature/login-rate-limit");
    await expect(api.getByTitle("Uncommitted changes")).toBeVisible();

    await expect(nav.locator('[data-repo-id="infra"]')).toContainText("Detached at b5b537a");
    await expect(nav.locator('[data-repo-id="blog"]')).toContainText("main (no commits)");
    await expect(nav.locator('[data-repo-id="old-service"]')).toContainText("Folder not found");
  });

  test("navigates with the keyboard and activates with a click", async ({ page }) => {
    const nav = sidebar(page);
    await nav.locator('[data-repo-id="api"]').focus();
    await page.keyboard.press("ArrowDown");
    await expect(nav.locator('[data-repo-id="infra"]')).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("ArrowUp");
    await expect(nav.locator('[data-nav="group:work"]')).toBeFocused();

    await page.keyboard.press("ArrowLeft");
    await expectCall(page, "set_group_collapsed", { groupId: "work", collapsed: true });
    await expect(nav.locator('[data-repo-id="api"]')).toBeHidden();

    await nav.locator('[data-repo-id="dotfiles"]').click();
    await expectCall(page, "set_active_repo", { repoId: "dotfiles" });
    await expect(nav.locator('[data-repo-id="dotfiles"]')).toHaveAttribute("aria-current", "true");
  });

  test("renames a repository with F2", async ({ page }) => {
    await sidebar(page).locator('[data-repo-id="web"]').focus();
    await page.keyboard.press("F2");
    const dialog = page.getByRole("dialog", { name: "Rename repository" });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("textbox").fill("Website");
    await page.keyboard.press("Enter");
    await expectCall(page, "rename_repo", { repoId: "web", name: "Website" });
    await expect(sidebar(page).locator('[data-repo-id="web"]')).toContainText("Website");
  });

  test("offers repository actions in a context menu", async ({ page }) => {
    await sidebar(page).locator('[data-repo-id="web"]').click({ button: "right" });
    await expect
      .poll(() => menu(page))
      .toEqual([
        "Open in VS Code",
        "Rename…",
        "Add to favorites",
        "Move to group",
        "Move to group/No group",
        "Move to group/Side projects",
        "Move to group/Work",
        "Move to group/New group…",
        "Compare branches with…",
        "Remove from Branchboard…",
      ]);

    await clickMenu(page, "Add to favorites");
    await expectCall(page, "set_repo_favorite", { repoId: "web", favorite: true });
    const favorites = sidebar(page).locator('[data-repo-id="web"]');
    await expect(favorites).toBeVisible();
    await expect(sidebar(page).locator(".header .title").first()).toHaveText("Favorites");
  });

  test("removes a repository after confirmation", async ({ page }) => {
    await sidebar(page).locator('[data-repo-id="web"]').focus();
    await page.keyboard.press("Delete");
    await expectCall(page, "remove_repo", { repoId: "web" });
    await expect(sidebar(page).locator('[data-repo-id="web"]')).toHaveCount(0);
  });

  test("keeps a repository when removal is cancelled", async ({ page }) => {
    await page.evaluate(() => {
      window.__fake.setConfirm(false);
    });
    await sidebar(page).locator('[data-repo-id="web"]').focus();
    await page.keyboard.press("Delete");
    await expect
      .poll(async () => page.evaluate(() => window.__fake.calls().length))
      .toBeGreaterThan(0);
    await expect(sidebar(page).locator('[data-repo-id="web"]')).toBeVisible();
  });

  test("offers to locate a missing repository", async ({ page }) => {
    await sidebar(page).locator('[data-repo-id="old-service"]').click();
    await expect(page.getByText("old-service can't be found")).toBeVisible();
    await page.evaluate(() => {
      window.__fake.setPickedFolders(["/home/dev/code/old-service-moved"]);
    });
    await page.getByRole("button", { name: "Locate…" }).click();
    await expectCall(page, "relocate_repo", {
      repoId: "old-service",
      path: "/home/dev/code/old-service-moved",
    });
  });

  test("switches repositories with the quick switcher", async ({ page }) => {
    await page.keyboard.press(`${await mod(page)}+p`);
    const switcher = page.getByRole("dialog", { name: "Switch repository" });
    await expect(switcher).toBeVisible();
    await page.keyboard.type("dot");
    await expect(switcher.getByRole("option")).toHaveCount(1);
    await page.keyboard.press("Enter");
    await expect(switcher).toBeHidden();
    await expectCall(page, "set_active_repo", { repoId: "dotfiles" });
  });
});

test.describe("adding repositories", () => {
  test("starts with an empty state and adds folders from the picker", async ({ page }) => {
    await openApp(page, "empty");
    await expect(page.getByText("No repositories yet")).toBeVisible();
    await page.evaluate(() => {
      window.__fake.setPickedFolders(["/home/dev/code/shop", "/home/dev/code/not-a-repo"]);
    });
    await sidebar(page).getByRole("button", { name: "Add repositories" }).first().click();

    await expect(sidebar(page).getByText("shop")).toBeVisible();
    await expect(
      page.getByText("/home/dev/code/not-a-repo is not a git repository."),
    ).toBeVisible();
  });

  test("adds folders dropped onto the window", async ({ page }) => {
    await openApp(page, "empty");
    await page.evaluate(() => window.__fake.drop(["/home/dev/code/dropped"]));
    await expectCall(page, "add_repo", { path: "/home/dev/code/dropped" });
    await expect(sidebar(page).getByText("dropped")).toBeVisible();
  });
});

test.describe("command palette", () => {
  test("lists applicable commands with shortcuts and runs them", async ({ page }) => {
    await openApp(page);
    await page.keyboard.press(`${await mod(page)}+k`);
    const palette = page.getByRole("dialog", { name: "Command palette" });
    await expect(palette).toBeVisible();
    await expect(palette.getByRole("option", { name: /Open in VS Code/ })).toContainText(/E/);

    await page.keyboard.type("stage all");
    await expect(palette.getByRole("option").first()).toContainText("Stage all changes");
    await page.keyboard.press("Enter");
    await expect(palette).toBeHidden();
    await expectCall(page, "stage_paths", {
      repoId: "api",
      paths: [
        "README.md",
        "config.toml",
        "src/handlers/login.ts",
        "assets/logo.png",
        "data/big.csv",
        "notes.md",
      ],
    });
  });

  test("hides commands that do not apply", async ({ page }) => {
    await openApp(page, "empty");
    await page.keyboard.press(`${await mod(page)}+k`);
    const options = page.getByRole("dialog", { name: "Command palette" }).getByRole("option");
    await expect(options.filter({ hasText: "Add repositories…" })).toHaveCount(1);
    await expect(options.filter({ hasText: "Fetch" })).toHaveCount(0);
  });
});
