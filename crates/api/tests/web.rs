//! Serving the built UI with `--web-dir`: files, the `index.html`
//! fallback for pages, and the API keeping its paths.

use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

/// A web directory with an index page, a script, and a file named like
/// an API path.
fn web_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sheetsolver-web-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(
        dir.join("index.html"),
        "<!doctype html><title>sheetsolver</title>",
    )
    .unwrap();
    std::fs::write(dir.join("assets/app.js"), "console.log('cut');").unwrap();
    std::fs::write(dir.join("openapi.json"), "not the API's").unwrap();
    dir
}

fn app(web_dir: Option<PathBuf>) -> Router {
    api::router(&api::Config {
        web_dir,
        ..api::Config::default()
    })
}

/// GETs `uri`, as a browser page load if `page`.
async fn get(app: &Router, uri: &str, page: bool) -> (StatusCode, Option<String>, String) {
    let mut request = Request::get(uri);
    if page {
        request = request.header(header::ACCEPT, "text/html,application/xhtml+xml,*/*;q=0.8");
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|value| value.to_str().unwrap().to_string());
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        content_type,
        String::from_utf8(body.to_vec()).unwrap(),
    )
}

fn assert_not_found_json(status: StatusCode, body: &str) {
    assert_eq!(status, StatusCode::NOT_FOUND);
    let error: Value = serde_json::from_str(body).unwrap();
    assert_eq!(error["error"], "not_found");
}

#[tokio::test]
async fn serves_files_from_the_web_directory() {
    let app = app(Some(web_dir("files")));
    let (status, content_type, body) = get(&app, "/assets/app.js", false).await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.unwrap().contains("javascript"));
    assert_eq!(body, "console.log('cut');");

    let (status, content_type, body) = get(&app, "/", true).await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.unwrap().starts_with("text/html"));
    assert!(body.contains("<title>sheetsolver</title>"));
}

#[tokio::test]
async fn pages_that_are_not_files_get_the_index() {
    let app = app(Some(web_dir("fallback")));
    let (status, _, body) = get(&app, "/plans/42", true).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<title>sheetsolver</title>"));

    // Not a page load: a missing file stays missing.
    let (status, _, body) = get(&app, "/assets/missing.js", false).await;
    assert_not_found_json(status, &body);
}

#[tokio::test]
async fn api_paths_win_over_files_and_keep_their_json_404() {
    let app = app(Some(web_dir("api")));
    let (status, content_type, body) = get(&app, "/openapi.json", true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    let document: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(document["openapi"], "3.1.0");

    // Unknown API paths are JSON 404s, even for a page load.
    for uri in [
        "/jobs/0f8b3c8e-5a43-4d4e-9d8b-3f7c2a1e6b90",
        "/jobs/x/y/z",
        "/jobs",
    ] {
        let (status, _, body) = get(&app, uri, true).await;
        if uri == "/jobs" {
            // `/jobs` only takes POST.
            assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{uri}");
        } else {
            assert_not_found_json(status, &body);
        }
    }
}

#[tokio::test]
async fn without_a_web_directory_nothing_changes() {
    let app = app(None);
    for uri in ["/", "/assets/app.js", "/plans/42"] {
        let (status, _, body) = get(&app, uri, true).await;
        assert_not_found_json(status, &body);
    }
}

#[test]
fn the_web_directory_must_exist_and_should_have_an_index() {
    let dir = web_dir("check");
    assert_eq!(api::check_web_dir(&dir), Ok(None));
    std::fs::remove_file(dir.join("index.html")).unwrap();
    assert!(
        api::check_web_dir(&dir)
            .unwrap()
            .unwrap()
            .contains("no index.html")
    );
    assert!(api::check_web_dir(&dir.join("nowhere")).is_err());
}
