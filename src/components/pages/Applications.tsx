import { useState, useEffect, useRef } from 'react';
import { Button, Typography, Tag, message as antMsg, Switch, Select, Drawer, Divider, Space, } from 'antd';
import { SettingOutlined, CodeOutlined, SwapOutlined, CloseOutlined, EditOutlined, SaveOutlined } from '@ant-design/icons';
import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { StreamLanguage } from '@codemirror/language';
import { toml } from '@codemirror/legacy-modes/mode/toml';
import { oneDark } from '@codemirror/theme-one-dark';
import { useApps, useProfiles, useProxyConfig } from '../../hooks/useApi';
import { useI18n } from '../../i18n';
import { saveApps, setProxyConfig, toggleProxy, getAutoStart, fetchCodexConfigFile } from '../../api';
import type { AppData } from '../../api';

const { Text, Title } = Typography;
const CODEX_MODELS = ['gpt-5.5', 'gpt-5.4', 'gpt-5.4-mini', 'gpt-5.3-codex', 'gpt-5.2'];

function protocolLabel(p: string, t: (k: string) => string) {
  return p === 'responses' ? t('protocol_responses') : p === 'anthropic' ? t('protocol_anthropic') : p;
}

function CodeMirrorBox({ value }: { value: string }) {
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  useEffect(() => {
    if (!editorRef.current) return;
    const extensions = [basicSetup, EditorView.editable.of(false), StreamLanguage.define(toml), oneDark];
    const state = EditorState.create({ doc: value, extensions });
    const view = new EditorView({ state, parent: editorRef.current });
    viewRef.current = view;
    return () => view.destroy();
  }, []);
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const cur = view.state.doc.toString();
    if (cur !== value) view.dispatch({ changes: { from: 0, to: cur.length, insert: value } });
  }, [value]);
  return <div ref={editorRef} style={{ borderRadius: 6, overflow: 'hidden', border: '1px solid var(--border-subtle)' }} />;
}

