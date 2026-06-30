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

// ── Chat SSE → Responses SSE ──

fn transform_chat_sse(event: &SseEvent) -> Option<Bytes> {
    if event.data == "[DONE]" {
        return None;
    }
    let val: serde_json::Value = serde_json::from_str(&event.data).ok()?;
    let delta = val.get("choices")?.as_array()?.first()?.get("delta")?;
    let text = delta.get("content")?.as_str()?;
    if text.is_empty() {
        return None;
    }
    let sse_data = serde_json::json!({
        "type": "response.output_text.delta",
        "delta": text,
        "item_id": gen_item_id(),
        "output_index": 0,
        "content_index": 0,
    });
    Some(Bytes::from(format!("data: {}\n\n", sse_data)))
}

// ── Anthropic SSE → Responses SSE ──

fn transform_anthropic_sse(event: &SseEvent) -> Option<Bytes> {
    let val: serde_json::Value = serde_json::from_str(&event.data).ok()?;

    if event.event == "content_block_delta" {
        let text = val.get("delta")?.get("text")?.as_str()?;
        if text.is_empty() {
            return None;
        }
        let sse_data = serde_json::json!({
            "type": "response.output_text.delta",
            "delta": text,
            "item_id": gen_item_id(),
            "output_index": 0,
            "content_index": 0,
        });
        Some(Bytes::from(format!("data: {}\n\n", sse_data)))
    } else {
        None
    }
}

fn gen_item_id() -> String {
    use uuid::Uuid;
    format!("item_{}", &Uuid::new_v4().to_string().replace('-', "")[..16])
}

// ── Stream transformer ──

pub struct SseTransformStream<S> {
    inner: S,
    parser: SseParser,
    is_chat: bool,
    pending: Vec<Bytes>,
}

impl<S> SseTransformStream<S>
where
    S: Stream<Item = io::Result<Bytes>> + Unpin,
{
    pub fn new(inner: S, is_chat: bool) -> Self {
        Self { inner, parser: SseParser::new(), is_chat, pending: Vec::new() }
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

        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    let events = self.parser.push(&chunk);
                    for event in &events {
                        let transformed = if self.is_chat {
                            transform_chat_sse(event)
                        } else {
                            transform_anthropic_sse(event)
                        };
                        if let Some(bs) = transformed {
                            self.pending.push(bs);
                        }
                    }
                    if !self.pending.is_empty() {
                        return Poll::Ready(Some(Ok(self.pending.remove(0))));
                    }
                }
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
