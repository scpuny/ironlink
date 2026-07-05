//! Tauri commands for OCR status and image recognition.

use std::sync::Arc;

use crate::config::AppState;
use crate::ocr;

/// Status of OCR capabilities.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OcrStatus {
    pub enabled: bool,
    pub models_ready: bool,
    pub engine_loaded: bool,
    pub models_path: String,
}

#[tauri::command]
/// Check OCR status.
pub async fn check_ocr_status(state: tauri::State<'_, Arc<AppState>>) -> Result<OcrStatus, String> {
    let settings = state.settings.lock().await;
    Ok(OcrStatus {
        enabled: settings.ocr_enabled,
        models_ready: ocr::models_ready(),
        engine_loaded: ocr::is_loaded(),
        models_path: ocr::model_dir().to_string_lossy().to_string(),
    })
}

#[tauri::command]
/// Run OCR on an image file. Returns recognized text regions.
pub async fn run_ocr(image_path: String) -> Result<Vec<ocr::OcrTextResult>, String> {
    tokio::task::spawn_blocking(move || ocr::run_ocr(&image_path))
        .await
        .map_err(|e| format!("OCR task failed: {e}"))?
}
