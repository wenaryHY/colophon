import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { apiData, API_PREFIX } from '../lib/api';
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
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const navigate = useNavigate();
  const toast = useToast();
  const { t } = useI18n();

  useEffect(() => {
    setLoading(true);
    apiData<{ plugins: PluginInfo[] }>(`${API_PREFIX}/admin/plugins`)
      .then((d) => setPlugins(d.plugins))
      .catch((e) => toast(e instanceof Error ? e.message : t('loadFailed', '加载失败'), 'error'))
      .finally(() => setLoading(false));
  }, []);

  const toggle = async (id: string) => {
    setTogglingId(id);
    try {
      await fetch(`${API_PREFIX}/admin/plugins/${id}/toggle`, {
        method: 'POST',
        credentials: 'include',
      });
      setPlugins((prev) =>
        prev.map((p) => (p.id === id ? { ...p, enabled: !p.enabled } : p)),
      );
    } catch (e) {
      toast(e instanceof Error ? e.message : '操作失败', 'error');
    } finally {
      setTogglingId(null);
    }
  };

  if (loading) {
    return (
      <div className="setup-loading">{t('loading', '加载中...')}</div>
    );
  }

  const enabledCount = plugins.filter((p) => p.enabled).length;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
      <PageHeader
        title={t('plugins')}
        subtitle={t('pluginsSubtitle', '管理已安装的插件，启用或禁用功能扩展')}
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
              {t('totalPlugins', '插件总数')}
            </div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: 'var(--md-on-surface)' }}>
              {plugins.length}
            </div>
          </div>
          <div>
            <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginBottom: '6px' }}>
              {t('enabledPlugins', '已启用')}
            </div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: 'var(--md-primary)' }}>
              {enabledCount}
            </div>
          </div>
          <div>
            <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginBottom: '6px' }}>
              {t('disabledPlugins', '已禁用')}
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
            {t('installedPlugins', '已安装插件')}
          </div>
          <div style={{ fontSize: '12.5px', color: 'var(--md-outline)', marginTop: '4px' }}>
            {t('installedPluginsDesc', '所有已扫描到的插件')}
          </div>
        </div>
        <div style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
          {plugins.length === 0 ? (
            <div style={{ padding: '28px', textAlign: 'center', color: 'var(--md-outline)' }}>
              {t('noPlugins', '暂无已安装的插件')}
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
                      {p.enabled ? t('enabled', '已启用') : t('disabled', '已禁用')}
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
                    onClick={() => toggle(p.id)}
                    loading={togglingId === p.id}
                  >
                    {p.enabled ? t('disable', '禁用') : t('enable', '启用')}
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
