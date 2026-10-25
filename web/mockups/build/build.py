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
    "Part 9": "Divider",
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
        f'<line x1="0" y1="0" x2="0" y2="6" class="p-hatch"/></pattern>'
        f'<pattern id="{uid}-grain" width="40" height="5" patternUnits="userSpaceOnUse">'
        f'<path d="M0 2.5 Q10 1.6 20 2.5 T40 2.5" class="p-grain"/></pattern></defs>')
    # The panel itself shows through wherever the saw removes material.
    out.append(f'<rect x="{ml}" y="{mt}" width="{L * s:.2f}" height="{W * s:.2f}" class="p-kerf"/>')

    for leaf in leaves(root):
        x, y, w, h = box(leaf)
        kind = leaf["kind"]
        if kind == "part":
            part = PARTS[leaf["part_id"]]
            out.append(f'<rect x="{x:.2f}" y="{y:.2f}" width="{w:.2f}" height="{h:.2f}" class="p-part"/>')
            if part.get("grain"):
                out.append(f'<rect x="{x:.2f}" y="{y:.2f}" width="{w:.2f}" height="{h:.2f}" '
                           f'fill="url(#{uid}-grain)" class="p-nostroke"/>')
            if leaf.get("rotated") and labels != "none" and w > 30 and h > 24:
                out.append(f'<g transform="translate({x + w - 16:.2f} {y + h - 16:.2f})" class="p-rot">{ROTATE_ICON}</g>')
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
                out.append(stacked_label("Leftover", f'{fmt(leaf["length"])} × {fmt(leaf["width"])}',
                                         x, y, w, h, name_size, dim_size, "p-leftover-text")
                           if labels == "full" else
                           stacked_label("Leftover", None, x, y, w, h, name_size, dim_size, "p-leftover-text"))

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
.p-kerf { fill: var(--plan-kerf); }
.p-edge { fill: none; stroke: var(--text); stroke-opacity: .55; stroke-width: 1; }
.p-part { fill: var(--plan-part); stroke: var(--plan-part-stroke); stroke-width: .75; }
.p-nostroke { stroke: none; }
.p-grain { fill: none; stroke: var(--plan-grain); stroke-width: 1; }
.p-offcut-bg { fill: var(--plan-offcut); }
.p-hatch { stroke: var(--plan-hatch); stroke-width: 1.4; }
.p-offcut { stroke: none; }
.p-leftover { fill: var(--plan-leftover); stroke: var(--plan-leftover-stroke); stroke-width: 1.5; stroke-dasharray: 5 4; }
.p-part-text { fill: var(--plan-part-text); }
.p-name { font-weight: 600; }
.p-dim { font-weight: 450; }
.p-part-text .p-dim { fill-opacity: .78; }
.p-offcut-text { fill: var(--plan-offcut-text); }
.p-leftover-text { fill: var(--text-2); }
.p-rot { stroke: var(--plan-part-stroke); }
.p-ruler line { stroke: var(--plan-ruler); stroke-width: 1; }
.p-ruler text { fill: var(--text-2); font-size: 11px; font-weight: 500; }
.p-ruler .p-ruler-weak { fill: var(--text-3); }
.p-ruler-bg { fill: var(--surface); }
.p-badge circle { fill: var(--plan-badge); stroke: var(--surface); stroke-width: 1.5; }
.p-badge text { fill: var(--plan-badge-text); font-size: 10px; font-weight: 700; }
.p-badge.on circle { fill: var(--accent); }
.p-badge.on text { fill: var(--on-accent); }
.p-badge.done { opacity: .35; }
.p-cutline { fill: var(--accent); }
.p-result { fill: var(--accent-tint); stroke: var(--accent); stroke-width: 2; }
"""


# --------------------------------------------------------------------------
# Page shell and shared pieces
# --------------------------------------------------------------------------

CSS = (Path(__file__).parent / "styles.css").read_text() + PLAN_CSS

LOGO = ('<svg width="26" height="26" viewBox="0 0 26 26" aria-hidden="true">'
        '<rect x="1" y="1" width="24" height="24" rx="6.5" fill="var(--accent)"/>'
        '<path d="M5.5 14.5h15M12 14.5v6M15.5 5.5v9" stroke="#fff" stroke-width="1.8" '
        'stroke-linecap="round"/></svg>')


def icon(name: str, size: int = 16) -> str:
    paths = {
        "plus": '<path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
        "trash": '<path d="M3.5 4.5h9M6.5 4.5V3h3v1.5M4.8 4.5l.6 8.5h5.2l.6-8.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>',
        "alert": '<path d="M8 1.8 15 14H1z" fill="currentColor"/><path d="M8 6v4" stroke="#fff" stroke-width="1.6" stroke-linecap="round"/><circle cx="8" cy="12" r=".9" fill="#fff"/>',
        "chev": '<path d="M6 3.5 10.5 8 6 12.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>',
        "back": '<path d="M10 3.5 5.5 8l4.5 4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>',
        "stop": '<rect x="4" y="4" width="8" height="8" rx="1.5" fill="currentColor"/>',
        "play": '<path d="M5 3.2v9.6L13 8z" fill="currentColor"/>',
        "check": '<path d="M3.5 8.5 6.5 11.5 12.5 4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>',
        "clock": '<circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M8 4.6V8l2.4 1.6" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>',
        "edit": '<path d="M10.5 2.5 13.5 5.5 6 13H3v-3z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/>',
    }
    return (f'<svg width="{size}" height="{size}" viewBox="0 0 16 16" aria-hidden="true">'
            f'{paths[name]}</svg>')


def shell(title: str, step: int, body: str, actions: str = "") -> str:
    def st(n, label):
        cur = ' aria-current="step"' if n == step else ""
        cls = ' class="done"' if n < step else ""
        mark = icon("check", 11) if n < step else str(n)
        return f'<a href="#"{cur}{cls}><span class="dot">{mark}</span>{label}</a>'

    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{esc(title)} · Sheetsolver</title>
<style>{CSS}</style>
</head>
<body>
<header class="topbar">
  <div class="brand">{LOGO}<span>Sheetsolver</span></div>
  <nav class="steps" aria-label="Progress">{st(1, "Job")}{st(2, "Search")}{st(3, "Plan")}</nav>
  <div class="topbar-actions">{actions}</div>
</header>
{body}
</body>
</html>
"""


