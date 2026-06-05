import { useState } from 'react';
import type { EditorMode } from './MarkdownEditor/MarkdownEditor';
import { useI18n } from '../i18n';

// —————————————————————————————— 工具函数 ——————————————————————————————

/** 向 CodeMirror 编辑器插入文本 */
function insertMarkdownToEditor(text: string) {
  const fn = (window as any).inkforgeInsertMarkdown;
  if (fn) fn(text);
}

// —————————————————————————————— 样式常量 ——————————————————————————————

const TOOL_BTN_STYLE: React.CSSProperties = {
  display: 'flex', alignItems: 'center', justifyContent: 'center',
  minWidth: 44, height: 44, padding: '0 10px',
  border: 'none', background: 'transparent',
  color: 'var(--md-on-surface-variant)',
  fontFamily: "'Plus Jakarta Sans', sans-serif",
  fontSize: '0.875rem', fontWeight: 600,
  cursor: 'pointer', borderRadius: 'var(--radius-sm)',
  transition: 'all var(--transition-fast)',
  WebkitTapHighlightColor: 'transparent',
  touchAction: 'manipulation', userSelect: 'none', whiteSpace: 'nowrap',
};

// —————————————————————————————— Props ——————————————————————————————

export interface MobileEditorToolbarProps {
  /** 当前编辑器模式，源码模式下格式按钮禁用 */
  editorMode: EditorMode;
  /** 纯文字数 */
  wordCount: number;
  /** 预估阅读时间（分钟） */
  readTimeMin: number;
  /** 加粗回调 */
  onFormatBold: () => void;
  /** 斜体回调 */
  onFormatItalic: () => void;
  /** 下划线回调 */
  onFormatUnderline: () => void;
  /** 插入标题回调，level 为 1-6 */
  onInsertHeading: (level: number) => void;
  /** 插入引用回调 */
  onInsertQuote: () => void;
  /** 插入列表回调，type 为 unordered 或 ordered */
  onInsertList: (type: 'unordered' | 'ordered') => void;
  /** 插入链接回调（链接地址由工具栏内部管理并直接插入编辑器） */
  onInsertLink: () => void;
  /** 插入图片回调 */
  onInsertImage: () => void;
  /** 插入代码块回调 */
  onInsertCode: () => void;
  /** 打开预览回调 */
  onOpenPreview: () => void;
}

// —————————————————————————————— 组件 ——————————————————————————————

