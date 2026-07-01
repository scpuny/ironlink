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
    *state.relay_profiles.lock().await = profiles.clone();
    if let Err(e) = config::profiles::write(&state.relay_profiles.lock().await) {
        tracing::warn!("Failed to persist profiles: {e}");
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
