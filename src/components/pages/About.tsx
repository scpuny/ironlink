import { useState, useEffect } from 'react';
import { Typography, Space, Tag, Card, Button, Divider, Row, Col, message } from 'antd';
import {
  GithubOutlined,
  AppleOutlined, WindowsOutlined, LinuxOutlined,
  DownloadOutlined, ReloadOutlined,
} from '@ant-design/icons';
import { useI18n } from '../../i18n';
import { checkVersion } from '../../api';
import type { VersionInfo } from '../../api';
import logoImg from '../../assets/logo.png';

const { Text, Title } = Typography;

const platforms = [
  { icon: <AppleOutlined />, label: 'macOS', status: 'ready' },
  { icon: <WindowsOutlined />, label: 'Windows', status: 'coming' },
  { icon: <LinuxOutlined />, label: 'Linux', status: 'coming' },
] as const;

export default function About() {
  const { t } = useI18n();
  const [version, setVersion] = useState<VersionInfo | null>(null);
  const [checking, setChecking] = useState(false);

  const doCheck = async () => {
    setChecking(true);
    try {
      const info = await checkVersion();
      setVersion(info);
      if (info.has_update) {
        message.info(t('about_update_available', { v: info.latest_version }));
      } else {
        message.success(t('about_up_to_date'));
      }
    } catch {
      message.error(t('about_check_failed'));
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => { doCheck(); }, []);

  return (
    <div style={{ maxWidth: 720, margin: '0 auto', padding: '32px 16px' }}>
      {/* Hero */}
      <Card
        style={{ borderRadius: 16, textAlign: 'center', padding: '8px 0', background: 'transparent', border: 'none' }}
        styles={{ body: { padding: '32px 24px' } }}
      >
        <div style={{
          width: 88, height: 88, borderRadius: 20, margin: '0 auto 20px',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          background: 'linear-gradient(135deg, #6c5ce7 0%, #a29bfe 100%)',
          boxShadow: '0 8px 32px rgba(108,92,231,0.3)',
        }}>
          <img src={logoImg} alt="IronLink" style={{ width: 56, height: 56, objectFit: 'contain', borderRadius: 12 }} />
        </div>
        <Title level={2} style={{ margin: 0, fontSize: 28, fontWeight: 700, letterSpacing: '-0.3px' }}>
          IronLink
        </Title>
        <Text type="secondary" style={{ fontSize: 14, display: 'block', marginTop: 4 }}>
          {t('about_subtitle')}
        </Text>
        <div style={{ marginTop: 12 }}>
          {version ? (
            <>
              <Tag color="purple" style={{ borderRadius: 6, fontSize: 12, padding: '2px 10px' }}>
                v{version.current_version}
              </Tag>
              {version.has_update && (
                <Tag color="green" style={{ borderRadius: 6, fontSize: 12, padding: '2px 10px', marginLeft: 8 }}>
                  {t('about_new_version')} v{version.latest_version}
                </Tag>
              )}
            </>
          ) : (
            <Tag style={{ borderRadius: 6, fontSize: 12, padding: '2px 10px' }}>
              {t('loading')}
            </Tag>
          )}
          <Tag style={{ borderRadius: 6, fontSize: 12, padding: '2px 10px', marginLeft: 8 }}>
            Tauri + React + Rust
          </Tag>
        </div>
      </Card>

      {/* Description */}
      <Card style={{ borderRadius: 12, marginTop: 8, border: '1px solid var(--border-subtle)' }}
        styles={{ body: { padding: '20px 24px' } }}>
        <Text style={{ fontSize: 14, lineHeight: 1.8, color: 'var(--text-secondary)' }}>
          {t('about_description')}
        </Text>
      </Card>

      {/* Features */}
      <Row gutter={[12, 12]} style={{ marginTop: 12 }}>
        {[
          { title: t('about_feature_1_title'), desc: t('about_feature_1_desc') },
          { title: t('about_feature_2_title'), desc: t('about_feature_2_desc') },
          { title: t('about_feature_3_title'), desc: t('about_feature_3_desc') },
        ].map((f, i) => (
          <Col span={8} key={i}>
            <Card hoverable style={{ borderRadius: 12, height: '100%', border: '1px solid var(--border-subtle)' }}
              styles={{ body: { padding: '16px 20px' } }}>
              <Text strong style={{ fontSize: 13 }}>{f.title}</Text>
              <div style={{ marginTop: 6 }}>
                <Text style={{ fontSize: 12, color: 'var(--text-tertiary)', lineHeight: 1.6 }}>{f.desc}</Text>
              </div>
            </Card>
          </Col>
        ))}
      </Row>

      {/* Platform */}
      <Card title={<Text strong style={{ fontSize: 14 }}>{t('about_platforms')}</Text>}
        style={{ borderRadius: 12, marginTop: 12, border: '1px solid var(--border-subtle)' }}
        styles={{ body: { padding: '16px 24px' } }}>
        <Space size={16} wrap>
          {platforms.map(p => (
            <Space key={p.label} size={8}>
              <span style={{ fontSize: 18, opacity: p.status === 'ready' ? 1 : 0.35 }}>{p.icon}</span>
              <Text style={{ fontSize: 13, opacity: p.status === 'ready' ? 1 : 0.45 }}>{p.label}</Text>
              {p.status === 'ready' && (
                <Tag color="green" style={{ borderRadius: 4, fontSize: 10, lineHeight: '16px', padding: '0 6px' }}>
                  {t('about_ready')}
                </Tag>
              )}
              {p.status === 'coming' && (
                <Tag style={{ borderRadius: 4, fontSize: 10, lineHeight: '16px', padding: '0 6px', opacity: 0.5 }}>
                  {t('about_coming_soon')}
                </Tag>
              )}
            </Space>
          ))}
        </Space>
      </Card>

      {/* Update */}
      <Card style={{ borderRadius: 12, marginTop: 12, border: '1px solid var(--border-subtle)' }}
        styles={{ body: { padding: '20px 24px' } }}>
        <Space style={{ width: '100%', justifyContent: 'space-between' }} align="center">
          <div>
            <Text strong style={{ fontSize: 14 }}>{t('about_update_title')}</Text>
            <div style={{ marginTop: 4 }}>
              <Text style={{ fontSize: 12, color: 'var(--text-tertiary)' }}>
                {version?.has_update
                  ? t('about_update_available', { v: version.latest_version })
                  : t('about_update_desc')}
              </Text>
            </div>
          </div>
          <Space>
            <Button icon={<ReloadOutlined />} onClick={doCheck} loading={checking} style={{ borderRadius: 8 }}>
              {t('about_check_update')}
            </Button>
            <Button type="primary" icon={<DownloadOutlined />}
              href={version?.release_url || 'https://github.com/scpuny/ironlink/releases'}
              target="_blank" style={{ borderRadius: 8 }}>
              {t('about_download')}
            </Button>
          </Space>
        </Space>
      </Card>

      <Divider style={{ margin: '20px 0 12px', opacity: 0.3 }} />

      {/* Footer */}
      <div style={{ textAlign: 'center' }}>
        <Space size={16}>
          <Button type="link" size="small" icon={<GithubOutlined />}
            href="https://github.com/scpuny/ironlink" target="_blank"
            style={{ fontSize: 12, opacity: 0.6 }}>
            GitHub
          </Button>
        </Space>
        <div style={{ marginTop: 8 }}>
          <Text style={{ fontSize: 11, color: 'var(--text-muted)' }}>
            © 2026 IronLink. {t('about_license')}
          </Text>
        </div>
      </div>
    </div>
  );
}
