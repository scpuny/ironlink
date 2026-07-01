// ── OpenAI Chat API types ──

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A message in an OpenAI Chat Completions request.
pub struct ChatMessage {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// An OpenAI Chat Completions API request body.
pub struct ChatRequest {
    pub model: String, pub messages: Vec<ChatMessage>, pub stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub top_p: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub tools: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub tool_choice: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A single choice in a Chat Completions response.
pub struct ChatChoice {
    pub index: u32, pub message: ChatMessage,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Token usage statistics for a Chat Completions response.
pub struct ChatUsage { pub prompt_tokens: u32, pub completion_tokens: u32, pub total_tokens: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
/// An OpenAI Chat Completions API response body.
pub struct ChatResponse {
    pub id: String, pub object: String, pub created: i64, pub model: String,
    pub choices: Vec<ChatChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub usage: Option<ChatUsage>,
}
