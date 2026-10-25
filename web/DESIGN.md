# Web UI design (Stage 7)

The design for the Sheetsolver web UI: who it is for, its screens and
flows, its visual language, and what it needs from the API. The static
mockups in [`mockups/`](mockups/) show it with real data. Their
screenshots are in [`mockups/screenshots/`](mockups/screenshots/), at
desktop (1440 px) and tablet (820 px) widths, in light and dark.

This is the design phase only: no app code yet, and no Rust changes.
Scope follows `docs/SCOPE.md`. Every label and help text comes from
`docs/FIELDS.md`, and the mockup generator reads them straight from that
file.

| Mockup | Shows |
|---|---|
| [`editor.html`](mockups/editor.html) | Job editor: parts, panels, goal, saw settings, time limit |
| [`editor-errors.html`](mockups/editor-errors.html) | The same after a `422 invalid_instance`, with field-level problems |
| [`solving.html`](mockups/solving.html) | Search in progress, with better plans streaming in |
| [`plan-panel.html`](mockups/plan-panel.html) | Plan viewer, one panel: drawing, dimensions, cut order |
| [`plan-overview.html`](mockups/plan-overview.html) | Plan viewer, overview of every panel plus the parts summary |
| [`states.html`](mockups/states.html) | Empty, waiting, no-plan, cancelled, failed, expired, busy, connection lost |

**Data.** The mockups use `data/instances/generated__bin_packing_3_non_exact_kerf3_2.json`:
a 2440 × 1220 board, 3.2 mm kerf, 3 stages, 21 parts in 11 sizes, some
with grain and some with half-millimetre sizes. They draw its two oracle
plans from `data/golden/9b656a0d/`: `q64` stands in for "plan 1" during
the search, and `q2048`, which wastes less, is "plan 2", the final plan.
Geometry, sizes, quantities and grain are verbatim. Two things are made
up: the names ("Part 4" became "Side"), and the times at which plans
were found (the golden files have no timings). The suggested
`tests__bin_packing_3nvo` instance is a 20 × 20 mm toy with no offcuts,
so it can't show offcut dimensions or several panels.

**Regenerating.** `python3 web/mockups/build/build.py` writes the HTML
files, and `node web/mockups/build/screenshot.js` writes the PNGs (with
the preinstalled Playwright and Chromium). The HTML files are
self-contained: inline CSS and SVG, no scripts, no network.

---

## 1. Users and their tasks

**Who.** Cabinet makers, joiners, small furniture workshops and serious
hobbyists (`docs/AIMS.md`). They know their material and their saw, not
optimisation. They use the app in two places:

- **At a desk or laptop**, planning a job: typing in a cut list,
  checking it, finding a plan. Mouse and keyboard, a large screen,
  time to read.
- **At the saw, on a tablet**: following the plan cut by cut, often
  with dusty hands or gloves and from arm's length, sometimes in poor
  light. Touch only, glanced at between cuts.

**Main tasks**, in order of frequency:

1. Enter or adjust a job: parts (size, quantity, grain), the panels on
   hand, and the saw settings.
2. Find a plan, and judge it while the search runs: is it good enough,
   or should it run longer?
3. Read the plan: how many panels, how much waste, where each part
   goes.
4. Cut it at the saw: panel by panel, cut by cut, with the measurement
   for each cut.
5. Fix a job the server rejects, or one with no plan.

**What they must never need**: solver vocabulary (bin, item, profit,
cut thickness), unit conversions, or a hover to discover what a field
means.

---

## 2. Information architecture

Three steps, always visible in the top bar as a segmented progress
control: **Job → Search → Plan**. A step can be revisited (Edit job
from the plan) but not skipped ahead of the data (Plan is disabled
until a plan exists).

```
Job (editor)                    ─ Find cutting plan ─▶  Search (progress)
  Parts table + field guide                              time bar, live figures,
  Panels table + field guide                             best plan so far,
  Goal · Saw and cutting · Search                        plans found
         ▲                                                   │ Open plan / done
         └──────────── Edit job ──────────────  Plan ◀───────┘
                                                 ├─ Overview: every panel, parts summary
                                                 └─ Panel by panel: drawing + cut order
```

### URLs

A single-page app with hash routes. Reloading a page or sending its link
to the workshop tablet resumes it:

