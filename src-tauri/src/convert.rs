// ── Protocol conversion: Responses API ↔ Chat API / Anthropic Messages API ──

use crate::models::*;

/// Extract messages array from Responses API input.
/// Returns (system_message, user_messages)
pub fn extract_messages(input: &serde_json::Value) -> (Option<String>, Vec<(String, String)>) {
    let mut system = None;
    let mut messages = Vec::new();

    match input {
        serde_json::Value::String(s) => {
            messages.push(("user".into(), s.clone()));
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                let role = item.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                if role == "system" || role == "developer" {
                    if let Some(text) = extract_content_text(item.get("content")) {
                        system = Some(text);
                    }
                } else if role == "user" || role == "assistant" {
                    if let Some(text) = extract_content_text(item.get("content")) {
                        messages.push((role.to_string(), text));
                    }
                }
            }
        }
        _ => {}
    }

    (system, messages)
}

fn extract_content_text(content: Option<&serde_json::Value>) -> Option<String> {
    match content {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Array(arr)) => {
            arr.iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str().map(String::from)))
                .next()
        }
        _ => None,
    }
}

// ── Responses API → Chat API (request) ──

pub fn responses_to_chat_request(body: &serde_json::Value) -> anyhow::Result<ChatRequest> {
    let resp: ResponsesRequest = serde_json::from_value(body.clone())?;
    let (system, user_messages) = extract_messages(&resp.input);

    let mut messages = Vec::new();
    if let Some(sys) = system.or(resp.instructions.clone()) {
        messages.push(ChatMessage {
            role: "system".into(),
            content: Some(sys),
        });
    }
    for (role, content) in &user_messages {
        messages.push(ChatMessage {
            role: role.clone(),
            content: Some(content.clone()),
        });
    }

    Ok(ChatRequest {
        model: resp.model.unwrap_or_else(|| "gpt-4".into()),
        messages,
        stream: resp.stream,
        max_tokens: resp.max_output_tokens,
        temperature: resp.temperature,
        top_p: resp.top_p,
        tools: resp.tools,
        tool_choice: resp.tool_choice,
    })
}

// ── Chat API → Responses API (response) ──

pub fn chat_to_responses_response(chat: &ChatResponse, model: &str) -> ResponsesResponse {
    let mut output = Vec::new();
    for choice in &chat.choices {
        let mut content = Vec::new();
        if let Some(text) = &choice.message.content {
            content.push(ResponsesOutputContent {
                content_type: "output_text".into(),
                text: text.clone(),
            });
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
        .map(|(role, content)| AnthropicMessage { role, content })
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
        if item.content_type == "text" {
            content.push(ResponsesOutputContent {
                content_type: "output_text".into(),
                text: item.text.clone(),
            });
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
