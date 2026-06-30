use std::sync::Arc;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::Response,
};
use bytes::Bytes;
use futures::TryStreamExt;
use std::io;

use crate::config::AppState;
use crate::convert;
use crate::models::*;
use crate::sse::SseTransformStream;

/// Build backend URL based on path and backend type.
fn build_backend_url(base: &str, path: &str, backend_type: &BackendType) -> String {
    let base = base.trim_end_matches('/');
    match (backend_type, path) {
        (BackendType::OpenaiChat, "responses") => format!("{}/chat/completions", base),
        (BackendType::Anthropic, "responses") => format!("{}/messages", base),
        _ => format!("{}/{}", base, path),
    }
}

/// GET /v1/models - fake model list
pub async fn handle_models(
    State(state): State<Arc<AppState>>,
) -> axum::Json<serde_json::Value> {
    let models = state.models.lock().await;
    axum::Json(serde_json::json!({
        "object": "list",
        "data": models.clone(),
    }))
}

/// Catch-all: intercept /v1/*path, forward to backend with protocol conversion
pub async fn handle_proxy(
    State(state): State<Arc<AppState>>,
    method: Method,
    Path(path): Path<String>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    let backend = state.backend.lock().await.clone();
    let url = build_backend_url(&backend.api_base, &path, &backend.backend_type);

    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap();

    let req_method: reqwest::Method = method.as_str().parse().unwrap_or(reqwest::Method::POST);
    let mut req_builder = client.request(req_method, &url);

    // Auth headers
    match &backend.backend_type {
        BackendType::Anthropic => {
            if !backend.api_key.is_empty() {
                req_builder = req_builder.header("x-api-key", &backend.api_key);
                req_builder = req_builder.header("anthropic-version", "2023-06-01");
            }
        }
        _ => {
            if !backend.api_key.is_empty() {
                req_builder = req_builder.header(header::AUTHORIZATION, format!("Bearer {}", backend.api_key));
            }
        }
    }
    req_builder = req_builder.header(header::CONTENT_TYPE, "application/json");

    // Transform request body
    if !body.is_empty() {
        let val: serde_json::Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid JSON body".into()),
        };

        let transformed = match backend.backend_type {
            BackendType::OpenaiChat if path == "responses" => {
                match convert::responses_to_chat_request(&val) {
                    Ok(chat_req) => serde_json::to_vec(&chat_req).unwrap_or_default(),
                    Err(e) => return error_response(StatusCode::BAD_REQUEST, format!("Conversion: {e}")),
                }
            }
            BackendType::Anthropic if path == "responses" => {
                match convert::responses_to_anthropic_request(&val) {
                    Ok(anth_req) => serde_json::to_vec(&anth_req).unwrap_or_default(),
                    Err(e) => return error_response(StatusCode::BAD_REQUEST, format!("Conversion: {e}")),
                }
            }
            _ => body.to_vec(),
        };
        req_builder = req_builder.body(transformed);
    }

    // Forward to backend
    let resp = match client.execute(req_builder.build().unwrap()).await {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_GATEWAY, format!("Backend: {e}")),
    };

    let status = resp.status();
    let is_stream = resp.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("text/event-stream"))
        .unwrap_or(false);

    if !status.is_success() {
        let bytes = resp.bytes().await.unwrap_or_default();
        return Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(bytes))
            .unwrap();
    }

    if is_stream {
        // SSE streaming with protocol conversion
        let is_chat = backend.backend_type == BackendType::OpenaiChat;
        let raw_stream = resp.bytes_stream()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e));
        let sse_stream = SseTransformStream::new(raw_stream, is_chat);

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(Body::from_stream(sse_stream))
            .unwrap()
    } else {
        // Non-streaming: convert response
        let bytes = resp.bytes().await.unwrap_or_default();
        let converted = if !bytes.is_empty() {
            match backend.backend_type {
                BackendType::OpenaiChat => {
                    if let Ok(chat) = serde_json::from_slice::<ChatResponse>(&bytes) {
                        let id = state.models.lock().await
                            .first().map(|m| m.id.clone())
                            .unwrap_or_else(|| chat.model.clone());
                        serde_json::to_vec(&convert::chat_to_responses_response(&chat, &id))
                            .unwrap_or_else(|_| bytes.to_vec())
                    } else {
                        bytes.to_vec()
                    }
                }
                BackendType::Anthropic => {
                    if let Ok(anth) = serde_json::from_slice::<AnthropicResponse>(&bytes) {
                        let id = state.models.lock().await
                            .first().map(|m| m.id.clone())
                            .unwrap_or_else(|| anth.model.clone());
                        serde_json::to_vec(&convert::anthropic_to_responses_response(&anth, &id))
                            .unwrap_or_else(|_| bytes.to_vec())
                    } else {
                        bytes.to_vec()
                    }
                }
                BackendType::OpenaiResponses => bytes.to_vec(),
            }
        } else {
            bytes.to_vec()
        };

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(converted))
            .unwrap()
    }
}

fn error_response(status: StatusCode, msg: String) -> Response<Body> {
    let body = serde_json::json!({ "error": { "message": msg } });
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}
