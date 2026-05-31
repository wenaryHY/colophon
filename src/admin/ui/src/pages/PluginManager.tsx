import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation } from '@tanstack/react-query';
import { apiData, API_PREFIX, getQueryClient } from '../lib/api';
import { PageHeader } from '../components/PageHeader';
import { Button } from '../components/Button';
import { useI18n } from '../i18n';
import { useToast } from '../contexts/ToastContext';

interface PluginInfo {
  id: string;
  title: string;
  version: string;
  description?: string;
  author?: string;
  enabled: boolean;
  has_settings: boolean;
  has_admin: boolean;
}

const sectionStyle: React.CSSProperties = {
  background: 'var(--md-surface-container-lowest)',
  borderRadius: 'var(--radius-lg)',
  overflow: 'hidden',
};

const CARD_STYLE: React.CSSProperties = {
  borderRadius: 'var(--radius-lg)',
  background: 'var(--md-surface-container)',
  padding: '20px',
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
  gap: '16px',
  flexWrap: 'wrap',
};

export default function PluginManager() {
  const navigate = useNavigate();
  const toast = useToast();
  const { t } = useI18n();

  const { data: pluginData, isLoading } = useQuery({
    queryKey: ['plugins'],
    queryFn: () => apiData<{ plugins: PluginInfo[] }>(`${API_PREFIX}/admin/plugins`),
  });

  const toggleMutation = useMutation({
    mutationFn: (id: string) =>
      fetch(`${API_PREFIX}/admin/plugins/${id}/toggle`, {
        method: 'POST',
        credentials: 'include',
      }),
    onSuccess: () => {
      getQueryClient().invalidateQueries({ queryKey: ['plugins'] });
    },
    onError: (e) => {
      toast(e instanceof Error ? e.message : t('loadFailed', '操作失败'), 'error');
    },
  });

  if (isLoading) {
    return (
      <div className="setup-loading">{t('loading')}</div>
    );
  }

  const plugins = pluginData?.plugins ?? [];
  const enabledCount = plugins.filter((p) => p.enabled).length;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
      <PageHeader
        title={t('plugins')}
        subtitle={t('pluginsSubtitle')}
      />

      {/* 概览卡片 */}
      <section style={sectionStyle}>
        <div style={{ padding: '20px 24px', background: 'var(--md-surface-container-low)' }}>
          <div style={{ fontSize: '15px', fontWeight: 700, color: 'var(--md-on-surface)' }}>
            {t('currentStatus')}
          </div>
        </div>
        <div
          style={{
            padding: '20px 24px',
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(140px, 1fr))',
            gap: '16px',
          }}
        >
          <div>
            <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginBottom: '6px' }}>
              {t('totalPlugins')}
            </div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: 'var(--md-on-surface)' }}>
              {plugins.length}
            </div>
          </div>
          <div>
            <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginBottom: '6px' }}>
              {t('enabledPlugins')}
            </div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: 'var(--md-primary)' }}>
              {enabledCount}
            </div>
          </div>
          <div>
            <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginBottom: '6px' }}>
              {t('disabledPlugins')}
            </div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: 'var(--md-outline)' }}>
              {plugins.length - enabledCount}
            </div>
          </div>
        </div>
      </section>

      {/* 插件列表 */}
      <section style={sectionStyle}>
        <div style={{ padding: '20px 24px', background: 'var(--md-surface-container-low)' }}>
          <div style={{ fontSize: '15px', fontWeight: 700, color: 'var(--md-on-surface)' }}>
            {t('installedPlugins')}
          </div>
          <div style={{ fontSize: '12.5px', color: 'var(--md-outline)', marginTop: '4px' }}>
            {t('installedPluginsDesc')}
          </div>
        </div>
        <div style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
          {plugins.length === 0 ? (
            <div style={{ padding: '28px', textAlign: 'center', color: 'var(--md-outline)' }}>
              {t('noPlugins')}
            </div>
          ) : (
            plugins.map((p) => (
              <div key={p.id} style={CARD_STYLE}>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flexWrap: 'wrap' }}>
                    <strong style={{ fontSize: '15px', color: 'var(--md-on-surface)' }}>
                      {p.title}
                    </strong>
                    <span style={{ color: 'var(--md-on-surface-variant)', fontSize: '12px' }}>
                      v{p.version}
                    </span>
                    <span
                      style={{
                        padding: '2px 10px',
                        borderRadius: 'var(--radius-full)',
                        background: p.enabled
                          ? 'var(--md-primary-container)'
                          : 'var(--md-surface-container-high)',
                        color: p.enabled
                          ? 'var(--md-on-primary-container)'
                          : 'var(--md-on-surface-variant)',
                        fontSize: '12px',
                        fontWeight: 600,
                      }}
                    >
                      {p.enabled ? t('enabled') : t('disabled')}
                    </span>
                  </div>
                  {p.description && (
                    <p
                      style={{
                        margin: '6px 0 0',
                        color: 'var(--md-on-surface-variant)',
                        fontSize: '13px',
                        lineHeight: 1.5,
                      }}
                    >
                      {p.description}
                    </p>
                  )}
                  <p style={{ margin: '4px 0 0', color: 'var(--md-outline)', fontSize: '12px' }}>
                    {p.id}
                    {p.author ? ` · ${p.author}` : ''}
                  </p>
                </div>
                <div style={{ display: 'flex', gap: '8px', flexShrink: 0 }}>
                  {p.has_settings && (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => navigate(`/plugins/${p.id}/settings`)}
                    >
                      {t('settings')}
                    </Button>
                  )}
                  <Button
                    variant={p.enabled ? 'warning' : 'primary'}
                    size="sm"
                    onClick={() => toggleMutation.mutate(p.id)}
                    loading={toggleMutation.isPending}
                  >
                    {p.enabled ? t('disable') : t('enable')}
                  </Button>
                </div>
              </div>
            ))
          )}
        </div>
      </section>
    </div>
  );
}
