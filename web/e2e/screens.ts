// The screens of the screen budget (web/DESIGN.md §10), set up with the
// mockups' job, and how a screen is measured: visible controls and lines of
// prose. Used by the budget test and by `npm run screens`.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, type APIRequestContext, type Browser, type Page } from "@playwright/test";

/** The mockups' job (web/DESIGN.md, *Data*): 21 parts in 11 sizes on 2440 × 1220 birch ply. */
const INSTANCE = JSON.parse(
  readFileSync(join(import.meta.dirname, "../../data/instances/generated__bin_packing_3_non_exact_kerf3_2.json"), "utf8"),
);

/** The mockups' names; part 9 is left unnamed, as in `editor.html`. */
const NAMES: Record<string, string> = {
  "Panel 1": "Birch ply 18 mm",
  "Part 1": "Drawer front",
  "Part 2": "Fixed shelf",
  "Part 3": "Top",
  "Part 4": "Side",
  "Part 5": "Door",
  "Part 6": "Cleat",
  "Part 7": "Kicker",
  "Part 8": "Shelf",
  "Part 9": "Part 9",
  "Part 10": "Rail",
  "Part 11": "Stretcher",
};

/** The job as sent: the mockups' instance with the readable names. */
export function referenceInstance() {
  const instance = structuredClone(INSTANCE);
  for (const item of [...instance.parts, ...instance.panels]) item.id = NAMES[item.id] ?? item.id;
  return instance;
}

/** The same job as the editor's draft (`src/lib/job/draft.ts`), as stored in the browser. */
function referenceDraft(timeLimit: number) {
  const instance = referenceInstance();
  let key = 1;
  return {
    name: "Kitchen units",
    objective: instance.objective,
    kerf: String(instance.saw.kerf),
    stages: instance.saw.stages,
    cutType: instance.saw.cut_type,
    firstCuts: instance.saw.first_cuts,
    timeLimit: String(timeLimit),
    parts: instance.parts.map((p: { id: string; length: number; width: number; quantity: number; grain?: boolean }) => ({
      key: key++,
      id: p.id === "Part 9" ? "" : p.id,
      length: String(p.length),
      width: String(p.width),
      quantity: String(p.quantity),
      grain: p.grain ?? false,
      value: "",
    })),
    panels: instance.panels.map((p: { id: string; length: number; width: number }) => ({
      key: key++,
      id: p.id,
      length: String(p.length),
      width: String(p.width),
      quantity: "",
      cost: "",
    })),
  };
}

/** Puts the mockups' job in the editor. */
export async function loadReferenceDraft(page: Page, timeLimit = 2) {
  await page.goto("/#/");
  await page.evaluate((draft) => {
    localStorage.clear();
    localStorage.setItem("sheetsolver.draft.v1", JSON.stringify(draft));
  }, referenceDraft(timeLimit));
  await page.reload();
  await expect(page.locator("#part-0-id")).toHaveValue("Drawer front");
}

/** Sends the mockups' job and returns its id. */
export async function submitReference(request: APIRequestContext, timeLimit: number): Promise<string> {
  const response = await request.post("/jobs", { data: { name: "Kitchen units", time_limit: timeLimit, instance: referenceInstance() } });
  expect(response.status()).toBe(201);
  return (await response.json()).id;
}

/** Waits until the job's search has ended. */
export async function waitDone(request: APIRequestContext, id: string) {
  await expect.poll(async () => (await (await request.get(`/jobs/${id}`)).json()).state, { timeout: 30_000 }).toBe("done");
}

/** What one screen shows, as counted for the budget. */
export interface Measure {
  /** Buttons, links, inputs, selects and disclosures on screen, outside the parts and panels tables. */
  controls: number;
  /** Inputs and buttons in the rows of the parts and panels tables. */
  tableControls: number;
  /** Lines of explanatory text: paragraphs, help, hints, notes. */
  prose: number;
  /** Every line of text on screen, prose or not, outside the drawings: what there is to read. */
  text: number;
}

/**
 * Text that explains rather than shows a value: paragraphs, help boxes and
 * popovers, hints and notes. Figures, tables, lists of cuts or parts,
 * headings and the labels of controls are not prose.
 */
export const PROSE = "p, .help-box, .pop, .hint, .table-foot small, .stale-bar > div:first-child, .state li";

