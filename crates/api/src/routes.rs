//! The HTTP handlers.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::Stream;
use futures_util::stream;
use tokio::sync::watch;
use uuid::Uuid;

use crate::client::Client;
use crate::error::ApiError;
use crate::jobs::{Job, JobData, JobStore, SubmitError};
use crate::request::parse_job;
use crate::types::{ErrorBody, JobRequest, JobState, JobStatus, PlanReport, StateEvent};

/// How often an idle event stream sends a comment to stay open.
const KEEP_ALIVE: Duration = Duration::from_secs(15);

/// Submit an instance to solve.
///
/// The job then runs on its own: poll `GET /jobs/{id}` or follow
/// `GET /jobs/{id}/events`.
#[utoipa::path(
    post,
    path = "/jobs",
    operation_id = "submit_job",
    tag = "jobs",
    request_body = JobRequest,
    responses(
        (status = 201, description = "The job was created; usually `queued` or `running`.", body = JobStatus,
            headers(("Location" = String, description = "The job's URL, `/jobs/{id}`."))),
        (status = 400, description = "The body is not JSON (`invalid_json`).", body = ErrorBody),
        (status = 413, description = "The body is larger than 2 MB (`too_large`).", body = ErrorBody),
        (status = 422, description = "The body is not a valid job (`invalid_instance`); `problems` lists every problem found.", body = ErrorBody),
        (status = 429, description = "This client already has as many jobs queued or running as it may (`too_many_jobs`); stop one or wait for one to end.", body = ErrorBody),
        (status = 503, description = "Too many jobs are waiting (`busy`).", body = ErrorBody,
            headers(("Retry-After" = u64, description = "Seconds after which to try again: when the first running job reaches its time limit, at the latest."))),
    ),
)]
pub async fn submit(
    State(store): State<Arc<JobStore>>,
    Client(client): Client,
    body: Result<Bytes, BytesRejection>,
) -> Result<Response, ApiError> {
    let body = body.map_err(|rejection| match rejection.status() {
        StatusCode::PAYLOAD_TOO_LARGE => ApiError::TooLarge,
        _ => ApiError::InvalidJson(rejection.body_text()),
    })?;
    let value: serde_json::Value =
        serde_json::from_slice(&body).map_err(|error| ApiError::InvalidJson(error.to_string()))?;
    let spec = parse_job(&value).map_err(ApiError::InvalidInstance)?;
    let job = store.submit(spec, client).map_err(|error| match error {
        SubmitError::Busy { retry_after } => ApiError::Busy {
            // Whole seconds, rounded up, and never 0: a solver may free
            // up any moment, but asking again at once is pointless.
            retry_after: retry_after.as_secs_f64().ceil().max(1.0) as u64,
        },
        SubmitError::TooManyJobs { max } => ApiError::TooManyJobs { max },
    })?;
    let location = format!("/jobs/{}", job.id);
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(store.status(&job)),
    )
        .into_response())
}

/// The job's state and best plan so far.
#[utoipa::path(
    get,
    path = "/jobs/{id}",
    operation_id = "get_job",
    tag = "jobs",
    params(("id" = Uuid, Path, description = "The job's id.")),
    responses(
        (status = 200, description = "The job status.", body = JobStatus),
        (status = 404, description = "No job with this id, or it has expired (`not_found`).", body = ErrorBody),
    ),
)]
pub async fn status(
    State(store): State<Arc<JobStore>>,
    Path(id): Path<String>,
) -> Result<Json<JobStatus>, ApiError> {
    let job = find(&store, &id)?;
    Ok(Json(store.status(&job)))
}

/// Stop the search; the best plan so far is kept.
///
/// A queued job is cancelled at once; a running job stops at the solver's
/// next check, normally within milliseconds. The response is the status
/// at the time of the request, so it may still say `running`.
#[utoipa::path(
    delete,
    path = "/jobs/{id}",
    operation_id = "cancel_job",
    tag = "jobs",
    params(("id" = Uuid, Path, description = "The job's id.")),
    responses(
        (status = 202, description = "The stop was requested (or the job had already ended).", body = JobStatus),
        (status = 404, description = "No job with this id, or it has expired (`not_found`).", body = ErrorBody),
    ),
)]
pub async fn cancel(
    State(store): State<Arc<JobStore>>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<JobStatus>), ApiError> {
    let job = find(&store, &id)?;
    job.cancel();
    Ok((StatusCode::ACCEPTED, Json(store.status(&job))))
}

