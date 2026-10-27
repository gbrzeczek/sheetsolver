// Where a problem from the API (or from the local checks, which use the
// same paths) belongs on the page.
import { field, type FieldKey } from "../api/fields";
import type { Problem } from "./draft";

export interface ProblemPlace {
  /** The section of the page: the parts or panels table, the settings, or the page as a whole. */
  section: "parts" | "panels" | "settings" | "page";
  /** 0-based row in a table, if the problem is in one row. */
  row?: number;
  /** The panel edge, for a problem with one of its trims. */
  edge?: string;
  /** The field's key, if the problem is in one field. */
  field?: FieldKey;
  /** "Row 2, Width", "Saw blade thickness (kerf)", "Parts". */
  label: string;
}

const ROW = /^instance\.(parts|panels)\[(\d+)\]\.(\w+)$/;
const TRIM = /^instance\.panels\[(\d+)\]\.trims\.(left|right|bottom|top)\.(width|cut)$/;

/** The edges as the trims editor names them (docs/FIELDS.md, *Panel trims*). */
export const EDGE_NAMES: Record<string, string> = {
  left: "Left end",
  right: "Right end",
  bottom: "Bottom edge",
  top: "Top edge",
};

const SETTINGS: Record<string, FieldKey> = {
  "instance.objective": "Instance.objective",
  "instance.saw.kerf": "Saw.kerf",
  "instance.saw.stages": "Saw.stages",
  "instance.saw.cut_type": "Saw.cut_type",
  "instance.saw.first_cuts": "Saw.first_cuts",
  "instance.saw.minimum_offcut_width": "Saw.minimum_offcut_width",
  time_limit: "JobRequest.time_limit",
  name: "JobRequest.name",
};

export function place(path: string | null): ProblemPlace {
  if (path === null) return { section: "page", label: "Job" };
  const trim = TRIM.exec(path);
  if (trim) {
    const [, index, edge, name] = trim;
    const key = `Trim.${name}` as FieldKey;
    return {
      section: "panels",
      row: Number(index),
      edge,
      field: key,
      label: `Row ${Number(index) + 1}, ${EDGE_NAMES[edge!]} ${field(key).label.toLowerCase()}`,
    };
  }
  const row = ROW.exec(path);
  if (row) {
    const [, list, index, name] = row;
    const key = `${list === "parts" ? "Part" : "Panel"}.${name}` as FieldKey;
    const label = safeLabel(key) ?? name!;
    return { section: list as "parts" | "panels", row: Number(index), field: key, label: `Row ${Number(index) + 1}, ${label}` };
  }
  if (path === "instance.parts" || path.startsWith("instance.parts[")) return { section: "parts", label: "Parts" };
  if (path === "instance.panels" || path.startsWith("instance.panels[")) return { section: "panels", label: "Panels" };
  const setting = SETTINGS[path];
  if (setting) return { section: "settings", field: setting, label: field(setting).label };
  return { section: "page", label: "Job" };
}

function safeLabel(key: FieldKey): string | undefined {
  try {
    return field(key).label;
  } catch {
    return undefined;
  }
}

/** Problems by exact field path, for looking up one cell. */
export function byPath(problems: readonly Problem[]): Map<string, string> {
  const map = new Map<string, string>();
  for (const p of problems) {
    if (p.field !== null && !map.has(p.field)) map.set(p.field, p.message);
  }
  return map;
}
