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
    normalize_chat_messages(&mut messages);
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
    if let Some(v) = body.get("max_tokens") {
        result["max_tokens"] = v.clone();
    }
    if let Some(v) = body.get("max_completion_tokens") {
        result["max_completion_tokens"] = v.clone();
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

    // Tools — convert Responses tools to Chat function tools
    let mut has_chat_tools = false;
    if let Some(tools) = body.get("tools").and_then(Value::as_array) {
        let chat_tools = responses_tools_to_chat(tools, &tool_ctx);
        has_chat_tools = !chat_tools.is_empty();
        if has_chat_tools {
            result["tools"] = json!(chat_tools);
        }
    }

    // [FIX #1] Only write tool_choice when tools are present.
    // CodexPlusPlus: responses_tool_choice_to_chat returns Option<Value>,
    //                 None → don't write the field at all.
    if has_chat_tools {
        if let Some(tc) = body.get("tool_choice") {
            let tc_val = responses_tool_choice_to_chat(tc, &tool_ctx);
            let has_reasoning = body.get("reasoning").filter(|r| !r.is_null()).is_some();
            // When reasoning is requested, some upstream models (vLLM with Qwen, etc.)
            // need tool_choice=auto explicitly to allow tool calling alongside thinking.
            if has_reasoning {
                if let Some(s) = tc_val.as_str() {
                    if s == "none" {
                        result["tool_choice"] = json!("none");
                    } else {
                        result["tool_choice"] = json!("auto");
                    }
                } else {
                    result["tool_choice"] = tc_val;
                }
            } else {
                result["tool_choice"] = tc_val;
            }
        }
    }

    // Reasoning — model-aware style and effort mapping
    apply_chat_reasoning(&mut result, body, model);

    // Parallel tool calls — must be present alongside tools
    if has_chat_tools {
        if let Some(v) = body.get("parallel_tool_calls") {
            result["parallel_tool_calls"] = v.clone();
        }
    }

    // Passthrough extra fields
    for key in &["frequency_penalty", "logit_bias", "logprobs", "metadata", "n",
                  "presence_penalty", "response_format", "seed", "service_tier",
                  "stop", "stream_options", "top_logprobs", "user"] {
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
        // [FIX #24] CodexPlusPlus: input can be a single Object (not wrapped in array)
        Value::Object(_) => {
            let mut pending_tcs = Vec::new();
            let mut pending_reasoning = Vec::new();
            let mut seen_ids = BTreeSet::new();
            append_item(input, messages, &mut pending_tcs, &mut pending_reasoning,
                        &mut seen_ids, ctx);
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
                let chat_role = responses_role_to_chat_role(Some(role));
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
            // [FIX #28] Triple fallback: content.tool_use_id → tool_call_id → call_id
            let call_id = item.get("tool_call_id")
                .or_else(|| item.get("call_id"))
                .or_else(|| {
                    item.get("content")
                        .and_then(|c| c.get("tool_use_id"))
                        .or_else(|| item.pointer("/content/tool_use_id"))
                })
                .and_then(Value::as_str).unwrap_or("");
            if call_id.is_empty() { return; }

            let output = match item.get("output").or_else(|| item.get("content")) {
                Some(Value::String(s)) => canonicalize_json_string(s),
                Some(v) => canonical_json_string(v),
                None => String::new(),
            };
            if !seen_ids.contains(call_id) {
                flush_pending(messages, pending_tcs, pending_reasoning);
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": output
                }));
                return;
            }
            flush_pending(messages, pending_tcs, pending_reasoning);
            messages.push(json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": output
            }));
        }
        "reasoning" => {
            if let Some(text) = item.get("text").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                pending_reasoning.push(text.to_string());
            }
        }
        "input_text" | "input_image" | "input_file" | "input_audio" => {
            // Standalone input items (not wrapped in message type)
            flush_pending(messages, pending_tcs, pending_reasoning);
            let role = item.get("role").and_then(Value::as_str);
            let chat_role = responses_role_to_chat_role(role);
            let content_part = responses_content_to_chat_content(chat_role, &Value::Array(vec![item.clone()]));
            let message = json!({"role": chat_role, "content": content_part});
            messages.push(message);
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
        "function_call" => {
            // Flatten namespace for function_call history items
            let name = item.get("name").and_then(Value::as_str).unwrap_or("");
            let ns = item.get("namespace").and_then(Value::as_str).filter(|n| !n.is_empty());
            let chat_name = if let Some(ns) = ns { flatten_namespace_name(ns, name) } else { name.to_string() };
            json!({
                "id": item.get("call_id").or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or(""),
                "type": "function",
                "function": {
                    "name": chat_name,
                    "arguments": arguments_text(item.get("arguments"))
                }
            })
        }
        "custom_tool_call" => {
            // custom_tool_call history: wrap input in {input: ...}
            let name = item.get("name").and_then(Value::as_str).unwrap_or("");
            let input = item.get("input").or_else(|| item.get("arguments")).unwrap_or(&Value::Null);
            let args = response_output_text(Some(input));
            json!({
                "id": item.get("call_id").or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or(""),
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": canonical_json_string(&json!({"input": args}))
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
            let safe_name = truncate_tool_name(name);
            converted.push(build_custom_proxy_tool(&safe_name, ""));
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
                    // Preserve original input_schema/parameters so upstream sees full tool def
                    let raw_params = tool.get("input_schema")
                        .or_else(|| tool.get("parameters"))
                        .or_else(|| tool.pointer("/function/parameters"));
                    if let Some(params) = raw_params {
                        let params = normalize_tool_params(params);
                        converted.push(serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": name,
                                "description": description,
                                "parameters": params,
                                "strict": tool.get("strict").cloned()
                            }
                        }));
                    } else {
                        converted.push(build_custom_proxy_tool(name, description));
                    }
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
        // Already has a nested "function" object
        // Remove top-level Responses API fields so the result is clean Chat format
        let mut ct = tool.clone();
        ct["type"] = json!("function");
        if let Some(obj) = ct.as_object_mut() {
            obj.remove("name");
            obj.remove("description");
            obj.remove("parameters");
        }
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
    // Ensure parameters always has type/properties/required (vLLM requires this)
    let params = normalize_tool_params(&params);
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



fn namespace_to_chat_tools(namespace_tool: &Value, _ctx: &ToolContext) -> Vec<Value> {
    let mut tools = Vec::new();
    let namespace = namespace_tool.get("name").and_then(Value::as_str).unwrap_or("");
    if namespace.is_empty() { return tools; }
    // [FIX #17] Capture namespace-level description for merging
    let namespace_description = namespace_tool.get("description").and_then(Value::as_str).unwrap_or("");

    if let Some(children) = namespace_tool.get("tools").and_then(Value::as_array) {
        for child in children {
            let tool_type = child.get("type").and_then(Value::as_str).unwrap_or("");
            let Some(name) = child.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()) else {
                continue;
            };
            let chat_name = truncate_tool_name(&flatten_namespace_name(namespace, name));

            match tool_type {
                "function" => {
                    if let Some(mut ct) = function_tool_to_chat(child) {
                        if let Some(f) = ct.get_mut("function").and_then(Value::as_object_mut) {
                            f.insert("name".to_string(), json!(chat_name));
                            // [FIX #17] Merge namespace description into child description
                            let child_desc = f.get("description").and_then(Value::as_str).unwrap_or("");
                            let merged = combine_namespace_description(namespace_description, child_desc);
                            f.insert("description".to_string(), json!(merged));
                        }
                        tools.push(ct);
                    }
                }
                "custom" | "web_search" | "local_shell" | "computer_use" => {
                    // Namespace-inner custom/built-in tools (e.g. CodeGraph tools)
                    // Convert to flat Chat function format
                    let child_desc = child.get("description").and_then(Value::as_str).unwrap_or("");
                    let description = combine_namespace_description(namespace_description, child_desc);
                    let raw_params = child.get("input_schema")
                        .or_else(|| child.get("parameters"))
                        .or_else(|| child.pointer("/function/parameters"));
                    if let Some(params) = raw_params {
                        let params = normalize_tool_params(params);
                        tools.push(serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": chat_name,
                                "description": description,
                                "parameters": params,
                                "strict": child.get("strict").cloned()
                            }
                        }));
                    } else {
                        tools.push(build_custom_proxy_tool(&chat_name, &description));
                    }
                }
                _ => {} // skip unknown
            }
        }
    }
    tools
}

