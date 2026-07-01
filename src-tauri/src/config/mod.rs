//! Application configuration, Codex config patching, and shared application state.

pub mod apps;
pub mod profiles;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::models::*;

pub const PROXY_PORT: u16 = 15723;
pub const ORIG_PORT: u16 = 57321;

fn settings_path() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("settings.json")
}

pub fn read_auto_start() -> bool {
    serde_json::from_str::<bool>(&std::fs::read_to_string(&settings_path()).unwrap_or_default()).ok().unwrap_or(false)
}

pub fn write_auto_start(enabled: bool) -> anyhow::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&path, if enabled { "true" } else { "false" })?;
    Ok(())
}

fn write_proxy_enabled_state(enabled: bool) -> anyhow::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let existing = std::fs::read_to_string(&path).unwrap_or_else(|_| "{}".into());
    let mut obj: serde_json::Value = serde_json::from_str(&existing).unwrap_or(serde_json::json!({}));
    if let Some(map) = obj.as_object_mut() { map.insert("proxy_enabled".into(), serde_json::Value::Bool(enabled)); }
    std::fs::write(&path, serde_json::to_string_pretty(&obj)?)?;
    Ok(())
}

pub fn auto_restore_codex_configs_if_needed() {
    let proxy_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);
    let codex_config = read_codex_config();
    let bak_path = codex_config_bak_path();
    if codex_config.contains(&proxy_url) && bak_path.exists() {
        info!("Startup: Codex config has stale proxy URL, restoring from backup");
        restore_codex_configs();
    }
}

/// Shared application state.
pub struct AppState {
    pub proxy_enabled: Mutex<bool>,
    pub backend: Mutex<BackendConfig>,
    pub models: Mutex<Vec<ModelEntry>>,
    pub relay_profiles: Mutex<Vec<RelayProfile>>,
    pub active_relay_id: Mutex<String>,
    pub apps: Mutex<Vec<AppConfig>>,
    pub proxy_config: Mutex<ProxyConfig>,
    pub log_buffer: Mutex<Vec<String>>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        let profiles_data = profiles::read();
        let profiles = if profiles_data.is_empty() {
            vec![RelayProfile {
                id: "default".into(), provider_id: "deepseek".into(), name: "DeepSeek".into(),
                base_url: "https://api.deepseek.com/v1".into(), api_key: String::new(),
                protocol: "chatCompletions".into(), model: "deepseek-chat".into(),
                test_model: String::new(), model_list: Vec::new(), enabled: true, active: true,
            }]
        } else { profiles_data };
        let active_id = profiles.iter().find(|p| p.active).map(|p| p.id.clone()).unwrap_or_else(|| profiles[0].id.clone());

        let app_list = apps::read();

        auto_restore_codex_configs_if_needed();
        if let Err(e) = write_proxy_enabled_state(false) { warn!("Failed to persist proxy_enabled state: {e}"); }

        Arc::new(Self {
            proxy_enabled: Mutex::new(false),
            backend: Mutex::new(BackendConfig {
                backend_type: BackendType::OpenaiChat, api_base: "https://api.deepseek.com/v1".into(),
                api_key: String::new(), name: Some("DeepSeek".into()), model: Some("deepseek-chat".into()),
                test_model: None, auth_type: Some("bearer".into()), custom_headers: None,
                config_contents: None, user_agent: None,
            }),
            models: Mutex::new(vec![ModelEntry {
                id: "deepseek/deepseek-v4-flash".into(), object: "model".into(),
                created: chrono::Utc::now().timestamp(), owned_by: "ironlink".into(),
            }]),
            relay_profiles: Mutex::new(profiles), active_relay_id: Mutex::new(active_id),
            apps: Mutex::new(app_list),
            proxy_config: Mutex::new(ProxyConfig::default()),
            log_buffer: Mutex::new(Vec::new()),
        })
    }
}

pub fn toggle_proxy(enabled: bool, default_model: &str, reasoning_effort: &str) -> bool {
    let codex_path = codex_config_path();
    let bak_path = codex_config_bak_path();
    if enabled {
        if !codex_path.exists() {
            warn!("Codex config not found at {:?}", codex_path);
            return false;
        }
        let original = std::fs::read_to_string(&codex_path).unwrap_or_default();
        if let Some(parent) = bak_path.parent() { let _ = std::fs::create_dir_all(parent); }
        if let Err(e) = std::fs::write(&bak_path, &original) {
            warn!("Failed to backup Codex config: {e}");
        }
        if let Err(e) = write_proxy_config(&original, default_model, reasoning_effort) {
            warn!("Failed to write proxy config: {e}");
            return false;
        }
        info!("Proxy enabled — Codex config rewritten to use IronLink");
    } else {
        restore_codex_configs();
        info!("Proxy disabled — Codex config restored from backup");
        let _ = std::fs::remove_file(&bak_path);
    }
    if let Err(e) = write_proxy_enabled_state(enabled) {
        warn!("Failed to persist proxy_enabled state: {e}");
    }
    true
}

