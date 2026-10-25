//! Guillotine cutting solver.
//!
//! A port of upstream `rectangleguillotine`'s tree search for the features
//! marked In in `docs/SCOPE.md`: [`solve`] runs upstream's `tree_search()`
//! in its deterministic mode, and gives the same cutting plan as the
//! oracle (`tools/oracle`) for the same queue size.
//!
//! - `problem`: the conversion from `model` types to upstream's terms.
//! - `scheme`: the branching scheme (`BranchingScheme`).
//! - `plan`: turning a search node into a cutting plan.

mod plan;
mod problem;
mod scheme;

use model::{Instance, Objective, Solution, Stages, ValidationError};
use treesearch::{Parameters, Stop, iterative_beam_search};

use crate::plan::{Plan, plan_of};
use crate::problem::{Orientation, Problem};
use crate::scheme::{GuideKind, Scheme};

/// How to search.
#[derive(Debug, Clone)]
pub struct SolveParams {
    /// The beam width of each search: larger finds better plans, slower.
    /// Each search runs once with 1 and once with this size, as upstream's
    /// deterministic mode does (`not_anytime_tree_search_queue_size`).
    pub queue_size: u64,
    /// When to give up early: a deadline or a cancellation flag. The best
    /// plan found so far is returned.
    pub stop: Stop,
}

impl Default for SolveParams {
    /// Upstream's default queue size, 2048, and no time limit.
    fn default() -> Self {
        SolveParams {
            queue_size: 2048,
            stop: Stop::default(),
        }
    }
}

/// What [`solve`] found.
#[derive(Debug, Clone)]
pub struct SolveOutput {
    /// The best plan found. Empty if none was found that cuts every part
    /// (the knapsack objective accepts any plan).
    pub solution: Solution,
    /// Nodes expanded, over all searches.
    pub nodes_expanded: u64,
}

/// Why an instance cannot be solved.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SolveError {
    /// The instance is not valid.
    #[error("invalid instance: {0:?}")]
    Invalid(Vec<ValidationError>),
    /// Unlimited stages are not supported yet (Stage 5 in `docs/STAGES.md`).
    #[error("unlimited stages are not supported yet")]
    UnlimitedStages,
}

/// Finds a cutting plan for `instance`.
///
/// Runs one beam search per guide and per first-cut direction (both when
/// first cuts may go either way), one after the other, and keeps the first
/// plan that is strictly better than every plan before it. `on_solution`
/// is called with each such plan.
///
/// upstream: src/rectangleguillotine/tree_search.cpp, `tree_search`, as
/// `optimize()` calls it with `OptimizationMode::NotAnytimeSequential`.
/// Upstream also computes bounds first and stops once a plan reaches one;
/// that can only skip work that would find nothing strictly better, so it
/// is left out (see `docs/TESTING.md`).
pub fn solve(
    instance: &Instance,
    params: &SolveParams,
    mut on_solution: impl FnMut(&Solution),
) -> Result<SolveOutput, SolveError> {
    instance.validate().map_err(SolveError::Invalid)?;
    if instance.saw.stages == Stages::Unlimited {
        return Err(SolveError::UnlimitedStages);
    }
    let problem = Problem::new(instance);

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
        stop: params.stop.clone(),
        ..Parameters::fixed_queue_size(params.queue_size)
    };

    let mut best = Plan::empty(&problem);
    let mut nodes_expanded = 0;
    'searches: for guide in guides {
        for &orientation in &orientations {
            let scheme = Scheme::new(&problem, guide, orientation);
            let mut proven_optimal = false;
            let output = iterative_beam_search(&scheme, &search_parameters, |output| {
                let plan = plan_of(&scheme, &problem, &output.best);
                let feasible = plan.feasible;
                if plan.is_better_than(&best, problem.objective) {
                    best = plan;
                    on_solution(&best.to_solution(&problem, instance));
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

    Ok(SolveOutput {
        solution: best.to_solution(&problem, instance),
        nodes_expanded,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use super::*;

    fn instance(stages: &str, objective: &str) -> Instance {
        serde_json::from_str(&format!(
            r#"{{
                "objective": "{objective}",
                "saw": {{ "kerf": 3.2, "stages": {stages}, "cut_type": "non_exact" }},
                "panels": [{{ "id": "Ply", "length": 2500, "width": 1250, "quantity": 2 }}],
                "parts": [
                    {{ "id": "Side", "length": 720, "width": 560, "quantity": 4, "grain": true }},
                    {{ "id": "Shelf", "length": 800, "width": 300, "quantity": 6 }}
                ]
            }}"#
        ))
        .unwrap()
    }

    #[test]
    fn rejects_invalid_instances_and_unlimited_stages() {
        let mut invalid = instance("3", "bin_packing");
        invalid.parts.clear();
        assert_eq!(
            solve(&invalid, &SolveParams::default(), |_| {}).unwrap_err(),
            SolveError::Invalid(vec![ValidationError::NoParts])
        );
        let unlimited = instance(r#""unlimited""#, "bin_packing");
        assert_eq!(
            solve(&unlimited, &SolveParams::default(), |_| {}).unwrap_err(),
            SolveError::UnlimitedStages
        );
    }

    #[test]
    fn reports_each_improvement_and_returns_the_last() {
        let instance = instance("3", "knapsack");
        let params = SolveParams {
            queue_size: 16,
            ..SolveParams::default()
        };
        let mut reported = Vec::new();
        let output = solve(&instance, &params, |solution| {
            reported.push(solution.clone())
        })
        .unwrap();
        assert!(!reported.is_empty());
        assert_eq!(reported.last(), Some(&output.solution));
        assert!(checker::check(&instance, &output.solution).is_feasible());
    }

    #[test]
    fn cancelled_before_starting_finds_nothing() {
        let instance = instance("2", "bin_packing");
        let cancel = Arc::new(AtomicBool::new(true));
        let params = SolveParams {
            queue_size: 16,
            stop: Stop {
                deadline: None,
                cancel: Some(cancel),
            },
        };
        let output = solve(&instance, &params, |_| panic!("no plan expected")).unwrap();
        assert!(output.solution.patterns.is_empty());
        assert_eq!(output.nodes_expanded, 0);
    }

    #[test]
    fn profits_must_differ_by_more_than_a_millionth() {
        assert!(!plan::strictly_greater_profit(1.0 + 1e-7, 1.0));
        assert!(plan::strictly_greater_profit(1.0 + 1e-5, 1.0));
        assert!(!plan::strictly_greater_profit(1.0, 1.0));
        assert!(plan::strictly_greater_profit(1e-5, 0.0));
    }
}
