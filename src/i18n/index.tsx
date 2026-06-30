import React, { createContext, useContext, useState, useCallback } from 'react';

export type Lang = 'zh' | 'en';

const messages: Record<Lang, Record<string, string>> = {
  zh: {
    'proxy_active': '代理已启用',
    'proxy_off': '代理已关闭',
    'overview': '概览',
    'providers': '供应商',
    'models': '模型管理',
    'config': '配置文件',
    'auth': '认证',
    'logs': '日志',
    'settings': '设置',
    'light_mode': '浅色模式',
    'dark_mode': '深色模式',
    'proxy_active_desc': '所有 Codex API 流量将被拦截并转发',
    'proxy_disabled': '代理已关闭',
    'proxy_disabled_desc': 'Codex API 流量直连原始服务器',
    'enable': '启用',
    'disable': '禁用',
    'port': '端口',
    'backend_type': '后端类型',
    'api_base': 'API 地址',
    'quick_config': '配置 Codex 使用此代理，编辑',
    'backend_config': '后端配置',
    'provider_type': '供应商类型',
    'name': '供应商名称',
    'chat_compatible': 'OpenAI Chat 兼容',
    'responses': 'OpenAI Responses',
    'claude': 'Anthropic (Claude)',
    'api_base_url': 'API 地址',
    'api_key': 'API 密钥',
    'default_model': '默认模型',
    'auth_type': '认证方式',
    'test_model': '测试模型',
    'test_model_placeholder': '留空则使用默认模型',
    'custom_headers': '自定义请求头',
    'config_contents': '高级配置',
    'config_contents_placeholder': '自定义 TOML 配置内容',
    'save': '保存',
    'saving': '保存中...',
    'saved': '已保存',
    'model_list': '模型列表',
    'add_model': '添加模型',
    'model_id': '模型 ID',
    'owned_by': '所属',
    'delete': '删除',
    'no_models': '暂未配置模型',
    'config_file': '配置文件',
    'proxy_logs': '代理日志',
    'auto_refresh': '自动刷新',
    'no_logs': '暂无日志',
    'loading': '加载中...',
    'failed_to_load': '加载失败',
    'system_settings': '系统设置',
    'theme': '主题',
    'language': '语言',
    'about': '关于',
    'app_name': '应用名称',
    'frontend': '前端框架',
    'create_from_preset': '从预设创建',
    'search_provider': '搜索供应商…',
    'no_match_provider': '没有匹配的供应商',
  },
  en: {
    'proxy_active': 'Proxy Active',
    'proxy_off': 'Proxy Off',
    'overview': 'Overview',
    'providers': 'Providers',
    'models': 'Models',
    'config': 'Config',
    'auth': 'Auth',
    'logs': 'Logs',
    'settings': 'Settings',
    'light_mode': 'Light Mode',
    'dark_mode': 'Dark Mode',
    'proxy_active_desc': 'All Codex API traffic will be intercepted and forwarded',
    'proxy_disabled': 'Proxy Disabled',
    'proxy_disabled_desc': 'Codex API traffic goes directly to the original server',
    'enable': 'Enable',
    'disable': 'Disable',
    'port': 'Port',
    'backend_type': 'Backend Type',
    'api_base': 'API Base',
    'quick_config': 'Configure Codex to use this proxy by editing',
    'backend_config': 'Backend Configuration',
    'provider_type': 'Provider Type',
    'name': 'Provider Name',
    'chat_compatible': 'OpenAI Chat Compatible',
    'responses': 'OpenAI Responses',
    'claude': 'Anthropic (Claude)',
    'api_base_url': 'API Base URL',
    'api_key': 'API Key',
    'default_model': 'Default Model',
    'auth_type': 'Auth Type',
    'test_model': 'Test Model',
    'test_model_placeholder': 'Leave empty to use default model',
    'custom_headers': 'Custom Headers',
    'config_contents': 'Advanced Config',
    'config_contents_placeholder': 'Custom TOML configuration content',
    'save': 'Save',
    'saving': 'Saving...',
    'saved': 'Saved',
    'model_list': 'Model List',
    'add_model': 'Add Model',
    'model_id': 'Model ID',
    'owned_by': 'Owned By',
    'delete': 'Delete',
    'no_models': 'No models configured',
    'config_file': 'Config File',
    'proxy_logs': 'Proxy Logs',
    'auto_refresh': 'Auto refresh',
    'no_logs': 'No logs yet.',
    'loading': 'Loading...',
    'failed_to_load': 'Failed to load.',
    'system_settings': 'System Settings',
    'theme': 'Theme',
    'language': 'Language',
    'about': 'About',
    'app_name': 'App Name',
    'frontend': 'Frontend',
    'create_from_preset': 'Create from Preset',
    'search_provider': 'Search provider…',
    'no_match_provider': 'No matching provider',
  },
};

interface I18nCtx {
  lang: Lang;
  setLang: (l: Lang) => void;
  t: (key: string) => string;
}

const I18nContext = createContext<I18nCtx>({
  lang: 'zh',
  setLang: () => {},
  t: (k: string) => k,
});

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [lang, setLang] = useState<Lang>(() => {
    return (localStorage.getItem('codex-proxy-lang') as Lang) || 'zh';
  });

  const handleSetLang = useCallback((l: Lang) => {
    setLang(l);
    localStorage.setItem('codex-proxy-lang', l);
  }, []);

  const t = useCallback((key: string) => {
    return messages[lang][key] || key;
  }, [lang]);

  return (
    <I18nContext.Provider value={{ lang, setLang: handleSetLang, t }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n() {
  return useContext(I18nContext);
}
