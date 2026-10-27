//! Differential test: each algorithm must give exactly the oracle's plan
//! (`data/golden/`) for the same algorithm and parameters, `solve` must
//! give upstream's own selection's plan (`*.auto.json`), and every plan
//! must pass the checker (see `docs/TESTING.md`).
//!
//! Every golden file runs, in debug builds too: the workspace optimises
//! the solver crates there (root `Cargo.toml`) and keeps overflow checks,
//! which the open-dimension searches, adding lengths along one panel,
//! need most. The whole set takes a few minutes; set `GOLDEN_FILTER` to a
//! substring of the file names to run only some.

use std::fs;
use std::path::{Path, PathBuf};

use guillotine::{
    Algorithm, SolveParams, complete_open_dimension, select_algorithms, solve, solve_with_algorithm,
};
use model::{Instance, Solution};
use serde_json::Value;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Runs every golden file whose `<algorithm and queue size>` part of the
/// name is `what`, and reports all mismatches.
fn check(what: &str) {
    let filter = std::env::var("GOLDEN_FILTER").unwrap_or_default();
    let mut golden_paths: Vec<PathBuf> = fs::read_dir(data_dir().join("golden/9b656a0d"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_str().unwrap();
            name.split_once('.').map(|(_, rest)| rest) == Some(&format!("{what}.json"))
                && name.contains(&filter)
        })
        .collect();
    golden_paths.sort();

    let mut compared = 0;
    let mut failures = Vec::new();
    for path in golden_paths {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let stem = name.split_once('.').unwrap().0;
        let instance: Instance =
            read_json(&data_dir().join("instances").join(format!("{stem}.json")));
        let golden: Value = read_json(&path);
        let oracle = golden["oracle"].clone();
        let mut expected: Solution = serde_json::from_value(golden).unwrap();
        complete_open_dimension(&instance, &mut expected);

        let mut params = SolveParams::default();
        let algorithm = match oracle["algorithm"].as_str().unwrap() {
            "tree_search" => {
                params.queue_size = oracle["queue_size"].as_u64().unwrap();
                Some(Algorithm::TreeSearch)
            }
            "maximal_spaces" => {
                params.maximal_spaces_queue_size = oracle["queue_size"].as_u64().unwrap();
                Some(Algorithm::MaximalSpaces)
            }
            "hypergraph" => Some(Algorithm::Hypergraph),
            "sequential_single_knapsack" => Some(Algorithm::SequentialSingleKnapsack),
            "sequential_value_correction" => Some(Algorithm::SequentialValueCorrection),
            "auto" => {
                // Each improvement's label names the upstream algorithm
                // that found it: ours must have selected it too.
                let labels: Vec<&str> = oracle["improvements"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|label| label.as_str().unwrap())
                    .collect();
                let selected = select_algorithms(&instance).unwrap();
                for label in &labels {
                    let upstream = match label.split(' ').next().unwrap() {
                        "TS" => Algorithm::TreeSearch,
                        "TSMS" => Algorithm::MaximalSpaces,
                        "TSHIC" => Algorithm::Hypergraph,
                        "SSK" => Algorithm::SequentialSingleKnapsack,
                        "SVC" => Algorithm::SequentialValueCorrection,
                        // Column generation is Out.
                        "CG" | "CGS" => continue,
                        other => panic!("{name}: unknown label {other}"),
                    };
                    if !selected.contains(&upstream) {
                        failures.push(format!("{name}: {upstream:?} not selected: {selected:?}"));
                    }
                }
                // upstream's best plan is column generation's: ours cannot
                // be the same.
                if labels.last().is_some_and(|label| label.starts_with("CG")) {
                    continue;
                }
                None
            }
            other => panic!("{name}: unknown algorithm {other}"),
        };
        let output = match algorithm {
            Some(algorithm) => solve_with_algorithm(&instance, algorithm, &params),
            None => solve(&instance, &params, |_| {}),
        }
        .unwrap_or_else(|error| panic!("{name}: {error}"));
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
    eprintln!("compared {compared} golden files ({what})");
    assert!(compared > 0 || !filter.is_empty(), "nothing compared");
    assert!(
        failures.is_empty(),
        "{} of {compared} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn tree_search_at_queue_size_64() {
    check("q64");
}

#[test]
fn tree_search_at_queue_size_2048() {
    check("q2048");
}

#[test]
fn maximal_spaces_at_queue_size_16() {
    check("maximal_spaces.q16");
}

#[test]
fn hypergraph_infinite_copies() {
    check("hypergraph");
}

#[test]
fn sequential_single_knapsack() {
    check("sequential_single_knapsack");
}

#[test]
fn sequential_value_correction() {
    check("sequential_value_correction");
}

#[test]
fn automatic_selection() {
    check("auto");
}

/// Every open-dimension golden plan, completed as the port completes its
/// own, passes the checker. This needs no solving: it checks the golden
/// plans themselves, apart from the searches above.
#[test]
fn completed_open_dimension_plans_pass_the_checker() {
    let mut checked = 0;
    let mut failures = Vec::new();
    for entry in fs::read_dir(data_dir().join("golden/9b656a0d")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let stem = name.split_once('.').unwrap().0;
        let instance: Instance =
            read_json(&data_dir().join("instances").join(format!("{stem}.json")));
        if !instance.objective.is_open_dimension() {
            continue;
        }
        let mut plan: Solution = read_json(&path);
        if plan.patterns.is_empty() {
            continue;
        }
        complete_open_dimension(&instance, &mut plan);
        checked += 1;
        let report = checker::check(&instance, &plan);
        if !report.is_feasible() {
            failures.push(format!("{name}: {:?}", report.violations));
        }
    }
    assert!(checked > 0, "nothing checked");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
