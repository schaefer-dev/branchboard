// Budgets for the embedded terminal (ADR 0013): memory it adds, how it keeps
// up with herdr redrawing the screen, and that it costs nothing while idle.
// Chromium only, for its DevTools metrics; numbers are attached to the report.

import type { CDPSession, Page } from "@playwright/test";

import { expect, openApp, test } from "./helpers";

const MB = 1024 * 1024;

async function metrics(cdp: CDPSession): Promise<Record<string, number>> {
  const { metrics: list } = await cdp.send("Performance.getMetrics");
  return Object.fromEntries(list.map((metric) => [metric.name, metric.value]));
}

async function heap(cdp: CDPSession): Promise<number> {
  await cdp.send("HeapProfiler.collectGarbage");
  return (await metrics(cdp)).JSHeapUsedSize ?? 0;
}

async function showTerminal(page: Page): Promise<void> {
  await page
    .getByRole("group", { name: "Right pane" })
    .first()
    .getByRole("button", { name: "Terminal" })
    .click();
  await expect
    .poll(() =>
      page.evaluate(() => window.__terminal?.buffer.active.getLine(0)?.translateToString()),
    )
    .toContain("herdr (fake)");
}

test.describe("embedded terminal budgets", () => {
  test.skip(({ browserName }) => browserName !== "chromium", "needs Chromium's DevTools metrics");

  test("adds little memory and keeps up with herdr redrawing", async ({ page }, info) => {
    await openApp(page, "agents");
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Performance.enable");
    await cdp.send("HeapProfiler.enable");

    const before = await heap(cdp);
    await showTerminal(page);
    await page.waitForTimeout(300);
    const added = (await heap(cdp)) - before;
    info.annotations.push({ type: "heap added", description: `${(added / MB).toFixed(1)} MB` });
    expect(added).toBeLessThan(30 * MB);

    // 300 full-screen redraws with colors, as herdr sends while an agent
    // streams output, delivered in 16 KB chunks like the PTY coalescing.
    const result = await page.evaluate(async () => {
      const term = window.__terminal;
      if (!term) throw new Error("no terminal");
      const frame = (index: number) => {
        let text = "\x1b[H";
        for (let row = 0; row < term.rows; row++) {
          const color = 31 + ((row + index) % 6);
          text += `\x1b[${String(color)}m${`line ${String(index)}:${String(row)} `.repeat(12).slice(0, term.cols - 1)}\x1b[0m\r\n`;
        }
        return text;
      };
      const stream = Array.from({ length: 300 }, (_, index) => frame(index)).join("");
      let longestGap = 0;
      let last = performance.now();
      let sampling = true;
      const sample = () => {
        const now = performance.now();
        longestGap = Math.max(longestGap, now - last);
        last = now;
        if (sampling) requestAnimationFrame(sample);
      };
      requestAnimationFrame(sample);
      const start = performance.now();
      for (let offset = 0; offset < stream.length; offset += 16 * 1024) {
        window.__fake.terminalOutput(stream.slice(offset, offset + 16 * 1024));
        await new Promise((resolve) => setTimeout(resolve, 4));
      }
      await new Promise<void>((resolve) => {
        term.write("", resolve);
      });
      const elapsed = performance.now() - start;
      sampling = false;
      return { bytes: stream.length, elapsed, longestGap };
    });
    info.annotations.push({
      type: "redraw stream",
      description: `${(result.bytes / MB).toFixed(1)} MB in ${result.elapsed.toFixed(0)} ms, longest frame gap ${result.longestGap.toFixed(0)} ms`,
    });
    expect(result.elapsed).toBeLessThan(10_000);
    expect(result.longestGap).toBeLessThan(500);
  });

  test("costs no CPU while herdr is idle", async ({ page }, info) => {
    await openApp(page, "agents");
    await showTerminal(page);
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Performance.enable");
    await page.waitForTimeout(500);
    const start = await metrics(cdp);
    await page.waitForTimeout(3000);
    const end = await metrics(cdp);
    const busy = (end.TaskDuration ?? 0) - (start.TaskDuration ?? 0);
    info.annotations.push({
      type: "idle CPU",
      description: `${(busy * 1000).toFixed(0)} ms busy in 3 s`,
    });
    // A blinking cursor or a timer would show up here.
    expect(busy).toBeLessThan(0.15);
  });
});
