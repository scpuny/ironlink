//! HTTP proxy handler with protocol-aware routing.
//!
//! Receives Requests API requests from Codex Desktop, selects the correct
//! downstream app, converts to the upstream wire protocol, forwards,
//! and converts responses back to Requests API SSE or JSON format.
//!
//! Routing priority:
//!   1. Prefix match — model contains "/" (e.g. "deepseek/xxx")
//!   2. Model match  — model matches app's model_list or default_model
//!   3. Mapping match — per-app model_mappings lookup
//!   4. Fallback — first enabled app

pub mod auth;
pub mod error;
pub mod ocr_interceptor;
pub mod routing;

use std::sync::Arc;
use std::io;

use axum::{
    body::Body,
    extract::{Path, State, WebSocketUpgrade},
    http::{header, HeaderMap, Method, StatusCode},
    response::Response,
};
use bytes::Bytes;
use futures::{SinkExt, TryStreamExt};

use crate::config::AppState;
use crate::protocol;
use crate::models::*;
use crate::protocol::SseTransformStream;

pub use self::error::{error_response, upstream_error_response};

/// GET /v1/models — return model list in OpenAI-compatible format.
///
/// Returns `{"object":"list","data":[{"id":"...","object":"model","owned_by":"..."}]}`
/// which is the format Codex Desktop GUI expects from /v1/models.
/// Also writes the model catalog JSON file for `codex /model` CLI to read.
pub async fn handle_models(
    State(state): State<Arc<AppState>>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;
    tracing::info!("GET /v1/models called by Codex");
    let profiles = state.relay_profiles.lock().await;
    let apps = state.apps.lock().await;

    // Write/update the model catalog file for CLI use, respecting model mappings
    let catalog_path = crate::config::model_catalog_path();
    let codex_app = apps.iter().find(|a| a.id == "codex-desktop");
    let catalog_result = if let Some(app) = codex_app {
        if app.model_replacement_enabled && !app.model_mappings.is_empty() {
            crate::config::write_mapped_model_catalog(&catalog_path, app, &profiles, &state.models.lock().await)
        } else {
            crate::config::write_ironlink_model_catalog(&catalog_path, &profiles, &state.models.lock().await)
        }
    } else {
        crate::config::write_ironlink_model_catalog(&catalog_path, &profiles, &state.models.lock().await)
    };
    if let Err(e) = catalog_result {
        tracing::warn!("Failed to write model catalog: {e}");
    }

    // Build OpenAI-compatible model list for Desktop GUI
    let mut data = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for p in profiles.iter().filter(|p| p.enabled) {
        let all_models: Vec<&str> = p.model_list
            .iter()
            .flat_map(|m| m.split_whitespace())
            .chain(std::iter::once(p.model.as_str()))
            .filter(|m| !m.is_empty())
            .collect();
        for model_id in all_models {
            if !seen.insert(model_id.to_string()) { continue; }
            data.push(serde_json::json!({
                "id": model_id,
                "object": "model",
                "owned_by": p.provider_id,
                "created": 0
            }));
        }
    }

    let response = serde_json::json!({"object": "list", "data": data});
    (StatusCode::OK, [("content-type", "application/json")], serde_json::to_string(&response).unwrap_or_default())
}

