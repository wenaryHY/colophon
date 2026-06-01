import { useState, useCallback } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { apiData, API_PREFIX, getQueryClient } from '../lib/api';
import { useI18n } from '../i18n';
import { useToast } from '../contexts/ToastContext';
import { PageHeader } from '../components/PageHeader';
import { Button } from '../components/Button';
import { Input } from '../components/Input';
import { Modal } from '../components/Modal';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { EmptyState } from '../components/EmptyState';
import {
  IconWebhook,
  IconPlus,
  IconPencil,
  IconTrash2,
  IconEye,
  IconExternalLink,
} from '../components/Icons';
import type {
  Webhook,
  WebhookDelivery,
  CreateWebhookRequest,
  UpdateWebhookRequest,
} from '../types';

const CARD_STYLE: React.CSSProperties = {
  borderRadius: 'var(--radius-lg)',
  background: 'var(--md-surface-container)',
  padding: '16px 20px',
  display: 'flex',
  flexDirection: 'column' as const,
  gap: '12px',
};

const EMPTY_STYLE: React.CSSProperties = {
  minHeight: '40vh',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
};

// ── 格式化时间差 ──
function timeAgo(dateStr: string | null | undefined): string {
  if (!dateStr) return '—';
  const now = Date.now();
  const then = new Date(dateStr + 'Z').getTime();
  const diffSec = Math.floor((now - then) / 1000);
  if (diffSec < 60) return `${diffSec}s ago`;
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  return `${Math.floor(diffSec / 86400)}d ago`;
}

// ── Webhook 表单 ──
function WebhookForm({
  initial,
  onSubmit,
  onCancel,
  saving,
}: {
  initial?: Webhook;
  onSubmit: (data: CreateWebhookRequest | UpdateWebhookRequest) => void;
  onCancel: () => void;
  saving: boolean;
}) {
  const { t } = useI18n();
  const isEdit = !!initial;

  const [name, setName] = useState(initial?.name ?? '');
  const [url, setUrl] = useState(initial?.url ?? '');
  const [events, setEvents] = useState(initial?.events ?? 'post.after_publish');
  const [secret, setSecret] = useState(initial?.secret ?? '');
  const [enabled, setEnabled] = useState(isEdit ? initial!.enabled === 1 : true);
  const [maxRetries, setMaxRetries] = useState(isEdit ? String(initial!.max_retries) : '3');

  const handleSubmit = () => {
    const base = {
      name: name.trim(),
      url: url.trim(),
      events: events.trim() || 'post.after_publish',
      enabled,
      max_retries: Math.max(0, Math.min(5, parseInt(maxRetries) || 3)),
    };
    onSubmit({ ...base, secret: secret.trim() || undefined });
  };

  const formStyle: React.CSSProperties = {
    display: 'flex',
    flexDirection: 'column' as const,
    gap: '16px',
  };

  const labelStyle: React.CSSProperties = {
    fontSize: '13px',
    fontWeight: 600,
    color: 'var(--md-on-surface-variant)',
    marginBottom: '6px',
    display: 'block',
  };

  const checkboxRowStyle: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    gap: '8px',
    paddingTop: '4px',
  };

  return (
    <div style={formStyle}>
      <div>
        <label style={labelStyle}>{t('webhookName')} *</label>
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="My Webhook"
        />
      </div>
      <div>
        <label style={labelStyle}>{t('webhookUrl')} *</label>
        <Input
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder={t('webhookUrlPlaceholder')}
        />
      </div>
      <div>
        <label style={labelStyle}>{t('webhookEvents')}</label>
        <Input
          value={events}
          onChange={(e) => setEvents(e.target.value)}
          placeholder={t('webhookEventsPlaceholder')}
        />
        <div style={{ fontSize: '11px', color: 'var(--md-outline)', marginTop: '4px' }}>
          {t('webhookEventsPlaceholder')}
        </div>
      </div>
      <div>
        <label style={labelStyle}>{t('webhookSecret')}</label>
        <Input
          value={secret}
          onChange={(e) => setSecret(e.target.value)}
          placeholder={t('webhookSecretPlaceholder')}
        />
      </div>
      <div>
        <label style={labelStyle}>{t('webhookMaxRetries')}</label>
        <Input
          type="number"
          value={maxRetries}
          onChange={(e) => setMaxRetries(e.target.value)}
          min="0"
          max="5"
        />
      </div>
      <div style={checkboxRowStyle}>
        <input
          type="checkbox"
          checked={enabled}
          onChange={(e) => setEnabled(e.target.checked)}
          style={{ width: '18px', height: '18px', accentColor: 'var(--md-primary)' }}
        />
        <label style={{ fontSize: '13px', color: 'var(--md-on-surface-variant)', cursor: 'pointer' }}>
          {t('webhookEnabled')}
        </label>
      </div>
      <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end', marginTop: '4px' }}>
        <Button variant="ghost" onClick={onCancel}>
          {t('cancel')}
        </Button>
        <Button onClick={handleSubmit} disabled={saving || !name.trim() || !url.trim()} loading={saving}>
          {saving ? t('saving') : t('save')}
        </Button>
      </div>
    </div>
  );
}

