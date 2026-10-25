#!/usr/bin/env python3
"""Measure what the two Stage 5 decisions (docs/SCOPE.md) cost, with upstream.

Usage:

    python3 -I tools/measure_decisions.py ORACLE_BINARY

1. Reduction: on every unlimited-stage instance with several panels, runs
   the sequential heuristic upstream selects with its subproblems reduced
   (upstream's behaviour, --reduce-subproblems) and not (the port's), and
   prints panels used or value cut, and time.
2. Sequential heuristics on 2 and 3 stages: on every 2- or 3-stage
   instance with several panels, runs the sequential heuristic upstream
   would add (value correction, or single knapsack with many parts per
   panel), and prints its result and time next to the tree search's (the
   golden file at queue size 2048, timed again here). Upstream's
   subproblems may use column generation (Out), which the port could not.

Times are one run of the oracle's --benchmark mode, in seconds.
"""

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from run_oracle import INSTANCES, number_of_panels  # noqa: E402


def sequential_algorithm(data):
    """upstream's choice between the two heuristics (optimize.cpp)."""
    lengths = [panel["length"] * 10 * panel["width"] * 10 for panel in data["panels"]]
    parts = data["parts"]
    count = sum(part["quantity"] for part in parts)
    mean_item_space = sum(part["quantity"] * part["length"] * 10 * part["width"] * 10
                          for part in parts) / count
    mean_items_in_bins = int(max(lengths) / mean_item_space)
    return ("sequential_single_knapsack" if mean_items_in_bins > 16
            else "sequential_value_correction")


def run(oracle, instance, output, algorithm, *extra):
    result = subprocess.run(
        [oracle, str(instance), str(output), "--algorithm", algorithm,
         "--benchmark", "1", *extra],
        capture_output=True, text=True, check=True)
    metrics = json.loads(output.read_text())["upstream_metrics"]
    return float(result.stdout.strip()), metrics


def figure(data, metrics):
    if not metrics["feasible"]:
        return "no plan"
    if data["objective"] == "knapsack":
        return f"value {metrics['profit']:.0f}"
    return f"{metrics['number_of_bins']} panels"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("oracle")
    args = parser.parse_args()

    instances = []
    for path in sorted(INSTANCES.glob("*.json")):
        if path.name == "MANIFEST.json":
            continue
        data = json.loads(path.read_text())
        if data["objective"] in ("bin_packing", "knapsack") and number_of_panels(data) > 1:
            instances.append((path, data))

    with tempfile.TemporaryDirectory() as temp:
        output = Path(temp) / "solution.json"
        print("1. Subproblem reduction (unlimited stages, several panels)")
        print(f"{'instance':<62} {'algorithm':<28} {'not reduced':>22} {'reduced':>22}")
        for path, data in instances:
            if data["saw"]["stages"] != "unlimited":
                continue
            algorithm = sequential_algorithm(data)
            plain = run(args.oracle, path, output, algorithm)
            reduced = run(args.oracle, path, output, algorithm, "--reduce-subproblems")
            print(f"{path.stem:<62} {algorithm:<28} "
                  f"{figure(data, plain[1]):>14} {plain[0]:6.2f} s "
                  f"{figure(data, reduced[1]):>14} {reduced[0]:6.2f} s")

        print()
        print("2. Sequential heuristics on 2 and 3 stages, several panels")
        print(f"{'instance':<62} {'algorithm':<28} {'tree search q2048':>22} {'heuristic':>22}")
        for path, data in instances:
            if data["saw"]["stages"] == "unlimited":
                continue
            algorithm = sequential_algorithm(data)
            tree = run(args.oracle, path, output, "tree_search", "--queue-size", "2048")
            heuristic = run(args.oracle, path, output, algorithm)
            print(f"{path.stem:<62} {algorithm:<28} "
                  f"{figure(data, tree[1]):>14} {tree[0]:6.2f} s "
                  f"{figure(data, heuristic[1]):>14} {heuristic[0]:6.2f} s")


if __name__ == "__main__":
    main()
