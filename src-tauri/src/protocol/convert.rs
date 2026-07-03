//! Direct Responses ↔ Chat protocol conversion.
//!
//! Uses ToolContext structs (like CodexPlusPlus) instead of abstract canonical types.
//! All functions work directly with serde_json::Value for minimum indirection.

use std::collections::BTreeSet;
use serde_json::{json, Value};

use crate::protocol::tool_context::*;

// ── Responses Request → Chat Completions Request ──

/// Convert a Responses API request body to a Chat Completions request body.
pub fn responses_to_chat(body: &Value) -> anyhow::Result<Value> {
    let tool_ctx = ToolContext::from_request(body.get("tools"));
    let mut result = json!({});

    // Model
    if let Some(model) = body.get("model") {
        result["model"] = model.clone();
    }

    // Instructions → system message
    let mut messages: Vec<Value> = Vec::new();
    if let Some(instructions) = body.get("instructions") {
        let text = instruction_text(instructions);
        if !text.is_empty() {
            messages.push(json!({"role": "system", "content": text}));
        }
    }

    // Input items → chat messages
    if let Some(input) = body.get("input") {
        append_input(input, &mut messages, &tool_ctx);
    }
    messages = collapse_system_to_head(messages);
    result["messages"] = json!(messages);

    // Max tokens
    let model = body.get("model").and_then(Value::as_str).unwrap_or("");
    if let Some(v) = body.get("max_output_tokens") {
        if model.starts_with('o') {
            result["max_completion_tokens"] = v.clone();
        } else {
            result["max_tokens"] = v.clone();
        }
    }

    // Passthrough scalar fields
    for key in &["temperature", "top_p", "stream"] {
        if let Some(v) = body.get(*key) { result[*key] = v.clone(); }
    }

    // Stream options — always include usage
    if body.get("stream").and_then(Value::as_bool).unwrap_or(false) {
        let mut opts = body.get("stream_options").cloned().unwrap_or_else(|| json!({}));
        opts["include_usage"] = json!(true);
        result["stream_options"] = opts;
    }

    // Tools
    if let Some(tools) = body.get("tools").and_then(Value::as_array) {
        let chat_tools = responses_tools_to_chat(tools, &tool_ctx);
        if !chat_tools.is_empty() {
            result["tools"] = json!(chat_tools);
        }
    }

    // Tool choice
    if let Some(tc) = body.get("tool_choice") {
        result["tool_choice"] = tc.clone();
    }

    // Reasoning
    // Reasoning effort mapping
    if let Some(reasoning) = body.get("reasoning").filter(|r| !r.is_null()) {
        let effort = reasoning.get("effort").and_then(Value::as_str).unwrap_or("");
        if !effort.is_empty() && effort != "none" && effort != "off" && effort != "disabled" {
            result["reasoning"] = json!({"effort": effort});
        }
    }

    // Passthrough extra fields
    for key in &["frequency_penalty", "logit_bias", "logprobs", "metadata", "n",
                  "presence_penalty", "response_format", "seed", "service_tier",
                  "stop", "top_logprobs", "user"] {
        if let Some(v) = body.get(*key) { result[*key] = v.clone(); }
    }

    Ok(result)
}

// ── Responses Input → Chat Messages ──

fn append_input(input: &Value, messages: &mut Vec<Value>, ctx: &ToolContext) {
    match input {
        Value::String(text) => {
            messages.push(json!({"role": "user", "content": text}));
        }
        Value::Array(items) => {
            let mut pending_tcs = Vec::new();
            let mut pending_reasoning = Vec::new();
            let mut seen_ids = BTreeSet::new();
            for item in items {
                append_item(item, messages, &mut pending_tcs, &mut pending_reasoning,
                            &mut seen_ids, ctx);
            }
            flush_pending(messages, &mut pending_tcs, &mut pending_reasoning);
        }
        _ => {}
    }
}

