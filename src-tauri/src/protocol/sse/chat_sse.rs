// ── Chat Completions SSE → Responses API SSE state machine ──

use std::collections::BTreeMap;
use serde_json::Value;

use crate::protocol::core::traits::SseTransform;
use crate::protocol::tools::context::CodexToolContext;
use crate::protocol::sse::parser::{take_sse_block, append_utf8_safe, is_done_block};

// ── State types ──

#[derive(Debug, Default)]
struct TextItem {
    output_index: Option<u32>,
    item_id: String,
    text: String,
    added: bool,
    done: bool,
}

#[derive(Debug, Default)]
struct ReasoningItem {
    output_index: Option<u32>,
    item_id: String,
    text: String,
    added: bool,
    done: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum InlineThinkMode { #[default] Detecting, Reasoning, Text }

#[derive(Debug, Default)]
struct InlineThink {
    mode: InlineThinkMode,
    buffer: String,
}

#[derive(Debug, Default)]
struct ToolCallState {
    output_index: Option<u32>,
    item_id: String,
    call_id: String,
    name: String,
    arguments: String,
    added: bool,
    done: bool,
}

// ── Main state machine ──

/// State machine that converts Chat Completions SSE chunks into Responses API SSE events.
pub struct ChatSseConverter {
    buffer: String,
    utf8_remainder: Vec<u8>,
    state: ChatSseState,
    failed: bool,
}

impl Default for ChatSseConverter {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            utf8_remainder: Vec::new(),
            state: ChatSseState::default(),
            failed: false,
        }
    }
}

impl ChatSseConverter {
    /// Create a converter initialized with the original request's tool context.
    pub fn with_request(orig: &serde_json::Value) -> Self {
        Self {
            state: ChatSseState::with_request(orig),
            ..Self::default()
        }
    }
}

impl SseTransform for ChatSseConverter {
    fn push_bytes(&mut self, bytes: &[u8]) -> Vec<u8> {
        append_utf8_safe(&mut self.buffer, &mut self.utf8_remainder, bytes);
        let mut output = String::new();
        while let Some(block) = take_sse_block(&mut self.buffer) {
            if block.trim().is_empty() { continue; }
            self.handle_block(&block, &mut output);
            if self.failed { break; }
        }
        output.into_bytes()
    }

    fn finish(&mut self) -> Vec<u8> {
        if !self.utf8_remainder.is_empty() {
            self.buffer.push_str(&String::from_utf8_lossy(&self.utf8_remainder));
            self.utf8_remainder.clear();
        }
        let mut output = String::new();
        if !self.failed { self.state.finalize_into(&mut output); }
        output.into_bytes()
    }

    fn fail(&mut self, message: String, error_type: Option<String>) -> Vec<u8> {
        let mut output = String::new();
        self.state.failed_into(&mut output, message, error_type);
        self.failed = true;
        output.into_bytes()
    }
}

/// Check if the tool name corresponds to a custom tool (e.g. web_search).

/// Build output item for a custom tool call (different format from function_call).
/// Restores the original tool name (e.g.  for )
/// so Codex Desktop can route it to the correct custom tool handler.
fn custom_tool_output_item(state: &ToolCallState, ctx: &CodexToolContext) -> Value {
    let original_name = ctx.original_custom_tool_name(&state.name);
    serde_json::json!({
        "id": format!("ctc_{}", state.call_id),
        "type": "custom_tool_call",
        "status": "completed",
        "call_id": state.call_id,
        "name": original_name,
        "input": state.arguments,
    })
}

/// Format and append an SSE event string to the output buffer.
pub fn push_sse(output: &mut String, event: &str, data: Value) {
    output.push_str("event: ");
    output.push_str(event);
    output.push_str("\ndata: ");
    output.push_str(&serde_json::to_string(&data).unwrap_or_default());
    output.push_str("\n\n");
}

// ── SSE event names ──

