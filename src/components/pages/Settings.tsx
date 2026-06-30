import { useState, useEffect } from 'react';
import { Card, Button, Typography, Space, Tag, Radio, Row, Col, Switch, message } from 'antd';
import {
  MoonOutlined, SunOutlined, GlobalOutlined, CheckOutlined,
  ThunderboltOutlined, DesktopOutlined,
} from '@ant-design/icons';
import { useI18n } from '../../i18n';
import { useAppearance } from '../../appearance/store';
import { getAutoStart, setAutoStart, toggleProxy } from '../../api';
import { THEME_STYLES_META } from '../../appearance/themeTokens';
import type { ThemeStyle } from '../../appearance/themeTokens';
import type { TextSize, FontChoice } from '../../appearance/store';

const { Text, Title } = Typography;

const STYLE_SWATCHES: Record<ThemeStyle, { bg: string; surface: string; accent: string }> = {
  graphite: { bg: '#0c0d10', surface: '#15161a', accent: '#ff6a3d' },
  aurora: { bg: '#0e0d18', surface: '#17162a', accent: '#8b7cff' },
  slate: { bg: '#0d0f12', surface: '#15181d', accent: '#4d8df6' },
  carbon: { bg: '#0a100e', surface: '#141c1a', accent: '#3bc8a2' },
  nocturne: { bg: '#0a0812', surface: '#161220', accent: '#b088e0' },
  amber: { bg: '#0e0c08', surface: '#1a1610', accent: '#e8a040' },
};



