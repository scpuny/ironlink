// ── Backend/Relay profile types ──

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
/// Supported upstream API protocol types.
pub enum BackendType {
    #[serde(rename = "openai-chat")] OpenaiChat,
    #[serde(rename = "openai-responses")] OpenaiResponses,
    #[serde(rename = "anthropic")] Anthropic,
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::OpenaiChat => write!(f, "openai-chat"), Self::OpenaiResponses => write!(f, "openai-responses"), Self::Anthropic => write!(f, "anthropic") }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Legacy backend configuration. Kept for backward compatibility with old settings.
pub struct BackendConfig {
    #[serde(rename = "type")]
    pub backend_type: BackendType,
    pub api_base: String,
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub test_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub auth_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub custom_headers: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub config_contents: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub user_agent: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
/// An upstream provider profile. IronLink routes requests to these.
pub struct RelayProfile {
    pub id: String,
    #[serde(default)] pub provider_id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub protocol: String,
    pub model: String,
    pub test_model: String,
    pub model_list: Vec<String>,
    pub enabled: bool,
    pub active: bool,
    /// Per-model capabilities: model_name → ["text", "vision", "image"]
    /// text=文本, vision=图像理解, image=图像生成
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub model_capabilities: HashMap<String, Vec<String>>,
    /// Per-model context window override: model_name → window size
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub model_context_windows: HashMap<String, i64>,
    /// Per-model max context window override: model_name → window size
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub model_max_context_windows: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A single model entry returned from the GET /v1/models endpoint.
pub struct ModelEntry {
    pub id: String, pub object: String, pub created: i64, pub owned_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_context_window: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_modalities: Option<Vec<String>>,
}

/// Known vision-capable model prefixes (image understanding).
const VISION_PREFIXES: &[&str] = &[
    "gpt-4o", "gpt-4-turbo", "gpt-5",
    "claude-3-5-sonnet", "claude-3-opus", "claude-3-haiku",
    "claude-sonnet-4", "claude-opus-4",
    "gemini-1.5", "gemini-2.0", "gemini-2.5",
    "grok-vision", "qwen-vl", "llava",
];

/// Known image-generation model prefixes.
const GEN_PREFIXES: &[&str] = &[
    "dall-e", "stable-diffusion", "sdxl",
];

/// Auto-detect modalities for a model name based on known prefixes.
pub fn auto_detect_modalities(model_name: &str) -> Vec<String> {
    let mut caps = vec!["text".to_string()];
    let lower = model_name.to_lowercase();
    if VISION_PREFIXES.iter().any(|p| lower.starts_with(p)) {
        caps.push("vision".to_string());
    }
    if GEN_PREFIXES.iter().any(|p| lower.contains(p)) {
        caps.push("image".to_string());
    }
    caps
}

/// Get the effective modalities for a model, respecting manual overrides.
pub fn get_model_modalities(
    model_capabilities: &HashMap<String, Vec<String>>,
    model_name: &str,
) -> Vec<String> {
    model_capabilities
        .get(model_name)
        .filter(|c| !c.is_empty())
        .cloned()
        .unwrap_or_else(|| auto_detect_modalities(model_name))
}

/// Check whether a model supports vision (image understanding).
pub fn supports_vision(
    model_capabilities: &HashMap<String, Vec<String>>,
    model_name: &str,
) -> bool {
    let mods = get_model_modalities(model_capabilities, model_name);
    mods.iter().any(|m| m == "vision")
}
