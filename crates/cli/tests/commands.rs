//! Runs the built `cli` binary on small instances: each subcommand must
//! succeed on good input and fail, with a non-zero exit status, on bad.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

/// A small 3-stage instance with golden files.
fn instance() -> PathBuf {
    data_dir().join("instances/tests__bin_packing_3nvo.json")
}

fn golden() -> PathBuf {
    data_dir().join("golden/9b656a0d/tests__bin_packing_3nvo.q64.json")
}

/// A fresh, empty directory for one test's files.
fn scratch_dir(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Shifts the first part of a plan by one tenth of a millimetre.
fn break_plan(plan: &str) -> String {
    let mut value: serde_json::Value = serde_json::from_str(plan).unwrap();
    let first_child = &mut value["patterns"][0]["root"]["children"][0];
    first_child["x"] = serde_json::json!(0.1);
    value.to_string()
}

#[test]
fn solve_prints_a_plan_that_check_accepts() {
    let dir = scratch_dir("solve");
    let plan = dir.join("plan.json");
    let output = cli(&[
        "solve",
        instance().to_str().unwrap(),
        "--queue-size",
        "64",
        "--time-limit",
        "5",
        "--output",
        plan.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{output:?}");

    let output = cli(&[
        "check",
        instance().to_str().unwrap(),
        plan.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{output:?}");
    assert!(stdout(&output).contains("feasible"));
}

#[test]
fn solve_handles_unlimited_stages() {
    let instance = data_dir().join("instances/generated__knapsack_unlimited_non_exact_kerf0.json");
    let output = cli(&["solve", instance.to_str().unwrap(), "--time-limit", "5"]);
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn check_rejects_a_broken_plan() {
    let dir = scratch_dir("check");
    let plan = dir.join("plan.json");
    fs::write(&plan, break_plan(&fs::read_to_string(golden()).unwrap())).unwrap();
    let output = cli(&[
        "check",
        instance().to_str().unwrap(),
        plan.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stdout(&output).contains("violations"));
}

#[test]
fn diff_oracle_passes_on_matching_golden_files() {
    let output = cli(&["diff-oracle", "--filter", "tests__bin_packing_3nvo."]);
    assert!(output.status.success(), "{output:?}");
    let text = stdout(&output);
    assert!(text.contains("2 same, 0 differ"), "{text}");
    assert!(text.contains("PASS"), "{text}");
}

#[test]
fn diff_oracle_compares_unlimited_stages() {
    let output = cli(&[
        "diff-oracle",
        "--filter",
        "generated__knapsack_unlimited_non_exact_kerf0.hypergraph",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert!(stdout(&output).contains("1 same"), "{output:?}");
}

#[test]
fn diff_oracle_fails_on_a_different_golden_plan() {
    let dir = scratch_dir("diff");
    fs::write(
        dir.join("tests__bin_packing_3nvo.q64.json"),
        break_plan(&fs::read_to_string(golden()).unwrap()),
    )
    .unwrap();
    let output = cli(&["diff-oracle", "--golden", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = stdout(&output);
    assert!(text.contains("DIFFERS"), "{text}");
    assert!(text.contains("FAIL"), "{text}");
}

#[test]
fn bench_set_times_an_instance() {
    let output = cli(&[
        "bench-set",
        instance().to_str().unwrap(),
        "--queue-size",
        "64",
        "--repeat",
        "1",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert!(stdout(&output).contains("tests__bin_packing_3nvo"));
}
