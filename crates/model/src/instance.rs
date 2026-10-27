//! What the user asks for: the parts they need, the panels they have and
//! how their saw cuts.
//!
//! Doc comments on user-facing fields are the exact help text from
//! `docs/FIELDS.md`; they become the OpenAPI descriptions the UI shows.

use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::length::Length;
use crate::solution::CutDirection;

/// A cutting job: everything the solver needs to produce a cutting plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    #[serde(default)]
    pub objective: Objective,
    /// How the saw cuts.
    pub saw: Saw,
    /// The stock panels to cut from, in the order they should be used.
    pub panels: Vec<Panel>,
    /// The parts to cut.
    pub parts: Vec<Part>,
}

/// What the plan optimises. See [`Instance::objective`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum Objective {
    /// Use as few panels as possible (upstream `BinPacking`).
    #[default]
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Saw {
    /// The width of material each cut removes, in millimetres — the
    /// thickness of your saw blade's teeth, not the blade body. Every cut
    /// between two parts uses this much material, so parts are spaced by
    /// it. Typical values: 3.2–4.4 mm for panel and table saws, 2.2–2.8 mm
    /// for track saws. Leave it at 0 only if your dimensions already
    /// include it.
    #[serde(default = "default_kerf")]
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
    #[serde(default)]
    pub stages: Stages,
    /// Whether a part may be cut free from a small offcut after the last
    /// stage. **Allowed** (recommended): a strip may hold parts of
    /// different sizes; a final trimming cut separates each part from its
    /// offcut. **Not allowed**: at the last stage every part must fill its
    /// strip exactly, with no offcut — fewer cuts, more waste. **One part
    /// per strip**: every part in a last-stage strip is the same part —
    /// the simplest plans to cut, the most waste.
    #[serde(default)]
    pub cut_type: CutType,
    /// Which way the first, full-panel cuts run. **Either** (recommended):
    /// the solver tries both and keeps the better plan. With one panel
    /// size, every panel in the plan is cut the same way; with several
    /// sizes, each panel may be cut differently. The search time is shared
    /// between the two directions. **Along the length**: rip the panel into long
    /// strips first, then cut the strips across — for saws or workflows
    /// that always rip first. **Across the length**: cut the panel across
    /// into shorter strips first. Parts with grain keep their direction
    /// whichever you choose. With unlimited stages, cuts may always go
    /// either way.
    #[serde(default)]
    pub first_cuts: FirstCuts,
    /// The narrowest offcut the plan may leave, in millimetres: thin
    /// offcuts can be thrown back by the blade or slip out of a panel
    /// saw's clamps. Every offcut, and the leftover, is at least this
    /// wide, measured across the cut that frees it; along that cut it may
    /// be as narrow as the parts beside it. Parts narrower than this are
    /// still cut. Along a trimmed edge, its *Trimmed strip* says whether
    /// the trim counts in the offcut's width. 0 allows any width, down to
    /// strips of saw dust; 20–50 mm is typical. A larger value can mean
    /// more waste.
    #[serde(default, skip_serializing_if = "Length::is_zero")]
    pub minimum_offcut_width: Length,
}

/// The kerf when none is given: none, so nothing is cut away until the
/// user enters their blade (owner's decision, 2026-10-27).
fn default_kerf() -> Length {
    Length::ZERO
}

/// Which way the first cuts may run. See [`Saw::first_cuts`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum FirstCuts {
    /// Whichever gives the better plan (upstream `Any`).
    #[default]
    Either,
    /// Only along the panel's length (upstream `Horizontal`).
    AlongLength,
    /// Only across the panel's length (upstream `Vertical`).
    AcrossLength,
}

impl std::fmt::Display for FirstCuts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FirstCuts::Either => "either direction",
            FirstCuts::AlongLength => "only along the length",
            FirstCuts::AcrossLength => "only across the length",
        })
    }
}

