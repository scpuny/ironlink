// ── Canonical → Anthropic Messages API ──

use serde_json::Value;
use crate::protocol::core::types::*;
use crate::protocol::core::traits::OutputProtocol;

/// Builds Anthropic Messages API wire format from canonical ProtocolRequest/ProtocolResponse.
pub struct AnthropicOutput;

impl OutputProtocol for AnthropicOutput {
    fn name(&self) -> &str { "anthropic" }

    fn build_request(&self, req: &ProtocolRequest) -> anyhow::Result<Value> {
        let mut result = serde_json::json!({
            "model": req.model,
            "max_tokens": req.max_tokens.unwrap_or(4096),
            "stream": req.stream,
        });

        let mut system_parts = Vec::new();
        if let Some(ref sys) = req.system {
            if !sys.is_empty() { system_parts.push(sys.clone()); }
        }
        for msg in &req.messages {
            if msg.role == MessageRole::System {
                let text = content_to_text(&msg.content);
                if !text.is_empty() { system_parts.push(text); }
            }
        }
        if !system_parts.is_empty() {
            result["system"] = Value::String(system_parts.join("\n\n"));
        }

        let messages: Vec<Value> = req.messages.iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| {
                let role = match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Tool => "user", // Anthropic uses user role for tool results
                    MessageRole::System => "user",
                };
                let mut msg = serde_json::json!({
                    "role": role,
                    "content": build_anthropic_content(m),
                });
                msg
            })
            .collect();
        result["messages"] = Value::Array(messages);

        if let Some(temp) = req.temperature {
            result["temperature"] = Value::Number(serde_json::Number::from_f64(temp as f64).unwrap());
        }

        Ok(result)
    }

    fn build_response(&self, resp: &ProtocolResponse) -> anyhow::Result<Value> {
        let mut content = Vec::new();
        for item in &resp.output {
            match item {
                OutputItem::Message { content: ref parts, .. } => {
                    for part in parts {
                        match part {
                            ContentPart::Text(t) => content.push(serde_json::json!({
                                "type": "text",
                                "text": t,
                            })),
                            ContentPart::Thinking(t) => content.push(serde_json::json!({
                                "type": "thinking",
                                "thinking": t,
                            })),
                            _ => {}
                        }
                    }
                }
                OutputItem::Reasoning { ref text } => {
                    content.push(serde_json::json!({
                        "type": "thinking",
                        "thinking": text,
                    }));
                }
                OutputItem::ToolCall { ref id, ref name, ref arguments, .. } => {
                    content.push(serde_json::json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": serde_json::from_str::<Value>(arguments).unwrap_or(Value::Object(serde_json::Map::new())),
                    }));
                }
                OutputItem::CustomToolCall { .. } => {
                    content.push(serde_json::json!({
                        "type": "text",
                        "text": "[custom_tool_call]",
                    }));
                }
            }
        }

        let stop_reason = match resp.status {
            ResponseStatus::Incomplete => Some("max_tokens"),
            ResponseStatus::Failed => Some("error"),
            _ => Some("end_turn"),
        };

        Ok(serde_json::json!({
            "id": resp.id,
            "type": "message",
            "role": "assistant",
            "content": content,
            "model": resp.model,
            "stop_reason": stop_reason,
            "stop_sequence": Value::Null,
            "usage": {
                "input_tokens": resp.usage.input_tokens,
                "output_tokens": resp.usage.output_tokens,
            }
        }))
    }
}

/// Build the content array of an Anthropic message.
fn build_anthropic_content(msg: &ProtocolMessage) -> Value {
    let parts: Vec<Value> = msg.content.iter().map(|p| match p {
        ContentPart::Text(t) => serde_json::json!({"type": "text", "text": t}),
        ContentPart::Image { url, .. } => serde_json::json!({
            "type": "image",
            "source": {"type": "base64", "media_type": "image/jpeg", "data": url},
        }),
        ContentPart::File { data, filename } => serde_json::json!({
            "type": "text",
            "text": format!("[{filename} data: {data}]"),
        }),
        ContentPart::Refusal(r) => serde_json::json!({"type": "text", "text": r}),
        ContentPart::Thinking(t) => serde_json::json!({"type": "thinking", "thinking": t}),
        ContentPart::InputAudio { data, format } => serde_json::json!({
            "type": "text",
            "text": format!("[audio {format} data: {data}]"),
        }),
    }).collect();

    if parts.is_empty() {
        Value::String(String::new())
    } else if parts.len() == 1 && parts[0].get("type").and_then(Value::as_str) == Some("text") {
        parts[0]["text"].clone()
    } else {
        Value::Array(parts)
    }
}

/// Extract text from content parts joined by newlines.
fn content_to_text(parts: &[ContentPart]) -> String {
    parts.iter()
        .filter_map(|p| if let ContentPart::Text(t) = p { Some(t.as_str()) } else { None })
        .collect::<Vec<_>>().join("\n")
}
