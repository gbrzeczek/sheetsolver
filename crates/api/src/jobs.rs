//! Jobs: the queue, the solver threads and what clients read.
//!
//! Each job's state lives in a `tokio::sync::watch` channel: the solver
//! thread writes it, `GET` reads it and each event stream waits for its
//! changes. A semaphore with one permit per solver bounds how many jobs
//! run at once; it hands permits out first come, first served, so jobs
//! leave the queue in the order they were submitted.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use guillotine::{SolveParams, solve};
use model::{Instance, Solution};
use tokio::sync::{Semaphore, watch};
use treesearch::Stop;
use uuid::Uuid;

use crate::request::JobSpec;
use crate::types::{EndedBy, JobState, JobStatus, PlanReport, StateEvent};

/// Settings of the job store.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Jobs solved at the same time.
    pub max_running_jobs: usize,
    /// Jobs that may wait for a solver.
    pub max_queued_jobs: usize,
    /// How long a job stays readable after it ends.
    pub keep_jobs_for: Duration,
    /// The solver's beam width (`SolveParams::queue_size`).
    pub queue_size: u64,
}

/// A job's changing state.
#[derive(Debug, Clone)]
pub struct JobData {
    /// Where the job is in its life.
    pub state: JobState,
    /// When the search started; `None` while queued.
    started: Option<Instant>,
    /// When the job ended; `None` until then.
    ended: Option<Instant>,
    /// Why the job ended, unless it failed.
    pub ended_by: Option<EndedBy>,
    /// How many better plans have been found.
    pub plans_found: u32,
    /// The best plan so far.
    pub best: Option<Arc<PlanReport>>,
    /// Why the job failed.
    pub message: Option<String>,
}

impl JobData {
    /// Seconds spent searching, to the millisecond.
    pub fn elapsed(&self) -> f64 {
        let elapsed = match (self.started, self.ended) {
            (Some(started), Some(ended)) => ended - started,
            (Some(started), None) => started.elapsed(),
            (None, _) => Duration::ZERO,
        };
        seconds(elapsed)
    }

    /// The data of a `state` event.
    pub fn state_event(&self) -> StateEvent {
        StateEvent {
            state: self.state,
            ended_by: self.ended_by,
            elapsed: self.elapsed(),
            plans_found: self.plans_found,
            message: self.message.clone(),
        }
    }
}

/// A duration in seconds, rounded to the millisecond.
fn seconds(duration: Duration) -> f64 {
    duration.as_millis() as f64 / 1000.0
}

/// One submitted job.
#[derive(Debug)]
pub struct Job {
    /// The job's id.
    pub id: Uuid,
    /// The time limit, in seconds.
    pub time_limit: u32,
    /// The order of submission: queued jobs start in this order.
    sequence: u64,
    /// The instance to solve.
    instance: Instance,
    /// Set to stop the search: by `DELETE`, or when a plan fails the
    /// checker.
    stop: Arc<AtomicBool>,
    /// The changing state.
    data: watch::Sender<JobData>,
}

impl Job {
    /// A receiver that sees every change of the job's state.
    pub fn subscribe(&self) -> watch::Receiver<JobData> {
        self.data.subscribe()
    }

    /// Asks the job to stop. A queued job is cancelled at once; a running
    /// one stops at the solver's next check.
    pub fn cancel(&self) {
        self.data.send_if_modified(|data| match data.state {
            JobState::Queued => {
                data.state = JobState::Cancelled;
                data.ended_by = Some(EndedBy::Cancelled);
                data.ended = Some(Instant::now());
                true
            }
            JobState::Running => {
                self.stop.store(true, Ordering::Relaxed);
                false
            }
            JobState::Done | JobState::Cancelled | JobState::Failed => false,
        });
    }

    fn state(&self) -> JobState {
        self.data.borrow().state
    }

    /// Whether the job has ended more than `keep` ago.
    fn expired(&self, keep: Duration) -> bool {
        self.data
            .borrow()
            .ended
            .is_some_and(|ended| ended.elapsed() > keep)
    }

    /// When the job reaches its time limit, if it is running.
    fn deadline(&self) -> Option<Instant> {
        let data = self.data.borrow();
        let started = data.started.filter(|_| data.state == JobState::Running)?;
        Some(started + Duration::from_secs(self.time_limit.into()))
    }
}

