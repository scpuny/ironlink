//! Protocol conversion library and built-in registry.
//! 
//! Architecture: wire format -> InputProtocol -> canonical types -> OutputProtocol -> wire format.
//! 
//! To add a new protocol:
//!   - Add an InputProtocol impl under input/
//!   - Add an OutputProtocol impl under output/
//!   - Add SSE handling under sse/ if needed
//!   - Register in get_input()/get_output() below

// ── Protocol conversion library ──

pub mod core;
pub mod input;
pub mod output;
pub mod sse;
pub mod tools;
pub mod reasoning;

use serde_json::Value;
pub use crate::protocol::core::traits::{InputProtocol, OutputProtocol, ProtocolPair};
pub use crate::protocol::core::types::*;
pub use crate::protocol::sse::transform::SseTransformStream;
pub use crate::protocol::sse::chat_sse::ChatSseConverter;

// ── Built-in protocol registry ──

/// Get the built-in input protocol by name.
pub fn get_input(name: &str) -> Option<Box<dyn InputProtocol>> {
    match name {
        "responses" => Some(Box::new(input::responses::ResponsesInput)),
        _ => None,
    }
}

/// Get the built-in output protocol by name.
pub fn get_output(name: &str) -> Option<Box<dyn OutputProtocol>> {
    match name {
        "chat_completions" | "openai-chat" | "chatCompletions" => Some(Box::new(output::chat::ChatOutput)),
        "anthropic" => Some(Box::new(output::anthropic::AnthropicOutput)),
        "responses" | "openai-responses" | "openai_responses" => Some(Box::new(output::responses::ResponsesOutput)),
        _ => None,
    }
}

/// Convenience: convert a Responses API request body (what Codex sends)
/// into an upstream request body for the given output protocol.
pub fn responses_to_upstream(body: &Value, output_protocol: &str) -> anyhow::Result<Value> {
    let input = get_input("responses").ok_or_else(|| anyhow::anyhow!("input protocol 'responses' not found"))?;
    let output = get_output(output_protocol).ok_or_else(|| anyhow::anyhow!("output protocol '{output_protocol}' not found"))?;
    let pair = ProtocolPair::new(input, output);
    pair.convert_request(body)
}

/// Convenience: convert an upstream response body (from the given protocol)
/// into a Responses API response body (what Codex expects).
pub fn upstream_to_responses(body: &Value, input_protocol: &str) -> anyhow::Result<Value> {
    let input = get_input(input_protocol);
    let output = get_output("responses").ok_or_else(|| anyhow::anyhow!("output protocol 'responses' not found"))?;

    match input {
        Some(inp) => {
            let pair = ProtocolPair::new(inp, output);
            pair.convert_response(body)
        }
        None => {
            if input_protocol == "chat_completions" || input_protocol == "openai-chat" || input_protocol == "chatCompletions" {
                let canonical = direct_parse_chat_response(body)?;
                output.build_response(&canonical)
            } else if input_protocol == "anthropic" {
                let canonical = direct_parse_anthropic_response(body)?;
                output.build_response(&canonical)
            } else {
                Err(anyhow::anyhow!("no input parser for protocol '{input_protocol}'"))
            }
        }
    }
}

// ── Direct response parsers (legacy, for protocols without dedicated InputProtocol) ──

