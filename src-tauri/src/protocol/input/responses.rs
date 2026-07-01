// ── Responses API → canonical types ──

use std::collections::BTreeSet;
use serde_json::Value;

use crate::protocol::core::types::*;
use crate::protocol::core::traits::InputProtocol;

/// Parses OpenAI Responses API wire format into canonical ProtocolRequest.
pub struct ResponsesInput;

impl InputProtocol for ResponsesInput {
    fn name(&self) -> &str { "responses" }

    fn parse_request(&self, body: &Value) -> anyhow::Result<ProtocolRequest> {
        let model = body.get("model").and_then(Value::as_str).unwrap_or("").to_string();
        let stream = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
        let stream_options = body.get("stream_options").cloned();
        let has_tools = body.get("tools").and_then(Value::as_array).is_some_and(|t| !t.is_empty());

        let mut system = None;
        if let Some(instructions) = body.get("instructions") {
            let text = instruction_text(instructions);
            if !text.is_empty() { system = Some(text); }
        }

        let mut messages = Vec::new();
        if let Some(input) = body.get("input") {
            let mut pending_tc = Vec::new();
            let mut pending_r = Vec::new();
            let mut seen_ids = BTreeSet::new();
            append_responses_input(input, &mut messages, &mut pending_tc, &mut pending_r, &mut seen_ids);
            flush_tool_calls(&mut messages, &mut pending_tc, &mut pending_r);
            flush_reasoning(&mut messages, &mut pending_r);
        }

        let tools = if has_tools {
            body.get("tools").and_then(Value::as_array)
                .map(|arr| arr.iter().map(parse_tool_definition).filter_map(|t| t).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let tool_choice = body.get("tool_choice").cloned();

        let reasoning = parse_reasoning(body);

        Ok(ProtocolRequest {
            model, messages, system, tools, tool_choice, reasoning,
            max_tokens: body.get("max_output_tokens").and_then(Value::as_u64).map(|v| v as u32)
                .or_else(|| body.get("max_tokens").and_then(Value::as_u64).map(|v| v as u32)),
            temperature: body.get("temperature").and_then(Value::as_f64).map(|v| v as f32),
            top_p: body.get("top_p").and_then(Value::as_f64).map(|v| v as f32),
            stream, stream_options,
            metadata: body.get("metadata").cloned(),
            extra_fields: collect_extra_fields(body, &["model", "input", "instructions", "tools", "tool_choice",
                "reasoning", "max_output_tokens", "max_tokens", "temperature", "top_p", "stream", "stream_options",
                "metadata", "parallel_tool_calls"]),
        })
    }

    fn parse_response(&self, body: &Value) -> anyhow::Result<ProtocolResponse> {
        unimplemented!("Responses input only used for request conversion; response passthrough is direct")
    }
}

// ── Helper: parse reasoning config ──

/// Extract reasoning configuration from a Responses API request body.
fn parse_reasoning(body: &Value) -> Option<ReasoningConfig> {
    let reasoning = body.get("reasoning")?;
    if reasoning.is_null() { return None; }
    let effort = reasoning.get("effort").and_then(Value::as_str).map(|s| s.to_string());
    let enabled = !matches!(effort.as_deref(), Some("none" | "off" | "disabled"));
    Some(ReasoningConfig { enabled: enabled || effort.is_some(), effort })
}

// ── Helper: extract instruction text ──

/// Extract instruction/system text from a Responses API instructions field.
fn instruction_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts.iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str).or_else(|| p.as_str()))
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        other => other.as_str().unwrap_or_default().to_string(),
    }
}

// ── Helper: parse tool definition ──

