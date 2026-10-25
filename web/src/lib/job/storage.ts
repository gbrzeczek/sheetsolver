// Per-browser conveniences: the draft job and the names of submitted jobs.
// Storage can be missing or throw (private windows, blocked site data), so
// every access is guarded and the app works without it.
import type { Draft } from "./draft";

const DRAFT = "sheetsolver.draft.v1";
const NAMES = "sheetsolver.job-names.v1";

function read<T>(key: string): T | null {
  try {
    const text = localStorage.getItem(key);
    return text ? (JSON.parse(text) as T) : null;
  } catch {
    return null;
  }
}

function write(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Not saved: fine, it is only a convenience.
  }
}

export function loadDraft(): Draft | null {
  const draft = read<Draft>(DRAFT);
  return draft && Array.isArray(draft.parts) && Array.isArray(draft.panels) ? draft : null;
}

export function saveDraft(draft: Draft): void {
  write(DRAFT, draft);
}

/** The job name, which the API does not keep yet (web/DESIGN.md §8, #21). */
export function jobName(id: string): string {
  return read<Record<string, string>>(NAMES)?.[id] ?? "";
}

export function rememberJobName(id: string, name: string): void {
  const names = read<Record<string, string>>(NAMES) ?? {};
  names[id] = name;
  // Keep the map small: jobs expire on the server after an hour anyway.
  const recent = Object.entries(names).slice(-50);
  write(NAMES, Object.fromEntries(recent));
}
