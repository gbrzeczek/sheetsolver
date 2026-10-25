//! Independent feasibility checker for cutting plans.
//!
//! Given an instance and a solution, reports every rule the solution
//! breaks and computes its metrics. It is written from the problem
//! definition, not from solver code, and must never depend on the
//! `treesearch` or `guillotine` crates (see `docs/TESTING.md`).
//!
//! Stage 1 fills this crate in (see `docs/STAGES.md`).
