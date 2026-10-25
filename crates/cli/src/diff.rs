//! `diff-oracle`: the differential suite against the golden files (see
//! `docs/TESTING.md`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Context;
use guillotine::{SolveError, SolveParams, solve};
use model::{Instance, Solution, Stages};
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

/// A golden file: `<instance>.q<queue size>.json`.
struct GoldenFile {
    path: PathBuf,
    instance: String,
    queue_size: u64,
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
        let name = format!("{} q{}", file.instance, file.queue_size);
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

/// The golden files selected by `options`, sorted by instance, then queue
/// size.
fn golden_files(options: &Options) -> anyhow::Result<Vec<GoldenFile>> {
    let entries = fs::read_dir(options.golden)
        .with_context(|| format!("reading {}", options.golden.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((instance, queue_size)) = name
            .strip_suffix(".json")
            .and_then(|stem| stem.rsplit_once(".q"))
            .and_then(|(instance, size)| Some((instance.to_string(), size.parse().ok()?)))
        else {
            continue;
        };
        if !name.contains(options.filter)
            || !(options.queue_sizes.is_empty() || options.queue_sizes.contains(&queue_size))
        {
            continue;
        }
        files.push(GoldenFile {
            path,
            instance,
            queue_size,
        });
    }
    files.sort_by(|a, b| (&a.instance, a.queue_size).cmp(&(&b.instance, b.queue_size)));
    Ok(files)
}

/// Solves one golden file's instance at its queue size and compares.
fn compare(file: &GoldenFile, instances: &Path) -> Outcome {
    let instance_path = instances.join(format!("{}.json", file.instance));
    let instance: Instance = match read_json(&instance_path) {
        Ok(instance) => instance,
        Err(error) => return Outcome::Error(format!("{error:#}")),
    };
    if instance.saw.stages == Stages::Unlimited {
        return Outcome::Skipped("unlimited stages (Stage 5)");
    }
    let expected = match read_golden(file) {
        Ok(expected) => expected,
        Err(error) => return Outcome::Error(format!("{error:#}")),
    };

    let params = SolveParams {
        queue_size: file.queue_size,
        ..SolveParams::default()
    };
    let solution = match solve(&instance, &params, |_| {}) {
        Ok(output) => output.solution,
        Err(SolveError::UnlimitedStages) => return Outcome::Skipped("unlimited stages (Stage 5)"),
        Err(error) => return Outcome::Error(error.to_string()),
    };

    if solution != expected {
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

/// Reads a golden file's plan, checking its recorded queue size.
fn read_golden(file: &GoldenFile) -> anyhow::Result<Solution> {
    let golden: Value = read_json(&file.path)?;
    let recorded = golden["oracle"]["queue_size"].as_u64();
    anyhow::ensure!(
        recorded == Some(file.queue_size),
        "{}: oracle.queue_size is {recorded:?}, the name says {}",
        file.path.display(),
        file.queue_size
    );
    serde_json::from_value(golden).with_context(|| format!("parsing {}", file.path.display()))
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
