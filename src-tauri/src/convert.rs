// ── Protocol conversion: Responses API ↔ Chat API / Anthropic Messages API ──

use crate::models::*;

/// Extract messages array from Responses API input.
/// Returns (system_message, messages) where each message is (role, content, optional_reasoning_content)
pub fn extract_messages(input: &serde_json::Value) -> (Option<String>, Vec<(String, String, Option<String>)>) {
    let mut system = None;
    let mut messages = Vec::new();

    match input {
        serde_json::Value::String(s) => {
            messages.push(("user".into(), s.clone(), None));
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                let role = item.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                match role {
                    "system" | "developer" => {
                        if let Some(text) = extract_all_text(item.get("content")) {
                            system = Some(text);
                        }
                    }
                    "user" => {
                        let content = item.get("content");
                        // Handle both string content and array content (with files, images)
                        let text = match content {
                            Some(serde_json::Value::String(s)) => Some(s.clone()),
                            Some(serde_json::Value::Array(arr)) => {
                                let parts: Vec<String> = arr.iter().filter_map(|c| {
                                    c.get("text").and_then(|t| t.as_str()).map(String::from)
                                }).collect();
                                if parts.is_empty() { None } else { Some(parts.join("\n")) }
                            }
                            _ => None
                        };
                        if let Some(t) = text {
                            messages.push((role.to_string(), t, None));
                        }
                    }
                    "assistant" => {
                        let (text, thinking) = extract_assistant_content(item.get("content"));
                        if text.is_some() || thinking.is_some() {
                            messages.push((role.to_string(), text.unwrap_or_default(), thinking));
                        }
                        // Handle tool_calls in the assistant message
                        if let Some(tool_calls) = item.get("tool_calls").and_then(|v| v.as_array()) {
                            for tc in tool_calls {
                                if let Some(tc_text) = extract_tool_call_text(tc) {
                                    messages.push(("assistant".into(), tc_text, None));
                                }
                            }
                        }
                    }
                    "tool" => {
                        // Tool result messages: role=tool, content=result
                        let tc = item.get("tool_call_id").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let result = extract_all_text(item.get("content")).unwrap_or_default();
                        // Wrap tool result so the upstream chat model understands it
                        let wrapped = format!("[tool_call_id: {}]\nresult: {}", tc, result);
                        messages.push(("tool".into(), wrapped, None));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    (system, messages)
}

fn extract_tool_call_text(tc: &serde_json::Value) -> Option<String> {
    let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("call_unknown");
    let func = tc.get("function")?;
    let name = func.get("name").and_then(|v| v.as_str())?;
    let args = func.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
    Some(format!("[tool_call: {}] {}({})", id, name, args))
}

/// Extract text content from a Responses API content field, including thinking blocks.
fn extract_all_text(content: Option<&serde_json::Value>) -> Option<String> {
    match content {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Array(arr)) => {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(text.to_string());
                }
                if item.get("type").and_then(|t| t.as_str()) == Some("thinking") {
                    if let Some(thinking) = item.get("thinking").and_then(|t| t.as_str()) {
                        parts.push(thinking.to_string());
                    }
                }
                // Handle input files (type: "input_file", "input_image")
                if let Some(file_data) = item.get("file_data").and_then(|f| f.as_str()) {
                    let filename = item.get("filename").and_then(|f| f.as_str()).unwrap_or("file");
                    parts.push(format!("[{filename} data: {file_data}]"));
                }
                if let Some(image_url) = item.get("image_url").and_then(|u| u.as_str()) {
                    parts.push(format!("[image: {image_url}]"));
                }
            }
            if parts.is_empty() { None } else { Some(parts.join("\n")) }
        }
        _ => None,
    }
}

/// For assistant messages, separate visible text from thinking/reasoning content.
fn extract_assistant_content(content: Option<&serde_json::Value>) -> (Option<String>, Option<String>) {
    match content {
        Some(serde_json::Value::String(s)) => (Some(s.clone()), None),
        Some(serde_json::Value::Array(arr)) => {
            let mut text_parts = Vec::new();
            let mut thinking_parts = Vec::new();
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text.to_string());
                }
                if item.get("type").and_then(|t| t.as_str()) == Some("thinking") {
                    if let Some(thinking) = item.get("thinking").and_then(|t| t.as_str()) {
                        thinking_parts.push(thinking.to_string());
                    }
                }
            }
            let text = if text_parts.is_empty() { None } else { Some(text_parts.join("\n")) };
            let thinking = if thinking_parts.is_empty() { None } else { Some(thinking_parts.join("\n")) };
            (text, thinking)
        }
        _ => (None, None),
    }
}

// ── Responses API → Chat API (request) ──

fn tools_to_chat_format(tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
    tools.iter().filter_map(|tool| {
        if tool.get("function").is_some() {
            return Some(tool.clone());
        }
        let name = tool.get("name")?.as_str()?;
        let mut function_obj = serde_json::Map::new();
        function_obj.insert("name".into(), serde_json::Value::String(name.to_string()));
        if let Some(desc) = tool.get("description") {
            function_obj.insert("description".into(), desc.clone());
        }
        if let Some(params) = tool.get("parameters") {
            function_obj.insert("parameters".into(), params.clone());
        } else {
            function_obj.insert("parameters".into(), serde_json::json!({
                "type": "object",
                "properties": {}
            }));
        }
        if let Some(strict) = tool.get("strict") {
            function_obj.insert("strict".into(), strict.clone());
        }
        let mut chat_tool = serde_json::Map::new();
        chat_tool.insert("type".into(), serde_json::Value::String("function".into()));
        chat_tool.insert("function".into(), serde_json::Value::Object(function_obj));
        Some(serde_json::Value::Object(chat_tool))
    }).collect()
}

