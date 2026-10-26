// Calls to the web API (docs/API.md). The UI is served by the API itself,
// so every URL is relative; the dev server proxies them (vite.config.ts).
import createClient from "openapi-fetch";
import type { paths } from "./schema";
import type { ErrorBody, JobRequest, JobStatus } from "./types";

const client = createClient<paths>({ baseUrl: "" });

/** The outcome of a call: the job status, or why there is none. */
export type JobResult =
  | { ok: true; job: JobStatus }
  | { ok: false; status: number; error: ErrorBody; retryAfter: number | null };

/** An answer that was not JSON at all (a proxy or network problem). */
function unreadable(status: number): ErrorBody {
  return {
    error: "invalid_json",
    message:
      status === 0
        ? "The server cannot be reached. Check the connection and try again."
        : `The server answered with an unexpected error (HTTP ${status}).`,
    problems: [],
  } as ErrorBody;
}

async function settle(
  call: Promise<{ data?: JobStatus; error?: unknown; response: Response }>,
): Promise<JobResult> {
  try {
    const { data, error, response } = await call;
    if (data) return { ok: true, job: data };
    const body = error && typeof error === "object" && "error" in error ? (error as ErrorBody) : unreadable(response.status);
    const retry = Number(response.headers.get("Retry-After"));
    return {
      ok: false,
      status: response.status,
      error: body,
      retryAfter: Number.isFinite(retry) && retry > 0 ? retry : null,
    };
  } catch {
    return { ok: false, status: 0, error: unreadable(0), retryAfter: null };
  }
}

/** `POST /jobs`: submit a job. */
export function submitJob(request: JobRequest): Promise<JobResult> {
  return settle(client.POST("/jobs", { body: request }));
}

/** `GET /jobs/{id}`: the job's status and best plan so far. */
export function getJob(id: string): Promise<JobResult> {
  return settle(client.GET("/jobs/{id}", { params: { path: { id } } }));
}

/** `DELETE /jobs/{id}`: stop the search, keeping the best plan so far. */
export function cancelJob(id: string): Promise<JobResult> {
  return settle(client.DELETE("/jobs/{id}", { params: { path: { id } } }));
}
