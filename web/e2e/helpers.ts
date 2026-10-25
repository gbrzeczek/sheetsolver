import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, type Page } from "@playwright/test";

export interface PartInput {
  name?: string;
  length: string;
  width: string;
  quantity?: string;
  grain?: boolean;
}

/** Fills the job setup with parts and one panel, starting from a new job. */
export async function enterJob(page: Page, parts: PartInput[], panel: { name?: string; length: string; width: string }, timeLimit = "2") {
  await page.goto("/#/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  for (const [i, part] of parts.entries()) {
    if (i > 0) await page.getByRole("button", { name: "+ Add part" }).click();
    if (part.name) await page.locator(`#part-${i}-id`).fill(part.name);
    await page.locator(`#part-${i}-length`).fill(part.length);
    await page.locator(`#part-${i}-width`).fill(part.width);
    if (part.quantity) await page.locator(`#part-${i}-quantity`).fill(part.quantity);
    if (part.grain) await page.getByLabel(`Keep grain direction, row ${i + 1}`).check();
  }
  if (panel.name) await page.locator("#panel-0-id").fill(panel.name);
  await page.locator("#panel-0-length").fill(panel.length);
  await page.locator("#panel-0-width").fill(panel.width);
  await page.locator("summary", { hasText: "More cutting settings" }).click();
  await page.locator("#time-limit").fill(timeLimit);
}

export async function findPlan(page: Page) {
  await page.getByRole("button", { name: /Find cutting plan/ }).last().click();
  await expect(page).toHaveURL(/#\/jobs\/[0-9a-f-]+/);
}

/**
 * The help texts of docs/FIELDS.md, markdown removed, keyed like the UI's
 * help elements: "Part.id", "Saw.kerf", "JobRequest.time_limit", "Ui.job_name".
 */
export function fieldsMd(): Map<string, { label: string; help: string }> {
  const md = readFileSync(join(import.meta.dirname, "../../docs/FIELDS.md"), "utf8");
  const help = new Map<string, { label: string; help: string }>();
  const schema: Record<string, string> = { "Panels (stock sheets)": "Panel", Parts: "Part", "Saw and cutting": "Saw" };
  let chapter = "";
  let section = "";
  for (const line of md.split("\n")) {
    if (line.startsWith("## ")) chapter = line.slice(3).trim();
    else if (line.startsWith("### ")) section = line.slice(4).trim();
    else if ((chapter === "Fields" || chapter === "UI-only fields") && line.startsWith("| `")) {
      const cells = line.split(" | ");
      const name = cells[0]!.replace(/[|` ]/g, "");
      const owner = chapter === "UI-only fields" ? "Ui" : section === "Job" ? (name === "time_limit" ? "JobRequest" : "Instance") : schema[section]!;
      help.set(`${owner}.${name}`, {
        label: cells[1]!,
        help: cells[2]!.replace(/\*\*(.+?)\*\*/g, "$1").replace(/\*(.+?)\*/g, "$1"),
      });
    }
  }
  return help;
}
