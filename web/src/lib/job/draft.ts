// The job as the woodworker edits it: every input kept as typed (text), so
// a half-typed "3," is not lost, and turned into an API request on submit.
import { API_FIELDS } from "../api/fields.generated";
import type { CutType, FirstCuts, Instance, JobRequest, Objective, Panel, Part, Stages, TrimCut, Trims } from "../api/types";

export interface PartRow {
  /** Stable identity for list rendering; not sent. */
  key: number;
  id: string;
  length: string;
  width: string;
  quantity: string;
  grain: boolean;
  value: string;
}

/** The four edges of a panel, as drawn on the plan (docs/FIELDS.md, *Panel trims*). */
export const EDGES = ["left", "right", "bottom", "top"] as const;
export type Edge = (typeof EDGES)[number];

/**
 * Each edge's *Trimmed strip* when none is chosen: upstream's defaults, kept
 * by the owner (docs/FIELDS.md; docs/SCOPE.md, 2026-10-27). The API applies
 * the same; it depends on the edge, so openapi.json gives no single default.
 */
export const DEFAULT_TRIM_CUT: Record<Edge, TrimCut> = {
  left: "cut_off_first",
  right: "left_on_offcut",
  bottom: "cut_off_first",
  top: "left_on_offcut",
};

export interface TrimRow {
  width: string;
  cut: TrimCut;
}

export interface PanelRow {
  key: number;
  id: string;
  length: string;
  width: string;
  quantity: string;
  cost: string;
  trims: Record<Edge, TrimRow>;
}

export interface Draft {
  /** Optional (docs/FIELDS.md, *Job name*). */
  name: string;
  objective: Objective;
  kerf: string;
  stages: Stages;
  cutType: CutType;
  firstCuts: FirstCuts;
  minimumOffcutWidth: string;
  timeLimit: string;
  parts: PartRow[];
  panels: PanelRow[];
}

/** A problem with one field, keyed by its path in the request body, as the API reports it. */
export interface Problem {
  /** `instance.parts[1].width`, `instance.saw.kerf`, `time_limit`, `instance.parts`, or null. */
  field: string | null;
  message: string;
}

let nextKey = 1;
export const newKey = (): number => nextKey++;

/**
 * Takes in a draft whose rows got their keys elsewhere (a stored draft from
 * an earlier visit), so that new rows never reuse one of its keys.
 */
export function adoptKeys(draft: Draft): Draft {
  for (const row of [...draft.parts, ...draft.panels]) nextKey = Math.max(nextKey, row.key + 1);
  return draft;
}

export function emptyPart(): PartRow {
  return { key: newKey(), id: "", length: "", width: "", quantity: "", grain: false, value: "" };
}

export function noTrims(): Record<Edge, TrimRow> {
  return {
    left: { width: "", cut: DEFAULT_TRIM_CUT.left },
    right: { width: "", cut: DEFAULT_TRIM_CUT.right },
    bottom: { width: "", cut: DEFAULT_TRIM_CUT.bottom },
    top: { width: "", cut: DEFAULT_TRIM_CUT.top },
  };
}

export function emptyPanel(): PanelRow {
  return { key: newKey(), id: "", length: "", width: "", quantity: "", cost: "", trims: noTrims() };
}

/**
 * A draft stored before a field existed gets that field's default, so a
 * draft kept in the browser across an update still opens.
 */
export function upgradeDraft(draft: Draft): Draft {
  draft.minimumOffcutWidth ??= "";
  for (const panel of draft.panels) panel.trims ??= noTrims();
  return draft;
}

/** A new job with the FIELDS.md defaults, one empty part and one empty panel. */
export function newDraft(): Draft {
  return {
    name: "",
    objective: API_FIELDS["Instance.objective"].default as Objective,
    kerf: String(API_FIELDS["Saw.kerf"].default),
    stages: API_FIELDS["Saw.stages"].default as Stages,
    cutType: API_FIELDS["Saw.cut_type"].default as CutType,
    firstCuts: API_FIELDS["Saw.first_cuts"].default as FirstCuts,
    minimumOffcutWidth: "",
    timeLimit: String(API_FIELDS["JobRequest.time_limit"].default),
    parts: [emptyPart()],
    panels: [emptyPanel()],
  };
}

/**
 * The names sent for each row: the typed name, or for an empty one the
 * default from FIELDS.md ("Part 3" for row 3), skipping names already used
 * (docs/FIELDS.md, "Names are optional").
 */
export function namesFor(rows: readonly { id: string }[], prefix: "Part" | "Panel"): string[] {
  const used = new Set(rows.map((r) => r.id.trim()).filter(Boolean));
  return rows.map((row, index) => {
    const typed = row.id.trim();
    if (typed) return typed;
    let n = index + 1;
    while (used.has(`${prefix} ${n}`)) n++;
    const name = `${prefix} ${n}`;
    used.add(name);
    return name;
  });
}

