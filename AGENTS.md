# AGENTS.md

Instructions for AI coding agents (and humans) working in this repository.

## What this is

`sheetsolver` is a Rust reimplementation of a **subset** of
[fontanf/packingsolver](https://github.com/fontanf/packingsolver) — the
2D guillotine (`rectangleguillotine`) and, later, the 2D non-guillotine
(`rectangle`) solvers — exposed as a library and a web API, with a web UI.

It is built **for woodworkers** cutting parts from sheet material. Anything
the user sees uses woodworking terms (panel, part, kerf, grain, offcut),
never solver terms (bin, item, profit, cut thickness).

Read these before starting any task:

| File | What it answers |
|---|---|
| [docs/AIMS.md](docs/AIMS.md) | Why this exists, goals and non-goals |
| [docs/SCOPE.md](docs/SCOPE.md) | Which features are in, out, or undecided |
| [docs/STAGES.md](docs/STAGES.md) | Implementation order and the exit criteria of each stage |
| [docs/FIELDS.md](docs/FIELDS.md) | Every user-facing field: label, exact help text, unit, default, upstream mapping |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Repository layout, crates, data flow, API shape |
| [docs/TESTING.md](docs/TESTING.md) | Feasibility checker, differential testing against upstream |
| [docs/UPSTREAM.md](docs/UPSTREAM.md) | Pinned upstream commit and the map of upstream files to port |

## Ground rules

1. **Work stage by stage.** Only implement what the current stage in
   `docs/STAGES.md` asks for. Do not start a later stage until the current
   one meets its exit criteria. If a stage needs to change, update
   `docs/STAGES.md` in the same change and say why.
2. **Scope is closed by default.** If a feature is not listed as *In* in
   `docs/SCOPE.md`, do not implement it. Undecided items stay unimplemented
   until the owner decides; ask instead of guessing.
3. **Port behaviour, not syntax.** Write idiomatic Rust, but the search
   must make the same decisions as upstream (same branching order, same
   tie-breaking, same dominance rules) so differential tests can require
   identical results. When a faithful port would be unidiomatic, keep the
   behaviour and comment the upstream file and function it mirrors:
   `// upstream: src/rectangleguillotine/tree_search.cpp, BranchingScheme::insertion`.
4. **No solution leaves the solver unchecked.** Every solution produced in
   tests goes through the feasibility checker (`docs/TESTING.md`). The
   checker is written independently of the solver and must not import
   solver code.
5. **Readability is a requirement.** The owner reads and maintains this
   code. Prefer clear names over upstream's abbreviations (`l, r, b, t` →
   `left, right, bottom, top`; `df` → `depth`), small functions, and doc
   comments on every public item. Explain *why* in comments, not *what*.
6. **Performance matters, but measure first.** The hot path is node
   expansion in the tree search. Nodes start as `Rc` handles, like
   upstream's `shared_ptr` (see `docs/ARCHITECTURE.md`); move to an arena
   or other allocation tricks only when a benchmark shows it matters.
7. **No `unsafe`** without a written justification in the code and a
   benchmark that requires it.
8. **Every user-facing field has precise help text**, written in
   `docs/FIELDS.md` first and copied into the field's doc comment, which
   becomes its OpenAPI description and the UI's help text. A field
   without help text is not done.
9. **Upstream is read-only reference.** Never vendor upstream source into
   this repo. The C++ oracle in `tools/oracle` builds upstream at the
   pinned commit from `docs/UPSTREAM.md`.

## Conventions

- Rust stable, edition 2024, Cargo workspace under `crates/`.
- Integer geometry: lengths are `i64` counting **tenths of a millimetre** (upstream `Length`), never floats. Convert to and from decimal millimetres only at the JSON boundary (`model`), rejecting more than one decimal. Areas are `i64` in hundredths of mm².
- Errors: `thiserror` in libraries, `anyhow` only in binaries.
- Serialization: `serde`; the JSON shapes in `docs/ARCHITECTURE.md` are the
  public contract — change them deliberately and update the doc.
- Formatting and lints must pass: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`.

## Commands

These become valid as the stages land (see `docs/STAGES.md`).

```bash
cargo build --workspace
cargo test --workspace
cargo test -p checker
cargo bench -p guillotine
cargo run -p cli -- solve data/instances/example.json --time-limit 5
cargo run -p api
```

## Definition of done for any change

- Tests added or updated, and `cargo test --workspace` passes.
- `cargo fmt --check` and `cargo clippy` are clean.
- Docs in `docs/` updated if scope, stages, contracts or behaviour changed.
- For solver changes: differential tests against the oracle still pass
  (see `docs/TESTING.md`).
