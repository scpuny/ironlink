//! Application configuration, Codex config patching, and shared application state.

pub mod apps;
pub mod profiles;
pub mod settings;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};
use toml_edit;

use crate::models::*;

/// Read proxy port from settings, falling back to default.
pub fn proxy_port() -> u16 {
    settings::AppSettings::load().proxy_port
}
pub const ORIG_PORT: u16 = 57321;

fn settings_path() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ironlink").join("settings.json")
}

pub fn read_auto_start() -> bool {
    settings::AppSettings::load().auto_start
}

pub fn write_auto_start(enabled: bool) -> anyhow::Result<()> {
    let mut s = settings::AppSettings::load();
    s.auto_start = enabled;
    s.save()
}

fn write_proxy_enabled_state(_enabled: bool) -> anyhow::Result<()> {
    // No longer persisted; proxy_enabled is runtime-only in AppState.
    Ok(())
}

pub fn auto_restore_codex_configs_if_needed() {
    let proxy_url = format!("http://127.0.0.1:{}/v1", proxy_port());
    let codex_config = read_codex_config();
    let bak_path = app_codex_bak_path();
    if codex_config.contains(&proxy_url) && bak_path.exists() {
        info!("Startup: Codex config has stale proxy URL, restoring from backup");
        let _ = std::fs::copy(&bak_path, &codex_config_path(None));
        info!("Codex config restored from backup");
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
    /// Sender to trigger graceful proxy server shutdown.
    pub proxy_shutdown_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub settings: Mutex<settings::AppSettings>,
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
        let _ = write_proxy_enabled_state(false);

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
            proxy_shutdown_tx: Mutex::new(None),
            settings: Mutex::new(settings::AppSettings::load()),
        })
    }
}

/// Enable/disable config injection for all enabled apps.
///
/// Enabled:  backup per-app config, rewrite with proxy settings.
/// Disabled: restore per-app config from backup.
pub fn toggle_proxy(enabled: bool, default_model: &str, reasoning_effort: &str,
                    profiles: &[crate::models::RelayProfile], apps: &[crate::models::AppConfig]) -> bool {
    if enabled {
        let mut any_ok = false;
        for app in apps.iter().filter(|a| a.enabled) {
            let Some(inj) = &app.config_injection else { continue; };
            let config_path = std::path::Path::new(&inj.config_path);
            if !config_path.exists() {
                warn!("App '{}' config not found at {:?}", app.name, config_path);
                continue;
            }
            let original = match std::fs::read_to_string(config_path) {
                Ok(c) => c,
                Err(e) => { warn!("Failed to read '{}' config: {e}", app.name); continue; }
            };

            if inj.backup_enabled {
                let bak_path = app_config_bak_path(inj);
                if let Some(parent) = bak_path.parent() { let _ = std::fs::create_dir_all(parent); }
                if let Err(e) = std::fs::write(&bak_path, &original) {
                    warn!("Backup failed for '{}': {e}", app.name);
                }
            }

            if let Err(e) = write_app_proxy_config(&original, &app.default_model, reasoning_effort, profiles, inj, &app.config_snippet, app) {
                warn!("Failed to inject config for '{}': {e}", app.name);
            } else {
                any_ok = true;
                info!("Config injected for '{}'", app.name);
            }
        }

        // Legacy fallback: try default Codex path
        if !any_ok {
            let codex_path = codex_config_path(None);
            if codex_path.exists() {
                let original = std::fs::read_to_string(&codex_path).unwrap_or_default();
                let bak_path = app_codex_bak_path();
                if let Some(parent) = bak_path.parent() { let _ = std::fs::create_dir_all(parent); }
                let _ = std::fs::write(&bak_path, &original);
                let _ = write_proxy_config(&original, default_model, reasoning_effort, profiles);
            }
        }
        any_ok
    } else {
        // Restore from per-app backups
        for app in apps.iter().filter(|a| a.config_injection.is_some()) {
            if let Some(inj) = &app.config_injection {
                if inj.backup_enabled {
                    restore_app_config(inj);
                }
            }
        }
        // Legacy fallback
        restore_codex_configs();

        // Delete model catalog so Codex reverts to its own models
        let catalog_path = model_catalog_path();
        if catalog_path.exists() {
            if let Err(e) = std::fs::remove_file(&catalog_path) {
                warn!("Failed to delete model catalog: {e}");
            } else {
                info!("Model catalog deleted: {:?}", catalog_path);
            }
        }

        true
    }
}

