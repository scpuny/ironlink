// ── SSE parsing and stream transformation ──

use bytes::Bytes;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures::Stream;
use std::io;

/// A single parsed SSE event.
#[derive(Debug)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

/// SSE parser that handles cross-chunk boundaries.
pub struct SseParser {
    buffer: Vec<u8>,
}

impl SseParser {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn push(&mut self, data: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(data);
        let mut events = Vec::new();

        loop {
            let sep = self.buffer.windows(2).position(|w| w == b"\n\n");
            let sep2 = self.buffer.windows(4).position(|w| w == b"\r\n\r\n");
            let pos = match (sep, sep2) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => break,
            };

            if let Some(pos) = pos {
                let raw: Vec<u8> = self.buffer.drain(..=pos + 1).collect();
                if let Some(event) = parse_sse_event(&raw) {
                    events.push(event);
                }
            } else {
                break;
            }
        }
        events
    }
}

fn parse_sse_event(raw: &[u8]) -> Option<SseEvent> {
    let text = std::str::from_utf8(raw).ok()?;
    let mut event = String::from("message");
    let mut data = String::new();

    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(val) = line.strip_prefix("event:") {
            event = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("data:") {
            data = val.trim().to_string();
        }
    }

    if data.is_empty() {
        return None;
    }
    Some(SseEvent { event, data })
}

// ── Response ID generation ──

fn gen_id() -> String {
    use uuid::Uuid;
    format!("resp_{}", &Uuid::new_v4().to_string().replace('-', "")[..16])
}

fn gen_item_id() -> String {
    use uuid::Uuid;
    format!("item_{}", &Uuid::new_v4().to_string().replace('-', "")[..16])
}

// ── Responses API lifecycle event builders ──

fn response_created_event(response_id: &str, model: &str) -> Bytes {
    let sse_data = serde_json::json!({
        "type": "response.created",
        "response": {
            "id": response_id,
            "object": "response",
            "model": model,
            "output": [],
            "usage": null,
        }
    });
    Bytes::from(format!("data: {}\n\n", sse_data))
}

fn output_item_added_event(item_id: &str) -> Bytes {
    let sse_data = serde_json::json!({
        "type": "response.output_item.added",
        "item": {
            "id": item_id,
            "type": "message",
            "role": "assistant",
            "content": []
        }
    });
    Bytes::from(format!("data: {}\n\n", sse_data))
}

fn content_part_added_event(content_type: &str) -> Bytes {
    let content = match content_type {
        "thinking" => serde_json::json!([{"type": "thinking", "thinking": ""}]),
        _ => serde_json::json!([{"type": "output_text", "text": ""}]),
    };
    let sse_data = serde_json::json!({
        "type": "response.content_part.added",
        "part_index": 0,
        "content": content,
    });
    Bytes::from(format!("data: {}\n\n", sse_data))
}

fn output_text_delta_event(item_id: &str, delta: &str) -> Bytes {
    let sse_data = serde_json::json!({
        "type": "response.output_text.delta",
        "delta": delta,
        "item_id": item_id,
        "output_index": 0,
        "content_index": 0,
    });
    Bytes::from(format!("data: {}\n\n", sse_data))
}

fn output_text_done_event(item_id: &str, text: &str) -> Bytes {
    let sse_data = serde_json::json!({
        "type": "response.output_text.done",
        "item_id": item_id,
        "output_index": 0,
        "content_index": 0,
        "text": text,
    });
    Bytes::from(format!("data: {}\n\n", sse_data))
}

fn thinking_delta_event(item_id: &str, delta: &str) -> Bytes {
    let sse_data = serde_json::json!({
        "type": "response.thinking.delta",
        "delta": delta,
        "item_id": item_id,
        "output_index": 0,
        "content_index": 0,
    });
    Bytes::from(format!("data: {}\n\n", sse_data))
}

fn thinking_done_event(item_id: &str, thinking: &str) -> Bytes {
    let sse_data = serde_json::json!({
        "type": "response.thinking.done",
        "item_id": item_id,
        "output_index": 0,
        "content_index": 0,
        "thinking": thinking,
    });
    Bytes::from(format!("data: {}\n\n", sse_data))
}

fn output_item_done_event(item_id: &str, thinking: &str, text: &str) -> Bytes {
    let mut content = Vec::new();
    if !thinking.is_empty() {
        content.push(serde_json::json!({"type": "thinking", "thinking": thinking}));
    }
    if !text.is_empty() {
        content.push(serde_json::json!({"type": "output_text", "text": text}));
    }
    if content.is_empty() {
        content.push(serde_json::json!({"type": "output_text", "text": ""}));
    }
    let sse_data = serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "id": item_id,
            "type": "message",
            "role": "assistant",
            "content": content,
        }
    });
    Bytes::from(format!("data: {}\n\n", sse_data))
}

