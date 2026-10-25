// Labels and help text for every input: from openapi.json (generated), and
// for the UI-only fields from docs/FIELDS.md, "UI-only fields" (copied
// here; fields.test.ts keeps the copy exact).
import { API_FIELDS, type ApiField, type ApiFieldKey } from "./fields.generated";

export const UI_FIELDS = {
  "Ui.job_name": {
    label: "Job name",
    help: 'Your name for this job, shown at the top of the job and its cutting plan (e.g. "Kitchen units"). Optional.',
  },
} as const satisfies Record<string, ApiField>;

/** Every input field: an input component must name one, so it has help text. */
export type FieldKey = ApiFieldKey | keyof typeof UI_FIELDS;

export function field(key: FieldKey): ApiField {
  return key in UI_FIELDS ? UI_FIELDS[key as keyof typeof UI_FIELDS] : API_FIELDS[key as ApiFieldKey];
}

/** The option names of a field with options, in order. */
export function options(key: FieldKey): readonly { value: string | number; title: string }[] {
  return field(key).options ?? [];
}

/** The name of one option ("Use as few panels as possible"). */
export function optionTitle(key: FieldKey, value: string | number): string {
  return options(key).find((o) => o.value === value)?.title ?? String(value);
}

function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

/**
 * Help text as HTML: `**bold**` and `*italic*` are the only markup
 * (docs/API.md). With `current`, the sentence(s) about that option, from its
 * bold name to the next bold name, are wrapped in `<span class="current">`.
 * The words are never changed.
 */
export function helpHtml(text: string, current?: string): string {
  const parts = text.split(/(\*\*.+?\*\*)/);
  let out = "";
  let open = false;
  for (const part of parts) {
    if (part.startsWith("**") && part.endsWith("**") && part.length > 4) {
      const word = part.slice(2, -2);
      if (open) {
        out += "</span>";
        open = false;
      }
      if (current !== undefined && word === current) {
        out += '<span class="current">';
        open = true;
      }
      out += `<strong>${escapeHtml(word)}</strong>`;
    } else {
      out += escapeHtml(part).replace(/\*(.+?)\*/g, "<em>$1</em>");
    }
  }
  return open ? `${out}</span>` : out;
}
