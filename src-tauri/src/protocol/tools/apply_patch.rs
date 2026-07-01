// ── apply_patch format parser ──

use serde_json::Value;

/// Parse a raw "*** Begin Patch/*** End Patch" text into structured operations.
pub fn parse_apply_patch_operations(input: &str) -> Vec<Value> {
    let mut ops = Vec::new();
    let mut current: Option<serde_json::Map<String, Value>> = None;
    let mut content_lines: Vec<String> = Vec::new();
    let mut hunks: Vec<Value> = Vec::new();
    let mut current_hunk: Option<serde_json::Map<String, Value>> = None;
    let mut hunk_lines: Vec<Value> = Vec::new();

    for line in input.lines() {
        if line == "*** Begin Patch" || line == "*** End Patch" { continue; }
        if let Some(path) = line.strip_prefix("*** Add File: ") {
            flush_hunk(&mut current_hunk, &mut hunk_lines, &mut hunks);
            flush_op(&mut current, &mut content_lines, &mut hunks, &mut ops);
            current = Some(serde_json::Map::from_iter([
                ("type".into(), Value::String("add_file".into())),
                ("path".into(), Value::String(path.to_string())),
            ]));
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Delete File: ") {
            flush_hunk(&mut current_hunk, &mut hunk_lines, &mut hunks);
            flush_op(&mut current, &mut content_lines, &mut hunks, &mut ops);
            current = Some(serde_json::Map::from_iter([
                ("type".into(), Value::String("delete_file".into())),
                ("path".into(), Value::String(path.to_string())),
            ]));
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Update File: ") {
            flush_hunk(&mut current_hunk, &mut hunk_lines, &mut hunks);
            flush_op(&mut current, &mut content_lines, &mut hunks, &mut ops);
            current = Some(serde_json::Map::from_iter([
                ("type".into(), Value::String("update_file".into())),
                ("path".into(), Value::String(path.to_string())),
            ]));
            continue;
        }
        if let Some(move_to) = line.strip_prefix("*** Move to: ") {
            if let Some(op) = current.as_mut() { op.insert("move_to".into(), Value::String(move_to.to_string())); }
            continue;
        }
        if line.starts_with("@@") {
            flush_hunk(&mut current_hunk, &mut hunk_lines, &mut hunks);
            current_hunk = Some(serde_json::Map::from_iter([
                ("context".into(), Value::String(line.strip_prefix("@@").unwrap_or("").trim().to_string())),
            ]));
            continue;
        }
        if let Some(op) = current.as_ref() {
            let op_type = op.get("type").and_then(Value::as_str).unwrap_or("");
            match op_type {
                "add_file" | "replace_file" => {
                    if let Some(text) = line.strip_prefix('+') { content_lines.push(text.to_string()); }
                }
                "update_file" => {
                    let (op_kind, text) = match line.chars().next() {
                        Some('+') => ("add", &line[1..]),
                        Some('-') => ("remove", &line[1..]),
                        Some(' ') => ("context", &line[1..]),
                        _ => ("context", line),
                    };
                    hunk_lines.push(serde_json::json!({"op": op_kind, "text": text}));
                }
                _ => {}
            }
        }
    }
    flush_hunk(&mut current_hunk, &mut hunk_lines, &mut hunks);
    flush_op(&mut current, &mut content_lines, &mut hunks, &mut ops);
    ops
}

/// Flush accumulated hunk lines into a hunk object.
fn flush_hunk(ch: &mut Option<serde_json::Map<String, Value>>, hl: &mut Vec<Value>, hunks: &mut Vec<Value>) {
    if let Some(mut h) = ch.take() {
        h.insert("lines".into(), Value::Array(std::mem::take(hl)));
        hunks.push(Value::Object(h));
    }
}

/// Flush the current operation with its accumulated content into the operations list.
fn flush_op(current: &mut Option<serde_json::Map<String, Value>>, cl: &mut Vec<String>, hunks: &mut Vec<Value>, ops: &mut Vec<Value>) {
    if let Some(mut op) = current.take() {
        match op.get("type").and_then(Value::as_str).unwrap_or("") {
            "add_file" | "replace_file" => { op.insert("content".into(), Value::String(cl.join("\n"))); }
            "update_file" => { op.insert("hunks".into(), Value::Array(std::mem::take(hunks))); }
            _ => {}
        }
        cl.clear();
        ops.push(Value::Object(op));
    }
}
