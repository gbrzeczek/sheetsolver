# Scope

Status values: **In** (implement), **Out** (do not implement), **Open**
(owner has not decided — do not implement, ask).

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

| Objective | Status |
|---|---|
| Bin packing (minimize number/cost of sheets) | Open |
| Bin packing with leftovers | Open |
| Knapsack (maximize profit of placed items) | Open |
| Open dimension X / Y (strip packing) | Open |
| Variable-sized bin packing | Open |
| Bin packing with cutting cost, feasibility | Out |

## Instance features (guillotine)

| Feature | Upstream field | Status |
|---|---|---|
| Item types: width, height, copies, profit | `ItemType` | **In** |
| Item rotation allowed/forbidden per item | `oriented` | Open |
| Bin types: width, height, copies, cost | `BinType` | **In** |
| Trims (left/right/bottom/top, soft/hard) | `*_trim`, `TrimType` | Open |
| Defects | `defects` | Open |
| Number of stages (2, 3, unlimited) | `number_of_stages` | Open |
| Cut type (exact, non-exact, homogenous, roadef2018) | `CutType` | Open |
| First stage orientation | `first_stage_orientation` | Open |
| Saw blade (cut) thickness | `cut_thickness` | Open |
| Min/max distance between 1st/2nd stage cuts | `*_distance_*_cuts` | Open |
| Minimum waste length | `minimum_waste_length` | Open |
| Max number of 1st/2nd stage cuts | `maximum_number_*_cuts` | Open |
| Cut through defects | `cut_through_defects` | Open |
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
| Persistence of jobs/instances (database) | Open |
| Authentication / multi-tenant | Open |
| Export (PDF, DXF, CSV cut lists) | Open |

## Decisions log

Record every scope decision here with a date, so agents can see why.

- 2026-10-25 — Repository created. Guillotine tree search, library, API
  and dev CLI are in. Everything else open or out as listed above.
