//! Reading the body of `POST /jobs`, reporting every problem at once.
//!
//! The instance is read with `model`'s serde code, which stops at the
//! first problem. When it fails, the body is walked field by field
//! against a description of the instance's shape ([`INSTANCE`]) to find
//! every field with a wrong type or a missing value, so the woodworker
//! can fix them all in one go. The walk only finds problems; the
//! instance itself always comes from serde, so the two cannot disagree on
//! what a valid instance is. Should the walk miss a problem serde finds,
//! serde's own message is reported.

use model::{Instance, Length, Stages};
use serde_json::{Map, Value};

use crate::fields::{CUT_TYPE, Choice, FIRST_CUTS, OBJECTIVE};
use crate::types::{
    DEFAULT_TIME_LIMIT, FieldProblem, MAX_NAME_CHARS, MAX_TIME_LIMIT, MIN_TIME_LIMIT,
};

/// A job to run: a valid instance and its time limit in seconds.
#[derive(Debug, Clone)]
pub struct JobSpec {
    /// The job's name, trimmed; `None` if it has none.
    pub name: Option<String>,
    /// The instance; [`Instance::validate`] accepts it.
    pub instance: Instance,
    /// The time limit, in seconds.
    pub time_limit: u32,
}

/// Reads a job from the request body, already parsed as JSON. Returns
/// every problem found if it is not a valid job.
pub fn parse_job(body: &Value) -> Result<JobSpec, Vec<FieldProblem>> {
    let Some(body) = body.as_object() else {
        return Err(vec![FieldProblem {
            field: None,
            message: "the request must be a JSON object with an instance".into(),
        }]);
    };
    let mut problems = Vec::new();

    let time_limit = match body.get("time_limit") {
        None | Some(Value::Null) => Some(DEFAULT_TIME_LIMIT),
        Some(value) => match value.as_u64() {
            Some(seconds) if (MIN_TIME_LIMIT as u64..=MAX_TIME_LIMIT as u64).contains(&seconds) => {
                Some(seconds as u32)
            }
            _ => {
                problems.push(FieldProblem::at(
                    "time_limit",
                    format!(
                        "time limit must be a whole number of seconds from {MIN_TIME_LIMIT} to {MAX_TIME_LIMIT}"
                    ),
                ));
                None
            }
        },
    };

    let name = read_name(body.get("name"), &mut problems);

    let instance = match body.get("instance") {
        None | Some(Value::Null) => {
            problems.push(FieldProblem::at("instance", "this field is required"));
            None
        }
        Some(value) => read_instance(value, &mut problems),
    };

    match (instance, time_limit) {
        (Some(instance), Some(time_limit)) if problems.is_empty() => Ok(JobSpec {
            name,
            instance,
            time_limit,
        }),
        _ => Err(problems),
    }
}

/// Reads the optional job name: trimmed, and no name if blank. Adds a
/// problem if it is not one line of at most [`MAX_NAME_CHARS`]
/// characters.
fn read_name(value: Option<&Value>, problems: &mut Vec<FieldProblem>) -> Option<String> {
    let text = match value {
        None | Some(Value::Null) => return None,
        Some(Value::String(text)) => text.trim(),
        Some(_) => {
            problems.push(FieldProblem::at("name", "must be text"));
            return None;
        }
    };
    let length = text.chars().count();
    if length > MAX_NAME_CHARS {
        problems.push(FieldProblem::at(
            "name",
            format!("job name must be at most {MAX_NAME_CHARS} characters, not {length}"),
        ));
    } else if text.chars().any(char::is_control) {
        problems.push(FieldProblem::at(
            "name",
            "job name must be one line, without tabs or line breaks",
        ));
    }
    (!text.is_empty()).then(|| text.to_string())
}

/// Reads and validates the instance, adding its problems to `problems`.
fn read_instance(value: &Value, problems: &mut Vec<FieldProblem>) -> Option<Instance> {
    match serde_json::from_value::<Instance>(value.clone()) {
        Ok(instance) => match instance.validate_fields() {
            Ok(()) => Some(instance),
            Err(errors) => {
                problems.extend(errors.into_iter().map(|error| {
                    FieldProblem::at(format!("instance.{}", error.field), error.error.to_string())
                }));
                None
            }
        },
        Err(error) => {
            let before = problems.len();
            check_object(value, &INSTANCE, "instance", problems);
            if problems.len() == before {
                problems.push(FieldProblem::at("instance", error.to_string()));
            }
            None
        }
    }
}

