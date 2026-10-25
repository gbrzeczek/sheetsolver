//! Unlimited stages: plans deeper than 3 levels, whose cuts change
//! direction at every stage, from panels cut either way first, are
//! accepted; the same plans break a stage limit.

// The plans below nest deeper than `json!` expands by default.
#![recursion_limit = "256"]

use checker::{Problem, check};
use model::{Instance, Solution};
use serde_json::{Value, json};

/// A 100 × 50 mm panel, 2 mm kerf, two of them, with `stages`.
fn instance(stages: Value) -> Instance {
    let instance: Instance = serde_json::from_value(json!({
        "objective": "bin_packing",
        "saw": { "kerf": 2, "stages": stages, "cut_type": "exact" },
        "panels": [{ "id": "P", "length": 100, "width": 50, "quantity": 2 }],
        "parts": [
            { "id": "A", "length": 30, "width": 20, "quantity": 2, "grain": true },
            { "id": "B", "length": 16, "width": 10, "quantity": 2, "grain": true },
            { "id": "C", "length": 60, "width": 50, "quantity": 1, "grain": true }
        ]
    }))
    .unwrap();
    instance.validate().unwrap();
    instance
}

/// Panel 1, first cuts across the length, 6 levels: a 48 mm strip; in
/// it, a 30 mm piece across; in that, A and a 16 mm strip; in that, B and
/// an offcut; then B again one level down, after a piece that passes a
/// stage unchanged. Panel 2, first cuts along the length: C on its own.
fn deep_plan() -> Solution {
    serde_json::from_value(json!({ "patterns": [
        {
            "panel_id": "P", "repeat": 1, "first_cuts": "across_length",
            "root": { "kind": "piece", "x": 0, "y": 0, "length": 100, "width": 50, "stage": 0, "children": [
                { "kind": "piece", "x": 0, "y": 0, "length": 48, "width": 50, "stage": 1, "children": [
                    { "kind": "piece", "x": 0, "y": 0, "length": 48, "width": 20, "stage": 2, "children": [
                        { "kind": "part", "part_id": "A", "rotated": false, "x": 0, "y": 0, "length": 30, "width": 20, "stage": 3 },
                        { "kind": "piece", "x": 32, "y": 0, "length": 16, "width": 20, "stage": 3, "children": [
                            { "kind": "part", "part_id": "B", "rotated": false, "x": 32, "y": 0, "length": 16, "width": 10, "stage": 4 },
                            { "kind": "offcut", "x": 32, "y": 12, "length": 16, "width": 8, "stage": 4 }
                        ]}
                    ]},
                    { "kind": "piece", "x": 0, "y": 22, "length": 48, "width": 28, "stage": 2, "children": [
                        { "kind": "piece", "x": 0, "y": 22, "length": 30, "width": 28, "stage": 3, "children": [
                            { "kind": "part", "part_id": "A", "rotated": false, "x": 0, "y": 22, "length": 30, "width": 20, "stage": 4 },
                            { "kind": "offcut", "x": 0, "y": 44, "length": 30, "width": 6, "stage": 4 }
                        ]},
                        { "kind": "piece", "x": 32, "y": 22, "length": 16, "width": 28, "stage": 3, "children": [
                            { "kind": "piece", "x": 32, "y": 22, "length": 16, "width": 28, "stage": 4, "children": [
                                { "kind": "piece", "x": 32, "y": 22, "length": 16, "width": 28, "stage": 5, "children": [
                                    { "kind": "part", "part_id": "B", "rotated": false, "x": 32, "y": 22, "length": 16, "width": 10, "stage": 6 },
                                    { "kind": "offcut", "x": 32, "y": 34, "length": 16, "width": 16, "stage": 6 }
                                ]}
                            ]}
                        ]}
                    ]}
                ]},
                { "kind": "offcut", "x": 50, "y": 0, "length": 50, "width": 50, "stage": 1 }
            ]}
        },
        {
            "panel_id": "P", "repeat": 1, "first_cuts": "along_length",
            "root": { "kind": "piece", "x": 0, "y": 0, "length": 100, "width": 50, "stage": 0, "children": [
                { "kind": "piece", "x": 0, "y": 0, "length": 100, "width": 50, "stage": 1, "children": [
                    { "kind": "part", "part_id": "C", "rotated": false, "x": 0, "y": 0, "length": 60, "width": 50, "stage": 2 },
                    { "kind": "offcut", "x": 62, "y": 0, "length": 38, "width": 50, "stage": 2 }
                ]}
            ]}
        }
    ]}))
    .unwrap()
}

#[test]
fn deep_plans_with_turning_cuts_are_accepted_with_unlimited_stages() {
    let report = check(&instance(json!("unlimited")), &deep_plan());
    assert!(report.is_feasible(), "{:?}", report.violations);
    assert_eq!(report.metrics.panels_used, 2);
    assert_eq!(report.metrics.parts_cut, 5);
}

#[test]
fn the_same_plan_breaks_a_limit_of_three_stages() {
    let report = check(&instance(json!(3)), &deep_plan());
    let problems: Vec<Problem> = report.violations.into_iter().map(|v| v.problem).collect();
    // The deepest cut that changes a piece is at stage 6: pieces that
    // pass a stage unchanged cost no cut, but keep their stage number.
    assert_eq!(
        problems,
        vec![Problem::TooManyStages {
            allowed: 3,
            used: 6
        }]
    );
}

#[test]
fn cuts_must_still_turn_at_every_stage() {
    // A stage-4 piece cut along x, like its parent's stage: B's piece
    // then lies beside, not above, the offcut.
    let mut plan = deep_plan();
    let strip = &mut plan.patterns[0].root.children[0].children[0].children[1];
    let offcut = &mut strip.children[1];
    offcut.x = model::Length::from_mm(32);
    offcut.y = model::Length::from_mm(0);
    let report = check(&instance(json!("unlimited")), &plan);
    assert!(!report.is_feasible());
}
