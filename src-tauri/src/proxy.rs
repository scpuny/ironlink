use std::sync::Arc;
use axum::{
    body::Body,
    extract::{Path, State, WebSocketUpgrade},
    http::{header, HeaderMap, Method, StatusCode},
    response::Response,
};
use bytes::Bytes;
use futures::{SinkExt, StreamExt, TryStreamExt};
use std::io;

use crate::config;
use crate::config::AppState;
use crate::convert;
use crate::models::*;
use crate::sse::SseTransformStream;

/// Find a relay profile by parsing the model from the request body.
/// Supports three formats:
///   1. "{provider_id}/{model_name}" — explicit provider routing (e.g. "deepseek/deepseek-v4-flash")
///   2. "{model_name}" — bare model name matched against profile's model_list
///   3. First enabled profile as fallback
fn find_profile<'a>(profiles: &'a [RelayProfile], body: &serde_json::Value) -> Option<&'a RelayProfile> {
    let model = body.get("model").and_then(|v| v.as_str()).unwrap_or("");
    if model.is_empty() {
        return profiles.iter().find(|p| p.enabled);
    }

    // Format 1: Try provider_id/model_name prefix
    if let Some(slash) = model.find('/') {
        let prefix = &model[..slash];
        let name = &model[slash + 1..];
        if let Some(profile) = profiles.iter().find(|p| p.enabled && p.provider_id == prefix) {
            return Some(profile);
        }
        // Also try matching just the name part against profiles
        if let Some(profile) = profiles.iter().find(|p| {
            p.enabled && (p.model_list.iter().any(|m| m == name) || p.model == name)
        }) {
            return Some(profile);
        }
    }

    // Format 2: bare model name — match against any enabled profile's model_list or default model
    if let Some(profile) = profiles.iter().find(|p| {
        p.enabled && (p.model_list.iter().any(|m| m == model) || p.model == model)
    }) {
        return Some(profile);
    }

    // Format 2 (alias): also try with underscores replaced by hyphens and vice versa
    let aliased = model.replace('-', "_");
    if aliased != model {
        if let Some(profile) = profiles.iter().find(|p| {
            p.enabled && (p.model_list.iter().any(|m| m == &aliased) || p.model == aliased)
        }) {
            return Some(profile);
        }
    }
    // Try hyphens instead of underscores
    let aliased = model.replace('_', "-");
    if aliased != model {
        if let Some(profile) = profiles.iter().find(|p| {
            p.enabled && (p.model_list.iter().any(|m| m == &aliased) || p.model == aliased)
        }) {
            return Some(profile);
        }
    }

    // Format 3: fallback to first enabled profile
    profiles.iter().find(|p| p.enabled)
}

/// Build backend URL based on path and relay profile protocol.
fn profile_url(base: &str, path: &str, protocol: &str) -> String {
    let base = base.trim_end_matches('/');
    match (protocol, path) {
        ("anthropic", "responses") | ("anthropic", "chat/completions") => format!("{}/messages", base),
        ("responses", "responses") => format!("{}/responses", base),
        ("responses", "chat/completions") => format!("{}/responses", base),
        ("chatCompletions", "responses") => format!("{}/chat/completions", base),
        _ => format!("{}/{}", base, path),
    }
}

fn is_anthropic(base_url: &str) -> bool {
    base_url.contains("anthropic.com") || base_url.contains("api.claude.ai")
}

