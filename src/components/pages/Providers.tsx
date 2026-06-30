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
import { useProfiles } from '../../hooks/useApi';
import { saveProfiles } from '../../api';
import type { RelayProfileData } from '../../api';

const { Text } = Typography;

const CAT_LABELS: Record<string, string> = {
  official: '官方',
  cn_official: '中国官方',
  aggregator: '聚合/中转',
  third_party: '第三方',
};

const CAT_ORDER = ['official', 'cn_official', 'aggregator', 'third_party'];

function protocolLabel(p: string) {
  return p === 'responses' ? 'Responses API' : 'Chat Completions';
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
  protocol: 'responses' | 'chatCompletions';
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
  const [profiles, setProfiles] = useState<RelayProfile[]>([]);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
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

  // Sync draft when editing a profile
  useEffect(() => {
    if (editingId && !draft) {
      const p = profiles.find(pr => pr.id === editingId);
      if (p) setDraft(p);
    }
  }, [editingId]);

  const persistProfiles = useCallback((ps: RelayProfile[]) => {
    saveProfiles(ps.map(toApiProfile)).then(refetchProfiles).catch(() => antMsg.error('保存失败'));
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
      id: genId(), providerId: '', name: '新供应商', baseUrl: '', apiKey: '',
      protocol: 'chatCompletions', model: '', testModel: '', modelList: '', active: false, enabled: true,
    };
    setProfiles(prev => [...prev, np]);
    setEditingId(np.id);
    setDraft(np);
  };

  const [fetchingModels, setFetchingModels] = useState<string | null>(null);

  const handleFetchModels = async (profile: RelayProfile) => {
    if (!profile.baseUrl) { antMsg.warning('请先填写 Base URL'); return; }
    setFetchingModels(profile.id);
    try {
      const url = profile.baseUrl.replace(/\/+$/, '') + '/models';
      const headers: Record<string, string> = { 'Content-Type': 'application/json' };
      if (profile.apiKey) headers['Authorization'] = 'Bearer ' + profile.apiKey;
      const res = await fetch(url, { headers });
      if (!res.ok) throw new Error('HTTP ' + res.status);
      const json = await res.json();
      const models: string[] = (json.data || json).map((m: any) => m.id || m);
      const modelList = models.join('\n');
      setDraft(prev => prev ? { ...prev, modelList } : null);
      setProfiles(prev => prev.map(p => p.id === profile.id ? { ...p, modelList } : p));
    } catch (e: any) {
      antMsg.error('获取模型列表失败: ' + e.message);
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
    if (!profile.baseUrl) { antMsg.warning('请先填写 Base URL'); return; }
    if (!profile.apiKey) { antMsg.warning('请先填写 API Key'); return; }
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
      antMsg.success('测试通过: ' + reply.slice(0, 80));
    } catch (e: any) {
      antMsg.error('测试失败: ' + e.message);
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
    setProfiles(prev => [...prev, { ...src, id: genId(), name: src.name + ' (副本)', active: false }]);
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
              <Text strong style={{ fontSize: 16 }}>供应商列表</Text>
              <Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>{profiles.length} 个供应商配置</Text>
            </div>
          </div>

          <div style={{
            display: 'flex', alignItems: 'center', gap: 12, padding: '12px 16px',
            background: 'var(--config-row-bg)', borderRadius: 8, marginBottom: 16,
            border: '1px solid var(--border-subtle)'
          }}>
            <Switch checked={enabled} onChange={setEnabled} size="small" />
            <div style={{ display: 'flex', flexDirection: 'column' }}>
              <Text strong style={{ fontSize: 13 }}>启用供应商</Text>
              <Text type="secondary" style={{ fontSize: 11 }}>关闭后不会在切换时写入 Codex 的配置文件</Text>
            </div>
          </div>

          <div style={{ display: 'flex', gap: 8, justifyContent: 'end' }}>
            <Button icon={<PlusOutlined />} onClick={handleAddEmpty}>添加供应商</Button>
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
        <Button icon={<ArrowLeftOutlined />} onClick={handleCancelEdit}>返回列表</Button>
        <Button type="primary" icon={<SaveOutlined />} onClick={handleSaveEdit}>保存</Button>
      </div>

      <div style={{ padding: 16, background: 'var(--card-bg)' }}>
        <div style={{ marginBottom: 20 }}>
          <Button icon={<ApiOutlined />} onClick={() => setPresetOpen(!presetOpen)} type="dashed" block>
            {presetOpen ? '收起预设' : '预设供应商'}
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

        <div style={{ maxWidth: 600, display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <Field label="名称">
              <Input value={draft?.name} onChange={e => setDraft(p => p ? { ...p, name: e.target.value, providerId: p.providerId || e.target.value.toLowerCase().replace(/[^a-z0-9]/g, '-') } : null)} />
            </Field>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, paddingTop: 24 }}>
              <Switch checked={draft?.enabled ?? true} size="small" onChange={v => setDraft(p => p ? { ...p, enabled: v } : null)} />
              <Text style={{ fontSize: 12 }}>{draft?.enabled ? '已启用' : '已禁用'}</Text>
            </div>
          </div>
          <Field label="Base URL">
            <Input value={draft?.baseUrl} onChange={e => setDraft(p => p ? { ...p, baseUrl: e.target.value } : null)}
              placeholder="https://api.openai.com/v1" />
          </Field>
          <Field label="API Key">
            <Input.Password value={draft?.apiKey} onChange={e => setDraft(p => p ? { ...p, apiKey: e.target.value } : null)}
              placeholder="sk-..." />
          </Field>
          <Field label="上游协议">
            <div style={{ display: 'flex', gap: 8 }}>
              <Button
                type={draft?.protocol === 'responses' ? 'primary' : 'default'}
                onClick={() => setDraft(p => p ? { ...p, protocol: 'responses' } : null)}
                style={{ borderRadius: 6 }}
              >Responses API</Button>
              <Button
                type={draft?.protocol === 'chatCompletions' ? 'primary' : 'default'}
                onClick={() => setDraft(p => p ? { ...p, protocol: 'chatCompletions' } : null)}
                style={{ borderRadius: 6 }}
              >Chat Completions</Button>
            </div>
          </Field>
          <Field label="模型列表">
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <Button
                  icon={<DownloadOutlined />}
                  onClick={() => draft && handleFetchModels(draft)}
                  loading={fetchingModels === draft?.id}
                  size="small"
                >从上游获取</Button>
                {draft?.modelList.trim() && (
                  <Tag>{draft.modelList.trim().split('\n').filter(Boolean).length} 个</Tag>
                )}
              </div>
              {draft?.modelList.trim() ? (
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, maxHeight: 180, overflowY: 'auto', padding: '4px 0' }}>
                  {draft.modelList.trim().split('\n').map((m, i) => (
                    <Tag key={i} style={{ padding: '2px 6px', fontSize: 12, lineHeight: '22px', borderRadius: 4, cursor: 'pointer' }}
                      color={draft.model === m ? 'primary' : 'default'}
                      onClick={() => setDraft(p => p ? { ...p, model: m } : null)}
                    >
                      <Checkbox
                        checked={draft.modelList.split('\n').includes(m)}
                        style={{ marginRight: 4 }}
                        onClick={e => e.stopPropagation()}
                        onChange={(e) => {
                          const models = draft.modelList.split('\n').filter(Boolean);
                          const next = e.target.checked
                            ? [...models, m].join('\n')
                            : models.filter(x => x !== m).join('\n');
                          setDraft(p => p ? { ...p, modelList: next, model: e.target.checked && !p.model ? m : p.model } : null);
                        }}
                      />
                      {m}
                    </Tag>
                  ))}
                </div>
              ) : null}
            </div>
          </Field>
          <Field label="默认模型">
            <Select
              value={draft?.model || undefined}
              onChange={v => setDraft(p => p ? { ...p, model: v } : null)}
              placeholder="从已选模型中选择"
              style={{ width: "100%" }}
              allowClear
              options={(draft?.modelList || "").split("\n").filter(Boolean).map(m => ({ value: m, label: m }))}
            />
          </Field>
          <Field label="测试模型">
            <Select
              value={draft?.testModel || undefined}
              onChange={v => setDraft(p => p ? { ...p, testModel: v } : null)}
              placeholder="从已选模型中选择"
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
        aria-label="拖动排序"
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
            {profile.enabled ? '已启用' : '已禁用'}
          </Tag>
          {profile.modelList.trim() && (
            <Tag style={{ marginLeft: 4, fontSize: 9, lineHeight: '16px', padding: '0 4px', verticalAlign: 'middle' }}>
              {profile.modelList.trim().split('\n').filter(Boolean).length} 模型
            </Tag>
          )}
        </strong>
        <small>{protocolLabel(profile.protocol)} · {profile.baseUrl || '未设置'}</small>
      </span>
      <span className="relay-card-actions" onClick={e => e.stopPropagation()}>
        <Switch
          size="small"
          checked={profile.enabled}
          onChange={(c) => onToggleEnabled(profile.id, c)}
          disabled={!enabled}
          style={{ marginRight: 8 }}
        />
        <Tooltip title="测试连通性">
            <Button type="text" size="small" icon={<ApiOutlined />} loading={testing}
              onClick={() => onTest(profile)} />
          </Tooltip>
        <span className="relay-card-extra">
          <Tooltip title="编辑">
            <Button type="text" icon={<EditOutlined />} onClick={onEdit} />
          </Tooltip>
          <Tooltip title="复制">
            <Button type="text" icon={<CopyOutlined />} onClick={() => onDuplicate(profile.id)} />
          </Tooltip>
          <Tooltip title="删除">
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
          placeholder="搜索供应商…"
          value={search}
          onChange={e => onSearch(e.target.value)}
          autoFocus
        />
      </div>

      {filtered.length === 0 && (
        <div className="preset-empty" style={{ padding: 16, textAlign: 'center', color: 'var(--text-tertiary)', fontSize: 13 }}>
          没有匹配「{search}」的供应商
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
              {CAT_LABELS[cat] || cat}
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