function isBlankPart(row: PartRow): boolean {
  return !row.id.trim() && !row.length.trim() && !row.width.trim() && !row.quantity.trim() && !row.value.trim() && !row.grain;
}

function isBlankPanel(row: PanelRow): boolean {
  return (
    !row.id.trim() && !row.length.trim() && !row.width.trim() && !row.quantity.trim() && !row.cost.trim() &&
    EDGES.every((edge) => !row.trims[edge].width.trim())
  );
}

/** Rows to send: all of them except wholly empty rows at the end, so positions match row numbers. */
export function rowsToSend<T>(rows: readonly T[], blank: (row: T) => boolean): T[] {
  let end = rows.length;
  while (end > 0 && blank(rows[end - 1]!)) end--;
  return rows.slice(0, end);
}

// --- Parsing, with the API's wording for the same mistakes ---

type Parsed<T> = { ok: true; value: T } | { ok: false; message: string };

const DECIMAL = /^\d+(?:[.,]\d+)?$/;

/** The longest length the API accepts, in millimetres (docs/FIELDS.md). */
const LONGEST = API_FIELDS["Part.length"].maximum ?? Infinity;

/**
 * Millimetres with at most one decimal. A comma is accepted as the decimal
 * mark, and spaces as thousands separators ("1 040.5", as the plan shows it).
 */
export function parseLength(text: string, what: string, minimum: number): Parsed<number> {
  const t = text.trim().replace(/(?<=\d)[\s\u202f\u2009](?=\d{3}(?:\D|$))/g, "");
  if (!t) return { ok: false, message: `enter the ${what} in millimetres` };
  if (!DECIMAL.test(t)) return { ok: false, message: "must be a length in millimetres, such as 720 or 3.2" };
  const normal = t.replace(",", ".");
  const decimals = normal.split(".")[1] ?? "";
  // "1,200" or "2.500" is a thousands separator as often as a decimal mark;
  // reading it as 1.2 mm would silently make a tiny part.
  if (decimals.length === 3) {
    const whole = normal.split(".")[0];
    return { ok: false, message: `write ${t} without a separator: ${whole}${decimals} for ${whole}${decimals} mm, or ${Number(normal)} for ${Number(normal)} mm` };
  }
  if (decimals.length > 1 && !/^\d0+$/.test(decimals)) {
    return { ok: false, message: `length ${normal} mm has more than one decimal; lengths are in steps of 0.1 mm` };
  }
  const value = Math.round(Number(normal) * 10) / 10;
  if (value < minimum) {
    return { ok: false, message: minimum > 0 ? `${what} must be at least 0.1 mm` : `${what} must be at least 0 mm` };
  }
  if (value > LONGEST) return { ok: false, message: `${what} must be at most ${LONGEST} mm` };
  return { ok: true, value };
}

export function parseCount(text: string, minimum: number, maximum?: number): Parsed<number> {
  const t = text.trim();
  if (!/^\d+$/.test(t)) return { ok: false, message: "must be a whole number, such as 1 or 4" };
  const value = Number(t);
  if (value < minimum || (maximum !== undefined && value > maximum)) {
    return {
      ok: false,
      message: maximum !== undefined ? `must be from ${minimum} to ${maximum}` : `must be at least ${minimum}`,
    };
  }
  return { ok: true, value };
}

export function parseNumber(text: string): Parsed<number> {
  const t = text.trim().replace(",", ".");
  const value = Number(t);
  if (!t || !Number.isFinite(value) || value < 0) return { ok: false, message: "must be a number of at least 0" };
  return { ok: true, value };
}

/**
 * The request for `POST /jobs`, and every problem the UI can find without
 * the server. The request is only complete when there are no problems.
 * Problem paths are the API's, so local and server problems show the same way.
 */