/// GET /v1/models — aggregate models from all enabled providers.
/// Returns a proper ModelsResponse matching Codex's expected format,
/// so Codex can display these models in its chat model dropdown.
pub async fn handle_models(
    State(state): State<Arc<AppState>>,
) -> axum::Json<ModelsResponse> {
    tracing::info!("→ GET /v1/models called by Codex");
    let profiles = state.relay_profiles.lock().await;
    let mut models: Vec<ModelInfo> = Vec::new();

    for p in profiles.iter().filter(|p| p.enabled) {
        tracing::debug!("  profile: {} ({}) has {} models", p.name, p.provider_id, p.model_list.len());
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
                display_name: format!("{} — {}", p.name, model_id),
                description: Some(format!("IronLink proxy via {}", p.name)),
                default_reasoning_level: Some("medium".into()),
                supported_reasoning_levels: vec![
                    ReasoningEffortPreset {
                        effort: "low".into(), description: "Low".into(),
                        label: Some("Low".into()), level: Some("low".into()),
                    },
                    ReasoningEffortPreset {
                        effort: "medium".into(), description: "Medium".into(),
                        label: Some("Medium".into()), level: Some("medium".into()),
                    },
                    ReasoningEffortPreset {
                        effort: "high".into(), description: "High".into(),
                        label: Some("High".into()), level: Some("high".into()),
                    },
                ],
                shell_type: "shell_command".into(),
                visibility: "list".into(),
                supported_in_api: true,
                priority: 10,
                additional_speed_tiers: vec![],
                base_instructions: "You are a helpful assistant.".into(),
                supports_reasoning_summaries: true,
                default_reasoning_summary: "auto".into(),
                support_verbosity: true,
                default_verbosity: Some("low".into()),
                web_search_tool_type: "text_and_image".into(),
                truncation_policy: TruncationPolicyConfig {
                    mode: "tokens".into(),
                    limit: 10000,
                },
                supports_parallel_tool_calls: true,
                supports_image_detail_original: true,
                context_window: Some(128000),
                max_context_window: Some(128000),
                auto_compact_token_limit: Some(65536),
                effective_context_window_percent: 95,
                experimental_supported_tools: vec![],
                input_modalities: vec!["text".into()],
                supports_search_tool: true,
                use_responses_lite: false,
                apply_patch_tool_type: Some("freeform".into()),
                auto_review_model_override: None,
            });
        }
    }

    // Also include any models manually configured via the legacy ModelList page
    {
        let legacy_models = state.models.lock().await;
        let mut seen = std::collections::HashSet::new();
        for m in legacy_models.iter() {
            if !seen.insert(&m.id) { continue; }
            models.push(ModelInfo {
                slug: m.id.clone(),
                display_name: format!("Custom — {}", m.id),
                description: Some("Custom model".into()),
                default_reasoning_level: Some("medium".into()),
                supported_reasoning_levels: vec![],
                shell_type: "shell_command".into(),
                visibility: "list".into(),
                supported_in_api: true,
                priority: 0,
                additional_speed_tiers: vec![],
                base_instructions: "You are a helpful assistant.".into(),
                supports_reasoning_summaries: false,
                default_reasoning_summary: "auto".into(),
                support_verbosity: false,
                default_verbosity: None,
                web_search_tool_type: "text_and_image".into(),
                truncation_policy: TruncationPolicyConfig {
                    mode: "tokens".into(),
                    limit: 10000,
                },
                supports_parallel_tool_calls: false,
                supports_image_detail_original: false,
                context_window: None,
                max_context_window: None,
                auto_compact_token_limit: None,
                effective_context_window_percent: 90,
                experimental_supported_tools: vec![],
                input_modalities: vec!["text".into()],
                supports_search_tool: false,
                use_responses_lite: false,
                apply_patch_tool_type: None,
                auto_review_model_override: None,
            });
        }
    }

    let response = ModelsResponse { models };
    tracing::info!("← GET /v1/models returning {} models", response.models.len());
    axum::Json(response)
}

