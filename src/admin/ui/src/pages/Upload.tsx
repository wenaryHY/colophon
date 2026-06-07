import { useRef, useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { apiData, paginationPages, API, API_PREFIX, getQueryClient } from '../lib/api';

import { esc } from '../lib/utils';
import type { MediaItem, PaginatedResponse } from '../types';
import { PageHeader } from '../components/PageHeader';
import { Card } from '../components/Card';
import { Pagination } from '../components/Pagination';
import { Select as IfSelect, SelectItem } from '../components/Select';
import { useToast } from '../contexts/ToastContext';
import { useMediaCategories } from '../hooks/useMediaCategories';
import { useI18n } from '../i18n';
import { MediaCategorySelect } from '../components/media/MediaCategorySelect';
import { IconUpload, IconCheckCircle, IconAlertCircle, IconTrash2, IconSearch, IconEdit2, IconFolder } from '../components/Icons';

const dropZoneBase: React.CSSProperties = {
  border: '2px dashed var(--md-outline-variant)', borderRadius: '14px',
  padding: '52px 24px', textAlign: 'center', cursor: 'pointer',
  transition: 'all 0.2s ease',
};
const dropZoneActive: React.CSSProperties = {
  ...dropZoneBase,
  borderColor: 'var(--md-primary)',
  background: 'var(--md-primary-container)',
};

export default function Upload() {
  const { t } = useI18n();
  const toast = useToast();
  const { categories } = useMediaCategories();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [result, setResult] = useState<{ success: boolean; message: string } | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [page, setPage] = useState(1);
  const [kind, setKind] = useState('');
  const [category, setCategory] = useState('');
  const [keyword, setKeyword] = useState('');
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');

  const { data: payload, isLoading } = useQuery({
    queryKey: ['media', { page, kind, category, keyword }],
    queryFn: () => {
      const query = new URLSearchParams({ page: String(page), page_size: '16' });
      if (kind) query.set('kind', kind);
      if (category) query.set('category', category);
      if (keyword.trim()) query.set('keyword', keyword.trim());
      return apiData<PaginatedResponse<MediaItem>>(`${API_PREFIX}/admin/media?${query.toString()}`);
    },
  });

  const items = payload?.items ?? [];
  const pages = payload ? paginationPages(payload) : 1;

  const invalidateMedia = () => getQueryClient().invalidateQueries({ queryKey: ['media'] });

  const deleteMutation = useMutation({
    mutationFn: (id: string) =>
      apiData(`${API_PREFIX}/admin/media/${id}`, { method: 'DELETE' }),
    onSuccess: () => { toast(t('deleteMediaSuccess'), 'success'); invalidateMedia(); },
    onError: (error) => toast(error instanceof Error ? error.message : t('deleteMediaFailed'), 'error'),
  });

  const renameMutation = useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      apiData(`${API_PREFIX}/admin/media/${id}`, {
        method: 'PATCH',
        body: JSON.stringify({ name }),
      }),
    onSuccess: () => { toast(t('renameSuccess'), 'success'); setRenamingId(null); invalidateMedia(); },
    onError: (error) => toast(error instanceof Error ? error.message : t('renameFailed'), 'error'),
  });

  const setCategoryMutation = useMutation({
    mutationFn: ({ id, cat }: { id: string; cat: string }) =>
      apiData(`${API_PREFIX}/admin/media/${id}/category`, {
        method: 'PATCH',
        body: JSON.stringify({ category: cat || null }),
      }),
    onSuccess: () => invalidateMedia(),
    onError: (error) => toast(error instanceof Error ? error.message : t('updateCategoryFailed'), 'error'),
  });

  async function doUpload(file: File) {
    const fd = new FormData();
    fd.append('file', file);
    if (category) fd.append('category', category);
    setResult(null);
    try {
      const res = await fetch(`${API}${API_PREFIX}/admin/media`, {
        method: 'POST',
        body: fd,
        credentials: 'include',
      });
      const json = await res.json();
      if (!res.ok || json.code !== 0) throw new Error(json.message || t('uploadFailed'));
      setResult({ success: true, message: json.data.public_url });
      toast(t('uploadSuccess'), 'success');
      invalidateMedia();
      setPage(1);
    } catch (error) {
      const msg = error instanceof Error ? error.message : t('uploadFailed');
      setResult({ success: false, message: msg });
      toast(msg, 'error');
    }
    if (fileInputRef.current) fileInputRef.current.value = '';
  }

  function deleteMedia(id: string) {
    if (!window.confirm(t('deleteMediaConfirm'))) return;
    deleteMutation.mutate(id);
  }

  function startRename(item: MediaItem) {
    setRenamingId(item.id);
    setRenameValue(item.original_name);
  }

  function commitRename(id: string) {
    if (!renameValue.trim()) { setRenamingId(null); return; }
    renameMutation.mutate({ id, name: renameValue.trim() });
  }

  function setItemCategory(id: string, cat: string) {
    setCategoryMutation.mutate({ id, cat });
  }

  function insertIntoEditor(url: string) {
    const insertFn = (window as any).inkforgeInsertMarkdown;
    if (!insertFn) { toast(t('openEditorFirst'), 'error'); return; }
    const isImage = /\.(jpg|jpeg|png|gif|webp|svg)$/i.test(url);
    const text = isImage ? `\n![](${url})\n` : `\n[文件](${url})\n`;
    insertFn(text);
    toast(t('insertedToEditor'), 'success');
  }

  function handleFileSelect(e: React.ChangeEvent<HTMLInputElement>) {
    if (e.target.files?.[0]) void doUpload(e.target.files[0]);
  }

  function handleDrop(e: React.DragEvent) {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files[0]) void doUpload(e.dataTransfer.files[0]);
  }

  return (
    <>
      <PageHeader title={t('mediaTitle')} subtitle={t('mediaSubtitle')} />

      <Card>
        <div style={{ padding: '22px' }}
          onClick={() => fileInputRef.current?.click()}
          onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
          onDragLeave={() => setDragOver(false)}
          onDrop={handleDrop}
        >
          <div style={dragOver ? dropZoneActive : dropZoneBase}
            onMouseEnter={(e: React.MouseEvent<HTMLDivElement>) => {
              if (!dragOver) {
                e.currentTarget.style.borderColor = 'var(--md-primary)';
                e.currentTarget.style.background = 'var(--md-primary-container)';
              }
            }}
            onMouseLeave={(e: React.MouseEvent<HTMLDivElement>) => {
              if (!dragOver) {
                e.currentTarget.style.borderColor = 'var(--md-outline-variant)';
                e.currentTarget.style.background = 'transparent';
              }
            }}
          >
            <div style={{
              width: '56px', height: '56px', margin: '0 auto 16px',
              borderRadius: '14px', display: 'flex', alignItems: 'center',
              justifyContent: 'center',
              background: dragOver ? 'var(--md-primary-container)' : 'var(--md-surface-container)',
              transition: 'all 0.2s',
            }}>
              <IconUpload size={28} style={{ color: dragOver ? 'var(--md-primary)' : 'var(--md-outline)' }} />
            </div>
            <p style={{ fontSize: '15px', fontWeight: 600, color: 'var(--md-on-surface)', marginBottom: '4px' }}>
              {t('uploadAreaTitle')}
            </p>
            <p style={{ fontSize: '12.5px', color: 'var(--md-outline)' }}>
              {t('uploadSupportedFormats')}
            </p>
          </div>
          <input ref={fileInputRef} type="file" hidden onChange={handleFileSelect} />

          {result && (
            <div className="if-slide-up" style={{ marginTop: '20px' }}>
              {result.success ? (
                <div style={{
                  background: 'var(--md-primary-container)', padding: '18px 20px', borderRadius: '14px',
                  display: 'flex', alignItems: 'flex-start', gap: '12px',
                }}>
                  <IconCheckCircle size={22} style={{ color: 'var(--md-primary)', flexShrink: 0, marginTop: '2px' }} />
                  <div style={{ flex: 1 }}>
                    <p style={{ fontWeight: 700, fontSize: '14px', color: 'var(--md-on-primary-container)', marginBottom: '4px' }}>{t('uploadSuccess')}</p>
                    <a href={result.message} target="_blank" rel="noreferrer"
                      style={{ color: 'var(--md-on-primary-container)', fontFamily: 'monospace', fontSize: '12px', wordBreak: 'break-all', textDecoration: 'none', opacity: 0.8 }}
                      onMouseEnter={(e: React.MouseEvent<HTMLAnchorElement>) => e.currentTarget.style.textDecoration = 'underline'}
                      onMouseLeave={(e: React.MouseEvent<HTMLAnchorElement>) => e.currentTarget.style.textDecoration = 'none'}
                    >{esc(result.message)}</a>
                    <div style={{ display: 'flex', gap: '8px', marginTop: '12px', flexWrap: 'wrap' }}>
                      <button type="button"
                        onClick={(e: React.MouseEvent<HTMLButtonElement>) => { e.stopPropagation(); navigator.clipboard.writeText(result.message).then(() => toast(t('copySuccess'), 'success')).catch(() => toast(t('copyFailed'), 'error')); }}
                        style={{ padding: '6px 14px', borderRadius: '8px', border: 'none', background: 'var(--md-surface-container)', color: 'var(--md-on-primary-container)', fontSize: '12.5px', fontWeight: 600, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '5px', transition: 'all 0.15s' }}
                        onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-surface-container-high)'; }}
                        onMouseLeave={e => { e.currentTarget.style.background = 'var(--md-surface-container)'; }}
                      >
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" /><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" /></svg>
                        {t('copyLink')}
                      </button>
                      <button type="button"
                        onClick={(e: React.MouseEvent<HTMLButtonElement>) => { e.stopPropagation(); insertIntoEditor(result.message); }}
                        style={{ padding: '6px 14px', borderRadius: '8px', border: 'none', background: 'var(--md-primary)', color: 'var(--md-on-primary)', fontSize: '12.5px', fontWeight: 600, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '5px', transition: 'all 0.15s' }}
                        onMouseEnter={e => { e.currentTarget.style.opacity = '0.9'; }}
                        onMouseLeave={e => { e.currentTarget.style.opacity = '1'; }}
                      >
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M12 20h9" /><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" /></svg>
                        {t('insertToEditor')}
                      </button>
                    </div>
                  </div>
                </div>
              ) : (
                <div style={{ background: 'var(--md-error-container)', padding: '18px 20px', borderRadius: '14px', display: 'flex', alignItems: 'center', gap: '12px' }}>
                  <IconAlertCircle size={22} style={{ color: 'var(--md-error)', flexShrink: 0 }} />
                  <div>
                    <p style={{ fontWeight: 700, fontSize: '14px', color: 'var(--md-on-error-container)' }}>{t('uploadFailed')}</p>
                    <p style={{ fontSize: '12.5px', color: 'var(--md-on-error-container)', opacity: 0.85, marginTop: '3px' }}>{esc(result.message)}</p>
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      </Card>

      <Card>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '16px', padding: '20px 22px', flexWrap: 'wrap' }}>
          <span style={{ fontSize: '15px', fontWeight: 700, color: 'var(--md-on-surface)' }}>{t('mediaListTitle')}</span>
          <div style={{ display: 'flex', gap: '10px', alignItems: 'center', flexWrap: 'wrap' }}>
            <div style={{ position: 'relative' }}>
              <IconSearch size={15} style={{ position: 'absolute', left: '10px', top: '50%', transform: 'translateY(-50%)', color: 'var(--md-outline)', pointerEvents: 'none' }} />
              <input type="text" placeholder={t('searchFilePlaceholder')} value={keyword}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => { setKeyword(e.target.value); setPage(1); }}
                style={{ paddingLeft: '32px', paddingRight: '10px', height: '34px', borderRadius: 'var(--radius-sm)', border: 'none', fontSize: '13px', outline: 'none', width: '180px', background: 'var(--md-surface-container-low)', color: 'var(--md-on-surface)' }}
                onFocus={(e: React.FocusEvent<HTMLInputElement>) => { e.currentTarget.style.outline = '2px solid var(--md-primary)'; e.currentTarget.style.outlineOffset = '-2px'; }}
                onBlur={(e: React.FocusEvent<HTMLInputElement>) => { e.currentTarget.style.outline = 'none'; }}
              />
            </div>
            <div style={{ width: '130px' }}>
              <IfSelect value={kind} onChange={(e) => { setKind(e.target.value); setPage(1); }}>
                <SelectItem value="">{t('allTypes')}</SelectItem>
                <SelectItem value="image">{t('imageType')}</SelectItem>
                <SelectItem value="audio">{t('audioType')}</SelectItem>
              </IfSelect>
            </div>
            <div style={{ width: '130px' }}>
              <MediaCategorySelect
                categories={categories}
                value={category}
                onChange={(val: string) => { setCategory(val); setPage(1); }}
                placeholder={t('allCategories')}
                includeEmpty
              />
            </div>
          </div>
        </div>

        <div style={{ padding: '0 22px 22px' }}>
          {isLoading ? (
            <div style={{ fontSize: '13.5px', color: 'var(--md-outline)', textAlign: 'center', padding: '40px' }}>{t('loading')}</div>
          ) : items.length > 0 ? (
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: '14px' }}>
              {items.map((item) => (
                <div key={item.id} style={{
                  borderRadius: '14px',
                  background: 'var(--md-surface-container-low)',
                  padding: '16px',
                  display: 'flex', flexDirection: 'column', gap: '10px',
                  transition: 'all 0.15s ease',
                }}
                  onMouseEnter={(e: React.MouseEvent<HTMLDivElement>) => (e.currentTarget as HTMLDivElement).style.background = 'var(--md-surface-container)'}
                  onMouseLeave={(e: React.MouseEvent<HTMLDivElement>) => (e.currentTarget as HTMLDivElement).style.background = 'var(--md-surface-container-low)'}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                    {item.kind === 'image' ? (
                      <img src={`${API === '' ? '' : API}${item.public_url}`} alt={item.original_name}
                        style={{ height: '60px', width: '60px', objectFit: 'cover', borderRadius: 'var(--radius-sm)', flexShrink: 0 }}
                      />
                    ) : (
                      <div style={{ width: '60px', height: '60px', borderRadius: 'var(--radius-sm)', background: 'var(--md-surface-container)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--md-outline)" strokeWidth="1.8"><path d="M9 18V5l12-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" /></svg>
                      </div>
                    )}
                    <div style={{ flex: 1, minWidth: 0 }}>
                      {renamingId === item.id ? (
                        <div style={{ display: 'flex', gap: '4px' }}>
                          <input
                            value={renameValue}
                            onChange={(e: React.ChangeEvent<HTMLInputElement>) => setRenameValue(e.target.value)}
                            onKeyDown={(e: React.KeyboardEvent<HTMLInputElement>) => {
                              if (e.key === 'Enter') void commitRename(item.id);
                              if (e.key === 'Escape') setRenamingId(null);
                            }}
                            autoFocus
                            style={{ flex: 1, fontSize: '12.5px', fontWeight: 600, color: 'var(--md-on-surface)', border: '1px solid var(--md-primary)', borderRadius: '6px', padding: '2px 6px', outline: 'none', background: 'var(--md-surface-container-lowest)', minWidth: 0 }}
                          />
                          <button type="button" onClick={() => void commitRename(item.id)}
                            style={{ padding: '2px 6px', borderRadius: '6px', border: 'none', background: 'var(--md-primary)', color: 'var(--md-on-primary)', fontSize: '11px', cursor: 'pointer', flexShrink: 0 }}>
                            <IconCheckCircle size={13} />
                          </button>
                        </div>
                      ) : (
                        <div style={{ fontWeight: 600, fontSize: '12.5px', color: 'var(--md-on-surface)', wordBreak: 'break-all', lineHeight: 1.4 }} title={esc(item.original_name)}>
                          {esc(item.original_name)}
                        </div>
                      )}
                      <div style={{ fontSize: '11.5px', color: 'var(--md-outline)', marginTop: '2px' }}>
                        {item.kind === 'image' ? t('imageLabel') : t('audioLabel')} · {Math.ceil(item.size_bytes / 1024)} KB
                      </div>
                    </div>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                    <IconFolder size={12} style={{ color: 'var(--md-outline)', flexShrink: 0 }} />
                    <select
                      value={item.category || ''}
                      onChange={(e: React.ChangeEvent<HTMLSelectElement>) => void setItemCategory(item.id, e.target.value)}
                      style={{ flex: 1, fontSize: '11.5px', color: 'var(--md-on-surface-variant)', border: 'none', borderRadius: 'var(--radius-sm)', padding: '3px 6px', background: 'var(--md-surface-container)', cursor: 'pointer', outline: 'none', minWidth: 0 }}
                    >
                      <option value="">{t('noCategory')}</option>
                      {categories.map(c => (
                        <option key={c.id} value={c.slug}>{c.name}</option>
                      ))}
                    </select>
                  </div>

                  <div style={{ display: 'flex', gap: '6px', justifyContent: 'flex-end' }}>
                    <button type="button" onClick={() => insertIntoEditor(item.public_url)}
                      title={t('insertToEditor')}
                      style={{ padding: '5px 10px', borderRadius: '7px', border: 'none', background: 'var(--md-surface-container)', color: 'var(--md-on-surface-variant)', fontSize: '11.5px', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '4px', transition: 'all 0.15s' }}
                      onMouseEnter={(e: React.MouseEvent<HTMLButtonElement>) => { (e.currentTarget as HTMLButtonElement).style.background = 'var(--md-primary-container)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--md-on-primary-container)'; }}
                      onMouseLeave={(e: React.MouseEvent<HTMLButtonElement>) => { (e.currentTarget as HTMLButtonElement).style.background = 'var(--md-surface-container)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--md-on-surface-variant)'; }}
                    >
                      <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2"><path d="M12 20h9" /><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" /></svg>
                      {t('insertAction')}
                    </button>
                    <button type="button" onClick={() => startRename(item)}
                      title={t('renameAction')}
                      style={{ padding: '5px 7px', borderRadius: '7px', border: 'none', background: 'var(--md-surface-container)', color: 'var(--md-on-surface-variant)', fontSize: '11.5px', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '4px', transition: 'all 0.15s' }}
                      onMouseEnter={(e: React.MouseEvent<HTMLButtonElement>) => { (e.currentTarget as HTMLButtonElement).style.background = 'var(--md-primary-container)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--md-on-primary-container)'; }}
                      onMouseLeave={(e: React.MouseEvent<HTMLButtonElement>) => { (e.currentTarget as HTMLButtonElement).style.background = 'var(--md-surface-container)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--md-on-surface-variant)'; }}
                    >
                      <IconEdit2 size={11} />
                    </button>
                    <button type="button" onClick={() => void deleteMedia(item.id)}
                      title={t('deletePost')}
                      style={{ padding: '5px 7px', borderRadius: '7px', border: 'none', background: 'var(--md-surface-container)', color: 'var(--md-error)', fontSize: '11.5px', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '4px', transition: 'all 0.15s' }}
                      onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = 'var(--md-error-container)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--md-on-error-container)'; }}
                      onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = 'var(--md-surface-container)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--md-error)'; }}
                    >
                      <IconTrash2 size={11} />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div style={{ textAlign: 'center', padding: '48px 0', fontSize: '13.5px', color: 'var(--md-outline)' }}>
              {t('noMediaFiles')}
            </div>
          )}
          <Pagination page={page} pages={pages} onPageChange={setPage} />
        </div>
      </Card>
    </>
  );
}
