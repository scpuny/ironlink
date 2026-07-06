// ── Anthropic SSE → Responses API SSE ──
//!
//! Stateful converter that handles:
//!   - text content blocks → output_text content
//!   - tool_use content blocks → function_call output items
//!   - thinking content blocks → reasoning output items with signature tracking
//!   - input_json_delta → function_call_arguments.delta
//!   - thinking_delta with signature → signature tracked for final reasoning block

use std::collections::BTreeMap;
use serde_json::{json, Value};
use crate::protocol::sse::parser::parse_sse_block;

/// State machine converting Anthropic SSE chunks into Responses API SSE events.
pub struct AnthropicSseConverter {
    /// Response ID extracted from message_start
    response_id: String,
    /// Model name from message_start
    model: String,
    /// Whether message_start has been received
    started: bool,
    /// Whether message_stop has been received
    stopped: bool,
    /// Accumulated usage from message_delta
    usage: Value,
    /// Per-block state for tracking content blocks
    blocks: BTreeMap<u64, BlockState>,
    /// Next output index counter
    next_output_index: u32,
}

#[derive(Debug, Clone)]
struct BlockState {
    block_type: String,       // "text", "tool_use", "thinking"
    item_id: String,
    output_index: u32,
    text: String,
    arguments: String,        // for tool_use: accumulated input JSON
    tool_name: String,
    tool_call_id: String,
    thinking_signature: Option<String>, // for thinking: signature from last delta
}

impl Default for AnthropicSseConverter {
    fn default() -> Self {
        Self {
            response_id: String::new(),
            model: String::new(),
            started: false,
            stopped: false,
            usage: json!({"input_tokens": 0, "output_tokens": 0}),
            blocks: BTreeMap::new(),
            next_output_index: 0,
        }
    }
}

impl AnthropicSseConverter {
    /// Process a single SSE block from the Anthropic stream.
    /// Returns Responses API SSE text to emit.
    pub fn push_block(&mut self, block: &str) -> String {
        if block.trim().is_empty() { return String::new(); }
        let event = match parse_sse_block(block) {
            Some(e) => e,
            None => return String::new(),
        };
        self.transform_event(&event)
    }

    fn next_index(&mut self) -> u32 {
        let idx = self.next_output_index;
        self.next_output_index += 1;
        idx
    }

    fn block_id(&self, idx: u64) -> String {
        format!("block_{}_{}", &self.response_id[..8.min(self.response_id.len())], idx)
    }

