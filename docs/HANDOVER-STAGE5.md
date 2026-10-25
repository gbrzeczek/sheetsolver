# Stage 5 hand-over (work in progress)

Written 2026-10-25 so the work can resume in another session. Delete this
file when Stage 5 is marked done in `STAGES.md`.

## Decisions taken with the owner (recorded in `SCOPE.md`)

- The sequential heuristics solve their subproblems **without reduction**
  (upstream reduces them). The oracle does the same. Measure the cost
  (step 7 below).
- The sequential heuristics run for **unlimited stages only**. 2- and
  3-stage instances with several panels stay on the tree search. Measure
  what adding the heuristics there would give (step 7).
- The hypergraph search's 0.1 m² panel limit is ported as is.

## Done (all uncommitted work is in the WIP commit that adds this file)

1. **Oracle** (`tools/oracle/main.cpp`): `--algorithm tree_search |
   maximal_spaces | hypergraph | sequential_single_knapsack |
   sequential_value_correction | auto`, plus `--reduce-subproblems` for
   measuring. `--benchmark R` works with every algorithm. A
   `SHEETSOLVER_TRACE=1` environment variable prints each subproblem of the
   sequential heuristics, for debugging. `tree_search` output is byte-identical
   to the old golden files.
   - `run_oracle.py --algorithm A` picks the instances A applies to.
   - The golden files are generated: 38 maximal spaces, 19 hypergraph,
     35 each for the two sequential heuristics, 73 auto, and tree search
     at q64/q2048 for every instance. Naming is in `TESTING.md`.
2. **Instances**: `generate_instances.py` adds 42 `generated__unlimited_*`
   instances (single panel, several panels, small parts, many copies on
   small panels). The existing files are unchanged.
3. **Checker**: `crates/checker/tests/unlimited.rs`: deep plans with
   turning cuts and pass-through pieces are accepted, and they break a
   3-stage limit. The checker's golden test handles the new names, the
   empty knapsack plans, and the open-dimension defect (see below).
4. **Ports**:
   - `treesearch/src/child_by_child.rs`: upstream `iterative_beam_search`,
     global history only.
   - `Stop.flags` and `Stop::with_flag`: the end flags of enclosing
     searches.
   - `guillotine` modules: `maximal_spaces.rs` (blocks, scheme, plan
     replay, sequential feasibility), `hypergraph.rs`, `restage.rs`
     (`minimize_number_of_stages`), `sequential.rs` (both heuristics),
     `selection.rs` (upstream's `optimize()` selection and run order),
     `tree_search.rs` (moved out of `lib.rs`), `cpp_sort.rs` (libc++
     `std::sort` exactly, because upstream's ties depend on it), and
     `open_dimension.rs`.
   - `plan.rs` gained repeat counts, `append_bin`, and fallible builder
     operations.
   - `problem.rs` gained an internal `Objective` with Feasibility, panel
     costs, unlimited stages resolved as upstream does, and
     `number_of_stacks`.
   - Public API: `Algorithm`, `select_algorithms`, `solve_with_algorithm`,
     `complete_open_dimension`, `SolveParams.maximal_spaces_queue_size`,
     `SolveOutput.algorithms`, and `SolveError::NotApplicable`.
     `UnlimitedStages` is gone.
5. **`diff-oracle`** reads the algorithm and parameters from each golden
   file's `oracle` header. It skips the 4 `auto` files whose best plan is
   column generation's (Out). `SHEETSOLVER_DUMP=dir` writes our plan when
   it differs. `crates/guillotine/tests/golden.rs` does the same per
   algorithm, and checks the selection against the `auto` improvement
   labels. The slow open-dimension files only run in release builds.

## Results so far

- Maximal spaces: 38/38 identical. Hypergraph and sequential (70 files):
  identical after the fixes below. Tree search: 216 still identical.
- **The full suite was rerun after the last fixes, but its result is not
  yet known.** Rerun it first: `cargo run --release -p cli -- diff-oracle
  --quiet`, about 10 minutes.

## Findings (all documented in `TESTING.md` / code comments)

- **Open dimensions:** upstream copies a plan cut on a shorter panel onto
  the full panel and leaves the rest out of the tree. The port reproduces
  that, then `complete_open_dimension` adds the cut and a leftover. The
  tests apply the same completion to the golden plan.
- **Repeated last panel with a leftover:** written as two patterns (the
  repeats but the last with an offcut, then the last), in both the oracle
  and the port.
- **Stack ids:** a subproblem's part types keep their original stack ids.
  Without part types 0..k−1, upstream's selection falls back to the tree
  search. That was why the sequential heuristics differed.
- **Empty knapsack plans** are feasible to upstream.

## Left to do

1. Rerun the full `diff-oracle` and fix anything left. Then run
   `cargo test --workspace`, debug, plus release for the guillotine golden
   test.
2. **Performance (exit criterion ~1.2×):** `compute_blocks` was the hot
   spot. It is now at parity on
   `generated__open_dimension_x_unlimited_exact_kerf3_2` (19.0 s against
   19.8 s for upstream). The last profile shows `search` itself dominant,
   plus SipHash in `HashSet`/`HashMap`; a faster hasher is an option
   (`ARCHITECTURE.md` allows it). To do:
   - Write `crates/guillotine/benches/instances_unlimited.txt`, one
     `<algorithm> <instance stem>` per line: one or two moderate instances
     per algorithm. `solve.rs` and `bench_oracle.py` already read it.
   - Run `cargo bench -p guillotine` against
     `python3 -I tools/bench_oracle.py tools/oracle/build/sheetsolver_oracle`,
     and record the ratios.
3. **Measure the decisions:** run
   `python3 -I tools/measure_decisions.py tools/oracle/build/sheetsolver_oracle`
   and record the numbers in `STAGES.md` (and `SCOPE.md` if they argue for
   revisiting a decision).
4. Mark Stage 5 done in `STAGES.md`, with a result note in the style of
   Stages 0–4 (counts, deviations, benchmarks). Update the `diff-oracle`
   help text in `crates/cli/src/main.rs`, which still says unlimited
   stages are skipped. Fix the `diff_oracle_skips_unlimited_stages` test
   in `crates/cli/tests/commands.rs`: those files are now compared. Then
   update `README.md`'s status.
5. Final: `cargo fmt --check`, `cargo clippy --workspace --all-targets --
   -D warnings`, `cargo test --workspace`. Commit; do not push without the
   owner's go-ahead.

## Building the oracle in a fresh checkout

See `tools/README.md`. The first build downloads upstream and HiGHS
(several minutes).