/// Parse a single tool definition from Responses API format into canonical form.
fn parse_tool_definition(tool: &Value) -> Option<ToolDefinition> {
    if let Some(name) = tool.as_str().filter(|n| !n.is_empty()) {
        return Some(ToolDefinition {
            name: name.to_string(),
            description: None,
            parameters: serde_json::json!({"type": "object", "properties": {}, "required": []}),
            tool_type: ToolType::Custom(CustomToolKind::Raw),
            strict: None,
        });
    }
    let tool_type_str = tool.get("type").and_then(Value::as_str).unwrap_or("");
    let name = tool.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    match tool_type_str {
        "function" => {
            let (name, desc, params, strict) = if let Some(func) = tool.get("function") {
                (
                    func.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                    func.get("description").and_then(Value::as_str).map(|s| s.to_string()),
                    func.get("parameters").cloned(),
                    func.get("strict").and_then(Value::as_bool),
                )
            } else {
                (name,
                 tool.get("description").and_then(Value::as_str).map(|s| s.to_string()),
                 tool.get("parameters").cloned(),
                 tool.get("strict").and_then(Value::as_bool))
            };
            let params = params.unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}, "required": []}));
            Some(ToolDefinition { name, description: desc, parameters: params, tool_type: ToolType::Function, strict })
        }
        "custom" | "web_search" | "local_shell" | "computer_use" => {
            let desc = tool.get("description").and_then(Value::as_str).map(|s| s.to_string());
            let kind = match tool_type_str {
                "web_search" => ToolType::WebSearch,
                "local_shell" => ToolType::LocalShell,
                "computer_use" => ToolType::ComputerUse,
                _ => {
                    let is_patch = name == "apply_patch"
                        || tool.pointer("/format/definition").and_then(Value::as_str).is_some_and(|d| d.contains("begin_patch"));
                    ToolType::Custom(if is_patch { CustomToolKind::ApplyPatch } else { CustomToolKind::Raw })
                }
            };
            Some(ToolDefinition {
                name, description: desc,
                parameters: serde_json::json!({"type": "object", "properties": {}, "required": []}),
                tool_type: kind, strict: None,
            })
        }
        "namespace" => {
            let children = tool.get("tools").and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(parse_tool_definition).collect())
                .unwrap_or_default();
            Some(ToolDefinition {
                name, description: tool.get("description").and_then(Value::as_str).map(|s| s.to_string()),
                parameters: serde_json::json!({}), tool_type: ToolType::Namespace(children), strict: None,
            })
        }
        _ => None,
    }
}

// ── Helper: append responses input items to messages ──

fn append_responses_input(
    input: &Value,
    messages: &mut Vec<ProtocolMessage>,
    pending_tc: &mut Vec<ToolCall>,
    pending_r: &mut Vec<String>,
    seen_ids: &mut BTreeSet<String>,
) {
    match input {
        Value::String(text) => {
            messages.push(ProtocolMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text(text.clone())],
                reasoning_content: None, tool_calls: Vec::new(), tool_call_id: None, name: None,
            });
        }
        Value::Array(items) => {
            for item in items {
                append_responses_item(item, messages, pending_tc, pending_r, seen_ids);
            }
        }
        Value::Object(obj) => {
            append_responses_item(&Value::Object(obj.clone()), messages, pending_tc, pending_r, seen_ids);
        }
        _ => {}
    }
}

