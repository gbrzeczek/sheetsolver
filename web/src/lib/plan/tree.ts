// Walking the cutting tree of a plan (docs/ARCHITECTURE.md, "Solution").
import type { Instance, Pattern, PlanNode, Trims } from "../api/types";

export type Kind = PlanNode["kind"];

export function children(node: PlanNode): PlanNode[] {
  return node.children ?? [];
}

/**
 * The piece the stage cuts start from: the panel, or with trims cut off
 * first (docs/ARCHITECTURE.md), the trimmed panel after them.
 */
export function cuttingRoot(root: PlanNode): PlanNode {
  const kids = children(root);
  return kids[0]?.kind === "trim" ? kids[kids.length - 1]! : root;
}

/** Which edge a trim cut off first is at. */
export function trimEdge(trim: PlanNode, panel: PlanNode): "left" | "right" | "bottom" | "top" {
  if (trim.width === panel.width) return trim.x === 0 ? "left" : "right";
  return trim.y === 0 ? "bottom" : "top";
}

export function* leaves(node: PlanNode): Generator<PlanNode> {
  const kids = children(node);
  if (kids.length === 0) yield node;
  else for (const child of kids) yield* leaves(child);
}

export function* nodes(node: PlanNode): Generator<PlanNode> {
  yield node;
  for (const child of children(node)) yield* nodes(child);
}

export function area(node: { length: number; width: number }): number {
  return node.length * node.width;
}

export function partId(node: PlanNode): string | null {
  return node.kind === "part" ? node.part_id : null;
}

export function isRotated(node: PlanNode): boolean {
  return node.kind === "part" && node.rotated;
}

/** Figures for one panel of a plan. The API gives them for the whole plan only (§8, #8). */
export interface PatternStats {
  parts: number;
  panelArea: number;
  partArea: number;
  leftoverArea: number;
  wasteArea: number;
  /** Waste as a fraction of the panel without the leftover, as `metrics.waste_fraction`. */
  wasteFraction: number;
  offcuts: PlanNode[];
  leftovers: PlanNode[];
  partLeaves: PlanNode[];
}

/** The trims of the panel a pattern cuts, from the job. */
export function panelTrims(instance: Instance | null | undefined, pattern: Pattern): Trims | undefined {
  return instance?.panels.find((p) => p.id === pattern.panel_id)?.trims;
}

/** The width of each edge's trim, 0 where there is none. */
export function trimWidths(trims: Trims | undefined): { left: number; right: number; bottom: number; top: number } {
  return {
    left: trims?.left?.width ?? 0,
    right: trims?.right?.width ?? 0,
    bottom: trims?.bottom?.width ?? 0,
    top: trims?.top?.width ?? 0,
  };
}

/**
 * The panel's figures. As in the API's metrics (upstream's definition), the
 * panel's area is the part inside its trims, cut off first or left on the
 * offcut: no part can be in them.
 */
export function patternStats(pattern: Pattern, trims?: Trims): PatternStats {
  const all = [...leaves(pattern.root)];
  const partLeaves = all.filter((l) => l.kind === "part");
  const leftovers = all.filter((l) => l.kind === "leftover");
  const t = trimWidths(trims);
  const root = pattern.root;
  const inside = { x0: t.left, y0: t.bottom, x1: root.length - t.right, y1: root.width - t.top };
  const panelArea = (inside.x1 - inside.x0) * (inside.y1 - inside.y0);
  const partArea = partLeaves.reduce((sum, l) => sum + area(l), 0);
  // Only the part of the leftover inside the trims.
  const leftoverArea = leftovers.reduce((sum, l) => {
    const w = Math.min(l.x + l.length, inside.x1) - Math.max(l.x, inside.x0);
    const h = Math.min(l.y + l.width, inside.y1) - Math.max(l.y, inside.y0);
    return sum + (w > 0 && h > 0 ? w * h : 0);
  }, 0);
  const wasteArea = panelArea - partArea - leftoverArea;
  return {
    parts: partLeaves.length,
    panelArea,
    partArea,
    leftoverArea,
    wasteArea,
    wasteFraction: panelArea - leftoverArea > 0 ? wasteArea / (panelArea - leftoverArea) : 0,
    offcuts: all.filter((l) => l.kind === "offcut"),
    leftovers,
    partLeaves,
  };
}

/**
 * The thousands separator in shown sizes: a narrow no-break space, so
 * "1 040.5" never breaks across lines (owner's decision, docs/FIELDS.md).
 */
export const THOUSANDS = "\u202f";

/** Millimetres for reading: 720, 3.2, 1 040.5, 2 440. */
export function mm(value: number): string {
  const rounded = Math.round(value * 10) / 10;
  const [whole, decimal] = (Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1)).split(".");
  const grouped = whole!.length >= 4 ? whole!.replace(/\B(?=(\d{3})+$)/g, THOUSANDS) : whole!;
  return decimal ? `${grouped}.${decimal}` : grouped;
}

export function size(node: { length: number; width: number }): string {
  return `${mm(node.length)} × ${mm(node.width)}`;
}

export function percent(fraction: number): string {
  return `${(fraction * 100).toFixed(1)}%`;
}

export function squareMetres(mm2: number): string {
  return `${(mm2 / 1e6).toFixed(2)} m²`;
}
