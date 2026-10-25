//! The API end to end, through the router: submit, poll, stream, cancel
//! and every error. Every plan served is checked with the checker.

use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use model::{Instance, Solution};
use serde_json::{Value, json};
use tower::ServiceExt;

/// A small instance that solves in well under a second.
const SMALL: &str = "tests__bin_packing_3nvo";
/// An instance that takes far longer than the tests wait, with
/// [`slow_config`].
const SLOW: &str = "generated__bin_packing_3_exact_kerf0";

fn instance(stem: &str) -> Value {
    let path = format!(
        "{}/../../data/instances/{stem}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Small beam widths keep debug-build tests fast.
fn config() -> api::Config {
    api::Config {
        max_running_jobs: 2,
        queue_size: 64,
        ..api::Config::default()
    }
}

/// One solver, and a beam width no test waits for.
fn slow_config() -> api::Config {
    api::Config {
        max_running_jobs: 1,
        queue_size: 1 << 30,
        ..api::Config::default()
    }
}

async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Vec<u8>>,
) -> (StatusCode, Value, axum::http::HeaderMap) {
    let mut request = Request::builder().method(method).uri(uri);
    if body.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let request = request
        .body(body.map_or_else(Body::empty, Body::from))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value, headers)
}

async fn submit(app: &Router, stem: &str, time_limit: u32) -> Value {
    let body = json!({ "instance": instance(stem), "time_limit": time_limit });
    let (status, job, headers) = send(
        app,
        Method::POST,
        "/jobs",
        Some(body.to_string().into_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{job}");
    let id = job["id"].as_str().unwrap();
    assert_eq!(headers[header::LOCATION], format!("/jobs/{id}"));
    job
}

async fn get(app: &Router, id: &str) -> Value {
    let (status, job, _) = send(app, Method::GET, &format!("/jobs/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    job
}

/// Polls the job until `done`, `cancelled` or `failed`.
async fn wait_until_final(app: &Router, id: &str, within: Duration) -> Value {
    let start = Instant::now();
    loop {
        let job = get(app, id).await;
        if ["done", "cancelled", "failed"].contains(&job["state"].as_str().unwrap()) {
            return job;
        }
        assert!(
            start.elapsed() < within,
            "still {} after {within:?}",
            job["state"]
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn wait_until_running(app: &Router, id: &str) {
    let start = Instant::now();
    while get(app, id).await["state"] != "running" {
        assert!(start.elapsed() < Duration::from_secs(10));
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// Checks a served plan report: it passes the checker, and its metrics
/// are the checker's.
fn assert_valid_plan(stem: &str, report: &Value) {
    let instance: Instance = serde_json::from_value(instance(stem)).unwrap();
    let plan: Solution = serde_json::from_value(report["plan"].clone()).unwrap();
    let check = checker::check(&instance, &plan);
    assert!(check.is_feasible(), "{:?}", check.violations);
    assert_eq!(
        report["metrics"],
        serde_json::to_value(&check.metrics).unwrap()
    );
    assert!(report["number"].as_u64().unwrap() >= 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_submitted_job_runs_to_a_checked_plan() {
    let app = api::router(&config());
    let job = submit(&app, SMALL, 5).await;
    assert!(["queued", "running"].contains(&job["state"].as_str().unwrap()));
    assert_eq!(job["time_limit"], 5);
    assert!(job["message"].is_null());

    let id = job["id"].as_str().unwrap();
    let job = wait_until_final(&app, id, Duration::from_secs(60)).await;
    assert_eq!(job["state"], "done", "{job}");
    assert!(job["message"].is_null());
    let best = &job["best"];
    assert_valid_plan(SMALL, best);
    assert_eq!(best["number"], job["plans_found"]);
    assert!(best["found_after"].as_f64().unwrap() <= job["elapsed"].as_f64().unwrap());
    // Every part is cut.
    let instance: Instance = serde_json::from_value(instance(SMALL)).unwrap();
    let wanted: u64 = instance
        .parts
        .iter()
        .map(|part| u64::from(part.quantity))
        .sum();
    assert_eq!(best["metrics"]["parts_cut"], wanted);

    // A finished job stays as it ended.
    let again = get(&app, id).await;
    assert_eq!(again["elapsed"], job["elapsed"]);
}

/// One server-sent event.
#[derive(Debug)]
struct Event {
    name: String,
    data: Value,
}

/// Reads the whole event stream of a job, until the server ends it.
async fn read_events(app: &Router, id: &str) -> Vec<Event> {
    let request = Request::get(format!("/jobs/{id}/events"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    let body = tokio::time::timeout(Duration::from_secs(60), response.into_body().collect())
        .await
        .expect("the stream ends")
        .unwrap()
        .to_bytes();
    let text = String::from_utf8(body.to_vec()).unwrap();
    text.split("\n\n")
        .filter_map(|block| {
            let mut name = None;
            let mut data = None;
            for line in block.lines() {
                if let Some(value) = line.strip_prefix("event: ") {
                    name = Some(value.to_string());
                } else if let Some(value) = line.strip_prefix("data: ") {
                    data = Some(serde_json::from_str(value).unwrap());
                }
            }
            Some(Event {
                name: name?,
                data: data?,
            })
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_event_stream_sends_each_better_plan_then_the_final_state() {
    let app = api::router(&config());
    let job = submit(&app, SMALL, 5).await;
    let id = job["id"].as_str().unwrap();
    let events = read_events(&app, id).await;

    let last = events.last().expect("some events");
    assert_eq!(last.name, "state");
    assert_eq!(last.data["state"], "done");
    let mut number = 0;
    for event in &events {
        match event.name.as_str() {
            "plan" => {
                assert_valid_plan(SMALL, &event.data);
                let next = event.data["number"].as_u64().unwrap();
                assert!(next > number, "plan numbers increase");
                number = next;
            }
            "state" => assert!(event.data["plans_found"].as_u64().unwrap() >= number),
            other => panic!("unexpected event {other}"),
        }
    }
    assert!(number >= 1, "at least one plan");
    let states: Vec<&str> = events
        .iter()
        .filter(|event| event.name == "state")
        .map(|event| event.data["state"].as_str().unwrap())
        .collect();
    assert!(
        [&["queued", "running", "done"][..], &["running", "done"]].contains(&&states[..]),
        "{states:?}"
    );

    // The last plan streamed is the job's best.
    let job = get(&app, id).await;
    let plans: Vec<&Event> = events.iter().filter(|event| event.name == "plan").collect();
    assert_eq!(plans.last().unwrap().data, job["best"]);
    assert_eq!(job["plans_found"], number);

    // Connecting after the end replays the best plan and the final state.
    let replay = read_events(&app, id).await;
    let names: Vec<&str> = replay.iter().map(|event| event.name.as_str()).collect();
    assert_eq!(names, ["plan", "state"]);
    assert_eq!(replay[0].data, job["best"]);
    assert_eq!(replay[1].data["state"], "done");
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_running_job_keeps_its_best_plan() {
    let app = api::router(&slow_config());
    let job = submit(&app, SLOW, 600).await;
    let id = job["id"].as_str().unwrap();
    wait_until_running(&app, id).await;
    // Give it time for a first plan (queue size 1 finds one at once).
    let start = Instant::now();
    while get(&app, id).await["best"].is_null() {
        assert!(start.elapsed() < Duration::from_secs(30), "no first plan");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let (status, response, _) = send(&app, Method::DELETE, &format!("/jobs/{id}"), None).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(response["id"], id);
    let job = wait_until_final(&app, id, Duration::from_secs(10)).await;
    assert_eq!(job["state"], "cancelled");
    assert!(job["elapsed"].as_f64().unwrap() < 60.0);
    assert_valid_plan(SLOW, &job["best"]);

    // Cancelling again changes nothing.
    let (status, again, _) = send(&app, Method::DELETE, &format!("/jobs/{id}"), None).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(again["state"], "cancelled");
    assert_eq!(again["elapsed"], job["elapsed"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_queued_job_is_cancelled_at_once_and_jobs_beyond_the_queue_are_refused() {
    let app = api::router(&api::Config {
        max_queued_jobs: 1,
        ..slow_config()
    });
    let running = submit(&app, SLOW, 600).await;
    let running = running["id"].as_str().unwrap();
    wait_until_running(&app, running).await;

    let queued = submit(&app, SMALL, 5).await;
    assert_eq!(queued["state"], "queued");
    assert_eq!(queued["elapsed"], 0.0);
    let queued = queued["id"].as_str().unwrap();

    // One job waits already: the next is refused.
    let body = json!({ "instance": instance(SMALL) })
        .to_string()
        .into_bytes();
    let (status, error, _) = send(&app, Method::POST, "/jobs", Some(body)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["error"], "busy");

    let (status, job, _) = send(&app, Method::DELETE, &format!("/jobs/{queued}"), None).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(job["state"], "cancelled");
    assert!(job["best"].is_null());
    let events = read_events(&app, queued).await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data["state"], "cancelled");

    send(&app, Method::DELETE, &format!("/jobs/{running}"), None).await;
    let job = wait_until_final(&app, running, Duration::from_secs(10)).await;
    assert_eq!(job["state"], "cancelled");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_invalid_instance_is_answered_422_with_every_problem() {
    let app = api::router(&config());
    let mut body = json!({ "instance": instance(SMALL), "time_limit": 0 });
    body["instance"]["saw"]["kerf"] = json!(3.25);
    body["instance"]["parts"][1]["grain"] = json!("yes");
    let (status, error, _) = send(
        &app,
        Method::POST,
        "/jobs",
        Some(body.to_string().into_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["error"], "invalid_instance");
    assert_eq!(error["message"], "The job cannot be solved as entered.");
    let fields: Vec<&str> = error["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|problem| problem["field"].as_str().unwrap())
        .collect();
    assert_eq!(
        fields,
        ["time_limit", "instance.saw.kerf", "instance.parts[1].grain"]
    );

    // Valid JSON types, but nothing to cut.
    let mut body = json!({ "instance": instance(SMALL) });
    body["instance"]["parts"] = json!([]);
    let (status, error, _) = send(
        &app,
        Method::POST,
        "/jobs",
        Some(body.to_string().into_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        error["problems"],
        json!([{ "field": "instance.parts", "message": "add at least one part" }])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn other_errors_have_their_codes() {
    let app = api::router(&config());

    let (status, error, _) = send(&app, Method::POST, "/jobs", Some(b"{ not json".to_vec())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["error"], "invalid_json");
    assert_eq!(error["problems"], json!([]));

    let too_large = vec![b' '; api::MAX_BODY_BYTES + 1];
    let (status, error, _) = send(&app, Method::POST, "/jobs", Some(too_large)).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(error["error"], "too_large");

    for uri in [
        "/jobs/0f8b3c8e-5a43-4d4e-9d8b-3f7c2a1e6b90",
        "/jobs/not-a-uuid",
        "/jobs/0f8b3c8e-5a43-4d4e-9d8b-3f7c2a1e6b90/events",
        "/nowhere",
    ] {
        let (status, error, _) = send(&app, Method::GET, uri, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(error["error"], "not_found");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn finished_jobs_expire() {
    let app = api::router(&api::Config {
        keep_jobs_for: Duration::from_millis(200),
        ..config()
    });
    let job = submit(&app, SMALL, 5).await;
    let id = job["id"].as_str().unwrap();
    wait_until_final(&app, id, Duration::from_secs(60)).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (status, _, _) = send(&app, Method::GET, &format!("/jobs/{id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn browsers_may_call_from_other_origins() {
    let app = api::router(&config());
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/jobs")
        .header(header::ORIGIN, "http://localhost:5173")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let headers = response.headers();
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    let methods = headers[header::ACCESS_CONTROL_ALLOW_METHODS]
        .to_str()
        .unwrap();
    assert!(methods.contains("DELETE"), "{methods}");

    let restricted = api::router(&api::Config {
        cors_origins: vec!["http://localhost:5173".parse().unwrap()],
        ..config()
    });
    let request = Request::get("/openapi.json")
        .header(header::ORIGIN, "http://elsewhere.example")
        .body(Body::empty())
        .unwrap();
    let response = restricted.oneshot(request).await.unwrap();
    assert!(
        !response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
}
