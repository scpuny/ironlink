import { useState } from 'react';
import { Card, Typography, Space, Switch, Button, Tag, theme } from 'antd';
import { ReloadOutlined } from '@ant-design/icons';
import { useLogs } from '../../hooks/useApi';
import { useI18n } from '../../i18n';

export default function LogViewer() {
  const { token } = theme.useToken();
  const [autoRefresh, setAutoRefresh] = useState(false);
  const { data: lines, loading, refetch } = useLogs(autoRefresh);
  const { t } = useI18n();

  return (
    <Card
      className="hover-card"
      style={{
        borderRadius: 12, display: 'flex', flexDirection: 'column',
        minHeight: 'calc(100vh - 160px)',
      }}
      styles={{
        body: { flex: 1, display: 'flex', flexDirection: 'column', padding: 16 },
      }}
      title={<span style={{ fontSize: 15, fontWeight: 600 }}>{t('proxy_logs')}</span>}
      extra={
        <Space size="middle">
          <Space size={6}>
            <Typography.Text style={{ fontSize: 12, color: token.colorTextTertiary }}>
              {t('auto_refresh')}
            </Typography.Text>
            <Switch size="small" checked={autoRefresh} onChange={setAutoRefresh} />
          </Space>
          <Button size="small" icon={<ReloadOutlined />} onClick={refetch} type="text" />
        </Space>
      }
    >
      <div style={{
        flex: 1, overflowY: 'auto', padding: 12, borderRadius: 8,
        background: token.colorFillTertiary, fontFamily: '"JetBrains Mono", "Fira Code", monospace',
        fontSize: 12, lineHeight: 1.8,
      }}>
        {loading && lines.length === 0 ? (
          <Typography.Text type="secondary">{t('loading')}</Typography.Text>
        ) : lines.length === 0 ? (
          <Typography.Text type="secondary">{t('no_logs')}</Typography.Text>
        ) : (
          lines.map((line, i) => (
            <div key={i} style={{
              padding: '1px 6px', borderRadius: 4, transition: 'background 0.15s',
              whiteSpace: 'pre-wrap', wordBreak: 'break-all',
            }}
              onMouseEnter={e => e.currentTarget.style.background = token.colorBgTextHover}
              onMouseLeave={e => e.currentTarget.style.background = 'transparent'}>
              <Tag style={{ fontSize: 10, marginRight: 6, opacity: 0.5, fontFamily: 'monospace' }}>
                {String(i + 1).padStart(3, '0')}
              </Tag>
              {line}
            </div>
          ))
        )}
      </div>
    </Card>
  );
}
