// ── Status + proxy config commands ──

use std::sync::Arc;
use tauri::State;

use crate::config::{self, AppState, PROXY_PORT};
use crate::models::*;

#[tauri::command]
/// Tauri command — return proxy status to the frontend.
pub async fn get_status(state: State<'_, Arc<AppState>>) -> Result<ProxyStatus, String> {
    let enabled = *state.proxy_enabled.lock().await;
    let backend = state.backend.lock().await.clone();
    Ok(ProxyStatus { enabled, backend: backend.backend_type.to_string(), api_base: backend.api_base, port: crate::config::PROXY_PORT })
}

#[tauri::command]
/// Tauri command — return recent proxy logs.
pub async fn get_logs(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    Ok(state.log_buffer.lock().await.clone())
}

#[tauri::command]
/// Tauri command — return proxy-level configuration.
pub async fn get_proxy_config(state: State<'_, Arc<AppState>>) -> Result<ProxyConfig, String> {
    Ok(state.proxy_config.lock().await.clone())
}

#[tauri::command]
/// Tauri command — update proxy-level configuration.
pub async fn set_proxy_config(state: State<'_, Arc<AppState>>, config: ProxyConfig) -> Result<(), String> {
    *state.proxy_config.lock().await = config;
    Ok(())
}
