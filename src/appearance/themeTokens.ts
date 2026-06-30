// AiWriter color tokens — six theme styles × dark/light
// Extracted from AiWriter's data-theme-style CSS variables

export type ThemeStyle = 'graphite' | 'aurora' | 'slate' | 'carbon' | 'nocturne' | 'amber';
export type ThemeMode = 'dark' | 'light';

export interface ThemeTokens {
  colorPrimary: string;
  colorPrimaryHover: string;
  colorPrimaryActive: string;
  colorPrimaryBg: string;
  colorBgContainer: string;
  colorBgElevated: string;
  colorBgLayout: string;
  colorBorderSecondary: string;
  colorBorder: string;
  colorText: string;
  colorTextSecondary: string;
  colorTextTertiary: string;
  colorSuccess: string;
  colorWarning: string;
  colorError: string;
  colorLink: string;
  colorLinkHover: string;
  controlItemBgActive: string;
  colorBgTextHover: string;
}

type ThemeStyleTokens = Record<ThemeMode, ThemeTokens>;

export const THEME_STYLES_META: { id: ThemeStyle; label: string; desc: string; tag: string }[] = [
  { id: 'graphite', label: '石墨', desc: '暖橙色调，纸墨感，适合通用开发', tag: '暖橙' },
  { id: 'aurora', label: '极光', desc: '紫蓝色调，沉稳专业，适合深度编码', tag: '紫蓝' },
  { id: 'slate', label: '石板', desc: '冷静理性，适合技术开发', tag: '冷蓝' },
  { id: 'carbon', label: '碳素', desc: '青绿色调，安静护眼，适合长时编码', tag: '青绿' },
  { id: 'nocturne', label: '夜曲', desc: '靛紫色调，优雅深邃，适合创意开发', tag: '靛紫' },
  { id: 'amber', label: '琥珀', desc: '暖橙复古，适合随笔/博客', tag: '琥珀' },
];

// ── Graphite (default) ──
const graphite: ThemeStyleTokens = {
  dark: {
    colorPrimary: '#ff6a3d', colorPrimaryHover: '#ff9a52', colorPrimaryActive: '#e55024',
    colorPrimaryBg: 'rgba(255,106,61,0.16)',
    colorBgContainer: '#15161a', colorBgElevated: '#191a1f', colorBgLayout: '#0c0d10',
    colorBorderSecondary: 'rgba(255,255,255,0.07)', colorBorder: 'rgba(255,255,255,0.1)',
    colorText: '#f1f1ef', colorTextSecondary: '#a7a8ad', colorTextTertiary: '#6c6e74',
    colorSuccess: '#3ad17e', colorWarning: '#e3a23a', colorError: '#f0573f',
    colorLink: '#ff6a3d', colorLinkHover: '#ff9a52',
    controlItemBgActive: 'rgba(255,106,61,0.16)', colorBgTextHover: 'rgba(255,255,255,0.06)',
  },
  light: {
    colorPrimary: '#ff5a2c', colorPrimaryHover: '#df471f', colorPrimaryActive: '#c43a14',
    colorPrimaryBg: 'rgba(255,90,44,0.12)',
    colorBgContainer: '#ffffff', colorBgElevated: '#ffffff', colorBgLayout: '#f4f3ef',
    colorBorderSecondary: 'rgba(20,18,16,0.06)', colorBorder: 'rgba(20,18,16,0.10)',
    colorText: '#1c1a17', colorTextSecondary: '#5c554b', colorTextTertiary: '#948c80',
    colorSuccess: '#1f9d57', colorWarning: '#d98a1f', colorError: '#d83a2a',
    colorLink: '#ff5a2c', colorLinkHover: '#df471f',
    controlItemBgActive: 'rgba(255,90,44,0.12)', colorBgTextHover: 'rgba(20,18,16,0.04)',
  },
};

