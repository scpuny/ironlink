//! Application (downstream client) configuration.
//!
//! An "app" is a downstream AI client that IronLink serves, e.g. Codex Desktop,
//! Claude Desktop, Cursor. Each app knows its own wire protocol and has its
//! own model mapping table (codex_model_name -> provider_id + upstream_model).

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
    /// Per-app model mappings: Codex model name -> upstream provider + model
    #[serde(default)]
    pub model_mappings: HashMap<String, MappingTarget>,
}

/// Where to route a matching model request: which provider and what model to use upstream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingTarget {
    pub provider_id: String,
    pub upstream_model: String,
}
