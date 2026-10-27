// Stage 8 exit criterion: the screen budget of web/DESIGN.md §10 holds on
// every screen, at every width, for the mockups' job.
import { expect, test } from "@playwright/test";
import { SCREENS, WIDTHS, budget, eachScreen, measure } from "./screens";

test.setTimeout(240_000);

test("every screen stays within its budget at 1440, 820 and 360 px", async ({ browser, request }, info) => {
  test.skip(info.project.name !== "desktop", "one run sets its own widths");
  const limits = budget();
  expect([...limits.keys()].sort()).toEqual(SCREENS.flatMap((s) => WIDTHS.map((w) => `${s}.${w.width}`)).sort());
  const over: string[] = [];
  await eachScreen(browser, request, info.project.use.baseURL!, async (page, screen, { width }) => {
    const key = `${screen}.${width}`;
    const limit = limits.get(key)!;
    const shown = await measure(page);
    for (const what of ["controls", "prose", "text"] as const) {
      if (shown[what] > limit[what]) over.push(`${key}: ${shown[what]} ${what}, budget ${limit[what]}`);
    }
    // No screen scrolls sideways.
    expect(await page.evaluate(() => document.documentElement.scrollWidth), key).toBeLessThanOrEqual(width);
  });
  expect(over).toEqual([]);
});