/// GET /v1/responses/websocket or /v1/realtime — WebSocket proxy passthrough.
pub async fn handle_websocket(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response<Body> {
    ws.on_upgrade(move |socket| handle_ws_socket(socket, state))
}

/// POST /v1/{*path} — main proxy handler.
///
/// Routes using the four-step priority (prefix → model → mapping → fallback),
/// converts the request body to the upstream protocol, forwards,
/// then converts the response back to Codex's Responses API format.
pub async fn handle_proxy(
    State(state): State<Arc<AppState>>,
    method: Method,
    Path(path): Path<String>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    let enabled = *state.proxy_enabled.lock().await;
    if !enabled {
        return error_response(StatusCode::FORBIDDEN, "Proxy is disabled. Enable it in IronLink settings.".into());
    }

    let body_val: serde_json::Value =
        if body.is_empty() { serde_json::Value::Null }
        else {
            match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("Invalid JSON in request body: {}", e);
                    return error_response(StatusCode::BAD_REQUEST,
                        format!("Invalid JSON in request body: {}", e));
                }
            }
        };

    // Route: find app by protocol → look up app's model_mappings → select provider
    let (backend_type, base_url, api_key, protocol, mapped_upstream_model) = {
        let profiles = state.relay_profiles.lock().await;
        let apps = state.apps.lock().await;
        let incoming_model = body_val.get("model").and_then(|v| v.as_str()).unwrap_or("");

        // Determine incoming protocol from URL path
        let request_protocol = if path.contains("chat/completions") { "chatCompletions" }
            else if path.contains("anthropic") || path.contains("messages") { "anthropic" }
            else { "responses" };

        let selected = routing::select_provider(&apps, &profiles, incoming_model, request_protocol);

        match selected {
            Some((p, upstream_override)) => {
                let bt = match p.protocol.as_str() {
                    "anthropic" => BackendType::Anthropic,
                    "responses" | "openai-responses" => BackendType::OpenaiResponses,
                    _ => BackendType::OpenaiChat,
                };
                let up_model = upstream_override.unwrap_or_else(|| p.model.clone());
                tracing::info!("Route: model={} -> provider={} upstream={} proto={}",
                    incoming_model, p.name, up_model, p.protocol);
                (bt, p.base_url.clone(), p.api_key.clone(), p.protocol.clone(), up_model)
            }
            None => {
                tracing::error!("No enabled provider matched request.");
                return error_response(StatusCode::BAD_REQUEST,
                    "No enabled provider matches this request. Enable a provider and ensure its API key is set.".into());
            }
        }
    };

    // Rewrite model field using the mapped upstream model
    let mut body_val = if let Some(obj) = body_val.as_object() {
        if let Some(model_val) = obj.get("model").and_then(|v| v.as_str()) {
            let upstream_bare = mapped_upstream_model.rsplit('/').next().unwrap_or(&mapped_upstream_model).to_string();
            tracing::info!("Model rewrite: {} -> {}", model_val, upstream_bare);
            let mut new_obj = obj.clone();
            new_obj.insert("model".into(), serde_json::Value::String(upstream_bare));
            serde_json::Value::Object(new_obj)
        } else { body_val }
    } else { body_val };
    // ── OCR Intercept: if request has images and model can't handle them, run OCR ──
    {
        let incoming_model = body_val.get("model").and_then(|v| v.as_str()).unwrap_or("");
        let request_protocol = if path.contains("chat/completions") { "chatCompletions" }
            else if path.contains("anthropic") || path.contains("messages") { "anthropic" }
            else { "responses" };

        // Read global OCR setting
        let (ocr_enabled, ocr_models_ready) = {
            let s = state.settings.lock().await;
            (s.ocr_enabled, s.ocr_models_downloaded)
        };

        // Check per-mapping ocr_fallback
        let mapping_fallback = {
            let apps = state.apps.lock().await;
            apps.iter()
                .find(|a| a.enabled && a.protocol == request_protocol)
                .and_then(|app| app.model_mappings.get(incoming_model))
                .map(|m| m.ocr_fallback)
                .unwrap_or(false)
        };

        let should_ocr = ocr_enabled || mapping_fallback;

        if should_ocr {
            // Look up the selected provider's model capabilities
            let model_capabilities = {
                let profiles = state.relay_profiles.lock().await;
                profiles.iter()
                    .find(|p| p.enabled)
                    .map(|p| p.model_capabilities.clone())
                    .unwrap_or_default()
            };

            let upstream_model = mapped_upstream_model
                .rsplit('/')
                .next()
                .unwrap_or(&mapped_upstream_model);

            let decision = crate::proxy::ocr_interceptor::ocr_intercept(
                &body_val,
                upstream_model,
                &model_capabilities,
                ocr_models_ready,
            );

            match decision {
                crate::proxy::ocr_interceptor::OcrDecision::OcrApplied(modified) => {
                    body_val = modified;
                }
                _ => {} // passthrough or skipped — keep body_val as-is
            }
        }
    }

    let url = routing::profile_url(&base_url, &path, &protocol);

    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(300))
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let req_method: reqwest::Method = method.as_str().parse().unwrap_or(reqwest::Method::POST);
    let mut req_builder = client.request(req_method, &url);

    // Auth headers
    let mut request_headers = HeaderMap::new();
    auth::build_auth_headers(&mut request_headers, &backend_type, &api_key);
    for (k, v) in request_headers {
        if let Some((name, value)) = k.zip(v.to_str().ok()) {
            req_builder = req_builder.header(name, value);
        }
    }

    // Transform request body based on upstream protocol
    if !body.is_empty() {
        let transformed = convert_request(&body_val, &protocol)
            .unwrap_or_else(|e| {
                tracing::error!("Request conversion failed for protocol={}: {}", protocol, e);
                body.to_vec()
            });
        req_builder = req_builder.body(transformed);
    }

    tracing::info!("Forwarding {} {} -> {} (protocol={})", method, path, url, protocol);
    crate::config::push_log(&state, format!("-> {} {} -> {} (proto={})", method, path, url, protocol)).await;

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
        let err_body_str = String::from_utf8_lossy(&bytes).chars().take(500).collect::<String>();
        tracing::error!("Upstream returned {}: {} for url={}", status.as_u16(), err_body_str, url);
        crate::config::push_log(&state, format!("x upstream {} {}", status.as_u16(), url)).await;
        // Convert upstream error to proper Responses API error format
        return upstream_error_response(status, &bytes);
    }

    if is_stream {
        let sse_stream = convert_stream(resp, &protocol, Some(&body_val));

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(Body::from_stream(sse_stream))
            .unwrap()
    } else {
        let bytes = resp.bytes().await.unwrap_or_default();
        let converted = convert_response(&bytes, &protocol, Some(&body_val));

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(converted))
            .unwrap()
    }
}

