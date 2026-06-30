import { useState, useEffect, useMemo, useCallback } from 'react';
import { Button, Input, Typography, Tag, message as antMsg, Tooltip, Switch, Card, Checkbox, Select } from 'antd';
import {
  PlusOutlined, EditOutlined, CopyOutlined,
  DeleteOutlined, ApiOutlined, HolderOutlined,
  ArrowLeftOutlined, SearchOutlined, SaveOutlined, DownloadOutlined,
} from '@ant-design/icons';
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core';
import {
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
// ponytail: module-level drag state


import { PRESETS } from '../../presets';
import type { ProviderPreset } from '../../presets';
import { useProfiles, useProxyConfig } from '../../hooks/useApi';
import { useI18n } from '../../i18n';
import { saveProfiles, fetchUpstreamModels, setProxyConfig } from '../../api';
import type { RelayProfileData } from '../../api';
import type { ProxyConfig } from '../../types';

const { Text } = Typography;

const CAT_LABELS: Record<string, string> = {
  official: 'cat_official',
  cn_official: 'cat_cn_official',
  aggregator: 'cat_aggregator',
  third_party: 'cat_third_party',
};

const CAT_ORDER = ['official', 'cn_official', 'aggregator', 'third_party'];

function protocolLabel(p: string, _t?: (k: string) => string) {
  return p === 'responses' ? (_t ? _t('protocol_responses') : 'Responses API') : p === 'anthropic' ? (_t ? _t('protocol_anthropic') : 'Anthropic') : (_t ? _t('protocol_chat') : 'Chat Completions');
}

function initialFor(name: string) {
  return name.charAt(0).toUpperCase();
}

interface RelayProfile {
  id: string;
  providerId: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  protocol: 'responses' | 'chatCompletions' | 'anthropic';
  model: string;
  testModel: string;
  modelList: string;
  enabled: boolean;
  active: boolean;
}

function fromApiProfile(p: RelayProfileData): RelayProfile {
  return {
    id: p.id, providerId: p.provider_id, name: p.name, baseUrl: p.base_url, apiKey: p.api_key,
    protocol: p.protocol as 'responses' | 'chatCompletions',
    model: p.model, testModel: p.test_model, modelList: p.model_list, active: p.active, enabled: p.enabled,
  };
}

function toApiProfile(p: RelayProfile): RelayProfileData {
  return {
    id: p.id, provider_id: p.providerId, name: p.name, base_url: p.baseUrl, api_key: p.apiKey,
    protocol: p.protocol, model: p.model, test_model: p.testModel,
    model_list: p.modelList, active: p.active, enabled: p.enabled,
  };
}

let nextId = 1;
function genId() {
  return `prov_${Date.now().toString(36)}_${nextId++}`;
}

function createProfileFromPreset(preset: ProviderPreset): RelayProfile {
  return {
    id: genId(), providerId: preset.name.toLowerCase().replace(/[^a-z0-9]/g, '-'), name: preset.name, baseUrl: preset.baseUrl, apiKey: '',
    protocol: preset.protocol, model: preset.model, testModel: preset.model,
    modelList: (preset.modelList || []).join('\n'), active: false, enabled: true,
  };
}

export default function Providers() {
  const { data: profilesData, refetch: refetchProfiles } = useProfiles();
  const { data: proxyCfg, refetch: refetchProxyCfg } = useProxyConfig();
  const [profiles, setProfiles] = useState<RelayProfile[]>([]);
  const [proxyDraft, setProxyDraft] = useState<ProxyConfig | null>(null);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  const { t } = useI18n();
  const [enabled, setEnabled] = useState(true);
  const [presetOpen, setPresetOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState<RelayProfile | null>(null);

  useEffect(() => {
    if (profilesData) {
      if (profilesData.length > 0) {
        setProfiles(profilesData.map(fromApiProfile));
      }
    }
  }, [profilesData]);

  // Get first model from enabled providers (with prefix)
  const firstProviderModel = useMemo(() => {
    for (const p of profiles) {
      if (!p.enabled) continue;
      if (p.model) return p.providerId + '/' + p.model;
      const models = p.modelList.split(/[\n,]/).filter(Boolean);
      if (models.length > 0) return p.providerId + '/' + models[0].trim().split(/\s+/)[0];
    }
    return '';
  }, [profiles]);

  useEffect(() => {
    if (proxyCfg) {
      // Use provider model as default if config has empty or hardcoded placeholder
      const needsDefault = !proxyCfg.default_model || proxyCfg.default_model === 'deepseek-v4-flash-free';
      setProxyDraft(needsDefault && firstProviderModel
        ? { ...proxyCfg, default_model: firstProviderModel }
        : proxyCfg
      );
    }
  }, [proxyCfg, firstProviderModel]);

  // Sync draft when editing a profile
  useEffect(() => {
    if (editingId && !draft) {
      const p = profiles.find(pr => pr.id === editingId);
      if (p) setDraft(p);
    }
  }, [editingId]);

  const persistProfiles = useCallback((ps: RelayProfile[]) => {
    saveProfiles(ps.map(toApiProfile)).then(refetchProfiles).catch(() => antMsg.error(t('save_failed')));
  }, [refetchProfiles]);

  const handlePresetSelect = (preset: ProviderPreset) => {
    const np = createProfileFromPreset(preset);
    setProfiles(prev => [...prev, np]);
    setPresetOpen(false);
    setSearch('');
    setEditingId(np.id);
    setDraft(np);
  };

  const handleAddEmpty = () => {
    const np: RelayProfile = {
      id: genId(), providerId: '', name: t('new_provider'), baseUrl: '', apiKey: '',
      protocol: 'chatCompletions', model: '', testModel: '', modelList: '', active: false, enabled: true,
    };
    setProfiles(prev => [...prev, np]);
    setEditingId(np.id);
    setDraft(np);
  };

  const [fetchingModels, setFetchingModels] = useState<string | null>(null);

  const handleFetchModels = async (profile: RelayProfile) => {
    if (!profile.baseUrl) { antMsg.warning(t('fill_base_url')); return; }
    setFetchingModels(profile.id);
    try {
      const url = profile.baseUrl.replace(/\/+$/, '') + '/models';
      const models = await fetchUpstreamModels(url, profile.apiKey);
      const modelList = models.join('\n');
      setDraft(prev => prev ? { ...prev, modelList } : null);
      setProfiles(prev => prev.map(p => p.id === profile.id ? { ...p, modelList } : p));
    } catch (e: any) {
      antMsg.error(t('fetch_models_failed', { msg: e?.message || String(e) }));
    } finally {
      setFetchingModels(null);
    }
  };

  const handleToggleEnabled = (id: string, on: boolean) => {
    setProfiles(prev => {
      const next = prev.map(p => p.id === id ? { ...p, enabled: on } : p);
      persistProfiles(next);
      return next;
    });
  };

  const [testingId, setTestingId] = useState<string | null>(null);

  const handleTest = async (profile: RelayProfile) => {
    if (!profile.baseUrl) { antMsg.warning(t('fill_base_url')); return; }
    if (!profile.apiKey) { antMsg.warning(t('fill_api_key')); return; }
    setTestingId(profile.id);
    try {
      const url = (profile.baseUrl.replace(/\/+$/, '') + '/chat/completions').replace('/v1/v1/', '/v1/').replace('//chat', '/chat');
      const body = {
        model: profile.model || 'gpt-4o',
        messages: [{ role: 'user', content: 'hi' }],
        max_tokens: 10,
      };
      const res = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer ' + profile.apiKey,
        },
        body: JSON.stringify(body),
      });
      if (!res.ok) throw new Error('HTTP ' + res.status + ': ' + (await res.text()).slice(0, 100));
      const json = await res.json();
      const reply = json?.choices?.[0]?.message?.content || '(no content)';
      antMsg.success(t('test_passed', { reply: reply.slice(0, 80) }));
    } catch (e: any) {
      antMsg.error(t('test_failed', { msg: e.message }));
    } finally {
      setTestingId(null);
    }
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const fromId = String(active.id);
    const toId = String(over.id);
    setProfiles(prev => {
      const next = [...prev];
      const fromIdx = next.findIndex(p => p.id === fromId);
      const toIdx = next.findIndex(p => p.id === toId);
      if (fromIdx < 0 || toIdx < 0) return prev;
      const [moved] = next.splice(fromIdx, 1);
      next.splice(toIdx, 0, moved);
      persistProfiles(next);
      return next;
    });
  };

  const handleRemove = (id: string) => {
    setProfiles(prev => {
      const next = prev.filter(p => p.id !== id);
      if (next.length === 0) return prev; // don't remove last
      persistProfiles(next);
      return next;
    });
    if (editingId === id) { setEditingId(null); setDraft(null); }
  };

  const handleDuplicate = (id: string) => {
    const src = profiles.find(p => p.id === id);
    if (!src) return;
    setProfiles(prev => [...prev, { ...src, id: genId(), name: src.name + t('copy_suffix'), active: false }]);
  };

  const handleSaveEdit = () => {
    if (!draft) return;
    setProfiles(prev => {
      const next = prev.map(p => p.id === draft.id ? draft : p);
      persistProfiles(next);
      return next;
    });
    setEditingId(null);
    setDraft(null);
  };

  const handleCancelEdit = () => { setEditingId(null); setDraft(null); };

  const editingProfile = editingId ? (profiles.find(p => p.id === editingId) || null) : null;

  // ── List View ──
  if (!editingProfile) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
        <Card style={{ borderRadius: 12 }} className="hover-card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
            <div>
              <Text strong style={{ fontSize: 16 }}>{t('provider_list')}</Text>
              <Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>{t('provider_count', { count: profiles.length })}</Text>
            </div>
          </div>

          <div style={{
            display: 'flex', alignItems: 'center', gap: 12, padding: '12px 16px',
            background: 'var(--config-row-bg)', borderRadius: 8, marginBottom: 16,
            border: '1px solid var(--border-subtle)'
          }}>
            <Switch checked={enabled} onChange={setEnabled} size="small" />
            <div style={{ display: 'flex', flexDirection: 'column' }}>
              <Text strong style={{ fontSize: 13 }}>{t('enable_providers')}</Text>
              <Text type="secondary" style={{ fontSize: 11 }}>{t('enable_providers_desc')}</Text>
            </div>
          </div>

          <div style={{ display: 'flex', gap: 8, justifyContent: 'end' }}>
            <Button icon={<PlusOutlined />} onClick={handleAddEmpty}>{t('add_provider')}</Button>
          </div>

          {presetOpen && (
            <PresetSelector search={search} onSearch={setSearch} onSelect={handlePresetSelect} />
          )}
        </Card>

        <Card style={{ borderRadius: 12 }}>
          <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
            <SortableContext items={profiles.map(p => p.id)} strategy={verticalListSortingStrategy}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {profiles.map((profile) => (
                  <ProfileCard
                    key={profile.id}
                    profile={profile}
                    totalCount={profiles.length}
                    enabled={enabled}
                    onToggleEnabled={handleToggleEnabled}
                    onTest={handleTest}
                    testing={testingId === profile.id}
                    onEdit={() => setEditingId(profile.id)}
                    onDuplicate={handleDuplicate}
                    onRemove={handleRemove}
                  />
                ))}
              </div>
            </SortableContext>
          </DndContext>
        </Card>

        {/* Proxy Config Card */}
        <Card style={{ borderRadius: 12 }} className="hover-card">
          <Text strong style={{ fontSize: 14 }}>{t('proxy_config_title')}</Text>
          <Text type="secondary" style={{ display: 'block', fontSize: 11, marginBottom: 16 }}>
            {t('proxy_config_desc')}
          </Text>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <div>
              <Text style={{ fontSize: 12, display: 'block', marginBottom: 4, color: 'var(--text-secondary)' }}>{t('default_model')}</Text>
              <Select
                value={proxyDraft?.default_model || undefined}
                onChange={v => setProxyDraft(p => p ? { ...p, default_model: v || '' } : null)}
                placeholder={t('select_model_placeholder')}
                style={{ width: '100%' }}
                allowClear
                options={(function() {
                  const seen = new Set<string>();
                  const result: { value: string; label: string }[] = [];
                  const isTextModel = (name: string) =>
                    !/video|image|vision|tts|whisper|embed|realtime|audio|img|dalle/i.test(name);
                  for (const p of profiles) {
                    if (!p.enabled) continue;
                    const add = (v: string) => {
                      if (!v || !isTextModel(v)) return;
                      const key = p.providerId + '/' + v;
                      if (!seen.has(key)) { seen.add(key); result.push({ value: key, label: key }); }
                    };
                    // Split by newline then by space to handle messy model_list entries
                    p.modelList.split(/[\n,]/).filter(Boolean).forEach(line => {
                      line.split(/\s+/).filter(Boolean).forEach(m => add(m.trim()));
                    });
                  }
                  // Also add provider.model as first option
                  for (const p of profiles) {
                    if (p.enabled && p.model && isTextModel(p.model)) {
                      const key = p.providerId + '/' + p.model;
                      if (!seen.has(key)) { seen.add(key); result.unshift({ value: key, label: key }); }
                    }
                  }
                  return result;
                })()}
              />
            </div>
            <div>
              <Text style={{ fontSize: 12, display: 'block', marginBottom: 4, color: 'var(--text-secondary)' }}>{t('model_reasoning_effort')}</Text>
              <Select
                value={proxyDraft?.reasoning_effort || 'medium'}
                onChange={v => setProxyDraft(p => p ? { ...p, reasoning_effort: v } : null)}
                style={{ width: 200 }}
                options={[
                  { value: 'low', label: 'low' },
                  { value: 'medium', label: 'medium' },
                  { value: 'high', label: 'high' },
                ]}
              />
            </div>
            <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
              <Button
                type="primary"
                size="small"
                icon={<SaveOutlined />}
                onClick={() => {
                  if (proxyDraft) {
                    setProxyConfig(proxyDraft).then(refetchProxyCfg).catch(() => antMsg.error(t('save_failed')));
                  }
                }}
              >{t('save')}</Button>
            </div>
          </div>
        </Card>
      </div>
    );
  }

  // ── Detail/Editor View ──
  return (
    <div style={{ border: '1px solid var(--border-subtler)', borderRadius: 12, background: 'var(--bg-primary)', overflow: 'hidden' }}>
      <div style={{
        padding: '10px 16px', borderBottom: '1px solid var(--border-subtler)',
        background: 'var(--card-bg)', display: 'flex', alignItems: 'center', gap: 12,
      }}>
        <Button icon={<ArrowLeftOutlined />} onClick={handleCancelEdit}>{t('back_to_list')}</Button>
        <Button type="primary" icon={<SaveOutlined />} onClick={handleSaveEdit}>{t('save')}</Button>
      </div>

      <div style={{ padding: 16, background: 'var(--card-bg)' }}>
        <div style={{ marginBottom: 20 }}>
          <Button icon={<ApiOutlined />} onClick={() => setPresetOpen(!presetOpen)} type="dashed" block>
            {presetOpen ? t('collapse_presets') : t('preset_providers')}
          </Button>
          {presetOpen && (
            <div style={{ marginTop: 12 }}>
              <PresetSelector search={search} onSearch={setSearch} onSelect={(preset) => {
                setDraft(createProfileFromPreset(preset));
                setSearch('');
              }} />
            </div>
          )}
        </div>

        <div style={{ minWidth: 600, width: '100%', display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <Field label={t('field_provider_id')}>
              <Input
                value={draft?.providerId}
                onChange={e => {
                  const v = e.target.value.replace(/[^a-zA-Z0-9_-]/g, '');
                  setDraft(p => p ? { ...p, providerId: v } : null);
                }}
                placeholder="deepseek"
                style={{ fontFamily: 'monospace' }}
              />
            </Field>
            <Field label={t('field_name')}>
              <Input value={draft?.name} onChange={e => setDraft(p => p ? { ...p, name: e.target.value } : null)} />
            </Field>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, paddingTop: 24 }}>
              <Switch checked={draft?.enabled ?? true} size="small" onChange={v => setDraft(p => p ? { ...p, enabled: v } : null)} />
              <Text style={{ fontSize: 12 }}>{draft?.enabled ? t('enabled') : t('disabled')}</Text>
            </div>
          </div>
          <Field label={t('field_base_url')}>
            <Input value={draft?.baseUrl} onChange={e => setDraft(p => p ? { ...p, baseUrl: e.target.value } : null)}
              placeholder="https://api.openai.com/v1" />
          </Field>
          <Field label={t('field_api_key')}>
            <Input.Password value={draft?.apiKey} onChange={e => setDraft(p => p ? { ...p, apiKey: e.target.value } : null)}
              placeholder="sk-..." />
          </Field>
          <Field label={t('field_protocol')}>
            <div style={{ display: 'flex', gap: 8 }}>
              <Button
                type={draft?.protocol === 'responses' ? 'primary' : 'default'}
                onClick={() => setDraft(p => p ? { ...p, protocol: 'responses' } : null)}
                style={{ borderRadius: 6 }}
              >{t('protocol_responses')}</Button>
              <Button
                type={draft?.protocol === 'chatCompletions' ? 'primary' : 'default'}
                onClick={() => setDraft(p => p ? { ...p, protocol: 'chatCompletions' } : null)}
                style={{ borderRadius: 6 }}
              >{t('protocol_chat')}</Button>
              <Button
                type={draft?.protocol === 'anthropic' ? 'primary' : 'default'}
                onClick={() => setDraft(p => p ? { ...p, protocol: 'anthropic' } : null)}
                style={{ borderRadius: 6 }}
              >{t('protocol_anthropic')}</Button>
            </div>
          </Field>
          <Field label={t('field_model_list')}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <Button
                  icon={<DownloadOutlined />}
                  onClick={() => draft && handleFetchModels(draft)}
                  loading={fetchingModels === draft?.id}
                  size="small"
                >{t('fetch_models')}</Button>
                {draft?.modelList.trim() && (
                  <Tag>{draft.modelList.trim().split('\n').filter(Boolean).length} 个</Tag>
                )}
              </div>
              {draft?.modelList.trim() ? (
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: 6, maxHeight: 180, overflowY: 'auto', padding: '4px 0' }}>
                  {draft.modelList.trim().split('\n').map((m, i) => (
                    <div key={i} style={{
                      display: 'flex', alignItems: 'center', gap: 6, padding: '4px 8px',
                      borderRadius: 6, cursor: 'pointer', fontSize: 12, lineHeight: '22px',
                      border: '1px solid ' + (draft.model === m ? 'var(--accent-border)' : 'var(--border-subtle)'),
                      background: draft.model === m ? 'var(--accent-bg)' : 'transparent',
                      transition: 'all 0.15s',
                    }}
                      onClick={() => setDraft(p => p ? { ...p, model: m } : null)}
                      onMouseEnter={e => { if (draft.model !== m) e.currentTarget.style.borderColor = 'var(--accent-border)'; }}
                      onMouseLeave={e => { if (draft.model !== m) e.currentTarget.style.borderColor = 'var(--border-subtle)'; }}
                    >
                      <Checkbox
                        checked={draft.modelList.split('\n').includes(m)}
                        onClick={e => e.stopPropagation()}
                        onChange={(e) => {
                          const models = draft.modelList.split('\n').filter(Boolean);
                          const next = e.target.checked
                            ? [...models, m].join('\n')
                            : models.filter(x => x !== m).join('\n');
                          setDraft(p => p ? { ...p, modelList: next, model: e.target.checked && !p.model ? m : p.model } : null);
                        }}
                      />
                      <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{m}</span>
                    </div>
                  ))}
                </div>
              ) : null}
            </div>
          </Field>
          <Field label={t('default_model')}>
            <Select
              value={draft?.model || undefined}
              onChange={v => setDraft(p => p ? { ...p, model: v } : null)}
              placeholder={t('select_model_placeholder')}
              style={{ width: "100%" }}
              allowClear
              options={(draft?.modelList || "").split("\n").filter(Boolean).map(m => ({ value: m, label: m }))}
            />
          </Field>
          <Field label={t('test_model')}>
            <Select
              value={draft?.testModel || undefined}
              onChange={v => setDraft(p => p ? { ...p, testModel: v } : null)}
              placeholder={t('select_model_placeholder')}
              style={{ width: "100%" }}
              allowClear
              options={(draft?.modelList || "").split("\n").filter(Boolean).map(m => ({ value: m, label: m }))}
            />
          </Field>
        </div>
      </div>
    </div>
  );
}

