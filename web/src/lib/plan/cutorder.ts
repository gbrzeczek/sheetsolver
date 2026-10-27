// The cut order a woodworker follows at the saw, derived from the plan tree
// (web/DESIGN.md §2, "How the cut order is derived").
import type { Pattern, PlanNode } from "../api/types";
import { children, cuttingRoot, trimEdge } from "./tree";

/** Which way a cut runs: along the panel's length (at a fixed y) or across it (at a fixed x). */
export type Direction = "along" | "across";

export interface Cut {
  /** 1-based position in the order. */
  number: number;
  /** The piece being cut. */
  piece: PlanNode;
  direction: Direction;
  /** The size to cut off, in mm: what the fence or stop is set to. */
  measure: number;
  /** The piece, part or offcut this cut produces. */
  produces: PlanNode;
  /** What is left after the last cut of a piece, if this is that cut. */
  leaves: PlanNode | null;
  /** The remainder is thinner than the kerf: saw dust, not an offcut. */
  leavesSawDust: boolean;
  /** "panel" for the full-panel cuts, else the name of the strip they belong to. */
  group: string;
  /** The last-stage trimming cut (stage `stages + 1`, docs/ARCHITECTURE.md). */
  trim: boolean;
  /** For a trim cut off first, before every other cut: the panel edge it trims. */
  edge?: "left" | "right" | "bottom" | "top";
}

/**
 * The direction of the cuts that split `piece` into its children. Not in the
 * plan (§8, #13 deferred), so inferred from geometry: children side by side
 * along x are cut across the length, otherwise along it. With one child
 * (a trim), the cut runs across when the child is shorter in length.
 */
export function cutDirection(piece: PlanNode): Direction {
  const kids = children(piece);
  const [a, b] = kids;
  if (a && b) return a.y === b.y && a.x !== b.x ? "across" : "along";
  if (a) return a.length < piece.length ? "across" : "along";
  return "along";
}

function sameSize(a: PlanNode, b: PlanNode): boolean {
  return a.length === b.length && a.width === b.width;
}

/** A piece the single child fills exactly: a wrapper, no cut needed. */
function isWrapper(piece: PlanNode): boolean {
  const kids = children(piece);
  return kids.length === 1 && sameSize(kids[0]!, piece);
}

/** What a node really is: through wrapper pieces down to the part or piece inside. */
export function unwrap(node: PlanNode): PlanNode {
  return isWrapper(node) ? unwrap(children(node)[0]!) : node;
}

export interface CutOrder {
  cuts: Cut[];
  /** Names of the intermediate pieces: Strip A, Piece A1, Piece A1.1. */
  names: Map<PlanNode, string>;
}

/**
 * The cuts of one pattern in a workable order: cut a piece into all its
 * children, then work through each child in turn. So the full-panel cuts
 * come first, then each strip is finished before the next.
 */
export function cutOrder(pattern: Pattern, kerf: number, stages: number | "unlimited"): CutOrder {
  const panel = pattern.root;
  // With trims cut off first, they are the first cuts, then the trimmed
  // panel is cut as a panel would be.
  const root = cuttingRoot(panel);
  const names = new Map<PlanNode, string>();
  nameStrips(root, names);
  const lastStage = stages === "unlimited" ? Infinity : stages;
  const cuts: Cut[] = [];
  if (root !== panel) {
    for (const trim of children(panel).slice(0, -1)) {
      const edge = trimEdge(trim, panel);
      const across = edge === "left" || edge === "right";
      cuts.push({
        number: cuts.length + 1,
        piece: panel,
        direction: across ? "across" : "along",
        measure: across ? trim.length : trim.width,
        produces: trim,
        leaves: null,
        leavesSawDust: false,
        group: "trims",
        trim: false,
        edge,
      });
    }
  }

  const process = (piece: PlanNode, group: string): void => {
    const kids = children(piece);
    if (kids.length === 0) return;
    if (isWrapper(piece)) {
      process(kids[0]!, group);
      return;
    }
    const direction = cutDirection(piece);
    const trim = kids.every((k) => k.stage > lastStage);
    // One cut after every child but the last, which is what remains; a
    // single smaller child (a trim) still needs its cut.
    const count = kids.length === 1 ? 1 : kids.length - 1;
    for (let i = 0; i < count; i++) {
      const child = kids[i]!;
      const rest = kids.length > 1 && i === kids.length - 2 ? kids[i + 1]! : null;
      const restThin = rest !== null && rest.kind === "offcut" && Math.min(rest.length, rest.width) < kerf;
      cuts.push({
        number: cuts.length + 1,
        piece,
        direction,
        measure: direction === "across" ? child.length : child.width,
        produces: unwrap(child),
        leaves: rest && unwrap(rest),
        leavesSawDust: restThin,
        group,
        trim,
      });
    }
    for (const child of kids) {
      process(child, piece === root ? (names.get(child) ?? "panel") : group);
    }
  };
  process(root, "panel");
  return { cuts, names };
}

function nameStrips(root: PlanNode, names: Map<PlanNode, string>): void {
  // Wrapper pieces take no name of their own: look through them.
  const through = (node: PlanNode): PlanNode => (isWrapper(node) && children(node)[0]!.kind === "piece" ? through(children(node)[0]!) : node);
  const top = through(root);
  let letter = 0;
  const visit = (piece: PlanNode, prefix: string, depth: number): void => {
    let n = 0;
    for (const raw of children(piece)) {
      const child = through(raw);
      if (child.kind === "piece" || child.kind === "part") n++;
      if (child.kind !== "piece") continue;
      let name: string;
      let next: string;
      if (depth === 0) {
        next = String.fromCharCode(65 + (letter++ % 26));
        name = `Strip ${next}`;
      } else {
        next = depth === 1 ? `${prefix}${n}` : `${prefix}.${n}`;
        name = `Piece ${next}`;
      }
      names.set(child, name);
      if (raw !== child) names.set(raw, name);
      visit(child, next, depth + 1);
    }
  };
  visit(top, "", 0);
}
