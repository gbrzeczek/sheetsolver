//! Several panels, filled one at a time.
//!
//! upstream: src/algorithms/sequential_value_correction.hpp,
//! `sequential_value_correction`, as optimize.cpp runs it
//! (`optimize_sequential_value_correction`, and with one pass
//! `optimize_sequential_single_knapsack`), for the bin packing and knapsack
//! objectives. Variable-sized bin packing, leftovers and parts fixed in
//! panels are not In and left out.
//!
//! Each panel in turn gets the most valuable plan of the parts still to
//! cut (a one-panel subproblem, solved as `optimize()` would), repeated on
//! as many identical panels as the remaining quantities allow. Value
//! correction then starts again with each part's value raised by the waste
//! of the panels it was cut from, so that hard parts are cut earlier.

use treesearch::Stop;

use crate::plan::Plan;
use crate::problem::{BinType, ItemType, Objective, Problem};
use crate::selection::optimize_subproblem;

/// Which of upstream's two uses of the algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Variant {
    /// One pass, values as given (upstream
    /// `optimize_sequential_single_knapsack`).
    SingleKnapsack,
    /// Up to 32 passes (upstream `optimize_sequential_value_correction`).
    ValueCorrection,
}

/// upstream's parameters for each variant (optimize.cpp, optimize.hpp).
struct Settings {
    maximum_number_of_iterations: u32,
    initial_profit_exponent: f64,
    subproblem_queue_size: u64,
    subproblem_maximal_spaces_queue_size: u64,
}

impl Variant {
    fn settings(self) -> Settings {
        match self {
            Variant::SingleKnapsack => Settings {
                maximum_number_of_iterations: 1,
                initial_profit_exponent: 1.0,
                subproblem_queue_size: 2048,
                subproblem_maximal_spaces_queue_size: 16,
            },
            Variant::ValueCorrection => Settings {
                maximum_number_of_iterations: 32,
                initial_profit_exponent: 1.1,
                subproblem_queue_size: 128,
                subproblem_maximal_spaces_queue_size: 16,
            },
        }
    }
}

/// upstream: `SequentialValueCorrectionParameters::bin_packing_goal`.
const BIN_PACKING_GOAL: u32 = 2;