const aurora: ThemeStyleTokens = {
  dark: {
    colorPrimary: '#8b7cff', colorPrimaryHover: '#38d6e6', colorPrimaryActive: '#6b5ce0',
    colorPrimaryBg: 'rgba(139,124,255,0.18)',
    colorBgContainer: '#17162a', colorBgElevated: '#1d1b34', colorBgLayout: '#0e0d18',
    colorBorderSecondary: 'rgba(255,255,255,0.055)', colorBorder: 'rgba(255,255,255,0.07)',
    colorText: '#ecebf7', colorTextSecondary: '#a9a4c6', colorTextTertiary: '#726c8c',
    colorSuccess: '#34d399', colorWarning: '#f0b84e', colorError: '#f0795f',
    colorLink: '#8b7cff', colorLinkHover: '#38d6e6',
    controlItemBgActive: 'rgba(139,124,255,0.18)', colorBgTextHover: 'rgba(255,255,255,0.06)',
  },
  light: {
    colorPrimary: '#7b6beb', colorPrimaryHover: '#38d6e6', colorPrimaryActive: '#5a4bc9',
    colorPrimaryBg: 'rgba(123,107,235,0.12)',
    colorBgContainer: '#ffffff', colorBgElevated: '#ffffff', colorBgLayout: '#f4f2fa',
    colorBorderSecondary: 'rgba(20,18,26,0.06)', colorBorder: 'rgba(20,18,26,0.09)',
    colorText: '#1c1a22', colorTextSecondary: '#5c5570', colorTextTertiary: '#928ba3',
    colorSuccess: '#1f9d57', colorWarning: '#d98a1f', colorError: '#d83a2a',
    colorLink: '#7b6beb', colorLinkHover: '#38d6e6',
    controlItemBgActive: 'rgba(123,107,235,0.12)', colorBgTextHover: 'rgba(20,18,26,0.04)',
  },
};

const slate: ThemeStyleTokens = {
  dark: {
    colorPrimary: '#4d8df6', colorPrimaryHover: '#6ea4f9', colorPrimaryActive: '#3a7ae0',
    colorPrimaryBg: 'rgba(77,141,246,0.16)',
    colorBgContainer: '#15181d', colorBgElevated: '#1b1f25', colorBgLayout: '#0d0f12',
    colorBorderSecondary: 'rgba(255,255,255,0.06)', colorBorder: 'rgba(255,255,255,0.08)',
    colorText: '#e7eaf0', colorTextSecondary: '#9aa2b1', colorTextTertiary: '#646c7a',
    colorSuccess: '#34d399', colorWarning: '#f0b84e', colorError: '#f0795f',
    colorLink: '#4d8df6', colorLinkHover: '#6ea4f9',
    controlItemBgActive: 'rgba(77,141,246,0.16)', colorBgTextHover: 'rgba(255,255,255,0.06)',
  },
  light: {
    colorPrimary: '#4d8df6', colorPrimaryHover: '#3a7ae0', colorPrimaryActive: '#2d68c9',
    colorPrimaryBg: 'rgba(77,141,246,0.10)',
    colorBgContainer: '#ffffff', colorBgElevated: '#ffffff', colorBgLayout: '#f5f6f9',
    colorBorderSecondary: 'rgba(30,32,38,0.06)', colorBorder: 'rgba(30,32,38,0.09)',
    colorText: '#1c1e24', colorTextSecondary: '#5c6370', colorTextTertiary: '#949aa8',
    colorSuccess: '#1f9d57', colorWarning: '#d98a1f', colorError: '#d83a2a',
    colorLink: '#4d8df6', colorLinkHover: '#3a7ae0',
    controlItemBgActive: 'rgba(77,141,246,0.10)', colorBgTextHover: 'rgba(30,32,38,0.04)',
  },
};

const carbon: ThemeStyleTokens = {
  dark: {
    colorPrimary: '#3bc8a2', colorPrimaryHover: '#5fdbb8', colorPrimaryActive: '#2aad8a',
    colorPrimaryBg: 'rgba(59,200,162,0.16)',
    colorBgContainer: '#141c1a', colorBgElevated: '#1a2422', colorBgLayout: '#0a100e',
    colorBorderSecondary: 'rgba(255,255,255,0.06)', colorBorder: 'rgba(255,255,255,0.08)',
    colorText: '#e2efe9', colorTextSecondary: '#94b0a6', colorTextTertiary: '#5c7a70',
    colorSuccess: '#34d399', colorWarning: '#e8b84e', colorError: '#e87a6a',
    colorLink: '#3bc8a2', colorLinkHover: '#5fdbb8',
    controlItemBgActive: 'rgba(59,200,162,0.16)', colorBgTextHover: 'rgba(255,255,255,0.06)',
  },
  light: {
    colorPrimary: '#2dad88', colorPrimaryHover: '#239473', colorPrimaryActive: '#1a7a5e',
    colorPrimaryBg: 'rgba(45,173,136,0.10)',
    colorBgContainer: '#ffffff', colorBgElevated: '#ffffff', colorBgLayout: '#f3f8f6',
    colorBorderSecondary: 'rgba(20,28,24,0.06)', colorBorder: 'rgba(20,28,24,0.09)',
    colorText: '#1a2420', colorTextSecondary: '#547065', colorTextTertiary: '#8ca69b',
    colorSuccess: '#1f9d57', colorWarning: '#d98a1f', colorError: '#d83a2a',
    colorLink: '#2dad88', colorLinkHover: '#239473',
    controlItemBgActive: 'rgba(45,173,136,0.10)', colorBgTextHover: 'rgba(20,28,24,0.04)',
  },
};