// ── Reasoning ──
// Uses protocol::reasoning::styles::apply_reasoning_options from the existing module.

// ── Chat Completion Response → Responses Response ──
// ── Chat role mapping and content conversion (ported from CodexPlusPlus) ──

/// Map Responses API roles to Chat Completions roles.
fn responses_role_to_chat_role(role: Option<&str>) -> &'static str {
    match role {
        Some("developer") | Some("system") => "system",
        Some("assistant") => "assistant",
        Some("tool") => "tool",
        Some("latest_reminder") => "user",
        Some("user") | None => "user",
        Some(_) => "user",
    }
}

/// Convert Responses API content items to Chat Completions content.
/// Handles `input_image` → `image_url` conversion like CodexPlusPlus.
fn responses_content_to_chat_content(_role: &str, content: &Value) -> Value {
    if content.is_null() || content.is_string() {
        return content.clone();
    }
    let Some(parts) = content.as_array() else {
        return content.clone();
    };
    let mut chat_parts = Vec::new();
    let mut has_non_text_part = false;

    for part in parts {
        match part.get("type").and_then(Value::as_str).unwrap_or("") {
            "input_text" | "output_text" | "text" => {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        chat_parts.push(json!({"type": "text", "text": text}));
                    }
                }
            }
            "refusal" => {
                if let Some(text) = part.get("refusal").and_then(Value::as_str) {
                    if !text.is_empty() {
                        chat_parts.push(json!({"type": "text", "text": text}));
                    }
                }
            }
            "input_image" => {
                if let Some(image_url) = part.get("image_url") {
                    let image_url = if image_url.is_object() {
                        image_url.clone()
                    } else {
                        json!({"url": image_url.as_str().unwrap_or_default()})
                    };
                    chat_parts.push(json!({"type": "image_url", "image_url": image_url}));
                    has_non_text_part = true;
                }
            }
            _ => {}
        }
    }

    if !has_non_text_part {
        return Value::String(
            chat_parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("
"),
        );
    }
    Value::Array(chat_parts)
}