fn append_responses_item(
    item: &Value,
    messages: &mut Vec<ProtocolMessage>,
    pending_tc: &mut Vec<ToolCall>,
    pending_r: &mut Vec<String>,
    seen_ids: &mut BTreeSet<String>,
) {
    let item_type = item.get("type").and_then(Value::as_str);
    match item_type {
        Some("message") | None => {
            flush_tool_calls(messages, pending_tc, pending_r);
            let role = match item.get("role").and_then(Value::as_str) {
                Some("developer") | Some("system") => MessageRole::System,
                Some("assistant") => MessageRole::Assistant,
                Some("tool") => MessageRole::Tool,
                _ => MessageRole::User,
            };
            let content = item.get("content");
            let (parts, reasoning) = extract_content_parts(content);

            let mut msg = ProtocolMessage {
                role, content: parts, reasoning_content: reasoning,
                tool_calls: Vec::new(), tool_call_id: None, name: None,
            };

            if role == MessageRole::Tool {
                msg.tool_call_id = item.get("tool_call_id").and_then(Value::as_str).map(|s| s.to_string());
                let text: String = msg.content.iter()
                    .filter_map(|p| if let ContentPart::Text(t) = p { Some(t.as_str()) } else { None })
                    .collect::<Vec<_>>().join("\n");
                msg.content = vec![ContentPart::Text(text)];
            }

            if role == MessageRole::Assistant {
                if let Some(tcs) = item.get("tool_calls").and_then(Value::as_array) {
                    for tc in tcs {
                        if let Some(tc_call) = extract_tool_call(tc) {
                            msg.tool_calls.push(tc_call);
                        }
                    }
                }
            }

            messages.push(msg);
        }
        Some("function_call") => {
            let call_id = item.get("call_id").or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or("").to_string();
            if call_id.is_empty() { return; }
            seen_ids.insert(call_id.clone());
            let name = item.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let args = responses_arguments_to_chat(item.get("arguments").unwrap_or(&serde_json::json!(null)));
            pending_tc.push(ToolCall { id: call_id, name, arguments: args, tool_type: ToolType::Function });
        }
        Some("function_call_output") => {
            let call_id = item.get("call_id").and_then(Value::as_str).unwrap_or("");
            if call_id.is_empty() { return; }
            flush_tool_calls(messages, pending_tc, pending_r);
            let output = response_output_text(item.get("output").unwrap_or(&serde_json::json!(null)));
            messages.push(ProtocolMessage {
                role: MessageRole::Tool,
                content: vec![ContentPart::Text(output)],
                reasoning_content: None, tool_calls: Vec::new(),
                tool_call_id: Some(call_id.to_string()), name: None,
            });
        }
        Some("custom_tool_call") => {
            let call_id = item.get("call_id").or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or("").to_string();
            if call_id.is_empty() { return; }
            seen_ids.insert(call_id.clone());
            let name = item.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let input = response_output_text(item.get("input").or_else(|| item.get("arguments")).unwrap_or(&serde_json::json!(null)));
            pending_tc.push(ToolCall { id: call_id, name, arguments: input, tool_type: ToolType::Custom(CustomToolKind::Raw) });
        }
        Some("custom_tool_call_output") => {
            let call_id = item.get("call_id").and_then(Value::as_str).unwrap_or("");
            if call_id.is_empty() { return; }
            flush_tool_calls(messages, pending_tc, pending_r);
            let output = response_output_text(item.get("output").unwrap_or(&serde_json::json!(null)));
            messages.push(ProtocolMessage {
                role: MessageRole::Tool,
                content: vec![ContentPart::Text(output)],
                reasoning_content: None, tool_calls: Vec::new(),
                tool_call_id: Some(call_id.to_string()), name: None,
            });
        }
        Some("tool_call") => {
            if let Some(tool_use) = item.get("tool_use") {
                let call_id = tool_use.get("id").or_else(|| item.get("call_id")).or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or("").to_string();
                if call_id.is_empty() { return; }
                seen_ids.insert(call_id.clone());
                let name = tool_use.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                let args = responses_arguments_to_chat(tool_use.get("input").unwrap_or(&serde_json::json!({})));
                pending_tc.push(ToolCall { id: call_id, name, arguments: args, tool_type: ToolType::Function });
            }
        }
        Some("tool_result") => {
            let content = item.get("content").unwrap_or(&serde_json::json!(null));
            let call_id = content.get("tool_use_id").or_else(|| item.get("tool_call_id")).or_else(|| item.get("call_id")).and_then(Value::as_str).unwrap_or("").to_string();
            if call_id.is_empty() { return; }
            flush_tool_calls(messages, pending_tc, pending_r);
            let output_obj = content.get("content").unwrap_or(content);
            let output = response_output_text(output_obj);
            messages.push(ProtocolMessage {
                role: MessageRole::Tool, content: vec![ContentPart::Text(output)],
                reasoning_content: None, tool_calls: Vec::new(),
                tool_call_id: Some(call_id), name: None,
            });
        }
        Some("reasoning") => {
            if let Some(text) = extract_reasoning_summary_text(item) {
                if !text.is_empty() { pending_r.push(text); }
            }
        }
        _ => {
            flush_tool_calls(messages, pending_tc, pending_r);
            if item.get("role").is_some() || item.get("content").is_some() {
                let role = match item.get("role").and_then(Value::as_str) {
                    Some("developer") | Some("system") => MessageRole::System,
                    Some("assistant") => MessageRole::Assistant,
                    Some("tool") => MessageRole::Tool,
                    _ => MessageRole::User,
                };
                let (parts, reasoning) = extract_content_parts(item.get("content"));
                let mut msg = ProtocolMessage {
                    role, content: parts, reasoning_content: reasoning,
                    tool_calls: Vec::new(), tool_call_id: None, name: None,
                };
                if role == MessageRole::Assistant && !pending_r.is_empty() && pending_tc.is_empty() {
                    msg.reasoning_content = Some(std::mem::take(pending_r).join("\n"));
                }
                messages.push(msg);
            }
        }
    }
}

