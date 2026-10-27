// Stage 8 exit criterion: every collapsed section opens, and its summary
// line reflects the values inside (web/DESIGN.md §10).
import { expect, test, type Locator, type Page } from "@playwright/test";
import { loadReferenceDraft, submitReference, waitDone } from "./screens";

/** The section (a `details.fold`, or the settings) whose summary starts with `title`. */
const section = (page: Page, title: string) => page.locator("details", { has: page.locator("summary", { hasText: title }) });
// innerText, so the title and the values (blocks of their own) are separate words.
const summaryOf = async (details: Locator) => (await details.locator("summary").innerText()).replace(/\s+/g, " ").trim();

async function open(details: Locator) {
  await expect(details).not.toHaveAttribute("open", "");
  await details.locator("summary").click();
  await expect(details).toHaveAttribute("open", "");
}

test("job setup: More settings sums up what is inside", async ({ page }) => {
  await loadReferenceDraft(page);
  const more = section(page, "More settings");
  expect(await summaryOf(more)).toBe("More settings Kitchen units · 3 stages · trimming allowed · along the length first · 2 s");
  await open(more);
  await expect(page.locator("#job-name")).toHaveValue("Kitchen units");
  await expect(page.locator("#time-limit")).toHaveValue("2");

  await page.locator("#job-name").fill("");
  await page.locator(".radio-row label", { hasText: "Unlimited" }).click();
  await page.locator(".radio-row label", { hasText: "Either" }).click();
  await page.locator("#time-limit").fill("5");
  // Trimming does not apply to unlimited stages, so it leaves the summary.
  expect(await summaryOf(more)).toBe("More settings Untitled job · unlimited stages · first cuts either way · 5 s");
});

test("job setup: Cost shows with two or more panel sizes", async ({ page }) => {
  await loadReferenceDraft(page);
  const cost = page.getByRole("columnheader", { name: /^Cost/ });
  await expect(cost).toHaveCount(0);
  await page.getByRole("button", { name: "+ Add panel size" }).click();
  await expect(cost).toBeVisible();
  await page.locator("#panel-1-cost").fill("40");
  // A cost typed stays in sight even with one panel size left.
  await page.getByRole("button", { name: "Remove panel 1" }).click();
  await expect(cost).toBeVisible();
  await page.locator("#panel-0-cost").fill("");
  await expect(cost).toHaveCount(0);
});

test("cutting plan: figures, search, parts checklist and the panel's lists", async ({ page, request }) => {
  const id = await submitReference(request, 2);
  await waitDone(request, id);
  const job = await (await request.get(`/jobs/${id}`)).json();
  const m = job.best.metrics;
  const waste = `${(m.waste_fraction * 100).toFixed(1)}%`;
  await loadReferenceDraft(page);
  await page.goto(`/#/jobs/${id}`);
  await expect(page.locator(".headline")).toHaveText("All 21 parts fit");

  const figures = section(page, "Figures");
  expect(await summaryOf(figures)).toBe(`Figures 21 / 21 parts · ${m.panels_used} panels · ${waste} waste · 1 leftover to keep`);
  await open(figures);
  await expect(figures.locator(".stat").nth(0)).toContainText(`${m.panels_used}panels`);
  await expect(figures.locator(".stat").nth(1)).toContainText("21 / 21parts");
  await expect(figures.locator(".stat").nth(2)).toContainText(`${waste}waste`);
  await expect(figures.locator(".stat").nth(3)).toContainText("1leftover to keep");

  const search = section(page, "Search");
  expect(await summaryOf(search)).toBe("Search ✓ Complete plan · time limit reached after 2 s");
  await open(search);
  await expect(search.getByText(`This is plan ${job.best.number}, found after`)).toBeVisible();
  await expect(search.getByRole("button", { name: "Search for 60 s" })).toBeVisible();
  await expect(search.getByText("Kerf 3.2 mm · 3 cutting stages · first cuts along the length · last-stage trimming allowed.")).toBeVisible();

  const checklist = section(page, "Parts checklist");
  expect(await summaryOf(checklist)).toBe("Parts checklist all 21 parts cut");
  await open(checklist);
  await expect(checklist.getByRole("row")).toHaveCount(12);
  await expect(checklist.locator("tr.not-cut")).toHaveCount(0);

  // One panel: what is on it, its offcuts, and the cut order's help.
  await page.getByRole("link", { name: "Cut order →" }).first().click();
  await expect(page.getByText(/^Cut 1 of \d+$/)).toBeVisible();
  const onPanel = section(page, "On this panel");
  const parts = Number((await summaryOf(onPanel)).match(/^On this panel (\d+) parts?$/)![1]);
  await open(onPanel);
  const counts = await onPanel.locator(".manifest-row strong").allTextContents();
  expect(counts.reduce((sum, c) => sum + Number(c.replace("× ", "")), 0)).toBe(parts);

  const offcuts = section(page, "Offcuts");
  const [, count, largest] = (await summaryOf(offcuts)).match(/^Offcuts (\d+) · the largest (.+)$/)!;
  await open(offcuts);
  await expect(offcuts.locator(".manifest-row").first().locator("div")).toHaveText(largest!);
  await expect(offcuts.locator(".manifest-row")).toHaveCount(Math.min(5, Number(count)));

  const about = page.getByRole("button", { name: "About the cut order" });
  await expect(page.locator("#cut-order-help")).toBeHidden();
  await about.click();
  await expect(page.locator("#cut-order-help")).toHaveText(/^Each size is the piece to cut off/);
});

test("searching: the plans found sum up the list", async ({ page, request }) => {
  const id = await submitReference(request, 60);
  await loadReferenceDraft(page);
  await page.goto(`/#/jobs/${id}`);
  await expect(page.locator(".headline")).toHaveText(/so far$/, { timeout: 20_000 });
  const plans = section(page, "Plans found");
  expect(await summaryOf(plans)).toMatch(/^Plans found \d+ plans? · the latest after \d+\.\d\d s$/);
  await open(plans);
  // Read together, as a better plan may arrive at any time: the newest plan
  // listed is the one the summary counts up to.
  const [found, newest] = await plans.evaluate((d) => [
    d.querySelector("summary")!.textContent!.match(/(\d+) plans?/)![1],
    d.querySelector("li b")!.textContent,
  ]);
  expect(newest).toBe(`Plan ${found}`);
  await expect(page.getByRole("link", { name: "Open plan" })).toBeVisible();
  await request.delete(`/jobs/${id}`);
});