/// Ensure assistant messages with tool_calls always have a content field.
/// Without this, some upstream APIs reject the message.
fn normalize_chat_messages(messages: &mut Vec<Value>) {
    for message in messages.iter_mut() {
        if message.get("role").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let has_content = match message.get("content") {
            Some(Value::Null) | None => false,
            Some(Value::String(s)) => !s.is_empty(),
            Some(Value::Array(parts)) => !parts.is_empty(),
            Some(_) => true,
        };
        let has_tool_calls = message
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|tcs| !tcs.is_empty());
        if has_tool_calls && !has_content {
            message["content"] = json!("");
        }
    }
}



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

    // Copy fields from original request into response (tools, instructions, etc.)
    // Codex Desktop needs these for context compaction and MCP tool tracking.
    if let Some(orig) = original_request {
        for key in ["instructions", "max_output_tokens", "parallel_tool_calls",
                     "previous_response_id", "reasoning", "temperature",
                     "tool_choice", "tools", "top_p", "metadata"] {
            if let Some(v) = orig.get(key) { response[key] = v.clone(); }
        }
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
        // [FIX #43] Reconstruct input from Chat format ({"input": raw}) to raw text
        let reconstructed = reconstruct_custom_tool_input(&args);
        return json!({
            "id": format!("ctc_{call_id}"),
            "type": "custom_tool_call",
            "status": "completed",
            "call_id": call_id,
            "name": ctx.original_responses_name(chat_name),
            "input": reconstructed
        });
    }let (orig_name, namespace) = ctx.function_name_for_chat(chat_name);
    let mut item = json!({
        "id": call_id,
        "type": "function_call",
        "status": "completed",
        "call_id": call_id,
        "name": orig_name,
        "arguments": args
    });
    if !namespace.is_empty() {
        item["namespace"] = json!(namespace);
    }
    item
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

// ── Tool choice conversion ──

