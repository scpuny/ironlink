use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::models::*;

pub const PROXY_PORT: u16 = 15723;
pub const ORIG_PORT: u16 = 57321;

/// Path to IronLink settings file
fn settings_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("settings.json")
}

/// Read auto-start setting
pub fn read_auto_start() -> bool {
    let path = settings_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    serde_json::from_str(&content).ok().filter(|v: &bool| *v).unwrap_or(false)
}

/// Write auto-start setting
pub fn write_auto_start(enabled: bool) -> anyhow::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, if enabled { "true" } else { "false" })?;
    Ok(())
}


/// Shared application state.
pub struct AppState {
    pub proxy_enabled: Mutex<bool>,
    pub backend: Mutex<BackendConfig>,
    pub models: Mutex<Vec<ModelEntry>>,
    pub relay_profiles: Mutex<Vec<RelayProfile>>,
    pub active_relay_id: Mutex<String>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        tracing::info!("Relay profiles path: {}", profiles_path().display());
        let profiles = read_profiles();
        let profiles = if profiles.is_empty() {
            vec![RelayProfile {
                id: "default".into(),
                provider_id: "deepseek".into(),
                name: "DeepSeek".into(),
                base_url: "https://api.deepseek.com/v1".into(),
                api_key: String::new(),
                protocol: "chatCompletions".into(),
                model: "deepseek-chat".into(),
                test_model: String::new(),
                model_list: String::new(),
                enabled: true,
                active: true,
            }]
        } else {
            profiles
        };
        let active_id = profiles.iter().find(|p| p.active).map(|p| p.id.clone()).unwrap_or_else(|| profiles[0].id.clone());
        Arc::new(Self {
            proxy_enabled: Mutex::new(read_auto_start() && toggle_proxy(true)),
            backend: Mutex::new(BackendConfig {
                backend_type: BackendType::OpenaiChat,
                api_base: "https://api.deepseek.com/v1".into(),
                api_key: String::new(),
                name: Some("DeepSeek".into()),
                model: Some("deepseek-chat".into()),
                test_model: None,
                auth_type: Some("bearer".into()),
                custom_headers: None,
                config_contents: None,
                user_agent: None,
            }),
            models: Mutex::new(vec![ModelEntry {
                id: "deepseek-chat".into(),
                object: "model".into(),
                created: chrono::Utc::now().timestamp(),
                owned_by: "custom".into(),
            }]),
            relay_profiles: Mutex::new(profiles),
            active_relay_id: Mutex::new(active_id),
        })
    }
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("config.toml")
}

/// Read raw config.toml content.
pub fn read_raw() -> String {
    let path = config_path();
    std::fs::read_to_string(&path).unwrap_or_default()
}

/// Write raw content to config.toml.
pub fn write_raw(content: &str) -> anyhow::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    info!("config.toml written");
    Ok(())
}

/// Toggle base_url between proxy and original port.
fn codex_config_bak_path() -> PathBuf {
    let mut p = codex_config_path();
    p.set_file_name("config.toml.ironlink.bak");
    p
}

fn codex_auth_bak_path() -> PathBuf {
    let mut p = codex_auth_path();
    p.set_file_name("auth.json.ironlink.bak");
    p
}

/// Backup Codex config files before modifying
fn backup_codex_configs() -> anyhow::Result<()> {
    let src = codex_config_path();
    let dst = codex_config_bak_path();
    if src.exists() {
        std::fs::copy(&src, &dst)?;
        info!("Backed up {} -> {}", src.display(), dst.display());
    }
    let src_auth = codex_auth_path();
    let dst_auth = codex_auth_bak_path();
    if src_auth.exists() {
        std::fs::copy(&src_auth, &dst_auth)?;
        info!("Backed up {} -> {}", src_auth.display(), dst_auth.display());
    }
    Ok(())
}

/// Restore Codex config files from backup
fn restore_codex_configs() -> anyhow::Result<()> {
    let bak = codex_config_bak_path();
    let dst = codex_config_path();
    if bak.exists() {
        std::fs::copy(&bak, &dst)?;
        let _ = std::fs::remove_file(&bak);
        info!("Restored {} from backup", dst.display());
    } else {
        warn!("No backup found at {}, skipping config.toml restore", bak.display());
    }
    let bak_auth = codex_auth_bak_path();
    let dst_auth = codex_auth_path();
    if bak_auth.exists() {
        std::fs::copy(&bak_auth, &dst_auth)?;
        let _ = std::fs::remove_file(&bak_auth);
        info!("Restored {} from backup", dst_auth.display());
    } else {
        warn!("No backup found at {}, skipping auth.json restore", bak_auth.display());
    }
    Ok(())
}

