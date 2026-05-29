import { useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useQuery, useMutation } from '@tanstack/react-query';
import { listTrash, restoreTrashItem, purgeTrashItem, purgeExpiredTrash } from '../lib/api';
import { getQueryClient } from '../lib/api';
import type { TrashItem } from '../types';
import { PageHeader } from '../components/PageHeader';
import { Card } from '../components/Card';
import { Button } from '../components/Button';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { EmptyState } from '../components/EmptyState';
import { CardTableSkeleton } from '../components/Skeleton';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';
import {
  IconTrash2, IconRefreshCw, IconFileText, IconFolderOpen,
  IconTag, IconImage, IconFolder, IconCheck, IconMessageSquare
} from '../components/Icons';

/* ═════════════ 类型标签配置 ═════════════ */
const TYPE_CONFIG: Record<string, { labelKey: string; icon: React.ReactNode; color: string }> = {
  post: { labelKey: 'tabPost', icon: <IconFileText size={14} />, color: '#3b82f6' },
  category: { labelKey: 'tabCategory', icon: <IconFolderOpen size={14} />, color: '#8b5cf6' },
  tag: { labelKey: 'tabTag', icon: <IconTag size={14} />, color: '#10b981' },
  media: { labelKey: 'tabMedia', icon: <IconImage size={14} />, color: '#f59e0b' },
  media_category: { labelKey: 'tabMediaCategory', icon: <IconFolder size={14} />, color: '#ec4899' },
  comment: { labelKey: 'tabComment', icon: <IconMessageSquare size={14} />, color: '#6366f1' },
};

const TABS = [
  { key: '', labelKey: 'tabAll' },
  { key: 'post', labelKey: 'tabPost' },
  { key: 'category', labelKey: 'tabCategory' },
  { key: 'tag', labelKey: 'tabTag' },
  { key: 'media', labelKey: 'tabMedia' },
  { key: 'media_category', labelKey: 'tabMediaCategory' },
  { key: 'comment', labelKey: 'tabComment' },
];

/* ═════════════ 样式 ═════════════ */
const TH: React.CSSProperties = {
  padding: '12px 16px',
  textAlign: 'left',
  fontSize: '11px',
  fontWeight: 700,
  color: 'var(--md-outline)',
  textTransform: 'uppercase',
  letterSpacing: '0.06em',
  background: 'var(--md-surface-container)',
};

const TD: React.CSSProperties = {
  padding: '14px 16px',
  fontSize: '13px',
  color: 'var(--md-on-surface)',
  verticalAlign: 'middle',
};

const iconBtn = (color: string): React.CSSProperties => ({
  display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
  width: '32px', height: '32px', borderRadius: '8px',
  color, cursor: 'pointer', border: 'none', background: 'transparent',
  transition: 'all 0.15s ease', flexShrink: 0,
});

/* ═════════════ 工具函数 ═════════════ */
function formatDeletedAt(dateStr: string): string {
  if (!dateStr) return '-';
  return dateStr.replace('T', ' ').slice(0, 16);
}

function expiresLabel(days: number, format: (key: string, params?: Record<string, string | number>) => string, t: (key: string) => string): { text: string; color: string } {
  if (days <= 0) return { text: t('expired'), color: '#ef4444' };
  if (days <= 3) return { text: format('daysUntilDelete', { count: days }), color: '#ef4444' };
  if (days <= 7) return { text: format('daysUntilDelete', { count: days }), color: '#f59e0b' };
  return { text: format('daysUntilDelete', { count: days }), color: 'var(--md-outline)' };
}

