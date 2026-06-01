/**
* MediaPicker - 从媒体库选择文件并插入编辑器的弹窗组件
* 通过 window.inkforgeInsertMarkdown(text) 插入到当前编辑器
*/
import { useCallback, useEffect, useState } from 'react';
import { apiData, API_PREFIX } from '../lib/api';
import type { MediaItem, PaginatedResponse } from '../types';
import { Modal } from './Modal';
import { IconSearch, IconFolder } from './Icons';
import { paginationPages } from '../lib/api';
import { useI18n } from '../i18n';

const CATEGORIES = ['封面图', '文章配图', '头像/头像', '音频文件', '其他'];

interface Props {
  open: boolean;
  onClose: () => void;
}

export function MediaPicker({ open, onClose }: Props) {
  const { t } = useI18n();
  const [items, setItems] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [keyword, setKeyword] = useState('');
  const [kind, setKind] = useState('');
  const [category, setCategory] = useState('');
  const [page, setPage] = useState(1);
  const [pages, setPages] = useState(1);

  const fetchItems = useCallback(async (kw: string, k: string, cat: string, pg: number) => {
    setLoading(true);
    try {
      const query = new URLSearchParams({ page: String(pg), page_size: '20' });
      if (k) query.set('kind', k);
      if (cat) query.set('category', cat);
      if (kw.trim()) query.set('keyword', kw.trim());
      const r = await apiData<PaginatedResponse<MediaItem>>(`${API_PREFIX}/admin/media?${query.toString()}`);
      setItems(r.items || []);
      setPages(paginationPages(r));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (open) void fetchItems(keyword, kind, category, page);
  }, [open, keyword, kind, category, page, fetchItems]);

  function insert(item: MediaItem) {
    // 使用最小缩略图（或原图 fallback）
    const url = item.thumbnails?.[0]?.public_url ?? item.public_url;
    const originalUrl = item.public_url;
    const isImage = /\.(jpg|jpeg|png|gif|webp|svg)$/i.test(item.original_name);
    const markdown = isImage
      ? `\n![${item.original_name}](${originalUrl})\n`
      : `\n[${item.original_name}](${originalUrl})\n`;
    const fn = (window as any).inkforgeInsertMarkdown;
    if (fn) fn(markdown);
    else navigator.clipboard.writeText(markdown);
    onClose();
  }

  return (
    <Modal open={open} onClose={onClose} title={t('selectMedia')} width="860px">
      <div style={{ display: 'flex', gap: '10px', marginBottom: '16px', flexWrap: 'wrap' }}>
        <div style={{ position: 'relative', flex: '1', minWidth: '160px' }}>
          <IconSearch size={14} style={{ position: 'absolute', left: '10px', top: '50%', transform: 'translateY(-50%)', color: 'var(--md-outline)', pointerEvents: 'none' }} />
          <input type="text" placeholder={t('searchFilePlaceholder')} value={keyword}
            onChange={e => { setKeyword(e.target.value); setPage(1); }}
            style={{ width: '100%', paddingLeft: '32px', paddingRight: '10px', height: '36px', borderRadius: 'var(--radius-md)', border: 'none', fontSize: '13px', outline: 'none', background: 'var(--md-surface-container-low)', color: 'var(--md-on-surface)' }}
            onFocus={e => { e.currentTarget.style.outline = '2px solid var(--md-primary)'; e.currentTarget.style.outlineOffset = '-2px'; }}
            onBlur={e => { e.currentTarget.style.outline = 'none'; }}
          />
        </div>
        <select value={kind} onChange={e => { setKind(e.target.value); setPage(1); }}
          style={{ height: '36px', borderRadius: 'var(--radius-md)', border: 'none', padding: '0 8px', fontSize: '13px', background: 'var(--md-surface-container-low)', color: 'var(--md-on-surface)', cursor: 'pointer', outline: 'none' }}>
          <option value="">{t('allTypes')}</option>
          <option value="image">{t('imageType')}</option>
          <option value="audio">{t('audioType')}</option>
        </select>
        <select value={category} onChange={e => { setCategory(e.target.value); setPage(1); }}
          style={{ height: '36px', borderRadius: 'var(--radius-md)', border: 'none', padding: '0 8px', fontSize: '13px', background: 'var(--md-surface-container-low)', color: 'var(--md-on-surface)', cursor: 'pointer', outline: 'none' }}>
          <option value="">{t('allCategories')}</option>
          {CATEGORIES.map(c => <option key={c} value={c}>{c}</option>)}
        </select>
      </div>

      {loading ? (
        <div style={{ textAlign: 'center', padding: '40px', color: 'var(--md-outline)', fontSize: '13.5px' }}>{t('loading')}</div>
      ) : items.length > 0 ? (
        <>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))', gap: '10px', maxHeight: '420px', overflowY: 'auto' }}>
            {items.map(item => (
              <div key={item.id} onClick={() => insert(item)}
                title={`${item.original_name}\n${t('clickToInsert')}`}
                style={{ borderRadius: 'var(--radius-md)', border: 'none', padding: '10px', cursor: 'pointer', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '8px', transition: 'background 0.15s ease', background: 'var(--md-surface-container-low)' }}
                onMouseEnter={e => {
                  (e.currentTarget as HTMLDivElement).style.background = 'var(--md-surface-container)';
                }}
                onMouseLeave={e => {
                  (e.currentTarget as HTMLDivElement).style.background = 'var(--md-surface-container-low)';
                }}
              >
                {item.kind === 'image' ? (
                  (() => {
                    const thumbnailUrl = item.thumbnails?.[0]?.public_url ?? item.public_url;
                    return (
                      <img src={thumbnailUrl} alt={item.original_name}
                        style={{ width: '80px', height: '80px', objectFit: 'cover', borderRadius: 'var(--radius-sm)', border: 'none' }}
                      />
                    );
                  })()
                ) : (
                  <div style={{ width: '80px', height: '80px', borderRadius: 'var(--radius-sm)', background: 'var(--md-surface-container-lowest)', display: 'flex', alignItems: 'center', justifyContent: 'center', border: 'none' }}>
                    <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="var(--md-outline)" strokeWidth="1.6"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
                  </div>
                )}
                <span style={{ fontSize: '11px', color: 'var(--md-on-surface-variant)', textAlign: 'center', wordBreak: 'break-all', lineHeight: 1.3, maxWidth: '100%', overflow: 'hidden', display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical' as const }}>
                  {item.original_name}
                </span>
              </div>
            ))}
          </div>
          {pages > 1 && (
            <div style={{ display: 'flex', justifyContent: 'center', gap: '8px', marginTop: '16px', flexWrap: 'wrap' }}>
              <button disabled={page <= 1} onClick={() => setPage(p => p - 1)}
                style={{ padding: '5px 14px', borderRadius: '20px', border: 'none', background: 'var(--md-surface-container-low)', color: page <= 1 ? 'var(--md-outline)' : 'var(--md-on-surface)', fontSize: '12.5px', cursor: page <= 1 ? 'not-allowed' : 'pointer' }}>
                {t('prev')}
              </button>
              <span style={{ padding: '5px 10px', fontSize: '12.5px', color: 'var(--md-outline)' }}>{page} / {pages}</span>
              <button disabled={page >= pages} onClick={() => setPage(p => p + 1)}
                style={{ padding: '5px 14px', borderRadius: '20px', border: 'none', background: 'var(--md-surface-container-low)', color: page >= pages ? 'var(--md-outline)' : 'var(--md-on-surface)', fontSize: '12.5px', cursor: page >= pages ? 'not-allowed' : 'pointer' }}>
                {t('next')}
              </button>
            </div>
          )}
        </>
      ) : (
        <div style={{ textAlign: 'center', padding: '48px 0', color: 'var(--md-outline)', fontSize: '13.5px' }}>
          <IconFolder size={32} style={{ marginBottom: '8px', opacity: 0.4 }} />
          <div>{t('noMediaFilesHint')}</div>
        </div>
      )}
    </Modal>
  );
}
