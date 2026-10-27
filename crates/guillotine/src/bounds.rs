//! Cheap bounds, and when a plan reaches one.
//!
//! upstream: src/rectangleguillotine/optimize.cpp, `optimize_trivial_bound`,
//! and optimize.hpp, `Output::is_proven_optimal`.
//!
//! Upstream computes bounds before it searches and ends every running
//! search once the best plan reaches one. That never changes the plan: a
//! later plan only replaces the best when it is strictly better, and none
//! can be better than a bound. It saves time, most of all in the
//! sequential heuristics' one-panel subproblems, where every remaining part
//! often fits and the first plan that cuts them all ends the subproblem.
//!
//! Only upstream's area bounds are ported. Its other bounds come from its
//! one-dimensional solver and, for bin packing, from dual feasible
//! functions; leaving them out only means searching on where upstream
//! would have stopped, with the same plan (see `docs/TESTING.md`).

use crate::cpp_sort;
use crate::plan::Plan;
use crate::problem::{Objective, Problem};

/// The best a plan could possibly do for the problem's objective.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Bound {
    /// No bound: the feasibility objective, which ends on the first plan
    /// that cuts every part.
    None,
    /// Knapsack: the largest value a plan could cut.
    Value(f64),
    /// Bin packing: the fewest panels a plan could use.
    Panels(u32),
    /// Open dimensions: the shortest length a plan could use along the
    /// open side.
    Length(i64),
}

impl Bound {
    /// The area bound for `problem`'s objective.
    ///
    /// upstream: optimize.cpp, `optimize_trivial_bound`.
    pub(crate) fn of(problem: &Problem) -> Bound {
        match problem.objective {
            Objective::Knapsack => Bound::Value(value_bound(problem)),
            Objective::BinPacking => Bound::Panels(panel_bound(problem)),
            Objective::OpenDimensionX | Objective::OpenDimensionY => {
                Bound::Length(length_bound(problem))
            }
            Objective::Feasibility => Bound::None,
        }
    }

    /// Whether `plan` reaches the bound, so that nothing strictly better
    /// exists and every search can stop.
    ///
    /// upstream: optimize.hpp, `Output::is_proven_optimal` (the knapsack
    /// goal is left out: it is never set, so it is infinite).
    pub(crate) fn reached_by(self, plan: &Plan, objective: Objective) -> bool {
        if !plan.feasible {
            return false;
        }
        match (objective, self) {
            (Objective::Knapsack, Bound::Value(value)) => equal_profit(value, plan.profit),
            (Objective::BinPacking, Bound::Panels(panels)) => plan.number_of_bins == panels,
            (Objective::OpenDimensionX, Bound::Length(length)) => plan.width == length,
            (Objective::OpenDimensionY, Bound::Length(length)) => plan.height == length,
            (Objective::Feasibility, _) => true,
            _ => false,
        }
    }
}

/// The value of the parts with the best value per area that fill the
/// panels' total area, the last one in part (Dantzig's bound).
fn value_bound(problem: &Problem) -> f64 {
    let total_capacity: i64 = problem
        .bin_types
        .iter()
        .map(|bin| bin.area() * i64::from(bin.copies))
        .sum();
    let items = &problem.item_types;
    let mut order: Vec<usize> = (0..items.len()).collect();
    cpp_sort::sort(&mut order, |&first, &second| {
        items[first].profit * items[second].size.area() as f64
            > items[second].profit * items[first].size.area() as f64
    });
    let mut bound = 0.0;
    let mut remaining_capacity = total_capacity;
    for item in order.iter().map(|&index| &items[index]) {
        if remaining_capacity <= 0 {
            break;
        }
        let area = item.size.area();
        if area <= 0 {
            continue;
        }
        let total_area = area * i64::from(item.copies);
        if total_area <= remaining_capacity {
            bound += item.profit * f64::from(item.copies);
            remaining_capacity -= total_area;
        } else {
            bound += item.profit * (remaining_capacity as f64 / area as f64);
            remaining_capacity = 0;
        }
    }
    bound
}

/// The panels needed to hold the parts' area, using the panel types in
/// order. It counts only the panels there are, so unlike upstream's other
/// bounds it never proves an instance infeasible.
fn panel_bound(problem: &Problem) -> u32 {
    let mut remaining_item_area = problem.item_area;
    let mut bound = 0;
    for bin in &problem.bin_types {
        if remaining_item_area <= 0 {
            break;
        }
        let area = bin.area();
        if area <= 0 {
            continue;
        }
        let needed = (remaining_item_area + area - 1) / area;
        let used = needed.min(i64::from(bin.copies));
        bound += used as u32;
        remaining_item_area -= used * area;
    }
    bound
}

