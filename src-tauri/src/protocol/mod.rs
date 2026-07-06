//! Protocol conversion library — direct wire-format transformation.
//!
//! Uses struct-based ToolContext (like CodexPlusPlus/cc-switch) instead of
//! abstract canonical intermediate types. SSE streaming handled by sse/ modules.
//!
//! To add a new protocol:
//!   - Add conversion logic to convert.rs
//!   - Add SSE handling under sse/ if needed

pub mod core;
pub mod input;
pub mod output;
pub mod sse;
pub mod tools;
pub mod reasoning;
pub mod tool_context;
pub mod convert;

use serde_json::Value;
pub use crate::protocol::sse::transform::SseTransformStream;
pub use crate::protocol::sse::chat_sse::ChatSseConverter;

// ── Request conversion ──

/// Convert a Responses API request body to a target upstream protocol format.
pub fn responses_to_upstream(body: &Value, output_protocol: &str) -> anyhow::Result<Value> {
    match output_protocol {
        "chat_completions" | "openai-chat" | "chatCompletions" => {
            convert::responses_to_chat(body)
        }
        "anthropic" => anthropic_request_fallback(body),
        "responses" | "openai-responses" | "openai_responses" => Ok(body.clone()),
        other => Err(anyhow::anyhow!("unknown output protocol: {other}")),
    }
}

/// Convert an upstream response body to Responses API format.
pub fn upstream_to_responses(body: &Value, input_protocol: &str, original_request: Option<&Value>) -> anyhow::Result<Value> {
    match input_protocol {
        "chat_completions" | "openai-chat" | "chatCompletions" => {
            convert::chat_to_responses(body, original_request)
        }
        "anthropic" => anthropic_response_fallback(body),
        "responses" | "openai-responses" | "openai_responses" => Ok(body.clone()),
        other => Err(anyhow::anyhow!("unknown input protocol: {other}")),
    }
}

// ── Anthropic fallback (uses existing canonical types from input/output modules) ──

use crate::protocol::core::traits::{InputProtocol, OutputProtocol};
use crate::protocol::core::types::*;

fn anthropic_request_fallback(body: &Value) -> anyhow::Result<Value> {
    use crate::protocol::input::responses::ResponsesInput;
    let input = ResponsesInput;
    let req = input.parse_request(body)
        .map_err(|e| anyhow::anyhow!("failed to parse Responses request: {e}"))?;
    let output = crate::protocol::output::anthropic::AnthropicOutput;
    output.build_request(&req)
}

fn anthropic_response_fallback(body: &Value) -> anyhow::Result<Value> {
    let output = crate::protocol::output::responses::ResponsesOutput;
    let canonical = direct_parse_anthropic_response(body)?;
    output.build_response(&canonical)
}

// ── Legacy helpers for Anthropic/Chat response parsing ──

fn direct_parse_anthropic_response(body: &Value) -> anyhow::Result<ProtocolResponse> {
    let content_blocks = body.get("content").and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("missing content"))?;

    let mut output = Vec::new();
    let mut tool_calls = Vec::new();
    let mut text_parts = Vec::new();

    for block in content_blocks {
        match block.get("type").and_then(Value::as_str).unwrap_or("") {
            "text" => {
                if let Some(t) = block.get("text").and_then(Value::as_str) {
                    if !t.is_empty() { text_parts.push(t.to_string()); }
                }
            }
            "thinking" | "reasoning" => {
                if let Some(t) = block.get("thinking").or_else(|| block.get("text")).and_then(Value::as_str) {
                    let mut reasoning_text = t.to_string();
                    // Include signature if present (for extended thinking continuation)
                    if let Some(sig) = block.get("signature").and_then(Value::as_str) {
                        if !sig.is_empty() {
                            reasoning_text.push_str(&format!("\n\n<thinking_signature>{sig}</thinking_signature>"));
                        }
                    }
                    output.push(OutputItem::Reasoning { text: reasoning_text });
                }
            }
            "tool_use" => {
                let id = block.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                let name = block.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                // Anthropic sends input as JSON object; serialize for standard format
                let input = block.get("input").map(|v| {
                    if v.is_string() { v.as_str().unwrap_or("").to_string() }
                    else { serde_json::to_string(v).unwrap_or_default() }
                }).unwrap_or_default();
                tool_calls.push(ToolCall {
                    id, name, arguments: input,
                    tool_type: ToolType::Function,
                });
            }
            _ => {}
        }
    }

    if !text_parts.is_empty() || !tool_calls.is_empty() {
        let content: Vec<ContentPart> = if text_parts.is_empty() {
            vec![ContentPart::Text(String::new())]
        } else {
            text_parts.iter().map(|t| ContentPart::Text(t.clone())).collect()
        };
        output.push(OutputItem::Message {
            role: "assistant".to_string(),
            content,
        });
    }

    // Add tool_calls from last message
    for tc in &tool_calls {
        output.push(OutputItem::ToolCall {
            id: tc.id.clone(),
            name: tc.name.clone(),
            arguments: tc.arguments.clone(),
            tool_type: ToolType::Function,
        });
    }

    // Parse stop_reason for proper status
    let stop_reason = body.get("stop_reason").and_then(Value::as_str);
    let status = match stop_reason {
        Some("max_tokens") => ResponseStatus::Incomplete,
        Some("error") => ResponseStatus::Failed,
        _ => ResponseStatus::Completed,
    };

    let usage_obj = body.get("usage").unwrap_or(&Value::Null);
    let input_tokens = usage_obj.get("input_tokens").and_then(Value::as_u64)
        .or_else(|| usage_obj.get("prompt_tokens").and_then(Value::as_u64)).unwrap_or(0);
    let output_tokens = usage_obj.get("output_tokens").and_then(Value::as_u64)
        .or_else(|| usage_obj.get("completion_tokens").and_then(Value::as_u64)).unwrap_or(0);
    let cached_input_tokens = usage_obj.get("cache_read_input_tokens")
        .and_then(Value::as_u64);

    Ok(ProtocolResponse {
        id: body.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
        model: body.get("model").and_then(Value::as_str).unwrap_or("").to_string(),
        created_at: body.get("created").and_then(Value::as_u64).unwrap_or(0),
        status,
        output,
        usage: Usage {
            input_tokens, output_tokens,
            total_tokens: input_tokens + output_tokens,
            cached_input_tokens,
            extra: vec![],
        },
        passthrough: PassthroughFields::default(),
    })
}

