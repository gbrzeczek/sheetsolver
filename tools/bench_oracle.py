#!/usr/bin/env python3
"""Time upstream's tree search on the benchmark instances.

Usage:

    python3 -I tools/bench_oracle.py ORACLE_BINARY [--repeats R]

Runs the oracle with --benchmark on every instance listed in
crates/guillotine/benches/instances.txt, at queue sizes 64 and 2048, and
prints the fastest of R runs of upstream's tree_search() for each, in
milliseconds. Compare with `cargo bench -p guillotine`, which runs
guillotine::solve on the same instances. Each run's plan is checked
against the golden file, so the times are for the same work.
"""

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

COMMIT = "9b656a0dcafd1e9019bf39a5910d4307af65579a"
ROOT = Path(__file__).resolve().parent.parent
INSTANCES = ROOT / "data" / "instances"
GOLDEN = ROOT / "data" / "golden" / COMMIT[:8]
LIST = ROOT / "crates" / "guillotine" / "benches" / "instances.txt"
QUEUE_SIZES = (64, 2048)


def bench_instances():
    lines = (line.strip() for line in LIST.read_text().splitlines())
    return [line for line in lines if line and not line.startswith("#")]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("oracle")
    parser.add_argument("--repeats", type=int, default=10)
    args = parser.parse_args()

    failures = 0
    with tempfile.TemporaryDirectory() as temp:
        output = Path(temp) / "solution.json"
        for queue_size in QUEUE_SIZES:
            for stem in bench_instances():
                result = subprocess.run(
                    [args.oracle, str(INSTANCES / f"{stem}.json"), str(output),
                     "--queue-size", str(queue_size), "--benchmark", str(args.repeats)],
                    capture_output=True, text=True, check=True)
                seconds = float(result.stdout.strip())
                golden = json.loads((GOLDEN / f"{stem}.q{queue_size}.json").read_text())
                same = json.loads(output.read_text())["patterns"] == golden["patterns"]
                if not same:
                    failures += 1
                note = "" if same else "  (differs from the golden file)"
                print(f"solve_q{queue_size}/{stem}: {seconds * 1000:.2f} ms{note}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