| Route | Screen |
|---|---|
| `#/` | Job editor (draft kept in the browser) |
| `#/jobs/{id}` | Search progress; turns into the plan once the job ends |
| `#/jobs/{id}/plan` | Plan overview |
| `#/jobs/{id}/plan/panels/{n}` | One panel, with `?cut={k}` for the current cut |

### Screens

**Job editor** (`editor.html`)
- *Parts*: an editable table with Name, Length, Width, Quantity, Keep
  grain direction, and Value only when the goal is "Best use of the
  panels I have". Below the table is a **field guide** with the help
  text of every column (see §5). *Add part* appends a row and focuses
  its Name. Rows are numbered, because 422 problems point at positions.
- *Panels*: the same pattern with Name, Length, Width, Quantity
  (placeholder "Unlimited") and Cost (placeholder "Panel area"). With
  the "Shortest length/width" goals, *Add panel size* is disabled and
  Quantity accepts only 1 or empty, explained inline (`FIELDS.md`
  constraint).
- *Goal*: four radio rows, then the full help text with the selected
  option's sentence emphasised.
- *Saw and cutting*: Kerf (mm), Cutting stages (2 / 3 / Unlimited),
  Last-stage trimming, and First cuts as segmented controls, each with
  its help text below. With Unlimited stages, Last-stage trimming is
  disabled, because its help text says it does not apply.
- *Search*: Time limit (s).
- *Find cutting plan*: in the right column on desktop, and in a sticky
  bottom bar on tablet. A summary ("21 parts · 1 panel size") sits next
  to it.

**Search** (`solving.html`)
- A status pill ("Searching", or "Waiting" when queued), a title, and
  one line explaining that the best plan so far is shown and replaced.
- A time bar: `elapsed` of `time_limit`, animated client-side between
  events.
- Four figures for the best plan so far: Panels used, Waste (with the
  improvement since plan 1), Parts cut (*n of total*), Plans found.
- *Best plan so far*: every panel drawn with part names, replaced by a
  short cross-fade when a `plan` event arrives (no fade with reduced
  motion).
- *Plans found*: a timeline, newest first, with the plan number, panels,
  waste, and time found. When `number` jumps (the stream skips plans),
  the timeline shows only the plans it received.
- Actions: **Stop and keep this plan** (`DELETE /jobs/{id}`) and
  **Open plan** (available as soon as there is a plan, so the
  woodworker can start reading it while the search continues; the
  viewer then shows a slim "still searching" bar and updates itself).
- When the job ends (`done`, or `cancelled` with a plan), the screen
  goes straight to the plan overview.

**Plan: overview** (`plan-overview.html`)
- A header with a status pill (Search finished / Stopped), "Plan *n*,
  found after *t* s of a *T* s search", stages and kerf.
- Four figures: Panels, Parts cut, Waste (% and m²), and Leftover
  (size, if any) or, for the "Shortest length/width" goals, the used
  length or width.
- A segmented control: **Overview | Panel by panel**.
- One card per pattern, with every part labelled (name and size where
  they fit) and offcuts sized. `× n` marks a pattern cut on several
  panels. Clicking a card opens it in the panel view.
- *Parts* table: every part of the job with its size, grain, how many
  are needed, how many are cut, and on which panels. With "Best use of
  the panels I have", parts not cut are listed first, marked "not cut".
- A legend.

**Plan: panel by panel** (`plan-panel.html`). This is the screen used
at the saw.
- A strip of panel thumbnails (the current one outlined) to switch
  panels.
- The drawing (§4.6): the panel at the largest size that fits,
  dimension rulers, every part and offcut labelled, and numbered cut
  badges. Toggles: *Sizes* and *Cut order*.
- **Cut order** list (right column on desktop, directly under the
  drawing on tablet): every cut of this panel in a workable order,
  grouped as *Panel into strips*, then *Strip A*, *Strip B*, …. Each
  cut shows:
  - **the size to cut off** in bold (`643.2 mm`), which is what you set
    the fence or stop to, and its direction (*along the length* /
    *across the length*, the words from `FIELDS.md`; *trim* for the
    last-stage trimming cut);
  - **what it produces** (`→ Strip A 2440 × 643.2`, `→ Side 1040.5 × 630`);
  - for the last cut of a piece, **what is left** (`leaves offcut 2440 × 27.2`).
- The current cut is highlighted in the list and on the drawing: the
  accent saw line, the piece produced outlined, earlier badges dimmed.
  Tapping a cut, or a badge on the drawing, makes it current. On a
  tablet, large ‹ › buttons and a swipe step through the cuts. The
  current cut is kept in the URL, so a tablet that sleeps comes back
  to the same cut.