/// SSE event name for response creation.
const SSE_RESPONSE_CREATED: &str = "response.created";
const SSE_RESPONSE_IN_PROGRESS: &str = "response.in_progress";
/// SSE event name for response completion.
const SSE_RESPONSE_COMPLETED: &str = "response.completed";
const SSE_RESPONSE_FAILED: &str = "response.failed";
const SSE_OUTPUT_ITEM_ADDED: &str = "response.output_item.added";
const SSE_OUTPUT_ITEM_DONE: &str = "response.output_item.done";
const SSE_CONTENT_PART_ADDED: &str = "response.content_part.added";
const SSE_CONTENT_PART_DONE: &str = "response.content_part.done";
/// SSE event name for output text delta.
const SSE_OUTPUT_TEXT_DELTA: &str = "response.output_text.delta";
const SSE_OUTPUT_TEXT_DONE: &str = "response.output_text.done";
const SSE_REASONING_DELTA: &str = "response.reasoning_summary_text.delta";
const SSE_REASONING_DONE: &str = "response.reasoning_summary_text.done";
const SSE_REASONING_PART_ADDED: &str = "response.reasoning_summary_part.added";
const SSE_REASONING_PART_DONE: &str = "response.reasoning_summary_part.done";
const SSE_FUNC_ARGS_DELTA: &str = "response.function_call_arguments.delta";
const SSE_FUNC_ARGS_DONE: &str = "response.function_call_arguments.done";

const THINK_OPEN: &str = "<think>";
const THINK_CLOSE: &str = "</think>";

impl ChatSseConverter {
    fn handle_block(&mut self, block: &str, output: &mut String) {
        let mut event_name: Option<String> = None;
        let mut data_parts = Vec::new();
        for line in block.lines() {
            if let Some(ev) = line.strip_prefix("event:").map(|s| s.trim()) { event_name = Some(ev.to_string()); }
            if let Some(d) = line.strip_prefix("data:").map(|s| s.trim()) { data_parts.push(d.to_string()); }
        }
        if data_parts.is_empty() { return; }
        let data = data_parts.join("\n");
        if is_done_block(&data) { self.state.finalize_into(output); return; }
        let Ok(chunk) = serde_json::from_str::<Value>(&data) else { return; };
        if event_name.as_deref() == Some("error") || chunk.get("error").is_some() {
            let (msg, err_type) = extract_chat_error(&chunk);
            self.state.failed_into(output, msg, err_type);
            self.failed = true;
            return;
        }
        self.state.handle_chat_chunk(&chunk, output);
    }
}

// ── Full state ──

#[allow(dead_code)]
struct ChatSseState {
    response_started: bool,
    completed: bool,
    response_id: String,
    model: String,
    created_at: u64,
    next_output_index: u32,
    text: TextItem,
    reasoning: ReasoningItem,
    inline_think: InlineThink,
    tools: BTreeMap<usize, ToolCallState>,
    output_items: Vec<(u32, Value)>,
    latest_usage: Option<Value>,
    finish_reason: Option<String>,
    tool_context: CodexToolContext,
    original_request: Option<Value>,
}

impl Default for ChatSseState {
    fn default() -> Self {
        Self {
            response_started: false, completed: false,
            response_id: "resp_compat".to_string(), model: String::new(), created_at: 0,
            next_output_index: 0, text: TextItem::default(), reasoning: ReasoningItem::default(),
            inline_think: InlineThink::default(), tools: BTreeMap::new(),
            output_items: Vec::new(), latest_usage: None, finish_reason: None,
            tool_context: CodexToolContext::default(), original_request: None,
        }
    }
}

#[allow(dead_code)]
impl ChatSseState {
    fn with_request(orig: &Value) -> Self {
        Self {
            tool_context: CodexToolContext::from_request(orig.get("tools")),
            original_request: Some(orig.clone()),
            ..Self::default()
        }
    }

