//! Instance and solution types for sheetsolver.
//!
//! These types are the public contract of the project: the solver reads
//! them, the checker judges them and the API serves them as JSON. Field
//! names and doc comments follow `docs/FIELDS.md`.

mod instance;
mod length;
#[cfg(feature = "openapi")]
mod openapi;
mod solution;

pub use instance::{
    CutType, FieldError, FirstCuts, Instance, MAX_PANELS, MAX_PARTS, MAX_QUANTITY, MAX_ROWS,
    Objective, Panel, Part, Saw, Stages, Trim, TrimCut, Trims, ValidationError,
};
pub use length::{Area, Length};
pub use solution::{CutDirection, Metrics, Node, NodeKind, Pattern, Solution};
