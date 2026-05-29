import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation } from '@tanstack/react-query';
import { apiData, API_PREFIX, getQueryClient } from '../lib/api';
import { esc } from '../lib/utils';
import type { Tag } from '../types';
import { PageHeader } from '../components/PageHeader';
import { Card } from '../components/Card';
import { Button } from '../components/Button';
import { Modal } from '../components/Modal';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { Input } from '../components/Input';
import { EmptyState } from '../components/EmptyState';
import { Skeleton } from '../components/Skeleton';
import { IconTag, IconPlus, IconPencil, IconTrash2, IconCheck } from '../components/Icons';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';

/* ═════════════ MD3 样式常量 ═════════════ */
const tagCardBase: React.CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: '8px',
  padding: '8px 14px',
  borderRadius: 'var(--radius-full)',
  fontSize: '13px',
  fontWeight: 600,
  background: 'var(--md-surface-container)',
  transition: 'all 0.16s ease',
  cursor: 'default',
};

const iconBtn: React.CSSProperties = {
  display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
  width: '28px', height: '28px',
  borderRadius: 'var(--radius-full)',
  border: 'none',
  cursor: 'pointer',
  transition: 'all 0.15s ease',
  flexShrink: 0,
  background: 'var(--md-surface-container-low)',
};

