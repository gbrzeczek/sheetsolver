//! One broken plan per rule: each test starts from a small valid plan,
//! breaks one thing, and checks the checker reports exactly that.

use checker::{Problem, check};
use model::{Instance, Length, Solution};
use serde_json::{Value, json};

/// A 100 × 50 mm panel, 2 mm kerf, 3 stages with trimming.
fn instance() -> Value {
    json!({
        "objective": "bin_packing",
        "saw": { "kerf": 2, "stages": 3, "cut_type": "non_exact" },
        "panels": [{ "id": "P", "length": 100, "width": 50, "quantity": 1 }],
        "parts": [
            { "id": "A", "length": 40, "width": 20, "quantity": 2, "grain": true },
            { "id": "B", "length": 30, "width": 50, "quantity": 1 }
        ]
    })
}

/// Valid plan: first cuts across the length give a 40 mm strip (two A
/// parts and an offcut), part B, and an offcut. Each cut removes 2 mm.
fn solution() -> Value {
    json!({ "patterns": [{
        "panel_id": "P", "repeat": 1, "first_cuts": "across_length",
        "root": { "kind": "piece", "x": 0, "y": 0, "length": 100, "width": 50, "stage": 0, "children": [
            { "kind": "piece", "x": 0, "y": 0, "length": 40, "width": 50, "stage": 1, "children": [
                { "kind": "part", "part_id": "A", "rotated": false, "x": 0, "y": 0, "length": 40, "width": 20, "stage": 2 },
                { "kind": "part", "part_id": "A", "rotated": false, "x": 0, "y": 22, "length": 40, "width": 20, "stage": 2 },
                { "kind": "offcut", "x": 0, "y": 44, "length": 40, "width": 6, "stage": 2 }
            ]},
            { "kind": "part", "part_id": "B", "rotated": false, "x": 42, "y": 0, "length": 30, "width": 50, "stage": 1 },
            { "kind": "offcut", "x": 74, "y": 0, "length": 26, "width": 50, "stage": 1 }
        ]}
    }]})
}

/// The first strip (the piece holding the two A parts).
fn strip(solution: &mut Value) -> &mut Value {
    &mut solution["patterns"][0]["root"]["children"][0]
}

fn problems(instance: &Value, solution: &Value) -> Vec<Problem> {
    let instance: Instance = serde_json::from_value(instance.clone()).unwrap();
    instance.validate().unwrap();
    let solution: Solution = serde_json::from_value(solution.clone()).unwrap();
    check(&instance, &solution)
        .violations
        .into_iter()
        .map(|v| v.problem)
        .collect()
}

#[track_caller]
fn assert_only(instance: &Value, solution: &Value, expected: Problem) {
    assert_eq!(problems(instance, solution), vec![expected]);
}

fn mm(mm: i64) -> Length {
    Length::from_mm(mm)
}

#[test]
fn the_base_plan_is_feasible_with_the_expected_metrics() {
    let instance: Instance = serde_json::from_value(instance()).unwrap();
    let solution: Solution = serde_json::from_value(solution()).unwrap();
    let report = check(&instance, &solution);
    assert_eq!(report.violations, vec![]);
    let m = report.metrics;
    assert_eq!((m.panels_used, m.parts_cut), (1, 3));
    assert_eq!(m.part_area.mm2(), 2.0 * 800.0 + 1500.0);
    assert_eq!(m.waste_area.mm2(), 5000.0 - 3100.0);
    assert_eq!(m.total_value, 3100.0);
    assert_eq!((m.used_length, m.used_width), (mm(100), mm(50)));
}

#[test]
fn unknown_panel() {
    let mut s = solution();
    s["patterns"][0]["panel_id"] = json!("Q");
    assert_only(&instance(), &s, Problem::UnknownPanel("Q".into()));
}

#[test]
fn zero_repeat() {
    let mut s = solution();
    s["patterns"][0]["repeat"] = json!(0);
    let found = problems(&instance(), &s);
    assert!(found.contains(&Problem::ZeroRepeat), "{found:?}");
}

#[test]
fn pattern_without_parts() {
    let mut s = solution();
    s["patterns"][0]["root"]["children"] = json!([
        { "kind": "offcut", "x": 0, "y": 0, "length": 100, "width": 50, "stage": 1 }
    ]);
    let found = problems(&instance(), &s);
    assert!(found.contains(&Problem::NoParts), "{found:?}");
}

