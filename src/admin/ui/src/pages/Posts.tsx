import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { apiData, API_PREFIX, paginationPages } from '../lib/api';
import { esc } from '../lib/utils';
import { SlotRenderer } from '../lib/slots';
import type { AdminPost, Category, PaginatedResponse, Setting } from '../types';
import { PageHeader } from '../components/PageHeader';
import { Card } from '../components/Card';
import { StatsCard } from '../components/StatsCard';
import { Button } from '../components/Button';
import { StatusBadge } from '../components/StatusBadge';
import { Pagination } from '../components/Pagination';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { PostsSkeleton } from '../components/Skeleton';
import {
  IconFileText, IconCheckCircle, IconEdit, IconMessageSquare,
  IconPlus, IconPencil, IconEye, IconTrash2, IconCheck
} from '../components/Icons';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';

interface DeleteTarget { id: string; title: string; }
type ContentTypeTab = 'post' | 'page';

/* ═════════════ 样式常量 — MD3 Tonal Layering ═════════════ */
const T = {
  th: {
    padding: '12px 16px',
    textAlign: 'left' as const,
    fontSize: '11px', fontWeight: 700,
    color: 'var(--md-on-surface-variant)',
    textTransform: 'uppercase' as const,
    letterSpacing: '0.06em',
    background: 'var(--md-surface-container-low)',
  },
  td: {
    padding: '14px 16px',
    fontSize: '13px',
    color: 'var(--md-on-surface)',
    verticalAlign: 'middle',
  },
  catBadge: {
    display: 'inline-flex', alignItems: 'center',
    padding: '4px 10px', borderRadius: 'var(--radius-full)',
    fontSize: '12px', fontWeight: 600,
    background: 'var(--md-surface-container)',
    color: 'var(--md-on-surface-variant)',
    whiteSpace: 'nowrap',
  },
  iconBtn: (color: string): React.CSSProperties => ({
    display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
    width: '36px', height: '36px',
    borderRadius: 'var(--radius-full)',
    color: color,
    cursor: 'pointer', border: 'none', background: 'transparent',
    transition: 'all var(--transition-fast)',
    flexShrink: 0,
  }),
};

/* ═════════════ 工具函数 ═════════════ */
function formatDate(dateStr: string | null | undefined): string {
  return dateStr?.slice(0, 10) || '—';
}

function buildPublicUrl(siteUrl: string, slug: string, contentType: 'post' | 'page', pageRenderMode: 'editor' | 'custom_html') {
  const base = (siteUrl || window.location.origin).replace(/\/$/, '');
  const path = contentType === 'page' && pageRenderMode === 'custom_html'
    ? `/pages/${slug}`
    : `/${contentType === 'page' ? 'pages' : 'posts'}/${slug}`;
  return `${base}${path}`;
}

function PostEmptyState({ t }: { t: (key: string) => string }) {
  return (
    <div style={{ padding: '64px 16px', textAlign: 'center' }}>
      <div style={{
        width: '72px', height: '72px', margin: '0 auto 18px',
        borderRadius: '18px',
        background: 'var(--md-primary-container)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
      }}>
        <IconFileText size={30} style={{ color: 'var(--md-on-primary-container)' }} />
      </div>
      <h3 style={{ fontSize: '16px', fontWeight: 700, color: 'var(--md-on-surface)', marginBottom: '6px' }}>{t('noPosts')}</h3>
      <p style={{ fontSize: '13.5px', color: 'var(--md-outline)', maxWidth: '240px', margin: '0 auto', lineHeight: 1.65 }}>
        {t('noPostsHint')}
      </p>
    </div>
  );
}

