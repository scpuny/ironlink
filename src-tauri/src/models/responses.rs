// ── OpenAI Responses API types ──

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// An OpenAI Responses API request body (Codex native format).
pub struct ResponsesRequest {
    pub model: Option<String>,
    pub input: Vec<ResponsesInputItem>,
    pub instructions: Option<String>,
    pub stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
/// An item in the Responses API input array.
pub enum ResponsesInputItem {
    #[serde(rename = "message")] Message { role: String, content: Vec<ResponsesInputContent> },
    #[serde(rename = "function_call_output")] FunctionCallOutput { call_id: String, output: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Content within a Responses API input message.
pub struct ResponsesInputContent {
    #[serde(rename = "type")] pub content_type: String,
    pub text: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Content within a Responses API output message.
pub struct ResponsesOutputContent {
    #[serde(rename = "type")] pub content_type: String,
    pub text: Option<String>,
    pub thinking: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// An output item in a Responses API response.
pub struct ResponsesOutput {
    #[serde(rename = "type")] pub output_type: String,
    pub role: String,
    pub content: Vec<ResponsesOutputContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Token usage in a Responses API response.
pub struct ResponsesUsage {
    pub input_tokens: u32, pub output_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// An OpenAI Responses API response body.
pub struct ResponsesResponse {
    pub id: String, pub object: String, pub created: i64, pub model: String,
    pub output: Vec<ResponsesOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub usage: Option<ResponsesUsage>,
}