#[test]
fn root_must_be_the_panel() {
    let mut s = solution();
    s["patterns"][0]["root"]["width"] = json!(60);
    let found = problems(&instance(), &s);
    assert!(found.contains(&Problem::RootIsNotThePanel), "{found:?}");
}

#[test]
fn wrong_stage() {
    let mut s = solution();
    strip(&mut s)["children"][2]["stage"] = json!(3);
    assert_only(
        &instance(),
        &s,
        Problem::WrongStage {
            expected: 2,
            found: 3,
        },
    );
}

#[test]
fn leaf_with_children() {
    let mut s = solution();
    let offcut = &mut strip(&mut s)["children"][2];
    offcut["children"] =
        json!([{ "kind": "offcut", "x": 0, "y": 44, "length": 40, "width": 6, "stage": 3 }]);
    assert_only(&instance(), &s, Problem::LeafWithChildren);
}

#[test]
fn piece_without_children() {
    let mut s = solution();
    s["patterns"][0]["root"]["children"][2]["kind"] = json!("piece");
    assert_only(&instance(), &s, Problem::PieceWithoutChildren);
}

#[test]
fn child_must_span_its_parent() {
    let mut s = solution();
    strip(&mut s)["children"][2]["length"] = json!(39);
    assert_only(
        &instance(),
        &s,
        Problem::NotSpanningParent { axis: "length" },
    );
}

#[test]
fn pieces_must_be_one_kerf_apart() {
    let mut s = solution();
    // 1 mm after the first part instead of the 2 mm kerf.
    strip(&mut s)["children"][1]["y"] = json!(21);
    let found = problems(&instance(), &s);
    assert!(
        found.contains(&Problem::WrongPosition {
            expected: mm(22),
            found: mm(21)
        }),
        "{found:?}"
    );
}

#[test]
fn children_must_not_reach_beyond_the_piece() {
    let mut s = solution();
    strip(&mut s)["children"][2]["width"] = json!(7);
    assert_only(
        &instance(),
        &s,
        Problem::BeyondParent {
            children_end: mm(51),
            piece_end: mm(50),
        },
    );
}

#[test]
fn more_than_a_kerf_left_unaccounted() {
    let mut s = solution();
    strip(&mut s)["children"].as_array_mut().unwrap().pop();
    assert_only(
        &instance(),
        &s,
        Problem::UnaccountedRemainder { remainder: mm(8) },
    );
}

#[test]
fn up_to_one_kerf_may_be_left_at_the_edge() {
    let mut s = solution();
    // The offcut stops 2 mm (one kerf) short of the strip's edge.
    strip(&mut s)["children"][2]["width"] = json!(4);
    assert_eq!(problems(&instance(), &s), vec![]);
}

#[test]
fn unknown_part() {
    let mut s = solution();
    strip(&mut s)["children"][0]["part_id"] = json!("Z");
    let found = problems(&instance(), &s);
    assert!(
        found.contains(&Problem::UnknownPart("Z".into())),
        "{found:?}"
    );
}

#[test]
fn part_size_must_match() {
    let mut s = solution();
    s["patterns"][0]["root"]["children"][1]["rotated"] = json!(true);
    assert_only(
        &instance(),
        &s,
        Problem::WrongPartSize {
            part_id: "B".into(),
            rotated: true,
            length: mm(30),
            width: mm(50),
        },
    );
}

#[test]
fn parts_with_grain_must_not_be_turned() {
    let mut i = instance();
    i["parts"][0]["length"] = json!(20);
    i["parts"][0]["width"] = json!(20);
    let mut s = solution();
    // Make the strip 20 mm long so square A parts fit, and turn one.
    let strip = strip(&mut s);
    strip["length"] = json!(20);
    for child in strip["children"].as_array_mut().unwrap() {
        child["length"] = json!(20);
    }
    strip["children"][0]["rotated"] = json!(true);
    let root = &mut s["patterns"][0]["root"]["children"];
    root[1]["x"] = json!(22);
    root[2]["x"] = json!(54);
    root[2]["length"] = json!(46);
    assert_only(&i, &s, Problem::GrainTurned("A".into()));
}

#[test]
fn leftover_only_at_the_end_of_the_last_panel() {
    let mut s = solution();
    s["patterns"][0]["root"]["children"][2]["kind"] = json!("leftover");
    assert_eq!(problems(&instance(), &s), vec![]);

    strip(&mut s)["children"][2]["kind"] = json!("leftover");
    assert_only(&instance(), &s, Problem::MisplacedLeftover);
}

