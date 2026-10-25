# Testing

The port is only cheap if it is verifiable. Three layers, in order of
importance.

## 1. Feasibility checker (`crates/checker`)

An independent judge of any `(Instance, Solution)` pair.

- Written from the problem definition and `SCOPE.md`, **not** from solver
  code. It must not depend on `treesearch` or `guillotine`.
- Checks: node geometry inside parent; children tile the parent along one
  cut orientation (guillotine structure); depth/stage limits; cut
  thickness between siblings; item dimensions match an item type (with
  rotation only if allowed); copies used ≤ copies available; bin copies;
  plus every other **In** constraint as it is added.
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
  golden files under `data/golden/<commit>/`, so `cargo test` does not
  need a C++ toolchain. Regenerating them is a separate, documented step.
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

- `criterion` benchmarks in `crates/guillotine/benches/` on a few fixed
  instances and queue sizes; compare nodes/second with the oracle.
- Do not merge a hot-path change that regresses benchmarks by >5% without
  a reason in the commit message.
