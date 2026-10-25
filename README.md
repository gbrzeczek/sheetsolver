# sheetsolver

Cutting plans for woodworkers: enter the parts you need and the panels
you have, and get a plan of guillotine cuts that wastes as little board
as possible.

Under the hood: a Rust reimplementation of the 2D guillotine solver from
[fontanf/packingsolver](https://github.com/fontanf/packingsolver),
as a library, a web API and a web UI.

Status: the solver library and a developer CLI work for 2 and 3 cutting
stages (Stages 0–4 of [docs/STAGES.md](docs/STAGES.md)). On every test
instance it gives exactly upstream's cutting plan for the same settings,
and it runs in 0.61–0.69× upstream's time on the benchmark instances
([details](docs/STAGES.md)). Unlimited stages (Stage 5) are ported and
match upstream; their timing is still to be measured. The web API
(Stage 6, [docs/API.md](docs/API.md)) is done. Next: the web UI.

## Web API

```bash
# Serve the API on http://127.0.0.1:8080 (--help lists the limits).
cargo run --release -p api

# Or print its OpenAPI description, e.g. to generate a client.
cargo run -p api -- --print-openapi > openapi.json
```

Submit a job with `POST /jobs`, follow it with `GET /jobs/{id}/events`,
stop it with `DELETE /jobs/{id}`; see [docs/API.md](docs/API.md).

## Docker image

One image holds the API and the web UI it serves (see
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#deployment)). Until the UI
in `web/` builds, the image serves the API alone.

```bash
# Build and run it locally; then open http://localhost:8080/openapi.json.
docker build -t sheetsolver .
docker run --rm -p 8080:8080 sheetsolver

# Settings are SHEETSOLVER_* variables named after the api options:
docker run --rm -p 9000:9000 -e SHEETSOLVER_LISTEN=0.0.0.0:9000 \
  -e SHEETSOLVER_MAX_RUNNING_JOBS=2 sheetsolver
```

Pushes to `main` and `v*` tags build the image and publish it as
`ghcr.io/<owner>/sheetsolver` (`.github/workflows/docker.yml`). On the
server, `deploy/docker-compose.yml` runs it on the external `web`
Docker network, behind your reverse proxy (`http://sheetsolver:8080`, no
host port), deployed over SSH through Tailscale after each build once
configured: see [docs/DEPLOY.md](docs/DEPLOY.md).

## Developer CLI

Run from the repository; `--release` makes the larger searches fast.
Add `--help` to any subcommand for all its options.

```bash
# Solve an instance: the plan as JSON on stdout (or -o FILE), a summary on stderr.
cargo run --release -p cli -- solve data/instances/tests__bin_packing_3nvo.json --time-limit 5 -o plan.json

# Check a plan with the independent checker: metrics and every violation.
cargo run --release -p cli -- check data/instances/tests__bin_packing_3nvo.json plan.json

# Compare the solver with upstream's golden plans: the full differential suite.
cargo run --release -p cli -- diff-oracle -q

# Time the solver on the benchmark instances.
cargo run --release -p cli -- bench-set
```

`diff-oracle` prints one line per golden file (`-q`: failures only)
and a summary, and exits with status 1 if any plan differs from
upstream's or breaks a rule:

```
278 golden files in 5.5 s: 216 same, 0 differ, 0 infeasible, 0 errors, 62 skipped
PASS
```

The 62 skipped files are the unlimited-stage instances, which the solver
does not handle yet. `check` also exits with status 1 for a plan that
breaks a rule, so both work in scripts.

Start with [AGENTS.md](AGENTS.md) and [docs/AIMS.md](docs/AIMS.md).
