// ── ModelInfo / ModelsResponse for Codex model discovery ──

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Response body for GET /v1/models — list of models Codex can use.
pub struct ModelsResponse { pub models: Vec<ModelInfo> }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Metadata for a single model, matching Codex's expected format.
pub struct ModelInfo {
    pub slug: String, pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub default_reasoning_level: Option<String>,
    pub supported_reasoning_levels: Vec<ReasoningEffortPreset>,
    pub shell_type: String, pub visibility: String, pub supported_in_api: bool, pub priority: i32,
    #[serde(default)] pub additional_speed_tiers: Vec<String>,
    pub base_instructions: String,
    #[serde(default)] pub supports_reasoning_summaries: bool,
    #[serde(default)] pub default_reasoning_summary: String,
    #[serde(default)] pub support_verbosity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub default_verbosity: Option<String>,
    pub web_search_tool_type: String,
    pub truncation_policy: TruncationPolicyConfig,
    pub supports_parallel_tool_calls: bool,
    #[serde(default)] pub supports_image_detail_original: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub context_window: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub max_context_window: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub auto_compact_token_limit: Option<i64>,
    #[serde(default = "default_effective_context_window_percent")] pub effective_context_window_percent: i64,
    #[serde(default)] pub experimental_supported_tools: Vec<String>,
    #[serde(default)] pub input_modalities: Vec<String>,
    #[serde(default)] pub supports_search_tool: bool,
    #[serde(default)] pub use_responses_lite: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub apply_patch_tool_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub auto_review_model_override: Option<String>,
}

fn default_effective_context_window_percent() -> i64 { 90 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// A reasoning effort level option displayed in Codex's model picker.
pub struct ReasoningEffortPreset {
    pub effort: String, pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Configuration for automatic context truncation.
pub struct TruncationPolicyConfig {
    #[serde(default)] pub auto: bool, #[serde(default)] pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub max_tokens: Option<i64>,
}
