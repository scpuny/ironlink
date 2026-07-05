//! OCR engine — dual-model image text recognition (Chinese + Latin).
//!
//! Models are bundled with the app at `resources/models/ppocrv5/`
//! and resolved at runtime via [`model_dir()`].
//!
//! Two recognition models run in parallel on each detected text region;
//! the result with higher confidence wins. Detection runs once via the
//! Chinese engine (same detection config for both).

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Model files expected in the model directory.
const DET_MODEL: &str = "pp-ocrv5_mobile_det.onnx";
const REC_MODEL: &str = "pp-ocrv5_mobile_rec.onnx";
const DICT_FILE: &str = "ppocrv5_dict.txt";
const LATIN_REC_MODEL: &str = "latin_pp-ocrv5_mobile_rec.onnx";
const LATIN_DICT_FILE: &str = "ppocrv5_latin_dict.txt";
/// Text line orientation classifier (ch_ppocr_mobile_v2.0_cls_infer).
const CLS_MODEL: &str = "ch_ppocr_mobile_v2.0_cls_infer.onnx";
const DOC_ORI_MODEL: &str = "pp-lcnet_x1_0_doc_ori.onnx";

static MODEL_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Holds both the Chinese and Latin OCR engines.
struct DualEngine {
    ch: pure_onnx_ocr_sync::OcrEngine,
    latin: pure_onnx_ocr_sync::OcrEngine,
}

/// Wrapped in Box so we can take() it out.
static ENGINE: Mutex<Option<Box<DualEngine>>> = Mutex::new(None);

/// Set the model directory (called once during app setup).
pub fn init(dir: PathBuf) {
    let _ = MODEL_DIR.set(dir);
}

/// Get the model directory.
pub fn model_dir() -> PathBuf {
    MODEL_DIR.get().cloned().unwrap_or_else(|| {
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/models/ppocrv5");
        tracing::warn!("MODEL_DIR not initialized, using dev fallback: {:?}", dev);
        dev
    })
}

/// Check whether all model files exist on disk.
pub fn models_ready() -> bool {
    let dir = model_dir();
    dir.join(DET_MODEL).exists()
        && dir.join(REC_MODEL).exists()        // Chinese rec
        && dir.join(DICT_FILE).exists()        // Chinese dict
        && dir.join(LATIN_REC_MODEL).exists()  // Latin rec
        && dir.join(LATIN_DICT_FILE).exists()  // Latin dict
        && dir.join(CLS_MODEL).exists()        // line orientation
        && dir.join(DOC_ORI_MODEL).exists()    // doc orientation
}

/// Whether the engine is currently loaded in memory.
pub fn is_loaded() -> bool {
    ENGINE.lock().unwrap().is_some()
}

/// Result of OCR on a single detected text region.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OcrTextResult {
    pub text: String,
    pub confidence: f32,
    pub polygon: Vec<[f64; 2]>,
}

/// Build both engines in the background (warmup).
pub fn warmup() -> bool {
    if !models_ready() {
        return false;
    }
    if ENGINE.lock().unwrap().is_some() {
        return true;
    }

    let dir = model_dir();
    let build_one = |rec: PathBuf, dict: PathBuf| {
        pure_onnx_ocr_sync::OcrEngineBuilder::new()
            .det_model_path(dir.join(DET_MODEL))
            .rec_model_path(rec)
            .text_line_ori_model_path(dir.join(CLS_MODEL))
            .doc_ori_model_path(dir.join(DOC_ORI_MODEL))
            .dictionary_path(dict)
            .det_limit_side_len(960)
            .det_unclip_ratio(1.6)
            .rec_batch_size(8)
            .build()
    };

    let ch = match build_one(dir.join(REC_MODEL), dir.join(DICT_FILE)) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Chinese OCR engine warmup failed: {e}");
            return false;
        }
    };

    let latin = match build_one(dir.join(LATIN_REC_MODEL), dir.join(LATIN_DICT_FILE)) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Latin OCR engine warmup failed: {e}");
            return false;
        }
    };

    *ENGINE.lock().unwrap() = Some(Box::new(DualEngine { ch, latin }));
    true
}

