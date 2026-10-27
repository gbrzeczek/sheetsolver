# Implementation stages

Each stage has a goal, deliverables, and exit criteria. A stage is done
only when every exit criterion holds. Mark stages done here, with the
date, as they land.

---

## Stage 0 — Skeleton and oracle ✅ done 2026-10-25

**Goal:** a workspace that builds, and a way to get reference answers from
upstream.

Result:
- Workspace with the six crates (empty), lints, CI workflow.
- `tools/oracle` runs upstream's tree search only (no reduction,
  `not-anytime-sequential`, fixed queue size). Upstream still computes
  its bounds first (needs HiGHS); see the assumption in `TESTING.md`.
- `data/instances/`: 42 instances converted from upstream (90 skipped as
  out of scope, reasons in `MANIFEST.json`) and 72 generated ones
  covering every objective × stages × cut type × kerf (0 / 3.2 mm)
  combination, with some half-millimetre part sizes.
- Lengths are decimal millimetres in JSON (at most one decimal) and tenths
  of a millimetre inside the oracle; the oracle rejects more decimals.
- `data/golden/9b656a0d/`: oracle solutions at queue sizes 64 and 2048,
  reproduced byte for byte by `tools/run_oracle.py --check`. 4 of them
  (open dimension Y, 2 stages, exact cuts) contain no solution, and the
  instances are indeed infeasible with vertical first cuts: every strip
  must match a part's dimension, and the narrowest set of strips that
  fits every part is wider than the panel.

Deliverables:
- Cargo workspace with empty crates from `ARCHITECTURE.md`, CI-ready
  (`fmt`, `clippy`, `test`).
- `tools/oracle`: a small C++ program that builds upstream packingsolver at
  the pinned commit (`UPSTREAM.md`) via CMake `FetchContent`, reads an
  instance in **our** JSON format, runs `optimize()` with given parameters,
  and writes the solution in **our** JSON format.
- `data/instances/`: a first set of instances in our format, converted
  from `upstream/data/rectangleguillotine` (script in `tools/`).

Exit criteria:
- `cargo test --workspace` passes (trivially).
- Oracle produces a solution JSON for every instance in `data/instances/`
  in deterministic mode, and the output is reproducible across runs.

## Stage 1 — Model and feasibility checker ✅ done 2026-10-25

Result:
- `model`: `Length` (tenths of a mm, decimal mm in JSON), `Area`,
  `Instance` with `validate()`, `Solution` tree, `Metrics`. Doc comments
  are the `FIELDS.md` help text.
- `checker`: tree rules (stages, guillotine layout, kerf spacing, part
  sizes, grain), stage counting and cut types as upstream defines them
  (`solution.cpp`), quantities, leftovers, metrics.
- All 224 oracle solutions pass, and every metric equals upstream's
  (counts, areas, waste, value, used length/width). The 4 empty oracle
  results are rejected for missing parts, as they should be. 25
  negative tests, one per rule. The `ARCHITECTURE.md` JSON examples are
  tested too.
- `[verify]` resolved: "homogenous" means one part type per last-stage
  strip (stage-1 strips with 2 stages, stage-2 strips with 3), not one
  size. Help text fixed.
- Found: upstream turns "unlimited" stages into exact cuts in any
  direction, and its tree search only produces 3 levels; see `SCOPE.md`.


**Goal:** the types everything else speaks, and an independent judge.

Deliverables:
- `model` crate: `Instance`, `Part`, `Panel`, saw settings and
  `Solution` (tree: patterns → nodes with explicit `NodeKind`), with
  `serde` and validation (`Instance::validate`). Field names and doc
  comments follow `FIELDS.md` exactly.
- Resolve every **[verify]** marker in `FIELDS.md` against upstream code,
  and fix the help text if it was wrong.
- `checker` crate: `check(&Instance, &Solution) -> Result<Metrics, Vec<Violation>>`
  covering every **In** constraint in `SCOPE.md`, plus metrics (sheets
  used, waste, profit).

Exit criteria:
- Every oracle solution from Stage 0 passes the checker, and the metrics
  match the oracle's reported metrics.
- Hand-written negative tests: at least one failing solution per
  constraint (overlap, out of bounds, wrong copies, non-guillotine cut,
  too many stages, …).

## Stage 2 — Generic iterative beam search ✅ done 2026-10-25

