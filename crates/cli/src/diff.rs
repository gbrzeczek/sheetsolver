//! `diff-oracle`: the differential suite against the golden files (see
//! `docs/TESTING.md`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Context;
use guillotine::{Algorithm, SolveParams, solve, solve_with_algorithm};
use model::{Instance, Solution};
use serde_json::Value;

use crate::files::{count, describe, read_json};

/// What to compare.
pub struct Options<'a> {
    /// Directory of golden files.
    pub golden: &'a Path,
    /// Directory of instance files.
    pub instances: &'a Path,
    /// Queue sizes to compare; empty for all.
    pub queue_sizes: &'a [u64],
    /// Substring golden file names must contain.
    pub filter: &'a str,
    /// Print only the files that do not match.
    pub quiet: bool,
}

/// One golden file's result.
enum Outcome {
    /// The plan equals the golden one and passes the checker.
    Same,
    /// The plan differs from the golden one.
    Differs(String),
    /// The plan equals the golden one but breaks rules.
    Infeasible(String),
    /// Not compared yet: the solver does not support the instance.
    Skipped(&'static str),
    /// The instance or golden file could not be read, or solving failed.
    Error(String),
}

impl Outcome {
    fn label(&self) -> &'static str {
        match self {
            Outcome::Same => "same",
            Outcome::Differs(_) => "DIFFERS",
            Outcome::Infeasible(_) => "INFEASIBLE",
            Outcome::Skipped(_) => "skipped",
            Outcome::Error(_) => "ERROR",
        }
    }

    fn detail(&self) -> &str {
        match self {
            Outcome::Same => "",
            Outcome::Differs(detail) | Outcome::Infeasible(detail) | Outcome::Error(detail) => {
                detail
            }
            Outcome::Skipped(reason) => reason,
        }
    }

    fn is_failure(&self) -> bool {
        matches!(
            self,
            Outcome::Differs(_) | Outcome::Infeasible(_) | Outcome::Error(_)
        )
    }
}

/// A golden file: `<instance>.<what>.json`, where `<what>` names the
/// algorithm and queue size the oracle used (see `tools/run_oracle.py`);
/// the file's `oracle` header says the same.
struct GoldenFile {
    path: PathBuf,
    instance: String,
    /// The `<what>` part of the name, e.g. `q64` or `maximal_spaces.q16`.
    what: String,
    /// The queue size in the name, if any.
    queue_size: Option<u64>,
}

/// Runs the suite, prints one line per golden file and a summary, and
/// returns whether every compared instance matched.
pub fn run(options: &Options) -> anyhow::Result<bool> {
    let files = golden_files(options)?;
    anyhow::ensure!(
        !files.is_empty(),
        "no golden files match in {}",
        options.golden.display()
    );
    println!(
        "comparing {} golden files in {}",
        files.len(),
        options.golden.display()
    );

    let start = Instant::now();
    let mut counts = [0usize; 5];
    for file in &files {
        let file_start = Instant::now();
        let outcome = compare(file, options.instances);
        counts[match outcome {
            Outcome::Same => 0,
            Outcome::Differs(_) => 1,
            Outcome::Infeasible(_) => 2,
            Outcome::Skipped(_) => 3,
            Outcome::Error(_) => 4,
        }] += 1;
        if options.quiet && !outcome.is_failure() {
            continue;
        }
        let name = format!("{} {}", file.instance, file.what);
        let line = format!(
            "{:<10} {name:<72} {:7.2} s",
            outcome.label(),
            file_start.elapsed().as_secs_f64()
        );
        match outcome.detail() {
            "" => println!("{line}"),
            detail => println!("{line}  {detail}"),
        }
    }

    let [same, differs, infeasible, skipped, errors] = counts;
    println!(
        "\n{} golden files in {:.1} s: {same} same, {differs} differ, {infeasible} infeasible, \
         {errors} errors, {skipped} skipped",
        files.len(),
        start.elapsed().as_secs_f64()
    );
    let passed = differs + infeasible + errors == 0;
    println!("{}", if passed { "PASS" } else { "FAIL" });
    Ok(passed)
}

