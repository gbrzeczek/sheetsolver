# Field reference

The single source of truth for every field the user can set: its UI
label, its help text, unit, default, valid values, and the upstream field
it maps to.

Rules:

- The **Help text** column is the exact copy shown in the UI. It also
  becomes the doc comment on the Rust field, from which `utoipa` puts it
  into `openapi.json` and the UI reads it (see `ARCHITECTURE.md`).
  Change it here first, then in code.
- Help text says what the field means, in woodworking terms, what it does
  to the plan, and a typical value. No solver jargon.
- Only fields marked **In** in `SCOPE.md` are listed under "Fields". Later
  fields are listed at the end without help text yet.
- Help text describes upstream behaviour as confirmed against upstream's
  code and the oracle's solutions (Stage 1). The precise rules are in
  `crates/checker`.

## Conventions shown to the user

- All lengths are in **millimetres with at most one decimal** (e.g. 720,
  3.2, 18.5). More decimals are rejected with a clear message, never
  silently rounded. Internally lengths are whole tenths of a millimetre.
- Areas shown to the user are in mm² (or m² where more readable).
- A panel is drawn with its **length horizontal** and its **width
  vertical**. The panel's length is along its X axis, its width along its
  Y axis (upstream `width` = our length, upstream `height` = our width).
- A part's **length** is laid along the panel's length unless the part is
  rotated. For a part with grain, enter the length along the grain, and
  enter the panel's length along its grain too.

## Fields

### Job

| Field | UI label | Help text | Unit / values | Default | Upstream |
|---|---|---|---|---|---|
| `objective` | Goal | What the plan should optimise. **Use as few panels as possible**: cut every part, using the fewest panels; panels are used in the order listed. **Best use of the panels I have**: the panels can't fit every part, so choose which parts to cut to get the most out of them (by part value — see *Value*). **Shortest length from one panel**: you have one panel of fixed width and any length you need (e.g. a long board or a roll); cut every part using the shortest possible length. **Shortest width from one panel**: the same, but the length is fixed and the width is minimised. | `bin_packing`, `knapsack`, `open_dimension_x`, `open_dimension_y` | `bin_packing` | `objective` |
| `time_limit` | Time limit | How long the solver may search for a better plan. The first plan usually appears within a second; longer searches find plans with less waste. You can stop the search at any time and keep the best plan so far. | seconds, 1–600 | 10 | `timer.set_time_limit` |

### Panels (stock sheets)

| Field | UI label | Help text | Unit / values | Default | Upstream |
|---|---|---|---|---|---|
| `id` | Name | Your name for this panel, shown on the cutting plan (e.g. "Birch ply 18 mm"). | text | "Panel 1", … | — |
| `length` | Length | The panel's long side, as delivered, in millimetres. For a panel with grain, the side along the grain. For "Shortest length from one panel", this is the maximum length available. | mm (0.1 mm steps), ≥ 0.1 | — | `width` |
| `width` | Width | The panel's short side, as delivered, in millimetres. For "Shortest width from one panel", this is the maximum width available. | mm (0.1 mm steps), ≥ 0.1 | — | `height` |
| `quantity` | Quantity | How many panels of this size you have. Leave empty for "as many as needed". | count ≥ 1, or empty = unlimited | empty | `copies` (−1 = unlimited) |
| `cost` | Cost | Used only to compare panel sizes when several are listed: a lower total cost is preferred. Leave empty to use the panel's area. | number ≥ 0 | area | `cost` |

Only one panel type is allowed with the "Shortest length/width" goals,
and its quantity must be 1 or empty (upstream: open-dimension objectives
use a single bin).

### Parts

