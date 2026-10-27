// Stage 7 exit criterion: no input without help text; wording matches FIELDS.md.
import { expect, test } from "@playwright/test";
import { enterJob, fieldsMd } from "./helpers";

// Fields whose label says it all have no (?) button (web/DESIGN.md §5).
const SELF_EXPLANATORY = ["JobRequest.name", "Part.id", "Panel.id", "Part.width", "Panel.quantity"];

test("every input has a help button with the short help first and More for the full text", async ({ page }) => {
  const help = fieldsMd();
  await page.goto("/#/");
  // Show every field: the Value column needs the goal "Best use of the panels
  // I have", the Cost column two panel sizes, the rest the collapsed settings
  // and the first panel's trims.
  await page.locator("#objective").selectOption("knapsack");
  await page.getByRole("button", { name: "+ Add panel size" }).click();
  await page.locator("summary", { hasText: "More settings" }).click();
  await page.locator("details.trims > summary").first().click();

  const text = (el: Element) => el.textContent!.replace(/\s+/g, " ").trim();
  const buttons = page.locator("button.help-btn");
  const count = await buttons.count();
  const seen = new Set<string>();
  for (let i = 0; i < count; i++) {
    const button = buttons.nth(i);
    const id = (await button.getAttribute("aria-controls"))!;
    if (!id.startsWith("help-")) continue; // not a field's help (the cut order's)
    if (/-\d+$/.test(id)) continue; // the same field for another panel (its trims)
    const key = id.replace(/^help-/, "").replace("-", ".");
    const expected = help.get(key);
    expect(expected, `FIELDS.md has help for ${key}`).toBeDefined();
    expect(await button.getAttribute("aria-label")).toBe(`About ${expected!.label}`);
    await button.click();
    const target = page.locator(`#${id}`);
    await expect(target).toBeVisible();
    // First the short help, then More gives the full text in its place.
    await expect(page.locator(`#${id}-short`)).toBeVisible();
    expect(await page.locator(`#${id}-short`).evaluate(text), `${key} short`).toBe(expected!.short);
    await expect(page.locator(`#${id}-full`)).toBeHidden();
    await target.getByRole("button", { name: `More about ${expected!.label}` }).click();
    await expect(page.locator(`#${id}-full`)).toBeVisible();
    await expect(page.locator(`#${id}-short`)).toBeHidden();
    expect(await page.locator(`#${id}-full`).evaluate(text), key).toBe(expected!.help);
    await button.click({ force: true });
    await expect(target).toBeHidden();
    seen.add(key);
  }
  // Every field of FIELDS.md but the self-explanatory ones has its help button on the page.
  expect([...help.keys()].filter((k) => !seen.has(k)).sort()).toEqual([...SELF_EXPLANATORY].sort());
  expect([...seen].filter((k) => SELF_EXPLANATORY.includes(k))).toEqual([]);

  // And every input points at its help: the short help, or for the
  // self-explanatory ones the full text.
  for (const input of await page.locator("main input, form input, select").all()) {
    const described = await input.getAttribute("aria-describedby");
    expect(described, await input.evaluate((e) => e.outerHTML)).toBeTruthy();
    expect(await page.locator(`#${described}`).count(), described!).toBe(1);
  }
  await expect(page.locator("#kerf")).toHaveAttribute("aria-describedby", "help-Saw-kerf-short");
  for (const key of SELF_EXPLANATORY) {
    expect(await page.locator(`#help-${key.replace(".", "-")}`).evaluate(text), key).toBe(help.get(key)!.help);
  }
});

test("problems are shown next to their field, and clear when it is edited", async ({ page }) => {
  await enterJob(page, [{ name: "Side", length: "720", width: "560.25" }, { name: "Side", length: "100", width: "100", quantity: "0" }], { length: "2440", width: "1220" });
  await page.getByRole("button", { name: /Find cutting plan/ }).last().click();
  const alert = page.getByRole("alert").first();
  await expect(alert).toContainText("The job cannot be solved as entered.");
  await expect(alert).toContainText("Row 1, Width: length 560.25 mm has more than one decimal");
  await expect(alert).toContainText('Row 2, Name: part name "Side" is used more than once');
  await expect(alert).toContainText("Row 2, Quantity: quantity must be at least 1");
  await expect(page.locator("#part-0-width")).toHaveAttribute("aria-invalid", "true");
  await page.locator("#part-0-width").fill("560.2");
  await expect(page.locator("#part-0-width")).not.toHaveAttribute("aria-invalid", "true");
  await expect(alert).not.toContainText("Row 1, Width");
});

test("problems the server finds map to the same fields (422)", async ({ page }) => {
  await enterJob(page, [{ name: "Side", length: "720", width: "560" }], { length: "2440", width: "1220" });
  // A problem only the server could know: replay a real 422 body shape.
  await page.route("**/jobs", (route) =>
    route.fulfill({
      status: 422,
      contentType: "application/json",
      body: JSON.stringify({
        error: "invalid_instance",
        message: "The job cannot be solved as entered.",
        problems: [{ field: "instance.panels[0].width", message: "width of panel \"Panel 1\" must be at least 0.1 mm" }],
      }),
    }),
  );
  await page.getByRole("button", { name: /Find cutting plan/ }).last().click();
  await expect(page.getByRole("alert").first()).toContainText("Row 1, Width: width of panel");
  await expect(page.locator("#panel-0-width")).toHaveAttribute("aria-invalid", "true");
});

test("New job starts over, with Undo", async ({ page }) => {
  await enterJob(page, [{ name: "Side", length: "720", width: "560" }], { length: "2440", width: "1220" });
  await page.getByRole("button", { name: "New job" }).click();
  await expect(page.locator("#part-0-id")).toHaveValue("");
  await expect(page.locator("#part-0-length")).toHaveValue("");
  await page.getByRole("status").getByRole("button", { name: "Undo" }).click();
  await expect(page.locator("#part-0-id")).toHaveValue("Side");
});

test("the draft survives a reload", async ({ page }) => {
  await enterJob(page, [{ name: "Side", length: "720", width: "560" }], { length: "2440", width: "1220" });
  await page.locator("#job-name").fill("Wardrobe");
  await page.reload();
  await expect(page.locator("#job-name")).toHaveValue("Wardrobe");
  await expect(page.locator("#part-0-length")).toHaveValue("720");
});

test("rows can be added to a job brought back from an earlier visit", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await enterJob(page, [{ name: "Side", length: "720", width: "560" }, { name: "Shelf", length: "500", width: "300" }], { length: "2440", width: "1220" });
  await page.reload();
  await expect(page.locator("#part-1-id")).toHaveValue("Shelf");
  await page.getByRole("button", { name: "+ Add part" }).click();
  await page.getByRole("button", { name: "+ Add panel size" }).click();
  await expect(page.locator("#part-2-length")).toBeVisible();
  await expect(page.locator("#panel-1-length")).toBeVisible();
  expect(errors).toEqual([]);
});
