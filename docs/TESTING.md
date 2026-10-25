# Testing

The port is only cheap if it is verifiable. Three layers, in order of
importance.

## 1. Feasibility checker (`crates/checker`)

An independent judge of any `(Instance, Solution)` pair.

- Written from the problem definition and `SCOPE.md`, **not** from solver
  code. It must not depend on `treesearch` or `guillotine`.
- Checks: the root is the whole panel; children sit side by side in the
  stage's cut direction, span their parent, start exactly one kerf after
  the previous child, and leave at most one kerf at the end; stages
  increase by one per level; parts have their part's size (rotation only
  without grain); stage count and cut type as upstream defines them; the
  leftover only at the end of the last panel; panel and part quantities;
  plus every other **In** constraint as it is added.
- The definitions of stages and cut types follow upstream's own solution
  check (`src/rectangleguillotine/solution.cpp`), because that is the
  problem we port; the code is written independently of our solver.
- Returns all violations, not just the first, each with the node path.
- Computes metrics independently (bins used, waste, profit); tests compare
  them with the solver's own metrics.

Every new constraint in scope ships with checker support **and** at least
one negative test before the solver supports it.

## 2. Differential testing against the oracle (`tools/oracle`)

Upstream packingsolver at the pinned commit is the reference.

- Deterministic mode: upstream `OptimizationMode::NotAnytimeSequential`
  with fixed queue sizes. Rust must produce the **same solution** (same
  tree, same metrics), not just an equally good one. This catches subtle
  porting errors in branching order, tie-breaking and dominance.
- Oracle outputs are generated once per pinned commit and committed as
  golden files under `data/golden/<first 8 characters of commit>/`, named
  `<instance>.q<queue size>.json`, at queue sizes 64 and 2048, so
  `cargo test` does not need a C++ toolchain. Regenerating them is a
  separate step, documented in `tools/README.md`.
- `cargo run --release -p cli -- diff-oracle` runs the whole comparison
  and prints a report, one line per golden file; `cargo test` runs the
  same comparison in `crates/guillotine/tests/golden.rs`.
- Golden files keep upstream's own metrics under `upstream_metrics`; the
  checker's metrics are compared against them in Stage 1.
- A golden file with no patterns means upstream found no solution; the
  Rust port must also find none.
- **Bounds (verified in Stage 3):** upstream computes bounds before the
  tree search and stops the search once a solution reaches one. The Rust
  port does not compute them. This cannot change the best solution: a
  later solution only replaces the best one if it is strictly better, and
  none can be better than a valid bound. Stage 3 confirmed it on every
  2- and 3-stage instance: the port, which never stops early, gives the
  golden plans, and so does upstream's `tree_search()` run alone
  (`sheetsolver_oracle --benchmark`) on the benchmark instances. The one
  early stop the port keeps is upstream's own inside `tree_search()`: once
  a search with a fixed first-cut direction proves its result optimal,
  the remaining searches are skipped.
- When the Rust port intentionally deviates (a bug fix, a scope
  difference), the test for that instance is marked with the reason and
  checked against the checker plus a quality bound instead.

## 3. Property-based and quality tests

- `proptest` generators for random valid instances (small sizes):
  solver output always passes the checker; solving never panics; a
  solution with all items is found when a trivially feasible one exists.
- Quality under time limit: on the benchmark set, best-found objective is
  no worse than the oracle's golden anytime result (run in CI nightly or
  manually, not on every commit).

## Benchmarks

- `criterion` benchmarks in `crates/guillotine/benches/` run
  `guillotine::solve` on the instances listed in `instances.txt`, at the
  golden queue sizes (`cargo bench -p guillotine`).
- `tools/bench_oracle.py` times upstream's `tree_search()` on the same
  instances (the oracle's `--benchmark` mode: no bounds, no JSON parsing
  in the timing) and checks each plan against the golden file, so both
  sides time the same work.
- Do not merge a hot-path change that regresses benchmarks by >5% without
  a reason in the commit message.
