#!/usr/bin/env python3
"""Convert upstream packingsolver guillotine instances to sheetsolver JSON.

Usage:

    python3 -I tools/convert_instances.py UPSTREAM_CHECKOUT data/instances

Reads every instance under UPSTREAM_CHECKOUT/data/rectangleguillotine (CSV
directories and instance.json files), keeps those that only use features
marked In in docs/SCOPE.md, and writes one JSON file per kept instance plus
MANIFEST.json, which lists the source of every converted instance and the
reason every other instance was skipped.

Field mapping (docs/FIELDS.md): upstream width -> length (x), upstream
height -> width (y), bin -> panel, item -> part, copies -> quantity,
oriented -> grain, profit -> value, cut_thickness -> kerf,
first_stage_orientation -> first_cuts.
"""

import csv
import json
import sys
from pathlib import Path

IN_SCOPE_OBJECTIVES = {
    "bin_packing": "bin_packing",
    "binpacking": "bin_packing",
    "knapsack": "knapsack",
    "open_dimension_x": "open_dimension_x",
    "opendimensionx": "open_dimension_x",
    "open_dimension_y": "open_dimension_y",
    "opendimensiony": "open_dimension_y",
}

CUT_TYPES = {
    "non-exact": "non_exact",
    "nonexact": "non_exact",
    "exact": "exact",
    "homogenous": "homogenous",
}

# Upstream first_stage_orientation -> our first_cuts. Upstream "vertical"
# cuts are at a fixed x: they run across the panel's length.
FIRST_CUTS = {
    "vertical": "across_length",
    "horizontal": "along_length",
    "any": "either",
}

# Upstream parameters we do not support, with the value that means "not
# used". An instance that sets one of them to anything else is skipped.
UNSUPPORTED_PARAMETER_DEFAULTS = {
    "minimum_distance_1_cuts": 0,
    "min1cut": 0,
    "maximum_distance_1_cuts": -1,
    "max1cut": -1,
    "minimum_distance_2_cuts": 0,
    "min2cut": 0,
    "maximum_distance_2_cuts": -1,
    "max2cut": -1,
    "minimum_waste_length": 0,
    "min_waste": 0,
    "maximum_number_1_cuts": -1,
    "maximum_number_2_cuts": -1,
    "cut_through_defects": None,  # only meaningful with defects
    "waste_cost": 0,
}


class Skip(Exception):
    """The instance uses a feature that is not in scope."""


def number(text):
    value = float(text)
    return int(value) if value.is_integer() else value


def convert_parameters(params):
    """Convert upstream instance-level parameters (lower-case keys)."""
    if "objective" not in params:
        raise Skip("no objective")
    objective_key = str(params["objective"]).lower().replace("-", "_")
    if objective_key not in IN_SCOPE_OBJECTIVES:
        raise Skip(f"objective {params['objective']}")

    stages = params.get("number_of_stages", 3)
    if str(stages) in ("u", "unlimited"):
        stages = "unlimited"
    else:
        stages = int(stages)
        if stages not in (2, 3):
            raise Skip(f"number_of_stages {stages}")

    cut_type_key = str(params.get("cut_type", "non-exact")).lower()
    if cut_type_key not in CUT_TYPES:
        raise Skip(f"cut_type {params['cut_type']}")

    orientation = str(params.get("first_stage_orientation", "vertical")).lower()
    if orientation not in FIRST_CUTS:
        raise Skip(f"first_stage_orientation {orientation}")

    known = {"objective", "number_of_stages", "cut_type",
             "first_stage_orientation", "cut_thickness"}
    for key, value in params.items():
        if key in known:
            continue
        if key in UNSUPPORTED_PARAMETER_DEFAULTS:
            default = UNSUPPORTED_PARAMETER_DEFAULTS[key]
            if default is not None and number(value) == default:
                continue
            if default is None:
                continue
        raise Skip(f"parameter {key}={value}")

    return IN_SCOPE_OBJECTIVES[objective_key], {
        "kerf": int(number(params.get("cut_thickness", 0))),
        "stages": stages,
        "cut_type": CUT_TYPES[cut_type_key],
        # Always explicit: our default ("either") differs from upstream's.
        "first_cuts": FIRST_CUTS[orientation],
    }


def convert_panel(index, width, height, copies, cost):
    panel = {"id": f"Panel {index + 1}", "length": width, "width": height}
    if copies != -1:
        panel["quantity"] = copies
    if cost is not None:
        panel["cost"] = cost
    return panel


def convert_part(index, width, height, copies, oriented, profit):
    if copies == -1:
        raise Skip("part with unlimited quantity")
    part = {"id": f"Part {index + 1}", "length": width, "width": height,
            "quantity": copies}
    if oriented:
        part["grain"] = True
    if profit is not None:
        if profit < 0:
            raise Skip("negative part value")
        part["value"] = profit
    return part


