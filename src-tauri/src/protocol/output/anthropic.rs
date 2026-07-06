// ── Canonical → Anthropic Messages API ──
//
// Handles all critical conversions:
//   - Tools: Responses tools → Anthropic input_schema format
//   - Tool_choice: Responses tool_choice → Anthropic tool_choice
//   - Thinking: Responses reasoning → Anthropic thinking config
//   - Messages: tool_result / tool_use content blocks
//   - Response: Anthropic response → Responses output format

use serde_json::{json, Value};
use crate::protocol::core::types::*;
use crate::protocol::core::traits::OutputProtocol;

/// Builds Anthropic Messages API wire format from canonical ProtocolRequest/ProtocolResponse.
pub struct AnthropicOutput;

impl OutputProtocol for AnthropicOutput {
    fn name(&self) -> &str { "anthropic" }

    fn build_request(&self, req: &ProtocolRequest) -> anyhow::Result<Value> {
        let mut result = serde_json::json!({
            "model": req.model,
            "stream": req.stream,
        });

        // ── max_tokens ──
        // Anthropic requires max_tokens. If thinking is enabled, it also needs budget_tokens.
        result["max_tokens"] = json!(req.max_tokens.unwrap_or(4096));

        // ── System ──
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

        // ── Messages ──
        let messages: Vec<Value> = req.messages.iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| build_anthropic_message(m))
            .collect();
        result["messages"] = Value::Array(messages);

        // ── Temperature ──
        if let Some(temp) = req.temperature {
            result["temperature"] = json!(temp);
        }

        // ── Top P ──
        if let Some(top_p) = req.top_p {
            result["top_p"] = json!(top_p);
        }

        // ── Tools ──
        if !req.tools.is_empty() {
            let anthropic_tools: Vec<Value> = req.tools.iter()
                .filter_map(build_anthropic_tool)
                .collect();
            if !anthropic_tools.is_empty() {
                result["tools"] = Value::Array(anthropic_tools);
            }
        }

        // ── Tool_choice ──
        if let Some(ref tc) = req.tool_choice {
            if let Some(anthropic_tc) = convert_tool_choice(tc) {
                result["tool_choice"] = anthropic_tc;
            }
        }

        // ── Thinking (Anthropic extended thinking) ──
        if let Some(ref reasoning) = req.reasoning {
            if reasoning.enabled {
                let budget = req.max_tokens.map(|t| (t as f64 * 0.8) as u32).unwrap_or(16000).max(1024);
                result["thinking"] = json!({
                    "type": "enabled",
                    "budget_tokens": budget,
                });
            }
        }