/// Restore a specific app's config from its backup.
pub fn restore_app_config(inj: &crate::models::AppInjection) {
    let config_path = std::path::Path::new(&inj.config_path);
    let bak_path = app_config_bak_path(inj);
    if bak_path.exists() {
        if let Err(e) = std::fs::copy(&bak_path, config_path) {
            warn!("Failed to restore config from backup: {e}");
        } else {
            info!("Config restored from: {:?}", bak_path);
        }
    }
}

pub fn restore_codex_configs() {
    let codex_path = codex_config_path(None);
    let bak_path = app_codex_bak_path();
    if bak_path.exists() {
        if let Err(e) = std::fs::copy(&bak_path, &codex_path) {
            warn!("Failed to restore Codex config from backup: {e}");
        }
    }
}

/// Check if a config file still contains IronLink proxy settings.
/// If `model_provider` is "ironlink" or `model_providers.ironlink` exists with the
/// proxy URL, the config is still "managed" by IronLink and needs restoration.
pub fn config_has_ironlink_settings(content: &str) -> bool {
    if let Ok(doc) = content.parse::<toml_edit::DocumentMut>() {
        // Check model_provider == "ironlink"
        if let Some(v) = doc.get("model_provider").and_then(|v| v.as_str()) {
            if v == "ironlink" { return true; }
        }
        // Check model_providers.ironlink.base_url contains "127.0.0.1" or ironlink proxy
        if let Some(custom) = doc.get("model_providers").and_then(|t| t.get("ironlink")) {
            if let Some(url) = custom.get("base_url").and_then(|v| v.as_str()) {
                if url.contains("127.0.0.1") || url.contains("localhost") {
                    return true;
                }
            }
        }
    }
    false
}

/// Restore from backup only if the current config still has IronLink proxy settings.
/// This prevents overwriting a config the user has already switched away from.
pub fn restore_app_config_if_ironlink(inj: &crate::models::AppInjection) {
    let config_path = std::path::Path::new(&inj.config_path);
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(config_path) {
            if !config_has_ironlink_settings(&content) {
                info!("Skipping restore for {:?}: config no longer uses IronLink proxy", config_path);
                return;
            }
        }
    }
    restore_app_config(inj);
}

pub fn restore_codex_configs_if_ironlink() {
    let codex_path = codex_config_path(None);
    if codex_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&codex_path) {
            if !config_has_ironlink_settings(&content) {
                info!("Skipping legacy restore: Codex config no longer uses IronLink proxy");
                return;
            }
        }
    }
    restore_codex_configs();
}

fn codex_config_path(override_dir: Option<&str>) -> PathBuf {
    match override_dir {
        Some(dir) => PathBuf::from(dir).join("config.toml"),
        None => {
            let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".codex").join("config.toml")
        }
    }
}

/// Unified backup path for Codex config (consistent with app_config_bak_path for codex_toml).
/// Uses ~/.ironlink/codex_toml.bak to match the per-app backup naming.
fn app_codex_bak_path() -> PathBuf {
    ironlink_dir().join("codex_toml.bak")
}

pub fn read_codex_config() -> String {
    std::fs::read_to_string(&codex_config_path(None)).unwrap_or_default()
}

