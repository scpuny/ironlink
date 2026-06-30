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
                id: "deepseek-chat".into(),
                object: "model".into(),
                created: chrono::Utc::now().timestamp(),
                owned_by: "custom".into(),
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
pub fn toggle_proxy(enable: bool, default_model: &str, reasoning_effort: &str) -> bool {
    if enable {
        // Backup originals first
        if let Err(e) = backup_codex_configs() {
            warn!("Failed to backup Codex configs: {e}");
            return false;
        }
        // Modify config.toml: set base_url to proxy port
        let content = read_codex_config();
        let proxy_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);

        if !content.contains(&proxy_url) {
            if let Err(e) = write_proxy_config(&content, default_model, reasoning_effort) {
                warn!("Failed to write proxy config: {e}");
                return false;
            }
            info!("Proxy enabled — config.toml written with all required sections");
        }

        // Merge IronLink's auth into Codex auth.json (preserve other fields)
        let ironlink_auth = read_auth();
        if !ironlink_auth.is_empty() {
            let codex_auth_str = read_codex_auth();
            let merged = if let Ok(mut codex_val) = serde_json::from_str::<serde_json::Value>(&codex_auth_str) {
                if let Ok(ironlink_val) = serde_json::from_str::<serde_json::Value>(&ironlink_auth) {
                    if let Some(obj) = codex_val.as_object_mut() {
                        if let Some(ironlink_obj) = ironlink_val.as_object() {
                            for (k, v) in ironlink_obj {
                                obj.insert(k.clone(), v.clone());
                            }
                        }
                    }
                }
                serde_json::to_string_pretty(&codex_val).unwrap_or(ironlink_auth)
            } else {
                ironlink_auth
            };
            if let Err(e) = write_codex_auth(&merged) {
                warn!("Failed to apply IronLink auth to Codex: {e}");
            } else {
                info!("IronLink auth merged into ~/.codex/auth.json");
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

/// Add a log entry to the in-memory ring buffer (keeps last 500)
pub fn push_log(state: &AppState, msg: String) {
    let mut buf = state.log_buffer.blocking_lock();
    buf.push(msg);
    if buf.len() > 500 {
        buf.remove(0);
    }
}

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

/// Modify only the proxy-related fields in Codex config.toml, preserving everything else.
/// Parse original → update specific keys → serialize back.
fn write_proxy_config(original: &str, default_model: &str, reasoning_effort: &str) -> anyhow::Result<()> {
    let proxy_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);

    // Parse original TOML, or start empty
    let mut root: toml::Value = original.parse::<toml::Value>().unwrap_or(toml::Value::Table(toml::value::Table::new()));

    let table = root.as_table_mut().unwrap();

    // Preserve user's model from original
    let user_model = original.lines()
        .find(|l| l.trim().starts_with("model =") && !l.trim().starts_with("model_"))
        .and_then(|l| {
            let v = l.trim();
            v.find('"').and_then(|start| {
                v[start+1..].find('"').map(|end| &v[start+1..start+1+end])
            })
        })
        .unwrap_or("deepseek-v4-flash-free")
        .to_string();

    // Preserve sandbox_mode from original
    let sandbox_mode = original.lines()
        .find(|l| l.trim().starts_with("sandbox_mode"))
        .and_then(|l| l.trim().split('=').nth(1).map(|s| s.trim().trim_matches('"').to_string()))
        .unwrap_or_else(|| "danger-full-access".to_string());

    // Modify only the fields we need — leave everything else intact
    // Use user's original model if available, otherwise fall back to default_model param
    let effective_model = if user_model.is_empty() || user_model == "deepseek-v4-flash-free" {
        default_model
    } else {
        &user_model
    };
    table.insert("model".into(), toml::Value::String(effective_model.to_string()));
    table.insert("model_provider".into(), toml::Value::String("custom".into()));
    table.insert("model_reasoning_effort".into(), toml::Value::String(reasoning_effort.to_string()));
    table.insert("sandbox_mode".into(), toml::Value::String(sandbox_mode.clone()));

    // Set/overwrite [model_providers.custom]
    let custom = toml::Value::Table({
        let mut m = toml::value::Table::new();
        m.insert("name".into(), toml::Value::String("IronLink Proxy".into()));
        m.insert("wire_api".into(), toml::Value::String("responses".into()));
        m.insert("requires_openai_auth".into(), toml::Value::Boolean(true));
        m.insert("base_url".into(), toml::Value::String(proxy_url.clone()));
        m
    });
    let mut mp = match table.remove("model_providers") {
        Some(toml::Value::Table(t)) => t,
        _ => toml::value::Table::new(),
    };
    mp.insert("custom".into(), custom);
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