fn append_item(
    item: &Value,
    messages: &mut Vec<Value>,
    pending_tcs: &mut Vec<Value>,
    pending_reasoning: &mut Vec<String>,
    seen_ids: &mut BTreeSet<String>,
    _ctx: &ToolContext,
) {
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");

    match item_type {
        "message" => {
            flush_pending(messages, pending_tcs, pending_reasoning);
            if let Some(role) = item.get("role").and_then(Value::as_str) {
                let _content_arr = item.get("content").and_then(Value::as_array);
                // Map Responses API roles to Chat Completions roles
                // - "developer" → "system" (DeepSeek doesn't support developer role)
                let chat_role = if role == "developer" { "system" } else { role };
                let mut entry = json!({"role": chat_role, "content": content_text(item)});

                // Assistant messages with tool_calls
                if role == "assistant" {
                    if let Some(tcs) = item.get("tool_calls")
                        .or_else(|| item.get("function_calls"))
                        .and_then(Value::as_array) {
                        let chat_tcs: Vec<Value> = tcs.iter().map(|tc| json!({
                            "id": tc.get("id").and_then(Value::as_str).unwrap_or(""),
                            "type": "function",
                            "function": {
                                "name": tc.pointer("/function/name").or_else(|| tc.get("name")).and_then(Value::as_str).unwrap_or(""),
                                "arguments": arguments_text(tc.pointer("/function/arguments").or_else(|| tc.get("arguments")))
                            }
                        })).collect();
                        entry["tool_calls"] = json!(chat_tcs);
                    }
                    // Reasoning content
                    if let Some(rc) = item.get("reasoning_content").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                        entry["reasoning_content"] = json!(rc);
                    }
                }

                // Tool role messages
                if role == "tool" {
                    if let Some(cid) = item.get("tool_call_id").or_else(|| item.get("call_id")).and_then(Value::as_str) {
                        entry["tool_call_id"] = json!(cid);
                    }
                }

                messages.push(entry);
            }
        }
        "function_call" | "custom_tool_call" | "tool_call" => {
            let tc = tool_call_from(item, item_type);
            if let Some(call_id) = tc.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                seen_ids.insert(call_id.to_string());
                pending_tcs.push(tc);
            }
        }
        "function_call_output" | "custom_tool_call_output" | "tool_result" => {
            let call_id = item.get("call_id")
                .or_else(|| item.get("tool_call_id"))
                .and_then(Value::as_str).unwrap_or("");
            if call_id.is_empty() { return; }

            if !seen_ids.contains(call_id) {
                flush_pending(messages, pending_tcs, pending_reasoning);
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": response_output_text(item.get("output").or_else(|| item.get("content")))
                }));
                return;
            }
            flush_pending(messages, pending_tcs, pending_reasoning);
            messages.push(json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": response_output_text(item.get("output").or_else(|| item.get("content")))
            }));
        }
        "reasoning" => {
            if let Some(text) = item.get("text").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                pending_reasoning.push(text.to_string());
            }
        }
        _ => {}
    }
}

fn tool_call_from(item: &Value, item_type: &str) -> Value {
    match item_type {
        "tool_call" => {
            let tu = item.get("tool_use").unwrap_or(item);
            json!({
                "id": tu.get("id").or_else(|| item.get("call_id")).or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or(""),
                "type": "function",
                "function": {
                    "name": tu.get("name").and_then(Value::as_str).unwrap_or(""),
                    "arguments": arguments_text(tu.get("input").or_else(|| tu.get("arguments")))
                }
            })
        }
        _ => {
            json!({
                "id": item.get("call_id").or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or(""),
                "type": "function",
                "function": {
                    "name": item.get("name").and_then(Value::as_str).unwrap_or(""),
                    "arguments": arguments_text(item.get("input").or_else(|| item.get("arguments")).or_else(|| item.get("output")))
                }
            })
        }
    }
}

fn flush_pending(
    messages: &mut Vec<Value>,
    pending_tcs: &mut Vec<Value>,
    pending_reasoning: &mut Vec<String>,
) {
    if !pending_reasoning.is_empty() {
        messages.push(json!({
            "role": "assistant",
            "content": "",
            "reasoning_content": pending_reasoning.join("\n")
        }));
        pending_reasoning.clear();
    }
    if !pending_tcs.is_empty() {
        if let Some(last) = messages.last_mut() {
            if last.get("role") == Some(&json!("assistant")) {
                last["tool_calls"] = json!(std::mem::take(pending_tcs));
                return;
            }
        }
        messages.push(json!({
            "role": "assistant",
            "content": "",
            "tool_calls": std::mem::take(pending_tcs)
        }));
    }
}