// ── Helper: extract content parts from Responses content field ──

fn extract_content_parts(content: Option<&Value>) -> (Vec<ContentPart>, Option<String>) {
    match content {
        None | Some(Value::Null) => (Vec::new(), None),
        Some(Value::String(s)) => (vec![ContentPart::Text(s.clone())], None),
        Some(Value::Array(arr)) => {
            let mut parts = Vec::new();
            let mut reasoning = None;
            for item in arr {
                let ctype = item.get("type").and_then(Value::as_str).unwrap_or("");
                match ctype {
                    "input_text" | "output_text" | "text" => {
                        if let Some(text) = item.get("text").and_then(Value::as_str) {
                            if !text.is_empty() { parts.push(ContentPart::Text(text.to_string())); }
                        }
                    }
                    "thinking" => {
                        if let Some(text) = item.get("thinking").and_then(Value::as_str) {
                            reasoning = Some(text.to_string());
                        } else if let Some(text) = item.get("text").and_then(Value::as_str) {
                            reasoning = Some(text.to_string());
                        }
                    }
                    "refusal" => {
                        if let Some(text) = item.get("refusal").and_then(Value::as_str) {
                            if !text.is_empty() { parts.push(ContentPart::Refusal(text.to_string())); }
                        }
                    }
                    "input_image" | "image" | "image_url" => {
                        let url = item.get("image_url").and_then(Value::as_str)
                            .or_else(|| item.get("url").and_then(Value::as_str))
                            .or_else(|| item.get("file_data").and_then(Value::as_str))
                            .unwrap_or("").to_string();
                        let detail = item.get("detail").and_then(Value::as_str).map(|s| s.to_string());
                        parts.push(ContentPart::Image { url, detail });
                    }
                    "input_file" | "file" => {
                        let data = item.get("file_data").and_then(Value::as_str).unwrap_or("").to_string();
                        let filename = item.get("filename").and_then(Value::as_str).unwrap_or("file").to_string();
                        parts.push(ContentPart::File { data, filename });
                    }
                    "input_audio" => {
                        let data = item.get("data").or_else(|| item.get("file_data")).and_then(Value::as_str).unwrap_or("").to_string();
                        let format = item.get("format").and_then(Value::as_str).unwrap_or("wav").to_string();
                        parts.push(ContentPart::InputAudio { data, format });
                    }
                    _ => {
                        if let Some(text) = item.get("text").and_then(Value::as_str) {
                            if !text.is_empty() { parts.push(ContentPart::Text(text.to_string())); }
                        }
                    }
                }
            }
            (parts, reasoning)
        }
        _ => (Vec::new(), None),
    }
}

// ── Helpers ──

fn extract_tool_call(tc: &Value) -> Option<ToolCall> {
    let id = tc.get("id").and_then(Value::as_str).map(|s| s.to_string())?;
    let func = tc.get("function")?;
    let name = func.get("name").and_then(Value::as_str).map(|s| s.to_string())?;
    let args = func.get("arguments").and_then(Value::as_str).map(|s| s.to_string()).unwrap_or_else(|| "{}".to_string());
    Some(ToolCall { id, name, arguments: args, tool_type: ToolType::Function })
}

