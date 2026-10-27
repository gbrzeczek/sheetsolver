// Stage 9 exit criteria: the trims and the minimum offcut width are entered,
// solved and honoured in the plan, and trims cut off first and trims left on
// the offcut give different plans under a minimum offcut width.
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { enterJob, findPlan } from "./helpers";

const PARTS = [
  { name: "Side", length: "720", width: "560", quantity: "4", grain: true },
  { name: "Shelf", length: "1180", width: "300.5", quantity: "4" },
  { name: "Door", length: "700", width: "396", quantity: "4" },
];

/** Opens the first panel's trims, if they are not open already (the desk keeps the job beside the plan). */
async function openTrims(page: Page) {
  const trims = page.locator("details.trims").first();
  if ((await trims.getAttribute("open")) === null) await trims.locator("summary").click();
  await expect(trims).toHaveAttribute("open", "");
}

type Node = { kind: string; x: number; y: number; length: number; width: number; stage: number; children?: Node[] };

/** Enters the job with 10 mm trims on every edge, each `cut`, and a 30 mm minimum offcut width. */
async function enterTrimmedJob(page: Page, cut: "Cut off first" | "Left on the offcut") {
  await enterJob(page, PARTS, { name: "Chipboard", length: "2500", width: "1250" });
  await page.locator("#kerf").fill("3.2");
  await page.locator("#minimum-offcut-width").fill("30");
  await expect(page.locator("summary", { hasText: "More settings" })).toContainText("offcuts at least 30 mm");

  const trims = page.locator("details.trims").first();
  await expect(trims.locator("summary")).toHaveText(/Trims\s*none/);
  await openTrims(page);
  for (const edge of ["left", "right", "bottom", "top"]) {
    await page.locator(`#panel-0-trim-${edge}`).fill("10");
    await page.locator(`#panel-0-trim-${edge}-cut`).selectOption({ label: cut });
  }
  await expect(trims.locator("summary")).toHaveText(new RegExp(`Trims\\s*10 mm all round, ${cut.toLowerCase()}`));
}

async function solved(page: Page, request: APIRequestContext) {
  await findPlan(page);
  await expect(page.locator(".headline")).toHaveText("All 12 parts fit", { timeout: 20_000 });
  const id = page.url().split("/jobs/")[1]!.split(/[/?]/)[0]!;
  const job = await (await request.get(`/jobs/${id}`)).json();
  expect(job.state).toBe("done");
  return job;
}

/** Every offcut and leftover, with the size across the cut that frees it. */
function offcutWidths(node: Node, across: "x" | "y", out: number[] = []): number[] {
  for (const child of node.children ?? []) {
    if (child.kind === "offcut" || child.kind === "leftover") out.push(across === "x" ? child.length : child.width);
    if (child.kind === "piece" && child.stage > 0) offcutWidths(child, across === "x" ? "y" : "x", out);
  }
  return out;
}

test("trims cut off first are cut before anything else, and offcuts keep the minimum width", async ({ page, request }) => {
  await enterTrimmedJob(page, "Cut off first");
  const job = await solved(page, request);
  expect(job.instance.panels[0].trims.left).toEqual({ width: 10, cut: "cut_off_first" });
  expect(job.instance.saw.minimum_offcut_width).toBe(30);

  // Every panel is cut into its four trims, then the trimmed panel.
  for (const pattern of job.best.plan.patterns) {
    const kids: Node[] = pattern.root.children;
    expect(kids.map((k) => k.kind)).toEqual(["trim", "trim", "trim", "trim", "piece"]);
    expect(kids[4]).toMatchObject({ x: 10, y: 10, length: 2480, width: 1230, stage: 0 });
    const first = pattern.first_cuts === "across_length" ? "x" : "y";
    for (const width of offcutWidths(kids[4]!, first)) expect(width).toBeGreaterThanOrEqual(30);
  }

  // The drawing shows the trims, the legend names them, and the cut order
  // starts with them.
  await page.getByRole("link", { name: "Cut order →" }).first().click();
  await expect(page.locator(".legend")).toContainText("Trim, no part there");
  await expect(page.locator("svg.plan-svg .p-trim").first()).toBeVisible();
  const cuts = page.locator(".cutlist li.cut");
  await expect(page.locator(".cutlist li.group").first()).toHaveText(/Trims\s*cut off first/);
  // One group for the four trims, then the panel's strips.
  await expect(page.locator(".cutlist li.group").nth(1)).toHaveText(/Panel into strips/);
  await expect(cuts.nth(0)).toContainText("10 mm");
  await expect(cuts.nth(0)).toContainText("left end trim");
  await expect(cuts.nth(3)).toContainText("top edge trim");
  await expect(page.locator(".now")).toContainText("left end trim");
});

test("trims left on the offcut give a different plan under a minimum offcut width", async ({ page, request }) => {
  await enterTrimmedJob(page, "Cut off first");
  const hard = await solved(page, request);

  // Back to the job, kept in the browser, to change how the trims are cut.
  await page.goto("/#/");
  await openTrims(page);
  for (const edge of ["left", "right", "bottom", "top"]) {
    await page.locator(`#panel-0-trim-${edge}-cut`).selectOption({ label: "Left on the offcut" });
  }
  const soft = await solved(page, request);
  expect(soft.instance.panels[0].trims.right).toEqual({ width: 10, cut: "left_on_offcut" });

  // No trim is cut off on its own: the left one is the first offcut, widened
  // to the minimum offcut width, and the parts stay out of every trim.
  const pattern = soft.best.plan.patterns[0];
  const kids: Node[] = pattern.root.children;
  expect(kids.some((k) => k.kind === "trim")).toBe(false);
  const across = pattern.first_cuts === "across_length";
  expect(kids[0]).toMatchObject({ kind: "offcut", [across ? "length" : "width"]: 30 });
  expect(JSON.stringify(soft.best.plan)).not.toBe(JSON.stringify(hard.best.plan));
  await expect(page.locator("svg.plan-svg .p-trim-soft").first()).toBeVisible();
});