// ── 投递记录面板 ──
function DeliveriesPanel({
  webhookId,
  webhookName,
  onClose,
}: {
  webhookId: string;
  webhookName: string;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [page, setPage] = useState(1);
  const pageSize = 15;

  const { data: deliveriesResp, isLoading } = useQuery({
    queryKey: ['webhookDeliveries', webhookId, page],
    queryFn: () =>
      apiData<{
        items: WebhookDelivery[];
        total: number;
        page: number;
        page_size: number;
      }>(`${API_PREFIX}/admin/webhooks/${webhookId}/deliveries?page=${page}&page_size=${pageSize}`),
    staleTime: 5 * 60 * 1000, // 5 分钟内不重新请求
  });

  const deliveries = deliveriesResp?.items ?? [];
  const total = deliveriesResp?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
      <div style={{
        fontSize: '14px',
        fontWeight: 600,
        color: 'var(--md-on-surface)',
        paddingBottom: '8px',
        borderBottom: '1px solid var(--md-outline-variant)',
      }}>
        {t('webhookDeliveries')}: {webhookName}
      </div>

      {isLoading ? (
        <div style={{ textAlign: 'center', padding: '20px', color: 'var(--md-outline)' }}>
          {t('loading')}
        </div>
      ) : deliveries.length === 0 ? (
        <div style={{ textAlign: 'center', padding: '20px', color: 'var(--md-outline)' }}>
          {t('webhookNoDeliveries')}
        </div>
      ) : (
        <>
          {deliveries.map((d) => (
            <div
              key={d.id}
              style={{
                borderRadius: 'var(--radius-md)',
                background: 'var(--md-surface-container-lowest)',
                padding: '12px 16px',
                display: 'flex',
                flexDirection: 'column',
                gap: '6px',
                fontSize: '12px',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                  <span style={{
                    padding: '2px 8px',
                    borderRadius: 'var(--radius-full)',
                    fontSize: '11px',
                    fontWeight: 600,
                    background: d.success ? '#dcfce7' : '#fef2f2',
                    color: d.success ? '#16a34a' : '#dc2626',
                  }}>
                    {d.success ? t('webhookStatusSuccess') : t('webhookStatusFailed')}
                  </span>
                  <span style={{ color: 'var(--md-on-surface-variant)' }}>
                    {d.event}
                  </span>
                </div>
                <div style={{ display: 'flex', gap: '12px', color: 'var(--md-outline)' }}>
                  {d.response_status && (
                    <span>HTTP {d.response_status}</span>
                  )}
                  {d.duration_ms != null && (
                    <span>{t('webhookDuration')}: {d.duration_ms}ms</span>
                  )}
                  <span>{timeAgo(d.created_at)}</span>
                </div>
              </div>
              {d.response_body && d.response_status && d.response_status >= 400 && (
                <div style={{
                  fontSize: '11px',
                  color: 'var(--md-error)',
                  background: 'rgba(239,68,68,0.06)',
                  padding: '6px 10px',
                  borderRadius: 'var(--radius-sm)',
                  whiteSpace: 'pre-wrap',
                  wordBreak: 'break-all',
                  maxHeight: '80px',
                  overflow: 'auto',
                }}>
                  {d.response_body.substring(0, 300)}
                </div>
              )}
            </div>
          ))}

          {/* 分页 */}
          {totalPages > 1 && (
            <div style={{ display: 'flex', justifyContent: 'center', gap: '8px', paddingTop: '4px' }}>
              <Button variant="ghost" size="sm" disabled={page <= 1} onClick={() => setPage(page - 1)}>
                {t('prev')}
              </Button>
              <span style={{ fontSize: '12px', color: 'var(--md-on-surface-variant)', alignSelf: 'center' }}>
                {page} / {totalPages}
              </span>
              <Button variant="ghost" size="sm" disabled={page >= totalPages} onClick={() => setPage(page + 1)}>
                {t('next')}
              </Button>
            </div>
          )}
        </>
      )}

      <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: '8px' }}>
        <Button variant="ghost" onClick={onClose}>
          {t('close')}
        </Button>
      </div>
    </div>
  );
}

