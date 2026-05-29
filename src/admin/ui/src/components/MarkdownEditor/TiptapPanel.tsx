import { useEffect, useRef } from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import Image from '@tiptap/extension-image';
import Placeholder from '@tiptap/extension-placeholder';
import Underline from '@tiptap/extension-underline';
import Link from '@tiptap/extension-link';
import { TextStyle } from '@tiptap/extension-text-style';
import Highlight from '@tiptap/extension-highlight';
import TextAlign from '@tiptap/extension-text-align';
import { Color } from '@tiptap/extension-color';
import { Markdown } from 'tiptap-markdown';

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
}

export function TiptapPanel({ value, onChange }: Props) {
  const isExternalUpdateRef = useRef(false);

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2, 3, 4] },
      }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Image.configure({ inline: false }),
      Placeholder.configure({
        placeholder: '开始写作...',
      }),
      Underline,
      Link.configure({ openOnClick: false }),
      TextStyle,
      Color,
      Highlight.configure({ multicolor: true }),
      TextAlign.configure({ types: ['heading', 'paragraph'] }),
      Markdown.configure({
        html: false,
        transformPastedText: true,
        transformCopiedText: true,
      }),
    ],
    content: value || '',
    onUpdate: ({ editor }) => {
      if (isExternalUpdateRef.current) return;
      const md = editor.storage.markdown!.getMarkdown();
      onChange(md);
    },
    editorProps: {
      attributes: {
        class: 'ProseMirror',
      },
    },
  });

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
      <div style={{
        display: 'flex', gap: 4, padding: '4px 8px',
        background: 'var(--md-surface-container-low)',
        borderBottom: '1px solid var(--md-outline-variant)',
        flexWrap: 'wrap', flexShrink: 0,
      }}>
        {/* 段落 */}
        <button onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()} className={`toolbar-btn ${editor.isActive('heading', { level: 1 }) ? 'is-active' : ''}`}>H1</button>
        <button onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()} className={`toolbar-btn ${editor.isActive('heading', { level: 2 }) ? 'is-active' : ''}`}>H2</button>
        <button onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()} className={`toolbar-btn ${editor.isActive('heading', { level: 3 }) ? 'is-active' : ''}`}>H3</button>
        <span style={{ width: 1, background: 'var(--md-outline-variant)', margin: '4px 4px' }} />
        {/* 行内 */}
        <button onClick={() => editor.chain().focus().toggleBold().run()} className={`toolbar-btn ${editor.isActive('bold') ? 'is-active' : ''}`}><b>B</b></button>
        <button onClick={() => editor.chain().focus().toggleItalic().run()} className={`toolbar-btn ${editor.isActive('italic') ? 'is-active' : ''}`}><i>I</i></button>
        <button onClick={() => editor.chain().focus().toggleUnderline().run()} className={`toolbar-btn ${editor.isActive('underline') ? 'is-active' : ''}`}><u>U</u></button>
        <button onClick={() => editor.chain().focus().toggleStrike().run()} className={`toolbar-btn ${editor.isActive('strike') ? 'is-active' : ''}`}><s>S</s></button>
        <span style={{ width: 1, background: 'var(--md-outline-variant)', margin: '4px 4px' }} />
        {/* 块级 */}
        <button onClick={() => editor.chain().focus().toggleBlockquote().run()} className={`toolbar-btn ${editor.isActive('blockquote') ? 'is-active' : ''}`}>"</button>
        <button onClick={() => editor.chain().focus().toggleCodeBlock().run()} className={`toolbar-btn ${editor.isActive('codeBlock') ? 'is-active' : ''}`}>&lt;/&gt;</button>
        <button onClick={() => editor.chain().focus().toggleBulletList().run()} className={`toolbar-btn ${editor.isActive('bulletList') ? 'is-active' : ''}`}>•</button>
        <button onClick={() => editor.chain().focus().toggleOrderedList().run()} className={`toolbar-btn ${editor.isActive('orderedList') ? 'is-active' : ''}`}>1.</button>
        <span style={{ width: 1, background: 'var(--md-outline-variant)', margin: '4px 4px' }} />
        {/* 链接 + HTML 颜色 */}
        <button onClick={() => {
          const url = window.prompt('URL');
          if (url) editor.chain().focus().setLink({ href: url }).run();
          else editor.chain().focus().unsetLink().run();
        }} className={`toolbar-btn ${editor.isActive('link') ? 'is-active' : ''}`}>🔗</button>
        <input type="color" onChange={(e) => editor.chain().focus().setColor(e.target.value).run()} style={{ width: 24, height: 24, padding: 0, border: 'none', cursor: 'pointer' }} title="文字颜色" />
      </div>

      {/* 编辑器内容区 */}
      <div
        style={{
          flex: 1,
          overflow: 'auto',
          padding: '14px 18px',
          background: 'var(--bg-card)',
        }}
      >
        <EditorContent editor={editor} />
      </div>
    </div>
  );
}
