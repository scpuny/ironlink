//! Profile CRUD API handlers.

use std::sync::Arc;
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::config::{self, AppState};
use crate::models::*;

/// GET /api/profiles — return all relay profiles (apps).
pub async fn get_profiles(State(state): State<Arc<AppState>>) -> Json<Vec<RelayProfile>> {
    Json(state.relay_profiles.lock().await.clone())
}

/// PUT /api/profiles — replace all relay profiles.
pub async fn put_profiles(State(state): State<Arc<AppState>>, Json(profiles): Json<Vec<RelayProfile>>) -> StatusCode {
    *state.relay_profiles.lock().await = profiles.clone();
    if let Err(e) = config::profiles::write(&profiles) { tracing::warn!("Failed to persist profiles: {e}"); }
    StatusCode::OK
}

#[derive(Deserialize)]
/// Request body for profile activation.
pub struct ActivateProfileBody { pub id: String }

/// POST /api/profiles/activate — set a profile as active.
pub async fn post_profiles_activate(State(state): State<Arc<AppState>>, Json(body): Json<ActivateProfileBody>) -> StatusCode {
    let mut profiles = state.relay_profiles.lock().await;
    for p in profiles.iter_mut() { p.active = p.id == body.id; }
    if let Err(e) = config::profiles::write(&profiles) { tracing::warn!("Failed to persist profiles: {e}"); }
    StatusCode::OK
}
