//! Guillotine cutting solver.
//!
//! A port of upstream `rectangleguillotine` for the features marked In in
//! `docs/SCOPE.md`. [`solve`] picks the algorithms upstream's `optimize()`
//! would pick (limited to the In ones) and runs them in upstream's
//! deterministic mode; [`solve_with_algorithm`] runs one of them, and gives
//! the same cutting plan as the oracle (`tools/oracle --algorithm`).
//!
//! - `problem`: the conversion from `model` types to upstream's terms.
//! - `plan`: cutting plans as upstream builds and compares them.
//! - `scheme`, `tree_search`: the 3-level tree search (2 and 3 stages).
//! - `maximal_spaces`: one panel, unlimited stages.
//! - `hypergraph`: one small panel, many copies, unlimited stages.
//! - `sequential`: several panels, filled one at a time.
//! - `selection`: which algorithms run.
//! - `open_dimension`: completing open-dimension plans (a deviation).
//! - `cpp_sort`: `std::sort` as upstream's standard library runs it.

mod cpp_sort;
mod hypergraph;
mod maximal_spaces;
mod open_dimension;
mod plan;
mod problem;
mod restage;
mod scheme;
mod selection;
mod sequential;
mod tree_search;

use model::{Instance, Solution, ValidationError};
use treesearch::Stop;

use crate::plan::Plan;
use crate::problem::Problem;

pub use crate::open_dimension::complete_open_dimension;
pub use crate::selection::Algorithm;

/// How to search.
#[derive(Debug, Clone)]
pub struct SolveParams {
    /// The beam width of the tree search (2 and 3 stages, and several
    /// panels with unlimited stages): larger finds better plans, slower.
    /// Each search runs once with 1 and once with this size, as upstream's
    /// deterministic mode does (`not_anytime_tree_search_queue_size`).
    pub queue_size: u64,
    /// The beam width of the maximal-spaces search (one panel, unlimited
    /// stages; upstream `not_anytime_tree_search_maximal_spaces_queue_size`).
    /// The sequential heuristics use upstream's own sizes for their
    /// subproblems.
    pub maximal_spaces_queue_size: u64,
    /// When to give up early: a deadline or a cancellation flag. The best
    /// plan found so far is returned.
    pub stop: Stop,
}

impl Default for SolveParams {
    /// Upstream's default queue sizes, 2048 and 16, and no time limit.
    fn default() -> Self {
        SolveParams {
            queue_size: 2048,
            maximal_spaces_queue_size: 16,
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
    /// The algorithms that ran, in order.
    pub algorithms: Vec<Algorithm>,
    /// Nodes expanded by the tree search, over all its searches.
    pub nodes_expanded: u64,
}

/// Why an instance cannot be solved.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SolveError {
    /// The instance is not valid.
    #[error("invalid instance: {0:?}")]
    Invalid(Vec<ValidationError>),
    /// The algorithm cannot solve this instance (e.g. a one-panel search
    /// asked to cut several panels).
    #[error("{algorithm:?} does not apply: {reason}")]
    NotApplicable {
        /// The algorithm asked for.
        algorithm: Algorithm,
        /// What it needs that the instance lacks.
        reason: &'static str,
    },
}

/// Finds a cutting plan for `instance`.
///
/// Runs the algorithms [`select_algorithms`] picks, one after the other,
/// and keeps the first plan that is strictly better than every plan
/// before it. `on_solution` is called with each such plan: every
/// improvement of the tree search, and the result of each other
/// algorithm if it improves.
///
/// upstream: src/rectangleguillotine/optimize.cpp, `optimize`, with
/// `OptimizationMode::NotAnytimeSequential` and no reduction. Upstream
/// also computes bounds first and stops once a plan reaches one; that can
/// only skip work that would find nothing strictly better, so it is left
/// out (see `docs/TESTING.md`).
pub fn solve(
    instance: &Instance,
    params: &SolveParams,
    mut on_solution: impl FnMut(&Solution),
) -> Result<SolveOutput, SolveError> {
    instance.validate().map_err(SolveError::Invalid)?;
    let problem = Problem::new(instance);
    let mut nodes_expanded = 0;
    let (best, algorithms) = selection::optimize(&problem, params, &mut nodes_expanded, |plan| {
        on_solution(&plan.to_solution(&problem, instance))
    });
    Ok(SolveOutput {
        solution: best.to_solution(&problem, instance),
        algorithms,
        nodes_expanded,
    })
}

/// The algorithms [`solve`] runs on `instance`, in order: upstream's
/// automatic selection, limited to the algorithms that are In (see
/// `docs/SCOPE.md` and [`Algorithm`]).
pub fn select_algorithms(instance: &Instance) -> Result<Vec<Algorithm>, SolveError> {
    instance.validate().map_err(SolveError::Invalid)?;
    Ok(selection::select(&Problem::new(instance)))
}

/// Runs one algorithm alone, as the oracle's `--algorithm` does.
pub fn solve_with_algorithm(
    instance: &Instance,
    algorithm: Algorithm,
    params: &SolveParams,
) -> Result<SolveOutput, SolveError> {
    instance.validate().map_err(SolveError::Invalid)?;
    let problem = Problem::new(instance);
    if let Some(reason) = selection::not_applicable(&problem, algorithm) {
        return Err(SolveError::NotApplicable { algorithm, reason });
    }
    let mut nodes_expanded = 0;
    let plan: Plan = selection::run(&problem, algorithm, params, &mut nodes_expanded, |_| {});
    Ok(SolveOutput {
        solution: plan.to_solution(&problem, instance),
        algorithms: vec![algorithm],
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
    fn rejects_invalid_instances() {
        let mut invalid = instance("3", "bin_packing");
        invalid.parts.clear();
        assert_eq!(
            solve(&invalid, &SolveParams::default(), |_| {}).unwrap_err(),
            SolveError::Invalid(vec![ValidationError::NoParts])
        );
    }

    #[test]
    fn solves_unlimited_stages_with_upstreams_algorithms() {
        // Two panels: the tree search and the sequential heuristic.
        let unlimited = instance(r#""unlimited""#, "bin_packing");
        let output = solve(&unlimited, &SolveParams::default(), |_| {}).unwrap();
        assert_eq!(
            output.algorithms,
            vec![Algorithm::TreeSearch, Algorithm::SequentialValueCorrection]
        );
        assert!(checker::check(&unlimited, &output.solution).is_feasible());

        // One panel: the maximal-spaces search.
        let mut one_panel = instance(r#""unlimited""#, "knapsack");
        one_panel.panels[0].quantity = Some(1);
        let output = solve(&one_panel, &SolveParams::default(), |_| {}).unwrap();
        assert_eq!(output.algorithms, vec![Algorithm::MaximalSpaces]);
        assert!(checker::check(&one_panel, &output.solution).is_feasible());
        assert_eq!(
            solve_with_algorithm(
                &one_panel,
                Algorithm::SequentialValueCorrection,
                &SolveParams::default()
            )
            .map(|output| output.algorithms),
            Ok(vec![Algorithm::SequentialValueCorrection])
        );

        // The one-panel searches refuse several panels.
        assert!(matches!(
            solve_with_algorithm(
                &unlimited,
                Algorithm::MaximalSpaces,
                &SolveParams::default()
            ),
            Err(SolveError::NotApplicable { .. })
        ));
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
                ..Stop::default()
            },
            ..SolveParams::default()
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
