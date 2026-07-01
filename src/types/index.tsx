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
