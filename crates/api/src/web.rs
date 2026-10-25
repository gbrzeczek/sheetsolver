//! Serving the built web UI next to the API (`--web-dir`).
//!
//! The API's own routes are matched first; every other request lands
//! here. A path under the API (`/jobs…`, `/openapi.json`) that matched no
//! route is a JSON `404 not_found`, as without a UI. Anything else is a
//! file from the web directory, and a browser asking for a page that is
//! not a file gets `index.html`, so a client-side route can be reloaded.

use std::path::{Path, PathBuf};

use axum::extract::Request;
use axum::http::{Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use crate::error::ApiError;

/// Whether `path` belongs to the API rather than the UI.
pub fn is_api_path(path: &str) -> bool {
    path == "/jobs" || path.starts_with("/jobs/") || path == "/openapi.json"
}

/// The answer to a request no API route matched.
pub async fn fallback(web_dir: Option<&Path>, request: Request) -> Response {
    let Some(web_dir) = web_dir.filter(|_| !is_api_path(request.uri().path())) else {
        return ApiError::NotFound.into_response();
    };
    let wants_page = matches!(*request.method(), Method::GET | Method::HEAD)
        && request
            .headers()
            .get(header::ACCEPT)
            .and_then(|accept| accept.to_str().ok())
            .is_some_and(|accept| accept.contains("text/html"));
    let index = web_dir.join("index.html");
    // Kept for the index fallback: the file service consumes the request.
    let (parts, body) = request.into_parts();
    let fallback_request = Request::from_parts(parts.clone(), axum::body::Body::empty());
    let response = ServeDir::new(web_dir)
        .oneshot(Request::from_parts(parts, body))
        .await
        .expect("serving files is infallible")
        .into_response();
    if response.status() != StatusCode::NOT_FOUND {
        return response;
    }
    if wants_page && index.is_file() {
        return ServeFile::new(index)
            .oneshot(fallback_request)
            .await
            .expect("serving files is infallible")
            .into_response();
    }
    ApiError::NotFound.into_response()
}

/// Checks that `web_dir` can be served: it must be a directory. Returns
/// a warning if it has no `index.html` (the API still works then).
pub fn check_web_dir(web_dir: &Path) -> Result<Option<String>, WebDirError> {
    if !web_dir.is_dir() {
        return Err(WebDirError::NotADirectory(web_dir.to_path_buf()));
    }
    Ok((!web_dir.join("index.html").is_file()).then(|| {
        format!(
            "{} has no index.html: only its files are served, no UI page",
            web_dir.display()
        )
    }))
}

/// Why a web directory cannot be served.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WebDirError {
    /// The path is not a directory.
    #[error("the web directory {} does not exist or is not a directory", .0.display())]
    NotADirectory(PathBuf),
}
