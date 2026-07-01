// ── Reasoning style inference and effort mapping ──

use serde_json::Value;
use crate::protocol::core::types::ProtocolRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Reasoning effort dialect for different model providers.
pub enum ReasoningStyle {
    Default,
    DeepSeek,
    LowHigh,
    OpenRouter,
    Thinking,
    EnableThinking,
    ReasoningSplit,
}

/// Infer the reasoning style from model name.
pub fn infer_style(model: &str) -> ReasoningStyle {
    let m = model.to_ascii_lowercase();
    if m.contains("openrouter") || m.starts_with("openrouter/") { return ReasoningStyle::OpenRouter; }
    if m.contains("deepseek") { return ReasoningStyle::DeepSeek; }
    if m.contains("qwen") || m.contains("dashscope") || m.contains("bailian") { return ReasoningStyle::EnableThinking; }
    if m.contains("kimi") || m.contains("moonshot") || m.contains("glm") || m.contains("zhipu") || m.contains("z.ai") || m.contains("mimo") { return ReasoningStyle::Thinking; }
    if m.contains("minimax") { return ReasoningStyle::ReasoningSplit; }
    if m.contains("siliconflow") { return ReasoningStyle::EnableThinking; }
    if m.contains("stepfun") { return ReasoningStyle::LowHigh; }
    ReasoningStyle::Default
}

/// Map a Codex reasoning effort to what the upstream model understands.
pub fn map_effort(effort: &str, style: ReasoningStyle) -> Option<&'static str> {
    let e = effort.trim().to_ascii_lowercase();
    if matches!(e.as_str(), "none" | "off" | "disabled") { return None; }
    match style {
        ReasoningStyle::DeepSeek => match e.as_str() { "max" | "xhigh" => Some("max"), _ => Some("high") },
        ReasoningStyle::LowHigh => match e.as_str() { "minimal" | "low" => Some("low"), _ => Some("high") },
        ReasoningStyle::OpenRouter => match e.as_str() {
            "max" | "xhigh" => Some("xhigh"), "high" => Some("high"), "medium" => Some("medium"),
            "low" => Some("low"), "minimal" => Some("minimal"), _ => None,
        },
        _ => match e.as_str() {
            "minimal" => Some("minimal"), "low" => Some("low"), "medium" => Some("medium"),
            "high" => Some("high"), "xhigh" => Some("xhigh"), "max" => Some("max"), _ => None,
        },
    }
}

/// Check if the model supports `reasoning_effort` field.
pub fn supports_reasoning_effort(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    (m.len() > 1 && m.starts_with('o') && m.as_bytes().get(1).is_some_and(|b| b.is_ascii_digit()))
        || m.strip_prefix("gpt-").and_then(|r| r.chars().next()).is_some_and(|c| c.is_ascii_digit() && c >= '5')
        || matches!(infer_style(model), ReasoningStyle::DeepSeek | ReasoningStyle::LowHigh)
}

/// Apply reasoning options to a Chat Completions request body.
pub fn apply_reasoning_options(result: &mut Value, req: &ProtocolRequest) {
    let Some(ref reasoning) = req.reasoning else { return; };
    let style = infer_style(&req.model);
    let model = &req.model;

    match style {
        ReasoningStyle::Thinking => {
            result["thinking"] = serde_json::json!({"type": if reasoning.enabled { "enabled" } else { "disabled" }});
        }
        ReasoningStyle::EnableThinking => {
            result["enable_thinking"] = serde_json::json!(reasoning.enabled);
        }
        ReasoningStyle::ReasoningSplit => {
            result["reasoning_split"] = serde_json::json!(reasoning.enabled);
        }
        _ => {}
    }

    if !reasoning.enabled {
        if style == ReasoningStyle::OpenRouter { result["reasoning"] = serde_json::json!({"effort": "none"}); }
        return;
    }

    let Some(ref effort) = reasoning.effort else { return; };
    let Some(mapped) = map_effort(effort, style) else { return; };

    match style {
        ReasoningStyle::OpenRouter => {
            result["reasoning"] = serde_json::json!({"effort": mapped});
        }
        ReasoningStyle::DeepSeek | ReasoningStyle::LowHigh | ReasoningStyle::Default
            if supports_reasoning_effort(model) => {
            result["reasoning_effort"] = serde_json::json!(mapped);
        }
        _ => {}
    }
}
