#!/usr/bin/env python3
"""Generate the static Stage 7 design mockups in web/mockups/.

    python3 web/mockups/build/build.py

Reads, never writes, the repository's real data:
  - docs/FIELDS.md for every label and help text (copied verbatim),
  - an instance from data/instances/ and two oracle plans for it from
    data/golden/ (geometry, sizes, quantities and grain are verbatim).

Only the part and panel *names* are replaced (NAMES below): the generated
instance calls them "Part 1".."Part 11", which reads poorly in a mockup.

Each page is written as one self-contained HTML file (CSS inlined, SVG
inline, no scripts, no network). The real app in web/src will replace this
script; it exists so the mockups stay reproducible and honest.
"""

from __future__ import annotations

import html
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "web" / "mockups"
INSTANCE = ROOT / "data/instances/generated__bin_packing_3_non_exact_kerf3_2.json"
PLAN_1 = ROOT / "data/golden/9b656a0d/generated__bin_packing_3_non_exact_kerf3_2.q64.json"
PLAN_2 = ROOT / "data/golden/9b656a0d/generated__bin_packing_3_non_exact_kerf3_2.q2048.json"

# Readable names for the generated instance's parts and panel (sizes untouched).
NAMES = {
    "Panel 1": "Birch ply 18 mm",
    "Part 1": "Drawer front",
    "Part 2": "Fixed shelf",
    "Part 3": "Top",
    "Part 4": "Side",
    "Part 5": "Door",
    "Part 6": "Cleat",
    "Part 7": "Kicker",
    "Part 8": "Shelf",
    "Part 9": "Part 9",  # left unnamed: the UI's generated name
    "Part 10": "Rail",
    "Part 11": "Stretcher",
}

# Illustrative timings for the two plans (the golden files carry none).
FOUND_AFTER = {1: 0.04, 2: 0.91}

esc = html.escape


# --------------------------------------------------------------------------
# FIELDS.md: labels and help text
# --------------------------------------------------------------------------

def load_fields() -> dict[tuple[str, str], dict]:
    """Parse the field tables of docs/FIELDS.md into {(section, field): row}."""
    sections = {"Job": "job", "Panels (stock sheets)": "panels", "Parts": "parts",
                "Saw and cutting": "saw"}
    fields: dict[tuple[str, str], dict] = {}
    section = None
    for line in (ROOT / "docs/FIELDS.md").read_text().splitlines():
        if line.startswith("### "):
            section = sections.get(line[4:].strip())
        elif line.startswith("## "):
            section = None
        elif section and line.startswith("| `"):
            cells = [c.strip() for c in line.strip().strip("|").split("|")]
            name = cells[0].strip("`")
            fields[(section, name)] = {
                "label": cells[1], "help": cells[2], "unit": cells[3], "default": cells[4],
            }
    return fields


FIELDS = load_fields()


def label(section: str, name: str) -> str:
    return FIELDS[(section, name)]["label"]


def help_html(section: str, name: str, current: str | None = None) -> str:
    """Help text as HTML: **bold** becomes <strong>. With `current`, the
    sentence(s) about that option (from its bold name to the next bold name)
    are emphasised. The words themselves are never changed."""
    text = FIELDS[(section, name)]["help"]
    parts = re.split(r"(\*\*.+?\*\*)", text)
    out, in_current = [], False
    for p in parts:
        if p.startswith("**"):
            word = p[2:-2]
            if in_current:
                out.append("</span>")
                in_current = False
            if current is not None and word == current:
                out.append('<span class="current">')
                in_current = True
            out.append(f"<strong>{esc(word)}</strong>")
        else:
            out.append(re.sub(r"\*(.+?)\*", r"<em>\1</em>", esc(p)))
    if in_current:
        out.append("</span>")
    return "".join(out)


def options(section: str, name: str) -> list[str]:
    """Option names: the bold words of a help text, in order."""
    return re.findall(r"\*\*(.+?)\*\*", FIELDS[(section, name)]["help"])


# --------------------------------------------------------------------------
# Data
# --------------------------------------------------------------------------

def fmt(v: float) -> str:
    """Millimetres as the user typed them: 720, 3.2, 1040.5."""
    v = round(v, 1)
    return str(int(v)) if v == int(v) else f"{v:.1f}"


def fmt_area_m2(mm2: float) -> str:
    return f"{mm2 / 1e6:.2f} m²"


def load_instance() -> dict:
    inst = json.loads(INSTANCE.read_text())
    for p in inst["panels"] + inst["parts"]:
        p["id"] = NAMES.get(p["id"], p["id"])
    return inst


def load_plan(path: Path) -> dict:
    raw = json.loads(path.read_text())

    def clean(n: dict) -> dict:
        m = {k: v for k, v in n.items() if k != "children"}
        if "part_id" in m:
            m["part_id"] = NAMES.get(m["part_id"], m["part_id"])
        kids = n.get("children") or []
        if kids:
            m["children"] = [clean(c) for c in kids]
        return m

    pats = []
    for p in raw["patterns"]:
        pats.append({"panel_id": NAMES.get(p["panel_id"], p["panel_id"]),
                     "repeat": p["repeat"], "first_cuts": p["first_cuts"],
                     "root": clean(p["root"])})
    return {"patterns": pats}


INST = load_instance()
KERF = INST["saw"]["kerf"]
STAGES = INST["saw"]["stages"]
PARTS = {p["id"]: p for p in INST["parts"]}
PANELS = {p["id"]: p for p in INST["panels"]}


def leaves(n: dict):
    if "children" in n:
        for c in n["children"]:
            yield from leaves(c)
    else:
        yield n


def area(n: dict) -> float:
    return n["length"] * n["width"]


def pattern_stats(p: dict) -> dict:
    ls = list(leaves(p["root"]))
    panel = area(p["root"])
    parts = [l for l in ls if l["kind"] == "part"]
    part_area = sum(area(l) for l in parts)
    leftover = sum(area(l) for l in ls if l["kind"] == "leftover")
    waste = panel - part_area - leftover
    return {"parts": len(parts), "part_area": part_area, "panel_area": panel,
            "leftover_area": leftover, "waste_area": waste,
            "waste_fraction": waste / (panel - leftover),
            "offcuts": [l for l in ls if l["kind"] == "offcut"],
            "leftovers": [l for l in ls if l["kind"] == "leftover"],
            "part_leaves": parts}


def plan_metrics(plan: dict) -> dict:
    m = {"panels_used": 0, "parts_cut": 0, "panel_area": 0.0, "part_area": 0.0,
         "leftover_area": 0.0, "waste_area": 0.0}
    for p in plan["patterns"]:
        s = pattern_stats(p)
        r = p["repeat"]
        m["panels_used"] += r
        m["parts_cut"] += s["parts"] * r
        for k in ("panel_area", "part_area", "leftover_area", "waste_area"):
            m[k] += s[k] * r
    m["waste_fraction"] = m["waste_area"] / (m["panel_area"] - m["leftover_area"])
    return m


PLAN1 = load_plan(PLAN_1)
PLAN2 = load_plan(PLAN_2)
M1 = plan_metrics(PLAN1)
M2 = plan_metrics(PLAN2)
PARTS_TOTAL = sum(p.get("quantity", 1) for p in INST["parts"])


