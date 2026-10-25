//! Which algorithms solve an instance, and running them.
//!
//! upstream: src/rectangleguillotine/optimize.cpp, `optimize`: the
//! automatic selection and the order the selected algorithms run in.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use treesearch::Stop;

use crate::SolveParams;
use crate::bounds::Bound;
use crate::hypergraph::tree_search_hypergraph_infinite_copies;
use crate::maximal_spaces::tree_search_maximal_spaces;
use crate::plan::Plan;
use crate::problem::{Objective, Problem};
use crate::sequential::{Variant, sequential_value_correction};
use crate::tree_search::tree_search;

/// An algorithm of upstream's that is In (see `docs/SCOPE.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Algorithm {
    /// The tree search over 3 levels of strips (upstream `tree_search`):
    /// 2 and 3 stages; with unlimited stages, next to the sequential
    /// heuristics for several panels.
    TreeSearch,
    /// Placing blocks of parts in the smallest empty space (upstream
    /// `tree_search_maximal_spaces`): one panel, unlimited stages.
    MaximalSpaces,
    /// Dynamic programming over the cuts, ignoring the part quantities
    /// (upstream `tree_search_hypergraph_infinite_copies`): one small panel,
    /// many copies of each part, value objective, unlimited stages.
    Hypergraph,
    /// Fill one panel at a time with the most valuable parts (upstream
    /// `optimize_sequential_single_knapsack`): several panels, many parts
    /// per panel.
    SequentialSingleKnapsack,
    /// Fill one panel at a time, then retry with the parts' values
    /// corrected by the waste they caused (upstream
    /// `optimize_sequential_value_correction`): several panels.
    SequentialValueCorrection,
}

/// upstream: optimize.hpp, `OptimizeParameters::many_items_in_bins_threshold`.
const MANY_ITEMS_IN_BINS_THRESHOLD: i64 = 16;
/// upstream: `OptimizeParameters::many_item_type_copies_factor`.
const MANY_ITEM_TYPE_COPIES_FACTOR: i64 = 1;
/// upstream: `OptimizeParameters::tree_search_hypergraph_infinite_copies_maximum_bin_area`,
/// in hundredths of mm²: a panel of at most 0.1 m².
const HYPERGRAPH_MAXIMUM_BIN_AREA: i64 = 10_000_000;

/// How many parts fit in the largest panel, by area, rounded down.
///
/// upstream: optimize.cpp, `mean_number_of_items_in_bins`, from
/// `largest_bin_space` and `mean_item_space`.
fn mean_number_of_items_in_bins(problem: &Problem) -> i64 {
    let largest_bin_space = problem
        .bin_types
        .iter()
        .map(|bin| bin.size.area() as f64)
        .fold(0.0, f64::max);
    let mean_item_space = problem
        .item_types
        .iter()
        .map(|item| f64::from(item.copies) * item.size.area() as f64)
        .sum::<f64>()
        / f64::from(problem.number_of_items);
    (largest_bin_space / mean_item_space) as i64
}

/// Whether the parts come in more copies than a panel holds parts.
///
/// upstream: `mean_item_type_copies(instance) > many_item_type_copies_factor
/// * mean_number_of_items_in_bins`.
fn many_copies(problem: &Problem) -> bool {
    let mean_item_type_copies =
        f64::from(problem.number_of_items) / problem.item_types.len() as f64;
    mean_item_type_copies
        > (MANY_ITEM_TYPE_COPIES_FACTOR * mean_number_of_items_in_bins(problem)) as f64
}

/// The algorithms to run, in upstream's order.
///
/// upstream: optimize.cpp, `optimize`, "Automatic selection", minus the
/// algorithms that are not In: column generation (strips and general),
/// which upstream adds for knapsack and the open dimensions with 2 stages
/// or 3 homogenous ones on one panel, and for several panels. Decided for
/// Stage 5 (`docs/SCOPE.md`): with 2 or 3 stages, only the tree search
/// runs, where upstream would also run the sequential heuristics on
/// several panels.
pub(crate) fn select(problem: &Problem) -> Vec<Algorithm> {
    if !problem.unlimited_stages() {
        return vec![Algorithm::TreeSearch];
    }
    // upstream: the one-panel searches need each part type in its own
    // stack (`number_of_stacks() == number_of_item_types()`); otherwise,
    // with unlimited stages, the tree search runs.
    let one_stack_per_part_type = problem.number_of_stacks == problem.item_types.len();
    if problem.number_of_bins <= 1 {
        if !one_stack_per_part_type {
            return vec![Algorithm::TreeSearch];
        }
        let bin_area = if problem.bin_types.len() == 1 {
            problem.bin_types[0].size.area()
        } else {
            0
        };
        if problem.objective == Objective::Knapsack
            && problem.number_of_bins == 1
            && many_copies(problem)
            && bin_area <= HYPERGRAPH_MAXIMUM_BIN_AREA
        {
            return vec![Algorithm::Hypergraph];
        }
        return vec![Algorithm::MaximalSpaces];
    }
    let many_items_in_bins = mean_number_of_items_in_bins(problem) > MANY_ITEMS_IN_BINS_THRESHOLD;
    let sequential = if many_items_in_bins {
        Algorithm::SequentialSingleKnapsack
    } else {
        Algorithm::SequentialValueCorrection
    };
    match problem.objective {
        // The objectives with several panels.
        Objective::Knapsack | Objective::BinPacking if many_copies(problem) => vec![sequential],
        Objective::Knapsack | Objective::BinPacking => vec![Algorithm::TreeSearch, sequential],
        // Open dimensions have one panel; feasibility is internal.
        Objective::OpenDimensionX | Objective::OpenDimensionY | Objective::Feasibility => {
            vec![Algorithm::TreeSearch]
        }
    }
}

