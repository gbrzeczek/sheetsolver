// A per-browser convenience: the draft job.
// Storage can be missing or throw (private windows, blocked site data), so
// every access is guarded and the app works without it.
import { adoptKeys, type Draft } from "./draft";

const DRAFT = "sheetsolver.draft.v1";

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
  return draft && Array.isArray(draft.parts) && Array.isArray(draft.panels) ? adoptKeys(draft) : null;
}

export function saveDraft(draft: Draft): void {
  write(DRAFT, draft);
}
