//! Instance and solution types for sheetsolver.
//!
//! These types are the public contract of the project: the solver reads
//! them, the checker judges them and the API serves them as JSON. Field
//! names and doc comments follow `docs/FIELDS.md`.

mod instance;
mod length;
mod solution;

pub use instance::{
    CutType, FirstCuts, Instance, Objective, Panel, Part, Saw, Stages, ValidationError,
};
pub use length::{Area, Length};
pub use solution::{CutDirection, Metrics, Node, NodeKind, Pattern, Solution};