/// Convert Responses API tool_choice to Chat format, handling namespace/custom tools.
/// Never returns None — preserves the original intent.
fn responses_tool_choice_to_chat(tool_choice: &Value, ctx: &ToolContext) -> Value {
    match tool_choice {
        Value::Object(obj) if obj.get("type").and_then(Value::as_str) == Some("function") => {
            // Namespace function: {type:"function", namespace:"codegraph", name:"explore"}
            if let Some(ns) = obj.get("namespace").and_then(Value::as_str).filter(|n| !n.is_empty()) {
                let name = obj.get("name").and_then(Value::as_str).unwrap_or("");
                let chat_name = flatten_namespace_name(ns, name);
                return json!({"type": "function", "function": {"name": chat_name}});
            }
            // Nested function object: {type:"function", function:{namespace:"codegraph", name:"explore"}}
            if let Some(func) = obj.get("function").and_then(Value::as_object) {
                if let Some(ns) = func.get("namespace").and_then(Value::as_str).filter(|n| !n.is_empty()) {
                    let name = func.get("name").and_then(Value::as_str).unwrap_or("");
                    let chat_name = flatten_namespace_name(ns, name);
                    return json!({"type": "function", "function": {"name": chat_name}});
                }
                if let Some(name) = func.get("name").and_then(Value::as_str) {
                    return json!({"type": "function", "function": {"name": name}});
                }
            }
            // Simple: {type:"function", name:"x"}
            let name = obj.get("name").and_then(Value::as_str).unwrap_or("");
            json!({"type": "function", "function": {"name": name}})
        }
        Value::Object(obj) if obj.get("type").and_then(Value::as_str) == Some("custom") => {
            // Custom tool choice: {type:"custom", name:"apply_patch"}
            let name = obj.get("name").and_then(Value::as_str).unwrap_or("");
            // Look up the spec — apply_patch → apply_patch_batch
            let upstream_name = if ctx.is_custom_proxy(&format!("{}_batch", name)) {
                format!("{}_batch", name)
            } else {
                name.to_string()
            };
            json!({"type": "function", "function": {"name": upstream_name}})
        }
        _ => tool_choice.clone(),
    }
}



/// Normalize tool parameters to always have type/properties/required
fn normalize_tool_params(parameters: &Value) -> Value {
    let mut normalized = if parameters.is_object() {
        parameters.clone()
    } else {
        json!({})
    };
    if normalized.get("type").is_none() {
        normalized["type"] = json!("object");
    }
    if normalized.get("properties").is_none() {
        normalized["properties"] = json!({});
    }
    if normalized.get("required").is_none() {
        normalized["required"] = json!([]);
    }
    normalized
}

// ── Model-aware reasoning style conversion ──

