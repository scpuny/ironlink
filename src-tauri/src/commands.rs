use std::sync::Arc;
use tauri::State;

use crate::config::{self, AppState, PROXY_PORT};
use crate::models::*;

// ── Status ──

#[tauri::command]
pub async fn get_status(state: State<'_, Arc<AppState>>) -> Result<ProxyStatus, String> {
    let enabled = *state.proxy_enabled.lock().await;
    let backend = state.backend.lock().await.clone();
    Ok(ProxyStatus {
        enabled,
        backend: backend.backend_type.to_string(),
        api_base: backend.api_base,
        port: PROXY_PORT,
    })
}

// ── Backend Config ──

#[tauri::command]
pub async fn get_backend(state: State<'_, Arc<AppState>>) -> Result<BackendConfig, String> {
    Ok(state.backend.lock().await.clone())
}

#[tauri::command]
pub async fn update_backend(state: State<'_, Arc<AppState>>, backend: BackendConfig) -> Result<(), String> {
    *state.backend.lock().await = backend;
    Ok(())
}

// ── Models ──

#[tauri::command]
pub async fn get_models(state: State<'_, Arc<AppState>>) -> Result<Vec<ModelEntry>, String> {
    Ok(state.models.lock().await.clone())
}

#[tauri::command]
pub async fn update_models(state: State<'_, Arc<AppState>>, models: Vec<ModelEntry>) -> Result<(), String> {
    *state.models.lock().await = models;
    Ok(())
}

// ── Proxy Toggle ──

#[tauri::command]
pub async fn toggle_proxy(state: State<'_, Arc<AppState>>, enabled: bool) -> Result<bool, String> {
    let result = config::toggle_proxy(enabled);
    *state.proxy_enabled.lock().await = enabled;
    Ok(result)
}

// ── Config File ──

#[tauri::command]
pub async fn get_config_file() -> String {
    config::read_raw()
}

#[tauri::command]
pub async fn write_config_file(content: String) -> Result<(), String> {
    config::write_raw(&content).map_err(|e| e.to_string())
}

// ── Logs ──

#[tauri::command]
pub async fn get_logs() -> Vec<String> {
    vec!["Proxy server started.".into()]
}

// ── Auth File ──

#[tauri::command]
pub async fn get_auth_file() -> String {
    config::read_auth()
}

#[tauri::command]
pub async fn write_auth_file(content: String) -> Result<(), String> {
    config::write_auth(&content).map_err(|e| e.to_string())
}

// ── Auto-start ──

#[tauri::command]
pub async fn get_auto_start() -> bool {
    config::read_auto_start()
}

#[tauri::command]
pub async fn set_auto_start(enabled: bool) -> Result<(), String> {
    config::write_auto_start(enabled).map_err(|e| e.to_string())
}

// ── Codex Config Files ──

#[tauri::command]
pub async fn get_codex_config_file() -> String {
    config::read_codex_config()
}

#[tauri::command]
pub async fn get_codex_auth_file() -> String {
    config::read_codex_auth()
}

#[tauri::command]
pub async fn write_codex_auth_file(content: String) -> Result<(), String> {
    config::write_codex_auth(&content).map_err(|e| e.to_string())
}

// ── Relay Profiles ──


#[tauri::command]
pub async fn get_profiles(state: State<'_, Arc<AppState>>) -> Result<Vec<RelayProfile>, String> {
    Ok(state.relay_profiles.lock().await.clone())
}

#[tauri::command]
pub async fn save_profiles(state: State<'_, Arc<AppState>>, profiles: Vec<RelayProfile>) -> Result<(), String> {
    *state.relay_profiles.lock().await = profiles.clone();
    if let Err(e) = config::write_profiles(&state.relay_profiles.lock().await) {
        tracing::warn!("Failed to persist profiles: {e}");
    }
    // Sync active profile to legacy backend config
    let profiles_locked = state.relay_profiles.lock().await;
    if let Some(active) = profiles_locked.iter().find(|p| p.active) {
        let backend = BackendConfig {
            backend_type: match active.protocol.as_str() {
                "responses" => BackendType::OpenaiResponses,
                _ => BackendType::OpenaiChat,
            },
            api_base: active.base_url.clone(),
            api_key: active.api_key.clone(),
            name: Some(active.name.clone()),
            model: if active.model.is_empty() { None } else { Some(active.model.clone()) },
            test_model: if active.test_model.is_empty() { None } else { Some(active.test_model.clone()) },
            auth_type: Some("bearer".into()),
            custom_headers: None,
            config_contents: None,
            user_agent: None,
        };
        drop(profiles_locked);
        *state.backend.lock().await = backend;
    }
    Ok(())
}

#[tauri::command]
pub async fn activate_profile(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let mut profiles = state.relay_profiles.lock().await;
    for p in profiles.iter_mut() {
        p.active = p.id == id;
    }
    if let Err(e) = config::write_profiles(&profiles) {
        tracing::warn!("Failed to persist profiles: {e}");
    }
    drop(profiles);
    // Sync to backend after activation
    let profiles_again = state.relay_profiles.lock().await;
    if let Some(active) = profiles_again.iter().find(|p| p.active) {
        let backend = BackendConfig {
            backend_type: match active.protocol.as_str() {
                "responses" => BackendType::OpenaiResponses,
                _ => BackendType::OpenaiChat,
            },
            api_base: active.base_url.clone(),
            api_key: active.api_key.clone(),
            name: Some(active.name.clone()),
            model: if active.model.is_empty() { None } else { Some(active.model.clone()) },
            test_model: if active.test_model.is_empty() { None } else { Some(active.test_model.clone()) },
            auth_type: Some("bearer".into()),
            custom_headers: None,
            config_contents: None,
            user_agent: None,
        };
        drop(profiles_again);
        *state.backend.lock().await = backend;
    }
    Ok(())
}
