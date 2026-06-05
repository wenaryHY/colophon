import { useCallback, useEffect, useRef, useState } from 'react';
import type { EditorView } from '@codemirror/view';
import { CodeMirrorPanel } from './CodeMirrorPanel';
import { TiptapPanel } from './TiptapPanel';
import { MediaPicker } from '../MediaPicker';
import { useI18n } from '../../i18n';

// 尝试从 Markdown 模板文本中提取对称包裹标记（如 **text** → { start: '**', end: '**' }）
function guessMarker(template: string): { start: string; end: string } | null {
  const patterns: [string, string][] = [
    ['**', '**'], ['__', '__'], ['*', '*'], ['_', '_'],
    ['~~', '~~'], ['==', '=='], ['`', '`'], ['```', '```'],
  ];
  for (const [start, end] of patterns) {
    if (template.startsWith(start) && template.endsWith(end) && template.length >= start.length + end.length) {
      return { start, end };
    }
  }
  return null;
}

export type EditorMode = 'source' | 'wysiwyg';

interface Props {
  value: string;
  onChange: (value: string) => void;
  onHtmlChange?: (html: string) => void;
  /** 编辑器模式变化回调，用于父组件感知当前模式 */
  onModeChange?: (mode: EditorMode) => void;
  /** 是否显示顶部源码/可视化 Tab 栏，默认 true */
  showTabBar?: boolean;
  /** 从外部控制的模式。设置此 prop 时，切换按钮仍然可用，但初始值由 prop 决定。变化时 MarkdownEditor 会跟随切换。 */
  forcedMode?: EditorMode;
}

export function MarkdownEditor({ value, onChange, onHtmlChange, onModeChange, showTabBar = true, forcedMode }: Props) {
  const { t } = useI18n();
  const [mode, setMode] = useState<EditorMode>(forcedMode || 'source');
  const [mediaOpen, setMediaOpen] = useState(false);
  const cmViewRef = useRef<EditorView | null>(null);

  // 当外部 forcedMode 变化时，跟进切换
  useEffect(() => {
    if (forcedMode && forcedMode !== mode) {
      setMode(forcedMode);
    }
  }, [forcedMode]);

  const handleModeChange = useCallback((newMode: EditorMode) => {
    setMode(newMode);
    onModeChange?.(newMode);
  }, [onModeChange]);

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
        const selection = v.state.selection.main;
        const hasSelection = selection.from !== selection.to;

        if (hasSelection) {
          const selectedText = v.state.sliceDoc(selection.from, selection.to);
          const marker = guessMarker(text);
          if (marker) {
            // 对称标记（如 **粗体文本**）→ 包裹选中内容
            const wrapped = marker.start + selectedText + marker.end;
            v.dispatch({
              changes: { from: selection.from, to: selection.to, insert: wrapped },
              selection: { anchor: selection.from + wrapped.length },
            });
          } else {
            // 非对称格式（如 > 引用、# 标题）→ 在选中行前插入
            const line = v.state.doc.lineAt(selection.from);
            v.dispatch({
              changes: { from: line.from, insert: text },
              selection: { anchor: line.from + text.length },
            });
          }
        } else {
          // 无选中 → 插入模板文本（保持现有行为）
          const pos = selection.head;
          v.dispatch({
            changes: { from: pos, insert: text },
            selection: { anchor: pos + text.length },
          });
        }
        v.focus();
      };
    }
  }, []);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', gap: '0' }}>
      {/* Tab Bar — 仅在 showTabBar 时渲染 */}
      {showTabBar && (
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: '2px',
        padding: '8px 10px 0',
        borderBottom: '1px solid var(--border-light)',
      }}>
        <TabButton active={mode === 'source'} onClick={() => handleModeChange('source')}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>
          </svg>
          {t('sourceCode')}
        </TabButton>
        <TabButton active={mode === 'wysiwyg'} onClick={() => handleModeChange('wysiwyg')}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/>
          </svg>
          {t('visual')}
        </TabButton>

        {/* Media library button */}
        <button
          type="button"
          onClick={() => setMediaOpen(true)}
          title={t('insertMedia')}
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
          {t('mediaLibrary')}
        </button>

        <span style={{ marginLeft: 'auto', fontSize: '12px', color: 'var(--text-muted)' }}>
          {mode === 'source' ? t('markdownSource') : t('wysiwyg')}
        </span>
      </div>
      )}

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
