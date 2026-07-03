import { useMemo } from 'react';
import { Card, Button, Tag, Typography, Space, Spin, message, Row, Col } from 'antd';
import { PoweroffOutlined, PlayCircleOutlined, ReloadOutlined, GlobalOutlined, ApiOutlined, SwapOutlined, LinkOutlined } from '@ant-design/icons';
import { useStatus, useApps, useProfiles } from '../../hooks/useApi';
import { toggleProxy } from '../../api';
import { useI18n } from '../../i18n';

const { Text } = Typography;

function protocolLabel(p: string) {
  return p === 'responses' ? 'Responses' : p === 'anthropic' ? 'Claude' : 'Chat';
}

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
  const { data: status, loading, refetch } = useStatus();
  const { data: apps } = useApps();
  const { data: profiles } = useProfiles();
  const { t } = useI18n();

  const enabledApps = useMemo(() => (apps || []).filter(a => a.enabled), [apps]);
  const enabledProviders = useMemo(() => (profiles || []).filter(p => p.enabled), [profiles]);
  const totalMappings = useMemo(() =>
    enabledApps.reduce((sum, a) => sum + Object.keys(a.model_mappings || {}).length, 0),
  [enabledApps]);

  const handleToggle = async () => {
    if (!status) return;
    try {
      const ok = await toggleProxy(!status.enabled);
      if (ok) await refetch();
    } catch { message.error(t('operation_failed')); }
  };

  if (loading) return <Spin style={{ display: 'block', marginTop: 80 }}><div style={{ padding: 40, textAlign: 'center', color: 'var(--text-tertiary)' }}>{t('loading')}</div></Spin>;
  if (!status) return <Card style={{ borderRadius: 12, marginTop: 40, textAlign: 'center', padding: 40 }}><Text type="danger">{t('failed_to_load')}</Text></Card>;

  return (
    <Space direction="vertical" size="middle" style={{ width: '100%' }}>
      {/* Hero — proxy status */}
      <div className="fluent-card" style={{
        borderRadius: 12, padding: '20px 28px',
        background: status.enabled
          ? 'linear-gradient(135deg, #1a3a2a 0%, #1e2d3d 100%)'
          : 'var(--card-bg)',
        border: status.enabled ? '1px solid #29b95533' : '1px solid var(--border-subtle)',
      }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Space size="middle">
            <div style={{
              width: 14, height: 14, borderRadius: '50%', flexShrink: 0,
              background: status.enabled ? '#29b955' : 'var(--text-tertiary)',
              boxShadow: status.enabled ? '0 0 12px #29b95588' : 'none',
              transition: 'all 0.3s',
            }} />
            <div>
              <Text strong style={{ fontSize: 18, display: 'block', color: status.enabled ? '#e8f5e9' : undefined }}>
                {status.enabled ? t('proxy_active') : t('proxy_off')}
              </Text>
              <Text style={{ fontSize: 12, color: status.enabled ? '#a5d6a7cc' : 'var(--text-tertiary)' }}>
                {status.enabled ? t('proxy_active_desc') : t('proxy_disabled_desc')}
              </Text>
            </div>
          </Space>
          <Space size="small">
            <Button icon={<ReloadOutlined />} onClick={refetch} size="small" type="text" style={{ borderRadius: 6, color: status.enabled ? '#a5d6a7' : undefined }} />
            <Button
              type={status.enabled ? 'default' : 'primary'}
              danger={status.enabled}
              icon={status.enabled ? <PoweroffOutlined /> : <PlayCircleOutlined />}
              onClick={handleToggle}
              shape="round" size="small"
              style={status.enabled ? { borderColor: '#29b95544', color: '#e8f5e9', background: 'rgba(255,255,255,0.06)' } : undefined}
            >
              {status.enabled ? t('disable') : t('enable')}
            </Button>
          </Space>
        </div>
      </div>

      {/* Stats row — equal-height cards */}
      <Row gutter={12}>
        {([
          { icon: <GlobalOutlined />, bg: 'var(--accent-bg)', color: 'var(--accent-border)', label: t('port'), value: String(status.port), sub: undefined },
          { icon: <ApiOutlined />, bg: '#0078D418', color: '#0078D4', label: t('applications'), value: `${enabledApps.length} / ${(apps || []).length}`, sub: totalMappings > 0 ? `${totalMappings} ${t('model_mappings')}` : undefined },
          { icon: <SwapOutlined />, bg: 'rgba(255,165,0,0.15)', color: '#cc7a00', label: t('providers'), value: `${enabledProviders.length} / ${(profiles || []).length}`, sub: `${enabledProviders.reduce((s, p) => s + (p.model_list?.length || 0), 0)} ${t('models')}` },
          { icon: <LinkOutlined />, bg: '#7c4dff18', color: '#7c4dff', label: t('proxy_url'), value: `http://127.0.0.1:${status.port}/v1`, sub: undefined },
        ] as const).map((card, i) => (
          <Col span={6} key={i}>
            <div className="fluent-card" style={{
              borderRadius: 10, padding: '16px 20px',
              display: 'flex', alignItems: 'center', gap: 14, height: '100%',
            }}>
              <div style={{
                width: 38, height: 38, borderRadius: 8, flexShrink: 0,
                background: card.bg, color: card.color,
                display: 'flex', alignItems: 'center', justifyContent: 'center',
              }}>
                {card.icon}
              </div>
              <div style={{ minWidth: 0 }}>
                <div style={{ fontSize: 11, color: 'var(--text-secondary)', marginBottom: 2 }}>{card.label}</div>
                <div style={{
                  fontSize: 16, fontWeight: 600, lineHeight: 1.3,
                  overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                }}>{card.value}</div>
                {card.sub && <div style={{ fontSize: 10, color: 'var(--text-tertiary)', marginTop: 1 }}>{card.sub}</div>}
              </div>
            </div>
          </Col>
        ))}
      </Row>

      {/* Connections overview — inline app + provider briefs */}
      <Row gutter={12}>
        <Col span={12}>
          <div className="fluent-card" style={{ borderRadius: 12, padding: '16px 20px', height: '100%' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 12 }}>
              <ApiOutlined style={{ fontSize: 14, color: 'var(--accent-border)' }} />
              <Text strong style={{ fontSize: 13 }}>{t('applications')}</Text>
              <Text type="secondary" style={{ fontSize: 11 }}>({enabledApps.length}/{apps?.length || 0})</Text>
            </div>
            {apps && apps.length > 0 ? (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                {apps.map(app => {
                  const mappingCount = Object.keys(app.model_mappings || {}).length;
                  const models = app.models?.length ? app.models : null;
                  return (
                    <div key={app.id} style={{
                      display: 'flex', alignItems: 'center', gap: 8, padding: '7px 10px',
                      borderRadius: 6, background: 'var(--config-row-bg)',
                    }}>
                      <div style={{
                        width: 22, height: 22, borderRadius: 5, flexShrink: 0,
                        background: app.enabled ? 'var(--accent-bg)' : 'var(--config-row-bg)',
                        color: app.enabled ? 'var(--accent-border)' : 'var(--text-tertiary)',
                        display: 'flex', alignItems: 'center', justifyContent: 'center',
                        fontSize: 10, fontWeight: 700,
                      }}>{app.name.charAt(0)}</div>
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                          <Text style={{ fontSize: 12, fontWeight: 500 }} ellipsis>{app.name}</Text>
                          <Tag style={{ margin: 0, fontSize: 9, lineHeight: '14px', padding: '0 5px', borderRadius: 3 }}>{protocolLabel(app.protocol)}</Tag>
                          {app.model_replacement_enabled && mappingCount > 0 && (
                            <Tag color="purple" style={{ margin: 0, fontSize: 9, lineHeight: '14px', padding: '0 5px', borderRadius: 3 }}>{t('model_replacement_label')}</Tag>
                          )}
                          <Tag color={app.enabled ? 'green' : 'default'} style={{ margin: 0, fontSize: 9, lineHeight: '14px', padding: '0 5px', borderRadius: 3, marginLeft: 'auto' }}>
                            {app.enabled ? t('enabled') : t('disabled')}
                          </Tag>
                        </div>
                        {app.model_replacement_enabled && mappingCount > 0 ? (
                          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 2, marginTop: 2 }}>
                            {Object.entries(app.model_mappings).slice(0, 3).map(([codex, target]) => (
                              <Tag key={codex}  variant="filled" style={{ fontSize: 8, lineHeight: '14px', backgroundColor: 'var(--accent-bg)', borderRadius: 2, margin: 0 }}>
                                {codex}<span style={{ opacity: 0.5 }}>→</span>{target.upstream_model}
                              </Tag>
                            ))}
                            {mappingCount > 3 && <Text style={{ fontSize: 9, color: 'var(--text-tertiary)', lineHeight: '18px' }}>+{mappingCount - 3}</Text>}
                          </div>
                        ) : models && models.length > 0 ? (
                          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 2, marginTop: 2 }}>
                            {models.slice(0, 3).map(m => (
                              <Tag key={m}  variant="filled"  style={{ fontSize: 8, lineHeight: '14px', borderRadius: 2, backgroundColor: 'var(--accent-bg)', margin: 0, fontFamily: 'monospace' }}>
                                {m}
                              </Tag>
                            ))}
                            {models.length > 3 && <Text style={{ fontSize: 9, color: 'var(--text-tertiary)', lineHeight: '18px' }}>+{models.length - 3}</Text>}
                          </div>
                        ) : (
                          <Text style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>{t('no_config')}</Text>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            ) : (
              <Text type="secondary" style={{ fontSize: 12 }}>{t('no_config')}</Text>
            )}
          </div>
        </Col>
        <Col span={12}>
          <div className="fluent-card" style={{ borderRadius: 12, padding: '16px 20px', height: '100%' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 12 }}>
              <SwapOutlined style={{ fontSize: 14, color: '#cc7a00' }} />
              <Text strong style={{ fontSize: 13 }}>{t('providers')}</Text>
              <Text type="secondary" style={{ fontSize: 11 }}>({enabledProviders.length}/{profiles?.length || 0})</Text>
            </div>
            {profiles && profiles.length > 0 ? (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                {profiles.map(p => {
                  const modelCount = (p.model_list || []).length;
                  return (
                    <div key={p.id} style={{
                      display: 'flex', alignItems: 'center', gap: 8, padding: '7px 10px',
                      borderRadius: 6, background: 'var(--config-row-bg)',
                    }}>
                      <div style={{
                        width: 22, height: 22, borderRadius: 5, flexShrink: 0,
                        background: p.enabled ? 'rgba(255,165,0,0.15)' : 'var(--config-row-bg)',
                        color: p.enabled ? '#cc7a00' : 'var(--text-tertiary)',
                        display: 'flex', alignItems: 'center', justifyContent: 'center',
                        fontSize: 10, fontWeight: 700,
                      }}>{p.name.charAt(0)}</div>
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <Text style={{ fontSize: 12, fontWeight: 500 }} ellipsis>{p.name}</Text>
                        <Text type="secondary" style={{ fontSize: 10, display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                          {p.base_url}
                        </Text>
                      </div>
                      <Tag style={{ margin: 0, fontSize: 9, lineHeight: '14px', padding: '0 5px', borderRadius: 3 }}>{protocolLabel(p.protocol)}</Tag>
                      <Tag color={p.enabled ? 'green' : 'default'} style={{ margin: 0, fontSize: 9, lineHeight: '14px', padding: '0 5px', borderRadius: 3 }}>
                        {modelCount} m
                      </Tag>
                    </div>
                  );
                })}
              </div>
            ) : (
              <Text type="secondary" style={{ fontSize: 12 }}>{t('no_config')}</Text>
            )}
          </div>
        </Col>
      </Row>

      {/* API Endpoints */}
      <div className="fluent-card" style={{ borderRadius: 12, padding: '16px 20px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 12 }}>
          <LinkOutlined style={{ fontSize: 14, color: 'var(--accent-border)' }} />
          <Text strong style={{ fontSize: 13 }}>{t('api_endpoints')}</Text>
        </div>
        <div style={{ display: 'grid', gap: 6 }}>
          <EndpointRow method="GET" path="/v1/models" desc={t('endpoint_models')} />
          <EndpointRow method="POST" path="/v1/chat/completions" desc={t('endpoint_chat')} />
          <EndpointRow method="POST" path="/v1/responses" desc={t('endpoint_responses')} />
        </div>
      </div>
    </Space>
  );
}
