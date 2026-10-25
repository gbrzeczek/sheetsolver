import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { API_FIELDS } from "./fields.generated";
import { helpHtml, optionTitle, UI_FIELDS } from "./fields";

const FIELDS_MD = readFileSync(join(__dirname, "../../../../docs/FIELDS.md"), "utf8");

describe("field help", () => {
  it("has a label and help text for every input the API describes", () => {
    expect(Object.keys(API_FIELDS).length).toBeGreaterThanOrEqual(17);
    for (const [key, f] of Object.entries(API_FIELDS)) {
      expect(f.label, key).not.toBe("");
      expect(f.help.length, key).toBeGreaterThan(20);
    }
  });

  it("copies the UI-only fields exactly from FIELDS.md", () => {
    const chapter = FIELDS_MD.split("## UI-only fields")[1]!.split("\n## ")[0]!;
    for (const [key, f] of Object.entries(UI_FIELDS)) {
      const name = key.split(".")[1]!;
      const row = chapter.split("\n").find((line) => line.startsWith(`| \`${name}\``));
      expect(row, key).toBeDefined();
      const cells = row!.split(" | ");
      expect(cells[1], key).toBe(f.label);
      expect(cells[2], key).toBe(f.help);
    }
  });

  it("names options as FIELDS.md does", () => {
    expect(optionTitle("Instance.objective", "knapsack")).toBe("Best use of the panels I have");
    expect(optionTitle("Saw.stages", "unlimited")).toBe("Unlimited");
  });

  it("renders bold and italic only, and emphasises the current option", () => {
    expect(helpHtml("A **B**: c. **D**: e <x>.", "B")).toBe(
      'A <span class="current"><strong>B</strong>: c. </span><strong>D</strong>: e &#60;x&#62;.',
    );
    expect(helpHtml("see *Value*")).toBe("see <em>Value</em>");
  });
});