export default function Settings() {
  const { t, lang, setLang } = useI18n();
  const { themeStyle, setThemeStyle, textSize, setTextSize, fontFamily, setFontFamily, themeMode, setThemeMode } = useAppearance();
  const [autoStart, setAutoStartState] = useState(false);
  const [togglingAuto, setTogglingAuto] = useState(false);

  useEffect(() => {
    getAutoStart().then(setAutoStartState).catch(() => {});
  }, []);

  const handleAutoStartToggle = async (checked: boolean) => {
    setTogglingAuto(true);
    try {
      await setAutoStart(checked);
      setAutoStartState(checked);
      if (checked) {
        await toggleProxy(true);
      }
    } catch {
      message.error(t('settings_failed'));
    } finally {
      setTogglingAuto(false);
    }
  };

  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      {/* ── Appearance ── */}
      <Card className="hover-card" style={{ borderRadius: 12 }}>
        <Title level={5} style={{ marginTop: 0, marginBottom: 20 }}>{t('appearance')}</Title>

        <Space orientation="vertical" size="large" style={{ width: '100%' }}>

          {/* Theme Style */}
          <div>
            <Text strong style={{ fontSize: 14, display: 'block', marginBottom: 4 }}>{t('theme_style')}</Text>
            <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 12 }}>{t('theme_style_desc')}</Text>

            <Row gutter={[16, 16]}>
              {THEME_STYLES_META.map((meta) => {
                const swatch = STYLE_SWATCHES[meta.id];
                const selected = themeStyle === meta.id;
                return (
                  <Col key={meta.id} xs={24} sm={12} md={8} lg={8}>
                    <div
                      onClick={() => setThemeStyle(meta.id)}
                      role="button"
                      tabIndex={0}
                      onKeyDown={e => e.key === 'Enter' && setThemeStyle(meta.id)}
                      style={{
                        cursor: 'pointer', borderRadius: 12, overflow: 'hidden', height: '100%',
                        background: 'var(--card-bg)',
                        border: selected ? '2px solid ' + swatch.accent : '1px solid var(--border-subtle)',
                        boxShadow: selected ? '0 0 0 1px ' + swatch.accent + '22, var(--shadow-md)' : 'var(--shadow-sm)',
                        transition: 'all 0.2s cubic-bezier(0.2,0.72,0.2,1)',
                        transform: selected ? 'translateY(-2px)' : 'none',
                      }}
                      onMouseEnter={e => { if (!selected) { e.currentTarget.style.transform = 'translateY(-2px)'; e.currentTarget.style.boxShadow = 'var(--shadow-hover)'; }}}
                      onMouseLeave={e => { if (!selected) { e.currentTarget.style.transform = 'none'; e.currentTarget.style.boxShadow = 'var(--shadow-sm)'; }}}
                    >
                      <div style={{
                        height: 48,
                        background: 'linear-gradient(135deg, ' + swatch.bg + ', ' + swatch.surface + ')',
                        position: 'relative', overflow: 'hidden',
                      }}>
                        <div style={{
                          position: 'absolute', inset: 0,
                          background: 'linear-gradient(135deg, ' + swatch.accent + '88, transparent 60%)',
                        }} />
                        <div style={{
                          position: 'absolute', bottom: 0, left: 0, right: 0, height: 1,
                          background: swatch.accent + '44',
                        }} />
                        {selected && <div style={{
                          position: 'absolute', top: 8, right: 8, width: 22, height: 22, borderRadius: '50%',
                          background: swatch.accent + '33', display: 'flex', alignItems: 'center', justifyContent: 'center',
                        }}>
                          <CheckOutlined style={{ color: swatch.accent, fontSize: 13 }} />
                        </div>}
                      </div>
                      <div style={{ padding: '10px 14px' }}>
                        <Text strong style={{ fontSize: 13 }}>{meta.label}</Text>
                        <div>
                          <Text type="secondary" style={{ fontSize: 11 }}>{meta.desc}</Text>
                        </div>
                      </div>
                    </div>
                  </Col>
                );
              })}
            </Row>
          </div>

          {/* Theme Mode */}
          <Space orientation="vertical" style={{ width: '100%' }} size="middle">
            <div className="config-row">
              <Space>
                <GlobalOutlined style={{ fontSize: 18 }} />
                <div>
                  <Text style={{ fontSize: 14, fontWeight: 500 }}>{t('theme_mode')}</Text>
                  <div><Text type="secondary" style={{ fontSize: 12 }}>{t('theme_mode_desc')}</Text></div>
                </div>
              </Space>
              <Radio.Group
                value={themeMode}
                onChange={e => setThemeMode(e.target.value)}
                optionType="button"
                buttonStyle="solid"
              >
                <Radio.Button value="system"><DesktopOutlined /> {t('follow_system')}</Radio.Button>
                <Radio.Button value="dark"><MoonOutlined /> {t('dark_mode')}</Radio.Button>
                <Radio.Button value="light"><SunOutlined /> {t('light_mode')}</Radio.Button>
              </Radio.Group>
            </div>
          </Space>

          {/* Text Size */}
          <Space orientation="vertical" style={{ width: '100%' }} size="middle">
            <div className="config-row">
              <Space>
                <GlobalOutlined style={{ fontSize: 18 }} />
                <div>
                  <Text style={{ fontSize: 14, fontWeight: 500 }}>{t('text_size')}</Text>
                  <div><Text type="secondary" style={{ fontSize: 12 }}>{t('text_size_desc')}</Text></div>
                </div>
              </Space>
              <Radio.Group
                value={textSize}
                onChange={e => setTextSize(e.target.value)}
                optionType="button"
                buttonStyle="solid"
              >
                {(['small', 'default', 'large', 'xlarge'] as TextSize[]).map(s => {
                  const labels: Record<string, string> = { small: t('text_size_small'), default: t('text_size_default'), large: t('text_size_large'), xlarge: t('text_size_xlarge') };
                  return <Radio.Button key={s} value={s}>{labels[s]}</Radio.Button>;
                })}
              </Radio.Group>
            </div>
          </Space>

          {/* Font Family */}
          <Space orientation="vertical" style={{ width: '100%' }} size="middle">
            <div className="config-row">
              <Space>
                <GlobalOutlined style={{ fontSize: 18 }} />
                <div>
                  <Text style={{ fontSize: 14, fontWeight: 500 }}>{t('font_family')}</Text>
                  <div><Text type="secondary" style={{ fontSize: 12 }}>{t('font_family_desc')}</Text></div>
                </div>
              </Space>
              <Radio.Group
                value={fontFamily}
                onChange={e => setFontFamily(e.target.value)}
                optionType="button"
                buttonStyle="solid"
              >
                {(['system', 'yahei', 'pingfang', 'noto', 'serif'] as FontChoice[]).map(s => {
                  const labels: Record<string, string> = { system: t('font_system'), yahei: t('font_yahei'), pingfang: t('font_pingfang'), noto: t('font_noto'), serif: t('font_serif') };
                  return <Radio.Button key={s} value={s}>{labels[s]}</Radio.Button>;
                })}
              </Radio.Group>
            </div>
          </Space>
        </Space>
      </Card>

      {/* ── System Settings ── */}
      <Card className="hover-card" style={{ borderRadius: 12 }}>
        <Title level={5} style={{ marginTop: 0 }}>{t('system_settings')}</Title>
        <Space orientation="vertical" style={{ width: '100%' }} size="middle">
          <div className="config-row">
            <Space>
              <ThunderboltOutlined style={{ fontSize: 18 }} />
              <div>
                <Text style={{ fontSize: 14, fontWeight: 500 }}>{t('auto_start')}</Text>
                <div><Text type="secondary" style={{ fontSize: 12 }}>{t('auto_start_desc')}</Text></div>
              </div>
            </Space>
            <Switch checked={autoStart} onChange={handleAutoStartToggle} loading={togglingAuto} size="small" />
          </div>
          <div className="config-row">
            <Space>
              <GlobalOutlined style={{ fontSize: 18 }} />
              <div>
                <Text style={{ fontSize: 14, fontWeight: 500 }}>{t('language')}</Text>
                <div><Text type="secondary" style={{ fontSize: 12 }}>{lang === 'zh' ? '简体中文' : 'English'}</Text></div>
              </div>
            </Space>
            <Button size="small" onClick={() => setLang(lang === 'zh' ? 'en' : 'zh')} shape="round">
              {lang === 'zh' ? 'English' : '中文'}
            </Button>
          </div>
        </Space>
      </Card>


    </Space>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 12, padding: '8px 12px', borderRadius: 6,
      background: 'rgba(0,0,0,0.12)',
    }}>
      <Text type="secondary" style={{ fontSize: 13, minWidth: 80 }}>{label}</Text>
      <Text code style={{ fontSize: 12 }}>{value}</Text>
      <div style={{ flex: 1 }} />
      <Tag style={{ fontSize: 10, opacity: 0.6 }}>info</Tag>
    </div>
  );
}
