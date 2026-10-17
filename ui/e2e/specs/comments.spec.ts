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