- Below: *Parts on this panel* and *Offcuts* (largest first), so usable
  offcuts can be set aside.

### How the cut order is derived

The plan tree already holds everything (`docs/ARCHITECTURE.md`,
Solution). For each piece, the UI lists the cuts that separate its
children in tree order (one cut per child but the last), then recurses
into each child. So all the full-panel cuts come first, then each strip
is finished before the next. This is how a panel saw is worked: rip all
the strips, then crosscut each one. The size to cut off is the child's
extent along the cut axis. Two cases need care:

- A piece with a single child of the same size (a wrapper the tree uses
  to reach the next stage) needs no cut and is skipped.
- A final child thinner than the kerf (the golden data has a 0.4 mm
  one) is not an offcut a person can pick up: it is listed as "saw dust
  (0.4 mm)" and drawn without a label.

Pieces are named hierarchically (Strip A, Piece A1, Piece A1.1), so the
woodworker can pencil the name on each piece.

---

## 3. Interaction flows

**First use.** The editor opens with one empty part row, one panel row
named "Panel 1", and the `FIELDS.md` defaults (Use as few panels as
possible, kerf 3.2, 3 stages, trimming Allowed, first cuts Either, 10 s).
The empty Parts card says how to start (`states.html`, "New job").

**Entering parts quickly.**
- Tab moves across a row. Enter in the last cell of the last row adds a
  row.
- Numeric cells use `inputmode="decimal"`, so tablets show a number pad.
- A comma is accepted as the decimal mark (Polish and German
  keyboards) and stored as a point.
- Removing a row offers *Undo* in a toast instead of asking for
  confirmation.

**Validation, before and after submitting.**
1. *While typing* (client-side, on blur), the UI checks what can be
   checked without the solver: a number, at most one decimal, ≥ 0.1 mm,
   quantity ≥ 1, time limit 1–600, names present. Messages use the
   same wording as the API's, beside the field.
2. *On submit*, the server is the authority. A `422` comes back with
   `problems[]`. Each `field` path (`instance.parts[1].width`) is mapped
   to its row and column: the cell gets the error style, the message
   appears under the row, and a summary banner at the top lists every
   problem as a link that scrolls to and focuses the field
   (`editor-errors.html`). A problem about a whole list
   (`instance.parts`) attaches to the card heading. One with
   `field: null` stays in the banner only.
3. Problems clear as soon as their field is edited. No resubmission is
   needed to see the error disappear.

The UI sends rows **in display order** and **never drops a row
silently**. A wholly empty trailing row is not sent. Any other row is
sent as is, so positions in `problems` always match row numbers.

**Searching.**
1. `POST /jobs` → `201`, then route to `#/jobs/{id}`.
2. Open `EventSource(/jobs/{id}/events)`:
   - each `plan` event replaces the best plan and appends to the
     timeline;
   - each `state` event updates the pill, `elapsed` and `plans_found`;
   - a final `state` closes the source (`API.md`: otherwise it
     reconnects forever).
3. Between events, the time bar advances from the last `elapsed` with
   the local clock. It stops at `time_limit`.
4. A dropped connection shows "Reconnecting…". `EventSource` reconnects
   by itself, and the replayed `plan` and `state` events bring the
   screen up to date.
5. Stop → `DELETE` → the button turns into "Stopping…" until the
   `cancelled` state arrives.

**Opening a link later.** `GET /jobs/{id}`:
- `404` means expired: offer *Search again* from the draft saved in
  this browser (see the open question in §9).
- `best: null` in a final state shows the no-plan states.

**At the saw.** Open *Panel by panel* → panel 1, cut 1 → step with › or
by tapping. Finished panels can be ticked off in the thumbnail strip
(stored locally, per job).

---

## 4. Visual language

Apple-inspired: quiet neutral surfaces, one accent, type and spacing
doing the work. Depth comes from soft shadows and a translucent top
bar, never from borders everywhere. All tokens live as CSS custom
properties on `:root` and are redefined for dark mode, both with
`prefers-color-scheme` and with an explicit `data-theme` override
(`mockups/build/styles.css` is the starting point for the app's
stylesheet).

### 4.1 Typography

System font stack, so Apple devices get SF Pro, Windows gets Segoe UI,
and others Inter or Roboto:

