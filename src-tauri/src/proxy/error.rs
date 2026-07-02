//! Error response formatting for proxy responses.
//!
//! Produces JSON error bodies compatible with Codex's Responses API error format:
//!
//! ```json
//! {
//!   "error": {
//!     "message": "...",
//!     "type": "invalid_request_error" | "server_error" | "rate_limit_error" | "authentication_error" | "..."
//!   }
//! }
//! ```

use axum::{body::Body, http::StatusCode, response::Response};

/// Determine the error type string from an HTTP status code.
pub fn error_type_from_status(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 => "invalid_request_error",
        401 | 403 => "authentication_error",
        404 => "not_found_error",
        429 => "rate_limit_error",
        500..=599 => "server_error",
        _ => "api_error",
    }
}

/// Build a JSON error response with status code, message, and auto-detected error type.
pub fn error_response(status: StatusCode, msg: String) -> Response<Body> {
    let err_type = error_type_from_status(status);
    let body = serde_json::json!({
        "error": {
            "message": msg,
            "type": err_type,
        }
    });
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(r#"{"error":{"message":"serialization failed","type":"server_error"}}"#))
                .unwrap()
        })
}

/// Build an error response from upstream JSON error body.
///
/// Parses common error formats (OpenAI, Anthropic, generic) and returns
/// a standardized Responses API error.
pub fn upstream_error_response(status: StatusCode, upstream_body: &[u8]) -> Response<Body> {
    let (message, err_type) = parse_upstream_error(upstream_body, status);
    let body = serde_json::json!({
        "error": {
            "message": message,
            "type": err_type,
        }
    });
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(r#"{"error":{"message":"serialization failed","type":"server_error"}}"#))
                .unwrap()
        })
}

/// Parse upstream error body into (message, error_type).
///
/// Handles OpenAI Chat API, Anthropic API, and generic JSON error formats.
fn parse_upstream_error(body: &[u8], status: StatusCode) -> (String, String) {
    let err_type = error_type_from_status(status).to_string();

    // Try to parse as JSON
    let value: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => {
            // Non-JSON body — truncate and return
            let raw = String::from_utf8_lossy(body).chars().take(200).collect::<String>();
            return (raw, err_type);
        }
    };

    // OpenAI Chat API / Responses API: { "error": { "message": "...", "type": "...", "code": "..." } }
    if let Some(err) = value.get("error") {
        let msg = err
            .get("message")
            .and_then(|v| v.as_str())
            .or_else(|| err.as_str())
            .unwrap_or("Unknown error")
            .to_string();
        let upstream_type = err
            .get("type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| err_type.clone());
        return (msg, upstream_type);
    }

    // Anthropic API: { "type": "error", "error": { "type": "...", "message": "..." } }
    if let Some(inner) = value.get("error").and_then(|v| v.as_object()) {
        let msg = inner
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error")
            .to_string();
        let upstream_type = inner
            .get("type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| err_type.clone());
        return (msg, upstream_type);
    }

    // Anthropic older format: { "type": "error", "message": "...", ... }
    if let Some(msg) = value.get("message").and_then(|v| v.as_str()) {
        return (msg.to_string(), err_type);
    }

    // Detail field (common in validation errors)
    if let Some(detail) = value.get("detail").and_then(|v| v.as_str()) {
        return (detail.to_string(), err_type);
    }

    // Fallback: serialize the entire value
    let fallback = serde_json::to_string(&value).unwrap_or_default();
    let trimmed = fallback.chars().take(200).collect::<String>();
    (trimmed, err_type)
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_error_type_mapping() {
        assert_eq!(error_type_from_status(StatusCode::BAD_REQUEST), "invalid_request_error");
        assert_eq!(error_type_from_status(StatusCode::UNAUTHORIZED), "authentication_error");
        assert_eq!(error_type_from_status(StatusCode::FORBIDDEN), "authentication_error");
        assert_eq!(error_type_from_status(StatusCode::NOT_FOUND), "not_found_error");
        assert_eq!(error_type_from_status(StatusCode::TOO_MANY_REQUESTS), "rate_limit_error");
        assert_eq!(error_type_from_status(StatusCode::INTERNAL_SERVER_ERROR), "server_error");
        assert_eq!(error_type_from_status(StatusCode::SERVICE_UNAVAILABLE), "server_error");
        assert_eq!(error_type_from_status(StatusCode::BAD_GATEWAY), "server_error");
        assert_eq!(error_type_from_status(StatusCode::OK), "api_error");
    }

    #[test]
    fn test_parse_openai_error() {
        let body = br#"{"error": {"message": "Insufficient quota", "type": "insufficient_quota"}}"#;
        let (msg, err_type) = parse_upstream_error(body, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(msg, "Insufficient quota");
        assert_eq!(err_type, "insufficient_quota");
    }

    #[test]
    fn test_parse_anthropic_error() {
        let body = br#"{"type": "error", "error": {"type": "authentication_error", "message": "Invalid API key"}}"#;
        let (msg, err_type) = parse_upstream_error(body, StatusCode::UNAUTHORIZED);
        assert_eq!(msg, "Invalid API key");
        assert_eq!(err_type, "authentication_error");
    }

    #[test]
    fn test_parse_anthropic_flat_error() {
        let body = br#"{"type": "error", "message": "Rate limit exceeded"}"#;
        let (msg, _) = parse_upstream_error(body, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(msg, "Rate limit exceeded");
    }

    #[test]
    fn test_parse_non_json_error() {
        let body = b"<html>Server Error</html>";
        let (msg, _) = parse_upstream_error(body, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(msg.contains("html"));
    }

    #[test]
    fn test_parse_empty_body() {
        let body = b"";
        let (msg, _) = parse_upstream_error(body, StatusCode::BAD_REQUEST);
        // Empty body returns empty string since serde_json::from_slice fails
        assert!(msg.is_empty() || msg == "null", "Empty body should produce empty or null message, got: {}", msg);
    }

    #[test]
    fn test_parse_detail_field() {
        let body = br#"{"detail": "Validation failed for field 'model'"}"#;
        let (msg, _) = parse_upstream_error(body, StatusCode::BAD_REQUEST);
        assert_eq!(msg, "Validation failed for field 'model'");
    }

    #[test]
    fn test_error_response_builder() {
        let resp = error_response(StatusCode::BAD_REQUEST, "test error".into());
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_upstream_error_response_builder() {
        let body = br#"{"error": {"message": "fail", "type": "server_error"}}"#;
        let resp = upstream_error_response(StatusCode::INTERNAL_SERVER_ERROR, body);
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}