// ── Responses Tools → Chat Function Tools ──

fn responses_tools_to_chat(tools: &[Value], ctx: &ToolContext) -> Vec<Value> {
    let mut converted = Vec::new();
    for tool in tools {
        if let Some(name) = tool.as_str().filter(|n| !n.is_empty()) {
            converted.push(build_custom_proxy_tool(name, ""));
            continue;
        }
        match tool.get("type").and_then(Value::as_str).unwrap_or("") {
            "function" => {
                if let Some(ct) = function_tool_to_chat(tool) {
                    converted.push(ct);
                }
            }
            "custom" | "web_search" | "local_shell" | "computer_use" => {
                let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or("");
                let name = tool.get("name").and_then(Value::as_str)
                    .filter(|n| !n.is_empty())
                    .unwrap_or(tool_type);
                let description = tool.get("description").and_then(Value::as_str).unwrap_or("");
                if name == "apply_patch" {
                    converted.extend(build_apply_patch_proxy_tools(name, description));
                } else {
                    converted.push(build_custom_proxy_tool(name, description));
                }
            }
            "namespace" => {
                converted.extend(namespace_to_chat_tools(tool, ctx));
            }
            _ => {}
        }
    }
    converted
}

fn function_tool_to_chat(tool: &Value) -> Option<Value> {
    if tool.get("type").and_then(Value::as_str) != Some("function") {
        return None;
    }
    if let Some(_fn_obj) = tool.get("function") {
        let mut ct = tool.clone();
        ct["type"] = json!("function");
        if let Some(strict) = tool.get("strict").cloned() {
            if let Some(f) = ct.get_mut("function").and_then(Value::as_object_mut) {
                f.entry("strict".to_string()).or_insert(strict);
            }
        }
        return Some(ct);
    }
    let name = tool.get("name").and_then(Value::as_str).filter(|n| !n.is_empty())?;
    let params = tool.get("parameters").cloned()
        .unwrap_or_else(|| json!({"type": "object", "properties": {}, "required": []}));
    // Ensure parameters.type is "object"
    let params = fix_tool_params(params);
    Some(json!({
        "type": "function",
        "function": {
            "name": name,
            "description": tool.get("description").cloned().unwrap_or(json!("")),
            "parameters": params,
            "strict": tool.get("strict").cloned()
        }
    }))
}

fn fix_tool_params(mut params: Value) -> Value {
    match params.get_mut("type") {
        Some(t) if t.as_str() == Some("object") => params,
        Some(_) => {
            params["type"] = json!("object");
            params
        }
        None => {
            json!({"type": "object", "properties": {}, "required": []})
        }
    }
}

fn namespace_to_chat_tools(namespace_tool: &Value, _ctx: &ToolContext) -> Vec<Value> {
    let mut tools = Vec::new();
    let namespace = namespace_tool.get("name").and_then(Value::as_str).unwrap_or("");
    if namespace.is_empty() { return tools; }

    if let Some(children) = namespace_tool.get("tools").and_then(Value::as_array) {
        for child in children {
            if child.get("type").and_then(Value::as_str) != Some("function") {
                continue;
            }
            let Some(name) = child.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()) else {
                continue;
            };
            let chat_name = flatten_namespace_name(namespace, name);
            if let Some(mut ct) = function_tool_to_chat(child) {
                if let Some(f) = ct.get_mut("function").and_then(Value::as_object_mut) {
                    f.insert("name".to_string(), json!(chat_name));
                }
                tools.push(ct);
            }
        }
    }
    tools
}

// ── Reasoning ──
// Uses protocol::reasoning::styles::apply_reasoning_options from the existing module.

// ── Chat Completion Response → Responses Response ──

