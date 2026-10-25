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

## Stage 5 — Unlimited stages

Status (2026-10-25): every ported algorithm matches upstream on every
golden file and passes the checker; what is left is timing against the
oracle and measuring the two decisions below, which need the oracle
built (see `HANDOVER-STAGE5.md`). The owner approved starting Stage 6
(API) and the design of Stage 7 (UI) in parallel meanwhile, since
neither changes the solver.

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

## Stage 6 — Web API

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

## Stage 7 — Web UI

Deliverables (in `web/`, see `ARCHITECTURE.md`):
- Instance editor (parts, panels, saw settings), job submission, live
  progress, cutting-plan viewer (SVG, one panel at a time + overview,
  dimensions on every part and offcut).
- Every input shows its help text from `openapi.json` (i.e. from
  `FIELDS.md`) next to the field, not only on hover, so it also works on
  touch screens in the workshop.
- TypeScript client generated from `openapi.json`.

Exit criteria:
- End-to-end test: enter instance → solve → see a valid plan.
- No input without help text; UI wording matches `FIELDS.md`.

## Later (only after scope decisions)

- Remaining guillotine features marked Open in `SCOPE.md`.
- General hypergraph tree search, if benchmarks show it helps (Open in
  `SCOPE.md`).
- Non-guillotine `rectangle` solver (repeat Stages 3–6 for it).
- Persistence, auth, exports.
