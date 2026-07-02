// ── Anthropic SSE → Responses API SSE ──
//!
//! Stateful converter that tracks item_ids and emits all required Responses API events.

use serde_json::Value;
use crate::protocol::sse::parser::parse_sse_block;

/// State machine converting Anthropic SSE chunks into Responses API SSE events.
pub struct AnthropicSseConverter {
    /// Response ID extracted from message_start
    response_id: String,
    /// Current item_id for the active content block
    item_id_prefix: String,
    /// Whether message_start has been received
    started: bool,
    /// Whether message_stop has been received
    stopped: bool,
    /// Accumulated usage from message_delta
    usage: Value,
    /// Model name from message_start
    model: String,
}

impl Default for AnthropicSseConverter {
    fn default() -> Self {
        Self {
            response_id: String::new(),
            item_id_prefix: String::new(),
            started: false,
            stopped: false,
            usage: serde_json::json!({"input_tokens": 0, "output_tokens": 0}),
            model: String::new(),
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

    fn transform_event(&mut self, event: &crate::protocol::core::types::SseEvent) -> String {
        let val: Value = match serde_json::from_str(&event.data) {
            Ok(v) => v, Err(_) => return String::new(),
        };
        let mut output = String::new();

        match event.event.as_str() {
            "message_start" => {
                let id = val.pointer("/message/id").and_then(Value::as_str).unwrap_or("unknown");
                self.model = val.pointer("/message/model").and_then(Value::as_str).unwrap_or("claude").to_string();
                self.response_id = format!("resp_{}", &id[..8.min(id.len())]);
                self.item_id_prefix = format!("item_{}", &id[..8.min(id.len())]);
                self.started = true;

                push_anthropic_sse(&mut output, "response.created", serde_json::json!({
                    "type": "response.created",
                    "response": {"id": self.response_id, "object": "response", "model": self.model, "output": [], "usage": null}
                }));
                push_anthropic_sse(&mut output, "response.in_progress", serde_json::json!({
                    "type": "response.in_progress",
                    "response": {"id": self.response_id, "object": "response", "model": self.model, "output": [], "usage": null}
                }));
            }
            "content_block_start" => {
                let block_type = val.pointer("/content_block/type").and_then(Value::as_str).unwrap_or("text");
                let text = val.pointer("/content_block/text").and_then(Value::as_str).unwrap_or("");
                let block_idx = val.get("index").and_then(Value::as_u64).unwrap_or(0);
                let item_id = format!("{}_{}", self.item_id_prefix, block_idx);

                match block_type {
                    "thinking" | "reasoning" => {
                        push_anthropic_sse(&mut output, "response.output_item.added", serde_json::json!({
                            "type": "response.output_item.added", "output_index": block_idx,
                            "item": {"id": item_id, "type": "reasoning", "status": "in_progress", "reasoning_content": "", "summary": []}
                        }));
                        push_anthropic_sse(&mut output, "response.reasoning_summary_part.added", serde_json::json!({
                            "type": "response.reasoning_summary_part.added", "item_id": item_id,
                            "output_index": block_idx, "summary_index": 0,
                            "part": {"type": "summary_text", "text": ""}
                        }));
                        if !text.is_empty() {
                            push_anthropic_sse(&mut output, "response.reasoning_summary_text.delta", serde_json::json!({
                                "type": "response.reasoning_summary_text.delta", "item_id": item_id,
                                "output_index": block_idx, "summary_index": 0, "delta": text
                            }));
                        }
                    }
                    _ => {
                        push_anthropic_sse(&mut output, "response.output_item.added", serde_json::json!({
                            "type": "response.output_item.added", "output_index": block_idx,
                            "item": {"id": item_id, "type": "message", "status": "in_progress", "role": "assistant", "content": []}
                        }));
                        push_anthropic_sse(&mut output, "response.content_part.added", serde_json::json!({
                            "type": "response.content_part.added", "item_id": item_id,
                            "output_index": block_idx, "content_index": 0,
                            "part": {"type": "output_text", "text": "", "annotations": []}
                        }));
                        if !text.is_empty() {
                            push_anthropic_sse(&mut output, "response.output_text.delta", serde_json::json!({
                                "type": "response.output_text.delta", "item_id": item_id,
                                "output_index": block_idx, "content_index": 0, "delta": text
                            }));
                        }
                    }
                }
            }
            "content_block_delta" => {
                let delta_type = val.pointer("/delta/type").and_then(Value::as_str).unwrap_or("text_delta");
                let text = val.pointer("/delta/text").and_then(Value::as_str).unwrap_or("");
                let block_idx = val.get("index").and_then(Value::as_u64).unwrap_or(0);
                let item_id = format!("{}_{}", self.item_id_prefix, block_idx);

                if text.is_empty() { return String::new(); }

                match delta_type {
                    "thinking_delta" | "reasoning_delta" | "signature_delta" => {
                        push_anthropic_sse(&mut output, "response.reasoning_summary_text.delta", serde_json::json!({
                            "type": "response.reasoning_summary_text.delta", "item_id": item_id,
                            "output_index": block_idx, "summary_index": 0, "delta": text
                        }));
                    }
                    _ => {
                        push_anthropic_sse(&mut output, "response.output_text.delta", serde_json::json!({
                            "type": "response.output_text.delta", "item_id": item_id,
                            "output_index": block_idx, "content_index": 0, "delta": text
                        }));
                    }
                }
            }
            "content_block_stop" => {
                let block_idx = val.get("index").and_then(Value::as_u64).unwrap_or(0);
                let item_id = format!("{}_{}", self.item_id_prefix, block_idx);

                push_anthropic_sse(&mut output, "response.content_part.done", serde_json::json!({
                    "type": "response.content_part.done", "item_id": item_id,
                    "output_index": block_idx, "content_index": 0,
                    "part": {"type": "output_text", "text": "", "annotations": []}
                }));
                // Emit output_item.done to complete the item
                push_anthropic_sse(&mut output, "response.output_item.done", serde_json::json!({
                    "type": "response.output_item.done", "output_index": block_idx,
                    "item": {"id": item_id}
                }));
            }
            "message_delta" => {
                if let Some(usage) = val.get("usage") {
                    self.usage = usage.clone();
                }
                // delta may contain stop_reason and stop_sequence
                let stop_reason = val.pointer("/delta/stop_reason").and_then(Value::as_str).unwrap_or("end_turn");
                let status = if stop_reason == "max_tokens" { "incomplete" } else { "completed" };

                push_anthropic_sse(&mut output, "response.completed", serde_json::json!({
                    "type": "response.completed",
                    "response": {
                        "id": self.response_id, "object": "response",
                        "model": self.model, "status": status,
                        "output": [],
                        "usage": self.usage
                    }
                }));
            }
            "message_stop" => {
                self.stopped = true;
                // Already emitted response.completed from message_delta; nothing extra needed
            }
            "ping" => {
                // Anthropic sends periodic pings; ignore
            }
            _ => {}
        }
        output
    }

    /// Called after stream ends. Emits any final events if not already done.
    pub fn finish(&mut self) -> String {
        let mut output = String::new();
        if !self.stopped && self.started {
            push_anthropic_sse(&mut output, "response.completed", serde_json::json!({
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
    /// Produces a `response.failed` event so Codex sees a proper error instead of a hung stream.
    pub fn fail(&mut self, message: String, error_type: Option<String>) -> String {
        let mut output = String::new();
        let mut response = serde_json::json!({
            "id": self.response_id,
            "object": "response",
            "model": self.model,
            "status": "failed",
            "output": [],
            "usage": self.usage
        });
        let mut error = serde_json::json!({
            "message": message
        });
        if let Some(et) = error_type.filter(|v| !v.is_empty()) {
            error["type"] = serde_json::json!(et);
        }
        response["error"] = error;
        push_anthropic_sse(&mut output, "response.failed", serde_json::json!({
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

    fn make_anthropic_block(event: &str, data: &serde_json::Value) -> String {
        format!("event: {}\ndata: {}\n\n", event, serde_json::to_string(data).unwrap())
    }

    #[test]
    fn test_anthropic_sse_message_start() {
        let mut conv = AnthropicSseConverter::default();
        let data = serde_json::json!({
            "type": "message_start",
            "message": {
                "id": "msg_01abcd1234",
                "model": "claude-3-opus",
                "role": "assistant",
                "content": []
            }
        });
        let out = conv.push_block(&make_anthropic_block("message_start", &data));
        assert!(out.contains("response.created"));
        assert!(out.contains("response.in_progress"));
        assert!(out.contains("claude-3-opus"));
    }

    #[test]
    fn test_anthropic_sse_text_content() {
        let mut conv = AnthropicSseConverter::default();

        // message_start
        let start_data = serde_json::json!({
            "type": "message_start",
            "message": {"id": "msg_01test", "model": "claude-3", "content": []}
        });
        conv.push_block(&make_anthropic_block("message_start", &start_data));

        // content_block_start
        let block_data = serde_json::json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "text", "text": "Hello"}
        });
        let out = conv.push_block(&make_anthropic_block("content_block_start", &block_data));
        assert!(out.contains("response.output_item.added"));
        assert!(out.contains("response.output_text.delta"));
    }

    #[test]
    fn test_anthropic_sse_text_delta() {
        let mut conv = AnthropicSseConverter::default();
        let start_data = serde_json::json!({
            "type": "message_start",
            "message": {"id": "msg_01test", "model": "claude-3", "content": []}
        });
        conv.push_block(&make_anthropic_block("message_start", &start_data));

        let block_data = serde_json::json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "text", "text": ""}
        });
        conv.push_block(&make_anthropic_block("content_block_start", &block_data));

        let delta_data = serde_json::json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": " world"}
        });
        let out = conv.push_block(&make_anthropic_block("content_block_delta", &delta_data));
        assert!(out.contains("response.output_text.delta"));
        assert!(out.contains("world"));
    }

    #[test]
    fn test_anthropic_sse_thinking_content() {
        let mut conv = AnthropicSseConverter::default();
        let start_data = serde_json::json!({
            "type": "message_start",
            "message": {"id": "msg_01think", "model": "claude-3", "content": []}
        });
        conv.push_block(&make_anthropic_block("message_start", &start_data));

        let block_data = serde_json::json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "thinking", "text": "Let me reason..."}
        });
        let out = conv.push_block(&make_anthropic_block("content_block_start", &block_data));
        assert!(out.contains("reasoning_summary_text.delta"));
        assert!(out.contains("Let me reason"));
    }