| Field | UI label | Help text | Unit / values | Default | Upstream |
|---|---|---|---|---|---|
| `id` | Name | Your name for this part, shown on the cutting plan and labels (e.g. "Side left"). | text | "Part 1", … | — |
| `length` | Length | The part's finished length in millimetres, before edge banding. For a part with grain, the side along the grain. | mm (0.1 mm steps), ≥ 0.1 | — | `width` |
| `width` | Width | The part's finished width in millimetres, before edge banding. | mm (0.1 mm steps), ≥ 0.1 | — | `height` |
| `quantity` | Quantity | How many identical parts you need. With "Best use of the panels I have", this is the most that will be cut; fewer may be cut if they don't fit. | count ≥ 1 | 1 | `copies` |
| `grain` | Keep grain direction | Tick this if the part must keep its grain running the same way as the panel's grain (veneered or solid-wood panels, brushed or wood-look melamine). The part is then never turned 90°. Untick it for plain MDF, chipboard or any part whose look doesn't depend on direction — turning parts usually saves material. | yes / no | no | `oriented` (`true` = no rotation) |
| `value` | Value | Only used with "Best use of the panels I have": how much you want this part, compared with the others. The plan maximises the total value of the parts it cuts. Leave empty to value parts by their area in mm² (the plan then uses as much of the panel as possible). Set every part to 1 to cut as many parts as possible. | number ≥ 0 | area (mm²) | `profit` |

### Saw and cutting

| Field | UI label | Help text | Unit / values | Default | Upstream |
|---|---|---|---|---|---|
| `kerf` | Saw blade thickness (kerf) | The width of material each cut removes, in millimetres — the thickness of your saw blade's teeth, not the blade body. Every cut between two parts uses this much material, so parts are spaced by it. Typical values: 3.2–4.4 mm for panel and table saws, 2.2–2.8 mm for track saws. Set 0 only if your dimensions already include it. | mm (0.1 mm steps), ≥ 0 | 3.2 | `cut_thickness` |
| `stages` | Cutting stages | How many times a piece is turned and cut again. **Stage 1**: full-length cuts across the whole panel, splitting it into strips. **Stage 2**: each strip is cut across into pieces. **Stage 3**: each piece is cut once more, across stage-2 cuts. More stages let parts nest more tightly, so less waste, but mean more handling at the saw. 2 suits simple strip cutting; 3 suits most panel-saw work; "Unlimited" gives the least waste with the most handling: cuts may then go either way at every stage, and the last-stage trimming setting does not apply. | `2`, `3`, `unlimited` | `3` | `number_of_stages` |
| `cut_type` | Last-stage trimming | Whether a part may be cut free from a small offcut after the last stage. **Allowed** (recommended): a strip may hold parts of different sizes; a final trimming cut separates each part from its offcut. **Not allowed**: at the last stage every part must fill its strip exactly, with no offcut — fewer cuts, more waste. **One part per strip**: every part in a last-stage strip is the same part — the simplest plans to cut, the most waste. | `non_exact`, `exact`, `homogenous` | `non_exact` | `cut_type` |
| `first_cuts` | First cuts | Which way the first, full-panel cuts run. **Either** (recommended): the solver tries both and keeps the plan that wastes less; each panel may be cut differently. The search time is shared between the two directions. **Along the length**: rip the panel into long strips first, then cut the strips across — for saws or workflows that always rip first. **Across the length**: cut the panel across into shorter strips first. Parts with grain keep their direction whichever you choose. With unlimited stages, cuts may always go either way. | `either`, `along_length`, `across_length` | `either` | `first_stage_orientation` (`any`, `horizontal`, `vertical`) |

## Later fields (help text to be written when they move to In)

| Field | Upstream |
|---|---|
| Edge trims (per side, soft/hard) | `left_trim`, `right_trim`, `bottom_trim`, `top_trim`, `TrimType` |
| Minimum offcut width | `minimum_waste_length` |
| Defects (position, size) | `defects` |
| Cut through defects | `cut_through_defects` |
| Min/max strip width, max number of strips | `minimum_distance_*_cuts`, `maximum_distance_*_cuts`, `maximum_number_*_cuts` |
| Keep the leftover in one piece | objective `bin_packing_with_leftovers` |
| Choose the cheapest panel sizes | objective `variable_sized_bin_packing` |
