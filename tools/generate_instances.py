#!/usr/bin/env python3
"""Generate woodworking-like sheetsolver instances for differential tests.

Usage:

    python3 -I tools/generate_instances.py data/instances

Upstream's guillotine test instances leave gaps in the combinations we
support (no open-dimension-y, few bin-packing instances with exact or
homogenous cuts, few with a kerf). This script fills them: one instance for
every combination of objective, number of stages, cut type and kerf (0 and
3.2 mm), cycling through the first-cut directions, with cabinet-like parts on standard board sizes, some of them
with half-millimetre dimensions.

A second set (`generated__panels_*`) lists two or three panel sizes, for
every combination of objective (bin packing and knapsack), 2 and 3 stages,
first-cut direction and kerf, cycling through the cut types. It exercises
upstream's paths for several panel sizes: with first cuts "either", one
search in which each new panel picks its own direction, and for knapsack,
the panels taken smallest first whatever order they are listed in.

A third set (`generated__unlimited_*`) covers how upstream solves
unlimited stages: a single panel (the maximal-spaces search), several
panels (the sequential heuristics, with few or many parts per panel), and
many copies of few parts on small panels (the hypergraph search, which
upstream only picks for panels of at most 0.1 m²).

A fourth set (`generated__trims_*`) trims the panels' edges, each edge
cut off first or left on the offcut, with and without a minimum offcut
width: every objective with 2 and 3 stages and each cut type, several
panel sizes with their own trims, unlimited stages (solved on the trimmed
panel without a minimum offcut width, by the tree search with one), and
one job twice, every edge cut off first and every edge left on the
offcut, which give different plans under a minimum offcut width (the
Stage 9 exit criterion). Parts are never narrower than the minimum
offcut width: upstream throws on such plans (docs/SCOPE.md).

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
# Cycled over the instances, so every objective and stage count sees each.
FIRST_CUTS = ["either", "along_length", "across_length"]

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


def instance(rng, objective, stages, cut_type, kerf, first_cuts):
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
        "saw": {"kerf": kerf, "stages": stages, "cut_type": cut_type,
                "first_cuts": first_cuts},
        "panels": panels,
        "parts": items,
    }


# The multi-panel set: only the objectives that allow several panel sizes.
MULTI_PANEL_SEED = 20261026
MULTI_PANEL_OBJECTIVES = ["bin_packing", "knapsack"]
MULTI_PANEL_STAGES = [2, 3]


def multi_panel_instance(rng, objective, stages, cut_type, kerf, first_cuts):
    """Two or three distinct board sizes, listed in random order."""
    boards = rng.sample(BOARDS, rng.choice([2, 3]))
    items = parts(rng, rng.randint(6, 14))
    panels = [{"id": f"Panel {i + 1}", "length": length, "width": width}
              for i, (length, width) in enumerate(boards)]
    if objective == "bin_packing":
        # Panels are used in the order listed, so every size but the last
        # is limited, or the later ones would never be used.
        for panel in panels[:-1]:
            panel["quantity"] = rng.choice([1, 2])
    else:
        # One or two of each: not enough for every part.
        for panel in panels:
            panel["quantity"] = rng.choice([1, 2])
        for part in items:
            part["quantity"] *= 2
    return {
        "objective": objective,
        "saw": {"kerf": kerf, "stages": stages, "cut_type": cut_type,
                "first_cuts": first_cuts},
        "panels": panels,
        "parts": items,
    }


# The unlimited-stage set. Upstream ignores the cut type and the first-cut
# direction with unlimited stages, so they only vary for realism.
UNLIMITED_SEED = 20261027
# Small offcut-sized boards, at most 0.1 m² (upstream's hypergraph limit).
SMALL_BOARDS = [(400, 250), (316, 316), (300, 200)]


def unlimited_parts(rng, count, quantities, max_length=1200, max_width=700):
    """Like `parts`, with given quantities and an upper size."""
    result = []
    for i in range(count):
        length = rng.randrange(min(150, max_length // 2), max_length, 10) + rng.choice([0, 0, 0.5])
        width = rng.randrange(min(80, max_width // 2), min(int(length), max_width) + 1, 10) \
            + rng.choice([0, 0, 0.5])
        result.append({
            "id": f"Part {i + 1}",
            "length": length,
            "width": width,
            "quantity": rng.choice(quantities),
            **({"grain": True} if rng.random() < 0.4 else {}),
        })
    return result


def fill_to(rng, items, area, fraction):
    """Drops parts until their total area is at most `fraction` of `area`."""
    while sum(p["length"] * p["width"] * p["quantity"] for p in items) > fraction * area:
        part = rng.choice(items)
        if part["quantity"] > 1:
            part["quantity"] -= 1
        elif len(items) > 1:
            items.remove(part)
    for i, part in enumerate(items):
        part["id"] = f"Part {i + 1}"
    return items


def unlimited_instances(rng):
    """Yields (name, instance) for the unlimited-stage set."""
    def saw(kerf, first_cuts):
        return {"kerf": kerf, "stages": "unlimited",
                "cut_type": rng.choice(CUT_TYPES), "first_cuts": first_cuts}

    for index, (kerf, first_cuts) in enumerate(
            [(k, f) for k in KERFS for f in FIRST_CUTS]):
        kerf_name = str(kerf).replace(".", "_")
        suffix = f"{first_cuts}_kerf{kerf_name}"
        length, width = rng.choice(BOARDS)

        # One panel, all parts fit: bin packing on a single panel.
        items = fill_to(rng, unlimited_parts(rng, rng.randint(5, 10), [1, 1, 2, 3]),
                        length * width, 0.75)
        yield f"single_bin_packing_{suffix}", {
            "objective": "bin_packing", "saw": saw(kerf, first_cuts),
            "panels": [{"id": "Panel 1", "length": length, "width": width, "quantity": 1}],
            "parts": items}

        # One panel, too many parts: knapsack, some with values.
        items = unlimited_parts(rng, rng.randint(6, 12), [1, 2, 3, 4])
        for part in items:
            if rng.random() < 0.3:
                part["value"] = round(part["length"] * part["width"] * rng.uniform(0.5, 2.0))
        yield f"single_knapsack_{suffix}", {
            "objective": "knapsack", "saw": saw(kerf, first_cuts),
            "panels": [{"id": "Panel 1", "length": length, "width": width, "quantity": 1}],
            "parts": items}

        # Several panels, few parts per panel: sequential value correction.
        items = unlimited_parts(rng, rng.randint(5, 9), [2, 3, 4, 6])
        yield f"panels_bin_packing_{suffix}", {
            "objective": "bin_packing", "saw": saw(kerf, first_cuts),
            "panels": [{"id": "Panel 1", "length": length, "width": width}],
            "parts": items}

        # Several panels, many small parts per panel: sequential single
        # knapsack (more than 16 parts per panel on average).
        items = unlimited_parts(rng, rng.randint(4, 7), [8, 12, 20],
                                max_length=400, max_width=300)
        yield f"panels_small_parts_{suffix}", {
            "objective": "bin_packing", "saw": saw(kerf, first_cuts),
            "panels": [{"id": "Panel 1", "length": length, "width": width}],
            "parts": items}

        # Two panel sizes, knapsack: panels used smallest first.
        boards = rng.sample(BOARDS, 2)
        items = unlimited_parts(rng, rng.randint(6, 10), [2, 4, 6])
        yield f"panels_knapsack_{suffix}", {
            "objective": "knapsack", "saw": saw(kerf, first_cuts),
            "panels": [{"id": f"Panel {i + 1}", "length": l, "width": w,
                        "quantity": rng.choice([1, 2])}
                       for i, (l, w) in enumerate(boards)],
            "parts": items}

        # Many copies of few parts on one small panel: the hypergraph
        # search (knapsack only).
        small_length, small_width = SMALL_BOARDS[index % len(SMALL_BOARDS)]
        items = unlimited_parts(rng, rng.randint(2, 4), [40, 60, 100],
                                max_length=small_length // 2, max_width=small_width // 2)
        yield f"copies_knapsack_{suffix}", {
            "objective": "knapsack", "saw": saw(kerf, first_cuts),
            "panels": [{"id": "Offcut", "length": small_length, "width": small_width,
                        "quantity": 1}],
            "parts": items}

        # Many copies, several small panels: the sequential heuristics with
        # hypergraph subproblems.
        items = unlimited_parts(rng, rng.randint(2, 3), [30, 50],
                                max_length=small_length // 2, max_width=small_width // 2)
        yield f"copies_panels_{suffix}", {
            "objective": rng.choice(["bin_packing", "knapsack"]), "saw": saw(kerf, first_cuts),
            "panels": [{"id": "Offcut", "length": small_length, "width": small_width,
                        "quantity": rng.choice([3, 5])}],
            "parts": items}


# The trims set.
TRIMS_SEED = 20261028
TRIM_CUTS = ["cut_off_first", "left_on_offcut"]
MINIMUM_OFFCUT_WIDTHS = [0, 20, 0, 40]


def random_trims(rng, all_edges=False):
    """Trims on some or all edges of a panel, 3 to 25 mm, each edge cut off
    first or left on the offcut (sometimes left to its default)."""
    trims = {}
    for edge in ("left", "right", "bottom", "top"):
        if all_edges or rng.random() < 0.7:
            trim = {"width": rng.choice([3, 5, 8, 10, 12.5, 15, 25])}
            if rng.random() < 0.75:
                trim["cut"] = rng.choice(TRIM_CUTS)
            trims[edge] = trim
    return trims or {"left": {"width": 10}}


def trims_instances(rng):
    """Yields (name, instance) for the trims set."""
    count = 0
    for objective in OBJECTIVES:
        for stages in [2, 3]:
            for cut_type in CUT_TYPES:
                minimum = MINIMUM_OFFCUT_WIDTHS[count % len(MINIMUM_OFFCUT_WIDTHS)]
                kerf = KERFS[(count // 2) % len(KERFS)]
                first_cuts = FIRST_CUTS[count % len(FIRST_CUTS)]
                data = instance(rng, objective, stages, cut_type, kerf, first_cuts)
                for panel in data["panels"]:
                    panel["trims"] = random_trims(rng)
                data["saw"]["minimum_offcut_width"] = minimum
                kerf_name = str(kerf).replace(".", "_")
                yield (f"{objective}_{stages}_{cut_type}_kerf{kerf_name}_min{minimum}", data)
                count += 1

    for objective in MULTI_PANEL_OBJECTIVES:
        for stages in [2, 3]:
            for first_cuts in FIRST_CUTS:
                minimum = MINIMUM_OFFCUT_WIDTHS[count % len(MINIMUM_OFFCUT_WIDTHS)]
                kerf = KERFS[count % len(KERFS)]
                cut_type = CUT_TYPES[count % len(CUT_TYPES)]
                data = multi_panel_instance(rng, objective, stages, cut_type, kerf, first_cuts)
                for panel in data["panels"]:
                    panel["trims"] = random_trims(rng)
                data["saw"]["minimum_offcut_width"] = minimum
                yield (f"panels_{objective}_{stages}_{first_cuts}_min{minimum}", data)
                count += 1

    # Unlimited stages: one panel (maximal spaces, or the tree search with
    # a minimum offcut width), several panels (the sequential heuristics),
    # and many copies on a small panel (the hypergraph search).
    for minimum in [0, 20]:
        for kerf in KERFS:
            kerf_name = str(kerf).replace(".", "_")
            suffix = f"kerf{kerf_name}_min{minimum}"
            saw = {"kerf": kerf, "stages": "unlimited", "cut_type": "non_exact",
                   "first_cuts": "either", "minimum_offcut_width": minimum}
            length, width = rng.choice(BOARDS)
            items = fill_to(rng, unlimited_parts(rng, rng.randint(5, 9), [1, 1, 2, 3]),
                            length * width, 0.7)
            yield f"unlimited_single_bin_packing_{suffix}", {
                "objective": "bin_packing", "saw": dict(saw),
                "panels": [{"id": "Panel 1", "length": length, "width": width,
                            "quantity": 1, "trims": random_trims(rng)}],
                "parts": items}
            items = unlimited_parts(rng, rng.randint(6, 10), [1, 2, 3, 4])
            yield f"unlimited_single_knapsack_{suffix}", {
                "objective": "knapsack", "saw": dict(saw),
                "panels": [{"id": "Panel 1", "length": length, "width": width,
                            "quantity": 1, "trims": random_trims(rng)}],
                "parts": items}
            items = unlimited_parts(rng, rng.randint(5, 8), [2, 3, 4])
            yield f"unlimited_panels_bin_packing_{suffix}", {
                "objective": "bin_packing", "saw": dict(saw),
                "panels": [{"id": "Panel 1", "length": length, "width": width,
                            "trims": random_trims(rng)}],
                "parts": items}
            small_length, small_width = rng.choice(SMALL_BOARDS)
            items = unlimited_parts(rng, rng.randint(2, 3), [40, 60],
                                    max_length=small_length // 2, max_width=small_width // 2)
            yield f"unlimited_copies_knapsack_{suffix}", {
                "objective": "knapsack", "saw": dict(saw),
                "panels": [{"id": "Offcut", "length": small_length, "width": small_width,
                            "quantity": 1, "trims": random_trims(rng)}],
                "parts": items}

    # The same job with every edge cut off first, then left on the offcut.
    items = parts(rng, 10)
    for cut in TRIM_CUTS:
        yield f"every_edge_{cut}", {
            "objective": "bin_packing",
            "saw": {"kerf": 3.2, "stages": 3, "cut_type": "non_exact",
                    "first_cuts": "across_length", "minimum_offcut_width": 30},
            "panels": [{"id": "Panel 1", "length": 2500, "width": 1250, "trims": {
                edge: {"width": 10, "cut": cut} for edge in ("left", "right", "bottom", "top")}}],
            "parts": items}


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
                    first_cuts = FIRST_CUTS[count % len(FIRST_CUTS)]
                    data = instance(rng, objective, stages, cut_type, kerf, first_cuts)
                    (out / name).write_text(json.dumps(data, indent=2) + "\n")
                    count += 1
    # A separate generator, so adding this set left the files above unchanged.
    rng = random.Random(MULTI_PANEL_SEED)
    multi_count = 0
    for objective in MULTI_PANEL_OBJECTIVES:
        for stages in MULTI_PANEL_STAGES:
            for first_cuts in FIRST_CUTS:
                for kerf in KERFS:
                    # Shifted by one for each objective and stage count, so
                    # every first-cut direction sees every cut type.
                    shift = multi_count // (len(FIRST_CUTS) * len(KERFS))
                    cut_type = CUT_TYPES[(multi_count + shift) % len(CUT_TYPES)]
                    kerf_name = str(kerf).replace(".", "_")
                    name = (f"generated__panels_{objective}_{stages}_{first_cuts}"
                            f"_{cut_type}_kerf{kerf_name}.json")
                    data = multi_panel_instance(rng, objective, stages, cut_type, kerf, first_cuts)
                    (out / name).write_text(json.dumps(data, indent=2) + "\n")
                    multi_count += 1
    rng = random.Random(UNLIMITED_SEED)
    unlimited_count = 0
    for name, data in unlimited_instances(rng):
        (out / f"generated__unlimited_{name}.json").write_text(json.dumps(data, indent=2) + "\n")
        unlimited_count += 1
    rng = random.Random(TRIMS_SEED)
    trims_count = 0
    for name, data in trims_instances(rng):
        (out / f"generated__trims_{name}.json").write_text(json.dumps(data, indent=2) + "\n")
        trims_count += 1
    print(f"generated {count} + {multi_count} with several panel sizes"
          f" + {unlimited_count} with unlimited stages + {trims_count} with trims")


if __name__ == "__main__":
    main()