/// Drop the engine and free its memory.
pub fn shutdown() {
    *ENGINE.lock().unwrap() = None;
    tracing::info!("OCR engine shut down, memory freed");
}

/// Run OCR on an image file using both engines, picking best result per region.
pub fn run_ocr(image_path: &str) -> Result<Vec<OcrTextResult>, String> {
    // Fast path: engines already loaded
    if let Some(engine) = ENGINE.lock().unwrap().as_ref() {
        return run_dual(engine, image_path);
    }

    // Cold start: build engines
    if !models_ready() {
        return Err(format!("OCR models not found at {:?}", model_dir()));
    }
    tracing::info!("OCR cold start: building engines (may take ~60s)...");

    let dir = model_dir();
    let build_one = |rec: PathBuf, dict: PathBuf| {
        pure_onnx_ocr_sync::OcrEngineBuilder::new()
            .det_model_path(dir.join(DET_MODEL))
            .rec_model_path(rec)
            .text_line_ori_model_path(dir.join(CLS_MODEL))
            .doc_ori_model_path(dir.join(DOC_ORI_MODEL))
            .dictionary_path(dict)
            .det_limit_side_len(960)
            .det_unclip_ratio(1.6)
            .rec_batch_size(8)
            .build()
            .map_err(|e| format!("Failed to build OCR engine: {e}"))
    };

    let ch = build_one(dir.join(REC_MODEL), dir.join(DICT_FILE))?;
    let latin = build_one(dir.join(LATIN_REC_MODEL), dir.join(LATIN_DICT_FILE))?;
    let engine = DualEngine { ch, latin };

    let result = run_dual(&engine, image_path);
    *ENGINE.lock().unwrap() = Some(Box::new(engine));
    result
}

/// Run both engines and pick the best per detected region.
fn run_dual(engine: &DualEngine, image_path: &str) -> Result<Vec<OcrTextResult>, String> {
    let ch_results = engine
        .ch
        .run_from_path(image_path)
        .map_err(|e| format!("Chinese OCR failed: {e}"))?;

    let latin_results = engine
        .latin
        .run_from_path(image_path)
        .map_err(|e| format!("Latin OCR failed: {e}"))?;

    // Both use the same detection config → same number of results in the same order.
    // For each region pick the higher-confidence recognition output.
    let paired = ch_results.into_iter().zip(latin_results.into_iter());
    let output: Vec<OcrTextResult> = paired
        .filter(|(ch, latin)| {
            // Keep everything above threshold; dual model gives us richer coverage
            ch.confidence >= 0.55 || latin.confidence >= 0.55
        })
        .map(|(ch, latin)| {
            let best = if latin.confidence > ch.confidence {
                latin
            } else {
                ch
            };
            let polygon: Vec<[f64; 2]> = best
                .bounding_box
                .exterior()
                .points()
                .map(|p| [p.x(), p.y()])
                .collect();
            OcrTextResult {
                text: best.text,
                confidence: best.confidence,
                polygon,
            }
        })
        .collect();

    Ok(output)
}

/// 将OCR结果按页面阅读顺序拼接完整文本
pub fn concat_text(results: &[OcrTextResult]) -> String {
    let mut list = results.to_vec();
    list.sort_by(|a, b| {
        let min_y_a = a.polygon.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
        let min_y_b = b.polygon.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
        let y_cmp = min_y_a.partial_cmp(&min_y_b).unwrap();
        if y_cmp != std::cmp::Ordering::Equal {
            return y_cmp;
        }
        let min_x_a = a.polygon.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        let min_x_b = b.polygon.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        min_x_a.partial_cmp(&min_x_b).unwrap()
    });

    let mut buf = String::new();
    for item in list {
        buf.push_str(&item.text);
        buf.push('\n');
    }
    buf
}
