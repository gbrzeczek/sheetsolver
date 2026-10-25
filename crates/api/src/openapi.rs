//! The OpenAPI document, generated from the handlers and the types.

use serde_json::Value;
use utoipa::OpenApi;

use crate::routes;
use crate::types::{PlanReport, StateEvent};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "sheetsolver",
        description = "Cutting plans for sheet material: submit the parts, panels and saw \
            settings, follow the search as it finds better plans, stop it when you like. \
            See docs/API.md in the repository.",
    ),
    paths(
        routes::submit,
        routes::status,
        routes::cancel,
        routes::events,
        routes::openapi
    ),
    // The event payloads appear in no response body; list them so that
    // clients get their types.
    components(schemas(PlanReport, StateEvent))
)]
struct ApiDoc;

/// The OpenAPI 3.1 description of the API, as served at `/openapi.json`.
pub fn document() -> Value {
    let mut document = serde_json::to_value(ApiDoc::openapi()).expect("the document serializes");
    hoist_descriptions(&mut document);
    unwrap_descriptions(&mut document);
    document
}

/// Joins the lines of each description into paragraphs.
///
/// Doc comments are wrapped at 80 columns; the UI shows descriptions as
/// help text, which must read exactly as in `docs/FIELDS.md`. A blank
/// line still separates paragraphs.
fn unwrap_descriptions(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object.iter_mut() {
                match value {
                    Value::String(text) if key == "description" => *text = unwrap(text),
                    _ => unwrap_descriptions(value),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(unwrap_descriptions),
        _ => {}
    }
}

fn unwrap(text: &str) -> String {
    text.split("\n\n")
        .map(|paragraph| {
            paragraph
                .split('\n')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Moves the description of an optional reference to the property.
///
/// utoipa writes an `Option<T>` field whose `T` is a component as
/// `oneOf: [{type: null}, {$ref, description}]`, with the field's doc
/// comment inside the `oneOf`. Clients look for it on the property.
fn hoist_descriptions(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get_mut("properties") {
                for property in properties.values_mut() {
                    hoist_one(property);
                }
            }
            object.values_mut().for_each(hoist_descriptions);
        }
        Value::Array(items) => items.iter_mut().for_each(hoist_descriptions),
        _ => {}
    }
}

fn hoist_one(property: &mut Value) {
    let Some(property) = property.as_object_mut() else {
        return;
    };
    if property.contains_key("description") {
        return;
    }
    let description = property
        .get_mut("oneOf")
        .and_then(Value::as_array_mut)
        .and_then(|branches| {
            branches
                .iter_mut()
                .find_map(|branch| branch.as_object_mut()?.remove("description"))
        });
    if let Some(description) = description {
        property.insert("description".into(), description);
    }
}
