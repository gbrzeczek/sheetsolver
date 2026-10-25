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
| `treesearchsolver` (separate repo): `iterative_beam_search_2` | Generic beam search | `treesearch` |
| `include/packingsolver/algorithms/common.hpp` | `Objective`, `OptimizationMode`, shared types | `model` |

Read but do not port unless scope changes: `reduction.cpp`,
`post_process.cpp`, `algorithms/sequential_value_correction.hpp`.

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
