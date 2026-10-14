// Renders hero.html to assets/readme-hero.png. Usage: node assets/hero/render.mjs [out.png]
import { chromium } from "@playwright/test";
import { fileURLToPath } from "node:url";
import path from "node:path";

const dir = path.dirname(fileURLToPath(import.meta.url));
const out = process.argv[2] ?? path.join(dir, "..", "readme-hero.png");

const browser = await chromium.launch();
const page = await browser.newPage({
  viewport: { width: 1200, height: 480 },
  deviceScaleFactor: 2,
});
await page.goto(`file://${path.join(dir, "hero.html")}`);
await page.evaluate(() => document.fonts.ready);
await page.screenshot({ path: out, omitBackground: true });
await browser.close();
console.log(`wrote ${out}`);
