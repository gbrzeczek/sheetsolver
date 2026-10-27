// The job and its plan side by side from 1280 px, tabs below; a plan for an
// older job is marked out of date at every width (web/DESIGN.md §2).
import { expect, test, type Page } from "@playwright/test";
import { enterJob, findPlan } from "./helpers";

const jobId = (page: Page) => page.url().split("/jobs/")[1]!.split(/[/?]/)[0]!;

async function solvedJob(page: Page) {
  await enterJob(page, [{ name: "Side", length: "720", width: "560", quantity: "2" }], { length: "2440", width: "1220" }, "1");
  await findPlan(page);
  await expect(page.locator(".headline")).toHaveText("All 2 parts fit", { timeout: 20_000 });
}

test.describe("at the desk", () => {
  test.skip(({ viewport }) => (viewport?.width ?? 0) < 1280, "the workbench needs 1280 px");

  test("the job stays editable next to its plan, which goes out of date on an edit", async ({ page }) => {
    await solvedJob(page);
    // Both halves on screen, no tabs.
    await expect(page.locator("#part-0-quantity")).toBeVisible();
    await expect(page.getByRole("navigation", { name: "Job views" })).toHaveCount(0);
    await expect(page.locator(".stale-bar")).toHaveCount(0);

    // An edit marks the plan out of date; undoing the edit by hand clears it.
    await page.locator("#part-0-quantity").fill("3");
    await expect(page.getByText("The job has changed since this plan was found.")).toBeVisible();
    await expect(page.locator("main.is-stale")).toBeVisible();
    await page.locator("#part-0-quantity").fill("2");
    await expect(page.locator(".stale-bar")).toHaveCount(0);

    // The job name and the time limit do not change the plan.
    await page.locator("#job-name").fill("Wardrobe");
    await page.locator("#time-limit").fill("5");
    await expect(page.locator(".stale-bar")).toHaveCount(0);

    // Find plan again sends the job in the editor; the search streams in beside it.
    const first = jobId(page);
    await page.locator("#part-0-quantity").fill("3");
    await page.getByRole("button", { name: "Find plan again" }).click();
    await expect.poll(() => jobId(page)).not.toBe(first);
    await expect(page.locator(".headline")).toHaveText(/^All 3 parts fit/, { timeout: 20_000 });
    await expect(page.locator(".stale-bar")).toHaveCount(0);
    await expect(page.locator("#part-0-quantity")).toHaveValue("3");
  });

  test("Edit this plan's job takes the plan's job into the editor, with Undo", async ({ page }) => {
    await solvedJob(page);
    await page.locator("#part-0-width").fill("600");
    await page.getByRole("button", { name: "Edit this plan's job" }).click();
    await expect(page.locator("#part-0-width")).toHaveValue("560");
    await expect(page.locator(".stale-bar")).toHaveCount(0);
    await page.getByRole("status").getByRole("button", { name: "Undo" }).click();
    await expect(page.locator("#part-0-width")).toHaveValue("600");
    await expect(page.locator(".stale-bar")).toBeVisible();
  });

  test("Ctrl+Enter finds the plan from any field", async ({ page }) => {
    await enterJob(page, [{ name: "Side", length: "720", width: "560" }], { length: "2440", width: "1220" }, "1");
    await expect(page.getByText("No plan yet")).toBeVisible();
    await page.locator("#part-0-width").press("Control+Enter");
    await expect(page).toHaveURL(/#\/jobs\/[0-9a-f-]+/);
    await expect(page.locator(".headline")).toHaveText("The part fits", { timeout: 20_000 });
  });

  test("a panel opens full screen, and All panels comes back to the workbench", async ({ page }) => {
    await solvedJob(page);
    await page.getByRole("link", { name: "Cut order →" }).first().click();
    await expect(page.getByText(/^Cut 1 of \d+$/)).toBeVisible();
    await expect(page.locator("#part-0-length")).toHaveCount(0);
    await page.getByRole("link", { name: /^All panels/ }).click();
    await expect(page.locator("#part-0-length")).toBeVisible();
    await expect(page.locator(".headline")).toHaveText("All 2 parts fit");
  });
});

test.describe("below 1280 px", () => {
  test.skip(({ viewport }) => (viewport?.width ?? 0) >= 1280, "tabs are for narrow windows");

  test("the tabs stay, and the Cutting plan tab marks a plan for an older job", async ({ page }) => {
    await solvedJob(page);
    await expect(page.getByRole("navigation", { name: "Job views" })).toBeVisible();
    await page.getByRole("link", { name: "Job setup" }).click();
    await page.locator("#part-0-quantity").fill("3");
    await page.getByRole("link", { name: /^Cutting plan/ }).click();
    await expect(page.getByText("The job has changed since this plan was found.")).toBeVisible();
    const first = jobId(page);
    await page.getByRole("button", { name: "Find plan again" }).click();
    await expect.poll(() => jobId(page)).not.toBe(first);
    await expect(page.locator(".headline")).toHaveText(/^All 3 parts fit/, { timeout: 20_000 });
  });
});
