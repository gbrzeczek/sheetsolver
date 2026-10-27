// The state of the app: the job being edited, and the job being solved.
import { cancelJob, getJob, submitJob } from "../api/client";
import { followJob } from "../api/events";
import { isFinal, type ErrorBody, type JobRequest, type JobStatus, type PlanReport } from "../api/types";
import { draftMatches, fromInstance, newDraft, toRequest, type Draft, type Problem } from "./draft";
import { loadDraft, saveDraft } from "./storage";
import { router } from "../router.svelte";
import { layout } from "../layout.svelte";

/** A plan found, as the "Plans found" list shows it. */
export interface PlanSummary {
  number: number;
  foundAfter: number;
  panels: number;
  waste: number;
}

/** Why the cutting plan cannot be shown. */
export type JobTrouble =
  | { kind: "not_found" }
  | { kind: "busy"; retryAt: number }
  | { kind: "unreachable"; message: string };

class Session {
  draft = $state<Draft>(loadDraft() ?? newDraft());
  /** Problems with the draft, from the local checks or a 422. */
  problems = $state<Problem[]>([]);
  /** The summary line of a 422, or of the local checks. */
  problemsMessage = $state<string | null>(null);
  submitting = $state(false);
  /** Set when submitting failed for a reason other than the job itself. */
  submitTrouble = $state<JobTrouble | null>(null);

  jobId = $state<string | null>(null);
  job = $state<JobStatus | null>(null);
  best = $state<PlanReport | null>(null);
  history = $state<PlanSummary[]>([]);
  connected = $state(true);
  trouble = $state<JobTrouble | null>(null);
  stopping = $state(false);
  /** When `job.elapsed` was last received, for the time bar between events. */
  elapsedAt = $state(0);

  /** The draft that *Edit this plan's job* replaced, while its Undo is offered. */
  replaced = $state<{ draft: Draft; timer: number } | null>(null);

  #unfollow: (() => void) | null = null;
  #poll: ReturnType<typeof setInterval> | null = null;

  save(): void {
    saveDraft($state.snapshot(this.draft) as Draft);
  }

  /** Clears the problems of one field once it is edited. */
  touch(path: string): void {
    if (this.problems.some((p) => p.field === path)) {
      this.problems = this.problems.filter((p) => p.field !== path);
    }
  }

  /** Checks the draft, then submits it. Returns whether a job was created. */
  async submit(timeLimit?: number): Promise<boolean> {
    const { request, problems } = toRequest($state.snapshot(this.draft) as Draft);
    if (problems.length) {
      this.problems = problems;
      this.problemsMessage = "The job cannot be solved as entered.";
      // "Find plan again" on the Cutting plan tab: show where the problems are.
      if (!this.editorShown) router.go({ name: "setup" });
      return false;
    }
    if (timeLimit !== undefined) request.time_limit = timeLimit;
    return this.#send(request);
  }

  /** Submits the job shown, again, e.g. with a longer time limit. */
  async again(timeLimit?: number): Promise<boolean> {
    const job = this.job;
    if (!job) return false;
    const request: JobRequest = { instance: $state.snapshot(job.instance), time_limit: timeLimit ?? job.time_limit };
    if (job.name) request.name = job.name;
    return this.#send(request);
  }

