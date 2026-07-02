// ── GET /api/status, GET /api/logs, GET /api/models ──

use std::sync::Arc;
use axum::{extract::State, Json};

use crate::config::AppState;
use crate::models::*;

/// GET /api/status — return current proxy status to the frontend.
pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<ProxyStatus> {
    let enabled = *state.proxy_enabled.lock().await;
    let profiles = state.relay_profiles.lock().await.clone();
    let (backend, api_base) = match profiles.iter().find(|p| p.enabled) {
        Some(p) => (p.protocol.clone(), p.base_url.clone()),
        None => ("none".to_string(), String::new()),
    };
    Json(ProxyStatus { enabled, backend, api_base, port: crate::config::proxy_port() })
}

/// GET /api/logs — return recent proxy activity logs.
pub async fn get_logs(State(state): State<Arc<AppState>>) -> Json<Vec<String>> {
    Json(state.log_buffer.lock().await.clone())
}

/// GET /api/models — return the list of managed models.
pub async fn get_models(State(state): State<Arc<AppState>>) -> Json<Vec<ModelEntry>> {
    Json(state.models.lock().await.clone())
}

/// PUT /api/models — replace the managed models list.
pub async fn put_models(State(state): State<Arc<AppState>>, Json(models): Json<Vec<ModelEntry>>) -> axum::http::StatusCode {
    *state.models.lock().await = models;
    axum::http::StatusCode::OK
}