# --------------------------------------------------------------------------
# Cut order: derived from the plan tree
# --------------------------------------------------------------------------

def cut_direction(piece: dict) -> str:
    """'across' when the children sit side by side along x (cuts at a fixed
    x), 'along' when they sit along y. Derived from geometry so it also
    works for unlimited stages, where directions mix."""
    kids = piece["children"]
    if len(kids) > 1 and kids[0]["y"] == kids[1]["y"] and kids[0]["x"] != kids[1]["x"]:
        return "across"
    if len(kids) > 1:
        return "along"
    # One child: a trim; it runs across the axis where the child is shorter.
    c = kids[0]
    return "across" if c["length"] < piece["length"] else "along"


def describe(node: dict, names: dict) -> str:
    size = f'{fmt(node["length"])} × {fmt(node["width"])}'
    if node["kind"] == "part":
        return f'<b>{esc(node["part_id"])}</b> {size}'
    if node["kind"] == "offcut":
        return f"offcut {size}"
    if node["kind"] == "leftover":
        return f"<b>leftover</b> {size}"
    return f'<b>{names[id(node)]}</b> {size}'


def cut_steps(pattern: dict) -> list[dict]:
    """The cuts of one pattern in a workable order: cut a piece into all its
    children, then work through each child in turn. Each step records the
    size to cut off (what the woodworker sets the fence or stop to)."""
    root = pattern["root"]
    names: dict[int, str] = {}
    letters = iter("ABCDEFGHIJKLMNOPQRSTUVWXYZ")

    def name_children(piece: dict, prefix: str, depth: int):
        n = 0
        for c in piece.get("children", []):
            if c["kind"] == "piece":
                n += 1
                if depth == 0:
                    names[id(c)] = f"Strip {next(letters)}"
                    name_children(c, names[id(c)].split()[1], depth + 1)
                else:
                    sep = "" if depth == 1 else "."
                    names[id(c)] = f"Piece {prefix}{sep}{n}"
                    name_children(c, f"{prefix}{sep}{n}", depth + 1)
            elif c["kind"] == "part":
                n += 1

    name_children(root, "", 0)
    steps: list[dict] = []

    def group_of(piece: dict) -> str:
        return "panel" if piece is root else names[id(piece)]

    def process(piece: dict, group: str):
        kids = piece.get("children", [])
        if not kids:
            return
        if len(kids) == 1 and kids[0]["length"] == piece["length"] and kids[0]["width"] == piece["width"]:
            # A wrapper piece the part fills exactly: nothing to cut.
            process(kids[0], group)
            return
        direction = cut_direction(piece)
        trim = all(k["stage"] > (STAGES if isinstance(STAGES, int) else 99) for k in kids)
        for i, child in enumerate(kids):
            last = i == len(kids) - 1
            if last and len(kids) > 1:
                break
            measure = child["length"] if direction == "across" else child["width"]
            rest = kids[i + 1] if i + 1 < len(kids) else None
            # The step's remainder note: only for the last cut of a piece.
            rest_note = None
            if rest is not None and i + 1 == len(kids) - 1:
                if rest["kind"] == "offcut" and min(rest["length"], rest["width"]) < KERF:
                    rest_note = f'saw dust ({fmt(min(rest["length"], rest["width"]))} mm)'
                else:
                    rest_note = describe(rest, names)
            steps.append({"n": len(steps) + 1, "piece": piece, "index": i,
                          "direction": direction, "measure": measure,
                          "child": child, "result": describe(child, names),
                          "rest": rest_note, "group": group, "trim": trim})
        for child in kids:
            process(child, group if piece is not root else names.get(id(child), "panel"))

    # Stage 1 first: all cuts of the panel into strips, then strip by strip.
    process(root, "panel")
    for s in steps:
        s["names"] = names
    return steps


# --------------------------------------------------------------------------
# SVG cutting plan
# --------------------------------------------------------------------------

def text_w(s: str, size: float, bold: bool = False) -> float:
    return len(s) * size * (0.6 if bold else 0.56)


# A double arrow along the length: the part keeps the panel's grain direction.
GRAIN_ICON = ('<path d="M1 4h12M3.5 1.5 1 4l2.5 2.5M10.5 1.5 13 4l-2.5 2.5" fill="none" '
              'stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>')

ROTATE_ICON = ('<path d="M9.6 4.3A4 4 0 1 0 10 6" fill="none" stroke-width="1.3" '
               'stroke-linecap="round"/><path d="M10.4 1.8v2.8H7.6" fill="none" '
               'stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>')


