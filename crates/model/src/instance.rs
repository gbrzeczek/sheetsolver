//! What the user asks for: the parts they need, the panels they have and
//! how their saw cuts.
//!
//! Doc comments on user-facing fields are the exact help text from
//! `docs/FIELDS.md`; they become the OpenAPI descriptions the UI shows.

use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::length::Length;

/// A cutting job: everything the solver needs to produce a cutting plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    /// What the plan should optimise. **Use as few panels as possible**:
    /// cut every part, using the fewest panels; panels are used in the
    /// order listed. **Best use of the panels I have**: the panels can't
    /// fit every part, so choose which parts to cut to get the most out of
    /// them (by part value — see *Value*). **Shortest length from one
    /// panel**: you have one panel of fixed width and any length you need
    /// (e.g. a long board or a roll); cut every part using the shortest
    /// possible length. **Shortest width from one panel**: the same, but
    /// the length is fixed and the width is minimised.
    pub objective: Objective,
    /// How the saw cuts.
    pub saw: Saw,
    /// The stock panels to cut from, in the order they should be used.
    pub panels: Vec<Panel>,
    /// The parts to cut.
    pub parts: Vec<Part>,
}

/// What the plan optimises. See [`Instance::objective`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    /// Use as few panels as possible (upstream `BinPacking`).
    BinPacking,
    /// Best use of the panels I have (upstream `Knapsack`).
    Knapsack,
    /// Shortest length from one panel (upstream `OpenDimensionX`).
    OpenDimensionX,
    /// Shortest width from one panel (upstream `OpenDimensionY`).
    OpenDimensionY,
}

impl Objective {
    /// Whether every part must be cut. Only the knapsack objective may
    /// leave parts out.
    pub fn cuts_every_part(self) -> bool {
        self != Objective::Knapsack
    }

    /// Whether the objective works on a single panel with one open side.
    pub fn is_open_dimension(self) -> bool {
        matches!(self, Objective::OpenDimensionX | Objective::OpenDimensionY)
    }
}

/// Saw and cutting settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saw {
    /// The width of material each cut removes, in millimetres — the
    /// thickness of your saw blade's teeth, not the blade body. Every cut
    /// between two parts uses this much material, so parts are spaced by
    /// it. Typical values: 3.2–4.4 mm for panel and table saws, 2.2–2.8 mm
    /// for track saws. Set 0 only if your dimensions already include it.
    pub kerf: Length,
    /// How many times a piece is turned and cut again. **Stage 1**:
    /// full-length cuts across the whole panel, splitting it into strips.
    /// **Stage 2**: each strip is cut across into pieces. **Stage 3**: each
    /// piece is cut once more, across stage-2 cuts. More stages let parts
    /// nest more tightly, so less waste, but mean more handling at the
    /// saw. 2 suits simple strip cutting; 3 suits most panel-saw work;
    /// "Unlimited" gives the least waste with the most handling: cuts may
    /// then go either way at every stage, and the last-stage trimming
    /// setting does not apply.
    pub stages: Stages,
    /// Whether a part may be cut free from a small offcut after the last
    /// stage. **Allowed** (recommended): a strip may hold parts of
    /// different sizes; a final trimming cut separates each part from its
    /// offcut. **Not allowed**: at the last stage every part must fill its
    /// strip exactly, with no offcut — fewer cuts, more waste. **One part
    /// per strip**: every part in a last-stage strip is the same part —
    /// the simplest plans to cut, the most waste.
    pub cut_type: CutType,
}

/// The maximum number of cutting stages. JSON: `2`, `3` or `"unlimited"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stages {
    /// At most this many stages. Only 2 and 3 are valid (see
    /// [`Instance::validate`]).
    Limited(u32),
    /// No limit on the number of stages.
    Unlimited,
}

impl Serialize for Stages {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Stages::Limited(count) => serializer.serialize_u32(*count),
            Stages::Unlimited => serializer.serialize_str("unlimited"),
        }
    }
}

impl<'de> Deserialize<'de> for Stages {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Count(u32),
            Word(String),
        }
        match Raw::deserialize(deserializer)? {
            Raw::Count(count) => Ok(Stages::Limited(count)),
            Raw::Word(word) if word == "unlimited" => Ok(Stages::Unlimited),
            Raw::Word(word) => Err(serde::de::Error::custom(format!(
                "stages must be 2, 3 or \"unlimited\", not \"{word}\""
            ))),
        }
    }
}

/// Whether parts may be trimmed free after the last stage. See
/// [`Saw::cut_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutType {
    /// Last-stage trimming allowed (upstream `NonExact`).
    NonExact,
    /// No last-stage trimming: parts fill their strip exactly (upstream
    /// `Exact`).
    Exact,
    /// No last-stage trimming, and one part type per last-stage strip
    /// (upstream `Homogenous`).
    Homogenous,
}