  async #send(request: JobRequest): Promise<boolean> {
    // One job at a time: a second one would search unseen, holding a solver.
    if (this.submitting) return false;
    this.submitting = true;
    this.submitTrouble = null;
    const result = await submitJob(request);
    this.submitting = false;
    if (result.ok) {
      this.problems = [];
      this.problemsMessage = null;
      this.#show(result.job);
      // The workbench, or below 1280 px the Cutting plan tab.
      router.go({ name: "plan", id: result.job.id, panel: null, cut: null });
      return true;
    }
    this.#rejected(result.status, result.error, result.retryAfter);
    return false;
  }

  #rejected(status: number, error: ErrorBody, retryAfter: number | null): void {
    if (status === 422) {
      this.problems = error.problems.map((p) => ({ field: p.field ?? null, message: p.message }));
      this.problemsMessage = error.message;
      if (!this.editorShown) router.go({ name: "setup" });
    } else if (status === 503) {
      this.submitTrouble = { kind: "busy", retryAt: Date.now() + (retryAfter ?? 30) * 1000 };
    } else {
      this.submitTrouble = { kind: "unreachable", message: error.message };
    }
  }

  /** Shows a job: from a submit, or from a link. */
  async open(id: string): Promise<void> {
    if (this.jobId === id && (this.job || this.trouble)) return;
    this.#stopFollowing();
    this.jobId = id;
    this.job = null;
    this.best = null;
    this.history = [];
    this.trouble = null;
    this.stopping = false;
    const result = await getJob(id);
    if (this.jobId !== id) return;
    if (result.ok) this.#show(result.job);
    else if (result.status === 404) this.trouble = { kind: "not_found" };
    else this.trouble = { kind: "unreachable", message: result.error.message };
  }

  #show(job: JobStatus): void {
    if (this.jobId !== job.id) {
      this.#stopFollowing();
      // The previous job's plan must not stand in for this job's, which may have none.
      this.best = null;
      this.history = [];
    }
    this.jobId = job.id;
    this.job = job;
    this.trouble = null;
    this.elapsedAt = performance.now();
    if (job.best) this.#plan(job.best);
    if (!isFinal(job.state)) this.#follow(job.id);
  }

  #plan(report: PlanReport): void {
    this.best = report;
    if (!this.history.some((h) => h.number === report.number)) {
      this.history = [
        { number: report.number, foundAfter: report.found_after, panels: report.metrics.panels_used, waste: report.metrics.waste_fraction },
        ...this.history,
      ];
    }
  }

  #follow(id: string): void {
    this.#unfollow = followJob(id, {
      plan: (report) => {
        if (this.jobId === id) this.#plan(report);
      },
      state: (event) => {
        if (this.jobId !== id || !this.job) return;
        this.job = { ...this.job, ...event, best: this.best };
        this.elapsedAt = performance.now();
        if (isFinal(event.state)) {
          this.stopping = false;
          this.#stopFollowing();
        }
      },
      connection: (connected) => {
        this.connected = connected;
      },
    });
    // The queue position is not sent as an event: poll while queued.
    this.#poll = setInterval(async () => {
      if (this.jobId !== id || !this.job || this.job.state !== "queued") return;
      const result = await getJob(id);
      if (result.ok && this.jobId === id && this.job?.state === "queued") {
        this.job = { ...this.job, queue_position: result.job.queue_position };
      }
    }, 2000);
  }

  #stopFollowing(): void {
    this.#unfollow?.();
    this.#unfollow = null;
    if (this.#poll) clearInterval(this.#poll);
    this.#poll = null;
  }

  async stop(): Promise<void> {
    if (!this.jobId) return;
    this.stopping = true;
    const result = await cancelJob(this.jobId);
    if (result.ok && isFinal(result.job.state)) {
      this.stopping = false;
      this.#show(result.job);
    }
  }

  /**
   * Puts the shown job back into the editor ("Edit job", "Edit this plan's
   * job"). The draft it replaces can be brought back while Undo is offered.
   */
  edit(): void {
    // A draft that already asks for this plan is kept, with its own job name
    // and time limit.
    if (this.job && this.stale) {
      if (this.replaced) clearTimeout(this.replaced.timer);
      this.replaced = { draft: $state.snapshot(this.draft) as Draft, timer: window.setTimeout(() => (this.replaced = null), 8000) };
      this.draft = fromInstance($state.snapshot(this.job.instance), this.job.time_limit, this.job.name ?? "");
      this.problems = [];
      this.save();
    }
    if (!this.editorShown) router.go({ name: "setup" });
  }

  /** Brings back the draft that `edit` replaced. */
  undoEdit(): void {
    if (!this.replaced) return;
    clearTimeout(this.replaced.timer);
    this.draft = this.replaced.draft;
    this.replaced = null;
    this.save();
  }

  /**
   * Whether the job in the editor asks for another plan than the job shown
   * (DESIGN.md §2, "A plan for an older job").
   */
  get stale(): boolean {
    return this.job !== null && !draftMatches(this.draft, this.job.instance);
  }

  /** Whether the current screen shows the editor: Job setup, or the workbench. */
  get editorShown(): boolean {
    const route = router.route;
    return route.name === "setup" || (layout.wide && route.panel === null);
  }

  /** The name of the shown job, if it has one. */
  get jobName(): string {
    return this.job?.name ?? "";
  }
}

export const session = new Session();
