//! `openapi.json`: every field is described, and instance fields carry
//! the exact help text of `docs/FIELDS.md`.

use std::collections::BTreeMap;

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

/// The help text of each field in `docs/FIELDS.md`, by section.
fn help_texts() -> BTreeMap<(String, String), String> {
    let text = include_str!("../../../docs/FIELDS.md");
    let mut section = String::new();
    let mut texts = BTreeMap::new();
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("### ") {
            section = title.to_string();
        } else if section.is_empty() || line.starts_with("## Later") {
            continue;
        } else if let Some(row) = line.strip_prefix("| `") {
            let cells: Vec<&str> = row.split(" | ").collect();
            let field = cells[0].trim_end_matches('`').to_string();
            texts.insert((section.clone(), field), cells[2].to_string());
        }
        if line.starts_with("## Later") {
            section.clear();
        }
    }
    texts
}

#[test]
fn instance_fields_carry_the_help_text_of_fields_md() {
    let document = document();
    let schemas = &document["components"]["schemas"];
    let texts = help_texts();
    assert!(texts.len() >= 17, "{texts:?}");
    for ((section, field), help) in &texts {
        let schema = match (section.as_str(), field.as_str()) {
            ("Job", "time_limit") => "JobRequest",
            ("Job", _) => "Instance",
            ("Panels (stock sheets)", _) => "Panel",
            ("Parts", _) => "Part",
            ("Saw and cutting", _) => "Saw",
            _ => panic!("unknown section {section}"),
        };
        let description = &schemas[schema]["properties"][field]["description"];
        assert_eq!(
            description.as_str(),
            Some(help.as_str()),
            "{schema}.{field} differs from FIELDS.md"
        );
    }
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