fn write_proxy_config(original: &str, default_model: &str, reasoning_effort: &str, profiles: &[crate::models::RelayProfile]) -> anyhow::Result<()> {
    let proxy_url = format!("http://127.0.0.1:{}/v1", proxy_port());
    let mut doc: toml_edit::DocumentMut = original.parse().map_err(|e| anyhow::anyhow!("TOML parse error: {e}"))?;

    // core model & reasoning effort
    doc["model"] = toml_edit::value(default_model);
    doc["reasoning_effort"] = toml_edit::value(reasoning_effort);

    // Write model catalog to Codex config dir so relative path resolves correctly
    let catalog_path = model_catalog_path();
    write_ironlink_model_catalog(&catalog_path, profiles)?;
    doc["model_catalog_json"] = toml_edit::value(crate::config::model_catalog_path().to_string_lossy().as_ref());

    // Set active model_provider and [model_providers.ironlink] table
    doc["model_provider"] = toml_edit::value("ironlink");
    // [model_providers.ironlink] table
    if !doc.contains_key("model_providers") {
        doc["model_providers"] = toml_edit::table();
    }
    let ironlink_table = doc["model_providers"]["ironlink"]
        .or_insert(toml_edit::table());
    if let Some(t) = ironlink_table.as_table_mut() {
        t.set_implicit(true);
        t["name"] = toml_edit::value("IronLink Proxy");
        t["base_url"] = toml_edit::value(&proxy_url);
        t["wire_api"] = toml_edit::value("responses");
        t["supports_websockets"] = toml_edit::value(false);
        t["requires_openai_auth"] = toml_edit::value(true);
        t["allow_insecure"] = toml_edit::value(true);
    }

    // marketplaces.openai-bundled.source_type = "local"
    if !doc.contains_key("marketplaces") {
        doc["marketplaces"] = toml_edit::table();
    }
    let bundled = doc["marketplaces"]["openai-bundled"]
        .or_insert(toml_edit::table());
    if let Some(t) = bundled.as_table_mut() {
        t.set_implicit(true);
        t["source_type"] = toml_edit::value("local");
    }

    let config = doc.to_string();
    let path = codex_config_path(None);
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&path, &config)?;
    info!("proxy config.toml written (toml_edit)");
    Ok(())
}

/// Extract sandbox_mode from original config, default to "danger-full-access".
fn _codex_sandbox_mode(original: &str) -> String {
    if let Ok(doc) = original.parse::<toml_edit::DocumentMut>() {
        doc.get("sandbox_mode")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| "danger-full-access".to_string())
    } else {
        "danger-full-access".to_string()
    }
}

/// Backup path for a specific app injection.
pub fn app_config_bak_path(inj: &crate::models::AppInjection) -> PathBuf {
    let base = inj.config_dir.as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| ironlink_dir());
    base.join(format!("{}.bak", inj.config_type))
}

/// Atomically write content to a file: write to .tmp then rename.
pub fn atomic_write(path: &std::path::Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, content)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Write proxy config for a specific app, respecting field-level control and snippet.
fn write_app_proxy_config(original: &str, default_model: &str, reasoning_effort: &str,
                          profiles: &[crate::models::RelayProfile],
                          inj: &crate::models::AppInjection, snippet: &Option<String>,
                          app: &crate::models::AppConfig) -> anyhow::Result<()> {
    let proxy_url = format!("http://127.0.0.1:{}/v1", proxy_port());
    let config_path = std::path::Path::new(&inj.config_path);
    let fields = inj.fields.as_ref();

    match inj.config_type.as_str() {
        "codex_toml" => {
            let mut doc: toml_edit::DocumentMut = original.parse()
                .map_err(|e| anyhow::anyhow!("TOML parse: {e}"))?;

            let wants = |f: &str| fields.as_ref().map_or(true, |fl| fl.contains(&f.to_string()));

            if wants("model") { doc["model"] = toml_edit::value(default_model); }
            if wants("reasoning_effort") { doc["reasoning_effort"] = toml_edit::value(reasoning_effort); }

            if wants("model_catalog_json") {
                let catalog_path = model_catalog_path();
                if app.model_replacement_enabled {
                    // Model mapping enabled: catalog only contains mapped models
                    write_mapped_model_catalog(&catalog_path, app, profiles)?;
                } else {
                    // Model mapping disabled: catalog contains all provider models
                    write_ironlink_model_catalog(&catalog_path, profiles)?;
                }
                doc["model_catalog_json"] = toml_edit::value(crate::config::model_catalog_path().to_string_lossy().as_ref());
            }

            if wants("model_provider") {
                doc["model_provider"] = toml_edit::value("ironlink");
            }

            if wants("model_providers") {
                if !doc.contains_key("model_providers") {
                    doc["model_providers"] = toml_edit::table();
                }
                let ironlink_table = doc["model_providers"]["ironlink"]
                    .or_insert(toml_edit::table());
                if let Some(t) = ironlink_table.as_table_mut() {
                    t.set_implicit(true);
                    t["name"] = toml_edit::value("IronLink Proxy");
                    t["base_url"] = toml_edit::value(&proxy_url);
                    t["wire_api"] = toml_edit::value("responses");
                    t["supports_websockets"] = toml_edit::value(false);
                    t["requires_openai_auth"] = toml_edit::value(true);
                    t["allow_insecure"] = toml_edit::value(true);
                }
            }

            if wants("marketplaces") {
                if !doc.contains_key("marketplaces") {
                    doc["marketplaces"] = toml_edit::table();
                }
                let bundled = doc["marketplaces"]["openai-bundled"]
                    .or_insert(toml_edit::table());
                if let Some(t) = bundled.as_table_mut() {
                    t.set_implicit(true);
                    t["source_type"] = toml_edit::value("local");
                }
            }

            let mut config_str = doc.to_string();
            if let Some(snip) = snippet.as_ref().filter(|s| !s.trim().is_empty()) {
                config_str.push_str("\n# ironlink: user snippet\n");
                config_str.push_str(snip);
                config_str.push('\n');
            }

            atomic_write(&config_path, &config_str)?;
            info!("Codex config written (field-filtered) for: {:?}", config_path);
        }
        _ => {
            // Fallback for unknown config types
            write_proxy_config(original, default_model, reasoning_effort, profiles)?;
        }
    }
    Ok(())
}