    #[test]
    fn test_anthropic_sse_message_delta() {
        let mut conv = AnthropicSseConverter::default();
        let start_data = serde_json::json!({
            "type": "message_start",
            "message": {"id": "msg_01done", "model": "claude-3", "content": []}
        });
        conv.push_block(&make_anthropic_block("message_start", &start_data));

        let delta_data = serde_json::json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"input_tokens": 10, "output_tokens": 5}
        });
        let out = conv.push_block(&make_anthropic_block("message_delta", &delta_data));
        assert!(out.contains("response.completed"));
    }

    #[test]
    fn test_anthropic_sse_fail_event() {
        let mut conv = AnthropicSseConverter::default();
        let start_data = serde_json::json!({
            "type": "message_start",
            "message": {"id": "msg_01fail", "model": "claude-3", "content": []}
        });
        conv.push_block(&make_anthropic_block("message_start", &start_data));

        let failed = conv.fail("Upstream error".into(), Some("server_error".into()));
        assert!(failed.contains("response.failed"));
        assert!(failed.contains("Upstream error"));
        assert!(failed.contains("server_error"));
    }

    #[test]
    fn test_anthropic_sse_finish_empty() {
        let mut conv = AnthropicSseConverter::default();
        let out = conv.finish();
        assert!(out.is_empty(), "finish() before start should be empty");
    }

    #[test]
    fn test_anthropic_sse_ping_ignored() {
        let mut conv = AnthropicSseConverter::default();
        let data = serde_json::json!({"type": "ping"});
        let out = conv.push_block(&make_anthropic_block("ping", &data));
        assert!(out.is_empty(), "ping events should be ignored");
    }

    #[test]
    fn test_anthropic_sse_full_roundtrip() {
        let mut conv = AnthropicSseConverter::default();

        // Start
        let start_data = serde_json::json!({
            "type": "message_start",
            "message": {"id": "msg_01round", "model": "claude-3", "content": []}
        });
        conv.push_block(&make_anthropic_block("message_start", &start_data));

        // Content start
        let block_data = serde_json::json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "text", "text": "Hi"}
        });
        conv.push_block(&make_anthropic_block("content_block_start", &block_data));

        // Content delta
        let delta_data = serde_json::json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": " there"}
        });
        conv.push_block(&make_anthropic_block("content_block_delta", &delta_data));

        // Content stop
        let stop_data = serde_json::json!({
            "type": "content_block_stop",
            "index": 0
        });
        let out = conv.push_block(&make_anthropic_block("content_block_stop", &stop_data));
        assert!(out.contains("response.content_part.done"));
        assert!(out.contains("response.output_item.done"));

        // Message delta
        let msg_delta = serde_json::json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"input_tokens": 5, "output_tokens": 3}
        });
        let out2 = conv.push_block(&make_anthropic_block("message_delta", &msg_delta));
        assert!(out2.contains("response.completed"));

        // Finish
        let done = conv.finish();
        assert!(done.is_empty() || done.contains("response.completed"));
    }
}

