import type { ProxyStatus, BackendConfig, ModelEntry } from '../types';

let invokeFn: typeof import('@tauri-apps/api/core').invoke | null = null;

async function getInvoke() {
  if (!invokeFn) {
    const mod = await import('@tauri-apps/api/core');
    invokeFn = mod.invoke;
  }
  return invokeFn!;
}

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const invoke = await getInvoke();
  return invoke<T>(cmd, args);
}

// ── Status ──

export function fetchStatus(): Promise<ProxyStatus> {
  return tauriInvoke<ProxyStatus>('get_status');
}

// ── Backend Config ──

export function fetchBackend(): Promise<BackendConfig> {
  return tauriInvoke<BackendConfig>('get_backend');
}

export function updateBackend(config: BackendConfig): Promise<void> {
  return tauriInvoke<void>('update_backend', { backend: config });
}

// ── Models ──

export function fetchModels(): Promise<ModelEntry[]> {
  return tauriInvoke<ModelEntry[]>('get_models');
}

export function updateModels(models: ModelEntry[]): Promise<void> {
  return tauriInvoke<void>('update_models', { models });
}

// ── Proxy Toggle ──

export function toggleProxy(enable: boolean): Promise<boolean> {
  return tauriInvoke<boolean>('toggle_proxy', { enabled: enable });
}

// ── Config File ──

export function fetchConfigFile(): Promise<string> {
  return tauriInvoke<string>('get_config_file');
}

export function updateConfigFile(content: string): Promise<void> {
  return tauriInvoke<void>('write_config_file', { content });
}

// ── Auto-start ──

export function getAutoStart(): Promise<boolean> {
  return tauriInvoke<boolean>('get_auto_start');
}

export function setAutoStart(enabled: boolean): Promise<void> {
  return tauriInvoke<void>('set_auto_start', { enabled });
}

// ── Codex Config Files ──

export function fetchCodexConfigFile(): Promise<string> {
  return tauriInvoke<string>('get_codex_config_file');
}

export function readFileContent(path: string): Promise<string> {
  return tauriInvoke<string>('read_file_content', { path });
}

// ── Logs ──

export function fetchLogs(): Promise<string[]> {
  return tauriInvoke<string[]>('get_logs');
}

// ── Providers (upstream relay profiles) ──

export type RelayProfileData = {
  id: string;
  provider_id: string;
  name: string;
  base_url: string;
  api_key: string;
  protocol: string;
  model: string;
  test_model: string;
  model_list: string[];
  enabled: boolean;
  active: boolean;
};

export function fetchProfiles(): Promise<RelayProfileData[]> {
  return tauriInvoke<RelayProfileData[]>('get_profiles');
}

export function saveProfiles(profiles: RelayProfileData[]): Promise<void> {
  return tauriInvoke<void>('save_profiles', { profiles });
}

export function activateProfile(id: string): Promise<void> {
  return tauriInvoke<void>('activate_profile', { id });
}

// ── Applications (downstream apps like Codex, Claude) ──

export type AppInjection = {
  config_type: string;
  config_path: string;
  config_dir?: string;
  backup_enabled: boolean;
  fields?: string[];
};

export type AppData = {
  id: string;
  name: string;
  protocol: string;
  enabled: boolean;
  default_model: string;
  models: string[];
  config_injection: AppInjection | null;
  model_mappings: Record<string, { provider_id: string; upstream_model: string }>;
  model_replacement_enabled: boolean;
  model_display_names: Record<string, string>;
  config_snippet?: string;
};

export function fetchApps(): Promise<AppData[]> {
  return tauriInvoke<AppData[]>('get_apps');
}

export function saveApps(apps: AppData[]): Promise<void> {
  return tauriInvoke<void>('save_apps', { apps });
}

// ── Proxy Config ──

export function fetchProxyConfig(): Promise<import("../types").ProxyConfig> {
  return tauriInvoke<import("../types").ProxyConfig>('get_proxy_config');
}

export function setProxyConfig(config: import("../types").ProxyConfig): Promise<void> {
  return tauriInvoke<void>('set_proxy_config', { config });
}

export function testProviderConnection(baseUrl: string, apiKey: string, model: string): Promise<string> {
  return tauriInvoke<string>('test_provider_connection', { baseUrl, apiKey, model });
}

export function fetchUpstreamModels(url: string, apiKey: string): Promise<string[]> {
  return tauriInvoke<string[]>('fetch_upstream_models', { url, apiKey }).catch(e => {
    throw new Error(typeof e === 'string' ? e : e?.message || String(e));
  });
}


// ── Config Preview ──

export interface ConfigFileEntry {
  name: string;
  path: string;
  content: string;
}

export function getAppConfigFiles(appId: string): Promise<ConfigFileEntry[]> {
  return tauriInvoke<ConfigFileEntry[]>('get_app_config_files', { appId });
}

export function previewAppConfig(appId: string): Promise<string> {
  return tauriInvoke<string>('preview_app_config', { appId });
}

// ── Model Catalog ──

export function getModelCatalog(): Promise<string> {
  return tauriInvoke<string>('get_model_catalog');
}

export function regenerateModelCatalog(): Promise<void> {
  return tauriInvoke<void>('regenerate_model_catalog');
}

// ── System Settings ──

export function fetchSettings(): Promise<import("../types").AppSettings> {
  return tauriInvoke<import("../types").AppSettings>('get_settings');
}

export function saveSettings(s: import("../types").AppSettings): Promise<void> {
  return tauriInvoke<void>('save_settings', { s });
}

// ── Config Export / Import ──

export function exportConfig(): Promise<string> {
  return tauriInvoke<string>('export_config');
}

export function importConfig(json: string): Promise<string> {
  return tauriInvoke<string>('import_config', { json });
}
