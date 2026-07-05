//! OCR interceptor for the proxy pipeline.
//!
//! Injected into `handle_proxy` after model routing, before protocol conversion.
//! Scans the request body for images and decides whether to:
//!   - Passthrough (model supports vision natively)
//!   - Run OCR + inject text (model is text-only, OCR enabled)
//!   - Skip (model is text-only, OCR disabled)

use std::collections::HashMap;

use crate::models::profile::supports_vision;
use crate::ocr::{self, OcrTextResult};
use serde_json::Value;

/// Decision result from the interceptor.
#[derive(Debug)]
pub enum OcrDecision {
    /// Pass through unchanged (model has vision capability or no images found).
    Passthrough,
    /// OCR was run; use the modified body.
    OcrApplied(Value),
    /// Skip — model is text-only, OCR disabled.
    Skipped,
}

/// Main interceptor entry point.
///
/// Called AFTER the "should we OCR?" decision is made in the proxy handler.
/// This function only checks vision capability and runs OCR if needed.
///
/// # Arguments
/// * `body` — The JSON request body to intercept.
/// * `upstream_model` — The model name that will receive the request.
/// * `model_capabilities` — Capabilities map from the selected provider.
/// * `models_ready` — Whether OCR models are available.
///
/// # Returns
/// `OcrDecision` indicating what action was taken.
pub fn ocr_intercept(
    body: &Value,
    upstream_model: &str,
    model_capabilities: &HashMap<String, Vec<String>>,
    models_ready: bool,
) -> OcrDecision {
    // 1. Check if the request body contains any input_image items
    let latest_image = find_latest_input_image(body);
    let latest_image = match latest_image {
        Some(img) => img,
        None => return OcrDecision::Passthrough, // no images → nothing to do
    };

    // 2. Check if the upstream model supports vision natively
    let has_vision = supports_vision(model_capabilities, upstream_model);
    tracing::info!(
        "OCR interceptor: model={}, has_vision={}, models_downloaded={}",
        upstream_model,
        has_vision,
        models_ready,
    );

    if has_vision {
        tracing::info!("OCR interceptor: model supports vision, passthrough");
        return OcrDecision::Passthrough;
    }

    // 3. Check models are downloaded
    if !models_ready && !ocr::models_ready() {
        tracing::warn!("OCR interceptor: models not downloaded, skipping");
        return OcrDecision::Skipped;
    }

    // 4. Extract image to a temp file and run OCR
    tracing::info!("OCR interceptor: running OCR on latest image");
    let image_data = match extract_image_data(&latest_image) {
        Some(data) => data,
        None => {
            tracing::warn!("OCR interceptor: failed to extract image data");
            return OcrDecision::Skipped;
        }
    };

    // Write image to temp file
    let temp_dir = std::env::temp_dir().join("ironlink-ocr");
    let _ = std::fs::create_dir_all(&temp_dir);
    let temp_path = temp_dir.join(format!(
        "ocr_{}.png",
        std::time::UNIX_EPOCH
            .elapsed()
            .unwrap_or_default()
            .as_nanos()
    ));

    if let Err(e) = std::fs::write(&temp_path, &image_data) {
        tracing::warn!("OCR interceptor: failed to write temp image: {e}");
        return OcrDecision::Skipped;
    }

    // Run OCR (blocking — called from spawn_blocking won't block the runtime)
    let ocr_results = match ocr::run_ocr(&temp_path.to_string_lossy()) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("OCR interceptor: OCR failed: {e}");
            let _ = std::fs::remove_file(&temp_path);
            return OcrDecision::Skipped;
        }
    };


    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);

    // 5. Inject OCR text into the request body
    let modified = inject_ocr_text(body, &latest_image, &ocr_results);

    tracing::info!(
        "OCR interceptor: OCR completed, {} text items found",
        ocr_results.len()
    );
    OcrDecision::OcrApplied(modified)
}

/// Information about an image found in the request body.
struct ImageInfo {
    /// Path indices: parent_path[0] = input item index, parent_path[1] = content part index
    parent_path: Vec<usize>,
    /// The raw image bytes (decoded from base64 data URL).
    data: Vec<u8>,
}

/// Find the latest (last) `input_image` in the request's `input` array.
/// Only looks at the last user message.
fn find_latest_input_image(body: &Value) -> Option<ImageInfo> {
    let input = body.get("input")?.as_array()?;

    // Iterate input items from the end
    for (item_idx, item) in input.iter().enumerate().rev() {
        let role = item.get("role")?.as_str()?;
        if role != "user" {
            continue;
        }
        let content = item.get("content")?.as_array()?;

        // Within this message's content, find the last image
        for part_idx in (0..content.len()).rev() {
            let part = &content[part_idx];
            if part.get("type")?.as_str()? != "input_image" {
                continue;
            }
            let image_url = part
                .get("image_url")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let data = decode_image_data(image_url, part)?;
            return Some(ImageInfo {
                parent_path: vec![item_idx, part_idx],
                data,
            });
        }
    }
    None
}

/// Decode image data from a data URL or base64 string.
fn decode_image_data(image_url: &str, part: &Value) -> Option<Vec<u8>> {
    if !image_url.is_empty() {
        decode_data_url(image_url)
    } else {
        // Try file_data or data field (base64)
        let b64 = part
            .get("file_data")
            .and_then(|v| v.as_str())
            .or_else(|| part.get("data").and_then(|v| v.as_str()))?;
        base64_decode(b64)
    }
}

/// Decode a data URL like `data:image/png;base64,iVBOR...`
fn decode_data_url(data_url: &str) -> Option<Vec<u8>> {
    let base64_part = data_url.split(',').nth(1)?;
    base64_decode(base64_part)
}

/// Base64 decode a string (handles URL-safe variants and whitespace).
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    // Try standard base64 first, then URL-safe
    base64::engine::general_purpose::STANDARD
        .decode(&cleaned)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(&cleaned))
        .ok()
}

/// Extract image data bytes from the ImageInfo.
fn extract_image_data(info: &ImageInfo) -> Option<Vec<u8>> {
    Some(info.data.clone())
}

/// Inject OCR text results into the request body, replacing the image
/// with text content.
fn inject_ocr_text(body: &Value, image: &ImageInfo, results: &[OcrTextResult]) -> Value {
    let mut body = body.clone();

    //  Build OCR text content
    let ocr_text = ocr::concat_text(&results);

    let text_content = if ocr_text.is_empty() {
        "[OCR: No text detected in image]".to_string()
    } else {
        format!("[OCR extracted text]:\n{}", ocr_text)
    };

    // Navigate to the content array and replace the image part with text
    if let Some(input) = body.get_mut("input").and_then(|v| v.as_array_mut()) {
        let item_idx = image.parent_path[0];
        let part_idx = image.parent_path[1];
        if let Some(content) = input
            .get_mut(item_idx)
            .and_then(|v| v.get_mut("content"))
            .and_then(|v| v.as_array_mut())
        {
            if part_idx < content.len() {
                content[part_idx] = serde_json::json!({
                    "type": "input_text",
                    "text": text_content,
                });
            }
        }
    }

    body
}
