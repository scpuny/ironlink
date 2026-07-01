import { useState, useEffect, useMemo } from 'react';
import { Card, Button, Typography, Space, Select, message, Table, Popconfirm } from 'antd';
import { SaveOutlined, ReloadOutlined, SwapOutlined, PlusOutlined, DeleteOutlined } from '@ant-design/icons';
import { useModelMappings, useProfiles } from '../../hooks/useApi';
import { saveModelMappings } from '../../api';
import type { ModelMapping } from '../../api';
import { useI18n } from '../../i18n';

const CODEX_MODEL_OPTIONS = [
  { label: 'GPT-5.5', value: 'gpt-5.5' },
  { label: 'GPT-5.4', value: 'gpt-5.4' },
  { label: 'GPT-5.4-Mini', value: 'gpt-5.4-mini' },
  { label: 'GPT-5.3-Codex', value: 'gpt-5.3-codex' },
  { label: 'GPT-5.2', value: 'gpt-5.2' },
];

let tempIdCounter = 0;
function genTempId() {
  return `__new_${++tempIdCounter}`;
}

export default function ModelMappings() {
  const { t } = useI18n();
  const { data: mappingsData, refetch: refetchMappings } = useModelMappings();
  const { data: profilesData } = useProfiles();
  const [mappings, setMappings] = useState<(ModelMapping & { _key?: string })[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (mappingsData) setMappings(mappingsData);
  }, [mappingsData]);

  const profileOptions = useMemo(() => {
    if (!profilesData) return [];
    return profilesData
      .filter(p => p.enabled)
      .map(p => ({
        label: p.name,
        value: p.id,
      }));
  }, [profilesData]);

  const modelOptionsForProfile = (profileId: string): { label: string; value: string }[] => {
    if (!profilesData) return [];
    const profile = profilesData.find(p => p.id === profileId);
    if (!profile) return [];
    const models = [...profile.model_list];
    if (profile.model && !models.includes(profile.model)) {
      models.unshift(profile.model);
    }
    return models.map(m => ({
      label: `${profile.provider_id}/${m}`,
      value: `${profile.provider_id}/${m}`,
    }));
  };

  const usedCodexModels = useMemo(
    () => new Set(mappings.filter(m => m.codex_model).map(m => m.codex_model)),
    [mappings]
  );

  const rowKey = (record: ModelMapping & { _key?: string }) => record._key || record.codex_model || 'empty';

  const updateMapping = (key: string, field: string, value: string) => {
    setMappings(prev => prev.map(m => rowKey(m) === key ? { ...m, [field]: value } : m));
  };

  const handleAdd = () => {
    const firstProfile = profileOptions[0];
    const firstUpstream = firstProfile
      ? modelOptionsForProfile(firstProfile.value)[0]?.value || ''
      : '';
    setMappings(prev => [...prev, {
      codex_model: '',
      upstream_model: firstUpstream,
      profile_id: firstProfile?.value || '',
      _key: genTempId(),
    }]);
  };

  const handleDelete = (key: string) => {
    setMappings(prev => prev.filter(m => rowKey(m) !== key));
  };

  const handleSave = async () => {
    const clean = mappings.map(({ _key, ...rest }) => rest);
    setSaving(true);
    try {
      await saveModelMappings(clean);
      message.success(t('saved'));
      refetchMappings();
    } catch {
      message.error(t('save_failed'));
    } finally {
      setSaving(false);
    }
  };

  const columns = [
    {
      title: t('codex_model_col'),
      dataIndex: 'codex_model',
      key: 'codex_model',
      width: 220,
      render: (model: string, record: ModelMapping & { _key?: string }) => (
        <Select
          value={model || undefined}
          onChange={v => updateMapping(rowKey(record), 'codex_model', v)}
          options={CODEX_MODEL_OPTIONS.filter(
            o => o.value === model || !usedCodexModels.has(o.value)
          )}
          style={{ width: '100%' }}
          placeholder={t('select_codex_model_placeholder') || 'Select Codex model'}
        />
      ),
    },
    {
      title: t('upstream_profile_col'),
      dataIndex: 'profile_id',
      key: 'profile_id',
      width: 220,
      render: (profileId: string, record: ModelMapping & { _key?: string }) => (
        <Select
          value={profileId}
          onChange={v => updateMapping(rowKey(record), 'profile_id', v)}
          options={profileOptions}
          style={{ width: '100%' }}
        />
      ),
    },
    {
      title: t('upstream_model_col'),
      dataIndex: 'upstream_model',
      key: 'upstream_model',
      render: (upstreamModel: string, record: ModelMapping & { _key?: string }) => {
        const models = modelOptionsForProfile(record.profile_id);
        return (
          <Select
            value={upstreamModel}
            onChange={v => updateMapping(rowKey(record), 'upstream_model', v)}
            options={models}
            style={{ width: '100%' }}
            showSearch
            placeholder={t('select_model_placeholder')}
          />
        );
      },
    },
    {
      title:  t('model_mapping_action'),
      key: 'actions',
      width: 60,
      render: (_: any, record: ModelMapping & { _key?: string }) => (
        <Popconfirm title={t('confirm_delete')} onConfirm={() => handleDelete(rowKey(record))}>
          <Button type="text" danger icon={<DeleteOutlined />} />
        </Popconfirm>
      ),
    },
  ];

  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Card className="hover-card" style={{ borderRadius: 12 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Space>
            <SwapOutlined style={{ fontSize: 20, color: 'var(--accent-border)' }} />
            <div>
              <Typography.Title level={5} style={{ margin: 0 }}>
                {t('model_mappings')}
              </Typography.Title>
              <Typography.Text type="secondary" style={{ fontSize: 13 }}>
                {t('model_mappings_desc')}
              </Typography.Text>
            </div>
          </Space>
          <Space>
            <Button icon={<ReloadOutlined />} onClick={refetchMappings} type="text" />
            <Button type="primary" icon={<SaveOutlined />} onClick={handleSave} loading={saving} shape="round" >
              {saving ? t('saving') : t('save')}
            </Button>
          </Space>
        </div>
      </Card>

      <Card className="hover-card" style={{ borderRadius: 12 }}>
        <div style={{ marginBottom: 12 }}>
          <Button icon={<PlusOutlined />} onClick={handleAdd} >
            {t('add_mapping') || 'Add Mapping'}
          </Button>
        </div>
        <div style={{border: '1px solid var(--border-subtle)'}}>
          <Table
            dataSource={mappings}
            columns={columns}
            rowKey={rowKey}
            pagination={false}
            locale={{ emptyText: t('no_mappings') }}
            styles={{
              header: {
                cell: {
                  textAlign: 'center'
                }
              }
            }}
          />
        </div>
      </Card>


      <Card style={{ borderRadius: 12, background: 'var(--accent-bg)', border: '1px solid var(--accent-border)' }}>
        <Typography.Text style={{ fontSize: 13 }}>
          {t('mapping_hint')}
        </Typography.Text>
      </Card>
    </Space >
  );
}
