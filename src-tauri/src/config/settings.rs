//! Application settings — system-level preferences.
//!
//! Persisted as `~/.ironlink/settings.json`.
//! Appearance (theme, font, text-size) lives in frontend localStorage;
//! this module holds settings that affect runtime behavior.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

fn default_true() -> bool { true }
fn default_port() -> u16 { 15723 }
fn default_language() -> String { "zh".to_string() }

/// IronLink system settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    // ── Proxy ──
    #[serde(default = "default_port")]
    pub proxy_port: u16,

    // ── Startup / Window ──
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub minimize_to_tray_on_close: bool,
    #[serde(default)]
    pub start_minimized: bool,

    // ── Config injection ──
    #[serde(default = "default_true")]
    pub config_injection_enabled: bool,

    // ── Language (backup, frontend localStorage is source of truth) ──
    #[serde(default = "default_language")]
    pub language: String,

    // ponytail: future expansion slots — no schema migration needed later
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_settings_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unified_session: Option<bool>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            proxy_port: 15723,
            auto_start: false,
            minimize_to_tray_on_close: false,
            start_minimized: false,
            config_injection_enabled: true,
            language: "zh".to_string(),
            skill_settings_enabled: None,
            unified_session: None,
        }
    }
}

impl AppSettings {
    fn path() -> PathBuf {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".ironlink").join("settings.json")
    }

    /// Load from disk, migrating from legacy single-bool format if needed.
    pub fn load() -> Self {
        let path = Self::path();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        // Try new format first
        if let Ok(s) = serde_json::from_str::<AppSettings>(&content) {
            return s;
        }

        // Try legacy format: just a bool (auto_start value)
        if let Ok(val) = serde_json::from_str::<bool>(&content) {
            let mut s = Self::default();
            s.auto_start = val;
            // Migrate to new format
            let _ = s.save();
            return s;
        }

        tracing::warn!("Failed to parse settings.json, using defaults");
        Self::default()
    }

    /// Persist to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        tracing::info!("settings.json saved");
        Ok(())
    }
}
