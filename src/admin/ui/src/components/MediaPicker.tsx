/**
 * MediaPicker - 从媒体库选择文件并插入编辑器的弹窗组件
 * 通过 window.colophonInsertMarkdown(text) 插入到当前编辑器
 *
 * 支持两个 tab：媒体库（浏览已有文件） / 上传（直接上传新文件）
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { apiData, API_PREFIX } from '../lib/api';
import type { MediaItem, PaginatedResponse } from '../types';
import { Modal } from './Modal';
import { IconSearch, IconFolder, IconUpload, Spinner, IconAlertCircle } from './Icons';
import { paginationPages } from '../lib/api';
import { uploadMedia, type UploadProgress } from '../lib/media';
import { useI18n } from '../i18n';
import { useToast } from '../contexts/ToastContext';

type ActiveTab = 'browse' | 'upload';

const CATEGORIES = ['封面图', '文章配图', '头像/头像', '音频文件', '其他'];

const HIGHLIGHT_DURATION_MS = 3000;

interface Props {
  open: boolean;
  onClose: () => void;
}

export function MediaPicker({ open, onClose }: Props) {
  const { t } = useI18n();
  const toast = useToast();
  const fileInputRef = useRef<HTMLInputElement>(null);

  // ── tab 状态 ──
  const [activeTab, setActiveTab] = useState<ActiveTab>('browse');

  // ── 浏览 tab 状态 ──
  const [items, setItems] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [keyword, setKeyword] = useState('');
  const [kind, setKind] = useState('');
  const [category, setCategory] = useState('');
  const [page, setPage] = useState(1);
  const [pages, setPages] = useState(1);
  const [newlyUploadedId, setNewlyUploadedId] = useState<string | null>(null);

  // ── 上传 tab 状态 ──
  const [uploadingFile, setUploadingFile] = useState<File | null>(null);
  const [isUploading, setIsUploading] = useState(false);
  const [uploadProgress, setUploadProgress] = useState<UploadProgress | null>(null);

  // ── 高亮自动清除 ──
  useEffect(() => {
    if (!newlyUploadedId) return;
    const timer = setTimeout(() => setNewlyUploadedId(null), HIGHLIGHT_DURATION_MS);
    return () => clearTimeout(timer);
  }, [newlyUploadedId]);

  // ── 重置弹窗状态 ──
  useEffect(() => {
    if (!open) return;
    setActiveTab('browse');
    setUploadingFile(null);
    setIsUploading(false);
    setNewlyUploadedId(null);
  }, [open]);

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
    const originalUrl = item.public_url;
    const isImage = /\.(jpg|jpeg|png|gif|webp|svg)$/i.test(item.original_name);
    const markdown = isImage
      ? `\n![${item.original_name}](${originalUrl})\n`
      : `\n[${item.original_name}](${originalUrl})\n`;
    const fn = (window as any).colophonInsertMarkdown;
    if (fn) fn(markdown);
    else navigator.clipboard.writeText(markdown);
    onClose();
  }

  // ── 上传逻辑 ──
  function handleFileSelect(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    setUploadingFile(file);
    void doUpload(file);
  }

  async function doUpload(file: File) {
    setIsUploading(true);
    setUploadProgress(null);
    try {
      const uploaded = await uploadMedia(file, (progress) => {
        setUploadProgress(progress);
      });
      toast(t('uploadSuccess'), 'success');
      setNewlyUploadedId(uploaded.id);
      // 刷新列表，切到第一页
      await fetchItems(keyword, kind, category, 1);
      setPage(1);
      setActiveTab('browse');
      toast(t('switchingToLibrary'), 'info');
    } catch (error) {
      const msg = error instanceof Error ? error.message : t('uploadFailed');
      toast(msg, 'error');
      setUploadingFile(null);
    } finally {
      setIsUploading(false);
      setUploadProgress(null);
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  }

  function triggerFileSelect() {
    fileInputRef.current?.click();
  }

  // ── tab 标题 ──
  const modalTitle = activeTab === 'upload' ? t('uploadMediaTitle') : t('selectMedia');

  return (
    <Modal open={open} onClose={onClose} title={modalTitle} width="860px">
      {/* ── Tab 切换栏 ── */}
      <div style={{ display: 'flex', marginBottom: '16px', borderBottom: '1px solid var(--md-outline-variant)' }}>
        <button
          onClick={() => setActiveTab('browse')}
          style={{
            padding: '10px 20px',
            border: 'none',
            background: 'none',
            borderRadius: 0,
            fontWeight: activeTab === 'browse' ? 600 : 400,
            color: activeTab === 'browse' ? 'var(--md-primary)' : 'var(--md-on-surface-variant)',
            borderBottom: activeTab === 'browse' ? '2px solid var(--md-primary)' : '2px solid transparent',
            marginBottom: '-1px',
            cursor: 'pointer',
            fontSize: '13.5px',
            transition: 'all 0.15s ease',
          }}
        >
          {t('mediaLibraryTab')}
        </button>
        <button
          onClick={() => setActiveTab('upload')}
          style={{
            padding: '10px 20px',
            border: 'none',
            background: 'none',
            borderRadius: 0,
            fontWeight: activeTab === 'upload' ? 600 : 400,
            color: activeTab === 'upload' ? 'var(--md-primary)' : 'var(--md-on-surface-variant)',
            borderBottom: activeTab === 'upload' ? '2px solid var(--md-primary)' : '2px solid transparent',
            marginBottom: '-1px',
            cursor: 'pointer',
            fontSize: '13.5px',
            transition: 'all 0.15s ease',
          }}
        >
          {t('uploadTab')}
        </button>
      </div>

      {/* ── 浏览 tab ── */}
      {activeTab === 'browse' && (
        <>
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
                {items.map(item => {
                  const isJustUploaded = item.id === newlyUploadedId;
                  return (
                    <div key={item.id} onClick={() => insert(item)}
                      title={`${item.original_name}\n${t('clickToInsert')}`}
                      style={{
                        borderRadius: 'var(--radius-md)',
                        border: 'none',
                        padding: '10px',
                        cursor: 'pointer',
                        display: 'flex',
                        flexDirection: 'column',
                        alignItems: 'center',
                        gap: '8px',
                        transition: 'all 0.3s ease',
                        background: isJustUploaded ? 'var(--md-primary-container)' : 'var(--md-surface-container-low)',
                        outline: isJustUploaded ? '2px solid var(--md-primary)' : 'none',
                        outlineOffset: '-2px',
                        transform: isJustUploaded ? 'scale(1.02)' : 'scale(1)',
                      }}
                      onMouseEnter={e => {
                        const el = e.currentTarget as HTMLDivElement;
                        if (!isJustUploaded) el.style.background = 'var(--md-surface-container)';
                      }}
                      onMouseLeave={e => {
                        const el = e.currentTarget as HTMLDivElement;
                        if (!isJustUploaded) el.style.background = 'var(--md-surface-container-low)';
                      }}
                    >
                      {item.kind === 'image' ? (
                        (() => {
                          const url = item.public_url;
                          return (
                            <img src={url} alt={item.original_name}
                              style={{ width: '80px', height: '80px', objectFit: 'cover', borderRadius: 'var(--radius-sm)', border: 'none' }}
                            />
                          );
                        })()
                      ) : (
                        <div style={{ width: '80px', height: '80px', borderRadius: 'var(--radius-sm)', background: 'var(--md-surface-container-lowest)', display: 'flex', alignItems: 'center', justifyContent: 'center', border: 'none' }}>
                          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="var(--md-outline)" strokeWidth="1.6"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
                        </div>
                      )}
                      <span style={{ fontSize: '11px', color: isJustUploaded ? 'var(--md-on-primary-container)' : 'var(--md-on-surface-variant)', textAlign: 'center', wordBreak: 'break-all', lineHeight: 1.3, maxWidth: '100%', overflow: 'hidden', display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical' as const,
                        fontWeight: isJustUploaded ? 600 : 400,
                      }}>
                        {item.original_name}
                      </span>
                    </div>
                  );
                })}
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
        </>
      )}

      {/* ── 上传 tab ── */}
      {activeTab === 'upload' && (
        <div>
          {/* 拖拽区域 */}
          <div
            onClick={() => { if (!isUploading) triggerFileSelect(); }}
            style={{
              border: '2px dashed var(--md-outline-variant)',
              borderRadius: '14px',
              padding: '48px 24px',
              textAlign: 'center',
              cursor: isUploading ? 'default' : 'pointer',
              transition: 'all 0.2s ease',
              background: isUploading ? 'var(--md-surface-container-low)' : 'transparent',
              opacity: isUploading ? 0.6 : 1,
            }}
            onMouseEnter={e => {
              if (!isUploading) {
                e.currentTarget.style.borderColor = 'var(--md-primary)';
                e.currentTarget.style.background = 'var(--md-primary-container)';
              }
            }}
            onMouseLeave={e => {
              if (!isUploading) {
                e.currentTarget.style.borderColor = 'var(--md-outline-variant)';
                e.currentTarget.style.background = 'transparent';
              }
            }}
          >
            <div style={{
              width: '56px', height: '56px', margin: '0 auto 16px',
              borderRadius: '14px', display: 'flex', alignItems: 'center',
              justifyContent: 'center',
              background: 'var(--md-surface-container)',
            }}>
              {isUploading ? (
                <Spinner size={28} />
              ) : (
                <IconUpload size={28} style={{ color: 'var(--md-outline)' }} />
              )}
            </div>
            <p style={{ fontSize: '15px', fontWeight: 600, color: 'var(--md-on-surface)', marginBottom: '4px' }}>
              {isUploading ? t('uploading') : t('uploadPickerHint')}
            </p>
            <p style={{ fontSize: '12.5px', color: 'var(--md-outline)' }}>
              {t('uploadPickerFormats')}
            </p>
          </div>

          <input
            ref={fileInputRef}
            type="file"
            hidden
            accept="image/*"
            onChange={handleFileSelect}
            disabled={isUploading}
          />

          {/* 手动选择按钮 */}
          <div style={{ textAlign: 'center', marginTop: '16px' }}>
            <button
              type="button"
              onClick={triggerFileSelect}
              disabled={isUploading}
              style={{
                padding: '8px 24px',
                borderRadius: 'var(--radius-md)',
                border: 'none',
                background: isUploading ? 'var(--md-surface-container-low)' : 'var(--md-primary)',
                color: isUploading ? 'var(--md-outline)' : 'var(--md-on-primary)',
                fontSize: '13.5px',
                fontWeight: 600,
                cursor: isUploading ? 'not-allowed' : 'pointer',
                transition: 'all 0.15s ease',
              }}
              onMouseEnter={e => {
                if (!isUploading) e.currentTarget.style.opacity = '0.9';
              }}
              onMouseLeave={e => {
                if (!isUploading) e.currentTarget.style.opacity = '1';
              }}
            >
              {t('chooseFile')}
            </button>
          </div>

          {/* 上传中 / 进度状态 */}
          {isUploading && uploadingFile && (
            uploadProgress ? (
              <div style={{
                width: '100%',
                marginTop: 12,
              }}>
                <div style={{
                  display: 'flex', justifyContent: 'space-between',
                  fontSize: 12, color: 'var(--md-on-surface-variant)',
                  marginBottom: 4,
                }}>
                  <span>{uploadingFile.name}</span>
                  <span>{uploadProgress.percentage}%</span>
                </div>
                <div style={{
                  width: '100%', height: 4,
                  background: 'var(--md-surface-container-highest)',
                  borderRadius: 2, overflow: 'hidden',
                }}>
                  <div style={{
                    width: `${uploadProgress.percentage}%`,
                    height: '100%',
                    background: 'var(--md-primary)',
                    borderRadius: 2,
                    transition: 'width 0.15s ease',
                  }} />
                </div>
              </div>
            ) : (
              <div style={{
                marginTop: '20px',
                padding: '14px 18px',
                borderRadius: '12px',
                background: 'var(--md-surface-container)',
                display: 'flex',
                alignItems: 'center',
                gap: '12px',
              }}>
                <Spinner size={18} />
                <span style={{ fontSize: '13px', color: 'var(--md-on-surface-variant)', wordBreak: 'break-all' }}>
                  {uploadingFile.name}
                </span>
              </div>
            )
          )}

          {/* 上传失败状态 */}
          {!isUploading && uploadingFile && (
            <div style={{
              marginTop: '20px',
              padding: '14px 18px',
              borderRadius: '12px',
              background: 'var(--md-error-container)',
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
            }}>
              <IconAlertCircle size={18} style={{ color: 'var(--md-error)', flexShrink: 0 }} />
              <span style={{ fontSize: '13px', color: 'var(--md-on-error-container)' }}>
                {t('uploadFailed')}
              </span>
            </div>
          )}
        </div>
      )}
    </Modal>
  );
}