    fn handle_chat_chunk(&mut self, chunk: &Value, out: &mut String) {
        if let Some(id) = chunk.get("id").and_then(Value::as_str) {
            self.response_id = if id.starts_with("resp_") { id.to_string() } else { format!("resp_{id}") };
        }
        if let Some(model) = chunk.get("model").and_then(Value::as_str) { if !model.is_empty() { self.model = model.to_string(); } }
        if let Some(created) = chunk.get("created").and_then(Value::as_u64) { self.created_at = created; }
        self.ensure_started(out);
        if let Some(usage) = chunk.get("usage").filter(|v| !v.is_null()) { self.latest_usage = Some(chat_usage(Some(usage))); }
        let Some(choice) = chunk.get("choices").and_then(Value::as_array).and_then(|c| c.first()) else { return; };
        if let Some(delta) = choice.get("delta") {
            if let Some(r) = reasoning_text(delta) { self.push_reasoning(&r, out); }
            if let Some(content) = delta.get("content").and_then(Value::as_str) {
                if !content.is_empty() { self.push_content(content, out); }
            }
            if let Some(tcs) = delta.get("tool_calls").and_then(Value::as_array) {
                self.flush_inline_think(out);
                self.finalize_reasoning(out);
                for tc in tcs { self.push_tool_call(tc, out); }
            }
        }
        if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) { self.finish_reason = Some(fr.to_string()); }
    }

    fn ensure_started(&mut self, out: &mut String) {
        if self.response_started { return; }
        self.response_started = true;
        push_sse(out, SSE_RESPONSE_CREATED, json_response("response.created", "in_progress", self));
        push_sse(out, SSE_RESPONSE_IN_PROGRESS, json_response("response.in_progress", "in_progress", self));
    }

    fn push_reasoning(&mut self, delta: &str, out: &mut String) {
        if !self.reasoning.added {
            let oi = self.next_output_index();
            self.reasoning = ReasoningItem {
                output_index: Some(oi),
                item_id: format!("rs_{}", self.response_id),
                added: true, ..ReasoningItem::default()
            };
            push_sse(out, SSE_OUTPUT_ITEM_ADDED, serde_json::json!({
                "type": SSE_OUTPUT_ITEM_ADDED, "output_index": oi,
                "item": {"id": &self.reasoning.item_id, "type": "reasoning", "status": "in_progress", "reasoning_content": "", "summary": []}
            }));
            push_sse(out, SSE_REASONING_PART_ADDED, serde_json::json!({
                "type": SSE_REASONING_PART_ADDED, "item_id": &self.reasoning.item_id,
                "output_index": oi, "summary_index": 0,
                "part": {"type": "summary_text", "text": ""}
            }));
        }
        self.reasoning.text.push_str(delta);
        push_sse(out, SSE_REASONING_DELTA, serde_json::json!({
            "type": SSE_REASONING_DELTA, "item_id": &self.reasoning.item_id,
            "output_index": self.reasoning.output_index.unwrap_or(0), "summary_index": 0, "delta": delta
        }));
    }

    fn push_content(&mut self, delta: &str, out: &mut String) {
        match self.inline_think.mode {
            InlineThinkMode::Text => {
                self.finalize_reasoning(out);
                self.push_text(delta, out);
            }
            InlineThinkMode::Detecting => {
                self.inline_think.buffer.push_str(delta);
                match think_decision(&self.inline_think.buffer) {
                    ThinkDecision::NeedMore => {}
                    ThinkDecision::Reasoning => {
                        self.inline_think.mode = InlineThinkMode::Reasoning;
                        self.drain_inline_think(out);
                    }
                    ThinkDecision::Text => {
                        self.inline_think.mode = InlineThinkMode::Text;
                        let text = std::mem::take(&mut self.inline_think.buffer);
                        self.finalize_reasoning(out);
                        self.push_text(&text, out);
                    }
                }
            }
            InlineThinkMode::Reasoning => {
                self.inline_think.buffer.push_str(delta);
                self.drain_inline_think(out);
            }
        }
    }

    fn drain_inline_think(&mut self, out: &mut String) {
        let Some((reasoning, answer)) = split_think_block(&self.inline_think.buffer) else { return; };
        self.inline_think.mode = InlineThinkMode::Text;
        if !reasoning.is_empty() {
            self.push_reasoning(&reasoning, out);
            self.finalize_reasoning(out);
        }
        if !answer.is_empty() { self.push_text(&answer, out); }
    }

    fn flush_inline_think(&mut self, out: &mut String) {
        if self.inline_think.mode == InlineThinkMode::Detecting && !self.inline_think.buffer.is_empty() {
            let buffered = std::mem::take(&mut self.inline_think.buffer);
            let reasoning = buffered.strip_prefix(THINK_OPEN).map(|s| s.trim().to_string()).unwrap_or(buffered);
            if !reasoning.is_empty() {
                self.push_reasoning(&reasoning, out);
                self.finalize_reasoning(out);
            }
        }
    }

    fn push_text(&mut self, delta: &str, out: &mut String) {
        if !self.text.added {
            let oi = self.next_output_index();
            let item_id = format!("{}_msg", self.response_id);
            self.text = TextItem { output_index: Some(oi), item_id: item_id.clone(), added: true, ..TextItem::default() };
            push_sse(out, SSE_OUTPUT_ITEM_ADDED, serde_json::json!({
                "type": SSE_OUTPUT_ITEM_ADDED, "output_index": oi,
                "item": {"id": item_id, "type": "message", "status": "in_progress", "role": "assistant", "content": []}
            }));
            push_sse(out, SSE_CONTENT_PART_ADDED, serde_json::json!({
                "type": SSE_CONTENT_PART_ADDED, "item_id": item_id,
                "output_index": oi, "content_index": 0,
                "part": {"type": "output_text", "text": "", "annotations": []}
            }));
        }
        self.text.text.push_str(delta);
        push_sse(out, SSE_OUTPUT_TEXT_DELTA, serde_json::json!({
            "type": SSE_OUTPUT_TEXT_DELTA, "item_id": &self.text.item_id,
            "output_index": self.text.output_index.unwrap_or(0), "content_index": 0, "delta": delta
        }));
    }

    /// Process a tool call delta chunk and emit Responses API SSE events.
    fn push_tool_call(&mut self, tc: &Value, out: &mut String) {
        let idx = tc.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let args_delta = tc.pointer("/function/arguments").and_then(Value::as_str).unwrap_or("");
        let name_delta = tc.pointer("/function/name").and_then(Value::as_str).unwrap_or("");
        let call_id = tc.get("id").and_then(Value::as_str).unwrap_or(&format!("call_{idx}")).to_string();

        let need_index = self.tools.get(&idx).map(|s| !s.added).unwrap_or(true);
        let next_idx = if !self.tools.contains_key(&idx) || need_index { Some(self.next_output_index()) } else { None };

        let state = self.tools.entry(idx).or_insert_with(|| ToolCallState {
            call_id: call_id.clone(),
            name: name_delta.to_string(),
            ..ToolCallState::default()
        });
        if !name_delta.is_empty() { state.name = name_delta.to_string(); }
        if !args_delta.is_empty() { state.arguments.push_str(args_delta); }

        if !state.added && !state.name.is_empty() {
            let assigned = next_idx.unwrap_or(0);
            state.output_index = Some(assigned);
            state.item_id = format!("fc_{}", state.call_id);
            state.added = true;

            let is_custom = self.tool_context.is_custom_tool_proxy(&state.name);
            let original_name = self.tool_context.original_custom_tool_name(&state.name);
            let item_type = if is_custom { "custom_tool_call" } else { "function_call" };
            let name_for_sse = if is_custom { &original_name } else { &state.name };

            push_sse(out, SSE_OUTPUT_ITEM_ADDED, serde_json::json!({
                "type": SSE_OUTPUT_ITEM_ADDED, "output_index": assigned,
                "item": {"id": &state.item_id, "type": item_type, "status": "in_progress",
                         "call_id": &state.call_id, "name": name_for_sse,
                         "arguments": "", "input": ""}
            }));
        }
        if !args_delta.is_empty() {
            let is_custom = self.tool_context.is_custom_tool_proxy(&state.name);
            if !is_custom {
                if let Some(oi) = state.output_index {
                    push_sse(out, SSE_FUNC_ARGS_DELTA, serde_json::json!({
                        "type": SSE_FUNC_ARGS_DELTA, "item_id": &state.item_id,
                        "output_index": oi, "delta": args_delta
                    }));
                }
            }
        }
    }

    /// Emit completion events and finalize the response.
    fn finalize_into(&mut self, out: &mut String) {
        if self.completed { return; }
        self.ensure_started(out);
        self.flush_inline_think(out);
        self.finalize_reasoning(out);
        self.finalize_text(out);
        self.finalize_tools(out);

        let status = if self.finish_reason.as_deref() == Some("length") { "incomplete" } else { "completed" };
        let mut response = json_response(SSE_RESPONSE_COMPLETED, status, self);
        if status == "incomplete" { response["incomplete_details"] = serde_json::json!({"reason": "max_output_tokens"}); }
        copy_original_fields(&mut response, self.original_request.as_ref());
        push_sse(out, SSE_RESPONSE_COMPLETED, serde_json::json!({"type": SSE_RESPONSE_COMPLETED, "response": response}));
        out.push_str("data: [DONE]\n\n");
        self.completed = true;
    }

    /// Emit reasoning completion SSE events.
    fn finalize_reasoning(&mut self, out: &mut String) {
        if !self.reasoning.added || self.reasoning.done { return; }
        let oi = self.reasoning.output_index.unwrap_or(0);
        let item = serde_json::json!({
            "id": self.reasoning.item_id, "type": "reasoning",
            "reasoning_content": self.reasoning.text,
            "summary": [{"type": "summary_text", "text": self.reasoning.text}]
        });
        self.output_items.push((oi, item.clone()));
        self.reasoning.done = true;
        push_sse(out, SSE_REASONING_DONE, serde_json::json!({"type": SSE_REASONING_DONE, "item_id": &self.reasoning.item_id, "output_index": oi, "summary_index": 0, "text": &self.reasoning.text}));
        push_sse(out, SSE_REASONING_PART_DONE, serde_json::json!({"type": SSE_REASONING_PART_DONE, "item_id": &self.reasoning.item_id, "output_index": oi, "summary_index": 0, "part": {"type": "summary_text", "text": &self.reasoning.text}}));
        push_sse(out, SSE_OUTPUT_ITEM_DONE, serde_json::json!({"type": SSE_OUTPUT_ITEM_DONE, "output_index": oi, "item": item}));
    }

    /// Emit text completion SSE events.
    fn finalize_text(&mut self, out: &mut String) {
        if !self.text.added || self.text.done { return; }
        let oi = self.text.output_index.unwrap_or(0);
        let item = serde_json::json!({
            "id": self.text.item_id, "type": "message", "status": "completed", "role": "assistant",
            "content": [{"type": "output_text", "text": &self.text.text, "annotations": []}]
        });
        self.output_items.push((oi, item.clone()));
        self.text.done = true;
        push_sse(out, SSE_OUTPUT_TEXT_DONE, serde_json::json!({"type": SSE_OUTPUT_TEXT_DONE, "item_id": &self.text.item_id, "output_index": oi, "content_index": 0, "text": &self.text.text}));
        push_sse(out, SSE_CONTENT_PART_DONE, serde_json::json!({"type": SSE_CONTENT_PART_DONE, "item_id": &self.text.item_id, "output_index": oi, "content_index": 0, "part": {"type": "output_text", "text": &self.text.text, "annotations": []}}));
        push_sse(out, SSE_OUTPUT_ITEM_DONE, serde_json::json!({"type": SSE_OUTPUT_ITEM_DONE, "output_index": oi, "item": item}));
    }

    /// Emit tool call completion SSE events.
    fn finalize_tools(&mut self, out: &mut String) {
        let keys: Vec<usize> = self.tools.keys().copied().collect();
        let mut pending_indices: Vec<(usize, u32)> = Vec::new();
        for &key in &keys {
            if self.tools.get(&key).map(|s| s.done).unwrap_or(true) { continue; }
            if let Some(state) = self.tools.get(&key) {
                if !state.added {
                    let assigned = self.next_output_index();
                    pending_indices.push((key, assigned));
                }
            }
        }
        for (key, assigned) in pending_indices {
            let state = self.tools.get_mut(&key).expect("tool state");
            state.added = true;
            if state.name.is_empty() { state.name = "unknown_tool".to_string(); }
            state.output_index = Some(assigned);
            state.item_id = format!("fc_{}", state.call_id);
            push_sse(out, SSE_OUTPUT_ITEM_ADDED, serde_json::json!({
                "type": SSE_OUTPUT_ITEM_ADDED, "output_index": assigned,
                "item": {"id": &state.item_id, "type": "function_call", "status": "in_progress","call_id": &state.call_id, "name": &state.name, "arguments": ""}
            }));
        }
        for key in keys {
            let state = match self.tools.get_mut(&key) {
                Some(s) if !s.done => s,
                _ => continue,
            };
            let oi = state.output_index.unwrap_or(0);

            let is_custom = self.tool_context.is_custom_tool_proxy(&state.name);
            let item = if is_custom {
                custom_tool_output_item(state, &self.tool_context)
            } else {
                serde_json::json!({
                    "id": &state.item_id, "type": "function_call", "status": "completed",
                    "call_id": &state.call_id, "name": &state.name, "arguments": &state.arguments
                })
            };

            state.done = true;
            self.output_items.push((oi, item.clone()));

            if !is_custom {
                push_sse(out, SSE_FUNC_ARGS_DONE, serde_json::json!({"type": SSE_FUNC_ARGS_DONE, "item_id": &state.item_id, "output_index": oi, "arguments": &state.arguments}));
            }

            push_sse(out, SSE_OUTPUT_ITEM_DONE, serde_json::json!({"type": SSE_OUTPUT_ITEM_DONE, "output_index": oi, "item": item}));
        }
    }

    /// Emit error SSE events and mark the response as failed.
    fn failed_into(&mut self, out: &mut String, message: String, error_type: Option<String>) {
        self.completed = true;
        let mut error = serde_json::json!({"message": message});
        if let Some(et) = error_type.filter(|v| !v.is_empty()) { error["type"] = serde_json::json!(et); }
        let mut response = json_response(SSE_RESPONSE_FAILED, "failed", self);
        response["error"] = error;
        push_sse(out, SSE_RESPONSE_FAILED, serde_json::json!({"type": SSE_RESPONSE_FAILED, "response": response}));
    }

    fn next_output_index(&mut self) -> u32 {
        let idx = self.next_output_index;
        self.next_output_index += 1;
        idx
    }
}