def render_panel(pattern: dict, *, width: float, uid: str, labels: str = "full",
                 badges: bool = False, rulers: bool = False, highlight: int | None = None,
                 done_before: int | None = None, name_size: float = 12,
                 title: str | None = None) -> str:
    """One panel of a plan as inline SVG.

    labels: 'full' (name and size on parts, size on offcuts), 'names' (part
    names only), 'none'. Drawn with the panel's length horizontal and y = 0
    at the bottom edge, as in docs/FIELDS.md."""
    root = pattern["root"]
    L, W = root["length"], root["width"]
    ml, mr, mt, mb = (70, 36, 34, 8) if rulers else (1, 1, 1, 1)
    s = width / L
    vw, vh = width + ml + mr, W * s + mt + mb

    def box(n):
        return (ml + n["x"] * s, mt + (W - n["y"] - n["width"]) * s,
                n["length"] * s, n["width"] * s)

    dim_size = name_size - 1
    out = [f'<svg viewBox="0 0 {vw:.1f} {vh:.1f}" role="img" class="plan-svg" '
           f'aria-labelledby="{uid}-t">']
    out.append(f'<title id="{uid}-t">{esc(title or "Cutting plan")}</title>')
    out.append(
        f'<defs><pattern id="{uid}-hatch" width="6" height="6" patternUnits="userSpaceOnUse" '
        f'patternTransform="rotate(45)"><rect width="6" height="6" class="p-offcut-bg"/>'
        f'<line x1="0" y1="0" x2="0" y2="6" class="p-hatch"/></pattern></defs>')
    # The panel itself shows through wherever the saw removes material.
    out.append(f'<rect x="{ml}" y="{mt}" width="{L * s:.2f}" height="{W * s:.2f}" class="p-kerf"/>')

    for leaf in leaves(root):
        x, y, w, h = box(leaf)
        kind = leaf["kind"]
        if kind == "part":
            part = PARTS[leaf["part_id"]]
            out.append(f'<rect x="{x:.2f}" y="{y:.2f}" width="{w:.2f}" height="{h:.2f}" class="p-part"/>')
            if part.get("grain") and labels != "none" and w > 70 and h > 48:
                out.append(f'<g transform="translate({x + w - 19:.2f} {y + h - 12:.2f})" class="p-glyph">{GRAIN_ICON}</g>')
            if leaf.get("rotated") and labels != "none" and w > 30 and h > 24:
                out.append(f'<g transform="translate({x + w - 16:.2f} {y + h - 16:.2f})" class="p-glyph">{ROTATE_ICON}</g>')
            out.append(part_label(leaf, x, y, w, h, labels, name_size, dim_size))
        elif kind == "offcut":
            out.append(f'<rect x="{x:.2f}" y="{y:.2f}" width="{w:.2f}" height="{h:.2f}" '
                       f'fill="url(#{uid}-hatch)" class="p-offcut"/>')
            if labels == "full" and min(leaf["length"], leaf["width"]) >= KERF:
                out.append(dim_label(leaf, x, y, w, h, dim_size - 0.5, "p-offcut-text"))
        elif kind == "leftover":
            out.append(f'<rect x="{x + 0.75:.2f}" y="{y + 0.75:.2f}" width="{w - 1.5:.2f}" '
                       f'height="{h - 1.5:.2f}" class="p-leftover"/>')
            if labels != "none":
                out.append(stacked_label("Keep · leftover", f'{fmt(leaf["length"])} × {fmt(leaf["width"])}',
                                         x, y, w, h, name_size, dim_size, "p-leftover-text")
                           if labels == "full" else
                           stacked_label("Keep", None, x, y, w, h, name_size, dim_size, "p-leftover-text"))

    out.append(f'<rect x="{ml}" y="{mt}" width="{L * s:.2f}" height="{W * s:.2f}" class="p-edge"/>')

    if rulers:
        out.append(render_rulers(pattern, ml, mt, s))

    if badges:
        steps = cut_steps(pattern)
        placed: list[tuple[float, float]] = []
        hl = []
        marks = []
        for st in steps:
            piece, child = st["piece"], st["child"]
            px, py, pw, ph = box(piece)
            r = 9
            if st["direction"] == "across":
                gx0 = ml + (child["x"] + child["length"]) * s
                gw = max(KERF * s, 1.5)
                line = (gx0, py, gw, ph)
                bx, by = gx0 + gw / 2, py + 12
                step_dx, step_dy = 0, 2 * r + 3
            else:
                # Drawing y is flipped: the gap lies just above the child.
                gy1 = mt + (W - child["y"] - child["width"]) * s
                gh = max(KERF * s, 1.5)
                line = (px, gy1 - gh, pw, gh)
                bx, by = px + 12, gy1 - gh / 2
                step_dx, step_dy = 2 * r + 3, 0
            for _ in range(12):
                if all((bx - qx) ** 2 + (by - qy) ** 2 >= (2 * r + 2) ** 2 for qx, qy in placed):
                    break
                bx += step_dx
                by += step_dy
            placed.append((bx, by))
            cls = "p-badge"
            if highlight == st["n"]:
                cls += " on"
                lx, ly, lw, lh = line
                hl.append(f'<rect x="{lx:.2f}" y="{ly - 0.75:.2f}" width="{lw:.2f}" height="{lh + 1.5:.2f}" class="p-cutline"/>'
                          if st["direction"] == "along" else
                          f'<rect x="{lx - 0.75:.2f}" y="{ly:.2f}" width="{lw + 1.5:.2f}" height="{lh:.2f}" class="p-cutline"/>')
                cx, cy, cw, ch = box(child)
                hl.append(f'<rect x="{cx:.2f}" y="{cy:.2f}" width="{cw:.2f}" height="{ch:.2f}" class="p-result"/>')
            elif done_before is not None and st["n"] < done_before:
                cls += " done"
            marks.append(f'<g class="{cls}"><circle cx="{bx:.2f}" cy="{by:.2f}" r="{r}"/>'
                         f'<text x="{bx:.2f}" y="{by + 3.6:.2f}" text-anchor="middle">{st["n"]}</text></g>')
        out.extend(hl)
        out.extend(marks)

    out.append("</svg>")
    return "".join(out)


def part_label(leaf, x, y, w, h, labels, name_size, dim_size) -> str:
    if labels == "none":
        return ""
    name = leaf["part_id"]
    dims = f'{fmt(leaf["length"])} × {fmt(leaf["width"])}'
    if labels == "names":
        dims = None
    return stacked_label(name, dims, x, y, w, h, name_size, dim_size, "p-part-text")


def stacked_label(name, dims, x, y, w, h, ns, ds, cls) -> str:
    """Name over size, centred; falls back to vertical text, then size only,
    then nothing (the part is still listed beside the drawing)."""
    nw = text_w(name, ns, True)
    dw = text_w(dims, ds) if dims else 0
    need_w = max(nw, dw) + 10
    need_h = (ns + ds + 6) if dims else ns + 6
    cx, cy = x + w / 2, y + h / 2

    def block(rotate: bool) -> str:
        t = f' transform="rotate(-90 {cx:.2f} {cy:.2f})"' if rotate else ""
        if dims:
            return (f'<g class="{cls}"{t}><text x="{cx:.2f}" y="{cy - 1.5:.2f}" text-anchor="middle" '
                    f'class="p-name" font-size="{ns}">{esc(name)}</text>'
                    f'<text x="{cx:.2f}" y="{cy + ds + 1:.2f}" text-anchor="middle" class="p-dim" '
                    f'font-size="{ds}">{esc(dims)}</text></g>')
        return (f'<g class="{cls}"{t}><text x="{cx:.2f}" y="{cy + ns * 0.35:.2f}" text-anchor="middle" '
                f'class="p-name" font-size="{ns}">{esc(name)}</text></g>')

    if w >= need_w and h >= need_h:
        return block(False)
    if h >= need_w and w >= need_h and h > w:
        return block(True)
    if dims:
        return dim_label_raw(dims, x, y, w, h, ds, cls)
    return ""


def dim_label(leaf, x, y, w, h, size, cls) -> str:
    return dim_label_raw(f'{fmt(leaf["length"])} × {fmt(leaf["width"])}', x, y, w, h, size, cls)


def dim_label_raw(dims, x, y, w, h, size, cls) -> str:
    tw = text_w(dims, size)
    cx, cy = x + w / 2, y + h / 2
    if w >= tw + 8 and h >= size + 5:
        return (f'<text x="{cx:.2f}" y="{cy + size * 0.35:.2f}" text-anchor="middle" '
                f'class="{cls} p-dim" font-size="{size}">{esc(dims)}</text>')
    if h >= tw + 8 and w >= size + 5:
        return (f'<text x="{cx:.2f}" y="{cy + size * 0.35:.2f}" text-anchor="middle" '
                f'transform="rotate(-90 {cx:.2f} {cy:.2f})" class="{cls} p-dim" '
                f'font-size="{size}">{esc(dims)}</text>')
    return ""