# --------------------------------------------------------------------------
# Instance editor
# --------------------------------------------------------------------------

def text_input(value, *, cls="", placeholder="", suffix=None, label_text=None) -> str:
    v = f' value="{esc(str(value))}"' if value not in (None, "") else ""
    ph = f' placeholder="{esc(placeholder)}"' if placeholder else ""
    aria = f' aria-label="{esc(label_text)}"' if label_text else ""
    inp = f'<input class="input {cls}"{v}{ph}{aria} readonly>'
    if suffix:
        return f'<div class="input-wrap">{inp}<span class="suffix">{suffix}</span></div>'
    return inp


def parts_table(errors: dict, focus: tuple[int, str] | None, show_value: bool) -> str:
    cols = [("id", "w-name", None), ("length", "w-len", "mm"), ("width", "w-len", "mm"),
            ("quantity", "w-qty", None), ("grain", "w-chk c", None)]
    if show_value:
        cols.append(("value", "w-qty", None))
    head = ['<th></th>']
    for f, cls, unit in cols:
        u = f' <span class="unit">{unit}</span>' if unit else ""
        fc = " focus-col" if focus and focus[1] == f else ""
        head.append(f'<th class="{cls}{fc}">{esc(label("parts", f))}{u}</th>')
    head.append('<th class="w-del"></th>')
    rows = []
    for i, p in enumerate(INST["parts"]):
        cells = [f'<td class="idx">{i + 1}</td>']
        for f, cls, unit in cols:
            key = (i, f)
            state = "invalid" if key in errors else ("focus" if focus == key else "")
            lab = f'{label("parts", f)}, row {i + 1}'
            if f == "grain":
                on = " on" if p.get("grain") else ""
                cells.append(f'<td class="c"><div class="check-cell"><span class="check{on}" role="checkbox" '
                             f'aria-checked="{"true" if on else "false"}" aria-label="{esc(lab)}"></span></div></td>')
            elif f == "value":
                cells.append(f'<td>{text_input(p.get("value"), placeholder="Area", cls=state, label_text=lab)}</td>')
            else:
                val = errors.get(key, (None, None))[0] or (fmt(p[f]) if f in ("length", "width") else p.get(f, ""))
                cells.append(f'<td>{text_input(val, cls=state, label_text=lab)}</td>')
        cells.append(f'<td><button class="icon-btn" aria-label="Remove row {i + 1}">{icon("trash")}</button></td>')
        rows.append(f'<tr class="row">{"".join(cells)}</tr>')
        row_errs = [(f, errors[(i, f)][1]) for f, _, _ in cols if (i, f) in errors]
        for f, msg in row_errs:
            rows.append(f'<tr class="err-row"><td></td><td colspan="{len(cols) + 1}"><div class="cell-error" id="err-{i}-{f}">'
                        f'{icon("alert", 14)}<span><strong>{esc(label("parts", f))}:</strong> {esc(msg)}</span></div></td></tr>')
    return (f'<div class="table-wrap"><table class="edit"><thead><tr>{"".join(head)}</tr></thead>'
            f'<tbody>{"".join(rows)}</tbody></table></div>')


