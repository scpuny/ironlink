import React, { useState, useEffect } from 'react';
import { ConfigProvider, theme, Layout, Menu, Typography, Button, Space, Modal } from 'antd';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  DashboardOutlined, ToolOutlined, SunOutlined, MoonOutlined, TranslationOutlined,
  GithubOutlined, ApiOutlined, BarsOutlined, InfoCircleOutlined,
} from '@ant-design/icons';
import { I18nProvider, useI18n } from './i18n';
import { AppearanceProvider, useAppearance } from './appearance/store';
import { STYLE_TOKENS, DEFAULT_STYLE } from './appearance/themeTokens';
import type { ThemeTokens } from './appearance/themeTokens';
import StatusPanel from './components/pages/StatusPanel';
import Applications from './components/pages/Applications';
import LogViewer from './components/pages/LogViewer';
import AboutPage from './components/pages/About';
import SettingsPanel from './components/pages/Settings';
import Providers from './components/pages/Providers';
import { useStatus } from './hooks/useApi';
import logoImg from './assets/logo.png';
const { Sider, Content, Header } = Layout;

const navItems = [
  { key: 'Status', icon: <DashboardOutlined />, labelKey: 'overview' },
  { key: 'Applications', icon: <ApiOutlined />, labelKey: 'applications' },
  { key: 'Providers', icon: <ApiOutlined />, labelKey: 'providers' },
  { key: 'Settings', icon: <ToolOutlined />, labelKey: 'settings' },
  { key: 'About', icon: <InfoCircleOutlined />, labelKey: 'about' },
];

const pages: Record<string, React.FC> = {
  Providers: Providers,
  Applications: Applications,
  About: AboutPage,
  Settings: SettingsPanel,
};

function antdTokens(t: ThemeTokens, fontCss: string, baseSize: number) {
  return {
    ...t,
    fontFamily: (fontCss ? fontCss + ' ' : '') + '"PingFang SC","Noto Sans SC","Microsoft YaHei",-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif',
    fontSize: baseSize,
    borderRadius: 8,
    borderRadiusLG: 11,
    colorTextQuaternary: '#5a5f6a',
    colorInfo: t.colorPrimary,
    colorErrorBg: `${t.colorError}1a`,
    colorSuccessBg: `${t.colorSuccess}1a`,
    colorWarningBg: `${t.colorWarning}1a`,
    boxShadow: 'var(--shadow-sm)',
    boxShadowSecondary: 'var(--shadow-lg)',
  };
}

