// ── Model listing commands ──

use std::sync::Arc;
use tauri::State;

use crate::config::AppState;
use crate::models::*;

#[tauri::command]
/// Tauri command — return the managed models list.
pub async fn get_models(state: State<'_, Arc<AppState>>) -> Result<Vec<ModelEntry>, String> {
    Ok(state.models.lock().await.clone())
}

#[tauri::command]
/// Tauri command — replace the managed models list.
pub async fn update_models(state: State<'_, Arc<AppState>>, models: Vec<ModelEntry>) -> Result<(), String> {
    *state.models.lock().await = models;
    Ok(())
}

#[tauri::command]
/// Tauri command — fetch available models from an upstream API.
pub async fn fetch_upstream_models(url: String, api_key: String) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder().no_proxy().build().map_err(|e| e.to_string())?;
    let mut req = client.get(&url).header("Content-Type", "application/json");
    if !api_key.is_empty() { req = req.header("Authorization", format!("Bearer {}", api_key)); }
    let resp = req.send().await.map_err(|e| format!("Network: {e}"))?;
    let text = resp.text().await.map_err(|e| format!("Read: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("Parse: {e}"))?;
    let models = json["data"].as_array()
        .map(|arr| arr.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
        .unwrap_or_default();
    Ok(models)
}
