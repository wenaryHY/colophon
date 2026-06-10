import { useState, useEffect, useRef, useCallback } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { apiData, API, API_PREFIX, getQueryClient } from '../lib/api';
import { esc, generateSlugPreviewFromTitle } from '../lib/utils';
import type { AdminPost, Category, Tag } from '../types';

import { Button } from '../components/Button';
import { Input } from '../components/Input';
import { Modal } from '../components/Modal';
import { MarkdownEditor } from '../components/MarkdownEditor';
import type { EditorMode } from '../components/MarkdownEditor/MarkdownEditor';
import { MediaPicker } from '../components/MediaPicker';
import { IconFileText, IconPencil } from '../components/Icons';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';
import { useAutoSaveDraft, type DraftData } from '../hooks/useAutoSaveDraft';
import { usePreview, SESSION_STORAGE_KEY_FOR_PREVIEW_PARAMETERS_PASSED_TO_NEW_TAB } from '../preview';

type PageEditMode = 'editor' | 'custom_html';

interface RenderModeChoice {
  resolve: (mode: 'editor' | 'custom_html') => void;
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

const MENU_ITEM_STYLE: React.CSSProperties = {
  display: 'flex', alignItems: 'center', gap: 8,
  width: '100%', padding: '10px 12px',
  border: 'none', background: 'transparent', cursor: 'pointer',
  borderRadius: 'var(--radius-sm)', fontSize: '14px', color: 'var(--md-on-surface)',
  fontFamily: "'Plus Jakarta Sans', sans-serif",
};

const SUBMENU_ITEM_STYLE: React.CSSProperties = {
  display: 'flex', alignItems: 'center', gap: 10,
  width: '100%', padding: '10px 12px 10px 24px',
  border: 'none', background: 'transparent', cursor: 'pointer',
  fontSize: '0.85rem', color: 'var(--md-on-surface)',
};

const BUTTON_RESET: React.CSSProperties = {
  display: 'flex', alignItems: 'center', justifyContent: 'center',
  width: 40, height: 40, border: 'none', background: 'transparent',
  color: 'var(--md-on-surface)', cursor: 'pointer',
  borderRadius: 'var(--radius-sm)',
};

// —————————————————————————————— 工具函数 ——————————————————————————————

/** 纯文本字数统计 */
function countWords(text: string): { chars: number; readMinutes: number } {
  const cjk = (text.match(/[\u4e00-\u9fff\u3400-\u4dbf]/g) || []).length;
  const withoutCjk = text.replace(/[\u4e00-\u9fff\u3400-\u4dbf]/g, ' ');
  const enWords = withoutCjk.split(/\s+/).filter(Boolean).length;
  const total = cjk + enWords;
  return { chars: total, readMinutes: total === 0 ? 0 : Math.ceil(total / 300) };
}

/** 将时间戳转换为相对时间描述 */
function formatRelativeTime(timestamp: number): string {
  const seconds = Math.floor((Date.now() - timestamp) / 1000);
  if (seconds < 10) return '刚刚';
  if (seconds < 60) return `${seconds}秒前`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}分钟前`;
  return `${Math.floor(minutes / 60)}小时前`;
}

/** 向 CodeMirror 编辑器插入文本 */
function insertMarkdownToEditor(text: string) {
  const fn = (window as any).colophonInsertMarkdown;
  if (fn) fn(text);
}

// —————————————————————————————— 子组件 ——————————————————————————————

/** AppBar 溢出菜单 */
function AppbarMenu({
  status, setStatus, categoryId, setCategoryId,
  selectedTagIds, toggleTag, categories, tags,
  handleSave, saving,
  t, esc, format, setAppbarMenuOpen,
  publishStatusOpen, setPublishStatusOpen,
  categoryMenuOpen, setCategoryMenuOpen,
  tagsMenuOpen, setTagsMenuOpen,
}: {
  status: 'published' | 'draft';
  setStatus: (s: 'published' | 'draft') => void;
  categoryId: string;
  setCategoryId: (id: string) => void;
  selectedTagIds: string[];
  toggleTag: (id: string) => void;
  categories: Category[];
  tags: Tag[];
  handleSave: () => void;
  saving: boolean;
  t: (key: string) => string;
  esc: (text: string) => string;
  format: (key: string, vars?: Record<string, string | number>) => string;
  setAppbarMenuOpen: (v: boolean) => void;
  publishStatusOpen: boolean;
  setPublishStatusOpen: (v: boolean) => void;
  categoryMenuOpen: boolean;
  setCategoryMenuOpen: (v: boolean) => void;
  tagsMenuOpen: boolean;
  setTagsMenuOpen: (v: boolean) => void;
}) {
  return (
    <>
      <div onClick={() => setAppbarMenuOpen(false)} style={{ position: 'fixed', inset: 0, zIndex: 199 }} />
      <div style={{
        position: 'absolute', top: 48, right: 0,
        background: 'var(--md-surface)', borderRadius: 'var(--radius-md)',
        boxShadow: 'var(--md-elevation-3)', padding: 8, minWidth: 200, zIndex: 200,
      }}>
        {/* —— 发布状态（滑动子菜单） —— */}
        <button
          onClick={(e) => { e.stopPropagation(); setPublishStatusOpen(!publishStatusOpen); }}
          style={MENU_ITEM_STYLE}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M12 20V10"/><path d="M18 20V4"/><path d="M6 20v-4"/></svg>
          {t('publishSettings')}
          <span style={{ marginLeft: 'auto', fontSize: '0.75rem', color: 'var(--md-on-surface-muted)', background: 'var(--md-surface-container)', padding: '2px 8px', borderRadius: 'var(--radius-full)' }}>
            {status === 'published' ? t('publishedOption') : t('draftOption')}
          </span>
        </button>
        {publishStatusOpen && (
          <div style={{ background: 'var(--md-surface-container-low)', borderRadius: 'var(--radius-sm)', margin: '2px 0' }}>
            <button onClick={() => { setStatus('draft'); setPublishStatusOpen(false); }} style={{ ...SUBMENU_ITEM_STYLE, fontWeight: status === 'draft' ? 600 : 400, color: status === 'draft' ? 'var(--md-primary)' : 'var(--md-on-surface)' }}>
              {t('draftOption')}
            </button>
            <button onClick={() => { setStatus('published'); setPublishStatusOpen(false); }} style={{ ...SUBMENU_ITEM_STYLE, fontWeight: status === 'published' ? 600 : 400, color: status === 'published' ? 'var(--md-primary)' : 'var(--md-on-surface)' }}>
              {t('publishedOption')}
            </button>
          </div>
        )}

        {/* —— 分类（滑动子菜单） —— */}
        <button
          onClick={(e) => { e.stopPropagation(); setCategoryMenuOpen(!categoryMenuOpen); }}
          style={MENU_ITEM_STYLE}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M4 4h16v16H4z"/><path d="M4 9h16"/><path d="M9 4v16"/></svg>
          {t('categoryLabel')}
          <span style={{ marginLeft: 'auto', fontSize: '0.75rem', color: 'var(--md-on-surface-muted)', background: 'var(--md-surface-container)', padding: '2px 8px', borderRadius: 'var(--radius-full)' }}>
            {categoryId ? categories.find(c => c.id === categoryId)?.name || t('selected') : t('noCategory')}
          </span>
        </button>
        {categoryMenuOpen && (
          <div style={{ background: 'var(--md-surface-container-low)', borderRadius: 'var(--radius-sm)', margin: '2px 0', maxHeight: 160, overflowY: 'auto' }}>
            {categories.map(cat => (
              <button key={cat.id} onClick={() => { setCategoryId(cat.id); setCategoryMenuOpen(false); setAppbarMenuOpen(false); }} style={SUBMENU_ITEM_STYLE}>
                {cat.id === categoryId ? '✓ ' : ''}{esc(cat.name)}
              </button>
            ))}
            <button onClick={() => { setCategoryId(''); setCategoryMenuOpen(false); setAppbarMenuOpen(false); }} style={{ ...SUBMENU_ITEM_STYLE, color: 'var(--md-outline)' }}>
              {t('noCategory')}
            </button>
          </div>
        )}

        {/* —— 标签（滑动子菜单） —— */}
        <button
          onClick={(e) => { e.stopPropagation(); setTagsMenuOpen(!tagsMenuOpen); }}
          style={MENU_ITEM_STYLE}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M12 2H2v10l9.29 9.29c.94.94 2.48.94 3.42 0l6.58-6.58c.94-.94.94-2.48 0-3.42L12 2z"/><circle cx="7" cy="7" r="1.5"/></svg>
          {t('tagsLabel')}
          <span style={{ marginLeft: 'auto', fontSize: '0.75rem', color: 'var(--md-on-surface-muted)', background: 'var(--md-surface-container)', padding: '2px 8px', borderRadius: 'var(--radius-full)' }}>
            {selectedTagIds.length > 0 ? format('selectedWithCount', { count: selectedTagIds.length }) : t('noneSelected')}
          </span>
        </button>
        {tagsMenuOpen && (
          <div style={{ background: 'var(--md-surface-container-low)', borderRadius: 'var(--radius-sm)', margin: '2px 0', maxHeight: 160, overflowY: 'auto' }}>
            {tags.length > 0 ? tags.map(tag => (
              <label key={tag.id} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '10px 12px 10px 24px', fontSize: '0.85rem', cursor: 'pointer' }}>
                <input type="checkbox" checked={selectedTagIds.includes(tag.id)} onChange={() => toggleTag(tag.id)} style={{ width: 18, height: 18, accentColor: 'var(--md-primary)', cursor: 'pointer' }} />
                {esc(tag.name)}
              </label>
            )) : <span style={{ padding: '10px 12px 10px 24px', fontSize: '0.8rem', color: 'var(--md-outline)', display: 'block' }}>{t('noTagsAvailable')}</span>}
          </div>
        )}

        <div style={{ height: 1, background: 'var(--md-outline-variant)', margin: '4px 0' }} />

        {/* —— 保存 —— */}
        <button
          onClick={() => { setAppbarMenuOpen(false); handleSave(); }}
          disabled={saving}
          style={{ ...MENU_ITEM_STYLE, color: 'var(--md-primary)', fontWeight: 600 }}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>
          {saving ? t('saving') : t('save')}
        </button>
      </div>
    </>
  );
}

/** 底部工具栏 */
function BottomToolbar({
  editorMode, expanded, onToggleExpand,
  onOpenPreview, wordCountInfo, format,
  onFormatBold, onFormatItalic, onFormatUnderline,
  onInsertHeading, onInsertQuote, onInsertList,
  onInsertLink, onInsertImage, onInsertCode,
  isSaving, lastSavedAt,
}: {
  editorMode: EditorMode;
  expanded: boolean;
  onToggleExpand: () => void;
  onOpenPreview: () => void;
  wordCountInfo: { chars: number; readMinutes: number };
  format: (key: string, vars?: Record<string, string>) => string;
  onFormatBold: () => void;
  onFormatItalic: () => void;
  onFormatUnderline: () => void;
  onInsertHeading: (level: number) => void;
  onInsertQuote: () => void;
  onInsertList: (type: 'unordered' | 'ordered') => void;
  onInsertLink: (url: string) => void;
  onInsertImage: () => void;
  onInsertCode: () => void;
  isSaving: boolean;
  lastSavedAt: number | null;
}) {
  const isSource = editorMode === 'source';
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
      {/* 主行：B / I / U + 空白 + ··· */}
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
          onClick={onToggleExpand}
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

      {/* 展开行：H / 引用 / 列表 / 链接 / 图片 / 代码 / 预览 */}
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
        {/* H 下拉 */}
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

        {/* 列表下拉 */}
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

        {/* 链接下拉 */}
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
                  <button onClick={() => { if (linkUrl.trim()) { onInsertLink(linkUrl.trim()); } setLinkOpen(false); setLinkUrl(''); }} style={{
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

        {/* 代码 */}
        <button onClick={onInsertCode} style={TOOL_BTN_STYLE} aria-label="代码">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="m8 6-6 6 6 6"/><path d="m16 6 6 6-6 6"/></svg>
        </button>

        <div style={{ flex: 1 }} />

        {/* 预览 */}
        <button onClick={onOpenPreview} style={{
          ...TOOL_BTN_STYLE, background: 'var(--md-surface-container-high)', color: 'var(--md-on-surface)',
          fontWeight: 600, borderRadius: 'var(--radius-full)', padding: '0 16px', gap: 4,
        }} aria-label="预览">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
          预览
        </button>
      </div>

      {/* 状态栏：字数 + 阅读时间 + 保存状态 */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 8,
        padding: '6px 16px 12px', fontSize: 11, fontWeight: 500,
        color: 'var(--md-on-surface-muted)',
        borderTop: '1px solid var(--md-outline-variant)',
        opacity: 0.8,
      }}>
        <span>{format('wordCount', { count: wordCountInfo.chars.toString() })}</span>
        <span>·</span>
        <span>{format('readTime', { minutes: wordCountInfo.readMinutes.toString() })}</span>
        {isSaving ? (
          <span style={{ marginLeft: 'auto', color: 'var(--md-outline)' }}>保存中...</span>
        ) : lastSavedAt ? (
          <span style={{ marginLeft: 'auto', color: 'var(--md-outline)' }}>
            已保存 {formatRelativeTime(lastSavedAt)}
          </span>
        ) : null}
      </div>
    </div>
  );
}

/** 预览底部 Sheet */
function PreviewSheet({
  previewMode, setPreviewMode, onClose, preview,
}: {
  previewMode: 'content' | 'theme';
  setPreviewMode: (m: 'content' | 'theme') => void;
  onClose: () => void;
  preview: {
    openInNewTab: (mode?: 'content' | 'theme') => void;
    getRequestParams: () => { content: string; content_type: string } | null;
    getThemeParams: () => { theme_slug: string; theme_config?: string } | null;
  };
}) {
  const [iframeReady, setIframeReady] = useState(false);

  // 每次打开或切换模式时：先清旧 sessionStorage，写入新参数，再加载 iframe
  useEffect(() => {
    setIframeReady(false);
    sessionStorage.removeItem(SESSION_STORAGE_KEY_FOR_PREVIEW_PARAMETERS_PASSED_TO_NEW_TAB);

    const requestParams = preview.getRequestParams();
    const themeParams = preview.getThemeParams();
    const previewData = {
      mode: previewMode,
      content: requestParams?.content || '',
      content_type: requestParams?.content_type || 'post',
      theme_slug: themeParams?.theme_slug || '',
      theme_config: themeParams?.theme_config || '',
    };

    sessionStorage.setItem(
      SESSION_STORAGE_KEY_FOR_PREVIEW_PARAMETERS_PASSED_TO_NEW_TAB,
      JSON.stringify(previewData),
    );

    // 延迟一帧确保 sessionStorage 写入后再渲染 iframe
    const timer = setTimeout(() => setIframeReady(true), 50);
    return () => clearTimeout(timer);
  }, [previewMode]);

  return (
    <>
      {/* 遮罩 */}
      <div
        onClick={onClose}
        style={{
          position: 'fixed', inset: 0,
          background: 'rgba(61, 47, 41, 0.3)', zIndex: 90,
          opacity: 1, pointerEvents: 'auto',
        }}
      />
      {/* Sheet */}
      <div style={{
        position: 'fixed', left: 0, right: 0, bottom: 0,
        zIndex: 95, maxHeight: '92vh',
        background: 'var(--md-surface)',
        borderRadius: '32px 32px 0 0',
        boxShadow: 'var(--md-elevation-3)',
        transform: 'translateY(0)',
        transition: 'transform 0.35s cubic-bezier(0.32, 0.72, 0, 1)',
        display: 'flex', flexDirection: 'column',
      }}>
        {/* Header */}
        <div style={{
          display: 'flex', alignItems: 'center', justifyContent: 'space-between',
          padding: '12px 24px 8px', flexShrink: 0,
        }}>
          {/* 内容/主题切换 */}
          <div style={{ display: 'flex', gap: 0, background: 'var(--md-surface-container)', padding: 3, borderRadius: 'var(--radius-full)' }}>
            <button onClick={() => setPreviewMode('content')} style={{
              padding: '6px 16px', borderRadius: 'var(--radius-full)', border: 'none', cursor: 'pointer',
              fontSize: '12.5px', fontWeight: previewMode === 'content' ? 600 : 400,
              background: previewMode === 'content' ? 'var(--md-surface-container-lowest)' : 'transparent',
              color: previewMode === 'content' ? 'var(--md-on-surface)' : 'var(--md-on-surface-variant)',
            }}>
              内容预览
            </button>
            <button onClick={() => setPreviewMode('theme')} style={{
              padding: '6px 16px', borderRadius: 'var(--radius-full)', border: 'none', cursor: 'pointer',
              fontSize: '12.5px', fontWeight: previewMode === 'theme' ? 600 : 400,
              background: previewMode === 'theme' ? 'var(--md-surface-container-lowest)' : 'transparent',
              color: previewMode === 'theme' ? 'var(--md-on-surface)' : 'var(--md-on-surface-variant)',
            }}>
              主题预览
            </button>
          </div>
          <button onClick={onClose} style={{
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            width: 36, height: 36, border: 'none', background: 'transparent',
            color: 'var(--md-on-surface-muted)', cursor: 'pointer',
            borderRadius: 'var(--radius-full)', fontSize: '1.25rem',
          }} aria-label="关闭预览">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M18 6 6 18M6 6l12 12"/></svg>
          </button>
        </div>

        {/* Body — 使用 iframe 加载预览（sessionStorage 就绪后才渲染） */}
        <div style={{ flex: 1, overflow: 'hidden', position: 'relative', minHeight: 300 }}>
          {iframeReady ? (
            <iframe
              src={`/preview?mode=${previewMode}`}
              style={{ width: '100%', height: '100%', border: 'none' }}
              title="文章预览"
            />
          ) : (
            <div style={{
              display: 'flex', alignItems: 'center', justifyContent: 'center',
              height: '100%', color: 'var(--md-on-surface-variant)', fontSize: '14px',
            }}>
              正在加载预览...
            </div>
          )}
        </div>

        {/* Footer */}
        <div style={{
          padding: '12px 24px', flexShrink: 0,
          borderTop: '1px solid var(--md-outline-variant)',
          paddingBottom: 'calc(env(safe-area-inset-bottom, 12px) + 12px)',
        }}>
          <button
            onClick={() => preview.openInNewTab(previewMode)}
            style={{
              display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
              width: '100%', padding: '14px 0',
              background: 'var(--md-surface-container)', color: 'var(--md-on-surface)',
              fontFamily: "'Plus Jakarta Sans', sans-serif", fontSize: '0.938rem', fontWeight: 600,
              border: 'none', borderRadius: 'var(--radius-full)', cursor: 'pointer',
              minHeight: 48,
            }}
          >
            在新标签页查看
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><path d="M15 3h6v6"/><path d="M10 14 21 3"/></svg>
          </button>
        </div>
      </div>
    </>
  );
}

// —————————————————————————————— 主组件 ——————————————————————————————

export default function PostEditorMobile() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t, format } = useI18n();
  const toast = useToast();

  const isEdit = !!id;
  const [saving, setSaving] = useState(false);
  const [post, setPost] = useState<AdminPost | null>(null);

  // 表单字段
  const [title, setTitle] = useState('');
  const [userProvidedSlugOverride, setUserProvidedSlugOverride] = useState('');
  const [content, setContent] = useState('');
  const [contentHtml, setContentHtml] = useState('');
  const [excerpt, setExcerpt] = useState('');
  const [status, setStatus] = useState<'published' | 'draft'>('draft');
  const [categoryId, setCategoryId] = useState('');
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>([]);
  const [contentType, setContentType] = useState<'post' | 'page'>('post');
  const [pageEditMode, setPageEditMode] = useState<PageEditMode>('editor');
  const [customHtmlFile, setCustomHtmlFile] = useState<File | null>(null);
  const [draftRecovery, setDraftRecovery] = useState<DraftData | null>(null);

  // 移动端特有 state
  const [editorMode, setEditorMode] = useState<EditorMode>('wysiwyg');
  const [appbarMenuOpen, setAppbarMenuOpen] = useState(false);
  const [categoryMenuOpen, setCategoryMenuOpen] = useState(false);
  const [tagsMenuOpen, setTagsMenuOpen] = useState(false);
  const [publishStatusOpen, setPublishStatusOpen] = useState(false);
  const [showPreviewSheet, setShowPreviewSheet] = useState(false);
  const [previewMode, setPreviewMode] = useState<'content' | 'theme'>('content');
  const [mediaPickerOpen, setMediaPickerOpen] = useState(false);
  const [toolbarExpanded, setToolbarExpanded] = useState(false);
  const [renderModeChoice, setRenderModeChoice] = useState<RenderModeChoice | null>(null);

  // 预览上下文
  const preview = usePreview();

  // 用 ref 保持最新 content，避免频繁重新注册场景
  const contentRef = useRef(content);
  contentRef.current = content;

  // 用 ref 保持 pageEditMode，getRequestParams 中判断是否自定义HTML模式
  const pageEditModeRef = useRef(pageEditMode);
  pageEditModeRef.current = pageEditMode;

  // 注册预览场景
  useEffect(() => {
    preview.registerScene('post-editor', {
      getRequestParams: () => {
        let previewContent = contentRef.current;
        if (!previewContent && contentType === 'page' && pageEditModeRef.current === 'custom_html') {
          previewContent = '# 自定义HTML页面预览\n\n当前页面使用自定义HTML模式渲染，预览功能暂不支持自定义HTML。';
        }
        return {
          content: previewContent,
          content_type: contentType,
        };
      },
      getThemeParams: () => ({
        theme_slug: 'default',
      }),
    });
    return () => preview.unregisterScene();
  }, [contentType, preview.registerScene, preview.unregisterScene]);

  // 内容变化时触发预览刷新
  useEffect(() => {
    preview.refresh();
  }, [content, preview.refresh]);

  // 字数统计
  const wordCountInfo = countWords(content);

  // 草稿自动保存
  const { restore: restoreDraft, clear: clearDraft, isSaving, lastSavedAt } = useAutoSaveDraft(
    id,
    { title, content, contentHtml, excerpt, categoryId, tagIds: selectedTagIds },
    !!title || !!content
  );

  // 新建文章时静默恢复草稿（编辑模式由 postData useEffect 处理）
  useEffect(() => {
    if (isEdit) return;
    const draft = restoreDraft();
    if (draft) {
      setTitle(draft.title);
      setContent(draft.content);
      setContentHtml(draft.contentHtml);
      setExcerpt(draft.excerpt);
      setCategoryId(draft.categoryId);
      setSelectedTagIds(draft.tagIds);
    }
  }, []); // 仅在 mount 时执行一次

  // 加载文章详情（编辑模式）
  const { data: postData, isLoading: postLoading, isError: postError } = useQuery({
    queryKey: ['post', id],
    queryFn: () => apiData<AdminPost>(`${API_PREFIX}/admin/posts/${id}`),
    enabled: isEdit,
    staleTime: 0,
  });

  useEffect(() => {
    if (!postData) return;
    setPost(postData);
    setTitle(postData.title || '');
    setContent(postData.content_md || '');
    setContentHtml(postData.content_html || '');
    setExcerpt(postData.excerpt || '');
    setStatus(postData.status === 'published' ? 'published' : 'draft');
    setCategoryId(postData.category_id || '');
    setSelectedTagIds(postData.tags?.map((tag) => tag.id) || []);
    setContentType(postData.content_type || 'post');
    setPageEditMode(postData.page_render_mode === 'custom_html' ? 'custom_html' : 'editor');
    setUserProvidedSlugOverride(postData.slug || '');
    const draft = restoreDraft();
    if (draft && (!postData.updated_at || draft.savedAt > new Date(postData.updated_at).getTime())) {
      const isContentActuallyDifferent =
        draft.title !== (postData.title || '') ||
        draft.content !== (postData.content_md || '') ||
        draft.excerpt !== (postData.excerpt || '') ||
        draft.categoryId !== (postData.category_id || '');
      if (isContentActuallyDifferent) {
        setDraftRecovery(draft);
      }
    }
  }, [postData, restoreDraft]);

  // 加载分类
  const { data: categories = [] } = useQuery({
    queryKey: ['categories'],
    queryFn: () => apiData<Category[]>(`${API_PREFIX}/categories`),
  });

  // 加载标签
  const { data: tags = [] } = useQuery({
    queryKey: ['tags'],
    queryFn: () => apiData<Tag[]>(`${API_PREFIX}/tags`),
  });

  function toggleTag(tagId: string) {
    setSelectedTagIds((prev) =>
      prev.includes(tagId) ? prev.filter((item) => item !== tagId) : [...prev, tagId]
    );
  }

  async function handleSave(chosenRenderMode?: 'editor' | 'custom_html') {
    if (!title.trim()) { toast(t('titleRequired'), 'error'); return; }
    setSaving(true);
    try {
      const isPage = contentType === 'page';
      const hasCustomHtml = !!(post?.custom_html_path || customHtmlFile);
      const hasMdContent = !!content.trim();

      let renderMode: 'editor' | 'custom_html' = 'editor';
      if (isPage) {
        if (hasCustomHtml && hasMdContent) {
          if (chosenRenderMode) {
            renderMode = chosenRenderMode;
          } else {
            setSaving(false);
            const mode = await new Promise<'editor' | 'custom_html'>((resolve) => {
              setRenderModeChoice({ resolve });
            });
            setRenderModeChoice(null);
            setSaving(true);
            renderMode = mode;
          }
        } else if (hasCustomHtml) {
          renderMode = 'custom_html';
        } else {
          renderMode = 'editor';
        }
      }

      const body: Record<string, unknown> = {
        title: title.trim(),
        excerpt: excerpt.trim() || null,
        content_md: content,
        content_html: contentHtml || undefined,
        status,
        visibility: 'public',
        category_id: categoryId || null,
        content_type: contentType,
        allow_comment: contentType === 'post',
        pinned: false,
        page_render_mode: renderMode,
      };

      const trimmedSlugOverride = userProvidedSlugOverride.trim();
      if (trimmedSlugOverride) {
        body.slug = trimmedSlugOverride;
      }

      if (contentType === 'post') {
        body.tag_ids = selectedTagIds;
      }

      if (post?.custom_html_path && !customHtmlFile) {
        body.custom_html_path = post.custom_html_path;
      }

      if (isEdit && post?.id) {
        await apiData(`${API_PREFIX}/admin/posts/${post.id}`, { method: 'PATCH', body: JSON.stringify(body) });
      } else {
        await apiData(`${API_PREFIX}/admin/posts`, { method: 'POST', body: JSON.stringify(body) });
      }

      // 上传自定义HTML
      if (isPage && customHtmlFile) {
        const slug = post?.slug || title.trim().toLowerCase().replace(/[^a-z0-9\u4e00-\u9fff]+/g, '-').replace(/^-|-$/g, '');
        const fd = new FormData();
        fd.append('file', customHtmlFile);
        fd.append('slug', slug);
        const uploadRes = await fetch(`${API}${API_PREFIX}/admin/pages/upload`, {
          method: 'POST',
          body: fd,
          credentials: 'include',
        }).then(r => r.json());

        if (uploadRes.code !== 0) throw new Error(uploadRes.message || '上传失败');

        const postId = post?.id;
        if (postId) {
          await apiData(`${API_PREFIX}/admin/posts/${postId}`, {
            method: 'PATCH',
            body: JSON.stringify({
              custom_html_path: uploadRes.data.custom_html_path,
              page_render_mode: renderMode,
            }),
          });
        }
      }

      toast(t('saveSuccess'), 'success');
      getQueryClient().invalidateQueries({ queryKey: ['posts'] });
      if (id) getQueryClient().invalidateQueries({ queryKey: ['post', id] });
      clearDraft();
      navigate('/posts');
    } catch (error) {
      toast(error instanceof Error ? error.message : t('saveFailed'), 'error');
    } finally {
      setSaving(false);
    }
  }

  // —————————————————————————————— 工具栏格式化回调 ——————————————————————————————

  const handleFormatBold = useCallback(() => {
    if (editorMode === 'source') return;
    insertMarkdownToEditor('**粗体文本**');
  }, [editorMode]);

  const handleFormatItalic = useCallback(() => {
    if (editorMode === 'source') return;
    insertMarkdownToEditor('*斜体文本*');
  }, [editorMode]);

  const handleFormatUnderline = useCallback(() => {
    if (editorMode === 'source') return;
    insertMarkdownToEditor('<u>下划线文本</u>');
  }, [editorMode]);

  const handleInsertHeading = useCallback((level: number) => {
    insertMarkdownToEditor(`${'#'.repeat(level)} 标题`);
  }, []);

  const handleInsertQuote = useCallback(() => {
    insertMarkdownToEditor('\n> 引用文本\n');
  }, []);

  const handleInsertList = useCallback((type: 'unordered' | 'ordered') => {
    if (type === 'unordered') {
      insertMarkdownToEditor('\n- 列表项\n');
    } else {
      insertMarkdownToEditor('\n1. 列表项\n');
    }
  }, []);

  const handleInsertLink = useCallback(() => {
    // 链接插入已由 MobileEditorToolbar 内部处理（调用 insertMarkdownToEditor）
  }, []);

  const handleInsertImage = useCallback(() => {
    setMediaPickerOpen(true);
  }, []);

  const handleInsertCode = useCallback(() => {
    insertMarkdownToEditor('\n```\n代码块\n```\n');
  }, []);

  // —————————————————————————————— 渲染 ——————————————————————————————

  if (postLoading) {
    return (
      <div style={{ padding: '40px', textAlign: 'center', color: 'var(--md-on-surface-variant)', fontFamily: "'Plus Jakarta Sans', sans-serif" }}>
        {t('loading')}
      </div>
    );
  }

  if (postError) {
    return (
      <div style={{ padding: '40px', textAlign: 'center', fontFamily: "'Plus Jakarta Sans', sans-serif" }}>
        <h3>{t('loadFailed')}</h3>
        <p style={{ color: 'var(--md-on-surface-variant)' }}>{t('loadPostFailedHint')}</p>
      </div>
    );
  }

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column', background: 'var(--md-surface)' }}>

      {/* ==================== 草稿恢复提示 ==================== */}
      {draftRecovery && (
        <div style={{
          padding: '12px 16px', marginBottom: 0, borderRadius: 0,
          background: 'var(--md-primary-container)', color: 'var(--md-on-primary-container)',
          display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexShrink: 0,
          fontSize: '13px',
        }}>
          <span>{format('draftRecoveryHint', { date: new Date(draftRecovery.savedAt).toLocaleString() })}</span>
          <div style={{ display: 'flex', gap: 8 }}>
            <button className="md3-btn" onClick={() => {
              setTitle(draftRecovery.title);
              setContent(draftRecovery.content);
              setContentHtml(draftRecovery.contentHtml);
              setExcerpt(draftRecovery.excerpt);
              setCategoryId(draftRecovery.categoryId);
              setSelectedTagIds(draftRecovery.tagIds);
              setDraftRecovery(null);
              toast('已恢复上次编辑内容', 'info');
            }}>{t('draftRecover')}</button>
            <button className="md3-btn" onClick={() => {
              clearDraft();
              setDraftRecovery(null);
            }}>{t('draftDiscard')}</button>
          </div>
        </div>
      )}

      {/* ==================== AppBar (44px, glassmorphism) ==================== */}
      <header style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        height: 44, minHeight: 44, padding: '0 12px',
        background: 'rgba(255, 248, 246, 0.75)',
        backdropFilter: 'blur(16px)', WebkitBackdropFilter: 'blur(16px)',
        borderBottom: '1px solid var(--md-outline-variant)',
        position: 'sticky', top: 0, zIndex: 30, userSelect: 'none',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <button onClick={() => navigate('/posts')} style={BUTTON_RESET}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>
          </button>
          <span style={{ fontFamily: "'Manrope', sans-serif", fontSize: 16, fontWeight: 600, color: 'var(--md-on-surface)' }}>
            {isEdit ? t('editPostTitle') : t('createPostTitle')}
          </span>
        </div>
        <div style={{ position: 'relative' }}>
          <button onClick={() => setAppbarMenuOpen(!appbarMenuOpen)} style={BUTTON_RESET}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
              <circle cx="12" cy="5" r="2"/>
              <circle cx="12" cy="12" r="2"/>
              <circle cx="12" cy="19" r="2"/>
            </svg>
          </button>
          {appbarMenuOpen && (
            <AppbarMenu
              status={status} setStatus={setStatus}
              categoryId={categoryId} setCategoryId={setCategoryId}
              selectedTagIds={selectedTagIds} toggleTag={toggleTag}
              categories={categories} tags={tags}
              handleSave={handleSave} saving={saving}
              t={t} esc={esc} format={format}
              setAppbarMenuOpen={setAppbarMenuOpen}
              publishStatusOpen={publishStatusOpen} setPublishStatusOpen={setPublishStatusOpen}
              categoryMenuOpen={categoryMenuOpen} setCategoryMenuOpen={setCategoryMenuOpen}
              tagsMenuOpen={tagsMenuOpen} setTagsMenuOpen={setTagsMenuOpen}
            />
          )}
        </div>
      </header>

      {/* ==================== Editor Area ==================== */}
      <main style={{
        flex: 1, overflowY: 'auto', padding: '24px 16px 32px',
        WebkitOverflowScrolling: 'touch', overscrollBehavior: 'contain',
      }}>
        {/* 内容类型切换（仅新建时） */}
        {!isEdit && (
          <div style={{
            display: 'inline-flex', gap: 0, marginBottom: 16,
            background: 'var(--md-surface-container)', padding: 3, borderRadius: 'var(--radius-full)',
          }}>
            <button
              onClick={() => setContentType('post')}
              style={{
                padding: '6px 16px', borderRadius: 'var(--radius-full)',
                border: 'none', cursor: 'pointer',
                fontSize: '12.5px', fontWeight: contentType === 'post' ? 600 : 400,
                background: contentType === 'post' ? 'var(--md-primary)' : 'transparent',
                color: contentType === 'post' ? 'var(--md-on-primary)' : 'var(--md-on-surface-variant)',
                transition: 'all var(--transition-fast)',
                display: 'flex', alignItems: 'center', gap: 5,
              }}
            >
              <IconFileText size={13} /> {t('postTab')}
            </button>
            <button
              onClick={() => setContentType('page')}
              style={{
                padding: '6px 16px', borderRadius: 'var(--radius-full)',
                border: 'none', cursor: 'pointer',
                fontSize: '12.5px', fontWeight: contentType === 'page' ? 600 : 400,
                background: contentType === 'page' ? 'var(--md-primary)' : 'transparent',
                color: contentType === 'page' ? 'var(--md-on-primary)' : 'var(--md-on-surface-variant)',
                transition: 'all var(--transition-fast)',
                display: 'flex', alignItems: 'center', gap: 5,
              }}
            >
              <IconPencil size={13} /> {t('pageTab')}
            </button>
          </div>
        )}

        {/* 标题 */}
        <div style={{ marginBottom: 16 }}>
          <Input
            label={t('titleLabel')}
            placeholder={t('titlePlaceholder')}
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            style={{
              fontSize: '1.75rem', fontWeight: 700,
              fontFamily: "'Manrope', sans-serif",
              border: 'none', padding: '8px 0',
            }}
          />
        </div>

        {/* URL Slug */}
        <div style={{ marginBottom: 16, display: 'flex', flexDirection: 'column', gap: 4 }}>
          <Input
            label={t('postSlugLabel')}
            placeholder={generateSlugPreviewFromTitle(title)}
            value={userProvidedSlugOverride}
            onChange={(e) => setUserProvidedSlugOverride(e.target.value)}
          />
          <span style={{ fontSize: 12, color: 'var(--md-on-surface-variant)' }}>
            {t('postSlugAutoHint')}
          </span>
        </div>

        {/* 分隔线 */}
        <div style={{ width: '100%', height: 1, background: 'var(--md-outline-variant)', marginBottom: 24 }} />

        {/* 页面模式切换（仅页面类型） */}
        {contentType === 'page' && (
          <div style={{
            display: 'flex', gap: 0, marginBottom: 16,
            background: 'var(--md-surface-container)', padding: 3, borderRadius: 'var(--radius-full)',
          }}>
            <button onClick={() => setPageEditMode('editor')} style={{
              flex: 1, padding: '8px 12px', borderRadius: 'var(--radius-full)',
              border: 'none', cursor: 'pointer',
              fontSize: '12.5px', fontWeight: pageEditMode === 'editor' ? 600 : 400,
              background: pageEditMode === 'editor' ? 'var(--md-surface-container-lowest)' : 'transparent',
              color: pageEditMode === 'editor' ? 'var(--md-on-surface)' : 'var(--md-on-surface-variant)',
            }}>{t('markdownEditor')}</button>
            <button onClick={() => setPageEditMode('custom_html')} style={{
              flex: 1, padding: '8px 12px', borderRadius: 'var(--radius-full)',
              border: 'none', cursor: 'pointer',
              fontSize: '12.5px', fontWeight: pageEditMode === 'custom_html' ? 600 : 400,
              background: pageEditMode === 'custom_html' ? 'var(--md-surface-container-lowest)' : 'transparent',
              color: pageEditMode === 'custom_html' ? 'var(--md-on-surface)' : 'var(--md-on-surface-variant)',
            }}>{t('customHtml')}</button>
          </div>
        )}

        {/* 自定义 HTML 上传（仅页面且自定义 HTML 模式） */}
        {pageEditMode === 'custom_html' && (
          <div style={{
            border: '2px dashed var(--md-outline-variant)', borderRadius: 'var(--radius-lg)',
            padding: 24, textAlign: 'center',
            background: 'var(--md-surface-container-low)', marginBottom: 16,
          }}>
            <p style={{ fontSize: 14, fontWeight: 600, color: 'var(--md-on-surface)', marginBottom: 8 }}>
              {t('uploadCustomHtml')}
            </p>
            <input
              type="file"
              accept=".html,.htm,.zip"
              style={{ display: 'none' }}
              id="custom-html-upload-mobile"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) setCustomHtmlFile(f);
              }}
            />
            <Button
              variant="ghost"
              onClick={() => document.getElementById('custom-html-upload-mobile')?.click()}
            >
              {t('selectFile')}
            </Button>
            {customHtmlFile && (
              <div style={{
                marginTop: 8, padding: '6px 12px', borderRadius: 'var(--radius-sm)',
                background: 'var(--md-primary-container)',
                fontSize: 12, color: 'var(--md-on-primary-container)',
              }}>
                {format('selectedFile', { name: customHtmlFile.name, size: Math.ceil(customHtmlFile.size / 1024) })}
              </div>
            )}
            {post?.custom_html_path && !customHtmlFile && (
              <div style={{
                marginTop: 8, padding: '8px 14px', borderRadius: 'var(--radius-sm)',
                background: 'var(--md-surface-container)',
                fontSize: 12, color: 'var(--md-on-surface-variant)',
              }}>
                {format('currentPagePath', { path: post.custom_html_path })}
                <br />{t('reuploadOverrides')}
              </div>
            )}
          </div>
        )}

        {/* MarkdownEditor — 移动端隐藏内部 Tab 栏 */}
        <div style={{ marginBottom: 16, minHeight: '40vh' }}>
          <MarkdownEditor
            value={content}
            onChange={setContent}
            onHtmlChange={setContentHtml}
            onModeChange={setEditorMode}
            showTabBar={true}
          />
        </div>

        {/* 字数统计 */}
        <div style={{
          display: 'flex', alignItems: 'center', gap: 8,
          fontSize: 12, color: 'var(--md-on-surface-variant)', marginBottom: 16,
        }}>
          <span>{format('wordCount', { count: wordCountInfo.chars.toString() })}</span>
          <span>·</span>
          <span>{format('readTime', { minutes: wordCountInfo.readMinutes.toString() })}</span>
        </div>

        {/* 摘要（折叠） */}
        <details id="excerpt-details" style={{ borderTop: '1px solid var(--md-outline-variant)', paddingTop: 12 }}>
          <summary style={{
            display: 'flex', alignItems: 'center', gap: 8, padding: '8px 4px',
            fontSize: 13, color: 'var(--md-on-surface-variant)',
            cursor: 'pointer', userSelect: 'none', borderRadius: 'var(--radius-sm)', listStyle: 'none',
          }}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/><line x1="3" y1="14" x2="21" y2="14"/><line x1="3" y1="18" x2="21" y2="18"/></svg>
            <span>{t('excerptLabel')}</span>
            <span style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--md-outline)' }}>{excerpt.length}/300</span>
          </summary>
          <textarea
            value={excerpt}
            onChange={e => setExcerpt(e.target.value)}
            placeholder={t('excerptPlaceholder')}
            maxLength={300}
            rows={3}
            style={{
              width: '100%', padding: 12, boxSizing: 'border-box',
              fontSize: 14, lineHeight: 1.5, resize: 'vertical',
              background: 'var(--md-surface-container-low)',
              border: '1px solid var(--md-outline-variant)',
              borderRadius: 'var(--radius-sm)',
              color: 'var(--md-on-surface)',
              outline: 'none', fontFamily: 'inherit',
              marginTop: 8,
            }}
          />
        </details>
      </main>

      {/* ==================== Bottom Toolbar ==================== */}
      <BottomToolbar
        editorMode={editorMode}
        expanded={toolbarExpanded}
        onToggleExpand={() => setToolbarExpanded(!toolbarExpanded)}
        onOpenPreview={() => setShowPreviewSheet(true)}
        wordCountInfo={wordCountInfo}
        format={format}
        onFormatBold={handleFormatBold}
        onFormatItalic={handleFormatItalic}
        onFormatUnderline={handleFormatUnderline}
        onInsertHeading={handleInsertHeading}
        onInsertQuote={handleInsertQuote}
        onInsertList={handleInsertList}
        onInsertLink={handleInsertLink}
        onInsertImage={handleInsertImage}
        onInsertCode={handleInsertCode}
        isSaving={isSaving}
        lastSavedAt={lastSavedAt}
      />

      {/* ==================== Preview Bottom Sheet ==================== */}
      {showPreviewSheet && (
        <PreviewSheet
          previewMode={previewMode}
          setPreviewMode={setPreviewMode}
          onClose={() => setShowPreviewSheet(false)}
          preview={preview}
        />
      )}

      {/* ==================== Modals ==================== */}
      <Modal
        open={!!renderModeChoice}
        onClose={() => { renderModeChoice?.resolve('editor'); }}
        title={t('choosePageMode')}
        width="480px"
        actions={
          <>
            <Button variant="ghost" onClick={() => { renderModeChoice?.resolve('editor'); }}>{t('cancel')}</Button>
          </>
        }
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <p style={{ fontSize: 14, color: 'var(--md-on-surface-variant)', lineHeight: 1.6 }}>
            {t('choosePageModeDesc')}
          </p>
          <button
            onClick={() => { renderModeChoice?.resolve('editor'); }}
            style={{
              display: 'flex', alignItems: 'center', gap: 12,
              padding: '16px 18px', borderRadius: 'var(--radius-lg)',
              border: 'none', background: 'var(--md-surface-container-lowest)',
              cursor: 'pointer', transition: 'all var(--transition-normal)', textAlign: 'left' as const,
            }}
            onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-primary-container)'; }}
            onMouseLeave={e => { e.currentTarget.style.background = 'var(--md-surface-container-lowest)'; }}
          >
            <div style={{
              width: 40, height: 40, borderRadius: 12,
              background: 'var(--md-primary-container)', display: 'flex',
              alignItems: 'center', justifyContent: 'center', flexShrink: 0,
            }}>
              <IconFileText size={20} style={{ color: 'var(--md-on-primary-container)' }} />
            </div>
            <div>
              <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--md-on-surface)' }}>{t('useMarkdownEditor')}</div>
              <div style={{ fontSize: 12, color: 'var(--md-outline)', marginTop: 2 }}>{t('useMarkdownEditorDesc')}</div>
            </div>
          </button>
          <button
            onClick={() => { renderModeChoice?.resolve('custom_html'); }}
            style={{
              display: 'flex', alignItems: 'center', gap: 12,
              padding: '16px 18px', borderRadius: 'var(--radius-lg)',
              border: 'none', background: 'var(--md-surface-container-lowest)',
              cursor: 'pointer', transition: 'all var(--transition-normal)', textAlign: 'left' as const,
            }}
            onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-secondary-container)'; }}
            onMouseLeave={e => { e.currentTarget.style.background = 'var(--md-surface-container-lowest)'; }}
          >
            <div style={{
              width: 40, height: 40, borderRadius: 12,
              background: 'var(--md-secondary-container)', display: 'flex',
              alignItems: 'center', justifyContent: 'center', flexShrink: 0,
            }}>
              <IconPencil size={20} style={{ color: 'var(--md-on-secondary-container)' }} />
            </div>
            <div>
              <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--md-on-surface)' }}>{t('useCustomHtml')}</div>
              <div style={{ fontSize: 12, color: 'var(--md-outline)', marginTop: 2 }}>{t('useCustomHtmlDesc')}</div>
            </div>
          </button>
        </div>
      </Modal>

      {/* MediaPicker */}
      <MediaPicker open={mediaPickerOpen} onClose={() => setMediaPickerOpen(false)} />
    </div>
  );
}
