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

## Stage 1 — Model and feasibility checker

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

## Stage 2 — Generic iterative beam search

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

## Stage 3 — Guillotine branching scheme

**Goal:** the actual solver, behaviour-identical to upstream.

Deliverables:
- `guillotine` crate: port of `rectangleguillotine` `BranchingScheme`
  (insertions, subplates, front, guides, bounds, dominance) for the **In**
  features only, plus conversion from search nodes to `model::Solution`.
- `solve(&Instance, &Params, on_solution) -> Output`.
- Criterion benchmarks on a few representative instances.

Exit criteria:
- Differential test: in deterministic mode with fixed queue sizes, the
  solution equals the oracle's on every instance in `data/instances/`.
- All solutions pass the checker.
- Benchmarks within ~1.2× of the oracle for the same queue sizes.

## Stage 4 — Developer CLI

Deliverables:
- `cli` crate: `solve`, `check`, `diff-oracle`, `bench-set` subcommands.

Exit criteria:
- One command runs the full differential suite and prints a report.

## Stage 5 — Web API

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

## Stage 6 — Web UI

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
- Multi-bin heuristics (sequential value correction) if needed for quality.
- Non-guillotine `rectangle` solver (repeat Stages 3–5 for it).
- Persistence, auth, exports.
