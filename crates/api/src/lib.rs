//! sheetsolver web API: the solver as a service.
//!
//! `POST /jobs` submits an instance, `GET /jobs/{id}` and
//! `GET /jobs/{id}/events` follow the search, `DELETE /jobs/{id}` stops
//! it and `GET /openapi.json` describes it all. The contract is
//! `docs/API.md`.
//!
//! - `types`: the JSON bodies.
//! - `request`: reading a submitted job, reporting every problem.
//! - `jobs`: the queue and the solver threads.
//! - `routes`: the HTTP handlers.
//! - `openapi`: the generated OpenAPI document.

mod error;
mod jobs;
mod openapi;
mod request;
mod routes;
pub mod types;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, Method, header};
use axum::routing::{get, post};
use tower_http::cors::{AllowOrigin, CorsLayer};

pub use crate::openapi::document as openapi_document;

/// The largest request body accepted, in bytes.
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Server settings (the command-line options of the `api` binary).
#[derive(Debug, Clone)]
pub struct Config {
    /// Jobs solved at the same time, each on its own thread.
    pub max_running_jobs: usize,
    /// Jobs that may wait for a solver; beyond that `POST /jobs` answers
    /// `503 busy`.
    pub max_queued_jobs: usize,
    /// How long a job stays readable after it ends.
    pub keep_jobs_for: Duration,
    /// The solver's beam width.
    pub queue_size: u64,
    /// Browser origins allowed to call the API; empty allows any.
    pub cors_origins: Vec<HeaderValue>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            max_running_jobs: std::thread::available_parallelism().map_or(1, |n| n.get()),
            max_queued_jobs: 32,
            keep_jobs_for: Duration::from_secs(3600),
            queue_size: 2048,
            cors_origins: Vec::new(),
        }
    }
}

/// The API's routes. Must be served inside a Tokio runtime: jobs are
/// spawned on it.
pub fn router(config: &Config) -> Router {
    let store = Arc::new(jobs::JobStore::new(jobs::Limits {
        max_running_jobs: config.max_running_jobs,
        max_queued_jobs: config.max_queued_jobs,
        keep_jobs_for: config.keep_jobs_for,
        queue_size: config.queue_size,
    }));
    let origins = if config.cors_origins.is_empty() {
        AllowOrigin::any()
    } else {
        AllowOrigin::list(config.cors_origins.iter().cloned())
    };
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE])
        .expose_headers([header::LOCATION]);

    Router::new()
        .route("/jobs", post(routes::submit))
        .route("/jobs/{id}", get(routes::status).delete(routes::cancel))
        .route("/jobs/{id}/events", get(routes::events))
        .route("/openapi.json", get(routes::openapi))
        .fallback(routes::not_found)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(cors)
        .with_state(store)
}