/// A stock panel size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Panel {
    /// Your name for this panel, shown on the cutting plan (e.g. "Birch ply
    /// 18 mm").
    pub id: String,
    /// The panel's long side, as delivered, in millimetres. For a panel
    /// with grain, the side along the grain. For "Shortest length from one
    /// panel", this is the maximum length available.
    pub length: Length,
    /// The panel's short side, as delivered, in millimetres. For "Shortest
    /// width from one panel", this is the maximum width available.
    pub width: Length,
    /// How many panels of this size you have. Leave empty for "as many as
    /// needed".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,
    /// Used only to compare panel sizes when several are listed: a lower
    /// total cost is preferred. Leave empty to use the panel's area.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

impl Panel {
    /// The cost used by the solver: the given cost, or the area in mm².
    pub fn effective_cost(&self) -> f64 {
        self.cost
            .unwrap_or_else(|| self.length.times(self.width).mm2())
    }
}

/// A part to cut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    /// Your name for this part, shown on the cutting plan and labels (e.g.
    /// "Side left").
    pub id: String,
    /// The part's finished length in millimetres, before edge banding. For
    /// a part with grain, the side along the grain.
    pub length: Length,
    /// The part's finished width in millimetres, before edge banding.
    pub width: Length,
    /// How many identical parts you need. With "Best use of the panels I
    /// have", this is the most that will be cut; fewer may be cut if they
    /// don't fit.
    pub quantity: u32,
    /// Tick this if the part must keep its grain running the same way as
    /// the panel's grain (veneered or solid-wood panels, brushed or
    /// wood-look melamine). The part is then never turned 90°. Untick it
    /// for plain MDF, chipboard or any part whose look doesn't depend on
    /// direction — turning parts usually saves material.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grain: bool,
    /// Only used with "Best use of the panels I have": how much you want
    /// this part, compared with the others. The plan maximises the total
    /// value of the parts it cuts. Leave empty to value parts by their area
    /// in mm² (the plan then uses as much of the panel as possible). Set
    /// every part to 1 to cut as many parts as possible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
}

impl Part {
    /// The value used by the solver: the given value, or the area in mm².
    pub fn effective_value(&self) -> f64 {
        self.value
            .unwrap_or_else(|| self.length.times(self.width).mm2())
    }
}

/// A reason an instance cannot be solved as given.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ValidationError {
    /// No panels were given.
    #[error("add at least one panel")]
    NoPanels,
    /// No parts were given.
    #[error("add at least one part")]
    NoParts,
    /// Two panels or two parts share a name.
    #[error("{what} name \"{id}\" is used more than once")]
    DuplicateId {
        /// "panel" or "part".
        what: &'static str,
        /// The repeated name.
        id: String,
    },
    /// A name is empty.
    #[error("{what} {index} has no name")]
    EmptyId {
        /// "panel" or "part".
        what: &'static str,
        /// 1-based position in the list.
        index: usize,
    },
    /// A length or width is zero or negative.
    #[error("{field} of {what} \"{id}\" must be at least 0.1 mm")]
    NonPositiveSize {
        /// "panel" or "part".
        what: &'static str,
        /// The panel's or part's name.
        id: String,
        /// "length" or "width".
        field: &'static str,
    },
    /// A quantity is zero.
    #[error("quantity of {what} \"{id}\" must be at least 1")]
    ZeroQuantity {
        /// "panel" or "part".
        what: &'static str,
        /// The panel's or part's name.
        id: String,
    },
    /// A cost or value is negative or not a number.
    #[error("{field} of {what} \"{id}\" must be a number of at least 0")]
    InvalidNumber {
        /// "panel" or "part".
        what: &'static str,
        /// The panel's or part's name.
        id: String,
        /// "cost" or "value".
        field: &'static str,
    },
    /// The kerf is negative.
    #[error("saw blade thickness must be at least 0 mm")]
    NegativeKerf,
    /// The number of stages is not 2, 3 or unlimited.
    #[error("cutting stages must be 2, 3 or unlimited, not {0}")]
    UnsupportedStages(u32),
    /// An open-dimension objective needs exactly one panel.
    #[error("\"Shortest length/width from one panel\" needs exactly one panel, with quantity 1")]
    OpenDimensionNeedsOnePanel,
}

impl Instance {
    /// Checks everything that makes an instance unsolvable as given, and
    /// returns every problem found, not just the first.
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        if self.saw.kerf < Length::ZERO {
            errors.push(ValidationError::NegativeKerf);
        }
        if let Stages::Limited(count) = self.saw.stages {
            if !(2..=3).contains(&count) {
                errors.push(ValidationError::UnsupportedStages(count));
            }
        }

        if self.panels.is_empty() {
            errors.push(ValidationError::NoPanels);
        }
        if self.parts.is_empty() {
            errors.push(ValidationError::NoParts);
        }

