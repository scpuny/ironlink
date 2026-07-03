//! Application config persistence.
//!
//! Stores downstream app definitions in ~/.ironlink/apps.json.
//! Each app bundles its protocol, models, mappings, and config injection info.

use std::path::PathBuf;
use crate::models::{AppConfig, AppInjection};

fn apps_path() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("apps.json")
}

/// Read apps from disk, or return defaults.
pub fn read() -> Vec<AppConfig> {
    let path = apps_path();
    match std::fs::read_to_string(&path) {
        Ok(c) => serde_json::from_str(&c).unwrap_or_else(|_| default_apps()),
        Err(_) => default_apps(),
    }
}

/// Persist apps to disk.
pub fn write(apps: &[AppConfig]) -> anyhow::Result<()> {
    let path = apps_path();
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&path, serde_json::to_string_pretty(apps)?)?;
    tracing::info!("apps.json written ({} apps)", apps.len());
    Ok(())
}

/// Default app list — Codex Desktop (enabled) + Claude Desktop (disabled).
fn default_apps() -> Vec<AppConfig> {
    vec![
        AppConfig {
            id: "codex-desktop".into(),
            name: "Codex Desktop".into(),
            protocol: "responses".into(),
            enabled: true,
            default_model: "gpt-5.5".into(),
            models: vec![
                "gpt-5.5".into(), "gpt-5.4".into(), "gpt-5.4-mini".into(),
                "gpt-5.3-codex".into(), "gpt-5.2".into(),
            ],
            config_injection: Some(AppInjection {
                config_type: "codex_toml".into(),
                config_path: {
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                    format!("{}/.codex/config.toml", home)
                },
                config_dir: None,
                backup_enabled: true,
                fields: None,
            }),
            model_mappings: std::collections::HashMap::new(),
            model_replacement_enabled: false,
            model_display_names: std::collections::HashMap::new(),
            config_snippet: None,
        },
        AppConfig {
            id: "claude-desktop".into(),
            name: "Claude Desktop".into(),
            protocol: "anthropic".into(),
            enabled: false,
            default_model: "claude-sonnet-4".into(),
            models: vec![
                "claude-sonnet-4-20250514".into(), "claude-4-opus-20250514".into(),
            ],
            config_injection: None,
            model_mappings: std::collections::HashMap::new(),
            model_replacement_enabled: false,
            model_display_names: std::collections::HashMap::new(),
            config_snippet: None,
        },
    ]
}
