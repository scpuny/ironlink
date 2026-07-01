// ── Backend/Relay profile types ──

use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A single model entry returned from the GET /v1/models endpoint.
pub struct ModelEntry {
    pub id: String, pub object: String, pub created: i64, pub owned_by: String,
}