```
-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display",
"Segoe UI Variable Text", "Segoe UI", Inter, Roboto, "Helvetica Neue", Arial, sans-serif
```

All figures and measurements use `font-variant-numeric: tabular-nums`,
so columns of millimetres line up.

| Token | Desktop | Tablet / touch | Use |
|---|---|---|---|
| `--t-large` | 600 32/1.15 | same | Page title |
| `--t-figure` | 600 28/1.1 | same | Metric values |
| `--t-title` | 600 22/1.25 | same | Card titles |
| `--t-headline` | 600 17/1.3 | same | Sub-headings, small card titles |
| `--t-body` | 400 15/1.45 | 400 16/1.45 | Body, inputs (≥ 16 px on touch avoids iOS zoom) |
| `--t-callout` | 500 15/1.4 | same | Buttons, options, cut sizes |
| `--t-help` | 400 13/1.5 | 400 14/1.5 | Help text, secondary lines |
| `--t-label` | 600 13/1.3 | same | Field labels, group headers |
| `--t-caption` | 500 12/1.3 | same | Table headers, chips, legends |

Titles use a small negative letter-spacing (−0.01 to −0.02 em), as SF
Display does.

### 4.2 Colour tokens

One accent, blue, for the primary action, the selection, focus, the
current cut, and parts in the drawing. Red and green are reserved for
status (errors, "done", improvements), not decoration.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#f5f5f7` | `#000000` | Page background |
| `--surface` | `#ffffff` | `#1c1c1e` | Cards |
| `--surface-2` | `#fafafc` | `#232325` | Field guide, inset areas |
| `--fill` / `--fill-2` | grey 12% / 7% | grey 24% / 14% | Inputs, segmented track, neutral buttons |
| `--text` | `#1d1d1f` | `#f5f5f7` | Primary text |
| `--text-2` | `#6e6e73` | `#a1a1a6` | Help text, secondary (≥ 4.5:1 on its surfaces) |
| `--text-3` | `#8e8e93` | `#7c7c80` | Placeholders, row numbers (non-essential only) |
| `--separator` | 13% grey | 55% grey | Hairlines (0.5 px) |
| `--accent` | `#0071e3` | `#2997ff` | The accent |
| `--accent-tint` | accent 10% | accent 16% | Selected rows and options, tinted buttons |
| `--danger` | `#d70015` | `#ff453a` | Field errors, failed state |
| `--success` | `#1f7a35` | `#30d158` | Done, improvements |

Cutting-plan colours (`--plan-*`):

| Token | Light | Dark | Meaning |
|---|---|---|---|
| `--plan-part` / `-stroke` | `#e6f0fc` / `#7aa9e0` | `#18324f` / `#3a6ea8` | Part (accent family) |
| `--plan-grain` | accent 9% | light blue 8% | Wavy grain lines on parts with *Keep grain direction* |
| `--plan-offcut` + `--plan-hatch` | `#ededf0` + `#d4d4d9` | `#2a2a2d` + `#3c3c40` | Offcut: neutral, 45° hatch |
| `--plan-leftover` + stroke | near white + dashed grey | `#222224` + dashed | Reusable leftover |
| `--plan-kerf` | `#8e8e93` | `#0b0b0c` | Saw cut: the panel shows through the gaps between pieces |
| `--plan-badge` | `#1d1d1f` | `#f5f5f7` | Cut-number badges; the current one uses the accent |

Part, offcut and leftover differ in **pattern and shape** (solid, hatch,
dashed outline) as well as colour, so the drawing reads in greyscale,
on a cheap printout, and for colour-blind users.

### 4.3 Spacing, radii, depth

- Spacing on a 4 px grid: `4 8 12 16 20 24 32 40 48 64`. Cards have
  24 px padding; columns and cards have a 20–24 px gap; pages have
  32–40 px gutters on desktop and 20 px on tablet.
- Radii: `6` (small), `8` (inputs, segmented), `12` (option rows,
  banners), `18` (cards), pill (buttons, chips, status).
- Depth: `--shadow-1` (a 0.5 px ring and a 1 px soft shadow) on cards;
  `--shadow-2` for popovers and toasts. In dark mode the shadows become
  faint light rings. The top bar is translucent with a background blur.
- Controls are 36 px high on desktop and **44 px on tablets and touch
  screens** (`pointer: coarse` or narrower than 1100 px), the minimum
  touch target.

