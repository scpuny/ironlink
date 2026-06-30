export type BackendType = 'openai-chat' | 'openai-responses' | 'anthropic';
export type AuthType = 'bearer' | 'x-api-key' | 'none';

export interface BackendConfig {
  type: BackendType;
  api_base: string;
  api_key: string;
  /** Display name for the provider */
  name?: string;
  /** Default model used for requests */
  model?: string;
  /** Test model for validation */
  test_model?: string;
  /** Auth type */
  auth_type?: AuthType;
  /** Custom headers (JSON) */
  custom_headers?: string;
  /** Advanced config content (TOML) */
  config_contents?: string;
  /** Custom user agent */
  user_agent?: string;
}

export interface ModelEntry {
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

export interface ApiResponse<T> {
  ok: boolean;
  data?: T;
  error?: string;
}
