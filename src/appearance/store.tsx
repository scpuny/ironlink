import React, { createContext, useContext, useState, useCallback, useEffect } from 'react';
import type { ThemeStyle } from './themeTokens';
import { DEFAULT_STYLE } from './themeTokens';

const STYLE_KEY = 'codex-proxy-theme-style';
const TEXT_SIZE_KEY = 'codex-proxy-text-size';
const FONT_KEY = 'codex-proxy-font';
const THEME_KEY = 'codex-proxy-theme';

export type TextSize = 'small' | 'default' | 'large' | 'xlarge';
export type FontChoice = 'system' | 'yahei' | 'pingfang' | 'noto' | 'serif';
export type ThemeMode = 'dark' | 'light' | 'system';

export interface AppearanceCtx {
  themeStyle: ThemeStyle;
  setThemeStyle: (s: ThemeStyle) => void;
  textSize: TextSize;
  setTextSize: (s: TextSize) => void;
  fontFamily: FontChoice;
  setFontFamily: (f: FontChoice) => void;
  themeMode: ThemeMode;
  setThemeMode: (m: ThemeMode) => void;
}

const AppearanceContext = createContext<AppearanceCtx>({
  themeStyle: DEFAULT_STYLE,
  setThemeStyle: () => {},
  textSize: 'default',
  setTextSize: () => {},
  fontFamily: 'system',
  setFontFamily: () => {},
  themeMode: 'dark',
  setThemeMode: () => {},
});


export function AppearanceProvider({ children }: { children: React.ReactNode }) {
  const [themeStyle, setThemeStyleRaw] = useState<ThemeStyle>(
    () => (localStorage.getItem(STYLE_KEY) as ThemeStyle) || DEFAULT_STYLE
  );
  const [textSize, setTextSizeRaw] = useState<TextSize>(
    () => (localStorage.getItem(TEXT_SIZE_KEY) as TextSize) || 'default'
  );
  const [fontFamily, setFontFamilyRaw] = useState<FontChoice>(
    () => (localStorage.getItem(FONT_KEY) as FontChoice) || 'system'
  );
  const [themeMode, setThemeModeRaw] = useState<ThemeMode>(
    () => (localStorage.getItem(THEME_KEY) as ThemeMode) || 'dark'
  );

  useEffect(() => {
    document.documentElement.setAttribute('data-theme-style', themeStyle);
    localStorage.setItem(STYLE_KEY, themeStyle);
  }, [themeStyle]);



  useEffect(() => {
    document.documentElement.setAttribute('data-text-size', textSize);
    localStorage.setItem(TEXT_SIZE_KEY, textSize);
    const scales = { small: 0.9, default: 1, large: 1.1, xlarge: 1.2 };
    document.documentElement.style.setProperty('--font-scale', String(scales[textSize]));
  }, [textSize]);

  useEffect(() => {
    
    document.documentElement.setAttribute('data-font-family', fontFamily);
  }, [fontFamily]);

  const [systemDark, setSystemDark] = useState(() =>
    window.matchMedia('(prefers-color-scheme: dark)').matches
  );

  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, []);

  useEffect(() => {
    const isDark = themeMode === 'system' ? systemDark : themeMode === 'dark';
    document.documentElement.className = isDark ? '' : 'light';
    localStorage.setItem(THEME_KEY, themeMode);
  }, [themeMode, systemDark]);

  const setThemeStyle = useCallback((s: ThemeStyle) => setThemeStyleRaw(s), []);
  const setTextSize = useCallback((s: TextSize) => setTextSizeRaw(s), []);
  const setFontFamily = useCallback((f: FontChoice) => setFontFamilyRaw(f), []);
  const setThemeMode = useCallback((m: ThemeMode) => setThemeModeRaw(m), []);

  return (
    <AppearanceContext.Provider value={{ themeStyle, setThemeStyle, textSize, setTextSize, fontFamily, setFontFamily, themeMode, setThemeMode }}>
      {children}
    </AppearanceContext.Provider>
  );
}

export function useAppearance() {
  return useContext(AppearanceContext);
}

export type AppearanceState = AppearanceCtx;