fn response_completed_event(response_id: &str, item_id: &str, model: &str, thinking: &str, text: &str) -> Bytes {
    let mut content = Vec::new();
    if !thinking.is_empty() {
        content.push(serde_json::json!({"type": "thinking", "thinking": thinking}));
    }
    if !text.is_empty() {
        content.push(serde_json::json!({"type": "output_text", "text": text}));
    }
    if content.is_empty() {
        content.push(serde_json::json!({"type": "output_text", "text": ""}));
    }
    let sse_data = serde_json::json!({
        "type": "response.completed",
        "response": {
            "id": response_id,
            "object": "response",
            "model": model,
            "output": [{
                "id": item_id,
                "type": "message",
                "role": "assistant",
                "content": content,
            }],
            "usage": {
                "input_tokens": 0,
                "output_tokens": 0,
                "total_tokens": 0
            }
        }
    });
    Bytes::from(format!("data: {}\n\n", sse_data))
}

// ── Stream transformer ──

pub struct SseTransformStream<S> {
    inner: S,
    parser: SseParser,
    is_chat: bool,
    pending: Vec<Bytes>,
    started: bool,
    ended: bool,
    thinking_active: bool,
    output_text_active: bool,
    accumulated_thinking: String,
    accumulated_text: String,
    item_id: String,
    response_id: String,
    model: String,
}

impl<S> SseTransformStream<S>
where
    S: Stream<Item = io::Result<Bytes>> + Unpin,
{
    pub fn new(inner: S, is_chat: bool) -> Self {
        Self {
            inner,
            parser: SseParser::new(),
            is_chat,
            pending: Vec::new(),
            started: false,
            ended: false,
            thinking_active: false,
            output_text_active: false,
            accumulated_thinking: String::new(),
            accumulated_text: String::new(),
            item_id: gen_item_id(),
            response_id: gen_id(),
            model: String::new(),
        }
    }
}

