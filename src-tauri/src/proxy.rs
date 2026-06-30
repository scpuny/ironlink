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

/// Find a relay profile by parsing the model from the request body.
/// Model format: `{provider_id}/{model_name}` or bare `{model_name}`.
fn find_profile<'a>(profiles: &'a [RelayProfile], body: &serde_json::Value) -> Option<&'a RelayProfile> {
    let model = body.get("model").and_then(|v| v.as_str()).unwrap_or("");
    if model.is_empty() {
        // Fall back to first enabled profile
        return profiles.iter().find(|p| p.enabled);
    }
    // Try provider_id/model_name prefix
    if let Some(slash) = model.find('/') {
        let prefix = &model[..slash];
        if let Some(profile) = profiles.iter().find(|p| p.enabled && p.provider_id == prefix) {
            return Some(profile);
        }
    }
    // Fall back: match model name against any enabled profile's model_list
    profiles.iter().find(|p| {
        if !p.enabled { return false; }
        p.model_list.iter().any(|m| m == model) || p.model == model
    })
    .or_else(|| profiles.iter().find(|p| p.enabled))
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

/// GET /v1/models — aggregate models from all enabled providers
pub async fn handle_models(
    State(state): State<Arc<AppState>>,
) -> axum::Json<serde_json::Value> {
    let profiles = state.relay_profiles.lock().await;
    let mut entries: Vec<serde_json::Value> = Vec::new();

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

            // Bare model ID (e.g. "deepseek-chat")
            entries.push(serde_json::json!({
                "id": model_id.to_string(),
                "object": "model",
                "created": chrono::Utc::now().timestamp(),
                "owned_by": "custom",
                "metadata": {
                    "provider_id": p.provider_id,
                    "display_name": format!("{} — {}", p.name, model_id),
                    "visibility": "list",
                    "context_window": 128000,
                    "supports_responses_api": true,
                }
            }));

            // Also emit a prefixed ID (e.g. "deepseek/deepseek-chat") so Codex can
            // associate the model with the correct relay profile via model routing.
            // This matches the `{provider_id}/{model}` format used in config.toml.
            let prefixed_id = format!("{}/{}", p.provider_id, model_id);
            if seen.insert(prefixed_id.clone()) {
                entries.push(serde_json::json!({
                    "id": prefixed_id,
                    "object": "model",
                    "created": chrono::Utc::now().timestamp(),
                    "owned_by": "custom",
                    "metadata": {
                        "provider_id": p.provider_id,
                        "display_name": format!("{} — {} ({})", p.name, model_id, prefixed_id),
                        "visibility": "list",
                        "context_window": 128000,
                        "supports_responses_api": true,
                    }
                }));
            }
        }
    }

    // Also include any models manually configured via the legacy ModelList page
    {
        let legacy_models = state.models.lock().await;
        let mut legacy_seen = std::collections::HashSet::new();
        for m in legacy_models.iter() {
            if !legacy_seen.insert(&m.id) { continue; }
            entries.push(serde_json::json!({
                "id": m.id,
                "object": "model",
                "created": m.created,
                "owned_by": "custom",
                "metadata": {
                    "provider_id": m.owned_by,
                    "display_name": format!("Custom — {}", m.id),
                    "visibility": "list",
                    "context_window": 128000,
                    "supports_responses_api": true,
                }
            }));
        }
    }

    axum::Json(serde_json::json!({
        "object": "list",
        "data": entries,
    }))
}

/// Catch-all: proxy /v1/*path, routing to the correct provider based on model
pub async fn handle_proxy(
    State(state): State<Arc<AppState>>,
    method: Method,
    Path(path): Path<String>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    // Check if proxy is enabled
    if !*state.proxy_enabled.lock().await {
        crate::config::push_log(&state, "✗ proxy disabled".into()).await;
        tracing::warn!("Request rejected: proxy is disabled");
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "Proxy is disabled".into());
    }
    // Parse model from request body, find matching provider
    let profiles = state.relay_profiles.lock().await;
    let body_val: serde_json::Value = if !body.is_empty() {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid JSON body".into()),
        }
    } else {
        serde_json::Value::Null
    };

    let profile = find_profile(&profiles, &body_val);
    if profile.is_none() {
        crate::config::push_log(&state, format!("✗ no matching provider for {}", path)).await;
        tracing::warn!("No matching profile found for path={} body={}", path, String::from_utf8_lossy(&body).lines().next().unwrap_or(""));
    }

    // Extract all needed fields from profile while we still have the borrow on profiles
    let (backend_type, base_url, api_key, protocol, profile_model, profile_model_list) = match profile {
        Some(p) => {
            let bt = match p.protocol.as_str() {
                "responses" => BackendType::OpenaiResponses,
                "anthropic" => BackendType::Anthropic,
                _ => BackendType::OpenaiChat,
            };
            let anthro = is_anthropic(&p.base_url);
            let bt = if p.protocol.as_str() == "anthropic" || anthro { BackendType::Anthropic } else { bt };
            (bt, p.base_url.clone(), p.api_key.clone(), p.protocol.clone(), p.model.clone(), p.model_list.clone())
        }
        None => {
            tracing::error!("No enabled provider matched request. Check providers are enabled and have valid API keys.");
            return error_response(StatusCode::BAD_REQUEST, "No enabled provider matches this request. Enable a provider and ensure its API key is set.".into());
        }
    };
    drop(profiles);

    // Rewrite model field:
    // 1. Strip providerId/ prefix if present (e.g. "deepseek/deepseek-chat" → "deepseek-chat")
    // 2. If the resulting model is NOT in the profile's model_list (and not the profile's default model),
    //    replace it with the profile's default model to avoid sending invalid models to the upstream.
    let body_val = if let Some(obj) = body_val.as_object() {
        if let Some(model_val) = obj.get("model").and_then(|v| v.as_str()) {
            // Strip prefix
            let stripped = if let Some(slash) = model_val.find('/') {
                model_val[slash + 1..].to_string()
            } else {
                model_val.to_string()
            };

            // Check if the model is known to this profile
            let is_valid = profile_model_list.iter().any(|m| m == &stripped) || profile_model == stripped;
            let final_model = if is_valid || stripped.is_empty() {
                stripped
            } else {
                tracing::warn!(
                    "Model '{}' is not in profile '{}' model list, replacing with '{}'",
                    stripped, profile_model, profile_model
                );
                profile_model.clone()
            };

            let mut new_obj = obj.clone();
            new_obj.insert("model".into(), serde_json::Value::String(final_model));
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
