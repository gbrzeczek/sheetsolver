// The summary line of a panel's trims (DESIGN.md §10): its values in a few
// words, so the closed editor still says what is in it.
import { optionTitle } from "../api/fields";
import { mm } from "../plan/tree";
import type { TrimCut } from "../api/types";
import { EDGES, type Edge, type TrimRow } from "./draft";

/** The width typed for an edge, in mm, or 0 for none or not a number yet. */
function width(row: TrimRow): number {
  const value = Number(row.width.trim().replace(",", "."));
  return Number.isFinite(value) && value > 0 ? value : 0;
}

/**
 * "none", "10 mm all round, cut off first", or each trimmed edge with its
 * width, then how the strips are cut: "left 10 · bottom 5 mm · cut off
 * first", "left 10 · right 10 mm · right left on the offcut".
 */
export function trimsSummary(trims: Record<Edge, TrimRow>): string {
  const trimmed = EDGES.filter((edge) => width(trims[edge]) > 0);
  if (trimmed.length === 0) return "none";
  const cuts = new Set(trimmed.map((edge) => trims[edge].cut));
  const cutName = (cut: TrimCut) => optionTitle("Trim.cut", cut).toLowerCase();
  const how =
    cuts.size === 1
      ? cutName([...cuts][0]!)
      : trimmed
          .filter((edge) => trims[edge].cut === "left_on_offcut")
          .join(", ")
          .concat(" ", cutName("left_on_offcut"));
  const widths = new Set(trimmed.map((edge) => width(trims[edge])));
  if (trimmed.length === 4 && widths.size === 1) return `${mm([...widths][0]!)} mm all round, ${how}`;
  return `${trimmed.map((edge) => `${edge} ${mm(width(trims[edge]))}`).join(" · ")} mm · ${how}`;
}
