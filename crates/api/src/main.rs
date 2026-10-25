//! sheetsolver web API server. `--help` lists the options; `docs/API.md`
//! describes the API.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use axum::http::HeaderValue;
use clap::Parser;

/// Serves the sheetsolver web API, and the web UI with `--web-dir`.
///
/// Every option can also be set by an environment variable, named after
/// it (`SHEETSOLVER_LISTEN`, `SHEETSOLVER_WEB_DIR`, …), as containers do.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Options {
    /// Address and port to listen on.
    #[arg(long, env = "SHEETSOLVER_LISTEN", default_value = "127.0.0.1:8080")]
    listen: SocketAddr,
    /// Directory of the built web UI, served at `/` (index.html for pages
    /// that are not files). Without it, only the API is served.
    #[arg(long, env = "SHEETSOLVER_WEB_DIR")]
    web_dir: Option<PathBuf>,
    /// Jobs solved at the same time, each on its own thread; further jobs
    /// wait in state `queued`. Default: the number of CPU cores.
    #[arg(long, env = "SHEETSOLVER_MAX_RUNNING_JOBS")]
    max_running_jobs: Option<usize>,
    /// Jobs that may wait for a solver; beyond that `POST /jobs` answers
    /// `503 busy`.
    #[arg(long, env = "SHEETSOLVER_MAX_QUEUED_JOBS", default_value_t = 32)]
    max_queued_jobs: usize,
    /// Seconds a job stays readable after it ends.
    #[arg(long, env = "SHEETSOLVER_KEEP_JOBS_FOR", default_value_t = 3600)]
    keep_jobs_for: u64,
    /// The solver's beam width: larger finds better plans, slower.
    #[arg(long, env = "SHEETSOLVER_QUEUE_SIZE", default_value_t = 2048)]
    queue_size: u64,
    /// Allowed browser origin, e.g. http://localhost:5173; repeat for
    /// several (comma-separated in the variable). Default: any origin.
    /// Only needed for a UI served from another origin, such as its
    /// development server.
    #[arg(
        long = "cors-origin",
        env = "SHEETSOLVER_CORS_ORIGINS",
        value_delimiter = ','
    )]
    cors_origins: Vec<String>,
    /// Print openapi.json to stdout and exit, without serving.
    #[arg(long)]
    print_openapi: bool,
}

fn main() -> anyhow::Result<()> {
    let options = Options::parse();
    if options.print_openapi {
        println!(
            "{}",
            serde_json::to_string_pretty(&api::openapi_document())?
        );
        return Ok(());
    }
    let mut config = api::Config {
        max_queued_jobs: options.max_queued_jobs,
        keep_jobs_for: Duration::from_secs(options.keep_jobs_for),
        queue_size: options.queue_size,
        ..api::Config::default()
    };
    if let Some(web_dir) = &options.web_dir {
        if let Some(warning) = api::check_web_dir(web_dir)? {
            eprintln!("warning: {warning}");
        }
        config.web_dir = Some(web_dir.clone());
    }
    if let Some(running) = options.max_running_jobs {
        anyhow::ensure!(running >= 1, "--max-running-jobs must be at least 1");
        config.max_running_jobs = running;
    }
    for origin in &options.cors_origins {
        config.cors_origins.push(
            HeaderValue::from_str(origin)
                .with_context(|| format!("--cors-origin {origin:?} is not a valid origin"))?,
        );
    }

    // The solvers run on the blocking pool, one thread per running job.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(config.max_running_jobs)
        .build()
        .context("starting the runtime")?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind(options.listen)
            .await
            .with_context(|| format!("listening on {}", options.listen))?;
        eprintln!(
            "sheetsolver api on http://{} ({} solvers, queue size {}{})",
            listener.local_addr()?,
            config.max_running_jobs,
            config.queue_size,
            config
                .web_dir
                .as_ref()
                .map(|dir| format!(", web UI from {}", dir.display()))
                .unwrap_or_default()
        );
        axum::serve(listener, api::router(&config))
            .with_graceful_shutdown(shutdown_signal())
            .await
            .context("serving")
    })
}

/// Resolves on Ctrl-C, or on SIGTERM, which `docker stop` sends.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}