function AppInner() {
  const [page, setPage] = useState('Status');
  const [logModalOpen, setLogModalOpen] = useState(false);
  const [closeDialogOpen, setCloseDialogOpen] = useState(false);

  // Listen for close-requested event from the backend
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen('close-requested', () => {
        setCloseDialogOpen(true);
      });
    })();
    return () => { unlisten?.(); };
  }, []);
  const { data: status, refetch: refetchStatus } = useStatus();

  // Poll status every 2s to sync header with StatusPanel
  useEffect(() => {
    const id = setInterval(refetchStatus, 2000);
    return () => clearInterval(id);
  }, []);
  const { t, lang, setLang } = useI18n();
  const { themeStyle, themeMode, setThemeMode, textSize, fontFamily } = useAppearance();
  const fontFamilyCss = ({
    system: '', yahei: '"Microsoft YaHei",', pingfang: '"PingFang SC",',
    noto: '"Noto Sans SC",', serif: 'Georgia, "Noto Serif SC",',
  } as Record<string, string>)[fontFamily];
  const baseFontSize = ({ small: 13, default: 14, large: 15, xlarge: 16 } as Record<string, number>)[textSize];

  const resolveDark = () =>
    themeMode === 'system' ? window.matchMedia('(prefers-color-scheme: dark)').matches : themeMode === 'dark';

  const resolveThemeMode = (): 'dark' | 'light' =>
    themeMode === 'system' ? (resolveDark() ? 'dark' : 'light') : themeMode;

  const toggleTheme = () => {
    // Only toggle between light and dark; "system" from Settings is resolved at call time
    setThemeMode(isDark ? 'light' : 'dark');
  };

  const PageComponent = pages[page] || StatusPanel;
  const isDark = resolveDark();
  const tokens = STYLE_TOKENS[themeStyle] || STYLE_TOKENS[DEFAULT_STYLE];
  const cur = tokens[resolveThemeMode()];

  useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty("--bg-layout", cur.colorBgLayout);
    root.style.setProperty("--text-primary", cur.colorText);
    root.style.setProperty("--text-secondary", cur.colorTextSecondary);
    root.style.setProperty("--text-tertiary", cur.colorTextTertiary);
    root.style.setProperty("--text-muted", isDark ? "rgba(255,255,255,0.22)" : "rgba(0,0,0,0.22)");
    root.style.setProperty("--card-bg", cur.colorBgContainer + "99");
    root.style.setProperty("--config-row-bg", isDark ? "rgba(255,255,255,0.04)" : "rgba(0,0,0,0.03)");
    root.style.setProperty("--config-row-hover", isDark ? "rgba(255,255,255,0.07)" : "rgba(0,0,0,0.05)");
    root.style.setProperty("--border-subtle", cur.colorBorderSecondary);
    root.style.setProperty("--border-subtler", isDark ? "rgba(255,255,255,0.04)" : "rgba(0,0,0,0.04)");
    root.style.setProperty("--shadow-sm", isDark ? "0 1px 4px rgba(0,0,0,0.20)" : "0 1px 4px rgba(0,0,0,0.08)");
    root.style.setProperty("--shadow-md", isDark ? "0 4px 20px rgba(0,0,0,0.28)" : "0 4px 20px rgba(0,0,0,0.10)");
    root.style.setProperty("--shadow-hover", isDark ? "0 4px 24px rgba(0,0,0,0.32)" : "0 4px 24px rgba(0,0,0,0.12)");
    root.style.setProperty("--bg-primary", cur.colorPrimary);
    root.style.setProperty("--accent-bg", `${cur.colorPrimary}1a`);
    root.style.setProperty("--accent-border", cur.colorPrimary);
    root.style.setProperty("--accent-hover", cur.colorPrimaryHover);
    root.style.setProperty("--font-size-base", `${baseFontSize}px`);
    root.style.setProperty("--font-family-custom", fontFamilyCss || 'inherit');
  }, [cur, isDark, baseFontSize, fontFamilyCss]);

  const menuSelectedBg = isDark ? 'rgba(255,255,255,0.10)' : 'rgba(0,0,0,0.06)';


  const sidebarBg = isDark ? tokens.dark.colorBgContainer : tokens.light.colorBgContainer;
  const sidebarBorder = isDark ? tokens.dark.colorBorderSecondary : tokens.light.colorBorderSecondary;
  const headerBg = isDark ? tokens.dark.colorBgLayout : tokens.light.colorBgContainer;
  const headerBorder = isDark ? tokens.dark.colorBorderSecondary : tokens.light.colorBorderSecondary;
  const layoutBg = isDark ? tokens.dark.colorBgLayout : tokens.light.colorBgLayout;
  const dotColor = status?.enabled ? tokens.dark.colorSuccess : tokens.dark.colorTextTertiary;


  return (
    <ConfigProvider
      theme={{
        algorithm: resolveDark() ? theme.darkAlgorithm : theme.defaultAlgorithm,
        token: antdTokens(cur, fontFamilyCss, baseFontSize),
        components: {
          Layout: { headerBg, siderBg: 'transparent', bodyBg: 'transparent', triggerBg: cur.colorBgElevated },
          Menu: { itemBg: 'transparent', itemSelectedBg: menuSelectedBg, itemHoverBg: menuSelectedBg, itemBorderRadius: 8, itemMarginBlock: 2, itemMarginInline: 4 },
          Card: { paddingLG: 20, borderRadiusLG: 12 },
          Button: { borderRadius: 8, borderRadiusLG: 10 },
          Modal: { contentBg: cur.colorBgElevated, headerBg: cur.colorBgElevated },
          Select: { optionSelectedBg: menuSelectedBg },
        },
      }}
    >
      <Layout style={{ height: '100vh', background: layoutBg }}>
        <Sider width={180} style={{ borderRight: `1px solid ${sidebarBorder}`,
          background: sidebarBg,
          display: 'flex', flexDirection: 'column', }}>
          <div style={{ padding: '14px 14px 10px', display: 'flex', alignItems: 'center', gap: 8 }}>
            <img src={logoImg} alt="" style={{ width: 26, height: 26, borderRadius: 7 }} />
            <span style={{ fontWeight: 700, fontSize: 15, letterSpacing: '-0.3px' }}>IronLink</span>
          </div>

          <Menu
            mode="inline"
            selectedKeys={[page]}
            onClick={({ key }) => setPage(key)}
            items={navItems.map(i => ({ key: i.key, icon: i.icon, label: t(i.labelKey) }))}
            style={{ borderInlineEnd: 'none', marginTop: 6, flex: 1, background: 'transparent' }}
          />

          <div style={{ width: '100%', padding: '10px 12px 14px', borderTop: `1px solid ${isDark ? tokens.dark.colorBorderSecondary : tokens.light.colorBorderSecondary}`, position: 'absolute', bottom: 0 }}>
            <Space style={{ width: '100%', justifyContent: 'center' }} size={4}>
              <Button type="text" size="small"
                onClick={() => setLang(lang === 'zh' ? 'en' : 'zh')}
                icon={<TranslationOutlined />}
                style={{ color: isDark ? tokens.dark.colorTextTertiary : tokens.light.colorTextTertiary, fontSize: 14 }}
              />
              <Button type="text" size="small"
                onClick={toggleTheme}
                icon={isDark ? <SunOutlined /> : <MoonOutlined />}
                style={{ color: isDark ? tokens.dark.colorTextTertiary : tokens.light.colorTextTertiary, fontSize: 14 }}
              />
              <Button type="text" size="small" icon={<GithubOutlined />}
                style={{ color: isDark ? tokens.dark.colorTextTertiary : tokens.light.colorTextTertiary, fontSize: 14 }}
              />
            </Space>
          </div>
        </Sider>

        <Layout style={{ background: layoutBg }}>
          <Header style={{
            display: 'flex', alignItems: 'center', gap: 12,
            height: 46, lineHeight: '46px', padding: '0 20px',
            background: headerBg, borderBottom: `1px solid ${headerBorder}`,
          }}>
            <div style={{
              width: 8, height: 8, borderRadius: '50%',
              background: dotColor,
              boxShadow: status?.enabled ? `0 0 6px ${tokens.dark.colorSuccess}66` : 'none',
              transition: 'all 0.3s',
            }} />
            <Typography.Text style={{ fontSize: 13, fontWeight: 500, color: isDark ? tokens.dark.colorText : tokens.light.colorText }}>
              {status?.enabled ? t('proxy_active') : t('proxy_off')}
            </Typography.Text>
            <Typography.Text style={{ fontSize: 11, color: isDark ? tokens.dark.colorTextTertiary : tokens.light.colorTextTertiary }}>·</Typography.Text>
            <Typography.Text style={{ fontSize: 12, color: isDark ? tokens.dark.colorTextSecondary : tokens.light.colorTextSecondary }}>
              {status?.backend || '-'}
            </Typography.Text>
            {status?.api_base && (
              <>
                <Typography.Text style={{ fontSize: 11, color: isDark ? tokens.dark.colorTextTertiary : tokens.light.colorTextTertiary }}>·</Typography.Text>
                <Typography.Text style={{ fontSize: 12, color: isDark ? tokens.dark.colorTextTertiary : tokens.light.colorTextTertiary, maxWidth: 320, fontFamily: '"SF Mono", Consolas, "Noto Sans Mono", monospace' }} ellipsis>
                  {status.api_base}
                </Typography.Text>
              </>
            )}
            <div style={{ flex: 1 }} />
            <Button type="text" size="small"
              disabled={!status?.enabled}
              onClick={() => setLogModalOpen(true)}
              icon={<BarsOutlined />}
              style={{ color: isDark ? tokens.dark.colorTextTertiary : tokens.light.colorTextTertiary, fontSize: 13 }}
            >
              {t('logs')}
            </Button>
            <Button type="text" size="small"
              onClick={() => setLang(lang === 'zh' ? 'en' : 'zh')}
              icon={<TranslationOutlined />}
              style={{ color: isDark ? tokens.dark.colorTextTertiary : tokens.light.colorTextTertiary, fontSize: 13 }}
            >
              {lang === 'zh' ? 'EN' : '中'}
            </Button>
            <Button type="text" size="small"
              onClick={toggleTheme}
              icon={isDark ? <SunOutlined /> : <MoonOutlined />}
              style={{ color: isDark ? tokens.dark.colorTextTertiary : tokens.light.colorTextTertiary, fontSize: 13 }}
            />
          </Header>
          <Modal
            title={t('close_dialog_title')}
            open={closeDialogOpen}
            onCancel={() => setCloseDialogOpen(false)}
            footer={[
              <Button key="hide" onClick={async () => {
                setCloseDialogOpen(false);
                await invoke('hide_window');
              }}>{t('close_hide_tray')}</Button>,
              <Button key="quit" type="primary" danger onClick={async () => {
                setCloseDialogOpen(false);
                await invoke('quit_app');
              }}>{t('close_quit')}</Button>,
            ]}
            width={400}
          >
            <Typography.Text>{t('close_dialog_message')}</Typography.Text>
          </Modal>
          <Modal
            open={logModalOpen}
            onCancel={() => setLogModalOpen(false)}
            footer={null}
            width={800}
            styles={{ body: { padding: 0 } }}
          >
            <div style={{ maxHeight: '60vh', overflow: 'hidden' }}>
              <LogViewer />
            </div>
          </Modal>

          <Content style={{ padding: 24, overflowY: 'auto' }}>
            <div style={{ maxWidth: 1400, margin: '0 auto', width: '100%' }}>
              <PageComponent />
            </div>
          </Content>
        </Layout>
      </Layout>
    </ConfigProvider>
  );
}

export default function App() {
  return (
    <I18nProvider>
      <AppearanceProvider>
      <AppInner />
    </AppearanceProvider>
      </I18nProvider>
  );
}