/// The JSON type a field must have.
#[derive(Debug, Clone, Copy)]
enum Kind {
    /// A string.
    Text,
    /// A [`Length`] in millimetres.
    Length,
    /// A whole number that fits a `u32`.
    Count,
    /// Any number.
    Number,
    /// `true` or `false`.
    Flag,
    /// One of the string options of this field.
    Choice(&'static Choice),
    /// [`Stages`].
    Stages,
    /// An object of this shape.
    Object(&'static Shape),
    /// A list of objects of this shape.
    List(&'static Shape),
}

/// Whether a field may be left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presence {
    /// It must be given.
    Required,
    /// It may be left out, but not be `null`.
    Optional,
    /// It may be left out or `null`.
    Nullable,
}

/// One field of an object.
#[derive(Debug)]
struct Field {
    name: &'static str,
    kind: Kind,
    presence: Presence,
}

/// The fields of an object, as `model`'s serde code reads them.
type Shape = [Field];

const fn field(name: &'static str, kind: Kind, presence: Presence) -> Field {
    Field {
        name,
        kind,
        presence,
    }
}

use Presence::{Nullable, Optional, Required};

/// `model::Instance`.
static INSTANCE: [Field; 4] = [
    field("objective", Kind::Choice(&OBJECTIVE), Optional),
    field("saw", Kind::Object(&SAW), Required),
    field("panels", Kind::List(&PANEL), Required),
    field("parts", Kind::List(&PART), Required),
];

/// `model::Saw`.
static SAW: [Field; 4] = [
    field("kerf", Kind::Length, Optional),
    field("stages", Kind::Stages, Optional),
    field("cut_type", Kind::Choice(&CUT_TYPE), Optional),
    field("first_cuts", Kind::Choice(&FIRST_CUTS), Optional),
];

/// `model::Panel`.
static PANEL: [Field; 5] = [
    field("id", Kind::Text, Required),
    field("length", Kind::Length, Required),
    field("width", Kind::Length, Required),
    field("quantity", Kind::Count, Nullable),
    field("cost", Kind::Number, Nullable),
];

/// `model::Part`.
static PART: [Field; 6] = [
    field("id", Kind::Text, Required),
    field("length", Kind::Length, Required),
    field("width", Kind::Length, Required),
    field("quantity", Kind::Count, Optional),
    field("grain", Kind::Flag, Optional),
    field("value", Kind::Number, Nullable),
];

/// Checks that `value` is an object of `shape`, adding a problem for each
/// field at fault. Fields not in `shape` are ignored, as serde does.
fn check_object(value: &Value, shape: &Shape, path: &str, problems: &mut Vec<FieldProblem>) {
    let Some(object) = value.as_object() else {
        problems.push(FieldProblem::at(path, "must be an object of named fields"));
        return;
    };
    for field in shape {
        check_field(object, field, &format!("{path}.{}", field.name), problems);
    }
}

fn check_field(
    object: &Map<String, Value>,
    field: &Field,
    path: &str,
    problems: &mut Vec<FieldProblem>,
) {
    let value = match (object.get(field.name), field.presence) {
        (None, Required) | (Some(Value::Null), Required) => {
            problems.push(FieldProblem::at(path, "this field is required"));
            return;
        }
        (None, _) | (Some(Value::Null), Nullable) => return,
        (Some(value), _) => value,
    };
    let message = match field.kind {
        Kind::Object(shape) => return check_object(value, shape, path, problems),
        Kind::List(shape) => {
            match value.as_array() {
                Some(items) => {
                    for (index, item) in items.iter().enumerate() {
                        check_object(item, shape, &format!("{path}[{index}]"), problems);
                    }
                }
                None => problems.push(FieldProblem::at(path, "must be a list")),
            }
            return;
        }
        kind => check_value(value, kind),
    };
    if let Some(message) = message {
        problems.push(FieldProblem::at(path, message));
    }
}

/// What is wrong with `value` as a `kind`, if anything.
fn check_value(value: &Value, kind: Kind) -> Option<String> {
    let ok = match kind {
        Kind::Text => value.is_string(),
        Kind::Length if value.is_number() => {
            // Length's own message explains the one-decimal rule.
            return serde_json::from_value::<Length>(value.clone())
                .err()
                .map(|error| error.to_string());
        }
        Kind::Length => false,
        Kind::Count => value.as_u64().is_some_and(|n| u32::try_from(n).is_ok()),
        Kind::Number => value.is_number(),
        Kind::Flag => value.is_boolean(),
        Kind::Choice(choice) => value.as_str().is_some_and(|s| choice.has_text(s)),
        Kind::Stages => serde_json::from_value::<Stages>(value.clone()).is_ok(),
        Kind::Object(_) | Kind::List(_) => unreachable!("checked by check_field"),
    };
    if ok {
        return None;
    }
    Some(match kind {
        Kind::Text => "must be text".to_string(),
        Kind::Length => "must be a length in millimetres, such as 720 or 3.2".to_string(),
        Kind::Count => "must be a whole number, such as 1 or 4".to_string(),
        Kind::Number => "must be a number".to_string(),
        Kind::Flag => "must be true or false".to_string(),
        Kind::Choice(choice) => format!("must be one of: {}", choice.texts().join(", ")),
        Kind::Stages => "must be 2, 3 or \"unlimited\"".to_string(),
        Kind::Object(_) | Kind::List(_) => unreachable!("checked by check_field"),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn example() -> Value {
        json!({
            "instance": {
                "objective": "bin_packing",
                "saw": { "kerf": 3.2, "stages": 3, "cut_type": "non_exact", "first_cuts": "either" },
                "panels": [ { "id": "Birch ply 18 mm", "length": 2500, "width": 1250, "quantity": 10 } ],
                "parts": [ { "id": "Side left", "length": 720, "width": 560, "quantity": 4, "grain": true } ]
            },
            "time_limit": 5
        })
    }

    fn fields(problems: &[FieldProblem]) -> Vec<&str> {
        problems
            .iter()
            .map(|problem| problem.field.as_deref().unwrap_or("-"))
            .collect()
    }

    #[test]
    fn reads_the_documented_example() {
        let job = parse_job(&example()).unwrap();
        assert_eq!(job.time_limit, 5);
        assert_eq!(job.instance.parts[0].id, "Side left");
    }

    #[test]
    fn time_limit_defaults_to_ten_seconds_and_is_bounded() {
        let mut body = example();
        body.as_object_mut().unwrap().remove("time_limit");
        assert_eq!(parse_job(&body).unwrap().time_limit, 10);
        for bad in [json!(0), json!(601), json!(2.5), json!("10")] {
            body["time_limit"] = bad;
            assert_eq!(fields(&parse_job(&body).unwrap_err()), ["time_limit"]);
        }
    }

    #[test]
    fn reports_every_mistyped_field() {
        let mut body = example();
        body["instance"]["saw"]["kerf"] = json!(3.25);
        body["instance"]["saw"]["cut_type"] = json!("rough");
        body["instance"]["parts"][0]["width"] = json!("560");
        body["instance"]["parts"][0]["quantity"] = json!(-1);
        body["instance"]["panels"][0]
            .as_object_mut()
            .unwrap()
            .remove("length");
        body["time_limit"] = json!(0);
        let problems = parse_job(&body).unwrap_err();
        assert_eq!(
            fields(&problems),
            [
                "time_limit",
                "instance.saw.kerf",
                "instance.saw.cut_type",
                "instance.panels[0].length",
                "instance.parts[0].width",
                "instance.parts[0].quantity",
            ]
        );
        assert_eq!(
            problems[1].message,
            "length 3.25 mm has more than one decimal; lengths are in steps of 0.1 mm"
        );
    }

    #[test]
    fn reports_validation_problems_with_their_fields() {
        let mut body = example();
        body["instance"]["parts"] = json!([]);
        body["instance"]["panels"][0]["quantity"] = json!(0);
        let problems = parse_job(&body).unwrap_err();
        assert_eq!(
            fields(&problems),
            ["instance.parts", "instance.panels[0].quantity"]
        );
        assert_eq!(problems[0].message, "add at least one part");
    }

    #[test]
    fn a_missing_instance_or_a_non_object_body_is_a_problem() {
        assert_eq!(fields(&parse_job(&json!({})).unwrap_err()), ["instance"]);
        assert_eq!(fields(&parse_job(&json!([1, 2])).unwrap_err()), ["-"]);
        let body = json!({ "instance": { "objective": "knapsack", "saw": { "kerf": "3" } } });
        assert_eq!(
            fields(&parse_job(&body).unwrap_err()),
            ["instance.saw.kerf", "instance.panels", "instance.parts"]
        );
    }

    #[test]
    fn options_are_the_enums_values() {
        use crate::fields::{CHOICES, OptionValue};
        for choice in CHOICES {
            for (option, _) in choice.options {
                let value = match option {
                    OptionValue::Text(text) => json!(text),
                    OptionValue::Number(number) => json!(number),
                };
                let ok = match choice.schema {
                    "Objective" => serde_json::from_value::<model::Objective>(value).is_ok(),
                    "Stages" => serde_json::from_value::<model::Stages>(value).is_ok(),
                    "CutType" => serde_json::from_value::<model::CutType>(value).is_ok(),
                    "FirstCuts" => serde_json::from_value::<model::FirstCuts>(value).is_ok(),
                    other => panic!("unknown choice {other}"),
                };
                assert!(ok, "{} {option:?}", choice.schema);
            }
        }
    }

    /// The walk must accept everything serde accepts, or it would report
    /// problems with valid fields next to a real one.
    #[test]
    fn the_walk_accepts_every_test_instance() {
        let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/instances");
        let mut checked = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|name| name == "MANIFEST.json")
                || path.extension().is_none_or(|extension| extension != "json")
            {
                continue;
            }
            let value: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            serde_json::from_value::<Instance>(value.clone()).unwrap();
            let mut problems = Vec::new();
            check_object(&value, &INSTANCE, "instance", &mut problems);
            assert_eq!(problems, [], "{}", path.display());
            checked += 1;
        }
        assert!(checked > 100);
    }
}
