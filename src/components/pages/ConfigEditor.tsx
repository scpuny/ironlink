import { useState, useEffect, useRef } from 'react';
import { Card, Spin, Tabs } from 'antd';
import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { json } from '@codemirror/lang-json';
import { StreamLanguage } from '@codemirror/language';
import { toml } from '@codemirror/legacy-modes/mode/toml';
import { oneDark } from '@codemirror/theme-one-dark';
import { fetchCodexConfigFile, fetchCodexAuthFile } from '../../api';
import { useI18n } from '../../i18n';
import { useAppearance } from '../../appearance/store';

export default function ConfigEditor() {
  const { t } = useI18n();
  const { themeMode } = useAppearance();
  const [configContent, setConfigContent] = useState('');
  const [authContent, setAuthContent] = useState('');
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    Promise.all([
      fetchCodexConfigFile().then(setConfigContent).catch(() => ''),
      fetchCodexAuthFile().then(setAuthContent).catch(() => ''),
    ]).finally(() => setLoading(false));
  }, []);

  if (loading) return <Spin description={t('loading')} style={{ display: 'block', marginTop: 80 }} />;

  return (
    <Card className="hover-card" style={{ borderRadius: 12 }}
      title={<span style={{ fontSize: 15, fontWeight: 600 }}>{t('codex_config_title')}</span>}>
      <Tabs
        items={[
          {
            key: 'config',
            label: 'config.toml',
            children: <CodeMirrorBox value={configContent} lang="toml" themeMode={themeMode} />,
          },
          {
            key: 'auth',
            label: 'auth.json',
            children: <CodeMirrorBox value={authContent} lang="json" themeMode={themeMode} />,
          },
        ]}
      />
    </Card>
  );
}

function CodeMirrorBox({ value, lang, themeMode }: { value: string; lang: string; themeMode: string }) {
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);

  useEffect(() => {
    if (!editorRef.current) return;

    const extensions = [basicSetup, EditorView.editable.of(false)];
    if (lang === 'json') extensions.push(json());
    if (lang === 'toml') extensions.push(StreamLanguage.define(toml));
    if (themeMode === 'dark') extensions.push(oneDark);

    const state = EditorState.create({ doc: value, extensions });
    const view = new EditorView({ state, parent: editorRef.current });
    viewRef.current = view;

    return () => view.destroy();
  }, [lang, themeMode]);

  // Update content when value changes externally
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const cur = view.state.doc.toString();
    if (cur !== value) {
      view.dispatch({
        changes: { from: 0, to: cur.length, insert: value },
      });
    }
  }, [value]);

  return (
    <div ref={editorRef} style={{
      borderRadius: 8, overflow: 'hidden',
      border: '1px solid var(--border-subtle)',
      maxHeight: 'calc(100vh - 300px)', overflowY: 'auto'
    }} />
  );
}