/// Convert a Chat Completions response body to Responses API format.
pub fn chat_to_responses(body: &Value, original_request: Option<&Value>) -> anyhow::Result<Value> {
    let ctx = original_request
        .and_then(|r| r.get("tools"))
        .map(|t| ToolContext::from_request(Some(t)))
        .unwrap_or_default();

    let choices = body.get("choices").and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("chat response missing choices"))?;
    let choice = choices.first()
        .ok_or_else(|| anyhow::anyhow!("chat response choices empty"))?;
    let message = choice.get("message")
        .ok_or_else(|| anyhow::anyhow!("chat response missing message"))?;

    let resp_id = response_id(body.get("id").and_then(Value::as_str));
    let mut output = Vec::new();

    // Reasoning
    if let Some(reasoning) = chat_reasoning_to_output(message, &resp_id) {
        output.push(reasoning);
    }

    // Message
    if let Some(msg) = chat_message_to_output(message, &resp_id) {
        output.push(msg);
    }

    // Tool calls
    output.extend(chat_tool_calls_to_output(message, &ctx));

    let status = response_status(choice.get("finish_reason").and_then(Value::as_str));
    let mut response = json!({
        "id": resp_id,
        "object": "response",
        "status": status,
        "model": body.get("model").and_then(Value::as_str).unwrap_or(""),
        "output": output,
        "usage": chat_usage_to_responses(body.get("usage"))
    });

    if choice.get("finish_reason").and_then(Value::as_str) == Some("length") {
        response["incomplete_details"] = json!({"reason": "max_output_tokens"});
    }

    // Copy fields from original request response
    if let Some(created) = body.get("created").and_then(Value::as_u64) {
        response["created_at"] = json!(created);
    }

    Ok(response)
}

fn chat_reasoning_to_output(message: &Value, resp_id: &str) -> Option<Value> {
    // Check reasoning_content field
    let text = message.get("reasoning_content").and_then(Value::as_str)
        .or_else(|| message.get("reasoning").and_then(Value::as_str))
        .filter(|s| !s.is_empty())?;
    Some(json!({
        "id": format!("{resp_id}_reason"),
        "type": "reasoning",
        "status": "completed",
        "text": text
    }))
}

fn chat_message_to_output(message: &Value, resp_id: &str) -> Option<Value> {
    let mut content = Vec::new();

    if let Some(text) = message.get("content").and_then(Value::as_str) {
        // Strip leading think block from text
        let clean = strip_think_block(text);
        if !clean.is_empty() {
            content.push(json!({"type": "output_text", "text": clean, "annotations": []}));
        }
    } else if let Some(parts) = message.get("content").and_then(Value::as_array) {
        for part in parts {
            match part.get("type").and_then(Value::as_str).unwrap_or("") {
                "text" | "input_text" | "output_text" => {
                    if let Some(text) = part.get("text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                        content.push(json!({"type": "output_text", "text": text, "annotations": []}));
                    }
                }
                "refusal" => {
                    if let Some(refusal) = part.get("refusal").and_then(Value::as_str).filter(|r| !r.is_empty()) {
                        content.push(json!({"type": "refusal", "refusal": refusal}));
                    }
                }
                _ => {}
            }
        }
    }

    if content.is_empty() { return None; }

    Some(json!({
        "id": format!("{resp_id}_msg"),
        "type": "message",
        "status": "completed",
        "role": "assistant",
        "content": content
    }))
}

fn chat_tool_calls_to_output(message: &Value, ctx: &ToolContext) -> Vec<Value> {
    let mut output = Vec::new();
    if let Some(tcs) = message.get("tool_calls").and_then(Value::as_array) {
        for (idx, tc) in tcs.iter().enumerate() {
            output.push(tool_call_to_output_item(tc, idx, ctx));
        }
    }
    output
}

fn tool_call_to_output_item(tc: &Value, idx: usize, ctx: &ToolContext) -> Value {
    let call_id = tc.get("id").and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("call_{idx}"));
    let func = tc.get("function").unwrap_or(&Value::Null);
    let chat_name = func.get("name").and_then(Value::as_str).unwrap_or("");
    let args = arguments_text(func.get("arguments"));

    // Check if this was originally a custom tool
    if ctx.is_custom_proxy(chat_name) {
        return json!({
            "id": format!("ctc_{call_id}"),
            "type": "custom_tool_call",
            "status": "completed",
            "call_id": call_id,
            "name": ctx.original_responses_name(chat_name),
            "input": args
        });
    }

    let (orig_name, _namespace) = ctx.function_name_for_chat(chat_name);
    json!({
        "id": call_id,
        "type": "function_call",
        "status": "completed",
        "call_id": call_id,
        "name": orig_name,
        "arguments": args
    })
}