// -- Protocol conversion helpers --

/// Convert a Codex Responses API request body into the upstream protocol's wire format.
fn convert_request(body: &serde_json::Value, protocol: &str) -> anyhow::Result<Vec<u8>> {
    match protocol {
        "responses" | "openai-responses" | "openai_responses" => {
            Ok(serde_json::to_vec(body)?)
        }
        "chat_completions" | "openai-chat" | "chatCompletions" => {
            match protocol::responses_to_upstream(body, "chat_completions") {
                Ok(v) => Ok(serde_json::to_vec(&v)?),
                Err(e) => Err(anyhow::anyhow!("Chat conversion: {e}")),
            }
        }
        "anthropic" => {
            match protocol::responses_to_upstream(body, "anthropic") {
                Ok(v) => Ok(serde_json::to_vec(&v)?),
                Err(e) => Err(anyhow::anyhow!("Anthropic conversion: {e}")),
            }
        }
        _ => Ok(serde_json::to_vec(body)?),
    }
}

/// Convert upstream JSON response back to Responses API wire format.
fn convert_response(bytes: &[u8], protocol: &str, original_request: Option<&serde_json::Value>) -> Vec<u8> {
    match protocol {
        "responses" | "openai-responses" | "openai_responses" => bytes.to_vec(),
        "chat_completions" | "openai-chat" | "chatCompletions" => {
            match serde_json::from_slice::<serde_json::Value>(bytes) {
                Ok(v) => match protocol::upstream_to_responses(&v, "chat_completions", original_request) {
                    Ok(converted) => serde_json::to_vec(&converted).unwrap_or_else(|_| {
                        tracing::warn!("Failed to serialize chat response conversion result, using raw bytes");
                        bytes.to_vec()
                    }),
                    Err(e) => {
                        tracing::warn!("Chat response conversion failed: {}, using raw bytes", e);
                        bytes.to_vec()
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to parse upstream Chat response as JSON: {}, using raw bytes", e);
                    bytes.to_vec()
                }
            }
        }
        "anthropic" => {
            match serde_json::from_slice::<serde_json::Value>(bytes) {
                Ok(v) => match protocol::upstream_to_responses(&v, "anthropic", None) {
                    Ok(converted) => serde_json::to_vec(&converted).unwrap_or_else(|_| {
                        tracing::warn!("Failed to serialize anthropic response conversion result, using raw bytes");
                        bytes.to_vec()
                    }),
                    Err(e) => {
                        tracing::warn!("Anthropic response conversion failed: {}, using raw bytes", e);
                        bytes.to_vec()
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to parse upstream Anthropic response as JSON: {}, using raw bytes", e);
                    bytes.to_vec()
                }
            }
        }
        _ => bytes.to_vec(),
    }
}

/// Wrap an upstream SSE byte stream into a Responses API SSE event stream.
fn convert_stream(resp: reqwest::Response, protocol: &str, original_request: Option<&serde_json::Value>) -> std::pin::Pin<Box<dyn futures::Stream<Item = io::Result<Bytes>> + Send>> {
    let raw = resp.bytes_stream().map_err(|e| io::Error::new(io::ErrorKind::Other, e));
    match protocol {
        "responses" | "openai-responses" | "openai_responses" => Box::pin(raw),
        "chat_completions" | "openai-chat" | "chatCompletions" => Box::pin(SseTransformStream::new(raw, true, original_request)),
        "anthropic" => Box::pin(SseTransformStream::new(raw, false, None)),
        _ => Box::pin(raw),
    }
}

// -- WebSocket handler --

async fn handle_ws_socket(socket: axum::extract::ws::WebSocket, state: Arc<AppState>) {
    use futures::StreamExt;
    let (mut client_sender, mut client_receiver) = socket.split();

    let first_msg_and_upstream = {
        let msg = client_receiver.next().await;
        match msg {
            Some(Ok(axum::extract::ws::Message::Text(payload))) => {
                let body_val: serde_json::Value = serde_json::from_str(&payload).unwrap_or_default();
                let incoming_model = body_val.get("model").and_then(|v| v.as_str()).unwrap_or("");

                let profiles = state.relay_profiles.lock().await;
                let apps = state.apps.lock().await;
                let selected = routing::select_provider(&apps, &profiles, incoming_model, "responses");

                match selected {
                    Some((p, upstream_override)) => {
                        let upstream_model = upstream_override.unwrap_or_else(|| p.model.clone());
                        let upstream_bare = upstream_model.rsplit('/').next().unwrap_or(&upstream_model);
                        let modified = if let Some(obj) = body_val.as_object() {
                            let mut new_obj = obj.clone();
                            new_obj.insert("model".into(), serde_json::Value::String(upstream_bare.to_string()));
                            format!("{}", serde_json::Value::Object(new_obj))
                        } else { payload.to_string() };

                        let ws_base = p.base_url
                            .replace("http://", "ws://")
                            .replace("https://", "wss://");
                        let up_url = format!("{}/realtime", ws_base.trim_end_matches('/'));

                        tracing::info!("WS proxy: {} -> {} (model: {})", incoming_model, up_url, upstream_bare);
                        Some((modified, up_url))
                    }
                    None => {
                        tracing::warn!("WS proxy: no enabled app found");
                        None
                    }
                }
            }
            _ => None,
        }
    };

    let Some((modified_payload, upstream_url)) = first_msg_and_upstream else { return; };

    let (mut up_sender, mut up_receiver) = match tokio_tungstenite::connect_async(&upstream_url).await {
        Ok((ws_stream, _)) => ws_stream.split(),
        Err(e) => { tracing::error!("WS upstream connection failed: {}", e); return; }
    };

    if let Err(e) = up_sender.send(tokio_tungstenite::tungstenite::Message::text(modified_payload)).await {
        tracing::error!("WS send to upstream failed: {}", e); return;
    }

    let client_to_upstream = async {
        while let Some(msg) = client_receiver.next().await {
            match msg {
                Ok(axum::extract::ws::Message::Text(text)) => {
                    let _ = up_sender.send(tokio_tungstenite::tungstenite::Message::text(text.to_string())).await;
                }
                Ok(axum::extract::ws::Message::Binary(data)) => {
                    let _ = up_sender.send(tokio_tungstenite::tungstenite::Message::binary(data.to_vec())).await;
                }
                Ok(axum::extract::ws::Message::Close(_)) => break,
                _ => {}
            }
        }
    };

    let upstream_to_client = async {
        while let Some(msg) = up_receiver.next().await {
            match msg {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    let _ = client_sender.send(axum::extract::ws::Message::Text(text.into())).await;
                }
                Ok(tokio_tungstenite::tungstenite::Message::Binary(data)) => {
                    let _ = client_sender.send(axum::extract::ws::Message::Binary(data.into())).await;
                }
                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = client_to_upstream => {},
        _ = upstream_to_client => {},
    }
}