/// Preview what an app's config would look like after injection, without writing.
pub fn preview_app_config(original: &str, default_model: &str, reasoning_effort: &str,
                          profiles: &[crate::models::RelayProfile],
                          inj: &crate::models::AppInjection,
                          snippet: &Option<String>,
                          app: &crate::models::AppConfig) -> String {
    let proxy_url = format!("http://127.0.0.1:{}/v1", proxy_port());
    let fields = inj.fields.as_ref();

    match inj.config_type.as_str() {
        "codex_toml" => {
            let mut doc = match original.parse::<toml_edit::DocumentMut>() {
                Ok(d) => d,
                Err(_) => return format!("# Failed to parse existing config as TOML\n{}", original),
            };

            let wants = |f: &str| fields.as_ref().map_or(true, |fl| fl.contains(&f.to_string()));

            // Apply injection changes directly to the document
            if wants("model") {
                doc["model"] = toml_edit::value(default_model);
            }
            if wants("reasoning_effort") {
                doc["reasoning_effort"] = toml_edit::value(reasoning_effort);
            }
            if wants("model_catalog_json") {
                doc["model_catalog_json"] = toml_edit::value(crate::config::model_catalog_path().to_string_lossy().as_ref());
            }
            if wants("model_provider") {
                doc["model_provider"] = toml_edit::value("ironlink");
            }
            if wants("model_providers") {
                doc["model_providers"]["ironlink"]["name"] = toml_edit::value("IronLink");
                doc["model_providers"]["ironlink"]["base_url"] = toml_edit::value(&proxy_url);
                doc["model_providers"]["ironlink"]["wire_api"] = toml_edit::value("responses");
                doc["model_providers"]["ironlink"]["requires_openai_auth"] = toml_edit::value(false);
                doc["model_providers"]["ironlink"]["allow_insecure"] = toml_edit::value(true);
            }
            if wants("marketplaces") {
                doc["marketplaces"]["openai-bundled"]["source_type"] = toml_edit::value("local");
            }

            let mut result = String::new();
            result.push_str("# IronLink Proxy Config Preview\n");
            result.push_str(&doc.to_string());

            if let Some(snip) = snippet.as_ref().filter(|s| !s.trim().is_empty()) {
                result.push('\n');
                result.push_str("# --- User snippet ---\n");
                result.push_str(snip);
                result.push('\n');
            }

            // Append a preview of the model catalog JSON when model replacement is enabled
            if wants("model_catalog_json") && app.model_replacement_enabled {
                if let Ok(catalog_json) = preview_model_catalog(profiles, app) {
                    result.push_str("\n\n# --- ironlink-model-catalog.json (preview) ---\n");
                    result.push_str(&catalog_json);
                    result.push('\n');
                }
            }

            result
        }
        _ => format!("# Preview not available for config type: {}", inj.config_type),
    }
}