def guide(section: str, fields: list[str], active: str | None = None,
          muted: dict[str, str] | None = None, title: str = "About these fields") -> str:
    muted = muted or {}
    items = []
    for f in fields:
        cls = "item" + (" active" if f == active else "") + (" muted" if f in muted else "")
        note = f' <span class="chip">{esc(muted[f])}</span>' if f in muted else ""
        wide = " wide" if len(FIELDS[(section, f)]["help"]) > 330 else ""
        items.append(f'<div class="{cls}{wide}"><dt>{esc(label(section, f))}{note}</dt>'
                     f'<dd>{help_html(section, f)}</dd></div>')
    return f'<div class="guide"><h3>{esc(title)}</h3><dl>{"".join(items)}</dl></div>'


def panels_table() -> str:
    head = ('<th></th><th class="w-name">Name</th><th class="w-len">Length <span class="unit">mm</span></th>'
            '<th class="w-len">Width <span class="unit">mm</span></th><th class="w-qty">Quantity</th>'
            '<th class="w-qty">Cost</th><th class="w-del"></th>')
    rows = []
    for i, p in enumerate(INST["panels"]):
        rows.append(
            f'<tr class="row"><td class="idx">{i + 1}</td><td>{text_input(p["id"], label_text="Name")}</td>'
            f'<td>{text_input(fmt(p["length"]), label_text="Length")}</td><td>{text_input(fmt(p["width"]), label_text="Width")}</td>'
            f'<td>{text_input(p.get("quantity"), placeholder="Unlimited", label_text="Quantity")}</td>'
            f'<td>{text_input(p.get("cost"), placeholder="Panel area", label_text="Cost")}</td>'
            f'<td><button class="icon-btn" aria-label="Remove panel">{icon("trash")}</button></td></tr>')
    return (f'<div class="table-wrap"><table class="edit"><thead><tr>{head}</tr></thead>'
            f'<tbody>{"".join(rows)}</tbody></table></div>')


def choice_list(section: str, field: str, current: str) -> str:
    out = []
    for o in options(section, field):
        on = " on" if o == current else ""
        out.append(f'<label class="choice{on}"><span class="radio"></span><span>{esc(o)}</span></label>')
    return f'<div class="choices" role="radiogroup" aria-label="{esc(label(section, field))}">{"".join(out)}</div>'


def segmented(opts: list[str], current: str, aria: str) -> str:
    items = "".join(f'<label class="{"on" if o == current else ""}">{esc(o)}</label>' for o in opts)
    return f'<div class="segmented" role="radiogroup" aria-label="{esc(aria)}">{items}</div>'


def field_block(section: str, field: str, control: str, current: str | None = None,
                unit: str | None = None, show_label: bool = True) -> str:
    u = f' <span class="unit">{unit}</span>' if unit else ""
    lab = f'<span class="field-label">{esc(label(section, field))}{u}</span>' if show_label else ""
    return (f'<div class="field">{lab}'
            f'{control}<p class="help">{help_html(section, field, current)}</p></div>')


def settings_column() -> str:
    goal_opts = options("job", "objective")
    goal = field_block("job", "objective", choice_list("job", "objective", goal_opts[0]), goal_opts[0],
                       show_label=False)
    kerf = field_block("saw", "kerf", f'<div class="input-short">{text_input(fmt(KERF), suffix="mm", label_text="Kerf")}</div>')
    stages = field_block("saw", "stages", segmented(["2", "3", "Unlimited"], "3", label("saw", "stages")))
    ct_opts = options("saw", "cut_type")
    cut_type = field_block("saw", "cut_type", segmented(ct_opts, ct_opts[0], label("saw", "cut_type")), ct_opts[0])
    fc_opts = options("saw", "first_cuts")
    first = field_block("saw", "first_cuts", segmented(fc_opts, fc_opts[1], label("saw", "first_cuts")), fc_opts[1])
    tl = field_block("job", "time_limit", f'<div class="input-short">{text_input("10", suffix="s", label_text="Time limit")}</div>')
    return f"""
<div class="stack">
  <section class="card"><div class="card-head"><h2>{esc(label("job", "objective"))}</h2></div><div class="card-body">{goal}</div></section>
  <section class="card"><div class="card-head"><h2>Saw and cutting</h2></div><div class="card-body">{kerf}{stages}{cut_type}{first}</div></section>
  <section class="card"><div class="card-head"><h2>Search</h2></div><div class="card-body">{tl}</div></section>
  <section class="card cta hide-tablet">
    <a class="btn btn-primary btn-lg btn-block" href="#">{icon("play")}Find cutting plan</a>
    <div class="summary"><span>{PARTS_TOTAL} parts · {len(INST["parts"])} sizes</span><span>1 panel size</span></div>
  </section>
</div>"""


