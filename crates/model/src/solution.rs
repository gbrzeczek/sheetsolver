//! What the solver returns: a cutting plan as a tree of cuts per panel.

use serde::{Deserialize, Serialize};

use crate::length::{Area, Length};

/// A cutting plan: one pattern per distinct way of cutting a panel.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Solution {
    /// The cutting patterns, in the order the panels are used.
    pub patterns: Vec<Pattern>,
}

/// One way of cutting a panel, possibly repeated on several identical
/// panels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Pattern {
    /// The name of the panel this pattern cuts.
    pub panel_id: String,
    /// How many panels are cut exactly this way.
    pub repeat: u32,
    /// The direction of the first, full-panel cuts.
    pub first_cuts: CutDirection,
    /// The whole panel: the root of the cutting tree, at stage 0.
    pub root: Node,
}

/// The direction of a set of parallel cuts, relative to the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum CutDirection {
    /// Cuts at a fixed `x` that run across the panel's length, from one
    /// long edge to the other (upstream "vertical"). The pieces they
    /// produce sit side by side along `x`.
    AcrossLength,
    /// Cuts at a fixed `y` that run along the panel's length (upstream
    /// "horizontal"). The pieces they produce sit side by side along `y`.
    AlongLength,
}

impl std::fmt::Display for CutDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CutDirection::AcrossLength => "across the length",
            CutDirection::AlongLength => "along the length",
        })
    }
}

impl CutDirection {
    /// The direction of the next stage's cuts: each stage turns the piece.
    pub fn turned(self) -> CutDirection {
        match self {
            CutDirection::AcrossLength => CutDirection::AlongLength,
            CutDirection::AlongLength => CutDirection::AcrossLength,
        }
    }
}

/// A rectangle in the cutting tree: the panel itself, an intermediate
/// piece, or a final part, offcut or leftover.
///
/// `x` runs along the panel's length and `y` along its width, both from
/// the panel's corner at (0, 0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Node {
    /// What this rectangle is.
    #[serde(flatten)]
    pub kind: NodeKind,
    /// Position along the panel's length.
    pub x: Length,
    /// Position along the panel's width.
    pub y: Length,
    /// Size along the panel's length.
    pub length: Length,
    /// Size along the panel's width.
    pub width: Length,
    /// The number of cuts that produced this rectangle: 0 for the panel,
    /// 1 for the pieces of the first cuts, and so on.
    pub stage: u32,
    /// The rectangles this piece is cut into, side by side in the
    /// direction of the next stage's cuts. Empty for everything but
    /// pieces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "openapi", schema(no_recursion))]
    pub children: Vec<Node>,
}

/// The kind of a [`Node`].
///
/// Its OpenAPI schema is written by hand (`openapi.rs`), to describe the
/// `kind` field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeKind {
    /// The panel or an intermediate piece, cut further into its children.
    Piece,
    /// A finished part.
    Part {
        /// The name of the part.
        part_id: String,
        /// Whether the part is turned 90°: its length lies along the
        /// panel's width.
        rotated: bool,
    },
    /// Waste.
    Offcut,
    /// The reusable remainder of the last panel, beyond its last
    /// first-stage cut.
    Leftover,
    /// A strip cut off an edge of the panel before any other cut (a trim
    /// *Cut off first*). Trims are children of the panel, at stage 0,
    /// before the trimmed panel, which is a piece at stage 0 too.
    Trim,
}

impl Node {
    /// Position along the axis that `direction` cuts across.
    pub fn start(&self, direction: CutDirection) -> Length {
        match direction {
            CutDirection::AcrossLength => self.x,
            CutDirection::AlongLength => self.y,
        }
    }

    /// Size along the axis that `direction` cuts across.
    pub fn size(&self, direction: CutDirection) -> Length {
        match direction {
            CutDirection::AcrossLength => self.length,
            CutDirection::AlongLength => self.width,
        }
    }

    /// End position along the axis that `direction` cuts across.
    pub fn end(&self, direction: CutDirection) -> Length {
        self.start(direction) + self.size(direction)
    }

    /// The rectangle's area.
    pub fn area(&self) -> Area {
        self.length.times(self.width)
    }

    /// Whether `other` covers exactly the same rectangle.
    pub fn same_rectangle(&self, other: &Node) -> bool {
        (self.x, self.y, self.length, self.width) == (other.x, other.y, other.length, other.width)
    }

    /// This node and every node below it, parents before children.
    pub fn descendants(&self) -> Vec<&Node> {
        let mut nodes = vec![self];
        let mut next = 0;
        while next < nodes.len() {
            nodes.extend(nodes[next].children.iter());
            next += 1;
        }
        nodes
    }
}

/// Figures describing a cutting plan. Computed by the checker.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Metrics {
    /// Panels cut, counting repeats.
    pub panels_used: u32,
    /// Parts cut, counting repeats.
    pub parts_cut: u32,
    /// Total area of the panels used.
    pub panel_area: Area,
    /// Total area of the parts cut.
    pub part_area: Area,
    /// Area of the reusable leftover of the last panel.
    pub leftover_area: Area,
    /// Panel area that is neither a part nor the leftover: offcuts and
    /// material removed by the saw.
    pub waste_area: Area,
    /// `waste_area` as a fraction of the panel area without the leftover
    /// (0 to 1).
    pub waste_fraction: f64,
    /// Total value of the parts cut (see `Part::value`).
    pub total_value: f64,
    /// On the last panel, how far along its length the cuts reach (the
    /// result for "Shortest length from one panel").
    pub used_length: Length,
    /// On the last panel, how far along its width the cuts reach (the
    /// result for "Shortest width from one panel").
    pub used_width: Length,
}
