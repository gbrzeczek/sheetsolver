# Scope

Status values: **In** (implement), **Later** (decided, but after the first
version — do not implement yet), **Out** (do not implement), **Open**
(owner has not decided — do not implement, ask).

The product is built for **woodworkers** cutting sheet material (plywood,
MDF, chipboard, melamine-faced board) on panel saws, beam saws and track
saws. When a choice of default or wording is unclear, choose what suits
that user. See `AIMS.md` and `FIELDS.md`.

Upstream names are given so the port can be traced back.

## Problem types

| Feature | Upstream | Status |
|---|---|---|
| 2D guillotine cutting | `rectangleguillotine` | **In** |
| 2D non-guillotine packing | `rectangle` | Open (likely a later stage) |
| 1D, 3D box, box stacks, irregular | `onedimensional`, `box`, `boxstacks`, `irregular` | Out |

## Algorithms (guillotine)

| Feature | Upstream | Status |
|---|---|---|
| Iterative beam search over the guillotine branching scheme | `tree_search.cpp` + `treesearchsolver::iterative_beam_search_2` | **In** |
| Instance reduction / preprocessing | `reduction.cpp` | Open |
| Post-processing: minimize stages, group identical bins, sort subplates | `post_process.cpp` | Open |
| Sequential single knapsack and sequential value correction (multi-panel heuristics: fill one panel at a time with a single-panel search) | `optimize.cpp` `optimize_sequential_single_knapsack`, `algorithms/sequential_value_correction.hpp` | **In** (Stage 5) — upstream's route to unlimited stages on several panels |
| Tree search, maximal spaces (single panel, unlimited stages) | `tree_search_maximal_spaces.cpp` | **In** (Stage 5) |
| Column generation (strips and general) | `column_generation*.cpp` | Out |
| Hypergraph tree search, infinite copies (single panel, many copies, unlimited stages) | `tree_search_hypergraph_infinite_copies.cpp` | **In** (Stage 5) |
| Hypergraph tree search, general | `tree_search_hypergraph.cpp` | Open — upstream never selects it automatically; port only if benchmarks show it helps |
| Automatic algorithm selection for the in-scope algorithms | `optimize.cpp` `optimize()` | **In** (Stage 5) |
| Dichotomic search, sequential strips 1D | | Out |
| Dual feasible functions bounds | `dual_feasible_functions.cpp` | Out |
| LP solvers (CLP, HiGHS, Xpress, …) | `columngenerationsolver` | Out |

## Objectives

| Objective | Upstream | Status |
|---|---|---|
| Bin packing (cut all parts from as few panels as possible) | `BinPacking` | **In** |
| Knapsack (choose which parts to cut to maximize total value; value defaults to part area) | `Knapsack` | **In** |
| Open dimension X / Y (one panel side unlimited, minimize the used length) | `OpenDimensionX`, `OpenDimensionY` | **In** |
| Bin packing with leftovers | `BinPackingWithLeftovers` | Later |
| Variable-sized bin packing | `VariableSizedBinPacking` | Later |
| Bin packing with cutting cost, feasibility, open dimension XY | | Out |

## Instance features (guillotine)

| Feature | Upstream field | Status |
|---|---|---|
| Part types: width, height, quantity, value | `ItemType` | **In** |
| Rotation allowed/forbidden per part (grain direction) | `oriented` | **In** |
| Panel types: width, height, quantity, cost | `BinType` | **In** |
| Saw blade (kerf) thickness | `cut_thickness` | **In** |
| Number of stages 2 and 3 | `number_of_stages` | **In** |
| Unlimited stages | `number_of_stages` = unlimited | **In** (Stage 5). Upstream makes unlimited mean exact cuts in any direction at every stage, and solves it with the maximal-spaces and hypergraph searches, not the 3-level tree search. |
| Cut type (exact, non-exact, homogenous) | `CutType` | **In** |
| Cut type roadef2018 | `CutType::Roadef2018` | Out |
| First cuts direction: either (default), along the length, across the length | `first_stage_orientation` (`any`, `horizontal`, `vertical`) | **In** |
| Trims (left/right/bottom/top, soft/hard) | `*_trim`, `TrimType` | Later — first after v1; damaged factory edges are common in wood |
| Minimum waste length | `minimum_waste_length` | Later |
| Defects | `defects` | Later |
| Cut through defects | `cut_through_defects` | Later (with defects) |
| Min/max distance between 1st/2nd stage cuts | `*_distance_*_cuts` | Later |
| Max number of 1st/2nd stage cuts | `maximum_number_*_cuts` | Later |
| Stacks / item ordering constraints | `stack_id` | Out |
| Resources | `resources` | Out |
| Cutting costs, waste cost | `cutting_costs`, `waste_cost` | Out |

