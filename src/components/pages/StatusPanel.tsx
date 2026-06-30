import { useState, useMemo } from 'react';
import { Card, Button, theme, Row, Col, Tag, Typography, Space, Spin, message } from 'antd';
import { PoweroffOutlined, PlayCircleOutlined, ReloadOutlined, GlobalOutlined, ApiOutlined, LinkOutlined } from '@ant-design/icons';
import { useStatus, useProfiles } from '../../hooks/useApi';
import { toggleProxy } from '../../api';
import { useI18n } from '../../i18n';

const { Text } = Typography;

function EndpointRow({ method, path, desc }: { method: string; path: string; desc: string }) {
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 12, padding: '8px 12px', borderRadius: 8,
      background: 'var(--config-row-bg)',
    }}>
      <Tag color={method === 'GET' ? 'green' : 'blue'} style={{ margin: 0, fontSize: 10, fontFamily: 'monospace' }}>
        {method}
      </Tag>
      <code style={{ fontSize: 12, fontFamily: 'monospace', color: 'var(--accent-border)', flexShrink: 0 }}>{path}</code>
      <span style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{desc}</span>
    </div>
  );
}

export default function StatusPanel() {
  const { token } = theme.useToken();
  const { data: status, loading, refetch } = useStatus();
  const { data: rawProfiles } = useProfiles();
  const enabledProfiles = useMemo(() => {
    if (!rawProfiles) return [];
    return rawProfiles
      .map(p => ({
        id: p.id, name: p.name, baseUrl: p.base_url,
        protocol: p.protocol, models: p.model_list || [],
      }))
      .filter(p => p.models.length > 0);
  }, [rawProfiles]);
  const [toggling, setToggling] = useState(false);
  const { t } = useI18n();

  const handleToggle = async () => {
    if (!status) return;
    setToggling(true);
    try {
      await toggleProxy(!status.enabled);
      await refetch();
      if (!status.enabled) {
        message.success(t('proxy_enabled_msg'));
      } else {
        message.success(t('proxy_disabled_msg'));
      }
    } catch {
      message.error(t('operation_failed'));
    } finally {
      setToggling(false);
    }
  };

  if (loading) return <Spin description={t('loading')} style={{ display: 'block', marginTop: 80 }} />;
  if (!status) return <Card><Typography.Text type="danger">{t('failed_to_load')}</Typography.Text></Card>;

  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      {/* Status hero */}
      <Card className="hover-card" style={{ borderRadius: 12 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Space size="middle">
            <div style={{
              width: 14, height: 14, borderRadius: '50%', position: 'relative',
              background: status.enabled ? token.colorSuccess : token.colorTextTertiary,
              transition: 'all 0.3s',
            }}>
              {status.enabled && <div style={{
                position: 'absolute', inset: -3, borderRadius: '50%',
                background: token.colorSuccessBg,
              }} />}
            </div>
            <div>
              <Typography.Title level={4} style={{ margin: 0, fontSize: 18 }}>
                {status.enabled ? t('proxy_active') : t('proxy_disabled')}
              </Typography.Title>
              <Typography.Text type="secondary" style={{ fontSize: 13 }}>
                {status.enabled ? t('proxy_active_desc') : t('proxy_disabled_desc')}
              </Typography.Text>
            </div>
          </Space>
          <Space size="small">
            <Button icon={<ReloadOutlined />} onClick={refetch} size="small" type="text" />
            <Button
              type={status.enabled ? 'default' : 'primary'}
              danger={status.enabled}
              icon={status.enabled ? <PoweroffOutlined /> : <PlayCircleOutlined />}
              onClick={handleToggle}
              loading={toggling}
              shape="round"
            >
              {status.enabled ? t('disable') : t('enable')}
            </Button>
          </Space>
        </div>
      </Card>

      {/* Stats */}
      <Row gutter={16}>
        <Col span={8}>
          <Card className="hover-card" size="small" style={{ borderRadius: 10, height: '100%' }}
            styles={{ body: { display: 'flex', alignItems: 'center', gap: 12, padding: '16px 20px' } }}>
            <div style={{
              width: 36, height: 36, borderRadius: 8, display: 'flex', alignItems: 'center', justifyContent: 'center',
              background: 'var(--accent-bg)', color: 'var(--accent-border)', fontSize: 16, flexShrink: 0,
            }}>
              <GlobalOutlined />
            </div>
            <div>
              <div style={{ fontSize: 11, color: 'var(--text-secondary)', marginBottom: 2 }}>{t('port')}</div>
              <div style={{ fontSize: 22, fontFamily: 'monospace', fontWeight: 600, lineHeight: 1.2 }}>
                {status.port}
              </div>
            </div>
          </Card>
        </Col>
        <Col span={8}>
          <Card className="hover-card" size="small" style={{ borderRadius: 10, height: '100%' }}
            styles={{ body: { display: 'flex', alignItems: 'center', gap: 12, padding: '16px 20px' } }}>
            <div style={{
              width: 36, height: 36, borderRadius: 8, display: 'flex', alignItems: 'center', justifyContent: 'center',
              background: status.backend === 'anthropic' ? 'rgba(255,165,0,0.2)' : 'var(--accent-bg)',
              color: status.backend === 'anthropic' ? '#ffa500' : 'var(--accent-border)',
              fontSize: 16, flexShrink: 0,
            }}>
              <ApiOutlined />
            </div>
            <div>
              <div style={{ fontSize: 11, color: 'var(--text-secondary)', marginBottom: 2 }}>{t('backend_type')}</div>
              <Tag color={status.backend === 'anthropic' ? 'orange' : 'green'}
                style={{ margin: 0, fontSize: 12, padding: '2px 10px' }}>
                {status.backend}
              </Tag>
            </div>
          </Card>
        </Col>
        <Col span={8}>
          <Card className="hover-card" size="small" style={{ borderRadius: 10, height: '100%' }}
            styles={{ body: { display: 'flex', alignItems: 'center', gap: 12, padding: '16px 20px' } }}>
            <div style={{
              width: 36, height: 36, borderRadius: 8, display: 'flex', alignItems: 'center', justifyContent: 'center',
              background: 'var(--accent-bg)', color: 'var(--accent-border)', fontSize: 16, flexShrink: 0,
            }}>
              <LinkOutlined />
            </div>
            <div style={{ minWidth: 0 }}>
              <div style={{ fontSize: 11, color: 'var(--text-secondary)', marginBottom: 2 }}>{t('api_base')}</div>
              <Typography.Text copyable={{ text: status.api_base }} style={{
                fontSize: 12, fontFamily: 'monospace', display: 'block', lineHeight: 1.3,
                overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
              }}>
                {status.api_base || '-'}
              </Typography.Text>
            </div>
          </Card>
        </Col>
      </Row>

      {/* Enabled Providers & Models */}
      {enabledProfiles.length > 0 && (
        <Card className="hover-card" style={{ borderRadius: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 16 }}>
            <ApiOutlined style={{ fontSize: 16, color: 'var(--accent-border)' }} />
            <Text strong style={{ fontSize: 14 }}>{t('enabled_providers')}</Text>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            {enabledProfiles.map(provider => (
              <div key={provider.id} className="config-row" style={{ flexDirection: 'column', alignItems: 'stretch', gap: 8 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <Tag color="green" style={{ margin: 0 }}>{provider.name}</Tag>
                  <Tag style={{ margin: 0, fontSize: 10 }}>{provider.protocol === 'responses' ? 'Responses' : 'Chat'}</Tag>
                  <span style={{ fontSize: 11, color: 'var(--text-secondary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {provider.baseUrl}
                  </span>
                </div>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                  {provider.models.map(m => (
                    <Tag key={m} style={{ fontSize: 10, padding: '0 6px', lineHeight: '20px', margin: 0 }}>{m}</Tag>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </Card>
      )}

      {/* API Endpoints */}
      <Card className="hover-card" style={{ borderRadius: 12 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 16 }}>
          <ApiOutlined style={{ fontSize: 16, color: 'var(--accent-border)' }} />
          <Text strong style={{ fontSize: 14 }}>{t('api_endpoints')}</Text>
        </div>

        <div style={{ display: 'grid', gap: 8 }}>
          <EndpointRow
            method="GET"
            path="/v1/models"
            desc={t('endpoint_models')}
          />
          <EndpointRow
            method="POST"
            path="/v1/chat/completions"
            desc={t('endpoint_chat')}
          />
          <EndpointRow
            method="POST"
            path="/v1/responses"
            desc={t('endpoint_responses')}
          />
        </div>

        <div style={{
          marginTop: 16, padding: '10px 14px', borderRadius: 8,
          background: token.colorFillTertiary, fontSize: 12, fontFamily: 'monospace',
        }}>
          <div style={{ color: 'var(--text-secondary)', marginBottom: 4 }}>{t('codex_config_hint')}</div>
          base_url = &quot;<Typography.Text copyable style={{ fontSize: 12, fontFamily: 'monospace' }}>http://127.0.0.1:{status.port}/v1</Typography.Text>&quot;
        </div>

        <Typography.Text type="secondary" style={{ fontSize: 11, display: 'block', marginTop: 8 }}>
          {t('backup_hint')}
        </Typography.Text>
      </Card>
    </Space>
  );
}