        let panels = self
            .panels
            .iter()
            .map(|p| (p.id.as_str(), p.length, p.width, p.quantity, p.cost));
        validate_items("panel", panels, &mut errors);
        let parts = self
            .parts
            .iter()
            .map(|p| (p.id.as_str(), p.length, p.width, Some(p.quantity), p.value));
        validate_items("part", parts, &mut errors);

        if self.objective.is_open_dimension()
            && (self.panels.len() != 1 || self.panels[0].quantity.is_some_and(|q| q != 1))
        {
            errors.push(ValidationError::OpenDimensionNeedsOnePanel);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// The part with this name.
    pub fn part(&self, id: &str) -> Option<&Part> {
        self.parts.iter().find(|part| part.id == id)
    }

    /// The panel with this name.
    pub fn panel(&self, id: &str) -> Option<&Panel> {
        self.panels.iter().find(|panel| panel.id == id)
    }
}

/// Checks the fields panels and parts have in common: name, size, quantity
/// and cost/value.
fn validate_items<'a>(
    what: &'static str,
    items: impl Iterator<Item = (&'a str, Length, Length, Option<u32>, Option<f64>)>,
    errors: &mut Vec<ValidationError>,
) {
    let number_field = if what == "panel" { "cost" } else { "value" };
    let mut seen = HashSet::new();
    for (index, (id, length, width, quantity, number)) in items.enumerate() {
        if id.trim().is_empty() {
            errors.push(ValidationError::EmptyId {
                what,
                index: index + 1,
            });
        } else if !seen.insert(id) {
            errors.push(ValidationError::DuplicateId {
                what,
                id: id.to_string(),
            });
        }
        for (field, size) in [("length", length), ("width", width)] {
            if size <= Length::ZERO {
                errors.push(ValidationError::NonPositiveSize {
                    what,
                    id: id.to_string(),
                    field,
                });
            }
        }
        if quantity == Some(0) {
            errors.push(ValidationError::ZeroQuantity {
                what,
                id: id.to_string(),
            });
        }
        if number.is_some_and(|n| !n.is_finite() || n < 0.0) {
            errors.push(ValidationError::InvalidNumber {
                what,
                id: id.to_string(),
                field: number_field,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> Instance {
        serde_json::from_str(
            r#"{
                "objective": "bin_packing",
                "saw": { "kerf": 3.2, "stages": 3, "cut_type": "non_exact" },
                "panels": [{ "id": "Birch ply 18 mm", "length": 2500, "width": 1250, "quantity": 10 }],
                "parts": [{ "id": "Side left", "length": 720, "width": 560, "quantity": 4, "grain": true }]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn reads_the_documented_example() {
        let instance = example();
        assert_eq!(instance.saw.kerf, Length::from_tenths(32));
        assert_eq!(instance.saw.stages, Stages::Limited(3));
        assert!(instance.parts[0].grain);
        assert_eq!(instance.panels[0].quantity, Some(10));
        assert_eq!(instance.validate(), Ok(()));
    }

    #[test]
    fn reads_unlimited_stages_and_rejects_other_words() {
        let stages: Stages = serde_json::from_str(r#""unlimited""#).unwrap();
        assert_eq!(stages, Stages::Unlimited);
        assert!(serde_json::from_str::<Stages>(r#""many""#).is_err());
        assert_eq!(serde_json::to_string(&Stages::Limited(2)).unwrap(), "2");
    }

    #[test]
    fn default_value_and_cost_are_areas_in_square_millimetres() {
        let instance = example();
        assert_eq!(instance.parts[0].effective_value(), 720.0 * 560.0);
        assert_eq!(instance.panels[0].effective_cost(), 2500.0 * 1250.0);
    }

    #[test]
    fn reports_every_problem() {
        let mut instance = example();
        instance.saw.stages = Stages::Limited(4);
        instance.parts.push(instance.parts[0].clone());
        instance.parts[1].quantity = 0;
        instance.parts[1].width = Length::ZERO;
        let errors = instance.validate().unwrap_err();
        assert_eq!(
            errors,
            vec![
                ValidationError::UnsupportedStages(4),
                ValidationError::DuplicateId {
                    what: "part",
                    id: "Side left".into()
                },
                ValidationError::NonPositiveSize {
                    what: "part",
                    id: "Side left".into(),
                    field: "width"
                },
                ValidationError::ZeroQuantity {
                    what: "part",
                    id: "Side left".into()
                },
            ]
        );
    }

    #[test]
    fn open_dimension_needs_one_panel_of_quantity_one() {
        let mut instance = example();
        instance.objective = Objective::OpenDimensionX;
        assert_eq!(
            instance.validate(),
            Err(vec![ValidationError::OpenDimensionNeedsOnePanel])
        );
        instance.panels[0].quantity = Some(1);
        assert_eq!(instance.validate(), Ok(()));
    }
}