/// Parse a Chat Completions response body directly into canonical format.
fn direct_parse_chat_response(body: &Value) -> anyhow::Result<ProtocolResponse> {
    let choices = body.get("choices").and_then(Value::as_array).ok_or_else(|| anyhow::anyhow!("missing choices"))?;
    let choice = choices.first().ok_or_else(|| anyhow::anyhow!("empty choices"))?;
    let msg = choice.get("message").ok_or_else(|| anyhow::anyhow!("missing message"))?;

    let mut output = Vec::new();
    if let Some(rc) = msg.get("reasoning_content").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        output.push(OutputItem::Reasoning { text: rc.to_string() });
    }
    let _content_text = msg.get("content").and_then(Value::as_str).unwrap_or("");
    if let Some(text) = msg.get("content").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        output.push(OutputItem::Message { role: "assistant".to_string(), content: vec![ContentPart::Text(text.to_string())] });
    } else if let Some(parts) = msg.get("content").and_then(Value::as_array) {
        let content: Vec<ContentPart> = parts.iter().filter_map(|p| {
            match p.get("type").and_then(Value::as_str).unwrap_or("") {
                "text" | "output_text" => p.get("text").and_then(Value::as_str).map(|t| ContentPart::Text(t.to_string())),
                "refusal" => p.get("refusal").and_then(Value::as_str).map(|r| ContentPart::Refusal(r.to_string())),
                _ => None,
            }
        }).collect();
        if !content.is_empty() { output.push(OutputItem::Message { role: "assistant".to_string(), content }); }
    }
    if let Some(tcs) = msg.get("tool_calls").and_then(Value::as_array) {
        for tc in tcs {
            if let Some(func) = tc.get("function") {
                output.push(OutputItem::ToolCall {
                    id: tc.get("id").and_then(Value::as_str).unwrap_or("call_0").to_string(),
                    name: func.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                    arguments: func.get("arguments").and_then(Value::as_str).unwrap_or("{}").to_string(),
                    tool_type: core::types::ToolType::Function,
                });
            }
        }
    }

    let usage = body.get("usage");
    Ok(ProtocolResponse {
        id: body.get("id").and_then(Value::as_str).unwrap_or("resp_compat").to_string(),
        model: body.get("model").and_then(Value::as_str).unwrap_or("").to_string(),
        created_at: body.get("created").and_then(Value::as_u64).unwrap_or(0),
        status: if choice.get("finish_reason").and_then(Value::as_str) == Some("length") {
            ResponseStatus::Incomplete
        } else { ResponseStatus::Completed },
        output,
        usage: core::types::Usage {
            input_tokens: usage.and_then(|u| u.get("prompt_tokens").and_then(Value::as_u64)).unwrap_or(0),
            output_tokens: usage.and_then(|u| u.get("completion_tokens").and_then(Value::as_u64)).unwrap_or(0),
            total_tokens: usage.and_then(|u| u.get("total_tokens").and_then(Value::as_u64)).unwrap_or(0),
            cached_input_tokens: None,
            extra: Vec::new(),
        },
        extra_fields: Vec::new(),
    })
}

/// Parse an Anthropic Messages response body directly into canonical format.
fn direct_parse_anthropic_response(body: &Value) -> anyhow::Result<ProtocolResponse> {
    let mut content = Vec::new();
    if let Some(blocks) = body.get("content").and_then(Value::as_array) {
        for block in blocks {
            match block.get("type").and_then(Value::as_str).unwrap_or("") {
                "text" => {
                    if let Some(text) = block.get("text").and_then(Value::as_str) {
                        content.push(ContentPart::Text(text.to_string()));
                    }
                }
                "thinking" | "reasoning" => {
                    if let Some(text) = block.get("thinking").and_then(Value::as_str).or_else(|| block.get("text").and_then(Value::as_str)) {
                        content.push(ContentPart::Thinking(text.to_string()));
                    }
                }
                "tool_use" => {
                    content.push(ContentPart::Text(format!(
                        "[tool_use: {}]({})",
                        block.get("name").and_then(Value::as_str).unwrap_or(""),
                        block.get("input").map(|v| v.to_string()).unwrap_or_default(),
                    )));
                }
                _ => {}
            }
        }
    }

    let usage_val = body.get("usage");
    Ok(ProtocolResponse {
        id: body.get("id").and_then(Value::as_str).unwrap_or("resp_unknown").to_string(),
        model: body.get("model").and_then(Value::as_str).unwrap_or("").to_string(),
        created_at: 0,
        status: ResponseStatus::Completed,
        output: vec![OutputItem::Message { role: "assistant".to_string(), content }],
        usage: core::types::Usage {
            input_tokens: usage_val.and_then(|u| u.get("input_tokens").and_then(Value::as_u64)).unwrap_or(0),
            output_tokens: usage_val.and_then(|u| u.get("output_tokens").and_then(Value::as_u64)).unwrap_or(0),
            total_tokens: 0,
            cached_input_tokens: None,
            extra: Vec::new(),
        },
        extra_fields: Vec::new(),
    })
}
