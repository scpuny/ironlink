// ── Error response formatting ──

use axum::{
    body::Body,
    http::StatusCode,
    response::Response,
};

/// Build a JSON error response with the given status code and message.
pub fn error_response(status: StatusCode, msg: String) -> Response<Body> {
    let body = serde_json::json!({ "error": { "message": msg } });
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}
