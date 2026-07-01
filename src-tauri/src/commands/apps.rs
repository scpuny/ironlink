//! App commands for Tauri.

use std::sync::Arc;
use tauri::State;

use crate::config::{self, AppState};
use crate::models::AppConfig;

#[tauri::command]
/// Return all app configurations.
pub async fn get_apps(state: State<'_, Arc<AppState>>) -> Result<Vec<AppConfig>, String> {
    Ok(state.apps.lock().await.clone())
}

#[tauri::command]
/// Save app configurations.
pub async fn save_apps(state: State<'_, Arc<AppState>>, apps: Vec<AppConfig>) -> Result<(), String> {
    *state.apps.lock().await = apps.clone();
    if let Err(e) = config::apps::write(&state.apps.lock().await) {
        tracing::warn!("Failed to persist apps: {e}");
    }
    Ok(())
}