def editor_page(errors: dict | None = None) -> str:
    errors = errors or {}
    focus = None if errors else (2, "length")
    goal_note = "Shown with the goal " + options("job", "objective")[1]
    banner = ""
    if errors:
        items = []
        for (i, f), (_, msg) in errors.items():
            items.append(f'<li><a href="#err-{i}-{f}">Part {i + 1} · {esc(label("parts", f))}</a> — {esc(msg)}</li>')
        banner = (f'<div class="banner banner-error" role="alert"><span class="icon">{icon("alert", 22)}</span>'
                  f'<div><h3>The job cannot be solved as entered.</h3>'
                  f'<p>{len(errors)} problems. Fix the highlighted fields, then find the plan again.</p>'
                  f'<ul>{"".join(items)}</ul></div></div>')
    parts_meta = f'{PARTS_TOTAL} parts in {len(INST["parts"])} sizes'
    body = f"""
<main class="page">
  <div class="page-head">
    <div><h1>Cutting job</h1><p class="sub">List the parts you need and the panels you have. Every field explains itself below it.</p></div>
  </div>
  {banner}
  <div class="layout-editor">
    <div class="stack">
      <section class="card" aria-labelledby="parts-h">
        <div class="card-head"><h2 id="parts-h">Parts</h2><span class="meta">{parts_meta}</span></div>
        <div class="card-body">
          {parts_table(errors, focus, show_value=False)}
          <div class="table-foot"><button class="btn btn-tinted">{icon("plus")}Add part</button>
            <span class="meta">Lengths in millimetres, at most one decimal (e.g. 720, 3.2, 18.5).</span></div>
          {guide("parts", ["id", "length", "width", "quantity", "grain", "value"], active=focus[1] if focus else None,
                 muted={"value": goal_note})}
        </div>
      </section>
      <section class="card" aria-labelledby="panels-h">
        <div class="card-head"><h2 id="panels-h">Panels</h2><span class="meta">The sheets you cut from</span></div>
        <div class="card-body">
          {panels_table()}
          <div class="table-foot"><button class="btn btn-tinted">{icon("plus")}Add panel size</button></div>
          {guide("panels", ["id", "length", "width", "quantity", "cost"])}
        </div>
      </section>
    </div>
    <aside class="sticky-col">{settings_column()}</aside>
  </div>
</main>
<div class="cta-bar show-tablet"><span class="meta">{PARTS_TOTAL} parts · 1 panel size</span>
  <a class="btn btn-primary btn-lg" href="#">{icon("play")}Find cutting plan</a></div>
"""
    return shell("Cutting job", 1, body)


# --------------------------------------------------------------------------
# Solving in progress
# --------------------------------------------------------------------------

def pct(f: float) -> str:
    return f"{f * 100:.1f}%"


def solving_page() -> str:
    elapsed, limit = 3.4, 10
    thumbs = []
    for k, p in enumerate(PLAN2["patterns"]):
        st = pattern_stats(p)
        thumbs.append(
            f'<figure class="card figure" style="margin:0"><div class="figure-head" style="margin-bottom:10px">'
            f'<div><h2 style="font:var(--t-headline)">Panel {k + 1}</h2><p class="sub">{st["parts"]} parts · waste {pct(st["waste_fraction"])}</p></div></div>'
            f'{render_panel(p, width=340, uid=f"s{k}", labels="names", name_size=11, title=f"Panel {k + 1} of the best plan so far")}</figure>')
    delta = (M1["waste_fraction"] - M2["waste_fraction"]) * 100
    body = f"""
<main class="page">
  <div class="page-head">
    <div>
      <span class="status-pill status-running"><span class="pulse"></span>Searching</span>
      <h1 style="margin-top:12px">Looking for a better plan</h1>
      <p class="sub">The best plan so far is shown below and is replaced as soon as a better one is found.
      Stop at any time to keep it.</p>
    </div>
    <div style="display:flex;gap:8px"><button class="btn">{icon("stop", 14)}Stop and keep this plan</button>
      <a class="btn btn-primary" href="#">Open plan{icon("chev", 14)}</a></div>
  </div>

  <section class="card card-body" style="margin-bottom:var(--s-5)">
    <div style="display:flex;justify-content:space-between;align-items:baseline;margin-bottom:10px">
      <span style="font:var(--t-headline)" class="num">{elapsed:.1f} s <span style="color:var(--text-2);font-weight:400">of {limit} s</span></span>
      <span style="color:var(--text-2);font:var(--t-help)" class="num">Time limit {limit} s · about {limit - elapsed:.0f} s left</span>
    </div>
    <div class="progress" role="progressbar" aria-valuenow="{elapsed}" aria-valuemin="0" aria-valuemax="{limit}" aria-label="Search time"><span style="width:{elapsed / limit * 100:.0f}%"></span></div>
  </section>

  <div class="metrics" style="margin-bottom:var(--s-5)">
    <div class="card metric"><div class="label">Panels used</div><div class="value">{M2["panels_used"]}</div><div class="note">2440 × 1220 Birch ply 18 mm</div></div>
    <div class="card metric"><div class="label">Waste</div><div class="value">{pct(M2["waste_fraction"])}</div><div class="note good">↓ {delta:.1f} points since plan 1</div></div>
    <div class="card metric"><div class="label">Parts cut</div><div class="value">{M2["parts_cut"]} <small>of {PARTS_TOTAL}</small></div><div class="note">Every part fits</div></div>
    <div class="card metric"><div class="label">Plans found</div><div class="value">2</div><div class="note">Latest after {FOUND_AFTER[2]:.1f} s</div></div>
  </div>

  <div class="layout-editor">
    <section>
      <div class="card-head" style="padding:0 0 var(--s-3)"><h2>Best plan so far</h2><span class="meta">Plan 2</span></div>
      <div class="thumb-grid">{"".join(thumbs)}</div>
    </section>
    <aside class="card">
      <div class="card-head"><h2>Plans found</h2></div>
      <div class="card-body">
        <ul class="timeline">
          <li class="best"><span class="n">2</span><div><div class="t1">{M2["panels_used"]} panels · {pct(M2["waste_fraction"])} waste</div>
            <div class="t2 num">Found after {FOUND_AFTER[2]:.2f} s</div></div><span class="delta">Best</span></li>
          <li><span class="n">1</span><div><div class="t1">{M1["panels_used"]} panels · {pct(M1["waste_fraction"])} waste</div>
            <div class="t2 num">Found after {FOUND_AFTER[1]:.2f} s</div></div><span></span></li>
        </ul>
        <p class="help" style="margin-top:var(--s-4)">{help_html("job", "time_limit")}</p>
      </div>
    </aside>
  </div>
</main>
"""
    return shell("Searching", 2, body)


