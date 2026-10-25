//! `solve` on a few representative instances, at the queue sizes of the
//! golden files. Compare with upstream's times from
//! `tools/bench_oracle.py`, which runs the same instances (listed in
//! `instances.txt`) through upstream's `tree_search()`.

// criterion's macros generate undocumented functions.
#![allow(missing_docs)]

use std::fs;
use std::path::Path;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use guillotine::{SolveParams, solve};
use model::Instance;

fn instances() -> Vec<(String, Instance)> {
    let benches = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches");
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/instances");
    fs::read_to_string(benches.join("instances.txt"))
        .unwrap()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|stem| {
            let text = fs::read_to_string(data.join(format!("{stem}.json"))).unwrap();
            (stem.to_string(), serde_json::from_str(&text).unwrap())
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

criterion_group!(benches, bench_solve);
criterion_main!(benches);
