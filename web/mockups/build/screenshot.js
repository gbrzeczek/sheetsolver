// Screenshot every mockup at desktop and tablet widths, light and dark.
//
//   node web/mockups/build/screenshot.js [page-name ...]
//
// Uses the Playwright and Chromium already installed in the environment.
const { chromium } = require("playwright");
const path = require("path");

const dir = path.resolve(__dirname, "..");
const pages = ["editor", "editor-errors", "solving", "plan-panel", "plan-overview", "states"];
const viewports = {
  desktop: { width: 1440, height: 900 },
  tablet: { width: 820, height: 1180 }, // iPad Air, portrait
};

(async () => {
  const only = process.argv.slice(2);
  const browser = await chromium.launch();
  for (const scheme of ["light", "dark"]) {
    for (const [vpName, viewport] of Object.entries(viewports)) {
      const context = await browser.newContext({
        viewport,
        colorScheme: scheme,
        deviceScaleFactor: 1,
        hasTouch: vpName === "tablet",
      });
      const page = await context.newPage();
      for (const name of pages) {
        if (only.length && !only.includes(name)) continue;
        await page.goto("file://" + path.join(dir, name + ".html"));
        const file = path.join(dir, "screenshots", `${name}.${vpName}.${scheme}.png`);
        // Grow the viewport to the whole page instead of fullPage, so
        // sticky bars sit where they would at the end of a scroll.
        await page.setViewportSize(viewport);
        const height = await page.evaluate(() => document.documentElement.scrollHeight);
        await page.setViewportSize({ width: viewport.width, height });
        await page.screenshot({ path: file });
        console.log("wrote", path.relative(process.cwd(), file));
      }
      await context.close();
    }
  }
  await browser.close();
})();
