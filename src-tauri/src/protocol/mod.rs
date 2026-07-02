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
        passthrough: PassthroughFields::default(),
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
        passthrough: PassthroughFields::default(),
    })
}
// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper: sample Responses API request (what Codex sends) ──
    fn sample_responses_request() -> Value {
        serde_json::json!({
            "model": "gpt-5.5",
            "input": "ping",
            "instructions": "You are a helpful assistant.",
            "max_output_tokens": 1024,
            "temperature": 0.7,
            "stream": false
        })
    }

    fn sample_responses_request_multi() -> Value {
        serde_json::json!({
            "model": "gpt-5.5",
            "input": [
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "hello"}]},
                {"type": "message", "role": "assistant", "content": [{"type": "input_text", "text": "hi there"}]},
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "what is 2+2?"}]}
            ],
            "instructions": "Be concise.",
            "max_output_tokens": 512,
            "stream": true
        })
    }

    // ── Helper: sample Chat Completions response ──
    fn sample_chat_response() -> Value {
        serde_json::json!({
            "id": "chatcmpl_abc123",
            "object": "chat.completion",
            "created": 1715000000,
            "model": "gpt-5.5",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you today?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        })
    }

    fn sample_chat_response_with_reasoning() -> Value {
        serde_json::json!({
            "id": "chatcmpl_def456",
            "object": "chat.completion",
            "created": 1715000001,
            "model": "gpt-5.5",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "The answer is 4.",
                    "reasoning_content": "Let me calculate: 2 + 2 = 4"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 15,
                "completion_tokens": 12,
                "total_tokens": 27
            }
        })
    }

    fn sample_chat_response_with_tools() -> Value {
        serde_json::json!({
            "id": "chatcmpl_tool123",
            "object": "chat.completion",
            "created": 1715000002,
            "model": "gpt-5.5",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\": \"Beijing\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 20,
                "completion_tokens": 15,
                "total_tokens": 35
            }
        })
    }

    // ── Helper: sample Anthropic response ──
    fn sample_anthropic_response() -> Value {
        serde_json::json!({
            "id": "msg_01abcd",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-opus",
            "content": [{"type": "text", "text": "Hello! I am Claude."}],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 8
            }
        })
    }

    // ── Test 1: Responses → Chat request conversion ──
    #[test]
    fn test_responses_to_chat_request() {
        let req = sample_responses_request();
        let result = responses_to_upstream(&req, "chat_completions");
        assert!(result.is_ok(), "Conversion should succeed: {:?}", result.err());

        let chat = result.unwrap();

        // Model should pass through
        assert_eq!(chat["model"].as_str(), Some("gpt-5.5"));

        // System instructions should be in messages[0]
        let msgs = chat["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"].as_str(), Some("system"));
        assert!(msgs[0]["content"].as_str().unwrap().contains("helpful assistant"));

        // User message should be messages[1]
        assert_eq!(msgs[1]["role"].as_str(), Some("user"));
        assert_eq!(msgs[1]["content"].as_str(), Some("ping"));

        // Parameters
        assert_eq!(chat["max_tokens"].as_u64(), Some(1024));
        let temp = chat["temperature"].as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.01, "temperature should be ~0.7, got {}", temp);
        assert_eq!(chat["stream"].as_bool(), Some(false));
    }

    // ── Test 2: Responses → Chat request with multi-message input ──
    #[test]
    fn test_responses_to_chat_multi_message() {
        let req = sample_responses_request_multi();
        let result = responses_to_upstream(&req, "chat_completions");
        assert!(result.is_ok(), "Multi-message conversion failed: {:?}", result.err());

        let chat = result.unwrap();
        let msgs = chat["messages"].as_array().unwrap();

        // system + 3 messages (user, assistant, user) = 4
        assert_eq!(msgs.len(), 4, "Should have system + 3 messages");
        assert_eq!(msgs[0]["role"].as_str(), Some("system"));
        assert_eq!(msgs[1]["role"].as_str(), Some("user"));
        assert_eq!(msgs[2]["role"].as_str(), Some("assistant"));
        assert_eq!(msgs[3]["role"].as_str(), Some("user"));

        assert_eq!(chat["stream"].as_bool(), Some(true));
        assert!(chat["stream_options"].is_object());
    }

    // ── Test 3: Chat response → Responses conversion (basic text) ──
    #[test]
    fn test_chat_to_responses_text() {
        let resp = sample_chat_response();
        let result = upstream_to_responses(&resp, "chat_completions");
        assert!(result.is_ok(), "Chat→Responses failed: {:?}", result.err());

        let r = result.unwrap();
        assert_eq!(r["model"].as_str(), Some("gpt-5.5"));
        assert_eq!(r["status"].as_str(), Some("completed"));

        // Should have one output message
        let output = r["output"].as_array().unwrap();
        assert_eq!(output.len(), 1, "Should have 1 output item");

        // Check usage passthrough
        let usage = r["usage"].as_object().unwrap();
        assert_eq!(usage["input_tokens"].as_u64(), Some(10));
        assert_eq!(usage["output_tokens"].as_u64(), Some(8));
        assert_eq!(usage["total_tokens"].as_u64(), Some(18));
    }

    // ── Test 4: Chat response with reasoning ──
    #[test]
    fn test_chat_to_responses_reasoning() {
        let resp = sample_chat_response_with_reasoning();
        let result = upstream_to_responses(&resp, "chat_completions");
        assert!(result.is_ok(), "Reasoning conversion failed: {:?}", result.err());

        let r = result.unwrap();
        let output = r["output"].as_array().unwrap();
        // Should have at least 1 output item (message or reasoning)
        assert!(!output.is_empty(), "Should have output items");
        // Check that reasoning content appears somewhere in the output
        let all_text: String = output.iter().map(|item| format!("{:?}", item)).collect();
        assert!(all_text.contains("4"), "Output should contain the answer");
    }

    // ── Test 5: Chat response with tool calls ──
    #[test]
    fn test_chat_to_responses_tools() {
        let resp = sample_chat_response_with_tools();
        let result = upstream_to_responses(&resp, "chat_completions");
        assert!(result.is_ok(), "Tool conversion failed: {:?}", result.err());

        let r = result.unwrap();
        let output = r["output"].as_array().unwrap();

        // Should have at least one output item (function call or message)
        assert!(!output.is_empty(), "Should have output items for tool response");
        // The output should relate to get_weather somehow
        let all_text: String = output.iter().map(|item| format!("{:?}", item)).collect();
        assert!(all_text.contains("weather") || all_text.contains("get_weather") || all_text.contains("Beijing"),
            "Output should contain tool info: {}", all_text);
    }

    // ── Test 6: Responses → Anthropic request conversion ──
    #[test]
    fn test_responses_to_anthropic_request() {
        let req = sample_responses_request_multi();
        let result = responses_to_upstream(&req, "anthropic");
        assert!(result.is_ok(), "Responses→Anthropic failed: {:?}", result.err());

        let anth = result.unwrap();

        // Anthropic-specific fields
        assert!(anth.get("max_tokens").is_some());
        assert!(anth.get("system").is_some());

        let msgs = anth["messages"].as_array().unwrap();
        assert!(!msgs.is_empty(), "Should have messages");
        assert_eq!(msgs[0]["role"].as_str(), Some("user"));
    }

    // ── Test 7: Anthropic response → Responses conversion ──
    #[test]
    fn test_anthropic_to_responses() {
        let resp = sample_anthropic_response();
        let result = upstream_to_responses(&resp, "anthropic");
        assert!(result.is_ok(), "Anthropic→Responses failed: {:?}", result.err());

        let r = result.unwrap();
        assert_eq!(r["status"].as_str(), Some("completed"));
        let output = r["output"].as_array().unwrap();
        assert!(!output.is_empty(), "Should have output items");
    }

    // ── Test 8: Passthrough fields are forwarded to Chat request ──
    #[test]
    fn test_passthrough_fields_to_chat() {
        let req = serde_json::json!({
            "model": "gpt-4",
            "input": "hello",
            "user": "test-user-123",
            "seed": 42,
            "stop": ["END"],
            "response_format": {"type": "json_object"},
            "frequency_penalty": 0.5,
            "presence_penalty": 0.3
        });
        let result = responses_to_upstream(&req, "chat_completions");
        assert!(result.is_ok(), "Passthrough conversion failed: {:?}", result.err());

        let chat = result.unwrap();
        assert_eq!(chat["user"].as_str(), Some("test-user-123"));
        assert_eq!(chat["seed"].as_u64(), Some(42));
        assert_eq!(chat["stop"].as_array().unwrap()[0].as_str(), Some("END"));
        assert_eq!(chat["response_format"]["type"].as_str(), Some("json_object"));
        assert!((chat["frequency_penalty"].as_f64().unwrap() - 0.5).abs() < 0.01);
        assert!((chat["presence_penalty"].as_f64().unwrap() - 0.3).abs() < 0.01);
    }

    // ── Test 9: Tool definitions are forwarded to Chat request ──
    #[test]
    fn test_tool_definitions_to_chat() {
        let req = serde_json::json!({
            "model": "gpt-4",
            "input": "what is the weather?",
            "tools": [{
                "type": "function",
                "name": "get_weather",
                "description": "Get weather for a location",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    },
                    "required": ["location"]
                }
            }],
            "tool_choice": {"type": "function", "function": {"name": "get_weather"}}
        });
        let result = responses_to_upstream(&req, "chat_completions");
        assert!(result.is_ok(), "Tool conversion failed: {:?}", result.err());

        let chat = result.unwrap();
        let tools = chat["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["function"]["name"].as_str(), Some("get_weather"));
        assert!(chat["tool_choice"].is_object());
    }

    // ── Test 10: Error on unknown protocol ──
    #[test]
    fn test_unknown_protocol_error() {
        let req = sample_responses_request();
        let result = responses_to_upstream(&req, "unknown_protocol");
        assert!(result.is_err(), "Should fail for unknown protocol");

        let result2 = upstream_to_responses(&req, "unknown_protocol");
        assert!(result2.is_err(), "Should fail for unknown protocol");
    }

    // ── Test 11: Chat response with incomplete status (finish_reason = length) ──
    #[test]
    fn test_chat_incomplete_response() {
        let resp = serde_json::json!({
            "id": "chatcmpl_incomplete",
            "object": "chat.completion",
            "created": 1715000003,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Partial response"},
                "finish_reason": "length"
            }],
            "usage": {"prompt_tokens": 5, "completion_tokens": 100, "total_tokens": 105}
        });
        let result = upstream_to_responses(&resp, "chat_completions");
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["status"].as_str(), Some("incomplete"));
    }

    // ── Test 12: Empty body handling ──
    #[test]
    fn test_empty_input() {
        let req = serde_json::json!({
            "model": "gpt-4",
            "input": ""
        });
        let result = responses_to_upstream(&req, "chat_completions");
        assert!(result.is_ok());
    }

    // ── Test 13: Responses → Anthropic passthrough fields ──
    #[test]
    fn test_anthropic_request_fields() {
        let req = serde_json::json!({
            "model": "claude-sonnet",
            "input": "hello",
            "max_output_tokens": 2048,
            "temperature": 0.3
        });
        let result = responses_to_upstream(&req, "anthropic");
        assert!(result.is_ok(), "Anthropic req conversion failed: {:?}", result.err());

        let anth = result.unwrap();
        assert_eq!(anth["max_tokens"].as_u64(), Some(2048));
        assert!((anth["temperature"].as_f64().unwrap() - 0.3).abs() < 0.01);
        assert_eq!(anth["stream"].as_bool(), Some(false));
    }

    // ── Test 14: Direct passthrough for responses → responses ──
    #[test]
    fn test_responses_direct_passthrough() {
        let req = sample_responses_request();
        let result = responses_to_upstream(&req, "responses");
        assert!(result.is_ok());

        let r = result.unwrap();
        // Should preserve original fields since it's direct passthrough
        assert_eq!(r["model"].as_str(), Some("gpt-5.5"));
    }
}