/// Why `algorithm` cannot run on `problem`, if it cannot.
pub(crate) fn not_applicable(problem: &Problem, algorithm: Algorithm) -> Option<&'static str> {
    let unlimited = problem.unlimited_stages();
    let one_panel = problem.number_of_bins <= 1;
    match algorithm {
        Algorithm::TreeSearch => None,
        Algorithm::MaximalSpaces | Algorithm::Hypergraph if !unlimited => {
            Some("it needs unlimited stages")
        }
        Algorithm::MaximalSpaces | Algorithm::Hypergraph if !one_panel => {
            Some("it cuts one panel only")
        }
        Algorithm::Hypergraph if problem.objective != Objective::Knapsack => {
            Some("it maximizes the value cut")
        }
        Algorithm::MaximalSpaces | Algorithm::Hypergraph => None,
        Algorithm::SequentialSingleKnapsack | Algorithm::SequentialValueCorrection => {
            if !unlimited {
                Some("it is only used with unlimited stages")
            } else if !matches!(
                problem.objective,
                Objective::BinPacking | Objective::Knapsack
            ) {
                Some("it needs several panels")
            } else {
                None
            }
        }
    }
}

/// Runs one algorithm. `on_plan` sees each improvement as the algorithm
/// finds it; the hypergraph search only returns its plan at the end, as
/// upstream's does.
pub(crate) fn run(
    problem: &Problem,
    algorithm: Algorithm,
    params: &SolveParams,
    nodes_expanded: &mut u64,
    on_plan: &mut dyn FnMut(&Plan),
) -> Plan {
    let stop = &params.stop;
    match algorithm {
        Algorithm::TreeSearch => {
            let (plan, nodes) = tree_search(problem, params.queue_size, stop, on_plan);
            *nodes_expanded += nodes;
            plan
        }
        Algorithm::MaximalSpaces => {
            tree_search_maximal_spaces(problem, params.maximal_spaces_queue_size, stop, on_plan)
        }
        Algorithm::Hypergraph => tree_search_hypergraph_infinite_copies(problem, stop),
        Algorithm::SequentialSingleKnapsack => {
            sequential_value_correction(problem, Variant::SingleKnapsack, stop, on_plan)
        }
        Algorithm::SequentialValueCorrection => {
            sequential_value_correction(problem, Variant::ValueCorrection, stop, on_plan)
        }
    }
}

/// Whether some part cannot fit any panel, which makes every objective but
/// knapsack infeasible.
///
/// upstream: optimize.cpp, `optimize_item_types_fit`.
fn some_part_never_fits(problem: &Problem) -> bool {
    problem.objective != Objective::Knapsack
        && problem.item_types.iter().any(|item| {
            item.copies > 0
                && !problem.bin_types.iter().any(|bin| {
                    let fits = |w: i64, h: i64| w <= bin.size.w && h <= bin.size.h;
                    fits(item.size.w, item.size.h)
                        || (!item.oriented && fits(item.size.h, item.size.w))
                })
        })
}

/// Runs the selected algorithms in order and keeps the first plan strictly
/// better than all before it. Returns it with the algorithms selected.
///
/// Once the best plan reaches the area bound, the running algorithm stops
/// and the later ones are skipped (upstream starts them, and they stop at
/// once); nothing strictly better exists, so the plan is the same.
///
/// upstream: optimize.cpp, `optimize`.
pub(crate) fn optimize(
    problem: &Problem,
    params: &SolveParams,
    nodes_expanded: &mut u64,
    mut on_plan: impl FnMut(&Plan),
) -> (Plan, Vec<Algorithm>) {
    let mut best = Plan::empty(problem);
    if some_part_never_fits(problem) {
        return (best, Vec::new());
    }
    // upstream: `optimize_trivial_bound`, before any search.
    let bound = Bound::of(problem);
    // upstream: the algorithm formatter's end flag, which every search
    // started from here also stops on.
    let end = Arc::new(AtomicBool::new(false));
    let params = SolveParams {
        stop: params.stop.with_flag(&end),
        ..params.clone()
    };
    let algorithms = select(problem);
    let mut consider = |plan: &Plan| {
        if plan.is_better_than(&best, problem.objective) {
            best = plan.clone();
            on_plan(&best);
            if bound.reached_by(&best, problem.objective) {
                end.store(true, Ordering::Relaxed);
            }
        }
    };
    for &algorithm in &algorithms {
        if params.stop.requested() {
            break;
        }
        let plan = run(problem, algorithm, &params, nodes_expanded, &mut consider);
        consider(&plan);
    }
    (best, algorithms)
}

/// Solves a one-panel subproblem as upstream's sequential heuristics do:
/// `optimize()` with the given queue sizes.
pub(crate) fn optimize_subproblem(
    problem: &Problem,
    queue_size: u64,
    maximal_spaces_queue_size: u64,
    stop: &Stop,
) -> Plan {
    let params = SolveParams {
        queue_size,
        maximal_spaces_queue_size,
        stop: stop.clone(),
    };
    optimize(problem, &params, &mut 0, |_| {}).0
}
