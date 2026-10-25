import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import type { Instance, Pattern, PlanNode } from "../api/types";
import { cutDirection, cutOrder } from "./cutorder";
import { fitLabel } from "./labels";
import { children, nodes, patternStats } from "./tree";

const DATA = join(__dirname, "../../../../data");
const GOLDEN = join(DATA, "golden/9b656a0d");

function instance(name: string): Instance {
  return JSON.parse(readFileSync(join(DATA, "instances", `${name}.json`), "utf8")) as Instance;
}

function golden(file: string): { patterns: Pattern[] | null; upstream_metrics?: { waste_percentage: number } } {
  return JSON.parse(readFileSync(join(GOLDEN, file), "utf8"));
}

const KERF3 = "generated__bin_packing_3_non_exact_kerf3_2";

describe("cut order", () => {
  it("lists the cuts of the mockup's panel 1 as a woodworker makes them", () => {
    const pattern = golden(`${KERF3}.q2048.json`).patterns![0]!;
    const { cuts, names } = cutOrder(pattern, 3.2, 3);
    expect(cuts).toHaveLength(19);
    expect(cuts.slice(0, 9).map((c) => c.measure)).toEqual([643.2, 420, 120, 320.5, 1040.5, 1040.5, 320, 630, 630]);
    expect(cuts.slice(0, 3).map((c) => c.direction)).toEqual(["along", "along", "along"]);
    expect(cuts[3]!.direction).toBe("across");
    expect(cuts.slice(0, 3).map((c) => names.get(c.produces))).toEqual(["Strip A", "Strip B", "Strip C"]);
    expect(cuts[3]!.group).toBe("Strip A");
    expect(names.get(cuts[3]!.produces)).toBe("Piece A1");
    // A cut yielding a part through a wrapper piece names the part.
    expect(cuts[13]!.produces).toMatchObject({ kind: "part", part_id: "Part 11" });
    // The last full-panel cut leaves the 27.2 mm offcut along the top edge.
    expect(cuts[2]!.leaves?.kind).toBe("offcut");
    expect(cuts[2]!.leaves?.width).toBe(27.2);
  });

  it("skips wrapper pieces and names a sub-kerf remainder saw dust", () => {
    // Panel 2 of the q64 plan ends with a 0.4 mm offcut, thinner than the kerf.
    const pattern = golden(`${KERF3}.q64.json`).patterns![1]!;
    const { cuts } = cutOrder(pattern, 3.2, 3);
    expect(cuts[0]!.leavesSawDust).toBe(true);
    expect(cuts.every((c) => c.measure > 0)).toBe(true);
  });

  it("marks the last-stage trimming cut of a 3-stage plan", () => {
    const pattern = golden(`${KERF3}.q2048.json`).patterns![2]!;
    const { cuts } = cutOrder(pattern, 3.2, 3);
    const trims = cuts.filter((c) => c.trim);
    expect(trims).toHaveLength(1);
    expect(trims[0]!.produces.kind).toBe("part");
  });

  // Every golden plan, including unlimited stages where directions mix:
  // the inferred direction must be the one the tree rules allow (children
  // side by side across the cut direction, each spanning its parent).
  const files = readdirSync(GOLDEN).filter((f) => f.endsWith(".json"));
  it(`infers a consistent cut direction in all ${files.length} golden plans`, () => {
    let pieces = 0;
    for (const file of files) {
      for (const pattern of golden(file).patterns ?? []) {
        for (const piece of nodes(pattern.root)) {
          const kids = children(piece);
          if (kids.length < 2) continue;
          pieces++;
          const direction = cutDirection(piece);
          // Side by side along the cut axis, in order, aligned on the other
          // axis. (Open-dimension roots: children may stop short of the
          // panel's open side, so extents are not compared.)
          kids.forEach((kid, i) => {
            if (direction === "across") expect(kid.y, file).toBe(piece.y);
            else expect(kid.x, file).toBe(piece.x);
            const prev = kids[i - 1];
            if (prev) expect(direction === "across" ? kid.x > prev.x : kid.y > prev.y, file).toBe(true);
          });
        }
      }
    }
    expect(pieces).toBeGreaterThan(1000);
  });

  it("produces, for every golden plan, one cut per child but the last of each real piece", () => {
    for (const file of files) {
      const name = file.split(".")[0]!; // e.g. ….maximal_spaces.q64.json
      const saw = instance(name).saw;
      for (const pattern of golden(file).patterns ?? []) {
        const { cuts } = cutOrder(pattern, saw.kerf, saw.stages);
        let expected = 0;
        for (const piece of nodes(pattern.root)) {
          const kids = children(piece);
          if (kids.length > 1) expected += kids.length - 1;
          else if (kids.length === 1 && (kids[0]!.length !== piece.length || kids[0]!.width !== piece.width)) expected += 1;
        }
        expect(cuts.length, file).toBe(expected);
        cuts.forEach((cut, i) => expect(cut.number).toBe(i + 1));
      }
    }
  });
});

describe("figures per panel", () => {
  it("add up to upstream's waste for every golden plan", () => {
    let checked = 0;
    for (const file of readdirSync(GOLDEN).filter((f) => f.endsWith(".json"))) {
      const g = golden(file);
      if (!g.patterns?.length || !g.upstream_metrics || !/bin_packing|knapsack/.test(file)) continue;
      let waste = 0;
      let usable = 0;
      for (const pattern of g.patterns) {
        const s = patternStats(pattern);
        waste += s.wasteArea * pattern.repeat;
        usable += (s.panelArea - s.leftoverArea) * pattern.repeat;
      }
      expect(waste / usable, file).toBeCloseTo(g.upstream_metrics.waste_percentage, 9);
      checked++;
    }
    expect(checked).toBeGreaterThan(50);
  });
});

describe("label fitting", () => {
  it("prefers name and size, then turned, then size only, then nothing", () => {
    expect(fitLabel(200, 60, "Side", "1040.5 × 630", 12, 11)).toEqual({ mode: "stacked", rotate: false });
    expect(fitLabel(30, 200, "Side", "1040.5 × 630", 12, 11)).toEqual({ mode: "stacked", rotate: true });
    expect(fitLabel(90, 16, "Kicker", "260 × 90", 12, 11)).toEqual({ mode: "single", rotate: false });
    expect(fitLabel(4, 4, "Kicker", "260 × 90", 12, 11)).toEqual({ mode: "none" });
  });
});

export type { PlanNode };
