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
///
/// Enable:  start proxy server + write Codex config injection.
/// Disable: restore Codex config + stop proxy server.
pub async fn toggle_proxy(state: State<'_, Arc<AppState>>, enabled: bool) -> Result<bool, String> {
    let s = state.inner().clone();
    if enabled {
        // 1. Config injection (write Codex config to point to IronLink)
        let pcfg = state.proxy_config.lock().await.clone();
        let profiles = state.relay_profiles.lock().await.clone();
        let settings = state.settings.lock().await.clone();
        if settings.config_injection_enabled {
            let apps = state.apps.lock().await.clone();
            config::toggle_proxy(true, &pcfg.default_model, &pcfg.reasoning_effort, &profiles, &apps);
        }

        // 2. Start proxy server in background
        tauri::async_runtime::spawn(async move {
            crate::start_proxy_server(s).await;
        });

        // 3. Mark enabled after server starts (small delay for binding)
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        *state.proxy_enabled.lock().await = true;
        Ok(true)
    } else {
        // 1. Restore Codex original config
        let apps = state.apps.lock().await.clone();
            // Per-app restore is handled inside toggle_proxy
            config::toggle_proxy(false, "", "", &[], &apps);

        // 2. Stop proxy server
        crate::stop_proxy_server(s).await;

        // 3. Mark disabled
        *state.proxy_enabled.lock().await = false;
        Ok(true)
    }
}
