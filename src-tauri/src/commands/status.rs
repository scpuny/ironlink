// ── Status + proxy config commands ──

use std::sync::Arc;
use tauri::{State, Manager};

use crate::config::AppState;
use crate::models::*;

#[tauri::command]
pub async fn get_status(state: State<'_, Arc<AppState>>) -> Result<ProxyStatus, String> {
    let enabled = *state.proxy_enabled.lock().await;
    let profiles = state.relay_profiles.lock().await.clone();
    let (backend, api_base) = match profiles.iter().find(|p| p.enabled) {
        Some(p) => (p.protocol.clone(), p.base_url.clone()),
        None => ("none".to_string(), String::new()),
    };
    Ok(ProxyStatus { enabled, backend, api_base, port: crate::config::proxy_port() })
}

#[tauri::command]
pub async fn get_logs(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    Ok(state.log_buffer.lock().await.clone())
}

#[tauri::command]
pub async fn get_proxy_config(state: State<'_, Arc<AppState>>) -> Result<ProxyConfig, String> {
    Ok(state.proxy_config.lock().await.clone())
}

#[tauri::command]
pub async fn set_proxy_config(state: State<'_, Arc<AppState>>, config: ProxyConfig) -> Result<(), String> {
    *state.proxy_config.lock().await = config;
    Ok(())
}

/// Result of a version check.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VersionInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
}

#[tauri::command]
pub async fn check_version() -> VersionInfo {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let release_url = "https://github.com/scpuny/ironlink/releases/latest".to_string();

    let client = reqwest::Client::builder()
        .user_agent("IronLink")
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(reqwest::header::ACCEPT, "application/vnd.github.v3+json".parse().unwrap());
            h
        })
        .build()
        .ok();

    let latest = match client {
        Some(c) => {
            match c.get("https://api.github.com/repos/scpuny/ironlink/releases/latest").send().await {
                Ok(resp) => {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        data["tag_name"].as_str()
                            .unwrap_or(&current)
                            .trim_start_matches('v')
                            .to_string()
                    } else {
                        current.clone()
                    }
                }
                Err(_) => current.clone(),
            }
        }
        None => current.clone(),
    };

    let has_update = latest != current;

    VersionInfo {
        current_version: current,
        latest_version: latest,
        has_update,
        release_url,
    }
}

#[tauri::command]
/// Quit the app (called from frontend close dialog).
pub async fn quit_app(app_handle: tauri::AppHandle) -> Result<(), String> {
    app_handle.exit(0);
    Ok(())
}

#[tauri::command]
/// Hide the main window to system tray (called from frontend close dialog).
pub async fn hide_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}
