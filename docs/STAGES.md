# Implementation stages

Each stage has a goal, deliverables, and exit criteria. A stage is done
only when every exit criterion holds. Mark stages done here, with the
date, as they land.

---

## Stage 0 — Skeleton and oracle

**Goal:** a workspace that builds, and a way to get reference answers from
upstream.

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
- `model` crate: `Instance`, `ItemType`, `BinType`, parameters and
  `Solution` (tree: bins → nodes with explicit `NodeKind`), with `serde`
  and validation (`Instance::validate`).
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
- OpenAPI spec generated from the Rust types (`utoipa`).

Exit criteria:
- Integration tests: submit, stream, cancel, invalid instance → 422.
- Solutions served by the API pass the checker.

## Stage 6 — Web UI

Deliverables (in `web/`, see `ARCHITECTURE.md`):
- Instance editor (items, sheets, parameters), job submission, live
  progress, solution viewer (SVG, one sheet at a time + overview).
- TypeScript client generated from `openapi.json`.

Exit criteria:
- End-to-end test: enter instance → solve → see a valid plan.

## Later (only after scope decisions)

- Remaining guillotine features marked Open in `SCOPE.md`.
- Multi-bin heuristics (sequential value correction) if needed for quality.
- Non-guillotine `rectangle` solver (repeat Stages 3–5 for it).
- Persistence, auth, exports.