/** 移动端编辑器底部格式化工具栏 */
export default function MobileEditorToolbar({
  editorMode,
  wordCount,
  readTimeMin,
  onFormatBold,
  onFormatItalic,
  onFormatUnderline,
  onInsertHeading,
  onInsertQuote,
  onInsertList,
  onInsertLink,
  onInsertImage,
  onInsertCode,
  onOpenPreview,
}: MobileEditorToolbarProps) {
  const isSource = editorMode === 'source';
  const { format } = useI18n();

  // 展开/收起由组件内部管理
  const [expanded, setExpanded] = useState(false);

  // 子菜单开关
  const [headingOpen, setHeadingOpen] = useState(false);
  const [listOpen, setListOpen] = useState(false);
  const [linkOpen, setLinkOpen] = useState(false);
  const [linkUrl, setLinkUrl] = useState('');

  return (
    <div style={{
      position: 'relative', zIndex: 20,
      background: 'rgba(255, 248, 246, 0.92)',
      backdropFilter: 'blur(20px)', WebkitBackdropFilter: 'blur(20px)',
      borderTop: '1px solid var(--md-outline-variant)',
      boxShadow: 'var(--md-elevation-2)',
      userSelect: 'none', flexShrink: 0,
    }}>
      {/* —— 主行：B / I / U + 空白 + 展开/收起 —— */}
      <div style={{
        display: 'flex', alignItems: 'center',
        padding: '8px 12px', gap: 2,
        opacity: isSource ? 0.4 : 1,
        transition: 'opacity 0.2s',
      }}>
        <button onClick={onFormatBold} style={{ ...TOOL_BTN_STYLE, fontWeight: 800, fontFamily: "'Manrope', sans-serif", pointerEvents: isSource ? 'none' : 'auto' }} aria-label="粗体">
          B
        </button>
        <button onClick={onFormatItalic} style={{ ...TOOL_BTN_STYLE, fontStyle: 'italic', pointerEvents: isSource ? 'none' : 'auto' }} aria-label="斜体">
          I
        </button>
        <button onClick={onFormatUnderline} style={{ ...TOOL_BTN_STYLE, textDecoration: 'underline', textUnderlineOffset: 2, pointerEvents: isSource ? 'none' : 'auto' }} aria-label="下划线">
          U
        </button>
        <div style={{ flex: 1 }} />
        <button
          onClick={() => setExpanded(!expanded)}
          style={{ ...TOOL_BTN_STYLE, color: expanded ? 'var(--md-primary)' : 'var(--md-on-surface-muted)' }}
          aria-label={expanded ? '收起格式' : '更多格式'}
        >
          {expanded ? (
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          ) : (
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="5" cy="12" r="2"/><circle cx="12" cy="12" r="2"/><circle cx="19" cy="12" r="2"/></svg>
          )}
        </button>
      </div>

      {/* —— 展开行：H / 引用 / 列表 / 链接 / 图片 / 代码 / 预览 —— */}
      <div style={{
        display: 'flex', flexWrap: 'wrap', gap: 2,
        padding: '0 12px 8px',
        borderTop: expanded ? '1px solid var(--md-outline-variant)' : 'none',
        paddingTop: expanded ? 8 : 0,
        maxHeight: expanded ? 200 : 0,
        opacity: expanded ? 1 : 0,
        overflow: expanded ? 'visible' : 'hidden',
        transition: 'max-height 0.25s ease, opacity 0.2s ease, padding 0.25s ease',
      }}>
        {/* 标题下拉子菜单 */}
        <div style={{ position: 'relative' }}>
          <button onClick={() => setHeadingOpen(!headingOpen)} style={TOOL_BTN_STYLE} aria-label="标题">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M4 4v16M20 4v16M4 12h16"/></svg>
          </button>
          {headingOpen && (
            <>
              <div onClick={() => setHeadingOpen(false)} style={{ position: 'fixed', inset: 0, zIndex: 199 }} />
              <div style={{
                position: 'absolute', bottom: '100%', left: 0, marginBottom: 8,
                background: 'var(--md-surface)', borderRadius: 'var(--radius-md)',
                boxShadow: 'var(--md-elevation-3)', padding: 8, minWidth: 180,
                maxWidth: 'calc(100vw - 32px)', zIndex: 200,
              }}>
                {(['H1','H2','H3','H4','H5','H6'] as const).map((h, i) => (
                  <button key={h} onClick={() => { setHeadingOpen(false); onInsertHeading(i + 1); }} style={{
                    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                    width: '100%', padding: '10px 12px', border: 'none', background: 'transparent',
                    cursor: 'pointer', borderRadius: 'var(--radius-sm)', fontSize: 14, color: 'var(--md-on-surface)',
                  }}>
                    <span style={{ fontFamily: "'Manrope', sans-serif", fontWeight: 700, fontSize: i < 2 ? 18 : i < 4 ? 16 : 14 }}>{h}</span>
                    <span style={{ color: 'var(--md-on-surface-muted)', fontSize: 12 }}>标题 {i + 1}</span>
                  </button>
                ))}
              </div>
            </>
          )}
        </div>

        {/* 引用 */}
        <button onClick={onInsertQuote} style={TOOL_BTN_STYLE} aria-label="引用">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" style={{ opacity: 0.8 }}><path d="M6 17h3l2-4V7H5v6h3zm8 0h3l2-4V7h-6v6h3z"/></svg>
        </button>

        {/* 列表下拉子菜单 */}
        <div style={{ position: 'relative' }}>
          <button onClick={() => setListOpen(!listOpen)} style={TOOL_BTN_STYLE} aria-label="列表">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><line x1="8" y1="6" x2="20" y2="6"/><line x1="8" y1="12" x2="20" y2="12"/><line x1="8" y1="18" x2="20" y2="18"/><circle cx="4" cy="6" r="1.5"/><circle cx="4" cy="12" r="1.5"/><circle cx="4" cy="18" r="1.5"/></svg>
          </button>
          {listOpen && (
            <>
              <div onClick={() => setListOpen(false)} style={{ position: 'fixed', inset: 0, zIndex: 199 }} />
              <div style={{
                position: 'absolute', bottom: '100%', left: 0, marginBottom: 8,
                background: 'var(--md-surface)', borderRadius: 'var(--radius-md)',
                boxShadow: 'var(--md-elevation-3)', padding: 8, minWidth: 160,
                maxWidth: 'calc(100vw - 32px)', zIndex: 200,
              }}>
                <button onClick={() => { setListOpen(false); onInsertList('unordered'); }} style={{
                  display: 'flex', alignItems: 'center', gap: 10, width: '100%',
                  padding: '10px 12px', border: 'none', background: 'transparent', cursor: 'pointer',
                  borderRadius: 'var(--radius-sm)', fontSize: 14, color: 'var(--md-on-surface)',
                }}>
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="8" y1="6" x2="20" y2="6"/><line x1="8" y1="12" x2="20" y2="12"/><line x1="8" y1="18" x2="20" y2="18"/><circle cx="4" cy="6" r="1.5"/><circle cx="4" cy="12" r="1.5"/><circle cx="4" cy="18" r="1.5"/></svg>
                  <span>无序列表</span>
                </button>
                <button onClick={() => { setListOpen(false); onInsertList('ordered'); }} style={{
                  display: 'flex', alignItems: 'center', gap: 10, width: '100%',
                  padding: '10px 12px', border: 'none', background: 'transparent', cursor: 'pointer',
                  borderRadius: 'var(--radius-sm)', fontSize: 14, color: 'var(--md-on-surface)',
                }}>
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="10" y1="6" x2="20" y2="6"/><line x1="10" y1="12" x2="20" y2="12"/><line x1="10" y1="18" x2="20" y2="18"/><path d="M4 5h1v4M4 10h2M3 15l2-2v4"/></svg>
                  <span>有序列表</span>
                </button>
              </div>
            </>
          )}
        </div>

        {/* 链接下拉子菜单 — 链接地址由工具栏内部管理 */}
        <div style={{ position: 'relative' }}>
          <button onClick={() => setLinkOpen(!linkOpen)} style={TOOL_BTN_STYLE} aria-label="插入链接">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>
          </button>
          {linkOpen && (
            <>
              <div onClick={() => setLinkOpen(false)} style={{ position: 'fixed', inset: 0, zIndex: 199 }} />
              <div style={{
                position: 'absolute', bottom: '100%', right: 0, marginBottom: 8,
                background: 'var(--md-surface)', borderRadius: 'var(--radius-md)',
                boxShadow: 'var(--md-elevation-3)', padding: 12, minWidth: 260,
                maxWidth: 'calc(100vw - 32px)', zIndex: 200,
              }}>
                <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--md-on-surface-variant)', textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 8 }}>链接</div>
                <input
                  type="url"
                  placeholder="粘贴链接地址..."
                  autoComplete="url"
                  value={linkUrl}
                  onChange={e => setLinkUrl(e.target.value)}
                  style={{
                    width: '100%', padding: '10px 12px',
                    border: '1px solid var(--md-outline-variant)',
                    borderRadius: 'var(--radius-sm)', fontSize: 14,
                    color: 'var(--md-on-surface)',
                    background: 'var(--md-surface-container-low)',
                    outline: 'none', boxSizing: 'border-box',
                  }}
                />
                <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', marginTop: 8 }}>
                  <button onClick={() => { setLinkOpen(false); setLinkUrl(''); }} style={{
                    padding: '8px 16px', border: 'none', borderRadius: 'var(--radius-full)',
                    fontSize: 13, fontWeight: 600, background: 'transparent',
                    color: 'var(--md-on-surface-muted)', cursor: 'pointer',
                  }}>移除</button>
                  <button onClick={() => {
                    if (linkUrl.trim()) {
                      insertMarkdownToEditor(`[链接文本](${linkUrl.trim()})`);
                    }
                    setLinkOpen(false);
                    setLinkUrl('');
                    onInsertLink();
                  }} style={{
                    padding: '8px 16px', border: 'none', borderRadius: 'var(--radius-full)',
                    fontSize: 13, fontWeight: 600, background: 'var(--md-primary)',
                    color: '#fff', cursor: 'pointer',
                  }}>应用</button>
                </div>
              </div>
            </>
          )}
        </div>

        {/* 图片 */}
        <button onClick={onInsertImage} style={TOOL_BTN_STYLE} aria-label="插入图片">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><circle cx="8.5" cy="8.5" r="1.5"/><path d="m21 15-5-5L5 21"/></svg>
        </button>

        {/* 代码块 */}
        <button onClick={onInsertCode} style={TOOL_BTN_STYLE} aria-label="代码">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="m8 6-6 6 6 6"/><path d="m16 6 6 6-6 6"/></svg>
        </button>

        <div style={{ flex: 1 }} />

        {/* 预览按钮 */}
        <button onClick={onOpenPreview} style={{
          ...TOOL_BTN_STYLE, background: 'var(--md-surface-container-high)', color: 'var(--md-on-surface)',
          fontWeight: 600, borderRadius: 'var(--radius-full)', padding: '0 16px', gap: 4,
        }} aria-label="预览">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
          预览
        </button>
      </div>

      {/* —— 状态栏：字数 + 阅读时间 —— */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 8,
        padding: '6px 16px 12px', fontSize: 11, fontWeight: 500,
        color: 'var(--md-on-surface-muted)',
        borderTop: '1px solid var(--md-outline-variant)',
        opacity: 0.8,
      }}>
        <span>{format('wordCount', { count: String(wordCount) })}</span>
        <span>·</span>
        <span>{format('readTime', { minutes: String(readTimeMin) })}</span>
      </div>
    </div>
  );
}
