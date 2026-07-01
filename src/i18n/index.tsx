import React, { createContext, useContext, useState, useCallback } from 'react';

export type Lang = 'zh' | 'en';

const messages: Record<Lang, Record<string, string>> = {
  zh: {
    // Appearance
    'appearance': '外观',
    'theme_style': '主题风格',
    'theme_style_desc': '选择配色方向，同时支持深色/浅色模式',
    'theme_mode': '主题模式',
    'theme_mode_desc': '选择深色或浅色模式',
    'text_size': '字号',
    'text_size_desc': '控制界面文字大小',
    'font_family': '字体',
    'font_family_desc': '选择界面显示字体',
    'text_size_small': '小',
    'text_size_default': '中',
    'text_size_large': '大',
    'text_size_xlarge': '特大',
    'font_system': '系统默认',
    'font_yahei': '微软雅黑',
    'font_pingfang': '苹方',
    'font_noto': 'Noto Sans',
    'font_serif': '衬线体',

    // System
    'auto_start': '自动启动代理',
    'auto_start_desc': '启动 IronLink 时自动启用代理',
    'language': '语言',
    'system_settings': '系统设置',
    'settings': '设置',
    'theme': '主题',
    'light_mode': '浅色模式',
    'dark_mode': '深色模式',
    'follow_system': '跟随系统',

    // Navigation
    'overview': '概览',
    'codex_platform': 'Codex 平台',
    'applications': '应用管理',
    'providers': '供应商',
    'about': '关于',
    'logs': '日志',

    // Common
    'save': '保存',
    'save_failed_msg': '保存失败',
    'cancel': '取消',
    'edit': '编辑',
    'copy': '复制',
    'delete': '删除',
    'enabled': '已启用',
    'disabled': '已禁用',
    'not_set': '未设置',
    'loading': '加载中...',
    'failed_to_load': '加载失败',
    'saving': '保存中...',
    'saved': '已保存',
    'confirm_delete': '确认删除?',
    'no_logs': '暂无日志',
    'auto_refresh': '自动刷新',
    'proxy_logs': '代理日志',

    // Providers
    'provider_list': '供应商列表',
    'provider_count': '{count} 个供应商配置',
    'enable_providers': '启用供应商',
    'enable_providers_desc': '关闭后不会在切换时写入 Codex 的配置文件',
    'add_provider': '添加供应商',
    'preset_providers': '预设供应商',
    'select_preset': '选择预设供应商',
    'search_provider': '搜索供应商…',
    'no_match_provider': '没有匹配的供应商',
    'no_match_search': '没有匹配的供应商: {query}',
    'create_from_preset': '从预设创建',
    'back_to_list': '返回列表',
    'collapse_presets': '收起预设',
    'cat_official': '官方',
    'cat_cn_official': '中国官方',
    'cat_aggregator': '聚合/中转',
    'cat_third_party': '第三方',
    'category_official': '官方',
    'category_cn_official': '中国官方',
    'category_aggregator': '聚合/中转',
    'category_third_party': '第三方',
    'protocol_chat': 'Chat Completions',
    'protocol_anthropic': 'Anthropic (Claude)',
    'protocol_responses': 'Responses API',
    'drag_tooltip': '拖动排序',
    'test_connectivity': '测试连通性',
    'models_count': '{count} 模型',
    'field_provider_id': '提供商 ID',
    'field_name': '名称',
    'field_base_url': 'Base URL',
    'field_api_key': 'API Key',
    'field_protocol': '上游协议',
    'field_model_list': '模型列表',
    'new_provider': '新供应商',
    'copy_suffix': ' (副本)',
    'fetch_models': '从上游获取',
    'fetch_models_failed': '获取模型列表失败: {msg}',
    'test_passed': '测试通过: {reply}',
    'test_failed': '测试失败: {msg}',
    'fill_base_url': '请先填写 Base URL',
    'fill_api_key': '请先填写 API Key',
    'model_reasoning_effort': '推理努力度',
    'model_id': '模型 ID',
    'model_list': '模型列表',
    'add_model': '添加模型',
    'no_models': '暂无模型',
    'owned_by': '所属',
    'created': '创建时间',
    'done': '完成',
    'active': '活跃',
    'required': '必填',

    // StatusPanel
    'proxy_active': '代理已启用',
    'proxy_off': '代理已关闭',
    'proxy_active_desc': '所有 Codex API 流量将被拦截并转发',
    'proxy_disabled': '代理已关闭',
    'proxy_disabled_desc': 'Codex API 流量直连原始服务器',
    'enable': '启用',
    'disable': '禁用',
    'port': '端口',
    'backend_type': '后端类型',
    'api_base': 'API 地址',
    'enabled_providers': '已启用供应商模型',
    'api_endpoints': 'API 端点',
    'endpoint_models': '获取所有已启用供应商的模型列表（聚合）',
    'endpoint_chat': 'Chat Completions — 标准 OpenAI 兼容接口',
    'endpoint_responses': 'Responses API — OpenAI 新版接口',
    'codex_config_hint': '# Codex 配置',
    'backup_hint': '启用代理时备份 Codex 原始配置；关闭时自动还原',
    'proxy_enabled_msg': '代理已启用，原始 Codex 配置已备份',
    'proxy_disabled_msg': '代理已关闭，原始 Codex 配置已还原',
    'operation_failed': '操作失败',

    // Codex Platform
    'codex_patch_config': 'Codex 配置注入',
    'codex_config_title': 'Codex 配置文件',
    'codex_config_not_found': '未找到 Codex 配置文件',
    'proxy_url': '代理地址',
    'default_model': '默认模型',
    'default_model_desc': '启用代理时写入 Codex 的默认模型',
    'reasoning_effort': '推理努力度',
    'reasoning_effort_desc': '启用代理时写入 Codex 的默认推理努力度',
    'effort_low': '低',
    'effort_medium': '中',
    'effort_high': '高',
    'save_settings': '保存设置',
    'view_codex_config': '查看配置',
    'restore_hint': '关闭代理时自动恢复 Codex 原始配置',

    // Applications
    'apps_desc': '管理连接 IronLink 的下游客户端（Codex Desktop、Claude Desktop 等），每个应用配置自己的模型映射',
    'model_mappings': '模型映射',
    'add_mapping': '添加映射',
    'mappings_count': '{count} 个映射',
    'no_mappings_hint': '暂无映射',

    // Backend config
    'backend_config': '后端配置',
    'backend_config_desc': '配置 IronLink 后端连接方式',
    'provider_type': '供应商类型',
    'name': '供应商名称',
    'chat_compatible': 'OpenAI Chat 兼容',
    'responses': 'OpenAI Responses',
    'claude': 'Anthropic (Claude)',
    'api_base_url': 'API 地址',
    'api_key': 'API Key',
    'auth_type': '认证方式',
    'bearer_token': 'Bearer Token',
    'x_api_key': 'X-API-Key',
    'none': '无',
    'test_model': '测试模型',
    'test_model_placeholder': '留空则使用默认模型',
    'user_agent': 'User-Agent',
    'user_agent_placeholder': '自定义 User-Agent',
    'custom_headers': '自定义请求头',
    'headers_placeholder': 'JSON 格式的自定义请求头',
    'config_contents': '高级配置',
    'config_contents_placeholder': '自定义 TOML 配置内容',
    'basic': '基本',
    'advanced': '高级',
    'proxy_config_title': 'Codex 代理配置',
    'proxy_config_desc': '配置启用代理时写入 Codex 的默认模型和推理努力度等字段',

    // Model mappings
    'model_mappings_desc': '将 Codex 模型映射到上游平台和模型',
    'select_model_placeholder': '从已选模型中选择',

    // About
    'about_subtitle': '多供应商代理 · 流量拦截与转发',
    'about_description': 'IronLink 是一款面向 AI 开发者的桌面代理工具，拦截 Codex Desktop 本地网络流量，实现多供应商聚合、协议转换、认证模拟和流量管理。',
    'about_platforms': '支持平台',
    'about_ready': '已支持',
    'about_coming_soon': '即将支持',
    'about_feature_1_title': '多供应商',
    'about_feature_1_desc': '同时启用多个 AI 供应商，自动路由',
    'about_feature_2_title': '协议转换',
    'about_feature_2_desc': 'OpenAI Chat / Responses / Anthropic 协议互转',
    'about_feature_3_title': '流量拦截',
    'about_feature_3_desc': '本地代理拦截，无需修改 DNS 或系统代理',
    'about_update_title': '更新',
    'about_update_desc': '检查新版本，获取最新功能和修复',
    'about_check_update': '检查更新',
    'about_download': '下载',
    'about_license': 'MIT 协议',
    'lang_label': '{lang}',
    'runtime': '运行时',
    'version': '版本',

    // Appearance in Settings
    'settings_failed': '设置失败',
    'app_name': 'IronLink',
    'frontend': '前端',

    // Features
    'plan_free': '免费版',
    'plan_plus': 'Plus 版',
    'plan_pro': 'Pro 版',
    'plan_enterprise': '企业版',
    'feature_safe_sandbox': '安全沙箱',
    'feature_codegraph_lite': '轻量 Codegraph',
    'feature_browser': '浏览器',
    'feature_mcp': 'MCP 支持',
    'feature_computer_use': '计算机使用',
    'feature_memory': '记忆',
    'feature_1m_context': '1M 上下文',
    'feature_basic_sandbox': '基础沙箱',
    'feature_basic_browser': '基础浏览器',
    'simulated_auth': '模拟认证',
    'finish': '完成',
    'connectivity_ok': '连接成功，发现 {count} 个模型',
    'connectivity_ok_no_models': '连接成功，但未获取到模型列表',
    'connectivity_failed': '连接测试失败',
    'providers_desc': '管理上游 AI 供应商',
    'select_upstream_model': '选择上游模型',
    'base_url': 'Base URL',
    'protocol': '协议',
  },

  en: {
    // Appearance

    // System

    // Navigation

    // Common

    // Providers

    // StatusPanel

    // Codex Platform

    // Applications

    // Backend config

    // Model mappings

    // About

    // Appearance in Settings

    // Features
  },
};

interface I18nCtx {
  lang: Lang;
  setLang: (l: Lang) => void;
  t: (key: string, params?: Record<string, string | number>) => string;
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

  const t = useCallback((key: string, params?: Record<string, string | number>) => {
    let msg = messages[lang][key] || key;
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        msg = msg.replace(`{${k}}`, String(v));
      }
    }
    return msg;
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
