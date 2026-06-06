import { useEffect, useRef, useState } from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import Image from '@tiptap/extension-image';
import Placeholder from '@tiptap/extension-placeholder';
import { TextStyle, Color } from '@tiptap/extension-text-style';
import Highlight from '@tiptap/extension-highlight';
import TextAlign from '@tiptap/extension-text-align';
import { Markdown } from 'tiptap-markdown';
import { useI18n } from '../../i18n';

// tiptap-markdown doesn't ship TypeScript types for editor.storage.markdown
declare module '@tiptap/core' {
  interface Storage {
    markdown?: {
      getMarkdown: () => string;
    };
  }
}

interface Props {
  value: string;
  onChange: (value: string) => void;
  onHtmlChange?: (html: string) => void;
}

const PRESET_COLORS = [
  '#000000', '#434343', '#666666', '#999999', '#b7b7b7', '#cccccc',
  '#980000', '#ff0000', '#ff9900', '#ffff00', '#00ff00', '#00ffff',
  '#4a86e8', '#0000ff', '#9900ff', '#ff00ff',
];

export function TiptapPanel({ value, onChange, onHtmlChange }: Props) {
  const { t } = useI18n();
  const isExternalUpdateRef = useRef(false);
  const [colorOpen, setColorOpen] = useState(false);
  const [linkDialogOpen, setLinkDialogOpen] = useState(false);
  const [linkUrl, setLinkUrl] = useState('');

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2, 3, 4] },
        link: { openOnClick: false },
      }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Image.configure({ inline: false }),
      Placeholder.configure({
        placeholder: t('startWriting'),
      }),
      TextStyle,
      Color,
      Highlight.configure({ multicolor: true }),
      TextAlign.configure({ types: ['heading', 'paragraph'] }),
      Markdown.configure({
        html: true,
        transformPastedText: true,
        transformCopiedText: true,
      }),
    ],
    content: value || '',
    onUpdate: ({ editor }) => {
      if (isExternalUpdateRef.current) return;
      const md = editor.storage.markdown!.getMarkdown();
      onChange(md);
      if (onHtmlChange) {
        onHtmlChange(editor.getHTML());
      }
    },
    editorProps: {
      attributes: {
        class: 'ProseMirror outline-none',
        role: 'textbox',
        'aria-multiline': 'true',
        'aria-label': t('editorContent'),
        'aria-describedby': 'editor-escape-instruction',
      },
      handleDOMEvents: {
        keydown: (_view, event) => {
          if (event.key === 'Escape') {
            // Re-focus the editor first to ensure the chain works, then blur
            (_view.dom as HTMLElement).blur();
            return true;
          }
          return false;
        },
      },
    },
  });

  /// 工具栏焦点导航：从当前焦点按钮移动到上一个或下一个按钮。
  /// 使用 `document.activeElement` 确定当前位置，避免 React state 闭包问题。
  const navigateToolbarFocus = (
    currentButton: HTMLElement,
    direction: 'next' | 'prev',
  ) => {
    const toolbar = currentButton.closest('[role="toolbar"]');
    if (!toolbar) return;
    const buttons = Array.from(
      toolbar.querySelectorAll<HTMLElement>('.toolbar-btn, .color-trigger-btn'),
    );
    if (buttons.length === 0) return;
    const currentIndex = buttons.indexOf(currentButton);
    if (currentIndex === -1) return;
    const nextIndex =
      direction === 'next'
        ? (currentIndex + 1) % buttons.length
        : (currentIndex - 1 + buttons.length) % buttons.length;
    buttons[nextIndex]?.focus();
  };

  // Sync external value changes (e.g. from CodeMirror source panel)
  useEffect(() => {
    if (!editor) return;
    const currentMarkdown = editor.storage.markdown!.getMarkdown();
    if (currentMarkdown !== value && value !== undefined) {
      isExternalUpdateRef.current = true;
      editor.commands.setContent(value || '');
      isExternalUpdateRef.current = false;
    }
  }, [value, editor]);

  // Register global insert function for media library
  useEffect(() => {
    if (!editor) return;
    (window as any).inkforgeInsertMarkdown = (text: string) => {
      // If it's an image markdown syntax, insert as image node
      const imageMatch = text.match(/^!\[([^\]]*)\]\(([^)]+)\)$/);
      if (imageMatch) {
        editor.chain().focus().setImage({ src: imageMatch[2], alt: imageMatch[1] }).run();
      } else {
        // Fallback: insert as text content
        editor.chain().focus().insertContent(text).run();
      }
    };
  }, [editor]);

  if (!editor) return null;

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {/* 格式化工具栏 */}
      <div
        role="toolbar"
        aria-label={t('editorToolbar')}
        aria-controls="tiptap-content-area"
        style={{
          display: 'flex', gap: 4, padding: '4px 8px',
          background: 'var(--md-surface-container-low)',
          borderBottom: '1px solid var(--md-outline-variant)',
          flexWrap: 'wrap', flexShrink: 0,
        }}
      >
        {/* 段落 */}
        <button
          role="button"
          aria-label={t('h1')}
          aria-pressed={editor.isActive('heading', { level: 1 })}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
          className={`toolbar-btn ${editor.isActive('heading', { level: 1 }) ? 'is-active' : ''}`}
        >H1</button>
        <button
          role="button"
          aria-label={t('h2')}
          aria-pressed={editor.isActive('heading', { level: 2 })}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
          className={`toolbar-btn ${editor.isActive('heading', { level: 2 }) ? 'is-active' : ''}`}
        >H2</button>
        <button
          role="button"
          aria-label={t('h3')}
          aria-pressed={editor.isActive('heading', { level: 3 })}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
          className={`toolbar-btn ${editor.isActive('heading', { level: 3 }) ? 'is-active' : ''}`}
        >H3</button>
        <span style={{ width: 1, background: 'var(--md-outline-variant)', margin: '4px 4px' }} />
        {/* 行内 */}
        <button
          role="button"
          aria-label={t('bold')}
          aria-pressed={editor.isActive('bold')}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => editor.chain().focus().toggleBold().run()}
          className={`toolbar-btn ${editor.isActive('bold') ? 'is-active' : ''}`}
        ><b>B</b></button>
        <button
          role="button"
          aria-label={t('italic')}
          aria-pressed={editor.isActive('italic')}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => editor.chain().focus().toggleItalic().run()}
          className={`toolbar-btn ${editor.isActive('italic') ? 'is-active' : ''}`}
        ><i>I</i></button>
        <button
          role="button"
          aria-label={t('underline')}
          aria-pressed={editor.isActive('underline')}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => editor.chain().focus().toggleUnderline().run()}
          className={`toolbar-btn ${editor.isActive('underline') ? 'is-active' : ''}`}
        ><u>U</u></button>
        <button
          role="button"
          aria-label={t('strike')}
          aria-pressed={editor.isActive('strike')}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => editor.chain().focus().toggleStrike().run()}
          className={`toolbar-btn ${editor.isActive('strike') ? 'is-active' : ''}`}
        ><s>S</s></button>
        <span style={{ width: 1, background: 'var(--md-outline-variant)', margin: '4px 4px' }} />
        {/* 块级 */}
        <button
          role="button"
          aria-label={t('blockquote')}
          aria-pressed={editor.isActive('blockquote')}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => editor.chain().focus().toggleBlockquote().run()}
          className={`toolbar-btn ${editor.isActive('blockquote') ? 'is-active' : ''}`}
        >"</button>
        <button
          role="button"
          aria-label={t('codeBlock')}
          aria-pressed={editor.isActive('codeBlock')}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => editor.chain().focus().toggleCodeBlock().run()}
          className={`toolbar-btn ${editor.isActive('codeBlock') ? 'is-active' : ''}`}
        >&lt;/&gt;</button>
        <button
          role="button"
          aria-label={t('bulletList')}
          aria-pressed={editor.isActive('bulletList')}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => editor.chain().focus().toggleBulletList().run()}
          className={`toolbar-btn ${editor.isActive('bulletList') ? 'is-active' : ''}`}
        >•</button>
        <button
          role="button"
          aria-label={t('orderedList')}
          aria-pressed={editor.isActive('orderedList')}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => editor.chain().focus().toggleOrderedList().run()}
          className={`toolbar-btn ${editor.isActive('orderedList') ? 'is-active' : ''}`}
        >1.</button>
        <span style={{ width: 1, background: 'var(--md-outline-variant)', margin: '4px 4px' }} />
        {/* 链接 + HTML 颜色 */}
        <button
          role="button"
          aria-label={t('link')}
          aria-pressed={editor.isActive('link')}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
              return;
            }
            if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'next');
              return;
            }
            if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
              e.preventDefault();
              navigateToolbarFocus(e.currentTarget, 'prev');
              return;
            }
          }}
          onClick={() => {
            const previousUrl = editor.getAttributes('link').href || '';
            setLinkUrl(previousUrl);
            setLinkDialogOpen(true);
          }}
          className={`toolbar-btn ${editor.isActive('link') ? 'is-active' : ''}`}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
            <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
          </svg>
        </button>
        <span style={{ position: 'relative' }}>
          <button
            role="button"
            onKeyDown={(e) => {
              if (e.key === 'Tab') {
                e.preventDefault();
                navigateToolbarFocus(e.currentTarget, e.shiftKey ? 'prev' : 'next');
                return;
              }
              if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
                e.preventDefault();
                navigateToolbarFocus(e.currentTarget, 'next');
                return;
              }
              if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
                e.preventDefault();
                navigateToolbarFocus(e.currentTarget, 'prev');
                return;
              }
            }}
            className={`color-trigger-btn toolbar-btn ${editor.isActive('textStyle') ? 'is-active' : ''}`}
            onClick={() => setColorOpen(!colorOpen)}
            aria-label={t('textColor')}
            aria-haspopup="true"
            aria-expanded={colorOpen}
          >A</button>
          {colorOpen && (
            <div style={{
              position: 'absolute', top: '100%', left: 0, zIndex: 100,
              background: 'var(--md-surface)', padding: 8, borderRadius: 8,
              boxShadow: '0 4px 12px rgba(0,0,0,0.15)', display: 'flex', flexWrap: 'wrap', gap: 4, width: 180
            }}>
              <button className="toolbar-btn" onClick={() => { editor.chain().focus().unsetColor().run(); setColorOpen(false); }} style={{ width: '100%', textAlign: 'left', fontSize: 12 }}>{t('defaultColor')}</button>
              {PRESET_COLORS.map(c => (
                <button
                  key={c}
                  onClick={() => { editor.chain().focus().setColor(c).run(); setColorOpen(false); }}
                  style={{ width: 24, height: 24, background: c, borderRadius: 4, border: c === '#000000' ? '1px solid #ccc' : 'none', cursor: 'pointer' }}
                />
              ))}
              <div style={{ width: '100%', marginTop: 4 }}>
                <input type="color" onChange={(e) => { editor.chain().focus().setColor(e.target.value).run(); setColorOpen(false); }} style={{ width: '100%', height: 28, cursor: 'pointer' }} />
              </div>
            </div>
          )}
        </span>
      </div>

      {/* 编辑器内容区 */}
      <div
        id="tiptap-content-area"
        style={{
          flex: 1,
          overflow: 'auto',
          padding: '14px 18px',
          background: 'var(--bg-card)',
        }}
      >
        <EditorContent editor={editor} />
      </div>

      {/* 辅助说明，用于打破焦点陷阱 */}
      <p id="editor-escape-instruction" className="sr-only">
        {t('escapeTip')}
      </p>

      {/* 链接弹窗 */}
      {linkDialogOpen && (
        <>
          {/* 遮罩层 */}
          <div
            style={{
              position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.5)',
              zIndex: 1000, display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}
            onClick={() => setLinkDialogOpen(false)}
          />
          {/* 弹窗卡片 */}
          <div style={{
            position: 'fixed', top: '50%', left: '50%', transform: 'translate(-50%,-50%)',
            zIndex: 1001, background: 'var(--md-surface-container-high)',
            borderRadius: 16, padding: 24, minWidth: 320, maxWidth: '90vw',
            boxShadow: 'var(--elevation-3)',
          }}>
            <h3 style={{ fontSize: 16, fontWeight: 700, margin: '0 0 16px', color: 'var(--md-on-surface)' }}>
              {t('link')}
            </h3>
            <input
              autoFocus
              type="url"
              placeholder="https://"
              value={linkUrl}
              onChange={(e) => setLinkUrl(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  if (linkUrl.trim()) {
                    editor.chain().focus().setLink({ href: linkUrl.trim() }).run();
                  }
                  setLinkDialogOpen(false);
                }
                if (e.key === 'Escape') {
                  e.stopPropagation();
                  setLinkDialogOpen(false);
                }
              }}
              style={{
                width: '100%', padding: '10px 14px',
                border: '1px solid var(--md-outline-variant)', borderRadius: 10,
                background: 'var(--md-surface-container-highest)',
                color: 'var(--md-on-surface)', fontSize: 14,
                outline: 'none',
              }}
            />
            <div style={{ display: 'flex', gap: 8, marginTop: 16, justifyContent: 'flex-end' }}>
              <button
                onClick={() => { editor.chain().focus().unsetLink().run(); setLinkDialogOpen(false); }}
                style={{
                  padding: '8px 16px', border: 'none', borderRadius: 10,
                  background: 'var(--md-surface-container)', color: 'var(--md-on-surface-variant)',
                  fontSize: 13, fontWeight: 600, cursor: 'pointer',
                }}
              >
                {editor.isActive('link') ? t('remove') : t('cancel')}
              </button>
              <button
                onClick={() => {
                  if (linkUrl.trim()) {
                    editor.chain().focus().setLink({ href: linkUrl.trim() }).run();
                  }
                  setLinkDialogOpen(false);
                }}
                style={{
                  padding: '8px 16px', border: 'none', borderRadius: 10,
                  background: 'var(--md-primary)', color: 'var(--md-on-primary)',
                  fontSize: 13, fontWeight: 600, cursor: 'pointer',
                }}
              >
                {t('confirm')}
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
