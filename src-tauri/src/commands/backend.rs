// ── Backend + proxy toggle commands ──

use std::sync::Arc;
use tauri::State;

use crate::config::{self, AppState};
use crate::models::*;

#[tauri::command]
/// Tauri command — return the current backend config.
pub async fn get_backend(state: State<'_, Arc<AppState>>) -> Result<BackendConfig, String> {
    Ok(state.backend.lock().await.clone())
}

#[tauri::command]
/// Tauri command — update the backend configuration.
pub async fn update_backend(state: State<'_, Arc<AppState>>, backend: BackendConfig) -> Result<(), String> {
    *state.backend.lock().await = backend;
    Ok(())
}

#[tauri::command]
/// Tauri command — enable or disable the proxy server.
pub async fn toggle_proxy(state: State<'_, Arc<AppState>>, enabled: bool) -> Result<bool, String> {
    let pcfg = state.proxy_config.lock().await.clone();
    let result = config::toggle_proxy(enabled, &pcfg.default_model, &pcfg.reasoning_effort);
    *state.proxy_enabled.lock().await = enabled;
    Ok(result)
}
