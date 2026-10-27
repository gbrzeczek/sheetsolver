//! Trims and the minimum offcut width: a small valid plan, then one broken
//! plan per rule.

#![recursion_limit = "256"]

use checker::{Problem, check};
use model::{Instance, Length, Solution};
use serde_json::{Value, json};

/// A 100 × 50 mm panel with a 5 mm left trim cut off first and a 5 mm
/// right trim left on the offcut, 2 mm kerf, offcuts at least 10 mm.
fn instance() -> Value {
    json!({
        "objective": "bin_packing",
        "saw": { "kerf": 2, "stages": 3, "cut_type": "non_exact", "minimum_offcut_width": 10 },
        "panels": [{ "id": "P", "length": 100, "width": 50, "quantity": 1, "trims": {
            "left": { "width": 5, "cut": "cut_off_first" },
            "right": { "width": 5, "cut": "left_on_offcut" }
        }}],
        "parts": [
            { "id": "A", "length": 40, "width": 18, "quantity": 2, "grain": true },
            { "id": "B", "length": 30, "width": 50, "quantity": 1 }
        ]
    })
}

/// The panel is cut into its left trim and the trimmed panel (5 to 100).
/// That is cut across the length into a strip of two A parts with a 10 mm
/// offcut, part B, and a 21 mm offcut that holds the right trim.
fn solution() -> Value {
    json!({ "patterns": [{
        "panel_id": "P", "repeat": 1, "first_cuts": "across_length",
        "root": { "kind": "piece", "x": 0, "y": 0, "length": 100, "width": 50, "stage": 0, "children": [
            { "kind": "trim", "x": 0, "y": 0, "length": 5, "width": 50, "stage": 0 },
            { "kind": "piece", "x": 5, "y": 0, "length": 95, "width": 50, "stage": 0, "children": [
                { "kind": "piece", "x": 5, "y": 0, "length": 40, "width": 50, "stage": 1, "children": [
                    { "kind": "part", "part_id": "A", "rotated": false, "x": 5, "y": 0, "length": 40, "width": 18, "stage": 2 },
                    { "kind": "part", "part_id": "A", "rotated": false, "x": 5, "y": 20, "length": 40, "width": 18, "stage": 2 },
                    { "kind": "offcut", "x": 5, "y": 40, "length": 40, "width": 10, "stage": 2 }
                ]},
                { "kind": "part", "part_id": "B", "rotated": false, "x": 47, "y": 0, "length": 30, "width": 50, "stage": 1 },
                { "kind": "offcut", "x": 79, "y": 0, "length": 21, "width": 50, "stage": 1 }
            ]}
        ]}
    }]})
}

fn report(instance: &Value, solution: &Value) -> checker::Report {
    let instance: Instance = serde_json::from_value(instance.clone()).unwrap();
    instance.validate().unwrap();
    let solution: Solution = serde_json::from_value(solution.clone()).unwrap();
    check(&instance, &solution)
}

fn problems(instance: &Value, solution: &Value) -> Vec<Problem> {
    report(instance, solution)
        .violations
        .into_iter()
        .map(|v| v.problem)
        .collect()
}

fn mm(mm: i64) -> Length {
    Length::from_mm(mm)
}

/// The trimmed panel, cut by the first cuts.
fn trimmed(solution: &mut Value) -> &mut Value {
    &mut solution["patterns"][0]["root"]["children"][1]
}

#[test]
fn the_trimmed_plan_is_feasible_and_its_area_excludes_the_trims() {
    let report = report(&instance(), &solution());
    assert_eq!(report.violations, vec![]);
    let m = report.metrics;
    // upstream: a panel's area is the part inside its trims, soft or hard.
    assert_eq!(m.panel_area.mm2(), 90.0 * 50.0);
    assert_eq!(m.part_area.mm2(), 2.0 * 720.0 + 1500.0);
    assert_eq!(m.waste_area.mm2(), 4500.0 - 2940.0);
    // The offcut holding the right trim is a first-stage piece.
    assert_eq!((m.used_length, m.used_width), (mm(100), mm(50)));
}

#[test]
fn a_trim_cut_off_first_must_be_cut_first() {
    let mut s = solution();
    let root = &mut s["patterns"][0]["root"];
    let trimmed = root["children"][1].clone();
    root["children"] = json!([trimmed]);
    assert_eq!(
        problems(&instance(), &s),
        vec![Problem::WrongTrims {
            expected: "left 5 mm".into()
        }]
    );
}

#[test]
fn a_trim_must_have_its_width() {
    let mut s = solution();
    s["patterns"][0]["root"]["children"][0]["length"] = json!(4);
    assert!(matches!(
        problems(&instance(), &s)[..],
        [Problem::WrongTrims { .. }]
    ));
}

#[test]
fn a_trim_left_on_the_offcut_is_not_cut_off() {
    let mut i = instance();
    i["panels"][0]["trims"]["left"]["cut"] = json!("left_on_offcut");
    // Its strip stays on the offcut beside it, so the panel itself is cut
    // by the first cuts: a trim there is misplaced.
    let found = problems(&i, &solution());
    assert!(found.contains(&Problem::MisplacedTrim), "{found:?}");
}

#[test]
fn a_trim_below_the_panel_is_misplaced() {
    let mut s = solution();
    trimmed(&mut s)["children"][2]["kind"] = json!("trim");
    assert_eq!(problems(&instance(), &s), vec![Problem::MisplacedTrim]);
}

#[test]
fn parts_stay_out_of_a_trim_left_on_the_offcut() {
    let mut i = instance();
    i["panels"][0]["trims"]["right"]["width"] = json!(30);
    assert_eq!(
        problems(&i, &solution()),
        vec![Problem::PartInTrim {
            part_id: "B".into(),
            edge: "right"
        }]
    );
}

#[test]
fn offcuts_are_at_least_the_minimum_width_across_the_cut_that_frees_them() {
    let mut i = instance();
    i["saw"]["minimum_offcut_width"] = json!(11);
    assert_eq!(
        problems(&i, &solution()),
        vec![Problem::OffcutTooNarrow {
            kind: "offcut",
            width: mm(10),
            minimum: mm(11)
        }]
    );
    // Along the cut, an offcut may be narrower: the 21 mm offcut is 50 mm
    // along its cut, the strip's offcut 40 mm.
    i["saw"]["minimum_offcut_width"] = json!(10);
    assert_eq!(problems(&i, &solution()), vec![]);
}

#[test]
fn the_leftover_is_at_least_the_minimum_width_too() {
    let mut i = instance();
    i["saw"]["minimum_offcut_width"] = json!(22);
    let mut s = solution();
    trimmed(&mut s)["children"][2]["kind"] = json!("leftover");
    let found = problems(&i, &s);
    assert!(found.contains(&Problem::OffcutTooNarrow {
        kind: "leftover",
        width: mm(21),
        minimum: mm(22)
    }));
}
