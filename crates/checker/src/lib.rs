//! Independent feasibility checker for cutting plans.
//!
//! Given an instance and a solution, reports every rule the solution
//! breaks and computes its metrics. It is written from the problem
//! definition, not from solver code, and must never depend on the
//! `treesearch` or `guillotine` crates (see `docs/TESTING.md`).
//!
//! ```text
//! let report = checker::check(&instance, &solution);
//! if !report.is_feasible() { for v in &report.violations { println!("{v}") } }
//! ```

mod stages;
mod tree;
mod violation;

use std::collections::HashMap;

use model::{Area, Instance, Length, Metrics, NodeKind, Solution};

pub use violation::{Problem, Violation};

/// The checker's verdict on a solution.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Figures describing the plan, computed whether or not it is feasible.
    pub metrics: Metrics,
    /// Every rule the plan breaks. Empty if the plan is feasible.
    pub violations: Vec<Violation>,
}

impl Report {
    /// Whether the plan breaks no rule.
    pub fn is_feasible(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Checks `solution` against `instance` and computes its metrics.
///
/// The instance is assumed to be valid (`Instance::validate`).
pub fn check(instance: &Instance, solution: &Solution) -> Report {
    let mut violations = Vec::new();
    let pattern_count = solution.patterns.len();

    for (index, pattern) in solution.patterns.iter().enumerate() {
        let name = format!("pattern {}", index + 1);
        let report = |problem| Violation {
            location: format!("{name}, panel"),
            problem,
        };
        if instance.panel(&pattern.panel_id).is_none() {
            violations.push(report(Problem::UnknownPanel(pattern.panel_id.clone())));
        }
        if pattern.repeat == 0 {
            violations.push(report(Problem::ZeroRepeat));
        }
        let has_parts = pattern
            .root
            .descendants()
            .iter()
            .any(|node| matches!(node.kind, NodeKind::Part { .. }));
        if !has_parts {
            violations.push(report(Problem::NoParts));
        }

        tree::check_pattern(
            instance,
            pattern,
            &name,
            index + 1 == pattern_count,
            &mut violations,
        );
        stages::check_pattern(instance, pattern, &name, &mut violations);
    }

    check_quantities(instance, solution, &mut violations);

    Report {
        metrics: metrics(instance, solution),
        violations,
    }
}

/// Panel and part quantities over the whole plan.
fn check_quantities(instance: &Instance, solution: &Solution, violations: &mut Vec<Violation>) {
    let mut report = |problem| {
        violations.push(Violation {
            location: "plan".to_string(),
            problem,
        })
    };

    let mut panels_used: HashMap<&str, u32> = HashMap::new();
    for pattern in &solution.patterns {
        *panels_used.entry(&pattern.panel_id).or_default() += pattern.repeat;
    }
    for panel in &instance.panels {
        let used = panels_used.get(panel.id.as_str()).copied().unwrap_or(0);
        if let Some(available) = panel.quantity {
            if used > available {
                report(Problem::TooManyPanels {
                    panel_id: panel.id.clone(),
                    used,
                    available,
                });
            }
        }
    }
    if instance.objective.is_open_dimension() {
        let total: u32 = panels_used.values().sum();
        if total > 1 {
            report(Problem::OpenDimensionUsesSeveralPanels(total));
        }
    }

    let parts_cut = parts_cut(solution);
    for part in &instance.parts {
        let cut = parts_cut.get(part.id.as_str()).copied().unwrap_or(0);
        if cut > part.quantity {
            report(Problem::TooManyParts {
                part_id: part.id.clone(),
                cut,
                wanted: part.quantity,
            });
        } else if cut < part.quantity && instance.objective.cuts_every_part() {
            report(Problem::MissingParts {
                part_id: part.id.clone(),
                cut,
                wanted: part.quantity,
            });
        }
    }
}

/// Copies cut of each part, counting repeats.
fn parts_cut(solution: &Solution) -> HashMap<&str, u32> {
    let mut cut = HashMap::new();
    for pattern in &solution.patterns {
        for node in pattern.root.descendants() {
            if let NodeKind::Part { part_id, .. } = &node.kind {
                *cut.entry(part_id.as_str()).or_default() += pattern.repeat;
            }
        }
    }
    cut
}

/// Computes the plan's metrics. Definitions match upstream's
/// (`Solution::append_bin`), so the two can be compared.
fn metrics(instance: &Instance, solution: &Solution) -> Metrics {
    let mut panels_used = 0;
    let mut parts_cut = 0;
    let mut panel_area = Area::ZERO;
    let mut part_area = Area::ZERO;
    let mut leftover_area = Area::ZERO;
    let mut total_value = 0.0;

    for pattern in &solution.patterns {
        let repeat = pattern.repeat;
        panels_used += repeat;
        panel_area = panel_area + pattern.root.area().times(repeat);
        for node in pattern.root.descendants() {
            match &node.kind {
                NodeKind::Part { part_id, .. } => {
                    parts_cut += repeat;
                    part_area = part_area + node.area().times(repeat);
                    if let Some(part) = instance.part(part_id) {
                        total_value += part.effective_value() * repeat as f64;
                    }
                }
                NodeKind::Leftover => leftover_area = leftover_area + node.area().times(repeat),
                NodeKind::Piece | NodeKind::Offcut => {}
            }
        }
    }

    // How far the cuts reach on the last panel, ignoring its leftover.
    let (used_length, used_width) = solution
        .patterns
        .last()
        .map(|pattern| {
            pattern
                .root
                .descendants()
                .into_iter()
                .filter(|node| node.stage > 0 && node.kind != NodeKind::Leftover)
                .fold((Length::ZERO, Length::ZERO), |(length, width), node| {
                    (
                        length.max(node.x + node.length),
                        width.max(node.y + node.width),
                    )
                })
        })
        .unwrap_or_default();

    let waste_area = panel_area - part_area - leftover_area;
    let usable_area = panel_area - leftover_area;
    Metrics {
        panels_used,
        parts_cut,
        panel_area,
        part_area,
        leftover_area,
        waste_area,
        waste_fraction: if usable_area == Area::ZERO {
            0.0
        } else {
            waste_area.hundredths_mm2() as f64 / usable_area.hundredths_mm2() as f64
        },
        total_value,
        used_length,
        used_width,
    }
}