def read_csv_rows(path):
    with open(path, newline="") as file:
        return [{k.strip().upper(): v.strip() for k, v in row.items() if k}
                for row in csv.DictReader(file)]


def zero(row, column):
    return number(row.get(column) or 0) == 0


def convert_csv_directory(directory):
    params = {}
    if (directory / "parameters.csv").exists():
        for row in read_csv_rows(directory / "parameters.csv"):
            params[row["NAME"].lower()] = row["VALUE"]
    objective, saw = convert_parameters(params)

    if (directory / "defects.csv").exists() and read_csv_rows(directory / "defects.csv"):
        raise Skip("defects")

    panels = []
    for i, row in enumerate(read_csv_rows(directory / "bins.csv")):
        for trim in ("LEFT_TRIM", "RIGHT_TRIM", "BOTTOM_TRIM", "TOP_TRIM", "RIGHT_TRIP"):
            if not zero(row, trim):
                raise Skip("trims")
        if not zero(row, "COPIES_MIN"):
            raise Skip("panel minimum quantity")
        panels.append(convert_panel(
            i, number(row["WIDTH"]), number(row["HEIGHT"]),
            number(row.get("COPIES") or 1),
            number(row["COST"]) if row.get("COST") else None))

    parts = []
    for i, row in enumerate(read_csv_rows(directory / "items.csv")):
        if not zero(row, "COPIES_MIN"):
            raise Skip("part minimum quantity")
        parts.append(convert_part(
            i, number(row["WIDTH"]), number(row["HEIGHT"]),
            number(row.get("COPIES") or 1),
            number(row.get("ORIENTED") or 0) == 1,
            number(row["PROFIT"]) if row.get("PROFIT") else None))
    check_stacks([row.get("STACK_ID") for row in read_csv_rows(directory / "items.csv")])

    return finish(objective, saw, panels, parts)


def check_stacks(stack_ids):
    """Upstream gives each item its own stack by default; shared stacks
    impose an order between items, which is out of scope."""
    used = [s for s in stack_ids if s not in (None, "", "-1", -1)]
    if len(used) != len(set(used)):
        raise Skip("stacks")


def convert_json_file(path):
    j = json.loads(path.read_text())
    params = {k: v for k, v in j.items() if k not in ("bin_types", "item_types")}
    if "cutting_costs" in params:
        raise Skip("cutting costs")
    objective, saw = convert_parameters(params)

    panels = []
    for i, b in enumerate(j["bin_types"]):
        if b.get("defects"):
            raise Skip("defects")
        if b.get("resources"):
            raise Skip("resources")
        if any(b.get(t, 0) for t in ("left_trim", "right_trim", "bottom_trim", "top_trim")):
            raise Skip("trims")
        if b.get("copies_min", 0):
            raise Skip("panel minimum quantity")
        panels.append(convert_panel(i, b["width"], b["height"],
                                    b.get("copies", 1), b.get("cost")))

    parts = []
    for i, t in enumerate(j["item_types"]):
        if t.get("copies_min", 0):
            raise Skip("part minimum quantity")
        parts.append(convert_part(i, t["width"], t["height"], t.get("copies", 1),
                                  bool(t.get("oriented", False)), t.get("profit")))
    check_stacks([t.get("stack_id") for t in j["item_types"]])

    return finish(objective, saw, panels, parts)


def finish(objective, saw, panels, parts):
    if objective.startswith("open_dimension") and len(panels) != 1:
        raise Skip("open dimension with several panel types")
    return {"objective": objective, "saw": saw, "panels": panels, "parts": parts}


def find_instances(root):
    """Yield (path, converter) for every upstream instance."""
    for path in sorted(root.rglob("*")):
        if path.is_dir() and (path / "items.csv").exists():
            yield path, convert_csv_directory
        elif path.is_file() and path.suffix == ".json":
            yield path, convert_json_file


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    upstream = Path(sys.argv[1])
    root = upstream / "data" / "rectangleguillotine"
    out = Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)

    converted, skipped = [], []
    for path, convert in find_instances(root):
        relative = path.relative_to(root)
        source = f"data/rectangleguillotine/{relative.as_posix()}"
        try:
            instance = convert(path)
        except Skip as reason:
            skipped.append({"source": source, "reason": str(reason)})
            continue
        stem = relative.parent if path.suffix == ".json" else relative
        name = "__".join(stem.parts) + ".json"
        (out / name).write_text(json.dumps(instance, indent=2) + "\n")
        converted.append({"file": name, "source": source})

    manifest = {
        "upstream_commit": "9b656a0dcafd1e9019bf39a5910d4307af65579a",
        "converted": converted,
        "skipped": skipped,
    }
    (out / "MANIFEST.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"converted {len(converted)}, skipped {len(skipped)}")


if __name__ == "__main__":
    main()
