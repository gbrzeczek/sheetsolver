// The job as the woodworker edits it: every input kept as typed (text), so
// a half-typed "3," is not lost, and turned into an API request on submit.
import { API_FIELDS } from "../api/fields.generated";
import type { CutType, FirstCuts, Instance, JobRequest, Objective, Panel, Part, Stages } from "../api/types";

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

export interface PanelRow {
  key: number;
  id: string;
  length: string;
  width: string;
  quantity: string;
  cost: string;
}

export interface Draft {
  /** Optional (docs/FIELDS.md, *Job name*). */
  name: string;
  objective: Objective;
  kerf: string;
  stages: Stages;
  cutType: CutType;
  firstCuts: FirstCuts;
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

export function emptyPart(): PartRow {
  return { key: newKey(), id: "", length: "", width: "", quantity: "", grain: false, value: "" };
}

export function emptyPanel(): PanelRow {
  return { key: newKey(), id: "", length: "", width: "", quantity: "", cost: "" };
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
  return !row.id.trim() && !row.length.trim() && !row.width.trim() && !row.quantity.trim() && !row.cost.trim();
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
  if (decimals.length > 1 && !/^\d0+$/.test(decimals)) {
    return { ok: false, message: `length ${normal} mm has more than one decimal; lengths are in steps of 0.1 mm` };
  }
  const value = Math.round(Number(normal) * 10) / 10;
  if (value < minimum) {
    return { ok: false, message: minimum > 0 ? `${what} must be at least 0.1 mm` : `${what} must be at least 0 mm` };
  }
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
    return panel;
  });

  let kerf = 0;
  const parsedKerf = parseLength(draft.kerf, "saw blade thickness", 0);
  if (parsedKerf.ok) kerf = parsedKerf.value;
  else at("instance.saw.kerf", parsedKerf.message);

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
    saw: { kerf, stages: draft.stages, cut_type: draft.cutType, first_cuts: draft.firstCuts },
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
  }));
  return {
    name,
    objective: instance.objective,
    kerf: fmt(instance.saw.kerf),
    stages: instance.saw.stages,
    cutType: instance.saw.cut_type,
    firstCuts: instance.saw.first_cuts,
    timeLimit: String(timeLimit),
    parts: parts.length ? parts : [emptyPart()],
    panels: panels.length ? panels : [emptyPanel()],
  };
}