    fn transform_event(&mut self, event: &crate::protocol::core::types::SseEvent) -> String {
        let val: Value = match serde_json::from_str(&event.data) {
            Ok(v) => v,
            Err(_) => return String::new(),
        };
        let mut output = String::new();

        match event.event.as_str() {
            "message_start" => {
                let id = val.pointer("/message/id")
                    .and_then(Value::as_str).unwrap_or("unknown");
                self.model = val.pointer("/message/model")
                    .and_then(Value::as_str).unwrap_or("claude").to_string();
                self.response_id = format!("resp_{}", &id[..8.min(id.len())]);
                self.started = true;

                push_anthropic_sse(&mut output, "response.created", json!({
                    "type": "response.created",
                    "response": {"id": self.response_id, "object": "response",
                                 "model": self.model, "output": [], "usage": null}
                }));
                push_anthropic_sse(&mut output, "response.in_progress", json!({
                    "type": "response.in_progress",
                    "response": {"id": self.response_id, "object": "response",
                                 "model": self.model, "output": [], "usage": null}
                }));
            }

            "content_block_start" => {
                let block_type = val.pointer("/content_block/type")
                    .and_then(Value::as_str).unwrap_or("text");
                let block_idx = val.get("index").and_then(Value::as_u64).unwrap_or(0);
                let item_id = self.block_id(block_idx);
                let oi = self.next_index();

                match block_type {
                    "thinking" => {
                        let text = val.pointer("/content_block/text")
                            .and_then(Value::as_str).unwrap_or("").to_string();
                        self.blocks.insert(block_idx, BlockState {
                            block_type: "thinking".to_string(),
                            item_id: item_id.clone(),
                            output_index: oi,
                            text: text.clone(),
                            arguments: String::new(),
                            tool_name: String::new(),
                            tool_call_id: String::new(),
                            thinking_signature: None,
                        });
                        push_anthropic_sse(&mut output, "response.output_item.added", json!({
                            "type": "response.output_item.added", "output_index": oi,
                            "item": {"id": item_id, "type": "reasoning", "status": "in_progress",
                                     "reasoning_content": "", "summary": []}
                        }));
                        push_anthropic_sse(&mut output, "response.reasoning_summary_part.added", json!({
                            "type": "response.reasoning_summary_part.added", "item_id": item_id,
                            "output_index": oi, "summary_index": 0,
                            "part": {"type": "summary_text", "text": ""}
                        }));
                        if !text.is_empty() {
                            push_anthropic_sse(&mut output, "response.reasoning_summary_text.delta", json!({
                                "type": "response.reasoning_summary_text.delta", "item_id": item_id,
                                "output_index": oi, "summary_index": 0, "delta": &text
                            }));
                        }
                    }

                    "tool_use" => {
                        let name = val.pointer("/content_block/name")
                            .and_then(Value::as_str).unwrap_or("").to_string();
                        let call_id = val.pointer("/content_block/id")
                            .and_then(Value::as_str).unwrap_or("").to_string();
                        let input_text = val.pointer("/content_block/input")
                            .and_then(|v| {
                                if v.is_string() { v.as_str().map(|s| s.to_string()) }
                                else { Some(serde_json::to_string(v).unwrap_or_default()) }
                            }).unwrap_or_default();

                        self.blocks.insert(block_idx, BlockState {
                            block_type: "tool_use".to_string(),
                            item_id: format!("fc_{}", &call_id),
                            output_index: oi,
                            text: String::new(),
                            arguments: input_text.clone(),
                            tool_name: name.clone(),
                            tool_call_id: call_id.clone(),
                            thinking_signature: None,
                        });

                        push_anthropic_sse(&mut output, "response.output_item.added", json!({
                            "type": "response.output_item.added", "output_index": oi,
                            "item": {
                                "id": format!("fc_{call_id}"), "type": "function_call",
                                "status": "in_progress",
                                "call_id": call_id, "name": name,
                                "arguments": "", "input": ""
                            }
                        }));
                        if !input_text.is_empty() {
                            push_anthropic_sse(&mut output, "response.function_call_arguments.delta", json!({
                                "type": "response.function_call_arguments.delta",
                                "item_id": format!("fc_{call_id}"),
                                "output_index": oi, "delta": &input_text
                            }));
                        }
                    }

                    // Default: text block
                    _ => {
                        let text = val.pointer("/content_block/text")
                            .and_then(Value::as_str).unwrap_or("").to_string();
                        self.blocks.insert(block_idx, BlockState {
                            block_type: "text".to_string(),
                            item_id: item_id.clone(),
                            output_index: oi,
                            text: text.clone(),
                            arguments: String::new(),
                            tool_name: String::new(),
                            tool_call_id: String::new(),
                            thinking_signature: None,
                        });
                        push_anthropic_sse(&mut output, "response.output_item.added", json!({
                            "type": "response.output_item.added", "output_index": oi,
                            "item": {"id": item_id, "type": "message", "status": "in_progress",
                                     "role": "assistant", "content": []}
                        }));
                        push_anthropic_sse(&mut output, "response.content_part.added", json!({
                            "type": "response.content_part.added", "item_id": item_id,
                            "output_index": oi, "content_index": 0,
                            "part": {"type": "output_text", "text": "", "annotations": []}
                        }));
                        if !text.is_empty() {
                            push_anthropic_sse(&mut output, "response.output_text.delta", json!({
                                "type": "response.output_text.delta", "item_id": item_id,
                                "output_index": oi, "content_index": 0, "delta": &text
                            }));
                        }
                    }
                }
            }

            "content_block_delta" => {
                let delta_type = val.pointer("/delta/type")
                    .and_then(Value::as_str).unwrap_or("text_delta");
                let block_idx = val.get("index").and_then(Value::as_u64).unwrap_or(0);
                let Some(state) = self.blocks.get_mut(&block_idx) else { return String::new(); };

                match delta_type {
                    "input_json_delta" => {
                        // Tool call argument delta
                        let partial = val.pointer("/delta/partial_json")
                            .and_then(Value::as_str).unwrap_or("");
                        if !partial.is_empty() {
                            state.arguments.push_str(partial);
                            push_anthropic_sse(&mut output, "response.function_call_arguments.delta", json!({
                                "type": "response.function_call_arguments.delta",
                                "item_id": &state.item_id,
                                "output_index": state.output_index,
                                "delta": partial
                            }));
                        }
                    }

                    "thinking_delta" => {
                        let text = val.pointer("/delta/thinking")
                            .and_then(Value::as_str).unwrap_or("");
                        if !text.is_empty() {
                            state.text.push_str(text);
                            push_anthropic_sse(&mut output, "response.reasoning_summary_text.delta", json!({
                                "type": "response.reasoning_summary_text.delta",
                                "item_id": &state.item_id,
                                "output_index": state.output_index,
                                "summary_index": 0, "delta": text
                            }));
                        }
                        // Track signature from the last thinking delta
                        if let Some(sig) = val.pointer("/delta/signature").and_then(Value::as_str) {
                            if !sig.is_empty() {
                                state.thinking_signature = Some(sig.to_string());
                            }
                        }
                    }

                    "signature_delta" => {
                        if let Some(sig) = val.pointer("/delta/signature").and_then(Value::as_str) {
                            if !sig.is_empty() {
                                state.thinking_signature = Some(sig.to_string());
                            }
                        }
                    }

                    _ => {
                        // text_delta and others
                        let text = val.pointer("/delta/text")
                            .and_then(Value::as_str).unwrap_or("");
                        if !text.is_empty() {
                            state.text.push_str(text);
                            push_anthropic_sse(&mut output, "response.output_text.delta", json!({
                                "type": "response.output_text.delta", "item_id": &state.item_id,
                                "output_index": state.output_index, "content_index": 0, "delta": text
                            }));
                        }
                    }
                }
            }

            "content_block_stop" => {
                let block_idx = val.get("index").and_then(Value::as_u64).unwrap_or(0);
                let Some(state) = self.blocks.get(&block_idx) else { return String::new(); };

                match state.block_type.as_str() {
                    "thinking" => {
                        // Emit reasoning completion
                        let oi = state.output_index;
                        let mut reasoning_item = json!({
                            "id": state.item_id, "type": "reasoning",
                            "reasoning_content": state.text,
                            "summary": [{"type": "summary_text", "text": state.text}]
                        });
                        if let Some(ref sig) = state.thinking_signature {
                            reasoning_item["signature"] = json!(sig);
                        }
                        push_anthropic_sse(&mut output, "response.reasoning_summary_text.done", json!({
                            "type": "response.reasoning_summary_text.done",
                            "item_id": &state.item_id, "output_index": oi,
                            "summary_index": 0, "text": &state.text
                        }));
                        push_anthropic_sse(&mut output, "response.reasoning_summary_part.done", json!({
                            "type": "response.reasoning_summary_part.done",
                            "item_id": &state.item_id, "output_index": oi,
                            "summary_index": 0,
                            "part": {"type": "summary_text", "text": &state.text}
                        }));
                        push_anthropic_sse(&mut output, "response.output_item.done", json!({
                            "type": "response.output_item.done", "output_index": oi,
                            "item": reasoning_item
                        }));
                    }

                    "tool_use" => {
                        // Emit function_call completion
                        let oi = state.output_index;
                        push_anthropic_sse(&mut output, "response.function_call_arguments.done", json!({
                            "type": "response.function_call_arguments.done",
                            "item_id": &state.item_id, "output_index": oi,
                            "arguments": &state.arguments
                        }));
                        push_anthropic_sse(&mut output, "response.output_item.done", json!({
                            "type": "response.output_item.done", "output_index": oi,
                            "item": {
                                "id": &state.item_id, "type": "function_call",
                                "status": "completed",
                                "call_id": state.tool_call_id,
                                "name": state.tool_name,
                                "arguments": &state.arguments
                            }
                        }));
                    }

                    _ => {
                        // Text block completion
                        let oi = state.output_index;
                        push_anthropic_sse(&mut output, "response.content_part.done", json!({
                            "type": "response.content_part.done", "item_id": &state.item_id,
                            "output_index": oi, "content_index": 0,
                            "part": {"type": "output_text", "text": &state.text, "annotations": []}
                        }));
                        push_anthropic_sse(&mut output, "response.output_text.done", json!({
                            "type": "response.output_text.done", "item_id": &state.item_id,
                            "output_index": oi, "content_index": 0, "text": &state.text
                        }));
                        push_anthropic_sse(&mut output, "response.output_item.done", json!({
                            "type": "response.output_item.done", "output_index": oi,
                            "item": {
                                "id": &state.item_id, "type": "message",
                                "status": "completed", "role": "assistant",
                                "content": [{"type": "output_text", "text": &state.text, "annotations": []}]
                            }
                        }));
                    }
                }
            }

            "message_delta" => {
                if let Some(usage) = val.get("usage") {
                    self.usage = usage.clone();
                }
                let stop_reason = val.pointer("/delta/stop_reason")
                    .and_then(Value::as_str).unwrap_or("end_turn");
                let status = if stop_reason == "max_tokens" { "incomplete" } else { "completed" };

                push_anthropic_sse(&mut output, "response.completed", json!({
                    "type": "response.completed",
                    "response": {
                        "id": self.response_id, "object": "response",
                        "model": self.model, "status": status,
                        "output": [], "usage": self.usage
                    }
                }));
            }

            "message_stop" => {
                self.stopped = true;
            }

            "ping" => {} // ignore
            _ => {}
        }
        output
    }

