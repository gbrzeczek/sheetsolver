#!/usr/bin/env python3
"""Run the oracle on every instance and write or verify the golden files.

Usage:

    python3 -I tools/run_oracle.py ORACLE_BINARY [--algorithm A] [--queue-size N]
        [--check] [--filter TEXT]

Without --check, writes the golden files of one algorithm (default:
tree_search) to data/golden/<commit>/, for every instance in
data/instances/ the algorithm applies to:

- tree_search: every instance, as <instance>.q<N>.json (N default 2048);
- maximal_spaces: unlimited stages, one panel, no minimum offcut width, as
  <instance>.maximal_spaces.q<N>.json (N default 16, upstream's);
- hypergraph: unlimited stages, one panel, knapsack, no minimum offcut
  width, as
  <instance>.hypergraph.json;
- sequential_single_knapsack, sequential_value_correction: unlimited
  stages, several panels, as <instance>.<algorithm>.json;
- auto (upstream's own selection): unlimited stages, as
  <instance>.auto.json.

With --check, runs the oracle again into a temporary directory and fails if
any output differs from the golden file, which shows the oracle is
reproducible. --filter limits either to the instances whose file name
contains TEXT.
"""

import argparse
import filecmp
import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path

COMMIT = "9b656a0dcafd1e9019bf39a5910d4307af65579a"
ROOT = Path(__file__).resolve().parent.parent
INSTANCES = ROOT / "data" / "instances"
GOLDEN = ROOT / "data" / "golden" / COMMIT[:8]
TIMEOUT_SECONDS = 600


ALGORITHMS = ["tree_search", "maximal_spaces", "hypergraph",
              "sequential_single_knapsack", "sequential_value_correction", "auto"]
DEFAULT_QUEUE_SIZES = {"tree_search": 2048, "maximal_spaces": 16}


def number_of_panels(data):
    """Total panel count as upstream resolves it: open dimensions have one,
    no quantity means one per part."""
    if data["objective"].startswith("open_dimension"):
        return 1
    parts = sum(part["quantity"] for part in data["parts"])
    return sum(panel.get("quantity", parts) for panel in data["panels"])


def applies(algorithm, data):
    """Whether the algorithm gets golden files for this instance."""
    if algorithm == "tree_search":
        return True
    if data["saw"]["stages"] != "unlimited":
        return False
    single = number_of_panels(data) <= 1
    # Upstream's single-panel searches for unlimited stages ignore the
    # minimum offcut width, so it never selects them with one.
    minimum = data["saw"].get("minimum_offcut_width", 0) > 0
    if algorithm == "maximal_spaces":
        return single and not minimum
    if algorithm == "hypergraph":
        return single and data["objective"] == "knapsack" and not minimum
    if algorithm in ("sequential_single_knapsack", "sequential_value_correction"):
        return not single
    return algorithm == "auto"


def golden_name(stem, algorithm, queue_size):
    if algorithm == "tree_search":
        return f"{stem}.q{queue_size}.json"
    if algorithm == "maximal_spaces":
        return f"{stem}.{algorithm}.q{queue_size}.json"
    return f"{stem}.{algorithm}.json"


def run(oracle, instance, output, algorithm, queue_size):
    started = time.monotonic()
    command = [oracle, str(instance), str(output), "--algorithm", algorithm]
    if queue_size is not None:
        command += ["--queue-size", str(queue_size)]
    result = subprocess.run(command, capture_output=True, text=True, timeout=TIMEOUT_SECONDS)
    elapsed = time.monotonic() - started
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or f"exit code {result.returncode}")
    return elapsed


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("oracle")
    parser.add_argument("--algorithm", choices=ALGORITHMS, default="tree_search")
    parser.add_argument("--queue-size", type=int)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--filter", default="")
    args = parser.parse_args()

    queue_size = args.queue_size or DEFAULT_QUEUE_SIZES.get(args.algorithm)
    instances = sorted(p for p in INSTANCES.glob("*.json")
                       if p.name != "MANIFEST.json" and args.filter in p.name
                       and applies(args.algorithm, json.loads(p.read_text())))
    failures = 0
    total = 0.0
    with tempfile.TemporaryDirectory() as temp:
        out_dir = Path(temp) if args.check else GOLDEN
        out_dir.mkdir(parents=True, exist_ok=True)
        for instance in instances:
            name = golden_name(instance.stem, args.algorithm, queue_size)
            output = out_dir / name
            try:
                elapsed = run(args.oracle, instance, output, args.algorithm, queue_size)
            except (RuntimeError, subprocess.TimeoutExpired) as error:
                print(f"FAIL  {instance.name}: {error}")
                failures += 1
                continue
            total += elapsed
            if args.check:
                golden = GOLDEN / name
                if not golden.exists():
                    print(f"FAIL  {instance.name}: no golden file")
                    failures += 1
                elif not filecmp.cmp(golden, output, shallow=False):
                    print(f"DIFF  {instance.name}")
                    failures += 1
                    continue
            print(f"ok    {instance.name} ({elapsed:.2f} s)")
    print(f"{len(instances) - failures}/{len(instances)} ok, {total:.1f} s total")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
