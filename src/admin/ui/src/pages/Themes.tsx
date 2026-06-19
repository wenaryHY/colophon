import { useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation } from '@tanstack/react-query';
import { PageHeader } from '../components/PageHeader';
import { Button } from '../components/Button';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { apiData, API_PREFIX, deleteTheme, getQueryClient, uploadTheme } from '../lib/api';
import type { ThemeConfigField, ThemeSummary } from '../types';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';

const sectionStyle: React.CSSProperties = {
  background: 'var(--md-surface-container-lowest)',
  borderRadius: 'var(--radius-lg)',
  overflow: 'hidden',
};

const labelMap = (t: (key: string) => string): Record<string, string> => ({
  text: t('fieldTypeText'),
  color: t('fieldTypeColor'),
  select: t('fieldTypeSelect'),
  number: t('fieldTypeNumber'),
});

export default function Themes() {
  const navigate = useNavigate();
  const toast = useToast();
  const { t, format } = useI18n();
  const [activatingSlug, setActivatingSlug] = useState<string | null>(null);
  const [isUploading, setIsUploading] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<{ slug: string; name: string } | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const { data: themes = [], isLoading } = useQuery({
    queryKey: ['themes'],
    queryFn: () => apiData<ThemeSummary[]>(`${API_PREFIX}/admin/themes`),
  });

  const activateMutation = useMutation({
    mutationFn: (slug: string) =>
      apiData(`${API_PREFIX}/admin/themes/${slug}/activate`, { method: 'POST' }),
    onSuccess: () => {
      toast(t('switchThemeSuccess'), 'success');
      setActivatingSlug(null);
      getQueryClient().invalidateQueries({ queryKey: ['themes'] });
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('switchThemeFailed'), 'error');
      setActivatingSlug(null);
    },
  });

  const handleActivate = (slug: string) => {
    setActivatingSlug(slug);
    activateMutation.mutate(slug);
  };

  const deleteMutation = useMutation({
    mutationFn: (slug: string) => deleteTheme(slug),
    onSuccess: (_data, _slug) => {
      toast(t('deleteThemeSuccess'), 'success');
      getQueryClient().invalidateQueries({ queryKey: ['themes'] });
    },
    onError: (error: Error) => {
      toast(error.message || t('deleteThemeFailed'), 'error');
    },
  });

  const handleDeleteClick = (slug: string, name: string) => {
    setDeleteTarget({ slug, name });
  };

  const handleConfirmDelete = () => {
    if (!deleteTarget) return;
    deleteMutation.mutate(deleteTarget.slug);
    setDeleteTarget(null);
  };

  const FILE_SIZE_MAX_BYTES = 100 * 1024 * 1024;

  const handleFileChange = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    if (file.size > FILE_SIZE_MAX_BYTES) {
      toast(t('uploadThemeFileTooLarge'), 'error');
      event.target.value = '';
      return;
    }

    setIsUploading(true);
    try {
      await uploadTheme(file);
      toast(t('uploadThemeSuccess'), 'success');
      getQueryClient().invalidateQueries({ queryKey: ['themes'] });
    } catch (error) {
      const message = error instanceof Error ? error.message : t('uploadThemeFailed');
      toast(message, 'error');
    } finally {
      setIsUploading(false);
      event.target.value = '';
    }
  };

  const handleUploadClick = () => {
    fileInputRef.current?.click();
  };

  const activeTheme = useMemo(() => themes.find((item) => item.active), [themes]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
      <input
        ref={fileInputRef}
        type="file"
        accept=".zip"
        style={{ display: 'none' }}
        onChange={(e) => { void handleFileChange(e); }}
      />
      <PageHeader
        title={t('themesTitle')}
        subtitle={t('themesSubtitle')}
        actions={
          <>
            <Button onClick={handleUploadClick} loading={isUploading}>
              {isUploading ? t('uploading') : t('uploadThemeButton')}
            </Button>
            <Button onClick={() => getQueryClient().invalidateQueries({ queryKey: ['themes'] })} disabled={isLoading || isUploading} loading={isLoading}>
              {t('refreshThemes')}
            </Button>
          </>
        }
      />

      <section style={sectionStyle}>
        <div style={{ padding: '20px 24px', background: 'var(--md-surface-container-low)' }}>
          <div style={{ fontSize: '15px', fontWeight: 700, color: 'var(--md-on-surface)' }}>{t('currentStatus')}</div>
          <div style={{ fontSize: '12.5px', color: 'var(--md-outline)', marginTop: '4px' }}>
            {t('currentStatusDesc')}
          </div>
        </div>
        <div style={{ padding: '20px 24px', display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: '16px' }}>
          <div>
            <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginBottom: '6px' }}>{t('activeThemeLabel')}</div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: 'var(--md-on-surface)' }}>{activeTheme?.manifest.name || t('notSet')}</div>
            <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginTop: '4px' }}>{activeTheme?.manifest.slug || '-'}</div>
          </div>
          <div>
            <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginBottom: '6px' }}>{t('scannedThemeCount')}</div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: 'var(--md-on-surface)' }}>{themes.length}</div>
          </div>
        </div>
      </section>

      <section style={sectionStyle}>
        <div style={{ padding: '20px 24px', background: 'var(--md-surface-container-low)' }}>
          <div style={{ fontSize: '15px', fontWeight: 700, color: 'var(--md-on-surface)' }}>{t('installedThemesTitle')}</div>
          <div style={{ fontSize: '12.5px', color: 'var(--md-outline)', marginTop: '4px' }}>
            {t('installedThemesDesc')}
          </div>
        </div>
        <div style={{ padding: '24px' }}>
          {themes.length === 0 && !isLoading ? (
            <div style={{ padding: '28px', textAlign: 'center', color: 'var(--md-outline)' }}>{t('noThemes')}</div>
          ) : (
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: '16px' }}>
              {themes.map((theme) => {
                const fields = Object.entries(theme.manifest.config || {}) as Array<[string, ThemeConfigField]>;
                return (
                  <article
                    key={theme.manifest.slug}
                    style={{
                      borderRadius: 'var(--radius-lg)',
                      background: theme.active ? 'var(--md-primary-container)' : 'var(--md-surface-container)',
                      padding: '18px',
                      display: 'flex',
                      flexDirection: 'column',
                      gap: '14px',
                      transition: 'transform 0.2s ease',
                    }}
                    onMouseEnter={(e) => { if (!theme.active) e.currentTarget.style.transform = 'scale(0.97)'; }}
                    onMouseLeave={(e) => { e.currentTarget.style.transform = 'scale(1)'; }}
                  >
                    <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: '12px' }}>
                      <div>
                        <div style={{ fontSize: '16px', fontWeight: 700, color: theme.active ? 'var(--md-primary)' : 'var(--md-on-surface)' }}>
                          {theme.manifest.name}
                        </div>
                        <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginTop: '4px', fontFamily: 'monospace' }}>
                          {theme.manifest.slug} · v{theme.manifest.version}
                        </div>
                      </div>
                      {theme.active && (
                        <span style={{
                          padding: '4px 10px',
                          borderRadius: 'var(--radius-full)',
                          background: 'var(--md-primary)',
                          color: 'var(--md-on-primary)',
                          fontSize: '11px',
                          fontWeight: 700,
                        }}>
                          {t('inUse')}
                        </span>
                      )}
                    </div>

                    <p style={{ margin: 0, fontSize: '13px', color: 'var(--md-on-surface-variant)', lineHeight: 1.65 }}>
                      {theme.manifest.description || t('noDescription')}
                    </p>

                    <div style={{ display: 'grid', gap: '8px', fontSize: '12.5px' }}>
                      <div style={{ color: 'var(--md-outline)' }}>{format('authorLabel', { value: theme.manifest.author || t('unknown') })}</div>
                      <div style={{ color: 'var(--md-outline)' }}>{format('minVersionLabel', { value: theme.manifest.min_colophon_version || t('undeclared') })}</div>
                      <div style={{ color: 'var(--md-outline)' }}>{format('configCountLabel', { count: fields.length })}</div>
                    </div>

                    {fields.length > 0 && (
                      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
                        {fields.slice(0, 6).map(([key, field]) => (
                          <span
                            key={key}
                            style={{
                              padding: '5px 8px',
                              borderRadius: 'var(--radius-full)',
                              background: 'var(--md-surface-container-high)',
                              color: 'var(--md-on-surface-variant)',
                              fontSize: '11px',
                            }}
                          >
                            {key} · {labelMap(t)[field.type] || field.type}
                          </span>
                        ))}
                      </div>
                    )}

                    <div style={{ display: 'flex', flexDirection: 'column', gap: '10px', marginTop: 'auto' }}>
                      <span style={{ fontSize: '12px', color: 'var(--md-outline)' }}>
                        {theme.active ? t('themeCurrentlyActive') : t('themeSwitchHint')}
                      </span>
                      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
                        <Button
                          onClick={() => navigate(`/themes/${theme.manifest.slug}`)}
                          variant="ghost"
                        >
                          {t('themeDetail')}
                        </Button>
                        <Button
                          onClick={() => void handleActivate(theme.manifest.slug)}
                          disabled={theme.active || activatingSlug !== null}
                          loading={activatingSlug === theme.manifest.slug}
                        >
                          {theme.active ? t('themeEnabled') : t('enableTheme')}
                        </Button>
                        {!theme.active && theme.manifest.slug !== 'default' && (
                          <Button
                            variant="danger"
                            onClick={() => handleDeleteClick(theme.manifest.slug, theme.manifest.name)}
                          >
                            {t('deleteThemeButton')}
                          </Button>
                        )}
                      </div>
                    </div>
                  </article>
                );
              })}
            </div>
          )}
        </div>
      </section>

      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={handleConfirmDelete}
        title={t('deleteThemeTitle')}
        message={format('deleteThemeMessage', { name: deleteTarget?.name || '' })}
        variant="danger"
        confirmText={t('deleteThemeButton')}
      />
    </div>
  );
}
