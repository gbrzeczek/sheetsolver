//! The rules a cutting plan can break.

use std::fmt;

use model::{CutDirection, FirstCuts, Length};

/// A rule a cutting plan breaks, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    /// Where in the plan: a pattern and a path of child positions, e.g.
    /// "pattern 2, piece 1.3", or "plan" for rules about the whole plan.
    pub location: String,
    /// What is wrong.
    pub problem: Problem,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.problem)
    }
}

/// What is wrong with a cutting plan.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Problem {
    /// A pattern names a panel that is not in the instance.
    #[error("unknown panel \"{0}\"")]
    UnknownPanel(String),
    /// A pattern is repeated zero times.
    #[error("pattern is repeated 0 times")]
    ZeroRepeat,
    /// A pattern contains no parts.
    #[error("pattern cuts no parts")]
    NoParts,
    /// The root of a pattern is not the whole panel as an uncut piece at
    /// stage 0.
    #[error("the root must be the whole panel: a piece at stage 0 at (0, 0) of the panel's size")]
    RootIsNotThePanel,
    /// The trims cut off first are missing, misplaced or of the wrong
    /// size: the panel must be cut into one trim per edge whose strip is
    /// cut off first, then the trimmed panel.
    #[error("the panel must first be cut into its trims ({expected}), then the trimmed panel")]
    WrongTrims {
        /// The trims expected, e.g. "left 10, bottom 5".
        expected: String,
    },
    /// A trim where the panel has none, or below the panel.
    #[error("a trim may only be a child of the panel")]
    MisplacedTrim,
    /// A part reaches into a trim.
    #[error("part \"{part_id}\" reaches into the {edge} trim")]
    PartInTrim {
        /// The part's name.
        part_id: String,
        /// "left", "right", "bottom" or "top".
        edge: &'static str,
    },
    /// An offcut or leftover is narrower than the minimum offcut width,
    /// measured across the cut that frees it.
    #[error(
        "{kind} is {width} wide across the cut that frees it, less than the minimum offcut width {minimum}"
    )]
    OffcutTooNarrow {
        /// "offcut" or "leftover".
        kind: &'static str,
        /// Its size across that cut.
        width: Length,
        /// The instance's minimum offcut width.
        minimum: Length,
    },
    /// A child's stage is not its parent's stage plus one.
    #[error("stage {found}, expected {expected}")]
    WrongStage {
        /// Parent's stage plus one.
        expected: u32,
        /// The child's stage.
        found: u32,
    },
    /// A part, offcut or leftover has children.
    #[error("only pieces can be cut further")]
    LeafWithChildren,
    /// A piece other than the panel has no children.
    #[error("piece is not cut into anything")]
    PieceWithoutChildren,
    /// A rectangle has a zero or negative size.
    #[error("size must be positive")]
    EmptyRectangle,
    /// A child does not span its parent across the cut direction.
    #[error("does not span the full {axis} of its parent piece")]
    NotSpanningParent {
        /// "length" or "width".
        axis: &'static str,
    },
    /// A child does not start where the previous cut ends.
    #[error("starts at {found}, but the previous cut ends at {expected}")]
    WrongPosition {
        /// Where it must start: the parent's edge, or the previous child's
        /// end plus the kerf.
        expected: Length,
        /// Where it starts.
        found: Length,
    },
    /// Children reach beyond their parent.
    #[error("children end at {children_end}, beyond the piece's edge at {piece_end}")]
    BeyondParent {
        /// Where the last child ends.
        children_end: Length,
        /// Where the parent ends.
        piece_end: Length,
    },
    /// More than one kerf of material is left after the last child without
    /// being part of the tree.
    #[error("{remainder} after the last child is not accounted for (more than one kerf)")]
    UnaccountedRemainder {
        /// The uncovered size.
        remainder: Length,
    },
    /// A part node names a part that is not in the instance.
    #[error("unknown part \"{0}\"")]
    UnknownPart(String),
    /// A part node's size does not match its part (taking `rotated` into
    /// account).
    #[error("size {length} × {width} does not match part \"{part_id}\" (rotated: {rotated})")]
    WrongPartSize {
        /// The part's name.
        part_id: String,
        /// Whether the node says the part is turned.
        rotated: bool,
        /// The node's length.
        length: Length,
        /// The node's width.
        width: Length,
    },
    /// A part with grain is turned 90°.
    #[error("part \"{0}\" has grain and must not be turned")]
    GrainTurned(String),
    /// A leftover that is not the last first-stage piece of the last,
    /// unrepeated pattern.
    #[error("a leftover may only be the last first-stage piece of the last panel, cut once")]
    MisplacedLeftover,
    /// A pattern's first cuts run in a direction the saw settings forbid.
    #[error("first cuts run {found}, but the saw settings allow {allowed}")]
    WrongFirstCutDirection {
        /// The saw setting.
        allowed: FirstCuts,
        /// The pattern's direction.
        found: CutDirection,
    },
    /// A pattern uses more stages than allowed.
    #[error("uses {used} cutting stages, at most {allowed} allowed")]
    TooManyStages {
        /// The instance's limit.
        allowed: u32,
        /// The stages this pattern needs.
        used: u32,
    },
    /// With "one part per strip", a strip holds different parts.
    #[error("strip holds different parts ({0}), but only one part per strip is allowed")]
    MixedStrip(String),
    /// A panel is used more often than available.
    #[error("panel \"{panel_id}\" used {used} times, only {available} available")]
    TooManyPanels {
        /// The panel's name.
        panel_id: String,
        /// Panels used, counting repeats.
        used: u32,
        /// The panel's quantity.
        available: u32,
    },
    /// An open-dimension plan uses more than one panel.
    #[error("\"Shortest length/width from one panel\" may use only one panel, uses {0}")]
    OpenDimensionUsesSeveralPanels(u32),
    /// A part is cut more often than needed.
    #[error("part \"{part_id}\" cut {cut} times, only {wanted} wanted")]
    TooManyParts {
        /// The part's name.
        part_id: String,
        /// Copies cut, counting repeats.
        cut: u32,
        /// The part's quantity.
        wanted: u32,
    },
    /// A part is cut less often than needed, for an objective that must cut
    /// every part.
    #[error("part \"{part_id}\" cut {cut} times, but all {wanted} must be cut")]
    MissingParts {
        /// The part's name.
        part_id: String,
        /// Copies cut, counting repeats.
        cut: u32,
        /// The part's quantity.
        wanted: u32,
    },
}
