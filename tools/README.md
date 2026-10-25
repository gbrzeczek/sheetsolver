# Tools

Everything here supports differential testing against upstream
packingsolver (see `docs/TESTING.md`). None of it is needed to build or
test the Rust crates: the golden files it produces are committed.

| Tool | What it does |
|---|---|
| `oracle/` | C++ program: upstream's guillotine tree search at the pinned commit, reading and writing our JSON formats. |
| `convert_instances.py` | Converts upstream's `data/rectangleguillotine` instances that only use in-scope features into `data/instances/`, and lists what it skipped and why in `data/instances/MANIFEST.json`. |
| `generate_instances.py` | Writes seeded, woodworking-like instances covering every combination of objective, stages, cut type and kerf (`data/instances/generated__*.json`). |
| `oracle/treesearch_toy.cpp`, `treesearch_toy.py` | Upstream's beam search on a toy knapsack scheme, and the script that writes its seeded instances and reference outputs to `data/treesearch/` for the `treesearch` crate's tests. |
| `run_oracle.py` | Runs the oracle on every instance and writes the golden files to `data/golden/<commit>/`, or with `--check`, verifies the oracle reproduces them. |

## Regenerating the golden files

Needs CMake ≥ 3.28 and a C++17 compiler. The first build downloads upstream
and its dependencies (including HiGHS) and takes several minutes.

```bash
git clone https://github.com/fontanf/packingsolver /tmp/packingsolver
git -C /tmp/packingsolver checkout 9b656a0dcafd1e9019bf39a5910d4307af65579a
python3 -I tools/convert_instances.py /tmp/packingsolver data/instances
python3 -I tools/generate_instances.py data/instances
cmake -S tools/oracle -B tools/oracle/build -DCMAKE_BUILD_TYPE=Release
cmake --build tools/oracle/build -j --target sheetsolver_oracle
python3 -I tools/run_oracle.py tools/oracle/build/sheetsolver_oracle
python3 -I tools/run_oracle.py tools/oracle/build/sheetsolver_oracle --check
cmake --build tools/oracle/build -j --target sheetsolver_treesearch_toy
python3 -I tools/treesearch_toy.py tools/oracle/build/sheetsolver_treesearch_toy
python3 -I tools/treesearch_toy.py tools/oracle/build/sheetsolver_treesearch_toy --check
```

When bumping the upstream commit, update it in `docs/UPSTREAM.md`,
`tools/oracle/CMakeLists.txt`, `tools/convert_instances.py`,
`tools/run_oracle.py` and `tools/treesearch_toy.py`.
