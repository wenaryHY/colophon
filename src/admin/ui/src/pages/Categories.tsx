import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation } from '@tanstack/react-query';
import { apiData, API_PREFIX, getQueryClient } from '../lib/api';
import { esc } from '../lib/utils';
import type { Category } from '../types';
import { PageHeader } from '../components/PageHeader';
import { Card } from '../components/Card';
import { Button } from '../components/Button';
import { Modal } from '../components/Modal';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { Input } from '../components/Input';
import { Textarea } from '../components/Textarea';
import { EmptyState } from '../components/EmptyState';
import { CardTableSkeleton } from '../components/Skeleton';
import { IconFolderOpen, IconPlus, IconPencil, IconTrash2, IconCheck } from '../components/Icons';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';

/* MD3 样式 */
const TH = {
  padding: '14px 16px', textAlign: 'left' as const, fontSize: '11.5px',
  fontWeight: 700, color: 'var(--md-on-surface-variant)', textTransform: 'uppercase' as const,
  letterSpacing: '0.06em', background: 'var(--md-surface-container-low)',
};
const TD = {
  padding: '15px 16px', fontSize: '13.5px', color: 'var(--md-on-surface)',
  verticalAlign: 'middle',
};
const iconBtn: React.CSSProperties = {
  display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
  width: '32px', height: '32px', borderRadius: 'var(--radius-full)',
  border: 'none', cursor: 'pointer', transition: 'all 0.15s ease',
  background: 'var(--md-surface-container-low)',
};

