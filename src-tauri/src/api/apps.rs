//! App CRUD API handlers.

use std::sync::Arc;
use axum::{extract::State, http::StatusCode, Json};

use crate::config::{self, AppState};
use crate::models::AppConfig;

/// GET /api/apps — return all apps.
pub async fn get_apps(State(state): State<Arc<AppState>>) -> Json<Vec<AppConfig>> {
    Json(state.apps.lock().await.clone())
}

/// PUT /api/apps — replace all apps.
pub async fn put_apps(State(state): State<Arc<AppState>>, Json(apps): Json<Vec<AppConfig>>) -> StatusCode {
    *state.apps.lock().await = apps.clone();
    if let Err(e) = config::apps::write(&apps) { tracing::warn!("Failed to persist apps: {e}"); }
    StatusCode::OK
}