export default function Applications() {
  const { t } = useI18n();
  const { data: appsData, refetch: refetchApps } = useApps();
  const { data: profilesData } = useProfiles();
  const { data: proxyCfg } = useProxyConfig();
  const [apps, setApps] = useState<AppData[]>([]);
  const [reasoningEffort, setReasoning] = useState('medium');
  const [proxyEnabled, setProxyEnabled] = useState(false);
  const [_, setAuto] = useState(false);
  const [codexConfig, setCodexConfig] = useState('');
  const [showConfig, setShowConfig] = useState(false);

  // Drawer state
  const [configDrawerApp, setConfigDrawerApp] = useState<AppData | null>(null);
  const [mappingsDrawerApp, setMappingsDrawerApp] = useState<AppData | null>(null);

  useEffect(() => {
    if (appsData) setApps(appsData);
  }, [appsData]);

  useEffect(() => {
    if (proxyCfg) setReasoning(proxyCfg.reasoning_effort);
    getAutoStart().then(setAuto).catch(() => {});
  }, [proxyCfg]);

  const doSave = async (list: AppData[]) => {
    try { await saveApps(list); await refetchApps(); }
    catch { antMsg.error(t('save_failed_msg')); }
  };

  const updateApp = (id: string, patch: Partial<AppData>) => {
    const next = apps.map(a => a.id === id ? { ...a, ...patch } : a);
    setApps(next); doSave(next);
  };

  // Mappings
  const toggleMapping = (appId: string, codexModel: string) => {
    const app = apps.find(a => a.id === appId);
    if (!app) return;
    const mappings = { ...app.model_mappings };
    if (codexModel in mappings) {
      delete mappings[codexModel];
    } else {
      const firstProvider = profilesData?.find(p => p.enabled);
      mappings[codexModel] = {
        provider_id: firstProvider?.provider_id || '',
        upstream_model: firstProvider?.model || '',
      };
    }
    updateApp(appId, { model_mappings: mappings });
  };

  const updateMappingTarget = (appId: string, codexModel: string, field: 'provider_id' | 'upstream_model', value: string) => {
    const app = apps.find(a => a.id === appId);
    if (!app || !(codexModel in app.model_mappings)) return;
    updateApp(appId, {
      model_mappings: { ...app.model_mappings, [codexModel]: { ...app.model_mappings[codexModel], [field]: value } },
    });
  };

  const modelsForProvider = (providerId: string) => {
    if (!profilesData) return [];
    const p = profilesData.find(p => p.enabled && p.provider_id === providerId);
    if (!p) return [];
    const models = [...(p.model_list || [])];
    if (p.model && !models.includes(p.model)) models.unshift(p.model);
    return models;
  };

  // Proxy
  const codexApp = apps.find(a => a.id === 'codex-desktop');
  const handleToggleProxy = async () => {
    const enable = !proxyEnabled;
    try {
      const ok = await toggleProxy(enable);
      if (ok) {
        setProxyEnabled(enable);
        if (enable) await setProxyConfig({ default_model: codexApp?.default_model || 'gpt-5.5', reasoning_effort: reasoningEffort });
        antMsg.success(enable ? t('proxy_enabled_msg') : t('proxy_disabled_msg'));
      }
    } catch { antMsg.error(t('operation_failed')); }
  };

  const handleViewCodexConfig = async () => {
    const content = await fetchCodexConfigFile();
    setCodexConfig(content || t('codex_config_not_found'));
    setShowConfig(true);
  };

  return (
    <div style={{ width: '100%', maxWidth: 800, margin: '0 auto' }}>
      {/* Fluent 2 header */}
      <div className="fluent-card" style={{ padding: '24px 28px', marginBottom: 20, borderRadius: 10 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <div className="fluent-icon-box" style={{
            width: 40, height: 40, borderRadius: 8,
            background: 'var(--accent-bg)', color: 'var(--accent-border)',
            display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
          }}>
            <SettingOutlined style={{ fontSize: 18 }} />
          </div>
          <div>
            <Title level={4} style={{ margin: 0, fontSize: 17, fontWeight: 600 }}>{t('applications')}</Title>
            <Text type="secondary" style={{ fontSize: 12, marginTop: 1, display: 'block' }}>{t('apps_desc')}</Text>
          </div>
        </div>
      </div>

      {/* App cards */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
        {apps.map(app => {
          const mappingCount = Object.keys(app.model_mappings).length;
          const isCodex = app.id === 'codex-desktop';
          const color = isCodex ? '#0078D4' : '#B088E0';
          return (
            <div key={app.id} className="fluent-list-card" style={{
              borderRadius: 8, padding: '16px 20px',
              background: 'var(--card-bg)',
              backdropFilter: 'blur(20px) saturate(160%)',
              WebkitBackdropFilter: 'blur(20px) saturate(160%)',
              border: '1px solid var(--border-subtle)',
              opacity: app.enabled ? 1 : 0.45,
              transition: 'all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
            }}>
              {/* Top row: icon + name + protocol tags + enable switch */}
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 10 }}>
                <Space size={10} align="start">
                  <div style={{
                    width: 34, height: 34, borderRadius: 6,
                    background: `${color}18`, color: color,
                    display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
                    fontSize: 15, fontWeight: 700,
                  }}>
                    {app.name.charAt(0)}
                  </div>
                  <div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                      <Text strong style={{ fontSize: 15, fontWeight: 600 }}>{app.name}</Text>
                      <Tag style={{ margin: 0, fontSize: 9, lineHeight: '16px', padding: '0 6px', borderRadius: 3 }}>{protocolLabel(app.protocol, t)}</Tag>
                      <Tag color={app.enabled ? 'green' : 'default'} style={{ margin: 0, fontSize: 9, lineHeight: '16px', padding: '0 6px', borderRadius: 3 }}>
                        {app.enabled ? t('enabled') : t('disabled')}
                      </Tag>
                    </div>
                    <Space size={14} style={{ fontSize: 11, color: 'var(--text-secondary)', marginTop: 2 }}>
                      <span>{t('default_model')}: <code style={{ fontSize: 10 }}>{app.default_model || '-'}</code></span>
                      {mappingCount > 0 && (
                        <span style={{ cursor: 'pointer', color: 'var(--accent-border)' }}
                          onClick={() => setMappingsDrawerApp(app)}>
                          {mappingCount} {t('mappings_count', { count: mappingCount }).replace('{count} ', '')}
                        </span>
                      )}
                    </Space>
                  </div>
                </Space>
                <Switch checked={app.enabled} onChange={c => updateApp(app.id, { enabled: c })} size="small" />
              </div>

              {/* Mapping tags */}
              {mappingCount > 0 && (
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginBottom: 8, marginTop: 2 }}>
                  {Object.entries(app.model_mappings).slice(0, 4).map(([codex, target]) => (
                    <Tag key={codex} style={{ fontSize: 9, lineHeight: '18px', padding: '0 6px', borderRadius: 3, margin: 0 }}>
                      {codex} <span style={{ opacity: 0.5 }}>→</span> {target.upstream_model}
                    </Tag>
                  ))}
                  {mappingCount > 4 && (
                    <Tag style={{ fontSize: 9, lineHeight: '18px', padding: '0 6px', borderRadius: 3, margin: 0 }}>
                      +{mappingCount - 4}
                    </Tag>
                  )}
                </div>
              )}

              {/* Config summary bar */}
              <div style={{
                display: 'flex', justifyContent: 'space-between', alignItems: 'center',
                padding: '8px 12px', borderRadius: 6,
                background: 'var(--config-row-bg)', marginBottom: 0,
              }}>
                <Space size={8}>
                  <CodeOutlined style={{ fontSize: 12, opacity: 0.4 }} />
                  {app.config_injection ? (
                    <>
                      <Tag style={{ fontSize: 9, fontFamily: 'monospace', lineHeight: '16px', padding: '0 6px', borderRadius: 3, margin: 0 }}>
                        {app.config_injection.config_type}
                      </Tag>
                      <Text style={{ fontSize: 11, fontFamily: 'monospace', color: 'var(--text-secondary)', maxWidth: 280 }} ellipsis>
                        {app.config_injection.config_path}
                      </Text>
                      <Tag color="blue" style={{ fontSize: 9, lineHeight: '16px', padding: '0 6px', borderRadius: 3, margin: 0 }}>
                        configured
                      </Tag>
                    </>
                  ) : (
                    <Text style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{t('config_not_set')}</Text>
                  )}
                </Space>
                <Space size={4}>
                  <Button type="text" size="small" icon={<EditOutlined />}
                    onClick={() => setConfigDrawerApp(app)}
                    style={{ borderRadius: 4, fontSize: 12 }}>
                    {t('config')}
                  </Button>
                  <Button type="text" size="small" icon={<SwapOutlined />}
                    onClick={() => setMappingsDrawerApp(app)}
                    style={{ borderRadius: 4, fontSize: 12 }}>
                    {t('model_mappings')}
                    {mappingCount > 0 ? ` (${mappingCount})` : ''}
                  </Button>
                </Space>
              </div>
            </div>
          );
        })}
      </div>

      {/* ── Config Injection Drawer ── */}
      <Drawer
        title={<Space size={8}><SettingOutlined /><span>{t('config_injection')}: {configDrawerApp?.name}</span></Space>}
        open={!!configDrawerApp}
        onClose={() => setConfigDrawerApp(null)}
        width={520}
        styles={{ body: { padding: '20px 24px' } }}
      >
        {configDrawerApp && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
            {/* Config type + path */}
            <div className="drawer-field">
              <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('config_type')}</Text>
              <Select
                size="small" style={{ width: '100%' }}
                value={configDrawerApp.config_injection?.config_type || 'codex_toml'}
                onChange={v => updateApp(configDrawerApp.id, {
                  config_injection: { config_type: v, config_path: configDrawerApp.config_injection?.config_path || '' }
                })}
                options={[
                  { value: 'codex_toml', label: 'codex_toml' },
                  { value: 'claude_json', label: 'claude_json' },
                  { value: 'custom', label: 'custom' },
                ]}
              />
            </div>

            <div className="drawer-field">
              <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('config_path')}</Text>
              <input className="drawer-input"
                value={configDrawerApp.config_injection?.config_path || ''}
                onChange={e => updateApp(configDrawerApp.id, {
                  config_injection: {
                    config_type: configDrawerApp.config_injection?.config_type || 'codex_toml',
                    config_path: e.target.value,
                  }
                })}
                placeholder="~/.codex/config.toml"
              />
            </div>

            {/* Default model */}
            <div className="drawer-field">
              <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('default_model')}</Text>
              <Select size="small" value={configDrawerApp.default_model} style={{ width: '100%' }}
                onChange={v => updateApp(configDrawerApp.id, { default_model: v })}
                options={CODEX_MODELS.map(m => ({ value: m, label: m }))}
              />
            </div>

            {/* Codex-specific proxy settings */}
            {configDrawerApp.id === 'codex-desktop' && (
              <>
                <Divider style={{ margin: '4px 0', fontSize: 11, color: 'var(--text-tertiary)' }}>
                  {t('proxy_settings')}
                </Divider>

                <div className="drawer-row">
                  <Text style={{ fontSize: 12 }}>{t('proxy')}</Text>
                  <Button size="small"
                    type={proxyEnabled ? 'default' : 'primary'}
                    danger={proxyEnabled}
                    onClick={handleToggleProxy}
                    style={{ borderRadius: 6 }}>
                    {proxyEnabled ? t('disable') : t('enable')}
                  </Button>
                </div>

                <div className="drawer-field">
                  <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('reasoning_effort')}</Text>
                  <Select size="small" value={reasoningEffort} onChange={setReasoning} style={{ width: '100%' }}
                    options={[
                      { value: 'low', label: t('effort_low') },
                      { value: 'medium', label: t('effort_medium') },
                      { value: 'high', label: t('effort_high') },
                    ]} />
                </div>

                <div className="drawer-field">
                  <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>Proxy URL</Text>
                  <input className="drawer-input" readOnly value="http://127.0.0.1:15723/v1" />
                </div>

                <Divider style={{ margin: '4px 0' }} />

                <div style={{ display: 'flex', gap: 8 }}>
                  <Button size="small" icon={<CodeOutlined />} onClick={handleViewCodexConfig} style={{ borderRadius: 6 }}>
                    {t('view_codex_config')}
                  </Button>
                  <Button size="small" icon={<SaveOutlined />} onClick={() => {
                    setProxyConfig({ default_model: codexApp?.default_model || 'gpt-5.5', reasoning_effort: reasoningEffort })
                      .then(() => antMsg.success(t('saved')))
                      .catch(() => antMsg.error(t('save_failed_msg')));
                  }} style={{ borderRadius: 6 }}>
                    {t('save')}
                  </Button>
                </div>
              </>
            )}

            {/* Codex config raw view */}
            {showConfig && codexConfig && configDrawerApp?.id === 'codex-desktop' && (
              <div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
                  <Text strong style={{ fontSize: 12 }}>{t('codex_config_title')}</Text>
                  <Button type="text" size="small" icon={<CloseOutlined />} onClick={() => setShowConfig(false)} style={{ borderRadius: 4 }} />
                </div>
                <CodeMirrorBox value={codexConfig} />
              </div>
            )}
          </div>
        )}
      </Drawer>

      {/* ── Model Mappings Drawer ── */}
      <Drawer
        title={<Space size={8}><SwapOutlined /><span>{t('model_mappings')}: {mappingsDrawerApp?.name}</span></Space>}
        open={!!mappingsDrawerApp}
        onClose={() => setMappingsDrawerApp(null)}
        width={500}
        styles={{ body: { padding: '16px 20px' } }}
      >
        {mappingsDrawerApp && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            {(mappingsDrawerApp.models.length > 0 ? mappingsDrawerApp.models : CODEX_MODELS).map(codexModel => {
              const mapping = mappingsDrawerApp.model_mappings[codexModel];
              return (
                <div key={codexModel} style={{
                  display: 'flex', gap: 6, alignItems: 'center', padding: '6px 8px',
                  borderRadius: 6, background: mapping ? 'var(--config-row-bg)' : 'transparent',
                  transition: 'background 0.15s',
                }}>
                  <Switch size="small" checked={!!mapping}
                    onChange={() => toggleMapping(mappingsDrawerApp.id, codexModel)} />
                  <code style={{ width: 100, fontSize: 12, fontFamily: 'monospace', fontWeight: mapping ? 500 : 400 }}>
                    {codexModel}
                  </code>
                  {mapping ? (
                    <>
                      <span style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>→</span>
                      <Select size="small" value={mapping.provider_id}
                        onChange={v => updateMappingTarget(mappingsDrawerApp.id, codexModel, 'provider_id', v)}
                        style={{ width: 120 }}
                        options={(profilesData || []).filter(p => p.enabled).map(p => ({ value: p.provider_id, label: p.name }))} />
                      <Select size="small" value={mapping.upstream_model}
                        onChange={v => updateMappingTarget(mappingsDrawerApp.id, codexModel, 'upstream_model', v)}
                        style={{ width: 150 }} showSearch
                        options={modelsForProvider(mapping.provider_id).map(m => ({ value: m, label: m }))} />
                    </>
                  ) : (
                    <Text style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{t('no_mappings_hint')}</Text>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </Drawer>
    </div>
  );
}
