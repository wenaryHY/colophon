import { useCallback, useRef, useState } from 'react';
import type { EditorView } from '@codemirror/view';
import { CodeMirrorPanel } from './CodeMirrorPanel';
import { TiptapPanel } from './TiptapPanel';
import { MediaPicker } from '../MediaPicker';

type Mode = 'source' | 'wysiwyg';

interface Props {
  value: string;
  onChange: (value: string) => void;
  onHtmlChange?: (html: string) => void;
}

export function MarkdownEditor({ value, onChange, onHtmlChange }: Props) {
  const [mode, setMode] = useState<Mode>('source');
  const [mediaOpen, setMediaOpen] = useState(false);
  const cmViewRef = useRef<EditorView | null>(null);

  const handleChange = useCallback((newValue: string) => {
    onChange(newValue);
  }, [onChange]);

  // Register CodeMirror instance for external content injection
  const handleEditorReady = useCallback((view: any) => {
    if (view && 'state' in view) {
      cmViewRef.current = view;
      (window as any).inkforgeInsertMarkdown = (text: string) => {
        if (!cmViewRef.current) return;
        const v = cmViewRef.current;
        const pos = v.state.selection.main.head;
        v.dispatch({ changes: { from: pos, insert: text } });
        v.focus();
      };
    }
  }, []);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', gap: '0' }}>
      {/* Tab Bar */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: '2px',
        padding: '8px 10px 0',
        borderBottom: '1px solid var(--border-light)',
      }}>
        <TabButton active={mode === 'source'} onClick={() => setMode('source')}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>
          </svg>
          源码
        </TabButton>
        <TabButton active={mode === 'wysiwyg'} onClick={() => setMode('wysiwyg')}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/>
          </svg>
          可视化
        </TabButton>

        {/* Media library button */}
        <button
          type="button"
          onClick={() => setMediaOpen(true)}
          title="插入媒体文件"
          style={{
            display: 'inline-flex', alignItems: 'center', gap: '5px',
            padding: '7px 12px', borderRadius: '8px',
            border: '1px solid var(--border-default)',
            background: 'var(--bg-subtle)',
            color: 'var(--text-secondary)',
            fontSize: '12.5px', fontWeight: 500, cursor: 'pointer',
            transition: 'all 0.15s ease',
            marginLeft: '6px',
          }}
          onMouseEnter={e => {
            (e.currentTarget as HTMLButtonElement).style.borderColor = 'var(--primary-500)';
            (e.currentTarget as HTMLButtonElement).style.color = 'var(--primary-500)';
          }}
          onMouseLeave={e => {
            (e.currentTarget as HTMLButtonElement).style.borderColor = 'var(--border-default)';
            (e.currentTarget as HTMLButtonElement).style.color = 'var(--text-secondary)';
          }}
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
            <circle cx="8.5" cy="8.5" r="1.5"/>
            <polyline points="21 15 16 10 5 21"/>
          </svg>
          媒体库
        </button>

        <span style={{ marginLeft: 'auto', fontSize: '12px', color: 'var(--text-muted)' }}>
          {mode === 'source' ? 'Markdown 源码' : '所见即所得'}
        </span>
      </div>

      {/* Editor Panels */}
      <div style={{ flex: 1, overflow: 'hidden', minHeight: '320px' }}>
        {mode === 'source' ? (
          <CodeMirrorPanel value={value} onChange={handleChange} onEditorReady={handleEditorReady} />
        ) : (
          <TiptapPanel value={value} onChange={handleChange} onHtmlChange={onHtmlChange} />
        )}
      </div>

      <MediaPicker open={mediaOpen} onClose={() => setMediaOpen(false)} />
    </div>
  );
}

function TabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: '5px',
        padding: '7px 14px',
        borderRadius: '8px 8px 0 0',
        border: 'none',
        borderBottom: active ? '2px solid var(--primary-500)' : '2px solid transparent',
        background: active ? 'var(--bg-card)' : 'transparent',
        color: active ? 'var(--primary-500)' : 'var(--text-muted)',
        fontSize: '13px',
        fontWeight: active ? 600 : 500,
        cursor: 'pointer',
        transition: 'all 0.15s ease',
        marginBottom: '-1px',
      }}
    >
      {children}
    </button>
  );
}