export default function Categories() {
  const toast = useToast();
  const { t, format } = useI18n();
  const [editorOpen, setEditorOpen] = useState(false);
  const [editing, setEditing] = useState<Category | null>(null);
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [desc, setDesc] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; name: string } | null>(null);
  const navigate = useNavigate();

  // 批量选择
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [batchDeleteOpen, setBatchDeleteOpen] = useState(false);

  const { data: items = [], isLoading } = useQuery({
    queryKey: ['categories'],
    queryFn: () => apiData<Category[]>(`${API_PREFIX}/categories`),
  });

  const saveMutation = useMutation({
    mutationFn: async ({ isEdit, catId, body }: {
      isEdit: boolean;
      catId?: string;
      body: { name: string; slug?: string; description: string | null };
    }) => {
      if (isEdit) {
        return apiData(`${API_PREFIX}/admin/categories/${catId}`, { method: 'PATCH', body: JSON.stringify(body) });
      }
      return apiData(`${API_PREFIX}/admin/categories`, { method: 'POST', body: JSON.stringify(body) });
    },
    onSuccess: () => {
      toast(t('saveSuccess'), 'success');
      closeEditor();
      getQueryClient().invalidateQueries({ queryKey: ['categories'] });
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('saveFailed'), 'error');
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (catId: string) =>
      apiData(`${API_PREFIX}/admin/categories/${catId}`, { method: 'DELETE' }),
    onSuccess: (_, catId) => {
      toast(t('deleteSuccess'), 'success');
      setSelectedIds(prev => { const next = new Set(prev); next.delete(catId); return next; });
      setDeleteTarget(null);
      getQueryClient().invalidateQueries({ queryKey: ['categories'] });
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('deleteFailed'), 'error');
      setDeleteTarget(null);
    },
  });

  const batchDeleteMutation = useMutation({
    mutationFn: async (ids: string[]) => {
      await Promise.all(ids.map(id =>
        apiData(`${API_PREFIX}/admin/categories/${id}`, { method: 'DELETE' })
      ));
    },
    onSuccess: (_, ids) => {
      toast(format('batchDeleteCategoriesSuccess', { count: ids.length }), 'success');
      setSelectedIds(new Set());
      setBatchDeleteOpen(false);
      getQueryClient().invalidateQueries({ queryKey: ['categories'] });
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('batchDeleteFailed'), 'error');
      setBatchDeleteOpen(false);
    },
  });

  function openEditor(item?: Category) {
    setEditing(item || null); setName(item?.name || ''); setSlug(item?.slug || '');
    setDesc(item?.description || ''); setEditorOpen(true);
  }

  function closeEditor() {
    setEditorOpen(false); setEditing(null); setName(''); setSlug(''); setDesc('');
  }

  function handleSave() {
    if (!name.trim()) { toast(t('categoryNameRequired'), 'error'); return; }
    const body = { name: name.trim(), slug: slug || undefined, description: desc || null };
    saveMutation.mutate({
      isEdit: !!editing?.id,
      catId: editing?.id,
      body,
    });
  }

  function confirmDelete() {
    if (!deleteTarget) return;
    deleteMutation.mutate(deleteTarget.id);
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

  if (isLoading) return <CardTableSkeleton cols={4} rows={4} />;

  return (
    <>
      <PageHeader
        title={t('categoriesTitle')}
        subtitle={format('categoriesCount', { count: items.length })}
        actions={
          <div style={{ display: 'flex', gap: '8px' }}>
            {selectedIds.size > 0 && (
              <Button variant="danger" onClick={() => setBatchDeleteOpen(true)}>
                <IconTrash2 size={14} /> {format('batchDeleteCategories', { count: selectedIds.size })}
              </Button>
            )}
            <Button onClick={() => openEditor()}><IconPlus size={14} /> {t('newCategory')}</Button>
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
          {t('activeCategories')}
        </button>
        <button
          onClick={() => navigate('/trash?tab=category')}
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
        <div style={{ overflowX: 'auto' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={{ ...TH, width: '44px', textAlign: 'center' as const }}>
                  <button
                    onClick={toggleSelectAll}
                    style={{
                      width: '18px', height: '18px', borderRadius: '4px',
                      border: `1.5px solid ${selectedIds.size === items.length && items.length > 0 ? 'var(--md-primary)' : 'var(--md-outline)'}`,
                      background: selectedIds.size === items.length && items.length > 0 ? 'var(--md-primary)' : 'transparent',
                      cursor: 'pointer', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                      transition: 'all 0.15s ease',
                    }}
                  >
                    {selectedIds.size === items.length && items.length > 0 && <IconCheck size={12} color="#fff" />}
                  </button>
                </th>
                <th style={TH}>{t('categoryName')}</th>
                <th style={TH}>{t('slugLabel')}</th>
                <th style={TH}>{t('descriptionLabel')}</th>
                <th style={{ ...TH, width: '100px', textAlign: 'right' as const }}>{t('actionsLabel')}</th>
              </tr>
            </thead>
            <tbody>
              {items.length > 0 ? items.map((cat) => {
                const isSelected = selectedIds.has(cat.id);
                return (
                  <tr key={cat.id}
                    onMouseEnter={e => { if (!isSelected) e.currentTarget.style.background = 'var(--md-surface-container)'; }}
                    onMouseLeave={e => { if (!isSelected) e.currentTarget.style.background = 'transparent'; }}
                    style={{ transition: 'background 0.12s ease', background: isSelected ? 'var(--md-primary-container)' : 'transparent' }}
                  >
                    <td style={{ ...TD, textAlign: 'center' }}>
                      <button
                        onClick={() => toggleSelect(cat.id)}
                        style={{
                          width: '18px', height: '18px', borderRadius: '4px',
                          border: `1.5px solid ${isSelected ? 'var(--md-primary)' : 'var(--md-outline)'}`,
                          background: isSelected ? 'var(--md-primary)' : 'transparent',
                          cursor: 'pointer', display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                          transition: 'all 0.15s ease',
                        }}
                      >
                        {isSelected && <IconCheck size={12} color="#fff" />}
                      </button>
                    </td>
                    <td style={{ ...TD, fontWeight: 600 }}>{esc(cat.name)}</td>
                    <td style={TD}>
                      <span style={{
                        background: 'var(--md-surface-container)', padding: '3px 8px', borderRadius: '6px',
                        fontSize: '12px', fontFamily: 'monospace', color: 'var(--md-outline)',
                      }}>{esc(cat.slug)}</span>
                    </td>
                    <td style={{ ...TD, color: 'var(--md-on-surface-variant)' }}>{esc(cat.description || '-')}</td>
                    <td style={TD}>
                      <div style={{ display: 'flex', gap: '4px', justifyContent: 'flex-end', alignItems: 'center' }}>
                        <button
                          onClick={() => openEditor(cat)}
                          title={t('editCategory')}
                          style={{ ...iconBtn, color: '#10b981' }}
                          onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-surface-container)'; e.currentTarget.style.transform = 'scale(0.95)'; }}
                          onMouseLeave={e => { e.currentTarget.style.background = 'var(--md-surface-container-low)'; e.currentTarget.style.transform = 'scale(1)'; }}
                        ><IconPencil size={16} /></button>
                        <button
                          onClick={() => setDeleteTarget({ id: cat.id, name: cat.name })}
                          title={t('deleteCategory')}
                          style={{ ...iconBtn, color: '#ef4444' }}
                          onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-surface-container)'; e.currentTarget.style.transform = 'scale(0.95)'; }}
                          onMouseLeave={e => { e.currentTarget.style.background = 'var(--md-surface-container-low)'; e.currentTarget.style.transform = 'scale(1)'; }}
                        ><IconTrash2 size={16} /></button>
                      </div>
                    </td>
                  </tr>
                );
              }) : (
                <tr>
                  <td colSpan={5}>
                    <EmptyState icon={<IconFolderOpen size={28} />} message={t('noCategories')} />
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </Card>

      <ConfirmDialog open={!!deleteTarget} onClose={() => setDeleteTarget(null)} onConfirm={confirmDelete}
        title={t('deleteCategoryTitle')} message={format('deleteCategoryMessage', { name: deleteTarget?.name || '' })} variant="danger" confirmText={t('delete')} />

      <ConfirmDialog open={batchDeleteOpen} onClose={() => setBatchDeleteOpen(false)} onConfirm={() => batchDeleteMutation.mutate([...selectedIds])}
        title={t('batchDeleteCategoryTitle')} message={format('batchDeleteCategoryMessage', { count: selectedIds.size })}
        variant="danger" confirmText={format('deleteCountConfirm', { count: selectedIds.size })} />

      <Modal open={editorOpen} onClose={closeEditor} title={editing ? t('editCategoryTitle') : t('createCategoryTitle')}
        actions={<><Button variant="ghost" onClick={closeEditor}>{t('cancel')}</Button><Button onClick={handleSave} disabled={saveMutation.isPending} loading={saveMutation.isPending}>{t('save')}</Button></>}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '18px' }}>
          <Input label={t('categoryName')} value={name} onChange={(e) => setName(e.target.value)} placeholder={t('categoryNamePlaceholder')} />
          <Input label={t('slugLabel')} value={slug} onChange={(e) => setSlug(e.target.value)} placeholder={t('categorySlugPlaceholder')} />
          <Textarea label={t('descriptionLabel')} value={desc} onChange={(e) => setDesc(e.target.value)} placeholder={t('categoryDescPlaceholder')} minRows={3} />
        </div>
      </Modal>
    </>
  );
}
