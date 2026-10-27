// Stage 7 exit criterion: enter an instance → solve → see a valid plan.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { enterJob, findPlan } from "./helpers";

test("enter a job, solve it, and follow the plan cut by cut", async ({ page, request }, info) => {
  await enterJob(
    page,
    [
      { name: "Side", length: "720", width: "560", quantity: "2", grain: true },
      { length: "564", width: "300.5", quantity: "3" }, // no name: becomes "Part 2"
      { name: "Shelf", length: "1100", width: "400" },
    ],
    { name: "Birch ply 18 mm", length: "2440", width: "1220" },
  );
  await page.locator("#job-name").fill("Test cabinet");
  await expect(page.locator("#part-1-id")).toHaveAttribute("placeholder", "Part 2");
  await findPlan(page);

  // The search runs to its 2 s limit, then the plan answers "did it fit?".
  await expect(page.locator(".headline")).toHaveText("All 6 parts fit", { timeout: 20_000 });
  await expect(page.getByText("✓ Complete plan")).toBeVisible();
  await expect(page.getByText("Test cabinet · plan")).toBeVisible();
  await expect(page.getByText("✓ No missing parts")).toBeVisible();

  // The served plan passed the API's checker; it cuts every part, and the
  // generated name went to the server.
  const id = page.url().split("/jobs/")[1]!.split(/[/?]/)[0]!;
  const job = await (await request.get(`/jobs/${id}`)).json();
  expect(job.state).toBe("done");
  expect(job.best.metrics.parts_cut).toBe(6);
  expect(job.instance.parts.map((p: { id: string }) => p.id)).toEqual(["Side", "Part 2", "Shelf"]);
  expect(job.name).toBe("Test cabinet");

  // Checklist: every part, cut as often as needed.
  const checklist = page.locator("section", { hasText: "Parts checklist" });
  await expect(checklist.getByRole("row")).toHaveCount(4);
  await expect(checklist.getByRole("cell", { name: "Part 2" })).toBeVisible();

  // One panel, full screen at every width: drawing, current cut, cut order.
  await page.getByRole("link", { name: "Cut order →" }).first().click();
  await expect(page.getByRole("img", { name: /Panel 1 of \d: Birch ply 18 mm/ })).toBeVisible();
  const cuts = page.locator(".wb-cuts .cutlist .cut");
  const count = await cuts.count();
  expect(count).toBeGreaterThan(2);
  await expect(page.getByText(`Cut 1 of ${count}`)).toBeVisible();
  await page.getByRole("button", { name: "Next cut" }).click();
  await expect(page.getByText(`Cut 2 of ${count}`)).toBeVisible();
  await expect(page).toHaveURL(/cut=2/);
  await expect(page.locator('.wb-cuts .cut[aria-current="step"] .n')).toHaveText("2");

  // Reloading keeps the place: the job comes back from the server, name
  // included, even without this browser's draft (another device).
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.getByText(`Cut 2 of ${count}`)).toBeVisible();
  await expect(page.locator(".context")).toHaveText("Test cabinet");

  // Printing gives one page per panel: drawing, cut order, parts.
  await page.goto(page.url().replace(/\/panels\/.*$/, ""));
  await expect(page.getByRole("button", { name: "Print" })).toBeVisible();
  const panels = job.best.plan.patterns.length;
  await page.emulateMedia({ media: "print" });
  await expect(page.locator(".print-sheet")).toHaveCount(panels);
  await expect(page.locator(".print-sheet").first()).toBeVisible();
  await expect(page.locator(".print-sheet").first().locator(".cutlist .cut")).toHaveCount(count);
  await expect(page.locator("main")).toBeHidden();
  await page.emulateMedia({ media: "screen" });
  await expect(page.locator(".print-sheet").first()).toBeHidden();

  // Edit job brings the job back, the generated name still generated. On
  // the workbench the editor is already on screen, holding this browser's
  // new draft, so the plan is out of date and offers to take its job back.
  if (info.project.name === "desktop") {
    await expect(page.getByText("The job has changed since this plan was found.")).toBeVisible();
    await page.getByRole("button", { name: "Edit this plan's job" }).click();
    await expect(page.locator(".stale-bar")).toBeHidden();
  } else {
    await page.getByRole("button", { name: "Edit job" }).click();
  }
  await expect(page.locator("#part-1-id")).toHaveValue("");
  await expect(page.locator("#part-0-id")).toHaveValue("Side");
});

test("stop a running search and keep the best plan so far", async ({ page }) => {
  // "Best use of the panels I have" on two panels: 88 parts that cannot all
  // fit, so the search has no bound to stop at and runs until it is stopped.
  const instance = JSON.parse(readFileSync(join(import.meta.dirname, "../../data/instances/generated__knapsack_3_non_exact_kerf3_2.json"), "utf8"));
  const parts = instance.parts.map((p: { length: number; width: number; quantity: number; grain?: boolean }) => ({
    length: String(p.length), width: String(p.width), quantity: String(p.quantity), grain: p.grain,
  }));
  await enterJob(page, parts, { length: "2800", width: "2070", quantity: "2" }, "60");
  await page.locator("#objective").selectOption("knapsack");
  await findPlan(page);
  await expect(page.getByRole("progressbar", { name: "Search time" })).toBeVisible();
  await expect(page.locator(".headline")).toHaveText(/^\d+ of 88 parts fit so far$/, { timeout: 20_000 });
  await expect(page.getByText("Plans found")).toBeVisible();
  await page.getByRole("button", { name: "Stop and keep this plan" }).click();
  await expect(page.getByText(/stopped after/)).toBeVisible({ timeout: 10_000 });
  await expect(page.locator(".headline")).toHaveText(/^\d+ of 88 parts fit$/);
  // Parts not cut come first in the checklist, marked as such.
  await expect(page.locator("tr.not-cut").first()).toBeVisible();
});

test("a part larger than every panel gives the no-plan state with its cause", async ({ page }) => {
  await enterJob(page, [{ name: "Worktop", length: "2600", width: "600", grain: true }], { length: "2440", width: "1220" }, "1");
  await findPlan(page);
  await expect(page.getByRole("heading", { name: "Not every part fits" })).toBeVisible({ timeout: 20_000 });
  await expect(page.getByText(/“Worktop”, 2\u202f600 × 600 mm/)).toBeVisible();
});
