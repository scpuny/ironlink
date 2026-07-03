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

#[tauri::command]
/// Test a provider connection by sending a short chat completion request from the backend.
/// Returns the response content on success, or an error message on failure.
pub async fn test_provider_connection(base_url: String, api_key: String, model: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| format!("Client error: {e}"))?;

    let url = base_url.trim_end_matches('/').to_string() + "/chat/completions";
    let url = url.replace("/v1/v1/", "/v1/").replace("//chat", "/chat");

    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 10,
    });

    let req = client.post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(serde_json::to_string(&body).unwrap());

    let resp = req.send().await.map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("Read error: {e}"))?;

    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status.as_u16(), text.chars().take(200).collect::<String>()));
    }

    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))?;
    let reply = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(no content)")
        .to_string();

    Ok(reply.chars().take(200).collect())
}