def render_rulers(pattern, ml, mt, s) -> str:
    root = pattern["root"]
    L, W = root["length"], root["width"]
    o = ['<g class="p-ruler">']
    # Panel length along the top.
    y0 = mt - 16
    o.append(f'<line x1="{ml}" y1="{y0}" x2="{ml + L * s:.2f}" y2="{y0}"/>'
             f'<line x1="{ml}" y1="{y0 - 5}" x2="{ml}" y2="{y0 + 5}"/>'
             f'<line x1="{ml + L * s:.2f}" y1="{y0 - 5}" x2="{ml + L * s:.2f}" y2="{y0 + 5}"/>')
    lab = f"{fmt(L)}"
    tx = ml + L * s / 2
    o.append(f'<rect x="{tx - text_w(lab, 11) / 2 - 6:.2f}" y="{y0 - 8}" width="{text_w(lab, 11) + 12:.2f}" '
             f'height="16" class="p-ruler-bg"/><text x="{tx:.2f}" y="{y0 + 4}" text-anchor="middle">{lab}</text>')
    # Panel width on the right.
    x1 = ml + L * s + 16
    o.append(f'<line x1="{x1:.2f}" y1="{mt}" x2="{x1:.2f}" y2="{mt + W * s:.2f}"/>'
             f'<line x1="{x1 - 5:.2f}" y1="{mt}" x2="{x1 + 5:.2f}" y2="{mt}"/>'
             f'<line x1="{x1 - 5:.2f}" y1="{mt + W * s:.2f}" x2="{x1 + 5:.2f}" y2="{mt + W * s:.2f}"/>')
    lab = f"{fmt(W)}"
    cy = mt + W * s / 2
    o.append(f'<g transform="rotate(-90 {x1:.2f} {cy:.2f})"><rect x="{x1 - text_w(lab, 11) / 2 - 6:.2f}" '
             f'y="{cy - 8:.2f}" width="{text_w(lab, 11) + 12:.2f}" height="16" class="p-ruler-bg"/>'
             f'<text x="{x1:.2f}" y="{cy + 4:.2f}" text-anchor="middle">{lab}</text></g>')
    # Strip sizes of the first cuts on the left (along the length) or on top.
    if pattern["first_cuts"] == "along_length":
        xr = ml - 12
        for c in root["children"]:
            ya = mt + (W - c["y"] - c["width"]) * s
            yb = mt + (W - c["y"]) * s
            o.append(f'<line x1="{xr - 4}" y1="{ya:.2f}" x2="{xr + 4}" y2="{ya:.2f}"/>'
                     f'<line x1="{xr - 4}" y1="{yb:.2f}" x2="{xr + 4}" y2="{yb:.2f}"/>'
                     f'<line x1="{xr}" y1="{ya:.2f}" x2="{xr}" y2="{yb:.2f}"/>')
            if yb - ya >= 14:
                cls = ' class="p-ruler-weak"' if c["kind"] in ("offcut", "leftover") else ""
                o.append(f'<text x="{xr - 8}" y="{(ya + yb) / 2 + 4:.2f}" text-anchor="end"{cls}>{fmt(c["width"])}</text>')
    o.append("</g>")
    return "".join(o)


# Plan colours and text, appended to the stylesheet.
PLAN_CSS = """
.plan-svg { font-family: var(--font); font-variant-numeric: tabular-nums; }
.p-kerf { fill: var(--kerf); }
.p-edge { fill: none; stroke: var(--part-line); stroke-width: 1; }
.p-part { fill: var(--part); stroke: var(--part-line); stroke-width: .75; }
.p-offcut-bg { fill: var(--offcut); }
.p-hatch { stroke: var(--hatch); stroke-width: 1.2; }
.p-offcut { stroke: none; }
.p-leftover { fill: var(--leftover); stroke: var(--muted); stroke-width: 1; stroke-dasharray: 4 3; }
.p-part-text { fill: var(--ink); }
.p-name { font-weight: 500; }
.p-dim { font-weight: 400; }
.p-part-text .p-dim { fill: var(--muted); }
.p-offcut-text { fill: var(--muted); }
.p-leftover-text { fill: var(--ink); }
.p-leftover-text .p-dim { fill: var(--muted); }
.p-glyph { stroke: var(--ink); opacity: .7; }
.p-ruler line { stroke: var(--faint); stroke-width: 1; }
.p-ruler text { fill: var(--muted); font-size: 11px; }
.p-ruler .p-ruler-weak { fill: var(--faint); }
.p-ruler-bg { fill: var(--surface); }
.p-badge circle { fill: var(--surface); stroke: var(--ink); stroke-width: 1; }
.p-badge text { fill: var(--ink); font-size: 10px; font-weight: 600; }
.p-badge.on circle { fill: var(--accent); stroke: var(--accent); }
.p-badge.on text { fill: var(--on-accent); }
.p-badge.done { opacity: .4; }
.p-cutline { fill: var(--accent); }
.p-result { fill: var(--accent); fill-opacity: .18; stroke: var(--accent); stroke-width: 2; }
"""


# --------------------------------------------------------------------------
# Page shell and shared pieces
# --------------------------------------------------------------------------

CSS = (Path(__file__).parent / "styles.css").read_text() + PLAN_CSS


# --------------------------------------------------------------------------
# Page shell and shared pieces
# --------------------------------------------------------------------------

LOGO = ('<svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true" fill="none" '
        'stroke="currentColor" stroke-width="1.6"><rect x="1.5" y="1.5" width="15" height="15" rx="1.5"/>'
        '<path d="M1.5 9.5h15M8 1.5v8M11.5 9.5v7"/></svg>')

JOB_NAME = "Kitchen units"
CONTEXT = JOB_NAME


def icon(name: str, size: int = 14) -> str:
    paths = {
        "plus": '<path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>',
        "close": '<path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>',
    }
    return (f'<svg width="{size}" height="{size}" viewBox="0 0 16 16" aria-hidden="true">'
            f'{paths[name]}</svg>')


def shell(title: str, tab: str, body: str, live: bool = False) -> str:
    def t(key, text):
        cur = ' aria-current="page"' if key == tab else ""
        dot = '<span class="live" aria-hidden="true"></span>' if live and key == "plan" else ""
        return f'<a href="#"{cur}>{text}{dot}</a>'

    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{esc(title)} · Sheetsolver</title>