### 4.4 Components

| Component | Notes |
|---|---|
| Top bar | Brand, step control (Job · Search · Plan), contextual actions |
| Card | Surface, radius 18, title (`--t-title`) and optional meta on the right |
| Button | Primary (filled accent), Tinted (accent tint), Neutral (fill), Plain (text); pill-shaped, 36/44/52 px |
| Text input | Filled, no border at rest; accent border and 3 px focus ring; unit suffix inside (`mm`, `s`) |
| Editable table | Rows separated by hairlines; numbered rows; a remove button per row; an error row under a faulty row |
| Field guide | Inset panel below a table with every column's label and help text; the focused column is accented |
| Segmented control | 2–3 short options (stages, trimming, first cuts, overview/panel) |
| Option list | Radio rows for options with longer names (Goal) |
| Checkbox | 22 px rounded square, centred in its cell (Keep grain direction) |
| Banner | Error summary with links to the fields |
| Metric tile | Label, large tabular figure, one-line note |
| Status pill | Searching (pulsing accent dot), Waiting, Search finished, Stopped, Failed |
| Progress bar | 6 px, accent fill |
| Timeline | Plans found, newest first |
| Panel thumbnail | Small plan drawing, name, parts count; selected outline |
| Plan drawing | §4.6 |
| Cut list | Grouped, numbered steps; current, done and upcoming styles |
| Data table | Parts and offcuts summaries; tabular figures, right-aligned counts |
| Empty / state card | Line illustration, title, one paragraph, actions |

### 4.5 Motion

Short and functional only:
- a 150–200 ms cross-fade when a better plan replaces the drawing;
- the pulsing dot on "Searching";
- the time bar advancing.

All of it is disabled under `prefers-reduced-motion`.

### 4.6 The cutting-plan drawing

- **Orientation**: length horizontal, width vertical, as `FIELDS.md`
  requires. y = 0 is the **bottom** edge, as in upstream, so the later
  "bottom/top trim" fields will match the drawing.
- **Saw cuts** are not drawn as lines: the panel is filled with the
  kerf colour and every piece is drawn on top, so each kerf shows as a
  gap of its true width. With 3.2 mm on a 2440 mm panel that is about
  1 px at desktop size, enough to see every cut.
- **Labels**: every part shows its name over its size as drawn
  (horizontal × vertical, e.g. `320.5 × 320`), offcuts show their size,
  and the leftover shows "Leftover" and its size. Labels are fitted, in
  this order:
  1. name and size;
  2. the same turned 90° when the piece is taller than it is wide;
  3. size only;
  4. nothing.
  An unlabelled piece can still be tapped for a popover, and it is in
  the *Parts on this panel* and *Offcuts* lists.
- **Turned parts** carry a small rotate glyph in the corner. **Grain**
  parts carry faint wavy lines along their length.
- **Rulers** (panel view): panel length on top, panel width on the
  right, and the first-stage strip sizes along the edge the strips are
  measured from. The cut list's first group can be checked against the
  ruler at a glance.
- **Cut badges**: a numbered dot at the start of each saw line. Badges
  are nudged along their line when they would overlap. The current cut
  draws its saw line in the accent and outlines the piece it produces.
- **Sizing**: the drawing is laid out at the pixel width it will be
  shown at, so 11–12 px labels stay 11–12 px. On a tablet the
  drawing is scaled about 0.8× and can be pinch-zoomed. The overview
  and progress thumbnails use smaller label sizes, or names only.

---

## 5. Help text: always visible

Stage 7's exit criterion is that no input lacks help text, and that the
help text is visible without hovering, so it works on a touch screen in
a workshop.

- **Single fields** (Goal, kerf, stages, trimming, first cuts, time
  limit) show the **full `FIELDS.md` help text directly under the
  control**, always.
- **Option fields** (Goal, Last-stage trimming, First cuts): the option
  names are the **bold** words of the help text. The sentence about the
  selected option (from its bold name to the next one) is shown in the
  primary colour, with the name in the accent; the rest stays
  secondary. The words never change: this only directs the eye.
- **Table columns** (parts, panels): repeating a 400-character help
  text in every row would bury the table. Instead, each table has a
  **field guide** directly under it, listing every column's label and
  full help text. Focusing a cell accents its column header and its
  guide entry (Length in `editor.html`). The input's
  `aria-describedby` points at the guide entry, so screen readers read
  the help with the field. On narrow screens the guide is one column.