fn codex_config_path() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".codex").join("config.toml")
}

fn codex_config_bak_path() -> PathBuf {
    let mut p = codex_config_path(); p.set_extension("toml.bak"); p
}

pub fn read_codex_config() -> String {
    std::fs::read_to_string(codex_config_path()).unwrap_or_default()
}

fn restore_codex_configs() {
    let bak_path = codex_config_bak_path();
    if bak_path.exists() {
        if let Err(e) = std::fs::copy(&bak_path, codex_config_path()) {
            warn!("Failed to restore Codex config from backup: {e}");
        }
    }
}

pub async fn push_log(state: &AppState, msg: String) {
    let mut log = state.log_buffer.lock().await;
    log.push(msg);
    if log.len() > 500 { log.remove(0); }
}

fn write_proxy_config(original: &str, default_model: &str, reasoning_effort: &str) -> anyhow::Result<()> {
    let proxy_url = format!("http://127.0.0.1:{}/v1", PROXY_PORT);
    let bare_model = default_model.split('/').last().unwrap_or(default_model);
    let mut root: toml::Value = original.parse::<toml::Value>().unwrap_or(toml::Value::Table(toml::value::Table::new()));
    let table = root.as_table_mut().unwrap();
    table.insert("model".into(), toml::Value::String(bare_model.to_string()));
    let sandbox_mode = original.lines()
        .find(|l| l.trim().starts_with("sandbox_mode"))
        .and_then(|l| l.trim().split('=').nth(1).map(|s| s.trim().trim_matches('"').to_string()))
        .unwrap_or_else(|| "danger-full-access".to_string());
    table.insert("model_provider".into(), toml::Value::String("ironlink".into()));
    table.insert("model_reasoning_effort".into(), toml::Value::String(reasoning_effort.to_string()));
    table.insert("sandbox_mode".into(), toml::Value::String(sandbox_mode));
    table.remove("openai_base_url"); table.remove("model_catalog_json"); table.remove("shell_environment_policy");

    let ironlink = toml::Value::Table({
        let mut m = toml::value::Table::new();
        m.insert("name".into(), toml::Value::String("IronLink".into()));
        m.insert("base_url".into(), toml::Value::String(proxy_url));
        m.insert("wire_api".into(), toml::Value::String("responses".into()));
        m.insert("supports_websockets".into(), toml::Value::Boolean(false));
        m.insert("requires_openai_auth".into(), toml::Value::Boolean(false));
        m.insert("allow_insecure".into(), toml::Value::Boolean(true));
        m
    });
    let mut mp = match table.remove("model_providers") { Some(toml::Value::Table(t)) => t, _ => toml::value::Table::new() };
    mp.insert("ironlink".into(), ironlink);
    table.insert("model_providers".into(), toml::Value::Table(mp));

    let mut mkts = match table.remove("marketplaces") { Some(toml::Value::Table(t)) => t, _ => toml::value::Table::new() };
    let bundled = match mkts.remove("openai-bundled") { Some(toml::Value::Table(t)) => t, _ => toml::value::Table::new() };
    let mut updated = bundled.clone();
    updated.insert("source_type".into(), toml::Value::String("local".into()));
    for (k, v) in bundled { if k != "source_type" { updated.entry(k).or_insert(v); } }
    mkts.insert("openai-bundled".into(), toml::Value::Table(updated));
    table.insert("marketplaces".into(), toml::Value::Table(mkts));

    let config = toml::to_string_pretty(&root)?;
    let path = codex_config_path();
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&path, &config)?;
    info!("proxy config.toml written");
    Ok(())
}

pub fn read_raw() -> String {
    let path = settings_path();
    std::fs::read_to_string(path.parent().unwrap_or(&path).join("config.toml")).unwrap_or_default()
}

pub fn write_raw(content: &str) -> anyhow::Result<()> {
    let path = settings_path();
    let dir = path.parent().unwrap_or(&path);
    let config_path = dir.join("config.toml");
    std::fs::create_dir_all(dir)?;
    std::fs::write(&config_path, content)?;
    info!("config.toml written");
    Ok(())
}
