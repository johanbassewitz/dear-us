use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Every error goes to the app as `{"error": "<message for people>", "code": "<machine code>"}`.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> ApiError {
        ApiError { status, code, message: message.into() }
    }
    pub fn bad(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::BAD_REQUEST, "invalid", message)
    }
    pub fn unauthorized() -> ApiError {
        ApiError::new(StatusCode::UNAUTHORIZED, "signed_out", "Please sign in again.")
    }
    pub fn forbidden() -> ApiError {
        ApiError::new(StatusCode::FORBIDDEN, "forbidden", "You're not part of this space.")
    }
    pub fn not_found(what: &str) -> ApiError {
        ApiError::new(StatusCode::NOT_FOUND, "not_found", format!("{what} not found"))
    }
    /// 402: the plan's allowance is used up. `code` tells the app which banner to show.
    pub fn quota(code: &'static str, message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::PAYMENT_REQUIRED, code, message)
    }
    pub fn unavailable(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "unavailable", message)
    }
    pub fn internal(e: impl std::fmt::Display) -> ApiError {
        tracing::error!("internal error: {e}");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "Something went wrong on our side. Your memories are safe on your PC.")
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::Database(d) = &e {
            match d.code().as_deref() {
                Some("23505") => return ApiError::new(StatusCode::CONFLICT, "exists", "That already exists."),
                Some("23503") => return ApiError::bad("That refers to something that doesn't exist (yet)."),
                Some("23514") | Some("22P02") => return ApiError::bad("Some of that data isn't valid."),
                _ => {}
            }
        }
        ApiError::internal(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message, "code": self.code }))).into_response()
    }
}