/// Runs the algorithm and returns its best plan. `on_plan` sees each
/// better plan at the end of an iteration, so that the caller can stop
/// the next iterations once one reaches a bound.
pub(crate) fn sequential_value_correction(
    problem: &Problem,
    variant: Variant,
    stop: &Stop,
    on_plan: &mut dyn FnMut(&Plan),
) -> Plan {
    let settings = variant.settings();
    let mut best = Plan::empty(problem);
    if problem.item_types.is_empty() {
        return best;
    }
    let knapsack = problem.objective == Objective::Knapsack;

    // The values the subproblems maximize: the parts' own for knapsack,
    // otherwise their area (to a power that breaks ties).
    let mut profits: Vec<f64> = problem
        .item_types
        .iter()
        .map(|item| {
            if knapsack {
                item.profit
            } else {
                (item.size.area() as f64).powf(settings.initial_profit_exponent)
            }
        })
        .collect();
    let largest_bin_space = problem
        .bin_types
        .iter()
        .map(|bin| bin.size.area() as f64)
        .fold(0.0, f64::max);

    // The panel type of each panel position: smallest first for knapsack,
    // otherwise in the order listed.
    let mut bin_type_order: Vec<usize> = (0..problem.bin_types.len()).collect();
    if knapsack {
        bin_type_order.sort_by_key(|&bin_type| problem.bin_types[bin_type].size.area());
    }
    let bin_type_by_position: Vec<usize> = bin_type_order
        .iter()
        .flat_map(|&bin_type| {
            std::iter::repeat_n(bin_type, problem.bin_types[bin_type].copies as usize)
        })
        .collect();

    for _iteration in 0..settings.maximum_number_of_iterations {
        let mut solution = Plan::empty(problem);
        let mut adjusted_space = vec![0.0f64; problem.item_types.len()];

        loop {
            if stop.requested() {
                return best;
            }
            if solution.number_of_items == problem.number_of_items
                || solution.number_of_bins == problem.number_of_bins
            {
                break;
            }
            let bin_type = bin_type_by_position[solution.number_of_bins as usize];
            let bin = &problem.bin_types[bin_type];

            // The most valuable plan for this panel of the parts left.
            let mut item_types = Vec::new();
            let mut kp_to_original = Vec::new();
            for (item_type, item) in problem.item_types.iter().enumerate() {
                let copies = item.copies as i64 - solution.item_copies[item_type] as i64;
                if copies <= 0 {
                    continue;
                }
                item_types.push(ItemType {
                    copies: copies as u32,
                    profit: profits[item_type],
                    ..item.clone()
                });
                kp_to_original.push(item_type);
            }
            let mut kp = Problem::from_types(
                Objective::Knapsack,
                problem.settings(),
                item_types,
                vec![BinType {
                    size: bin.size,
                    copies: 1,
                    cost: bin.cost,
                }],
            );
            // upstream: the part types keep their stack numbers.
            kp.number_of_stacks = kp_to_original.last().map_or(0, |&last| last + 1);
            let kp_plan = optimize_subproblem(
                &kp,
                settings.subproblem_queue_size,
                settings.subproblem_maximal_spaces_queue_size,
                stop,
            );
            if stop.requested() {
                return best;
            }

            // upstream keeps the plan of each candidate panel type and
            // picks the best value per cost; there is one candidate here.
            let mut pattern = None;
            if !kp_plan.bins().is_empty() {
                let mut original = Plan::empty(problem);
                original.append_bin(
                    problem,
                    &kp_plan,
                    0,
                    1,
                    Some(&[bin_type]),
                    Some(&kp_to_original),
                );
                if 0.0 < kp_plan.profit / bin.cost {
                    pattern = Some(original);
                }
            }
            let Some(pattern) = pattern else {
                // A useless panel stays empty; a later one may do better.
                solution.append_empty_bin(problem, bin_type, 1);
                continue;
            };

            // As many identical panels as the remaining quantities allow.
            let mut copies = bin.copies - solution.bin_copies(bin_type);
            for (item_type, item) in problem.item_types.iter().enumerate() {
                let remaining = item.copies - solution.item_copies[item_type];
                let packed = pattern.item_copies[item_type];
                if let Some(fit) = remaining.checked_div(packed) {
                    copies = copies.min(fit);
                }
            }
            assert!(copies >= 1, "a panel's plan cuts more parts than are left");

            // The panel's waste, shared among its parts by area.
            let mut item_space = 0.0;
            for (item_type, item) in problem.item_types.iter().enumerate() {
                item_space += (i64::from(pattern.item_copies[item_type]) * item.size.area()) as f64;
            }
            // upstream: stored as an integer area, so truncated.
            let waste = (bin.size.area() as f64 - item_space) as i64;
            for (item_type, item) in problem.item_types.iter().enumerate() {
                let packed = f64::from(pattern.item_copies[item_type]);
                let space = item.size.area() as f64;
                let ratio = packed * space / item_space;
                adjusted_space[item_type] +=
                    f64::from(copies) * (packed * space + ratio * waste as f64);
            }
            solution.append_bin(problem, &pattern, 0, copies, None, None);
        }

        if solution.is_better_than(&best, problem.objective) {
            best = solution.clone();
            on_plan(&best);
        }
        let all_cut = best.number_of_items == problem.number_of_items;
        if !knapsack && all_cut && best.number_of_bins <= BIN_PACKING_GOAL.max(2) {
            return best;
        }
        if knapsack && all_cut {
            return best;
        }

        // Correct the values by the waste each part type caused.
        for (item_type, item) in problem.item_types.iter().enumerate() {
            let cut = solution.item_copies[item_type];
            let new_profit = if knapsack {
                if cut == 0 {
                    continue;
                }
                item.profit / item.size.area() as f64 * adjusted_space[item_type] / f64::from(cut)
            } else {
                adjusted_space[item_type] +=
                    100.0 * largest_bin_space * f64::from(item.copies as i32 - cut as i32);
                adjusted_space[item_type] / f64::from(item.copies)
            };
            profits[item_type] = 0.5 * profits[item_type] + 0.5 * new_profit;
        }
    }
    best
}
