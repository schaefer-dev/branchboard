// Captures the main screens in light and dark mode into
// `e2e-results/screenshots/<browser>/`. These are review artifacts (uploaded
// by CI), not pixel-compared baselines.

import type { Page } from "@playwright/test";

import { changesList, diffPane, expect, mod, openApp, sidebar, test } from "./helpers";

const schemes = ["light", "dark"] as const;

async function capture(page: Page, project: string, name: string, scheme: string) {
  await page.screenshot({ path: `e2e-results/screenshots/${project}/${name}-${scheme}.png` });
}

for (const scheme of schemes) {
  test.describe(`screens (${scheme})`, () => {
    test.use({ colorScheme: scheme });

    test("changes with a diff", async ({ page }, info) => {
      await openApp(page);
      await changesList(page).locator('[data-key="unstaged:src/handlers/login.ts"]').click();
      await expect(diffPane(page).locator(".hunk")).toHaveCount(2);
      await capture(page, info.project.name, "changes", scheme);
    });

    test("commit message", async ({ page }, info) => {
      await openApp(page);
      await page
        .getByRole("textbox", { name: "Commit summary" })
        .fill("Add rate limit headers to the login handler");
      await page
        .getByRole("textbox", { name: "Commit description" })
        .fill("Limits are per user and reset every minute.\nThe headers follow the IETF draft.");
      await capture(page, info.project.name, "commit", scheme);
    });

    test("failed commit", async ({ page }, info) => {
      await openApp(page);
      await page.evaluate(() => {
        window.__fake.failNext(
          "commit",
          "eslint...................................................Failed\n" +
            "src/handlers/login.ts\n  4:10  error  'verify' is defined but never used",
        );
      });
      await page.getByRole("textbox", { name: "Commit summary" }).fill("Add rate limit headers");
      await page.getByRole("button", { name: "Commit 2 files" }).click();
      await expect(page.getByRole("alert")).toBeVisible();
      await capture(page, info.project.name, "commit-failed", scheme);
    });

    test("branches", async ({ page }, info) => {
      await openApp(page);
      await page
        .getByRole("listbox", { name: "Branches" })
        .locator('[data-branch="feature/login-rate-limit"]')
        .click();
      await expect(page.getByRole("region", { name: "Branch review" })).toBeVisible();
      await expect(page.getByText("Fetched 3 min ago")).toBeVisible();
      await capture(page, info.project.name, "branches", scheme);
    });

    test("branch review", async ({ page }, info) => {
      await openApp(page);
      await page
        .getByRole("listbox", { name: "Branches" })
        .locator('[data-branch="feature/login-rate-limit"]')
        .click();
      const review = page.getByRole("region", { name: "Branch review" });
      await review
        .getByRole("listbox", { name: "Files in this review" })
        .getByRole("option", { name: /rate-limit\.ts/ })
        .click();
      await expect(review.getByText("export const LIMIT = 60;")).toBeVisible();
      await capture(page, info.project.name, "review", scheme);
    });

    test("quick switcher", async ({ page }, info) => {
      await openApp(page);
      await page.keyboard.press(`${await mod(page)}+p`);
      await page.keyboard.type("in");
      await expect(page.getByRole("dialog", { name: "Switch repository" })).toBeVisible();
      await capture(page, info.project.name, "quick-switcher", scheme);
    });

    test("missing repository", async ({ page }, info) => {
      await openApp(page);
      await sidebar(page).locator('[data-repo-id="old-service"]').click();
      await expect(page.getByText("old-service can't be found")).toBeVisible();
      await capture(page, info.project.name, "missing-repository", scheme);
    });

    test("agents", async ({ page }, info) => {
      await openApp(page, "agents");
      await expect(sidebar(page).locator('[data-repo-id="api"]').getByRole("img")).toHaveAttribute(
        "data-state",
        "blocked",
      );
      await capture(page, info.project.name, "agents", scheme);
    });

    test("terminal", async ({ page }, info) => {
      await openApp(page, "agents");
      await page
        .getByRole("group", { name: "Right pane" })
        .getByRole("button", { name: "Terminal" })
        .click();
      // Something like herdr's tab bar and a Claude Code session.
      await page.evaluate(() => {
        window.__fake.terminalOutput(
          "\x1b[2J\x1b[H\x1b[7m 1 agents \x1b[0m  2 shell   +\r\n\r\n" +
            "\x1b[1m> \x1b[0mAdd rate limit headers to the login handler\r\n\r\n" +
            "\x1b[32m●\x1b[0m Update(src/handlers/login.ts)\r\n" +
            '  \x1b[32m+ res.setHeader("RateLimit-Remaining", left);\x1b[0m\r\n\r\n' +
            "\x1b[33m Do you want to make this edit? \x1b[0m\r\n \x1b[1m❯ 1. Yes\x1b[0m\r\n   2. No\r\n",
        );
      });
      await expect(page.getByRole("region", { name: "Terminal" })).toBeVisible();
      await capture(page, info.project.name, "terminal", scheme);
    });

    test("terminal without a workspace", async ({ page }, info) => {
      await openApp(page, "agents");
      await page
        .getByRole("group", { name: "Right pane" })
        .getByRole("button", { name: "Terminal" })
        .click();
      await sidebar(page).locator('[data-repo-id="blog"]').click();
      await expect(page.getByText("blog has no herdr workspace yet.")).toBeVisible();
      await capture(page, info.project.name, "terminal-new-workspace", scheme);
    });

    test("empty workspace", async ({ page }, info) => {
      await openApp(page, "empty");
      await expect(page.getByText("No repositories yet")).toBeVisible();
      await capture(page, info.project.name, "empty", scheme);
    });
  });
}
