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
│   ├── cli/               # developer CLI (solve, check, diff-oracle)
│   └── api/               # axum web API
├── tools/
│   └── oracle/            # C++ harness around upstream packingsolver
├── data/
│   └── instances/         # test instances in our JSON format
└── web/                   # UI (Stage 6)
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

Lengths are integers in millimetres. Field names are `snake_case` and use
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
    "kerf": 4,
    "stages": 3,
    "cut_type": "non_exact"
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
same way. Every node has an explicit kind. `x` runs along the panel's
length, `y` along its width.

```json
{
  "metrics": { "panels_used": 3, "waste_area": 123456, "waste_percentage": 4.2, "total_value": 42 },
  "patterns": [
    {
      "panel_id": "Birch ply 18 mm",
      "repeat": 2,
      "root": {
        "kind": "piece",
        "x": 0, "y": 0, "length": 2500, "width": 1250, "stage": 0,
        "children": [
          { "kind": "part", "part_id": "Side left", "rotated": false,
            "x": 0, "y": 0, "length": 720, "width": 560, "stage": 1, "children": [] },
          { "kind": "offcut", "x": 724, "y": 0, "length": 1776, "width": 1250, "stage": 1, "children": [] }
        ]
      }
    }
  ]
}
```

Kinds: `piece` (intermediate piece that is cut further), `part`,
`offcut` (waste), `leftover` (reusable remainder of the last panel),
`defect`.

## Solver interface

```rust
pub fn solve(
    instance: &Instance,
    params: &SolveParams,          // time limit, queue sizes, deterministic, …
    cancel: &CancellationToken,
    on_solution: impl FnMut(&Solution),
) -> SolveOutput;                   // best solution, bound, stats
```

Single-threaded first. Parallelism (several searches with different
settings, as upstream's anytime mode does) is added later behind the same
interface.

## Hot path guidelines

- Search nodes live in an arena (`Vec<Node>`) and refer to parents by
  index; no `Rc`/`Box` per node.
- Small fixed-size collections (subplate fronts, stage positions) use
  arrays or `smallvec`.
- Dominance hashing uses a fast non-cryptographic hasher (`rustc-hash` /
  `ahash`) — but tie-breaking must not depend on hash iteration order.
