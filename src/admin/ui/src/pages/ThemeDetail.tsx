import { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useQuery, useMutation } from '@tanstack/react-query';
import { PageHeader } from '../components/PageHeader';
import { Button } from '../components/Button';
import { Input } from '../components/Input';
import { Select } from '../components/Select';
import { IconEye } from '../components/Icons';
import { getThemeDetail, saveThemeConfig, activateTheme } from '../lib/api';
import { getQueryClient } from '../lib/api';
import type { ThemeConfigField } from '../types';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';
import { usePreview, PreviewRenderer } from '../preview';
import { FabContainer } from '../fab';

const sectionStyle: React.CSSProperties = {
  background: 'var(--md-surface-container-lowest)',
  borderRadius: 'var(--radius-lg)',
  overflow: 'hidden',
};

export default function ThemeDetail() {
  const { slug } = useParams<{ slug: string }>();
  const navigate = useNavigate();
  const toast = useToast();
  const { t, format } = useI18n();

  const [saving, setSaving] = useState(false);
  const [activating, setActivating] = useState(false);
  const [formData, setFormData] = useState<Record<string, unknown>>({});
  const [showPreview, setShowPreview] = useState(false);

  // 预览上下文
  const preview = usePreview();

  const { data: detail, isLoading } = useQuery({
    queryKey: ['theme-detail', slug],
    queryFn: () => getThemeDetail(slug!),
    enabled: !!slug,
    staleTime: 0,
  });

  // 当 detail 变化时初始化 formData
  const currentFormData = detail ? detail.config ?? {} : formData;

  // 注册预览场景
  useEffect(() => {
    if (detail) {
      preview.registerScene('theme-detail', {
        getContent: () => JSON.stringify(currentFormData),
        getContentType: () => 'json',
        getTheme: () => slug ?? 'default',
        getThemeConfig: () => currentFormData,
      });
    }
  }, [detail, currentFormData, slug, preview.registerScene]);

  const saveMutation = useMutation({
    mutationFn: async () => {
      if (!slug) return;
      await saveThemeConfig(slug, currentFormData);
    },
    onSuccess: () => {
      toast(t('saveThemeConfigSuccess'), 'success');
      setSaving(false);
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('saveThemeConfigFailed'), 'error');
      setSaving(false);
    },
  });

  const activateMutation = useMutation({
    mutationFn: async () => {
      if (!slug) return;
      await activateTheme(slug);
    },
    onSuccess: () => {
      toast(t('activateThemeSuccess'), 'success');
      setActivating(false);
      getQueryClient().invalidateQueries({ queryKey: ['theme-detail', slug] });
      getQueryClient().invalidateQueries({ queryKey: ['themes'] });
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('activateThemeFailed'), 'error');
      setActivating(false);
    },
  });

  function handleSave() {
    setSaving(true);
    saveMutation.mutate();
  }

  function handleActivate() {
    setActivating(true);
    activateMutation.mutate();
  }

  if (isLoading) {
    return <div style={{ padding: '20px', textAlign: 'center' }}>{t('loading')}</div>;
  }

  if (!detail) {
    return <div style={{ padding: '20px', textAlign: 'center', color: 'var(--md-outline)' }}>{t('themeNotFound')}</div>;
  }

  const { manifest, schema } = detail;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '20px', height: '100%' }}>
      <PageHeader
        title={manifest.name}
        subtitle={manifest.description}
        actions={
          <div style={{ display: 'flex', gap: '12px' }}>
            <Button
              onClick={() => void handleActivate()}
              loading={activating}
              variant="primary"
            >
              {t('activateTheme')}
            </Button>
            <Button onClick={() => navigate('/themes')} variant="ghost">
              {t('backToList')}
            </Button>
          </div>
        }
      />

      <div style={{ flex: 1, display: 'flex', gap: '20px', minHeight: 0 }}>
        {/* 左侧：主题信息 + 配置 */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '20px', overflowY: 'auto' }}>
          <section style={sectionStyle}>
            <div style={{ padding: '20px 24px', background: 'var(--md-surface-container-low)' }}>
              <div style={{ fontSize: '15px', fontWeight: 700, color: 'var(--md-on-surface)' }}>{t('themeInfo')}</div>
            </div>
            <div style={{ padding: '20px 24px', display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '16px' }}>
              <div>
                <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginBottom: '6px' }}>{t('identifierLabel')}</div>
                <div style={{ fontSize: '14px', fontFamily: 'monospace', color: 'var(--md-on-surface)' }}>{manifest.slug}</div>
              </div>
              <div>
                <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginBottom: '6px' }}>{t('versionLabel')}</div>
                <div style={{ fontSize: '14px', color: 'var(--md-on-surface)' }}>v{manifest.version}</div>
              </div>
              <div>
                <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginBottom: '6px' }}>{t('authorText')}</div>
                <div style={{ fontSize: '14px', color: 'var(--md-on-surface)' }}>{manifest.author || t('unknown')}</div>
              </div>
              <div>
                <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginBottom: '6px' }}>{t('minVersionText')}</div>
                <div style={{ fontSize: '14px', color: 'var(--md-on-surface)' }}>{manifest.min_inkforge_version || t('undeclared')}</div>
              </div>
            </div>
          </section>

          {Object.keys(schema).length > 0 && (
            <section style={sectionStyle}>
              <div style={{ padding: '20px 24px', background: 'var(--md-surface-container-low)' }}>
                <div style={{ fontSize: '15px', fontWeight: 700, color: 'var(--md-on-surface)' }}>{t('themeConfig')}</div>
                <div style={{ fontSize: '12.5px', color: 'var(--md-outline)', marginTop: '4px' }}>
                  {t('themeConfigDesc')}
                </div>
              </div>
              <div style={{ padding: '20px 24px', display: 'flex', flexDirection: 'column', gap: '16px' }}>
                {Object.entries(schema).map(([key, field]) => (
                  <ThemeConfigFieldInput
                    key={key}
                    field={field}
                    value={currentFormData[key]}
                    onChange={(val) => setFormData({ ...currentFormData, [key]: val })}
                    t={t}
                    format={format}
                  />
                ))}
                <div style={{ display: 'flex', gap: '12px', marginTop: '12px' }}>
                  <Button onClick={() => void handleSave()} loading={saving}>
                    {t('saveConfig')}
                  </Button>
                  <Button onClick={() => setFormData(detail.config)} variant="ghost">
                    {t('resetConfig')}
                  </Button>
                </div>
              </div>
            </section>
          )}
        </div>

        {/* 实时预览面板 */}
        <PreviewRenderer
          mode="inline"
          visible={showPreview}
        />
      </div>

      {/* FAB 浮动预览按钮 */}
      <FabContainer
        actions={[
          {
            id: 'preview',
            icon: <IconEye size={20} />,
            label: t('preview'),
            onClick: () => setShowPreview(!showPreview),
          },
        ]}
      />
    </div>
  );
}

