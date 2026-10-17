import { expect, expectCall, mod, openApp, test, changesList, sidebar } from "./helpers";

test("History works without herdr, pages virtually, searches the complete history and retains browsing state", async ({
  page,
}) => {
  await openApp(page);
  await page.keyboard.press(`${await mod(page)}+4`);
  const pane = page.getByRole("region", { name: "History", exact: true });
  const list = pane.getByRole("listbox", { name: "History commits" });
  await expect(pane).toBeVisible();
  await expect(pane.getByRole("button", { name: "Terminal", exact: true })).toHaveCount(0);
  await expect(list.getByRole("option").first()).toBeVisible();
  expect(await list.getByRole("option").count()).toBeLessThan(100);
  await pane.getByRole("button", { name: "Load more" }).click();
  await expectCall(page, "get_history", {
    repoId: "api",
    request: expect.objectContaining({ cursor: expect.objectContaining({ offset: 100 }) }),
  });
  await pane.getByRole("button", { name: "Load more" }).click();
  await expect(pane.getByRole("button", { name: "Load more" })).toHaveCount(0);
  expect(await list.getByRole("option").count()).toBeLessThan(100);
  await list.evaluate((element) => {
    element.scrollTop = 1400;
  });
  await expect.poll(() => list.evaluate((element) => element.scrollTop)).toBeGreaterThan(1000);
  await changesList(page).locator('[data-key="unstaged:src/handlers/login.ts"]').click();
  await page.getByRole("button", { name: "History", exact: true }).click();
  await expect.poll(() => list.evaluate((element) => element.scrollTop)).toBeGreaterThan(1000);
  await pane.getByRole("searchbox", { name: "Search history" }).fill("[LITERAL.*]");
  await expect(list.getByRole("option")).toHaveCount(1);
  await expect(list).toContainText("Historical needle");
  await changesList(page).locator('[data-key="unstaged:src/handlers/login.ts"]').click();
  await expect(pane).not.toBeVisible();
  await page.getByRole("button", { name: "History", exact: true }).click();
  await expect(list).toContainText("Historical needle");
  await sidebar(page).locator('[data-repo-id="web"]').click();
  await expect(pane.getByRole("searchbox")).toHaveValue("");
  await sidebar(page).locator('[data-repo-id="api"]').click();
  await expect(pane.getByRole("searchbox")).toHaveValue("[LITERAL.*]");
});

test("local-branch browsing never checks out, and watcher changes preserve the snapshot until Refresh", async ({
  page,
}) => {
  await openApp(page);
  await page.getByRole("button", { name: "History", exact: true }).click();
  const pane = page.getByRole("region", { name: "History", exact: true });
  await pane.getByLabel("History reference").selectOption("refs/heads/fix/header-overflow");
  await expect(pane.getByRole("listbox", { name: "History commits" })).toContainText(
    "update header.css",
  );
  const selected = pane.locator('[aria-selected="true"][data-oid]');
  const oid = await selected.getAttribute("data-oid");
  await expect(pane.getByRole("listbox", { name: "Files in history commit" })).toContainText(
    "header.css",
  );
  await page.evaluate(() => window.__fake.moveHistoryRef("api", "refs/heads/fix/header-overflow"));
  await expect(pane.getByText("History changed ·")).toBeVisible();
  await expect(selected).toHaveAttribute("data-oid", oid ?? "");
  await pane.getByRole("button", { name: "Load more" }).click();
  await expect(pane.getByText("New history tip", { exact: true })).toHaveCount(0);
  await pane.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(pane.getByText("New history tip", { exact: true })).toBeVisible();
  await expect(selected).toHaveAttribute("data-oid", oid ?? "");
  const switches = await page.evaluate(() =>
    window.__fake.calls().filter((c) => c.cmd === "switch_branch"),
  );
  expect(switches).toEqual([]);
});

for (const send of [false, true]) {
  test(`History comments ${send ? "send to an agent" : "copy without herdr"} and reveal outside the current search`, async ({
    page,
  }) => {
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
    await openApp(page, send ? "agents" : "default");
    if (send)
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
    await page.keyboard.press(`${await mod(page)}+4`);
    const pane = page.getByRole("region", { name: "History", exact: true });
    const selected = pane.locator('[aria-selected="true"][data-oid]');
    const oid = await selected.getAttribute("data-oid");
    await pane.getByRole("button", { name: "Comment on file", exact: true }).click();
    await page
      .getByRole("textbox", { name: "Comment", exact: true })
      .fill("Check this historical change.");
    await page.keyboard.press(`${await mod(page)}+Enter`);
    const bar = page.getByRole("region", { name: "Review comments" });
    await bar.getByRole("button", { name: send ? "Send review" : "Copy review" }).click();
    if (send) {
      await expectCall(page, "paste_to_agent", {
        paneId: "p1",
        text: expect.stringContaining("Check this historical change."),
      });
      await page.keyboard.press(`${await mod(page)}+4`);
    } else {
      await expect
        .poll(() => page.evaluate(() => document.body.dataset.copied))
        .toContain("Check this historical change.");
    }
    await pane.getByRole("searchbox").fill("no matching commit");
    await expect(pane.getByText("No matching commits.")).toBeVisible();
    await bar.getByRole("button", { name: "1 sent", exact: true }).click();
    await bar.getByTitle("Show this comment").click();
    await expect(pane.getByText(/Showing linked commit/)).toBeVisible();
    await expect(pane.getByRole("article", { name: "Comment on file" })).toContainText(
      "Check this historical change.",
    );
    await expectCall(page, "get_review_files", { repoId: "api", target: { kind: "commit", oid } });
  });
}