impl FirstCuts {
    /// Whether a pattern whose first cuts go in `direction` is allowed.
    pub fn allows(self, direction: CutDirection) -> bool {
        match self {
            FirstCuts::Either => true,
            FirstCuts::AlongLength => direction == CutDirection::AlongLength,
            FirstCuts::AcrossLength => direction == CutDirection::AcrossLength,
        }
    }
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

impl Default for Stages {
    /// 3 stages, which suit most panel-saw work.
    fn default() -> Self {
        Stages::Limited(3)
    }
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum CutType {
    /// Last-stage trimming allowed (upstream `NonExact`).
    #[default]
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    /// Strips to cut off the panel's edges, named as the panel is drawn on
    /// the plan: the left and right ends, and the bottom and top long
    /// edges. Each edge has its *Trim* width and its *Trimmed strip*: how
    /// the strip is cut away. Leave them empty for clean factory edges.
    #[serde(default, skip_serializing_if = "Trims::is_none")]
    pub trims: Trims,
}

/// The strips cut off a panel's four edges. An edge left out has no trim.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(from = "RawTrims")]
pub struct Trims {
    /// The left end, at the panel's origin (x = 0).
    #[serde(default, skip_serializing_if = "Trim::is_none")]
    pub left: Trim,
    /// The right end, opposite the left one.
    #[serde(default, skip_serializing_if = "Trim::is_none")]
    pub right: Trim,
    /// The bottom long edge (y = 0).
    #[serde(default, skip_serializing_if = "Trim::is_none")]
    pub bottom: Trim,
    /// The top long edge, opposite the bottom one.
    #[serde(default, skip_serializing_if = "Trim::is_none")]
    pub top: Trim,
}

/// The strip cut off one edge of a panel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Trim {
    /// How much to cut off this edge of the panel, in millimetres, to
    /// remove a chipped or damaged factory edge. No part is placed in it,
    /// and the saw cut that removes it is taken from it, so parts start
    /// exactly this far from the edge. Typical values: 5–15 mm on
    /// chipboard, MDF and melamine-faced board; more on a dented edge.
    /// Leave empty or 0 for a clean edge.
    #[serde(default)]
    pub width: Length,
    /// How the trimmed strip is cut away; it only matters with a *Minimum
    /// offcut width*. **Cut off first**: the strip is sawn off before any
    /// other cut, so the panel has a clean new edge and every offcut width
    /// is measured from it. **Left on the offcut**: the strip gets no cut
    /// of its own; it stays on the offcut along that edge and counts in
    /// its width, so fewer offcuts are too narrow. On the left end and the
    /// bottom edge, where cutting starts, such a strip is still cut away
    /// first, as an offcut at least *Minimum offcut width* wide, which can
    /// waste more. By default the left end and the bottom edge are cut off
    /// first and the right end and the top edge left on the offcut, which
    /// suits most jobs. With unlimited stages and no minimum offcut width,
    /// every strip is cut off first.
    #[serde(default)]
    pub cut: TrimCut,
}

impl Trim {
    /// No trim on this edge.
    pub fn is_none(&self) -> bool {
        self.width == Length::ZERO
    }

    /// Whether the strip is sawn off first (upstream `TrimType::Hard`) and
    /// actually there.
    pub fn is_cut_off_first(&self) -> bool {
        self.width > Length::ZERO && self.cut == TrimCut::CutOffFirst
    }

    /// Whether the strip stays on the offcut beside it (upstream
    /// `TrimType::Soft`) and is actually there.
    pub fn is_left_on_offcut(&self) -> bool {
        self.width > Length::ZERO && self.cut == TrimCut::LeftOnOffcut
    }
}

/// How a trimmed strip is cut away. See [`Trim::cut`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum TrimCut {
    /// Sawn off before any other cut (upstream `TrimType::Hard`).
    #[default]
    CutOffFirst,
    /// No cut of its own: it stays on the offcut beside it (upstream
    /// `TrimType::Soft`).
    LeftOnOffcut,
}

impl Trims {
    /// No trim on any edge.
    pub fn is_none(&self) -> bool {
        self.edges().iter().all(|(_, trim)| trim.is_none())
    }