// ── 主页面 ──
export default function Webhooks() {
  const { t } = useI18n();
  const toast = useToast();
  const [showForm, setShowForm] = useState(false);
  const [editTarget, setEditTarget] = useState<Webhook | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Webhook | null>(null);
  const [deliveryTarget, setDeliveryTarget] = useState<Webhook | null>(null);

  const { data: webhooks, isLoading } = useQuery({
    queryKey: ['webhooks'],
    queryFn: () => apiData<Webhook[]>(`${API_PREFIX}/admin/webhooks`),
    staleTime: 5 * 60 * 1000, // 5 分钟内不重新请求
  });

  const createMutation = useMutation({
    mutationFn: (data: CreateWebhookRequest) =>
      apiData<Webhook>(`${API_PREFIX}/admin/webhooks`, {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      getQueryClient().invalidateQueries({ queryKey: ['webhooks'] });
      setShowForm(false);
      setEditTarget(null);
      toast(t('createSuccess'), 'success');
    },
    onError: (e) => toast(e instanceof Error ? e.message : t('createFailed'), 'error'),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateWebhookRequest }) =>
      apiData<Webhook>(`${API_PREFIX}/admin/webhooks/${id}`, {
        method: 'PATCH',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      getQueryClient().invalidateQueries({ queryKey: ['webhooks'] });
      setShowForm(false);
      setEditTarget(null);
      toast(t('updateSuccess'), 'success');
    },
    onError: (e) => toast(e instanceof Error ? e.message : t('updateFailed'), 'error'),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) =>
      apiData(`${API_PREFIX}/admin/webhooks/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      getQueryClient().invalidateQueries({ queryKey: ['webhooks'] });
      setDeleteTarget(null);
      toast(t('deleteSuccess'), 'success');
    },
    onError: (e) => toast(e instanceof Error ? e.message : t('deleteFailed'), 'error'),
  });

  const handleFormSubmit = useCallback(
    (data: CreateWebhookRequest | UpdateWebhookRequest) => {
      if (editTarget) {
        updateMutation.mutate({ id: editTarget.id, data: data as UpdateWebhookRequest });
      } else {
        createMutation.mutate(data as CreateWebhookRequest);
      }
    },
    [editTarget, createMutation, updateMutation],
  );

  const handleEdit = (w: Webhook) => {
    setEditTarget(w);
    setShowForm(true);
  };

  const handleCreate = () => {
    setEditTarget(null);
    setShowForm(true);
  };

  // ── Loading ──
  if (isLoading) {
    return (
      <div style={EMPTY_STYLE}>
        <div style={{ color: 'var(--md-on-surface-variant)', fontSize: '14px' }}>{t('loading')}</div>
      </div>
    );
  }

  const list = webhooks ?? [];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
      <PageHeader
        title={t('webhooks')}
        subtitle={t('webhooksSubtitle', `共 ${list.length} 个 Webhook`)}
        actions={
          <Button onClick={handleCreate}>
            <IconPlus /> {t('webhookCreate')}
          </Button>
        }
      />

      {/* Webhook 列表 */}
      {list.length === 0 ? (
        <EmptyState
          icon={<IconWebhook size={28} />}
          message={t('noData')}
          action={
            <Button onClick={handleCreate}>
              <IconPlus /> {t('webhookCreate')}
            </Button>
          }
        />
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          {list.map((w) => {
            const eventList = w.events.split(',').map((e) => e.trim());
            return (
              <div key={w.id} style={CARD_STYLE}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                    <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                      <span style={{ fontSize: '15px', fontWeight: 700, color: 'var(--md-on-surface)' }}>
                        {w.name}
                      </span>
                      <span style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: '6px',
                        padding: '4px 12px',
                        borderRadius: 'var(--radius-full)',
                        fontSize: '12px',
                        fontWeight: 600,
                        background: w.enabled === 1 ? 'var(--success-50)' : 'var(--md-surface-container)',
                        color: w.enabled === 1 ? 'var(--success-700)' : 'var(--md-on-surface-variant)',
                      }}>
                        <span style={{
                          width: '6px', height: '6px', borderRadius: '50%',
                          background: w.enabled === 1 ? 'var(--success-500)' : 'var(--md-outline)',
                          flexShrink: 0,
                        }} />
                        {w.enabled === 1 ? t('webhookEnabled') : t('statusDraft')}
                      </span>
                    </div>
                    <div style={{
                      fontSize: '12px',
                      color: 'var(--md-on-surface-variant)',
                      fontFamily: 'monospace',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '4px',
                    }}>
                      <span>{w.url}</span>
                      <a
                        href={w.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        style={{ display: 'inline-flex', color: 'var(--md-primary)' }}
                      >
                        <IconExternalLink size={12} />
                      </a>
                    </div>
                    <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                      {eventList.map((ev) => (
                        <span
                          key={ev}
                          style={{
                            padding: '2px 10px',
                            borderRadius: 'var(--radius-full)',
                            fontSize: '11px',
                            fontWeight: 500,
                            background: 'var(--md-secondary-container)',
                            color: 'var(--md-on-secondary-container)',
                          }}
                        >
                          {ev}
                        </span>
                      ))}
                    </div>
                  </div>

                  {/* 操作按钮 */}
                  <div style={{ display: 'flex', gap: '4px', flexShrink: 0 }}>
                    <Button variant="ghost" size="sm" onClick={() => setDeliveryTarget(w)} title={t('webhookDeliveries')}>
                      <IconEye />
                    </Button>
                    <Button variant="ghost" size="sm" onClick={() => handleEdit(w)}>
                      <IconPencil />
                    </Button>
                    <Button variant="ghost" size="sm" onClick={() => setDeleteTarget(w)}>
                      <IconTrash2 />
                    </Button>
                  </div>
                </div>

                {/* 底部信息 */}
                <div style={{
                  display: 'flex',
                  gap: '20px',
                  fontSize: '11px',
                  color: 'var(--md-outline)',
                  borderTop: '1px solid var(--md-outline-variant)',
                  paddingTop: '10px',
                }}>
                  {w.last_error && (
                    <span style={{ color: 'var(--md-error)' }}>
                      {t('webhookLastError')}: {w.last_error.substring(0, 80)}
                    </span>
                  )}
                  {w.last_triggered_at && (
                    <span>{t('webhookLastTriggered')}: {timeAgo(w.last_triggered_at)}</span>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* 创建/编辑弹窗 */}
      {showForm && (
        <Modal
          open={showForm}
          onClose={() => {
            setShowForm(false);
            setEditTarget(null);
          }}
          title={editTarget ? t('webhookEdit') : t('webhookCreate')}
        >
          <WebhookForm
            initial={editTarget ?? undefined}
            onSubmit={handleFormSubmit}
            onCancel={() => {
              setShowForm(false);
              setEditTarget(null);
            }}
            saving={createMutation.isPending || updateMutation.isPending}
          />
        </Modal>
      )}

      {/* 删除确认 */}
      {deleteTarget && (
        <ConfirmDialog
          open={!!deleteTarget}
          onClose={() => setDeleteTarget(null)}
          onConfirm={() => deleteMutation.mutate(deleteTarget.id)}
          message={t('confirmDelete')}
        />
      )}

      {/* 投递记录弹窗 */}
      {deliveryTarget && (
        <Modal
          open={!!deliveryTarget}
          onClose={() => setDeliveryTarget(null)}
          title={t('webhookDeliveries')}
        >
          <DeliveriesPanel
            webhookId={deliveryTarget.id}
            webhookName={deliveryTarget.name}
            onClose={() => setDeliveryTarget(null)}
          />
        </Modal>
      )}
    </div>
  );
}
