import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { PageHeader } from '../components/PageHeader';
import { Card } from '../components/Card';
import { Button } from '../components/Button';
import { Input } from '../components/Input';
import { Textarea } from '../components/Textarea';
import { Modal } from '../components/Modal';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { EmptyState } from '../components/EmptyState';
import { CardTableSkeleton } from '../components/Skeleton';
import { useToast } from '../contexts/ToastContext';
import { useMediaCategories } from '../hooks/useMediaCategories';
import { useI18n } from '../i18n';
import type { CreateMediaCategoryRequest, MediaCategory, UpdateMediaCategoryRequest } from '../types';
import { esc } from '../lib/utils';
import { IconFolder, IconPlus, IconEdit2, IconTrash2, IconImage, IconFileText, IconArchive, IconFolderOpen } from '../components/Icons';

const TH = {
  padding: '14px 16px',
  textAlign: 'left' as const,
  fontSize: '11.5px',
  fontWeight: 700,
  color: 'var(--md-on-surface-variant)',
  textTransform: 'uppercase' as const,
  letterSpacing: '0.06em',
  background: 'var(--md-surface-container-low)',
};

const TD = {
  padding: '15px 16px',
  fontSize: '13.5px',
  color: 'var(--md-on-surface)',
  verticalAlign: 'middle' as const,
};

const iconBtn: React.CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: '32px',
  height: '32px',
  borderRadius: 'var(--radius-full)',
  border: 'none',
  cursor: 'pointer',
  transition: 'all 0.15s ease',
  background: 'var(--md-surface-container-low)',
};

const INITIAL_FORM = {
  name: '',
  slug: '',
  description: '',
  icon: '',
  color: '#ff6b35',
  sort_order: '0',
};

const getPredefinedIcons = (t: (key: string) => string) => [
  { id: 'folder', name: t('folderIcon'), icon: <IconFolder size={18} /> },
  { id: 'folder-open', name: t('folderOpenIcon'), icon: <IconFolderOpen size={18} /> },
  { id: 'image', name: t('imageIcon'), icon: <IconImage size={18} /> },
  { id: 'file', name: t('fileIcon'), icon: <IconFileText size={18} /> },
  { id: 'archive', name: t('archiveIcon'), icon: <IconArchive size={18} /> },
];

