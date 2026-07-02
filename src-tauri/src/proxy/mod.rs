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

pub use self::error::error_response;

/// GET /v1/models — aggregate models from all enabled apps.
pub async fn handle_models(
    State(state): State<Arc<AppState>>,
) -> axum::Json<ModelsResponse> {
    tracing::info!("-> GET /v1/models called by Codex");
    let profiles = state.relay_profiles.lock().await;
    let mut models: Vec<ModelInfo> = Vec::new();

    for p in profiles.iter().filter(|p| p.enabled) {
        let mut seen = std::collections::HashSet::<String>::new();
        let all_models: Vec<&str> = p.model_list
            .iter()
            .flat_map(|m| m.split_whitespace())
            .chain(std::iter::once(p.model.as_str()))
            .filter(|m| !m.is_empty())
            .collect();

        for model_id in all_models {
            if !seen.insert(model_id.to_string()) { continue; }
            let slug = format!("{}/{}", p.provider_id, model_id);
            models.push(ModelInfo {
                slug: slug.clone(),
                display_name: format!("{} -- {}", p.name, model_id),
                description: Some(format!("IronLink proxy via {}", p.name)),
                default_reasoning_level: Some("medium".into()),
                supported_reasoning_levels: vec![
                    ReasoningEffortPreset { effort: "low".into(), description: "Low".into(), label: Some("Low".into()), level: Some("low".into()) },
                    ReasoningEffortPreset { effort: "medium".into(), description: "Medium".into(), label: Some("Medium".into()), level: Some("medium".into()) },
                    ReasoningEffortPreset { effort: "high".into(), description: "High".into(), label: Some("High".into()), level: Some("high".into()) },
                ],
                shell_type: "macOS".into(), visibility: "users".into(),
                supported_in_api: false, priority: 0,
                additional_speed_tiers: vec![], base_instructions: String::new(),
                supports_reasoning_summaries: true, default_reasoning_summary: "concise".into(),
                support_verbosity: true, default_verbosity: Some("high".into()),
                web_search_tool_type: "disabled".into(),
                truncation_policy: TruncationPolicyConfig { auto: true, enabled: true, max_tokens: None },
                supports_parallel_tool_calls: true,
                supports_image_detail_original: false,
                context_window: None, max_context_window: None,
                auto_compact_token_limit: None,
                effective_context_window_percent: 90,
                experimental_supported_tools: vec![],
                input_modalities: vec!["text".into()],
                supports_search_tool: false, use_responses_lite: false,
                apply_patch_tool_type: None,
                auto_review_model_override: None,
            });
        }
    }
    axum::Json(ModelsResponse { models })
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
        else { serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null) };

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
    let body_val = if let Some(obj) = body_val.as_object() {
        if let Some(model_val) = obj.get("model").and_then(|v| v.as_str()) {
            let upstream_bare = mapped_upstream_model.rsplit('/').next().unwrap_or(&mapped_upstream_model).to_string();
            tracing::info!("Model rewrite: {} -> {}", model_val, upstream_bare);
            let mut new_obj = obj.clone();
            new_obj.insert("model".into(), serde_json::Value::String(upstream_bare));
            serde_json::Value::Object(new_obj)
        } else { body_val }
    } else { body_val };

    let url = routing::profile_url(&base_url, &path, &protocol);

    let client = reqwest::Client::builder()
        .no_proxy()
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
        let err_body = String::from_utf8_lossy(&bytes).chars().take(500).collect::<String>();
        tracing::error!("Upstream returned {}: {} for url={}", status.as_u16(), err_body, url);
        crate::config::push_log(&state, format!("x upstream {} {}", status.as_u16(), url)).await;
        return Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(bytes))
            .unwrap();
    }

    if is_stream {
        let sse_stream = convert_stream(resp, &protocol);

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(Body::from_stream(sse_stream))
            .unwrap()
    } else {
        let bytes = resp.bytes().await.unwrap_or_default();
        let converted = convert_response(&bytes, &protocol);

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
fn convert_response(bytes: &[u8], protocol: &str) -> Vec<u8> {
    match protocol {
        "responses" | "openai-responses" | "openai_responses" => bytes.to_vec(),
        "chat_completions" | "openai-chat" | "chatCompletions" => {
            serde_json::from_slice::<serde_json::Value>(bytes)
                .ok()
                .and_then(|v| protocol::upstream_to_responses(&v, "chat_completions").ok())
                .and_then(|v| serde_json::to_vec(&v).ok())
                .unwrap_or_else(|| bytes.to_vec())
        }
        "anthropic" => {
            serde_json::from_slice::<serde_json::Value>(bytes)
                .ok()
                .and_then(|v| protocol::upstream_to_responses(&v, "anthropic").ok())
                .and_then(|v| serde_json::to_vec(&v).ok())
                .unwrap_or_else(|| bytes.to_vec())
        }
        _ => bytes.to_vec(),
    }
}

/// Wrap an upstream SSE byte stream into a Responses API SSE event stream.
fn convert_stream(resp: reqwest::Response, protocol: &str) -> std::pin::Pin<Box<dyn futures::Stream<Item = io::Result<Bytes>> + Send>> {
    let raw = resp.bytes_stream().map_err(|e| io::Error::new(io::ErrorKind::Other, e));
    match protocol {
        "responses" | "openai-responses" | "openai_responses" => Box::pin(raw),
        "chat_completions" | "openai-chat" | "chatCompletions" => Box::pin(SseTransformStream::new(raw, true)),
        "anthropic" => Box::pin(SseTransformStream::new(raw, false)),
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
