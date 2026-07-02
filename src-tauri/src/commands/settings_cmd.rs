//! Tauri commands for settings CRUD, config import/export.


use crate::config::settings;

#[tauri::command]
/// Read full system settings.
pub async fn get_settings() -> settings::AppSettings {
    settings::AppSettings::load()
}

#[tauri::command]
/// Save full system settings.
pub async fn save_settings(s: settings::AppSettings) -> Result<(), String> {
    s.save().map_err(|e| e.to_string())
}

#[tauri::command]
/// Export all IronLink configuration as a JSON blob.
/// Includes settings, apps, relay_profiles, and model catalog.
pub async fn export_config() -> Result<String, String> {
    use std::collections::HashMap;

    let settings = settings::AppSettings::load();
    let apps = crate::config::apps::read();
    let profiles = crate::config::profiles::read();

    let mut map = HashMap::new();
    map.insert("version".to_string(), serde_json::json!("1.0"));
    map.insert("settings".to_string(), serde_json::to_value(&settings).unwrap_or_default());
    map.insert("apps".to_string(), serde_json::to_value(&apps).unwrap_or_default());
    map.insert("relay_profiles".to_string(), serde_json::to_value(&profiles).unwrap_or_default());

    serde_json::to_string_pretty(&map).map_err(|e| e.to_string())
}

#[tauri::command]
/// Import configuration from a JSON blob (inverse of export_config).
/// Merges settings, apps, and relay_profiles.
pub async fn import_config(json: String) -> Result<String, String> {
    let val: serde_json::Value = serde_json::from_str(&json).map_err(|e| format!("Invalid JSON: {e}"))?;

    let mut imported = Vec::new();

    if let Some(s) = val.get("settings").and_then(|v| serde_json::from_value::<settings::AppSettings>(v.clone()).ok()) {
        s.save().map_err(|e| format!("Failed to save settings: {e}"))?;
        imported.push("settings");
    }

    if let Some(apps) = val.get("apps").and_then(|v| serde_json::from_value::<Vec<crate::models::AppConfig>>(v.clone()).ok()) {
        crate::config::apps::write(&apps).map_err(|e| format!("Failed to save apps: {e}"))?;
        imported.push("apps");
    }

    if let Some(profiles) = val.get("relay_profiles").and_then(|v| serde_json::from_value::<Vec<crate::models::RelayProfile>>(v.clone()).ok()) {
        crate::config::profiles::write(&profiles).map_err(|e| format!("Failed to save profiles: {e}"))?;
        imported.push("relay_profiles");
    }

    if imported.is_empty() {
        return Err("No recognized sections found in the import file".to_string());
    }

    Ok(format!("Imported: {}", imported.join(", ")))
}
