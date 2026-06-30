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

/// Persist proxy enabled state to disk so we can restore on next startup.
fn write_proxy_enabled_state(enabled: bool) -> anyhow::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Read existing settings, merge in proxy_enabled
    let existing = std::fs::read_to_string(&path).unwrap_or_else(|_| "{}".into());
    let mut obj: serde_json::Value = serde_json::from_str(&existing).unwrap_or(serde_json::json!({}));
    if let Some(map) = obj.as_object_mut() {
        map.insert("proxy_enabled".into(), serde_json::Value::Bool(enabled));
    }
    std::fs::write(&path, serde_json::to_string_pretty(&obj)?)?;
    Ok(())
}

/// On startup: if proxy is disabled but Codex config still has proxy URL,
/// auto-restore the original configs from backup (if backup exists).
/// This prevents stale proxy URLs from breaking Codex when the app was
/// closed without first stopping the proxy.
pub fn auto_restore_codex_configs_if_needed() {
    let proxy_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);
    let codex_config = read_codex_config();
    let bak_path = codex_config_bak_path();

    // If current config has proxy URL AND backup exists, restore original
    if codex_config.contains(&proxy_url) && bak_path.exists() {
        info!("Startup: Codex config has stale proxy URL, restoring from backup");
        if let Err(e) = restore_codex_configs() {
            warn!("Startup: failed to restore Codex configs from backup: {e}");
        }
    }
}


/// Shared application state.
pub struct AppState {
    pub proxy_enabled: Mutex<bool>,
    pub backend: Mutex<BackendConfig>,
    pub models: Mutex<Vec<ModelEntry>>,
    pub relay_profiles: Mutex<Vec<RelayProfile>>,
    pub active_relay_id: Mutex<String>,
    pub proxy_config: Mutex<ProxyConfig>,
    pub log_buffer: Mutex<Vec<String>>,
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
                model_list: Vec::new(),
                enabled: true,
                active: true,
            }]
        } else {
            profiles
        };
        let active_id = profiles.iter().find(|p| p.active).map(|p| p.id.clone()).unwrap_or_else(|| profiles[0].id.clone());

        // Auto-restore Codex config if proxy was left running when app last closed
        auto_restore_codex_configs_if_needed();

        // Persist proxy_enabled state: save that we're starting disabled
        if let Err(e) = write_proxy_enabled_state(false) {
            warn!("Failed to persist proxy_enabled state: {e}");
        }

        Arc::new(Self {
            proxy_enabled: Mutex::new(false),  // Start disabled; user must click start
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
                id: "deepseek/deepseek-v4-flash".into(),
                object: "model".into(),
                created: chrono::Utc::now().timestamp(),
                owned_by: "ironlink".into(),
            }]),
            relay_profiles: Mutex::new(profiles),
            active_relay_id: Mutex::new(active_id),
            proxy_config: Mutex::new(ProxyConfig::default()),
            log_buffer: Mutex::new(Vec::new()),
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

/// Backup Codex config before modifying
fn backup_codex_configs() -> anyhow::Result<()> {
    let src = codex_config_path();
    let dst = codex_config_bak_path();
    if src.exists() {
        std::fs::copy(&src, &dst)?;
        info!("Backed up {} -> {}", src.display(), dst.display());
    }
    Ok(())
}

/// Restore Codex config from backup
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
    Ok(())
}

/// Enable/disable proxy. Always writes config.toml and auth.json on enable,
/// and always restores originals on disable — ensuring settings are always up-to-date.
pub fn toggle_proxy(enable: bool, default_model: &str, reasoning_effort: &str) -> bool {
    if enable {
        let proxy_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);
        let content = read_codex_config();

        // Backup originals, but only if the backup doesn't already contain original config.
        // If proxy URL is already in config, the backup already has the originals — skip.
        if !content.contains(&proxy_url) {
            if let Err(e) = backup_codex_configs() {
                warn!("Failed to backup Codex configs: {e}");
                return false;
            }
        }

        // Always write config.toml — ensures latest default_model is applied
        if let Err(e) = write_proxy_config(&content, default_model, reasoning_effort) {
            warn!("Failed to write proxy config: {e}");
            return false;
        }
        info!("Proxy enabled — config.toml written");

        // Persist proxy_enabled = true
        if let Err(e) = write_proxy_enabled_state(true) {
            warn!("Failed to persist proxy_enabled state: {e}");
        }

        true
    } else {
        // Restore from backup
        match restore_codex_configs() {
            Ok(_) => {
                info!("Proxy disabled — configs restored from backup");
                // Persist proxy_enabled = false
                if let Err(e) = write_proxy_enabled_state(false) {
                    warn!("Failed to persist proxy_enabled state: {e}");
                }
                true
            }
            Err(e) => {
                warn!("Failed to restore Codex configs: {e}");
                false
            }
        }
    }
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

