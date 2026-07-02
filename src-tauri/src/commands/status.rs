// ── Status + proxy config commands ──

use std::sync::Arc;
use tauri::State;

use crate::config::AppState;
use crate::models::*;

#[tauri::command]
/// Tauri command — return proxy status to the frontend.
pub async fn get_status(state: State<'_, Arc<AppState>>) -> Result<ProxyStatus, String> {
    let enabled = *state.proxy_enabled.lock().await;
    let profiles = state.relay_profiles.lock().await.clone();
    // Derive display info from the first enabled provider
    let (backend, api_base) = match profiles.iter().find(|p| p.enabled) {
        Some(p) => (p.protocol.clone(), p.base_url.clone()),
        None => ("none".to_string(), String::new()),
    };
    Ok(ProxyStatus { enabled, backend, api_base, port: crate::config::proxy_port() })
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