/// Enable/disable proxy by replacing base_url in Codex config.toml
pub fn toggle_proxy(enable: bool) -> bool {
    if enable {
        // Backup originals first
        if let Err(e) = backup_codex_configs() {
            warn!("Failed to backup Codex configs: {e}");
            return false;
        }
        // Modify config.toml: set base_url to proxy port
        let mut content = read_codex_config();
        let proxy_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);

        if !content.contains(&proxy_url) {
            content = replace_base_url_value(&content, &proxy_url);
            if let Err(e) = std::fs::write(codex_config_path(), &content) {
                warn!("Failed to write proxy config: {e}");
                return false;
            }
            info!("Proxy enabled — config.toml updated to use port {}", PROXY_PORT);
        }

        // Apply IronLink's auth to Codex auth.json
        let ironlink_auth = read_auth();
        if !ironlink_auth.is_empty() {
            if let Err(e) = write_codex_auth(&ironlink_auth) {
                warn!("Failed to apply IronLink auth to Codex: {e}");
            } else {
                info!("IronLink auth applied to ~/.codex/auth.json");
            }
        }
        true
    } else {
        // Restore from backup
        match restore_codex_configs() {
            Ok(_) => {
                info!("Proxy disabled — configs restored from backup");
                true
            }
            Err(e) => {
                warn!("Failed to restore Codex configs: {e}");
                false
            }
        }
    }
}

/// Replace the base_url value in config.toml content with the proxy URL.
/// Handles both `base_url = "..."` and `api_base = "..."` formats.
fn replace_base_url_value(content: &str, new_url: &str) -> String {
    let mut result = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("base_url") || trimmed.starts_with("api_base") {
            if let Some(_eq) = line.rfind('=') {
                let indent = &line[..line.len() - trimmed.len()];
                result.push_str(&format!("{}base_url = \"{}\"\n", indent, new_url));

                continue;
            }
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

/// Path to ~/.codex/config.toml
fn codex_config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".codex").join("config.toml")
}

/// Read ~/.codex/config.toml
pub fn read_codex_config() -> String {
    std::fs::read_to_string(codex_config_path()).unwrap_or_default()
}

/// Path to ~/.codex/auth.json
pub fn codex_auth_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".codex").join("auth.json")
}

/// Read ~/.codex/auth.json
pub fn read_codex_auth() -> String {
    std::fs::read_to_string(codex_auth_path()).unwrap_or_default()
}

/// Write to ~/.codex/auth.json
pub fn write_codex_auth(content: &str) -> anyhow::Result<()> {
    let path = codex_auth_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    info!("codex auth.json written to {}", path.display());
    Ok(())
}

/// Path to ~/.codex/auth.json

fn auth_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("auth.json")
}

/// Path to ~/.codex/relay_profiles.json
fn profiles_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("relay_profiles.json")
}

/// Read relay profiles from disk.
pub fn read_profiles() -> Vec<RelayProfile> {
    let path = profiles_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::info!("No saved profiles file ({}), using defaults", e);
            return vec![];
        }
    };
    let profiles: Vec<RelayProfile> = match serde_json::from_str(&content) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to parse relay_profiles.json: {e}");
            return vec![];
        }
    };
    tracing::info!("Loaded {} relay profiles from disk", profiles.len());
    profiles
}

/// Write relay profiles to disk.
pub fn write_profiles(profiles: &[RelayProfile]) -> anyhow::Result<()> {
    let path = profiles_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(profiles)?;
    std::fs::write(&path, json)?;
    info!("relay_profiles.json written ({} profiles)", profiles.len());
    Ok(())
}

/// Read current auth.json content.
pub fn read_auth() -> String {
    let path = auth_path();
    std::fs::read_to_string(&path).unwrap_or_default()
}

/// Write content to auth.json (creates parent dirs if needed).
pub fn write_auth(content: &str) -> anyhow::Result<()> {
    let path = auth_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    info!("auth.json written to {}", path.display());
    Ok(())
}
