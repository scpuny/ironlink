// ── Generic SSE parser ──

use crate::protocol::core::types::SseEvent;

fn strip_sse_field<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(field)?.strip_prefix(':')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// Parse one SSE block into an SseEvent.
pub fn parse_sse_block(block: &str) -> Option<SseEvent> {
    let mut event = String::from("message");
    let mut data_parts = Vec::new();
    for line in block.lines() {
        if let Some(ev) = strip_sse_field(line, "event") {
            event = ev.trim().to_string();
        }
        if let Some(d) = strip_sse_field(line, "data") {
            data_parts.push(d.to_string());
        }
    }
    if data_parts.is_empty() { return None; }
    let data = data_parts.join("\n");
    Some(SseEvent { event, data })
}

/// Take one complete SSE block from a buffer (delimited by \n\n or \r\n\r\n).
pub fn take_sse_block(buffer: &mut String) -> Option<String> {
    let lf = buffer.find("\n\n").map(|i| (i, 2));
    let crlf = buffer.find("\r\n\r\n").map(|i| (i, 4));
    let (idx, delim) = match (lf, crlf) {
        (Some(a), Some(b)) => if a.0 <= b.0 { a } else { b },
        (Some(v), None) | (None, Some(v)) => v,
        _ => return None,
    };
    let block = buffer[..idx].to_string();
    buffer.drain(..idx + delim);
    Some(block)
}

/// Safely append bytes to a String, handling UTF-8 boundaries.
pub fn append_utf8_safe(buffer: &mut String, remainder: &mut Vec<u8>, bytes: &[u8]) {
    if bytes.is_empty() { return; }
    let mut combined = Vec::new();
    if !remainder.is_empty() {
        combined.extend_from_slice(remainder);
        remainder.clear();
    }
    combined.extend_from_slice(bytes);
    match std::str::from_utf8(&combined) {
        Ok(text) => buffer.push_str(text),
        Err(error) => {
            let valid_end = error.valid_up_to();
            if valid_end > 0 {
                buffer.push_str(&String::from_utf8_lossy(&combined[..valid_end]));
            }
            remainder.extend_from_slice(&combined[valid_end..]);
        }
    }
}

/// Check if a block signals end of stream.
pub fn is_done_block(block: &str) -> bool {
    block.trim() == "[DONE]"
}
