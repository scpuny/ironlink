// ── Proxy status/config types ──

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Current proxy status returned to the frontend.
pub struct ProxyStatus {
    pub enabled: bool, pub backend: String, pub api_base: String, pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Proxy-level configuration (default model, reasoning effort).
pub struct ProxyConfig {
    pub default_model: String, pub reasoning_effort: String,
}

impl Default for ProxyConfig {
    fn default() -> Self { Self { default_model: "deepseek-v4-flash".into(), reasoning_effort: "medium".into() } }
}