/// Generate a preview of the model catalog JSON string, using model mappings when enabled.
fn preview_model_catalog(profiles: &[crate::models::RelayProfile], app: &crate::models::AppConfig) -> anyhow::Result<String> {
    let template_text = include_str!("../resources/gpt5_5_template.json");
    let template: serde_json::Value = serde_json::from_str(template_text)
        .map_err(|e| anyhow::anyhow!("Failed to parse template: {e}"))?;

    let mut entries = Vec::new();

    for (idx, (codex_model, target)) in app.model_mappings.iter().enumerate() {
        let provider_name = profiles.iter()
            .find(|p| p.enabled && p.provider_id == target.provider_id)
            .map(|p| p.name.as_str())
            .unwrap_or(&target.provider_id);

        let display_name = format!("{}/{}", target.provider_id, target.upstream_model);

        let mut entry = template.clone();
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("slug".to_string(), serde_json::json!(codex_model));
            obj.insert("display_name".to_string(), serde_json::json!(display_name));
            obj.insert("description".to_string(), serde_json::json!(format!("IronLink proxy via {}", provider_name)));
            obj.insert("priority".to_string(), serde_json::json!(1000 + idx));
            obj.insert("additional_speed_tiers".to_string(), serde_json::json!([]));
            obj.insert("service_tiers".to_string(), serde_json::json!([]));
            obj.insert("availability_nux".to_string(), serde_json::Value::Null);
            obj.insert("upgrade".to_string(), serde_json::Value::Null);
        }
        entries.push(entry);
    }

    let catalog = serde_json::json!({ "models": entries });
    Ok(serde_json::to_string_pretty(&catalog)?)
}

/// Generate the model catalog using app model_mappings.
/// Only models that have a mapping entry appear in the catalog.
/// Slug = the original Codex model name (key), display_name = providerId/upstream_model.
///
pub fn write_mapped_model_catalog(path: &std::path::Path, app: &crate::models::AppConfig, profiles: &[crate::models::RelayProfile]) -> anyhow::Result<()> {
    let template_text = include_str!("../resources/gpt5_5_template.json");
    let template: serde_json::Value = serde_json::from_str(template_text)
        .map_err(|e| anyhow::anyhow!("Failed to parse template: {e}"))?;

    let mut entries = Vec::new();

    for (idx, (codex_model, target)) in app.model_mappings.iter().enumerate() {
        // Find the provider to get a friendly display name
        let provider_name = profiles.iter()
            .find(|p| p.enabled && p.provider_id == target.provider_id)
            .map(|p| p.name.as_str())
            .unwrap_or(&target.provider_id);

        let display_name = format!("{}/{}", target.provider_id, target.upstream_model);

        let mut entry = template.clone();
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("slug".to_string(), serde_json::json!(codex_model));
            obj.insert("display_name".to_string(), serde_json::json!(display_name));
            obj.insert("description".to_string(), serde_json::json!(format!("IronLink proxy via {}", provider_name)));
            obj.insert("priority".to_string(), serde_json::json!(1000 + idx));
            obj.insert("additional_speed_tiers".to_string(), serde_json::json!([]));
            obj.insert("service_tiers".to_string(), serde_json::json!([]));
            obj.insert("availability_nux".to_string(), serde_json::Value::Null);
            obj.insert("upgrade".to_string(), serde_json::Value::Null);
        }
        entries.push(entry);
    }

    let catalog = serde_json::json!({ "models": entries });
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(path, serde_json::to_string_pretty(&catalog)?)?;
    info!("mapped model catalog written ({} entries from model_mappings)", entries.len());
    Ok(())
}

