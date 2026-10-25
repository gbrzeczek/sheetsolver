// Fitting labels into the rectangles of a drawing (web/DESIGN.md §4.6):
// name over size, the same turned 90°, size only, or nothing.

export type LabelFit =
  | { mode: "stacked"; rotate: boolean }
  | { mode: "single"; rotate: boolean }
  | { mode: "none" };

/** Width of `text` at `size` px, estimated: fitting must not need the DOM. */
export function textWidth(text: string, size: number, bold = false): number {
  return text.length * size * (bold ? 0.6 : 0.56);
}

/**
 * How to label a rectangle of `w` × `h` px with a `name` (bold) over a
 * `detail` line; either may be absent.
 */
export function fitLabel(w: number, h: number, name: string | null, detail: string | null, nameSize: number, detailSize: number): LabelFit {
  if (name && detail) {
    const needW = Math.max(textWidth(name, nameSize, true), textWidth(detail, detailSize)) + 10;
    const needH = nameSize + detailSize + 6;
    if (w >= needW && h >= needH) return { mode: "stacked", rotate: false };
    if (h >= needW && w >= needH && h > w) return { mode: "stacked", rotate: true };
  }
  const single = detail ?? name;
  if (!single) return { mode: "none" };
  const size = detail ? detailSize : nameSize;
  const tw = textWidth(single, size, !detail) + 8;
  if (w >= tw && h >= size + 5) return { mode: "single", rotate: false };
  if (h >= tw && w >= size + 5) return { mode: "single", rotate: true };
  return { mode: "none" };
}
