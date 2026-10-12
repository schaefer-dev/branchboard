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
      await changesList(page).locator('[data-key="unstaged:src/handlers/login.rs"]').click();
      await expect(diffPane(page).locator(".hunk")).toHaveCount(2);
      await capture(page, info.project.name, "changes", scheme);
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

    test("empty workspace", async ({ page }, info) => {
      await openApp(page, "empty");
      await expect(page.getByText("No repositories yet")).toBeVisible();
      await capture(page, info.project.name, "empty", scheme);
    });
  });
}
