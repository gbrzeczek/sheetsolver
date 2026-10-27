# Sheetsolver web UI

The UI of Stages 7 and 8: a Svelte 5 + TypeScript app built with Vite. The API
serves the built files (`--web-dir`), so in production both share one
origin. Design, screens and decisions: [DESIGN.md](DESIGN.md).

## Commands

```bash
npm ci                 # dependencies (exact versions, see .npmrc)
npm run openapi        # refresh openapi.json from the Rust types (cargo)
npm run dev            # dev server on :5173; proxies the API on :8080
                       #   (run `cargo run -p api` next to it)
npm run check          # svelte-check: types and Svelte warnings
npm test               # unit tests (Vitest), incl. against data/golden
npm run build          # web/dist
npm run e2e            # build, start the API on :8092, run Playwright
                       #   (the screen budget needs the Inter font:
                       #   `sudo apt-get install fonts-inter` on Ubuntu)
npm run screens        # screenshots of every screen at 1440, 820, 360 px
                       #   into mockups/screenshots (DESIGN.md §10)
```

`npm run gen` (run by the others) writes the generated files from
`openapi.json`: `src/lib/api/schema.d.ts` (types) and
`src/lib/api/fields.generated.ts` (labels, short and full help,
defaults, bounds, option names). Both are git-ignored; `openapi.json` is checked in and CI
fails if it differs from what the Rust types print.

## Layout

| Path | What |
|---|---|
| `src/lib/api/` | Typed client (`client.ts`), live events (`events.ts`), field help (`fields.ts`) |
| `src/lib/job/` | The draft job, generated names, local checks, 422 mapping, the session |
| `src/lib/plan/` | Cut order, per-panel figures, label fitting, drawing geometry |
| `src/lib/components/` | Inputs with help buttons, tables, the panel drawing, cut list |
| `src/routes/` | Job setup, Cutting plan |
| `src/styles/app.css` | Tokens and components, from `mockups/build/styles.css` |
| `e2e/` | Playwright tests against the real API; `screens.ts` measures the screen budget |
| `mockups/` | The static design mockups and their screenshots |

Rules that matter here:

- Every input names a field key (`"Saw.kerf"`), from which it gets its
  label and help text; an input without one does not compile.
- Wording comes from `docs/FIELDS.md`, via `openapi.json`, for every
  field, the job name included: the short help first, the full text
  under *More*.
- Each screen keeps within the budget of `DESIGN.md` §10; a new field
  goes into a collapsed section with a summary.
- Lengths are millimetres with at most one decimal, as typed; a comma is
  accepted as the decimal mark.
