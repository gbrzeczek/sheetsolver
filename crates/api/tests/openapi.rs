//! `openapi.json`: every field is described, and instance fields carry
//! the exact help text of `docs/FIELDS.md`.

use serde_json::Value;

fn document() -> Value {
    api::openapi_document()
}

/// Every property under `value`, with its path, for the schemas at any
/// depth (including `oneOf`/`allOf` branches and list items).
fn properties<'a>(value: &'a Value, path: &str, found: &mut Vec<(String, &'a Value)>) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Object(props)) = object.get("properties") {
                for (name, property) in props {
                    found.push((format!("{path}.{name}"), property));
                }
            }
            for (key, child) in object {
                properties(child, &format!("{path}/{key}"), found);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                properties(item, &format!("{path}[{index}]"), found);
            }
        }
        _ => {}
    }
}

#[test]
fn every_field_has_a_description() {
    let document = document();
    assert_eq!(document["openapi"], "3.1.0");
    let mut found = Vec::new();
    properties(&document, "", &mut found);
    assert!(found.len() > 50, "only {} properties found", found.len());
    let missing: Vec<&str> = found
        .iter()
        .filter(|(_, property)| {
            property["description"]
                .as_str()
                .is_none_or(|text| text.trim().is_empty())
        })
        .map(|(path, _)| path.as_str())
        .collect();
    assert_eq!(missing, Vec::<&str>::new(), "fields without a description");

    // Path parameters too.
    for (path, item) in document["paths"].as_object().unwrap() {
        for (method, operation) in item.as_object().unwrap() {
            for parameter in operation["parameters"].as_array().into_iter().flatten() {
                assert!(parameter["description"].is_string(), "{method} {path}");
            }
        }
    }
}

/// A field row of `docs/FIELDS.md`.
struct FieldRow {
    /// The schema whose property it is.
    schema: &'static str,
    /// The field's name.
    field: String,
    /// The UI label.
    label: String,
    /// The short help.
    short_help: String,
    /// The help text.
    help: String,
}

/// The cells of a table row, if `line` is one.
fn cells(line: &str) -> Option<Vec<&str>> {
    let row = line.strip_prefix("| `")?;
    Some(
        row.trim_end_matches(" |")
            .split(" | ")
            .map(|cell| cell.trim_matches('`'))
            .collect(),
    )
}

const FIELDS_MD: &str = include_str!("../../../docs/FIELDS.md");

/// The rows of the tables under "## Fields".
fn field_rows() -> Vec<FieldRow> {
    let mut chapter = "";
    let mut section = "";
    let mut rows = Vec::new();
    for line in FIELDS_MD.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            chapter = title;
        } else if let Some(title) = line.strip_prefix("### ") {
            section = title;
        } else if chapter == "Fields" {
            if let Some(cells) = cells(line) {
                let schema = match (section, cells[0]) {
                    ("Job", "time_limit" | "name") => "JobRequest",
                    ("Job", _) => "Instance",
                    ("Panels (stock sheets)", _) => "Panel",
                    ("Parts", _) => "Part",
                    ("Saw and cutting", _) => "Saw",
                    _ => panic!("unknown section {section}"),
                };
                rows.push(FieldRow {
                    schema,
                    field: cells[0].to_string(),
                    label: cells[1].to_string(),
                    short_help: cells[2].to_string(),
                    help: cells[3].to_string(),
                });
            }
        }
    }
    rows
}

/// The rows of the table under "## Option names": field, value, name.
fn option_rows() -> Vec<(String, String, String)> {
    let mut chapter = "";
    let mut rows = Vec::new();
    for line in FIELDS_MD.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            chapter = title;
        } else if chapter == "Option names" {
            if let Some(cells) = cells(line) {
                rows.push((cells[0].into(), cells[1].into(), cells[2].into()));
            }
        }
    }
    rows
}

#[test]
fn input_fields_carry_the_help_text_and_label_of_fields_md() {
    let document = document();
    let schemas = &document["components"]["schemas"];
    let rows = field_rows();
    assert_eq!(rows.len(), 18);
    for row in &rows {
        let property = &schemas[row.schema]["properties"][&row.field];
        let name = format!("{}.{}", row.schema, row.field);
        assert_eq!(
            property["description"].as_str(),
            Some(row.help.as_str()),
            "{name} help text"
        );
        assert_eq!(
            property["title"].as_str(),
            Some(row.label.as_str()),
            "{name} label"
        );
        assert_eq!(
            property["x-short-help"].as_str(),
            Some(row.short_help.as_str()),
            "{name} short help"
        );
    }
}

#[test]
fn short_help_is_one_sentence_of_at_most_15_words() {
    for row in field_rows() {
        let name = format!("{}.{}", row.schema, row.field);
        let words = row.short_help.split_whitespace().count();
        assert!(words <= 15, "{name}: {words} words");
        assert!(row.short_help.ends_with('.'), "{name} ends a sentence");
        assert!(
            !row.short_help.trim_end_matches('.').contains(". "),
            "{name} is one sentence"
        );
        assert!(
            row.short_help.len() < row.help.len(),
            "{name} is shorter than its help text"
        );
    }
}

#[test]
fn every_option_is_named_as_in_fields_md() {
    let document = document();
    let schemas = &document["components"]["schemas"];
    let mut found = Vec::new();
    for (field, schema) in [
        ("objective", "Objective"),
        ("stages", "Stages"),
        ("cut_type", "CutType"),
        ("first_cuts", "FirstCuts"),
    ] {
        for option in schemas[schema]["oneOf"].as_array().unwrap() {
            let value = match &option["const"] {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            found.push((
                field.to_string(),
                value,
                option["title"].as_str().unwrap().to_string(),
            ));
        }
    }
    assert_eq!(found, option_rows());
}

#[test]
fn the_event_payloads_are_components() {
    let document = document();
    for name in [
        "PlanReport",
        "StateEvent",
        "JobStatus",
        "ErrorBody",
        "JobRequest",
    ] {
        assert!(
            document["components"]["schemas"][name].is_object(),
            "{name}"
        );
    }
    assert_eq!(
        document["paths"]["/jobs/{id}/events"]["get"]["responses"]["200"]["content"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["text/event-stream"]
    );
}