    /// Called after stream ends. Emits any final events if not already done.
    pub fn finish(&mut self) -> String {
        let mut output = String::new();
        if !self.stopped && self.started {
            push_anthropic_sse(&mut output, "response.completed", json!({
                "type": "response.completed",
                "response": {
                    "id": self.response_id, "object": "response",
                    "model": self.model, "status": "completed",
                    "output": [], "usage": self.usage
                }
            }));
            self.stopped = true;
        }
        output
    }

    /// Emit error SSE events when the upstream stream fails mid-stream.
    pub fn fail(&mut self, message: String, error_type: Option<String>) -> String {
        let mut output = String::new();
        let mut response = json!({
            "id": self.response_id, "object": "response",
            "model": self.model, "status": "failed",
            "output": [], "usage": self.usage
        });
        let mut error = json!({"message": message});
        if let Some(et) = error_type.filter(|v| !v.is_empty()) {
            error["type"] = json!(et);
        }
        response["error"] = error;
        push_anthropic_sse(&mut output, "response.failed", json!({
            "type": "response.failed",
            "response": response
        }));
        self.stopped = true;
        output
    }
}

fn push_anthropic_sse(output: &mut String, event: &str, data: Value) {
    output.push_str("event: ");
    output.push_str(event);
    output.push_str("\ndata: ");
    output.push_str(&serde_json::to_string(&data).unwrap_or_default());
    output.push_str("\n\n");
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(event: &str, data: &serde_json::Value) -> String {
        format!("event: {}\ndata: {}\n\n", event, serde_json::to_string(data).unwrap())
    }

    #[test]
    fn test_anthropic_sse_message_start() {
        let mut conv = AnthropicSseConverter::default();
        let data = json!({
            "type": "message_start",
            "message": {"id": "msg_01abcd1234", "model": "claude-3-opus", "role": "assistant", "content": []}
        });
        let out = conv.push_block(&make_event("message_start", &data));
        assert!(out.contains("response.created"));
        assert!(out.contains("response.in_progress"));
        assert!(out.contains("claude-3-opus"));
    }

    #[test]
    fn test_anthropic_sse_text() {
        let mut conv = AnthropicSseConverter::default();
        conv.push_block(&make_event("message_start", &json!({
            "type": "message_start", "message": {"id": "msg_01t", "model": "claude-3", "content": []}
        })));
        conv.push_block(&make_event("content_block_start", &json!({
            "type": "content_block_start", "index": 0,
            "content_block": {"type": "text", "text": "Hello"}
        })));
        let out = conv.push_block(&make_event("content_block_delta", &json!({
            "type": "content_block_delta", "index": 0,
            "delta": {"type": "text_delta", "text": " world"}
        })));
        assert!(out.contains("response.output_text.delta"));
        assert!(out.contains("world"));
    }

    #[test]
    fn test_anthropic_sse_tool_use() {
        let mut conv = AnthropicSseConverter::default();
        conv.push_block(&make_event("message_start", &json!({
            "type": "message_start", "message": {"id": "msg_01tu", "model": "claude-3", "content": []}
        })));
        let out = conv.push_block(&make_event("content_block_start", &json!({
            "type": "content_block_start", "index": 0,
            "content_block": {
                "type": "tool_use", "id": "toolu_abc", "name": "codegraph__explore",
                "input": ""
            }
        })));
        assert!(out.contains("function_call"));
        assert!(out.contains("codegraph__explore"));
        assert!(out.contains("toolu_abc"));
    }

    #[test]
    fn test_anthropic_sse_tool_args_delta() {
        let mut conv = AnthropicSseConverter::default();
        conv.push_block(&make_event("message_start", &json!({
            "type": "message_start", "message": {"id": "msg_01ta", "model": "claude-3", "content": []}
        })));
        conv.push_block(&make_event("content_block_start", &json!({
            "type": "content_block_start", "index": 0,
            "content_block": {"type": "tool_use", "id": "toolu_xyz", "name": "get_weather", "input": ""}
        })));
        let out = conv.push_block(&make_event("content_block_delta", &json!({
            "type": "content_block_delta", "index": 0,
            "delta": {"type": "input_json_delta", "partial_json": "{\"loc"}
        })));
        assert!(out.contains("function_call_arguments.delta"));
        assert!(out.contains("loc"));
    }

    #[test]
    fn test_anthropic_sse_thinking() {
        let mut conv = AnthropicSseConverter::default();
        conv.push_block(&make_event("message_start", &json!({
            "type": "message_start", "message": {"id": "msg_01th", "model": "claude-3", "content": []}
        })));
        let out = conv.push_block(&make_event("content_block_start", &json!({
            "type": "content_block_start", "index": 0,
            "content_block": {"type": "thinking", "text": "I need to reason..."}
        })));
        assert!(out.contains("reasoning_summary_text.delta"));
    }

    #[test]
    fn test_anthropic_sse_thinking_with_signature() {
        let mut conv = AnthropicSseConverter::default();
        conv.push_block(&make_event("message_start", &json!({
            "type": "message_start", "message": {"id": "msg_01sig", "model": "claude-3", "content": []}
        })));
        conv.push_block(&make_event("content_block_start", &json!({
            "type": "content_block_start", "index": 0,
            "content_block": {"type": "thinking", "text": "Step 1..."}
        })));
        let _ = conv.push_block(&make_event("content_block_delta", &json!({
            "type": "content_block_delta", "index": 0,
            "delta": {"type": "thinking_delta", "thinking": "Step 2...",
                      "signature": "sig_abc123"}
        })));
        let out = conv.push_block(&make_event("content_block_stop", &json!({
            "type": "content_block_stop", "index": 0
        })));
        assert!(out.contains("response.output_item.done"));
        // Check the in-memory state has the signature
        let block = conv.blocks.get(&0).unwrap();
        assert_eq!(block.thinking_signature.as_deref(), Some("sig_abc123"));
    }

    #[test]
    fn test_anthropic_sse_message_delta() {
        let mut conv = AnthropicSseConverter::default();
        conv.push_block(&make_event("message_start", &json!({
            "type": "message_start", "message": {"id": "msg_01md", "model": "claude-3", "content": []}
        })));
        let out = conv.push_block(&make_event("message_delta", &json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"input_tokens": 10, "output_tokens": 5}
        })));
        assert!(out.contains("response.completed"));
    }

    #[test]
    fn test_anthropic_sse_full_roundtrip() {
        let mut conv = AnthropicSseConverter::default();
        conv.push_block(&make_event("message_start", &json!({
            "type": "message_start", "message": {"id": "msg_01rt", "model": "claude-3", "content": []}
        })));
        conv.push_block(&make_event("content_block_start", &json!({
            "type": "content_block_start", "index": 0,
            "content_block": {"type": "text", "text": "Hi"}
        })));
        conv.push_block(&make_event("content_block_delta", &json!({
            "type": "content_block_delta", "index": 0,
            "delta": {"type": "text_delta", "text": " there"}
        })));
        conv.push_block(&make_event("content_block_stop", &json!({
            "type": "content_block_stop", "index": 0
        })));
        conv.push_block(&make_event("message_delta", &json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"input_tokens": 5, "output_tokens": 3}
        })));
        let done = conv.finish();
        assert!(done.is_empty() || done.contains("response.completed"));
    }
}
