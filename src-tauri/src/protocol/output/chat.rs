// ── Canonical → Chat Completions ──

use serde_json::Value;
use crate::protocol::core::types::*;
use crate::protocol::core::traits::OutputProtocol;
use crate::protocol::reasoning::styles::apply_reasoning_options;

/// Builds OpenAI Chat Completions wire format from canonical ProtocolRequest/ProtocolResponse.
pub struct ChatOutput;

impl OutputProtocol for ChatOutput {
    fn name(&self) -> &str { "chat_completions" }

    fn build_request(&self, req: &ProtocolRequest) -> anyhow::Result<Value> {
        let mut result = serde_json::json!({});
        result["model"] = Value::String(req.model.clone());

        let mut messages = Vec::new();

        let mut system_parts = Vec::new();
        if let Some(ref sys) = req.system {
            if !sys.is_empty() { system_parts.push(sys.clone()); }
        }
        let mut rest_messages = Vec::new();
        for msg in &req.messages {
            match msg.role {
                MessageRole::System => {
                    let text = content_parts_to_text(&msg.content);
                    if !text.is_empty() { system_parts.push(text); }
                }
                _ => rest_messages.push(msg),
            }
        }

        if !system_parts.is_empty() {
            messages.push(serde_json::json!({"role": "system", "content": system_parts.join("\n\n")}));
        }

        for msg in &rest_messages {
            let role = match msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };
            let content = build_chat_content(msg);
            let mut entry = serde_json::json!({"role": role, "content": content});

            if msg.role == MessageRole::Assistant {
                if let Some(ref rc) = msg.reasoning_content {
                    if !rc.is_empty() { entry["reasoning_content"] = Value::String(rc.clone()); }
                }
                if !msg.tool_calls.is_empty() {
                    let tcs: Vec<Value> = msg.tool_calls.iter().map(|tc| {
                        serde_json::json!({
                            "id": tc.id, "type": "function",
                            "function": {"name": tc.name, "arguments": tc.arguments}
                        })
                    }).collect();
                    entry["tool_calls"] = Value::Array(tcs);
                }
            }
            if msg.role == MessageRole::Tool {
                if let Some(ref cid) = msg.tool_call_id {
                    entry["tool_call_id"] = Value::String(cid.clone());
                }
            }
            messages.push(entry);
        }

        for msg in &mut messages {
            if msg.get("role").and_then(Value::as_str) == Some("assistant") {
                let has_content = msg.get("content").and_then(|c| c.as_str()).map(|s| !s.is_empty()).unwrap_or(false);
                let has_tc = msg.get("tool_calls").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
                if !has_content && has_tc { msg["content"] = Value::String(String::new()); }
            }
        }

        result["messages"] = Value::Array(messages);

        if let Some(maxt) = req.max_tokens { result["max_tokens"] = Value::Number(serde_json::Number::from(maxt)); }
        if let Some(temp) = req.temperature { result["temperature"] = Value::Number(serde_json::Number::from_f64(temp as f64).unwrap()); }
        if let Some(top) = req.top_p { result["top_p"] = Value::Number(serde_json::Number::from_f64(top as f64).unwrap()); }
        result["stream"] = Value::Bool(req.stream);

        if req.stream {
            let mut opts = req.stream_options.clone().unwrap_or_else(|| serde_json::json!({}));
            opts["include_usage"] = Value::Bool(true);
            result["stream_options"] = opts;
        }

        apply_reasoning_options(&mut result, req);

        if !req.tools.is_empty() {
            let chat_tools: Vec<Value> = req.tools.iter().map(build_chat_tool).collect();
            result["tools"] = Value::Array(chat_tools);
        }
        if let Some(ref tc) = req.tool_choice {
            result["tool_choice"] = tc.clone();
        }
        if req.tools.iter().any(|t| matches!(t.tool_type, ToolType::Function)) {
            if let Some(ref tc) = req.tool_choice {
                result["tool_choice"] = tc.clone();
            }
        }

        // Explicitly forward known-safe passthrough fields
        if let Some(ref user) = req.passthrough.user {
            result["user"] = Value::String(user.clone());
        }
        if let Some(seed) = req.passthrough.seed {
            result["seed"] = Value::Number(serde_json::Number::from(seed));
        }
        if let Some(ref stop) = req.passthrough.stop {
            result["stop"] = stop.clone();
        }
        if let Some(ref rf) = req.passthrough.response_format {
            result["response_format"] = rf.clone();
        }
        if let Some(fp) = req.passthrough.frequency_penalty {
            result["frequency_penalty"] = Value::Number(serde_json::Number::from_f64(fp as f64).unwrap());
        }
        if let Some(pp) = req.passthrough.presence_penalty {
            result["presence_penalty"] = Value::Number(serde_json::Number::from_f64(pp as f64).unwrap());
        }

