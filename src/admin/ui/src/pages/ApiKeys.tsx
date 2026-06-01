import { useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { apiData, API_PREFIX, getQueryClient } from '../lib/api';
import { esc } from '../lib/utils';
import type { ApiKeyListItem, CreateApiKeyResponse } from '../types';
import { PageHeader } from '../components/PageHeader';
import { Card } from '../components/Card';
import { Button } from '../components/Button';
import { Modal } from '../components/Modal';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { Input } from '../components/Input';
import { EmptyState } from '../components/EmptyState';
import { Skeleton } from '../components/Skeleton';
import { IconKey, IconPlus, IconTrash2, IconCopy, IconPencil } from '../components/Icons';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';

export default function ApiKeys() {
  const toast = useToast();
  const { t, format } = useI18n();

  const { data: items = [], isLoading } = useQuery({
    queryKey: ['api-keys'],
    queryFn: () => apiData<ApiKeyListItem[]>(`${API_PREFIX}/admin/api-keys`),
  });

  // 创建弹窗
  const [createOpen, setCreateOpen] = useState(false);
  const [newKeyName, setNewKeyName] = useState('');
  const [newKeyExpiresAt, setNewKeyExpiresAt] = useState('');
  // 创建成功后显示完整 key
  const [createdKey, setCreatedKey] = useState<CreateApiKeyResponse | null>(null);

  // 编辑弹窗
  const [editOpen, setEditOpen] = useState(false);
  const [editingKeyId, setEditingKeyId] = useState<string | null>(null);
  const [editKeyName, setEditKeyName] = useState('');

  // 撤销确认
  const [revokeTarget, setRevokeTarget] = useState<ApiKeyListItem | null>(null);

  const createMutation = useMutation({
    mutationFn: (body: { name: string; expires_at?: string }) =>
      apiData<CreateApiKeyResponse>(`${API_PREFIX}/admin/api-keys`, {
        method: 'POST',
        body: JSON.stringify(body),
      }),
    onSuccess: (data) => {
      getQueryClient().invalidateQueries({ queryKey: ['api-keys'] });
      setCreatedKey(data);
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('saveFailed'), 'error');
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      apiData(`${API_PREFIX}/admin/api-keys/${id}`, {
        method: 'PATCH',
        body: JSON.stringify({ name }),
      }),
    onSuccess: () => {
      getQueryClient().invalidateQueries({ queryKey: ['api-keys'] });
      closeEditDialog();
      toast(t('apiKeyUpdated'), 'success');
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('saveFailed'), 'error');
    },
  });

  const revokeMutation = useMutation({
    mutationFn: (id: string) =>
      apiData(`${API_PREFIX}/admin/api-keys/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      getQueryClient().invalidateQueries({ queryKey: ['api-keys'] });
      setRevokeTarget(null);
      toast(t('apiKeyRevoked'), 'success');
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('deleteFailed'), 'error');
      setRevokeTarget(null);
    },
  });

  function openCreate() {
    setNewKeyName('');
    setNewKeyExpiresAt('');
    setCreatedKey(null);
    setCreateOpen(true);
  }

  function closeCreate() {
    setCreateOpen(false);
    setCreatedKey(null);
  }

  function handleCreate() {
    if (!newKeyName.trim()) {
      toast(t('apiKeyNameRequired'), 'error');
      return;
    }
    createMutation.mutate({
      name: newKeyName.trim(),
      expires_at: newKeyExpiresAt || undefined,
    });
  }

  function copyKeyToClipboard() {
    if (!createdKey?.api_key) return;
    navigator.clipboard.writeText(createdKey.api_key).then(
      () => toast(t('copySuccess'), 'success'),
      () => toast(t('copyFailed'), 'error'),
    );
  }

  function openEdit(key: ApiKeyListItem) {
    setEditingKeyId(key.id);
    setEditKeyName(key.name);
    setEditOpen(true);
  }

  function closeEditDialog() {
    setEditOpen(false);
    setEditingKeyId(null);
    setEditKeyName('');
  }

  function handleSaveEdit() {
    if (!editKeyName.trim() || !editingKeyId) return;
    updateMutation.mutate({ id: editingKeyId, name: editKeyName.trim() });
  }

  function confirmRevoke() {
    if (!revokeTarget) return;
    revokeMutation.mutate(revokeTarget.id);
  }

  const maskFullKey = (prefix: string) => `${prefix}${'*'.repeat(24)}`;

  return (
    <>
      <PageHeader
        title={t('apiKeysTitle')}
        subtitle={format('apiKeysCount', { count: items.length })}
        actions={
          <Button onClick={openCreate}><IconPlus size={14} /> {t('newApiKey')}</Button>
        }
      />

      <Card>
        <div style={{ padding: '22px' }}>
          {isLoading ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              {[...Array(3)].map((_, i) => (
                <Skeleton key={i} width="100%" height={60} className="rounded-lg" />
              ))}
            </div>
          ) : items.length > 0 ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              {/* 表头 */}
              <div style={{
                display: 'grid',
                gridTemplateColumns: '2fr 1.2fr 1fr 0.8fr 0.8fr 0.6fr',
                gap: '12px',
                padding: '8px 14px',
                fontSize: '12px',
                fontWeight: 700,
                color: 'var(--md-on-surface-variant)',
                textTransform: 'uppercase',
                letterSpacing: '0.05em',
              }}>
                <span>{t('apiKeyName')}</span>
                <span>{t('apiKeyPrefix')}</span>
                <span>{t('apiKeyLastUsed')}</span>
                <span>{t('apiKeyExpires')}</span>
                <span>{t('apiKeyCreated')}</span>
                <span>{t('actionsLabel')}</span>
              </div>

              {items.map((key) => (
                <div
                  key={key.id}
                  style={{
                    display: 'grid',
                    gridTemplateColumns: '2fr 1.2fr 1fr 0.8fr 0.8fr 0.6fr',
                    gap: '12px',
                    padding: '14px 14px',
                    borderRadius: '12px',
                    background: 'var(--md-surface-container)',
                    alignItems: 'center',
                    fontSize: '13px',
                    transition: 'background 0.15s ease',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.background = 'var(--md-surface-container-highest)';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.background = 'var(--md-surface-container)';
                  }}
                >
                  <span style={{ fontWeight: 600, color: 'var(--md-on-surface)' }}>
                    {esc(key.name)}
                  </span>

                  <code style={{
                    fontSize: '12px',
                    fontFamily: 'monospace',
                    color: 'var(--md-outline)',
                    background: 'var(--md-surface-container-low)',
                    padding: '2px 8px',
                    borderRadius: '5px',
                    whiteSpace: 'nowrap',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                  }}>
                    {maskFullKey(key.key_prefix)}
                  </code>

                  <span style={{ color: 'var(--md-on-surface-variant)', fontSize: '12px' }}>
                    {key.last_used_at
                      ? new Date(key.last_used_at).toLocaleDateString()
                      : t('apiKeyNeverUsed')}
                  </span>

                  <span style={{
                    color: key.expires_at
                      ? new Date(key.expires_at) < new Date()
                        ? 'var(--md-error)'
                        : 'var(--md-on-surface-variant)'
                      : 'var(--md-on-surface-variant)',
                    fontSize: '12px',
                  }}>
                    {key.expires_at
                      ? new Date(key.expires_at).toLocaleDateString()
                      : t('apiKeyNoExpiry')}
                  </span>

                  <span style={{ color: 'var(--md-on-surface-variant)', fontSize: '12px' }}>
                    {new Date(key.created_at).toLocaleDateString()}
                  </span>

                  <div style={{ display: 'flex', gap: '4px' }}>
                    <button
                      onClick={() => openEdit(key)}
                      title={t('edit')}
                      style={{
                        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                        width: '28px', height: '28px',
                        borderRadius: 'var(--radius-full)',
                        border: 'none',
                        background: 'var(--md-surface-container-low)',
                        color: '#6b7280',
                        cursor: 'pointer',
                        transition: 'all 0.15s ease',
                      }}
                      onMouseEnter={e => {
                        e.currentTarget.style.background = 'var(--md-surface-container)';
                        e.currentTarget.style.color = '#10b981';
                      }}
                      onMouseLeave={e => {
                        e.currentTarget.style.background = 'var(--md-surface-container-low)';
                        e.currentTarget.style.color = '#6b7280';
                      }}
                    >
                      <IconPencil size={13} />
                    </button>
                    <button
                      onClick={() => setRevokeTarget(key)}
                      title={t('revoke')}
                      style={{
                        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                        width: '28px', height: '28px',
                        borderRadius: 'var(--radius-full)',
                        border: 'none',
                        background: 'var(--md-surface-container-low)',
                        color: '#6b7280',
                        cursor: 'pointer',
                        transition: 'all 0.15s ease',
                      }}
                      onMouseEnter={e => {
                        e.currentTarget.style.background = 'var(--md-surface-container)';
                        e.currentTarget.style.color = '#ef4444';
                      }}
                      onMouseLeave={e => {
                        e.currentTarget.style.background = 'var(--md-surface-container-low)';
                        e.currentTarget.style.color = '#6b7280';
                      }}
                    >
                      <IconTrash2 size={13} />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <EmptyState
              icon={<IconKey size={28} />}
              message={t('noApiKeys')}
              action={
                <Button size="sm" onClick={openCreate}>
                  <IconPlus size={14} /> {t('createFirstApiKey')}
                </Button>
              }
            />
          )}
        </div>
      </Card>

      {/* 创建 Modal */}
      <Modal
        open={createOpen}
        onClose={closeCreate}
        title={t('createApiKeyTitle')}
        width="460px"
        actions={
          !createdKey ? (
            <>
              <Button variant="ghost" onClick={closeCreate}>{t('cancel')}</Button>
              <Button onClick={handleCreate} disabled={createMutation.isPending} loading={createMutation.isPending}>
                {t('create')}
              </Button>
            </>
          ) : (
            <Button variant="ghost" onClick={closeCreate}>{t('close')}</Button>
          )
        }
      >
        {!createdKey ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '18px' }}>
            <Input
              label={t('apiKeyName')}
              value={newKeyName}
              onChange={(e) => setNewKeyName(e.target.value)}
              placeholder={t('apiKeyNamePlaceholder')}
              autoFocus
            />
            <Input
              label={t('apiKeyExpires')}
              type="datetime-local"
              value={newKeyExpiresAt}
              onChange={(e) => setNewKeyExpiresAt(e.target.value)}
            />
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
            <div style={{
              padding: '12px 16px',
              borderRadius: '8px',
              background: 'var(--md-error-container)',
              color: 'var(--md-on-error-container)',
              fontSize: '13px',
              fontWeight: 600,
              lineHeight: 1.5,
            }}>
              {t('apiKeyShowOnceWarning')}
            </div>
            <div style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
              padding: '12px 16px',
              borderRadius: '8px',
              background: 'var(--md-surface-container)',
              fontFamily: 'monospace',
              fontSize: '14px',
              wordBreak: 'break-all',
              color: 'var(--md-on-surface)',
            }}>
              <span style={{ flex: 1 }}>{createdKey.api_key}</span>
              <button
                onClick={copyKeyToClipboard}
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  width: '32px', height: '32px',
                  borderRadius: 'var(--radius-full)',
                  border: 'none',
                  background: 'var(--md-primary-container)',
                  color: 'var(--md-on-primary-container)',
                  cursor: 'pointer',
                  flexShrink: 0,
                }}
              >
                <IconCopy size={14} />
              </button>
            </div>
            <div style={{ fontSize: '12px', color: 'var(--md-on-surface-variant)' }}>
              {t('apiKeyName')}: {esc(createdKey.name)}<br />
              {t('apiKeyExpires')}: {createdKey.expires_at ? new Date(createdKey.expires_at).toLocaleString() : t('apiKeyNoExpiry')}
            </div>
          </div>
        )}
      </Modal>

      {/* 编辑 Modal */}
      <Modal
        open={editOpen}
        onClose={closeEditDialog}
        title={t('editApiKeyTitle')}
        width="400px"
        actions={
          <>
            <Button variant="ghost" onClick={closeEditDialog}>{t('cancel')}</Button>
            <Button onClick={handleSaveEdit} disabled={updateMutation.isPending} loading={updateMutation.isPending}>
              {t('save')}
            </Button>
          </>
        }
      >
        <Input
          label={t('apiKeyName')}
          value={editKeyName}
          onChange={(e) => setEditKeyName(e.target.value)}
          placeholder={t('apiKeyNamePlaceholder')}
          autoFocus
        />
      </Modal>

      {/* 撤销确认 */}
      <ConfirmDialog
        open={!!revokeTarget}
        onClose={() => setRevokeTarget(null)}
        onConfirm={confirmRevoke}
        title={t('revokeApiKeyTitle')}
        message={format('revokeApiKeyMessage', { name: revokeTarget?.name || '' })}
        variant="danger"
        confirmText={t('revoke')}
      />
    </>
  );
}