/// Why a job could not be submitted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubmitError {
    /// Too many jobs are waiting already.
    #[error("too many jobs are waiting")]
    Busy {
        /// When to try again: the time until the first running job
        /// reaches its time limit, at the latest. A solver then takes the
        /// first queued job, which frees a place in the queue.
        retry_after: Duration,
    },
}

/// Every job the server knows, and the solvers that run them.
#[derive(Debug)]
pub struct JobStore {
    limits: Limits,
    jobs: Mutex<HashMap<Uuid, Arc<Job>>>,
    solvers: Arc<Semaphore>,
    /// The sequence number of the next job.
    next_sequence: AtomicU64,
}

impl JobStore {
    /// An empty store with these limits.
    pub fn new(limits: Limits) -> Self {
        let solvers = Arc::new(Semaphore::new(limits.max_running_jobs.max(1)));
        JobStore {
            limits,
            jobs: Mutex::new(HashMap::new()),
            solvers,
            next_sequence: AtomicU64::new(0),
        }
    }

    /// The job with this id, unless it has expired.
    pub fn get(&self, id: Uuid) -> Option<Arc<Job>> {
        let mut jobs = self.jobs.lock().expect("job store poisoned");
        let job = jobs.get(&id)?;
        if job.expired(self.limits.keep_jobs_for) {
            jobs.remove(&id);
            return None;
        }
        Some(Arc::clone(job))
    }

    /// The job status, as `GET /jobs/{id}` returns it.
    pub fn status(&self, job: &Job) -> JobStatus {
        let data = job.data.borrow().clone();
        let queue_position = (data.state == JobState::Queued).then(|| {
            let jobs = self.jobs.lock().expect("job store poisoned");
            let ahead = jobs
                .values()
                .filter(|other| other.sequence < job.sequence && other.state() == JobState::Queued)
                .count();
            ahead as u32 + 1
        });
        JobStatus {
            id: job.id,
            state: data.state,
            ended_by: data.ended_by,
            queue_position,
            time_limit: job.time_limit,
            elapsed: data.elapsed(),
            plans_found: data.plans_found,
            best: data.best.as_deref().cloned(),
            message: data.message,
            instance: job.instance.clone(),
        }
    }

    /// Adds a job and starts it as soon as a solver is free. Must be
    /// called inside a Tokio runtime.
    pub fn submit(&self, spec: JobSpec) -> Result<Arc<Job>, SubmitError> {
        let mut jobs = self.jobs.lock().expect("job store poisoned");
        let keep = self.limits.keep_jobs_for;
        jobs.retain(|_, job| !job.expired(keep));

        // A job waits only if every solver is busy; it may wait if fewer
        // than `max_queued_jobs` others do.
        let queued = jobs
            .values()
            .filter(|job| job.state() == JobState::Queued)
            .count();
        let free = self.solvers.available_permits();
        if queued + 1 > self.limits.max_queued_jobs + free {
            let now = Instant::now();
            let retry_after = jobs
                .values()
                .filter_map(|job| job.deadline())
                .min()
                .map_or(Duration::ZERO, |deadline| {
                    deadline.saturating_duration_since(now)
                });
            return Err(SubmitError::Busy { retry_after });
        }

        let (data, _) = watch::channel(JobData {
            state: JobState::Queued,
            started: None,
            ended: None,
            ended_by: None,
            plans_found: 0,
            best: None,
            message: None,
        });
        let job = Arc::new(Job {
            id: Uuid::new_v4(),
            time_limit: spec.time_limit,
            sequence: self.next_sequence.fetch_add(1, Ordering::Relaxed),
            instance: spec.instance,
            stop: Arc::new(AtomicBool::new(false)),
            data,
        });
        jobs.insert(job.id, Arc::clone(&job));
        tokio::spawn(run(
            Arc::clone(&job),
            Arc::clone(&self.solvers),
            self.limits.queue_size,
        ));
        Ok(job)
    }
}