/* ═════════════ 主组件 ═════════════ */
export default function Tags() {
  const toast = useToast();
  const { t, format } = useI18n();
  const navigate = useNavigate();

  const { data: items = [], isLoading } = useQuery({
    queryKey: ['tags'],
    queryFn: () => apiData<Tag[]>(`${API_PREFIX}/tags`),
  });

  // Modal 状态：新建或编辑
  const [editorOpen, setEditorOpen] = useState(false);
  const [editingTag, setEditingTag] = useState<Tag | null>(null);
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');

  // 删除状态
  const [deleteTag, setDeleteTag] = useState<Tag | null>(null);

  // 批量选择状态
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [batchDeleteOpen, setBatchDeleteOpen] = useState(false);

  const saveMutation = useMutation({
    mutationFn: async ({ isEdit, tagId, body }: {
      isEdit: boolean;
      tagId?: string;
      body: { name: string; slug?: string };
    }) => {
      if (isEdit) {
        return apiData(`${API_PREFIX}/admin/tags/${tagId}`, {
          method: 'PATCH',
          body: JSON.stringify(body),
        });
      }
      return apiData(`${API_PREFIX}/admin/tags`, {
        method: 'POST',
        body: JSON.stringify(body),
      });
    },
    onSuccess: () => {
      getQueryClient().invalidateQueries({ queryKey: ['tags'] });
      closeEditor();
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('saveFailed'), 'error');
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (tagId: string) =>
      apiData(`${API_PREFIX}/admin/tags/${tagId}`, { method: 'DELETE' }),
    onSuccess: (_, tagId) => {
      toast(t('deleteSuccess'), 'success');
      setSelectedIds(prev => { const next = new Set(prev); next.delete(tagId); return next; });
      setDeleteTag(null);
      getQueryClient().invalidateQueries({ queryKey: ['tags'] });
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('deleteFailed'), 'error');
      setDeleteTag(null);
    },
  });

  const batchDeleteMutation = useMutation({
    mutationFn: async (ids: string[]) => {
      await Promise.all(ids.map(id =>
        apiData(`${API_PREFIX}/admin/tags/${id}`, { method: 'DELETE' })
      ));
    },
    onSuccess: (_, ids) => {
      toast(format('tagsDeletedSuccess', { count: ids.length }), 'success');
      setSelectedIds(new Set());
      setBatchDeleteOpen(false);
      getQueryClient().invalidateQueries({ queryKey: ['tags'] });
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('batchDeleteFailed'), 'error');
      setBatchDeleteOpen(false);
    },
  });

  // 打开新建
  function openCreate() {
    setEditingTag(null);
    setName('');
    setSlug('');
    setEditorOpen(true);
  }

  // 打开编辑
  function openEdit(tag: Tag) {
    setEditingTag(tag);
    setName(tag.name);
    setSlug(tag.slug || '');
    setEditorOpen(true);
  }

  function closeEditor() {
    setEditorOpen(false);
    setEditingTag(null);
    setName('');
    setSlug('');
  }

  async function handleSave() {
    if (!name.trim()) { toast(t('tagNameRequired'), 'error'); return; }
    saveMutation.mutate({
      isEdit: !!editingTag,
      tagId: editingTag?.id,
      body: { name: name.trim(), slug: slug || undefined },
    });
  }

  function confirmDelete() {
    if (!deleteTag) return;
    deleteMutation.mutate(deleteTag.id);
  }

  // 选择逻辑
  function toggleSelect(id: string) {
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function toggleSelectAll() {
    if (selectedIds.size === items.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(items.map(t => t.id)));
    }
  }

  return (
    <>
      <PageHeader
        title={t('tagsTitle')}
        subtitle={format('tagsCount', { count: items.length })}
        actions={
          <div style={{ display: 'flex', gap: '8px' }}>
            {selectedIds.size > 0 && (
              <Button variant="danger" onClick={() => setBatchDeleteOpen(true)}>
                <IconTrash2 size={14} /> {format('batchDeleteTags', { count: selectedIds.size })}
              </Button>
            )}
            <Button onClick={openCreate}><IconPlus size={14} /> {t('newTag')}</Button>
          </div>
        }
      />

      <div style={{ display: 'flex', gap: '8px', marginBottom: '20px', background: 'var(--md-surface-container)', borderRadius: 'var(--radius-full)', padding: '4px' }}>
        <button
          style={{
            padding: '8px 18px', fontSize: '13px', fontWeight: 600, color: 'var(--md-on-primary-container)',
            background: 'var(--md-primary-container)', border: 'none', cursor: 'pointer',
            borderRadius: 'var(--radius-full)', transition: 'all 0.2s ease',
          }}
        >
          {t('activeTags')}
        </button>
        <button
          onClick={() => navigate('/trash?tab=tag')}
          style={{
            padding: '8px 18px', fontSize: '13px', fontWeight: 600, color: 'var(--md-on-surface-variant)',
            background: 'transparent', border: 'none', cursor: 'pointer',
            borderRadius: 'var(--radius-full)', transition: 'all 0.2s ease',
          }}
          onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-surface-container-high)'; e.currentTarget.style.color = 'var(--md-on-surface)'; }}
          onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = 'var(--md-on-surface-variant)'; }}
        >
          {t('deletedItems')}
        </button>
      </div>

      <Card>
        <div style={{ padding: '22px' }}>
          {/* 全选行 */}
          {!isLoading && items.length > 0 && (
            <div style={{
              display: 'flex', alignItems: 'center', gap: '12px',
              marginBottom: '16px', paddingBottom: '16px',
            }}>
              <button
                onClick={toggleSelectAll}
                style={{
                  width: '18px', height: '18px',
                  borderRadius: '4px',
                  border: `1.5px solid ${selectedIds.size === items.length ? 'var(--md-primary)' : 'var(--md-outline)'}`,
                  background: selectedIds.size === items.length ? 'var(--md-primary)' : 'transparent',
                  cursor: 'pointer',
                  display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                  transition: 'all 0.15s ease',
                  flexShrink: 0,
                }}
              >
                {selectedIds.size === items.length && <IconCheck size={12} color="#fff" />}
              </button>
              <span style={{ fontSize: '12px', color: 'var(--md-outline)' }}>
                {selectedIds.size > 0 ? format('selectedCount', { count: selectedIds.size }) : t('selectAll')}
              </span>
            </div>
          )}

          {isLoading ? (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '10px' }}>
              {[...Array(6)].map((_, i) => (
                <Skeleton key={i} width={100} height={36} className="rounded-lg" />
              ))}
            </div>
          ) : items.length > 0 ? (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '10px' }}>
              {items.map((tag) => {
                const isSelected = selectedIds.has(tag.id);
                return (
                  <div
                    key={tag.id}
                    style={{
                      ...tagCardBase,
                      background: isSelected ? 'var(--md-primary-container)' : 'var(--md-surface-container)',
                    }}
                    onMouseEnter={(e: React.MouseEvent<HTMLDivElement>) => {
                      if (!isSelected) {
                        e.currentTarget.style.background = 'var(--md-surface-container-highest)';
                        e.currentTarget.style.transform = 'scale(0.97)';
                      }
                    }}
                    onMouseLeave={(e: React.MouseEvent<HTMLDivElement>) => {
                      if (!isSelected) {
                        e.currentTarget.style.background = 'var(--md-surface-container)';
                        e.currentTarget.style.transform = 'scale(1)';
                      }
                    }}
                  >
                    {/* Checkbox */}
                    <button
                      onClick={() => toggleSelect(tag.id)}
                      style={{
                        width: '16px', height: '16px',
                        borderRadius: '4px',
                        border: `1.5px solid ${isSelected ? 'var(--md-primary)' : 'var(--md-outline)'}`,
                        background: isSelected ? 'var(--md-primary)' : 'transparent',
                        cursor: 'pointer',
                        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                        transition: 'all 0.15s ease',
                        padding: 0, flexShrink: 0,
                      }}
                    >
                      {isSelected && <IconCheck size={10} color="#fff" />}
                    </button>

                    {/* 标签名 */}
                    <span style={{ color: 'var(--md-on-surface)' }}>{esc(tag.name)}</span>

                    {/* Slug */}
                    <code style={{
                      fontSize: '11px', fontFamily: 'monospace', color: 'var(--md-outline)',
                      background: 'var(--md-surface-container-low)', padding: '2px 6px', borderRadius: '5px',
                    }}>{esc(tag.slug || '')}</code>

                    {/* 操作按钮 */}
                    <div style={{ display: 'flex', gap: '2px', marginLeft: '4px' }}>
                      <button
                        onClick={() => openEdit(tag)}
                        title={t('editTag')}
                        style={{
                          ...iconBtn,
                          color: '#6b7280',
                        }}
                        onMouseEnter={(e: React.MouseEvent<HTMLButtonElement>) => {
                          e.currentTarget.style.background = 'var(--md-surface-container)';
                          e.currentTarget.style.color = '#10b981';
                          e.currentTarget.style.transform = 'scale(0.95)';
                        }}
                        onMouseLeave={(e: React.MouseEvent<HTMLButtonElement>) => {
                          e.currentTarget.style.background = 'var(--md-surface-container-low)';
                          e.currentTarget.style.color = '#6b7280';
                          e.currentTarget.style.transform = 'scale(1)';
                        }}
                      ><IconPencil size={13} /></button>
                      <button
                        onClick={() => setDeleteTag(tag)}
                        title={t('deleteTag')}
                        style={{
                          ...iconBtn,
                          color: '#6b7280',
                        }}
                        onMouseEnter={(e: React.MouseEvent<HTMLButtonElement>) => {
                          e.currentTarget.style.background = 'var(--md-surface-container)';
                          e.currentTarget.style.color = '#ef4444';
                          e.currentTarget.style.transform = 'scale(0.95)';
                        }}
                        onMouseLeave={(e: React.MouseEvent<HTMLButtonElement>) => {
                          e.currentTarget.style.background = 'var(--md-surface-container-low)';
                          e.currentTarget.style.color = '#6b7280';
                          e.currentTarget.style.transform = 'scale(1)';
                        }}
                      ><IconTrash2 size={13} /></button>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <EmptyState icon={<IconTag size={28} />} message={t('noTags')}
              action={<Button size="sm" onClick={openCreate}><IconPlus size={14} /> {t('createFirstTag')}</Button>} />
          )}
        </div>
      </Card>

      {/* 单个删除确认 */}
      <ConfirmDialog
        open={!!deleteTag}
        onClose={() => setDeleteTag(null)}
        onConfirm={confirmDelete}
        title={t('deleteTagTitle')}
        message={format('deleteTagMessage', { name: deleteTag?.name || '' })}
        variant="danger"
        confirmText={t('delete')}
      />

      {/* 批量删除确认 */}
      <ConfirmDialog
        open={batchDeleteOpen}
        onClose={() => setBatchDeleteOpen(false)}
        onConfirm={() => batchDeleteMutation.mutate([...selectedIds])}
        title={t('batchDeleteTagTitle')}
        message={format('batchDeleteTagMessage', { count: selectedIds.size })}
        variant="danger"
        confirmText={format('deleteCountConfirm', { count: selectedIds.size })}
      />

      {/* 新建/编辑 Modal */}
      <Modal
        open={editorOpen}
        onClose={closeEditor}
        title={editingTag ? t('editTagTitle') : t('createTagTitle')}
        width="420px"
        actions={
          <>
            <Button variant="ghost" onClick={closeEditor}>{t('cancel')}</Button>
            <Button onClick={handleSave} disabled={saveMutation.isPending} loading={saveMutation.isPending}>{t('save')}</Button>
          </>
        }
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: '18px' }}>
          <Input
            label={t('categoryName')}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t('tagNamePlaceholder')}
            autoFocus
          />
          <Input
            label={t('slugLabel')}
            value={slug}
            onChange={(e) => setSlug(e.target.value)}
            placeholder={t('tagSlugPlaceholder')}
          />
        </div>
      </Modal>
    </>
  );
}
