import { useEffect, useRef, useState } from 'react';
import { useEditor, EditorContent, Extension } from '@tiptap/react';
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

// Custom extension to allow escaping the editor focus trap via the Escape key
const EscapeFocusTrap = Extension.create({
  name: 'escapeFocusTrap',
  addKeyboardShortcuts() {
    return {
      Escape: () => {
        (this.editor.view.dom as HTMLElement).blur();
        document.body.focus();
        return true;
      },
    };
  },
});

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
  const [focusedBtnIndex, setFocusedBtnIndex] = useState(0);
  const toolbarRef = useRef<HTMLDivElement>(null);

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
      EscapeFocusTrap,
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
    },
  });

  const handleToolbarKeyDown = (e: React.KeyboardEvent) => {
    if (!toolbarRef.current) return;
    const buttonEls = Array.from(
      toolbarRef.current.querySelectorAll('.toolbar-btn, .color-trigger-btn')
    ) as HTMLElement[];
    if (buttonEls.length === 0) return;

    const activeEl = document.activeElement as HTMLElement;
    const currentIndex = buttonEls.indexOf(activeEl);
    if (currentIndex === -1) return; // Focus is not on the toolbar buttons

    let nextIndex = currentIndex;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      nextIndex = (currentIndex + 1) % buttonEls.length;
      e.preventDefault();
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      nextIndex = (currentIndex - 1 + buttonEls.length) % buttonEls.length;
      e.preventDefault();
    } else {
      return;
    }

    setFocusedBtnIndex(nextIndex);
    buttonEls[nextIndex].focus();
  };

  const handleToolbarFocus = (e: React.FocusEvent) => {
    if (!toolbarRef.current) return;
    const buttonEls = Array.from(
      toolbarRef.current.querySelectorAll('.toolbar-btn, .color-trigger-btn')
    ) as HTMLElement[];
    
    const targetIndex = buttonEls.indexOf(e.target as HTMLElement);
    if (targetIndex !== -1) {
      setFocusedBtnIndex(targetIndex);
    }
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
        ref={toolbarRef}
        onKeyDown={handleToolbarKeyDown}
        onFocus={handleToolbarFocus}
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
          tabIndex={focusedBtnIndex === 0 ? 0 : -1}
          onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
          className={`toolbar-btn ${editor.isActive('heading', { level: 1 }) ? 'is-active' : ''}`}
        >H1</button>
        <button
          role="button"
          aria-label={t('h2')}
          aria-pressed={editor.isActive('heading', { level: 2 })}
          tabIndex={focusedBtnIndex === 1 ? 0 : -1}
          onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
          className={`toolbar-btn ${editor.isActive('heading', { level: 2 }) ? 'is-active' : ''}`}
        >H2</button>
        <button
          role="button"
          aria-label={t('h3')}
          aria-pressed={editor.isActive('heading', { level: 3 })}
          tabIndex={focusedBtnIndex === 2 ? 0 : -1}
          onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
          className={`toolbar-btn ${editor.isActive('heading', { level: 3 }) ? 'is-active' : ''}`}
        >H3</button>
        <span style={{ width: 1, background: 'var(--md-outline-variant)', margin: '4px 4px' }} />
        {/* 行内 */}
        <button
          role="button"
          aria-label={t('bold')}
          aria-pressed={editor.isActive('bold')}
          tabIndex={focusedBtnIndex === 3 ? 0 : -1}
          onClick={() => editor.chain().focus().toggleBold().run()}
          className={`toolbar-btn ${editor.isActive('bold') ? 'is-active' : ''}`}
        ><b>B</b></button>
        <button
          role="button"
          aria-label={t('italic')}
          aria-pressed={editor.isActive('italic')}
          tabIndex={focusedBtnIndex === 4 ? 0 : -1}
          onClick={() => editor.chain().focus().toggleItalic().run()}
          className={`toolbar-btn ${editor.isActive('italic') ? 'is-active' : ''}`}
        ><i>I</i></button>
        <button
          role="button"
          aria-label={t('underline')}
          aria-pressed={editor.isActive('underline')}
          tabIndex={focusedBtnIndex === 5 ? 0 : -1}
          onClick={() => editor.chain().focus().toggleUnderline().run()}
          className={`toolbar-btn ${editor.isActive('underline') ? 'is-active' : ''}`}
        ><u>U</u></button>
        <button
          role="button"
          aria-label={t('strike')}
          aria-pressed={editor.isActive('strike')}
          tabIndex={focusedBtnIndex === 6 ? 0 : -1}
          onClick={() => editor.chain().focus().toggleStrike().run()}
          className={`toolbar-btn ${editor.isActive('strike') ? 'is-active' : ''}`}
        ><s>S</s></button>
        <span style={{ width: 1, background: 'var(--md-outline-variant)', margin: '4px 4px' }} />
        {/* 块级 */}
        <button
          role="button"
          aria-label={t('blockquote')}
          aria-pressed={editor.isActive('blockquote')}
          tabIndex={focusedBtnIndex === 7 ? 0 : -1}
          onClick={() => editor.chain().focus().toggleBlockquote().run()}
          className={`toolbar-btn ${editor.isActive('blockquote') ? 'is-active' : ''}`}
        >"</button>
        <button
          role="button"
          aria-label={t('codeBlock')}
          aria-pressed={editor.isActive('codeBlock')}
          tabIndex={focusedBtnIndex === 8 ? 0 : -1}
          onClick={() => editor.chain().focus().toggleCodeBlock().run()}
          className={`toolbar-btn ${editor.isActive('codeBlock') ? 'is-active' : ''}`}
        >&lt;/&gt;</button>
        <button
          role="button"
          aria-label={t('bulletList')}
          aria-pressed={editor.isActive('bulletList')}
          tabIndex={focusedBtnIndex === 9 ? 0 : -1}
          onClick={() => editor.chain().focus().toggleBulletList().run()}
          className={`toolbar-btn ${editor.isActive('bulletList') ? 'is-active' : ''}`}
        >•</button>
        <button
          role="button"
          aria-label={t('orderedList')}
          aria-pressed={editor.isActive('orderedList')}
          tabIndex={focusedBtnIndex === 10 ? 0 : -1}
          onClick={() => editor.chain().focus().toggleOrderedList().run()}
          className={`toolbar-btn ${editor.isActive('orderedList') ? 'is-active' : ''}`}
        >1.</button>
        <span style={{ width: 1, background: 'var(--md-outline-variant)', margin: '4px 4px' }} />
        {/* 链接 + HTML 颜色 */}
        <button
          role="button"
          aria-label={t('link')}
          aria-pressed={editor.isActive('link')}
          tabIndex={focusedBtnIndex === 11 ? 0 : -1}
          onClick={() => {
            const url = window.prompt(t('url'));
            if (url) editor.chain().focus().setLink({ href: url }).run();
            else editor.chain().focus().unsetLink().run();
          }}
          className={`toolbar-btn ${editor.isActive('link') ? 'is-active' : ''}`}
        >🔗</button>
        <span style={{ position: 'relative' }}>
          <button
            role="button"
            className={`color-trigger-btn toolbar-btn ${editor.isActive('textStyle') ? 'is-active' : ''}`}
            tabIndex={focusedBtnIndex === 12 ? 0 : -1}
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
    </div>
  );
}
