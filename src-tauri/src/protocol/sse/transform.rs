// ── SSE stream transformer: Chat ↔ Anthropic ↔ Responses ──

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::protocol::core::traits::SseTransform;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::Value;

use crate::protocol::sse::anthropic_sse::AnthropicSseConverter;
use crate::protocol::sse::chat_sse::ChatSseConverter;
use crate::protocol::sse::parser::{append_utf8_safe, take_sse_block, is_done_block};

/// Wraps an upstream SSE byte stream and transforms events
/// from the upstream wire protocol to the Responses API SSE format.
pub struct SseTransformStream<S> {
    inner: S,
    mode: Mode,
    buffer: Vec<u8>,
    finished: bool,
}

enum Mode {
    Chat(ChatSseConverter),
    Anthropic(AnthropicSseConverter),
}

impl<S> SseTransformStream<S> {
    /// Create a new SSE transform stream for the given upstream type.
    ///
    /// `original_request` — the original Responses API request body (with `tools`),
    /// used to correctly emit custom tool calls (web_search, etc.) vs function calls.
    pub fn new(inner: S, is_chat: bool, original_request: Option<&Value>) -> Self {
        Self {
            inner,
            mode: if is_chat {
                Mode::Chat(
                    original_request
                        .map(|v| ChatSseConverter::with_request(v))
                        .unwrap_or_default()
                )
            } else {
                Mode::Anthropic(AnthropicSseConverter::default())
            },
            buffer: Vec::new(),
            finished: false,
        }
    }
}

impl<S: Stream<Item = io::Result<Bytes>> + Unpin> Stream for SseTransformStream<S> {
    type Item = io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if !self.buffer.is_empty() {
            return Poll::Ready(Some(Ok(Bytes::from(std::mem::take(&mut self.buffer)))));
        }
        if self.finished { return Poll::Ready(None); }

        let this = &mut *self;
        let inner_ptr = &mut this.inner as *mut S;
        let mode_ptr = &mut this.mode as *mut Mode;

        // ponytail: unsafe because Pin<&mut T> doesn't let us borrow two fields
        let inner = unsafe { &mut *inner_ptr };
        let mode = unsafe { &mut *mode_ptr };

        match mode {
            Mode::Chat(converter) => {
                match inner.poll_next_unpin(cx) {
                    Poll::Ready(Some(Ok(chunk))) => {
                        let out = converter.push_bytes(&chunk);
                        if !out.is_empty() { Poll::Ready(Some(Ok(Bytes::from(out)))) }
                        else { cx.waker().wake_by_ref(); Poll::Pending }
                    }
                    Poll::Ready(Some(Err(e))) => { this.finished = true; Poll::Ready(Some(Err(e))) }
                    Poll::Ready(None) => {
                        let out = converter.finish();
                        this.finished = true;
                        if !out.is_empty() { Poll::Ready(Some(Ok(Bytes::from(out)))) } else { Poll::Ready(None) }
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
            Mode::Anthropic(converter) => {
                match inner.poll_next_unpin(cx) {
                    Poll::Ready(Some(Ok(chunk))) => {
                        let mut buf = String::new();
                        let mut rem = Vec::new();
                        append_utf8_safe(&mut buf, &mut rem, &chunk);
                        let mut output = String::new();
                        while let Some(block) = take_sse_block(&mut buf) {
                            if block.trim().is_empty() { continue; }
                            if is_done_block(&block) {
                                output.push_str(&converter.finish());
                                this.finished = true;
                                return if output.is_empty() { Poll::Ready(None) } else { Poll::Ready(Some(Ok(Bytes::from(output)))) };
                            }
                            let block_str = block.to_string();
                            let converted = converter.push_block(&format!("{}\n\n", block_str));
                            if converted.is_empty() && !block_str.contains("message_start")
                                && !block_str.contains("content_block")
                                && !block_str.contains("ping")
                            {
                                let event_name = block_str.lines()
                                    .find(|l| l.starts_with("event:"))
                                    .map(|l| l["event:".len()..].trim())
                                    .unwrap_or("");
                                if event_name == "error" || event_name.contains("error") {
                                    let err_msg = block_str.lines()
                                        .find(|l| l.starts_with("data:"))
                                        .map(|l| l["data:".len()..].trim())
                                        .unwrap_or("Unknown upstream error");
                                    output.push_str(&converter.fail(err_msg.to_string(), Some("upstream_error".into())));
                                    this.finished = true;
                                    break;
                                }
                            }
                            output.push_str(&converted);
                        }
                        if !output.is_empty() { Poll::Ready(Some(Ok(Bytes::from(output)))) }
                        else { cx.waker().wake_by_ref(); Poll::Pending }
                    }
                    Poll::Ready(Some(Err(e))) => { this.finished = true; Poll::Ready(Some(Err(e))) }
                    Poll::Ready(None) => {
                        let out = converter.finish();
                        this.finished = true;
                        if !out.is_empty() { Poll::Ready(Some(Ok(Bytes::from(out)))) } else { Poll::Ready(None) }
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
        }
    }
}
