import { useMemo, useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { apiData, paginationPages, API_PREFIX, getQueryClient } from '../lib/api';
import { esc } from '../lib/utils';
import type { Comment, PaginatedResponse } from '../types';
import { PageHeader } from '../components/PageHeader';
import { Card } from '../components/Card';
import { StatsCard } from '../components/StatsCard';
import { Button } from '../components/Button';
import { StatusBadge } from '../components/StatusBadge';
import { Pagination } from '../components/Pagination';
import { EmptyState } from '../components/EmptyState';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { PostsSkeleton } from '../components/Skeleton';
import { IconMessageSquare, IconClock, IconCheckCircle, IconTrash2, IconExternalLink, IconCheck, IconBan } from '../components/Icons';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';

/* 样式 */
const TH = {
  padding: '14px 16px', textAlign: 'left' as const, fontSize: '11.5px',
  fontWeight: 700, color: 'var(--text-muted)', textTransform: 'uppercase' as const,
  letterSpacing: '0.06em', background: 'var(--bg-subtle)',
  borderBottom: '2px solid var(--border-light)',
};
const TD = {
  padding: '14px 16px', fontSize: '13px', color: 'var(--if-text)',
  borderBottom: '1px solid var(--border-light)', verticalAlign: 'middle',
};
const iconBtn: React.CSSProperties = {
  display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
  width: '32px', height: '32px', borderRadius: '8px',
  border: 'none', cursor: 'pointer', transition: 'all 0.15s ease',
};

export default function Comments() {
  const toast = useToast();
  const { t, format } = useI18n();
  const [page, setPage] = useState(1);
  const [deleteTarget, setDeleteTarget] = useState<Comment | null>(null);

  // 批量选择
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [batchDeleteOpen, setBatchDeleteOpen] = useState(false);

  const { data: payload, isLoading } = useQuery({
    queryKey: ['comments-legacy', { page }],
    queryFn: () => apiData<PaginatedResponse<Comment>>(`${API_PREFIX}/admin/comments?page=${page}&page_size=15`),
  });

  const items = payload?.items ?? [];
  const total = payload?.pagination.total ?? 0;
  const pages = payload ? paginationPages(payload) : 1;

  const invalidate = () => getQueryClient().invalidateQueries({ queryKey: ['comments-legacy'] });

  const approveMutation = useMutation({
    mutationFn: (id: string) => apiData(`${API_PREFIX}/admin/comments/${id}/approve`, { method: 'POST' }),
    onSuccess: () => { toast(t('approvedSuccess'), 'success'); invalidate(); },
    onError: (error) => toast(error instanceof Error ? error.message : t('actionFailed'), 'error'),
  });

  const rejectMutation = useMutation({
    mutationFn: (id: string) => apiData(`${API_PREFIX}/admin/comments/${id}/reject`, { method: 'POST' }),
    onSuccess: () => { toast(t('rejectedSuccess'), 'success'); invalidate(); },
    onError: (error) => toast(error instanceof Error ? error.message : t('actionFailed'), 'error'),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => apiData(`${API_PREFIX}/admin/comments/${id}`, { method: 'DELETE' }),
    onSuccess: (_, id) => {
      toast(t('deleteSuccess'), 'success');
      setSelectedIds(prev => { const next = new Set(prev); next.delete(id); return next; });
      setDeleteTarget(null);
      invalidate();
    },
    onError: (error) => { toast(error instanceof Error ? error.message : t('deleteFailed'), 'error'); setDeleteTarget(null); },
  });

  const batchApproveMutation = useMutation({
    mutationFn: async (pendingIds: string[]) => {
      await Promise.all(pendingIds.map(id => apiData(`${API_PREFIX}/admin/comments/${id}/approve`, { method: 'POST' })));
    },
    onSuccess: (_, pendingIds) => {
      toast(format('batchApproveSuccess', { count: pendingIds.length }), 'success');
      setSelectedIds(new Set());
      invalidate();
    },
    onError: (error) => toast(error instanceof Error ? error.message : t('batchActionFailed'), 'error'),
  });

  const batchRejectMutation = useMutation({
    mutationFn: async (pendingIds: string[]) => {
      await Promise.all(pendingIds.map(id => apiData(`${API_PREFIX}/admin/comments/${id}/reject`, { method: 'POST' })));
    },
    onSuccess: (_, pendingIds) => {
      toast(format('batchRejectSuccess', { count: pendingIds.length }), 'success');
      setSelectedIds(new Set());
      invalidate();
    },
    onError: (error) => toast(error instanceof Error ? error.message : t('batchActionFailed'), 'error'),
  });

  const batchDeleteMutation = useMutation({
    mutationFn: async (ids: string[]) => {
      await Promise.all(ids.map(id => apiData(`${API_PREFIX}/admin/comments/${id}`, { method: 'DELETE' })));
    },
    onSuccess: (_, ids) => {
      toast(format('batchDeleteCommentsSuccess', { count: ids.length }), 'success');
      setSelectedIds(new Set());
      setBatchDeleteOpen(false);
      invalidate();
    },
    onError: (error) => { toast(error instanceof Error ? error.message : t('batchDeleteCommentsFailed'), 'error'); setBatchDeleteOpen(false); },
  });

  function handleBatchApprove() {
    const pendingIds = [...selectedIds].filter(id => {
      const c = items.find(i => i.id === id);
      return c && c.status === 'pending';
    });
    if (pendingIds.length === 0) { toast(t('noPendingComments'), 'info'); return; }
    batchApproveMutation.mutate(pendingIds);
  }

  function handleBatchReject() {
    const pendingIds = [...selectedIds].filter(id => {
      const c = items.find(i => i.id === id);
      return c && c.status === 'pending';
    });
    if (pendingIds.length === 0) { toast(t('noPendingComments'), 'info'); return; }
    batchRejectMutation.mutate(pendingIds);
  }

  function toggleSelect(id: string) {
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function toggleSelectAll() {
    if (selectedIds.size === items.length) setSelectedIds(new Set());
    else setSelectedIds(new Set(items.map(c => c.id)));
  }

  const pendingCount = useMemo(() => items.filter((i) => i.status === 'pending').length, [items]);

  if (isLoading && items.length === 0) return <PostsSkeleton />;

  return (
    <>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px', marginBottom: '24px' }}>
        <StatsCard icon={<IconMessageSquare size={20} />} value={total} label={t('commentsTotal')} theme="blue" />
        <StatsCard icon={<IconClock size={20} />} value={pendingCount} label={t('pendingReview')} theme="amber" />
      </div>

      <PageHeader
        title={t('commentsTitle')}
        subtitle={format('commentsCount', { count: total })}
        actions={
          selectedIds.size > 0 ? (
            <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
              <Button variant="success" size="sm" onClick={handleBatchApprove}>
                <IconCheckCircle size={14} /> {format('batchApprove', { count: [...selectedIds].filter(id => items.find(i => i.id === id)?.status === 'pending').length })}
              </Button>
              <Button variant="ghost" size="sm" onClick={handleBatchReject}>
                <IconBan size={14} /> {t('batchReject')}
              </Button>
              <Button variant="danger" size="sm" onClick={() => setBatchDeleteOpen(true)}>
                <IconTrash2 size={14} /> {format('batchDeleteComments', { count: selectedIds.size })}
              </Button>
            </div>
          ) : undefined
        }
      />

      <Card style={{ overflow: 'hidden' }}>
        <div style={{ overflowX: 'auto' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead><tr>
              <th style={{ ...TH, width: '44px', textAlign: 'center' as const }}>
                <button
                  onClick={toggleSelectAll}
                  style={{
                    width: '18px', height: '18px', borderRadius: '4px',
                    border: `1.5px solid ${selectedIds.size === items.length && items.length > 0 ? 'var(--primary-500)' : 'var(--border-default)'}`,
                    background: selectedIds.size === items.length && items.length > 0 ? 'var(--primary-500)' : 'transparent',
                    cursor: 'pointer', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                    transition: 'all 0.15s ease',
                  }}
                >
                  {selectedIds.size === items.length && items.length > 0 && <IconCheck size={12} color="#fff" />}
                </button>
              </th>
              <th style={TH}>{t('userLabel')}</th>
              <th style={TH}>{t('contentLabel')}</th>
              <th style={TH}>{t('postLabel')}</th>
              <th style={{ ...TH, width: '88px' }}>{t('statusLabel')}</th>
              <th style={{ ...TH, width: '140px' }}>{t('timeLabel')}</th>
              <th style={{ ...TH, width: '120px', textAlign: 'right' as const }}>{t('actionsLabel')}</th>
            </tr></thead>
            <tbody>
              {items.length > 0 ? items.map((cmt) => {
                const isSelected = selectedIds.has(cmt.id);
                const isPending = cmt.status === 'pending';
                return (
                  <tr key={cmt.id}
                    onMouseEnter={e => { if (!isSelected) e.currentTarget.style.background = isPending ? '#fffbeb' : 'var(--primary-50)'; }}
                    onMouseLeave={e => { if (!isSelected) e.currentTarget.style.background = 'transparent'; }}
                    style={{ transition: 'background 0.12s ease', background: isSelected ? 'var(--primary-50)' : 'transparent' }}
                  >
                    <td style={{ ...TD, textAlign: 'center' }}>
                      <button
                        onClick={() => toggleSelect(cmt.id)}
                        style={{
                          width: '18px', height: '18px', borderRadius: '4px',
                          border: `1.5px solid ${isSelected ? 'var(--primary-500)' : 'var(--border-default)'}`,
                          background: isSelected ? 'var(--primary-500)' : 'transparent',
                          cursor: 'pointer', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                          transition: 'all 0.15s ease',
                        }}
                      >
                        {isSelected && <IconCheck size={12} color="#fff" />}
                      </button>
                    </td>
                    <td style={{ ...TD, width: '130px' }}>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
                        <span style={{ fontWeight: 600 }}>{esc(cmt.display_name)}</span>
                        <span style={{ fontSize: '12px', color: 'var(--text-muted)' }}>@{esc(cmt.username)}</span>
                      </div>
                    </td>
                    <td style={{ ...TD, maxWidth: '220px' }}>
                      <span style={{
                        display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical' as const,
                        overflow: 'hidden', lineHeight: 1.55, color: 'var(--text-secondary)',
                      }}>{esc(cmt.content)}</span>
                    </td>
                    <td style={{ ...TD, maxWidth: '160px' }}>
                      {cmt.post_title ? (
                        <a href={`/${cmt.post_content_type === 'page' ? 'pages' : 'posts'}/${cmt.post_slug || ''}`} target="_blank" rel="noreferrer"
                          title={cmt.post_title} style={{
                            fontSize: '12px', color: 'var(--primary-500)', textDecoration: 'none',
                            display: 'inline-flex', alignItems: 'center', gap: '4px',
                            overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                          }}
                          onMouseEnter={e => e.currentTarget.style.textDecoration = 'underline'}
                          onMouseLeave={e => e.currentTarget.style.textDecoration = 'none'}
                        ><IconExternalLink size={11} />{esc(cmt.post_title)}</a>
                      ) : (
                        <span style={{ fontSize: '12px', color: 'var(--text-muted)', textDecoration: 'line-through' }}>{t('deletedPost')}</span>
                      )}
                    </td>
                    <td style={TD}><StatusBadge status={cmt.status} /></td>
                    <td style={{ ...TD, fontFamily: 'monospace', fontSize: '12px', whiteSpace: 'nowrap', color: 'var(--text-muted)' }}>
                      {cmt.created_at?.slice(0, 16).replace('T', ' ') || '—'}
                    </td>
                    <td style={TD}>
                      <div style={{ display: 'flex', gap: '4px', justifyContent: 'flex-end', alignItems: 'center' }}>
                        {cmt.status === 'pending' && (
                          <>
                            <button
                              title={t('approveAction')}
                              style={{ ...iconBtn, background: 'transparent', color: '#10b981' }}
                              onClick={() => approveMutation.mutate(cmt.id)}
                              onMouseEnter={e => { e.currentTarget.style.background = '#ecfdf5'; e.currentTarget.style.color = '#059669'; }}
                              onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#10b981'; }}
                            ><IconCheckCircle size={16} /></button>
                            <button
                              title={t('rejectAction')}
                              style={{ ...iconBtn, background: 'transparent', color: '#f59e0b' }}
                              onClick={() => rejectMutation.mutate(cmt.id)}
                              onMouseEnter={e => { e.currentTarget.style.background = '#fffbeb'; e.currentTarget.style.color = '#d97706'; }}
                              onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#f59e0b'; }}
                            ><IconBan size={16} /></button>
                          </>
                        )}
                        <button
                          title={t('delete')}
                          style={{ ...iconBtn, background: 'transparent', color: '#ef4444' }}
                          onClick={() => setDeleteTarget(cmt)}
                          onMouseEnter={e => { e.currentTarget.style.background = '#fef2f2'; e.currentTarget.style.color = '#dc2626'; }}
                          onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#ef4444'; }}
                        ><IconTrash2 size={16} /></button>
                      </div>
                    </td>
                  </tr>
                );
              }) : (
                <tr><td colSpan={7}>
                  <EmptyState icon={<IconMessageSquare size={28} />} message={t('noComments')} />
                </td></tr>
              )}
            </tbody>
          </table>
        </div>
        <Pagination page={page} pages={pages} onPageChange={setPage} />
      </Card>

      <ConfirmDialog open={!!deleteTarget} onClose={() => setDeleteTarget(null)} onConfirm={() => deleteTarget && deleteMutation.mutate(deleteTarget.id)}
        title={t('deleteCommentTitle')} message={format('deleteCommentSimpleMessage', { name: deleteTarget?.display_name || '' })} variant="danger" confirmText={t('delete')} />

      <ConfirmDialog open={batchDeleteOpen} onClose={() => setBatchDeleteOpen(false)} onConfirm={() => batchDeleteMutation.mutate([...selectedIds])}
        title={t('batchDeleteCommentTitle')} message={format('batchDeleteCommentMessage', { count: selectedIds.size })}
        variant="danger" confirmText={format('deleteCommentsConfirm', { count: selectedIds.size })} />
    </>
  );
}
