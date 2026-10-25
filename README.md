# sheetsolver

Cutting plans for woodworkers: enter the parts you need and the panels
you have, and get a plan of guillotine cuts that wastes as little board
as possible.

Under the hood: a Rust reimplementation of the 2D guillotine solver from
[fontanf/packingsolver](https://github.com/fontanf/packingsolver),
as a library, a web API and a web UI.

Status: the solver library works for 2 and 3 cutting stages (Stages 0–3
of [docs/STAGES.md](docs/STAGES.md)). On every test instance it gives
exactly upstream's cutting plan for the same settings, and it runs in
0.61–0.69× upstream's time on the benchmark instances
([details](docs/STAGES.md)). Next: a developer CLI, then unlimited
stages, the web API and the web UI.

Start with [AGENTS.md](AGENTS.md) and [docs/AIMS.md](docs/AIMS.md).