# --------------------------------------------------------------------------
# Plan viewer
# --------------------------------------------------------------------------

def plan_header(view: str) -> str:
    left = PLAN2["patterns"][-1]
    lo = pattern_stats(left)["leftovers"][0]
    seg = (f'<div class="segmented" style="max-width:320px" role="tablist">'
           f'<label class="{"on" if view == "overview" else ""}">Overview</label>'
           f'<label class="{"on" if view == "panel" else ""}">Panel by panel</label></div>')
    return f"""
  <div class="page-head">
    <div>
      <span class="status-pill status-done"><span class="pulse"></span>Search finished · time limit reached</span>
      <h1 style="margin-top:12px">Cutting plan</h1>
      <p class="sub num">Plan 2, found after {FOUND_AFTER[2]:.1f} s of a 10 s search · 3 stages · kerf {fmt(KERF)} mm.
        A longer search may save material.</p>
    </div>
    <div style="display:flex;gap:8px"><a class="btn" href="#">{icon("edit", 14)}Edit job</a>
      <a class="btn btn-tinted" href="#">{icon("play", 14)}Search longer</a></div>
  </div>
  <div class="metrics" style="margin-bottom:var(--s-6)">
    <div class="card metric"><div class="label">Panels</div><div class="value">{M2["panels_used"]}</div><div class="note">Birch ply 18 mm, 2440 × 1220</div></div>
    <div class="card metric"><div class="label">Parts cut</div><div class="value">{M2["parts_cut"]} <small>of {PARTS_TOTAL}</small></div><div class="note">Every part is in the plan</div></div>
    <div class="card metric"><div class="label">Waste</div><div class="value">{pct(M2["waste_fraction"])}</div><div class="note num">{fmt_area_m2(M2["waste_area"])} of offcuts and saw dust</div></div>
    <div class="card metric"><div class="label">Leftover</div><div class="value num" style="font-size:24px">{fmt(lo["length"])} × {fmt(lo["width"])}</div><div class="note">Reusable strip on panel 3</div></div>
  </div>
  <div style="margin-bottom:var(--s-5)">{seg}</div>"""


