#!/usr/bin/env python3
"""Generate woodworking-like sheetsolver instances for differential tests.

Usage:

    python3 -I tools/generate_instances.py data/instances

Upstream's guillotine test instances leave gaps in the combinations we
support (no open-dimension-y, few bin-packing instances with exact or
homogenous cuts, few with a kerf). This script fills them: one instance for
every combination of objective, number of stages, cut type and kerf (0 and
3.2 mm), with cabinet-like parts on standard board sizes, some of them
with half-millimetre dimensions.

Output is deterministic (fixed seed), so regenerating never changes
existing files unless this script changes.
"""

import json
import random
import sys
from pathlib import Path

SEED = 20261025

OBJECTIVES = ["bin_packing", "knapsack", "open_dimension_x", "open_dimension_y"]
STAGES = [2, 3, "unlimited"]
CUT_TYPES = ["non_exact", "exact", "homogenous"]
KERFS = [0, 3.2]

# Common board sizes in mm (length x width).
BOARDS = [(2800, 2070), (2500, 1250), (2440, 1220), (3050, 1830)]


def parts(rng, count):
    """Cabinet-like parts: a few repeated sizes, some with grain."""
    result = []
    for i in range(count):
        # Mostly whole millimetres, sometimes half a millimetre, to
        # exercise the 0.1 mm precision.
        length = rng.randrange(150, 1200, 10) + rng.choice([0, 0, 0, 0.5])
        width = rng.randrange(80, min(int(length), 700) + 1, 10) + rng.choice([0, 0, 0, 0.5])
        result.append({
            "id": f"Part {i + 1}",
            "length": length,
            "width": width,
            "quantity": rng.choice([1, 1, 2, 2, 4, 6]),
            **({"grain": True} if rng.random() < 0.4 else {}),
        })
    return result


def instance(rng, objective, stages, cut_type, kerf):
    length, width = rng.choice(BOARDS)
    items = parts(rng, rng.randint(6, 14))
    if objective == "bin_packing":
        panels = [{"id": "Panel 1", "length": length, "width": width}]
    elif objective == "knapsack":
        # One or two panels: not enough for every part.
        panels = [{"id": "Panel 1", "length": length, "width": width,
                   "quantity": rng.choice([1, 2])}]
        for part in items:
            part["quantity"] *= 2
    else:
        # One panel with a fixed side and a long open side.
        long_side = 20000
        panels = [{"id": "Panel 1",
                   "length": long_side if objective == "open_dimension_x" else length,
                   "width": width if objective == "open_dimension_x" else long_side,
                   "quantity": 1}]
    return {
        "objective": objective,
        "saw": {"kerf": kerf, "stages": stages, "cut_type": cut_type},
        "panels": panels,
        "parts": items,
    }


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    rng = random.Random(SEED)
    count = 0
    for objective in OBJECTIVES:
        for stages in STAGES:
            for cut_type in CUT_TYPES:
                for kerf in KERFS:
                    kerf_name = str(kerf).replace(".", "_")
                    name = f"generated__{objective}_{stages}_{cut_type}_kerf{kerf_name}.json"
                    data = instance(rng, objective, stages, cut_type, kerf)
                    (out / name).write_text(json.dumps(data, indent=2) + "\n")
                    count += 1
    print(f"generated {count}")


if __name__ == "__main__":
    main()