fn tool_choice_to_chat_format(tool_choice: &serde_json::Value) -> serde_json::Value {
    if tool_choice.is_string() || tool_choice.get("function").is_some() {
        return tool_choice.clone();
    }
    if let Some(obj) = tool_choice.as_object() {
        if let Some(name) = obj.get("name") {
            let mut function_obj = serde_json::Map::new();
            function_obj.insert("name".into(), name.clone());
            let mut result = obj.clone();
            result.remove("name");
            result.insert("function".into(), serde_json::Value::Object(function_obj));
            return serde_json::Value::Object(result);
        }
    }
    tool_choice.clone()
}

pub fn responses_to_chat_request(body: &serde_json::Value) -> anyhow::Result<ChatRequest> {
    let resp: ResponsesRequest = serde_json::from_value(body.clone())?;
    let (system, user_messages) = extract_messages(&resp.input);

    let mut messages = Vec::new();
    if let Some(sys) = system.or(resp.instructions.clone()) {
        messages.push(ChatMessage {
            role: "system".into(),
            content: Some(sys),
            reasoning_content: None,
        });
    }
    for (role, content, reasoning_content) in &user_messages {
        messages.push(ChatMessage {
            role: role.clone(),
            content: Some(content.clone()),
            reasoning_content: reasoning_content.clone(),
        });
    }

    Ok(ChatRequest {
        model: resp.model.unwrap_or_else(|| "gpt-4".into()),
        messages,
        stream: resp.stream,
        max_tokens: resp.max_output_tokens,
        temperature: resp.temperature,
        top_p: resp.top_p,
        tools: resp.tools.as_ref().map(|t| tools_to_chat_format(t)),
        tool_choice: resp.tool_choice.map(|tc| tool_choice_to_chat_format(&tc)),
    })
}

// ── Chat API → Responses API (response) ──

pub fn chat_to_responses_response(chat: &ChatResponse, model: &str) -> ResponsesResponse {
    let mut output = Vec::new();
    for choice in &chat.choices {
        let mut content = Vec::new();
        if let Some(ref thinking) = choice.message.reasoning_content {
            if !thinking.is_empty() {
                content.push(ResponsesOutputContent {
                    content_type: "thinking".into(),
                    text: None,
                    thinking: Some(thinking.clone()),
                });
            }
        }
        if let Some(text) = &choice.message.content {
            if !text.is_empty() {
                content.push(ResponsesOutputContent {
                    content_type: "output_text".into(),
                    text: Some(text.clone()),
                    thinking: None,
                });
            }
        }
        output.push(ResponsesOutput {
            output_type: "message".into(),
            role: choice.message.role.clone(),
            content,
        });
    }

    let usage = chat.usage.as_ref().map(|u| ResponsesUsage {
        input_tokens: u.prompt_tokens,
        output_tokens: u.completion_tokens,
        total_tokens: Some(u.total_tokens),
    });

    ResponsesResponse {
        id: chat.id.replace("chatcmpl", "resp"),
        object: "response".into(),
        created: chat.created,
        model: model.into(),
        output,
        usage,
    }
}

// ── Responses API → Anthropic Messages API (request) ──

pub fn responses_to_anthropic_request(body: &serde_json::Value) -> anyhow::Result<AnthropicRequest> {
    let resp: ResponsesRequest = serde_json::from_value(body.clone())?;
    let (system, user_messages) = extract_messages(&resp.input);

    let messages: Vec<AnthropicMessage> = user_messages
        .into_iter()
        .map(|(role, content, _reasoning)| AnthropicMessage { role, content })
        .collect();

    let system_text = system.or(resp.instructions.clone());

    Ok(AnthropicRequest {
        model: resp.model.unwrap_or_else(|| "claude-sonnet-4-20250514".into()),
        max_tokens: resp.max_output_tokens.unwrap_or(4096),
        system: system_text,
        messages,
        stream: resp.stream,
        temperature: resp.temperature,
    })
}

// ── Anthropic → Responses API (response) ──

pub fn anthropic_to_responses_response(anth: &AnthropicResponse, model: &str) -> ResponsesResponse {
    let mut content = Vec::new();
    for item in &anth.content {
        match item.content_type.as_str() {
            "text" => {
                content.push(ResponsesOutputContent {
                    content_type: "output_text".into(),
                    text: Some(item.text.clone()),
                    thinking: None,
                });
            }
            "thinking" | "reasoning" => {
                content.push(ResponsesOutputContent {
                    content_type: "thinking".into(),
                    text: None,
                    thinking: Some(item.text.clone()),
                });
            }
            _ => {
                content.push(ResponsesOutputContent {
                    content_type: "output_text".into(),
                    text: Some(item.text.clone()),
                    thinking: None,
                });
            }
        }
    }

    let output = vec![ResponsesOutput {
        output_type: "message".into(),
        role: anth.role.clone(),
        content,
    }];

    let usage = anth.usage.as_ref().map(|u| ResponsesUsage {
        input_tokens: u.input_tokens,
        output_tokens: u.output_tokens,
        total_tokens: Some(u.input_tokens + u.output_tokens),
    });

    ResponsesResponse {
        id: format!("resp_{}", &anth.id[..8.min(anth.id.len())]),
        object: "response".into(),
        created: chrono::Utc::now().timestamp(),
        model: model.into(),
        output,
        usage,
    }
}

