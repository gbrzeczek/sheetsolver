// Walking the cutting tree of a plan (docs/ARCHITECTURE.md, "Solution").
import type { Pattern, PlanNode } from "../api/types";

export type Kind = PlanNode["kind"];

export function children(node: PlanNode): PlanNode[] {
  return node.children ?? [];
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

export function patternStats(pattern: Pattern): PatternStats {
  const all = [...leaves(pattern.root)];
  const partLeaves = all.filter((l) => l.kind === "part");
  const leftovers = all.filter((l) => l.kind === "leftover");
  const panelArea = area(pattern.root);
  const partArea = partLeaves.reduce((sum, l) => sum + area(l), 0);
  const leftoverArea = leftovers.reduce((sum, l) => sum + area(l), 0);
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

/** Millimetres as typed: 720, 3.2, 1040.5. */
export function mm(value: number): string {
  const rounded = Math.round(value * 10) / 10;
  return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1);
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