// ── Sub-components ──

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
      <Text style={{ fontSize: 13, fontWeight: 500 }}>{label}</Text>
      {children}
    </div>
  );
}

function ProfileCard({
  profile, totalCount, enabled, onToggleEnabled, onTest, testing, onEdit, onDuplicate, onRemove,
}: {
  profile: RelayProfile;
  totalCount: number;
  enabled: boolean;
  onToggleEnabled: (id: string, enabled: boolean) => void;
  onTest: (profile: RelayProfile) => void;
  testing: boolean;
  onEdit: () => void;
  onDuplicate: (id: string) => void;
  onRemove: (id: string) => void;
}) {
  const { t } = useI18n();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: profile.id });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
  };
  return (
    <div
      ref={setNodeRef}
      className={'relay-profile-card' + (profile.enabled ? ' active' : '') + (isDragging ? ' dragging' : '')}
      style={style}
    >
      <button
        className="relay-drag"
        type="button"
        aria-label={t('drag_tooltip')}
        {...attributes}
        {...listeners}
      >
        <HolderOutlined style={{ fontSize: 14 }} />
      </button>
      <span className={'relay-index' + (profile.enabled ? ' active' : '')}>
        {initialFor(profile.name)}
      </span>
      <span className="relay-summary">
        <strong>
          {profile.name}
          <Tag
            style={{ marginLeft: 6, fontSize: 9, lineHeight: '16px', padding: '0 6px', verticalAlign: 'middle' }}
            color={profile.enabled ? 'success' : 'default'}
          >
            {profile.enabled ? t('enabled') : t('disabled')}
          </Tag>
          {profile.modelList.trim() && (
            <Tag style={{ marginLeft: 4, fontSize: 9, lineHeight: '16px', padding: '0 4px', verticalAlign: 'middle' }}>
              {t('models_count', { count: profile.modelList.trim().split('\n').filter(Boolean).length })}
            </Tag>
          )}
        </strong>
        <small>{protocolLabel(profile.protocol, t)} · {profile.baseUrl || t('not_set')}</small>
      </span>
      <span className="relay-card-actions" onClick={e => e.stopPropagation()}>
        <Switch
          size="small"
          checked={profile.enabled}
          onChange={(c) => onToggleEnabled(profile.id, c)}
          disabled={!enabled}
          style={{ marginRight: 8 }}
        />
        <Tooltip title={t('test_connectivity')}>
          <Button type="text" size="small" icon={<ApiOutlined />} loading={testing}
            onClick={() => onTest(profile)} />
        </Tooltip>
        <span className="relay-card-extra">
          <Tooltip title={t('edit')}>
            <Button type="text" icon={<EditOutlined />} onClick={onEdit} />
          </Tooltip>
          <Tooltip title={t('copy')}>
            <Button type="text" icon={<CopyOutlined />} onClick={() => onDuplicate(profile.id)} />
          </Tooltip>
          <Tooltip title={t('delete')}>
            <Button type="text" icon={<DeleteOutlined />} disabled={totalCount <= 1}
              onClick={() => onRemove(profile.id)} />
          </Tooltip>
        </span>
      </span>
    </div>
  );
}