const nocturne: ThemeStyleTokens = {
  dark: {
    colorPrimary: '#b088e0', colorPrimaryHover: '#d0a8f8', colorPrimaryActive: '#8e66c4',
    colorPrimaryBg: 'rgba(176,136,224,0.16)',
    colorBgContainer: '#161220', colorBgElevated: '#1e1830', colorBgLayout: '#0a0812',
    colorBorderSecondary: 'rgba(255,255,255,0.06)', colorBorder: 'rgba(255,255,255,0.08)',
    colorText: '#e6def0', colorTextSecondary: '#a090bc', colorTextTertiary: '#6c5a88',
    colorSuccess: '#34d399', colorWarning: '#e8b84e', colorError: '#e87a6a',
    colorLink: '#b088e0', colorLinkHover: '#d0a8f8',
    controlItemBgActive: 'rgba(176,136,224,0.16)', colorBgTextHover: 'rgba(255,255,255,0.06)',
  },
  light: {
    colorPrimary: '#9068cc', colorPrimaryHover: '#7a4eb8', colorPrimaryActive: '#6438a0',
    colorPrimaryBg: 'rgba(144,104,204,0.10)',
    colorBgContainer: '#ffffff', colorBgElevated: '#ffffff', colorBgLayout: '#f5f0fa',
    colorBorderSecondary: 'rgba(24,18,32,0.06)', colorBorder: 'rgba(24,18,32,0.09)',
    colorText: '#1c1824', colorTextSecondary: '#5c5070', colorTextTertiary: '#9488a8',
    colorSuccess: '#1f9d57', colorWarning: '#d98a1f', colorError: '#d83a2a',
    colorLink: '#9068cc', colorLinkHover: '#7a4eb8',
    controlItemBgActive: 'rgba(144,104,204,0.10)', colorBgTextHover: 'rgba(24,18,32,0.04)',
  },
};

const amber: ThemeStyleTokens = {
  dark: {
    colorPrimary: '#e8a040', colorPrimaryHover: '#f0b860', colorPrimaryActive: '#d48c2e',
    colorPrimaryBg: 'rgba(232,160,64,0.16)',
    colorBgContainer: '#1a1610', colorBgElevated: '#221e14', colorBgLayout: '#0e0c08',
    colorBorderSecondary: 'rgba(255,255,255,0.06)', colorBorder: 'rgba(255,255,255,0.08)',
    colorText: '#ece4d8', colorTextSecondary: '#b0a490', colorTextTertiary: '#786c58',
    colorSuccess: '#74b87a', colorWarning: '#e8b84e', colorError: '#e07a5a',
    colorLink: '#e8a040', colorLinkHover: '#f0b860',
    controlItemBgActive: 'rgba(232,160,64,0.16)', colorBgTextHover: 'rgba(255,255,255,0.06)',
  },
  light: {
    colorPrimary: '#d48c2e', colorPrimaryHover: '#b87820', colorPrimaryActive: '#9c6414',
    colorPrimaryBg: 'rgba(212,140,46,0.10)',
    colorBgContainer: '#ffffff', colorBgElevated: '#ffffff', colorBgLayout: '#f6f2ea',
    colorBorderSecondary: 'rgba(24,20,14,0.06)', colorBorder: 'rgba(24,20,14,0.09)',
    colorText: '#1c1812', colorTextSecondary: '#5c5040', colorTextTertiary: '#948878',
    colorSuccess: '#1f9d57', colorWarning: '#d98a1f', colorError: '#d83a2a',
    colorLink: '#d48c2e', colorLinkHover: '#b87820',
    controlItemBgActive: 'rgba(212,140,46,0.10)', colorBgTextHover: 'rgba(24,20,14,0.04)',
  },
};

export const STYLE_TOKENS: Record<ThemeStyle, ThemeStyleTokens> = {
  graphite, aurora, slate, carbon, nocturne, amber,
};

export const DEFAULT_STYLE: ThemeStyle = 'graphite';