/// Generate the `ironlink-model-catalog.json` file that tells Codex which models are available.
/// Follows cc-switch's approach: clone the bundled gpt-5.5 template for each provider model.
pub fn write_ironlink_model_catalog(path: &std::path::Path, profiles: &[crate::models::RelayProfile]) -> anyhow::Result<()> {
    let template_text = include_str!("../resources/gpt5_5_template.json");
    let template: serde_json::Value = serde_json::from_str(template_text)
        .map_err(|e| anyhow::anyhow!("Failed to parse template: {e}"))?;

    let mut entries = Vec::new();

    for p in profiles.iter().filter(|p| p.enabled) {
        let mut seen = std::collections::HashSet::new();
        let all_models: Vec<&str> = p.model_list
            .iter()
            .flat_map(|m| m.split_whitespace())
            .chain(std::iter::once(p.model.as_str()))
            .filter(|m| !m.is_empty())
            .collect();

        for (idx, model_id) in all_models.iter().enumerate() {
            if !seen.insert(model_id.to_string()) { continue; }
            let slug = format!("{}/{}", p.provider_id, model_id);

            let mut entry = template.clone();
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("slug".to_string(), serde_json::json!(slug));
                obj.insert("display_name".to_string(), serde_json::json!(format!("{} -- {}", p.name, model_id)));
                obj.insert("description".to_string(), serde_json::json!(format!("IronLink proxy via {}", p.name)));
                obj.insert("priority".to_string(), serde_json::json!(1000 + idx));
                obj.insert("additional_speed_tiers".to_string(), serde_json::json!([]));
                // Match cc-switch's catalog format for Desktop compatibility
                obj.insert("service_tiers".to_string(), serde_json::json!([]));
                obj.insert("availability_nux".to_string(), serde_json::Value::Null);
                obj.insert("upgrade".to_string(), serde_json::Value::Null);
            }
            entries.push(entry);
        }
    }

    let catalog = serde_json::json!({ "models": entries });
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(path, serde_json::to_string_pretty(&catalog)?)?;
    info!("ironlink-model-catalog.json written ({} entries)", entries.len());
    Ok(())
}

/// Generate a model catalog for an app with model replacement enabled.
/// Uses the app's original model IDs as slugs and only replaces display names.
/// Models without a display name replacement are excluded from the catalog.
pub fn write_app_model_catalog(path: &std::path::Path, app: &crate::models::AppConfig) -> anyhow::Result<()> {
    let template_text = include_str!("../resources/gpt5_5_template.json");
    let template: serde_json::Value = serde_json::from_str(template_text)
        .map_err(|e| anyhow::anyhow!("Failed to parse template: {e}"))?;

    let mut entries = Vec::new();

    for (idx, model_id) in app.models.iter().enumerate() {
        // Only include models that have a non-empty display name replacement
        let display_name = match app.model_display_names.get(model_id) {
            Some(name) if !name.trim().is_empty() => name.trim().to_string(),
            _ => continue,
        };

        let mut entry = template.clone();
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("slug".to_string(), serde_json::json!(model_id));
            obj.insert("display_name".to_string(), serde_json::json!(display_name));
            obj.insert("description".to_string(), serde_json::json!(format!("IronLink model replacement: {}", display_name)));
            obj.insert("priority".to_string(), serde_json::json!(1000 + idx));
            obj.insert("additional_speed_tiers".to_string(), serde_json::json!([]));
            obj.insert("service_tiers".to_string(), serde_json::json!([]));
            obj.insert("availability_nux".to_string(), serde_json::Value::Null);
            obj.insert("upgrade".to_string(), serde_json::Value::Null);
        }
        entries.push(entry);
    }

    let catalog = serde_json::json!({ "models": entries });
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(path, serde_json::to_string_pretty(&catalog)?)?;
    info!("app model catalog written ({} entries, model_replacement_enabled)", entries.len());
    Ok(())
}

pub fn ironlink_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home).join(".ironlink")
}

/// Path to the Codex config directory (~/.codex).
pub fn codex_config_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home).join(".codex")
}

/// Path to the ironlink model catalog JSON (stored in Codex config dir so relative path resolves).
pub fn model_catalog_path() -> std::path::PathBuf {
    codex_config_dir().join("ironlink-model-catalog.json")
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

/// Append a log line to the shared log buffer (bounded to 500 entries).
pub async fn push_log(state: &Arc<AppState>, line: String) {
    let mut buf = state.log_buffer.lock().await;
    buf.push(line);
    let n = buf.len();
    if n > 500 {
        buf.drain(0..n - 500);
    }
}