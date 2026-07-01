//! Application config persistence.
//!
//! Stores downstream app definitions (Codex Desktop, Claude Desktop, etc.)
//! in ~/.ironlink/apps.json, each with its own model mappings.

use std::path::PathBuf;
use crate::models::AppConfig;

fn apps_path() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("apps.json")
}

/// Read apps from the JSON file on disk, or return a default Codex app.
pub fn read() -> Vec<AppConfig> {
    let path = apps_path();
    match std::fs::read_to_string(&path) {
        Ok(c) => serde_json::from_str(&c).unwrap_or_else(|_| default_apps()),
        Err(_) => default_apps(),
    }
}

/// Persist apps to the JSON file on disk.
pub fn write(apps: &[AppConfig]) -> anyhow::Result<()> {
    let path = apps_path();
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&path, serde_json::to_string_pretty(apps)?)?;
    tracing::info!("apps.json written ({} apps)", apps.len());
    Ok(())
}

/// Default app list — Codex Desktop with empty mappings.
fn default_apps() -> Vec<AppConfig> {
    vec![
        AppConfig {
            id: "codex-desktop".into(),
            name: "Codex Desktop".into(),
            protocol: "responses".into(),
            enabled: true,
            model_mappings: std::collections::HashMap::new(),
        },
        AppConfig {
            id: "claude-desktop".into(),
            name: "Claude Desktop".into(),
            protocol: "anthropic".into(),
            enabled: false,
            model_mappings: std::collections::HashMap::new(),
        },
    ]
}
