// ── Management API handlers for the UI ──

use std::sync::Arc;
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::config::{self, AppState, PROXY_PORT};
use crate::models::*;

// ── GET /api/status ──

pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Json<ProxyStatus> {
    let enabled = *state.proxy_enabled.lock().await;
    let backend = state.backend.lock().await.clone();
    Json(ProxyStatus {
        enabled,
        backend: backend.backend_type.to_string(),
        api_base: backend.api_base,
        port: PROXY_PORT,
    })
}

// ── GET /api/backend ──

pub async fn get_backend(
    State(state): State<Arc<AppState>>,
) -> Json<BackendConfig> {
    let backend = state.backend.lock().await.clone();
    Json(backend)
}

// ── PUT /api/backend ──

pub async fn put_backend(
    State(state): State<Arc<AppState>>,
    Json(backend): Json<BackendConfig>,
) -> StatusCode {
    *state.backend.lock().await = backend;
    StatusCode::OK
}

// ── GET /api/models ──

pub async fn get_models(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ModelEntry>> {
    let models = state.models.lock().await.clone();
    Json(models)
}

// ── PUT /api/models ──

pub async fn put_models(
    State(state): State<Arc<AppState>>,
    Json(models): Json<Vec<ModelEntry>>,
) -> StatusCode {
    *state.models.lock().await = models;
    StatusCode::OK
}

// ── POST /api/proxy ──

#[derive(Deserialize)]
pub struct ToggleBody {
    enabled: bool,
}

pub async fn toggle_proxy_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ToggleBody>,
) -> Json<bool> {
    let pcfg = state.proxy_config.lock().await.clone();
    let result = config::toggle_proxy(body.enabled, &pcfg.default_model, &pcfg.reasoning_effort);
    *state.proxy_enabled.lock().await = body.enabled;
    Json(result)
}

// ── GET /api/logs ──
// For now, returns an empty log list. A future enhancement could use file-based logging.

pub async fn get_logs(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<String>> {
    Json(state.log_buffer.lock().await.clone())
}

// ── GET /api/config ──

pub async fn get_config() -> (StatusCode, String) {
    let content = config::read_raw();
    (StatusCode::OK, content)
}

// ── PUT /api/config ──

#[derive(Deserialize)]
pub struct ConfigBody {
    content: String,
}

pub async fn put_config(
    Json(body): Json<ConfigBody>,
) -> StatusCode {
    match config::write_raw(&body.content) {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            tracing::error!("Failed to write config: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

// ── GET /api/profiles ──

pub async fn get_profiles(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<RelayProfile>> {
    let profiles = state.relay_profiles.lock().await.clone();
    Json(profiles)
}

// ── PUT /api/profiles ──

pub async fn put_profiles(
    State(state): State<Arc<AppState>>,
    Json(profiles): Json<Vec<RelayProfile>>,
) -> StatusCode {
    *state.relay_profiles.lock().await = profiles.clone();
    if let Err(e) = config::write_profiles(&profiles) {
        tracing::warn!("Failed to persist profiles: {e}");
    }
    // Sync active profile to legacy backend
    if let Some(active) = profiles.iter().find(|p| p.active) {
        let backend = BackendConfig {
            backend_type: match active.protocol.as_str() {
                "responses" => BackendType::OpenaiResponses,
                _ => BackendType::OpenaiChat,
            },
            api_base: active.base_url.clone(),
            api_key: active.api_key.clone(),
            name: Some(active.name.clone()),
            model: if active.model.is_empty() { None } else { Some(active.model.clone()) },
            test_model: if active.test_model.is_empty() { None } else { Some(active.test_model.clone()) },
            auth_type: Some("bearer".into()),
            custom_headers: None,
            config_contents: None,
            user_agent: None,
        };
        *state.backend.lock().await = backend;
    }
    StatusCode::OK
}

// ── POST /api/profiles/activate ──

#[derive(Deserialize)]
pub struct ActivateProfileBody {
    pub id: String,
}

pub async fn post_profiles_activate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ActivateProfileBody>,
) -> StatusCode {
    let mut profiles = state.relay_profiles.lock().await;
    for p in profiles.iter_mut() {
        p.active = p.id == body.id;
    }
    if let Err(e) = config::write_profiles(&profiles) {
        tracing::warn!("Failed to persist profiles: {e}");
    }
    drop(profiles);
    // Sync active profile to legacy backend
    let profiles = state.relay_profiles.lock().await;
    if let Some(active) = profiles.iter().find(|p| p.active) {
        let backend = BackendConfig {
            backend_type: match active.protocol.as_str() {
                "responses" => BackendType::OpenaiResponses,
                _ => BackendType::OpenaiChat,
            },
            api_base: active.base_url.clone(),
            api_key: active.api_key.clone(),
            name: Some(active.name.clone()),
            model: if active.model.is_empty() { None } else { Some(active.model.clone()) },
            test_model: if active.test_model.is_empty() { None } else { Some(active.test_model.clone()) },
            auth_type: Some("bearer".into()),
            custom_headers: None,
            config_contents: None,
            user_agent: None,
        };
        drop(profiles);
        *state.backend.lock().await = backend;
    }
    StatusCode::OK
}



