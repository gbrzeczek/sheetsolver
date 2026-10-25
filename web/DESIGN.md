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

**Design direction.** The look follows the owner's design handoff
(2026-10-25). It offered three concepts: *Workbench*, *Guided setup* and
*Job sheet*. The handoff recommends combining them, and this design does:

- **Guided setup's input hierarchy** for the job: parts first, then
  material, then the settings, with the advanced ones collapsed;
- **Workbench's plan view**: compact panel navigation, a large selected
  panel, and details on selection.

Where the handoff's sample wording differs from `FIELDS.md` ("sheets",
"Optimise for", "Stock limit"), the UI keeps the FIELDS.md terms (panel,
Goal, Quantity), as AGENTS.md requires. The handoff's layouts were
schematic; the drawings here are real plans, to scale.

| Mockup | Shows |
|---|---|
| [`editor.html`](mockups/editor.html) | Job setup: parts, panels, plan settings; one column help popover and the kerf help open |
| [`editor-errors.html`](mockups/editor-errors.html) | The same after a `422 invalid_instance`, with field-level problems and the advanced settings expanded |
| [`solving.html`](mockups/solving.html) | Cutting plan while the search runs, with better plans streaming in |
| [`plan-panel.html`](mockups/plan-panel.html) | Cutting plan, one panel: drawing, dimensions, current cut, cut order |
| [`plan-overview.html`](mockups/plan-overview.html) | Cutting plan, all panels and the parts checklist |
| [`states.html`](mockups/states.html) | Empty, waiting, no-plan, cancelled, failed, expired, busy, connection lost |

**Data.** The mockups use `data/instances/generated__bin_packing_3_non_exact_kerf3_2.json`:
a 2440 × 1220 board, 3.2 mm kerf, 3 stages, 21 parts in 11 sizes, some
with grain and some with half-millimetre sizes. They draw its two oracle
plans from `data/golden/9b656a0d/`: `q64` stands in for "plan 1" during
the search, and `q2048`, which wastes less, is "plan 2", the final plan.
Geometry, sizes, quantities and grain are verbatim. Two things are made
up: the names ("Part 4" became "Side"), and the times at which plans
were found (the golden files have no timings).

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
  checking it, finding a plan. Mouse and keyboard, a large screen.
- **At the saw, on a tablet**: following the plan cut by cut, often
  with dusty hands or gloves and from arm's length. Touch only, glanced
  at between cuts.

**What they need to know, in order** (this sets the hierarchy of every
screen):

1. Did everything fit, and how many panels does it take?
2. How much is wasted, and is there a piece worth keeping?
3. Where does each part go?
4. What is the next cut, and where do I set the fence?

**Main tasks**, in order of frequency:

1. Enter or adjust a job: parts (size, quantity, grain), the panels on
   hand, and the saw settings.
2. Find a plan, and judge it while the search runs.
3. Read the plan.
4. Cut it at the saw: panel by panel, cut by cut.
5. Fix a job the server rejects, or one with no plan.

**What they must never need**: solver vocabulary (bin, item, profit,
cut thickness), unit conversions, or a hover to discover what a field
means.

---

## 2. Information architecture

Two tabs under the app bar, always visible: **Job setup** and
**Cutting plan**. The search is not a separate step but a state of the
Cutting plan tab: while it runs, the tab shows a live dot and the best
plan so far. *Edit job* returns to Job setup; the last plan stays
available until the job is solved again.

```
App bar:  Sheetsolver · Kitchen units (job name, or "Untitled job")
Tabs:     Job setup | Cutting plan •

Job setup                          Cutting plan
  Parts to make (table)              while searching: progress, live figures, panels so far
  Panels you have (table)            when done: headline, figures, status
  Plan settings (aside)                ├─ All panels: every panel + parts checklist
    Goal, kerf, More cutting settings  └─ Panel n: drawing, current cut, cut order
  Find cutting plan →
```

### URLs

A single-page app with hash routes. Reloading a page or sending its link
to the workshop tablet resumes it:

| Route | Screen |
|---|---|
| `#/` | Job setup (draft kept in the browser) |
| `#/jobs/{id}` | Cutting plan: progress while searching, then all panels |
| `#/jobs/{id}/panels/{n}` | One panel, with `?cut={k}` for the current cut |