/** Counts what is visible on the whole page (scrolled through), print sheets excluded. */
export async function measure(page: Page): Promise<Measure> {
  return page.evaluate((proseSelector) => {
    const shown = (el: Element) =>
      el.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true }) &&
      el.getClientRects().length > 0 &&
      !el.closest(".print-sheets, .visually-hidden, [aria-hidden='true']");
    const controls = [...document.querySelectorAll("button, a[href], input, select, textarea, summary")].filter(shown);
    const inTable = (el: Element) => el.closest("table") !== null && el.closest("tbody") !== null && el.closest("form") !== null;
    let prose = 0;
    const blocks = [...document.querySelectorAll(proseSelector)].filter(shown);
    // Nested prose (a paragraph in a help box) counts once, with its outermost block.
    for (const block of blocks.filter((b) => !blocks.some((o) => o !== b && o.contains(b)))) {
      const tops = new Set<number>();
      const walker = document.createTreeWalker(block, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        if (!node.textContent?.trim() || !shown(node.parentElement!)) continue;
        const range = document.createRange();
        range.selectNodeContents(node);
        for (const rect of range.getClientRects()) if (rect.width > 0) tops.add(Math.round(rect.top + window.scrollY));
      }
      // Line boxes of one block whose tops differ by a pixel or two are one line.
      const sorted = [...tops].sort((a, b) => a - b);
      prose += sorted.filter((top, i) => i === 0 || top - sorted[i - 1]! > 4).length;
    }
    // Every line of text: one per block and line, however many text nodes it holds.
    const lines = new Set<string>();
    const blockOf = (el: Element) => {
      let at: Element | null = el;
      while (at && getComputedStyle(at).display.startsWith("inline")) at = at.parentElement;
      return at ?? el;
    };
    const ids = new Map<Element, number>();
    const all = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let node = all.nextNode(); node; node = all.nextNode()) {
      const parent = node.parentElement;
      if (!node.textContent?.trim() || !parent || parent.closest("svg, script, style") || !shown(parent)) continue;
      const block = blockOf(parent);
      if (!ids.has(block)) ids.set(block, ids.size);
      const range = document.createRange();
      range.selectNodeContents(node);
      for (const rect of range.getClientRects()) {
        if (rect.width > 0) lines.add(`${ids.get(block)}:${Math.round((rect.top + window.scrollY) / 6)}`);
      }
    }
    return {
      text: lines.size,
      controls: controls.filter((c) => !inTable(c)).length,
      tableControls: controls.filter(inTable).length,
      prose,
    };
  }, PROSE);
}

/** The widths of the budget: desk, tablet, phone. */
export const WIDTHS = [
  { width: 1440, height: 900, touch: false },
  { width: 820, height: 1180, touch: true },
  { width: 360, height: 800, touch: true },
] as const;

/** The screens of the budget, in order. */
export const SCREENS = ["setup-new", "setup", "plan", "panel", "searching"] as const;
export type Screen = (typeof SCREENS)[number];

/**
 * Opens every screen at every width with the mockups' job, and calls `at`
 * on each, once it has settled.
 */
export async function eachScreen(
  browser: Browser,
  request: APIRequestContext,
  baseURL: string,
  at: (page: Page, screen: Screen, width: (typeof WIDTHS)[number]) => Promise<void>,
) {
  const done = await submitReference(request, 2);
  await waitDone(request, done);
  for (const w of WIDTHS) {
    const context = await browser.newContext({ baseURL, viewport: { width: w.width, height: w.height }, hasTouch: w.touch, deviceScaleFactor: 1, colorScheme: "light" });
    const page = await context.newPage();
    const settle = () => page.waitForTimeout(400);

    await page.goto("/#/");
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await expect(page.locator("#part-0-id")).toBeVisible();
    await settle();
    await at(page, "setup-new", w);

    await loadReferenceDraft(page);
    await settle();
    await at(page, "setup", w);

    await page.goto(`/#/jobs/${done}`);
    await page.locator(".headline").waitFor();
    await settle();
    await at(page, "plan", w);

    await page.goto(`/#/jobs/${done}/panels/1?cut=7`);
    await page.getByText(/^Cut 7 of \d+$/).waitFor();
    await settle();
    await at(page, "panel", w);

    const running = await submitReference(request, 60);
    await page.goto(`/#/jobs/${running}`);
    await page.locator(".headline", { hasText: "so far" }).waitFor({ timeout: 20_000 });
    await page.waitForTimeout(1500);
    await at(page, "searching", w);
    await request.delete(`/jobs/${running}`);

    await context.close();
  }
}

/** The budget table of web/DESIGN.md §10: the most each screen may show, per width. */
export function budget(): Map<string, Omit<Measure, "tableControls">> {
  const md = readFileSync(join(import.meta.dirname, "../DESIGN.md"), "utf8");
  const table = md.slice(md.indexOf("<!-- budget -->"), md.indexOf("<!-- /budget -->"));
  const out = new Map<string, Omit<Measure, "tableControls">>();
  for (const line of table.split("\n")) {
    const cells = line.split("|").map((c) => c.trim()).filter(Boolean);
    if (cells.length !== 5 || !/^\d+$/.test(cells[1]!)) continue;
    const [screen, width, controls, prose, text] = cells;
    out.set(`${screen!.replace(/`/g, "")}.${width}`, { controls: Number(controls), prose: Number(prose), text: Number(text) });
  }
  return out;
}
