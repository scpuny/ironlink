//! Application (downstream client) configuration.
//!
//! An "app" is a downstream AI client that IronLink serves, e.g. Codex Desktop,
//! Claude Desktop, Cursor. Each app bundles:
//!   - Connection protocol and model info
//!   - Per-app model mappings (app_model → provider_id + upstream_model)
//!   - Config injection (how to rewrite the app's config to point to IronLink)

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// A downstream application that IronLink proxies requests for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub id: String,
    pub name: String,
    /// Wire protocol this app speaks: "responses", "anthropic", "chatCompletions"
    pub protocol: String,
    pub enabled: bool,
    /// Default model for this app (e.g. "gpt-5.5" for Codex)
    #[serde(default)]
    pub default_model: String,
    /// List of models this app supports/uses
    #[serde(default)]
    pub models: Vec<String>,
    /// How to inject IronLink proxy config into this app's config file
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_injection: Option<AppInjection>,
    /// Per-app model mappings: app model name → upstream provider + model
    #[serde(default)]
    pub model_mappings: HashMap<String, MappingTarget>,
    /// Custom TOML/JSON snippet to merge when injecting config (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_snippet: Option<String>,
}

/// Instructions for rewriting an app's configuration file to use IronLink as proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInjection {
    /// Config format type: "codex_toml", "claude_json", etc.
    pub config_type: String,
    /// Absolute path to the app's configuration file
    pub config_path: String,

    /// Override the app's config base directory (e.g. custom ~/.codex path).
    /// When set, the backup/restore path is derived from this directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,

    /// Whether to atomic-backup the config file before overwriting (default true).
    #[serde(default = "default_backup_enabled")]
    pub backup_enabled: bool,

    /// Which top-level fields to inject into the config file.
    /// - None (= default): inject all known fields (model, reason_effort, model_providers, etc.)
    /// - Some(list): only inject the specified fields.
    /// Supported field names: "model", "reasoning_effort", "model_catalog_json",
    /// "model_providers", "experimental_bearer_token", "marketplaces".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,
}

fn default_backup_enabled() -> bool { true }

/// Where to route a matching model: which provider and what model to use upstream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingTarget {
    pub provider_id: String,
    pub upstream_model: String,
}
