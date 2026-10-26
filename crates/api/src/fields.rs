//! What `openapi.json` says about each field the woodworker fills in,
//! beyond its doc comment: its UI label, default, bounds and the names
//! of its options.
//!
//! The labels and option names are those of `docs/FIELDS.md`; the
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
    /// The value used when the field is left out, as JSON.
    pub default: Option<&'static str>,
    /// The smallest value accepted.
    pub minimum: Option<f64>,
    /// The largest value accepted.
    pub maximum: Option<f64>,
    /// The most characters accepted, for text.
    pub max_length: Option<usize>,
}

const fn input(schema: &'static str, property: &'static str, title: &'static str) -> InputField {
    InputField {
        schema,
        property,
        title,
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

/// Every input field, in the order of `FIELDS.md`.
pub static INPUT_FIELDS: &[InputField] = &[
    input("JobRequest", "name", "Job name").max_length(crate::types::MAX_NAME_CHARS),
    input("Instance", "objective", "Goal").default("\"bin_packing\""),
    input("JobRequest", "time_limit", "Time limit")
        .default("10")
        .minimum(1.0)
        .maximum(600.0),
    input("Panel", "id", "Name"),
    input("Panel", "length", "Length").minimum(SHORTEST),
    input("Panel", "width", "Width").minimum(SHORTEST),
    input("Panel", "quantity", "Quantity").minimum(1.0),
    input("Panel", "cost", "Cost").minimum(0.0),
    input("Part", "id", "Name"),
    input("Part", "length", "Length").minimum(SHORTEST),
    input("Part", "width", "Width").minimum(SHORTEST),
    input("Part", "quantity", "Quantity")
        .default("1")
        .minimum(1.0),
    input("Part", "grain", "Keep grain direction").default("false"),
    input("Part", "value", "Value").minimum(0.0),
    input("Saw", "kerf", "Saw blade thickness (kerf)")
        .default("3.2")
        .minimum(0.0),
    input("Saw", "stages", "Cutting stages").default("3"),
    input("Saw", "cut_type", "Last-stage trimming").default("\"non_exact\""),
    input("Saw", "first_cuts", "First cuts").default("\"either\""),
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

/// Every field with options.
pub static CHOICES: [&Choice; 4] = [&OBJECTIVE, &STAGES, &CUT_TYPE, &FIRST_CUTS];

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
