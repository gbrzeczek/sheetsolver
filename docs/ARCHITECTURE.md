# Architecture

## Repository layout

```
sheetsolver/
├── AGENTS.md
├── docs/                  # aims, scope, stages, architecture, testing, upstream
├── crates/
│   ├── model/             # Instance, Solution, parameters; serde; validation
│   ├── checker/           # independent feasibility checker + metrics
│   ├── treesearch/        # generic iterative beam search
│   ├── guillotine/        # guillotine branching scheme + solve()
│   ├── cli/               # developer CLI (solve, check, diff-oracle, bench-set)
│   └── api/               # axum web API
├── tools/                 # see tools/README.md
│   ├── oracle/            # C++ harness around upstream packingsolver
│   ├── convert_instances.py
│   ├── generate_instances.py
│   └── run_oracle.py
├── data/
│   ├── instances/         # test instances in our JSON format (+ MANIFEST.json)
│   └── golden/<commit>/   # oracle solutions, one per instance and queue size
└── web/                   # UI (Stage 7)
```

Dependency direction (no cycles, no shortcuts):

```
model ← checker
model ← treesearch ← guillotine ← cli, api
checker ← cli, tests
```

`checker` must never depend on `treesearch` or `guillotine`.

## UI in the same repository

The UI lives in `web/` in this repository, decoupled by **contract**, not
by repository:

- The UI talks to the solver only through the HTTP API.
- The TypeScript client is generated from `openapi.json`, which is
  generated from the Rust types — one source of truth, and a type change
  that breaks the UI fails in the same change.
- One developer and AI agents benefit from seeing both sides at once.

Split `web/` into its own repository if it gets its own team, release
cycle, or starts talking to other backends. Nothing in this layout makes
that split hard.

## Data contracts (draft)

Lengths are millimetres with at most one decimal (`3.2`, `720`) in JSON,
and `i64` tenths of a millimetre inside the code (`3.2` → `32`). The
conversion happens only in `model`'s (de)serialization, which rejects
values with more than one decimal. Scaling every length by 10 does not
change any decision the search makes; the one place units leak is a part's
default value, which is defined as its area in mm² (not in internal units)
so that parts with and without an explicit value compare sensibly. Field names are `snake_case` and use
the woodworking terms from `FIELDS.md` (panel, part, length, width,
quantity, grain, value, kerf), not upstream's (bin, item, width, height,
copies, oriented, profit, cut thickness). The mapping to upstream lives
in one place: the `guillotine` crate's conversion from `model` types.
These shapes are the API contract; Stage 1 finalises them.

### Instance

```json
{
  "objective": "bin_packing",
  "saw": {
    "kerf": 3.2,
    "stages": 3,
    "cut_type": "non_exact",
    "first_cuts": "either"
  },
  "panels": [
    { "id": "Birch ply 18 mm", "length": 2500, "width": 1250, "quantity": 10 }
  ],
  "parts": [
    { "id": "Side left", "length": 720, "width": 560, "quantity": 4, "grain": true }
  ]
}
```

### Field help text

`FIELDS.md` holds the help text for every user-facing field. It flows to
the UI through the types, so it cannot drift:

```
FIELDS.md  →  /// doc comment on the model field  →  utoipa  →  openapi.json (description)  →  UI tooltip / help
```

A test in the `api` crate fails if any field in `openapi.json` has no
description.

User-supplied `id` strings are carried through to the solution; internal
code uses dense integer indices.

### Solution

A tree per cutting pattern; `repeat` says how many panels are cut the
same way, `first_cuts` the direction of the first, full-panel cuts
(`across_length`: at a fixed `x`; `along_length`: at a fixed `y`). Each
stage turns the piece, so the next stage cuts the other way. `x` runs
along the panel's length, `y` along its width. Leaves have no `children`
key.

```json
{
  "patterns": [
    {
      "panel_id": "Birch ply 18 mm",
      "repeat": 2,
      "first_cuts": "across_length",
      "root": {
        "kind": "piece", "x": 0, "y": 0, "length": 2500, "width": 1250, "stage": 0,
        "children": [
          { "kind": "piece", "x": 0, "y": 0, "length": 720, "width": 1250, "stage": 1, "children": [
            { "kind": "part", "part_id": "Side left", "rotated": false,
              "x": 0, "y": 0, "length": 720, "width": 560, "stage": 2 },
            { "kind": "part", "part_id": "Side left", "rotated": false,
              "x": 0, "y": 563.2, "length": 720, "width": 560, "stage": 2 },
            { "kind": "offcut", "x": 0, "y": 1126.4, "length": 720, "width": 123.6, "stage": 2 }
          ]},
          { "kind": "offcut", "x": 723.2, "y": 0, "length": 1776.8, "width": 1250, "stage": 1 }
        ]
      }
    }
  ]
}
```

Kinds: `piece` (the panel, or an intermediate piece that is cut
further), `part`, `offcut` (waste), `leftover` (reusable remainder of the
last panel: only as its last first-stage piece). Defects and trims get
kinds when they move to In.

The tree rules (children side by side in the stage's cut direction, each
spanning its parent, exactly one kerf apart, at most one kerf left at the
end) are the checker's (`crates/checker`).

### Metrics

Computed by the checker from an instance and a solution, not stored in
the solution: `panels_used`, `parts_cut`, `panel_area`, `part_area`,
`leftover_area`, `waste_area` (areas in mm²), `waste_fraction` (0–1, of
the panel area without the leftover), `total_value`, and `used_length` /
`used_width` (how far the cuts reach on the last panel). Their
definitions match upstream's, and the golden tests compare them.

## Solver interface

```rust
pub fn solve(
    instance: &Instance,
    params: &SolveParams,           // queue size; stop: deadline, cancellation flag
    on_solution: impl FnMut(&Solution),  // each strictly better plan
) -> Result<SolveOutput, SolveError>;    // best plan, nodes expanded
```

`SolveError` is an invalid instance (or, for `solve_with_algorithm`, an
algorithm that does not apply). `solve` runs the algorithms upstream's
`optimize()` would select, limited to the In ones (`select_algorithms`),
in upstream's deterministic mode, and keeps the first strictly better
plan: the tree search for 2 and 3 stages; with unlimited stages, the
maximal-spaces search (one panel), the hypergraph search (one small panel,
many copies, knapsack), or the tree search and the sequential heuristics
(several panels). `solve_with_algorithm` runs one of them, as the oracle's
`--algorithm` does. See `crates/guillotine/src/lib.rs`.

Single-threaded first. Parallelism (several searches with different
settings, as upstream's anytime mode does) is added later behind the same
interface.

## Hot path guidelines

- The `treesearch` crate is generic over the node type
  (`BranchingScheme::Node`, any cheap `Clone` handle), so each scheme
  picks its representation. Schemes start with `Rc<Node>` and parent
  `Rc`s, as upstream does with `shared_ptr`: discarded children are freed
  at once. A plain arena would keep every generated node until the end
  of an iteration (millions at large queue sizes); an arena with reuse is
  the option to benchmark if allocation shows up in profiles.
- Queues are `BTreeMap`s keyed by the scheme's `Priority` (guide value,
  then node id), matching upstream's `std::set` ordered by its comparator.
- Small fixed-size collections (subplate fronts, stage positions) use
  arrays or `smallvec`.
- Dominance hashing uses a fast non-cryptographic hasher (`rustc-hash` /
  `ahash`) — but tie-breaking must not depend on hash iteration order.
