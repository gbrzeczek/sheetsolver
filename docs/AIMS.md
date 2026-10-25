# Aims

## Why

packingsolver produces excellent 2D cutting plans, but for building a
product on top of it:

- its CLI and file formats (CSV inputs, CSV "certificate" outputs with
  magic type codes, layout only reconstructable from parent pointers) are a
  poor contract for a web API and frontend;
- most of its code (other problem types, column generation, Benders
  decomposition, LP solver backends) is irrelevant to our use;
- the owner needs to read, understand and modify the solver, and is more
  productive in Rust than in C++.

## Goals

1. **Solution quality parity** with upstream on the in-scope features: in
   deterministic mode, identical solutions; under a time limit, no worse
   on the benchmark set (see `TESTING.md`).
2. **Performance parity**: within ~1.2× of upstream wall-clock for the same
   number of expanded nodes.
3. **One typed boundary**: Rust types with `serde` are the instance and
   solution format; the API serves them directly. No CSV, no subprocesses.
4. **Solutions as trees**: sheets → cuts → pieces/waste/defects, with
   explicit kinds, ready for the UI to draw.
5. **Anytime + streaming**: the solver reports each improved solution;
   the API streams them to the client; jobs can be cancelled.
6. **Testability**: an independent feasibility checker, differential tests
   against the C++ oracle, property-based tests, benchmarks.
7. **Readable code** the owner can maintain without upstream knowledge.

## Non-goals

- Feature parity with packingsolver as a whole.
- Other problem types (1D, 3D box, box stacks, irregular).
- Exact methods, LP/MIP solvers, column generation, Benders.
- Compatibility with upstream's CSV/CLI formats (the oracle harness
  converts instead).
- Running the solver in the browser (WebAssembly) — possible later, not
  designed for now.
