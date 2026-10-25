// What the plan view says first (web/DESIGN.md §1: "Did everything fit, and
// how many panels does it take?").
import type { Instance, Metrics, Part, Solution } from "../api/types";
import { leaves, mm, partId } from "./tree";

export function totalParts(instance: Instance): number {
  return instance.parts.reduce((sum, p) => sum + (p.quantity ?? 1), 0);
}

/** "All 21 parts fit", "18 of 21 parts fit", "Uses 1850 mm of 2440 mm". */
export function headline(instance: Instance, metrics: Metrics): string {
  const total = totalParts(instance);
  if (instance.objective === "open_dimension_x") {
    return `Uses ${mm(metrics.used_length)} mm of ${mm(instance.panels[0]?.length ?? 0)} mm`;
  }
  if (instance.objective === "open_dimension_y") {
    return `Uses ${mm(metrics.used_width)} mm of ${mm(instance.panels[0]?.width ?? 0)} mm`;
  }
  if (metrics.parts_cut >= total) return total === 1 ? "The part fits" : `All ${total} parts fit`;
  return `${metrics.parts_cut} of ${total} parts fit`;
}

/** "3 panels of Birch ply 18 mm, 2440 × 1220 mm", or a list for several sizes. */
export function panelsLine(instance: Instance, plan: Solution): string {
  const counts = new Map<string, number>();
  for (const p of plan.patterns) counts.set(p.panel_id, (counts.get(p.panel_id) ?? 0) + p.repeat);
  const total = [...counts.values()].reduce((a, b) => a + b, 0);
  const describe = (id: string) => {
    const panel = instance.panels.find((p) => p.id === id);
    return panel ? `${id}, ${mm(panel.length)} × ${mm(panel.width)} mm` : id;
  };
  const noun = (n: number) => (n === 1 ? "panel" : "panels");
  if (counts.size === 1) {
    const [id] = counts.keys();
    return `${total} ${noun(total)} of ${describe(id!)}`;
  }
  return `${total} panels: ${[...counts].map(([id, n]) => `${n} × ${describe(id)}`).join("; ")}`;
}

/** For each part of the job: how many are needed, how many cut, and on which panels (1-based). */
export function checklist(instance: Instance, plan: Solution | null): { part: Part; needed: number; cut: number; panels: number[] }[] {
  const cut = new Map<string, number>();
  const where = new Map<string, Set<number>>();
  plan?.patterns.forEach((pattern, i) => {
    for (const leaf of leaves(pattern.root)) {
      const id = partId(leaf);
      if (!id) continue;
      cut.set(id, (cut.get(id) ?? 0) + pattern.repeat);
      if (!where.has(id)) where.set(id, new Set());
      where.get(id)!.add(i + 1);
    }
  });
  return instance.parts.map((part) => ({
    part,
    needed: part.quantity ?? 1,
    cut: cut.get(part.id) ?? 0,
    panels: [...(where.get(part.id) ?? [])],
  }));
}

/** Parts that fit on no panel, turned or not (a common reason for "no plan"). */
export function oversizedParts(instance: Instance): Part[] {
  return instance.parts.filter(
    (part) =>
      !instance.panels.some(
        (panel) =>
          (part.length <= panel.length && part.width <= panel.width) ||
          (!part.grain && part.width <= panel.length && part.length <= panel.width),
      ),
  );
}
