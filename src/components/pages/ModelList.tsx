import { useState, useEffect } from 'react';
import { Card, Button, Input, Space, Typography, Spin, Tag, Tooltip, Popconfirm, Row, Col, theme } from 'antd';
import { PlusOutlined, SaveOutlined, DeleteOutlined, EditOutlined, CheckCircleFilled } from '@ant-design/icons';
import { useModels } from '../../hooks/useApi';
import { useI18n } from '../../i18n';
import { updateModels } from '../../api';
import type { ModelEntry } from '../../types';

export default function ModelList() {
  const { t } = useI18n();
  const { data, loading, refetch } = useModels();
  const [models, setModels] = useState<ModelEntry[]>([]);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);


  useEffect(() => {
    if (data) setModels(data);
  }, [data]);

  const handleChange = (id: string, field: keyof ModelEntry, value: string | number) => {
    setModels(prev => prev.map(m => m.id === id ? { ...m, [field]: value } : m));
  };

  const handleAdd = () => {
    const newModel: ModelEntry = {
      id: `model-${Date.now()}`,
      object: 'model',
      created: Math.floor(Date.now() / 1000),
      owned_by: 'custom',
    };
    setModels(prev => [...prev, newModel]);
    setEditingId(newModel.id);
  };

  const handleDelete = (id: string) => {
    setModels(prev => prev.filter(m => m.id !== id));
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await updateModels(models);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
      await refetch();
    } catch { /* handled */ } finally {
      setSaving(false);
    }
  };

  if (loading) return <Spin description={t('loading')} style={{ display: 'block', marginTop: 80 }} />;
  if (!data) return <Card><Typography.Text type="danger">{t('failed_to_load')}</Typography.Text></Card>;

  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      {/* Header card */}
      <Card className="hover-card" style={{ borderRadius: 12 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Space>
            <Typography.Title level={5} style={{ margin: 0 }}>{t('model_list')}</Typography.Title>
            <Tag>{models.length}</Tag>
          </Space>
          <Space>
            <Button type="primary" icon={<SaveOutlined />} onClick={handleSave} loading={saving} shape="round" size="small">
              {saving ? t('saving') : t('save')}
            </Button>
            <Button icon={<PlusOutlined />} onClick={handleAdd} size="small">
              {t('add_model')}
            </Button>
          </Space>
        </div>
        {saved && <Typography.Text type="success" style={{ fontSize: 12, marginTop: 8, display: 'block' }}>{t('saved')}</Typography.Text>}
      </Card>

      {/* Model cards — bento grid */}
      {models.length === 0 ? (
        <Card style={{ borderRadius: 12, textAlign: 'center', padding: '40px 0' }}>
          <Typography.Text type="secondary">{t('no_models')}</Typography.Text>
        </Card>
      ) : (
        <Row gutter={[12, 12]}>
          {models.map(model => (
            <Col xs={24} sm={12} lg={8} xl={6} key={model.id}>
              <ModelCard
                model={model}
                editing={editingId === model.id}
                onStartEdit={() => setEditingId(model.id)}
                onChange={(field, value) => handleChange(model.id, field, value)}
                onDelete={() => handleDelete(model.id)}
                onStopEdit={() => setEditingId(null)}
              />
            </Col>
          ))}
        </Row>
      )}
    </Space>
  );
}

function ModelCard({
  model, editing, onStartEdit, onChange, onDelete, onStopEdit,
}: {
  model: ModelEntry;
  editing: boolean;
  onStartEdit: () => void;
  onChange: (field: keyof ModelEntry, value: string | number) => void;
  onDelete: () => void;
  onStopEdit: () => void;
}) {
  const { t } = useI18n();
  const { token } = theme.useToken();
  return (
    <Card
      className="hover-card"
      size="small"
      style={{
        borderRadius: 10, height: '100%',
        border: editing ? '1px solid ' + token.colorPrimary + '66' : undefined,
      }}
      actions={[
        <Tooltip title={t('edit')} key="edit">
          {editing
            ? <Button type="link" size="small" onClick={onStopEdit}>{t('done')}</Button>
            : <Button type="text" size="small" icon={<EditOutlined />} onClick={onStartEdit} />
          }
        </Tooltip>,
        <Tooltip title={t('delete')} key="delete">
          <Popconfirm title={t('confirm_delete')} onConfirm={onDelete}>
            <Button type="text" size="small" danger icon={<DeleteOutlined />} />
          </Popconfirm>
        </Tooltip>,
      ]}
    >
      {editing ? (
        <Space orientation="vertical" size="small" style={{ width: '100%' }}>
          <div>
            <Typography.Text type="secondary" style={{ fontSize: 11 }}>{t('model_id')}</Typography.Text>
            <Input
              size="small"
              value={model.id}
              onChange={e => onChange('id', e.target.value)}
              style={{ borderRadius: 6, marginTop: 2, fontFamily: 'monospace', fontSize: 12 }}
              placeholder="model-id"
            />
          </div>
          <div>
            <Typography.Text type="secondary" style={{ fontSize: 11 }}>{t('owned_by')}</Typography.Text>
            <Input
              size="small"
              value={model.owned_by}
              onChange={e => onChange('owned_by', e.target.value)}
              style={{ borderRadius: 6, marginTop: 2, fontSize: 12 }}
              placeholder="custom"
            />
          </div>
          <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
            <Tag color="blue" style={{ fontSize: 10, margin: 0 }}>{model.object}</Tag>
            <Typography.Text style={{ fontSize: 10, color: 'var(--text-muted)' }}>
              {t('created')}: {model.created}
            </Typography.Text>
          </div>
        </Space>
      ) : (
        <Space orientation="vertical" size={2} style={{ width: '100%', minHeight: 60 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <Typography.Text code style={{ fontSize: 13, flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {model.id || '—'}
            </Typography.Text>
            {model.id && <CheckCircleFilled style={{ color: token.colorPrimary, fontSize: 12 }} />}
          </div>
          <Typography.Text type="secondary" style={{ fontSize: 12 }}>
            {model.owned_by || 'custom'}
          </Typography.Text>
          <div style={{ marginTop: 4 }}>
            <Tag color="green" style={{ fontSize: 10, lineHeight: '18px' }}>{t('active')}</Tag>
          </div>
        </Space>
      )}
    </Card>
  );
}