/* ═════════════ 主组件 ═════════════ */
export default function Posts() {
  const { t, format } = useI18n();
  const toast = useToast();
  const [posts, setPosts] = useState<AdminPost[]>([]);
  const [total, setTotal] = useState(0);
  const [pages, setPages] = useState(1);
  const [page, setPage] = useState(1);
  const [commentTotal, setCommentTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const navigate = useNavigate();

  const [categories, setCategories] = useState<Category[]>([]);
  const [siteUrl, setSiteUrl] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<DeleteTarget | null>(null);

  // 内容类型 Tab：文章 / 页面
  const [contentTypeTab, setContentTypeTab] = useState<ContentTypeTab>('post');

  // 批量操作状态
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [batchDeleteTarget, setBatchDeleteTarget] = useState(false);

  const publishedCount = useMemo(() => posts.filter((p) => p.status === 'published').length, [posts]);
  const draftCount = useMemo(() => posts.filter((p) => p.status === 'draft').length, [posts]);

  const fetchPosts = useCallback(async (nextPage: number) => {
    setLoading(true);
    try {
      const params = new URLSearchParams({ page: String(nextPage), page_size: '10', content_type: contentTypeTab });
      const payload = await apiData<PaginatedResponse<AdminPost>>(`${API_PREFIX}/admin/posts?${params.toString()}`);
      setPosts(payload.items || []);
      setTotal(payload.pagination.total || 0);
      setPages(paginationPages(payload));
    } catch (error) {
      toast(error instanceof Error ? error.message : t('loadPostsFailed'), 'error');
    } finally {
      setLoading(false);
    }
  }, [toast, contentTypeTab]);

  const fetchMeta = useCallback(async () => {
    try {
      const [categoryData, commentData, settingData] = await Promise.all([
        apiData<Category[]>(`${API_PREFIX}/categories`),
        apiData<PaginatedResponse<unknown>>(`${API_PREFIX}/admin/comments?page=1&page_size=1`),
        apiData<Setting[]>(`${API_PREFIX}/admin/settings`),
      ]);
      setCategories(categoryData || []);
      setCommentTotal(commentData.pagination.total || 0);
      setSiteUrl(settingData.find((item) => item.key === 'site_url')?.value || '');
    } catch (error) {
      toast(error instanceof Error ? error.message : t('loadMetaFailed'), 'error');
    }
  }, [toast]);

  useEffect(() => { void fetchPosts(page); }, [page, fetchPosts]);
  useEffect(() => { void fetchMeta(); }, [fetchMeta]);

  // Tab 切换时重置分页和选择
  useEffect(() => { setPage(1); setSelectedIds(new Set()); }, [contentTypeTab]);
  useEffect(() => { setSelectedIds(new Set()); }, [page]);

  async function confirmDelete() {
    if (!deleteTarget) return;
    try {
      await apiData(`${API_PREFIX}/admin/posts/${deleteTarget.id}`, { method: 'DELETE' });
      toast(t('deleteSuccess'), 'success');
      await fetchPosts(page);
    } catch (error) {
      toast(error instanceof Error ? error.message : t('deleteFailed'), 'error');
    } finally {
      setDeleteTarget(null);
    }
  }

  async function handleBatchDelete() {
    if (selectedIds.size === 0) return;
    setBatchDeleteTarget(true);
  }

  async function confirmBatchDelete() {
    try {
      await Promise.all(
        [...selectedIds].map(id =>
          apiData(`${API_PREFIX}/admin/posts/${id}`, { method: 'DELETE' })
        )
      );
      toast(format('batchDeletePostsSuccess', { count: selectedIds.size }), 'success');
      setSelectedIds(new Set());
      await fetchPosts(page);
    } catch (error) {
      toast(error instanceof Error ? error.message : t('batchDeletePostsFailed'), 'error');
    } finally {
      setBatchDeleteTarget(false);
    }
  }

  function toggleSelectAll() {
    if (selectedIds.size === posts.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(posts.map(p => p.id)));
    }
  }

  function toggleSelect(id: string) {
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  if (loading && posts.length === 0) return <PostsSkeleton />;

  return (
    <>
      {/* 统计卡片 */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '16px', marginBottom: '24px' }}>
        <StatsCard icon={<IconFileText size={20} />} value={total} label={t('postsTotal')} theme="orange" />
        <StatsCard icon={<IconCheckCircle size={20} />} value={publishedCount} label={t('publishedCount')} theme="emerald" />
        <StatsCard icon={<IconEdit size={20} />} value={draftCount} label={t('draftCount')} theme="amber" />
        <StatsCard icon={<IconMessageSquare size={20} />} value={commentTotal} label={t('commentsTotal')} theme="blue" />
      </div>

      <PageHeader
        title={t('postsTitle')}
        subtitle={format('postsCount', { count: total })}
        actions={<Button onClick={() => navigate(contentTypeTab === 'post' ? '/posts/new' : '/posts/new?type=page')}><IconPlus /> {contentTypeTab === 'post' ? t('newPost') : t('newPage', '新建页面')}</Button>}
      />

      <SlotRenderer target="dashboard.widget" />

      {/* 内容类型 Tab — MD3 Segmented Button */}
      <div style={{
        display: 'inline-flex', gap: '0', marginBottom: '16px',
        background: 'var(--md-surface-container)', padding: '4px',
        borderRadius: 'var(--radius-full)',
      }}>
        <button
          onClick={() => setContentTypeTab('post')}
          style={{
            padding: '8px 20px', borderRadius: 'var(--radius-full)',
            border: 'none', cursor: 'pointer',
            fontSize: '13px', fontWeight: contentTypeTab === 'post' ? 600 : 400,
            background: contentTypeTab === 'post' ? 'var(--md-primary)' : 'transparent',
            color: contentTypeTab === 'post' ? 'var(--md-on-primary)' : 'var(--md-on-surface-variant)',
            transition: 'all var(--transition-normal)',
            display: 'flex', alignItems: 'center', gap: '6px',
          }}
        >
          <IconFileText size={14} /> 文章
        </button>
        <button
          onClick={() => setContentTypeTab('page')}
          style={{
            padding: '8px 20px', borderRadius: 'var(--radius-full)',
            border: 'none', cursor: 'pointer',
            fontSize: '13px', fontWeight: contentTypeTab === 'page' ? 600 : 400,
            background: contentTypeTab === 'page' ? 'var(--md-primary)' : 'transparent',
            color: contentTypeTab === 'page' ? 'var(--md-on-primary)' : 'var(--md-on-surface-variant)',
            transition: 'all var(--transition-normal)',
            display: 'flex', alignItems: 'center', gap: '6px',
          }}
        >
          <IconPencil size={14} /> 页面
        </button>
      </div>

      {/* 活跃 / 回收站 Tab */}
      <div style={{ display: 'flex', gap: '8px', marginBottom: '20px' }}>
        <button
          style={{
            padding: '8px 16px', fontSize: '13px', fontWeight: 600,
            color: 'var(--md-on-primary-container)',
            background: 'var(--md-primary-container)',
            borderRadius: 'var(--radius-full)', border: 'none', cursor: 'pointer',
            transition: 'all var(--transition-normal)',
          }}
        >
          {contentTypeTab === 'post' ? t('activePosts') : '活跃页面'}
        </button>
        <button
          onClick={() => navigate('/trash?tab=post')}
          style={{
            padding: '8px 16px', fontSize: '13px', fontWeight: 500,
            color: 'var(--md-on-surface-variant)',
            background: 'transparent',
            borderRadius: 'var(--radius-full)', border: 'none', cursor: 'pointer',
            transition: 'all var(--transition-normal)',
          }}
          onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-surface-container)'; }}
          onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; }}
        >
          {t('deletedItems')}
        </button>
      </div>

      <SlotRenderer target="post_list.action_bar" />

      <Card style={{ overflow: 'hidden' }}>
        {/* 批量操作栏 */}
        {selectedIds.size > 0 && (
          <div style={{
            padding: '12px 16px',
            background: 'var(--md-primary-container)',
            display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            animation: 'slideDown 0.2s ease',
          }}>
            <span style={{ fontSize: '13px', fontWeight: 600, color: 'var(--md-on-primary-container)' }}>
              {format('selectedCount', { count: selectedIds.size })}
            </span>
            <Button size="sm" variant="danger" onClick={handleBatchDelete}>
              <IconTrash2 size={14} /> {t('batchDelete')}
            </Button>
          </div>
        )}

        <div style={{ overflowX: 'auto' }}>
          <table style={{ width: '100%', minWidth: '700px', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={{ ...T.th, width: '44px', textAlign: 'center' as const }}>
                  <button
                    onClick={toggleSelectAll}
                    style={{
                      width: '18px', height: '18px',
                      borderRadius: '3px',
                      border: selectedIds.size === posts.length && posts.length > 0 ? 'none' : '2px solid var(--md-outline)',
                      background: selectedIds.size === posts.length && posts.length > 0 ? 'var(--md-primary)' : 'transparent',
                      cursor: 'pointer',
                      display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                      transition: 'all var(--transition-fast)',
                    }}
                  >
                    {selectedIds.size === posts.length && posts.length > 0 && (
                      <IconCheck size={12} color="#fff" />
                    )}
                  </button>
                </th>
                <th style={{ ...T.th }}>{t('titleLabel')}</th>
                <th style={{ ...T.th, width: '100px' }}>{t('categoryLabel')}</th>
                <th style={{ ...T.th, width: '88px' }}>{t('statusLabel')}</th>
                <th style={{ ...T.th, width: '110px' }}>{t('publishTimeLabel')}</th>
                <th style={{ ...T.th, width: '120px', textAlign: 'right' as const }}>{t('actionsLabel')}</th>
              </tr>
            </thead>
            <tbody>
              {posts.length > 0 ? posts.map((post, idx) => {
                const category = categories.find((item) => item.id === post.category_id);
                const isSelected = selectedIds.has(post.id);
                return (
                  <tr key={post.id}
                    onMouseEnter={e => { if (!isSelected) e.currentTarget.style.background = 'var(--md-surface-container-low)'; }}
                    onMouseLeave={e => { if (!isSelected) e.currentTarget.style.background = idx % 2 === 0 ? 'var(--md-surface-container-lowest)' : 'var(--md-surface-container-low)'; }}
                    style={{
                      transition: 'background var(--transition-fast)',
                      background: isSelected ? 'var(--md-primary-container)' : (idx % 2 === 0 ? 'var(--md-surface-container-lowest)' : 'var(--md-surface-container-low)'),
                    }}
                  >
                    <td style={{ ...T.td, textAlign: 'center' }}>
                      <button
                        onClick={() => toggleSelect(post.id)}
                        style={{
                          width: '18px', height: '18px',
                          borderRadius: '3px',
                          border: isSelected ? 'none' : '2px solid var(--md-outline)',
                          background: isSelected ? 'var(--md-primary)' : 'transparent',
                          cursor: 'pointer',
                          display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                          transition: 'all var(--transition-fast)',
                        }}
                      >
                        {isSelected && <IconCheck size={12} color="#fff" />}
                      </button>
                    </td>
                    <td style={{ ...T.td }}>
                      <button
                        onClick={() => navigate(`/posts/${post.id}/edit`)}
                        title={post.title}
                        style={{
                          display: 'inline-flex', alignItems: 'center', gap: '7px',
                          fontSize: '14px', fontWeight: 600, color: 'var(--md-on-surface)',
                          maxWidth: '280px', textDecoration: 'none',
                          overflow: 'hidden', background: 'none', border: 'none',
                          cursor: 'pointer', padding: 0,
                        }}
                      >
                        <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{esc(post.title)}</span>
                      </button>
                    </td>
                    <td style={{ ...T.td }}>
                      <span style={T.catBadge}>{category?.name || t('uncategorized')}</span>
                    </td>
                    <td style={{ ...T.td }}><StatusBadge status={post.status} /></td>
                    <td style={{ ...T.td, fontFamily: 'monospace', fontSize: '12.5px', color: 'var(--md-outline)' }}>
                      {formatDate(post.published_at)}
                    </td>
                    <td style={{ ...T.td }}>
                      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '4px', alignItems: 'center' }}>
                        <button type="button"
                          title={t('viewOnHomepage')}
                          style={T.iconBtn('var(--md-on-surface-variant)')}
                          onClick={() => {
                            const url = buildPublicUrl(siteUrl, post.slug, post.content_type, post.page_render_mode);
                            window.open(url, '_blank');
                          }}
                          onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-surface-container)'; }}
                          onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; }}
                        ><IconEye size={16} /></button>
                        <button type="button"
                          title={t('editPost')}
                          style={T.iconBtn('var(--md-on-surface-variant)')}
                          onClick={() => navigate(`/posts/${post.id}/edit`)}
                          onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-surface-container)'; }}
                          onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; }}
                        ><IconPencil size={16} /></button>
                        <button type="button"
                          title={t('deletePost')}
                          style={T.iconBtn('var(--md-error)')}
                          onClick={() => setDeleteTarget({ id: post.id, title: post.title })}
                          onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-error-container)'; }}
                          onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; }}
                        ><IconTrash2 size={16} /></button>
                      </div>
                    </td>
                  </tr>
                );
              }) : null}
            </tbody>
          </table>
        </div>

        {posts.length === 0 ? (
          <PostEmptyState t={t} />
        ) : (
          <div style={{
            padding: '14px 16px',
            display: 'flex', alignItems: 'center', justifyContent: 'space-between',
          }}>
            <span style={{ fontSize: '12.5px', color: 'var(--md-outline)' }}>
              第 {(page - 1) * 10 + 1}-{Math.min(page * 10, total)} 条，共 {total} 条
            </span>
            <Pagination page={page} pages={pages} onPageChange={setPage} />
          </div>
        )}
      </Card>

      {/* 单个删除确认 */}
      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={confirmDelete}
        title="删除文章"
        message={`确定要删除文章「${deleteTarget?.title || ''}」吗？此操作不可恢复。`}
        confirmText="确认删除" variant="danger"
      />

      {/* 批量删除确认 */}
      <ConfirmDialog
        open={batchDeleteTarget}
        onClose={() => setBatchDeleteTarget(false)}
        onConfirm={confirmBatchDelete}
        title="批量删除文章"
        message={`确定要删除选中的 ${selectedIds.size} 篇文章吗？此操作不可恢复。`}
        confirmText={`删除 ${selectedIds.size} 篇`} variant="danger"
      />

      <style>{`
        @keyframes slideDown {
          from { opacity: 0; transform: translateY(-8px); }
          to { opacity: 1; transform: translateY(0); }
        }
      `}</style>
    </>
  );
}
