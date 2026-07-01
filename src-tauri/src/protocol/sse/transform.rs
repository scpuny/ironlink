// ── SseTransformStream ──

use std::pin::Pin;
use std::task::{Context, Poll};
use std::io;

use bytes::Bytes;
use futures::{Stream, StreamExt};

use crate::protocol::core::traits::SseTransform;
use crate::protocol::sse::chat_sse::ChatSseConverter;
use crate::protocol::sse::anthropic_sse;
use crate::protocol::sse::parser::{take_sse_block, append_utf8_safe};

/// Wraps an upstream byte `Stream` and transforms its SSE events
/// into Responses API SSE events.
pub struct SseTransformStream<S> {
    inner: S,
    mode: Mode,
    buffer: Vec<u8>,
    finished: bool,
    converted: bool,
}

enum Mode {
    Chat(ChatSseConverter),
    Anthropic {
        buffer: String,
        remainder: Vec<u8>,
        response_started: bool,
        response_id: String,
        finalized: bool,
    },
}

impl<S> SseTransformStream<S> {
    /// Create a new SSE transform stream for the given upstream type.
    pub fn new(inner: S, is_chat: bool) -> Self {
        Self {
            inner,
            mode: if is_chat {
                Mode::Chat(ChatSseConverter::default())
            } else {
                Mode::Anthropic {
                    buffer: String::new(),
                    remainder: Vec::new(),
                    response_started: false,
                    response_id: String::new(),
                    finalized: false,
                }
            },
            buffer: Vec::new(),
            finished: false,
            converted: false,
        }
    }

    /// Create a Chat SSE converter with the original request for field passthrough.
    pub fn with_request(inner: S, original_request: serde_json::Value) -> Self {
        Self {
            inner,
            mode: Mode::Chat(ChatSseConverter::default()),
            buffer: Vec::new(),
            finished: false,
            converted: false,
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
            Mode::Anthropic { buffer, remainder, finalized, .. } => {
                if *finalized {
                    this.finished = true;
                    return Poll::Ready(None);
                }
                match inner.poll_next_unpin(cx) {
                    Poll::Ready(Some(Ok(chunk))) => {
                        append_utf8_safe(buffer, remainder, &chunk);
                        let mut output = String::new();
                        while let Some(block) = take_sse_block(buffer) {
                            if block.trim().is_empty() { continue; }
                            output.push_str(&anthropic_sse::transform_anthropic_chunk(&format!("{}\n\n", block)));
                        }
                        if !output.is_empty() { Poll::Ready(Some(Ok(Bytes::from(output)))) }
                        else { cx.waker().wake_by_ref(); Poll::Pending }
                    }
                    Poll::Ready(Some(Err(e))) => { this.finished = true; Poll::Ready(Some(Err(e))) }
                    Poll::Ready(None) => { *finalized = true; cx.waker().wake_by_ref(); Poll::Pending }
                    Poll::Pending => Poll::Pending,
                }
            }
        }
    }
}