fn chat_usage_to_responses(usage: Option<&Value>) -> Value {
    let u = usage.unwrap_or(&Value::Null);
    match u {
        Value::Null => json!({"input_tokens": 0, "output_tokens": 0, "total_tokens": 0}),
        _ => json!({
            "input_tokens": u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
            "output_tokens": u.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0),
            "total_tokens": u.get("total_tokens").and_then(Value::as_u64).unwrap_or(0)
        })
    }
}

fn response_id(id: Option<&str>) -> String {
    match id {
        Some(s) if s.starts_with("resp_") => s.to_string(),
        Some(s) => format!("resp_{s}"),
        None => "resp_compat".to_string(),
    }
}

fn response_status(finish_reason: Option<&str>) -> String {
    match finish_reason {
        Some("stop") | Some("end_turn") => "completed".to_string(),
        Some("length") => "incomplete".to_string(),
        Some("tool_calls") | Some("function_call") => "completed".to_string(),
        Some("content_filter") => "incomplete".to_string(),
        _ => "completed".to_string(),
    }
}

// ── Utilities ──

fn instruction_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr.iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

fn content_text(item: &Value) -> Value {
    if let Some(text) = item.get("content").and_then(Value::as_str) {
        return json!(text);
    }
    if let Some(arr) = item.get("content").and_then(Value::as_array) {
        let has_complex = arr.iter().any(|p| !matches!(p.get("type").and_then(Value::as_str), Some("text")) && !matches!(p.get("type").and_then(Value::as_str), Some("input_text")));
        if has_complex {
            let parts: Vec<Value> = arr.iter().map(|p| {
                let t = p.get("type").and_then(Value::as_str).unwrap_or("text");
                match t {
                    "text" | "input_text" | "output_text" => json!({"type": "text", "text": p.get("text").or_else(|| p.get("content")).and_then(Value::as_str).unwrap_or("")}),
                    "image_url" => json!({"type": "image_url", "image_url": {"url": p.get("url").or_else(|| p.pointer("/image_url/url")).and_then(Value::as_str).unwrap_or("")}}),
                    "refusal" => json!({"type": "text", "text": p.get("refusal").and_then(Value::as_str).unwrap_or("")}),
                    _ => json!({"type": "text", "text": ""}),
                }
            }).collect();
            return json!(parts);
        }
        let text: String = arr.iter()
            .filter_map(|p| p.get("text").or_else(|| p.get("content")).and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        return json!(text);
    }
    json!("")
}

fn arguments_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string()),
        None => "{}".to_string(),
    }
}

fn response_output_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(v) => serde_json::to_string_pretty(v).unwrap_or_default(),
        None => String::new(),
    }
}

fn collapse_system_to_head(mut messages: Vec<Value>) -> Vec<Value> {
    let mut system_parts = Vec::new();
    let mut rest = Vec::new();
    for msg in messages.drain(..) {
        if matches!(msg.get("role").and_then(Value::as_str), Some("system") | Some("developer")) {
            if let Some(content) = msg.get("content").and_then(Value::as_str) {
                system_parts.push(content.to_string());
            }
        } else {
            rest.push(msg);
        }
    }
    if system_parts.is_empty() {
        return rest;
    }
    let mut result = Vec::new();
    result.push(json!({"role": "system", "content": system_parts.join("\n\n")}));
    result.extend(rest);
    result
}

fn strip_think_block(text: &str) -> String {
    let text = text.trim();
    if text.starts_with("<think>") {
        if let Some(end) = text.find("</think>") {
            return text[end + 8..].trim_start_matches(['\r', '\n', '\t', ' ']).to_string();
        }
    }
    text.to_string()
}