    /// The four edges with their JSON names, in the order left, right,
    /// bottom, top.
    pub fn edges(&self) -> [(&'static str, Trim); 4] {
        [
            ("left", self.left),
            ("right", self.right),
            ("bottom", self.bottom),
            ("top", self.top),
        ]
    }
}

/// [`Trims`] as read, before each edge gets its default *Trimmed strip*.
#[derive(Deserialize)]
struct RawTrims {
    #[serde(default)]
    left: Option<RawTrim>,
    #[serde(default)]
    right: Option<RawTrim>,
    #[serde(default)]
    bottom: Option<RawTrim>,
    #[serde(default)]
    top: Option<RawTrim>,
}

#[derive(Deserialize)]
struct RawTrim {
    #[serde(default)]
    width: Length,
    #[serde(default)]
    cut: Option<TrimCut>,
}

impl From<RawTrims> for Trims {
    /// upstream: instance_builder.cpp, `InstanceBuilder::read`: the left
    /// and bottom trims default to `Hard`, the right and top ones to
    /// `Soft` (owner's decision to keep them, `SCOPE.md`, 2026-10-27).
    fn from(raw: RawTrims) -> Self {
        let edge = |raw: Option<RawTrim>, default: TrimCut| {
            raw.map_or(
                Trim {
                    width: Length::ZERO,
                    cut: default,
                },
                |raw| Trim {
                    width: raw.width,
                    cut: raw.cut.unwrap_or(default),
                },
            )
        };
        Trims {
            left: edge(raw.left, TrimCut::CutOffFirst),
            right: edge(raw.right, TrimCut::LeftOnOffcut),
            bottom: edge(raw.bottom, TrimCut::CutOffFirst),
            top: edge(raw.top, TrimCut::LeftOnOffcut),
        }
    }
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
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
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
    #[serde(default = "one")]
    pub quantity: u32,
    /// Tick if the part must keep the panel's grain direction (veneer,
    /// solid wood, wood-look melamine); it is then never turned. Leave
    /// unticked for MDF or chipboard: turning parts saves material.
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

/// A part's quantity when none is given.
fn one() -> u32 {
    1
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
    /// A length or width is longer than [`Length::MAX`].
    #[error("{field} of {what} \"{id}\" must be at most 100000 mm")]
    TooLarge {
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
    /// The kerf is longer than [`Length::MAX`].
    #[error("saw blade thickness must be at most 100000 mm")]
    KerfTooLarge,
    /// The number of stages is not 2, 3 or unlimited.
    #[error("cutting stages must be 2, 3 or unlimited, not {0}")]
    UnsupportedStages(u32),
    /// The minimum offcut width is negative.
    #[error("minimum offcut width must be at least 0 mm")]
    NegativeMinimumOffcutWidth,
    /// The minimum offcut width is longer than [`Length::MAX`].
    #[error("minimum offcut width must be at most 100000 mm")]
    MinimumOffcutWidthTooLarge,
    /// A trim is negative or longer than [`Length::MAX`].
    #[error("{edge} trim of panel \"{id}\" must be between 0 and 100000 mm")]
    TrimOutOfRange {
        /// The panel's name.
        id: String,
        /// "left", "right", "bottom" or "top".
        edge: &'static str,
    },
    /// Two opposite trims leave nothing of the panel.
    #[error("the {edges} trims of panel \"{id}\" together must be less than its {side}")]
    TrimsTooWide {
        /// The panel's name.
        id: String,
        /// "left and right" or "bottom and top".
        edges: &'static str,
        /// "length" or "width".
        side: &'static str,
    },
    /// An open-dimension objective needs exactly one panel.
    #[error("\"Shortest length/width from one panel\" needs exactly one panel, with quantity 1")]
    OpenDimensionNeedsOnePanel,
}

/// A [`ValidationError`] and the field it is about.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldError {
    /// The path of the field in the instance's JSON, with 0-based list
    /// positions: `saw.kerf`, `parts[2].quantity`, or a whole list
    /// (`parts`) for a problem with the list itself.
    pub field: String,
    /// What is wrong with it.
    pub error: ValidationError,
}

impl Instance {
    /// Checks everything that makes an instance unsolvable as given, and
    /// returns every problem found, not just the first.
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        self.validate_fields()
            .map_err(|errors| errors.into_iter().map(|error| error.error).collect())
    }

