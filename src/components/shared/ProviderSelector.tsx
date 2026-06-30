import { useState, useMemo } from 'react';
import { Button, Input, Collapse, Typography, Empty, Tag } from 'antd';
import { AppstoreAddOutlined, SearchOutlined } from '@ant-design/icons';
import { PRESETS, type ProviderPreset } from '../../presets';
import type { BackendConfig } from '../../types';
import { useI18n } from '../../i18n';

interface Props {
  onSelect: (config: Partial<BackendConfig>, models: string[]) => void;
}

const categoryLabels: Record<string, string> = {
  official: 'Official',
  cn_official: 'China Official',
  aggregator: 'Aggregator',
  third_party: 'Third Party',
};

function mapProtocol(preset: ProviderPreset): BackendConfig['type'] {
  return preset.protocol === 'responses' ? 'openai-responses' : 'openai-chat';
}

function badgeColor(cat: string) {
  if (cat === 'official') return 'blue';
  if (cat === 'cn_official') return 'green';
  if (cat === 'aggregator') return 'purple';
  return 'default';
}

export default function ProviderSelector({ onSelect }: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const { t } = useI18n();

  const categories = useMemo(() => [...new Set(PRESETS.map(p => p.category))], []);

  const filtered = useMemo(() => {
    if (!query.trim()) return PRESETS;
    const q = query.toLowerCase();
    return PRESETS.filter(p =>
      p.name.toLowerCase().includes(q) ||
      p.model.toLowerCase().includes(q) ||
      p.baseUrl.toLowerCase().includes(q)
    );
  }, [query]);

  const handleSelect = (preset: ProviderPreset) => {
    onSelect(
      {
        name: preset.name,
        type: mapProtocol(preset),
        api_base: preset.baseUrl.endsWith('/v1') || preset.baseUrl.endsWith('/v1/')
          ? preset.baseUrl
          : `${preset.baseUrl}/v1`,
        api_key: '',
        model: preset.model,
        test_model: preset.model,
        auth_type: preset.protocol === 'responses' ? 'bearer' : 'bearer',
      },
      preset.modelList ?? []
    );
    setOpen(false);
    setQuery('');
  };

  return (
    <div>
      <Button block icon={<AppstoreAddOutlined />} onClick={() => setOpen(o => !o)}>
        {t('create_from_preset')} ({PRESETS.length})
      </Button>

      {open && (
        <div style={{
          marginTop: 12, padding: 12, borderRadius: 6,
          border: '1px solid var(--border-subtle)', background: 'var(--config-row-bg)',
        }}>
          <Input
            prefix={<SearchOutlined />}
            placeholder={t('search_provider')}
            value={query}
            onChange={e => setQuery(e.target.value)}
            autoFocus
            style={{ marginBottom: 12 }}
          />

          {filtered.length === 0 ? (
            <Empty description={t('no_match_provider')} image={Empty.PRESENTED_IMAGE_SIMPLE} />
          ) : query.trim() ? (
            <div style={{ maxHeight: 320, overflowY: 'auto' }}>
              {filtered.map(p => (
                <Button key={p.id} type="text" block onClick={() => handleSelect(p)}
                  style={{ display: 'flex', alignItems: 'center', gap: 10, height: 'auto', padding: '8px 8px', textAlign: 'left' }}>
                  <Initial>{p.name}</Initial>
                  <div style={{ minWidth: 0 }}>
                    <div style={{ fontWeight: 500, fontSize: 13 }}>{p.name}</div>
                    <Typography.Text type="secondary" style={{ fontSize: 11 }}>{p.model}</Typography.Text>
                  </div>
                  <Tag color={badgeColor(p.category)} style={{ marginLeft: 'auto', fontSize: 10 }}>{p.baseUrl.replace(/^https?:\/\//, '').split('/')[0]}</Tag>
                </Button>
              ))}
            </div>
          ) : (
            <Collapse ghost defaultActiveKey={categories} size="small"
              items={categories.map(cat => {
                const items = PRESETS.filter(p => p.category === cat);
                return {
                  key: cat,
                  label: (
                    <Typography.Text style={{ fontSize: 11, fontWeight: 600, textTransform: 'uppercase', letterSpacing: 0.5 }}>
                      {categoryLabels[cat] || cat} ({items.length})
                    </Typography.Text>
                  ),
                  children: (
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                      {items.map(p => (
                        <button key={p.id} onClick={() => handleSelect(p)}
                          style={{
                            display: 'inline-flex', alignItems: 'center', gap: 8,
                            padding: '6px 12px', borderRadius: 6, cursor: 'pointer',
                            border: '1px solid var(--border-subtle)',
                            background: 'var(--config-row-bg)',
                            color: 'inherit', fontFamily: 'inherit', fontSize: 13,
                            transition: 'all 0.12s',
                          }}
                          onMouseEnter={e => { e.currentTarget.style.background = 'var(--config-row-hover)'; e.currentTarget.style.borderColor = 'rgba(34,197,94,0.4)'; }}
                          onMouseLeave={e => { e.currentTarget.style.background = 'var(--config-row-bg)'; e.currentTarget.style.borderColor = 'var(--border-subtle)'; }}
                        >
                          <span style={{
                            display: 'grid', placeItems: 'center', width: 22, height: 22, borderRadius: 4,
                            background: 'rgba(34,197,94,0.15)', fontSize: 10, fontWeight: 700, color: '#22c55e',
                          }}>{p.name.charAt(0)}</span>
                          <span style={{ fontWeight: 500 }}>{p.name}</span>
                          <Typography.Text type="secondary" style={{ fontSize: 11 }}>{p.model}</Typography.Text>
                        </button>
                      ))}
                    </div>
                  ),
                };
              })}
            />
          )}
        </div>
      )}
    </div>
  );
}

function Initial({ children }: { children: string }) {
  return (
    <span style={{
      display: 'grid', placeItems: 'center', width: 26, height: 26, borderRadius: 5,
      background: 'rgba(34,197,94,0.12)', fontSize: 12, fontWeight: 700, color: '#22c55e', flexShrink: 0,
    }}>{children.charAt(0)}</span>
  );
}