/// Build a JSON response object from the current converter state.
fn json_response(_event: &str, status: &str, state: &ChatSseState) -> Value {
    let mut items = state.output_items.clone();
    items.sort_by_key(|(i, _)| *i);
    serde_json::json!({
        "id": state.response_id, "object": "response", "created_at": state.created_at,
        "status": status, "model": state.model,
        "output": items.into_iter().map(|(_, item)| item).collect::<Vec<_>>(),
        "usage": state.latest_usage.clone().unwrap_or_else(|| serde_json::json!({"input_tokens": 0, "output_tokens": 0, "total_tokens": 0}))
    })
}

/// Copy safe scalar fields from the original request into the response.
/// NOTE: tools and instructions are NOT copied — they are request parameters,
/// not response fields. Copying them (especially tool schemas) into every
/// response.completed event would bloat the conversation history massively.
fn copy_original_fields(response: &mut Value, orig: Option<&Value>) {
    let Some(orig) = orig else { return; };
    for key in ["max_output_tokens", "parallel_tool_calls", "previous_response_id",
                 "reasoning", "temperature", "tool_choice", "top_p", "metadata"] {
        if let Some(v) = orig.get(key) { response[key] = v.clone(); }
    }
}

/// Build a usage stats object from Chat Completions API usage data.
fn chat_usage(usage: Option<&Value>) -> Value {
    let Some(usage) = usage.filter(|v| v.is_object() && !v.is_null()) else {
        return serde_json::json!({"input_tokens": 0, "output_tokens": 0, "total_tokens": 0});
    };
    let inp = usage.get("prompt_tokens").or_else(|| usage.get("input_tokens")).and_then(Value::as_u64).unwrap_or(0);
    let out = usage.get("completion_tokens").or_else(|| usage.get("output_tokens")).and_then(Value::as_u64).unwrap_or(0);
    let tot = usage.get("total_tokens").and_then(Value::as_u64).unwrap_or(inp + out);
    serde_json::json!({"input_tokens": inp, "output_tokens": out, "total_tokens": tot})
}

