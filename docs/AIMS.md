# Aims

## Who it is for

**Woodworkers** — cabinet makers, joiners, furniture workshops and serious
hobbyists — who cut parts from sheet material (plywood, MDF, chipboard,
melamine-faced board, solid-wood panels) on a panel saw, beam saw, table
saw or track saw, and want a cutting plan that wastes as little board as
possible and can actually be cut.

What this means for the product:

- **Woodworking vocabulary** in everything the user sees: *panel* (not
  bin), *part* (not item), *kerf*, *grain direction*, *offcut*, *cutting
  plan*. Upstream terms stay inside the code and the port comments.
  `FIELDS.md` is the mapping.
- **Millimetres** as the unit, with 0.1 mm precision (enough for blade thicknesses like 3.2 mm).
- **Defaults that suit a workshop**: rotation allowed unless the part has
  grain, 3 stages, no kerf (0 mm) until the user enters their blade.
- **Plans a person can follow at the saw**: guillotine cuts only, cut
  order and dimensions shown clearly.
- **Every field explained precisely** in the UI (see `FIELDS.md`): what it
  means, its unit, its effect on the plan, and a typical value.

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