/// The golden files selected by `options`, sorted by name.
fn golden_files(options: &Options) -> anyhow::Result<Vec<GoldenFile>> {
    let entries = fs::read_dir(options.golden)
        .with_context(|| format!("reading {}", options.golden.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let Some(name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(String::from)
        else {
            continue;
        };
        let Some((instance, what)) = name
            .strip_suffix(".json")
            .and_then(|stem| stem.split_once('.'))
        else {
            continue;
        };
        let queue_size = what
            .rsplit('.')
            .next()
            .and_then(|last| last.strip_prefix('q'))
            .and_then(|size| size.parse().ok());
        if !name.contains(options.filter)
            || !(options.queue_sizes.is_empty()
                || queue_size.is_some_and(|size| options.queue_sizes.contains(&size)))
        {
            continue;
        }
        files.push(GoldenFile {
            path,
            instance: instance.to_string(),
            what: what.to_string(),
            queue_size,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// What the oracle ran, from a golden file's `oracle` header.
enum Run {
    /// One algorithm, with these parameters.
    One(Algorithm, SolveParams),
    /// upstream's own selection (`optimize()`), with the labels of its
    /// improvements.
    Auto(Vec<String>),
}

/// Reads a golden file's plan and what produced it.
fn read_golden(file: &GoldenFile) -> anyhow::Result<(Solution, Run)> {
    let golden: Value = read_json(&file.path)?;
    let oracle = &golden["oracle"];
    let recorded = oracle["queue_size"].as_u64();
    anyhow::ensure!(
        recorded == file.queue_size,
        "{}: oracle.queue_size is {recorded:?}, the name says {:?}",
        file.path.display(),
        file.queue_size
    );
    let mut params = SolveParams::default();
    let algorithm = match oracle["algorithm"].as_str() {
        Some("tree_search") => {
            params.queue_size = recorded.unwrap_or(params.queue_size);
            Algorithm::TreeSearch
        }
        Some("maximal_spaces") => {
            params.maximal_spaces_queue_size = recorded.unwrap_or(params.maximal_spaces_queue_size);
            Algorithm::MaximalSpaces
        }
        Some("hypergraph") => Algorithm::Hypergraph,
        Some("sequential_single_knapsack") => Algorithm::SequentialSingleKnapsack,
        Some("sequential_value_correction") => Algorithm::SequentialValueCorrection,
        Some("auto") => {
            let labels = oracle["improvements"]
                .as_array()
                .map(|labels| {
                    labels
                        .iter()
                        .filter_map(|l| l.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let solution = serde_json::from_value(golden)
                .with_context(|| format!("parsing {}", file.path.display()))?;
            return Ok((solution, Run::Auto(labels)));
        }
        other => anyhow::bail!(
            "{}: unknown oracle.algorithm {other:?}",
            file.path.display()
        ),
    };
    let solution = serde_json::from_value(golden)
        .with_context(|| format!("parsing {}", file.path.display()))?;
    Ok((solution, Run::One(algorithm, params)))
}

/// Solves one golden file's instance as the oracle did and compares.
fn compare(file: &GoldenFile, instances: &Path) -> Outcome {
    let instance_path = instances.join(format!("{}.json", file.instance));
    let instance: Instance = match read_json(&instance_path) {
        Ok(instance) => instance,
        Err(error) => return Outcome::Error(format!("{error:#}")),
    };
    let (mut expected, run) = match read_golden(file) {
        Ok(golden) => golden,
        Err(error) => return Outcome::Error(format!("{error:#}")),
    };
    // The port completes open-dimension plans that upstream leaves
    // partly uncut (a deliberate deviation): the oracle's get the same.
    guillotine::complete_open_dimension(&instance, &mut expected);

    let result = match &run {
        Run::One(algorithm, params) => solve_with_algorithm(&instance, *algorithm, params),
        Run::Auto(labels) => {
            // upstream's best plan came from column generation, which is
            // Out: ours cannot be the same.
            if labels.last().is_some_and(|label| label.starts_with("CG")) {
                return Outcome::Skipped("upstream's best plan is column generation's (Out)");
            }
            solve(&instance, &SolveParams::default(), |_| {})
        }
    };
    let solution = match result {
        Ok(output) => output.solution,
        Err(error) => return Outcome::Error(error.to_string()),
    };

    if solution != expected {
        if let Some(dir) = std::env::var_os("SHEETSOLVER_DUMP") {
            let path = Path::new(&dir).join(format!("{}.{}.ours.json", file.instance, file.what));
            let _ = fs::write(
                path,
                serde_json::to_string_pretty(&solution).unwrap_or_default(),
            );
        }
        return Outcome::Differs(format!(
            "ours: {}; oracle: {}",
            summary(&instance, &solution),
            summary(&instance, &expected)
        ));
    }
    if solution.patterns.is_empty() {
        return Outcome::Same;
    }
    let report = checker::check(&instance, &solution);
    if report.is_feasible() {
        Outcome::Same
    } else {
        let violations: Vec<String> = report.violations.iter().map(|v| v.to_string()).collect();
        Outcome::Infeasible(violations.join("; "))
    }
}

/// How a plan differs at a glance: its metrics, or that it is empty.
fn summary(instance: &Instance, solution: &Solution) -> String {
    if solution.patterns.is_empty() {
        return "no plan".to_string();
    }
    let metrics = checker::check(instance, solution).metrics;
    format!(
        "{}, {}",
        count(solution.patterns.len() as u32, "pattern"),
        describe(&metrics, instance.objective)
    )
}