/// Apply reasoning options based on model name, matching CodexPlusPlus behavior.
fn apply_chat_reasoning(result: &mut Value, body: &Value, model: &str) {
    let Some(enabled) = reasoning_requested(body) else { return; };
    let style = infer_reasoning_style(model);

    match style {
        ReasoningStyle::Thinking => {
            result["thinking"] = json!({"type": if enabled { "enabled" } else { "disabled" }});
        }
        ReasoningStyle::EnableThinking => {
            result["enable_thinking"] = json!(enabled);
        }
        ReasoningStyle::ReasoningSplit => {
            result["reasoning_split"] = json!(enabled);
        }
        _ => {}
    }

    if !enabled {
        if style == ReasoningStyle::OpenRouter {
            result["reasoning"] = json!({"effort": "none"});
        }
        return;
    }

    let Some(effort) = body.pointer("/reasoning/effort").and_then(Value::as_str) else {
        return;
    };
    let Some(mapped) = map_reasoning_effort(effort, style) else {
        return;
    };

    match style {
        ReasoningStyle::OpenRouter => {
            result["reasoning"] = json!({"effort": mapped});
        }
        ReasoningStyle::DeepSeek | ReasoningStyle::LowHigh | ReasoningStyle::Default
            if supports_chat_reasoning_effort(model) =>
        {
            result["reasoning_effort"] = json!(mapped);
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReasoningStyle {
    Default,
    DeepSeek,
    LowHigh,
    OpenRouter,
    Thinking,
    EnableThinking,
    ReasoningSplit,
}

fn reasoning_requested(body: &Value) -> Option<bool> {
    if let Some(effort) = body.pointer("/reasoning/effort").and_then(Value::as_str) {
        return Some(!matches!(
            effort.trim().to_ascii_lowercase().as_str(),
            "none" | "off" | "disabled"
        ));
    }
    body.get("reasoning").map(|v| !v.is_null())
}

fn infer_reasoning_style(model: &str) -> ReasoningStyle {
    let m = model.to_ascii_lowercase();
    if m.contains("openrouter") || m.starts_with("openrouter/") {
        return ReasoningStyle::OpenRouter;
    }
    if m.contains("deepseek") {
        return ReasoningStyle::DeepSeek;
    }
    if m.contains("qwen") || m.contains("dashscope") || m.contains("bailian") {
        return ReasoningStyle::EnableThinking;
    }
    if m.contains("kimi") || m.contains("moonshot") || m.contains("glm")
        || m.contains("zhipu") || m.contains("z.ai") || m.contains("mimo")
    {
        return ReasoningStyle::Thinking;
    }
    if m.contains("minimax") {
        return ReasoningStyle::ReasoningSplit;
    }
    if m.contains("siliconflow") {
        return ReasoningStyle::EnableThinking;
    }
    if m.contains("stepfun") || m.contains("step-3.5-flash-2603") {
        return ReasoningStyle::LowHigh;
    }
    ReasoningStyle::Default
}

fn map_reasoning_effort(effort: &str, style: ReasoningStyle) -> Option<&'static str> {
    let e = effort.trim().to_ascii_lowercase();
    if matches!(e.as_str(), "none" | "off" | "disabled") {
        return None;
    }
    match style {
        ReasoningStyle::DeepSeek => match e.as_str() {
            "max" | "xhigh" => Some("max"),
            _ => Some("high"),
        },
        ReasoningStyle::LowHigh => match e.as_str() {
            "minimal" | "low" => Some("low"),
            _ => Some("high"),
        },
        ReasoningStyle::OpenRouter => match e.as_str() {
            "max" | "xhigh" => Some("xhigh"),
            "high" => Some("high"),
            "medium" => Some("medium"),
            "low" => Some("low"),
            "minimal" => Some("minimal"),
            _ => None,
        },
        _ => match e.as_str() {
            "minimal" => Some("minimal"),
            "low" => Some("low"),
            "medium" => Some("medium"),
            "high" => Some("high"),
            "xhigh" => Some("xhigh"),
            "max" => Some("max"),
            _ => None,
        },
    }
}

fn supports_chat_reasoning_effort(model: &str) -> bool {
    is_o_series(model)
        || model.to_lowercase().strip_prefix("gpt-")
            .and_then(|r| r.chars().next())
            .is_some_and(|ch| ch.is_ascii_digit() && ch >= '5')
        || infer_reasoning_style(model) == ReasoningStyle::DeepSeek
        || infer_reasoning_style(model) == ReasoningStyle::LowHigh
}

fn is_o_series(model: &str) -> bool {
    model.len() > 1 && model.starts_with('o')
        && model.as_bytes().get(1).is_some_and(|b| b.is_ascii_digit())
}

/// Sort-key canonical JSON string (deterministic for tool arguments).
fn canonical_json_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => serde_json::to_string(v).unwrap_or_default(),
        Value::Array(items) => {
            let parts: Vec<_> = items.iter().map(canonical_json_string).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(k, _)| *k);
            let parts: Vec<_> = entries.iter().map(|(k, v)| {
                format!("{}:{}", serde_json::to_string(k).unwrap_or_default(), canonical_json_string(v))
            }).collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

/// Try to parse a string as JSON and re-serialize in canonical form.
// [FIX #17] Merge namespace-level description with child tool description.
fn combine_namespace_description(namespace_desc: &str, child_desc: &str) -> String {
    let ns = namespace_desc.trim();
    let cd = child_desc.trim();
    match (ns.is_empty(), cd.is_empty()) {
        (true, true) => String::new(),
        (true, false) => cd.to_string(),
        (false, true) => ns.to_string(),
        (false, false) => format!("{ns}

{cd}"),
    }
}

// [FIX #19] Truncate tool names longer than 64 chars using a short hash suffix.
// Some upstream APIs (OpenAI, vLLM) reject tool names > 64 characters.
fn truncate_tool_name(name: &str) -> String {
    const MAX_TOOL_NAME: usize = 64;
    if name.len() <= MAX_TOOL_NAME {
        return name.to_string();
    }
    // Simple deterministic hash: sum of bytes mod 2^32
    let hash: u32 = name.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let hash_str = format!("{:08x}", hash);
    let prefix_len = MAX_TOOL_NAME - 9; // 8 hash chars + 1 underscore
    format!("{}_{}", &name[..prefix_len], hash_str)
}

fn canonicalize_json_string(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::Object(_)) => trimmed.to_string(),
        Ok(v) => canonical_json_string(&json!({"input": v})),
        Err(_) => canonical_json_string(&json!({"input": s})),
    }
}

// [FIX #43] Reconstruct custom tool call input from Chat function_call arguments.
// Chat format wraps custom tool input as {"input": "raw text"}.
// This unwraps it back to the raw text that Codex Desktop expects.
fn reconstruct_custom_tool_input(arguments: &str) -> String {
    let trimmed = arguments.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::Object(obj)) => {
            if let Some(input) = obj.get("input") {
                match input {
                    Value::String(s) => s.clone(),
                    other => serde_json::to_string(other).unwrap_or_default(),
                }
            } else {
                // No "input" key — return raw args (might be structured tool)
                arguments.to_string()
            }
        }
        _ => arguments.to_string(),
    }
}