export function toRequest(draft: Draft): { request: JobRequest; problems: Problem[] } {
  const problems: Problem[] = [];
  const at = (field: string, message: string) => problems.push({ field, message });

  const parts = rowsToSend(draft.parts, isBlankPart);
  const panels = rowsToSend(draft.panels, isBlankPanel);
  if (parts.length === 0) at("instance.parts", "add at least one part");
  if (panels.length === 0) at("instance.panels", "add at least one panel");

  const partNames = namesFor(parts, "Part");
  const panelNames = namesFor(panels, "Panel");
  duplicates(partNames).forEach((index) => at(`instance.parts[${index}].id`, `part name "${partNames[index]}" is used more than once`));
  duplicates(panelNames).forEach((index) => at(`instance.panels[${index}].id`, `panel name "${panelNames[index]}" is used more than once`));

  const outParts: Part[] = parts.map((row, i) => {
    const path = `instance.parts[${i}]`;
    const part: Part = { id: partNames[i]!, length: 0, width: 0, quantity: 1, grain: row.grain };
    const length = parseLength(row.length, "length", 0.1);
    const width = parseLength(row.width, "width", 0.1);
    if (length.ok) part.length = length.value;
    else at(`${path}.length`, length.message);
    if (width.ok) part.width = width.value;
    else at(`${path}.width`, width.message);
    if (row.quantity.trim()) {
      const quantity = parseCount(row.quantity, 1);
      if (quantity.ok) part.quantity = quantity.value;
      else at(`${path}.quantity`, quantity.message === "must be at least 1" ? "quantity must be at least 1" : quantity.message);
    }
    if (row.value.trim()) {
      const value = parseNumber(row.value);
      if (value.ok) part.value = value.value;
      else at(`${path}.value`, value.message);
    }
    return part;
  });

  const outPanels: Panel[] = panels.map((row, i) => {
    const path = `instance.panels[${i}]`;
    const panel: Panel = { id: panelNames[i]!, length: 0, width: 0 };
    const length = parseLength(row.length, "length", 0.1);
    const width = parseLength(row.width, "width", 0.1);
    if (length.ok) panel.length = length.value;
    else at(`${path}.length`, length.message);
    if (width.ok) panel.width = width.value;
    else at(`${path}.width`, width.message);
    if (row.quantity.trim()) {
      const quantity = parseCount(row.quantity, 1);
      if (quantity.ok) panel.quantity = quantity.value;
      else at(`${path}.quantity`, quantity.message === "must be at least 1" ? "quantity must be at least 1" : quantity.message);
    }
    if (row.cost.trim()) {
      const cost = parseNumber(row.cost);
      if (cost.ok) panel.cost = cost.value;
      else at(`${path}.cost`, cost.message);
    }
    // An edge with an empty or zero trim has none, and is not sent.
    const trims: Trims = {};
    const widths: Partial<Record<Edge, number>> = {};
    for (const edge of EDGES) {
      const trim = row.trims[edge];
      if (!trim.width.trim()) continue;
      const width = parseLength(trim.width, `${edge} trim`, 0);
      if (!width.ok) {
        at(`${path}.trims.${edge}.width`, width.message);
        continue;
      }
      widths[edge] = width.value;
      if (width.value > 0) trims[edge] = { width: width.value, cut: trim.cut };
    }
    // Opposite trims must leave some of the panel (the API's wording).
    const pairs = [
      ["right", "left and right", "length", (widths.left ?? 0) + (widths.right ?? 0), panel.length],
      ["top", "bottom and top", "width", (widths.bottom ?? 0) + (widths.top ?? 0), panel.width],
    ] as const;
    for (const [edge, edges, side, total, extent] of pairs) {
      if (extent > 0 && total > 0 && Math.round(total * 10) >= Math.round(extent * 10)) {
        at(`${path}.trims.${edge}.width`, `the ${edges} trims of panel "${panel.id}" together must be less than its ${side}`);
      }
    }
    if (Object.keys(trims).length) panel.trims = trims;
    return panel;
  });

  // An empty kerf means no kerf: there is nothing to cut away.
  let kerf = 0;
  if (draft.kerf.trim()) {
    const parsedKerf = parseLength(draft.kerf, "saw blade thickness", 0);
    if (parsedKerf.ok) kerf = parsedKerf.value;
    else at("instance.saw.kerf", parsedKerf.message);
  }

  // An empty minimum offcut width means none.
  let minimumOffcutWidth = 0;
  if (draft.minimumOffcutWidth.trim()) {
    const parsed = parseLength(draft.minimumOffcutWidth, "minimum offcut width", 0);
    if (parsed.ok) minimumOffcutWidth = parsed.value;
    else at("instance.saw.minimum_offcut_width", parsed.message);
  }

  const limit = API_FIELDS["JobRequest.time_limit"];
  let timeLimit = limit.default as number;
  const parsedLimit = parseCount(draft.timeLimit, limit.minimum, limit.maximum);
  if (parsedLimit.ok) timeLimit = parsedLimit.value;
  else at("time_limit", `time limit must be a whole number of seconds from ${limit.minimum} to ${limit.maximum}`);

  const openDimension = draft.objective === "open_dimension_x" || draft.objective === "open_dimension_y";
  if (openDimension && (outPanels.length !== 1 || (outPanels[0]?.quantity ?? 1) !== 1)) {
    at("instance.panels", '"Shortest length/width from one panel" needs exactly one panel, with quantity 1');
  }

  // Job name: optional, one line, at most 100 characters (docs/API.md).
  const name = draft.name.trim();
  const maxName = API_FIELDS["JobRequest.name"].maxLength ?? 100;
  if ([...name].length > maxName) at("name", `job name must be at most ${maxName} characters, not ${[...name].length}`);
  // eslint-disable-next-line no-control-regex -- control characters are what is checked
  else if (/[\u0000-\u001f\u007f]/.test(name)) at("name", "job name must be one line, without tabs or line breaks");

  const instance: Instance = {
    objective: draft.objective,
    saw: {
      kerf,
      stages: draft.stages,
      cut_type: draft.cutType,
      first_cuts: draft.firstCuts,
      minimum_offcut_width: minimumOffcutWidth,
    },
    panels: outPanels,
    parts: outParts,
  };
  const request: JobRequest = { instance, time_limit: timeLimit };
  if (name) request.name = name;
  return { request, problems };
}

