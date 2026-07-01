// ── Config file commands ──

use crate::config;

#[tauri::command]
/// Tauri command — read the raw config file content.
pub async fn get_config_file() -> String {
    config::read_raw()
}

#[tauri::command]
/// Tauri command — write raw content to the config file.
pub async fn write_config_file(content: String) -> Result<(), String> {
    config::write_raw(&content).map_err(|e| e.to_string())
}

#[tauri::command]
/// Tauri command — read Codex's config.toml.
pub async fn get_codex_config_file() -> String {
    config::read_codex_config()
}

#[tauri::command]
/// Tauri command — check if auto-start is enabled.
pub async fn get_auto_start() -> bool {
    config::read_auto_start()
}

#[tauri::command]
/// Tauri command — enable or disable auto-start.
pub async fn set_auto_start(enabled: bool) -> Result<(), String> {
    config::write_auto_start(enabled).map_err(|e| e.to_string())
}
