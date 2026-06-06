import { useEffect, useRef } from 'react';
import { EditorState, Prec } from '@codemirror/state';
import { EditorView, keymap, lineNumbers, highlightActiveLine } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { syntaxHighlighting, defaultHighlightStyle } from '@codemirror/language';

// Inline CodeMirror styles to avoid extra CSS file
const cmStyles = `
.cm-editor {
  height: 100%;
  font-family: 'JetBrains Mono', 'Fira Code', Consolas, monospace;
  font-size: 13.5px;
  line-height: 1.75;
  border-radius: 10px;
  border: 1px solid var(--border-default);
  overflow: hidden;
}
.cm-editor.cm-focused { border-color: var(--primary-500); outline: none; }
.cm-scroller { overflow: auto; }
.cm-content { padding: 14px 0; }
.cm-line { padding: 0 14px; }
.cm-gutters {
  background: var(--bg-subtle);
  border-right: 1px solid var(--border-light);
  color: var(--text-muted);
}
.cm-activeLineGutter { background: var(--bg-subtle); }
.cm-activeLine { background: rgba(255,107,53,0.03); }
`;

/** 有序/无序列表自动续号 keymap */
const continueListOnEnter = keymap.of([
  {
    key: 'Enter',
    run: (view: EditorView) => {
      const pos = view.state.selection.main.head;
      const line = view.state.doc.lineAt(pos);
      const lineText = line.text;

      // 匹配有序列表: "1. ", "2. ", "10. " 等
      const orderedMatch = lineText.match(/^(\s*)(\d+)\.\s/);
      if (orderedMatch) {
        const indent = orderedMatch[1];
        const num = parseInt(orderedMatch[2], 10);
        const nextNum = num + 1;

        // 当前行只有 "X. " 没有内容 → 取消列表格式
        if (lineText.trim() === `${num}.`) {
          view.dispatch({
            changes: { from: line.from, to: line.to, insert: '' },
            selection: { anchor: line.from },
          });
          return true;
        }

        // 正常续号
        view.dispatch({
          changes: { from: pos, insert: `\n${indent}${nextNum}. ` },
          selection: {
            anchor: pos + 1 + indent.length + String(nextNum).length + 2,
          },
        });
        return true;
      }

      // 匹配无序列表: "- ", "* " 等
      const unorderedMatch = lineText.match(/^(\s*)[-*]\s/);
      if (unorderedMatch) {
        const indent = unorderedMatch[1];

        if (lineText.trim() === '-' || lineText.trim() === '*') {
          view.dispatch({
            changes: { from: line.from, to: line.to, insert: '' },
            selection: { anchor: line.from },
          });
          return true;
        }

        view.dispatch({
          changes: { from: pos, insert: `\n${indent}- ` },
          selection: { anchor: pos + 1 + indent.length + 2 },
        });
        return true;
      }

      return false; // 不处理，让默认 Enter 行为生效
    },
  },
]);

interface Props {
  value: string;
  onChange: (value: string) => void;
  /** Called when the editor is ready with the EditorView instance */
  onEditorReady?: (view: EditorView) => void;
}

export function CodeMirrorPanel({ value, onChange, onEditorReady }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  // Ref to track if the current change came from this panel
  const isUpdatingRef = useRef(false);

  useEffect(() => {
    if (!containerRef.current) return;

    // Inject styles once
    if (!document.getElementById('cm-inline-styles')) {
      const style = document.createElement('style');
      style.id = 'cm-inline-styles';
      style.textContent = cmStyles;
      document.head.appendChild(style);
    }

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        isUpdatingRef.current = true;
        onChange(update.state.doc.toString());
        isUpdatingRef.current = false;
      }
    });

    const state = EditorState.create({
      doc: value,
      extensions: [
        lineNumbers(),
        highlightActiveLine(),
        history(),
        Prec.highest(continueListOnEnter),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        markdown({ base: markdownLanguage }),
        syntaxHighlighting(defaultHighlightStyle),
        updateListener,
        EditorView.lineWrapping,
      ],
    });

    const view = new EditorView({
      state,
      parent: containerRef.current,
    });

    viewRef.current = view;
    onEditorReady?.(view);

    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Sync external value changes (e.g. when Milkdown panel changes the shared state)
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    if (isUpdatingRef.current) return;
    const current = view.state.doc.toString();
    if (current !== value) {
      view.dispatch({
        changes: { from: 0, to: current.length, insert: value },
      });
    }
  }, [value]);

  return <div ref={containerRef} style={{ height: '100%' }} />;
}
