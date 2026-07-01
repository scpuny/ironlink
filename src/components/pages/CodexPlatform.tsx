import { useState, useEffect } from 'react';
import { Card, Button, Typography, Space, Select, Switch, message, Divider, Tag } from 'antd';
import { SaveOutlined, CodeOutlined, ReloadOutlined } from '@ant-design/icons';
import { useProxyConfig, useStatus } from '../../hooks/useApi';
import { toggleProxy, getAutoStart, setAutoStart, fetchCodexConfigFile } from '../../api';
import { useI18n } from '../../i18n';

const { Text, Title } = Typography;

export default function CodexPlatform() {
  const { t } = useI18n();
  const { data: proxyCfg } = useProxyConfig();
  const { data: status, refetch: refetchStatus } = useStatus();
  const [defaultModel, setDefaultModel] = useState('gpt-5.5');
  const [reasoningEffort, setReasoningEffort] = useState('medium');
  const [autoStart, setAutoStartState] = useState(false);
  const [toggling, setToggling] = useState(false);
  const [codexConfig, setCodexConfig] = useState('');
  const [showRaw, setShowRaw] = useState(false);

  useEffect(() => {
    if (proxyCfg) {
      setDefaultModel(proxyCfg.default_model);
      setReasoningEffort(proxyCfg.reasoning_effort);
    }
  }, [proxyCfg]);

  useEffect(() => {
    getAutoStart().then(setAutoStartState).catch(() => {});
  }, []);

  const handleToggleProxy = async () => {
    const enable = !status?.enabled;
    setToggling(true);
    try {
      await toggleProxy(enable);
      await refetchStatus();
      message.success(enable ? t('proxy_enabled_msg') : t('proxy_disabled_msg'));
    } catch {
      message.error(t('operation_failed'));
    } finally {
      setToggling(false);
    }
  };

  const handleSave = async () => {
    try {
      const { setProxyConfig } = await import('../../api');
      await setProxyConfig({ default_model: defaultModel, reasoning_effort: reasoningEffort });
      message.success(t('saved'));
    } catch {
      message.error(t('save_failed_msg'));
    }
  };

  const handleViewConfig = async () => {
    const content = await fetchCodexConfigFile();
    setCodexConfig(content || t('codex_config_not_found'));
    setShowRaw(true);
  };

  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      {/* Proxy status */}
      <Card className="hover-card" style={{ borderRadius: 12 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Space>
            <div style={{
              width: 12, height: 12, borderRadius: '50%',
              background: status?.enabled ? '#52c41a' : '#8c8c8c',
            }} />
            <div>
              <Text strong style={{ fontSize: 15 }}>
                {status?.enabled ? t('proxy_active') : t('proxy_disabled')}
              </Text>
              <div>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {t('proxy_url')}: <code>http://127.0.0.1:{status?.port || 15723}/v1</code>
                </Text>
              </div>
            </div>
          </Space>
          <Button
            type={status?.enabled ? 'default' : 'primary'}
            danger={status?.enabled}
            onClick={handleToggleProxy}
            loading={toggling}
            shape="round"
          >
            {status?.enabled ? t('disable') : t('enable')}
          </Button>
        </div>
      </Card>

      {/* Codex config patch settings */}
      <Card className="hover-card" style={{ borderRadius: 12 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 16 }}>
          <CodeOutlined style={{ fontSize: 18, color: 'var(--accent-border)' }} />
          <Title level={5} style={{ margin: 0 }}>{t('codex_patch_config')}</Title>
        </div>

        <Space direction="vertical" size="middle" style={{ width: '100%', maxWidth: 520 }}>
          <div>
            <Text style={{ fontSize: 13, display: 'block', marginBottom: 4 }}>{t('default_model')}</Text>
            <Select
              value={defaultModel}
              onChange={setDefaultModel}
              style={{ width: '100%' }}
              options={[
                { value: 'gpt-5.5', label: 'GPT-5.5' },
                { value: 'gpt-5.4', label: 'GPT-5.4' },
                { value: 'gpt-5.4-mini', label: 'GPT-5.4 Mini' },
                { value: 'gpt-5.3-codex', label: 'GPT-5.3 Codex' },
                { value: 'gpt-5.2', label: 'GPT-5.2' },
              ]}
            />
            <Text type="secondary" style={{ fontSize: 11 }}>{t('default_model_desc')}</Text>
          </div>

          <div>
            <Text style={{ fontSize: 13, display: 'block', marginBottom: 4 }}>{t('reasoning_effort')}</Text>
            <Select
              value={reasoningEffort}
              onChange={setReasoningEffort}
              style={{ width: '100%' }}
              options={[
                { value: 'low', label: t('effort_low') },
                { value: 'medium', label: t('effort_medium') },
                { value: 'high', label: t('effort_high') },
              ]}
            />
            <Text type="secondary" style={{ fontSize: 11 }}>{t('reasoning_effort_desc')}</Text>
          </div>

          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <div>
              <Text style={{ fontSize: 13 }}>{t('auto_start')}</Text>
              <div><Text type="secondary" style={{ fontSize: 11 }}>{t('auto_start_desc')}</Text></div>
            </div>
            <Switch checked={autoStart} onChange={async (c) => {
              try {
                await setAutoStart(c);
                setAutoStartState(c);
              } catch { message.error(t('operation_failed')); }
            }} size="small" />
          </div>

          <Divider style={{ margin: '12px 0' }} />

          <div style={{ display: 'flex', gap: 8 }}>
            <Button type="primary" icon={<SaveOutlined />} onClick={handleSave} shape="round">
              {t('save_settings')}
            </Button>
            <Button icon={<CodeOutlined />} onClick={handleViewConfig} shape="round">
              {t('view_codex_config')}
            </Button>
          </div>

          <Tag style={{ fontSize: 11, padding: '2px 8px' }} color="blue">
            {t('restore_hint')}
          </Tag>
        </Space>
      </Card>

      {/* Raw Codex config display */}
      {showRaw && (
        <Card className="hover-card" style={{ borderRadius: 12 }}
          title={
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <Text strong style={{ fontSize: 13 }}>{t('codex_config_title')}</Text>
              <Button size="small" type="text" icon={<ReloadOutlined />} onClick={handleViewConfig} />
            </div>
          }>
          <pre style={{
            fontSize: 11, fontFamily: 'monospace', whiteSpace: 'pre-wrap', wordBreak: 'break-word',
            background: 'var(--config-row-bg)', padding: 12, borderRadius: 8,
            maxHeight: 400, overflow: 'auto', margin: 0,
          }}>{codexConfig}</pre>
        </Card>
      )}
    </Space>
  );
}