/// Extract reasoning content text from a delta chunk.
fn reasoning_text(delta: &Value) -> Option<String> {
    for key in ["reasoning_content", "reasoning"] {
        if let Some(text) = delta.get(key).and_then(Value::as_str) {
            if !text.is_empty() { return Some(text.to_string()); }
        }
    }
    if let Some(r) = delta.get("reasoning") {
        for key in ["content", "text", "summary"] {
            if let Some(text) = r.get(key).and_then(Value::as_str) {
                if !text.is_empty() { return Some(text.to_string()); }
            }
        }
    }
    None
}

/// Extract error message and type from a Chat SSE error chunk.
fn extract_chat_error(value: &Value) -> (String, Option<String>) {
    let error = value.get("error").unwrap_or(value);
    let msg = error.as_str().map(|s| s.to_string())
        .or_else(|| error.get("message").or_else(|| error.get("detail")).and_then(Value::as_str).map(|s| s.to_string()))
        .unwrap_or_else(|| error.to_string());
    let err_type = error.get("type").or_else(|| error.get("code")).and_then(Value::as_str).map(|s| s.to_string());
    (msg, err_type)
}

enum ThinkDecision { NeedMore, Reasoning, Text }

/// Determine whether a text buffer contains think tags.
fn think_decision(buffer: &str) -> ThinkDecision {
    let trimmed = buffer.trim_start();
    if trimmed.is_empty() { return ThinkDecision::NeedMore; }
    if trimmed.starts_with(THINK_OPEN) { return ThinkDecision::Reasoning; }
    if THINK_OPEN.starts_with(trimmed) { return ThinkDecision::NeedMore; }
    ThinkDecision::Text
}

