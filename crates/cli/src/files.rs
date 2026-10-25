//! Reading instances and solutions, and where the repository's data lives.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use model::{Instance, Metrics, Objective, Solution};
use serde::de::DeserializeOwned;

/// The golden files of the pinned upstream commit (`docs/UPSTREAM.md`),
/// relative to [`data_dir`].
pub const GOLDEN_DIR: &str = "golden/9b656a0d";

/// The repository's `data` directory. The CLI is a developer tool run from
/// a checkout, so defaults point into the checkout it was built from,
/// whatever the working directory.
pub fn data_dir() -> PathBuf {
    repository_dir().join("data")
}

/// The root of the checkout this binary was built from.
pub fn repository_dir() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Canonical only to print a shorter path; the joined one works too.
    fs::canonicalize(&path).unwrap_or(path)
}

/// Reads and parses a JSON file.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Reads an instance and checks that it is valid.
pub fn read_instance(path: &Path) -> anyhow::Result<Instance> {
    let instance: Instance = read_json(path)?;
    if let Err(errors) = instance.validate() {
        anyhow::bail!("{}: invalid instance: {errors:?}", path.display());
    }
    Ok(instance)
}

/// Reads a solution. Unknown fields, such as a golden file's `oracle`
/// block, are ignored.
pub fn read_solution(path: &Path) -> anyhow::Result<Solution> {
    read_json(path)
}

/// A one-line summary of a plan's metrics, in woodworking terms.
pub fn describe(metrics: &Metrics, objective: Objective) -> String {
    let mut text = format!(
        "{}, {}, waste {:.2} %",
        count(metrics.panels_used, "panel"),
        count(metrics.parts_cut, "part"),
        metrics.waste_fraction * 100.0
    );
    match objective {
        Objective::BinPacking => {}
        Objective::Knapsack => text += &format!(", value {}", metrics.total_value),
        Objective::OpenDimensionX => text += &format!(", used length {}", metrics.used_length),
        Objective::OpenDimensionY => text += &format!(", used width {}", metrics.used_width),
    }
    text
}

/// `count` followed by `noun`, plural unless the count is one.
pub fn count(count: u32, noun: &str) -> String {
    match count {
        1 => format!("1 {noun}"),
        _ => format!("{count} {noun}s"),
    }
}
