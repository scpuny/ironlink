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

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_event() {
        let block = "event: message\ndata: {\"hello\": \"world\"}\n\n";
        let event = parse_sse_block(block);
        assert!(event.is_some());
        let e = event.unwrap();
        assert_eq!(e.event, "message");
        assert_eq!(e.data, "{\"hello\": \"world\"}");
    }

    #[test]
    fn test_parse_sse_event_without_event_field() {
        let block = "data: hello world\n\n";
        let event = parse_sse_block(block);
        assert!(event.is_some());
        let e = event.unwrap();
        assert_eq!(e.event, "message"); // default
        assert_eq!(e.data, "hello world");
    }

    #[test]
    fn test_parse_sse_custom_event() {
        let block = "event: response.completed\ndata: {\"status\": \"done\"}\n\n";
        let event = parse_sse_block(block);
        assert!(event.is_some());
        let e = event.unwrap();
        assert_eq!(e.event, "response.completed");
        assert_eq!(e.data, "{\"status\": \"done\"}");
    }

    #[test]
    fn test_parse_sse_empty_block() {
        assert!(parse_sse_block("").is_none());
        // Just newlines with no event/data fields returns None
        assert!(parse_sse_block("\n\n").is_none());
    }

    #[test]
    fn test_take_sse_block() {
        let mut buf = String::from("event: test\ndata: value\n\ntrailing");
        let block = take_sse_block(&mut buf);
        assert!(block.is_some());
        assert_eq!(block.unwrap(), "event: test\ndata: value");
        assert_eq!(buf, "trailing");
    }

    #[test]
    fn test_take_sse_block_empty() {
        let mut buf = String::from("no delimiter here");
        let block = take_sse_block(&mut buf);
        assert!(block.is_none());
        assert_eq!(buf, "no delimiter here");
    }

    #[test]
    fn test_take_sse_block_crlf() {
        let mut buf = String::from("event: test\r\ndata: value\r\n\r\n");
        let block = take_sse_block(&mut buf);
        assert!(block.is_some());
        assert!(block.unwrap().contains("event: test"));
    }

    #[test]
    fn test_append_utf8_safe() {
        let mut buf = String::new();
        let mut rem = Vec::new();
        append_utf8_safe(&mut buf, &mut rem, b"hello");
        assert_eq!(buf, "hello");
        assert!(rem.is_empty());
    }

    #[test]
    fn test_append_utf8_split_char() {
        let mut buf = String::new();
        let mut rem = Vec::new();
        // Send incomplete UTF-8 (first byte of a 2-byte char)
        append_utf8_safe(&mut buf, &mut rem, &[0xc3]);
        assert!(buf.is_empty());
        assert_eq!(rem.len(), 1);
        // Complete it
        append_utf8_safe(&mut buf, &mut rem, &[0xa9]);
        assert_eq!(buf, "\u{00e9}"); // é
        assert!(rem.is_empty());
    }

    #[test]
    fn test_is_done_block() {
        assert!(is_done_block("[DONE]"));
        assert!(!is_done_block("data: hello"));
        assert!(!is_done_block(""));
    }
}