interface ThemeConfigFieldInputProps {
  field: ThemeConfigField;
  value: unknown;
  onChange: (val: unknown) => void;
  t: (key: string) => string;
  format: (key: string, params?: Record<string, string | number>, fallback?: string) => string;
}

function ThemeConfigFieldInput({ field, value, onChange, t, format }: ThemeConfigFieldInputProps) {
  if (field.type === 'text') {
    return (
      <Input
        label={field.label}
        value={(value as string) || field.default || ''}
        onChange={(e) => onChange(e.target.value)}
        placeholder={field.default ? format('defaultValuePrefix', { value: field.default }) : ''}
      />
    );
  }

  if (field.type === 'color') {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
        <label style={{ fontSize: '13px', fontWeight: 600, color: 'var(--md-on-surface-variant)' }}>
          {field.label}
        </label>
        <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
          <input
            type="color"
            value={(value as string) || field.default || '#000000'}
            onChange={(e) => onChange(e.target.value)}
            style={{
              width: '60px',
              height: '40px',
              border: 'none',
              borderRadius: 'var(--radius-md)',
              cursor: 'pointer',
              background: 'var(--md-surface-container)',
            }}
          />
          <span style={{ fontSize: '13px', color: 'var(--md-on-surface-variant)', fontFamily: 'monospace' }}>
            {(value as string) || field.default || '#000000'}
          </span>
        </div>
      </div>
    );
  }

  if (field.type === 'number') {
    return (
      <Input
        label={field.label}
        type="number"
        value={(value as number) || field.default || 0}
        onChange={(e) => onChange(parseInt(e.target.value, 10))}
        placeholder={field.default ? format('defaultValuePrefix', { value: field.default }) : ''}
      />
    );
  }

  if (field.type === 'select') {
    return (
      <Select
        label={field.label}
        value={(value as string) || field.default || ''}
        onChange={(e) => onChange(e.target.value)}
      >
        <option value="">{t('pleaseSelectOption')}</option>
        {field.options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </Select>
    );
  }

  return null;
}