function PresetSelector({
  search, onSearch, onSelect,
}: {
  search: string;
  onSearch: (s: string) => void;
  onSelect: (preset: ProviderPreset) => void;
}) {
  const { t } = useI18n();
  const filtered = useMemo(() => {
    if (!search.trim()) return PRESETS;
    const q = search.toLowerCase().trim();
    return PRESETS.filter(p =>
      p.name.toLowerCase().includes(q) ||
      p.model.toLowerCase().includes(q) ||
      p.baseUrl.toLowerCase().includes(q)
    );
  }, [search]);

  return (
    <div style={{ marginTop: 16 }}>
      <div className="preset-search" style={{
        display: 'flex', alignItems: 'center', gap: 6, padding: '6px 10px',
        border: '1px solid var(--border-subtle)', borderRadius: 8, marginBottom: 12, maxWidth: 320,
      }}>
        <SearchOutlined style={{ opacity: 0.4, fontSize: 14 }} />
        <input
          className="preset-search-input"
          placeholder={'搜索供应商…'}
          value={search}
          onChange={e => onSearch(e.target.value)}
          autoFocus
        />
      </div>

      {filtered.length === 0 && (
        <div className="preset-empty" style={{ padding: 16, textAlign: 'center', color: 'var(--text-tertiary)', fontSize: 13 }}>
          {t('no_match_search', { query: search })}
        </div>
      )}

      {search.trim() && filtered.map(preset => (
        <PresetBtn key={preset.id} preset={preset} onSelect={onSelect} />
      ))}

      {!search.trim() && CAT_ORDER.map(cat => {
        const items = PRESETS.filter(p => p.category === cat);
        if (items.length === 0) return null;
        return (
          <div className="preset-category" key={cat} style={{ marginBottom: 12 }}>
            <Text style={{ fontSize: 12, color: 'var(--text-tertiary)', fontWeight: 500, display: 'block', marginBottom: 8 }}>
              {t(CAT_LABELS[cat] || cat)}
            </Text>
            <div className="preset-category-items" style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
              {items.map(preset => (
                <PresetBtn key={preset.id} preset={preset} onSelect={onSelect} />
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}

function PresetBtn({ preset, onSelect }: { preset: ProviderPreset; onSelect: (p: ProviderPreset) => void }) {
  return (
    <div
      className="preset-btn"
      role="button"
      tabIndex={0}
      onClick={() => onSelect(preset)}
      onKeyDown={e => e.key === 'Enter' && onSelect(preset)}
      title={preset.websiteUrl ? preset.websiteUrl + '\n' + preset.baseUrl : preset.baseUrl}
    >
      <span className="preset-btn-icon">{initialFor(preset.name)}</span>
      <span className="preset-btn-name">{preset.name}</span>
      <span className="preset-btn-model">{preset.model}</span>
    </div>
  );
}