        Ok(result)
    }

    fn build_response(&self, resp: &ProtocolResponse) -> anyhow::Result<Value> {
        let mut output_items = Vec::new();
        let mut tool_calls = Vec::new();
        let mut reasoning_text = String::new();

        for item in &resp.output {
            match item {
                OutputItem::Message { ref content, .. } => {
                    let text: Vec<String> = content.iter()
                        .filter_map(|p| if let ContentPart::Text(t) = p { Some(t.clone()) } else { None })
                        .collect();
                    output_items.push(text.join(""));
                }
                OutputItem::Reasoning { ref text } => {
                    reasoning_text.push_str(text);
                    reasoning_text.push('\n');
                }
                OutputItem::ToolCall { ref id, ref name, ref arguments, .. } => {
                    tool_calls.push(serde_json::json!({
                        "id": id, "type": "function",
                        "function": {"name": name, "arguments": arguments}
                    }));
                }
                OutputItem::CustomToolCall { .. } => {
                    output_items.push("[custom_tool_call]".to_string());
                }
            }
        }

        let content = if output_items.is_empty() { "" } else { &output_items.join("\n") };
        let mut message = serde_json::json!({"role": "assistant", "content": content});
        if !reasoning_text.trim().is_empty() {
            message["reasoning_content"] = Value::String(reasoning_text.trim().to_string());
        }
        if !tool_calls.is_empty() {
            message["tool_calls"] = Value::Array(tool_calls);
        }

        let finish_reason = match resp.status {
            ResponseStatus::Incomplete => "length",
            _ => "stop",
        };

        Ok(serde_json::json!({
            "id": format!("chatcmpl_{}", &resp.id[..16.min(resp.id.len())]),
            "object": "chat.completion",
            "created": resp.created_at,
            "model": resp.model,
            "choices": [{
                "index": 0,
                "message": message,
                "finish_reason": finish_reason,
            }],
            "usage": {
                "prompt_tokens": resp.usage.input_tokens,
                "completion_tokens": resp.usage.output_tokens,
                "total_tokens": resp.usage.total_tokens,
            }
        }))
    }
}

// ── Helpers ──

/// Build the content field of a Chat message, handling multi-part content.
fn build_chat_content(msg: &ProtocolMessage) -> Value {
    let has_non_text = msg.content.iter().any(|p| !matches!(p, ContentPart::Text(_)));
    if !has_non_text {
        let text: String = msg.content.iter()
            .filter_map(|p| if let ContentPart::Text(t) = p { Some(t.as_str()) } else { None })
            .collect::<Vec<_>>().join("\n");
        return Value::String(text);
    }
    let parts: Vec<Value> = msg.content.iter().map(|p| match p {
        ContentPart::Text(t) => serde_json::json!({"type": "text", "text": t}),
        ContentPart::Image { url, detail } => {
            let mut img = serde_json::json!({"type": "image_url", "image_url": {"url": url}});
            if let Some(d) = detail { img["image_url"]["detail"] = Value::String(d.clone()); }
            img
        }
        ContentPart::File { data, filename } => {
            serde_json::json!({"type": "text", "text": format!("[{filename} data: {data}]")})
        }
        ContentPart::Refusal(r) => serde_json::json!({"type": "text", "text": r}),
        ContentPart::Thinking(t) => serde_json::json!({"type": "text", "text": t}),
        ContentPart::InputAudio { data, format } => {
            serde_json::json!({"type": "input_audio", "data": data, "format": format})
        }
    }).collect();
    Value::Array(parts)
}

/// Build a Chat Completions tool definition from canonical form.
fn build_chat_tool(tool: &ToolDefinition) -> Value {
    match &tool.tool_type {
        ToolType::Function => {
            let mut t = serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description.clone().unwrap_or_default(),
                    "parameters": tool.parameters,
                }
            });
            if let Some(strict) = tool.strict {
                t["function"]["strict"] = Value::Bool(strict);
            }
            t
        }
        ToolType::Custom(_) | ToolType::WebSearch | ToolType::LocalShell | ToolType::ComputerUse => {
            let desc = tool.description.clone().unwrap_or_else(|| format!("FREEFORM custom tool: {}. Put only the tool input text here.", tool.name));
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": desc,
                    "parameters": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "input": {
                                "type": "string",
                                "description": "Raw freeform input for this custom tool."
                            }
                        },
                        "required": ["input"]
                    }
                }
            })
        }
        ToolType::Namespace(children) => {
            let _tools: Vec<Value> = children.iter().map(build_chat_tool).collect();
            serde_json::json!({
                "type": "function",
                "function": { "name": tool.name, "parameters": tool.parameters }
            })
        }
    }
}

/// Concatenate text content parts into a single string.
fn content_parts_to_text(parts: &[ContentPart]) -> String {
    parts.iter()
        .filter_map(|p| if let ContentPart::Text(t) = p { Some(t.as_str()) } else { None })
        .collect::<Vec<_>>().join("\n")
}