/// Split text by <think>...</think> tags into reasoning and visible text.
fn split_think_block(text: &str) -> Option<(String, String)> {
    let leading_ws = text.len() - text.trim_start().len();
    let after_ws = &text[leading_ws..];
    if !after_ws.starts_with(THINK_OPEN) { return None; }
    let body_start = leading_ws + THINK_OPEN.len();
    let close_rel = text[body_start..].find(THINK_CLOSE)?;
    let close_start = body_start + close_rel;
    let answer_start = close_start + THINK_CLOSE.len();
    Some((text[body_start..close_start].trim().to_string(), text[answer_start..].trim_start_matches(['\r','\n','\t',' ']).to_string()))
}


// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::core::traits::SseTransform;

    fn chat_chunk(text: &str) -> Vec<u8> {
        let data = serde_json::json!({
            "choices": [{"delta": {"content": text}, "index": 0}]
        });
        format!("data: {}

", serde_json::to_string(&data).unwrap()).into_bytes()
    }

    fn chat_reasoning_chunk(text: &str) -> Vec<u8> {
        let data = serde_json::json!({
            "choices": [{"delta": {"reasoning_content": text, "content": ""}, "index": 0}]
        });
        format!("data: {}

", serde_json::to_string(&data).unwrap()).into_bytes()
    }

    fn chat_tool_chunk(id: &str, name: &str, args: &str) -> Vec<u8> {
        let mut delta = serde_json::json!({});
        if !id.is_empty() {
            delta["tool_calls"] = serde_json::json!([{
                "index": 0, "id": id, "type": "function",
                "function": {"name": name, "arguments": args}
            }]);
        } else {
            delta["tool_calls"] = serde_json::json!([{
                "index": 0, "function": {"arguments": args}
            }]);
        }
        let data = serde_json::json!({"choices": [{"delta": delta, "index": 0}]});
        format!("data: {}

", serde_json::to_string(&data).unwrap()).into_bytes()
    }

    fn chat_usage_chunk() -> Vec<u8> {
        let data = serde_json::json!({
            "choices": [{"delta": {}, "index": 0, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });
        format!("data: {}

", serde_json::to_string(&data).unwrap()).into_bytes()
    }

    #[test]
    fn test_chat_sse_basic_text() {
        let mut conv = ChatSseConverter::default();
        let out = conv.push_bytes(&chat_chunk("Hello"));
        let out2 = conv.push_bytes(&chat_chunk(" world!"));
        let done = conv.finish();
        let combined = [out, out2, done].concat();
        let all = String::from_utf8_lossy(&combined);
        assert!(all.contains("response.output_text.delta"));
        assert!(all.contains("response.completed"));
    }

    #[test]
    fn test_chat_sse_reasoning() {
        let mut conv = ChatSseConverter::default();
        let out = conv.push_bytes(&chat_reasoning_chunk("Let me think..."));
        let out2 = conv.push_bytes(&chat_chunk("Answer is 42."));
        let done = conv.finish();
        let combined = [out, out2, done].concat();
        let all = String::from_utf8_lossy(&combined);
        assert!(all.contains("reasoning_summary_text.delta"));
        assert!(all.contains("response.output_text.delta"));
    }

    #[test]
    fn test_chat_sse_tool_calls() {
        let mut conv = ChatSseConverter::default();
        let out = conv.push_bytes(&chat_tool_chunk("call_1", "get_weather", r#"{"loc"#));
        let out2 = conv.push_bytes(&chat_tool_chunk("", "", r#""ation": "Beijing"}"#));
        let done = conv.finish();
        let combined = [out, out2, done].concat();
        let all = String::from_utf8_lossy(&combined);
        assert!(all.contains("function_call_arguments.delta"));
    }

    #[test]
    fn test_chat_sse_error_event() {
        let mut conv = ChatSseConverter::default();
        let failed = conv.fail("Rate limit exceeded".into(), Some("rate_limit_error".into()));
        let text = String::from_utf8_lossy(&failed);
        assert!(text.contains("response.failed"));
    }



    #[test]
    fn test_chat_sse_empty_input() {
        let mut conv = ChatSseConverter::default();
        let out = conv.push_bytes(b"");
        assert!(out.is_empty());
        let done = conv.finish();
        assert!(String::from_utf8_lossy(&done).contains("completed"));
    }

    #[test]
    fn test_chat_sse_usage_in_final() {
        let mut conv = ChatSseConverter::default();
        let _ = conv.push_bytes(&chat_chunk("Hello"));
        let _ = conv.push_bytes(&chat_usage_chunk());
        let done = conv.finish();
        let text = String::from_utf8_lossy(&done);
        assert!(text.contains("input_tokens"));
    }
}
