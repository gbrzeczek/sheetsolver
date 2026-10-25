import { describe, expect, it } from "vitest";
import type { Instance, Metrics } from "../api/types";
import { headline, oversizedParts, panelsLine, totalParts } from "./summary";

const instance: Instance = {
  objective: "bin_packing",
  saw: { kerf: 3.2, stages: 3, cut_type: "non_exact", first_cuts: "either" },
  panels: [{ id: "Birch", length: 2440, width: 1220 }],
  parts: [
    { id: "Side", length: 720, width: 560, quantity: 4, grain: true },
    { id: "Worktop", length: 2600, width: 600, quantity: 1, grain: false },
    { id: "Tall", length: 1200, width: 2000, quantity: 1, grain: false },
  ],
};
const metrics = { parts_cut: 6, used_length: 1850, used_width: 900 } as Metrics;

describe("plan summary", () => {
  it("answers whether everything fits", () => {
    expect(totalParts(instance)).toBe(6);
    expect(headline(instance, metrics)).toBe("All 6 parts fit");
    expect(headline(instance, { ...metrics, parts_cut: 4 })).toBe("4 of 6 parts fit");
    expect(headline({ ...instance, objective: "open_dimension_x" }, metrics)).toBe("Uses 1850 mm of 2440 mm");
  });

  it("finds parts that fit on no panel, turning those without grain", () => {
    expect(oversizedParts(instance).map((p) => p.id)).toEqual(["Worktop"]);
  });

  it("names the panels, or gives their size when the name was generated", () => {
    const plan = { patterns: [{ panel_id: "Birch", repeat: 3, first_cuts: "along_length", root: {} }] } as never;
    expect(panelsLine(instance, plan)).toBe("3 panels of Birch, 2440 × 1220 mm");
    const unnamed = { ...instance, panels: [{ id: "Panel 1", length: 2440, width: 1220 }] };
    const plan2 = { patterns: [{ panel_id: "Panel 1", repeat: 1, first_cuts: "along_length", root: {} }] } as never;
    expect(panelsLine(unnamed, plan2)).toBe("1 panel, 2440 × 1220 mm");
  });
});