def legend() -> str:
    def sw(cls, extra=""):
        return (f'<svg width="18" height="12" aria-hidden="true" class="plan-svg"><rect x=".5" y=".5" width="17" height="11" rx="2" class="{cls}" {extra}/></svg>')
    return (f'<div class="legend">'
            f'<span>{sw("p-part")}Part</span>'
            f'<span><svg width="18" height="12" class="plan-svg" aria-hidden="true"><rect x=".5" y=".5" width="17" height="11" rx="2" class="p-part"/><path d="M1 6 Q5 4.8 9 6 T17 6" class="p-grain" style="stroke:var(--plan-part-stroke)"/></svg>Keeps grain direction</span>'
            f'<span><svg width="14" height="14" viewBox="0 0 12 12" class="plan-svg p-rot" aria-hidden="true">{ROTATE_ICON}</svg>Turned 90°</span>'
            f'<span><svg width="18" height="12" class="plan-svg" aria-hidden="true"><defs><pattern id="lg-h" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="4" height="4" class="p-offcut-bg"/><line x1="0" y1="0" x2="0" y2="4" class="p-hatch"/></pattern></defs><rect x=".5" y=".5" width="17" height="11" rx="2" fill="url(#lg-h)"/></svg>Offcut</span>'
            f'<span>{sw("p-leftover")}Leftover (reusable)</span>'
            f'<span><svg width="18" height="12" class="plan-svg" aria-hidden="true"><rect x="8" y="0" width="2" height="12" class="p-kerf"/></svg>Saw cut ({fmt(KERF)} mm kerf)</span>'
            f'</div>')


def cut_list(pattern: dict, highlight: int, done_before: int) -> str:
    steps = cut_steps(pattern)
    out, group = [], None
    for st in steps:
        if st["group"] != group:
            group = st["group"]
            if group == "panel":
                title, meta = "Panel into strips", "first cuts, along the length"
            else:
                node = next(n for n in iter_nodes(pattern["root"]) if st["names"].get(id(n)) == group)
                title, meta = group, f'{fmt(node["length"])} × {fmt(node["width"])}'
            out.append(f'<li class="group"><span>{esc(title)}</span><span class="num">{meta}</span></li>')
        cls = "cut" + (" on" if st["n"] == highlight else "") + (" done" if st["n"] < done_before else "")
        dirw = "along the length" if st["direction"] == "along" else "across the length"
        if st["trim"]:
            dirw = "trim, " + dirw
        rest = f'<div class="r">leaves {st["rest"]}</div>' if st["rest"] else ""
        cur = ' aria-current="step"' if st["n"] == highlight else ""
        out.append(f'<li class="{cls}"{cur}><span class="n">{st["n"]}</span><div>'
                   f'<div class="m">{fmt(st["measure"])} mm<em>{dirw}</em></div>'
                   f'<div class="r">→ {st["result"]}</div>{rest}</div></li>')
    return f'<ol class="cutlist">{"".join(out)}</ol>', len(steps)


def iter_nodes(n):
    yield n
    for c in n.get("children", []):
        yield from iter_nodes(c)


def plan_panel_page() -> str:
    k = 0
    pat = PLAN2["patterns"][k]
    st = pattern_stats(pat)
    highlight = 7
    rail = []
    for j, p in enumerate(PLAN2["patterns"]):
        s2 = pattern_stats(p)
        on = " on" if j == k else ""
        cur = ' aria-current="page"' if on else ""
        rail.append(f'<a class="thumb{on}" href="#"{cur}>'
                    f'{render_panel(p, width=150, uid=f"r{j}", labels="none", title=f"Panel {j + 1}")}'
                    f'<div class="tt"><span>Panel {j + 1}</span><span>{s2["parts"]} parts</span></div></a>')
    figure = render_panel(pat, width=830, uid="main", labels="full", badges=True, rulers=True,
                          highlight=highlight, done_before=highlight, name_size=12,
                          title="Panel 1 of 3: Birch ply 18 mm, 2440 by 1220 mm")
    cl, ncuts = cut_list(pat, highlight, highlight)

    parts_rows = {}
    for l in st["part_leaves"]:
        r = parts_rows.setdefault(l["part_id"], {"n": 0, "leaf": l})
        r["n"] += 1
    prow = "".join(
        f'<tr><td>{esc(pid)}</td><td class="num">{fmt(PARTS[pid]["length"])} × {fmt(PARTS[pid]["width"])}</td>'
        f'<td class="r num">{r["n"]}</td></tr>' for pid, r in parts_rows.items())
    offs = sorted((o for o in st["offcuts"] if min(o["length"], o["width"]) >= KERF), key=area, reverse=True)
    orow = "".join(
        f'<tr><td class="num">{fmt(o["length"])} × {fmt(o["width"])}</td><td class="r num muted">{area(o) / 1e6:.3f} m²</td></tr>'
        for o in offs[:6])
    more = len(offs) - 6
    body = f"""
<main class="page page-wide">
  {plan_header("panel")}
  <div class="plan-layout">
    <div class="stack pl-main" style="gap:var(--s-5)">
      <nav class="rail" aria-label="Panels">{"".join(rail)}</nav>
      <section class="card figure">
        <div class="figure-head">
          <div><h2>Panel 1 <span style="color:var(--text-2);font-weight:400">of 3</span></h2>
            <p class="sub num">Birch ply 18 mm · 2440 × 1220 · {st["parts"]} parts · waste {pct(st["waste_fraction"])} · first cuts along the length</p></div>
          <div class="toggles"><span class="toggle on">{icon("check", 12)}Sizes</span><span class="toggle on">{icon("check", 12)}Cut order</span></div>
        </div>
        {figure}
        {legend()}
      </section>
    </div>
    <aside class="card scroll-card pl-cuts" aria-label="Cut order">
      <div class="card-head"><h2>Cut order</h2><span class="meta num">{ncuts} cuts</span></div>
      <p class="help" style="padding:0 20px">Each size is the piece to cut off, measured from the edge you cut from: set your fence or stop to it. Tap a cut to find it on the drawing.</p>
      {cl}
    </aside>
    <div class="pl-tables">
      <div class="overview-grid">
        <section class="card"><div class="card-head"><h2 style="font:var(--t-headline)">Parts on this panel</h2></div>
          <div class="card-body"><table class="data"><thead><tr><th>Part</th><th>Size</th><th class="r">Qty</th></tr></thead><tbody>{prow}</tbody></table></div></section>
        <section class="card"><div class="card-head"><h2 style="font:var(--t-headline)">Offcuts</h2><span class="meta">largest first</span></div>
          <div class="card-body"><table class="data"><thead><tr><th>Size</th><th class="r">Area</th></tr></thead><tbody>{orow}</tbody></table>
          <p class="help">{more} smaller offcuts not listed. Slivers thinner than the kerf are counted as saw dust.</p></div></section>
      </div>
    </div>
  </div>
</main>"""
    return shell("Cutting plan", 3, body)


