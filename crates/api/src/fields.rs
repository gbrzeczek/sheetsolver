//! What `openapi.json` says about each field the woodworker fills in,
//! beyond its doc comment: its UI label, short help, default, bounds and
//! the names of its options.
//!
//! The labels, short help and option names are those of `docs/FIELDS.md`; the
//! defaults and bounds are what the server applies (`model`'s serde
//! defaults, `Instance::validate`, the time limit). Tests in
//! `tests/openapi.rs` hold the table to both. utoipa cannot put these on
//! a field whose schema is a reference, so `openapi.rs` adds them to the
//! generated document from here.

/// An input field and what to add to its property in `openapi.json`.
#[derive(Debug)]
pub struct InputField {
    /// The component schema that has the property.
    pub schema: &'static str,
    /// The property's name.
    pub property: &'static str,
    /// The UI label (`FIELDS.md`, column "UI label").
    pub title: &'static str,
    /// The one-sentence help shown before the full help text
    /// (`FIELDS.md`, column "Short help"); the full text is the doc
    /// comment.
    pub short_help: &'static str,
    /// The value used when the field is left out, as JSON.
    pub default: Option<&'static str>,
    /// The smallest value accepted.
    pub minimum: Option<f64>,
    /// The largest value accepted.
    pub maximum: Option<f64>,
    /// The most characters accepted, for text.
    pub max_length: Option<usize>,
}

/// An input field; the short help is required, so no field lacks it.
const fn input(
    schema: &'static str,
    property: &'static str,
    title: &'static str,
    short_help: &'static str,
) -> InputField {
    InputField {
        schema,
        property,
        title,
        short_help,
        default: None,
        minimum: None,
        maximum: None,
        max_length: None,
    }
}

impl InputField {
    const fn default(mut self, json: &'static str) -> Self {
        self.default = Some(json);
        self
    }

    const fn minimum(mut self, minimum: f64) -> Self {
        self.minimum = Some(minimum);
        self
    }

    const fn maximum(mut self, maximum: f64) -> Self {
        self.maximum = Some(maximum);
        self
    }

    const fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }
}

/// The shortest length accepted: lengths are in steps of 0.1 mm and must
/// be positive.
const SHORTEST: f64 = 0.1;

/// The longest length accepted, in millimetres (`model::Length::MAX`).
const LONGEST: f64 = 100_000.0;

/// Every input field, in the order of `FIELDS.md`.
pub static INPUT_FIELDS: &[InputField] = &[
    input(
        "JobRequest",
        "name",
        "Job name",
        "Your name for this job, shown above its cutting plan.",
    )
    .max_length(crate::types::MAX_NAME_CHARS),
    input(
        "Instance",
        "objective",
        "Goal",
        "The plan's aim: fewest panels, most from your panels, or shortest length or width.",
    )
    .default("\"bin_packing\""),
    input(
        "JobRequest",
        "time_limit",
        "Time limit",
        "How long to search for a better plan; you can stop it at any time.",
    )
    .default("10")
    .minimum(1.0)
    .maximum(600.0),
    input(
        "Panel",
        "id",
        "Name",
        "Your name for this panel size, shown on the cutting plan.",
    ),
    input(
        "Panel",
        "length",
        "Length",
        "The panel's long side in millimetres, along the grain if it has one.",
    )
    .minimum(SHORTEST)
    .maximum(LONGEST),
    input(
        "Panel",
        "width",
        "Width",
        "The panel's short side in millimetres.",
    )
    .minimum(SHORTEST)
    .maximum(LONGEST),
    input(
        "Panel",
        "quantity",
        "Quantity",
        "How many panels of this size you have; empty means as many as needed.",
    )
    .minimum(1.0),
    input(
        "Panel",
        "cost",
        "Cost",
        "Compares panel sizes when several are listed: a lower total cost is preferred.",
    )
    .minimum(0.0),
    input(
        "Panel",
        "trims",
        "Trims",
        "Strips to cut off damaged panel edges before any part is placed.",
    ),
    input(
        "Trim",
        "width",
        "Trim",
        "How much to cut off this edge to remove damage, in millimetres.",
    )
    // Its upper bound is the panel: opposite trims leave some of it.
    .default("0")
    .minimum(0.0),
    // No default: it depends on the edge (`FIELDS.md`), so `Trims` says it.
    input(
        "Trim",
        "cut",
        "Trimmed strip",
        "Whether the strip is sawn off first or stays on the offcut beside it.",
    ),
    input(
        "Part",
        "id",
        "Name",
        "Your name for this part, shown on the cutting plan.",
    ),
    input(
        "Part",
        "length",
        "Length",
        "The part's finished length in millimetres, along the grain if it keeps it.",
    )
    .minimum(SHORTEST)
    .maximum(LONGEST),
    input(
        "Part",
        "width",
        "Width",
        "The part's finished width in millimetres.",
    )
    .minimum(SHORTEST)
    .maximum(LONGEST),
    input(
        "Part",
        "quantity",
        "Quantity",
        "How many identical parts you need.",
    )
    .default("1")
    .minimum(1.0),
    input(
        "Part",
        "grain",
        "Keep grain direction",
        "Tick for veneer, solid wood or wood-look panels: the part is then never turned.",
    )
    .default("false"),
    input(
        "Part",
        "value",
        "Value",
        "How much you want this part when not every part fits; empty means its area.",
    )
    .minimum(0.0),
    input(
        "Saw",
        "kerf",
        "Saw blade thickness (kerf)",
        "How much material each saw cut removes, in millimetres: usually 2.2–4.4 mm.",
    )
    .default("0")
    .minimum(0.0)
    .maximum(LONGEST),
    input(
        "Saw",
        "stages",
        "Cutting stages",
        "How often pieces are turned and cut again: more stages, less waste, more handling.",
    )
    .default("3"),
    input(
        "Saw",
        "cut_type",
        "Last-stage trimming",
        "Whether a part may be trimmed free from a small offcut after the last stage.",
    )
    .default("\"non_exact\""),
    input(
        "Saw",
        "minimum_offcut_width",
        "Minimum offcut width",
        "The narrowest offcut the plan may leave, so none is too thin to handle.",
    )
    .default("0")
    .minimum(0.0)
    .maximum(LONGEST),
    input(
        "Saw",
        "first_cuts",
        "First cuts",
        "Which way the first full-panel cuts run: either way, along or across the length.",
    )
    .default("\"either\""),
];

