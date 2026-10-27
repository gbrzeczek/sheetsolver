// On a phone the job setup fits the screen: the parts and panels tables
// shrink instead of widening the page, and column help stays on screen.
import { expect, test } from "@playwright/test";

test.use({ viewport: { width: 360, height: 800 }, hasTouch: true });

test.describe("on a phone", () => {
  test("the job setup never scrolls sideways", async ({ page }, info) => {
    test.skip(info.project.name !== "tablet", "one run is enough; the viewport is set here");
    await page.goto("/");
    await expect(page.locator("#part-0-id")).toBeVisible();
    const pageWidth = () => page.evaluate(() => document.documentElement.scrollWidth);
    expect(await pageWidth()).toBeLessThanOrEqual(360);

    // The rightmost column's help opens within the screen.
    await page.getByRole("button", { name: "About Keep grain direction" }).click();
    const pop = page.getByRole("dialog", { name: "Keep grain direction" });
    await expect(pop).toBeVisible();
    const box = (await pop.boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(360);
    expect(await pageWidth()).toBeLessThanOrEqual(360);
  });
});
