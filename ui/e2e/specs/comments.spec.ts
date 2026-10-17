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

const LOGIN = "src/handlers/login.ts";

function reviewBar(page: Page) {
  return page.getByRole("region", { name: "Review comments" });
}

function diffLine(page: Page, index: number) {
  return diffPane(page).locator(`[data-index="${String(index)}"]`);
}

/** Clicks the comment button of a diff line; `shift` extends from the last one. */
async function commentOn(page: Page, index: number, shift = false) {
  const line = diffLine(page, index);
  await line.hover();
  await line
    .getByRole("button", { name: "Comment on this line" })
    .click(shift ? { modifiers: ["Shift"] } : {});
}

async function writeComment(page: Page, text: string) {
  const editor = page.getByRole("form", { name: "Write a comment" });
  await expect(editor.getByRole("textbox", { name: "Comment" })).toBeFocused();
  await page.keyboard.type(text);
  await page.keyboard.press(`${await mod(page)}+Enter`);
  await expect(editor).toHaveCount(0);
}

async function showFile(page: Page, path: string) {
  await changesList(page).locator(`[data-key="unstaged:${path}"]`).click();
}

test.describe("review comments", () => {
  test("keeps a large diff virtualized after adding a measured comment row", async ({ page }) => {
    await openApp(page);
    await showFile(page, "data/big.csv");
    await diffPane(page).getByRole("button", { name: "Comment on this file" }).click();
    await writeComment(page, "Keep the generated data out of this review.");
    await diffPane(page).getByRole("button", { name: "Show diff" }).click();
    const viewport = diffPane(page).getByLabel("Diff", { exact: true });
    await expect(diffPane(page).locator('[data-index="0"]')).toBeVisible();
    await expect.poll(() => diffPane(page).locator("[data-index]").count()).toBeLessThan(100);
    await viewport.evaluate((element) => {
      element.scrollTop = element.scrollHeight;
    });
    await expect(diffPane(page).locator('[data-index="24999"]')).toBeVisible();
    await expect.poll(() => diffPane(page).locator("[data-index]").count()).toBeLessThan(100);
    await viewport.evaluate((element) => {
      element.scrollTop = 0;
    });
    await expect(diffPane(page).getByRole("article", { name: "Comment on file" })).toBeVisible();
  });

  test("keeps changed-line comments in the list as outdated", async ({ page }) => {
    await openApp(page);
    await showFile(page, LOGIN);
    await commentOn(page, 4);
    await writeComment(page, "Check the cache.");
    await page.evaluate(async () => {
      await window.__fake.replaceDiff("api", "unstaged:src/handlers/login.ts", {
        notes: [],
        binary: false,
        tooLarge: false,
        additions: 1,
        deletions: 0,
        lineCount: 1,
        hunks: [
          {
            header: "@@ -0,0 +5 @@",
            oldStart: 0,
            newStart: 5,
            lines: [
              {
                kind: "added",
                oldLine: null,
                newLine: 5,
                text: "  const users = loadUsers({ cache: false });",
                tokens: [],
                noNewline: false,
              },
            ],
          },
        ],
      });
    });
    await expect(diffPane(page).getByRole("article")).toHaveCount(0);
    await reviewBar(page).getByRole("button", { name: "1 pending" }).click();
    await expect(reviewBar(page)).toContainText("Outdated");
    await expect(reviewBar(page)).toContainText("Check the cache.");
  });

  test("comments on binary and rename-only files", async ({ page }) => {
    await openApp(page);
    await showFile(page, "assets/logo.png");
    await diffPane(page).getByRole("button", { name: "Comment on this file" }).click();
    await writeComment(page, "Use the updated logo.");
    await expect(diffPane(page).getByRole("article", { name: "Comment on file" })).toContainText(
      "Use the updated logo.",
    );
    await changesList(page).locator('[data-key="staged:legacy-notes.txt"]').click();
    await diffPane(page).getByRole("button", { name: "Comment on this file" }).click();
    await writeComment(page, "Check references after the rename.");
    await expect(diffPane(page).getByRole("article", { name: "Comment on file" })).toContainText(
      "Check references after the rename.",
    );
    await expect(reviewBar(page)).toContainText("2 pending");
  });

  test("sends directly when the repository has one agent", async ({ page }) => {
    await openApp(page, "agents");
    await page.evaluate(() =>
      window.__fake.setAgents({
        available: true,
        repos: [
          {
            repoId: "api",
            state: "idle",
            agents: [{ paneId: "p1", agent: "claude", state: "idle", title: null }],
          },
        ],
        agents: [
          {
            paneId: "p1",
            repoId: "api",
            agent: "claude",
            state: "idle",
            title: null,
            workspace: "api",
            tab: null,
          },
        ],
        focusedRepoId: "api",
      }),
    );
    await showFile(page, LOGIN);
    await commentOn(page, 4);
    await writeComment(page, "Check the cache.");
    await reviewBar(page).getByRole("button", { name: "Send review" }).click();
    await expectCall(page, "paste_to_agent", {
      paneId: "p1",
      text: `${LOGIN}:5\n\`\`\`ts\n  const users = loadUsers({ cache: true });\n\`\`\`\nCheck the cache.`,
    });
    await expect(page.getByRole("region", { name: "Terminal" })).toBeVisible();
  });

  test("comments on a line and copies the review without an agent", async ({ page }) => {
    // Records what the app copies instead of using the system clipboard.
    await page.addInitScript(() => {
      Object.defineProperty(navigator, "clipboard", {
        value: {
          writeText: (text: string) => {
            document.body.dataset.copied = text;
            return Promise.resolve();
          },
        },
      });
    });
    await openApp(page);
    await showFile(page, LOGIN);
    await commentOn(page, 4);
    await writeComment(page, "Pass the cache flag explicitly.");

    const card = diffPane(page).getByRole("article", { name: "Comment on line 5" });
    await expect(card).toContainText("Pending");
    await expect(card).toContainText("Pass the cache flag explicitly.");
    const bar = reviewBar(page);
    await expect(bar).toContainText("1 pending");
    await expect(bar.getByRole("button", { name: "Send review" })).toHaveCount(0);

    await bar.getByRole("button", { name: "Copy review" }).click();
    await expect
      .poll(() => page.evaluate(() => document.body.dataset.copied))
      .toBe(
        `${LOGIN}:5\n\`\`\`ts\n  const users = loadUsers({ cache: true });\n\`\`\`\nPass the cache flag explicitly.`,
      );
    await expect(bar).toContainText("1 sent");
    await expect(card).toContainText("Sent");
    await expect(bar.getByRole("button", { name: "Copy review" })).toBeDisabled();

    // The list of all comments; resolving one removes it.
    await bar.getByRole("button", { name: "1 sent" }).click();
    await expect(bar.getByRole("list", { name: "Comments" })).toContainText(`${LOGIN}:5`);
    await card.getByRole("button", { name: "Resolve" }).click();
    await expect(bar).toHaveCount(0);
  });

  test("comments on several lines by Shift-click and by dragging", async ({ page }) => {
    await openApp(page);
    await showFile(page, LOGIN);
    const editor = page.getByRole("form", { name: "Write a comment" });

    await commentOn(page, 4);
    await expect(editor).toContainText("Line 5");
    await commentOn(page, 6, true);
    await expect(editor).toContainText("Lines 5–7");
    await page.keyboard.press("Escape");
    await expect(editor).toHaveCount(0);

    // Dragging stops at the end of the hunk.
    const from = diffLine(page, 9);
    await from.hover();
    const button = await from.getByRole("button", { name: "Comment on this line" }).boundingBox();
    const to = await diffLine(page, 14).boundingBox();
    if (!button || !to) throw new Error("The diff lines are not on screen");
    await page.mouse.move(button.x + button.width / 2, button.y + button.height / 2);
    await page.mouse.down();
    await page.mouse.move(to.x + 200, to.y + to.height / 2, { steps: 5 });
    await page.mouse.up();
    await expect(editor).toContainText("Lines 9–11");

    // Removed lines only are commented on the old side.
    await page.keyboard.press("Escape");
    await commentOn(page, 7);
    await expect(editor).toContainText("Removed line 7");
  });

  test("comments on a whole file from the context menu", async ({ page }) => {
    await openApp(page);
    await changesList(page).locator('[data-key="unstaged:config.toml"]').click({ button: "right" });
    await clickMenu(page, "Comment on file");
    await writeComment(page, "Document the host.");
    await expect(diffPane(page).getByRole("article", { name: "Comment on file" })).toContainText(
      "Document the host.",
    );
  });

  test("comments in the branch review", async ({ page }) => {
    await openApp(page);
    await page.keyboard.press(`${await mod(page)}+2`);
    const review = page.getByRole("region", { name: "Branch review" });
    await review.getByRole("button", { name: "Comment on this file" }).click();
    await writeComment(page, "Explain the limit.");
    await expect(review.getByRole("article", { name: "Comment on file" })).toBeVisible();
    await expect(reviewBar(page)).toContainText("1 pending");
  });

  test("comments on a single commit and on staged changes", async ({ page }) => {
    await openApp(page);
    await changesList(page).locator('[data-key="staged:config.toml"]').click();
    await commentOn(page, 2);
    await writeComment(page, "Check this port.");
    await page.keyboard.press(`${await mod(page)}+2`);
    const review = page.getByRole("region", { name: "Branch review" });
    await review
      .getByRole("listbox", { name: "Commits" })
      .locator('[data-target^="commit:"]')
      .first()
      .click();
    await review.getByRole("button", { name: "Comment on this file" }).click();
    await writeComment(page, "Explain this commit.");
    await expect(review.getByRole("article", { name: "Comment on file" })).toContainText(
      "Explain this commit.",
    );
    await expect(reviewBar(page)).toContainText("2 pending");
  });

  test("a comment link restores its review target and file", async ({ page }) => {
    await openApp(page);
    await page.keyboard.press(`${await mod(page)}+2`);
    const review = page.getByRole("region", { name: "Branch review" });
    const files = review.getByRole("listbox", { name: "Files in this review" });
    await files.locator('[data-key="src/rate-limit.ts"]').click({ button: "right" });
    await clickMenu(page, "Comment on file");
    await writeComment(page, "Explain the limit.");
    await page
      .getByRole("listbox", { name: "Branches" })
      .locator('[data-branch="fix/header-overflow"]')
      .click();
    await expect(files.locator('[data-key="header.css"]')).toBeVisible();
    const bar = reviewBar(page);
    await bar.getByRole("button", { name: "1 pending" }).click();
    await bar.getByRole("button", { name: "src/rate-limit.ts (whole file)" }).click();
    await expect(files.locator('[data-key="src/rate-limit.ts"]')).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(review.getByRole("article", { name: "Comment on file" })).toContainText(
      "Explain the limit.",
    );
  });

  test("pastes the review into the agent the user picks, the last one used first", async ({
    page,
  }) => {
    await openApp(page, "agents");
    await showFile(page, LOGIN);
    await commentOn(page, 4);
    await writeComment(page, "Pass the cache flag explicitly.");

    const bar = reviewBar(page);
    await bar.getByRole("button", { name: "Send review" }).click();
    await expect
      .poll(() => menu(page))
      .toEqual([
        "Codex · Add rate limit headers (waiting for input)",
        "Claude · Refactor the login handler",
      ]);
    await clickMenu(page, "Claude · Refactor the login handler");
    await expectCall(page, "paste_to_agent", {
      paneId: "w1:p1",
      text: `${LOGIN}:5\n\`\`\`ts\n  const users = loadUsers({ cache: true });\n\`\`\`\nPass the cache flag explicitly.`,
    });
    await expectCall(page, "focus_agent", { paneId: "w1:p1" });
    await expect(page.getByRole("region", { name: "Terminal" })).toBeVisible();

    await page.keyboard.press(`${await mod(page)}+1`);
    await expect(bar).toContainText("1 sent");
    await commentOn(page, 8);
    await writeComment(page, "Keep the message short.");
    await bar.getByRole("button", { name: "Send review" }).click();
    await expect
      .poll(() => menu(page))
      .toEqual([
        "Claude · Refactor the login handler",
        "Codex · Add rate limit headers (waiting for input)",
      ]);
  });
});
