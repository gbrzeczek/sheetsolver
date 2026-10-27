// Screenshots and measures of every screen at 1440, 820 and 360 px, for
// web/DESIGN.md §10 and web/mockups/screenshots/. Runs only with
// SCREENS_DIR set: `npm run screens` (writes into web/mockups/screenshots).
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "@playwright/test";
import { eachScreen, measure, type Measure } from "./screens";

const dir = process.env.SCREENS_DIR;

test.skip(!dir, "set SCREENS_DIR to write the screenshots");
test.setTimeout(240_000);

test("every screen at every width", async ({ browser, request }, info) => {
  test.skip(info.project.name !== "desktop", "one run sets its own widths");
  mkdirSync(dir!, { recursive: true });
  const results: Record<string, Measure> = {};
  await eachScreen(browser, request, info.project.use.baseURL!, async (page, screen, { width, height }) => {
    results[`${screen}.${width}`] = await measure(page);
    // Grow the viewport to the whole page instead of fullPage, so sticky bars
    // sit where they would at the end of a scroll (as the mockups' script does).
    const full = await page.evaluate(() => document.documentElement.scrollHeight);
    await page.setViewportSize({ width, height: Math.max(height, full) });
    await page.waitForTimeout(200);
    await page.screenshot({ path: join(dir!, `${screen}.${width}.png`) });
    await page.setViewportSize({ width, height });
  });
  writeFileSync(join(dir!, "measures.json"), JSON.stringify(results, null, 2) + "\n");
});