        // ── Passthrough ──
        if let Some(ref stop) = req.passthrough.stop_sequences {
            if !stop.is_empty() {
                result["stop_sequences"] = json!(stop);
            }
        }
        if let Some(ref meta) = req.metadata {
            result["metadata"] = meta.clone();
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
                            ContentPart::Text(t) => content.push(json!({
                                "type": "text", "text": t,
                            })),
                            ContentPart::Thinking(t) => content.push(json!({
                                "type": "thinking", "thinking": t,
                            })),
                            _ => {}
                        }
                    }
                }
                OutputItem::Reasoning { ref text } => {
                    content.push(json!({
                        "type": "thinking", "thinking": text,
                    }));
                }
                OutputItem::ToolCall { ref id, ref name, ref arguments, .. } => {
                    let input: Value = serde_json::from_str(arguments)
                        .unwrap_or_else(|_| json!({"raw": arguments}));
                    content.push(json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": input,
                    }));
                }
                OutputItem::CustomToolCall { .. } => {
                    content.push(json!({
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

        Ok(json!({
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

// ── Build an Anthropic message from canonical form ──

fn build_anthropic_message(msg: &ProtocolMessage) -> Value {
    match msg.role {
        MessageRole::Tool => build_tool_result_message(msg),
        MessageRole::Assistant => build_assistant_message(msg),
        _ => build_user_message(msg),
    }
}

// ── User message ──

fn build_user_message(msg: &ProtocolMessage) -> Value {
    json!({
        "role": "user",
        "content": build_anthropic_content_blocks(&msg.content, None, None),
    })
}

// ── Tool result message (role: user, content: [{type: "tool_result", ...}]) ──

fn build_tool_result_message(msg: &ProtocolMessage) -> Value {
    let tool_use_id = msg.tool_call_id.as_deref().unwrap_or("");
    let text = content_to_text(&msg.content);
    json!({
        "role": "user",
        "content": [{
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": text
        }]
    })
}

// ── Assistant message (may contain text + tool_use blocks) ──

fn build_assistant_message(msg: &ProtocolMessage) -> Value {
    let mut content_blocks: Vec<Value> = Vec::new();

    // Add text content
    let text_content = build_anthropic_content_blocks(&msg.content, None, None);
    match text_content {
        Value::String(s) if !s.is_empty() => {
            content_blocks.push(json!({"type": "text", "text": s}));
        }
        Value::Array(arr) => content_blocks.extend(arr),
        _ => {}
    }

    // Add reasoning_content if present (as thinking block)
    if let Some(ref rc) = msg.reasoning_content {
        if !rc.is_empty() {
            content_blocks.push(json!({"type": "thinking", "thinking": rc}));
        }
    }

    // Add tool_calls as tool_use blocks
    for tc in &msg.tool_calls {
        let input: Value = serde_json::from_str(&tc.arguments)
            .unwrap_or_else(|_| json!({"raw": tc.arguments}));
        content_blocks.push(json!({
            "type": "tool_use",
            "id": tc.id,
            "name": tc.name,
            "input": input,
        }));
    }

    if content_blocks.is_empty() {
        return json!({"role": "assistant", "content": ""});
    }

    json!({"role": "assistant", "content": Value::Array(content_blocks)})
}

// ── Build Anthropic content blocks from canonical ContentParts ──

fn build_anthropic_content_blocks(parts: &[ContentPart], _thinking: Option<&str>, _tool_calls: Option<&[ToolCall]>) -> Value {
    let blocks: Vec<Value> = parts.iter().map(|p| match p {
        ContentPart::Text(t) => json!({"type": "text", "text": t}),
        ContentPart::Image { url, .. } => json!({
            "type": "image",
            "source": {"type": "base64", "media_type": "image/jpeg", "data": url},
        }),
        ContentPart::File { data, filename } => json!({
            "type": "text",
            "text": format!("[{filename} data: {data}]"),
        }),
        ContentPart::Refusal(r) => json!({"type": "text", "text": r}),
        ContentPart::Thinking(t) => json!({"type": "thinking", "thinking": t}),
        ContentPart::InputAudio { data, format } => json!({
            "type": "text",
            "text": format!("[audio {format} data: {data}]"),
        }),
    }).collect();

    if blocks.is_empty() {
        return Value::String(String::new());
    }
    if blocks.len() == 1 && blocks[0].get("type").and_then(Value::as_str) == Some("text") {
        return blocks[0]["text"].clone();
    }
    Value::Array(blocks)
}

// ── Convert canonical ToolDefinition → Anthropic tool format ──

fn build_anthropic_tool(tool: &ToolDefinition) -> Option<Value> {
    match &tool.tool_type {
        ToolType::Function => {
            let raw_name = if tool.name.is_empty() { return None; } else { &tool.name };
            // [FIX #19] Truncate tool names > 64 chars
            let name = truncate_tool_name(raw_name);
            let params = fix_tool_params(&tool.parameters);
            Some(json!({
                "name": name,
                "description": tool.description.as_deref().unwrap_or(""),
                "input_schema": params,
            }))
        }
        // Anthropic supports custom/built-in tools via text-embedded format.
        // [FIX #13] Match CodexPlusPlus FREEFORM description format.
        ToolType::Custom(_) | ToolType::WebSearch | ToolType::LocalShell | ToolType::ComputerUse => {
            let raw_name = &tool.name;
            let name = truncate_tool_name(raw_name);
            let desc = if tool.description.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                format!("FREEFORM custom tool: {name}. Put only the tool input text here.")
            } else {
                format!(
                    "{}\n\nThis is a FREEFORM tool. Do not wrap the input in JSON or markdown.",
                    tool.description.as_deref().unwrap_or("").trim()
                )
            };
            Some(json!({
                "name": name,
                "description": desc,
                "input_schema": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "input": {"type": "string", "description": "Raw freeform input for this tool."}
                    },
                    "required": ["input"]
                }
            }))
        }
        ToolType::Namespace(_children) => {
            // Anthropic doesn't support namespace tools natively.
            // We map each namespace child as a flat tool with namespace__name format.
            // This is handled upstream in the Responses→canonical conversion.
            None
        }
    }
}

// ── Convert Responses tool_choice → Anthropic tool_choice ──

fn convert_tool_choice(tc: &Value) -> Option<Value> {
    match tc {
        Value::String(s) => match s.as_str() {
            "none" => Some(json!({"type": "none"})),
            "auto" => Some(json!({"type": "auto"})),
            "required" => Some(json!({"type": "any"})),
            _ => Some(json!({"type": "auto"})),
        },
        Value::Object(obj) => {
            if obj.get("type").and_then(Value::as_str) == Some("function") {
                // {type:"function", name:"x"} or {type:"function", function:{name:"x"}}
                // Try flat name first: {name: "x"}
                if let Some(name) = obj.get("name").and_then(Value::as_str) {
                    return Some(json!({"type": "tool", "name": name}));
                }
                // Try nested function.name: {function: {name: "x"}}
                if let Some(name) = obj.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(Value::as_str)
                {
                    return Some(json!({"type": "tool", "name": name}));
                }
                Some(json!({"type": "auto"}))
            } else if let Some(name) = obj.get("name").and_then(Value::as_str) {
                Some(json!({"type": "tool", "name": name}))
            } else {
                Some(json!({"type": "auto"}))
            }
        }
        Value::Null => None,
        _ => Some(json!({"type": "auto"})),
    }
}

// ── Ensure tool parameters always have type/object ──

// [FIX #19] Truncate tool names longer than 64 chars using a short hash suffix.
// Some upstream APIs (OpenAI, Anthropic, vLLM) reject tool names > 64 characters.
fn truncate_tool_name(name: &str) -> String {
    const MAX_TOOL_NAME: usize = 64;
    if name.len() <= MAX_TOOL_NAME {
        return name.to_string();
    }
    // Simple deterministic hash
    let hash: u32 = name.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let hash_str = format!("{:08x}", hash);
    let prefix_len = MAX_TOOL_NAME - 9; // 8 hash chars + 1 underscore
    format!("{}_{}", &name[..prefix_len], hash_str)
}

fn fix_tool_params(params: &Value) -> Value {
    let mut p = params.clone();
    if let Some(obj) = p.as_object_mut() {
        if obj.get("type").and_then(Value::as_str).is_none_or(|t| t != "object") {
            obj["type"] = json!("object");
        }
    } else {
        p = json!({"type": "object", "properties": {}, "required": []});
    }
    p
}

// ── Utility ──

fn content_to_text(parts: &[ContentPart]) -> String {
    parts.iter()
        .filter_map(|p| if let ContentPart::Text(t) = p { Some(t.as_str()) } else { None })
        .collect::<Vec<_>>().join("\n")
}
