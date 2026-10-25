//! `check`: judge a plan with the feasibility checker.

use std::path::Path;

use crate::files::{describe, read_instance, read_solution};

/// Checks the plan in `solution_path` against `instance_path`, prints its
/// metrics and violations, and returns whether it is feasible.
pub fn run(instance_path: &Path, solution_path: &Path) -> anyhow::Result<bool> {
    let instance = read_instance(instance_path)?;
    let solution = read_solution(solution_path)?;
    let report = checker::check(&instance, &solution);

    let metrics = &report.metrics;
    println!("{}", describe(metrics, instance.objective));
    println!("  panel area    {:.2} mm²", metrics.panel_area.mm2());
    println!("  part area     {:.2} mm²", metrics.part_area.mm2());
    println!("  leftover area {:.2} mm²", metrics.leftover_area.mm2());
    println!("  waste area    {:.2} mm²", metrics.waste_area.mm2());
    println!("  total value   {}", metrics.total_value);
    println!("  used length   {}", metrics.used_length);
    println!("  used width    {}", metrics.used_width);
    if report.is_feasible() {
        println!("feasible");
    } else {
        println!("{} violations:", report.violations.len());
        for violation in &report.violations {
            println!("  {violation}");
        }
    }
    Ok(report.is_feasible())
}