<style>{CSS}</style>
</head>
<body>
<header class="chrome"><span class="brand">{LOGO}Sheetsolver</span><span class="context">{CONTEXT}</span></header>
<nav class="topnav" aria-label="Job views">{t("setup", "Job setup")}{t("plan", "Cutting plan")}</nav>
{body}
</body>
</html>
"""


def pct(f: float) -> str:
    return f"{f * 100:.1f}%"


# --------------------------------------------------------------------------
# Help on demand
# --------------------------------------------------------------------------

def help_btn(hid: str, section: str, field: str, open_: bool = False) -> str:
    return (f'<button class="help-btn" type="button" aria-expanded="{"true" if open_ else "false"}" '
            f'aria-controls="{hid}" aria-label="About {esc(label(section, field))}">?</button>')


def help_pop(hid: str, section: str, field: str, side: str) -> str:
    return (f'<div class="pop {side}" id="{hid}" role="dialog" aria-label="{esc(label(section, field))}">'
            f'<div class="pop-title"><span>{esc(label(section, field))}</span>'
            f'<button class="icon-btn" style="width:18px;height:18px" aria-label="Close">{icon("close", 12)}</button></div>'
            f'{help_html(section, field)}</div>')


# --------------------------------------------------------------------------
# Job setup
# --------------------------------------------------------------------------

def cell(value, *, cls="", placeholder="", label_text="", text=False) -> str:
    v = f' value="{esc(str(value))}"' if value not in (None, "") else ""
    ph = f' placeholder="{esc(placeholder)}"' if placeholder else ""
    t = " text" if text else ""
    inv = ' aria-invalid="true"' if "invalid" in cls else ""
    return f'<input class="cell{t} {cls}"{v}{ph} aria-label="{esc(label_text)}"{inv} readonly>'


def th(section, field, cls, unit=None, focus=False, pop=None) -> str:
    hid = f"help-{section}-{field}"
    u = f' <span class="unit">{unit}</span>' if unit else ""
    lab = label(section, field)
    if field == "grain":
        lab = "Keep grain ↔"
    fc = " focus-col" if focus else ""
    popover = help_pop(hid, section, field, pop) if pop else ""
    return (f'<th class="{cls}{fc}" scope="col">{esc(lab)}{u}{help_btn(hid, section, field, bool(pop))}{popover}</th>')


def parts_table(errors: dict, focus: tuple[int, str] | None, pop_field: str | None) -> str:
    cols = [("id", "w-name", None), ("length", "w-len num", "mm"), ("width", "w-len num", "mm"),
            ("quantity", "w-qty num", None), ("grain", "w-chk c", None)]
    head = '<th></th>' + "".join(
        th("parts", f, cls, unit, focus=bool(focus and focus[1] == f),
           pop=("right" if f in ("grain", "quantity") else "left") if f == pop_field else None)
        for f, cls, unit in cols) + '<th class="w-del"></th>'
    rows = []
    for i, p in enumerate(INST["parts"]):
        has_err = any((i, f) in errors for f, _, _ in cols)
        cells = [f'<td class="idx">{i + 1}</td>']
        for f, cls, _ in cols:
            key = (i, f)
            state = "invalid" if key in errors else ("focus" if focus == key else "")
            lab = f'{label("parts", f)}, row {i + 1}'
            if f == "grain":
                chk = " checked" if p.get("grain") else ""
                cells.append(f'<td class="c"><input type="checkbox"{chk} aria-label="{esc(lab)}"></td>')
            else:
                val = errors.get(key, (None, None))[0] or (fmt(p[f]) if f in ("length", "width") else p.get(f, ""))
                if f == "id" and val == f"Part {i + 1}":
                    # Left empty: the generated name shows as a placeholder.
                    cells.append(f'<td>{cell("", placeholder=val, label_text=lab, text=True)}</td>')
                    continue
                cells.append(f'<td>{cell(val, cls=state, label_text=lab, text=(f == "id"))}</td>')
        cells.append(f'<td><button class="icon-btn" aria-label="Remove row {i + 1}">{icon("close", 12)}</button></td>')
        rows.append(f'<tr class="{"has-err" if has_err else ""}">{"".join(cells)}</tr>')
        for f, _, _ in cols:
            if (i, f) in errors:
                rows.append(f'<tr class="err-row"><td></td><td colspan="{len(cols) + 1}"><div class="cell-error" id="err-{i}-{f}">'
                            f'<strong>{esc(label("parts", f))}:</strong> {esc(errors[(i, f)][1])}</div></td></tr>')
    return (f'<div class="table-wrap" style="overflow:visible"><table><thead><tr>{head}</tr></thead>'
            f'<tbody>{"".join(rows)}</tbody></table></div>')


def panels_table() -> str:
    cols = [("id", "w-name", None), ("length", "w-len num", "mm"), ("width", "w-len num", "mm"),
            ("quantity", "w-qty num", None), ("cost", "w-qty num", None)]
    head = '<th></th>' + "".join(th("panels", f, cls, unit) for f, cls, unit in cols) + '<th class="w-del"></th>'
    rows = []
    for i, p in enumerate(INST["panels"]):
        rows.append(
            f'<tr><td class="idx">{i + 1}</td><td>{cell(p["id"], label_text="Name", text=True)}</td>'
            f'<td>{cell(fmt(p["length"]), label_text="Length")}</td><td>{cell(fmt(p["width"]), label_text="Width")}</td>'
            f'<td>{cell(p.get("quantity"), placeholder="Unlimited", label_text="Quantity")}</td>'
            f'<td>{cell(p.get("cost"), placeholder="Area", label_text="Cost")}</td>'
            f'<td><button class="icon-btn" aria-label="Remove panel">{icon("close", 12)}</button></td></tr>')
    return (f'<div class="table-wrap" style="overflow:visible"><table><thead><tr>{head}</tr></thead>'
            f'<tbody>{"".join(rows)}</tbody></table></div>')


def setting(section: str, field: str, control: str, *, open_: bool = False,
            current: str | None = None, unit: str | None = None) -> str:
    hid = f"help-{section}-{field}"
    u = f' <span class="unit">{unit}</span>' if unit else ""
    box = (f'<div class="help-box" id="{hid}">{help_html(section, field, current)}</div>' if open_
           else f'<div class="help-box" id="{hid}" hidden></div>')
    return (f'<div class="field"><div class="label-row"><span class="field-label">{esc(label(section, field))}{u}</span>'
            f'{help_btn(hid, section, field, open_)}</div>{control}{box}</div>')


def setting_ui(field: str, control: str) -> str:
    row = {"label": label("job", field), "help": FIELDS[("job", field)]["help"]}
    hid = f"help-job-{field}"
    return (f'<div class="label-row"><span class="field-label">{esc(row["label"])} <span class="unit">optional</span></span>'
            f'<button class="help-btn" type="button" aria-expanded="false" aria-controls="{hid}" '
            f'aria-label="About {esc(row["label"])}">?</button></div>{control}'
            f'<div class="help-box" id="{hid}" hidden></div>')


def radio_row(opts: list[str], current: str, aria: str) -> str:
    items = "".join(f'<label class="{"on" if o == current else ""}">{esc(o)}</label>' for o in opts)
    return f'<div class="radio-row" role="radiogroup" aria-label="{esc(aria)}">{items}</div>'


def select(value: str, aria: str) -> str:
    return f'<select class="select" aria-label="{esc(aria)}"><option>{esc(value)}</option></select>'


def plan_settings(advanced_open: bool, open_help: str | None) -> str:
    goal_opts = options("job", "objective")
    ct = options("saw", "cut_type")
    fc = options("saw", "first_cuts")
    goal = setting("job", "objective", select(goal_opts[0], label("job", "objective")),
                   open_=open_help == "objective", current=goal_opts[0])
    kerf = setting("saw", "kerf", f'<div class="input-wrap input-short"><input class="input" value="{fmt(KERF)}" '
                   f'inputmode="decimal" aria-label="Kerf" readonly><span class="suffix">mm</span></div>',
                   open_=open_help == "kerf")
    adv = "".join([
        setting("saw", "stages", radio_row(["2", "3", "Unlimited"], "3", label("saw", "stages")), open_=open_help == "stages"),
        setting("saw", "cut_type", radio_row(ct, ct[0], label("saw", "cut_type")), open_=open_help == "cut_type", current=ct[0]),
        setting("saw", "first_cuts", radio_row(fc, fc[1], label("saw", "first_cuts")), open_=open_help == "first_cuts", current=fc[1]),
        setting("job", "time_limit", '<div class="input-wrap input-short"><input class="input" value="10" '
                'inputmode="numeric" aria-label="Time limit" readonly><span class="suffix">s</span></div>',
                open_=open_help == "time_limit"),
    ])
    op = " open" if advanced_open else ""
    return f"""
