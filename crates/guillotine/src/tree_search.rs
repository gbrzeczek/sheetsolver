//! The tree search over the 3-level branching scheme.
//!
//! upstream: src/rectangleguillotine/tree_search.cpp, `tree_search`, in
//! not-anytime-sequential mode.

use treesearch::{Parameters, Stop, iterative_beam_search};

use crate::plan::{Plan, plan_of};
use crate::problem::{Objective, Orientation, Problem};
use crate::scheme::{GuideKind, Scheme};

/// Runs one beam search per guide and per first-cut direction (both when
/// first cuts may go either way), one after the other, each with queue
/// size 1 and then `queue_size`, and keeps the first plan that is strictly
/// better than every plan before it. `on_plan` is called with each such
/// plan. Returns the best plan and the nodes expanded.
///
/// The caller's bound stops it through `stop`, once `on_plan` has seen a
/// plan that reaches the bound (see `bounds.rs`).
pub(crate) fn tree_search(
    problem: &Problem,
    queue_size: u64,
    stop: &Stop,
    mut on_plan: impl FnMut(&Plan),
) -> (Plan, u64) {
    let guides = if problem.objective == Objective::Knapsack {
        [
            GuideKind::AreaPerProfit,
            GuideKind::AreaPerProfitPerAverageItemArea,
        ]
    } else {
        [
            GuideKind::AreaPerItemArea,
            GuideKind::WasteRatePerAverageItemArea,
        ]
    };
    // With one panel type, "either" runs one search per direction; with
    // several, each panel may pick its direction within one search.
    let orientations =
        if problem.first_stage_orientation == Orientation::Any && problem.bin_types.len() == 1 {
            vec![Orientation::Vertical, Orientation::Horizontal]
        } else {
            vec![problem.first_stage_orientation]
        };
    let search_parameters = Parameters {
        stop: stop.clone(),
        ..Parameters::fixed_queue_size(queue_size)
    };

    let mut best = Plan::empty(problem);
    let mut nodes_expanded = 0;
    'searches: for guide in guides {
        for &orientation in &orientations {
            let scheme = Scheme::new(problem, guide, orientation);
            let mut proven_optimal = false;
            let output = iterative_beam_search(&scheme, &search_parameters, |output| {
                let plan = plan_of(&scheme, problem, &output.best);
                let feasible = plan.feasible;
                if plan.is_better_than(&best, problem.objective) {
                    best = plan;
                    on_plan(&best);
                }
                // upstream: a search that pruned nothing is exhaustive when
                // the first-cut direction is fixed. Its result is then a
                // bound, and once the best plan meets it (an infeasibility
                // proof, or the panel count or value just found), every
                // later search is skipped. With open dimensions no bound is
                // recorded.
                if output.optimal
                    && problem.first_stage_orientation != Orientation::Any
                    && (!feasible
                        || matches!(
                            problem.objective,
                            Objective::BinPacking | Objective::Knapsack
                        ))
                {
                    proven_optimal = true;
                }
            });
            nodes_expanded += output.nodes_expanded;
            if proven_optimal {
                break 'searches;
            }
        }
    }
    (best, nodes_expanded)
}