function duplicates(names: readonly string[]): number[] {
  const first = new Map<string, number>();
  const out: number[] = [];
  names.forEach((name, index) => {
    if (first.has(name)) out.push(index);
    else first.set(name, index);
  });
  return out;
}

const fmt = (n: number | null | undefined): string => (n === null || n === undefined ? "" : String(n));

/**
 * A draft from a submitted instance, for "Edit job" on a job opened from a
 * link. Names equal to their generated default come back empty, so they
 * stay generated.
 */
export function fromInstance(instance: Instance, timeLimit: number, name = ""): Draft {
  const parts = instance.parts.map((p, i): PartRow => ({
    key: newKey(),
    id: p.id === `Part ${i + 1}` ? "" : p.id,
    length: fmt(p.length),
    width: fmt(p.width),
    quantity: fmt(p.quantity),
    grain: p.grain ?? false,
    value: fmt(p.value),
  }));
  const panels = instance.panels.map((p, i): PanelRow => ({
    key: newKey(),
    id: p.id === `Panel ${i + 1}` ? "" : p.id,
    length: fmt(p.length),
    width: fmt(p.width),
    quantity: fmt(p.quantity),
    cost: fmt(p.cost),
    trims: trimRows(p.trims),
  }));
  return {
    name,
    objective: instance.objective,
    kerf: fmt(instance.saw.kerf),
    stages: instance.saw.stages,
    cutType: instance.saw.cut_type,
    firstCuts: instance.saw.first_cuts,
    minimumOffcutWidth: instance.saw.minimum_offcut_width ? fmt(instance.saw.minimum_offcut_width) : "",
    timeLimit: String(timeLimit),
    parts: parts.length ? parts : [emptyPart()],
    panels: panels.length ? panels : [emptyPanel()],
  };
}

/** The trims rows of a panel as sent, with each edge's default cut where it has none. */
function trimRows(trims: Trims | undefined): Record<Edge, TrimRow> {
  const rows = noTrims();
  for (const edge of EDGES) {
    const trim = trims?.[edge];
    if (trim?.width) rows[edge] = { width: fmt(trim.width), cut: trim.cut ?? DEFAULT_TRIM_CUT[edge] };
  }
  return rows;
}

/** A panel's trims by meaning: each edge's width and cut, an edge without a trim as none. */
function canonicalTrims(trims: Trims | undefined) {
  return EDGES.map((edge) => {
    const trim = trims?.[edge];
    return trim?.width ? [trim.width, trim.cut ?? DEFAULT_TRIM_CUT[edge]] : null;
  });
}

/**
 * Whether two instances ask for the same plan (DESIGN.md §2, "A plan for an
 * older job"). Optional fields compare by their meaning, not their spelling:
 * the server echoes a job with its defaults filled in, which must not make
 * a plan look out of date.
 */
export function sameInstance(a: Instance, b: Instance): boolean {
  return JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));
}

function canonical(instance: Instance) {
  return {
    objective: instance.objective,
    saw: [instance.saw.kerf, instance.saw.stages, instance.saw.cut_type, instance.saw.first_cuts, instance.saw.minimum_offcut_width ?? 0],
    parts: instance.parts.map((p) => [p.id, p.length, p.width, p.quantity ?? 1, p.grain ?? false, p.value ?? null]),
    panels: instance.panels.map((p) => [p.id, p.length, p.width, p.quantity ?? null, p.cost ?? null, canonicalTrims(p.trims)]),
  };
}

/** Whether the draft, as it would be sent now, asks for the plan of `instance`. */
export function draftMatches(draft: Draft, instance: Instance): boolean {
  const { request, problems } = toRequest(draft);
  return problems.length === 0 && sameInstance(request.instance, instance);
}
