//! Developer command-line tool: solve instances, check solutions, compare
//! the solver with the upstream oracle's golden plans and time it.
//!
//! ```text
//! cargo run --release -p cli -- diff-oracle
//! cargo run -p cli -- solve data/instances/tests__bin_packing_3nvo.json --time-limit 5
//! ```

mod bench;
mod check;
mod diff;
mod files;
mod solve;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};

/// sheetsolver developer tool.
#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Solve an instance and print the cutting plan as JSON.
    ///
    /// The plan goes to stdout (or --output); progress and a summary
    /// checked by the feasibility checker go to stderr.
    Solve {
        /// The instance JSON file.
        instance: PathBuf,
        /// Beam width of each search: larger finds better plans, slower.
        #[arg(long, default_value_t = 2048)]
        queue_size: u64,
        /// Stop after this many seconds and keep the best plan so far.
        #[arg(long, value_parser = parse_seconds)]
        time_limit: Option<Duration>,
        /// Write the plan to this file instead of stdout.
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// Print each better plan to stderr as the search finds it.
        #[arg(long)]
        progress: bool,
    },
    /// Check a cutting plan against its instance with the feasibility
    /// checker, and print its metrics and every violation.
    ///
    /// Exits with status 1 if the plan breaks any rule. Golden files can
    /// be checked directly: their extra fields are ignored.
    Check {
        /// The instance JSON file.
        instance: PathBuf,
        /// The solution JSON file.
        solution: PathBuf,
    },
    /// Solve every instance that has a golden file and compare with the
    /// oracle's plan: the full differential suite.
    ///
    /// Every plan must equal the golden one and pass the checker.
    /// Unlimited-stage instances are skipped until Stage 5. Exits with
    /// status 1 if any instance differs, fails the checker or errors.
    /// Run with --release: queue size 2048 is slow in debug builds.
    DiffOracle {
        /// Directory of golden files, `<instance>.q<queue size>.json`.
        #[arg(long, default_value_os_t = files::data_dir().join(files::GOLDEN_DIR))]
        golden: PathBuf,
        /// Directory of instance files.
        #[arg(long, default_value_os_t = files::data_dir().join("instances"))]
        instances: PathBuf,
        /// Only compare golden files at this queue size (repeatable).
        /// Default: every queue size found.
        #[arg(long = "queue-size")]
        queue_sizes: Vec<u64>,
        /// Only compare golden files whose name contains this text.
        #[arg(long, default_value = "")]
        filter: String,
        /// Print only instances that do not match, and the summary.
        #[arg(long, short)]
        quiet: bool,
    },
    /// Time the solver on a set of instances.
    ///
    /// Each instance is solved --repeat times per queue size; the fastest
    /// and median times are printed. Compare with upstream's times from
    /// `tools/bench_oracle.py`.
    BenchSet {
        /// Instance JSON files, or text files listing one instance name
        /// (a file stem in the instances directory) per line, `#` for
        /// comments. Default: `crates/guillotine/benches/instances.txt`.
        sets: Vec<PathBuf>,
        /// Directory of instance files, for names in list files.
        #[arg(long, default_value_os_t = files::data_dir().join("instances"))]
        instances: PathBuf,
        /// Queue size to time (repeatable). Default: 64 and 2048.
        #[arg(long = "queue-size")]
        queue_sizes: Vec<u64>,
        /// How many times to solve each instance per queue size.
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..))]
        repeat: u32,
    },
}

/// Parses a non-negative number of seconds, fractions allowed.
fn parse_seconds(text: &str) -> Result<Duration, String> {
    let seconds: f64 = text.parse().map_err(|_| format!("not a number: {text}"))?;
    Duration::try_from_secs_f64(seconds).map_err(|_| format!("not a valid duration: {text}"))
}

fn main() -> anyhow::Result<ExitCode> {
    let success = match Cli::parse().command {
        Command::Solve {
            instance,
            queue_size,
            time_limit,
            output,
            progress,
        } => solve::run(
            &instance,
            queue_size,
            time_limit,
            output.as_deref(),
            progress,
        )?,
        Command::Check { instance, solution } => check::run(&instance, &solution)?,
        Command::DiffOracle {
            golden,
            instances,
            queue_sizes,
            filter,
            quiet,
        } => diff::run(&diff::Options {
            golden: &golden,
            instances: &instances,
            queue_sizes: &queue_sizes,
            filter: &filter,
            quiet,
        })?,
        Command::BenchSet {
            sets,
            instances,
            queue_sizes,
            repeat,
        } => bench::run(&sets, &instances, &queue_sizes, repeat)?,
    };
    Ok(if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
