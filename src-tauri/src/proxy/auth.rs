// ── Provider-specific authentication header builders ──

use axum::http::{header, HeaderMap};

use crate::models::BackendType;

/// Build auth headers for the upstream request based on backend type.
pub fn build_auth_headers(headers: &mut HeaderMap, backend_type: &BackendType, api_key: &str) {
    match backend_type {
        BackendType::Anthropic => {
            if !api_key.is_empty() {
                headers.insert("x-api-key", api_key.parse().unwrap());
                headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
            }
        }
        _ => {
            if !api_key.is_empty() {
                headers.insert(header::AUTHORIZATION, format!("Bearer {}", api_key).parse().unwrap());
            }
        }
    }
    headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
}

/// Determine whether the given base URL belongs to Anthropic.
pub fn is_anthropic(base_url: &str) -> bool {
    base_url.contains("anthropic.com") || base_url.contains("api.claude.ai")
}
