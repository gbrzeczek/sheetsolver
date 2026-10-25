# Web API

The HTTP contract between the solver (`crates/api`) and the web UI
(`web/`). The machine-readable version is `GET /openapi.json`, generated
from the Rust types; this page explains how the pieces fit together. When
the two disagree, the code is wrong or this page is: fix one of them in
the same change.

The instance and plan shapes are exactly those of
[ARCHITECTURE.md](ARCHITECTURE.md#data-contracts-draft); the help text of
every field is in [FIELDS.md](FIELDS.md) and in the `description` of every
property in `openapi.json`.

## Conventions

- JSON in and out, UTF-8. The request body is read as JSON whatever
  its `Content-Type`. Field names are `snake_case` and use the
  woodworking terms: panel, part, kerf, grain, offcut, leftover, plan.
- Lengths are millimetres with at most one decimal (`720`, `3.2`); more
  decimals are rejected, never rounded. Areas are mm². Durations are
  seconds (`time_limit` whole seconds, measured times with decimals,
  to the millisecond).
- Optional fields are left out when empty, except where this page says
  they are `null`.
- No authentication, no persistence: jobs live in the server's memory
  (see [Limits](#limits)).

## Endpoints

| Method and path | What it does | Success |
|---|---|---|
| `POST /jobs` | Submit an instance to solve | `201 Created`, `Location: /jobs/{id}`, body: job status |
| `GET /jobs/{id}` | The job's state and best plan so far | `200 OK`, body: job status |
| `GET /jobs/{id}/events` | Live stream of better plans and state changes (Server-Sent Events) | `200 OK`, `text/event-stream` |
| `DELETE /jobs/{id}` | Stop the search; the best plan so far is kept | `202 Accepted`, body: job status |
| `GET /openapi.json` | The OpenAPI 3.1 description of this API | `200 OK` |

Any other path answers `404 not_found`. In `openapi.json` the operations
are `submit_job`, `get_job`, `cancel_job`, `job_events` and `get_openapi`.

### `POST /jobs`

Request body:

```json
{
  "instance": {
    "objective": "bin_packing",
    "saw": { "kerf": 3.2, "stages": 3, "cut_type": "non_exact", "first_cuts": "either" },
    "panels": [ { "id": "Birch ply 18 mm", "length": 2500, "width": 1250, "quantity": 10 } ],
    "parts": [ { "id": "Side left", "length": 720, "width": 560, "quantity": 4, "grain": true } ]
  },
  "time_limit": 10
}
```

- `instance`: the instance, exactly as in `ARCHITECTURE.md`.
- `time_limit` (optional, default 10): how long the solver may search,
  in whole seconds, 1–600 (`FIELDS.md`, *Time limit*). The clock starts
  when the job starts running, not while it waits.

The response is the job status (below), usually in state `queued` or
`running`. The job then runs on its own: poll `GET /jobs/{id}` or follow
`GET /jobs/{id}/events`.

### Job status

`POST /jobs`, `GET /jobs/{id}` and `DELETE /jobs/{id}` return:

```json
{
  "id": "0f8b3c8e-5a43-4d4e-9d8b-3f7c2a1e6b90",
  "state": "running",
  "time_limit": 10,
  "elapsed": 1.284,
  "plans_found": 3,
  "best": {
    "number": 3,
    "found_after": 0.912,
    "metrics": {
      "panels_used": 2, "parts_cut": 24,
      "panel_area": 6250000, "part_area": 5313920,
      "leftover_area": 0, "waste_area": 936080, "waste_fraction": 0.1497728,
      "total_value": 5313920, "used_length": 2500, "used_width": 1250
    },
    "plan": { "patterns": [ "…as in ARCHITECTURE.md…" ] }
  },
  "message": null
}
```

| Field | Meaning |
|---|---|
| `id` | The job's id (a UUID), used in the other URLs. |
| `state` | See [Job states](#job-states). |
| `time_limit` | The time limit the job runs with, in seconds. |
| `elapsed` | Seconds spent searching: 0 while queued, growing while running, final once the job has ended. |
| `plans_found` | How many better plans have been found so far. |
| `best` | The best plan so far, or `null` if none yet (or none exists: see below). A plan always cuts at least one part. |
| `message` | Why the job failed, for state `failed`; otherwise `null`. |

`best` (also the payload of the `plan` event):

| Field | Meaning |
|---|---|
| `number` | 1 for the first plan found, 2 for the next better one, … Equal to `plans_found` when it was found. |
| `found_after` | Seconds after the job started running when this plan was found. |
| `metrics` | Figures for this plan, computed by the independent checker (`crates/checker`), so the UI need not compute them. See below. |
| `plan` | The cutting plan: the `Solution` JSON from `ARCHITECTURE.md` (`patterns`, each with a cutting tree). |

`metrics` (definitions as in `ARCHITECTURE.md`, *Metrics*; all over the
whole plan, counting repeated patterns):

| Field | Meaning |
|---|---|
| `panels_used` | Panels cut. |
| `parts_cut` | Parts cut. |
| `panel_area` | Total area of the panels used, mm². |
| `part_area` | Total area of the parts cut, mm². |
| `leftover_area` | Area of the reusable leftover of the last panel, mm². |
| `waste_area` | Offcuts plus material removed by the saw, mm². |
| `waste_fraction` | `waste_area` as a fraction (0–1) of the panel area without the leftover. |
| `total_value` | Total value of the parts cut (part area in mm² unless values were given). |
| `used_length` / `used_width` | How far the cuts reach on the last panel, mm (the result for the "Shortest length/width" goals). |

Every plan the API serves has passed the checker. Should the solver ever
produce a plan that breaks a cutting rule, that plan is not served: the
search is stopped, the job ends in state `failed` with the violations in
`message`, and `best` stays the last valid plan. A job also fails if the
solver stops unexpectedly (`message`: "the solver stopped
unexpectedly").

A job can end with `best: null`: no plan cuts every part (for example, a
part larger than every panel, or too few panels), or the job was stopped
before the first plan. With the goal "Best use of the panels I have",
any plan that cuts at least one part counts, so a plan is found unless
no part fits on any panel or the search was stopped first.

### Job states

```
queued ──► running ──► done
   │          │
   │          ├──────► cancelled
   │          └──────► failed
   └─────────────────► cancelled
```

| State | Meaning | Final |
|---|---|---|
| `queued` | Waiting for a free solver (see [Limits](#limits)). | no |
| `running` | Searching. `best` improves over time. | no |
| `done` | The search finished: it was complete, or the time limit was reached. `best` is the result. | yes |
| `cancelled` | Stopped by `DELETE /jobs/{id}`. `best` is the best plan found before the stop, if any. | yes |
| `failed` | Stopped by an internal error; `message` says why. | yes |

### `DELETE /jobs/{id}`

Asks the search to stop. A queued job is cancelled at once; a running job
stops at the solver's next check, normally within milliseconds, and its
state becomes `cancelled` then. The response is the job status at the
time of the request, so it may still say `running`: follow the events or
poll for the final state. Deleting a job that has already ended changes
nothing (`202` with its final status). The job itself stays readable
until it expires (see [Limits](#limits)).

### `GET /jobs/{id}/events`

A [Server-Sent Events](https://html.spec.whatwg.org/multipage/server-sent-events.html)
stream (`text/event-stream`), for the browser's `EventSource`. Two event
names:

| Event | `data` (JSON) | Sent when |
|---|---|---|
| `plan` | A plan report: the same object as `best` in the job status (`number`, `found_after`, `metrics`, `plan`; `PlanReport` in `openapi.json`). | A better plan is found. |
| `state` | `{ "state": "running", "elapsed": 0.003, "plans_found": 0, "message": null }`, fields as in the job status (`StateEvent` in `openapi.json`). | The state changes. |

Order:

1. On connecting, the client gets the current best plan as a `plan`
   event (if there is one), then the current state as a `state` event.
   A client that connects late, or reconnects, so starts from the
   current status without a separate `GET`; it does not see the states
   it missed (a job that is already running sends no `queued`).
2. Then one event per change. A `plan` event always comes before the
   `state` event of the state change that follows it.
3. After a `state` event with a final state (`done`, `cancelled`,
   `failed`) the server ends the stream.

The browser's `EventSource` reconnects automatically whenever a stream
ends, so **close it yourself on a final `state` event**
(`source.close()`); otherwise it reconnects every few seconds and replays
the final status.

The stream sends the latest plan, not necessarily every plan: if plans
arrive faster than the client reads them, intermediate ones are skipped
(`number` then jumps). The last plan is never skipped. A comment line is
sent every 15 seconds to keep idle connections open.

### `GET /openapi.json`

OpenAPI 3.1, generated from the Rust types with `utoipa`. Every property
has a `description`: for instance fields and `time_limit`, exactly the
help text from `FIELDS.md` (one paragraph, markdown: `**bold**` marks the
name of an option; a test compares them). The UI shows these next to its
inputs and generates its TypeScript client from this file. The event
payloads are the components `PlanReport` and `StateEvent`.

`cargo run -p api -- --print-openapi` prints the same document without
starting the server, for generating the client.

## Errors

Every error response has this body:

```json
{
  "error": "invalid_instance",
  "message": "The job cannot be solved as entered.",
  "problems": [
    { "field": "instance.parts[1].length", "message": "length 3.25 mm has more than one decimal; lengths are in steps of 0.1 mm" }
  ]
}
```

- `error`: a stable code for programs (below).
- `message`: one sentence for people.
- `problems`: for `invalid_instance`, every problem found; empty
  otherwise. Problems are found in two rounds: first every field that is
  missing or has the wrong type (a length with more than one decimal, a
  word where a number belongs, an unknown option), all at once; once
  every field reads, the checks of `Instance::validate` (no parts,
  duplicate names, quantity 0, …), again all at once. So a body with
  both kinds lists only the first kind until those are fixed. `field` is the path of the field at fault in the request
  body (`instance.saw.kerf`, `instance.parts[2].quantity`, `time_limit`),
  with 0-based list positions. A problem with a whole list points at the
  list ("add at least one part": `instance.parts`); `field` is `null`
  only when the body is not a JSON object. `message` is written
  for the woodworker and can be shown next to the field as is, e.g.
  "this field is required", "must be one of: non_exact, exact,
  homogenous", "quantity of part \"Side left\" must be at least 1".

| Status | `error` | When |
|---|---|---|
| 400 | `invalid_json` | The body is not JSON at all (syntax error, or empty). `message` says where. |
| 404 | `not_found` | No job with this id (or the id is not a UUID), it has expired, or no such path. |
| 413 | `too_large` | The body is larger than 2 MB. |
| 422 | `invalid_instance` | The body is JSON but not a valid job: a missing or mistyped field, a length with more than one decimal, a time limit outside 1–600, or anything `Instance::validate` rejects (no parts, duplicate names, quantity 0, …). |
| 503 | `busy` | Every solver is busy and `--max-queued-jobs` jobs are already waiting; try again later. |

A JSON field the server does not know is ignored, as is a missing
optional field.

## CORS

The UI is served separately in development (e.g. the Vite dev server on
`http://localhost:5173`), so the API answers cross-origin requests: by
default from any origin (`Access-Control-Allow-Origin: *`; there are no
cookies or credentials to protect). `--cors-origin` restricts it to the
given origins. Allowed methods: `GET`, `POST`, `DELETE`; allowed request
header: `Content-Type`; exposed response header: `Location`.

## Limits

Command-line options of `cargo run -p api -- …` (`--help` lists them):

| Option | Default | What it limits |
|---|---|---|
| `--listen` | `127.0.0.1:8080` | Address and port to listen on. |
| `--max-running-jobs` | number of CPU cores | Jobs solved at the same time, each on its own thread of a blocking pool of that size. Further jobs wait in state `queued`, first come first served. |
| `--max-queued-jobs` | 32 | Jobs that may wait; beyond that `POST /jobs` answers `503 busy`. |
| `--keep-jobs-for` | 3600 | Seconds a job stays readable after it ends; then `GET` answers `404`. A queued or running job never expires. |
| `--queue-size` | 2048 | The solver's beam width (not user-facing; larger finds better plans, slower). |
| `--cors-origin` | any | Allowed browser origin; repeat for several. |
| `--print-openapi` | | Print `openapi.json` and exit. |

Fixed limits: `time_limit` 1–600 seconds (default 10), request body
2 MB.

The solver runs upstream's deterministic search: the same instance and
time limit give the same plans, unless the time limit cuts the search
short on a slower or busier machine.

## Changes

Changes to this contract since it was first written (commit 8531aa3),
for reconciling the UI. None changes a field name or a JSON shape.

- 2026-10-25, Stage 6 implementation:
  - `best` (and every `plan` event) always cuts at least one part: an
    empty plan is never served. With "Best use of the panels I have",
    `best` stays `null` when no part fits on any panel.
  - `problems` are reported in two rounds (wrong types first, then
    `Instance::validate`), not all in one; see [Errors](#errors).
    `field` is `null` only when the body is not a JSON object.
  - `404 not_found` also for an id that is not a UUID and for unknown
    paths. The body is read as JSON whatever its `Content-Type`.
  - `503 busy` precisely: every solver busy and `--max-queued-jobs`
    jobs waiting.
  - Measured times (`elapsed`, `found_after`) are rounded to the
    millisecond.
  - A job also fails if the solver stops unexpectedly.
  - `openapi.json`: descriptions are single paragraphs, equal to the
    help text in `FIELDS.md`; operation ids `submit_job`, `get_job`,
    `cancel_job`, `job_events`, `get_openapi`; the event payloads are
    the components `PlanReport` and `StateEvent`. New option
    `--print-openapi`.
