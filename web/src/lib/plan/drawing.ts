// The geometry of a panel drawing in screen pixels (web/DESIGN.md §4.6):
// length horizontal, width vertical, y = 0 at the bottom edge. Laid out at
// the width it is shown at, so labels keep their pixel size.
import type { Pattern, PlanNode, Trims } from "../api/types";
import type { Cut } from "./cutorder";
import { fitLabel, textWidth, type LabelFit } from "./labels";
import { children, cuttingRoot, leaves, mm, size, trimEdge, trimWidths } from "./tree";

export type LabelMode = "full" | "names" | "none";

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Shape extends Box {
  node: PlanNode;
  kind: "part" | "offcut" | "leftover" | "trim";
  name: string | null;
  detail: string | null;
  fit: LabelFit;
  grain: boolean;
  rotated: boolean;
}

export interface Badge {
  number: number;
  cx: number;
  cy: number;
  /** The saw line of this cut. */
  line: Box;
  /** What the cut produces. */
  result: Box;
}

export interface RulerMark {
  from: number;
  to: number;
  label: string | null;
  weak: boolean;
}

export interface Drawing {
  width: number;
  height: number;
  panel: Box;
  shapes: Shape[];
  /** Trims left on the offcut: no part is there, but they are no piece of their own. */
  trimBands: Box[];
  badges: Badge[];
  /** Panel length along the top, panel width on the right, strips on the left. */
  rulers: { top: RulerMark; right: RulerMark; strips: RulerMark[]; stripsSide: "left" | "bottom" } | null;
  nameSize: number;
  detailSize: number;
}

export interface DrawingOptions {
  /** Width of the panel itself, in px. */
  width: number;
  labels: LabelMode;
  rulers?: boolean;
  /** The cuts, to place numbered badges; omit for no badges. */
  cuts?: readonly Cut[];
  kerf: number;
  grain(partId: string): boolean;
  nameSize?: number;
  /** The panel's trims, from the job: those left on the offcut are drawn as bands. */
  trims?: Trims;
}

export const BADGE_RADIUS = 9;

