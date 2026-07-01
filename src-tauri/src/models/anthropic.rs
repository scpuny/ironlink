// ── Anthropic Messages API types ──

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A message in an Anthropic Messages API request.
pub struct AnthropicMessage { pub role: String, pub content: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
/// An Anthropic Messages API request body.
pub struct AnthropicRequest {
    pub model: String, pub max_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub system: Option<String>,
    pub messages: Vec<AnthropicMessage>, pub stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A single content item in an Anthropic response.
pub struct AnthropicContentItem {
    #[serde(rename = "type")] pub content_type: String, pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// An Anthropic Messages API response body.
pub struct AnthropicResponse {
    pub id: String, pub resp_type: String, pub role: String,
    pub content: Vec<AnthropicContentItem>, pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub stop_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Token usage statistics for an Anthropic response.
pub struct AnthropicUsage { pub input_tokens: u32, pub output_tokens: u32 }