/// Catch-all: proxy /v1/*path, routing to the correct provider based on model
pub async fn handle_proxy(
    State(state): State<Arc<AppState>>,
    method: Method,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    // Log ALL incoming requests for debugging
    tracing::info!("→ {} /v1/{} ({} bytes body)", method, path, body.len());

    // Check if proxy is enabled
    if !*state.proxy_enabled.lock().await {
        crate::config::push_log(&state, "✗ proxy disabled".into()).await;
        tracing::warn!("Request rejected: proxy is disabled");
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "Proxy is disabled".into());
    }

    // Reject WebSocket upgrade requests — our proxy does not support WebSocket proxying.
    // Codex should fall back to SSE streaming over HTTP.
    if headers
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
    {
        tracing::warn!("WebSocket upgrade rejected for /v1/{} — use Responses API SSE instead", path);
        return error_response(
            StatusCode::BAD_REQUEST,
            "WebSocket not supported by proxy. Use Responses API with stream: true instead.".into(),
        );
    }

    // Parse model from request body, find matching provider
    let profiles = state.relay_profiles.lock().await;
    let mappings = state.model_mappings.lock().await;
    let body_val: serde_json::Value = if !body.is_empty() {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid JSON body".into()),
        }
    } else {
        serde_json::Value::Null
    };

    // Check if the incoming model name has a mapping
    let incoming_model = body_val.get("model").and_then(|v| v.as_str()).unwrap_or("");
    let mapped_model = config::resolve_mapping(&mappings, incoming_model);

    let profile = if let Some(mapping) = mapped_model {
        // Use the mapped profile
        let p = profiles.iter().find(|p| p.enabled && p.id == mapping.profile_id);
        if p.is_none() {
            tracing::warn!("Mapping for '{}' points to unknown profile '{}', falling back", incoming_model, mapping.profile_id);
        }
        p
    } else {
        find_profile(&profiles, &body_val)
    };

    if profile.is_none() {
        crate::config::push_log(&state, format!("✗ no matching provider for {}", path)).await;
        tracing::warn!("No matching profile found for path={} model={}", path, incoming_model);
    }

    // Extract all needed fields from profile while we still have the borrow on profiles
    let (backend_type, base_url, api_key, protocol, _profile_model, _profile_model_list, mapped_upstream_model) = match profile {
        Some(p) => {
            let bt = match p.protocol.as_str() {
                "responses" => BackendType::OpenaiResponses,
                "anthropic" => BackendType::Anthropic,
                _ => BackendType::OpenaiChat,
            };
            let anthro = is_anthropic(&p.base_url);
            let bt = if p.protocol.as_str() == "anthropic" || anthro { BackendType::Anthropic } else { bt };

            // If there's a mapping, use the mapped upstream model name
            let mapped_model = mapped_model.map(|m| m.upstream_model.clone());
            let up_model = mapped_model.unwrap_or_else(|| p.model.clone());

            (bt, p.base_url.clone(), p.api_key.clone(), p.protocol.clone(), p.model.clone(), p.model_list.clone(), up_model)
        }
        None => {
            tracing::error!("No enabled provider matched request. Check providers are enabled and have valid API keys.");
            return error_response(StatusCode::BAD_REQUEST, "No enabled provider matches this request. Enable a provider and ensure its API key is set.".into());
        }
    };
    drop(profiles);
    drop(mappings);

    // Rewrite model field using the mapped upstream model
    let body_val = if let Some(obj) = body_val.as_object() {
        if let Some(model_val) = obj.get("model").and_then(|v| v.as_str()) {
            // Strip any prefix from mapped model to get the bare upstream model name
            let upstream_bare = if let Some(slash) = mapped_upstream_model.find('/') {
                mapped_upstream_model[slash + 1..].to_string()
            } else {
                mapped_upstream_model.clone()
            };

            tracing::info!("Model mapping: {} → {} (bare: {})", model_val, mapped_upstream_model, upstream_bare);

            let mut new_obj = obj.clone();
            new_obj.insert("model".into(), serde_json::Value::String(upstream_bare));
            serde_json::Value::Object(new_obj)
        } else {
            body_val
        }
    } else {
        body_val
    };

    let url = profile_url(&base_url, &path, &protocol);

    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap();

    let req_method: reqwest::Method = method.as_str().parse().unwrap_or(reqwest::Method::POST);
    let mut req_builder = client.request(req_method, &url);

    // Auth headers
    match &backend_type {
        BackendType::Anthropic => {
            if !api_key.is_empty() {
                req_builder = req_builder.header("x-api-key", &api_key);
                req_builder = req_builder.header("anthropic-version", "2023-06-01");
            }
        }
        _ => {
            if !api_key.is_empty() {
                req_builder = req_builder.header(header::AUTHORIZATION, format!("Bearer {}", api_key));
            }
        }
    }
    req_builder = req_builder.header(header::CONTENT_TYPE, "application/json");

    // Transform request body
    if !body.is_empty() {
        let transformed = match &backend_type {
            BackendType::OpenaiChat if path == "responses" => {
                match convert::responses_to_chat_request(&body_val) {
                    Ok(chat_req) => serde_json::to_vec(&chat_req).unwrap_or_default(),
                    Err(e) => return error_response(StatusCode::BAD_REQUEST, format!("Conversion: {e}")),
                }
            }
            BackendType::Anthropic if path == "responses" => {
                match convert::responses_to_anthropic_request(&body_val) {
                    Ok(anth_req) => serde_json::to_vec(&anth_req).unwrap_or_default(),
                    Err(e) => return error_response(StatusCode::BAD_REQUEST, format!("Conversion: {e}")),
                }
            }
            _ => body.to_vec(),
        };
        req_builder = req_builder.body(transformed);
    }

    tracing::info!("Forwarding {} {} -> {} (protocol={})", method, path, url, protocol);
    crate::config::push_log(&state, format!("→ {} {} -> {} (proto={})", method, path, url, protocol)).await;
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
        let err_body = String::from_utf8_lossy(&bytes).chars().take(500).collect::<String>();
        tracing::error!("Upstream returned {}: {} for url={}", status.as_u16(), err_body, url);
        crate::config::push_log(&state, format!("✗ upstream {} {}", status.as_u16(), url)).await;
        return Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(bytes))
            .unwrap();
    }

    if is_stream {
        let is_chat = backend_type == BackendType::OpenaiChat || protocol == "chatCompletions";
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
        let bytes = resp.bytes().await.unwrap_or_default();
        let converted = if !bytes.is_empty() {
            match backend_type {
                BackendType::OpenaiChat => {
                    if let Ok(chat) = serde_json::from_slice::<ChatResponse>(&bytes) {
                        let id = chat.model.clone();
                        serde_json::to_vec(&convert::chat_to_responses_response(&chat, &id))
                            .unwrap_or_else(|_| bytes.to_vec())
                    } else {
                        bytes.to_vec()
                    }
                }
                BackendType::Anthropic => {
                    if let Ok(anth) = serde_json::from_slice::<AnthropicResponse>(&bytes) {
                        let id = anth.model.clone();
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

/// GET /v1/responses/websocket or /v1/realtime — proxy WebSocket connections
/// to the mapped upstream provider. Model routing is done via model_mappings.
pub async fn handle_websocket(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response<Body> {
    ws.on_upgrade(move |client_ws| async move {
        let (mut client_sender, mut client_receiver) = client_ws.split();

        // Wait for the first message from the client to determine the model
        let first_msg = match client_receiver.next().await {
            Some(Ok(msg)) => msg,
            _ => return,
        };

        // Try to extract model name from the first message
        let payload = match &first_msg {
            axum::extract::ws::Message::Text(text) => text.to_string(),
            axum::extract::ws::Message::Binary(data) => String::from_utf8_lossy(data).to_string(),
            _ => {
                let _ = client_sender.send(first_msg).await;
                return;
            }
        };

        // Parse the model from the message payload
        let (modified_payload, upstream_url) = {
            let profiles = state.relay_profiles.lock().await;
            let mappings = state.model_mappings.lock().await;

            let body_val: serde_json::Value = serde_json::from_str(&payload).unwrap_or_default();
            let incoming_model = body_val.get("model").and_then(|v| v.as_str()).unwrap_or("");

            let mapped = config::resolve_mapping(&mappings, incoming_model);
            let profile = mapped
                .and_then(|m| profiles.iter().find(|p| p.enabled && p.id == m.profile_id))
                .or_else(|| profiles.iter().find(|p| p.enabled));

            match profile {
                Some(p) => {
                    // Get the upstream model name to use
                    let upstream_model = mapped
                        .map(|m| {
                            m.upstream_model.split('/').last().unwrap_or(&m.upstream_model).to_string()
                        })
                        .unwrap_or_else(|| p.model.clone());

                    // Rewrite the model field in the message
                    let modified = if let Some(obj) = body_val.as_object() {
                        let mut new_obj = obj.clone();
                        new_obj.insert("model".into(), serde_json::Value::String(upstream_model.clone()));
                        serde_json::to_string(&serde_json::Value::Object(new_obj))
                            .unwrap_or(payload.clone())
                    } else {
                        payload.clone()
                    };

                    // Build upstream WebSocket URL
                    let ws_base = p.base_url
                        .replace("http://", "ws://")
                        .replace("https://", "wss://");
                    let up_url = format!("{}/realtime", ws_base.trim_end_matches('/'));

                    tracing::info!("WebSocket proxy: {} -> {} (model: {})",
                        incoming_model, up_url, upstream_model);
                    (modified, up_url)
                }
                None => {
                    tracing::warn!("WebSocket proxy: no enabled profile found");
                    (payload.clone(), String::new())
                }
            }
        };

        if upstream_url.is_empty() {
            return;
        }

        // Connect to upstream WebSocket
        let (mut up_sender, mut up_receiver) = match tokio_tungstenite::connect_async(&upstream_url).await {
            Ok((ws_stream, _)) => ws_stream.split(),
            Err(e) => {
                tracing::error!("WebSocket upstream connection failed: {}", e);
                return;
            }
        };

        // Send the first (modified) message to upstream
        if let Err(e) = up_sender
            .send(tokio_tungstenite::tungstenite::Message::text(modified_payload))
            .await
        {
            tracing::error!("WebSocket send to upstream failed: {}", e);
            return;
        }

        // Bidirectionally forward messages
        let client_to_upstream = async move {
            while let Some(msg) = client_receiver.next().await {
                match msg {
                    Ok(axum::extract::ws::Message::Text(text)) => {
                        let s: String = text.to_string();
                        let _ = up_sender
                            .send(tokio_tungstenite::tungstenite::Message::text(s))
                            .await;
                    }
                    Ok(axum::extract::ws::Message::Binary(data)) => {
                        let _ = up_sender
                            .send(tokio_tungstenite::tungstenite::Message::binary(data.to_vec()))
                            .await;
                    }
                    Ok(axum::extract::ws::Message::Close(_)) => break,
                    _ => {}
                }
            }
        };

        let upstream_to_client = async move {
            while let Some(msg) = up_receiver.next().await {
                match msg {
                    Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                        let _ = client_sender
                            .send(axum::extract::ws::Message::Text(text.into()))
                            .await;
                    }
                    Ok(tokio_tungstenite::tungstenite::Message::Binary(data)) => {
                        let _ = client_sender
                            .send(axum::extract::ws::Message::Binary(data.into()))
                            .await;
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
    })
}
