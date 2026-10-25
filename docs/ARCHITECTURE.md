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

Lengths are integers in the instance's unit (e.g. mm). Field names are
`snake_case`. These shapes are the API contract; Stage 1 finalises them.

### Instance

```json
{
  "objective": "bin_packing",
  "parameters": {
    "number_of_stages": 3,
    "cut_type": "non_exact",
    "first_stage_orientation": "vertical",
    "cut_thickness": 0
  },
  "bin_types": [
    { "id": "sheet-2800x2070", "width": 2800, "height": 2070, "copies": 10, "cost": 1 }
  ],
  "item_types": [
    { "id": "door-left", "width": 720, "height": 2000, "copies": 4, "profit": 1, "rotatable": true }
  ]
}
```

User-supplied `id` strings are carried through to the solution; internal
code uses dense integer indices.

### Solution

A tree per used sheet pattern. Every node has an explicit kind.

```json
{
  "metrics": { "bins_used": 3, "waste_area": 123456, "waste_percentage": 4.2, "profit": 42 },
  "bins": [
    {
      "bin_type_id": "sheet-2800x2070",
      "copies": 2,
      "first_cut_orientation": "vertical",
      "root": {
        "kind": "plate",
        "x": 0, "y": 0, "width": 2800, "height": 2070, "depth": 0,
        "children": [
          { "kind": "item", "item_type_id": "door-left", "rotated": false,
            "x": 0, "y": 0, "width": 720, "height": 2000, "depth": 1, "children": [] },
          { "kind": "waste", "x": 720, "y": 0, "width": 30, "height": 2070, "depth": 1, "children": [] }
        ]
      }
    }
  ]
}
```

Kinds: `plate` (intermediate subplate), `item`, `waste`, `residual`
(reusable leftover), `defect`.

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