/// Waits for a solver, then solves the job on a blocking thread.
async fn run(job: Arc<Job>, solvers: Arc<Semaphore>, queue_size: u64) {
    let mut changes = job.subscribe();
    let permit = tokio::select! {
        permit = Arc::clone(&solvers).acquire_owned() => permit.expect("solver semaphore closed"),
        // Cancelled while queued: `Job::cancel` has ended it already.
        _ = changes.wait_for(|data| data.state != JobState::Queued) => return,
    };
    let started = job.data.send_if_modified(|data| {
        if data.state != JobState::Queued {
            return false;
        }
        data.state = JobState::Running;
        data.started = Some(Instant::now());
        true
    });
    if !started {
        return;
    }

    let solver_job = Arc::clone(&job);
    let ending = tokio::task::spawn_blocking(move || solve_job(&solver_job, queue_size))
        .await
        .unwrap_or_else(|_| Err("the solver stopped unexpectedly".to_string()));
    drop(permit);

    job.data.send_modify(|data| {
        match ending {
            Ok(ended_by) => {
                data.state = match ended_by {
                    EndedBy::Cancelled => JobState::Cancelled,
                    EndedBy::Complete | EndedBy::TimeLimit => JobState::Done,
                };
                data.ended_by = Some(ended_by);
            }
            Err(message) => {
                data.state = JobState::Failed;
                data.message = Some(message);
            }
        }
        data.ended = Some(Instant::now());
    });
}

/// Solves the job's instance, publishing each better plan that passes
/// the checker. Returns why the search ended, or why the job failed.
fn solve_job(job: &Job, queue_size: u64) -> Result<EndedBy, String> {
    let instance = &job.instance;
    let start = job.data.borrow().started.unwrap_or_else(Instant::now);
    let deadline = start + Duration::from_secs(job.time_limit.into());
    let params = SolveParams {
        queue_size,
        stop: Stop {
            deadline: Some(deadline),
            cancel: Some(Arc::clone(&job.stop)),
            ..Stop::default()
        },
        ..SolveParams::default()
    };
    let mut publisher = Publisher {
        job,
        instance,
        start,
        last: None,
        failure: None,
    };
    let result = solve(instance, &params, |solution| publisher.offer(solution));
    // The solver stops early only for the stop flag or the deadline (see
    // `treesearch::Stop`), so whichever holds now is why it stopped; if
    // neither does, it ran to its end. A plan that failed the checker
    // sets the flag too, but then the failure is reported instead.
    let cancelled = job.stop.load(Ordering::Relaxed);
    let timed_out = Instant::now() >= deadline;
    match result {
        // The final plan is the last one reported; offering it again
        // changes nothing unless a solver path forgot to report it.
        Ok(output) => publisher.offer(&output.solution),
        Err(error) => publisher.failure = Some(format!("the solver refused the job: {error}")),
    }
    match publisher.failure {
        Some(failure) => Err(failure),
        None if cancelled => Ok(EndedBy::Cancelled),
        None if timed_out => Ok(EndedBy::TimeLimit),
        None => Ok(EndedBy::Complete),
    }
}

/// Checks the plans the solver reports and publishes the good ones.
struct Publisher<'a> {
    job: &'a Job,
    instance: &'a Instance,
    start: Instant,
    /// The last plan published.
    last: Option<Solution>,
    /// Set once a plan fails the checker; nothing is published after.
    failure: Option<String>,
}

impl Publisher<'_> {
    fn offer(&mut self, solution: &Solution) {
        // An empty plan cuts nothing: it is no plan at all.
        if self.failure.is_some()
            || solution.patterns.is_empty()
            || self.last.as_ref() == Some(solution)
        {
            return;
        }
        let report = checker::check(self.instance, solution);
        if !report.is_feasible() {
            let violations: Vec<String> =
                report.violations.iter().map(ToString::to_string).collect();
            self.failure = Some(format!(
                "the solver produced a plan that breaks the cutting rules: {}",
                violations.join("; ")
            ));
            self.job.stop.store(true, Ordering::Relaxed);
            return;
        }
        let found_after = seconds(self.start.elapsed());
        self.job.data.send_modify(|data| {
            data.plans_found += 1;
            data.best = Some(Arc::new(PlanReport {
                number: data.plans_found,
                found_after,
                metrics: report.metrics,
                plan: solution.clone(),
            }));
        });
        self.last = Some(solution.clone());
    }
}