<h2 style="margin-bottom:16px">Plan settings</h2>
{goal}
{kerf}
<details class="advanced"{op}><summary><span class="sum">More cutting settings
  <small>3 stages · trimming allowed · {esc(fc[1].lower())} first · 10 s</small></span></summary>
  <div class="inner">{adv}</div></details>"""


def editor_page(errors: dict | None = None) -> str:
    errors = errors or {}
    focus = None if errors else (2, "length")
    alert = ""
    if errors:
        items = "".join(f'<li><a href="#err-{i}-{f}">Row {i + 1}, {esc(label("parts", f))}</a>: {esc(msg)}</li>'
                        for (i, f), (_, msg) in errors.items())
        alert = (f'<div class="alert" role="alert"><h3>The job cannot be solved as entered.</h3>'
                 f'<p>Fix the highlighted fields, then find the plan again.</p><ul>{items}</ul></div>')
    note = (f'<p class="hint" style="margin-top:12px;color:var(--bad)">{len(errors)} problems need attention.</p>' if errors
            else '<p class="hint" style="margin-top:12px">All required sizes are entered.</p>')
    body = f"""
<main class="narrow">
  <div class="heading"><div><div class="eyebrow">Job setup</div><h1>What are you cutting?</h1>
    <p class="muted">Sizes in millimetres, at most one decimal (720, 3.2, 18.5). Tap <span class="help-btn" aria-hidden="true" style="display:inline-grid">?</span> for what a field means.</p></div></div>
  {alert}
  <div class="setup-grid">
    <div>
      <div class="field job-name">{setting_ui("name", f'<input class="input" value="{JOB_NAME}" placeholder="Untitled job" aria-label="Job name" readonly>')}</div>
      <section class="form-block" aria-labelledby="parts-h">
        <div class="section-title"><h2 id="parts-h">Parts to make</h2><small>{PARTS_TOTAL} parts · {len(INST["parts"])} sizes</small></div>
        {parts_table(errors, focus, None if errors else "grain")}
        <div class="table-foot"><button class="link">{icon("plus")}Add part</button>
          <small>↔ The part keeps the panel’s grain direction.</small></div>
      </section>
      <section class="form-block" aria-labelledby="panels-h">
        <div class="section-title"><h2 id="panels-h">Panels you have</h2><small>{len(INST["panels"])} size</small></div>
        {panels_table()}
        <div class="table-foot"><button class="link">{icon("plus")}Add panel size</button>
          <small>Leave Quantity empty for as many as needed.</small></div>
      </section>
    </div>
    <aside class="setup-aside">
      {plan_settings(advanced_open=bool(errors), open_help=None if errors else "kerf")}
      <div class="summary-row" style="margin-top:6px"><span>Parts</span><strong>{PARTS_TOTAL} in {len(INST["parts"])} sizes</strong></div>
      <div class="summary-row"><span>Panels</span><strong>2440 × 1220, as many as needed</strong></div>
      <div class="aside-cta hide-tablet"><a class="button primary big block" href="#">Find cutting plan →</a>{note}</div>
    </aside>
  </div>
</main>
<div class="cta-bar show-tablet"><small>{PARTS_TOTAL} parts · 1 panel size</small><a class="button primary big" href="#">Find cutting plan →</a></div>
"""
    return shell("Job setup", "setup", body)


# --------------------------------------------------------------------------
# Cutting plan: shared header
# --------------------------------------------------------------------------

def plan_stats(m: dict, live: bool = False) -> str:
    left = PLAN2["patterns"][-1]
    lo = pattern_stats(left)["leftovers"][0]
    return f"""
  <div class="stats">
    <div class="stat"><strong>{m["panels_used"]}</strong>panels<small>2440 × 1220 mm each</small></div>
    <div class="stat"><strong>{m["parts_cut"]} / {PARTS_TOTAL}</strong>parts<small class="ok">✓ No missing parts</small></div>
    <div class="stat"><strong>{pct(m["waste_fraction"])}</strong>waste<small>{fmt_area_m2(m["waste_area"])} offcuts and saw dust</small></div>
    <div class="stat"><strong>1</strong>leftover to keep<small>{fmt(lo["length"])} × {fmt(lo["width"])} mm · panel 3</small></div>
  </div>"""


def plan_heading() -> str:
    return f"""
  <div class="heading">
    <div><div class="eyebrow">{esc(JOB_NAME)} · plan 2</div><h1>All {PARTS_TOTAL} parts fit</h1>
      <p class="muted">{M2["panels_used"]} panels of Birch ply 18 mm, 2440 × 1220 mm</p></div>
    <div class="actions"><a class="button" href="#">Edit job</a></div>
  </div>
  {plan_stats(M2)}"""


def plan_status() -> str:
    return (f'<p class="statusline" style="margin:-8px 0 20px"><span class="ok">✓ Complete plan</span> · time limit reached '
            f'after 10 s, so a longer search may still save material. <a class="link" href="#">Search for 60 s</a></p>')


def sheet_nav(current: int | None) -> str:
    """current: None = all panels, else the pattern index."""
    items = []
    cur = ' aria-current="page"' if current is None else ""
    items.append(f'<a class="sheet-choice" href="#"{cur}><div class="all" aria-hidden="true"><span></span><span></span><span></span></div>'
                 f'<strong>All panels</strong><small>Overview and parts</small></a>')
    for j, p in enumerate(PLAN2["patterns"]):
        st = pattern_stats(p)
        cur = ' aria-current="page"' if current == j else ""
        keep = "<small>1 strip to keep</small>" if st["leftovers"] else ""
        items.append(f'<a class="sheet-choice" href="#"{cur}>'
                     f'{render_panel(p, width=144, uid=f"n{j}", labels="none", title=f"Panel {j + 1}")}'
                     f'<strong>Panel {j + 1}</strong><small>{st["parts"]} parts · {pct(st["waste_fraction"])} waste</small>{keep}</a>')
    return f'<nav class="sheet-nav" aria-label="Panels">{"".join(items)}</nav>'


def legend() -> str:
    def sw(cls):
        return f'<svg width="16" height="11" aria-hidden="true" class="plan-svg"><rect x=".5" y=".5" width="15" height="10" class="{cls}"/></svg>'
    return (f'<div class="legend">'
            f'<span>{sw("p-part")}Part</span>'
            f'<span><svg width="14" height="9" viewBox="0 0 14 8" class="plan-svg p-glyph" aria-hidden="true">{GRAIN_ICON}</svg>Keeps grain direction</span>'
            f'<span><svg width="12" height="12" viewBox="0 0 12 12" class="plan-svg p-glyph" aria-hidden="true">{ROTATE_ICON}</svg>Turned 90°</span>'
            f'<span><svg width="16" height="11" class="plan-svg" aria-hidden="true"><defs><pattern id="lg-h" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="4" height="4" class="p-offcut-bg"/><line x1="0" y1="0" x2="0" y2="4" class="p-hatch"/></pattern></defs><rect x=".5" y=".5" width="15" height="10" fill="url(#lg-h)"/></svg>Offcut</span>'
            f'<span>{sw("p-leftover")}Leftover to keep</span>'
            f'<span><svg width="16" height="11" class="plan-svg" aria-hidden="true"><rect x="7" y="0" width="2" height="11" class="p-kerf"/></svg>Saw cut, {fmt(KERF)} mm</span>'
            f'</div>')


def search_details() -> str:
    return (f'<details class="more"><summary>Cutting settings and search details</summary>'
            f'<p>Kerf {fmt(KERF)} mm · 3 cutting stages · first cuts along the length · last-stage trimming allowed.</p>'
            f'<p>The search stopped at its 10-second limit; this plan was found after {FOUND_AFTER[2]:.1f} s. '
            f'Reaching the time limit does not prove that fewer panels or less waste is impossible.</p></details>')


# --------------------------------------------------------------------------
# Cutting plan: searching
# --------------------------------------------------------------------------

def solving_page() -> str:
    elapsed, limit = 3.4, 10
    cards = []
    for k, p in enumerate(PLAN2["patterns"]):
        st = pattern_stats(p)
        cards.append(
            f'<section class="viewer"><div class="viewer-head"><div><h3>Panel {k + 1} <small>/ 3</small></h3>'
            f'<small>{st["parts"]} parts · {pct(st["waste_fraction"])} waste</small></div></div>'
            f'{render_panel(p, width=470, uid=f"s{k}", labels="names", name_size=11, title=f"Panel {k + 1} of the best plan so far")}</section>')
    delta = (M1["waste_fraction"] - M2["waste_fraction"]) * 100
    body = f"""
