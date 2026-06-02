import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation } from '@tanstack/react-query';
import { apiData, API_PREFIX, paginationPages, getQueryClient } from '../lib/api';
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
import { useResponsive } from '../hooks/useResponsive';

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
  const { isMobile } = useResponsive();
  const [page, setPage] = useState(1);
  const navigate = useNavigate();

  const [deleteTarget, setDeleteTarget] = useState<DeleteTarget | null>(null);

  // 内容类型 Tab：文章 / 页面
  const [contentTypeTab, setContentTypeTab] = useState<ContentTypeTab>('post');

  // 批量操作状态
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [batchDeleteTarget, setBatchDeleteTarget] = useState(false);

  // 获取文章列表
  const { data: postsPayload, isLoading } = useQuery({
    queryKey: ['posts', { page, contentTypeTab }],
    queryFn: () => {
      const params = new URLSearchParams({ page: String(page), page_size: '10', content_type: contentTypeTab });
      return apiData<PaginatedResponse<AdminPost>>(`${API_PREFIX}/admin/posts?${params.toString()}`);
    },
  });

  // 获取元数据（分类、评论数、siteUrl）
  const { data: metaData } = useQuery({
    queryKey: ['posts-meta'],
    queryFn: async () => {
      const [categoryData, commentData, settingData] = await Promise.all([
        apiData<Category[]>(`${API_PREFIX}/categories`),
        apiData<PaginatedResponse<unknown>>(`${API_PREFIX}/admin/comments?page=1&page_size=1`),
        apiData<Setting[]>(`${API_PREFIX}/admin/settings`),
      ]);
      return { categories: categoryData, commentTotal: commentData.pagination.total, siteUrl: settingData.find((item) => item.key === 'site_url')?.value || '' };
    },
  });

  const posts = postsPayload?.items ?? [];
  const total = postsPayload?.pagination.total ?? 0;
  const pages = postsPayload ? paginationPages(postsPayload) : 1;
  const categories = metaData?.categories ?? [];
  const commentTotal = metaData?.commentTotal ?? 0;
  const siteUrl = metaData?.siteUrl ?? '';

  const publishedCount = useMemo(() => posts.filter((p) => p.status === 'published').length, [posts]);
  const draftCount = useMemo(() => posts.filter((p) => p.status === 'draft').length, [posts]);

  const invalidatePosts = () => getQueryClient().invalidateQueries({ queryKey: ['posts'] });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => apiData(`${API_PREFIX}/admin/posts/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      toast(t('deleteSuccess'), 'success');
      setDeleteTarget(null);
      invalidatePosts();
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('deleteFailed'), 'error');
      setDeleteTarget(null);
    },
  });

  const batchDeleteMutation = useMutation({
    mutationFn: async (ids: string[]) => {
      await Promise.all(ids.map(id =>
        apiData(`${API_PREFIX}/admin/posts/${id}`, { method: 'DELETE' })
      ));
    },
    onSuccess: (_, ids) => {
      toast(format('batchDeletePostsSuccess', { count: ids.length }), 'success');
      setSelectedIds(new Set());
      setBatchDeleteTarget(false);
      invalidatePosts();
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('batchDeletePostsFailed'), 'error');
      setBatchDeleteTarget(false);
    },
  });

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

  if (isLoading && posts.length === 0) return <PostsSkeleton />;

  return (
    <>
      {/* 统计卡片 */}
      <div style={{ display: 'grid', gridTemplateColumns: isMobile ? 'repeat(2, 1fr)' : 'repeat(4, 1fr)', gap: '16px', marginBottom: '24px' }}>
        <StatsCard icon={<IconFileText size={20} />} value={total} label={t('postsTotal')} theme="orange" />
        <StatsCard icon={<IconCheckCircle size={20} />} value={publishedCount} label={t('publishedCount')} theme="emerald" />
        <StatsCard icon={<IconEdit size={20} />} value={draftCount} label={t('draftCount')} theme="amber" />
        <StatsCard icon={<IconMessageSquare size={20} />} value={commentTotal} label={t('commentsTotal')} theme="blue" />
      </div>

      <PageHeader
        title={t('postsTitle')}
        subtitle={format('postsCount', { count: total })}
        actions={<Button onClick={() => navigate(contentTypeTab === 'post' ? '/posts/new' : '/posts/new?type=page')}><IconPlus /> {contentTypeTab === 'post' ? t('newPost') : t('newPage')}</Button>}
      />

      <SlotRenderer target="dashboard.widget" />

      {/* 内容类型 Tab — MD3 Segmented Button */}
      <div style={{
        display: 'inline-flex', gap: '0', marginBottom: '16px',
        background: 'var(--md-surface-container)', padding: '4px',
        borderRadius: 'var(--radius-full)',
      }}>
        <button
          onClick={() => { setContentTypeTab('post'); setPage(1); setSelectedIds(new Set()); }}
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
          <IconFileText size={14} /> {t('postTab')}
        </button>
        <button
          onClick={() => { setContentTypeTab('page'); setPage(1); setSelectedIds(new Set()); }}
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
          <IconPencil size={14} /> {t('pageTab')}
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
          {contentTypeTab === 'post' ? t('activePosts') : t('activePages')}
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

      {/* 移动端：卡片列表 */}
      {isMobile ? (
        <>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
            {posts.length > 0 ? posts.map((post) => {
              const category = categories.find((item) => item.id === post.category_id);
              return (
                <div
                  key={post.id}
                  style={{
                    background: 'var(--md-surface-container)',
                    borderRadius: 'var(--radius-lg)',
                    padding: '16px',
                    cursor: 'pointer',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '8px',
                  }}
                  onClick={() => navigate(`/posts/${post.id}/edit`)}
                >
                  <h3 style={{ fontSize: '15px', fontWeight: 600, margin: 0, color: 'var(--md-on-surface)', lineHeight: 1.4 }}>
                    {esc(post.title)}
                  </h3>
                  <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap', alignItems: 'center' }}>
                    <StatusBadge status={post.status} />
                    {category && (
                      <span style={T.catBadge}>{category.name}</span>
                    )}
                    <span style={{ fontSize: '12px', color: 'var(--md-on-surface-variant)' }}>
                      {post.created_at?.slice(0, 10)}
                    </span>
                  </div>
                </div>
              );
            }) : (
              <PostEmptyState t={t} />
            )}
          </div>
          {posts.length > 0 && (
            <div style={{
              padding: '14px 0',
              display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            }}>
              <span style={{ fontSize: '12.5px', color: 'var(--md-outline)' }}>
                {format('paginationInfo', { start: (page - 1) * 10 + 1, end: Math.min(page * 10, total), total })}
              </span>
              <Pagination page={page} pages={pages} onPageChange={setPage} />
            </div>
          )}
        </>
      ) : (
        /* 桌面端：表格布局 */
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
            <Button size="sm" variant="danger" onClick={() => setBatchDeleteTarget(true)}>
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
              {format('paginationInfo', { start: (page - 1) * 10 + 1, end: Math.min(page * 10, total), total })}
            </span>
            <Pagination page={page} pages={pages} onPageChange={setPage} />
          </div>
        )}
      </Card>
      )}

      {/* 单个删除确认 */}
      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={() => deleteTarget && deleteMutation.mutate(deleteTarget.id)}
        title={t('deletePostTitle')}
        message={format('deletePostMessage', { title: deleteTarget?.title || '' })}
        confirmText={t('deleteConfirm')} variant="danger"
      />

      {/* 批量删除确认 */}
      <ConfirmDialog
        open={batchDeleteTarget}
        onClose={() => setBatchDeleteTarget(false)}
        onConfirm={() => batchDeleteMutation.mutate([...selectedIds])}
        title={t('batchDeletePostsTitle')}
        message={format('batchDeletePostsMessage', { count: selectedIds.size })}
        confirmText={format('deleteCountPostsConfirm', { count: selectedIds.size })} variant="danger"
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