fn responses_arguments_to_chat(value: &Value) -> String {
    match value {
        Value::String(text) => normalize_tool_args(text),
        Value::Object(_) => canonical_json_string(value),
        Value::Null => "{}".to_string(),
        other => canonical_json_string(&serde_json::json!({"input": other})),
    }
}

fn normalize_tool_args(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() { return "{}".to_string(); }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::Object(_)) => trimmed.to_string(),
        Ok(value) => canonical_json_string(&serde_json::json!({"input": value})),
        Err(_) => canonical_json_string(&serde_json::json!({"input": text})),
    }
}

fn response_output_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => canonical_json_string(other),
    }
}

fn canonical_json_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => serde_json::to_string(v).unwrap_or_default(),
        Value::Array(vals) => {
            let parts: Vec<_> = vals.iter().map(canonical_json_string).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(k, _)| k.to_string());
            let parts: Vec<_> = entries.into_iter()
                .map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap_or_default(), canonical_json_string(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

/// Extract reasoning summary text from various response field locations.
fn extract_reasoning_summary_text(value: &Value) -> Option<String> {
    for key in ["reasoning_content", "content", "text"] {
        if let Some(text) = value.get(key).and_then(Value::as_str) {
            if !text.is_empty() { return Some(text.to_string()); }
        }
    }
    let summary = value.get("summary")?;
    if let Some(text) = summary.as_str() { return (!text.is_empty()).then(|| text.to_string()); }
    let parts = summary.as_array()?;
    let text_parts: Vec<String> = parts.iter()
        .filter_map(|p| p.get("text").and_then(Value::as_str)
            .or_else(|| p.get("content").and_then(Value::as_str))
            .or_else(|| p.as_str()))
        .filter(|t| !t.is_empty())
        .map(|s| s.to_string())
        .collect();
    if text_parts.is_empty() { None } else { Some(text_parts.join("\n\n")) }
}

/// Flush pending tool calls into the last assistant message or create a new one.
fn flush_tool_calls(messages: &mut Vec<ProtocolMessage>, pending: &mut Vec<ToolCall>, _pending_r: &mut Vec<String>) {
    if pending.is_empty() { return; }
    if let Some(last) = messages.last_mut() {
        if last.role == MessageRole::Assistant {
            last.tool_calls = std::mem::take(pending);
            return;
        }
    }
    messages.push(ProtocolMessage {
        role: MessageRole::Assistant,
        content: vec![ContentPart::Text(String::new())],
        reasoning_content: None,
        tool_calls: std::mem::take(pending),
        tool_call_id: None, name: None,
    });
}

/// Flush pending reasoning content into the last assistant message.
fn flush_reasoning(messages: &mut Vec<ProtocolMessage>, pending: &mut Vec<String>) {
    if pending.is_empty() { return; }
    let text = pending.join("\n");
    if let Some(last) = messages.last_mut() {
        if last.role == MessageRole::Assistant {
            last.reasoning_content = Some(text);
            return;
        }
    }
    messages.push(ProtocolMessage {
        role: MessageRole::Assistant,
        content: vec![ContentPart::Text(String::new())],
        reasoning_content: Some(text),
        tool_calls: Vec::new(),
        tool_call_id: None, name: None,
    });
}

/// Collect unknown fields from the request body for passthrough.
fn collect_extra_fields(body: &Value, skip: &[&str]) -> Vec<(String, Value)> {
    let obj = match body.as_object() { Some(o) => o, None => return Vec::new() };
    let skip_set: std::collections::HashSet<&str> = skip.iter().copied().collect();
    let mut extra = Vec::new();
    for (k, v) in obj {
        if !skip_set.contains(k.as_str()) {
            extra.push((k.clone(), v.clone()));
        }
    }
    extra
}
