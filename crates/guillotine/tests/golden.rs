//! Differential test: `guillotine::solve` must give exactly the oracle's
//! plan (`data/golden/`) for every instance with 2 or 3 stages, and every
//! plan must pass the checker (see `docs/TESTING.md`).
//!
//! Queue size 2048 takes a while in debug builds; run with `--release` for
//! speed. Set `GOLDEN_FILTER` to a substring of the file names to run only
//! some of them.

use std::fs;
use std::path::{Path, PathBuf};

use guillotine::{SolveParams, solve};
use model::{Instance, Solution, Stages};
use serde_json::Value;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Runs every golden file at `queue_size` and reports all mismatches.
fn check_queue_size(queue_size: u64) {
    let filter = std::env::var("GOLDEN_FILTER").unwrap_or_default();
    let mut golden_paths: Vec<PathBuf> = fs::read_dir(data_dir().join("golden/9b656a0d"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_str().unwrap();
            name.ends_with(&format!(".q{queue_size}.json")) && name.contains(&filter)
        })
        .collect();
    golden_paths.sort();
    assert!(!golden_paths.is_empty(), "no golden files found");

    let mut compared = 0;
    let mut failures = Vec::new();
    for path in golden_paths {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let stem = name.rsplit_once(".q").unwrap().0;
        let instance: Instance =
            read_json(&data_dir().join("instances").join(format!("{stem}.json")));
        if instance.saw.stages == Stages::Unlimited {
            continue;
        }
        let golden: Value = read_json(&path);
        assert_eq!(golden["oracle"]["queue_size"], queue_size, "{name}");
        let expected: Solution = serde_json::from_value(golden).unwrap();

        let params = SolveParams {
            queue_size,
            ..SolveParams::default()
        };
        let output = solve(&instance, &params, |_| {}).unwrap();
        compared += 1;

        if output.solution != expected {
            failures.push(format!("{name}: differs from the oracle"));
            continue;
        }
        if !output.solution.patterns.is_empty() {
            let report = checker::check(&instance, &output.solution);
            if !report.is_feasible() {
                failures.push(format!("{name}: {:?}", report.violations));
            }
        }
    }
    eprintln!("compared {compared} instances at queue size {queue_size}");
    assert!(compared > 0 || !filter.is_empty(), "nothing compared");
    assert!(
        failures.is_empty(),
        "{} of {compared} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn same_plans_as_the_oracle_at_queue_size_64() {
    check_queue_size(64);
}

#[test]
fn same_plans_as_the_oracle_at_queue_size_2048() {
    check_queue_size(2048);
}