- **Columns that don't apply** (Value when the goal isn't "Best use of
  the panels I have") are hidden from the table. Their guide entry
  stays, dimmed, with a chip saying when it applies.
- **Source.** Help text comes from the `description` of each property
  in `openapi.json` (§7). Its markup is rendered: `**bold**` becomes
  `<strong>` and `*italic*` becomes `<em>` (FIELDS.md uses `*Value*`).
  Nothing else is interpreted. Labels must come from the same place
  (see §8, missing `title`).

---

## 6. Accessibility

- **Contrast**: body and help text ≥ 4.5:1 in both themes. `--text-3`
  is used only for non-essential text (placeholders, row numbers). Plan
  labels on part fills are ≥ 7:1.
- **Not colour alone**: errors have an icon and a message; parts,
  offcuts and leftover differ by pattern; the current cut has a number
  and a list highlight as well as the accent.
- **Keyboard**: everything is reachable in DOM order. Tables support
  Tab across a row and Enter on the last cell to add a row. Segmented
  controls and option lists are radio groups with arrow keys. In the
  cut list, ↑ and ↓ move the current cut.
- **Screen readers**:
  - each input has an accessible name ("Length, row 3") and its help
    as `aria-describedby`;
  - the 422 banner is `role="alert"`, and each field error is linked
    by `aria-describedby` and `aria-invalid`;
  - the progress bar is a `progressbar`;
  - new plans are announced in a polite live region ("Plan 2: 3
    panels, 18.6% waste");
  - each drawing is `role="img"` with a title ("Panel 1 of 3: Birch
    ply 18 mm, 2440 by 1220 mm") and the cut list as its text
    equivalent.
- **Touch**: 44 px targets; no hover-only affordances; the cut-order
  toggles and stepping buttons are thumb-sized.
- **Zoom and text size**: layouts reflow to one column; the drawing
  scales; nothing relies on fixed pixel heights except the drawing's
  aspect ratio.
- **Reduced motion** is respected (§4.5).
- **Language**: `lang="en"`. Copy uses `FIELDS.md` terms only: panel,
  part, kerf, grain, offcut, leftover, plan.

---

## 7. Proposed front-end stack

| Concern | Choice | Why |
|---|---|---|
| Build | **Vite** + **TypeScript** (strict) | Instant dev server, plain static output (`web/dist`) that any server or the `api` crate can host; TS catches contract drift at compile time. |
| UI framework | **Svelte 5** (plain Vite plugin, no SvelteKit) | Templates read like HTML and CSS, which suits an owner who mostly writes Rust; compiled, so a small bundle with no virtual DOM, good on workshop tablets; scoped CSS fits the token approach; SVG is first-class in templates. **Preact + signals** is the alternative if JSX is preferred. Either way, no component library: the components in §4.4 are small and need to look like this design. |
| API types | **openapi-typescript** generates `src/api/schema.d.ts` from `openapi.json` | One source of truth: Rust types → utoipa → OpenAPI → TS. A Rust type change that breaks the UI fails `npm run check` in the same change (`ARCHITECTURE.md`). |
| API calls | **openapi-fetch** (a tiny typed `fetch`) for `POST/GET/DELETE /jobs` | Paths, bodies and responses typed from the schema, no hand-written client. |
| Events | A ~40-line typed wrapper around the browser's `EventSource` | OpenAPI cannot describe SSE well; the wrapper uses the generated `PlanReport` and `StateEvent` schemas (see §8.1) and closes on final states. |
| Help text and labels | `scripts/gen-fields.ts` reads `openapi.json` and writes `src/api/fields.generated.ts`: `{ "instance.parts[].length": { label, help, default, minimum, … } }` | Types carry descriptions only as comments; the UI needs them at run time. Every input component takes a `field` key typed as a union of the generated keys, so **an input without help text does not compile**. |
| State | Svelte runes; the draft job is kept in `localStorage` (wrapped in try/catch) | No server persistence (Open in `SCOPE.md`); losing the draft on reload would hurt. |
| Plan geometry | Pure TS module `src/plan/` (cut order, label fitting, statistics) with unit tests | Mirrors the mockup generator's logic; easy to test against golden files. |
| Tests | **Vitest** (logic), **Playwright** (end-to-end against `cargo run -p api`) | The Stage 7 exit criterion "enter instance → solve → see a valid plan" is one Playwright test; another fails if any input lacks visible help text. |
| Lint and format | `svelte-check`, ESLint, Prettier | Matches the Rust side's fmt and clippy gate. |

Proposed layout:

```
web/
  DESIGN.md
  mockups/              # this design (static)
  package.json  vite.config.ts  tsconfig.json
  scripts/gen-fields.ts
  src/
    api/                # schema.d.ts, fields.generated.ts (generated), client.ts, events.ts
    plan/               # cut order, label fitting, stats (+ tests)
    components/         # Button, Field, Segmented, EditTable, FieldGuide, PlanSvg, CutList, …
    routes/             # Editor, Search, PlanOverview, PlanPanel, states
    styles/tokens.css   # from mockups/build/styles.css
  e2e/                  # Playwright
```

`openapi.json` is fetched from a running API or written by a small
`cargo run -p api -- --print-openapi` (see §8.1) into `web/openapi.json`
and checked in, so the UI builds without a running server and the diff
shows contract changes.

---

## 8. API requirements from the UI

What the UI uses from `docs/API.md`, and what is missing or awkward.
`docs/API.md` is being implemented in parallel and may change, so these
are requests, not changes made here.

### What the UI uses

| Need | Endpoint / field | Status |
|---|---|---|
| Submit a job | `POST /jobs` with `instance`, `time_limit`; `201` + job status | ✅ fits |
| Follow the search live | `GET /jobs/{id}/events`: `plan` then `state` on connect, final `state` ends it | ✅ fits; the replay on connect makes reload/resume trivial |
| Resume from a link | `GET /jobs/{id}` | ✅ |
| Stop and keep the best plan | `DELETE /jobs/{id}` → `202` | ✅ |
| Figures for the summary | `best.metrics` (computed by the checker) | ✅, but per-panel figures are missing (see §8.2) |
| The drawing and cut order | `best.plan.patterns[].root` tree | ✅ mostly; see §8.3 |
| Field-level errors | `422 invalid_instance` with `problems[].field` paths and woodworker-worded messages | ✅ exactly what the UI needs |
| Busy, expired | `503 busy`, `404 not_found` | ✅ |
| Help text | `openapi.json` `description` per property | ✅, but labels, option names and defaults are missing (§8.1) |
| Dev server on another origin | CORS `*`, `Location` exposed | ✅ |

### 8.1 Missing for the generated client and the "no input without help" rule

1. **A UI label per field.** `openapi.json` carries the help text but
   not the `FIELDS.md` *UI label* ("Saw blade thickness (kerf)",
   "Keep grain direction"). Without it, the UI hard-codes labels and
   "UI wording matches FIELDS.md" (a Stage 7 exit criterion) can drift.
   **Request:** put the label in each property's `title` (utoipa
   supports `title`), taken from FIELDS.md like the description.
2. **Option names per enum value.** `objective`, `cut_type`,
   `first_cuts` and `stages` are enums whose user-facing names
   ("Use as few panels as possible", "Allowed", "Along the length")
   exist only as bold words in the help text. The mockups take them
   from the bold words in order, which is fragile: `stages` has bold
   "Stage 1/2/3" that are not its options.
   **Request:** describe each value, e.g. `oneOf: [{ const: "non_exact",
   title: "Allowed" }, …]` or an `x-enum-labels` list, generated from
   one table.
3. **Defaults and bounds in the schema**: `default` (kerf 3.2, stages 3,
   cut type `non_exact`, first cuts `either`, objective `bin_packing`,
   part quantity 1, time limit 10) and `minimum`/`maximum` (lengths
   ≥ 0.1, quantity ≥ 1, time limit 1–600), plus `multipleOf: 0.1` for
   lengths. The UI then fills a new job and validates as you type from
   the contract instead of copies.
4. **Named schemas for the event payloads.** SSE event data is not
   described by OpenAPI paths. **Request:** export `PlanReport` (the
   `best` object) and `StateEvent` as named `components.schemas`, so
   the TS types are generated.
5. **`stages` as `2 | 3 | "unlimited"`** mixes number and string. It
   works, but it generates an awkward union and needs `oneOf` in the
   schema. Acceptable if documented; `"2" | "3" | "unlimited"` would
   be simpler. Owner's call; not blocking.
6. **Help-text markup.** API.md says descriptions use `**bold**`, but
   FIELDS.md also uses `*Value*` (italic). Either document both, or
   drop the italic. The UI renders both and nothing else.
7. **Getting `openapi.json` without a running server**, e.g.
   `cargo run -p api -- --print-openapi`, for the UI build and CI.

### 8.2 Missing figures

8. **Per-pattern metrics.** The viewer shows waste, part count and
   offcuts per panel. These can be computed from the tree (the mockups
   do), but the API's promise that "the UI need not compute" metrics
   only holds for whole-plan figures. **Request** (nice to have):
   `metrics` per pattern (`parts_cut`, `waste_area`, `waste_fraction`,
   `leftover_area`). Otherwise the UI computes them from the tree.
9. **Why a job is done.** `done` covers both "the search was complete"
   (nothing better exists for this search) and "the time limit ran
   out". These deserve different advice: "this is the best this search
   can do" versus "a longer search may save material".
   **Request:** `ended_by: "complete" | "time_limit"` (or a boolean) on
   the status and the final `state` event.
10. **Queue position** for `queued` jobs (optional), to say "2 jobs
    ahead of yours".
11. **`Retry-After`** on `503 busy`, so the UI can count down instead of
    guessing.

### 8.3 Plan tree: things to pin down

12. **Children order.** The cut order relies on children being listed
    in position order along the cut axis (as in every golden file).
    **Request:** state this in ARCHITECTURE.md and have the checker
    enforce it.
13. **Cut direction per piece.** For 2 and 3 stages, the direction
    follows from `first_cuts` and the stage. With **unlimited** stages,
    cuts "may go either way at every stage", so the UI must infer the
    direction from the children's geometry (same `y` → across). That
    works, but is implicit. **Request:** a `cuts: "along_length" |
    "across_length"` field on every `piece` with children.
14. **Stage numbers beyond `stages`.** The golden 3-stage plans contain
    `stage: 4` nodes: the last-stage trimming cut (e.g. a `Top`
    770 × 370 trimmed from a 1040.5 × 370 piece). ARCHITECTURE.md
    doesn't mention this. The UI labels these cuts "trim". Please
    document that trimming appears as stage `stages + 1`.
15. **Wrapper pieces.** A `piece` whose single child has the same size
    needs no cut; the UI skips it. Worth one sentence in
    ARCHITECTURE.md so other clients do too.
16. **Sub-kerf offcuts.** Offcuts thinner than the kerf (0.4 mm in the
    golden data, from "at most one kerf left at the end") are counted
    in `waste_area`, which is right. The UI calls them saw dust and
    doesn't label them. Fine; just confirm they are intended.
17. **Repeated patterns.** `repeat: n` is drawn once with "× n". The cut
    order is the same for each copy. Fine as is.

### 8.4 Jobs and links

18. **The instance is not in the job status.** A job link opened on
    another device (desk → workshop tablet) can draw the plan, but the
    *Parts* summary (needed vs cut, grain, parts not cut for "Best use")
    and *Edit job* need the instance, which only the submitting browser
    has. **Request:** include `instance` in `GET /jobs/{id}` (or
    `GET /jobs/{id}/instance`). Without it, the plan viewer on another
    device shows only what the tree holds.
19. **Validation without solving** (optional): `POST /instances/validate`
    returning the same `problems`, so the UI can show server-side
    problems (duplicate names, open-dimension panel rules) before
    submitting. Not needed for v1: the client-side checks and the 422
    on submit cover it.
20. **Problem paths for unknown positions.** The UI maps
    `instance.parts[3].length` to row 4. Please guarantee that positions
    refer to the submitted arrays (they do today) and that `field` uses
    exactly the JSON property names.

---

## 9. Open questions for the owner

These are proposals; per AGENTS.md they stay unimplemented until
decided.

1. **Local draft and per-job notes** (the job kept in `localStorage`,
   ticked-off panels at the saw). This is client-side only, not the
   server persistence that is Open in SCOPE.md. OK to include?
2. **Printing** a plan with the browser (a print stylesheet: one panel
   per page with its cut list). Export is Open; is a print stylesheet
   in scope for v1?
3. **Dimension format**: plain `1040.5` (as typed), or with thousands
   separators (`1 040.5`)? The mockups use plain figures to match
   input.
4. **Part labels** in the drawing show the size as drawn (horizontal ×
   vertical). An alternative is to always show the part's own length ×
   width, as entered, with the turned glyph. The first matches the
   rulers and the saw; the second matches the cut list the user typed.
