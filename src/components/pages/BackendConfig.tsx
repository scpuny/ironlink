import { useState, useEffect } from 'react';
import { Card, Form, Input, Select, Button, Space, Typography, Tabs, Spin, message, Divider } from 'antd';
import { SaveOutlined } from '@ant-design/icons';
import { useBackend } from '../../hooks/useApi';
import { updateBackend, updateModels } from '../../api';
import type { BackendConfig as BackendConfigType, ModelEntry } from '../../types';
import { useI18n } from '../../i18n';
import ProviderSelector from '../shared/ProviderSelector';

export default function BackendConfig() {
  const { data, loading, refetch } = useBackend();
  const [form] = Form.useForm();
  const [saving, setSaving] = useState(false);
  const { t } = useI18n();

  useEffect(() => {
    if (data) form.setFieldsValue(data);
  }, [data, form]);

  const handleSave = async () => {
    const vals = await form.validateFields();
    setSaving(true);
    try {
      await updateBackend(vals);
      message.success(t('saved'));
      await refetch();
    } catch (err: any) {
      message.error(err?.message || 'Save failed');
    } finally {
      setSaving(false);
    }
  };

  const handlePreset = (partial: Partial<BackendConfigType>, modelList: string[]) => {
    form.setFieldsValue(partial);
    if (modelList.length > 0) {
      const entries: ModelEntry[] = modelList.map(id => ({
        id, object: 'model', created: Math.floor(Date.now() / 1000),
        owned_by: partial.api_base?.replace(/https?:\/\//, '').split('/')[0] || 'custom',
      }));
      updateModels(entries).catch(() => {});
    }
  };

  if (loading) return <Spin description={t('loading')} style={{ display: 'block', marginTop: 80 }} />;
  if (!data) return <Card><Typography.Text type="danger">{t('failed_to_load')}</Typography.Text></Card>;

  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <ProviderSelector onSelect={handlePreset} />

      <Card className="hover-card" style={{ borderRadius: 12 }}>
        <Typography.Title level={5} style={{ marginTop: 0, marginBottom: 20 }}>
          {t('backend_config')}
        </Typography.Title>

        <Form form={form} layout="vertical" style={{ maxWidth: 560 }}
          initialValues={{
            type: 'openai-chat', api_base: '', api_key: '',
            name: '', model: '', auth_type: 'bearer',
          }}
        >
          <Tabs
            items={[
              {
                key: 'basic',
                label: 'Basic',
                children: (
                  <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
                      <Form.Item label={<span style={{ fontSize: 13 }}>{t('name')}</span>} name="name">
                        <Input placeholder="DeepSeek" />
                      </Form.Item>
                      <Form.Item label={<span style={{ fontSize: 13 }}>{t('provider_type')}</span>} name="type">
                        <Select>
                          <Select.Option value="openai-chat">{t('chat_compatible')}</Select.Option>
                          <Select.Option value="openai-responses">{t('responses')}</Select.Option>
                          <Select.Option value="anthropic">{t('claude')}</Select.Option>
                        </Select>
                      </Form.Item>
                    </div>
                    <Form.Item label={<span style={{ fontSize: 13 }}>{t('api_base_url')}</span>} name="api_base"
                      rules={[{ required: true, message: 'Required' }]}>
                      <Input placeholder="https://api.deepseek.com/v1" />
                    </Form.Item>
                    <Form.Item label={<span style={{ fontSize: 13 }}>{t('api_key')}</span>} name="api_key">
                      <Input.Password placeholder="sk-..." />
                    </Form.Item>
                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
                      <Form.Item label={<span style={{ fontSize: 13 }}>{t('default_model')}</span>} name="model">
                        <Input placeholder="deepseek-chat" />
                      </Form.Item>
                      <Form.Item label={<span style={{ fontSize: 13 }}>{t('auth_type')}</span>} name="auth_type">
                        <Select>
                          <Select.Option value="bearer">Bearer Token</Select.Option>
                          <Select.Option value="x-api-key">x-api-key (Anthropic)</Select.Option>
                          <Select.Option value="none">None</Select.Option>
                        </Select>
                      </Form.Item>
                    </div>
                  </Space>
                ),
              },
              {
                key: 'advanced',
                label: 'Advanced',
                children: (
                  <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
                      <Form.Item label={<span style={{ fontSize: 13 }}>{t('test_model')}</span>} name="test_model">
                        <Input placeholder={t('test_model_placeholder')} />
                      </Form.Item>
                      <Form.Item label="User-Agent" name="user_agent">
                        <Input placeholder="codex-proxy/0.1.0" />
                      </Form.Item>
                    </div>
                    <Form.Item label={<span style={{ fontSize: 13 }}>{t('custom_headers')}</span>} name="custom_headers">
                      <Input.TextArea rows={3} placeholder='{"X-Custom-Header": "value"}' style={{ fontFamily: 'monospace', fontSize: 12 }} />
                    </Form.Item>
                    <Form.Item label={<span style={{ fontSize: 13 }}>{t('config_contents')}</span>} name="config_contents">
                      <Input.TextArea rows={5} placeholder={t('config_contents_placeholder')}
                        style={{ fontFamily: 'monospace', fontSize: 12 }} />
                    </Form.Item>
                  </Space>
                ),
              },
            ]}
          />

          <Divider style={{ margin: '16px 0' }} />
          <Button type="primary" icon={<SaveOutlined />} onClick={handleSave} loading={saving} shape="round">
            {t('save')}
          </Button>
        </Form>
      </Card>
    </Space>
  );
}
