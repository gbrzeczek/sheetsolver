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
| Sequential single knapsack / sequential value correction (multi-bin heuristics) | `algorithms/sequential_value_correction.hpp` | Open — needed if bin-packing quality depends on it |
| Tree search, maximal spaces | `tree_search_maximal_spaces.cpp` | Out (for now) |
| Column generation (strips and general) | `column_generation*.cpp` | Out |
| Hypergraph tree searches | `tree_search_hypergraph*.cpp` | Out |
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
| Number of stages (2, 3, unlimited) | `number_of_stages` | **In** |
| Cut type (exact, non-exact, homogenous) | `CutType` | **In** |
| Cut type roadef2018 | `CutType::Roadef2018` | Out |
| First stage orientation | `first_stage_orientation` | Open — needed to express "first cuts along the grain"; decide before Stage 3 |
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
- 2026-10-25 — Lengths have 0.1 mm precision (decimal millimetres in
  JSON, integer tenths inside), so kerfs like 3.2 mm can be entered
  exactly. A part's default value is its area in mm².
