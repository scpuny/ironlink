// ── Backend config + proxy toggle + config file ──

use std::sync::Arc;
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::config::{self, AppState};
use crate::models::*;


/// GET /api/backend — return the current backend configuration.
pub async fn get_backend(State(state): State<Arc<AppState>>) -> Json<BackendConfig> {
    Json(state.backend.lock().await.clone())
}

/// PUT /api/backend — update the backend configuration.
pub async fn put_backend(State(state): State<Arc<AppState>>, Json(backend): Json<BackendConfig>) -> StatusCode {
    *state.backend.lock().await = backend;
    StatusCode::OK
}


#[derive(Deserialize)]
/// Request body for the proxy toggle endpoint.
pub struct ToggleBody { enabled: bool }

/// POST /api/proxy — enable or disable the proxy (HTTP API, delegates to toggle command).
pub async fn toggle_proxy_handler(State(state): State<Arc<AppState>>, Json(body): Json<ToggleBody>) -> Json<bool> {
    let pcfg = state.proxy_config.lock().await.clone();
    let profiles = state.relay_profiles.lock().await.clone();
    let apps = state.apps.lock().await.clone();
    let result = config::toggle_proxy(body.enabled, &pcfg.default_model, &pcfg.reasoning_effort, &profiles, &apps);
    *state.proxy_enabled.lock().await = body.enabled;
    Json(result)
}


/// GET /api/config — return the raw config.toml content.
pub async fn get_config() -> (StatusCode, String) {
    (StatusCode::OK, config::read_raw())
}

#[derive(Deserialize)]
/// Request body for writing raw config.
pub struct ConfigBody { content: String }

/// PUT /api/config — write raw config.toml content.
pub async fn put_config(Json(body): Json<ConfigBody>) -> StatusCode {
    match config::write_raw(&body.content) {
        Ok(_) => StatusCode::OK,
        Err(e) => { tracing::error!("Failed to write config: {e}"); StatusCode::INTERNAL_SERVER_ERROR }
    }
}