/// One value of a field with a fixed set of options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionValue {
    /// A string value.
    Text(&'static str),
    /// A number value (only `stages` has them).
    Number(u32),
}

/// A field with a fixed set of options, and the name of each.
#[derive(Debug)]
pub struct Choice {
    /// The component schema of the field's type.
    pub schema: &'static str,
    /// Each value and its name for the woodworker (`FIELDS.md`, "Option
    /// names").
    pub options: &'static [(OptionValue, &'static str)],
}

use OptionValue::{Number, Text};

/// `model::Objective`.
pub static OBJECTIVE: Choice = Choice {
    schema: "Objective",
    options: &[
        (Text("bin_packing"), "Use as few panels as possible"),
        (Text("knapsack"), "Best use of the panels I have"),
        (Text("open_dimension_x"), "Shortest length from one panel"),
        (Text("open_dimension_y"), "Shortest width from one panel"),
    ],
};

/// `model::Stages`.
pub static STAGES: Choice = Choice {
    schema: "Stages",
    options: &[
        (Number(2), "2"),
        (Number(3), "3"),
        (Text("unlimited"), "Unlimited"),
    ],
};

/// `model::CutType`.
pub static CUT_TYPE: Choice = Choice {
    schema: "CutType",
    options: &[
        (Text("non_exact"), "Allowed"),
        (Text("exact"), "Not allowed"),
        (Text("homogenous"), "One part per strip"),
    ],
};

/// `model::FirstCuts`.
pub static FIRST_CUTS: Choice = Choice {
    schema: "FirstCuts",
    options: &[
        (Text("either"), "Either"),
        (Text("along_length"), "Along the length"),
        (Text("across_length"), "Across the length"),
    ],
};

/// `model::TrimCut`.
pub static TRIM_CUT: Choice = Choice {
    schema: "TrimCut",
    options: &[
        (Text("cut_off_first"), "Cut off first"),
        (Text("left_on_offcut"), "Left on the offcut"),
    ],
};

/// Every field with options.
pub static CHOICES: [&Choice; 5] = [&OBJECTIVE, &STAGES, &CUT_TYPE, &FIRST_CUTS, &TRIM_CUT];

impl Choice {
    /// Whether `value` is the string of one of the options.
    pub fn has_text(&self, value: &str) -> bool {
        self.options
            .iter()
            .any(|(option, _)| matches!(option, Text(text) if *text == value))
    }

    /// The string options, for messages.
    pub fn texts(&self) -> Vec<&'static str> {
        self.options
            .iter()
            .filter_map(|(option, _)| match option {
                Text(text) => Some(*text),
                Number(_) => None,
            })
            .collect()
    }
}
