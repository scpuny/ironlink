import { useState, useEffect, useRef } from 'react';
import { Button, Typography, Tag, message as antMsg, Switch, Select, Divider, Space, Modal, Tabs } from 'antd';
import { SettingOutlined, CodeOutlined, EditOutlined, CloseOutlined, ArrowLeftOutlined, CheckOutlined, ArrowRightOutlined, FolderOpenOutlined, FileTextOutlined } from '@ant-design/icons';
import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { StreamLanguage } from '@codemirror/language';
import { toml } from '@codemirror/legacy-modes/mode/toml';
import { oneDark } from '@codemirror/theme-one-dark';
import { useApps, useProfiles, useProxyConfig } from '../../hooks/useApi';
import { useI18n } from '../../i18n';
import { saveApps, setProxyConfig, toggleProxy, getAutoStart, fetchCodexConfigFile, previewAppConfig, getAppConfigFiles } from '../../api';
import type { AppData, ConfigFileEntry } from '../../api';

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
  const [configModalOpen, setConfigModalOpen] = useState(false);
  const [configModalContent, setConfigModalContent] = useState('');
  const [configModalTitle, setConfigModalTitle] = useState('');
  const [configModalLoading, setConfigModalLoading] = useState(false);
  const [configFiles, setConfigFiles] = useState<ConfigFileEntry[]>([]);
  const [configFileTab, setConfigFileTab] = useState('0');

  const openConfigModal = async (app: AppData) => {
    setConfigModalTitle(app.name + ' - ' + t('preview_config'));
    setConfigModalOpen(true);
    setConfigModalLoading(true);
    try {
      const text = await previewAppConfig(app.id);
      setConfigModalContent(text || '// ' + 'No preview content');
    } catch (e: any) {
      setConfigModalContent('// Error generating preview: ' + (e?.message || String(e)));
    }
    setConfigModalLoading(false);
  };

  const openViewConfigModal = async (app: AppData) => {
    setConfigModalTitle(app.name + ' - ' + t('current_config'));
    setConfigModalOpen(true);
    setConfigModalLoading(true);
    setConfigFileTab('0');
    try {
      const files = await getAppConfigFiles(app.id);
      setConfigFiles(files.length > 0 ? files : [{ name: t('config_file_empty'), path: '', content: '// ' + t('config_file_empty') }]);
      if (files.length > 0) {
        setConfigModalContent(files[0].content);
      } else {
        setConfigModalContent('// ' + t('config_file_empty'));
      }
    } catch (e: any) {
      setConfigFiles([{ name: 'Error', path: '', content: '// ' + t('error_reading_config') + ': ' + (e?.message || String(e)) }]);
      setConfigModalContent('// ' + t('error_reading_config') + ': ' + (e?.message || String(e)));
    }
    setConfigModalLoading(false);
  };

  // Edit mode: single form per app
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState<AppData | null>(null);

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

  const startEdit = (app: AppData) => {
    setDraft({ ...app, config_injection: app.config_injection ? { ...app.config_injection, config_dir: app.config_injection.config_dir, backup_enabled: app.config_injection.backup_enabled ?? true, fields: app.config_injection.fields } : null, model_mappings: { ...app.model_mappings } });
    setEditingId(app.id);
  };

  const cancelEdit = () => {
    setEditingId(null);
    setDraft(null);
  };

  const saveEdit = () => {
    if (!draft) return;
    updateApp(draft.id, draft);
    cancelEdit();
  };

  const updateDraft = (patch: Partial<AppData>) => {
    if (!draft) return;
    setDraft({ ...draft, ...patch });
  };

  // Mapping helpers
  const toggleMapping = (codexModel: string) => {
    if (!draft) return;
    const mappings = { ...draft.model_mappings };
    if (codexModel in mappings) {
      delete mappings[codexModel];
    } else {
      const firstProvider = profilesData?.find(p => p.enabled);
      mappings[codexModel] = {
        provider_id: firstProvider?.provider_id || '',
        upstream_model: firstProvider?.model || '',
      };
    }
    setDraft({ ...draft, model_mappings: mappings });
  };

  const updateMappingTarget = (codexModel: string, field: 'provider_id' | 'upstream_model', value: string) => {
    if (!draft || !(codexModel in draft.model_mappings)) return;
    setDraft({
      ...draft,
      model_mappings: { ...draft.model_mappings, [codexModel]: { ...draft.model_mappings[codexModel], [field]: value } },
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
    <>
    <div style={{ width: '100%', margin: '0 auto' }}>
      {/* Page header */}
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

      {/* App list */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
        {apps.map(app => {
          const mappingCount = Object.keys(app.model_mappings).length;
          const isCodex = app.id === 'codex-desktop';
          const isEditing = editingId === app.id;
          const color = isCodex ? '#0078D4' : '#B088E0';

          // ── Edit mode: single unified form ──
          if (isEditing && draft) {
            return (
              <div key={app.id} className="fluent-list-card" style={{
                borderRadius: 8, padding: '20px 24px',
                background: 'var(--card-bg)',
                backdropFilter: 'blur(20px) saturate(160%)',
                WebkitBackdropFilter: 'blur(20px) saturate(160%)',
                border: '2px solid var(--accent-border)',
              }}>
                {/* Header */}
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 20 }}>
                  <Space size={8}>
                    <Button type="text" size="small" icon={<ArrowLeftOutlined />} onClick={cancelEdit} style={{ borderRadius: 4 }} />
                    <Text strong style={{ fontSize: 15 }}>{draft.name}</Text>
                  </Space>
                  <Space size={4}>
                    <Button  onClick={cancelEdit} style={{ borderRadius: 4 }}>{t('cancel')}</Button>
                    <Button  type="primary" icon={<CheckOutlined />} onClick={saveEdit} style={{ borderRadius: 4 }}>{t('save')}</Button>
                  </Space>
                </div>

                {/* Basic settings */}
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 16 }}>
                  <div>
                    <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('default_model')}</Text>
                    <Select  value={draft.default_model} style={{ width: '100%' }}
                      onChange={v => updateDraft({ default_model: v })}
                      options={CODEX_MODELS.map(m => ({ value: m, label: m }))} />
                  </div>
                  <div>
                    <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('enabled')}</Text>
                    <Switch checked={draft.enabled} onChange={c => updateDraft({ enabled: c })} />
                  </div>
                </div>

                {/* Config injection */}
                <Text strong style={{ fontSize: 12, display: 'block', marginBottom: 8 }}>{t('config_injection')}</Text>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 16 }}>
                  <div>
                    <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('config_type')}</Text>
                    <Select  style={{ width: '100%' }}
                      value={draft.config_injection?.config_type || 'codex_toml'}
                      onChange={v => updateDraft({ config_injection: { config_type: v, config_path: draft.config_injection?.config_path || '', backup_enabled: draft.config_injection?.backup_enabled ?? true } })}
                      options={[
                        { value: 'codex_toml', label: 'codex_toml' },
                        { value: 'claude_json', label: 'claude_json' },
                        { value: 'custom', label: 'custom' },
                      ]} />
                  </div>
                  <div>
                    <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('config_path')}</Text>
                    <input className="drawer-input"
                      value={draft.config_injection?.config_path || ''}
                      onChange={e => updateDraft({ config_injection: { config_type: draft.config_injection?.config_type || 'codex_toml', config_path: e.target.value, backup_enabled: draft.config_injection?.backup_enabled ?? true } })}
                      placeholder="~/.codex/config.toml" />
                  </div>
                </div>

                {/* Advanced injection config */}
                <Divider style={{ margin: '4px 0', fontSize: 11, color: 'var(--text-tertiary)' }}>{t('advanced')}</Divider>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 16 }}>
                  <div style={{ gridColumn: '1 / -1' }}>
                    <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 2 }}>{t('config_dir')}</Text>
                    <Text type="secondary" style={{ fontSize: 11, display: 'block', marginBottom: 6 }}>{t('config_dir_desc')}</Text>
                    <div style={{ display: 'flex', gap: 8 }}>
                      <input className="drawer-input"
                        value={draft.config_injection?.config_dir || ''}
                        onChange={e => updateDraft({ config_injection: { ...draft.config_injection!, config_dir: e.target.value || undefined } })}
                        placeholder={t('config_dir_placeholder')}
                        style={{ flex: 1 }} />
                      <Button size="small" icon={<FolderOpenOutlined />} onClick={async () => {
                        const { open } = await import('@tauri-apps/plugin-dialog');
                        const dir = await open({ directory: true, multiple: false, title: t('config_dir') });
                        if (dir) updateDraft({ config_injection: { ...draft.config_injection!, config_dir: dir } });
                      }} style={{ borderRadius: 6, fontSize: 11 }}>{t('browse')}</Button>
                    </div>
                  </div>
                  <div>
                    <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('backup_enabled')}</Text>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                      <Switch checked={draft.config_injection?.backup_enabled ?? true}
                        onChange={c => updateDraft({ config_injection: { ...draft.config_injection!, backup_enabled: c } })} />
                      <Text style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{t('backup_enabled_desc')}</Text>
                    </div>
                  </div>
                </div>

                {/* Inject fields */}
                {draft.config_injection?.config_type === 'codex_toml' && (
                  <div style={{ marginBottom: 16 }}>
                    <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('inject_fields')}</Text>
                    <Text type="secondary" style={{ fontSize: 11, display: 'block', marginBottom: 8 }}>{t('inject_fields_desc')}</Text>
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                      {['model', 'reasoning_effort', 'model_catalog_json', 'model_providers', 'marketplaces'].map(f => {
                        const labels: Record<string, string> = {
                          model: t('inject_field_model'),
                          reasoning_effort: t('inject_field_reasoning'),
                          model_catalog_json: t('inject_field_catalog'),
                          model_providers: t('inject_field_providers'),
                          marketplaces: t('inject_field_marketplaces'),
                        };
                        const selected = !draft.config_injection?.fields || draft.config_injection.fields.includes(f);
                        return (
                          <Tag key={f}
                            color={selected ? 'blue' : 'default'}
                            style={{ cursor: 'pointer', borderRadius: 4, padding: '2px 8px', fontSize: 11 }}
                            onClick={() => {
                              const current = draft.config_injection?.fields || ['model', 'reasoning_effort', 'model_catalog_json', 'model_providers', 'marketplaces'];
                              const next = selected
                                ? current.filter(x => x !== f)
                                : [...current, f];
                              updateDraft({ config_injection: { ...draft.config_injection!, fields: next.length > 0 ? next : undefined } });
                            }}
                          >{labels[f]}</Tag>
                        );
                      })}
                    </div>
                  </div>
                )}

                {/* Config snippet */}
                <div style={{ marginBottom: 16 }}>
                  <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('config_snippet')}</Text>
                  <textarea className="drawer-textarea"
                    value={draft.config_snippet || ''}
                    onChange={e => updateDraft({ config_snippet: e.target.value || undefined })}
                    placeholder={t('config_snippet_placeholder')}
                    rows={3}
                  />
                </div>

                {/* Codex proxy settings */}
                {isCodex && (
                  <>
                    <Divider style={{ margin: '4px 0', fontSize: 11, color: 'var(--text-tertiary)' }}>{t('proxy_settings')}</Divider>
                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 16 }}>
                      <div>
                        <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('reasoning_effort')}</Text>
                        <Select  value={reasoningEffort} onChange={setReasoning} style={{ width: '100%' }}
                          options={[
                            { value: 'low', label: t('effort_low') },
                            { value: 'medium', label: t('effort_medium') },
                            { value: 'high', label: t('effort_high') },
                          ]} />
                      </div>
                      <div style={{ display: 'flex', alignItems: 'flex-end', gap: 8, paddingBottom: 2 }}>
                        <div>
                          <Text style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'block', marginBottom: 4 }}>{t('proxy')}</Text>
                          <Button  type={proxyEnabled ? 'default' : 'primary'} danger={proxyEnabled}
                            onClick={handleToggleProxy} style={{ borderRadius: 6, minWidth: 80 }}>
                            {proxyEnabled ? t('disable') : t('enable')}
                          </Button>
                        </div>
                        <Button  icon={<CodeOutlined />} onClick={handleViewCodexConfig} style={{ borderRadius: 6 }}>
                          {t('view_codex_config')}
                        </Button>
                      </div>
                    </div>

                    {showConfig && codexConfig && (
                      <div style={{ marginBottom: 16 }}>
                        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 6 }}>
                          <Text strong style={{ fontSize: 12 }}>{t('codex_config_title')}</Text>
                          <Button type="text"  icon={<CloseOutlined />} onClick={() => setShowConfig(false)} style={{ borderRadius: 4 }} />
                        </div>
                        <CodeMirrorBox value={codexConfig} />
                      </div>
                    )}
                  </>
                )}

                {/* Model mappings */}
                <Divider style={{ margin: '16px 0 12px', fontSize: 11, color: 'var(--text-tertiary)' }}>{t('model_mappings')}</Divider>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 5 }}>
                  {(draft.models.length > 0 ? draft.models : CODEX_MODELS).map(codexModel => {
                    const mapping = draft.model_mappings[codexModel];
                    return (
                      <div key={codexModel} style={{
                        display: 'flex', gap: 12, alignItems: 'center', padding: '5px 8px',
                        borderRadius: 6, background: 'var(--config-row-bg)',
                        minHeight: '45px'
                      }}>
                        <Switch  checked={!!mapping} onChange={() => toggleMapping(codexModel)} />
                        <code style={{ width: 100, fontSize: 12, fontFamily: 'monospace', fontWeight: mapping ? 500 : 400 }}>{codexModel}</code>
                        {mapping ? (
                          <>
                            <span style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>
                              <ArrowRightOutlined />
                            </span>
                            <Select  value={mapping.provider_id}
                              onChange={v => updateMappingTarget(codexModel, 'provider_id', v)}
                              style={{ width: 200 }}
                              options={(profilesData || []).filter(p => p.enabled).map(p => ({ value: p.provider_id, label: p.name }))} />
                            <Select  value={mapping.upstream_model}
                              onChange={v => updateMappingTarget(codexModel, 'upstream_model', v)}
                              style={{ width: 260 }} showSearch
                              options={modelsForProvider(mapping.provider_id).map(m => ({ value: m, label: m }))} />
                          </>
                        ) : (
                          <Text style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>{t('no_mappings_hint')}</Text>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            );
          }

          // ── View mode ──
          return (
            <div key={app.id} className="fluent-list-card" style={{
              borderRadius: 8, padding: '16px 20px',
              background: 'var(--card-bg)',
              backdropFilter: 'blur(20px) saturate(160%)',
              WebkitBackdropFilter: 'blur(20px) saturate(160%)',
              border: '1px solid var(--border-subtle)',
              opacity: app.enabled ? 1 : 0.45,
            }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 8 }}>
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
                      {mappingCount > 0 && <span>{mappingCount} {t('mappings_count', { count: mappingCount }).replace('{count} ', '')}</span>}
                    </Space>
                  </div>
                </Space>
                <Switch checked={app.enabled} onChange={c => updateApp(app.id, { enabled: c })}  />
              </div>

              {mappingCount > 0 && (
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginBottom: 8 }}>
                  {Object.entries(app.model_mappings).slice(0, 4).map(([codex, target]) => (
                    <Tag key={codex} style={{ fontSize: 9, lineHeight: '18px', padding: '0 6px', borderRadius: 3, margin: 0 }}>
                      {codex} <span style={{ opacity: 0.5 }}>→</span> {target.upstream_model}
                    </Tag>
                  ))}
                  {mappingCount > 4 && <Tag style={{ fontSize: 9, lineHeight: '18px', padding: '0 6px', borderRadius: 3, margin: 0 }}>+{mappingCount - 4}</Tag>}
                </div>
              )}

              <div style={{
                display: 'flex', justifyContent: 'space-between', alignItems: 'center',
                padding: '8px 12px', borderRadius: 6, background: 'var(--config-row-bg)',
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
                      <Tag color="blue" style={{ fontSize: 9, lineHeight: '16px', padding: '0 6px', borderRadius: 3, margin: 0 }}>configured</Tag>
                    </>
                  ) : (
                    <Text style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{t('config_not_set')}</Text>
                  )}
                </Space>
                <Space size={2}>
                  {app.config_injection?.config_path && (
                    <>
                      <Button type="text" size="small" icon={<FileTextOutlined />} onClick={() => openViewConfigModal(app)} style={{ borderRadius: 4, fontSize: 12 }}>{t('view_config')}</Button>
                      <Button type="text" size="small" icon={<CodeOutlined />} onClick={() => openConfigModal(app)} style={{ borderRadius: 4, fontSize: 12 }}>{t('preview_config')}</Button>
                    </>
                  )}
                  <Button type="text" size="small" icon={<EditOutlined />} onClick={() => startEdit(app)} style={{ borderRadius: 4, fontSize: 12 }}>{t('edit')}</Button>
                </Space>
              </div>
            </div>
          );
        })}
      </div>
    </div>

      {/* Config file viewer modal */}
      <Modal
        title={<span style={{ fontSize: 14, fontWeight: 600 }}>{configModalTitle}</span>}
        open={configModalOpen}
        onCancel={() => { setConfigModalOpen(false); setConfigFiles([]); }}
        footer={null}
        width={720}
        styles={{ body: { padding: 0, maxHeight: 520, overflow: 'auto' } }}
      >
        {configModalLoading ? (
          <div style={{ padding: 40, textAlign: 'center', color: 'var(--text-tertiary)' }}>{t('loading')}</div>
        ) : configFiles.length > 1 ? (
          <Tabs
            activeKey={configFileTab}
            onChange={setConfigFileTab}
            size="small"
            style={{ padding: '0 4px' }}
            items={configFiles.map((f, i) => ({
              key: String(i),
              label: <span style={{ fontSize: 12 }}>{f.name}</span>,
              children: (
                <div>
                  <div style={{ fontSize: 11, color: 'var(--text-tertiary)', marginBottom: 8, wordBreak: 'break-all' }}>{f.path}</div>
                  <CodeMirrorBox value={f.content} />
                </div>
              ),
            }))}
          />
        ) : (
          <div style={{ padding: '8px 12px' }}>
            <CodeMirrorBox value={configModalContent} />
          </div>
        )}
      </Modal>
    </>
  );
}
