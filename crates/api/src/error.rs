//! Error responses: every one has an [`ErrorBody`].

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::types::{ErrorBody, ErrorCode, FieldProblem};

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ApiError {
    /// The body is not JSON.
    #[error("The request body is not valid JSON: {0}.")]
    InvalidJson(String),
    /// No job with this id, or it has expired.
    #[error("No job with this id; it may have expired.")]
    NotFound,
    /// The body is larger than the limit.
    #[error("The request body is larger than {} MB.", crate::MAX_BODY_BYTES / (1024 * 1024))]
    TooLarge,
    /// The body is JSON but not a valid job.
    #[error("The job cannot be solved as entered.")]
    InvalidInstance(Vec<FieldProblem>),
    /// Too many jobs are waiting.
    #[error("Too many jobs are waiting for a solver; try again in {retry_after} s.")]
    Busy {
        /// Seconds after which to try again (`Retry-After`).
        retry_after: u64,
    },
}

impl ApiError {
    /// The HTTP status and the error code.
    fn code(&self) -> (StatusCode, ErrorCode) {
        match self {
            ApiError::InvalidJson(_) => (StatusCode::BAD_REQUEST, ErrorCode::InvalidJson),
            ApiError::NotFound => (StatusCode::NOT_FOUND, ErrorCode::NotFound),
            ApiError::TooLarge => (StatusCode::PAYLOAD_TOO_LARGE, ErrorCode::TooLarge),
            ApiError::InvalidInstance(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, ErrorCode::InvalidInstance)
            }
            ApiError::Busy { .. } => (StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Busy),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error) = self.code();
        let message = self.to_string();
        let retry_after = match &self {
            ApiError::Busy { retry_after } => Some(*retry_after),
            _ => None,
        };
        let problems = match self {
            ApiError::InvalidInstance(problems) => problems,
            _ => Vec::new(),
        };
        let body = ErrorBody {
            error,
            message,
            problems,
        };
        let mut response = (status, Json(body)).into_response();
        if let Some(seconds) = retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
    }
}