/// The length along the open side that the parts' area needs, and at least
/// the length of the part that needs the most of it.
fn length_bound(problem: &Problem) -> i64 {
    let open_x = problem.objective == Objective::OpenDimensionX;
    let bin = problem.bin_types[0].size;
    let fixed = if open_x { bin.h } else { bin.w };
    let mut bound = if fixed > 0 {
        (problem.item_area + fixed - 1) / fixed
    } else {
        0
    };
    for item in &problem.item_types {
        let (along, across) = if open_x {
            (item.size.w, item.size.h)
        } else {
            (item.size.h, item.size.w)
        };
        let extent = if item.oriented {
            along
        } else {
            along.min(across)
        };
        bound = bound.max(extent);
    }
    bound
}

/// Whether two values are equal up to a relative 1e-6.
///
/// upstream: algorithms/common.hpp, `equal_profit`.
fn equal_profit(first: f64, second: f64) -> bool {
    !strictly_greater_profit(first, second) && !strictly_greater_profit(second, first)
}

/// upstream: algorithms/common.hpp, `strictly_greater_profit`.
fn strictly_greater_profit(first: f64, second: f64) -> bool {
    if first == second {
        return false;
    }
    let denominator = first.max(second);
    if denominator != 0.0 && denominator.is_finite() {
        (first - second) / denominator > 1e-6
    } else {
        first - second > 1e-6
    }
}

#[cfg(test)]
mod tests {
    use model::Instance;

    use super::*;

    /// Parts of 1000 × 500 mm, `parts` of them, on `panels` panels of
    /// 2000 × 1000 mm: four parts fill a panel exactly.
    fn problem(objective: &str, parts: u32, panels: u32) -> Problem {
        let instance: Instance = serde_json::from_str(&format!(
            r#"{{
                "objective": "{objective}",
                "saw": {{ "kerf": 0, "stages": "unlimited", "cut_type": "non_exact" }},
                "panels": [{{ "id": "Ply", "length": 2000, "width": 1000, "quantity": {panels} }}],
                "parts": [{{ "id": "Door", "length": 1000, "width": 500, "quantity": {parts} }}]
            }}"#
        ))
        .unwrap();
        Problem::new(&instance)
    }

    #[test]
    fn panel_bound_is_the_area_rounded_up() {
        assert_eq!(Bound::of(&problem("bin_packing", 4, 3)), Bound::Panels(1));
        assert_eq!(Bound::of(&problem("bin_packing", 5, 3)), Bound::Panels(2));
    }

    #[test]
    fn value_bound_takes_the_last_part_in_part() {
        // A part's value defaults to its area in mm²: 500 000. Four parts
        // fill the panel, so nine are worth four parts at most; three fit.
        assert_eq!(
            Bound::of(&problem("knapsack", 9, 1)),
            Bound::Value(2_000_000.0)
        );
        assert_eq!(
            Bound::of(&problem("knapsack", 3, 1)),
            Bound::Value(1_500_000.0)
        );
        // Better value per area first, then the rest of the area in part:
        // two Fronts (worth twice their area) fill half the panel, and the
        // other half holds one and two thirds of a Back's area.
        let instance: Instance = serde_json::from_str(
            r#"{
                "objective": "knapsack",
                "saw": { "kerf": 0, "stages": "unlimited", "cut_type": "non_exact" },
                "panels": [{ "id": "Ply", "length": 2000, "width": 1000, "quantity": 1 }],
                "parts": [
                    { "id": "Back", "length": 1000, "width": 600, "quantity": 5 },
                    { "id": "Front", "length": 1000, "width": 500, "quantity": 2, "value": 1000000 }
                ]
            }"#,
        )
        .unwrap();
        let Bound::Value(value) = Bound::of(&Problem::new(&instance)) else {
            panic!("a knapsack bound is a value");
        };
        assert!(equal_profit(value, 2.0 * 1_000_000.0 + 1_000_000.0));
    }

    #[test]
    fn values_equal_up_to_a_millionth() {
        assert!(equal_profit(1_000_000.0, 1_000_000.5));
        assert!(!equal_profit(1_000_000.0, 1_000_002.0));
        assert!(equal_profit(0.0, 0.0));
        assert!(!equal_profit(0.0, 1.0));
    }
}
