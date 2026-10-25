//! `solve` on a few representative instances, at the queue sizes of the
//! golden files, and each unlimited-stage algorithm alone on a few more.
//! Compare with upstream's times from `tools/bench_oracle.py`, which runs
//! the same instances (listed in `instances.txt` and
//! `instances_unlimited.txt`) through upstream's `tree_search()` and the
//! oracle's `--algorithm`.

// criterion's macros generate undocumented functions.
#![allow(missing_docs)]

use std::fs;
use std::path::Path;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use guillotine::{Algorithm, SolveParams, solve, solve_with_algorithm};
use model::Instance;

/// The non-comment lines of a list in `benches/`.
fn lines(list: &str) -> Vec<String> {
    let benches = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches");
    fs::read_to_string(benches.join(list))
        .unwrap()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect()
}

fn instance(stem: &str) -> Instance {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/instances");
    let text = fs::read_to_string(data.join(format!("{stem}.json"))).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn instances() -> Vec<(String, Instance)> {
    lines("instances.txt")
        .into_iter()
        .map(|stem| {
            let instance = instance(&stem);
            (stem, instance)
        })
        .collect()
}

fn bench_solve(c: &mut Criterion) {
    let instances = instances();
    for queue_size in [64, 2048] {
        let mut group = c.benchmark_group(format!("solve_q{queue_size}"));
        group.sample_size(10);
        for (name, instance) in &instances {
            let params = SolveParams {
                queue_size,
                ..SolveParams::default()
            };
            group.bench_with_input(
                BenchmarkId::from_parameter(name),
                instance,
                |b, instance| b.iter(|| solve(instance, &params, |_| {}).unwrap()),
            );
        }
        group.finish();
    }
}

/// Each line of `instances_unlimited.txt`: an algorithm (the oracle's
/// name) and an instance, run with upstream's default parameters.
fn bench_unlimited(c: &mut Criterion) {
    let mut group = c.benchmark_group("unlimited");
    group.sample_size(10);
    for line in lines("instances_unlimited.txt") {
        let (name, stem) = line.split_once(' ').unwrap();
        let algorithm = match name {
            "tree_search" => Algorithm::TreeSearch,
            "maximal_spaces" => Algorithm::MaximalSpaces,
            "hypergraph" => Algorithm::Hypergraph,
            "sequential_single_knapsack" => Algorithm::SequentialSingleKnapsack,
            "sequential_value_correction" => Algorithm::SequentialValueCorrection,
            other => panic!("unknown algorithm {other}"),
        };
        let instance = instance(stem);
        let params = SolveParams::default();
        group.bench_with_input(BenchmarkId::new(name, stem), &instance, |b, instance| {
            b.iter(|| solve_with_algorithm(instance, algorithm, &params).unwrap())
        });
    }
    group.finish();
}

criterion_group!(benches, bench_solve, bench_unlimited);
criterion_main!(benches);