impl<S> Stream for SseTransformStream<S>
where
    S: Stream<Item = io::Result<Bytes>> + Unpin,
{
    type Item = io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if !self.pending.is_empty() {
            return Poll::Ready(Some(Ok(self.pending.remove(0))));
        }
        if self.ended {
            return Poll::Ready(None);
        }

        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    let events = self.parser.push(&chunk);
                    for event in &events {
                        if self.is_chat {
                            self.transform_chat_event(event);
                        } else {
                            self.transform_anthropic_event(event);
                        }
                    }
                    if !self.pending.is_empty() {
                        return Poll::Ready(Some(Ok(self.pending.remove(0))));
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    if self.started && !self.ended {
                        self.emit_end_events();
                    }
                    return Poll::Ready(Some(Err(e)));
                }
                Poll::Ready(None) => {
                    if self.started && !self.ended {
                        self.emit_end_events();
                        if !self.pending.is_empty() {
                            return Poll::Ready(Some(Ok(self.pending.remove(0))));
                        }
                    }
                    self.ended = true;
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl<S> SseTransformStream<S> {
    fn ensure_started(&mut self) {
        if !self.started {
            self.started = true;
            self.pending.push(response_created_event(&self.response_id, &self.model));
            self.pending.push(output_item_added_event(&self.item_id));
        }
    }

    fn close_thinking(&mut self) {
        if self.thinking_active {
            self.thinking_active = false;
            self.pending.push(thinking_done_event(&self.item_id, &self.accumulated_thinking));
        }
    }

    fn emit_end_events(&mut self) {
        self.ended = true;
        if self.thinking_active {
            self.pending.push(thinking_done_event(&self.item_id, &self.accumulated_thinking));
            self.thinking_active = false;
        }
        if self.output_text_active {
            self.pending.push(output_text_done_event(&self.item_id, &self.accumulated_text));
        }
        self.pending.push(output_item_done_event(&self.item_id, &self.accumulated_thinking, &self.accumulated_text));
        self.pending.push(response_completed_event(&self.response_id, &self.item_id, &self.model, &self.accumulated_thinking, &self.accumulated_text));
    }

    fn transform_chat_event(&mut self, event: &SseEvent) {
        if event.data == "[DONE]" {
            self.emit_end_events();
            return;
        }

        let val: serde_json::Value = match serde_json::from_str(&event.data) {
            Ok(v) => v,
            Err(_) => return,
        };

        if self.model.is_empty() {
            if let Some(m) = val.get("model").and_then(|v| v.as_str()) {
                self.model = m.to_string();
            }
        }

        // Extract finish_reason from the choice — if present and non-null, the stream is ending
        let choice = val.get("choices").and_then(|a| a.as_array()).and_then(|a| a.first());
        let finish_reason = choice.and_then(|c| c.get("finish_reason")).and_then(|v| v.as_str());

        let delta = choice.and_then(|c| c.get("delta"));

        let reasoning_text = delta.and_then(|d| d.get("reasoning_content")).and_then(|v| v.as_str()).unwrap_or("");
        let content_text = delta.and_then(|d| d.get("content")).and_then(|v| v.as_str()).unwrap_or("");

        // ── Handle thinking (reasoning_content) ──
        if !reasoning_text.is_empty() {
            self.ensure_started();
            if !self.thinking_active {
                self.close_thinking();
                self.pending.push(content_part_added_event("thinking"));
                self.thinking_active = true;
            }
            self.accumulated_thinking.push_str(reasoning_text);
            self.pending.push(thinking_delta_event(&self.item_id, reasoning_text));
        }

        // ── Handle regular content (output_text) ──
        if !content_text.is_empty() {
            self.ensure_started();
            if self.thinking_active {
                self.close_thinking();
            }
            if !self.output_text_active {
                self.pending.push(content_part_added_event("output_text"));
                self.output_text_active = true;
            }
            self.accumulated_text.push_str(content_text);
            self.pending.push(output_text_delta_event(&self.item_id, content_text));
        }

        // If finish_reason is present (non-null), emit end events
        if let Some(reason) = finish_reason {
            if !reason.is_empty() {
                self.emit_end_events();
            }
        }
    }

    /// Transform an Anthropic Messages SSE event into Responses API events.
    /// Supports both text and thinking/reasoning content blocks.
    fn transform_anthropic_event(&mut self, event: &SseEvent) {
        let val: serde_json::Value = match serde_json::from_str(&event.data) {
            Ok(v) => v,
            Err(_) => return,
        };

        match event.event.as_str() {
            "message_start" => {
                if self.model.is_empty() {
                    if let Some(m) = val.get("message").and_then(|m| m.get("model")).and_then(|v| v.as_str()) {
                        self.model = m.to_string();
                    }
                }
                self.ensure_started();
            }
            "content_block_start" => {
                self.ensure_started();
                let block_type = val.get("content_block").and_then(|b| b.get("type")).and_then(|v| v.as_str()).unwrap_or("text");
                let text = val.get("content_block").and_then(|b| b.get("text")).and_then(|v| v.as_str()).unwrap_or("");

                match block_type {
                    "thinking" | "reasoning" | "thinking_delta" | "signature" => {
                        if !self.thinking_active {
                            self.close_thinking();
                            self.pending.push(content_part_added_event("thinking"));
                            self.thinking_active = true;
                        }
                        if !text.is_empty() {
                            self.accumulated_thinking.push_str(text);
                            self.pending.push(thinking_delta_event(&self.item_id, text));
                        }
                    }
                    _ => {
                        if !self.output_text_active {
                            self.pending.push(content_part_added_event("output_text"));
                            self.output_text_active = true;
                        }
                        if !text.is_empty() {
                            self.accumulated_text.push_str(text);
                            self.pending.push(output_text_delta_event(&self.item_id, text));
                        }
                    }
                }
            }
            "content_block_delta" => {
                self.ensure_started();
                let delta_type = val.get("delta").and_then(|d| d.get("type")).and_then(|v| v.as_str()).unwrap_or("text_delta");
                let text = val.get("delta").and_then(|d| d.get("text")).and_then(|v| v.as_str()).unwrap_or("");

                match delta_type {
                    "thinking_delta" | "reasoning_delta" | "signature_delta" => {
                        if !self.thinking_active {
                            self.close_thinking();
                            self.pending.push(content_part_added_event("thinking"));
                            self.thinking_active = true;
                        }
                        if !text.is_empty() {
                            self.accumulated_thinking.push_str(text);
                            self.pending.push(thinking_delta_event(&self.item_id, text));
                        }
                    }
                    _ => {
                        if !self.output_text_active {
                            self.pending.push(content_part_added_event("output_text"));
                            self.output_text_active = true;
                        }
                        if !text.is_empty() {
                            self.accumulated_text.push_str(text);
                            self.pending.push(output_text_delta_event(&self.item_id, text));
                        }
                    }
                }
            }
            "content_block_stop" => {
                // Close any active blocks when the block ends
                if self.thinking_active {
                    self.close_thinking();
                }
            }
            "message_delta" => {
                // Contains stop_reason and usage — nothing to emit for SSE
            }
            "message_stop" => {
                if self.thinking_active {
                    self.close_thinking();
                }
                self.emit_end_events();
            }
            "ping" => {
                // Anthropic ping events — ignore
            }
            _ => {
                // Unknown events — ignore
            }
        }
    }
}

