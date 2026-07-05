export type BackendType = 'openai-chat' | 'openai-responses' | 'anthropic';
export type AuthType = 'bearer' | 'x-api-key' | 'none';

export interface BackendConfig {
  type: BackendType;
  api_base: string;
  api_key: string;
  name?: string;
  model?: string;
  test_model?: string;
  auth_type?: AuthType;
  custom_headers?: string;
  config_contents?: string;
  user_agent?: string;
}

export interface ModelEntry {
  context_window?: number;
  max_context_window?: number;
  input_modalities?: string[];
  id: string;
  object: string;
  created: number;
  owned_by: string;
}

export interface ProxyStatus {
  enabled: boolean;
  backend: string;
  api_base: string;
  port: number;
}

export interface ProxyConfig {
  default_model: string;
  reasoning_effort: string;
}

export interface ApiResponse<T> {
  ok: boolean;
  data?: T;
  error?: string;
}

// ── System Settings (mirrors Rust config::settings::AppSettings) ──

export interface AppSettings {
  proxy_port: number;
  auto_start: boolean;
  minimize_to_tray_on_close: boolean;
  start_minimized: boolean;
  config_injection_enabled: boolean;
  language: string;
  ocr_enabled: boolean;
  ocr_models_downloaded: boolean;
  skill_settings_enabled?: boolean;
  unified_session?: boolean;
}

// ── OCR / Image Recognition ──

export interface OcrTextResult {
  text: string;
  confidence: number;
  polygon: [number, number][];
}

export interface OcrStatus {
  enabled: boolean;
  models_downloaded: boolean;
  models_path: string;
}

// ── Model Capabilities ──

export type Modality = 'text' | 'image' | 'vision';
