// ── Anthropic SSE → Responses API SSE ──

use serde_json::Value;
use crate::protocol::sse::parser::parse_sse_block;

/// Transform a chunk of Anthropic SSE text into Responses API SSE events.
pub fn transform_anthropic_chunk(chunk: &str) -> String {
    let mut output = String::new();
    let mut buffer = chunk.to_string();
    let _remainder: Vec<u8> = Vec::new();

    while let Some(block) = {
        let lf = buffer.find("\n\n").map(|i| (i, 2));
        let crlf = buffer.find("\r\n\r\n").map(|i| (i, 4));
        match (lf, crlf) {
            (Some(a), Some(b)) => if a.0 <= b.0 { Some((a.0, a.1)) } else { Some((b.0, b.1)) },
            (Some(v), None) | (None, Some(v)) => Some(v),
            _ => None,
        }
    } {
        let s = buffer[..block.0].to_string();
        buffer.drain(..block.0 + block.1);
        if s.trim().is_empty() { continue; }
        if let Some(event) = parse_sse_block(&s) {
            transform_anthropic_event(&event, &mut output);
        }
    }
    output
}

/// Transform a single Anthropic SSE event into Responses API SSE events.
fn transform_anthropic_event(event: &crate::protocol::core::types::SseEvent, output: &mut String) {
    let val: Value = match serde_json::from_str(&event.data) { Ok(v) => v, Err(_) => return };

    match event.event.as_str() {
        "message_start" => {
            let id = val.pointer("/message/id").and_then(Value::as_str).unwrap_or("unknown");
            let model = val.pointer("/message/model").and_then(Value::as_str).unwrap_or("claude");
            let rid = format!("resp_{}", &id[..8.min(id.len())]);
            push_anthropic_sse(output, "response.created", serde_json::json!({
                "type": "response.created",
                "response": {"id": rid, "object": "response", "model": model, "output": [], "usage": null}
            }));
            push_anthropic_sse(output, "response.in_progress", serde_json::json!({
                "type": "response.in_progress",
                "response": {"id": rid, "object": "response", "model": model, "output": [], "usage": null}
            }));
        }
        "content_block_start" => {
            let block_type = val.pointer("/content_block/type").and_then(Value::as_str).unwrap_or("text");
            let text = val.pointer("/content_block/text").and_then(Value::as_str).unwrap_or("");
            let item_id = format!("item_{}", &event.data.len().to_string()[..8]);
            match block_type {
                "thinking" | "reasoning" => {
                    push_anthropic_sse(output, "response.output_item.added", serde_json::json!({
                        "type": "response.output_item.added", "output_index": 0,
                        "item": {"id": item_id, "type": "reasoning", "status": "in_progress", "reasoning_content": "", "summary": []}
                    }));
                    push_anthropic_sse(output, "response.reasoning_summary_part.added", serde_json::json!({
                        "type": "response.reasoning_summary_part.added", "item_id": item_id,
                        "output_index": 0, "summary_index": 0,
                        "part": {"type": "summary_text", "text": ""}
                    }));
                    if !text.is_empty() {
                        push_anthropic_sse(output, "response.reasoning_summary_text.delta", serde_json::json!({
                            "type": "response.reasoning_summary_text.delta", "item_id": item_id,
                            "output_index": 0, "summary_index": 0, "delta": text
                        }));
                    }
                }
                _ => {
                    push_anthropic_sse(output, "response.output_item.added", serde_json::json!({
                        "type": "response.output_item.added", "output_index": 0,
                        "item": {"id": item_id, "type": "message", "status": "in_progress", "role": "assistant", "content": []}
                    }));
                    push_anthropic_sse(output, "response.content_part.added", serde_json::json!({
                        "type": "response.content_part.added", "item_id": item_id,
                        "output_index": 0, "content_index": 0,
                        "part": {"type": "output_text", "text": "", "annotations": []}
                    }));
                    if !text.is_empty() {
                        push_anthropic_sse(output, "response.output_text.delta", serde_json::json!({
                            "type": "response.output_text.delta", "item_id": item_id,
                            "output_index": 0, "content_index": 0, "delta": text
                        }));
                    }
                }
            }
        }
        "content_block_delta" => {
            let delta_type = val.pointer("/delta/type").and_then(Value::as_str).unwrap_or("text_delta");
            let text = val.pointer("/delta/text").and_then(Value::as_str).unwrap_or("");
            if text.is_empty() { return; }
            let item_id = format!("item_{}", &event.data.len().to_string()[..8]);
            match delta_type {
                "thinking_delta" | "reasoning_delta" | "signature_delta" => {
                    push_anthropic_sse(output, "response.reasoning_summary_text.delta", serde_json::json!({
                        "type": "response.reasoning_summary_text.delta", "item_id": item_id,
                        "output_index": 0, "summary_index": 0, "delta": text
                    }));
                }
                _ => {
                    push_anthropic_sse(output, "response.output_text.delta", serde_json::json!({
                        "type": "response.output_text.delta", "item_id": item_id,
                        "output_index": 0, "content_index": 0, "delta": text
                    }));
                }
            }
        }
        "content_block_stop" => {
            let item_id = format!("item_{}", &event.data.len().to_string()[..8]);
            push_anthropic_sse(output, "response.content_part.done", serde_json::json!({
                "type": "response.content_part.done", "item_id": item_id,
                "output_index": 0, "content_index": 0,
                "part": {"type": "output_text", "text": "", "annotations": []}
            }));
        }
        "message_delta" | "message_stop" | "ping" => {}
        _ => {}
    }
}

fn push_anthropic_sse(output: &mut String, event: &str, data: Value) {
    output.push_str("event: ");
    output.push_str(event);
    output.push_str("\ndata: ");
    output.push_str(&serde_json::to_string(&data).unwrap_or_default());
    output.push_str("\n\n");
}
