#!/usr/bin/env python3
"""Generate toy knapsack instances and their reference search outputs.

Usage:

    python3 -I tools/treesearch_toy.py TOY_BINARY [--check]

Writes seeded instances to data/treesearch/instances/ and runs upstream's
iterative_beam_search_2 on each with several parameter sets, writing
data/treesearch/golden/<commit>/<instance>.<config>.json. With --check,
verifies the outputs are reproduced exactly instead of writing them.
The Rust tests in crates/treesearch/tests/ compare against these files.
"""

import argparse
import filecmp
import json
import random
import subprocess
import sys
import tempfile
from pathlib import Path

COMMIT = "9b656a0dcafd1e9019bf39a5910d4307af65579a"
ROOT = Path(__file__).resolve().parent.parent
INSTANCES = ROOT / "data" / "treesearch" / "instances"
GOLDEN = ROOT / "data" / "treesearch" / "golden" / COMMIT[:8]
SEED = 20261025

# name -> (number of items, weight range, capacity as a fraction of total weight)
SIZES = {
    "tiny": (6, (1, 9), 0.5),
    "small": (12, (1, 20), 0.4),
    "ties": (14, (2, 4), 0.5),  # few distinct weights: many equal priorities
    "medium": (20, (5, 60), 0.35),
    "large": (28, (10, 99), 0.3),
}

# name -> command-line arguments of the toy binary
CONFIGS = {
    # What packingsolver uses in deterministic mode: queue sizes 1, then N.
    "fixed64": ["--minimum-queue-size", "1", "--growth-factor", "64", "--maximum-queue-size", "65"],
    "fixed1024": ["--minimum-queue-size", "1", "--growth-factor", "1024", "--maximum-queue-size", "1025"],
    # What packingsolver uses in anytime mode, capped.
    "grow1.5": ["--growth-factor", "1.5", "--maximum-queue-size", "300"],
    "grow2": ["--growth-factor", "2"],
    # Stopped by the node limit in the middle of an iteration.
    "nodelimit": ["--growth-factor", "1.5", "--maximum-queue-size", "1000",
                  "--maximum-nodes-expanded", "150"],
}


def generate():
    INSTANCES.mkdir(parents=True, exist_ok=True)
    rng = random.Random(SEED)
    for name, (count, (low, high), fraction) in SIZES.items():
        items = []
        for _ in range(count):
            weight = rng.randint(low, high)
            items.append({"weight": weight, "profit": max(1, weight + rng.randint(-low, high))})
        capacity = int(sum(item["weight"] for item in items) * fraction)
        data = {"capacity": capacity, "items": items}
        (INSTANCES / f"{name}.json").write_text(json.dumps(data, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if not args.check:
        generate()
    failures = 0
    with tempfile.TemporaryDirectory() as temp:
        out_dir = Path(temp) if args.check else GOLDEN
        out_dir.mkdir(parents=True, exist_ok=True)
        for instance in sorted(INSTANCES.glob("*.json")):
            for config, config_args in CONFIGS.items():
                name = f"{instance.stem}.{config}.json"
                subprocess.run([args.binary, str(instance), str(out_dir / name), *config_args],
                               check=True)
                if args.check and not filecmp.cmp(GOLDEN / name, out_dir / name, shallow=False):
                    print(f"DIFF  {name}")
                    failures += 1
    count = len(list(INSTANCES.glob("*.json"))) * len(CONFIGS)
    print(f"{count - failures}/{count} ok")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
