// ── Canonical → Responses API ──

use serde_json::Value;
use crate::protocol::core::types::*;
use crate::protocol::core::traits::OutputProtocol;

/// Builds OpenAI Responses API wire format from canonical ProtocolRequest/ProtocolResponse.
pub struct ResponsesOutput;

impl OutputProtocol for ResponsesOutput {
    fn name(&self) -> &str { "responses" }

    fn build_request(&self, req: &ProtocolRequest) -> anyhow::Result<Value> {
        let mut result = serde_json::json!({
            "model": req.model,
            "stream": req.stream,
        });

        if let Some(ref sys) = req.system {
            result["instructions"] = Value::String(sys.clone());
        }

        let mut input = Vec::new();
        for msg in &req.messages {
            let role = match msg.role {
                MessageRole::System => "developer",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };

            if msg.role == MessageRole::Tool {
                let text = content_to_text(&msg.content);
                input.push(serde_json::json!({
                    "type": "function_call_output",
                    "call_id": msg.tool_call_id.clone().unwrap_or_default(),
                    "output": text,
                }));
                continue;
            }

            let content: Vec<Value> = msg.content.iter().map(|p| match p {
                ContentPart::Text(t) => serde_json::json!({"type": "input_text", "text": t}),
                ContentPart::Image { url, .. } => {
                    serde_json::json!({"type": "input_image", "image_url": url})
                }
                ContentPart::File { data, filename } => {
                    serde_json::json!({"type": "input_file", "file_data": data, "filename": filename})
                }
                ContentPart::Refusal(r) => serde_json::json!({"type": "input_text", "text": r}),
                ContentPart::Thinking(t) => serde_json::json!({"type": "thinking", "thinking": t}),
                ContentPart::InputAudio { data, format } => {
                    serde_json::json!({"type": "input_audio", "data": data, "format": format})
                }
            }).collect();

            let mut entry = serde_json::json!({
                "type": "message",
                "role": role,
                "content": content,
            });

            if msg.role == MessageRole::Assistant {
                if !msg.tool_calls.is_empty() {
                    entry["tool_calls"] = Value::Array(msg.tool_calls.iter().map(|tc| {
                        serde_json::json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {"name": tc.name, "arguments": tc.arguments}
                        })
                    }).collect());
                }
                if let Some(ref rc) = msg.reasoning_content {
                    entry["reasoning_content"] = Value::String(rc.clone());
                }
            }

