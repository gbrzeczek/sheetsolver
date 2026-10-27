//! Every oracle solution in `data/golden/` must pass the checker, and the
//! checker's metrics must equal upstream's own (see `docs/TESTING.md`).

use std::fs;
use std::path::{Path, PathBuf};

use checker::Problem;
use model::{Instance, Solution, Stages};
use serde_json::Value;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

/// (instance file stem, golden file path) for every golden file.
fn golden_files() -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();
    for commit_dir in fs::read_dir(data_dir().join("golden")).unwrap() {
        for entry in fs::read_dir(commit_dir.unwrap().path()).unwrap() {
            let path = entry.unwrap().path();
            let file_name = path.file_name().unwrap().to_str().unwrap();
            // `<instance>.<algorithm and queue size>.json`.
            let stem = file_name.split_once('.').unwrap().0.to_string();
            files.push((stem, path));
        }
    }
    files.sort();
    assert!(!files.is_empty(), "no golden files found");
    files
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn every_instance_is_valid() {
    for entry in fs::read_dir(data_dir().join("instances")).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().unwrap() == "MANIFEST.json" {
            continue;
        }
        let instance: Instance = read_json(&path);
        if let Err(errors) = instance.validate() {
            panic!("{}: {errors:?}", path.display());
        }
    }
}

#[test]
fn oracle_solutions_pass_and_metrics_match_upstream() {
    let mut failures = Vec::new();
    for (stem, path) in golden_files() {
        let instance: Instance =
            read_json(&data_dir().join("instances").join(format!("{stem}.json")));
        let solution: Solution = read_json(&path);
        let upstream: Value = read_json::<Value>(&path)["upstream_metrics"].clone();
        let report = checker::check(&instance, &solution);
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        if solution.patterns.is_empty() {
            // Upstream found no solution. Only objectives that may leave
            // parts out accept an empty plan, upstream and here.
            assert_eq!(
                upstream["feasible"],
                !instance.objective.cuts_every_part(),
                "{name}"
            );
            assert_eq!(
                report.is_feasible(),
                !instance.objective.cuts_every_part(),
                "{name}"
            );
            continue;
        }

        assert_eq!(upstream["feasible"], true, "{name}");
        if instance.objective.is_open_dimension()
            && instance.saw.stages == Stages::Unlimited
            && !report.is_feasible()
        {
            // Upstream cuts these plans on a shorter panel and leaves the
            // rest of the panel out of the tree; the solver completes them
            // (guillotine's open_dimension.rs). That must be all that is
            // wrong with them.
            for violation in &report.violations {
                assert!(
                    matches!(
                        violation.problem,
                        Problem::UnaccountedRemainder { .. } | Problem::NotSpanningParent { .. }
                    ),
                    "{name}: {violation}"
                );
            }
            continue;
        }
        if !report.is_feasible() {
            let lines: Vec<String> = report.violations.iter().map(|v| v.to_string()).collect();
            failures.push(format!("{name}:\n  {}", lines.join("\n  ")));
            continue;
        }

        let m = &report.metrics;
        let usable = m.panel_area - m.leftover_area;
        // Solved on the panel without its trims (unlimited stages, no
        // minimum offcut width): upstream's extent is measured from the
        // trimmed panel's corner, ours from the panel's.
        let moved = read_json::<Value>(&path)["oracle"]["trims"].is_string();
        let (dx, dy) = match (moved, solution.patterns.last()) {
            (true, Some(pattern)) => {
                let trims = instance.panel(&pattern.panel_id).unwrap().trims;
                (trims.left.width.tenths(), trims.bottom.width.tenths())
            }
            _ => (0, 0),
        };
        let ours = [
            ("number_of_bins", m.panels_used as i64),
            ("number_of_items", m.parts_cut as i64),
            ("full_area", m.panel_area.hundredths_mm2()),
            ("item_area", m.part_area.hundredths_mm2()),
            ("area", usable.hundredths_mm2()),
            ("waste", m.waste_area.hundredths_mm2()),
            ("width", m.used_length.tenths() - dx),
            ("height", m.used_width.tenths() - dy),
        ];
        for (key, value) in ours {
            if upstream[key].as_i64() != Some(value) {
                failures.push(format!(
                    "{name}: {key} is {value}, upstream {}",
                    upstream[key]
                ));
            }
        }
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0);
        if !close(m.total_value, upstream["profit"].as_f64().unwrap()) {
            failures.push(format!(
                "{name}: total_value {} != profit {}",
                m.total_value, upstream["profit"]
            ));
        }
        if !close(
            m.waste_fraction,
            upstream["waste_percentage"].as_f64().unwrap(),
        ) {
            failures.push(format!(
                "{name}: waste_fraction {} != waste_percentage {}",
                m.waste_fraction, upstream["waste_percentage"]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The instance and solution examples in `docs/ARCHITECTURE.md` are the
/// documented contract; they must stay valid.
#[test]
fn architecture_examples_are_valid() {
    let doc = fs::read_to_string(data_dir().join("../docs/ARCHITECTURE.md")).unwrap();
    let json_after = |heading: &str| -> String {
        let section = &doc[doc.find(heading).unwrap()..];
        let start = section.find("```json\n").unwrap() + "```json\n".len();
        section[start..start + section[start..].find("```").unwrap()].to_string()
    };
    let instance: Instance = serde_json::from_str(&json_after("### Instance")).unwrap();
    let solution: Solution = serde_json::from_str(&json_after("### Solution")).unwrap();
    instance.validate().unwrap();
    let report = checker::check(&instance, &solution);
    assert_eq!(report.violations, vec![]);
}
