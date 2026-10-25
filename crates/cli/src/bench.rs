//! `bench-set`: a quick timing of the solver on a set of instances.
//!
//! The criterion benchmarks (`cargo bench -p guillotine`) are the careful
//! measurement; this is for a fast look at any set of instances.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Context;
use guillotine::{SolveParams, solve};
use model::{Instance, Stages};

use crate::files::{read_instance, repository_dir};

/// The benchmark set shared with criterion and `tools/bench_oracle.py`.
const DEFAULT_SET: &str = "crates/guillotine/benches/instances.txt";

/// Times every instance in `sets` at every queue size and prints a table.
/// Returns whether every instance could be solved.
pub fn run(
    sets: &[PathBuf],
    instances_dir: &Path,
    queue_sizes: &[u64],
    repeat: u32,
) -> anyhow::Result<bool> {
    let default_set = [repository_dir().join(DEFAULT_SET)];
    let sets = if sets.is_empty() {
        &default_set[..]
    } else {
        sets
    };
    let queue_sizes = if queue_sizes.is_empty() {
        &[64, 2048][..]
    } else {
        queue_sizes
    };

    let mut instances = Vec::new();
    for set in sets {
        instances.extend(instances_in(set, instances_dir)?);
    }

    println!(
        "{:<56} {:>6} {:>10} {:>10} {:>12}",
        "instance", "queue", "fastest s", "median s", "nodes"
    );
    let mut success = true;
    for (name, instance) in &instances {
        if instance.saw.stages == Stages::Unlimited {
            println!("{name:<56} skipped: unlimited stages (Stage 5)");
            continue;
        }
        for &queue_size in queue_sizes {
            let params = SolveParams {
                queue_size,
                ..SolveParams::default()
            };
            let mut times = Vec::new();
            let mut nodes = 0;
            for _ in 0..repeat {
                let start = Instant::now();
                match solve(instance, &params, |_| {}) {
                    Ok(output) => nodes = output.nodes_expanded,
                    Err(error) => {
                        println!("{name:<56} {queue_size:>6} error: {error}");
                        success = false;
                        break;
                    }
                }
                times.push(start.elapsed());
            }
            if times.len() < repeat as usize {
                continue;
            }
            times.sort();
            println!(
                "{name:<56} {queue_size:>6} {:>10.4} {:>10.4} {nodes:>12}",
                seconds(times[0]),
                seconds(times[times.len() / 2])
            );
        }
    }
    Ok(success)
}

/// The instances named by `set`: an instance file itself, or a list of
/// instance names, one per line.
fn instances_in(set: &Path, instances_dir: &Path) -> anyhow::Result<Vec<(String, Instance)>> {
    let name_of = |path: &Path| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    if set.extension().is_some_and(|extension| extension == "json") {
        return Ok(vec![(name_of(set), read_instance(set)?)]);
    }
    let text = fs::read_to_string(set).with_context(|| format!("reading {}", set.display()))?;
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|name| {
            let path = instances_dir.join(format!("{name}.json"));
            Ok((name.to_string(), read_instance(&path)?))
        })
        .collect()
}

fn seconds(duration: Duration) -> f64 {
    duration.as_secs_f64()
}