    /// As [`Instance::validate`], with the field each problem is about.
    pub fn validate_fields(&self) -> Result<(), Vec<FieldError>> {
        let mut errors = Vec::new();
        let mut report = |field: String, error| errors.push(FieldError { field, error });

        if self.saw.kerf < Length::ZERO {
            report("saw.kerf".into(), ValidationError::NegativeKerf);
        } else if self.saw.kerf > Length::MAX {
            report("saw.kerf".into(), ValidationError::KerfTooLarge);
        }
        if let Stages::Limited(count) = self.saw.stages {
            if !(2..=3).contains(&count) {
                report(
                    "saw.stages".into(),
                    ValidationError::UnsupportedStages(count),
                );
            }
        }

        if self.saw.minimum_offcut_width < Length::ZERO {
            report(
                "saw.minimum_offcut_width".into(),
                ValidationError::NegativeMinimumOffcutWidth,
            );
        } else if self.saw.minimum_offcut_width > Length::MAX {
            report(
                "saw.minimum_offcut_width".into(),
                ValidationError::MinimumOffcutWidthTooLarge,
            );
        }

        if self.panels.is_empty() {
            report("panels".into(), ValidationError::NoPanels);
        }
        if self.parts.is_empty() {
            report("parts".into(), ValidationError::NoParts);
        }

        let panels = self
            .panels
            .iter()
            .map(|p| (p.id.as_str(), p.length, p.width, p.quantity, p.cost));
        validate_items("panel", panels, &mut report);
        for (index, panel) in self.panels.iter().enumerate() {
            validate_trims(index, panel, &mut report);
        }
        let parts = self
            .parts
            .iter()
            .map(|p| (p.id.as_str(), p.length, p.width, Some(p.quantity), p.value));
        validate_items("part", parts, &mut report);

        if self.objective.is_open_dimension()
            && (self.panels.len() != 1 || self.panels[0].quantity.is_some_and(|q| q != 1))
        {
            report("panels".into(), ValidationError::OpenDimensionNeedsOnePanel);
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

/// Checks a panel's trims: each between 0 and [`Length::MAX`], and
/// opposite ones leaving some of the panel (upstream:
/// instance_builder.cpp, `InstanceBuilder::add_trims`).
fn validate_trims(index: usize, panel: &Panel, report: &mut impl FnMut(String, ValidationError)) {
    let path = |edge: &str| format!("panels[{index}].trims.{edge}.width");
    let mut in_range = true;
    for (edge, trim) in panel.trims.edges() {
        if trim.width < Length::ZERO || trim.width > Length::MAX {
            in_range = false;
            report(
                path(edge),
                ValidationError::TrimOutOfRange {
                    id: panel.id.clone(),
                    edge,
                },
            );
        }
    }
    if !in_range {
        return;
    }
    let trims = &panel.trims;
    let pairs = [
        (
            "right",
            "left and right",
            "length",
            trims.left.width + trims.right.width,
            panel.length,
        ),
        (
            "top",
            "bottom and top",
            "width",
            trims.bottom.width + trims.top.width,
            panel.width,
        ),
    ];
    for (edge, edges, side, total, size) in pairs {
        if size > Length::ZERO && total >= size {
            report(
                path(edge),
                ValidationError::TrimsTooWide {
                    id: panel.id.clone(),
                    edges,
                    side,
                },
            );
        }
    }
}

/// Checks the fields panels and parts have in common: name, size, quantity
/// and cost/value. `report` takes the field's path and the problem.
fn validate_items<'a>(
    what: &'static str,
    items: impl Iterator<Item = (&'a str, Length, Length, Option<u32>, Option<f64>)>,
    report: &mut impl FnMut(String, ValidationError),
) {
    let number_field = if what == "panel" { "cost" } else { "value" };
    let mut seen = HashSet::new();
    for (index, (id, length, width, quantity, number)) in items.enumerate() {
        let path = |field: &str| format!("{what}s[{index}].{field}");
        if id.trim().is_empty() {
            report(
                path("id"),
                ValidationError::EmptyId {
                    what,
                    index: index + 1,
                },
            );
        } else if !seen.insert(id) {
            report(
                path("id"),
                ValidationError::DuplicateId {
                    what,
                    id: id.to_string(),
                },
            );
        }
        for (field, size) in [("length", length), ("width", width)] {
            if size <= Length::ZERO {
                report(
                    path(field),
                    ValidationError::NonPositiveSize {
                        what,
                        id: id.to_string(),
                        field,
                    },
                );
            } else if size > Length::MAX {
                report(
                    path(field),
                    ValidationError::TooLarge {
                        what,
                        id: id.to_string(),
                        field,
                    },
                );
            }
        }
        if quantity == Some(0) {
            report(
                path("quantity"),
                ValidationError::ZeroQuantity {
                    what,
                    id: id.to_string(),
                },
            );
        }
        if number.is_some_and(|n| !n.is_finite() || n < 0.0) {
            report(
                path(number_field),
                ValidationError::InvalidNumber {
                    what,
                    id: id.to_string(),
                    field: number_field,
                },
            );
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
        // Not given: the default.
        assert_eq!(instance.saw.first_cuts, FirstCuts::Either);
        assert!(instance.parts[0].grain);
        assert_eq!(instance.panels[0].quantity, Some(10));
        assert_eq!(instance.validate(), Ok(()));
    }

    #[test]
    fn fields_left_out_take_their_defaults_from_fields_md() {
        let instance: Instance = serde_json::from_str(
            r#"{
                "saw": {},
                "panels": [{ "id": "Ply", "length": 2500, "width": 1250 }],
                "parts": [{ "id": "Shelf", "length": 800, "width": 300 }]
            }"#,
        )
        .unwrap();
        assert_eq!(instance.objective, Objective::BinPacking);
        assert_eq!(instance.saw.kerf, Length::ZERO);
        assert_eq!(instance.saw.stages, Stages::Limited(3));
        assert_eq!(instance.saw.cut_type, CutType::NonExact);
        assert_eq!(instance.saw.first_cuts, FirstCuts::Either);
        assert_eq!(instance.parts[0].quantity, 1);
        assert!(!instance.parts[0].grain);
    }

    #[test]
    fn trims_default_to_none_and_their_cut_depends_on_the_edge() {
        let mut instance = example();
        assert!(instance.panels[0].trims.is_none());
        assert_eq!(instance.saw.minimum_offcut_width, Length::ZERO);
        instance.panels[0] = serde_json::from_str(
            r#"{ "id": "P", "length": 2500, "width": 1250, "trims": {
                "left": { "width": 10 }, "right": { "width": 10 },
                "bottom": { "width": 5, "cut": "left_on_offcut" }, "top": { "width": 5 } } }"#,
        )
        .unwrap();
        let trims = instance.panels[0].trims;
        // upstream's defaults: the left and bottom edges hard, the others soft.
        assert_eq!(trims.left.cut, TrimCut::CutOffFirst);
        assert_eq!(trims.right.cut, TrimCut::LeftOnOffcut);
        assert_eq!(trims.bottom.cut, TrimCut::LeftOnOffcut);
        assert_eq!(trims.top.cut, TrimCut::LeftOnOffcut);
        assert!(trims.left.is_cut_off_first() && trims.right.is_left_on_offcut());
        assert_eq!(instance.validate(), Ok(()));
        // Written back, an edge keeps its cut, and edges without a trim
        // are left out.
        let mut json = serde_json::to_value(&instance.panels[0]).unwrap();
        assert_eq!(json["trims"]["right"]["cut"], "left_on_offcut");
        json["trims"]["left"]["width"] = serde_json::json!(0);
        let panel: Panel = serde_json::from_value(json).unwrap();
        let written = serde_json::to_value(&panel).unwrap();
        assert!(written["trims"].get("left").is_none());
    }

    #[test]
    fn trims_and_the_minimum_offcut_width_are_bounded() {
        let mut instance = example();
        instance.saw.minimum_offcut_width = Length::from_tenths(-1);
        instance.panels[0].trims.left.width = Length::from_mm(1250);
        instance.panels[0].trims.right.width = Length::from_mm(1250);
        instance.panels[0].trims.top.width = Length::from_tenths(-1);
        let fields: Vec<String> = instance
            .validate_fields()
            .unwrap_err()
            .into_iter()
            .map(|error| error.field)
            .collect();
        assert_eq!(
            fields,
            ["saw.minimum_offcut_width", "panels[0].trims.top.width"]
        );
        instance.saw.minimum_offcut_width = Length::from_mm(20);
        instance.panels[0].trims.top.width = Length::ZERO;
        assert_eq!(
            instance.validate(),
            Err(vec![ValidationError::TrimsTooWide {
                id: "Birch ply 18 mm".into(),
                edges: "left and right",
                side: "length"
            }])
        );
        instance.panels[0].trims.right.width = Length::from_tenths(12_499);
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

    #[test]
    fn rejects_sizes_longer_than_100_m() {
        let mut instance = example();
        instance.panels[0].length = Length::from_mm(100_000);
        instance.saw.kerf = Length::from_mm(100_000);
        assert_eq!(instance.validate(), Ok(()));
        // 1e12 mm once came back as a plan with a negative panel area.
        instance.panels[0].length = Length::try_from_mm(1e12).unwrap();
        instance.parts[0].width = Length::from_tenths(Length::MAX.tenths() + 1);
        instance.saw.kerf = Length::from_tenths(Length::MAX.tenths() + 1);
        let fields: Vec<String> = instance
            .validate_fields()
            .unwrap_err()
            .into_iter()
            .map(|error| error.field)
            .collect();
        assert_eq!(fields, ["saw.kerf", "panels[0].length", "parts[0].width"]);
    }

    #[test]
    fn names_the_field_of_each_problem() {
        let mut instance = example();
        instance.saw.kerf = Length::from_tenths(-1);
        instance.parts.push(instance.parts[0].clone());
        instance.parts[1].quantity = 0;
        instance.panels.clear();
        let fields: Vec<String> = instance
            .validate_fields()
            .unwrap_err()
            .into_iter()
            .map(|error| error.field)
            .collect();
        assert_eq!(
            fields,
            ["saw.kerf", "panels", "parts[1].id", "parts[1].quantity"]
        );
    }
}
