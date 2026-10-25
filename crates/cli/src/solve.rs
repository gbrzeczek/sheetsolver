//! `solve`: solve one instance and print its plan.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Context;
use guillotine::{SolveParams, solve};
use treesearch::Stop;

use crate::files::{describe, read_instance};

/// Solves `instance_path` and writes the plan as JSON to `output` or
/// stdout. With `progress`, prints each better plan found to stderr.
/// Returns whether the plan passed the checker.
pub fn run(
    instance_path: &Path,
    queue_size: u64,
    time_limit: Option<Duration>,
    output: Option<&Path>,
    progress: bool,
) -> anyhow::Result<bool> {
    let instance = read_instance(instance_path)?;
    let start = Instant::now();
    let params = SolveParams {
        queue_size,
        stop: Stop {
            deadline: time_limit.map(|limit| start + limit),
            ..Stop::default()
        },
    };
    let result = solve(&instance, &params, |solution| {
        if !progress {
            return;
        }
        let metrics = checker::check(&instance, solution).metrics;
        eprintln!(
            "{:8.3} s  better plan: {}",
            start.elapsed().as_secs_f64(),
            describe(&metrics, instance.objective)
        );
    })
    .with_context(|| format!("solving {}", instance_path.display()))?;
    let elapsed = start.elapsed();

    let json = serde_json::to_string_pretty(&result.solution)?;
    match output {
        Some(path) => {
            fs::write(path, json + "\n").with_context(|| format!("writing {}", path.display()))?
        }
        None => println!("{json}"),
    }

    eprintln!(
        "solved in {:.3} s, {} nodes expanded",
        elapsed.as_secs_f64(),
        result.nodes_expanded
    );
    if result.solution.patterns.is_empty() {
        eprintln!("no plan found that cuts every part");
        return Ok(true);
    }
    let report = checker::check(&instance, &result.solution);
    eprintln!("plan: {}", describe(&report.metrics, instance.objective));
    for violation in &report.violations {
        eprintln!("violation: {violation}");
    }
    Ok(report.is_feasible())
}