/// Add a log entry to the in-memory ring buffer (keeps last 500)
pub async fn push_log(state: &AppState, msg: String) {
    let mut buf = state.log_buffer.lock().await;
    buf.push(msg);
    if buf.len() > 500 {
        buf.remove(0);
    }
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

/// Read model list from the active relay profile on disk.
fn read_active_profile_models() -> Vec<String> {
    let profiles = read_profiles();
    let active = profiles.iter().find(|p| p.active).or_else(|| profiles.first());
    match active {
        Some(p) => {
            let mut models: Vec<String> = p.model_list.clone();
            if !p.model.is_empty() && !models.contains(&p.model) {
                models.insert(0, p.model.clone());
            }
            models
        }
        None => vec![],
    }
}

/// Modify only the proxy-related fields in Codex config.toml, preserving everything else.
/// Parse original → update specific keys → serialize back.
fn write_proxy_config(original: &str, default_model: &str, reasoning_effort: &str) -> anyhow::Result<()> {
    let proxy_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);

    // Extract existing models from config BEFORE mutating
    let existing_models: Vec<String> = original
        .parse::<toml::Value>()
        .ok()
        .and_then(|v| {
            v.get("model_providers")
                .and_then(|mp| mp.get("ironlink"))
                .and_then(|c| c.get("models"))
                .and_then(|m| m.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        })
        .unwrap_or_default();

    // Parse original TOML, or start empty
    let mut root: toml::Value = original.parse::<toml::Value>().unwrap_or(toml::Value::Table(toml::value::Table::new()));
    let table = root.as_table_mut().unwrap();

    // Write the model as provided from proxy_config (with provider prefix).
    // Codex sends this model name to the proxy, and find_profile() uses the
    // prefix for explicit provider routing (e.g. "deepseek/deepseek-v4-flash").
    table.insert("model".into(), toml::Value::String(default_model.to_string()));

    // Preserve sandbox_mode from original config
    let sandbox_mode = original.lines()
        .find(|l| l.trim().starts_with("sandbox_mode"))
        .and_then(|l| l.trim().split('=').nth(1).map(|s| s.trim().trim_matches('"').to_string()))
        .unwrap_or_else(|| "danger-full-access".to_string());
    table.insert("model_provider".into(), toml::Value::String("ironlink".into()));
    table.insert("model_reasoning_effort".into(), toml::Value::String(reasoning_effort.to_string()));
    table.insert("sandbox_mode".into(), toml::Value::String(sandbox_mode.clone()));

    // Remove stale shell_environment_policy (written by old ccswitch tool)
    table.remove("shell_environment_policy");

    // Set/overwrite [model_providers.ironlink] with models list from active relay profile.
    // If no models from relay profile, try to preserve existing models from current config.
    let mut active_models = read_active_profile_models();

    // Fallback: if relay profile has no models, try to preserve existing models from config
    if active_models.is_empty() {
        if !existing_models.is_empty() {
            active_models = existing_models;
            tracing::info!("Falling back to existing models from config: {:?}", active_models);
        }
    }

    let ironlink = toml::Value::Table({
        let mut m = toml::value::Table::new();
        m.insert("name".into(), toml::Value::String("ironlink".into()));
        m.insert("wire_api".into(), toml::Value::String("responses".into()));
        m.insert("requires_openai_auth".into(), toml::Value::Boolean(false));
        // m.insert("env_key".into(), toml::Value::String("OPENAI_API_KEY".into()));
        m.insert("base_url".into(), toml::Value::String(proxy_url.clone()));
        m
    });
    let mut mp = match table.remove("model_providers") {
        Some(toml::Value::Table(t)) => t,
        _ => toml::value::Table::new(),
    };
    mp.insert("ironlink".into(), ironlink);
    table.insert("model_providers".into(), toml::Value::Table(mp));

    // Ensure [marketplaces.openai-bundled] has source_type = "local" but keep other keys
    let mut mkts = match table.remove("marketplaces") {
        Some(toml::Value::Table(t)) => t,
        _ => toml::value::Table::new(),
    };
    let bundled = match mkts.remove("openai-bundled") {
        Some(toml::Value::Table(t)) => t,
        _ => toml::value::Table::new(),
    };
    let mut updated = bundled.clone();
    updated.insert("source_type".into(), toml::Value::String("local".into()));
    // Preserve other bundled keys like last_updated, source
    for (k, v) in bundled {
        if k != "source_type" {
            updated.entry(k).or_insert(v);
        }
    }
    mkts.insert("openai-bundled".into(), toml::Value::Table(updated));
    table.insert("marketplaces".into(), toml::Value::Table(mkts));

    let config = toml::to_string_pretty(&root)?;
    let path = codex_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &config)?;
    info!("proxy config.toml written — modified only proxy fields, preserved all original sections");
    Ok(())
}
