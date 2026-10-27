// Following a job live: `GET /jobs/{id}/events` (Server-Sent Events).
import { isFinal, type PlanReport, type StateEvent } from "./types";

export interface JobEventHandlers {
  /** A better plan (or, on connecting, the current best plan). */
  plan(report: PlanReport): void;
  /** The job's state changed (or, on connecting, its current state). */
  state(event: StateEvent): void;
  /**
   * The connection dropped (`false`) or came back (`true`). `gaveUp`: the
   * browser will not reconnect by itself, as after a 404.
   */
  connection?(connected: boolean, gaveUp?: boolean): void;
}

/**
 * Follows a job until it reaches a final state. Returns a function that
 * stops following. The browser reconnects by itself after a drop, and the
 * server replays the current plan and state, so nothing is lost.
 */
export function followJob(id: string, on: JobEventHandlers): () => void {
  const source = new EventSource(`/jobs/${encodeURIComponent(id)}/events`);
  let closed = false;
  const close = () => {
    closed = true;
    source.close();
  };
  source.addEventListener("open", () => on.connection?.(true));
  source.addEventListener("error", () => {
    if (!closed) on.connection?.(false, source.readyState === EventSource.CLOSED);
  });
  source.addEventListener("plan", (event) => {
    on.plan(JSON.parse((event as MessageEvent<string>).data) as PlanReport);
  });
  source.addEventListener("state", (event) => {
    const state = JSON.parse((event as MessageEvent<string>).data) as StateEvent;
    on.state(state);
    // Otherwise EventSource reconnects forever and replays the final state.
    if (isFinal(state.state)) close();
  });
  return close;
}
