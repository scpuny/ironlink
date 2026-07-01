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
pub fn auto_restore_codex_configs_if_needed() {
    let proxy_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);
    let codex_config = read_codex_config();
    let bak_path = codex_config_bak_path();
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
    pub model_mappings: Mutex<Vec<ModelMapping>>,
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

        auto_restore_codex_configs_if_needed();
        if let Err(e) = write_proxy_enabled_state(false) {
            warn!("Failed to persist proxy_enabled state: {e}");
        }

        let mappings = read_model_mappings();

        Arc::new(Self {
            proxy_enabled: Mutex::new(false),
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
            model_mappings: Mutex::new(mappings),
        })
    }
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("config.toml")
}

pub fn read_raw() -> String {
    let path = config_path();
    std::fs::read_to_string(&path).unwrap_or_default()
}

pub fn write_raw(content: &str) -> anyhow::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    info!("config.toml written");
    Ok(())
}

fn codex_config_bak_path() -> PathBuf {
    let mut p = codex_config_path();
    p.set_file_name("config.toml.ironlink.bak");
    p
}

fn backup_codex_configs() -> anyhow::Result<()> {
    let src = codex_config_path();
    let dst = codex_config_bak_path();
    if src.exists() {
        std::fs::copy(&src, &dst)?;
        info!("Backed up {} -> {}", src.display(), dst.display());
    }
    Ok(())
}

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

pub fn toggle_proxy(enable: bool, default_model: &str, reasoning_effort: &str) -> bool {
    if enable {
        let proxy_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);
        let content = read_codex_config();
        if !content.contains(&proxy_url) {
            if let Err(e) = backup_codex_configs() {
                warn!("Failed to backup Codex configs: {e}");
                return false;
            }
        }
        if let Err(e) = write_proxy_config(&content, default_model, reasoning_effort) {
            warn!("Failed to write proxy config: {e}");
            return false;
        }
        info!("Proxy enabled — config.toml written");
        if let Err(e) = write_proxy_enabled_state(true) {
            warn!("Failed to persist proxy_enabled state: {e}");
        }
        true
    } else {
        match restore_codex_configs() {
            Ok(_) => {
                info!("Proxy disabled — configs restored from backup");
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

fn codex_config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".codex").join("config.toml")
}

pub fn read_codex_config() -> String {
    std::fs::read_to_string(codex_config_path()).unwrap_or_default()
}

pub async fn push_log(state: &AppState, msg: String) {
    let mut buf = state.log_buffer.lock().await;
    buf.push(msg);
    if buf.len() > 500 {
        buf.remove(0);
    }
}

fn profiles_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("relay_profiles.json")
}

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

// ── Model Mappings: map Codex model names → upstream model + profile ──

fn mappings_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("model_mappings.json")
}

pub fn read_model_mappings() -> Vec<ModelMapping> {
    let path = mappings_path();
    match std::fs::read_to_string(&path) {
        Ok(c) => serde_json::from_str(&c).unwrap_or_default(),
        Err(_) => {
            // Default mappings: each official Codex model → first enabled profile
            let profiles = read_profiles();
            if profiles.is_empty() { return vec![]; }
            profiles.iter().filter(|p| p.enabled).take(5).enumerate().map(|(i, p)| {
                let codex_models = ["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex", "gpt-5.2"];
                let target = if p.model_list.is_empty() { &p.model } else { &p.model_list[0] };
                ModelMapping {
                    codex_model: codex_models[i].to_string(),
                    upstream_model: format!("{}/{}", p.provider_id, target),
                    profile_id: p.id.clone(),
                }
            }).collect()
        }
    }
}

pub fn write_model_mappings(mappings: &[ModelMapping]) -> anyhow::Result<()> {
    let path = mappings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(mappings)?)?;
    info!("model_mappings.json written ({} mappings)", mappings.len());
    Ok(())
}

/// Look up the model mapping for a given Codex model name.
/// Returns the upstream model slug and profile ID if found.
pub fn resolve_mapping<'a>(mappings: &'a [ModelMapping], codex_model: &str) -> Option<&'a ModelMapping> {
    mappings.iter().find(|m| m.codex_model == codex_model)
}

fn write_proxy_config(original: &str, default_model: &str, reasoning_effort: &str) -> anyhow::Result<()> {
    let proxy_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);
    let bare_model = default_model
        .split('/')
        .last()
        .unwrap_or(default_model);

    let mut root: toml::Value = original.parse::<toml::Value>().unwrap_or(toml::Value::Table(toml::value::Table::new()));
    let table = root.as_table_mut().unwrap();

    table.insert("model".into(), toml::Value::String(bare_model.to_string()));

    let sandbox_mode = original.lines()
        .find(|l| l.trim().starts_with("sandbox_mode"))
        .and_then(|l| l.trim().split('=').nth(1).map(|s| s.trim().trim_matches('"').to_string()))
        .unwrap_or_else(|| "danger-full-access".to_string());

    // Use [model_providers.ironlink] instead of top-level openai_base_url interception.
    // This tells Codex to treat IronLink as a custom provider — all API requests go to
    // proxy.base_url, and Codex does NOT use the built-in OpenAI provider.
    table.insert("model_provider".into(), toml::Value::String("ironlink".into()));
    table.insert("model_reasoning_effort".into(), toml::Value::String(reasoning_effort.to_string()));
    table.insert("sandbox_mode".into(), toml::Value::String(sandbox_mode.clone()));

    // Remove stale top-level fields that conflict with custom provider usage
    table.remove("openai_base_url");
    table.remove("model_catalog_json");
    table.remove("shell_environment_policy");

    // Set/overwrite [model_providers.ironlink] — Codex reads this as a custom provider.
    // All requests (models, chat, responses) go to base_url.
    let ironlink = toml::Value::Table({
        let mut m = toml::value::Table::new();
        m.insert("name".into(), toml::Value::String("IronLink".into()));
        m.insert("base_url".into(), toml::Value::String(proxy_url.clone()));
        m.insert("wire_api".into(), toml::Value::String("responses".into()));
        m.insert("supports_websockets".into(), toml::Value::Boolean(false));
        m.insert("requires_openai_auth".into(), toml::Value::Boolean(false));
        m.insert("allow_insecure".into(), toml::Value::Boolean(true));
        m
    });
    let mut mp = match table.remove("model_providers") {
        Some(toml::Value::Table(t)) => t,
        _ => toml::value::Table::new(),
    };
    mp.insert("ironlink".into(), ironlink);
    table.insert("model_providers".into(), toml::Value::Table(mp));

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
    info!("proxy config.toml written");
    Ok(())
}