/// Live stream of better plans and state changes (Server-Sent Events).
///
/// Events: `plan` (data: a `PlanReport`) when a better plan is found, and
/// `state` (data: a `StateEvent`) when the state changes. On connecting,
/// the current best plan (if any), then the current state. The stream
/// ends after a `state` event with a final state; close the
/// `EventSource` then, or it reconnects.
#[utoipa::path(
    get,
    path = "/jobs/{id}/events",
    operation_id = "job_events",
    tag = "jobs",
    params(("id" = Uuid, Path, description = "The job's id.")),
    responses(
        (status = 200, description = "An event stream of `plan` events (data: `PlanReport`) and `state` events (data: `StateEvent`).",
            content_type = "text/event-stream", body = String),
        (status = 404, description = "No job with this id, or it has expired (`not_found`).", body = ErrorBody),
    ),
)]
pub async fn events(
    State(store): State<Arc<JobStore>>,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let job = find(&store, &id)?;
    let stream = event_stream(job);
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(KEEP_ALIVE)))
}

/// The OpenAPI 3.1 description of this API.
#[utoipa::path(
    get,
    path = "/openapi.json",
    operation_id = "get_openapi",
    tag = "openapi",
    responses((status = 200, description = "This document.", content_type = "application/json", body = Object)),
)]
pub async fn openapi() -> impl IntoResponse {
    Json(crate::openapi::document())
}

/// The job named by `id`; an id that is not a UUID names no job.
fn find(store: &JobStore, id: &str) -> Result<Arc<Job>, ApiError> {
    let id = Uuid::parse_str(id).map_err(|_| ApiError::NotFound)?;
    store.get(id).ok_or(ApiError::NotFound)
}

/// What an event stream has sent so far.
struct Progress {
    changes: watch::Receiver<JobData>,
    /// The number of the last plan sent; 0 before the first.
    plan_sent: u32,
    /// The last state sent.
    state_sent: Option<JobState>,
    /// Events taken from the last change, not sent yet.
    pending: Vec<Event>,
    /// Whether the final state has been taken.
    finished: bool,
    /// Keeps the job, and so its channel, alive while the stream is.
    _job: Arc<Job>,
}

/// The events of `job`: the current plan and state, then one batch per
/// change, until the final state.
///
/// Each change is read whole, so a plan is always sent before the state
/// that follows it, and the last plan is in the snapshot of the final
/// state, so it is never skipped. Plans in between may be: a slow reader
/// gets the latest.
fn event_stream(job: Arc<Job>) -> impl Stream<Item = Result<Event, Infallible>> {
    let progress = Progress {
        changes: job.subscribe(),
        plan_sent: 0,
        state_sent: None,
        pending: Vec::new(),
        finished: false,
        _job: job,
    };
    stream::unfold(progress, |mut progress| async move {
        loop {
            if !progress.pending.is_empty() {
                let event = progress.pending.remove(0);
                return Some((Ok(event), progress));
            }
            if progress.finished {
                return None;
            }
            // The first snapshot is the current one; then wait for changes.
            if progress.state_sent.is_some() && progress.changes.changed().await.is_err() {
                return None;
            }
            let data = progress.changes.borrow_and_update().clone();
            if let Some(best) = data.best.as_deref() {
                if best.number > progress.plan_sent {
                    progress.plan_sent = best.number;
                    progress.pending.push(plan_event(best));
                }
            }
            if progress.state_sent != Some(data.state) {
                progress.state_sent = Some(data.state);
                progress.finished = data.state.is_final();
                progress.pending.push(state_event(&data.state_event()));
            }
        }
    })
}

fn plan_event(plan: &PlanReport) -> Event {
    Event::default()
        .event("plan")
        .json_data(plan)
        .expect("a plan serializes")
}

fn state_event(state: &StateEvent) -> Event {
    Event::default()
        .event("state")
        .json_data(state)
        .expect("a state serializes")
}