## Product features

| Feature | Status |
|---|---|
| Library crate usable without the API | **In** |
| HTTP API: submit job, stream improved solutions, fetch result, cancel | **In** |
| Developer CLI (solve a JSON instance, diff against oracle) | **In** |
| Web UI (instance editor, solution viewer) | **In**, after the API stage |
| Precise help text for every UI field (from `FIELDS.md`) | **In** |
| Edge banding allowance, grain matching across parts, labels | Open |
| Persistence of jobs/instances (database) | Open |
| Authentication / multi-tenant | Open |
| Printing a plan (print stylesheet: one panel per page with its cut order and parts) | **In** (2026-10-25) |
| Keeping the job being edited in the browser (no server storage) | **In** (2026-10-25) |
| Export (PDF, DXF, CSV cut lists) | Open |

## Decisions log

Record every scope decision here with a date, so agents can see why.

- 2026-10-25 — Repository created. Guillotine tree search, library, API
  and dev CLI are in. Everything else open or out as listed above.
- 2026-10-25 — Target audience: woodworkers. First version: objectives
  bin packing, knapsack, open dimension X/Y; constraints part/panel
  quantities, per-part rotation, kerf, number of stages, cut type. Trims,
  minimum waste, defects, machine cut limits, leftovers and variable-sized
  panels moved to Later. Every UI field gets precise help text.
- 2026-10-25 — Stage 1 found that "unlimited" stages is not really
  supported by the in-scope tree search (see the table); moved to Open.
- 2026-10-25 — Unlimited stages back to In, delivered by a new Stage 5
  with the algorithms upstream uses for it: maximal-spaces search,
  hypergraph search with infinite copies, the sequential multi-panel
  heuristics and upstream's automatic algorithm selection. The general
  hypergraph search stays Open.
- 2026-10-25 — Stage 5: the sequential heuristics (single knapsack,
  value correction) solve each panel's subproblem without instance
  reduction, unlike upstream, whose subproblems call `optimize()` with
  reduction on (reduction stays Open). They are selected for unlimited
  stages only; 2- and 3-stage instances with several panels stay on the
  tree search alone, although upstream would add them there too (with
  column generation, which is Out). Both choices are measured at the end
  of Stage 5 so they can be revisited with numbers.
  Measured at the end of Stage 5 (`STAGES.md`): without reduction, the
  same panel counts and at most 1.4 % less value on 4 of 35 instances,
  and faster; kept. The heuristics on 2 and 3 stages would save one
  panel on 1 of 59 instances and 1.7 % value on another; open for the
  owner.
- 2026-10-25 — Stage 5 ports upstream's area bounds and the stop once a
  plan reaches one (a speed-up only: plans cannot change). Its bounds
  from the 1D solver and dual feasible functions stay Out (they need
  its 1D solver, partly column generation).
- 2026-10-25 — Stage 5 ports upstream's `minimize_number_of_stages`
  (`post_process.cpp`) only because the hypergraph search calls it on
  every plan it finds; post-processing as a separate step stays Open.
  Upstream picks the hypergraph search for panels of at most 0.1 m²
  (10⁷ in its units, which are ours): offcut-sized panels, never full
  sheets. Ported as is, so `solve()` picks what upstream picks.
- 2026-10-25 — First cuts direction is In: either (default), along or
  across the panel's length. "Either" shares the search time between the
  two directions (or uses twice the CPU once searches run in parallel);
  the default can change to a fixed direction later if server load
  needs it.
- 2026-10-25 — Lengths have 0.1 mm precision (decimal millimetres in
  JSON, integer tenths inside), so kerfs like 3.2 mm can be entered
  exactly. A part's default value is its area in mm².
- 2026-10-25 — Optional names (owner): a job may have an optional name;
  part and panel names are optional, and the UI fills an empty one with
  its default ("Part 3", "Panel 1"; `FIELDS.md`, *Conventions*). The job
  name lives in the browser until the API carries it (`web/DESIGN.md`
  §8, #21).
- 2026-10-25 — The job name is now carried by the API (`name` on
  `POST /jobs`, echoed in the job status); the UI's browser-only
  fallback is gone.
- 2026-10-25 — Owner's answers to the UI's open questions
  (`web/DESIGN.md` §9): the job being edited stays in the browser, with
  a "New job" button; printing is In as a print stylesheet (one panel
  per page), file exports stay Open; shown sizes group thousands with a
  narrow space (1 040.5); turned parts are labelled as drawn.