<main>
  <div class="heading">
    <div><div class="eyebrow">{esc(JOB_NAME)} · searching</div><h1>All {PARTS_TOTAL} parts fit so far</h1>
      <p class="muted">Looking for a plan with less waste. The best plan so far is shown and replaced as soon as a better one is found.</p></div>
    <div class="actions"><button class="button">Stop and keep this plan</button></div>
  </div>
  <div class="statusline num"><span style="color:var(--ink);font-weight:600">{elapsed:.1f} s</span> of {limit} s · about {limit - elapsed:.0f} s left</div>
  <div class="progress" role="progressbar" aria-label="Search time" aria-valuenow="{elapsed}" aria-valuemin="0" aria-valuemax="{limit}"><span style="width:{elapsed / limit * 100:.0f}%"></span></div>
  {plan_stats(M2, live=True)}
  <div class="live-grid">
    <div class="overview">{"".join(cards)}</div>
    <aside class="side">
      <div class="side-head"><h3>Plans found</h3><small>2</small></div>
      <ul class="plans" style="padding:0 16px 8px">
        <li><span><b>Plan 2</b> · {M2["panels_used"]} panels · {pct(M2["waste_fraction"])} waste</span><span class="ok">−{delta:.1f} pt</span></li>
        <li><span><b>Plan 1</b> · {M1["panels_used"]} panels · {pct(M1["waste_fraction"])} waste</span><span class="muted num">{FOUND_AFTER[1]:.2f} s</span></li>
      </ul>
      <p class="help" style="border-top:1px solid var(--line);padding-top:10px">Plan 2 was found after {FOUND_AFTER[2]:.2f} s. You can open it now; it updates itself while the search runs.</p>
      <div style="padding:0 16px 16px"><a class="button primary block" href="#">Open plan</a></div>
    </aside>
  </div>
</main>"""
    return shell("Searching", "plan", body, live=True)


# --------------------------------------------------------------------------
# Cutting plan: panel by panel and overview
# --------------------------------------------------------------------------

def cut_list(pattern: dict, highlight: int) -> tuple[str, int, dict]:
    steps = cut_steps(pattern)
    out, group, current = [], None, None
    for st in steps:
        if st["group"] != group:
            group = st["group"]
            if group == "panel":
                title, meta = "Panel into strips", "along the length"
            else:
                node = next(n for n in iter_nodes(pattern["root"]) if st["names"].get(id(n)) == group)
                title, meta = group, f'{fmt(node["length"])} × {fmt(node["width"])}'
            out.append(f'<li class="group"><span>{esc(title)}</span><span class="num">{meta}</span></li>')
        cls = "cut" + (" on" if st["n"] == highlight else "") + (" done" if st["n"] < highlight else "")
        dirw = ("trim, " if st["trim"] else "") + ("along the length" if st["direction"] == "along" else "across the length")
        rest = f'<div class="r">leaves {st["rest"]}</div>' if st["rest"] else ""
        cur = ' aria-current="step"' if st["n"] == highlight else ""
        if st["n"] == highlight:
            current = st
        out.append(f'<li class="{cls}"{cur}><span class="n">{st["n"]}</span><div>'
                   f'<div class="m">{fmt(st["measure"])} mm<em>{dirw}</em></div>'
                   f'<div class="r">→ {st["result"]}</div>{rest}</div></li>')
    return f'<ol class="cutlist">{"".join(out)}</ol>', len(steps), current


def iter_nodes(n):
    yield n
    for c in n.get("children", []):
        yield from iter_nodes(c)


def plan_panel_page() -> str:
    k = 0
    pat = PLAN2["patterns"][k]
    st = pattern_stats(pat)
    highlight = 7
    figure = render_panel(pat, width=700, uid="main", labels="full", badges=True, rulers=True,
                          highlight=highlight, done_before=highlight, name_size=11.5,
                          title="Panel 1 of 3: Birch ply 18 mm, 2440 by 1220 mm")
    cl, ncuts, cur = cut_list(pat, highlight)
    dirw = "along the length" if cur["direction"] == "along" else "across the length"

    counts: dict[str, int] = {}
    for l in st["part_leaves"]:
        counts[l["part_id"]] = counts.get(l["part_id"], 0) + 1
    on_panel = "".join(
        f'<div class="manifest-row"><div>{esc(pid)} <span>{fmt(PARTS[pid]["length"])} × {fmt(PARTS[pid]["width"])}</span></div><strong>× {n}</strong></div>'
        for pid, n in counts.items())
    offs = sorted((o for o in st["offcuts"] if min(o["length"], o["width"]) >= KERF), key=area, reverse=True)
    off_rows = "".join(
        f'<div class="manifest-row"><div>{fmt(o["length"])} × {fmt(o["width"])}</div><span>{area(o) / 1e6:.3f} m²</span></div>'
        for o in offs[:5])
    body = f"""
<main>
  {plan_heading()}
  {plan_status()}
  <div class="workbench">
    <div class="wb-nav">{sheet_nav(k)}</div>
    <div class="wb-main">
      <section class="viewer">
        <div class="viewer-head"><div><h2>Panel 1 <small>/ 3</small></h2>
          <small>Birch ply 18 mm · 2440 × 1220 · {st["parts"]} parts · {pct(st["waste_fraction"])} waste</small></div>
          <small>Grain →</small></div>
        {figure}
        {legend()}
        <div class="detail" aria-live="polite">
          <div><small>Cut {cur["n"]} of {ncuts}</small><div class="now">{fmt(cur["measure"])} mm <em>{dirw} → {cur["result"]}</em></div></div>
          <div class="stepper"><button class="button" aria-label="Previous cut">‹</button><button class="button primary" aria-label="Next cut">›</button></div>
        </div>
      </section>
    </div>
    <div class="wb-below">
      <div class="below" style="margin-top:0">
        <section class="manifest"><h3>On this panel</h3>{on_panel}</section>
        <section class="manifest"><h3>Offcuts <small>· largest first</small></h3>{off_rows}
          <p class="help" style="margin-top:8px">{len(offs) - 5} smaller offcuts not listed. Slivers thinner than the kerf count as saw dust.</p></section>
      </div>
      {search_details()}
    </div>
    <aside class="side scroll wb-cuts" aria-label="Cut order">
      <div class="side-head"><h3>Cut order</h3><small>{ncuts} cuts</small></div>
      <p class="help">Each size is the piece to cut off, measured from the edge you cut from: set the fence or stop to it.</p>
      {cl}
    </aside>
  </div>
