import { useState, useEffect, useRef } from 'react';
import { Button, Typography, Tag, message as antMsg, Switch, Card, Select, Collapse, Divider } from 'antd';
import { CodeOutlined, CloseOutlined } from '@ant-design/icons';
import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { StreamLanguage } from '@codemirror/language';
import { toml } from '@codemirror/legacy-modes/mode/toml';
import { oneDark } from '@codemirror/theme-one-dark';
import { useApps, useProfiles, useProxyConfig } from '../../hooks/useApi';
import { useI18n } from '../../i18n';
import { saveApps, setProxyConfig, toggleProxy, getAutoStart, fetchCodexConfigFile } from '../../api';
import type { AppData } from '../../api';

const { Text } = Typography;
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
      view.dispatch({
        changes: { from: 0, to: cur.length, insert: value },
      });
    }
  }, [value]);

  return (
    <div ref={editorRef} style={{
      borderRadius: 8, overflow: 'hidden',
      border: '1px solid var(--border-subtle)',
      maxHeight: 300, overflowY: 'auto',
    }} />
  );
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

  // ── 模型映射操作 ──
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

  // ── Codex 配置注入 ──
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
    <div style={{ width: '100%' }}>
      <Card className="hover-card" style={{ borderRadius: 12, marginBottom: 16 }}>
        <Typography.Title level={5} style={{ margin: 0 }}>{t('applications')}</Typography.Title>
        <Typography.Text type="secondary" style={{ fontSize: 13 }}>{t('apps_desc')}</Typography.Text>
      </Card>

      {apps.map(app => (
        <Card key={app.id} className="hover-card" size="small"
          style={{ borderRadius: 10, marginBottom: 12, opacity: app.enabled ? 1 : 0.55 }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>

            {/* ── Header row ── */}
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
              <div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
                  <Text strong style={{ fontSize: 16 }}>{app.name}</Text>
                  <Tag style={{ margin: 0 }}>{protocolLabel(app.protocol, t)}</Tag>
                </div>
                <div style={{ display: 'flex', gap: 12, fontSize: 12, color: 'var(--text-secondary)' }}>
                  <span>{t('default_model')}: <code style={{ fontSize: 11 }}>{app.default_model || '-'}</code></span>
                  <span>{app.models.length > 0 ? `${app.models.length} ${t('models')}` : ''}</span>
                </div>
              </div>
              <Switch checked={app.enabled} onChange={c => updateApp(app.id, { enabled: c })} size="small" />
            </div>

            {/* ── Collapsible details ── */}
            <Collapse ghost size="small" items={[
              // Config injection panel
              app.config_injection ? {
                key: 'injection',
                label: <span style={{ fontSize: 12 }}>{t('config_injection')}</span>,
                children: (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 8, padding: '4px 0' }}>
                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, fontSize: 12 }}>
                      <div>
                        <Text style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{t('config_type')}</Text>
                        <div><code style={{ fontSize: 11 }}>{app.config_injection.config_type}</code></div>
                      </div>
                      <div>
                        <Text style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{t('config_path')}</Text>
                        <div style={{ fontSize: 11, fontFamily: 'monospace', wordBreak: 'break-all' }}>{app.config_injection.config_path}</div>
                      </div>
                    </div>
                    {app.id === 'codex-desktop' && (
                      <>
                        <Divider style={{ margin: '4px 0' }} />
                        <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
                          <Text style={{ fontSize: 11 }}>{t('reasoning_effort')}:</Text>
                          <Select size="small" value={reasoningEffort} onChange={setReasoning}
                            style={{ width: 100 }}
                            options={[
                              { value: 'low', label: t('effort_low') },
                              { value: 'medium', label: t('effort_medium') },
                              { value: 'high', label: t('effort_high') },
                            ]} />
                          <div style={{ flex: 1 }} />
                          <Button size="small" type={proxyEnabled ? 'default' : 'primary'}
                            danger={proxyEnabled}
                            onClick={handleToggleProxy} shape="round">
                            {proxyEnabled ? t('disable') : t('enable')}
                          </Button>
                          <Button size="small" icon={<CodeOutlined />} onClick={handleViewCodexConfig}>
                            {t('view_codex_config')}
                          </Button>
                        </div>
                      </>
                    )}
                  </div>
                ),
              } : { key: 'empty', label: '', children: null },

              // Model mappings panel
              {
                key: 'mappings',
                label: <span style={{ fontSize: 12 }}>{t('model_mappings')}
                  {Object.keys(app.model_mappings).length > 0 &&
                    ` (${Object.keys(app.model_mappings).length})`}</span>,
                children: (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                    {(app.models.length > 0 ? app.models : CODEX_MODELS).map(codexModel => {
                      const mapping = app.model_mappings[codexModel];
                      return (
                        <div key={codexModel}
                          style={{
                            display: 'flex', gap: 6, alignItems: 'center', padding: '4px 8px',
                            borderRadius: 6, background: mapping ? 'var(--config-row-bg)' : 'transparent',
                          }}>
                          <Switch size="small" checked={!!mapping}
                            onChange={() => toggleMapping(app.id, codexModel)} />
                          <code style={{ width: 100, fontSize: 12, fontFamily: 'monospace' }}>{codexModel}</code>
                          {mapping && (
                            <>
                              <span style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>→</span>
                              <Select size="small" value={mapping.provider_id}
                                onChange={v => updateMappingTarget(app.id, codexModel, 'provider_id', v)}
                                style={{ width: 130 }}
                                options={(profilesData || []).filter(p => p.enabled).map(p => ({ value: p.provider_id, label: p.name }))} />
                              <Select size="small" value={mapping.upstream_model}
                                onChange={v => updateMappingTarget(app.id, codexModel, 'upstream_model', v)}
                                style={{ width: 160 }} showSearch
                                options={modelsForProvider(mapping.provider_id).map(m => ({ value: m, label: m }))} />
                            </>
                          )}
                        </div>
                      );
                    })}
                  </div>
                ),
              },
            ].filter(i => i.key !== 'empty')} />

          </div>
        </Card>
      ))}

      {/* ── Codex config raw view ── */}
      {showConfig && codexConfig && (
        <Card className="hover-card" size="small" style={{ borderRadius: 10 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
            <Text strong style={{ fontSize: 12 }}>{t('codex_config_title')}</Text>
            <Button type="text" size="small" icon={<CloseOutlined />} onClick={() => setShowConfig(false)} />
          </div>
          <CodeMirrorBox value={codexConfig} lang="toml" themeMode="dark" />
        </Card>
      )}
    </div>
  );
}
