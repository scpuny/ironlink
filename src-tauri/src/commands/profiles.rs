//! Profile commands for Tauri.

use std::sync::Arc;
use tauri::State;

use crate::config::{self, AppState};
use crate::models::*;

#[tauri::command]
/// Return all relay profiles (apps).
pub async fn get_profiles(state: State<'_, Arc<AppState>>) -> Result<Vec<RelayProfile>, String> {
    Ok(state.relay_profiles.lock().await.clone())
}

#[tauri::command]
/// Save relay profiles.
pub async fn save_profiles(state: State<'_, Arc<AppState>>, profiles: Vec<RelayProfile>) -> Result<(), String> {
    // Debug: log what we're saving
    for p in &profiles {
        tracing::info!(
            "save_profiles: name={}, ctx_windows={:?}, max_ctx={:?}, caps={:?}",
            p.name, p.model_context_windows, p.model_max_context_windows, p.model_capabilities
        );
    }
    *state.relay_profiles.lock().await = profiles.clone();
    if let Err(e) = config::profiles::write(&state.relay_profiles.lock().await) {
        tracing::error!("Failed to persist profiles: {e}");
        return Err(format!("Failed to persist profiles: {e}"));
    }
    Ok(())
}

#[tauri::command]
/// Activate a relay profile by ID.
pub async fn activate_profile(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let mut profiles = state.relay_profiles.lock().await;
    for p in profiles.iter_mut() { p.active = p.id == id; }
    if let Err(e) = config::profiles::write(&profiles) { tracing::warn!("Failed to persist profiles: {e}"); }
    Ok(())
}