</main>"""
    return shell("Cutting plan", "plan", body)


def plan_overview_page() -> str:
    cards = []
    for k, p in enumerate(PLAN2["patterns"]):
        st = pattern_stats(p)
        rep = f' <small>× {p["repeat"]}</small>' if p["repeat"] > 1 else ""
        cards.append(
            f'<section class="viewer"><div class="viewer-head"><div><h3>Panel {k + 1}{rep}</h3>'
            f'<small>{st["parts"]} parts · {pct(st["waste_fraction"])} waste</small></div>'
            f'<a class="link" href="#">Cut order →</a></div>'
            f'{render_panel(p, width=540, uid=f"o{k}", labels="full", name_size=10.5, title=f"Panel {k + 1}")}</section>')
    where: dict[str, list[int]] = {}
    for k, p in enumerate(PLAN2["patterns"]):
        for l in pattern_stats(p)["part_leaves"]:
            where.setdefault(l["part_id"], []).append(k + 1)
    rows = []
    for part in INST["parts"]:
        pid = part["id"]
        cut = len(where.get(pid, []))
        rows.append(f'<tr><td>{esc(pid)}</td><td class="num">{fmt(part["length"])}</td><td class="num">{fmt(part["width"])}</td>'
                    f'<td class="c">{"↔" if part.get("grain") else "—"}</td><td class="num">{part.get("quantity", 1)}</td>'
                    f'<td class="num"><span class="ok">✓</span> {cut}</td><td>{", ".join(map(str, sorted(set(where.get(pid, [])))))}</td></tr>')
    body = f"""
<main>
  {plan_heading()}
  {plan_status()}
  <div class="workbench two">
    {sheet_nav(None)}
    <div style="min-width:0">
      <div class="overview">{"".join(cards)}
        <section class="viewer"><div class="viewer-head"><div><h3>Parts checklist</h3><small>{PARTS_TOTAL} of {PARTS_TOTAL} included</small></div></div>
          <div class="table-wrap"><table><thead><tr><th>Part</th><th class="num">Length, mm</th><th class="num">Width, mm</th>
          <th class="c">Grain</th><th class="num">Needed</th><th class="num">Cut</th><th>Panels</th></tr></thead>
          <tbody>{"".join(rows)}</tbody></table></div></section>
      </div>
      {legend()}
      {search_details()}
    </div>
  </div>
</main>"""
    return shell("Cutting plan overview", "plan", body)


# --------------------------------------------------------------------------
# Empty, waiting and problem states
# --------------------------------------------------------------------------

def state(tag: str, title: str, text: str, actions: str = "", items: list[str] | None = None) -> str:
    ul = f'<ul>{"".join(f"<li>{i}</li>" for i in items)}</ul>' if items else ""
    act = f'<div class="actions">{actions}</div>' if actions else ""
    return f'<section class="state"><div class="eyebrow">{esc(tag)}</div><h2>{title}</h2><p>{text}</p>{ul}{act}</section>'


def states_page() -> str:
    edit = '<a class="button primary" href="#">Edit job</a>'
    again = '<a class="button primary" href="#">Search again</a>'
    cards = [
        state("New job", "What are you cutting?",
              "Add each part with its finished size and how many you need, then the panels you have.",
              f'<button class="button primary">{icon("plus")}Add part</button>'),
        state("Queued", "Waiting for a free solver",
              "<b>2 jobs ahead of yours.</b> Your search starts automatically; its time limit only counts once it does.",
              '<button class="button">Cancel</button>'),
        state("Done · no plan", "Not every part fits",
              "The search ended without a way to cut every part from the panels listed.",
              edit, ["<b>A part is larger than every panel:</b> “Worktop”, 2600 × 600 mm, is longer than 2440 × 1220 mm.",
                     "<b>Too few panels:</b> leave a panel’s Quantity empty for as many as needed.",
                     "<b>Settings too strict:</b> “Not allowed” trimming with 2 stages can leave no way to place every part."]),
        state("Done · no plan · goal “Best use”", "No part fits on any panel",
              "With the goal “Best use of the panels I have”, the search cuts as many parts as fit, but none fits on the panels listed.",
              edit, ["<b>Check the units:</b> panel sizes are in millimetres (2440, not 244).",
                     "<b>Grain:</b> untick “Keep grain direction” where the look allows it, so parts can turn."]),
        state("Cancelled · no plan", "Stopped before the first plan",
              "The search was stopped before it found a plan. The first plan usually appears within a second.",
              again + '<a class="button" href="#">Edit job</a>'),
        state("Failed", "The search stopped with an error",
              "This is a fault in Sheetsolver, not in your job; the best plan found before it is kept. "
              "Details for a bug report: <code>plan 4: part “Door” overlaps an offcut at x 723.2</code>",
              again + '<button class="button">Copy details</button>'),
        state("Not found", "This plan is no longer on the server",
              "Plans are kept for an hour after the search ends. Your job is still in this browser, so you can search again.",
              again),
        state("Busy", "The solver is busy",
              "Too many jobs are waiting. Nothing was lost; your job is still here.",
              '<button class="button" disabled>Try again in 24 s</button>'),
        state("Connection lost", "Reconnecting…",
              "The search keeps running on the server. The best plan so far comes back as soon as the connection does."),
    ]
    body = f"""
<main class="narrow">
  <div class="heading"><div><div class="eyebrow">Reference</div><h1>Empty, waiting and problem states</h1>
  <p class="muted">How each job state and error from docs/API.md looks. Field problems from a 422 are shown in the job setup.</p></div></div>
  <div class="states">{"".join(cards)}</div>
</main>"""
    return shell("States", "plan", body)


# --------------------------------------------------------------------------

def main() -> None:
    errors = {
        (1, "width"): ("320.55", "width 320.55 mm has more than one decimal; lengths are in steps of 0.1 mm"),
        (6, "quantity"): ("0", "quantity must be at least 1"),
        (10, "id"): ("Rail", "the name “Rail” is already used by part 10; give each part its own name"),
    }
    pages = {
        "editor.html": editor_page(),
        "editor-errors.html": editor_page(errors),
        "solving.html": solving_page(),
        "plan-panel.html": plan_panel_page(),
        "plan-overview.html": plan_overview_page(),
        "states.html": states_page(),
    }
    for name, content in pages.items():
        (OUT / name).write_text(content)
        print("wrote", (OUT / name).relative_to(ROOT))


if __name__ == "__main__":
    main()
