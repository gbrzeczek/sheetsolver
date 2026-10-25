//! The JSON bodies of the API (see `docs/API.md`).
//!
//! Doc comments on fields become the `description`s in `openapi.json`.

use model::{Instance, Metrics, Solution};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// The time limit when a job does not give one, in seconds.
pub const DEFAULT_TIME_LIMIT: u32 = 10;
/// The shortest time limit a job may ask for, in seconds.
pub const MIN_TIME_LIMIT: u32 = 1;
/// The longest time limit a job may ask for, in seconds.
pub const MAX_TIME_LIMIT: u32 = 600;

/// The body of `POST /jobs`: what to solve, and for how long.
///
/// Only used for the OpenAPI description: the server reads the body field
/// by field, to report every problem at once (see `request.rs`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobRequest {
    /// The parts to cut, the panels to cut them from and how the saw cuts.
    pub instance: Instance,
    /// How long the solver may search for a better plan. The first plan
    /// usually appears within a second; longer searches find plans with
    /// less waste. You can stop the search at any time and keep the best
    /// plan so far.
    #[serde(default = "default_time_limit")]
    pub time_limit: u32,
}

fn default_time_limit() -> u32 {
    DEFAULT_TIME_LIMIT
}

/// Where a job is in its life. `done`, `cancelled` and `failed` are final.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// Waiting for a free solver.
    Queued,
    /// Searching; the best plan improves over time.
    Running,
    /// The search finished: it was complete, or the time limit was reached.
    Done,
    /// Stopped by `DELETE /jobs/{id}`.
    Cancelled,
    /// Stopped by an internal error.
    Failed,
}

impl JobState {
    /// Whether the job has ended and will not change again.
    pub fn is_final(self) -> bool {
        matches!(
            self,
            JobState::Done | JobState::Cancelled | JobState::Failed
        )
    }
}

/// Why a job ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EndedBy {
    /// The search ran to its end within the time limit: a longer time
    /// limit would give the same plan.
    Complete,
    /// The time limit was reached: a longer search may find a better
    /// plan.
    TimeLimit,
    /// Stopped by `DELETE /jobs/{id}`.
    Cancelled,
}

/// A plan found by a job, with its figures.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct PlanReport {
    /// 1 for the first plan found, 2 for the next better one, and so on.
    pub number: u32,
    /// Seconds after the job started running when this plan was found.
    pub found_after: f64,
    /// Figures for this plan, computed by the independent checker.
    pub metrics: Metrics,
    /// The cutting plan: one cutting tree per distinct way of cutting a
    /// panel.
    pub plan: Solution,
}

/// A job's state and best plan so far.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct JobStatus {
    /// The job's id, used in the other URLs.
    pub id: Uuid,
    /// Where the job is in its life.
    pub state: JobState,
    /// Why the job ended: `complete` (the search ran to its end; a longer
    /// time limit would give the same plan), `time_limit` (a longer search
    /// may find a better plan) or `cancelled`. `null` until the job ends,
    /// and for a failed job.
    pub ended_by: Option<EndedBy>,
    /// For a queued job, its place in the queue: 1 runs next. `null` once
    /// the job has left the queue.
    pub queue_position: Option<u32>,
    /// The time limit the job runs with, in seconds.
    pub time_limit: u32,
    /// Seconds spent searching: 0 while queued, growing while running,
    /// final once the job has ended.
    pub elapsed: f64,
    /// How many better plans have been found so far.
    pub plans_found: u32,
    /// The best plan so far; `null` if none has been found (yet).
    pub best: Option<PlanReport>,
    /// Why the job failed, in state `failed`; otherwise `null`.
    pub message: Option<String>,
    /// The instance as submitted, with left-out fields filled with their
    /// defaults (and unknown fields dropped).
    pub instance: Instance,
}

/// The data of a `state` event: a job's state changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct StateEvent {
    /// Where the job is in its life.
    pub state: JobState,
    /// Why the job ended, as in the job status; `null` until it ends.
    pub ended_by: Option<EndedBy>,
    /// Seconds spent searching so far, as in the job status.
    pub elapsed: f64,
    /// How many better plans have been found so far.
    pub plans_found: u32,
    /// Why the job failed, in state `failed`; otherwise `null`.
    pub message: Option<String>,
}

/// A stable error code for programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The body is not JSON at all.
    InvalidJson,
    /// No job with this id, or it has expired.
    NotFound,
    /// The body is larger than the limit.
    TooLarge,
    /// The body is JSON but not a valid job.
    InvalidInstance,
    /// Too many jobs are already waiting.
    Busy,
}

/// The body of every error response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ErrorBody {
    /// A stable code for programs.
    pub error: ErrorCode,
    /// One sentence for people.
    pub message: String,
    /// For `invalid_instance`, every problem found; empty otherwise.
    pub problems: Vec<FieldProblem>,
}

/// One thing wrong with a submitted job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FieldProblem {
    /// The path of the field at fault in the request body, with 0-based
    /// list positions (`instance.parts[2].quantity`, `time_limit`), or of
    /// a whole list for a problem with the list (`instance.parts`);
    /// `null` when no part of the body is at fault.
    pub field: Option<String>,
    /// What is wrong, written for the woodworker, to show next to the
    /// field as is.
    pub message: String,
}

impl FieldProblem {
    /// A problem with the field at `field`.
    pub fn at(field: impl Into<String>, message: impl Into<String>) -> Self {
        FieldProblem {
            field: Some(field.into()),
            message: message.into(),
        }
    }
}
