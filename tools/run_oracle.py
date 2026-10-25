#!/usr/bin/env python3
"""Run the oracle on every instance and write or verify the golden files.

Usage:

    python3 -I tools/run_oracle.py ORACLE_BINARY [--queue-size N] [--check] [--filter TEXT]

Without --check, writes data/golden/<commit>/<instance>.q<N>.json for every
instance in data/instances/. With --check, runs the oracle again into a
temporary directory and fails if any output differs from the golden file,
which shows the oracle is reproducible. --filter limits either to the
instances whose file name contains TEXT.
"""

import argparse
import filecmp
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


def run(oracle, instance, output, queue_size):
    started = time.monotonic()
    result = subprocess.run(
        [oracle, str(instance), str(output), "--queue-size", str(queue_size)],
        capture_output=True, text=True, timeout=TIMEOUT_SECONDS)
    elapsed = time.monotonic() - started
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or f"exit code {result.returncode}")
    return elapsed


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("oracle")
    parser.add_argument("--queue-size", type=int, default=2048)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--filter", default="")
    args = parser.parse_args()

    instances = sorted(p for p in INSTANCES.glob("*.json")
                       if p.name != "MANIFEST.json" and args.filter in p.name)
    failures = 0
    total = 0.0
    with tempfile.TemporaryDirectory() as temp:
        out_dir = Path(temp) if args.check else GOLDEN
        out_dir.mkdir(parents=True, exist_ok=True)
        for instance in instances:
            name = f"{instance.stem}.q{args.queue_size}.json"
            output = out_dir / name
            try:
                elapsed = run(args.oracle, instance, output, args.queue_size)
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