### Screens

**Job setup** (`editor.html`). The page asks "What are you cutting?" and
fills a centred column about 1080 px wide.
- *Job name* (optional): one field above the parts, placeholder
  "Untitled job". It names the job in the app bar and above the cutting
  plan. Until the API carries it (§8, #21), it is kept with the draft
  in the browser.
- **Names are optional** for parts and panels. An empty Name cell shows
  its generated name greyed ("Part 9", "Panel 1": the FIELDS.md default,
  by row number, skipping names already used), and the UI sends that
  name with the job. Someone who only wants to type sizes can tab
  straight past the Name column. Row 9 in `editor.html` is left
  unnamed and appears as "Part 9" in the plan mockups.
- *Parts to make*: a compact table with Name, Length, Width, Quantity
  and Keep grain direction (headed "Keep grain ↔"). Value appears only
  when the goal is "Best use of the panels I have". Numbers are
  right-aligned. Each column header has a **(?) help button**
  (§5). Rows are numbered, because 422 problems point at positions.
  *+ Add part* appends a row and focuses its Name.
- *Panels you have*: the same table with Name, Length, Width, Quantity
  (placeholder "Unlimited") and Cost (placeholder "Area"). With the
  "Shortest length/width" goals, *Add panel size* is disabled and
  Quantity accepts only 1 or empty, explained inline.
- *Plan settings* (an aside, separated by a hairline):
  - **Goal** (a select) and **Saw blade thickness (kerf)**, always
    visible, because they change every plan;
  - **More cutting settings**, collapsed, with a one-line summary of
    their values ("3 stages · trimming allowed · along the length
    first · 10 s"): Cutting stages, Last-stage trimming, First cuts
    (segmented rows) and Time limit. With Unlimited stages, Last-stage
    trimming is disabled, as its help text says it does not apply;
  - a short summary (parts, panels) and **Find cutting plan →**, which
    sits in a sticky bottom bar on tablets.

**Cutting plan, searching** (`solving.html`)
- Eyebrow "Cutting plan · searching"; headline "All 21 parts fit so far"
  (or "No plan yet" before the first plan); one line saying the best
  plan so far is replaced as soon as a better one is found.
- A thin time bar: `elapsed` of `time_limit`, animated between events.
  When queued: "Waiting for a free solver · 2 jobs ahead of yours"
  (`queue_position`).
- The figures row (below) for the best plan so far.
- Every panel of the best plan so far, with part names. A better plan
  replaces them with a short cross-fade (none under reduced motion).
- *Plans found*: newest first, with panels, waste and the improvement.
  When `number` jumps, the list shows only the plans received.
- **Stop and keep this plan** (`DELETE /jobs/{id}`), and **Open plan**,
  which shows the panel view while the search continues and updates
  itself.

**Cutting plan, done** (`plan-overview.html`, `plan-panel.html`)
- Eyebrow "Kitchen units · plan 2" (the job name, or "Cutting plan"
  when there is none). The **headline answers question 1**:
  "All 21 parts fit". With "Best use of the panels I have" it reads
  "18 of 21 parts fit"; with the "Shortest" goals it reads "Uses
  1850 mm of 2440 mm". The subline gives the panels: "3 panels of Birch
  ply 18 mm, 2440 × 1220 mm".
- **Figures row** between two hairlines:
  - *3 panels* (2440 × 1220 mm each);
  - *21 / 21 parts* (✓ No missing parts, or the missing count in red);
  - *18.6% waste* (m² of offcuts and saw dust);
  - *1 leftover to keep* (its size and panel). Reusable material is
    kept apart from waste.
- **Status line** that separates a complete plan from a proven best
  one: "✓ Complete plan · time limit reached after 10 s, so a longer
  search may still save material", with *Search for 60 s*
  (`ended_by: "time_limit"`). With `ended_by: "complete"` it reads
  "✓ Complete plan · the search finished; this is the best plan it can
  find". Reaching a time limit never claims optimality.
- **Panel navigation** (left, 170 px; a horizontal strip on tablets):
  *All panels* first, then one card per pattern with a thumbnail, part
  count, waste and "1 strip to keep". The selected card has an accent
  inset bar. `× n` marks a pattern cut on several panels.
- **All panels**: every panel drawn with names and sizes and a
  *Cut order →* link, then the **parts checklist** (part, length,
  width, grain, needed, cut ✓, on which panels). With "Best use",
  parts not cut come first, marked "not cut".
- **One panel**: the drawing (§4.6) in a bordered viewer headed
  "Panel 1 / 3" with its material and figures and "Grain →". Under the
  drawing are the legend and the **current cut** bar: "Cut 7 of 19 ·
  320 mm along the length → Fixed shelf 320.5 × 320", with large ‹ ›
  buttons. Next to it (below it on tablets) is the **Cut order** list,
  grouped *Panel into strips*, *Strip A*, …. Each cut shows:
  - the **size to cut off** in bold (what you set the fence or stop
    to);
  - the direction (*along / across the length*, the FIELDS.md words;
    *trim* for last-stage trimming);
  - what it produces (`→ Side 1040.5 × 630`);
  - for the last cut of a piece, what is left (`leaves offcut 1040.5 ×
    10`).

  Done cuts are dimmed and the current one has the accent bar. Below
  the drawing are *On this panel* (parts × count) and *Offcuts*
  (largest first, so usable ones can be set aside).
- **Cutting settings and search details**: a disclosure at the bottom
  (kerf, stages, first cuts, trimming; when the plan was found; that a
  time limit does not prove the best result).
- Tapping a part in the drawing shows its name, size, grain and whether
  it was turned, in the detail bar. Tapping a cut badge or a list row
  makes that cut current. The current cut is kept in the URL, so a
  tablet that sleeps comes back to it.

### How the cut order is derived

The plan tree already holds everything (`docs/ARCHITECTURE.md`,
Solution). For each piece, the UI lists the cuts that separate its
children in tree order (one cut per child but the last), then recurses
into each child. So all the full-panel cuts come first, then each strip
is finished before the next, as a panel saw is worked. The size to cut
off is the child's extent along the cut axis. Two cases need care:

- A piece with a single child of the same size (a wrapper the tree uses
  to reach the next stage) needs no cut and is skipped.
- A final child thinner than the kerf (the golden data has a 0.4 mm
  one) is listed as "saw dust (0.4 mm)" and drawn without a label.

The cut direction is not in the plan (§8, #13 is deferred), so it is
inferred: children side by side with the same `y` are cut across the
length, otherwise along it. Pieces are named hierarchically (Strip A,
Piece A1, Piece A1.1) so the woodworker can pencil the name on each
piece.

---

## 3. Interaction flows

**First use.** Job setup opens with one empty part row, one panel row
named "Panel 1", and the FIELDS.md defaults (Use as few panels as
possible, kerf 3.2, 3 stages, trimming Allowed, first cuts Either,
10 s). The empty table says how to start (`states.html`, "New job").

**Entering parts quickly.**
- Tab moves across a row. Enter in the last cell of the last row adds a
  row.
- Numeric cells use `inputmode="decimal"`. A comma is accepted as the
  decimal mark and stored as a point.
- Removing a row offers *Undo* in a toast instead of asking for
  confirmation.

**Validation, before and after submitting.**
1. *While typing* (client-side, on blur), the UI checks what can be
   checked without the solver: a number, at most one decimal, ≥ 0.1 mm,
   quantity ≥ 1, time limit 1–600, names present. The bounds come from
   the schema (§8, #3), and messages use the API's wording, under the
   row.
2. *On submit*, the server is the authority. A `422` comes back with
   `problems[]`, and each `field` path (`instance.parts[1].width`) is
   mapped to its row and column:
   - the cell gets the error style and `aria-invalid`;
   - the message appears under the row ("Width: width 320.55 mm has more
     than one decimal…");
   - an alert above the form lists each problem as a link to its field
     ("Row 2, Width: …").

   A problem about a whole list (`instance.parts`) attaches to its
   section title. If the problem is in the collapsed settings, they
   open (`editor-errors.html`). The server reports problems in two
   rounds (wrong types, then validation), so fixing one batch can
   reveal more; the alert never claims the list is complete.
3. Problems clear as soon as their field is edited.

The UI sends rows **in display order** and **never drops a row
silently**. A wholly empty trailing row is not sent; any other row is
sent as is, so positions in `problems` always match row numbers.

**Searching.**
1. `POST /jobs` → `201`, then switch to the Cutting plan tab
   (`#/jobs/{id}`).
2. Open `EventSource(/jobs/{id}/events)`:
   - each `plan` event replaces the best plan;
   - each `state` event updates the status, `elapsed` and
     `plans_found`;
   - a final `state` closes the source (`API.md`).
3. Between events, the time bar advances from the last `elapsed` with
   the local clock.
4. A dropped connection shows "Reconnecting…". `EventSource` reconnects
   by itself, and the replayed events bring the screen up to date.
5. Stop → `DELETE` → the button reads "Stopping…" until `cancelled`
   arrives.

**Opening a link later.** `GET /jobs/{id}` includes the instance (§8,
#18), so the plan, the checklist and *Edit job* work on any device.
- `404` (expired or mistyped) offers *Search again* from the draft in
  this browser, if there is one.
- `best: null` in a final state shows the no-plan states, including "No
  part fits on any panel" for "Best use of the panels I have".

**At the saw.** Cutting plan → Panel 1 → cut 1 → › for each cut. The
detail bar repeats the current cut in large type, so it can be read
from arm's length.

---

## 4. Visual language

Restrained and workshop-practical, following the handoff:

- flat surfaces separated by 1 px hairlines instead of shadows;
- small radii (3–5 px);
- one muted slate-teal accent;
- compact tables with right-aligned figures;
- hierarchy carried by a few type sizes and an uppercase eyebrow
  label.

Shadows appear only on popovers, which float.

All tokens are CSS custom properties on `:root`, redefined for dark mode
both with `prefers-color-scheme` and with an explicit `data-theme`
override. `mockups/build/styles.css` is the starting point for the
app's stylesheet.

### 4.1 Typography

The platform's system UI font (Segoe UI on Windows, Inter or Roboto
elsewhere). Base 14 px for dense desktop work; 15 px with 44 px controls
on tablets. All figures use `font-variant-numeric: tabular-nums`.

| Token | Desktop | Tablet / touch | Use |
|---|---|---|---|
| `--t-h1` | 600 27/1.2, −0.7 px | same | Page headline ("All 21 parts fit") |
| `--t-stat` | 600 23/1.1 | same | Figures row |
| `--t-h2` | 600 17/1.3 | same | Section titles, current cut |
| `--t-h3` | 600 14/1.35 | same | Sub-sections |
| `--t-body` | 400 14/1.45 | 400 15/1.45 | Body, inputs |
| `--t-small` | 400 12/1.45 | 400 13/1.45 | Tables, help text, secondary lines |
| `--t-eyebrow` | 600 11/1.3, uppercase, +1.4 px | same | Context labels ("CUTTING PLAN · PLAN 2"), cut-order groups |

### 4.2 Colour tokens

One accent, slate-teal, for the primary action, the selection, the
current cut and focus. Green and red are only for status (complete,
missing parts, errors).

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#f5f6f6` | `#171b1d` | Page background |
| `--surface` | `#ffffff` | `#202527` | App bar, viewers, inputs |
| `--soft` | `#eef2f3` | `#2c3437` | Inline help, progress track |
| `--ink` | `#20292d` | `#e5e9e9` | Primary text |
| `--muted` | `#5d6b70` | `#a8b6bc` | Secondary text, help |
| `--faint` | `#8a979b` | `#7d8b90` | Row numbers, placeholders (non-essential only) |
| `--line` | `#dce1e2` | `#3c464a` | Hairlines, input borders |
| `--accent` | `#215d70` | `#a6d5e4` | The accent; in dark mode light, with dark text on it |
| `--on-accent` | `#ffffff` | `#152e36` | Text on the accent |
| `--accent-soft` | `#e3eef1` | `#263a41` | Selected rows and options |
| `--ok` | `#32664b` | `#a4d2b2` | Complete plan, no missing parts |
| `--bad` / `--bad-soft` | `#a33c28` / `#fbeeea` | `#ffb2a2` / `#3a2723` | Field errors |

Cutting-plan colours:

| Token | Light | Dark | Meaning |
|---|---|---|---|
| `--part` / `--part-line` | `#e8eff1` / `#8da6b0` | `#30434a` / `#839da7` | Part |
| `--offcut` + `--hatch` | `#eef2f3` + `#d3dadc` | `#2c3437` + `#3c464a` | Offcut: 45° hatch |
| `--leftover` | surface, dashed `--muted` | same | Leftover to keep |
| `--kerf` | `#5d6b70` | `#0f1213` | Saw cut: the panel shows through the gaps |

Part, offcut and leftover differ in **pattern** (plain, hatched, dashed)
as well as tone, so the drawing reads in greyscale, on a printout and
for colour-blind users. The selected part or the produced piece of the
current cut uses the accent.

### 4.3 Spacing, radii, borders

- Spacing on a 4 px grid: `4 8 12 16 20 24 28 32 40 48`. Page gutters
  are 28 px (20 on tablet). The setup column is ~1080 px; the plan view
  uses the full width.
- Radii: `3` (table inputs), `4` (fields, selects, panel cards), `5`
  (buttons), `8` (popovers' container if needed). Drawings and viewers
  are square-cornered.
- Borders: 1 px `--line` around viewers, inputs and panel cards;
  hairlines between table rows and above and below the figures row.
  No card shadows.
- Controls are 34 px high on desktop and **44 px on tablets and touch
  screens** (`pointer: coarse` or narrower than 1100 px). Table cells
  are 30 / 40 px. The (?) buttons are 18 px but have a 42 px invisible
  hit area.

### 4.4 Components

| Component | Notes |
|---|---|
| App bar | Brand, job context ("21 parts · Birch ply 18 mm") |
| Tabs | Job setup, Cutting plan; underline accent; a live dot while searching |
| Eyebrow + headline | Uppercase context label over a headline that answers the user's question |
| Figures row | Large figure + unit word + one small line, between hairlines |
| Status line | ✓ Complete plan · why the search ended · action |
| Button | Primary (accent fill), Secondary (surface + line), Link (accent text) |
| Table input | 1 px line, radius 3, right-aligned figures; error: red border and tint |
| Help button (?) | 18 px circle next to every label and column header; `aria-expanded` (§5) |
| Help popover | For table columns: label and full help text, close button, arrow to the header |
| Inline help | For settings: a soft box under the control |
| Select, segmented row | Goal as a select; stages, trimming, first cuts as segmented rows (selected: soft accent fill and an accent underline) |
| Disclosure | "More cutting settings", "Cutting settings and search details": ▸/▾ with a summary of values |
| Alert | Red-bordered summary of 422 problems with links to fields |
| Panel card | Thumbnail, "Panel n", parts and waste; selected: accent border and inset bar |
| Viewer | Bordered surface with heading, drawing, legend, detail bar |
| Cut list | Grouped steps with circled numbers; done (dashed, faint), current (accent), next |
| Manifest rows | Name and size, count on the right (On this panel, Offcuts) |
| State block | Eyebrow (state), title, one paragraph, causes, actions |

### 4.5 Motion

Short and functional only:
- a 150–200 ms cross-fade when a better plan replaces the drawing;
- the time bar advancing;
- disclosures and popovers appearing.

All of it is disabled under `prefers-reduced-motion`.

### 4.6 The cutting-plan drawing

- **Orientation**: length horizontal, width vertical, as `FIELDS.md`
  requires. y = 0 is the **bottom** edge, as in upstream, so the later
  "bottom/top trim" fields will match the drawing. "Grain →" in the
  viewer head shows the panel's grain direction.
- **Saw cuts** are not drawn as lines: the panel is filled with the
  kerf colour and every piece is drawn on top, so each kerf shows as a
  gap of its true width (about 1 px at desktop size).
- **Labels**: parts show their name over their size as drawn, offcuts
  their size, and the leftover "Keep · leftover" and its size. Labels
  are fitted, in this order:
  1. name and size;
  2. the same turned 90° for tall pieces;
  3. size only;
  4. nothing.

  Unlabelled pieces can still be tapped, and they are in the lists.
- **Glyphs**: ↔ in the corner of parts that keep the grain direction;
  ↻ on parts turned 90°. Glyphs appear only where they don't crowd the
  label.
- **Rulers** (panel view): panel length on top, width on the right, and
  the first-stage strip sizes along the edge they are measured from.
- **Cut badges**: small circled numbers at the start of each saw line,
  nudged along the line when they would overlap. The current cut draws
  its line in the accent and tints the piece it produces; earlier
  badges are faded.
- **Sizing**: the drawing is laid out at the pixel width it is shown at,
  so 11–12 px labels stay 11–12 px. Thumbnails omit labels. On tablets
  the drawing can be pinch-zoomed.

---

## 5. Help text: one tap away

Every field has its exact `FIELDS.md` help text **behind a (?) help
button next to its label**, a familiar pattern. It replaces the earlier
always-visible text, which made the setup screen a wall of prose. It
works on touch screens, unlike hover tooltips:

- **Table columns** (parts, panels): the (?) in each column header opens
  a **popover** under the header with the column's label and full help
  text. Tap again, tap elsewhere, press Esc or tap × to close
  (`editor.html` shows "Keep grain direction" open). One popover at a
  time.
- **Settings** (Goal, kerf, stages, trimming, first cuts, time limit):
  the (?) next to the label expands the help **inline, in a soft box
  under the control** (`editor.html` shows the kerf help open). It
  stays open until closed, so it can be read while choosing. For option
  fields, the sentence about the selected option is emphasised; the
  words never change.
- **Behaviour**:
  - the button is a real `<button>` with `aria-expanded` and
    `aria-controls`, labelled "About Saw blade thickness (kerf)";
  - the help element is linked to its input by `aria-describedby`, so
    screen readers read it with the field even when it is closed;
  - the hit area is 42 px.
- **No input without help**: every input component requires a `field`
  key, and its help comes from `fields.generated.ts` (§7). An input
  without help text does not compile, and an end-to-end test opens
  every (?) and checks that its text matches FIELDS.md.
- **Short visible hints stay** where they prevent mistakes:
  - "Sizes in millimetres, at most one decimal" under the page title;
  - "↔ The part keeps the panel's grain direction" under the parts
    table;
  - "Leave Quantity empty for as many as needed" under the panels.
- **Source**: labels come from each property's `title` and help from its
  `description` in `openapi.json`. Option names come from the `oneOf`
  titles (§8, #1–#2). Help markup is rendered (`**bold**` →
  `<strong>`, `*italic*` → `<em>`), nothing else.

`docs/STAGES.md` (Stage 7) says the same: help one tap away, not on
hover.

---

## 6. Accessibility

- **Contrast**: body and help text ≥ 4.5:1 in both themes; `--faint` only
  for non-essential text. Plan labels on part fills ≥ 7:1.
- **Not colour alone**: errors have a message and `aria-invalid`.
  Parts, offcuts and leftover differ by pattern. The current cut has a
  number and a list highlight as well as the accent.
- **Keyboard**: everything is reachable in DOM order. In tables, Tab
  moves across a row and Enter on the last cell adds a row. Segmented
  rows are radio groups with arrow keys. In the cut list, ↑ and ↓ move
  the current cut. Esc closes a help popover.
- **Screen readers**:
  - inputs have names ("Length, row 3") and their help as
    `aria-describedby`;
  - the 422 alert is `role="alert"`;
  - the progress bar is a `progressbar`;
  - new plans are announced politely ("Plan 2: 3 panels, 18.6%
    waste");
  - each drawing is `role="img"` with a title, and the cut list is its
    text equivalent.
- **Touch**: 44 px controls and hit areas; no hover-only affordances.
- **Zoom**: layouts reflow to one column; the drawing scales.
- **Reduced motion** is respected (§4.5).
- **Language**: `lang="en"`; FIELDS.md terms only (panel, part, kerf,
  grain, offcut, leftover, plan).

---

## 7. Proposed front-end stack

| Concern | Choice | Why |
|---|---|---|
| Build | **Vite** + **TypeScript** (strict) | Instant dev server, plain static output (`web/dist`) that any server or the `api` crate can host; TS catches contract drift at compile time. |
| UI framework | **Svelte 5** (plain Vite plugin, no SvelteKit) | Templates read like HTML and CSS, which suits an owner who mostly writes Rust; compiled, so a small bundle, good on workshop tablets; scoped CSS fits the token approach; SVG is first-class. **Preact + signals** is the alternative if JSX is preferred. No component library: the components in §4.4 are small and must look like this design. |
| API types | **openapi-typescript** generates `src/api/schema.d.ts` from `openapi.json` | One source of truth: Rust types → utoipa → OpenAPI → TS. A Rust change that breaks the UI fails `npm run check` in the same change. |
| API calls | **openapi-fetch** (a tiny typed `fetch`) | Paths, bodies and responses typed from the schema. |
| Events | A ~40-line typed wrapper around `EventSource` | Uses the generated `PlanReport` and `StateEvent` schemas; closes on final states. |
| Labels and help | `scripts/gen-fields.ts` reads `openapi.json` and writes `src/api/fields.generated.ts`: `{ "instance.parts[].length": { label, help, default, minimum, multipleOf, options, … } }` from `title`, `description`, `default`, bounds and `oneOf` titles | Types carry descriptions only as comments; the UI needs them at run time. Inputs take a `field` key typed as a union of the generated keys, so **an input without help text does not compile**. |
| State | Svelte runes; the draft job in `localStorage` (wrapped in try/catch) | No server persistence (Open in `SCOPE.md`). |
| Plan geometry | Pure TS module `src/plan/` (cut order, direction inference, label fitting, per-panel figures) with unit tests against golden plans | Mirrors the mockup generator; per-panel figures and cut directions are computed here because #8 and #13 are deferred. |
| Tests | **Vitest** (logic), **Playwright** (end-to-end against `cargo run -p api`) | The Stage 7 exit criterion "enter instance → solve → see a valid plan" is one Playwright test; another opens every (?) and compares its text with FIELDS.md. |
| Lint and format | `svelte-check`, ESLint, Prettier | Matches the Rust side's fmt and clippy gate. |

Proposed layout:

```
web/
  DESIGN.md
  mockups/              # this design (static)
  package.json  vite.config.ts  tsconfig.json  openapi.json (generated, checked in)
  scripts/gen-fields.ts
  src/
    api/                # schema.d.ts, fields.generated.ts (generated), client.ts, events.ts
    plan/               # cut order, directions, label fitting, figures (+ tests)
    components/         # Button, Field, HelpButton, HelpPopover, EditTable, PanelSvg, CutList, …
    routes/             # JobSetup, CuttingPlan (searching / all panels / panel), states
    styles/tokens.css   # from mockups/build/styles.css
  e2e/                  # Playwright
```

`web/openapi.json` is written by `cargo run -p api -- --print-openapi`
(done on the API branch) and checked in, so the UI builds without a
running server and the diff shows contract changes.

---

## 8. API requirements from the UI

What the UI needs from `docs/API.md`, and what was missing or awkward
in the first version of the contract. The orchestrating session triaged
these requests on 2026-10-25. The API is on branch `claude/stage6-api`;
its `docs/API.md` lists what changed in its *Changes* section. The
detailed items below keep the original reasoning. This table is the
current status.

### Status of the requests

**Status:** ✅ *done* means already in `docs/API.md` on
`claude/stage6-api`; 🔨 *accepted* means being implemented there;
⏸ *deferred* means not for now, and the UI uses its fallback.

| # | Request | Status | Screen / interaction | What the UI does |
|---|---|---|---|---|
| 1 | `title` = FIELDS.md UI label on every property | 🔨 accepted | Every input's label (editor) | Labels come from `title` via `fields.generated.ts` (§7) |
| 2 | Option names per enum value (`oneOf` + `const` + `title`) | 🔨 accepted | Goal options, segmented controls (editor) | Option names come from `oneOf` titles, not from the bold words |
| 3 | `default`, `minimum`/`maximum`, `multipleOf: 0.1` | 🔨 accepted | New job defaults; validation as you type | Defaults and client checks are generated from the schema |
| 4 | Named `PlanReport` and `StateEvent` schemas | ✅ done | Typed SSE handling (search) | The `EventSource` wrapper uses the generated types |
| 5 | `stages` type | 🔨 accepted (documented): stays `2 \| 3 \| "unlimited"` | Stages segmented control | Maps the union to "2", "3", "Unlimited" |
| 6 | Help-text markup | 🔨 accepted: bold and italic only | All help text | Renders `**bold**` and `*italic*`, nothing else |
| 7 | `cargo run -p api -- --print-openapi` | ✅ done | UI build and CI | `npm run gen` writes `web/openapi.json` from it |
| 8 | Per-pattern `metrics` | ⏸ deferred | Panel header, overview cards | **Computed from the tree** in `src/plan/` (as the mockups do), with unit tests against golden plans |
| 9 | `ended_by: "complete" \| "time_limit"` | 🔨 accepted | Plan header and search end | "Best plan this search can find" vs "Time limit reached; a longer search may save material" |
| 10 | `queue_position` for `queued` jobs | 🔨 accepted | Waiting state | "2 jobs ahead of yours" |
| 11 | `Retry-After` on `503 busy` | 🔨 accepted | Busy state | Counts down, then enables *Try again* |
| 12 | Children listed in position order | 🔨 accepted (docs) | Cut order | Relied on |
| 13 | `cuts` direction on every `piece` | ⏸ deferred | Cut order, badges (above all for unlimited stages) | **Inferred from geometry**: children with the same `y` are cut across, otherwise along. One child: across if it is shorter in length. Unit-tested on unlimited-stage golden plans |
| 14 | Trimming appears as stage `stages + 1` | 🔨 accepted (docs) | Cut order ("trim") | Relied on |
| 15 | Wrapper pieces (single child of the same size) | 🔨 accepted (docs) | Cut order | Skipped: no cut |
| 16 | Sub-kerf offcuts | 🔨 accepted (docs) | Drawing, offcut list | Shown as saw dust, unlabelled |
| 18 | The instance in `GET /jobs/{id}` | 🔨 accepted | A plan opened from a link on another device; *Edit job* | Parts summary, parts not cut, and *Edit job* work on any device; the local draft is only a convenience |
| 19 | `POST /instances/validate` | ⏸ deferred | Server-side problems before submitting | Client checks while typing, plus the 422 on submit |
| 20 | `problems[].field` positions = submitted arrays | 🔨 accepted (docs) | 422 mapping to rows | Relied on; rows are sent in display order and never dropped |
| 21 | Optional `name` on `POST /jobs`, echoed in the job status (new, 2026-10-25: the owner decided jobs have an optional name) | ❔ requested | App bar and plan heading, also on another device opening a job link | The name is kept with the draft in this browser; a link opened elsewhere shows "Cutting plan" without a name |

No request needed server-side persistence.

### Behaviour the UI adapts to (from the API implementation)

- **An empty plan is never served.** `best` always cuts at least one
  part. With "Best use of the panels I have", `best` stays `null` when
  no part fits on any panel. The UI shows a no-plan state for that
  goal too ("No part fits on any panel", see `states.html`), with the
  same causes as for "Use as few panels as possible" (a part larger than
  every panel, settings too strict) but without "too few panels".
- **422 problems come in two rounds**: wrong types first, then
  `Instance::validate`. The banner says "Fix the highlighted fields,
  then find the plan again" and never promises the list is complete. In
  practice the first round rarely reaches the user, because the
  client-side checks (§3, generated from the schema per #3) catch
  missing and mistyped fields before submitting. `field: null` now
  means only that the body was not a JSON object, which the UI never
  sends; it would be shown in the banner only.
- `404` also for a malformed id, so the "no longer on the server" state
  covers links that were mistyped or truncated.
- Times are rounded to the millisecond; the UI shows them to 0.1 s.

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
2. **Printing.** The handoff's third concept, *Job sheet* (a document
   with a per-panel manifest), suits printing: one panel per page with
   its cut list and parts. Export is Open; is a print stylesheet in
   scope for v1?
3. **Dimension format**: plain `1040.5` (as typed), or with thousands
   separators (`1 040.5`)? The mockups use plain figures to match
   input.
4. **Part labels** in the drawing show the size as drawn (horizontal ×
   vertical). The alternative is the part's own length × width as
   entered. The first matches the rulers and the saw; the second
   matches the parts table.

Decided (2026-10-25, owner): help text sits behind a (?) button next to
each label, opened by tap or click, instead of being always visible
(§5; `docs/STAGES.md` updated). The visual direction follows the design
handoff (Guided setup + Workbench). Jobs have an optional name; part and
panel names are optional and generated when empty (`docs/SCOPE.md`,
`docs/FIELDS.md`).
