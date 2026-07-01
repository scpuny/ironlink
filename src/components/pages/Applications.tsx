import { useState, useEffect, useRef } from 'react';
import { Button, Typography, Tag, message as antMsg, Switch, Select, Collapse, Divider, Space } from 'antd';
import { CodeOutlined, CloseOutlined, SettingOutlined, SwapOutlined } from '@ant-design/icons';
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

function CodeMirrorBox({ value, lang, themeMode }: { value: string; lang: string; themeMode: string }) {
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);

  useEffect(() => {
    if (!editorRef.current) return;
    const extensions = [basicSetup, EditorView.editable.of(false)];
    if (lang === 'toml') extensions.push(StreamLanguage.define(toml));
    if (themeMode === 'dark') extensions.push(oneDark);
    const state = EditorState.create({ doc: value, extensions });
    const view = new EditorView({ state, parent: editorRef.current });
    viewRef.current = view;
    return () => view.destroy();
  }, [lang, themeMode]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const cur = view.state.doc.toString();
    if (cur !== value) {
      view.dispatch({ changes: { from: 0, to: cur.length, insert: value } });
    }
  }, [value]);

  return <div ref={editorRef} style={{ borderRadius: 8, overflow: 'hidden', border: '1px solid var(--border-subtle)', maxHeight: 300, overflowY: 'auto' }} />;
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
      model_mappings: {
        ...app.model_mappings,
        [codexModel]: { ...app.model_mappings[codexModel], [field]: value },
      },
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

  const codexApp = apps.find(a => a.id === 'codex-desktop');
  const handleToggleProxy = async () => {
    const enable = !proxyEnabled;
    try {
      const ok = await toggleProxy(enable);
      if (ok) {
        setProxyEnabled(enable);
        if (enable) {
          await setProxyConfig({ default_model: codexApp?.default_model || 'gpt-5.5', reasoning_effort: reasoningEffort });
        }
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
    <div style={{ width: '100%', maxWidth: 900, margin: '0 auto' }}>
      {/* Fluent 2 header */}
      <div className="fluent-card" style={{ padding: '28px 32px', marginBottom: 24, borderRadius: 10 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <div className="fluent-icon-box" style={{
            width: 44, height: 44, borderRadius: 10,
            background: 'var(--accent-bg)', color: 'var(--accent-border)',
            display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
          }}>
            <SettingOutlined style={{ fontSize: 20 }} />
          </div>
          <div>
            <Title level={4} style={{ margin: 0, fontSize: 18, fontWeight: 600 }}>{t('applications')}</Title>
            <Text type="secondary" style={{ fontSize: 13, marginTop: 2, display: 'block' }}>{t('apps_desc')}</Text>
          </div>
        </div>
      </div>

      {/* App cards */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
        {apps.map(app => {
          const mappingCount = Object.keys(app.model_mappings).length;
          const isCodex = app.id === 'codex-desktop';
          return (
            <div key={app.id} className="fluent-app-card" style={{
              borderRadius: 10, overflow: 'hidden',
              background: 'var(--card-bg)',
              backdropFilter: 'blur(24px) saturate(160%)',
              WebkitBackdropFilter: 'blur(24px) saturate(160%)',
              border: '1px solid var(--border-subtle)',
              transition: 'all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
              opacity: app.enabled ? 1 : 0.5,
            }}>
              {/* Card header */}
              <div style={{ padding: '20px 24px', borderBottom: '1px solid var(--border-subtler)' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                  <Space size={12} align="start">
                    <div className="fluent-icon-box" style={{
                      width: 40, height: 40, borderRadius: 8,
                      background: isCodex ? 'rgba(0,120,212,0.15)' : 'rgba(176,136,224,0.15)',
                      color: isCodex ? '#0078D4' : '#B088E0',
                      display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
                      fontSize: 18, fontWeight: 700,
                    }}>
                      {app.name.charAt(0)}
                    </div>
                    <div>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 2 }}>
                        <Text strong style={{ fontSize: 16, fontWeight: 600 }}>{app.name}</Text>
                        <Tag style={{ margin: 0, fontSize: 10, lineHeight: '18px', padding: '0 8px', borderRadius: 4 }}>
                          {protocolLabel(app.protocol, t)}
                        </Tag>
                        <Tag color={app.enabled ? 'green' : 'default'} style={{ margin: 0, fontSize: 10, lineHeight: '18px', padding: '0 8px', borderRadius: 4 }}>
                          {app.enabled ? t('enabled') : t('disabled')}
                        </Tag>
                      </div>
                      <Space size={16} style={{ fontSize: 12, color: 'var(--text-secondary)' }}>
                        <span>{t('default_model')}: <code style={{ fontSize: 11 }}>{app.default_model || '-'}</code></span>
                        {app.models.length > 0 && <span>{app.models.length} {t("models")}</span>}
                        {mappingCount > 0 && <span>{mappingCount} {t('model_mappings').toLowerCase()}</span>}
                      </Space>
                    </div>
                  </Space>
                  <Switch checked={app.enabled} onChange={c => updateApp(app.id, { enabled: c })} size="small" />
                </div>
              </div>

              {/* Collapsible panels */}
              <div style={{ padding: '0 24px' }}>
                <Collapse ghost size="small"
                  expandIconPosition="end"
                  style={{ background: 'transparent' }}
                  items={[
                    // Config injection panel
                    app.config_injection ? {
                      key: 'injection',
                      label: (
                        <Space size={6}>
                          <SettingOutlined style={{ fontSize: 13, opacity: 0.5 }} />
                          <span style={{ fontSize: 13, fontWeight: 500 }}>{t('config_injection')}</span>
                        </Space>
                      ),
                      children: (
                        <div style={{ padding: '8px 0 12px 20px', borderLeft: '2px solid var(--border-subtler)', marginLeft: 6 }}>
                          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 12, fontSize: 12 }}>
                            <div>
                              <Text style={{ fontSize: 11, color: 'var(--text-tertiary)', display: 'block', marginBottom: 2 }}>{t('config_type')}</Text>
                              <Tag style={{ fontSize: 10, fontFamily: 'monospace' }}>{app.config_injection.config_type}</Tag>
                            </div>
                            <div>
                              <Text style={{ fontSize: 11, color: 'var(--text-tertiary)', display: 'block', marginBottom: 2 }}>{t('config_path')}</Text>
                              <div style={{ fontSize: 11, fontFamily: 'monospace', wordBreak: 'break-all', color: 'var(--text-secondary)' }}>{app.config_injection.config_path}</div>
                            </div>
                          </div>
                          {isCodex && (
                            <>
                              <Divider style={{ margin: '8px 0' }} />
                              <Space wrap size={8} align="center">
                                <Text style={{ fontSize: 12 }}>{t('reasoning_effort')}:</Text>
                                <Select size="small" value={reasoningEffort} onChange={setReasoning}
                                  style={{ width: 100 }}
                                  options={[
                                    { value: 'low', label: t('effort_low') },
                                    { value: 'medium', label: t('effort_medium') },
                                    { value: 'high', label: t('effort_high') },
                                  ]} />
                                <Button size="small"
                                  type={proxyEnabled ? 'default' : 'primary'}
                                  danger={proxyEnabled}
                                  onClick={handleToggleProxy}
                                  style={{ borderRadius: 6 }}>
                                  {proxyEnabled ? t('disable') : t('enable')}
                                </Button>
                                <Button size="small" icon={<CodeOutlined />} onClick={handleViewCodexConfig}
                                  style={{ borderRadius: 6 }}>
                                  {t('view_codex_config')}
                                </Button>
                              </Space>
                            </>
                          )}
                        </div>
                      ),
                    } : null,

                    // Model mappings panel
                    {
                      key: 'mappings',
                      label: (
                        <Space size={6}>
                          <SwapOutlined style={{ fontSize: 13, opacity: 0.5 }} />
                          <span style={{ fontSize: 13, fontWeight: 500 }}>{t('model_mappings')}</span>
                          {mappingCount > 0 && (
                            <Tag style={{ fontSize: 10, lineHeight: '16px', padding: '0 6px', borderRadius: 3 }}>
                              {mappingCount}
                            </Tag>
                          )}
                        </Space>
                      ),
                      children: (
                        <div style={{ padding: '4px 0 8px 20px', borderLeft: '2px solid var(--border-subtler)', marginLeft: 6 }}>
                          {(app.models.length > 0 ? app.models : CODEX_MODELS).map(codexModel => {
                            const mapping = app.model_mappings[codexModel];
                            return (
                              <div key={codexModel}
                                style={{
                                  display: 'flex', gap: 8, alignItems: 'center', padding: '6px 8px',
                                  borderRadius: 6, marginBottom: 2,
                                  background: mapping ? 'var(--config-row-bg)' : 'transparent',
                                  transition: 'background 0.15s',
                                }}>
                                <Switch size="small" checked={!!mapping}
                                  onChange={() => toggleMapping(app.id, codexModel)} />
                                <code style={{ width: 100, fontSize: 12, fontFamily: 'monospace', fontWeight: mapping ? 500 : 400 }}>
                                  {codexModel}
                                </code>
                                {mapping ? (
                                  <>
                                    <span style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>→</span>
                                    <Select size="small" value={mapping.provider_id}
                                      onChange={v => updateMappingTarget(app.id, codexModel, 'provider_id', v)}
                                      style={{ width: 130 }}
                                      options={(profilesData || []).filter(p => p.enabled).map(p => ({ value: p.provider_id, label: p.name }))} />
                                    <Select size="small" value={mapping.upstream_model}
                                      onChange={v => updateMappingTarget(app.id, codexModel, 'upstream_model', v)}
                                      style={{ width: 160 }} showSearch
                                      options={modelsForProvider(mapping.provider_id).map(m => ({ value: m, label: m }))} />
                                  </>
                                ) : (
                                  <Text style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>
                                    {t('no_mappings_hint')}
                                  </Text>
                                )}
                              </div>
                            );
                          })}
                        </div>
                      ),
                    },
                  ].filter(Boolean) as any}
                />
              </div>
            </div>
          );
        })}
      </div>

      {/* Codex config raw view with CodeMirror */}
      {showConfig && codexConfig && (
        <div className="fluent-app-card" style={{
          marginTop: 16, borderRadius: 10, overflow: 'hidden',
          background: 'var(--card-bg)', backdropFilter: 'blur(24px) saturate(160%)',
          WebkitBackdropFilter: 'blur(24px) saturate(160%)',
          border: '1px solid var(--border-subtle)',
        }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '14px 20px', borderBottom: '1px solid var(--border-subtler)' }}>
            <Text strong style={{ fontSize: 13 }}>{t('codex_config_title')}</Text>
            <Button type="text" size="small" icon={<CloseOutlined />} onClick={() => setShowConfig(false)}
              style={{ borderRadius: 6 }} />
          </div>
          <div style={{ padding: 12 }}>
            <CodeMirrorBox value={codexConfig} lang="toml" themeMode="dark" />
          </div>
        </div>
      )}
    </div>
  );
}
