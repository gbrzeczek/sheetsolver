# Upstream reference

- Repository: https://github.com/fontanf/packingsolver (MIT license)
- Pinned commit: `9b656a0dcafd1e9019bf39a5910d4307af65579a` (2026-10-25)

Bump the pin deliberately: regenerate golden files, review upstream
changes in the files below, and record the bump here.

## License

Upstream is MIT. A Rust port is a derivative work: keep upstream's
copyright notice in this repository's `LICENSE`/`NOTICE` alongside ours.

## Files to port (guillotine, in-scope)

| Upstream | Purpose | Target crate |
|---|---|---|
| `include/packingsolver/rectangleguillotine/instance.hpp`, `src/rectangleguillotine/instance.cpp` | Instance model, parameters, enums | `model` |
| `src/rectangleguillotine/instance_builder.cpp` | Validation and derived data | `model` |
| `include/packingsolver/rectangleguillotine/solution.hpp`, `src/rectangleguillotine/solution.cpp`, `solution_builder.cpp` | Solution tree and metrics | `model` (structure), `guillotine` (building) |
| `src/rectangleguillotine/tree_search.hpp`, `tree_search.cpp` | Branching scheme: the core of the solver | `guillotine` |
| `src/rectangleguillotine/optimize.cpp` | Algorithm selection and parameters (port only the tree-search path) | `guillotine` |
| `src/rectangleguillotine/instance_flipper.hpp`, `instance_flipper.cpp` | Swaps length and width, for first cuts along the length | `guillotine` |
| `treesearchsolver` (separate repo): `iterative_beam_search_2` | Generic beam search | `treesearch` |
| `include/packingsolver/algorithms/common.hpp` | `Objective`, `OptimizationMode`, shared types | `model` |

## Files to port in Stage 5 (unlimited stages)

| Upstream | Purpose | Target crate |
|---|---|---|
| `src/rectangleguillotine/tree_search_maximal_spaces.hpp`, `.cpp`, `block.cpp`, `sequential_feasibility.cpp` | Single-panel search for unlimited stages, its blocks, and its bin packing and open-dimension driver | `guillotine` (`maximal_spaces.rs`) |
| treesearchsolver `iterative_beam_search.hpp` (global history only) | Beam search generating children one at a time | `treesearch` (`child_by_child.rs`) |
| `src/rectangleguillotine/tree_search_hypergraph_infinite_copies.hpp`, `.cpp`; `post_process.cpp` `minimize_number_of_stages` (which it calls) | Single-panel dynamic programming for many copies | `guillotine` (`hypergraph.rs`, `restage.rs`) |
| libc++ `__algorithm/sort.h` (`std::sort`) | Upstream's order of tied candidates | `guillotine` (`cpp_sort.rs`) |
| `src/algorithms/sequential_value_correction.hpp`, `optimize.cpp` `optimize_sequential_single_knapsack` | Several panels, one at a time | `guillotine` |
| `src/rectangleguillotine/optimize.cpp` `optimize()` | Automatic algorithm selection | `guillotine` |

## Ported in Stage 9 (trims, minimum offcut width)

| Upstream | What | Where |
|---|---|---|
| `instance.hpp` `BinType::*_trim`, `TrimType`, `Instance::trim_start`/`x_start`/`y_start`, `BinType::area` (inside the trims) | Trims and where parts start | `model` (`Trims`), `guillotine` (`problem.rs`) |
| `instance_flipper.cpp` | Trims of the flipped instance | `guillotine` (`BinTrims::flipped`) |
| `tree_search.cpp` `bin_borders`, `x1_max`, `y2_max`, `x3_prev`/`y2_prev`, `child_tmp` (areas), `update` (the minimum waste with `z1`/`z2`, soft trim borders) | The search with trims and a minimum waste | `guillotine` (`scheme.rs`) |
| `tree_search.cpp` `to_solution` (soft trim cuts), `solution_builder.cpp` `add_bin` (hard trims) | Plans with trims | `guillotine` (`plan.rs`) |
| `optimize.cpp` `optimize` (no one-panel search with a minimum waste), `tree_search.cpp` (no exhaustive stop with one) | Selection | `guillotine` (`selection.rs`, `tree_search.rs`) |
| `solution.cpp` `trims_feasible`, `minimum_waste_length_feasible` (see `SCOPE.md` for the one difference) | The rules | `checker` |

Read but do not port unless scope changes: `reduction.cpp`,
`post_process.cpp`, `tree_search_hypergraph.cpp`.

## Useful upstream assets

- `data/rectangleguillotine/` — instances (~2 MB), source of
  `data/instances/` after conversion.
- `test/rectangleguillotine/` — upstream unit tests; port relevant cases.
- `wasm/src/packingsolver_wasm.cpp` — compact example of calling
  `optimize()` from C++; a good template for `tools/oracle`.
- `python/src/rectangleguillotine.cpp` — bindings showing which fields of
  `Solution`/`SolutionNode` matter.

## Upstream CLI flags we deliberately drop

Duplicate spellings (`--min1cut` vs `--minimum-distance-1-cuts`), CSV
inputs (`--items/--bins/--defects/--parameters`), certificate output,
`--seed` (unused), and the `--use-*` / `--not-anytime-*` algorithm knobs
for out-of-scope algorithms.