def plan_overview_page() -> str:
    cards = []
    for k, p in enumerate(PLAN2["patterns"]):
        st = pattern_stats(p)
        rep = f' <span class="chip">× {p["repeat"]}</span>' if p["repeat"] > 1 else ""
        cards.append(
            f'<a class="card figure" href="#" style="text-decoration:none;color:inherit;display:block">'
            f'<div class="figure-head" style="margin-bottom:12px"><div><h2 style="font:var(--t-headline)">Panel {k + 1}{rep}</h2>'
            f'<p class="sub num">{st["parts"]} parts · waste {pct(st["waste_fraction"])}</p></div>'
            f'<span class="btn btn-plain" style="height:auto">Cut order{icon("chev", 14)}</span></div>'
            f'{render_panel(p, width=560, uid=f"o{k}", labels="full", name_size=10.5, title=f"Panel {k + 1}")}</a>')
    # Parts summary: needed vs cut and where.
    where: dict[str, list[int]] = {}
    for k, p in enumerate(PLAN2["patterns"]):
        for l in pattern_stats(p)["part_leaves"]:
            where.setdefault(l["part_id"], []).append(k + 1)
    rows = []
    for part in INST["parts"]:
        pid = part["id"]
        panels = sorted(set(where.get(pid, [])))
        cut = len(where.get(pid, []))
        rows.append(f'<tr><td>{esc(pid)}</td><td class="num">{fmt(part["length"])} × {fmt(part["width"])}</td>'
                    f'<td class="muted">{"Yes" if part.get("grain") else "—"}</td>'
                    f'<td class="r num">{part.get("quantity", 1)}</td><td class="r num">{cut}</td>'
                    f'<td class="num muted">{", ".join(map(str, panels))}</td></tr>')
    body = f"""
<main class="page page-wide">
  {plan_header("overview")}
  <div class="overview-grid" style="margin-bottom:var(--s-6)">{"".join(cards)}
    <section class="card"><div class="card-head"><h2 style="font:var(--t-headline)">Parts</h2><span class="meta">{PARTS_TOTAL} of {PARTS_TOTAL} cut</span></div>
      <div class="card-body"><table class="data"><thead><tr><th>Part</th><th>Size</th><th>Grain</th><th class="r">Needed</th><th class="r">Cut</th><th>Panels</th></tr></thead>
      <tbody>{"".join(rows)}</tbody></table></div></section>
  </div>
  <section class="card card-body">{legend()}</section>
</main>"""
    return shell("Cutting plan overview", 3, body)


# --------------------------------------------------------------------------
# Empty, waiting and problem states
# --------------------------------------------------------------------------

ART_EMPTY = ('<svg width="96" height="72" viewBox="0 0 96 72" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.6">'
             '<rect x="4" y="8" width="88" height="56" rx="6"/><path d="M4 36h88M40 8v28M64 36v28" stroke-dasharray="4 4"/></svg>')
ART_NOPLAN = ('<svg width="96" height="72" viewBox="0 0 96 72" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.6">'
              '<rect x="4" y="16" width="60" height="44" rx="6"/><rect x="40" y="6" width="52" height="34" rx="4" stroke-dasharray="4 4"/></svg>')