export function layout(pattern: Pattern, options: DrawingOptions): Drawing {
  const root = pattern.root;
  const L = root.length;
  const W = root.width;
  const along = pattern.first_cuts === "along_length";
  // Room for the rulers: strips on the left for cuts along the length,
  // along the bottom for cuts across it.
  const [ml, mr, mt, mb] = options.rulers ? (along ? [70, 36, 34, 8] : [16, 36, 34, 30]) : [1, 1, 1, 1];
  const s = options.width / L;
  const nameSize = options.nameSize ?? 12;
  const detailSize = nameSize - 1;

  const box = (n: { x: number; y: number; length: number; width: number }): Box => ({
    x: ml + n.x * s,
    y: mt + (W - n.y - n.width) * s,
    w: n.length * s,
    h: n.width * s,
  });

  const shapes: Shape[] = [];
  for (const leaf of leaves(root)) {
    if (leaf.kind === "piece") continue;
    const b = box(leaf);
    const kind = leaf.kind;
    let name: string | null = null;
    let detail: string | null = null;
    if (kind === "part") {
      name = options.labels === "none" ? null : leaf.part_id;
      detail = options.labels === "full" ? size(leaf) : null;
    } else if (kind === "trim") {
      name = options.labels === "full" ? "Trim" : null;
    } else if (kind === "offcut") {
      // A sliver thinner than the kerf is saw dust: no label.
      detail = options.labels === "full" && Math.min(leaf.length, leaf.width) >= options.kerf ? size(leaf) : null;
    } else {
      name = options.labels === "none" ? null : options.labels === "full" ? "Keep · leftover" : "Keep";
      detail = options.labels === "full" ? size(leaf) : null;
    }
    const sizes = kind === "offcut" ? [nameSize, detailSize - 0.5] : [nameSize, detailSize];
    shapes.push({
      ...b,
      node: leaf,
      kind,
      name,
      detail,
      fit: fitLabel(b.w, b.h, name, detail, sizes[0]!, sizes[1]!),
      grain: kind === "part" && options.grain(leaf.part_id),
      rotated: kind === "part" && leaf.rotated,
    });
  }

  // A trim left on the offcut has no piece: show where it is.
  const cutFirst = new Set(
    children(root)
      .filter((c) => c.kind === "trim")
      .map((c) => trimEdge(c, root)),
  );
  const t = trimWidths(options.trims);
  const trimBands: Box[] = [];
  if (t.left && !cutFirst.has("left")) trimBands.push(box({ x: 0, y: 0, length: t.left, width: W }));
  if (t.right && !cutFirst.has("right")) trimBands.push(box({ x: L - t.right, y: 0, length: t.right, width: W }));
  if (t.bottom && !cutFirst.has("bottom")) trimBands.push(box({ x: 0, y: 0, length: L, width: t.bottom }));
  if (t.top && !cutFirst.has("top")) trimBands.push(box({ x: 0, y: W - t.top, length: L, width: t.top }));

  const badges: Badge[] = [];
  if (options.cuts) {
    const placed: [number, number][] = [];
    const r = BADGE_RADIUS;
    for (const cut of options.cuts) {
      const p = box(cut.piece);
      const c = cut.produces;
      const gap = Math.max(options.kerf * s, 1.5);
      let line: Box;
      let cx: number;
      let cy: number;
      let dx = 0;
      let dy = 0;
      if (cut.edge) {
        // A trim cut off first: the saw cut is inside the trim, at its
        // inner edge.
        const thin = Math.max(gap, 1.5);
        if (cut.edge === "left" || cut.edge === "right") {
          const gx = ml + (cut.edge === "left" ? c.x + c.length : c.x) * s - (cut.edge === "left" ? thin : 0);
          line = { x: gx, y: p.y, w: thin, h: p.h };
          cx = gx + thin / 2;
          cy = p.y + 12;
          dy = 2 * r + 3;
        } else {
          const gy = mt + (W - (cut.edge === "bottom" ? c.y + c.width : c.y)) * s;
          line = { x: p.x, y: cut.edge === "bottom" ? gy : gy - thin, w: p.w, h: thin };
          cx = p.x + 12;
          cy = line.y + thin / 2;
          dx = 2 * r + 3;
        }
      } else if (cut.direction === "across") {
        const gx = ml + (c.x + c.length) * s;
        line = { x: gx, y: p.y, w: gap, h: p.h };
        cx = gx + gap / 2;
        cy = p.y + 12;
        dy = 2 * r + 3;
      } else {
        // The gap lies just above the child (y is flipped).
        const gy = mt + (W - c.y - c.width) * s;
        line = { x: p.x, y: gy - gap, w: p.w, h: gap };
        cx = p.x + 12;
        cy = gy - gap / 2;
        dx = 2 * r + 3;
      }
      // Nudge along the saw line until clear of the badges placed so far.
      for (let i = 0; i < 12 && placed.some(([qx, qy]) => (cx - qx) ** 2 + (cy - qy) ** 2 < (2 * r + 2) ** 2); i++) {
        cx += dx;
        cy += dy;
      }
      placed.push([cx, cy]);
      badges.push({ number: cut.number, cx, cy, line, result: box(c) });
    }
  }

  let rulers: Drawing["rulers"] = null;
  if (options.rulers) {
    const strips: RulerMark[] = [];
    // The strips of the first cuts, after any trims cut off first.
    const stripsOf = cuttingRoot(root);
    if (along) {
      for (const c of children(stripsOf)) {
        const a = mt + (W - c.y - c.width) * s;
        const b = mt + (W - c.y) * s;
        strips.push({ from: a, to: b, label: b - a >= 14 ? mm(c.width) : null, weak: c.kind === "offcut" || c.kind === "leftover" });
      }
    } else {
      for (const c of children(stripsOf)) {
        const a = ml + c.x * s;
        const b = ml + (c.x + c.length) * s;
        strips.push({ from: a, to: b, label: b - a >= textWidth(mm(c.length), 11) + 8 ? mm(c.length) : null, weak: c.kind === "offcut" || c.kind === "leftover" });
      }
    }
    rulers = {
      top: { from: ml, to: ml + L * s, label: mm(L), weak: false },
      right: { from: mt, to: mt + W * s, label: mm(W), weak: false },
      strips,
      stripsSide: along ? "left" : "bottom",
    };
  }

  return {
    width: options.width + ml + mr,
    height: W * s + mt + mb,
    panel: { x: ml, y: mt, w: L * s, h: W * s },
    shapes,
    trimBands,
    badges,
    rulers,
    nameSize,
    detailSize,
  };
}
