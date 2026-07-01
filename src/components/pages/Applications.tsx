import { useState, useEffect } from 'react';
import { Button, Typography, Tag, message as antMsg, Switch, Card, Select } from 'antd';
import { SaveOutlined } from '@ant-design/icons';
import { useApps, useProfiles } from '../../hooks/useApi';
import { useI18n } from '../../i18n';
import { saveApps } from '../../api';
import type { AppData } from '../../api';

const { Text } = Typography;

const CODEX_MODELS = ['gpt-5.5', 'gpt-5.4', 'gpt-5.4-mini', 'gpt-5.3-codex', 'gpt-5.2'];

function protocolLabel(p: string, t: (k: string) => string) {
  return p === 'responses' ? t('protocol_responses') : p === 'anthropic' ? t('protocol_anthropic') : p === 'chatCompletions' ? t('protocol_chat') : p;
}

export default function Applications() {
  const { t } = useI18n();
  const { data: appsData, refetch: refetchApps } = useApps();
  const { data: profilesData } = useProfiles();
  const [apps, setApps] = useState<AppData[]>([]);

  useEffect(() => {
    if (appsData) setApps(appsData);
  }, [appsData]);

  const doSave = async (list: AppData[]) => {
    try {
      await saveApps(list);
      await refetchApps();
    } catch {
      antMsg.error(t('save_failed_msg'));
    }
  };

  const updateApp = (id: string, patch: Partial<AppData>) => {
    const next = apps.map(a => a.id === id ? { ...a, ...patch } : a);
    setApps(next);
    doSave(next);
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

  return (
    <div style={{ width: '100%' }}>
      <Card className="hover-card" style={{ borderRadius: 12, marginBottom: 16 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <div>
            <Typography.Title level={5} style={{ margin: 0 }}>{t('applications')}</Typography.Title>
            <Typography.Text type="secondary" style={{ fontSize: 13 }}>{t('apps_desc')}</Typography.Text>
          </div>
          <Button icon={<SaveOutlined />} onClick={() => doSave(apps)} shape="round" type="primary">
            {t('save')}
          </Button>
        </div>
      </Card>

      {apps.map(app => (
        <Card key={app.id} className="hover-card" size="small"
          style={{ borderRadius: 10, marginBottom: 8, opacity: app.enabled ? 1 : 0.6 }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            {/* App header */}
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <Text strong style={{ fontSize: 15 }}>{app.name}</Text>
                <Tag style={{ margin: 0 }}>{protocolLabel(app.protocol, t)}</Tag>
                <Tag color={app.enabled ? 'green' : 'default'} style={{ margin: 0, fontSize: 10 }}>
                  {app.enabled ? t('enabled') : t('disabled')}
                </Tag>
              </div>
              <Switch checked={app.enabled} onChange={c => updateApp(app.id, { enabled: c })} size="small" />
            </div>

            {/* Model mappings */}
            <div>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 6 }}>
                <Text strong style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{t('model_mappings')}</Text>
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                {CODEX_MODELS.map(codexModel => {
                  const mapping = app.model_mappings[codexModel];
                  const isActive = !!mapping;
                  return (
                    <div key={codexModel}
                      style={{
                        display: 'flex', gap: 6, alignItems: 'center', padding: '4px 8px',
                        borderRadius: 6, background: isActive ? 'var(--config-row-bg)' : 'transparent',
                      }}>
                      <Switch size="small" checked={isActive}
                        onChange={() => toggleMapping(app.id, codexModel)} />
                      <code style={{ width: 100, fontSize: 12, fontFamily: 'monospace' }}>{codexModel}</code>
                      {isActive && (
                        <>
                          <span style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>→</span>
                          <Select
                            size="small"
                            value={mapping.provider_id}
                            onChange={v => updateMappingTarget(app.id, codexModel, 'provider_id', v)}
                            style={{ width: 130 }}
                            options={(profilesData || [])
                              .filter(p => p.enabled)
                              .map(p => ({ value: p.provider_id, label: p.name }))}
                          />
                          <Select
                            size="small"
                            value={mapping.upstream_model}
                            onChange={v => updateMappingTarget(app.id, codexModel, 'upstream_model', v)}
                            style={{ width: 160 }}
                            options={modelsForProvider(mapping.provider_id)
                              .map(m => ({ value: m, label: m }))}
                            showSearch
                          />
                        </>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </Card>
      ))}
    </div>
  );
}
