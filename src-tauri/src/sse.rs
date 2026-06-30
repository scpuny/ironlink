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

    /// Push new bytes, return any complete events found.
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

fn content_part_added_event() -> Bytes {
    let sse_data = serde_json::json!({
        "type": "response.content_part.added",
        "part_index": 0,
        "content": [{"type": "output_text", "text": ""}]
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

fn output_item_done_event(item_id: &str, text: &str) -> Bytes {
    let sse_data = serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "id": item_id,
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": text}]
        }
    });
    Bytes::from(format!("data: {}\n\n", sse_data))
}

fn response_completed_event(response_id: &str, item_id: &str, model: &str, text: &str) -> Bytes {
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
                "content": [{"type": "output_text", "text": text}]
            }]
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
    /// Whether `response.created` has been emitted
    started: bool,
    /// Whether we've already emitted done/completed events
    ended: bool,
    /// Accumulated text for final events
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
        // Drain pending events first
        if !self.pending.is_empty() {
            return Poll::Ready(Some(Ok(self.pending.remove(0))));
        }

        // If we've already ended, return None
        if self.ended {
            return Poll::Ready(None);
        }

        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    let events = self.parser.push(&chunk);

                    for event in &events {
                        if self.is_chat {
                            // Transform Chat Completions SSE → Responses SSE
                            self.transform_chat_event(event);
                        } else {
                            // Transform Anthropic SSE → Responses SSE
                            self.transform_anthropic_event(event);
                        }
                    }

                    if !self.pending.is_empty() {
                        return Poll::Ready(Some(Ok(self.pending.remove(0))));
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    // On error, emit completion events if we started
                    if self.started && !self.ended {
                        self.emit_end_events();
                    }
                    return Poll::Ready(Some(Err(e)));
                }
                Poll::Ready(None) => {
                    // Stream ended naturally — emit completion events if we started
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
            self.pending
                .push(response_created_event(&self.response_id, &self.model));
            self.pending.push(output_item_added_event(&self.item_id));
            self.pending.push(content_part_added_event());
        }
    }

    fn emit_end_events(&mut self) {
        self.ended = true;
        self.pending.push(output_text_done_event(
            &self.item_id,
            &self.accumulated_text,
        ));
        self.pending.push(output_item_done_event(
            &self.item_id,
            &self.accumulated_text,
        ));
        self.pending.push(response_completed_event(
            &self.response_id,
            &self.item_id,
            &self.model,
            &self.accumulated_text,
        ));
    }

    /// Transform a Chat Completions SSE event into Responses API events.
    fn transform_chat_event(&mut self, event: &SseEvent) {
        // [DONE] signal — stream is complete
        if event.data == "[DONE]" {
            self.emit_end_events();
            return;
        }

        let val: serde_json::Value = match serde_json::from_str(&event.data) {
            Ok(v) => v,
            Err(_) => return,
        };

        // Extract model name from first chunk
        if self.model.is_empty() {
            if let Some(m) = val.get("model").and_then(|v| v.as_str()) {
                self.model = m.to_string();
            }
        }

        let choice = val
            .get("choices")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first());

        let delta = choice.and_then(|c| c.get("delta"));

        // Extract reasoning_content (DeepSeek thinking) — treat as regular text delta
        let reasoning_text = delta
            .and_then(|d| d.get("reasoning_content"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Extract regular content
        let content_text = delta
            .and_then(|d| d.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Emit reasoning_content first, then content
        if !reasoning_text.is_empty() {
            self.ensure_started();
            self.accumulated_text.push_str(reasoning_text);
            self.pending
                .push(output_text_delta_event(&self.item_id, reasoning_text));
        }

        if !content_text.is_empty() {
            self.ensure_started();
            self.accumulated_text.push_str(content_text);
            self.pending
                .push(output_text_delta_event(&self.item_id, content_text));
        }
    }

    /// Transform an Anthropic Messages SSE event into Responses API events.
    fn transform_anthropic_event(&mut self, event: &SseEvent) {
        let val: serde_json::Value = match serde_json::from_str(&event.data) {
            Ok(v) => v,
            Err(_) => return,
        };

        match event.event.as_str() {
            "message_start" => {
                // Extract model from message_start
                if self.model.is_empty() {
                    if let Some(m) = val.get("message").and_then(|m| m.get("model")).and_then(|v| v.as_str()) {
                        self.model = m.to_string();
                    }
                }
                // Extract existing content from the initial message
                // Anthropic starts with an empty content array, so no delta needed
                self.ensure_started();
            }
            "content_block_start" => {
                // Ensure lifecycle events are emitted
                self.ensure_started();
                // Content block start — we already emitted output_item_added + content_part_added
                // in ensure_started(). If there's initial text, treat it as a delta.
                if let Some(text) = val
                    .get("content_block")
                    .and_then(|b| b.get("text"))
                    .and_then(|v| v.as_str())
                {
                    if !text.is_empty() {
                        self.accumulated_text.push_str(text);
                        self.pending
                            .push(output_text_delta_event(&self.item_id, text));
                    }
                }
            }
            "content_block_delta" => {
                self.ensure_started();
                if let Some(text) = val
                    .get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|v| v.as_str())
                {
                    if !text.is_empty() {
                        self.accumulated_text.push_str(text);
                        self.pending
                            .push(output_text_delta_event(&self.item_id, text));
                    }
                }
            }
            "content_block_stop" => {
                // Nothing to emit here; we'll emit done on message_stop
            }
            "message_delta" => {
                // Contains stop_reason and usage info
            }
            "message_stop" => {
                self.emit_end_events();
            }
            _ => {}
        }
    }
}