export default function MediaCategories() {
  const { t, format } = useI18n();
  const toast = useToast();
  const navigate = useNavigate();
  const { categories, loading, fetch, create, update, remove } = useMediaCategories();
  const PREDEFINED_ICONS = useMemo(() => getPredefinedIcons(t), [t]);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editing, setEditing] = useState<MediaCategory | null>(null);
  const [saving, setSaving] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<MediaCategory | null>(null);
  const [form, setForm] = useState(INITIAL_FORM);

  const sortedCategories = useMemo(
    () => [...categories].sort((a, b) => a.sort_order - b.sort_order || a.name.localeCompare(b.name)),
    [categories],
  );

  function updateForm<K extends keyof typeof INITIAL_FORM>(key: K, value: (typeof INITIAL_FORM)[K]) {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  function resetEditor() {
    setEditorOpen(false);
    setEditing(null);
    setSaving(false);
    setForm(INITIAL_FORM);
  }

  function openCreate() {
    setEditing(null);
    setForm(INITIAL_FORM);
    setEditorOpen(true);
  }

  function openEdit(item: MediaCategory) {
    setEditing(item);
    setForm({
      name: item.name,
      slug: item.slug,
      description: item.description || '',
      icon: item.icon || '',
      color: item.color || '#ff6b35',
      sort_order: String(item.sort_order ?? 0),
    });
    setEditorOpen(true);
  }

  async function handleSave() {
    if (!form.name.trim()) {
      toast(t('mediaCategoryNameRequired'), 'error');
      return;
    }

    const sortOrder = Number(form.sort_order || '0');
    if (Number.isNaN(sortOrder)) {
      toast(t('sortOrderInvalid'), 'error');
      return;
    }

    const payload: CreateMediaCategoryRequest | UpdateMediaCategoryRequest = {
      name: form.name.trim(),
      slug: form.slug.trim() || undefined,
      description: form.description.trim() || undefined,
      icon: form.icon.trim() || undefined,
      color: form.color.trim() || undefined,
      sort_order: sortOrder,
    };

    setSaving(true);
    try {
      if (editing) {
        await update(editing.id, payload);
        toast(t('mediaCategoryUpdated'), 'success');
      } else {
        await create(payload as CreateMediaCategoryRequest);
        toast(t('mediaCategoryCreated'), 'success');
      }
      resetEditor();
    } catch (error) {
      toast(error instanceof Error ? error.message : t('saveMediaCategoryFailed'), 'error');
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete() {
    if (!deleteTarget) return;
    try {
      await remove(deleteTarget.id);
      toast(t('mediaCategoryDeleted'), 'success');
    } catch (error) {
      toast(error instanceof Error ? error.message : t('deleteMediaCategoryFailed'), 'error');
    } finally {
      setDeleteTarget(null);
    }
  }

  if (loading && categories.length === 0) {
    return <CardTableSkeleton cols={5} rows={4} />;
  }

  return (
    <>
      <PageHeader
        title={t('mediaCategoriesTitle')}
        subtitle={format('mediaCategoriesSubtitle', { count: categories.length })}
        actions={
          <Button onClick={openCreate}>
            <IconPlus size={14} /> {t('newMediaCategory')}
          </Button>
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
          {t('activeMediaCategories')}
        </button>
        <button
          onClick={() => navigate('/trash?tab=media_category')}
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
        <div style={{ padding: '18px 22px', display: 'flex', justifyContent: 'space-between', gap: '16px', flexWrap: 'wrap' }}>
          <div>
            <div style={{ fontSize: '14px', fontWeight: 700, color: 'var(--md-on-surface)' }}>{t('mediaCategoryListTitle')}</div>
            <div style={{ fontSize: '12.5px', color: 'var(--md-outline)', marginTop: '4px' }}>
              {t('mediaCategoryListDesc')}
            </div>
          </div>
          <Button variant="ghost" onClick={() => fetch()}>
            {t('refreshList')}
          </Button>
        </div>

        <div style={{ overflowX: 'auto' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={TH}>{t('mediaCategoryName')}</th>
                <th style={TH}>{t('mediaCategorySlug')}</th>
                <th style={TH}>{t('mediaCategoryStyle')}</th>
                <th style={TH}>{t('mediaCategorySort')}</th>
                <th style={TH}>{t('mediaCategoryDesc')}</th>
                <th style={{ ...TH, width: '104px', textAlign: 'right' as const }}>{t('actionsLabel')}</th>
              </tr>
            </thead>
            <tbody>
              {sortedCategories.length > 0 ? sortedCategories.map((item) => (
                <tr key={item.id}
                  onMouseEnter={e => e.currentTarget.style.background = 'var(--md-surface-container)'}
                  onMouseLeave={e => e.currentTarget.style.background = 'transparent'}
                  style={{ transition: 'background 0.12s ease' }}
                >
                  <td style={{ ...TD, fontWeight: 600 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                      <span
                        style={{
                          width: '28px',
                          height: '28px',
                          borderRadius: '8px',
                          display: 'inline-flex',
                          alignItems: 'center',
                          justifyContent: 'center',
                          background: item.color || 'var(--md-surface-container)',
                          color: '#fff',
                          flexShrink: 0,
                        }}
                      >
                        {item.icon === 'folder-open' ? <IconFolderOpen size={14} /> :
                         item.icon === 'image' ? <IconImage size={14} /> :
                         item.icon === 'file' ? <IconFileText size={14} /> :
                         item.icon === 'archive' ? <IconArchive size={14} /> :
                         <IconFolder size={14} />}
                      </span>
                      <span>{esc(item.name)}</span>
                    </div>
                  </td>
                  <td style={TD}>
                    <span style={{ background: 'var(--md-surface-container)', padding: '3px 8px', borderRadius: '6px', fontSize: '12px', fontFamily: 'monospace', color: 'var(--md-outline)' }}>
                      {esc(item.slug)}
                    </span>
                  </td>
                  <td style={TD}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <span style={{ width: '14px', height: '14px', borderRadius: '999px', background: item.color || '#d1d5db' }} />
                      <span style={{ fontSize: '12.5px', color: 'var(--md-on-surface-variant)' }}>
                        {PREDEFINED_ICONS.find(i => i.id === item.icon)?.name || t('defaultIcon')}
                      </span>
                    </div>
                  </td>
                  <td style={TD}>{item.sort_order}</td>
                  <td style={{ ...TD, color: 'var(--md-on-surface-variant)' }}>{esc(item.description || '—')}</td>
                  <td style={TD}>
                    <div style={{ display: 'flex', gap: '4px', justifyContent: 'flex-end' }}>
                      <button
                        type="button"
                        onClick={() => openEdit(item)}
                        title={t('editMediaCategory')}
                        style={{ ...iconBtn, color: '#10b981' }}
                        onMouseEnter={(e) => {
                          e.currentTarget.style.background = 'var(--md-surface-container)';
                          e.currentTarget.style.transform = 'scale(0.95)';
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.background = 'var(--md-surface-container-low)';
                          e.currentTarget.style.transform = 'scale(1)';
                        }}
                      >
                        <IconEdit2 size={16} />
                      </button>
                      <button
                        type="button"
                        onClick={() => setDeleteTarget(item)}
                        title={t('deleteMediaCategory')}
                        style={{ ...iconBtn, color: '#ef4444' }}
                        onMouseEnter={(e) => {
                          e.currentTarget.style.background = 'var(--md-surface-container)';
                          e.currentTarget.style.transform = 'scale(0.95)';
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.background = 'var(--md-surface-container-low)';
                          e.currentTarget.style.transform = 'scale(1)';
                        }}
                      >
                        <IconTrash2 size={16} />
                      </button>
                    </div>
                  </td>
                </tr>
              )) : (
                <tr>
                  <td colSpan={6}>
                    <EmptyState icon={<IconFolder size={28} />} message={t('noMediaCategories')} />
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </Card>

      <Modal
        open={editorOpen}
        onClose={resetEditor}
        title={editing ? t('editMediaCategoryTitle') : t('createMediaCategoryTitle')}
        actions={(
          <>
            <Button variant="ghost" onClick={resetEditor}>{t('cancel')}</Button>
            <Button onClick={handleSave} disabled={saving} loading={saving}>{t('save')}</Button>
          </>
        )}
      >
        <div style={{ display: 'grid', gap: '16px' }}>
          <Input label={t('mediaCategoryName')} value={form.name} onChange={(e) => updateForm('name', e.target.value)} placeholder={t('mediaCategoryNamePlaceholder')} />
          <Input label={t('mediaCategorySlug')} value={form.slug} onChange={(e) => updateForm('slug', e.target.value)} placeholder={t('mediaCategorySlugPlaceholder')} />
          <Textarea label={t('mediaCategoryDesc')} value={form.description} onChange={(e) => updateForm('description', e.target.value)} placeholder={t('mediaCategoryDescPlaceholder')} minRows={3} />
            <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
              <label style={{ fontSize: '13px', fontWeight: 600, color: 'var(--md-on-surface-variant)' }}>{t('iconLabel')}</label>
              <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                {PREDEFINED_ICONS.map(icon => (
                  <button
                    key={icon.id}
                    type="button"
                    title={icon.name}
                    onClick={() => updateForm('icon', icon.id)}
                    style={{
                      width: '40px', height: '40px', borderRadius: '8px',
                      display: 'flex', alignItems: 'center', justifyContent: 'center',
                      background: form.icon === icon.id ? 'var(--md-primary-container)' : 'var(--md-surface-container)',
                      color: form.icon === icon.id ? 'var(--md-primary-dim)' : 'var(--md-on-surface-variant)',
                      border: 'none',
                      cursor: 'pointer', transition: 'all 0.15s ease',
                      transform: form.icon === icon.id ? 'scale(0.95)' : 'scale(1)',
                    }}
                  >
                    {icon.icon}
                  </button>
                ))}
              </div>
            </div>
            <Input label={t('sortOrderLabel')} type="number" value={form.sort_order} onChange={(e) => updateForm('sort_order', e.target.value)} placeholder={t('sortOrderHint')} />
          <div style={{ display: 'grid', gridTemplateColumns: '1fr auto', gap: '16px', alignItems: 'end' }}>
            <Input label={t('colorLabel')} value={form.color} onChange={(e) => updateForm('color', e.target.value)} placeholder="#ff6b35" />
            <div style={{ display: 'flex', alignItems: 'center', gap: '10px', height: '40px' }}>
              <span style={{ fontSize: '12.5px', color: 'var(--md-outline)' }}>{t('previewLabel')}</span>
              <span style={{ width: '40px', height: '40px', borderRadius: '10px', background: form.color || '#ff6b35', display: 'inline-flex', alignItems: 'center', justifyContent: 'center', color: '#fff' }}>
                {form.icon === 'folder-open' ? <IconFolderOpen size={20} /> :
                 form.icon === 'image' ? <IconImage size={20} /> :
                 form.icon === 'file' ? <IconFileText size={20} /> :
                 form.icon === 'archive' ? <IconArchive size={20} /> :
                 <IconFolder size={20} />}
              </span>
            </div>
          </div>
        </div>
      </Modal>

      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleDelete}
        title={t('deleteMediaCategoryTitle')}
        message={format('deleteMediaCategoryMessage', { name: deleteTarget?.name || '' })}
        variant="danger"
        confirmText={t('deletePost')}
      />
    </>
  );
}