Result:
- `treesearch` crate: `BranchingScheme` trait, `iterative_beam_search`
  (upstream `iterative_beam_search_2`), `Parameters` (including
  `fixed_queue_size`, upstream's deterministic settings), `Stop`
  (deadline and cancellation flag), `Guide` (f64 ordered like C++ `<`).
- `tools/oracle/treesearch_toy.cpp`: upstream's search on a toy knapsack
  scheme; the same scheme in Rust in `crates/treesearch/tests/`.
- On 5 seeded instances × 5 parameter sets (deterministic, anytime-like,
  node limit), the Rust search reproduces upstream exactly: node
  counters, best solution and every new-solution callback. Breaking the
  tie-breaking or the dominance rule makes the test fail.
- Left out as unused: `goal`, `cutoff`, `global_history`, solution pools
  larger than one, JSON search-tree export. Upstream's other variant,
  `iterative_beam_search` (used by the maximal-spaces search), is ported
  in Stage 5.

**Goal:** a port of the part of `treesearchsolver` that the guillotine
solver uses.

Deliverables:
- `treesearch` crate: a `BranchingScheme` trait and
  `iterative_beam_search_2` (growing queue sizes, dominance via hashing,
  time limit, cancellation token, new-solution callback).
- Toy branching scheme (e.g. small knapsack) for tests.

Exit criteria:
- Same number of expanded nodes and same result as upstream
  `treesearchsolver` on the toy scheme (verified with a small C++ harness
  or by matching traced node counts).
- Cancellation and time limit tested.

## Stage 3 — Guillotine branching scheme ✅ done 2026-10-25

Result:
- `guillotine` crate: `problem` (model → upstream terms, flipped sizes
  for horizontal first cuts), `scheme` (upstream `BranchingScheme`: one
  part per child at depths 2/1/0/new panel, strip growth, the dominance
  rules between children and between fronts, guides 0/1/4/5, bounds and
  `better` for the four objectives), `plan` (upstream `SolutionBuilder`
  and `to_solution`, and `Solution::operator<` to pick between searches),
  and `solve()` (upstream `tree_search()` in not-anytime-sequential mode).
- With trims, defects, minimum waste, cut distances, cut counts, costs,
  resources and roadef2018 all out of scope, most of upstream's
  `update()` reduces to three rules; `scheme.rs` says which.
- Differential test (`crates/guillotine/tests/golden.rs`): all 108
  instances with 2 or 3 stages give exactly the golden plan at queue
  sizes 64 and 2048 (216 plans, the empty ones included), and every
  plan passes the checker. Reversing the node-id tie-break or the
  rotation order made 19 to 48 of the first 84 differ.
- 24 generated instances with two or three panel sizes
  (`generated__panels_*`, bin packing and knapsack) cover upstream's
  multi-panel paths: one search with first cuts "either" in which each
  new panel picks its direction, and knapsack using the smallest panels
  first.
- Bounds: as `TESTING.md` assumed, upstream's early stop on bounds never
  changes the plan (verified, see there). The port keeps only upstream's
  in-search stop after an exhaustive search.
- Benchmarks (`cargo bench -p guillotine` against
  `tools/bench_oracle.py`, which times upstream's `tree_search()` alone):
  the port takes 0.61–0.69× upstream's time on all 6 instances at both
  queue sizes (e.g. 566 ms against 869 ms for the slowest at 2048), with
  `Rc` nodes and the standard hasher; no arena needed.
- Found: with first cuts "either" and one panel size, upstream runs one search per direction, so every panel of a plan
  is cut the same way; only with several panel sizes may each panel pick
  its direction. The `first_cuts` help text said otherwise and is fixed.
  Ties between the searches go to the earliest, in the order guide 0
  across, guide 0 along, guide 1 across, guide 1 along.
- Found: upstream (and the oracle) rejects an open-dimension panel with no
  quantity; `model` accepts it and the port treats it as one panel.

**Goal:** the actual solver, behaviour-identical to upstream.

Deliverables:
- `guillotine` crate: port of `rectangleguillotine` `BranchingScheme`
  (insertions, subplates, front, guides, bounds, dominance) for the **In**
  features only, plus conversion from search nodes to `model::Solution`.
- All three first-cut directions: upstream solves "along the length" on a
  flipped instance (`instance_flipper.cpp`) and "either" by running one
  search per direction (`tree_search.cpp`, `tree_search()`).
- `solve(&Instance, &Params, on_solution) -> Output`.
- Criterion benchmarks on a few representative instances.

Instances with unlimited stages are out of this stage: upstream does not
solve them with this tree search (see Stage 5). The oracle's current
golden files for them are only a placeholder.

Exit criteria:
- Differential test: in deterministic mode with fixed queue sizes, the
  solution equals the oracle's on every instance in `data/instances/`
  with 2 or 3 stages.
- All solutions pass the checker.
- Benchmarks within ~1.2× of the oracle for the same queue sizes.

## Stage 4 — Developer CLI ✅ done 2026-10-25

Result:
- `cli` crate (clap, anyhow): `solve` (plan JSON to stdout or
  `--output`, `--queue-size`, `--time-limit`, `--progress`; the summary
  on stderr is checked by the checker), `check` (metrics and every
  violation; exit status 1 if infeasible), `diff-oracle` and `bench-set`
  (fastest and median of `--repeat` runs per queue size, by default on
  `crates/guillotine/benches/instances.txt`). Defaults point into the
  checkout the binary was built from, so they work from any directory.
- `cargo run --release -p cli -- diff-oracle` is the full differential
  suite: every golden file in `data/golden/9b656a0d/`, one line each
  (`--quiet` for failures only) and a summary, exit status 1 on any
  difference, checker violation or error. Today: 278 golden files in
  about 6 s, 216 same, 0 differ, 62 skipped (the 31 unlimited-stage
  instances, until Stage 5). `--filter` and `--queue-size` narrow it.
- `crates/cli/tests/commands.rs` runs the binary: each subcommand passes
  on good input and fails on a broken plan or golden file.
- `AGENTS.md` now names a real instance in its `solve` example.

Deliverables:
- `cli` crate: `solve`, `check`, `diff-oracle`, `bench-set` subcommands.

Exit criteria:
- One command runs the full differential suite and prints a report.

## Stage 5 — Unlimited stages ✅ done 2026-10-25

Result:
- Ported: treesearchsolver's `iterative_beam_search` (global history),
  the maximal-spaces search (blocks, scheme, plan replay, sequential
  feasibility), the hypergraph search with infinite copies (with
  `minimize_number_of_stages`), sequential single knapsack and sequential
  value correction, the tree search on unlimited stages, upstream's
  automatic selection and run order, libc++'s `std::sort` (upstream's ties
  depend on it), and upstream's area bounds with the stop once a plan
  reaches one (`bounds.rs`).
- Oracle `--algorithm` for each algorithm and `auto`; 42 new generated
  `generated__unlimited_*` instances. `diff-oracle`: 562 golden files,
  558 identical, 0 different, 4 skipped (their best plan is column
  generation's, which is Out); every plan passes the checker. `solve()`
  selects every algorithm upstream's own selection used (checked against
  the `auto` files' improvement labels).
- Checker: deep plans with turning cuts and pass-through pieces are
  accepted (`crates/checker/tests/unlimited.rs`).
- Benchmarks against the oracle built with libc++ (port's median over
  upstream's fastest of 10): tree search 0.59–0.72× (0.35× on bin
  packing with a kerf, where the bound now ends the search early),
  maximal spaces 0.95–1.05×, hypergraph 1.02–1.17×, sequential single
  knapsack 0.65–0.72×, sequential value correction 0.76–1.10×. Two
  speed-ups were needed: the hypergraph's raster-point tables (1.8× →
  1.0–1.2×), and the bounds, without which the sequential heuristics'
  one-panel subproblems searched on after cutting every part (1.55× →
  0.72×).
- Deviations: open-dimension plans get the uncut rest of the panel as a
  cut and a leftover (`open_dimension.rs`; the differential tests apply
  the same completion to the golden plans); upstream's bounds from its
  1D solver and dual feasible functions are not ported (same plans,
  `TESTING.md`).
- The oracle builds in the cloud environment with a git clone of HiGHS
  and must use libc++ on Linux: with libstdc++, 10 of the 35 sequential
  single knapsack golden files differ (`tools/README.md`).
- The two decisions, measured with `tools/measure_decisions.py`:
  - Subproblems without reduction (35 unlimited-stage instances with
    several panels): the same panel count on every bin-packing
    instance; on knapsack, reduction cuts more value on 4 instances
    (+0.4 % to +1.4 %), never less. Without reduction is faster on most
    instances (up to 2.7× on value correction). Kept.
  - Sequential heuristics on 2 and 3 stages (59 instances with several
    panels): upstream runs them next to the tree search and keeps the
    better plan. They would save a panel on 1 instance
    (`generated__panels_bin_packing_2_either_exact_kerf3_2`, 3 → 2
    panels) and cut 1.7 % more value on 1, at up to 5 s more time;
    elsewhere the tree search's plan is as good or better. Owner to
    decide whether to revisit (`SCOPE.md`).
- The owner approved Stages 6 and 7 in parallel with the end of this
  stage, since neither changes the solver.

**Goal:** real unlimited-stage plans, solved the way upstream solves them.

Plan (sub-steps, each ends with `diff-oracle` clean):
1. Oracle `--algorithm` (tree search, maximal spaces, hypergraph with
   infinite copies, sequential single knapsack, sequential value
   correction, and `auto` for upstream's own selection) and golden files
   per algorithm, replacing the placeholder unlimited-stage ones;
   `diff-oracle` learns them.
2. More generated unlimited-stage instances: single panel, several
   panels, many copies of few parts.
3. Checker tests: unlimited-stage plans deeper than 3 levels, with mixed
   cut directions, are accepted.
4. Port of treesearchsolver's `iterative_beam_search` and of
   `tree_search_maximal_spaces`.
5. Port of `tree_search_hypergraph_infinite_copies`.
6. Sequential single knapsack and sequential value correction, plus the
   tree search on unlimited stages (upstream's 3-level behaviour), which
   upstream runs next to them for several panels.
7. Upstream's algorithm selection in `solve()`; measure what the two
   decisions below cost (reduction, multi-panel heuristics on 2/3 stages).

Decisions (2026-10-25, see `SCOPE.md`): the sequential heuristics solve
their single-panel subproblems without instance reduction (upstream
reduces them; the oracle turns it off so golden files compare like with
like), and they run for unlimited stages only; 2- and 3-stage instances
stay on the tree search alone.

Upstream gives "unlimited" its own meaning: exact cuts, in either
direction at every stage, with no limit on depth. It solves single-panel
instances with the maximal-spaces search, or with the hypergraph search
when there are many copies, and multi-panel instances by filling one panel
at a time with those single-panel searches.

Deliverables:
- `guillotine` crate, new modules:
  - port of `tree_search_maximal_spaces` (its own branching scheme on the
    `treesearch` crate), and of treesearchsolver's `iterative_beam_search`
    (not `_2`), which it runs on;
  - port of `tree_search_hypergraph_infinite_copies` (dynamic programming
    over a hypergraph of cuts);
  - ports of sequential single knapsack and sequential value correction
    (`algorithms/sequential_value_correction.hpp`) for several panels;
  - upstream's automatic algorithm selection (`optimize.cpp`), limited to
    the in-scope algorithms, so `solve()` picks the same algorithm as
    upstream for every in-scope instance.
- Oracle: an `--algorithm` option to run each upstream algorithm alone
  (deterministic, fixed parameters), and golden files per algorithm,
  replacing the placeholder unlimited-stage golden files from Stage 0.
- More generated instances with unlimited stages: single panel, several
  panels, and many copies of few parts (the hypergraph case).
- Checker: tests that unlimited-stage plans with more than 3 levels and
  mixed cut directions are accepted.

Exit criteria:
- Differential tests: each ported algorithm gives the oracle's solution
  for the same algorithm and parameters on every instance where upstream
  would select it.
- `solve()` selects the same algorithm as upstream on every in-scope
  instance.
- All solutions pass the checker; benchmarks within ~1.2× of the oracle.

## Stage 6 — Web API ✅ done 2026-10-25

Result:
- `api` crate (axum, tokio; library plus binary): `POST /jobs`,
  `GET /jobs/{id}`, `GET /jobs/{id}/events` (SSE), `DELETE /jobs/{id}`,
  `GET /openapi.json`, as in `docs/API.md`, which the implementation
  sharpened in a few places (its "Changes" section lists them; no field
  or shape changed). `cargo run -p api -- --help` lists the limits.
- Each job's state is a `watch` channel: the solver thread writes it,
  `GET` reads it, each event stream sends the latest plan and then the
  state from one snapshot, so a plan always precedes the state after it
  and the last plan is never skipped. A FIFO semaphore with one permit
  per solver bounds the running jobs (`--max-running-jobs`, default the
  CPU cores) on a blocking pool of that size; `DELETE` sets the solver's
  `Stop` cancellation flag (a queued job is cancelled at once).
- Every plan passes the checker before it is served, and its `metrics`
  are the checker's; a plan that broke a rule would stop the job as
  `failed` instead. Empty plans are never served.
- `openapi.json` from `utoipa`: `model` derives `ToSchema` behind its
  `openapi` feature, with hand-written schemas for `Length`, `Area`,
  `Stages` and `NodeKind` (whose serde code is custom or whose tag a
  derived schema leaves undescribed). Descriptions are joined into
  paragraphs, so instance fields read exactly as in `FIELDS.md`.
- `422` lists every mistyped or missing field at once (a field-by-field
  walk of the body, used only to find problems; the instance itself is
  always read by serde) and then every `Instance::validate` problem with
  its field path, from the new `Instance::validate_fields`.
- Tests (`cargo test -p api`, about a second): every property and path
  parameter in `openapi.json` has a description; instance field
  descriptions equal `FIELDS.md`; submit and poll to a checked plan;
  the event stream (plans numbered upwards, all checked, final state
  last, replay after the end); cancelling a running and a queued job;
  `503 busy`; `422` with every problem; `400`, `404`, `413`; expiry;
  CORS. A test also walks all test instances to make sure the problem
  walk accepts everything serde accepts.
- Follow-up for the UI (its requests in `web/DESIGN.md` §8, as
  triaged): input properties carry `title` (UI label), `default`,
  `minimum`/`maximum`, and the option types name each value
  (`oneOf` `{const, title}`), from one table (`crates/api/src/fields.rs`)
  that tests hold to `FIELDS.md` and to what the server accepts; fields
  with a `FIELDS.md` default may be left out. The job status gains
  `ended_by` (also in `state` events), `queue_position` and `instance`;
  `503 busy` gains `Retry-After`. `ARCHITECTURE.md` now states the plan
  tree rules clients rely on (child order, alternating directions,
  wrapper pieces, trimming at stage `stages + 1`, sub-kerf offcuts), each
  checked against the checker and the golden plans.
- Deployment (owner's decision: one container): `--web-dir` serves the
  built UI at `/`, with `index.html` for client-side routes, the API's
  paths keeping priority (tests in `crates/api/tests/web.rs`); every
  option is also a `SHEETSOLVER_*` variable. A multi-stage `Dockerfile`
  (cargo-chef for cached dependencies, a placeholder for the Node stage
  until `web/` builds, a non-root Debian runtime), a workflow that
  pushes the image to GHCR, and `deploy/docker-compose.yml`; the deploy
  job waits for the server's details. See `ARCHITECTURE.md`,
  *Deployment*.

**Goal:** solver as a service.

Deliverables:
- `api` crate (axum + tokio): `POST /jobs`, `GET /jobs/{id}`,
  `GET /jobs/{id}/events` (SSE stream of improved solutions),
  `DELETE /jobs/{id}` (cancel), `GET /openapi.json`.
- Solver runs on a bounded blocking pool; concurrency limit configurable.
- OpenAPI spec generated from the Rust types (`utoipa`), with every
  field's description taken from its doc comment.

Exit criteria:
- Test: every field in `openapi.json` has a description.
- Integration tests: submit, stream, cancel, invalid instance → 422.
- Solutions served by the API pass the checker.

## Stage 7 — Web UI ✅ done 2026-10-25

Result:
- Design first (`web/DESIGN.md`, static mockups and screenshots in
  `web/mockups/`), following the owner's design handoff: *Guided setup*
  for the job, *Workbench* for the plan. Help text sits behind a (?)
  button on every label and column header (owner's decision), and names
  are optional: an empty part or panel name is sent as its generated
  default ("Part 9"), and the job name goes to the API (`name`, #21).
- `web/`: Vite + Svelte 5 + TypeScript (versions pinned, at least two
  weeks old). The client types come from `openapi.json` (checked in; CI
  fails if it differs from what the Rust types print), and every input's
  label, help, default and bounds from the same file
  (`scripts/gen-fields.mjs`), so an input without help does not compile.
- Job setup: parts and panels tables, Goal and kerf, the other settings
  collapsed with a summary; local checks with the API's paths and
  wording, and `422` problems next to their field; the draft kept in
  the browser. Cutting plan: the live search (time bar, figures, plans
  found, Stop), then the headline ("All 21 parts fit"), figures, a
  status line that tells a finished search from a time limit, all
  panels with a parts checklist, and one panel drawn to scale with
  rulers, numbered cuts, the current cut (in the URL) and the cut order;
  no-plan, failed, expired and busy states.
- The cut order, cut directions (inferred from geometry, §8 #13) and
  per-panel figures are computed in `src/lib/plan`; unit tests check
  them against all 562 golden plans (Vitest, 23 tests).
- Exit criteria, covered by `npm run e2e` (Playwright, desktop and
  tablet, against the real API serving the built UI; 16 tests): enter a
  job → solve → a plan the checker passed, followed cut by cut, reloaded
  without the browser's storage; every `FIELDS.md` field but the
  self-explanatory ones has its (?) button with the exact text, and
  every input points at its help; stop
  a running search; the no-plan cause; field problems; the draft
  survives a reload; New job with Undo; one print page per panel.
- The Dockerfile's Node stage builds `web/` into the image; CI has a
  `web` job (`openapi.json` in sync, `svelte-check`, Vitest, Playwright).
- Owner's decisions on the open questions (`web/DESIGN.md` §9): the
  draft stays in the browser, with "New job"; a print stylesheet (one
  A4 page per panel: drawing, cut order, parts checklist); sizes shown
  with thin-space thousands (1 040.5); turned parts labelled as drawn.

Design (screens, flows, visual language, front-end stack, and what the UI
needs from the API): `web/DESIGN.md`, with static mockups in
`web/mockups/`.

Deliverables (in `web/`, see `ARCHITECTURE.md`):
- Instance editor (parts, panels, saw settings), job submission, live
  progress, cutting-plan viewer (SVG, one panel at a time + overview,
  dimensions on every part and offcut).
- Every input has its help text from `openapi.json` (i.e. from
  `FIELDS.md`) one tap away: a help (?) button next to its label opens
  the full text, by tap or click, never on hover only, so it works on
  touch screens in the workshop (decided 2026-10-25; see
  `web/DESIGN.md` §5). Fields whose label says it all (job name, part
  and panel name, part width, panel quantity) have no button; their
  help text stays in `FIELDS.md` and is read by screen readers (owner's
  decision, 2026-10-27).
- TypeScript client generated from `openapi.json`.

Exit criteria:
- End-to-end test: enter instance → solve → see a valid plan.
- No input without its help text; every input but the self-explanatory
  ones (`web/DESIGN.md` §5) has a help button. UI wording matches
  `FIELDS.md`.

## Stages 8–16 (planned 2026-10-27)

The owner moved every Later item to In, plus grouping identical panels
and the non-guillotine solver (`SCOPE.md`, decisions log, 2026-10-27).
Stage 8 comes first so the new fields land in a calmer layout instead
of adding to the clutter. Stages 9–13 each run the whole pipeline for
one feature, in this order:

1. `FIELDS.md`: label, help text (short and full, Stage 8), unit,
   default, bounds, upstream mapping. Written first.
2. `model`: the field, validation, JSON; `422` paths.
3. `checker`: the new rule, written independently of the solver.
4. `tools/oracle` and golden data: upstream instances that use the
   feature, converted into `data/instances/`.
5. The solver: port upstream's handling, same decisions.
6. API and `openapi.json`; the UI (input, drawing, print).

Exit criteria shared by Stages 9–13 (each stage adds its own):
- Every plan in the tests passes the checker, and the checker rejects a
  hand-made plan that breaks the new rule.
- Differential tests: results identical to the oracle on every golden
  instance that uses the feature, and still on all the old ones.
- The field's (?) help in the UI matches `FIELDS.md`; an e2e test
  enters the field, solves and sees it honoured in the plan.
- `cargo fmt --check`, clippy, `cargo test --workspace`, `npm run
  check`, `npm test` and `npm run e2e` pass.

## Stage 8 — Less on screen at once ✅ done 2026-10-27

Result:
- `web/DESIGN.md` §10: the principle (always visible, collapsed with a
  summary of its values, only when it matters), per-screen tables for
  job setup, the plan, the search and one panel, and the budget. Visible
  controls, prose lines and all text lines were measured before and after
  at 1440, 820 and 360 px with the mockups' job: the text to read on the
  plan fell from 184/144/167 lines to 83/48/51, on one panel by about a
  quarter (most of what is left is its cut order), and the prose from up
  to 11 lines to at most 4 on any screen. `e2e/budget.spec.ts` reads the
  budget table from `DESIGN.md` and fails a screen that goes over it, at
  any width, or scrolls sideways.
- Two levels of help: `FIELDS.md` has a *Short help* column (one
  sentence, at most 15 words) and the rewritten rule; `openapi.json`
  carries it as `x-short-help` (from `crates/api/src/fields.rs`, a
  required argument of every input field), tests in `crates/api` hold it
  to `FIELDS.md` and to one sentence of 15 words, and
  `scripts/gen-fields.mjs` fails the build without it. The (?) shows the
  short help, *More* the full text in its place; inputs are described by
  the short help (the self-explanatory ones, which have no (?), by their
  full text).
- Job setup: always visible are the parts, the panels, Goal, kerf and
  *Find cutting plan*. The job name moved into *More settings* (renamed
  from "More cutting settings"), first in its summary. *Cost* shows with
  two or more panel sizes, or while a cost is typed. Gone: the "Tap (?)"
  sentence, the repeated parts count and the "Empty names are filled in"
  hint (the problems count stays, when there are problems).
- Cutting plan: the headline and the drawings first; *Figures*, *Search*
  (why it ended, the longer search, when the plan was found, the
  settings; it replaces the status line and the bottom disclosure),
  *Plans found*, *Parts checklist*, *On this panel* and *Offcuts* are
  sections with a summary line. All start closed, except *Search* after
  a failure and *Parts checklist* when parts are not cut; while the
  search runs, "Still searching" and *Stop* stay visible. The cut
  order's explanation is behind a (?). While searching, *Open plan* sits
  next to *Stop*, and the drawings use the full width.
- Phones: the drawings are laid out at the width they are shown at, so
  their labels stay legible at 360 px instead of shrinking with the
  drawing.
- Screenshots of the app at every width in `web/mockups/screenshots/`
  (`npm run screens`); the Stage 7 mockups' ones moved to
  `stage7-mockups/`. e2e (`sections.spec.ts`): every section opens, and
  its summary matches what is inside; the help test opens every (?),
  checks the short help, then *More* and the full text.

Goal: each screen shows what most jobs need; the rest is one tap away.
Today the setup and the plan show nearly everything flat.

Deliverables:
- `web/DESIGN.md` gets a principle and a budget: per screen, what is
  always visible, what sits in a collapsed section whose summary line
  shows its current values ("3 stages · trimming allowed · 10 s"), and
  what appears only when it matters. Measured before and after: visible
  controls and lines of prose per screen at 1440, 820 and 360 px.
- Two levels of help. `FIELDS.md` gains a **Short help** column: one
  sentence, at most ~15 words, shown first when (?) is tapped, with
  *More* for the full text, which stays as it is. Both are exact copies
  (`openapi.json` carries both; the build fails if either is missing).
  The `FIELDS.md` rule "the help text is the exact copy shown" is
  rewritten to match.
- Job setup: always visible are the parts, the panels, the goal, the
  kerf (it defaults to 0, so it must be seen) and *Find cutting plan*.
  Columns appear when they mean something: *Cost* only with two or more
  panel sizes, *Value* only with "Best use" (as now). Every other
  setting, and each one Stages 9–16 add, goes in a collapsed section
  with a summary.
- Cutting plan: the headline and the drawing first; the figures, the
  status details, the plans found and the parts checklist each in a
  section that can be collapsed, open or closed by default as the
  budget decides.
- The phone layout (360 px) is checked again under the new rules.

Exit criteria:
- The budget in `DESIGN.md` holds on every screen, at every width;
  screenshots in `web/mockups/` updated.
- e2e: every collapsed section opens, its summary reflects the values
  inside, and every field is still reachable with its short and full
  help.

## Stage 9 — Trims and minimum offcut width

Goal: cut off damaged factory edges, and never leave an offcut too
narrow to handle.

Deliverables: per-panel trims (left, right, bottom, top; soft or hard)
and the minimum offcut width (upstream `*_trim`, `TrimType`,
`minimum_waste_length`), through the whole pipeline. The drawing shows
trimmed edges; the cut order starts with the trim cuts. Wording of
soft/hard for woodworkers is settled in `FIELDS.md` first (hard: the
trim is a real cut, and offcut widths are measured from it).

Exit criteria: shared ones, plus a hard trim and a soft trim that give
different plans under a minimum offcut width.

## Stage 10 — Defects

Goal: mark knots, chips or stains on a panel so no part covers them.

Deliverables: defects per panel (position and size) and *cut through
defects* (upstream `defects`, `cut_through_defects`). The UI marks a
defect by typing its position and size from the panel's corner, in a
small editor per panel row, and draws defects hatched on the plan.
(Marking by tapping on the drawing is a possible later addition.)

Exit criteria: shared ones, plus a defect that changes the plan, and a
plan that cuts through a defect only when allowed.

## Stage 11 — Strip limits

Goal: plans that fit the machine: a beam saw's maximum and minimum strip
width, or a limit on the number of first and second cuts.

Deliverables: minimum and maximum distance between consecutive first
cuts and between second cuts, and the maximum number of each (upstream
`minimum_distance_*_cuts`, `maximum_distance_*_cuts`,
`maximum_number_*_cuts`). All advanced: collapsed in the UI, and only
for 2 and 3 stages if upstream restricts them so.

Exit criteria: shared ones.

## Stage 12 — Two more goals

Goal: "Keep the leftover in one piece" and "Choose the cheapest panel
sizes".

Deliverables: the objectives `bin_packing_with_leftovers` and
`variable_sized_bin_packing` (upstream `BinPackingWithLeftovers`,
`VariableSizedBinPacking`): model, the algorithms upstream selects for
them (within scope), headline and figures for each.

Exit criteria: shared ones, plus golden instances for both goals.

## Stage 13 — Identical panels grouped

Goal: a plan with the same layout on several panels says "×3" once,
instead of drawing it three times.

Deliverables: port `group_identical_bins` (`post_process.cpp`). The
plan JSON groups panels with a count (a deliberate change to
`ARCHITECTURE.md` and `API.md`, with the checker reading the new
shape); the UI, the cut order and the print pages show each layout once
with its count; the parts checklist still counts every part.

Exit criteria: shared ones (the oracle's grouping matches), plus the
golden plans whose panels repeat show grouped.

## Stage 14 — Any layout: model, checker, oracle

Goal: everything the non-guillotine solver needs before its search,
following *Through cuts or any layout* in `SCOPE.md`.

Deliverables:
- The setting in `FIELDS.md` first: **Cuts**, options **Through cuts
  only** (default; panel saw, table saw, track saw) and **Any layout**
  (CNC router), with help that says the second gives plans a panel saw
  cannot cut.
- `model`: the setting; `422` for settings that do not apply to any
  layout (stages, trimming, first cuts, minimum offcut width, strip
  limits, cut through defects); the plan says which kind it is, and an
  any-layout plan lists part positions per panel (`ARCHITECTURE.md`,
  `API.md`).
- The kerf by enlarging parts and panels, the trims by shrinking the
  panel, defects adjusted (`SCOPE.md`'s table); the defect narrower
  than the kerf settled and written down.
- `checker`: the any-layout rules, written independently.
- `tools/oracle` builds upstream's `rectangle` too, and golden data
  from `data/rectangle/` plus our own instances with kerf, trims and
  defects, converted the same way on both sides.
- `docs/UPSTREAM.md`: the map of `src/rectangle/` files to port.

Exit criteria: every golden any-layout plan from the oracle passes the
checker; hand-made infeasible plans (overlap, gap under the kerf, on a
defect, inside a trim) are each rejected.

## Stage 15 — Any layout: search

Goal: the port of upstream's non-guillotine search.

Deliverables: the branching scheme (`rectangle/tree_search.cpp`) on the
existing beam search, the maximal-spaces search, the sequential
heuristics for several panels (sharing Stage 5's drivers), automatic
selection among the in-scope algorithms (`SCOPE.md`, *Algorithms
(non-guillotine)*), and `group_identical_bins` for these plans.
Benchmarks for node expansion as in Stage 3.

Exit criteria: identical results to the oracle (run with the same
algorithm forced) on every golden instance; the gap to upstream's own
automatic choice measured and recorded in the decisions log; every plan
passes the checker.

## Stage 16 — Any layout: API and UI

Goal: the setting in the product.

Deliverables: the API serves both kinds of plan; the UI's **Cuts**
setting hides what does not apply and keeps its values for switching
back; the drawing, figures, print pages and the parts checklist work
without a cut order (positions from the panel edges, unused area
shown). `web/DESIGN.md` updated first.

Exit criteria: e2e on both settings, desktop, tablet and phone: enter
→ solve → a plan the checker passed, printed; switching the setting
back restores the hidden values.

## Later (only after scope decisions)

- Remaining Open items in `SCOPE.md`: instance reduction, the other
  post-processing steps, the general hypergraph tree search (if
  benchmarks show it helps), edge banding, grain matching across parts,
  labels.
- Persistence, auth, exports.