/* ═════════════ 预览面板 ═════════════ */
function PreviewPanel({ item, onClose, t, format }: { item: TrashItem; onClose: () => void; t: (key: string) => string; format: (key: string, params?: Record<string, string | number>) => string; }) {
  const config = TYPE_CONFIG[item.item_type];
  return (
    <div style={{
      position: 'fixed', top: 0, right: 0, bottom: 0, width: '400px',
      background: 'var(--md-surface-container-lowest)', boxShadow: 'var(--elevation-2)',
      zIndex: 1000, display: 'flex', flexDirection: 'column',
      animation: 'slideInRight 0.25s ease',
    }}>
      <div style={{
        padding: '20px 24px',
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        background: 'var(--md-surface-container)',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
          <span style={{
            width: '32px', height: '32px', borderRadius: '10px',
            background: `${config?.color || '#999'}18`,
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            color: config?.color || '#999',
          }}>
            {config?.icon}
          </span>
          <div>
            <div style={{ fontSize: '15px', fontWeight: 700, color: 'var(--md-on-surface)' }}>
              {t('previewTitle')}
            </div>
            <span style={{
              fontSize: '11px', fontWeight: 600,
              padding: '2px 8px', borderRadius: '999px',
              background: `${config?.color || '#999'}18`,
              color: config?.color || '#999',
            }}>
              {config?.labelKey ? t(config.labelKey) : item.item_type}
            </span>
          </div>
        </div>
        <button
          onClick={onClose}
          style={{
            border: 'none', background: 'var(--md-surface-container-high)', cursor: 'pointer',
            width: '28px', height: '28px', borderRadius: '8px',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            color: 'var(--md-on-surface-variant)', fontSize: '16px',
            transition: 'all 0.15s ease',
          }}
          onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-surface-container-highest)'; }}
          onMouseLeave={e => { e.currentTarget.style.background = 'var(--md-surface-container-high)'; }}
        >✕</button>
      </div>
      <div style={{ flex: 1, overflow: 'auto', padding: '24px' }}>
        <div style={{ marginBottom: '20px' }}>
          <div style={{ fontSize: '11px', fontWeight: 700, color: 'var(--md-outline)', textTransform: 'uppercase', letterSpacing: '0.06em', marginBottom: '6px' }}>
            {item.item_type === 'post' ? t('previewPostTitle') : item.item_type === 'comment' ? t('previewContent') : t('previewName')}
          </div>
          <div style={{ fontSize: '16px', fontWeight: 600, color: 'var(--md-on-surface)', lineHeight: 1.5 }}>
            {item.name}
          </div>
        </div>
        {item.subtitle && (
          <div style={{ marginBottom: '20px' }}>
            <div style={{ fontSize: '11px', fontWeight: 700, color: 'var(--md-outline)', textTransform: 'uppercase', letterSpacing: '0.06em', marginBottom: '6px' }}>
              {item.item_type === 'media' ? t('mimeTypeLabel') : t('slugText')}
            </div>
            <div style={{
              fontSize: '13px', color: 'var(--md-on-surface-variant)',
              fontFamily: 'monospace', background: 'var(--md-surface-container)',
              padding: '8px 12px', borderRadius: '8px',
            }}>
              {item.subtitle}
            </div>
          </div>
        )}
        <div style={{ marginBottom: '20px' }}>
          <div style={{ fontSize: '11px', fontWeight: 700, color: 'var(--md-outline)', textTransform: 'uppercase', letterSpacing: '0.06em', marginBottom: '6px' }}>
            {t('deletedAtLabel')}
          </div>
          <div style={{ fontSize: '13px', color: 'var(--md-on-surface-variant)' }}>
            {formatDeletedAt(item.deleted_at)}
          </div>
        </div>
        <div>
          <div style={{ fontSize: '11px', fontWeight: 700, color: 'var(--md-outline)', textTransform: 'uppercase', letterSpacing: '0.06em', marginBottom: '6px' }}>
            {t('cleanupCountdown')}
          </div>
          {(() => {
            const exp = expiresLabel(item.expires_in_days, format, t);
            return (
              <div style={{
                fontSize: '14px', fontWeight: 600, color: exp.color,
                display: 'flex', alignItems: 'center', gap: '6px',
              }}>
                <span style={{
                  width: '8px', height: '8px', borderRadius: '50%',
                  background: exp.color, display: 'inline-block',
                  animation: item.expires_in_days <= 3 ? 'pulse 1.5s infinite' : 'none',
                }} />
                {exp.text}
              </div>
            );
          })()}
        </div>
      </div>
    </div>
  );
}

/* ═════════════ 主组件 ═════════════ */
export default function RecycleBin() {
  const toast = useToast();
  const { t, format } = useI18n();
  const [searchParams, setSearchParams] = useSearchParams();
  const activeTab = searchParams.get('tab') || '';
  const setActiveTab = (tab: string) => {
    setSearchParams(tab ? { tab } : {});
  };
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [previewItem, setPreviewItem] = useState<TrashItem | null>(null);
  const [restoreTarget, setRestoreTarget] = useState<TrashItem | null>(null);
  const [purgeTarget, setPurgeTarget] = useState<TrashItem | null>(null);
  const [batchAction, setBatchAction] = useState<'restore' | 'purge' | null>(null);

  const { data: items = [], isLoading } = useQuery({
    queryKey: ['trash', { tab: activeTab }],
    queryFn: () => listTrash(activeTab || undefined),
  });

  const invalidate = () => getQueryClient().invalidateQueries({ queryKey: ['trash'] });

  const restoreMutation = useMutation({
    mutationFn: (item: TrashItem) => restoreTrashItem(item.item_type, item.id),
    onSuccess: (_, item) => {
      toast(format('restoreSuccess', { name: item.name }), 'success');
      setRestoreTarget(null);
      setPreviewItem(null);
      invalidate();
    },
    onError: (error) => toast(error instanceof Error ? error.message : t('actionFailed'), 'error'),
  });

  const purgeMutation = useMutation({
    mutationFn: (item: TrashItem) => purgeTrashItem(item.item_type, item.id),
    onSuccess: (_, item) => {
      toast(format('purgeSuccess', { name: item.name }), 'success');
      setPurgeTarget(null);
      setPreviewItem(null);
      invalidate();
    },
    onError: (error) => toast(error instanceof Error ? error.message : t('deleteFailed'), 'error'),
  });

  const batchMutation = useMutation({
    mutationFn: async ({ action, items }: { action: 'restore' | 'purge'; items: TrashItem[] }) => {
      if (action === 'restore') {
        await Promise.all(items.map(i => restoreTrashItem(i.item_type, i.id)));
      } else {
        await Promise.all(items.map(i => purgeTrashItem(i.item_type, i.id)));
      }
    },
    onSuccess: (_, { action, items: batchItems }) => {
      toast(
        action === 'restore'
          ? format('recycleBatchRestoreSuccess', { count: batchItems.length })
          : format('recycleBatchPurgeSuccess', { count: batchItems.length }),
        'success'
      );
      setSelectedIds(new Set());
      setBatchAction(null);
      invalidate();
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('recycleBatchActionFailed'), 'error');
      setBatchAction(null);
    },
  });

  const purgeExpiredMutation = useMutation({
    mutationFn: () => purgeExpiredTrash(),
    onSuccess: () => { toast(t('purgeExpiredSuccess'), 'success'); invalidate(); },
    onError: (error) => toast(error instanceof Error ? error.message : t('purgeExpiredFailed'), 'error'),
  });

  const tabCounts = useMemo(() => {
    const counts: Record<string, number> = { '': items.length };
    for (const item of items) {
      counts[item.item_type] = (counts[item.item_type] || 0) + 1;
    }
    return counts;
  }, [items]);

  const filteredItems = items;

  function handleBatchAction() {
    if (!batchAction || selectedIds.size === 0) return;
    const selected = filteredItems.filter(i => selectedIds.has(i.id));
    batchMutation.mutate({ action: batchAction, items: selected });
  }

  function toggleSelect(id: string) {
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }

  function toggleSelectAll() {
    if (selectedIds.size === filteredItems.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(filteredItems.map(i => i.id)));
    }
  }

  if (isLoading && items.length === 0) return <CardTableSkeleton cols={5} rows={4} />;

  return (
    <>
      <PageHeader
        title={t('recycleBinTitle')}
        subtitle={format('recycleBinCount', { count: items.length })}
        actions={
          <Button
            variant="ghost"
            onClick={() => purgeExpiredMutation.mutate()}
            loading={purgeExpiredMutation.isPending}
            disabled={purgeExpiredMutation.isPending}
          >
            <IconTrash2 size={14} /> {t('purgeExpired')}
          </Button>
        }
      />

      <div style={{
        display: 'flex', gap: '4px', marginBottom: '16px',
        background: 'var(--md-surface-container)', padding: '4px',
        borderRadius: 'var(--radius-md)',
        overflowX: 'auto',
      }}>
        {TABS.map(tab => {
          const isActive = activeTab === tab.key;
          const count = tab.key === '' ? items.length : (tabCounts[tab.key] || 0);
          return (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              style={{
                padding: '8px 16px', borderRadius: '999px',
                border: 'none', cursor: 'pointer',
                fontSize: '13px', fontWeight: isActive ? 600 : 400,
                background: isActive ? 'var(--md-primary-container)' : 'transparent',
                color: isActive ? 'var(--md-on-primary-container)' : 'var(--md-on-surface-variant)',
                transition: 'all 0.18s ease',
                display: 'flex', alignItems: 'center', gap: '6px',
                whiteSpace: 'nowrap',
              }}
              onMouseEnter={e => {
                if (!isActive) e.currentTarget.style.background = 'var(--md-surface-container-high)';
              }}
              onMouseLeave={e => {
                if (!isActive) e.currentTarget.style.background = 'transparent';
              }}
            >
              {t(tab.labelKey)}
              {count > 0 && (
                <span style={{
                  fontSize: '11px', fontWeight: 700,
                  padding: '1px 6px', borderRadius: '999px',
                  background: isActive ? 'var(--md-primary)' : 'var(--md-surface-container-high)',
                  color: isActive ? 'var(--md-on-primary)' : 'var(--md-on-surface-variant)',
                  minWidth: '18px', textAlign: 'center',
                }}>
                  {count}
                </span>
              )}
            </button>
          );
        })}
      </div>

      <Card style={{ overflow: 'hidden' }}>
        {selectedIds.size > 0 && (
          <div style={{
            padding: '12px 16px',
            background: 'var(--md-primary-container)',
            display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            animation: 'slideDown 0.2s ease',
          }}>
            <span style={{ fontSize: '13px', fontWeight: 600, color: 'var(--md-on-primary-container)' }}>
              {format('selectedItemsCount', { count: selectedIds.size })}
            </span>
            <div style={{ display: 'flex', gap: '8px' }}>
              <Button size="sm" variant="ghost" onClick={() => setBatchAction('restore')}>
                <IconRefreshCw size={14} /> {t('batchRestore')}
              </Button>
              <Button size="sm" variant="danger" onClick={() => setBatchAction('purge')}>
                <IconTrash2 size={14} /> {t('batchPurge')}
              </Button>
            </div>
          </div>
        )}

        <div style={{ overflowX: 'auto' }}>
          <table style={{ width: '100%', minWidth: '650px', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={{ ...TH, width: '44px', textAlign: 'center' }}>
                  <button
                    onClick={toggleSelectAll}
                    style={{
                      width: '18px', height: '18px', borderRadius: '4px',
                      border: `1.5px solid ${selectedIds.size === filteredItems.length && filteredItems.length > 0 ? 'var(--md-primary)' : 'var(--md-outline-variant)'}`,
                      background: selectedIds.size === filteredItems.length && filteredItems.length > 0 ? 'var(--md-primary)' : 'transparent',
                      cursor: 'pointer', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                      transition: 'all 0.15s ease',
                    }}
                  >
                    {selectedIds.size === filteredItems.length && filteredItems.length > 0 && (
                      <IconCheck size={12} color="var(--md-on-primary)" />
                    )}
                  </button>
                </th>
                <th style={TH}>{t('typeLabel')}</th>
                <th style={TH}>{t('nameLabel')}</th>
                <th style={{ ...TH, width: '140px' }}>{t('deletedAtLabel')}</th>
                <th style={{ ...TH, width: '100px' }}>{t('remainingTime')}</th>
                <th style={{ ...TH, width: '140px', textAlign: 'right' }}>{t('actionsLabel')}</th>
              </tr>
            </thead>
            <tbody>
              {filteredItems.length > 0 ? filteredItems.map(item => {
                const config = TYPE_CONFIG[item.item_type];
                const exp = expiresLabel(item.expires_in_days, format, t);
                const isSelected = selectedIds.has(item.id);
                return (
                  <tr
                    key={`${item.item_type}-${item.id}`}
                    onClick={() => setPreviewItem(item)}
                    style={{
                      cursor: 'pointer',
                      transition: 'background 0.12s ease',
                      background: isSelected ? 'var(--md-primary-container)' : 'transparent',
                    }}
                    onMouseEnter={e => { if (!isSelected) e.currentTarget.style.background = 'var(--md-surface-container)'; }}
                    onMouseLeave={e => { if (!isSelected) e.currentTarget.style.background = 'transparent'; }}
                  >
                    <td style={{ ...TD, textAlign: 'center' }} onClick={e => e.stopPropagation()}>
                      <button
                        onClick={() => toggleSelect(item.id)}
                        style={{
                          width: '18px', height: '18px', borderRadius: '4px',
                          border: `1.5px solid ${isSelected ? 'var(--md-primary)' : 'var(--md-outline-variant)'}`,
                          background: isSelected ? 'var(--md-primary)' : 'transparent',
                          cursor: 'pointer', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                          transition: 'all 0.15s ease',
                        }}
                      >
                        {isSelected && <IconCheck size={12} color="var(--md-on-primary)" />}
                      </button>
                    </td>
                    <td style={TD}>
                      <span style={{
                        display: 'inline-flex', alignItems: 'center', gap: '6px',
                        padding: '3px 10px', borderRadius: '999px', fontSize: '12px', fontWeight: 600,
                        background: `${config?.color || '#999'}12`,
                        color: config?.color || '#999',
                      }}>
                        {config?.icon} {config?.labelKey ? t(config.labelKey) : item.item_type}
                      </span>
                    </td>
                    <td style={{ ...TD, fontWeight: 600 }}>
                      <div>
                        <div style={{ fontSize: '13.5px', color: 'var(--md-on-surface)' }}>{item.name}</div>
                        {item.subtitle && (
                          <div style={{ fontSize: '11.5px', color: 'var(--md-outline)', marginTop: '2px', fontFamily: 'monospace' }}>
                            {item.subtitle}
                          </div>
                        )}
                      </div>
                    </td>
                    <td style={{ ...TD, fontFamily: 'monospace', fontSize: '12px', color: 'var(--md-on-surface-variant)' }}>
                      {formatDeletedAt(item.deleted_at)}
                    </td>
                    <td style={TD}>
                      <span style={{
                        fontSize: '12px', fontWeight: 600, color: exp.color,
                        display: 'flex', alignItems: 'center', gap: '4px',
                      }}>
                        <span style={{
                          width: '6px', height: '6px', borderRadius: '50%',
                          background: exp.color, display: 'inline-block',
                        }} />
                        {exp.text}
                      </span>
                    </td>
                    <td style={TD} onClick={e => e.stopPropagation()}>
                      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '4px' }}>
                        <button
                          title={t('restoreItem')}
                          style={iconBtn('#10b981')}
                          onClick={() => setRestoreTarget(item)}
                          onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-surface-container)'; e.currentTarget.style.color = '#059669'; }}
                          onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#10b981'; }}
                        >
                          <IconRefreshCw size={15} />
                        </button>
                        <button
                          title={t('purgeItem')}
                          style={iconBtn('#ef4444')}
                          onClick={() => setPurgeTarget(item)}
                          onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-error-container)'; e.currentTarget.style.color = 'var(--md-on-error-container)'; }}
                          onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#ef4444'; }}
                        >
                          <IconTrash2 size={15} />
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              }) : null}
            </tbody>
          </table>
        </div>

        {filteredItems.length === 0 && (
          <EmptyState
            icon={<IconTrash2 size={28} />}
            message={t('recycleBinEmpty')}
          />
        )}
      </Card>

      {previewItem && (
        <>
          <div
            style={{
              position: 'fixed', top: 0, left: 0, right: 0, bottom: 0,
              background: 'rgba(0,0,0,0.3)',
              zIndex: 999,
            }}
            onClick={() => setPreviewItem(null)}
          />
          <PreviewPanel item={previewItem} onClose={() => setPreviewItem(null)} t={t} format={format} />
        </>
      )}

      <ConfirmDialog
        open={!!restoreTarget}
        onClose={() => setRestoreTarget(null)}
        onConfirm={() => restoreTarget && restoreMutation.mutate(restoreTarget)}
        title={t('restoreTitle')}
        message={format('restoreMessage', { name: restoreTarget?.name || '' })}
        confirmText={t('restoreConfirm')}
        variant="info"
      />

      <ConfirmDialog
        open={!!purgeTarget}
        onClose={() => setPurgeTarget(null)}
        onConfirm={() => purgeTarget && purgeMutation.mutate(purgeTarget)}
        title={t('purgeTitle')}
        message={format('purgeMessage', { name: purgeTarget?.name || '' })}
        confirmText={t('purgeConfirm')}
        variant="danger"
      />

      <ConfirmDialog
        open={!!batchAction}
        onClose={() => setBatchAction(null)}
        onConfirm={handleBatchAction}
        title={batchAction === 'restore' ? t('batchRestoreTitle') : t('batchPurgeTitle')}
        message={
          batchAction === 'restore'
            ? format('batchRestoreMessage', { count: selectedIds.size })
            : format('batchPurgeMessage', { count: selectedIds.size })
        }
        confirmText={batchAction === 'restore' ? format('batchRestoreConfirm', { count: selectedIds.size }) : format('batchPurgeConfirm', { count: selectedIds.size })}
        variant={batchAction === 'restore' ? 'info' : 'danger'}
      />

      <style>{`
        @keyframes slideInRight {
          from { transform: translateX(100%); opacity: 0; }
          to { transform: translateX(0); opacity: 1; }
        }
        @keyframes slideDown {
          from { opacity: 0; transform: translateY(-8px); }
          to { opacity: 1; transform: translateY(0); }
        }
        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.4; }
        }
      `}</style>
    </>
  );
}
