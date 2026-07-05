// ── Config file commands ──

use std::sync::Arc;
use tauri::State;

use crate::config::{self, AppState};

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

#[tauri::command]
/// Read content of any file by absolute path. Returns empty string if file doesn't exist.
pub async fn read_file_content(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
/// Read the ironlink-model-catalog.json content as a string.
pub async fn get_model_catalog(_state: State<'_, Arc<AppState>>) -> Result<String, String> {
    // Use the same path that write_app_proxy_config uses
    let path = crate::config::model_catalog_path();
    if !path.exists() {
        // Fallback to old path
        let old_path = crate::config::ironlink_dir().join("ironlink-model-catalog.json");
        if old_path.exists() {
            return std::fs::read_to_string(&old_path).map_err(|e| format!("Failed to read model catalog: {}", e));
        }
        return Ok(String::new());
    }
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read model catalog: {}", e))
}

#[tauri::command]
/// Regenerate model catalog from current enabled providers.
pub async fn regenerate_model_catalog(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let profiles = state.relay_profiles.lock().await.clone();
    let apps = state.apps.lock().await.clone();
    let path = crate::config::model_catalog_path();
    let models = state.models.lock().await.clone();

    // If there's a codex desktop app with model replacement enabled, generate mapped catalog
    let codex_app = apps.iter().find(|a| a.id == "codex-desktop");
    if let Some(app) = codex_app {
        if app.model_replacement_enabled && !app.model_mappings.is_empty() {
            crate::config::write_mapped_model_catalog(&path, app, &profiles, &models).map_err(|e| e.to_string())
        } else {
            crate::config::write_ironlink_model_catalog(&path, &profiles, &models).map_err(|e| e.to_string())
        }
    } else {
        crate::config::write_ironlink_model_catalog(&path, &profiles, &models).map_err(|e| e.to_string())
    }
}

#[tauri::command]
/// Preview what an app's config would look like after injection, without writing files.
pub async fn preview_app_config(
    state: State<'_, Arc<AppState>>,
    app_id: String,
) -> Result<String, String> {
    let apps = state.apps.lock().await.clone();
    let profiles = state.relay_profiles.lock().await.clone();
    let pcfg = state.proxy_config.lock().await.clone();

    let app = apps.iter().find(|a| a.id == app_id)
        .ok_or_else(|| format!("App '{}' not found", app_id))?;

    let inj = app.config_injection.as_ref()
        .ok_or_else(|| format!("App '{}' has no injection config", app_id))?;

    let original = std::fs::read_to_string(&inj.config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    Ok(crate::config::preview_app_config(
        &original,
        &app.default_model,
        &pcfg.reasoning_effort,
        &profiles,
        inj,
        &app.config_snippet,
        app,
    ))
}


#[derive(serde::Serialize)]
/// A config file entry with name, path, and content.
pub struct ConfigFileEntry {
    pub name: String,
    pub path: String,
    pub content: String,
}

#[tauri::command]
/// List all relevant config files for a given app (main config, model catalog, backup, etc.)
pub async fn get_app_config_files(
    state: State<'_, Arc<AppState>>,
    app_id: String,
) -> Result<Vec<ConfigFileEntry>, String> {
    let apps = state.apps.lock().await.clone();
    let app = apps.iter().find(|a| a.id == app_id)
        .ok_or_else(|| format!("App '{}' not found", app_id))?;

    let mut files: Vec<ConfigFileEntry> = Vec::new();

    // 1. Main config file (if injection configured)
    if let Some(inj) = &app.config_injection {
        let path = &inj.config_path;
        let content = std::fs::read_to_string(path).unwrap_or_default();
        files.push(ConfigFileEntry {
            name: format!("{} (main)", inj.config_type),
            path: path.clone(),
            content,
        });

        // 2. Backup file (if exists) — use the same path as app_config_bak_path
        let bak_path = crate::config::app_config_bak_path(inj);
        if bak_path.exists() {
            let bak_content = std::fs::read_to_string(&bak_path).unwrap_or_default();
            files.push(ConfigFileEntry {
                name: "Backup".into(),
                path: bak_path.to_string_lossy().into_owned(),
                content: bak_content,
            });
        }
    }

    // 3. IronLink model catalog — use the same path as write_app_proxy_config
    let catalog_path = crate::config::model_catalog_path();
    if catalog_path.exists() {
        let content = std::fs::read_to_string(&catalog_path).unwrap_or_default();
        files.push(ConfigFileEntry {
            name: "Model Catalog".into(),
            path: catalog_path.to_string_lossy().into_owned(),
            content,
        });
        // Also show the old path catalog if it exists and is different
        let old_catalog = crate::config::ironlink_dir().join("ironlink-model-catalog.json");
        if old_catalog.exists() && old_catalog != catalog_path {
            let old_content = std::fs::read_to_string(&old_catalog).unwrap_or_default();
            if !old_content.is_empty() {
                files.push(ConfigFileEntry {
                    name: "Model Catalog (old)".into(),
                    path: old_catalog.to_string_lossy().into_owned(),
                    content: old_content,
                });
            }
        }
    }

    // 4. IronLink apps config
    let apps_path = crate::config::ironlink_dir().join("apps.json");
    if apps_path.exists() {
        let content = std::fs::read_to_string(&apps_path).unwrap_or_default();
        files.push(ConfigFileEntry {
            name: "IronLink Apps".into(),
            path: apps_path.to_string_lossy().into_owned(),
            content,
        });
    }

    // 5. IronLink relay profiles
    let profiles_path = crate::config::ironlink_dir().join("relay_profiles.json");
    if profiles_path.exists() {
        let content = std::fs::read_to_string(&profiles_path).unwrap_or_default();
        files.push(ConfigFileEntry {
            name: "IronLink Providers".into(),
            path: profiles_path.to_string_lossy().into_owned(),
            content,
        });
    }

    Ok(files)
}