            input.push(entry);
        }

        if !input.is_empty() {
            result["input"] = Value::Array(input);
        }

        if let Some(maxt) = req.max_tokens { result["max_output_tokens"] = Value::Number(serde_json::Number::from(maxt)); }
        if let Some(temp) = req.temperature { result["temperature"] = Value::Number(serde_json::Number::from_f64(temp as f64).unwrap()); }
        if let Some(top) = req.top_p { result["top_p"] = Value::Number(serde_json::Number::from_f64(top as f64).unwrap()); }
        if let Some(ref reasoning) = req.reasoning {
            if reasoning.effort.is_some() {
                result["reasoning"] = serde_json::json!({"effort": reasoning.effort});
            }
        }
        if !req.tools.is_empty() {
            result["tools"] = Value::Array(req.tools.iter().map(build_responses_tool).collect());
        }
        if let Some(ref tc) = req.tool_choice { result["tool_choice"] = tc.clone(); }
        if let Some(ref meta) = req.metadata { result["metadata"] = meta.clone(); }

        Ok(result)
    }

    fn build_response(&self, resp: &ProtocolResponse) -> anyhow::Result<Value> {
        let mut output = Vec::new();
        let mut current_msg_content = Vec::new();
        let mut current_tool_calls = Vec::new();
        let mut current_role = String::new();

        let mut flush_message = |role: &str, content: &mut Vec<Value>, tcs: &mut Vec<Value>, out: &mut Vec<Value>| {
            if !content.is_empty() || !tcs.is_empty() {
                let mut item = serde_json::json!({
                    "type": "message",
                    "role": role,
                    "content": std::mem::take(content),
                });
                if !tcs.is_empty() {
                    item["tool_calls"] = Value::Array(std::mem::take(tcs));
                }
                out.push(item);
            }
        };

        for item in &resp.output {
            match item {
                OutputItem::Message { ref role, content: ref parts } => {
                    flush_message(&current_role, &mut current_msg_content, &mut current_tool_calls, &mut output);
                    current_role = role.clone();
                    for part in parts {
                        match part {
                            ContentPart::Text(t) => current_msg_content.push(serde_json::json!({"type": "output_text", "text": t, "annotations": []})),
                            ContentPart::Refusal(r) => current_msg_content.push(serde_json::json!({"type": "refusal", "refusal": r})),
                            ContentPart::Thinking(t) => current_msg_content.push(serde_json::json!({"type": "thinking", "thinking": t})),
                            _ => {}
                        }
                    }
                }
                OutputItem::Reasoning { ref text } => {
                    current_msg_content.push(serde_json::json!({"type": "thinking", "thinking": text}));
                }
                OutputItem::ToolCall { ref id, ref name, ref arguments, .. } => {
                    current_tool_calls.push(serde_json::json!({
                        "id": id, "type": "function",
                        "function": {"name": name, "arguments": arguments}
                    }));
                }
                OutputItem::CustomToolCall { ref id, ref name, ref input } => {
                    flush_message(&current_role, &mut current_msg_content, &mut current_tool_calls, &mut output);
                    output.push(serde_json::json!({
                        "id": format!("ctc_{id}"),
                        "type": "custom_tool_call",
                        "status": "completed",
                        "call_id": id,
                        "name": name,
                        "input": input,
                    }));
                }
            }
        }
        flush_message(&current_role, &mut current_msg_content, &mut current_tool_calls, &mut output);

        let status = match resp.status {
            ResponseStatus::InProgress => "in_progress",
            ResponseStatus::Completed => "completed",
            ResponseStatus::Incomplete => "incomplete",
            ResponseStatus::Failed => "failed",
        };

        let mut result = serde_json::json!({
            "id": resp.id,
            "object": "response",
            "created_at": resp.created_at,
            "status": status,
            "model": resp.model,
            "output": output,
            "usage": {
                "input_tokens": resp.usage.input_tokens,
                "output_tokens": resp.usage.output_tokens,
                "total_tokens": resp.usage.total_tokens,
            }
        });

        if let Some(cached) = resp.usage.cached_input_tokens {
            if cached > 0 {
                result["usage"]["input_tokens_details"] = serde_json::json!({"cached_tokens": cached});
            }
        }

        Ok(result)
    }
}

/// Build a Responses API tool definition from canonical form.
fn build_responses_tool(tool: &ToolDefinition) -> Value {
    match &tool.tool_type {
        ToolType::Function => {
            let mut t = serde_json::json!({
                "type": "function",
                "name": tool.name,
                "description": tool.description.clone().unwrap_or_default(),
                "parameters": tool.parameters,
            });
            if let Some(strict) = tool.strict { t["strict"] = Value::Bool(strict); }
            t
        }
        ToolType::Custom(_) | ToolType::WebSearch | ToolType::LocalShell | ToolType::ComputerUse => {
            serde_json::json!({"type": "custom", "name": tool.name, "description": tool.description.clone().unwrap_or_default()})
        }
        ToolType::Namespace(children) => {
            serde_json::json!({
                "type": "namespace",
                "name": tool.name,
                "description": tool.description.clone().unwrap_or_default(),
                "tools": children.iter().map(build_responses_tool).collect::<Vec<_>>(),
            })
        }
    }
}

/// Extract text from content parts joined by newlines.
fn content_to_text(parts: &[ContentPart]) -> String {
    parts.iter()
        .filter_map(|p| if let ContentPart::Text(t) = p { Some(t.as_str()) } else { None })
        .collect::<Vec<_>>().join("\n")
}