def state_card(tag: str, art: str, title: str, text: str, actions: str, extra: str = "") -> str:
    return (f'<section class="card"><div class="state-tag">{esc(tag)}</div><div class="state">'
            f'<div class="art">{art}</div><h2>{title}</h2><p>{text}</p>{extra}'
            f'<div class="actions">{actions}</div></div></section>')


def states_page() -> str:
    big = max(INST["parts"], key=lambda p: p["length"])
    cards = [
        state_card("New job · no parts yet", ART_EMPTY, "Start with the parts you need",
                   "Add each part with its finished size and how many you need. Then add the panels you have, "
                   "check the saw settings, and find a cutting plan.",
                   f'<button class="btn btn-primary">{icon("plus")}Add part</button>'),
        state_card("Job state: queued", '<span class="status-pill status-running"><span class="pulse"></span>Waiting</span>',
                   "Waiting for a free solver",
                   "<b>2 jobs ahead of yours.</b> Yours starts automatically; the time limit only counts once it does.",
                   f'<button class="btn">{icon("stop", 14)}Cancel</button>'),
        state_card("Job state: done · no plan", ART_NOPLAN, "No plan cuts every part",
                   "The search finished without finding a way to cut every part from the panels listed. Common causes:",
                   f'<a class="btn btn-primary" href="#">{icon("edit", 14)}Edit job</a>',
                   f'<ul class="hint-list"><li>{icon("alert", 14)}<span><b>A part is larger than every panel.</b> '
                   f'“Worktop”, 2600 × 600 mm, is longer than every panel (2440 × 1220 mm).</span></li>'
                   f'<li>{icon("alert", 14)}<span><b>Too few panels.</b> Leave a panel’s Quantity empty for “as many as needed”.</span></li>'
                   f'<li>{icon("alert", 14)}<span><b>Settings too strict.</b> “Not allowed” trimming with 2 stages can leave no way to place every part.</span></li></ul>'),
        state_card("Job state: done · no plan · goal “Best use”", ART_NOPLAN, "No part fits on any panel",
                   "With the goal “Best use of the panels I have”, the search cuts as many parts as fit — but none of them fits on the panels listed. Common causes:",
                   f'<a class="btn btn-primary" href="#">{icon("edit", 14)}Edit job</a>',
                   f'<ul class="hint-list"><li>{icon("alert", 14)}<span><b>Every part is larger than the panels.</b> '
                   f'Check that the panel sizes are in millimetres (2440, not 244).</span></li>'
                   f'<li>{icon("alert", 14)}<span><b>Grain keeps parts from turning.</b> Untick “Keep grain direction” where the look allows it.</span></li></ul>'),
        state_card("Job state: cancelled · no plan", ART_NOPLAN, "Stopped before the first plan",
                   "The search was stopped before it found any plan. Search again and let it run a little longer — the first plan usually appears within a second.",
                   f'<button class="btn btn-primary">{icon("play")}Search again</button><a class="btn" href="#">Edit job</a>'),
        state_card("Job state: failed", f'<span style="color:var(--danger)">{icon("alert", 40)}</span>',
                   "The search stopped with an error",
                   "This is a fault in Sheetsolver, not in your job. The best plan found before it is kept if there was one. "
                   "Details for a bug report: <code style=\"font-size:13px\">plan 4: part “Door” overlaps an offcut at x 723.2</code>",
                   f'<button class="btn btn-primary">{icon("play")}Search again</button><button class="btn">Copy details</button>'),
        state_card("HTTP 404 · job expired", ART_EMPTY, "This plan is no longer on the server",
                   "Plans are kept for an hour after the search ends. Your job is still saved in this browser, so you can search again.",
                   f'<button class="btn btn-primary">{icon("play")}Search again</button>'),
        state_card("HTTP 503 · busy", f'<span style="color:var(--text-3)">{icon("clock", 44)}</span>', "The solver is busy",
                   "Too many jobs are waiting. Nothing was lost — your job is still here.",
                   f'<button class="btn" disabled aria-disabled="true">Try again in 24 s</button>'),
        state_card("Event stream lost", '<span class="status-pill" style="background:var(--fill);color:var(--text-2)"><span class="pulse" style="background:var(--text-3);box-shadow:none"></span>Reconnecting…</span>',
                   "Connection lost",
                   "The search keeps running on the server. The best plan so far reappears as soon as the connection is back.",
                   ""),
    ]
    _ = big
    body = f"""
<main class="page">
  <div class="page-head"><div><h1>Empty, waiting and problem states</h1>
  <p class="sub">How each job state and error from docs/API.md looks. Field-level 422 errors are shown on the job page itself.</p></div></div>
  <div class="state-grid">{"".join(cards)}</div>
</main>"""
    return shell("States", 2, body)


# --------------------------------------------------------------------------

def main() -> None:
    errors = {
        (1, "width"): ("320.55", "length 320.55 mm has more than one decimal; lengths are in steps of 0.1 mm"),
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