#[test]
fn trimming_cut_does_not_count_as_a_stage_with_non_exact() {
    let mut i = instance();
    i["saw"]["stages"] = json!(2);
    // Part A cut 40 × 20 from a 40 × 22 strip piece: one trimming cut.
    let mut s = solution();
    let strip = strip(&mut s);
    strip["children"][0] = json!(
        { "kind": "piece", "x": 0, "y": 0, "length": 40, "width": 20, "stage": 2, "children": [
            { "kind": "part", "part_id": "A", "rotated": false, "x": 0, "y": 0, "length": 38, "width": 20, "stage": 3 }
        ]}
    );
    i["parts"][0]["length"] = json!(38);
    i["parts"][0]["quantity"] = json!(1);
    i["parts"].as_array_mut().unwrap().push(json!(
        { "id": "A40", "length": 40, "width": 20, "quantity": 1 }
    ));
    strip["children"][1]["part_id"] = json!("A40");
    assert_eq!(problems(&i, &s), vec![]);

    i["saw"]["cut_type"] = json!("exact");
    assert_only(
        &i,
        &s,
        Problem::TooManyStages {
            allowed: 2,
            used: 3,
        },
    );
}

#[test]
fn too_many_stages() {
    let mut i = instance();
    i["saw"]["stages"] = json!(2);
    let mut s = solution();
    // Split the strip's offcut once more: a third stage.
    strip(&mut s)["children"][2] = json!(
        { "kind": "piece", "x": 0, "y": 44, "length": 40, "width": 6, "stage": 2, "children": [
            { "kind": "offcut", "x": 0, "y": 44, "length": 19, "width": 6, "stage": 3 },
            { "kind": "offcut", "x": 21, "y": 44, "length": 19, "width": 6, "stage": 3 }
        ]}
    );
    assert_only(
        &i,
        &s,
        Problem::TooManyStages {
            allowed: 2,
            used: 3,
        },
    );
}

#[test]
fn one_part_per_strip() {
    let mut i = instance();
    i["saw"]["stages"] = json!(2);
    i["saw"]["cut_type"] = json!("homogenous");
    assert_eq!(problems(&i, &solution()), vec![]);

    i["parts"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "C", "length": 40, "width": 6, "quantity": 1 }));
    let mut s = solution();
    strip(&mut s)["children"][2] = json!({ "kind": "part", "part_id": "C", "rotated": false, "x": 0, "y": 44, "length": 40, "width": 6, "stage": 2 });
    assert_only(&i, &s, Problem::MixedStrip("A, C".into()));
}

#[test]
fn too_many_panels() {
    let mut s = solution();
    s["patterns"][0]["repeat"] = json!(2);
    let found = problems(&instance(), &s);
    assert!(
        found.contains(&Problem::TooManyPanels {
            panel_id: "P".into(),
            used: 2,
            available: 1
        }),
        "{found:?}"
    );
}

#[test]
fn open_dimension_uses_one_panel() {
    let mut i = instance();
    i["objective"] = json!("open_dimension_x");
    i["panels"][0].as_object_mut().unwrap().remove("quantity");
    i["parts"][0]["quantity"] = json!(4);
    i["parts"][1]["quantity"] = json!(2);
    let mut s = solution();
    s["patterns"][0]["repeat"] = json!(2);
    assert_only(&i, &s, Problem::OpenDimensionUsesSeveralPanels(2));
}

#[test]
fn too_many_parts() {
    let mut i = instance();
    i["parts"][0]["quantity"] = json!(1);
    assert_only(
        &i,
        &solution(),
        Problem::TooManyParts {
            part_id: "A".into(),
            cut: 2,
            wanted: 1,
        },
    );
}

#[test]
fn missing_parts_only_matter_when_every_part_must_be_cut() {
    let mut i = instance();
    i["parts"][1]["quantity"] = json!(2);
    assert_only(
        &i,
        &solution(),
        Problem::MissingParts {
            part_id: "B".into(),
            cut: 1,
            wanted: 2,
        },
    );

    i["objective"] = json!("knapsack");
    assert_eq!(problems(&i, &solution()), vec![]);
}

#[test]
fn reports_every_violation_not_just_the_first() {
    let mut s = solution();
    s["patterns"][0]["panel_id"] = json!("Q");
    strip(&mut s)["children"][0]["part_id"] = json!("Z");
    let found = problems(&instance(), &s);
    assert!(found.contains(&Problem::UnknownPanel("Q".into())));
    assert!(found.contains(&Problem::UnknownPart("Z".into())));
}
